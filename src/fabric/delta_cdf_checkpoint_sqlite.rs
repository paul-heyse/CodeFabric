//! Dedicated SQLite authority for durable Delta CDF consumer checkpoints.
//!
//! This store owns only incremental-transport progress: the last exact source version whose
//! changes were durably applied by one named consumer, plus the exact downstream commit which
//! justified that progress. It cannot select a fabric epoch, discover a latest Delta version, or
//! reinterpret CDF rows. Exact source state remains Delta authority.

use std::fs::{self, File};
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use rusqlite::types::Value;
use rusqlite::{Connection, OpenFlags, OptionalExtension as _, TransactionBehavior, params};
use rustix::fs::{Mode, OFlags, open, openat};
use thiserror::Error;
use tokio::sync::oneshot;
use url::Url;

use super::delta_exact::{DeltaCdfDownstreamCommit, DurableDeltaCdfCheckpoint, ExactDeltaPin};

/// Handwritten schema version for the CDF transport-checkpoint database.
pub const DELTA_CDF_CHECKPOINT_SCHEMA_VERSION: u32 = 1;

const APPLICATION_ID: u32 = 0x4346_4346; // `CFCF`
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const CHECKPOINT_TABLE: &str = "delta_cdf_checkpoint";
const MAX_ROOT_BYTES: usize = 8 * 1024;
const MAX_CONSUMER_BYTES: usize = 1024;
const ZERO_32_HEX: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const SCHEMA_V1: &str = "CREATE TABLE delta_cdf_checkpoint (
    source_root TEXT NOT NULL
        CHECK (typeof(source_root) = 'text'
               AND length(CAST(source_root AS BLOB)) BETWEEN 1 AND 8192),
    consumer_id TEXT NOT NULL
        CHECK (typeof(consumer_id) = 'text'
               AND length(CAST(consumer_id AS BLOB)) BETWEEN 1 AND 1024
               AND length(trim(consumer_id)) > 0),
    cdf_activation_version BLOB NOT NULL
        CHECK (typeof(cdf_activation_version) = 'blob'
               AND length(cdf_activation_version) = 8),
    consumed_version BLOB NOT NULL
        CHECK (typeof(consumed_version) = 'blob'
               AND length(consumed_version) = 8),
    downstream_kind TEXT NOT NULL
        CHECK (downstream_kind IN ('DELTA', 'EXTERNAL')),
    downstream_delta_root TEXT
        CHECK (downstream_delta_root IS NULL
               OR (typeof(downstream_delta_root) = 'text'
                   AND length(CAST(downstream_delta_root AS BLOB)) BETWEEN 1 AND 8192)),
    downstream_delta_version BLOB
        CHECK (downstream_delta_version IS NULL
               OR (typeof(downstream_delta_version) = 'blob'
                   AND length(downstream_delta_version) = 8)),
    downstream_external_identity BLOB
        CHECK (downstream_external_identity IS NULL
               OR (typeof(downstream_external_identity) = 'blob'
                   AND length(downstream_external_identity) = 32
                   AND downstream_external_identity !=
                       X'0000000000000000000000000000000000000000000000000000000000000000')),
    PRIMARY KEY (source_root, consumer_id),
    CHECK (
        (downstream_kind = 'DELTA'
         AND downstream_delta_root IS NOT NULL
         AND downstream_delta_version IS NOT NULL
         AND downstream_external_identity IS NULL)
        OR
        (downstream_kind = 'EXTERNAL'
         AND downstream_delta_root IS NULL
         AND downstream_delta_version IS NULL
         AND downstream_external_identity IS NOT NULL)
    )
) WITHOUT ROWID, STRICT;
PRAGMA application_id = 1128678214;
PRAGMA user_version = 1;";

/// Exact lookup key for one CDF consumer over one canonical Delta table root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeltaCdfCheckpointKey {
    canonical_root: Url,
    consumer_id: Arc<str>,
}

impl DeltaCdfCheckpointKey {
    /// Canonicalize one table root and bind one nonempty consumer identity.
    ///
    /// # Errors
    ///
    /// Rejects an invalid Delta root, empty consumer identity, or identity exceeding the bounded
    /// exact-schema representation.
    pub fn try_new(
        table_root: &Url,
        consumer_id: impl Into<Arc<str>>,
    ) -> Result<Self, DeltaCdfCheckpointStoreError> {
        let canonical = ExactDeltaPin::new(table_root, 0)
            .map_err(|error| DeltaCdfCheckpointStoreError::InvalidRoot(error.to_string()))?;
        let consumer_id = consumer_id.into();
        validate_identity_bounds(canonical.canonical_root(), &consumer_id)?;
        Ok(Self {
            canonical_root: canonical.canonical_root().clone(),
            consumer_id,
        })
    }

    #[must_use]
    pub const fn canonical_root(&self) -> &Url {
        &self.canonical_root
    }

    #[must_use]
    pub fn consumer_id(&self) -> &str {
        &self.consumer_id
    }

    fn from_checkpoint(
        checkpoint: &DurableDeltaCdfCheckpoint,
    ) -> Result<Self, DeltaCdfCheckpointStoreError> {
        Self::try_new(
            checkpoint.consumed_through().canonical_root(),
            checkpoint.consumer_id(),
        )
    }
}

/// Outcome of atomically creating a consumer checkpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeltaCdfCheckpointInsert {
    Inserted(DurableDeltaCdfCheckpoint),
    Existing(DurableDeltaCdfCheckpoint),
}

/// Outcome of an exact monotonic compare-and-swap advancement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeltaCdfCheckpointCompareAndSwap {
    Advanced(DurableDeltaCdfCheckpoint),
    Conflict {
        observed: Option<DurableDeltaCdfCheckpoint>,
    },
}

