//! Complete metadata witness of the Linux sandbox's mounted runtime. This detects ordinary
//! trusted-host deployment changes; it is not an immutable image or a content/replay authority.

use std::ffi::{CStr, CString};
use std::io;
use std::os::fd::AsFd;
use std::path::Path;
use std::time::{Duration, Instant};

use rustix::fs::{
    AtFlags, Dir, FileType, Mode, OFlags, Stat, fstat, open, openat, readlinkat_raw, statat,
};

use super::frame;
use crate::cancellation::Cancellation;
use crate::resource_budget::{ResourceAmounts, ResourceBudget};

const MAX_ENTRIES: usize = 4_000_000;
const MAX_DEPTH: usize = 128;
const MAX_DURATION: Duration = Duration::from_secs(120);

pub(super) fn observe(
    budget: &ResourceBudget,
    cancellation: &Cancellation,
) -> io::Result<[u8; 32]> {
    observe_root(Path::new("/usr"), budget, cancellation)
}

fn observe_root(
    root: &Path,
    budget: &ResourceBudget,
    cancellation: &Cancellation,
) -> io::Result<[u8; 32]> {
    let mut walk = Walk {
        budget,
        cancellation,
        started: Instant::now(),
        entries: 0,
    };
    walk.check(0)?;
    // No descendant is opened by a host path. A renamed/replaced ancestor cannot redirect
    // traversal, and symlinks are observed as raw link text, never followed outside the mount.
    let descriptor = open(
        root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    let before = fstat(&descriptor)?;
    let mut hash = blake3::Hasher::new();
    hash.update(b"codefabric.linux-runtime-metadata-observation.v1\0");
    walk.directory(&descriptor, &before, 0, &mut hash)?;
    if stamp(&before)
        != stamp(&rustix::fs::statat(
            rustix::fs::CWD,
            root,
            AtFlags::SYMLINK_NOFOLLOW,
        )?)
    {
        return Err(changed());
    }
    tracing::debug!(
        entries = walk.entries,
        elapsed_millis = walk.started.elapsed().as_millis(),
        "observed mounted native runtime"
    );
    Ok(*hash.finalize().as_bytes())
}

struct Walk<'a> {
    budget: &'a ResourceBudget,
    cancellation: &'a Cancellation,
    started: Instant,
    entries: usize,
}

impl Walk<'_> {
    fn check(&self, depth: usize) -> io::Result<()> {
        if self.cancellation.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "runtime observation cancelled",
            ));
        }
        if self.entries >= MAX_ENTRIES || depth > MAX_DEPTH || self.started.elapsed() > MAX_DURATION
        {
            return Err(io::Error::other("runtime observation bound exceeded"));
        }
        Ok(())
    }

    fn directory(
        &mut self,
        fd: impl AsFd,
        before: &Stat,
        depth: usize,
        hash: &mut blake3::Hasher,
    ) -> io::Result<()> {
        self.check(depth)?;
        frame(hash, &stamp(before));
        let (names, _names_memory) = self.names(&fd)?;
        for name in names {
            self.check(depth)?;
            frame(hash, name.to_bytes());
            let before = statat(&fd, &name, AtFlags::SYMLINK_NOFOLLOW)?;
            match FileType::from_raw_mode(before.st_mode) {
                FileType::Directory => {
                    let child = openat(
                        &fd,
                        &name,
                        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                        Mode::empty(),
                    )?;
                    if stamp(&before) != stamp(&fstat(&child)?) {
                        return Err(changed());
                    }
                    self.directory(&child, &before, depth + 1, hash)?;
                }
                FileType::Symlink => {
                    frame(hash, &stamp(&before));
                    let mut target = [0_u8; 4097];
                    let length = readlinkat_raw(&fd, &name, &mut target[..])?;
                    if length == target.len() {
                        return Err(io::Error::other("runtime link target bound exceeded"));
                    }
                    frame(hash, &target[..length]);
                }
                // Never open a regular file, FIFO, device or socket for metadata observation.
                _ => frame(hash, &stamp(&before)),
            }
            if stamp(&before) != stamp(&statat(&fd, &name, AtFlags::SYMLINK_NOFOLLOW)?) {
                return Err(changed());
            }
        }
        if stamp(before) != stamp(&fstat(&fd)?) {
            return Err(changed());
        }
        frame(hash, b"directory-end");
        Ok(())
    }

    fn names(
        &mut self,
        fd: impl AsFd,
    ) -> io::Result<(Vec<CString>, crate::resource_budget::ResourceReservation)> {
        let _iteration = crate::inventory::reserve_memory(
            self.budget,
            crate::secure_path::DIRECTORY_ITERATION_MEMORY_BOUND as u64,
        )
        .map_err(io::Error::other)?;
        let mut memory =
            crate::inventory::reserve_memory(self.budget, 4096).map_err(io::Error::other)?;
        let mut names = Vec::new();
        for entry in Dir::read_from(fd)? {
            self.check(0)?;
            let entry = entry?;
            let name: &CStr = entry.file_name();
            if matches!(name.to_bytes(), b"." | b"..") {
                continue;
            }
            self.entries += 1;
            memory
                .try_grow(ResourceAmounts {
                    memory_bytes: name.to_bytes().len() as u64 + 64,
                    ..Default::default()
                })
                .map_err(io::Error::other)?;
            names.push(name.to_owned());
        }
        names.sort_unstable();
        Ok((names, memory))
    }
}

