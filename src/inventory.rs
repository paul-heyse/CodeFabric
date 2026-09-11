//! Bounded descriptor-relative source inventory and Merkle root construction.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use rusqlite::{OptionalExtension as _, params};
use thiserror::Error;

use crate::cancellation::Cancellation;
use crate::identity::{IdentityError, WorkspacePath, source_file_identity};
use crate::operational_store::{OperationalStore, OperationalStoreError};
use crate::resource_budget::{
    ChargedSlice, ChargedValue, ResourceAmounts, ResourceBudget, ResourceBudgetError,
    ResourceClass, ResourceReservation, ResourceScopeKind,
};
use crate::secure_path::{
    PlatformPath, SecureDirectoryEntryKind, SecurePathError, SecureRoot, StableReadError,
};
use crate::source_image::ORDINARY_SOURCE_MAXIMUM_BYTES;

/// The six independently configurable generic-walker dimensions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InventoryLimits {
    pub maximum_file_count: u64,
    pub maximum_directory_count: u64,
    pub maximum_directory_depth: u32,
    pub maximum_total_bytes_considered: u64,
    pub maximum_duration: Duration,
    pub maximum_entries_per_directory: usize,
}

impl Default for InventoryLimits {
    fn default() -> Self {
        Self {
            maximum_file_count: 1_000_000,
            maximum_directory_count: 100_000,
            maximum_directory_depth: 128,
            maximum_total_bytes_considered: 16 * 1024 * 1024 * 1024,
            maximum_duration: Duration::from_mins(5),
            maximum_entries_per_directory: 100_000,
        }
    }
}

pub use crate::registries::{
    GitInventoryClassification as InventoryClassification, InventoryFileKind,
    InventoryInclusionState as InclusionState,
};

/// All Lifecycle §34 fields, detached from filesystem and Git library types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceInventoryRecord {
    pub path: ChargedValue<WorkspacePath>,
    pub git_repo_path_bytes: Option<ChargedSlice<u8>>,
    pub filesystem_identity: Option<[u8; 16]>,
    pub file_id: Option<[u8; 16]>,
    pub content_digest: Option<[u8; 32]>,
    pub byte_length: u64,
    pub file_kind: InventoryFileKind,
    pub language: Option<&'static str>,
    pub classification: InventoryClassification,
    pub inclusion: InclusionState,
    pub git_blob_oid: Option<ChargedSlice<u8>>,
    pub current_file_owner: Option<[u8; 16]>,
}

/// One coherent current inventory and its Merkle root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceInventory {
    pub workspace_id: [u8; 16],
    pub source_generation: u64,
    pub records: ChargedSlice<SourceInventoryRecord>,
    pub digest: [u8; 32],
    #[cfg(feature = "daemon")]
    pub(crate) git_context:
        Option<ChargedValue<crate::git_state::captured_inputs::CapturedGitInputs>>,
}

#[cfg(feature = "daemon")]
impl SourceInventory {
    pub(crate) fn git_context_digest(&self) -> [u8; 32] {
        self.git_context
            .as_ref()
            .map_or([0; 32], |context| context.digest)
    }
}

/// A full authorized walk, never a changed-work subset. Construction is fenced by its owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompleteSourceInventory {
    inventory: SourceInventory,
    change_token: u64,
    read_failures: ChargedValue<BTreeMap<Vec<u8>, InventoryReadFailure>>,
}

/// A member remains in the inventory even when its bytes cannot yet be verified.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryReadFailure {
    Unreadable,
    Pending,
}

impl CompleteSourceInventory {
    #[must_use]
    pub const fn inventory(&self) -> &SourceInventory {
        &self.inventory
    }

    #[must_use]
    pub const fn change_token(&self) -> u64 {
        self.change_token
    }

    #[must_use]
    pub fn read_failure(&self, path: &[u8]) -> Option<InventoryReadFailure> {
        self.read_failures.get(path).copied()
    }
}

/// Changed members of a separately identified complete inventory, not a replacement inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChangedSourcePaths {
    workspace_id: [u8; 16],
    inventory_digest: [u8; 32],
    source_generation: u64,
    paths: BTreeSet<Vec<u8>>,
    removed_paths: BTreeSet<Vec<u8>>,
    predecessor: Option<(u64, [u8; 32])>,
}

impl ChangedSourcePaths {
    /// Bind unique changed paths to a complete inventory.
    ///
    /// # Errors
    /// Rejects duplicate paths and paths absent from the full inventory. Deletions belong in
    /// explicit withdrawal work, not this set of currently selected members.
    pub fn try_new(
        inventory: &CompleteSourceInventory,
        paths: impl IntoIterator<Item = Vec<u8>>,
    ) -> Result<Self, InventoryError> {
        let members = inventory
            .inventory
            .records
            .iter()
            .map(|record| record.path.raw_relative_path_bytes.as_slice())
            .collect::<BTreeSet<_>>();
        let mut changed = BTreeSet::new();
        for path in paths {
            if !members.contains(path.as_slice()) || !changed.insert(path) {
                return Err(InventoryError::InvalidChangedWork);
            }
        }
        Ok(Self {
            workspace_id: inventory.inventory.workspace_id,
            inventory_digest: inventory.inventory.digest,
            source_generation: inventory.inventory.source_generation,
            paths: changed,
            removed_paths: BTreeSet::new(),
            predecessor: None,
        })
    }

    /// Bind present changes and checked removals to two complete inventories.
    ///
    /// # Errors
    /// Rejects unrelated workspaces, non-forward generations, duplicates, and paths absent
    /// from both inventories. A removed path must have existed in the predecessor.
    pub fn try_between(
        previous: &CompleteSourceInventory,
        current: &CompleteSourceInventory,
        paths: impl IntoIterator<Item = Vec<u8>>,
    ) -> Result<Self, InventoryError> {
        if previous.inventory.workspace_id != current.inventory.workspace_id
            || previous.inventory.source_generation >= current.inventory.source_generation
        {
            return Err(InventoryError::InvalidChangedWork);
        }
        let current_paths = current
            .inventory
            .records
            .iter()
            .map(|record| record.path.raw_relative_path_bytes.as_slice())
            .collect::<BTreeSet<_>>();
        let previous_paths = previous
            .inventory
            .records
            .iter()
            .map(|record| record.path.raw_relative_path_bytes.as_slice())
            .collect::<BTreeSet<_>>();
        let mut present = BTreeSet::new();
        let mut removed = BTreeSet::new();
        for path in paths {
            let selected = if current_paths.contains(path.as_slice()) {
                &mut present
            } else if previous_paths.contains(path.as_slice()) {
                &mut removed
            } else {
                return Err(InventoryError::InvalidChangedWork);
            };
            if !selected.insert(path) {
                return Err(InventoryError::InvalidChangedWork);
            }
        }
        let mut changed = Self::try_new(current, present)?;
        changed.removed_paths = removed;
        changed.predecessor = Some((
            previous.inventory.source_generation,
            previous.inventory.digest,
        ));
        Ok(changed)
    }

    /// Exact predecessor generation and full inventory digest supporting removals.
    #[must_use]
    pub const fn predecessor(&self) -> Option<(u64, [u8; 32])> {
        self.predecessor
    }

    #[must_use]
    pub const fn workspace_id(&self) -> [u8; 16] {
        self.workspace_id
    }

    #[must_use]
    pub const fn removed_paths(&self) -> &BTreeSet<Vec<u8>> {
        &self.removed_paths
    }

    #[must_use]
    pub const fn inventory_digest(&self) -> [u8; 32] {
        self.inventory_digest
    }

    #[must_use]
    pub const fn source_generation(&self) -> u64 {
        self.source_generation
    }

    #[must_use]
    pub const fn paths(&self) -> &BTreeSet<Vec<u8>> {
        &self.paths
    }
}

/// One stable current-byte replacement used to advance an existing inventory generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventoryFileUpsert {
    pub path: WorkspacePath,
    pub file_id: [u8; 16],
    pub content_digest: [u8; 32],
    pub byte_length: u64,
    pub language: Option<&'static str>,
}

/// Operational observations for one walk.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InventoryMetrics {
    pub files: u64,
    pub directories: u64,
    pub bytes_considered: u64,
    pub excluded_files: u64,
    pub duration_micros: u64,
}

