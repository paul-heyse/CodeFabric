//! Fallible structural limits at native decode ingress.
//!
//! These checks precede deserialization. They bound decoded input geometry; they
//! do not replace allocation admission or retained backing receipts.

use crate::{DeltaResult, Error};

/// A native resource failure which optional-data paths must propagate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("native resource limit {kind}: requested {requested}, limit {limit}")]
pub struct ResourceExhausted {
    pub kind: &'static str,
    pub requested: usize,
    pub limit: usize,
}

/// Explicit finite JSON geometry, selected before native parser construction.
#[derive(Debug, Clone, Copy)]
pub struct JsonResourceLimits {
    pub max_bytes: usize,
    pub max_tokens: usize,
    pub max_depth: usize,
    pub max_string_bytes: usize,
    pub max_container_items: usize,
}

/// Source-derived counts for a subsequent native allocation admission decision.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JsonShape {
    pub bytes: usize,
    pub tokens: usize,
    pub string_bytes: usize,
    pub containers: usize,
    pub max_depth: usize,
}

fn check(kind: &'static str, requested: usize, limit: usize) -> DeltaResult<()> {
    if requested > limit {
        return Err(ResourceExhausted {
            kind,
            requested,
            limit,
        }
        .into());
    }
    Ok(())
}

impl JsonResourceLimits {
    /// Validate the finite policy before it can govern a native reader.
    pub fn validate(&self) -> DeltaResult<()> {
        for (kind, limit) in [
            ("json_bytes", self.max_bytes),
            ("json_tokens", self.max_tokens),
            ("json_depth", self.max_depth),
            ("json_string_bytes", self.max_string_bytes),
            ("json_container_items", self.max_container_items),
        ] {
            if limit == 0 {
                return Err(ResourceExhausted {
                    kind,
                    requested: 1,
                    limit,
                }
                .into());
            }
        }
        // The fixed scan stack is itself bounded, including invalid input.
        check("json_depth", self.max_depth, 128)
    }

    /// Inspect encoded geometry without heap allocation or decoded string copies.
    ///
    /// Native serde remains the sole grammar/semantic parser. This scanner counts
    /// value/key tokens, bounds all string source spans (an upper bound on UTF-8
    /// unescape output), and tracks each container separately. Malformed tokens
    /// are still passed to serde after bounded inspection. Whitespace inside a
    /// malformed primitive is counted conservatively; no valid input is hidden.
    pub fn inspect(&self, input: &[u8]) -> DeltaResult<JsonShape> {
        self.validate()?;
        check("json_bytes", input.len(), self.max_bytes)?;
        let mut shape = JsonShape {
            bytes: input.len(),
            ..JsonShape::default()
        };
        let mut items = [0usize; 128];
        let mut depth = 0usize;
        let mut at = 0usize;
        while at < input.len() {
            let byte = input[at];
            match byte {
                b' ' | b'\n' | b'\r' | b'\t' | b':' | b',' => {
                    at += 1;
                    continue;
                }
                b'}' | b']' => {
                    depth = depth.saturating_sub(1);
                    at += 1;
                    continue;
                }
                _ => {}
            }
            shape.tokens += 1; // At most input.len(), checked before the loop.
            check("json_tokens", shape.tokens, self.max_tokens)?;
            if depth > 0 {
                items[depth - 1] += 1;
                // Count both keys and values. This deliberately bounds the more
                // general token geometry, not an assumed map entry layout.
                check(
                    "json_container_items",
                    items[depth - 1],
                    self.max_container_items,
                )?;
            }
            match byte {
                b'{' | b'[' => {
                    depth += 1;
                    check("json_depth", depth, self.max_depth)?;
                    items[depth - 1] = 0;
                    shape.max_depth = shape.max_depth.max(depth);
                    shape.containers += 1;
                    at += 1;
                }
                b'"' => {
                    at += 1;
                    let start = at;
                    while at < input.len() && input[at] != b'"' {
                        if input[at] == b'\\' {
                            at += 1;
                            if at == input.len() {
                                break;
                            }
                        }
                        at += 1;
                    }
                    let len = at - start;
                    check("json_string_bytes", len, self.max_string_bytes)?;
                    shape.string_bytes += len; // Disjoint spans in input.
                    if at < input.len() {
                        at += 1;
                    }
                }
                _ => {
                    at += 1;
                    while at < input.len()
                        && !matches!(
                            input[at],
                            b' ' | b'\n'
                                | b'\r'
                                | b'\t'
                                | b':'
                                | b','
                                | b'['
                                | b'{'
                                | b']'
                                | b'}'
                                | b'"'
                        )
                    {
                        at += 1;
                    }
                }
            }
        }
        Ok(shape)
    }
}

