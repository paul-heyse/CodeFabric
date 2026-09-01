//! Daemon-wide bounded query admission, idempotency, events, cancellation, and restart state.
//!
//! The coordinator is the sole mutable authority for accepted semantic operations. It reserves
//! queue/task/journal/result capacity before acceptance, binds idempotency to the full normalized
//! operation, coalesces progress before allocating event sequence, and persists only control
//! records. Arrow response bytes live exclusively in manifest-last result packages.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{Mutex, Notify};

use super::command::{PrincipalId, WorkspaceId};

/// Every field that can change execution, delivery, freshness, or retention meaning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedQueryOperation {
    pub workspace_id: WorkspaceId,
    pub principal_id: PrincipalId,
    pub idempotency_key: Arc<str>,
    pub canonical_request: Arc<[u8]>,
    pub semantic_profile: Arc<str>,
    pub request_contract: Arc<str>,
    pub response_contract: Arc<str>,
    pub delivery_profile: Arc<str>,
    pub compression_profile: Arc<str>,
    pub freshness_policy: Arc<str>,
    pub epoch_policy: Arc<str>,
    pub deadline_unix_ms: i64,
    pub lease_expires_at_unix_ms: i64,
    pub maximum_result_bytes: u64,
    pub maximum_result_pages: u64,
}

impl NormalizedQueryOperation {
    /// Strictly validate and canonicalize the complete meaning-bearing operation.
    pub fn try_new(mut operation: Self) -> Result<Self, QueryCoordinatorError> {
        if operation
            .workspace_id
            .as_bytes()
            .iter()
            .all(|byte| *byte == 0)
            || operation
                .principal_id
                .as_bytes()
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(QueryCoordinatorError::InvalidIdentity);
        }
        if operation.idempotency_key.is_empty() || operation.idempotency_key.len() > 256 {
            return Err(QueryCoordinatorError::InvalidIdempotencyKey);
        }
        for (name, value) in [
            ("semantic_profile", &operation.semantic_profile),
            ("request_contract", &operation.request_contract),
            ("response_contract", &operation.response_contract),
            ("delivery_profile", &operation.delivery_profile),
            ("compression_profile", &operation.compression_profile),
            ("freshness_policy", &operation.freshness_policy),
            ("epoch_policy", &operation.epoch_policy),
        ] {
            if value.is_empty() || value.len() > 256 || !value.is_ascii() {
                return Err(QueryCoordinatorError::InvalidOperationField(name));
            }
        }
        if operation.deadline_unix_ms <= 0
            || operation.lease_expires_at_unix_ms <= operation.deadline_unix_ms
            || operation.maximum_result_bytes == 0
            || operation.maximum_result_pages == 0
        {
            return Err(QueryCoordinatorError::InvalidOperationBounds);
        }
        let decoded: serde_json::Value = serde_json::from_slice(&operation.canonical_request)
            .map_err(QueryCoordinatorError::CanonicalRequest)?;
        let canonical = serde_json_canonicalizer::to_vec(&decoded)
            .map_err(QueryCoordinatorError::CanonicalRequest)?;
        if canonical.as_slice() != operation.canonical_request.as_ref() {
            return Err(QueryCoordinatorError::NonCanonicalRequest);
        }
        operation.canonical_request = Arc::from(canonical);
        Ok(operation)
    }

    #[must_use]
    pub fn fingerprint(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        frame(&mut hasher, b"codefabric.normalized-query-operation.v1");
        frame(&mut hasher, self.workspace_id.as_bytes());
        frame(&mut hasher, self.principal_id.as_bytes());
        frame(&mut hasher, self.idempotency_key.as_bytes());
        frame(&mut hasher, &self.canonical_request);
        for value in [
            &self.semantic_profile,
            &self.request_contract,
            &self.response_contract,
            &self.delivery_profile,
            &self.compression_profile,
            &self.freshness_policy,
            &self.epoch_policy,
        ] {
            frame(&mut hasher, value.as_bytes());
        }
        frame(&mut hasher, &self.deadline_unix_ms.to_be_bytes());
        frame(&mut hasher, &self.lease_expires_at_unix_ms.to_be_bytes());
        frame(&mut hasher, &self.maximum_result_bytes.to_be_bytes());
        frame(&mut hasher, &self.maximum_result_pages.to_be_bytes());
        *hasher.finalize().as_bytes()
    }
}

/// Aggregate limits; every accepted query reserves its declared maximum result and journal tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueryCoordinatorPolicy {
    max_running: NonZeroUsize,
    max_running_per_workspace: NonZeroUsize,
    max_running_per_principal: NonZeroUsize,
    max_queued: NonZeroUsize,
    max_tasks: NonZeroUsize,
    max_events_per_query: NonZeroUsize,
    max_event_bytes_per_query: NonZeroUsize,
    max_total_result_bytes: NonZeroU64,
    max_total_result_pages: NonZeroU64,
    max_recovery_records: NonZeroUsize,
}

impl QueryCoordinatorPolicy {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        max_running: usize,
        max_running_per_workspace: usize,
        max_running_per_principal: usize,
        max_queued: usize,
        max_tasks: usize,
        max_events_per_query: usize,
        max_event_bytes_per_query: usize,
        max_total_result_bytes: u64,
        max_total_result_pages: u64,
        max_recovery_records: usize,
    ) -> Result<Self, QueryCoordinatorError> {
        let policy = Self {
            max_running: NonZeroUsize::new(max_running)
                .ok_or(QueryCoordinatorError::InvalidPolicy("max_running"))?,
            max_running_per_workspace: NonZeroUsize::new(max_running_per_workspace).ok_or(
                QueryCoordinatorError::InvalidPolicy("max_running_per_workspace"),
            )?,
            max_running_per_principal: NonZeroUsize::new(max_running_per_principal).ok_or(
                QueryCoordinatorError::InvalidPolicy("max_running_per_principal"),
            )?,
            max_queued: NonZeroUsize::new(max_queued)
                .ok_or(QueryCoordinatorError::InvalidPolicy("max_queued"))?,
            max_tasks: NonZeroUsize::new(max_tasks)
                .ok_or(QueryCoordinatorError::InvalidPolicy("max_tasks"))?,
            max_events_per_query: NonZeroUsize::new(max_events_per_query)
                .ok_or(QueryCoordinatorError::InvalidPolicy("max_events_per_query"))?,
            max_event_bytes_per_query: NonZeroUsize::new(max_event_bytes_per_query).ok_or(
                QueryCoordinatorError::InvalidPolicy("max_event_bytes_per_query"),
            )?,
            max_total_result_bytes: NonZeroU64::new(max_total_result_bytes).ok_or(
                QueryCoordinatorError::InvalidPolicy("max_total_result_bytes"),
            )?,
            max_total_result_pages: NonZeroU64::new(max_total_result_pages).ok_or(
                QueryCoordinatorError::InvalidPolicy("max_total_result_pages"),
            )?,
            max_recovery_records: NonZeroUsize::new(max_recovery_records)
                .ok_or(QueryCoordinatorError::InvalidPolicy("max_recovery_records"))?,
        };
        if policy.max_running_per_workspace.get() > policy.max_running.get()
            || policy.max_running_per_principal.get() > policy.max_running.get()
            || policy.max_events_per_query.get() < 3
        {
            return Err(QueryCoordinatorError::InvalidPolicy(
                "scope running bounds or terminal event reservation",
            ));
        }
        Ok(policy)
    }
}

/// Stable control event set; semantic response bytes are deliberately absent.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum QueryControlEventPayload {
    SnapshotPinned {
        epoch_id: String,
        source_generation: u64,
        activation_head: u64,
        lifecycle_watermark: u64,
    },
    Progress {
        stage: String,
        completed: u64,
        total: Option<u64>,
    },
    ResultReady {
        manifest_path: String,
        manifest_checksum: String,
        total_rows: u64,
        total_pages: u64,
        total_bytes: u64,
    },
    Terminal {
        state: QueryTerminalState,
        public_code: Option<String>,
    },
}

