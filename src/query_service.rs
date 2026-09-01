//! Accepted gRPC query handles and immutable canonical result artifacts.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::future::Future;
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use futures::{Stream, stream};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{Mutex, Notify};
use tonic::service::InterceptorLayer;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tracing::Instrument as _;

use crate::fabric::arrow_result_resource::QueryExecutionPin;
use crate::fabric::command::{LeaseId, PrincipalId, WorkspaceId};
use crate::fabric::published_arrow_result::{
    OpaqueResultLeaseToken, PublishedArrowResultDescriptor, PublishedArrowResultRegistry,
    PublishedArtifactId, PublishedReleaseOutcome, PublishedResultAccess, PublishedResultOwner,
    PublishedResultReadRequest, PublishedResultRegistryError, PublishedResultResourceId,
};
use crate::fabric::relational_query_runtime::RelationalQueryPublication;
use crate::fabric::{
    QueryExecutionArtifactAccumulator, QueryExecutionArtifactEvidence, QueryExecutionContext,
};
use crate::freshness::{FreshnessAdmission, FreshnessBarrier, FreshnessError, FreshnessState};
use crate::identity::{
    IdentityDomain, SemanticFingerprintDomain, decode_public_id, semantic_fingerprint,
};
use crate::integrity::{frame_digest, framed_digest};
use crate::operational_store::{OperationalStore, QueryExecutionTerminalRecord};
use crate::registries::CpgdFeatureMask;
use crate::rpc::generated::codefabric::cpgd::v1::cpg_query_service_server::CpgQueryService;
use crate::rpc::generated::codefabric::cpgd::v1::cpg_query_service_server::CpgQueryServiceServer;
use crate::rpc::generated::codefabric::cpgd::v1::query_event::Event;
use crate::rpc::generated::codefabric::cpgd::v1::{
    ArtifactReadyEvent, AttachQueryRequest, BundleIdentity, CancelQueryRequest,
    CancelQueryResponse, CancellationState, DeliveryPreference, EffectiveLimitsProfile,
    HandshakeRequest, HandshakeResponse, HostCapabilityProfile, PayloadCompression, QueryEvent,
    QueryEventHeader, QueryExecutionState, ReadResultRequest, ReadinessSummary,
    ReleaseResultRequest, ReleaseResultResponse, ResultChunk, SchemaFingerprint,
    SnapshotPinnedEvent, StartQueryRequest, StartQueryResponse, StatusRequest, StatusResponse,
    StreamQueryRequest, TerminalEvent, ValidateQueryRequest, ValidateQueryResponse, WorkspaceClaim,
    WorkspaceReadiness,
};
use crate::rpc::{
    AuthorizedUnixStream, MAX_CONTROL_MESSAGE_BYTES, MAX_PAYLOAD_CHUNK_BYTES, SameUserInterceptor,
    negotiate_feature_bits,
};
use crate::security::{KeyedAuthenticator, SecurityMacDomain, local_token_digest};
use crate::semantic_query_contract::QueryForm;
use crate::semantic_query_contract::{
    FreshnessPolicy, ParsedSemanticRequest, SemanticQueryError, SemanticSnapshotResponse,
    parse_request,
};

const RESULT_LEASE_SECONDS: i64 = 1_800;
const MAX_AGENT_INSTANCE_ID_BYTES: usize = 256;
const PRINCIPAL_DERIVATION_DOMAIN: &[u8] = b"codefabric.query-principal.v1\0";
const EXECUTION_PIN_DERIVATION_DOMAIN: &[u8] = b"codefabric.query-execution-pin.v1\0";
const RESULT_LEASE_ID_DERIVATION_DOMAIN: &[u8] = b"codefabric.result-lease-id.v1\0";
const ARROW_RESULT_TOKEN_CONTEXT: &[u8] = b"codefabric.published-arrow-result-token.v1\0";
const PROGRAMMATIC_QUERY_CONTRACT_ID: &str = "codefabric.programmatic-query-contract";
const PROGRAMMATIC_QUERY_CONTRACT_VERSION: &str = "programmatic-v1";
const PROGRAMMATIC_QUERY_CONTRACT_DIGEST_DOMAIN: &[u8] =
    b"codefabric.programmatic-query-contract.v1\0";
const PUBLISHED_RESULT_DESCRIPTOR_MEDIA_TYPE: &str =
    "application/vnd.codefabric.arrow-result-package+json";

type QueryStream = Pin<Box<dyn Stream<Item = Result<QueryEvent, Status>> + Send>>;
type ArtifactStream = Pin<Box<dyn Stream<Item = Result<ResultChunk, Status>> + Send>>;

pub(crate) fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
        })
}

/// Project one registry-issued Arrow publication into the released query event envelope.
///
/// The descriptor is canonical control metadata only. Every semantic row remains in the
/// owner-bound manifest/relation resources named by that descriptor.
pub fn published_arrow_artifact_ready_event(
    header: QueryEventHeader,
    descriptor: &PublishedArrowResultDescriptor,
    lease_token: &OpaqueResultLeaseToken,
) -> Result<ArtifactReadyEvent, Status> {
    let canonical_result_descriptor_json = descriptor
        .canonical_control_bytes()
        .map_err(published_result_status)?;
    let result_descriptor_checksum = framed_digest(&canonical_result_descriptor_json);
    Ok(ArtifactReadyEvent {
        header: Some(header),
        artifact_id: descriptor.artifact_id.public_id(),
        artifact_checksum: result_descriptor_checksum.clone(),
        content_type: PUBLISHED_RESULT_DESCRIPTOR_MEDIA_TYPE.to_owned(),
        encoding: PayloadCompression::Identity as i32,
        lease_expires_at_unix_ms: descriptor.lease_expires_at_unix_ms,
        lease_token: lease_token.public_token(),
        canonical_result_descriptor_json,
        result_descriptor_checksum,
        result_contract_version: descriptor.format.to_owned(),
        arrow_release: crate::fabric::arrow_result_resource::ARROW_RELEASE.to_owned(),
    })
}

fn execution_identity(request: &StartQueryRequest, sequence: u64, accepted_at: i64) -> String {
    let mut input = Vec::new();
    for field in [
        request.agent_instance_id.as_bytes(),
        request.workspace_id.as_bytes(),
        request.mcp_call_id.as_bytes(),
        request.rpc_attempt_id.as_bytes(),
        request.idempotency_key.as_bytes(),
        request.request_checksum.as_bytes(),
    ] {
        input.extend_from_slice(&(field.len() as u64).to_be_bytes());
        input.extend_from_slice(field);
    }
    input.extend_from_slice(&sequence.to_be_bytes());
    input.extend_from_slice(&accepted_at.to_be_bytes());
    format!("execution:{}", &framed_digest(&input)[3..35])
}

/// Durable terminal phase of one query execution artifact bundle.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QueryArtifactPhase {
    Succeeded,
    Failed,
    Cancelled,
}

/// Versioned persisted join between request identity, serving plans, metrics, and result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedQueryArtifactBundle {
    pub artifact_schema_version: String,
    pub workspace_id: String,
    pub execution: QueryExecutionContext,
    pub phase: QueryArtifactPhase,
    pub evidence: QueryExecutionArtifactEvidence,
    pub result_artifact_id: Option<String>,
    pub public_error_code: Option<String>,
    pub created_at_unix_ms: i64,
    pub expires_at_unix_ms: i64,
}

/// Restart reconciliation evidence for the terminal journal and its payload projections.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct QueryArtifactRecoveryReport {
    pub removed_staged_payloads: usize,
    pub verified_terminal_records: usize,
}

#[derive(Clone, Debug)]
pub struct ResultArtifactStore {
    root: PathBuf,
    lease_secret: [u8; 32],
    terminal_journal: Arc<std::sync::Mutex<OperationalStore>>,
}

impl ResultArtifactStore {
    /// Create a private immutable result root.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the private root cannot be created or operating-system entropy
    /// cannot be read.
    pub fn new(root: PathBuf) -> Result<Self, std::io::Error> {
        fs::create_dir_all(&root)?;
        fs::create_dir_all(root.join("query-plan-artifacts"))?;
        let terminal_journal = OperationalStore::open(&root.join("query-terminal.sqlite"))
            .map_err(std::io::Error::other)?;
        let mut lease_secret = [0_u8; 32];
        fs::File::open("/dev/urandom")?.read_exact(&mut lease_secret)?;
        let store = Self {
            root,
            lease_secret,
            terminal_journal: Arc::new(std::sync::Mutex::new(terminal_journal)),
        };
        store.reconcile_terminal_artifacts()?;
        Ok(store)
    }

