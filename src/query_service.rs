//! Accepted gRPC query handles and immutable canonical result artifacts.

use std::collections::BTreeMap;
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
use tokio::sync::{Mutex, Notify, RwLock};
use tonic::service::InterceptorLayer;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tracing::Instrument as _;

use crate::fabric::{FabricTable, QueryExecutionContext, QueryPlanArtifact, ServingQuerySession};
use crate::golden_corpus::CoreSourceCoverage;
use crate::identity::{SemanticFingerprintDomain, semantic_fingerprint};
use crate::integrity::{frame_digest, framed_digest};
use crate::lifecycle::{FreshnessAdmission, FreshnessBarrier, FreshnessState};
use crate::registries::CpgdFeatureMask;
use crate::registries::QUERY_FORM_VALUES;
use crate::rpc::generated::codefabric::cpgd::v1::cpg_query_service_server::CpgQueryService;
use crate::rpc::generated::codefabric::cpgd::v1::cpg_query_service_server::CpgQueryServiceServer;
use crate::rpc::generated::codefabric::cpgd::v1::query_event::Event;
use crate::rpc::generated::codefabric::cpgd::v1::{
    ArtifactReadyEvent, AttachQueryRequest, BundleIdentity, CancelQueryRequest,
    CancelQueryResponse, CancellationState, DeliveryPreference, EffectiveLimitsProfile,
    HandshakeRequest, HandshakeResponse, HostCapabilityProfile, PayloadCompression, QueryEvent,
    QueryEventHeader, QueryExecutionState, QueryStatusSummary, ReadResultRequest, ReadinessSummary,
    ReleaseResultRequest, ReleaseResultResponse, ResultChunk, SchemaFingerprint,
    SnapshotPinnedEvent, StartQueryRequest, StartQueryResponse, StatusRequest, StatusResponse,
    StreamQueryRequest, TerminalEvent, ValidateQueryRequest, ValidateQueryResponse, WorkspaceClaim,
    WorkspaceReadiness,
};
use crate::rpc::{
    AuthorizedUnixStream, MAX_CONTROL_MESSAGE_BYTES, MAX_PAYLOAD_CHUNK_BYTES, SameUserInterceptor,
    negotiate_feature_bits,
};
use crate::security::{
    KeyedAuthenticator, SecurityMacDomain, authenticator_hex, local_token_digest,
};
use crate::semantic_query::QueryForm;
use crate::semantic_query::{
    ExecutedSemanticResponse, FreshnessPolicy, SemanticQueryError, SemanticSnapshotResponse,
    ValidatedSemanticRequest, execute_request_in_context, snapshot_response, validate_request,
};

const RESULT_LEASE_SECONDS: i64 = 1_800;

type QueryStream = Pin<Box<dyn Stream<Item = Result<QueryEvent, Status>> + Send>>;
type ArtifactStream = Pin<Box<dyn Stream<Item = Result<ResultChunk, Status>> + Send>>;

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
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

#[derive(Clone, Debug)]
struct ResultArtifact {
    id: String,
    checksum: String,
    lease_token: String,
    lease_expires_at_unix_ms: i64,
    bytes: Arc<[u8]>,
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
    pub execution: QueryExecutionContext,
    pub phase: QueryArtifactPhase,
    pub plan_artifacts: Vec<QueryPlanArtifact>,
    pub result_artifact_id: Option<String>,
    pub public_error_code: Option<String>,
    pub created_at_unix_ms: i64,
    pub expires_at_unix_ms: i64,
}

/// Bounded operator explanation joining one exact Delta commit to the executions that read it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VersionExplanation {
    pub table_code: i16,
    pub table_name: String,
    pub delta_version: u64,
    pub delta_commit_info: serde_json::Value,
    pub executions: Vec<PersistedQueryArtifactBundle>,
    pub scanned_artifact_count: usize,
}

#[derive(Clone, Debug)]
pub struct ResultArtifactStore {
    root: PathBuf,
    lease_secret: [u8; 32],
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
        let mut lease_secret = [0_u8; 32];
        fs::File::open("/dev/urandom")?.read_exact(&mut lease_secret)?;
        Ok(Self { root, lease_secret })
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

    fn query_artifact_path(&self, execution_id: &str) -> PathBuf {
        let digest = framed_digest(execution_id.as_bytes());
        self.root
            .join("query-plan-artifacts")
            .join(format!("{}.json", &digest[3..]))
    }

    pub(crate) fn persist_query_artifact(
        &self,
        artifact: &PersistedQueryArtifactBundle,
    ) -> Result<(), Status> {
        let value = serde_json::to_value(artifact)
            .map_err(|_| Status::internal("query artifact serialization failed"))?;
        let bytes = crate::contracts::jcs::canonicalize_value(&value).map_err(|error| {
            Status::internal(format!("query artifact canonicalization failed: {error}"))
        })?;
        let final_path = self.query_artifact_path(&artifact.execution.execution_id);
        if final_path.exists() {
            let existing = fs::read(&final_path)
                .map_err(|_| Status::internal("query artifact read failed"))?;
            if existing == bytes {
                return Ok(());
            }
            return Err(Status::already_exists(
                "query execution already has a different terminal artifact",
            ));
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
        let bytes =
            fs::read(self.query_artifact_path(execution_id)).map_err(|error| {
                match error.kind() {
                    std::io::ErrorKind::NotFound => Status::not_found("query artifact not found"),
                    _ => Status::internal("query artifact read failed"),
                }
            })?;
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
            if !entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "json")
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

    /// Explain one committed table version from Delta history and retained query artifacts.
    ///
    /// The scan is bounded so the administrative/status path cannot become an unbounded query.
    ///
    /// # Errors
    ///
    /// Returns a gRPC status when the table/version is absent, history is unreadable, or the
    /// retained artifact census exceeds the operator bound.
    pub async fn explain_version(
        &self,
        table: &FabricTable,
        delta_version: u64,
    ) -> Result<VersionExplanation, Status> {
        const MAX_EXPLAIN_ARTIFACTS: usize = 4_096;
        let spec = crate::schema_registry::table_spec(table.table_code)
            .ok_or_else(|| Status::not_found("table code is not registered"))?;
        let mut historical = table.delta.clone();
        historical
            .load_version(delta_version)
            .await
            .map_err(|_| Status::not_found("Delta version is not available"))?;
        let commit = historical
            .history(Some(1))
            .await
            .map_err(|_| Status::internal("Delta history lookup failed"))?
            .next()
            .ok_or_else(|| Status::data_loss("Delta version has no commit-info action"))?;
        let delta_commit_info = serde_json::to_value(commit)
            .map_err(|_| Status::internal("Delta commit-info serialization failed"))?;

        let mut paths = fs::read_dir(self.root.join("query-plan-artifacts"))
            .map_err(|_| Status::internal("query artifact directory read failed"))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .collect::<Vec<_>>();
        paths.sort();
        if paths.len() > MAX_EXPLAIN_ARTIFACTS {
            return Err(Status::resource_exhausted(
                "query artifact explanation scan exceeds the bounded census",
            ));
        }
        let scanned_artifact_count = paths.len();
        let mut executions = Vec::new();
        for path in paths {
            let bytes = fs::read(path)
                .map_err(|_| Status::internal("query artifact explanation read failed"))?;
            let artifact: PersistedQueryArtifactBundle = serde_json::from_slice(&bytes)
                .map_err(|_| Status::data_loss("query artifact explanation schema is invalid"))?;
            if artifact.plan_artifacts.iter().any(|plan| {
                plan.source_table_versions
                    .get(&u16::try_from(table.table_code).unwrap_or(u16::MAX))
                    .copied()
                    == Some(delta_version)
            }) {
                executions.push(artifact);
            }
        }
        executions.sort_by(|left, right| {
            left.execution
                .execution_id
                .cmp(&right.execution.execution_id)
        });
        Ok(VersionExplanation {
            table_code: table.table_code,
            table_name: spec.name.to_owned(),
            delta_version,
            delta_commit_info,
            executions,
            scanned_artifact_count,
        })
    }

    fn insert(
        &self,
        bytes: Vec<u8>,
        agent_id: &str,
        workspace_id: &str,
        snapshot_id: &str,
    ) -> Result<ResultArtifact, Status> {
        let checksum = framed_digest(&bytes);
        let mut identity = semantic_fingerprint(SemanticFingerprintDomain::ResultArtifact);
        for field in [workspace_id, agent_id, snapshot_id, checksum.as_str()] {
            identity.update(&(field.len() as u64).to_be_bytes());
            identity.update(field.as_bytes());
        }
        let identity = identity.finalize();
        let id = format!("result:{}", &frame_digest(identity)[3..35]);
        let final_path = self.root.join(format!("{}.json", &checksum[3..]));
        if final_path.exists() {
            let existing = fs::read(&final_path)
                .map_err(|_| Status::internal("result artifact read failed"))?;
            if existing != bytes {
                return Err(Status::data_loss("result artifact identity collision"));
            }
        } else {
            let temporary =
                self.root
                    .join(format!(".{}.{}.tmp", &checksum[3..], std::process::id()));
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)
                .map_err(|_| Status::internal("result staging failed"))?;
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|_| Status::internal("result staging write failed"))?;
            fs::rename(&temporary, &final_path)
                .map_err(|_| Status::internal("result artifact publication failed"))?;
        }
        let mut lease = KeyedAuthenticator::new(&self.lease_secret, SecurityMacDomain::ResultLease);
        lease.update(id.as_bytes());
        lease.update(agent_id.as_bytes());
        lease.update(workspace_id.as_bytes());
        let lease_token = format!("lease:{}", authenticator_hex(lease.finalize()));
        Ok(ResultArtifact {
            id,
            checksum,
            lease_token,
            lease_expires_at_unix_ms: now_millis().saturating_add(RESULT_LEASE_SECONDS * 1_000),
            bytes: bytes.into(),
        })
    }
}