impl QueryControlEventPayload {
    const fn is_progress(&self) -> bool {
        matches!(self, Self::Progress { .. })
    }

    const fn is_terminal(&self) -> bool {
        matches!(self, Self::Terminal { .. })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryTerminalState {
    Succeeded,
    Failed,
    Cancelled,
    Lost,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueryControlEvent {
    pub sequence: u64,
    pub emitted_at_unix_ms: i64,
    pub payload: QueryControlEventPayload,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryExecutionPhase {
    Queued,
    Running,
    Terminal(QueryTerminalState),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueryAcceptance {
    pub query_id: String,
    pub operation_fingerprint: String,
    pub accepted_at_unix_ms: i64,
    pub lease_expires_at_unix_ms: i64,
    pub generation: u64,
    pub phase: QueryExecutionPhase,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueryAcceptanceOutcome {
    New(QueryAcceptance),
    Replay(QueryAcceptance),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DurableQueryRecord {
    acceptance: QueryAcceptance,
    workspace_id: String,
    principal_id: String,
    idempotency_key: String,
    operation_fingerprint: String,
    semantic_profile: String,
    reserved_result_bytes: u64,
    reserved_result_pages: u64,
    result_reservation_live: bool,
    events: Vec<QueryControlEvent>,
}

/// Persistent control journal. Implementations must serialize writes through one logical owner.
pub(crate) trait QueryCoordinatorJournal: fmt::Debug + Send + Sync + 'static {
    fn load(&self, maximum: usize) -> Result<Vec<DurableQueryRecord>, QueryCoordinatorError>;
    fn create(&self, record: &DurableQueryRecord) -> Result<(), QueryCoordinatorError>;
    fn replace(&self, record: &DurableQueryRecord) -> Result<(), QueryCoordinatorError>;
    fn delete(&self, query_id: &str) -> Result<(), QueryCoordinatorError>;
}

/// SQLite implementation storing one canonical record per query and no result bytes.
pub(crate) struct SqliteQueryCoordinatorJournal {
    path: PathBuf,
    connection: std::sync::Mutex<Connection>,
}

impl fmt::Debug for SqliteQueryCoordinatorJournal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqliteQueryCoordinatorJournal")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl SqliteQueryCoordinatorJournal {
    pub(crate) fn open(path: &Path) -> Result<Self, QueryCoordinatorError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(QueryCoordinatorError::Io)?;
        }
        let connection = Connection::open(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(QueryCoordinatorError::Io)?;
        }
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=FULL;
             PRAGMA foreign_keys=ON;
             PRAGMA trusted_schema=OFF;
             CREATE TABLE IF NOT EXISTS query_coordinator_record (
               query_id TEXT PRIMARY KEY NOT NULL,
               record_bytes BLOB NOT NULL,
               expires_at INTEGER NOT NULL
             ) STRICT;",
        )?;
        Ok(Self {
            path: path.to_owned(),
            connection: std::sync::Mutex::new(connection),
        })
    }

    fn canonical_record(record: &DurableQueryRecord) -> Result<Vec<u8>, QueryCoordinatorError> {
        serde_json_canonicalizer::to_vec(record).map_err(QueryCoordinatorError::JournalEncoding)
    }
}

impl QueryCoordinatorJournal for SqliteQueryCoordinatorJournal {
    fn load(&self, maximum: usize) -> Result<Vec<DurableQueryRecord>, QueryCoordinatorError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| QueryCoordinatorError::JournalState)?;
        let mut statement = connection.prepare(
            "SELECT record_bytes FROM query_coordinator_record ORDER BY query_id LIMIT ?1",
        )?;
        let maximum = i64::try_from(maximum).unwrap_or(i64::MAX);
        statement
            .query_map([maximum], |row| row.get::<_, Vec<u8>>(0))?
            .map(|row| {
                let bytes = row?;
                let record: DurableQueryRecord = serde_json::from_slice(&bytes)
                    .map_err(QueryCoordinatorError::JournalEncoding)?;
                if Self::canonical_record(&record)? != bytes {
                    return Err(QueryCoordinatorError::NonCanonicalJournalRecord);
                }
                Ok(record)
            })
            .collect()
    }

    fn create(&self, record: &DurableQueryRecord) -> Result<(), QueryCoordinatorError> {
        let bytes = Self::canonical_record(record)?;
        self.connection
            .lock()
            .map_err(|_| QueryCoordinatorError::JournalState)?
            .execute(
                "INSERT INTO query_coordinator_record(query_id, record_bytes, expires_at)
                 VALUES (?1, ?2, ?3)",
                params![
                    record.acceptance.query_id,
                    bytes,
                    record.acceptance.lease_expires_at_unix_ms
                ],
            )?;
        Ok(())
    }

    fn replace(&self, record: &DurableQueryRecord) -> Result<(), QueryCoordinatorError> {
        let bytes = Self::canonical_record(record)?;
        let changed = self
            .connection
            .lock()
            .map_err(|_| QueryCoordinatorError::JournalState)?
            .execute(
                "UPDATE query_coordinator_record
                 SET record_bytes=?2, expires_at=?3 WHERE query_id=?1",
                params![
                    record.acceptance.query_id,
                    bytes,
                    record.acceptance.lease_expires_at_unix_ms
                ],
            )?;
        if changed != 1 {
            return Err(QueryCoordinatorError::UnknownQuery(
                record.acceptance.query_id.clone(),
            ));
        }
        Ok(())
    }

    fn delete(&self, query_id: &str) -> Result<(), QueryCoordinatorError> {
        self.connection
            .lock()
            .map_err(|_| QueryCoordinatorError::JournalState)?
            .execute(
                "DELETE FROM query_coordinator_record WHERE query_id=?1",
                [query_id],
            )?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct IdempotencyScope {
    workspace_id: WorkspaceId,
    principal_id: PrincipalId,
    key: Arc<str>,
}

struct QueryHandle {
    operation: NormalizedQueryOperation,
    acceptance: QueryAcceptance,
    fingerprint: [u8; 32],
    cancelled: Arc<AtomicBool>,
    events: Vec<QueryControlEvent>,
    event_bytes: usize,
    result_reservation_live: bool,
    changed: Arc<Notify>,
}

impl fmt::Debug for QueryHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QueryHandle")
            .field("acceptance", &self.acceptance)
            .field("event_count", &self.events.len())
            .field("event_bytes", &self.event_bytes)
            .finish_non_exhaustive()
    }
}

struct CoordinatorState {
    handles: BTreeMap<String, QueryHandle>,
    idempotency: BTreeMap<IdempotencyScope, String>,
    queue: VecDeque<String>,
    running: usize,
    running_by_workspace: BTreeMap<WorkspaceId, usize>,
    running_by_principal: BTreeMap<PrincipalId, usize>,
    reserved_result_bytes: u64,
    reserved_result_pages: u64,
    tasks: BTreeMap<String, tokio::task::JoinHandle<()>>,
}

impl Default for CoordinatorState {
    fn default() -> Self {
        Self {
            handles: BTreeMap::new(),
            idempotency: BTreeMap::new(),
            queue: VecDeque::new(),
            running: 0,
            running_by_workspace: BTreeMap::new(),
            running_by_principal: BTreeMap::new(),
            reserved_result_bytes: 0,
            reserved_result_pages: 0,
            tasks: BTreeMap::new(),
        }
    }
}