#[derive(Debug, Error)]
pub enum InventoryError {
    #[error(transparent)]
    Resource(#[from] ResourceBudgetError),
    #[error(transparent)]
    SecurePath(#[from] SecurePathError),
    #[error(transparent)]
    StableRead(#[from] StableReadError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Store(#[from] OperationalStoreError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("inventory was cooperatively cancelled")]
    Cancelled,
    #[error("inventory exceeded the configured {0} bound")]
    BoundExceeded(&'static str),
    #[error("source changed during inventory")]
    SourceChanged,
    #[error("changed work is not a unique subset of the complete source inventory")]
    InvalidChangedWork,
}

/// Generic non-Git walker. WP17 may replace classification, never authorization.
pub struct InventoryWalker {
    budget: ResourceBudget,
    #[cfg(test)]
    fixture: bool,
    limits: InventoryLimits,
    metrics: InventoryMetrics,
    read_failures: BTreeMap<Vec<u8>, InventoryReadFailure>,
    read_failure_charge: Option<ResourceReservation>,
}

impl InventoryWalker {
    #[must_use]
    pub fn new_governed(limits: InventoryLimits, budget: ResourceBudget) -> Self {
        Self {
            budget,
            #[cfg(test)]
            fixture: false,
            limits,
            metrics: InventoryMetrics {
                files: 0,
                directories: 0,
                bytes_considered: 0,
                excluded_files: 0,
                duration_micros: 0,
            },
            read_failures: BTreeMap::new(),
            read_failure_charge: None,
        }
    }

    #[cfg(test)]
    pub fn new(limits: InventoryLimits) -> Self {
        let mut walker = Self::new_governed(
            limits,
            crate::provider_types::source_fixture_budget([2; 16]),
        );
        walker.fixture = true;
        walker
    }

    /// Walk, hash, and persist one coherent inventory.
    ///
    /// # Errors
    ///
    /// Returns cancellation, any named bound, path authorization, mutation, or SQL failure.
    pub fn walk_and_persist(
        &mut self,
        root: &SecureRoot,
        store: &mut OperationalStore,
        source_generation: u64,
        cancellation: &Cancellation,
    ) -> Result<SourceInventory, InventoryError> {
        let inventory = self.walk(root, store, source_generation, cancellation, false)?;
        persist_inventory(
            store,
            root.workspace_id(),
            source_generation,
            &inventory.records,
        )?;
        Ok(inventory)
    }

    /// Walk the entire selected root with an owner-supplied generation/change-token fence.
    /// Unreadable and unstable regular members are retained as explicit observations.
    ///
    /// # Errors
    /// Rejects an unstable fence, incomplete directory enumeration, cancellation, or any bound.
    pub fn walk_selected_with_fence(
        &mut self,
        root: &SecureRoot,
        store: &mut OperationalStore,
        source_generation: u64,
        change_token: u64,
        cancellation: &Cancellation,
        mut observe_change_token: impl FnMut() -> Option<u64>,
    ) -> Result<CompleteSourceInventory, InventoryError> {
        if observe_change_token() != Some(change_token) {
            return Err(InventoryError::SourceChanged);
        }
        let inventory = self.walk(root, store, source_generation, cancellation, true)?;
        require_source_generation(store, root.workspace_id(), source_generation)?;
        if observe_change_token() != Some(change_token) {
            return Err(InventoryError::SourceChanged);
        }
        persist_inventory(
            store,
            root.workspace_id(),
            source_generation,
            &inventory.records,
        )?;
        let failures = std::mem::take(&mut self.read_failures);
        let failure_charge = self
            .read_failure_charge
            .take()
            .ok_or(ResourceBudgetError::InvalidShrink)?;
        Ok(CompleteSourceInventory {
            inventory,
            change_token,
            read_failures: failure_charge.into_charged_value(failures),
        })
    }

    fn walk(
        &mut self,
        root: &SecureRoot,
        store: &mut OperationalStore,
        source_generation: u64,
        cancellation: &Cancellation,
        retain_read_failures: bool,
    ) -> Result<SourceInventory, InventoryError> {
        #[cfg(test)]
        let check_owner = !self.fixture;
        #[cfg(not(test))]
        let check_owner = true;
        if check_owner
            && self
                .budget
                .ancestor_owner(ResourceScopeKind::Workspace)
                .is_none_or(|owner| owner.id != root.workspace_id())
        {
            return Err(ResourceBudgetError::ForeignOwner.into());
        }
        require_source_generation(store, root.workspace_id(), source_generation)?;
        let started = Instant::now();
        #[cfg(feature = "daemon")]
        let mut inclusion = crate::source_inclusion::SourceInclusionPolicy::capture_secure(root);
        #[cfg(not(feature = "daemon"))]
        let inclusion = crate::source_inclusion::SourceInclusionPolicy::default();
        #[cfg(feature = "daemon")]
        let mut policy_charge = reserve_memory(&self.budget, inclusion.retained_bytes())?;
        let mut retained = reserve_memory(&self.budget, 0)?;
        let mut traversal = reserve_memory(&self.budget, 256)?;
        let mut records = Vec::new();
        let mut stack = vec![(Vec::<Vec<u8>>::new(), 0_u32)];
        self.metrics = InventoryMetrics::default();
        self.read_failures.clear();
        self.read_failure_charge = Some(reserve_memory(&self.budget, 0)?);
        while let Some((components, depth)) = stack.pop() {
            self.check_progress(started, cancellation)?;
            self.metrics.directories = self.metrics.directories.saturating_add(1);
            if self.metrics.directories > self.limits.maximum_directory_count {
                return Err(InventoryError::BoundExceeded("directory-count"));
            }
            let raw = join_components(&components);
            #[cfg(feature = "daemon")]
            {
                let before = inclusion.retained_bytes();
                inclusion.observe_secure_directory(root, &raw);
                policy_charge.try_grow(ResourceAmounts {
                    memory_bytes: inclusion.retained_bytes().saturating_sub(before),
                    ..ResourceAmounts::default()
                })?;
            }

            let platform_path = if components.is_empty() {
                None
            } else {
                Some(PlatformPath::from_raw_relative_bytes(
                    root.platform_code(),
                    raw,
                )?)
            };
            let mut directory_charge = reserve_memory(&self.budget, 0)?;
            let entries = root.list_directory_with_allocation(
                platform_path.as_ref(),
                self.limits.maximum_entries_per_directory,
                |bytes| {
                    if cancellation.is_cancelled()
                        || started.elapsed() > self.limits.maximum_duration
                    {
                        return Err(SecurePathError::ResourceExhausted);
                    }
                    directory_charge
                        .try_grow(ResourceAmounts {
                            memory_bytes: bytes as u64,
                            ..ResourceAmounts::default()
                        })
                        .map_err(|_| SecurePathError::ResourceExhausted)
                },
            );
            self.check_progress(started, cancellation)?;
            let entries = entries?;
            for entry in entries.into_iter().rev() {
                self.check_progress(started, cancellation)?;
                let path_len = components
                    .iter()
                    .map(Vec::len)
                    .sum::<usize>()
                    .checked_add(components.len() + entry.name.len())
                    .ok_or(ResourceBudgetError::Overflow)?;
                // This bounds simultaneous component copies, PlatformPath components and
                // stack vector replacements. It is based on the observed path, not file bytes.
                traversal.try_grow(ResourceAmounts {
                    memory_bytes: (path_len as u64)
                        .checked_mul(8)
                        .and_then(|n| {
                            n.checked_add(
                                ((components.len() + 1) * 4 * std::mem::size_of::<Vec<u8>>() + 256)
                                    as u64,
                            )
                        })
                        .ok_or(ResourceBudgetError::Overflow)?,
                    ..ResourceAmounts::default()
                })?;
                let mut child = components.clone();
                child.push(entry.name.clone());
                if !inclusion.includes(
                    &join_components(&child),
                    entry.kind == SecureDirectoryEntryKind::Directory,
                ) {
                    continue;
                }
                match entry.kind {
                    SecureDirectoryEntryKind::Directory => {
                        let next_depth = depth.saturating_add(1);
                        if next_depth > self.limits.maximum_directory_depth {
                            return Err(InventoryError::BoundExceeded("directory-depth"));
                        }
                        stack.push((child, next_depth));
                    }
                    SecureDirectoryEntryKind::RegularFile => {
                        reserve_record(&mut retained, &mut records)?;
                        self.add_regular(
                            root,
                            &child,
                            entry.size,
                            &mut records,
                            retain_read_failures,
                        )?;
                    }
                    SecureDirectoryEntryKind::Symlink | SecureDirectoryEntryKind::Other => {
                        reserve_record(&mut retained, &mut records)?;
                        self.add_excluded(root, &child, entry.kind, entry.size, &mut records)?;
                    }
                }
            }
        }
        records.sort_unstable_by(|left, right| {
            left.path
                .raw_relative_path_bytes
                .cmp(&right.path.raw_relative_path_bytes)
        });
        #[cfg(feature = "daemon")]
        let git_context = crate::git_state::captured_inputs::capture(
            &root.git_metadata_location(),
            &records,
            &inclusion,
            InventoryLimits {
                maximum_duration: self
                    .limits
                    .maximum_duration
                    .saturating_sub(started.elapsed()),
                ..self.limits
            },
            &self.budget,
            cancellation,
        )?;
        #[cfg(feature = "daemon")]
        for context in &git_context.paths {
            let Some(classification) = context.classification else {
                continue;
            };
            let index = records
                .binary_search_by(|record| record.path.raw_relative_path_bytes.cmp(&context.path))
                .expect("Git metadata only describes captured source members");
            let record = &mut records[index];
            record.classification = InventoryClassification::try_from(classification)
                .expect("classification originates in the closed native adapter");
            let relative = if context.repository_root.is_empty() {
                context.path.as_slice()
            } else {
                &context.path[context.repository_root.len() + 1..]
            };
            record.git_repo_path_bytes = Some(ChargedSlice::try_from_fn(
                &self.budget,
                ResourceClass::Data,
                relative.len(),
                || relative.to_vec(),
            )?);
            if let [stage] = context.stages.as_slice()
                && stage.stage == 0
            {
                record.git_blob_oid = Some(ChargedSlice::try_from_fn(
                    &self.budget,
                    ResourceClass::Data,
                    stage.object_id.len() + 1,
                    || {
                        let mut encoded = Vec::with_capacity(stage.object_id.len() + 1);
                        encoded.push(if stage.object_id.len() == 20 { 1 } else { 2 });
                        encoded.extend_from_slice(&stage.object_id);
                        encoded
                    },
                )?);
            }
        }
        let _merkle = reserve_memory(&self.budget, merkle_memory_bound(&records)?)?;
        #[cfg(feature = "daemon")]
        if !inclusion.unchanged_secure(root) {
            return Err(InventoryError::SourceChanged);
        }
        let digest = merkle_inventory_digest(&records);
        self.metrics.duration_micros =
            u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        retained.shrink(ResourceAmounts {
            memory_bytes: retained.amounts().memory_bytes
                - (records.capacity() * std::mem::size_of::<SourceInventoryRecord>()) as u64,
            ..ResourceAmounts::default()
        })?;
        Ok(SourceInventory {
            workspace_id: root.workspace_id(),
            source_generation,
            records: retained.into_charged_vec(records)?,
            digest,
            #[cfg(feature = "daemon")]
            git_context: Some(git_context),
        })
    }

    #[must_use]
    pub const fn metrics(&self) -> InventoryMetrics {
        self.metrics
    }

    fn add_regular(
        &mut self,
        root: &SecureRoot,
        components: &[Vec<u8>],
        observed_size: u64,
        records: &mut Vec<SourceInventoryRecord>,
        retain_read_failures: bool,
    ) -> Result<(), InventoryError> {
        self.metrics.files = self.metrics.files.saturating_add(1);
        if self.metrics.files > self.limits.maximum_file_count {
            return Err(InventoryError::BoundExceeded("file-count"));
        }
        self.metrics.bytes_considered = self
            .metrics
            .bytes_considered
            .checked_add(observed_size)
            .ok_or(InventoryError::BoundExceeded("total-bytes"))?;
        let path = PlatformPath::from_raw_relative_bytes(
            root.platform_code(),
            join_components(components),
        )?;
        let workspace_path = charged_workspace_path(root, &path, &self.budget)?;
        if self.metrics.bytes_considered > self.limits.maximum_total_bytes_considered {
            return Err(InventoryError::BoundExceeded("total-bytes"));
        }
        let mut read_charge = reserve_memory(&self.budget, 0)?;
        let (digest, filesystem_identity, inclusion, byte_length) = match root
            .read_stable_file_with_allocation(&path, ORDINARY_SOURCE_MAXIMUM_BYTES, |bytes| {
                read_charge
                    .try_grow(ResourceAmounts {
                        memory_bytes: bytes as u64,
                        ..ResourceAmounts::default()
                    })
                    .map_err(|_| SecurePathError::ResourceExhausted)
            }) {
            Ok(read) => (
                Some(crate::integrity::digest_bytes(&read.bytes)),
                Some(filesystem_identity(
                    read.metadata.device,
                    read.metadata.inode,
                )),
                InclusionState::Included,
                read.metadata.size,
            ),
            Err(StableReadError::SizeLimitExceeded { .. }) => {
                self.metrics.excluded_files = self.metrics.excluded_files.saturating_add(1);
                (None, None, InclusionState::ExcludedSizeLimit, observed_size)
            }
            Err(StableReadError::ChangedDuringRead) if retain_read_failures => {
                self.reserve_failure_path(&workspace_path.raw_relative_path_bytes)?;
                self.read_failures.insert(
                    workspace_path.raw_relative_path_bytes.clone(),
                    InventoryReadFailure::Pending,
                );
                (None, None, InclusionState::Included, observed_size)
            }
            Err(StableReadError::Secure(
                SecurePathError::SourceAccessDenied
                | SecurePathError::OperatingSystem
                | SecurePathError::OutsideAuthorizedRoot,
            )) if retain_read_failures => {
                self.reserve_failure_path(&workspace_path.raw_relative_path_bytes)?;
                self.read_failures.insert(
                    workspace_path.raw_relative_path_bytes.clone(),
                    InventoryReadFailure::Unreadable,
                );
                (None, None, InclusionState::Included, observed_size)
            }
            Err(StableReadError::ChangedDuringRead) => return Err(InventoryError::SourceChanged),
            Err(error) => return Err(error.into()),
        };
        let file_id = Some(source_file_identity(&workspace_path)?.id);
        records.push(SourceInventoryRecord {
            language: classify_language(&workspace_path.raw_relative_path_bytes),
            path: workspace_path,
            git_repo_path_bytes: None,
            filesystem_identity,
            file_id,
            content_digest: digest,
            byte_length,
            file_kind: InventoryFileKind::Regular,
            classification: InventoryClassification::UntrackedNotIgnored,
            inclusion,
            git_blob_oid: None,
            current_file_owner: None,
        });
        Ok(())
    }

    fn add_excluded(
        &mut self,
        root: &SecureRoot,
        components: &[Vec<u8>],
        kind: SecureDirectoryEntryKind,
        size: u64,
        records: &mut Vec<SourceInventoryRecord>,
    ) -> Result<(), InventoryError> {
        self.metrics.files = self.metrics.files.saturating_add(1);
        self.metrics.excluded_files = self.metrics.excluded_files.saturating_add(1);
        if self.metrics.files > self.limits.maximum_file_count {
            return Err(InventoryError::BoundExceeded("file-count"));
        }
        let platform = PlatformPath::from_raw_relative_bytes(
            root.platform_code(),
            join_components(components),
        )?;
        records.push(SourceInventoryRecord {
            path: charged_workspace_path(root, &platform, &self.budget)?,
            git_repo_path_bytes: None,
            filesystem_identity: None,
            file_id: None,
            content_digest: None,
            byte_length: size,
            file_kind: if kind == SecureDirectoryEntryKind::Symlink {
                InventoryFileKind::Symlink
            } else {
                InventoryFileKind::Special
            },
            language: None,
            classification: InventoryClassification::SpecialFile,
            inclusion: InclusionState::ExcludedSpecialFile,
            git_blob_oid: None,
            current_file_owner: None,
        });
        Ok(())
    }

    fn check_progress(
        &self,
        started: Instant,
        cancellation: &Cancellation,
    ) -> Result<(), InventoryError> {
        if cancellation.is_cancelled() {
            return Err(InventoryError::Cancelled);
        }
        if started.elapsed() > self.limits.maximum_duration {
            return Err(InventoryError::BoundExceeded("duration"));
        }
        Ok(())
    }

    fn reserve_failure_path(&mut self, path: &[u8]) -> Result<(), ResourceBudgetError> {
        self.read_failure_charge
            .as_mut()
            .ok_or(ResourceBudgetError::InvalidShrink)?
            .try_grow(ResourceAmounts {
                memory_bytes: path.len() as u64 + 512,
                ..ResourceAmounts::default()
            })
    }
}

/// Admission helpers remain application-owned; no filesystem or provider type enters the ledger.
pub(crate) fn reserve_memory(
    budget: &ResourceBudget,
    bytes: u64,
) -> Result<ResourceReservation, ResourceBudgetError> {
    budget.try_reserve(
        ResourceClass::Data,
        ResourceAmounts {
            memory_bytes: bytes,
            ..ResourceAmounts::default()
        },
    )
}

fn reserve_record(
    charge: &mut ResourceReservation,
    records: &mut Vec<SourceInventoryRecord>,
) -> Result<(), ResourceBudgetError> {
    charge.try_grow(ResourceAmounts {
        memory_bytes: 0,
        rows: 1,
        ..ResourceAmounts::default()
    })?;
    if records.len() == records.capacity() {
        let next = records
            .capacity()
            .max(1)
            .checked_mul(2)
            .ok_or(ResourceBudgetError::Overflow)?;
        charge.try_grow(ResourceAmounts {
            memory_bytes: next
                .checked_mul(std::mem::size_of::<SourceInventoryRecord>())
                .ok_or(ResourceBudgetError::Overflow)? as u64,
            ..ResourceAmounts::default()
        })?;
        records.reserve_exact(next - records.len());
    }
    Ok(())
}

/// Bound every current implementation Merkle path copy and directory node before building it.
pub(crate) fn merkle_memory_bound(
    records: &[SourceInventoryRecord],
) -> Result<u64, ResourceBudgetError> {
    records.iter().try_fold(512_u64, |sum, record| {
        let path = &record.path.raw_relative_path_bytes;
        let depth = path.iter().filter(|byte| **byte == b'/').count() as u64 + 1;
        // Each ancestor can occupy map key, traversal list, and one parent/child pair.
        let entry = (path.len() as u64 + 512)
            .checked_mul(depth)
            .and_then(|n| n.checked_mul(8))
            .ok_or(ResourceBudgetError::Overflow)?;
        sum.checked_add(entry).ok_or(ResourceBudgetError::Overflow)
    })
}

pub(crate) fn charged_workspace_path(
    root: &SecureRoot,
    path: &PlatformPath,
    budget: &ResourceBudget,
) -> Result<ChargedValue<WorkspacePath>, InventoryError> {
    let raw = path.raw_relative_path_bytes();
    // Percent encoding expands a byte at most 3x. Unicode decomposition/case folding may
    // expand a scalar; 18 decomposed scalars * 4 UTF-8 bytes, plus all coexisting path
    // views and vector growth, fit this conservative per-input-byte construction bound.
    let bound = (raw.len() as u64)
        .checked_mul(512)
        .and_then(|n| n.checked_add(2048))
        .ok_or(ResourceBudgetError::Overflow)?;
    let mut reservation = reserve_memory(budget, bound)?;
    let value = root.workspace_path(path)?;
    let retained = std::mem::size_of::<WorkspacePath>() as u64
        + value.raw_relative_path_bytes.capacity() as u64
        + value.canonical_component_bytes.capacity() as u64
        + value.comparison_key_bytes.capacity() as u64
        + value.display_string.capacity() as u64;
    if retained > bound {
        return Err(ResourceBudgetError::UnchargedAllocation {
            required: retained,
            reserved: bound,
        }
        .into());
    }
    reservation.shrink(ResourceAmounts {
        memory_bytes: bound - retained,
        ..ResourceAmounts::default()
    })?;
    Ok(reservation.into_charged_value(value))
}

pub(crate) fn persist_inventory(
    store: &mut OperationalStore,
    workspace_id: [u8; 16],
    source_generation: u64,
    records: &[SourceInventoryRecord],
) -> Result<(), InventoryError> {
    let inventory_digest = merkle_inventory_digest(records);
    let source_generation = i64::try_from(source_generation)
        .map_err(|_| InventoryError::BoundExceeded("source-generation"))?;
    store.write_transaction(|transaction| {
        let current = transaction.query_row(
            "SELECT source_generation FROM workspace_generation WHERE workspace_id=?1",
            [workspace_id.as_slice()],
            |row| row.get::<_, i64>(0),
        )?;
        if current != source_generation {
            return Err(InventoryError::SourceChanged);
        }
        transaction.execute(
            "DELETE FROM source_inventory WHERE workspace_id=?1 AND source_generation=?2",
            params![workspace_id.as_slice(), source_generation],
        )?;
        let mut statement = transaction.prepare(
            "INSERT INTO source_inventory(workspace_id, source_generation, path_bytes,
             path_display, comparison_key_bytes, file_id, content_digest, byte_length,
             file_kind_code, language_code, inventory_classification_code,
             inclusion_state_code, git_repo_path_bytes, git_blob_oid, current_file_owner)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        )?;
        for record in records {
            statement.execute(params![
                workspace_id.as_slice(),
                source_generation,
                &record.path.raw_relative_path_bytes,
                &record.path.display_string,
                &record.path.comparison_key_bytes,
                record.file_id.as_ref().map(<[u8; 16]>::as_slice),
                record.content_digest.as_ref().map(<[u8; 32]>::as_slice),
                i64::try_from(record.byte_length)
                    .map_err(|_| InventoryError::BoundExceeded("file-size"))?,
                record.file_kind as u16,
                record.language,
                record.classification as u16,
                record.inclusion as u16,
                record.git_repo_path_bytes.as_deref(),
                record.git_blob_oid.as_deref(),
                record.current_file_owner.as_ref().map(<[u8; 16]>::as_slice),
            ])?;
        }
        let changed = transaction.execute(
            "UPDATE worktree_state SET inventory_digest=?1 WHERE workspace_id=?2 AND source_generation=?3",
            params![inventory_digest.as_slice(), workspace_id.as_slice(), source_generation],
        )?;
        if changed != 1 {
            return Err(InventoryError::SourceChanged);
        }
        Ok(())
    })
}

/// Copy the prior coherent inventory, apply current-byte replacements/removals, and publish the
/// new Merkle digest in one transaction after the workspace generation has advanced.
///
/// # Errors
///
/// Returns an error when generations drift, an inventory record is invalid, a numeric bound is
/// exceeded, or the atomic operational-store transaction fails.
pub fn advance_inventory_generation(
    store: &mut OperationalStore,
    workspace_id: [u8; 16],
    prior_generation: u64,
    source_generation: u64,
    upserts: &[InventoryFileUpsert],
    removals: &BTreeSet<Vec<u8>>,
) -> Result<[u8; 32], InventoryError> {
    let prior_generation = i64::try_from(prior_generation)
        .map_err(|_| InventoryError::BoundExceeded("source-generation"))?;
    let source_generation = i64::try_from(source_generation)
        .map_err(|_| InventoryError::BoundExceeded("source-generation"))?;
    store.write_transaction(|transaction| {
        let current = transaction.query_row(
            "SELECT source_generation FROM workspace_generation WHERE workspace_id=?1",
            [workspace_id.as_slice()],
            |row| row.get::<_, i64>(0),
        )?;
        if current != source_generation {
            return Err(InventoryError::SourceChanged);
        }
        transaction.execute(
            "DELETE FROM source_inventory WHERE workspace_id=?1 AND source_generation=?2",
            params![workspace_id.as_slice(), source_generation],
        )?;
        transaction.execute(
            "INSERT INTO source_inventory(
               workspace_id,source_generation,path_bytes,path_display,comparison_key_bytes,file_id,
               content_digest,byte_length,file_kind_code,language_code,
               inventory_classification_code,inclusion_state_code,git_repo_path_bytes,git_blob_oid,
               current_file_owner
             )
             SELECT workspace_id,?1,path_bytes,path_display,comparison_key_bytes,file_id,
               content_digest,byte_length,file_kind_code,language_code,
               inventory_classification_code,inclusion_state_code,git_repo_path_bytes,git_blob_oid,
               current_file_owner
             FROM source_inventory WHERE workspace_id=?2 AND source_generation=?3",
            params![source_generation, workspace_id.as_slice(), prior_generation],
        )?;
        for path in removals {
            transaction.execute(
                "DELETE FROM source_inventory WHERE workspace_id=?1 AND source_generation=?2 AND path_bytes=?3",
                params![workspace_id.as_slice(), source_generation, path],
            )?;
        }
        for upsert in upserts {
            let prior = transaction
                .query_row(
                    "SELECT inventory_classification_code,git_repo_path_bytes,git_blob_oid FROM source_inventory WHERE workspace_id=?1 AND source_generation=?2 AND path_bytes=?3",
                    params![
                        workspace_id.as_slice(),
                        source_generation,
                        &upsert.path.raw_relative_path_bytes
                    ],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, Option<Vec<u8>>>(1)?,
                            row.get::<_, Option<Vec<u8>>>(2)?,
                        ))
                    },
                )
                .optional()?;
            let (classification, git_path, git_oid) = prior.unwrap_or((
                i64::from(InventoryClassification::UntrackedNotIgnored as u16),
                None,
                None,
            ));
            transaction.execute(
                "INSERT OR REPLACE INTO source_inventory(
                   workspace_id,source_generation,path_bytes,path_display,comparison_key_bytes,
                   file_id,content_digest,byte_length,file_kind_code,language_code,
                   inventory_classification_code,inclusion_state_code,git_repo_path_bytes,
                   git_blob_oid,current_file_owner
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
                params![
                    workspace_id.as_slice(),
                    source_generation,
                    &upsert.path.raw_relative_path_bytes,
                    &upsert.path.display_string,
                    &upsert.path.comparison_key_bytes,
                    upsert.file_id.as_slice(),
                    upsert.content_digest.as_slice(),
                    i64::try_from(upsert.byte_length)
                        .map_err(|_| InventoryError::BoundExceeded("file-size"))?,
                    i64::from(InventoryFileKind::Regular as u16),
                    upsert.language,
                    classification,
                    i64::from(InclusionState::Included as u16),
                    git_path,
                    git_oid,
                    upsert.file_id.as_slice(),
                ],
            )?;
        }
        let digest = persisted_inventory_digest(transaction, workspace_id, source_generation)?;
        let changed = transaction.execute(
            "UPDATE worktree_state SET inventory_digest=?1 WHERE workspace_id=?2 AND source_generation=?3",
            params![digest.as_slice(), workspace_id.as_slice(), source_generation],
        )?;
        if changed != 1 {
            return Err(InventoryError::SourceChanged);
        }
        Ok::<_, InventoryError>(digest)
    })
}

