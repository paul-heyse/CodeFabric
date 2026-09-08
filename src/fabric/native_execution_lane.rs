//! Joined execution for native libraries whose Tokio tasks bypass the DataFusion task tracer.
//!
//! The entire native operation, including construction and stream consumption, runs on a
//! private multi-thread runtime. Its registered blocking owner drops that runtime before
//! publishing a result. Tokio runtime destruction joins started blocking tasks even after
//! their native futures or observers were dropped. `shutdown_background` and timed runtime
//! shutdown would violate that ownership contract and are deliberately unavailable here.
//!
//! Thread counts are enforced by Tokio. Memory and native task slots are admitted capacity,
//! not RSS measurements or an allocator limit. Callers must bound the selected native
//! workload's buffer geometry, fanout and blocking nesting before choosing its envelope.
//! Stable Tokio does not offer a fallible hook for arbitrary native task creation.

use std::cell::Cell;
use std::fmt::{Display, Write};
use std::future::Future;
use std::num::{NonZeroU64, NonZeroUsize};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::time::Instant;

use thiserror::Error;
use tokio::runtime::Builder;

use crate::cancellation::{
    Cancellation, StructuredCancellationScope, StructuredTaskError, TaskObservation,
};
use crate::resource_budget::{
    ChargedSlice, ResourceAmounts, ResourceBudget, ResourceBudgetError, ResourceClass,
};

thread_local! {
    // Execution context only, never an owner registry or semantic lookup.
    static IN_NATIVE_LANE: Cell<bool> = const { Cell::new(false) };
}

struct NativeThreadContext(bool);
impl NativeThreadContext {
    fn enter() -> Self {
        Self(IN_NATIVE_LANE.replace(true))
    }
}
impl Drop for NativeThreadContext {
    fn drop(&mut self) {
        IN_NATIVE_LANE.set(self.0);
    }
}

/// A reviewed, finite workload profile. This does not change the workspace DataFusion pool.
#[derive(Clone, Copy, Debug)]
pub(crate) struct NativeLaneEnvelope {
    pub worker_threads: NonZeroUsize,
    pub blocking_threads: NonZeroUsize,
    /// Native thread stack setting and reserved stack bytes for each thread, including the
    /// hosting blocking coordinator. Its installed runtime must use no larger stack setting.
    pub thread_stack_bytes: NonZeroUsize,
    pub runtime_memory_bytes: NonZeroU64,
    pub native_buffer_bytes: u64,
    pub native_task_slots: NonZeroU64,
    pub task_memory_bytes: NonZeroU64,
    /// Maximum concurrently active blocking roots in the selected native workload.
    pub parallel_blocking_roots: NonZeroUsize,
    /// Maximum nested blocking calls below each root, verified for the selected workload.
    pub blocking_nesting: NonZeroUsize,
}

impl NativeLaneEnvelope {
    fn reservation(self) -> Result<ResourceAmounts, NativeLaneError> {
        // Kernel's multi-thread executor uses block_in_place plus a blocking send to return
        // each nested result. Reserve a return slot for every root, not just the leaf calls.
        let required_blocking = self
            .blocking_nesting
            .get()
            .checked_add(1)
            .and_then(|depth| depth.checked_mul(self.parallel_blocking_roots.get()))
            .and_then(|threads| threads.checked_add(self.worker_threads.get()))
            .ok_or(NativeLaneError::InvalidEnvelope(
                "blocking headroom overflow",
            ))?;
        if self.blocking_threads.get() < required_blocking {
            return Err(NativeLaneError::InvalidEnvelope(
                "insufficient nested blocking headroom",
            ));
        }
        let threads = self
            .worker_threads
            .get()
            .checked_add(self.blocking_threads.get())
            .and_then(|value| value.checked_add(1))
            .and_then(|value| u64::try_from(value).ok())
            .ok_or(NativeLaneError::InvalidEnvelope("thread count overflow"))?;
        let stack = u64::try_from(self.thread_stack_bytes.get())
            .map_err(|_| NativeLaneError::InvalidEnvelope("thread stack overflow"))?;
        let memory_bytes = threads
            .checked_mul(stack)
            .and_then(|value| value.checked_add(self.runtime_memory_bytes.get()))
            .and_then(|value| value.checked_add(self.native_buffer_bytes))
            .and_then(|value| {
                self.native_task_slots
                    .get()
                    .checked_mul(self.task_memory_bytes.get())
                    .and_then(|tasks| value.checked_add(tasks))
            })
            .ok_or(NativeLaneError::InvalidEnvelope("memory envelope overflow"))?;
        Ok(ResourceAmounts {
            memory_bytes,
            running_jobs: 1,
            ..ResourceAmounts::default()
        })
    }
}

/// Deliberate crate-internal allowlist. A new implementation requires an ownership audit:
/// no native future, stream, runtime, borrowed engine executor, or uncharged backing may escape.
/// Kept separate from Send: Send alone says nothing about terminal native work or allocations.
pub(crate) mod output_seal {
    pub(crate) trait Sealed {}
}
pub(crate) trait NativeLaneOutput: output_seal::Sealed + Send + 'static {
    fn validate_retained_owner(&self, budget: &ResourceBudget) -> Result<(), ResourceBudgetError>;
}
macro_rules! scalar_output {
    ($($scalar:ty),* $(,)?) => {$ (
        impl output_seal::Sealed for $scalar {}
        impl NativeLaneOutput for $scalar {
            fn validate_retained_owner(&self, _: &ResourceBudget) -> Result<(), ResourceBudgetError> { Ok(()) }
        }
    )*};
}
scalar_output!((), bool, u64, i64);
impl output_seal::Sealed for ChargedSlice<u8> {}
impl NativeLaneOutput for ChargedSlice<u8> {
    fn validate_retained_owner(&self, budget: &ResourceBudget) -> Result<(), ResourceBudgetError> {
        let owner = self.reservation().owner();
        let workspace = budget.ancestor_owner(crate::resource_budget::ResourceScopeKind::Workspace);
        if owner.same_root(budget)
            && workspace.is_some()
            && owner.ancestor_owner(crate::resource_budget::ResourceScopeKind::Workspace)
                == workspace
        {
            Ok(())
        } else {
            Err(ResourceBudgetError::ForeignOwner)
        }
    }
}

/// The lifecycle and budget scopes must come from the same installed workspace owner.
pub(crate) struct NativeLaneAdmission<'a> {
    pub scope: &'a StructuredCancellationScope,
    pub name: &'a str,
    pub budget: &'a ResourceBudget,
    pub class: ResourceClass,
    pub deadline: Instant,
}

