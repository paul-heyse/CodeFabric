//! Immutable Linux runtime inputs. Metadata selects an accelerator; captured bytes own identity.
//!
//! One private per-user cache is shared across daemons. A global file lock serializes publication
//! and eviction; independently opened shared image locks survive until actual provider owners
//! release them. No current image can be overwritten or reclaimed while leased.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Read as _, Write as _};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::cancellation::Cancellation;
use crate::disk_headroom::LocalDiskHeadroom;
use crate::resource_budget::{ResourceAmounts, ResourceBudget, ResourceClass, ResourceReservation};

mod capture;
pub(crate) mod observation;

const MAX_IMAGE_BYTES: u64 = 96 * 1024 * 1024 * 1024;
const MAX_CACHE_BYTES: u64 = 192 * 1024 * 1024 * 1024;
const MAX_CACHE_IMAGES: usize = 8;
const MAX_DURATION: Duration = Duration::from_secs(600);

#[derive(Clone, Debug)]
pub(crate) struct RuntimeImage(Arc<ImageOwner>);

#[derive(Debug)]
struct ImageOwner {
    root: PathBuf,
    manifest: Manifest,
    _lease: File,
    _charge: ResourceReservation,
}

impl PartialEq for RuntimeImage {
    fn eq(&self, other: &Self) -> bool {
        self.0.root == other.0.root && self.0.manifest == other.0.manifest
    }
}
impl Eq for RuntimeImage {}

impl RuntimeImage {
    pub(crate) fn root(&self) -> &Path {
        &self.0.root
    }
    pub(crate) fn digest(&self) -> [u8; 32] {
        self.0.manifest.content
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u8,
    observation: [u8; 32],
    content: [u8; 32],
    image_metadata: [u8; 32],
    bytes: u64,
    entries: usize,
}

#[derive(Default)]
pub(crate) struct RuntimeCache {
    retained: Option<RuntimeImage>,
}

impl RuntimeCache {
    pub(crate) fn acquire(
        &mut self,
        observed: [u8; 32],
        budget: &ResourceBudget,
        cancellation: &Cancellation,
    ) -> io::Result<RuntimeImage> {
        let base = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
            .filter(|path| path.is_absolute())
            .ok_or_else(|| io::Error::other("absolute user cache directory is unavailable"))?;
        let cache = base.join("codefabric/runtime-v1");
        private_directory(&cache)?;
        self.acquire_at(Path::new("/usr"), &cache, observed, budget, cancellation)
    }

    fn acquire_at(
        &mut self,
        source: &Path,
        cache: &Path,
        observed: [u8; 32],
        budget: &ResourceBudget,
        cancellation: &Cancellation,
    ) -> io::Result<RuntimeImage> {
        let started = Instant::now();
        if let Some(image) = &self.retained
            && image.0.manifest.observation == observed
        {
            validate_image(image.root(), &image.0.manifest, budget, cancellation)?;
            return Ok(image.clone());
        }
        let lock = lock_file(&cache.join("store.lock"))?;
        loop {
            check(cancellation, started)?;
            match lock.try_lock() {
                Ok(()) => break,
                Err(TryLockError::WouldBlock) => std::thread::sleep(Duration::from_millis(20)),
                Err(TryLockError::Error(error)) => return Err(error),
            }
        }
        let name = format!("image-{}", blake3::Hash::from(observed).to_hex());
        let entry = cache.join(&name);
        // Release this accelerator before collection; provider and candidate clones keep their
        // original locks and charges. Collection never equates cache eviction with process join.
        self.retained = None;
        let retained_bytes = collect(cache, &name, cancellation, started)?;
        if !entry.try_exists()? {
            if observation::observe_root(source, budget, cancellation)? != observed {
                return Err(io::Error::other("native runtime changed before capture"));
            }
            let nonce = crate::identity::random_registration_nonce().map_err(io::Error::other)?;
            let nonce = nonce
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            let staging = cache.join(format!(".stage-{nonce}"));
            fs::create_dir(&staging)?;
            fs::set_permissions(&staging, fs::Permissions::from_mode(0o700))?;
            let mut staging = Staging(Some(staging));
            let stage = staging.0.as_ref().expect("owned staging");
            let headroom = LocalDiskHeadroom::open(cache).map_err(io::Error::other)?;
            let available = MAX_CACHE_BYTES
                .saturating_sub(retained_bytes)
                .min(MAX_IMAGE_BYTES);
            let result = capture::copy(
                source,
                &stage.join("usr"),
                budget,
                &headroom,
                cancellation,
                started,
                available,
            )?;
            if observation::observe_root(source, budget, cancellation)? != observed {
                return Err(io::Error::other("native runtime changed during capture"));
            }
            let manifest = Manifest {
                version: 1,
                observation: observed,
                content: result.content,
                image_metadata: observation::observe_root(
                    &stage.join("usr"),
                    budget,
                    cancellation,
                )?,
                bytes: result.bytes,
                entries: result.entries,
            };
            let _lease = lock_file(&stage.join("lease"))?;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o400)
                .open(stage.join("manifest.json"))?;
            file.write_all(&serde_json::to_vec(&manifest)?)?;
            file.sync_all()?;
            File::open(stage)?.sync_all()?;
            fs::rename(stage, &entry)?;
            staging.0 = None;
            File::open(cache)?.sync_all()?;
            tracing::info!(
                bytes = manifest.bytes,
                entries = manifest.entries,
                "captured native runtime image"
            );
        }
        let manifest = read_manifest(&entry)?;
        if manifest.observation != observed {
            return Err(io::Error::other("runtime cache observation mismatch"));
        }
        // Global exclusion prevents an eviction between opening this inode and acquiring its
        // shared lease. A cloned File is never relocked (flock locks share a file description).
        let lease = lock_file(&entry.join("lease"))?;
        lease.try_lock_shared().map_err(io::Error::other)?;
        validate_image(&entry.join("usr"), &manifest, budget, cancellation)?;
        let charge = budget
            .try_reserve(
                ResourceClass::Data,
                ResourceAmounts {
                    disk_bytes: manifest.bytes,
                    memory_bytes: (entry.as_os_str().len() + 4096) as u64,
                    ..Default::default()
                },
            )
            .map_err(io::Error::other)?;
        let image = RuntimeImage(Arc::new(ImageOwner {
            root: entry.join("usr"),
            manifest,
            _lease: lease,
            _charge: charge,
        }));
        self.retained = Some(image.clone());
        Ok(image)
    }
}

