//! Catalog-generated `SQLite` operational state with one logical writer.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use rusqlite::backup::Backup;
use rusqlite::{Connection, OpenFlags, OptionalExtension as _, Transaction, TransactionBehavior};
use thiserror::Error;

use crate::contracts::index::model_artifact_index;
use crate::fabric::{MutationJournal, MutationPhaseSpec, PreparedMutation};

const SCHEMA_VERSION: u32 = 8;
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const OPERATIONAL_DDL: &str =
    include_str!("../contracts/generated/model/schema/operational-store.sql");
const SCHEMA_IR_ARTIFACT_ID: &str = "codefabric.schema.contract-ir";
static OPEN_WRITERS: OnceLock<Mutex<BTreeSet<PathBuf>>> = OnceLock::new();
type GeneratedColumnShapes = BTreeMap<String, Vec<(String, String, bool)>>;

/// Registered deterministic failure seams for migration and transaction recovery tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreFaultPoint {
    /// Abort after migration DDL but before the schema-version update and commit.
    MigrationBeforeCommit,
    /// Abort a normal write after the caller's statements but before commit.
    TransactionBeforeCommit,
}

impl StoreFaultPoint {
    /// Closed set used by the deterministic fault-matrix registry.
    pub const ALL: [Self; 2] = [Self::MigrationBeforeCommit, Self::TransactionBeforeCommit];
}

/// Exact read-back of the AC-G-27 `SQLite` settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PragmaState {
    pub journal_mode: String,
    pub synchronous: i64,
    pub foreign_keys: i64,
    pub trusted_schema: i64,
    pub secure_delete: i64,
    pub busy_timeout_ms: i64,
    pub wal_autocheckpoint_pages: i64,
}

/// Counts removed bounded-history rows without conflating durable fact retention.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RetentionReport {
    pub update_wave_items: usize,
    pub update_waves: usize,
    pub provider_runs: usize,
    pub git_operation_runs: usize,
    pub audit_events: usize,
}

/// One bounded provider-run lifecycle projection for the generated `provider_run` table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRunRecord {
    pub provider_run_id: Vec<u8>,
    pub workspace_id: Vec<u8>,
    pub analysis_context_id: Vec<u8>,
    pub wave_id: Vec<u8>,
    pub provider_code: i64,
    pub owner_id: Option<Vec<u8>>,
    pub build_unit_id: Option<Vec<u8>>,
    pub source_generation: i64,
    pub input_fingerprint: Vec<u8>,
    pub output_fingerprint: Option<Vec<u8>>,
    pub state_code: i64,
    pub accepted_at: String,
    pub terminal_at: Option<String>,
    pub diagnostic_id: Option<Vec<u8>>,
}

/// Stable operational-store failures.
#[derive(Debug, Error)]
pub enum OperationalStoreError {
    #[error("operational store I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("operational store SQLite failure: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("operational schema {found} is newer than supported schema {supported}")]
    NewerSchema { found: u32, supported: u32 },
    #[error("an operational writer is already open for {0}")]
    WriterAlreadyOpen(PathBuf),
    #[error("generated operational DDL lineage is invalid: {0}")]
    DdlLineage(String),
    #[error("injected operational-store fault at {0:?}")]
    InjectedFault(StoreFaultPoint),
    #[error("table mutation operation record conflict: {0}")]
    MutationRecord(String),
    #[error("provider run record conflict: {0}")]
    ProviderRunRecord(String),
}

#[derive(Debug)]
struct WriterRegistration {
    path: PathBuf,
}