#[async_trait]
pub trait SemanticQueryBackend: Send + Sync + 'static {
    async fn execute(
        &self,
        request: ValidatedSemanticRequest,
        freshness: FreshnessState,
        cancellation: crate::cancellation::Cancellation,
        execution: QueryExecutionContext,
    ) -> Result<ExecutedSemanticResponse, SemanticQueryError>;

    async fn public_snapshot(
        &self,
        workspace_id: &str,
    ) -> Result<SemanticSnapshotResponse, SemanticQueryError>;
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
        let proof = request
            .credential_proof
            .as_ref()
            .ok_or_else(|| Status::unauthenticated("capability proof is missing"))?;
        if proof.credential_id.is_empty()
            || request.agent_instance_id.is_empty()
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

#[async_trait]
impl SemanticQueryBackend for ServingQuerySession {
    async fn execute(
        &self,
        request: ValidatedSemanticRequest,
        freshness: FreshnessState,
        cancellation: crate::cancellation::Cancellation,
        execution: QueryExecutionContext,
    ) -> Result<ExecutedSemanticResponse, SemanticQueryError> {
        execute_request_in_context(self, request, freshness, cancellation, execution).await
    }

    async fn public_snapshot(
        &self,
        workspace_id: &str,
    ) -> Result<SemanticSnapshotResponse, SemanticQueryError> {
        let manifest = self.snapshot_manifest();
        if manifest.body.workspace_id != workspace_id {
            return Err(SemanticQueryError::Invalid(
                "status workspace differs from the pinned snapshot".to_owned(),
            ));
        }
        Ok(snapshot_response(&manifest, FreshnessState::Current))
    }
}

/// Production workspace router over genuine snapshot-leased serving sessions.
#[derive(Debug, Default)]
pub struct WorkspaceQueryBackend {
    sessions: RwLock<BTreeMap<String, Arc<ServingQuerySession>>>,
}

impl WorkspaceQueryBackend {
    /// Install or atomically replace the exact leased session for one workspace.
    pub async fn install(
        &self,
        session: Arc<ServingQuerySession>,
    ) -> Result<(), SemanticQueryError> {
        let workspace_id = session.snapshot_manifest().body.workspace_id;
        self.sessions.write().await.insert(workspace_id, session);
        Ok(())
    }

    #[must_use]
    pub async fn active_workspace_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    async fn session(
        &self,
        workspace_id: &str,
    ) -> Result<Arc<ServingQuerySession>, SemanticQueryError> {
        self.sessions
            .read()
            .await
            .get(workspace_id)
            .cloned()
            .ok_or_else(|| {
                SemanticQueryError::Invalid(
                    "workspace has no active snapshot-leased query session".to_owned(),
                )
            })
    }
}

#[async_trait]
impl SemanticQueryBackend for WorkspaceQueryBackend {
    async fn execute(
        &self,
        request: ValidatedSemanticRequest,
        freshness: FreshnessState,
        cancellation: crate::cancellation::Cancellation,
        execution: QueryExecutionContext,
    ) -> Result<ExecutedSemanticResponse, SemanticQueryError> {
        let session = self.session(&request.request.workspace_id).await?;
        execute_request_in_context(
            session.as_ref(),
            request,
            freshness,
            cancellation,
            execution,
        )
        .await
    }

    async fn public_snapshot(
        &self,
        workspace_id: &str,
    ) -> Result<SemanticSnapshotResponse, SemanticQueryError> {
        let session = self.session(workspace_id).await?;
        SemanticQueryBackend::public_snapshot(session.as_ref(), workspace_id).await
    }
}

#[derive(Clone, Debug, Default)]
struct QueryHandleState {
    events: Vec<QueryEvent>,
    terminal_state: Option<QueryExecutionState>,
}