/// Failures while opening and validating the dedicated checkpoint database.
#[derive(Debug, Error)]
pub enum SqliteDeltaCdfCheckpointOpenError {
    #[error("CDF-checkpoint parent is not a private owned directory: {0}")]
    UnsafeParent(PathBuf),
    #[error("CDF-checkpoint database is not a private owned regular file: {0}")]
    UnsafeDatabase(PathBuf),
    #[error("CDF-checkpoint database path has no file name: {0}")]
    InvalidPath(PathBuf),
    #[error("CDF-checkpoint database I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(
        "unsupported CDF-checkpoint schema version {observed}; supported version is {supported}"
    )]
    UnsupportedSchema { observed: u32, supported: u32 },
    #[error("CDF-checkpoint database schema is not the exact temporal schema: {0}")]
    UnexpectedSchema(String),
    #[error("failed to start the CDF-checkpoint worker: {0}")]
    Worker(std::io::Error),
}

/// Failures while reading or advancing durable CDF transport progress.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DeltaCdfCheckpointStoreError {
    #[error("CDF-checkpoint store is unavailable")]
    Unavailable,
    #[error("CDF-checkpoint store contains a contradictory row")]
    Corrupt,
    #[error("invalid canonical Delta root: {0}")]
    InvalidRoot(String),
    #[error("CDF consumer identity must be nonempty")]
    EmptyConsumerIdentity,
    #[error("CDF checkpoint identity {field} exceeds its exact-schema bound")]
    IdentityTooLong { field: &'static str },
    #[error("CDF checkpoint advancement changed canonical table root or consumer")]
    KeyMismatch,
    #[error("CDF checkpoint advancement changed the CDF activation version")]
    ActivationVersionMismatch,
    #[error(
        "CDF checkpoint advancement is not monotonic: current version {current_version}, proposed version {proposed_version}"
    )]
    NonMonotonic {
        current_version: u64,
        proposed_version: u64,
    },
}

enum StoreRequest {
    Load {
        key: DeltaCdfCheckpointKey,
        response: oneshot::Sender<
            Result<Option<DurableDeltaCdfCheckpoint>, DeltaCdfCheckpointStoreError>,
        >,
    },
    InsertIfAbsent {
        checkpoint: DurableDeltaCdfCheckpoint,
        response: oneshot::Sender<Result<DeltaCdfCheckpointInsert, DeltaCdfCheckpointStoreError>>,
    },
    CompareAndSwap {
        expected: DurableDeltaCdfCheckpoint,
        replacement: DurableDeltaCdfCheckpoint,
        response:
            oneshot::Sender<Result<DeltaCdfCheckpointCompareAndSwap, DeltaCdfCheckpointStoreError>>,
    },
}

/// Private exact-schema SQLite checkpoint authority backed by one blocking worker.
///
/// The async API never performs SQLite work on a Tokio executor. Dropping the final value closes
/// the request channel and joins the worker, which makes process-reopen tests exercise committed
/// WAL state rather than a live connection cache.
pub struct SqliteDeltaCdfCheckpointStore {
    database_path: PathBuf,
    sender: Option<Sender<StoreRequest>>,
    worker: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for SqliteDeltaCdfCheckpointStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SqliteDeltaCdfCheckpointStore")
            .field("database_path", &self.database_path)
            .finish_non_exhaustive()
    }
}

