//! Workspace-owned syntax accelerators. Published facts never depend on retaining this cache.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::provider_contracts::{ProviderJob, ProviderRunResult};
use crate::provider_native_rust_syntax::ExactRustSyntaxRunner;
use crate::provider_native_syntax::{
    ExactPythonSyntaxRunner, InProcessProviderJobs, ProviderNativeSourceImage,
    ProviderNativeSyntaxError, ProviderNativeSyntaxRun, PythonModuleInput,
};
use crate::resource_budget::{ResourceAmounts, ResourceBudget, ResourceClass, ResourceReservation};
use crate::source_image::InventoryCaptureBundle;

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct Key {
    language: u8,
    file: [u8; 16],
    context: [u8; 32],
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_native_syntax::{NativeSyntaxRelation, job_tests};

    #[test]
    fn retained_syntax_eviction_and_cancel_leave_owned_facts_and_allow_full_recovery() {
        let input = job_tests::source("def value():\n    return 7\n", 1);
        let jobs = job_tests::jobs(&input);
        let budget = crate::provider_contracts::fixture_provider_budget([6; 16], [0; 16]);
        let baseline = budget.observation().used.retained_generations;
        let mut cache = SyntaxCache::new(budget.clone());
        let first = cache
            .python(
                [3; 32],
                jobs.borrowed(),
                &input,
                job_tests::module(),
                1 << 26,
            )
            .unwrap();
        let count = first
            .relation(NativeSyntaxRelation::TreeSitterCstNode)
            .num_rows();
        assert!(count > 0);
        assert_eq!(cache.entries.len(), 1);

        let second = cache
            .python(
                [3; 32],
                jobs.borrowed(),
                &input,
                job_tests::module(),
                1 << 26,
            )
            .unwrap();
        assert_eq!(
            first.relation(NativeSyntaxRelation::TreeSitterCstNode),
            second.relation(NativeSyntaxRelation::TreeSitterCstNode)
        );
        assert_eq!(cache.observation.python_reuses, 1);
        assert_eq!(cache.observation.ruff_parse_reuses, 1);
        // The fixture's quarter-budget admits four 64 MiB native envelopes. A second tree
        // revision grows to five, which evicts the accelerator while returned Arrow stays owned.
        assert!(cache.entries.is_empty());
        assert_eq!(budget.observation().used.retained_generations, baseline);

        cache
            .python(
                [3; 32],
                jobs.borrowed(),
                &input,
                job_tests::module(),
                1 << 26,
            )
            .unwrap();
        cache.evict_idle(Instant::now() + Duration::from_secs(601));
        assert!(cache.entries.is_empty());
        assert_eq!(
            first
                .relation(NativeSyntaxRelation::TreeSitterCstNode)
                .num_rows(),
            count
        );

        cache
            .python(
                [3; 32],
                jobs.borrowed(),
                &input,
                job_tests::module(),
                1 << 26,
            )
            .unwrap();
        let cancelled = job_tests::jobs(&input);
        cancelled.ruff_owner.cancel();
        assert!(
            cache
                .python(
                    [3; 32],
                    cancelled.borrowed(),
                    &input,
                    job_tests::module(),
                    1 << 26
                )
                .is_err()
        );
        assert!(
            cache.entries.is_empty(),
            "Ruff failure drops the already-updated tree too"
        );
        let fresh = job_tests::jobs(&input);
        let recovered = cache
            .python(
                [3; 32],
                fresh.borrowed(),
                &input,
                job_tests::module(),
                1 << 26,
            )
            .unwrap();
        assert_eq!(
            first.relation(NativeSyntaxRelation::TreeSitterCstNode),
            recovered.relation(NativeSyntaxRelation::TreeSitterCstNode)
        );
        drop(cache);
        assert_eq!(budget.observation().used.retained_generations, baseline);
    }
}

enum Runner {
    Python(Box<ExactPythonSyntaxRunner>),
    Rust(Box<ExactRustSyntaxRunner>),
}

impl Runner {
    fn native_reservations(&self) -> ResourceAmounts {
        match self {
            Self::Python(runner) => runner.native_reservations(),
            Self::Rust(runner) => runner.native_reservations(),
        }
    }
}

struct Entry {
    runner: Runner,
    last_used: Instant,
    _metadata: ResourceReservation,
}

#[derive(Clone, Copy, Default, Serialize)]
pub(super) struct SyntaxCacheObservation {
    python_reuses: u64,
    rust_reuses: u64,
    ruff_parse_reuses: u64,
    evictions: u64,
    retained_entries: usize,
    native_reserved_bytes: u64,
}

pub(in crate::fabric) struct SyntaxCache {
    budget: ResourceBudget,
    entries: BTreeMap<Key, Entry>,
    observation: SyntaxCacheObservation,
}

impl SyntaxCache {
    pub(in crate::fabric) fn new(budget: ResourceBudget) -> Self {
        Self {
            budget,
            entries: BTreeMap::new(),
            observation: SyntaxCacheObservation::default(),
        }
    }

    pub(super) fn reconcile(&mut self, capture: &InventoryCaptureBundle) {
        let before = self.entries.len();
        self.entries.retain(|key, entry| {
            capture
                .images()
                .iter()
                .any(|image| image.file_id == key.file)
                && entry.last_used.elapsed() < Duration::from_secs(600)
        });
        self.observation.evictions += (before - self.entries.len()) as u64;
    }

