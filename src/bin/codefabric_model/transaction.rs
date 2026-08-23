//! Worktree-local crash-consistent reconciliation for model-derived outputs.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::unix::ffi::OsStringExt as _;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Component, Path, PathBuf};

use rustix::fs::{FlockOperation, flock};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::aggregate_driver;
use super::repository_model::{ArtifactRole, InventoryBounds, RepositoryModel, read_stable};

const ADMIN_DIRECTORY: &str = "codefabric-model/transaction-v1";
const JOURNAL_FILE: &str = "journal.json";
const PREVIOUS_JOURNAL_FILE: &str = "journal.previous.json";
const COMMITTED_TREE_FILE: &str = "committed-tree.json";
const LOCK_FILE: &str = "program.lock";
const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
const MAX_JOURNAL_BYTES: usize = 16 * 1024 * 1024;

/// One durable transaction phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum TransactionPhase {
    Prepared,
    Applying,
    Committed,
}

/// Per-destination durable phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum EntryPhase {
    Planned,
    TemporarySynced,
    OldBackedUp,
    NewInstalled,
}

/// One complete journal destination.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalEntry {
    path: String,
    old_digest: Option<String>,
    new_digest: Option<String>,
    temporary_name: String,
    backup_name: String,
    phase: EntryPhase,
}

/// Durable recovery record. It contains enough information to restore the complete old tree or
/// finish a transaction already marked committed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionJournal {
    schema_version: u64,
    recovery_version: String,
    transaction_id: String,
    source_identity: String,
    desired_tree_identity: String,
    phase: TransactionPhase,
    entries: Vec<JournalEntry>,
}

/// Last successfully committed model-owned tree. This is private transaction state, not release
/// or governance authority.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommittedTree {
    schema_version: u64,
    desired_tree_identity: String,
    outputs: BTreeMap<String, String>,
}

/// Bounded synchronization result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SyncReport {
    pub desired_tree_identity: String,
    pub added: usize,
    pub replaced: usize,
    pub deleted_stale: usize,
    pub unchanged: usize,
    pub transaction_applied: bool,
}