/// Keep a resource failure while permitting corrupt optional native metadata to
/// retain its existing fallback semantics.
pub(crate) fn optional_native<T>(result: DeltaResult<T>) -> DeltaResult<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.is_resource_exhausted() => Err(error),
        Err(_) => Ok(None),
    }
}

/// Generic native allocation request. `bytes` is the entire new backing layout,
/// not a delta: an old reservation remains live during a moving reallocation.
#[derive(Debug, Clone, Copy)]
pub struct AllocationRequest {
    pub kind: &'static str,
    pub bytes: usize,
}

/// A caller-owned charge released by its concrete owner's Drop implementation.
pub trait AllocationReceipt: std::fmt::Debug + Send + Sync {
    fn bytes(&self) -> usize;
}

/// Application-neutral admission, backed by the same aggregate owner as the
/// caller's native decoder and execution reservations. Implementations must not
/// reenter the scope which invokes them.
pub trait AllocationAdmission: std::fmt::Debug + Send + Sync {
    fn try_reserve(
        &self,
        request: AllocationRequest,
    ) -> Result<std::sync::Arc<dyn AllocationReceipt>, ResourceExhausted>;
}

/// Finite native receipt storage. Its bookkeeping is admitted before allocation.
///
/// Calls to `reserve` conservatively retain every charge in the finite bank;
/// `reserve_transient` instead returns a separate phase-lifetime receipt.
/// A clone may accompany shared backing only; a deep copy must first call
/// `reserve` with its full new allocation geometry. The last native backing owner
/// must retain this scope, not merely the task which happened to create it.
pub struct NativeResourceScope {
    admission: std::sync::Arc<dyn AllocationAdmission>,
    receipts: std::sync::Mutex<ReceiptSlots>,
    failure: std::sync::Mutex<Option<ResourceExhausted>>,
    allocations_sealed: std::sync::atomic::AtomicBool,
    _bookkeeping: std::sync::Arc<dyn AllocationReceipt>,
}

impl std::fmt::Debug for NativeResourceScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Native diagnostics must never recursively format a budget, receipt
        // bank, adopted predecessor graph, or caller's admission implementation.
        f.write_str("NativeResourceScope { .. }")
    }
}

#[derive(Debug)]
struct ReceiptSlots {
    slots: Box<[Option<std::sync::Arc<dyn AllocationReceipt>>]>,
    used: usize,
}

impl NativeResourceScope {
    /// Remember the first native exhaustion even across Option-based interfaces
    /// that cannot carry errors (for example DataFusion PruningStatistics).
    /// This never allocates and cannot clear an earlier failure.
    pub fn record_failure(&self, error: ResourceExhausted) {
        if let Ok(mut failure) = self.failure.lock() {
            failure.get_or_insert(error);
        }
    }

    pub fn failure(&self) -> Option<ResourceExhausted> {
        match self.failure.lock() {
            Ok(failure) => *failure,
            Err(_) => Some(ResourceExhausted {
                kind: "native_failure_state",
                requested: 1,
                limit: 0,
            }),
        }
    }

    /// Check before native side effects and before returning success. A failed
    /// phase cannot resume because transient pressure happened to subside later.
    pub fn check_available(&self) -> DeltaResult<()> {
        match self.failure() {
            Some(error) => Err(error.into()),
            None => Ok(()),
        }
    }

    /// Freeze admission after every native worker using this scope has joined.
    /// Existing backing remains valid and retains its receipts; a subsequent
    /// native operation must allocate against its own current scope.
    pub fn seal_allocations(&self) {
        self.allocations_sealed
            .store(true, std::sync::atomic::Ordering::Release);
    }

    pub fn allocations_sealed(&self) -> bool {
        self.allocations_sealed
            .load(std::sync::atomic::Ordering::Acquire)
    }

