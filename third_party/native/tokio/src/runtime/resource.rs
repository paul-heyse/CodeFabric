//! Exact native allocation geometry for an admitted local, timer-only runtime.
//!
//! This profile governs runtime containers. Task future/backing admission and
//! platform thread stack/TLS admission are separate requirements; a finite
//! queue size is never asserted to cover arbitrary native task allocations.
use std::{alloc::Layout, fmt, sync::Arc};

/// Allocation-free capacity failure from the local runtime profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceLayoutError {
    /// Native allocation family.
    pub kind: &'static str,
    /// Requested count or bytes.
    pub requested: usize,
    /// Accepted maximum.
    pub limit: usize,
}
impl fmt::Display for ResourceLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} requires {}, limit {}",
            self.kind, self.requested, self.limit
        )
    }
}
impl std::error::Error for ResourceLayoutError {}
fn overflow() -> ResourceLayoutError {
    ResourceLayoutError {
        kind: "tokio_native_layout",
        requested: usize::MAX,
        limit: isize::MAX as usize,
    }
}
pub(crate) fn add(a: usize, b: usize) -> Result<usize, ResourceLayoutError> {
    a.checked_add(b)
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(overflow)
}
pub(crate) fn mul(a: usize, b: usize) -> Result<usize, ResourceLayoutError> {
    a.checked_mul(b)
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(overflow)
}
pub(crate) fn array_bytes<T>(n: usize) -> Result<usize, ResourceLayoutError> {
    Layout::array::<T>(n)
        .map(|l| l.size())
        .map_err(|_| overflow())
}
pub(crate) fn arc_bytes<T>() -> Result<usize, ResourceLayoutError> {
    Layout::new::<[std::sync::atomic::AtomicUsize; 2]>()
        .extend(Layout::new::<T>())
        .map(|(l, _)| l.pad_to_align().size())
        .map_err(|_| overflow())
}

/// Original resource receipt. The application supplies this from its common ledger.
pub trait RuntimeAllocationReceipt: fmt::Debug + Send + Sync + 'static {
    /// Number of bytes reserved for the native runtime backing.
    fn bytes(&self) -> usize;
}
/// Native resource authority used before runtime construction and to latch later
/// container-capacity failures before enqueuing work.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeAllocationRequest {
    /// Native backing family, supplied from the actual allocation boundary.
    pub kind: &'static str,
    /// Complete new layout bytes before allocation.
    pub bytes: usize,
}
/// Original operation admission authority; failures must remain visible through join.
pub trait RuntimeAllocationAdmission: fmt::Debug + Send + Sync + 'static {
    /// Reserve the complete new allocation envelope before any native allocation.
    fn try_reserve(
        &self,
        request: RuntimeAllocationRequest,
    ) -> Result<Arc<dyn RuntimeAllocationReceipt>, ResourceLayoutError>;
    /// Record terminal pressure in the original operation owner.
    fn record_failure(&self, error: ResourceLayoutError);
}

