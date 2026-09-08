//! Replacement-safe ownership for every supervisor, admin, and gRPC Unix endpoint.

use std::ffi::OsString;
use std::io;
use std::os::fd::{AsFd as _, AsRawFd as _, OwnedFd};
use std::os::unix::net::{UnixListener as StdUnixListener, UnixStream as StdUnixStream};
use std::path::{Component, Path, PathBuf};

#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _, PermissionsExt as _};

use rustix::fs::{
    AtFlags, FileType, Mode, OFlags, chmodat, fstat, fsync, openat, statat, unlinkat,
};
use thiserror::Error;
use tokio::net::UnixListener;

use crate::secure_path::open_absolute_directory_nofollow;

/// A bound socket may remove only the exact device/inode/generation it created.
#[derive(Debug)]
pub struct OwnedUnixSocket {
    root: PathBuf,
    path: PathBuf,
    name: OsString,
    root_descriptor: OwnedFd,
    root_device: u64,
    root_inode: u64,
    parent_descriptor: OwnedFd,
    parent_device: u64,
    parent_inode: u64,
    device: u64,
    inode: u64,
    change_time_seconds: i64,
    change_time_nanoseconds: u64,
    owner_uid: u32,
    generation: u64,
    retired: bool,
}

impl OwnedUnixSocket {
    /// Bind beneath an already-private runtime root, replacing only a verified stale socket.
    pub fn bind(
        root: &Path,
        path: &Path,
        generation: u64,
    ) -> Result<(UnixListener, Self), OwnedUnixSocketError> {
        let root_descriptor = open_root_authority(root)?;
        Self::bind_with_owned_root(root, path, generation, root_descriptor)
    }

    /// Bind beneath a runtime-root descriptor already retained by the singleton lease.
    pub(crate) fn bind_with_root_descriptor(
        root: &Path,
        path: &Path,
        generation: u64,
        root_authority: &OwnedFd,
    ) -> Result<(UnixListener, Self), OwnedUnixSocketError> {
        let root_descriptor = root_authority
            .as_fd()
            .try_clone_to_owned()
            .map_err(|source| OwnedUnixSocketError::Io {
                path: root.to_owned(),
                source,
            })?;
        validate_root_descriptor(root, &root_descriptor)?;
        Self::bind_with_owned_root(root, path, generation, root_descriptor)
    }