    fn reconcile_terminal_artifacts(&self) -> Result<QueryArtifactRecoveryReport, std::io::Error> {
        let mut report = QueryArtifactRecoveryReport::default();
        let plan_artifact_root = self.root.join("query-plan-artifacts");
        for directory in [self.root.as_path(), plan_artifact_root.as_path()] {
            for entry in fs::read_dir(directory)? {
                let entry = entry?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with('.') && name.ends_with(".tmp") {
                    fs::remove_file(entry.path())?;
                    report.removed_staged_payloads =
                        report.removed_staged_payloads.saturating_add(1);
                }
            }
        }
        let journal = self
            .terminal_journal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let records = journal
            .query_execution_terminals(100_001)
            .map_err(std::io::Error::other)?;
        if records.len() > 100_000 {
            return Err(std::io::Error::other(
                "query terminal recovery exceeds the bounded journal census",
            ));
        }
        for record in records {
            if !journal
                .query_execution_terminal_lease_matches(&record)
                .map_err(std::io::Error::other)?
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "query terminal lease differs from journal authority for {}",
                        record.execution_id
                    ),
                ));
            }
            if let Some(fallback) = &record.fallback_envelope_bytes {
                if framed_digest(fallback) != record.bundle_checksum {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "query terminal fallback differs from journal authority for {}",
                            record.execution_id
                        ),
                    ));
                }
            } else if record.expires_at > now_millis() {
                let uri = record.primary_payload_uri.as_ref().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "query terminal has no retained payload for {}",
                            record.execution_id
                        ),
                    )
                })?;
                let payload = fs::read(uri)?;
                if framed_digest(&payload) != record.bundle_checksum {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "query terminal primary payload differs for {}",
                            record.execution_id
                        ),
                    ));
                }
            }
            report.verified_terminal_records = report.verified_terminal_records.saturating_add(1);
        }
        Ok(report)
    }

    fn query_handle_token(
        &self,
        domain: SecurityMacDomain,
        query_id: &str,
        agent_id: &str,
        workspace_id: &str,
    ) -> Vec<u8> {
        let mut authenticator = KeyedAuthenticator::new(&self.lease_secret, domain);
        for field in [query_id, agent_id, workspace_id] {
            authenticator.update(&(field.len() as u64).to_be_bytes());
            authenticator.update(field.as_bytes());
        }
        authenticator.finalize().to_vec()
    }

    fn query_artifact_path(&self, bundle_checksum: &str) -> PathBuf {
        self.root.join("query-plan-artifacts").join(format!(
            "{}.json",
            bundle_checksum.trim_start_matches("b3:")
        ))
    }

    #[allow(clippy::too_many_lines)] // Keeps payload, fallback, and journal ordering in one auditable transaction boundary.
    pub(crate) fn persist_query_artifact(
        &self,
        artifact: &PersistedQueryArtifactBundle,
    ) -> Result<QueryArtifactPhase, Status> {
        const MAX_FALLBACK_BYTES: usize = 16 * 1024 * 1024;
        let mut persisted = artifact.clone();
        let value = serde_json::to_value(&persisted)
            .map_err(|_| Status::internal("query artifact serialization failed"))?;
        let bytes = crate::contracts::jcs::canonicalize_value(&value).map_err(|error| {
            Status::internal(format!("query artifact canonicalization failed: {error}"))
        })?;
        let bundle_checksum = framed_digest(&bytes);
        let final_path = self.query_artifact_path(&bundle_checksum);
        let primary = (|| -> Result<(), Status> {
            if final_path.exists() {
                let existing = fs::read(&final_path)
                    .map_err(|_| Status::internal("query artifact read failed"))?;
                if existing == bytes {
                    return Ok(());
                }
                return Err(Status::data_loss("query artifact checksum collision"));
            }
            let file_name = final_path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| Status::internal("query artifact filename is invalid"))?;
            let temporary =
                final_path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)
                .map_err(|_| Status::internal("query artifact staging failed"))?;
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|_| Status::internal("query artifact staging write failed"))?;
            fs::rename(&temporary, &final_path)
                .map_err(|_| Status::internal("query artifact publication failed"))?;
            Ok(())
        })();
        let (primary_payload_uri, payload_status, fallback_envelope_bytes) = match primary {
            Ok(()) => (
                Some(final_path.to_string_lossy().into_owned()),
                "PRIMARY_AVAILABLE".to_owned(),
                None,
            ),
            Err(error) if error.code() != tonic::Code::DataLoss => {
                persisted.phase = QueryArtifactPhase::Failed;
                persisted.public_error_code = Some("INTERNAL".to_owned());
                "terminal_persistence".clone_into(&mut persisted.evidence.lifecycle_phase);
                persisted.evidence.failing_stage = Some("artifact_persistence".to_owned());
                let fallback = crate::contracts::jcs::canonicalize_value(
                    &serde_json::to_value(&persisted)
                        .map_err(|_| Status::internal("fallback serialization failed"))?,
                )
                .map_err(|_| Status::internal("fallback canonicalization failed"))?;
                if fallback.len() > MAX_FALLBACK_BYTES {
                    return Err(Status::resource_exhausted(
                        "query terminal fallback exceeds the bounded SQLite envelope",
                    ));
                }
                (None, "FALLBACK_ONLY".to_owned(), Some(fallback))
            }
            Err(error) => return Err(error),
        };
        let source_table_versions_bytes = crate::contracts::jcs::canonicalize_value(
            &serde_json::to_value(&persisted.evidence.source_table_versions)
                .map_err(|_| Status::internal("source table pin serialization failed"))?,
        )
        .map_err(|_| Status::internal("source table pin canonicalization failed"))?;
        let record = QueryExecutionTerminalRecord {
            execution_id: persisted.execution.execution_id.clone(),
            workspace_id: persisted.workspace_id.as_bytes().to_vec(),
            semantic_request_id: persisted.execution.semantic_request_id.clone(),
            mcp_call_id: persisted.execution.mcp_call_id.clone(),
            terminal_phase: format!("{:?}", persisted.phase).to_ascii_uppercase(),
            failing_stage: persisted.evidence.failing_stage.clone(),
            bundle_checksum: fallback_envelope_bytes.as_ref().map_or_else(
                || bundle_checksum.clone(),
                |fallback| framed_digest(fallback),
            ),
            primary_payload_uri,
            payload_status,
            fallback_envelope_bytes,
            snapshot_id: persisted.evidence.snapshot_id.clone(),
            publication_id: persisted.evidence.publication_id.clone(),
            source_table_versions_bytes,
            created_at: persisted.created_at_unix_ms,
            expires_at: persisted.expires_at_unix_ms,
        };
        self.terminal_journal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .commit_query_execution_terminal(&record)
            .map(|()| persisted.phase)
            .map_err(|error| match error {
                crate::operational_store::OperationalStoreError::QueryExecutionTerminalRecord(
                    _,
                ) => Status::already_exists(
                    "query execution already has a different terminal meaning",
                ),
                other => {
                    Status::unavailable(format!("query terminal journal commit failed: {other}"))
                }
            })
    }

    /// Read and validate one execution-scoped artifact bundle from durable storage.
    ///
    /// # Errors
    ///
    /// Returns an I/O or data-loss status when the artifact is absent or malformed.
    pub fn read_query_artifact(
        &self,
        execution_id: &str,
    ) -> Result<PersistedQueryArtifactBundle, Status> {
        let record = self
            .terminal_journal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .read_query_execution_terminal(execution_id)
            .map_err(|_| Status::unavailable("query terminal journal read failed"))?
            .ok_or_else(|| Status::not_found("query terminal record not found"))?;
        if record.expires_at <= now_millis() {
            return Err(Status::failed_precondition(
                "query provenance gap: terminal payload lease expired",
            ));
        }
        let bytes = if let Some(uri) = &record.primary_payload_uri {
            match fs::read(uri) {
                Ok(bytes) => bytes,
                Err(_) => record.fallback_envelope_bytes.clone().ok_or_else(|| {
                    Status::data_loss("query provenance gap: primary payload is missing")
                })?,
            }
        } else {
            record.fallback_envelope_bytes.clone().ok_or_else(|| {
                Status::data_loss("query provenance gap: fallback payload is missing")
            })?
        };
        if framed_digest(&bytes) != record.bundle_checksum {
            return Err(Status::data_loss(
                "query terminal payload checksum differs from journal authority",
            ));
        }
        let artifact: PersistedQueryArtifactBundle = serde_json::from_slice(&bytes)
            .map_err(|_| Status::data_loss("query artifact schema is invalid"))?;
        if artifact.execution.execution_id != execution_id {
            return Err(Status::data_loss(
                "query artifact execution identity differs",
            ));
        }
        Ok(artifact)
    }

    /// Remove only query artifact bundles whose declared retention lease has expired.
    ///
    /// # Errors
    ///
    /// Returns an I/O or data-loss status instead of treating an unreadable artifact as expired.
    pub fn prune_expired_query_artifacts(&self, observed_at_unix_ms: i64) -> Result<usize, Status> {
        let mut removed = 0_usize;
        let entries = fs::read_dir(self.root.join("query-plan-artifacts"))
            .map_err(|_| Status::internal("query artifact directory read failed"))?;
        for entry in entries {
            let entry = entry.map_err(|_| Status::internal("query artifact entry read failed"))?;
            if entry
                .path()
                .extension()
                .is_none_or(|extension| extension != "json")
            {
                continue;
            }
            let bytes = fs::read(entry.path())
                .map_err(|_| Status::internal("query artifact retention read failed"))?;
            let artifact: PersistedQueryArtifactBundle = serde_json::from_slice(&bytes)
                .map_err(|_| Status::data_loss("query artifact retention schema is invalid"))?;
            if artifact.expires_at_unix_ms <= observed_at_unix_ms {
                fs::remove_file(entry.path())
                    .map_err(|_| Status::internal("query artifact retention removal failed"))?;
                removed = removed.saturating_add(1);
            }
        }
        Ok(removed)
    }
}

#[async_trait]
pub trait SemanticQueryBackend: Send + Sync + 'static {
    /// Apply the execution-capability policy owned by this backend.
    ///
    /// The service supplies only a strictly decoded, canonical released envelope. The backend is
    /// authoritative for semantic typing and whether the request has a complete executable
    /// realization in its installed epoch/catalog; a generated predecessor registry is not
    /// service authority.
    fn validate_execution_request(
        &self,
        request: &ParsedSemanticRequest,
    ) -> Result<(), SemanticQueryError>;

    async fn execute(
        &self,
        request: ParsedSemanticRequest,
        freshness: FreshnessState,
        cancellation: crate::cancellation::Cancellation,
        context: SemanticBackendExecutionContext,
        artifacts: QueryExecutionArtifactAccumulator,
    ) -> SemanticBackendOutcome;

    async fn public_snapshot(
        &self,
        workspace_id: &str,
    ) -> Result<SemanticSnapshotResponse, SemanticQueryError>;
}

/// Authenticated delivery identity and immutable execution authorities supplied to one backend.
///
/// The agent/workspace strings are the exact identities already authenticated and authorized by
/// the RPC boundary. The registry is the daemon-wide instance also used by `ReadResult` and
/// `ReleaseResult`; a relational backend must publish into this instance rather than constructing
/// an isolated result registry.
#[derive(Clone, Debug)]
pub struct SemanticBackendExecutionContext {
    execution: QueryExecutionContext,
    agent_instance_id: Arc<str>,
    workspace_id: Arc<str>,
    owner: PublishedResultOwner,
    query_execution_pin: QueryExecutionPin,
    result_lease_id: LeaseId,
    result_lease_token: OpaqueResultLeaseToken,
    published_results: Arc<PublishedArrowResultRegistry>,
}

impl SemanticBackendExecutionContext {
    fn new(
        execution: QueryExecutionContext,
        agent_instance_id: impl Into<Arc<str>>,
        workspace_id: impl Into<Arc<str>>,
        owner: PublishedResultOwner,
        query_execution_pin: QueryExecutionPin,
        result_lease_id: LeaseId,
        result_lease_token: OpaqueResultLeaseToken,
        published_results: Arc<PublishedArrowResultRegistry>,
    ) -> Self {
        Self {
            execution,
            agent_instance_id: agent_instance_id.into(),
            workspace_id: workspace_id.into(),
            owner,
            query_execution_pin,
            result_lease_id,
            result_lease_token,
            published_results,
        }
    }

    #[must_use]
    pub const fn execution(&self) -> &QueryExecutionContext {
        &self.execution
    }

    #[must_use]
    pub fn agent_instance_id(&self) -> &str {
        &self.agent_instance_id
    }

    #[must_use]
    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    /// Return the typed owner established from the authenticated credential and workspace claim.
    #[must_use]
    pub const fn owner(&self) -> PublishedResultOwner {
        self.owner
    }

    /// Return the deterministic execution pin for the accepted query and authenticated owner.
    #[must_use]
    pub const fn query_execution_pin(&self) -> QueryExecutionPin {
        self.query_execution_pin
    }

    /// Return the distinct internal lease identity allocated for this result publication.
    #[must_use]
    pub const fn result_lease_id(&self) -> LeaseId {
        self.result_lease_id
    }

    /// Return the opaque serving credential allocated for this result publication.
    #[must_use]
    pub const fn result_lease_token(&self) -> OpaqueResultLeaseToken {
        self.result_lease_token
    }

    #[must_use]
    pub fn published_results(&self) -> Arc<PublishedArrowResultRegistry> {
        Arc::clone(&self.published_results)
    }
}

/// Successful relational publication plus the control metadata needed by the RPC boundary.
///
/// Semantic rows remain owned by the Arrow registry. This value carries no row serialization and
/// deliberately keeps the separately issued opaque lease credential out of the public descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedArrowSemanticSuccess {
    publication: RelationalQueryPublication,
    lease_token: OpaqueResultLeaseToken,
    snapshot: SemanticSnapshotResponse,
    evidence: QueryExecutionArtifactEvidence,
}

impl PublishedArrowSemanticSuccess {
    #[must_use]
    pub const fn new(
        publication: RelationalQueryPublication,
        lease_token: OpaqueResultLeaseToken,
        snapshot: SemanticSnapshotResponse,
        evidence: QueryExecutionArtifactEvidence,
    ) -> Self {
        Self {
            publication,
            lease_token,
            snapshot,
            evidence,
        }
    }

    #[must_use]
    pub const fn publication(&self) -> &RelationalQueryPublication {
        &self.publication
    }

    #[must_use]
    pub const fn descriptor(&self) -> &PublishedArrowResultDescriptor {
        self.publication.descriptor()
    }

    #[must_use]
    pub const fn lease_token(&self) -> &OpaqueResultLeaseToken {
        &self.lease_token
    }

    #[must_use]
    pub const fn snapshot(&self) -> &SemanticSnapshotResponse {
        &self.snapshot
    }

    #[must_use]
    pub const fn evidence(&self) -> &QueryExecutionArtifactEvidence {
        &self.evidence
    }
}

/// Closed terminal outcome of the programmatic relational backend.
#[derive(Debug)]
pub enum SemanticBackendOutcome {
    PublishedArrow(PublishedArrowSemanticSuccess),
    Failed {
        error: SemanticQueryError,
        evidence: QueryExecutionArtifactEvidence,
    },
    Cancelled {
        error: SemanticQueryError,
        evidence: QueryExecutionArtifactEvidence,
    },
}

