//! One cooperative cancellation handle shared by every in-process operation.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Cloneable cancellation state threaded from a control boundary to bounded work loops.
///
/// The polling interval is part of the handle so provider resource profiles can tighten
/// responsiveness without introducing provider-specific cancellation contracts.
#[derive(Clone, Debug)]
pub struct Cancellation {
    requested: Arc<AtomicBool>,
    check_interval: u32,
}

impl Default for Cancellation {
    fn default() -> Self {
        Self {
            requested: Arc::new(AtomicBool::new(false)),
            check_interval: u32::MAX,
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
        }
    }

    #[allow(dead_code)] // Used only by daemon/provider feature combinations.
    pub(crate) fn from_shared(requested: Arc<AtomicBool>, check_interval: u32) -> Self {
        Self {
            requested,
            check_interval: check_interval.max(1),
        }
    }

    /// Request cancellation. Repeated requests are harmless.
    pub fn cancel(&self) {
        self.requested.store(true, Ordering::Release);
    }

    /// Whether cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }

    /// Work interval at which a bounded operation should poll this handle.
    #[must_use]
    pub const fn check_interval(&self) -> u32 {
        self.check_interval
    }

    #[allow(dead_code)] // Used only by repository-state/gix feature combinations.
    pub(crate) fn interrupt_flag(&self) -> &AtomicBool {
        self.requested.as_ref()
    }
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
}
