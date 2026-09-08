// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements. See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership. The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License. You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. See the License for the
// specific language governing permissions and limitations
// under the License.

//! Opt-in fallible admission for native reader allocations.
//!
//! This is an incremental reader facility, not a general allocator or an RSS limit.
//! A policy retains every successful reservation until its last owner is dropped.
//! Growth requests charge the complete new allocation while keeping the previous
//! reservation: replacement/reallocation peaks are never charged as a size delta.

use std::fmt::{Debug, Display, Formatter};
use std::sync::{Arc, Mutex};

/// A finite, allocation-free resource failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceExhausted {
    /// Allocation or structural bound that was denied.
    pub kind: &'static str,
    /// Requested bytes or elements, according to `kind`.
    pub requested: usize,
    /// Applicable finite ceiling.
    pub limit: usize,
}
impl Display for ResourceExhausted {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} requires {}, limit {}",
            self.kind, self.requested, self.limit
        )
    }
}
impl std::error::Error for ResourceExhausted {}

/// A reservation released by its concrete owner's destructor.
pub trait ResourceReceipt: Debug + Send + Sync {
    /// Number of bytes admitted by this reservation.
    fn bytes(&self) -> usize;
}

/// One allocation's complete new capacity, including reallocation peaks.
#[derive(Clone, Copy, Debug)]
pub struct ResourceRequest {
    /// Required new allocation capacity, not its delta from an existing allocation.
    pub bytes: usize,
    /// Static diagnostic category.
    pub kind: &'static str,
}

/// Application-owned shared budget. Returning `Err` must not consume capacity.
pub trait ResourceAdmission: Debug + Send + Sync {
    /// Admit before allocating. A successful receipt must cover the requested bytes.
    fn try_reserve(
        &self,
        request: ResourceRequest,
    ) -> Result<Arc<dyn ResourceReceipt>, ResourceExhausted>;
}

/// Explicit finite JSON reader geometry.
#[derive(Clone, Copy, Debug)]
pub struct ReaderResourceLimits {
    /// Maximum reservations retained by this policy.
    pub allocations: usize,
    /// Maximum entries in any tape, positions, or output collection.
    pub collection_entries: usize,
    /// Maximum bytes in the decoded string tape or one output string buffer.
    pub string_bytes: usize,
    /// Maximum native decoder stack entries.
    pub nesting: usize,
}

struct PolicyOwner {
    admission: Arc<dyn ResourceAdmission>,
    limits: ReaderResourceLimits,
    receipts: Mutex<Vec<Arc<dyn ResourceReceipt>>>,
    _registry_receipt: Arc<dyn ResourceReceipt>,
}