/// Capability-token identity and the exact workspace claims granted to one adapter profile.
#[derive(Clone, Debug)]
pub struct QueryAuthorization {
    token_digest: [u8; 32],
    claims: BTreeMap<String, WorkspaceClaim>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkspacePermission {
    Query,
}

impl WorkspacePermission {
    const fn claim(self) -> &'static str {
        match self {
            Self::Query => "query",
        }
    }
}

impl QueryAuthorization {
    /// Construct a non-empty local capability and closed claim set.
    ///
    /// # Errors
    ///
    /// Rejects empty token material, duplicate/empty workspace identities, or empty permissions.
    pub fn new(capability_token: &[u8], claims: Vec<WorkspaceClaim>) -> Result<Self, Status> {
        if capability_token.len() < 16 || capability_token.len() > 4_096 {
            return Err(Status::invalid_argument(
                "capability token length is outside the local profile",
            ));
        }
        let mut by_workspace = BTreeMap::new();
        for claim in claims {
            let mut permissions = claim.permission_claims.iter().collect::<Vec<_>>();
            permissions.sort_unstable();
            if claim.workspace_id.is_empty()
                || !claim
                    .permission_claims
                    .iter()
                    .any(|permission| permission == WorkspacePermission::Query.claim())
                || permissions.windows(2).any(|pair| pair[0] == pair[1])
                || by_workspace
                    .insert(claim.workspace_id.clone(), claim)
                    .is_some()
            {
                return Err(Status::invalid_argument(
                    "workspace claims are empty or duplicated",
                ));
            }
        }
        Ok(Self {
            token_digest: capability_digest(capability_token),
            claims: by_workspace,
        })
    }

    fn authorize_handshake(
        &self,
        request: &HandshakeRequest,
    ) -> Result<Vec<WorkspaceClaim>, Status> {
        validate_agent_instance_id(&request.agent_instance_id)?;
        if request.adapter_instance_id != request.agent_instance_id {
            return Err(Status::unauthenticated(
                "adapter and agent instance identities differ",
            ));
        }
        let proof = request
            .credential_proof
            .as_ref()
            .ok_or_else(|| Status::unauthenticated("capability proof is missing"))?;
        if proof.credential_id != request.agent_instance_id
            || !constant_time_equal(
                &self.token_digest,
                &capability_digest(&proof.capability_token),
            )
        {
            return Err(Status::unauthenticated("capability proof differs"));
        }
        if request.desired_workspace_ids.is_empty() {
            return Err(Status::invalid_argument("desired workspace set is empty"));
        }
        let desired = request
            .desired_workspace_ids
            .iter()
            .collect::<BTreeSet<_>>();
        if desired.len() != request.desired_workspace_ids.len() {
            return Err(Status::invalid_argument(
                "desired workspace set contains duplicates",
            ));
        }
        request
            .desired_workspace_ids
            .iter()
            .map(|workspace_id| {
                self.claims
                    .get(workspace_id)
                    .cloned()
                    .ok_or_else(|| Status::permission_denied("workspace is not authorized"))
            })
            .collect()
    }

    fn authorize_workspace(
        &self,
        workspace_id: &str,
        permission: WorkspacePermission,
    ) -> Result<(), Status> {
        self.claims
            .get(workspace_id)
            .filter(|claim| {
                claim
                    .permission_claims
                    .iter()
                    .any(|candidate| candidate == permission.claim())
            })
            .map(|_| ())
            .ok_or_else(|| Status::permission_denied("workspace action is not authorized"))
    }

