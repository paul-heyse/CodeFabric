//! Dedicated temporal SQLite journal for [`super::command::CommandRecord`] values.
//!
//! The journal implements the durable compare-and-swap port consumed by the single
//! [`super::command_actor::FabricCommandActor`]. It deliberately stores no semantic-current
//! pointer: command progress, retry state, and reconciliation state are temporal coordination
//! facts, while the admitted fabric epoch remains authoritative elsewhere.

use std::fs::{self, File};
use std::num::NonZeroU16;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use async_trait::async_trait;
use rusqlite::{Connection, OpenFlags, OptionalExtension as _, Transaction, TransactionBehavior};
use rustix::fs::{Mode, OFlags, open, openat};
use thiserror::Error;
use tokio::sync::oneshot;

use super::command::{
    CommandRecord, CommandStateKind, IdempotencyKey, OperationId, ReducerTransition, WorkspaceId,
};
use super::command_actor::{
    CommandPortError, DurableAdmissionWrite, DurableCommandRecordPort, DurableTransitionWrite,
};

/// Handwritten temporal schema version. Generated semantic DDL is never accepted here.
pub const COMMAND_RECORD_SCHEMA_VERSION: u32 = 1;

const APPLICATION_ID: u32 = 0x4346_434d; // `CFCM`
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const RECORD_TABLE: &str = "fabric_command_record";
const MAX_RECORD_BYTES: usize = 64 * 1024;
const SCHEMA_V1: &str = "CREATE TABLE fabric_command_record (
    operation_id BLOB NOT NULL PRIMARY KEY
        CHECK (typeof(operation_id) = 'blob' AND length(operation_id) = 16),
    idempotency_key BLOB NOT NULL UNIQUE
        CHECK (typeof(idempotency_key) = 'blob' AND length(idempotency_key) = 32),
    workspace_id BLOB NOT NULL
        CHECK (typeof(workspace_id) = 'blob' AND length(workspace_id) = 16),
    revision BLOB NOT NULL
        CHECK (typeof(revision) = 'blob' AND length(revision) = 8),
    state_kind TEXT NOT NULL
        CHECK (state_kind IN (
            'ADMITTED', 'EXECUTING', 'COMMIT_PREPARED', 'AWAITING_RECONCILIATION',
            'RETRY_READY', 'SUCCEEDED', 'FAILED', 'CANCELLED'
        )),
    is_terminal INTEGER NOT NULL
        CHECK (typeof(is_terminal) = 'integer' AND is_terminal IN (0, 1)),
    record_jcs BLOB NOT NULL
        CHECK (typeof(record_jcs) = 'blob' AND length(record_jcs) BETWEEN 2 AND 65536)
) WITHOUT ROWID, STRICT;
PRAGMA application_id = 1128678221;
PRAGMA user_version = 1;";

/// Failures while opening and validating the dedicated command journal.
#[derive(Debug, Error)]
pub enum SqliteCommandRecordOpenError {
    #[error("command-journal parent is not a private owned directory: {0}")]
    UnsafeParent(PathBuf),
    #[error("command journal is not a private owned regular file: {0}")]
    UnsafeDatabase(PathBuf),
    #[error("command-journal database path has no file name: {0}")]
    InvalidPath(PathBuf),
    #[error("command-journal I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(
        "unsupported command-journal schema version {observed}; supported version is {supported}"
    )]
    UnsupportedSchema { observed: u32, supported: u32 },
    #[error("command-journal database schema is not the exact temporal schema: {0}")]
    UnexpectedSchema(String),
    #[error("failed to start the command-journal worker: {0}")]
    Worker(std::io::Error),
}

/// Bounded number of nonterminal records returned by one recovery read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandRecoveryPageSize(NonZeroU16);

impl CommandRecoveryPageSize {
    /// Maximum recovery rows decoded by one worker request.
    pub const MAX: u16 = 1024;

    /// Construct a nonzero bounded page size.
    #[must_use]
    pub const fn new(value: u16) -> Option<Self> {
        match NonZeroU16::new(value) {
            Some(value) if value.get() <= Self::MAX => Some(Self(value)),
            Some(_) | None => None,
        }
    }

    /// Requested row count.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0.get()
    }
}

/// One deterministic page of nonterminal temporal records for restart reconciliation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandRecoveryPage {
    records: Vec<CommandRecord>,
    next_after: Option<OperationId>,
}

impl CommandRecoveryPage {
    /// Records ordered by canonical operation identity.
    #[must_use]
    pub fn records(&self) -> &[CommandRecord] {
        &self.records
    }

    /// Exclusive operation cursor for the next page, present only when another row exists.
    #[must_use]
    pub const fn next_after(&self) -> Option<OperationId> {
        self.next_after
    }
}

enum StoreRequest {
    LookupAdmission {
        operation_id: OperationId,
        idempotency_key: IdempotencyKey,
        response: oneshot::Sender<Result<Option<CommandRecord>, CommandPortError>>,
    },
    LookupOperation {
        operation_id: OperationId,
        response: oneshot::Sender<Result<Option<CommandRecord>, CommandPortError>>,
    },
    InsertIfAbsent {
        admitted: CommandRecord,
        response: oneshot::Sender<Result<DurableAdmissionWrite, CommandPortError>>,
    },
    CompareAndSwap {
        transition: ReducerTransition,
        response: oneshot::Sender<Result<DurableTransitionWrite, CommandPortError>>,
    },
    LoadNonterminal {
        after: Option<OperationId>,
        page_size: CommandRecoveryPageSize,
        response: oneshot::Sender<Result<CommandRecoveryPage, CommandPortError>>,
    },
}

