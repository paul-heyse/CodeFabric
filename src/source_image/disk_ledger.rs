//! Workspace-owned accounting for the actual private source-blob directory, not lease rows.

use std::collections::BTreeMap;
use std::os::unix::ffi::OsStrExt as _;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags, fstat, fstatvfs, openat, statat};
use rustix::io::Errno;

use super::{BlobReference, BlobStore, SourceImageError, digest_name, random_registration_nonce};
use crate::cancellation::Cancellation;
use crate::disk_headroom::LocalDiskHeadroom;
use crate::resource_budget::{
    ResourceAmounts, ResourceBudget, ResourceBudgetError, ResourceClass, ResourceReservation,
    ResourceScopeKind,
};

/// One mandatory workspace owner. Clones share a census, reservations, and write/GC exclusion.
#[derive(Clone)]
pub struct SourceBlobDiskLedger {
    inner: Arc<LedgerOwner>,
}

struct LedgerOwner {
    budget: ResourceBudget,
    headroom: LocalDiskHeadroom,
    state: Mutex<LedgerState>,
    _metadata: ResourceReservation,
}

#[derive(Default)]
struct LedgerState {
    root: Option<RootBinding>,
    entries: BTreeMap<Vec<u8>, DiskEntry>,
    ready: bool,
}

struct RootBinding {
    path: PathBuf,
    identity: (u64, u64),
    allocation_unit: u64,
    _metadata: ResourceReservation,
}

struct DiskEntry {
    seen: bool,
    charge: ResourceReservation,
}

/// Disk and metadata observations include unregistered and abandoned temporary files.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceBlobDiskObservation {
    pub ready: bool,
    pub files: usize,
    pub disk_bytes: u64,
}

impl SourceBlobDiskLedger {
    /// Construct before any capture work, using the existing workspace envelope.
    /// No filesystem IO or globally discovered budget occurs here.
    ///
    /// # Errors
    /// Rejects non-workspace ownership or unavailable ledger metadata capacity.
    pub(crate) fn try_new(
        budget: ResourceBudget,
        headroom: LocalDiskHeadroom,
    ) -> Result<Self, SourceImageError> {
        if budget.owner().kind != ResourceScopeKind::Workspace {
            return Err(ResourceBudgetError::ForeignOwner.into());
        }
        let metadata = super::reserve_memory(&budget, std::mem::size_of::<LedgerOwner>() as u64)?;
        Ok(Self {
            inner: Arc::new(LedgerOwner {
                budget,
                headroom,
                state: Mutex::new(LedgerState::default()),
                _metadata: metadata,
            }),
        })
    }

    pub(super) fn validate_owner(&self, budget: &ResourceBudget) -> Result<(), SourceImageError> {
        if !budget.is_descendant_of(&self.inner.budget) {
            return Err(ResourceBudgetError::ForeignOwner.into());
        }
        Ok(())
    }