#[derive(Clone, Debug)]
struct PlannedEntry {
    path: String,
    old_bytes: Option<Vec<u8>>,
    new_bytes: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
struct SyncPlan {
    source_identity: String,
    desired_tree_identity: String,
    entries: Vec<PlannedEntry>,
    desired_outputs: BTreeMap<String, String>,
    unchanged: usize,
}

#[derive(Clone, Debug)]
struct TransactionPaths {
    repository_root: PathBuf,
    #[cfg_attr(not(test), allow(dead_code))] // Retained for linked-worktree topology proofs.
    git_dir: PathBuf,
    #[cfg_attr(not(test), allow(dead_code))] // Retained for linked-worktree topology proofs.
    common_dir: PathBuf,
    admin_root: PathBuf,
}

/// Shared-reader lock. Closing the descriptor releases the kernel lock; Drop also requests an
/// explicit unlock so nested tests can prove the protocol deterministically.
pub struct ReadGuard {
    lock: Option<File>,
    outer: bool,
}

impl Drop for ReadGuard {
    fn drop(&mut self) {
        LOCK_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
        if self.outer {
            if let Some(lock) = &self.lock {
                let _ = flock(lock, FlockOperation::Unlock);
            }
            clear_lock_state();
        }
    }
}

struct WriteGuard {
    lock: Option<File>,
    outer: bool,
}

impl Drop for WriteGuard {
    fn drop(&mut self) {
        LOCK_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
        if self.outer {
            if let Some(lock) = &self.lock {
                let _ = flock(lock, FlockOperation::Unlock);
            }
            clear_lock_state();
        }
    }
}

thread_local! {
    static LOCK_DEPTH: Cell<u32> = const { Cell::new(0) };
    static LOCK_MODE: Cell<u8> = const { Cell::new(0) };
    static LOCK_ADMIN: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

fn clear_lock_state() {
    LOCK_DEPTH.with(|depth| depth.set(0));
    LOCK_MODE.with(|mode| mode.set(0));
    LOCK_ADMIN.with(|admin| *admin.borrow_mut() = None);
}

fn nested_guard(paths: &TransactionPaths, requested_mode: u8) -> Result<bool, TransactionError> {
    let depth = LOCK_DEPTH.with(Cell::get);
    if depth == 0 {
        return Ok(false);
    }
    let same_admin = LOCK_ADMIN.with(|admin| admin.borrow().as_ref() == Some(&paths.admin_root));
    let mode = LOCK_MODE.with(Cell::get);
    if !same_admin || requested_mode == 2 && mode != 2 {
        return Err(TransactionError::NestedLock);
    }
    LOCK_DEPTH.with(|depth| depth.set(depth.get().saturating_add(1)));
    Ok(true)
}

fn record_outer_lock(paths: &TransactionPaths, mode: u8) {
    LOCK_DEPTH.with(|depth| depth.set(1));
    LOCK_MODE.with(|state| state.set(mode));
    LOCK_ADMIN.with(|admin| *admin.borrow_mut() = Some(paths.admin_root.clone()));
}

/// Obtain the shared protocol lock after performing exclusive recovery. Supported readers call
/// this before consuming committed generated outputs.
///
/// # Errors
///
/// Returns an error for repository topology, durable-state, locking, or recovery failures.
pub fn read_guard(repository_root: &Path) -> Result<ReadGuard, TransactionError> {
    let paths = transaction_paths(repository_root)?;
    if nested_guard(&paths, 1)? {
        return Ok(ReadGuard {
            lock: None,
            outer: false,
        });
    }
    {
        let _exclusive = acquire_write(&paths)?;
        recover_locked(&paths)?;
    }
    let lock = open_lock(&paths)?;
    flock(&lock, FlockOperation::LockShared).map_err(TransactionError::Lock)?;
    record_outer_lock(&paths, 1);
    Ok(ReadGuard {
        lock: Some(lock),
        outer: true,
    })
}

/// Compile, validate, and atomically reconcile the complete aggregate `DesiredTree`.
///
/// # Errors
///
/// Returns an error for model compilation, source drift, user edits, unsafe paths, durable-state
/// failures, or a transaction that cannot be recovered safely.
pub fn sync(repository_root: &Path) -> Result<SyncReport, TransactionError> {
    let paths = transaction_paths(repository_root)?;
    let _exclusive = acquire_write(&paths)?;
    recover_locked(&paths)?;
    let plan = compile_sync_plan(&paths)?;
    apply_plan(&paths, plan, None)
}

fn transaction_paths(repository_root: &Path) -> Result<TransactionPaths, TransactionError> {
    let repository_root =
        fs::canonicalize(repository_root).map_err(|source| io(repository_root, source))?;
    let inventory = super::model_git_state::inventory(&repository_root)?;
    let git_dir = inventory
        .topology
        .git_dir
        .map(|bytes| PathBuf::from(OsString::from_vec(bytes)))
        .ok_or(TransactionError::GitTopology)?;
    let common_dir = inventory
        .topology
        .common_dir
        .map(|bytes| PathBuf::from(OsString::from_vec(bytes)))
        .ok_or(TransactionError::GitTopology)?;
    let git_dir = fs::canonicalize(&git_dir).map_err(|source| io(&git_dir, source))?;
    let common_dir = fs::canonicalize(&common_dir).map_err(|source| io(&common_dir, source))?;
    let admin_root = git_dir.join(ADMIN_DIRECTORY);
    create_private_directory(&admin_root)?;
    Ok(TransactionPaths {
        repository_root,
        git_dir,
        common_dir,
        admin_root,
    })
}

fn create_private_directory(path: &Path) -> Result<(), TransactionError> {
    if path.exists() {
        let metadata = fs::symlink_metadata(path).map_err(|source| io(path, source))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(TransactionError::UnsafeAdminRoot(path.to_path_buf()));
        }
    } else {
        fs::create_dir_all(path).map_err(|source| io(path, source))?;
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|source| io(path, source))?;
    Ok(())
}

fn open_lock(paths: &TransactionPaths) -> Result<File, TransactionError> {
    let path = paths.admin_root.join(LOCK_FILE);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&path)
        .map_err(|source| io(&path, source))?;
    let metadata = file.metadata().map_err(|source| io(&path, source))?;
    if !metadata.is_file() {
        return Err(TransactionError::UnsafeAdminRoot(path));
    }
    Ok(file)
}

fn acquire_write(paths: &TransactionPaths) -> Result<WriteGuard, TransactionError> {
    if nested_guard(paths, 2)? {
        return Ok(WriteGuard {
            lock: None,
            outer: false,
        });
    }
    let lock = open_lock(paths)?;
    flock(&lock, FlockOperation::LockExclusive).map_err(TransactionError::Lock)?;
    record_outer_lock(paths, 2);
    Ok(WriteGuard {
        lock: Some(lock),
        outer: true,
    })
}

fn source_identity(model: &RepositoryModel) -> Result<String, TransactionError> {
    let sources = model
        .claims
        .values()
        .filter(|claim| {
            matches!(
                claim.role,
                ArtifactRole::Authority
                    | ArtifactRole::EvidenceAuthority
                    | ArtifactRole::Acceptance
            )
        })
        .map(|claim| (claim.path.display().to_owned(), claim.source_digest.clone()))
        .collect::<BTreeMap<_, _>>();
    canonical_digest(&sources)
}

