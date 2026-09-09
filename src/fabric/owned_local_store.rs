//! Finite workspace ownership around the native local object store.
//!
//! Listings below measure physical occupancy only; Delta's transaction log remains table
//! authority. Native mutation futures run in one admitted native runtime at a time. Their
//! reservations survive cancelled futures and are reconciled only after that runtime joins.
//! Read runtimes may overlap. Clones share the same owner, root allowlist and disk capacity.
//!
//! Failed native completion can leave reserved `#<digits>` staging paths that the native
//! ObjectStore API itself cannot delete. These remain charged by physical census. Automated
//! recovery of such leftovers still requires a later owned physical cleanup seam.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use bytes::Bytes;
use futures::{FutureExt, StreamExt, TryStreamExt, stream::BoxStream};
use object_store::local::LocalFileSystem;
use object_store::path::Path;
use object_store::{
    CopyOptions, GetOptions, GetResult, GetResultPayload, ListResult, MultipartUpload, ObjectMeta,
    ObjectStore, ObjectStoreExt, PutMultipartOptions, PutOptions, PutPayload, PutResult,
    RenameOptions, UploadPart,
};
use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags, fstat, mkdirat, openat, statat};
use thiserror::Error;
use tokio::runtime::Id as RuntimeId;
use url::Url;

use super::native_execution_lane::NativeLaneJoined;
use crate::disk_headroom::{DiskGrowthPermit, DiskHeadroomError, LocalDiskHeadroom};
use crate::resource_budget::{
    ResourceAmounts, ResourceBudget, ResourceBudgetError, ResourceClass, ResourceReservation,
    ResourceScopeKind,
};
use crate::secure_path::open_absolute_directory_nofollow;

/// Explicit finite policy; no numerical release profile is invented at this boundary.
#[derive(Clone, Debug)]
pub(crate) struct OwnedLocalStoreLimits {
    pub max_roots: usize,
    pub max_object_bytes: u64,
    pub max_read_bytes: u64,
    pub max_path_bytes: usize,
    pub max_list_entries: usize,
    pub max_directory_depth: usize,
    pub max_pending_operations: usize,
    pub max_multipart_parts: usize,
}

