//! Shared native worker allocation. A lease follows actual work ownership, not a waiting RPC.

use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use super::{ResourceAmounts, ResourceBudget, ResourceClass};
use crate::cancellation::Cancellation;

#[derive(Debug, thiserror::Error)]
pub(crate) enum NativeCpuError {
    #[error(transparent)]
    Budget(#[from] super::ResourceBudgetError),
    #[error("native CPU admission cancelled")]
    Cancelled,
    #[error("native CPU admission deadline exceeded")]
    Deadline,
    #[error("native CPU scheduler closed")]
    Closed,
    #[error("native CPU activity already has an owner")]
    Active,
}

pub(crate) struct NativeCpuPool {
    slots: Arc<Semaphore>,
    capacity: usize,
    budget: ResourceBudget,
    admissions: AtomicU64,
    peak_slots: AtomicUsize,
    waiters: AtomicUsize,
    wait_micros: AtomicU64,
    longest_wait_micros: AtomicU64,
}

#[derive(Clone, Copy, serde::Serialize)]
pub(crate) struct NativeCpuObservation {
    pub capacity: usize,
    pub allocated_slots: usize,
    pub peak_allocated_slots: usize,
    pub admissions: u64,
    pub waiting_contexts: usize,
    pub admission_wait_micros: u64,
    pub longest_admission_wait_micros: u64,
}

impl NativeCpuPool {
    pub(crate) fn new(capacity: NonZeroUsize, budget: ResourceBudget) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(capacity.get())),
            capacity: capacity.get(),
            budget,
            admissions: AtomicU64::new(0),
            peak_slots: AtomicUsize::new(0),
            waiters: AtomicUsize::new(0),
            wait_micros: AtomicU64::new(0),
            longest_wait_micros: AtomicU64::new(0),
        }
    }

    /// Wait for the selected width through Tokio's FIFO queue. Width does not depend on
    /// transient contention: Cargo exposes it to build scripts and checker pools retain it.
    pub(crate) async fn admit(
        &self,
        preferred: NonZeroUsize,
        cancellation: &Cancellation,
        timeout: Duration,
    ) -> Result<NativeCpuLease, NativeCpuError> {
        if cancellation.is_cancelled() {
            return Err(NativeCpuError::Cancelled);
        }
        let _queued = self.budget.try_reserve(
            ResourceClass::Data,
            ResourceAmounts {
                queued_jobs: 1,
                memory_bytes: 512,
                ..ResourceAmounts::default()
            },
        )?;
        self.waiters.fetch_add(1, Ordering::Relaxed);
        let waiting = AdmissionWait {
            pool: self,
            started: Instant::now(),
        };
        let workers = preferred.get().min(self.capacity);
        let acquire = Arc::clone(&self.slots)
            .acquire_many_owned(workers.try_into().expect("bounded native slot count"));
        tokio::pin!(acquire);
        let deadline = tokio::time::sleep(timeout);
        tokio::pin!(deadline);
        let permit = loop {
            tokio::select! {
                permit = &mut acquire => break permit.map_err(|_| NativeCpuError::Closed)?,
                () = &mut deadline => return Err(NativeCpuError::Deadline),
                () = tokio::time::sleep(Duration::from_millis(20)) => {
                    if cancellation.is_cancelled() { return Err(NativeCpuError::Cancelled); }
                }
            }
        };
        if cancellation.is_cancelled() {
            return Err(NativeCpuError::Cancelled);
        }
        drop(waiting);
        self.admissions.fetch_add(1, Ordering::Relaxed);
        self.peak_slots.fetch_max(
            self.capacity - self.slots.available_permits(),
            Ordering::Relaxed,
        );
        Ok(NativeCpuLease {
            workers,
            _permit: Arc::new(permit),
        })
    }

    pub(crate) fn observation(&self) -> NativeCpuObservation {
        NativeCpuObservation {
            capacity: self.capacity,
            allocated_slots: self.capacity - self.slots.available_permits(),
            peak_allocated_slots: self.peak_slots.load(Ordering::Relaxed),
            admissions: self.admissions.load(Ordering::Relaxed),
            waiting_contexts: self.waiters.load(Ordering::Relaxed),
            admission_wait_micros: self.wait_micros.load(Ordering::Relaxed),
            longest_admission_wait_micros: self.longest_wait_micros.load(Ordering::Relaxed),
        }
    }
}

/// Includes admitted, cancelled, timed-out and dropped wait futures. There is no detached
/// observer task and no interval in which losing an RPC forgets a queued context's ownership.
struct AdmissionWait<'a> {
    pool: &'a NativeCpuPool,
    started: Instant,
}

impl Drop for AdmissionWait<'_> {
    fn drop(&mut self) {
        let micros = u64::try_from(self.started.elapsed().as_micros()).unwrap_or(u64::MAX);
        self.pool.waiters.fetch_sub(1, Ordering::Relaxed);
        let _ =
            self.pool
                .wait_micros
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |previous| {
                    Some(previous.saturating_add(micros))
                });
        self.pool
            .longest_wait_micros
            .fetch_max(micros, Ordering::Relaxed);
    }
}

