//! OS-backed single-writer lease paired with durable monotonic writer generations.
//!
//! The kernel lock proves local exclusivity only while this guard is alive. The generation port
//! proves which lease is current at durable backend boundaries and across process restarts. A
//! lock file never acts as semantic current-state authority and its contents are never read.

use std::fs::{self, File};
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use rustix::fs::{FlockOperation, Mode, OFlags, flock, open, openat};
use rustix::io::Errno;
use thiserror::Error;

use super::command::{LeaseId, WorkspaceId, WriterFence, WriterGeneration};

/// Stable failures from an application-owned durable writer-generation store.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum WriterGenerationPortError {
    #[error("durable writer-generation store is unavailable")]
    Unavailable,
    #[error("writer generation is exhausted")]
    Exhausted,
    #[error("durable writer-generation state is contradictory")]
    Corrupt,
}

/// Atomic durable generation allocation and observation.
///
/// Implementations must allocate a value strictly greater than every generation previously
/// committed for the workspace in the same transaction which records the supplied lease ID.
pub trait DurableWriterGenerationPort: Send + Sync {
    fn allocate_next(
        &self,
        workspace_id: WorkspaceId,
        lease_id: LeaseId,
    ) -> Result<WriterGeneration, WriterGenerationPortError>;

    fn observe_current(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Option<WriterFence>, WriterGenerationPortError>;
}

/// Closed failures at lease acquisition and durable-boundary validation.
#[derive(Debug, Error)]
pub enum WorkspaceWriterLeaseError {
    #[error("workspace writer admin directory is unsafe: {0}")]
    UnsafeAdminDirectory(PathBuf),
    #[error("workspace writer lock file is unsafe: {0}")]
    UnsafeLockFile(PathBuf),
    #[error("workspace already has a local writer")]
    AlreadyHeld,
    #[error("workspace writer lease I/O failed: {0}")]
    Io(Errno),
    #[error(transparent)]
    Generation(#[from] WriterGenerationPortError),
    #[error("writer fence is stale or no longer current")]
    StaleFence,
}

/// Exclusive local writer authority. This type is intentionally neither `Clone` nor `Copy`.
pub struct WorkspaceWriterLease {
    workspace_id: WorkspaceId,
    fence: WriterFence,
    lock_path: PathBuf,
    lock: File,
    released: bool,
}

impl std::fmt::Debug for WorkspaceWriterLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WorkspaceWriterLease")
            .field("workspace_id", &self.workspace_id)
            .field("fence", &self.fence)
            .field("lock_path", &self.lock_path)
            .finish_non_exhaustive()
    }
}

impl WorkspaceWriterLease {
    /// Acquire the nonblocking OS lock, then allocate one strictly newer durable generation.
    ///
    /// The caller must supply an already-created private directory. This method refuses symlink,
    /// non-directory, group-accessible, or world-accessible roots and opens the lock file with
    /// `NOFOLLOW` relative to the verified directory descriptor.
    pub fn acquire(
        admin_root: &Path,
        workspace_id: WorkspaceId,
        lease_id: LeaseId,
        generations: &dyn DurableWriterGenerationPort,
    ) -> Result<Self, WorkspaceWriterLeaseError> {
        let directory_metadata = fs::symlink_metadata(admin_root)
            .map_err(|_| WorkspaceWriterLeaseError::UnsafeAdminDirectory(admin_root.into()))?;
        if !directory_metadata.is_dir()
            || directory_metadata.file_type().is_symlink()
            || directory_metadata.permissions().mode() & 0o077 != 0
        {
            return Err(WorkspaceWriterLeaseError::UnsafeAdminDirectory(
                admin_root.into(),
            ));
        }
        let directory = open(
            admin_root,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY,
            Mode::empty(),
        )
        .map_err(WorkspaceWriterLeaseError::Io)?;
        let file_name = lock_file_name(workspace_id);
        let lock_path = admin_root.join(&file_name);
        let descriptor = openat(
            &directory,
            file_name,
            OFlags::RDWR | OFlags::CREATE | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|error| {
            if error == Errno::LOOP {
                WorkspaceWriterLeaseError::UnsafeLockFile(lock_path.clone())
            } else {
                WorkspaceWriterLeaseError::Io(error)
            }
        })?;
        let lock = File::from(descriptor);
        let metadata = lock
            .metadata()
            .map_err(|_| WorkspaceWriterLeaseError::UnsafeLockFile(lock_path.clone()))?;
        if !metadata.is_file()
            || metadata.nlink() != 1
            || metadata.permissions().mode() & 0o777 != 0o600
        {
            return Err(WorkspaceWriterLeaseError::UnsafeLockFile(lock_path));
        }
        flock(&lock, FlockOperation::NonBlockingLockExclusive).map_err(|error| {
            if error == Errno::WOULDBLOCK || error == Errno::AGAIN {
                WorkspaceWriterLeaseError::AlreadyHeld
            } else {
                WorkspaceWriterLeaseError::Io(error)
            }
        })?;

        let generation = generations.allocate_next(workspace_id, lease_id)?;
        let fence = WriterFence {
            lease_id,
            generation,
        };
        validate_writer_fence(generations, workspace_id, fence)?;
        Ok(Self {
            workspace_id,
            fence,
            lock_path,
            lock,
            released: false,
        })
    }

    /// Workspace guarded by the OS lock and durable generation.
    #[must_use]
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    /// Exact lease/generation pair required at each durable effect boundary.
    #[must_use]
    pub const fn fence(&self) -> WriterFence {
        self.fence
    }

