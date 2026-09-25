//! Descriptor-relative immutable capture. Reflinks are native accelerators, never hard links.

use std::fs::{self, File};
use std::io::{self, Read as _, Write as _};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt as _;
use std::path::Path;
use std::time::Instant;

use rustix::fs::{
    AtFlags, FileType, Mode, OFlags, Stat, Timestamps, fchmod, fstat, futimens, openat,
    readlinkat_raw, statat,
};

use super::{check, frame, observation};
use crate::cancellation::Cancellation;
use crate::disk_headroom::LocalDiskHeadroom;
use crate::resource_budget::{ResourceBudget, ResourceClass};

pub(super) struct Captured {
    pub content: [u8; 32],
    pub bytes: u64,
    pub entries: usize,
}

pub(super) fn copy(
    source: &Path,
    destination: &Path,
    budget: &ResourceBudget,
    headroom: &LocalDiskHeadroom,
    cancellation: &Cancellation,
    started: Instant,
    maximum_bytes: u64,
) -> io::Result<Captured> {
    let descriptor =
        crate::secure_path::open_absolute_directory_nofollow(source).map_err(io::Error::other)?;
    let before = fstat(&descriptor)?;
    let _memory =
        crate::inventory::reserve_memory(budget, 1024 * 1024).map_err(io::Error::other)?;
    let mut walker = Capture {
        census: observation::Walk {
            budget,
            cancellation,
            started,
            duration: super::MAX_DURATION,
            entries: 0,
        },
        headroom,
        started,
        maximum_bytes,
        bytes: 0,
        allocation_unit: headroom
            .observe()
            .map_err(io::Error::other)?
            .allocation_unit,
        buffer: vec![0; 1024 * 1024],
    };
    let mut hash = blake3::Hasher::new();
    hash.update(b"codefabric.linux-runtime-image.v1\0");
    walker.directory(&descriptor, &before, destination, 0, &mut hash)?;
    Ok(Captured {
        content: *hash.finalize().as_bytes(),
        bytes: walker.bytes,
        entries: walker.census.entries,
    })
}

struct Capture<'a> {
    census: observation::Walk<'a>,
    headroom: &'a LocalDiskHeadroom,
    started: Instant,
    maximum_bytes: u64,
    bytes: u64,
    allocation_unit: u64,
    buffer: Vec<u8>,
}