#[derive(Clone)]
pub(crate) struct NativeCpuLease {
    workers: usize,
    _permit: Arc<OwnedSemaphorePermit>,
}

impl NativeCpuLease {
    pub(crate) fn workers(&self) -> NonZeroUsize {
        NonZeroUsize::new(self.workers).expect("admitted nonempty share")
    }
}

/// A retained checker is idle between complete runs. Its process worker also owns this cell,
/// so cancelled construction, lost control handles and failed joins cannot release active slots.
#[derive(Clone, Default)]
pub(crate) struct NativeCpuActivity(Arc<Mutex<Option<NativeCpuLease>>>);

impl NativeCpuActivity {
    pub(crate) fn begin(&self, lease: NativeCpuLease) -> Result<(), NativeCpuError> {
        let mut active = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if active.is_some() {
            return Err(NativeCpuError::Active);
        }
        *active = Some(lease);
        Ok(())
    }

    /// Only a complete native response or a joined process owner may retire active CPU work.
    pub(crate) fn complete(&self) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }

    pub(crate) fn process_owner(&self) -> NativeCpuProcessOwner {
        NativeCpuProcessOwner(self.clone())
    }
}

pub(crate) struct NativeCpuProcessOwner(NativeCpuActivity);
impl Drop for NativeCpuProcessOwner {
    fn drop(&mut self) {
        self.0.complete();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn native_shares_are_bounded_cancel_waiters_and_follow_actual_owner_lifetime() {
        let budget = crate::fabric::workspace_resources::test_workspace_budget();
        let pool = NativeCpuPool::new(NonZeroUsize::new(3).unwrap(), budget.clone());
        let preferred = NonZeroUsize::new(2).unwrap();
        let first = pool
            .admit(preferred, &Cancellation::default(), Duration::from_secs(1))
            .await
            .unwrap();
        let second = pool
            .admit(
                NonZeroUsize::new(1).unwrap(),
                &Cancellation::default(),
                Duration::from_secs(1),
            )
            .await
            .unwrap();
        assert_eq!(first.workers().get(), 2);
        assert_eq!(second.workers().get(), 1);
        assert_eq!(pool.observation().allocated_slots, 3);
        let cancel = Cancellation::default();
        let waiting = pool.admit(preferred, &cancel, Duration::from_secs(1));
        tokio::pin!(waiting);
        tokio::select! {
            _ = &mut waiting => panic!("exceeded shared worker capacity"),
            () = tokio::time::sleep(Duration::from_millis(25)) => {}
        }
        cancel.cancel();
        assert!(matches!(waiting.await, Err(NativeCpuError::Cancelled)));
        assert_eq!(budget.observation().used.queued_jobs, 0);
        let activity = NativeCpuActivity::default();
        activity.begin(first).unwrap();
        let owner = activity.process_owner();
        drop(activity);
        assert_eq!(
            pool.observation().allocated_slots,
            3,
            "control handle loss cannot free a live worker"
        );
        drop(owner);
        assert_eq!(pool.observation().allocated_slots, 1);
        let retry = pool
            .admit(preferred, &Cancellation::default(), Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(retry.workers().get(), 2);
        drop((second, retry));
        assert_eq!(pool.observation().allocated_slots, 0);
        assert_eq!(pool.observation().peak_allocated_slots, 3);
        assert_eq!(pool.observation().waiting_contexts, 0);
        assert!(
            pool.observation().admission_wait_micros
                >= pool.observation().longest_admission_wait_micros
        );
    }

    #[tokio::test]
    async fn older_contexts_cannot_be_overtaken_and_dropped_waiters_release_the_queue() {
        let budget = crate::fabric::workspace_resources::test_workspace_budget();
        let pool = NativeCpuPool::new(NonZeroUsize::new(3).unwrap(), budget.clone());
        let cancellation = Cancellation::default();
        let width = NonZeroUsize::new(2).unwrap();
        let active = pool
            .admit(width, &cancellation, Duration::from_secs(1))
            .await
            .unwrap();
        let mut older = Box::pin(pool.admit(width, &cancellation, Duration::from_secs(1)));
        assert!(futures::poll!(&mut older).is_pending());
        let mut later = Box::pin(pool.admit(
            NonZeroUsize::new(1).unwrap(),
            &cancellation,
            Duration::from_secs(1),
        ));
        assert!(
            futures::poll!(&mut later).is_pending(),
            "a narrower later job cannot overtake the oldest context"
        );
        assert_eq!(pool.observation().waiting_contexts, 2);
        drop(older);
        let next = later.await.unwrap();
        assert_eq!(pool.observation().waiting_contexts, 0);
        assert_eq!(budget.observation().used.queued_jobs, 0);
        assert_eq!(pool.observation().allocated_slots, 3);
        drop((active, next));
        assert_eq!(pool.observation().allocated_slots, 0);
    }
}