fn persisted_inventory_digest(
    connection: &rusqlite::Connection,
    workspace_id: [u8; 16],
    source_generation: i64,
) -> Result<[u8; 32], InventoryError> {
    let mut statement = connection.prepare(
        "SELECT path_bytes,content_digest,byte_length,file_kind_code,
           inventory_classification_code,inclusion_state_code
         FROM source_inventory WHERE workspace_id=?1 AND source_generation=?2 ORDER BY path_bytes",
    )?;
    let leaves = statement
        .query_map(params![workspace_id.as_slice(), source_generation], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, Option<Vec<u8>>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut encoded = Vec::with_capacity(leaves.len());
    for (path, digest, length, file_kind, classification, inclusion) in leaves {
        let digest = digest
            .map(|bytes| <[u8; 32]>::try_from(bytes).map_err(|_| InventoryError::SourceChanged))
            .transpose()?;
        let leaf = inventory_leaf_fields_digest(
            &path,
            digest,
            u64::try_from(length).map_err(|_| InventoryError::SourceChanged)?,
            u16::try_from(file_kind).map_err(|_| InventoryError::SourceChanged)?,
            u16::try_from(classification).map_err(|_| InventoryError::SourceChanged)?,
            u16::try_from(inclusion).map_err(|_| InventoryError::SourceChanged)?,
        );
        encoded.push((path, leaf));
    }
    Ok(merkle_from_leaves(encoded))
}

fn require_source_generation(
    store: &OperationalStore,
    workspace_id: [u8; 16],
    expected_generation: u64,
) -> Result<(), InventoryError> {
    let expected = i64::try_from(expected_generation)
        .map_err(|_| InventoryError::BoundExceeded("source-generation"))?;
    let current = store
        .reader_factory()
        .open()?
        .with_connection(|connection| {
            connection.query_row(
                "SELECT source_generation FROM workspace_generation WHERE workspace_id=?1",
                [workspace_id.as_slice()],
                |row| row.get::<_, i64>(0),
            )
        })?;
    if current != expected {
        return Err(InventoryError::SourceChanged);
    }
    Ok(())
}

pub(crate) fn merkle_inventory_digest(records: &[SourceInventoryRecord]) -> [u8; 32] {
    merkle_from_leaves(records.iter().map(|record| {
        (
            record.path.raw_relative_path_bytes.clone(),
            inventory_leaf_digest(record),
        )
    }))
}

fn merkle_from_leaves(leaves: impl IntoIterator<Item = (Vec<u8>, [u8; 32])>) -> [u8; 32] {
    let mut directories = BTreeMap::<Vec<u8>, Vec<(Vec<u8>, u8, [u8; 32])>>::new();
    let mut directory_paths = BTreeSet::from([Vec::new()]);
    for (path, digest) in leaves {
        let (parent, name) = split_parent(&path);
        let mut ancestor = parent.clone();
        // Every inserted directory already has all of its ancestors inserted. Stop at the
        // first shared ancestor instead of scanning every known directory for every leaf.
        while directory_paths.insert(ancestor.clone()) {
            ancestor = split_parent(&ancestor).0;
        }
        directories
            .entry(parent)
            .or_default()
            .push((name, 1, digest));
    }
    let mut directory_paths = directory_paths.into_iter().collect::<Vec<_>>();
    directory_paths.sort_by_key(|path| {
        std::cmp::Reverse(if path.is_empty() {
            0
        } else {
            path.split(|byte| *byte == b'/').count()
        })
    });
    for path in directory_paths {
        let mut children = directories.remove(&path).unwrap_or_default();
        children.sort_by(|left, right| left.0.cmp(&right.0));
        let digest = inventory_directory_digest(&children);
        if path.is_empty() {
            return digest;
        }
        let (parent, name) = split_parent(&path);
        directories
            .entry(parent)
            .or_default()
            .push((name, 2, digest));
    }
    inventory_directory_digest(&[])
}

fn inventory_leaf_digest(record: &SourceInventoryRecord) -> [u8; 32] {
    inventory_leaf_fields_digest(
        &record.path.raw_relative_path_bytes,
        record.content_digest,
        record.byte_length,
        record.file_kind as u16,
        record.classification as u16,
        record.inclusion as u16,
    )
}

fn inventory_leaf_fields_digest(
    path: &[u8],
    content_digest: Option<[u8; 32]>,
    byte_length: u64,
    file_kind: u16,
    classification: u16,
    inclusion: u16,
) -> [u8; 32] {
    let mut hasher = crate::integrity::IntegrityHasher::for_domain(
        crate::integrity::IntegrityDomain::InventoryFile,
    );
    hash_length_prefixed(&mut hasher, path);
    hasher.update(&content_digest.unwrap_or([0; 32]));
    hasher.update(&byte_length.to_be_bytes());
    hasher.update(&file_kind.to_be_bytes());
    hasher.update(&classification.to_be_bytes());
    hasher.update(&inclusion.to_be_bytes());
    hasher.finalize()
}

fn inventory_directory_digest(children: &[(Vec<u8>, u8, [u8; 32])]) -> [u8; 32] {
    let mut hasher = crate::integrity::IntegrityHasher::for_domain(
        crate::integrity::IntegrityDomain::InventoryDirectory,
    );
    for (name, kind, digest) in children {
        hash_length_prefixed(&mut hasher, name);
        hasher.update(&[*kind]);
        hasher.update(digest);
    }
    hasher.finalize()
}

fn hash_length_prefixed(hasher: &mut crate::integrity::IntegrityHasher, bytes: &[u8]) {
    hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(bytes);
}

fn split_parent(path: &[u8]) -> (Vec<u8>, Vec<u8>) {
    path.iter().rposition(|byte| *byte == b'/').map_or_else(
        || (Vec::new(), path.to_vec()),
        |index| (path[..index].to_vec(), path[index + 1..].to_vec()),
    )
}

fn filesystem_identity(device: u64, inode: u64) -> [u8; 16] {
    let mut identity = [0_u8; 16];
    identity[..8].copy_from_slice(&device.to_be_bytes());
    identity[8..].copy_from_slice(&inode.to_be_bytes());
    identity
}

fn join_components(components: &[Vec<u8>]) -> Vec<u8> {
    components.join(&b'/')
}

fn classify_language(path: &[u8]) -> Option<&'static str> {
    if path.ends_with(b".rs") {
        Some("rust")
    } else if path.ends_with(b".py") || path.ends_with(b".pyi") {
        Some("python")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::symlink;

    use super::*;
    use crate::identity::PlatformCode;
    use crate::secure_path::open_workspace_root;
    use crate::workspace_registry::{WorkspaceRegistry, WorkspaceSourceRegistration};

    #[test]
    fn large_inventory_merkle_preserves_shared_ancestors_and_raw_path_order() {
        let mut leaves = (0..16_384_u64)
            .map(|index| {
                (
                    format!("packages/p{:04}/src/deep/m{:02}.rs", index / 8, index % 8)
                        .into_bytes(),
                    crate::integrity::digest_bytes(&index.to_be_bytes()),
                )
            })
            .collect::<Vec<_>>();
        leaves.push((b"README.md".to_vec(), [1; 32]));
        leaves.push((b"packages/\xff/src/a.py".to_vec(), [2; 32]));
        let started = Instant::now();
        let digest = merkle_from_leaves(leaves.clone());
        // Frozen from the pre-optimization implementation at 20230937. The selected source
        // identity must remain compatible across process restart and ordering differences.
        assert_eq!(
            digest,
            [
                231, 215, 28, 199, 98, 48, 213, 58, 146, 198, 59, 58, 186, 73, 92, 96, 205, 22,
                229, 158, 97, 25, 77, 232, 163, 201, 18, 12, 175, 91, 27, 173,
            ]
        );
        eprintln!(
            "inventory Merkle: leaves={} elapsed={:?} digest={digest:?}",
            leaves.len(),
            started.elapsed()
        );
        leaves.reverse();
        assert_eq!(merkle_from_leaves(leaves), digest);
    }

    #[test]
    fn rt_cpg_wp79_metadata_exhaustion_and_foreign_owner_never_close_inventory() {
        let (_directory, mut store, workspace, root) = fixture();
        for index in 0..100 {
            fs::write(root.join(format!("{index:03}_{}.py", "x".repeat(180))), b"").unwrap();
        }
        let secure = open_workspace_root(&mut store, workspace).unwrap();
        let owner = crate::provider_types::source_fixture_budget(workspace);
        let mut policy = owner.policy();
        policy.limits.memory_bytes = 64 * 1024;
        let operation = owner.operation([7; 16], policy).unwrap();
        let result = InventoryWalker::new_governed(InventoryLimits::default(), operation.clone())
            .walk_selected_with_fence(&secure, &mut store, 0, 0, &Cancellation::default(), || {
                Some(0)
            });
        assert!(matches!(
            result,
            Err(InventoryError::Resource(_))
                | Err(InventoryError::SecurePath(
                    SecurePathError::ResourceExhausted
                ))
                | Err(InventoryError::StableRead(StableReadError::Secure(
                    SecurePathError::ResourceExhausted
                )))
        ));
        assert_eq!(operation.observation().used.memory_bytes, 0);
        assert!(operation.observation().peak.memory_bytes <= 64 * 1024);
        let foreign = crate::provider_types::source_fixture_budget([3; 16]);
        assert!(matches!(
            InventoryWalker::new_governed(InventoryLimits::default(), foreign)
                .walk_selected_with_fence(
                    &secure,
                    &mut store,
                    0,
                    0,
                    &Cancellation::default(),
                    || Some(0)
                ),
            Err(InventoryError::Resource(ResourceBudgetError::ForeignOwner))
        ));
    }

    fn fixture() -> (
        tempfile::TempDir,
        OperationalStore,
        [u8; 16],
        std::path::PathBuf,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("workspace");
        fs::create_dir(&root).unwrap();
        let mut store = OperationalStore::open(&directory.path().join("state.sqlite3")).unwrap();
        let workspace_id = WorkspaceRegistry::new(&mut store)
            .add(&root, WorkspaceSourceRegistration::Directory)
            .unwrap()
            .workspace_id;
        (directory, store, workspace_id, root)
    }

    #[test]
    #[cfg(feature = "daemon")]
    fn nested_linked_repository_classifies_only_its_selected_source_paths() {
        let (directory, mut store, workspace, path) = fixture();
        let nested = path.join("nested");
        let main = directory.path().join("external-main");
        crate::git_state::watch_topology::linked_fixture(&nested, &main);
        fs::write(main.join(".git/info/exclude"), "*.rs\n").unwrap();
        fs::write(path.join("outer.rs"), "fn outer() {}\n").unwrap();
        fs::write(nested.join("current.rs"), "fn nested() {}\n").unwrap();
        let root = open_workspace_root(&mut store, workspace).unwrap();
        let captured = InventoryWalker::new(InventoryLimits::default())
            .walk_and_persist(&root, &mut store, 0, &Cancellation::default())
            .unwrap();
        let context = captured.git_context.as_ref().unwrap();
        assert_eq!(context.paths.len(), 1);
        assert_eq!(context.paths[0].path, b"nested/current.rs");
        assert_eq!(context.paths[0].repository_root, b"nested");
        assert_eq!(
            context.paths[0].classification,
            Some(InventoryClassification::UntrackedIgnored as u16)
        );
        let member = captured
            .records
            .iter()
            .find(|record| record.path.raw_relative_path_bytes == b"nested/current.rs")
            .unwrap();
        assert_eq!(
            member.git_repo_path_bytes.as_deref(),
            Some(b"current.rs".as_slice())
        );
        assert!(
            captured
                .records
                .iter()
                .all(|record| record.content_digest.is_some())
        );
    }

    #[test]
    #[cfg(feature = "daemon")]
    fn captured_git_context_retains_all_conflict_stages_attributes_and_authoritative_bytes() {
        use gix::bstr::ByteSlice as _;
        let (_directory, mut store, workspace, path) = fixture();
        let repository = gix::init(&path).unwrap();
        fs::write(path.join(".gitignore"), "*.rs\n").unwrap();
        fs::write(
            path.join(".gitattributes"),
            "*.rs text eol=crlf filter=blocked\n",
        )
        .unwrap();
        fs::write(path.join("current.rs"), "fn captured() {}\r\n").unwrap();
        fs::write(path.join("untracked.rs"), "fn untracked() {}\n").unwrap();
        symlink(path.join("current.rs"), path.join("link.rs")).unwrap();
        let mut index = gix::index::State::new(gix::hash::Kind::Sha1);
        for (stage, hex) in [
            (gix::index::entry::Stage::Base, b'1'),
            (gix::index::entry::Stage::Ours, b'2'),
            (gix::index::entry::Stage::Theirs, b'3'),
        ] {
            index.dangerously_push_entry(
                Default::default(),
                gix::hash::ObjectId::from_hex(&[hex; 40]).unwrap(),
                gix::index::entry::Flags::from_stage(stage),
                gix::index::entry::Mode::FILE,
                b"current.rs".as_bstr(),
            );
        }
        index.sort_entries();
        gix::index::File::from_state(index, repository.index_path())
            .write(Default::default())
            .unwrap();
        let root = open_workspace_root(&mut store, workspace).unwrap();
        let mut walker = InventoryWalker::new(InventoryLimits::default());
        let captured = walker
            .walk_and_persist(&root, &mut store, 0, &Cancellation::default())
            .unwrap();
        let context = captured.git_context.as_ref().unwrap();
        assert_eq!(
            context
                .paths
                .iter()
                .find(|row| row.path == b"link.rs")
                .unwrap()
                .classification,
            Some(InventoryClassification::SpecialFile as u16)
        );
        let current = context
            .paths
            .iter()
            .find(|row| row.path == b"current.rs")
            .unwrap();
        assert_eq!(current.status, "observed");
        assert_eq!(
            current.classification,
            Some(InventoryClassification::TrackedButIgnoredPatternMatches as u16)
        );
        assert_eq!(
            current
                .stages
                .iter()
                .map(|stage| stage.stage)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(
            current
                .attributes
                .iter()
                .find(|attribute| attribute.name == "eol")
                .unwrap()
                .value
                .as_deref(),
            Some(b"crlf".as_slice())
        );
        assert_eq!(
            current
                .attributes
                .iter()
                .find(|attribute| attribute.name == "filter")
                .unwrap()
                .value
                .as_deref(),
            Some(b"blocked".as_slice())
        );
        let source = captured
            .records
            .iter()
            .find(|record| record.path.raw_relative_path_bytes == b"current.rs")
            .unwrap();
        assert_eq!(
            source.content_digest,
            Some(crate::integrity::digest_bytes(b"fn captured() {}\r\n"))
        );
        assert!(
            source.git_blob_oid.is_none(),
            "a conflicted index has no single authoritative blob"
        );
        assert_eq!(
            context
                .paths
                .iter()
                .find(|row| row.path == b"untracked.rs")
                .unwrap()
                .classification,
            Some(InventoryClassification::UntrackedIgnored as u16)
        );
        fs::write(path.join(".git/info/attributes"), "current.rs eol=lf\n").unwrap();
        let changed = walker
            .walk_and_persist(&root, &mut store, 0, &Cancellation::default())
            .unwrap();
        assert_eq!(
            captured.digest, changed.digest,
            "source bytes and classifications are unchanged"
        );
        assert_ne!(
            captured.git_context_digest(),
            changed.git_context_digest(),
            "metadata changes must invalidate context selection"
        );
        assert!(
            changed
                .records
                .iter()
                .all(|record| !record.path.raw_relative_path_bytes.starts_with(b".git/"))
        );
    }

    #[test]
    #[cfg(feature = "daemon")]
    fn selected_submodule_boundaries_keep_conflicted_gitlinks_and_retract_pruned_inputs() {
        use gix::bstr::ByteSlice as _;
        let (_directory, mut store, workspace, path) = fixture();
        let repository = gix::init(&path).unwrap();
        fs::create_dir_all(path.join(".venv/vendor")).unwrap();
        gix::init(path.join(".venv/vendor")).unwrap();
        fs::create_dir_all(path.join(".venv/sibling")).unwrap();
        fs::write(path.join(".venv/vendor/selected.py"), "selected = 1\n").unwrap();
        fs::write(path.join(".venv/sibling/excluded.py"), "excluded = 1\n").unwrap();
        fs::write(path.join(".gitmodules"), "[submodule \"vendor\"]\npath = .venv/vendor\n[submodule \"missing\"]\npath = missing\n[submodule \"escape\"]\npath = ../outside\n").unwrap();
        let mut index = gix::index::State::new(gix::hash::Kind::Sha1);
        for (stage, hex) in [
            (gix::index::entry::Stage::Ours, b'2'),
            (gix::index::entry::Stage::Theirs, b'3'),
        ] {
            index.dangerously_push_entry(
                Default::default(),
                gix::hash::ObjectId::from_hex(&[hex; 40]).unwrap(),
                gix::index::entry::Flags::from_stage(stage),
                gix::index::entry::Mode::COMMIT,
                b".venv/vendor".as_bstr(),
            );
        }
        index.sort_entries();
        gix::index::File::from_state(index, repository.index_path())
            .write(Default::default())
            .unwrap();
        let root = open_workspace_root(&mut store, workspace).unwrap();
        let mut walker = InventoryWalker::new(InventoryLimits::default());
        let first = walker
            .walk_and_persist(&root, &mut store, 0, &Cancellation::default())
            .unwrap();
        assert!(
            first
                .records
                .iter()
                .any(|record| record.path.raw_relative_path_bytes == b".venv/vendor/selected.py")
        );
        assert!(!first.records.iter().any(|record| {
            record
                .path
                .raw_relative_path_bytes
                .starts_with(b".venv/sibling")
        }));
        let boundaries = &first.git_context.as_ref().unwrap().submodules;
        let vendor = boundaries
            .iter()
            .find(|boundary| boundary.name.as_deref() == Some(b"vendor"))
            .unwrap();
        assert_eq!(vendor.path.as_deref(), Some(b".venv/vendor".as_slice()));
        assert!(vendor.captured_sources && vendor.repository_observed);
        assert_eq!(
            vendor
                .stages
                .iter()
                .map(|stage| stage.stage)
                .collect::<Vec<_>>(),
            [2, 3]
        );
        assert_eq!(vendor.stages[0].object_id, vec![0x22; 20]);
        assert_eq!(vendor.stages[1].object_id, vec![0x33; 20]);
        let missing = boundaries
            .iter()
            .find(|boundary| boundary.name.as_deref() == Some(b"missing"))
            .unwrap();
        assert!(!missing.captured_sources && !missing.repository_observed);
        let escape = boundaries
            .iter()
            .find(|boundary| boundary.name.as_deref() == Some(b"escape"))
            .unwrap();
        assert!(escape.path.is_none());
        assert_eq!(escape.configuration_status, "invalid_path");
        fs::remove_file(path.join(".gitmodules")).unwrap();
        let removed = walker
            .walk_and_persist(&root, &mut store, 0, &Cancellation::default())
            .unwrap();
        assert!(
            removed.records.is_empty(),
            "declaration removal retracts the pruned source subtree"
        );
        assert_ne!(first.git_context_digest(), removed.git_context_digest());
    }

    #[test]
    #[cfg(feature = "daemon")]
    fn recursive_configuration_captures_raw_nested_roots_and_retracts_descendants() {
        use std::os::unix::ffi::OsStrExt as _;
        let (_directory, mut store, workspace, path) = fixture();
        let nested = path.join(std::ffi::OsStr::from_bytes(b"nested-\xff"));
        let dependency = nested.join("target/dependency");
        let deep = dependency.join("node_modules/deep");
        fs::create_dir_all(&deep).unwrap();
        fs::create_dir_all(dependency.join("node_modules/unselected")).unwrap();
        gix::init(&nested).unwrap();
        gix::init(&dependency).unwrap();
        fs::write(
            nested.join(".gitmodules"),
            "[submodule \"dependency\"]\npath = target/dependency\n",
        )
        .unwrap();
        fs::write(
            dependency.join(".gitmodules"),
            "[submodule \"deep\"]\npath = node_modules/deep\n",
        )
        .unwrap();
        fs::write(dependency.join("local.py"), "local = 1\n").unwrap();
        fs::write(deep.join("selected.py"), "selected = 2\n").unwrap();
        fs::write(
            dependency.join("node_modules/unselected/ignored.py"),
            "ignored = 3\n",
        )
        .unwrap();
        let root = open_workspace_root(&mut store, workspace).unwrap();
        let mut walker = InventoryWalker::new(InventoryLimits::default());
        let first = walker
            .walk_and_persist(&root, &mut store, 0, &Cancellation::default())
            .unwrap();
        let paths = first
            .records
            .iter()
            .map(|record| record.path.raw_relative_path_bytes.as_slice())
            .collect::<BTreeSet<_>>();
        assert!(paths.contains(b"nested-\xff/target/dependency/local.py".as_slice()));
        assert!(
            paths.contains(
                b"nested-\xff/target/dependency/node_modules/deep/selected.py".as_slice()
            )
        );
        assert!(!paths.contains(
            b"nested-\xff/target/dependency/node_modules/unselected/ignored.py".as_slice()
        ));
        let boundaries = &first.git_context.as_ref().unwrap().submodules;
        assert_eq!(boundaries.len(), 2);
        let deep_boundary = boundaries
            .iter()
            .find(|boundary| boundary.name.as_deref() == Some(b"deep"))
            .unwrap();
        assert_eq!(
            deep_boundary.path.as_deref(),
            Some(b"nested-\xff/target/dependency/node_modules/deep".as_slice())
        );
        assert!(deep_boundary.captured_sources);
        assert!(!deep_boundary.repository_observed);
        fs::remove_file(nested.join(".gitmodules")).unwrap();
        let after = walker
            .walk_and_persist(&root, &mut store, 0, &Cancellation::default())
            .unwrap();
        assert!(
            after.records.is_empty(),
            "removing a parent declaration withdraws the entire pruned chain"
        );
        assert_ne!(first.digest, after.digest);
    }

    #[test]
    #[cfg(feature = "daemon")]
    fn captured_configuration_selects_pruned_dependencies_and_removal_retracts_them() {
        let (_directory, mut store, workspace, path) = fixture();
        fs::create_dir_all(path.join(".venv/lib/site-packages/pkg")).unwrap();
        fs::create_dir_all(path.join(".venv/bin")).unwrap();
        fs::write(path.join(".venv/bin/unselected.py"), "unselected = True\n").unwrap();
        fs::write(
            path.join(".venv/lib/site-packages/pkg/__init__.pyi"),
            "def external() -> int: ...\n",
        )
        .unwrap();
        fs::write(
            path.join("pyrefly.toml"),
            "site-package-path=['.venv/lib/site-packages']\n",
        )
        .unwrap();
        let root = open_workspace_root(&mut store, workspace).unwrap();
        let mut walker = InventoryWalker::new(InventoryLimits::default());
        let first = walker
            .walk_and_persist(&root, &mut store, 0, &Cancellation::default())
            .unwrap();
        let paths = first
            .records
            .iter()
            .map(|record| record.path.raw_relative_path_bytes.as_slice())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            [
                b".venv/lib/site-packages/pkg/__init__.pyi".as_slice(),
                b"pyrefly.toml"
            ]
        );
        fs::write(path.join("pyrefly.toml"), "site-package-path=[]\n").unwrap();
        let second = walker
            .walk_and_persist(&root, &mut store, 0, &Cancellation::default())
            .unwrap();
        assert_eq!(second.records.len(), 1);
        assert_ne!(first.digest, second.digest);
    }

    #[test]
    fn wp16_inventory_behavioral_acceptance() {
        let (_directory, mut store, workspace_id, root_path) = fixture();
        fs::create_dir(root_path.join("src")).unwrap();
        fs::create_dir(root_path.join("target")).unwrap();
        fs::create_dir(root_path.join(".git")).unwrap();
        fs::write(root_path.join("src/lib.rs"), b"fn one() {}\n").unwrap();
        fs::write(root_path.join("README.md"), b"read me\n").unwrap();
        fs::write(root_path.join("target/ignored.rs"), b"ignored\n").unwrap();
        fs::write(root_path.join(".git/config"), b"secret\n").unwrap();
        let root = open_workspace_root(&mut store, workspace_id).unwrap();
        let mut walker = InventoryWalker::new(InventoryLimits::default());
        let first = walker
            .walk_and_persist(&root, &mut store, 0, &Cancellation::default())
            .unwrap();
        assert_eq!(first.records.len(), 2);
        assert!(
            first
                .records
                .iter()
                .all(|record| record.content_digest.is_some())
        );
        assert_eq!(
            first
                .records
                .iter()
                .find(|record| record.path.raw_relative_path_bytes == b"src/lib.rs")
                .unwrap()
                .language,
            Some("rust")
        );
        assert!(first.records.iter().all(|record| {
            !record.path.raw_relative_path_bytes.starts_with(b".git/")
                && !record.path.raw_relative_path_bytes.starts_with(b"target/")
        }));

        fs::write(root_path.join("src/lib.rs"), b"fn two() {}\n").unwrap();
        let second = walker
            .walk_and_persist(&root, &mut store, 0, &Cancellation::default())
            .unwrap();
        assert_ne!(first.digest, second.digest);
        assert_ne!(
            first.records[1].content_digest,
            second.records[1].content_digest
        );

        fs::rename(root_path.join("README.md"), root_path.join("RENAMED.md")).unwrap();
        let renamed = walker
            .walk_and_persist(&root, &mut store, 0, &Cancellation::default())
            .unwrap();
        let old = second
            .records
            .iter()
            .find(|record| record.path.raw_relative_path_bytes == b"README.md")
            .unwrap();
        let new = renamed
            .records
            .iter()
            .find(|record| record.path.raw_relative_path_bytes == b"RENAMED.md")
            .unwrap();
        assert_eq!(old.content_digest, new.content_digest);
        assert_eq!(old.filesystem_identity, new.filesystem_identity);
        assert_ne!(old.file_id, new.file_id, "rename evidence is not identity");

        assert!(matches!(
            InventoryWalker::new(InventoryLimits::default()).walk_and_persist(
                &root,
                &mut store,
                1,
                &Cancellation::default()
            ),
            Err(InventoryError::SourceChanged)
        ));
    }

    #[test]
    fn wp16_inventory_structural_acceptance() {
        let (_directory, mut store, workspace_id, root_path) = fixture();
        fs::write(root_path.join("unit.py"), b"value = 1\n").unwrap();
        let root = open_workspace_root(&mut store, workspace_id).unwrap();
        let mut walker = InventoryWalker::new(InventoryLimits::default());
        let inventory = walker
            .walk_and_persist(&root, &mut store, 0, &Cancellation::default())
            .unwrap();
        assert_eq!(inventory.records.len(), 1);
        let persisted = store
            .reader_factory()
            .open()
            .unwrap()
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT path_bytes, path_display, comparison_key_bytes, file_id,
                            content_digest, byte_length, file_kind_code, language_code,
                            inventory_classification_code, inclusion_state_code,
                            git_repo_path_bytes, git_blob_oid, current_file_owner
                     FROM source_inventory WHERE workspace_id=?1 AND source_generation=0",
                    [workspace_id.as_slice()],
                    |row| {
                        Ok((
                            row.get::<_, Vec<u8>>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Vec<u8>>(2)?,
                            row.get::<_, Vec<u8>>(3)?,
                            row.get::<_, Vec<u8>>(4)?,
                            row.get::<_, i64>(5)?,
                            row.get::<_, u16>(6)?,
                            row.get::<_, String>(7)?,
                            row.get::<_, u16>(8)?,
                            row.get::<_, u16>(9)?,
                            row.get::<_, Option<Vec<u8>>>(10)?,
                            row.get::<_, Option<Vec<u8>>>(11)?,
                            row.get::<_, Option<Vec<u8>>>(12)?,
                        ))
                    },
                )
            })
            .unwrap();
        assert_eq!(persisted.0, b"unit.py");
        assert_eq!(persisted.1, "unit.py");
        assert!(!persisted.2.is_empty());
        assert_eq!(persisted.3.len(), 16);
        assert_eq!(persisted.4.len(), 32);
        assert_eq!(persisted.5, 10);
        assert_eq!(persisted.6, InventoryFileKind::Regular as u16);
        assert_eq!(persisted.7, "python");
        assert_eq!(
            persisted.8,
            InventoryClassification::UntrackedNotIgnored as u16
        );
        assert_eq!(persisted.9, InclusionState::Included as u16);
        assert_eq!(
            (persisted.10, persisted.11, persisted.12),
            (None, None, None)
        );
    }

    #[test]
    fn wp16_inventory_negative_zero_state() {
        let (directory, mut store, workspace_id, root_path) = fixture();
        fs::create_dir(root_path.join("deep")).unwrap();
        fs::write(root_path.join("one.rs"), b"1").unwrap();
        fs::write(root_path.join("two.py"), b"22").unwrap();
        fs::write(root_path.join("deep/three.rs"), b"333").unwrap();
        let outside = directory.path().join("outside.rs");
        fs::write(&outside, b"outside").unwrap();
        symlink(&outside, root_path.join("escape.rs")).unwrap();
        let oversized = fs::File::create(root_path.join("oversized.rs")).unwrap();
        oversized
            .set_len(ORDINARY_SOURCE_MAXIMUM_BYTES + 1)
            .unwrap();
        drop(oversized);
        let root = open_workspace_root(&mut store, workspace_id).unwrap();

        let cancelled = Cancellation::default();
        cancelled.cancel();
        assert!(matches!(
            InventoryWalker::new(InventoryLimits::default())
                .walk_and_persist(&root, &mut store, 0, &cancelled),
            Err(InventoryError::Cancelled)
        ));

        let cases = [
            InventoryLimits {
                maximum_file_count: 0,
                ..InventoryLimits::default()
            },
            InventoryLimits {
                maximum_directory_count: 0,
                ..InventoryLimits::default()
            },
            InventoryLimits {
                maximum_directory_depth: 0,
                ..InventoryLimits::default()
            },
            InventoryLimits {
                maximum_total_bytes_considered: 0,
                ..InventoryLimits::default()
            },
            InventoryLimits {
                maximum_duration: Duration::ZERO,
                ..InventoryLimits::default()
            },
            InventoryLimits {
                maximum_entries_per_directory: 0,
                ..InventoryLimits::default()
            },
        ];
        for limits in cases {
            assert!(
                InventoryWalker::new(limits)
                    .walk_and_persist(&root, &mut store, 0, &Cancellation::default())
                    .is_err()
            );
        }

        let inventory = InventoryWalker::new(InventoryLimits::default())
            .walk_and_persist(&root, &mut store, 0, &Cancellation::default())
            .unwrap();
        let symlink_record = inventory
            .records
            .iter()
            .find(|record| record.path.raw_relative_path_bytes == b"escape.rs")
            .unwrap();
        assert_eq!(
            symlink_record.inclusion,
            InclusionState::ExcludedSpecialFile
        );
        let oversized_record = inventory
            .records
            .iter()
            .find(|record| record.path.raw_relative_path_bytes == b"oversized.rs")
            .unwrap();
        assert_eq!(
            oversized_record.inclusion,
            InclusionState::ExcludedSizeLimit
        );
        assert_eq!(oversized_record.content_digest, None);
    }

    #[test]
    fn wp16_inventory_operational_acceptance() {
        let (_directory, mut store, workspace_id, root_path) = fixture();
        fs::create_dir(root_path.join("src")).unwrap();
        fs::write(root_path.join("src/lib.rs"), b"abc").unwrap();
        let root = open_workspace_root(&mut store, workspace_id).unwrap();
        let mut walker = InventoryWalker::new(InventoryLimits::default());
        walker
            .walk_and_persist(&root, &mut store, 0, &Cancellation::default())
            .unwrap();
        let metrics = walker.metrics();
        assert_eq!(metrics.files, 1);
        assert_eq!(metrics.directories, 2);
        assert_eq!(metrics.bytes_considered, 3);
        assert_eq!(metrics.excluded_files, 0);
        assert!(metrics.duration_micros > 0);
    }

    #[test]
    fn wp16_inventory_platform_path_is_byte_native() {
        let platform = if cfg!(target_os = "macos") {
            PlatformCode::MacOs
        } else {
            PlatformCode::Unix
        };
        let path = PlatformPath::from_raw_relative_bytes(platform, b"src/lib.rs".to_vec()).unwrap();
        assert_eq!(path.raw_relative_path_bytes(), b"src/lib.rs");
    }
}