/// Shared lifetime of admitted native work and retained reader backing.
#[derive(Clone)]
pub struct ReaderResourcePolicy(Arc<PolicyOwner>);
impl Debug for ReaderResourcePolicy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReaderResourcePolicy")
            .field("limits", &self.0.limits)
            .finish()
    }
}
impl ReaderResourcePolicy {
    /// Admit the finite reservation registry before allocating it.
    pub fn try_new(
        admission: Arc<dyn ResourceAdmission>,
        limits: ReaderResourceLimits,
    ) -> Result<Self, ResourceExhausted> {
        for (kind, value) in [
            ("JSON reservation count", limits.allocations),
            ("JSON collection entries", limits.collection_entries),
            ("JSON string bytes", limits.string_bytes),
            ("JSON nesting", limits.nesting),
        ] {
            if value == 0 {
                return Err(ResourceExhausted {
                    kind,
                    requested: 1,
                    limit: 0,
                });
            }
            Self::check(value, isize::MAX as usize, kind)?;
        }
        Self::check(
            limits.collection_entries,
            u32::MAX as usize - 1,
            "JSON tape index geometry",
        )?;
        let bytes = limits
            .allocations
            .checked_mul(std::mem::size_of::<Arc<dyn ResourceReceipt>>())
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<PolicyOwner>()))
            .and_then(|bytes| bytes.checked_add(2 * std::mem::size_of::<usize>()))
            .ok_or(ResourceExhausted {
                kind: "reservation registry",
                requested: usize::MAX,
                limit: usize::MAX - 1,
            })?;
        Self::check(bytes, isize::MAX as usize, "JSON allocation layout")?;
        let receipt = admission.try_reserve(ResourceRequest {
            bytes,
            kind: "reservation registry",
        })?;
        if receipt.bytes() < bytes {
            return Err(ResourceExhausted {
                kind: "short reservation receipt",
                requested: bytes,
                limit: receipt.bytes(),
            });
        }
        let mut receipts = Vec::new();
        receipts
            .try_reserve_exact(limits.allocations)
            .map_err(|_| ResourceExhausted {
                kind: "JSON registry allocation",
                requested: bytes,
                limit: 0,
            })?;
        Ok(Self(Arc::new(PolicyOwner {
            admission,
            limits,
            receipts: Mutex::new(receipts),
            _registry_receipt: receipt,
        })))
    }

    /// The immutable, finite structural limits.
    pub fn limits(&self) -> ReaderResourceLimits {
        self.0.limits
    }

    /// Reserve a complete new allocation; previous receipts remain held.
    pub(crate) fn reserve(
        &self,
        bytes: usize,
        kind: &'static str,
    ) -> Result<(), ResourceExhausted> {
        if bytes == 0 {
            return Ok(());
        }
        Self::check(bytes, isize::MAX as usize, "JSON allocation layout")?;
        let mut receipts = self.0.receipts.lock().map_err(|_| ResourceExhausted {
            kind: "reservation registry unavailable",
            requested: 1,
            limit: 0,
        })?;
        Self::check(
            receipts.len().saturating_add(1),
            self.0.limits.allocations,
            "reservation count",
        )?;
        let receipt = self
            .0
            .admission
            .try_reserve(ResourceRequest { bytes, kind })?;
        if receipt.bytes() < bytes {
            return Err(ResourceExhausted {
                kind: "short reservation receipt",
                requested: bytes,
                limit: receipt.bytes(),
            });
        }
        receipts.push(receipt);
        Ok(())
    }

    pub(crate) fn check(
        value: usize,
        limit: usize,
        kind: &'static str,
    ) -> Result<(), ResourceExhausted> {
        if value > limit {
            Err(ResourceExhausted {
                kind,
                requested: value,
                limit,
            })
        } else {
            Ok(())
        }
    }
}

impl From<ResourceExhausted> for arrow_schema::ArrowError {
    fn from(error: ResourceExhausted) -> Self {
        Self::ResourceOwnerError(arrow_schema::resource::ResourceOwnerError {
            kind: error.kind,
            requested: error.requested,
            limit: error.limit,
        })
    }
}

