//! Bounded registry-owned operations; observing a result never substitutes for joining work.

use std::collections::BTreeMap;
use std::future::Future;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as ResourceMutex, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, oneshot};
use tokio::task::{AbortHandle, JoinError, JoinHandle};
use tokio_util::sync::CancellationToken;

use super::Cancellation;

/// Only genuinely asynchronous, cancellation-safe work may be forcibly aborted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TaskCancellationMode {
    AbortableAsync,
    /// Native CPU work and process cleanup must actually finish after observing cancellation.
    Cooperative,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AdmissionClass {
    Data,
    Control,
}

/// Every descendant shares the same finite registry, including reserved control operations.
#[derive(Clone)]
pub(crate) struct StructuredCancellationScope {
    path: Arc<str>,
    pub(super) token: CancellationToken,
    pub(super) requested: Arc<AtomicBool>,
    registry: Arc<Mutex<StructuredTaskRegistry>>,
    native_flags: NativeFlags,
    class: AdmissionClass,
    control_enabled: bool,
}

impl std::fmt::Debug for StructuredCancellationScope {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StructuredCancellationScope")
            .field("path", &self.path)
            .field("cancelled", &self.is_cancelled())
            .field("class", &self.class)
            .finish_non_exhaustive()
    }
}

struct StructuredTaskRegistry {
    data_capacity: usize,
    control_capacity: usize,
    data_tasks: usize,
    control_tasks: usize,
    tasks: BTreeMap<String, Arc<TaskEntry>>,
    native_flags: NativeFlags,
    joined_tasks: u64,
    cancelled_tasks_joined: u64,
    last_cancellation_to_join_millis: Option<u64>,
    maximum_cancellation_to_join_millis: Option<u64>,
}

/// Process-wide registry observations, not a claim that cancellation intent stopped any task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredOperationObservation {
    pub owned_data_tasks: usize,
    pub owned_control_tasks: usize,
    pub data_capacity: usize,
    pub control_capacity: usize,
    pub joined_tasks: u64,
    pub cancelled_tasks_joined: u64,
    pub last_cancellation_to_join_millis: Option<u64>,
    pub maximum_cancellation_to_join_millis: Option<u64>,
}

impl Drop for StructuredTaskRegistry {
    fn drop(&mut self) {
        // Explicit cancel_and_join is the shutdown contract. Emergency owner loss must still
        // signal native work and cannot release its guard while the worker remains alive.
        for flag in self
            .native_flags
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
        {
            flag.signal(Instant::now());
        }
        for entry in self.tasks.values() {
            entry.cancellation.cancel();
            if entry.mode == TaskCancellationMode::AbortableAsync {
                entry.abort.abort();
            }
        }
    }
}

type ResourceHold = Arc<ResourceMutex<Option<Box<dyn Send + 'static>>>>;
// Only admitted operations are indexed, not every scope ever constructed. This index is bounded
// by the same data/control task capacities and lets raw native atomic polling observe ancestors.
type NativeFlags = Arc<ResourceMutex<BTreeMap<String, NativeTaskSignal>>>;
struct NativeTaskSignal {
    flag: Arc<AtomicBool>,
    requested_at: Arc<ResourceMutex<Option<Instant>>>,
}

impl NativeTaskSignal {
    fn signal(&self, now: Instant) {
        self.requested_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_or_insert(now);
        self.flag.store(true, Ordering::Release);
    }
}
type JoinOutcome = Result<(), Arc<JoinError>>;

struct TaskEntry {
    state: Mutex<TaskState>,
    abort: AbortHandle,
    cancellation: CancellationToken,
    mode: TaskCancellationMode,
    class: AdmissionClass,
    resources: ResourceHold,
    requested_at: Arc<ResourceMutex<Option<Instant>>>,
    terminal_observed_at: OnceLock<Instant>,
}

struct TaskState {
    handle: Option<JoinHandle<()>>,
    outcome: Option<JoinOutcome>,
}

