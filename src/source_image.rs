//! Immutable source-image capture, content-addressed blobs, and lease-safe reclamation.

use std::collections::BTreeSet;
use std::fs;
use std::io::{Read as _, Write as _};
use std::os::fd::OwnedFd;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rusqlite::{OptionalExtension as _, params};
use rustix::fs::{Mode, OFlags, fchmod, fsync, open, openat, renameat, unlinkat};
use rustix::io::Errno;
use thiserror::Error;

use crate::identity::{
    IdentityDomain, IdentityError, WorkspacePath, encode_public_id, random_registration_nonce,
    source_file_identity,
};
use crate::operational_store::{OperationalStore, OperationalStoreError};
pub use crate::provider_types::ProviderText;
pub use crate::registries::NewlineKind;
use crate::secure_path::{
    PlatformPath, SecurePathError, StableFileMetadata, StableFileRead, StableReadError,
    open_workspace_root,
};
use crate::snapshot::ServingSnapshotManifest;
use crate::workspace_registry::{WorkspaceRegistry, WorkspaceRegistryError};

/// Default maximum for ordinary source files (16 MiB).
pub const ORDINARY_SOURCE_MAXIMUM_BYTES: u64 = 16 * 1024 * 1024;
/// Maximum enabled only by explicit policy (64 MiB).
pub const EXPLICIT_SOURCE_MAXIMUM_BYTES: u64 = 64 * 1024 * 1024;
/// Default stable-read retry count.
pub const DEFAULT_STABLE_READ_RETRIES: u8 = 3;
/// Closed fault seam census for source capture, blob publication, leases, and reclamation.
pub const SOURCE_IMAGE_FAULT_POINT_CODES: [&str; 4] = [
    "SOURCE_CAPTURE_AFTER_FIRST_READ",
    "SOURCE_BLOB_BEFORE_RENAME",
    "SOURCE_LEASE_AFTER_ORPHAN",
    "SOURCE_GC_BEFORE_DELETE",
];

/// Source language controls encoding admission without changing original bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceLanguage {
    Rust,
    Python,
    Other,
}

/// Closed source-entry kinds admitted to source-image capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum SourceFileKind {
    Regular = 10,
}

/// Encoding classification retained with every immutable snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceEncoding {
    Utf8,
    Utf8Bom,
    PythonLatin1,
    Unsupported { declared: Option<String> },
}

impl SourceEncoding {
    const fn code(&self) -> u16 {
        match self {
            Self::Utf8 => 10,
            Self::Utf8Bom => 20,
            Self::PythonLatin1 => 30,
            Self::Unsupported { .. } => 40,
        }
    }
}

/// Little-endian `u64` line-start artifact and its content identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineIndex {
    pub offsets: Arc<[u64]>,
    pub serialized: Arc<[u8]>,
    pub digest: [u8; 32],
    pub format_version: u16,
    pub newline_kind: NewlineKind,
}

/// Read-only content-addressed blob reference; no live source path is disclosed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlobReference {
    pub digest: [u8; 32],
    pub relative_name: String,
    pub byte_length: u64,
}

/// AC-G-33 immutable source image consumed in and out of process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceImage {
    pub workspace_id: [u8; 16],
    pub worktree_id: Option<[u8; 16]>,
    pub source_generation: u64,
    pub file_id: [u8; 16],
    pub path: WorkspacePath,
    pub language: SourceLanguage,
    pub bytes: Arc<[u8]>,
    pub digest: [u8; 32],
    pub byte_length: u64,
    pub file_kind: SourceFileKind,
    pub blob: BlobReference,
    pub lease: SourceBlobLease,
    pub encoding: SourceEncoding,
    pub provider_text: Option<ProviderText>,
    pub line_index: LineIndex,
    pub metadata: StableFileMetadata,
}

/// Provider-facing name for the same exact immutable DTO.
pub type SourceSnapshot = SourceImage;

/// Explicit capability evidence for a source image that cannot be admitted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCapabilityGap {
    pub capability_code: &'static str,
    pub reason: &'static str,
    pub observed_bytes: Option<u64>,
    pub maximum_bytes: Option<u64>,
}

/// Bounded source capture result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CaptureOutcome {
    Published(Box<SourceImage>),
    Excluded(SourceCapabilityGap),
    Deferred,
}

/// Typed source-image lease holder classes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum SourceBlobHolderKind {
    ProviderRun = 10,
    SourceArtifact = 20,
    ServingSnapshot = 30,
}

/// Durable lease identity returned to its holder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceBlobLease {
    pub lease_id: [u8; 16],
    pub blob_digest: [u8; 32],
    pub expires_at: u64,
}

/// Operational counters; durations are observations, never design thresholds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceImageMetrics {
    pub capture_attempts: u64,
    pub published_images: u64,
    pub deferred_images: u64,
    pub oversized_images: u64,
    pub captured_bytes: u64,
    pub stable_read_retries: u64,
    pub capture_duration_micros: u64,
    pub acquired_leases: u64,
    pub renewed_leases: u64,
    pub released_leases: u64,
    pub orphaned_leases: u64,
    pub live_holders: u64,
    pub orphan_holders: u64,
    pub reclaimed_blobs: u64,
    pub reclaimed_bytes: u64,
}

/// One bounded garbage-collection pass.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GarbageCollectionReport {
    pub blobs: u64,
    pub bytes: u64,
}