fn compile_sync_plan(paths: &TransactionPaths) -> Result<SyncPlan, TransactionError> {
    let before =
        RepositoryModel::discover(&paths.repository_root, InventoryBounds::default(), true)?;
    let before_source = source_identity(&before)?;
    let report = aggregate_driver::check_family(&paths.repository_root)?;
    let after =
        RepositoryModel::discover(&paths.repository_root, InventoryBounds::default(), true)?;
    if before_source != source_identity(&after)? {
        return Err(TransactionError::SourceDrift);
    }
    let stage_root = PathBuf::from(&report.stage_root);
    let mut desired = BTreeMap::<String, Vec<u8>>::new();
    for path in &report.rendered_outputs {
        validate_output_path(&paths.repository_root, path)?;
        if before.claims.get(path.as_bytes()).is_some_and(|claim| {
            !matches!(claim.role, ArtifactRole::Derived | ArtifactRole::Ignored)
        }) {
            return Err(TransactionError::SourceOutputOverlap(path.clone()));
        }
        desired.insert(
            path.clone(),
            read_stable(&stage_root.join(path), MAX_FILE_BYTES)?,
        );
    }
    let previous = read_committed_tree(paths)?.unwrap_or_default();
    for (path, expected_digest) in &previous.outputs {
        let absolute = paths.repository_root.join(path);
        match read_optional_regular(&paths.repository_root, path)? {
            Some(bytes) if digest_bytes(&bytes) == *expected_digest => {}
            Some(bytes) if desired.get(path) == Some(&bytes) => {}
            Some(_) => return Err(TransactionError::UserEdit(path.clone())),
            None if absolute.exists() => {
                return Err(TransactionError::PathTypeChanged(path.clone()));
            }
            None => {}
        }
    }

    let mut all_paths = desired.keys().cloned().collect::<BTreeSet<_>>();
    all_paths.extend(previous.outputs.keys().cloned());
    let mut entries = Vec::new();
    let mut unchanged = 0;
    for path in all_paths {
        let old_bytes = read_optional_regular(&paths.repository_root, &path)?;
        let new_bytes = desired.get(&path).cloned();
        if old_bytes == new_bytes {
            unchanged += 1;
        } else {
            entries.push(PlannedEntry {
                path,
                old_bytes,
                new_bytes,
            });
        }
    }
    let desired_outputs = desired
        .iter()
        .map(|(path, bytes)| (path.clone(), digest_bytes(bytes)))
        .collect();
    Ok(SyncPlan {
        source_identity: before_source,
        desired_tree_identity: report.tree_digest,
        entries,
        desired_outputs,
        unchanged,
    })
}