    fn check_allocation_available(&self) -> DeltaResult<()> {
        self.check_available()?;
        if self.allocations_sealed() {
            return Err(ResourceExhausted {
                kind: "native allocation scope has joined",
                requested: 1,
                limit: 0,
            }
            .into());
        }
        Ok(())
    }

    fn remember_result<T>(&self, result: &DeltaResult<T>) {
        if let Err(Error::ResourceExhausted(error)) = result {
            self.record_failure(*error);
        }
    }

    /// Admit a temporary synchronous allocation phase before its first allocation.
    /// The returned owner must outlive every allocation in that phase. Retained
    /// output needs a separate reservation admitted before the phase starts; it
    /// cannot acquire its first charge after decoding has already allocated it.
    pub fn reserve_transient(
        &self,
        request: AllocationRequest,
    ) -> DeltaResult<std::sync::Arc<dyn AllocationReceipt>> {
        self.check_allocation_available()?;
        let result = self.reserve_transient_inner(request);
        self.remember_result(&result);
        result
    }

    fn reserve_transient_inner(
        &self,
        request: AllocationRequest,
    ) -> DeltaResult<std::sync::Arc<dyn AllocationReceipt>> {
        check(
            "native_allocation_layout",
            request.bytes,
            isize::MAX as usize,
        )?;
        let receipt = self.admission.try_reserve(request)?;
        check(
            "native_receipt_receipt_bytes",
            request.bytes,
            receipt.bytes(),
        )?;
        Ok(receipt)
    }

    pub fn try_new(
        admission: std::sync::Arc<dyn AllocationAdmission>,
        max_reservations: usize,
    ) -> DeltaResult<std::sync::Arc<Self>> {
        if max_reservations == 0 {
            return Err(ResourceExhausted {
                kind: "native_receipt_slots",
                requested: 1,
                limit: 0,
            }
            .into());
        }
        let bytes = max_reservations
            .checked_mul(std::mem::size_of::<
                Option<std::sync::Arc<dyn AllocationReceipt>>,
            >())
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Self>()))
            .and_then(|bytes| bytes.checked_add(2 * std::mem::size_of::<usize>()))
            .ok_or(ResourceExhausted {
                kind: "native_receipt_bookkeeping",
                requested: usize::MAX,
                limit: isize::MAX as usize,
            })?;
        check("native_receipt_bookkeeping", bytes, isize::MAX as usize)?;
        let bookkeeping = admission.try_reserve(AllocationRequest {
            kind: "native_receipt_bookkeeping",
            bytes,
        })?;
        check("native_receipt_receipt_bytes", bytes, bookkeeping.bytes())?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(max_reservations)
            .map_err(|_| ResourceExhausted {
                kind: "native_receipt_allocator",
                requested: bytes,
                limit: 0,
            })?;
        slots.resize_with(max_reservations, || None);
        Ok(std::sync::Arc::new(Self {
            admission,
            receipts: std::sync::Mutex::new(ReceiptSlots {
                slots: slots.into_boxed_slice(),
                used: 0,
            }),
            failure: std::sync::Mutex::new(None),
            allocations_sealed: std::sync::atomic::AtomicBool::new(false),
            _bookkeeping: bookkeeping,
        }))
    }

    /// Admit and retain the full new backing before a native allocator is called.
    /// Old allocations stay charged, including when realloc cannot grow in place.
    pub fn reserve(&self, request: AllocationRequest) -> DeltaResult<()> {
        self.check_allocation_available()?;
        let result = self.reserve_inner(request);
        self.remember_result(&result);
        result
    }

    fn reserve_inner(&self, request: AllocationRequest) -> DeltaResult<()> {
        check(
            "native_allocation_layout",
            request.bytes,
            isize::MAX as usize,
        )?;
        let mut receipts = self.receipts.lock().map_err(|_| ResourceExhausted {
            kind: "native_receipt_state",
            requested: 1,
            limit: 0,
        })?;
        let index = receipts.used;
        check("native_receipt_slots", index + 1, receipts.slots.len())?;
        let receipt = self.admission.try_reserve(request)?;
        check(
            "native_receipt_receipt_bytes",
            request.bytes,
            receipt.bytes(),
        )?;
        receipts.slots[index] = Some(receipt);
        receipts.used += 1;
        Ok(())
    }
}

