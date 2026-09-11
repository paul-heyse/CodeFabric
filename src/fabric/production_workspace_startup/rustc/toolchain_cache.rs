//! Immutable compiler inputs retained by the workspace, independently of Cargo output state.

use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::cancellation::Cancellation;
use crate::resource_budget::{ChargedValue, ResourceBudget};

use super::{ProductionWorkspaceStartupError, ToolchainInputs, step};

/// Deployment observations only. Captured bytes and their digests remain context authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FileWitness {
    path: PathBuf,
    resolved: PathBuf,
    metadata: (u64, u64, u64, u32, i64, i64, i64, i64),
}

impl FileWitness {
    pub(super) fn read(path: &Path) -> Result<Self, ProductionWorkspaceStartupError> {
        let resolved =
            std::fs::canonicalize(path).map_err(|error| step("rust-toolchain-file", error))?;
        let metadata =
            std::fs::metadata(&resolved).map_err(|error| step("rust-toolchain-file", error))?;
        Ok(Self {
            path: path.to_owned(),
            resolved,
            metadata: Self::metadata(&metadata),
        })
    }

    fn metadata(value: &std::fs::Metadata) -> (u64, u64, u64, u32, i64, i64, i64, i64) {
        (
            value.dev(),
            value.ino(),
            value.len(),
            value.mode(),
            value.mtime(),
            value.mtime_nsec(),
            value.ctime(),
            value.ctime_nsec(),
        )
    }

    pub(super) fn matches_file(&self, file: &std::fs::File) -> bool {
        file.metadata()
            .is_ok_and(|value| self.matches_metadata(&value))
    }

    pub(super) fn matches_metadata(&self, value: &std::fs::Metadata) -> bool {
        Self::metadata(value) == self.metadata
    }

    pub(super) fn unchanged(&self) -> bool {
        Self::read(&self.path).is_ok_and(|value| value == *self)
    }

    pub(super) fn memory_bytes(&self) -> u64 {
        (self.path.capacity() + self.resolved.capacity()) as u64
    }
}

#[derive(Eq, PartialEq)]
pub(super) struct ToolchainSelectionKey {
    pub root: PathBuf,
    pub host: String,
    pub extractor: PathBuf,
    pub linker_manifest: [u8; 32],
}

struct Retained {
    inputs: ChargedValue<ToolchainInputs>,
    last_used: Instant,
}

#[derive(Clone, Copy, Default, serde::Serialize)]
pub(in crate::fabric::production_workspace_startup) struct ToolchainCacheObservation {
    captures: u64,
    reuses: u64,
    evictions: u64,
    retained_entries: usize,
    retained_bytes: u64,
}

pub(in crate::fabric) struct ToolchainCache {
    budget: ResourceBudget,
    retained: Option<Retained>,
    observation: ToolchainCacheObservation,
}

impl ToolchainCache {
    pub(in crate::fabric) fn new(budget: ResourceBudget) -> Self {
        Self {
            budget,
            retained: None,
            observation: ToolchainCacheObservation::default(),
        }
    }

    pub(super) fn acquire(
        &mut self,
        cancellation: &Cancellation,
    ) -> Result<ChargedValue<ToolchainInputs>, ProductionWorkspaceStartupError> {
        self.evict_idle(Instant::now());
        // The host C driver/search selection is cheap relative to the sysroot and is freshly
        // captured for every pass. It must not inherit an old /etc/alternatives selection.
        let _selection_capacity = crate::inventory::reserve_memory(&self.budget, 64 * 1024 * 1024)
            .map_err(|error| step("rust-toolchain-selection-memory", error))?;
        let selection = super::select_toolchain_inputs(cancellation)?;
        self.acquire_selected(cancellation, selection)
    }