/// Failures from capture, persistence, and immutable-blob operations.
#[derive(Debug, Error)]
pub enum SourceImageError {
    #[error(transparent)]
    SecurePath(#[from] SecurePathError),
    #[error(transparent)]
    StableRead(#[from] StableReadError),
    #[error(transparent)]
    Store(#[from] OperationalStoreError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Registry(#[from] WorkspaceRegistryError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error("immutable blob-store I/O failed")]
    BlobIo,
    #[error("existing content-addressed blob does not match its name")]
    BlobDigestMismatch,
    #[error("source generation changed during capture")]
    GenerationChanged,
    #[error("source generation overflow")]
    GenerationOverflow,
    #[error("source blob lease does not exist or is no longer active")]
    LeaseInactive,
    #[error("retained serving-snapshot provenance is malformed: {0}")]
    RetainedProvenance(String),
}

/// Capture policy loaded from the generated deployment profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceCapturePolicy {
    pub maximum_bytes: u64,
    pub stable_read_retries: u8,
    pub lease_ttl: Duration,
}

impl Default for SourceCapturePolicy {
    fn default() -> Self {
        Self {
            maximum_bytes: ORDINARY_SOURCE_MAXIMUM_BYTES,
            stable_read_retries: DEFAULT_STABLE_READ_RETRIES,
            lease_ttl: Duration::from_mins(5),
        }
    }
}

/// One request to bind authoritative bytes to a source generation and holder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaptureRequest {
    pub workspace_id: [u8; 16],
    pub source_generation: u64,
    pub change_token: u64,
    pub path: PlatformPath,
    pub language: SourceLanguage,
    pub holder_kind: SourceBlobHolderKind,
    pub holder_id: [u8; 16],
}

/// Content-addressed directory whose committed filenames are lowercase BLAKE3 hex.
pub struct BlobStore {
    root: PathBuf,
    descriptor: OwnedFd,
}

impl BlobStore {
    /// Create or validate one private blob directory.
    ///
    /// # Errors
    ///
    /// Returns an I/O failure for symlinks, non-directories, or unsafe permissions.
    pub fn open(root: &Path) -> Result<Self, SourceImageError> {
        fs::create_dir_all(root).map_err(|_| SourceImageError::BlobIo)?;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700))
            .map_err(|_| SourceImageError::BlobIo)?;
        let metadata = fs::symlink_metadata(root).map_err(|_| SourceImageError::BlobIo)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(SourceImageError::BlobIo);
        }
        let descriptor = open(
            root,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY,
            Mode::empty(),
        )
        .map_err(|_| SourceImageError::BlobIo)?;
        Ok(Self {
            root: root.to_owned(),
            descriptor,
        })
    }

    /// Return the internal path for a digest-named immutable blob.
    #[must_use]
    pub fn path_for(&self, digest: &[u8; 32]) -> PathBuf {
        self.root.join(digest_name(digest))
    }

    fn put(&self, bytes: &[u8]) -> Result<BlobReference, SourceImageError> {
        let digest = crate::integrity::digest_bytes(bytes);
        let name = digest_name(&digest);
        match self.read_named(&name) {
            Ok(existing) => {
                if existing != bytes {
                    return Err(SourceImageError::BlobDigestMismatch);
                }
            }
            Err(SourceImageError::BlobIo) => self.publish(&name, bytes)?,
            Err(error) => return Err(error),
        }
        Ok(BlobReference {
            digest,
            relative_name: name,
            byte_length: u64::try_from(bytes.len()).map_err(|_| SourceImageError::BlobIo)?,
        })
    }

    fn read_named(&self, name: &str) -> Result<Vec<u8>, SourceImageError> {
        let descriptor = openat(
            &self.descriptor,
            name,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(|error| {
            if error == Errno::NOENT {
                SourceImageError::BlobIo
            } else {
                SourceImageError::BlobDigestMismatch
            }
        })?;
        let mut file = fs::File::from(descriptor);
        let metadata = file.metadata().map_err(|_| SourceImageError::BlobIo)?;
        if !metadata.is_file() || metadata.permissions().mode() & 0o777 != 0o400 {
            return Err(SourceImageError::BlobDigestMismatch);
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|_| SourceImageError::BlobIo)?;
        if digest_name(&crate::integrity::digest_bytes(&bytes)) != name {
            return Err(SourceImageError::BlobDigestMismatch);
        }
        Ok(bytes)
    }

    fn publish(&self, name: &str, bytes: &[u8]) -> Result<(), SourceImageError> {
        let temporary = format!(
            ".tmp-{}-{}",
            std::process::id(),
            u128::from_be_bytes(random_registration_nonce()?)
        );
        let descriptor = openat(
            &self.descriptor,
            &temporary,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|_| SourceImageError::BlobIo)?;
        let mut file = fs::File::from(descriptor);
        let result = (|| {
            file.write_all(bytes)
                .map_err(|_| SourceImageError::BlobIo)?;
            file.sync_all().map_err(|_| SourceImageError::BlobIo)?;
            fchmod(&file, Mode::RUSR).map_err(|_| SourceImageError::BlobIo)?;
            renameat(&self.descriptor, &temporary, &self.descriptor, name)
                .map_err(|_| SourceImageError::BlobIo)?;
            fsync(&self.descriptor).map_err(|_| SourceImageError::BlobIo)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = unlinkat(&self.descriptor, &temporary, rustix::fs::AtFlags::empty());
        }
        result
    }

    fn remove(&self, digest: &[u8; 32]) -> Result<(), SourceImageError> {
        match unlinkat(
            &self.descriptor,
            digest_name(digest),
            rustix::fs::AtFlags::empty(),
        ) {
            Ok(()) | Err(Errno::NOENT) => {
                fsync(&self.descriptor).map_err(|_| SourceImageError::BlobIo)
            }
            Err(_) => Err(SourceImageError::BlobIo),
        }
    }
}

/// Daemon lifecycle-owned source capture, lease, and garbage-collection service.
pub struct SourceImageStore {
    blobs: BlobStore,
    policy: SourceCapturePolicy,
    metrics: SourceImageMetrics,
}

enum StableCapture {
    Stable(StableFileRead),
    Terminal(CaptureOutcome),
}

impl SourceImageStore {
    /// Open one source-image store.
    ///
    /// # Errors
    ///
    /// Returns an immutable blob-directory validation failure.
    pub fn open(root: &Path, policy: SourceCapturePolicy) -> Result<Self, SourceImageError> {
        if policy.maximum_bytes == 0 || policy.maximum_bytes > EXPLICIT_SOURCE_MAXIMUM_BYTES {
            return Err(SourceImageError::BlobIo);
        }
        Ok(Self {
            blobs: BlobStore::open(root)?,
            policy,
            metrics: SourceImageMetrics::default(),
        })
    }

    /// Capture and lease one coherent source snapshot.
    ///
    /// # Errors
    ///
    /// Returns a secure-path, generation, blob, identity, or persistence failure.
    pub fn capture(
        &mut self,
        store: &mut OperationalStore,
        request: &CaptureRequest,
    ) -> Result<CaptureOutcome, SourceImageError> {
        self.capture_with_fence(store, request, || Some(request.change_token))
    }

    /// Capture with a caller-owned seqlock/watcher fence (`None` means mutation in progress).
    ///
    /// # Errors
    ///
    /// Returns the same failures as [`Self::capture`]. A changed token defers publication.
    pub fn capture_with_fence(
        &mut self,
        store: &mut OperationalStore,
        request: &CaptureRequest,
        mut observe_change_token: impl FnMut() -> Option<u64>,
    ) -> Result<CaptureOutcome, SourceImageError> {
        let started = Instant::now();
        self.metrics.capture_attempts = self.metrics.capture_attempts.saturating_add(1);
        if current_source_generation(store, request.workspace_id)? != request.source_generation {
            return Err(SourceImageError::GenerationChanged);
        }
        let root = open_workspace_root(store, request.workspace_id)?;
        let workspace_path = root.workspace_path(&request.path)?;
        let stable = match self.stable_read(&root, request, &mut observe_change_token, started)? {
            StableCapture::Stable(read) => read,
            StableCapture::Terminal(outcome) => return Ok(outcome),
        };
        if current_source_generation(store, request.workspace_id)? != request.source_generation {
            return Err(SourceImageError::GenerationChanged);
        }
        if observe_change_token() != Some(request.change_token) {
            self.metrics.deferred_images = self.metrics.deferred_images.saturating_add(1);
            self.record_duration(started);
            return Ok(CaptureOutcome::Deferred);
        }
        let digest = crate::integrity::digest_bytes(&stable.bytes);
        let blob = self.blobs.put(&stable.bytes)?;
        let line_index = build_line_index(&stable.bytes);
        let line_blob = self.blobs.put(&line_index.serialized)?;
        debug_assert_eq!(line_blob.digest, line_index.digest);
        let (encoding, provider_text) = classify_encoding(request.language, &stable.bytes);
        let file_id = source_file_identity(&workspace_path)?.id;
        let record = WorkspaceRegistry::new(store).show(request.workspace_id)?;
        persist_blob(
            store,
            digest,
            u64::try_from(stable.bytes.len()).map_err(|_| SourceImageError::BlobIo)?,
            line_index.digest,
            encoding.code(),
            line_index.newline_kind as u16,
        )?;
        let lease = acquire_source_blob_lease(
            store,
            digest,
            request.workspace_id,
            request.source_generation,
            request.holder_kind,
            request.holder_id,
            self.policy.lease_ttl,
        )?;
        self.metrics.acquired_leases = self.metrics.acquired_leases.saturating_add(1);
        self.refresh_lease_metrics(store)?;
        self.metrics.published_images = self.metrics.published_images.saturating_add(1);
        self.metrics.captured_bytes = self
            .metrics
            .captured_bytes
            .saturating_add(u64::try_from(stable.bytes.len()).unwrap_or(u64::MAX));
        self.record_duration(started);
        Ok(CaptureOutcome::Published(Box::new(SourceImage {
            workspace_id: request.workspace_id,
            worktree_id: record.worktree_id,
            source_generation: request.source_generation,
            file_id,
            path: workspace_path,
            language: request.language,
            bytes: Arc::from(stable.bytes),
            digest,
            byte_length: blob.byte_length,
            file_kind: SourceFileKind::Regular,
            blob,
            lease,
            encoding,
            provider_text,
            line_index,
            metadata: stable.metadata,
        })))
    }

    fn stable_read(
        &mut self,
        root: &crate::secure_path::SecureRoot,
        request: &CaptureRequest,
        observe_change_token: &mut impl FnMut() -> Option<u64>,
        started: Instant,
    ) -> Result<StableCapture, SourceImageError> {
        for attempt in 0..=self.policy.stable_read_retries {
            if observe_change_token() != Some(request.change_token) {
                return Ok(StableCapture::Terminal(self.deferred(started)));
            }
            match root.read_stable_file(&request.path, self.policy.maximum_bytes) {
                Ok(read) if observe_change_token() == Some(request.change_token) => {
                    return Ok(StableCapture::Stable(read));
                }
                Ok(_) | Err(StableReadError::ChangedDuringRead)
                    if attempt < self.policy.stable_read_retries =>
                {
                    self.metrics.stable_read_retries =
                        self.metrics.stable_read_retries.saturating_add(1);
                }
                Ok(_) | Err(StableReadError::ChangedDuringRead) => {
                    return Ok(StableCapture::Terminal(self.deferred(started)));
                }
                Err(StableReadError::SizeLimitExceeded { observed, limit }) => {
                    self.metrics.oversized_images = self.metrics.oversized_images.saturating_add(1);
                    self.record_duration(started);
                    return Ok(StableCapture::Terminal(CaptureOutcome::Excluded(
                        SourceCapabilityGap {
                            capability_code: "SOURCE_BYTES",
                            reason: "source-image-size-limit",
                            observed_bytes: Some(observed),
                            maximum_bytes: Some(limit),
                        },
                    )));
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(StableCapture::Terminal(self.deferred(started)))
    }

    fn deferred(&mut self, started: Instant) -> CaptureOutcome {
        self.metrics.deferred_images = self.metrics.deferred_images.saturating_add(1);
        self.record_duration(started);
        CaptureOutcome::Deferred
    }

    fn refresh_lease_metrics(&mut self, store: &OperationalStore) -> Result<(), SourceImageError> {
        let (live, orphaned) = store
            .reader_factory()
            .open()?
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT COALESCE(SUM(state_code=1), 0), COALESCE(SUM(state_code=2), 0)
                     FROM source_blob_lease",
                    [],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
                )
            })?;
        self.metrics.live_holders = u64::try_from(live).map_err(|_| SourceImageError::BlobIo)?;
        self.metrics.orphan_holders =
            u64::try_from(orphaned).map_err(|_| SourceImageError::BlobIo)?;
        Ok(())
    }

    /// Acquire one serving-snapshot holder over a closed set of source blobs.
    ///
    /// Each snapshot lease uses its own holder identity, while all member blobs share
    /// one source-blob lease record. An empty source set requires no coupled lease.
    ///
    /// # Errors
    ///
    /// Returns a persistence, identity, or missing-blob failure.
    pub fn acquire_serving_snapshot_lease(
        &mut self,
        store: &mut OperationalStore,
        workspace_id: [u8; 16],
        source_generation: u64,
        holder_id: [u8; 16],
        blob_digests: &[[u8; 32]],
        ttl: Duration,
    ) -> Result<Option<[u8; 16]>, SourceImageError> {
        let mut source_lease_id = None;
        for &blob_digest in blob_digests {
            let lease = acquire_source_blob_lease(
                store,
                blob_digest,
                workspace_id,
                source_generation,
                SourceBlobHolderKind::ServingSnapshot,
                holder_id,
                ttl,
            )?;
            if let Some(expected) = source_lease_id
                && expected != lease.lease_id
            {
                return Err(SourceImageError::BlobDigestMismatch);
            }
            source_lease_id = Some(lease.lease_id);
        }
        self.metrics.acquired_leases = self
            .metrics
            .acquired_leases
            .saturating_add(u64::from(source_lease_id.is_some()));
        self.refresh_lease_metrics(store)?;
        Ok(source_lease_id)
    }

    /// Renew an active source-blob lease.
    ///
    /// # Errors
    ///
    /// Returns a persistence failure or `LeaseInactive`.
    pub fn renew(
        &mut self,
        store: &mut OperationalStore,
        lease_id: [u8; 16],
        ttl: Duration,
    ) -> Result<(), SourceImageError> {
        let expires_at = unix_seconds()?.saturating_add(ttl.as_secs());
        let expires_at_sql = sql_i64(expires_at)?;
        let updated = store.write_transaction(|transaction| {
            transaction
                .execute(
                    "UPDATE source_blob_lease SET expires_at=?2 WHERE lease_id=?1 AND state_code=1",
                    params![lease_id.as_slice(), expires_at_sql],
                )
                .map_err(SourceImageError::from)
        })?;
        if updated != 1 {
            return Err(SourceImageError::LeaseInactive);
        }
        self.metrics.renewed_leases = self.metrics.renewed_leases.saturating_add(1);
        self.refresh_lease_metrics(store)?;
        Ok(())
    }

    /// Release one lease idempotently.
    ///
    /// # Errors
    ///
    /// Returns a persistence failure.
    pub fn release(
        &mut self,
        store: &mut OperationalStore,
        lease_id: [u8; 16],
    ) -> Result<(), SourceImageError> {
        let removed = store.write_transaction(|transaction| {
            transaction.execute(
                "DELETE FROM source_blob_lease_member WHERE lease_id=?1",
                [lease_id.as_slice()],
            )?;
            transaction
                .execute(
                    "DELETE FROM source_blob_lease WHERE lease_id=?1",
                    [lease_id.as_slice()],
                )
                .map_err(SourceImageError::from)
        })?;
        self.metrics.released_leases = self
            .metrics
            .released_leases
            .saturating_add(u64::try_from(removed).unwrap_or(u64::MAX));
        self.refresh_lease_metrics(store)?;
        Ok(())
    }

    /// Mark every process-local active holder orphaned after restart.
    ///
    /// # Errors
    ///
    /// Returns a persistence failure.
    pub fn orphan_after_restart(
        &mut self,
        store: &mut OperationalStore,
        now: u64,
    ) -> Result<u64, SourceImageError> {
        let now_sql = sql_i64(now)?;
        let count = store.write_transaction(|transaction| {
            transaction
                .execute(
                    "UPDATE source_blob_lease SET state_code=2, orphaned_at=?1 WHERE state_code=1",
                    [now_sql],
                )
                .map_err(SourceImageError::from)
        })?;
        let count = u64::try_from(count).unwrap_or(u64::MAX);
        self.metrics.orphaned_leases = self.metrics.orphaned_leases.saturating_add(count);
        self.refresh_lease_metrics(store)?;
        Ok(count)
    }

    /// Reclaim a bounded set of expired, unheld blobs and their line indexes.
    ///
    /// # Errors
    ///
    /// Returns a persistence or immutable-blob I/O failure.
    pub fn collect_garbage(
        &mut self,
        store: &mut OperationalStore,
        now: u64,
        orphan_grace: Duration,
        maximum_blobs: usize,
    ) -> Result<GarbageCollectionReport, SourceImageError> {
        let grace = orphan_grace.as_secs();
        let now_sql = sql_i64(now)?;
        let grace_sql = sql_i64(grace)?;
        let maximum_blobs_sql =
            i64::try_from(maximum_blobs).map_err(|_| SourceImageError::BlobIo)?;
        let retained_publication_blobs =
            store
                .reader_factory()
                .open()?
                .with_connection(|connection| {
                    let mut statement = connection.prepare(
                        "SELECT manifest_json_bytes FROM serving_snapshot_manifest
                     ORDER BY snapshot_id",
                    )?;
                    statement
                        .query_map([], |row| row.get::<_, Vec<u8>>(0))?
                        .collect::<Result<Vec<_>, _>>()
                })?
                .into_iter()
                .try_fold(BTreeSet::new(), |mut retained, bytes| {
                    let manifest: ServingSnapshotManifest = serde_json::from_slice(&bytes)
                        .map_err(|error| SourceImageError::RetainedProvenance(error.to_string()))?;
                    retained.extend(manifest.raw_source_blob_digests().map_err(|error| {
                        SourceImageError::RetainedProvenance(error.to_string())
                    })?);
                    Ok::<_, SourceImageError>(retained)
                })?;
        let candidates = store
            .reader_factory()
            .open()?
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT blob_digest, byte_length, line_index_digest FROM source_blob b
                 WHERE NOT EXISTS (
                   SELECT 1 FROM source_blob_lease_member m
                   JOIN source_blob_lease l ON l.lease_id=m.lease_id
                   WHERE m.blob_digest=b.blob_digest
                   AND ((l.state_code=1 AND l.expires_at>?1)
                     OR (l.state_code=2 AND l.orphaned_at IS NOT NULL AND l.orphaned_at+?2>?1))
                 ) ORDER BY blob_digest LIMIT ?3",
                )?;
                statement
                    .query_map(params![now_sql, grace_sql, maximum_blobs_sql], |row| {
                        Ok((
                            row.get::<_, Vec<u8>>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, Vec<u8>>(2)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()
            })?;
        let candidates = candidates
            .into_iter()
            .filter(
                |(digest, _, _)| match <&[u8; 32]>::try_from(digest.as_slice()) {
                    Ok(digest) => !retained_publication_blobs.contains(digest),
                    Err(_) => true,
                },
            )
            .collect::<Vec<_>>();
        let mut report = GarbageCollectionReport::default();
        for (raw_digest, byte_length, raw_line_digest) in candidates {
            let byte_length = u64::try_from(byte_length).map_err(|_| SourceImageError::BlobIo)?;
            let digest = fixed_digest(&raw_digest)?;
            let line_digest = fixed_digest(&raw_line_digest)?;
            let deleted = store.write_transaction(|transaction| {
                let publication_protected = {
                    let mut statement = transaction.prepare(
                        "SELECT manifest_json_bytes FROM serving_snapshot_manifest
                         ORDER BY snapshot_id",
                    )?;
                    let manifests = statement
                        .query_map([], |row| row.get::<_, Vec<u8>>(0))?
                        .collect::<Result<Vec<_>, _>>()?;
                    manifests.into_iter().try_fold(false, |protected, bytes| {
                        if protected {
                            return Ok::<_, SourceImageError>(true);
                        }
                        let manifest: ServingSnapshotManifest = serde_json::from_slice(&bytes)
                            .map_err(|error| {
                                SourceImageError::RetainedProvenance(error.to_string())
                            })?;
                        Ok(manifest
                            .raw_source_blob_digests()
                            .map_err(|error| {
                                SourceImageError::RetainedProvenance(error.to_string())
                            })?
                            .contains(&digest))
                    })?
                };
                let protected: bool = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM source_blob_lease_member m
                     JOIN source_blob_lease l ON l.lease_id=m.lease_id
                     WHERE m.blob_digest=?1
                     AND ((state_code=1 AND expires_at>?2)
                       OR (state_code=2 AND orphaned_at IS NOT NULL AND orphaned_at+?3>?2)))",
                    params![digest.as_slice(), now_sql, grace_sql],
                    |row| row.get(0),
                )?;
                if protected || publication_protected {
                    return Ok::<bool, SourceImageError>(false);
                }
                self.blobs.remove(&digest)?;
                transaction.execute(
                    "DELETE FROM source_blob_lease_member WHERE blob_digest=?1",
                    [digest.as_slice()],
                )?;
                transaction.execute(
                    "DELETE FROM source_blob_lease WHERE NOT EXISTS (
                       SELECT 1 FROM source_blob_lease_member m
                       WHERE m.lease_id=source_blob_lease.lease_id
                     )",
                    [],
                )?;
                transaction.execute(
                    "DELETE FROM source_blob WHERE blob_digest=?1",
                    [digest.as_slice()],
                )?;
                let line_referenced: bool = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM source_blob WHERE line_index_digest=?1)",
                    [line_digest.as_slice()],
                    |row| row.get(0),
                )?;
                if !line_referenced {
                    self.blobs.remove(&line_digest)?;
                }
                Ok(true)
            })?;
            if deleted {
                report.blobs = report.blobs.saturating_add(1);
                report.bytes = report.bytes.saturating_add(byte_length);
            }
        }
        self.metrics.reclaimed_blobs = self.metrics.reclaimed_blobs.saturating_add(report.blobs);
        self.metrics.reclaimed_bytes = self.metrics.reclaimed_bytes.saturating_add(report.bytes);
        self.refresh_lease_metrics(store)?;
        Ok(report)
    }

    /// Current operational counters.
    #[must_use]
    pub const fn metrics(&self) -> SourceImageMetrics {
        self.metrics
    }

    /// Project one active durable holder and all of its blob members into the generated
    /// provider-control schema. The URI is capability-like and path-free; the provider
    /// sandbox resolves it only after receiving read-only access to this store.
    ///
    /// # Errors
    ///
    /// Returns `LeaseInactive` for an absent/orphaned holder and rejects malformed durable
    /// identities or values that cannot be represented by the protocol.
    pub fn source_snapshot_lease(
        &self,
        store: &OperationalStore,
        lease_id: [u8; 16],
        source_manifest_digest: [u8; 32],
    ) -> Result<
        crate::rpc::generated::codefabric::provider::v1::SourceSnapshotLease,
        SourceImageError,
    > {
        let (workspace_id, source_generation, expires_at, blobs) = store
            .reader_factory()
            .open()?
            .with_connection(|connection| {
                let header = connection
                    .query_row(
                        "SELECT workspace_id, source_generation, expires_at
                         FROM source_blob_lease WHERE lease_id=?1 AND state_code=1",
                        [lease_id.as_slice()],
                        |row| {
                            Ok((
                                row.get::<_, Vec<u8>>(0)?,
                                row.get::<_, i64>(1)?,
                                row.get::<_, i64>(2)?,
                            ))
                        },
                    )
                    .optional()?;
                let Some(header) = header else {
                    return Ok(None);
                };
                let mut statement = connection.prepare(
                    "SELECT m.blob_digest, b.byte_length
                     FROM source_blob_lease_member m
                     JOIN source_blob b ON b.blob_digest=m.blob_digest
                     WHERE m.lease_id=?1 ORDER BY m.blob_digest",
                )?;
                let blobs = statement
                    .query_map([lease_id.as_slice()], |row| {
                        Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Some((header.0, header.1, header.2, blobs)))
            })?
            .ok_or(SourceImageError::LeaseInactive)?;
        let workspace_id = <[u8; 16]>::try_from(workspace_id.as_slice())
            .map_err(|_| SourceImageError::BlobDigestMismatch)?;
        let source_generation =
            u64::try_from(source_generation).map_err(|_| SourceImageError::GenerationChanged)?;
        let expires_at_unix_ms = expires_at
            .checked_mul(1_000)
            .ok_or(SourceImageError::BlobIo)?;
        let blobs = blobs
            .into_iter()
            .map(|(digest, byte_length)| {
                let digest = <[u8; 32]>::try_from(digest.as_slice())
                    .map_err(|_| SourceImageError::BlobDigestMismatch)?;
                let byte_length =
                    u64::try_from(byte_length).map_err(|_| SourceImageError::BlobIo)?;
                let hex = digest_name(&digest);
                Ok(
                    crate::rpc::generated::codefabric::provider::v1::BlobReference {
                        blob_id: format!("source-blob:{hex}"),
                        content_digest: format!("b3:{hex}"),
                        byte_length,
                        read_only_uri: format!("codefabric-blob:{hex}"),
                    },
                )
            })
            .collect::<Result<Vec<_>, SourceImageError>>()?;
        if blobs.is_empty() {
            return Err(SourceImageError::LeaseInactive);
        }
        Ok(
            crate::rpc::generated::codefabric::provider::v1::SourceSnapshotLease {
                lease_id: digest_name_16(&lease_id),
                workspace_id: encode_public_id(IdentityDomain::Workspace, None, workspace_id)?,
                source_generation,
                source_manifest_digest: format!("b3:{}", digest_name(&source_manifest_digest)),
                expires_at_unix_ms,
                blobs,
            },
        )
    }

    fn record_duration(&mut self, started: Instant) {
        self.metrics.capture_duration_micros = self
            .metrics
            .capture_duration_micros
            .saturating_add(u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX));
    }
}