#[allow(clippy::too_many_lines)] // The durable state machine stays linear for kill-point review.
fn apply_plan(
    paths: &TransactionPaths,
    plan: SyncPlan,
    failure_after: Option<usize>,
) -> Result<SyncReport, TransactionError> {
    if source_identity(&RepositoryModel::discover(
        &paths.repository_root,
        InventoryBounds::default(),
        true,
    )?)? != plan.source_identity
    {
        return Err(TransactionError::SourceDrift);
    }
    for entry in &plan.entries {
        let observed = read_optional_regular(&paths.repository_root, &entry.path)?;
        if observed != entry.old_bytes {
            return Err(TransactionError::UserEdit(entry.path.clone()));
        }
    }

    let transaction_id = transaction_id(&plan)?;
    let mut journal = TransactionJournal {
        schema_version: 1,
        recovery_version: "model-transaction-recovery-v1".to_owned(),
        transaction_id: transaction_id.clone(),
        source_identity: plan.source_identity.clone(),
        desired_tree_identity: plan.desired_tree_identity.clone(),
        phase: TransactionPhase::Prepared,
        entries: plan
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| JournalEntry {
                path: entry.path.clone(),
                old_digest: entry.old_bytes.as_deref().map(digest_bytes),
                new_digest: entry.new_bytes.as_deref().map(digest_bytes),
                temporary_name: format!(".codefabric-model-new-{transaction_id}-{index}"),
                backup_name: format!(".codefabric-model-old-{transaction_id}-{index}"),
                phase: EntryPhase::Planned,
            })
            .collect(),
    };
    if journal.entries.is_empty() {
        write_committed_tree(
            paths,
            &CommittedTree {
                schema_version: 1,
                desired_tree_identity: plan.desired_tree_identity.clone(),
                outputs: plan.desired_outputs,
            },
        )?;
        return Ok(SyncReport {
            desired_tree_identity: plan.desired_tree_identity,
            added: 0,
            replaced: 0,
            deleted_stale: 0,
            unchanged: plan.unchanged,
            transaction_applied: false,
        });
    }
    write_journal(paths, &journal)?;
    let mut durable_steps = 1;
    fail_if_requested(failure_after, durable_steps)?;
    journal.phase = TransactionPhase::Applying;
    write_journal(paths, &journal)?;
    durable_steps += 1;
    fail_if_requested(failure_after, durable_steps)?;

    for index in 0..journal.entries.len() {
        let planned = &plan.entries[index];
        let relative = &journal.entries[index].path;
        let destination = paths.repository_root.join(relative);
        let parent = destination
            .parent()
            .ok_or_else(|| TransactionError::UnsafeOutputPath(relative.clone()))?;
        create_safe_parent(&paths.repository_root, parent)?;
        let temporary = parent.join(&journal.entries[index].temporary_name);
        let backup = parent.join(&journal.entries[index].backup_name);
        remove_if_exists(&temporary)?;
        remove_if_exists(&backup)?;
        if let Some(new_bytes) = &planned.new_bytes {
            write_new_file(&temporary, new_bytes)?;
            journal.entries[index].phase = EntryPhase::TemporarySynced;
            write_journal(paths, &journal)?;
            durable_steps += 1;
            fail_if_requested(failure_after, durable_steps)?;
        }
        if planned.old_bytes.is_some() {
            fs::rename(&destination, &backup).map_err(|source| io(&destination, source))?;
            sync_directory(parent)?;
            journal.entries[index].phase = EntryPhase::OldBackedUp;
            write_journal(paths, &journal)?;
            durable_steps += 1;
            fail_if_requested(failure_after, durable_steps)?;
        }
        if planned.new_bytes.is_some() {
            fs::rename(&temporary, &destination).map_err(|source| io(&destination, source))?;
            sync_directory(parent)?;
        }
        journal.entries[index].phase = EntryPhase::NewInstalled;
        write_journal(paths, &journal)?;
        durable_steps += 1;
        fail_if_requested(failure_after, durable_steps)?;
    }
    journal.phase = TransactionPhase::Committed;
    write_journal(paths, &journal)?;
    durable_steps += 1;
    fail_if_requested(failure_after, durable_steps)?;
    let committed = CommittedTree {
        schema_version: 1,
        desired_tree_identity: plan.desired_tree_identity.clone(),
        outputs: plan.desired_outputs,
    };
    write_committed_tree(paths, &committed)?;
    cleanup_committed(paths, &journal)?;
    let added = plan
        .entries
        .iter()
        .filter(|entry| entry.old_bytes.is_none() && entry.new_bytes.is_some())
        .count();
    let replaced = plan
        .entries
        .iter()
        .filter(|entry| entry.old_bytes.is_some() && entry.new_bytes.is_some())
        .count();
    let deleted_stale = plan
        .entries
        .iter()
        .filter(|entry| entry.old_bytes.is_some() && entry.new_bytes.is_none())
        .count();
    Ok(SyncReport {
        desired_tree_identity: plan.desired_tree_identity,
        added,
        replaced,
        deleted_stale,
        unchanged: plan.unchanged,
        transaction_applied: true,
    })
}

fn recover_locked(paths: &TransactionPaths) -> Result<(), TransactionError> {
    let Some(journal) = read_journal(paths)? else {
        return Ok(());
    };
    validate_journal(&journal)?;
    if journal.phase == TransactionPhase::Committed {
        for entry in &journal.entries {
            let destination = paths.repository_root.join(&entry.path);
            match &entry.new_digest {
                Some(expected) => {
                    let observed = read_optional_regular(&paths.repository_root, &entry.path)?
                        .ok_or_else(|| TransactionError::RecoveryIncomplete(entry.path.clone()))?;
                    if digest_bytes(&observed) != *expected {
                        return Err(TransactionError::RecoveryIncomplete(entry.path.clone()));
                    }
                }
                None if destination.exists() => {
                    return Err(TransactionError::RecoveryIncomplete(entry.path.clone()));
                }
                None => {}
            }
        }
        cleanup_committed(paths, &journal)?;
        return Ok(());
    }

    for entry in journal.entries.iter().rev() {
        let destination = paths.repository_root.join(&entry.path);
        let parent = destination
            .parent()
            .ok_or_else(|| TransactionError::UnsafeOutputPath(entry.path.clone()))?;
        let temporary = parent.join(&entry.temporary_name);
        let backup = parent.join(&entry.backup_name);
        match &entry.old_digest {
            Some(expected) => {
                if backup.exists() {
                    remove_if_exists(&destination)?;
                    fs::rename(&backup, &destination).map_err(|source| io(&destination, source))?;
                } else {
                    let current = read_optional_regular(&paths.repository_root, &entry.path)?;
                    if current.as_deref().map(digest_bytes).as_deref() != Some(expected.as_str()) {
                        return Err(TransactionError::MissingBackup(entry.path.clone()));
                    }
                }
            }
            None => remove_if_exists(&destination)?,
        }
        remove_if_exists(&temporary)?;
        remove_if_exists(&backup)?;
        sync_directory(parent)?;
    }
    remove_journals(paths)?;
    Ok(())
}