    fn bind_with_owned_root(
        root: &Path,
        path: &Path,
        generation: u64,
        root_descriptor: OwnedFd,
    ) -> Result<(UnixListener, Self), OwnedUnixSocketError> {
        if generation == 0 {
            return Err(OwnedUnixSocketError::InvalidGeneration);
        }
        let root_stat = fstat(&root_descriptor).map_err(|source| OwnedUnixSocketError::Io {
            path: root.to_owned(),
            source: source.into(),
        })?;
        let (parent_descriptor, parent_stat, name) =
            open_target_parent(&root_descriptor, &root_stat, root, path)?;
        let authority_path = descriptor_entry_path(&parent_descriptor, &name);
        match statat(&parent_descriptor, &name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(metadata) => {
                validate_stale_candidate(path, &metadata)?;
                match StdUnixStream::connect(&authority_path) {
                    Ok(_) => return Err(OwnedUnixSocketError::Live(path.to_owned())),
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                        ) => {}
                    Err(source) => {
                        return Err(OwnedUnixSocketError::Io {
                            path: path.to_owned(),
                            source,
                        });
                    }
                }
                let current = statat(&parent_descriptor, &name, AtFlags::SYMLINK_NOFOLLOW)
                    .map_err(|source| OwnedUnixSocketError::Io {
                        path: path.to_owned(),
                        source: source.into(),
                    })?;
                if current.st_dev != metadata.st_dev
                    || current.st_ino != metadata.st_ino
                    || current.st_ctime != metadata.st_ctime
                    || current.st_ctime_nsec != metadata.st_ctime_nsec
                    || current.st_uid != metadata.st_uid
                    || !FileType::from_raw_mode(current.st_mode).is_socket()
                {
                    return Err(OwnedUnixSocketError::Replacement(path.to_owned()));
                }
                unlinkat(&parent_descriptor, &name, AtFlags::empty()).map_err(|source| {
                    OwnedUnixSocketError::Io {
                        path: path.to_owned(),
                        source: source.into(),
                    }
                })?;
                sync_directory(&parent_descriptor, path)?;
            }
            Err(rustix::io::Errno::NOENT) => {}
            Err(source) => {
                return Err(OwnedUnixSocketError::Io {
                    path: path.to_owned(),
                    source: source.into(),
                });
            }
        }
        revalidate_directory_authority(
            root,
            &root_descriptor,
            root_stat.st_dev,
            root_stat.st_ino,
            path,
            &parent_descriptor,
            parent_stat.st_dev,
            parent_stat.st_ino,
        )?;
        let standard_listener =
            StdUnixListener::bind(&authority_path).map_err(|source| OwnedUnixSocketError::Io {
                path: path.to_owned(),
                source,
            })?;
        chmodat(
            &parent_descriptor,
            &name,
            Mode::RUSR | Mode::WUSR,
            AtFlags::empty(),
        )
        .map_err(|source| OwnedUnixSocketError::Io {
            path: path.to_owned(),
            source: source.into(),
        })?;
        standard_listener
            .set_nonblocking(true)
            .map_err(|source| OwnedUnixSocketError::Io {
                path: path.to_owned(),
                source,
            })?;
        let listener = UnixListener::from_std(standard_listener).map_err(|source| {
            OwnedUnixSocketError::Io {
                path: path.to_owned(),
                source,
            }
        })?;
        let metadata =
            statat(&parent_descriptor, &name, AtFlags::SYMLINK_NOFOLLOW).map_err(|source| {
                OwnedUnixSocketError::Io {
                    path: path.to_owned(),
                    source: source.into(),
                }
            })?;
        let owner_uid = rustix::process::geteuid().as_raw();
        if !FileType::from_raw_mode(metadata.st_mode).is_socket()
            || metadata.st_uid != owner_uid
            || metadata.st_mode & 0o777 != 0o600
            || metadata.st_dev != parent_stat.st_dev
        {
            return Err(OwnedUnixSocketError::UnsafeSocket(path.to_owned()));
        }
        sync_directory(&parent_descriptor, path)?;
        Ok((
            listener,
            Self {
                root: root.to_owned(),
                path: path.to_owned(),
                name,
                root_descriptor,
                root_device: root_stat.st_dev,
                root_inode: root_stat.st_ino,
                parent_descriptor,
                parent_device: parent_stat.st_dev,
                parent_inode: parent_stat.st_ino,
                device: metadata.st_dev,
                inode: metadata.st_ino,
                change_time_seconds: metadata.st_ctime,
                change_time_nanoseconds: metadata.st_ctime_nsec,
                owner_uid,
                generation,
                retired: false,
            },
        ))
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn retire(&mut self) -> Result<(), OwnedUnixSocketError> {
        if self.retired {
            return Ok(());
        }
        self.revalidate_authority()?;
        match statat(
            &self.parent_descriptor,
            &self.name,
            AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Ok(metadata)
                if self.matches(&metadata)
                    && FileType::from_raw_mode(metadata.st_mode).is_socket() =>
            {
                unlinkat(&self.parent_descriptor, &self.name, AtFlags::empty()).map_err(
                    |source| OwnedUnixSocketError::Io {
                        path: self.path.clone(),
                        source: source.into(),
                    },
                )?;
                sync_directory(&self.parent_descriptor, &self.path)?;
                self.retired = true;
                Ok(())
            }
            Err(rustix::io::Errno::NOENT) => {
                self.retired = true;
                Ok(())
            }
            _ => Err(OwnedUnixSocketError::Replacement(self.path.clone())),
        }
    }

    fn matches(&self, metadata: &rustix::fs::Stat) -> bool {
        metadata.st_dev == self.device
            && metadata.st_ino == self.inode
            && metadata.st_ctime == self.change_time_seconds
            && metadata.st_ctime_nsec == self.change_time_nanoseconds
            && metadata.st_uid == self.owner_uid
    }