thread_local! {
    // Synchronous allocation context only; every public decode/build boundary installs its
    // explicitly supplied policy and restores the previous context before returning.
    static CURRENT: std::cell::RefCell<Option<ReaderResourcePolicy>> = const { std::cell::RefCell::new(None) };
}
struct Scope(Option<ReaderResourcePolicy>);
impl Drop for Scope {
    fn drop(&mut self) {
        CURRENT.with(|current| {
            current.replace(self.0.take());
        });
    }
}
pub(crate) fn scoped<T>(policy: Option<&ReaderResourcePolicy>, operation: impl FnOnce() -> T) -> T {
    let policy = required_policy().or_else(|| policy.cloned());
    let _scope = Scope(CURRENT.with(|current| current.replace(policy)));
    operation()
}
pub(crate) fn current() -> Option<ReaderResourcePolicy> {
    CURRENT.with(|current| current.borrow().clone())
}
pub(crate) fn reserve(bytes: usize, kind: &'static str) -> Result<(), arrow_schema::ArrowError> {
    if let Some(policy) = current() {
        policy.reserve(bytes, kind)?;
    }
    Ok(())
}
pub(crate) fn checked_mul(
    a: usize,
    b: usize,
    kind: &'static str,
) -> Result<usize, arrow_schema::ArrowError> {
    a.checked_mul(b).ok_or_else(|| {
        ResourceExhausted {
            kind,
            requested: usize::MAX,
            limit: usize::MAX - 1,
        }
        .into()
    })
}
pub(crate) fn checked_add(
    a: usize,
    b: usize,
    kind: &'static str,
) -> Result<usize, arrow_schema::ArrowError> {
    a.checked_add(b).ok_or_else(|| {
        ResourceExhausted {
            kind,
            requested: usize::MAX,
            limit: usize::MAX - 1,
        }
        .into()
    })
}
fn limit(policy: &ReaderResourcePolicy, kind: &'static str) -> usize {
    match kind {
        "JSON string tape" | "JSON string backing" => policy.limits().string_bytes,
        "JSON decoder stack" => policy.limits().nesting,
        _ => policy.limits().collection_entries,
    }
}
pub(crate) fn new_vec<T>(
    capacity: usize,
    kind: &'static str,
) -> Result<Vec<T>, arrow_schema::ArrowError> {
    if let Some(policy) = current() {
        ReaderResourcePolicy::check(capacity, limit(&policy, kind), kind)?;
        policy.reserve(
            checked_add(
                checked_mul(capacity, std::mem::size_of::<T>(), kind)?,
                buffer_owner_bytes(),
                kind,
            )?,
            kind,
        )?;
    }
    let mut value = Vec::new();
    value
        .try_reserve_exact(capacity)
        .map_err(|_| ResourceExhausted {
            kind: "JSON vector allocation",
            requested: capacity,
            limit: 0,
        })?;
    Ok(value)
}
pub(crate) fn reserve_vec<T>(
    value: &mut Vec<T>,
    additional: usize,
    kind: &'static str,
) -> Result<(), arrow_schema::ArrowError> {
    if let Some(policy) = current() {
        let required = checked_add(value.len(), additional, kind)?;
        let limit = limit(&policy, kind);
        ReaderResourcePolicy::check(required, limit, kind)?;
        if required > value.capacity() {
            let capacity = required.max(value.capacity().saturating_mul(2)).min(limit);
            policy.reserve(
                checked_add(
                    checked_mul(capacity, std::mem::size_of::<T>(), kind)?,
                    buffer_owner_bytes(),
                    kind,
                )?,
                kind,
            )?;
            value
                .try_reserve_exact(capacity - value.len())
                .map_err(|_| ResourceExhausted {
                    kind: "JSON allocation failed",
                    requested: capacity,
                    limit: 0,
                })?;
        }
    }
    Ok(())
}
pub(crate) fn buffer(
    count: usize,
    width: usize,
    kind: &'static str,
) -> Result<(), arrow_schema::ArrowError> {
    if let Some(policy) = current() {
        ReaderResourcePolicy::check(count, limit(&policy, kind), kind)?;
        let bytes = checked_mul(count, width, kind)?;
        // MutableBuffer rounds to 64 bytes. Each buffer can also acquire one lifetime box.
        let rounded = checked_add(bytes, 63, kind)? / 64 * 64;
        policy.reserve(checked_add(rounded, buffer_owner_bytes(), kind)?, kind)?;
    }
    Ok(())
}
pub(crate) fn nulls(rows: usize) -> Result<(), arrow_schema::ArrowError> {
    buffer(rows.div_ceil(8), 1, "JSON validity bitmap")
}
pub(crate) fn boxed<T: super::reader::ArrayDecoder + 'static>(
    value: T,
) -> Result<Box<dyn super::reader::ArrayDecoder>, arrow_schema::ArrowError> {
    reserve(std::mem::size_of::<T>(), "JSON decoder node")?;
    Ok(Box::new(value))
}
pub(crate) fn array<T: arrow_array::Array + 'static>(
    value: T,
) -> Result<arrow_array::ArrayRef, arrow_schema::ArrowError> {
    reserve(
        std::mem::size_of::<T>() + 2 * std::mem::size_of::<usize>(),
        "JSON array owner",
    )?;
    Ok(Arc::new(value))
}
#[derive(Debug)]
struct BackingOwner {
    _policy: ReaderResourcePolicy,
    bytes: usize,
}
impl arrow_buffer::MemoryReservation for BackingOwner {
    fn size(&self) -> usize {
        self.bytes
    }
    fn resize(&mut self, bytes: usize) {
        self.bytes = bytes;
    }
}
pub(crate) fn claim(array: &dyn arrow_array::Array) -> Result<(), arrow_schema::ArrowError> {
    let Some(policy) = current() else {
        return Ok(());
    };
    array.try_visit_buffers(&mut |buffer| {
        buffer
            .try_add_claim(
                policy.limits().allocations,
                |layout| policy.reserve(layout.size(), "JSON combined backing owner"),
                |bytes| {
                    policy.reserve(std::mem::size_of::<BackingOwner>(), "JSON backing lifetime")?;
                    Ok(Box::new(BackingOwner {
                        _policy: policy.clone(),
                        bytes,
                    }))
                },
            )
            .map_err(|error| match error {
                arrow_buffer::BufferClaimError::Admission(error) => {
                    arrow_schema::ArrowError::from(error)
                }
                arrow_buffer::BufferClaimError::Capacity { requested, limit } => {
                    ResourceExhausted {
                        kind: "JSON combined backing count",
                        requested,
                        limit,
                    }
                    .into()
                }
            })
    })
}
/// Exact native Arrow owner layout; payload and receipt boxes are admitted separately.
fn buffer_owner_bytes() -> usize {
    arrow_buffer::allocation_owner_size()
}
pub(crate) fn schema_fields(
    fields: &arrow_schema::Fields,
    depth: usize,
) -> Result<usize, arrow_schema::ArrowError> {
    if let Some(policy) = current() {
        ReaderResourcePolicy::check(depth, policy.limits().nesting, "JSON schema nesting")?;
        ReaderResourcePolicy::check(
            fields.len(),
            policy.limits().collection_entries,
            "JSON schema fields",
        )?;
    }
    let mut count = fields.len();
    for field in fields {
        let child = match field.data_type() {
            arrow_schema::DataType::Struct(children) => schema_fields(children, depth + 1)?,
            arrow_schema::DataType::List(child)
            | arrow_schema::DataType::LargeList(child)
            | arrow_schema::DataType::ListView(child)
            | arrow_schema::DataType::LargeListView(child)
            | arrow_schema::DataType::FixedSizeList(child, _)
            | arrow_schema::DataType::Map(child, _) => {
                // Field recursion without allocating an intermediate Fields collection.
                schema_field(child, depth + 1)?
            }
            _ => 0,
        };
        count = checked_add(count, child, "JSON schema fields")?;
    }
    Ok(count)
}
fn schema_field(
    field: &arrow_schema::Field,
    depth: usize,
) -> Result<usize, arrow_schema::ArrowError> {
    if let Some(policy) = current() {
        ReaderResourcePolicy::check(depth, policy.limits().nesting, "JSON schema nesting")?;
    }
    let nested = match field.data_type() {
        arrow_schema::DataType::Struct(children) => schema_fields(children, depth + 1)?,
        arrow_schema::DataType::List(child)
        | arrow_schema::DataType::LargeList(child)
        | arrow_schema::DataType::ListView(child)
        | arrow_schema::DataType::LargeListView(child)
        | arrow_schema::DataType::FixedSizeList(child, _)
        | arrow_schema::DataType::Map(child, _) => schema_field(child, depth + 1)?,
        _ => 0,
    };
    checked_add(1, nested, "JSON schema fields")
}
pub(crate) fn byte_buffer(bytes: usize) -> Result<(), arrow_schema::ArrowError> {
    if let Some(policy) = current() {
        ReaderResourcePolicy::check(bytes, policy.limits().string_bytes, "JSON string backing")?;
    }
    buffer(bytes, 1, "JSON string backing")
}
pub(crate) fn offsets<O>(rows: usize) -> Result<(), arrow_schema::ArrowError> {
    buffer(
        checked_add(rows, 1, "JSON offsets")?,
        std::mem::size_of::<O>(),
        "JSON offsets",
    )
}
pub(crate) fn view_buffers(rows: usize, data: usize) -> Result<u32, arrow_schema::ArrowError> {
    let block = u32::try_from(data.max(1)).map_err(|_| ResourceExhausted {
        kind: "JSON view block",
        requested: data,
        limit: u32::MAX as usize,
    })?;
    buffer(rows, 16, "JSON views")?;
    nulls(rows)?;
    if data != 0 {
        byte_buffer(data.max(8))?; // Vec<u8>'s minimum first allocation.
        // Exactly one completed block, Vec<Buffer>'s first allocation is four entries.
        reserve(
            4 * std::mem::size_of::<arrow_buffer::Buffer>(),
            "JSON completed view blocks",
        )?;
    }
    // GenericByteViewArray converts completed Vec<Buffer> to Arc<[Buffer]>,
    // including an Arc header for the empty block list.
    reserve(
        2 * std::mem::size_of::<usize>()
            + usize::from(data != 0) * std::mem::size_of::<arrow_buffer::Buffer>(),
        "JSON shared view blocks",
    )?;
    Ok(block)
}

