//! Dedicated temporal SQLite persistence for durable writer generations.
//!
//! This database is deliberately incapable of selecting semantic current state. Its sole user
//! table records the latest generation/lease pair per workspace plus acquisition time. An exact
//! schema census prevents an operational or semantic database from being reused accidentally.

use std::fs::{self, File};
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::types::Value;
use rusqlite::{Connection, OpenFlags, OptionalExtension as _, TransactionBehavior, params};
use rustix::fs::{Mode, OFlags, open, openat};
use thiserror::Error;

use super::command::{LeaseId, WorkspaceId, WriterFence, WriterGeneration};
use super::writer_lease::{DurableWriterGenerationPort, WriterGenerationPortError};

/// Handwritten temporal schema version. This store never consumes generated semantic DDL.
pub const WRITER_GENERATION_SCHEMA_VERSION: u32 = 1;

const APPLICATION_ID: u32 = 0x4346_4757; // `CFGW`
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const STATE_TABLE: &str = "writer_generation_state";
const SCHEMA_V1: &str = "CREATE TABLE writer_generation_state (
    workspace_id BLOB NOT NULL PRIMARY KEY
        CHECK (typeof(workspace_id) = 'blob' AND length(workspace_id) = 16),
    current_generation BLOB NOT NULL
        CHECK (typeof(current_generation) = 'blob'
               AND length(current_generation) = 8
               AND current_generation != X'0000000000000000'),
    current_lease_id BLOB NOT NULL
        CHECK (typeof(current_lease_id) = 'blob' AND length(current_lease_id) = 16),
    acquired_at_unix_micros INTEGER NOT NULL
        CHECK (typeof(acquired_at_unix_micros) = 'integer'
               AND acquired_at_unix_micros >= 0)
) WITHOUT ROWID, STRICT;
PRAGMA application_id = 1128679255;
PRAGMA user_version = 1;";

/// Failures while opening and validating the dedicated generation database.
#[derive(Debug, Error)]
pub enum SqliteWriterGenerationOpenError {
    #[error("writer-generation database parent is not a private owned directory: {0}")]
    UnsafeParent(PathBuf),
    #[error("writer-generation database is not a private owned regular file: {0}")]
    UnsafeDatabase(PathBuf),
    #[error("writer-generation database path has no file name: {0}")]
    InvalidPath(PathBuf),
    #[error("writer-generation database I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(
        "unsupported writer-generation schema version {observed}; supported version is {supported}"
    )]
    UnsupportedSchema { observed: u32, supported: u32 },
    #[error("writer-generation database schema is not the exact temporal schema: {0}")]
    UnexpectedSchema(String),
}

/// One dedicated temporal database with one mutex-owned writer connection.
///
/// Separate instances may open the same database. `BEGIN IMMEDIATE` and the busy timeout preserve
/// atomic allocation across those connections; the mutex prevents concurrent use of this
/// instance's sole SQLite connection.
pub struct SqliteWriterGenerationStore {
    database_path: PathBuf,
    connection: Mutex<Connection>,
}

impl std::fmt::Debug for SqliteWriterGenerationStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SqliteWriterGenerationStore")
            .field("database_path", &self.database_path)
            .finish_non_exhaustive()
    }
}