impl OwnedLocalStoreLimits {
    fn validate(&self) -> Result<(), OwnedLocalStoreError> {
        if self.max_roots == 0
            || self.max_object_bytes == 0
            || self.max_read_bytes == 0
            || self.max_path_bytes == 0
            || self.max_list_entries == 0
            || self.max_directory_depth == 0
            || self.max_pending_operations == 0
            || self.max_multipart_parts == 0
            || self.max_roots > self.max_list_entries
            || usize::try_from(self.max_object_bytes).is_err()
            || usize::try_from(self.max_read_bytes).is_err()
        {
            return Err(OwnedLocalStoreError::InvalidLimits);
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub(crate) enum OwnedLocalStoreError {
    #[error("invalid finite local store limits")]
    InvalidLimits,
    #[error("local store requires the existing workspace resource owner")]
    ForeignOwner,
    #[error("local store physical census is unavailable")]
    CensusUnavailable,
    #[error("local store path is outside its admitted physical roots")]
    DeniedPath,
    #[error("local store path or root changed, is a symlink, or is not a regular file/directory")]
    ChangedPath,
    #[error("local store bound exceeded: {0}")]
    Bound(&'static str),
    #[error("local store mutation requires its admitted native runtime")]
    MutationRuntime,
    #[error("another native mutation still owns the physical store")]
    MutationBusy,
    #[error("native mutation admission closed before cleanup")]
    MutationClosed,
    #[error("native runtime join token does not identify this mutation")]
    ForeignJoin,
    #[error("multipart upload is already terminal or has unfinished parts")]
    UploadState,
    #[error(transparent)]
    Headroom(#[from] DiskHeadroomError),
    #[error("local store physical observation: {0}")]
    Physical(String),
    #[error("local store native operation: {0}")]
    Native(String),
    #[error(transparent)]
    Budget(#[from] ResourceBudgetError),
}

fn object_error(error: impl std::error::Error + Send + Sync + 'static) -> object_store::Error {
    object_store::Error::Generic {
        store: "CodeFabricOwnedLocalStore",
        source: Box::new(error),
    }
}

fn physical(error: impl fmt::Display) -> OwnedLocalStoreError {
    OwnedLocalStoreError::Physical(error.to_string())
}

#[derive(Clone)]
pub(crate) struct OwnedLocalStore {
    inner: Arc<StoreOwner>,
}

struct StoreOwner {
    backend: LocalFileSystem,
    budget: ResourceBudget,
    headroom: LocalDiskHeadroom,
    limits: OwnedLocalStoreLimits,
    allocation_unit: u64,
    state: Mutex<StoreState>,
    #[cfg(test)]
    bootstrap_fail_step: std::sync::atomic::AtomicUsize,
    _metadata: ResourceReservation,
}

struct StoreState {
    roots: Vec<RootBinding>,
    ready: bool,
    generation: u64,
    next_read: u64,
    pending_reads: BTreeMap<u64, PendingRead>,
    active: Option<MutationState>,
    bootstrap: Option<WorkspaceBootstrap>,
    data_disk: ResourceReservation,
    control_disk: ResourceReservation,
    census: OwnedLocalStoreObservation,
}

struct RootBinding {
    path: PathBuf,
    descriptor: OwnedFd,
    identity: (u64, u64),
    class: ResourceClass,
}

// The two parent directory inodes are accounted independently; their descendants are never
// store capabilities. Only activation-control and epochs become recursive census roots.
struct WorkspaceBootstrap {
    anchor_path: PathBuf,
    anchor: OwnedFd,
    anchor_identity: (u64, u64),
    workspace_id: [u8; 16],
    directories: [BootstrapDirectory; 4],
    pending: bool,
    // System calls are synchronous. Failed calls retain the conservative growth admission
    // until a later complete bootstrap census; there is no detached worker to infer finished.
    growth: [Option<DiskGrowthPermit>; 2],
}

struct BootstrapDirectory {
    path: PathBuf,
    descriptor: Option<OwnedFd>,
    identity: Option<(u64, u64)>,
    class: ResourceClass,
}

struct MutationState {
    generation: u64,
    runtime: RuntimeId,
    joined: Option<NativeLaneJoined>,
    accepting: bool,
    next_upload: u64,
    admitted_operations: usize,
    growth: Vec<DiskGrowthPermit>,
    admitted_paths: BTreeMap<PathBuf, usize>,
    uploads: BTreeMap<u64, Arc<UploadOwner>>,
}

struct PendingRead {
    runtime: RuntimeId,
    _charge: Arc<ReadCharge>,
    list: Option<ListAllocation>,
}

struct ListAllocation {
    prefix: PathBuf,
    entries: usize,
}

struct ReadCharge {
    reservation: Mutex<ResourceReservation>,
}

struct ReadTicket {
    id: u64,
    charge: Arc<ReadCharge>,
}

/// Real physical occupancy and admitted pending work, never Delta snapshot meaning.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct OwnedLocalStoreObservation {
    pub roots: usize,
    pub files: usize,
    pub directories: usize,
    pub physical_bytes: u64,
    pub reserved_disk_bytes: u64,
    pub pending_read_operations: usize,
    pub pending_uploads: usize,
    pub mutation_active: bool,
    pub bootstrap_pending: bool,
    pub ready: bool,
}

impl fmt::Debug for OwnedLocalStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwnedLocalStore")
            .field("observation", &self.observe())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for OwnedLocalStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CodeFabricOwnedLocalStore")
    }
}

impl OwnedLocalStore {
    pub(crate) fn try_new(
        budget: ResourceBudget,
        headroom: LocalDiskHeadroom,
        limits: OwnedLocalStoreLimits,
    ) -> Result<Self, OwnedLocalStoreError> {
        limits.validate()?;
        if budget.owner().kind != ResourceScopeKind::Workspace {
            return Err(OwnedLocalStoreError::ForeignOwner);
        }
        let allocation_unit = headroom.observe()?.allocation_unit;
        // Reserve the bounded owner tables before their Vec/BTreeMap entries are created.
        let roots_memory = limits
            .max_roots
            .checked_mul(
                std::mem::size_of::<RootBinding>()
                    .checked_add(limits.max_path_bytes)
                    .ok_or(OwnedLocalStoreError::Bound("root bookkeeping"))?,
            )
            .ok_or(OwnedLocalStoreError::Bound("root bookkeeping"))?;
        let operations_memory = limits
            .max_pending_operations
            .checked_mul(
                limits
                    .max_path_bytes
                    .checked_mul(4)
                    .and_then(|bytes| bytes.checked_add(1024))
                    .ok_or(OwnedLocalStoreError::Bound("operation bookkeeping"))?,
            )
            .ok_or(OwnedLocalStoreError::Bound("operation bookkeeping"))?;
        // Five bounded path buffers: the pinned state directory and four bootstrap entries.
        // The fixed descriptors themselves are inline in StoreOwner/StoreState.
        let bootstrap_memory = limits
            .max_path_bytes
            .checked_mul(5)
            .ok_or(OwnedLocalStoreError::Bound("bootstrap bookkeeping"))?;
        let metadata = budget.try_reserve(
            ResourceClass::Data,
            memory(usize_u64(
                roots_memory
                    .checked_add(bootstrap_memory)
                    .and_then(|bytes| bytes.checked_add(operations_memory))
                    .and_then(|bytes| bytes.checked_add(std::mem::size_of::<StoreOwner>()))
                    .ok_or(OwnedLocalStoreError::Bound("owner bookkeeping"))?,
            )?),
        )?;
        let mut roots = Vec::new();
        roots
            .try_reserve_exact(limits.max_roots)
            .map_err(|_| OwnedLocalStoreError::Bound("root bookkeeping allocation"))?;
        let data_disk = budget.try_reserve(ResourceClass::Data, ResourceAmounts::default())?;
        let control_disk =
            budget.try_reserve(ResourceClass::Control, ResourceAmounts::default())?;
        Ok(Self {
            inner: Arc::new(StoreOwner {
                backend: LocalFileSystem::new(),
                budget,
                headroom,
                limits,
                allocation_unit,
                state: Mutex::new(StoreState {
                    roots,
                    ready: true,
                    generation: 0,
                    next_read: 0,
                    pending_reads: BTreeMap::new(),
                    active: None,
                    bootstrap: None,
                    data_disk,
                    control_disk,
                    census: OwnedLocalStoreObservation::default(),
                }),
                #[cfg(test)]
                bootstrap_fail_step: std::sync::atomic::AtomicUsize::new(0),
                _metadata: metadata,
            }),
        })
    }

    pub(crate) fn budget(&self) -> &ResourceBudget {
        &self.inner.budget
    }

    fn state(&self) -> Result<MutexGuard<'_, StoreState>, OwnedLocalStoreError> {
        self.inner
            .state
            .lock()
            .map_err(|_| OwnedLocalStoreError::CensusUnavailable)
    }

    /// Bootstrap exactly one workspace's native Delta directories. The caller must retain
    /// this store on failure and retry this method before exposing it to work. All filesystem
    /// calls complete on this stack; async mutation cleanup still requires NativeLaneJoined.
    /// Existing roots are opened without following links and physically recounted on restart.
    pub(crate) fn bootstrap_workspace_roots(
        &self,
        state_root: &FsPath,
        workspace_id: [u8; 16],
    ) -> Result<(), OwnedLocalStoreError> {
        self.check_path_bound(state_root)?;
        if workspace_id != self.inner.budget.owner().id {
            return Err(OwnedLocalStoreError::ForeignOwner);
        }
        let mut state = self.state()?;
        if state.active.is_some() || !state.pending_reads.is_empty() {
            return Err(OwnedLocalStoreError::MutationBusy);
        }
        if let Some(bootstrap) = &state.bootstrap {
            if bootstrap.anchor_path != state_root || bootstrap.workspace_id != workspace_id {
                return Err(OwnedLocalStoreError::DeniedPath);
            }
        } else {
            if !state.roots.is_empty()
                || self.inner.limits.max_roots < 2
                || self.inner.limits.max_list_entries < 4
            {
                return Err(OwnedLocalStoreError::DeniedPath);
            }
            self.inner.headroom.validate_directory(state_root)?;
            let anchor = open_absolute_directory_nofollow(state_root).map_err(physical)?;
            let stat = fstat(&anchor).map_err(physical)?;
            let mut hex = [0_u8; 32];
            for (index, byte) in workspace_id.iter().copied().enumerate() {
                hex[index * 2] = b"0123456789abcdef"[usize::from(byte >> 4)];
                hex[index * 2 + 1] = b"0123456789abcdef"[usize::from(byte & 15)];
            }
            let workspace_name = std::str::from_utf8(&hex).expect("hex is ASCII");
            let path = |components: &[&str]| -> Result<PathBuf, OwnedLocalStoreError> {
                let bytes = components
                    .iter()
                    .try_fold(
                        state_root.as_os_str().as_bytes().len(),
                        |bytes, component| {
                            bytes
                                .checked_add(1)
                                .and_then(|bytes| bytes.checked_add(component.len()))
                        },
                    )
                    .ok_or(OwnedLocalStoreError::Bound("bootstrap path"))?;
                if bytes > self.inner.limits.max_path_bytes {
                    return Err(OwnedLocalStoreError::Bound("bootstrap path"));
                }
                let mut path = PathBuf::new();
                path.try_reserve_exact(self.inner.limits.max_path_bytes)
                    .map_err(|_| OwnedLocalStoreError::Bound("bootstrap path allocation"))?;
                path.push(state_root);
                for component in components {
                    path.push(component);
                }
                Ok(path)
            };
            let directory =
                |components: &[&str], class| -> Result<BootstrapDirectory, OwnedLocalStoreError> {
                    Ok(BootstrapDirectory {
                        path: path(components)?,
                        descriptor: None,
                        identity: None,
                        class,
                    })
                };
            let directories = [
                directory(&["fabric"], ResourceClass::Control)?,
                directory(&["fabric", workspace_name], ResourceClass::Control)?,
                directory(
                    &["fabric", workspace_name, "activation-control"],
                    ResourceClass::Control,
                )?,
                directory(&["fabric", workspace_name, "epochs"], ResourceClass::Data)?,
            ];
            let anchor_path = path(&[])?;
            let data_growth = self.inner.allocation_unit;
            let control_growth = data_growth
                .checked_mul(3)
                .ok_or(OwnedLocalStoreError::Bound("bootstrap directory growth"))?;
            let data_permit = self
                .inner
                .headroom
                .try_reserve_growth(data_growth, ResourceClass::Data)?;
            let control_permit = self
                .inner
                .headroom
                .try_reserve_growth(control_growth, ResourceClass::Control)?;
            // No mkdir has happened yet. If the second class fails, restore the first class.
            state.data_disk.try_grow(disk(data_growth))?;
            if let Err(error) = state.control_disk.try_grow(disk(control_growth)) {
                state.data_disk.shrink(disk(data_growth))?;
                return Err(error.into());
            }
            state.bootstrap = Some(WorkspaceBootstrap {
                anchor_path,
                anchor,
                anchor_identity: (stat.st_dev, stat.st_ino),
                workspace_id,
                directories,
                pending: true,
                growth: [Some(data_permit), Some(control_permit)],
            });
        }
        state.ready = false;
        let bootstrap = state.bootstrap.as_mut().expect("installed bootstrap");
        bootstrap.pending = true;
        let observed =
            open_absolute_directory_nofollow(&bootstrap.anchor_path).map_err(physical)?;
        let stat = fstat(&observed).map_err(physical)?;
        if (stat.st_dev, stat.st_ino) != bootstrap.anchor_identity {
            return Err(OwnedLocalStoreError::ChangedPath);
        }
        for index in 0..4 {
            #[cfg(test)]
            if self
                .inner
                .bootstrap_fail_step
                .load(std::sync::atomic::Ordering::Acquire)
                == index + 1
            {
                return Err(OwnedLocalStoreError::Bound("injected bootstrap failure"));
            }
            let (parents, entries) = bootstrap.directories.split_at_mut(index);
            let parent = match index {
                0 => &bootstrap.anchor,
                1 => parents[0]
                    .descriptor
                    .as_ref()
                    .ok_or(OwnedLocalStoreError::CensusUnavailable)?,
                _ => parents[1]
                    .descriptor
                    .as_ref()
                    .ok_or(OwnedLocalStoreError::CensusUnavailable)?,
            };
            let entry = &mut entries[0];
            let name = entry
                .path
                .file_name()
                .ok_or(OwnedLocalStoreError::DeniedPath)?;
            // Never recreate an already pinned directory if it disappears or is substituted.
            if entry.identity.is_none() {
                match mkdirat(parent, name, Mode::RWXU) {
                    Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                    Err(error) => return Err(physical(error)),
                }
            }
            let descriptor =
                openat(parent, name, directory_flags(), Mode::empty()).map_err(physical)?;
            let stat = fstat(&descriptor).map_err(physical)?;
            let identity = (stat.st_dev, stat.st_ino);
            if stat.st_dev != bootstrap.anchor_identity.0
                || entry.identity.is_some_and(|old| old != identity)
            {
                return Err(OwnedLocalStoreError::ChangedPath);
            }
            entry.identity = Some(identity);
            entry.descriptor = Some(descriptor);
        }
        let StoreState {
            bootstrap, roots, ..
        } = &mut *state;
        let bootstrap = bootstrap.as_ref().expect("installed bootstrap");
        for entry in &bootstrap.directories[2..] {
            if let Some(existing) = roots.iter().find(|root| root.path == entry.path) {
                if Some(existing.identity) != entry.identity || existing.class != entry.class {
                    return Err(OwnedLocalStoreError::ChangedPath);
                }
                continue;
            }
            let descriptor = openat(
                entry.descriptor.as_ref().expect("opened directory"),
                ".",
                directory_flags(),
                Mode::empty(),
            )
            .map_err(physical)?;
            roots.push(RootBinding {
                path: entry.path.clone(),
                descriptor,
                identity: entry.identity.expect("opened directory"),
                class: entry.class,
            });
        }
        self.reconcile_locked(&mut state)?;
        let bootstrap = state.bootstrap.as_mut().expect("installed bootstrap");
        bootstrap.pending = false;
        bootstrap.growth = [None, None];
        Ok(())
    }

    /// Admit an existing private physical directory. Overlapping roots are rejected so a
    /// restart census cannot double-register the same subtree under separate capacities.
    pub(crate) fn register_root(
        &self,
        root: &Url,
        class: ResourceClass,
    ) -> Result<(), OwnedLocalStoreError> {
        if root.scheme() != "file"
            || root.host_str().is_some()
            || root.query().is_some()
            || root.fragment().is_some()
        {
            return Err(OwnedLocalStoreError::DeniedPath);
        }
        let path = root
            .to_file_path()
            .map_err(|()| OwnedLocalStoreError::DeniedPath)?;
        self.check_path_bound(&path)?;
        self.inner.headroom.validate_directory(&path)?;
        let descriptor = open_absolute_directory_nofollow(&path).map_err(physical)?;
        let stat = fstat(&descriptor).map_err(physical)?;
        let mut state = self.state()?;
        if state.active.is_some() || !state.pending_reads.is_empty() {
            return Err(OwnedLocalStoreError::MutationBusy);
        }
        if state
            .bootstrap
            .as_ref()
            .is_some_and(|bootstrap| bootstrap.pending)
        {
            return Err(OwnedLocalStoreError::CensusUnavailable);
        }
        for existing in &state.roots {
            if existing.path == path && existing.class == class {
                return self.reconcile_locked(&mut state);
            }
            if path.starts_with(&existing.path) || existing.path.starts_with(&path) {
                return Err(OwnedLocalStoreError::DeniedPath);
            }
        }
        if state.roots.len() >= self.inner.limits.max_roots {
            return Err(OwnedLocalStoreError::Bound("roots"));
        }
        state.roots.push(RootBinding {
            path,
            descriptor,
            identity: (stat.st_dev, stat.st_ino),
            class,
        });
        self.reconcile_locked(&mut state)
    }

    /// Acquire the single conservative writer lane. Every mutating ObjectStore method checks
    /// this exact runtime identity; a concurrent read lane cannot borrow the mutation grant.
    pub(crate) fn begin_mutation(&self) -> Result<OwnedLocalMutation, OwnedLocalStoreError> {
        let runtime = current_runtime()?;
        let mut state = self.state()?;
        if !state.ready {
            return Err(OwnedLocalStoreError::CensusUnavailable);
        }
        if state.active.is_some() {
            return Err(OwnedLocalStoreError::MutationBusy);
        }
        state.generation = state
            .generation
            .checked_add(1)
            .ok_or(OwnedLocalStoreError::Bound("mutation generations"))?;
        let generation = state.generation;
        state.active = Some(MutationState {
            generation,
            runtime,
            joined: None,
            accepting: true,
            next_upload: 0,
            admitted_operations: 0,
            growth: Vec::new(),
            admitted_paths: BTreeMap::new(),
            uploads: BTreeMap::new(),
        });
        Ok(OwnedLocalMutation {
            store: self.clone(),
            generation,
            runtime,
        })
    }

    /// Release only read reservations whose runtime has actually joined. A returned Bytes
    /// allocation retains its own Arc charge independently of this pending-operation table.
    pub(crate) fn reconcile_after_join(
        &self,
        joined: NativeLaneJoined,
    ) -> Result<(), OwnedLocalStoreError> {
        let mut state = self.state()?;
        state
            .pending_reads
            .retain(|_, pending| pending.runtime != joined.runtime_id());
        if state.pending_reads.is_empty() {
            if let Some(active) = state
                .active
                .as_ref()
                .filter(|active| active.joined.is_some())
            {
                let mutation = OwnedLocalMutation {
                    store: self.clone(),
                    generation: active.generation,
                    runtime: active.runtime,
                };
                let joined = active.joined.expect("filtered joined mutation");
                mutation.reconcile_locked_after_join(&mut state, joined)?;
            }
        }
        Ok(())
    }

    /// Retry a failed or deferred physical census using retained proof that the old runtime
    /// already joined. This never grants a new mutator or treats a live lane as terminal.
    pub(crate) fn retry_after_join_reconciliation(&self) -> Result<(), OwnedLocalStoreError> {
        let mut state = self.state()?;
        let active = state
            .active
            .as_ref()
            .ok_or(OwnedLocalStoreError::ForeignJoin)?;
        let joined = active.joined.ok_or(OwnedLocalStoreError::ForeignJoin)?;
        let mutation = OwnedLocalMutation {
            store: self.clone(),
            generation: active.generation,
            runtime: active.runtime,
        };
        mutation.reconcile_locked_after_join(&mut state, joined)
    }

    pub(crate) fn observe(&self) -> Result<OwnedLocalStoreObservation, OwnedLocalStoreError> {
        let state = self.state()?;
        let mut observation = state.census;
        observation.roots = state.roots.len();
        observation.reserved_disk_bytes =
            state.data_disk.amounts().disk_bytes + state.control_disk.amounts().disk_bytes;
        observation.pending_read_operations = state.pending_reads.len();
        observation.pending_uploads = state
            .active
            .as_ref()
            .map_or(0, |active| active.uploads.len());
        observation.mutation_active = state.active.is_some();
        observation.bootstrap_pending = state
            .bootstrap
            .as_ref()
            .is_some_and(|bootstrap| bootstrap.pending);
        observation.ready = state.ready;
        Ok(observation)
    }

    fn check_path_bound(&self, path: &FsPath) -> Result<(), OwnedLocalStoreError> {
        if path.as_os_str().as_bytes().len() > self.inner.limits.max_path_bytes {
            return Err(OwnedLocalStoreError::Bound("path bytes"));
        }
        if !path.is_absolute()
            || path
                .components()
                .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
        {
            return Err(OwnedLocalStoreError::DeniedPath);
        }
        Ok(())
    }

    fn validate_path(
        &self,
        state: &StoreState,
        location: &Path,
    ) -> Result<(PathBuf, ResourceClass, usize), OwnedLocalStoreError> {
        if !state.ready {
            return Err(OwnedLocalStoreError::CensusUnavailable);
        }
        let path = self
            .inner
            .backend
            .path_to_filesystem(location)
            .map_err(physical)?;
        self.check_path_bound(&path)?;
        let root = state
            .roots
            .iter()
            .find(|root| path.starts_with(&root.path))
            .ok_or(OwnedLocalStoreError::DeniedPath)?;
        validate_root(root)?;
        let relative = path
            .strip_prefix(&root.path)
            .map_err(|_| OwnedLocalStoreError::DeniedPath)?;
        let depth = relative.components().count();
        if depth > self.inner.limits.max_directory_depth {
            return Err(OwnedLocalStoreError::Bound("directory depth"));
        }
        // The native local backend remains the mutator. Fail closed on existing symlinks,
        // device files or replaced ancestor directories before passing it a physical path.
        let mut directory =
            openat(&root.descriptor, ".", directory_flags(), Mode::empty()).map_err(physical)?;
        for (index, component) in relative.components().enumerate() {
            let Component::Normal(name) = component else {
                return Err(OwnedLocalStoreError::DeniedPath);
            };
            let stat = match statat(&directory, name, AtFlags::SYMLINK_NOFOLLOW) {
                Ok(stat) => stat,
                Err(rustix::io::Errno::NOENT) => break,
                Err(error) => return Err(physical(error)),
            };
            let kind = FileType::from_raw_mode(stat.st_mode);
            if kind.is_dir() {
                directory =
                    openat(&directory, name, directory_flags(), Mode::empty()).map_err(physical)?;
                let opened = fstat(&directory).map_err(physical)?;
                if opened.st_dev != stat.st_dev || opened.st_ino != stat.st_ino {
                    return Err(OwnedLocalStoreError::ChangedPath);
                }
            } else if !kind.is_file() || index + 1 != depth {
                return Err(OwnedLocalStoreError::ChangedPath);
            }
        }
        Ok((path, root.class, depth))
    }

    fn admit_mutation(&self, location: &Path, bytes: u64) -> Result<(), OwnedLocalStoreError> {
        if bytes > self.inner.limits.max_object_bytes {
            return Err(OwnedLocalStoreError::Bound("object bytes"));
        }
        let runtime = current_runtime()?;
        let mut state = self.state()?;
        let (physical_path, class, depth) = self.validate_path(&state, location)?;
        let active = state
            .active
            .as_ref()
            .ok_or(OwnedLocalStoreError::MutationRuntime)?;
        if active.runtime != runtime {
            return Err(OwnedLocalStoreError::MutationRuntime);
        }
        if !active.accepting {
            return Err(OwnedLocalStoreError::MutationClosed);
        }
        if active.admitted_operations >= self.inner.limits.max_pending_operations {
            return Err(OwnedLocalStoreError::Bound("mutation operations"));
        }
        // Lists may still be buffering native metadata while another admitted native worker
        // adds files. Grow each matching list owner before admitting those physical changes.
        let extra_entries = depth
            .checked_add(2)
            .ok_or(OwnedLocalStoreError::Bound("list growth"))?;
        let extra_memory = metadata_capacity(
            extra_entries,
            physical_path
                .as_os_str()
                .as_bytes()
                .len()
                .checked_add(32)
                .ok_or(OwnedLocalStoreError::Bound("list growth"))?,
        )?;
        for pending in state.pending_reads.values_mut() {
            if let Some(list) = pending
                .list
                .as_mut()
                .filter(|list| physical_path.starts_with(&list.prefix))
            {
                let entries = list
                    .entries
                    .checked_add(extra_entries)
                    .ok_or(OwnedLocalStoreError::Bound("list entries"))?;
                if entries > self.inner.limits.max_list_entries {
                    return Err(OwnedLocalStoreError::Bound("list entries"));
                }
                pending
                    ._charge
                    .reservation
                    .lock()
                    .map_err(|_| OwnedLocalStoreError::CensusUnavailable)?
                    .try_grow(memory(extra_memory))?;
                list.entries = entries;
            }
        }
        let file = rounded(bytes, self.inner.allocation_unit)?;
        let directories = usize_u64(depth)?
            .checked_mul(self.inner.allocation_unit)
            .ok_or(OwnedLocalStoreError::Bound("directory growth"))?;
        let growth = file
            .checked_add(directories)
            .ok_or(OwnedLocalStoreError::Bound("disk growth"))?;
        let permit = self.inner.headroom.try_reserve_growth(growth, class)?;
        disk_reservation(&mut state, class).try_grow(disk(growth))?;
        let active = state.active.as_mut().expect("validated active mutation");
        active.growth.push(permit);
        active.admitted_paths.insert(physical_path, extra_entries);
        active.admitted_operations += 1;
        Ok(())
    }

    fn validate_mutation(&self, location: &Path) -> Result<(), OwnedLocalStoreError> {
        let state = self.state()?;
        self.validate_path(&state, location)?;
        let active = state
            .active
            .as_ref()
            .ok_or(OwnedLocalStoreError::MutationRuntime)?;
        if active.runtime != current_runtime()? {
            return Err(OwnedLocalStoreError::MutationRuntime);
        }
        if !active.accepting {
            return Err(OwnedLocalStoreError::MutationClosed);
        }
        Ok(())
    }

    fn read_ticket(&self, location: &Path, bytes: u64) -> Result<ReadTicket, OwnedLocalStoreError> {
        if bytes > self.inner.limits.max_read_bytes {
            return Err(OwnedLocalStoreError::Bound("read bytes"));
        }
        let runtime = current_runtime()?;
        let mut state = self.state()?;
        let (_, class, _) = self.validate_path(&state, location)?;
        if state.pending_reads.len() >= self.inner.limits.max_pending_operations {
            return Err(OwnedLocalStoreError::Bound("read operations"));
        }
        let reservation = self.inner.budget.try_reserve(
            class,
            memory(
                bytes
                    .checked_add(
                        usize_u64(self.inner.limits.max_path_bytes)?
                            .checked_mul(3)
                            .and_then(|bytes| bytes.checked_add(512))
                            .ok_or(OwnedLocalStoreError::Bound("read metadata"))?,
                    )
                    .ok_or(OwnedLocalStoreError::Bound("read metadata"))?,
            ),
        )?;
        let charge = Arc::new(ReadCharge {
            reservation: Mutex::new(reservation),
        });
        state.next_read = state
            .next_read
            .checked_add(1)
            .ok_or(OwnedLocalStoreError::Bound("read identities"))?;
        let id = state.next_read;
        state.pending_reads.insert(
            id,
            PendingRead {
                runtime,
                _charge: Arc::clone(&charge),
                list: None,
            },
        );
        Ok(ReadTicket { id, charge })
    }

    fn complete_read(&self, ticket: &ReadTicket) -> Result<(), OwnedLocalStoreError> {
        let mut state = self.state()?;
        state.pending_reads.remove(&ticket.id);
        if state.pending_reads.is_empty()
            && let Some(active) = state.active.as_ref()
            && let Some(joined) = active.joined
        {
            let mutation = OwnedLocalMutation {
                store: self.clone(),
                generation: active.generation,
                runtime: active.runtime,
            };
            mutation.reconcile_locked_after_join(&mut state, joined)?;
        }
        Ok(())
    }

    fn owned_payload(
        &self,
        location: &Path,
        payload: PutPayload,
    ) -> Result<PutPayload, OwnedLocalStoreError> {
        let length = usize_u64(payload.content_length())?;
        if length > self.inner.limits.max_object_bytes {
            return Err(OwnedLocalStoreError::Bound("object bytes"));
        }
        let (_, class, _) = self.validate_path(&*self.state()?, location)?;
        let charge = self.inner.budget.try_reserve(class, memory(length))?;
        let mut data = Vec::with_capacity(payload.content_length());
        for part in payload.iter() {
            data.extend_from_slice(part);
        }
        // Copying severs potentially oversized slice backing retained by the caller's Bytes.
        Ok(Bytes::from_owner(WriteBytes {
            bytes: data,
            _charge: charge,
        })
        .into())
    }

    fn prepare_list(
        &self,
        ticket: &ReadTicket,
        prefix: &Path,
        streaming: bool,
    ) -> Result<(), OwnedLocalStoreError> {
        let mut state = self.state()?;
        let (physical_path, class, _) = self.validate_path(&state, prefix)?;
        let scratch = self
            .inner
            .limits
            .max_directory_depth
            .checked_add(1)
            .and_then(|depth| {
                depth.checked_mul(crate::secure_path::DIRECTORY_ITERATION_MEMORY_BOUND)
            })
            .ok_or(OwnedLocalStoreError::Bound("list census scratch"))?;
        let _scratch = self
            .inner
            .budget
            .try_reserve(class, memory(usize_u64(scratch)?))?;
        let mut census = PhysicalCensus::default();
        match std::fs::symlink_metadata(&physical_path) {
            Ok(metadata) if metadata.is_dir() => {
                let descriptor =
                    open_absolute_directory_nofollow(&physical_path).map_err(physical)?;
                census_directory(
                    &descriptor,
                    0,
                    physical_path.as_os_str().as_bytes().len(),
                    &self.inner.limits,
                    self.inner.allocation_unit,
                    &mut census,
                )?;
            }
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => return Err(OwnedLocalStoreError::ChangedPath),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(physical(error)),
        }
        let mut entries = census
            .files
            .checked_add(census.directories)
            .ok_or(OwnedLocalStoreError::Bound("list entries"))?;
        let mut paths = census
            .maximum_path_bytes
            .max(physical_path.as_os_str().as_bytes().len());
        // An earlier admitted native task may still be creating its path after this
        // physical walk. Include those exact admitted paths, even before they are visible.
        if let Some(active) = &state.active {
            for (path, growth) in &active.admitted_paths {
                if path.starts_with(&physical_path) {
                    entries = entries
                        .checked_add(*growth)
                        .ok_or(OwnedLocalStoreError::Bound("list entries"))?;
                    paths = paths.max(
                        path.as_os_str()
                            .as_bytes()
                            .len()
                            .checked_add(32)
                            .ok_or(OwnedLocalStoreError::Bound("list metadata"))?,
                    );
                }
            }
        }
        let fixed_batch = if streaming {
            1024_usize
                .checked_mul(std::mem::size_of::<object_store::Result<ObjectMeta>>())
                .ok_or(OwnedLocalStoreError::Bound("native list batch"))?
        } else {
            0
        };
        let capacity = metadata_capacity(entries, paths)?
            .checked_add(usize_u64(fixed_batch)?)
            .and_then(|bytes| bytes.checked_add(u64::try_from(scratch).ok()?))
            .ok_or(OwnedLocalStoreError::Bound("list metadata"))?;
        ticket
            .charge
            .reservation
            .lock()
            .map_err(|_| OwnedLocalStoreError::CensusUnavailable)?
            .try_grow(memory(capacity))?;
        state
            .pending_reads
            .get_mut(&ticket.id)
            .ok_or(OwnedLocalStoreError::CensusUnavailable)?
            .list = Some(ListAllocation {
            prefix: physical_path,
            entries,
        });
        Ok(())
    }

    fn finish_list(
        &self,
        ticket: &ReadTicket,
        retained_bytes: u64,
    ) -> Result<(), OwnedLocalStoreError> {
        let state = self.state()?;
        let pending = state
            .pending_reads
            .get(&ticket.id)
            .ok_or(OwnedLocalStoreError::CensusUnavailable)?;
        // A completed listing owns no live filesystem operation. Bound its metadata,
        // but do not retain a reader ticket until the serving runtime shuts down.
        let reservation = pending
            ._charge
            .reservation
            .lock()
            .map_err(|_| OwnedLocalStoreError::CensusUnavailable)?;
        let retained = retained_bytes
            .checked_add(512)
            .ok_or(OwnedLocalStoreError::Bound("list metadata"))?;
        let current = reservation.amounts().memory_bytes;
        let over_limit = retained > current;
        drop(reservation);
        drop(state);
        self.complete_read(ticket)?;
        if over_limit {
            Err(OwnedLocalStoreError::Bound("list metadata admission"))
        } else {
            Ok(())
        }
    }

    fn validate_meta(&self, meta: &ObjectMeta) -> Result<(), OwnedLocalStoreError> {
        if meta.location.as_ref().len() > self.inner.limits.max_path_bytes
            || meta
                .e_tag
                .as_ref()
                .is_some_and(|value| value.len() > self.inner.limits.max_path_bytes)
            || meta
                .version
                .as_ref()
                .is_some_and(|value| value.len() > self.inner.limits.max_path_bytes)
        {
            return Err(OwnedLocalStoreError::Bound("metadata value"));
        }
        self.validate_path(&*self.state()?, &meta.location)?;
        Ok(())
    }

    fn reconcile_locked(&self, state: &mut StoreState) -> Result<(), OwnedLocalStoreError> {
        state.ready = false;
        let scratch_bytes = self
            .inner
            .limits
            .max_directory_depth
            .checked_add(1)
            .and_then(|depth| {
                depth.checked_mul(crate::secure_path::DIRECTORY_ITERATION_MEMORY_BOUND)
            })
            .ok_or(OwnedLocalStoreError::Bound("census scratch"))?;
        let _scratch = self
            .inner
            .budget
            .try_reserve(ResourceClass::Control, memory(usize_u64(scratch_bytes)?))?;
        let mut census = PhysicalCensus::default();
        if let Some(bootstrap) = &state.bootstrap {
            if state.roots.len() != 2
                || bootstrap
                    .directories
                    .iter()
                    .any(|entry| entry.descriptor.is_none())
            {
                return Err(OwnedLocalStoreError::CensusUnavailable);
            }
            for entry in &bootstrap.directories[..2] {
                let descriptor = open_absolute_directory_nofollow(&entry.path).map_err(physical)?;
                let stat = fstat(&descriptor).map_err(physical)?;
                if Some((stat.st_dev, stat.st_ino)) != entry.identity {
                    return Err(OwnedLocalStoreError::ChangedPath);
                }
                let bytes = u64::try_from(stat.st_blocks)
                    .map_err(physical)?
                    .checked_mul(512)
                    .ok_or(OwnedLocalStoreError::Bound("bootstrap census"))?
                    .max(self.inner.allocation_unit);
                census.bytes = census
                    .bytes
                    .checked_add(bytes)
                    .ok_or(OwnedLocalStoreError::Bound("bootstrap census"))?;
                census.control_bytes = census
                    .control_bytes
                    .checked_add(bytes)
                    .ok_or(OwnedLocalStoreError::Bound("bootstrap census"))?;
                census.directories += 1;
            }
        }
        for root in &state.roots {
            validate_root(root)?;
            let before = census.bytes;
            census_directory(
                &root.descriptor,
                0,
                root.path.as_os_str().as_bytes().len(),
                &self.inner.limits,
                self.inner.allocation_unit,
                &mut census,
            )?;
            let bytes = census.bytes - before;
            let total = match root.class {
                ResourceClass::Data => &mut census.data_bytes,
                ResourceClass::Control => &mut census.control_bytes,
            };
            *total = total
                .checked_add(bytes)
                .ok_or(OwnedLocalStoreError::Bound("census bytes"))?;
            validate_root(root)?;
        }
        // Grow both classes before releasing either previous charge. A failed admission
        // leaves every old physical byte charged while the store remains fail-closed.
        for (reservation, required) in [
            (&mut state.data_disk, census.data_bytes),
            (&mut state.control_disk, census.control_bytes),
        ] {
            if required > reservation.amounts().disk_bytes {
                reservation.try_grow(disk(required - reservation.amounts().disk_bytes))?;
            }
        }
        resize_disk(&mut state.data_disk, census.data_bytes)?;
        resize_disk(&mut state.control_disk, census.control_bytes)?;
        state.census = OwnedLocalStoreObservation {
            roots: state.roots.len(),
            files: census.files,
            directories: census.directories,
            physical_bytes: census.bytes,
            ..OwnedLocalStoreObservation::default()
        };
        state.ready = true;
        Ok(())
    }
}

/// A mutation lease is not released by Drop: only the unforgeable joined-runtime token can
/// finish it. Losing this handle fails subsequent mutation admission closed.
#[derive(Clone, Debug)]
pub(crate) struct OwnedLocalMutation {
    store: OwnedLocalStore,
    generation: u64,
    runtime: RuntimeId,
}

impl OwnedLocalMutation {
    fn validate(&self, active: &MutationState) -> Result<(), OwnedLocalStoreError> {
        if active.generation != self.generation || active.runtime != self.runtime {
            return Err(OwnedLocalStoreError::ForeignJoin);
        }
        Ok(())
    }

    /// Call in the still-live native lane after its operation future is dropped or finished.
    /// Store ownership, not the public upload handle, keeps native uploads alive until here.
    pub(crate) async fn drain_cleanup(&self) -> Result<(), OwnedLocalStoreError> {
        if current_runtime()? != self.runtime {
            return Err(OwnedLocalStoreError::MutationRuntime);
        }
        let uploads = {
            let mut state = self.store.state()?;
            let active = state
                .active
                .as_mut()
                .ok_or(OwnedLocalStoreError::ForeignJoin)?;
            self.validate(active)?;
            // Closing and snapshotting are one synchronized operation. Multipart creation
            // also holds this lock through native creation and owner registration.
            active.accepting = false;
            active.uploads.values().cloned().collect::<Vec<_>>()
        };
        let mut error = None;
        for upload in uploads {
            let mut state = upload.native.lock().await;
            if !state.terminal {
                match state
                    .upload
                    .as_mut()
                    .ok_or(OwnedLocalStoreError::UploadState)?
                    .abort()
                    .await
                {
                    Ok(()) => state.terminal = true,
                    Err(source) => {
                        error.get_or_insert_with(|| {
                            OwnedLocalStoreError::Native(source.to_string())
                        });
                        // LocalUpload::abort takes its staging path before IO; retain the owner
                        // through the runtime barrier even on errors. Census retains residual bytes.
                        state.terminal = true;
                    }
                }
            }
        }
        error.map_or(Ok(()), Err)
    }

    pub(crate) fn reconcile_after_join(
        &self,
        joined: NativeLaneJoined,
    ) -> Result<(), OwnedLocalStoreError> {
        let mut state = self.store.state()?;
        self.reconcile_locked_after_join(&mut state, joined)
    }

    fn reconcile_locked_after_join(
        &self,
        state: &mut StoreState,
        joined: NativeLaneJoined,
    ) -> Result<(), OwnedLocalStoreError> {
        if joined.runtime_id() != self.runtime {
            return Err(OwnedLocalStoreError::ForeignJoin);
        }
        let active = state
            .active
            .as_ref()
            .ok_or(OwnedLocalStoreError::ForeignJoin)?;
        self.validate(active)?;
        // Keep the unforgeable barrier evidence even if census or capacity admission fails,
        // so reserved control work can retry without reconstructing native authority.
        let active = state.active.as_mut().expect("validated active mutation");
        active.joined = Some(joined);
        active.accepting = false;
        // A missed before_join callback must not drop a live native multipart object here.
        for upload in active.uploads.values() {
            let mut upload = upload
                .native
                .try_lock()
                .map_err(|_| OwnedLocalStoreError::UploadState)?;
            if !upload.terminal {
                return Err(OwnedLocalStoreError::UploadState);
            }
            // Native abort unlinks staging paths but retains its open file descriptor.
            // Close that descriptor after worker join and before census can release bytes.
            drop(upload.upload.take());
        }
        state
            .pending_reads
            .retain(|_, pending| pending.runtime != joined.runtime_id());
        // Other read runtimes may own File/ReadDir descriptors for an inode this writer
        // unlinked or replaced. A path census cannot see those live blocks. Retain all
        // admitted disk bytes and headroom permits until the last such runtime joins.
        if !state.pending_reads.is_empty() {
            return Ok(());
        }
        self.store.reconcile_locked(state)?;
        state.active.take();
        Ok(())
    }
}

struct WriteBytes {
    bytes: Vec<u8>,
    _charge: ResourceReservation,
}
impl AsRef<[u8]> for WriteBytes {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}
struct ReadBytes {
    bytes: Bytes,
    _charge: Arc<ReadCharge>,
}
impl AsRef<[u8]> for ReadBytes {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

#[async_trait]
impl ObjectStore for OwnedLocalStore {
    async fn put_opts(
        &self,
        location: &Path,
        payload: PutPayload,
        options: PutOptions,
    ) -> object_store::Result<PutResult> {
        self.admit_mutation(
            location,
            usize_u64(payload.content_length()).map_err(object_error)?,
        )
        .map_err(object_error)?;
        let payload = self
            .owned_payload(location, payload)
            .map_err(object_error)?;
        self.inner
            .backend
            .put_opts(location, payload, options)
            .await
    }

    async fn put_multipart_opts(
        &self,
        location: &Path,
        options: PutMultipartOptions,
    ) -> object_store::Result<Box<dyn MultipartUpload>> {
        self.admit_mutation(location, 0).map_err(object_error)?;
        let mut state = self.state().map_err(object_error)?;
        let active = state
            .active
            .as_mut()
            .ok_or_else(|| object_error(OwnedLocalStoreError::MutationRuntime))?;
        if active.runtime != current_runtime().map_err(object_error)? {
            return Err(object_error(OwnedLocalStoreError::MutationRuntime));
        }
        if !active.accepting {
            return Err(object_error(OwnedLocalStoreError::MutationClosed));
        }
        // Pinned LocalFileSystem::put_multipart_opts has no suspension points. Poll it
        // synchronously while holding the admission lock, then register the native owner
        // before cleanup can close admission or snapshot uploads. Do not await a future
        // while holding the lock; a changed native implementation must fail closed here.
        let upload = self
            .inner
            .backend
            .put_multipart_opts(location, options)
            .now_or_never()
            .ok_or_else(|| {
                object_error(OwnedLocalStoreError::Native(
                    "pinned local multipart creation unexpectedly suspended".into(),
                ))
            })??;
        let owner = Arc::new(UploadOwner {
            native: tokio::sync::Mutex::new(NativeUpload {
                upload: Some(upload),
                terminal: false,
            }),
            parts: Mutex::new(UploadParts {
                count: 0,
                bytes: 0,
                unfinished: 0,
            }),
        });
        active.next_upload = active
            .next_upload
            .checked_add(1)
            .ok_or_else(|| object_error(OwnedLocalStoreError::Bound("upload identities")))?;
        active
            .uploads
            .insert(active.next_upload, Arc::clone(&owner));
        Ok(Box::new(OwnedUpload {
            store: self.clone(),
            location: location.clone(),
            owner,
        }))
    }

    async fn get_opts(
        &self,
        location: &Path,
        options: GetOptions,
    ) -> object_store::Result<GetResult> {
        let ticket = self.read_ticket(location, 0).map_err(object_error)?;
        let outcome = async {
            let head = options.head;
            let result = self.inner.backend.get_opts(location, options).await?;
            self.validate_meta(&result.meta).map_err(object_error)?;
            if head {
                return Ok(GetResult {
                    payload: GetResultPayload::Stream(futures::stream::empty().boxed()),
                    ..result
                });
            }
            let length = result
                .range
                .end
                .checked_sub(result.range.start)
                .ok_or_else(|| object_error(OwnedLocalStoreError::Bound("read range")))?;
            if length > self.inner.limits.max_read_bytes {
                return Err(object_error(OwnedLocalStoreError::Bound("read bytes")));
            }
            ticket
                .charge
                .reservation
                .lock()
                .map_err(|_| object_error(OwnedLocalStoreError::CensusUnavailable))?
                .try_grow(memory(length))
                .map_err(object_error)?;
            let GetResult {
                meta,
                range,
                attributes,
                payload,
            } = result;
            let bytes = GetResult {
                meta: meta.clone(),
                range: range.clone(),
                attributes: attributes.clone(),
                payload,
            }
            .bytes()
            .await?;
            let bytes = Bytes::from_owner(ReadBytes {
                bytes,
                _charge: Arc::clone(&ticket.charge),
            });
            Ok(GetResult {
                meta,
                range,
                attributes,
                payload: GetResultPayload::Stream(
                    futures::stream::once(async move { Ok(bytes) }).boxed(),
                ),
            })
        }
        .await;
        // Even NotFound/head/error completion is terminal. If this future is
        // cancelled during native IO, the ticket remains until its runtime joins.
        self.complete_read(&ticket).map_err(object_error)?;
        outcome
    }

    async fn get_ranges(
        &self,
        location: &Path,
        ranges: &[Range<u64>],
    ) -> object_store::Result<Vec<Bytes>> {
        if ranges.len() > self.inner.limits.max_pending_operations {
            return Err(object_error(OwnedLocalStoreError::Bound("read ranges")));
        }
        let total = ranges
            .iter()
            .try_fold(0_u64, |total, range| {
                range
                    .end
                    .checked_sub(range.start)
                    .and_then(|length| total.checked_add(length))
            })
            .ok_or_else(|| object_error(OwnedLocalStoreError::Bound("read ranges")))?;
        let ticket = self.read_ticket(location, total).map_err(object_error)?;
        let outcome = async {
            // The native backend clones all range descriptors and builds a Bytes vector even
            // for zero-length ranges. Each returned Bytes::from_owner adds its own allocation;
            // keep that bookkeeping charged through the shared owners, not just payload bytes.
            let per_range = std::mem::size_of::<Range<u64>>()
                + 2 * std::mem::size_of::<Bytes>()
                + std::mem::size_of::<ReadBytes>()
                + std::mem::size_of::<std::sync::atomic::AtomicUsize>()
                + 64;
            let bookkeeping = ranges
                .len()
                .checked_mul(per_range)
                .ok_or_else(|| object_error(OwnedLocalStoreError::Bound("range bookkeeping")))?;
            ticket
                .charge
                .reservation
                .lock()
                .map_err(|_| object_error(OwnedLocalStoreError::CensusUnavailable))?
                .try_grow(memory(usize_u64(bookkeeping).map_err(object_error)?))
                .map_err(object_error)?;
            let bytes = self.inner.backend.get_ranges(location, ranges).await?;
            let bytes = bytes
                .into_iter()
                .map(|bytes| {
                    Bytes::from_owner(ReadBytes {
                        bytes,
                        _charge: Arc::clone(&ticket.charge),
                    })
                })
                .collect();
            Ok(bytes)
        }
        .await;
        self.complete_read(&ticket).map_err(object_error)?;
        outcome
    }

    fn delete_stream(
        &self,
        locations: BoxStream<'static, object_store::Result<Path>>,
    ) -> BoxStream<'static, object_store::Result<Path>> {
        let store = self.clone();
        locations
            .then(move |location| {
                let store = store.clone();
                async move {
                    let location = location?;
                    store.validate_mutation(&location).map_err(object_error)?;
                    store.inner.backend.delete(&location).await?;
                    Ok(location)
                }
            })
            .boxed()
    }

    fn list(&self, prefix: Option<&Path>) -> BoxStream<'static, object_store::Result<ObjectMeta>> {
        let store = self.clone();
        let prefix = prefix.cloned();
        futures::stream::once(async move {
            let prefix = prefix.ok_or_else(|| object_error(OwnedLocalStoreError::DeniedPath))?;
            let ticket = store.read_ticket(&prefix, 0).map_err(object_error)?;
            if let Err(error) = store.prepare_list(&ticket, &prefix, true) {
                store.complete_read(&ticket).map_err(object_error)?;
                return Err(object_error(error));
            }
            let input = store.inner.backend.list(Some(&prefix));
            let maximum = store.inner.limits.max_list_entries;
            let result = futures::stream::try_unfold(
                (store, input, ticket, 0_usize, 0_u64),
                move |(store, mut input, ticket, count, retained)| async move {
                    let next = match input.try_next().await {
                        Ok(next) => next,
                        Err(error) => {
                            drop(input);
                            store.complete_read(&ticket).map_err(object_error)?;
                            return Err(error);
                        }
                    };
                    match next {
                        None => {
                            drop(input);
                            store.finish_list(&ticket, retained).map_err(object_error)?;
                            Ok(None)
                        }
                        Some(meta) => {
                            if count >= maximum {
                                return Err(object_error(OwnedLocalStoreError::Bound(
                                    "list entries",
                                )));
                            }
                            store.validate_meta(&meta).map_err(object_error)?;
                            let retained = retained
                                .checked_add(
                                    metadata_capacity(1, meta.location.as_ref().len())
                                        .map_err(object_error)?,
                                )
                                .ok_or_else(|| {
                                    object_error(OwnedLocalStoreError::Bound("list metadata"))
                                })?;
                            Ok(Some((meta, (store, input, ticket, count + 1, retained))))
                        }
                    }
                },
            )
            .boxed();
            Ok::<_, object_store::Error>(result)
        })
        .try_flatten()
        .boxed()
    }

    async fn list_with_delimiter(&self, prefix: Option<&Path>) -> object_store::Result<ListResult> {
        let prefix = prefix.ok_or_else(|| object_error(OwnedLocalStoreError::DeniedPath))?;
        let ticket = self.read_ticket(prefix, 0).map_err(object_error)?;
        let outcome = async {
            self.prepare_list(&ticket, prefix, false)
                .map_err(object_error)?;
            let result = self.inner.backend.list_with_delimiter(Some(prefix)).await?;
            if result
                .objects
                .len()
                .checked_add(result.common_prefixes.len())
                .is_none_or(|count| count > self.inner.limits.max_list_entries)
            {
                return Err(object_error(OwnedLocalStoreError::Bound("list entries")));
            }
            let mut retained = 0_u64;
            for meta in &result.objects {
                self.validate_meta(meta).map_err(object_error)?;
                retained = retained
                    .checked_add(
                        metadata_capacity(1, meta.location.as_ref().len()).map_err(object_error)?,
                    )
                    .ok_or_else(|| object_error(OwnedLocalStoreError::Bound("list metadata")))?;
            }
            for prefix in &result.common_prefixes {
                self.validate_path(&*self.state().map_err(object_error)?, prefix)
                    .map_err(object_error)?;
                retained = retained
                    .checked_add(metadata_capacity(1, prefix.as_ref().len()).map_err(object_error)?)
                    .ok_or_else(|| object_error(OwnedLocalStoreError::Bound("list metadata")))?;
            }
            self.finish_list(&ticket, retained).map_err(object_error)?;
            Ok(result)
        }
        .await;
        if outcome.is_err() {
            self.complete_read(&ticket).map_err(object_error)?;
        }
        outcome
    }

    async fn copy_opts(
        &self,
        from: &Path,
        to: &Path,
        options: CopyOptions,
    ) -> object_store::Result<()> {
        self.validate_path(&*self.state().map_err(object_error)?, from)
            .map_err(object_error)?;
        let size = self.inner.backend.head(from).await?.size;
        self.admit_mutation(to, size).map_err(object_error)?;
        self.inner.backend.copy_opts(from, to, options).await
    }

    async fn rename_opts(
        &self,
        from: &Path,
        to: &Path,
        options: RenameOptions,
    ) -> object_store::Result<()> {
        self.validate_path(&*self.state().map_err(object_error)?, from)
            .map_err(object_error)?;
        let size = self.inner.backend.head(from).await?.size;
        self.admit_mutation(to, size).map_err(object_error)?;
        self.inner.backend.rename_opts(from, to, options).await
    }
}

struct UploadOwner {
    native: tokio::sync::Mutex<NativeUpload>,
    parts: Mutex<UploadParts>,
}
struct NativeUpload {
    upload: Option<Box<dyn MultipartUpload>>,
    terminal: bool,
}
struct UploadParts {
    count: usize,
    bytes: u64,
    unfinished: usize,
}
struct OwnedUpload {
    store: OwnedLocalStore,
    location: Path,
    owner: Arc<UploadOwner>,
}
impl fmt::Debug for OwnedUpload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OwnedLocalMultipartUpload")
    }
}

