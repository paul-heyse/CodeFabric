//! Borrowed context workers stay inside the existing source operation and its owned join.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{Scope, ScopedJoinHandle};

use crate::resource_budget::{ResourceAmounts, ResourceBudget, ResourceClass};

use super::{ProductionWorkspaceStartupError, step};

pub(super) fn spawn<'scope, 'env, T: Send + 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    budget: &ResourceBudget,
    name: &str,
    operation: impl FnOnce() -> T + Send + 'scope,
) -> Result<ScopedJoinHandle<'scope, T>, ProductionWorkspaceStartupError> {
    const STACK_BYTES: usize = 2 * 1024 * 1024;
    // Conservative stack/bookkeeping reservation; this does not equate virtual stacks to RSS.
    let owner = budget
        .try_reserve(
            ResourceClass::Data,
            ResourceAmounts {
                running_jobs: 1,
                memory_bytes: STACK_BYTES as u64 + 8192,
                ..ResourceAmounts::default()
            },
        )
        .map_err(|error| step("context-worker-admission", error))?;
    let runtime = tokio::runtime::Handle::current();
    std::thread::Builder::new()
        .name(name.into())
        .stack_size(STACK_BYTES)
        .spawn_scoped(scope, move || {
            let _owner = owner;
            let _runtime = runtime.enter();
            operation()
        })
        .map_err(|error| step("context-worker-start", error))
}

pub(super) fn join<T>(
    worker: ScopedJoinHandle<'_, T>,
) -> Result<T, ProductionWorkspaceStartupError> {
    worker
        .join()
        .map_err(|_| step("context-worker-join", "provider context worker panicked"))
}

/// A finite input list is its own FIFO backlog; do not create a thread or copied task per item.
/// Each output keeps its ordinal, so completion order cannot reorder admitted contexts.
pub(super) fn map<T: Sync, R: Send>(
    inputs: &[T],
    workers: usize,
    budget: &ResourceBudget,
    operation: impl Fn(&T) -> R + Sync,
) -> Result<Vec<R>, ProductionWorkspaceStartupError> {
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        let mut failure = None;
        for _ in 0..workers.max(1).min(inputs.len()) {
            match spawn(scope, budget, "cargo-context", || {
                let mut results = Vec::new();
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(input) = inputs.get(index) else {
                        break;
                    };
                    results.push((index, operation(input)));
                }
                results
            }) {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    failure = Some(error);
                    break;
                }
            }
        }
        let mut results = Vec::new();
        for handle in handles {
            match join(handle) {
                Ok(output) => results.extend(output),
                Err(error) => {
                    failure.get_or_insert(error);
                }
            }
        }
        if let Some(error) = failure {
            return Err(error);
        }
        results.sort_unstable_by_key(|(index, _)| *index);
        Ok(results.into_iter().map(|(_, output)| output).collect())
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Barrier;
    use std::sync::atomic::AtomicBool;

    use super::*;

    #[tokio::test]
    async fn borrowed_context_workers_bound_concurrency_preserve_order_and_join_failures() {
        let budget = crate::fabric::workspace_resources::test_workspace_budget();
        let rendezvous = Barrier::new(2);
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let results = map(&[0, 1, 2, 3], 2, &budget, |index| {
            let now = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(now, Ordering::SeqCst);
            if *index < 2 {
                rendezvous.wait();
            }
            active.fetch_sub(1, Ordering::SeqCst);
            index * 7
        })
        .unwrap();
        assert_eq!(results, [0, 7, 14, 21]);
        assert_eq!(peak.load(Ordering::SeqCst), 2);
        assert_eq!(budget.observation().used.running_jobs, 0);
        assert_eq!(budget.observation().used.memory_bytes, 0);

        let completed = AtomicBool::new(false);
        let result = map(&[0, 1], 2, &budget, |index| {
            rendezvous.wait();
            assert_ne!(*index, 0, "intentional context failure");
            completed.store(true, Ordering::SeqCst);
        });
        assert_eq!(result.unwrap_err().step, "context-worker-join");
        assert!(
            completed.load(Ordering::SeqCst),
            "a failed worker cannot detach its sibling"
        );
        assert_eq!(budget.observation().used.running_jobs, 0);
        assert_eq!(budget.observation().used.memory_bytes, 0);
    }
}