fn validate_journal(journal: &TransactionJournal) -> Result<(), TransactionError> {
    if journal.schema_version != 1
        || journal.recovery_version != "model-transaction-recovery-v1"
        || journal.transaction_id.is_empty()
    {
        return Err(TransactionError::InvalidJournal);
    }
    let mut paths = BTreeSet::new();
    for entry in &journal.entries {
        validate_relative(&entry.path)?;
        if !paths.insert(&entry.path)
            || entry.temporary_name.contains('/')
            || entry.backup_name.contains('/')
        {
            return Err(TransactionError::InvalidJournal);
        }
    }
    Ok(())
}

fn cleanup_committed(
    paths: &TransactionPaths,
    journal: &TransactionJournal,
) -> Result<(), TransactionError> {
    for entry in &journal.entries {
        let destination = paths.repository_root.join(&entry.path);
        let parent = destination
            .parent()
            .ok_or_else(|| TransactionError::UnsafeOutputPath(entry.path.clone()))?;
        remove_if_exists(&parent.join(&entry.temporary_name))?;
        remove_if_exists(&parent.join(&entry.backup_name))?;
        sync_directory(parent)?;
    }
    remove_journals(paths)
}

fn read_journal(paths: &TransactionPaths) -> Result<Option<TransactionJournal>, TransactionError> {
    let current = paths.admin_root.join(JOURNAL_FILE);
    let previous = paths.admin_root.join(PREVIOUS_JOURNAL_FILE);
    if !current.exists() && !previous.exists() {
        return Ok(None);
    }
    for candidate in [&current, &previous] {
        if !candidate.exists() {
            continue;
        }
        let bytes = read_bounded(candidate, MAX_JOURNAL_BYTES)?;
        if let Ok(journal) = serde_json::from_slice::<TransactionJournal>(&bytes) {
            return Ok(Some(journal));
        }
    }
    Err(TransactionError::InvalidJournal)
}

fn write_journal(
    paths: &TransactionPaths,
    journal: &TransactionJournal,
) -> Result<(), TransactionError> {
    write_durable_json(
        &paths.admin_root,
        JOURNAL_FILE,
        Some(PREVIOUS_JOURNAL_FILE),
        journal,
    )
}

fn remove_journals(paths: &TransactionPaths) -> Result<(), TransactionError> {
    remove_if_exists(&paths.admin_root.join(JOURNAL_FILE))?;
    remove_if_exists(&paths.admin_root.join(PREVIOUS_JOURNAL_FILE))?;
    sync_directory(&paths.admin_root)
}

fn read_committed_tree(
    paths: &TransactionPaths,
) -> Result<Option<CommittedTree>, TransactionError> {
    let path = paths.admin_root.join(COMMITTED_TREE_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = read_bounded(&path, MAX_JOURNAL_BYTES)?;
    let tree: CommittedTree = serde_json::from_slice(&bytes)?;
    if tree.schema_version != 1 {
        return Err(TransactionError::InvalidJournal);
    }
    Ok(Some(tree))
}

fn write_committed_tree(
    paths: &TransactionPaths,
    tree: &CommittedTree,
) -> Result<(), TransactionError> {
    write_durable_json(&paths.admin_root, COMMITTED_TREE_FILE, None, tree)
}

fn write_durable_json<T: Serialize>(
    directory: &Path,
    name: &str,
    previous_name: Option<&str>,
    value: &T,
) -> Result<(), TransactionError> {
    let destination = directory.join(name);
    if let Some(previous_name) = previous_name
        && destination.exists()
    {
        let previous = directory.join(previous_name);
        remove_if_exists(&previous)?;
        fs::copy(&destination, &previous).map_err(|source| io(&previous, source))?;
        File::open(&previous)
            .and_then(|file| file.sync_all())
            .map_err(|source| io(&previous, source))?;
    }
    let temporary = directory.join(format!(".{name}.new"));
    remove_if_exists(&temporary)?;
    let bytes = serde_json::to_vec(value)?;
    write_new_file(&temporary, &bytes)?;
    fs::rename(&temporary, &destination).map_err(|source| io(&destination, source))?;
    sync_directory(directory)
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), TransactionError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|source| io(path, source))?;
    file.write_all(bytes).map_err(|source| io(path, source))?;
    file.sync_all().map_err(|source| io(path, source))
}

fn sync_directory(path: &Path) -> Result<(), TransactionError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| io(path, source))
}

fn validate_output_path(root: &Path, relative: &str) -> Result<(), TransactionError> {
    validate_relative(relative)?;
    let absolute = root.join(relative);
    let parent = absolute
        .parent()
        .ok_or_else(|| TransactionError::UnsafeOutputPath(relative.to_owned()))?;
    reject_symlink_ancestors(root, parent)
}