#[async_trait]
impl MultipartUpload for OwnedUpload {
    fn put_part(&mut self, payload: PutPayload) -> UploadPart {
        let preparation = (|| {
            let size = usize_u64(payload.content_length())?;
            let mut parts = self
                .owner
                .parts
                .lock()
                .map_err(|_| OwnedLocalStoreError::UploadState)?;
            let bytes = parts
                .bytes
                .checked_add(size)
                .ok_or(OwnedLocalStoreError::Bound("multipart bytes"))?;
            if bytes > self.store.inner.limits.max_object_bytes
                || parts.count >= self.store.inner.limits.max_multipart_parts
            {
                return Err(OwnedLocalStoreError::Bound("multipart bytes/parts"));
            }
            self.store.admit_mutation(&self.location, size)?;
            let payload = self.store.owned_payload(&self.location, payload)?;
            let mut native = self
                .owner
                .native
                .try_lock()
                .map_err(|_| OwnedLocalStoreError::UploadState)?;
            if native.terminal {
                return Err(OwnedLocalStoreError::UploadState);
            }
            let future = native
                .upload
                .as_mut()
                .ok_or(OwnedLocalStoreError::UploadState)?
                .put_part(payload);
            parts.bytes = bytes;
            parts.count += 1;
            parts.unfinished += 1;
            Ok(future)
        })();
        let owner = Arc::clone(&self.owner);
        async move {
            let future = preparation.map_err(object_error)?;
            let result = future.await;
            let mut parts = owner
                .parts
                .lock()
                .map_err(|_| object_error(OwnedLocalStoreError::UploadState))?;
            parts.unfinished -= 1;
            result
        }
        .boxed()
    }