/// One daemon-wide coordinator. Clones share all scheduling and durability state.
#[derive(Clone)]
pub struct QueryCoordinator {
    policy: QueryCoordinatorPolicy,
    generation: u64,
    cursor_secret: [u8; 32],
    journal: Arc<dyn QueryCoordinatorJournal>,
    state: Arc<Mutex<CoordinatorState>>,
}

impl fmt::Debug for QueryCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QueryCoordinator")
            .field("policy", &self.policy)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl QueryCoordinator {
    /// Reopen durable control state. Nonterminal work becomes `LOST`; it is never rerun.
    pub(crate) fn try_new(
        policy: QueryCoordinatorPolicy,
        generation: u64,
        cursor_secret: [u8; 32],
        journal: Arc<dyn QueryCoordinatorJournal>,
        observed_at_unix_ms: i64,
    ) -> Result<Self, QueryCoordinatorError> {
        if generation == 0 || cursor_secret.iter().all(|byte| *byte == 0) {
            return Err(QueryCoordinatorError::InvalidCoordinatorIdentity);
        }
        let recovered = journal.load(policy.max_recovery_records.get().saturating_add(1))?;
        if recovered.len() > policy.max_recovery_records.get() {
            return Err(QueryCoordinatorError::RecoveryLimit);
        }
        let mut state = CoordinatorState::default();
        for mut durable in recovered {
            if durable.acceptance.lease_expires_at_unix_ms <= observed_at_unix_ms {
                journal.delete(&durable.acceptance.query_id)?;
                continue;
            }
            if !matches!(durable.acceptance.phase, QueryExecutionPhase::Terminal(_)) {
                durable.acceptance.phase = QueryExecutionPhase::Terminal(QueryTerminalState::Lost);
                durable.result_reservation_live = false;
                let sequence = next_sequence(&durable.events)?;
                durable.events.push(QueryControlEvent {
                    sequence,
                    emitted_at_unix_ms: observed_at_unix_ms,
                    payload: QueryControlEventPayload::Terminal {
                        state: QueryTerminalState::Lost,
                        public_code: Some("QUERY_LOST_DURING_RESTART".to_owned()),
                    },
                });
                journal.replace(&durable)?;
            }
            let workspace_id = WorkspaceId::from_bytes(decode_hex16(&durable.workspace_id)?);
            let principal_id = PrincipalId::from_bytes(decode_hex16(&durable.principal_id)?);
            let fingerprint = decode_hex32(&durable.operation_fingerprint)?;
            let scope = IdempotencyScope {
                workspace_id,
                principal_id,
                key: Arc::from(durable.idempotency_key.as_str()),
            };
            if durable.result_reservation_live {
                state
                    .reserved_result_bytes
                    .checked_add(durable.reserved_result_bytes)
                    .filter(|value| *value <= policy.max_total_result_bytes.get())
                    .ok_or(QueryCoordinatorError::RecoveryCapacity)?;
                state.reserved_result_bytes += durable.reserved_result_bytes;
                state
                    .reserved_result_pages
                    .checked_add(durable.reserved_result_pages)
                    .filter(|value| *value <= policy.max_total_result_pages.get())
                    .ok_or(QueryCoordinatorError::RecoveryCapacity)?;
                state.reserved_result_pages += durable.reserved_result_pages;
            }
            let operation = NormalizedQueryOperation {
                workspace_id,
                principal_id,
                idempotency_key: Arc::clone(&scope.key),
                canonical_request: Arc::from(b"{}".as_slice()),
                semantic_profile: Arc::from(durable.semantic_profile.as_str()),
                request_contract: Arc::from("recovered"),
                response_contract: Arc::from("recovered"),
                delivery_profile: Arc::from("recovered"),
                compression_profile: Arc::from("recovered"),
                freshness_policy: Arc::from("recovered"),
                epoch_policy: Arc::from("recovered"),
                deadline_unix_ms: durable.acceptance.accepted_at_unix_ms.saturating_add(1),
                lease_expires_at_unix_ms: durable.acceptance.lease_expires_at_unix_ms,
                maximum_result_bytes: durable.reserved_result_bytes,
                maximum_result_pages: durable.reserved_result_pages,
            };
            let event_bytes = durable.events.iter().try_fold(0_usize, |total, event| {
                let bytes = serde_json_canonicalizer::to_vec(event)
                    .map_err(QueryCoordinatorError::JournalEncoding)?;
                total
                    .checked_add(bytes.len())
                    .ok_or(QueryCoordinatorError::CounterOverflow)
            })?;
            let query_id = durable.acceptance.query_id.clone();
            state.idempotency.insert(scope, query_id.clone());
            state.handles.insert(
                query_id,
                QueryHandle {
                    operation,
                    acceptance: durable.acceptance,
                    fingerprint,
                    cancelled: Arc::new(AtomicBool::new(false)),
                    events: durable.events,
                    event_bytes,
                    result_reservation_live: durable.result_reservation_live,
                    changed: Arc::new(Notify::new()),
                },
            );
        }
        Ok(Self {
            policy,
            generation,
            cursor_secret,
            journal,
            state: Arc::new(Mutex::new(state)),
        })
    }

    /// Reserve every bounded resource and return an exact new or replayed acceptance.
    pub async fn accept(
        &self,
        operation: NormalizedQueryOperation,
        observed_at_unix_ms: i64,
    ) -> Result<QueryAcceptanceOutcome, QueryCoordinatorError> {
        let operation = NormalizedQueryOperation::try_new(operation)?;
        if operation.deadline_unix_ms <= observed_at_unix_ms {
            return Err(QueryCoordinatorError::DeadlineElapsed);
        }
        let fingerprint = operation.fingerprint();
        let scope = IdempotencyScope {
            workspace_id: operation.workspace_id,
            principal_id: operation.principal_id,
            key: Arc::clone(&operation.idempotency_key),
        };
        let mut state = self.state.lock().await;
        if let Some(query_id) = state.idempotency.get(&scope) {
            let handle = state
                .handles
                .get(query_id)
                .ok_or(QueryCoordinatorError::CoordinatorState)?;
            if handle.fingerprint != fingerprint {
                return Err(QueryCoordinatorError::IdempotencyConflict);
            }
            return Ok(QueryAcceptanceOutcome::Replay(handle.acceptance.clone()));
        }
        if state.queue.len() >= self.policy.max_queued.get()
            || state.tasks.len() >= self.policy.max_tasks.get()
        {
            return Err(QueryCoordinatorError::AdmissionBackpressure);
        }
        let next_bytes = state
            .reserved_result_bytes
            .checked_add(operation.maximum_result_bytes)
            .ok_or(QueryCoordinatorError::CounterOverflow)?;
        let next_pages = state
            .reserved_result_pages
            .checked_add(operation.maximum_result_pages)
            .ok_or(QueryCoordinatorError::CounterOverflow)?;
        if next_bytes > self.policy.max_total_result_bytes.get()
            || next_pages > self.policy.max_total_result_pages.get()
        {
            return Err(QueryCoordinatorError::ResultCapacityBackpressure);
        }
        let query_id = format!("query:{}", hex(&fingerprint));
        if state.handles.contains_key(&query_id) {
            return Err(QueryCoordinatorError::QueryIdentityCollision);
        }
        let acceptance = QueryAcceptance {
            query_id: query_id.clone(),
            operation_fingerprint: hex(&fingerprint),
            accepted_at_unix_ms: observed_at_unix_ms,
            lease_expires_at_unix_ms: operation.lease_expires_at_unix_ms,
            generation: self.generation,
            phase: QueryExecutionPhase::Queued,
        };
        let handle = QueryHandle {
            operation,
            acceptance: acceptance.clone(),
            fingerprint,
            cancelled: Arc::new(AtomicBool::new(false)),
            events: Vec::with_capacity(self.policy.max_events_per_query.get()),
            event_bytes: 0,
            result_reservation_live: true,
            changed: Arc::new(Notify::new()),
        };
        self.journal.create(&durable_record(&handle))?;
        state.reserved_result_bytes = next_bytes;
        state.reserved_result_pages = next_pages;
        state.idempotency.insert(scope, query_id.clone());
        state.queue.push_back(query_id.clone());
        state.handles.insert(query_id, handle);
        self.dispatch_locked(&mut state)?;
        let acceptance = state
            .handles
            .get(&acceptance.query_id)
            .ok_or(QueryCoordinatorError::CoordinatorState)?
            .acceptance
            .clone();
        Ok(QueryAcceptanceOutcome::New(acceptance))
    }

    /// Wait until fair admission starts this query; queued cancellation fails without execution.
    pub async fn await_running(
        &self,
        query_id: &str,
    ) -> Result<QueryExecutionPermit, QueryCoordinatorError> {
        loop {
            let notified = {
                let state = self.state.lock().await;
                let handle = state
                    .handles
                    .get(query_id)
                    .ok_or_else(|| QueryCoordinatorError::UnknownQuery(query_id.to_owned()))?;
                match handle.acceptance.phase {
                    QueryExecutionPhase::Running => {
                        return Ok(QueryExecutionPermit {
                            query_id: query_id.to_owned(),
                            cancellation: Arc::clone(&handle.cancelled),
                            completed: false,
                        });
                    }
                    QueryExecutionPhase::Terminal(QueryTerminalState::Cancelled) => {
                        return Err(QueryCoordinatorError::Cancelled);
                    }
                    QueryExecutionPhase::Terminal(_) => {
                        return Err(QueryCoordinatorError::AlreadyTerminal);
                    }
                    QueryExecutionPhase::Queued => Arc::clone(&handle.changed),
                }
            };
            notified.notified().await;
        }
    }

    /// Attach the one task owner after a new acceptance; replay never creates a second task.
    pub async fn register_task(
        &self,
        query_id: &str,
        task: tokio::task::JoinHandle<()>,
    ) -> Result<(), QueryCoordinatorError> {
        let mut state = self.state.lock().await;
        if !state.handles.contains_key(query_id) {
            task.abort();
            return Err(QueryCoordinatorError::UnknownQuery(query_id.to_owned()));
        }
        if state.tasks.len() >= self.policy.max_tasks.get() || state.tasks.contains_key(query_id) {
            task.abort();
            return Err(QueryCoordinatorError::TaskCapacity);
        }
        state.tasks.insert(query_id.to_owned(), task);
        Ok(())
    }

    /// Append a control event, coalescing adjacent progress before sequence allocation.
    pub async fn append_event(
        &self,
        query_id: &str,
        payload: QueryControlEventPayload,
        observed_at_unix_ms: i64,
    ) -> Result<QueryControlEvent, QueryCoordinatorError> {
        if payload.is_terminal() {
            return Err(QueryCoordinatorError::TerminalRequiresClosure);
        }
        let mut state = self.state.lock().await;
        let handle = state
            .handles
            .get_mut(query_id)
            .ok_or_else(|| QueryCoordinatorError::UnknownQuery(query_id.to_owned()))?;
        if matches!(handle.acceptance.phase, QueryExecutionPhase::Terminal(_)) {
            return Err(QueryCoordinatorError::AlreadyTerminal);
        }
        let coalesced = payload.is_progress()
            && handle
                .events
                .last()
                .is_some_and(|event| event.payload.is_progress());
        let sequence = if coalesced {
            handle.events.last().map_or(1, |event| event.sequence)
        } else {
            next_sequence(&handle.events)?
        };
        let event = QueryControlEvent {
            sequence,
            emitted_at_unix_ms: observed_at_unix_ms,
            payload,
        };
        let encoded = serde_json_canonicalizer::to_vec(&event)
            .map_err(QueryCoordinatorError::JournalEncoding)?;
        let replaced_bytes = if coalesced {
            handle
                .events
                .last()
                .map(|previous| {
                    serde_json_canonicalizer::to_vec(previous)
                        .map(|bytes| bytes.len())
                        .map_err(QueryCoordinatorError::JournalEncoding)
                })
                .transpose()?
                .unwrap_or(0)
        } else {
            0
        };
        let next_count = handle.events.len() + usize::from(!coalesced);
        let next_bytes = handle
            .event_bytes
            .checked_sub(replaced_bytes)
            .and_then(|bytes| bytes.checked_add(encoded.len()))
            .ok_or(QueryCoordinatorError::CounterOverflow)?;
        // One event slot and a conservative 1 KiB tail remain reserved for Terminal.
        if next_count >= self.policy.max_events_per_query.get()
            || next_bytes.saturating_add(1_024) > self.policy.max_event_bytes_per_query.get()
        {
            return Err(QueryCoordinatorError::JournalCapacity);
        }
        if coalesced {
            *handle
                .events
                .last_mut()
                .ok_or(QueryCoordinatorError::CoordinatorState)? = event.clone();
        } else {
            handle.events.push(event.clone());
        }
        handle.event_bytes = next_bytes;
        self.journal.replace(&durable_record(handle))?;
        handle.changed.notify_waiters();
        Ok(event)
    }

    /// Close once, release running capacity, and retain or release the reserved result envelope.
    pub async fn terminal(
        &self,
        query_id: &str,
        terminal: QueryTerminalState,
        public_code: Option<String>,
        actual_result: Option<(u64, u64)>,
        observed_at_unix_ms: i64,
    ) -> Result<QueryControlEvent, QueryCoordinatorError> {
        let mut state = self.state.lock().await;
        let (
            workspace,
            principal,
            prior_phase,
            event,
            notify,
            release_result,
            reserved_bytes,
            reserved_pages,
        ) = {
            let handle = state
                .handles
                .get_mut(query_id)
                .ok_or_else(|| QueryCoordinatorError::UnknownQuery(query_id.to_owned()))?;
            if matches!(handle.acceptance.phase, QueryExecutionPhase::Terminal(_)) {
                return Err(QueryCoordinatorError::AlreadyTerminal);
            }
            let release_result = if terminal == QueryTerminalState::Succeeded {
                let (bytes, pages) = actual_result.ok_or(QueryCoordinatorError::ResultMissing)?;
                if bytes > handle.operation.maximum_result_bytes
                    || pages > handle.operation.maximum_result_pages
                {
                    return Err(QueryCoordinatorError::ResultReservationExceeded);
                }
                false
            } else {
                true
            };
            let sequence = next_sequence(&handle.events)?;
            let event = QueryControlEvent {
                sequence,
                emitted_at_unix_ms: observed_at_unix_ms,
                payload: QueryControlEventPayload::Terminal {
                    state: terminal,
                    public_code,
                },
            };
            let encoded = serde_json_canonicalizer::to_vec(&event)
                .map_err(QueryCoordinatorError::JournalEncoding)?;
            let next_bytes = handle
                .event_bytes
                .checked_add(encoded.len())
                .ok_or(QueryCoordinatorError::CounterOverflow)?;
            if handle.events.len() >= self.policy.max_events_per_query.get()
                || next_bytes > self.policy.max_event_bytes_per_query.get()
            {
                return Err(QueryCoordinatorError::TerminalReservationBroken);
            }
            let prior_phase = handle.acceptance.phase;
            handle.events.push(event.clone());
            handle.event_bytes = next_bytes;
            handle.acceptance.phase = QueryExecutionPhase::Terminal(terminal);
            if release_result {
                handle.result_reservation_live = false;
            }
            self.journal.replace(&durable_record(handle))?;
            (
                handle.operation.workspace_id,
                handle.operation.principal_id,
                prior_phase,
                event,
                Arc::clone(&handle.changed),
                release_result,
                handle.operation.maximum_result_bytes,
                handle.operation.maximum_result_pages,
            )
        };
        if prior_phase == QueryExecutionPhase::Running {
            release_running(&mut state, workspace, principal)?;
        } else {
            state.queue.retain(|queued| queued != query_id);
        }
        if release_result {
            release_result_reservation(&mut state, reserved_bytes, reserved_pages)?;
        }
        state.tasks.remove(query_id);
        self.dispatch_locked(&mut state)?;
        drop(state);
        notify.notify_waiters();
        Ok(event)
    }

    /// Signal cancellation at every stage; queued work closes immediately.
    pub async fn cancel(
        &self,
        query_id: &str,
        observed_at_unix_ms: i64,
    ) -> Result<(), QueryCoordinatorError> {
        let queued = {
            let state = self.state.lock().await;
            let handle = state
                .handles
                .get(query_id)
                .ok_or_else(|| QueryCoordinatorError::UnknownQuery(query_id.to_owned()))?;
            handle.cancelled.store(true, Ordering::Release);
            handle.acceptance.phase == QueryExecutionPhase::Queued
        };
        if queued {
            self.terminal(
                query_id,
                QueryTerminalState::Cancelled,
                Some("CANCELLED".to_owned()),
                None,
                observed_at_unix_ms,
            )
            .await?;
        }
        Ok(())
    }

    #[must_use]
    pub async fn cancellation(&self, query_id: &str) -> Option<Arc<AtomicBool>> {
        self.state
            .lock()
            .await
            .handles
            .get(query_id)
            .map(|handle| Arc::clone(&handle.cancelled))
    }

    /// Return an immutable bounded event suffix.
    pub async fn events_after(
        &self,
        query_id: &str,
        after_sequence: u64,
    ) -> Result<Vec<QueryControlEvent>, QueryCoordinatorError> {
        let state = self.state.lock().await;
        let handle = state
            .handles
            .get(query_id)
            .ok_or_else(|| QueryCoordinatorError::UnknownQuery(query_id.to_owned()))?;
        Ok(handle
            .events
            .iter()
            .filter(|event| event.sequence > after_sequence)
            .cloned()
            .collect())
    }

    /// Mint a generation/principal/profile/expiry/content-bound opaque resume cursor.
    pub async fn mint_cursor(
        &self,
        query_id: &str,
        after_sequence: u64,
        expires_at_unix_ms: i64,
    ) -> Result<String, QueryCoordinatorError> {
        let state = self.state.lock().await;
        let handle = state
            .handles
            .get(query_id)
            .ok_or_else(|| QueryCoordinatorError::UnknownQuery(query_id.to_owned()))?;
        if expires_at_unix_ms > handle.acceptance.lease_expires_at_unix_ms {
            return Err(QueryCoordinatorError::CursorExpiry);
        }
        let event_checksum = handle
            .events
            .iter()
            .find(|event| event.sequence == after_sequence)
            .map(|event| {
                serde_json_canonicalizer::to_vec(event)
                    .map(|bytes| hex(blake3::hash(&bytes).as_bytes()))
                    .map_err(QueryCoordinatorError::JournalEncoding)
            })
            .transpose()?;
        let payload = CursorPayload {
            query_id: query_id.to_owned(),
            principal_id: hex(handle.operation.principal_id.as_bytes()),
            generation: self.generation,
            semantic_profile: handle.operation.semantic_profile.to_string(),
            after_sequence,
            event_checksum,
            expires_at_unix_ms,
        };
        encode_cursor(&payload, &self.cursor_secret)
    }

    pub async fn verify_cursor(
        &self,
        cursor: &str,
        principal_id: PrincipalId,
        observed_at_unix_ms: i64,
    ) -> Result<(String, u64), QueryCoordinatorError> {
        let payload = decode_cursor(cursor, &self.cursor_secret)?;
        if payload.generation != self.generation
            || payload.principal_id != hex(principal_id.as_bytes())
            || payload.expires_at_unix_ms <= observed_at_unix_ms
        {
            return Err(QueryCoordinatorError::CursorBinding);
        }
        let state = self.state.lock().await;
        let handle = state
            .handles
            .get(&payload.query_id)
            .ok_or_else(|| QueryCoordinatorError::UnknownQuery(payload.query_id.clone()))?;
        if payload.semantic_profile.as_str() != handle.operation.semantic_profile.as_ref() {
            return Err(QueryCoordinatorError::CursorBinding);
        }
        let expected_checksum = handle
            .events
            .iter()
            .find(|event| event.sequence == payload.after_sequence)
            .map(|event| {
                serde_json_canonicalizer::to_vec(event)
                    .map(|bytes| hex(blake3::hash(&bytes).as_bytes()))
                    .map_err(QueryCoordinatorError::JournalEncoding)
            })
            .transpose()?;
        if payload.event_checksum != expected_checksum {
            return Err(QueryCoordinatorError::CursorBinding);
        }
        Ok((payload.query_id, payload.after_sequence))
    }

    /// Release a succeeded result reservation after the durable tombstone wins.
    pub async fn release_result(&self, query_id: &str) -> Result<(), QueryCoordinatorError> {
        let mut state = self.state.lock().await;
        let (phase, reservation_live, reserved_bytes, reserved_pages) = {
            let handle = state
                .handles
                .get(query_id)
                .ok_or_else(|| QueryCoordinatorError::UnknownQuery(query_id.to_owned()))?;
            (
                handle.acceptance.phase,
                handle.result_reservation_live,
                handle.operation.maximum_result_bytes,
                handle.operation.maximum_result_pages,
            )
        };
        if phase != QueryExecutionPhase::Terminal(QueryTerminalState::Succeeded) {
            return Err(QueryCoordinatorError::ResultNotReleasable);
        }
        if !reservation_live {
            return Ok(());
        }
        {
            let handle = state
                .handles
                .get_mut(query_id)
                .ok_or(QueryCoordinatorError::CoordinatorState)?;
            handle.result_reservation_live = false;
            if let Err(error) = self.journal.replace(&durable_record(handle)) {
                handle.result_reservation_live = true;
                return Err(error);
            }
        }
        release_result_reservation(&mut state, reserved_bytes, reserved_pages)
    }

    /// Expire terminal entries and their tombstones under the one declared lease policy.
    pub async fn collect_expired(
        &self,
        observed_at_unix_ms: i64,
    ) -> Result<usize, QueryCoordinatorError> {
        let mut state = self.state.lock().await;
        let expired = state
            .handles
            .iter()
            .filter_map(|(query_id, handle)| {
                (handle.acceptance.lease_expires_at_unix_ms <= observed_at_unix_ms)
                    .then(|| query_id.clone())
            })
            .collect::<Vec<_>>();
        for query_id in &expired {
            let handle = state
                .handles
                .remove(query_id)
                .ok_or(QueryCoordinatorError::CoordinatorState)?;
            state.queue.retain(|queued| queued != query_id);
            if handle.acceptance.phase == QueryExecutionPhase::Running {
                release_running(
                    &mut state,
                    handle.operation.workspace_id,
                    handle.operation.principal_id,
                )?;
            }
            if handle.result_reservation_live {
                release_result_reservation(
                    &mut state,
                    handle.operation.maximum_result_bytes,
                    handle.operation.maximum_result_pages,
                )?;
            }
            state.tasks.remove(query_id).inspect(|task| task.abort());
            state.idempotency.remove(&IdempotencyScope {
                workspace_id: handle.operation.workspace_id,
                principal_id: handle.operation.principal_id,
                key: Arc::clone(&handle.operation.idempotency_key),
            });
            self.journal.delete(query_id)?;
        }
        self.dispatch_locked(&mut state)?;
        Ok(expired.len())
    }

    fn dispatch_locked(&self, state: &mut CoordinatorState) -> Result<(), QueryCoordinatorError> {
        let mut attempts = state.queue.len();
        while state.running < self.policy.max_running.get() && attempts > 0 {
            attempts -= 1;
            let Some(query_id) = state.queue.pop_front() else {
                break;
            };
            let (workspace, principal, terminal) = {
                let handle = state
                    .handles
                    .get(&query_id)
                    .ok_or(QueryCoordinatorError::CoordinatorState)?;
                (
                    handle.operation.workspace_id,
                    handle.operation.principal_id,
                    matches!(handle.acceptance.phase, QueryExecutionPhase::Terminal(_)),
                )
            };
            if terminal {
                continue;
            }
            if state
                .running_by_workspace
                .get(&workspace)
                .copied()
                .unwrap_or(0)
                >= self.policy.max_running_per_workspace.get()
                || state
                    .running_by_principal
                    .get(&principal)
                    .copied()
                    .unwrap_or(0)
                    >= self.policy.max_running_per_principal.get()
            {
                state.queue.push_back(query_id);
                continue;
            }
            state.running += 1;
            *state.running_by_workspace.entry(workspace).or_default() += 1;
            *state.running_by_principal.entry(principal).or_default() += 1;
            let handle = state
                .handles
                .get_mut(&query_id)
                .ok_or(QueryCoordinatorError::CoordinatorState)?;
            handle.acceptance.phase = QueryExecutionPhase::Running;
            self.journal.replace(&durable_record(handle))?;
            handle.changed.notify_waiters();
        }
        Ok(())
    }
}