/// One exact-schema SQLite journal owned by a dedicated blocking worker.
///
/// The async command actor never runs SQLite I/O on a Tokio executor thread. Dropping the final
/// store closes the request channel and joins the worker so WAL state is closed deterministically.
pub struct SqliteCommandRecordStore {
    database_path: PathBuf,
    sender: Option<Sender<StoreRequest>>,
    worker: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for SqliteCommandRecordStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SqliteCommandRecordStore")
            .field("database_path", &self.database_path)
            .finish_non_exhaustive()
    }
}

impl SqliteCommandRecordStore {
    /// Open or initialize one private, exact-schema temporal command journal.
    ///
    /// The direct parent must already be a non-symlink directory owned by the effective user with
    /// mode `0700`. The database is created with mode `0600`, bound by device/inode across the
    /// descriptor-to-SQLite handoff, and rejected if any undeclared user object exists.
    ///
    /// # Errors
    ///
    /// Rejects unsafe paths/files, incompatible schemas, SQLite setup failures, and worker-start
    /// failures.
    pub fn open(path: &Path) -> Result<Self, SqliteCommandRecordOpenError> {
        let prepared_file = prepare_private_database_file(path)?;
        let prepared_metadata =
            prepared_file
                .metadata()
                .map_err(|source| SqliteCommandRecordOpenError::Io {
                    path: path.to_owned(),
                    source,
                })?;
        let mut connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        validate_same_private_file(path, &prepared_metadata)?;
        apply_pragmas(&connection)?;
        initialize_or_validate_schema(&mut connection)?;
        drop(prepared_file);

        let (sender, receiver) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("codefabric-command-journal".to_owned())
            .spawn(move || run_worker(connection, receiver))
            .map_err(SqliteCommandRecordOpenError::Worker)?;
        Ok(Self {
            database_path: path.to_owned(),
            sender: Some(sender),
            worker: Some(worker),
        })
    }

    /// Path of this dedicated temporal store.
    #[must_use]
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    /// Read one bounded page of nonterminal records for restart reconciliation.
    ///
    /// This is a temporal recovery projection only. It cannot select a semantic head, retry a
    /// command, or turn an unknown commit into a known failure.
    ///
    /// # Errors
    ///
    /// Returns unavailable/corrupt journal state and rejects every noncanonical stored record.
    pub async fn load_nonterminal_page(
        &self,
        after: Option<OperationId>,
        page_size: CommandRecoveryPageSize,
    ) -> Result<CommandRecoveryPage, CommandPortError> {
        let (response, receiver) = oneshot::channel();
        self.send(StoreRequest::LoadNonterminal {
            after,
            page_size,
            response,
        })?;
        receiver
            .await
            .map_err(|_| CommandPortError::DurableStoreUnavailable)?
    }

    fn send(&self, request: StoreRequest) -> Result<(), CommandPortError> {
        self.sender
            .as_ref()
            .ok_or(CommandPortError::DurableStoreUnavailable)?
            .send(request)
            .map_err(|_| CommandPortError::DurableStoreUnavailable)
    }
}

impl Drop for SqliteCommandRecordStore {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[async_trait]
impl DurableCommandRecordPort for SqliteCommandRecordStore {
    async fn lookup_admission(
        &self,
        operation_id: OperationId,
        idempotency_key: IdempotencyKey,
    ) -> Result<Option<CommandRecord>, CommandPortError> {
        let (response, receiver) = oneshot::channel();
        self.send(StoreRequest::LookupAdmission {
            operation_id,
            idempotency_key,
            response,
        })?;
        receiver
            .await
            .map_err(|_| CommandPortError::DurableStoreUnavailable)?
    }

    async fn lookup_operation(
        &self,
        operation_id: OperationId,
    ) -> Result<Option<CommandRecord>, CommandPortError> {
        let (response, receiver) = oneshot::channel();
        self.send(StoreRequest::LookupOperation {
            operation_id,
            response,
        })?;
        receiver
            .await
            .map_err(|_| CommandPortError::DurableStoreUnavailable)?
    }

    async fn insert_if_absent(
        &self,
        admitted: CommandRecord,
    ) -> Result<DurableAdmissionWrite, CommandPortError> {
        let (response, receiver) = oneshot::channel();
        self.send(StoreRequest::InsertIfAbsent { admitted, response })?;
        receiver
            .await
            .map_err(|_| CommandPortError::DurableStoreUnavailable)?
    }

    async fn compare_and_swap(
        &self,
        transition: ReducerTransition,
    ) -> Result<DurableTransitionWrite, CommandPortError> {
        let (response, receiver) = oneshot::channel();
        self.send(StoreRequest::CompareAndSwap {
            transition,
            response,
        })?;
        receiver
            .await
            .map_err(|_| CommandPortError::DurableStoreUnavailable)?
    }

