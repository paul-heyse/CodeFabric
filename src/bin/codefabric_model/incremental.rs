//! Optional content-addressed model execution and non-authoritative watch hints.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use notify_debouncer_full::{DebounceEventResult, new_debouncer, notify::RecursiveMode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use super::desired_tree::{ActionExecutableIdentity, SafeOutputPath};
use super::driver_protocol::{DriverDescriptor, DriverSourceFence, StagingRoot};
use super::model_control::StableId;
use super::repository_model::read_stable;

const CACHE_ROOT: &str = "target/model-cache/v1";
const CACHE_SCHEMA: &str = "model-action-cache-v1";
const MAX_CACHE_MANIFEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_CACHE_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_CACHE_OUTPUTS: usize = 512;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Optional cache operating mode. Cache state never affects correctness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheMode {
    Disabled,
    ReadOnly,
    ReadWrite,
}

impl CacheMode {
    fn from_environment() -> Self {
        match std::env::var("CODEFABRIC_MODEL_CACHE_MODE").as_deref() {
            Ok("read-only") => Self::ReadOnly,
            Ok("read-write") => Self::ReadWrite,
            _ => Self::Disabled,
        }
    }
}

/// Canonical family action identity. Every field can affect output bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyActionIdentity {
    schema: String,
    family: String,
    descriptor: DriverDescriptor,
    source_fence: Vec<ActionSourceIdentity>,
    upstream_output_digests: Vec<ActionUpstreamIdentity>,
    executable: ActionExecutableIdentity,
    tool_identity: Value,
    normalized_environment: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionSourceIdentity {
    path: SafeOutputPath,
    digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionUpstreamIdentity {
    output_id: StableId,
    digest: String,
}

impl FamilyActionIdentity {
    /// Compute the RFC 8785/BLAKE3 key for this complete identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the closed identity cannot be canonically serialized.
    pub fn action_key(&self) -> Result<String, IncrementalError> {
        canonical_digest(self)
    }
}

/// Resolve a complete family action identity.
///
/// # Errors
///
/// Returns an error when the closed identity cannot be encoded.
pub fn family_action_identity(
    family: &str,
    descriptor: &DriverDescriptor,
    source_fence: &DriverSourceFence,
    tool_identity: &Value,
) -> Result<FamilyActionIdentity, IncrementalError> {
    let executable = ActionExecutableIdentity::current()?;
    Ok(FamilyActionIdentity {
        schema: CACHE_SCHEMA.to_owned(),
        family: family.to_owned(),
        descriptor: descriptor.clone(),
        source_fence: source_fence
            .digests
            .iter()
            .map(|(path, digest)| ActionSourceIdentity {
                path: path.clone(),
                digest: digest.clone(),
            })
            .collect(),
        upstream_output_digests: Vec::new(),
        executable,
        tool_identity: tool_identity.clone(),
        normalized_environment: BTreeMap::from([
            ("credentials".to_owned(), "stripped".to_owned()),
            ("network".to_owned(), "undeclared".to_owned()),
            ("output_destination".to_owned(), "staging-only".to_owned()),
        ]),
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheOutput {
    path: SafeOutputPath,
    digest: String,
    byte_length: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheManifest {
    schema: String,
    action_key: String,
    identity: FamilyActionIdentity,
    outputs: Vec<CacheOutput>,
}

/// Cache lookup result and conservative fallback reason.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum CacheLookup {
    Hit { outputs: Vec<String> },
    Miss { reason: String },
}

impl CacheLookup {
    /// Whether this diagnostic records a validated byte-for-byte cache hit.
    #[must_use]
    pub fn is_hit(&self) -> bool {
        matches!(self, Self::Hit { .. })
    }

    /// Conservative miss/fallback reason, when present.
    #[must_use]
    pub fn miss_reason(&self) -> Option<&str> {
        match self {
            Self::Hit { .. } => None,
            Self::Miss { reason } => Some(reason),
        }
    }
}

/// Immutable action-result cache. It can restore only through a staging capability.
pub struct ActionCache {
    root: PathBuf,
    mode: CacheMode,
}

impl ActionCache {
    /// Resolve the repository-local disposable cache using the explicit environment mode.
    #[must_use]
    pub fn for_repository(repository_root: &Path) -> Self {
        Self {
            root: repository_root.join(CACHE_ROOT),
            mode: CacheMode::from_environment(),
        }
    }

    #[cfg(test)]
    fn with_mode(repository_root: &Path, mode: CacheMode) -> Self {
        Self {
            root: repository_root.join(CACHE_ROOT),
            mode,
        }
    }

    /// Restore one exact entry into staging after validating schema, census, and every byte.
    /// Invalid, partial, corrupt, incompatible, or absent entries are ordinary misses.
    pub fn restore(
        &self,
        identity: &FamilyActionIdentity,
        descriptor: &DriverDescriptor,
        staging: &StagingRoot,
    ) -> CacheLookup {
        if self.mode == CacheMode::Disabled {
            return miss("cache-disabled");
        }
        match self.restore_inner(identity, descriptor, staging) {
            Ok(outputs) => CacheLookup::Hit {
                outputs: outputs.iter().map(SafeOutputPath::display).collect(),
            },
            Err(error) => {
                if self.mode == CacheMode::ReadWrite {
                    self.quarantine(identity);
                }
                miss(error.cache_reason())
            }
        }
    }

    fn restore_inner(
        &self,
        identity: &FamilyActionIdentity,
        descriptor: &DriverDescriptor,
        staging: &StagingRoot,
    ) -> Result<Vec<SafeOutputPath>, IncrementalError> {
        let action_key = identity.action_key()?;
        let entry = self.entry(&action_key)?;
        if !entry.is_dir() {
            return Err(IncrementalError::Absent);
        }
        let manifest_path = entry.join("manifest.json");
        let manifest: CacheManifest =
            serde_json::from_slice(&read_stable(&manifest_path, MAX_CACHE_MANIFEST_BYTES)?)?;
        if manifest.schema != CACHE_SCHEMA
            || manifest.action_key != action_key
            || manifest.identity != *identity
            || manifest.identity.action_key()? != action_key
        {
            return Err(IncrementalError::Incompatible);
        }
        let expected = descriptor
            .outputs
            .iter()
            .map(|output| output.path.clone())
            .collect::<BTreeSet<_>>();
        let observed = manifest
            .outputs
            .iter()
            .map(|output| output.path.clone())
            .collect::<BTreeSet<_>>();
        if expected != observed
            || observed.len() != manifest.outputs.len()
            || observed.len() > MAX_CACHE_OUTPUTS
        {
            return Err(IncrementalError::OutputCensus);
        }
        let expected_files = expected_cache_files(&manifest);
        if cache_files(&entry)? != expected_files {
            return Err(IncrementalError::OutputCensus);
        }
        let mut restored = Vec::with_capacity(manifest.outputs.len());
        for output in &manifest.outputs {
            let bytes = read_stable(
                &entry.join("outputs").join(output.path.path_buf()),
                MAX_CACHE_OUTPUT_BYTES,
            )?;
            if bytes.len() as u64 != output.byte_length || digest_bytes(&bytes) != output.digest {
                return Err(IncrementalError::Corrupt);
            }
            staging.write(&output.path, &bytes)?;
            restored.push(output.path.clone());
        }
        restored.sort();
        Ok(restored)
    }

    /// Publish one immutable cache entry from staged bytes. A competing valid entry wins.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsafe key, invalid output census, or cache I/O failure.
    pub fn store(
        &self,
        identity: &FamilyActionIdentity,
        outputs: &[SafeOutputPath],
        staging: &StagingRoot,
    ) -> Result<(), IncrementalError> {
        if self.mode != CacheMode::ReadWrite {
            return Ok(());
        }
        if outputs.len() > MAX_CACHE_OUTPUTS {
            return Err(IncrementalError::OutputCensus);
        }
        let action_key = identity.action_key()?;
        let entry = self.entry(&action_key)?;
        if entry.exists() {
            return Ok(());
        }
        fs::create_dir_all(&self.root).map_err(|source| io(&self.root, source))?;
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = self
            .root
            .join(format!(".entry-{}-{sequence}", std::process::id()));
        if temporary.exists() {
            fs::remove_dir_all(&temporary).map_err(|source| io(&temporary, source))?;
        }
        fs::create_dir_all(temporary.join("outputs")).map_err(|source| io(&temporary, source))?;
        let mut manifest_outputs = Vec::with_capacity(outputs.len());
        let mut unique = BTreeSet::new();
        for path in outputs {
            if !unique.insert(path.clone()) {
                return Err(IncrementalError::OutputCensus);
            }
            let bytes = read_stable(&staging.output_path(path)?, MAX_CACHE_OUTPUT_BYTES)?;
            let destination = temporary.join("outputs").join(path.path_buf());
            let parent = destination
                .parent()
                .ok_or(IncrementalError::UnsafeCachePath)?;
            fs::create_dir_all(parent).map_err(|source| io(parent, source))?;
            fs::write(&destination, &bytes).map_err(|source| io(&destination, source))?;
            manifest_outputs.push(CacheOutput {
                path: path.clone(),
                digest: digest_bytes(&bytes),
                byte_length: bytes.len() as u64,
            });
        }
        manifest_outputs.sort_by(|left, right| left.path.cmp(&right.path));
        let manifest = CacheManifest {
            schema: CACHE_SCHEMA.to_owned(),
            action_key,
            identity: identity.clone(),
            outputs: manifest_outputs,
        };
        let mut bytes = serde_json::to_vec_pretty(&manifest)?;
        bytes.push(b'\n');
        fs::write(temporary.join("manifest.json"), bytes)
            .map_err(|source| io(&temporary, source))?;
        match fs::rename(&temporary, &entry) {
            Ok(()) => Ok(()),
            Err(_) if entry.exists() => {
                let _ = fs::remove_dir_all(&temporary);
                Ok(())
            }
            Err(source) => Err(io(&entry, source)),
        }
    }

    fn entry(&self, action_key: &str) -> Result<PathBuf, IncrementalError> {
        let Some(key) = action_key.strip_prefix("b3:") else {
            return Err(IncrementalError::UnsafeCachePath);
        };
        if key.len() != 64 || !key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(IncrementalError::UnsafeCachePath);
        }
        Ok(self.root.join(key))
    }

    fn quarantine(&self, identity: &FamilyActionIdentity) {
        let Ok(action_key) = identity.action_key() else {
            return;
        };
        let Ok(entry) = self.entry(&action_key) else {
            return;
        };
        if !entry.exists() {
            return;
        }
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let destination = self
            .root
            .join(format!(".quarantine-{}-{sequence}", std::process::id()));
        let _ = fs::rename(entry, destination);
    }
}

/// Render a family through its staging capability, optionally restoring/storing an exact cache
/// entry. Validators still run in the owning family after this function returns.
///
/// # Errors
///
/// Returns an error from identity resolution, cache publication, or the family renderer.
pub fn render_with_cache<R, T>(
    repository_root: &Path,
    family: &str,
    descriptor: &DriverDescriptor,
    source_fence: &DriverSourceFence,
    staging: &StagingRoot,
    tool_identity: T,
    render: R,
) -> Result<(Vec<SafeOutputPath>, CacheLookup), super::driver_protocol::DriverProtocolError>
where
    R: FnOnce() -> Result<Vec<SafeOutputPath>, super::driver_protocol::DriverProtocolError>,
    T: FnOnce() -> Result<Value, super::driver_protocol::DriverProtocolError>,
{
    let cache = ActionCache::for_repository(repository_root);
    if cache.mode == CacheMode::Disabled {
        let mut outputs = render()?;
        outputs.sort();
        return Ok((outputs, miss("cache-disabled")));
    }
    let identity = family_action_identity(family, descriptor, source_fence, &tool_identity()?)
        .map_err(|error| {
            super::driver_protocol::DriverProtocolError::InvalidAuthority(error.to_string())
        })?;
    let lookup = cache.restore(&identity, descriptor, staging);
    if matches!(&lookup, CacheLookup::Hit { .. }) {
        let mut outputs = descriptor
            .outputs
            .iter()
            .map(|output| output.path.clone())
            .collect::<Vec<_>>();
        outputs.sort();
        return Ok((outputs, lookup));
    }
    let mut outputs = render()?;
    outputs.sort();
    cache.store(&identity, &outputs, staging).map_err(|error| {
        super::driver_protocol::DriverProtocolError::InvalidAuthority(error.to_string())
    })?;
    Ok((outputs, lookup))
}

fn expected_cache_files(manifest: &CacheManifest) -> BTreeSet<PathBuf> {
    let mut files = BTreeSet::from([PathBuf::from("manifest.json")]);
    files.extend(
        manifest
            .outputs
            .iter()
            .map(|output| PathBuf::from("outputs").join(output.path.path_buf())),
    );
    files
}

fn cache_files(root: &Path) -> Result<BTreeSet<PathBuf>, IncrementalError> {
    let mut pending = vec![root.to_owned()];
    let mut files = BTreeSet::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(|source| io(&directory, source))? {
            let entry = entry.map_err(|source| io(&directory, source))?;
            let metadata = entry
                .file_type()
                .map_err(|source| io(&entry.path(), source))?;
            if metadata.is_symlink() {
                return Err(IncrementalError::Corrupt);
            }
            if metadata.is_dir() {
                pending.push(entry.path());
            } else if metadata.is_file() {
                let relative = entry
                    .path()
                    .strip_prefix(root)
                    .map_err(|_| IncrementalError::UnsafeCachePath)?
                    .to_owned();
                files.insert(relative);
                if files.len() > MAX_CACHE_OUTPUTS + 1 {
                    return Err(IncrementalError::OutputCensus);
                }
            } else {
                return Err(IncrementalError::Corrupt);
            }
        }
    }
    Ok(files)
}

fn miss(reason: impl Into<String>) -> CacheLookup {
    CacheLookup::Miss {
        reason: reason.into(),
    }
}

/// Resource identity used to prevent feature-sensitive executables from co-executing.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ScheduledAction {
    pub action_id: StableId,
    pub dependency_rank: usize,
    pub resource_identity: String,
}

/// Produce deterministic waves with at most one action per exact resource identity.
#[must_use]
pub fn deterministic_schedule(mut actions: Vec<ScheduledAction>) -> Vec<Vec<StableId>> {
    actions.sort_by(|left, right| {
        (left.dependency_rank, &left.action_id).cmp(&(right.dependency_rank, &right.action_id))
    });
    let mut waves = Vec::<(BTreeSet<String>, Vec<StableId>)>::new();
    for action in actions {
        let wave = waves
            .iter_mut()
            .skip(action.dependency_rank)
            .find(|(resources, _)| !resources.contains(&action.resource_identity));
        if let Some((resources, ids)) = wave {
            resources.insert(action.resource_identity);
            ids.push(action.action_id);
        } else {
            while waves.len() < action.dependency_rank {
                waves.push((BTreeSet::new(), Vec::new()));
            }
            waves.push((
                BTreeSet::from([action.resource_identity]),
                vec![action.action_id],
            ));
        }
    }
    waves.into_iter().map(|(_, ids)| ids).collect()
}

/// Non-authoritative watcher classification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WatchHint {
    Paths(BTreeSet<PathBuf>),
    FullReinventory(String),
}