/// Execution ownership returned only after fair running admission.
#[derive(Debug)]
pub struct QueryExecutionPermit {
    query_id: String,
    cancellation: Arc<AtomicBool>,
    completed: bool,
}

impl QueryExecutionPermit {
    #[must_use]
    pub fn query_id(&self) -> &str {
        &self.query_id
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.load(Ordering::Acquire)
    }

    pub fn complete(mut self) {
        self.completed = true;
    }
}

impl Drop for QueryExecutionPermit {
    fn drop(&mut self) {
        if !self.completed {
            self.cancellation.store(true, Ordering::Release);
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CursorPayload {
    query_id: String,
    principal_id: String,
    generation: u64,
    semantic_profile: String,
    after_sequence: u64,
    event_checksum: Option<String>,
    expires_at_unix_ms: i64,
}

fn encode_cursor(
    payload: &CursorPayload,
    secret: &[u8; 32],
) -> Result<String, QueryCoordinatorError> {
    let bytes = serde_json_canonicalizer::to_vec(payload)
        .map_err(QueryCoordinatorError::JournalEncoding)?;
    let mac = blake3::keyed_hash(secret, &bytes);
    Ok(format!("{}.{}", hex(&bytes), hex(mac.as_bytes())))
}

fn decode_cursor(cursor: &str, secret: &[u8; 32]) -> Result<CursorPayload, QueryCoordinatorError> {
    let (payload, mac) = cursor
        .split_once('.')
        .ok_or(QueryCoordinatorError::InvalidCursor)?;
    let bytes = decode_hex(payload)?;
    let observed_mac = decode_hex32(mac)?;
    if !constant_time_equal(blake3::keyed_hash(secret, &bytes).as_bytes(), &observed_mac) {
        return Err(QueryCoordinatorError::InvalidCursor);
    }
    let payload: CursorPayload =
        serde_json::from_slice(&bytes).map_err(QueryCoordinatorError::JournalEncoding)?;
    if serde_json_canonicalizer::to_vec(&payload).map_err(QueryCoordinatorError::JournalEncoding)?
        != bytes
    {
        return Err(QueryCoordinatorError::InvalidCursor);
    }
    Ok(payload)
}

fn durable_record(handle: &QueryHandle) -> DurableQueryRecord {
    DurableQueryRecord {
        acceptance: handle.acceptance.clone(),
        workspace_id: hex(handle.operation.workspace_id.as_bytes()),
        principal_id: hex(handle.operation.principal_id.as_bytes()),
        idempotency_key: handle.operation.idempotency_key.to_string(),
        operation_fingerprint: hex(&handle.fingerprint),
        semantic_profile: handle.operation.semantic_profile.to_string(),
        reserved_result_bytes: handle.operation.maximum_result_bytes,
        reserved_result_pages: handle.operation.maximum_result_pages,
        result_reservation_live: handle.result_reservation_live,
        events: handle.events.clone(),
    }
}

fn release_running(
    state: &mut CoordinatorState,
    workspace: WorkspaceId,
    principal: PrincipalId,
) -> Result<(), QueryCoordinatorError> {
    state.running = state
        .running
        .checked_sub(1)
        .ok_or(QueryCoordinatorError::CoordinatorState)?;
    decrement_scope(&mut state.running_by_workspace, workspace)?;
    decrement_scope(&mut state.running_by_principal, principal)
}

fn decrement_scope<K: Ord + Copy>(
    values: &mut BTreeMap<K, usize>,
    key: K,
) -> Result<(), QueryCoordinatorError> {
    let value = values
        .get_mut(&key)
        .ok_or(QueryCoordinatorError::CoordinatorState)?;
    *value = value
        .checked_sub(1)
        .ok_or(QueryCoordinatorError::CoordinatorState)?;
    if *value == 0 {
        values.remove(&key);
    }
    Ok(())
}

fn release_result_reservation(
    state: &mut CoordinatorState,
    reserved_bytes: u64,
    reserved_pages: u64,
) -> Result<(), QueryCoordinatorError> {
    state.reserved_result_bytes = state
        .reserved_result_bytes
        .checked_sub(reserved_bytes)
        .ok_or(QueryCoordinatorError::CoordinatorState)?;
    state.reserved_result_pages = state
        .reserved_result_pages
        .checked_sub(reserved_pages)
        .ok_or(QueryCoordinatorError::CoordinatorState)?;
    Ok(())
}

fn next_sequence(events: &[QueryControlEvent]) -> Result<u64, QueryCoordinatorError> {
    events.last().map_or(Ok(1), |event| {
        event
            .sequence
            .checked_add(1)
            .ok_or(QueryCoordinatorError::CounterOverflow)
    })
}

fn frame(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn decode_hex(value: &str) -> Result<Vec<u8>, QueryCoordinatorError> {
    if value.len() % 2 != 0 {
        return Err(QueryCoordinatorError::InvalidHex);
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| Ok((decode_nibble(pair[0])? << 4) | decode_nibble(pair[1])?))
        .collect()
}

fn decode_hex16(value: &str) -> Result<[u8; 16], QueryCoordinatorError> {
    decode_hex(value)?
        .try_into()
        .map_err(|_| QueryCoordinatorError::InvalidHex)
}

fn decode_hex32(value: &str) -> Result<[u8; 32], QueryCoordinatorError> {
    decode_hex(value)?
        .try_into()
        .map_err(|_| QueryCoordinatorError::InvalidHex)
}

const fn decode_nibble(byte: u8) -> Result<u8, QueryCoordinatorError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(QueryCoordinatorError::InvalidHex),
    }
}

fn constant_time_equal(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

#[derive(Debug, Error)]
pub enum QueryCoordinatorError {
    #[error("invalid query coordinator policy {0}")]
    InvalidPolicy(&'static str),
    #[error("invalid query coordinator identity")]
    InvalidCoordinatorIdentity,
    #[error("invalid query owner identity")]
    InvalidIdentity,
    #[error("invalid idempotency key")]
    InvalidIdempotencyKey,
    #[error("invalid normalized operation field {0}")]
    InvalidOperationField(&'static str),
    #[error("invalid normalized operation bounds")]
    InvalidOperationBounds,
    #[error("canonical request is not canonical JSON")]
    NonCanonicalRequest,
    #[error("query deadline elapsed")]
    DeadlineElapsed,
    #[error("idempotency key is already bound to different operation meaning")]
    IdempotencyConflict,
    #[error("query admission is at bounded capacity")]
    AdmissionBackpressure,
    #[error("query result reservation is at bounded capacity")]
    ResultCapacityBackpressure,
    #[error("query task capacity is exhausted")]
    TaskCapacity,
    #[error("query journal capacity is exhausted")]
    JournalCapacity,
    #[error("terminal journal reservation was violated")]
    TerminalReservationBroken,
    #[error("terminal events require coordinator closure")]
    TerminalRequiresClosure,
    #[error("query is already terminal")]
    AlreadyTerminal,
    #[error("query was cancelled")]
    Cancelled,
    #[error("unknown query {0}")]
    UnknownQuery(String),
    #[error("query identity collision")]
    QueryIdentityCollision,
    #[error("query result is missing")]
    ResultMissing,
    #[error("query result exceeded its accepted reservation")]
    ResultReservationExceeded,
    #[error("query result is not releasable")]
    ResultNotReleasable,
    #[error("query cursor expiry exceeds the query lease")]
    CursorExpiry,
    #[error("query cursor is invalid")]
    InvalidCursor,
    #[error("query cursor binding differs")]
    CursorBinding,
    #[error("query coordinator recovery census exceeds its bound")]
    RecoveryLimit,
    #[error("query coordinator recovery exceeds current capacity")]
    RecoveryCapacity,
    #[error("query coordinator state is unavailable")]
    CoordinatorState,
    #[error("query coordinator journal state is unavailable")]
    JournalState,
    #[error("query coordinator journal record is not canonical")]
    NonCanonicalJournalRecord,
    #[error("query coordinator counter overflow")]
    CounterOverflow,
    #[error("invalid lowercase hexadecimal value")]
    InvalidHex,
    #[error("query canonical JSON failure: {0}")]
    CanonicalRequest(#[source] serde_json::Error),
    #[error("query journal JSON failure: {0}")]
    JournalEncoding(#[source] serde_json::Error),
    #[error("query journal SQLite failure: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("query journal I/O failure: {0}")]
    Io(#[source] std::io::Error),
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;
    use std::time::Duration;

    use tempfile::TempDir;

    use super::*;

    fn policy(max_running: usize, max_bytes: u64, max_pages: u64) -> QueryCoordinatorPolicy {
        QueryCoordinatorPolicy::try_new(
            max_running,
            1,
            1,
            16,
            16,
            16,
            32 * 1024,
            max_bytes,
            max_pages,
            64,
        )
        .expect("valid coordinator policy")
    }

    fn operation(key: &str, marker: u8) -> NormalizedQueryOperation {
        NormalizedQueryOperation::try_new(NormalizedQueryOperation {
            workspace_id: WorkspaceId::from_bytes([0x31; 16]),
            principal_id: PrincipalId::from_bytes([0x41; 16]),
            idempotency_key: Arc::from(key),
            canonical_request: Arc::from(
                format!(r#"{{"form":"definition","marker":{marker}}}"#).into_bytes(),
            ),
            semantic_profile: Arc::from("codefabric.semantic.v2"),
            request_contract: Arc::from("codefabric.request.v2"),
            response_contract: Arc::from("codefabric.response.v2"),
            delivery_profile: Arc::from("arrow-pages"),
            compression_profile: Arc::from("identity"),
            freshness_policy: Arc::from("current-activation"),
            epoch_policy: Arc::from("exact-selected"),
            deadline_unix_ms: 5_000,
            lease_expires_at_unix_ms: 10_000,
            maximum_result_bytes: 128,
            maximum_result_pages: 8,
        })
        .expect("valid operation")
    }

    fn acceptance(outcome: QueryAcceptanceOutcome) -> QueryAcceptance {
        match outcome {
            QueryAcceptanceOutcome::New(value) | QueryAcceptanceOutcome::Replay(value) => value,
        }
    }

    fn coordinator(
        temp: &TempDir,
        policy: QueryCoordinatorPolicy,
        generation: u64,
        observed_at_unix_ms: i64,
    ) -> QueryCoordinator {
        let journal = Arc::new(
            SqliteQueryCoordinatorJournal::open(&temp.path().join("query.sqlite"))
                .expect("open journal"),
        );
        QueryCoordinator::try_new(policy, generation, [0x71; 32], journal, observed_at_unix_ms)
            .expect("open coordinator")
    }

    #[tokio::test]
    async fn wp36_int_full_operation_idempotency_cursor_and_journal_bindings_are_exact() {
        let temp = tempfile::tempdir().expect("tempdir");
        let coordinator = coordinator(&temp, policy(2, 1_024, 64), 7, 1_000);
        assert_eq!(
            std::fs::metadata(temp.path().join("query.sqlite"))
                .expect("journal metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        let base = operation("same-key", 1);
        let first = acceptance(
            coordinator
                .accept(base.clone(), 1_000)
                .await
                .expect("accept query"),
        );
        let replay = coordinator
            .accept(base.clone(), 1_001)
            .await
            .expect("exact replay");
        assert!(matches!(replay, QueryAcceptanceOutcome::Replay(_)));
        assert_eq!(acceptance(replay), first);

        let mut variants = Vec::new();
        let mut value = base.clone();
        value.workspace_id = WorkspaceId::from_bytes([0x32; 16]);
        variants.push(value);
        let mut value = base.clone();
        value.principal_id = PrincipalId::from_bytes([0x42; 16]);
        variants.push(value);
        let mut value = base.clone();
        value.canonical_request = Arc::from(br#"{"form":"definition","marker":2}"#.as_slice());
        variants.push(value);
        for (field, changed) in [
            ("semantic", "codefabric.semantic.v3"),
            ("request", "codefabric.request.v3"),
            ("response", "codefabric.response.v3"),
            ("delivery", "row-projection"),
            ("compression", "zstd"),
            ("freshness", "bounded-stale"),
            ("epoch", "explicit-pin"),
        ] {
            let mut value = base.clone();
            match field {
                "semantic" => value.semantic_profile = Arc::from(changed),
                "request" => value.request_contract = Arc::from(changed),
                "response" => value.response_contract = Arc::from(changed),
                "delivery" => value.delivery_profile = Arc::from(changed),
                "compression" => value.compression_profile = Arc::from(changed),
                "freshness" => value.freshness_policy = Arc::from(changed),
                "epoch" => value.epoch_policy = Arc::from(changed),
                _ => unreachable!(),
            }
            variants.push(value);
        }
        let mut value = base.clone();
        value.deadline_unix_ms += 1;
        variants.push(value);
        let mut value = base.clone();
        value.lease_expires_at_unix_ms += 1;
        variants.push(value);
        let mut value = base.clone();
        value.maximum_result_bytes += 1;
        variants.push(value);
        let mut value = base.clone();
        value.maximum_result_pages += 1;
        variants.push(value);
        assert!(
            variants
                .iter()
                .all(|value| value.fingerprint() != base.fingerprint())
        );

        let mut conflict = base.clone();
        conflict.compression_profile = Arc::from("zstd");
        assert!(matches!(
            coordinator.accept(conflict, 1_002).await,
            Err(QueryCoordinatorError::IdempotencyConflict)
        ));

        let event = coordinator
            .append_event(
                &first.query_id,
                QueryControlEventPayload::SnapshotPinned {
                    epoch_id: "epoch:36".to_owned(),
                    source_generation: 11,
                    activation_head: 12,
                    lifecycle_watermark: 13,
                },
                1_003,
            )
            .await
            .expect("snapshot event");
        let cursor = coordinator
            .mint_cursor(&first.query_id, event.sequence, 9_000)
            .await
            .expect("cursor");
        assert_eq!(
            coordinator
                .verify_cursor(&cursor, base.principal_id, 1_004)
                .await
                .expect("cursor binding"),
            (first.query_id.clone(), event.sequence)
        );
        let mut forged = cursor.into_bytes();
        let last = forged.last_mut().expect("cursor byte");
        *last = if *last == b'a' { b'b' } else { b'a' };
        assert!(matches!(
            coordinator
                .verify_cursor(
                    std::str::from_utf8(&forged).expect("ASCII cursor"),
                    base.principal_id,
                    1_004,
                )
                .await,
            Err(QueryCoordinatorError::InvalidCursor)
        ));
    }

    #[tokio::test]
    async fn wp36_beh_bounded_fair_admission_progress_coalescing_and_terminal_dispatch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let coordinator = coordinator(&temp, policy(1, 1_024, 64), 8, 1_000);
        let first = acceptance(
            coordinator
                .accept(operation("first", 1), 1_000)
                .await
                .expect("first acceptance"),
        );
        let second = acceptance(
            coordinator
                .accept(operation("second", 2), 1_000)
                .await
                .expect("second acceptance"),
        );
        assert_eq!(first.phase, QueryExecutionPhase::Running);
        assert_eq!(second.phase, QueryExecutionPhase::Queued);

        coordinator
            .append_event(
                &first.query_id,
                QueryControlEventPayload::SnapshotPinned {
                    epoch_id: "epoch:36".to_owned(),
                    source_generation: 1,
                    activation_head: 2,
                    lifecycle_watermark: 3,
                },
                1_001,
            )
            .await
            .expect("snapshot");
        let first_progress = coordinator
            .append_event(
                &first.query_id,
                QueryControlEventPayload::Progress {
                    stage: "execute".to_owned(),
                    completed: 1,
                    total: Some(8),
                },
                1_002,
            )
            .await
            .expect("progress");
        let coalesced = coordinator
            .append_event(
                &first.query_id,
                QueryControlEventPayload::Progress {
                    stage: "execute".to_owned(),
                    completed: 7,
                    total: Some(8),
                },
                1_003,
            )
            .await
            .expect("coalesced progress");
        assert_eq!(first_progress.sequence, coalesced.sequence);
        assert_eq!(
            coordinator
                .events_after(&first.query_id, 0)
                .await
                .unwrap()
                .len(),
            2
        );

        coordinator
            .terminal(
                &first.query_id,
                QueryTerminalState::Failed,
                Some("EXPECTED_TEST_FAILURE".to_owned()),
                None,
                1_004,
            )
            .await
            .expect("first terminal");
        let permit = tokio::time::timeout(
            Duration::from_secs(1),
            coordinator.await_running(&second.query_id),
        )
        .await
        .expect("second dispatch")
        .expect("second running");
        assert_eq!(permit.query_id(), second.query_id);
        coordinator
            .append_event(
                &second.query_id,
                QueryControlEventPayload::ResultReady {
                    manifest_path: "packages/result/manifest.json".to_owned(),
                    manifest_checksum: "b3:test".to_owned(),
                    total_rows: 8,
                    total_pages: 2,
                    total_bytes: 96,
                },
                1_005,
            )
            .await
            .expect("result event");
        coordinator
            .terminal(
                &second.query_id,
                QueryTerminalState::Succeeded,
                None,
                Some((96, 2)),
                1_006,
            )
            .await
            .expect("success terminal");
        permit.complete();
        coordinator
            .release_result(&second.query_id)
            .await
            .expect("release result reservation");
    }

    #[tokio::test]
    async fn wp36_neg_capacity_canonicality_terminal_and_materialization_bypasses_fail_closed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let coordinator = coordinator(&temp, policy(1, 128, 8), 9, 1_000);
        let first = acceptance(
            coordinator
                .accept(operation("capacity", 1), 1_000)
                .await
                .expect("consume capacity"),
        );
        assert!(matches!(
            coordinator.accept(operation("overflow", 2), 1_000).await,
            Err(QueryCoordinatorError::ResultCapacityBackpressure)
        ));
        let mut noncanonical = operation("bad-json", 3);
        noncanonical.canonical_request = Arc::from(br#"{ "form": "definition" }"#.as_slice());
        assert!(matches!(
            coordinator.accept(noncanonical, 1_000).await,
            Err(QueryCoordinatorError::NonCanonicalRequest)
        ));
        assert!(matches!(
            coordinator
                .append_event(
                    &first.query_id,
                    QueryControlEventPayload::Terminal {
                        state: QueryTerminalState::Succeeded,
                        public_code: None,
                    },
                    1_001,
                )
                .await,
            Err(QueryCoordinatorError::TerminalRequiresClosure)
        ));
        assert!(matches!(
            coordinator
                .terminal(
                    &first.query_id,
                    QueryTerminalState::Succeeded,
                    None,
                    Some((129, 1)),
                    1_002,
                )
                .await,
            Err(QueryCoordinatorError::ResultReservationExceeded)
        ));
    }

    #[tokio::test]
    async fn wp36_ops_restart_marks_unsealed_lost_without_rerun_and_invalidates_cursors() {
        let temp = tempfile::tempdir().expect("tempdir");
        let base_policy = policy(1, 256, 16);
        let restart_operation = operation("restart", 1);
        let principal = restart_operation.principal_id;
        let (query_id, cursor) = {
            let coordinator = coordinator(&temp, base_policy, 10, 1_000);
            let accepted = acceptance(
                coordinator
                    .accept(restart_operation.clone(), 1_000)
                    .await
                    .expect("accept before restart"),
            );
            coordinator
                .append_event(
                    &accepted.query_id,
                    QueryControlEventPayload::SnapshotPinned {
                        epoch_id: "epoch:old".to_owned(),
                        source_generation: 1,
                        activation_head: 2,
                        lifecycle_watermark: 3,
                    },
                    1_001,
                )
                .await
                .expect("durable event");
            let cursor = coordinator
                .mint_cursor(&accepted.query_id, 1, 9_000)
                .await
                .expect("old generation cursor");
            (accepted.query_id, cursor)
        };

        let restarted = coordinator(&temp, base_policy, 11, 1_100);
        let replay = acceptance(
            restarted
                .accept(restart_operation, 1_101)
                .await
                .expect("terminal replay"),
        );
        assert_eq!(
            replay.phase,
            QueryExecutionPhase::Terminal(QueryTerminalState::Lost)
        );
        let events = restarted.events_after(&query_id, 0).await.expect("events");
        assert!(matches!(
            events.last().map(|event| &event.payload),
            Some(QueryControlEventPayload::Terminal {
                state: QueryTerminalState::Lost,
                ..
            })
        ));
        assert!(matches!(
            restarted.verify_cursor(&cursor, principal, 1_101).await,
            Err(QueryCoordinatorError::CursorBinding)
        ));

        let new_query = acceptance(
            restarted
                .accept(operation("after-restart", 2), 1_101)
                .await
                .expect("lost work released its reservation"),
        );
        restarted
            .cancel(&new_query.query_id, 1_102)
            .await
            .expect("running cancellation signal");
        assert!(
            restarted
                .cancellation(&new_query.query_id)
                .await
                .expect("cancellation state")
                .load(Ordering::Acquire)
        );
        restarted
            .terminal(
                &new_query.query_id,
                QueryTerminalState::Cancelled,
                Some("CANCELLED".to_owned()),
                None,
                1_103,
            )
            .await
            .expect("running cancellation terminal");
        assert_eq!(restarted.collect_expired(10_000).await.unwrap(), 2);
    }
}
