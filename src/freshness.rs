//! Query admission against the workspace source-reconciliation watermark.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use thiserror::Error;
use tokio::sync::Notify;

pub use crate::registries::FreshnessState;

/// Query-side freshness admission policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FreshnessAdmission {
    BestAvailable,
    AwaitLatest,
    RequireCurrent,
}

/// Closed failures for freshness admission.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum FreshnessError {
    #[error("FRESHNESS_STALE")]
    Stale,
    #[error("FRESHNESS_UNAVAILABLE")]
    Unavailable,
}

/// Workspace-scoped source freshness barrier shared by query admissions.
#[derive(Clone, Debug)]
pub struct FreshnessBarrier {
    admitted: Arc<AtomicU64>,
    reconciled: Arc<AtomicU64>,
    unavailable: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl Default for FreshnessBarrier {
    fn default() -> Self {
        Self {
            admitted: Arc::new(AtomicU64::new(0)),
            reconciled: Arc::new(AtomicU64::new(0)),
            unavailable: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
        }
    }
}

impl FreshnessBarrier {
    #[must_use]
    pub fn with_watermarks(admitted: u64, reconciled: u64, unavailable: bool) -> Self {
        Self {
            admitted: Arc::new(AtomicU64::new(admitted)),
            reconciled: Arc::new(AtomicU64::new(reconciled.min(admitted))),
            unavailable: Arc::new(AtomicBool::new(unavailable)),
            notify: Arc::new(Notify::new()),
        }
    }

    /// Admit a relevant source change before reconciliation begins.
    #[must_use]
    pub fn admit(&self) -> u64 {
        self.admitted.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Advance the reconciliation watermark monotonically.
    pub fn reconcile(&self, watermark: u64) {
        self.reconciled.fetch_max(watermark, Ordering::AcqRel);
        self.notify.notify_waiters();
    }

    /// Mark current source facts unavailable and wake pending admissions.
    pub fn mark_unavailable(&self) {
        self.unavailable.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }

    #[must_use]
    pub fn state(&self) -> FreshnessState {
        if self.unavailable.load(Ordering::Acquire) {
            FreshnessState::Unavailable
        } else if self.reconciled.load(Ordering::Acquire) >= self.admitted.load(Ordering::Acquire) {
            FreshnessState::Current
        } else {
            FreshnessState::PotentiallyStale
        }
    }

    /// Await the reconciliation watermark captured at query admission.
    ///
    /// # Errors
    ///
    /// Returns [`FreshnessError::Stale`] when strict admission cannot observe current state before
    /// the timeout, or [`FreshnessError::Unavailable`] when the workspace source is unavailable.
    pub async fn admit_query(
        &self,
        policy: FreshnessAdmission,
        timeout: Duration,
    ) -> Result<FreshnessState, FreshnessError> {
        let target = self.admitted.load(Ordering::Acquire);
        match policy {
            FreshnessAdmission::BestAvailable => return Ok(self.state()),
            FreshnessAdmission::RequireCurrent if self.state() != FreshnessState::Current => {
                return Err(FreshnessError::Stale);
            }
            FreshnessAdmission::RequireCurrent => return Ok(FreshnessState::Current),
            FreshnessAdmission::AwaitLatest => {}
        }
        tokio::time::timeout(timeout, async {
            loop {
                if self.unavailable.load(Ordering::Acquire) {
                    return Err(FreshnessError::Unavailable);
                }
                if self.reconciled.load(Ordering::Acquire) >= target {
                    return Ok(FreshnessState::Current);
                }
                self.notify.notified().await;
            }
        })
        .await
        .map_err(|_| FreshnessError::Stale)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn strict_and_waiting_admission_follow_the_captured_watermark() {
        let barrier = FreshnessBarrier::default();
        let watermark = barrier.admit();
        assert_eq!(barrier.state(), FreshnessState::PotentiallyStale);
        assert_eq!(
            barrier
                .admit_query(FreshnessAdmission::RequireCurrent, Duration::from_millis(5))
                .await,
            Err(FreshnessError::Stale)
        );

        let waiting = barrier.clone();
        let task = tokio::spawn(async move {
            waiting
                .admit_query(FreshnessAdmission::AwaitLatest, Duration::from_secs(1))
                .await
        });
        tokio::task::yield_now().await;
        barrier.reconcile(watermark);
        assert_eq!(task.await.unwrap(), Ok(FreshnessState::Current));
    }

    #[tokio::test]
    async fn unavailable_source_rejects_waiting_admission() {
        let barrier = FreshnessBarrier::with_watermarks(8, 7, false);
        barrier.mark_unavailable();
        assert_eq!(barrier.state(), FreshnessState::Unavailable);
        assert_eq!(
            barrier
                .admit_query(FreshnessAdmission::AwaitLatest, Duration::from_millis(5))
                .await,
            Err(FreshnessError::Unavailable)
        );
    }
}
