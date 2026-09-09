//! Thin `codefabric.cpgd.v2` transport over lifecycle, session, coordinator, and result authority.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use futures::{Stream, stream};
use prost::Message as _;
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use tonic::metadata::MetadataValue;
use tonic::{Code, Request, Response, Status};

use crate::fabric::arrow_result_resource::QueryExecutionPin;
use crate::fabric::command::{LeaseId, PrincipalId, WorkspaceId};
use crate::fabric::production_kernel::{
    CompiledSemanticRelease, LifecycleAuthority, ProductionLifecyclePhase, WorkspaceSlotRegistry,
};
use crate::fabric::programmatic_workspace::WorkspaceEpochQueryAuthority;
use crate::fabric::published_arrow_result::{OpaqueResultLeaseToken, PublishedResultOwner};
use crate::fabric::query_coordinator::{
    NormalizedQueryOperation, QueryAcceptanceOutcome, QueryControlEvent, QueryControlEventPayload,
    QueryCoordinator, QueryCoordinatorError, QueryExecutionPermit, QueryExecutionPhase,
    QuerySessionAuthority, QuerySessionSharingClass, QueryTerminalState,
};
use crate::fabric::streamed_result_package::{
    PendingResultObjectSet, ResultPublicationIntentError, ResultPublicationIntentRecorder,
};
use crate::fabric::streamed_result_registry::{
    ReferenceResourcePublication, StreamedReferenceSelector, StreamedReleaseOutcome,
    StreamedResourceRead, StreamedResourceSelector, StreamedResultRegistry,
    StreamedResultRegistryError,
};
use crate::fabric::{QueryExecutionArtifactAccumulator, QueryExecutionContext};
use crate::identity::{IdentityDomain, decode_public_id};
use crate::query_backend::{
    PreparedSemanticExecution, ResolvedSemanticExecutionRequest, SemanticBackendExecutionContext,
    SemanticBackendOutcome, SemanticCollectionItemKind, SemanticExecutionPreparation,
    SemanticInputAnswer, SemanticInputConstraints, SemanticInputKind, SemanticInputRequirement,
    SemanticInputValue, SemanticQueryBackend, SemanticStringFormat, now_millis,
};
#[cfg(test)]
use crate::registries::FreshnessState;
use crate::relational_semantic_query::{ProducerFamilyDisposition, SemanticValueKind};
use crate::rpc::generated::codefabric::cpgd::v2::authorized_choice::Value as WireChoiceValue;
use crate::rpc::generated::codefabric::cpgd::v2::challenge_constraints::Constraint as WireConstraint;
use crate::rpc::generated::codefabric::cpgd::v2::cpg_query_service_server::CpgQueryService;
use crate::rpc::generated::codefabric::cpgd::v2::get_reference_request::Operation as ReferenceOperation;
use crate::rpc::generated::codefabric::cpgd::v2::get_reference_response::Result as ReferenceResult;
use crate::rpc::generated::codefabric::cpgd::v2::input_answer::Value as WireAnswerValue;
use crate::rpc::generated::codefabric::cpgd::v2::query_event::Event;
use crate::rpc::generated::codefabric::cpgd::v2::resource_selector::Selector as WireResourceSelector;
use crate::rpc::generated::codefabric::cpgd::v2::start_query_request::Leg as StartLeg;
use crate::rpc::generated::codefabric::cpgd::v2::start_query_response::Outcome as StartOutcome;
use crate::rpc::generated::codefabric::cpgd::v2::{
    AcceptedQuery, AuthorityGeneration, AuthorizedChoice, CancelQueryRequest, CancelQueryResponse,
    CancellationAcknowledgement, ChallengeCollectionConstraints, ChallengeCollectionItemKind,
    ChallengeConstraints, ChallengeEnumConstraints, ChallengeExplanationCode, ChallengeInputKind,
    ChallengeIntegerConstraints, ChallengeStringConstraints, ChallengeStringFormat,
    EffectiveLimits, GetReferenceRequest, GetReferenceResponse, GetStatusRequest,
    GetStatusResponse, HandshakeRequest, HandshakeResponse, InputAnswer, InputChallenge,
    InputRequirement, LifecycleState, ProgressEvent, ProgressStage, QueryChallengeContinuation,
    QueryEvent, QueryEventHeader, QueryExecutionState, QueryPreparation, QuerySubmission,
    ReadResourceRequest, ReferenceCompletion, ReferenceCompletionCandidate,
    ReferenceCompletionRequest, ReferenceDocument, ReferenceKind, ReferenceTemplateVariable,
    ReleaseResourceRequest, ReleaseResourceResponse, ReleaseState, RequestContext,
    ReservedControlContract, ReservedControlOperation, ResourceChunk, ResourceDescriptor,
    ResourceKind, ResultReadyEvent, SafeDiagnosticReference, SafeErrorCode, SafeErrorLayer,
    SafeErrorMetadata, SnapshotPinnedEvent, StartQueryRequest, StartQueryResponse, TerminalEvent,
    TerminalObservation, ValidateQueryRequest, ValidateQueryResponse, ValidationIssue,
    ValidationRejection, WatchQueryRequest,
};
use crate::rpc::{
    MAX_CONTROL_MESSAGE_BYTES, MAX_PAYLOAD_CHUNK_BYTES, MAX_QUERY_TRANSPORT_STREAMS,
    VerifiedPeerIdentity,
};
use crate::semantic_query_contract::{FreshnessPolicy, SemanticQueryError, parse_request};
use crate::session_authority::{
    AuthorizedSession, LaunchGrantAuthority, SESSION_METADATA_KEY, SessionAuthorityError,
    SessionOperation,
};

type QueryEventStream = Pin<Box<dyn Stream<Item = Result<QueryEvent, Status>> + Send>>;
type ResourceStream = Pin<Box<dyn Stream<Item = Result<ResourceChunk, Status>> + Send>>;

const RPC_MINOR: u32 = 0;
const SEMANTIC_PROFILE: &str = "codefabric.semantic-query.v2";
const MAX_EXECUTION_BUDGET: Duration = Duration::from_secs(300);
const RESULT_LEASE_GRACE: Duration = Duration::from_mins(30);
const MAX_REFERENCE_COMPLETION_CANDIDATES: u32 = 100;
const MAX_VALIDATION_ISSUES: u32 = 64;
const MAX_CHALLENGES: usize = 128;
const MAX_START_OUTCOMES: usize = 256;
const MAX_CHALLENGE_FIELDS: usize = 16;
const MAX_CHALLENGE_CHOICES: usize = 64;
const MAX_CHALLENGE_COLLECTION_ITEMS: u32 = 256;
const MAX_CHALLENGE_ANSWER_BYTES: usize = 64 * 1024;
const MAX_CHALLENGE_MESSAGE_BYTES: usize = 128 * 1024;
const MAX_CHALLENGE_STRING_BYTES: usize = 4 * 1024;
const MAX_CHALLENGE_ROUNDS: u32 = 3;
const CHALLENGE_TTL: Duration = Duration::from_secs(60);
const CHALLENGE_TOMBSTONE_TTL: Duration = Duration::from_secs(60);
const RESERVED_CONTROL_CAPACITY: usize = 1;

#[derive(Debug)]
struct RpcAdmission {
    data: Arc<Semaphore>,
    control: Arc<Semaphore>,
    control_waiters: Arc<Semaphore>,
}

impl RpcAdmission {
    fn new(data_capacity: usize, control_wait_capacity: usize) -> Self {
        assert!(
            data_capacity > 0 && control_wait_capacity > 0,
            "RPC data and control-wait capacities are nonzero"
        );
        Self {
            data: Arc::new(Semaphore::new(data_capacity)),
            control: Arc::new(Semaphore::new(RESERVED_CONTROL_CAPACITY)),
            control_waiters: Arc::new(Semaphore::new(control_wait_capacity)),
        }
    }

    fn data(&self) -> Result<OwnedSemaphorePermit, Status> {
        acquire_rpc_permit(Arc::clone(&self.data))
    }

    async fn control(&self, budget: RpcBudget) -> Result<OwnedSemaphorePermit, Status> {
        // Bound the cross-connection waiter population before entering Tokio's fair semaphore
        // queue. The waiter permit is needed only while admission is pending; once the execution
        // permit is acquired, another bounded waiter may enter the queue.
        let waiter = acquire_rpc_permit(Arc::clone(&self.control_waiters))?;
        let admitted = budget
            .run(async {
                Arc::clone(&self.control)
                    .acquire_owned()
                    .await
                    .map_err(|_| public_status(Code::Unavailable, "RPC_ADMISSION_CLOSED"))
            })
            .await?;
        drop(waiter);
        Ok(admitted)
    }
}

fn acquire_rpc_permit(semaphore: Arc<Semaphore>) -> Result<OwnedSemaphorePermit, Status> {
    semaphore
        .try_acquire_owned()
        .map_err(|_| public_status(Code::ResourceExhausted, "RPC_CAPACITY"))
}

#[derive(Clone, Copy, Debug)]
struct RpcBudget {
    deadline: Instant,
}

impl RpcBudget {
    fn from_duration(duration: Duration) -> Result<Self, Status> {
        let deadline = Instant::now()
            .checked_add(duration)
            .ok_or_else(|| public_status(Code::InvalidArgument, "EXECUTION_BUDGET"))?;
        Ok(Self { deadline })
    }

    fn remaining(self) -> Result<Duration, Status> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| public_status(Code::DeadlineExceeded, "RPC_BUDGET_EXHAUSTED"))
    }

    async fn run<T, F>(self, future: F) -> Result<T, Status>
    where
        F: Future<Output = Result<T, Status>>,
    {
        let deadline = tokio::time::Instant::from_std(self.deadline);
        tokio::time::timeout_at(deadline, future)
            .await
            .map_err(|_| public_status(Code::DeadlineExceeded, "RPC_BUDGET_EXHAUSTED"))?
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LiveReferenceEntry {
    kind: ReferenceKind,
    public_kind: &'static str,
    presentation_key: &'static str,
    released_version: String,
    content: Vec<u8>,
}

/// One authorization-time projection of the installed workspace reference relation.
///
/// The projection owns canonical bytes so completion and the eventual reference read select from
/// the same live relation without retaining a workspace lease across an RPC await. The protocol
/// enum remains only a selector vocabulary; it is never treated as evidence that a corresponding
/// reference exists.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct LiveReferenceProjection {
    entries: BTreeMap<ReferenceKind, LiveReferenceEntry>,
}

impl LiveReferenceProjection {
    fn from_workspace(
        release: &CompiledSemanticRelease,
        authority: &WorkspaceEpochQueryAuthority,
        lifecycle: ProductionLifecyclePhase,
        lifecycle_sequence: u64,
    ) -> Result<Self, Status> {
        let forms = authority
            .advertised_semantic_forms()
            .map_err(|_| public_status(Code::Internal, "REFERENCE_AUTHORITY"))?;
        let released_version = release.suite().as_str();
        let query_forms = forms.iter().map(|form| form.label()).collect::<Vec<_>>();
        let ingress = authority.ingress_catalog();
        let execution = authority.execution_catalog();
        let producer_closure = authority.producer_closure();
        let supported_bindings = ingress
            .program_bindings
            .iter()
            .filter(|binding| forms.contains(&binding.compatibility_form))
            .collect::<Vec<_>>();
        let supported_program_ids = supported_bindings
            .iter()
            .map(|binding| binding.program_binding_id.as_ref())
            .collect::<BTreeSet<_>>();

        let capabilities = producer_closure
            .families
            .iter()
            .map(|family| {
                let availability = match family.disposition {
                    ProducerFamilyDisposition::RuntimeProducer(_) => "available",
                    ProducerFamilyDisposition::UnsupportedRemainder(_) => "unsupported",
                };
                serde_json::json!({
                    "family": family.family_id,
                    "availability": availability,
                })
            })
            .collect::<Vec<_>>();
        let programs = supported_bindings
            .iter()
            .map(|binding| {
                serde_json::json!({
                    "form": binding.compatibility_form.label(),
                    "output_role": binding.output_role_id,
                })
            })
            .collect::<Vec<_>>();
        let request_schemas = supported_bindings
            .iter()
            .map(|binding| {
                let selections = ingress
                    .selections
                    .iter()
                    .filter(|row| row.program_binding_id == binding.program_binding_id)
                    .map(|row| {
                        serde_json::json!({
                            "field": row.selection_id,
                            "value_kind": semantic_value_kind_name(row.value_kind),
                            "minimum_values": row.minimum_values,
                            "maximum_values": row.maximum_values,
                            "resolution_required": !row.resolutions.is_empty(),
                        })
                    })
                    .collect::<Vec<_>>();
                let returns = ingress
                    .returns
                    .iter()
                    .filter(|row| row.program_binding_id == binding.program_binding_id)
                    .map(|row| {
                        serde_json::json!({
                            "field": row.return_id,
                            "value_kind": semantic_value_kind_name(row.value_kind),
                            "minimum_values": row.minimum_values,
                            "maximum_values": row.maximum_values,
                        })
                    })
                    .collect::<Vec<_>>();
                let relation_inputs = ingress
                    .request_inputs
                    .iter()
                    .filter(|row| row.program_binding_id == binding.program_binding_id)
                    .map(|row| {
                        serde_json::json!({
                            "input": row.input_id,
                            "minimum_rows": row.minimum_rows,
                            "maximum_rows": row.maximum_rows,
                            "fields": row.fields.iter().map(|field| serde_json::json!({
                                "field": field.field_id.as_str(),
                                "value_kind": semantic_value_kind_name(field.value_kind),
                                "required": field.required,
                            })).collect::<Vec<_>>(),
                        })
                    })
                    .collect::<Vec<_>>();
                serde_json::json!({
                    "form": binding.compatibility_form.label(),
                    "selections": selections,
                    "returns": returns,
                    "relation_inputs": relation_inputs,
                })
            })
            .collect::<Vec<_>>();
        let response_schemas = execution
            .programs
            .iter()
            .filter(|row| supported_program_ids.contains(row.program_binding_id.as_ref()))
            .filter_map(|row| {
                supported_bindings
                    .iter()
                    .find(|binding| binding.program_binding_id == row.program_binding_id)
                    .map(|binding| {
                        serde_json::json!({
                            "form": binding.compatibility_form.label(),
                            "output_role": binding.output_role_id,
                            "field_count": row.output_fields.len(),
                        })
                    })
            })
            .collect::<Vec<_>>();
        let common = serde_json::json!({
            "suite": release.suite().as_str(),
            "released_version": released_version,
            "semantic_profile": SEMANTIC_PROFILE,
            "workspace_epoch": format!("epoch:{}", hex(authority.epoch_id().as_bytes())),
            "lifecycle": lifecycle.code(),
            "lifecycle_sequence": lifecycle_sequence,
        });
        let documents = [
            (
                ReferenceKind::Capability,
                serde_json::json!({"capabilities": capabilities}),
            ),
            (
                ReferenceKind::Guide,
                serde_json::json!({"query_forms": query_forms}),
            ),
            (
                ReferenceKind::Recipe,
                serde_json::json!({"programs": programs}),
            ),
            (
                ReferenceKind::RequestSchema,
                serde_json::json!({"request_schemas": request_schemas}),
            ),
            (
                ReferenceKind::ResponseSchema,
                serde_json::json!({"response_schemas": response_schemas}),
            ),
            (
                ReferenceKind::Snapshot,
                serde_json::json!({
                    "query_forms": forms.len(),
                    "capability_rows": producer_closure.families.len(),
                    "program_rows": supported_bindings.len(),
                }),
            ),
        ];
        let mut entries = BTreeMap::new();
        for (kind, projection) in documents {
            let (public_kind, presentation_key) = reference_kind_projection(kind)
                .ok_or_else(|| public_status(Code::Internal, "REFERENCE_AUTHORITY"))?;
            let content = serde_json_canonicalizer::to_vec(&serde_json::json!({
                "kind": public_kind,
                "authority": common,
                "projection": projection,
            }))
            .map_err(|_| public_status(Code::Internal, "REFERENCE_ENCODING"))?;
            let prior = entries.insert(
                kind,
                LiveReferenceEntry {
                    kind,
                    public_kind,
                    presentation_key,
                    released_version: released_version.to_owned(),
                    content,
                },
            );
            if prior.is_some() {
                return Err(public_status(Code::Internal, "REFERENCE_AUTHORITY"));
            }
        }
        Ok(Self { entries })
    }

    fn resolve(
        &self,
        kind: ReferenceKind,
        version: Option<&str>,
    ) -> Result<&LiveReferenceEntry, Status> {
        self.entries
            .get(&kind)
            .filter(|entry| version.is_none_or(|value| value == entry.released_version))
            .ok_or_else(|| public_status(Code::PermissionDenied, "REFERENCE_SELECTOR"))
    }
}

#[derive(Clone, Debug)]
struct PreparedSubmission {
    parsed: crate::semantic_query_contract::ParsedSemanticRequest,
    answers: Vec<SemanticInputAnswer>,
    requirements: Vec<SemanticInputRequirement>,
    resolved: Option<ResolvedSemanticExecutionRequest>,
}

#[derive(Clone, Debug)]
struct ChallengeRecord {
    challenge_id: String,
    round: u32,
    issued_at_unix_ms: i64,
    expires_at_unix_ms: i64,
    session_id: String,
    session_generation: u64,
    daemon_generation: u64,
    policy_generation: u64,
    revocation_generation: u64,
    principal_id: PrincipalId,
    workspace_id: WorkspaceId,
    semantic_request_id: String,
    semantic_profile: String,
    maximum_result_bytes: u64,
    maximum_result_pages: u64,
    prepared: PreparedSubmission,
    requirements: Vec<SemanticInputRequirement>,
    used: bool,
    scope: StartScope,
    fingerprint: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ChallengeTombstoneDisposition {
    Expired,
    Used,
}

#[derive(Clone, Debug)]
struct ChallengeTombstone {
    challenge_id: String,
    round: u32,
    session_id: String,
    session_generation: u64,
    daemon_generation: u64,
    policy_generation: u64,
    revocation_generation: u64,
    principal_id: PrincipalId,
    workspace_id: WorkspaceId,
    semantic_request_id: String,
    scope: StartScope,
    fingerprint: [u8; 32],
    retain_until_unix_ms: i64,
    disposition: ChallengeTombstoneDisposition,
}

#[derive(Clone, Debug)]
struct ReadyStart<A> {
    prepared: PreparedSemanticExecution<A>,
    semantic_request_id: String,
    semantic_profile: String,
    maximum_result_bytes: u64,
    maximum_result_pages: u64,
    scope: StartScope,
    fingerprint: [u8; 32],
    pending_reserved: bool,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct StartScope {
    principal_id: PrincipalId,
    workspace_id: WorkspaceId,
    policy_generation: u64,
    revocation_generation: u64,
    semantic_request_id: String,
}

#[derive(Clone, Debug)]
enum StartOutcomeRecord {
    Pending {
        fingerprint: [u8; 32],
        expires_at_unix_ms: i64,
    },
    Challenge {
        fingerprint: [u8; 32],
        token: [u8; 32],
    },
    Accepted {
        fingerprint: [u8; 32],
        acceptance: crate::fabric::query_coordinator::QueryAcceptance,
        semantic_request_id: String,
    },
    Rejected {
        fingerprint: [u8; 32],
        expires_at_unix_ms: i64,
        rejection: StoredValidationRejection,
    },
}

#[derive(Clone, Debug)]
struct StoredValidationRejection {
    semantic_request_id: Option<String>,
    issues: Vec<ValidationIssue>,
    error: SafeErrorMetadata,
}

impl StoredValidationRejection {
    fn to_wire(&self, session: &AuthorizedSession, correlation_id: &str) -> ValidationRejection {
        let mut error = self.error.clone();
        error.correlation_id = correlation_id.to_owned();
        ValidationRejection {
            authority: Some(authority(session)),
            semantic_request_id: self.semantic_request_id.clone(),
            issues: self.issues.clone(),
            error: Some(error),
        }
    }
}

enum ChallengeContinuationOutcome<A> {
    Ready(ReadyStart<A>),
    Closed(StartOutcome),
}

#[derive(Debug, Default)]
struct StartState {
    challenges: BTreeMap<[u8; 32], ChallengeRecord>,
    challenge_tombstones: BTreeMap<[u8; 32], ChallengeTombstone>,
    outcomes: BTreeMap<StartScope, StartOutcomeRecord>,
}

#[derive(Clone)]
pub struct QueryApplicationService<B: SemanticQueryBackend> {
    release: Arc<CompiledSemanticRelease>,
    backend: Arc<B>,
    lifecycle: Arc<LifecycleAuthority>,
    workspace_slots: Arc<WorkspaceSlotRegistry>,
    coordinator: Arc<QueryCoordinator>,
    sessions: Arc<LaunchGrantAuthority>,
    results: Arc<StreamedResultRegistry>,
    retention_running: Arc<AtomicBool>,
    retention_failed: Arc<AtomicBool>,
    retention_sequence: Arc<AtomicU64>,
    starts: Arc<Mutex<StartState>>,
    daemon_instance_id: Arc<str>,
}

impl<B: SemanticQueryBackend> std::fmt::Debug for QueryApplicationService<B> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("QueryApplicationService")
            .field("semantic_release", &self.release.suite().as_str())
            .field("lifecycle", &self.lifecycle.observe())
            .field("workspace_count", &self.workspace_slots.len())
            .field("daemon_instance_id", &self.daemon_instance_id)
            .finish_non_exhaustive()
    }
}

impl<B: SemanticQueryBackend> QueryApplicationService<B> {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    fn new(
        release: Arc<CompiledSemanticRelease>,
        backend: Arc<B>,
        lifecycle: Arc<LifecycleAuthority>,
        workspace_slots: Arc<WorkspaceSlotRegistry>,
        coordinator: Arc<QueryCoordinator>,
        sessions: Arc<LaunchGrantAuthority>,
        results: Arc<StreamedResultRegistry>,
        daemon_instance_id: impl Into<Arc<str>>,
    ) -> Self {
        if let Some(builder) = backend.retained_package_builder() {
            results.install_package_builder(builder);
        }
        Self {
            release,
            backend,
            lifecycle,
            workspace_slots,
            coordinator,
            sessions,
            results,
            retention_running: Arc::new(AtomicBool::new(false)),
            retention_failed: Arc::new(AtomicBool::new(false)),
            retention_sequence: Arc::new(AtomicU64::new(0)),
            starts: Arc::new(Mutex::new(StartState::default())),
            daemon_instance_id: daemon_instance_id.into(),
        }
    }

    #[must_use]
    pub(crate) const fn release(&self) -> &Arc<CompiledSemanticRelease> {
        &self.release
    }

    async fn authorize<T>(
        &self,
        request: &Request<T>,
        operation: SessionOperation,
        workspace_id: Option<WorkspaceId>,
    ) -> Result<AuthorizedSession, Status> {
        let peer = peer(request)?;
        let token = session_token(request)?;
        let observed_at_unix_ms = now_millis();
        let session = self
            .sessions
            .authorize(&token, peer, operation, workspace_id, observed_at_unix_ms)
            .await
            .map_err(session_status)?;
        if self.retention_failed.load(Ordering::Acquire) {
            return Err(public_status(
                Code::Unavailable,
                "RESULT_RECOVERY_UNAVAILABLE",
            ));
        }
        self.schedule_retention(observed_at_unix_ms).await?;
        Ok(session)
    }