    /// Complete descriptor-relative census before opening a new capture store. Failure
    /// leaves the ledger unavailable and retains every already observed allocation.
    pub(super) fn reconcile(
        &self,
        blobs: &BlobStore,
        cancellation: &Cancellation,
    ) -> Result<(), SourceImageError> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| SourceImageError::BlobIo)?;
        state.ready = false;
        if cancellation.is_cancelled() {
            return Err(crate::inventory::InventoryError::Cancelled.into());
        }
        self.inner
            .headroom
            .validate_directory(&blobs.root)
            .map_err(|_| SourceImageError::PhysicalDisk)?;
        let stat = fstat(&blobs.descriptor).map_err(|_| SourceImageError::BlobIo)?;
        let identity = (stat.st_dev, stat.st_ino);
        if let Some(binding) = &state.root {
            if binding.path != blobs.root || binding.identity != identity {
                return Err(SourceImageError::BlobIo);
            }
        } else {
            let fs = fstatvfs(&blobs.descriptor).map_err(|_| SourceImageError::BlobIo)?;
            let allocation_unit = if fs.f_frsize == 0 {
                fs.f_bsize
            } else {
                fs.f_frsize
            };
            if allocation_unit == 0 {
                return Err(SourceImageError::BlobIo);
            }
            let metadata = super::reserve_memory(
                &self.inner.budget,
                blobs.root.as_os_str().as_bytes().len() as u64
                    + std::mem::size_of::<RootBinding>() as u64,
            )?;
            state.root = Some(RootBinding {
                path: blobs.root.clone(),
                identity,
                allocation_unit,
                _metadata: metadata,
            });
        }
        for entry in state.entries.values_mut() {
            entry.seen = false;
        }
        let _iteration = super::reserve_memory(
            &self.inner.budget,
            crate::secure_path::DIRECTORY_ITERATION_MEMORY_BOUND as u64,
        )?;
        let descriptor = openat(
            &blobs.descriptor,
            ".",
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(|_| SourceImageError::BlobIo)?;
        let mut directory = Dir::new(descriptor).map_err(|_| SourceImageError::BlobIo)?;
        while let Some(entry) = directory.read() {
            if cancellation.is_cancelled() {
                return Err(crate::inventory::InventoryError::Cancelled.into());
            }
            let entry = entry.map_err(|_| SourceImageError::BlobIo)?;
            let name = entry.file_name().to_bytes();
            if matches!(name, b"." | b"..") {
                continue;
            }
            let bytes = observed_allocation(blobs, name)?.ok_or(SourceImageError::BlobIo)?;
            admit_entry(&self.inner.budget, &mut state, name, bytes)?;
        }
        let after = fstat(&blobs.descriptor).map_err(|_| SourceImageError::BlobIo)?;
        if (after.st_dev, after.st_ino) != identity
            || (
                after.st_mtime,
                after.st_mtime_nsec,
                after.st_ctime,
                after.st_ctime_nsec,
            ) != (
                stat.st_mtime,
                stat.st_mtime_nsec,
                stat.st_ctime,
                stat.st_ctime_nsec,
            )
        {
            return Err(SourceImageError::BlobIo);
        }
        if cancellation.is_cancelled() {
            return Err(crate::inventory::InventoryError::Cancelled.into());
        }
        // Absence is established only after the entire exact directory has been enumerated.
        state.entries.retain(|_, entry| entry.seen);
        state.ready = true;
        Ok(())
    }

    pub(super) fn put(
        &self,
        blobs: &BlobStore,
        bytes: &[u8],
        budget: &ResourceBudget,
    ) -> Result<BlobReference, SourceImageError> {
        self.validate_owner(budget)?;
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| SourceImageError::BlobIo)?;
        require_binding(&mut state, blobs)?;
        let digest = crate::integrity::digest_bytes(bytes);
        let name = digest_name(&digest);
        if let Some(allocated) = observed_allocation(blobs, name.as_bytes())? {
            admit_entry(&self.inner.budget, &mut state, name.as_bytes(), allocated)?;
            let _read = super::reserve_memory(budget, bytes.len() as u64)?;
            if blobs.read_named(&name, bytes.len())? != bytes {
                return Err(SourceImageError::BlobDigestMismatch);
            }
        } else {
            let unit = state
                .root
                .as_ref()
                .ok_or(SourceImageError::BlobIo)?
                .allocation_unit;
            let allocation = rounded_allocation(bytes.len() as u64, unit)?;
            self.inner
                .headroom
                .validate_directory(&blobs.root)
                .map_err(|_| SourceImageError::PhysicalDisk)?;
            let _growth = self
                .inner
                .headroom
                .try_reserve_growth(allocation, ResourceClass::Data)
                .map_err(|_| SourceImageError::PhysicalDisk)?;
            let temporary = format!(
                ".tmp-{}-{}",
                std::process::id(),
                u128::from_be_bytes(random_registration_nonce()?)
            );
            admit_entry(
                &self.inner.budget,
                &mut state,
                temporary.as_bytes(),
                allocation,
            )?;
            // Both publish and its cleanup have closed their descriptors before reconciliation.
            let result = blobs.publish(&name, bytes, &temporary);
            let target = observed_allocation(blobs, name.as_bytes());
            let temporary_allocation = observed_allocation(blobs, temporary.as_bytes());
            match (target, temporary_allocation) {
                (Ok(Some(allocated)), Ok(None)) => {
                    let entry = state
                        .entries
                        .remove(temporary.as_bytes())
                        .ok_or(SourceImageError::BlobIo)?;
                    state.entries.insert(name.as_bytes().to_vec(), entry);
                    if let Err(error) = resize_entry(
                        state
                            .entries
                            .get_mut(name.as_bytes())
                            .ok_or(SourceImageError::BlobIo)?,
                        allocated,
                    ) {
                        state.ready = false;
                        return Err(error);
                    }
                }
                (Ok(None), Ok(None)) => {
                    state.entries.remove(temporary.as_bytes());
                    if result.is_ok() {
                        state.ready = false;
                        return Err(SourceImageError::BlobIo);
                    }
                }
                (Ok(None), Ok(Some(allocated))) => {
                    admit_entry(
                        &self.inner.budget,
                        &mut state,
                        temporary.as_bytes(),
                        allocated,
                    )?;
                }
                _ => {
                    // Uncertain observation cannot release a possibly material file.
                    state.ready = false;
                    return Err(SourceImageError::BlobIo);
                }
            }
            result?;
        }
        Ok(BlobReference {
            digest,
            relative_name: name,
            byte_length: bytes.len() as u64,
        })
    }

    pub(super) fn remove(
        &self,
        blobs: &BlobStore,
        digest: &[u8; 32],
    ) -> Result<(), SourceImageError> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| SourceImageError::BlobIo)?;
        require_binding(&mut state, blobs)?;
        let name = digest_name(digest);
        let result = blobs.remove(digest);
        match observed_allocation(blobs, name.as_bytes()) {
            Ok(None) if result.is_ok() => {
                state.entries.remove(name.as_bytes());
            }
            Ok(Some(bytes)) => {
                admit_entry(&self.inner.budget, &mut state, name.as_bytes(), bytes)?;
            }
            _ => {
                state.ready = false;
            }
        }
        result
    }

    /// Queryable local operational evidence; this never selects source semantics.
    ///
    /// # Errors
    /// Returns an unavailable poisoned ownership domain.
    pub fn observation(&self) -> Result<SourceBlobDiskObservation, SourceImageError> {
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| SourceImageError::BlobIo)?;
        Ok(SourceBlobDiskObservation {
            ready: state.ready,
            files: state.entries.len(),
            disk_bytes: state
                .entries
                .values()
                .map(|entry| entry.charge.amounts().disk_bytes)
                .sum(),
        })
    }
}