fn validate_relative(relative: &str) -> Result<(), TransactionError> {
    let path = Path::new(relative);
    if relative.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || relative.starts_with("contracts/acceptance/")
        || relative.starts_with("contracts/fixtures/")
        || relative.starts_with("docs/upfront_design/")
        || relative.starts_with("tooling/model-transition/")
    {
        return Err(TransactionError::UnsafeOutputPath(relative.to_owned()));
    }
    Ok(())
}

fn create_safe_parent(root: &Path, parent: &Path) -> Result<(), TransactionError> {
    if !parent.exists() {
        fs::create_dir_all(parent).map_err(|source| io(parent, source))?;
    }
    reject_symlink_ancestors(root, parent)
}

fn reject_symlink_ancestors(root: &Path, descendant: &Path) -> Result<(), TransactionError> {
    let relative = descendant
        .strip_prefix(root)
        .map_err(|_| TransactionError::UnsafeOutputPath(descendant.display().to_string()))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        if current.exists() {
            let metadata = fs::symlink_metadata(&current).map_err(|source| io(&current, source))?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(TransactionError::SymlinkOrPathType(current));
            }
        }
    }
    Ok(())
}

fn read_optional_regular(root: &Path, relative: &str) -> Result<Option<Vec<u8>>, TransactionError> {
    validate_output_path(root, relative)?;
    let path = root.join(relative);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            Ok(Some(read_stable(&path, MAX_FILE_BYTES)?))
        }
        Ok(_) => Err(TransactionError::PathTypeChanged(relative.to_owned())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(io(&path, source)),
    }
}

fn remove_if_exists(path: &Path) -> Result<(), TransactionError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            fs::remove_dir_all(path).map_err(|source| io(path, source))
        }
        Ok(_) => fs::remove_file(path).map_err(|source| io(path, source)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(io(path, source)),
    }
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, TransactionError> {
    let mut file = File::open(path).map_err(|source| io(path, source))?;
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(u64::try_from(limit).expect("bounded limit fits u64") + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| io(path, source))?;
    if bytes.len() > limit {
        return Err(TransactionError::JournalTooLarge);
    }
    Ok(bytes)
}

fn transaction_id(plan: &SyncPlan) -> Result<String, TransactionError> {
    let material = (
        &plan.source_identity,
        &plan.desired_tree_identity,
        plan.entries
            .iter()
            .map(|entry| {
                (
                    &entry.path,
                    entry.old_bytes.as_deref().map(digest_bytes),
                    entry.new_bytes.as_deref().map(digest_bytes),
                )
            })
            .collect::<Vec<_>>(),
    );
    Ok(canonical_digest(&material)?
        .trim_start_matches("b3:")
        .chars()
        .take(20)
        .collect())
}

fn canonical_digest(value: &impl Serialize) -> Result<String, TransactionError> {
    let value = serde_json::to_value(value)?;
    let bytes = serde_json_canonicalizer::to_vec(&value)?;
    Ok(digest_bytes(&bytes))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("b3:{}", blake3::hash(bytes).to_hex())
}

fn fail_if_requested(failure_after: Option<usize>, step: usize) -> Result<(), TransactionError> {
    if failure_after == Some(step) {
        return Err(TransactionError::InjectedFailure(step));
    }
    Ok(())
}

