//! Compatibility-probe support for exercising the public v2 service over a real UDS.
//!
//! This module is compiled only by the explicit compatibility-probe feature. It composes the
//! production handler, session authority, coordinator, result registry, and lifecycle while a
//! deliberately tiny semantic backend makes guarded rounds and cancellation deterministic.

use std::collections::{BTreeSet, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use arrow::array::{ArrayRef, Int64Array};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use async_trait::async_trait;
use datafusion::physical_plan::stream::RecordBatchStreamAdapter;
use futures::stream;
use object_store::memory::InMemory;

use crate::cancellation::Cancellation;
use crate::fabric::arrow_result_resource::{
    QueryExecutionPin, ResultCoverage, ResultResourceLease,
};
use crate::fabric::command::{EpochId, LeaseId, PrincipalId, WorkspaceId};
use crate::fabric::production_kernel::{
    LifecycleAuthority, ProductionLifecyclePhase, WorkspaceSlotRegistry,
};
use crate::fabric::query_coordinator::{
    QueryControlEvent, QueryControlEventPayload, QueryCoordinator, QueryCoordinatorPolicy,
    QueryCoordinatorSnapshot, QueryExecutionPhase, QueryTerminalState,
    SqliteQueryCoordinatorJournal,
};
use crate::fabric::streamed_result_package::{
    ObjectStoreResultSink, PendingResultObjectSet, ResultObjectSink, ResultProvenance,
    ResultPublicationIntentError, ResultPublicationIntentRecorder, StreamedRelationInput,
    StreamedResultPackageBuilder, StreamedResultPackageLimits,
};
use crate::fabric::streamed_result_registry::{
    ReferenceResourcePublication, ReferenceResourceRegistration, StreamedReferenceSelector,
    StreamedResultRegistration, StreamedResultRegistry,
};
use crate::identity::{IdentityDomain, encode_public_id};
use crate::query_backend::{
    PreparedSemanticExecution, ResolvedSemanticExecutionRequest, SemanticAuthorizedChoice,
    SemanticBackendExecutionContext, SemanticBackendOutcome, SemanticExecutionPreparation,
    SemanticInputAnswer, SemanticInputConstraints, SemanticInputKind, SemanticInputRequirement,
    SemanticInputValue, SemanticQueryBackend,
};
use crate::query_service::ProductionQueryService;
use crate::relational_program::RelationId;
use crate::semantic_query_contract::{
    ParsedSemanticRequest, SemanticQueryError, SemanticSnapshotResponse,
};
use crate::session_authority::{
    LaunchGrantAuthority, LaunchPolicyId, LaunchPolicyRevision, RegisteredLaunchGrant,
    RevocationGeneration, SessionOperation,
};

pub const INTEROP_DAEMON_GENERATION: u64 = 7;
pub const INTEROP_SUPERVISOR_GENERATION: u64 = 9;
pub const INTEROP_POLICY_GENERATION: u64 = 8;
pub const INTEROP_REVOCATION_GENERATION: u64 = 9;
pub const INTEROP_PRINCIPAL: PrincipalId = PrincipalId::from_bytes([0x11; 16]);
pub const INTEROP_WORKSPACE: WorkspaceId = WorkspaceId::from_bytes([0x22; 16]);
pub const INTEROP_SEMANTIC_PROFILE: &str = "codefabric.semantic-query.v2";
/// One effective transport/registry chunk capability shared by the production interop fixture.
pub const INTEROP_MAXIMUM_RESOURCE_CHUNK_BYTES: u64 = 64 * 1_024;

/// Exact production-service specialization used by the cross-language wire oracle.
pub type InteropProductionQueryService = ProductionQueryService<InteropSemanticBackend>;

/// Deterministic backend used only to make transport-visible guarded and cancellation paths race
/// free. The production service still owns parsing, challenge sealing, admission, events,
/// resources, authorization, and cancellation.
#[derive(Debug)]
pub struct InteropSemanticBackend {
    package_builder: StreamedResultPackageBuilder,
    observed_queries: std::sync::Mutex<Vec<String>>,
    publication_injected: std::sync::Mutex<HashSet<String>>,
}

impl InteropSemanticBackend {
    fn new(package_builder: StreamedResultPackageBuilder) -> Self {
        Self {
            package_builder,
            observed_queries: std::sync::Mutex::new(Vec::new()),
            publication_injected: std::sync::Mutex::new(HashSet::new()),
        }
    }

    fn observed_queries(&self) -> Vec<String> {
        self.observed_queries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn mark_publication_injected(&self, query_id: &str) {
        self.publication_injected
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(query_id.to_owned());
    }

    fn publication_was_injected(&self, query_id: &str) -> bool {
        self.publication_injected
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(query_id)
    }
}

#[async_trait]
impl SemanticQueryBackend for InteropSemanticBackend {
    type ExecutionAuthority = ();

    fn retained_package_builder(&self) -> Option<StreamedResultPackageBuilder> {
        Some(self.package_builder.clone())
    }

    fn validate_execution_request(
        &self,
        _request: &ParsedSemanticRequest,
    ) -> Result<(), SemanticQueryError> {
        Ok(())
    }

    fn prepare_execution_request(
        &self,
        request: &ParsedSemanticRequest,
        answers: &[SemanticInputAnswer],
    ) -> Result<SemanticExecutionPreparation, SemanticQueryError> {
        for (index, answer) in answers.iter().enumerate() {
            let round = index + 1;
            if answer.semantic_field_id != format!("field:round-{round}")
                || answer.value != SemanticInputValue::Choice(format!("choice:round-{round}"))
            {
                return Err(SemanticQueryError::Invalid(
                    "interop guarded input does not match its authorized choice".to_owned(),
                ));
            }
        }
        if answers.len() < 3 {
            let round = answers.len() + 1;
            return Ok(SemanticExecutionPreparation::InputRequired(vec![
                SemanticInputRequirement {
                    semantic_field_id: format!("field:round-{round}"),
                    input_kind: SemanticInputKind::Enum,
                    presentation_key: format!("interop.input.round-{round}"),
                    description_key: Some("interop.input.description".to_owned()),
                    required: true,
                    constraints: Some(SemanticInputConstraints::Enum {
                        minimum_selections: 1,
                        maximum_selections: 1,
                    }),
                    authorized_choices: vec![SemanticAuthorizedChoice {
                        choice_id: format!("choice:round-{round}"),
                        presentation_key: format!("interop.choice.round-{round}"),
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
    ) -> Result<PreparedSemanticExecution<Self::ExecutionAuthority>, SemanticQueryError> {
        let workspace_id = resolved.parsed().request.workspace_id.clone();
        Ok(PreparedSemanticExecution::new(
            resolved,
            (),
            SemanticSnapshotResponse {
                snapshot_id: "epoch:interop-production-handler".to_owned(),
                workspace_id,
                repository_id: None,
                worktree_id: None,
                source_generation: 11,
                source_inventory_digest: format!("b3:{}", "11".repeat(32)),
                durable_base_publication: "delta:interop@11".to_owned(),
                base_table_version_digest: format!("b3:{}", "22".repeat(32)),
                overlay_generation: 3,
                overlay_checksum: format!("b3:{}", "33".repeat(32)),
                analysis_context_set_id: "analysis:interop".to_owned(),
                analysis_context_ids: vec!["analysis:interop".to_owned()],
                freshness_state: crate::registries::FreshnessState::Current,
                source_trust_state: "EXACT_TYPED_INPUTS".to_owned(),
                event_stream_health: "ACTIVATION_CHAIN_SELECTED".to_owned(),
                git_acceleration_status: "NON_AUTHORITY".to_owned(),
                git_operation_summary: None,
                pending_update_count: 0,
                ontology_version: "not-applicable-programmatic-authority".to_owned(),
                schema_bundle_version: "schema:interop".to_owned(),
                provider_bundle_version: "provider:interop".to_owned(),
                derivation_bundle_version: "derivation:interop".to_owned(),
                query_language_version: "2.0".to_owned(),
                capability_summaries: Vec::new(),
                diagnostic_references: Vec::new(),
            },
        ))
    }

    async fn execute(
        &self,
        prepared: PreparedSemanticExecution<Self::ExecutionAuthority>,
        _freshness: crate::freshness::FreshnessState,
        cancellation: Cancellation,
        context: SemanticBackendExecutionContext,
        artifacts: crate::fabric::QueryExecutionArtifactAccumulator,
    ) -> SemanticBackendOutcome {
        let query_id = context.execution().execution_id.clone();
        self.observed_queries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(query_id.clone());
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if cancellation.is_cancelled() {
                return SemanticBackendOutcome::Cancelled {
                    error: SemanticQueryError::Phase {
                        code: "CANCELLED",
                        phase: "execution",
                        pointer: "query".to_owned(),
                        message: "interop cancellation observed".to_owned(),
                    },
                    evidence: artifacts.snapshot(),
                };
            }
            if self.publication_was_injected(&query_id) {
                return SemanticBackendOutcome::Failed {
                    error: SemanticQueryError::Phase {
                        code: "INTEROP_PUBLICATION_COMPLETE",
                        phase: "execution",
                        pointer: "query".to_owned(),
                        message: "the compatibility harness already closed publication".to_owned(),
                    },
                    evidence: artifacts.snapshot(),
                };
            }
            if Instant::now() >= deadline {
                return SemanticBackendOutcome::Failed {
                    error: SemanticQueryError::Phase {
                        code: "INTEROP_TIMEOUT",
                        phase: "execution",
                        pointer: "query".to_owned(),
                        message: "interop execution was not closed".to_owned(),
                    },
                    evidence: artifacts.snapshot(),
                };
            }
            let _ = &prepared;
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}

/// Retained controller for state transitions and deterministic package publication while the
/// production service itself is owned by Tonic.
#[derive(Clone, Debug)]
pub struct ProductionRpcInteropControl {
    lifecycle: Arc<LifecycleAuthority>,
    coordinator: Arc<QueryCoordinator>,
    results: Arc<StreamedResultRegistry>,
    backend: Arc<InteropSemanticBackend>,
    package_builder: StreamedResultPackageBuilder,
}

impl ProductionRpcInteropControl {
    /// Drive the production lifecycle through every legal predecessor into `Ready`.
    pub fn mark_ready(&self) {
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
            self.lifecycle
                .advance(expected, next)
                .expect("interop lifecycle transition");
        }
    }

    /// Wait until the backend has observed at least `count` production execution tasks.
    pub async fn wait_for_query_count(&self, count: usize) -> Vec<String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let observed = self.backend.observed_queries();
            if observed.len() >= count {
                return observed;
            }
            assert!(Instant::now() < deadline, "interop query start deadline");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    /// Seed one daemon-minted reference handle through the same registry used by the service.
    pub async fn seed_reference(&self) -> (ReferenceResourceRegistration, Vec<u8>) {
        let content = br#"{"interop_reference":"guide"}"#.to_vec();
        self.publish_reference(content, "reference:interop-guide", "2.3")
            .await
    }

    /// Seed a maximum-sized deterministic reference for real transport flow-control probes.
    pub async fn seed_bounded_reference(
        &self,
        byte_length: usize,
    ) -> (ReferenceResourceRegistration, Vec<u8>) {
        assert!(
            (1..=1024 * 1024).contains(&byte_length),
            "interop reference must remain within the production registry bound"
        );
        let content = (0..byte_length)
            .map(|offset| u8::try_from(offset % 251).expect("bounded reference byte"))
            .collect::<Vec<_>>();
        self.publish_reference(content, "reference:interop-bounded", "2.3-bounded")
            .await
    }

    async fn publish_reference(
        &self,
        content: Vec<u8>,
        reference_id: &str,
        version: &str,
    ) -> (ReferenceResourceRegistration, Vec<u8>) {
        let now = now_millis();
        let registration = self
            .results
            .publish_reference(ReferenceResourcePublication {
                principal_id: INTEROP_PRINCIPAL,
                workspace_id: INTEROP_WORKSPACE,
                daemon_generation: INTEROP_DAEMON_GENERATION,
                policy_generation: INTEROP_POLICY_GENERATION,
                revocation_generation: INTEROP_REVOCATION_GENERATION,
                selector: StreamedReferenceSelector {
                    kind: crate::rpc::generated::codefabric::cpgd::v2::ReferenceKind::Guide
                        .as_str_name()
                        .to_owned(),
                    version: Some(version.to_owned()),
                },
                reference_id: reference_id.to_owned(),
                media_type: "application/json".to_owned(),
                content: content.clone(),
                issued_at_unix_ms: now,
                expires_at_unix_ms: now.saturating_add(60_000),
            })
            .await
            .expect("interop reference publication");
        (registration, content)
    }

    /// Return the enforced durable event bound used by the production coordinator.
    #[must_use]
    pub fn maximum_events_per_query(&self) -> usize {
        self.coordinator.maximum_events_per_query()
    }

    /// Observe bounded coordinator counters without reaching into transport state.
    pub async fn coordinator_snapshot(&self) -> QueryCoordinatorSnapshot {
        self.coordinator.snapshot().await
    }

    /// Observe the exact public event suffix retained for one interop query.
    pub async fn query_events(&self, query_id: &str) -> Vec<QueryControlEvent> {
        self.coordinator
            .events_after(query_id, 0)
            .await
            .expect("interop query events")
    }

    /// Observe the durable execution phase used to distinguish stream drop from cancellation.
    pub async fn query_phase(&self, query_id: &str) -> QueryExecutionPhase {
        self.coordinator
            .phase(query_id)
            .await
            .expect("interop query phase")
    }

    /// Observe the complete daemon-owned task scope after cancellation or drain.
    pub async fn owned_task_count(&self) -> usize {
        self.coordinator
            .owned_task_count()
            .await
            .expect("interop owned task observation")
    }

    /// Seal and register one real manifest-last result, append the production `ResultReady` event,
    /// and durably close the query as succeeded.
    pub async fn publish_result(&self, query_id: &str) -> StreamedResultRegistration {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if matches!(
                self.coordinator.phase(query_id).await,
                Ok(QueryExecutionPhase::Running)
            ) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "interop query never became running"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let schema = Arc::new(Schema::new(vec![Field::new(
            "value",
            DataType::Int64,
            false,
        )]));
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![Arc::new(Int64Array::from(vec![7_i64, 11, 13])) as ArrayRef],
        )
        .expect("interop Arrow batch");
        let relation = StreamedRelationInput {
            relation_id: RelationId::new("query.result.interop.v1").expect("relation id"),
            schema: Arc::clone(&schema),
            stream: Box::pin(RecordBatchStreamAdapter::new(
                schema,
                stream::iter(vec![Ok(batch)]),
            )),
            max_rows: 3,
            row_selection: None,
            coverage: ResultCoverage::complete(3),
            provenance: vec![ResultProvenance {
                kind: "compatibility_oracle".to_owned(),
                identity: "wp45.production-uds".to_owned(),
            }],
        };
        let now = now_millis();
        let query_pin =
            QueryExecutionPin::from_bytes(*blake3::hash(query_id.as_bytes()).as_bytes());
        let intent = CoordinatorPublicationIntent {
            coordinator: Arc::clone(&self.coordinator),
            query_id: query_id.to_owned(),
        };
        let package = self
            .package_builder
            .seal(
                EpochId::from_bytes([0x44; 16]),
                query_pin,
                br#"{"status":"ok"}"#,
                vec![relation],
                ResultResourceLease::try_new(
                    LeaseId::from_bytes([0x45; 16]),
                    now,
                    now.saturating_add(60_000),
                )
                .expect("interop result lease"),
                &Cancellation::default(),
                Instant::now() + Duration::from_secs(10),
                &intent,
            )
            .await
            .expect("seal interop result package");
        let registration = self
            .results
            .publish_sealed_package_for_interop(
                query_id,
                INTEROP_PRINCIPAL,
                INTEROP_WORKSPACE,
                INTEROP_DAEMON_GENERATION,
                INTEROP_POLICY_GENERATION,
                INTEROP_REVOCATION_GENERATION,
                package,
            )
            .await
            .expect("register interop result package");
        self.coordinator
            .append_event(
                query_id,
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
            .await
            .expect("append interop ResultReady");
        self.coordinator
            .terminal(
                query_id,
                QueryTerminalState::Succeeded,
                None,
                Some((registration.total_bytes, registration.total_pages)),
                now_millis(),
            )
            .await
            .expect("close interop result query");
        self.backend.mark_publication_injected(query_id);
        registration
    }
}

#[derive(Debug)]
struct CoordinatorPublicationIntent {
    coordinator: Arc<QueryCoordinator>,
    query_id: String,
}

#[async_trait]
impl ResultPublicationIntentRecorder for CoordinatorPublicationIntent {
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

/// Compose one actual production handler and its retained test controller.
pub async fn production_rpc_interop_fixture(
    journal_path: &Path,
    peer_uid: u32,
    peer_pid: u32,
    launch_grant: [u8; 32],
) -> (InteropProductionQueryService, ProductionRpcInteropControl) {
    let lifecycle = Arc::new(LifecycleAuthority::new());
    let sink: Arc<dyn ResultObjectSink> =
        Arc::new(ObjectStoreResultSink::new(Arc::new(InMemory::new())));
    let limits = StreamedResultPackageLimits::try_new(
        4,
        16,
        64,
        1 << 20,
        1_024,
        8 << 20,
        1 << 20,
        32,
        64 * 1_024,
    )
    .expect("interop package limits");
    let fixture_process = crate::resource_budget::ResourceBudget::try_process(
        [111; 16],
        crate::fabric::workspace_resources::local_resource_policy(),
    )
    .expect("interop process resource policy");
    let fixture_resources = fixture_process
        .workspace(
            *INTEROP_WORKSPACE.as_bytes(),
            crate::fabric::workspace_resources::local_resource_policy(),
        )
        .expect("interop workspace resource owner");
    let package_builder =
        StreamedResultPackageBuilder::new(sink, limits, fixture_resources.clone());
    let backend = Arc::new(InteropSemanticBackend::new(package_builder.clone()));
    let coordinator_policy =
        QueryCoordinatorPolicy::try_new(2, 2, 2, 4, 4, 32, 64 * 1_024, 64 << 20, 1_024, 16)
            .expect("interop coordinator policy");
    let journal = Arc::new(
        SqliteQueryCoordinatorJournal::open(journal_path).expect("interop coordinator journal"),
    );
    let coordinator = Arc::new(
        QueryCoordinator::try_new(
            coordinator_policy,
            INTEROP_DAEMON_GENERATION,
            [0x71; 32],
            journal,
            now_millis(),
            fixture_resources.clone(),
        )
        .expect("interop coordinator"),
    );
    let sessions = Arc::new(
        LaunchGrantAuthority::try_new(
            INTEROP_DAEMON_GENERATION,
            INTEROP_SUPERVISOR_GENERATION,
            8,
            8,
        )
        .expect("interop session authority"),
    );
    let now = now_millis();
    sessions
        .register(RegisteredLaunchGrant {
            grant_id: format!("launch:interop:{}", hex(&launch_grant[..4])),
            grant_digest: *blake3::hash(&launch_grant).as_bytes(),
            policy_id: LaunchPolicyId::try_new("policy:interop").expect("interop policy ID"),
            policy_revision: LaunchPolicyRevision::new(INTEROP_POLICY_GENERATION)
                .expect("interop policy generation"),
            revocation_generation: RevocationGeneration::new(INTEROP_REVOCATION_GENERATION)
                .expect("interop revocation generation"),
            principal_id: *INTEROP_PRINCIPAL.as_bytes(),
            workspace_ids: vec![*INTEROP_WORKSPACE.as_bytes()],
            operations: BTreeSet::from([
                SessionOperation::Status,
                SessionOperation::Reference,
                SessionOperation::Validate,
                SessionOperation::Start,
                SessionOperation::Watch,
                SessionOperation::Cancel,
                SessionOperation::ReadResource,
                SessionOperation::ReleaseResource,
            ]),
            semantic_profiles: BTreeSet::from([INTEROP_SEMANTIC_PROFILE.to_owned()]),
            maximum_resource_chunk_bytes: INTEROP_MAXIMUM_RESOURCE_CHUNK_BYTES,
            maximum_result_bytes: 8 << 20,
            maximum_result_pages: 16,
            maximum_request_state_ttl_seconds: 30,
            issued_at_unix_ms: now.saturating_sub(1_000),
            expires_at_unix_ms: now.saturating_add(120_000),
            daemon_generation: INTEROP_DAEMON_GENERATION,
            supervisor_generation: INTEROP_SUPERVISOR_GENERATION,
            peer_uid,
            peer_pid: Some(peer_pid),
            peer_start_identity: crate::session_authority::observed_process_start_identity(
                peer_pid,
            ),
        })
        .await
        .expect("register interop launch grant");
    let results = Arc::new(
        StreamedResultRegistry::try_new(
            usize::try_from(INTEROP_MAXIMUM_RESOURCE_CHUNK_BYTES)
                .expect("interop resource chunk bound fits usize"),
            fixture_resources,
        )
        .expect("interop result registry"),
    );
    let release = Arc::new(
        crate::semantic_release::compile_current_v23_release(
            crate::production_provider_recipe::current_v23_provider_program_definition()
                .expect("interop provider definition"),
        )
        .expect("interop compiled release"),
    );
    let service = ProductionQueryService::try_new(
        release,
        Arc::clone(&backend),
        Arc::clone(&lifecycle),
        Arc::new(WorkspaceSlotRegistry::new()),
        Arc::clone(&coordinator),
        sessions,
        Arc::clone(&results),
        "daemon:interop-production-handler",
    )
    .expect("interop backend and application service use one release");
    let control = ProductionRpcInteropControl {
        lifecycle,
        coordinator,
        results,
        backend,
        package_builder,
    };
    (service, control)
}

#[must_use]
pub fn interop_workspace_public_id() -> String {
    encode_public_id(
        IdentityDomain::Workspace,
        None,
        *INTEROP_WORKSPACE.as_bytes(),
    )
    .expect("interop workspace public identity")
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
        })
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