fn rounded_allocation(bytes: u64, unit: u64) -> Result<u64, ResourceBudgetError> {
    bytes
        .checked_add(unit - 1)
        .map(|n| n / unit)
        .and_then(|n| n.checked_mul(unit))
        .ok_or(ResourceBudgetError::Overflow)
}

fn require_binding(state: &mut LedgerState, blobs: &BlobStore) -> Result<(), SourceImageError> {
    let ready = state.ready;
    state.ready = false;
    let stat = fstat(&blobs.descriptor).map_err(|_| SourceImageError::BlobIo)?;
    let current = crate::secure_path::open_absolute_directory_nofollow(&blobs.root)
        .map_err(|_| SourceImageError::BlobIo)?;
    let current = fstat(&current).map_err(|_| SourceImageError::BlobIo)?;
    if !ready
        || (current.st_dev, current.st_ino) != (stat.st_dev, stat.st_ino)
        || state.root.as_ref().is_none_or(|binding| {
            binding.path != blobs.root || binding.identity != (stat.st_dev, stat.st_ino)
        })
    {
        return Err(SourceImageError::BlobIo);
    }
    state.ready = true;
    Ok(())
}

fn observed_allocation(blobs: &BlobStore, name: &[u8]) -> Result<Option<u64>, SourceImageError> {
    match statat(
        &blobs.descriptor,
        std::ffi::OsStr::from_bytes(name),
        AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Ok(stat) => {
            if !FileType::from_raw_mode(stat.st_mode).is_file() {
                return Err(SourceImageError::BlobIo);
            }
            let logical = u64::try_from(stat.st_size).map_err(|_| SourceImageError::BlobIo)?;
            let blocks = u64::try_from(stat.st_blocks).map_err(|_| SourceImageError::BlobIo)?;
            let filesystem = fstatvfs(&blobs.descriptor).map_err(|_| SourceImageError::BlobIo)?;
            let unit = if filesystem.f_frsize == 0 {
                filesystem.f_bsize
            } else {
                filesystem.f_frsize
            };
            if unit == 0 {
                return Err(SourceImageError::BlobIo);
            }
            Ok(Some(
                rounded_allocation(logical, unit)?.max(
                    blocks
                        .checked_mul(512)
                        .ok_or(ResourceBudgetError::Overflow)?,
                ),
            ))
        }
        Err(Errno::NOENT) => Ok(None),
        Err(_) => Err(SourceImageError::BlobIo),
    }
}