#[derive(Debug)]
struct QueryHandle {
    execution: QueryExecutionContext,
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
    artifact_records: Arc<Mutex<BTreeMap<String, ResultArtifact>>>,
    handles: Arc<Mutex<BTreeMap<String, Arc<QueryHandle>>>>,
    idempotency: Arc<Mutex<BTreeMap<String, String>>>,
    host_profile_digests: Arc<Mutex<BTreeMap<String, String>>>,
    freshness: FreshnessBarrier,
    freshness_timeout: std::time::Duration,
    query_bundle: BundleIdentity,
    core_source_coverage: Option<CoreSourceCoverage>,
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

fn query_bundle_identity() -> BundleIdentity {
    let authority: serde_json::Value = serde_json::from_str(include_str!(
        "../contracts/bundles/query-language-bundle.json"
    ))
    .expect("generated query-language bundle must be valid JSON");
    let string = |field: &str| {
        authority[field]
            .as_str()
            .unwrap_or_else(|| panic!("query-language bundle is missing {field}"))
            .to_owned()
    };
    let bundle = BundleIdentity {
        bundle_id: string("artifact_id"),
        bundle_version: string("bundle_version"),
        bundle_digest: string("bundle_digest"),
    };
    assert_eq!(
        bundle.bundle_id, "codefabric.bundles.query-language-bundle",
        "query-language bundle identity is unexpected"
    );
    assert!(valid_bundle_digest(&bundle.bundle_digest));
    bundle
}

fn supported_query_forms() -> Vec<String> {
    QUERY_FORM_VALUES
        .iter()
        .filter_map(|entry| QueryForm::try_from(entry.code).ok())
        .filter(|form| form.currently_supported())
        .map(|form| form.registry_slug().to_owned())
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
            artifact_records: Arc::new(Mutex::new(BTreeMap::new())),
            handles: Arc::new(Mutex::new(BTreeMap::new())),
            idempotency: Arc::new(Mutex::new(BTreeMap::new())),
            host_profile_digests: Arc::new(Mutex::new(BTreeMap::new())),
            freshness,
            freshness_timeout,
            query_bundle: query_bundle_identity(),
            core_source_coverage: None,
            tasks: Arc::new(Mutex::new(BTreeMap::new())),
            execution_sequence: AtomicU64::new(0),
        }
    }

    #[must_use]
    pub fn with_core_source_coverage(mut self, coverage: CoreSourceCoverage) -> Self {
        self.core_source_coverage = Some(coverage);
        self
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
        let _ = artifacts.persist_query_artifact(&terminal_query_artifact(
            handle.execution.clone(),
            QueryArtifactPhase::Cancelled,
            Vec::new(),
            None,
            Some("CANCELLED".to_owned()),
        ));
        append_terminal(
            &handle,
            QueryEvent {
                event: Some(Event::Terminal(TerminalEvent {
                    header: None,
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
        "INTERNAL" => tonic::Code::Internal,
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
        .expect("RPC boundary error identity must be registry generated");
    let phase =
        crate::registries::registry_state_name(crate::registries::PHASE_VALUES, phase as u16)
            .expect("generated phase has a registry name");
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
    execution: QueryExecutionContext,
    phase: QueryArtifactPhase,
    plan_artifacts: Vec<QueryPlanArtifact>,
    result_artifact_id: Option<String>,
    public_error_code: Option<String>,
) -> PersistedQueryArtifactBundle {
    let created_at_unix_ms = now_millis();
    PersistedQueryArtifactBundle {
        artifact_schema_version: "codefabric.query-execution-artifact-bundle.v1".to_owned(),
        execution,
        phase,
        plan_artifacts,
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

#[allow(clippy::too_many_arguments, clippy::too_many_lines)] // One spawned query owns this complete immutable execution context and terminal sequence.
async fn execute_accepted_query<B: SemanticQueryBackend>(
    backend: Arc<B>,
    artifacts: Arc<ResultArtifactStore>,
    artifact_records: Arc<Mutex<BTreeMap<String, ResultArtifact>>>,
    handle: Arc<QueryHandle>,
    query_id: String,
    validated: ValidatedSemanticRequest,
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
                        handle.execution.clone(),
                    ),
                )
                .await
                {
                    Ok(Ok(executed)) => Ok(executed),
                    Ok(Err(error)) => Err(public_boundary_error(
                        "INTERNAL",
                        crate::registries::Phase::Execution,
                        &error.to_string(),
                    )),
                    Err(_) => Err(public_boundary_error(
                        "FRESHNESS_DEADLINE_EXCEEDED",
                        crate::registries::Phase::Execution,
                        "query deadline elapsed",
                    )),
                }
            }
            Err(crate::lifecycle::LifecycleError::Stale) => Err(public_boundary_error(
                "CURRENT_FACTS_UNAVAILABLE",
                crate::registries::Phase::PolicyValidation,
                "current source state is not available",
            )),
            Err(crate::lifecycle::LifecycleError::Unavailable) => Err(public_boundary_error(
                "CURRENT_FACTS_UNAVAILABLE",
                crate::registries::Phase::PolicyValidation,
                "workspace source is unavailable",
            )),
            Err(error) => Err(public_boundary_error(
                "INTERNAL",
                crate::registries::Phase::PolicyValidation,
                &error.to_string(),
            )),
        }
    };
    if handle.cancelled.load(Ordering::Acquire) {
        return;
    }
    let executed = match executed {
        Ok(executed) => executed,
        Err(error) => {
            let _ = artifacts.persist_query_artifact(&terminal_query_artifact(
                handle.execution.clone(),
                QueryArtifactPhase::Failed,
                Vec::new(),
                None,
                Some(error.code.to_owned()),
            ));
            let execution_state = QueryExecutionState::Failed;
            let freshness_state = match freshness.state() {
                FreshnessState::Current => "CURRENT",
                FreshnessState::PotentiallyStale => "POTENTIALLY_STALE",
                FreshnessState::Unavailable => "UNAVAILABLE",
            };
            append_terminal(
                &handle,
                QueryEvent {
                    event: Some(Event::Terminal(TerminalEvent {
                        header: Some(event_header(&query_id, 1, None)),
                        execution_state: execution_state as i32,
                        availability_state: if error.status.code() == tonic::Code::Unavailable {
                            "UNAVAILABLE"
                        } else {
                            "FAILED"
                        }
                        .to_owned(),
                        freshness_state: freshness_state.to_owned(),
                        limit_state: "NOT_APPLIED".to_owned(),
                        dependency_state: "NOT_EXECUTED".to_owned(),
                        canonical_response_checksum: None,
                        canonical_error_record_json: Some(error.canonical_record_json.into_bytes()),
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
                execution_state,
            )
            .await;
            return;
        }
    };
    let snapshot_id = executed.response.snapshot.snapshot_id.clone();
    let Ok(artifact) = artifacts.insert(
        executed.canonical_bytes,
        &handle.agent_instance_id,
        &handle.workspace_id,
        &snapshot_id,
    ) else {
        append_terminal(
            &handle,
            QueryEvent {
                event: Some(Event::Terminal(TerminalEvent {
                    header: Some(event_header(&query_id, 1, Some(snapshot_id))),
                    execution_state: QueryExecutionState::Failed as i32,
                    availability_state: "UNAVAILABLE".to_owned(),
                    freshness_state: "UNKNOWN".to_owned(),
                    limit_state: "NOT_APPLIED".to_owned(),
                    dependency_state: "SATISFIED".to_owned(),
                    canonical_response_checksum: None,
                    canonical_error_record_json: None,
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
    };
    if handle.cancelled.load(Ordering::Acquire) {
        return;
    }
    let Ok(snapshot_bytes) = crate::contracts::jcs::canonicalize_value(
        &serde_json::to_value(&executed.response.snapshot).unwrap_or(serde_json::Value::Null),
    ) else {
        return;
    };
    if artifacts
        .persist_query_artifact(&terminal_query_artifact(
            handle.execution.clone(),
            QueryArtifactPhase::Succeeded,
            executed.plan_artifacts.clone(),
            Some(artifact.id.clone()),
            None,
        ))
        .is_err()
    {
        append_terminal(
            &handle,
            QueryEvent {
                event: Some(Event::Terminal(TerminalEvent {
                    header: Some(event_header(&query_id, 1, Some(snapshot_id))),
                    execution_state: QueryExecutionState::Failed as i32,
                    availability_state: "UNAVAILABLE".to_owned(),
                    freshness_state: "UNKNOWN".to_owned(),
                    limit_state: "NOT_APPLIED".to_owned(),
                    dependency_state: "SATISFIED".to_owned(),
                    canonical_response_checksum: None,
                    canonical_error_record_json: None,
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
    artifact_records
        .lock()
        .await
        .insert(artifact.id.clone(), artifact.clone());
    let events = vec![
        QueryEvent {
            event: Some(Event::SnapshotPinned(SnapshotPinnedEvent {
                header: Some(event_header(&query_id, 1, Some(snapshot_id.clone()))),
                metadata_checksum: framed_digest(&snapshot_bytes),
                canonical_public_snapshot_metadata_json: snapshot_bytes,
            })),
        },
        QueryEvent {
            event: Some(Event::ArtifactReady(ArtifactReadyEvent {
                header: Some(event_header(&query_id, 2, Some(snapshot_id))),
                artifact_id: artifact.id.clone(),
                artifact_checksum: artifact.checksum.clone(),
                content_type: "application/json".to_owned(),
                encoding: PayloadCompression::Identity as i32,
                lease_expires_at_unix_ms: artifact.lease_expires_at_unix_ms,
                lease_token: artifact.lease_token.clone(),
            })),
        },
        QueryEvent {
            event: Some(Event::Terminal(TerminalEvent {
                header: Some(event_header(&query_id, 3, None)),
                execution_state: QueryExecutionState::Succeeded as i32,
                availability_state: "AVAILABLE".to_owned(),
                freshness_state: crate::registries::registry_state_name(
                    crate::registries::FRESHNESS_STATE_VALUES,
                    executed.response.freshness_state as u16,
                )
                .expect("generated freshness state")
                .to_owned(),
                limit_state: crate::registries::registry_state_name(
                    crate::registries::LIMIT_STATE_VALUES,
                    executed.response.limit_state as u16,
                )
                .expect("generated limit state")
                .to_owned(),
                dependency_state: crate::registries::registry_state_name(
                    crate::registries::DEPENDENCY_STATE_VALUES,
                    crate::registries::DependencyState::Ready as u16,
                )
                .expect("generated dependency state")
                .to_owned(),
                canonical_response_checksum: Some(artifact.checksum.clone()),
                canonical_error_record_json: None,
                artifact_id: Some(artifact.id.clone()),
                result_row_count: executed
                    .response
                    .query_results
                    .iter()
                    .map(|result| result.output_row_count as u64)
                    .sum(),
                result_byte_count: artifact.bytes.len() as u64,
                cleanup_state: "RETAINED_BY_LEASE".to_owned(),
                semantic_execution_state: crate::registries::registry_state_name(
                    crate::registries::QUERY_EXECUTION_STATE_VALUES,
                    executed.response.execution_state as u16,
                )
                .expect("generated query execution state")
                .to_owned(),
                completeness_state: crate::registries::registry_state_name(
                    crate::registries::COMPLETENESS_STATE_VALUES,
                    executed.response.completeness_state as u16,
                )
                .expect("generated completeness state")
                .to_owned(),
                truncated: executed.response.limit_state
                    == crate::registries::LimitState::ExplicitLimitReached,
                query_statuses: executed
                    .response
                    .query_results
                    .iter()
                    .map(|result| QueryStatusSummary {
                        query_id: result.query_id.clone(),
                        execution_state: crate::registries::registry_state_name(
                            crate::registries::QUERY_EXECUTION_STATE_VALUES,
                            result.execution_state as u16,
                        )
                        .expect("generated query execution state")
                        .to_owned(),
                        canonical_error_record_json: result.errors.first().and_then(|error| {
                            crate::contracts::jcs::canonicalize_value(
                                &serde_json::to_value(error).ok()?,
                            )
                            .ok()
                        }),
                        notices: result.notices.clone(),
                    })
                    .collect(),
                notices: executed
                    .response
                    .query_results
                    .iter()
                    .flat_map(|result| result.notices.iter().cloned())
                    .collect(),
            })),
        },
    ];
    let mut state = handle.state.lock().await;
    if state.terminal_state.is_none() && !handle.cancelled.load(Ordering::Acquire) {
        state.events.extend(events);
        state.terminal_state = Some(QueryExecutionState::Succeeded);
        drop(state);
        handle.changed.notify_waiters();
    }
}

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
        self.host_profile_digests.lock().await.insert(
            request.agent_instance_id.clone(),
            derived_host_profile_digest,
        );
        let active_snapshot = self
            .backend
            .public_snapshot(&claims[0].workspace_id)
            .await
            .map_err(|error| Status::failed_precondition(error.to_string()))?;
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
            installed_bundles: vec![self.query_bundle.clone()],
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
        self.authorization
            .authorize_workspace(&request.workspace_id, WorkspacePermission::Query)?;
        let snapshot = self
            .backend
            .public_snapshot(&request.workspace_id)
            .await
            .map_err(|error| Status::failed_precondition(error.to_string()))?;
        let mut capability_statuses = Vec::new();
        if let Some(coverage) = &self.core_source_coverage {
            capability_statuses.push(BTreeMap::from([
                ("capability_code".to_owned(), "CORE_SOURCE_V1".to_owned()),
                ("capability_state".to_owned(), "CURRENT".to_owned()),
                ("reason_code".to_owned(), "NOT_APPLICABLE".to_owned()),
                ("diagnostic_id".to_owned(), "NOT_APPLICABLE".to_owned()),
                (
                    "precision_profile".to_owned(),
                    coverage.precision_profile.to_owned(),
                ),
                (
                    "coverage_profile_id".to_owned(),
                    coverage.coverage_profile_id.clone(),
                ),
                (
                    "coverage_profile_digest".to_owned(),
                    coverage.coverage_profile_digest.clone(),
                ),
                (
                    "scenario_count".to_owned(),
                    coverage.scenario_ids.len().to_string(),
                ),
                (
                    "scenario_ids".to_owned(),
                    coverage
                        .scenario_ids
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(","),
                ),
            ]));
        }
        capability_statuses.extend(snapshot.capability_summaries.clone());
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
        if self
            .host_profile_digests
            .lock()
            .await
            .get(&request.agent_instance_id)
            .is_none_or(|digest| digest != &request.host_capability_profile_digest)
        {
            return Err(Status::failed_precondition(
                "query host profile was not validated by the handshake",
            ));
        }
        validate_checksum(&request.request_checksum, &request.canonical_request_json)?;
        let validated = validate_request(&request.canonical_request_json)
            .map_err(|error| Status::invalid_argument(error.to_string()))?;
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
        if self
            .host_profile_digests
            .lock()
            .await
            .get(&request.agent_instance_id)
            .is_none_or(|digest| digest != &request.host_capability_profile_digest)
        {
            return Err(Status::failed_precondition(
                "query host profile was not validated by the handshake",
            ));
        }
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
        let validated = validate_request(&request.canonical_request_json)
            .map_err(|error| Status::invalid_argument(error.to_string()))?;
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
                Arc::clone(&self.artifact_records),
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
        self.artifacts
            .persist_query_artifact(&terminal_query_artifact(
                handle.execution.clone(),
                QueryArtifactPhase::Cancelled,
                Vec::new(),
                None,
                Some("CANCELLED".to_owned()),
            ))?;
        append_terminal(
            &handle,
            QueryEvent {
                event: Some(Event::Terminal(TerminalEvent {
                    header: Some(event_header(&request.daemon_query_id, 1, None)),
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
        let records = self.artifact_records.lock().await;
        let artifact = records
            .get(&request.artifact_id)
            .ok_or_else(|| Status::not_found("result artifact not found"))?;
        if artifact.lease_token != request.lease_token {
            return Err(Status::permission_denied("result lease token differs"));
        }
        if artifact.lease_expires_at_unix_ms <= now_millis() {
            return Err(Status::failed_precondition("result lease has expired"));
        }
        let offset = usize::try_from(request.offset)
            .map_err(|_| Status::out_of_range("result offset is invalid"))?;
        if offset > artifact.bytes.len() {
            return Err(Status::out_of_range(
                "result offset exceeds artifact length",
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
        let end = offset.saturating_add(maximum).min(artifact.bytes.len());
        let payload = artifact.bytes[offset..end].to_vec();
        let event = ResultChunk {
            artifact_id: artifact.id.clone(),
            offset: request.offset,
            uncompressed_length: payload.len() as u64,
            payload_checksum: framed_digest(&payload),
            payload,
            artifact_checksum: artifact.checksum.clone(),
            content_type: "application/json".to_owned(),
            encoding: PayloadCompression::Identity as i32,
            final_chunk: end == artifact.bytes.len(),
            lease_expires_at_unix_ms: artifact.lease_expires_at_unix_ms,
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
        let mut records = self.artifact_records.lock().await;
        let released = records
            .get(&request.artifact_id)
            .is_some_and(|artifact| artifact.lease_token == request.lease_token);
        if released {
            records.remove(&request.artifact_id);
        }
        Ok(Response::new(ReleaseResultResponse {
            artifact_id: request.artifact_id,
            released,
            remaining_lease_expires_at_unix_ms: None,
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt as _;

    use hyper_util::rt::TokioIo;
    use tokio::net::UnixStream;
    use tokio::sync::oneshot;
    use tonic::transport::Endpoint;
    use tower::service_fn;

    use super::*;

    #[test]
    fn wp61_structural_acceptance() {
        use crate::registries::{PUBLIC_ERROR_ENTRIES, PUBLIC_ERROR_IDS, Phase};

        assert_eq!(PUBLIC_ERROR_ENTRIES.len(), PUBLIC_ERROR_IDS.len());
        for (entry, name) in PUBLIC_ERROR_ENTRIES.iter().zip(PUBLIC_ERROR_IDS) {
            assert_eq!(entry.name, *name);
            let boundary = public_boundary_error(entry.name, Phase::Execution, "fixture");
            assert_eq!(boundary.status.code(), grpc_code(entry.grpc_status));
            let record: serde_json::Value =
                serde_json::from_str(&boundary.canonical_record_json).unwrap();
            assert_eq!(record["code"], entry.code);
            assert_eq!(record["severity"], entry.severity);
            assert_eq!(record["retryability"], entry.retryability);
            assert_eq!(record["grpc_status"], entry.grpc_status);
            assert_eq!(record["mcp_mapping"], entry.mcp_mapping);
        }
    }

    #[test]
    fn wp62_operational_acceptance() {
        use crate::registries::Phase;

        let rejection = public_boundary_error(
            "INVALID_REQUEST_SCHEMA",
            Phase::LogicalPlanning,
            "unapproved table in bound plan",
        );
        assert_eq!(rejection.status.code(), tonic::Code::InvalidArgument);
        let record: serde_json::Value =
            serde_json::from_str(&rejection.canonical_record_json).unwrap();
        assert_eq!(record["name"], "INVALID_REQUEST_SCHEMA");
        assert_eq!(record["phase"], "LOGICAL_PLANNING");
        assert_eq!(record["grpc_status"], "INVALID_ARGUMENT");

        let profile = effective_limits_profile();
        assert_eq!(
            profile.profile_digest,
            limits_profile_digest([
                profile.maximum_control_message_bytes,
                profile.maximum_payload_chunk_bytes,
                profile.maximum_inline_response_bytes,
                u64::from(profile.maximum_concurrent_queries),
                u64::from(profile.query_orphan_replay_seconds),
            ])
        );
        assert_ne!(
            profile.profile_digest,
            limits_profile_digest([
                profile.maximum_control_message_bytes + 1,
                profile.maximum_payload_chunk_bytes,
                profile.maximum_inline_response_bytes,
                u64::from(profile.maximum_concurrent_queries),
                u64::from(profile.query_orphan_replay_seconds),
            ])
        );
    }

    #[test]
    fn wp56_operational_acceptance() {
        let bundle = query_bundle_identity();
        assert_eq!(bundle.bundle_id, "codefabric.bundles.query-language-bundle");
        assert_eq!(bundle.bundle_version, "1.0");
        assert_eq!(
            supported_query_forms(),
            vec![
                "find code entities",
                "retrieve facts about code",
                "follow code relationships",
                "find connecting fact paths",
                "match a code fact pattern",
                "combine result sets",
                "summarize objective facts",
                "retrieve source and syntax context",
            ]
        );
    }
    use crate::rpc::generated::codefabric::cpgd::v1::cpg_query_service_client::CpgQueryServiceClient;
    use crate::rpc::generated::codefabric::cpgd::v1::{CredentialProof, VersionRange};
    use crate::semantic_query::{
        QueryResultRecord, SemanticQueryResponse, SemanticSnapshotResponse,
    };

    struct FakeBackend;

    struct BlockingBackend {
        started: Arc<Notify>,
        release: Arc<Notify>,
        cancelled: Arc<Notify>,
    }

    fn authorization() -> QueryAuthorization {
        QueryAuthorization::new(
            b"test-capability-token",
            vec![WorkspaceClaim {
                workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                repository_id: None,
                worktree_id: None,
                workspace_kind: "directory".to_owned(),
                readiness: WorkspaceReadiness::Ready as i32,
                permission_claims: vec!["query".to_owned()],
            }],
        )
        .unwrap()
    }

    fn fake_snapshot(freshness: FreshnessState) -> SemanticSnapshotResponse {
        SemanticSnapshotResponse {
            snapshot_id: "snapshot:00000000000000000000000000000000".to_owned(),
            workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
            repository_id: None,
            worktree_id: None,
            source_generation: 1,
            source_inventory_digest: framed_digest(b"inventory"),
            durable_base_publication: "publication:00000000000000000000000000000000".to_owned(),
            base_table_version_digest: framed_digest(b"tables"),
            overlay_generation: 0,
            overlay_checksum: framed_digest(b"overlay"),
            analysis_context_set_id: "context-set:00000000000000000000000000000000".to_owned(),
            analysis_context_ids: vec!["context:source".to_owned()],
            freshness_state: freshness,
            source_trust_state: "CURRENT_BYTES_VERIFIED".to_owned(),
            event_stream_health: "HEALTHY".to_owned(),
            git_acceleration_status: "NOT_REQUIRED".to_owned(),
            git_operation_summary: None,
            pending_update_count: 0,
            ontology_version: "1.3".to_owned(),
            schema_bundle_version: "1.0".to_owned(),
            provider_bundle_version: "1.0".to_owned(),
            derivation_bundle_version: "1.0".to_owned(),
            query_language_version: "1.3".to_owned(),
            capability_summaries: Vec::new(),
            diagnostic_references: Vec::new(),
        }
    }

    #[async_trait]
    impl SemanticQueryBackend for FakeBackend {
        async fn execute(
            &self,
            request: ValidatedSemanticRequest,
            freshness: FreshnessState,
            _cancellation: crate::cancellation::Cancellation,
            _execution: QueryExecutionContext,
        ) -> Result<ExecutedSemanticResponse, SemanticQueryError> {
            let response = SemanticQueryResponse {
                specification: "composable semantic CPG fact query response",
                version: "1.3",
                semantic_request_id: request.request.semantic_request_id,
                execution_state: crate::registries::QueryExecutionState::Complete,
                availability_state: crate::registries::QueryAvailabilityState::Available,
                completeness_state: crate::registries::CompletenessState::Complete,
                freshness_state: freshness,
                limit_state: crate::registries::LimitState::NotApplied,
                successful_query_count: 1,
                failed_query_count: 0,
                not_executed_dependency_count: 0,
                snapshot: fake_snapshot(freshness),
                entities: BTreeMap::new(),
                facts: BTreeMap::new(),
                paths: BTreeMap::new(),
                groups: BTreeMap::new(),
                source_contexts: BTreeMap::new(),
                query_results: vec![QueryResultRecord {
                    query_id: "q1".to_owned(),
                    request: crate::semantic_query::QueryForm::FindEntities,
                    execution_state: crate::registries::QueryExecutionState::Complete,
                    availability_state: crate::registries::QueryAvailabilityState::Available,
                    completeness_state: crate::registries::CompletenessState::Complete,
                    freshness_state: freshness,
                    limit_state: crate::registries::LimitState::NotApplied,
                    dependency_state: crate::registries::DependencyState::Ready,
                    resolved_semantics: BTreeMap::new(),
                    entity_ids: Vec::new(),
                    fact_ids: Vec::new(),
                    path_ids: Vec::new(),
                    group_ids: Vec::new(),
                    source_context_ids: Vec::new(),
                    coverage: BTreeMap::new(),
                    errors: Vec::new(),
                    notices: Vec::new(),
                    output_row_count: 1,
                    result_checksum: framed_digest(b"row"),
                }],
                errors: Vec::new(),
            };
            let canonical_bytes = crate::contracts::jcs::canonicalize_value(
                &serde_json::to_value(&response).unwrap(),
            )?;
            Ok(ExecutedSemanticResponse {
                response_digest: framed_digest(&canonical_bytes),
                response,
                canonical_bytes,
                plan_artifacts: Vec::new(),
            })
        }

        async fn public_snapshot(
            &self,
            workspace_id: &str,
        ) -> Result<SemanticSnapshotResponse, SemanticQueryError> {
            let snapshot = fake_snapshot(FreshnessState::Current);
            if snapshot.workspace_id != workspace_id {
                return Err(SemanticQueryError::Invalid("workspace differs".to_owned()));
            }
            Ok(snapshot)
        }
    }

    #[async_trait]
    impl SemanticQueryBackend for BlockingBackend {
        async fn execute(
            &self,
            request: ValidatedSemanticRequest,
            freshness: FreshnessState,
            cancellation: crate::cancellation::Cancellation,
            execution: QueryExecutionContext,
        ) -> Result<ExecutedSemanticResponse, SemanticQueryError> {
            self.started.notify_one();
            loop {
                tokio::select! {
                    () = self.release.notified() => break,
                    () = tokio::time::sleep(std::time::Duration::from_millis(5)) => {
                        if cancellation.is_cancelled() {
                            self.cancelled.notify_one();
                            return Err(SemanticQueryError::Invalid("query cancelled".to_owned()));
                        }
                    }
                }
            }
            SemanticQueryBackend::execute(&FakeBackend, request, freshness, cancellation, execution)
                .await
        }

        async fn public_snapshot(
            &self,
            workspace_id: &str,
        ) -> Result<SemanticSnapshotResponse, SemanticQueryError> {
            SemanticQueryBackend::public_snapshot(&FakeBackend, workspace_id).await
        }
    }

    fn canonical_request() -> Vec<u8> {
        crate::contracts::jcs::canonicalize_slice(br#"{"specification":"composable semantic CPG fact query","version":"1.3","semantic_request_id":"rpc-gate-b","workspace_id":"workspace:00000000000000000000000000000000","freshness_policy":"best_available_snapshot","queries":[{"query_id":"q1","request":"find code entities","label":null,"input":null,"where":null,"limit":{"first":1,"offset":0}}],"response_projection":null,"cost_budget":{"maximum_rows":1}}"#).unwrap()
    }

    fn canonical_current_required_request() -> Vec<u8> {
        crate::contracts::jcs::canonicalize_slice(br#"{"specification":"composable semantic CPG fact query","version":"1.3","semantic_request_id":"rpc-current","workspace_id":"workspace:00000000000000000000000000000000","freshness_policy":"current_required","queries":[{"query_id":"q1","request":"find code entities","label":null,"input":null,"where":null,"limit":{"first":1,"offset":0}}],"response_projection":null,"cost_budget":{"maximum_rows":1}}"#).unwrap()
    }

    fn test_host_profile() -> HostCapabilityProfile {
        let mut profile = HostCapabilityProfile {
            delivery_modes: vec![
                DeliveryPreference::Inline as i32,
                DeliveryPreference::Resource as i32,
                DeliveryPreference::Auto as i32,
            ],
            compression_algorithms: vec![PayloadCompression::Identity as i32],
            supports_resource_links: true,
            supports_trace_context: true,
            maximum_frame_bytes: 1_048_576,
            profile_digest: String::new(),
        };
        profile.profile_digest = host_capability_profile_digest(&profile).unwrap();
        profile
    }

    fn handshake_for(token: &[u8], agent_instance_id: &str) -> HandshakeRequest {
        HandshakeRequest {
            rpc_versions: Some(VersionRange {
                minimum: "1.0".to_owned(),
                maximum: "1.0".to_owned(),
            }),
            semantic_query_versions: Some(VersionRange {
                minimum: "1.3".to_owned(),
                maximum: "1.3".to_owned(),
            }),
            required_feature_bits: CpgdFeatureMask::REQUIRED.bits(),
            optional_feature_bits: CpgdFeatureMask::SUPPORTED
                .missing_from(CpgdFeatureMask::REQUIRED)
                .bits(),
            desired_workspace_ids: vec!["workspace:00000000000000000000000000000000".to_owned()],
            host_capabilities: Some(test_host_profile()),
            credential_proof: Some(CredentialProof {
                credential_id: "test-credential".to_owned(),
                capability_token: token.to_vec(),
            }),
            agent_instance_id: agent_instance_id.to_owned(),
            ..HandshakeRequest::default()
        }
    }

    fn handshake(token: &[u8]) -> HandshakeRequest {
        handshake_for(token, "test-agent")
    }

    fn start_request(agent_instance_id: &str) -> StartQueryRequest {
        StartQueryRequest {
            agent_instance_id: agent_instance_id.to_owned(),
            host_capability_profile_digest: test_host_profile().profile_digest,
            ..StartQueryRequest::default()
        }
    }

    async fn register_host<B: SemanticQueryBackend>(
        service: &ProductionQueryService<B>,
        agent_instance_id: &str,
    ) {
        service
            .handshake(Request::new(handshake_for(
                b"test-capability-token",
                agent_instance_id,
            )))
            .await
            .unwrap();
    }

    async fn start_test_query<B: SemanticQueryBackend>(
        service: &ProductionQueryService<B>,
        agent_instance_id: &str,
        idempotency_key: &str,
    ) -> StartQueryResponse {
        let canonical = canonical_request();
        service
            .start_query(Request::new(StartQueryRequest {
                agent_instance_id: agent_instance_id.to_owned(),
                workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                semantic_query_version: "1.3".to_owned(),
                canonical_request_json: canonical.clone(),
                request_checksum: framed_digest(&canonical),
                delivery_preference: DeliveryPreference::Resource as i32,
                deadline_unix_ms: now_millis() + 60_000,
                idempotency_key: idempotency_key.to_owned(),
                payload_compression: PayloadCompression::Identity as i32,
                ..start_request(agent_instance_id)
            }))
            .await
            .unwrap()
            .into_inner()
    }

    #[tokio::test]
    async fn wp67_behavioral_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let service = ProductionQueryService::new(
            Arc::new(FakeBackend),
            ResultArtifactStore::new(root.path().to_path_buf()).unwrap(),
            authorization(),
            FreshnessBarrier::default(),
            std::time::Duration::from_secs(2),
        );

        let mut incomplete = handshake_for(b"test-capability-token", "wp67-agent");
        incomplete.required_feature_bits = CpgdFeatureMask::NONE.bits();
        incomplete.optional_feature_bits = CpgdFeatureMask::SUPPORTED
            .missing_from(CpgdFeatureMask::QUERY_RESUME)
            .bits();
        let status = service
            .handshake(Request::new(incomplete))
            .await
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::FailedPrecondition);

        register_host(&service, "wp67-agent").await;
        let started = start_test_query(&service, "wp67-agent", "wp67-forged-resume").await;
        let mut legacy_unkeyed = blake3::Hasher::new();
        legacy_unkeyed.update(b"codefabric.query.resume.v1\0");
        legacy_unkeyed.update(started.daemon_query_id.as_bytes());
        let status = service
            .stream_query(Request::new(StreamQueryRequest {
                daemon_query_id: started.daemon_query_id,
                resume_token: legacy_unkeyed.finalize().as_bytes().to_vec(),
                after_sequence: 0,
            }))
            .await
            .err()
            .expect("the legacy public derivation must not authenticate");
        assert_eq!(status.code(), tonic::Code::PermissionDenied);
    }

    #[tokio::test]
    async fn wp67_negative_zero_state() {
        let root = tempfile::tempdir().unwrap();
        let backend = Arc::new(BlockingBackend {
            started: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
            cancelled: Arc::new(Notify::new()),
        });
        let service = ProductionQueryService::new(
            Arc::clone(&backend),
            ResultArtifactStore::new(root.path().to_path_buf()).unwrap(),
            authorization(),
            FreshnessBarrier::default(),
            std::time::Duration::from_secs(2),
        );
        register_host(&service, "wp67-cancel-agent").await;
        let started = start_test_query(&service, "wp67-cancel-agent", "wp67-distinct-token").await;
        backend.started.notified().await;
        assert_ne!(started.resume_token, started.cancel_token);

        let status = service
            .cancel_query(Request::new(CancelQueryRequest {
                daemon_query_id: started.daemon_query_id.clone(),
                cancel_token: started.resume_token.clone(),
                agent_instance_id: "wp67-cancel-agent".to_owned(),
                workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                reason: "wrong token class".to_owned(),
            }))
            .await
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::PermissionDenied);

        let expired = ResultArtifact {
            id: "artifact:wp67-expired".to_owned(),
            checksum: framed_digest(b"expired"),
            lease_token: "expired-lease".to_owned(),
            lease_expires_at_unix_ms: now_millis() - 1,
            bytes: Arc::from(b"expired".as_slice()),
        };
        service
            .artifact_records
            .lock()
            .await
            .insert(expired.id.clone(), expired.clone());
        let status = service
            .read_result(Request::new(ReadResultRequest {
                artifact_id: expired.id,
                offset: 0,
                maximum_bytes: Some(1),
                lease_token: expired.lease_token,
                accepted_compression: PayloadCompression::Identity as i32,
            }))
            .await
            .err()
            .expect("an expired result lease must not produce a stream");
        assert_eq!(status.code(), tonic::Code::FailedPrecondition);

        service
            .cancel_query(Request::new(CancelQueryRequest {
                daemon_query_id: started.daemon_query_id,
                cancel_token: started.cancel_token,
                agent_instance_id: "wp67-cancel-agent".to_owned(),
                workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                reason: "test cleanup".to_owned(),
            }))
            .await
            .unwrap();
        backend.release.notify_one();
    }

    #[tokio::test]
    async fn wp67_operational_acceptance() {
        assert_eq!(CpgdFeatureMask::REQUIRED, CpgdFeatureMask::QUERY_RESUME);
        assert!(
            QueryAuthorization::new(
                b"test-capability-token",
                vec![WorkspaceClaim {
                    workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                    repository_id: None,
                    worktree_id: None,
                    workspace_kind: "directory".to_owned(),
                    readiness: WorkspaceReadiness::Ready as i32,
                    permission_claims: vec!["status-only".to_owned()],
                }],
            )
            .is_err()
        );

        let mut reordered_profile = test_host_profile();
        reordered_profile.delivery_modes.reverse();
        assert_eq!(
            host_capability_profile_digest(&reordered_profile).unwrap(),
            reordered_profile.profile_digest
        );

        let root = tempfile::tempdir().unwrap();
        let service = ProductionQueryService::new(
            Arc::new(FakeBackend),
            ResultArtifactStore::new(root.path().to_path_buf()).unwrap(),
            authorization(),
            FreshnessBarrier::default(),
            std::time::Duration::from_secs(2),
        );
        let handshake = service
            .handshake(Request::new(handshake_for(
                b"test-capability-token",
                "wp67-operational-agent",
            )))
            .await
            .unwrap()
            .into_inner();
        assert_ne!(
            handshake.negotiated_feature_bits & CpgdFeatureMask::REQUIRED.bits(),
            0
        );
        let started =
            start_test_query(&service, "wp67-operational-agent", "wp67-operational").await;
        assert_eq!(started.resume_token.len(), 32);
        assert_eq!(started.cancel_token.len(), 32);
        assert_ne!(started.resume_token, started.cancel_token);
    }

    #[tokio::test]
    async fn wp39_structural_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let socket = root.path().join("query.sock");
        let service = ProductionQueryService::new(
            Arc::new(FakeBackend),
            ResultArtifactStore::new(root.path().join("results")).unwrap(),
            authorization(),
            FreshnessBarrier::default(),
            std::time::Duration::from_secs(2),
        );
        let allowed_uid = fs::metadata(root.path()).unwrap().uid();
        let (shutdown, shutdown_receiver) = oneshot::channel();
        let server_socket = socket.clone();
        let server = tokio::spawn(async move {
            serve_query_uds(&server_socket, allowed_uid, service, async {
                let _ = shutdown_receiver.await;
            })
            .await
        });
        while !socket.exists() {
            tokio::task::yield_now().await;
        }
        let connector_socket = socket.clone();
        let channel = Endpoint::try_from("http://[::]:50051")
            .unwrap()
            .connect_with_connector(service_fn(move |_| {
                let connector_socket = connector_socket.clone();
                async move {
                    UnixStream::connect(connector_socket)
                        .await
                        .map(TokioIo::new)
                }
            }))
            .await
            .unwrap();
        let mut client = CpgQueryServiceClient::new(channel);
        assert_eq!(
            client
                .handshake(handshake(b"wrong-capability-token"))
                .await
                .unwrap_err()
                .code(),
            tonic::Code::Unauthenticated
        );
        let response = client
            .handshake(handshake(b"test-capability-token"))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.authorized_workspaces.len(), 1);
        assert_eq!(response.negotiated_rpc_version, "1.0");
        shutdown.send(()).unwrap();
        server.await.unwrap().unwrap();
        assert!(!socket.exists());
    }

    #[tokio::test]
    async fn wp39_behavioral_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let service = ProductionQueryService::new(
            Arc::new(FakeBackend),
            ResultArtifactStore::new(root.path().to_path_buf()).unwrap(),
            authorization(),
            FreshnessBarrier::default(),
            std::time::Duration::from_secs(2),
        );
        register_host(&service, "test-agent").await;
        let canonical = canonical_request();
        let started = service
            .start_query(Request::new(StartQueryRequest {
                agent_instance_id: "test-agent".to_owned(),
                workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                semantic_query_version: "1.3".to_owned(),
                canonical_request_json: canonical.clone(),
                request_checksum: framed_digest(&canonical),
                delivery_preference: DeliveryPreference::Resource as i32,
                deadline_unix_ms: now_millis() + 60_000,
                idempotency_key: "same-request".to_owned(),
                payload_compression: PayloadCompression::Identity as i32,
                ..start_request("test-agent")
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(
            started.query_execution_state,
            QueryExecutionState::Accepted as i32
        );
        let events = service
            .stream_query(Request::new(StreamQueryRequest {
                daemon_query_id: started.daemon_query_id,
                resume_token: started.resume_token,
                after_sequence: 0,
            }))
            .await
            .unwrap()
            .into_inner();
        let events = futures::StreamExt::collect::<Vec<_>>(events).await;
        assert_eq!(events.len(), 3);
        assert!(matches!(
            events[0].as_ref().unwrap().event.as_ref(),
            Some(Event::SnapshotPinned(_))
        ));
        assert!(matches!(
            events[2].as_ref().unwrap().event.as_ref(),
            Some(Event::Terminal(_))
        ));
        let artifact = service
            .artifact_records
            .lock()
            .await
            .values()
            .next()
            .cloned()
            .unwrap();
        let result = service
            .read_result(Request::new(ReadResultRequest {
                artifact_id: artifact.id,
                offset: 0,
                maximum_bytes: Some(MAX_PAYLOAD_CHUNK_BYTES as u64),
                lease_token: artifact.lease_token,
                accepted_compression: PayloadCompression::Identity as i32,
            }))
            .await
            .unwrap()
            .into_inner();
        let chunks = futures::StreamExt::collect::<Vec<_>>(result).await;
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].as_ref().unwrap().final_chunk);
    }

    #[tokio::test]
    async fn wp65_operational_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let service = ProductionQueryService::new(
            Arc::new(FakeBackend),
            ResultArtifactStore::new(root.path().to_path_buf()).unwrap(),
            authorization(),
            FreshnessBarrier::default(),
            std::time::Duration::from_secs(2),
        );
        register_host(&service, "artifact-agent").await;
        let canonical = canonical_request();
        let started = service
            .start_query(Request::new(StartQueryRequest {
                agent_instance_id: "artifact-agent".to_owned(),
                workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                mcp_call_id: "mcp-call-65".to_owned(),
                rpc_attempt_id: "rpc-attempt-65".to_owned(),
                semantic_request_id: Some("rpc-gate-b".to_owned()),
                semantic_query_version: "1.3".to_owned(),
                canonical_request_json: canonical.clone(),
                request_checksum: framed_digest(&canonical),
                delivery_preference: DeliveryPreference::Resource as i32,
                deadline_unix_ms: now_millis() + 60_000,
                idempotency_key: "wp65-persist".to_owned(),
                payload_compression: PayloadCompression::Identity as i32,
                ..start_request("artifact-agent")
            }))
            .await
            .unwrap()
            .into_inner();
        let execution_id = started.daemon_query_id.clone();
        let events = service
            .stream_query(Request::new(StreamQueryRequest {
                daemon_query_id: started.daemon_query_id,
                resume_token: started.resume_token,
                after_sequence: 0,
            }))
            .await
            .unwrap()
            .into_inner();
        let events = futures::StreamExt::collect::<Vec<_>>(events).await;
        assert_eq!(events.len(), 3);

        let persisted = service
            .artifacts
            .read_query_artifact(&execution_id)
            .unwrap();
        assert_eq!(persisted.phase, QueryArtifactPhase::Succeeded);
        assert_eq!(persisted.execution.semantic_request_id, "rpc-gate-b");
        assert_eq!(persisted.execution.mcp_call_id, "mcp-call-65");
        assert!(persisted.result_artifact_id.is_some());
        assert!(persisted.expires_at_unix_ms > persisted.created_at_unix_ms);
        assert_eq!(
            service
                .artifacts
                .prune_expired_query_artifacts(persisted.created_at_unix_ms)
                .unwrap(),
            0
        );
        assert_eq!(
            service
                .artifacts
                .prune_expired_query_artifacts(persisted.expires_at_unix_ms)
                .unwrap(),
            1
        );
        assert_eq!(
            service
                .artifacts
                .read_query_artifact(&execution_id)
                .unwrap_err()
                .code(),
            tonic::Code::NotFound
        );
    }

    #[tokio::test]
    async fn wp65_negative_zero_state() {
        let root = tempfile::tempdir().unwrap();
        let backend = Arc::new(BlockingBackend {
            started: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
            cancelled: Arc::new(Notify::new()),
        });
        let service = ProductionQueryService::new(
            Arc::clone(&backend),
            ResultArtifactStore::new(root.path().to_path_buf()).unwrap(),
            authorization(),
            FreshnessBarrier::default(),
            std::time::Duration::from_secs(2),
        );
        register_host(&service, "cancel-agent").await;
        let canonical = canonical_request();
        let started = service
            .start_query(Request::new(StartQueryRequest {
                agent_instance_id: "cancel-agent".to_owned(),
                workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                mcp_call_id: "mcp-cancel-65".to_owned(),
                semantic_query_version: "1.3".to_owned(),
                canonical_request_json: canonical.clone(),
                request_checksum: framed_digest(&canonical),
                delivery_preference: DeliveryPreference::Resource as i32,
                deadline_unix_ms: now_millis() + 60_000,
                idempotency_key: "wp65-cancel".to_owned(),
                payload_compression: PayloadCompression::Identity as i32,
                ..start_request("cancel-agent")
            }))
            .await
            .unwrap()
            .into_inner();
        backend.started.notified().await;
        service
            .cancel_query(Request::new(CancelQueryRequest {
                daemon_query_id: started.daemon_query_id.clone(),
                cancel_token: started.cancel_token,
                agent_instance_id: "cancel-agent".to_owned(),
                workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                reason: "drop the stream".to_owned(),
            }))
            .await
            .unwrap();
        let artifact = service
            .artifacts
            .read_query_artifact(&started.daemon_query_id)
            .unwrap();
        assert_eq!(artifact.phase, QueryArtifactPhase::Cancelled);
        assert_eq!(artifact.public_error_code.as_deref(), Some("CANCELLED"));
        assert!(artifact.plan_artifacts.is_empty());
        assert!(artifact.result_artifact_id.is_none());
        backend.release.notify_one();
    }

    #[tokio::test]
    async fn wp39_negative_zero_state() {
        let root = tempfile::tempdir().unwrap();
        let service = ProductionQueryService::new(
            Arc::new(FakeBackend),
            ResultArtifactStore::new(root.path().to_path_buf()).unwrap(),
            authorization(),
            FreshnessBarrier::default(),
            std::time::Duration::from_secs(2),
        );
        register_host(&service, "test-agent").await;
        let canonical = canonical_request();
        let status = service
            .start_query(Request::new(StartQueryRequest {
                agent_instance_id: "test-agent".to_owned(),
                workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                canonical_request_json: canonical,
                request_checksum: framed_digest(b"wrong"),
                delivery_preference: DeliveryPreference::Inline as i32,
                deadline_unix_ms: now_millis() - 1,
                idempotency_key: "expired".to_owned(),
                payload_compression: PayloadCompression::Identity as i32,
                ..start_request("test-agent")
            }))
            .await
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::DeadlineExceeded);
    }

    #[tokio::test]
    async fn wp39_operational_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let backend = Arc::new(BlockingBackend {
            started: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
            cancelled: Arc::new(Notify::new()),
        });
        let service = ProductionQueryService::new(
            Arc::clone(&backend),
            ResultArtifactStore::new(root.path().to_path_buf()).unwrap(),
            authorization(),
            FreshnessBarrier::default(),
            std::time::Duration::from_secs(2),
        );
        register_host(&service, "test-agent").await;
        let canonical = canonical_request();
        let started = service
            .start_query(Request::new(StartQueryRequest {
                agent_instance_id: "test-agent".to_owned(),
                workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                semantic_query_version: "1.3".to_owned(),
                canonical_request_json: canonical.clone(),
                request_checksum: framed_digest(&canonical),
                delivery_preference: DeliveryPreference::Resource as i32,
                deadline_unix_ms: now_millis() + 60_000,
                idempotency_key: "cancel-request".to_owned(),
                payload_compression: PayloadCompression::Identity as i32,
                ..start_request("test-agent")
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(
            started.query_execution_state,
            QueryExecutionState::Accepted as i32
        );
        backend.started.notified().await;
        let cancelled = service
            .cancel_query(Request::new(CancelQueryRequest {
                daemon_query_id: started.daemon_query_id.clone(),
                cancel_token: started.cancel_token.clone(),
                agent_instance_id: "test-agent".to_owned(),
                workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                reason: "test cancellation".to_owned(),
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(cancelled.state, CancellationState::Cancelled as i32);
        let events = service
            .stream_query(Request::new(StreamQueryRequest {
                daemon_query_id: started.daemon_query_id,
                resume_token: started.resume_token,
                after_sequence: 0,
            }))
            .await
            .unwrap()
            .into_inner();
        let events = futures::StreamExt::collect::<Vec<_>>(events).await;
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0].as_ref().unwrap().event.as_ref(),
            Some(Event::Terminal(value))
                if value.execution_state == QueryExecutionState::Cancelled as i32
        ));
        backend.release.notify_one();
        tokio::task::yield_now().await;
        assert!(service.artifact_records.lock().await.is_empty());
    }

    async fn query_client(socket: PathBuf) -> CpgQueryServiceClient<tonic::transport::Channel> {
        let channel = Endpoint::try_from("http://[::]:50051")
            .unwrap()
            .connect_with_connector(service_fn(move |_| {
                let socket = socket.clone();
                async move { UnixStream::connect(socket).await.map(TokioIo::new) }
            }))
            .await
            .unwrap();
        CpgQueryServiceClient::new(channel)
    }

    #[test]
    fn wp63_structural_acceptance() {
        let daemon = include_str!("daemon.rs");
        let coordinator = include_str!("coordinator.rs");
        assert!(daemon.contains("ProductionQueryService::new("));
        assert!(daemon.contains("serve_query_uds("));
        assert!(daemon.contains("WorkspaceQueryBackend"));
        assert!(daemon.contains("query_socket_endpoint"));
        assert!(coordinator.contains("build_continuous_engine("));
        assert!(coordinator.contains("ContinuousWorkspaceEngine::new("));
    }

    #[tokio::test]
    async fn wp63_negative_zero_state() {
        let root = tempfile::tempdir().unwrap();
        let socket = root.path().join("query.sock");
        let service = ProductionQueryService::new(
            Arc::new(FakeBackend),
            ResultArtifactStore::new(root.path().join("results")).unwrap(),
            authorization(),
            FreshnessBarrier::default(),
            std::time::Duration::from_secs(2),
        );
        let allowed_uid = fs::metadata(root.path()).unwrap().uid().saturating_add(1);
        let (shutdown, shutdown_receiver) = oneshot::channel();
        let server_socket = socket.clone();
        let server = tokio::spawn(async move {
            serve_query_uds(&server_socket, allowed_uid, service, async {
                let _ = shutdown_receiver.await;
            })
            .await
        });
        while !socket.exists() {
            tokio::task::yield_now().await;
        }
        let mut client = query_client(socket.clone()).await;
        let status = client
            .handshake(handshake(b"test-capability-token"))
            .await
            .unwrap_err();
        assert_ne!(status.code(), tonic::Code::Ok);
        assert!(!status.message().is_empty());
        shutdown.send(()).unwrap();
        server.await.unwrap().unwrap();
        assert!(!socket.exists());
    }

    #[tokio::test]
    async fn wp63_operational_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let socket = root.path().join("query.sock");
        let backend = Arc::new(BlockingBackend {
            started: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
            cancelled: Arc::new(Notify::new()),
        });
        let service = ProductionQueryService::new(
            Arc::clone(&backend),
            ResultArtifactStore::new(root.path().join("results")).unwrap(),
            authorization(),
            FreshnessBarrier::default(),
            std::time::Duration::from_secs(2),
        );
        let allowed_uid = fs::metadata(root.path()).unwrap().uid();
        let (shutdown, shutdown_receiver) = oneshot::channel();
        let server_socket = socket.clone();
        let server = tokio::spawn(async move {
            serve_query_uds(&server_socket, allowed_uid, service, async {
                let _ = shutdown_receiver.await;
            })
            .await
        });
        while !socket.exists() {
            tokio::task::yield_now().await;
        }
        let mut client = query_client(socket.clone()).await;
        client
            .handshake(handshake(b"test-capability-token"))
            .await
            .unwrap();
        let canonical = canonical_request();
        client
            .start_query(StartQueryRequest {
                agent_instance_id: "test-agent".to_owned(),
                workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                semantic_query_version: "1.3".to_owned(),
                canonical_request_json: canonical.clone(),
                request_checksum: framed_digest(&canonical),
                delivery_preference: DeliveryPreference::Resource as i32,
                deadline_unix_ms: now_millis() + 60_000,
                idempotency_key: "daemon-shutdown-cancellation".to_owned(),
                payload_compression: PayloadCompression::Identity as i32,
                ..start_request("test-agent")
            })
            .await
            .unwrap();
        backend.started.notified().await;
        drop(client);
        shutdown.send(()).unwrap();
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            backend.cancelled.notified(),
        )
        .await
        .expect("in-flight backend observed daemon cancellation");
        server.await.unwrap().unwrap();
        assert!(!socket.exists());
    }

    #[tokio::test]
    async fn wp45_strict_current_query_is_rejected_before_backend_execution() {
        let root = tempfile::tempdir().unwrap();
        let freshness = FreshnessBarrier::default();
        let _ = freshness.admit();
        let service = ProductionQueryService::new(
            Arc::new(FakeBackend),
            ResultArtifactStore::new(root.path().to_path_buf()).unwrap(),
            authorization(),
            freshness,
            std::time::Duration::from_millis(50),
        );
        register_host(&service, "test-agent").await;
        let canonical = canonical_current_required_request();
        let started = service
            .start_query(Request::new(StartQueryRequest {
                agent_instance_id: "test-agent".to_owned(),
                workspace_id: "workspace:00000000000000000000000000000000".to_owned(),
                semantic_query_version: "1.3".to_owned(),
                canonical_request_json: canonical.clone(),
                request_checksum: framed_digest(&canonical),
                delivery_preference: DeliveryPreference::Resource as i32,
                deadline_unix_ms: now_millis() + 1_000,
                idempotency_key: "strict-stale".to_owned(),
                payload_compression: PayloadCompression::Identity as i32,
                ..start_request("test-agent")
            }))
            .await
            .unwrap()
            .into_inner();
        let events = service
            .stream_query(Request::new(StreamQueryRequest {
                daemon_query_id: started.daemon_query_id,
                resume_token: started.resume_token,
                after_sequence: 0,
            }))
            .await
            .unwrap()
            .into_inner();
        let events = futures::StreamExt::collect::<Vec<_>>(events).await;
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0].as_ref().unwrap().event.as_ref(),
            Some(Event::Terminal(value))
                if value.execution_state == QueryExecutionState::Failed as i32
                    && value.freshness_state == "POTENTIALLY_STALE"
        ));
        assert!(service.artifact_records.lock().await.is_empty());
    }
}