impl TaskEntry {
    async fn observe(&self) -> JoinOutcome {
        let mut state = self.state.lock().await;
        if let Some(outcome) = &state.outcome {
            return outcome.clone();
        }
        // Await by mutable reference while the handle remains in its registry-owned entry.
        // Dropping this joining future drops only the mutex guard, never the task handle.
        let outcome = state
            .handle
            .as_mut()
            .expect("unobserved task has a handle")
            .await
            .map_err(Arc::new);
        state.handle.take();
        let _ = self.terminal_observed_at.set(Instant::now());
        state.outcome = Some(outcome.clone());
        let resources = self
            .resources
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        drop(resources);
        outcome
    }
}

/// Dropping an observation neither cancels work nor gives back its task/resource reservations.
pub(crate) struct TaskObservation<T> {
    receiver: oneshot::Receiver<T>,
    key: String,
    entry: Arc<TaskEntry>,
    registry: Arc<Mutex<StructuredTaskRegistry>>,
}

impl<T> std::fmt::Debug for TaskObservation<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TaskObservation")
            .field("task", &self.key)
            .finish_non_exhaustive()
    }
}

impl<T> TaskObservation<T> {
    pub(crate) async fn wait(self) -> Result<T, StructuredTaskError> {
        let result = self.receiver.await;
        observe_registered(&self.registry, &self.key, &self.entry).await?;
        result.map_err(|_| StructuredTaskError::ObservationClosed)
    }
}

impl StructuredCancellationScope {
    pub(crate) fn try_root(
        name: &str,
        maximum_tasks: NonZeroUsize,
    ) -> Result<Self, StructuredTaskError> {
        Self::root(name, maximum_tasks.get(), 0)
    }

    /// Both classes are separately bounded and share one registry; control never borrows data slots.
    pub(crate) fn try_root_with_control_reserve(
        name: &str,
        maximum_data_tasks: NonZeroUsize,
        reserved_control_tasks: NonZeroUsize,
    ) -> Result<Self, StructuredTaskError> {
        Self::root(name, maximum_data_tasks.get(), reserved_control_tasks.get())
    }

    fn root(
        name: &str,
        data_capacity: usize,
        control_capacity: usize,
    ) -> Result<Self, StructuredTaskError> {
        validate_segment(name)?;
        data_capacity
            .checked_add(control_capacity)
            .ok_or(StructuredTaskError::InvalidCapacity)?;
        let native_flags = Arc::new(ResourceMutex::new(BTreeMap::new()));
        Ok(Self {
            path: Arc::from(name),
            token: CancellationToken::new(),
            requested: Arc::new(AtomicBool::new(false)),
            registry: Arc::new(Mutex::new(StructuredTaskRegistry {
                data_capacity,
                control_capacity,
                data_tasks: 0,
                control_tasks: 0,
                tasks: BTreeMap::new(),
                native_flags: Arc::clone(&native_flags),
                joined_tasks: 0,
                cancelled_tasks_joined: 0,
                last_cancellation_to_join_millis: None,
                maximum_cancellation_to_join_millis: None,
            })),
            native_flags,
            class: AdmissionClass::Data,
            control_enabled: control_capacity != 0,
        })
    }

    pub(crate) fn child(&self, name: &str) -> Result<Self, StructuredTaskError> {
        validate_segment(name)?;
        if self.is_cancelled() {
            return Err(StructuredTaskError::ScopeClosed(self.path.to_string()));
        }
        if self.path.len().saturating_add(name.len()).saturating_add(1) > 4096 {
            return Err(StructuredTaskError::InvalidScopeSegment(name.to_owned()));
        }
        Ok(Self {
            path: Arc::from(format!("{}/{name}", self.path)),
            token: self.token.child_token(),
            requested: Arc::new(AtomicBool::new(false)),
            registry: Arc::clone(&self.registry),
            native_flags: Arc::clone(&self.native_flags),
            class: self.class,
            control_enabled: self.control_enabled,
        })
    }