#[derive(Clone)]
struct NativeResourceContext {
    scope: Option<std::sync::Arc<NativeResourceScope>>,
    json: Option<JsonResourceLimits>,
}

thread_local! {
    static NATIVE_RESOURCES: std::cell::RefCell<Option<NativeResourceContext>> = const { std::cell::RefCell::new(None) };
    static REQUIRED_NATIVE_RESOURCES: std::cell::RefCell<Option<NativeResourceContext>> = const { std::cell::RefCell::new(None) };
}

/// A validated policy for an exclusively owned native worker thread. All work
/// on the thread, including blocking descendants, belongs to the same operation.
/// Install it through the runtime's worker start/stop lifecycle, never across an
/// await on a shared executor thread.
#[derive(Clone)]
pub struct NativeResourceThreadPolicy {
    context: NativeResourceContext,
}

impl NativeResourceThreadPolicy {
    pub fn try_new(
        scope: std::sync::Arc<NativeResourceScope>,
        json: JsonResourceLimits,
    ) -> DeltaResult<Self> {
        json.validate()?;
        Ok(Self {
            context: NativeResourceContext {
                scope: Some(scope),
                json: Some(json),
            },
        })
    }

    /// Require this owner for the entire lifetime of one dedicated worker.
    /// The guard cannot move to another thread, and nested installs restore the
    /// previous policy. Explicit decoder options cannot weaken this requirement.
    pub fn enter_thread(&self) -> NativeResourceThreadGuard {
        let previous =
            REQUIRED_NATIVE_RESOURCES.with(|current| current.replace(Some(self.context.clone())));
        NativeResourceThreadGuard {
            previous,
            _same_thread: std::marker::PhantomData,
        }
    }
}