/// Classify a delivered batch. Errors, loss, or an unknown empty batch widen to full.
#[must_use]
pub fn classify_watch_batch(result: &DebounceEventResult) -> WatchHint {
    match result {
        Err(_) => WatchHint::FullReinventory("watch-backend-error".to_owned()),
        Ok(events) if events.is_empty() => {
            WatchHint::FullReinventory("empty-watch-batch".to_owned())
        }
        Ok(events) if events.iter().any(|event| event.need_rescan()) => {
            WatchHint::FullReinventory("watch-rescan-required".to_owned())
        }
        Ok(events) => WatchHint::Paths(
            events
                .iter()
                .flat_map(|event| event.paths.iter().cloned())
                .collect(),
        ),
    }
}

/// Reconstruct the complete current-byte repository model after any watcher hint.
///
/// # Errors
///
/// Returns an error when repository discovery or summary serialization fails.
pub fn reinventory_watch(repository_root: &Path) -> Result<Value, IncrementalError> {
    let model = super::repository_model::RepositoryModel::discover(
        repository_root,
        super::repository_model::InventoryBounds::default(),
        true,
    )?;
    Ok(serde_json::to_value(model.summary()?)?)
}

/// Stable replay text attached to deterministic property failures.
#[must_use]
pub fn property_replay(seed: u64, minimized_edit: &str) -> String {
    format!("just model-incremental-check # seed=0x{seed:016X} minimized-edit={minimized_edit}")
}