    fn revalidate_authority(&self) -> Result<(), OwnedUnixSocketError> {
        revalidate_directory_authority(
            &self.root,
            &self.root_descriptor,
            self.root_device,
            self.root_inode,
            &self.path,
            &self.parent_descriptor,
            self.parent_device,
            self.parent_inode,
        )
    }
}

impl Drop for OwnedUnixSocket {
    fn drop(&mut self) {
        if self.retired {
            return;
        }
        if self.revalidate_authority().is_ok()
            && let Ok(metadata) = statat(
                &self.parent_descriptor,
                &self.name,
                AtFlags::SYMLINK_NOFOLLOW,
            )
            && FileType::from_raw_mode(metadata.st_mode).is_socket()
            && self.matches(&metadata)
        {
            let _ = unlinkat(&self.parent_descriptor, &self.name, AtFlags::empty());
            let _ = sync_directory(&self.parent_descriptor, &self.path);
        }
    }
}

fn open_root_authority(root: &Path) -> Result<OwnedFd, OwnedUnixSocketError> {
    let descriptor = open_absolute_directory_nofollow(root)
        .map_err(|_| OwnedUnixSocketError::UnsafeRoot(root.to_owned()))?;
    validate_root_descriptor(root, &descriptor)?;
    Ok(descriptor)
}

fn validate_root_descriptor(root: &Path, descriptor: &OwnedFd) -> Result<(), OwnedUnixSocketError> {
    let metadata = fstat(descriptor).map_err(|source| OwnedUnixSocketError::Io {
        path: root.to_owned(),
        source: source.into(),
    })?;
    if !FileType::from_raw_mode(metadata.st_mode).is_dir()
        || metadata.st_uid != rustix::process::geteuid().as_raw()
        || metadata.st_mode & 0o777 != 0o700
    {
        return Err(OwnedUnixSocketError::UnsafeRoot(root.to_owned()));
    }
    Ok(())
}

fn open_target_parent(
    root_descriptor: &OwnedFd,
    root_stat: &rustix::fs::Stat,
    root: &Path,
    path: &Path,
) -> Result<(OwnedFd, rustix::fs::Stat, OsString), OwnedUnixSocketError> {
    if !path.is_absolute()
        || !path.starts_with(root)
        || path == root
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(OwnedUnixSocketError::UnsafeSocket(path.to_owned()));
    }
    let parent = path
        .parent()
        .ok_or_else(|| OwnedUnixSocketError::UnsafeSocket(path.to_owned()))?;
    let relative = parent
        .strip_prefix(root)
        .map_err(|_| OwnedUnixSocketError::UnsafeSocket(path.to_owned()))?;
    let mut descriptor = root_descriptor
        .as_fd()
        .try_clone_to_owned()
        .map_err(|source| OwnedUnixSocketError::Io {
            path: root.to_owned(),
            source,
        })?;
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(OwnedUnixSocketError::UnsafeSocket(path.to_owned()));
        };
        let next = openat(
            &descriptor,
            component,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY,
            Mode::empty(),
        )
        .map_err(|source| OwnedUnixSocketError::Io {
            path: path.to_owned(),
            source: source.into(),
        })?;
        let metadata = fstat(&next).map_err(|source| OwnedUnixSocketError::Io {
            path: path.to_owned(),
            source: source.into(),
        })?;
        if !FileType::from_raw_mode(metadata.st_mode).is_dir()
            || metadata.st_uid != rustix::process::geteuid().as_raw()
            || metadata.st_mode & 0o077 != 0
            || metadata.st_dev != root_stat.st_dev
        {
            return Err(OwnedUnixSocketError::UnsafeRoot(parent.to_owned()));
        }
        descriptor = next;
    }
    let metadata = fstat(&descriptor).map_err(|source| OwnedUnixSocketError::Io {
        path: parent.to_owned(),
        source: source.into(),
    })?;
    let name = path
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| OwnedUnixSocketError::UnsafeSocket(path.to_owned()))?
        .to_owned();
    Ok((descriptor, metadata, name))
}