    async fn schedule_retention(&self, observed_at_unix_ms: i64) -> Result<(), Status> {
        if self
            .retention_running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(());
        }
        let results = Arc::clone(&self.results);
        let coordinator = Arc::clone(&self.coordinator);
        let running = Arc::clone(&self.retention_running);
        let failed = Arc::clone(&self.retention_failed);
        let sequence = self.retention_sequence.fetch_add(1, Ordering::AcqRel);
        let task_name = format!("retention:{sequence}");
        if let Err(error) = self
            .coordinator
            .spawn_service_task(&task_name, async move {
                if collect_retention_authority(&results, &coordinator, observed_at_unix_ms)
                    .await
                    .is_err()
                {
                    failed.store(true, Ordering::Release);
                }
                running.store(false, Ordering::Release);
            })
            .await
        {
            self.retention_running.store(false, Ordering::Release);
            self.retention_failed.store(true, Ordering::Release);
            return Err(public_status(
                Code::Unavailable,
                if matches!(error, QueryCoordinatorError::TaskCapacity) {
                    "RETENTION_TASK_CAPACITY"
                } else {
                    "RETENTION_TASK_OWNERSHIP"
                },
            ));
        }
        Ok(())
    }

    async fn processing_resource(
        &self,
        query_id: &str,
        session: &AuthorizedSession,
        read: &StreamedResourceRead,
    ) -> Result<String, Status> {
        let (workspace, locator) = self
            .coordinator
            .retained_result_selection(query_id, query_session_authority(session)?)
            .await
            .map_err(coordinator_status)?;
        if !session.workspace_ids().contains(&workspace) {
            return Err(public_status(Code::PermissionDenied, "WORKSPACE_DENIED"));
        }
        match self.results.processing_handle(query_id, read).await {
            Ok(handle) => return Ok(handle),
            Err(StreamedResultRegistryError::UnknownPackage) => {}
            Err(error) => return Err(result_status(error)),
        }
        match self
            .results
            .reissue_retained(
                query_id,
                session.principal_id(),
                workspace,
                session.daemon_generation(),
                session.policy_generation(),
                session.revocation_generation(),
                &locator,
                now_millis(),
            )
            .await
        {
            Ok(_) | Err(StreamedResultRegistryError::PackageIdentityCollision) => {}
            Err(error) => return Err(result_status(error)),
        }
        self.results
            .processing_handle(query_id, read)
            .await
            .map_err(result_status)
    }

    async fn collect_retention(&self, observed_at_unix_ms: i64) -> Result<(), Status> {
        collect_retention_authority(&self.results, &self.coordinator, observed_at_unix_ms).await
    }

    async fn validate_request(
        &self,
        session: &AuthorizedSession,
        canonical_request: &[u8],
        request_checksum: &str,
        semantic_profile: &str,
        maximum_result_bytes: u64,
        maximum_result_pages: u64,
    ) -> Result<crate::semantic_query_contract::ParsedSemanticRequest, Status> {
        if request_checksum != crate::integrity::framed_digest(canonical_request) {
            return Err(public_status(Code::InvalidArgument, "REQUEST_INTEGRITY"));
        }
        if semantic_profile != session.semantic_profile()
            || semantic_profile != SEMANTIC_PROFILE
            || maximum_result_bytes == 0
            || maximum_result_pages == 0
            || maximum_result_bytes > session.maximum_result_bytes()
            || maximum_result_pages > session.maximum_result_pages()
        {
            return Err(public_status(Code::PermissionDenied, "REQUEST_SCOPE"));
        }
        let parsed = parse_request(canonical_request).map_err(semantic_status)?;
        let workspace = workspace_id(&parsed.request.workspace_id)?;
        if !session.permits_workspace(workspace) {
            return Err(public_status(Code::PermissionDenied, "WORKSPACE_DENIED"));
        }
        Ok(parsed)
    }

    async fn validate_submission(
        &self,
        session: &AuthorizedSession,
        query: &QuerySubmission,
    ) -> Result<PreparedSubmission, Status> {
        let limits = query
            .result_limits
            .as_ref()
            .ok_or_else(|| public_status(Code::InvalidArgument, "RESULT_LIMITS"))?;
        let parsed = self
            .validate_request(
                session,
                &query.canonical_request_json,
                &query.request_checksum,
                &query.semantic_profile,
                limits.maximum_result_bytes,
                limits.maximum_result_pages,
            )
            .await?;
        if query
            .semantic_request_id
            .as_ref()
            .is_some_and(|identity| identity != &parsed.request.semantic_request_id)
        {
            return Err(public_status(Code::InvalidArgument, "SEMANTIC_REQUEST_ID"));
        }
        self.prepare_semantic_submission(parsed, Vec::new()).await
    }

    async fn prepare_semantic_submission(
        &self,
        parsed: crate::semantic_query_contract::ParsedSemanticRequest,
        answers: Vec<SemanticInputAnswer>,
    ) -> Result<PreparedSubmission, Status> {
        self.backend
            .await_freshness(&parsed)
            .await
            .map_err(semantic_status)?;
        let preparation = self
            .backend
            .prepare_execution_request(&parsed, &answers)
            .map_err(semantic_status)?;
        let (requirements, resolved) = match preparation {
            SemanticExecutionPreparation::InputRequired(requirements) => {
                validate_requirements(&requirements)?;
                if requirements.is_empty() {
                    return Err(public_status(Code::Internal, "CHALLENGE_REQUIREMENTS"));
                }
                (requirements, None)
            }
            SemanticExecutionPreparation::Ready(resolved) => {
                if resolved.parsed() != &parsed || resolved.answers() != answers {
                    return Err(public_status(Code::Internal, "RESOLVED_OPERATION"));
                }
                (Vec::new(), Some(resolved))
            }
        };
        Ok(PreparedSubmission {
            parsed,
            answers,
            requirements,
            resolved,
        })
    }

    async fn issue_challenge(
        &self,
        session: &AuthorizedSession,
        prepared: PreparedSubmission,
        scope: StartScope,
        fingerprint: [u8; 32],
        semantic_profile: String,
        maximum_result_bytes: u64,
        maximum_result_pages: u64,
        observed_at_unix_ms: i64,
        correlation_id: &str,
    ) -> Result<StartOutcome, Status> {
        let token = random32().map_err(|()| public_status(Code::Internal, "CHALLENGE_ENTROPY"))?;
        let challenge_id = format!("challenge:{}", hex(blake3::hash(&token).as_bytes()));
        let ttl_millis = challenge_ttl_millis(session);
        let expires_at_unix_ms = observed_at_unix_ms
            .saturating_add(ttl_millis)
            .min(session.expires_at_unix_ms());
        if expires_at_unix_ms <= observed_at_unix_ms {
            return Err(public_status(Code::DeadlineExceeded, "CHALLENGE_EXPIRED"));
        }
        let requirements = prepared.requirements.clone();
        let record = ChallengeRecord {
            challenge_id: challenge_id.clone(),
            round: 1,
            issued_at_unix_ms: observed_at_unix_ms,
            expires_at_unix_ms,
            session_id: session.session_id().to_owned(),
            session_generation: session.session_generation(),
            daemon_generation: session.daemon_generation(),
            policy_generation: session.policy_generation(),
            revocation_generation: session.revocation_generation(),
            principal_id: session.principal_id(),
            workspace_id: scope.workspace_id,
            semantic_request_id: scope.semantic_request_id.clone(),
            semantic_profile,
            maximum_result_bytes,
            maximum_result_pages,
            prepared,
            requirements: requirements.clone(),
            used: false,
            scope: scope.clone(),
            fingerprint,
        };
        let mut starts = self.starts.lock().await;
        prune_start_state(&mut starts, observed_at_unix_ms);
        if let Some(outcome) =
            replay_start_outcome(&starts, &scope, fingerprint, session, correlation_id)?
        {
            return Ok(outcome);
        }
        if challenge_record_count(&starts) >= MAX_CHALLENGES
            || starts.outcomes.len() >= MAX_START_OUTCOMES
        {
            return Err(public_status(Code::ResourceExhausted, "CHALLENGE_CAPACITY"));
        }
        if starts.challenges.contains_key(&token)
            || starts.challenge_tombstones.contains_key(&token)
            || starts.challenges.insert(token, record).is_some()
        {
            return Err(public_status(Code::Internal, "CHALLENGE_IDENTITY"));
        }
        starts
            .outcomes
            .insert(scope, StartOutcomeRecord::Challenge { fingerprint, token });
        let record = starts
            .challenges
            .get(&token)
            .expect("challenge was just inserted");
        Ok(StartOutcome::InputChallenge(challenge_wire(
            record, token, session,
        )?))
    }

    async fn replay_start(
        &self,
        session: &AuthorizedSession,
        scope: &StartScope,
        fingerprint: [u8; 32],
        observed_at_unix_ms: i64,
        correlation_id: &str,
    ) -> Result<Option<StartOutcome>, Status> {
        let mut starts = self.starts.lock().await;
        prune_start_state(&mut starts, observed_at_unix_ms);
        replay_start_outcome(&starts, scope, fingerprint, session, correlation_id)
    }

    async fn record_accepted_start(
        &self,
        ready: &ReadyStart<B::ExecutionAuthority>,
        acceptance: crate::fabric::query_coordinator::QueryAcceptance,
        semantic_request_id: String,
    ) -> Result<(), Status> {
        let mut starts = self.starts.lock().await;
        if let Some(existing) = starts.outcomes.get(&ready.scope) {
            let existing_fingerprint = match existing {
                StartOutcomeRecord::Pending { fingerprint, .. }
                | StartOutcomeRecord::Challenge { fingerprint, .. }
                | StartOutcomeRecord::Accepted { fingerprint, .. }
                | StartOutcomeRecord::Rejected { fingerprint, .. } => fingerprint,
            };
            if existing_fingerprint != &ready.fingerprint {
                return Err(public_status(Code::AlreadyExists, "IDEMPOTENCY_CONFLICT"));
            }
        } else if starts.outcomes.len() >= MAX_START_OUTCOMES {
            return Err(public_status(Code::ResourceExhausted, "START_CAPACITY"));
        }
        starts.outcomes.insert(
            ready.scope.clone(),
            StartOutcomeRecord::Accepted {
                fingerprint: ready.fingerprint,
                acceptance,
                semantic_request_id,
            },
        );
        Ok(())
    }

    async fn reserve_start_acceptance(
        &self,
        scope: &StartScope,
        fingerprint: [u8; 32],
        observed_at_unix_ms: i64,
    ) -> Result<(), Status> {
        let mut starts = self.starts.lock().await;
        prune_start_state(&mut starts, observed_at_unix_ms);
        if starts.outcomes.contains_key(scope) {
            return Err(public_status(Code::Aborted, "START_IN_PROGRESS"));
        }
        if starts.outcomes.len() >= MAX_START_OUTCOMES {
            return Err(public_status(Code::ResourceExhausted, "START_CAPACITY"));
        }
        let ttl_millis = i64::try_from(CHALLENGE_TTL.as_millis()).unwrap_or(i64::MAX);
        starts.outcomes.insert(
            scope.clone(),
            StartOutcomeRecord::Pending {
                fingerprint,
                expires_at_unix_ms: observed_at_unix_ms.saturating_add(ttl_millis),
            },
        );
        Ok(())
    }

    async fn clear_pending_start(&self, scope: &StartScope, fingerprint: [u8; 32]) {
        let mut starts = self.starts.lock().await;
        if matches!(
            starts.outcomes.get(scope),
            Some(StartOutcomeRecord::Pending {
                fingerprint: observed,
                ..
            }) if *observed == fingerprint
        ) {
            starts.outcomes.remove(scope);
        }
    }

    async fn consume_challenge(
        &self,
        session: &AuthorizedSession,
        continuation: QueryChallengeContinuation,
        observed_at_unix_ms: i64,
        correlation_id: &str,
    ) -> Result<ChallengeContinuationOutcome<B::ExecutionAuthority>, Status> {
        let encoded_answer_bytes = continuation
            .answers
            .iter()
            .try_fold(0_usize, |total, answer| {
                total.checked_add(answer.encoded_len())
            })
            .ok_or_else(|| public_status(Code::ResourceExhausted, "CHALLENGE_ANSWERS"))?;
        if encoded_answer_bytes > MAX_CHALLENGE_ANSWER_BYTES {
            return Err(public_status(Code::ResourceExhausted, "CHALLENGE_ANSWERS"));
        }
        let token: [u8; 32] = continuation
            .daemon_continuation
            .as_slice()
            .try_into()
            .map_err(|_| public_status(Code::InvalidArgument, "CHALLENGE_CONTINUATION"))?;
        let mut starts = self.starts.lock().await;
        let record = if let Some(record) = starts.challenges.get(&token).cloned() {
            record
        } else {
            let tombstone = starts
                .challenge_tombstones
                .get(&token)
                .cloned()
                .ok_or_else(|| public_status(Code::InvalidArgument, "CHALLENGE_CONTINUATION"))?;
            if !challenge_tombstone_authorizes(&tombstone, &continuation, session) {
                return Err(public_status(Code::PermissionDenied, "CHALLENGE_BINDING"));
            }
            return match tombstone.disposition {
                ChallengeTombstoneDisposition::Used => {
                    Err(public_status(Code::InvalidArgument, "CHALLENGE_REPLAY"))
                }
                ChallengeTombstoneDisposition::Expired => {
                    let status = public_status(Code::InvalidArgument, "CHALLENGE_EXPIRED");
                    Ok(close_tombstoned_challenge_rejection(
                        &mut starts,
                        &tombstone,
                        token,
                        session,
                        &status,
                        correlation_id,
                    ))
                }
            };
        };
        if record.used {
            return Err(public_status(Code::InvalidArgument, "CHALLENGE_REPLAY"));
        }
        if continuation.challenge_id != record.challenge_id
            || continuation.round != record.round
            || record.session_id != session.session_id()
            || record.session_generation != session.session_generation()
            || record.daemon_generation != session.daemon_generation()
            || record.policy_generation != session.policy_generation()
            || record.revocation_generation != session.revocation_generation()
            || record.principal_id != session.principal_id()
            || !session.permits_workspace(record.workspace_id)
        {
            return Err(public_status(Code::PermissionDenied, "CHALLENGE_BINDING"));
        }
        if observed_at_unix_ms >= record.expires_at_unix_ms {
            let status = public_status(Code::InvalidArgument, "CHALLENGE_EXPIRED");
            return Ok(close_challenge_rejection(
                &mut starts,
                &record,
                token,
                session,
                &status,
                correlation_id,
            ));
        }
        let answers = match answers_from_wire(&record.requirements, &continuation.answers) {
            Ok(answers) => answers,
            Err(status) => {
                return Ok(close_challenge_rejection(
                    &mut starts,
                    &record,
                    token,
                    session,
                    &status,
                    correlation_id,
                ));
            }
        };
        let mut accumulated_answers = record.prepared.answers.clone();
        accumulated_answers.extend(answers);
        drop(starts);
        let preparation_started = Instant::now();
        let prepared_result = self
            .prepare_semantic_submission(record.prepared.parsed.clone(), accumulated_answers)
            .await;
        let admitted_result = if let Ok(prepared) = &prepared_result {
            if let Some(resolved) = prepared.resolved.clone() {
                Some(self.backend.admit_fresh_execution_request(resolved).await)
            } else {
                None
            }
        } else {
            None
        };
        let observed_at_unix_ms = observed_at_unix_ms.saturating_add(
            i64::try_from(preparation_started.elapsed().as_millis()).unwrap_or(i64::MAX),
        );
        let mut starts = self.starts.lock().await;
        if starts
            .challenges
            .get(&token)
            .is_none_or(|record| record.used)
        {
            return Err(public_status(Code::InvalidArgument, "CHALLENGE_REPLAY"));
        }
        if observed_at_unix_ms >= record.expires_at_unix_ms {
            let status = public_status(Code::InvalidArgument, "CHALLENGE_EXPIRED");
            return Ok(close_challenge_rejection(
                &mut starts,
                &record,
                token,
                session,
                &status,
                correlation_id,
            ));
        }
        let prepared = match prepared_result {
            Ok(prepared) => prepared,
            Err(status) if status.code() == Code::InvalidArgument => {
                return Ok(close_challenge_rejection(
                    &mut starts,
                    &record,
                    token,
                    session,
                    &status,
                    correlation_id,
                ));
            }
            Err(status) => return Err(status),
        };
        if let Some(admitted_result) = admitted_result {
            let admitted = match admitted_result {
                Ok(admitted) => admitted,
                Err(error) => {
                    let status = semantic_status(error);
                    return Ok(close_challenge_rejection(
                        &mut starts,
                        &record,
                        token,
                        session,
                        &status,
                        correlation_id,
                    ));
                }
            };
            if let Some(consumed) = starts.challenges.get_mut(&token) {
                consumed.used = true;
            }
            let ttl_millis = challenge_ttl_millis(session);
            starts.outcomes.insert(
                record.scope.clone(),
                StartOutcomeRecord::Pending {
                    fingerprint: record.fingerprint,
                    expires_at_unix_ms: observed_at_unix_ms.saturating_add(ttl_millis),
                },
            );
            return Ok(ChallengeContinuationOutcome::Ready(ReadyStart {
                prepared: admitted,
                semantic_request_id: record.semantic_request_id,
                semantic_profile: record.semantic_profile,
                maximum_result_bytes: record.maximum_result_bytes,
                maximum_result_pages: record.maximum_result_pages,
                scope: record.scope,
                fingerprint: record.fingerprint,
                pending_reserved: true,
            }));
        }
        let next_round = match next_challenge_round(record.round) {
            Ok(round) => round,
            Err(status) => {
                return Ok(close_challenge_rejection(
                    &mut starts,
                    &record,
                    token,
                    session,
                    &status,
                    correlation_id,
                ));
            }
        };
        let next_token =
            random32().map_err(|()| public_status(Code::Internal, "CHALLENGE_ENTROPY"))?;
        let next_challenge_id = format!("challenge:{}", hex(blake3::hash(&next_token).as_bytes()));
        let ttl_millis = challenge_ttl_millis(session);
        let expires_at_unix_ms = observed_at_unix_ms
            .saturating_add(ttl_millis)
            .min(session.expires_at_unix_ms());
        if expires_at_unix_ms <= observed_at_unix_ms {
            let status = public_status(Code::InvalidArgument, "CHALLENGE_EXPIRED");
            return Ok(close_challenge_rejection(
                &mut starts,
                &record,
                token,
                session,
                &status,
                correlation_id,
            ));
        }
        let next_record = ChallengeRecord {
            challenge_id: next_challenge_id,
            round: next_round,
            issued_at_unix_ms: observed_at_unix_ms,
            expires_at_unix_ms,
            session_id: record.session_id,
            session_generation: record.session_generation,
            daemon_generation: record.daemon_generation,
            policy_generation: record.policy_generation,
            revocation_generation: record.revocation_generation,
            principal_id: record.principal_id,
            workspace_id: record.workspace_id,
            semantic_request_id: record.semantic_request_id,
            semantic_profile: record.semantic_profile,
            maximum_result_bytes: record.maximum_result_bytes,
            maximum_result_pages: record.maximum_result_pages,
            requirements: prepared.requirements.clone(),
            prepared,
            used: false,
            scope: record.scope.clone(),
            fingerprint: record.fingerprint,
        };
        if challenge_record_count(&starts) >= MAX_CHALLENGES {
            return Err(public_status(Code::ResourceExhausted, "CHALLENGE_CAPACITY"));
        }
        if let Some(consumed) = starts.challenges.get_mut(&token) {
            consumed.used = true;
        }
        if starts.challenges.contains_key(&next_token)
            || starts.challenge_tombstones.contains_key(&next_token)
            || starts.challenges.insert(next_token, next_record).is_some()
        {
            return Err(public_status(Code::Internal, "CHALLENGE_IDENTITY"));
        }
        starts.outcomes.insert(
            record.scope,
            StartOutcomeRecord::Challenge {
                fingerprint: record.fingerprint,
                token: next_token,
            },
        );
        let next_record = starts
            .challenges
            .get(&next_token)
            .expect("next challenge was just inserted");
        Ok(ChallengeContinuationOutcome::Closed(
            StartOutcome::InputChallenge(challenge_wire(next_record, next_token, session)?),
        ))
    }
}

/// Thin Tonic transport adapter over the application-owned query service.
///
/// The adapter stores no semantic, lifecycle, session, result, or release authority of its own.
/// Generated request/response values terminate in this module and application state remains in
/// [`QueryApplicationService`].
#[derive(Clone)]
pub struct ProductionQueryService<B: SemanticQueryBackend> {
    application: Arc<QueryApplicationService<B>>,
    admission: Arc<RpcAdmission>,
}

#[derive(Debug, thiserror::Error)]
pub enum QueryServiceCompositionError {
    #[error("semantic backend is bound to a different compiled release")]
    ReleaseMismatch,
}

impl<B: SemanticQueryBackend> ProductionQueryService<B> {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        release: Arc<CompiledSemanticRelease>,
        backend: Arc<B>,
        lifecycle: Arc<LifecycleAuthority>,
        workspace_slots: Arc<WorkspaceSlotRegistry>,
        coordinator: Arc<QueryCoordinator>,
        sessions: Arc<LaunchGrantAuthority>,
        results: Arc<StreamedResultRegistry>,
        daemon_instance_id: impl Into<Arc<str>>,
    ) -> Result<Self, QueryServiceCompositionError> {
        if backend.application_release_pin().is_some_and(|observed| {
            observed
                != crate::fabric::programmatic_query_backend::compiled_query_release_pin(
                    release.as_ref(),
                )
        }) {
            return Err(QueryServiceCompositionError::ReleaseMismatch);
        }
        let transport_data_capacity = usize::try_from(MAX_QUERY_TRANSPORT_STREAMS / 2)
            .expect("transport stream bound fits usize");
        let transport_control_wait_capacity = usize::try_from(MAX_QUERY_TRANSPORT_STREAMS / 2)
            .expect("transport control-wait bound fits usize");
        let admission = Arc::new(RpcAdmission::new(
            coordinator.maximum_tasks().min(transport_data_capacity),
            transport_control_wait_capacity,
        ));
        Ok(Self {
            application: Arc::new(QueryApplicationService::new(
                release,
                backend,
                lifecycle,
                workspace_slots,
                coordinator,
                sessions,
                results,
                daemon_instance_id,
            )),
            admission,
        })
    }

    #[must_use]
    pub(crate) const fn application(&self) -> &Arc<QueryApplicationService<B>> {
        &self.application
    }
}

impl<B: SemanticQueryBackend> Deref for ProductionQueryService<B> {
    type Target = QueryApplicationService<B>;

    fn deref(&self) -> &Self::Target {
        self.application.as_ref()
    }
}

impl<B: SemanticQueryBackend> std::fmt::Debug for ProductionQueryService<B> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProductionQueryServiceV2TransportAdapter")
            .field("application", &self.application)
            .finish()
    }
}

fn close_challenge_rejection<A>(
    starts: &mut StartState,
    record: &ChallengeRecord,
    token: [u8; 32],
    session: &AuthorizedSession,
    status: &Status,
    correlation_id: &str,
) -> ChallengeContinuationOutcome<A> {
    if let Some(consumed) = starts.challenges.get_mut(&token) {
        consumed.used = true;
    }
    let semantic_request_id = Some(record.prepared.parsed.request.semantic_request_id.clone());
    let issues = vec![validation_issue(status)];
    let mut error = public_error_detail(status.code(), status_public_code(status));
    error.retryable = false;
    error.diagnostic_reference = Some(SafeDiagnosticReference::QueryChallengeRejected as i32);
    error.correlation_id = correlation_id.to_owned();
    let rejection = ValidationRejection {
        authority: Some(authority(session)),
        semantic_request_id: semantic_request_id.clone(),
        issues: issues.clone(),
        error: Some(error.clone()),
    };
    error.correlation_id.clear();
    starts.outcomes.insert(
        record.scope.clone(),
        StartOutcomeRecord::Rejected {
            fingerprint: record.fingerprint,
            expires_at_unix_ms: record.expires_at_unix_ms,
            rejection: StoredValidationRejection {
                semantic_request_id,
                issues,
                error,
            },
        },
    );
    ChallengeContinuationOutcome::Closed(StartOutcome::ValidationRejection(rejection))
}

fn challenge_tombstone_authorizes(
    tombstone: &ChallengeTombstone,
    continuation: &QueryChallengeContinuation,
    session: &AuthorizedSession,
) -> bool {
    continuation.challenge_id == tombstone.challenge_id
        && continuation.round == tombstone.round
        && tombstone.session_id == session.session_id()
        && tombstone.session_generation == session.session_generation()
        && tombstone.daemon_generation == session.daemon_generation()
        && tombstone.policy_generation == session.policy_generation()
        && tombstone.revocation_generation == session.revocation_generation()
        && tombstone.principal_id == session.principal_id()
        && session.permits_workspace(tombstone.workspace_id)
}

fn close_tombstoned_challenge_rejection<A>(
    starts: &mut StartState,
    tombstone: &ChallengeTombstone,
    token: [u8; 32],
    session: &AuthorizedSession,
    status: &Status,
    correlation_id: &str,
) -> ChallengeContinuationOutcome<A> {
    if let Some(consumed) = starts.challenge_tombstones.get_mut(&token) {
        consumed.disposition = ChallengeTombstoneDisposition::Used;
    }
    let semantic_request_id = Some(tombstone.semantic_request_id.clone());
    let issues = vec![validation_issue(status)];
    let mut error = public_error_detail(status.code(), status_public_code(status));
    error.retryable = false;
    error.diagnostic_reference = Some(SafeDiagnosticReference::QueryChallengeRejected as i32);
    error.correlation_id = correlation_id.to_owned();
    let rejection = ValidationRejection {
        authority: Some(authority(session)),
        semantic_request_id: semantic_request_id.clone(),
        issues: issues.clone(),
        error: Some(error.clone()),
    };
    error.correlation_id.clear();
    starts.outcomes.insert(
        tombstone.scope.clone(),
        StartOutcomeRecord::Rejected {
            fingerprint: tombstone.fingerprint,
            expires_at_unix_ms: tombstone.retain_until_unix_ms,
            rejection: StoredValidationRejection {
                semantic_request_id,
                issues,
                error,
            },
        },
    );
    ChallengeContinuationOutcome::Closed(StartOutcome::ValidationRejection(rejection))
}

fn start_scope_and_fingerprint(
    session: &AuthorizedSession,
    parsed: &crate::semantic_query_contract::ParsedSemanticRequest,
    semantic_profile: &str,
    maximum_result_bytes: u64,
    maximum_result_pages: u64,
) -> Result<(StartScope, [u8; 32]), Status> {
    let scope = StartScope {
        principal_id: session.principal_id(),
        workspace_id: workspace_id(&parsed.request.workspace_id)?,
        policy_generation: session.policy_generation(),
        revocation_generation: session.revocation_generation(),
        semantic_request_id: parsed.request.semantic_request_id.clone(),
    };
    let fingerprint = identity32(
        b"codefabric.atomic-start-meaning.v2",
        &[
            &parsed.canonical_bytes,
            semantic_profile.as_bytes(),
            &maximum_result_bytes.to_be_bytes(),
            &maximum_result_pages.to_be_bytes(),
        ],
    );
    Ok((scope, fingerprint))
}

fn prune_start_state(starts: &mut StartState, observed_at_unix_ms: i64) {
    starts
        .challenge_tombstones
        .retain(|_, tombstone| tombstone.retain_until_unix_ms > observed_at_unix_ms);
    let tombstone_ttl_millis =
        i64::try_from(CHALLENGE_TOMBSTONE_TTL.as_millis()).unwrap_or(i64::MAX);
    let expired_tokens = starts
        .challenges
        .iter()
        .filter_map(|(token, record)| {
            (record.expires_at_unix_ms <= observed_at_unix_ms).then_some(*token)
        })
        .collect::<Vec<_>>();
    for token in expired_tokens {
        let record = starts
            .challenges
            .remove(&token)
            .expect("expired challenge was selected from the same bounded map");
        let retain_until_unix_ms = record
            .expires_at_unix_ms
            .saturating_add(tombstone_ttl_millis);
        if retain_until_unix_ms > observed_at_unix_ms {
            let prior = starts.challenge_tombstones.insert(
                token,
                ChallengeTombstone {
                    challenge_id: record.challenge_id,
                    round: record.round,
                    session_id: record.session_id,
                    session_generation: record.session_generation,
                    daemon_generation: record.daemon_generation,
                    policy_generation: record.policy_generation,
                    revocation_generation: record.revocation_generation,
                    principal_id: record.principal_id,
                    workspace_id: record.workspace_id,
                    semantic_request_id: record.semantic_request_id,
                    scope: record.scope,
                    fingerprint: record.fingerprint,
                    retain_until_unix_ms,
                    disposition: if record.used {
                        ChallengeTombstoneDisposition::Used
                    } else {
                        ChallengeTombstoneDisposition::Expired
                    },
                },
            );
            debug_assert!(prior.is_none(), "live and tombstoned tokens are disjoint");
        }
    }
    let live_challenges = starts.challenges.keys().copied().collect::<BTreeSet<_>>();
    starts.outcomes.retain(|_, outcome| match outcome {
        StartOutcomeRecord::Pending {
            expires_at_unix_ms, ..
        } => *expires_at_unix_ms > observed_at_unix_ms,
        StartOutcomeRecord::Challenge { token, .. } => live_challenges.contains(token),
        StartOutcomeRecord::Accepted { acceptance, .. } => {
            acceptance.lease_expires_at_unix_ms > observed_at_unix_ms
        }
        StartOutcomeRecord::Rejected {
            expires_at_unix_ms, ..
        } => *expires_at_unix_ms > observed_at_unix_ms,
    });
}

fn challenge_record_count(starts: &StartState) -> usize {
    starts
        .challenges
        .len()
        .saturating_add(starts.challenge_tombstones.len())
}

fn replay_start_outcome(
    starts: &StartState,
    scope: &StartScope,
    fingerprint: [u8; 32],
    session: &AuthorizedSession,
    correlation_id: &str,
) -> Result<Option<StartOutcome>, Status> {
    let Some(outcome) = starts.outcomes.get(scope) else {
        return Ok(None);
    };
    let observed_fingerprint = match outcome {
        StartOutcomeRecord::Pending { fingerprint, .. }
        | StartOutcomeRecord::Challenge { fingerprint, .. }
        | StartOutcomeRecord::Accepted { fingerprint, .. }
        | StartOutcomeRecord::Rejected { fingerprint, .. } => *fingerprint,
    };
    if observed_fingerprint != fingerprint {
        return Err(public_status(Code::AlreadyExists, "IDEMPOTENCY_CONFLICT"));
    }
    match outcome {
        StartOutcomeRecord::Pending { .. } => {
            Err(public_status(Code::Aborted, "START_IN_PROGRESS"))
        }
        StartOutcomeRecord::Challenge { token, .. } => {
            let record = starts
                .challenges
                .get(token)
                .ok_or_else(|| public_status(Code::Internal, "START_OUTCOME_STATE"))?;
            Ok(Some(StartOutcome::InputChallenge(challenge_wire(
                record, *token, session,
            )?)))
        }
        StartOutcomeRecord::Accepted {
            acceptance,
            semantic_request_id,
            ..
        } => Ok(Some(StartOutcome::Accepted(accepted_wire(
            session,
            acceptance,
            semantic_request_id,
            true,
        )))),
        StartOutcomeRecord::Rejected { rejection, .. } => Ok(Some(
            StartOutcome::ValidationRejection(rejection.to_wire(session, correlation_id)),
        )),
    }
}

