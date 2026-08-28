//! Catalog-generated `SQLite` operational state with one logical writer.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use rusqlite::backup::Backup;
use rusqlite::{
    Connection, OpenFlags, OptionalExtension as _, Transaction, TransactionBehavior, params,
};
use thiserror::Error;

use crate::contracts::index::model_artifact_index;
use crate::fabric::{MutationJournal, MutationPhaseSpec, PreparedMutation};
use crate::model_generated::semantic_lane_fragments::{
    SEMANTIC_INGEST_CONTRACTS, SEMANTIC_INVALIDATION_CONTRACTS,
};
use crate::snapshot::ServingSnapshotManifest;

const SCHEMA_VERSION: u32 = 13;
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const OPERATIONAL_DDL: &str =
    include_str!("../contracts/generated/model/schema/operational-store.sql");
const SCHEMA_IR_BYTES: &[u8] = include_bytes!("../contracts/schema/schema-contract-ir.json");
const SCHEMA_VALIDATION_BYTES: &[u8] =
    include_bytes!("../contracts/generated/model/schema/schema-validation.json");
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
    /// Abort the ontology activation transaction before its commit.
    OntologyActivationBeforeCommit,
    /// Commit ontology activation, then simulate loss of the response.
    OntologyActivationAfterCommitResponseLost,
}

impl StoreFaultPoint {
    /// Closed set used by the deterministic fault-matrix registry.
    pub const ALL: [Self; 4] = [
        Self::MigrationBeforeCommit,
        Self::TransactionBeforeCommit,
        Self::OntologyActivationBeforeCommit,
        Self::OntologyActivationAfterCommitResponseLost,
    ];
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
    pub sandbox_profile_digest: Option<String>,
    pub state_code: i64,
    pub accepted_at: String,
    pub terminal_at: Option<String>,
    pub diagnostic_id: Option<Vec<u8>>,
}

/// One immutable authoritative terminal record for an allocated query execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryExecutionTerminalRecord {
    pub execution_id: String,
    pub workspace_id: Vec<u8>,
    pub semantic_request_id: String,
    pub mcp_call_id: String,
    pub terminal_phase: String,
    pub failing_stage: Option<String>,
    pub bundle_checksum: String,
    pub primary_payload_uri: Option<String>,
    pub payload_status: String,
    pub fallback_envelope_bytes: Option<Vec<u8>>,
    pub snapshot_id: Option<String>,
    pub publication_id: Option<String>,
    pub source_table_versions_bytes: Vec<u8>,
    pub created_at: i64,
    pub expires_at: i64,
}

/// Persisted candidate lifecycle projection. The generated tables remain authoritative.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OntologyCandidateProjection {
    pub candidate_identity: String,
    pub workspace_id: Vec<u8>,
    pub state: String,
    pub manifest_digest: String,
    pub program_identity: String,
    pub package_identity: String,
    pub config_identity: String,
    pub policy_identity: String,
    pub exact_table_set_identity: String,
    pub predecessor_epoch_identity: Option<String>,
    pub rollback_retain_until: i64,
}

/// Canonical accountable owner decision. Its identity is derived from all decision fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OntologyOwnerDecision {
    decision_identity: String,
    candidate_identity: String,
    owner_identity: String,
    policy_identity: String,
    decision_bytes: Vec<u8>,
    accepted_at: i64,
}

impl OntologyOwnerDecision {
    /// Construct a canonical owner decision after the caller has authenticated the owner.
    pub fn new(
        candidate_identity: impl Into<String>,
        owner_identity: impl Into<String>,
        policy_identity: impl Into<String>,
        accepted_at: i64,
    ) -> Result<Self, OperationalStoreError> {
        let candidate_identity = candidate_identity.into();
        let owner_identity = owner_identity.into();
        let policy_identity = policy_identity.into();
        if candidate_identity.is_empty() || owner_identity.is_empty() || policy_identity.is_empty()
        {
            return Err(OperationalStoreError::OntologyActivation(
                "owner decision contains an empty identity".into(),
            ));
        }
        let decision_bytes = crate::contracts::jcs::canonicalize_value(&serde_json::json!({
            "candidate_identity": candidate_identity,
            "owner_identity": owner_identity,
            "policy_identity": policy_identity,
            "accepted_at": accepted_at,
        }))
        .map_err(|error| OperationalStoreError::OntologyActivation(error.to_string()))?;
        let decision_identity = crate::integrity::framed_digest(&decision_bytes);
        Ok(Self {
            decision_identity,
            candidate_identity,
            owner_identity,
            policy_identity,
            decision_bytes,
            accepted_at,
        })
    }

    #[must_use]
    pub fn identity(&self) -> &str {
        &self.decision_identity
    }
}

/// Identity-only activation request accepted by the durable owner route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OntologyActivationRequest {
    pub request_key: String,
    pub candidate_identity: String,
    pub decision_identity: String,
    pub expected_predecessor_identity: Option<String>,
    pub expected_pointer_generation: i64,
    pub requested_at: i64,
}

/// Committed activation result, reconstructed identically after a lost response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OntologyActivationOutcome {
    pub request_key: String,
    pub candidate_identity: String,
    pub epoch_identity: String,
    pub pointer_generation: i64,
    pub receipt_set_identity: String,
    pub idempotent_replay: bool,
}

/// Complete immutable interpretation authority selected by the active ontology epoch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveOntologyAuthority {
    pub epoch_identity: String,
    pub candidate_identity: String,
    pub result_authority_identity: String,
    pub program_identity: String,
    pub function_catalog_identity: String,
    pub policy_identity: String,
    pub query_form_identity: String,
    pub checksum_version: String,
    pub exact_table_set_identity: String,
}

fn query_execution_terminal_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<QueryExecutionTerminalRecord> {
    Ok(QueryExecutionTerminalRecord {
        execution_id: row.get(0)?,
        workspace_id: row.get(1)?,
        semantic_request_id: row.get(2)?,
        mcp_call_id: row.get(3)?,
        terminal_phase: row.get(4)?,
        failing_stage: row.get(5)?,
        bundle_checksum: row.get(6)?,
        primary_payload_uri: row.get(7)?,
        payload_status: row.get(8)?,
        fallback_envelope_bytes: row.get(9)?,
        snapshot_id: row.get(10)?,
        publication_id: row.get(11)?,
        source_table_versions_bytes: row.get(12)?,
        created_at: row.get(13)?,
        expires_at: row.get(14)?,
    })
}

fn ontology_candidate_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<OntologyCandidateProjection> {
    Ok(OntologyCandidateProjection {
        candidate_identity: row.get(0)?,
        workspace_id: row.get(1)?,
        state: row.get(2)?,
        manifest_digest: row.get(3)?,
        program_identity: row.get(4)?,
        package_identity: row.get(5)?,
        config_identity: row.get(6)?,
        policy_identity: row.get(7)?,
        exact_table_set_identity: row.get(8)?,
        predecessor_epoch_identity: row.get(9)?,
        rollback_retain_until: row.get(10)?,
    })
}

fn framed_digest_parts(parts: impl IntoIterator<Item = impl AsRef<[u8]>>) -> String {
    let mut bytes = Vec::new();
    for part in parts {
        let part = part.as_ref();
        bytes.extend_from_slice(&(part.len() as u64).to_be_bytes());
        bytes.extend_from_slice(part);
    }
    crate::integrity::framed_digest(&bytes)
}

fn ontology_activation_request_digest(
    request: &OntologyActivationRequest,
) -> Result<String, OperationalStoreError> {
    if request.request_key.is_empty()
        || request.candidate_identity.is_empty()
        || request.decision_identity.is_empty()
        || request.expected_pointer_generation < 0
    {
        return Err(OperationalStoreError::OntologyActivation(
            "activation request contains an empty identity or negative generation".into(),
        ));
    }
    let bytes = crate::contracts::jcs::canonicalize_value(&serde_json::json!({
        "request_key": request.request_key,
        "candidate_identity": request.candidate_identity,
        "decision_identity": request.decision_identity,
        "expected_predecessor_identity": request.expected_predecessor_identity,
        "expected_pointer_generation": request.expected_pointer_generation,
    }))
    .map_err(|error| OperationalStoreError::OntologyActivation(error.to_string()))?;
    Ok(crate::integrity::framed_digest(&bytes))
}

fn candidate_for_activation(
    transaction: &Transaction<'_>,
    candidate_identity: &str,
) -> Result<OntologyCandidateProjection, OperationalStoreError> {
    transaction
        .query_row(
            "SELECT candidate_identity, workspace_id, state, manifest_digest,
               program_identity, package_identity, config_identity, policy_identity,
               exact_table_set_identity, predecessor_epoch_identity, rollback_retain_until
             FROM ontology_candidate WHERE candidate_identity=?1",
            [candidate_identity],
            ontology_candidate_from_row,
        )
        .map_err(Into::into)
}

fn current_ontology_pointer(
    transaction: &Transaction<'_>,
    workspace_id: &[u8],
) -> Result<Option<(String, String, i64)>, OperationalStoreError> {
    transaction
        .query_row(
            "SELECT candidate_identity, epoch_identity, pointer_generation
             FROM ontology_active_pointer WHERE workspace_id=?1",
            [workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(Into::into)
}

fn activation_replay(
    transaction: &Transaction<'_>,
    request: &OntologyActivationRequest,
    request_digest: &str,
) -> Result<Option<OntologyActivationOutcome>, OperationalStoreError> {
    let existing = transaction
        .query_row(
            "SELECT candidate_identity, decision_identity, request_digest, state
             FROM ontology_activation_request WHERE request_key=?1",
            [&request.request_key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((candidate_identity, decision_identity, stored_digest, state)) = existing else {
        return Ok(None);
    };
    if candidate_identity != request.candidate_identity
        || decision_identity != request.decision_identity
        || stored_digest != request_digest
    {
        return Err(OperationalStoreError::OntologyActivation(format!(
            "request-key collision for {}",
            request.request_key
        )));
    }
    if state != "COMPLETED" {
        return Err(OperationalStoreError::OntologyActivationOutcomeUnknown {
            request_key: request.request_key.clone(),
        });
    }
    let (receipt_set_identity, pointer_generation): (String, i64) = transaction.query_row(
        "SELECT receipt_set_identity, pointer_generation FROM ontology_acceptance
         WHERE candidate_identity=?1 AND request_key=?2",
        params![request.candidate_identity, request.request_key],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let (active_candidate, epoch_identity, active_generation) = transaction.query_row(
        "SELECT candidate_identity, epoch_identity, pointer_generation
         FROM ontology_active_pointer
         WHERE workspace_id=(SELECT workspace_id FROM ontology_activation_request
                             WHERE request_key=?1)",
        [&request.request_key],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        },
    )?;
    if active_candidate != request.candidate_identity || active_generation != pointer_generation {
        return Err(OperationalStoreError::OntologyActivation(
            "completed request does not own the active pointer".into(),
        ));
    }
    Ok(Some(OntologyActivationOutcome {
        request_key: request.request_key.clone(),
        candidate_identity,
        epoch_identity,
        pointer_generation,
        receipt_set_identity,
        idempotent_replay: true,
    }))
}

fn json_string<'a>(
    value: &'a serde_json::Value,
    field: &str,
) -> Result<&'a str, OperationalStoreError> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            OperationalStoreError::OntologyActivation(format!(
                "receipt or artifact field {field} is absent"
            ))
        })
}

