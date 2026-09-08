//! Application cancellation identity and daemon-owned structured task scopes.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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

#[cfg(feature = "daemon")]
mod owned;
#[cfg(feature = "daemon")]
pub use owned::StructuredOperationObservation;
#[cfg(feature = "daemon")]
pub(crate) use owned::{
    StructuredCancellationScope, StructuredTaskError, TaskCancellationMode, TaskObservation,
};

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "daemon")]
    use std::{num::NonZeroUsize, time::Duration};

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
        assert_eq!(query.live_task_count().await.expect("query task count"), 0);
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
        assert_eq!(
            leaked.live_task_count().await.expect("leaked task count"),
            0
        );
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
        assert_eq!(root.live_task_count().await.expect("root task count"), 0);
    }
}