    pub(crate) fn child_control(&self, name: &str) -> Result<Self, StructuredTaskError> {
        if !self.control_enabled {
            return Err(StructuredTaskError::ControlCapacityUnavailable);
        }
        let mut child = self.child(name)?;
        child.class = AdmissionClass::Control;
        Ok(child)
    }

    #[must_use]
    pub(crate) fn probe(&self, check_interval: u32) -> Cancellation {
        Cancellation::from_scope(self, check_interval)
    }

    #[must_use]
    pub(crate) fn is_cancelled(&self) -> bool {
        self.requested.load(Ordering::Acquire) || self.token.is_cancelled()
    }

    pub(crate) fn cancel(&self) {
        let observed_at = Instant::now();
        self.requested.store(true, Ordering::Release);
        self.token.cancel();
        let prefix = format!("{}/", self.path);
        let flags = self
            .native_flags
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (_, flag) in flags
            .range(prefix.clone()..)
            .take_while(|(key, _)| key.starts_with(&prefix))
        {
            flag.signal(observed_at);
        }
    }

    pub(crate) async fn cancelled(&self) {
        self.token.cancelled().await;
    }

    pub(crate) async fn spawn<F>(&self, name: &str, future: F) -> Result<(), StructuredTaskError>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.spawn_async_owned(name, TaskCancellationMode::AbortableAsync, (), future)
            .await
            .map(drop)
    }

    pub(crate) async fn spawn_observed<T, F>(
        &self,
        name: &str,
        future: F,
    ) -> Result<TaskObservation<T>, StructuredTaskError>
    where
        T: Send + 'static,
        F: Future<Output = T> + Send + 'static,
    {
        self.spawn_async_owned(name, TaskCancellationMode::AbortableAsync, (), future)
            .await
    }

    pub(crate) async fn spawn_async_owned<T, G, F>(
        &self,
        name: &str,
        mode: super::TaskCancellationMode,
        resource_guard: G,
        future: F,
    ) -> Result<super::TaskObservation<T>, StructuredTaskError>
    where
        T: Send + 'static,
        G: Send + 'static,
        F: Future<Output = T> + Send + 'static,
    {
        self.spawn_owned(name, mode, resource_guard, move |resources, sender| {
            tokio::spawn(async move {
                let result = future.await;
                let _ = sender.send(result);
                drop(resources);
            })
        })
        .await
    }

    pub(crate) async fn spawn_blocking_owned<T, G, F>(
        &self,
        name: &str,
        resource_guard: G,
        operation: F,
    ) -> Result<TaskObservation<T>, StructuredTaskError>
    where
        T: Send + 'static,
        G: Send + 'static,
        F: FnOnce(Cancellation) -> T + Send + 'static,
    {
        let probe = self.probe(1);
        self.spawn_owned(
            name,
            TaskCancellationMode::Cooperative,
            resource_guard,
            move |resources, sender| {
                // Own the blocking JoinHandle itself, never an abortable async bridge to it.
                tokio::task::spawn_blocking(move || {
                    let result = operation(probe);
                    let _ = sender.send(result);
                    drop(resources);
                })
            },
        )
        .await
    }

    async fn spawn_owned<T, G, F>(
        &self,
        name: &str,
        mode: TaskCancellationMode,
        resource_guard: G,
        spawn: F,
    ) -> Result<TaskObservation<T>, StructuredTaskError>
    where
        T: Send + 'static,
        G: Send + 'static,
        F: FnOnce(ResourceHold, oneshot::Sender<T>) -> JoinHandle<()>,
    {
        validate_segment(name)?;
        self.reap_finished().await?;
        let key = format!("{}/{name}", self.path);
        let mut registry = self.registry.lock().await;
        // Admission and synchronous parent cancellation share this short lock. No await or
        // user operation executes under it; a racing cancellation cannot miss a newly added flag.
        let mut flags = self
            .native_flags
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.is_cancelled() {
            return Err(StructuredTaskError::ScopeClosed(self.path.to_string()));
        }
        if registry.tasks.contains_key(&key) {
            return Err(StructuredTaskError::DuplicateTask(key));
        }
        let (used, maximum) = match self.class {
            AdmissionClass::Data => (registry.data_tasks, registry.data_capacity),
            AdmissionClass::Control => (registry.control_tasks, registry.control_capacity),
        };
        if used >= maximum {
            return Err(StructuredTaskError::TaskCapacity { maximum });
        }
        let resources: ResourceHold = Arc::new(ResourceMutex::new(Some(Box::new(resource_guard))));
        let (sender, receiver) = oneshot::channel();
        let handle = spawn(Arc::clone(&resources), sender);
        let requested_at = Arc::new(ResourceMutex::new(None));
        let entry = Arc::new(TaskEntry {
            requested_at: Arc::clone(&requested_at),
            terminal_observed_at: OnceLock::new(),
            abort: handle.abort_handle(),
            cancellation: self.token.clone(),
            mode,
            class: self.class,
            resources,
            state: Mutex::new(TaskState {
                handle: Some(handle),
                outcome: None,
            }),
        });
        registry.tasks.insert(key.clone(), Arc::clone(&entry));
        flags.insert(
            key.clone(),
            NativeTaskSignal {
                flag: Arc::clone(&self.requested),
                requested_at,
            },
        );
        match self.class {
            AdmissionClass::Data => registry.data_tasks += 1,
            AdmissionClass::Control => registry.control_tasks += 1,
        }
        Ok(TaskObservation {
            receiver,
            key,
            entry,
            registry: Arc::clone(&self.registry),
        })
    }

    /// Signal cancellation and join. Expired cooperative work stays owned and capacity-charged.
    /// No task registry lock is held while awaiting work or its cleanup.
    pub(crate) async fn cancel_and_join(
        &self,
        cleanup_reserve: Duration,
    ) -> Result<(), StructuredTaskError> {
        self.cancel();
        let Some(deadline) = tokio::time::Instant::now()
            .checked_add(cleanup_reserve)
            .filter(|_| !cleanup_reserve.is_zero())
        else {
            return Err(StructuredTaskError::CleanupReserveExhausted {
                scope: self.path.to_string(),
            });
        };
        let tasks = self.snapshot(false).await;
        let mut failure = None;
        for (key, entry) in &tasks {
            match tokio::time::timeout_at(deadline, observe_registered(&self.registry, key, entry))
                .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    failure.get_or_insert(error);
                }
                Err(_) => {
                    let unfinished = tasks
                        .iter()
                        .filter(|(_, task)| !task.abort.is_finished())
                        .count();
                    tracing::warn!(scope = %self.path, unfinished, cleanup_millis = cleanup_reserve.as_millis(), "structured task cleanup deadline elapsed");
                    for (key, task) in tasks
                        .iter()
                        .filter(|(_, task)| !task.abort.is_finished())
                        .take(8)
                    {
                        tracing::warn!(scope = %self.path, task = %key, mode = ?task.mode, "task remains owned after cleanup deadline");
                    }
                    for (_, task) in &tasks {
                        if task.mode == TaskCancellationMode::AbortableAsync {
                            task.abort.abort();
                        }
                    }
                    // Async abort completion must actually be observed. Cooperative native/process
                    // work is never treated as stopped merely because its awaiter timed out.
                    for (key, task) in &tasks {
                        if task.mode == TaskCancellationMode::AbortableAsync
                            || task.abort.is_finished()
                        {
                            let _ = observe_registered(&self.registry, key, task).await;
                        }
                    }
                    return Err(StructuredTaskError::CleanupReserveExhausted {
                        scope: self.path.to_string(),
                    });
                }
            }
        }
        failure.map_or(Ok(()), Err)
    }

    pub(crate) async fn live_task_count(&self) -> Result<usize, StructuredTaskError> {
        self.reap_finished().await?;
        Ok(self.snapshot(false).await.len())
    }

    pub(crate) async fn process_operation_observation(&self) -> StructuredOperationObservation {
        let registry = self.registry.lock().await;
        StructuredOperationObservation {
            owned_data_tasks: registry.data_tasks,
            owned_control_tasks: registry.control_tasks,
            data_capacity: registry.data_capacity,
            control_capacity: registry.control_capacity,
            joined_tasks: registry.joined_tasks,
            cancelled_tasks_joined: registry.cancelled_tasks_joined,
            last_cancellation_to_join_millis: registry.last_cancellation_to_join_millis,
            maximum_cancellation_to_join_millis: registry.maximum_cancellation_to_join_millis,
        }
    }

    async fn snapshot(&self, finished: bool) -> Vec<(String, Arc<TaskEntry>)> {
        let prefix = format!("{}/", self.path);
        self.registry
            .lock()
            .await
            .tasks
            .iter()
            .filter(|(key, entry)| {
                if finished {
                    entry.abort.is_finished()
                } else {
                    key.starts_with(&prefix)
                }
            })
            .map(|(key, entry)| (key.clone(), Arc::clone(entry)))
            .collect()
    }

    async fn reap_finished(&self) -> Result<(), StructuredTaskError> {
        let mut failure = None;
        for (key, entry) in self.snapshot(true).await {
            if let Err(error) = observe_registered(&self.registry, &key, &entry).await {
                failure.get_or_insert(error);
            }
        }
        failure.map_or(Ok(()), Err)
    }
}

