//! Daemon-wide bounded query admission, idempotency, events, cancellation, and restart state.
//!
//! The coordinator is the sole mutable authority for accepted semantic operations. It reserves
//! queue/task/journal/result capacity before acceptance, binds idempotency to the full normalized
//! operation, coalesces progress before allocating event sequence, and persists only control
//! records. Arrow response bytes live exclusively in manifest-last result packages.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
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
use super::streamed_result_package::PendingResultObjectSet;

const MAX_CANCELLATION_IDENTITIES_PER_QUERY: usize = 64;

/// Closed sharing policy for accepted query authority across authenticated daemon sessions.
///
/// The current release permits reconnect only for a freshly authorized session belonging to the
/// same principal and exact policy/revocation generations. A future broader sharing policy must
/// add a new explicit variant and update every authorization match; unknown durable values fail
/// deserialization rather than silently widening access.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QuerySessionSharingClass {
    PrincipalBound,
}

/// Current authenticated session authority presented to query/cursor/reissue authorization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuerySessionAuthority {
    principal_id: PrincipalId,
    policy_generation: u64,
    revocation_generation: u64,
    session_sharing_class: QuerySessionSharingClass,
}

impl QuerySessionAuthority {
    pub fn try_new(
        principal_id: PrincipalId,
        policy_generation: u64,
        revocation_generation: u64,
        session_sharing_class: QuerySessionSharingClass,
    ) -> Result<Self, QueryCoordinatorError> {
        if principal_id.as_bytes().iter().all(|byte| *byte == 0)
            || policy_generation == 0
            || revocation_generation == 0
        {
            return Err(QueryCoordinatorError::InvalidSessionAuthority);
        }
        Ok(Self {
            principal_id,
            policy_generation,
            revocation_generation,
            session_sharing_class,
        })
    }
}

