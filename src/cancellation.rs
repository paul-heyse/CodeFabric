//! Application cancellation identity and daemon-owned structured task scopes.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "daemon")]
use std::collections::BTreeMap;
#[cfg(feature = "daemon")]
use std::future::Future;
#[cfg(feature = "daemon")]
use std::num::NonZeroUsize;
#[cfg(feature = "daemon")]
use std::time::Duration;

#[cfg(feature = "daemon")]
use tokio::sync::{Mutex, oneshot};
#[cfg(feature = "daemon")]
use tokio::task::JoinHandle;
#[cfg(feature = "daemon")]
use tokio_util::sync::CancellationToken;

/// Cloneable cancellation state threaded from a control boundary to bounded work loops.
///
/// The polling interval is part of the handle so provider resource profiles can tighten
/// responsiveness without introducing provider-specific cancellation contracts.
#[derive(Clone, Debug)]
pub struct Cancellation {
    requested: Arc<AtomicBool>,
    check_interval: u32,
    #[cfg(feature = "daemon")]
    token: Option<CancellationToken>,
}

impl Default for Cancellation {
    fn default() -> Self {
        Self {
            requested: Arc::new(AtomicBool::new(false)),
            check_interval: u32::MAX,
            #[cfg(feature = "daemon")]
            token: None,
        }
    }
}

impl Cancellation {
    /// Construct a fresh handle with a bounded polling interval.
    #[must_use]
    pub fn with_check_interval(check_interval: u32) -> Self {
        Self {
            requested: Arc::new(AtomicBool::new(false)),
            check_interval: check_interval.max(1),
            #[cfg(feature = "daemon")]
            token: None,
        }
    }

    #[allow(dead_code)] // Used only by daemon/provider feature combinations.
    pub(crate) fn from_shared(requested: Arc<AtomicBool>, check_interval: u32) -> Self {
        Self {
            requested,
            check_interval: check_interval.max(1),
            #[cfg(feature = "daemon")]
            token: None,
        }
    }

    /// Bind a synchronous leaf probe to one daemon-owned structured scope.
    #[cfg(feature = "daemon")]
    #[must_use]
    pub(crate) fn from_scope(scope: &StructuredCancellationScope, check_interval: u32) -> Self {
        Self {
            requested: Arc::clone(&scope.requested),
            check_interval: check_interval.max(1),
            token: Some(scope.token.clone()),
        }
    }

    /// Request cancellation. Repeated requests are harmless.
    pub fn cancel(&self) {
        self.requested.store(true, Ordering::Release);
        #[cfg(feature = "daemon")]
        if let Some(token) = &self.token {
            token.cancel();
        }
    }

    /// Whether cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.requested.load(Ordering::Acquire) || {
            #[cfg(feature = "daemon")]
            {
                self.token
                    .as_ref()
                    .is_some_and(CancellationToken::is_cancelled)
            }
            #[cfg(not(feature = "daemon"))]
            {
                false
            }
        }
    }

    /// Work interval at which a bounded operation should poll this handle.
    #[must_use]
    pub const fn check_interval(&self) -> u32 {
        self.check_interval
    }

    #[allow(dead_code)] // Used only by repository-input/gix feature combinations.
    pub(crate) fn interrupt_flag(&self) -> &AtomicBool {
        self.requested.as_ref()
    }
}

/// One daemon-owned cancellation subtree backed by Tokio's structured token primitive.
///
/// All descendants share one task registry, while each child owns an independent token. Closing a
/// scope rejects new tasks in that subtree; closing a parent propagates to all descendants.
#[cfg(feature = "daemon")]
#[derive(Clone)]
pub(crate) struct StructuredCancellationScope {
    path: Arc<str>,
    token: CancellationToken,
    requested: Arc<AtomicBool>,
    registry: Arc<Mutex<StructuredTaskRegistry>>,
}

#[cfg(feature = "daemon")]
impl std::fmt::Debug for StructuredCancellationScope {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StructuredCancellationScope")
            .field("path", &self.path)
            .field("cancelled", &self.is_cancelled())
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "daemon")]
#[derive(Debug)]
struct StructuredTaskRegistry {
    maximum_tasks: NonZeroUsize,
    tasks: BTreeMap<String, JoinHandle<()>>,
}

/// Result observation is separate from task ownership: dropping it never cancels accepted work.
#[cfg(feature = "daemon")]
#[derive(Debug)]
pub(crate) struct TaskObservation<T> {
    receiver: oneshot::Receiver<T>,
}