fn io(path: &Path, source: std::io::Error) -> TransactionError {
    TransactionError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Transaction and recovery failures.
#[derive(Debug, Error)]
pub enum TransactionError {
    #[error("repository worktree topology is unavailable")]
    GitTopology,
    #[error("unsafe model transaction administration root: {0}")]
    UnsafeAdminRoot(PathBuf),
    #[error("unsafe model output path: {0}")]
    UnsafeOutputPath(String),
    #[error("model output path contains a symlink or non-directory ancestor: {0}")]
    SymlinkOrPathType(PathBuf),
    #[error("model output path changed type: {0}")]
    PathTypeChanged(String),
    #[error("model output overlaps a governed source: {0}")]
    SourceOutputOverlap(String),
    #[error("governed source bytes changed after model planning")]
    SourceDrift,
    #[error("generated destination changed since the last committed plan: {0}")]
    UserEdit(String),
    #[error("transaction journal is invalid or truncated without a valid predecessor")]
    InvalidJournal,
    #[error("transaction journal exceeds its fixed bound")]
    JournalTooLarge,
    #[error("recovery cannot restore missing backup for {0}")]
    MissingBackup(String),
    #[error("committed recovery state is incomplete for {0}")]
    RecoveryIncomplete(String),
    #[error("injected transaction failure after durable step {0}")]
    InjectedFailure(usize),
    #[error("model transaction lock failed")]
    Lock(#[source] rustix::io::Errno),
    #[error("nested model lock targets another worktree or attempts a read-to-write upgrade")]
    NestedLock,
    #[error("model transaction I/O failed for {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Repository(#[from] super::repository_model::RepositoryModelError),
    #[error(transparent)]
    Aggregate(#[from] aggregate_driver::AggregateError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_repository() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let status = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(root.path())
            .status()
            .unwrap();
        assert!(status.success());
        root
    }

    fn paths(root: &Path) -> TransactionPaths {
        transaction_paths(root).unwrap()
    }

    fn plan(root: &Path, old: Option<&[u8]>, new: Option<&[u8]>) -> SyncPlan {
        let path = "generated/model.txt".to_owned();
        if let Some(bytes) = old {
            fs::create_dir_all(root.join("generated")).unwrap();
            fs::write(root.join(&path), bytes).unwrap();
        }
        SyncPlan {
            source_identity: canonical_digest(&BTreeMap::<String, String>::new()).unwrap(),
            desired_tree_identity: digest_bytes(new.unwrap_or_default()),
            entries: vec![PlannedEntry {
                path: path.clone(),
                old_bytes: old.map(<[u8]>::to_vec),
                new_bytes: new.map(<[u8]>::to_vec),
            }],
            desired_outputs: new
                .map(|bytes| BTreeMap::from([(path, digest_bytes(bytes))]))
                .unwrap_or_default(),
            unchanged: 0,
        }
    }

    fn apply_without_source_probe(
        paths: &TransactionPaths,
        mut plan: SyncPlan,
        failure_after: Option<usize>,
    ) -> Result<SyncReport, TransactionError> {
        let model =
            RepositoryModel::discover(&paths.repository_root, InventoryBounds::default(), true)
                .unwrap();
        plan.source_identity = source_identity(&model).unwrap();
        apply_plan(paths, plan, failure_after)
    }

    #[test]
    fn model_transaction_recovers_to_complete_old_or_new_tree_at_every_kill_point() {
        for failure_after in 1..=6 {
            let root = git_repository();
            let paths = paths(root.path());
            let result = apply_without_source_probe(
                &paths,
                plan(root.path(), Some(b"old"), Some(b"new")),
                Some(failure_after),
            );
            assert!(matches!(result, Err(TransactionError::InjectedFailure(_))));
            recover_locked(&paths).unwrap();
            let bytes = fs::read(root.path().join("generated/model.txt")).unwrap();
            assert!(bytes == b"old" || bytes == b"new");
            assert!(!paths.admin_root.join(JOURNAL_FILE).exists());
        }
    }

    #[test]
    fn model_sync_adds_replaces_and_deletes_stale_outputs_exactly() {
        let root = git_repository();
        let paths = paths(root.path());
        let added = apply_without_source_probe(&paths, plan(root.path(), None, Some(b"one")), None)
            .unwrap();
        assert_eq!(added.added, 1);
        let replaced =
            apply_without_source_probe(&paths, plan(root.path(), Some(b"one"), Some(b"two")), None)
                .unwrap();
        assert_eq!(replaced.replaced, 1);
        let deleted =
            apply_without_source_probe(&paths, plan(root.path(), Some(b"two"), None), None)
                .unwrap();
        assert_eq!(deleted.deleted_stale, 1);
        assert!(!root.path().join("generated/model.txt").exists());
    }

    #[test]
    fn model_sync_rejects_symlink_swap_user_edit_and_path_type_change() {
        let root = git_repository();
        let paths = paths(root.path());
        fs::create_dir(root.path().join("generated")).unwrap();
        std::os::unix::fs::symlink("/tmp", root.path().join("generated/link")).unwrap();
        assert!(matches!(
            read_optional_regular(root.path(), "generated/link/output"),
            Err(TransactionError::SymlinkOrPathType(_))
        ));
        let model_plan = plan(root.path(), Some(b"planned"), Some(b"new"));
        fs::write(root.path().join("generated/model.txt"), b"edited").unwrap();
        assert!(matches!(
            apply_without_source_probe(&paths, model_plan, None),
            Err(TransactionError::UserEdit(_))
        ));
        fs::remove_file(root.path().join("generated/model.txt")).unwrap();
        fs::create_dir(root.path().join("generated/model.txt")).unwrap();
        assert!(matches!(
            read_optional_regular(root.path(), "generated/model.txt"),
            Err(TransactionError::PathTypeChanged(_))
        ));
    }

    #[test]
    fn model_supported_readers_share_one_lock_protocol_and_recover_truncated_journal() {
        let root = git_repository();
        let paths = paths(root.path());
        let result = apply_without_source_probe(
            &paths,
            plan(root.path(), Some(b"old"), Some(b"new")),
            Some(3),
        );
        assert!(result.is_err());
        fs::write(paths.admin_root.join(JOURNAL_FILE), b"{").unwrap();
        let _reader = read_guard(root.path()).unwrap();
        assert_eq!(
            fs::read(root.path().join("generated/model.txt")).unwrap(),
            b"old"
        );
    }

    #[test]
    fn model_source_recheck_aborts_before_apply_on_drift() {
        let root = git_repository();
        let paths = paths(root.path());
        let mut stale = plan(root.path(), None, Some(b"new"));
        stale.source_identity = "b3:stale".to_owned();
        assert!(matches!(
            apply_plan(&paths, stale, None),
            Err(TransactionError::SourceDrift)
        ));
        assert!(!root.path().join("generated/model.txt").exists());
    }

    #[test]
    fn model_blocked_sync_never_exposes_a_mixed_tree_to_supported_readers() {
        let root = git_repository();
        let paths = paths(root.path());
        let result = apply_without_source_probe(
            &paths,
            plan(root.path(), Some(b"old"), Some(b"new")),
            Some(4),
        );
        assert!(result.is_err());
        let journal = read_journal(&paths).unwrap().unwrap();
        let entry = &journal.entries[0];
        let parent = root
            .path()
            .join(&entry.path)
            .parent()
            .unwrap()
            .to_path_buf();
        fs::remove_file(parent.join(&entry.backup_name)).unwrap();
        assert!(matches!(
            read_guard(root.path()),
            Err(TransactionError::MissingBackup(_))
        ));
    }

    #[test]
    fn model_nested_reader_writer_commands_do_not_deadlock() {
        let root = git_repository();
        let paths = paths(root.path());
        let outer = acquire_write(&paths).unwrap();
        let inner_reader = read_guard(root.path()).unwrap();
        let inner_writer = acquire_write(&paths).unwrap();
        assert_eq!(LOCK_DEPTH.with(Cell::get), 3);
        drop(inner_writer);
        drop(inner_reader);
        assert_eq!(LOCK_DEPTH.with(Cell::get), 1);
        drop(outer);
        assert_eq!(LOCK_DEPTH.with(Cell::get), 0);
    }

    #[test]
    fn model_sync_then_plan_is_zero_and_second_sync_is_byte_idempotent() {
        let root = git_repository();
        let paths = paths(root.path());
        let first =
            apply_without_source_probe(&paths, plan(root.path(), None, Some(b"desired")), None)
                .unwrap();
        assert!(first.transaction_applied);
        let second = SyncPlan {
            source_identity: source_identity(
                &RepositoryModel::discover(root.path(), InventoryBounds::default(), true).unwrap(),
            )
            .unwrap(),
            desired_tree_identity: first.desired_tree_identity.clone(),
            entries: Vec::new(),
            desired_outputs: BTreeMap::from([(
                "generated/model.txt".to_owned(),
                digest_bytes(b"desired"),
            )]),
            unchanged: 1,
        };
        let second = apply_plan(&paths, second, None).unwrap();
        assert!(!second.transaction_applied);
        assert_eq!(second.unchanged, 1);
        assert_eq!(
            fs::read(root.path().join("generated/model.txt")).unwrap(),
            b"desired"
        );
    }

    #[test]
    fn model_journal_and_backups_survive_disposable_target_cleanup() {
        let root = git_repository();
        let paths = paths(root.path());
        assert!(!paths.admin_root.starts_with(root.path().join("target")));
        fs::create_dir_all(root.path().join("target/model-stage")).unwrap();
        let result = apply_without_source_probe(
            &paths,
            plan(root.path(), Some(b"old"), Some(b"new")),
            Some(4),
        );
        assert!(result.is_err());
        fs::remove_dir_all(root.path().join("target")).unwrap();
        recover_locked(&paths).unwrap();
        assert_eq!(
            fs::read(root.path().join("generated/model.txt")).unwrap(),
            b"old"
        );
    }

    #[test]
    fn model_linked_worktrees_do_not_share_per_worktree_transaction_state() {
        let root = git_repository();
        fs::write(root.path().join("seed"), b"seed").unwrap();
        let status = std::process::Command::new("git")
            .args(["add", "seed"])
            .current_dir(root.path())
            .status()
            .unwrap();
        assert!(status.success());
        let status = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=Model Test",
                "-c",
                "user.email=model@example.invalid",
                "commit",
                "-m",
                "seed",
                "--quiet",
            ])
            .current_dir(root.path())
            .status()
            .unwrap();
        assert!(status.success());
        let linked = tempfile::tempdir().unwrap();
        let status = std::process::Command::new("git")
            .args([
                "worktree",
                "add",
                "--detach",
                linked.path().to_str().unwrap(),
                "HEAD",
            ])
            .current_dir(root.path())
            .status()
            .unwrap();
        assert!(status.success());
        let primary = paths(root.path());
        let secondary = paths(linked.path());
        assert_eq!(primary.common_dir, secondary.common_dir);
        assert_ne!(primary.git_dir, secondary.git_dir);
        assert_ne!(primary.admin_root, secondary.admin_root);
    }
}