/// Atomically advance one accepted coherent source generation.
///
/// # Errors
///
/// Returns a mismatch, overflow, or persistence failure.
pub fn advance_source_generation(
    store: &mut OperationalStore,
    workspace_id: [u8; 16],
    expected_generation: u64,
) -> Result<u64, SourceImageError> {
    let next = expected_generation
        .checked_add(1)
        .ok_or(SourceImageError::GenerationOverflow)?;
    let expected_generation_sql = sql_i64(expected_generation)?;
    let next_sql = sql_i64(next)?;
    let updated = store.write_transaction(|transaction| {
        let generation = transaction.execute(
            "UPDATE workspace_generation SET source_generation=?3, updated_at=?4
             WHERE workspace_id=?1 AND source_generation=?2",
            params![
                workspace_id.as_slice(),
                expected_generation_sql,
                next_sql,
                unix_seconds()?.to_string()
            ],
        )?;
        let worktree = transaction.execute(
            "UPDATE worktree_state SET source_generation=?3, newest_dirty_generation=?3,
             updated_at=?4 WHERE workspace_id=?1 AND source_generation=?2",
            params![
                workspace_id.as_slice(),
                expected_generation_sql,
                next_sql,
                unix_seconds()?.to_string()
            ],
        )?;
        if generation != 1 || worktree != 1 {
            return Err(SourceImageError::GenerationChanged);
        }
        Ok(next)
    })?;
    Ok(updated)
}