fn validate_image(
    root: &Path,
    manifest: &Manifest,
    budget: &ResourceBudget,
    cancellation: &Cancellation,
) -> io::Result<()> {
    if observation::observe_root(root, budget, cancellation)? != manifest.image_metadata {
        return Err(io::Error::other("captured native runtime was modified"));
    }
    Ok(())
}

fn read_manifest(entry: &Path) -> io::Result<Manifest> {
    let mut bytes = Vec::new();
    let file = OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(entry.join("manifest.json"))?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > 4096 {
        return Err(io::Error::other("invalid runtime manifest file"));
    }
    file.take(4097).read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err(io::Error::other("runtime manifest bound exceeded"));
    }
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    if manifest.version != 1
        || manifest.content == [0; 32]
        || manifest.bytes > MAX_IMAGE_BYTES
        || manifest.entries > 4_000_000
    {
        return Err(io::Error::other("invalid runtime image manifest"));
    }
    Ok(manifest)
}

/// Reclaim only unleased recomputable images. At most eight live images and 192 GiB are
/// admitted globally; pressure on a pinned image is a capacity error, never a deletion permit.
fn collect(
    cache: &Path,
    selected: &str,
    cancellation: &Cancellation,
    started: Instant,
) -> io::Result<u64> {
    let mut bytes = 0_u64;
    let mut images = 0;
    for entry in fs::read_dir(cache)? {
        check(cancellation, started)?;
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            return Err(io::Error::other("unknown runtime cache entry"));
        };
        if name.starts_with(".stage-") {
            remove_image(&entry.path())?;
        } else if name.starts_with("image-") {
            if !entry.file_type()?.is_dir() {
                return Err(io::Error::other("runtime image entry is not a directory"));
            }
            let lease = lock_file(&entry.path().join("lease"))?;
            match lease.try_lock() {
                Ok(()) if name != selected => remove_image(&entry.path())?,
                Ok(()) | Err(TryLockError::WouldBlock) => {
                    bytes = bytes
                        .checked_add(read_manifest(&entry.path())?.bytes)
                        .ok_or_else(|| io::Error::other("runtime cache size overflow"))?;
                    images += 1;
                }
                Err(TryLockError::Error(error)) => return Err(error),
            }
        } else if name != "store.lock" {
            return Err(io::Error::other("unknown runtime cache entry"));
        }
    }
    if images > MAX_CACHE_IMAGES
        || (images == MAX_CACHE_IMAGES && !cache.join(selected).try_exists()?)
        || bytes > MAX_CACHE_BYTES
    {
        return Err(io::Error::other(
            "leased native runtime cache capacity exceeded",
        ));
    }
    Ok(bytes)
}

fn lock_file(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != rustix::process::getuid().as_raw()
        || metadata.mode() & 0o077 != 0
        || metadata.nlink() != 1
    {
        return Err(io::Error::other("runtime lock is not private"));
    }
    Ok(file)
}

fn private_directory(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::create_dir(path) {
        Ok(()) => fs::set_permissions(path, fs::Permissions::from_mode(0o700))?,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let descriptor =
        crate::secure_path::open_absolute_directory_nofollow(path).map_err(io::Error::other)?;
    let stat = rustix::fs::fstat(descriptor)?;
    if stat.st_uid != rustix::process::getuid().as_raw() || stat.st_mode & 0o077 != 0 {
        return Err(io::Error::other("native runtime cache is not private"));
    }
    Ok(())
}

fn check(cancellation: &Cancellation, started: Instant) -> io::Result<()> {
    if cancellation.is_cancelled() {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "runtime capture cancelled",
        ));
    }
    if started.elapsed() > MAX_DURATION {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "runtime capture deadline exceeded",
        ));
    }
    Ok(())
}

fn frame(hash: &mut blake3::Hasher, bytes: &[u8]) {
    hash.update(&(bytes.len() as u64).to_be_bytes());
    hash.update(bytes);
}

fn remove_image(path: &Path) -> io::Result<()> {
    // Captured symlinks are never traversed by reclamation. The parent is private and all
    // callers hold global exclusion plus, for published images, an exclusive image lease.
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        for entry in fs::read_dir(path)? {
            remove_image(&entry?.path())?;
        }
        fs::remove_dir(path)
    } else {
        fs::remove_file(path)
    }
}

struct Staging(Option<PathBuf>);
impl Drop for Staging {
    fn drop(&mut self) {
        if let Some(path) = &self.0
            && let Err(error) = remove_image(path)
        {
            tracing::warn!(%error, "runtime staging cleanup remains for the next cache owner");
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