/// Closed local profile. Networking, signal/process drivers, metrics histograms,
/// alternative timers and arbitrary Builder mutation are unavailable.
#[derive(Clone, Copy, Debug)]
pub struct LocalRuntimeProfile {
    /// Native scheduler worker count.
    pub worker_threads: usize,
    /// Additional native blocking threads, excluding scheduler workers.
    pub blocking_threads: usize,
    /// Original blocking queue descriptor capacity. In-flight tasks are separate.
    pub blocking_queue: usize,
    /// Exact requested native stack size; platform metadata remains separately admitted.
    pub thread_stack_bytes: usize,
    /// Cumulative async task submissions, including completed-but-retained tasks.
    pub async_tasks: usize,
    /// Cumulative blocking task submissions, including scheduler launch/handoff.
    pub blocking_tasks: usize,
}
/// Source-derived native heap and inline TLS geometry, not RSS or OS allocation census.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalRuntimeLayout {
    /// Native original scheduler/queue/park backing.
    pub scheduler_bytes: usize,
    /// Original blocking pool, fixed queue, thread handles and shutdown channel.
    pub blocking_bytes: usize,
    /// Native timer wheel and non-I/O parker backing.
    pub driver_bytes: usize,
    /// Default builder hook and exact supplied callback Arc allocations.
    pub callback_bytes: usize,
    /// Policy state allocation retained by native handles.
    pub owner_bytes: usize,
    /// Potential per-thread native park allocations, including the calling coordinator.
    pub thread_park_bytes: usize,
    /// Inline Tokio TLS state per participating thread, separate from heap bytes.
    pub tokio_tls_bytes_per_thread: usize,
    /// Maximum native threads, excluding the external hosting coordinator.
    pub native_threads: usize,
    /// Total of the native heap families above. Excludes task cells and platform thread state.
    pub heap_bytes: usize,
}
impl LocalRuntimeProfile {
    /// Validate counts before inspecting or constructing any native containers.
    pub fn validate(self) -> Result<(), ResourceLayoutError> {
        if self.worker_threads == 0 || self.worker_threads > 65535 {
            return Err(ResourceLayoutError {
                kind: "tokio_worker_threads",
                requested: self.worker_threads,
                limit: 65535,
            });
        }
        if self.blocking_threads == 0 {
            return Err(ResourceLayoutError {
                kind: "tokio_blocking_threads",
                requested: 1,
                limit: 0,
            });
        }
        if self.blocking_queue < self.worker_threads {
            return Err(ResourceLayoutError {
                kind: "tokio_blocking_queue",
                requested: self.worker_threads,
                limit: self.blocking_queue,
            });
        }
        if self.async_tasks == 0 || self.blocking_tasks < self.worker_threads {
            return Err(ResourceLayoutError {
                kind: "tokio_task_slots",
                requested: self.worker_threads.max(1),
                limit: self.blocking_tasks.min(self.async_tasks),
            });
        }
        if self.thread_stack_bytes == 0 {
            return Err(ResourceLayoutError {
                kind: "tokio_thread_stack",
                requested: 1,
                limit: 0,
            });
        }
        add(self.worker_threads, self.blocking_threads)?;
        if cfg!(any(
            tokio_unstable,
            loom,
            feature = "taskdump",
            feature = "tracing",
            all(feature = "parking_lot", not(feature = "native-owned-local"))
        )) {
            return Err(ResourceLayoutError {
                kind: "tokio_local_profile_configuration",
                requested: 1,
                limit: 0,
            });
        }
        Ok(())
    }
    /// Borrow-only, allocation-free geometry. Actual callback types are included.
    pub fn layout<Start, Stop>(self) -> Result<LocalRuntimeLayout, ResourceLayoutError> {
        self.validate()?;
        let native_threads = add(self.worker_threads, self.blocking_threads)?;
        let scheduler_bytes =
            crate::runtime::scheduler::multi_thread::resource_heap_bytes(self.worker_threads)?;
        let blocking_bytes =
            crate::runtime::blocking::resource_heap_bytes(native_threads, self.blocking_queue)?;
        let driver_bytes = add(
            crate::runtime::time::resource_heap_bytes(),
            crate::runtime::park::resource_heap_bytes()?,
        )?;
        // Builder::new's zero-capture default name Arc plus two callback Arcs.
        let callback_bytes = add(
            arc_bytes::<()>()?,
            add(arc_bytes::<Start>()?, arc_bytes::<Stop>()?)?,
        )?;
        let owner_bytes = arc_bytes::<ResourceState>()?;
        let thread_park_bytes = mul(
            add(native_threads, 1)?,
            crate::runtime::park::resource_heap_bytes()?,
        )?;
        let heap_bytes = [
            scheduler_bytes,
            blocking_bytes,
            driver_bytes,
            callback_bytes,
            owner_bytes,
            thread_park_bytes,
        ]
        .into_iter()
        .try_fold(0, add)?;
        Ok(LocalRuntimeLayout {
            scheduler_bytes,
            blocking_bytes,
            driver_bytes,
            callback_bytes,
            owner_bytes,
            thread_park_bytes,
            tokio_tls_bytes_per_thread: crate::runtime::context::resource_tls_bytes(),
            native_threads,
            heap_bytes,
        })
    }
    /// Admit and construct the original native runtime. Receipts remain attached
    /// to both the original scheduler handle and blocking spawner after this call.
    pub fn build<Start, Stop>(
        self,
        admission: Arc<dyn RuntimeAllocationAdmission>,
        start: Start,
        stop: Stop,
    ) -> Result<super::Runtime, LocalRuntimeBuildError>
    where
        Start: Fn() + Send + Sync + 'static,
        Stop: Fn() + Send + Sync + 'static,
    {
        let layout = self.layout::<Start, Stop>()?;
        let receipt = admission
            .try_reserve(RuntimeAllocationRequest {
                kind: "tokio_local_runtime",
                bytes: layout.heap_bytes,
            })
            .map_err(|error| {
                admission.record_failure(error);
                error
            })?;
        if receipt.bytes() < layout.heap_bytes {
            let error = ResourceLayoutError {
                kind: "tokio_runtime_receipt",
                requested: layout.heap_bytes,
                limit: receipt.bytes(),
            };
            admission.record_failure(error);
            return Err(error.into());
        }
        let state = ResourceAnchor {
            _state: Arc::new(ResourceState {
                profile: self,
                admission,
                async_tasks: std::sync::atomic::AtomicUsize::new(0),
                blocking_tasks: std::sync::atomic::AtomicUsize::new(0),
                failure: std::sync::Mutex::new(None),
                closed: std::sync::atomic::AtomicBool::new(false),
                _receipt: receipt.clone(),
            }),
            _receipt: receipt,
        };
        let mut builder = super::Builder::new_multi_thread();
        builder.local_resource = Some(state._state.clone());
        builder
            .worker_threads(self.worker_threads)
            .max_blocking_threads(self.blocking_threads)
            .thread_stack_size(self.thread_stack_bytes)
            .on_thread_start(start)
            .on_thread_stop(stop)
            .enable_time();
        builder
            .build()
            .map_err(|error| match *state._state.failure.lock().unwrap() {
                Some(error) => LocalRuntimeBuildError::Resource(error),
                None => LocalRuntimeBuildError::Io(error),
            })
    }
}
/// Construction failure retains original native I/O error semantics.
#[derive(Debug)]
pub enum LocalRuntimeBuildError {
    /// Allocation/capacity denial.
    Resource(ResourceLayoutError),
    /// Native OS/runtime construction error.
    Io(std::io::Error),
}
impl From<ResourceLayoutError> for LocalRuntimeBuildError {
    fn from(e: ResourceLayoutError) -> Self {
        Self::Resource(e)
    }
}
impl From<std::io::Error> for LocalRuntimeBuildError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl fmt::Display for LocalRuntimeBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Io(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for LocalRuntimeBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Io(e) => Some(e),
        }
    }
}
#[derive(Debug)]
pub(crate) struct ResourceState {
    pub(crate) profile: LocalRuntimeProfile,
    pub(crate) admission: Arc<dyn RuntimeAllocationAdmission>,
    async_tasks: std::sync::atomic::AtomicUsize,
    blocking_tasks: std::sync::atomic::AtomicUsize,
    failure: std::sync::Mutex<Option<ResourceLayoutError>>,
    closed: std::sync::atomic::AtomicBool,
    // Receipt drops after policy/payload descriptors.
    _receipt: Arc<dyn RuntimeAllocationReceipt>,
}
pub(crate) fn current() -> Option<Arc<ResourceState>> {
    let handle = super::Handle::try_current().ok()?;
    match handle.inner {
        crate::runtime::scheduler::Handle::MultiThread(handle) => handle.local_resource.clone(),
        _ => None,
    }
}

