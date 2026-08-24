//! Bounded descriptor-relative source inventory and Merkle root construction.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rusqlite::{OptionalExtension as _, params};
use thiserror::Error;

use crate::identity::{IdentityError, WorkspacePath, source_file_identity};
use crate::operational_store::{OperationalStore, OperationalStoreError};
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

/// Cooperative cancellation observed at every directory entry.
#[derive(Debug, Default)]
pub struct InventoryCancellation(AtomicBool);

impl InventoryCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub use crate::registries::{
    GitInventoryClassification as InventoryClassification, InventoryFileKind,
    InventoryInclusionState as InclusionState,
};

/// All Lifecycle §34 fields, detached from filesystem and Git library types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceInventoryRecord {
    pub path: WorkspacePath,
    pub git_repo_path_bytes: Option<Vec<u8>>,
    pub filesystem_identity: Option<[u8; 16]>,
    pub file_id: Option<[u8; 16]>,
    pub content_digest: Option<[u8; 32]>,
    pub byte_length: u64,
    pub file_kind: InventoryFileKind,
    pub language: Option<&'static str>,
    pub classification: InventoryClassification,
    pub inclusion: InclusionState,
    pub git_blob_oid: Option<Vec<u8>>,
    pub current_file_owner: Option<[u8; 16]>,
}

/// One coherent current inventory and its Merkle root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceInventory {
    pub workspace_id: [u8; 16],
    pub source_generation: u64,
    pub records: Vec<SourceInventoryRecord>,
    pub digest: [u8; 32],
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
}

/// Generic non-Git walker. WP17 may replace classification, never authorization.
pub struct InventoryWalker {
    limits: InventoryLimits,
    metrics: InventoryMetrics,
}