/// Lifetime guard for an exclusively owned native worker.
///
/// ```compile_fail
/// use buoyant_kernel::resource::NativeResourceThreadGuard;
/// fn needs_send<T: Send>() {}
/// needs_send::<NativeResourceThreadGuard>();
/// ```
pub struct NativeResourceThreadGuard {
    previous: Option<NativeResourceContext>,
    _same_thread: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl Drop for NativeResourceThreadGuard {
    fn drop(&mut self) {
        REQUIRED_NATIVE_RESOURCES.with(|current| {
            current.replace(self.previous.take());
        });
    }
}

pub(crate) fn ensure_required_scope(
    scope: &std::sync::Arc<NativeResourceScope>,
) -> DeltaResult<()> {
    let required = REQUIRED_NATIVE_RESOURCES.with(|current| {
        current
            .borrow()
            .as_ref()
            .and_then(|value| value.scope.clone())
    });
    if required.is_some_and(|required| !std::sync::Arc::ptr_eq(scope, &required)) {
        return Err(ResourceExhausted {
            kind: "native_foreign_scope",
            requested: 1,
            limit: 0,
        }
        .into());
    }
    Ok(())
}

/// The native scope of the current synchronous allocation phase, if governed.
pub fn current_resource_scope() -> Option<std::sync::Arc<NativeResourceScope>> {
    if let Some(scope) = REQUIRED_NATIVE_RESOURCES.with(|current| {
        current
            .borrow()
            .as_ref()
            .and_then(|value| value.scope.clone())
    }) {
        return Some(scope);
    }
    NATIVE_RESOURCES.with(|current| {
        current
            .borrow()
            .as_ref()
            .and_then(|value| value.scope.clone())
    })
}

/// Geometry selected for the current synchronous native decode phase.
pub fn current_json_resource_limits() -> Option<JsonResourceLimits> {
    if let Some(limits) = REQUIRED_NATIVE_RESOURCES
        .with(|current| current.borrow().as_ref().and_then(|value| value.json))
    {
        return Some(limits);
    }
    NATIVE_RESOURCES.with(|current| current.borrow().as_ref().and_then(|value| value.json))
}

pub(crate) struct NativeResourceGuard {
    previous: Option<NativeResourceContext>,
    // A thread-local guard must never migrate to a different worker.
    _same_thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl Drop for NativeResourceGuard {
    fn drop(&mut self) {
        NATIVE_RESOURCES.with(|current| {
            current.replace(self.previous.take());
        });
    }
}

pub(crate) fn enter_resource_scope(
    mut scope: Option<std::sync::Arc<NativeResourceScope>>,
    mut json: Option<JsonResourceLimits>,
) -> DeltaResult<NativeResourceGuard> {
    if let Some(required) = REQUIRED_NATIVE_RESOURCES.with(|current| current.borrow().clone()) {
        if let (Some(explicit), Some(required_scope)) = (&scope, &required.scope) {
            if !std::sync::Arc::ptr_eq(explicit, required_scope) {
                return Err(ResourceExhausted {
                    kind: "native_foreign_scope",
                    requested: 1,
                    limit: 0,
                }
                .into());
            }
        }
        scope = required.scope;
        json = required.json;
    }
    if scope.is_some() && json.is_none() {
        return Err(ResourceExhausted {
            kind: "native_json_policy",
            requested: 1,
            limit: 0,
        }
        .into());
    }
    if let Some(limits) = json {
        limits.validate()?;
    }
    if current_resource_scope().is_some() && scope.is_none() {
        return Err(ResourceExhausted {
            kind: "native_scope_downgrade",
            requested: 1,
            limit: 0,
        }
        .into());
    }
    let previous = NATIVE_RESOURCES
        .with(|current| current.replace(Some(NativeResourceContext { scope, json })));
    Ok(NativeResourceGuard {
        previous,
        _same_thread: std::marker::PhantomData,
    })
}

/// Run one synchronous native allocation phase with explicit scope and geometry.
///
/// The closure must perform the work synchronously, never return an unpolled
/// future or use this scope across an await. Async handlers carry the explicit
/// policy to each synchronous decode closure instead. Nesting and unwinding
/// restore the previous worker-local context.
pub fn with_resource_scope<T>(
    scope: std::sync::Arc<NativeResourceScope>,
    json: JsonResourceLimits,
    work: impl FnOnce() -> DeltaResult<T>,
) -> DeltaResult<T> {
    let _guard = enter_resource_scope(Some(scope.clone()), Some(json))?;
    scope.check_available()?;
    let result = work();
    // Option-based library contracts can only report unavailable facts. Their
    // loss of the local error must not erase terminal resource exhaustion.
    scope.check_available()?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> JsonResourceLimits {
        JsonResourceLimits {
            max_bytes: 4096,
            max_tokens: 128,
            max_depth: 16,
            max_string_bytes: 64,
            max_container_items: 32,
        }
    }

    #[test]
    fn native_json_geometry_precedes_decode() {
        let mut limits = limits();
        limits.max_string_bytes = 3;
        let mut called = false;
        let result = limits.inspect(br#"{"x":"oversized"}"#).and_then(|_| {
            called = true;
            Ok(())
        });
        assert!(result.unwrap_err().is_resource_exhausted());
        assert!(!called);
    }

    #[test]
    fn native_json_counts_escaped_strings_without_decoding() {
        let shape = limits()
            .inspect(br#"{"x":["a\"b","\u0061",null]}"#)
            .unwrap();
        assert_eq!(shape.tokens, 6);
        assert_eq!(shape.containers, 2);
        assert_eq!(shape.max_depth, 2);
        assert_eq!(shape.string_bytes, 11);
    }

    #[test]
    fn native_json_rejects_depth_count_and_invalid_policy_before_parser() {
        let mut limits = limits();
        limits.max_depth = 2;
        assert!(
            limits
                .inspect(b"[[[]]]")
                .unwrap_err()
                .is_resource_exhausted()
        );
        limits.max_container_items = 2;
        assert!(
            limits
                .inspect(b"[0,0,0]")
                .unwrap_err()
                .is_resource_exhausted()
        );
        limits.max_bytes = 0;
        assert!(limits.inspect(b"{}").unwrap_err().is_resource_exhausted());
    }

    #[test]
    fn optional_metadata_preserves_typed_resource_failure() {
        let error = Error::ResourceExhausted(ResourceExhausted {
            kind: "json_bytes",
            requested: 2,
            limit: 1,
        })
        .with_backtrace();
        assert!(
            optional_native::<()>(Err(error))
                .unwrap_err()
                .is_resource_exhausted()
        );
        assert!(
            optional_native::<()>(Err(Error::generic("invalid optional metadata")))
                .unwrap()
                .is_none()
        );
        assert_eq!(optional_native(Ok(3)).unwrap(), Some(3));
    }
}