async fn observe_registered(
    registry: &Mutex<StructuredTaskRegistry>,
    key: &str,
    entry: &Arc<TaskEntry>,
) -> Result<(), StructuredTaskError> {
    let result = entry.observe().await;
    let mut registry = registry.lock().await;
    // Another observer may already have removed this task and reused its name.
    if registry
        .tasks
        .get(key)
        .is_some_and(|current| Arc::ptr_eq(current, entry))
    {
        registry.tasks.remove(key);
        registry.joined_tasks = registry.joined_tasks.saturating_add(1);
        if let Some(requested_at) = *entry
            .requested_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            && let Some(joined_at) = entry.terminal_observed_at.get()
        {
            let millis = u64::try_from(
                joined_at
                    .saturating_duration_since(requested_at)
                    .as_millis(),
            )
            .unwrap_or(u64::MAX);
            registry.cancelled_tasks_joined = registry.cancelled_tasks_joined.saturating_add(1);
            registry.last_cancellation_to_join_millis = Some(millis);
            registry.maximum_cancellation_to_join_millis = Some(
                registry
                    .maximum_cancellation_to_join_millis
                    .unwrap_or(0)
                    .max(millis),
            );
        }
        registry
            .native_flags
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(key);
        match entry.class {
            AdmissionClass::Data => registry.data_tasks -= 1,
            AdmissionClass::Control => registry.control_tasks -= 1,
        }
    }
    result.map_err(|source| StructuredTaskError::Join {
        task: key.to_owned(),
        source,
    })
}

fn validate_segment(value: &str) -> Result<(), StructuredTaskError> {
    if value.is_empty()
        || value.len() > 160
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
    {
        return Err(StructuredTaskError::InvalidScopeSegment(value.to_owned()));
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum StructuredTaskError {
    #[error("invalid structured-task scope segment {0:?}")]
    InvalidScopeSegment(String),
    #[error("structured-task scope is closed: {0}")]
    ScopeClosed(String),
    #[error("structured task is already owned: {0}")]
    DuplicateTask(String),
    #[error("structured-task capacity {maximum} is exhausted")]
    TaskCapacity { maximum: usize },
    #[error("structured-task capacity arithmetic overflowed")]
    InvalidCapacity,
    #[error("structured-task root has no reserved control capacity")]
    ControlCapacityUnavailable,
    #[error("structured task {task} failed to join: {source}")]
    Join {
        task: String,
        #[source]
        source: Arc<JoinError>,
    },
    #[error("structured-task cleanup reserve was exhausted for {scope}")]
    CleanupReserveExhausted { scope: String },
    #[error("structured-task result observation closed before completion")]
    ObservationClosed,
}

#[cfg(test)]
mod tests;