fn validated_receipt_set_identity(
    transaction: &Transaction<'_>,
    candidate: &OntologyCandidateProjection,
) -> Result<String, OperationalStoreError> {
    let exact_table_identity = {
        let mut statement = transaction.prepare(
            "SELECT table_code, table_uri, delta_version, schema_identity, content_identity
             FROM ontology_candidate_exact_table WHERE candidate_identity=?1
             ORDER BY table_code",
        )?;
        let rows = statement
            .query_map([&candidate.candidate_identity], |row| {
                Ok((
                    row.get::<_, i16>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if rows.is_empty() {
            return Err(OperationalStoreError::OntologyActivation(
                "candidate exact table set is empty".into(),
            ));
        }
        framed_digest_parts(rows.iter().map(|(code, uri, version, schema, content)| {
            framed_digest_parts([
                code.to_be_bytes().as_slice(),
                uri.as_bytes(),
                u64::try_from(*version)
                    .unwrap_or_default()
                    .to_be_bytes()
                    .as_slice(),
                schema.as_slice(),
                content.as_slice(),
            ])
        }))
    };
    if exact_table_identity != candidate.exact_table_set_identity {
        return Err(OperationalStoreError::OntologyActivation(
            "candidate exact table set identity drifted".into(),
        ));
    }
    let counts = (
        transaction.query_row(
            "SELECT count(*) FROM ontology_gate_execution WHERE candidate_identity=?1",
            [&candidate.candidate_identity],
            |row| row.get::<_, i64>(0),
        )?,
        transaction.query_row(
            "SELECT count(*) FROM ontology_gate_receipt WHERE candidate_identity=?1",
            [&candidate.candidate_identity],
            |row| row.get::<_, i64>(0),
        )?,
        transaction.query_row(
            "SELECT count(*) FROM ontology_gate_artifact WHERE candidate_identity=?1",
            [&candidate.candidate_identity],
            |row| row.get::<_, i64>(0),
        )?,
    );
    if counts.0 == 0 || counts.0 != counts.1 || counts.1 != counts.2 {
        return Err(OperationalStoreError::OntologyActivation(
            "execution, receipt, and artifact ledgers are not bijective".into(),
        ));
    }
    let mut statement = transaction.prepare(
        "SELECT execution.operation_id, execution.execution_identity,
           execution.semantic_checksum, execution.artifact_identity,
           execution.receipt_identity, receipt.receipt_bytes,
           receipt.expected_result_contract, artifact.artifact_bytes
         FROM ontology_gate_execution AS execution
         JOIN ontology_gate_receipt AS receipt
           ON receipt.receipt_identity=execution.receipt_identity
          AND receipt.candidate_identity=execution.candidate_identity
          AND receipt.operation_id=execution.operation_id
          AND receipt.semantic_checksum=execution.semantic_checksum
          AND receipt.artifact_identity=execution.artifact_identity
         JOIN ontology_gate_artifact AS artifact
           ON artifact.artifact_identity=execution.artifact_identity
          AND artifact.candidate_identity=execution.candidate_identity
          AND artifact.operation_id=execution.operation_id
         WHERE execution.candidate_identity=?1 ORDER BY execution.operation_id",
    )?;
    let rows = statement
        .query_map([&candidate.candidate_identity], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Vec<u8>>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Vec<u8>>(7)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if i64::try_from(rows.len()).unwrap_or(-1) != counts.0 {
        return Err(OperationalStoreError::OntologyActivation(
            "opaque receipt joins are incomplete".into(),
        ));
    }
    let mut identities = Vec::with_capacity(rows.len());
    for (
        operation,
        execution,
        checksum,
        artifact,
        receipt,
        receipt_bytes,
        contract,
        artifact_bytes,
    ) in rows
    {
        if crate::contracts::jcs::canonicalize_slice(&receipt_bytes)
            .map_err(|error| OperationalStoreError::OntologyActivation(error.to_string()))?
            != receipt_bytes
            || crate::contracts::jcs::canonicalize_slice(&artifact_bytes)
                .map_err(|error| OperationalStoreError::OntologyActivation(error.to_string()))?
                != artifact_bytes
        {
            return Err(OperationalStoreError::OntologyActivation(
                "receipt or artifact bytes are not canonical".into(),
            ));
        }
        let receipt_value: serde_json::Value = serde_json::from_slice(&receipt_bytes)
            .map_err(|error| OperationalStoreError::OntologyActivation(error.to_string()))?;
        for (field, expected) in [
            ("operation_id", operation.as_str()),
            ("execution_identity", execution.as_str()),
            ("candidate_identity", candidate.candidate_identity.as_str()),
            ("program_identity", candidate.program_identity.as_str()),
            ("package_identity", candidate.package_identity.as_str()),
            ("config_identity", candidate.config_identity.as_str()),
            ("policy_identity", candidate.policy_identity.as_str()),
            (
                "exact_table_set_identity",
                candidate.exact_table_set_identity.as_str(),
            ),
            ("semantic_checksum", checksum.as_str()),
            ("expected_result_contract", contract.as_str()),
            ("artifact_identity", artifact.as_str()),
            ("receipt_identity", receipt.as_str()),
        ] {
            if json_string(&receipt_value, field)? != expected {
                return Err(OperationalStoreError::OntologyActivation(format!(
                    "receipt field {field} differs from durable authority"
                )));
            }
        }
        let recomputed_receipt = framed_digest_parts([
            b"ontology-candidate-gate-receipt.v1".as_slice(),
            operation.as_bytes(),
            execution.as_bytes(),
            candidate.candidate_identity.as_bytes(),
            candidate.program_identity.as_bytes(),
            candidate.package_identity.as_bytes(),
            json_string(&receipt_value, "session_identity")?.as_bytes(),
            candidate.config_identity.as_bytes(),
            candidate.policy_identity.as_bytes(),
            candidate.exact_table_set_identity.as_bytes(),
            checksum.as_bytes(),
            contract.as_bytes(),
            artifact.as_bytes(),
        ]);
        if recomputed_receipt != receipt {
            return Err(OperationalStoreError::OntologyActivation(
                "opaque receipt identity does not recompute".into(),
            ));
        }
        let artifact_value: serde_json::Value = serde_json::from_slice(&artifact_bytes)
            .map_err(|error| OperationalStoreError::OntologyActivation(error.to_string()))?;
        for (field, expected) in [
            ("operation_id", operation.as_str()),
            ("execution_identity", execution.as_str()),
            ("candidate_identity", candidate.candidate_identity.as_str()),
            ("artifact_identity", artifact.as_str()),
        ] {
            if json_string(&artifact_value, field)? != expected {
                return Err(OperationalStoreError::OntologyActivation(format!(
                    "artifact field {field} differs from durable authority"
                )));
            }
        }
        let terminal_action_count = artifact_value
            .get("terminal_action_count")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                OperationalStoreError::OntologyActivation(
                    "artifact terminal action count is absent".into(),
                )
            })?;
        let metrics = artifact_value
            .get("metrics")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| {
                OperationalStoreError::OntologyActivation("artifact metrics are absent".into())
            })?;
        let recomputed_artifact = framed_digest_parts(
            [
                execution.as_bytes().to_vec(),
                candidate.candidate_identity.as_bytes().to_vec(),
                operation.as_bytes().to_vec(),
                u16::try_from(terminal_action_count)
                    .map_err(|_| {
                        OperationalStoreError::OntologyActivation(
                            "terminal action count exceeds u16".into(),
                        )
                    })?
                    .to_be_bytes()
                    .to_vec(),
                json_string(&artifact_value, "physical_plan_diagnostic")?
                    .as_bytes()
                    .to_vec(),
            ]
            .into_iter()
            .chain(metrics.iter().map(|(name, value)| {
                format!("{name}:{}", value.as_u64().unwrap_or_default()).into_bytes()
            })),
        );
        if recomputed_artifact != artifact {
            return Err(OperationalStoreError::OntologyActivation(
                "diagnostic artifact identity does not recompute".into(),
            ));
        }
        identities.push(framed_digest_parts([
            operation.as_bytes(),
            execution.as_bytes(),
            checksum.as_bytes(),
            artifact.as_bytes(),
            receipt.as_bytes(),
        ]));
    }
    Ok(framed_digest_parts(identities))
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
    #[error("query execution terminal record conflict: {0}")]
    QueryExecutionTerminalRecord(String),
    #[error("ONTOLOGY_ACTIVATION_TRANSACTION_INVALID:{0}")]
    OntologyActivation(String),
    #[error("ontology activation outcome is unknown for request {request_key}")]
    OntologyActivationOutcomeUnknown { request_key: String },
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
    #[allow(clippy::too_many_lines)] // Retention is one transaction over the closed operational-table set.
    pub fn cleanup_terminal_before(
        &mut self,
        cutoff: &str,
    ) -> Result<RetentionReport, OperationalStoreError> {
        let protected_scopes = {
            let mut statement = self.connection.prepare(
                "SELECT manifest_json_bytes FROM serving_snapshot_manifest ORDER BY snapshot_id",
            )?;
            let manifests = statement
                .query_map([], |row| row.get::<_, Vec<u8>>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            manifests.into_iter().try_fold(
                BTreeSet::new(),
                |mut scopes, bytes| -> Result<_, OperationalStoreError> {
                    let manifest: ServingSnapshotManifest = serde_json::from_slice(&bytes)
                        .map_err(|error| {
                            OperationalStoreError::DdlLineage(format!(
                                "retained snapshot manifest is malformed: {error}"
                            ))
                        })?;
                    let workspace = manifest.raw_workspace_id().map_err(|error| {
                        OperationalStoreError::DdlLineage(format!(
                            "retained snapshot workspace is malformed: {error}"
                        ))
                    })?;
                    let generation = i64::try_from(manifest.body.source.source_generation)
                        .map_err(|_| {
                            OperationalStoreError::DdlLineage(
                                "retained snapshot source generation exceeds i64".into(),
                            )
                        })?;
                    scopes.insert((workspace, generation));
                    Ok(scopes)
                },
            )?
        };
        self.write_transaction(|transaction| {
            let terminal_waves = {
                let mut statement = transaction.prepare(
                    "SELECT wave_id, workspace_id, source_generation FROM update_wave
                     WHERE terminal_at IS NOT NULL AND terminal_at < ?1 ORDER BY wave_id",
                )?;
                statement
                    .query_map([cutoff], |row| {
                        Ok((
                            row.get::<_, Vec<u8>>(0)?,
                            row.get::<_, Vec<u8>>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?
            };
            let mut update_wave_items = 0;
            let mut update_waves = 0;
            for (wave_id, workspace, generation) in terminal_waves {
                let workspace = <[u8; 16]>::try_from(workspace.as_slice()).map_err(|_| {
                    OperationalStoreError::DdlLineage("update-wave workspace is not Id16".into())
                })?;
                if protected_scopes.contains(&(workspace, generation)) {
                    continue;
                }
                update_wave_items += transaction.execute(
                    "DELETE FROM update_wave_item WHERE wave_id=?1",
                    [wave_id.as_slice()],
                )?;
                update_waves += transaction.execute(
                    "DELETE FROM update_wave WHERE wave_id=?1",
                    [wave_id.as_slice()],
                )?;
            }
            let terminal_provider_runs = {
                let mut statement = transaction.prepare(
                    "SELECT provider_run_id, workspace_id, source_generation FROM provider_run
                     WHERE terminal_at IS NOT NULL AND terminal_at < ?1 ORDER BY provider_run_id",
                )?;
                statement
                    .query_map([cutoff], |row| {
                        Ok((
                            row.get::<_, Vec<u8>>(0)?,
                            row.get::<_, Vec<u8>>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?
            };
            let mut provider_runs = 0;
            for (run_id, workspace, generation) in terminal_provider_runs {
                let workspace = <[u8; 16]>::try_from(workspace.as_slice()).map_err(|_| {
                    OperationalStoreError::DdlLineage("provider-run workspace is not Id16".into())
                })?;
                if !protected_scopes.contains(&(workspace, generation)) {
                    provider_runs += transaction.execute(
                        "DELETE FROM provider_run WHERE provider_run_id=?1",
                        [run_id.as_slice()],
                    )?;
                }
            }
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
        for (field, bytes) in [
            ("provider_run_id", record.provider_run_id.as_slice()),
            ("workspace_id", record.workspace_id.as_slice()),
            ("analysis_context_id", record.analysis_context_id.as_slice()),
            ("wave_id", record.wave_id.as_slice()),
        ] {
            if bytes.len() != 16 {
                return Err(OperationalStoreError::ProviderRunRecord(format!(
                    "{field} must be an Id16"
                )));
            }
        }
        for (field, bytes) in [
            ("owner_id", record.owner_id.as_deref()),
            ("build_unit_id", record.build_unit_id.as_deref()),
            ("diagnostic_id", record.diagnostic_id.as_deref()),
        ] {
            if bytes.is_some_and(|bytes| bytes.len() != 16) {
                return Err(OperationalStoreError::ProviderRunRecord(format!(
                    "{field} must be an Id16 when present"
                )));
            }
        }
        if record
            .sandbox_profile_digest
            .as_deref()
            .is_some_and(|value| !valid_sandbox_profile_digest(value))
        {
            return Err(OperationalStoreError::ProviderRunRecord(
                "sandbox_profile_digest must be a canonical sha256: or b3: digest".into(),
            ));
        }
        self.write_transaction(|transaction| {
            let changed = transaction.execute(
                "INSERT INTO provider_run (
                   provider_run_id, workspace_id, analysis_context_id, wave_id,
                   provider_code, owner_id, build_unit_id, source_generation,
                   input_fingerprint, output_fingerprint, sandbox_profile_digest, state_code,
                   accepted_at, terminal_at, diagnostic_id
                 ) VALUES (
                   ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
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
                   AND provider_run.sandbox_profile_digest IS excluded.sandbox_profile_digest
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
                    record.sandbox_profile_digest,
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

    /// Close provider runs left non-terminal by a prior daemon process.
    ///
    /// Queued work never received a permit and is recovered as cancelled; running work lost its
    /// process and is recovered as crashed. This method is invoked once during daemon recovery,
    /// before new runtimes admit work.
    ///
    /// # Errors
    ///
    /// Returns a transaction failure or an invalid empty recovery timestamp.
    pub fn recover_incomplete_provider_runs(
        &mut self,
        recovered_at: &str,
    ) -> Result<usize, OperationalStoreError> {
        if recovered_at.is_empty() {
            return Err(OperationalStoreError::ProviderRunRecord(
                "provider recovery timestamp is empty".into(),
            ));
        }
        self.write_transaction(|transaction| {
            transaction
                .execute(
                    "UPDATE provider_run
                     SET state_code=CASE
                         WHEN state_code=?1 THEN ?2
                         WHEN state_code=?3 THEN ?4
                         ELSE state_code
                       END,
                       terminal_at=?5
                     WHERE terminal_at IS NULL AND state_code IN (?1,?3)",
                    rusqlite::params![
                        i64::from(crate::registries::ProviderRunState::Queued as u16),
                        i64::from(crate::registries::ProviderRunState::Cancelled as u16),
                        i64::from(crate::registries::ProviderRunState::Running as u16),
                        i64::from(crate::registries::ProviderRunState::Crashed as u16),
                        recovered_at,
                    ],
                )
                .map_err(OperationalStoreError::from)
        })
    }

    /// Validate every durable ontology activation before the daemon admits new work.
    ///
    /// Activation is a single `SQLite` transaction, so a normal restart can observe only a
    /// fully committed epoch or no epoch. This check treats any non-terminal request, detached
    /// active row, or pointer whose acceptance/result-authority closure is incomplete as durable
    /// corruption instead of attempting to repair or infer authority.
    ///
    /// # Errors
    ///
    /// Returns [`OperationalStoreError::OntologyActivation`] when the persisted activation
    /// closure is not self-consistent, or a `SQLite` error when validation cannot be completed.
    pub fn validate_ontology_activation_recovery(&self) -> Result<(), OperationalStoreError> {
        let non_terminal_requests = self.connection.query_row(
            "SELECT COUNT(*) FROM ontology_activation_request WHERE state <> 'COMPLETED'",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        if non_terminal_requests != 0 {
            return Err(OperationalStoreError::OntologyActivation(format!(
                "restart observed {non_terminal_requests} non-terminal activation request(s)"
            )));
        }

        let invalid_pointers = self.connection.query_row(
            "SELECT COUNT(*)
             FROM ontology_active_pointer AS pointer
             LEFT JOIN ontology_candidate AS candidate
               ON candidate.candidate_identity=pointer.candidate_identity
              AND candidate.workspace_id=pointer.workspace_id
             LEFT JOIN ontology_serving_epoch AS epoch
               ON epoch.epoch_identity=pointer.epoch_identity
              AND epoch.workspace_id=pointer.workspace_id
              AND epoch.candidate_identity=pointer.candidate_identity
             LEFT JOIN ontology_result_authority AS authority
               ON authority.result_authority_identity=epoch.result_authority_identity
              AND authority.workspace_id=pointer.workspace_id
             LEFT JOIN ontology_acceptance AS acceptance
               ON acceptance.candidate_identity=pointer.candidate_identity
              AND acceptance.workspace_id=pointer.workspace_id
              AND acceptance.pointer_generation=pointer.pointer_generation
             LEFT JOIN ontology_activation_request AS request
               ON request.request_key=acceptance.request_key
              AND request.workspace_id=pointer.workspace_id
              AND request.candidate_identity=pointer.candidate_identity
             WHERE candidate.candidate_identity IS NULL
                OR candidate.state <> 'ACTIVE'
                OR epoch.epoch_identity IS NULL
                OR epoch.state <> 'ACTIVE'
                OR epoch.predecessor_epoch_identity IS NOT pointer.predecessor_epoch_identity
                OR authority.result_authority_identity IS NULL
                OR authority.exact_table_set_identity <> candidate.exact_table_set_identity
                OR acceptance.candidate_identity IS NULL
                OR request.request_key IS NULL
                OR request.state <> 'COMPLETED'",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        let detached_active_candidates = self.connection.query_row(
            "SELECT COUNT(*) FROM ontology_candidate AS candidate
             WHERE candidate.state='ACTIVE'
               AND NOT EXISTS (
                 SELECT 1 FROM ontology_active_pointer AS pointer
                 WHERE pointer.workspace_id=candidate.workspace_id
                   AND pointer.candidate_identity=candidate.candidate_identity
               )",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        let detached_active_epochs = self.connection.query_row(
            "SELECT COUNT(*) FROM ontology_serving_epoch AS epoch
             WHERE epoch.state='ACTIVE'
               AND NOT EXISTS (
                 SELECT 1 FROM ontology_active_pointer AS pointer
                 WHERE pointer.workspace_id=epoch.workspace_id
                   AND pointer.candidate_identity=epoch.candidate_identity
                   AND pointer.epoch_identity=epoch.epoch_identity
               )",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        if invalid_pointers != 0 || detached_active_candidates != 0 || detached_active_epochs != 0 {
            return Err(OperationalStoreError::OntologyActivation(format!(
                "restart activation closure is invalid: invalid_pointers={invalid_pointers}, \
                 detached_active_candidates={detached_active_candidates}, \
                 detached_active_epochs={detached_active_epochs}"
            )));
        }
        Ok(())
    }

    /// Commit one terminal meaning and its primary payload lease atomically.
    ///
    /// # Errors
    ///
    /// A repeated identical checksum is idempotent; any conflicting terminal meaning fails
    /// closed and leaves the existing row unchanged.
    pub fn commit_query_execution_terminal(
        &mut self,
        record: &QueryExecutionTerminalRecord,
    ) -> Result<(), OperationalStoreError> {
        if record.execution_id.is_empty()
            || record.workspace_id.is_empty()
            || record.bundle_checksum.is_empty()
            || record.source_table_versions_bytes.is_empty()
        {
            return Err(OperationalStoreError::QueryExecutionTerminalRecord(
                "required terminal identity or provenance is empty".to_owned(),
            ));
        }
        self.write_transaction(|transaction| {
            let existing = transaction
                .query_row(
                    "SELECT execution_id, workspace_id, semantic_request_id, mcp_call_id,
                       terminal_phase, failing_stage, bundle_checksum, primary_payload_uri,
                       payload_status, fallback_envelope_bytes, snapshot_id, publication_id,
                       source_table_versions_bytes, created_at, expires_at
                     FROM query_execution_terminal WHERE execution_id=?1",
                    [&record.execution_id],
                    query_execution_terminal_from_row,
                )
                .optional()?;
            if let Some(existing) = existing {
                if existing == *record {
                    return Ok(());
                }
                return Err(OperationalStoreError::QueryExecutionTerminalRecord(
                    record.execution_id.clone(),
                ));
            }
            transaction.execute(
                "INSERT INTO query_execution_terminal(
                   execution_id, workspace_id, semantic_request_id, mcp_call_id,
                   terminal_phase, failing_stage, bundle_checksum, primary_payload_uri,
                   payload_status, fallback_envelope_bytes, snapshot_id, publication_id,
                   source_table_versions_bytes, created_at, expires_at
                 ) VALUES (
                   ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
                 )",
                params![
                    record.execution_id,
                    record.workspace_id,
                    record.semantic_request_id,
                    record.mcp_call_id,
                    record.terminal_phase,
                    record.failing_stage,
                    record.bundle_checksum,
                    record.primary_payload_uri,
                    record.payload_status,
                    record.fallback_envelope_bytes,
                    record.snapshot_id,
                    record.publication_id,
                    record.source_table_versions_bytes,
                    record.created_at,
                    record.expires_at,
                ],
            )?;
            if let Some(uri) = &record.primary_payload_uri {
                transaction.execute(
                    "INSERT INTO result_artifact_lease(lease_id, artifact_uri, checksum, expires_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![
                        record.execution_id.as_bytes(),
                        uri,
                        record.bundle_checksum.as_bytes(),
                        record.expires_at,
                    ],
                )?;
            }
            Ok(())
        })
    }

    /// Read one authoritative terminal record by execution identity.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` error when the journal cannot be queried.
    pub fn read_query_execution_terminal(
        &self,
        execution_id: &str,
    ) -> Result<Option<QueryExecutionTerminalRecord>, OperationalStoreError> {
        self.connection
            .query_row(
                "SELECT execution_id, workspace_id, semantic_request_id, mcp_call_id,
                   terminal_phase, failing_stage, bundle_checksum, primary_payload_uri,
                   payload_status, fallback_envelope_bytes, snapshot_id, publication_id,
                   source_table_versions_bytes, created_at, expires_at
                 FROM query_execution_terminal WHERE execution_id=?1",
                [execution_id],
                query_execution_terminal_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    /// Read a bounded canonical terminal census for recovery and explain traversal.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` error when the journal cannot be queried.
    pub fn query_execution_terminals(
        &self,
        maximum: usize,
    ) -> Result<Vec<QueryExecutionTerminalRecord>, OperationalStoreError> {
        let maximum = i64::try_from(maximum).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare(
            "SELECT execution_id, workspace_id, semantic_request_id, mcp_call_id,
               terminal_phase, failing_stage, bundle_checksum, primary_payload_uri,
               payload_status, fallback_envelope_bytes, snapshot_id, publication_id,
               source_table_versions_bytes, created_at, expires_at
             FROM query_execution_terminal ORDER BY execution_id LIMIT ?1",
        )?;
        statement
            .query_map([maximum], query_execution_terminal_from_row)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Verify that the payload lease projection matches one terminal-journal authority row.
    ///
    /// Fallback-only terminal records intentionally have no payload lease.
    ///
    /// # Errors
    ///
    /// Returns a `SQLite` error when the lease projection cannot be queried.
    pub fn query_execution_terminal_lease_matches(
        &self,
        record: &QueryExecutionTerminalRecord,
    ) -> Result<bool, OperationalStoreError> {
        let lease = self
            .connection
            .query_row(
                "SELECT artifact_uri, checksum, expires_at
                 FROM result_artifact_lease WHERE lease_id=?1",
                [record.execution_id.as_bytes()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        match (&record.primary_payload_uri, lease) {
            (None, None) => Ok(true),
            (Some(uri), Some((lease_uri, checksum, expires_at))) => Ok(lease_uri == *uri
                && checksum == record.bundle_checksum.as_bytes()
                && expires_at == record.expires_at),
            _ => Ok(false),
        }
    }

    /// Persist a sealed candidate and the complete trusted-runner evidence ledger before any
    /// activation transaction is allowed to begin.
    ///
    /// # Errors
    ///
    /// Rejects identity drift, non-canonical evidence, incomplete bindings, or `SQLite` errors.
    pub fn persist_proved_ontology_candidate(
        &mut self,
        report: &crate::ontology_candidate::CandidateClosureReport,
        persisted_at: i64,
    ) -> Result<(), OperationalStoreError> {
        let evidence = report.durable_evidence();
        if evidence.gate_evidence.is_empty()
            || evidence.exact_tables.is_empty()
            || evidence.candidate_identity != report.candidate_identity()
            || crate::integrity::framed_digest(&evidence.manifest_bytes) != evidence.manifest_digest
        {
            return Err(OperationalStoreError::OntologyActivation(
                "trusted candidate evidence is incomplete or inconsistent".into(),
            ));
        }
        self.write_transaction(|transaction| {
            let existing = transaction
                .query_row(
                    "SELECT manifest_digest, state FROM ontology_candidate
                     WHERE candidate_identity=?1",
                    [&evidence.candidate_identity],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?;
            if let Some((manifest_digest, state)) = existing {
                if manifest_digest == evidence.manifest_digest
                    && matches!(
                        state.as_str(),
                        "PROVED" | "ACCEPTED" | "ACTIVE" | "SUPERSEDED"
                    )
                {
                    return Ok(());
                }
                return Err(OperationalStoreError::OntologyActivation(format!(
                    "candidate identity collision for {}",
                    evidence.candidate_identity
                )));
            }
            transaction.execute(
                "INSERT INTO ontology_candidate(
                   candidate_identity, workspace_id, state, manifest_bytes, manifest_digest,
                   program_identity, package_identity, config_identity, policy_identity,
                   exact_table_set_identity, predecessor_epoch_identity, rollback_retain_until,
                   created_at, updated_at
                 ) VALUES (?1,?2,'BUILDING',?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?12)",
                params![
                    evidence.candidate_identity,
                    evidence.workspace_id.as_slice(),
                    evidence.manifest_bytes,
                    evidence.manifest_digest,
                    evidence.program_identity,
                    evidence.package_identity,
                    evidence.config_identity,
                    evidence.policy_identity,
                    evidence.exact_table_set_identity,
                    evidence.predecessor_epoch_identity,
                    evidence.rollback_retain_until,
                    persisted_at,
                ],
            )?;
            for table in &evidence.exact_tables {
                let delta_version = i64::try_from(table.delta_version).map_err(|_| {
                    OperationalStoreError::OntologyActivation(format!(
                        "Delta version exceeds i64 for table {}",
                        table.table_code
                    ))
                })?;
                transaction.execute(
                    "INSERT INTO ontology_candidate_exact_table(
                       candidate_identity, workspace_id, table_code, table_uri, delta_version,
                       schema_identity, content_identity
                     ) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                    params![
                        evidence.candidate_identity,
                        evidence.workspace_id.as_slice(),
                        table.table_code,
                        table.table_uri,
                        delta_version,
                        table.schema_identity.as_slice(),
                        table.content_identity.as_slice(),
                    ],
                )?;
            }
            transaction.execute(
                "UPDATE ontology_candidate SET state='SEALED', updated_at=?2
                 WHERE candidate_identity=?1 AND state='BUILDING'",
                params![evidence.candidate_identity, persisted_at],
            )?;
            for gate in &evidence.gate_evidence {
                transaction.execute(
                    "INSERT INTO ontology_gate_artifact(
                       artifact_identity, workspace_id, candidate_identity, operation_id,
                       artifact_bytes, created_at
                     ) VALUES (?1,?2,?3,?4,?5,?6)",
                    params![
                        gate.artifact_identity,
                        evidence.workspace_id.as_slice(),
                        evidence.candidate_identity,
                        gate.operation_id,
                        gate.artifact_bytes,
                        persisted_at,
                    ],
                )?;
                transaction.execute(
                    "INSERT INTO ontology_gate_receipt(
                       receipt_identity, workspace_id, candidate_identity, operation_id,
                       receipt_bytes, semantic_checksum, expected_result_contract,
                       artifact_identity
                     ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![
                        gate.receipt_identity,
                        evidence.workspace_id.as_slice(),
                        evidence.candidate_identity,
                        gate.operation_id,
                        gate.receipt_bytes,
                        gate.semantic_checksum,
                        gate.expected_result_contract,
                        gate.artifact_identity,
                    ],
                )?;
                transaction.execute(
                    "INSERT INTO ontology_gate_execution(
                       execution_identity, workspace_id, candidate_identity, operation_id,
                       semantic_checksum, artifact_identity, receipt_identity, completed_at
                     ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![
                        gate.execution_identity,
                        evidence.workspace_id.as_slice(),
                        evidence.candidate_identity,
                        gate.operation_id,
                        gate.semantic_checksum,
                        gate.artifact_identity,
                        gate.receipt_identity,
                        persisted_at,
                    ],
                )?;
            }
            transaction.execute(
                "INSERT INTO ontology_result_authority(
                   result_authority_identity, workspace_id, program_identity,
                   function_catalog_identity, policy_identity, query_form_identity,
                   checksum_version, exact_table_set_identity, created_at
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
                 ON CONFLICT(result_authority_identity) DO NOTHING",
                params![
                    evidence.result_authority_identity,
                    evidence.workspace_id.as_slice(),
                    evidence.program_identity,
                    evidence.function_catalog_identity,
                    evidence.result_policy_identity,
                    evidence.query_form_identity,
                    evidence.checksum_version,
                    evidence.exact_table_set_identity,
                    persisted_at,
                ],
            )?;
            let persisted_authority = transaction.query_row(
                "SELECT workspace_id, program_identity, function_catalog_identity,
                   policy_identity, query_form_identity, checksum_version,
                   exact_table_set_identity
                 FROM ontology_result_authority WHERE result_authority_identity=?1",
                [&evidence.result_authority_identity],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            )?;
            if persisted_authority
                != (
                    evidence.workspace_id.to_vec(),
                    evidence.program_identity.clone(),
                    evidence.function_catalog_identity.clone(),
                    evidence.result_policy_identity.clone(),
                    evidence.query_form_identity.clone(),
                    evidence.checksum_version.clone(),
                    evidence.exact_table_set_identity.clone(),
                )
            {
                return Err(OperationalStoreError::OntologyActivation(
                    "result authority identity collision".into(),
                ));
            }
            let changed = transaction.execute(
                "UPDATE ontology_candidate SET state='PROVED', updated_at=?2
                 WHERE candidate_identity=?1 AND state='SEALED'",
                params![evidence.candidate_identity, persisted_at],
            )?;
            if changed != 1 {
                return Err(OperationalStoreError::OntologyActivation(
                    "trusted runner could not advance SEALED candidate to PROVED".into(),
                ));
            }
            Ok(())
        })
    }

    /// Persist one accountable owner decision separately from observations and gate metrics.
    pub fn record_ontology_owner_decision(
        &mut self,
        decision: &OntologyOwnerDecision,
    ) -> Result<(), OperationalStoreError> {
        if crate::integrity::framed_digest(&decision.decision_bytes) != decision.decision_identity {
            return Err(OperationalStoreError::OntologyActivation(
                "owner decision bytes do not match their identity".into(),
            ));
        }
        self.write_transaction(|transaction| {
            let (workspace_id, state, policy_identity) = transaction.query_row(
                "SELECT workspace_id, state, policy_identity FROM ontology_candidate
                 WHERE candidate_identity=?1",
                [&decision.candidate_identity],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )?;
            if state != "PROVED" || policy_identity != decision.policy_identity {
                return Err(OperationalStoreError::OntologyActivation(
                    "owner decision does not match a PROVED candidate policy".into(),
                ));
            }
            let existing = transaction
                .query_row(
                    "SELECT decision_identity, owner_identity, policy_identity, decision_bytes,
                       accepted_at FROM ontology_owner_decision WHERE candidate_identity=?1",
                    [&decision.candidate_identity],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, Vec<u8>>(3)?,
                            row.get::<_, i64>(4)?,
                        ))
                    },
                )
                .optional()?;
            if let Some(existing) = existing {
                if existing
                    == (
                        decision.decision_identity.clone(),
                        decision.owner_identity.clone(),
                        decision.policy_identity.clone(),
                        decision.decision_bytes.clone(),
                        decision.accepted_at,
                    )
                {
                    return Ok(());
                }
                return Err(OperationalStoreError::OntologyActivation(
                    "candidate already has a different owner decision".into(),
                ));
            }
            transaction.execute(
                "INSERT INTO ontology_owner_decision(
                   decision_identity, workspace_id, candidate_identity, owner_identity,
                   policy_identity, decision_bytes, accepted_at
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![
                    decision.decision_identity,
                    workspace_id,
                    decision.candidate_identity,
                    decision.owner_identity,
                    decision.policy_identity,
                    decision.decision_bytes,
                    decision.accepted_at,
                ],
            )?;
            Ok(())
        })
    }

    /// Atomically accept a proved candidate and advance the ontology pointer with CAS.
    pub fn activate_ontology_candidate(
        &mut self,
        request: &OntologyActivationRequest,
    ) -> Result<OntologyActivationOutcome, OperationalStoreError> {
        self.activate_ontology_candidate_with_fault(request, None)
    }

    /// Resolve an identity-only admin command to the current durable predecessor and CAS
    /// generation. The command cannot inject pointer, policy, or proof contents.
    pub fn resolve_ontology_activation_request(
        &self,
        workspace_id: [u8; 16],
        candidate_identity: &str,
        decision_identity: &str,
        request_key: &str,
        requested_at: i64,
    ) -> Result<OntologyActivationRequest, OperationalStoreError> {
        let replay = self
            .connection
            .query_row(
                "SELECT workspace_id, candidate_identity, decision_identity,
                   expected_predecessor_identity, expected_pointer_generation, created_at
                 FROM ontology_activation_request WHERE request_key=?1",
                [request_key],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .optional()?;
        if let Some((
            recorded_workspace,
            recorded_candidate,
            recorded_decision,
            expected_predecessor_identity,
            expected_pointer_generation,
            recorded_at,
        )) = replay
        {
            if recorded_workspace != workspace_id
                || recorded_candidate != candidate_identity
                || recorded_decision != decision_identity
            {
                return Err(OperationalStoreError::OntologyActivation(
                    "activation request key is already bound to different identities".into(),
                ));
            }
            return Ok(OntologyActivationRequest {
                request_key: request_key.into(),
                candidate_identity: recorded_candidate,
                decision_identity: recorded_decision,
                expected_predecessor_identity,
                expected_pointer_generation,
                requested_at: recorded_at,
            });
        }
        let candidate = self
            .ontology_candidate(candidate_identity)?
            .ok_or_else(|| {
                OperationalStoreError::OntologyActivation(format!(
                    "candidate {candidate_identity} is absent"
                ))
            })?;
        if candidate.workspace_id != workspace_id {
            return Err(OperationalStoreError::OntologyActivation(
                "admin workspace does not own the candidate".into(),
            ));
        }
        let pointer = self
            .connection
            .query_row(
                "SELECT epoch_identity, pointer_generation FROM ontology_active_pointer
                 WHERE workspace_id=?1",
                [workspace_id.as_slice()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        let (expected_predecessor_identity, expected_pointer_generation) =
            pointer.map_or((None, 0), |(epoch, generation)| (Some(epoch), generation));
        if candidate.predecessor_epoch_identity != expected_predecessor_identity {
            return Err(OperationalStoreError::OntologyActivation(
                "candidate is not sealed against the current predecessor epoch".into(),
            ));
        }
        Ok(OntologyActivationRequest {
            request_key: request_key.into(),
            candidate_identity: candidate_identity.into(),
            decision_identity: decision_identity.into(),
            expected_predecessor_identity,
            expected_pointer_generation,
            requested_at,
        })
    }

    fn activate_ontology_candidate_with_fault(
        &mut self,
        request: &OntologyActivationRequest,
        fault: Option<StoreFaultPoint>,
    ) -> Result<OntologyActivationOutcome, OperationalStoreError> {
        let request_digest = ontology_activation_request_digest(request)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(outcome) = activation_replay(&transaction, request, &request_digest)? {
            transaction.commit()?;
            return Ok(outcome);
        }
        let candidate = candidate_for_activation(&transaction, &request.candidate_identity)?;
        if candidate.state != "PROVED" {
            return Err(OperationalStoreError::OntologyActivation(format!(
                "candidate {} is not PROVED",
                request.candidate_identity
            )));
        }
        if candidate.predecessor_epoch_identity != request.expected_predecessor_identity {
            return Err(OperationalStoreError::OntologyActivation(
                "candidate predecessor binding differs from the request".into(),
            ));
        }
        let (owner_candidate, decision_policy) = transaction
            .query_row(
                "SELECT candidate_identity, policy_identity FROM ontology_owner_decision
                 WHERE decision_identity=?1",
                [&request.decision_identity],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or_else(|| {
                OperationalStoreError::OntologyActivation(format!(
                    "owner decision {} is absent",
                    request.decision_identity
                ))
            })?;
        if owner_candidate != request.candidate_identity
            || decision_policy != candidate.policy_identity
        {
            return Err(OperationalStoreError::OntologyActivation(
                "owner decision candidate or policy does not match".into(),
            ));
        }
        let current = current_ontology_pointer(&transaction, &candidate.workspace_id)?;
        match &current {
            Some((_, epoch, generation))
                if *generation == request.expected_pointer_generation
                    && Some(epoch.as_str()) == request.expected_predecessor_identity.as_deref() => {
            }
            None if request.expected_pointer_generation == 0
                && request.expected_predecessor_identity.is_none() => {}
            _ => {
                return Err(OperationalStoreError::OntologyActivation(
                    "ontology pointer CAS predecessor or generation differs".into(),
                ));
            }
        }
        let receipt_set_identity = validated_receipt_set_identity(&transaction, &candidate)?;
        let pointer_generation = request
            .expected_pointer_generation
            .checked_add(1)
            .ok_or_else(|| {
                OperationalStoreError::OntologyActivation(
                    "ontology pointer generation overflow".into(),
                )
            })?;
        let epoch_identity = crate::integrity::framed_digest(
            &[
                candidate.workspace_id.as_slice(),
                request.candidate_identity.as_bytes(),
                request.request_key.as_bytes(),
                &pointer_generation.to_be_bytes(),
            ]
            .concat(),
        );
        transaction.execute(
            "INSERT INTO ontology_activation_request(
               request_key, workspace_id, candidate_identity, decision_identity,
               request_digest, expected_predecessor_identity, expected_pointer_generation,
               state, created_at, completed_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,'COMMITTING',?8,NULL)",
            params![
                request.request_key,
                candidate.workspace_id,
                request.candidate_identity,
                request.decision_identity,
                request_digest,
                request.expected_predecessor_identity,
                request.expected_pointer_generation,
                request.requested_at,
            ],
        )?;
        transaction.execute(
            "INSERT INTO ontology_acceptance(
               candidate_identity, workspace_id, request_key, decision_identity,
               receipt_set_identity, pointer_generation, accepted_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                request.candidate_identity,
                candidate.workspace_id,
                request.request_key,
                request.decision_identity,
                receipt_set_identity,
                pointer_generation,
                request.requested_at,
            ],
        )?;
        transaction.execute(
            "UPDATE ontology_candidate SET state='ACCEPTED', updated_at=?2
             WHERE candidate_identity=?1 AND state='PROVED'",
            params![request.candidate_identity, request.requested_at],
        )?;
        if let Some((prior_candidate, _, _)) = &current {
            transaction.execute(
                "UPDATE ontology_candidate SET state='SUPERSEDED', updated_at=?2
                 WHERE candidate_identity=?1 AND state='ACTIVE'",
                params![prior_candidate, request.requested_at],
            )?;
            transaction.execute(
                "UPDATE ontology_serving_epoch SET state='SUPERSEDED'
                 WHERE candidate_identity=?1 AND state='ACTIVE'",
                [prior_candidate],
            )?;
        }
        let result_policy_identity = framed_digest_parts([
            b"candidate-policy.v1".as_slice(),
            candidate.policy_identity.as_bytes(),
        ]);
        let result_authority_identity: String = transaction.query_row(
            "SELECT result_authority_identity FROM ontology_result_authority
             WHERE workspace_id=?1 AND program_identity=?2 AND policy_identity=?3
               AND exact_table_set_identity=?4",
            params![
                candidate.workspace_id,
                candidate.program_identity,
                result_policy_identity,
                candidate.exact_table_set_identity,
            ],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO ontology_serving_epoch(
               epoch_identity, workspace_id, candidate_identity, predecessor_epoch_identity,
               result_authority_identity, state, activated_at, retained_until
             ) VALUES (?1,?2,?3,?4,?5,'ACTIVE',?6,?7)",
            params![
                epoch_identity,
                candidate.workspace_id,
                request.candidate_identity,
                request.expected_predecessor_identity,
                result_authority_identity,
                request.requested_at,
                candidate.rollback_retain_until,
            ],
        )?;
        let pointer_changed = if current.is_some() {
            transaction.execute(
                "UPDATE ontology_active_pointer
                 SET candidate_identity=?2, epoch_identity=?3,
                     predecessor_epoch_identity=?4, pointer_generation=?5, updated_at=?6
                 WHERE workspace_id=?1 AND pointer_generation=?7",
                params![
                    candidate.workspace_id,
                    request.candidate_identity,
                    epoch_identity,
                    request.expected_predecessor_identity,
                    pointer_generation,
                    request.requested_at,
                    request.expected_pointer_generation,
                ],
            )?
        } else {
            transaction.execute(
                "INSERT INTO ontology_active_pointer(
                   workspace_id, candidate_identity, epoch_identity,
                   predecessor_epoch_identity, pointer_generation, updated_at
                 ) VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    candidate.workspace_id,
                    request.candidate_identity,
                    epoch_identity,
                    request.expected_predecessor_identity,
                    pointer_generation,
                    request.requested_at,
                ],
            )?
        };
        if pointer_changed != 1 {
            return Err(OperationalStoreError::OntologyActivation(
                "ontology pointer CAS lost".into(),
            ));
        }
        transaction.execute(
            "UPDATE ontology_candidate SET state='ACTIVE', updated_at=?2
             WHERE candidate_identity=?1 AND state='ACCEPTED'",
            params![request.candidate_identity, request.requested_at],
        )?;
        transaction.execute(
            "UPDATE ontology_activation_request SET state='COMPLETED', completed_at=?2
             WHERE request_key=?1 AND state='COMMITTING'",
            params![request.request_key, request.requested_at],
        )?;
        if fault == Some(StoreFaultPoint::OntologyActivationBeforeCommit) {
            return Err(OperationalStoreError::InjectedFault(
                StoreFaultPoint::OntologyActivationBeforeCommit,
            ));
        }
        transaction.commit()?;
        if fault == Some(StoreFaultPoint::OntologyActivationAfterCommitResponseLost) {
            return Err(OperationalStoreError::OntologyActivationOutcomeUnknown {
                request_key: request.request_key.clone(),
            });
        }
        Ok(OntologyActivationOutcome {
            request_key: request.request_key.clone(),
            candidate_identity: request.candidate_identity.clone(),
            epoch_identity,
            pointer_generation,
            receipt_set_identity,
            idempotent_replay: false,
        })
    }

    /// Reconcile an activation after restart or a lost response and persist that observation.
    pub fn reconcile_ontology_activation(
        &mut self,
        request: &OntologyActivationRequest,
    ) -> Result<OntologyActivationOutcome, OperationalStoreError> {
        let request_digest = ontology_activation_request_digest(request)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let outcome =
            activation_replay(&transaction, request, &request_digest)?.ok_or_else(|| {
                OperationalStoreError::OntologyActivationOutcomeUnknown {
                    request_key: request.request_key.clone(),
                }
            })?;
        let workspace_id: Vec<u8> = transaction.query_row(
            "SELECT workspace_id FROM ontology_activation_request WHERE request_key=?1",
            [&request.request_key],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO ontology_recovery(
               request_key, workspace_id, candidate_identity, outcome,
               observed_pointer_generation, reconciled_at
             ) VALUES (?1,?2,?3,'COMMITTED',?4,?5)
             ON CONFLICT(request_key) DO UPDATE SET
               outcome=excluded.outcome,
               observed_pointer_generation=excluded.observed_pointer_generation,
               reconciled_at=excluded.reconciled_at",
            params![
                request.request_key,
                workspace_id,
                request.candidate_identity,
                outcome.pointer_generation,
                request.requested_at,
            ],
        )?;
        transaction.commit()?;
        Ok(outcome)
    }

    /// Read a candidate projection after reopen without exposing receipt trust fields.
    pub fn ontology_candidate(
        &self,
        candidate_identity: &str,
    ) -> Result<Option<OntologyCandidateProjection>, OperationalStoreError> {
        self.connection
            .query_row(
                "SELECT candidate_identity, workspace_id, state, manifest_digest,
                   program_identity, package_identity, config_identity, policy_identity,
                   exact_table_set_identity, predecessor_epoch_identity, rollback_retain_until
                 FROM ontology_candidate WHERE candidate_identity=?1",
                [candidate_identity],
                ontology_candidate_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    /// Resolve the active epoch to its complete versioned result authority.
    pub fn active_ontology_authority(
        &self,
        workspace_id: [u8; 16],
    ) -> Result<Option<ActiveOntologyAuthority>, OperationalStoreError> {
        self.connection
            .query_row(
                "SELECT pointer.epoch_identity, pointer.candidate_identity,
                   authority.result_authority_identity, authority.program_identity,
                   authority.function_catalog_identity, authority.policy_identity,
                   authority.query_form_identity, authority.checksum_version,
                   authority.exact_table_set_identity
                 FROM ontology_active_pointer AS pointer
                 JOIN ontology_serving_epoch AS epoch
                   ON epoch.epoch_identity=pointer.epoch_identity
                  AND epoch.candidate_identity=pointer.candidate_identity
                 JOIN ontology_result_authority AS authority
                   ON authority.result_authority_identity=epoch.result_authority_identity
                 WHERE pointer.workspace_id=?1 AND epoch.state='ACTIVE'",
                [workspace_id.as_slice()],
                |row| {
                    Ok(ActiveOntologyAuthority {
                        epoch_identity: row.get(0)?,
                        candidate_identity: row.get(1)?,
                        result_authority_identity: row.get(2)?,
                        program_identity: row.get(3)?,
                        function_catalog_identity: row.get(4)?,
                        policy_identity: row.get(5)?,
                        query_form_identity: row.get(6)?,
                        checksum_version: row.get(7)?,
                        exact_table_set_identity: row.get(8)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
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
                migrate_v8_to_v9(&transaction)?;
                migrate_v9_to_v10(&transaction)?;
                migrate_v10_to_v11(&transaction)?;
                migrate_v11_to_v12(&transaction)?;
            }
            2 => {
                migrate_v2_to_v3(&transaction)?;
                migrate_v3_to_v4(&transaction)?;
                migrate_v4_to_v5(&transaction)?;
                migrate_v5_to_v6(&transaction)?;
                migrate_v6_to_v7(&transaction)?;
                migrate_v7_to_v8(&transaction)?;
                migrate_v8_to_v9(&transaction)?;
                migrate_v9_to_v10(&transaction)?;
                migrate_v10_to_v11(&transaction)?;
                migrate_v11_to_v12(&transaction)?;
            }
            3 => {
                migrate_v3_to_v4(&transaction)?;
                migrate_v4_to_v5(&transaction)?;
                migrate_v5_to_v6(&transaction)?;
                migrate_v6_to_v7(&transaction)?;
                migrate_v7_to_v8(&transaction)?;
                migrate_v8_to_v9(&transaction)?;
                migrate_v9_to_v10(&transaction)?;
                migrate_v10_to_v11(&transaction)?;
                migrate_v11_to_v12(&transaction)?;
            }
            4 => {
                migrate_v4_to_v5(&transaction)?;
                migrate_v5_to_v6(&transaction)?;
                migrate_v6_to_v7(&transaction)?;
                migrate_v7_to_v8(&transaction)?;
                migrate_v8_to_v9(&transaction)?;
                migrate_v9_to_v10(&transaction)?;
                migrate_v10_to_v11(&transaction)?;
                migrate_v11_to_v12(&transaction)?;
            }
            5 => {
                migrate_v5_to_v6(&transaction)?;
                migrate_v6_to_v7(&transaction)?;
                migrate_v7_to_v8(&transaction)?;
                migrate_v8_to_v9(&transaction)?;
                migrate_v9_to_v10(&transaction)?;
                migrate_v10_to_v11(&transaction)?;
                migrate_v11_to_v12(&transaction)?;
            }
            6 => {
                migrate_v6_to_v7(&transaction)?;
                migrate_v7_to_v8(&transaction)?;
                migrate_v8_to_v9(&transaction)?;
                migrate_v9_to_v10(&transaction)?;
                migrate_v10_to_v11(&transaction)?;
                migrate_v11_to_v12(&transaction)?;
            }
            7 => {
                migrate_v7_to_v8(&transaction)?;
                migrate_v8_to_v9(&transaction)?;
                migrate_v9_to_v10(&transaction)?;
                migrate_v10_to_v11(&transaction)?;
                migrate_v11_to_v12(&transaction)?;
            }
            8 => {
                migrate_v8_to_v9(&transaction)?;
                migrate_v9_to_v10(&transaction)?;
                migrate_v10_to_v11(&transaction)?;
                migrate_v11_to_v12(&transaction)?;
            }
            9 => {
                migrate_v9_to_v10(&transaction)?;
                migrate_v10_to_v11(&transaction)?;
                migrate_v11_to_v12(&transaction)?;
            }
            10 => {
                migrate_v10_to_v11(&transaction)?;
                migrate_v11_to_v12(&transaction)?;
            }
            11 => migrate_v11_to_v12(&transaction)?,
            12 => {}
            _ => {
                return Err(OperationalStoreError::DdlLineage(format!(
                    "no migration is registered from schema {version}"
                )));
            }
        }
        if version != 0 {
            migrate_v12_to_v13(&transaction)?;
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
    workspace_id: Vec<u8>,
    analysis_context_id: Option<Vec<u8>>,
    source_generation: i64,
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
    #[allow(clippy::too_many_lines)] // Mutation replay and first-write validation share one immediate transaction.
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
                        workspace_id, analysis_context_id, source_generation,
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
                        workspace_id: row.get(3)?,
                        analysis_context_id: row.get(4)?,
                        source_generation: row.get(5)?,
                        owner_set_fingerprint: row.get(6)?,
                        input_checksum: row.get(7)?,
                        expected_output_checksum: row.get(8)?,
                        expected_predecessor: row.get(9)?,
                        state_code: row.get(10)?,
                        delta_version: row.get(11)?,
                    })
                },
            )
            .optional()?;
        if let Some(stored) = stored {
            let exact = stored.application_id == spec.application_id
                && stored.publication_id.as_slice() == spec.publication_id
                && stored.workspace_id.as_slice() == spec.workspace_id
                && stored.analysis_context_id.as_deref()
                    == spec.analysis_context_id.as_ref().map(<[u8; 16]>::as_slice)
                && stored.source_generation == spec.source_generation
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
                 application_version, publication_id, workspace_id,
                 analysis_context_id, source_generation, owner_set_fingerprint,
                 input_checksum, expected_output_checksum, expected_predecessor,
                 state_code, delta_version, created_at, completed_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,10,NULL,
                       strftime('%Y-%m-%dT%H:%M:%fZ','now'),NULL)",
            rusqlite::params![
                spec.operation_id,
                spec.table_code,
                spec.phase.as_str(),
                spec.application_id,
                application_version,
                spec.publication_id,
                spec.workspace_id,
                spec.analysis_context_id,
                spec.source_generation,
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

fn migrate_v8_to_v9(transaction: &Transaction<'_>) -> Result<(), OperationalStoreError> {
    transaction.execute_batch(
        "ALTER TABLE table_mutation_operation RENAME TO table_mutation_operation_v8;",
    )?;
    transaction.execute_batch(&generated_table_ddl("table_mutation_operation")?)?;
    transaction.execute_batch(
        "INSERT INTO table_mutation_operation(
           operation_id, table_code, mutation_phase, application_id,
           application_version, publication_id, workspace_id,
           analysis_context_id, source_generation, owner_set_fingerprint,
           input_checksum, expected_output_checksum, expected_predecessor,
           state_code, delta_version, created_at, completed_at)
         SELECT operation_id, table_code, mutation_phase, application_id,
           application_version, publication_id, zeroblob(16), NULL, 0,
           owner_set_fingerprint, input_checksum, expected_output_checksum,
           expected_predecessor, state_code, delta_version, created_at, completed_at
         FROM table_mutation_operation_v8;
         DROP TABLE table_mutation_operation_v8;",
    )?;
    Ok(())
}

fn migrate_v9_to_v10(transaction: &Transaction<'_>) -> Result<(), OperationalStoreError> {
    let already_present = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?1)",
        ["query_execution_terminal"],
        |row| row.get::<_, bool>(0),
    )?;
    if !already_present {
        transaction.execute_batch(&generated_table_ddl("query_execution_terminal")?)?;
    }
    Ok(())
}

fn migrate_v10_to_v11(transaction: &Transaction<'_>) -> Result<(), OperationalStoreError> {
    if !table_has_column(transaction, "provider_run", "sandbox_profile_digest")? {
        transaction.execute_batch("ALTER TABLE provider_run RENAME TO provider_run_v10;")?;
        transaction.execute_batch(&generated_table_ddl("provider_run")?)?;
        transaction.execute_batch(
            "INSERT INTO provider_run(
               provider_run_id, workspace_id, analysis_context_id, wave_id,
               provider_code, owner_id, build_unit_id, source_generation,
               input_fingerprint, output_fingerprint, sandbox_profile_digest,
               state_code, accepted_at, terminal_at, diagnostic_id)
             SELECT provider_run_id, workspace_id, analysis_context_id, wave_id,
               provider_code, owner_id, build_unit_id, source_generation,
               input_fingerprint, output_fingerprint, NULL,
               state_code, accepted_at, terminal_at, diagnostic_id
             FROM provider_run_v10;
             DROP TABLE provider_run_v10;",
        )?;
    }
    Ok(())
}

fn migrate_v11_to_v12(transaction: &Transaction<'_>) -> Result<(), OperationalStoreError> {
    for table in [
        "ontology_candidate",
        "ontology_candidate_exact_table",
        "ontology_gate_execution",
        "ontology_gate_receipt",
        "ontology_gate_artifact",
        "ontology_owner_decision",
        "ontology_activation_request",
        "ontology_acceptance",
        "ontology_active_pointer",
        "ontology_recovery",
        "ontology_result_authority",
        "ontology_serving_epoch",
    ] {
        transaction.execute_batch(&generated_table_ddl(table)?)?;
    }
    Ok(())
}

fn migrate_v12_to_v13(transaction: &Transaction<'_>) -> Result<(), OperationalStoreError> {
    if !table_has_column(transaction, "snapshot_lease", "ontology_epoch_identity")? {
        transaction.execute_batch(
            "ALTER TABLE snapshot_lease ADD COLUMN ontology_epoch_identity TEXT;
             ALTER TABLE snapshot_lease ADD COLUMN result_authority_identity TEXT;",
        )?;
    }
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
    crate::integrity::framed_digest(OPERATIONAL_DDL.as_bytes())
}

fn verify_ddl_lineage() -> Result<(), OperationalStoreError> {
    verify_semantic_fragment_table_contracts()?;
    let index = model_artifact_index().map_err(|error| {
        OperationalStoreError::DdlLineage(format!("artifact index unavailable: {error}"))
    })?;
    let schema_ir = index
        .artifacts
        .iter()
        .find(|artifact| artifact.artifact_id == SCHEMA_IR_ARTIFACT_ID)
        .ok_or_else(|| OperationalStoreError::DdlLineage("schema IR is absent".into()))?;
    if crate::integrity::framed_digest(SCHEMA_IR_BYTES) != schema_ir.source_digest {
        return Err(OperationalStoreError::DdlLineage(
            "packaged schema-IR bytes differ from the artifact index".into(),
        ));
    }
    let validation: serde_json::Value = serde_json::from_slice(SCHEMA_VALIDATION_BYTES)
        .map_err(|error| OperationalStoreError::DdlLineage(error.to_string()))?;
    let expected_source = validation
        .get("source_digest")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            OperationalStoreError::DdlLineage(
                "generated schema validation source digest is absent".into(),
            )
        })?;
    let first_line = OPERATIONAL_DDL.lines().next().unwrap_or_default();
    if !first_line.contains(expected_source) || !first_line.contains("@generated") {
        return Err(OperationalStoreError::DdlLineage(
            "DDL header does not bind the generated schema source identity".into(),
        ));
    }
    Ok(())
}

fn verify_semantic_fragment_table_contracts() -> Result<(), OperationalStoreError> {
    validate_semantic_table_codes(
        SEMANTIC_INGEST_CONTRACTS
            .iter()
            .flat_map(|contract| contract.output_table_codes.iter().copied())
            .chain(
                SEMANTIC_INVALIDATION_CONTRACTS
                    .iter()
                    .flat_map(|contract| contract.invalidated_table_codes.iter().copied()),
            ),
    )
}

fn validate_semantic_table_codes(
    table_codes: impl IntoIterator<Item = i16>,
) -> Result<(), OperationalStoreError> {
    for table_code in table_codes {
        let Some(table) = crate::schema_registry::table_spec(table_code) else {
            return Err(OperationalStoreError::DdlLineage(format!(
                "semantic fragment targets absent table {table_code}"
            )));
        };
        if crate::schema_registry::table_scope_spec(table_code).is_none() {
            return Err(OperationalStoreError::DdlLineage(format!(
                "semantic fragment table {} ({table_code}) has no generated owner scope",
                table.name
            )));
        }
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

fn valid_sandbox_profile_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .or_else(|| value.strip_prefix("b3:"))
        .is_some_and(|payload| {
            payload.len() == 64
                && payload
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        })
}

fn generated_table_names() -> BTreeSet<String> {
    crate::schema_registry::operational_table_specs()
        .iter()
        .map(|spec| spec.name.to_owned())
        .collect()
}

fn generated_table_ddl(table: &str) -> Result<String, OperationalStoreError> {
    crate::schema_registry::operational_table_spec(table)
        .map(|spec| spec.sqlite_ddl.to_owned())
        .ok_or_else(|| {
            OperationalStoreError::DdlLineage(format!("generated specs have no table {table}"))
        })
}

fn generated_column_shapes() -> Result<GeneratedColumnShapes, OperationalStoreError> {
    let mut result = BTreeMap::new();
    for table in crate::schema_registry::operational_table_specs() {
        let columns = table
            .arrow_schema
            .fields()
            .iter()
            .zip(&table.sqlite_column_types)
            .map(|(field, sqlite_type)| {
                let sqlite_type = match sqlite_type {
                    crate::schema_registry::OperationalSqliteType::Integer => "INTEGER",
                    crate::schema_registry::OperationalSqliteType::Real => "REAL",
                    crate::schema_registry::OperationalSqliteType::Text => "TEXT",
                    crate::schema_registry::OperationalSqliteType::Blob => "BLOB",
                };
                (
                    field.name().to_owned(),
                    sqlite_type.to_owned(),
                    !field.is_nullable(),
                )
            })
            .collect::<Vec<_>>();
        if columns.is_empty() {
            return Err(OperationalStoreError::DdlLineage(format!(
                "generated table spec {} has no columns",
                table.name
            )));
        }
        result.insert(table.name.to_owned(), columns);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use datafusion::prelude::SessionConfig;
    use tempfile::TempDir;

    use crate::fabric::{
        CurrentPublicationRecord, PublicationOutcome, PublicationScope, PublicationTableRecord,
    };
    use crate::governed_session::GovernedSession;
    use crate::ontology_candidate::{CandidateClosureReport, CandidateClosureRunner};
    use crate::ontology_gate::GateResourceEnvelope;
    use crate::ontology_program::{OntologyPackagingProfile, build_ontology_program_package};

    fn database() -> (TempDir, PathBuf) {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("operational.sqlite3");
        (directory, path)
    }

    fn candidate_publication(
        workspace_id: [u8; 16],
        marker: u8,
        delta_version: u64,
    ) -> PublicationOutcome {
        let publication_id = [marker; 16];
        PublicationOutcome {
            publication_id,
            scope: PublicationScope {
                workspace_id,
                source_generation: i64::from(marker),
                analysis_context_set_id: [marker.wrapping_add(1); 16],
                analysis_context_ids: vec![[marker.wrapping_add(2); 16]],
            },
            pointer: CurrentPublicationRecord {
                workspace_id,
                publication_id,
                pointer_generation: i64::from(marker),
                updated_at_micros: 1_000,
            },
            tables: BTreeMap::from([(
                1,
                PublicationTableRecord {
                    publication_id,
                    workspace_id,
                    table_code: 1,
                    table_uri: format!("file:///ontology-candidate/{marker}"),
                    delta_version,
                    schema_fingerprint: [marker.wrapping_add(3); 32],
                    row_count: 1,
                    owner_count: 1,
                    table_checksum: [marker.wrapping_add(4); 32],
                    primary_key_digest: [marker.wrapping_add(5); 32],
                    required: true,
                    validated: true,
                },
            )]),
        }
    }

    async fn proved_candidate(
        workspace_id: [u8; 16],
        marker: u8,
        delta_version: u64,
        predecessor: Option<String>,
    ) -> CandidateClosureReport {
        CandidateClosureRunner::new_for_epoch(
            build_ontology_program_package(&OntologyPackagingProfile::default()).unwrap(),
            candidate_publication(workspace_id, marker, delta_version),
            GovernedSession::new(SessionConfig::new(), "policy.ontology.v1").unwrap(),
            predecessor,
            99_999,
        )
        .unwrap()
        .execute(&GateResourceEnvelope::default())
        .await
        .unwrap()
    }

    fn owner_decision(report: &CandidateClosureReport, accepted_at: i64) -> OntologyOwnerDecision {
        OntologyOwnerDecision::new(
            report.candidate_identity(),
            "owner:ontology-release",
            report.durable_evidence().policy_identity.clone(),
            accepted_at,
        )
        .unwrap()
    }

    fn activation_request(
        report: &CandidateClosureReport,
        decision: &OntologyOwnerDecision,
        request_key: &str,
        predecessor: Option<String>,
        generation: i64,
    ) -> OntologyActivationRequest {
        OntologyActivationRequest {
            request_key: request_key.into(),
            candidate_identity: report.candidate_identity().into(),
            decision_identity: decision.identity().into(),
            expected_predecessor_identity: predecessor,
            expected_pointer_generation: generation,
            requested_at: 2_000 + generation,
        }
    }

    #[test]
    fn wp69_operational_acceptance() {
        let specs = crate::schema_registry::operational_table_specs();
        let names = generated_table_names();
        let shapes = generated_column_shapes().unwrap();
        assert_eq!(names.len(), specs.len());
        assert_eq!(shapes.len(), specs.len());
        for spec in specs {
            assert!(names.contains(spec.name));
            assert_eq!(generated_table_ddl(spec.name).unwrap(), spec.sqlite_ddl);
            assert!(
                spec.sqlite_ddl
                    .starts_with(&format!("CREATE TABLE {} (", spec.name))
            );
            assert!(spec.sqlite_ddl.ends_with(") STRICT;\n"));
            assert_eq!(shapes[spec.name].len(), spec.arrow_schema.fields().len());
        }
    }

    #[test]
    fn semantic_fragment_tables_are_schema_registered_and_scoped() {
        verify_semantic_fragment_table_contracts().unwrap();
        let error = validate_semantic_table_codes([i16::MAX]).unwrap_err();
        assert!(error.to_string().contains("targets absent table"));
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
    fn wp66_structural_acceptance() {
        let (_directory, path) = database();
        let mut store = OperationalStore::open(&path).unwrap();
        let workspace = [0x61_u8; 16];
        let wave = [0x62_u8; 16];
        let owner_from_fact_batch = [0x63_u8; 16];
        store
            .write_transaction(|transaction| {
                transaction.execute(
                    "INSERT INTO update_wave(
                       wave_id, workspace_id, source_generation, event_watermark,
                       state_code, candidate_strategy_code, input_fingerprint,
                       candidate_count, started_at, terminal_at, diagnostic_id)
                     VALUES (?1, ?2, 7, 9, 70, 10, ?3, 1,
                             '2026-08-25T00:00:00Z', '2026-08-25T00:00:01Z', NULL)",
                    rusqlite::params![
                        wave.as_slice(),
                        workspace.as_slice(),
                        [0x64_u8; 32].as_slice(),
                    ],
                )?;
                Ok::<_, OperationalStoreError>(())
            })
            .unwrap();
        store
            .record_provider_run(&ProviderRunRecord {
                provider_run_id: vec![0x65; 16],
                workspace_id: workspace.to_vec(),
                analysis_context_id: vec![0x66; 16],
                wave_id: wave.to_vec(),
                provider_code: 10,
                owner_id: Some(owner_from_fact_batch.to_vec()),
                build_unit_id: None,
                source_generation: 7,
                input_fingerprint: vec![0x67; 32],
                output_fingerprint: Some(vec![0x68; 32]),
                sandbox_profile_digest: Some(format!("b3:{}", "55".repeat(32))),
                state_code: 70,
                accepted_at: "2026-08-25T00:00:00Z".into(),
                terminal_at: Some("2026-08-25T00:00:01Z".into()),
                diagnostic_id: None,
            })
            .unwrap();
        let joined = store
            .reader_factory()
            .open()
            .unwrap()
            .with_connection(|connection| {
                connection.query_row(
                    "SELECT COUNT(*) FROM update_wave AS wave
                     JOIN provider_run AS run
                       ON run.wave_id=wave.wave_id
                      AND run.workspace_id=wave.workspace_id
                      AND run.source_generation=wave.source_generation
                     WHERE run.owner_id=?1",
                    [owner_from_fact_batch.as_slice()],
                    |row| row.get::<_, i64>(0),
                )
            })
            .unwrap();
        assert_eq!(joined, 1);

        let mut malformed = ProviderRunRecord {
            provider_run_id: vec![0x70; 16],
            workspace_id: workspace.to_vec(),
            analysis_context_id: vec![0x71; 16],
            wave_id: wave.to_vec(),
            provider_code: 10,
            owner_id: Some(vec![0; 15]),
            build_unit_id: None,
            source_generation: 7,
            input_fingerprint: vec![0; 32],
            output_fingerprint: None,
            sandbox_profile_digest: None,
            state_code: 10,
            accepted_at: "2026-08-25T00:00:02Z".into(),
            terminal_at: None,
            diagnostic_id: None,
        };
        assert!(matches!(
            store.record_provider_run(&malformed),
            Err(OperationalStoreError::ProviderRunRecord(_))
        ));
        malformed.owner_id = Some(owner_from_fact_batch.to_vec());
        assert!(store.record_provider_run(&malformed).is_ok());
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

    #[test]
    fn provider_run_sandbox_profile_digest_migrates_from_v10() {
        let (_directory, path) = database();
        let store = OperationalStore::open(&path).unwrap();
        drop(store);

        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "ALTER TABLE provider_run DROP COLUMN sandbox_profile_digest;
                 PRAGMA user_version=10;",
            )
            .unwrap();
        drop(connection);

        let migrated = OperationalStore::open(&path).unwrap();
        assert_eq!(user_version(&migrated.connection).unwrap(), SCHEMA_VERSION);
        let present = migrated
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info('provider_run') WHERE name='sandbox_profile_digest')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap();
        assert!(present);
    }

    #[test]
    fn ontology_operational_tables_migrate_from_v11() {
        let (_directory, path) = database();
        let store = OperationalStore::open(&path).unwrap();
        drop(store);
        let connection = Connection::open(&path).unwrap();
        for table in [
            "ontology_candidate",
            "ontology_candidate_exact_table",
            "ontology_gate_execution",
            "ontology_gate_receipt",
            "ontology_gate_artifact",
            "ontology_owner_decision",
            "ontology_activation_request",
            "ontology_acceptance",
            "ontology_active_pointer",
            "ontology_recovery",
            "ontology_result_authority",
            "ontology_serving_epoch",
        ] {
            connection
                .execute_batch(&format!("DROP TABLE {table};"))
                .unwrap();
        }
        connection.pragma_update(None, "user_version", 11).unwrap();
        drop(connection);
        let migrated = OperationalStore::open(&path).unwrap();
        assert_eq!(user_version(&migrated.connection).unwrap(), SCHEMA_VERSION);
        assert_eq!(
            migrated
                .connection
                .query_row(
                    "SELECT count(*) FROM sqlite_schema
                     WHERE type='table' AND name LIKE 'ontology_%'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            12
        );
        for column in ["ontology_epoch_identity", "result_authority_identity"] {
            assert!(
                migrated
                    .connection
                    .query_row(
                        "SELECT EXISTS(
                           SELECT 1 FROM pragma_table_info('snapshot_lease') WHERE name=?1
                         )",
                        [column],
                        |row| row.get::<_, bool>(0),
                    )
                    .unwrap(),
                "missing migrated snapshot_lease.{column}"
            );
        }
    }

    #[tokio::test]
    async fn ontology_candidate_receipt_binding_matrix() {
        let (_directory, path) = database();
        let report = proved_candidate([0x71; 16], 0x72, 17, None).await;
        let decision = owner_decision(&report, 2_000);
        let request = activation_request(&report, &decision, "activate-receipt-matrix", None, 0);
        let mut store = OperationalStore::open(&path).unwrap();
        store
            .persist_proved_ontology_candidate(&report, 1_000)
            .unwrap();
        store.record_ontology_owner_decision(&decision).unwrap();
        let projection = store
            .ontology_candidate(report.candidate_identity())
            .unwrap()
            .unwrap();
        assert_eq!(projection.state, "PROVED");
        assert_eq!(
            projection.exact_table_set_identity,
            report.durable_evidence().exact_table_set_identity
        );
        drop(store);

        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "UPDATE ontology_gate_receipt SET receipt_bytes=X'7b7d'
                 WHERE receipt_identity=(SELECT receipt_identity FROM ontology_gate_receipt
                                         ORDER BY receipt_identity LIMIT 1)",
                [],
            )
            .unwrap();
        drop(connection);
        let mut reopened = OperationalStore::open(&path).unwrap();
        let error = reopened.activate_ontology_candidate(&request).unwrap_err();
        assert!(error.to_string().contains("receipt"));
        assert_eq!(
            reopened
                .ontology_candidate(report.candidate_identity())
                .unwrap()
                .unwrap()
                .state,
            "PROVED"
        );
    }

    #[tokio::test]
    async fn ontology_activation_state_transaction_atomicity() {
        let (_directory, path) = database();
        let report = proved_candidate([0x73; 16], 0x74, 18, None).await;
        let decision = owner_decision(&report, 2_100);
        let request = activation_request(&report, &decision, "activate-atomic", None, 0);
        let mut store = OperationalStore::open(&path).unwrap();
        store
            .persist_proved_ontology_candidate(&report, 1_100)
            .unwrap();
        store.record_ontology_owner_decision(&decision).unwrap();
        let error = store
            .activate_ontology_candidate_with_fault(
                &request,
                Some(StoreFaultPoint::OntologyActivationBeforeCommit),
            )
            .unwrap_err();
        assert!(matches!(error, OperationalStoreError::InjectedFault(_)));
        assert_eq!(
            store
                .connection
                .query_row("SELECT count(*) FROM ontology_active_pointer", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            0
        );
        let first = store.activate_ontology_candidate(&request).unwrap();
        assert_eq!(first.pointer_generation, 1);
        assert!(!first.idempotent_replay);
        let replay = store.activate_ontology_candidate(&request).unwrap();
        assert!(replay.idempotent_replay);
        assert_eq!(first.epoch_identity, replay.epoch_identity);
        assert_eq!(
            store
                .ontology_candidate(report.candidate_identity())
                .unwrap()
                .unwrap()
                .state,
            "ACTIVE"
        );
    }

    #[tokio::test]
    async fn ontology_candidate_delta_exact_version_binding() {
        let (_directory, path) = database();
        let report = proved_candidate([0x75; 16], 0x76, 23, None).await;
        let mut store = OperationalStore::open(&path).unwrap();
        store
            .persist_proved_ontology_candidate(&report, 1_200)
            .unwrap();
        drop(store);
        let reopened = OperationalStore::open(&path).unwrap();
        let tuple = reopened
            .connection
            .query_row(
                "SELECT table_uri, delta_version, length(schema_identity),
                   length(content_identity) FROM ontology_candidate_exact_table
                 WHERE candidate_identity=?1",
                [report.candidate_identity()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(tuple, ("file:///ontology-candidate/118".into(), 23, 32, 32));
    }

    #[tokio::test]
    async fn ontology_decision_observation_separation() {
        let (_directory, path) = database();
        let report = proved_candidate([0x77; 16], 0x78, 29, None).await;
        let decision = owner_decision(&report, 2_300);
        assert!(!String::from_utf8_lossy(&decision.decision_bytes).contains("metrics"));
        assert!(!String::from_utf8_lossy(&decision.decision_bytes).contains("artifact"));
        let mut store = OperationalStore::open(&path).unwrap();
        store
            .persist_proved_ontology_candidate(&report, 1_300)
            .unwrap();
        store.record_ontology_owner_decision(&decision).unwrap();
        let stored: (String, Vec<u8>) = store
            .connection
            .query_row(
                "SELECT decision_identity, decision_bytes FROM ontology_owner_decision
                 WHERE candidate_identity=?1",
                [report.candidate_identity()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            stored,
            (decision.identity().into(), decision.decision_bytes)
        );
    }

    #[tokio::test]
    async fn ontology_activation_restart_idempotency() {
        let (_directory, path) = database();
        let report = proved_candidate([0x79; 16], 0x7a, 31, None).await;
        let decision = owner_decision(&report, 2_400);
        let request = activation_request(&report, &decision, "activate-lost-response", None, 0);
        let mut store = OperationalStore::open(&path).unwrap();
        store
            .persist_proved_ontology_candidate(&report, 1_400)
            .unwrap();
        store.record_ontology_owner_decision(&decision).unwrap();
        let error = store
            .activate_ontology_candidate_with_fault(
                &request,
                Some(StoreFaultPoint::OntologyActivationAfterCommitResponseLost),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            OperationalStoreError::OntologyActivationOutcomeUnknown { .. }
        ));
        drop(store);
        let mut reopened = OperationalStore::open(&path).unwrap();
        reopened.validate_ontology_activation_recovery().unwrap();
        let recovered = reopened.reconcile_ontology_activation(&request).unwrap();
        assert!(recovered.idempotent_replay);
        assert_eq!(recovered.pointer_generation, 1);
        assert_eq!(
            reopened
                .connection
                .query_row("SELECT count(*) FROM ontology_acceptance", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            1
        );
        reopened
            .connection
            .execute(
                "UPDATE ontology_activation_request SET state='COMMITTING' WHERE request_key=?1",
                [&request.request_key],
            )
            .unwrap();
        assert!(matches!(
            reopened.validate_ontology_activation_recovery(),
            Err(OperationalStoreError::OntologyActivation(message))
                if message.contains("non-terminal activation request")
        ));
    }

    #[tokio::test]
    async fn ontology_admin_activation_owner_route() {
        let (_directory, path) = database();
        let workspace = [0x7b; 16];
        let report = proved_candidate(workspace, 0x7c, 37, None).await;
        let decision = owner_decision(&report, 2_500);
        let mut store = OperationalStore::open(&path).unwrap();
        let command = crate::daemon::WorkspaceAdminCommand::ActivateCandidate {
            workspace_id: workspace,
            candidate_identity: report.candidate_identity().into(),
            decision_identity: decision.identity().into(),
            request_key: "admin-owner-route".into(),
        };
        let command_value = serde_json::to_value(&command).unwrap();
        assert_eq!(
            command_value
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "candidate_identity".into(),
                "command".into(),
                "decision_identity".into(),
                "request_key".into(),
                "workspace_id".into(),
            ])
        );
        let mut forged = command_value;
        forged["policy_identity"] = serde_json::Value::String("caller-forbidden".into());
        assert!(serde_json::from_value::<crate::daemon::WorkspaceAdminCommand>(forged).is_err());
        store
            .persist_proved_ontology_candidate(&report, 1_500)
            .unwrap();
        store.record_ontology_owner_decision(&decision).unwrap();
        let resolved = store
            .resolve_ontology_activation_request(
                workspace,
                report.candidate_identity(),
                decision.identity(),
                "admin-owner-route",
                2_500,
            )
            .unwrap();
        assert_eq!(resolved.expected_pointer_generation, 0);
        assert!(resolved.expected_predecessor_identity.is_none());
        let outcome = store.activate_ontology_candidate(&resolved).unwrap();
        assert_eq!(outcome.pointer_generation, 1);
        let replay = store
            .resolve_ontology_activation_request(
                workspace,
                report.candidate_identity(),
                decision.identity(),
                "admin-owner-route",
                9_999,
            )
            .unwrap();
        let replay_outcome = store.activate_ontology_candidate(&replay).unwrap();
        assert!(replay_outcome.idempotent_replay);
        assert_eq!(replay.requested_at, 2_500);
        assert_eq!(
            store
                .active_ontology_authority(workspace)
                .unwrap()
                .unwrap()
                .candidate_identity,
            report.candidate_identity()
        );
    }

    #[tokio::test]
    async fn ontology_activation_concurrency_forward_rollback() {
        let (_directory, path) = database();
        let workspace = [0x7d; 16];
        let first = proved_candidate(workspace, 0x7e, 41, None).await;
        let first_decision = owner_decision(&first, 2_600);
        let mut store = OperationalStore::open(&path).unwrap();
        store
            .persist_proved_ontology_candidate(&first, 1_600)
            .unwrap();
        store
            .record_ontology_owner_decision(&first_decision)
            .unwrap();
        let first_request = store
            .resolve_ontology_activation_request(
                workspace,
                first.candidate_identity(),
                first_decision.identity(),
                "activate-first",
                2_600,
            )
            .unwrap();
        let first_outcome = store.activate_ontology_candidate(&first_request).unwrap();

        let successor = proved_candidate(
            workspace,
            0x7f,
            43,
            Some(first_outcome.epoch_identity.clone()),
        )
        .await;
        let successor_decision = owner_decision(&successor, 2_700);
        store
            .persist_proved_ontology_candidate(&successor, 1_700)
            .unwrap();
        store
            .record_ontology_owner_decision(&successor_decision)
            .unwrap();
        let winner = store
            .resolve_ontology_activation_request(
                workspace,
                successor.candidate_identity(),
                successor_decision.identity(),
                "activate-successor-winner",
                2_700,
            )
            .unwrap();
        let mut loser = winner.clone();
        loser.request_key = "activate-successor-loser".into();
        let successor_outcome = store.activate_ontology_candidate(&winner).unwrap();
        assert_eq!(successor_outcome.pointer_generation, 2);
        assert!(store.activate_ontology_candidate(&loser).is_err());

        let rollback = proved_candidate(
            workspace,
            0x80,
            41,
            Some(successor_outcome.epoch_identity.clone()),
        )
        .await;
        let rollback_decision = owner_decision(&rollback, 2_800);
        store
            .persist_proved_ontology_candidate(&rollback, 1_800)
            .unwrap();
        store
            .record_ontology_owner_decision(&rollback_decision)
            .unwrap();
        let rollback_request = store
            .resolve_ontology_activation_request(
                workspace,
                rollback.candidate_identity(),
                rollback_decision.identity(),
                "activate-forward-rollback",
                2_800,
            )
            .unwrap();
        let rollback_outcome = store
            .activate_ontology_candidate(&rollback_request)
            .unwrap();
        assert_eq!(rollback_outcome.pointer_generation, 3);
        assert_ne!(
            rollback_outcome.epoch_identity,
            first_outcome.epoch_identity
        );
        assert_eq!(
            store
                .active_ontology_authority(workspace)
                .unwrap()
                .unwrap()
                .candidate_identity,
            rollback.candidate_identity()
        );
    }
}