impl ResourceState {
    pub(crate) fn fail(&self, error: ResourceLayoutError) {
        let mut failure = self.failure.lock().unwrap();
        if failure.is_none() {
            *failure = Some(error);
            drop(failure);
            self.admission.record_failure(error);
        }
    }
    pub(crate) fn close(&self) {
        self.closed
            .store(true, std::sync::atomic::Ordering::Release);
    }
    fn check_available(&self) -> Result<(), ResourceLayoutError> {
        if let Some(error) = *self.failure.lock().unwrap() {
            return Err(error);
        }
        if self.closed.load(std::sync::atomic::Ordering::Acquire) {
            let error = ResourceLayoutError {
                kind: "tokio_runtime_shutdown",
                requested: 1,
                limit: 0,
            };
            self.fail(error);
            return Err(error);
        }
        Ok(())
    }
    fn reserve_task(
        &self,
        bytes: usize,
        blocking: bool,
    ) -> Result<Arc<dyn RuntimeAllocationReceipt>, ResourceLayoutError> {
        self.check_available()?;
        let (count, limit, kind) = if blocking {
            (
                &self.blocking_tasks,
                self.profile.blocking_tasks,
                "tokio_blocking_task",
            )
        } else {
            (
                &self.async_tasks,
                self.profile.async_tasks,
                "tokio_async_task",
            )
        };
        count
            .fetch_update(
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
                |count| count.checked_add(1).filter(|next| *next <= limit),
            )
            .map_err(|count| {
                let e = ResourceLayoutError {
                    kind,
                    requested: count.saturating_add(1),
                    limit,
                };
                self.fail(e);
                e
            })?;
        let receipt = self
            .admission
            .try_reserve(RuntimeAllocationRequest { kind, bytes })
            .map_err(|e| {
                self.fail(e);
                e
            })?;
        if receipt.bytes() < bytes {
            let e = ResourceLayoutError {
                kind: "tokio_task_receipt",
                requested: bytes,
                limit: receipt.bytes(),
            };
            self.fail(e);
            return Err(e);
        }
        Ok(receipt)
    }
}
thread_local! { static TASK_RECEIPT: std::cell::RefCell<Option<Arc<dyn RuntimeAllocationReceipt>>> = const {std::cell::RefCell::new(None)}; }
pub(crate) fn take_task_receipt() -> Option<Arc<dyn RuntimeAllocationReceipt>> {
    TASK_RECEIPT.with(|slot| slot.borrow_mut().take())
}
fn with_task_receipt<T>(receipt: Arc<dyn RuntimeAllocationReceipt>, f: impl FnOnce() -> T) -> T {
    struct Restore(Option<Arc<dyn RuntimeAllocationReceipt>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TASK_RECEIPT.with(|slot| *slot.borrow_mut() = self.0.take());
        }
    }
    let old = TASK_RECEIPT.with(|slot| slot.borrow_mut().replace(receipt));
    let _restore = Restore(old);
    f()
}
pub(crate) fn spawn_async<F>(
    handle: &super::Handle,
    future: F,
) -> super::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    let state = match &handle.inner {
        super::scheduler::Handle::MultiThread(handle) => {
            handle.local_resource.as_ref().expect("owned local runtime")
        }
        _ => unreachable!("owned multi-thread runtime"),
    };
    type Scheduler = super::scheduler::multi_thread::HandleRef;
    let size = std::mem::size_of::<F>();
    let bytes = if size > super::BOX_FUTURE_THRESHOLD {
        add(
            size,
            super::task::resource_task_bytes::<std::pin::Pin<Box<F>>, Scheduler>(),
        )
    } else {
        Ok(super::task::resource_task_bytes::<F, Scheduler>())
    };
    let receipt = bytes.and_then(|bytes| state.reserve_task(bytes, false));
    match receipt {
        Err(error) => {
            state.fail(error);
            drop(future);
            super::task::JoinHandle::resource_denied(super::task::Id::next(), error)
        }
        Ok(receipt) => with_task_receipt(receipt, || {
            let meta = crate::util::trace::SpawnMeta::new_unnamed(size);
            if size > super::BOX_FUTURE_THRESHOLD {
                handle.spawn_named(Box::pin(future), meta)
            } else {
                handle.spawn_named(future, meta)
            }
        }),
    }
}
pub(crate) fn spawn_blocking<F, R>(
    handle: &super::Handle,
    func: F,
    spawn: impl FnOnce(F) -> super::task::JoinHandle<R>,
) -> super::task::JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let state = match &handle.inner {
        super::scheduler::Handle::MultiThread(handle) => {
            handle.local_resource.as_ref().expect("owned local runtime")
        }
        _ => unreachable!("owned multi-thread runtime"),
    };
    match super::blocking::resource_task_bytes::<F, R>()
        .and_then(|bytes| state.reserve_task(bytes, true))
    {
        Err(error) => {
            state.fail(error);
            drop(func);
            super::task::JoinHandle::resource_denied(super::task::Id::next(), error)
        }
        Ok(receipt) => with_task_receipt(receipt, || spawn(func)),
    }
}