fn challenge_wire(
    record: &ChallengeRecord,
    token: [u8; 32],
    session: &AuthorizedSession,
) -> Result<InputChallenge, Status> {
    if record.session_id != session.session_id()
        || record.session_generation != session.session_generation()
        || record.daemon_generation != session.daemon_generation()
        || record.policy_generation != session.policy_generation()
        || record.revocation_generation != session.revocation_generation()
        || record.principal_id != session.principal_id()
        || !session.permits_workspace(record.workspace_id)
    {
        return Err(public_status(Code::FailedPrecondition, "CHALLENGE_BINDING"));
    }
    let challenge = InputChallenge {
        authority: Some(authority(session)),
        semantic_request_id: record.prepared.parsed.request.semantic_request_id.clone(),
        challenge_id: record.challenge_id.clone(),
        round: record.round,
        remaining_rounds: remaining_challenge_rounds(record.round),
        issued_at_unix_ms: record.issued_at_unix_ms,
        expires_at_unix_ms: record.expires_at_unix_ms,
        maximum_answer_bytes: u32::try_from(MAX_CHALLENGE_ANSWER_BYTES).unwrap_or(u32::MAX),
        explanation_code: ChallengeExplanationCode::RequiredInputMissing as i32,
        requirements: record
            .requirements
            .iter()
            .map(requirement_to_wire)
            .collect::<Result<Vec<_>, _>>()?,
        daemon_continuation: token.to_vec(),
    };
    if challenge.encoded_len() > MAX_CHALLENGE_MESSAGE_BYTES {
        return Err(public_status(
            Code::ResourceExhausted,
            "CHALLENGE_REQUIREMENTS",
        ));
    }
    Ok(challenge)
}

const fn remaining_challenge_rounds(round: u32) -> u32 {
    MAX_CHALLENGE_ROUNDS.saturating_sub(round)
}

fn challenge_ttl_millis(session: &AuthorizedSession) -> i64 {
    let ttl_seconds = CHALLENGE_TTL
        .as_secs()
        .min(session.maximum_request_state_ttl_seconds());
    i64::try_from(Duration::from_secs(ttl_seconds).as_millis()).unwrap_or(i64::MAX)
}

fn next_challenge_round(round: u32) -> Result<u32, Status> {
    if round == 0 || round >= MAX_CHALLENGE_ROUNDS {
        Err(public_status(
            Code::InvalidArgument,
            "CHALLENGE_ROUND_LIMIT",
        ))
    } else {
        Ok(round + 1)
    }
}

async fn collect_retention_authority(
    results: &StreamedResultRegistry,
    coordinator: &QueryCoordinator,
    observed_at_unix_ms: i64,
) -> Result<(), Status> {
    results
        .collect_expired(observed_at_unix_ms)
        .await
        .map_err(result_status)?;
    coordinator
        .collect_expired(observed_at_unix_ms)
        .await
        .map_err(coordinator_status)?;
    for (query_id, object_set) in coordinator.pending_result_cleanups().await {
        let finalized_locally = results
            .finalize_released_query(&query_id)
            .await
            .or_else(|error| match error {
                StreamedResultRegistryError::UnknownPackage => Ok(false),
                other => Err(other),
            })
            .map_err(result_status)?;
        if !finalized_locally {
            results
                .cleanup_pending_object_set(&object_set)
                .await
                .map_err(result_status)?;
        }
        coordinator
            .mark_result_cleanup_complete(&query_id)
            .await
            .map_err(coordinator_status)?;
    }
    coordinator
        .collect_expired(observed_at_unix_ms)
        .await
        .map_err(coordinator_status)?;
    Ok(())
}

fn accepted_wire(
    session: &AuthorizedSession,
    acceptance: &crate::fabric::query_coordinator::QueryAcceptance,
    semantic_request_id: &str,
    idempotent_replay: bool,
) -> AcceptedQuery {
    AcceptedQuery {
        authority: Some(authority(session)),
        daemon_query_id: acceptance.query_id.clone(),
        semantic_request_id: semantic_request_id.to_owned(),
        operation_fingerprint: acceptance.operation_fingerprint.clone(),
        accepted_at_unix_ms: acceptance.accepted_at_unix_ms,
        observation_expires_at_unix_ms: acceptance.lease_expires_at_unix_ms,
        state: execution_state(acceptance.phase) as i32,
        idempotent_replay,
    }
}

#[tonic::async_trait]
impl<B: SemanticQueryBackend> CpgQueryService for ProductionQueryService<B> {
    async fn handshake(
        &self,
        request: Request<HandshakeRequest>,
    ) -> Result<Response<HandshakeResponse>, Status> {
        let budget = handshake_budget(request.get_ref())?;
        let _admission = self.admission.control(budget).await?;
        budget
            .run(async {
                let peer = peer(&request)?;
                let request = request.into_inner();
                if request.minimum_minor > RPC_MINOR
                    || request.maximum_minor < RPC_MINOR
                    || request.required_feature_bits != 0
                    || request.adapter_version.is_empty()
                {
                    return Err(public_status(Code::FailedPrecondition, "RPC_INCOMPATIBLE"));
                }
                let session = self
                    .sessions
                    .consume_grant(
                        &request.launch_grant,
                        peer,
                        &request.desired_semantic_profiles,
                        request.maximum_resource_chunk_bytes,
                        now_millis(),
                    )
                    .await
                    .map_err(session_status)?;
                Ok(Response::new(HandshakeResponse {
                    session_token: session.token().to_vec(),
                    authority: Some(authority(&session)),
                    selected_minor: RPC_MINOR,
                    selected_feature_bits: request.optional_feature_bits & 0,
                    selected_semantic_profile: session.semantic_profile().to_owned(),
                    lifecycle: lifecycle_state(self.lifecycle.observe().phase()) as i32,
                    effective_limits: Some(EffectiveLimits {
                        maximum_control_message_bytes: MAX_CONTROL_MESSAGE_BYTES as u64,
                        maximum_resource_chunk_bytes: session.maximum_resource_chunk_bytes(),
                        maximum_result_bytes: session.maximum_result_bytes(),
                        maximum_result_pages: session.maximum_result_pages(),
                        maximum_concurrent_queries: u32::try_from(
                            self.coordinator.maximum_running_queries_per_principal(),
                        )
                        .unwrap_or(u32::MAX),
                        maximum_watch_events: u32::try_from(
                            self.coordinator.maximum_events_per_query(),
                        )
                        .unwrap_or(u32::MAX),
                        maximum_challenge_fields: 16,
                        maximum_choices_per_field: 64,
                        maximum_challenge_rounds: MAX_CHALLENGE_ROUNDS,
                        maximum_reference_completion_candidates:
                            MAX_REFERENCE_COMPLETION_CANDIDATES,
                        maximum_validation_issues: MAX_VALIDATION_ISSUES,
                    }),
                    session_expires_at_unix_ms: session.expires_at_unix_ms(),
                    reference_index_revision: "codefabric.live-reference-index.v2".to_owned(),
                    reserved_control: Some(reserved_control_contract()),
                }))
            })
            .await
    }

    async fn get_status(
        &self,
        request: Request<GetStatusRequest>,
    ) -> Result<Response<GetStatusResponse>, Status> {
        let budget = request_budget(request.get_ref().context.as_ref())?;
        let _admission = self.admission.control(budget).await?;
        budget
            .run(async {
                let session = self
                    .authorize(&request, SessionOperation::Status, None)
                    .await?;
                validate_context(request.get_ref().context.as_ref(), &session)?;
                let projection = self.lifecycle.observe();
                let coordinator = self.coordinator.snapshot().await;
                let active_epoch_id = session.workspace_ids().iter().find_map(|workspace| {
                    self.workspace_slots
                        .slot(*workspace)
                        .and_then(|slot| slot.lease().ok())
                        .map(|lease| {
                            format!(
                                "epoch:{}",
                                hex(lease.workspace().selection().epoch_id().as_bytes())
                            )
                        })
                });
                let source_observations = session.workspace_ids().iter().filter_map(|workspace| {
                    let lease = self.workspace_slots.slot(*workspace)?.lease().ok()?;
                    let runtime = lease.workspace().runtime();
                    let authority = runtime.query_authority();
                    let observed = authority.source_observation()?;
                    let generation = authority.activation_pins().source_generation.get();
                    let reconciled_watermark = observed.freshness.reconciled();
                    let source_reconciled_watermark = observed.source_freshness.reconciled();
                    let requested_watermark = observed.freshness.requested();
                    Some(crate::rpc::generated::codefabric::cpgd::v2::WorkspaceSourceObservation {
                        workspace_id: runtime.public_workspace_id().ok()?,
                        selected_source_generation: generation,
                        requested_watermark,
                        reconciled_watermark,
                        freshness: i32::from(observed.state_for(generation) as u16),
                        watch_healthy: observed.watch_healthy(),
                        rescan_required: observed.rescan_required(),
                        runnable_pending: reconciled_watermark < requested_watermark,
                        source_reconciled_watermark: Some(source_reconciled_watermark),
                        source_freshness: Some(i32::from(observed.source_state_for(generation) as u16)),
                        semantic_pending: Some(authority.semantic_pending()),
                    })
                }).collect();
                let status_value = serde_json::json!({
                    "semantic_release": self.release.suite().as_str(),
                    "lifecycle": projection.phase().code(),
                    "lifecycle_sequence": projection.sequence(),
                    "active_epoch_id": active_epoch_id,
                    "running_queries": coordinator.running,
                    "queued_queries": coordinator.queued,
                    "accepted_queries": coordinator.accepted,
                    "reserved_result_bytes": coordinator.reserved_result_bytes,
                    "reserved_result_pages": coordinator.reserved_result_pages,
                });
                let canonical_public_status_json = serde_json_canonicalizer::to_vec(&status_value)
                    .map_err(|_| public_status(Code::Internal, "STATUS_ENCODING"))?;
                Ok(Response::new(GetStatusResponse {
                    authority: Some(authority(&session)),
                    lifecycle: lifecycle_state(projection.phase()) as i32,
                    lifecycle_sequence: projection.sequence(),
                    failure: projection.failure_code().map(|_| {
                        safe_error(
                            SafeErrorCode::DaemonUnavailable,
                            SafeErrorLayer::Lifecycle,
                            true,
                            "lifecycle.failed_closed",
                            "",
                        )
                    }),
                    active_epoch_id,
                    running_queries: u32::try_from(coordinator.running).unwrap_or(u32::MAX),
                    queued_queries: u32::try_from(coordinator.queued).unwrap_or(u32::MAX),
                    canonical_public_status_json,
                    source_observations,
                }))
            })
            .await
    }

    async fn get_reference(
        &self,
        request: Request<GetReferenceRequest>,
    ) -> Result<Response<GetReferenceResponse>, Status> {
        let budget = request_budget(request.get_ref().context.as_ref())?;
        let _admission = self.admission.data()?;
        budget
            .run(async {
                let session = self
                    .authorize(&request, SessionOperation::Reference, None)
                    .await?;
                validate_context(request.get_ref().context.as_ref(), &session)?;
                let workspace_id = sole_reference_workspace(session.workspace_ids())?;
                let request = request.into_inner();
                let projection = self.lifecycle.observe();
                let workspace_lease = self
                    .workspace_slots
                    .slot(workspace_id)
                    .and_then(|slot| slot.lease().ok());
                let references = if let Some(lease) = workspace_lease.as_ref() {
                    LiveReferenceProjection::from_workspace(
                        self.release.as_ref(),
                        lease.workspace().runtime().query_authority(),
                        projection.phase(),
                        projection.sequence(),
                    )?
                } else {
                    missing_reference_query_forms(projection.phase())?;
                    LiveReferenceProjection::default()
                };
                let result = match request
                    .operation
                    .ok_or_else(|| public_status(Code::InvalidArgument, "REFERENCE_OPERATION"))?
                {
                    ReferenceOperation::Read(read) => {
                        let kind = ReferenceKind::try_from(read.kind)
                            .map_err(|_| public_status(Code::InvalidArgument, "REFERENCE_KIND"))?;
                        if kind == ReferenceKind::Unspecified
                            || read.version.as_ref().is_some_and(|value| {
                                value.is_empty() || value.len() > 64 || !value.is_ascii()
                            })
                        {
                            return Err(public_status(Code::InvalidArgument, "REFERENCE_SELECTOR"));
                        }
                        let entry = references.resolve(kind, read.version.as_deref())?;
                        let selector = StreamedReferenceSelector {
                            kind: kind.as_str_name().to_owned(),
                            version: read.version.clone(),
                        };
                        let content = entry.content.clone();
                        let reference_id = format!("b3:{}", hex(blake3::hash(&content).as_bytes()));
                        let expires_at_unix_ms = now_millis()
                            .saturating_add(
                                i64::try_from(RESULT_LEASE_GRACE.as_millis()).unwrap_or(i64::MAX),
                            )
                            .min(session.expires_at_unix_ms());
                        let registration = self
                            .results
                            .publish_reference(ReferenceResourcePublication {
                                principal_id: session.principal_id(),
                                workspace_id,
                                daemon_generation: session.daemon_generation(),
                                policy_generation: session.policy_generation(),
                                revocation_generation: session.revocation_generation(),
                                selector,
                                reference_id: reference_id.clone(),
                                media_type: "application/json".to_owned(),
                                content,
                                issued_at_unix_ms: now_millis(),
                                expires_at_unix_ms,
                            })
                            .await
                            .map_err(result_status)?;
                        ReferenceResult::Reference(ReferenceDocument {
                            reference_id,
                            resource: Some(ResourceDescriptor {
                                kind: ResourceKind::Reference as i32,
                                public_handle: registration.public_handle,
                                package_id: None,
                                page_ordinal: None,
                                media_type: "application/json".to_owned(),
                                byte_length: registration.byte_length,
                                content_checksum: registration.content_checksum,
                                expires_at_unix_ms: registration.expires_at_unix_ms,
                                authority: Some(authority(&session)),
                            }),
                        })
                    }
                    ReferenceOperation::Completion(completion) => {
                        ReferenceResult::Completion(reference_completion(completion, &references)?)
                    }
                };
                drop(workspace_lease);
                Ok(Response::new(GetReferenceResponse {
                    authority: Some(authority(&session)),
                    result: Some(result),
                }))
            })
            .await
    }

    async fn validate_query(
        &self,
        request: Request<ValidateQueryRequest>,
    ) -> Result<Response<ValidateQueryResponse>, Status> {
        let budget = request_budget(request.get_ref().context.as_ref())?;
        let _admission = self.admission.data()?;
        budget
            .run(async {
                let session = self
                    .authorize(&request, SessionOperation::Validate, None)
                    .await?;
                validate_context(request.get_ref().context.as_ref(), &session)?;
                let request = request.into_inner();
                let query = request
                    .query
                    .ok_or_else(|| public_status(Code::InvalidArgument, "QUERY_SUBMISSION"))?;
                let result = self.validate_submission(&session, &query).await;
                match result {
                    Ok(prepared) => Ok(Response::new(ValidateQueryResponse {
                        authority: Some(authority(&session)),
                        preparation: Some(QueryPreparation {
                            canonical_normalized_request_json: prepared.parsed.canonical_bytes,
                            semantic_request_id: prepared.parsed.request.semantic_request_id,
                            input_requirements: prepared
                                .requirements
                                .iter()
                                .map(requirement_to_wire)
                                .collect::<Result<Vec<_>, _>>()?,
                            errors: Vec::new(),
                            warnings: Vec::new(),
                            cost_class: "bounded-programmatic".to_owned(),
                            estimated_result_bytes: query
                                .result_limits
                                .as_ref()
                                .map_or(0, |limits| limits.maximum_result_bytes),
                            estimated_result_pages: query
                                .result_limits
                                .as_ref()
                                .map_or(0, |limits| limits.maximum_result_pages),
                        }),
                    })),
                    Err(status) if status.code() == Code::InvalidArgument => {
                        Ok(Response::new(ValidateQueryResponse {
                            authority: Some(authority(&session)),
                            preparation: Some(QueryPreparation {
                                canonical_normalized_request_json: Vec::new(),
                                semantic_request_id: query.semantic_request_id.unwrap_or_default(),
                                input_requirements: Vec::new(),
                                errors: vec![validation_issue(&status)],
                                warnings: Vec::new(),
                                cost_class: "rejected".to_owned(),
                                estimated_result_bytes: 0,
                                estimated_result_pages: 0,
                            }),
                        }))
                    }
                    Err(status) => Err(status),
                }
            })
            .await
    }