    /// Re-read durable generation authority before one backend mutation.
    pub fn validate(
        &self,
        generations: &dyn DurableWriterGenerationPort,
    ) -> Result<(), WorkspaceWriterLeaseError> {
        validate_writer_fence(generations, self.workspace_id, self.fence)
    }

    /// Explicitly release the exact OS writer lock and report the owner outcome.
    ///
    /// # Errors
    ///
    /// Returns the kernel unlock failure. Ordinary drop remains only a partial-construction
    /// safety net and is not successful shutdown evidence.
    pub fn release(mut self) -> Result<(), WorkspaceWriterLeaseError> {
        flock(&self.lock, FlockOperation::Unlock).map_err(WorkspaceWriterLeaseError::Io)?;
        self.released = true;
        Ok(())
    }
}

impl Drop for WorkspaceWriterLease {
    fn drop(&mut self) {
        if !self.released {
            let _ = flock(&self.lock, FlockOperation::Unlock);
        }
    }
}

/// Validate a fence without requiring possession of an in-process guard.
///
/// Durable adapters call this immediately before mutation. It deliberately cannot prove that an
/// OS lock is still held; production callers retain the guard for that independent proof.
pub fn validate_writer_fence(
    generations: &dyn DurableWriterGenerationPort,
    workspace_id: WorkspaceId,
    fence: WriterFence,
) -> Result<(), WorkspaceWriterLeaseError> {
    if generations.observe_current(workspace_id)? == Some(fence) {
        Ok(())
    } else {
        Err(WorkspaceWriterLeaseError::StaleFence)
    }
}

fn lock_file_name(workspace_id: WorkspaceId) -> String {
    let mut name = String::with_capacity(32 + ".writer.lock".len());
    for byte in workspace_id.as_bytes() {
        use std::fmt::Write as _;
        write!(name, "{byte:02x}").expect("writing into a String cannot fail");
    }
    name.push_str(".writer.lock");
    name
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use tempfile::TempDir;

    use super::*;

    #[derive(Default)]
    struct MemoryGenerations {
        current: Mutex<BTreeMap<WorkspaceId, WriterFence>>,
    }

    impl DurableWriterGenerationPort for MemoryGenerations {
        fn allocate_next(
            &self,
            workspace_id: WorkspaceId,
            lease_id: LeaseId,
        ) -> Result<WriterGeneration, WriterGenerationPortError> {
            let mut current = self.current.lock().expect("generation mutex is healthy");
            let next = current
                .get(&workspace_id)
                .map_or(1, |fence| fence.generation.get().saturating_add(1));
            let generation =
                WriterGeneration::new(next).ok_or(WriterGenerationPortError::Exhausted)?;
            current.insert(
                workspace_id,
                WriterFence {
                    lease_id,
                    generation,
                },
            );
            Ok(generation)
        }

        fn observe_current(
            &self,
            workspace_id: WorkspaceId,
        ) -> Result<Option<WriterFence>, WriterGenerationPortError> {
            Ok(self
                .current
                .lock()
                .expect("generation mutex is healthy")
                .get(&workspace_id)
                .copied())
        }
    }

    fn private_root() -> TempDir {
        let root = TempDir::new().expect("temporary writer lease directory");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("make writer lease directory private");
        root
    }

    #[test]
    fn os_lock_and_generation_jointly_fence_one_workspace_writer() {
        let root = private_root();
        let generations = MemoryGenerations::default();
        let workspace = WorkspaceId::from_bytes([1; 16]);
        let first = WorkspaceWriterLease::acquire(
            root.path(),
            workspace,
            LeaseId::from_bytes([2; 16]),
            &generations,
        )
        .expect("first writer acquires lock and generation");
        assert_eq!(first.fence().generation.get(), 1);
        first.validate(&generations).unwrap();

        assert!(matches!(
            WorkspaceWriterLease::acquire(
                root.path(),
                workspace,
                LeaseId::from_bytes([3; 16]),
                &generations,
            ),
            Err(WorkspaceWriterLeaseError::AlreadyHeld)
        ));
        assert_eq!(
            generations
                .observe_current(workspace)
                .unwrap()
                .unwrap()
                .generation
                .get(),
            1,
            "a failed OS-lock acquisition cannot consume a generation"
        );

        let stale = first.fence();
        drop(first);
        let successor = WorkspaceWriterLease::acquire(
            root.path(),
            workspace,
            LeaseId::from_bytes([3; 16]),
            &generations,
        )
        .expect("successor acquires a higher generation");
        assert_eq!(successor.fence().generation.get(), 2);
        assert!(matches!(
            validate_writer_fence(&generations, workspace, stale),
            Err(WorkspaceWriterLeaseError::StaleFence)
        ));
        successor.validate(&generations).unwrap();
    }

    #[test]
    fn unsafe_admin_directory_is_rejected_before_lock_creation() {
        let root = TempDir::new().expect("temporary unsafe lease directory");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o755))
            .expect("make fixture intentionally unsafe");
        let generations = MemoryGenerations::default();
        assert!(matches!(
            WorkspaceWriterLease::acquire(
                root.path(),
                WorkspaceId::from_bytes([4; 16]),
                LeaseId::from_bytes([5; 16]),
                &generations,
            ),
            Err(WorkspaceWriterLeaseError::UnsafeAdminDirectory(_))
        ));
        assert!(fs::read_dir(root.path()).unwrap().next().is_none());
    }
}