pub(crate) fn is_owned(handle: &super::Handle) -> bool {
    matches!(&handle.inner,super::scheduler::Handle::MultiThread(h) if h.local_resource.is_some())
}

pub(crate) fn admit_block_on<F>(
    handle: &super::Handle,
) -> Result<Option<Arc<dyn RuntimeAllocationReceipt>>, ResourceLayoutError> {
    let state = match &handle.inner {
        super::scheduler::Handle::MultiThread(h) => h.local_resource.as_ref(),
        _ => None,
    };
    let Some(state) = state else { return Ok(None) };
    state.check_available()?;
    let bytes = std::mem::size_of::<F>();
    if bytes <= super::BOX_FUTURE_THRESHOLD {
        return Ok(None);
    };
    let receipt = state
        .admission
        .try_reserve(RuntimeAllocationRequest {
            kind: "tokio_block_on_future",
            bytes,
        })
        .map_err(|e| {
            state.fail(e);
            e
        })?;
    if receipt.bytes() < bytes {
        let e = ResourceLayoutError {
            kind: "tokio_block_on_receipt",
            requested: bytes,
            limit: receipt.bytes(),
        };
        state.fail(e);
        return Err(e);
    }
    Ok(Some(receipt))
}
pub(crate) fn mandatory_blocking<F, R>(
    handle: &super::Handle,
    func: F,
    spawn: impl FnOnce(F) -> Option<super::task::JoinHandle<R>>,
) -> Option<super::task::JoinHandle<R>>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let state = match &handle.inner {
        super::scheduler::Handle::MultiThread(h) => {
            h.local_resource.as_ref().expect("owned runtime")
        }
        _ => unreachable!(),
    };
    match super::blocking::resource_task_bytes::<F, R>()
        .and_then(|bytes| state.reserve_task(bytes, true))
    {
        Err(error) => {
            state.fail(error);
            drop(func);
            None
        }
        Ok(receipt) => with_task_receipt(receipt, || spawn(func)),
    }
}