#[cfg(feature = "daemon")]
impl<T> TaskObservation<T> {
    pub(crate) async fn wait(self) -> Result<T, StructuredTaskError> {
        self.receiver
            .await
            .map_err(|_| StructuredTaskError::ObservationClosed)
    }
}

#[cfg(feature = "daemon")]
impl StructuredCancellationScope {
    pub(crate) fn try_root(
        name: &str,
        maximum_tasks: NonZeroUsize,
    ) -> Result<Self, StructuredTaskError> {
        validate_segment(name)?;
        Ok(Self {
            path: Arc::from(name),
            token: CancellationToken::new(),
            requested: Arc::new(AtomicBool::new(false)),
            registry: Arc::new(Mutex::new(StructuredTaskRegistry {
                maximum_tasks,
                tasks: BTreeMap::new(),
            })),
        })
    }

    pub(crate) fn child(&self, name: &str) -> Result<Self, StructuredTaskError> {
        validate_segment(name)?;
        Ok(Self {
            path: Arc::from(format!("{}/{}", self.path, name)),
            token: self.token.child_token(),
            requested: Arc::new(AtomicBool::new(false)),
            registry: Arc::clone(&self.registry),
        })
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
        self.requested.store(true, Ordering::Release);
        self.token.cancel();
    }

    /// Wait until this scope or any parent requests cooperative cancellation.
    pub(crate) async fn cancelled(&self) {
        self.token.cancelled().await;
    }