    fn acquire_selected(
        &mut self,
        cancellation: &Cancellation,
        selection: super::SelectedToolchainInputs,
    ) -> Result<ChargedValue<ToolchainInputs>, ProductionWorkspaceStartupError> {
        if let Some(mut retained) = self.retained.take() {
            let validation = retained.inputs.unchanged(cancellation);
            let unchanged = match validation {
                Ok(unchanged) => retained.inputs.selection == selection.key && unchanged,
                Err(error) => {
                    self.observation.evictions += 1;
                    return Err(error);
                }
            };
            if unchanged {
                retained.last_used = Instant::now();
                let inputs = retained.inputs.clone();
                self.retained = Some(retained);
                self.observation.reuses += 1;
                return Ok(inputs);
            }
            self.observation.evictions += 1;
        }
        let inputs = super::capture_toolchain(&self.budget, cancellation, selection)?;
        self.observation.captures += 1;
        self.retained = Some(Retained {
            inputs: inputs.clone(),
            last_used: Instant::now(),
        });
        Ok(inputs)
    }

    pub(in crate::fabric::production_workspace_startup) fn clear(&mut self) {
        self.observation.evictions += u64::from(self.retained.take().is_some());
    }

    pub(in crate::fabric::production_workspace_startup) fn evict_idle(&mut self, now: Instant) {
        let observation = self.budget.observation();
        let headroom = u128::from(
            observation
                .policy
                .limits
                .memory_bytes
                .saturating_sub(observation.policy.control_reserve.memory_bytes),
        )
        .saturating_sub(observation.used.memory_bytes);
        if self.retained.as_ref().is_some_and(|entry| {
            now.saturating_duration_since(entry.last_used) >= Duration::from_secs(600)
        }) || headroom < 2 * 1024 * 1024 * 1024
        {
            self.clear();
        }
    }

    pub(in crate::fabric::production_workspace_startup) fn observation(
        &self,
    ) -> ToolchainCacheObservation {
        ToolchainCacheObservation {
            retained_entries: usize::from(self.retained.is_some()),
            retained_bytes: self
                .retained
                .as_ref()
                .map_or(0, |entry| entry.inputs.memory_bytes().unwrap_or(u64::MAX)),
            ..self.observation
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn fixture() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("bin")).unwrap();
        std::fs::create_dir(root.path().join("lib")).unwrap();
        for (path, bytes) in [
            ("bin/cargo", "cargo"),
            ("bin/rustc", "rustc"),
            ("lib/runtime.so", "first runtime"),
            ("extractor", "extractor"),
        ] {
            std::fs::write(root.path().join(path), bytes).unwrap();
        }
        root
    }

    fn selection(root: &Path) -> super::super::SelectedToolchainInputs {
        super::super::SelectedToolchainInputs {
            key: ToolchainSelectionKey {
                root: root.to_owned(),
                host: "test-host".to_owned(),
                extractor: root.join("extractor"),
                linker_manifest: [0; 32],
            },
            linker: Vec::new(),
            runtime_artifacts: Vec::new(),
        }
    }