/// Evidence minted only after destruction of this exact native runtime joined its workers.
#[derive(Clone, Copy, Debug)]
pub(crate) struct NativeLaneJoined {
    runtime_id: tokio::runtime::Id,
}

/// Native dependency policies installed for the lifetime of each dedicated OS
/// thread. Construction must already have admitted their finite backing. Even a
/// late worker starting after failure must install all required policies.
pub(crate) trait NativeLaneResourcePolicy: Send + Sync + 'static {
    /// Concrete native policies supply the original operation bank. None is
    /// retained for the finite standalone lifecycle harness only.
    fn runtime_admission(&self) -> Option<Arc<dyn tokio::runtime::resource::RuntimeAllocationAdmission>> { None }
    /// Bind an allocation owner to one complete native operation. Concrete
    /// policies reject reuse before any native runtime or worker is constructed.
    fn begin_operation(&self) -> Result<(), NativeResourceFailure> { Ok(()) }
    fn enter_thread(&self);
    fn exit_thread(&self);
    fn check_available(&self) -> Result<(), NativeResourceFailure>;
    fn joined(&self, proof: NativeLaneJoined) -> Result<(), NativeResourceFailure>;
}

/// Allocation-free native pressure crossing the joined execution boundary.
#[derive(Clone, Copy, Debug)]
pub(crate) struct NativeResourceFailure {
    pub kind: &'static str,
    pub requested: usize,
    pub limit: usize,
}
impl From<NativeResourceFailure> for NativeLaneError {
    fn from(failure: NativeResourceFailure) -> Self {
        Self::NativeResourceExhausted {
            kind: failure.kind,
            requested: failure.requested,
            limit: failure.limit,
        }
    }
}

impl From<tokio::runtime::resource::ResourceLayoutError> for NativeLaneError {
    fn from(error: tokio::runtime::resource::ResourceLayoutError) -> Self {
        NativeResourceFailure { kind: error.kind, requested: error.requested, limit: error.limit }.into()
    }
}
impl From<tokio::runtime::resource::LocalRuntimeBuildError> for NativeLaneError {
    fn from(error: tokio::runtime::resource::LocalRuntimeBuildError) -> Self {
        match error {
            tokio::runtime::resource::LocalRuntimeBuildError::Resource(error) => error.into(),
            tokio::runtime::resource::LocalRuntimeBuildError::Io(error) => error.into(),
        }
    }
}

struct NativeResourceThreadContext(Option<Arc<dyn NativeLaneResourcePolicy>>);
impl NativeResourceThreadContext {
    fn enter(policy: Option<Arc<dyn NativeLaneResourcePolicy>>) -> Self {
        if let Some(policy) = &policy {
            policy.enter_thread();
        }
        Self(policy)
    }
}
impl Drop for NativeResourceThreadContext {
    fn drop(&mut self) {
        if let Some(policy) = &self.0 {
            policy.exit_thread();
        }
    }
}

impl NativeLaneJoined {
    pub(crate) const fn runtime_id(self) -> tokio::runtime::Id {
        self.runtime_id
    }
}

/// Cleanup runs on every completed/failed/cancelled/panicking native operation.
/// The asynchronous half aborts native uploads while their runtime remains alive. The
/// synchronous half reconciles physical charges only after all hidden workers have joined.
/// That synchronous callback is application reconciliation only: it must not create native
/// operations or spawn tasks after the runtime's join evidence has been minted.
pub(crate) struct NativeLaneCleanup<C, R> {
    pub before_join: C,
    pub after_join: R,
}

/// Typed admission outcomes; callers never infer pressure or cancellation from text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub(crate) enum NativeAdmissionFailure {
    #[error("a native operation cannot enter another native execution lane")]
    NestedLane,
    #[error("another native mutation still owns the workspace store")]
    MutationBusy,
    #[error("the workspace physical store requires joined reconciliation")]
    StoreNotReconciled,
    #[error("native admission bound exceeded: {0}")]
    NativeLimit(&'static str),
}

/// Opaque native errors become owned diagnostics before crossing the runtime boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeDiagnosticCategory {
    RuntimeConstruction,
    TaskJoin,
}