    async fn complete(&mut self) -> object_store::Result<PutResult> {
        self.store
            .validate_mutation(&self.location)
            .map_err(object_error)?;
        if self
            .owner
            .parts
            .lock()
            .map_err(|_| object_error(OwnedLocalStoreError::UploadState))?
            .unfinished
            != 0
        {
            return Err(object_error(OwnedLocalStoreError::UploadState));
        }
        let mut native = self.owner.native.lock().await;
        if native.terminal {
            return Err(object_error(OwnedLocalStoreError::UploadState));
        }
        let result = native
            .upload
            .as_mut()
            .ok_or_else(|| object_error(OwnedLocalStoreError::UploadState))?
            .complete()
            .await;
        native.terminal = true;
        result
    }

    async fn abort(&mut self) -> object_store::Result<()> {
        self.store
            .validate_mutation(&self.location)
            .map_err(object_error)?;
        let mut native = self.owner.native.lock().await;
        if native.terminal {
            return Err(object_error(OwnedLocalStoreError::UploadState));
        }
        let result = native
            .upload
            .as_mut()
            .ok_or_else(|| object_error(OwnedLocalStoreError::UploadState))?
            .abort()
            .await;
        native.terminal = true;
        result
    }
}

fn current_runtime() -> Result<RuntimeId, OwnedLocalStoreError> {
    tokio::runtime::Handle::try_current()
        .map(|handle| handle.id())
        .map_err(|_| OwnedLocalStoreError::MutationRuntime)
}
fn memory(bytes: u64) -> ResourceAmounts {
    ResourceAmounts {
        memory_bytes: bytes,
        ..ResourceAmounts::default()
    }
}
fn disk(bytes: u64) -> ResourceAmounts {
    ResourceAmounts {
        disk_bytes: bytes,
        ..ResourceAmounts::default()
    }
}
fn usize_u64(value: usize) -> Result<u64, OwnedLocalStoreError> {
    u64::try_from(value).map_err(|_| OwnedLocalStoreError::Bound("byte arithmetic"))
}
fn metadata_capacity(entries: usize, path_bytes: usize) -> Result<u64, OwnedLocalStoreError> {
    // Native local e-tags are bounded stat-derived values; paths can be percent encoded.
    // Include Vec growth, BTreeSet nodes and both native and returned metadata values.
    entries
        .checked_mul(
            path_bytes
                .checked_mul(6)
                .and_then(|bytes| bytes.checked_add(512))
                .ok_or(OwnedLocalStoreError::Bound("list metadata"))?,
        )
        .ok_or(OwnedLocalStoreError::Bound("list metadata"))
        .and_then(usize_u64)
}