    async fn start_query(
        &self,
        request: Request<StartQueryRequest>,
    ) -> Result<Response<StartQueryResponse>, Status> {
        let budget = request_budget(request.get_ref().context.as_ref())?;
        let _admission = self.admission.data()?;
        budget
            .run(async {
                if !self.lifecycle.observe().semantic_admission_open() {
                    return Err(public_status(Code::Unavailable, "LIFECYCLE_NOT_READY"));
                }
                let session = self
                    .authorize(&request, SessionOperation::Start, None)
                    .await?;
                let context = request
                    .get_ref()
                    .context
                    .clone()
                    .ok_or_else(|| public_status(Code::InvalidArgument, "REQUEST_CONTEXT"))?;
                validate_context(Some(&context), &session)?;
                let request = request.into_inner();
                let observed_at = now_millis();
                let ready = match request
                    .leg
                    .ok_or_else(|| public_status(Code::InvalidArgument, "START_LEG"))?
                {
                    StartLeg::Initial(initial) => {
                        let query = initial.query.ok_or_else(|| {
                            public_status(Code::InvalidArgument, "QUERY_SUBMISSION")
                        })?;
                        let limits = query
                            .result_limits
                            .as_ref()
                            .ok_or_else(|| public_status(Code::InvalidArgument, "RESULT_LIMITS"))?;
                        let maximum_result_bytes = limits.maximum_result_bytes;
                        let maximum_result_pages = limits.maximum_result_pages;
                        let parsed = match self
                            .validate_request(
                                &session,
                                &query.canonical_request_json,
                                &query.request_checksum,
                                &query.semantic_profile,
                                maximum_result_bytes,
                                maximum_result_pages,
                            )
                            .await
                        {
                            Ok(parsed)
                                if query.semantic_request_id.as_ref().is_none_or(|identity| {
                                    identity == &parsed.request.semantic_request_id
                                }) =>
                            {
                                parsed
                            }
                            Ok(_) => {
                                return Err(public_status(
                                    Code::InvalidArgument,
                                    "SEMANTIC_REQUEST_ID",
                                ));
                            }
                            Err(status) if status.code() == Code::InvalidArgument => {
                                return Ok(Response::new(StartQueryResponse {
                                    outcome: Some(StartOutcome::ValidationRejection(
                                        ValidationRejection {
                                            authority: Some(authority(&session)),
                                            semantic_request_id: query.semantic_request_id,
                                            issues: vec![validation_issue(&status)],
                                            error: Some(safe_error(
                                                SafeErrorCode::ValidationRejected,
                                                SafeErrorLayer::Validation,
                                                false,
                                                "",
                                                &context.correlation_id,
                                            )),
                                        },
                                    )),
                                }));
                            }
                            Err(status) => return Err(status),
                        };
                        let (scope, fingerprint) = start_scope_and_fingerprint(
                            &session,
                            &parsed,
                            &query.semantic_profile,
                            maximum_result_bytes,
                            maximum_result_pages,
                        )?;
                        if let Some(outcome) = self
                            .replay_start(
                                &session,
                                &scope,
                                fingerprint,
                                observed_at,
                                &context.correlation_id,
                            )
                            .await?
                        {
                            return Ok(Response::new(StartQueryResponse {
                                outcome: Some(outcome),
                            }));
                        }
                        let prepared =
                            match self.prepare_semantic_submission(parsed, Vec::new()).await {
                                Ok(prepared) => prepared,
                                Err(status) if status.code() == Code::InvalidArgument => {
                                    return Ok(Response::new(StartQueryResponse {
                                        outcome: Some(StartOutcome::ValidationRejection(
                                            ValidationRejection {
                                                authority: Some(authority(&session)),
                                                semantic_request_id: query.semantic_request_id,
                                                issues: vec![validation_issue(&status)],
                                                error: Some(safe_error(
                                                    SafeErrorCode::ValidationRejected,
                                                    SafeErrorLayer::Validation,
                                                    false,
                                                    "",
                                                    &context.correlation_id,
                                                )),
                                            },
                                        )),
                                    }));
                                }
                                Err(status) => return Err(status),
                            };
                        if !prepared.requirements.is_empty() {
                            let outcome = self
                                .issue_challenge(
                                    &session,
                                    prepared,
                                    scope,
                                    fingerprint,
                                    query.semantic_profile,
                                    maximum_result_bytes,
                                    maximum_result_pages,
                                    observed_at,
                                    &context.correlation_id,
                                )
                                .await?;
                            return Ok(Response::new(StartQueryResponse {
                                outcome: Some(outcome),
                            }));
                        }
                        let admitted = self
                            .backend
                            .admit_fresh_execution_request(prepared.resolved.clone().ok_or_else(
                                || public_status(Code::Internal, "RESOLVED_OPERATION"),
                            )?)
                            .await
                            .map_err(semantic_status)?;
                        self.reserve_start_acceptance(&scope, fingerprint, observed_at)
                            .await?;
                        ReadyStart {
                            prepared: admitted,
                            semantic_request_id: scope.semantic_request_id.clone(),
                            semantic_profile: query.semantic_profile,
                            maximum_result_bytes,
                            maximum_result_pages,
                            scope,
                            fingerprint,
                            pending_reserved: true,
                        }
                    }
                    StartLeg::Continuation(continuation) => {
                        match self
                            .consume_challenge(
                                &session,
                                continuation,
                                observed_at,
                                &context.correlation_id,
                            )
                            .await?
                        {
                            ChallengeContinuationOutcome::Ready(ready) => ready,
                            ChallengeContinuationOutcome::Closed(outcome) => {
                                return Ok(Response::new(StartQueryResponse {
                                    outcome: Some(outcome),
                                }));
                            }
                        }
                    }
                };
                let resolved = ready.prepared.resolved();
                let parsed = resolved.parsed();
                let semantic_request_id = parsed.request.semantic_request_id.clone();
                let workspace_id = workspace_id(&parsed.request.workspace_id)?;
                let remaining_budget = budget.remaining()?;
                let budget_millis = i64::try_from(remaining_budget.as_millis())
                    .map_err(|_| public_status(Code::InvalidArgument, "EXECUTION_BUDGET"))?;
                let acceptance_observed_at = now_millis();
                let deadline_unix_ms = acceptance_observed_at
                    .checked_add(budget_millis)
                    .ok_or_else(|| public_status(Code::InvalidArgument, "EXECUTION_BUDGET"))?;
                let lease_grace = i64::try_from(RESULT_LEASE_GRACE.as_millis()).unwrap_or(i64::MAX);
                let lease_expires_at_unix_ms = deadline_unix_ms
                    .saturating_add(lease_grace)
                    .min(session.expires_at_unix_ms());
                if lease_expires_at_unix_ms <= deadline_unix_ms {
                    return Err(public_status(Code::DeadlineExceeded, "SESSION_BUDGET"));
                }
                let operation = NormalizedQueryOperation::try_new(NormalizedQueryOperation {
                    workspace_id,
                    principal_id: session.principal_id(),
                    policy_generation: session.policy_generation(),
                    revocation_generation: session.revocation_generation(),
                    session_sharing_class: QuerySessionSharingClass::PrincipalBound,
                    idempotency_key: Arc::from(ready.semantic_request_id.as_str()),
                    canonical_request: Arc::from(resolved.canonical_operation().to_vec()),
                    semantic_profile: Arc::from(ready.semantic_profile.as_str()),
                    request_contract: Arc::from("codefabric.semantic-query-request.v2"),
                    response_contract: Arc::from("codefabric.semantic-query-response.v2"),
                    delivery_profile: Arc::from("daemon-resource"),
                    compression_profile: Arc::from("identity"),
                    freshness_policy: Arc::from(freshness_policy_name(
                        parsed.request.freshness_policy,
                    )),
                    epoch_policy: Arc::from("selected-active-epoch"),
                    deadline_unix_ms,
                    lease_expires_at_unix_ms,
                    maximum_result_bytes: ready.maximum_result_bytes,
                    maximum_result_pages: ready.maximum_result_pages,
                })
                .map_err(coordinator_status)?;
                let lifecycle_admission = self
                    .lifecycle
                    .try_semantic_admission()
                    .map_err(|_| public_status(Code::Unavailable, "LIFECYCLE_NOT_READY"))?;
                let admitted_lifecycle_sequence = lifecycle_admission.sequence();
                let outcome = match self
                    .coordinator
                    .accept(operation, acceptance_observed_at)
                    .await
                {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        if ready.pending_reserved {
                            self.clear_pending_start(&ready.scope, ready.fingerprint)
                                .await;
                        }
                        return Err(coordinator_status(error));
                    }
                };
                debug_assert_eq!(
                    self.lifecycle.observe().sequence(),
                    admitted_lifecycle_sequence,
                    "the lifecycle generation cannot close while query acceptance holds its permit"
                );
                let (acceptance, replay) = match outcome {
                    QueryAcceptanceOutcome::New(acceptance) => (acceptance, false),
                    QueryAcceptanceOutcome::Replay(acceptance) => (acceptance, true),
                };
                self.record_accepted_start(&ready, acceptance.clone(), semantic_request_id.clone())
                    .await?;
                if !replay {
                    let task = execute_accepted_query(ExecutionTask {
                        backend: Arc::clone(&self.backend),
                        coordinator: Arc::clone(&self.coordinator),
                        results: Arc::clone(&self.results),
                        query_id: acceptance.query_id.clone(),
                        prepared: ready.prepared.clone(),
                        principal_id: session.principal_id(),
                        workspace_id,
                        correlation_id: context.correlation_id.clone(),
                        deadline: budget.deadline,
                        daemon_generation: session.daemon_generation(),
                        policy_generation: session.policy_generation(),
                        revocation_generation: session.revocation_generation(),
                    });
                    if self
                        .coordinator
                        .spawn_task(&acceptance.query_id, task)
                        .await
                        .is_err()
                    {
                        let _ = self
                            .coordinator
                            .terminal(
                                &acceptance.query_id,
                                QueryTerminalState::Failed,
                                Some("TASK_ATTACHMENT_FAILED".to_owned()),
                                None,
                                now_millis(),
                            )
                            .await;
                    }
                }
                Ok(Response::new(StartQueryResponse {
                    outcome: Some(StartOutcome::Accepted(accepted_wire(
                        &session,
                        &acceptance,
                        &semantic_request_id,
                        replay,
                    ))),
                }))
            })
            .await
    }

    type WatchQueryStream = QueryEventStream;

    async fn watch_query(
        &self,
        request: Request<WatchQueryRequest>,
    ) -> Result<Response<Self::WatchQueryStream>, Status> {
        let budget = request_budget(request.get_ref().context.as_ref())?;
        let admission_permit = self.admission.data()?;
        budget
            .run(async {
                let peer = peer(&request)?;
                let session_token = session_token(&request)?;
                let session = self
                    .authorize(&request, SessionOperation::Watch, None)
                    .await?;
                validate_context(request.get_ref().context.as_ref(), &session)?;
                let request = request.into_inner();
                let workspace = self
                    .coordinator
                    .authorize_query(&request.daemon_query_id, query_session_authority(&session)?)
                    .await
                    .map_err(coordinator_status)?;
                if !session.permits_workspace(workspace) {
                    return Err(public_status(Code::PermissionDenied, "WORKSPACE_DENIED"));
                }
                let after_sequence = if let Some(cursor) = request.cursor {
                    let cursor = std::str::from_utf8(&cursor)
                        .map_err(|_| public_status(Code::InvalidArgument, "QUERY_CURSOR"))?;
                    let (query_id, after) = self
                        .coordinator
                        .verify_cursor(cursor, query_session_authority(&session)?, now_millis())
                        .await
                        .map_err(coordinator_status)?;
                    if query_id != request.daemon_query_id {
                        return Err(public_status(Code::PermissionDenied, "CURSOR_BINDING"));
                    }
                    after
                } else {
                    0
                };
                let state = WatchState {
                    sessions: Arc::clone(&self.sessions),
                    coordinator: Arc::clone(&self.coordinator),
                    results: Arc::clone(&self.results),
                    peer,
                    session_token,
                    query_id: request.daemon_query_id,
                    principal_id: session.principal_id(),
                    workspace_id: workspace,
                    daemon_generation: session.daemon_generation(),
                    policy_generation: session.policy_generation(),
                    revocation_generation: session.revocation_generation(),
                    cursor_expiry: session
                        .expires_at_unix_ms()
                        .min(budget_expiry_unix_ms(budget)?),
                    authority: authority(&session),
                    after_sequence,
                    pending: Vec::new(),
                    budget,
                    admission_permit: Some(admission_permit),
                    #[cfg(test)]
                    event_fetch_blocker: None,
                };
                let stream: QueryEventStream = Box::pin(stream::unfold(state, watch_next));
                Ok(Response::new(stream))
            })
            .await
    }

    async fn cancel_query(
        &self,
        request: Request<CancelQueryRequest>,
    ) -> Result<Response<CancelQueryResponse>, Status> {
        let budget = request_budget(request.get_ref().context.as_ref())?;
        let _admission = self.admission.control(budget).await?;
        budget
            .run(async {
                let session = self
                    .authorize(&request, SessionOperation::Cancel, None)
                    .await?;
                validate_context(request.get_ref().context.as_ref(), &session)?;
                let request = request.into_inner();
                if request.cancellation_id.is_empty() {
                    return Err(public_status(Code::InvalidArgument, "CANCELLATION_ID"));
                }
                let query_id = request.daemon_query_id;
                let workspace = match self
                    .coordinator
                    .authorize_query(&query_id, query_session_authority(&session)?)
                    .await
                {
                    Ok(workspace) => workspace,
                    Err(QueryCoordinatorError::UnknownQuery(_)) => {
                        return Ok(Response::new(CancelQueryResponse {
                            authority: Some(authority(&session)),
                            cancellation_id: request.cancellation_id,
                            acknowledgement: CancellationAcknowledgement::QueryNotFound as i32,
                            terminal: None,
                            idempotent_replay: false,
                        }));
                    }
                    Err(error) => return Err(coordinator_status(error)),
                };
                if !session.permits_workspace(workspace) {
                    return Err(public_status(Code::PermissionDenied, "WORKSPACE_DENIED"));
                }
                let observed_at = now_millis();
                let cancellation = self
                    .coordinator
                    .cancel_idempotent(&query_id, &request.cancellation_id, observed_at)
                    .await
                    .map_err(coordinator_status)?;
                self.coordinator
                    .join_query_tasks(&query_id, Duration::from_secs(2))
                    .await
                    .map_err(coordinator_status)?;
                let acknowledgement = if cancellation.idempotent_replay {
                    CancellationAcknowledgement::Replayed
                } else if matches!(cancellation.phase, QueryExecutionPhase::Terminal(_)) {
                    CancellationAcknowledgement::AlreadyTerminal
                } else {
                    CancellationAcknowledgement::Accepted
                };
                Ok(Response::new(CancelQueryResponse {
                    authority: Some(authority(&session)),
                    cancellation_id: request.cancellation_id,
                    acknowledgement: acknowledgement as i32,
                    terminal: match cancellation.phase {
                        QueryExecutionPhase::Terminal(state) => Some(TerminalObservation {
                            state: terminal_state(state) as i32,
                            observed_at_unix_ms: observed_at,
                            error: (state != QueryTerminalState::Succeeded).then(|| {
                                safe_error(
                                    terminal_safe_code(state),
                                    SafeErrorLayer::Query,
                                    false,
                                    "query.terminal",
                                    "",
                                )
                            }),
                        }),
                        _ => None,
                    },
                    idempotent_replay: cancellation.idempotent_replay,
                }))
            })
            .await
    }

    type ReadResourceStream = ResourceStream;

    async fn read_processing_remainder(
        &self,
        request: Request<
            crate::rpc::generated::codefabric::cpgd::v2::ReadProcessingRemainderRequest,
        >,
    ) -> Result<
        Response<crate::rpc::generated::codefabric::cpgd::v2::ReadProcessingRemainderResponse>,
        Status,
    > {
        use crate::rpc::generated::codefabric::cpgd::v2::ReadProcessingRemainderResponse;
        let budget = request_budget(request.get_ref().context.as_ref())?;
        let _admission = self.admission.data()?;
        budget
            .run(Box::pin(async {
                let session = self
                    .authorize(&request, SessionOperation::ReadResource, None)
                    .await?;
                validate_context(request.get_ref().context.as_ref(), &session)?;
                let value = request.get_ref();
                if value.daemon_query_id.is_empty()
                    || value.daemon_query_id.len() > 256
                    || value.query_id.is_empty()
                    || value.query_id.len() > 256
                {
                    return Err(public_status(Code::InvalidArgument, "PROCESSING_SELECTOR"));
                }
                let offset = usize::try_from(value.offset)
                    .map_err(|_| public_status(Code::OutOfRange, "PROCESSING_OFFSET"))?;
                let mut read = StreamedResourceRead {
                    principal_id: session.principal_id(),
                    workspace_ids: Arc::new(session.workspace_ids().clone()),
                    daemon_generation: session.daemon_generation(),
                    policy_generation: session.policy_generation(),
                    revocation_generation: session.revocation_generation(),
                    public_handle: String::new(),
                    selector: StreamedResourceSelector::Processing(value.query_id.clone()),
                    offset: value.offset,
                    maximum_bytes: 0,
                    observed_at_unix_ms: now_millis(),
                };
                let public_handle = self
                    .processing_resource(&value.daemon_query_id, &session, &read)
                    .await?;
                read.public_handle.clone_from(&public_handle);
                let (package_id, epoch, summary) = self
                    .results
                    .read_processing(read, budget.deadline)
                    .await
                    .map_err(result_status)?;
                // Recheck live session authority after native work and before disclosing a page.
                self.authorize(&request, SessionOperation::ReadResource, None)
                    .await?;
                let handle = summary
                    .processing
                    .next_offset
                    .map(|_| public_handle.clone());
                Ok(Response::new(ReadProcessingRemainderResponse {
                    authority: Some(authority(&session)),
                    package_id,
                    epoch_id: format!("snapshot:{}", hex(epoch.as_bytes())),
                    processing: Some(processing_page_summary(summary, offset, handle)?),
                    public_handle,
                }))
            }))
            .await
    }

    async fn read_resource(
        &self,
        request: Request<ReadResourceRequest>,
    ) -> Result<Response<Self::ReadResourceStream>, Status> {
        let budget = request_budget(request.get_ref().context.as_ref())?;
        let admission_permit = self.admission.data()?;
        budget
            .run(async {
                let peer = peer(&request)?;
                let session_bytes = session_token(&request)?;
                let observed_at_unix_ms = now_millis();
                let session = self
                    .sessions
                    .authorize(
                        &session_bytes,
                        peer,
                        SessionOperation::ReadResource,
                        None,
                        observed_at_unix_ms,
                    )
                    .await
                    .map_err(session_status)?;
                if self.retention_failed.load(Ordering::Acquire) {
                    return Err(public_status(
                        Code::Unavailable,
                        "RESULT_RECOVERY_UNAVAILABLE",
                    ));
                }
                self.schedule_retention(observed_at_unix_ms).await?;
                validate_context(request.get_ref().context.as_ref(), &session)?;
                let request = request.into_inner();
                if request.public_handle.is_empty() || request.public_handle.len() > 256 {
                    return Err(public_status(Code::InvalidArgument, "PUBLIC_HANDLE"));
                }
                let selector = match request
                    .selector
                    .and_then(|selector| selector.selector)
                    .ok_or_else(|| public_status(Code::InvalidArgument, "RESOURCE_SELECTOR"))?
                {
                    WireResourceSelector::Manifest(_) => StreamedResourceSelector::Manifest,
                    WireResourceSelector::Page(page) => {
                        StreamedResourceSelector::Page(page.page_ordinal)
                    }
                    WireResourceSelector::Reference(reference) => {
                        let kind = ReferenceKind::try_from(reference.kind)
                            .map_err(|_| public_status(Code::InvalidArgument, "REFERENCE_KIND"))?;
                        if kind == ReferenceKind::Unspecified
                            || reference.version.as_ref().is_some_and(|value| {
                                value.is_empty() || value.len() > 64 || !value.is_ascii()
                            })
                        {
                            return Err(public_status(Code::InvalidArgument, "RESOURCE_SELECTOR"));
                        }
                        StreamedResourceSelector::Reference(StreamedReferenceSelector {
                            kind: kind.as_str_name().to_owned(),
                            version: reference.version,
                        })
                    }
                };
                let maximum_bytes = usize::try_from(request.maximum_bytes)
                    .map_err(|_| public_status(Code::InvalidArgument, "RESOURCE_BOUND"))?
                    .min(
                        usize::try_from(session.maximum_resource_chunk_bytes())
                            .unwrap_or(usize::MAX),
                    )
                    .min(MAX_PAYLOAD_CHUNK_BYTES);
                if maximum_bytes == 0 {
                    return Err(public_status(Code::InvalidArgument, "RESOURCE_BOUND"));
                }
                let state = ReadState {
                    sessions: Arc::clone(&self.sessions),
                    results: Arc::clone(&self.results),
                    peer,
                    session_token: session_bytes,
                    principal_id: session.principal_id(),
                    workspace_ids: Arc::new(session.workspace_ids().clone()),
                    daemon_generation: session.daemon_generation(),
                    authority: authority(&session),
                    public_handle: request.public_handle,
                    selector,
                    offset: request.offset,
                    maximum_bytes,
                    done: false,
                    budget,
                    admission_permit: Some(admission_permit),
                    #[cfg(test)]
                    resource_read_blocker: None,
                };
                let stream: ResourceStream = Box::pin(stream::unfold(state, read_next));
                Ok(Response::new(stream))
            })
            .await
    }

    async fn release_resource(
        &self,
        request: Request<ReleaseResourceRequest>,
    ) -> Result<Response<ReleaseResourceResponse>, Status> {
        let budget = request_budget(request.get_ref().context.as_ref())?;
        let _admission = self.admission.control(budget).await?;
        budget
            .run(async {
                let session = self
                    .authorize(&request, SessionOperation::ReleaseResource, None)
                    .await?;
                validate_context(request.get_ref().context.as_ref(), &session)?;
                let request = request.into_inner();
                if request.public_handle.is_empty() {
                    return Err(public_status(Code::InvalidArgument, "PUBLIC_HANDLE"));
                }
                let (outcome, query_id, idempotent_replay) = self
                    .results
                    .release(
                        session.principal_id(),
                        session.workspace_ids(),
                        session.daemon_generation(),
                        session.policy_generation(),
                        session.revocation_generation(),
                        &request.public_handle,
                        &request.release_id,
                        now_millis(),
                    )
                    .await
                    .map_err(result_status)?;
                if let Some(query_id) = query_id {
                    self.coordinator
                        .authorize_query(&query_id, query_session_authority(&session)?)
                        .await
                        .map_err(coordinator_status)?;
                    self.coordinator
                        .release_result(&query_id)
                        .await
                        .map_err(coordinator_status)?;
                    self.results
                        .finalize_released_query(&query_id)
                        .await
                        .map_err(result_status)?;
                    self.coordinator
                        .mark_result_cleanup_complete(&query_id)
                        .await
                        .map_err(coordinator_status)?;
                }
                Ok(Response::new(ReleaseResourceResponse {
                    authority: Some(authority(&session)),
                    release_id: request.release_id,
                    state: match outcome {
                        StreamedReleaseOutcome::Released => ReleaseState::Released,
                        StreamedReleaseOutcome::AlreadyReleased => ReleaseState::AlreadyReleased,
                    } as i32,
                    idempotent_replay,
                }))
            })
            .await
    }
}

struct ExecutionTask<B: SemanticQueryBackend> {
    backend: Arc<B>,
    coordinator: Arc<QueryCoordinator>,
    results: Arc<StreamedResultRegistry>,
    query_id: String,
    prepared: PreparedSemanticExecution<B::ExecutionAuthority>,
    principal_id: PrincipalId,
    workspace_id: WorkspaceId,
    correlation_id: String,
    deadline: Instant,
    daemon_generation: u64,
    policy_generation: u64,
    revocation_generation: u64,
}

#[derive(Debug)]
struct CoordinatorPublicationIntentRecorder {
    coordinator: Arc<QueryCoordinator>,
    query_id: String,
}

#[async_trait::async_trait]
impl ResultPublicationIntentRecorder for CoordinatorPublicationIntentRecorder {
    async fn record_publication_intent(
        &self,
        intent: PendingResultObjectSet,
    ) -> Result<(), ResultPublicationIntentError> {
        self.coordinator
            .append_event(
                &self.query_id,
                QueryControlEventPayload::PublicationPending { object_set: intent },
                now_millis(),
            )
            .await
            .map(|_| ())
            .map_err(|_| ResultPublicationIntentError)
    }
}

async fn execute_accepted_query<B: SemanticQueryBackend>(task: ExecutionTask<B>) {
    let ExecutionTask {
        backend,
        coordinator,
        results,
        query_id,
        prepared,
        principal_id,
        workspace_id,
        correlation_id,
        deadline,
        daemon_generation,
        policy_generation,
        revocation_generation,
    } = task;
    let parsed = prepared.resolved().parsed();
    let snapshot = prepared.snapshot().clone();
    let execution_deadline = tokio::time::Instant::from_std(deadline);
    let Ok(permit) = await_running_until(&coordinator, &query_id, deadline).await else {
        return;
    };
    let observed_at = now_millis();
    let _ = coordinator
        .append_event(
            &query_id,
            QueryControlEventPayload::SnapshotPinned {
                epoch_id: snapshot.snapshot_id.clone(),
                source_generation: snapshot.source_generation,
                activation_head: snapshot.overlay_generation,
                lifecycle_watermark: snapshot.source_generation,
                freshness: Some(snapshot.freshness_state),
                analysis_context_set_id: Some(snapshot.analysis_context_set_id.clone()),
            },
            observed_at,
        )
        .await;
    let _ = coordinator
        .append_event(
            &query_id,
            QueryControlEventPayload::Progress {
                stage: "executing".to_owned(),
                completed: 0,
                total: None,
            },
            now_millis(),
        )
        .await;
    let Some(cancellation) = coordinator.cancellation_probe(&query_id, 1).await else {
        permit.complete();
        return;
    };
    let query_pin = QueryExecutionPin::from_bytes(identity32(
        b"codefabric.query-execution.v2",
        &[
            query_id.as_bytes(),
            principal_id.as_bytes(),
            workspace_id.as_bytes(),
        ],
    ));
    let Ok(lease_id) = random16().map(LeaseId::from_bytes) else {
        let _ = coordinator
            .terminal(
                &query_id,
                QueryTerminalState::Failed,
                Some("ENTROPY_UNAVAILABLE".to_owned()),
                None,
                now_millis(),
            )
            .await;
        permit.complete();
        return;
    };
    let Ok(lease_token) =
        random32().and_then(|bytes| OpaqueResultLeaseToken::try_from_bytes(bytes).map_err(|_| ()))
    else {
        let _ = coordinator
            .terminal(
                &query_id,
                QueryTerminalState::Failed,
                Some("ENTROPY_UNAVAILABLE".to_owned()),
                None,
                now_millis(),
            )
            .await;
        permit.complete();
        return;
    };
    let execution = QueryExecutionContext {
        execution_id: query_id.clone(),
        semantic_request_id: parsed.request.semantic_request_id.clone(),
        request_correlation_id: correlation_id,
    };
    let artifacts = QueryExecutionArtifactAccumulator::new(execution.clone());
    let context = SemanticBackendExecutionContext::new(
        execution,
        hex(principal_id.as_bytes()),
        parsed.request.workspace_id.clone(),
        PublishedResultOwner::new(workspace_id, principal_id),
        query_pin,
        lease_id,
        lease_token,
        Arc::new(CoordinatorPublicationIntentRecorder {
            coordinator: Arc::clone(&coordinator),
            query_id: query_id.clone(),
        }),
        deadline,
    );
    let execution = backend.execute(
        prepared,
        snapshot.freshness_state,
        cancellation,
        context,
        artifacts,
    );
    let outcome = if let Ok(outcome) = tokio::time::timeout_at(execution_deadline, execution).await
    {
        outcome
    } else {
        close_query_at_deadline(&coordinator, &query_id).await;
        permit.complete();
        return;
    };
    match outcome {
        SemanticBackendOutcome::PublishedArrow(success) => {
            let (publication, _snapshot, _evidence) = success.into_parts();
            match results
                .publish(
                    &query_id,
                    daemon_generation,
                    policy_generation,
                    revocation_generation,
                    publication,
                )
                .await
            {
                Ok(registration) => {
                    let result_event = coordinator
                        .append_event(
                            &query_id,
                            QueryControlEventPayload::ResultReady {
                                package_id: registration.package_id.clone(),
                                manifest_resource_id: registration.manifest_resource_id.clone(),
                                manifest_checksum: registration
                                    .retained_locator
                                    .expected_manifest_checksum
                                    .clone(),
                                total_rows: registration.total_rows,
                                total_pages: registration.total_pages,
                                total_bytes: registration.total_bytes,
                                retained_locator: registration.retained_locator.clone(),
                            },
                            now_millis(),
                        )
                        .await;
                    if result_event.is_ok() {
                        let _ = coordinator
                            .terminal(
                                &query_id,
                                QueryTerminalState::Succeeded,
                                None,
                                Some((registration.total_bytes, registration.total_pages)),
                                now_millis(),
                            )
                            .await;
                    } else {
                        let _ = results.discard_unjournaled_query(&query_id).await;
                        let _ = coordinator
                            .terminal(
                                &query_id,
                                QueryTerminalState::Failed,
                                Some("RESULT_REGISTRATION_FAILED".to_owned()),
                                None,
                                now_millis(),
                            )
                            .await;
                    }
                }
                Err(_) => {
                    let _ = coordinator
                        .terminal(
                            &query_id,
                            QueryTerminalState::Failed,
                            Some("RESULT_REGISTRATION_FAILED".to_owned()),
                            None,
                            now_millis(),
                        )
                        .await;
                }
            }
        }
        SemanticBackendOutcome::Failed { error, .. } => {
            tracing::warn!(query_id, error = %error, "semantic query execution failed");
            let public_code = match &error {
                SemanticQueryError::Phase {
                    code: code @ ("RESOURCE_CAPACITY" | "QUERY_HARD_LIMIT_EXCEEDED"),
                    ..
                } => *code,
                _ => "QUERY_EXECUTION_FAILED",
            };
            let _ = coordinator
                .terminal(
                    &query_id,
                    QueryTerminalState::Failed,
                    Some(public_code.to_owned()),
                    None,
                    now_millis(),
                )
                .await;
        }
        SemanticBackendOutcome::Cancelled { .. } => {
            let _ = coordinator
                .terminal(
                    &query_id,
                    QueryTerminalState::Cancelled,
                    Some("CANCELLED".to_owned()),
                    None,
                    now_millis(),
                )
                .await;
        }
    }
    permit.complete();
}

async fn await_running_until(
    coordinator: &QueryCoordinator,
    query_id: &str,
    deadline: Instant,
) -> Result<QueryExecutionPermit, QueryCoordinatorError> {
    if let Ok(outcome) = tokio::time::timeout_at(
        tokio::time::Instant::from_std(deadline),
        coordinator.await_running(query_id),
    )
    .await
    {
        outcome
    } else {
        close_query_at_deadline(coordinator, query_id).await;
        Err(QueryCoordinatorError::DeadlineElapsed)
    }
}

async fn close_query_at_deadline(coordinator: &QueryCoordinator, query_id: &str) {
    let _ = coordinator
        .terminal(
            query_id,
            QueryTerminalState::Failed,
            Some("DEADLINE_EXCEEDED".to_owned()),
            None,
            now_millis(),
        )
        .await;
}

struct WatchState {
    sessions: Arc<LaunchGrantAuthority>,
    coordinator: Arc<QueryCoordinator>,
    results: Arc<StreamedResultRegistry>,
    peer: VerifiedPeerIdentity,
    session_token: Vec<u8>,
    query_id: String,
    principal_id: PrincipalId,
    workspace_id: WorkspaceId,
    daemon_generation: u64,
    policy_generation: u64,
    revocation_generation: u64,
    cursor_expiry: i64,
    authority: AuthorityGeneration,
    after_sequence: u64,
    pending: Vec<QueryControlEvent>,
    budget: RpcBudget,
    admission_permit: Option<OwnedSemaphorePermit>,
    #[cfg(test)]
    event_fetch_blocker: Option<tokio::sync::oneshot::Receiver<()>>,
}

async fn watch_next(mut state: WatchState) -> Option<(Result<QueryEvent, Status>, WatchState)> {
    loop {
        if let Err(status) = state.budget.remaining() {
            state.admission_permit.take();
            return Some((Err(status), state));
        }
        let sessions = Arc::clone(&state.sessions);
        let session_token = state.session_token.clone();
        let peer = state.peer;
        let session =
            match bounded_stream_step(state.budget, &mut state.admission_permit, async move {
                sessions
                    .authorize(
                        &session_token,
                        peer,
                        SessionOperation::Watch,
                        None,
                        now_millis(),
                    )
                    .await
                    .map_err(session_status)
            })
            .await
            {
                Ok(session) => session,
                Err(status) => return Some((Err(status), state)),
            };
        if session.principal_id() != state.principal_id
            || !session.permits_workspace(state.workspace_id)
            || session.daemon_generation() != state.daemon_generation
            || session.policy_generation() != state.policy_generation
            || session.revocation_generation() != state.revocation_generation
        {
            state.admission_permit.take();
            return Some((
                Err(public_status(Code::PermissionDenied, "SESSION_BINDING")),
                state,
            ));
        }
        if let Some(event) = state.pending.pop() {
            state.after_sequence = event.sequence;
            let coordinator = Arc::clone(&state.coordinator);
            let query_id = state.query_id.clone();
            let cursor =
                match bounded_stream_step(state.budget, &mut state.admission_permit, async move {
                    coordinator
                        .mint_cursor(&query_id, event.sequence, state.cursor_expiry)
                        .await
                        .map_err(coordinator_status)
                })
                .await
                {
                    Ok(cursor) => cursor,
                    Err(status) => return Some((Err(status), state)),
                };
            let coordinator = Arc::clone(&state.coordinator);
            let results = Arc::clone(&state.results);
            let query_id = state.query_id.clone();
            let principal_id = state.principal_id;
            let workspace_id = state.workspace_id;
            let daemon_generation = state.daemon_generation;
            let policy_generation = state.policy_generation;
            let revocation_generation = state.revocation_generation;
            let authority = state.authority.clone();
            let wire = bounded_stream_step(
                state.budget,
                &mut state.admission_permit,
                Box::pin(async move {
                    event_to_wire(
                        &coordinator,
                        &results,
                        &query_id,
                        principal_id,
                        workspace_id,
                        daemon_generation,
                        policy_generation,
                        revocation_generation,
                        event,
                        cursor,
                        authority,
                    )
                    .await
                }),
            )
            .await;
            return Some((wire, state));
        }
        let coordinator = Arc::clone(&state.coordinator);
        let query_id = state.query_id.clone();
        let after_sequence = state.after_sequence;
        #[cfg(test)]
        let event_fetch_blocker = state.event_fetch_blocker.take();
        match bounded_stream_step(state.budget, &mut state.admission_permit, async move {
            #[cfg(test)]
            if let Some(blocker) = event_fetch_blocker {
                let _ = blocker.await;
            }
            coordinator
                .events_after(&query_id, after_sequence)
                .await
                .map_err(coordinator_status)
        })
        .await
        {
            Ok(events) if !events.is_empty() => {
                state.pending = events.into_iter().rev().collect();
            }
            Ok(_) => {
                let coordinator = Arc::clone(&state.coordinator);
                let query_id = state.query_id.clone();
                match bounded_stream_step(state.budget, &mut state.admission_permit, async move {
                    coordinator
                        .phase(&query_id)
                        .await
                        .map_err(coordinator_status)
                })
                .await
                {
                    Ok(QueryExecutionPhase::Terminal(_)) => return None,
                    Ok(_) => {
                        if let Err(status) =
                            bounded_stream_step(state.budget, &mut state.admission_permit, async {
                                tokio::time::sleep(Duration::from_millis(10)).await;
                                Ok(())
                            })
                            .await
                        {
                            return Some((Err(status), state));
                        }
                    }
                    Err(status) => return Some((Err(status), state)),
                }
            }
            Err(status) => return Some((Err(status), state)),
        }
    }
}

async fn bounded_stream_step<T, F>(
    budget: RpcBudget,
    admission_permit: &mut Option<OwnedSemaphorePermit>,
    future: F,
) -> Result<T, Status>
where
    F: Future<Output = Result<T, Status>>,
{
    match budget.run(future).await {
        Ok(value) => Ok(value),
        Err(status) => {
            admission_permit.take();
            Err(status)
        }
    }
}

