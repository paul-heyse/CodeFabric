//! Terminal ownership of native DataFusion futures and blocking closures.
//!
//! DataFusion owns its tasks and cancellation. Its supported tracer supplies synchronous
//! enrollment and a witness when each future is destroyed or blocking closure exits. These
//! witnesses are not Tokio join outcomes. They retain the admission owner through native
//! termination, including when the caller abandons its drain future. No cleanup task detaches.
//!
//! The finite envelope covers this instrumentation's bookkeeping, not allocator-complete RSS
//! or native operator memory (which remains governed by the workspace memory pool).

use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use datafusion::common::runtime::{JoinSetTracer, set_join_set_tracer};
use datafusion::physical_plan::{RecordBatchStream, SendableRecordBatchStream};
use futures::future::BoxFuture;
use futures::{FutureExt, Stream};
use tokio::sync::Notify;

use crate::cancellation::Cancellation;

/// Finite combined task/retained-stream admission profile; deeper topologies fail explicitly.
pub(crate) const MAX_NATIVE_TASKS: usize = 4096;
/// Includes erased-future/closure boxes, retained owner references, guards and allocator slack.
const TASK_BOOKKEEPING_BYTES: u64 = 512;
pub(crate) const NATIVE_BOOKKEEPING_BYTES: u64 =
    4096 + MAX_NATIVE_TASKS as u64 * TASK_BOOKKEEPING_BYTES;
/// Cleanup can outlive the execution deadline, while ownership persists beyond this wait bound.
pub(crate) const CLEANUP_RESERVE: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeOperationObservation {
    pub live_tasks: usize,
    pub peak_tasks: usize,
    pub completed_tasks: u64,
    pub live_streams: usize,
    pub completed_streams: u64,
    pub capacity_failures: u64,
    pub completed_operations: u64,
    pub cancelled_operations: u64,
    pub maximum_cancellation_to_terminal_millis: u64,
}

#[derive(Default)]
struct Observations {
    live: AtomicUsize,
    peak: AtomicUsize,
    completed: AtomicU64,
    live_streams: AtomicUsize,
    completed_streams: AtomicU64,
    capacity_failures: AtomicU64,
    completed_operations: AtomicU64,
    cancelled_operations: AtomicU64,
    maximum_cancellation_to_terminal_millis: AtomicU64,
}

#[derive(Clone, Default)]
pub(crate) struct NativeOperationRegistry(Arc<Observations>);

impl NativeOperationRegistry {
    pub(crate) fn observation(&self) -> NativeOperationObservation {
        NativeOperationObservation {
            live_tasks: self.0.live.load(Ordering::Acquire),
            peak_tasks: self.0.peak.load(Ordering::Acquire),
            completed_tasks: self.0.completed.load(Ordering::Acquire),
            live_streams: self.0.live_streams.load(Ordering::Acquire),
            completed_streams: self.0.completed_streams.load(Ordering::Acquire),
            capacity_failures: self.0.capacity_failures.load(Ordering::Acquire),
            completed_operations: self.0.completed_operations.load(Ordering::Acquire),
            cancelled_operations: self.0.cancelled_operations.load(Ordering::Acquire),
            maximum_cancellation_to_terminal_millis: self
                .0
                .maximum_cancellation_to_terminal_millis
                .load(Ordering::Acquire),
        }
    }