fn rounded(bytes: u64, unit: u64) -> Result<u64, OwnedLocalStoreError> {
    bytes
        .max(1)
        .checked_add(unit - 1)
        .map(|bytes| bytes / unit)
        .and_then(|units| units.checked_mul(unit))
        .ok_or(OwnedLocalStoreError::Bound("disk rounding"))
}
fn disk_reservation(state: &mut StoreState, class: ResourceClass) -> &mut ResourceReservation {
    match class {
        ResourceClass::Data => &mut state.data_disk,
        ResourceClass::Control => &mut state.control_disk,
    }
}
fn resize_disk(
    reservation: &mut ResourceReservation,
    size: u64,
) -> Result<(), OwnedLocalStoreError> {
    let current = reservation.amounts().disk_bytes;
    if size > current {
        reservation.try_grow(disk(size - current))?;
    } else {
        reservation.shrink(disk(current - size))?;
    }
    Ok(())
}
fn directory_flags() -> OFlags {
    OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::NOFOLLOW
}
fn validate_root(root: &RootBinding) -> Result<(), OwnedLocalStoreError> {
    let descriptor = open_absolute_directory_nofollow(&root.path).map_err(physical)?;
    let observed = fstat(&descriptor).map_err(physical)?;
    if (observed.st_dev, observed.st_ino) != root.identity {
        return Err(OwnedLocalStoreError::ChangedPath);
    }
    Ok(())
}