impl Capture<'_> {
    fn check(&self, depth: usize) -> io::Result<()> {
        check(self.census.cancellation, self.started)?;
        if depth > 128 || self.census.entries >= 4_000_000 {
            return Err(io::Error::other(
                "runtime capture entry/depth bound exceeded",
            ));
        }
        Ok(())
    }

    fn directory(
        &mut self,
        source: impl AsFd,
        before: &Stat,
        destination: &Path,
        depth: usize,
        hash: &mut blake3::Hasher,
    ) -> io::Result<()> {
        self.check(depth)?;
        self.account(self.allocation_unit)?;
        let _directory_growth = self
            .headroom
            .try_reserve_growth(self.allocation_unit, ResourceClass::Data)
            .map_err(io::Error::other)?;
        fs::create_dir(destination)?;
        let target = crate::secure_path::open_absolute_directory_nofollow(destination)
            .map_err(io::Error::other)?;
        let (names, _memory) = self.census.names(&source)?;
        frame(hash, b"directory");
        metadata(hash, before);
        for name in names {
            self.check(depth)?;
            frame(hash, name.to_bytes());
            let observed = statat(&source, &name, AtFlags::SYMLINK_NOFOLLOW)?;
            let path = destination.join(std::ffi::OsStr::from_bytes(name.to_bytes()));
            match FileType::from_raw_mode(observed.st_mode) {
                FileType::Directory => {
                    let child = openat(
                        &source,
                        &name,
                        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                        Mode::empty(),
                    )?;
                    same(&observed, &fstat(&child)?)?;
                    self.directory(&child, &observed, &path, depth + 1, hash)?;
                }
                FileType::RegularFile => {
                    let child = openat(
                        &source,
                        &name,
                        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
                        Mode::empty(),
                    )?;
                    same(&observed, &fstat(&child)?)?;
                    self.file(child, &observed, &target, &name, hash)?;
                }
                FileType::Symlink => {
                    let mut bytes = [0_u8; 4097];
                    let length = readlinkat_raw(&source, &name, &mut bytes[..])?;
                    if length == bytes.len() {
                        return Err(io::Error::other("runtime link target bound exceeded"));
                    }
                    frame(hash, b"symlink");
                    frame(hash, &bytes[..length]);
                    frame(hash, &observed.st_mtime.to_be_bytes());
                    frame(hash, &observed.st_mtime_nsec.to_be_bytes());
                    self.account(length as u64)?;
                    let _link_growth = self
                        .headroom
                        .try_reserve_growth(length as u64, ResourceClass::Data)
                        .map_err(io::Error::other)?;
                    // Symlink text is preserved even for absent/outside targets. /etc and host
                    // paths remain absent in the sandbox; capture never follows such a link.
                    std::os::unix::fs::symlink(
                        std::ffi::OsStr::from_bytes(&bytes[..length]),
                        &path,
                    )?;
                    rustix::fs::utimensat(
                        &target,
                        &name,
                        &timestamps(&observed)?,
                        AtFlags::SYMLINK_NOFOLLOW,
                    )?;
                }
                _ => {
                    return Err(io::Error::other(
                        "native runtime contains a non-capturable special file",
                    ));
                }
            }
            same(
                &observed,
                &statat(&source, &name, AtFlags::SYMLINK_NOFOLLOW)?,
            )?;
        }
        same(before, &fstat(source)?)?;
        let allocated = u64::try_from(fstat(&target)?.st_blocks)
            .map_err(io::Error::other)?
            .saturating_mul(512);
        if allocated > self.allocation_unit {
            self.account(allocated - self.allocation_unit)?;
        }
        seal(&target, before)?;
        File::from(target).sync_all()?;
        frame(hash, b"directory-end");
        Ok(())
    }

    fn account(&mut self, bytes: u64) -> io::Result<()> {
        let bytes = bytes
            .max(1)
            .checked_add(self.allocation_unit - 1)
            .map(|bytes| bytes / self.allocation_unit * self.allocation_unit)
            .ok_or_else(|| io::Error::other("runtime allocation size overflow"))?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|bytes| *bytes <= self.maximum_bytes)
            .ok_or_else(|| io::Error::other("native runtime image disk capacity exceeded"))?;
        Ok(())
    }

    fn file(
        &mut self,
        source: OwnedFd,
        before: &Stat,
        target: impl AsFd,
        name: &std::ffi::CStr,
        hash: &mut blake3::Hasher,
    ) -> io::Result<()> {
        let size = u64::try_from(before.st_size).map_err(io::Error::other)?;
        self.account(size)?;
        let _growth = self
            .headroom
            .try_reserve_growth(size, ResourceClass::Data)
            .map_err(io::Error::other)?;
        let output = openat(
            target,
            name,
            OFlags::CREATE | OFlags::EXCL | OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::RUSR | Mode::WUSR,
        )?;
        let mut source = File::from(source);
        let mut output = File::from(output);
        // Native FICLONE snapshots the data extent when supported; never share a writable inode.
        let cloned = match rustix::fs::ioctl_ficlone(&output, &source) {
            Ok(()) => true,
            Err(error)
                if matches!(
                    error,
                    rustix::io::Errno::OPNOTSUPP
                        | rustix::io::Errno::XDEV
                        | rustix::io::Errno::INVAL
                        | rustix::io::Errno::NOSYS
                        | rustix::io::Errno::NOTTY
                ) =>
            {
                false
            }
            Err(error) => return Err(error.into()),
        };
        frame(hash, b"file");
        metadata(hash, before);
        frame(hash, &size.to_be_bytes());
        let mut content = blake3::Hasher::new();
        let mut total = 0_u64;
        loop {
            self.check(0)?;
            let count = if cloned {
                output.read(&mut self.buffer)?
            } else {
                source.read(&mut self.buffer)?
            };
            if count == 0 {
                break;
            }
            total = total
                .checked_add(count as u64)
                .filter(|total| *total <= size)
                .ok_or_else(|| io::Error::other("runtime file grew during capture"))?;
            content.update(&self.buffer[..count]);
            if !cloned {
                output.write_all(&self.buffer[..count])?;
            }
        }
        if total != size {
            return Err(io::Error::other("runtime file truncated during capture"));
        }
        same(before, &fstat(&source)?)?;
        frame(hash, content.finalize().as_bytes());
        seal(&output, before)?;
        output.sync_all()?;
        Ok(())
    }
}

fn same(before: &Stat, after: &Stat) -> io::Result<()> {
    if observation::stamp(before) != observation::stamp(after) {
        return Err(io::Error::other("native runtime changed during capture"));
    }
    Ok(())
}

fn metadata(hash: &mut blake3::Hasher, stat: &Stat) {
    // The image is application-owned, read-only and has no set-id bits. Captured mtimes retain
    // native tool input behavior; host ownership, inode numbers and access times are not identity.
    frame(hash, &(stat.st_mode & 0o555).to_be_bytes());
    frame(hash, &stat.st_mtime.to_be_bytes());
    frame(hash, &stat.st_mtime_nsec.to_be_bytes());
}

fn seal(file: impl AsFd, stat: &Stat) -> io::Result<()> {
    fchmod(&file, Mode::from_raw_mode(stat.st_mode & 0o555))?;
    futimens(file, &timestamps(stat)?)?;
    Ok(())
}

fn timestamps(stat: &Stat) -> io::Result<Timestamps> {
    Ok(Timestamps {
        last_access: rustix::fs::Timespec {
            tv_sec: 0,
            tv_nsec: rustix::fs::UTIME_OMIT,
        },
        last_modification: rustix::fs::Timespec {
            tv_sec: stat.st_mtime,
            tv_nsec: stat.st_mtime_nsec.try_into().map_err(io::Error::other)?,
        },
    })
}