async fn event_to_wire(
    coordinator: &QueryCoordinator,
    results: &StreamedResultRegistry,
    query_id: &str,
    principal_id: PrincipalId,
    workspace_id: WorkspaceId,
    daemon_generation: u64,
    policy_generation: u64,
    revocation_generation: u64,
    event: QueryControlEvent,
    cursor: String,
    authority: AuthorityGeneration,
) -> Result<QueryEvent, Status> {
    let header = || QueryEventHeader {
        authority: Some(authority.clone()),
        daemon_query_id: query_id.to_owned(),
        sequence: event.sequence,
        emitted_at_unix_ms: event.emitted_at_unix_ms,
        cursor: cursor.as_bytes().to_vec(),
    };
    let event = match event.payload {
        QueryControlEventPayload::SnapshotPinned {
            epoch_id,
            source_generation,
            activation_head,
            lifecycle_watermark,
            freshness,
            analysis_context_set_id,
        } => Event::SnapshotPinned(SnapshotPinnedEvent {
            header: Some(header()),
            epoch_id,
            source_generation,
            activation_head,
            lifecycle_watermark,
            freshness: freshness.map(|state| i32::from(state as u16)),
            analysis_context_set_id,
        }),
        QueryControlEventPayload::Progress {
            stage,
            completed,
            total,
        } => {
            if stage != "executing" {
                return Err(public_status(Code::DataLoss, "PRIVATE_EVENT_EXPOSED"));
            }
            Event::Progress(ProgressEvent {
                header: Some(header()),
                stage: ProgressStage::Executing as i32,
                completed,
                total,
            })
        }
        QueryControlEventPayload::PublicationPending { .. } => {
            return Err(public_status(Code::DataLoss, "PRIVATE_EVENT_EXPOSED"));
        }
        QueryControlEventPayload::ResultReady {
            package_id,
            manifest_resource_id,
            manifest_checksum: _,
            total_rows,
            total_pages,
            total_bytes,
            retained_locator,
        } => {
            let observed_at_unix_ms = now_millis();
            let registration = match results
                .registration_for_query(
                    query_id,
                    principal_id,
                    workspace_id,
                    daemon_generation,
                    policy_generation,
                    revocation_generation,
                    observed_at_unix_ms,
                )
                .await
            {
                Ok(registration) => registration,
                Err(StreamedResultRegistryError::UnknownPackage) => {
                    let retained_workspace = coordinator
                        .authorize_retained_result_reissue(
                            query_id,
                            query_session_authority_values(
                                principal_id,
                                policy_generation,
                                revocation_generation,
                            )?,
                        )
                        .await
                        .map_err(coordinator_status)?;
                    if retained_workspace != workspace_id {
                        return Err(public_status(Code::PermissionDenied, "WORKSPACE_DENIED"));
                    }
                    results
                        .reissue_retained(
                            query_id,
                            principal_id,
                            workspace_id,
                            daemon_generation,
                            policy_generation,
                            revocation_generation,
                            &retained_locator,
                            observed_at_unix_ms,
                        )
                        .await
                        .map_err(result_status)?
                }
                Err(error) => return Err(result_status(error)),
            };
            if registration.package_id != package_id
                || registration.manifest_resource_id != manifest_resource_id
                || registration.retained_locator != retained_locator
            {
                return Err(public_status(Code::DataLoss, "RESULT_EVENT_BINDING"));
            }
            Event::ResultReady(ResultReadyEvent {
                processing: registration
                    .processing
                    .into_iter()
                    .map(|value| {
                        let handle = registration
                            .processing_handles
                            .get(&value.query_id)
                            .cloned();
                        processing_page_summary(value, 0, handle)
                    })
                    .collect::<Result<_, _>>()?,
                header: Some(header()),
                package_id: registration.package_id.clone(),
                manifest: Some(ResourceDescriptor {
                    kind: ResourceKind::ResultManifest as i32,
                    public_handle: registration.manifest.public_handle,
                    package_id: Some(registration.package_id.clone()),
                    page_ordinal: None,
                    media_type: registration.manifest.media_type,
                    byte_length: registration.manifest.byte_length,
                    content_checksum: registration.manifest.content_checksum,
                    expires_at_unix_ms: registration.manifest.expires_at_unix_ms,
                    authority: Some(authority.clone()),
                }),
                total_rows,
                total_pages,
                total_bytes,
                pages: registration
                    .pages
                    .into_iter()
                    .map(|page| ResourceDescriptor {
                        kind: ResourceKind::ResultPage as i32,
                        public_handle: page.public_handle,
                        package_id: Some(registration.package_id.clone()),
                        page_ordinal: page.page_ordinal,
                        media_type: page.media_type,
                        byte_length: page.byte_length,
                        content_checksum: page.content_checksum,
                        expires_at_unix_ms: page.expires_at_unix_ms,
                        authority: Some(authority.clone()),
                    })
                    .collect(),
            })
        }
        QueryControlEventPayload::Terminal { state, public_code } => {
            Event::Terminal(TerminalEvent {
                header: Some(header()),
                state: terminal_state(state) as i32,
                error: public_code.map(|public_code| {
                    let capacity = public_code == "RESOURCE_CAPACITY";
                    safe_error(
                        if capacity {
                            SafeErrorCode::CapacityUnavailable
                        } else if public_code == "QUERY_HARD_LIMIT_EXCEEDED" {
                            SafeErrorCode::QueryHardLimitExceeded
                        } else {
                            terminal_safe_code(state)
                        },
                        SafeErrorLayer::Query,
                        capacity,
                        "query.terminal",
                        "",
                    )
                }),
            })
        }
    };
    Ok(QueryEvent { event: Some(event) })
}

struct ReadState {
    sessions: Arc<LaunchGrantAuthority>,
    results: Arc<StreamedResultRegistry>,
    peer: VerifiedPeerIdentity,
    session_token: Vec<u8>,
    principal_id: PrincipalId,
    workspace_ids: Arc<BTreeSet<WorkspaceId>>,
    daemon_generation: u64,
    authority: AuthorityGeneration,
    public_handle: String,
    selector: StreamedResourceSelector,
    offset: u64,
    maximum_bytes: usize,
    done: bool,
    budget: RpcBudget,
    admission_permit: Option<OwnedSemaphorePermit>,
    #[cfg(test)]
    resource_read_blocker: Option<tokio::sync::oneshot::Receiver<()>>,
}

async fn read_next(mut state: ReadState) -> Option<(Result<ResourceChunk, Status>, ReadState)> {
    if state.done {
        return None;
    }
    if let Err(status) = state.budget.remaining() {
        state.admission_permit.take();
        return Some((Err(status), state));
    }
    let sessions = Arc::clone(&state.sessions);
    let session_token = state.session_token.clone();
    let peer = state.peer;
    let session = match bounded_stream_step(state.budget, &mut state.admission_permit, async move {
        sessions
            .authorize(
                &session_token,
                peer,
                SessionOperation::ReadResource,
                None,
                now_millis(),
            )
            .await
            .map_err(session_status)
    })
    .await
    {
        Ok(session) => session,
        Err(status) => return Some((Err(status), state)),
    };
    if session.principal_id() != state.principal_id {
        state.admission_permit.take();
        return Some((
            Err(public_status(Code::PermissionDenied, "SESSION_BINDING")),
            state,
        ));
    }
    let results = Arc::clone(&state.results);
    let read = StreamedResourceRead {
        principal_id: state.principal_id,
        workspace_ids: Arc::clone(&state.workspace_ids),
        daemon_generation: state.daemon_generation,
        policy_generation: state.authority.policy_generation,
        revocation_generation: state.authority.revocation_generation,
        public_handle: state.public_handle.clone(),
        selector: state.selector.clone(),
        offset: state.offset,
        maximum_bytes: state.maximum_bytes,
        observed_at_unix_ms: now_millis(),
    };
    #[cfg(test)]
    let resource_read_blocker = state.resource_read_blocker.take();
    let chunk = bounded_stream_step(state.budget, &mut state.admission_permit, async move {
        #[cfg(test)]
        if let Some(blocker) = resource_read_blocker {
            let _ = blocker.await;
        }
        results.read(read).await.map_err(result_status)
    })
    .await;
    match chunk {
        Ok(chunk) => {
            state.offset = chunk.next_offset;
            state.done = chunk.complete;
            Some((
                Ok(ResourceChunk {
                    authority: Some(state.authority.clone()),
                    public_handle: chunk.public_handle,
                    offset: chunk.offset,
                    content: chunk.bytes.to_vec(),
                    content_checksum: chunk.content_checksum,
                    end_of_resource: chunk.complete,
                }),
                state,
            ))
        }
        Err(status) => Some((Err(status), state)),
    }
}

fn peer<T>(request: &Request<T>) -> Result<VerifiedPeerIdentity, Status> {
    request
        .extensions()
        .get::<VerifiedPeerIdentity>()
        .copied()
        .ok_or_else(|| public_status(Code::Unauthenticated, "PEER_IDENTITY"))
}

fn session_token<T>(request: &Request<T>) -> Result<Vec<u8>, Status> {
    request
        .metadata()
        .get_bin(SESSION_METADATA_KEY)
        .ok_or_else(|| public_status(Code::Unauthenticated, "SESSION_MISSING"))?
        .to_bytes()
        .map(|bytes| bytes.to_vec())
        .map_err(|_| public_status(Code::Unauthenticated, "SESSION_INVALID"))
}

fn workspace_id(value: &str) -> Result<WorkspaceId, Status> {
    decode_public_id(IdentityDomain::Workspace, None, value)
        .map(WorkspaceId::from_bytes)
        .map_err(|_| public_status(Code::InvalidArgument, "WORKSPACE_ID"))
}

fn execution_budget(value: Option<prost_types::Duration>) -> Result<Duration, Status> {
    let value = value.ok_or_else(|| public_status(Code::InvalidArgument, "EXECUTION_BUDGET"))?;
    if value.seconds < 0 || !(0..1_000_000_000).contains(&value.nanos) {
        return Err(public_status(Code::InvalidArgument, "EXECUTION_BUDGET"));
    }
    let seconds = u64::try_from(value.seconds)
        .map_err(|_| public_status(Code::InvalidArgument, "EXECUTION_BUDGET"))?;
    let duration = Duration::new(seconds, value.nanos as u32);
    if duration.is_zero() || duration > MAX_EXECUTION_BUDGET {
        return Err(public_status(Code::InvalidArgument, "EXECUTION_BUDGET"));
    }
    Ok(duration)
}

fn request_budget(context: Option<&RequestContext>) -> Result<RpcBudget, Status> {
    let context = context.ok_or_else(|| public_status(Code::InvalidArgument, "REQUEST_CONTEXT"))?;
    RpcBudget::from_duration(execution_budget(context.remaining_budget)?)
}

fn budget_expiry_unix_ms(budget: RpcBudget) -> Result<i64, Status> {
    let remaining_millis = i64::try_from(budget.remaining()?.as_millis())
        .map_err(|_| public_status(Code::InvalidArgument, "EXECUTION_BUDGET"))?;
    Ok(now_millis().saturating_add(remaining_millis))
}

fn handshake_budget(request: &HandshakeRequest) -> Result<RpcBudget, Status> {
    if !safe_key(&request.correlation_id, 128) {
        return Err(public_status(Code::InvalidArgument, "REQUEST_CONTEXT"));
    }
    RpcBudget::from_duration(execution_budget(request.remaining_budget)?)
}

fn validate_context(
    context: Option<&RequestContext>,
    _session: &AuthorizedSession,
) -> Result<Duration, Status> {
    let context = context.ok_or_else(|| public_status(Code::InvalidArgument, "REQUEST_CONTEXT"))?;
    if !safe_key(&context.correlation_id, 128) {
        return Err(public_status(Code::PermissionDenied, "REQUEST_CONTEXT"));
    }
    execution_budget(context.remaining_budget)
}

fn authority(session: &AuthorizedSession) -> AuthorityGeneration {
    AuthorityGeneration {
        session_id: session.session_id().to_owned(),
        session_generation: session.session_generation(),
        daemon_generation: session.daemon_generation(),
        supervisor_generation: session.supervisor_generation(),
        policy_generation: session.policy_generation(),
        revocation_generation: session.revocation_generation(),
    }
}

fn query_session_authority(session: &AuthorizedSession) -> Result<QuerySessionAuthority, Status> {
    query_session_authority_values(
        session.principal_id(),
        session.policy_generation(),
        session.revocation_generation(),
    )
}

fn query_session_authority_values(
    principal_id: PrincipalId,
    policy_generation: u64,
    revocation_generation: u64,
) -> Result<QuerySessionAuthority, Status> {
    QuerySessionAuthority::try_new(
        principal_id,
        policy_generation,
        revocation_generation,
        QuerySessionSharingClass::PrincipalBound,
    )
    .map_err(coordinator_status)
}

fn reserved_control_contract() -> ReservedControlContract {
    ReservedControlContract {
        reserved_capacity: 1,
        operations: [
            ReservedControlOperation::Handshake,
            ReservedControlOperation::GetStatus,
            ReservedControlOperation::CancelQuery,
            ReservedControlOperation::ReleaseResource,
        ]
        .into_iter()
        .map(|operation| operation as i32)
        .collect(),
    }
}

fn freshness_policy_name(value: FreshnessPolicy) -> &'static str {
    match value {
        FreshnessPolicy::CurrentRequired => "current_required",
        FreshnessPolicy::WaitForCurrent => "wait_for_current",
        FreshnessPolicy::BestAvailableSnapshot => "best_available_snapshot",
        FreshnessPolicy::AwaitLatest => "await_latest",
        FreshnessPolicy::RequireCurrentForTargets => "require_current_for_targets",
        FreshnessPolicy::RequireSourceCurrent => "require_source_current",
        FreshnessPolicy::RequireSemanticCurrent => "require_semantic_current",
    }
}

fn validate_requirements(requirements: &[SemanticInputRequirement]) -> Result<(), Status> {
    if requirements.len() > MAX_CHALLENGE_FIELDS {
        return Err(public_status(
            Code::ResourceExhausted,
            "CHALLENGE_REQUIREMENTS",
        ));
    }
    let mut field_ids = BTreeSet::new();
    for requirement in requirements {
        if !safe_key(&requirement.semantic_field_id, 128)
            || !safe_key(&requirement.presentation_key, 128)
            || requirement
                .description_key
                .as_ref()
                .is_some_and(|value| !safe_key(value, 128))
            || !field_ids.insert(requirement.semantic_field_id.as_str())
            || requirement.authorized_choices.len() > MAX_CHALLENGE_CHOICES
        {
            return Err(public_status(
                Code::InvalidArgument,
                "CHALLENGE_REQUIREMENT",
            ));
        }
        let constraint_matches = match (&requirement.input_kind, &requirement.constraints) {
            (
                SemanticInputKind::String,
                Some(SemanticInputConstraints::String {
                    minimum_length,
                    maximum_length,
                    ..
                }),
            ) => {
                valid_optional_bounds(*minimum_length, *maximum_length)
                    && maximum_length.is_none_or(|maximum| {
                        usize::try_from(maximum)
                            .is_ok_and(|maximum| maximum <= MAX_CHALLENGE_STRING_BYTES)
                    })
            }
            (
                SemanticInputKind::Integer,
                Some(SemanticInputConstraints::Integer { minimum, maximum }),
            ) => valid_optional_bounds(*minimum, *maximum),
            (SemanticInputKind::Boolean, None) => true,
            (
                SemanticInputKind::Enum,
                Some(SemanticInputConstraints::Enum {
                    minimum_selections,
                    maximum_selections,
                }),
            ) => *minimum_selections <= *maximum_selections && *maximum_selections == 1,
            (
                kind,
                Some(SemanticInputConstraints::Collection {
                    item_kind,
                    minimum_items,
                    maximum_items,
                    ..
                }),
            ) => {
                *minimum_items <= *maximum_items
                    && *maximum_items <= MAX_CHALLENGE_COLLECTION_ITEMS
                    && collection_kind_matches(*kind, *item_kind)
            }
            _ => false,
        };
        let choices_required = matches!(
            requirement.input_kind,
            SemanticInputKind::Enum | SemanticInputKind::EnumCollection
        );
        if !constraint_matches || choices_required == requirement.authorized_choices.is_empty() {
            return Err(public_status(
                Code::InvalidArgument,
                "CHALLENGE_REQUIREMENT",
            ));
        }
        let mut choice_ids = BTreeSet::new();
        for choice in &requirement.authorized_choices {
            if !safe_key(&choice.choice_id, 128)
                || !safe_key(&choice.presentation_key, 128)
                || !choice_ids.insert(choice.choice_id.as_str())
                || matches!(&choice.value, SemanticInputValue::String(value) if value.len() > MAX_CHALLENGE_STRING_BYTES)
                || !matches!(
                    choice.value,
                    SemanticInputValue::String(_)
                        | SemanticInputValue::Integer(_)
                        | SemanticInputValue::Boolean(_)
                )
            {
                return Err(public_status(
                    Code::InvalidArgument,
                    "CHALLENGE_REQUIREMENT",
                ));
            }
        }
    }
    let encoded_bytes = requirements
        .iter()
        .try_fold(0_usize, |total, requirement| {
            let wire = requirement_to_wire(requirement)?;
            total
                .checked_add(wire.encoded_len())
                .ok_or_else(|| public_status(Code::ResourceExhausted, "CHALLENGE_REQUIREMENTS"))
        })?;
    if encoded_bytes > MAX_CHALLENGE_MESSAGE_BYTES {
        return Err(public_status(
            Code::ResourceExhausted,
            "CHALLENGE_REQUIREMENTS",
        ));
    }
    Ok(())
}

fn valid_optional_bounds<T: Ord>(minimum: Option<T>, maximum: Option<T>) -> bool {
    match (minimum, maximum) {
        (Some(minimum), Some(maximum)) => minimum <= maximum,
        _ => true,
    }
}

fn collection_kind_matches(
    input_kind: SemanticInputKind,
    item_kind: SemanticCollectionItemKind,
) -> bool {
    matches!(
        (input_kind, item_kind),
        (
            SemanticInputKind::StringCollection,
            SemanticCollectionItemKind::String
        ) | (
            SemanticInputKind::IntegerCollection,
            SemanticCollectionItemKind::Integer
        ) | (
            SemanticInputKind::BooleanCollection,
            SemanticCollectionItemKind::Boolean
        ) | (
            SemanticInputKind::EnumCollection,
            SemanticCollectionItemKind::Enum
        )
    )
}

fn safe_key(value: &str, maximum_length: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum_length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
}