#[derive(Default)]
struct PhysicalCensus {
    bytes: u64,
    files: usize,
    directories: usize,
    data_bytes: u64,
    control_bytes: u64,
    maximum_path_bytes: usize,
}
fn census_directory(
    descriptor: &OwnedFd,
    depth: usize,
    path_bytes: usize,
    limits: &OwnedLocalStoreLimits,
    unit: u64,
    census: &mut PhysicalCensus,
) -> Result<(), OwnedLocalStoreError> {
    if depth > limits.max_directory_depth {
        return Err(OwnedLocalStoreError::Bound("census depth"));
    }
    if path_bytes > limits.max_path_bytes {
        return Err(OwnedLocalStoreError::Bound("census path"));
    }
    census.maximum_path_bytes = census.maximum_path_bytes.max(path_bytes);
    let before = fstat(descriptor).map_err(physical)?;
    census.directories += 1;
    let allocated = u64::try_from(before.st_blocks)
        .map_err(physical)?
        .checked_mul(512)
        .ok_or(OwnedLocalStoreError::Bound("allocated bytes"))?;
    census.bytes = census
        .bytes
        .checked_add(allocated.max(unit))
        .ok_or(OwnedLocalStoreError::Bound("census bytes"))?;
    let copied = openat(descriptor, ".", directory_flags(), Mode::empty()).map_err(physical)?;
    let mut directory = Dir::new(copied).map_err(physical)?;
    while let Some(entry) = directory.read() {
        let entry = entry.map_err(physical)?;
        let name = entry.file_name().to_bytes();
        if matches!(name, b"." | b"..") {
            continue;
        }
        let child_path_bytes = path_bytes
            .checked_add(name.len())
            .and_then(|bytes| bytes.checked_add(1))
            .ok_or(OwnedLocalStoreError::Bound("census path"))?;
        if child_path_bytes > limits.max_path_bytes
            || census.files + census.directories >= limits.max_list_entries
        {
            return Err(OwnedLocalStoreError::Bound("census entries/path"));
        }
        census.maximum_path_bytes = census.maximum_path_bytes.max(child_path_bytes);
        let stat =
            statat(descriptor, entry.file_name(), AtFlags::SYMLINK_NOFOLLOW).map_err(physical)?;
        let kind = FileType::from_raw_mode(stat.st_mode);
        if kind.is_dir() {
            let child = openat(
                descriptor,
                entry.file_name(),
                directory_flags(),
                Mode::empty(),
            )
            .map_err(physical)?;
            let opened = fstat(&child).map_err(physical)?;
            if opened.st_dev != stat.st_dev
                || opened.st_ino != stat.st_ino
                || opened.st_dev != before.st_dev
            {
                return Err(OwnedLocalStoreError::ChangedPath);
            }
            census_directory(&child, depth + 1, child_path_bytes, limits, unit, census)?;
        } else if kind.is_file() {
            census.files += 1;
            let blocks = u64::try_from(stat.st_blocks)
                .map_err(physical)?
                .checked_mul(512)
                .ok_or(OwnedLocalStoreError::Bound("allocated bytes"))?;
            let length = u64::try_from(stat.st_size).map_err(physical)?;
            census.bytes = census
                .bytes
                .checked_add(blocks.max(rounded(length, unit)?))
                .ok_or(OwnedLocalStoreError::Bound("census bytes"))?;
        } else {
            return Err(OwnedLocalStoreError::ChangedPath);
        }
    }
    let after = fstat(descriptor.as_fd()).map_err(physical)?;
    if (
        before.st_mtime,
        before.st_mtime_nsec,
        before.st_ctime,
        before.st_ctime_nsec,
    ) != (
        after.st_mtime,
        after.st_mtime_nsec,
        after.st_ctime,
        after.st_ctime_nsec,
    ) {
        return Err(OwnedLocalStoreError::ChangedPath);
    }
    Ok(())
}

#[cfg(test)]
#[path = "owned_local_store_tests.rs"]
mod tests;