impl WriterRegistration {
    fn acquire(path: &Path) -> Result<Self, OperationalStoreError> {
        let parent = path.parent().ok_or_else(|| OperationalStoreError::Io {
            path: path.to_owned(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "database path has no parent",
            ),
        })?;
        let parent = fs::canonicalize(parent).map_err(|source| OperationalStoreError::Io {
            path: parent.to_owned(),
            source,
        })?;
        let file_name = path.file_name().ok_or_else(|| OperationalStoreError::Io {
            path: path.to_owned(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "database path has no filename",
            ),
        })?;
        let normalized = parent.join(file_name);
        let mut writers = OPEN_WRITERS
            .get_or_init(|| Mutex::new(BTreeSet::new()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !writers.insert(normalized.clone()) {
            return Err(OperationalStoreError::WriterAlreadyOpen(normalized));
        }
        Ok(Self { path: normalized })
    }
}

impl Drop for WriterRegistration {
    fn drop(&mut self) {
        OPEN_WRITERS
            .get_or_init(|| Mutex::new(BTreeSet::new()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.path);
    }
}

/// The sole logical writer connection, owned by one coordinator thread.
#[derive(Debug)]
pub struct OperationalStore {
    connection: Connection,
    database_path: PathBuf,
    _registration: WriterRegistration,
}

/// Cloneable factory for independent transactionally consistent read connections.
#[derive(Clone, Debug)]
pub struct OperationalReaderFactory {
    database_path: PathBuf,
}

/// One read-only, query-only status connection.
pub struct OperationalReader {
    connection: Connection,
}

impl OperationalStore {
    /// Open, validate, back up, and migrate one operational database.
    ///
    /// # Errors
    ///
    /// Returns an I/O, schema-lineage, `SQLite`, writer-ownership, or migration error.
    pub fn open(path: &Path) -> Result<Self, OperationalStoreError> {
        Self::open_with_fault(path, None)
    }

    fn open_with_fault(
        path: &Path,
        fault: Option<StoreFaultPoint>,
    ) -> Result<Self, OperationalStoreError> {
        verify_ddl_lineage()?;
        prepare_private_database_file(path)?;
        let registration = WriterRegistration::acquire(path)?;
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        apply_writer_pragmas(&connection)?;
        let found = user_version(&connection)?;
        if found > SCHEMA_VERSION {
            return Err(OperationalStoreError::NewerSchema {
                found,
                supported: SCHEMA_VERSION,
            });
        }
        let mut store = Self {
            connection,
            database_path: registration.path.clone(),
            _registration: registration,
        };
        if found < SCHEMA_VERSION {
            store.migrate_from(found, fault)?;
        }
        store.assert_generated_table_census()?;
        Ok(store)
    }

    /// Construct the reusable read-connection factory for this store.
    #[must_use]
    pub fn reader_factory(&self) -> OperationalReaderFactory {
        OperationalReaderFactory {
            database_path: self.database_path.clone(),
        }
    }

    /// Read back the exact durability and safety pragmas.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` error if any pragma cannot be queried.
    pub fn pragma_state(&self) -> Result<PragmaState, OperationalStoreError> {
        pragma_state(&self.connection).map_err(Into::into)
    }

    /// Run the only supported write transaction: coordinator-thread `BEGIN IMMEDIATE`.
    ///
    /// # Errors
    ///
    /// Returns an ownership, `SQLite`, caller, or injected-fault error.
    pub fn write_transaction<T, E>(
        &mut self,
        operation: impl FnOnce(&Transaction<'_>) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<OperationalStoreError>,
    {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(OperationalStoreError::from)
            .map_err(E::from)?;
        let result = operation(&transaction)?;
        transaction
            .commit()
            .map_err(OperationalStoreError::from)
            .map_err(E::from)?;
        Ok(result)
    }

    #[cfg(test)]
    fn write_transaction_with_fault<T>(
        &mut self,
        operation: impl FnOnce(&Transaction<'_>) -> Result<T, OperationalStoreError>,
        fault: Option<StoreFaultPoint>,
    ) -> Result<T, OperationalStoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = operation(&transaction)?;
        if fault == Some(StoreFaultPoint::TransactionBeforeCommit) {
            return Err(OperationalStoreError::InjectedFault(
                StoreFaultPoint::TransactionBeforeCommit,
            ));
        }
        transaction.commit()?;
        Ok(result)
    }

    /// Delete only terminal, unprotected operational history before a timestamp.
    ///
    /// # Errors
    ///
    /// Returns an ownership or `SQLite` error.
    pub fn cleanup_terminal_before(
        &mut self,
        cutoff: &str,
    ) -> Result<RetentionReport, OperationalStoreError> {
        self.write_transaction(|transaction| {
            let update_wave_items = transaction.execute(
                "DELETE FROM update_wave_item WHERE wave_id IN (SELECT wave_id FROM update_wave WHERE terminal_at IS NOT NULL AND terminal_at < ?1)",
                [cutoff],
            )?;
            let update_waves = transaction.execute(
                "DELETE FROM update_wave WHERE terminal_at IS NOT NULL AND terminal_at < ?1",
                [cutoff],
            )?;
            let provider_runs = transaction.execute(
                "DELETE FROM provider_run WHERE terminal_at IS NOT NULL AND terminal_at < ?1",
                [cutoff],
            )?;
            let git_operation_runs = transaction.execute(
                "DELETE FROM git_operation_run WHERE terminal_at IS NOT NULL AND terminal_at < ?1",
                [cutoff],
            )?;
            let audit_events = transaction.execute(
                "DELETE FROM audit_event WHERE workspace_id IS NULL AND occurred_at < ?1",
                [cutoff],
            )?;
            Ok(RetentionReport {
                update_wave_items,
                update_waves,
                provider_runs,
                git_operation_runs,
                audit_events,
            })
        })
    }

    /// Insert or advance one provider run without allowing immutable identity drift.
    ///
    /// # Errors
    ///
    /// Returns a transaction error or `ProviderRunRecord` when an existing run ID is
    /// reused with different immutable inputs.
    pub fn record_provider_run(
        &mut self,
        record: &ProviderRunRecord,
    ) -> Result<(), OperationalStoreError> {
        self.write_transaction(|transaction| {
            let changed = transaction.execute(
                "INSERT INTO provider_run (
                   provider_run_id, workspace_id, analysis_context_id, wave_id,
                   provider_code, owner_id, build_unit_id, source_generation,
                   input_fingerprint, output_fingerprint, state_code, accepted_at,
                   terminal_at, diagnostic_id
                 ) VALUES (
                   ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14
                 )
                 ON CONFLICT(provider_run_id) DO UPDATE SET
                   output_fingerprint = excluded.output_fingerprint,
                   state_code = excluded.state_code,
                   terminal_at = excluded.terminal_at,
                   diagnostic_id = excluded.diagnostic_id
                 WHERE provider_run.workspace_id = excluded.workspace_id
                   AND provider_run.analysis_context_id = excluded.analysis_context_id
                   AND provider_run.wave_id = excluded.wave_id
                   AND provider_run.provider_code = excluded.provider_code
                   AND provider_run.source_generation = excluded.source_generation
                   AND provider_run.input_fingerprint = excluded.input_fingerprint
                   AND provider_run.accepted_at = excluded.accepted_at",
                rusqlite::params![
                    record.provider_run_id,
                    record.workspace_id,
                    record.analysis_context_id,
                    record.wave_id,
                    record.provider_code,
                    record.owner_id,
                    record.build_unit_id,
                    record.source_generation,
                    record.input_fingerprint,
                    record.output_fingerprint,
                    record.state_code,
                    record.accepted_at,
                    record.terminal_at,
                    record.diagnostic_id,
                ],
            )?;
            if changed != 1 {
                return Err(OperationalStoreError::ProviderRunRecord(
                    String::from_utf8_lossy(&record.provider_run_id).into_owned(),
                ));
            }
            Ok(())
        })
    }

    /// Checkpoint the WAL during drain without changing journal mode.
    ///
    /// # Errors
    ///
    /// Returns an ownership or `SQLite` error.
    pub fn checkpoint(&mut self) -> Result<(), OperationalStoreError> {
        self.connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }

    /// Copy the live WAL database through `SQLite`'s online backup API.
    ///
    /// # Errors
    ///
    /// Returns an I/O or `SQLite` backup error.
    pub fn backup_to(&self, destination: &Path) -> Result<(), OperationalStoreError> {
        create_private_empty_file(destination)?;
        let mut target = Connection::open(destination)?;
        let backup = Backup::new(&self.connection, &mut target)?;
        backup.run_to_completion(32, Duration::from_millis(1), None)?;
        drop(backup);
        target.close().map_err(|(_, error)| error)?;
        Ok(())
    }

    fn migrate_from(
        &mut self,
        version: u32,
        fault: Option<StoreFaultPoint>,
    ) -> Result<(), OperationalStoreError> {
        let backup_path = next_migration_backup_path(&self.database_path, SCHEMA_VERSION);
        self.backup_to(&backup_path)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        match version {
            0 => transaction.execute_batch(OPERATIONAL_DDL)?,
            1 => {
                migrate_v1_to_v2(&transaction)?;
                migrate_v2_to_v3(&transaction)?;
                migrate_v3_to_v4(&transaction)?;
                migrate_v4_to_v5(&transaction)?;
                migrate_v5_to_v6(&transaction)?;
                migrate_v6_to_v7(&transaction)?;
                migrate_v7_to_v8(&transaction)?;
            }
            2 => {
                migrate_v2_to_v3(&transaction)?;
                migrate_v3_to_v4(&transaction)?;
                migrate_v4_to_v5(&transaction)?;
                migrate_v5_to_v6(&transaction)?;
                migrate_v6_to_v7(&transaction)?;
                migrate_v7_to_v8(&transaction)?;
            }
            3 => {
                migrate_v3_to_v4(&transaction)?;
                migrate_v4_to_v5(&transaction)?;
                migrate_v5_to_v6(&transaction)?;
                migrate_v6_to_v7(&transaction)?;
                migrate_v7_to_v8(&transaction)?;
            }
            4 => {
                migrate_v4_to_v5(&transaction)?;
                migrate_v5_to_v6(&transaction)?;
                migrate_v6_to_v7(&transaction)?;
                migrate_v7_to_v8(&transaction)?;
            }
            5 => {
                migrate_v5_to_v6(&transaction)?;
                migrate_v6_to_v7(&transaction)?;
                migrate_v7_to_v8(&transaction)?;
            }
            6 => {
                migrate_v6_to_v7(&transaction)?;
                migrate_v7_to_v8(&transaction)?;
            }
            7 => migrate_v7_to_v8(&transaction)?,
            _ => {
                return Err(OperationalStoreError::DdlLineage(format!(
                    "no migration is registered from schema {version}"
                )));
            }
        }
        if fault == Some(StoreFaultPoint::MigrationBeforeCommit) {
            return Err(OperationalStoreError::InjectedFault(
                StoreFaultPoint::MigrationBeforeCommit,
            ));
        }
        transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        transaction.commit()?;
        Ok(())
    }

    fn assert_generated_table_census(&self) -> Result<(), OperationalStoreError> {
        let expected = generated_table_names();
        let mut statement = self.connection.prepare(
            "SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )?;
        let actual = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<BTreeSet<_>, _>>()?;
        if actual != expected {
            return Err(OperationalStoreError::DdlLineage(format!(
                "database table census differs: expected {expected:?}, found {actual:?}"
            )));
        }
        let expected_columns = generated_column_shapes()?;
        for (table, expected) in expected_columns {
            let mut statement = self.connection.prepare(
                "SELECT name, type, \"notnull\" FROM pragma_table_info(?1) ORDER BY cid",
            )?;
            let actual = statement
                .query_map([&table], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)? != 0,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            if actual != expected {
                return Err(OperationalStoreError::DdlLineage(format!(
                    "database column census for {table} differs: expected {expected:?}, found {actual:?}"
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
struct StoredMutation {
    application_id: String,
    application_version: i64,
    publication_id: Vec<u8>,
    owner_set_fingerprint: Vec<u8>,
    input_checksum: Vec<u8>,
    expected_output_checksum: Vec<u8>,
    expected_predecessor: Option<i64>,
    state_code: i64,
    delta_version: Option<i64>,
}

fn sqlite_version(version: Option<u64>) -> Result<Option<i64>, OperationalStoreError> {
    version
        .map(|value| {
            i64::try_from(value).map_err(|_| {
                OperationalStoreError::MutationRecord("Delta version exceeds i64".into())
            })
        })
        .transpose()
}

fn delta_version(version: Option<i64>) -> Result<Option<u64>, OperationalStoreError> {
    version
        .map(|value| {
            u64::try_from(value)
                .map_err(|_| OperationalStoreError::MutationRecord("negative Delta version".into()))
        })
        .transpose()
}

impl OperationalStore {
    fn prepare_mutation(
        &mut self,
        spec: &MutationPhaseSpec,
    ) -> Result<PreparedMutation, OperationalStoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let stored = transaction
            .query_row(
                "SELECT application_id, application_version, publication_id,
                        owner_set_fingerprint, input_checksum, expected_output_checksum,
                        expected_predecessor, state_code, delta_version
                   FROM table_mutation_operation
                  WHERE operation_id=?1 AND table_code=?2 AND mutation_phase=?3",
                rusqlite::params![spec.operation_id, spec.table_code, spec.phase.as_str()],
                |row| {
                    Ok(StoredMutation {
                        application_id: row.get(0)?,
                        application_version: row.get(1)?,
                        publication_id: row.get(2)?,
                        owner_set_fingerprint: row.get(3)?,
                        input_checksum: row.get(4)?,
                        expected_output_checksum: row.get(5)?,
                        expected_predecessor: row.get(6)?,
                        state_code: row.get(7)?,
                        delta_version: row.get(8)?,
                    })
                },
            )
            .optional()?;
        if let Some(stored) = stored {
            let exact = stored.application_id == spec.application_id
                && stored.publication_id.as_slice() == spec.publication_id
                && stored.owner_set_fingerprint.as_slice() == spec.owner_set_fingerprint
                && stored.input_checksum.as_slice() == spec.input_checksum
                && stored.expected_output_checksum.as_slice() == spec.expected_output_checksum
                && stored.expected_predecessor == sqlite_version(spec.expected_predecessor)?
                && matches!(stored.state_code, 10 | 20)
                && (stored.state_code == 20) == stored.delta_version.is_some();
            if !exact {
                return Err(OperationalStoreError::MutationRecord(
                    "operation identity was reused with different fields".into(),
                ));
            }
            transaction.commit()?;
            return Ok(PreparedMutation {
                spec: spec.clone(),
                application_version: stored.application_version,
                committed_delta_version: delta_version(stored.delta_version)?,
            });
        }
        let predecessor_claimed = transaction.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM table_mutation_operation
                  WHERE application_id=?1 AND expected_predecessor IS ?2
                    AND operation_id<>?3
             )",
            rusqlite::params![
                spec.application_id,
                sqlite_version(spec.expected_predecessor)?,
                spec.operation_id,
            ],
            |row| row.get::<_, bool>(0),
        )?;
        if predecessor_claimed {
            return Err(OperationalStoreError::MutationRecord(
                "application predecessor is already claimed by another operation".into(),
            ));
        }
        let prior: Option<i64> = transaction.query_row(
            "SELECT MAX(application_version) FROM table_mutation_operation WHERE application_id=?1",
            [&spec.application_id],
            |row| row.get(0),
        )?;
        let application_version = prior.unwrap_or(0).checked_add(1).ok_or_else(|| {
            OperationalStoreError::MutationRecord("application version exhausted".into())
        })?;
        transaction.execute(
            "INSERT INTO table_mutation_operation(
                 operation_id, table_code, mutation_phase, application_id,
                 application_version, publication_id, owner_set_fingerprint,
                 input_checksum, expected_output_checksum, expected_predecessor,
                 state_code, delta_version, created_at, completed_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,10,NULL,
                       strftime('%Y-%m-%dT%H:%M:%fZ','now'),NULL)",
            rusqlite::params![
                spec.operation_id,
                spec.table_code,
                spec.phase.as_str(),
                spec.application_id,
                application_version,
                spec.publication_id,
                spec.owner_set_fingerprint,
                spec.input_checksum,
                spec.expected_output_checksum,
                sqlite_version(spec.expected_predecessor)?,
            ],
        )?;
        transaction.commit()?;
        Ok(PreparedMutation {
            spec: spec.clone(),
            application_version,
            committed_delta_version: None,
        })
    }

    fn commit_mutation(
        &mut self,
        prepared: &PreparedMutation,
        version: u64,
    ) -> Result<(), OperationalStoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version = sqlite_version(Some(version))?.expect("Some remains Some");
        let changed = transaction.execute(
            "UPDATE table_mutation_operation
                SET state_code=20, delta_version=?1,
                    completed_at=COALESCE(completed_at,strftime('%Y-%m-%dT%H:%M:%fZ','now'))
              WHERE operation_id=?2 AND table_code=?3 AND mutation_phase=?4
                AND application_id=?5 AND application_version=?6
                AND (delta_version IS NULL OR delta_version=?1)",
            rusqlite::params![
                version,
                prepared.spec.operation_id,
                prepared.spec.table_code,
                prepared.spec.phase.as_str(),
                prepared.spec.application_id,
                prepared.application_version,
            ],
        )?;
        if changed != 1 {
            return Err(OperationalStoreError::MutationRecord(
                "prepared operation is absent or committed at another Delta version".into(),
            ));
        }
        transaction.commit()?;
        Ok(())
    }
}

impl MutationJournal for OperationalStore {
    fn prepare(&mut self, spec: &MutationPhaseSpec) -> Result<PreparedMutation, String> {
        self.prepare_mutation(spec)
            .map_err(|error| error.to_string())
    }

    fn mark_committed(
        &mut self,
        prepared: &PreparedMutation,
        delta_version: u64,
    ) -> Result<(), String> {
        self.commit_mutation(prepared, delta_version)
            .map_err(|error| error.to_string())
    }
}

fn migrate_v1_to_v2(transaction: &Transaction<'_>) -> Result<(), OperationalStoreError> {
    transaction
        .execute_batch("ALTER TABLE workspace_registration RENAME TO workspace_registration_v1;")?;
    let workspace_v2 = generated_table_ddl("workspace_registration")?.replacen(
        "CREATE TABLE workspace_registration (",
        "CREATE TABLE workspace_registration_v2 (",
        1,
    );
    transaction.execute_batch(&workspace_v2)?;
    transaction.execute_batch(
        "INSERT INTO workspace_registration_v2 (
           workspace_id, workspace_registration_nonce, registration_revision,
           administrative_key, root_path_bytes, root_path_display,
           root_directory_file_identity, platform_code, case_sensitivity_mode,
           authorization_revision, allowed_source_disclosure_rules,
           repository_id, worktree_id, authorization_fingerprint,
           context_fingerprint, status_code, created_at, updated_at
         )
         SELECT workspace_id, workspace_registration_nonce, registration_revision,
           administrative_key, root_path_bytes, root_path_display,
           X'', 0, 'unknown', 0, X'5b5d',
           repository_id, worktree_id, authorization_fingerprint,
           context_fingerprint, 100, created_at, updated_at
         FROM workspace_registration_v1;
         DROP TABLE workspace_registration_v1;
         ALTER TABLE workspace_registration_v2 RENAME TO workspace_registration;",
    )?;
    transaction.execute_batch(&generated_table_ddl("repository_registration")?)?;
    transaction.execute_batch(&generated_table_ddl("worktree_registration")?)?;
    Ok(())
}

fn migrate_v2_to_v3(transaction: &Transaction<'_>) -> Result<(), OperationalStoreError> {
    for table in [
        "source_inventory",
        "source_blob",
        "source_blob_lease",
        "source_blob_lease_member",
    ] {
        transaction.execute_batch(&generated_table_ddl(table)?)?;
    }
    Ok(())
}

fn migrate_v3_to_v4(transaction: &Transaction<'_>) -> Result<(), OperationalStoreError> {
    if !table_has_column(transaction, "worktree_state", "inventory_digest")? {
        transaction
            .execute_batch("ALTER TABLE worktree_state ADD COLUMN inventory_digest BLOB;")?;
    }
    // Historical migration fixtures can contain the current generated table while
    // exercising an older version of another table. Preserve an already-v4 Git vector.
    if table_has_column(transaction, "git_state_vector", "head_target")? {
        return Ok(());
    }
    transaction.execute_batch("ALTER TABLE git_state_vector RENAME TO git_state_vector_v3;")?;
    transaction.execute_batch(&generated_table_ddl("git_state_vector")?)?;
    transaction.execute_batch(
        "INSERT INTO git_state_vector (
           workspace_id, source_generation, repository_id, worktree_id,
           head_kind_code, head_target, head_tree, index_fingerprint,
           index_entry_count, has_conflict_stages, repository_state_code,
           inclusion_policy_fingerprint, attributes_fingerprint,
           worktree_inventory_digest, captured_at
         )
         SELECT old.workspace_id, old.source_generation,
           state.repository_id, state.worktree_id,
           CASE WHEN old.head_oid IS NULL THEN 30 ELSE 10 END,
           CASE length(old.head_oid)
             WHEN 20 THEN CAST(X'01' || old.head_oid AS BLOB)
             WHEN 32 THEN CAST(X'02' || old.head_oid AS BLOB)
             ELSE NULL
           END,
           CASE length(old.head_tree_oid)
             WHEN 20 THEN CAST(X'01' || old.head_tree_oid AS BLOB)
             WHEN 32 THEN CAST(X'02' || old.head_tree_oid AS BLOB)
             ELSE NULL
           END,
           old.index_fingerprint, NULL, 0, 10,
           old.inclusion_fingerprint, zeroblob(32), old.worktree_fingerprint,
           old.captured_at
         FROM git_state_vector_v3 AS old
         JOIN worktree_state AS state USING (workspace_id)
         WHERE state.repository_id IS NOT NULL AND state.worktree_id IS NOT NULL;
         DROP TABLE git_state_vector_v3;",
    )?;
    Ok(())
}

fn migrate_v4_to_v5(transaction: &Transaction<'_>) -> Result<(), OperationalStoreError> {
    transaction.execute_batch(&generated_table_ddl("table_mutation_operation")?)?;
    Ok(())
}

fn migrate_v5_to_v6(transaction: &Transaction<'_>) -> Result<(), OperationalStoreError> {
    transaction.execute_batch(
        "DROP TABLE snapshot_lease;
         DROP TABLE result_artifact_lease;
         DROP TABLE serving_snapshot_manifest;
         DROP TABLE active_snapshot;",
    )?;
    for table in [
        "snapshot_lease",
        "result_artifact_lease",
        "serving_snapshot_manifest",
        "active_snapshot",
    ] {
        transaction.execute_batch(&generated_table_ddl(table)?)?;
    }
    Ok(())
}

fn migrate_v6_to_v7(transaction: &Transaction<'_>) -> Result<(), OperationalStoreError> {
    transaction.execute_batch(&generated_table_ddl("operational_dependency_edge")?)?;
    Ok(())
}

fn migrate_v7_to_v8(transaction: &Transaction<'_>) -> Result<(), OperationalStoreError> {
    transaction.execute_batch(&generated_table_ddl("git_candidate_cache")?)?;
    Ok(())
}

fn table_has_column(
    transaction: &Transaction<'_>,
    table: &str,
    column: &str,
) -> Result<bool, OperationalStoreError> {
    let mut statement = transaction.prepare("SELECT name FROM pragma_table_info(?1)")?;
    let mut rows = statement.query([table])?;
    while let Some(row) = rows.next()? {
        if row.get::<_, String>(0)? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

impl OperationalReaderFactory {
    /// Open a separate read-only, query-only connection.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` error if the operational store cannot be opened read-only.
    pub fn open(&self) -> Result<OperationalReader, OperationalStoreError> {
        let connection = Connection::open_with_flags(
            &self.database_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        connection.busy_timeout(BUSY_TIMEOUT)?;
        connection.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF; PRAGMA query_only=ON;",
        )?;
        Ok(OperationalReader { connection })
    }
}

impl OperationalReader {
    /// Borrow the read-only connection for one bounded query operation.
    ///
    /// # Errors
    ///
    /// Returns the caller's `SQLite` error.
    pub fn with_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T> {
        operation(&self.connection)
    }

    /// Execute one read-only callback whose domain error carries more context than `SQLite`.
    #[cfg(feature = "daemon")]
    pub(crate) fn with_connection_result<T, E>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T, E>,
    ) -> Result<T, E> {
        operation(&self.connection)
    }
}

/// Digest of the exact generated DDL bytes compiled into the daemon.
#[must_use]
pub fn operational_ddl_digest() -> String {
    format!("b3:{}", blake3::hash(OPERATIONAL_DDL.as_bytes()).to_hex())
}

fn verify_ddl_lineage() -> Result<(), OperationalStoreError> {
    let index = model_artifact_index().map_err(|error| {
        OperationalStoreError::DdlLineage(format!("artifact index unavailable: {error}"))
    })?;
    let expected = index
        .artifacts
        .iter()
        .find(|artifact| artifact.artifact_id == SCHEMA_IR_ARTIFACT_ID)
        .ok_or_else(|| OperationalStoreError::DdlLineage("schema IR is absent".into()))?
        .canonical_digest
        .as_str();
    let first_line = OPERATIONAL_DDL.lines().next().unwrap_or_default();
    if !first_line.contains(expected) || !first_line.contains("@generated") {
        return Err(OperationalStoreError::DdlLineage(
            "DDL header does not bind the packaged schema-IR identity".into(),
        ));
    }
    Ok(())
}

fn prepare_private_database_file(path: &Path) -> Result<(), OperationalStoreError> {
    if path.exists() {
        let metadata = fs::symlink_metadata(path).map_err(|source| OperationalStoreError::Io {
            path: path.to_owned(),
            source,
        })?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(OperationalStoreError::Io {
                path: path.to_owned(),
                source: std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "database must be a private non-symlink file",
                ),
            });
        }
        return Ok(());
    }
    create_private_empty_file(path)
}

fn create_private_empty_file(path: &Path) -> Result<(), OperationalStoreError> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|source| OperationalStoreError::Io {
            path: path.to_owned(),
            source,
        })?;
    file.sync_all()
        .map_err(|source| OperationalStoreError::Io {
            path: path.to_owned(),
            source,
        })?;
    Ok(())
}

fn next_migration_backup_path(database_path: &Path, target: u32) -> PathBuf {
    let base = format!(
        "{}.pre-migration-v{target}.backup.sqlite3",
        database_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    );
    let parent = database_path.parent().unwrap_or_else(|| Path::new("."));
    let first = parent.join(&base);
    if !first.exists() {
        return first;
    }
    for sequence in 1_u32.. {
        let candidate = parent.join(format!("{base}.{sequence}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("u32 backup sequence is practically unbounded")
}

fn apply_writer_pragmas(connection: &Connection) -> rusqlite::Result<()> {
    connection.busy_timeout(BUSY_TIMEOUT)?;
    let mode: String = connection.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
    if mode != "wal" {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "SQLite selected journal_mode={mode}"
        )));
    }
    connection.execute_batch(
        "PRAGMA synchronous=FULL;
         PRAGMA foreign_keys=ON;
         PRAGMA trusted_schema=OFF;
         PRAGMA secure_delete=FAST;
         PRAGMA wal_autocheckpoint=1000;",
    )
}

fn pragma_state(connection: &Connection) -> rusqlite::Result<PragmaState> {
    Ok(PragmaState {
        journal_mode: connection.query_row("PRAGMA journal_mode", [], |row| row.get(0))?,
        synchronous: connection.query_row("PRAGMA synchronous", [], |row| row.get(0))?,
        foreign_keys: connection.query_row("PRAGMA foreign_keys", [], |row| row.get(0))?,
        trusted_schema: connection.query_row("PRAGMA trusted_schema", [], |row| row.get(0))?,
        secure_delete: connection.query_row("PRAGMA secure_delete", [], |row| row.get(0))?,
        busy_timeout_ms: connection.query_row("PRAGMA busy_timeout", [], |row| row.get(0))?,
        wal_autocheckpoint_pages: connection
            .query_row("PRAGMA wal_autocheckpoint", [], |row| row.get(0))?,
    })
}

fn user_version(connection: &Connection) -> rusqlite::Result<u32> {
    connection.query_row("PRAGMA user_version", [], |row| row.get(0))
}

fn generated_table_names() -> BTreeSet<String> {
    OPERATIONAL_DDL
        .lines()
        .filter_map(|line| line.strip_prefix("CREATE TABLE "))
        .filter_map(|line| line.strip_suffix(" ("))
        .map(str::to_owned)
        .collect()
}

fn generated_table_ddl(table: &str) -> Result<String, OperationalStoreError> {
    let start = format!("CREATE TABLE {table} (");
    let mut collecting = false;
    let mut lines = Vec::new();
    for line in OPERATIONAL_DDL.lines() {
        if line == start {
            collecting = true;
        }
        if collecting {
            lines.push(line);
            if line == ") STRICT;" {
                return Ok(format!("{}\n", lines.join("\n")));
            }
        }
    }
    Err(OperationalStoreError::DdlLineage(format!(
        "generated DDL has no table {table}"
    )))
}

fn generated_column_shapes() -> Result<GeneratedColumnShapes, OperationalStoreError> {
    let mut result = BTreeMap::new();
    for table in generated_table_names() {
        let ddl = generated_table_ddl(&table)?;
        let columns = ddl
            .lines()
            .skip(1)
            .take_while(|line| *line != ") STRICT;")
            .filter_map(|line| {
                let declaration = line.trim().trim_end_matches(',');
                if declaration.starts_with("PRIMARY KEY") || declaration.starts_with("UNIQUE") {
                    return None;
                }
                let mut parts = declaration.split_whitespace();
                Some((
                    parts.next()?.to_owned(),
                    parts.next()?.to_owned(),
                    declaration.ends_with("NOT NULL"),
                ))
            })
            .collect::<Vec<_>>();
        if columns.is_empty() {
            return Err(OperationalStoreError::DdlLineage(format!(
                "generated DDL table {table} has no columns"
            )));
        }
        result.insert(table, columns);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn database() -> (TempDir, PathBuf) {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("operational.sqlite3");
        (directory, path)
    }

    #[test]
    fn wp13_behavioral_acceptance() {
        let (_directory, path) = database();
        let mut store = OperationalStore::open(&path).unwrap();
        assert_eq!(user_version(&store.connection).unwrap(), SCHEMA_VERSION);
        let reader = store.reader_factory().open().unwrap();
        reader
            .with_connection(|connection| {
                connection.query_row("SELECT COUNT(*) FROM workspace_registration", [], |row| {
                    row.get::<_, i64>(0)
                })
            })
            .unwrap();

        store
            .write_transaction_with_fault(
                |transaction| {
                    transaction.execute(
                        "INSERT INTO audit_event(event_id, workspace_id, event_code, actor_id, occurred_at, details_digest, diagnostic_id) VALUES (?1, NULL, 1, 'test', '2026-01-01T00:00:00Z', ?2, NULL)",
                        rusqlite::params![vec![1_u8; 16], vec![2_u8; 32]],
                    )?;
                    Ok(())
                },
                Some(StoreFaultPoint::TransactionBeforeCommit),
            )
            .unwrap_err();
        let count = reader
            .with_connection(|connection| {
                connection.query_row("SELECT COUNT(*) FROM audit_event", [], |row| {
                    row.get::<_, i64>(0)
                })
            })
            .unwrap();
        assert_eq!(count, 0);

        store
            .write_transaction(|transaction| {
                transaction.execute(
                    "INSERT INTO audit_event(event_id, workspace_id, event_code, actor_id, occurred_at, details_digest, diagnostic_id) VALUES (?1, NULL, 1, 'test', '2026-01-01T00:00:00Z', ?2, NULL)",
                    rusqlite::params![vec![3_u8; 16], vec![4_u8; 32]],
                )?;
                Ok::<(), OperationalStoreError>(())
            })
            .unwrap();
        reader
            .with_connection(|connection| {
                connection.execute_batch("BEGIN;")?;
                assert_eq!(
                    connection.query_row("SELECT COUNT(*) FROM audit_event", [], |row| {
                        row.get::<_, i64>(0)
                    })?,
                    1
                );
                Ok(())
            })
            .unwrap();

        let backup = path.with_file_name("live.backup.sqlite3");
        store.backup_to(&backup).unwrap();
        let restored =
            Connection::open_with_flags(&backup, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        assert_eq!(user_version(&restored).unwrap(), SCHEMA_VERSION);
        assert_eq!(
            restored
                .query_row("SELECT COUNT(*) FROM audit_event", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        drop(restored);
        drop(reader);
        drop(store);

        let newer = Connection::open(&path).unwrap();
        newer
            .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        drop(newer);
        assert!(matches!(
            OperationalStore::open(&path).unwrap_err(),
            OperationalStoreError::NewerSchema { .. }
        ));
    }

    #[test]
    fn wp13_structural_acceptance() {
        let (_directory, path) = database();
        let store = OperationalStore::open(&path).unwrap();
        assert_eq!(
            store.pragma_state().unwrap(),
            PragmaState {
                journal_mode: "wal".into(),
                synchronous: 2,
                foreign_keys: 1,
                trusted_schema: 0,
                secure_delete: 2,
                busy_timeout_ms: 5_000,
                wal_autocheckpoint_pages: 1_000,
            }
        );
        let digest = operational_ddl_digest();
        assert!(digest.starts_with("b3:") && digest.len() == 67);
        assert_eq!(
            generated_table_names().len(),
            OPERATIONAL_DDL
                .lines()
                .filter(|line| line.starts_with("CREATE TABLE "))
                .count()
        );
        assert!(verify_ddl_lineage().is_ok());
    }

    #[test]
    fn wp13_negative_zero_state() {
        let (_directory, path) = database();
        let _store = OperationalStore::open(&path).unwrap();
        assert!(matches!(
            OperationalStore::open(&path).unwrap_err(),
            OperationalStoreError::WriterAlreadyOpen(_)
        ));
        let tables = generated_table_names();
        for prohibited in [
            "source_bytes",
            "arrow_rows",
            "query_result_bytes",
            "parser_nodes",
            "progress_events",
        ] {
            assert!(!tables.contains(prohibited));
            assert!(!OPERATIONAL_DDL.contains(&format!("CREATE TABLE {prohibited}")));
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One oracle covers the ordered migration/fault/retention proof.
    fn wp13_operational_acceptance() {
        assert_eq!(StoreFaultPoint::ALL.len(), 2);
        let migration_backup_marker = format!("pre-migration-v{SCHEMA_VERSION}");
        let (_directory, path) = database();
        assert!(matches!(
            OperationalStore::open_with_fault(&path, Some(StoreFaultPoint::MigrationBeforeCommit))
                .unwrap_err(),
            OperationalStoreError::InjectedFault(StoreFaultPoint::MigrationBeforeCommit)
        ));
        let raw = Connection::open(&path).unwrap();
        assert_eq!(user_version(&raw).unwrap(), 0);
        drop(raw);
        let first_migration_backup = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains(&migration_backup_marker)
            })
            .unwrap()
            .path();
        let restored = Connection::open(&first_migration_backup).unwrap();
        assert_eq!(user_version(&restored).unwrap(), 0);
        assert_eq!(
            restored
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
        drop(restored);
        assert_eq!(
            fs::read_dir(path.parent().unwrap())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .contains(&migration_backup_marker))
                .count(),
            1
        );
        let mut store = OperationalStore::open(&path).unwrap();
        assert_eq!(user_version(&store.connection).unwrap(), SCHEMA_VERSION);
        assert_eq!(
            fs::read_dir(path.parent().unwrap())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .contains(&migration_backup_marker))
                .count(),
            2
        );

        store
            .write_transaction(|transaction| {
                transaction.execute(
                    "INSERT INTO workspace_registration(workspace_id, workspace_registration_nonce, registration_revision, administrative_key, root_path_bytes, root_path_display, root_directory_file_identity, platform_code, case_sensitivity_mode, authorization_revision, allowed_source_disclosure_rules, repository_id, worktree_id, authorization_fingerprint, context_fingerprint, status_code, created_at, updated_at) VALUES (?1, ?2, 1, ?3, ?4, '/workspace', ?5, 1, 'sensitive', 1, ?6, NULL, NULL, ?7, ?8, 1, '2026-01-01', '2026-01-01')",
                    rusqlite::params![vec![7_u8; 16], vec![6_u8; 16], vec![5_u8; 16], b"/workspace", vec![9_u8; 16], br#"["metadata"]"#, vec![4_u8; 32], vec![3_u8; 32]],
                )?;
                transaction.execute(
                    "INSERT INTO snapshot_lease(
                       lease_id, lease_kind_code, workspace_id, snapshot_id,
                       base_publication_id, required_delta_versions_bytes,
                       requires_overlay, agent_instance_id, created_at,
                       last_heartbeat_at, expires_at, state_code,
                       process_instance_id, orphaned_at, artifact_expires_at,
                       source_blob_lease_id
                     ) VALUES (?1, 10, ?2, ?3, ?4, X'7b7d', 0, NULL,
                               1, 1, 300, 10, ?5, NULL, NULL, NULL)",
                    rusqlite::params![
                        vec![1_u8; 16],
                        vec![7_u8; 16],
                        vec![2_u8; 16],
                        vec![3_u8; 16],
                        vec![4_u8; 16]
                    ],
                )?;
                for (id, terminal_at) in [(1_u8, Some("2026-01-01")), (2, None)] {
                    transaction.execute(
                        "INSERT INTO git_operation_run(git_operation_run_id, workspace_id, baseline_fingerprint, result_fingerprint, candidate_count, verified_count, state_code, started_at, terminal_at, diagnostic_id) VALUES (?1, ?2, ?3, NULL, 0, 0, 1, '2026-01-01', ?4, NULL)",
                        rusqlite::params![vec![id; 16], vec![7_u8; 16], vec![8_u8; 32], terminal_at],
                    )?;
                }
                Ok::<(), OperationalStoreError>(())
            })
            .unwrap();
        let report = store.cleanup_terminal_before("2026-02-01").unwrap();
        assert_eq!(report.git_operation_runs, 1);
        let remaining = store
            .reader_factory()
            .open()
            .unwrap()
            .with_connection(|connection| {
                connection.query_row("SELECT COUNT(*) FROM git_operation_run", [], |row| {
                    row.get::<_, i64>(0)
                })
            })
            .unwrap();
        assert_eq!(remaining, 1);
        let protected = store
            .reader_factory()
            .open()
            .unwrap()
            .with_connection(|connection| {
                Ok((
                    connection.query_row(
                        "SELECT COUNT(*) FROM workspace_registration",
                        [],
                        |row| row.get::<_, i64>(0),
                    )?,
                    connection.query_row("SELECT COUNT(*) FROM snapshot_lease", [], |row| {
                        row.get::<_, i64>(0)
                    })?,
                ))
            })
            .unwrap();
        assert_eq!(protected, (1, 1));
        store.checkpoint().unwrap();
    }

    #[test]
    fn wp14_operational_schema_v1_migrates_to_current() {
        let (_directory, path) = database();
        let store = OperationalStore::open(&path).unwrap();
        drop(store);

        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "INSERT INTO workspace_registration (
                   workspace_id, workspace_registration_nonce, registration_revision,
                   administrative_key, root_path_bytes, root_path_display,
                   root_directory_file_identity, platform_code, case_sensitivity_mode,
                   authorization_revision, allowed_source_disclosure_rules,
                   repository_id, worktree_id, authorization_fingerprint,
                   context_fingerprint, status_code, created_at, updated_at
                 ) VALUES (
                   zeroblob(16), zeroblob(16), 1, X'aa', X'2f', '/',
                   X'bb', 2, 'sensitive', 1, X'5b5d', NULL, NULL,
                   zeroblob(32), zeroblob(32), 20, 'before', 'before'
                 );
                 ALTER TABLE workspace_registration RENAME TO workspace_registration_v2;
                 CREATE TABLE workspace_registration (
                   workspace_id BLOB NOT NULL,
                   workspace_registration_nonce BLOB NOT NULL,
                   registration_revision INTEGER NOT NULL,
                   administrative_key BLOB NOT NULL,
                   root_path_bytes BLOB NOT NULL,
                   root_path_display TEXT NOT NULL,
                   repository_id BLOB,
                   worktree_id BLOB,
                   authorization_fingerprint BLOB NOT NULL,
                   context_fingerprint BLOB NOT NULL,
                   status_code INTEGER NOT NULL,
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL,
                   PRIMARY KEY (workspace_id),
                   UNIQUE (administrative_key)
                 ) STRICT;
                 INSERT INTO workspace_registration
                 SELECT workspace_id, workspace_registration_nonce,
                   registration_revision, administrative_key, root_path_bytes,
                   root_path_display, repository_id, worktree_id,
                   authorization_fingerprint, context_fingerprint, status_code,
                   created_at, updated_at
                 FROM workspace_registration_v2;
                 DROP TABLE workspace_registration_v2;
                 DROP TABLE repository_registration;
                 DROP TABLE worktree_registration;
                 DROP TABLE source_inventory;
                 DROP TABLE source_blob;
                 DROP TABLE source_blob_lease;
                 DROP TABLE source_blob_lease_member;
                 DROP TABLE table_mutation_operation;
                 DROP TABLE operational_dependency_edge;
                 DROP TABLE git_candidate_cache;
                 PRAGMA user_version=1;",
            )
            .unwrap();
        drop(connection);

        let migrated = OperationalStore::open(&path).unwrap();
        assert_eq!(user_version(&migrated.connection).unwrap(), SCHEMA_VERSION);
        let migrated_fields = migrated
            .connection
            .query_row(
                "SELECT root_directory_file_identity, platform_code,
                        case_sensitivity_mode, authorization_revision,
                        allowed_source_disclosure_rules, status_code
                 FROM workspace_registration",
                [],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Vec<u8>>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            migrated_fields,
            (Vec::new(), 0, "unknown".to_owned(), 0, b"[]".to_vec(), 100)
        );
        let legacy_unique_indexes = migrated
            .connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_index_list('workspace_registration') WHERE origin='u'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(legacy_unique_indexes, 0);
    }
}