    /// Derive the backend-only execution capabilities from an already installed local
    /// authorization. Kept crate-private so production composition tests and the RPC boundary use
    /// the same derivation without exposing raw result-lease construction outside the daemon.
    pub(crate) fn backend_execution_context(
        &self,
        execution: QueryExecutionContext,
        agent_instance_id: &str,
        workspace_id: &str,
        published_results: Arc<PublishedArrowResultRegistry>,
    ) -> Result<SemanticBackendExecutionContext, Status> {
        validate_agent_instance_id(agent_instance_id)?;
        let workspace_bytes = decode_public_id(IdentityDomain::Workspace, None, workspace_id)
            .map_err(|_| Status::failed_precondition("authorized workspace identity is invalid"))?;
        let workspace = WorkspaceId::from_bytes(workspace_bytes);

        let principal_digest = keyed_context_digest(
            &self.token_digest,
            PRINCIPAL_DERIVATION_DOMAIN,
            &[agent_instance_id.as_bytes()],
        );
        let principal_bytes = first_16(principal_digest);
        if principal_bytes.iter().all(|byte| *byte == 0) {
            return Err(Status::internal(
                "authenticated principal derivation produced a reserved identity",
            ));
        }
        let principal = PrincipalId::from_bytes(principal_bytes);
        let owner = PublishedResultOwner::new(workspace, principal);

        let query_execution_digest = keyed_context_digest(
            &self.token_digest,
            EXECUTION_PIN_DERIVATION_DOMAIN,
            &[
                workspace.as_bytes(),
                principal.as_bytes(),
                execution.execution_id.as_bytes(),
                execution.semantic_request_id.as_bytes(),
                execution.mcp_call_id.as_bytes(),
            ],
        );
        let query_execution_pin = QueryExecutionPin::from_bytes(query_execution_digest);
        let lease_bytes = first_16(keyed_context_digest(
            &self.token_digest,
            RESULT_LEASE_ID_DERIVATION_DOMAIN,
            &[query_execution_pin.as_bytes()],
        ));
        if lease_bytes.iter().all(|byte| *byte == 0) {
            return Err(Status::internal(
                "result lease derivation produced a reserved identity",
            ));
        }
        let result_lease_id = LeaseId::from_bytes(lease_bytes);
        let mut token = KeyedAuthenticator::new(&self.token_digest, SecurityMacDomain::ResultLease);
        for field in [
            ARROW_RESULT_TOKEN_CONTEXT,
            workspace.as_bytes(),
            principal.as_bytes(),
            query_execution_pin.as_bytes(),
            result_lease_id.as_bytes(),
        ] {
            token.update(&(field.len() as u64).to_be_bytes());
            token.update(field);
        }
        let result_lease_token = OpaqueResultLeaseToken::try_from_bytes(token.finalize())
            .map_err(|_| Status::internal("result lease token derivation failed"))?;

        Ok(SemanticBackendExecutionContext::new(
            execution,
            agent_instance_id,
            workspace_id,
            owner,
            query_execution_pin,
            result_lease_id,
            result_lease_token,
            published_results,
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NegotiatedQuerySession {
    host_profile_digest: String,
    authorized_workspaces: BTreeSet<String>,
}

impl NegotiatedQuerySession {
    fn new(host_profile_digest: String, claims: &[WorkspaceClaim]) -> Self {
        Self {
            host_profile_digest,
            authorized_workspaces: claims
                .iter()
                .map(|claim| claim.workspace_id.clone())
                .collect(),
        }
    }

    fn authorize(
        &self,
        workspace_id: &str,
        host_profile_digest: Option<&str>,
    ) -> Result<(), Status> {
        if !self.authorized_workspaces.contains(workspace_id) {
            return Err(Status::permission_denied(
                "workspace is outside the negotiated session",
            ));
        }
        if host_profile_digest.is_some_and(|digest| digest != self.host_profile_digest) {
            return Err(Status::failed_precondition(
                "query host profile differs from the negotiated session",
            ));
        }
        Ok(())
    }
}

fn validate_agent_instance_id(value: &str) -> Result<(), Status> {
    if value.is_empty()
        || value.len() > MAX_AGENT_INSTANCE_ID_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(Status::invalid_argument(
            "agent instance identity is empty, oversized, or noncanonical",
        ));
    }
    Ok(())
}

fn keyed_context_digest(key: &[u8; 32], domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_keyed(key);
    hasher.update(domain);
    for field in fields {
        hasher.update(&(field.len() as u64).to_be_bytes());
        hasher.update(field);
    }
    *hasher.finalize().as_bytes()
}

fn first_16(value: [u8; 32]) -> [u8; 16] {
    let mut prefix = [0; 16];
    prefix.copy_from_slice(&value[..16]);
    prefix
}

fn capability_digest(token: &[u8]) -> [u8; 32] {
    local_token_digest(SecurityMacDomain::LocalCapabilityToken, token)
}

fn limits_profile_digest(values: [u64; 5]) -> String {
    let mut fingerprint = semantic_fingerprint(SemanticFingerprintDomain::LocalQueryLimits);
    for value in values {
        fingerprint.update(&value.to_be_bytes());
    }
    frame_digest(fingerprint.finalize())
}

fn effective_limits_profile() -> EffectiveLimitsProfile {
    let values = [
        MAX_CONTROL_MESSAGE_BYTES as u64,
        MAX_PAYLOAD_CHUNK_BYTES as u64,
        MAX_PAYLOAD_CHUNK_BYTES as u64,
        4,
        300,
    ];
    EffectiveLimitsProfile {
        maximum_control_message_bytes: values[0],
        maximum_payload_chunk_bytes: values[1],
        maximum_inline_response_bytes: values[2],
        maximum_concurrent_queries: u32::try_from(values[3]).expect("constant fits u32"),
        query_orphan_replay_seconds: u32::try_from(values[4]).expect("constant fits u32"),
        profile_digest: limits_profile_digest(values),
    }
}

pub(crate) fn host_capability_profile_digest(
    profile: &HostCapabilityProfile,
) -> Result<String, Status> {
    let mut delivery_modes = profile
        .delivery_modes
        .iter()
        .map(|value| match DeliveryPreference::try_from(*value) {
            Ok(DeliveryPreference::Inline) => Ok("inline"),
            Ok(DeliveryPreference::Resource) => Ok("resource"),
            Ok(DeliveryPreference::Auto) => Ok("automatic"),
            _ => Err(Status::failed_precondition(
                "host capability profile contains an unsupported delivery mode",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    delivery_modes.sort_unstable();
    if delivery_modes.is_empty() || delivery_modes.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Status::failed_precondition(
            "host capability delivery modes are empty or duplicated",
        ));
    }
    let mut compression_algorithms = profile
        .compression_algorithms
        .iter()
        .map(|value| match PayloadCompression::try_from(*value) {
            Ok(PayloadCompression::Identity) => Ok("identity"),
            Ok(PayloadCompression::Zstd) => Ok("zstd"),
            _ => Err(Status::failed_precondition(
                "host capability profile contains an unsupported compression",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    compression_algorithms.sort_unstable();
    if compression_algorithms.is_empty()
        || compression_algorithms
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        || !compression_algorithms.contains(&"identity")
        || profile.maximum_frame_bytes == 0
        || profile.maximum_frame_bytes > MAX_CONTROL_MESSAGE_BYTES as u64
    {
        return Err(Status::failed_precondition(
            "host capability profile is empty, duplicated, or outside service limits",
        ));
    }
    let value = serde_json::json!({
        "compression_algorithms": compression_algorithms,
        "delivery_modes": delivery_modes,
        "maximum_frame_bytes": profile.maximum_frame_bytes,
        "supports_resource_links": profile.supports_resource_links,
        "supports_trace_context": profile.supports_trace_context,
    });
    let canonical = crate::contracts::jcs::canonicalize_value(&value)
        .map_err(|error| Status::internal(format!("host profile canonicalization: {error}")))?;
    Ok(framed_digest(&canonical))
}

fn constant_time_equal(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn published_access(
    artifact_id: &str,
    lease_token: &str,
    owner: Option<&crate::rpc::generated::codefabric::cpgd::v1::ResultOwner>,
) -> Result<PublishedResultAccess, Status> {
    let owner = owner.ok_or_else(|| {
        Status::invalid_argument("Arrow result operations require an explicit result owner")
    })?;
    Ok(PublishedResultAccess {
        artifact_id: PublishedArtifactId::try_from_public_id(artifact_id)
            .map_err(published_result_status)?,
        owner: PublishedResultOwner::try_from_public_ids(&owner.workspace_id, &owner.agent_id)
            .map_err(published_result_status)?,
        lease_token: OpaqueResultLeaseToken::try_from_public_token(lease_token)
            .map_err(published_result_status)?,
    })
}

fn published_result_status(error: PublishedResultRegistryError) -> Status {
    match error {
        PublishedResultRegistryError::InvalidPublicIdentity
        | PublishedResultRegistryError::InvalidOpaqueToken => {
            Status::invalid_argument(error.to_string())
        }
        PublishedResultRegistryError::UnknownArtifact(_)
        | PublishedResultRegistryError::UnknownResource(_) => Status::not_found(error.to_string()),
        PublishedResultRegistryError::WrongOwner
        | PublishedResultRegistryError::WrongOpaqueToken => {
            Status::permission_denied(error.to_string())
        }
        PublishedResultRegistryError::Released | PublishedResultRegistryError::Expired => {
            Status::failed_precondition(error.to_string())
        }
        PublishedResultRegistryError::Package(
            crate::fabric::arrow_result_resource::ArrowResultResourceError::ChunkLimitExceeded {
                ..
            },
        ) => Status::resource_exhausted(error.to_string()),
        _ => Status::internal(error.to_string()),
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct PublicStatusView {
    ready: bool,
    workspace_id: String,
    agent_instance_id: String,
    snapshot: SemanticSnapshotResponse,
    versions: BTreeMap<String, String>,
    supported_languages: Vec<String>,
    supported_request_forms: Vec<String>,
    capability_statuses: Vec<BTreeMap<String, String>>,
    freshness_state: &'static str,
    service_limits: PublicServiceLimits,
    notices: Vec<String>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_field_names)] // The public JSON contract intentionally uses the registered maximum_* field names.
struct PublicServiceLimits {
    maximum_control_message_bytes: u64,
    maximum_payload_chunk_bytes: u64,
    maximum_concurrent_queries: u32,
}

#[derive(Clone, Debug, Default)]
struct QueryHandleState {
    events: Vec<QueryEvent>,
    terminal_state: Option<QueryExecutionState>,
}

#[derive(Debug)]
struct QueryHandle {
    execution: QueryExecutionContext,
    artifacts: QueryExecutionArtifactAccumulator,
    resume_token: Vec<u8>,
    cancel_token: Vec<u8>,
    agent_instance_id: String,
    workspace_id: String,
    cancelled: Arc<AtomicBool>,
    state: Mutex<QueryHandleState>,
    changed: Notify,
}

pub struct ProductionQueryService<B> {
    backend: Arc<B>,
    authorization: QueryAuthorization,
    artifacts: Arc<ResultArtifactStore>,
    published_results: Arc<PublishedArrowResultRegistry>,
    handles: Arc<Mutex<BTreeMap<String, Arc<QueryHandle>>>>,
    idempotency: Arc<Mutex<BTreeMap<String, String>>>,
    negotiated_sessions: Arc<Mutex<BTreeMap<String, NegotiatedQuerySession>>>,
    freshness: FreshnessBarrier,
    freshness_timeout: std::time::Duration,
    query_contract: BundleIdentity,
    tasks: Arc<Mutex<BTreeMap<String, tokio::task::JoinHandle<()>>>>,
    execution_sequence: AtomicU64,
}

fn valid_bundle_digest(value: &str) -> bool {
    value.len() == 67
        && value.starts_with("b3:")
        && value[3..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn query_contract_digest<'a>(forms: impl IntoIterator<Item = &'a str>) -> String {
    let mut preimage = Vec::from(PROGRAMMATIC_QUERY_CONTRACT_DIGEST_DOMAIN);
    for form in forms {
        let bytes = form.as_bytes();
        preimage.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        preimage.extend_from_slice(bytes);
    }
    framed_digest(&preimage)
}

/// Project the typed programmatic query contract into the released `BundleIdentity` wire
/// envelope. The Protobuf field name is retained for compatibility; no generated bundle,
/// package, registry, or runtime-selected model participates in query authority.
fn programmatic_query_contract_identity() -> BundleIdentity {
    let identity = BundleIdentity {
        bundle_id: PROGRAMMATIC_QUERY_CONTRACT_ID.to_owned(),
        bundle_version: PROGRAMMATIC_QUERY_CONTRACT_VERSION.to_owned(),
        bundle_digest: query_contract_digest(QueryForm::ALL.into_iter().map(QueryForm::slug)),
    };
    assert!(valid_bundle_digest(&identity.bundle_digest));
    identity
}

fn supported_query_forms() -> Vec<String> {
    QueryForm::ALL
        .into_iter()
        .map(|form| form.slug().to_owned())
        .collect()
}

impl<B> ProductionQueryService<B> {
    pub fn new(
        backend: Arc<B>,
        artifacts: ResultArtifactStore,
        authorization: QueryAuthorization,
        freshness: FreshnessBarrier,
        freshness_timeout: std::time::Duration,
    ) -> Self {
        Self {
            backend,
            authorization,
            artifacts: Arc::new(artifacts),
            published_results: Arc::new(PublishedArrowResultRegistry::new()),
            handles: Arc::new(Mutex::new(BTreeMap::new())),
            idempotency: Arc::new(Mutex::new(BTreeMap::new())),
            negotiated_sessions: Arc::new(Mutex::new(BTreeMap::new())),
            freshness,
            freshness_timeout,
            query_contract: programmatic_query_contract_identity(),
            tasks: Arc::new(Mutex::new(BTreeMap::new())),
            execution_sequence: AtomicU64::new(0),
        }
    }

    /// Install the daemon-wide Arrow result registry used by the relational publication path.
    ///
    /// The registry is shared with the admitted query runtime, so RPC reads and releases never
    /// reconstruct artifact identity, owner authority, epoch pins, or lease state.
    #[must_use]
    pub fn with_published_results(
        mut self,
        published_results: Arc<PublishedArrowResultRegistry>,
    ) -> Self {
        self.published_results = published_results;
        self
    }

    /// Return the exact registry handle for composition with the admitted relational runtime.
    #[must_use]
    pub fn published_results(&self) -> Arc<PublishedArrowResultRegistry> {
        Arc::clone(&self.published_results)
    }
}

async fn cancel_active_queries(
    handles: Arc<Mutex<BTreeMap<String, Arc<QueryHandle>>>>,
    tasks: Arc<Mutex<BTreeMap<String, tokio::task::JoinHandle<()>>>>,
    artifacts: Arc<ResultArtifactStore>,
) {
    let active = handles.lock().await.values().cloned().collect::<Vec<_>>();
    for handle in active {
        if handle.state.lock().await.terminal_state.is_some() {
            continue;
        }
        handle.cancelled.store(true, Ordering::Release);
        handle.artifacts.set_phase("cancelled_during_shutdown");
        handle.artifacts.set_failure("cancellation");
        let committed = artifacts.persist_query_artifact(&terminal_query_artifact(
            &handle.workspace_id,
            handle.artifacts.snapshot(),
            QueryArtifactPhase::Cancelled,
            None,
            Some("CANCELLED".to_owned()),
        ));
        if committed.is_err() {
            suppress_terminal_claim(&handle).await;
            continue;
        }
        let sequence = next_event_sequence(&handle).await;
        append_terminal(
            &handle,
            QueryEvent {
                event: Some(Event::Terminal(TerminalEvent {
                    header: Some(event_header(&handle.execution.execution_id, sequence, None)),
                    execution_state: QueryExecutionState::Cancelled as i32,
                    availability_state: "UNAVAILABLE".to_owned(),
                    freshness_state: "UNKNOWN".to_owned(),
                    limit_state: "NOT_APPLIED".to_owned(),
                    dependency_state: "NOT_EXECUTED".to_owned(),
                    canonical_response_checksum: None,
                    canonical_error_record_json: None,
                    artifact_id: None,
                    result_row_count: 0,
                    result_byte_count: 0,
                    cleanup_state: "COMPLETE".to_owned(),
                    semantic_execution_state: "CANCELLED".to_owned(),
                    completeness_state: "UNAVAILABLE".to_owned(),
                    truncated: false,
                    query_statuses: Vec::new(),
                    notices: Vec::new(),
                })),
            },
            QueryExecutionState::Cancelled,
        )
        .await;
    }
    let running = std::mem::take(&mut *tasks.lock().await);
    for (_, mut task) in running {
        if tokio::time::timeout(std::time::Duration::from_secs(2), &mut task)
            .await
            .is_err()
        {
            task.abort();
            let _ = task.await;
        }
    }
}

/// Bind and serve the authenticated local gRPC boundary until the supplied shutdown completes.
///
/// # Errors
///
/// Returns socket binding/permission failures or a tonic transport failure.
pub async fn serve_query_uds<B, F>(
    socket: &Path,
    allowed_uid: u32,
    service: ProductionQueryService<B>,
    shutdown: F,
) -> Result<(), QueryTransportError>
where
    B: SemanticQueryBackend,
    F: Future<Output = ()> + Send + 'static,
{
    if socket.exists() {
        return Err(QueryTransportError::SocketExists(socket.to_path_buf()));
    }
    let listener =
        tokio::net::UnixListener::bind(socket).map_err(|source| QueryTransportError::Io {
            path: socket.to_path_buf(),
            source,
        })?;
    fs::set_permissions(socket, fs::Permissions::from_mode(0o600)).map_err(|source| {
        QueryTransportError::Io {
            path: socket.to_path_buf(),
            source,
        }
    })?;
    let incoming = stream::unfold(listener, move |listener| async move {
        let accepted = listener
            .accept()
            .await
            .and_then(|(stream, _)| AuthorizedUnixStream::authenticate(stream, allowed_uid));
        Some((accepted, listener))
    });
    let handles = Arc::clone(&service.handles);
    let tasks = Arc::clone(&service.tasks);
    let artifacts = Arc::clone(&service.artifacts);
    let service = CpgQueryServiceServer::new(service)
        .max_decoding_message_size(MAX_CONTROL_MESSAGE_BYTES)
        .max_encoding_message_size(MAX_CONTROL_MESSAGE_BYTES);
    let shutdown = async move {
        shutdown.await;
        cancel_active_queries(handles, tasks, artifacts).await;
    };
    let result = Server::builder()
        .layer(InterceptorLayer::new(SameUserInterceptor::new(allowed_uid)))
        .add_service(service)
        .serve_with_incoming_shutdown(incoming, shutdown)
        .await;
    let _ = fs::remove_file(socket);
    result.map_err(QueryTransportError::Transport)
}

#[derive(Debug, Error)]
pub enum QueryTransportError {
    #[error("query socket already exists at {0}")]
    SocketExists(PathBuf),
    #[error("query transport I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("query transport failed: {0}")]
    Transport(tonic::transport::Error),
}

fn validate_checksum(expected: &str, bytes: &[u8]) -> Result<(), Status> {
    if expected != framed_digest(bytes) {
        return Err(Status::invalid_argument("request checksum differs"));
    }
    Ok(())
}

struct PublicBoundaryError {
    status: Status,
    canonical_record_json: String,
    code: &'static str,
}

#[derive(serde::Serialize)]
struct PublicErrorRecord<'a> {
    code: u16,
    name: &'a str,
    phase: &'a str,
    severity: &'a str,
    retryability: &'a str,
    grpc_status: &'a str,
    mcp_mapping: &'a str,
    detail: &'a str,
}

fn grpc_code(name: &str) -> tonic::Code {
    match name {
        "ABORTED" => tonic::Code::Aborted,
        "ALREADY_EXISTS" => tonic::Code::AlreadyExists,
        "CANCELLED" => tonic::Code::Cancelled,
        "DATA_LOSS" => tonic::Code::DataLoss,
        "DEADLINE_EXCEEDED" => tonic::Code::DeadlineExceeded,
        "FAILED_PRECONDITION" => tonic::Code::FailedPrecondition,
        "INVALID_ARGUMENT" => tonic::Code::InvalidArgument,
        "NOT_FOUND" => tonic::Code::NotFound,
        "OUT_OF_RANGE" => tonic::Code::OutOfRange,
        "PERMISSION_DENIED" => tonic::Code::PermissionDenied,
        "RESOURCE_EXHAUSTED" => tonic::Code::ResourceExhausted,
        "UNAUTHENTICATED" => tonic::Code::Unauthenticated,
        "UNAVAILABLE" => tonic::Code::Unavailable,
        "UNIMPLEMENTED" => tonic::Code::Unimplemented,
        _ => tonic::Code::Internal,
    }
}

fn public_boundary_error(
    name: &'static str,
    phase: crate::registries::Phase,
    detail: &str,
) -> PublicBoundaryError {
    let entry = crate::registries::public_error(name)
        .expect("RPC boundary error identity must be registered");
    let phase =
        crate::registries::registry_state_name(crate::registries::PHASE_VALUES, phase as u16)
            .expect("registered phase has a stable wire name");
    let canonical_record_json = serde_json::to_string(&PublicErrorRecord {
        code: entry.code,
        name: entry.name,
        phase,
        severity: entry.severity,
        retryability: entry.retryability,
        grpc_status: entry.grpc_status,
        mcp_mapping: entry.mcp_mapping,
        detail,
    })
    .expect("public error record contains only JSON-safe strings");
    PublicBoundaryError {
        status: Status::new(grpc_code(entry.grpc_status), entry.public_message_template),
        canonical_record_json,
        code: entry.name,
    }
}

fn terminal_query_artifact(
    workspace_id: &str,
    evidence: QueryExecutionArtifactEvidence,
    phase: QueryArtifactPhase,
    result_artifact_id: Option<String>,
    public_error_code: Option<String>,
) -> PersistedQueryArtifactBundle {
    let created_at_unix_ms = now_millis();
    PersistedQueryArtifactBundle {
        artifact_schema_version: "codefabric.query-execution-artifact-bundle.v2".to_owned(),
        workspace_id: workspace_id.to_owned(),
        execution: evidence.execution.clone(),
        phase,
        evidence,
        result_artifact_id,
        public_error_code,
        created_at_unix_ms,
        expires_at_unix_ms: created_at_unix_ms
            .saturating_add(RESULT_LEASE_SECONDS.saturating_mul(1_000)),
    }
}

fn event_header(query_id: &str, sequence: u64, snapshot_id: Option<String>) -> QueryEventHeader {
    QueryEventHeader {
        daemon_query_id: query_id.to_owned(),
        sequence,
        snapshot_id,
        event_at_unix_ms: now_millis(),
        event_checksum: framed_digest(format!("{query_id}:{sequence}").as_bytes()),
    }
}

fn event_checksum(event: &Event) -> Option<&str> {
    match event {
        Event::SnapshotPinned(value) => value.header.as_ref(),
        Event::Progress(value) => value.header.as_ref(),
        Event::ResponseChunk(value) => value.header.as_ref(),
        Event::ArtifactReady(value) => value.header.as_ref(),
        Event::Terminal(value) => value.header.as_ref(),
    }
    .map(|header| header.event_checksum.as_str())
}

fn stream_after(handle: Arc<QueryHandle>, after_sequence: u64) -> QueryStream {
    let after_sequence = usize::try_from(after_sequence).unwrap_or(usize::MAX);
    Box::pin(stream::unfold(
        (handle, after_sequence),
        |(handle, index)| async move {
            loop {
                let notified = handle.changed.notified();
                {
                    let state = handle.state.lock().await;
                    if let Some(event) = state.events.get(index).cloned() {
                        drop(notified);
                        return Some((Ok(event), (Arc::clone(&handle), index + 1)));
                    }
                    if state.terminal_state.is_some() {
                        drop(notified);
                        return None;
                    }
                }
                notified.await;
            }
        },
    ))
}

async fn append_terminal(handle: &QueryHandle, event: QueryEvent, state: QueryExecutionState) {
    let mut current = handle.state.lock().await;
    if current.terminal_state.is_none() {
        current.events.push(event);
        current.terminal_state = Some(state);
        drop(current);
        handle.changed.notify_waiters();
    }
}

async fn next_event_sequence(handle: &QueryHandle) -> u64 {
    u64::try_from(handle.state.lock().await.events.len())
        .unwrap_or(u64::MAX)
        .saturating_add(1)
}

async fn suppress_terminal_claim(handle: &QueryHandle) {
    let mut current = handle.state.lock().await;
    if current.terminal_state.is_none() {
        current.terminal_state = Some(QueryExecutionState::Failed);
        drop(current);
        handle.changed.notify_waiters();
    }
}

async fn finalize_unsuccessful_query(
    artifacts: &ResultArtifactStore,
    handle: &QueryHandle,
    query_id: &str,
    mut evidence: QueryExecutionArtifactEvidence,
    requested_phase: QueryArtifactPhase,
    error: PublicBoundaryError,
    freshness: FreshnessState,
) {
    if evidence.failing_stage.is_none() {
        evidence.failing_stage = Some(
            match requested_phase {
                QueryArtifactPhase::Cancelled => "cancellation",
                QueryArtifactPhase::Failed => "execution",
                QueryArtifactPhase::Succeeded => "terminal_persistence",
            }
            .to_owned(),
        );
    }
    let persisted = artifacts.persist_query_artifact(&terminal_query_artifact(
        &handle.workspace_id,
        evidence.clone(),
        requested_phase,
        None,
        Some(error.code.to_owned()),
    ));
    let Ok(persisted_phase) = persisted else {
        suppress_terminal_claim(handle).await;
        return;
    };
    let execution_state = if persisted_phase == QueryArtifactPhase::Cancelled {
        QueryExecutionState::Cancelled
    } else {
        QueryExecutionState::Failed
    };
    append_terminal(
        handle,
        QueryEvent {
            event: Some(Event::Terminal(TerminalEvent {
                header: Some(event_header(query_id, 1, evidence.snapshot_id)),
                execution_state: execution_state as i32,
                availability_state: if error.status.code() == tonic::Code::Unavailable {
                    "UNAVAILABLE"
                } else {
                    "FAILED"
                }
                .to_owned(),
                freshness_state: match freshness {
                    FreshnessState::Current => "CURRENT",
                    FreshnessState::PotentiallyStale => "POTENTIALLY_STALE",
                    FreshnessState::Unavailable => "UNAVAILABLE",
                }
                .to_owned(),
                limit_state: "NOT_APPLIED".to_owned(),
                dependency_state: "NOT_EXECUTED".to_owned(),
                canonical_response_checksum: None,
                canonical_error_record_json: Some(error.canonical_record_json.into_bytes()),
                artifact_id: None,
                result_row_count: evidence
                    .partial_metrics
                    .get("output_rows")
                    .copied()
                    .unwrap_or_default(),
                result_byte_count: 0,
                cleanup_state: "COMPLETE".to_owned(),
                semantic_execution_state: if execution_state == QueryExecutionState::Cancelled {
                    "CANCELLED"
                } else {
                    "FAILED"
                }
                .to_owned(),
                completeness_state: "UNAVAILABLE".to_owned(),
                truncated: false,
                query_statuses: Vec::new(),
                notices: Vec::new(),
            })),
        },
        execution_state,
    )
    .await;
}

fn published_success_access(success: &PublishedArrowSemanticSuccess) -> PublishedResultAccess {
    PublishedResultAccess {
        artifact_id: success.descriptor().artifact_id,
        owner: success.descriptor().owner,
        lease_token: *success.lease_token(),
    }
}

fn release_published_success(
    published_results: &PublishedArrowResultRegistry,
    success: &PublishedArrowSemanticSuccess,
) {
    let _ = published_results.release(published_success_access(success), now_millis());
}

fn validate_published_success(
    published_results: &PublishedArrowResultRegistry,
    handle: &QueryHandle,
    success: &PublishedArrowSemanticSuccess,
) -> Result<(), Status> {
    if success.evidence().execution != handle.execution {
        return Err(Status::failed_precondition(
            "published result execution evidence differs from the accepted query",
        ));
    }
    if success.snapshot().workspace_id != handle.workspace_id {
        return Err(Status::permission_denied(
            "published result snapshot differs from the authorized workspace",
        ));
    }
    let workspace_id = decode_public_id(IdentityDomain::Workspace, None, &handle.workspace_id)
        .map_err(|_| Status::failed_precondition("authorized workspace identity is invalid"))?;
    if success.descriptor().owner.workspace_id().as_bytes() != &workspace_id {
        return Err(Status::permission_denied(
            "published result owner differs from the authorized workspace",
        ));
    }
    published_results
        .read_chunk(PublishedResultReadRequest {
            access: published_success_access(success),
            resource_id: success.descriptor().manifest.authorization_resource_id,
            observed_at_unix_ms: now_millis(),
            offset: 0,
            max_bytes: 1,
        })
        .map(|_| ())
        .map_err(published_result_status)
}

fn published_result_state(
    completion: crate::fabric::arrow_result_resource::ResultCompleteness,
) -> (&'static str, &'static str) {
    match completion {
        crate::fabric::arrow_result_resource::ResultCompleteness::Complete => {
            ("AVAILABLE", "COMPLETE")
        }
        crate::fabric::arrow_result_resource::ResultCompleteness::Partial => ("PARTIAL", "PARTIAL"),
        crate::fabric::arrow_result_resource::ResultCompleteness::Unknown => {
            ("PARTIAL", "INDETERMINATE")
        }
    }
}

async fn finalize_published_query(
    artifacts: &ResultArtifactStore,
    published_results: &PublishedArrowResultRegistry,
    handle: &QueryHandle,
    query_id: &str,
    success: PublishedArrowSemanticSuccess,
) {
    if let Err(error) = validate_published_success(published_results, handle, &success) {
        let freshness = success.snapshot.freshness_state;
        release_published_success(published_results, &success);
        let mut evidence = success.evidence;
        evidence.lifecycle_phase = "published_result_validation".to_owned();
        evidence.failing_stage = Some("published_result_validation".to_owned());
        finalize_unsuccessful_query(
            artifacts,
            handle,
            query_id,
            evidence,
            QueryArtifactPhase::Failed,
            public_boundary_error(
                "INTERNAL",
                crate::registries::Phase::Execution,
                error.message(),
            ),
            freshness,
        )
        .await;
        return;
    }

    let PublishedArrowSemanticSuccess {
        publication,
        lease_token,
        snapshot,
        mut evidence,
    } = success;
    let descriptor = publication.descriptor();
    let snapshot_id = snapshot.snapshot_id.clone();
    let snapshot_bytes = match serde_json::to_value(&snapshot)
        .map_err(|error| error.to_string())
        .and_then(|value| {
            crate::contracts::jcs::canonicalize_value(&value).map_err(|error| error.to_string())
        }) {
        Ok(bytes) => bytes,
        Err(error) => {
            evidence.lifecycle_phase = "terminal_persistence".to_owned();
            evidence.failing_stage = Some("snapshot_metadata_encoding".to_owned());
            let access = PublishedResultAccess {
                artifact_id: descriptor.artifact_id,
                owner: descriptor.owner,
                lease_token,
            };
            let _ = published_results.release(access, now_millis());
            finalize_unsuccessful_query(
                artifacts,
                handle,
                query_id,
                evidence,
                QueryArtifactPhase::Failed,
                public_boundary_error("INTERNAL", crate::registries::Phase::Execution, &error),
                snapshot.freshness_state,
            )
            .await;
            return;
        }
    };
    let artifact_event = match published_arrow_artifact_ready_event(
        event_header(query_id, 2, Some(snapshot_id.clone())),
        descriptor,
        &lease_token,
    ) {
        Ok(event) => event,
        Err(error) => {
            evidence.lifecycle_phase = "terminal_persistence".to_owned();
            evidence.failing_stage = Some("result_descriptor_encoding".to_owned());
            let access = PublishedResultAccess {
                artifact_id: descriptor.artifact_id,
                owner: descriptor.owner,
                lease_token,
            };
            let _ = published_results.release(access, now_millis());
            finalize_unsuccessful_query(
                artifacts,
                handle,
                query_id,
                evidence,
                QueryArtifactPhase::Failed,
                public_boundary_error(
                    "INTERNAL",
                    crate::registries::Phase::Execution,
                    error.message(),
                ),
                snapshot.freshness_state,
            )
            .await;
            return;
        }
    };
    let artifact_id = descriptor.artifact_id.public_id();
    let descriptor_checksum = artifact_event.result_descriptor_checksum.clone();
    let result_rows = descriptor.total_rows;
    // The terminal count describes semantic Arrow result bytes, not the control manifest or
    // schema metadata carried separately in the package descriptor. Keeping the two projections
    // identical lets a presentation adapter detect substitution without reinterpreting either.
    let result_bytes = descriptor.total_ipc_bytes;
    let (availability_state, completeness_state) = published_result_state(descriptor.completion);
    if evidence.snapshot_id.is_none() {
        evidence.snapshot_id = Some(snapshot_id.clone());
    }
    let Ok(persisted_phase) = artifacts.persist_query_artifact(&terminal_query_artifact(
        &handle.workspace_id,
        evidence,
        QueryArtifactPhase::Succeeded,
        Some(artifact_id.clone()),
        None,
    )) else {
        let access = PublishedResultAccess {
            artifact_id: descriptor.artifact_id,
            owner: descriptor.owner,
            lease_token,
        };
        let _ = published_results.release(access, now_millis());
        suppress_terminal_claim(handle).await;
        return;
    };
    if persisted_phase != QueryArtifactPhase::Succeeded {
        let access = PublishedResultAccess {
            artifact_id: descriptor.artifact_id,
            owner: descriptor.owner,
            lease_token,
        };
        let _ = published_results.release(access, now_millis());
        append_terminal(
            handle,
            QueryEvent {
                event: Some(Event::Terminal(TerminalEvent {
                    header: Some(event_header(query_id, 1, Some(snapshot_id))),
                    execution_state: QueryExecutionState::Failed as i32,
                    availability_state: "UNAVAILABLE".to_owned(),
                    freshness_state: "UNKNOWN".to_owned(),
                    limit_state: "NOT_APPLIED".to_owned(),
                    dependency_state: "SATISFIED".to_owned(),
                    canonical_response_checksum: None,
                    canonical_error_record_json: Some(
                        public_boundary_error(
                            "INTERNAL",
                            crate::registries::Phase::Execution,
                            "primary query artifact publication failed; fallback terminal envelope committed",
                        )
                        .canonical_record_json
                        .into_bytes(),
                    ),
                    artifact_id: None,
                    result_row_count: 0,
                    result_byte_count: 0,
                    cleanup_state: "COMPLETE".to_owned(),
                    semantic_execution_state: "FAILED".to_owned(),
                    completeness_state: "UNAVAILABLE".to_owned(),
                    truncated: false,
                    query_statuses: Vec::new(),
                    notices: Vec::new(),
                })),
            },
            QueryExecutionState::Failed,
        )
        .await;
        return;
    }

    let events = vec![
        QueryEvent {
            event: Some(Event::SnapshotPinned(SnapshotPinnedEvent {
                header: Some(event_header(query_id, 1, Some(snapshot_id.clone()))),
                metadata_checksum: framed_digest(&snapshot_bytes),
                canonical_public_snapshot_metadata_json: snapshot_bytes,
            })),
        },
        QueryEvent {
            event: Some(Event::ArtifactReady(artifact_event)),
        },
        QueryEvent {
            event: Some(Event::Terminal(TerminalEvent {
                header: Some(event_header(query_id, 3, Some(snapshot_id))),
                execution_state: QueryExecutionState::Succeeded as i32,
                availability_state: availability_state.to_owned(),
                freshness_state: crate::registries::registry_state_name(
                    crate::registries::FRESHNESS_STATE_VALUES,
                    snapshot.freshness_state as u16,
                )
                .expect("registered freshness state")
                .to_owned(),
                limit_state: "NOT_APPLIED".to_owned(),
                dependency_state: "READY".to_owned(),
                canonical_response_checksum: Some(descriptor_checksum),
                canonical_error_record_json: None,
                artifact_id: Some(artifact_id),
                result_row_count: result_rows,
                result_byte_count: result_bytes,
                cleanup_state: "RETAINED_BY_LEASE".to_owned(),
                semantic_execution_state: "COMPLETE".to_owned(),
                completeness_state: completeness_state.to_owned(),
                truncated: false,
                query_statuses: Vec::new(),
                notices: Vec::new(),
            })),
        },
    ];
    let mut state = handle.state.lock().await;
    if state.terminal_state.is_none() && !handle.cancelled.load(Ordering::Acquire) {
        state.events.extend(events);
        state.terminal_state = Some(QueryExecutionState::Succeeded);
        drop(state);
        handle.changed.notify_waiters();
    } else {
        drop(state);
        let access = PublishedResultAccess {
            artifact_id: descriptor.artifact_id,
            owner: descriptor.owner,
            lease_token,
        };
        let _ = published_results.release(access, now_millis());
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)] // One spawned query owns this complete immutable execution context and terminal sequence.
async fn execute_accepted_query<B: SemanticQueryBackend>(
    backend: Arc<B>,
    artifacts: Arc<ResultArtifactStore>,
    published_results: Arc<PublishedArrowResultRegistry>,
    backend_context: SemanticBackendExecutionContext,
    handle: Arc<QueryHandle>,
    query_id: String,
    validated: ParsedSemanticRequest,
    deadline_unix_ms: i64,
    freshness: FreshnessBarrier,
    freshness_timeout: std::time::Duration,
) {
    let remaining = deadline_unix_ms.saturating_sub(now_millis());
    let executed = if remaining <= 0 {
        Err(public_boundary_error(
            "FRESHNESS_DEADLINE_EXCEEDED",
            crate::registries::Phase::Execution,
            "query deadline elapsed",
        ))
    } else {
        let request_timeout = std::time::Duration::from_millis(remaining.cast_unsigned());
        let admission = match validated.request.freshness_policy {
            FreshnessPolicy::CurrentRequired => FreshnessAdmission::RequireCurrent,
            FreshnessPolicy::WaitForCurrent => FreshnessAdmission::AwaitLatest,
            FreshnessPolicy::BestAvailableSnapshot => FreshnessAdmission::BestAvailable,
            FreshnessPolicy::AwaitLatest => FreshnessAdmission::AwaitLatest,
            FreshnessPolicy::RequireCurrentForTargets
            | FreshnessPolicy::RequireSourceCurrent
            | FreshnessPolicy::RequireSemanticCurrent => FreshnessAdmission::RequireCurrent,
        };
        match freshness
            .admit_query(admission, freshness_timeout.min(request_timeout))
            .await
        {
            Ok(admitted_freshness) => {
                let cancellation = crate::cancellation::Cancellation::from_shared(
                    Arc::clone(&handle.cancelled),
                    64,
                );
                match tokio::time::timeout(
                    request_timeout,
                    backend.execute(
                        validated,
                        admitted_freshness,
                        cancellation,
                        backend_context,
                        handle.artifacts.clone(),
                    ),
                )
                .await
                {
                    Ok(outcome) => Ok(outcome),
                    Err(_) => Err(public_boundary_error(
                        "FRESHNESS_DEADLINE_EXCEEDED",
                        crate::registries::Phase::Execution,
                        "query deadline elapsed",
                    )),
                }
            }
            Err(FreshnessError::Stale) => Err(public_boundary_error(
                "CURRENT_FACTS_UNAVAILABLE",
                crate::registries::Phase::PolicyValidation,
                "current source state is not available",
            )),
            Err(FreshnessError::Unavailable) => Err(public_boundary_error(
                "CURRENT_FACTS_UNAVAILABLE",
                crate::registries::Phase::PolicyValidation,
                "workspace source is unavailable",
            )),
        }
    };
    if handle.cancelled.load(Ordering::Acquire) {
        if let Ok(SemanticBackendOutcome::PublishedArrow(success)) = &executed {
            release_published_success(&published_results, success);
        }
        return;
    }
    match executed {
        Ok(SemanticBackendOutcome::PublishedArrow(success)) => {
            finalize_published_query(&artifacts, &published_results, &handle, &query_id, success)
                .await;
        }
        Ok(SemanticBackendOutcome::Failed { error, evidence }) => {
            let boundary = public_boundary_error(
                "INTERNAL",
                crate::registries::Phase::Execution,
                &error.to_string(),
            );
            finalize_unsuccessful_query(
                &artifacts,
                &handle,
                &query_id,
                evidence,
                QueryArtifactPhase::Failed,
                boundary,
                freshness.state(),
            )
            .await;
        }
        Ok(SemanticBackendOutcome::Cancelled { error, evidence }) => {
            let boundary = public_boundary_error(
                "CANCELLED",
                crate::registries::Phase::Execution,
                &error.to_string(),
            );
            finalize_unsuccessful_query(
                &artifacts,
                &handle,
                &query_id,
                evidence,
                QueryArtifactPhase::Cancelled,
                boundary,
                freshness.state(),
            )
            .await;
        }
        Err(error) => {
            finalize_unsuccessful_query(
                &artifacts,
                &handle,
                &query_id,
                handle.artifacts.snapshot(),
                QueryArtifactPhase::Failed,
                error,
                freshness.state(),
            )
            .await;
        }
    }
}

#[allow(clippy::too_many_lines)] // One service implementation keeps the RPC boundary exhaustive.
#[tonic::async_trait]
impl<B: SemanticQueryBackend> CpgQueryService for ProductionQueryService<B> {
    async fn handshake(
        &self,
        request: Request<HandshakeRequest>,
    ) -> Result<Response<HandshakeResponse>, Status> {
        let request = request.into_inner();
        let claims = self.authorization.authorize_handshake(&request)?;
        let rpc_versions = request
            .rpc_versions
            .as_ref()
            .ok_or_else(|| Status::failed_precondition("RPC version range is missing"))?;
        let semantic_versions = request
            .semantic_query_versions
            .as_ref()
            .ok_or_else(|| Status::failed_precondition("semantic version range is missing"))?;
        if rpc_versions.minimum != "1.0"
            || rpc_versions.maximum != "1.0"
            || semantic_versions.minimum != "1.3"
            || semantic_versions.maximum != "1.3"
        {
            return Err(Status::failed_precondition(
                "no accepted RPC or semantic query version overlap",
            ));
        }
        let host_profile = request
            .host_capabilities
            .as_ref()
            .ok_or_else(|| Status::failed_precondition("host capability profile is missing"))?;
        let derived_host_profile_digest = host_capability_profile_digest(host_profile)?;
        if host_profile.profile_digest != derived_host_profile_digest {
            return Err(Status::failed_precondition(
                "host capability profile digest differs from its typed fields",
            ));
        }
        let negotiated = negotiate_feature_bits(
            CpgdFeatureMask::from_wire(request.required_feature_bits),
            CpgdFeatureMask::from_wire(request.optional_feature_bits),
            CpgdFeatureMask::SUPPORTED,
            CpgdFeatureMask::REQUIRED,
        )?;
        let active_snapshot = self
            .backend
            .public_snapshot(&claims[0].workspace_id)
            .await
            .map_err(|error| Status::failed_precondition(error.to_string()))?;
        self.negotiated_sessions.lock().await.insert(
            request.agent_instance_id.clone(),
            NegotiatedQuerySession::new(derived_host_profile_digest, &claims),
        );
        let capability_codes = ["SOURCE_BYTES", "SOURCE_INVENTORY"]
            .into_iter()
            .filter_map(crate::registries::capability_code)
            .map(u32::from)
            .collect::<Vec<_>>();
        Ok(Response::new(HandshakeResponse {
            daemon_instance_id: "codefabric-local-daemon".to_owned(),
            daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
            rust_build: env!("CARGO_PKG_VERSION").to_owned(),
            negotiated_rpc_version: "1.0".to_owned(),
            negotiated_semantic_query_version: "1.3".to_owned(),
            negotiated_feature_bits: negotiated.bits(),
            negotiated_compression: PayloadCompression::Identity as i32,
            installed_bundles: vec![self.query_contract.clone()],
            active_schema_fingerprints: vec![SchemaFingerprint {
                schema_id: "codefabric.snapshot.base-table-versions".to_owned(),
                version: "1".to_owned(),
                digest: active_snapshot.base_table_version_digest.clone(),
            }],
            effective_limits: Some(effective_limits_profile()),
            authorized_workspaces: claims,
            server_time_unix_ms: now_millis(),
            readiness: Some(ReadinessSummary {
                readiness: WorkspaceReadiness::Ready as i32,
                reason_code: None,
                active_snapshot_id: Some(active_snapshot.snapshot_id),
                supported_language_codes: vec![10, 20],
                supported_query_forms: supported_query_forms(),
                capability_codes,
            }),
        }))
    }

    async fn get_status(
        &self,
        request: Request<StatusRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let request = request.into_inner();
        validate_agent_instance_id(&request.agent_instance_id)?;
        self.authorization
            .authorize_workspace(&request.workspace_id, WorkspacePermission::Query)?;
        self.negotiated_sessions
            .lock()
            .await
            .get(&request.agent_instance_id)
            .ok_or_else(|| Status::failed_precondition("query session handshake is absent"))?
            .authorize(&request.workspace_id, None)?;
        let snapshot = self
            .backend
            .public_snapshot(&request.workspace_id)
            .await
            .map_err(|error| Status::failed_precondition(error.to_string()))?;
        let capability_statuses = snapshot.capability_summaries.clone();
        let status = PublicStatusView {
            ready: true,
            workspace_id: request.workspace_id.clone(),
            agent_instance_id: request.agent_instance_id,
            snapshot,
            versions: BTreeMap::from([
                ("daemon".to_owned(), env!("CARGO_PKG_VERSION").to_owned()),
                ("rpc".to_owned(), "1.0".to_owned()),
                ("semantic_query".to_owned(), "1.3".to_owned()),
            ]),
            supported_languages: vec!["python".to_owned(), "rust".to_owned()],
            supported_request_forms: supported_query_forms(),
            capability_statuses,
            freshness_state: match self.freshness.state() {
                FreshnessState::Current => "CURRENT",
                FreshnessState::PotentiallyStale => "POTENTIALLY_STALE",
                FreshnessState::Unavailable => "UNAVAILABLE",
            },
            service_limits: PublicServiceLimits {
                maximum_control_message_bytes: MAX_CONTROL_MESSAGE_BYTES as u64,
                maximum_payload_chunk_bytes: MAX_PAYLOAD_CHUNK_BYTES as u64,
                maximum_concurrent_queries: 4,
            },
            notices: Vec::new(),
        };
        let canonical = crate::contracts::jcs::canonicalize_value(
            &serde_json::to_value(status)
                .map_err(|_| Status::internal("status serialization failed"))?,
        )
        .map_err(|_| Status::internal("status canonicalization failed"))?;
        Ok(Response::new(StatusResponse {
            workspace_id: request.workspace_id,
            readiness: WorkspaceReadiness::Ready as i32,
            status_checksum: framed_digest(&canonical),
            canonical_public_status_json: canonical,
            observed_at_unix_ms: now_millis(),
        }))
    }

    async fn validate_query(
        &self,
        request: Request<ValidateQueryRequest>,
    ) -> Result<Response<ValidateQueryResponse>, Status> {
        let request = request.into_inner();
        self.authorization
            .authorize_workspace(&request.workspace_id, WorkspacePermission::Query)?;
        self.negotiated_sessions
            .lock()
            .await
            .get(&request.agent_instance_id)
            .ok_or_else(|| Status::failed_precondition("query session handshake is absent"))?
            .authorize(
                &request.workspace_id,
                Some(&request.host_capability_profile_digest),
            )?;
        validate_checksum(&request.request_checksum, &request.canonical_request_json)?;
        let validated = parse_request(&request.canonical_request_json)
            .map_err(|error| Status::invalid_argument(error.to_string()))?;
        self.backend
            .validate_execution_request(&validated)
            .map_err(|error| Status::failed_precondition(error.to_string()))?;
        if validated.request.workspace_id != request.workspace_id {
            return Err(Status::invalid_argument(
                "request workspace differs from RPC workspace",
            ));
        }
        Ok(Response::new(ValidateQueryResponse {
            valid: true,
            canonical_normalized_request_json: validated.canonical_bytes,
            normalized_request_checksum: validated.request_digest,
            effective_semantic_request_id: validated.request.semantic_request_id,
            provisional_snapshot_checks: vec!["workspace-authorized".to_owned()],
            canonical_error_records_json: Vec::new(),
            cost_class: "bounded-wave5".to_owned(),
        }))
    }

    async fn start_query(
        &self,
        request: Request<StartQueryRequest>,
    ) -> Result<Response<StartQueryResponse>, Status> {
        let request = request.into_inner();
        self.authorization
            .authorize_workspace(&request.workspace_id, WorkspacePermission::Query)?;
        self.negotiated_sessions
            .lock()
            .await
            .get(&request.agent_instance_id)
            .ok_or_else(|| Status::failed_precondition("query session handshake is absent"))?
            .authorize(
                &request.workspace_id,
                Some(&request.host_capability_profile_digest),
            )?;
        if request.deadline_unix_ms <= now_millis() {
            return Err(Status::deadline_exceeded("query deadline elapsed"));
        }
        if request.idempotency_key.is_empty() || request.idempotency_key.len() > 256 {
            return Err(Status::invalid_argument("invalid idempotency key"));
        }
        if !matches!(
            DeliveryPreference::try_from(request.delivery_preference),
            Ok(DeliveryPreference::Inline
                | DeliveryPreference::Resource
                | DeliveryPreference::Auto)
        ) || PayloadCompression::try_from(request.payload_compression)
            != Ok(PayloadCompression::Identity)
        {
            return Err(Status::invalid_argument(
                "unsupported delivery or compression",
            ));
        }
        let mut idempotency = self.idempotency.lock().await;
        if let Some(existing) = idempotency.get(&request.idempotency_key) {
            let handle = self
                .handles
                .lock()
                .await
                .get(existing)
                .cloned()
                .ok_or_else(|| Status::internal("idempotency index points to a missing handle"))?;
            let terminal_state = handle
                .state
                .lock()
                .await
                .terminal_state
                .unwrap_or(QueryExecutionState::Accepted);
            return Ok(Response::new(StartQueryResponse {
                daemon_query_id: existing.clone(),
                resume_token: handle.resume_token.clone(),
                accepted_at_unix_ms: now_millis(),
                query_execution_state: terminal_state as i32,
                queue_class: "immediate".to_owned(),
                queue_position: None,
                negotiated_request_version: "1.3".to_owned(),
                negotiated_response_version: "1.3".to_owned(),
                effective_semantic_request_id: request.semantic_request_id.unwrap_or_default(),
                cancel_token: handle.cancel_token.clone(),
            }));
        }
        validate_checksum(&request.request_checksum, &request.canonical_request_json)?;
        let validated = parse_request(&request.canonical_request_json)
            .map_err(|error| Status::invalid_argument(error.to_string()))?;
        self.backend
            .validate_execution_request(&validated)
            .map_err(|error| Status::failed_precondition(error.to_string()))?;
        if validated.request.workspace_id != request.workspace_id {
            return Err(Status::invalid_argument(
                "request workspace differs from RPC workspace",
            ));
        }
        let semantic_request_id = validated.request.semantic_request_id.clone();
        if request
            .semantic_request_id
            .as_ref()
            .is_some_and(|identity| identity != &semantic_request_id)
        {
            return Err(Status::invalid_argument(
                "RPC semantic request identity differs from canonical request",
            ));
        }
        let accepted_at = now_millis();
        let sequence = self.execution_sequence.fetch_add(1, Ordering::Relaxed);
        let query_id = execution_identity(&request, sequence, accepted_at);
        let execution = QueryExecutionContext {
            execution_id: query_id.clone(),
            semantic_request_id: semantic_request_id.clone(),
            mcp_call_id: request.mcp_call_id.clone(),
        };
        let backend_context = self.authorization.backend_execution_context(
            execution.clone(),
            &request.agent_instance_id,
            &request.workspace_id,
            Arc::clone(&self.published_results),
        )?;
        let resume_token = self.artifacts.query_handle_token(
            SecurityMacDomain::QueryResumeToken,
            &query_id,
            &request.agent_instance_id,
            &request.workspace_id,
        );
        let cancel_token = self.artifacts.query_handle_token(
            SecurityMacDomain::QueryCancelToken,
            &query_id,
            &request.agent_instance_id,
            &request.workspace_id,
        );
        let response_cancel_token = cancel_token.clone();
        let handle = Arc::new(QueryHandle {
            execution: execution.clone(),
            artifacts: QueryExecutionArtifactAccumulator::new(execution.clone()),
            resume_token: resume_token.clone(),
            cancel_token,
            agent_instance_id: request.agent_instance_id,
            workspace_id: request.workspace_id,
            cancelled: Arc::new(AtomicBool::new(false)),
            state: Mutex::new(QueryHandleState::default()),
            changed: Notify::new(),
        });
        self.handles
            .lock()
            .await
            .insert(query_id.clone(), Arc::clone(&handle));
        idempotency.insert(request.idempotency_key, query_id.clone());
        drop(idempotency);
        let span = tracing::info_span!(
            "query_execution",
            execution_id = %execution.execution_id,
            semantic_request_id = %execution.semantic_request_id,
            mcp_call_id = %execution.mcp_call_id,
        );
        let task = tokio::spawn(
            execute_accepted_query(
                Arc::clone(&self.backend),
                Arc::clone(&self.artifacts),
                Arc::clone(&self.published_results),
                backend_context,
                Arc::clone(&handle),
                query_id.clone(),
                validated,
                request.deadline_unix_ms,
                self.freshness.clone(),
                self.freshness_timeout,
            )
            .instrument(span),
        );
        self.tasks.lock().await.insert(query_id.clone(), task);
        Ok(Response::new(StartQueryResponse {
            daemon_query_id: query_id,
            resume_token,
            accepted_at_unix_ms: accepted_at,
            query_execution_state: QueryExecutionState::Accepted as i32,
            queue_class: "accepted".to_owned(),
            queue_position: None,
            negotiated_request_version: "1.3".to_owned(),
            negotiated_response_version: "1.3".to_owned(),
            effective_semantic_request_id: semantic_request_id,
            cancel_token: response_cancel_token,
        }))
    }

    type StreamQueryStream = QueryStream;

    async fn stream_query(
        &self,
        request: Request<StreamQueryRequest>,
    ) -> Result<Response<Self::StreamQueryStream>, Status> {
        let request = request.into_inner();
        let handle = self
            .handles
            .lock()
            .await
            .get(&request.daemon_query_id)
            .cloned()
            .ok_or_else(|| Status::not_found("query handle not found"))?;
        self.authorization
            .authorize_workspace(&handle.workspace_id, WorkspacePermission::Query)?;
        if handle.resume_token != request.resume_token {
            return Err(Status::permission_denied("resume token differs"));
        }
        Ok(Response::new(stream_after(handle, request.after_sequence)))
    }

    type AttachQueryStream = QueryStream;

    async fn attach_query(
        &self,
        request: Request<AttachQueryRequest>,
    ) -> Result<Response<Self::AttachQueryStream>, Status> {
        let request = request.into_inner();
        self.authorization
            .authorize_workspace(&request.workspace_id, WorkspacePermission::Query)?;
        self.negotiated_sessions
            .lock()
            .await
            .get(&request.agent_instance_id)
            .ok_or_else(|| Status::failed_precondition("query session handshake is absent"))?
            .authorize(&request.workspace_id, None)?;
        let handle = self
            .handles
            .lock()
            .await
            .get(&request.daemon_query_id)
            .cloned()
            .ok_or_else(|| Status::not_found("query handle not found"))?;
        if handle.workspace_id != request.workspace_id
            || handle.agent_instance_id != request.agent_instance_id
            || handle.resume_token != request.resume_token
        {
            return Err(Status::permission_denied(
                "query attachment identity differs",
            ));
        }
        if let Some(expected) = request.after_event_checksum.as_deref()
            && request.after_sequence != 0
        {
            let state = handle.state.lock().await;
            let index = usize::try_from(request.after_sequence - 1)
                .map_err(|_| Status::out_of_range("attachment sequence is invalid"))?;
            let actual = state
                .events
                .get(index)
                .and_then(|event| event.event.as_ref())
                .and_then(event_checksum)
                .ok_or_else(|| Status::out_of_range("attachment sequence is unavailable"))?;
            if actual != expected {
                return Err(Status::failed_precondition("attachment checksum differs"));
            }
        }
        Ok(Response::new(stream_after(handle, request.after_sequence)))
    }

    async fn cancel_query(
        &self,
        request: Request<CancelQueryRequest>,
    ) -> Result<Response<CancelQueryResponse>, Status> {
        let request = request.into_inner();
        self.authorization
            .authorize_workspace(&request.workspace_id, WorkspacePermission::Query)?;
        self.negotiated_sessions
            .lock()
            .await
            .get(&request.agent_instance_id)
            .ok_or_else(|| Status::failed_precondition("query session handshake is absent"))?
            .authorize(&request.workspace_id, None)?;
        let Some(handle) = self
            .handles
            .lock()
            .await
            .get(&request.daemon_query_id)
            .cloned()
        else {
            return Ok(Response::new(CancelQueryResponse {
                daemon_query_id: request.daemon_query_id,
                state: CancellationState::NotFound as i32,
                acknowledged_at_unix_ms: now_millis(),
                terminal_state: None,
                cleaning_up_components: Vec::new(),
                forced_termination: false,
            }));
        };
        if handle.workspace_id != request.workspace_id
            || handle.agent_instance_id != request.agent_instance_id
            || handle.cancel_token != request.cancel_token
        {
            return Err(Status::permission_denied(
                "query cancellation identity differs",
            ));
        }
        let terminal = handle.state.lock().await.terminal_state;
        if let Some(terminal) = terminal {
            return Ok(Response::new(CancelQueryResponse {
                daemon_query_id: request.daemon_query_id,
                state: CancellationState::AlreadyTerminal as i32,
                acknowledged_at_unix_ms: now_millis(),
                terminal_state: Some(terminal as i32),
                cleaning_up_components: Vec::new(),
                forced_termination: false,
            }));
        }
        handle.cancelled.store(true, Ordering::Release);
        handle.artifacts.set_phase("cancelled_by_client");
        handle.artifacts.set_failure("cancellation");
        self.artifacts
            .persist_query_artifact(&terminal_query_artifact(
                &handle.workspace_id,
                handle.artifacts.snapshot(),
                QueryArtifactPhase::Cancelled,
                None,
                Some("CANCELLED".to_owned()),
            ))?;
        let sequence = next_event_sequence(&handle).await;
        append_terminal(
            &handle,
            QueryEvent {
                event: Some(Event::Terminal(TerminalEvent {
                    header: Some(event_header(&request.daemon_query_id, sequence, None)),
                    execution_state: QueryExecutionState::Cancelled as i32,
                    availability_state: "UNAVAILABLE".to_owned(),
                    freshness_state: "UNKNOWN".to_owned(),
                    limit_state: "NOT_APPLIED".to_owned(),
                    dependency_state: "NOT_EXECUTED".to_owned(),
                    canonical_response_checksum: None,
                    canonical_error_record_json: None,
                    artifact_id: None,
                    result_row_count: 0,
                    result_byte_count: 0,
                    cleanup_state: "COMPLETE".to_owned(),
                    semantic_execution_state: "CANCELLED".to_owned(),
                    completeness_state: "UNAVAILABLE".to_owned(),
                    truncated: false,
                    query_statuses: Vec::new(),
                    notices: Vec::new(),
                })),
            },
            QueryExecutionState::Cancelled,
        )
        .await;
        Ok(Response::new(CancelQueryResponse {
            daemon_query_id: request.daemon_query_id,
            state: CancellationState::Cancelled as i32,
            acknowledged_at_unix_ms: now_millis(),
            terminal_state: Some(QueryExecutionState::Cancelled as i32),
            cleaning_up_components: Vec::new(),
            forced_termination: false,
        }))
    }

    type ReadResultStream = ArtifactStream;

    async fn read_result(
        &self,
        request: Request<ReadResultRequest>,
    ) -> Result<Response<Self::ReadResultStream>, Status> {
        let request = request.into_inner();
        if PayloadCompression::try_from(request.accepted_compression)
            != Ok(PayloadCompression::Identity)
        {
            return Err(Status::invalid_argument(
                "only identity encoding is supported",
            ));
        }
        let maximum = usize::try_from(
            request
                .maximum_bytes
                .unwrap_or(MAX_PAYLOAD_CHUNK_BYTES as u64)
                .min(MAX_PAYLOAD_CHUNK_BYTES as u64),
        )
        .map_err(|_| Status::out_of_range("maximum result bytes is invalid"))?;
        if maximum == 0 {
            return Err(Status::invalid_argument(
                "maximum result bytes must be positive",
            ));
        }
        if request.authorization_resource_id.is_empty() || request.owner.is_none() {
            return Err(Status::invalid_argument(
                "Arrow result reads require an owner and authorization resource",
            ));
        }
        let access = published_access(
            &request.artifact_id,
            &request.lease_token,
            request.owner.as_ref(),
        )?;
        let resource_id =
            PublishedResultResourceId::try_from_public_id(&request.authorization_resource_id)
                .map_err(published_result_status)?;
        let chunk = self
            .published_results
            .read_chunk(PublishedResultReadRequest {
                access,
                resource_id,
                observed_at_unix_ms: now_millis(),
                offset: request.offset,
                max_bytes: maximum,
            })
            .map_err(published_result_status)?;
        let payload = chunk.bytes.to_vec();
        let event = ResultChunk {
            artifact_id: request.artifact_id,
            offset: chunk.offset,
            uncompressed_length: u64::try_from(payload.len()).unwrap_or(u64::MAX),
            payload_checksum: framed_digest(&payload),
            payload,
            artifact_checksum: String::new(),
            content_type: chunk.media_type.to_owned(),
            encoding: PayloadCompression::Identity as i32,
            final_chunk: chunk.complete,
            lease_expires_at_unix_ms: chunk.lease_expires_at_unix_ms,
            authorization_resource_id: chunk.resource_id.public_id(),
            next_offset: chunk.next_offset,
            total_length: chunk.total_length,
            content_checksum: frame_digest(chunk.content_checksum),
        };
        Ok(Response::new(Box::pin(stream::once(
            async move { Ok(event) },
        ))))
    }

    async fn release_result(
        &self,
        request: Request<ReleaseResultRequest>,
    ) -> Result<Response<ReleaseResultResponse>, Status> {
        let request = request.into_inner();
        let access = published_access(
            &request.artifact_id,
            &request.lease_token,
            request.owner.as_ref(),
        )?;
        let state = self
            .published_results
            .release(access, now_millis())
            .map_err(published_result_status)?;
        let release_state = match state {
            PublishedReleaseOutcome::Released => "released",
            PublishedReleaseOutcome::AlreadyReleased => "already_released",
        };
        Ok(Response::new(ReleaseResultResponse {
            artifact_id: request.artifact_id,
            released: true,
            remaining_lease_expires_at_unix_ms: None,
            release_state: release_state.to_owned(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::generated::codefabric::cpgd::v1::CredentialProof;

    #[test]
    fn programmatic_query_contract_identity_is_typed_ordered_and_causal() {
        let identity = programmatic_query_contract_identity();
        let released_forms = QueryForm::ALL
            .into_iter()
            .map(QueryForm::slug)
            .collect::<Vec<_>>();

        assert_eq!(identity.bundle_id, PROGRAMMATIC_QUERY_CONTRACT_ID);
        assert_eq!(identity.bundle_version, PROGRAMMATIC_QUERY_CONTRACT_VERSION);
        assert_eq!(
            identity.bundle_digest,
            query_contract_digest(released_forms.iter().copied())
        );
        assert_ne!(
            identity.bundle_digest,
            query_contract_digest(released_forms.iter().rev().copied()),
            "presentation order is part of the released contract"
        );

        let mut changed_forms = released_forms;
        changed_forms[0] = "changed query form";
        assert_ne!(
            identity.bundle_digest,
            query_contract_digest(changed_forms),
            "changing a typed query form must change the advertised contract identity"
        );
    }

    fn query_claim(workspace_id: &str) -> WorkspaceClaim {
        WorkspaceClaim {
            workspace_id: workspace_id.to_owned(),
            repository_id: None,
            worktree_id: None,
            workspace_kind: "programmatic".to_owned(),
            readiness: WorkspaceReadiness::Ready as i32,
            permission_claims: vec!["query".to_owned()],
        }
    }

    fn handshake_request(
        agent_instance_id: &str,
        desired_workspace_ids: Vec<String>,
    ) -> HandshakeRequest {
        HandshakeRequest {
            adapter_instance_id: agent_instance_id.to_owned(),
            desired_workspace_ids,
            credential_proof: Some(CredentialProof {
                credential_id: agent_instance_id.to_owned(),
                capability_token: b"wp37-session-capability".to_vec(),
            }),
            agent_instance_id: agent_instance_id.to_owned(),
            ..HandshakeRequest::default()
        }
    }

    #[test]
    fn public_lifecycle_identity_contract_rejects_substitution_and_duplicate_workspaces() {
        let authorization = QueryAuthorization::new(
            b"wp37-session-capability",
            vec![query_claim("workspace-a"), query_claim("workspace-b")],
        )
        .expect("closed authorization");

        let accepted = authorization
            .authorize_handshake(&handshake_request(
                "agent-a",
                vec!["workspace-a".to_owned()],
            ))
            .expect("bound handshake");
        assert_eq!(accepted, [query_claim("workspace-a")]);

        let mut wrong_adapter = handshake_request("agent-a", vec!["workspace-a".to_owned()]);
        wrong_adapter.adapter_instance_id = "agent-b".to_owned();
        assert_eq!(
            authorization
                .authorize_handshake(&wrong_adapter)
                .expect_err("adapter substitution")
                .code(),
            tonic::Code::Unauthenticated
        );

        let mut wrong_credential = handshake_request("agent-a", vec!["workspace-a".to_owned()]);
        wrong_credential
            .credential_proof
            .as_mut()
            .expect("credential")
            .credential_id = "agent-b".to_owned();
        assert_eq!(
            authorization
                .authorize_handshake(&wrong_credential)
                .expect_err("credential substitution")
                .code(),
            tonic::Code::Unauthenticated
        );

        let duplicated = handshake_request(
            "agent-a",
            vec!["workspace-a".to_owned(), "workspace-a".to_owned()],
        );
        assert_eq!(
            authorization
                .authorize_handshake(&duplicated)
                .expect_err("duplicate workspace")
                .code(),
            tonic::Code::InvalidArgument
        );
    }

    #[test]
    fn negotiated_query_session_binds_workspace_and_host_profile() {
        let session = NegotiatedQuerySession::new(
            "b3:host-profile".to_owned(),
            &[query_claim("workspace-a")],
        );
        session
            .authorize("workspace-a", Some("b3:host-profile"))
            .expect("exact session identity");
        assert_eq!(
            session
                .authorize("workspace-b", Some("b3:host-profile"))
                .expect_err("workspace substitution")
                .code(),
            tonic::Code::PermissionDenied
        );
        assert_eq!(
            session
                .authorize("workspace-a", Some("b3:other-profile"))
                .expect_err("profile substitution")
                .code(),
            tonic::Code::FailedPrecondition
        );
    }
}