/// Read the durable generation from an independent query-only connection.
///
/// # Errors
///
/// Returns a persistence failure when the workspace is missing or unreadable.
pub fn current_source_generation(
    store: &OperationalStore,
    workspace_id: [u8; 16],
) -> Result<u64, SourceImageError> {
    store
        .reader_factory()
        .open()?
        .with_connection(|connection| {
            connection.query_row(
                "SELECT source_generation FROM workspace_generation WHERE workspace_id=?1",
                [workspace_id.as_slice()],
                |row| row.get::<_, i64>(0),
            )
        })
        .map_err(SourceImageError::from)
        .and_then(|value| u64::try_from(value).map_err(|_| SourceImageError::GenerationChanged))
}

fn persist_blob(
    store: &mut OperationalStore,
    digest: [u8; 32],
    byte_length: u64,
    line_index_digest: [u8; 32],
    encoding_code: u16,
    newline_code: u16,
) -> Result<(), SourceImageError> {
    let byte_length_sql = sql_i64(byte_length)?;
    store.write_transaction(|transaction| {
        transaction.execute(
            "INSERT OR IGNORE INTO source_blob(blob_digest, byte_length, line_index_digest,
             encoding_code, newline_code, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                digest.as_slice(),
                byte_length_sql,
                line_index_digest.as_slice(),
                encoding_code,
                newline_code,
                unix_seconds()?.to_string()
            ],
        )?;
        let exact = transaction
            .query_row(
                "SELECT byte_length, line_index_digest, encoding_code, newline_code
                 FROM source_blob WHERE blob_digest=?1",
                [digest.as_slice()],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, u16>(2)?,
                        row.get::<_, u16>(3)?,
                    ))
                },
            )
            .optional()?;
        if exact
            != Some((
                byte_length_sql,
                line_index_digest.to_vec(),
                encoding_code,
                newline_code,
            ))
        {
            return Err(SourceImageError::BlobDigestMismatch);
        }
        Ok(())
    })
}