    pub(crate) fn operation(
        &self,
        cancellation: Cancellation,
        retention: impl Send + Sync + 'static,
    ) -> NativeOperation {
        NativeOperation(Arc::new(Operation {
            state: Mutex::new(State::default()),
            changed: Notify::new(),
            cancellation,
            aggregate: self.clone(),
            capacity: MAX_NATIVE_TASKS,
            _retention: Box::new(retention),
        }))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum NativeOperationError {
    #[error("another DataFusion task tracer is installed")]
    TracerConflict,
    #[error("native operation has already executed")]
    AlreadyExecuted,
    #[error("native operation task capacity {maximum} exceeded")]
    TaskCapacity { maximum: usize },
    #[error("native operation cleanup reserve exhausted; live work retains its admission")]
    CleanupReserveExhausted,
}

static TRACER: OwnerTracer = OwnerTracer;
static INSTALLED: OnceLock<Result<(), NativeOperationError>> = OnceLock::new();

pub(crate) fn install() -> Result<(), NativeOperationError> {
    *INSTALLED.get_or_init(|| {
        set_join_set_tracer(&TRACER).map_err(|_| NativeOperationError::TracerConflict)
    })
}

tokio::task_local! {
    static OWNER: Arc<Operation>;
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum RootState {
    #[default]
    NotStarted,
    Live,
    Closed,
    Terminal,
}

#[derive(Default)]
struct State {
    root: RootState,
    live: usize,
    exceeded: bool,
    cancelled_at: Option<Instant>,
}

struct Operation {
    state: Mutex<State>,
    changed: Notify,
    cancellation: Cancellation,
    aggregate: NativeOperationRegistry,
    capacity: usize,
    // Kept by all native guards, including queued closures and unpolled futures.
    _retention: Box<dyn Send + Sync>,
}

#[derive(Clone)]
pub(crate) struct NativeOperation(Arc<Operation>);

impl NativeOperation {
    pub(crate) fn scope<F: Future>(
        &self,
        future: F,
    ) -> Result<impl Future<Output = F::Output>, NativeOperationError> {
        let mut state = self.0.lock();
        if state.root != RootState::NotStarted {
            return Err(NativeOperationError::AlreadyExecuted);
        }
        state.root = RootState::Live;
        drop(state);
        Ok(OWNER.scope(
            Arc::clone(&self.0),
            RootFuture {
                future: Some(Box::pin(future)),
                guard: Some(RootGuard {
                    owner: Arc::clone(&self.0),
                    completed: false,
                }),
            },
        ))
    }

    pub(crate) fn cancel(&self) {
        self.0.cancel();
    }

    pub(crate) fn failure(&self) -> Option<NativeOperationError> {
        self.0
            .lock()
            .exceeded
            .then_some(NativeOperationError::TaskCapacity {
                maximum: self.0.capacity,
            })
    }

    pub(crate) async fn failed(&self) {
        loop {
            let changed = self.0.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.failure().is_some() {
                return;
            }
            changed.await;
        }
    }

    pub(crate) async fn drain(&self) {
        loop {
            let changed = self.0.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let done = {
                let state = self.0.lock();
                state.root != RootState::Live && state.live == 0
            };
            if done {
                return;
            }
            changed.await;
        }
    }
}

impl Operation {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn cancel(&self) {
        self.lock().cancelled_at.get_or_insert_with(Instant::now);
        self.cancellation.cancel();
        self.changed.notify_waiters();
    }

    fn enroll(self: &Arc<Self>, kind: NativeKind) -> NativeGuard {
        let mut state = self.lock();
        if state.live >= self.capacity {
            state.exceeded = true;
            state.cancelled_at.get_or_insert_with(Instant::now);
            self.aggregate
                .0
                .capacity_failures
                .fetch_add(1, Ordering::AcqRel);
            drop(state);
            self.cancellation.cancel();
            self.changed.notify_waiters();
            // The native hook has no fallible return and must preserve its erased output type.
            // Unwind synchronously BEFORE native scheduling; the owned execution boundary catches
            // this marker, destroys its producer, drains admitted children and reports capacity.
            std::panic::panic_any(NativeCapacityExceeded);
        }
        state.live += 1;
        match kind {
            NativeKind::Task => {
                let live = self.aggregate.0.live.fetch_add(1, Ordering::AcqRel) + 1;
                self.aggregate.0.peak.fetch_max(live, Ordering::AcqRel);
            }
            NativeKind::Stream => {
                self.aggregate.0.live_streams.fetch_add(1, Ordering::AcqRel);
            }
        }
        drop(state);
        NativeGuard {
            owner: Arc::clone(self),
            kind,
        }
    }

    fn observe_terminal(&self, state: &mut State) {
        if state.root == RootState::Closed && state.live == 0 {
            state.root = RootState::Terminal;
            self.aggregate
                .0
                .completed_operations
                .fetch_add(1, Ordering::AcqRel);
            if let Some(cancelled_at) = state.cancelled_at {
                self.aggregate
                    .0
                    .cancelled_operations
                    .fetch_add(1, Ordering::AcqRel);
                self.aggregate
                    .0
                    .maximum_cancellation_to_terminal_millis
                    .fetch_max(
                        u64::try_from(cancelled_at.elapsed().as_millis()).unwrap_or(u64::MAX),
                        Ordering::AcqRel,
                    );
            }
        }
    }
}

#[derive(Debug)]
struct NativeCapacityExceeded;

enum NativeKind {
    Task,
    Stream,
}

struct NativeGuard {
    owner: Arc<Operation>,
    kind: NativeKind,
}

impl Drop for NativeGuard {
    fn drop(&mut self) {
        let mut state = self.owner.lock();
        state.live -= 1;
        match self.kind {
            NativeKind::Task => {
                self.owner.aggregate.0.live.fetch_sub(1, Ordering::AcqRel);
                self.owner
                    .aggregate
                    .0
                    .completed
                    .fetch_add(1, Ordering::AcqRel);
            }
            NativeKind::Stream => {
                self.owner
                    .aggregate
                    .0
                    .live_streams
                    .fetch_sub(1, Ordering::AcqRel);
                self.owner
                    .aggregate
                    .0
                    .completed_streams
                    .fetch_add(1, Ordering::AcqRel);
            }
        }
        self.owner.observe_terminal(&mut state);
        drop(state);
        self.owner.changed.notify_waiters();
    }
}

struct RootGuard {
    owner: Arc<Operation>,
    completed: bool,
}

impl Drop for RootGuard {
    fn drop(&mut self) {
        if !self.completed {
            self.owner.cancel();
        }
        let mut state = self.owner.lock();
        state.root = RootState::Closed;
        self.owner.observe_terminal(&mut state);
        drop(state);
        self.owner.changed.notify_waiters();
    }
}

struct RootFuture<F: Future> {
    future: Option<Pin<Box<F>>>,
    guard: Option<RootGuard>,
}

impl<F: Future> Future for RootFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let result = this
            .future
            .as_mut()
            .expect("root future present")
            .as_mut()
            .poll(cx);
        if result.is_ready() {
            this.guard.as_mut().expect("root guard present").completed = true;
        }
        result
    }
}

impl<F: Future> Drop for RootFuture<F> {
    fn drop(&mut self) {
        // Native destructors may spawn children. The root and task-local remain live through Drop.
        drop(self.future.take());
        drop(self.guard.take());
    }
}

type NativeOutput = Box<dyn Any + Send>;

struct NativeFuture {
    future: Option<BoxFuture<'static, NativeOutput>>,
    guard: Option<NativeGuard>,
}

impl Future for NativeFuture {
    type Output = NativeOutput;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut()
            .future
            .as_mut()
            .expect("native future present")
            .as_mut()
            .poll(cx)
    }
}