/// An admitted operation error; native error objects never cross the runtime boundary.
#[derive(Debug, Error)]
pub(crate) enum NativeLaneError {
    #[error("native resource {kind} requires {requested}, limit {limit}")]
    NativeResourceExhausted {
        kind: &'static str,
        requested: usize,
        limit: usize,
    },
    #[error(transparent)]
    AdmissionDenied(#[from] NativeAdmissionFailure),
    #[error("invalid native execution envelope: {0}")]
    InvalidEnvelope(&'static str),
    #[error(transparent)]
    Budget(#[from] ResourceBudgetError),
    #[error(transparent)]
    Registry(#[from] StructuredTaskError),
    #[error("native runtime construction failed: {0}")]
    Runtime(#[from] std::io::Error),
    #[error("native {category:?} failure: {message}")]
    NativeFailure {
        category: NativeDiagnosticCategory,
        message: String,
    },
    #[error("native operation cancelled; runtime workers have joined")]
    Cancelled,
    #[error("native operation deadline elapsed; runtime workers have joined")]
    Deadline,
    #[error("native operation failed: {0}")]
    Operation(String),
    #[error("native operation panicked; runtime workers have joined")]
    Panicked,
    #[error(
        "native cleanup failed before join={before_join:?}, after join={after_join:?}; operation={operation:?}"
    )]
    Cleanup {
        before_join: Option<String>,
        after_join: Option<String>,
        operation: Option<Box<Self>>,
    },
}

impl NativeLaneError {
    pub(crate) fn operation(error: &impl Display) -> Self {
        Self::Operation(diagnostic(error))
    }

    /// Classifiers may construct ordinary native errors, but only this closed, bounded subset
    /// may cross the runtime boundary. Drop opaque payloads while that runtime still owns work.
    fn into_owned_failure(self) -> Self {
        if let Self::Cleanup {
            before_join,
            after_join,
            operation,
        } = self
        {
            let before_join = before_join.map(|message| diagnostic(&message));
            let mut after_join = after_join.map(|message| diagnostic(&message));
            let mut next = operation.map(|error| *error);
            let mut nested = false;
            // Iteratively consume the supplied chain: even destruction of a deeply nested
            // user-created error must not recurse through arbitrarily many boxed cleanups.
            let terminal = loop {
                match next.take() {
                    Some(Self::Cleanup { operation, .. }) => {
                        nested = true;
                        next = operation.map(|error| *error);
                    }
                    Some(error) => break Some(Box::new(error.into_owned_failure())),
                    None => break None,
                }
            };
            if nested {
                after_join = Some(diagnostic(&format_args!(
                    "additional reported cleanup failures; {}",
                    after_join.as_deref().unwrap_or("")
                )));
            }
            return Self::Cleanup {
                before_join,
                after_join,
                operation: terminal,
            };
        }
        match self {
            Self::Runtime(error) => Self::NativeFailure {
                category: NativeDiagnosticCategory::RuntimeConstruction,
                message: diagnostic(&error),
            },
            Self::NativeFailure { category, message } => Self::NativeFailure {
                category,
                message: diagnostic(&message),
            },
            Self::Operation(message) => Self::operation(&message),
            Self::Registry(error) => match error {
                StructuredTaskError::Join { task, source } => Self::NativeFailure {
                    category: NativeDiagnosticCategory::TaskJoin,
                    message: diagnostic(&format_args!("{task}: {source}")),
                },
                StructuredTaskError::InvalidScopeSegment(value) => {
                    Self::Registry(StructuredTaskError::InvalidScopeSegment(diagnostic(&value)))
                }
                StructuredTaskError::ScopeClosed(value) => {
                    Self::Registry(StructuredTaskError::ScopeClosed(diagnostic(&value)))
                }
                StructuredTaskError::DuplicateTask(value) => {
                    Self::Registry(StructuredTaskError::DuplicateTask(diagnostic(&value)))
                }
                StructuredTaskError::CleanupReserveExhausted { scope } => {
                    Self::Registry(StructuredTaskError::CleanupReserveExhausted {
                        scope: diagnostic(&scope),
                    })
                }
                error @ (StructuredTaskError::TaskCapacity { .. }
                | StructuredTaskError::InvalidCapacity
                | StructuredTaskError::ControlCapacityUnavailable
                | StructuredTaskError::ObservationClosed) => Self::Registry(error),
            },
            error @ (Self::NativeResourceExhausted { .. }
            | Self::AdmissionDenied(_)
            | Self::InvalidEnvelope(_)
            | Self::Budget(_)
            | Self::Cancelled
            | Self::Deadline
            | Self::Panicked) => error,
            Self::Cleanup { .. } => unreachable!("cleanup chain normalized above"),
        }
    }
}

/// Immutable execution policy; every call constructs and then synchronously joins one runtime.
#[derive(Clone, Copy, Debug)]
pub(crate) struct NativeExecutionLane {
    envelope: NativeLaneEnvelope,
    reservation: ResourceAmounts,
}

impl NativeExecutionLane {
    pub(crate) fn try_new(envelope: NativeLaneEnvelope) -> Result<Self, NativeLaneError> {
        Ok(Self {
            reservation: envelope.reservation()?,
            envelope,
        })
    }

    pub(crate) const fn admitted_capacity(&self) -> ResourceAmounts {
        self.reservation
    }

    /// Reserve before runtime construction. Dropping the returned observation does not release
    /// either the common budget reservation or the registry's operation capacity.
    pub(crate) async fn spawn<T, E, F, O, C, CF, CE, R, RE>(
        &self,
        admission: NativeLaneAdmission<'_>,
        operation: O,
        cleanup: NativeLaneCleanup<C, R>,
    ) -> Result<TaskObservation<Result<T, NativeLaneError>>, NativeLaneError>
    where
        T: NativeLaneOutput,
        E: Display,
        F: Future<Output = Result<T, E>>,
        O: FnOnce(Cancellation) -> F + Send + 'static,
        C: FnOnce() -> CF + Send + 'static,
        CF: Future<Output = Result<(), CE>>,
        CE: Display,
        R: FnOnce(NativeLaneJoined) -> Result<(), RE> + Send + 'static,
        RE: Display,
    {
        self.spawn_classified(
            admission,
            move |probe| async move {
                operation(probe)
                    .await
                    .map_err(|error| NativeLaneError::operation(&error))
            },
            cleanup,
        )
        .await
    }

    /// Preserve typed failures, normalizing their payloads inside the private runtime before
    /// joining or composing cleanup errors. The classifier cannot return a native handle.
    pub(crate) async fn spawn_classified<T, F, O, C, CF, CE, R, RE>(
        &self,
        admission: NativeLaneAdmission<'_>,
        operation: O,
        cleanup: NativeLaneCleanup<C, R>,
    ) -> Result<TaskObservation<Result<T, NativeLaneError>>, NativeLaneError>
    where
        T: NativeLaneOutput,
        F: Future<Output = Result<T, NativeLaneError>>,
        O: FnOnce(Cancellation) -> F + Send + 'static,
        C: FnOnce() -> CF + Send + 'static,
        CF: Future<Output = Result<(), CE>>,
        CE: Display,
        R: FnOnce(NativeLaneJoined) -> Result<(), RE> + Send + 'static,
        RE: Display,
    {
        self.spawn_with_resource_policy(admission, None, operation, cleanup)
            .await
    }

    /// Execute a complete native phase with allocation policies on the
    /// coordinator, async workers and blocking workers. Failure is checked
    /// before future construction, after its result and after every worker joins.
    pub(crate) async fn spawn_with_resource_policy<T, F, O, C, CF, CE, R, RE>(
        &self,
        admission: NativeLaneAdmission<'_>,
        resource_policy: Option<Arc<dyn NativeLaneResourcePolicy>>,
        operation: O,
        cleanup: NativeLaneCleanup<C, R>,
    ) -> Result<TaskObservation<Result<T, NativeLaneError>>, NativeLaneError>
    where
        T: NativeLaneOutput,
        F: Future<Output = Result<T, NativeLaneError>>,
        O: FnOnce(Cancellation) -> F + Send + 'static,
        C: FnOnce() -> CF + Send + 'static,
        CF: Future<Output = Result<(), CE>>,
        CE: Display,
        R: FnOnce(NativeLaneJoined) -> Result<(), RE> + Send + 'static,
        RE: Display,
    {
        if IN_NATIVE_LANE.get() {
            return Err(NativeAdmissionFailure::NestedLane.into());
        }
        let reservation = admission
            .budget
            .try_reserve(admission.class, self.reservation)?;
        if let Some(policy) = &resource_policy { policy.begin_operation()?; }
        let scope = admission.scope.clone();
        let output_budget = admission.budget.clone();
        let deadline = admission.deadline;
        let envelope = self.envelope;
        Ok(admission.scope.spawn_blocking_owned(admission.name, reservation, move |probe| {
            if IN_NATIVE_LANE.get() {
                return Err(NativeAdmissionFailure::NestedLane.into());
            }
            let _native_context = NativeThreadContext::enter();
            let _resource_context = NativeResourceThreadContext::enter(resource_policy.clone());
            let worker_start_policy = resource_policy.clone();
            let worker_stop_policy = resource_policy.clone();
            let on_start = move || {
                    IN_NATIVE_LANE.set(true);
                    if let Some(policy) = &worker_start_policy { policy.enter_thread(); }
                };
            let on_stop = move || {
                    if let Some(policy) = &worker_stop_policy { policy.exit_thread(); }
                    IN_NATIVE_LANE.set(false);
                };
            let runtime = if let Some(native) = resource_policy.as_ref().and_then(|policy| policy.runtime_admission()) {
                let tasks = usize::try_from(envelope.native_task_slots.get()).map_err(|_| NativeLaneError::InvalidEnvelope("native task count overflow"))?;
                tokio::runtime::resource::LocalRuntimeProfile {
                    worker_threads: envelope.worker_threads.get(), blocking_threads: envelope.blocking_threads.get(),
                    blocking_queue: tasks.max(envelope.worker_threads.get()), thread_stack_bytes: envelope.thread_stack_bytes.get(),
                    async_tasks: tasks, blocking_tasks: tasks,
                }.build(native, on_start, on_stop)?
            } else {
                Builder::new_multi_thread().worker_threads(envelope.worker_threads.get())
                    .max_blocking_threads(envelope.blocking_threads.get()).thread_stack_size(envelope.thread_stack_bytes.get())
                    .thread_name("cf-native-lane").on_thread_start(on_start).on_thread_stop(on_stop).enable_all().build()?
            };
            let operation_result = catch_unwind(AssertUnwindSafe(|| runtime.try_block_on(async {
                if probe.is_cancelled() {
                    return Err(NativeLaneError::Cancelled);
                }
                if Instant::now() >= deadline {
                    return Err(NativeLaneError::Deadline);
                }
                if let Some(policy) = &resource_policy { policy.check_available()?; }
                // The future is constructed inside the private runtime, including libraries
                // that capture Handle::current while creating a table, engine or stream.
                tokio::select! {
                    biased;
                    () = scope.cancelled() => Err(NativeLaneError::Cancelled),
                    () = tokio::time::sleep_until(deadline.into()) => Err(NativeLaneError::Deadline),
                    result = operation(probe) => {
                        if let Some(policy) = &resource_policy { policy.check_available()?; }
                        let value = result.map_err(NativeLaneError::into_owned_failure)?;
                        value.validate_retained_owner(&output_budget)?;
                        Ok(value)
                    },
                }
                // select drops the operation future here before any cleanup callback runs.
            }).map_err(NativeLaneError::from).and_then(|result| result)));
            let result = match operation_result {
                Ok(result) => result,
                Err(payload) => {
                    // A panic payload can own native objects whose Drop uses Handle::current.
                    // Dispose it inside the still-live lane, so resulting work joins below.
                    let _entered = runtime.enter();
                    drop(payload);
                    Err(NativeLaneError::Panicked)
                }
            };
            let cleanup_outcome = catch_unwind(AssertUnwindSafe(|| {
                runtime.try_block_on(async { (cleanup.before_join)().await.map_err(|error| diagnostic(&error)) })
                    .unwrap_or_else(|error| Err(diagnostic(&error)))
            }));
            let before_join = {
                let _entered = runtime.enter();
                cleanup_result(cleanup_outcome)
            };
            // No deadline can bypass this barrier. A worker that ignores cancellation remains
            // admitted and visible to the outer structured registry until it actually exits.
            let runtime_id = runtime.handle().id();
            drop(runtime);
            let joined = NativeLaneJoined { runtime_id };
            let resource_join = resource_policy.as_ref().map_or(Ok(()), |policy| policy.joined(joined));
            // A first terminal cancellation/error stays primary. A failure from
            // a hidden worker still rejects an otherwise successful result.
            let result = result.and_then(|value| resource_join.map(|()| value).map_err(Into::into));
            let after_join = cleanup_result(catch_unwind(AssertUnwindSafe(|| {
                (cleanup.after_join)(joined).map_err(|error| diagnostic(&error))
            })));
            if before_join.is_some() || after_join.is_some() {
                return Err(NativeLaneError::Cleanup {
                    before_join, after_join, operation: result.err().map(Box::new),
                });
            }
            result
        }).await?)
    }
}

fn cleanup_result(result: std::thread::Result<Result<(), String>>) -> Option<String> {
    match result {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error),
        Err(_) => Some("cleanup panicked".into()),
    }
}

fn diagnostic(error: &impl Display) -> String {
    // Formatting directly into a bounded writer prevents an arbitrarily large native error
    // from creating a second, unbounded String on its way out of the admitted operation.
    struct BoundedDiagnostic(String);
    impl Write for BoundedDiagnostic {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            let remaining = 4096usize.saturating_sub(self.0.len());
            let mut end = remaining.min(text.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            self.0.push_str(&text[..end]);
            if end == text.len() {
                Ok(())
            } else {
                Err(std::fmt::Error)
            }
        }
    }
    let mut target = BoundedDiagnostic(String::with_capacity(4096));
    let _ = write!(target, "{error}");
    target.0
}

#[cfg(test)]
mod tests {
    use std::future::{pending, ready};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, mpsc};
    use std::time::Duration;

    use super::*;
    use crate::resource_budget::ResourceBudgetPolicy;

    struct NativePanicPayload {
        runtime: tokio::runtime::Handle,
        dropped: Arc<AtomicBool>,
    }
    impl Drop for NativePanicPayload {
        fn drop(&mut self) {
            assert_eq!(tokio::runtime::Handle::current().id(), self.runtime.id());
            self.dropped.store(true, Ordering::Release);
        }
    }

    fn lane() -> NativeExecutionLane {
        NativeExecutionLane::try_new(NativeLaneEnvelope {
            worker_threads: NonZeroUsize::new(2).unwrap(),
            blocking_threads: NonZeroUsize::new(12).unwrap(),
            thread_stack_bytes: NonZeroUsize::new(2 * 1024 * 1024).unwrap(),
            runtime_memory_bytes: NonZeroU64::new(1024 * 1024).unwrap(),
            native_buffer_bytes: 4 * 1024 * 1024,
            native_task_slots: NonZeroU64::new(16).unwrap(),
            task_memory_bytes: NonZeroU64::new(16 * 1024).unwrap(),
            parallel_blocking_roots: NonZeroUsize::new(2).unwrap(),
            blocking_nesting: NonZeroUsize::new(4).unwrap(),
        })
        .unwrap()
    }

    fn budget() -> ResourceBudget {
        let policy = ResourceBudgetPolicy {
            limits: ResourceAmounts {
                memory_bytes: 128 * 1024 * 1024,
                disk_bytes: 1024,
                running_jobs: 2,
                queued_jobs: 64,
                retained_generations: 2,
                retained_bytes: 1024,
                rows: 1024,
                pages: 16,
            },
            control_reserve: ResourceAmounts {
                memory_bytes: 64 * 1024 * 1024,
                running_jobs: 1,
                queued_jobs: 32,
                ..ResourceAmounts::default()
            },
        };
        ResourceBudget::try_process([73; 16], policy)
            .unwrap()
            .workspace([74; 16], policy)
            .unwrap()
    }

    fn scopes() -> StructuredCancellationScope {
        StructuredCancellationScope::try_root_with_control_reserve(
            "lane-tests",
            NonZeroUsize::new(2).unwrap(),
            NonZeroUsize::new(1).unwrap(),
        )
        .unwrap()
    }

    thread_local! {
        static TEST_RESOURCE_POLICY: Cell<bool> = const { Cell::new(false) };
    }

    #[derive(Default)]
    struct TestResourcePolicy {
        active_threads: AtomicUsize,
        entered_threads: AtomicUsize,
        failed: AtomicBool,
        joined: AtomicBool,
    }
    impl NativeLaneResourcePolicy for TestResourcePolicy {
        fn enter_thread(&self) {
            assert!(!TEST_RESOURCE_POLICY.replace(true));
            self.active_threads.fetch_add(1, Ordering::AcqRel);
            self.entered_threads.fetch_add(1, Ordering::AcqRel);
        }
        fn exit_thread(&self) {
            assert!(TEST_RESOURCE_POLICY.replace(false));
            self.active_threads.fetch_sub(1, Ordering::AcqRel);
        }
        fn check_available(&self) -> Result<(), NativeResourceFailure> {
            if self.failed.load(Ordering::Acquire) {
                return Err(NativeResourceFailure {
                    kind: "native test allocation",
                    requested: 2,
                    limit: 1,
                });
            }
            Ok(())
        }
        fn joined(&self, _: NativeLaneJoined) -> Result<(), NativeResourceFailure> {
            // Only the coordinator's context remains after runtime destruction.
            assert_eq!(self.active_threads.load(Ordering::Acquire), 1);
            self.joined.store(true, Ordering::Release);
            self.check_available()
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_resource_policy_covers_all_workers_until_joined() {
        let scope = scopes();
        let budget = budget();
        let policy = Arc::new(TestResourcePolicy::default());
        let result = lane()
            .spawn_with_resource_policy(
                admission(&scope, &budget, ResourceClass::Data),
                Some(policy.clone()),
                |_| async {
                    assert!(TEST_RESOURCE_POLICY.get());
                    tokio::spawn(async {
                        assert!(TEST_RESOURCE_POLICY.get());
                    })
                    .await
                    .unwrap();
                    tokio::task::spawn_blocking(|| assert!(TEST_RESOURCE_POLICY.get()))
                        .await
                        .unwrap();
                    Ok(7_u64)
                },
                NativeLaneCleanup {
                    before_join: || ready(Ok::<(), &str>(())),
                    after_join: |_| Ok::<(), &str>(()),
                },
            )
            .await
            .unwrap()
            .wait()
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result, 7);
        assert!(policy.joined.load(Ordering::Acquire));
        assert!(policy.entered_threads.load(Ordering::Acquire) >= 4);
        assert_eq!(policy.active_threads.load(Ordering::Acquire), 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_resource_failure_during_hidden_worker_join_overrides_success() {
        let scope = scopes();
        let budget = budget();
        let policy = Arc::new(TestResourcePolicy::default());
        let worker_policy = policy.clone();
        let (release, wait) = mpsc::channel();
        let result = lane()
            .spawn_with_resource_policy(
                admission(&scope, &budget, ResourceClass::Data),
                Some(policy.clone()),
                move |_| async move {
                    let _background = tokio::task::spawn_blocking(move || {
                        wait.recv().unwrap();
                        assert!(TEST_RESOURCE_POLICY.get());
                        worker_policy.failed.store(true, Ordering::Release);
                    });
                    Ok(7_u64)
                },
                NativeLaneCleanup {
                    before_join: move || async move {
                        release.send(()).unwrap();
                        Ok::<(), &str>(())
                    },
                    after_join: |_| Ok::<(), &str>(()),
                },
            )
            .await
            .unwrap()
            .wait()
            .await
            .unwrap();
        assert!(matches!(
            result,
            Err(NativeLaneError::NativeResourceExhausted {
                kind: "native test allocation",
                ..
            })
        ));
        assert!(policy.joined.load(Ordering::Acquire));
        assert_eq!(policy.active_threads.load(Ordering::Acquire), 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_resource_failure_precedes_operation_factory() {
        let scope = scopes();
        let budget = budget();
        let policy = Arc::new(TestResourcePolicy::default());
        policy.failed.store(true, Ordering::Release);
        let result = lane()
            .spawn_with_resource_policy(
                admission(&scope, &budget, ResourceClass::Data),
                Some(policy.clone()),
                |_| {
                    panic!("failed native admission must precede factory execution");
                    #[allow(unreachable_code)]
                    ready(Ok(()))
                },
                NativeLaneCleanup {
                    before_join: || ready(Ok::<(), &str>(())),
                    after_join: |_| Ok::<(), &str>(()),
                },
            )
            .await
            .unwrap()
            .wait()
            .await
            .unwrap();
        assert!(matches!(
            result,
            Err(NativeLaneError::NativeResourceExhausted { .. })
        ));
        assert!(policy.joined.load(Ordering::Acquire));
        assert_eq!(policy.active_threads.load(Ordering::Acquire), 0);
    }

    fn admission<'a>(
        scope: &'a StructuredCancellationScope,
        budget: &'a ResourceBudget,
        class: ResourceClass,
    ) -> NativeLaneAdmission<'a> {
        NativeLaneAdmission {
            scope,
            name: "native",
            budget,
            class,
            deadline: Instant::now() + Duration::from_secs(10),
        }
    }

    // Panic-safe release prevents a failed assertion from stranding the runtime join barrier.
    struct ReleaseOnDrop(Option<mpsc::Sender<()>>);
    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            if let Some(sender) = self.0.take() {
                let _ = sender.send(());
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_lane_cancelled_observer_retains_hidden_worker_and_control_capacity() {
        let lane = lane();
        let budget = budget();
        let root = scopes();
        let data = root.child("data").unwrap();
        let control = root.child_control("control").unwrap();
        let (release, released) = mpsc::channel();
        let release = ReleaseOnDrop(Some(release));
        let (started, running) = tokio::sync::oneshot::channel();
        let (cleaning, cleanup_started) = tokio::sync::oneshot::channel();
        let worker_finished = Arc::new(AtomicBool::new(false));
        let finished = worker_finished.clone();
        let after_join = Arc::new(AtomicBool::new(false));
        let joined = after_join.clone();
        let final_finished = worker_finished.clone();
        let task = lane
            .spawn(
                admission(&data, &budget, ResourceClass::Data),
                move |_| async move {
                    // Native libraries can drop this handle. The private runtime still owns the worker.
                    drop(tokio::task::spawn_blocking(move || {
                        started.send(()).unwrap();
                        released.recv_timeout(Duration::from_secs(10)).unwrap();
                        finished.store(true, Ordering::Release);
                    }));
                    pending::<Result<(), &'static str>>().await
                },
                NativeLaneCleanup {
                    before_join: move || async move {
                        let _ = cleaning.send(());
                        Ok::<(), &'static str>(())
                    },
                    after_join: move |token: NativeLaneJoined| {
                        assert_ne!(token.runtime_id(), tokio::runtime::Handle::current().id());
                        assert!(final_finished.load(Ordering::Acquire));
                        joined.store(true, Ordering::Release);
                        Ok::<(), &'static str>(())
                    },
                },
            )
            .await
            .unwrap();
        running.await.unwrap();
        data.cancel();
        cleanup_started.await.unwrap();
        drop(task);
        assert_eq!(budget.observation().used.running_jobs, 1);
        assert_eq!(
            budget.observation().used.memory_bytes,
            u128::from(lane.admitted_capacity().memory_bytes)
        );
        assert_eq!(data.live_task_count().await.unwrap(), 1);
        assert!(!after_join.load(Ordering::Acquire));
        assert!(
            data.cancel_and_join(Duration::from_millis(20))
                .await
                .is_err()
        );
        assert_eq!(budget.observation().used.running_jobs, 1);

        let second = root.child("second").unwrap();
        assert!(matches!(
            lane.spawn(
                admission(&second, &budget, ResourceClass::Data),
                |_| ready(Ok::<(), &'static str>(())),
                NativeLaneCleanup {
                    before_join: || ready(Ok::<(), &'static str>(())),
                    after_join: |_| Ok::<(), &'static str>(()),
                }
            )
            .await,
            Err(NativeLaneError::Budget(_))
        ));
        let status = lane
            .spawn(
                admission(&control, &budget, ResourceClass::Control),
                |_| ready(Ok::<u64, &'static str>(42)),
                NativeLaneCleanup {
                    before_join: || ready(Ok::<(), &'static str>(())),
                    after_join: |_| Ok::<(), &'static str>(()),
                },
            )
            .await
            .unwrap()
            .wait()
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status, 42);
        assert_eq!(budget.observation().used.running_jobs, 1);
        drop(release);
        data.cancel_and_join(Duration::from_secs(3)).await.unwrap();
        assert!(after_join.load(Ordering::Acquire));
        assert_eq!(budget.observation().used.running_jobs, 0);
        assert_eq!(budget.observation().used.memory_bytes, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_lane_live_deadline_retains_hidden_worker_until_runtime_join() {
        let lane = lane();
        let budget = budget();
        let root = scopes();
        let (release, released) = mpsc::channel();
        let release = ReleaseOnDrop(Some(release));
        let (started, running) = tokio::sync::oneshot::channel();
        let (cleaning, cleanup_started) = tokio::sync::oneshot::channel();
        let mut request = admission(&root, &budget, ResourceClass::Data);
        request.deadline = Instant::now() + Duration::from_secs(1);
        let task = lane
            .spawn(
                request,
                move |_| async move {
                    drop(tokio::task::spawn_blocking(move || {
                        started.send(()).unwrap();
                        released.recv_timeout(Duration::from_secs(10)).unwrap();
                    }));
                    pending::<Result<(), &'static str>>().await
                },
                NativeLaneCleanup {
                    before_join: move || async move {
                        cleaning.send(()).unwrap();
                        Ok::<(), &'static str>(())
                    },
                    after_join: |_| Ok::<(), &'static str>(()),
                },
            )
            .await
            .unwrap();
        running.await.unwrap();
        cleanup_started.await.unwrap();
        assert_eq!(budget.observation().used.running_jobs, 1);
        assert_eq!(root.live_task_count().await.unwrap(), 1);
        drop(release);
        assert!(matches!(
            task.wait().await.unwrap(),
            Err(NativeLaneError::Deadline)
        ));
        assert_eq!(budget.observation().used.running_jobs, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_lane_operation_panic_still_cleans_and_joins_before_result() {
        let lane = lane();
        let budget = budget();
        let root = scopes();
        let (release, released) = mpsc::channel();
        let release = ReleaseOnDrop(Some(release));
        let (cleaning, cleanup_started) = tokio::sync::oneshot::channel();
        let worker_finished = Arc::new(AtomicBool::new(false));
        let finished = worker_finished.clone();
        let payload_dropped = Arc::new(AtomicBool::new(false));
        let dropped = payload_dropped.clone();
        let task = lane
            .spawn(
                admission(&root, &budget, ResourceClass::Data),
                move |_| async move {
                    let (started, running) = tokio::sync::oneshot::channel();
                    drop(tokio::task::spawn_blocking(move || {
                        started.send(()).unwrap();
                        released.recv_timeout(Duration::from_secs(10)).unwrap();
                        finished.store(true, Ordering::Release);
                    }));
                    running.await.unwrap();
                    std::panic::panic_any(NativePanicPayload {
                        runtime: tokio::runtime::Handle::current(),
                        dropped,
                    });
                    #[allow(unreachable_code)]
                    Ok::<(), &'static str>(())
                },
                NativeLaneCleanup {
                    before_join: move || async move {
                        cleaning.send(()).unwrap();
                        Ok::<(), &'static str>(())
                    },
                    after_join: move |_| {
                        assert!(worker_finished.load(Ordering::Acquire));
                        Ok::<(), &'static str>(())
                    },
                },
            )
            .await
            .unwrap();
        cleanup_started.await.unwrap();
        assert_eq!(budget.observation().used.running_jobs, 1);
        drop(release);
        assert!(matches!(
            task.wait().await.unwrap(),
            Err(NativeLaneError::Panicked)
        ));
        assert_eq!(budget.observation().used.running_jobs, 0);
        assert!(payload_dropped.load(Ordering::Acquire));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_lane_cleanup_panic_preserves_join_and_original_failure() {
        let lane = lane();
        let budget = budget();
        let root = scopes();
        let reconciled = Arc::new(AtomicBool::new(false));
        let after = reconciled.clone();
        let payload_dropped = Arc::new(AtomicBool::new(false));
        let dropped = payload_dropped.clone();
        let error = lane
            .spawn(
                admission(&root, &budget, ResourceClass::Data),
                |_| ready(Err::<(), &'static str>("native rejected")),
                NativeLaneCleanup {
                    before_join: move || async move {
                        std::panic::panic_any(NativePanicPayload {
                            runtime: tokio::runtime::Handle::current(),
                            dropped,
                        });
                        #[allow(unreachable_code)]
                        Ok::<(), &'static str>(())
                    },
                    after_join: move |_| {
                        after.store(true, Ordering::Release);
                        Ok::<(), &'static str>(())
                    },
                },
            )
            .await
            .unwrap()
            .wait()
            .await
            .unwrap()
            .unwrap_err();
        let NativeLaneError::Cleanup {
            before_join,
            after_join,
            operation,
        } = error
        else {
            panic!("wrong failure");
        };
        assert_eq!(before_join.as_deref(), Some("cleanup panicked"));
        assert!(after_join.is_none());
        assert!(
            matches!(operation.as_deref(), Some(NativeLaneError::Operation(message)) if message == "native rejected")
        );
        assert!(reconciled.load(Ordering::Acquire));
        assert!(payload_dropped.load(Ordering::Acquire));
        assert_eq!(budget.observation().used.running_jobs, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_lane_deadline_cleans_without_constructing_native_work() {
        let lane = lane();
        let budget = budget();
        let root = scopes();
        let constructed = Arc::new(AtomicBool::new(false));
        let work = constructed.clone();
        let mut request = admission(&root, &budget, ResourceClass::Data);
        request.deadline = Instant::now()
            .checked_sub(Duration::from_millis(1))
            .unwrap();
        let result = lane
            .spawn(
                request,
                move |_| {
                    work.store(true, Ordering::Release);
                    ready(Ok::<(), &'static str>(()))
                },
                NativeLaneCleanup {
                    before_join: || ready(Ok::<(), &'static str>(())),
                    after_join: |_| Ok::<(), &'static str>(()),
                },
            )
            .await
            .unwrap()
            .wait()
            .await
            .unwrap();
        assert!(matches!(result, Err(NativeLaneError::Deadline)));
        assert!(!constructed.load(Ordering::Acquire));
        assert_eq!(budget.observation().used.running_jobs, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_lane_repeated_runs_join_before_reusing_admission() {
        let lane = lane();
        let budget = budget();
        let root = scopes();
        let ids = Arc::new(std::sync::Mutex::new(Vec::new()));
        for expected in 0..5u64 {
            let ids = ids.clone();
            let outer = tokio::runtime::Handle::current().id();
            let result = lane
                .spawn(
                    admission(&root, &budget, ResourceClass::Data),
                    move |_| async move {
                        let native = tokio::runtime::Handle::current();
                        assert_ne!(native.id(), outer);
                        assert_eq!(
                            native.runtime_flavor(),
                            tokio::runtime::RuntimeFlavor::MultiThread
                        );
                        let value = tokio::task::spawn_blocking(move || expected).await.unwrap();
                        Ok::<u64, &'static str>(value)
                    },
                    NativeLaneCleanup {
                        before_join: || ready(Ok::<(), &'static str>(())),
                        after_join: move |token: NativeLaneJoined| {
                            ids.lock().unwrap().push(token.runtime_id());
                            Ok::<(), &'static str>(())
                        },
                    },
                )
                .await
                .unwrap()
                .wait()
                .await
                .unwrap()
                .unwrap();
            assert_eq!(result, expected);
            assert_eq!(budget.observation().used.running_jobs, 0);
        }
        let ids = ids.lock().unwrap();
        assert_eq!(ids.len(), 5);
        for (index, id) in ids.iter().enumerate() {
            assert!(!ids[..index].contains(id));
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_lane_retained_output_keeps_backing_charge_and_rejects_foreign_owner() {
        let lane = lane();
        let owner = budget();
        let root = scopes();
        for foreign in [false, true] {
            let backing = if foreign { budget() } else { owner.clone() };
            let allocation_owner = backing.clone();
            let result = lane
                .spawn(
                    admission(&root, &owner, ResourceClass::Data),
                    move |_| {
                        ready(ChargedSlice::try_from_fn(
                            &allocation_owner,
                            ResourceClass::Data,
                            16,
                            || vec![7u8; 16],
                        ))
                    },
                    NativeLaneCleanup {
                        before_join: || ready(Ok::<(), &'static str>(())),
                        after_join: |_| Ok::<(), &'static str>(()),
                    },
                )
                .await
                .unwrap()
                .wait()
                .await
                .unwrap();
            assert_eq!(owner.observation().used.running_jobs, 0);
            if foreign {
                assert!(matches!(
                    result,
                    Err(NativeLaneError::Budget(ResourceBudgetError::ForeignOwner))
                ));
            } else {
                let bytes = result.unwrap();
                let charged = owner.observation().used.memory_bytes;
                assert!(charged >= 16);
                let slice = bytes.slice(0..1).unwrap();
                drop(bytes);
                assert_eq!(owner.observation().used.memory_bytes, charged);
                assert_eq!(slice[0], 7);
                drop(slice);
            }
            assert_eq!(owner.observation().used.memory_bytes, 0);
            assert_eq!(backing.observation().used.memory_bytes, 0);
        }
    }

    fn delta_test_resources(
        budget: &ResourceBudget,
    ) -> crate::fabric::resource_ownership::WorkspaceFabricResources {
        use crate::fabric::datafusion_cache::DataFusionCachePolicy;
        use crate::fabric::resource_ownership::{
            NativeFabricResourceConfig, WorkspaceFabricResources,
        };
        WorkspaceFabricResources::try_new(
            budget.clone(),
            NativeFabricResourceConfig {
                memory_limit_bytes: 96 * 1024 * 1024,
                max_spill_bytes: 256,
                max_spill_merge_fan_in: 2,
                tracked_consumer_count: NonZeroUsize::new(16).unwrap(),
                cache_policy: DataFusionCachePolicy::try_new(1024, 1024, 1024, 1, 1, 1024).unwrap(),
            },
            Arc::new(datafusion::execution::object_store::DefaultObjectStoreRegistry::new()),
        )
        .unwrap()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_lane_delta_write_and_exact_kernel_read_share_workspace_pool() {
        use arrow_array::{Int64Array, RecordBatch};
        use arrow_schema::{DataType, Field, Schema};
        use datafusion::execution::SessionStateBuilder;
        use datafusion::execution::object_store::DefaultObjectStoreRegistry;
        use datafusion::prelude::SessionConfig;
        use deltalake::DeltaTableBuilder;
        use deltalake::delta_datafusion::SessionFallbackPolicy;
        use deltalake::operations::create::CreateBuilder;
        use futures::TryStreamExt;

        let lane = lane();
        let budget = budget();
        let root = scopes();
        let temporary = tempfile::tempdir().unwrap();
        let url = url::Url::from_directory_path(temporary.path()).unwrap();
        let resources = delta_test_resources(&budget);
        let runtime = resources
            .runtime_with_registry(Arc::new(DefaultObjectStoreRegistry::new()))
            .unwrap();
        let pool = runtime.memory_pool.clone();
        let retained_capacity = budget.observation().used.memory_bytes;
        let total = lane
            .spawn(
                admission(&root, &budget, ResourceClass::Data),
                move |_| async move {
                    // Real pinned Delta create, writer, snapshot/kernel replay, and stream read all live
                    // inside one native runtime. Neither the table nor its physical stream escapes.
                    let state = Arc::new(
                        SessionStateBuilder::new()
                            .with_default_features()
                            .with_query_planner(
                                deltalake::delta_datafusion::planner::DeltaPlanner::new(),
                            )
                            .with_config(SessionConfig::new().with_target_partitions(2))
                            .with_runtime_env(runtime)
                            .build(),
                    );
                    assert!(Arc::ptr_eq(&pool, &state.runtime_env().memory_pool));
                    let table = CreateBuilder::new()
                        .with_location(url.as_str())
                        .with_column("value", deltalake::kernel::DataType::LONG, false, None)
                        .await
                        .unwrap();
                    let batch = RecordBatch::try_new(
                        Arc::new(Schema::new(vec![Field::new(
                            "value",
                            DataType::Int64,
                            false,
                        )])),
                        vec![Arc::new(Int64Array::from(vec![1, 2, 3]))],
                    )
                    .unwrap();
                    let table = table
                        .write([batch])
                        .with_session_state(state.clone())
                        .with_session_fallback_policy(SessionFallbackPolicy::RequireSessionState)
                        .with_write_batch_size(3)
                        .await
                        .unwrap();
                    assert_eq!(table.version(), Some(1));
                    drop(table);
                    let exact = DeltaTableBuilder::from_url(url)
                        .unwrap()
                        .with_version(1)
                        .load()
                        .await
                        .unwrap();
                    let (table, mut stream) =
                        exact.scan_table().with_session_state(state).await.unwrap();
                    let mut sum = 0i64;
                    while let Some(batch) = stream.try_next().await.unwrap() {
                        let values = batch
                            .column(0)
                            .as_any()
                            .downcast_ref::<Int64Array>()
                            .unwrap();
                        sum += values.values().iter().sum::<i64>();
                    }
                    drop(stream);
                    drop(table);
                    Ok::<i64, &'static str>(sum)
                },
                NativeLaneCleanup {
                    before_join: || ready(Ok::<(), &'static str>(())),
                    after_join: |_| Ok::<(), &'static str>(()),
                },
            )
            .await
            .unwrap()
            .wait()
            .await
            .unwrap()
            .unwrap();
        assert_eq!(total, 6);
        assert_eq!(budget.observation().used.running_jobs, 0);
        assert_eq!(budget.observation().used.memory_bytes, retained_capacity);
        assert_eq!(resources.observation().native_reserved_bytes, 0);
    }

    #[test]
    fn native_lane_rejects_nested_entry_and_resets_reused_host_thread_after_panic() {
        let host = Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(1)
            .enable_all()
            .build()
            .unwrap();
        host.block_on(async {
            let lane = lane();
            let budget = budget();
            let root = scopes();
            let nested_root = root.clone();
            let nested_budget = budget.clone();
            let coordinator_id = Arc::new(std::sync::Mutex::new(None));
            let capture = coordinator_id.clone();
            let result = lane
                .spawn(
                    admission(&root, &budget, ResourceClass::Data),
                    move |_| async move {
                        *capture.lock().unwrap() = Some(std::thread::current().id());
                        let error = lane
                            .spawn(
                                admission(&nested_root, &nested_budget, ResourceClass::Data),
                                |_| ready(Ok::<(), &'static str>(())),
                                NativeLaneCleanup {
                                    before_join: || ready(Ok::<(), &'static str>(())),
                                    after_join: |_| Ok::<(), &'static str>(()),
                                },
                            )
                            .await
                            .unwrap_err();
                        assert!(matches!(
                            error,
                            NativeLaneError::AdmissionDenied(NativeAdmissionFailure::NestedLane)
                        ));
                        let denied = tokio::spawn(async move {
                            lane.spawn(
                                admission(&nested_root, &nested_budget, ResourceClass::Data),
                                |_| ready(Ok::<(), &'static str>(())),
                                NativeLaneCleanup {
                                    before_join: || ready(Ok::<(), &'static str>(())),
                                    after_join: |_| Ok::<(), &'static str>(()),
                                },
                            )
                            .await
                            .unwrap_err()
                        })
                        .await
                        .unwrap();
                        assert!(matches!(
                            denied,
                            NativeLaneError::AdmissionDenied(NativeAdmissionFailure::NestedLane)
                        ));
                        panic!("unwind the native coordinator");
                        #[allow(unreachable_code)]
                        Ok::<(), &'static str>(())
                    },
                    NativeLaneCleanup {
                        before_join: || ready(Ok::<(), &'static str>(())),
                        after_join: |_| Ok::<(), &'static str>(()),
                    },
                )
                .await
                .unwrap()
                .wait()
                .await
                .unwrap();
            assert!(matches!(result, Err(NativeLaneError::Panicked)));
            let expected_id = coordinator_id.lock().unwrap().unwrap();
            tokio::task::spawn_blocking(move || {
                assert_eq!(std::thread::current().id(), expected_id);
                assert!(!IN_NATIVE_LANE.get());
            })
            .await
            .unwrap();
            lane.spawn(
                admission(&root, &budget, ResourceClass::Data),
                |_| ready(Ok::<(), &'static str>(())),
                NativeLaneCleanup {
                    before_join: || ready(Ok::<(), &'static str>(())),
                    after_join: |_| Ok::<(), &'static str>(()),
                },
            )
            .await
            .unwrap()
            .wait()
            .await
            .unwrap()
            .unwrap();
            assert_eq!(budget.observation().used.running_jobs, 0);
            assert_eq!(lane.admitted_capacity().queued_jobs, 0);
        });
    }

    #[test]
    fn native_lane_rejects_insufficient_blocking_or_overflowed_envelopes() {
        let mut envelope = lane().envelope;
        envelope.blocking_threads = NonZeroUsize::new(1).unwrap();
        assert!(matches!(
            NativeExecutionLane::try_new(envelope),
            Err(NativeLaneError::InvalidEnvelope(_))
        ));
        envelope = lane().envelope;
        envelope.native_buffer_bytes = u64::MAX;
        assert!(matches!(
            NativeExecutionLane::try_new(envelope),
            Err(NativeLaneError::InvalidEnvelope(_))
        ));
        assert_eq!(diagnostic(&"é".repeat(5000)).len(), 4096);
    }
}