fn acquire_source_blob_lease(
    store: &mut OperationalStore,
    blob_digest: [u8; 32],
    workspace_id: [u8; 16],
    source_generation: u64,
    holder_kind: SourceBlobHolderKind,
    holder_id: [u8; 16],
    ttl: Duration,
) -> Result<SourceBlobLease, SourceImageError> {
    let lease_id = random_registration_nonce()?;
    let expires_at = unix_seconds()?.saturating_add(ttl.as_secs());
    let source_generation_sql = sql_i64(source_generation)?;
    let expires_at_sql = sql_i64(expires_at)?;
    let actual_lease_id = store.write_transaction(|transaction| {
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM source_blob WHERE blob_digest=?1)",
            [blob_digest.as_slice()],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(SourceImageError::BlobDigestMismatch);
        }
        transaction.execute(
            "INSERT OR IGNORE INTO source_blob_lease(lease_id, workspace_id,
             source_generation, holder_kind_code, holder_id, state_code, expires_at, orphaned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, NULL)",
            params![
                lease_id.as_slice(),
                workspace_id.as_slice(),
                source_generation_sql,
                holder_kind as u16,
                holder_id.as_slice(),
                expires_at_sql
            ],
        )?;
        let actual = transaction.query_row(
            "SELECT lease_id FROM source_blob_lease WHERE workspace_id=?1
             AND source_generation=?2 AND holder_kind_code=?3 AND holder_id=?4",
            params![
                workspace_id.as_slice(),
                source_generation_sql,
                holder_kind as u16,
                holder_id.as_slice()
            ],
            |row| row.get::<_, Vec<u8>>(0),
        )?;
        let actual = <[u8; 16]>::try_from(actual.as_slice())
            .map_err(|_| SourceImageError::BlobDigestMismatch)?;
        transaction.execute(
            "UPDATE source_blob_lease SET state_code=1, expires_at=?2, orphaned_at=NULL
             WHERE lease_id=?1",
            params![actual.as_slice(), expires_at_sql],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO source_blob_lease_member(lease_id, blob_digest)
             VALUES (?1, ?2)",
            params![actual.as_slice(), blob_digest.as_slice()],
        )?;
        Ok(actual)
    })?;
    Ok(SourceBlobLease {
        lease_id: actual_lease_id,
        blob_digest,
        expires_at,
    })
}

fn build_line_index(bytes: &[u8]) -> LineIndex {
    let mut offsets = vec![0_u64];
    let mut saw_lf = false;
    let mut saw_crlf = false;
    let mut saw_cr = false;
    let mut index = 0_usize;
    while index < bytes.len() {
        if bytes[index] == b'\r' {
            if bytes.get(index + 1) == Some(&b'\n') {
                index += 2;
                saw_crlf = true;
            } else {
                index += 1;
                saw_cr = true;
            }
            offsets.push(u64::try_from(index).unwrap_or(u64::MAX));
        } else if bytes[index] == b'\n' {
            index += 1;
            saw_lf = true;
            offsets.push(u64::try_from(index).unwrap_or(u64::MAX));
        } else {
            index += 1;
        }
    }
    let kinds = u8::from(saw_lf) + u8::from(saw_crlf) + u8::from(saw_cr);
    let newline_kind = match (kinds, saw_lf, saw_crlf, saw_cr) {
        (0, _, _, _) => NewlineKind::None,
        (1, true, _, _) => NewlineKind::Lf,
        (1, _, true, _) => NewlineKind::Crlf,
        (1, _, _, true) => NewlineKind::Cr,
        _ => NewlineKind::Mixed,
    };
    let serialized = offsets
        .iter()
        .flat_map(|offset| offset.to_le_bytes())
        .collect::<Vec<_>>();
    LineIndex {
        offsets: Arc::from(offsets),
        digest: crate::integrity::digest_bytes(&serialized),
        serialized: Arc::from(serialized),
        format_version: 1,
        newline_kind,
    }
}

fn classify_encoding(
    language: SourceLanguage,
    bytes: &[u8],
) -> (SourceEncoding, Option<ProviderText>) {
    let bom = bytes.starts_with(&[0xef, 0xbb, 0xbf]);
    let payload = if bom { &bytes[3..] } else { bytes };
    if let Ok(text) = std::str::from_utf8(payload) {
        let offsets = text
            .char_indices()
            .map(|(offset, _)| u64::try_from(offset + usize::from(bom) * 3).unwrap_or(u64::MAX))
            .chain(std::iter::once(
                u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            ))
            .collect::<Vec<_>>();
        return (
            if bom {
                SourceEncoding::Utf8Bom
            } else {
                SourceEncoding::Utf8
            },
            Some(ProviderText {
                text: Arc::from(text),
                original_byte_offsets: Arc::from(offsets),
            }),
        );
    }
    if language == SourceLanguage::Python {
        let declared = python_encoding_cookie(bytes);
        if declared
            .as_deref()
            .is_some_and(|name| matches!(name, "latin-1" | "latin1" | "iso-8859-1" | "iso-latin-1"))
        {
            let text = bytes
                .iter()
                .map(|byte| char::from(*byte))
                .collect::<String>();
            let offsets = (0..=bytes.len())
                .map(|offset| u64::try_from(offset).unwrap_or(u64::MAX))
                .collect::<Vec<_>>();
            return (
                SourceEncoding::PythonLatin1,
                Some(ProviderText {
                    text: Arc::from(text),
                    original_byte_offsets: Arc::from(offsets),
                }),
            );
        }
        return (SourceEncoding::Unsupported { declared }, None);
    }
    (SourceEncoding::Unsupported { declared: None }, None)
}