impl SqliteWriterGenerationStore {
    /// Open or initialize one exact-schema private temporal database.
    ///
    /// The direct parent must already exist, be owned by the effective user, be a non-symlink
    /// directory, and have mode `0700`. The database is created as `0600`; existing files with
    /// another owner, type, link count, or mode are rejected before SQLite opens them.
    ///
    /// # Errors
    ///
    /// Rejects unsafe paths/files, incompatible or expanded schemas, and SQLite initialization
    /// failures.
    pub fn open(path: &Path) -> Result<Self, SqliteWriterGenerationOpenError> {
        let prepared_file = prepare_private_database_file(path)?;
        let prepared_metadata =
            prepared_file
                .metadata()
                .map_err(|source| SqliteWriterGenerationOpenError::Io {
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
        Ok(Self {
            database_path: path.to_owned(),
            connection: Mutex::new(connection),
        })
    }

    /// Path of this store's dedicated database.
    #[must_use]
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }
}

impl DurableWriterGenerationPort for SqliteWriterGenerationStore {
    fn allocate_next(
        &self,
        workspace_id: WorkspaceId,
        lease_id: LeaseId,
    ) -> Result<WriterGeneration, WriterGenerationPortError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| WriterGenerationPortError::Unavailable)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| WriterGenerationPortError::Unavailable)?;
        let current = transaction
            .query_row(
                "SELECT current_generation, current_lease_id, acquired_at_unix_micros
                 FROM writer_generation_state
                 WHERE workspace_id = ?1",
                params![workspace_id.as_bytes().as_slice()],
                |row| {
                    Ok((
                        row.get::<_, Value>(0)?,
                        row.get::<_, Value>(1)?,
                        row.get::<_, Value>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| WriterGenerationPortError::Unavailable)?;
        let next = match current {
            Some(row) => decode_temporal_row(row)?
                .generation
                .get()
                .checked_add(1)
                .ok_or(WriterGenerationPortError::Exhausted)?,
            None => 1,
        };
        let generation = WriterGeneration::new(next).ok_or(WriterGenerationPortError::Exhausted)?;
        let generation_bytes = next.to_be_bytes();
        let acquired_at = unix_micros()?;
        let affected = transaction
            .execute(
                "INSERT INTO writer_generation_state (
                     workspace_id,
                     current_generation,
                     current_lease_id,
                     acquired_at_unix_micros
                 ) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(workspace_id) DO UPDATE SET
                     current_generation = excluded.current_generation,
                     current_lease_id = excluded.current_lease_id,
                     acquired_at_unix_micros = excluded.acquired_at_unix_micros",
                params![
                    workspace_id.as_bytes().as_slice(),
                    generation_bytes.as_slice(),
                    lease_id.as_bytes().as_slice(),
                    acquired_at,
                ],
            )
            .map_err(|_| WriterGenerationPortError::Unavailable)?;
        if affected != 1 {
            return Err(WriterGenerationPortError::Corrupt);
        }
        transaction
            .commit()
            .map_err(|_| WriterGenerationPortError::Unavailable)?;
        Ok(generation)
    }

    fn observe_current(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Option<WriterFence>, WriterGenerationPortError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| WriterGenerationPortError::Unavailable)?;
        let row = connection
            .query_row(
                "SELECT current_generation, current_lease_id, acquired_at_unix_micros
                 FROM writer_generation_state
                 WHERE workspace_id = ?1",
                params![workspace_id.as_bytes().as_slice()],
                |row| {
                    Ok((
                        row.get::<_, Value>(0)?,
                        row.get::<_, Value>(1)?,
                        row.get::<_, Value>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| WriterGenerationPortError::Unavailable)?;
        row.map(decode_temporal_row).transpose()
    }
}

fn prepare_private_database_file(path: &Path) -> Result<File, SqliteWriterGenerationOpenError> {
    let parent = path
        .parent()
        .filter(|candidate| !candidate.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent_metadata = fs::symlink_metadata(parent)
        .map_err(|_| SqliteWriterGenerationOpenError::UnsafeParent(parent.to_owned()))?;
    let owner = rustix::process::geteuid().as_raw();
    if !parent_metadata.is_dir()
        || parent_metadata.file_type().is_symlink()
        || parent_metadata.uid() != owner
        || parent_metadata.permissions().mode() & 0o777 != 0o700
    {
        return Err(SqliteWriterGenerationOpenError::UnsafeParent(
            parent.to_owned(),
        ));
    }
    let file_name = path
        .file_name()
        .ok_or_else(|| SqliteWriterGenerationOpenError::InvalidPath(path.to_owned()))?;
    let directory = open(
        parent,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY,
        Mode::empty(),
    )
    .map_err(|_| SqliteWriterGenerationOpenError::UnsafeParent(parent.to_owned()))?;
    let descriptor = openat(
        &directory,
        file_name,
        OFlags::RDWR | OFlags::CREATE | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|_| SqliteWriterGenerationOpenError::UnsafeDatabase(path.to_owned()))?;
    let file = File::from(descriptor);
    let metadata = file
        .metadata()
        .map_err(|source| SqliteWriterGenerationOpenError::Io {
            path: path.to_owned(),
            source,
        })?;
    if !private_database_metadata(&metadata, owner) {
        return Err(SqliteWriterGenerationOpenError::UnsafeDatabase(
            path.to_owned(),
        ));
    }
    file.sync_all()
        .map_err(|source| SqliteWriterGenerationOpenError::Io {
            path: path.to_owned(),
            source,
        })?;
    Ok(file)
}

fn validate_same_private_file(
    path: &Path,
    prepared: &fs::Metadata,
) -> Result<(), SqliteWriterGenerationOpenError> {
    let observed = fs::symlink_metadata(path)
        .map_err(|_| SqliteWriterGenerationOpenError::UnsafeDatabase(path.to_owned()))?;
    if !private_database_metadata(&observed, rustix::process::geteuid().as_raw())
        || observed.dev() != prepared.dev()
        || observed.ino() != prepared.ino()
    {
        return Err(SqliteWriterGenerationOpenError::UnsafeDatabase(
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

fn apply_pragmas(connection: &Connection) -> Result<(), SqliteWriterGenerationOpenError> {
    connection.busy_timeout(BUSY_TIMEOUT)?;
    let journal_mode: String =
        connection.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
    if journal_mode != "wal" {
        return Err(SqliteWriterGenerationOpenError::UnexpectedSchema(format!(
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
        return Err(SqliteWriterGenerationOpenError::UnexpectedSchema(format!(
            "synchronous is {synchronous}, expected FULL (2)"
        )));
    }
    Ok(())
}

fn initialize_or_validate_schema(
    connection: &mut Connection,
) -> Result<(), SqliteWriterGenerationOpenError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let observed_version: u32 =
        transaction.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let observed_application: u32 =
        transaction.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    match observed_version {
        0 => {
            if observed_application != 0 || !schema_objects(&transaction)?.is_empty() {
                return Err(SqliteWriterGenerationOpenError::UnexpectedSchema(
                    "unversioned database already contains application identity or objects".into(),
                ));
            }
            transaction.execute_batch(SCHEMA_V1)?;
        }
        WRITER_GENERATION_SCHEMA_VERSION => {
            if observed_application != APPLICATION_ID {
                return Err(SqliteWriterGenerationOpenError::UnexpectedSchema(format!(
                    "application_id is {observed_application}, expected {APPLICATION_ID}"
                )));
            }
        }
        observed => {
            return Err(SqliteWriterGenerationOpenError::UnsupportedSchema {
                observed,
                supported: WRITER_GENERATION_SCHEMA_VERSION,
            });
        }
    }
    validate_schema(&transaction)?;
    transaction.commit()?;
    Ok(())
}

fn validate_schema(connection: &Connection) -> Result<(), SqliteWriterGenerationOpenError> {
    let version: u32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let application: u32 = connection.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    if version != WRITER_GENERATION_SCHEMA_VERSION || application != APPLICATION_ID {
        return Err(SqliteWriterGenerationOpenError::UnexpectedSchema(
            "version or application identity changed during schema validation".into(),
        ));
    }
    let objects = schema_objects(connection)?;
    if objects != vec![("table".to_owned(), STATE_TABLE.to_owned())] {
        return Err(SqliteWriterGenerationOpenError::UnexpectedSchema(format!(
            "user object census is {objects:?}"
        )));
    }
    let mut statement = connection.prepare("PRAGMA table_info(writer_generation_state)")?;
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
        (0, "workspace_id".to_owned(), "BLOB".to_owned(), 1, 1),
        (1, "current_generation".to_owned(), "BLOB".to_owned(), 1, 0),
        (2, "current_lease_id".to_owned(), "BLOB".to_owned(), 1, 0),
        (
            3,
            "acquired_at_unix_micros".to_owned(),
            "INTEGER".to_owned(),
            1,
            0,
        ),
    ];
    if columns != expected {
        return Err(SqliteWriterGenerationOpenError::UnexpectedSchema(format!(
            "column census is {columns:?}"
        )));
    }
    let table_sql: String = connection.query_row(
        "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
        [STATE_TABLE],
        |row| row.get(0),
    )?;
    for required in [
        "length(workspace_id) = 16",
        "length(current_generation) = 8",
        "current_generation != X'0000000000000000'",
        "length(current_lease_id) = 16",
        "acquired_at_unix_micros >= 0",
        "WITHOUT ROWID",
        "STRICT",
    ] {
        if !table_sql.contains(required) {
            return Err(SqliteWriterGenerationOpenError::UnexpectedSchema(format!(
                "temporal table is missing required constraint {required}"
            )));
        }
    }
    Ok(())
}

fn schema_objects(
    connection: &Connection,
) -> Result<Vec<(String, String)>, SqliteWriterGenerationOpenError> {
    let mut statement = connection.prepare(
        "SELECT type, name
         FROM sqlite_schema
         WHERE name NOT LIKE 'sqlite_%'
         ORDER BY type, name",
    )?;
    Ok(statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?)
}

fn decode_temporal_row(
    (generation, lease, acquired_at): (Value, Value, Value),
) -> Result<WriterFence, WriterGenerationPortError> {
    let Value::Blob(generation) = generation else {
        return Err(WriterGenerationPortError::Corrupt);
    };
    let generation_bytes: [u8; 8] = generation
        .try_into()
        .map_err(|_| WriterGenerationPortError::Corrupt)?;
    let generation = WriterGeneration::new(u64::from_be_bytes(generation_bytes))
        .ok_or(WriterGenerationPortError::Corrupt)?;
    let Value::Blob(lease) = lease else {
        return Err(WriterGenerationPortError::Corrupt);
    };
    let lease: [u8; 16] = lease
        .try_into()
        .map_err(|_| WriterGenerationPortError::Corrupt)?;
    if !matches!(acquired_at, Value::Integer(value) if value >= 0) {
        return Err(WriterGenerationPortError::Corrupt);
    }
    Ok(WriterFence {
        lease_id: LeaseId::from_bytes(lease),
        generation,
    })
}

fn unix_micros() -> Result<i64, WriterGenerationPortError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| WriterGenerationPortError::Unavailable)?;
    i64::try_from(duration.as_micros()).map_err(|_| WriterGenerationPortError::Unavailable)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;

    use tempfile::TempDir;

    use super::*;

    fn private_root() -> TempDir {
        let root = TempDir::new().expect("temporary generation-store directory");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("make generation-store directory private");
        root
    }

    fn path(root: &TempDir) -> PathBuf {
        root.path().join("writer-generations.sqlite3")
    }

    fn workspace(seed: u8) -> WorkspaceId {
        WorkspaceId::from_bytes([seed; 16])
    }

    fn lease(seed: u8) -> LeaseId {
        LeaseId::from_bytes([seed; 16])
    }

    #[test]
    fn restart_preserves_monotonic_generation_and_private_file() {
        let root = private_root();
        let database = path(&root);
        let workspace = workspace(1);
        {
            let store = SqliteWriterGenerationStore::open(&database).expect("open first store");
            assert_eq!(store.allocate_next(workspace, lease(2)).unwrap().get(), 1);
            assert_eq!(store.database_path(), database);
        }
        assert_eq!(
            fs::symlink_metadata(&database)
                .expect("generation database metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        let reopened = SqliteWriterGenerationStore::open(&database).expect("reopen store");
        assert_eq!(
            reopened.observe_current(workspace).unwrap(),
            Some(WriterFence {
                lease_id: lease(2),
                generation: WriterGeneration::new(1).unwrap(),
            })
        );
        assert_eq!(
            reopened.allocate_next(workspace, lease(3)).unwrap().get(),
            2
        );
        assert_eq!(
            reopened.observe_current(workspace).unwrap(),
            Some(WriterFence {
                lease_id: lease(3),
                generation: WriterGeneration::new(2).unwrap(),
            })
        );
    }

    #[test]
    fn concurrent_connections_allocate_one_contiguous_generation_sequence() {
        const WRITERS: u8 = 12;
        let root = private_root();
        let database = path(&root);
        drop(SqliteWriterGenerationStore::open(&database).expect("initialize schema"));
        let database = Arc::new(database);
        let handles = (1..=WRITERS)
            .map(|seed| {
                let database = Arc::clone(&database);
                thread::spawn(move || {
                    let store = SqliteWriterGenerationStore::open(&database)
                        .expect("open concurrent store");
                    store
                        .allocate_next(workspace(4), lease(seed))
                        .expect("allocate concurrent generation")
                        .get()
                })
            })
            .collect::<Vec<_>>();
        let mut generations = handles
            .into_iter()
            .map(|handle| handle.join().expect("allocation thread does not panic"))
            .collect::<Vec<_>>();
        generations.sort_unstable();
        assert_eq!(generations, (1..=u64::from(WRITERS)).collect::<Vec<_>>());
        let store = SqliteWriterGenerationStore::open(&database).expect("open final store");
        assert_eq!(
            store
                .observe_current(workspace(4))
                .unwrap()
                .unwrap()
                .generation
                .get(),
            u64::from(WRITERS)
        );
    }

    #[test]
    fn corrupt_generation_blob_is_rejected_as_temporal_corruption() {
        let root = private_root();
        let database = path(&root);
        let store = SqliteWriterGenerationStore::open(&database).expect("open store");
        store.allocate_next(workspace(5), lease(6)).unwrap();
        drop(store);

        let connection = Connection::open(&database).expect("open corruption fixture");
        connection
            .execute_batch("PRAGMA ignore_check_constraints=ON;")
            .unwrap();
        connection
            .execute(
                "UPDATE writer_generation_state
                 SET current_generation = X'0000000000000000'
                 WHERE workspace_id = ?1",
                params![workspace(5).as_bytes().as_slice()],
            )
            .unwrap();
        drop(connection);

        let reopened = SqliteWriterGenerationStore::open(&database).expect("schema remains valid");
        assert_eq!(
            reopened.observe_current(workspace(5)),
            Err(WriterGenerationPortError::Corrupt)
        );
        assert_eq!(
            reopened.allocate_next(workspace(5), lease(7)),
            Err(WriterGenerationPortError::Corrupt)
        );
    }

    #[test]
    fn temporal_schema_census_has_no_semantic_current_authority() {
        let root = private_root();
        let database = path(&root);
        let store = SqliteWriterGenerationStore::open(&database).expect("open store");
        let connection = store
            .connection
            .lock()
            .expect("generation connection mutex is healthy");
        assert_eq!(
            schema_objects(&connection).unwrap(),
            vec![("table".to_owned(), STATE_TABLE.to_owned())]
        );
        let columns = connection
            .prepare("PRAGMA table_info(writer_generation_state)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            columns,
            vec![
                "workspace_id",
                "current_generation",
                "current_lease_id",
                "acquired_at_unix_micros",
            ]
        );
        assert!(columns.iter().all(|column| {
            !column.contains("epoch")
                && !column.contains("model")
                && !column.contains("source")
                && !column.contains("provider")
                && !column.contains("semantic")
        }));
        assert_eq!(
            connection
                .query_row::<u32, _, _>("PRAGMA user_version", [], |row| row.get(0))
                .unwrap(),
            WRITER_GENERATION_SCHEMA_VERSION
        );
    }

    #[test]
    fn generation_blob_preserves_full_u64_range_and_checks_overflow() {
        let root = private_root();
        let database = path(&root);
        let store = SqliteWriterGenerationStore::open(&database).expect("open store");
        store.allocate_next(workspace(8), lease(9)).unwrap();
        drop(store);

        let near_max = u64::MAX - 1;
        let connection = Connection::open(&database).expect("open range fixture");
        connection
            .execute(
                "UPDATE writer_generation_state
                 SET current_generation = ?2
                 WHERE workspace_id = ?1",
                params![
                    workspace(8).as_bytes().as_slice(),
                    near_max.to_be_bytes().as_slice(),
                ],
            )
            .unwrap();
        drop(connection);

        let store = SqliteWriterGenerationStore::open(&database).expect("reopen store");
        assert_eq!(
            store
                .observe_current(workspace(8))
                .unwrap()
                .unwrap()
                .generation
                .get(),
            near_max
        );
        assert_eq!(
            store.allocate_next(workspace(8), lease(10)).unwrap().get(),
            u64::MAX
        );
        assert_eq!(
            store.allocate_next(workspace(8), lease(11)),
            Err(WriterGenerationPortError::Exhausted)
        );
    }

    #[test]
    fn public_parent_is_rejected_before_database_creation() {
        let root = TempDir::new().expect("temporary public directory");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o755))
            .expect("make fixture public");
        let database = path(&root);
        assert!(matches!(
            SqliteWriterGenerationStore::open(&database),
            Err(SqliteWriterGenerationOpenError::UnsafeParent(_))
        ));
        assert!(!database.exists());
    }
}