fn requirement_to_wire(requirement: &SemanticInputRequirement) -> Result<InputRequirement, Status> {
    let constraints = requirement
        .constraints
        .as_ref()
        .map(constraint_to_wire)
        .transpose()?
        .map(|constraint| ChallengeConstraints {
            constraint: Some(constraint),
        });
    let authorized_choices = requirement
        .authorized_choices
        .iter()
        .map(|choice| {
            let value = match &choice.value {
                SemanticInputValue::String(value) => WireChoiceValue::StringValue(value.clone()),
                SemanticInputValue::Integer(value) => WireChoiceValue::IntegerValue(*value),
                SemanticInputValue::Boolean(value) => WireChoiceValue::BooleanValue(*value),
                _ => {
                    return Err(public_status(
                        Code::InvalidArgument,
                        "CHALLENGE_REQUIREMENT",
                    ));
                }
            };
            Ok(AuthorizedChoice {
                choice_id: choice.choice_id.clone(),
                presentation_key: choice.presentation_key.clone(),
                value: Some(value),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(InputRequirement {
        semantic_field_id: requirement.semantic_field_id.clone(),
        input_kind: input_kind_to_wire(requirement.input_kind) as i32,
        presentation_key: requirement.presentation_key.clone(),
        description_key: requirement.description_key.clone(),
        required: requirement.required,
        constraints,
        authorized_choices,
    })
}

fn constraint_to_wire(constraint: &SemanticInputConstraints) -> Result<WireConstraint, Status> {
    Ok(match constraint {
        SemanticInputConstraints::String {
            minimum_length,
            maximum_length,
            format,
        } => WireConstraint::StringConstraints(ChallengeStringConstraints {
            minimum_length: *minimum_length,
            maximum_length: *maximum_length,
            format: match format {
                SemanticStringFormat::Plain => ChallengeStringFormat::Plain,
                SemanticStringFormat::Identifier => ChallengeStringFormat::Identifier,
                SemanticStringFormat::ReleaseVersion => ChallengeStringFormat::ReleaseVersion,
            } as i32,
        }),
        SemanticInputConstraints::Integer { minimum, maximum } => {
            WireConstraint::IntegerConstraints(ChallengeIntegerConstraints {
                minimum: *minimum,
                maximum: *maximum,
            })
        }
        SemanticInputConstraints::Enum {
            minimum_selections,
            maximum_selections,
        } => WireConstraint::EnumConstraints(ChallengeEnumConstraints {
            minimum_selections: *minimum_selections,
            maximum_selections: *maximum_selections,
        }),
        SemanticInputConstraints::Collection {
            item_kind,
            minimum_items,
            maximum_items,
            unique_items,
        } => WireConstraint::CollectionConstraints(ChallengeCollectionConstraints {
            item_kind: match item_kind {
                SemanticCollectionItemKind::String => ChallengeCollectionItemKind::String,
                SemanticCollectionItemKind::Integer => ChallengeCollectionItemKind::Integer,
                SemanticCollectionItemKind::Boolean => ChallengeCollectionItemKind::Boolean,
                SemanticCollectionItemKind::Enum => ChallengeCollectionItemKind::Enum,
            } as i32,
            minimum_items: *minimum_items,
            maximum_items: *maximum_items,
            unique_items: *unique_items,
        }),
    })
}

const fn input_kind_to_wire(kind: SemanticInputKind) -> ChallengeInputKind {
    match kind {
        SemanticInputKind::String => ChallengeInputKind::String,
        SemanticInputKind::Integer => ChallengeInputKind::Integer,
        SemanticInputKind::Boolean => ChallengeInputKind::Boolean,
        SemanticInputKind::Enum => ChallengeInputKind::Enum,
        SemanticInputKind::StringCollection => ChallengeInputKind::StringCollection,
        SemanticInputKind::IntegerCollection => ChallengeInputKind::IntegerCollection,
        SemanticInputKind::BooleanCollection => ChallengeInputKind::BooleanCollection,
        SemanticInputKind::EnumCollection => ChallengeInputKind::EnumCollection,
    }
}

fn answers_from_wire(
    requirements: &[SemanticInputRequirement],
    answers: &[InputAnswer],
) -> Result<Vec<SemanticInputAnswer>, Status> {
    if answers.len() > requirements.len() {
        return Err(public_status(Code::InvalidArgument, "CHALLENGE_ANSWER"));
    }
    let by_field = requirements
        .iter()
        .map(|requirement| (requirement.semantic_field_id.as_str(), requirement))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut decoded = Vec::with_capacity(answers.len());
    for answer in answers {
        let requirement = by_field
            .get(answer.semantic_field_id.as_str())
            .copied()
            .ok_or_else(|| public_status(Code::InvalidArgument, "CHALLENGE_ANSWER"))?;
        if !seen.insert(answer.semantic_field_id.as_str()) {
            return Err(public_status(Code::InvalidArgument, "CHALLENGE_ANSWER"));
        }
        let value = decode_answer(requirement, answer.value.as_ref())?;
        decoded.push(SemanticInputAnswer {
            semantic_field_id: answer.semantic_field_id.clone(),
            value,
        });
    }
    if requirements.iter().any(|requirement| {
        requirement.required && !seen.contains(requirement.semantic_field_id.as_str())
    }) {
        return Err(public_status(Code::InvalidArgument, "CHALLENGE_ANSWER"));
    }
    Ok(decoded)
}

fn decode_answer(
    requirement: &SemanticInputRequirement,
    value: Option<&WireAnswerValue>,
) -> Result<SemanticInputValue, Status> {
    let value = match (requirement.input_kind, value) {
        (SemanticInputKind::String, Some(WireAnswerValue::StringValue(value))) => {
            SemanticInputValue::String(value.clone())
        }
        (SemanticInputKind::Integer, Some(WireAnswerValue::IntegerValue(value))) => {
            SemanticInputValue::Integer(*value)
        }
        (SemanticInputKind::Boolean, Some(WireAnswerValue::BooleanValue(value))) => {
            SemanticInputValue::Boolean(*value)
        }
        (SemanticInputKind::Enum, Some(WireAnswerValue::ChoiceId(value))) => {
            SemanticInputValue::Choice(value.clone())
        }
        (SemanticInputKind::StringCollection, Some(WireAnswerValue::StringCollection(value))) => {
            SemanticInputValue::Strings(value.values.clone())
        }
        (SemanticInputKind::IntegerCollection, Some(WireAnswerValue::IntegerCollection(value))) => {
            SemanticInputValue::Integers(value.values.clone())
        }
        (SemanticInputKind::BooleanCollection, Some(WireAnswerValue::BooleanCollection(value))) => {
            SemanticInputValue::Booleans(value.values.clone())
        }
        (SemanticInputKind::EnumCollection, Some(WireAnswerValue::ChoiceCollection(value))) => {
            SemanticInputValue::Choices(value.choice_ids.clone())
        }
        _ => return Err(public_status(Code::InvalidArgument, "CHALLENGE_ANSWER")),
    };
    enforce_answer_constraints(requirement, &value)?;
    Ok(value)
}

fn enforce_answer_constraints(
    requirement: &SemanticInputRequirement,
    value: &SemanticInputValue,
) -> Result<(), Status> {
    let valid = match (&requirement.constraints, value) {
        (
            Some(SemanticInputConstraints::String {
                minimum_length,
                maximum_length,
                format,
            }),
            SemanticInputValue::String(value),
        ) => {
            let length = u32::try_from(value.chars().count()).unwrap_or(u32::MAX);
            minimum_length.is_none_or(|minimum| length >= minimum)
                && maximum_length.is_none_or(|maximum| length <= maximum)
                && string_format_matches(*format, value)
        }
        (
            Some(SemanticInputConstraints::Integer { minimum, maximum }),
            SemanticInputValue::Integer(value),
        ) => {
            minimum.is_none_or(|minimum| *value >= minimum)
                && maximum.is_none_or(|maximum| *value <= maximum)
        }
        (None, SemanticInputValue::Boolean(_)) => true,
        (Some(SemanticInputConstraints::Enum { .. }), SemanticInputValue::Choice(choice_id)) => {
            requirement
                .authorized_choices
                .iter()
                .any(|choice| choice.choice_id == *choice_id)
        }
        (
            Some(SemanticInputConstraints::Collection {
                minimum_items,
                maximum_items,
                unique_items,
                ..
            }),
            SemanticInputValue::Strings(values),
        ) => collection_bounds(values, *minimum_items, *maximum_items, *unique_items),
        (
            Some(SemanticInputConstraints::Collection {
                minimum_items,
                maximum_items,
                unique_items,
                ..
            }),
            SemanticInputValue::Integers(values),
        ) => collection_bounds(values, *minimum_items, *maximum_items, *unique_items),
        (
            Some(SemanticInputConstraints::Collection {
                minimum_items,
                maximum_items,
                unique_items,
                ..
            }),
            SemanticInputValue::Booleans(values),
        ) => collection_bounds(values, *minimum_items, *maximum_items, *unique_items),
        (
            Some(SemanticInputConstraints::Collection {
                minimum_items,
                maximum_items,
                unique_items,
                ..
            }),
            SemanticInputValue::Choices(values),
        ) => {
            collection_bounds(values, *minimum_items, *maximum_items, *unique_items)
                && values.iter().all(|choice_id| {
                    requirement
                        .authorized_choices
                        .iter()
                        .any(|choice| choice.choice_id == *choice_id)
                })
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(public_status(Code::InvalidArgument, "CHALLENGE_ANSWER"))
    }
}

fn collection_bounds<T: Ord>(
    values: &[T],
    minimum_items: u32,
    maximum_items: u32,
    unique_items: bool,
) -> bool {
    let length = u32::try_from(values.len()).unwrap_or(u32::MAX);
    length >= minimum_items
        && length <= maximum_items
        && (!unique_items || values.iter().collect::<BTreeSet<_>>().len() == values.len())
}

fn string_format_matches(format: SemanticStringFormat, value: &str) -> bool {
    match format {
        SemanticStringFormat::Plain => true,
        SemanticStringFormat::Identifier => safe_key(value, 128),
        SemanticStringFormat::ReleaseVersion => {
            !value.is_empty()
                && value.len() <= 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'+'))
        }
    }
}

fn reference_completion(
    request: ReferenceCompletionRequest,
    references: &LiveReferenceProjection,
) -> Result<ReferenceCompletion, Status> {
    if request.prefix.len() > 64
        || !request.prefix.is_ascii()
        || request.maximum_candidates == 0
        || request.maximum_candidates > MAX_REFERENCE_COMPLETION_CANDIDATES
        || request.selector.as_ref().is_some_and(|selector| {
            selector.is_empty() || selector.len() > 128 || !selector.is_ascii()
        })
    {
        return Err(public_status(Code::InvalidArgument, "REFERENCE_COMPLETION"));
    }
    let kind_filter = request
        .kind
        .map(|kind| {
            let kind = ReferenceKind::try_from(kind)
                .map_err(|_| public_status(Code::InvalidArgument, "REFERENCE_KIND"))?;
            if kind == ReferenceKind::Unspecified {
                return Err(public_status(Code::InvalidArgument, "REFERENCE_KIND"));
            }
            Ok(kind)
        })
        .transpose()?;
    let variable = ReferenceTemplateVariable::try_from(request.variable)
        .map_err(|_| public_status(Code::InvalidArgument, "REFERENCE_VARIABLE"))?;
    let candidates = match variable {
        ReferenceTemplateVariable::Kind => references
            .entries
            .values()
            .filter(|entry| kind_filter.is_none_or(|kind| kind == entry.kind))
            .map(|entry| {
                (
                    entry.public_kind.to_owned(),
                    entry.presentation_key.to_owned(),
                )
            })
            .collect::<BTreeSet<_>>(),
        ReferenceTemplateVariable::ReleasedVersion => references
            .entries
            .values()
            .filter(|entry| kind_filter.is_none_or(|kind| kind == entry.kind))
            .map(|entry| {
                (
                    entry.released_version.clone(),
                    "reference.release.current".to_owned(),
                )
            })
            .collect::<BTreeSet<_>>(),
        ReferenceTemplateVariable::Unspecified => {
            return Err(public_status(Code::InvalidArgument, "REFERENCE_VARIABLE"));
        }
    };
    // Selector-bearing completion is intentionally non-enumerating. An unsupported, denied, or
    // merely absent selector has the same empty projection, so candidate existence cannot be used
    // as an authorization oracle. The eventual reference read resolves and reauthorizes again.
    let authorized = if request.selector.is_some() {
        BTreeSet::new()
    } else {
        candidates
            .into_iter()
            .filter(|(value, _)| value.starts_with(&request.prefix))
            .collect::<BTreeSet<_>>()
    };
    let total = u32::try_from(authorized.len()).unwrap_or(u32::MAX);
    let candidates = authorized
        .into_iter()
        .take(usize::try_from(request.maximum_candidates).unwrap_or(usize::MAX))
        .map(|(value, presentation_key)| ReferenceCompletionCandidate {
            value: value.clone(),
            presentation_key: presentation_key.clone(),
        })
        .collect::<Vec<_>>();
    Ok(ReferenceCompletion {
        has_more: total > u32::try_from(candidates.len()).unwrap_or(u32::MAX),
        total,
        candidates,
    })
}

const fn reference_kind_projection(kind: ReferenceKind) -> Option<(&'static str, &'static str)> {
    match kind {
        ReferenceKind::Capability => Some(("capability", "reference.kind.capability")),
        ReferenceKind::Guide => Some(("guide", "reference.kind.guide")),
        ReferenceKind::Recipe => Some(("recipe", "reference.kind.recipe")),
        ReferenceKind::RequestSchema => Some(("request-schema", "reference.kind.request_schema")),
        ReferenceKind::ResponseSchema => {
            Some(("response-schema", "reference.kind.response_schema"))
        }
        ReferenceKind::Snapshot => Some(("snapshot", "reference.kind.snapshot")),
        ReferenceKind::Unspecified => None,
    }
}

const fn semantic_value_kind_name(kind: SemanticValueKind) -> &'static str {
    match kind {
        SemanticValueKind::Boolean => "boolean",
        SemanticValueKind::Int64 => "int64",
        SemanticValueKind::UInt64 => "uint64",
        SemanticValueKind::Text => "text",
    }
}

fn validation_issue(status: &Status) -> ValidationIssue {
    let detail = public_error_detail(status.code(), status_public_code(status));
    ValidationIssue {
        code: detail.code,
        semantic_field_id: "query".to_owned(),
        presentation_key: status
            .metadata()
            .get("codefabric-error-code")
            .and_then(|value| value.to_str().ok())
            .map_or_else(
                || "query.validation_rejected".to_owned(),
                |code| format!("query.validation.{}", code.to_ascii_lowercase()),
            ),
        retryable: false,
    }
}

fn status_public_code(status: &Status) -> &str {
    status
        .metadata()
        .get("codefabric-error-code")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("INTERNAL")
}

fn safe_error(
    code: SafeErrorCode,
    layer: SafeErrorLayer,
    retryable: bool,
    diagnostic_reference: &str,
    correlation_id: &str,
) -> SafeErrorMetadata {
    SafeErrorMetadata {
        code: code as i32,
        layer: layer as i32,
        retryable,
        retry_after_ms: None,
        diagnostic_reference: match diagnostic_reference {
            "" => None,
            "lifecycle.failed_closed" => {
                Some(SafeDiagnosticReference::LifecycleFailedClosed as i32)
            }
            "query.challenge_rejected" => {
                Some(SafeDiagnosticReference::QueryChallengeRejected as i32)
            }
            "query.terminal" => Some(SafeDiagnosticReference::QueryTerminal as i32),
            _ => {
                debug_assert!(false, "unreleased diagnostic reference");
                None
            }
        },
        correlation_id: correlation_id.to_owned(),
    }
}

fn processing_page_summary(
    value: crate::fabric::processing_status::QueryProcessing,
    offset: usize,
    remainder_handle: Option<String>,
) -> Result<crate::rpc::generated::codefabric::cpgd::v2::QueryProcessingSummary, Status> {
    use crate::rpc::generated::codefabric::cpgd::v2::{
        ProcessingRemainder, ProcessingState, QueryProcessingSummary,
    };
    if !value.validate_page(offset) {
        return Err(public_status(Code::DataLoss, "RESULT_EVENT_BINDING"));
    }
    let summary = value.processing;
    let remainder = summary
        .remainder
        .into_iter()
        .map(|row| {
            let state = match row.state.as_str() {
                "pending" => ProcessingState::Pending,
                "running" => ProcessingState::Running,
                "partial" => ProcessingState::Partial,
                "unknown" => ProcessingState::Unknown,
                "unavailable" => ProcessingState::Unavailable,
                "excluded" => ProcessingState::Excluded,
                "limited" => ProcessingState::Limited,
                "unsupported" => ProcessingState::Unsupported,
                "failed" => ProcessingState::Failed,
                "cancelled" => ProcessingState::Cancelled,
                _ => return Err(public_status(Code::DataLoss, "RESULT_EVENT_BINDING")),
            };
            Ok(ProcessingRemainder {
                language: row.language,
                scope_kind: row.scope_kind,
                path: row.path,
                path_bytes: row.path_bytes,
                target: row.target,
                target_kind: row.target_kind,
                analysis_context_id: row.analysis_context_id,
                state: state as i32,
                reason_code: row.reason,
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(QueryProcessingSummary {
        query_id: value.query_id,
        source_generation: summary.source_generation,
        scope: summary.scope,
        family: summary.family,
        languages: summary.languages,
        requested_partitions: summary.requested_partitions,
        completed_partitions: summary.completed_partitions,
        remaining_partitions: summary.remaining_partitions,
        remainder,
        next_offset: summary.next_offset.map(|value| value as u64),
        maximum_rows: value.maximum_rows,
        additional_rows: value.additional_rows,
        remainder_handle,
        remainder_offset: Some(offset as u64),
    })
}

fn terminal_safe_code(state: QueryTerminalState) -> SafeErrorCode {
    match state {
        QueryTerminalState::Cancelled => SafeErrorCode::Cancelled,
        QueryTerminalState::Lost => SafeErrorCode::DaemonUnavailable,
        QueryTerminalState::Failed => SafeErrorCode::Internal,
        QueryTerminalState::Succeeded => SafeErrorCode::Unspecified,
    }
}

fn lifecycle_state(phase: ProductionLifecyclePhase) -> LifecycleState {
    match phase {
        ProductionLifecyclePhase::Ready => LifecycleState::Ready,
        ProductionLifecyclePhase::Draining | ProductionLifecyclePhase::Stopped => {
            LifecycleState::Draining
        }
        ProductionLifecyclePhase::FailedClosed => LifecycleState::FailedClosed,
        _ => LifecycleState::Bootstrapping,
    }
}

fn sole_reference_workspace(workspace_ids: &BTreeSet<WorkspaceId>) -> Result<WorkspaceId, Status> {
    let mut workspaces = workspace_ids.iter().copied();
    let Some(workspace_id) = workspaces.next() else {
        return Err(public_status(
            Code::PermissionDenied,
            "REFERENCE_WORKSPACE_SCOPE",
        ));
    };
    if workspaces.next().is_some() {
        return Err(public_status(
            Code::PermissionDenied,
            "REFERENCE_WORKSPACE_SCOPE",
        ));
    }
    Ok(workspace_id)
}

fn missing_reference_query_forms(
    phase: ProductionLifecyclePhase,
) -> Result<Vec<&'static str>, Status> {
    if matches!(
        phase,
        ProductionLifecyclePhase::Configured
            | ProductionLifecyclePhase::DaemonLeased
            | ProductionLifecyclePhase::WriterFenced
            | ProductionLifecyclePhase::CommandRecovered
            | ProductionLifecyclePhase::GenesisRequired
            | ProductionLifecyclePhase::SelectedEpochRecovered
            | ProductionLifecyclePhase::EpochBuiltAndProved
    ) {
        Ok(Vec::new())
    } else {
        Err(public_status(Code::Internal, "REFERENCE_AUTHORITY"))
    }
}

fn execution_state(phase: QueryExecutionPhase) -> QueryExecutionState {
    match phase {
        QueryExecutionPhase::Queued => QueryExecutionState::Queued,
        QueryExecutionPhase::Running => QueryExecutionState::Running,
        QueryExecutionPhase::Terminal(state) => terminal_state(state),
    }
}

fn terminal_state(state: QueryTerminalState) -> QueryExecutionState {
    match state {
        QueryTerminalState::Succeeded => QueryExecutionState::Succeeded,
        QueryTerminalState::Failed => QueryExecutionState::Failed,
        QueryTerminalState::Cancelled => QueryExecutionState::Cancelled,
        QueryTerminalState::Lost => QueryExecutionState::Lost,
    }
}

fn semantic_status(error: SemanticQueryError) -> Status {
    let (code, public_code) = match error {
        SemanticQueryError::Phase {
            code: "FRESHNESS_DEADLINE",
            ..
        } => (Code::DeadlineExceeded, "FRESHNESS_DEADLINE"),
        SemanticQueryError::Phase {
            code: "FRESHNESS_UNAVAILABLE" | "FRESHNESS_SELECTION_CHANGED",
            ..
        } => (Code::Unavailable, "FRESHNESS_UNAVAILABLE"),
        SemanticQueryError::Phase {
            code: "SOURCE_ACCESS_DENIED",
            ..
        } => (Code::PermissionDenied, "SOURCE_ACCESS_DENIED"),
        SemanticQueryError::RequestTooLarge => {
            (Code::ResourceExhausted, "SEMANTIC_REQUEST_CAPACITY")
        }
        SemanticQueryError::Invalid(_) | SemanticQueryError::Canonical(_) => {
            (Code::InvalidArgument, "SEMANTIC_REQUEST")
        }
        SemanticQueryError::Phase {
            code: "RESOURCE_CAPACITY",
            ..
        } => (Code::ResourceExhausted, "RESOURCE_CAPACITY"),
        SemanticQueryError::Phase {
            code: "QUERY_HARD_LIMIT_EXCEEDED",
            ..
        } => (Code::ResourceExhausted, "QUERY_HARD_LIMIT_EXCEEDED"),
        SemanticQueryError::Phase {
            code: "SEMANTIC_REFERENCE_UNAVAILABLE",
            ..
        } => (Code::InvalidArgument, "SEMANTIC_REFERENCE_UNAVAILABLE"),
        SemanticQueryError::Phase { .. } => (Code::FailedPrecondition, "SEMANTIC_REQUEST"),
    };
    public_status(code, public_code)
}

fn session_status(error: SessionAuthorityError) -> Status {
    let (code, public_code) = match error {
        SessionAuthorityError::Capacity => (Code::ResourceExhausted, "SESSION_CAPACITY"),
        SessionAuthorityError::OperationDenied => (Code::PermissionDenied, "OPERATION_DENIED"),
        SessionAuthorityError::ProfileUnavailable | SessionAuthorityError::GenerationMismatch => {
            (Code::FailedPrecondition, "SESSION_GENERATION")
        }
        SessionAuthorityError::InvalidAuthority | SessionAuthorityError::Entropy => {
            (Code::Internal, "SESSION_AUTHORITY")
        }
        _ => (Code::Unauthenticated, "SESSION_INVALID"),
    };
    public_status(code, public_code)
}

fn coordinator_status(error: QueryCoordinatorError) -> Status {
    let (code, public_code) = match error {
        QueryCoordinatorError::IdempotencyConflict => (Code::AlreadyExists, "IDEMPOTENCY_CONFLICT"),
        QueryCoordinatorError::AdmissionBackpressure
        | QueryCoordinatorError::ResultCapacityBackpressure
        | QueryCoordinatorError::TaskCapacity
        | QueryCoordinatorError::JournalCapacity => (Code::ResourceExhausted, "QUERY_CAPACITY"),
        QueryCoordinatorError::Resource(
            crate::resource_budget::ResourceBudgetError::Exhausted { .. }
            | crate::resource_budget::ResourceBudgetError::ScopeCapacity,
        ) => (Code::ResourceExhausted, "QUERY_CAPACITY"),
        QueryCoordinatorError::UnknownQuery(_) => (Code::NotFound, "QUERY_NOT_FOUND"),
        QueryCoordinatorError::QueryOwnerMismatch
        | QueryCoordinatorError::CursorBinding
        | QueryCoordinatorError::InvalidCursor => (Code::PermissionDenied, "QUERY_BINDING"),
        QueryCoordinatorError::DeadlineElapsed | QueryCoordinatorError::CursorExpiry => {
            (Code::DeadlineExceeded, "QUERY_DEADLINE")
        }
        QueryCoordinatorError::AlreadyTerminal => (Code::FailedPrecondition, "QUERY_TERMINAL"),
        QueryCoordinatorError::ResultNotReleasable => {
            (Code::FailedPrecondition, "RESULT_NOT_RETAINED")
        }
        QueryCoordinatorError::Cancelled => (Code::Cancelled, "QUERY_CANCELLED"),
        QueryCoordinatorError::NonCanonicalRequest
        | QueryCoordinatorError::CanonicalRequest(_)
        | QueryCoordinatorError::InvalidIdentity
        | QueryCoordinatorError::InvalidIdempotencyKey
        | QueryCoordinatorError::InvalidOperationField(_)
        | QueryCoordinatorError::InvalidOperationBounds => (Code::InvalidArgument, "QUERY_REQUEST"),
        QueryCoordinatorError::InvalidRetainedPackageLocator
        | QueryCoordinatorError::InvalidResultRetentionState => {
            (Code::DataLoss, "RESULT_EVENT_BINDING")
        }
        _ => (Code::Internal, "QUERY_COORDINATOR"),
    };
    public_status(code, public_code)
}

fn result_status(error: StreamedResultRegistryError) -> Status {
    let (code, public_code) = match error {
        StreamedResultRegistryError::ProcessingUnavailable => {
            (Code::Unavailable, "PROCESSING_UNAVAILABLE")
        }
        StreamedResultRegistryError::SourceAccessDenied => {
            (Code::PermissionDenied, "SOURCE_ACCESS_DENIED")
        }
        StreamedResultRegistryError::UnknownPackage
        | StreamedResultRegistryError::UnknownResource => (Code::NotFound, "RESOURCE_NOT_FOUND"),
        StreamedResultRegistryError::WrongOwner
        | StreamedResultRegistryError::WrongWorkspace
        | StreamedResultRegistryError::BudgetOwnerMismatch => {
            (Code::PermissionDenied, "RESOURCE_BINDING")
        }
        StreamedResultRegistryError::GenerationMismatch => {
            (Code::FailedPrecondition, "GENERATION_MISMATCH")
        }
        StreamedResultRegistryError::PolicyGenerationMismatch => {
            (Code::FailedPrecondition, "POLICY_GENERATION_MISMATCH")
        }
        StreamedResultRegistryError::ReferenceCapacity => {
            (Code::ResourceExhausted, "RESOURCE_CAPACITY")
        }
        StreamedResultRegistryError::Expired => (Code::DeadlineExceeded, "RESOURCE_EXPIRED"),
        StreamedResultRegistryError::Resource(
            crate::resource_budget::ResourceBudgetError::Exhausted { .. }
            | crate::resource_budget::ResourceBudgetError::ScopeCapacity,
        ) => (Code::ResourceExhausted, "RESOURCE_CAPACITY"),
        StreamedResultRegistryError::Released => (Code::FailedPrecondition, "RESOURCE_RELEASED"),
        StreamedResultRegistryError::InvalidChunkBound => (Code::InvalidArgument, "RESOURCE_BOUND"),
        StreamedResultRegistryError::RangeOutsideResource
        | StreamedResultRegistryError::RangeOverflow => (Code::OutOfRange, "RESOURCE_RANGE"),
        StreamedResultRegistryError::ResourceIntegrity => (Code::DataLoss, "RESOURCE_INTEGRITY"),
        StreamedResultRegistryError::RetainedLocatorMismatch => {
            (Code::DataLoss, "RESULT_EVENT_BINDING")
        }
        StreamedResultRegistryError::RecoveryUnavailable => {
            (Code::Unavailable, "RESULT_RECOVERY_UNAVAILABLE")
        }
        _ => (Code::Internal, "RESOURCE_FAILURE"),
    };
    public_status(code, public_code)
}

fn public_status(code: Code, public_code: &'static str) -> Status {
    let mut status = Status::new(code, "request rejected");
    status.metadata_mut().insert(
        "codefabric-error-code",
        MetadataValue::from_static(public_code),
    );
    let detail = public_error_detail(code, public_code).encode_to_vec();
    status.metadata_mut().insert_bin(
        "codefabric-safe-error-bin",
        MetadataValue::from_bytes(&detail),
    );
    status
}

fn public_error_detail(code: Code, public_code: &str) -> SafeErrorMetadata {
    let safe_code = match public_code {
        "QUERY_HARD_LIMIT_EXCEEDED" => SafeErrorCode::QueryHardLimitExceeded,
        "SEMANTIC_REFERENCE_UNAVAILABLE" => SafeErrorCode::ValidationRejected,
        "IDEMPOTENCY_CONFLICT" => SafeErrorCode::IdempotencyConflict,
        "CHALLENGE_EXPIRED" => SafeErrorCode::ContinuationExpired,
        "CHALLENGE_REPLAY" => SafeErrorCode::ContinuationReplayed,
        "GENERATION_MISMATCH" | "POLICY_GENERATION_MISMATCH" | "SESSION_GENERATION" => {
            SafeErrorCode::GenerationMismatch
        }
        "QUERY_NOT_FOUND" => SafeErrorCode::QueryNotFound,
        "RESOURCE_NOT_FOUND" => SafeErrorCode::ResourceNotFound,
        "RESOURCE_EXPIRED" => SafeErrorCode::ResourceExpired,
        "RESOURCE_RELEASED" => SafeErrorCode::ResourceReleased,
        "RESULT_NOT_RETAINED" => SafeErrorCode::ResultNotRetained,
        "RESOURCE_RANGE" => SafeErrorCode::RangeNotSatisfiable,
        "QUERY_CAPACITY" | "SESSION_CAPACITY" | "CHALLENGE_CAPACITY" | "START_CAPACITY"
        | "RESOURCE_CAPACITY" => SafeErrorCode::CapacityUnavailable,
        "QUERY_CANCELLED" => SafeErrorCode::Cancelled,
        "QUERY_DEADLINE" => SafeErrorCode::ResumeWindowExpired,
        "FRESHNESS_DEADLINE" => SafeErrorCode::FreshnessDeadline,
        "FRESHNESS_UNAVAILABLE" => SafeErrorCode::FreshnessUnavailable,
        "LIFECYCLE_NOT_READY" | "RESULT_RECOVERY_UNAVAILABLE" => SafeErrorCode::DaemonUnavailable,
        _ => match code {
            Code::InvalidArgument => SafeErrorCode::InvalidRequest,
            Code::Unauthenticated | Code::PermissionDenied => SafeErrorCode::NotAuthorized,
            Code::AlreadyExists => SafeErrorCode::IdempotencyConflict,
            Code::ResourceExhausted => SafeErrorCode::CapacityUnavailable,
            Code::Cancelled => SafeErrorCode::Cancelled,
            Code::DeadlineExceeded => SafeErrorCode::DaemonUnavailable,
            Code::Unavailable => SafeErrorCode::DaemonUnavailable,
            _ => SafeErrorCode::Internal,
        },
    };
    let layer = match public_code {
        value
            if value.starts_with("SESSION_")
                || value.starts_with("REQUEST_")
                || value.starts_with("WORKSPACE_")
                || value.starts_with("SOURCE_ACCESS_")
                || value.starts_with("OPERATION_")
                || value.ends_with("_BINDING") =>
        {
            SafeErrorLayer::Authorization
        }
        value if value.starts_with("RESOURCE_") || value.starts_with("PUBLIC_HANDLE") => {
            SafeErrorLayer::Resource
        }
        value if value.starts_with("LIFECYCLE_") || value.starts_with("STATUS_") => {
            SafeErrorLayer::Lifecycle
        }
        value
            if value.starts_with("QUERY_")
                || value.starts_with("FRESHNESS_")
                || value.starts_with("START_")
                || value.starts_with("CHALLENGE_")
                || value.starts_with("IDEMPOTENCY_") =>
        {
            SafeErrorLayer::Query
        }
        value
            if value.starts_with("REFERENCE_")
                || value.starts_with("SEMANTIC_")
                || value.starts_with("RESULT_LIMITS")
                || value.starts_with("EXECUTION_BUDGET") =>
        {
            SafeErrorLayer::Validation
        }
        _ => SafeErrorLayer::Transport,
    };
    SafeErrorMetadata {
        code: safe_code as i32,
        layer: layer as i32,
        retryable: matches!(
            safe_code,
            SafeErrorCode::CapacityUnavailable
                | SafeErrorCode::DaemonUnavailable
                | SafeErrorCode::ResumeWindowExpired
                | SafeErrorCode::FreshnessDeadline
                | SafeErrorCode::FreshnessUnavailable
        ),
        retry_after_ms: None,
        diagnostic_reference: None,
        correlation_id: String::new(),
    }
}

fn identity32(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    frame(&mut hasher, domain);
    for field in fields {
        frame(&mut hasher, field);
    }
    *hasher.finalize().as_bytes()
}

fn frame(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(value);
}

fn random16() -> Result<[u8; 16], ()> {
    crate::identity::random_registration_nonce().map_err(|_| ())
}

fn random32() -> Result<[u8; 32], ()> {
    let first = random16()?;
    let second = random16()?;
    let mut value = [0_u8; 32];
    value[..16].copy_from_slice(&first);
    value[16..].copy_from_slice(&second);
    Ok(value)
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream as StdUnixStream;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::cancellation::Cancellation;
    use crate::fabric::query_coordinator::{QueryCoordinatorPolicy, SqliteQueryCoordinatorJournal};
    use crate::query_backend::{
        SemanticAuthorizedChoice, SemanticExecutionPreparation, SemanticInputConstraints,
    };
    use crate::rpc::SameUserInterceptor;
    use crate::rpc::generated::codefabric::cpgd::v2::InitialQueryStart;
    use crate::session_authority::{
        LaunchPolicyId, LaunchPolicyRevision, RegisteredLaunchGrant, RevocationGeneration,
    };

    #[derive(Debug)]
    struct TestExecutionLease(Arc<AtomicUsize>);

    impl TestExecutionLease {
        fn new(live: Arc<AtomicUsize>) -> Self {
            live.fetch_add(1, Ordering::SeqCst);
            Self(live)
        }
    }

    impl Clone for TestExecutionLease {
        fn clone(&self) -> Self {
            Self::new(Arc::clone(&self.0))
        }
    }

    impl Drop for TestExecutionLease {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    #[derive(Debug, Default)]
    struct ThreeRoundBackend {
        preparation_calls: AtomicUsize,
        fail_preparation: std::sync::atomic::AtomicBool,
        admission_calls: AtomicUsize,
        live_execution_leases: Arc<AtomicUsize>,
        application_release_pin: Option<[u8; 32]>,
    }

    #[async_trait::async_trait]
    impl SemanticQueryBackend for ThreeRoundBackend {
        type ExecutionAuthority = TestExecutionLease;

        fn application_release_pin(&self) -> Option<[u8; 32]> {
            self.application_release_pin
        }

        fn validate_execution_request(
            &self,
            _request: &crate::semantic_query_contract::ParsedSemanticRequest,
        ) -> Result<(), SemanticQueryError> {
            Ok(())
        }

        fn prepare_execution_request(
            &self,
            request: &crate::semantic_query_contract::ParsedSemanticRequest,
            answers: &[SemanticInputAnswer],
        ) -> Result<SemanticExecutionPreparation, SemanticQueryError> {
            self.preparation_calls.fetch_add(1, Ordering::SeqCst);
            if self.fail_preparation.load(Ordering::SeqCst) {
                return Err(SemanticQueryError::Invalid(
                    "simulated replacement catalog rejects preparation".to_owned(),
                ));
            }
            for (index, answer) in answers.iter().enumerate() {
                let round = index + 1;
                if answer.semantic_field_id != format!("field:round-{round}")
                    || answer.value != SemanticInputValue::Choice(format!("choice:round-{round}"))
                {
                    return Err(SemanticQueryError::Invalid(
                        "guard answer differs from the typed round requirement".to_owned(),
                    ));
                }
            }
            if answers.len() < usize::try_from(MAX_CHALLENGE_ROUNDS).unwrap() {
                let round = answers.len() + 1;
                return Ok(SemanticExecutionPreparation::InputRequired(vec![
                    SemanticInputRequirement {
                        semantic_field_id: format!("field:round-{round}"),
                        input_kind: SemanticInputKind::Enum,
                        presentation_key: format!("input.round-{round}"),
                        description_key: None,
                        required: true,
                        constraints: Some(SemanticInputConstraints::Enum {
                            minimum_selections: 1,
                            maximum_selections: 1,
                        }),
                        authorized_choices: vec![SemanticAuthorizedChoice {
                            choice_id: format!("choice:round-{round}"),
                            presentation_key: format!("choice.round-{round}"),
                            value: SemanticInputValue::String(format!("answer-{round}")),
                        }],
                    },
                ]));
            }
            Ok(SemanticExecutionPreparation::Ready(
                ResolvedSemanticExecutionRequest::try_new(request.clone(), answers.to_vec())?,
            ))
        }

        fn admit_execution_request(
            &self,
            resolved: ResolvedSemanticExecutionRequest,
        ) -> Result<PreparedSemanticExecution<Self::ExecutionAuthority>, SemanticQueryError>
        {
            self.admission_calls.fetch_add(1, Ordering::SeqCst);
            let workspace_id = resolved.parsed().request.workspace_id.clone();
            Ok(PreparedSemanticExecution::new(
                resolved,
                TestExecutionLease::new(Arc::clone(&self.live_execution_leases)),
                crate::semantic_query_contract::SemanticSnapshotResponse {
                    snapshot_id: "epoch:test".to_owned(),
                    workspace_id,
                    repository_id: None,
                    worktree_id: None,
                    source_generation: 1,
                    source_inventory_digest: format!("b3:{}", "11".repeat(32)),
                    durable_base_publication: "delta:test@1".to_owned(),
                    base_table_version_digest: format!("b3:{}", "22".repeat(32)),
                    overlay_generation: 0,
                    overlay_checksum: format!("b3:{}", "33".repeat(32)),
                    analysis_context_set_id: "analysis:test".to_owned(),
                    analysis_context_ids: vec!["analysis:test".to_owned()],
                    freshness_state: FreshnessState::Current,
                    source_trust_state: "EXACT_TYPED_INPUTS".to_owned(),
                    event_stream_health: "ACTIVATION_CHAIN_SELECTED".to_owned(),
                    git_acceleration_status: "NON_AUTHORITY".to_owned(),
                    git_operation_summary: None,
                    pending_update_count: 0,
                    ontology_version: "not-applicable-programmatic-authority".to_owned(),
                    schema_bundle_version: "schema:test".to_owned(),
                    provider_bundle_version: "provider:test".to_owned(),
                    derivation_bundle_version: "derivation:test".to_owned(),
                    query_language_version: "2.0".to_owned(),
                    capability_summaries: Vec::new(),
                    diagnostic_references: Vec::new(),
                },
            ))
        }

        async fn execute(
            &self,
            _request: PreparedSemanticExecution<Self::ExecutionAuthority>,
            _freshness: FreshnessState,
            _cancellation: Cancellation,
            _context: SemanticBackendExecutionContext,
            _artifacts: QueryExecutionArtifactAccumulator,
        ) -> SemanticBackendOutcome {
            panic!("ledger tests never execute accepted work")
        }
    }

    const TEST_REQUEST: &[u8] = br#"{
        "specification":"composable semantic CPG fact query",
        "version":"2.0",
        "semantic_request_id":"request:guard-ledger",
        "scope":{"workspace_id":"workspace:00000000000000000000000000000000"},
        "freshness":{"policy":"best_available_snapshot"},
        "queries":[{
            "request":"find code entities",
            "query_id":"q1",
            "looking_for":"syntax nodes",
            "within":[],
            "where":[],
            "return":{"limit":{"maximum_results":1}}
        }]
    }"#;

    fn current_peer() -> VerifiedPeerIdentity {
        let (peer, _other) = StdUnixStream::pair().expect("peer socketpair");
        peer.set_nonblocking(true).expect("peer nonblocking");
        let stream = tokio::net::UnixStream::from_std(peer).expect("Tokio peer stream");
        SameUserInterceptor::new(rustix::process::geteuid().as_raw())
            .authenticate_stream(&stream)
            .expect("kernel peer identity")
    }

    async fn test_session_with_operations(
        operations: BTreeSet<SessionOperation>,
    ) -> (
        Arc<LaunchGrantAuthority>,
        AuthorizedSession,
        VerifiedPeerIdentity,
    ) {
        let peer = current_peer();
        let raw = [0x5a; 32];
        let observed_at_unix_ms = now_millis();
        let authority = Arc::new(LaunchGrantAuthority::try_new(7, 9, 4, 4).unwrap());
        authority
            .register(RegisteredLaunchGrant {
                grant_id: "launch:guard-ledger".to_owned(),
                grant_digest: *blake3::hash(&raw).as_bytes(),
                policy_id: LaunchPolicyId::try_new("policy:guard-ledger").unwrap(),
                policy_revision: LaunchPolicyRevision::new(8).unwrap(),
                revocation_generation: RevocationGeneration::new(9).unwrap(),
                principal_id: [0x11; 16],
                workspace_ids: vec![[0x22; 16]],
                operations,
                semantic_profiles: BTreeSet::from([SEMANTIC_PROFILE.to_owned()]),
                maximum_resource_chunk_bytes: 1_024,
                maximum_result_bytes: 8_192,
                maximum_result_pages: 8,
                maximum_request_state_ttl_seconds: 60,
                issued_at_unix_ms: observed_at_unix_ms.saturating_sub(1_000),
                expires_at_unix_ms: observed_at_unix_ms.saturating_add(60_000),
                daemon_generation: 7,
                supervisor_generation: 9,
                peer_uid: peer.uid(),
                peer_pid: peer.pid(),
                peer_start_identity: peer
                    .pid()
                    .and_then(crate::session_authority::observed_process_start_identity),
            })
            .await
            .unwrap();
        let session = authority
            .consume_grant(
                &raw,
                peer,
                &[SEMANTIC_PROFILE.to_owned()],
                1_024,
                observed_at_unix_ms,
            )
            .await
            .unwrap();
        (authority, session, peer)
    }

    async fn test_session() -> (Arc<LaunchGrantAuthority>, AuthorizedSession) {
        let (authority, session, _peer) =
            test_session_with_operations(BTreeSet::from([SessionOperation::Start])).await;
        (authority, session)
    }

    fn test_coordinator(temp: &tempfile::TempDir) -> Arc<QueryCoordinator> {
        let policy =
            QueryCoordinatorPolicy::try_new(1, 1, 1, 4, 4, 8, 4_096, 16_384, 16, 16).unwrap();
        let journal = Arc::new(
            SqliteQueryCoordinatorJournal::open(&temp.path().join("query.sqlite")).unwrap(),
        );
        Arc::new(
            QueryCoordinator::try_new(
                policy,
                7,
                [0x71; 32],
                journal,
                100,
                crate::fabric::workspace_resources::test_workspace_budget(),
            )
            .unwrap(),
        )
    }

    fn coordinator_operation(key: &str, marker: u8) -> NormalizedQueryOperation {
        NormalizedQueryOperation::try_new(NormalizedQueryOperation {
            workspace_id: WorkspaceId::from_bytes([0x22; 16]),
            principal_id: PrincipalId::from_bytes([0x11; 16]),
            policy_generation: 1,
            revocation_generation: 1,
            session_sharing_class: QuerySessionSharingClass::PrincipalBound,
            idempotency_key: Arc::from(key),
            canonical_request: Arc::from(
                format!(r#"{{"kind":"budget","marker":{marker}}}"#).into_bytes(),
            ),
            semantic_profile: Arc::from(SEMANTIC_PROFILE),
            request_contract: Arc::from("codefabric.semantic-query-request.v2"),
            response_contract: Arc::from("codefabric.semantic-query-response.v2"),
            delivery_profile: Arc::from("daemon-resource"),
            compression_profile: Arc::from("identity"),
            freshness_policy: Arc::from("fresh"),
            epoch_policy: Arc::from("selected-active-epoch"),
            deadline_unix_ms: 5_000,
            lease_expires_at_unix_ms: 10_000,
            maximum_result_bytes: 1_024,
            maximum_result_pages: 4,
        })
        .unwrap()
    }

    async fn test_service(
        temp: &tempfile::TempDir,
    ) -> (
        ProductionQueryService<ThreeRoundBackend>,
        AuthorizedSession,
        PreparedSubmission,
    ) {
        let (sessions, session) = test_session().await;
        let service = ProductionQueryService::try_new(
            Arc::new(crate::fabric::production_kernel::compile_test_semantic_release()),
            Arc::new(ThreeRoundBackend::default()),
            Arc::new(LifecycleAuthority::new()),
            Arc::new(WorkspaceSlotRegistry::new()),
            test_coordinator(temp),
            sessions,
            Arc::new(
                StreamedResultRegistry::try_new(
                    1_024,
                    crate::fabric::workspace_resources::test_workspace_budget(),
                )
                .unwrap(),
            ),
            "daemon:guard-ledger",
        )
        .unwrap();
        let parsed = parse_request(TEST_REQUEST).unwrap();
        let prepared = service
            .prepare_semantic_submission(parsed, Vec::new())
            .await
            .unwrap();
        (service, session, prepared)
    }

    #[tokio::test]
    async fn injected_release_runtime_boundary_integrity() {
        let temp = tempfile::tempdir().unwrap();
        let release = Arc::new(crate::fabric::production_kernel::compile_test_semantic_release());
        let (sessions, _session) = test_session().await;
        let service = ProductionQueryService::try_new(
            Arc::clone(&release),
            Arc::new(ThreeRoundBackend::default()),
            Arc::new(LifecycleAuthority::new()),
            Arc::new(WorkspaceSlotRegistry::new()),
            test_coordinator(&temp),
            sessions,
            Arc::new(
                StreamedResultRegistry::try_new(
                    32,
                    crate::fabric::workspace_resources::test_workspace_budget(),
                )
                .unwrap(),
            ),
            "daemon:injected-release-oracle",
        )
        .unwrap();

        assert!(Arc::ptr_eq(service.application().release(), &release));
        assert_eq!(
            service.application().release().suite().as_str(),
            "codefabric-relational-data-fabric@2.3.0"
        );
        assert_eq!(
            service.application().daemon_instance_id.as_ref(),
            "daemon:injected-release-oracle"
        );
    }

    #[tokio::test]
    async fn single_release_consumer_composition() {
        let temp = tempfile::tempdir().unwrap();
        let release = Arc::new(crate::fabric::production_kernel::compile_test_semantic_release());
        let release_pin =
            crate::fabric::programmatic_query_backend::compiled_query_release_pin(&release);
        let backend = Arc::new(ThreeRoundBackend {
            application_release_pin: Some(release_pin),
            ..ThreeRoundBackend::default()
        });
        let (sessions, _session) = test_session().await;
        let service = ProductionQueryService::try_new(
            Arc::clone(&release),
            Arc::clone(&backend),
            Arc::new(LifecycleAuthority::new()),
            Arc::new(WorkspaceSlotRegistry::new()),
            test_coordinator(&temp),
            sessions,
            Arc::new(
                StreamedResultRegistry::try_new(
                    32,
                    crate::fabric::workspace_resources::test_workspace_budget(),
                )
                .unwrap(),
            ),
            "daemon:single-release-oracle",
        )
        .unwrap();

        assert!(Arc::ptr_eq(service.application().release(), &release));
        assert_eq!(backend.application_release_pin(), Some(release_pin));
        assert_eq!(
            crate::fabric::programmatic_query_backend::compiled_query_release_pin(
                service.application().release().as_ref(),
            ),
            release_pin
        );
    }

    #[tokio::test]
    async fn grpc_flow_control_and_release_mismatch_faults() {
        let release = Arc::new(crate::fabric::production_kernel::compile_test_semantic_release());
        let expected =
            crate::fabric::programmatic_query_backend::compiled_query_release_pin(&release);
        let mismatch = Arc::new(ThreeRoundBackend {
            application_release_pin: Some(identity32(b"mismatched-release", &[&expected])),
            ..ThreeRoundBackend::default()
        });
        let temp = tempfile::tempdir().unwrap();
        let (sessions, _session) = test_session().await;
        let service = ProductionQueryService::try_new(
            release,
            mismatch,
            Arc::new(LifecycleAuthority::new()),
            Arc::new(WorkspaceSlotRegistry::new()),
            test_coordinator(&temp),
            sessions,
            Arc::new(
                StreamedResultRegistry::try_new(
                    32,
                    crate::fabric::workspace_resources::test_workspace_budget(),
                )
                .unwrap(),
            ),
            "daemon:mismatch-oracle",
        );
        assert!(matches!(
            service,
            Err(QueryServiceCompositionError::ReleaseMismatch)
        ));

        let admission = RpcAdmission::new(1, 1);
        let data = admission.data().expect("one bounded data stream");
        assert_eq!(
            admission.data().unwrap_err().code(),
            Code::ResourceExhausted
        );
        let control = admission
            .control(RpcBudget::from_duration(Duration::from_secs(1)).unwrap())
            .await
            .expect("reserved control remains prompt while data capacity is occupied");
        drop(data);
        drop(control);
        assert!(admission.data().is_ok());
        assert!(
            admission
                .control(RpcBudget::from_duration(Duration::from_secs(1)).unwrap())
                .await
                .is_ok()
        );
    }

    fn scope(session: &AuthorizedSession, key: &str) -> StartScope {
        StartScope {
            principal_id: session.principal_id(),
            workspace_id: WorkspaceId::from_bytes([0x22; 16]),
            policy_generation: session.policy_generation(),
            revocation_generation: session.revocation_generation(),
            semantic_request_id: key.to_owned(),
        }
    }

    fn challenge(outcome: StartOutcome) -> InputChallenge {
        let StartOutcome::InputChallenge(challenge) = outcome else {
            panic!("expected typed input challenge")
        };
        challenge
    }

    fn continuation(challenge: &InputChallenge, choice_id: String) -> QueryChallengeContinuation {
        QueryChallengeContinuation {
            challenge_id: challenge.challenge_id.clone(),
            round: challenge.round,
            daemon_continuation: challenge.daemon_continuation.clone(),
            answers: vec![InputAnswer {
                semantic_field_id: format!("field:round-{}", challenge.round),
                value: Some(WireAnswerValue::ChoiceId(choice_id)),
            }],
        }
    }

    fn open_test_lifecycle(lifecycle: &LifecycleAuthority) {
        for (expected, next) in [
            (
                ProductionLifecyclePhase::Configured,
                ProductionLifecyclePhase::DaemonLeased,
            ),
            (
                ProductionLifecyclePhase::DaemonLeased,
                ProductionLifecyclePhase::WriterFenced,
            ),
            (
                ProductionLifecyclePhase::WriterFenced,
                ProductionLifecyclePhase::EndpointsBoundBootstrapping,
            ),
            (
                ProductionLifecyclePhase::EndpointsBoundBootstrapping,
                ProductionLifecyclePhase::SoleTargetAuthorityObserved,
            ),
            (
                ProductionLifecyclePhase::SoleTargetAuthorityObserved,
                ProductionLifecyclePhase::SoleTargetAuthorityCommitted,
            ),
            (
                ProductionLifecyclePhase::SoleTargetAuthorityCommitted,
                ProductionLifecyclePhase::Ready,
            ),
        ] {
            lifecycle.advance(expected, next).unwrap();
        }
    }

    fn initial_start_request(
        session: &AuthorizedSession,
        query: QuerySubmission,
        correlation_id: &str,
    ) -> Request<StartQueryRequest> {
        let mut request = Request::new(StartQueryRequest {
            context: Some(RequestContext {
                correlation_id: correlation_id.to_owned(),
                remaining_budget: Some(prost_types::Duration {
                    seconds: 5,
                    nanos: 0,
                }),
            }),
            leg: Some(StartLeg::Initial(InitialQueryStart { query: Some(query) })),
        });
        request.extensions_mut().insert(current_peer());
        request.metadata_mut().insert_bin(
            SESSION_METADATA_KEY,
            MetadataValue::from_bytes(session.token()),
        );
        request
    }

    #[tokio::test]
    async fn wp45_explicit_validation_is_pure_and_creates_no_start_authority() {
        let temp = tempfile::tempdir().unwrap();
        let (service, session, _prepared) = test_service(&temp).await;
        let mut request_value: serde_json::Value = serde_json::from_slice(TEST_REQUEST).unwrap();
        request_value["scope"]["workspace_id"] = serde_json::Value::String(
            crate::identity::encode_public_id(IdentityDomain::Workspace, None, [0x22; 16]).unwrap(),
        );
        let canonical_request = serde_json_canonicalizer::to_vec(&request_value).unwrap();
        let query = QuerySubmission {
            request_checksum: crate::integrity::framed_digest(&canonical_request),
            canonical_request_json: canonical_request,
            semantic_request_id: Some("request:guard-ledger".to_owned()),
            semantic_profile: SEMANTIC_PROFILE.to_owned(),
            result_limits: Some(crate::rpc::generated::codefabric::cpgd::v2::ResultLimits {
                maximum_result_bytes: 8_192,
                maximum_result_pages: 8,
            }),
        };
        let before = service.coordinator.snapshot().await;
        let first = service
            .validate_submission(&session, &query)
            .await
            .expect("first pure validation");
        let second = service
            .validate_submission(&session, &query)
            .await
            .expect("repeat pure validation");
        assert_eq!(first.parsed.canonical_bytes, second.parsed.canonical_bytes);
        assert_eq!(first.requirements, second.requirements);
        assert_eq!(service.coordinator.snapshot().await, before);
        assert_eq!(service.backend.admission_calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            service.backend.live_execution_leases.load(Ordering::SeqCst),
            0
        );
        let starts = service.starts.lock().await;
        assert!(starts.challenges.is_empty());
        assert!(starts.outcomes.is_empty());
    }

    #[tokio::test]
    async fn wp45_start_replay_precedes_changed_catalog_preparation() {
        let temp = tempfile::tempdir().unwrap();
        let (service, session, _prepared) = test_service(&temp).await;
        open_test_lifecycle(&service.lifecycle);
        let mut request_value: serde_json::Value = serde_json::from_slice(TEST_REQUEST).unwrap();
        request_value["scope"]["workspace_id"] = serde_json::Value::String(
            crate::identity::encode_public_id(IdentityDomain::Workspace, None, [0x22; 16]).unwrap(),
        );
        let canonical_request = serde_json_canonicalizer::to_vec(&request_value).unwrap();
        let query = QuerySubmission {
            request_checksum: crate::integrity::framed_digest(&canonical_request),
            canonical_request_json: canonical_request,
            semantic_request_id: Some("request:guard-ledger".to_owned()),
            semantic_profile: SEMANTIC_PROFILE.to_owned(),
            result_limits: Some(crate::rpc::generated::codefabric::cpgd::v2::ResultLimits {
                maximum_result_bytes: 8_192,
                maximum_result_pages: 8,
            }),
        };
        let before = service.backend.preparation_calls.load(Ordering::SeqCst);
        let first = service
            .start_query(initial_start_request(
                &session,
                query.clone(),
                "correlation:catalog-before",
            ))
            .await
            .unwrap()
            .into_inner();
        let StartOutcome::InputChallenge(first_challenge) = first.outcome.unwrap() else {
            panic!("new start must materialize its original challenge")
        };
        assert_eq!(
            service.backend.preparation_calls.load(Ordering::SeqCst),
            before + 1
        );

        service
            .backend
            .fail_preparation
            .store(true, Ordering::SeqCst);
        let replay = service
            .start_query(initial_start_request(
                &session,
                query,
                "correlation:catalog-after",
            ))
            .await
            .unwrap()
            .into_inner();
        let StartOutcome::InputChallenge(replayed_challenge) = replay.outcome.unwrap() else {
            panic!("recorded challenge must replay despite replacement catalog state")
        };
        assert_eq!(
            replayed_challenge.challenge_id,
            first_challenge.challenge_id
        );
        assert_eq!(
            service.backend.preparation_calls.load(Ordering::SeqCst),
            before + 1,
            "replay must not consult the replacement catalog"
        );
    }

    #[tokio::test]
    async fn wp45_guard_three_round_ledger_replays_each_truthful_outcome() {
        let temp = tempfile::tempdir().unwrap();
        let (service, session, prepared) = test_service(&temp).await;
        let scope = scope(&session, "start:three-rounds");
        let fingerprint = [0x41; 32];
        let mut current = challenge(
            service
                .issue_challenge(
                    &session,
                    prepared,
                    scope.clone(),
                    fingerprint,
                    SEMANTIC_PROFILE.to_owned(),
                    8_192,
                    8,
                    200,
                    "correlation:three-rounds",
                )
                .await
                .unwrap(),
        );
        assert_eq!((current.round, current.remaining_rounds), (1, 2));

        for round in 1..=2 {
            let replay = challenge(
                service
                    .replay_start(
                        &session,
                        &scope,
                        fingerprint,
                        200 + i64::from(round),
                        "correlation:three-rounds",
                    )
                    .await
                    .unwrap()
                    .expect("current ledger outcome"),
            );
            assert_eq!(replay.challenge_id, current.challenge_id);
            assert_eq!(replay.daemon_continuation, current.daemon_continuation);
            let outcome = service
                .consume_challenge(
                    &session,
                    continuation(&current, format!("choice:round-{round}")),
                    210 + i64::from(round),
                    "correlation:three-rounds",
                )
                .await
                .unwrap();
            let ChallengeContinuationOutcome::Closed(StartOutcome::InputChallenge(next)) = outcome
            else {
                panic!("round {round} must advance to a new typed challenge")
            };
            assert_eq!(next.round, round + 1);
            assert_eq!(next.remaining_rounds, MAX_CHALLENGE_ROUNDS - next.round);
            assert_ne!(next.daemon_continuation, current.daemon_continuation);
            current = next;
        }

        let replay = challenge(
            service
                .replay_start(
                    &session,
                    &scope,
                    fingerprint,
                    220,
                    "correlation:three-rounds",
                )
                .await
                .unwrap()
                .expect("third-round ledger outcome"),
        );
        assert_eq!(replay.challenge_id, current.challenge_id);
        assert_eq!((replay.round, replay.remaining_rounds), (3, 0));
        let outcome = service
            .consume_challenge(
                &session,
                continuation(&current, "choice:round-3".to_owned()),
                221,
                "correlation:three-rounds",
            )
            .await
            .unwrap();
        let ChallengeContinuationOutcome::Ready(ready) = outcome else {
            panic!("third valid answer must resolve exactly once")
        };
        assert_eq!(ready.prepared.resolved().answers().len(), 3);
        assert_ne!(
            ready.prepared.resolved().canonical_operation(),
            ready
                .prepared
                .resolved()
                .parsed()
                .canonical_bytes
                .as_slice(),
            "accepted operation identity must include all guarded answers"
        );
        assert!(matches!(
            service.starts.lock().await.outcomes.get(&scope),
            Some(StartOutcomeRecord::Pending { fingerprint: stored, .. }) if stored == &fingerprint
        ));
    }

    #[tokio::test]
    async fn wp45_guard_invalid_answer_closes_and_replays_stable_rejection() {
        let temp = tempfile::tempdir().unwrap();
        let (service, session, prepared) = test_service(&temp).await;
        let scope = scope(&session, "start:invalid-answer");
        let fingerprint = [0x42; 32];
        let issued = challenge(
            service
                .issue_challenge(
                    &session,
                    prepared,
                    scope.clone(),
                    fingerprint,
                    SEMANTIC_PROFILE.to_owned(),
                    8_192,
                    8,
                    200,
                    "correlation:invalid-answer",
                )
                .await
                .unwrap(),
        );
        let closed = service
            .consume_challenge(
                &session,
                continuation(&issued, "choice:forged".to_owned()),
                201,
                "correlation:invalid-answer",
            )
            .await
            .unwrap();
        let ChallengeContinuationOutcome::Closed(StartOutcome::ValidationRejection(rejection)) =
            closed
        else {
            panic!("invalid consumed answer must close the ledger")
        };
        assert_eq!(
            rejection.semantic_request_id.as_deref(),
            Some("request:guard-ledger")
        );
        let StartOutcome::ValidationRejection(replay) = service
            .replay_start(
                &session,
                &scope,
                fingerprint,
                202,
                "correlation:invalid-answer",
            )
            .await
            .unwrap()
            .expect("stable rejected start outcome")
        else {
            panic!("identical initial replay must return the stored rejection")
        };
        assert_eq!(replay, rejection);
        let issued_token: [u8; 32] = issued.daemon_continuation.as_slice().try_into().unwrap();
        assert!(
            service
                .starts
                .lock()
                .await
                .challenges
                .get(&issued_token)
                .is_some_and(|record| record.used)
        );
    }

    #[tokio::test]
    async fn wp45_challenge_expiry_and_replay_are_distinct_typed_outcomes() {
        let temp = tempfile::tempdir().unwrap();
        let (service, session, prepared) = test_service(&temp).await;
        let scope = scope(&session, "start:expired-answer");
        let fingerprint = [0x43; 32];
        let issued = challenge(
            service
                .issue_challenge(
                    &session,
                    prepared,
                    scope.clone(),
                    fingerprint,
                    SEMANTIC_PROFILE.to_owned(),
                    8_192,
                    8,
                    200,
                    "correlation:issued",
                )
                .await
                .unwrap(),
        );

        let closed = service
            .consume_challenge(
                &session,
                continuation(&issued, "choice:round-1".to_owned()),
                issued.expires_at_unix_ms,
                "correlation:expired",
            )
            .await
            .unwrap();
        let ChallengeContinuationOutcome::Closed(StartOutcome::ValidationRejection(rejection)) =
            closed
        else {
            panic!("an expired continuation must close as a typed validation rejection")
        };
        let error = rejection.error.as_ref().expect("safe typed error");
        assert_eq!(error.code, SafeErrorCode::ContinuationExpired as i32);
        assert_eq!(error.layer, SafeErrorLayer::Query as i32);
        assert_eq!(
            error.diagnostic_reference,
            Some(SafeDiagnosticReference::QueryChallengeRejected as i32)
        );
        assert_eq!(error.correlation_id, "correlation:expired");

        let replay_status = match service
            .consume_challenge(
                &session,
                continuation(&issued, "choice:round-1".to_owned()),
                issued.expires_at_unix_ms.saturating_add(1),
                "correlation:replayed",
            )
            .await
        {
            Err(status) => status,
            Ok(_) => panic!("a consumed continuation must reject as replay"),
        };
        assert_eq!(replay_status.code(), Code::InvalidArgument);
        assert_eq!(status_public_code(&replay_status), "CHALLENGE_REPLAY");
        let replay_detail = SafeErrorMetadata::decode(
            replay_status
                .metadata()
                .get_bin("codefabric-safe-error-bin")
                .unwrap()
                .to_bytes()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            replay_detail.code,
            SafeErrorCode::ContinuationReplayed as i32
        );

        let StartOutcome::ValidationRejection(replayed_rejection) = service
            .replay_start(
                &session,
                &scope,
                fingerprint,
                issued.expires_at_unix_ms.saturating_sub(1),
                "correlation:fresh-request",
            )
            .await
            .unwrap()
            .expect("closed challenge outcome remains replayable before expiry")
        else {
            panic!("closed challenge must replay its typed rejection")
        };
        assert_eq!(
            replayed_rejection.error.unwrap().correlation_id,
            "correlation:fresh-request",
            "replay must bind the current request correlation, not retain stale wire authority"
        );
    }

    #[tokio::test]
    async fn wp45_challenge_expiry_remains_typed_after_unrelated_prune() {
        let temp = tempfile::tempdir().unwrap();
        let (service, session, prepared) = test_service(&temp).await;
        let original_scope = scope(&session, "start:expired-after-prune");
        let fingerprint = [0x45; 32];
        let issued = challenge(
            service
                .issue_challenge(
                    &session,
                    prepared,
                    original_scope,
                    fingerprint,
                    SEMANTIC_PROFILE.to_owned(),
                    8_192,
                    8,
                    200,
                    "correlation:issued-before-prune",
                )
                .await
                .unwrap(),
        );
        let token: [u8; 32] = issued.daemon_continuation.as_slice().try_into().unwrap();
        let observed_after_expiry = issued.expires_at_unix_ms.saturating_add(1);

        assert!(
            service
                .replay_start(
                    &session,
                    &scope(&session, "start:unrelated-prune"),
                    [0x46; 32],
                    observed_after_expiry,
                    "correlation:unrelated-prune",
                )
                .await
                .unwrap()
                .is_none()
        );
        {
            let starts = service.starts.lock().await;
            assert!(!starts.challenges.contains_key(&token));
            assert!(matches!(
                starts
                    .challenge_tombstones
                    .get(&token)
                    .map(|tombstone| tombstone.disposition),
                Some(ChallengeTombstoneDisposition::Expired)
            ));
            assert!(challenge_record_count(&starts) <= MAX_CHALLENGES);
        }

        let closed = service
            .consume_challenge(
                &session,
                continuation(&issued, "choice:round-1".to_owned()),
                observed_after_expiry,
                "correlation:expired-after-prune",
            )
            .await
            .unwrap();
        let ChallengeContinuationOutcome::Closed(StartOutcome::ValidationRejection(rejection)) =
            closed
        else {
            panic!("the bounded expiry tombstone must preserve the typed rejection")
        };
        assert_eq!(
            rejection.error.unwrap().code,
            SafeErrorCode::ContinuationExpired as i32
        );

        let replay = match service
            .consume_challenge(
                &session,
                continuation(&issued, "choice:round-1".to_owned()),
                observed_after_expiry.saturating_add(1),
                "correlation:replay-after-prune",
            )
            .await
        {
            Err(status) => status,
            Ok(_) => panic!("a consumed expiry tombstone must reject as replay"),
        };
        assert_eq!(status_public_code(&replay), "CHALLENGE_REPLAY");
    }

    #[tokio::test]
    async fn wp45_lazy_service_retention_reclaims_coordinator_authority() {
        let temp = tempfile::tempdir().unwrap();
        let (service, _session, _prepared) = test_service(&temp).await;
        let operation = NormalizedQueryOperation::try_new(NormalizedQueryOperation {
            workspace_id: WorkspaceId::from_bytes([0x22; 16]),
            principal_id: PrincipalId::from_bytes([0x11; 16]),
            policy_generation: 1,
            revocation_generation: 1,
            session_sharing_class: QuerySessionSharingClass::PrincipalBound,
            idempotency_key: Arc::from("semantic:retention"),
            canonical_request: Arc::from(br#"{"kind":"retention"}"#.to_vec()),
            semantic_profile: Arc::from(SEMANTIC_PROFILE),
            request_contract: Arc::from("codefabric.semantic-query-request.v2"),
            response_contract: Arc::from("codefabric.semantic-query-response.v2"),
            delivery_profile: Arc::from("daemon-resource"),
            compression_profile: Arc::from("identity"),
            freshness_policy: Arc::from("fresh"),
            epoch_policy: Arc::from("selected-active-epoch"),
            deadline_unix_ms: 5_000,
            lease_expires_at_unix_ms: 10_000,
            maximum_result_bytes: 1_024,
            maximum_result_pages: 4,
        })
        .unwrap();
        let acceptance = match service.coordinator.accept(operation, 1_000).await.unwrap() {
            QueryAcceptanceOutcome::New(value) | QueryAcceptanceOutcome::Replay(value) => value,
        };
        service.collect_retention(10_000).await.unwrap();
        assert!(matches!(
            service.coordinator.phase(&acceptance.query_id).await,
            Err(QueryCoordinatorError::UnknownQuery(_))
        ));
    }

    fn test_reference_entry(kind: ReferenceKind, released_version: &str) -> LiveReferenceEntry {
        let (public_kind, presentation_key) = reference_kind_projection(kind).unwrap();
        LiveReferenceEntry {
            kind,
            public_kind,
            presentation_key,
            released_version: released_version.to_owned(),
            content: format!(r#"{{"kind":"{public_kind}"}}"#).into_bytes(),
        }
    }

    fn test_reference_projection() -> LiveReferenceProjection {
        LiveReferenceProjection {
            entries: [
                ReferenceKind::Capability,
                ReferenceKind::Guide,
                ReferenceKind::Recipe,
                ReferenceKind::RequestSchema,
                ReferenceKind::ResponseSchema,
                ReferenceKind::Snapshot,
            ]
            .into_iter()
            .map(|kind| (kind, test_reference_entry(kind, "2.3.0")))
            .collect(),
        }
    }

    #[test]
    fn wp45_live_reference_completion_caps_total_and_filters_the_shared_relation() {
        let references = test_reference_projection();
        let capped = reference_completion(
            ReferenceCompletionRequest {
                variable: ReferenceTemplateVariable::Kind as i32,
                prefix: String::new(),
                kind: None,
                selector: None,
                maximum_candidates: 2,
            },
            &references,
        )
        .unwrap();
        assert_eq!(capped.total, 6);
        assert!(capped.has_more);
        assert_eq!(capped.candidates.len(), 2);

        let request_schema = reference_completion(
            ReferenceCompletionRequest {
                variable: ReferenceTemplateVariable::Kind as i32,
                prefix: "req".to_owned(),
                kind: None,
                selector: None,
                maximum_candidates: 100,
            },
            &references,
        )
        .unwrap();
        assert_eq!(request_schema.total, 1);
        assert!(!request_schema.has_more);
        assert_eq!(request_schema.candidates[0].value, "request-schema");

        let version = reference_completion(
            ReferenceCompletionRequest {
                variable: ReferenceTemplateVariable::ReleasedVersion as i32,
                prefix: "2.".to_owned(),
                kind: Some(ReferenceKind::RequestSchema as i32),
                selector: None,
                maximum_candidates: 100,
            },
            &references,
        )
        .unwrap();
        assert_eq!(version.total, 1);
        assert_eq!(version.candidates[0].value, "2.3.0");
    }

    #[test]
    fn wp45_reference_denials_do_not_disclose_selector_existence() {
        let references = test_reference_projection();
        let completion = |selector: &str| {
            reference_completion(
                ReferenceCompletionRequest {
                    variable: ReferenceTemplateVariable::Kind as i32,
                    prefix: String::new(),
                    kind: None,
                    selector: Some(selector.to_owned()),
                    maximum_candidates: 100,
                },
                &references,
            )
            .unwrap()
        };
        assert_eq!(completion("request-schema"), completion("entity-inventory"));
        assert_eq!(completion("request-schema").total, 0);

        let absent_kind = LiveReferenceProjection {
            entries: BTreeMap::from([(
                ReferenceKind::RequestSchema,
                test_reference_entry(ReferenceKind::RequestSchema, "2.3.0"),
            )]),
        }
        .resolve(ReferenceKind::Snapshot, Some("2.3.0"))
        .unwrap_err();
        let denied_version = references
            .resolve(ReferenceKind::RequestSchema, Some("9.9.9"))
            .unwrap_err();
        assert_eq!(absent_kind.code(), Code::PermissionDenied);
        assert_eq!(denied_version.code(), Code::PermissionDenied);
        assert_eq!(
            absent_kind.metadata().get("codefabric-error-code"),
            denied_version.metadata().get("codefabric-error-code")
        );
    }

    #[test]
    fn reference_workspace_scope_requires_exactly_one_authorized_workspace() {
        let workspace = WorkspaceId::from_bytes([0x31; 16]);
        assert_eq!(
            sole_reference_workspace(&BTreeSet::from([workspace])).unwrap(),
            workspace
        );

        for denied in [
            BTreeSet::new(),
            BTreeSet::from([workspace, WorkspaceId::from_bytes([0x32; 16])]),
        ] {
            let status = sole_reference_workspace(&denied).unwrap_err();
            assert_eq!(status.code(), Code::PermissionDenied);
            assert_eq!(
                status.metadata().get("codefabric-error-code").unwrap(),
                "REFERENCE_WORKSPACE_SCOPE"
            );
        }
    }

    #[test]
    fn reference_is_empty_only_during_pre_install_bootstrapping() {
        for phase in [
            ProductionLifecyclePhase::Configured,
            ProductionLifecyclePhase::DaemonLeased,
            ProductionLifecyclePhase::WriterFenced,
            ProductionLifecyclePhase::CommandRecovered,
            ProductionLifecyclePhase::GenesisRequired,
            ProductionLifecyclePhase::SelectedEpochRecovered,
            ProductionLifecyclePhase::EpochBuiltAndProved,
        ] {
            assert!(missing_reference_query_forms(phase).unwrap().is_empty());
        }

        for phase in [
            ProductionLifecyclePhase::WorkspaceInstalledClosed,
            ProductionLifecyclePhase::EndpointsBoundBootstrapping,
            ProductionLifecyclePhase::SoleTargetAuthorityObserved,
            ProductionLifecyclePhase::SoleTargetAuthorityCommitted,
            ProductionLifecyclePhase::Ready,
            ProductionLifecyclePhase::Draining,
            ProductionLifecyclePhase::Stopped,
            ProductionLifecyclePhase::FailedClosed,
        ] {
            let status = missing_reference_query_forms(phase).unwrap_err();
            assert_eq!(status.code(), Code::Internal);
            assert_eq!(
                status.metadata().get("codefabric-error-code").unwrap(),
                "REFERENCE_AUTHORITY"
            );
        }
    }

    #[test]
    fn challenge_round_progression_matches_the_advertised_maximum() {
        assert_eq!(MAX_CHALLENGE_ROUNDS, 3);
        assert_eq!(remaining_challenge_rounds(1), 2);
        assert_eq!(next_challenge_round(1).unwrap(), 2);
        assert_eq!(remaining_challenge_rounds(2), 1);
        assert_eq!(next_challenge_round(2).unwrap(), 3);
        assert_eq!(remaining_challenge_rounds(3), 0);
        let exhausted = next_challenge_round(3).unwrap_err();
        assert_eq!(exhausted.code(), Code::InvalidArgument);
        assert_eq!(
            exhausted.metadata().get("codefabric-error-code").unwrap(),
            "CHALLENGE_ROUND_LIMIT"
        );
        assert!(next_challenge_round(0).is_err());
    }

    #[tokio::test]
    async fn wp45_reserved_control_keeps_cancel_and_release_admissible() {
        let contract = reserved_control_contract();
        assert_eq!(contract.reserved_capacity, 1);
        assert_eq!(
            contract.operations,
            [
                ReservedControlOperation::Handshake,
                ReservedControlOperation::GetStatus,
                ReservedControlOperation::CancelQuery,
                ReservedControlOperation::ReleaseResource,
            ]
            .into_iter()
            .map(|operation| operation as i32)
            .collect::<Vec<_>>()
        );

        let admission = RpcAdmission::new(1, 1);
        let data = admission.data().expect("one data operation admitted");
        let saturated = admission.data().unwrap_err();
        assert_eq!(saturated.code(), Code::ResourceExhausted);
        assert_eq!(status_public_code(&saturated), "RPC_CAPACITY");
        let control = admission
            .control(RpcBudget::from_duration(Duration::from_secs(1)).unwrap())
            .await
            .expect("reserved control is independent of saturated data capacity");

        let waiting_control =
            admission.control(RpcBudget::from_duration(Duration::from_secs(1)).unwrap());
        tokio::pin!(waiting_control);
        assert!(
            tokio::time::timeout(Duration::from_millis(2), waiting_control.as_mut())
                .await
                .is_err(),
            "a second control operation waits instead of racing the reserved slot"
        );
        let overflow = admission
            .control(RpcBudget::from_duration(Duration::from_secs(1)).unwrap())
            .await
            .expect_err("the bounded control-wait queue sheds overflow immediately");
        assert_eq!(overflow.code(), Code::ResourceExhausted);
        assert_eq!(status_public_code(&overflow), "RPC_CAPACITY");
        drop(control);
        let queued_control =
            tokio::time::timeout(Duration::from_millis(50), waiting_control.as_mut())
                .await
                .expect("queued control admission resumes within its budget")
                .expect("queued control admission succeeds");
        drop(queued_control);

        let held_control = admission
            .control(RpcBudget::from_duration(Duration::from_secs(1)).unwrap())
            .await
            .expect("control capacity remains reusable");
        let expired = admission
            .control(RpcBudget::from_duration(Duration::from_millis(2)).unwrap())
            .await
            .expect_err("bounded control wait expires under sustained contention");
        assert_eq!(expired.code(), Code::DeadlineExceeded);
        assert_eq!(status_public_code(&expired), "RPC_BUDGET_EXHAUSTED");

        let replacement_waiter =
            admission.control(RpcBudget::from_duration(Duration::from_secs(1)).unwrap());
        tokio::pin!(replacement_waiter);
        assert!(
            tokio::time::timeout(Duration::from_millis(2), replacement_waiter.as_mut())
                .await
                .is_err(),
            "an expired waiter releases its bounded queue slot"
        );
        drop(held_control);
        let replacement_control =
            tokio::time::timeout(Duration::from_millis(50), replacement_waiter.as_mut())
                .await
                .expect("replacement waiter resumes after the active control operation")
                .expect("replacement waiter is admitted");
        drop(replacement_control);
        drop(data);
        let _data_again = admission
            .data()
            .expect("released data capacity is reusable");
        let _control_again = admission
            .control(RpcBudget::from_duration(Duration::from_secs(1)).unwrap())
            .await
            .expect("released control capacity is reusable");
    }

    #[tokio::test]
    async fn wp45_relative_budget_is_consumed_across_the_operation_lifetime() {
        let budget = RpcBudget::from_duration(Duration::from_millis(5)).unwrap();
        let status = budget
            .run(async {
                tokio::time::sleep(Duration::from_millis(25)).await;
                Ok::<_, Status>(())
            })
            .await
            .unwrap_err();
        assert_eq!(status.code(), Code::DeadlineExceeded);
        assert_eq!(status_public_code(&status), "RPC_BUDGET_EXHAUSTED");
        assert!(matches!(budget.remaining(), Err(error) if error.code() == Code::DeadlineExceeded));
    }

    #[tokio::test]
    async fn wp45_queued_execution_deadline_releases_reservations() {
        let temp = tempfile::tempdir().unwrap();
        let coordinator = test_coordinator(&temp);
        let first = match coordinator
            .accept(coordinator_operation("budget-running", 1), 1_000)
            .await
            .unwrap()
        {
            QueryAcceptanceOutcome::New(value) | QueryAcceptanceOutcome::Replay(value) => value,
        };
        let queued = match coordinator
            .accept(coordinator_operation("budget-queued", 2), 1_001)
            .await
            .unwrap()
        {
            QueryAcceptanceOutcome::New(value) | QueryAcceptanceOutcome::Replay(value) => value,
        };
        assert_eq!(
            coordinator.phase(&first.query_id).await.unwrap(),
            QueryExecutionPhase::Running
        );
        assert_eq!(
            coordinator.phase(&queued.query_id).await.unwrap(),
            QueryExecutionPhase::Queued
        );
        let before = coordinator.snapshot().await;
        assert_eq!((before.queued, before.reserved_result_bytes), (1, 2_048));

        let backend = Arc::new(ThreeRoundBackend::default());
        let parsed = parse_request(TEST_REQUEST).unwrap();
        let answers = (1..=MAX_CHALLENGE_ROUNDS)
            .map(|round| SemanticInputAnswer {
                semantic_field_id: format!("field:round-{round}"),
                value: SemanticInputValue::Choice(format!("choice:round-{round}")),
            })
            .collect();
        let resolved = ResolvedSemanticExecutionRequest::try_new(parsed, answers).unwrap();
        let prepared = backend.admit_execution_request(resolved).unwrap();
        execute_accepted_query(ExecutionTask {
            backend,
            coordinator: Arc::clone(&coordinator),
            results: Arc::new(
                StreamedResultRegistry::try_new(
                    1_024,
                    crate::fabric::workspace_resources::test_workspace_budget(),
                )
                .unwrap(),
            ),
            query_id: queued.query_id.clone(),
            prepared,
            principal_id: PrincipalId::from_bytes([0x11; 16]),
            workspace_id: WorkspaceId::from_bytes([0x22; 16]),
            correlation_id: "correlation:queued-budget".to_owned(),
            deadline: Instant::now() + Duration::from_millis(10),
            daemon_generation: 7,
            policy_generation: 1,
            revocation_generation: 1,
        })
        .await;

        let after = coordinator.snapshot().await;
        assert_eq!((after.running, after.queued), (1, 0));
        assert_eq!(after.reserved_result_bytes, 1_024);
        assert_eq!(after.reserved_result_pages, 4);
        assert_eq!(
            coordinator.phase(&queued.query_id).await.unwrap(),
            QueryExecutionPhase::Terminal(QueryTerminalState::Failed)
        );
        let events = coordinator.events_after(&queued.query_id, 0).await.unwrap();
        assert!(matches!(
            events.last().map(|event| &event.payload),
            Some(QueryControlEventPayload::Terminal {
                state: QueryTerminalState::Failed,
                public_code: Some(code),
            }) if code == "DEADLINE_EXCEEDED"
        ));
    }

    #[tokio::test]
    async fn wp45_watch_iteration_deadline_releases_admission() {
        let (sessions, session, peer) =
            test_session_with_operations(BTreeSet::from([SessionOperation::Watch])).await;
        let temp = tempfile::tempdir().unwrap();
        let admission = RpcAdmission::new(1, 1);
        let (_keep_blocked, blocker) = tokio::sync::oneshot::channel();
        let state = WatchState {
            sessions,
            coordinator: test_coordinator(&temp),
            results: Arc::new(
                StreamedResultRegistry::try_new(
                    1_024,
                    crate::fabric::workspace_resources::test_workspace_budget(),
                )
                .unwrap(),
            ),
            peer,
            session_token: session.token().to_vec(),
            query_id: "query:blocked-watch".to_owned(),
            principal_id: session.principal_id(),
            workspace_id: WorkspaceId::from_bytes([0x22; 16]),
            daemon_generation: session.daemon_generation(),
            policy_generation: session.policy_generation(),
            revocation_generation: session.revocation_generation(),
            cursor_expiry: session.expires_at_unix_ms(),
            authority: authority(&session),
            after_sequence: 0,
            pending: Vec::new(),
            budget: RpcBudget::from_duration(Duration::from_millis(5)).unwrap(),
            admission_permit: Some(
                admission
                    .data()
                    .expect("watch stream is initially admitted"),
            ),
            event_fetch_blocker: Some(blocker),
        };
        let (item, returned) = tokio::time::timeout(Duration::from_millis(250), watch_next(state))
            .await
            .expect("watch state machine must enforce its own operation budget")
            .expect("budget expiry is a terminal stream item");
        let status = match item {
            Ok(_) => panic!("blocked watch unexpectedly produced an event"),
            Err(status) => status,
        };
        assert_eq!(status.code(), Code::DeadlineExceeded);
        assert_eq!(status_public_code(&status), "RPC_BUDGET_EXHAUSTED");
        assert!(returned.admission_permit.is_none());
        let _reused = admission
            .data()
            .expect("a replacement watch stream is admitted after timeout");
    }

    #[tokio::test]
    async fn wp45_read_iteration_deadline_releases_admission() {
        let (sessions, session, peer) =
            test_session_with_operations(BTreeSet::from([SessionOperation::ReadResource])).await;
        let admission = RpcAdmission::new(1, 1);
        let (_keep_blocked, blocker) = tokio::sync::oneshot::channel();
        let state = ReadState {
            sessions,
            results: Arc::new(
                StreamedResultRegistry::try_new(
                    1_024,
                    crate::fabric::workspace_resources::test_workspace_budget(),
                )
                .unwrap(),
            ),
            peer,
            session_token: session.token().to_vec(),
            principal_id: session.principal_id(),
            workspace_ids: Arc::new(session.workspace_ids().clone()),
            daemon_generation: session.daemon_generation(),
            authority: authority(&session),
            public_handle: "resource:blocked-read".to_owned(),
            selector: StreamedResourceSelector::Manifest,
            offset: 0,
            maximum_bytes: 1,
            done: false,
            budget: RpcBudget::from_duration(Duration::from_millis(5)).unwrap(),
            admission_permit: Some(admission.data().expect("read stream is initially admitted")),
            resource_read_blocker: Some(blocker),
        };
        let (item, returned) = tokio::time::timeout(Duration::from_millis(250), read_next(state))
            .await
            .expect("read state machine must enforce its own operation budget")
            .expect("budget expiry is a terminal stream item");
        let status = match item {
            Ok(_) => panic!("blocked resource read unexpectedly produced a chunk"),
            Err(status) => status,
        };
        assert_eq!(status.code(), Code::DeadlineExceeded);
        assert_eq!(status_public_code(&status), "RPC_BUDGET_EXHAUSTED");
        assert!(returned.admission_permit.is_none());
        let _reused = admission
            .data()
            .expect("a replacement resource stream is admitted after timeout");
    }

    #[test]
    fn source_hard_limit_is_typed_and_not_retryable() {
        let status = semantic_status(SemanticQueryError::Phase {
            code: "QUERY_HARD_LIMIT_EXCEEDED",
            phase: "physical_execution",
            pointer: "/runtime/source_context".to_owned(),
            message: "private cause".to_owned(),
        });
        assert_eq!(status.code(), Code::ResourceExhausted);
        let detail = SafeErrorMetadata::decode(
            status
                .metadata()
                .get_bin("codefabric-safe-error-bin")
                .unwrap()
                .to_bytes()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(detail.code, SafeErrorCode::QueryHardLimitExceeded as i32);
        assert_eq!(detail.layer, SafeErrorLayer::Query as i32);
        assert!(!detail.retryable);
        assert!(!status.message().contains("private cause"));
    }

    #[test]
    fn wp45_safe_metadata_and_keys_are_typed_and_control_character_free() {
        for hostile in [
            "correlation\nforged",
            "correlation\rforged",
            "correlation forged",
        ] {
            assert!(!safe_key(hostile, 128));
        }
        assert!(safe_key("correlation:valid-1", 128));

        let status = public_status(Code::ResourceExhausted, "RPC_CAPACITY");
        let detail = SafeErrorMetadata::decode(
            status
                .metadata()
                .get_bin("codefabric-safe-error-bin")
                .unwrap()
                .to_bytes()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(detail.code, SafeErrorCode::CapacityUnavailable as i32);
        assert_eq!(detail.layer, SafeErrorLayer::Transport as i32);
        assert!(detail.retryable);
        assert!(detail.diagnostic_reference.is_none());
        assert!(detail.correlation_id.is_empty());
    }

    #[test]
    fn wp48_resource_bound_and_extent_failures_keep_distinct_typed_authority() {
        let invalid_bound = result_status(StreamedResultRegistryError::InvalidChunkBound);
        assert_eq!(invalid_bound.code(), Code::InvalidArgument);
        assert_eq!(status_public_code(&invalid_bound), "RESOURCE_BOUND");
        let invalid_bound_detail = SafeErrorMetadata::decode(
            invalid_bound
                .metadata()
                .get_bin("codefabric-safe-error-bin")
                .unwrap()
                .to_bytes()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            invalid_bound_detail.code,
            SafeErrorCode::InvalidRequest as i32
        );
        assert_eq!(invalid_bound_detail.layer, SafeErrorLayer::Resource as i32);
        assert!(!invalid_bound_detail.retryable);

        for error in [
            StreamedResultRegistryError::RangeOutsideResource,
            StreamedResultRegistryError::RangeOverflow,
        ] {
            let outside_extent = result_status(error);
            assert_eq!(outside_extent.code(), Code::OutOfRange);
            assert_eq!(status_public_code(&outside_extent), "RESOURCE_RANGE");
            let outside_extent_detail = SafeErrorMetadata::decode(
                outside_extent
                    .metadata()
                    .get_bin("codefabric-safe-error-bin")
                    .unwrap()
                    .to_bytes()
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(
                outside_extent_detail.code,
                SafeErrorCode::RangeNotSatisfiable as i32
            );
            assert_eq!(outside_extent_detail.layer, SafeErrorLayer::Resource as i32);
            assert!(!outside_extent_detail.retryable);
        }
    }

    #[test]
    fn wp48_semantic_request_capacity_is_distinct_from_transport_and_schema_failure() {
        let status = semantic_status(SemanticQueryError::RequestTooLarge);
        assert_eq!(status.code(), Code::ResourceExhausted);
        assert_eq!(status_public_code(&status), "SEMANTIC_REQUEST_CAPACITY");
        let detail = SafeErrorMetadata::decode(
            status
                .metadata()
                .get_bin("codefabric-safe-error-bin")
                .unwrap()
                .to_bytes()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(detail.code, SafeErrorCode::CapacityUnavailable as i32);
        assert_eq!(detail.layer, SafeErrorLayer::Validation as i32);
        assert!(detail.retryable);
    }

    #[test]
    fn wp45_challenge_output_is_bounded_after_typed_projection() {
        let choices = (0..MAX_CHALLENGE_CHOICES)
            .map(|index| SemanticAuthorizedChoice {
                choice_id: format!("choice:{index}"),
                presentation_key: format!("choice.{index}"),
                value: SemanticInputValue::String("x".repeat(MAX_CHALLENGE_STRING_BYTES)),
            })
            .collect();
        let requirement = SemanticInputRequirement {
            semantic_field_id: "field:oversized".to_owned(),
            input_kind: SemanticInputKind::Enum,
            presentation_key: "input.oversized".to_owned(),
            description_key: None,
            required: true,
            constraints: Some(SemanticInputConstraints::Enum {
                minimum_selections: 1,
                maximum_selections: 1,
            }),
            authorized_choices: choices,
        };
        let status = validate_requirements(&[requirement]).unwrap_err();
        assert_eq!(status.code(), Code::ResourceExhausted);
        assert_eq!(status_public_code(&status), "CHALLENGE_REQUIREMENTS");
    }
}