fn python_encoding_cookie(bytes: &[u8]) -> Option<String> {
    for line in bytes.split(|byte| *byte == b'\n').take(2) {
        let Ok(ascii) = std::str::from_utf8(line) else {
            continue;
        };
        let Some(coding) = ascii.find("coding") else {
            continue;
        };
        let Some(suffix) = ascii.get(coding + "coding".len()..) else {
            continue;
        };
        let suffix = suffix.trim_start();
        let Some(suffix) = suffix
            .strip_prefix(':')
            .or_else(|| suffix.strip_prefix('='))
        else {
            continue;
        };
        let label = suffix
            .trim_start()
            .chars()
            .take_while(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
            })
            .collect::<String>()
            .to_ascii_lowercase()
            .replace('_', "-");
        if !label.is_empty() {
            return Some(label);
        }
    }
    None
}

fn digest_name(digest: &[u8; 32]) -> String {
    crate::integrity::frame_digest(*digest)[3..].to_owned()
}

fn digest_name_16(digest: &[u8; 16]) -> String {
    let mut name = String::with_capacity(32);
    for byte in digest {
        use std::fmt::Write as _;
        write!(name, "{byte:02x}").expect("writing to a String is infallible");
    }
    name
}

fn fixed_digest(bytes: &[u8]) -> Result<[u8; 32], SourceImageError> {
    bytes
        .try_into()
        .map_err(|_| SourceImageError::BlobDigestMismatch)
}

fn unix_seconds() -> Result<u64, SourceImageError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| SourceImageError::BlobIo)
}

fn sql_i64(value: u64) -> Result<i64, SourceImageError> {
    i64::try_from(value).map_err(|_| SourceImageError::GenerationOverflow)
}

#[cfg(test)]
mod tests {
    use std::io::{Seek as _, SeekFrom};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::thread;

    use super::*;
    use crate::identity::PlatformCode;
    use crate::snapshot::{
        ServingSnapshotManifestBody, SnapshotBasePublication, SnapshotBundles,
        SnapshotContextRecord, SnapshotContexts, SnapshotIndexes, SnapshotOverlay, SnapshotSource,
    };
    use crate::workspace_registry::WorkspaceSourceRegistration;

    fn platform() -> PlatformCode {
        if cfg!(target_os = "macos") {
            PlatformCode::MacOs
        } else {
            PlatformCode::Unix
        }
    }

    fn fixture() -> (
        tempfile::TempDir,
        OperationalStore,
        [u8; 16],
        PathBuf,
        SourceImageStore,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("workspace");
        fs::create_dir(&root).unwrap();
        let mut store = OperationalStore::open(&directory.path().join("state.sqlite3")).unwrap();
        let workspace_id = WorkspaceRegistry::new(&mut store)
            .add(&root, WorkspaceSourceRegistration::Directory)
            .unwrap()
            .workspace_id;
        let images = SourceImageStore::open(
            &directory.path().join("source-blobs"),
            SourceCapturePolicy::default(),
        )
        .unwrap();
        (directory, store, workspace_id, root, images)
    }

    fn request(workspace_id: [u8; 16], path: &[u8]) -> CaptureRequest {
        CaptureRequest {
            workspace_id,
            source_generation: 0,
            change_token: 0,
            path: PlatformPath::from_raw_relative_bytes(platform(), path.to_vec()).unwrap(),
            language: SourceLanguage::Rust,
            holder_kind: SourceBlobHolderKind::ProviderRun,
            holder_id: [9; 16],
        }
    }

    fn retained_manifest(
        workspace_id: [u8; 16],
        source_digest: [u8; 32],
    ) -> ServingSnapshotManifest {
        let context_id = [0x51_u8; 16];
        let context_set = crate::identity::context_set_identity(workspace_id, &[context_id])
            .unwrap()
            .id;
        let framed = |byte| crate::integrity::frame_digest([byte; 32]);
        ServingSnapshotManifestBody {
            manifest_version: "1.0".into(),
            workspace_id: encode_public_id(IdentityDomain::Workspace, None, workspace_id).unwrap(),
            repository_id: None,
            worktree_id: None,
            registration_revision: 1,
            source: SnapshotSource {
                source_generation: 0,
                admitted_event_sequence: 0,
                reconciled_event_sequence: 0,
                inventory_digest: framed(1),
                authorization_fingerprint: framed(2),
                inclusion_policy_fingerprint: framed(3),
                path_profile_version: "1".into(),
                source_trust_state: "CURRENT".into(),
                event_stream_health: "HEALTHY".into(),
                git_acceleration_status: "NOT_REQUIRED".into(),
                git_state_fingerprint: None,
            },
            contexts: SnapshotContexts {
                context_set_id: encode_public_id(IdentityDomain::ContextSet, None, context_set)
                    .unwrap(),
                default_python_context_id: None,
                default_rust_context_id: None,
                records: vec![SnapshotContextRecord {
                    analysis_context_id: encode_public_id(
                        IdentityDomain::AnalysisContext,
                        None,
                        context_id,
                    )
                    .unwrap(),
                    context_manifest_digest: framed(4),
                    capability_partition_digest: framed(5),
                }],
            },
            base_publication: SnapshotBasePublication {
                publication_id: encode_public_id(IdentityDomain::Publication, None, [0x52; 16])
                    .unwrap(),
                tables: Vec::new(),
            },
            overlay: SnapshotOverlay {
                overlay_generation: 0,
                overlay_digest: framed(0),
                total_memory_bytes: 0,
                tables: Vec::new(),
            },
            indexes: SnapshotIndexes {
                capability_index_digest: framed(6),
                diagnostic_index_digest: framed(7),
                dependency_graph_digest: framed(8),
            },
            bundles: SnapshotBundles {
                ontology_bundle_id: "ontology:1".into(),
                schema_bundle_id: "schema:1".into(),
                provider_bundle_id: "provider:1".into(),
                derivation_bundle_id: "derivation:1".into(),
                query_language_bundle_id: "query:1".into(),
                model_pack_bundle_id: "model:1".into(),
                toolchain_bundle_id: "toolchain:1".into(),
            },
            limits_profile_digest: framed(9),
            source_blob_digests: vec![crate::integrity::frame_digest(source_digest)],
        }
        .derive()
        .unwrap()
    }

    #[test]
    fn wp16_behavioral_acceptance() {
        let (directory, mut store, workspace_id, root, mut images) = fixture();
        let bytes = b"first\r\nsecond\nthird\rlast";
        fs::write(root.join("source.rs"), bytes).unwrap();
        let CaptureOutcome::Published(image) = images
            .capture(&mut store, &request(workspace_id, b"source.rs"))
            .unwrap()
        else {
            panic!("stable source was not published");
        };
        assert_eq!(image.bytes.as_ref(), bytes);
        assert_eq!(image.digest, crate::integrity::digest_bytes(bytes));
        assert_eq!(image.line_index.offsets.as_ref(), &[0, 7, 14, 20]);
        assert_eq!(image.line_index.newline_kind, NewlineKind::Mixed);
        assert_eq!(image.line_index.serialized.len(), 4 * 8);
        assert_eq!(
            image.line_index.digest,
            crate::integrity::digest_bytes(&image.line_index.serialized)
        );
        assert_eq!(build_line_index(b"").offsets.as_ref(), &[0]);
        assert_eq!(build_line_index(b"no-newline").offsets.as_ref(), &[0]);
        assert_eq!(build_line_index(b"a\nb\n").offsets.as_ref(), &[0, 2, 4]);
        assert_eq!(build_line_index(b"a\r\nb").offsets.as_ref(), &[0, 3]);
        assert_eq!(
            classify_encoding(SourceLanguage::Rust, b"utf8").0,
            SourceEncoding::Utf8
        );
        assert_eq!(
            classify_encoding(SourceLanguage::Python, b"\xef\xbb\xbfvalue = 1\n").0,
            SourceEncoding::Utf8Bom
        );
        assert_eq!(
            classify_encoding(
                SourceLanguage::Python,
                b"#!/usr/bin/python\n# coding: latin-1\nname = '\xff'\n"
            )
            .0,
            SourceEncoding::PythonLatin1
        );
        assert!(matches!(
            classify_encoding(SourceLanguage::Rust, b"\xff").0,
            SourceEncoding::Unsupported { .. }
        ));

        assert_eq!(current_source_generation(&store, workspace_id).unwrap(), 0);
        assert_eq!(
            advance_source_generation(&mut store, workspace_id, 0).unwrap(),
            1
        );
        drop(images);
        let database = directory.path().join("state.sqlite3");
        drop(store);
        let reopened = OperationalStore::open(&database).unwrap();
        assert_eq!(
            current_source_generation(&reopened, workspace_id).unwrap(),
            1
        );
    }