/// Whether the current runtime has the original required local admission profile.
/// Writers use this before constructing native background work; a thread-local
/// application scope without this runtime does not establish task admission.
pub fn current_profile() -> Option<LocalRuntimeProfile> {
    current().map(|state| state.profile)
}

/// Check current native admission before selected third-party work is constructed.
pub fn check_current_profile() -> Result<LocalRuntimeProfile, ResourceLayoutError> {
    let state = current().ok_or(ResourceLayoutError {
        kind: "tokio_local_profile_required",
        requested: 1,
        limit: 0,
    })?;
    state.check_available()?;
    Ok(state.profile)
}

#[cfg(feature = "fs")]
pub(crate) fn missing_mandatory_task() -> std::io::Error {
    if current().is_some_and(|state| state.failure.lock().unwrap().is_some()) {
        return std::io::ErrorKind::OutOfMemory.into();
    }
    std::io::Error::new(std::io::ErrorKind::Other, "background task failed")
}

/// Inline owner outside each original native Arc allocation. Keeping a separate
/// receipt reference after `state` also covers the ResourceState Arc deallocation.
#[derive(Clone, Debug)]
pub(crate) struct ResourceAnchor {
    _state: Arc<ResourceState>,
    _receipt: Arc<dyn RuntimeAllocationReceipt>,
}
impl ResourceAnchor {
    pub(crate) fn new(state: &Arc<ResourceState>) -> Self {
        Self {
            _state: state.clone(),
            _receipt: state._receipt.clone(),
        }
    }
}

/// Require the exact original admission authority on the current native runtime.
pub fn check_current_admission(
    expected: &Arc<dyn RuntimeAllocationAdmission>,
) -> Result<LocalRuntimeProfile, ResourceLayoutError> {
    let state = current().ok_or(ResourceLayoutError {
        kind: "tokio_local_profile_required",
        requested: 1,
        limit: 0,
    })?;
    if !Arc::ptr_eq(expected, &state.admission) {
        return Err(ResourceLayoutError {
            kind: "tokio_native_admission_identity",
            requested: 1,
            limit: 0,
        });
    }
    state.check_available()?;
    Ok(state.profile)
}

/// Actual native JoinSet container geometry. These are descriptor bounds only;
/// callers must preadmit before the infallible constructor and retain receipts
/// until original entry Wakers are gone. Task Cells are separately guarded.
pub fn join_set_new_bytes<T>() -> Result<usize, ResourceLayoutError> {
    crate::util::idle_notified_set::resource_new_bytes::<super::task::JoinHandle<T>>()
}
/// Original Arc<ListEntry<JoinHandle<T>>> allocation per inserted task.
pub fn join_set_entry_bytes<T>() -> Result<usize, ResourceLayoutError> {
    crate::util::idle_notified_set::resource_entry_bytes::<super::task::JoinHandle<T>>()
}
/// Complete temporary pointer Vec used by one abort_all traversal.
pub fn join_set_abort_bytes<T>(len: usize) -> Result<usize, ResourceLayoutError> {
    array_bytes::<*mut super::task::JoinHandle<T>>(len)
}

#[cfg(all(test, not(loom)))]
#[path = "resource_tests.rs"]
mod tests;

/// Run the original block-in-place handoff, surfacing admission denial before
/// invoking the user's blocking closure. The original reset guard restores the
/// worker core when a denied handoff leaves it in the shared core slot.
pub fn try_block_in_place<F,R>(f:F)->Result<R,ResourceLayoutError>
where F:FnOnce()->R {
    let state=current();
    if let Some(state)=&state {state.check_available()?;}
    crate::task::block_in_place(|| {
        if let Some(state)=&state {state.check_available()?;}
        Ok(f())
    })
}