/// Bounded native diagnostic formatting. The payload is admitted before heap allocation.
pub(crate) fn json_error(args: std::fmt::Arguments<'_>) -> arrow_schema::ArrowError {
    if current().is_none() {
        return arrow_schema::ArrowError::JsonError(args.to_string());
    }
    struct Text {
        bytes: [u8; 512],
        len: usize,
    }
    impl std::fmt::Write for Text {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            let mut n = text.len().min(self.bytes.len() - self.len);
            while !text.is_char_boundary(n) {
                n -= 1;
            }
            self.bytes[self.len..self.len + n].copy_from_slice(&text.as_bytes()[..n]);
            self.len += n;
            if n != text.len() {
                Err(std::fmt::Error)
            } else {
                Ok(())
            }
        }
    }
    let mut text = Text {
        bytes: [0; 512],
        len: 0,
    };
    let _ = std::fmt::write(&mut text, args);
    if let Err(error) = reserve(text.len, "JSON diagnostic") {
        return error;
    }
    arrow_schema::ArrowError::JsonError(
        std::str::from_utf8(&text.bytes[..text.len])
            .unwrap()
            .to_owned(),
    )
}

/// Reserve the pinned arrow-cast parser's possible error String before calling it.
/// parse_decimal, string_to_datetime and timezone parsing produce at most one native
/// formatted String, with one input copy and fewer than 128 literal/context bytes.
/// Rust 1.98 format initially reserves <= 2 * literal bytes and String growth doubles;
/// summing all replacement capacities is strictly below 4 * (input + 256).
/// This admits the failure allocation even when a caller discards the parser error.
pub(crate) fn parser_error_capacity(input: usize) -> Result<(), arrow_schema::ArrowError> {
    reserve(
        checked_mul(
            checked_add(input, 256, "JSON scalar parser error")?,
            4,
            "JSON scalar parser error",
        )?,
        "JSON scalar parser error",
    )
}