    #[test]
    fn wp16_structural_acceptance() {
        let (_directory, mut store, workspace_id, root, mut images) = fixture();
        fs::write(root.join("source.rs"), b"pub fn value() -> u8 { 7 }\n").unwrap();
        let CaptureOutcome::Published(image) = images
            .capture(&mut store, &request(workspace_id, b"source.rs"))
            .unwrap()
        else {
            panic!("stable source was not published");
        };
        assert_eq!(image.blob.relative_name.len(), 64);
        assert_eq!(SourceBlobHolderKind::ProviderRun as u16, 10);
        assert_eq!(SourceBlobHolderKind::SourceArtifact as u16, 20);
        assert_eq!(SourceBlobHolderKind::ServingSnapshot as u16, 30);
        assert!(
            image
                .blob
                .relative_name
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        );
        let metadata = fs::metadata(images.blobs.path_for(&image.digest)).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o400);
        fs::write(root.join("second.rs"), b"pub fn second() {}\n").unwrap();
        let second_request = request(workspace_id, b"second.rs");
        let CaptureOutcome::Published(second) =
            images.capture(&mut store, &second_request).unwrap()
        else {
            panic!("second stable source was not published");
        };
        assert_eq!(image.lease.lease_id, second.lease.lease_id);
        let rpc_lease = images
            .source_snapshot_lease(&store, image.lease.lease_id, [0x42; 32])
            .unwrap();
        assert_eq!(rpc_lease.source_generation, 0);
        assert_eq!(rpc_lease.blobs.len(), 2);
        assert_eq!(
            rpc_lease.source_manifest_digest,
            format!("b3:{}", "42".repeat(32))
        );
        assert!(rpc_lease.workspace_id.starts_with("workspace:"));
        assert!(
            rpc_lease
                .blobs
                .windows(2)
                .all(|pair| pair[0].content_digest < pair[1].content_digest)
        );
        assert!(rpc_lease.blobs.iter().all(|blob| {
            blob.blob_id.starts_with("source-blob:")
                && blob.content_digest.starts_with("b3:")
                && blob.read_only_uri.starts_with("codefabric-blob:")
        }));
        let tables = store
            .reader_factory()
            .open()
            .unwrap()
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT name FROM sqlite_schema WHERE type='table' AND name LIKE 'source_%' ORDER BY name",
                )?;
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()
            })
            .unwrap();
        assert_eq!(
            tables,
            [
                "source_blob",
                "source_blob_lease",
                "source_blob_lease_member",
                "source_inventory"
            ]
        );
    }

    #[test]
    fn wp24_source_blob_lease_coupling() {
        let (_directory, mut store, workspace_id, root, mut images) = fixture();
        fs::write(root.join("snapshot.rs"), b"pub fn snapshot() {}\n").unwrap();
        let CaptureOutcome::Published(image) = images
            .capture(&mut store, &request(workspace_id, b"snapshot.rs"))
            .unwrap()
        else {
            panic!("stable source was not published");
        };
        let source_lease_id = images
            .acquire_serving_snapshot_lease(
                &mut store,
                workspace_id,
                0,
                [0x91; 16],
                &[image.digest],
                Duration::from_mins(5),
            )
            .unwrap()
            .unwrap();
        let serving_holders: i64 = store
            .reader_factory()
            .open()
            .unwrap()
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT COUNT(*) FROM source_blob_lease
                     WHERE lease_id=?1 AND holder_kind_code=?2 AND holder_id=?3",
                    params![
                        source_lease_id.as_slice(),
                        SourceBlobHolderKind::ServingSnapshot as u16,
                        [0x91_u8; 16].as_slice(),
                    ],
                    |row| row.get(0),
                )
            })
            .unwrap();
        assert_eq!(serving_holders, 1);
        images.release(&mut store, source_lease_id).unwrap();
        let remaining: i64 = store
            .reader_factory()
            .open()
            .unwrap()
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT COUNT(*) FROM source_blob_lease WHERE lease_id=?1",
                    [source_lease_id.as_slice()],
                    |row| row.get(0),
                )
            })
            .unwrap();
        assert_eq!(remaining, 0);
    }

    #[test]
    fn wp16_negative_zero_state() {
        let (_directory, mut store, workspace_id, root, _images) = fixture();
        fs::write(root.join("large.rs"), b"12345").unwrap();
        let mut bounded = SourceImageStore::open(
            &root.parent().unwrap().join("small-blobs"),
            SourceCapturePolicy {
                maximum_bytes: 4,
                ..SourceCapturePolicy::default()
            },
        )
        .unwrap();
        assert!(matches!(
            bounded
                .capture(&mut store, &request(workspace_id, b"large.rs"))
                .unwrap(),
            CaptureOutcome::Excluded(SourceCapabilityGap {
                capability_code: "SOURCE_BYTES",
                reason: "source-image-size-limit",
                observed_bytes: Some(5),
                maximum_bytes: Some(4),
            })
        ));

        let left = vec![b'a'; 1024];
        let right = vec![b'b'; 1024];
        fs::write(root.join("racy.rs"), &left).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let token = Arc::new(AtomicU64::new(0));
        let writer_stop = Arc::clone(&stop);
        let writer_token = Arc::clone(&token);
        let writer_path = root.join("racy.rs");
        let writer_left = left.clone();
        let writer_right = right.clone();
        let writer = thread::spawn(move || {
            let mut select_left = false;
            while !writer_stop.load(Ordering::Acquire) {
                writer_token.fetch_add(1, Ordering::AcqRel);
                fs::write(
                    &writer_path,
                    if select_left {
                        &writer_left
                    } else {
                        &writer_right
                    },
                )
                .unwrap();
                writer_token.fetch_add(1, Ordering::AcqRel);
                select_left = !select_left;
            }
        });
        let mut images = SourceImageStore::open(
            &root.parent().unwrap().join("race-blobs"),
            SourceCapturePolicy::default(),
        )
        .unwrap();
        let expected = [
            crate::integrity::digest_bytes(&left),
            crate::integrity::digest_bytes(&right),
        ];
        let race_request = request(workspace_id, b"racy.rs");
        for _ in 0..250 {
            let outcome = images
                .capture_with_fence(&mut store, &race_request, || {
                    let observed = token.load(Ordering::Acquire);
                    observed.is_multiple_of(2).then_some(observed)
                })
                .unwrap();
            if let CaptureOutcome::Published(image) = outcome {
                assert!(expected.contains(&image.digest));
            }
        }
        stop.store(true, Ordering::Release);
        writer.join().unwrap();
    }

    #[test]
    fn wp66_negative_zero_state() {
        let (_directory, mut store, workspace_id, root, mut images) = fixture();
        fs::write(root.join("retained.rs"), b"pub fn retained() {}\n").unwrap();
        let CaptureOutcome::Published(image) = images
            .capture(&mut store, &request(workspace_id, b"retained.rs"))
            .unwrap()
        else {
            panic!("retained source was not captured");
        };
        let manifest = retained_manifest(workspace_id, image.digest);
        let snapshot_id = manifest.raw_snapshot_id().unwrap();
        let publication_id = manifest.raw_publication_id().unwrap();
        let manifest_body = manifest.body.canonical_body().unwrap();
        let manifest_json = serde_json::to_vec(&manifest).unwrap();
        let manifest_digest = manifest.raw_manifest_digest().unwrap();
        store
            .write_transaction(|transaction| {
                transaction.execute(
                    "INSERT INTO serving_snapshot_manifest(
                       snapshot_id, workspace_id, publication_id, state_code,
                       manifest_body_bytes, manifest_json_bytes, manifest_digest,
                       created_at, activated_at, retired_at)
                     VALUES (?1, ?2, ?3, 30, ?4, ?5, ?6, 1, NULL, 2)",
                    params![
                        snapshot_id.as_slice(),
                        workspace_id.as_slice(),
                        publication_id.as_slice(),
                        manifest_body,
                        manifest_json,
                        manifest_digest.as_slice(),
                    ],
                )?;
                Ok::<_, SourceImageError>(())
            })
            .unwrap();
        images.release(&mut store, image.lease.lease_id).unwrap();
        let now = unix_seconds().unwrap();
        assert_eq!(
            images
                .collect_garbage(&mut store, now, Duration::ZERO, 10)
                .unwrap()
                .blobs,
            0,
            "retained publication provenance permitted source-blob collection"
        );
        assert!(images.blobs.path_for(&image.digest).exists());

        store
            .write_transaction(|transaction| {
                transaction.execute(
                    "DELETE FROM serving_snapshot_manifest WHERE snapshot_id=?1",
                    [snapshot_id.as_slice()],
                )?;
                Ok::<_, SourceImageError>(())
            })
            .unwrap();
        assert_eq!(
            images
                .collect_garbage(&mut store, now, Duration::ZERO, 10)
                .unwrap()
                .blobs,
            1
        );
    }

    #[test]
    fn wp16_operational_acceptance() {
        let (_directory, mut store, workspace_id, root, mut images) = fixture();
        fs::write(root.join("lease.rs"), b"leased\n").unwrap();
        let CaptureOutcome::Published(image) = images
            .capture(&mut store, &request(workspace_id, b"lease.rs"))
            .unwrap()
        else {
            panic!("stable source was not published");
        };
        let mut second_request = request(workspace_id, b"lease.rs");
        second_request.holder_kind = SourceBlobHolderKind::SourceArtifact;
        second_request.holder_id = [8; 16];
        let CaptureOutcome::Published(second_image) =
            images.capture(&mut store, &second_request).unwrap()
        else {
            panic!("second holder was not leased");
        };
        let now = unix_seconds().unwrap();
        images
            .renew(&mut store, image.lease.lease_id, Duration::from_mins(10))
            .unwrap();
        assert_eq!(
            images
                .collect_garbage(&mut store, now, Duration::from_secs(30), 10)
                .unwrap()
                .blobs,
            0
        );
        images.release(&mut store, image.lease.lease_id).unwrap();
        assert_eq!(images.metrics().live_holders, 1);
        assert_eq!(
            images
                .collect_garbage(&mut store, now, Duration::from_secs(30), 10)
                .unwrap()
                .blobs,
            0
        );
        images
            .release(&mut store, second_image.lease.lease_id)
            .unwrap();
        assert_eq!(
            images
                .collect_garbage(&mut store, now, Duration::from_secs(30), 10)
                .unwrap()
                .blobs,
            1
        );
        assert_eq!(
            images
                .collect_garbage(&mut store, now, Duration::from_secs(30), 10)
                .unwrap(),
            GarbageCollectionReport::default()
        );

        fs::write(root.join("orphan.rs"), b"orphan\n").unwrap();
        let mut orphan_request = request(workspace_id, b"orphan.rs");
        orphan_request.holder_id = [7; 16];
        let CaptureOutcome::Published(_orphan) =
            images.capture(&mut store, &orphan_request).unwrap()
        else {
            panic!("stable source was not published");
        };
        assert_eq!(images.orphan_after_restart(&mut store, now).unwrap(), 1);
        assert_eq!(
            images
                .collect_garbage(&mut store, now + 29, Duration::from_secs(30), 10)
                .unwrap()
                .blobs,
            0
        );
        assert_eq!(
            images
                .collect_garbage(&mut store, now + 31, Duration::from_secs(30), 10)
                .unwrap()
                .blobs,
            1
        );
        let metrics = images.metrics();
        assert_eq!(metrics.published_images, 3);
        assert_eq!(metrics.acquired_leases, 3);
        assert_eq!(metrics.renewed_leases, 1);
        assert_eq!(metrics.reclaimed_blobs, 2);
        assert_eq!((metrics.live_holders, metrics.orphan_holders), (0, 0));
        assert!(metrics.capture_duration_micros > 0);
    }

    #[test]
    fn wp16_operational_schema_v2_migrates_to_v3() {
        let (directory, store, workspace_id, _root, _images) = fixture();
        let database = directory.path().join("state.sqlite3");
        drop(store);
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "DROP TABLE source_inventory;
                 DROP TABLE source_blob;
                 DROP TABLE source_blob_lease;
                 DROP TABLE source_blob_lease_member;
                 DROP TABLE table_mutation_operation;
                 DROP TABLE operational_dependency_edge;
                 DROP TABLE git_candidate_cache;
                 PRAGMA user_version=2;",
            )
            .unwrap();
        drop(connection);
        let migrated = OperationalStore::open(&database).unwrap();
        assert_eq!(
            current_source_generation(&migrated, workspace_id).unwrap(),
            0
        );
        let count = migrated
            .reader_factory()
            .open()
            .unwrap()
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE type='table' AND name LIKE 'source_%'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
            })
            .unwrap();
        assert_eq!(count, 4);
    }

    #[test]
    #[ignore = "bounded 10,000-attempt three-size security campaign"]
    fn wp16_source_capture_race_campaign() {
        let campaign: serde_json::Value = serde_json::from_str(include_str!(
            "../contracts/fixtures/security/source-capture-race-v1.json"
        ))
        .unwrap();
        assert_eq!(campaign["capture_attempts"], 10_000);
        assert_eq!(
            campaign["file_sizes"],
            serde_json::json!([1024, 1_048_576, 15_728_640])
        );

        let (_directory, mut store, workspace_id, root, mut images) = fixture();
        for (size_index, size) in [1024_usize, 1_048_576, 15_728_640].into_iter().enumerate() {
            fs::write(root.join("campaign.rs"), vec![b'x'; size]).unwrap();
            let token = Arc::new(AtomicU64::new(0));
            let stop = Arc::new(AtomicBool::new(false));
            let writer_token = Arc::clone(&token);
            let writer_stop = Arc::clone(&stop);
            let writer_path = root.join("campaign.rs");
            let writer = thread::spawn(move || {
                let mut value = b'a';
                let mut file = fs::File::options().write(true).open(writer_path).unwrap();
                while !writer_stop.load(Ordering::Acquire) {
                    writer_token.fetch_add(1, Ordering::AcqRel);
                    file.seek(SeekFrom::End(-1)).unwrap();
                    file.write_all(&[value]).unwrap();
                    file.flush().unwrap();
                    writer_token.fetch_add(1, Ordering::AcqRel);
                    value = if value == b'a' { b'b' } else { b'a' };
                }
            });
            while token.load(Ordering::Acquire) < 2 {
                thread::yield_now();
            }
            let request = request(workspace_id, b"campaign.rs");
            let attempts: usize = 10_000_usize / 3 + usize::from(size_index == 0);
            for _ in 0..attempts {
                assert!(matches!(
                    images
                        .capture_with_fence(&mut store, &request, || {
                            let observed = token.load(Ordering::Acquire);
                            observed.is_multiple_of(2).then_some(observed)
                        })
                        .unwrap(),
                    CaptureOutcome::Deferred
                ));
            }
            stop.store(true, Ordering::Release);
            writer.join().unwrap();
        }
    }
}