fn admit_entry(
    budget: &ResourceBudget,
    state: &mut LedgerState,
    name: &[u8],
    disk_bytes: u64,
) -> Result<(), SourceImageError> {
    if let Some(entry) = state.entries.get_mut(name) {
        let result = resize_entry(entry, disk_bytes);
        if result.is_err() {
            state.ready = false;
        }
        return result;
    }
    // A single-entry BTree may allocate a complete node. Charge that worst case before
    // each insertion, including both names alive during temporary-to-final transfer.
    let metadata = 12
        * (std::mem::size_of::<DiskEntry>()
            + std::mem::size_of::<Vec<u8>>()
            + std::mem::size_of::<usize>())
        + 128
        // Both the temporary key and the canonical 64-byte digest key may coexist.
        + name.len().max(64) * 2;
    let charge = budget.try_reserve(
        ResourceClass::Data,
        ResourceAmounts {
            disk_bytes,
            memory_bytes: metadata as u64,
            pages: 1,
            ..ResourceAmounts::default()
        },
    )?;
    state
        .entries
        .insert(name.to_vec(), DiskEntry { seen: true, charge });
    Ok(())
}

fn resize_entry(entry: &mut DiskEntry, disk_bytes: u64) -> Result<(), SourceImageError> {
    let previous = entry.charge.amounts().disk_bytes;
    if disk_bytes > previous {
        entry.charge.try_grow(ResourceAmounts {
            disk_bytes: disk_bytes - previous,
            ..ResourceAmounts::default()
        })?;
    } else {
        entry.charge.shrink(ResourceAmounts {
            disk_bytes: previous - disk_bytes,
            ..ResourceAmounts::default()
        })?;
    }
    entry.seen = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn ledger(directory: &std::path::Path, budget: &ResourceBudget) -> SourceBlobDiskLedger {
        SourceBlobDiskLedger::try_new(budget.clone(), LocalDiskHeadroom::open(directory).unwrap())
            .unwrap()
    }

    #[test]
    fn rt_cpg_wp79_blob_disk_retains_deduplicates_reopens_and_releases_after_gc() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("blobs");
        let budget = crate::provider_types::source_fixture_budget([2; 16]);
        let ledger = ledger(directory.path(), &budget);
        let blobs = BlobStore::open(&root).unwrap();
        ledger.reconcile(&blobs, &Cancellation::default()).unwrap();
        let blob = blobs.put_governed(b"source", &budget, &ledger).unwrap();
        let expected = ledger.observation().unwrap();
        assert_eq!(expected.files, 1);
        let unit = ledger
            .inner
            .state
            .lock()
            .unwrap()
            .root
            .as_ref()
            .unwrap()
            .allocation_unit;
        assert!(expected.disk_bytes >= unit);
        assert_eq!(
            budget.observation().used.disk_bytes,
            u128::from(expected.disk_bytes)
        );
        let shared = ledger.clone();
        drop(blobs);
        assert_eq!(shared.observation().unwrap(), expected);
        let reopened = BlobStore::open(&root).unwrap();
        shared
            .reconcile(&reopened, &Cancellation::default())
            .unwrap();
        reopened.put_governed(b"source", &budget, &shared).unwrap();
        assert_eq!(shared.observation().unwrap(), expected);
        let mut small_policy = budget.policy();
        small_policy.limits.memory_bytes = 1;
        let small = budget.operation([8; 16], small_policy).unwrap();
        assert!(matches!(
            reopened.put_governed(b"source", &small, &shared),
            Err(SourceImageError::Resource(_))
        ));
        assert_eq!(shared.observation().unwrap(), expected);
        assert_eq!(small.observation().used.memory_bytes, 0);
        shared.remove(&reopened, &blob.digest).unwrap();
        assert!(!reopened.path_for(&blob.digest).exists());
        assert_eq!(shared.observation().unwrap().disk_bytes, 0);
        assert_eq!(budget.observation().used.disk_bytes, 0);
    }

    #[test]
    fn rt_cpg_wp79_blob_failed_cleanup_and_unregistered_restart_files_stay_charged() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("blobs");
        let budget = crate::provider_types::source_fixture_budget([2; 16]);
        let owner = ledger(directory.path(), &budget);
        let mut blobs = BlobStore::open(&root).unwrap();
        owner.reconcile(&blobs, &Cancellation::default()).unwrap();
        blobs.publication_fault = 1;
        assert!(blobs.put_governed(b"first", &budget, &owner).is_err());
        assert_eq!(owner.observation().unwrap().files, 0);
        blobs.publication_fault = 2;
        assert!(blobs.put_governed(b"orphan", &budget, &owner).is_err());
        let after = owner.observation().unwrap();
        assert_eq!(after.files, 1);
        let unit = owner
            .inner
            .state
            .lock()
            .unwrap()
            .root
            .as_ref()
            .unwrap()
            .allocation_unit;
        assert!(after.disk_bytes >= unit);
        drop((blobs, owner));
        // A new process lineage does not reuse lease rows to guess directory size.
        fs::write(
            root.join("unregistered-artifact"),
            b"retained independently of SQL",
        )
        .unwrap();
        let restarted_budget = crate::provider_types::source_fixture_budget([2; 16]);
        let restarted = ledger(directory.path(), &restarted_budget);
        let reopened = BlobStore::open(&root).unwrap();
        restarted
            .reconcile(&reopened, &Cancellation::default())
            .unwrap();
        assert_eq!(restarted.observation().unwrap().files, 2);
        assert!(restarted.observation().unwrap().disk_bytes >= after.disk_bytes + unit);
        let cancelled = Cancellation::default();
        cancelled.cancel();
        assert!(restarted.reconcile(&reopened, &cancelled).is_err());
        assert!(!restarted.observation().unwrap().ready);
        assert_eq!(restarted.observation().unwrap().files, 2);
    }

    #[test]
    fn rt_cpg_wp79_blob_disk_exhaustion_foreign_owner_and_root_fail_before_write() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("blobs");
        let budget = crate::provider_types::source_fixture_budget([2; 16]);
        let mut policy = budget.policy();
        policy.limits.disk_bytes = 1;
        let small_root = ResourceBudget::try_process([4; 16], policy).unwrap();
        let small = small_root.workspace([2; 16], policy).unwrap();
        let owner = ledger(directory.path(), &small);
        let blobs = BlobStore::open(&root).unwrap();
        owner.reconcile(&blobs, &Cancellation::default()).unwrap();
        assert!(blobs.put_governed(b"bounded", &small, &owner).is_err());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        assert_eq!(small.observation().used.disk_bytes, 0);
        assert!(owner.validate_owner(&budget).is_err());
        let other = BlobStore::open(&directory.path().join("other")).unwrap();
        assert!(owner.reconcile(&other, &Cancellation::default()).is_err());
        assert!(!owner.observation().unwrap().ready);
    }
}