    pub(super) fn evict_idle(&mut self, now: Instant) {
        let before = self.entries.len();
        self.entries.retain(|_, entry| {
            now.saturating_duration_since(entry.last_used) < Duration::from_secs(600)
        });
        self.observation.evictions += (before - self.entries.len()) as u64;
        self.trim(0);
    }

    pub(super) fn observation(&self) -> SyntaxCacheObservation {
        SyntaxCacheObservation {
            retained_entries: self.entries.len(),
            native_reserved_bytes: self
                .entries
                .values()
                .map(|entry| entry.runner.native_reservations().memory_bytes)
                .sum(),
            ..self.observation
        }
    }

    fn evict_oldest(&mut self) -> bool {
        let Some(key) = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(key, _)| *key)
        else {
            return false;
        };
        self.entries.remove(&key);
        self.observation.evictions += 1;
        true
    }

    fn trim(&mut self, working_bytes: u64) {
        loop {
            let used = self.budget.observation();
            let limits = used.policy.limits;
            let native = self.observation().native_reserved_bytes;
            // Preserve room for a complete parser/projection job and the rest of the workspace.
            // These are declared native envelopes, not measured RSS or allocator interception.
            let pressure = native > limits.retained_bytes / 4
                || used.used.memory_bytes + u128::from(working_bytes)
                    > u128::from(limits.memory_bytes - used.policy.control_reserve.memory_bytes)
                || used.used.retained_generations + 8 > u128::from(limits.retained_generations);
            if !pressure || !self.evict_oldest() {
                break;
            }
        }
    }

    fn take(&mut self, key: Key, working_bytes: u64) -> Option<Entry> {
        let before = self.entries.len();
        self.entries.retain(|candidate, _| {
            candidate.file != key.file
                || candidate.language != key.language
                || candidate.context == key.context
        });
        self.observation.evictions += (before - self.entries.len()) as u64;
        let entry = self.entries.remove(&key);
        self.trim(working_bytes);
        entry
    }

    fn metadata(&self) -> Result<ResourceReservation, ProviderNativeSyntaxError> {
        self.budget
            .try_reserve(
                ResourceClass::Data,
                ResourceAmounts {
                    memory_bytes: (std::mem::size_of::<Entry>()
                        + std::mem::size_of::<Key>()
                        + std::mem::size_of::<ExactPythonSyntaxRunner>()
                            .max(std::mem::size_of::<ExactRustSyntaxRunner>())
                        + 128) as u64,
                    ..ResourceAmounts::default()
                },
            )
            .map_err(crate::provider_contracts::ProviderContractError::from)
            .map_err(Into::into)
    }

    pub(super) fn python(
        &mut self,
        context: [u8; 32],
        jobs: InProcessProviderJobs<'_>,
        source: &ProviderNativeSourceImage,
        module: PythonModuleInput<'_>,
        maximum_output_bytes: u64,
    ) -> Result<ProviderNativeSyntaxRun, ProviderNativeSyntaxError> {
        let key = Key {
            language: 1,
            file: source.file_id,
            context,
        };
        let mut entry = match self.take(key, maximum_output_bytes.saturating_mul(8)) {
            Some(entry) => {
                self.observation.python_reuses += 1;
                entry
            }
            None => Entry {
                runner: Runner::Python(Box::new(ExactPythonSyntaxRunner::new(jobs)?)),
                last_used: Instant::now(),
                _metadata: self.metadata()?,
            },
        };
        let Runner::Python(runner) = &mut entry.runner else {
            unreachable!("language-specific key")
        };
        let before = runner.lifecycle_observation().ruff_reused_parses;
        // The entry is absent from the cache during mutation. Failure/cancellation drops both
        // providers, including a tree committed before a subsequent Ruff/projection failure.
        let result = runner.run_captured(jobs, source, module)?;
        self.observation.ruff_parse_reuses +=
            runner.lifecycle_observation().ruff_reused_parses - before;
        entry.last_used = Instant::now();
        self.entries.insert(key, entry);
        self.trim(0);
        Ok(result)
    }

    pub(super) fn rust(
        &mut self,
        job: &ProviderJob,
        source: &ProviderNativeSourceImage,
    ) -> Result<ProviderRunResult, ProviderNativeSyntaxError> {
        let key = Key {
            language: 2,
            file: source.file_id,
            context: job.context().context_fingerprint(),
        };
        let mut entry = match self.take(key, job.ceilings().max_bytes().saturating_mul(4)) {
            Some(entry) => {
                self.observation.rust_reuses += 1;
                entry
            }
            None => Entry {
                runner: Runner::Rust(Box::new(ExactRustSyntaxRunner::new(job)?)),
                last_used: Instant::now(),
                _metadata: self.metadata()?,
            },
        };
        let Runner::Rust(runner) = &mut entry.runner else {
            unreachable!("language-specific key")
        };
        let result = runner.run_captured(job, source)?;
        entry.last_used = Instant::now();
        self.entries.insert(key, entry);
        self.trim(0);
        Ok(result)
    }
}