impl SqliteDeltaCdfCheckpointStore {
    /// Open or initialize one private exact-schema CDF-checkpoint database.
    ///
    /// The direct parent must be a non-symlink directory owned by the effective user with mode
    /// `0700`. The database is created as `0600`, pinned by device/inode across the descriptor to
    /// SQLite handoff, and rejected if it contains any undeclared user object.
    ///
    /// # Errors
    ///
    /// Rejects unsafe files, incompatible schemas, SQLite initialization failures, and worker
    /// startup failures.
    pub fn open(path: &Path) -> Result<Self, SqliteDeltaCdfCheckpointOpenError> {
        let prepared_file = prepare_private_database_file(path)?;
        let prepared_metadata =
            prepared_file
                .metadata()
                .map_err(|source| SqliteDeltaCdfCheckpointOpenError::Io {
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
            .name("codefabric-cdf-checkpoints".to_owned())
            .spawn(move || run_worker(connection, receiver))
            .map_err(SqliteDeltaCdfCheckpointOpenError::Worker)?;
        Ok(Self {
            database_path: path.to_owned(),
            sender: Some(sender),
            worker: Some(worker),
        })
    }

    #[must_use]
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    /// Load the exact checkpoint for one canonical table-root/consumer pair.
    ///
    /// `None` means only that no transport checkpoint exists for this exact key. It says nothing
    /// about source-table existence, latest version, or semantic current state.
    ///
    /// # Errors
    ///
    /// Returns unavailable or corrupt state; malformed stored roots, widths, types, variants, and
    /// checkpoint invariants all fail closed.
    pub async fn load(
        &self,
        key: DeltaCdfCheckpointKey,
    ) -> Result<Option<DurableDeltaCdfCheckpoint>, DeltaCdfCheckpointStoreError> {
        let (response, receiver) = oneshot::channel();
        self.send(StoreRequest::Load { key, response })?;
        receiver
            .await
            .map_err(|_| DeltaCdfCheckpointStoreError::Unavailable)?
    }

    /// Atomically create a checkpoint or return the exact row that already owns its key.
    ///
    /// # Errors
    ///
    /// Rejects unrepresentable identities and unavailable or corrupt durable state.
    pub async fn insert_if_absent(
        &self,
        checkpoint: DurableDeltaCdfCheckpoint,
    ) -> Result<DeltaCdfCheckpointInsert, DeltaCdfCheckpointStoreError> {
        validate_checkpoint_storage(&checkpoint)?;
        let (response, receiver) = oneshot::channel();
        self.send(StoreRequest::InsertIfAbsent {
            checkpoint,
            response,
        })?;
        receiver
            .await
            .map_err(|_| DeltaCdfCheckpointStoreError::Unavailable)?
    }

    /// Advance one exact current row with compare-and-swap semantics.
    ///
    /// This method validates only durable transport progression: same key, same CDF activation
    /// boundary, and a strictly increasing consumed version. CDF execution and downstream-success
    /// semantics remain owned by `DurableDeltaCdfCheckpoint::advance_after_downstream_success`.
    ///
    /// # Errors
    ///
    /// Rejects identity drift, activation-boundary drift, non-monotonic versions, unrepresentable
    /// rows, and unavailable or corrupt durable state.
    pub async fn compare_and_swap(
        &self,
        expected: DurableDeltaCdfCheckpoint,
        replacement: DurableDeltaCdfCheckpoint,
    ) -> Result<DeltaCdfCheckpointCompareAndSwap, DeltaCdfCheckpointStoreError> {
        validate_advancement(&expected, &replacement)?;
        let (response, receiver) = oneshot::channel();
        self.send(StoreRequest::CompareAndSwap {
            expected,
            replacement,
            response,
        })?;
        receiver
            .await
            .map_err(|_| DeltaCdfCheckpointStoreError::Unavailable)?
    }

    fn send(&self, request: StoreRequest) -> Result<(), DeltaCdfCheckpointStoreError> {
        self.sender
            .as_ref()
            .ok_or(DeltaCdfCheckpointStoreError::Unavailable)?
            .send(request)
            .map_err(|_| DeltaCdfCheckpointStoreError::Unavailable)
    }
}

impl Drop for SqliteDeltaCdfCheckpointStore {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run_worker(mut connection: Connection, receiver: Receiver<StoreRequest>) {
    while let Ok(request) = receiver.recv() {
        match request {
            StoreRequest::Load { key, response } => {
                let _ = response.send(load_checkpoint_on(&connection, &key));
            }
            StoreRequest::InsertIfAbsent {
                checkpoint,
                response,
            } => {
                let _ = response.send(insert_if_absent_sync(&mut connection, checkpoint));
            }
            StoreRequest::CompareAndSwap {
                expected,
                replacement,
                response,
            } => {
                let _ = response.send(compare_and_swap_sync(
                    &mut connection,
                    expected,
                    replacement,
                ));
            }
        }
    }
}

fn insert_if_absent_sync(
    connection: &mut Connection,
    checkpoint: DurableDeltaCdfCheckpoint,
) -> Result<DeltaCdfCheckpointInsert, DeltaCdfCheckpointStoreError> {
    validate_checkpoint_storage(&checkpoint)?;
    let key = DeltaCdfCheckpointKey::from_checkpoint(&checkpoint)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    let affected = insert_checkpoint(&transaction, &checkpoint)?;
    let observed =
        load_checkpoint_on(&transaction, &key)?.ok_or(DeltaCdfCheckpointStoreError::Corrupt)?;
    let outcome = if affected == 1 {
        if observed != checkpoint {
            return Err(DeltaCdfCheckpointStoreError::Corrupt);
        }
        DeltaCdfCheckpointInsert::Inserted(checkpoint)
    } else {
        DeltaCdfCheckpointInsert::Existing(observed)
    };
    transaction.commit().map_err(unavailable)?;
    Ok(outcome)
}

fn compare_and_swap_sync(
    connection: &mut Connection,
    expected: DurableDeltaCdfCheckpoint,
    replacement: DurableDeltaCdfCheckpoint,
) -> Result<DeltaCdfCheckpointCompareAndSwap, DeltaCdfCheckpointStoreError> {
    validate_advancement(&expected, &replacement)?;
    let key = DeltaCdfCheckpointKey::from_checkpoint(&expected)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    let observed = load_checkpoint_on(&transaction, &key)?;
    let Some(current) = observed else {
        transaction.rollback().map_err(unavailable)?;
        return Ok(DeltaCdfCheckpointCompareAndSwap::Conflict { observed: None });
    };
    if current != expected {
        transaction.rollback().map_err(unavailable)?;
        return Ok(DeltaCdfCheckpointCompareAndSwap::Conflict {
            observed: Some(current),
        });
    }

    let encoded = encode_checkpoint(&replacement)?;
    let expected_version = expected.consumed_through().version().to_be_bytes();
    let affected = transaction
        .execute(
            "UPDATE delta_cdf_checkpoint SET
                 cdf_activation_version = ?3,
                 consumed_version = ?4,
                 downstream_kind = ?5,
                 downstream_delta_root = ?6,
                 downstream_delta_version = ?7,
                 downstream_external_identity = ?8
             WHERE source_root = ?1
               AND consumer_id = ?2
               AND consumed_version = ?9",
            params![
                encoded.source_root,
                encoded.consumer_id,
                encoded.cdf_activation_version,
                encoded.consumed_version,
                encoded.downstream_kind,
                encoded.downstream_delta_root,
                encoded.downstream_delta_version,
                encoded.downstream_external_identity,
                expected_version.as_slice(),
            ],
        )
        .map_err(unavailable)?;
    if affected != 1 {
        return Err(DeltaCdfCheckpointStoreError::Corrupt);
    }
    let readback =
        load_checkpoint_on(&transaction, &key)?.ok_or(DeltaCdfCheckpointStoreError::Corrupt)?;
    if readback != replacement {
        return Err(DeltaCdfCheckpointStoreError::Corrupt);
    }
    transaction.commit().map_err(unavailable)?;
    Ok(DeltaCdfCheckpointCompareAndSwap::Advanced(replacement))
}

fn insert_checkpoint(
    connection: &Connection,
    checkpoint: &DurableDeltaCdfCheckpoint,
) -> Result<usize, DeltaCdfCheckpointStoreError> {
    let encoded = encode_checkpoint(checkpoint)?;
    connection
        .execute(
            "INSERT OR IGNORE INTO delta_cdf_checkpoint (
                 source_root, consumer_id, cdf_activation_version, consumed_version,
                 downstream_kind, downstream_delta_root, downstream_delta_version,
                 downstream_external_identity
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                encoded.source_root,
                encoded.consumer_id,
                encoded.cdf_activation_version,
                encoded.consumed_version,
                encoded.downstream_kind,
                encoded.downstream_delta_root,
                encoded.downstream_delta_version,
                encoded.downstream_external_identity,
            ],
        )
        .map_err(unavailable)
}

struct EncodedCheckpoint {
    source_root: String,
    consumer_id: String,
    cdf_activation_version: Vec<u8>,
    consumed_version: Vec<u8>,
    downstream_kind: &'static str,
    downstream_delta_root: Option<String>,
    downstream_delta_version: Option<Vec<u8>>,
    downstream_external_identity: Option<Vec<u8>>,
}

fn encode_checkpoint(
    checkpoint: &DurableDeltaCdfCheckpoint,
) -> Result<EncodedCheckpoint, DeltaCdfCheckpointStoreError> {
    validate_checkpoint_storage(checkpoint)?;
    let (downstream_kind, downstream_delta_root, downstream_delta_version, external_identity) =
        match checkpoint.downstream_commit() {
            DeltaCdfDownstreamCommit::Delta(pin) => {
                validate_root_bound(pin.canonical_root(), "downstream_delta_root")?;
                (
                    "DELTA",
                    Some(pin.canonical_root().to_string()),
                    Some(pin.version().to_be_bytes().to_vec()),
                    None,
                )
            }
            DeltaCdfDownstreamCommit::External(identity) => {
                ("EXTERNAL", None, None, Some(identity.to_vec()))
            }
        };
    Ok(EncodedCheckpoint {
        source_root: checkpoint.consumed_through().canonical_root().to_string(),
        consumer_id: checkpoint.consumer_id().to_owned(),
        cdf_activation_version: checkpoint.cdf_activation_version().to_be_bytes().to_vec(),
        consumed_version: checkpoint
            .consumed_through()
            .version()
            .to_be_bytes()
            .to_vec(),
        downstream_kind,
        downstream_delta_root,
        downstream_delta_version,
        downstream_external_identity: external_identity,
    })
}

fn load_checkpoint_on(
    connection: &Connection,
    key: &DeltaCdfCheckpointKey,
) -> Result<Option<DurableDeltaCdfCheckpoint>, DeltaCdfCheckpointStoreError> {
    connection
        .query_row(
            "SELECT source_root, consumer_id, cdf_activation_version, consumed_version,
                    downstream_kind, downstream_delta_root, downstream_delta_version,
                    downstream_external_identity
             FROM delta_cdf_checkpoint
             WHERE source_root = ?1 AND consumer_id = ?2",
            params![key.canonical_root().as_str(), key.consumer_id()],
            stored_row,
        )
        .optional()
        .map_err(unavailable)?
        .map(|row| decode_checkpoint(row, key))
        .transpose()
}

struct StoredCheckpointRow {
    source_root: Value,
    consumer_id: Value,
    cdf_activation_version: Value,
    consumed_version: Value,
    downstream_kind: Value,
    downstream_delta_root: Value,
    downstream_delta_version: Value,
    downstream_external_identity: Value,
}

fn stored_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredCheckpointRow> {
    Ok(StoredCheckpointRow {
        source_root: row.get(0)?,
        consumer_id: row.get(1)?,
        cdf_activation_version: row.get(2)?,
        consumed_version: row.get(3)?,
        downstream_kind: row.get(4)?,
        downstream_delta_root: row.get(5)?,
        downstream_delta_version: row.get(6)?,
        downstream_external_identity: row.get(7)?,
    })
}

fn decode_checkpoint(
    row: StoredCheckpointRow,
    expected_key: &DeltaCdfCheckpointKey,
) -> Result<DurableDeltaCdfCheckpoint, DeltaCdfCheckpointStoreError> {
    let source_root = decode_text(row.source_root, MAX_ROOT_BYTES)?;
    let consumer_id = decode_text(row.consumer_id, MAX_CONSUMER_BYTES)?;
    if consumer_id.trim().is_empty() {
        return Err(DeltaCdfCheckpointStoreError::Corrupt);
    }
    let activation_version = decode_u64(row.cdf_activation_version)?;
    let consumed_version = decode_u64(row.consumed_version)?;
    let parsed_source =
        Url::parse(&source_root).map_err(|_| DeltaCdfCheckpointStoreError::Corrupt)?;
    let consumed_through = ExactDeltaPin::new(&parsed_source, consumed_version)
        .map_err(|_| DeltaCdfCheckpointStoreError::Corrupt)?;
    if consumed_through.canonical_root().as_str() != source_root {
        return Err(DeltaCdfCheckpointStoreError::Corrupt);
    }
    let observed_key = DeltaCdfCheckpointKey::try_new(
        consumed_through.canonical_root(),
        Arc::<str>::from(consumer_id.as_str()),
    )
    .map_err(|_| DeltaCdfCheckpointStoreError::Corrupt)?;
    if &observed_key != expected_key {
        return Err(DeltaCdfCheckpointStoreError::Corrupt);
    }

    let Value::Text(kind) = row.downstream_kind else {
        return Err(DeltaCdfCheckpointStoreError::Corrupt);
    };
    let downstream_commit = match kind.as_str() {
        "DELTA" => {
            let root = decode_text(row.downstream_delta_root, MAX_ROOT_BYTES)?;
            let version = decode_u64(row.downstream_delta_version)?;
            if row.downstream_external_identity != Value::Null {
                return Err(DeltaCdfCheckpointStoreError::Corrupt);
            }
            let parsed = Url::parse(&root).map_err(|_| DeltaCdfCheckpointStoreError::Corrupt)?;
            let pin = ExactDeltaPin::new(&parsed, version)
                .map_err(|_| DeltaCdfCheckpointStoreError::Corrupt)?;
            if pin.canonical_root().as_str() != root {
                return Err(DeltaCdfCheckpointStoreError::Corrupt);
            }
            DeltaCdfDownstreamCommit::Delta(pin)
        }
        "EXTERNAL" => {
            if row.downstream_delta_root != Value::Null
                || row.downstream_delta_version != Value::Null
            {
                return Err(DeltaCdfCheckpointStoreError::Corrupt);
            }
            let identity = decode_blob::<32>(row.downstream_external_identity)?;
            if identity.iter().all(|byte| *byte == 0) {
                return Err(DeltaCdfCheckpointStoreError::Corrupt);
            }
            DeltaCdfDownstreamCommit::External(identity)
        }
        _ => return Err(DeltaCdfCheckpointStoreError::Corrupt),
    };
    DurableDeltaCdfCheckpoint::try_new(
        consumer_id,
        activation_version,
        consumed_through,
        downstream_commit,
    )
    .map_err(|_| DeltaCdfCheckpointStoreError::Corrupt)
}

fn decode_text(value: Value, max_bytes: usize) -> Result<String, DeltaCdfCheckpointStoreError> {
    let Value::Text(value) = value else {
        return Err(DeltaCdfCheckpointStoreError::Corrupt);
    };
    if value.is_empty() || value.len() > max_bytes {
        return Err(DeltaCdfCheckpointStoreError::Corrupt);
    }
    Ok(value)
}

fn decode_u64(value: Value) -> Result<u64, DeltaCdfCheckpointStoreError> {
    Ok(u64::from_be_bytes(decode_blob::<8>(value)?))
}

fn decode_blob<const WIDTH: usize>(
    value: Value,
) -> Result<[u8; WIDTH], DeltaCdfCheckpointStoreError> {
    let Value::Blob(value) = value else {
        return Err(DeltaCdfCheckpointStoreError::Corrupt);
    };
    value
        .try_into()
        .map_err(|_| DeltaCdfCheckpointStoreError::Corrupt)
}

fn validate_advancement(
    expected: &DurableDeltaCdfCheckpoint,
    replacement: &DurableDeltaCdfCheckpoint,
) -> Result<(), DeltaCdfCheckpointStoreError> {
    validate_checkpoint_storage(expected)?;
    validate_checkpoint_storage(replacement)?;
    if DeltaCdfCheckpointKey::from_checkpoint(expected)?
        != DeltaCdfCheckpointKey::from_checkpoint(replacement)?
    {
        return Err(DeltaCdfCheckpointStoreError::KeyMismatch);
    }
    if expected.cdf_activation_version() != replacement.cdf_activation_version() {
        return Err(DeltaCdfCheckpointStoreError::ActivationVersionMismatch);
    }
    let current_version = expected.consumed_through().version();
    let proposed_version = replacement.consumed_through().version();
    if proposed_version <= current_version {
        return Err(DeltaCdfCheckpointStoreError::NonMonotonic {
            current_version,
            proposed_version,
        });
    }
    Ok(())
}

fn validate_checkpoint_storage(
    checkpoint: &DurableDeltaCdfCheckpoint,
) -> Result<(), DeltaCdfCheckpointStoreError> {
    validate_identity_bounds(
        checkpoint.consumed_through().canonical_root(),
        checkpoint.consumer_id(),
    )?;
    if let DeltaCdfDownstreamCommit::Delta(pin) = checkpoint.downstream_commit() {
        validate_root_bound(pin.canonical_root(), "downstream_delta_root")?;
    }
    Ok(())
}

fn validate_identity_bounds(
    root: &Url,
    consumer_id: &str,
) -> Result<(), DeltaCdfCheckpointStoreError> {
    validate_root_bound(root, "source_root")?;
    if consumer_id.trim().is_empty() {
        return Err(DeltaCdfCheckpointStoreError::EmptyConsumerIdentity);
    }
    if consumer_id.len() > MAX_CONSUMER_BYTES {
        return Err(DeltaCdfCheckpointStoreError::IdentityTooLong {
            field: "consumer_id",
        });
    }
    Ok(())
}

fn validate_root_bound(
    root: &Url,
    field: &'static str,
) -> Result<(), DeltaCdfCheckpointStoreError> {
    if root.as_str().is_empty() || root.as_str().len() > MAX_ROOT_BYTES {
        return Err(DeltaCdfCheckpointStoreError::IdentityTooLong { field });
    }
    Ok(())
}

fn unavailable(_error: rusqlite::Error) -> DeltaCdfCheckpointStoreError {
    DeltaCdfCheckpointStoreError::Unavailable
}

fn prepare_private_database_file(path: &Path) -> Result<File, SqliteDeltaCdfCheckpointOpenError> {
    let parent = path
        .parent()
        .filter(|candidate| !candidate.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent_metadata = fs::symlink_metadata(parent)
        .map_err(|_| SqliteDeltaCdfCheckpointOpenError::UnsafeParent(parent.to_owned()))?;
    let owner = rustix::process::geteuid().as_raw();
    if !parent_metadata.is_dir()
        || parent_metadata.file_type().is_symlink()
        || parent_metadata.uid() != owner
        || parent_metadata.permissions().mode() & 0o777 != 0o700
    {
        return Err(SqliteDeltaCdfCheckpointOpenError::UnsafeParent(
            parent.to_owned(),
        ));
    }
    let file_name = path
        .file_name()
        .ok_or_else(|| SqliteDeltaCdfCheckpointOpenError::InvalidPath(path.to_owned()))?;
    let directory = open(
        parent,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY,
        Mode::empty(),
    )
    .map_err(|_| SqliteDeltaCdfCheckpointOpenError::UnsafeParent(parent.to_owned()))?;
    let descriptor = openat(
        &directory,
        file_name,
        OFlags::RDWR | OFlags::CREATE | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|_| SqliteDeltaCdfCheckpointOpenError::UnsafeDatabase(path.to_owned()))?;
    let file = File::from(descriptor);
    let metadata = file
        .metadata()
        .map_err(|source| SqliteDeltaCdfCheckpointOpenError::Io {
            path: path.to_owned(),
            source,
        })?;
    if !private_database_metadata(&metadata, owner) {
        return Err(SqliteDeltaCdfCheckpointOpenError::UnsafeDatabase(
            path.to_owned(),
        ));
    }
    file.sync_all()
        .map_err(|source| SqliteDeltaCdfCheckpointOpenError::Io {
            path: path.to_owned(),
            source,
        })?;
    Ok(file)
}

fn validate_same_private_file(
    path: &Path,
    prepared: &fs::Metadata,
) -> Result<(), SqliteDeltaCdfCheckpointOpenError> {
    let observed = fs::symlink_metadata(path)
        .map_err(|_| SqliteDeltaCdfCheckpointOpenError::UnsafeDatabase(path.to_owned()))?;
    if !private_database_metadata(&observed, rustix::process::geteuid().as_raw())
        || observed.dev() != prepared.dev()
        || observed.ino() != prepared.ino()
    {
        return Err(SqliteDeltaCdfCheckpointOpenError::UnsafeDatabase(
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

fn apply_pragmas(connection: &Connection) -> Result<(), SqliteDeltaCdfCheckpointOpenError> {
    connection.busy_timeout(BUSY_TIMEOUT)?;
    let journal_mode: String =
        connection.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
    if journal_mode != "wal" {
        return Err(SqliteDeltaCdfCheckpointOpenError::UnexpectedSchema(
            format!("journal_mode is {journal_mode}, expected wal"),
        ));
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
        return Err(SqliteDeltaCdfCheckpointOpenError::UnexpectedSchema(
            format!("synchronous is {synchronous}, expected FULL (2)"),
        ));
    }
    Ok(())
}

fn initialize_or_validate_schema(
    connection: &mut Connection,
) -> Result<(), SqliteDeltaCdfCheckpointOpenError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let observed_version: u32 =
        transaction.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let observed_application: u32 =
        transaction.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    match observed_version {
        0 => {
            if observed_application != 0 || !schema_objects(&transaction)?.is_empty() {
                return Err(SqliteDeltaCdfCheckpointOpenError::UnexpectedSchema(
                    "unversioned database already contains application identity or objects"
                        .to_owned(),
                ));
            }
            transaction.execute_batch(SCHEMA_V1)?;
        }
        DELTA_CDF_CHECKPOINT_SCHEMA_VERSION => {
            if observed_application != APPLICATION_ID {
                return Err(SqliteDeltaCdfCheckpointOpenError::UnexpectedSchema(
                    format!("application_id is {observed_application}, expected {APPLICATION_ID}"),
                ));
            }
        }
        observed => {
            return Err(SqliteDeltaCdfCheckpointOpenError::UnsupportedSchema {
                observed,
                supported: DELTA_CDF_CHECKPOINT_SCHEMA_VERSION,
            });
        }
    }
    validate_schema(&transaction)?;
    transaction.commit()?;
    Ok(())
}

fn validate_schema(connection: &Connection) -> Result<(), SqliteDeltaCdfCheckpointOpenError> {
    let version: u32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let application: u32 = connection.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    if version != DELTA_CDF_CHECKPOINT_SCHEMA_VERSION || application != APPLICATION_ID {
        return Err(SqliteDeltaCdfCheckpointOpenError::UnexpectedSchema(
            "version or application identity changed during schema validation".to_owned(),
        ));
    }
    let objects = schema_objects(connection)?;
    if objects != vec![("table".to_owned(), CHECKPOINT_TABLE.to_owned())] {
        return Err(SqliteDeltaCdfCheckpointOpenError::UnexpectedSchema(
            format!("user object census is {objects:?}"),
        ));
    }
    let mut statement = connection.prepare("PRAGMA table_info(delta_cdf_checkpoint)")?;
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
        (0, "source_root".to_owned(), "TEXT".to_owned(), 1, 1),
        (1, "consumer_id".to_owned(), "TEXT".to_owned(), 1, 2),
        (
            2,
            "cdf_activation_version".to_owned(),
            "BLOB".to_owned(),
            1,
            0,
        ),
        (3, "consumed_version".to_owned(), "BLOB".to_owned(), 1, 0),
        (4, "downstream_kind".to_owned(), "TEXT".to_owned(), 1, 0),
        (
            5,
            "downstream_delta_root".to_owned(),
            "TEXT".to_owned(),
            0,
            0,
        ),
        (
            6,
            "downstream_delta_version".to_owned(),
            "BLOB".to_owned(),
            0,
            0,
        ),
        (
            7,
            "downstream_external_identity".to_owned(),
            "BLOB".to_owned(),
            0,
            0,
        ),
    ];
    if columns != expected {
        return Err(SqliteDeltaCdfCheckpointOpenError::UnexpectedSchema(
            format!("column census is {columns:?}"),
        ));
    }
    let table_sql: String = connection.query_row(
        "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
        [CHECKPOINT_TABLE],
        |row| row.get(0),
    )?;
    for required in [
        "length(CAST(source_root AS BLOB)) BETWEEN 1 AND 8192",
        "length(CAST(consumer_id AS BLOB)) BETWEEN 1 AND 1024",
        "length(cdf_activation_version) = 8",
        "length(consumed_version) = 8",
        "downstream_kind IN ('DELTA', 'EXTERNAL')",
        "length(downstream_delta_version) = 8",
        "length(downstream_external_identity) = 32",
        ZERO_32_HEX,
        "PRIMARY KEY (source_root, consumer_id)",
        "WITHOUT ROWID",
        "STRICT",
    ] {
        if !table_sql.contains(required) {
            return Err(SqliteDeltaCdfCheckpointOpenError::UnexpectedSchema(
                format!("checkpoint table is missing required constraint {required}"),
            ));
        }
    }
    Ok(())
}

fn schema_objects(
    connection: &Connection,
) -> Result<Vec<(String, String)>, SqliteDeltaCdfCheckpointOpenError> {
    let mut statement = connection.prepare(
        "SELECT type, name FROM sqlite_schema
         WHERE name NOT LIKE 'sqlite_%'
         ORDER BY type, name",
    )?;
    Ok(statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn private_root() -> TempDir {
        let root = TempDir::new().expect("temporary CDF-checkpoint directory");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("make CDF-checkpoint directory private");
        root
    }

    fn database_path(root: &TempDir) -> PathBuf {
        root.path().join("delta-cdf-checkpoints.sqlite3")
    }

    fn table_root(root: &TempDir, name: &str) -> Url {
        let path = root.path().join(name);
        fs::create_dir(&path).expect("create local Delta-root fixture");
        Url::from_directory_path(path).expect("local fixture has a file URL")
    }

    fn pin(root: &Url, version: u64) -> ExactDeltaPin {
        ExactDeltaPin::new(root, version).expect("construct exact Delta pin")
    }

    fn checkpoint(
        source_root: &Url,
        consumer: &str,
        activation_version: u64,
        consumed_version: u64,
        downstream_commit: DeltaCdfDownstreamCommit,
    ) -> DurableDeltaCdfCheckpoint {
        DurableDeltaCdfCheckpoint::try_new(
            consumer,
            activation_version,
            pin(source_root, consumed_version),
            downstream_commit,
        )
        .expect("construct durable CDF checkpoint")
    }

    #[tokio::test]
    async fn process_reopen_round_trips_both_commit_variants_and_full_u64_versions() {
        let root = private_root();
        let database = database_path(&root);
        let source_delta = table_root(&root, "source-delta");
        let source_external = table_root(&root, "source-external");
        let downstream = table_root(&root, "downstream-delta");
        let delta = checkpoint(
            &source_delta,
            "delta-materializer",
            u64::MAX - 12,
            u64::MAX - 8,
            DeltaCdfDownstreamCommit::Delta(pin(&downstream, u64::MAX - 3)),
        );
        let external = checkpoint(
            &source_external,
            "search-index",
            7,
            19,
            DeltaCdfDownstreamCommit::External([0x5a; 32]),
        );
        let delta_key = DeltaCdfCheckpointKey::try_new(&source_delta, "delta-materializer")
            .expect("construct exact Delta checkpoint key");
        let external_key = DeltaCdfCheckpointKey::try_new(&source_external, "search-index")
            .expect("construct exact external checkpoint key");
        let mut decorated_source = source_delta.clone();
        decorated_source.set_query(Some("not-table-identity"));
        decorated_source.set_fragment(Some("also-not-table-identity"));
        assert_eq!(
            DeltaCdfCheckpointKey::try_new(&decorated_source, "delta-materializer").unwrap(),
            delta_key
        );

        {
            let store = SqliteDeltaCdfCheckpointStore::open(&database)
                .expect("open initial CDF-checkpoint store");
            assert_eq!(
                store.insert_if_absent(delta.clone()).await.unwrap(),
                DeltaCdfCheckpointInsert::Inserted(delta.clone())
            );
            assert_eq!(
                store.insert_if_absent(external.clone()).await.unwrap(),
                DeltaCdfCheckpointInsert::Inserted(external.clone())
            );
            assert_eq!(
                store.load(delta_key.clone()).await.unwrap(),
                Some(delta.clone())
            );
            assert_eq!(
                store.load(external_key.clone()).await.unwrap(),
                Some(external.clone())
            );
        }

        assert_eq!(
            fs::symlink_metadata(&database)
                .expect("CDF-checkpoint database metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let reopened =
            SqliteDeltaCdfCheckpointStore::open(&database).expect("reopen CDF-checkpoint store");
        assert_eq!(reopened.load(delta_key).await.unwrap(), Some(delta));
        assert_eq!(reopened.load(external_key).await.unwrap(), Some(external));
        assert_eq!(
            reopened
                .load(DeltaCdfCheckpointKey::try_new(&source_delta, "different-consumer").unwrap())
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn monotonic_compare_and_swap_rejects_stale_and_mismatched_advancement() {
        let root = private_root();
        let database = database_path(&root);
        let source = table_root(&root, "source");
        let downstream = table_root(&root, "downstream");
        let store =
            SqliteDeltaCdfCheckpointStore::open(&database).expect("open CDF-checkpoint store");
        let initial = checkpoint(
            &source,
            "projection",
            2,
            4,
            DeltaCdfDownstreamCommit::External([1; 32]),
        );
        let advanced = checkpoint(
            &source,
            "projection",
            2,
            5,
            DeltaCdfDownstreamCommit::Delta(pin(&downstream, 11)),
        );
        assert!(matches!(
            store.insert_if_absent(initial.clone()).await.unwrap(),
            DeltaCdfCheckpointInsert::Inserted(_)
        ));
        assert_eq!(
            store
                .compare_and_swap(initial.clone(), advanced.clone())
                .await
                .unwrap(),
            DeltaCdfCheckpointCompareAndSwap::Advanced(advanced.clone())
        );

        let stale_replacement = checkpoint(
            &source,
            "projection",
            2,
            6,
            DeltaCdfDownstreamCommit::External([2; 32]),
        );
        assert_eq!(
            store
                .compare_and_swap(initial.clone(), stale_replacement)
                .await
                .unwrap(),
            DeltaCdfCheckpointCompareAndSwap::Conflict {
                observed: Some(advanced.clone())
            }
        );
        assert_eq!(
            store
                .compare_and_swap(
                    advanced.clone(),
                    checkpoint(
                        &source,
                        "projection",
                        2,
                        4,
                        DeltaCdfDownstreamCommit::External([3; 32]),
                    ),
                )
                .await,
            Err(DeltaCdfCheckpointStoreError::NonMonotonic {
                current_version: 5,
                proposed_version: 4,
            })
        );
        assert_eq!(
            store
                .compare_and_swap(
                    advanced.clone(),
                    checkpoint(
                        &source,
                        "projection",
                        3,
                        6,
                        DeltaCdfDownstreamCommit::External([4; 32]),
                    ),
                )
                .await,
            Err(DeltaCdfCheckpointStoreError::ActivationVersionMismatch)
        );
        assert_eq!(
            store
                .compare_and_swap(
                    advanced.clone(),
                    checkpoint(
                        &source,
                        "other-projection",
                        2,
                        6,
                        DeltaCdfDownstreamCommit::External([5; 32]),
                    ),
                )
                .await,
            Err(DeltaCdfCheckpointStoreError::KeyMismatch)
        );
        drop(store);

        let reopened = SqliteDeltaCdfCheckpointStore::open(&database)
            .expect("reopen advanced CDF-checkpoint store");
        assert_eq!(
            reopened
                .load(DeltaCdfCheckpointKey::try_new(&source, "projection").unwrap())
                .await
                .unwrap(),
            Some(advanced)
        );
    }

    #[tokio::test]
    async fn concurrent_exact_compare_and_swap_has_one_winner() {
        let root = private_root();
        let database = database_path(&root);
        let source = table_root(&root, "source");
        let first = SqliteDeltaCdfCheckpointStore::open(&database).expect("open first store");
        let second = SqliteDeltaCdfCheckpointStore::open(&database).expect("open second store");
        let initial = checkpoint(
            &source,
            "consumer",
            1,
            3,
            DeltaCdfDownstreamCommit::External([1; 32]),
        );
        first.insert_if_absent(initial.clone()).await.unwrap();
        let candidate_a = checkpoint(
            &source,
            "consumer",
            1,
            4,
            DeltaCdfDownstreamCommit::External([0xaa; 32]),
        );
        let candidate_b = checkpoint(
            &source,
            "consumer",
            1,
            4,
            DeltaCdfDownstreamCommit::External([0xbb; 32]),
        );

        let (result_a, result_b) = tokio::join!(
            first.compare_and_swap(initial.clone(), candidate_a.clone()),
            second.compare_and_swap(initial, candidate_b.clone()),
        );
        let result_a = result_a.unwrap();
        let result_b = result_b.unwrap();
        let winner = match (&result_a, &result_b) {
            (
                DeltaCdfCheckpointCompareAndSwap::Advanced(winner),
                DeltaCdfCheckpointCompareAndSwap::Conflict {
                    observed: Some(observed),
                },
            )
            | (
                DeltaCdfCheckpointCompareAndSwap::Conflict {
                    observed: Some(observed),
                },
                DeltaCdfCheckpointCompareAndSwap::Advanced(winner),
            ) => {
                assert_eq!(observed, winner);
                winner.clone()
            }
            outcomes => panic!("expected one exact CAS winner and one conflict, got {outcomes:?}"),
        };
        assert!(winner == candidate_a || winner == candidate_b);
    }

    #[tokio::test]
    async fn strict_decoder_rejects_malformed_delta_and_external_variants() {
        let root = private_root();
        let database = database_path(&root);
        let source = table_root(&root, "source");
        let downstream = table_root(&root, "downstream");
        let external = checkpoint(
            &source,
            "external",
            1,
            2,
            DeltaCdfDownstreamCommit::External([7; 32]),
        );
        let delta = checkpoint(
            &source,
            "delta",
            1,
            2,
            DeltaCdfDownstreamCommit::Delta(pin(&downstream, 8)),
        );
        let store = SqliteDeltaCdfCheckpointStore::open(&database).expect("open store");
        store.insert_if_absent(external).await.unwrap();
        store.insert_if_absent(delta).await.unwrap();
        drop(store);

        let connection = Connection::open(&database).expect("open corruption fixture");
        connection
            .execute_batch("PRAGMA ignore_check_constraints=ON;")
            .unwrap();
        connection
            .execute(
                "UPDATE delta_cdf_checkpoint
                 SET downstream_external_identity = ?2
                 WHERE consumer_id = ?1",
                params!["external", vec![7_u8; 31]],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE delta_cdf_checkpoint
                 SET downstream_delta_version = ?2
                 WHERE consumer_id = ?1",
                params!["delta", vec![8_u8; 7]],
            )
            .unwrap();
        drop(connection);

        let reopened = SqliteDeltaCdfCheckpointStore::open(&database)
            .expect("schema remains structurally valid");
        for consumer in ["external", "delta"] {
            assert_eq!(
                reopened
                    .load(DeltaCdfCheckpointKey::try_new(&source, consumer).unwrap())
                    .await,
                Err(DeltaCdfCheckpointStoreError::Corrupt)
            );
        }
    }

    #[test]
    fn strict_scalar_decoders_reject_wrong_storage_classes_and_widths() {
        assert_eq!(
            decode_blob::<8>(Value::Integer(7)),
            Err(DeltaCdfCheckpointStoreError::Corrupt)
        );
        assert_eq!(
            decode_blob::<8>(Value::Blob(vec![7; 7])),
            Err(DeltaCdfCheckpointStoreError::Corrupt)
        );
        assert_eq!(
            decode_text(Value::Blob(b"file:///table/".to_vec()), MAX_ROOT_BYTES),
            Err(DeltaCdfCheckpointStoreError::Corrupt)
        );
    }

    #[test]
    fn unsafe_parent_and_expanded_schema_are_rejected() {
        let public_root = TempDir::new().expect("temporary public directory");
        fs::set_permissions(public_root.path(), fs::Permissions::from_mode(0o755))
            .expect("make fixture public");
        let public_database = database_path(&public_root);
        assert!(matches!(
            SqliteDeltaCdfCheckpointStore::open(&public_database),
            Err(SqliteDeltaCdfCheckpointOpenError::UnsafeParent(_))
        ));
        assert!(!public_database.exists());

        let private_root = private_root();
        let database = database_path(&private_root);
        drop(SqliteDeltaCdfCheckpointStore::open(&database).expect("initialize exact schema"));
        let connection = Connection::open(&database).expect("open schema fixture");
        connection
            .execute("CREATE TABLE semantic_current (epoch BLOB) STRICT", [])
            .unwrap();
        drop(connection);
        assert!(matches!(
            SqliteDeltaCdfCheckpointStore::open(&database),
            Err(SqliteDeltaCdfCheckpointOpenError::UnexpectedSchema(_))
        ));
    }
}