impl InventoryWalker {
    #[must_use]
    pub const fn new(limits: InventoryLimits) -> Self {
        Self {
            limits,
            metrics: InventoryMetrics {
                files: 0,
                directories: 0,
                bytes_considered: 0,
                excluded_files: 0,
                duration_micros: 0,
            },
        }
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
        cancellation: &InventoryCancellation,
    ) -> Result<SourceInventory, InventoryError> {
        require_source_generation(store, root.workspace_id(), source_generation)?;
        let started = Instant::now();
        let mut records = Vec::new();
        let mut stack = vec![(Vec::<Vec<u8>>::new(), 0_u32)];
        self.metrics = InventoryMetrics::default();
        while let Some((components, depth)) = stack.pop() {
            self.check_progress(started, cancellation)?;
            self.metrics.directories = self.metrics.directories.saturating_add(1);
            if self.metrics.directories > self.limits.maximum_directory_count {
                return Err(InventoryError::BoundExceeded("directory-count"));
            }
            let raw = join_components(&components);
            let platform_path = if components.is_empty() {
                None
            } else {
                Some(PlatformPath::from_raw_relative_bytes(
                    root.platform_code(),
                    raw,
                )?)
            };
            let entries = root.list_directory(
                platform_path.as_ref(),
                self.limits.maximum_entries_per_directory,
            )?;
            for entry in entries.into_iter().rev() {
                self.check_progress(started, cancellation)?;
                let mut child = components.clone();
                child.push(entry.name.clone());
                if entry.name == b".git" {
                    continue;
                }
                if excluded_directory(&entry.name)
                    && entry.kind == SecureDirectoryEntryKind::Directory
                {
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
                        self.add_regular(root, &child, entry.size, &mut records)?;
                    }
                    SecureDirectoryEntryKind::Symlink | SecureDirectoryEntryKind::Other => {
                        self.add_excluded(root, &child, entry.kind, entry.size, &mut records)?;
                    }
                }
            }
        }
        records.sort_by(|left, right| {
            left.path
                .raw_relative_path_bytes
                .cmp(&right.path.raw_relative_path_bytes)
        });
        let digest = merkle_inventory_digest(&records);
        persist_inventory(store, root.workspace_id(), source_generation, &records)?;
        self.metrics.duration_micros =
            u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        Ok(SourceInventory {
            workspace_id: root.workspace_id(),
            source_generation,
            records,
            digest,
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
        let workspace_path = root.workspace_path(&path)?;
        if self.metrics.bytes_considered > self.limits.maximum_total_bytes_considered {
            return Err(InventoryError::BoundExceeded("total-bytes"));
        }
        let (digest, filesystem_identity, inclusion) = match root
            .read_stable_file(&path, ORDINARY_SOURCE_MAXIMUM_BYTES)
        {
            Ok(read) => (
                Some(*blake3::hash(&read.bytes).as_bytes()),
                Some(filesystem_identity(
                    read.metadata.device,
                    read.metadata.inode,
                )),
                InclusionState::Included,
            ),
            Err(StableReadError::SizeLimitExceeded { .. }) => {
                self.metrics.excluded_files = self.metrics.excluded_files.saturating_add(1);
                (None, None, InclusionState::ExcludedSizeLimit)
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
            byte_length: observed_size,
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
            path: root.workspace_path(&platform)?,
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
        cancellation: &InventoryCancellation,
    ) -> Result<(), InventoryError> {
        if cancellation.is_cancelled() {
            return Err(InventoryError::Cancelled);
        }
        if started.elapsed() > self.limits.maximum_duration {
            return Err(InventoryError::BoundExceeded("duration"));
        }
        Ok(())
    }
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
    let mut leaf_paths = Vec::new();
    for (path, digest) in leaves {
        let (parent, name) = split_parent(&path);
        directories
            .entry(parent)
            .or_default()
            .push((name, 1, digest));
        leaf_paths.push(path);
    }
    let mut directory_paths = directories.keys().cloned().collect::<Vec<_>>();
    directory_paths.push(Vec::new());
    for path in leaf_paths {
        let mut parent = split_parent(&path).0;
        while !parent.is_empty() {
            if !directory_paths.contains(&parent) {
                directory_paths.push(parent.clone());
            }
            parent = split_parent(&parent).0;
        }
    }
    directory_paths.sort_by_key(|path| {
        std::cmp::Reverse(if path.is_empty() {
            0
        } else {
            path.split(|byte| *byte == b'/').count()
        })
    });
    directory_paths.dedup();
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
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.inventory.file.v1\0");
    hash_length_prefixed(&mut hasher, path);
    hasher.update(&content_digest.unwrap_or([0; 32]));
    hasher.update(&byte_length.to_be_bytes());
    hasher.update(&file_kind.to_be_bytes());
    hasher.update(&classification.to_be_bytes());
    hasher.update(&inclusion.to_be_bytes());
    *hasher.finalize().as_bytes()
}

fn inventory_directory_digest(children: &[(Vec<u8>, u8, [u8; 32])]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"codefabric.inventory.directory.v1\0");
    for (name, kind, digest) in children {
        hash_length_prefixed(&mut hasher, name);
        hasher.update(&[*kind]);
        hasher.update(digest);
    }
    *hasher.finalize().as_bytes()
}

fn hash_length_prefixed(hasher: &mut blake3::Hasher, bytes: &[u8]) {
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

fn excluded_directory(name: &[u8]) -> bool {
    matches!(
        name,
        b"target" | b".venv" | b"node_modules" | b"__pycache__"
    )
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
            .walk_and_persist(&root, &mut store, 0, &InventoryCancellation::default())
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
            .walk_and_persist(&root, &mut store, 0, &InventoryCancellation::default())
            .unwrap();
        assert_ne!(first.digest, second.digest);
        assert_ne!(
            first.records[1].content_digest,
            second.records[1].content_digest
        );

        fs::rename(root_path.join("README.md"), root_path.join("RENAMED.md")).unwrap();
        let renamed = walker
            .walk_and_persist(&root, &mut store, 0, &InventoryCancellation::default())
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
                &InventoryCancellation::default()
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
            .walk_and_persist(&root, &mut store, 0, &InventoryCancellation::default())
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

        let cancelled = InventoryCancellation::default();
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
                    .walk_and_persist(&root, &mut store, 0, &InventoryCancellation::default())
                    .is_err()
            );
        }

        let inventory = InventoryWalker::new(InventoryLimits::default())
            .walk_and_persist(&root, &mut store, 0, &InventoryCancellation::default())
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
            .walk_and_persist(&root, &mut store, 0, &InventoryCancellation::default())
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