impl Drop for NativeFuture {
    fn drop(&mut self) {
        drop(self.future.take());
        drop(self.guard.take());
    }
}

struct NativeBlock {
    block: Option<Box<dyn FnOnce() -> NativeOutput + Send>>,
    guard: Option<NativeGuard>,
}

impl NativeBlock {
    fn run(mut self) -> NativeOutput {
        self.block.take().expect("native closure present")()
    }
}

impl Drop for NativeBlock {
    fn drop(&mut self) {
        drop(self.block.take());
        drop(self.guard.take());
    }
}

struct OwnerTracer;

impl JoinSetTracer for OwnerTracer {
    fn trace_future(
        &self,
        future: BoxFuture<'static, NativeOutput>,
    ) -> BoxFuture<'static, NativeOutput> {
        let Ok(owner) = OWNER.try_with(Arc::clone) else {
            return future;
        };
        let guard = owner.enroll(NativeKind::Task);
        OWNER
            .scope(
                owner,
                NativeFuture {
                    future: Some(future),
                    guard: Some(guard),
                },
            )
            .boxed()
    }

    fn trace_block(
        &self,
        block: Box<dyn FnOnce() -> NativeOutput + Send>,
    ) -> Box<dyn FnOnce() -> NativeOutput + Send> {
        let Ok(owner) = OWNER.try_with(Arc::clone) else {
            return block;
        };
        let guard = owner.enroll(NativeKind::Task);
        let native_block = NativeBlock {
            block: Some(block),
            guard: Some(guard),
        };
        Box::new(move || OWNER.sync_scope(owner, || native_block.run()))
    }
}

/// Keep lazy stream polls and destruction inside the creating native operation. The retained
/// stream is itself enrolled: returning live native execution cannot falsely pass the drain.
pub(crate) fn bind_stream(stream: SendableRecordBatchStream) -> SendableRecordBatchStream {
    let Ok(owner) = OWNER.try_with(Arc::clone) else {
        return stream;
    };
    let guard = owner.enroll(NativeKind::Stream);
    Box::pin(NativeStream {
        stream: Some(stream),
        owner,
        guard: Some(guard),
    })
}

struct NativeStream {
    stream: Option<SendableRecordBatchStream>,
    owner: Arc<Operation>,
    guard: Option<NativeGuard>,
}

impl Stream for NativeStream {
    type Item = datafusion::common::Result<arrow_array::RecordBatch>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        OWNER.sync_scope(Arc::clone(&this.owner), || {
            this.stream
                .as_mut()
                .expect("native stream present")
                .as_mut()
                .poll_next(cx)
        })
    }
}

impl RecordBatchStream for NativeStream {
    fn schema(&self) -> arrow_schema::SchemaRef {
        self.stream
            .as_ref()
            .expect("native stream present")
            .schema()
    }
}

impl Drop for NativeStream {
    fn drop(&mut self) {
        OWNER.sync_scope(Arc::clone(&self.owner), || drop(self.stream.take()));
        drop(self.guard.take());
    }
}

#[cfg(test)]
mod tests;