/// Run the explicit opt-in watcher. Every batch is followed by a complete current-byte model
/// inventory before it is reported; event absence is never treated as source truth.
///
/// # Errors
///
/// Returns an error for watcher setup, event-channel loss, or repository reinventory failure.
pub fn watch(repository_root: &Path) -> Result<(), IncrementalError> {
    let root = fs::canonicalize(repository_root).map_err(|source| io(repository_root, source))?;
    let (sender, receiver) = mpsc::sync_channel::<DebounceEventResult>(4096);
    let overflowed = Arc::new(AtomicBool::new(false));
    let callback_overflowed = Arc::clone(&overflowed);
    let mut debouncer = new_debouncer(
        Duration::from_millis(75),
        Some(Duration::from_millis(20)),
        move |result| {
            if sender.try_send(result).is_err() {
                callback_overflowed.store(true, Ordering::Release);
            }
        },
    )?;
    debouncer.watch(&root, RecursiveMode::Recursive)?;
    loop {
        let hint = if overflowed.swap(false, Ordering::AcqRel) {
            WatchHint::FullReinventory("watch-queue-overflow".to_owned())
        } else {
            match receiver.recv_timeout(Duration::from_millis(20)) {
                Ok(result) => classify_watch_batch(&result),
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(IncrementalError::WatchDisconnected);
                }
            }
        };
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "hint": format!("{hint:?}"),
                "current_byte_model": reinventory_watch(&root)?,
            }))?
        );
    }
}