fn changed() -> io::Error {
    io::Error::other("native runtime changed during observation")
}

fn stamp(value: &Stat) -> [u8; 192] {
    // atime is deliberately excluded: observing/using a runtime must not invalidate it.
    // ctime catches same-size in-place writes even when the deployer restores mtime.
    let fields: [i128; 12] = [
        value.st_dev.into(),
        value.st_ino.into(),
        value.st_nlink.into(),
        value.st_mode.into(),
        value.st_uid.into(),
        value.st_gid.into(),
        value.st_rdev.into(),
        value.st_size.into(),
        value.st_mtime.into(),
        value.st_mtime_nsec.into(),
        value.st_ctime.into(),
        value.st_ctime_nsec.into(),
    ];
    let mut bytes = [0; 192];
    for (chunk, field) in bytes.chunks_exact_mut(16).zip(fields) {
        chunk.copy_from_slice(&field.to_be_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::{
        ffi::OsStrExt as _,
        fs::{PermissionsExt as _, symlink},
    };

    #[test]
    fn runtime_observation_tracks_negative_raw_and_mode_changes_without_following_links() {
        let fixture = tempfile::tempdir().unwrap();
        let budget = crate::fabric::workspace_resources::test_workspace_budget();
        let root = fixture.path().join("usr");
        std::fs::create_dir(&root).unwrap();
        let observe = || observe_root(&root, &budget, &Cancellation::default()).unwrap();
        let absent = observe();
        let path = root.join(std::ffi::OsStr::from_bytes(b"library-\xff"));
        std::fs::write(&path, b"old").unwrap();
        let first = observe();
        assert_ne!(absent, first);
        assert_eq!(first, observe());
        std::fs::read(&path).unwrap();
        assert_eq!(first, observe(), "access time is not a deployment change");
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::fs::write(&path, b"new").unwrap();
        std::fs::File::open(&path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        let changed = observe();
        assert_ne!(first, changed);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_ne!(changed, observe());
        symlink("cycle", root.join("cycle")).unwrap();
        symlink("../outside", root.join("link")).unwrap();
        let linked = observe();
        std::fs::write(fixture.path().join("outside"), b"not mounted").unwrap();
        assert_eq!(
            linked,
            observe(),
            "outside link target is not a runtime input"
        );
        std::fs::remove_file(root.join("link")).unwrap();
        symlink("../another", root.join("link")).unwrap();
        assert_ne!(linked, observe());
        let before_removal = observe();
        std::fs::remove_file(path).unwrap();
        assert_ne!(before_removal, observe());
        assert_eq!(budget.observation().used.memory_bytes, 0);
        let cancelled = Cancellation::default();
        cancelled.cancel();
        assert_eq!(
            observe_root(&root, &budget, &cancelled).unwrap_err().kind(),
            io::ErrorKind::Interrupted
        );
    }

    #[test]
    fn runtime_observation_does_not_open_special_files_or_follow_a_root_symlink() {
        let fixture = tempfile::tempdir().unwrap();
        let budget = crate::fabric::workspace_resources::test_workspace_budget();
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            fixture.path().join("pipe"),
            Mode::RUSR | Mode::WUSR,
        )
        .unwrap();
        observe_root(fixture.path(), &budget, &Cancellation::default()).unwrap();
        let alias = fixture.path().join("alias");
        symlink(fixture.path(), &alias).unwrap();
        assert!(observe_root(&alias, &budget, &Cancellation::default()).is_err());
    }
}