#[cfg(test)]
#[path = "resource_tests.rs"]
mod tests;

/// Native collection constructors format at most one field name using Debug,
/// whose escaping takes at most ten bytes per input byte, plus < 256 other bytes.
pub(crate) fn collection_error_capacity(name: &str) -> Result<(), arrow_schema::ArrowError> {
    parser_error_capacity(checked_mul(
        name.len(),
        10,
        "JSON collection constructor error",
    )?)
}

pub(crate) fn finish_metadata(reset_offset_width: usize) -> Result<(), arrow_schema::ArrowError> {
    // Native ArrayDataBuilder::add_buffer grows an empty Vec<Buffer> to four
    // entries. GenericByteBuilder additionally resets its offsets Vec with one
    // push, whose new native capacity is four elements.
    reserve(
        4 * std::mem::size_of::<arrow_buffer::Buffer>() + 4 * reset_offset_width,
        "JSON builder finish metadata",
    )
}

pub(crate) fn is_resource_error(error: &arrow_schema::ArrowError) -> bool {
    matches!(error, arrow_schema::ArrowError::ResourceOwnerError(_))
        || matches!(error, arrow_schema::ArrowError::ExternalError(source) if source.is::<ResourceExhausted>())
}

#[derive(Debug)]
struct ReaderFailure {
    source: arrow_schema::ArrowError,
    _policy: ReaderResourcePolicy,
}
impl Display for ReaderFailure {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.source, f)
    }
}
impl std::error::Error for ReaderFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
/// Preserve admitted native error backing when an error outlives its decoder.
/// Resource exhaustion is an inline fixed-size Arrow control outcome; other
/// native reader errors keep the explicit policy owner.
pub(crate) fn own_error(error: arrow_schema::ArrowError) -> arrow_schema::ArrowError {
    let Some(policy) = current() else {
        return error;
    };
    if is_resource_error(&error) {
        return error;
    }
    if let Err(denied) = policy.reserve(
        std::mem::size_of::<ReaderFailure>(),
        "JSON retained error owner",
    ) {
        return denied.into();
    }
    arrow_schema::ArrowError::ExternalError(Box::new(ReaderFailure {
        _policy: policy,
        source: error,
    }))
}