    async fn first_nonterminal(&self) -> Result<Option<CommandRecord>, CommandPortError> {
        let page = self
            .load_nonterminal_page(
                None,
                CommandRecoveryPageSize::new(1).expect("one is a valid recovery page size"),
            )
            .await?;
        Ok(page.records().first().copied())
    }
}

fn run_worker(mut connection: Connection, receiver: Receiver<StoreRequest>) {
    while let Ok(request) = receiver.recv() {
        match request {
            StoreRequest::LookupAdmission {
                operation_id,
                idempotency_key,
                response,
            } => {
                let _ = response.send(lookup_admission_sync(
                    &connection,
                    operation_id,
                    idempotency_key,
                ));
            }
            StoreRequest::LookupOperation {
                operation_id,
                response,
            } => {
                let _ = response.send(lookup_operation_sync(&connection, operation_id));
            }
            StoreRequest::InsertIfAbsent { admitted, response } => {
                let _ = response.send(insert_if_absent_sync(&mut connection, admitted));
            }
            StoreRequest::CompareAndSwap {
                transition,
                response,
            } => {
                let _ = response.send(compare_and_swap_sync(&mut connection, transition));
            }
            StoreRequest::LoadNonterminal {
                after,
                page_size,
                response,
            } => {
                let _ = response.send(load_nonterminal_page_sync(&connection, after, page_size));
            }
        }
    }
}

fn load_nonterminal_page_sync(
    connection: &Connection,
    after: Option<OperationId>,
    page_size: CommandRecoveryPageSize,
) -> Result<CommandRecoveryPage, CommandPortError> {
    let mut records = match after {
        Some(after) => {
            let mut statement = connection
                .prepare(
                    "SELECT operation_id, idempotency_key, workspace_id, revision,
                            state_kind, is_terminal, record_jcs
                     FROM fabric_command_record
                     WHERE operation_id > ?1
                     ORDER BY operation_id",
                )
                .map_err(unavailable)?;
            let rows = statement
                .query([after.as_bytes().as_slice()])
                .map_err(unavailable)?;
            decode_nonterminal_rows(rows, page_size)?
        }
        None => {
            let mut statement = connection
                .prepare(
                    "SELECT operation_id, idempotency_key, workspace_id, revision,
                            state_kind, is_terminal, record_jcs
                     FROM fabric_command_record
                     ORDER BY operation_id",
                )
                .map_err(unavailable)?;
            let rows = statement.query([]).map_err(unavailable)?;
            decode_nonterminal_rows(rows, page_size)?
        }
    };
    let has_more = records.len() > usize::from(page_size.get());
    if has_more {
        records.pop();
    }
    let next_after = has_more.then(|| {
        records
            .last()
            .expect("a positive page size with an extra row retains one record")
            .command()
            .identity
            .operation_id
    });
    Ok(CommandRecoveryPage {
        records,
        next_after,
    })
}

fn decode_nonterminal_rows(
    mut rows: rusqlite::Rows<'_>,
    page_size: CommandRecoveryPageSize,
) -> Result<Vec<CommandRecord>, CommandPortError> {
    let target = usize::from(page_size.get()) + 1;
    let mut records = Vec::with_capacity(target);
    while let Some(row) = rows.next().map_err(unavailable)? {
        let record = decode_row(stored_row(row).map_err(unavailable)?)?;
        if !record.state().is_terminal() {
            records.push(record);
            if records.len() == target {
                break;
            }
        }
    }
    Ok(records)
}

fn lookup_admission_sync(
    connection: &Connection,
    operation_id: OperationId,
    idempotency_key: IdempotencyKey,
) -> Result<Option<CommandRecord>, CommandPortError> {
    lookup_admission_on(connection, operation_id, idempotency_key)
}

fn lookup_admission_on(
    connection: &Connection,
    operation_id: OperationId,
    idempotency_key: IdempotencyKey,
) -> Result<Option<CommandRecord>, CommandPortError> {
    let mut statement = connection
        .prepare(
            "SELECT operation_id, idempotency_key, workspace_id, revision,
                    state_kind, is_terminal, record_jcs
             FROM fabric_command_record
             WHERE operation_id = ?1 OR idempotency_key = ?2
             ORDER BY operation_id",
        )
        .map_err(unavailable)?;
    let mut records = statement
        .query_map(
            rusqlite::params![
                operation_id.as_bytes().as_slice(),
                idempotency_key.as_bytes().as_slice()
            ],
            stored_row,
        )
        .map_err(unavailable)?
        .map(|row| row.map_err(unavailable).and_then(decode_row))
        .collect::<Result<Vec<_>, _>>()?;
    match records.len() {
        0 => Ok(None),
        1 => Ok(records.pop()),
        _ => Err(CommandPortError::CorruptRecord),
    }
}

fn lookup_operation_sync(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<Option<CommandRecord>, CommandPortError> {
    connection
        .query_row(
            "SELECT operation_id, idempotency_key, workspace_id, revision,
                    state_kind, is_terminal, record_jcs
             FROM fabric_command_record
             WHERE operation_id = ?1",
            [operation_id.as_bytes().as_slice()],
            stored_row,
        )
        .optional()
        .map_err(unavailable)?
        .map(decode_row)
        .transpose()
}

fn insert_if_absent_sync(
    connection: &mut Connection,
    admitted: CommandRecord,
) -> Result<DurableAdmissionWrite, CommandPortError> {
    if admitted.revision() != 0 {
        return Err(CommandPortError::CorruptRecord);
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    let affected = insert_record(&transaction, admitted)?;
    let outcome = if affected == 1 {
        DurableAdmissionWrite::Inserted(admitted)
    } else {
        let command = admitted.command();
        let existing = lookup_admission_on(
            &transaction,
            command.identity.operation_id,
            command.identity.idempotency_key,
        )?
        .ok_or(CommandPortError::CorruptRecord)?;
        DurableAdmissionWrite::Existing(existing)
    };
    transaction.commit().map_err(unavailable)?;
    Ok(outcome)
}

fn compare_and_swap_sync(
    connection: &mut Connection,
    transition: ReducerTransition,
) -> Result<DurableTransitionWrite, CommandPortError> {
    let predecessor = transition.predecessor();
    let successor = transition.successor();
    let operation_id = predecessor.command().identity.operation_id;
    let expected_revision = predecessor.revision();
    let next_revision = expected_revision
        .checked_add(1)
        .ok_or(CommandPortError::CorruptRecord)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    let observed = lookup_operation_sync(&transaction, operation_id)?;
    let Some(current) = observed else {
        transaction.rollback().map_err(unavailable)?;
        return Ok(DurableTransitionWrite::RevisionConflict { observed: None });
    };
    if current.revision() != expected_revision {
        transaction.rollback().map_err(unavailable)?;
        return Ok(DurableTransitionWrite::RevisionConflict {
            observed: Some(current),
        });
    }
    if current != predecessor {
        return Err(CommandPortError::CorruptRecord);
    }
    if transition.effect() == super::command::ReductionEffect::IdempotentReplay
        || successor.command() != current.command()
        || successor.command().identity.operation_id != operation_id
        || successor.revision() != next_revision
    {
        return Err(CommandPortError::CorruptRecord);
    }

    let encoded = encode_record(successor)?;
    let command = successor.command();
    let revision = successor.revision().to_be_bytes();
    let expected = expected_revision.to_be_bytes();
    let affected = transaction
        .execute(
            "UPDATE fabric_command_record SET
                 idempotency_key = ?2,
                 workspace_id = ?3,
                 revision = ?4,
                 state_kind = ?5,
                 is_terminal = ?6,
                 record_jcs = ?7
             WHERE operation_id = ?1 AND revision = ?8",
            rusqlite::params![
                operation_id.as_bytes().as_slice(),
                command.identity.idempotency_key.as_bytes().as_slice(),
                command.ownership.workspace_id.as_bytes().as_slice(),
                revision.as_slice(),
                state_kind_name(successor.state().kind()),
                i64::from(successor.state().is_terminal()),
                encoded,
                expected.as_slice(),
            ],
        )
        .map_err(unavailable)?;
    if affected != 1 {
        return Err(CommandPortError::CorruptRecord);
    }
    transaction.commit().map_err(unavailable)?;
    Ok(DurableTransitionWrite::Stored(successor))
}

fn insert_record(
    transaction: &Transaction<'_>,
    record: CommandRecord,
) -> Result<usize, CommandPortError> {
    let command = record.command();
    let revision = record.revision().to_be_bytes();
    let encoded = encode_record(record)?;
    transaction
        .execute(
            "INSERT OR IGNORE INTO fabric_command_record (
                 operation_id, idempotency_key, workspace_id, revision,
                 state_kind, is_terminal, record_jcs
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                command.identity.operation_id.as_bytes().as_slice(),
                command.identity.idempotency_key.as_bytes().as_slice(),
                command.ownership.workspace_id.as_bytes().as_slice(),
                revision.as_slice(),
                state_kind_name(record.state().kind()),
                i64::from(record.state().is_terminal()),
                encoded,
            ],
        )
        .map_err(unavailable)
}

struct StoredRow {
    operation_id: Vec<u8>,
    idempotency_key: Vec<u8>,
    workspace_id: Vec<u8>,
    revision: Vec<u8>,
    state_kind: String,
    is_terminal: i64,
    record_jcs: Vec<u8>,
}

fn stored_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredRow> {
    Ok(StoredRow {
        operation_id: row.get(0)?,
        idempotency_key: row.get(1)?,
        workspace_id: row.get(2)?,
        revision: row.get(3)?,
        state_kind: row.get(4)?,
        is_terminal: row.get(5)?,
        record_jcs: row.get(6)?,
    })
}

fn decode_row(row: StoredRow) -> Result<CommandRecord, CommandPortError> {
    if row.operation_id.len() != 16
        || row.idempotency_key.len() != 32
        || row.workspace_id.len() != 16
        || row.revision.len() != 8
        || row.record_jcs.len() > MAX_RECORD_BYTES
        || !matches!(row.is_terminal, 0 | 1)
    {
        return Err(CommandPortError::CorruptRecord);
    }
    let record: CommandRecord =
        serde_json::from_slice(&row.record_jcs).map_err(|_| CommandPortError::CorruptRecord)?;
    record
        .validate_persisted_invariants()
        .map_err(|_| CommandPortError::CorruptRecord)?;
    if encode_record(record)? != row.record_jcs {
        return Err(CommandPortError::CorruptRecord);
    }
    let command = record.command();
    let revision = u64::from_be_bytes(
        row.revision
            .try_into()
            .map_err(|_| CommandPortError::CorruptRecord)?,
    );
    if command.identity.operation_id.as_bytes().as_slice() != row.operation_id
        || command.identity.idempotency_key.as_bytes().as_slice() != row.idempotency_key
        || command.ownership.workspace_id.as_bytes().as_slice() != row.workspace_id
        || record.revision() != revision
        || state_kind_name(record.state().kind()) != row.state_kind
        || record.state().is_terminal() != (row.is_terminal == 1)
    {
        return Err(CommandPortError::CorruptRecord);
    }
    Ok(record)
}

fn encode_record(record: CommandRecord) -> Result<Vec<u8>, CommandPortError> {
    record
        .validate_persisted_invariants()
        .map_err(|_| CommandPortError::CorruptRecord)?;
    let bytes =
        serde_json_canonicalizer::to_vec(&record).map_err(|_| CommandPortError::CorruptRecord)?;
    if !(2..=MAX_RECORD_BYTES).contains(&bytes.len()) {
        return Err(CommandPortError::CorruptRecord);
    }
    Ok(bytes)
}

const fn state_kind_name(kind: CommandStateKind) -> &'static str {
    match kind {
        CommandStateKind::Admitted => "ADMITTED",
        CommandStateKind::Executing => "EXECUTING",
        CommandStateKind::CommitPrepared => "COMMIT_PREPARED",
        CommandStateKind::AwaitingReconciliation => "AWAITING_RECONCILIATION",
        CommandStateKind::RetryReady => "RETRY_READY",
        CommandStateKind::Succeeded => "SUCCEEDED",
        CommandStateKind::Failed => "FAILED",
        CommandStateKind::Cancelled => "CANCELLED",
    }
}

fn unavailable(_error: rusqlite::Error) -> CommandPortError {
    CommandPortError::DurableStoreUnavailable
}

fn prepare_private_database_file(path: &Path) -> Result<File, SqliteCommandRecordOpenError> {
    let parent = path
        .parent()
        .filter(|candidate| !candidate.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent_metadata = fs::symlink_metadata(parent)
        .map_err(|_| SqliteCommandRecordOpenError::UnsafeParent(parent.to_owned()))?;
    let owner = rustix::process::geteuid().as_raw();
    if !parent_metadata.is_dir()
        || parent_metadata.file_type().is_symlink()
        || parent_metadata.uid() != owner
        || parent_metadata.permissions().mode() & 0o777 != 0o700
    {
        return Err(SqliteCommandRecordOpenError::UnsafeParent(
            parent.to_owned(),
        ));
    }
    let file_name = path
        .file_name()
        .ok_or_else(|| SqliteCommandRecordOpenError::InvalidPath(path.to_owned()))?;
    let directory = open(
        parent,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY,
        Mode::empty(),
    )
    .map_err(|_| SqliteCommandRecordOpenError::UnsafeParent(parent.to_owned()))?;
    let descriptor = openat(
        &directory,
        file_name,
        OFlags::RDWR | OFlags::CREATE | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|_| SqliteCommandRecordOpenError::UnsafeDatabase(path.to_owned()))?;
    let file = File::from(descriptor);
    let metadata = file
        .metadata()
        .map_err(|source| SqliteCommandRecordOpenError::Io {
            path: path.to_owned(),
            source,
        })?;
    if !private_database_metadata(&metadata, owner) {
        return Err(SqliteCommandRecordOpenError::UnsafeDatabase(
            path.to_owned(),
        ));
    }
    file.sync_all()
        .map_err(|source| SqliteCommandRecordOpenError::Io {
            path: path.to_owned(),
            source,
        })?;
    Ok(file)
}

fn validate_same_private_file(
    path: &Path,
    prepared: &fs::Metadata,
) -> Result<(), SqliteCommandRecordOpenError> {
    let observed = fs::symlink_metadata(path)
        .map_err(|_| SqliteCommandRecordOpenError::UnsafeDatabase(path.to_owned()))?;
    if !private_database_metadata(&observed, rustix::process::geteuid().as_raw())
        || observed.dev() != prepared.dev()
        || observed.ino() != prepared.ino()
    {
        return Err(SqliteCommandRecordOpenError::UnsafeDatabase(
            path.to_owned(),
        ));
    }
    Ok(())
}

fn private_database_metadata(metadata: &fs::Metadata, owner: u32) -> bool {
    metadata.is_file()
        && !metadata.file_type().is_symlink()
        && metadata.uid() == owner
        && metadata.nlink() == 1
        && metadata.permissions().mode() & 0o777 == 0o600
}

fn apply_pragmas(connection: &Connection) -> Result<(), SqliteCommandRecordOpenError> {
    connection.busy_timeout(BUSY_TIMEOUT)?;
    let journal_mode: String =
        connection.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
    if journal_mode != "wal" {
        return Err(SqliteCommandRecordOpenError::UnexpectedSchema(format!(
            "journal_mode is {journal_mode}, expected wal"
        )));
    }
    connection.execute_batch(
        "PRAGMA synchronous=FULL;
         PRAGMA foreign_keys=ON;
         PRAGMA trusted_schema=OFF;
         PRAGMA secure_delete=FAST;
         PRAGMA wal_autocheckpoint=1000;",
    )?;
    let synchronous: i64 = connection.query_row("PRAGMA synchronous", [], |row| row.get(0))?;
    if synchronous != 2 {
        return Err(SqliteCommandRecordOpenError::UnexpectedSchema(format!(
            "synchronous is {synchronous}, expected FULL (2)"
        )));
    }
    Ok(())
}

fn initialize_or_validate_schema(
    connection: &mut Connection,
) -> Result<(), SqliteCommandRecordOpenError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let observed_version: u32 =
        transaction.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let observed_application: u32 =
        transaction.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    match observed_version {
        0 => {
            if observed_application != 0 || !schema_objects(&transaction)?.is_empty() {
                return Err(SqliteCommandRecordOpenError::UnexpectedSchema(
                    "unversioned database already contains application identity or objects".into(),
                ));
            }
            transaction.execute_batch(SCHEMA_V1)?;
        }
        COMMAND_RECORD_SCHEMA_VERSION => {
            if observed_application != APPLICATION_ID {
                return Err(SqliteCommandRecordOpenError::UnexpectedSchema(format!(
                    "application_id is {observed_application}, expected {APPLICATION_ID}"
                )));
            }
        }
        observed => {
            return Err(SqliteCommandRecordOpenError::UnsupportedSchema {
                observed,
                supported: COMMAND_RECORD_SCHEMA_VERSION,
            });
        }
    }
    validate_schema(&transaction)?;
    transaction.commit()?;
    Ok(())
}

fn validate_schema(connection: &Connection) -> Result<(), SqliteCommandRecordOpenError> {
    let version: u32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let application: u32 = connection.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    if version != COMMAND_RECORD_SCHEMA_VERSION || application != APPLICATION_ID {
        return Err(SqliteCommandRecordOpenError::UnexpectedSchema(
            "version or application identity changed during schema validation".into(),
        ));
    }
    let objects = schema_objects(connection)?;
    if objects != vec![("table".to_owned(), RECORD_TABLE.to_owned())] {
        return Err(SqliteCommandRecordOpenError::UnexpectedSchema(format!(
            "user object census is {objects:?}"
        )));
    }
    let mut statement = connection.prepare("PRAGMA table_info(fabric_command_record)")?;
    let columns = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let expected = vec![
        (0, "operation_id".to_owned(), "BLOB".to_owned(), 1, 1),
        (1, "idempotency_key".to_owned(), "BLOB".to_owned(), 1, 0),
        (2, "workspace_id".to_owned(), "BLOB".to_owned(), 1, 0),
        (3, "revision".to_owned(), "BLOB".to_owned(), 1, 0),
        (4, "state_kind".to_owned(), "TEXT".to_owned(), 1, 0),
        (5, "is_terminal".to_owned(), "INTEGER".to_owned(), 1, 0),
        (6, "record_jcs".to_owned(), "BLOB".to_owned(), 1, 0),
    ];
    if columns != expected {
        return Err(SqliteCommandRecordOpenError::UnexpectedSchema(format!(
            "column census is {columns:?}"
        )));
    }
    let table_sql: String = connection.query_row(
        "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
        [RECORD_TABLE],
        |row| row.get(0),
    )?;
    for required in [
        "length(operation_id) = 16",
        "length(idempotency_key) = 32",
        "length(workspace_id) = 16",
        "length(revision) = 8",
        "length(record_jcs) BETWEEN 2 AND 65536",
        "WITHOUT ROWID",
        "STRICT",
    ] {
        if !table_sql.contains(required) {
            return Err(SqliteCommandRecordOpenError::UnexpectedSchema(format!(
                "temporal table is missing required constraint {required}"
            )));
        }
    }
    Ok(())
}

fn schema_objects(
    connection: &Connection,
) -> Result<Vec<(String, String)>, SqliteCommandRecordOpenError> {
    let mut statement = connection.prepare(
        "SELECT type, name FROM sqlite_schema
         WHERE name NOT LIKE 'sqlite_%'
         ORDER BY type, name",
    )?;
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fabric::command::{
        ActorId, AdmissionContext, AuthorizationDecision, AuthorizationRef, CommandCancellation,
        CommandEvent, CommandFailure, CommandIdentity, CommandOwnership, CommandPins,
        CommandReducer, CompilerReleaseRef, DiagnosticRef, EpochId, ExpectedHead, FabricCommand,
        FabricCommandPayload, FailureClass, FailureCode, LeaseId, ModelHeadRef, PrincipalId,
        ProofReceiptRef, ProviderSetRef, Reduction, ReductionContext, ResourceEnvelopeRef,
        SourceGeneration, WriterFence, WriterGeneration,
    };

    fn command(seed: u8) -> FabricCommand {
        FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes([seed; 16]),
                idempotency_key: IdempotencyKey::from_bytes([seed; 32]),
            },
            ownership: CommandOwnership {
                workspace_id: WorkspaceId::from_bytes([0x10; 16]),
                principal_id: PrincipalId::from_bytes([0x11; 16]),
                authorization: AuthorizationRef::from_bytes([0x12; 32]),
            },
            expected_head: ExpectedHead::Empty,
            writer_fence: WriterFence {
                lease_id: LeaseId::from_bytes([0x13; 16]),
                generation: WriterGeneration::new(1).unwrap(),
            },
            pins: CommandPins {
                compiler_release: CompilerReleaseRef::from_bytes([0x20; 32]),
                model_head: ModelHeadRef::from_bytes([0x21; 32]),
                source_generation: SourceGeneration::new(0),
                provider_set: ProviderSetRef::from_bytes([0x22; 32]),
            },
            resources: ResourceEnvelopeRef::from_bytes([0x23; 32]),
            payload: FabricCommandPayload::ActivateEpoch {
                candidate_epoch: EpochId::from_bytes([seed.wrapping_add(1); 16]),
                proof_receipt: ProofReceiptRef::from_bytes([seed.wrapping_add(1); 32]),
            },
        }
    }

    fn admitted(command: &FabricCommand) -> CommandRecord {
        CommandReducer::admit(
            None,
            command,
            AdmissionContext {
                workspace_id: command.ownership.workspace_id,
                current_head: command.expected_head,
                active_fence: command.writer_fence,
                authorization: AuthorizationDecision::Authorized(command.ownership.authorization),
            },
        )
        .unwrap()
        .record()
    }

    fn start_reduction(command: &FabricCommand) -> Reduction {
        let admitted = admitted(command);
        CommandReducer::reduce(
            &admitted,
            CommandEvent::Start {
                owner: super::super::command::ExecutionOwner {
                    actor_id: ActorId::from_bytes([0x30; 16]),
                    fence: command.writer_fence,
                },
            },
            ReductionContext {
                current_head: command.expected_head,
                active_fence: command.writer_fence,
            },
        )
        .unwrap()
    }

    fn started(command: &FabricCommand) -> CommandRecord {
        start_reduction(command).record
    }

    fn open_store() -> (tempfile::TempDir, PathBuf, SqliteCommandRecordStore) {
        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let path = root.path().join("commands.sqlite3");
        let store = SqliteCommandRecordStore::open(&path).unwrap();
        (root, path, store)
    }

    #[tokio::test]
    async fn exact_admission_cas_and_restart_reconstruct_canonical_record() {
        let (root, path, store) = open_store();
        let command = command(1);
        let admitted = admitted(&command);
        assert_eq!(
            store
                .lookup_admission(
                    command.identity.operation_id,
                    command.identity.idempotency_key
                )
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            store.insert_if_absent(admitted).await.unwrap(),
            DurableAdmissionWrite::Inserted(admitted)
        );
        assert_eq!(
            store.insert_if_absent(admitted).await.unwrap(),
            DurableAdmissionWrite::Existing(admitted)
        );

        let start = start_reduction(&command);
        let started = start.record;
        assert_eq!(
            store
                .compare_and_swap(start.transition().unwrap())
                .await
                .unwrap(),
            DurableTransitionWrite::Stored(started)
        );
        assert_eq!(
            store
                .compare_and_swap(start.transition().unwrap())
                .await
                .unwrap(),
            DurableTransitionWrite::RevisionConflict {
                observed: Some(started)
            }
        );
        drop(store);

        let reopened = SqliteCommandRecordStore::open(&path).unwrap();
        assert_eq!(
            reopened
                .lookup_operation(command.identity.operation_id)
                .await
                .unwrap(),
            Some(started)
        );
        drop(reopened);
        drop(root);
    }

    #[tokio::test]
    async fn crossed_unique_admission_keys_are_reported_as_corruption() {
        let (_root, _path, store) = open_store();
        let first = command(1);
        let second = command(2);
        store.insert_if_absent(admitted(&first)).await.unwrap();
        store.insert_if_absent(admitted(&second)).await.unwrap();

        assert_eq!(
            store
                .lookup_admission(first.identity.operation_id, second.identity.idempotency_key)
                .await,
            Err(CommandPortError::CorruptRecord)
        );
    }

    #[tokio::test]
    async fn compare_and_swap_cannot_replace_the_admitted_command() {
        let (_root, _path, store) = open_store();
        let original = command(1);
        store.insert_if_absent(admitted(&original)).await.unwrap();

        let mut changed = original;
        changed.payload = FabricCommandPayload::ActivateEpoch {
            candidate_epoch: EpochId::from_bytes([0xee; 16]),
            proof_receipt: ProofReceiptRef::from_bytes([0xef; 32]),
        };
        let changed_start = start_reduction(&changed);
        assert_eq!(
            store
                .compare_and_swap(changed_start.transition().unwrap())
                .await,
            Err(CommandPortError::CorruptRecord)
        );
        assert_eq!(
            store
                .lookup_operation(original.identity.operation_id)
                .await
                .unwrap(),
            Some(admitted(&original))
        );
    }

    #[tokio::test]
    async fn compare_and_swap_rejects_a_valid_transition_from_another_predecessor_state() {
        let (_root, _path, store) = open_store();
        let command = command(7);
        let admitted = admitted(&command);
        store.insert_if_absent(admitted).await.unwrap();
        let owner = super::super::command::ExecutionOwner {
            actor_id: ActorId::from_bytes([0x30; 16]),
            fence: command.writer_fence,
        };
        let cancellation = CommandReducer::reduce(
            &admitted,
            CommandEvent::CancelBeforeCommit {
                owner,
                cancellation: CommandCancellation {
                    diagnostic: DiagnosticRef::from_bytes([0x61; 32]),
                },
            },
            ReductionContext {
                current_head: command.expected_head,
                active_fence: command.writer_fence,
            },
        )
        .unwrap();
        store
            .compare_and_swap(cancellation.transition().unwrap())
            .await
            .unwrap();

        let alternate_executing = started(&command);
        let alternate_failure = CommandReducer::reduce(
            &alternate_executing,
            CommandEvent::ReportKnownFailure {
                owner,
                failure: CommandFailure {
                    code: FailureCode::InvalidInput,
                    class: FailureClass::Permanent,
                    diagnostic: DiagnosticRef::from_bytes([0x62; 32]),
                },
            },
            ReductionContext {
                current_head: command.expected_head,
                active_fence: command.writer_fence,
            },
        )
        .unwrap();
        assert_eq!(
            store
                .compare_and_swap(alternate_failure.transition().unwrap())
                .await,
            Err(CommandPortError::CorruptRecord)
        );
        assert_eq!(
            store
                .lookup_operation(command.identity.operation_id)
                .await
                .unwrap(),
            Some(cancellation.record)
        );
    }

    #[tokio::test]
    async fn recovery_pages_are_bounded_ordered_and_exclude_terminal_records() {
        let (_root, _path, store) = open_store();
        for seed in [3, 1, 2] {
            let command = command(seed);
            let admitted = admitted(&command);
            store.insert_if_absent(admitted).await.unwrap();
            let reduction = if seed == 2 {
                CommandReducer::reduce(
                    &admitted,
                    CommandEvent::CancelBeforeCommit {
                        owner: super::super::command::ExecutionOwner {
                            actor_id: ActorId::from_bytes([0x30; 16]),
                            fence: command.writer_fence,
                        },
                        cancellation: CommandCancellation {
                            diagnostic: DiagnosticRef::from_bytes([0x44; 32]),
                        },
                    },
                    ReductionContext {
                        current_head: command.expected_head,
                        active_fence: command.writer_fence,
                    },
                )
                .unwrap()
            } else {
                start_reduction(&command)
            };
            store
                .compare_and_swap(reduction.transition().unwrap())
                .await
                .unwrap();
        }

        let one = CommandRecoveryPageSize::new(1).unwrap();
        assert!(CommandRecoveryPageSize::new(0).is_none());
        assert!(CommandRecoveryPageSize::new(CommandRecoveryPageSize::MAX + 1).is_none());
        let first = store.load_nonterminal_page(None, one).await.unwrap();
        assert_eq!(first.records().len(), 1);
        assert_eq!(
            first.records()[0].command().identity.operation_id,
            command(1).identity.operation_id
        );
        let second = store
            .load_nonterminal_page(first.next_after(), one)
            .await
            .unwrap();
        assert_eq!(second.records().len(), 1);
        assert_eq!(
            second.records()[0].command().identity.operation_id,
            command(3).identity.operation_id
        );
        assert_eq!(second.next_after(), None);
    }

    #[tokio::test]
    async fn noncanonical_or_column_divergent_records_fail_closed() {
        let (_root, path, store) = open_store();
        let command = command(1);
        store.insert_if_absent(admitted(&command)).await.unwrap();
        drop(store);

        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "UPDATE fabric_command_record SET state_kind = 'SUCCEEDED'
                 WHERE operation_id = ?1",
                [command.identity.operation_id.as_bytes().as_slice()],
            )
            .unwrap();
        drop(connection);

        let reopened = SqliteCommandRecordStore::open(&path).unwrap();
        assert_eq!(
            reopened
                .lookup_operation(command.identity.operation_id)
                .await,
            Err(CommandPortError::CorruptRecord)
        );
    }

    #[tokio::test]
    async fn recovery_scan_cannot_hide_a_nonterminal_record_behind_the_redundant_terminal_column() {
        let (_root, path, store) = open_store();
        let command = command(8);
        let admitted = admitted(&command);
        store.insert_if_absent(admitted).await.unwrap();
        store
            .compare_and_swap(start_reduction(&command).transition().unwrap())
            .await
            .unwrap();
        drop(store);

        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "UPDATE fabric_command_record SET is_terminal = 1 WHERE operation_id = ?1",
                [command.identity.operation_id.as_bytes().as_slice()],
            )
            .unwrap();
        drop(connection);

        let reopened = SqliteCommandRecordStore::open(&path).unwrap();
        assert_eq!(
            reopened
                .load_nonterminal_page(None, CommandRecoveryPageSize::new(1).unwrap())
                .await,
            Err(CommandPortError::CorruptRecord)
        );
    }

    #[tokio::test]
    async fn canonical_but_reducer_impossible_record_fails_closed() {
        let (_root, path, store) = open_store();
        let command = command(2);
        let admitted = admitted(&command);
        store.insert_if_absent(admitted).await.unwrap();
        drop(store);

        let mut impossible = serde_json::to_value(admitted).unwrap();
        impossible["state"]["Admitted"]["attempt"] = serde_json::Value::from(0);
        let canonical = serde_json_canonicalizer::to_vec(&impossible).unwrap();
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "UPDATE fabric_command_record SET record_jcs = ?2 WHERE operation_id = ?1",
                rusqlite::params![
                    command.identity.operation_id.as_bytes().as_slice(),
                    canonical
                ],
            )
            .unwrap();
        drop(connection);

        let reopened = SqliteCommandRecordStore::open(&path).unwrap();
        assert_eq!(
            reopened
                .lookup_operation(command.identity.operation_id)
                .await,
            Err(CommandPortError::CorruptRecord)
        );
    }

    #[test]
    fn unsafe_parent_is_rejected_before_sqlite_open() {
        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(
            SqliteCommandRecordStore::open(&root.path().join("commands.sqlite3")),
            Err(SqliteCommandRecordOpenError::UnsafeParent(_))
        ));
    }
}