    #[test]
    fn retained_toolchain_reuses_immutable_bytes_and_detects_same_size_deployment_changes() {
        let root = fixture();
        let budget = crate::fabric::workspace_resources::test_workspace_budget();
        let baseline = budget.observation().used.memory_bytes;
        let mut cache = ToolchainCache::new(budget.clone());
        let cancellation = Cancellation::default();
        let first = cache
            .acquire_selected(&cancellation, selection(root.path()))
            .unwrap();
        let repeated = cache
            .acquire_selected(&cancellation, selection(root.path()))
            .unwrap();
        assert!(
            std::ptr::eq(std::ptr::from_ref(&*first), std::ptr::from_ref(&*repeated)),
            "lease shares the original charged capture"
        );
        assert_eq!(cache.observation.reuses, 1);
        let runtime = root.path().join("lib/runtime.so");
        let old_mtime = std::fs::metadata(&runtime).unwrap().modified().unwrap();
        std::fs::write(&runtime, b"other runtime").unwrap();
        std::fs::File::open(&runtime)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(old_mtime))
            .unwrap();
        let replaced = cache
            .acquire_selected(&cancellation, selection(root.path()))
            .unwrap();
        assert_ne!(
            first.dependencies.manifest_digest,
            replaced.dependencies.manifest_digest
        );
        assert!(
            first
                .dependencies
                .entries
                .iter()
                .any(|entry| &*entry.bytes == b"first runtime")
        );
        assert_eq!(cache.observation.captures, 2);
        cache.clear();
        assert!(
            budget.observation().used.memory_bytes > baseline,
            "active immutable leases keep their charge after eviction"
        );
        drop((first, repeated, replaced));
        assert_eq!(budget.observation().used.memory_bytes, baseline);
    }

    #[test]
    fn retained_toolchain_fences_added_removed_and_retargeted_inputs() {
        let root = fixture();
        let mut cache =
            ToolchainCache::new(crate::fabric::workspace_resources::test_workspace_budget());
        let cancellation = Cancellation::default();
        let first = cache
            .acquire_selected(&cancellation, selection(root.path()))
            .unwrap();
        std::fs::write(root.path().join("lib/another.so"), b"extra").unwrap();
        let added = cache
            .acquire_selected(&cancellation, selection(root.path()))
            .unwrap();
        assert_ne!(
            first.dependencies.manifest_digest,
            added.dependencies.manifest_digest
        );
        std::fs::remove_file(root.path().join("lib/another.so")).unwrap();
        let removed = cache
            .acquire_selected(&cancellation, selection(root.path()))
            .unwrap();
        assert_eq!(
            first.dependencies.manifest_digest,
            removed.dependencies.manifest_digest
        );
        std::fs::write(root.path().join("other-extractor"), b"next extractor").unwrap();
        std::fs::remove_file(root.path().join("extractor")).unwrap();
        symlink("other-extractor", root.path().join("extractor")).unwrap();
        let replaced = cache
            .acquire_selected(&cancellation, selection(root.path()))
            .unwrap();
        assert_ne!(
            first.dependencies.manifest_digest,
            replaced.dependencies.manifest_digest
        );
        std::fs::remove_file(root.path().join("lib/runtime.so")).unwrap();
        symlink("/etc/passwd", root.path().join("lib/runtime.so")).unwrap();
        assert!(
            cache
                .acquire_selected(&cancellation, selection(root.path()))
                .is_err(),
            "escaped toolchain inputs are rejected before reading"
        );
        assert!(
            cache.retained.is_none(),
            "failed replacement cannot reinstall an obsolete capture"
        );
    }

    #[test]
    fn retained_toolchain_changes_selection_and_expires_without_invalidating_leases() {
        let root = fixture();
        let mut cache =
            ToolchainCache::new(crate::fabric::workspace_resources::test_workspace_budget());
        let cancellation = Cancellation::default();
        let first = cache
            .acquire_selected(&cancellation, selection(root.path()))
            .unwrap();
        let mut changed = selection(root.path());
        changed.key.linker_manifest = [1; 32];
        let second = cache.acquire_selected(&cancellation, changed).unwrap();
        assert!(!std::ptr::eq(
            std::ptr::from_ref(&*first),
            std::ptr::from_ref(&*second)
        ));
        cache.evict_idle(Instant::now() + Duration::from_secs(601));
        assert!(cache.retained.is_none());
        assert!(!second.dependencies.entries.is_empty());
        let third = cache
            .acquire_selected(&cancellation, selection(root.path()))
            .unwrap();
        cancellation.cancel();
        assert!(
            cache
                .acquire_selected(&cancellation, selection(root.path()))
                .is_err()
        );
        assert!(cache.retained.is_none());
        assert_eq!(first.dependencies, third.dependencies);
        assert!(
            cache
                .acquire_selected(&Cancellation::default(), selection(root.path()))
                .is_ok()
        );
    }

    #[test]
    fn toolchain_capture_rejects_a_replaced_open_file_and_cancelled_reads() {
        let root = fixture();
        let path = root.path().join("lib/runtime.so");
        let witness = FileWitness::read(&path).unwrap();
        std::fs::write(root.path().join("replacement"), b"other runtime").unwrap();
        std::fs::rename(root.path().join("replacement"), &path).unwrap();
        assert!(
            super::super::capture_file(&path, &witness, 1024, &Cancellation::default()).is_err()
        );
        let current = FileWitness::read(&path).unwrap();
        let cancelled = Cancellation::default();
        cancelled.cancel();
        assert!(super::super::capture_file(&path, &current, 1024, &cancelled).is_err());
        assert!(super::super::capture_file(&path, &current, 4, &Cancellation::default()).is_err());
    }
}