fn validate_stale_candidate(
    path: &Path,
    metadata: &rustix::fs::Stat,
) -> Result<(), OwnedUnixSocketError> {
    if !FileType::from_raw_mode(metadata.st_mode).is_socket()
        || metadata.st_uid != rustix::process::geteuid().as_raw()
        || metadata.st_mode & 0o777 != 0o600
    {
        return Err(OwnedUnixSocketError::UnsafeSocket(path.to_owned()));
    }
    Ok(())
}

fn descriptor_entry_path(parent: &OwnedFd, name: &std::ffi::OsStr) -> PathBuf {
    #[cfg(target_os = "linux")]
    let descriptor_root = PathBuf::from(format!("/proc/self/fd/{}", parent.as_raw_fd()));
    #[cfg(not(target_os = "linux"))]
    let descriptor_root = PathBuf::from(format!("/dev/fd/{}", parent.as_raw_fd()));
    descriptor_root.join(name)
}

#[allow(clippy::too_many_arguments)]
fn revalidate_directory_authority(
    root: &Path,
    root_descriptor: &OwnedFd,
    root_device: u64,
    root_inode: u64,
    path: &Path,
    parent_descriptor: &OwnedFd,
    parent_device: u64,
    parent_inode: u64,
) -> Result<(), OwnedUnixSocketError> {
    let owner = rustix::process::geteuid().as_raw();
    let root_stat = fstat(root_descriptor).map_err(|source| OwnedUnixSocketError::Io {
        path: root.to_owned(),
        source: source.into(),
    })?;
    let parent_stat = fstat(parent_descriptor).map_err(|source| OwnedUnixSocketError::Io {
        path: path.to_owned(),
        source: source.into(),
    })?;
    if root_stat.st_dev != root_device
        || root_stat.st_ino != root_inode
        || root_stat.st_uid != owner
        || root_stat.st_mode & 0o777 != 0o700
        || parent_stat.st_dev != parent_device
        || parent_stat.st_ino != parent_inode
        || parent_stat.st_uid != owner
        || parent_stat.st_mode & 0o077 != 0
    {
        return Err(OwnedUnixSocketError::UnsafeRoot(root.to_owned()));
    }
    Ok(())
}

fn sync_directory(directory: &OwnedFd, path: &Path) -> Result<(), OwnedUnixSocketError> {
    fsync(directory).map_err(|source| OwnedUnixSocketError::Io {
        path: path.to_owned(),
        source: source.into(),
    })
}

#[derive(Debug, Error)]
pub enum OwnedUnixSocketError {
    #[error("Unix socket generation is invalid")]
    InvalidGeneration,
    #[error("private runtime root is unsafe: {0}")]
    UnsafeRoot(PathBuf),
    #[error("Unix socket path is unsafe: {0}")]
    UnsafeSocket(PathBuf),
    #[error("Unix socket endpoint is live: {0}")]
    Live(PathBuf),
    #[error("Unix socket path was replaced: {0}")]
    Replacement(PathBuf),
    #[error("Unix socket I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener as StdUnixListener;

    fn private_root() -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("temporary socket root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("private socket-root permissions");
        root
    }

    #[tokio::test]
    async fn wp44_ops_owned_socket_recovers_stale_endpoint_and_retires_exact_inode() {
        let root = private_root();
        let path = root.path().join("query.sock");
        let stale = StdUnixListener::bind(&path).expect("stale socket");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("stale socket permissions");
        drop(stale);

        let (listener, mut owned) =
            OwnedUnixSocket::bind(root.path(), &path, 7).expect("replace verified stale socket");
        assert_eq!(owned.generation(), 7);
        assert_eq!(owned.path(), path);
        let metadata = fs::symlink_metadata(&path).expect("owned socket metadata");
        assert!(metadata.file_type().is_socket());
        assert_eq!(metadata.mode() & 0o777, 0o600);
        drop(listener);
        owned.retire().expect("retire exact socket");
        assert!(!path.exists());
        owned.retire().expect("retirement is idempotent");
    }