/// Every field that can change execution, delivery, freshness, or retention meaning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedQueryOperation {
    pub workspace_id: WorkspaceId,
    pub principal_id: PrincipalId,
    pub policy_generation: u64,
    pub revocation_generation: u64,
    pub session_sharing_class: QuerySessionSharingClass,
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
            || operation.policy_generation == 0
            || operation.revocation_generation == 0
        {
            return Err(QueryCoordinatorError::InvalidSessionAuthority);
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
        frame(&mut hasher, b"codefabric.normalized-query-operation.v2");
        frame(&mut hasher, self.workspace_id.as_bytes());
        frame(&mut hasher, self.principal_id.as_bytes());
        frame(&mut hasher, &self.policy_generation.to_be_bytes());
        frame(&mut hasher, &self.revocation_generation.to_be_bytes());
        frame(
            &mut hasher,
            match self.session_sharing_class {
                QuerySessionSharingClass::PrincipalBound => b"principal-bound",
            },
        );
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

    fn authorizes_session(&self, authority: QuerySessionAuthority) -> bool {
        self.policy_generation == authority.policy_generation
            && self.revocation_generation == authority.revocation_generation
            && self.session_sharing_class == authority.session_sharing_class
            && match self.session_sharing_class {
                QuerySessionSharingClass::PrincipalBound => {
                    self.principal_id == authority.principal_id
                }
            }
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
#[serde(deny_unknown_fields)]
pub struct RetainedPackageLocator {
    pub manifest_object_path: String,
    pub page_object_paths: Vec<String>,
    pub epoch_id: String,
    pub query_execution: String,
    pub lease_id: String,
    pub lease_issued_at_unix_ms: i64,
    pub lease_expires_at_unix_ms: i64,
    pub expected_manifest_checksum: String,
    pub expected_manifest_byte_length: u64,
    pub package_id: String,
    pub manifest_resource_id: String,
}

/// Durable ownership state for one accepted query's bounded result reservation and object set.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ResultRetentionState {
    Reserved,
    CleanupPending,
    Released,
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
    /// Exact private object set durably declared before the first page write.
    PublicationPending { object_set: PendingResultObjectSet },
    ResultReady {
        package_id: String,
        manifest_resource_id: String,
        manifest_checksum: String,
        total_rows: u64,
        total_pages: u64,
        total_bytes: u64,
        retained_locator: RetainedPackageLocator,
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

    const fn is_public(&self) -> bool {
        !matches!(self, Self::PublicationPending { .. })
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

fn public_events_after(
    events: &[QueryControlEvent],
    after_sequence: u64,
) -> Result<Vec<QueryControlEvent>, QueryCoordinatorError> {
    let mut public_sequence = 0_u64;
    let mut suffix = Vec::new();
    for event in events.iter().filter(|event| event.payload.is_public()) {
        public_sequence = public_sequence
            .checked_add(1)
            .ok_or(QueryCoordinatorError::CounterOverflow)?;
        if public_sequence > after_sequence {
            let mut projected = event.clone();
            projected.sequence = public_sequence;
            suffix.push(projected);
        }
    }
    Ok(suffix)
}

fn public_event_at(
    events: &[QueryControlEvent],
    public_sequence: u64,
) -> Result<Option<QueryControlEvent>, QueryCoordinatorError> {
    if public_sequence == 0 {
        return Ok(None);
    }
    let mut ordinal = 0_u64;
    for event in events.iter().filter(|event| event.payload.is_public()) {
        ordinal = ordinal
            .checked_add(1)
            .ok_or(QueryCoordinatorError::CounterOverflow)?;
        if ordinal == public_sequence {
            let mut projected = event.clone();
            projected.sequence = public_sequence;
            return Ok(Some(projected));
        }
    }
    Ok(None)
}

fn public_event_checksum(
    events: &[QueryControlEvent],
    public_sequence: u64,
) -> Result<Option<String>, QueryCoordinatorError> {
    match public_event_at(events, public_sequence)? {
        None if public_sequence == 0 => Ok(None),
        None => Err(QueryCoordinatorError::CursorBinding),
        Some(event) => serde_json_canonicalizer::to_vec(&event)
            .map(|bytes| Some(hex(blake3::hash(&bytes).as_bytes())))
            .map_err(QueryCoordinatorError::JournalEncoding),
    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueryCoordinatorSnapshot {
    pub running: usize,
    pub queued: usize,
    pub accepted: usize,
    pub reserved_result_bytes: u64,
    pub reserved_result_pages: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueryAcceptanceOutcome {
    New(QueryAcceptance),
    Replay(QueryAcceptance),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueryCancellationOutcome {
    pub phase: QueryExecutionPhase,
    pub idempotent_replay: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DurableQueryRecord {
    acceptance: QueryAcceptance,
    workspace_id: String,
    principal_id: String,
    policy_generation: u64,
    revocation_generation: u64,
    session_sharing_class: QuerySessionSharingClass,
    idempotency_key: String,
    operation_fingerprint: String,
    semantic_profile: String,
    reserved_result_bytes: u64,
    reserved_result_pages: u64,
    cancellation_ids: BTreeSet<String>,
    result_retention: ResultRetentionState,
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
    cancellation_ids: BTreeSet<String>,
    events: Vec<QueryControlEvent>,
    event_bytes: usize,
    result_retention: ResultRetentionState,
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
    task_reservations: BTreeSet<String>,
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
            task_reservations: BTreeSet::new(),
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
    /// The enforced running-query ceiling for one authenticated principal.
    #[must_use]
    pub(crate) fn maximum_running_queries_per_principal(&self) -> usize {
        self.policy.max_running_per_principal.get()
    }

    /// The enforced control-event retention ceiling for one query.
    #[must_use]
    pub(crate) fn maximum_events_per_query(&self) -> usize {
        self.policy.max_events_per_query.get()
    }

    /// The enforced daemon-wide task reservation ceiling.
    #[must_use]
    pub(crate) fn maximum_tasks(&self) -> usize {
        self.policy.max_tasks.get()
    }

    /// Cheap control-only observation used by status and admission diagnostics.
    pub async fn snapshot(&self) -> QueryCoordinatorSnapshot {
        let state = self.state.lock().await;
        QueryCoordinatorSnapshot {
            running: state.running,
            queued: state.queue.len(),
            accepted: state.handles.len(),
            reserved_result_bytes: state.reserved_result_bytes,
            reserved_result_pages: state.reserved_result_pages,
        }
    }

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
            let workspace_id = WorkspaceId::from_bytes(decode_hex16(&durable.workspace_id)?);
            let principal_id = PrincipalId::from_bytes(decode_hex16(&durable.principal_id)?);
            QuerySessionAuthority::try_new(
                principal_id,
                durable.policy_generation,
                durable.revocation_generation,
                durable.session_sharing_class,
            )?;
            let fingerprint = decode_hex32(&durable.operation_fingerprint)?;
            validate_cancellation_ids(&durable.cancellation_ids)?;
            for event in &durable.events {
                validate_result_ready_payload(&event.payload)?;
                if let QueryControlEventPayload::PublicationPending { object_set } = &event.payload
                {
                    validate_pending_result_object_set(object_set)?;
                }
            }
            validate_result_retention(&durable)?;
            let retained_object_set = cleanup_object_set(&durable.events).is_some();
            let mut durable_changed = false;
            if !matches!(durable.acceptance.phase, QueryExecutionPhase::Terminal(_)) {
                durable.acceptance.phase = QueryExecutionPhase::Terminal(QueryTerminalState::Lost);
                durable.result_retention = if retained_object_set {
                    ResultRetentionState::CleanupPending
                } else {
                    ResultRetentionState::Released
                };
                let sequence = next_sequence(&durable.events)?;
                durable.events.push(QueryControlEvent {
                    sequence,
                    emitted_at_unix_ms: observed_at_unix_ms,
                    payload: QueryControlEventPayload::Terminal {
                        state: QueryTerminalState::Lost,
                        public_code: Some("QUERY_LOST_DURING_RESTART".to_owned()),
                    },
                });
                durable_changed = true;
            }
            if durable.acceptance.lease_expires_at_unix_ms <= observed_at_unix_ms {
                if retained_object_set && durable.result_retention != ResultRetentionState::Released
                {
                    durable.result_retention = ResultRetentionState::CleanupPending;
                    durable_changed = true;
                } else {
                    journal.delete(&durable.acceptance.query_id)?;
                    continue;
                }
            }
            validate_result_retention(&durable)?;
            if durable_changed {
                journal.replace(&durable)?;
            }
            let scope = IdempotencyScope {
                workspace_id,
                principal_id,
                key: Arc::from(durable.idempotency_key.as_str()),
            };
            if durable.result_retention == ResultRetentionState::Reserved {
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
                policy_generation: durable.policy_generation,
                revocation_generation: durable.revocation_generation,
                session_sharing_class: durable.session_sharing_class,
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
                    cancellation_ids: durable.cancellation_ids,
                    events: durable.events,
                    event_bytes,
                    result_retention: durable.result_retention,
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
            || state.task_reservations.len() >= self.policy.max_tasks.get()
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
            cancellation_ids: BTreeSet::new(),
            events: Vec::with_capacity(self.policy.max_events_per_query.get()),
            event_bytes: 0,
            result_retention: ResultRetentionState::Reserved,
            changed: Arc::new(Notify::new()),
        };
        self.journal.create(&durable_record(&handle))?;
        state.reserved_result_bytes = next_bytes;
        state.reserved_result_pages = next_pages;
        state.idempotency.insert(scope, query_id.clone());
        state.queue.push_back(query_id.clone());
        state.task_reservations.insert(query_id.clone());
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
        if !state.task_reservations.contains(query_id) || state.tasks.contains_key(query_id) {
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
        validate_result_ready_payload(&payload)?;
        if let QueryControlEventPayload::PublicationPending { object_set } = &payload {
            validate_pending_result_object_set(object_set)?;
        }
        let mut state = self.state.lock().await;
        let handle = state
            .handles
            .get_mut(query_id)
            .ok_or_else(|| QueryCoordinatorError::UnknownQuery(query_id.to_owned()))?;
        if matches!(handle.acceptance.phase, QueryExecutionPhase::Terminal(_)) {
            return Err(QueryCoordinatorError::AlreadyTerminal);
        }
        match &payload {
            QueryControlEventPayload::PublicationPending { object_set } => {
                if cleanup_object_set(&handle.events).is_some() {
                    return Err(QueryCoordinatorError::PublicationIntentConflict);
                }
                validate_pending_result_object_set(object_set)?;
            }
            QueryControlEventPayload::ResultReady {
                retained_locator: locator,
                ..
            } => {
                let expected = publication_intent(&handle.events)
                    .ok_or(QueryCoordinatorError::PublicationIntentMissing)?;
                if expected != pending_object_set_from_locator(locator)
                    || locator.page_object_paths.is_empty()
                {
                    return Err(QueryCoordinatorError::PublicationIntentConflict);
                }
                if retained_locator(&handle.events).is_some() {
                    return Err(QueryCoordinatorError::PublicationIntentConflict);
                }
            }
            _ => {}
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
                if retained_locator(&handle.events).is_none() {
                    return Err(QueryCoordinatorError::ResultMissing);
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
                handle.result_retention = if cleanup_object_set(&handle.events).is_some() {
                    ResultRetentionState::CleanupPending
                } else {
                    ResultRetentionState::Released
                };
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
        state.task_reservations.remove(query_id);
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

    /// Apply one bounded cancellation identity exactly once and return the observed phase.
    pub async fn cancel_idempotent(
        &self,
        query_id: &str,
        cancellation_id: &str,
        observed_at_unix_ms: i64,
    ) -> Result<QueryCancellationOutcome, QueryCoordinatorError> {
        if !valid_cancellation_id(cancellation_id) {
            return Err(QueryCoordinatorError::InvalidCancellationId);
        }
        let mut state = self.state.lock().await;
        let (notify, reserved_bytes, reserved_pages) = {
            let handle = state
                .handles
                .get_mut(query_id)
                .ok_or_else(|| QueryCoordinatorError::UnknownQuery(query_id.to_owned()))?;
            if handle.cancellation_ids.contains(cancellation_id) {
                return Ok(QueryCancellationOutcome {
                    phase: handle.acceptance.phase,
                    idempotent_replay: true,
                });
            }
            if handle.cancellation_ids.len() >= MAX_CANCELLATION_IDENTITIES_PER_QUERY {
                return Err(QueryCoordinatorError::CancellationCapacity);
            }
            match handle.acceptance.phase {
                phase @ QueryExecutionPhase::Terminal(_) => {
                    handle.cancellation_ids.insert(cancellation_id.to_owned());
                    if let Err(error) = self.journal.replace(&durable_record(handle)) {
                        handle.cancellation_ids.remove(cancellation_id);
                        return Err(error);
                    }
                    return Ok(QueryCancellationOutcome {
                        phase,
                        idempotent_replay: false,
                    });
                }
                phase @ QueryExecutionPhase::Running => {
                    handle.cancellation_ids.insert(cancellation_id.to_owned());
                    if let Err(error) = self.journal.replace(&durable_record(handle)) {
                        handle.cancellation_ids.remove(cancellation_id);
                        return Err(error);
                    }
                    // Persist the identity before the process-local side effect. Loss of the
                    // eventual RPC acknowledgement therefore replays after daemon recovery.
                    handle.cancelled.store(true, Ordering::Release);
                    return Ok(QueryCancellationOutcome {
                        phase,
                        idempotent_replay: false,
                    });
                }
                QueryExecutionPhase::Queued => {}
            }

            let sequence = next_sequence(&handle.events)?;
            let event = QueryControlEvent {
                sequence,
                emitted_at_unix_ms: observed_at_unix_ms,
                payload: QueryControlEventPayload::Terminal {
                    state: QueryTerminalState::Cancelled,
                    public_code: Some("CANCELLED".to_owned()),
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
            let prior_event_bytes = handle.event_bytes;
            let prior_retention = handle.result_retention;
            handle.cancellation_ids.insert(cancellation_id.to_owned());
            handle.events.push(event);
            handle.event_bytes = next_bytes;
            handle.acceptance.phase = QueryExecutionPhase::Terminal(QueryTerminalState::Cancelled);
            handle.result_retention = if cleanup_object_set(&handle.events).is_some() {
                ResultRetentionState::CleanupPending
            } else {
                ResultRetentionState::Released
            };
            if let Err(error) = self.journal.replace(&durable_record(handle)) {
                handle.events.pop();
                handle.event_bytes = prior_event_bytes;
                handle.acceptance.phase = QueryExecutionPhase::Queued;
                handle.result_retention = prior_retention;
                handle.cancellation_ids.remove(cancellation_id);
                return Err(error);
            }
            handle.cancelled.store(true, Ordering::Release);
            (
                Arc::clone(&handle.changed),
                handle.operation.maximum_result_bytes,
                handle.operation.maximum_result_pages,
            )
        };
        state.queue.retain(|queued| queued != query_id);
        release_result_reservation(&mut state, reserved_bytes, reserved_pages)?;
        state.tasks.remove(query_id);
        state.task_reservations.remove(query_id);
        self.dispatch_locked(&mut state)?;
        drop(state);
        notify.notify_waiters();
        Ok(QueryCancellationOutcome {
            phase: QueryExecutionPhase::Terminal(QueryTerminalState::Cancelled),
            idempotent_replay: false,
        })
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
        public_events_after(&handle.events, after_sequence)
    }

    /// Reauthorize a query handle against the coordinator's original accepted principal.
    pub async fn authorize_query(
        &self,
        query_id: &str,
        authority: QuerySessionAuthority,
    ) -> Result<WorkspaceId, QueryCoordinatorError> {
        let state = self.state.lock().await;
        let handle = state
            .handles
            .get(query_id)
            .ok_or_else(|| QueryCoordinatorError::UnknownQuery(query_id.to_owned()))?;
        if !handle.operation.authorizes_session(authority) {
            return Err(QueryCoordinatorError::QueryOwnerMismatch);
        }
        Ok(handle.operation.workspace_id)
    }

    /// Authorize reconstruction of one retained succeeded result after process-local handles were
    /// lost. Explicit release, expiry, failure, cancellation, and a different principal all deny
    /// reissue before the private locator is consumed.
    pub async fn authorize_retained_result_reissue(
        &self,
        query_id: &str,
        authority: QuerySessionAuthority,
    ) -> Result<WorkspaceId, QueryCoordinatorError> {
        let state = self.state.lock().await;
        let handle = state
            .handles
            .get(query_id)
            .ok_or_else(|| QueryCoordinatorError::UnknownQuery(query_id.to_owned()))?;
        if !handle.operation.authorizes_session(authority) {
            return Err(QueryCoordinatorError::QueryOwnerMismatch);
        }
        if handle.acceptance.phase != QueryExecutionPhase::Terminal(QueryTerminalState::Succeeded)
            || handle.result_retention != ResultRetentionState::Reserved
        {
            return Err(QueryCoordinatorError::ResultNotReleasable);
        }
        Ok(handle.operation.workspace_id)
    }

    /// Return the current accepted execution phase without exposing the mutable handle.
    pub async fn phase(
        &self,
        query_id: &str,
    ) -> Result<QueryExecutionPhase, QueryCoordinatorError> {
        self.state
            .lock()
            .await
            .handles
            .get(query_id)
            .map(|handle| handle.acceptance.phase)
            .ok_or_else(|| QueryCoordinatorError::UnknownQuery(query_id.to_owned()))
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
        let event_checksum = public_event_checksum(&handle.events, after_sequence)?;
        let payload = CursorPayload {
            query_id: query_id.to_owned(),
            principal_id: hex(handle.operation.principal_id.as_bytes()),
            generation: self.generation,
            policy_generation: handle.operation.policy_generation,
            revocation_generation: handle.operation.revocation_generation,
            session_sharing_class: handle.operation.session_sharing_class,
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
        authority: QuerySessionAuthority,
        observed_at_unix_ms: i64,
    ) -> Result<(String, u64), QueryCoordinatorError> {
        let payload = decode_cursor(cursor, &self.cursor_secret)?;
        if payload.generation != self.generation
            || payload.principal_id != hex(authority.principal_id.as_bytes())
            || payload.policy_generation != authority.policy_generation
            || payload.revocation_generation != authority.revocation_generation
            || payload.session_sharing_class != authority.session_sharing_class
            || payload.expires_at_unix_ms <= observed_at_unix_ms
        {
            return Err(QueryCoordinatorError::CursorBinding);
        }
        let state = self.state.lock().await;
        let handle = state
            .handles
            .get(&payload.query_id)
            .ok_or_else(|| QueryCoordinatorError::UnknownQuery(payload.query_id.clone()))?;
        if !handle.operation.authorizes_session(authority)
            || payload.policy_generation != handle.operation.policy_generation
            || payload.revocation_generation != handle.operation.revocation_generation
            || payload.session_sharing_class != handle.operation.session_sharing_class
            || payload.semantic_profile.as_str() != handle.operation.semantic_profile.as_ref()
        {
            return Err(QueryCoordinatorError::CursorBinding);
        }
        let expected_checksum = public_event_checksum(&handle.events, payload.after_sequence)?;
        if payload.event_checksum != expected_checksum {
            return Err(QueryCoordinatorError::CursorBinding);
        }
        Ok((payload.query_id, payload.after_sequence))
    }

    /// Durably revoke reissue before any object cleanup begins.
    pub async fn release_result(&self, query_id: &str) -> Result<(), QueryCoordinatorError> {
        let mut state = self.state.lock().await;
        let (phase, retention, reserved_bytes, reserved_pages) = {
            let handle = state
                .handles
                .get(query_id)
                .ok_or_else(|| QueryCoordinatorError::UnknownQuery(query_id.to_owned()))?;
            (
                handle.acceptance.phase,
                handle.result_retention,
                handle.operation.maximum_result_bytes,
                handle.operation.maximum_result_pages,
            )
        };
        if phase != QueryExecutionPhase::Terminal(QueryTerminalState::Succeeded) {
            return Err(QueryCoordinatorError::ResultNotReleasable);
        }
        if retention != ResultRetentionState::Reserved {
            return Ok(());
        }
        {
            let handle = state
                .handles
                .get_mut(query_id)
                .ok_or(QueryCoordinatorError::CoordinatorState)?;
            handle.result_retention = ResultRetentionState::CleanupPending;
            if let Err(error) = self.journal.replace(&durable_record(handle)) {
                handle.result_retention = ResultRetentionState::Reserved;
                return Err(error);
            }
        }
        release_result_reservation(&mut state, reserved_bytes, reserved_pages)
    }

    /// Return the exact private object locators whose durable reissue revocation won but whose
    /// idempotent object cleanup has not yet been durably acknowledged.
    pub async fn pending_result_cleanups(&self) -> Vec<(String, PendingResultObjectSet)> {
        self.state
            .lock()
            .await
            .handles
            .iter()
            .filter_map(|(query_id, handle)| {
                (handle.result_retention == ResultRetentionState::CleanupPending)
                    .then(|| {
                        cleanup_object_set(&handle.events).map(|value| (query_id.clone(), value))
                    })
                    .flatten()
            })
            .collect()
    }

    /// Durably acknowledge that every locator-bound object was deleted or already absent.
    pub async fn mark_result_cleanup_complete(
        &self,
        query_id: &str,
    ) -> Result<(), QueryCoordinatorError> {
        let mut state = self.state.lock().await;
        let handle = state
            .handles
            .get_mut(query_id)
            .ok_or_else(|| QueryCoordinatorError::UnknownQuery(query_id.to_owned()))?;
        match handle.result_retention {
            ResultRetentionState::Released => Ok(()),
            ResultRetentionState::Reserved => Err(QueryCoordinatorError::ResultNotReleasable),
            ResultRetentionState::CleanupPending => {
                handle.result_retention = ResultRetentionState::Released;
                if let Err(error) = self.journal.replace(&durable_record(handle)) {
                    handle.result_retention = ResultRetentionState::CleanupPending;
                    return Err(error);
                }
                Ok(())
            }
        }
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
                (handle.acceptance.lease_expires_at_unix_ms <= observed_at_unix_ms
                    && handle.result_retention != ResultRetentionState::CleanupPending)
                    .then(|| query_id.clone())
            })
            .collect::<Vec<_>>();
        for query_id in &expired {
            let stage_result_cleanup = state.handles.get(query_id).is_some_and(|handle| {
                handle.result_retention == ResultRetentionState::Reserved
                    && cleanup_object_set(&handle.events).is_some()
            });
            if stage_result_cleanup {
                let (workspace, principal, prior_phase, notify, reserved_bytes, reserved_pages) = {
                    let handle = state
                        .handles
                        .get_mut(query_id)
                        .ok_or(QueryCoordinatorError::CoordinatorState)?;
                    let prior_phase = handle.acceptance.phase;
                    if !matches!(prior_phase, QueryExecutionPhase::Terminal(_)) {
                        let event = QueryControlEvent {
                            sequence: next_sequence(&handle.events)?,
                            emitted_at_unix_ms: observed_at_unix_ms,
                            payload: QueryControlEventPayload::Terminal {
                                state: QueryTerminalState::Lost,
                                public_code: Some("QUERY_EXPIRED".to_owned()),
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
                        handle.events.push(event);
                        handle.event_bytes = next_bytes;
                        handle.acceptance.phase =
                            QueryExecutionPhase::Terminal(QueryTerminalState::Lost);
                    }
                    handle.result_retention = ResultRetentionState::CleanupPending;
                    self.journal.replace(&durable_record(handle))?;
                    (
                        handle.operation.workspace_id,
                        handle.operation.principal_id,
                        prior_phase,
                        Arc::clone(&handle.changed),
                        handle.operation.maximum_result_bytes,
                        handle.operation.maximum_result_pages,
                    )
                };
                if prior_phase == QueryExecutionPhase::Running {
                    release_running(&mut state, workspace, principal)?;
                } else if prior_phase == QueryExecutionPhase::Queued {
                    state.queue.retain(|queued| queued != query_id);
                }
                release_result_reservation(&mut state, reserved_bytes, reserved_pages)?;
                state.tasks.remove(query_id).inspect(|task| task.abort());
                state.task_reservations.remove(query_id);
                notify.notify_waiters();
                continue;
            }
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
            if handle.result_retention == ResultRetentionState::Reserved {
                release_result_reservation(
                    &mut state,
                    handle.operation.maximum_result_bytes,
                    handle.operation.maximum_result_pages,
                )?;
            }
            state.tasks.remove(query_id).inspect(|task| task.abort());
            state.task_reservations.remove(query_id);
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
    policy_generation: u64,
    revocation_generation: u64,
    session_sharing_class: QuerySessionSharingClass,
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
        policy_generation: handle.operation.policy_generation,
        revocation_generation: handle.operation.revocation_generation,
        session_sharing_class: handle.operation.session_sharing_class,
        idempotency_key: handle.operation.idempotency_key.to_string(),
        operation_fingerprint: hex(&handle.fingerprint),
        semantic_profile: handle.operation.semantic_profile.to_string(),
        reserved_result_bytes: handle.operation.maximum_result_bytes,
        reserved_result_pages: handle.operation.maximum_result_pages,
        cancellation_ids: handle.cancellation_ids.clone(),
        result_retention: handle.result_retention,
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

fn validate_result_ready_payload(
    payload: &QueryControlEventPayload,
) -> Result<(), QueryCoordinatorError> {
    let QueryControlEventPayload::ResultReady {
        package_id,
        manifest_resource_id,
        manifest_checksum,
        total_pages,
        total_bytes,
        retained_locator,
        ..
    } = payload
    else {
        return Ok(());
    };
    let object_set = pending_object_set_from_locator(retained_locator);
    validate_pending_result_object_set(&object_set)?;
    let b3 = |value: &str| {
        value
            .strip_prefix("b3:")
            .is_some_and(|digest| decode_hex32(digest).is_ok())
    };
    if usize::try_from(*total_pages).ok() != Some(retained_locator.page_object_paths.len())
        || decode_hex16(&retained_locator.lease_id).is_err()
        || retained_locator.lease_expires_at_unix_ms <= retained_locator.lease_issued_at_unix_ms
        || retained_locator.expected_manifest_byte_length == 0
        || !b3(&retained_locator.expected_manifest_checksum)
        || !b3(package_id)
        || !b3(manifest_resource_id)
        || !b3(manifest_checksum)
        || retained_locator.package_id != *package_id
        || retained_locator.manifest_resource_id != *manifest_resource_id
        || retained_locator.expected_manifest_checksum != *manifest_checksum
        || *total_pages == 0
        || *total_bytes == 0
    {
        return Err(QueryCoordinatorError::InvalidRetainedPackageLocator);
    }
    Ok(())
}

fn validate_pending_result_object_set(
    object_set: &PendingResultObjectSet,
) -> Result<(), QueryCoordinatorError> {
    let package_root = format!(
        "packages/{}/{}",
        object_set.epoch_id, object_set.query_execution
    );
    let expected_manifest_path = format!("{package_root}/manifest.json");
    let page_prefix = format!("{package_root}/pages/");
    let page_paths = &object_set.page_object_paths;
    let valid_pages = !page_paths.is_empty()
        && page_paths.len() <= 1_024
        && page_paths.iter().all(|page| {
            page.is_ascii()
                && page.len() <= 1_024
                && page.strip_prefix(&page_prefix).is_some_and(|relative| {
                    let Some((relation_hex, page_file)) = relative.split_once('/') else {
                        return false;
                    };
                    !relation_hex.is_empty()
                        && relation_hex.len() % 2 == 0
                        && relation_hex.bytes().all(|byte| byte.is_ascii_hexdigit())
                        && page_file.len() == 26
                        && page_file[..20].bytes().all(|byte| byte.is_ascii_digit())
                        && page_file.ends_with(".arrow")
                })
        })
        && page_paths.iter().collect::<BTreeSet<_>>().len() == page_paths.len()
        && !page_paths
            .iter()
            .any(|page| page == &object_set.manifest_object_path);
    if object_set.manifest_object_path != expected_manifest_path
        || !valid_pages
        || decode_hex16(&object_set.epoch_id).is_err()
        || decode_hex32(&object_set.query_execution).is_err()
    {
        return Err(QueryCoordinatorError::InvalidRetainedPackageLocator);
    }
    Ok(())
}

fn pending_object_set_from_locator(locator: &RetainedPackageLocator) -> PendingResultObjectSet {
    PendingResultObjectSet {
        manifest_object_path: locator.manifest_object_path.clone(),
        page_object_paths: locator.page_object_paths.clone(),
        epoch_id: locator.epoch_id.clone(),
        query_execution: locator.query_execution.clone(),
    }
}

fn publication_intent(events: &[QueryControlEvent]) -> Option<PendingResultObjectSet> {
    events.iter().rev().find_map(|event| match &event.payload {
        QueryControlEventPayload::PublicationPending { object_set } => Some(object_set.clone()),
        _ => None,
    })
}

fn cleanup_object_set(events: &[QueryControlEvent]) -> Option<PendingResultObjectSet> {
    events.iter().rev().find_map(|event| match &event.payload {
        QueryControlEventPayload::ResultReady {
            retained_locator, ..
        } => Some(pending_object_set_from_locator(retained_locator)),
        QueryControlEventPayload::PublicationPending { object_set } => Some(object_set.clone()),
        _ => None,
    })
}

fn retained_locator(events: &[QueryControlEvent]) -> Option<RetainedPackageLocator> {
    events.iter().rev().find_map(|event| match &event.payload {
        QueryControlEventPayload::ResultReady {
            retained_locator, ..
        } => Some(retained_locator.clone()),
        _ => None,
    })
}

fn validate_result_retention(record: &DurableQueryRecord) -> Result<(), QueryCoordinatorError> {
    let terminal = matches!(record.acceptance.phase, QueryExecutionPhase::Terminal(_));
    let succeeded =
        record.acceptance.phase == QueryExecutionPhase::Terminal(QueryTerminalState::Succeeded);
    let locator = retained_locator(&record.events);
    let cleanup = cleanup_object_set(&record.events);
    let valid = match record.result_retention {
        ResultRetentionState::Reserved => !terminal || (succeeded && locator.is_some()),
        ResultRetentionState::CleanupPending => terminal && cleanup.is_some(),
        ResultRetentionState::Released => terminal,
    };
    if !valid {
        return Err(QueryCoordinatorError::InvalidResultRetentionState);
    }
    Ok(())
}

fn valid_cancellation_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && value.is_ascii()
}

fn validate_cancellation_ids(
    cancellation_ids: &BTreeSet<String>,
) -> Result<(), QueryCoordinatorError> {
    if cancellation_ids.len() > MAX_CANCELLATION_IDENTITIES_PER_QUERY {
        return Err(QueryCoordinatorError::CancellationCapacity);
    }
    if cancellation_ids
        .iter()
        .any(|cancellation_id| !valid_cancellation_id(cancellation_id))
    {
        return Err(QueryCoordinatorError::InvalidCancellationId);
    }
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
    #[error("invalid query session authority")]
    InvalidSessionAuthority,
    #[error("invalid idempotency key")]
    InvalidIdempotencyKey,
    #[error("invalid cancellation identity")]
    InvalidCancellationId,
    #[error("query cancellation identity capacity is exhausted")]
    CancellationCapacity,
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
    #[error("query belongs to another principal")]
    QueryOwnerMismatch,
    #[error("query identity collision")]
    QueryIdentityCollision,
    #[error("query result is missing")]
    ResultMissing,
    #[error("query result exceeded its accepted reservation")]
    ResultReservationExceeded,
    #[error("query result is not releasable")]
    ResultNotReleasable,
    #[error("durable result retention state is incoherent")]
    InvalidResultRetentionState,
    #[error("retained result package locator is invalid")]
    InvalidRetainedPackageLocator,
    #[error("result publication has no preceding durable object-set intent")]
    PublicationIntentMissing,
    #[error("result publication differs from its durable object-set intent")]
    PublicationIntentConflict,
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
            policy_generation: 7,
            revocation_generation: 3,
            session_sharing_class: QuerySessionSharingClass::PrincipalBound,
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

    fn authority(operation: &NormalizedQueryOperation) -> QuerySessionAuthority {
        QuerySessionAuthority::try_new(
            operation.principal_id,
            operation.policy_generation,
            operation.revocation_generation,
            operation.session_sharing_class,
        )
        .expect("valid session authority")
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

    fn retained_locator() -> RetainedPackageLocator {
        RetainedPackageLocator {
            manifest_object_path: format!(
                "packages/{}/{}/manifest.json",
                "44".repeat(16),
                "55".repeat(32)
            ),
            page_object_paths: (0..2)
                .map(|ordinal| {
                    format!(
                        "packages/{}/{}/pages/{}/{ordinal:020}.arrow",
                        "44".repeat(16),
                        "55".repeat(32),
                        "88".repeat(16)
                    )
                })
                .collect(),
            epoch_id: "44".repeat(16),
            query_execution: "55".repeat(32),
            lease_id: "66".repeat(16),
            lease_issued_at_unix_ms: 1_000,
            lease_expires_at_unix_ms: 9_000,
            expected_manifest_checksum: format!("b3:{}", "33".repeat(32)),
            expected_manifest_byte_length: 512,
            package_id: format!("b3:{}", "11".repeat(32)),
            manifest_resource_id: format!("b3:{}", "22".repeat(32)),
        }
    }

    fn pending_object_set(locator: &RetainedPackageLocator) -> PendingResultObjectSet {
        pending_object_set_from_locator(locator)
    }

    async fn append_result_ready(
        coordinator: &QueryCoordinator,
        query_id: &str,
        locator: RetainedPackageLocator,
        observed_at_unix_ms: i64,
    ) {
        coordinator
            .append_event(
                query_id,
                QueryControlEventPayload::PublicationPending {
                    object_set: pending_object_set(&locator),
                },
                observed_at_unix_ms,
            )
            .await
            .expect("journal publication intent before any object write");
        coordinator
            .append_event(
                query_id,
                QueryControlEventPayload::ResultReady {
                    package_id: locator.package_id.clone(),
                    manifest_resource_id: locator.manifest_resource_id.clone(),
                    manifest_checksum: locator.expected_manifest_checksum.clone(),
                    total_rows: 8,
                    total_pages: u64::try_from(locator.page_object_paths.len()).unwrap(),
                    total_bytes: 96,
                    retained_locator: locator,
                },
                observed_at_unix_ms.saturating_add(1),
            )
            .await
            .expect("journal sealed result");
    }

    #[tokio::test]
    async fn wp45_beh_private_journal_events_do_not_create_public_sequence_gaps() {
        let temp = tempfile::tempdir().expect("tempdir");
        let coordinator = coordinator(&temp, policy(1, 1_024, 64), 7, 1_000);
        let operation = operation("private-public-sequence", 1);
        let session_authority = authority(&operation);
        let accepted = acceptance(
            coordinator
                .accept(operation, 1_000)
                .await
                .expect("accept query"),
        );
        coordinator
            .append_event(
                &accepted.query_id,
                QueryControlEventPayload::SnapshotPinned {
                    epoch_id: "epoch:public-sequence".to_owned(),
                    source_generation: 1,
                    activation_head: 2,
                    lifecycle_watermark: 3,
                },
                1_001,
            )
            .await
            .expect("snapshot event");
        append_result_ready(&coordinator, &accepted.query_id, retained_locator(), 1_002).await;
        coordinator
            .terminal(
                &accepted.query_id,
                QueryTerminalState::Succeeded,
                None,
                Some((96, 2)),
                1_004,
            )
            .await
            .expect("terminal event");

        let public_events = coordinator
            .events_after(&accepted.query_id, 0)
            .await
            .expect("public event projection");
        assert_eq!(
            public_events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert!(matches!(
            public_events[1].payload,
            QueryControlEventPayload::ResultReady { .. }
        ));
        assert!(public_events.iter().all(|event| event.payload.is_public()));

        let cursor = coordinator
            .mint_cursor(&accepted.query_id, 2, 9_000)
            .await
            .expect("cursor bound to projected public event");
        assert_eq!(
            coordinator
                .verify_cursor(&cursor, session_authority, 1_005)
                .await
                .expect("verify public cursor"),
            (accepted.query_id.clone(), 2)
        );
        assert_eq!(
            coordinator
                .events_after(&accepted.query_id, 2)
                .await
                .expect("resume after public cursor")
                .into_iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![3]
        );
        assert!(matches!(
            coordinator.mint_cursor(&accepted.query_id, 4, 9_000).await,
            Err(QueryCoordinatorError::CursorBinding)
        ));
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
        value.policy_generation += 1;
        variants.push(value);
        let mut value = base.clone();
        value.revocation_generation += 1;
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
                .verify_cursor(&cursor, authority(&base), 1_004)
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
                    authority(&base),
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
        append_result_ready(&coordinator, &second.query_id, retained_locator(), 1_005).await;
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
        let session_authority = authority(&restart_operation);
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
            restarted
                .verify_cursor(&cursor, session_authority, 1_101)
                .await,
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

    #[tokio::test]
    async fn wp45_cancel_is_idempotent_for_queued_running_and_terminal_work() {
        let temp = tempfile::tempdir().expect("tempdir");
        let coordinator = coordinator(&temp, policy(1, 1_024, 64), 7, 1_000);
        let running = acceptance(
            coordinator
                .accept(operation("cancel-running", 1), 1_000)
                .await
                .expect("running acceptance"),
        );
        assert_eq!(running.phase, QueryExecutionPhase::Running);
        let queued = acceptance(
            coordinator
                .accept(operation("cancel-queued", 2), 1_001)
                .await
                .expect("queued acceptance"),
        );
        assert_eq!(queued.phase, QueryExecutionPhase::Queued);

        let cancelled = coordinator
            .cancel_idempotent(&queued.query_id, "cancel:queued", 1_002)
            .await
            .expect("queued cancellation");
        assert_eq!(
            cancelled.phase,
            QueryExecutionPhase::Terminal(QueryTerminalState::Cancelled)
        );
        assert!(!cancelled.idempotent_replay);
        let replay = coordinator
            .cancel_idempotent(&queued.query_id, "cancel:queued", 1_003)
            .await
            .expect("queued cancellation replay");
        assert_eq!(replay.phase, cancelled.phase);
        assert!(replay.idempotent_replay);
        let already_terminal = coordinator
            .cancel_idempotent(&queued.query_id, "cancel:terminal-observation", 1_004)
            .await
            .expect("new cancellation identity observes terminal state");
        assert_eq!(already_terminal.phase, cancelled.phase);
        assert!(!already_terminal.idempotent_replay);

        let signalled = coordinator
            .cancel_idempotent(&running.query_id, "cancel:running", 1_005)
            .await
            .expect("running cancellation signal");
        assert_eq!(signalled.phase, QueryExecutionPhase::Running);
        assert!(!signalled.idempotent_replay);
        assert!(
            coordinator
                .cancellation(&running.query_id)
                .await
                .expect("running cancellation authority")
                .load(Ordering::Acquire)
        );
        let replay = coordinator
            .cancel_idempotent(&running.query_id, "cancel:running", 1_006)
            .await
            .expect("running cancellation replay");
        assert_eq!(replay.phase, QueryExecutionPhase::Running);
        assert!(replay.idempotent_replay);
        assert!(matches!(
            coordinator
                .cancel_idempotent(&running.query_id, "", 1_007)
                .await,
            Err(QueryCoordinatorError::InvalidCancellationId)
        ));
    }

    #[tokio::test]
    async fn wp45_neg_policy_revocation_and_session_sharing_bind_authorization_and_cursor() {
        let temp = tempfile::tempdir().expect("tempdir");
        let coordinator = coordinator(&temp, policy(1, 1_024, 64), 7, 1_000);
        let operation = operation("authority-bound", 1);
        let exact_authority = authority(&operation);
        let accepted = acceptance(
            coordinator
                .accept(operation.clone(), 1_000)
                .await
                .expect("authority-bound acceptance"),
        );

        assert_eq!(
            coordinator
                .authorize_query(&accepted.query_id, exact_authority)
                .await
                .expect("exact authority"),
            operation.workspace_id
        );
        let next_policy = QuerySessionAuthority::try_new(
            operation.principal_id,
            operation.policy_generation + 1,
            operation.revocation_generation,
            operation.session_sharing_class,
        )
        .expect("next policy authority");
        let next_revocation = QuerySessionAuthority::try_new(
            operation.principal_id,
            operation.policy_generation,
            operation.revocation_generation + 1,
            operation.session_sharing_class,
        )
        .expect("next revocation authority");
        for changed_authority in [next_policy, next_revocation] {
            assert!(matches!(
                coordinator
                    .authorize_query(&accepted.query_id, changed_authority)
                    .await,
                Err(QueryCoordinatorError::QueryOwnerMismatch)
            ));
        }

        let cursor = coordinator
            .mint_cursor(&accepted.query_id, 0, 9_000)
            .await
            .expect("authority-bound cursor");
        let payload = decode_cursor(&cursor, &[0x71; 32]).expect("decode test cursor");
        assert_eq!(
            payload.session_sharing_class,
            QuerySessionSharingClass::PrincipalBound
        );
        assert_eq!(payload.policy_generation, operation.policy_generation);
        assert_eq!(
            payload.revocation_generation,
            operation.revocation_generation
        );
        assert_eq!(
            coordinator
                .verify_cursor(&cursor, exact_authority, 1_001)
                .await
                .expect("exact cursor authority"),
            (accepted.query_id.clone(), 0)
        );
        for changed_authority in [next_policy, next_revocation] {
            assert!(matches!(
                coordinator
                    .verify_cursor(&cursor, changed_authority, 1_001)
                    .await,
                Err(QueryCoordinatorError::CursorBinding)
            ));
        }

        let mut changed_policy_operation = operation.clone();
        changed_policy_operation.policy_generation += 1;
        assert!(matches!(
            coordinator.accept(changed_policy_operation, 1_001).await,
            Err(QueryCoordinatorError::IdempotencyConflict)
        ));
        let mut changed_revocation_operation = operation.clone();
        changed_revocation_operation.revocation_generation += 1;
        assert!(matches!(
            coordinator
                .accept(changed_revocation_operation, 1_001)
                .await,
            Err(QueryCoordinatorError::IdempotencyConflict)
        ));
        assert!(matches!(
            QuerySessionAuthority::try_new(
                operation.principal_id,
                0,
                operation.revocation_generation,
                operation.session_sharing_class,
            ),
            Err(QueryCoordinatorError::InvalidSessionAuthority)
        ));
        assert!(matches!(
            QuerySessionAuthority::try_new(
                operation.principal_id,
                operation.policy_generation,
                0,
                operation.session_sharing_class,
            ),
            Err(QueryCoordinatorError::InvalidSessionAuthority)
        ));
    }

    #[tokio::test]
    async fn wp45_ops_restart_after_cancellation_side_effect_before_ack_reports_replay() {
        let temp = tempfile::tempdir().expect("tempdir");
        let base_policy = policy(1, 1_024, 64);
        let query_id = {
            let coordinator = coordinator(&temp, base_policy, 7, 1_000);
            let accepted = acceptance(
                coordinator
                    .accept(operation("cancel-ack-loss", 1), 1_000)
                    .await
                    .expect("running acceptance"),
            );
            let outcome = coordinator
                .cancel_idempotent(&accepted.query_id, "cancel:durable-before-signal", 1_001)
                .await
                .expect("durable cancellation side effect");
            assert_eq!(outcome.phase, QueryExecutionPhase::Running);
            assert!(!outcome.idempotent_replay);
            assert!(
                coordinator
                    .cancellation(&accepted.query_id)
                    .await
                    .expect("process-local cancellation side effect")
                    .load(Ordering::Acquire)
            );
            // Model transport loss after the side effect by discarding `outcome` and restarting.
            accepted.query_id
        };

        let restarted = coordinator(&temp, base_policy, 8, 1_100);
        assert_eq!(
            restarted.phase(&query_id).await.expect("recovered phase"),
            QueryExecutionPhase::Terminal(QueryTerminalState::Lost)
        );
        let replay = restarted
            .cancel_idempotent(&query_id, "cancel:durable-before-signal", 1_101)
            .await
            .expect("recovered cancellation replay");
        assert_eq!(
            replay.phase,
            QueryExecutionPhase::Terminal(QueryTerminalState::Lost)
        );
        assert!(replay.idempotent_replay);

        let new_observation = restarted
            .cancel_idempotent(&query_id, "cancel:terminal-observation", 1_102)
            .await
            .expect("new terminal cancellation observation");
        assert!(!new_observation.idempotent_replay);
        drop(restarted);
        let restarted_again = coordinator(&temp, base_policy, 9, 1_200);
        assert!(
            restarted_again
                .cancel_idempotent(&query_id, "cancel:terminal-observation", 1_201)
                .await
                .expect("second durable cancellation replay")
                .idempotent_replay
        );
    }

    #[tokio::test]
    async fn wp45_neg_durable_cancellation_identity_ledger_is_strictly_bounded() {
        let temp = tempfile::tempdir().expect("tempdir");
        let base_policy = policy(1, 1_024, 64);
        let query_id = {
            let coordinator = coordinator(&temp, base_policy, 7, 1_000);
            let accepted = acceptance(
                coordinator
                    .accept(operation("cancel-ledger-bound", 1), 1_000)
                    .await
                    .expect("running acceptance"),
            );
            coordinator
                .terminal(
                    &accepted.query_id,
                    QueryTerminalState::Failed,
                    Some("EXPECTED_TEST_FAILURE".to_owned()),
                    None,
                    1_001,
                )
                .await
                .expect("terminal query");
            for ordinal in 0..MAX_CANCELLATION_IDENTITIES_PER_QUERY {
                let outcome = coordinator
                    .cancel_idempotent(
                        &accepted.query_id,
                        &format!("cancel:bounded:{ordinal}"),
                        1_002 + i64::try_from(ordinal).unwrap(),
                    )
                    .await
                    .expect("bounded cancellation identity");
                assert!(!outcome.idempotent_replay);
            }
            assert!(matches!(
                coordinator
                    .cancel_idempotent(&accepted.query_id, "cancel:overflow", 2_000)
                    .await,
                Err(QueryCoordinatorError::CancellationCapacity)
            ));
            assert!(
                coordinator
                    .cancel_idempotent(&accepted.query_id, "cancel:bounded:0", 2_001)
                    .await
                    .expect("replay remains available at capacity")
                    .idempotent_replay
            );
            accepted.query_id
        };

        let restarted = coordinator(&temp, base_policy, 8, 1_100);
        assert!(
            restarted
                .cancel_idempotent(&query_id, "cancel:bounded:63", 2_002)
                .await
                .expect("durable bounded replay")
                .idempotent_replay
        );
        assert!(matches!(
            restarted
                .cancel_idempotent(&query_id, "cancel:overflow", 2_003)
                .await,
            Err(QueryCoordinatorError::CancellationCapacity)
        ));
        assert!(matches!(
            restarted
                .cancel_idempotent(&query_id, "cancel:\u{80}", 2_004)
                .await,
            Err(QueryCoordinatorError::InvalidCancellationId)
        ));
    }

    #[tokio::test]
    async fn wp45_atomic_task_slot_reservation_rejects_concurrent_overacceptance() {
        let temp = tempfile::tempdir().expect("tempdir");
        let bounded =
            QueryCoordinatorPolicy::try_new(1, 1, 1, 16, 1, 16, 32 * 1024, 1_024, 64, 64).unwrap();
        let coordinator = coordinator(&temp, bounded, 7, 1_000);
        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        let left = {
            let coordinator = coordinator.clone();
            let barrier = Arc::clone(&barrier);
            async move {
                barrier.wait().await;
                coordinator
                    .accept(operation("task-slot-left", 1), 1_000)
                    .await
            }
        };
        let right = {
            let coordinator = coordinator.clone();
            let barrier = Arc::clone(&barrier);
            async move {
                barrier.wait().await;
                coordinator
                    .accept(operation("task-slot-right", 2), 1_000)
                    .await
            }
        };
        let (left, right) = tokio::join!(left, right);
        let accepted = match (left, right) {
            (Ok(QueryAcceptanceOutcome::New(accepted)), Err(error))
            | (Err(error), Ok(QueryAcceptanceOutcome::New(accepted))) => {
                assert!(matches!(
                    error,
                    QueryCoordinatorError::AdmissionBackpressure
                ));
                accepted
            }
            other => panic!("exactly one atomic task reservation must win: {other:?}"),
        };
        {
            let state = coordinator.state.lock().await;
            assert_eq!(state.task_reservations.len(), 1);
            assert!(state.task_reservations.contains(&accepted.query_id));
            assert!(state.tasks.is_empty());
        }
        let task = tokio::spawn(std::future::pending::<()>());
        coordinator
            .register_task(&accepted.query_id, task)
            .await
            .expect("fulfill the exact reserved task slot");
        coordinator
            .terminal(
                &accepted.query_id,
                QueryTerminalState::Failed,
                Some("EXPECTED_TEST_FAILURE".to_owned()),
                None,
                1_001,
            )
            .await
            .expect("terminal closure releases the task slot");
        assert!(coordinator.state.lock().await.task_reservations.is_empty());
        coordinator
            .accept(operation("task-slot-after-release", 3), 1_002)
            .await
            .expect("released slot admits the next bounded task");
    }

    #[tokio::test]
    async fn wp45_restart_after_publication_intent_recovers_every_pre_result_ready_kill_point() {
        let temp = tempfile::tempdir().expect("tempdir");
        let base_policy = policy(1, 1_024, 64);
        let expected = pending_object_set(&retained_locator());
        let query_id = {
            let coordinator = coordinator(&temp, base_policy, 7, 1_000);
            let accepted = acceptance(
                coordinator
                    .accept(operation("publication-intent-crash", 1), 1_000)
                    .await
                    .expect("accept before simulated crash"),
            );
            coordinator
                .append_event(
                    &accepted.query_id,
                    QueryControlEventPayload::PublicationPending {
                        object_set: expected.clone(),
                    },
                    1_001,
                )
                .await
                .expect("persist exact object set before the first write");
            accepted.query_id
        };

        let restarted = coordinator(&temp, base_policy, 8, 1_100);
        assert_eq!(
            acceptance(
                restarted
                    .accept(operation("publication-intent-crash", 1), 1_101)
                    .await
                    .expect("replay lost operation")
            )
            .phase,
            QueryExecutionPhase::Terminal(QueryTerminalState::Lost)
        );
        assert_eq!(
            restarted.pending_result_cleanups().await,
            vec![(query_id.clone(), expected)]
        );
        let public_events = restarted.events_after(&query_id, 0).await.expect("events");
        assert!(public_events.iter().all(|event| {
            !matches!(
                event.payload,
                QueryControlEventPayload::PublicationPending { .. }
            )
        }));
    }

    #[tokio::test]
    async fn wp45_restart_after_result_ready_before_terminal_preserves_exact_cleanup_locator() {
        let temp = tempfile::tempdir().expect("tempdir");
        let base_policy = policy(1, 1_024, 64);
        let expected_locator = retained_locator();
        let query_id = {
            let coordinator = coordinator(&temp, base_policy, 7, 1_000);
            let accepted = acceptance(
                coordinator
                    .accept(operation("result-ready-crash", 1), 1_000)
                    .await
                    .expect("accept before simulated crash"),
            );
            append_result_ready(
                &coordinator,
                &accepted.query_id,
                expected_locator.clone(),
                1_001,
            )
            .await;
            accepted.query_id
        };

        let restarted = coordinator(&temp, base_policy, 8, 1_100);
        let replay = acceptance(
            restarted
                .accept(operation("result-ready-crash", 1), 1_101)
                .await
                .expect("restart observes one lost accepted query"),
        );
        assert_eq!(
            replay.phase,
            QueryExecutionPhase::Terminal(QueryTerminalState::Lost)
        );
        assert_eq!(restarted.snapshot().await.reserved_result_bytes, 0);
        assert_eq!(
            restarted.pending_result_cleanups().await,
            vec![(query_id, pending_object_set(&expected_locator))]
        );
    }

    #[tokio::test]
    async fn wp45_expiry_stages_durable_cleanup_before_deleting_the_query_tombstone() {
        let temp = tempfile::tempdir().expect("tempdir");
        let base_policy = policy(1, 1_024, 64);
        let expected_locator = retained_locator();
        let initial = coordinator(&temp, base_policy, 7, 1_000);
        let accepted = acceptance(
            initial
                .accept(operation("expiry-cleanup", 1), 1_000)
                .await
                .expect("accept retained query"),
        );
        append_result_ready(
            &initial,
            &accepted.query_id,
            expected_locator.clone(),
            1_001,
        )
        .await;
        initial
            .terminal(
                &accepted.query_id,
                QueryTerminalState::Succeeded,
                None,
                Some((96, 2)),
                1_002,
            )
            .await
            .unwrap();
        assert_eq!(initial.collect_expired(10_000).await.unwrap(), 1);
        assert_eq!(
            initial.pending_result_cleanups().await,
            vec![(
                accepted.query_id.clone(),
                pending_object_set(&expected_locator)
            )]
        );
        drop(initial);

        let restarted = coordinator(&temp, base_policy, 8, 10_001);
        assert_eq!(
            restarted.pending_result_cleanups().await,
            vec![(
                accepted.query_id.clone(),
                pending_object_set(&expected_locator)
            )]
        );
        restarted
            .mark_result_cleanup_complete(&accepted.query_id)
            .await
            .expect("object cleanup acknowledgement is durable");
        assert_eq!(restarted.collect_expired(10_002).await.unwrap(), 1);
        assert!(matches!(
            restarted.events_after(&accepted.query_id, 0).await,
            Err(QueryCoordinatorError::UnknownQuery(_))
        ));
    }

    #[tokio::test]
    async fn wp45_restart_retains_locator_and_release_denies_reissue_durably() {
        let temp = tempfile::tempdir().expect("tempdir");
        let base_policy = policy(1, 1_024, 64);
        let operation = operation("retained-reissue", 1);
        let session_authority = authority(&operation);
        let workspace = operation.workspace_id;
        let expected_locator = retained_locator();
        let query_id = {
            let coordinator = coordinator(&temp, base_policy, 7, 1_000);
            let accepted = acceptance(
                coordinator
                    .accept(operation, 1_000)
                    .await
                    .expect("accept retained query"),
            );
            append_result_ready(
                &coordinator,
                &accepted.query_id,
                expected_locator.clone(),
                1_001,
            )
            .await;
            coordinator
                .terminal(
                    &accepted.query_id,
                    QueryTerminalState::Succeeded,
                    None,
                    Some((96, 2)),
                    1_002,
                )
                .await
                .expect("retain succeeded result");
            accepted.query_id
        };

        let restarted = coordinator(&temp, base_policy, 8, 1_100);
        assert_eq!(
            restarted
                .authorize_retained_result_reissue(&query_id, session_authority)
                .await
                .expect("same owner may reopen retained package"),
            workspace
        );
        let wrong_principal = QuerySessionAuthority::try_new(
            PrincipalId::from_bytes([0x42; 16]),
            session_authority.policy_generation,
            session_authority.revocation_generation,
            session_authority.session_sharing_class,
        )
        .expect("different principal authority");
        assert!(matches!(
            restarted
                .authorize_retained_result_reissue(&query_id, wrong_principal)
                .await,
            Err(QueryCoordinatorError::QueryOwnerMismatch)
        ));
        let events = restarted.events_after(&query_id, 0).await.expect("events");
        assert!(events.iter().any(|event| {
            matches!(
                &event.payload,
                QueryControlEventPayload::ResultReady { retained_locator, .. }
                    if retained_locator == &expected_locator
            )
        }));
        restarted
            .release_result(&query_id)
            .await
            .expect("persist explicit release tombstone");
        assert!(matches!(
            restarted
                .authorize_retained_result_reissue(&query_id, session_authority)
                .await,
            Err(QueryCoordinatorError::ResultNotReleasable)
        ));
        drop(restarted);

        let restarted_again = coordinator(&temp, base_policy, 9, 1_200);
        assert_eq!(
            restarted_again.pending_result_cleanups().await,
            vec![(query_id.clone(), pending_object_set(&expected_locator))]
        );
        assert!(matches!(
            restarted_again
                .authorize_retained_result_reissue(&query_id, session_authority)
                .await,
            Err(QueryCoordinatorError::ResultNotReleasable)
        ));
    }
}