    pub(crate) async fn spawn<F>(&self, name: &str, future: F) -> Result<(), StructuredTaskError>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        validate_segment(name)?;
        let key = self.task_key(name);
        self.reap_finished().await?;
        let mut registry = self.registry.lock().await;
        if self.is_cancelled() {
            return Err(StructuredTaskError::ScopeClosed(self.path.to_string()));
        }
        if registry.tasks.contains_key(&key) {
            return Err(StructuredTaskError::DuplicateTask(key));
        }
        if registry.tasks.len() >= registry.maximum_tasks.get() {
            return Err(StructuredTaskError::TaskCapacity {
                maximum: registry.maximum_tasks.get(),
            });
        }
        registry.tasks.insert(key, tokio::spawn(future));
        Ok(())
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
        let (sender, receiver) = oneshot::channel();
        self.spawn(name, async move {
            let outcome = future.await;
            let _ = sender.send(outcome);
        })
        .await?;
        Ok(TaskObservation { receiver })
    }

    /// Close this subtree, cooperatively cancel it, and observe every owned task.
    ///
    /// A task that ignores cancellation is aborted only after the shared cleanup reserve expires;
    /// all remaining handles are still awaited before the timeout is returned.
    pub(crate) async fn cancel_and_join(
        &self,
        cleanup_reserve: Duration,
    ) -> Result<(), StructuredTaskError> {
        if cleanup_reserve.is_zero() {
            return Err(StructuredTaskError::CleanupReserveExhausted {
                scope: self.path.to_string(),
            });
        }
        self.cancel();
        let prefix = format!("{}/", self.path);
        let mut tasks = {
            let mut registry = self.registry.lock().await;
            let keys = registry
                .tasks
                .keys()
                .filter(|key| key.starts_with(&prefix))
                .cloned()
                .collect::<Vec<_>>();
            keys.into_iter()
                .filter_map(|key| registry.tasks.remove(&key).map(|task| (key, task)))
                .collect::<Vec<_>>()
        };
        let deadline = tokio::time::Instant::now() + cleanup_reserve;
        for index in 0..tasks.len() {
            let key = tasks[index].0.clone();
            let outcome = tokio::time::timeout_at(deadline, &mut tasks[index].1).await;
            match outcome {
                Ok(Ok(())) => {}
                Ok(Err(source)) => {
                    abort_and_observe(&mut tasks[index + 1..]).await;
                    return Err(StructuredTaskError::Join { task: key, source });
                }
                Err(_) => {
                    tasks[index].1.abort();
                    let _ = (&mut tasks[index].1).await;
                    abort_and_observe(&mut tasks[index + 1..]).await;
                    return Err(StructuredTaskError::CleanupReserveExhausted {
                        scope: self.path.to_string(),
                    });
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn live_task_count(&self) -> usize {
        let prefix = format!("{}/", self.path);
        self.registry
            .lock()
            .await
            .tasks
            .keys()
            .filter(|key| key.starts_with(&prefix))
            .count()
    }

    async fn reap_finished(&self) -> Result<(), StructuredTaskError> {
        let finished = {
            let mut registry = self.registry.lock().await;
            let keys = registry
                .tasks
                .iter()
                .filter_map(|(key, task)| task.is_finished().then(|| key.clone()))
                .collect::<Vec<_>>();
            keys.into_iter()
                .filter_map(|key| registry.tasks.remove(&key).map(|task| (key, task)))
                .collect::<Vec<_>>()
        };
        for (key, task) in finished {
            task.await
                .map_err(|source| StructuredTaskError::Join { task: key, source })?;
        }
        Ok(())
    }

    fn task_key(&self, name: &str) -> String {
        format!("{}/{}", self.path, name)
    }
}

#[cfg(feature = "daemon")]
async fn abort_and_observe(tasks: &mut [(String, JoinHandle<()>)]) {
    for (_, task) in tasks.iter() {
        task.abort();
    }
    for (_, task) in tasks.iter_mut() {
        let _ = task.await;
    }
}

#[cfg(feature = "daemon")]
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

#[cfg(feature = "daemon")]
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
    #[error("structured task {task} failed to join: {source}")]
    Join {
        task: String,
        #[source]
        source: tokio::task::JoinError,
    },
    #[error("structured-task cleanup reserve was exhausted for {scope}")]
    CleanupReserveExhausted { scope: String },
    #[error("structured-task result observation closed before completion")]
    ObservationClosed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_share_idempotent_cancellation_and_polling_policy() {
        let first = Cancellation::with_check_interval(0);
        let second = first.clone();
        assert_eq!(first.check_interval(), 1);
        assert!(!second.is_cancelled());
        first.cancel();
        first.cancel();
        assert!(second.is_cancelled());
    }

    #[cfg(feature = "daemon")]
    #[tokio::test]
    async fn parent_propagates_without_cancelling_a_sibling_from_its_child() {
        let root =
            StructuredCancellationScope::try_root("daemon", NonZeroUsize::new(8).unwrap()).unwrap();
        let left = root.child("left").unwrap();
        let right = root.child("right").unwrap();
        let left_probe = left.probe(1);
        let right_probe = right.probe(1);

        left.cancel();
        assert!(left_probe.is_cancelled());
        assert!(!right_probe.is_cancelled());
        root.cancel();
        assert!(right_probe.is_cancelled());
    }

    #[cfg(feature = "daemon")]
    #[tokio::test]
    async fn structured_task_tree_ownership_integrity() {
        let root =
            StructuredCancellationScope::try_root("daemon", NonZeroUsize::new(8).unwrap()).unwrap();
        let query = root.child("query:1").unwrap();
        let probe = query.probe(1);
        let observation = query
            .spawn_observed("execution", async move {
                tokio::time::sleep(Duration::from_millis(10)).await;
                7_u8
            })
            .await
            .unwrap();
        drop(observation);
        assert!(!probe.is_cancelled());
        query.cancel_and_join(Duration::from_secs(1)).await.unwrap();
        assert_eq!(query.live_task_count().await, 0);
    }

    #[cfg(feature = "daemon")]
    #[tokio::test]
    async fn unjoined_task_and_cleanup_reserve_faults() {
        let root =
            StructuredCancellationScope::try_root("daemon", NonZeroUsize::new(8).unwrap()).unwrap();
        let leaked = root.child("leaked").unwrap();
        leaked
            .spawn("pending", std::future::pending::<()>())
            .await
            .unwrap();
        assert!(matches!(
            leaked.cancel_and_join(Duration::from_millis(1)).await,
            Err(StructuredTaskError::CleanupReserveExhausted { .. })
        ));
        assert_eq!(leaked.live_task_count().await, 0);
        assert!(matches!(
            leaked.spawn("replacement", async {}).await,
            Err(StructuredTaskError::ScopeClosed(_))
        ));
    }

    #[cfg(feature = "daemon")]
    #[tokio::test]
    async fn completed_tasks_are_observed_before_capacity_is_reused() {
        let root =
            StructuredCancellationScope::try_root("daemon", NonZeroUsize::new(1).unwrap()).unwrap();
        root.spawn("first", async {}).await.unwrap();
        tokio::task::yield_now().await;
        root.spawn("second", async {}).await.unwrap();
        root.cancel_and_join(Duration::from_secs(1)).await.unwrap();
        assert_eq!(root.live_task_count().await, 0);
    }
}