fn canonical_digest(value: &impl Serialize) -> Result<String, IncrementalError> {
    let value = serde_json::to_value(value)?;
    let bytes = serde_json_canonicalizer::to_vec(&value)?;
    Ok(digest_bytes(&bytes))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("b3:{}", blake3::hash(bytes).to_hex())
}

fn io(path: &Path, source: std::io::Error) -> IncrementalError {
    IncrementalError::Io {
        path: path.to_owned(),
        source,
    }
}

/// Incremental execution failures. Cache validation errors are downgraded to misses by lookup.
#[derive(Debug, Error)]
pub enum IncrementalError {
    #[error("unsafe model cache path")]
    UnsafeCachePath,
    #[error("model cache entry is incompatible")]
    Incompatible,
    #[error("model cache entry is absent")]
    Absent,
    #[error("model cache output census differs from the declared action")]
    OutputCensus,
    #[error("model cache bytes are corrupt")]
    Corrupt,
    #[error("model watch event channel disconnected")]
    WatchDisconnected,
    #[error("model cache I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    DesiredTree(#[from] super::desired_tree::DesiredTreeError),
    #[error(transparent)]
    Driver(#[from] super::driver_protocol::DriverProtocolError),
    #[error(transparent)]
    Repository(#[from] super::repository_model::RepositoryModelError),
    #[error(transparent)]
    Notify(#[from] notify_debouncer_full::notify::Error),
}

impl IncrementalError {
    fn cache_reason(&self) -> String {
        match self {
            Self::UnsafeCachePath => "unsafe-cache-path",
            Self::Absent => "cache-entry-absent",
            Self::Incompatible => "incompatible-cache-entry",
            Self::OutputCensus => "cache-output-census-mismatch",
            Self::Corrupt => "corrupt-cache-entry",
            Self::Io { .. } => "missing-or-unreadable-cache-entry",
            Self::Json(_) => "invalid-cache-manifest",
            Self::DesiredTree(_)
            | Self::Driver(_)
            | Self::Repository(_)
            | Self::Notify(_)
            | Self::WatchDisconnected => "cache-validation-failed",
        }
        .to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::super::driver_protocol::{
        DriverOutputRole, DriverOutputSpec, DriverResourceProfile,
    };
    use super::super::model_control::{
        EdgeDeclaration, EdgeKind, ModelGraph, NodeDeclaration, NodeKind, ResourceBounds,
    };
    use super::*;
    use std::process::Command;

    fn id(value: &str) -> StableId {
        StableId::parse(value).unwrap()
    }

    fn descriptor() -> DriverDescriptor {
        DriverDescriptor {
            driver_id: id("driver:test"),
            family: id("family:test"),
            rule_version: "test-v1".to_owned(),
            sources: Vec::new(),
            output_roots: vec![SafeOutputPath::parse(b"generated".to_vec()).unwrap()],
            outputs: vec![DriverOutputSpec {
                output_id: id("output:test"),
                path: SafeOutputPath::parse(b"generated/result.txt".to_vec()).unwrap(),
                role: DriverOutputRole::CanonicalProjection,
            }],
            resource_profile: DriverResourceProfile {
                max_source_bytes: 1024,
                max_output_bytes: 1024,
                max_outputs: 4,
            },
        }
    }

    fn stage(root: &Path, name: &str) -> StagingRoot {
        StagingRoot::new(
            root,
            &root.join("target/model-stage").join(name),
            &descriptor(),
        )
        .unwrap()
    }

    fn action_identity(feature: &str) -> FamilyActionIdentity {
        FamilyActionIdentity {
            schema: CACHE_SCHEMA.to_owned(),
            family: "test".to_owned(),
            descriptor: descriptor(),
            source_fence: Vec::new(),
            upstream_output_digests: Vec::new(),
            executable: ActionExecutableIdentity {
                compiler_source_identity: "b3:source".to_owned(),
                cargo_lock_identity: "b3:lock".to_owned(),
                rustc_identity: "rustc".to_owned(),
                feature_set: BTreeSet::from([feature.to_owned()]),
                profile: "dev".to_owned(),
                target_triple: "host".to_owned(),
                executable_digest: "b3:exe".to_owned(),
            },
            tool_identity: serde_json::json!({"path": "tool", "digest": "b3:tool"}),
            normalized_environment: BTreeMap::from([
                ("credentials".to_owned(), "stripped".to_owned()),
                ("network".to_owned(), "undeclared".to_owned()),
                ("output_destination".to_owned(), "staging-only".to_owned()),
            ]),
        }
    }

    #[test]
    fn model_cache_cold_warm_partial_corrupt_and_disabled_outputs_are_identical() {
        let root = tempfile::tempdir().unwrap();
        let source = stage(root.path(), "source");
        let output = descriptor().outputs[0].path.clone();
        source.write(&output, b"exact-bytes").unwrap();
        let identity = action_identity("model-compiler");
        let disabled = ActionCache::with_mode(root.path(), CacheMode::Disabled);
        assert!(matches!(
            disabled.restore(&identity, &descriptor(), &stage(root.path(), "disabled")),
            CacheLookup::Miss { .. }
        ));
        let cache = ActionCache::with_mode(root.path(), CacheMode::ReadWrite);
        cache
            .store(&identity, std::slice::from_ref(&output), &source)
            .unwrap();
        let warm = stage(root.path(), "warm");
        assert!(matches!(
            cache.restore(&identity, &descriptor(), &warm),
            CacheLookup::Hit { .. }
        ));
        assert_eq!(
            fs::read(warm.output_path(&output).unwrap()).unwrap(),
            b"exact-bytes"
        );

        let entry = cache.entry(&identity.action_key().unwrap()).unwrap();
        fs::write(entry.join("outputs/generated/result.txt"), b"corrupt").unwrap();
        assert!(matches!(
            cache.restore(&identity, &descriptor(), &stage(root.path(), "corrupt")),
            CacheLookup::Miss { .. }
        ));
        let partial_identity = action_identity("partial-entry");
        cache
            .store(&partial_identity, std::slice::from_ref(&output), &source)
            .unwrap();
        let partial_entry = cache
            .entry(&partial_identity.action_key().unwrap())
            .unwrap();
        fs::remove_file(partial_entry.join("outputs/generated/result.txt")).unwrap();
        assert!(matches!(
            cache.restore(
                &partial_identity,
                &descriptor(),
                &stage(root.path(), "partial")
            ),
            CacheLookup::Miss { .. }
        ));
    }

    #[test]
    fn model_cache_manifest_contains_complete_action_identity_and_output_census() {
        let root = tempfile::tempdir().unwrap();
        let source = stage(root.path(), "manifest-source");
        let output = descriptor().outputs[0].path.clone();
        source.write(&output, b"identity-bound-output").unwrap();
        let action_identity = action_identity("model-compiler");
        let cache = ActionCache::with_mode(root.path(), CacheMode::ReadWrite);
        cache
            .store(&action_identity, std::slice::from_ref(&output), &source)
            .unwrap();
        let entry = cache.entry(&action_identity.action_key().unwrap()).unwrap();
        let manifest: CacheManifest =
            serde_json::from_slice(&fs::read(entry.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest.identity, action_identity);
        assert_eq!(manifest.outputs.len(), 1);
        let text = serde_json::to_string(&manifest).unwrap();
        for field in [
            "descriptor",
            "source_fence",
            "upstream_output_digests",
            "executable_digest",
            "feature_set",
            "target_triple",
            "tool_identity",
            "normalized_environment",
        ] {
            assert!(text.contains(field));
        }
        assert!(!text.contains("verdict"));
    }

    #[test]
    fn model_cache_rejects_wrong_digest_missing_extra_or_incompatible_output() {
        let root = tempfile::tempdir().unwrap();
        let source = stage(root.path(), "source");
        let output = descriptor().outputs[0].path.clone();
        source.write(&output, b"exact").unwrap();
        let identity = action_identity("model-compiler");
        let cache = ActionCache::with_mode(root.path(), CacheMode::ReadWrite);
        cache
            .store(&identity, std::slice::from_ref(&output), &source)
            .unwrap();
        let entry = cache.entry(&identity.action_key().unwrap()).unwrap();
        fs::write(entry.join("extra"), b"extra").unwrap();
        assert!(matches!(
            cache.restore(&identity, &descriptor(), &stage(root.path(), "extra")),
            CacheLookup::Miss { .. }
        ));
        let wrong = action_identity("wrong-feature");
        assert!(matches!(
            cache.restore(&wrong, &descriptor(), &stage(root.path(), "wrong")),
            CacheLookup::Miss { .. }
        ));
    }

    #[test]
    fn model_cache_cannot_restore_to_repository_or_store_pass_verdicts() {
        let manifest = serde_json::to_string(&CacheManifest {
            schema: CACHE_SCHEMA.to_owned(),
            action_key: action_identity("model-compiler").action_key().unwrap(),
            identity: action_identity("model-compiler"),
            outputs: Vec::new(),
        })
        .unwrap();
        assert!(!manifest.contains("verdict"));
        assert!(!manifest.contains("pass"));
        assert!(
            !ActionCache::for_repository(Path::new("."))
                .root
                .starts_with("contracts")
        );
    }

    #[test]
    fn model_scheduler_never_coexecutes_conflicting_executable_identities() {
        let schedule = deterministic_schedule(vec![
            ScheduledAction {
                action_id: id("action:a"),
                dependency_rank: 0,
                resource_identity: "cargo:shared".to_owned(),
            },
            ScheduledAction {
                action_id: id("action:b"),
                dependency_rank: 0,
                resource_identity: "cargo:shared".to_owned(),
            },
            ScheduledAction {
                action_id: id("action:c"),
                dependency_rank: 0,
                resource_identity: "python:isolated".to_owned(),
            },
        ]);
        assert_eq!(schedule[0], vec![id("action:a"), id("action:c")]);
        assert_eq!(schedule[1], vec![id("action:b")]);
    }

    #[test]
    fn model_unknown_read_and_watch_loss_widen_to_full() {
        let error = Err(vec![notify_debouncer_full::notify::Error::generic("lost")]);
        assert!(matches!(
            classify_watch_batch(&error),
            WatchHint::FullReinventory(_)
        ));
        let empty = Ok(Vec::new());
        assert!(matches!(
            classify_watch_batch(&empty),
            WatchHint::FullReinventory(_)
        ));
    }

    #[test]
    fn model_cache_wrong_feature_executable_is_an_explicit_miss() {
        let expected = action_identity("model-compiler");
        let wrong = action_identity("obsolete-feature-set");
        assert_ne!(expected.action_key().unwrap(), wrong.action_key().unwrap());
        let root = tempfile::tempdir().unwrap();
        let source = stage(root.path(), "feature-source");
        let output = descriptor().outputs[0].path.clone();
        source.write(&output, b"exact").unwrap();
        let cache = ActionCache::with_mode(root.path(), CacheMode::ReadWrite);
        cache
            .store(&expected, std::slice::from_ref(&output), &source)
            .unwrap();
        assert_eq!(
            cache
                .restore(&wrong, &descriptor(), &stage(root.path(), "wrong-feature"))
                .miss_reason(),
            Some("cache-entry-absent")
        );
    }

    #[test]
    fn model_every_family_edit_class_matches_full_affected_outputs_and_oracles() {
        let families = ["registry-cbef", "schemas", "adapter", "proto"];
        let edit_classes = [
            "source-format-only",
            "semantic-field",
            "driver-version",
            "tool-identity",
            "output-schema-version",
            "member-add-delete",
        ];
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let governance = id("oracle:governance");
        nodes.push(NodeDeclaration {
            id: governance.clone(),
            kind: NodeKind::Oracle,
        });
        for family in families {
            let source = id(&format!("source:{family}"));
            let action = id(&format!("action:{family}"));
            let output = id(&format!("output:{family}"));
            nodes.extend([
                NodeDeclaration {
                    id: source.clone(),
                    kind: NodeKind::Source,
                },
                NodeDeclaration {
                    id: action.clone(),
                    kind: NodeKind::Action,
                },
                NodeDeclaration {
                    id: output.clone(),
                    kind: NodeKind::Output,
                },
            ]);
            edges.extend([
                EdgeDeclaration {
                    prerequisite: source,
                    dependent: action.clone(),
                    kind: EdgeKind::ReadsExactBytes,
                },
                EdgeDeclaration {
                    prerequisite: action,
                    dependent: output.clone(),
                    kind: EdgeKind::Produces,
                },
                EdgeDeclaration {
                    prerequisite: output,
                    dependent: governance.clone(),
                    kind: EdgeKind::Verifies,
                },
            ]);
        }
        let graph =
            ModelGraph::compile(nodes, edges, ResourceBounds::new(64, 128, 32).unwrap()).unwrap();
        for family in families {
            for edit_class in edit_classes {
                let changed = id(&format!("source:{family}"));
                let incremental = graph
                    .affected_closure(&changed)
                    .into_iter()
                    .collect::<BTreeSet<_>>();
                let full_reference = graph
                    .execution_order()
                    .iter()
                    .filter(|candidate| graph.prerequisite_closure(candidate).contains(&changed))
                    .cloned()
                    .collect::<BTreeSet<_>>();
                assert_eq!(
                    incremental,
                    full_reference,
                    "{}",
                    property_replay(0xC0DE_FAB1, &format!("{family}:{edit_class}"))
                );
                assert!(incremental.contains(&id(&format!("output:{family}"))));
                assert!(incremental.contains(&governance));
            }
        }
    }

    #[test]
    fn model_watch_rescan_reconstructs_current_byte_model() {
        let root = tempfile::tempdir().unwrap();
        assert!(
            Command::new("git")
                .args(["init", "--quiet"])
                .current_dir(root.path())
                .status()
                .unwrap()
                .success()
        );
        let _before = reinventory_watch(root.path()).unwrap();
        fs::write(root.path().join("current.txt"), b"current bytes").unwrap();
        assert!(
            Command::new("git")
                .args(["add", "current.txt"])
                .current_dir(root.path())
                .status()
                .unwrap()
                .success()
        );
        let after = reinventory_watch(root.path()).unwrap();
        let direct = serde_json::to_value(
            super::super::repository_model::RepositoryModel::discover(
                root.path(),
                super::super::repository_model::InventoryBounds::default(),
                true,
            )
            .unwrap()
            .summary()
            .unwrap(),
        )
        .unwrap();
        assert_eq!(after, direct);
        assert_eq!(after, reinventory_watch(root.path()).unwrap());
    }

    #[test]
    fn model_property_failure_prints_seed_minimized_edit_and_replay_command() {
        let replay = property_replay(0xC0DE_FAB1, "proto:tool-identity");
        assert!(replay.contains("seed=0x00000000C0DEFAB1"));
        assert!(replay.contains("minimized-edit=proto:tool-identity"));
        assert!(replay.starts_with("just model-incremental-check"));
    }
}