    #[tokio::test]
    async fn wp44_neg_live_and_unsafe_socket_candidates_are_never_replaced() {
        let root = private_root();
        let path = root.path().join("supervisor.sock");
        let (_listener, _owned) =
            OwnedUnixSocket::bind(root.path(), &path, 1).expect("first owned socket");
        assert!(matches!(
            OwnedUnixSocket::bind(root.path(), &path, 2),
            Err(OwnedUnixSocketError::Live(candidate)) if candidate == path
        ));

        let unsafe_path = root.path().join("not-a-socket");
        fs::write(&unsafe_path, b"operator data").expect("unsafe candidate");
        fs::set_permissions(&unsafe_path, fs::Permissions::from_mode(0o600))
            .expect("candidate permissions");
        assert!(matches!(
            OwnedUnixSocket::bind(root.path(), &unsafe_path, 3),
            Err(OwnedUnixSocketError::UnsafeSocket(candidate)) if candidate == unsafe_path
        ));
        assert_eq!(
            fs::read(&unsafe_path).expect("candidate preserved"),
            b"operator data"
        );
        assert!(matches!(
            OwnedUnixSocket::bind(root.path(), &root.path().join("zero.sock"), 0),
            Err(OwnedUnixSocketError::InvalidGeneration)
        ));
    }

    #[tokio::test]
    async fn wp44_ops_replacement_inode_is_preserved_during_retire_and_drop() {
        let root = private_root();
        let path = root.path().join("query.sock");
        let (listener, mut owned) =
            OwnedUnixSocket::bind(root.path(), &path, 11).expect("owned socket");
        drop(listener);
        fs::remove_file(&path).expect("unlink owned pathname while descriptor remains closed");
        let replacement = StdUnixListener::bind(&path).expect("replacement socket");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("replacement permissions");
        let replacement_metadata = fs::symlink_metadata(&path).expect("replacement metadata");

        assert!(matches!(
            owned.retire(),
            Err(OwnedUnixSocketError::Replacement(candidate)) if candidate == path
        ));
        drop(owned);
        let observed = fs::symlink_metadata(&path).expect("replacement preserved");
        assert_eq!(observed.dev(), replacement_metadata.dev());
        assert_eq!(observed.ino(), replacement_metadata.ino());
        drop(replacement);
        fs::remove_file(&path).expect("test replacement cleanup");
    }

    #[tokio::test]
    async fn wp44_neg_private_root_and_path_components_fail_closed() {
        let root = private_root();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o755))
            .expect("weaken root permissions");
        let path = root.path().join("unsafe.sock");
        assert!(matches!(
            OwnedUnixSocket::bind(root.path(), &path, 1),
            Err(OwnedUnixSocketError::UnsafeRoot(candidate)) if candidate == root.path()
        ));
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn wp44_neg_runtime_root_ancestor_symlink_and_replacement_cannot_redirect_retirement() {
        use std::os::unix::fs::symlink;

        let base = private_root();
        let real_parent = base.path().join("real-parent");
        let real_root = real_parent.join("runtime");
        fs::create_dir(&real_parent).unwrap();
        fs::create_dir(&real_root).unwrap();
        fs::set_permissions(&real_parent, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(&real_root, fs::Permissions::from_mode(0o700)).unwrap();
        let alias = base.path().join("runtime-alias");
        symlink(&real_root, &alias).unwrap();
        assert!(matches!(
            OwnedUnixSocket::bind(&alias, &alias.join("query.sock"), 1),
            Err(OwnedUnixSocketError::UnsafeRoot(candidate)) if candidate == alias
        ));

        let path = real_root.join("query.sock");
        let (listener, mut owned) =
            OwnedUnixSocket::bind(&real_root, &path, 2).expect("descriptor-owned socket");
        let renamed_root = real_parent.join("runtime-original");
        fs::rename(&real_root, &renamed_root).unwrap();
        fs::create_dir(&real_root).unwrap();
        fs::set_permissions(&real_root, fs::Permissions::from_mode(0o700)).unwrap();
        let substitute = real_root.join("query.sock");
        fs::write(&substitute, b"substitute authority").unwrap();
        fs::set_permissions(&substitute, fs::Permissions::from_mode(0o600)).unwrap();

        drop(listener);
        owned
            .retire()
            .expect("retire through retained original-root descriptor");
        assert!(!renamed_root.join("query.sock").exists());
        assert_eq!(fs::read(&substitute).unwrap(), b"substitute authority");
    }
}