thread_local! {
    static REQUIRED_POLICY: std::cell::RefCell<Option<ReaderResourcePolicy>> = const { std::cell::RefCell::new(None) };
}
/// A policy installed for one dedicated native worker's lifetime. This guard is
/// not Send and must be dropped on the thread that installed it. Do not use it
/// around a shared-runtime async future: install it in dedicated worker lifecycle
/// hooks, and use an ordinary lexical guard on the dedicated coordinator thread.
///
/// ```compile_fail
/// fn requires_send<T: Send>() {}
/// requires_send::<arrow_json::resource::ReaderThreadGuard>();
/// ```
#[derive(Debug)]
pub struct ReaderThreadGuard {
    previous: Option<ReaderResourcePolicy>,
    _same_thread: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl Drop for ReaderThreadGuard {
    fn drop(&mut self) {
        REQUIRED_POLICY.with(|current| {
            current.replace(self.previous.take());
        });
    }
}
impl ReaderResourcePolicy {
    /// Current required worker owner, or the current synchronous decode owner.
    /// This is a lifetime handle; it does not admit a new allocation or authorize
    /// moving a thread guard across an await.
    pub fn current() -> Option<Self> {
        effective_policy(None)
    }

    /// Whether two handles refer to the exact same allocation lifetime.
    pub fn same_owner(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Require this owner for every JSON reader constructed on this thread.
    /// An explicit or absent per-reader option cannot override this requirement.
    /// Nesting a different thread owner is rejected before replacing the context.
    pub fn enter_thread(&self) -> Result<ReaderThreadGuard, ResourceExhausted> {
        if let Some(required) = required_policy() {
            if !Arc::ptr_eq(&required.0, &self.0) {
                return Err(ResourceExhausted {
                    kind: "JSON thread owner replacement",
                    requested: 1,
                    limit: 0,
                });
            }
        }
        let previous = REQUIRED_POLICY.with(|current| current.replace(Some(self.clone())));
        Ok(ReaderThreadGuard {
            previous,
            _same_thread: std::marker::PhantomData,
        })
    }
}
fn required_policy() -> Option<ReaderResourcePolicy> {
    REQUIRED_POLICY.with(|current| current.borrow().clone())
}
pub(crate) fn effective_policy(
    explicit: Option<&ReaderResourcePolicy>,
) -> Option<ReaderResourcePolicy> {
    required_policy()
        .or_else(|| explicit.cloned())
        .or_else(current)
}
pub(crate) fn validate_reader_owner(
    owner: Option<&ReaderResourcePolicy>,
) -> Result<(), arrow_schema::ArrowError> {
    if let Some(required) = required_policy() {
        if !owner.is_some_and(|owner| Arc::ptr_eq(&required.0, &owner.0)) {
            return Err(ResourceExhausted {
                kind: "JSON preexisting reader owner",
                requested: 1,
                limit: 0,
            }
            .into());
        }
    }
    Ok(())
}
