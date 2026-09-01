//! Production-composition vertical for the programmatic workspace runtime.
//!
//! The fixture provisions real Delta histories and private SQLite stores. Test-local ports are
//! limited to explicit policy/diagnostic relation readers; the workspace factory, command runtime,
//! activation effect/router, query runtime, and restart reconstruction are production types.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::num::NonZeroUsize;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use arrow_array::{RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use async_trait::async_trait;
use datafusion::catalog::MemTable;
use datafusion::common::TableReference;
use deltalake::DeltaTableBuilder;
use tempfile::TempDir;
use url::Url;

use super::activation::{
    ActivationAttempt, ActivationCommit, ActivationEvent, ActivationEventId, ActivationOrdinal,
    ActivationReadbackRef, BackendCommitRef, CompatibilityClassRef, FabricEpochPins,
    OverlaySegmentSetRef, PolicySetRef,
};
use super::activation_control_delta::{
    ActivationControlDeltaProvider, DeltaActivationRuntimeAuthority,
    provision_activation_control_history,
};
use super::activation_transaction::ExactDeltaProgrammaticEpochRebuilder;
use super::arrow_result_resource::ArrowResultResourceLimits;
use super::child_session::resource_governance::{
    EpochResourceCoordinator, EpochResourcePolicy, EpochWorkClass, EpochWorkClassPolicy,
};
use super::child_session::{ChildRegistryAllowlist, ChildResourceLimits, ChildTableGrant};
use super::command::{
    ActorId, AuthorizationDecision, AuthorizationRef, CommandIdentity, CommandOwnership,
    CommandPins, CommandResult, CommandStateKind, DiagnosticRef, DurableCommandState, EpochId,
    ExecutionOwner, ExpectedHead, FabricCommand, FabricCommandPayload, IdempotencyKey,
    InputReleaseRef, LeaseId, OperationId, OperationSelectionRef, PrincipalId, ProgramReleaseRef,
    ProofReceiptRef, ProviderSetRef, ResourceEnvelopeRef, RetentionPolicyRef, SourceGeneration,
    SourceImageSetRef, TransactionRef, UnknownCommitReason, WorkspaceId, WriterFence,
};
use super::command_actor::{CommandPortError, FabricCommandActorConfig};
use super::command_record_sqlite::CommandRecoveryPageSize;
use super::command_runtime::FabricCommandRuntimeConfig;
use super::command_runtime_ports::{
    CommandAuthorizationPort, InterruptedCommitDiagnosticQuery,
    InterruptedCommitDiagnosticRelationPort,
};
use super::delta_cdf_checkpoint_sqlite::SqliteDeltaCdfCheckpointStore;
use super::delta_guarded_maintenance::{
    DeltaMaintenanceAuthorityError, DeltaMaintenanceSafetyEvidence, DeltaMaintenanceSafetyPort,
};
use super::delta_semantic_read::{
    ExactDeltaSemanticReadError, ExactDeltaSemanticReadRequest, prepare_exact_delta_semantic_read,
};
use super::delta_write::ControlledDeltaWriteOutcome;
use super::epoch_runtime::{FABRIC_CATALOG, FabricEpochRuntimeConfig, FabricSchemaRole};
use super::production_kernel::{ActiveWorkspaceError, WorkspaceSlot};
use super::programmatic_activation_admission::ExactProgrammaticSuccessorQueryAuthorityRecipe;
use super::programmatic_activation_command_ports::{
    ActivationCommandRequestKey, ActivationCommandRequestMaterial,
};
use super::programmatic_activation_command_sqlite::{
    ActivationReconciliationIdentityPolicy, ExactDeltaActivationCommandCandidateRebuilder,
    ExactProgrammaticActivationProofAuthority, SqliteProgrammaticActivationCommandStateStore,
};
use super::programmatic_command_capability::ProgrammaticCommandCapabilityDisposition;
use super::programmatic_command_runtime_factory::{
    ExactProgrammaticCommandEffectClosure, ExactProgrammaticCommandRuntimePartsFactory,
    ProgrammaticActivationCommandEffects, ProgrammaticCommandCapabilityGapInput,
    ProgrammaticCommandRuntimeAuthorityBinding, ProgrammaticNonActivationCommandEffects,
};
use super::programmatic_delta_runtime::ProgrammaticDeltaRuntimePorts;
use super::programmatic_epoch::{ProgrammaticFabricEpoch, ProgrammaticFabricEpochBuilder};
use super::programmatic_ingress_port::ApplicationOwnedSemanticIngressPort;
use super::programmatic_observation_delta::ProgrammaticObservationWriteIdentity;
use super::programmatic_query_backend::{
    ExactProgrammaticSnapshotProjection, ProgrammaticScopeAuthorizationPort,
    ProgrammaticSemanticIngressPort, ProgrammaticSemanticQueryBackend,
    ProgrammaticSemanticQueryPorts, ReleasedV13ProgrammaticScopeAuthorization,
};
use super::programmatic_schema::{
    DEPENDENCY_OBSERVATION_RELATION_ID, FIELD_OBSERVATION_RELATION_ID,
    PROVENANCE_OBSERVATION_RELATION_ID, ProgrammaticRelationId, ProviderInput,
    RELATION_OBSERVATION_RELATION_ID, SCHEMA_OBSERVATION_RELATION_ID,
};
use super::programmatic_workspace::{
    ProgrammaticDaemonComposition, ProgrammaticWorkspaceConstruction,
    ProgrammaticWorkspaceReleasePins, ProgrammaticWorkspaceRuntime,
    ProgrammaticWorkspaceRuntimeFactory, WorkspaceEpochQueryAuthority,
    programmatic_fabric_epoch_authority_pin,
};
use super::proof::{
    CandidateProofInput, CapabilityId, CapabilityOracleRequirement, CapabilityRequest,
    CausalFaultExecution, CausalFaultId, CausalFaultOutcome, CausalFaultProgramRef,
    CoverageScopeId, ExpectationId, IndependentEvidenceAuthority, IndependentProofInput,
    OracleExecution, OracleId, OracleImplementationRef, OracleRequest, ProofCandidatePins,
    ProofOwnerId, ProofProvenanceEdge, ProofRelationId, ProofRunId, ProofTerminalStatus,
    ProofViolation, ProofViolationKind, ProvenanceSubject, RequiredCausalEffect,
    RequiredCausalFault, SemanticClaimRef, SemanticExpectation, SourceAnchorRef, ViolationId,
    evaluate_candidate_proof,
};
use super::published_arrow_result::{PublishedArrowResultRegistry, PublishedResultOwner};
use super::relational_query_runtime::{RelationalQueryAuthorization, RelationalQueryPublication};
use super::request_owned_relation::RequestOwnedRelationLimits;
use super::writer_generation_sqlite::SqliteWriterGenerationStore;
use super::writer_lease::DurableWriterGenerationPort as _;
use super::{QueryExecutionArtifactAccumulator, QueryExecutionContext};
use crate::cancellation::Cancellation;
use crate::daemon::{
    AdminCommand, DaemonConfig, ReloadableConfig, StaticConfig, administer,
    serve_with_programmatic_query_backend, wait_for_discovery,
};
use crate::freshness::FreshnessState;
use crate::query_service::{QueryAuthorization, SemanticBackendOutcome, SemanticQueryBackend};
use crate::relational_program::{FieldId, RelationId, ScalarOperator};
use crate::relational_semantic_query::{
    EpochBoundExecutionOperatorRow, EpochBoundExecutionProgramRow, EpochBoundExecutionReturnRow,
    EpochBoundExecutionScopeRow, EpochBoundExecutionSelectionRow, EpochBoundProgramBindingRow,
    EpochBoundReturnBindingRow, EpochBoundScopeBindingRow, EpochBoundSelectionBindingRow,
    EpochBoundSelectionFold, EpochBoundSemanticExecutionCatalog, EpochBoundSemanticIngressCatalog,
    EpochBoundSemanticIngressLimits, ProducerClosureProof, ProgramRelationSchemaRow,
    ProgramRelationalOperator, ReleasedSemanticForm, SemanticClauseValue, SemanticQueryAuthority,
    SemanticQueryClass, SemanticRequestLimits, SemanticValueKind,
    compile_epoch_bound_semantic_request, epoch_bound_semantic_ingress_limits_pin,
    validate_epoch_bound_semantic_ingress,
};
use crate::rpc::generated::codefabric::cpgd::v1::{WorkspaceClaim, WorkspaceReadiness};
use crate::schema_contract::{
    FIELD_ID_METADATA_KEY, FieldIndexMapping, RELATION_ID_METADATA_KEY, SchemaContract,
};
use crate::semantic_query_contract::{ParsedSemanticRequest, parse_request};

const QUERY_RELATION: &str = "fact.vertical_input";

const fn id16(seed: u8) -> [u8; 16] {
    [seed; 16]
}

const fn id32(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn runtime_config() -> FabricEpochRuntimeConfig {
    FabricEpochRuntimeConfig::try_new(64 * 1024 * 1024, 256 * 1024 * 1024, 8, 8, 1_024, 1, true)
        .expect("explicit bounded runtime configuration")
}

fn epoch_builder(epoch_id: EpochId, input_value: &'static str) -> ProgrammaticFabricEpochBuilder {
    let mut builder = ProgrammaticFabricEpochBuilder::try_new(epoch_id, runtime_config())
        .expect("fresh programmatic epoch builder");
    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("value", DataType::Utf8, false).with_metadata(HashMap::from([(
                FIELD_ID_METADATA_KEY.to_owned(),
                "fact.vertical_input.value".to_owned(),
            )])),
        ],
        HashMap::from([(
            RELATION_ID_METADATA_KEY.to_owned(),
            QUERY_RELATION.to_owned(),
        )]),
    ));
    let values = match input_value {
        "initial-input" => vec![input_value],
        "successor-input" => vec![input_value, input_value],
        "third-input" => vec![input_value, input_value, input_value],
        unexpected => panic!("unreleased vertical input recipe {unexpected}"),
    };
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![Arc::new(StringArray::from(values))],
    )
    .expect("typed provider batch");
    let table_reference = TableReference::full(
        FABRIC_CATALOG,
        FabricSchemaRole::Fact.as_str(),
        "vertical_input",
    );
    let contract = Arc::new(
        SchemaContract::try_new(
            "provider:programmatic-vertical-input:v1",
            table_reference.clone(),
            Arc::clone(&schema),
            Arc::clone(&schema),
            vec![FieldIndexMapping::direct(0, 0)],
        )
        .expect("typed provider contract"),
    );
    builder
        .register_provider(ProviderInput::new(
            ProgrammaticRelationId::new(QUERY_RELATION),
            table_reference,
            contract,
            Arc::new(MemTable::try_new(schema, vec![vec![batch]]).expect("typed provider table")),
        ))
        .expect("register typed provider input");
    builder
}

fn child_resources() -> ChildResourceLimits {
    ChildResourceLimits::try_new(8 * 1024 * 1024, 32 * 1024 * 1024, 4, 2, 128, 1)
        .expect("explicit child resources")
}

fn resource_policy() -> EpochResourcePolicy {
    let policies = vec![
        EpochWorkClassPolicy::new(EpochWorkClass::SecurityRecovery, 0, true),
        EpochWorkClassPolicy::new(EpochWorkClass::SourceReconciliation, 1, true),
        EpochWorkClassPolicy::new(EpochWorkClass::StrictCurrentUpdate, 2, true),
        EpochWorkClassPolicy::new(EpochWorkClass::SourceUpdate, 3, true),
        EpochWorkClassPolicy::new(EpochWorkClass::InteractiveQuery, 4, false),
        EpochWorkClassPolicy::new(EpochWorkClass::SemanticDerived, 5, false),
        EpochWorkClassPolicy::new(EpochWorkClass::DurableFlushArtifact, 6, false),
        EpochWorkClassPolicy::new(EpochWorkClass::Maintenance, 7, false),
    ];
    EpochResourcePolicy::try_new(
        child_resources(),
        policies,
        4,
        1,
        8,
        30_000,
        1,
        2,
        8,
        64 * 1024 * 1024,
        60_000,
    )
    .expect("explicit epoch resource policy")
}

fn result_limits() -> ArrowResultResourceLimits {
    ArrowResultResourceLimits::try_new(
        4,
        8,
        10_000,
        16,
        20_000,
        1 << 20,
        2 << 20,
        1 << 20,
        2 << 20,
        1 << 20,
        64 * 1024,
    )
    .expect("explicit result limits")
}

fn request_limits() -> RequestOwnedRelationLimits {
    RequestOwnedRelationLimits::try_new(4, 64, 16, 1_024, 128, 2_048, 64 * 1024)
        .expect("explicit request-owned relation limits")
}

fn semantic_ingress_limits() -> EpochBoundSemanticIngressLimits {
    EpochBoundSemanticIngressLimits::try_new(
        SemanticRequestLimits::try_new(16, 64, 32, 32, 64, 32, 1_000)
            .expect("semantic compiler limits"),
        512,
        512,
        128,
        512,
        8,
    )
    .expect("semantic ingress limits")
}

fn releases() -> ProgrammaticWorkspaceReleasePins {
    ProgrammaticWorkspaceReleasePins::try_new(
        InputReleaseRef::from_bytes(id32(0x11)),
        ProgramReleaseRef::from_bytes(id32(0x12)),
        super::command::ProviderReleaseRef::from_bytes(id32(0x13)),
        super::command::ApplicationReleaseRef::from_bytes(id32(0x14)),
        super::command::SourceAuthorityRef::from_bytes(id32(0x15)),
    )
    .expect("complete release and source-authority pins")
}

async fn durable_epoch(
    root: &Path,
    epoch_id: EpochId,
    write_seed: u8,
    input_value: &'static str,
) -> Arc<ProgrammaticFabricEpoch> {
    let builder = epoch_builder(epoch_id, input_value);
    let mut roots = BTreeMap::new();
    for relation in [
        RELATION_OBSERVATION_RELATION_ID,
        FIELD_OBSERVATION_RELATION_ID,
        SCHEMA_OBSERVATION_RELATION_ID,
        DEPENDENCY_OBSERVATION_RELATION_ID,
        PROVENANCE_OBSERVATION_RELATION_ID,
    ] {
        let path = root.join(relation.replace('.', "_"));
        fs::create_dir_all(&path).expect("observation history directory");
        roots.insert(
            ProgrammaticRelationId::new(relation),
            Url::from_directory_path(path).expect("observation history file URL"),
        );
    }
    let targets = builder
        .provision_observation_histories(roots)
        .await
        .expect("provision observation histories");
    Arc::new(
        builder
            .seal(
                ProgrammaticObservationWriteIdentity::new(
                    epoch_id,
                    OperationId::from_bytes(id16(write_seed)),
                    super::command::WriterGeneration::new(1).expect("writer generation"),
                    TransactionRef::from_bytes(id32(write_seed.wrapping_add(1))),
                ),
                targets,
            )
            .await
            .expect("seal durable epoch"),
    )
}

fn semantic_catalogs(
    epoch: &ProgrammaticFabricEpoch,
    release_pins: ProgrammaticWorkspaceReleasePins,
) -> (
    Arc<EpochBoundSemanticIngressCatalog>,
    Arc<EpochBoundSemanticExecutionCatalog>,
    Arc<ProducerClosureProof>,
) {
    let fabric_epoch_pin = programmatic_fabric_epoch_authority_pin(epoch);
    let source_pin = *release_pins.source_authority().as_bytes();
    let field_id = FieldId::new("fact.vertical_input.value").expect("query field identity");
    let relation_id = RelationId::new(QUERY_RELATION).expect("query relation");
    let program_binding_id: Arc<str> = Arc::from("program.vertical-input.find-entities");
    let input_node_id: Arc<str> = Arc::from("node.vertical-input.input");
    let filter_node_id: Arc<str> = Arc::from("node.vertical-input.filter");
    let limit_node_id: Arc<str> = Arc::from("node.vertical-input.limit");
    let limits_pin = epoch_bound_semantic_ingress_limits_pin(semantic_ingress_limits());
    let ingress = Arc::new(EpochBoundSemanticIngressCatalog {
        fabric_epoch_pin,
        program_catalog_pin: id32(0x21),
        source_pin,
        policy_pin: id32(0x22),
        producer_closure_proof_pin: id32(0x23),
        limits_pin,
        program_bindings: vec![EpochBoundProgramBindingRow {
            program_binding_id: Arc::clone(&program_binding_id),
            program_binding_pin: id32(0x25),
            compatibility_form: ReleasedSemanticForm::FindCodeEntities,
            output_role_id: Arc::from("role.entities"),
            execution_program_pin: id32(0x26),
        }],
        consumer_slots: Vec::new(),
        selections: vec![EpochBoundSelectionBindingRow {
            program_binding_id: Arc::clone(&program_binding_id),
            selection_id: Arc::from("selection.looking-for"),
            value_kind: SemanticValueKind::Text,
            minimum_values: 1,
            maximum_values: 1,
        }],
        returns: vec![EpochBoundReturnBindingRow {
            program_binding_id: Arc::clone(&program_binding_id),
            return_id: Arc::from("return.maximum-results"),
            value_kind: SemanticValueKind::UInt64,
            minimum_values: 1,
            maximum_values: 1,
        }],
        scopes: [
            ("scope.specification", SemanticValueKind::Text, 1, 1),
            ("scope.version", SemanticValueKind::Text, 1, 1),
            ("scope.workspace", SemanticValueKind::Text, 1, 1),
            ("scope.freshness", SemanticValueKind::Text, 1, 1),
            ("scope.cost-maximum-rows", SemanticValueKind::UInt64, 0, 1),
            (
                "scope.response-projection-field",
                SemanticValueKind::Text,
                0,
                128,
            ),
            (
                "scope.response-projection-enabled",
                SemanticValueKind::Boolean,
                0,
                128,
            ),
        ]
        .into_iter()
        .map(
            |(scope_id, value_kind, minimum_values, maximum_values)| EpochBoundScopeBindingRow {
                scope_id: Arc::from(scope_id),
                value_kind,
                minimum_values,
                maximum_values,
            },
        )
        .collect(),
        request_inputs: Vec::new(),
    });
    let execution = Arc::new(EpochBoundSemanticExecutionCatalog {
        fabric_epoch_pin,
        program_catalog_pin: id32(0x21),
        source_pin,
        policy_pin: id32(0x22),
        producer_closure_proof_pin: id32(0x23),
        execution_catalog_pin: id32(0x27),
        program_release_pin: *release_pins.program_release().as_bytes(),
        authority: SemanticQueryAuthority::ApplicationOwned(Arc::from(
            "query.programmatic.vertical",
        )),
        semantic_class: SemanticQueryClass::Fact(Arc::from("typed_provider_input")),
        programs: vec![EpochBoundExecutionProgramRow {
            program_binding_id: Arc::clone(&program_binding_id),
            execution_program_pin: id32(0x26),
            root_node_id: Arc::clone(&limit_node_id),
            output_relation_id: relation_id.clone(),
            output_fields: vec![field_id.clone()],
        }],
        operators: vec![
            EpochBoundExecutionOperatorRow {
                program_binding_id: Arc::clone(&program_binding_id),
                execution_program_pin: id32(0x26),
                node_id: Arc::clone(&input_node_id),
                ordinal: 0,
                input_node_ids: Vec::new(),
                operator: ProgramRelationalOperator::Input {
                    relation_id: relation_id.clone(),
                },
                output_fields: vec![field_id.clone()],
            },
            EpochBoundExecutionOperatorRow {
                program_binding_id: Arc::clone(&program_binding_id),
                execution_program_pin: id32(0x26),
                node_id: Arc::clone(&filter_node_id),
                ordinal: 1,
                input_node_ids: vec![input_node_id],
                operator: ProgramRelationalOperator::Filter,
                output_fields: vec![field_id.clone()],
            },
            EpochBoundExecutionOperatorRow {
                program_binding_id: Arc::clone(&program_binding_id),
                execution_program_pin: id32(0x26),
                node_id: Arc::clone(&limit_node_id),
                ordinal: 2,
                input_node_ids: vec![Arc::clone(&filter_node_id)],
                operator: ProgramRelationalOperator::Limit { skip: 0 },
                output_fields: vec![field_id.clone()],
            },
        ],
        relation_schemas: vec![ProgramRelationSchemaRow {
            relation_id,
            fields: vec![field_id.clone()],
        }],
        consumer_slots: Vec::new(),
        selections: vec![EpochBoundExecutionSelectionRow {
            program_binding_id: Arc::clone(&program_binding_id),
            execution_program_pin: id32(0x26),
            selection_id: Arc::from("selection.looking-for"),
            operator_node_id: filter_node_id,
            input_field_id: field_id.clone(),
            scalar_operator: ScalarOperator::Equal,
            fold: EpochBoundSelectionFold::All,
        }],
        returns: vec![EpochBoundExecutionReturnRow {
            program_binding_id: Arc::clone(&program_binding_id),
            execution_program_pin: id32(0x26),
            return_id: Arc::from("return.maximum-results"),
            value: SemanticClauseValue::UInt64(10),
            realization_node_id: limit_node_id,
            realization_field_ids: vec![field_id],
            realization_pin: id32(0x29),
        }],
        required_fact_families: Vec::new(),
        request_inputs: Vec::new(),
        scopes: [
            "scope.specification",
            "scope.version",
            "scope.workspace",
            "scope.freshness",
            "scope.cost-maximum-rows",
            "scope.response-projection-field",
            "scope.response-projection-enabled",
        ]
        .into_iter()
        .enumerate()
        .map(|(ordinal, scope_id)| EpochBoundExecutionScopeRow {
            scope_id: Arc::from(scope_id),
            authorization_input_id: Arc::from(format!("authorization.{scope_id}")),
            handoff_pin: id32(0x2a + u8::try_from(ordinal).expect("bounded scope count")),
        })
        .collect(),
    });
    let closure = Arc::new(ProducerClosureProof {
        proof_pin: id32(0x23),
        application_authority_id: Arc::from("query.programmatic.vertical"),
        families: Vec::new(),
    });
    (ingress, execution, closure)
}

fn query_authorization(resource_policy_pin: [u8; 32]) -> RelationalQueryAuthorization {
    RelationalQueryAuthorization::try_new(
        id32(0x31),
        id32(0x22),
        resource_policy_pin,
        vec![
            ChildTableGrant::try_new(ProgrammaticRelationId::new(QUERY_RELATION))
                .expect("query table grant"),
        ],
        child_resources(),
        10_000,
        ChildRegistryAllowlist::default(),
    )
    .expect("exact query authorization")
}

fn query_authority(
    workspace_id: WorkspaceId,
    activation_pins: FabricEpochPins,
    epoch: Arc<ProgrammaticFabricEpoch>,
    release_pins: ProgrammaticWorkspaceReleasePins,
    resource_policy_pin: [u8; 32],
) -> Arc<WorkspaceEpochQueryAuthority> {
    let (ingress, execution, closure) = semantic_catalogs(&epoch, release_pins);
    Arc::new(
        WorkspaceEpochQueryAuthority::try_new(
            workspace_id,
            activation_pins,
            Arc::clone(&epoch),
            Arc::new(
                EpochResourceCoordinator::try_new(
                    *epoch.identity(),
                    resource_policy_pin,
                    resource_policy(),
                )
                .expect("successor resources"),
            ),
            ingress,
            execution,
            closure,
            query_authorization(resource_policy_pin),
            request_limits(),
            result_limits(),
            60_000,
        )
        .expect("successor query authority"),
    )
}

fn passing_proof(pins: FabricEpochPins) -> Arc<super::proof::ProofRelations> {
    let proof_pins = ProofCandidatePins {
        epoch: pins.epoch,
        input_release: pins.input_release,
        program_release: pins.program_release,
        application_release: pins.application_release,
        source_authority: pins.source_authority,
        provider_release: pins.provider_release,
        source_generation: pins.source_generation,
        source_images: SourceImageSetRef::from_bytes(id32(0x40)),
        provider_set: pins.provider_set,
        table_versions: pins.table_versions,
        overlay_segments: pins.overlay_segments,
        policy_set: pins.policy_set,
        resource_envelope: pins.resource_envelope,
    };
    let producer = ProofOwnerId::new(id32(0x41)).expect("producer owner");
    let oracle_id = OracleId::new(id16(0x42)).expect("oracle id");
    let capability_id = CapabilityId::new(id16(0x43)).expect("capability id");
    let expectation_id = ExpectationId::new(id16(0x44)).expect("expectation id");
    let fault_id = CausalFaultId::new(id16(0x45)).expect("fault id");
    let violation_id = ViolationId::new(id16(0x46)).expect("violation id");
    let run_id = ProofRunId::new(id16(0x47)).expect("run id");
    let scope = CoverageScopeId::new(id16(0x48)).expect("scope id");
    let implementation = OracleImplementationRef::new(id32(0x49)).expect("implementation");
    let violation_relation = ProofRelationId::new(id16(0x4a)).expect("violation relation");
    let authority = IndependentEvidenceAuthority {
        author: ProofOwnerId::new(id32(0x4b)).expect("author"),
        reviewer: ProofOwnerId::new(id32(0x4c)).expect("reviewer"),
        acceptance_authority: ProofOwnerId::new(id32(0x4d)).expect("acceptance authority"),
    };
    let oracle = OracleRequest {
        oracle_id,
        implementation,
        violation_relation,
        requested_scopes: vec![scope],
    };
    let expectation = SemanticExpectation {
        expectation_id,
        oracle_id,
        coverage_scope: scope,
        claim: SemanticClaimRef::new(id32(0x4e)).expect("semantic claim"),
        source_anchor: SourceAnchorRef::new(id32(0x4f)).expect("source anchor"),
        authority,
    };
    let fault = RequiredCausalFault {
        fault_id,
        oracle_id,
        coverage_scope: scope,
        program: CausalFaultProgramRef::new(id32(0x50)).expect("fault program"),
        required_effect: RequiredCausalEffect::SemanticDiscrimination,
        authority,
    };
    let execution = OracleExecution {
        oracle_id,
        run_id,
        candidate_pins: proof_pins,
        completed_scopes: vec![scope],
        unavailable_scopes: Vec::new(),
    };
    let violation = ProofViolation {
        violation_id,
        oracle_id,
        expectation_id: Some(expectation_id),
        fault_id: Some(fault_id),
        kind: ProofViolationKind::SemanticMismatch,
    };
    let root = ProvenanceSubject::OracleRun(run_id);
    let provenance = [
        ProvenanceSubject::Epoch(proof_pins.epoch),
        ProvenanceSubject::InputRelease(proof_pins.input_release),
        ProvenanceSubject::ProgramRelease(proof_pins.program_release),
        ProvenanceSubject::ApplicationRelease(proof_pins.application_release),
        ProvenanceSubject::SourceAuthority(proof_pins.source_authority),
        ProvenanceSubject::SourceGeneration(proof_pins.source_generation),
        ProvenanceSubject::SourceImages(proof_pins.source_images),
        ProvenanceSubject::ProviderRelease(proof_pins.provider_release),
        ProvenanceSubject::ProviderSet(proof_pins.provider_set),
        ProvenanceSubject::TableVersions(proof_pins.table_versions),
        ProvenanceSubject::OverlaySegments(proof_pins.overlay_segments),
        ProvenanceSubject::PolicySet(proof_pins.policy_set),
        ProvenanceSubject::ResourceEnvelope(proof_pins.resource_envelope),
        ProvenanceSubject::OracleImplementation(implementation),
        ProvenanceSubject::ViolationRelation(violation_relation),
        ProvenanceSubject::Capability(capability_id),
        ProvenanceSubject::Expectation(expectation_id),
        ProvenanceSubject::SemanticClaim(expectation.claim),
        ProvenanceSubject::SourceAnchor(expectation.source_anchor),
        ProvenanceSubject::CausalFault(fault_id),
        ProvenanceSubject::CausalFaultProgram(fault.program),
    ]
    .into_iter()
    .map(|subject| ProofProvenanceEdge {
        from: root,
        to: subject,
    })
    .collect::<Vec<_>>();
    let oracles = [oracle];
    let capabilities = [CapabilityRequest { capability_id }];
    let requirements = [CapabilityOracleRequirement {
        capability_id,
        oracle_id,
    }];
    let executions = [execution];
    let violations = [violation];
    let fault_executions = [CausalFaultExecution {
        fault_id,
        outcome: CausalFaultOutcome::Detected { violation_id },
    }];
    let expectations = [expectation];
    let faults = [fault];
    let relations = evaluate_candidate_proof(
        &CandidateProofInput {
            producer_owner: producer,
            candidate_pins: proof_pins,
            oracle_requests: &oracles,
            capability_requests: &capabilities,
            capability_requirements: &requirements,
            oracle_executions: &executions,
            violations: &violations,
            fault_executions: &fault_executions,
            provenance_edges: &provenance,
        },
        &IndependentProofInput {
            expectations: &expectations,
            required_faults: &faults,
        },
    )
    .expect("evaluate real proof relations");
    assert_eq!(relations.terminal(), ProofTerminalStatus::Pass);
    Arc::new(relations)
}

struct ExactCommandPolicyRow {
    workspace_id: WorkspaceId,
    principal_id: PrincipalId,
    authorization: AuthorizationRef,
}

#[async_trait]
impl CommandAuthorizationPort for ExactCommandPolicyRow {
    async fn authorize(
        &self,
        command: &FabricCommand,
        current_head: ExpectedHead,
    ) -> Result<AuthorizationDecision, CommandPortError> {
        if command.ownership.workspace_id != self.workspace_id
            || command.ownership.principal_id != self.principal_id
            || command.ownership.authorization != self.authorization
            || command.expected_head != current_head
        {
            return Ok(AuthorizationDecision::Denied(DiagnosticRef::from_bytes(
                id32(0x61),
            )));
        }
        Ok(AuthorizationDecision::Authorized(self.authorization))
    }
}

struct ExactInterruptedDiagnosticRow {
    workspace_id: WorkspaceId,
    diagnostic: DiagnosticRef,
}

#[async_trait]
impl InterruptedCommitDiagnosticRelationPort for ExactInterruptedDiagnosticRow {
    async fn read_interruption_diagnostic(
        &self,
        query: InterruptedCommitDiagnosticQuery,
    ) -> Result<Option<DiagnosticRef>, CommandPortError> {
        if query.workspace_id != self.workspace_id {
            return Err(CommandPortError::ContextUnavailable);
        }
        Ok(Some(self.diagnostic))
    }
}

fn activation_command(
    workspace_id: WorkspaceId,
    expected_head: ExpectedHead,
    target_epoch: EpochId,
    writer_fence: WriterFence,
    operation_seed: u8,
    proof_receipt: ProofReceiptRef,
    resource_policy_pin: [u8; 32],
) -> FabricCommand {
    FabricCommand {
        identity: CommandIdentity {
            operation_id: OperationId::from_bytes(id16(operation_seed)),
            idempotency_key: IdempotencyKey::from_bytes(id32(operation_seed)),
        },
        ownership: CommandOwnership {
            workspace_id,
            principal_id: PrincipalId::from_bytes(id16(0x62)),
            authorization: AuthorizationRef::from_bytes(id32(0x63)),
        },
        expected_head,
        writer_fence,
        pins: CommandPins {
            input_release: InputReleaseRef::from_bytes(id32(0x11)),
            program_release: ProgramReleaseRef::from_bytes(id32(0x12)),
            application_release: crate::fabric::command::ApplicationReleaseRef::from_bytes(id32(
                0x14,
            )),
            source_authority: crate::fabric::command::SourceAuthorityRef::from_bytes(id32(0x15)),
            provider_release: crate::fabric::command::ProviderReleaseRef::from_bytes(id32(0x13)),
            source_generation: SourceGeneration::new(7),
            provider_set: ProviderSetRef::from_bytes(id32(0x66)),
        },
        resources: ResourceEnvelopeRef::from_bytes(resource_policy_pin),
        payload: FabricCommandPayload::ActivateEpoch {
            candidate_epoch: target_epoch,
            proof_receipt,
        },
    }
}

fn activation_pins(
    command: &FabricCommand,
    candidate: &ProgrammaticFabricEpoch,
    proof_receipt: ProofReceiptRef,
) -> FabricEpochPins {
    FabricEpochPins {
        epoch: *candidate.identity(),
        input_release: command.pins.input_release,
        program_release: command.pins.program_release,
        application_release: command.pins.application_release,
        source_authority: command.pins.source_authority,
        provider_release: command.pins.provider_release,
        source_generation: command.pins.source_generation,
        provider_set: command.pins.provider_set,
        table_versions: candidate.observation_publication().table_version_set_ref(),
        overlay_segments: OverlaySegmentSetRef::from_bytes(id32(0x67)),
        policy_set: PolicySetRef::from_bytes(id32(0x68)),
        resource_envelope: command.resources,
        proof_receipt,
    }
}

fn activation_event(
    command: FabricCommand,
    pins: FabricEpochPins,
    event_seed: u8,
    predecessor_event_id: Option<ActivationEventId>,
    ordinal: u64,
) -> ActivationEvent {
    ActivationEvent::try_from_attempt(
        ActivationEventId::from_bytes(id32(event_seed)),
        ActivationAttempt::for_test(
            command,
            1,
            ExecutionOwner {
                actor_id: ActorId::from_bytes(id16(0x69)),
                fence: command.writer_fence,
            },
        ),
        predecessor_event_id,
        ActivationOrdinal::new(ordinal).expect("activation ordinal"),
        pins,
        CompatibilityClassRef::from_bytes(id32(0x6a)),
        RetentionPolicyRef::from_bytes(id32(0x6b)),
        ActivationCommit {
            operation_selection: OperationSelectionRef::from_bytes(id32(
                event_seed.wrapping_add(1),
            )),
            transaction: TransactionRef::from_bytes(id32(event_seed.wrapping_add(2))),
            backend_commit: BackendCommitRef::from_bytes(id32(event_seed.wrapping_add(3))),
            readback: ActivationReadbackRef::from_bytes(id32(event_seed.wrapping_add(4))),
        },
    )
    .expect("valid activation event")
}

fn unavailable_effects() -> ProgrammaticNonActivationCommandEffects {
    let gap = |seed| {
        ProgrammaticCommandCapabilityGapInput::new(
            ProgrammaticCommandCapabilityDisposition::Unavailable,
            DiagnosticRef::from_bytes(id32(seed)),
        )
    };
    ProgrammaticNonActivationCommandEffects::try_new(
        gap(0x71),
        gap(0x72),
        gap(0x73),
        gap(0x74),
        gap(0x75),
        gap(0x76),
    )
    .expect("exhaustive non-activation capability closure")
}

fn successor_recipe(
    authority: &WorkspaceEpochQueryAuthority,
    pins: FabricEpochPins,
) -> ExactProgrammaticSuccessorQueryAuthorityRecipe {
    ExactProgrammaticSuccessorQueryAuthorityRecipe::try_new(
        authority.workspace_id(),
        pins,
        programmatic_fabric_epoch_authority_pin(authority.epoch()),
        authority.resources().policy().clone(),
        authority.ingress_catalog().as_ref().clone(),
        authority.execution_catalog().as_ref().clone(),
        authority.producer_closure().as_ref().clone(),
        authority.authorization().clone(),
        authority.request_owned_relation_limits(),
        authority.result_limits(),
        authority.result_lease_millis(),
    )
    .expect("complete successor query-authority recipe")
}

#[allow(clippy::too_many_arguments)]
fn command_runtime_factory(
    state_root: &Path,
    workspace_id: WorkspaceId,
    current_event: ActivationEvent,
    current_fence: WriterFence,
    control: &ActivationControlDeltaProvider,
    resource_policy_pin: [u8; 32],
    activation_store: Arc<SqliteProgrammaticActivationCommandStateStore>,
    successor_pins: FabricEpochPins,
    successor: &WorkspaceEpochQueryAuthority,
    successor_input: &'static str,
) -> Arc<ExactProgrammaticCommandRuntimePartsFactory> {
    let proof_authority = Arc::new(
        ExactProgrammaticActivationProofAuthority::try_new(
            workspace_id,
            successor_pins,
            passing_proof(successor_pins),
            Some(successor_pins.proof_receipt),
            None,
            DiagnosticRef::from_bytes(id32(0x77)),
        )
        .expect("exact passing proof authority"),
    );
    let recovery_rebuilder = Arc::new(ExactDeltaProgrammaticEpochRebuilder::new(move |epoch_id| {
        Ok::<_, std::convert::Infallible>(epoch_builder(epoch_id, successor_input))
    }));
    let activation = ProgrammaticActivationCommandEffects::new(
        activation_store,
        proof_authority,
        DiagnosticRef::from_bytes(id32(0x78)),
        recovery_rebuilder,
        Arc::new(successor_recipe(successor, successor_pins)),
    );
    let config = FabricCommandRuntimeConfig::new(
        state_root.join("command-admin"),
        state_root.join("writer-generations.sqlite3"),
        state_root.join("command-records.sqlite3"),
        workspace_id,
        LeaseId::from_bytes(id16(0x79)),
        ActorId::from_bytes(id16(0x7a)),
        FabricCommandActorConfig::default(),
    );
    Arc::new(ExactProgrammaticCommandRuntimePartsFactory::new(
        config,
        ProgrammaticCommandRuntimeAuthorityBinding::new(
            workspace_id,
            current_event.pins().epoch,
            current_event.event_id(),
            current_fence,
            *control.control_relation().fingerprint(),
            resource_policy_pin,
        ),
        Arc::new(ExactCommandPolicyRow {
            workspace_id,
            principal_id: PrincipalId::from_bytes(id16(0x62)),
            authorization: AuthorizationRef::from_bytes(id32(0x63)),
        }),
        ExactProgrammaticCommandEffectClosure::new(activation, unavailable_effects()),
        Arc::new(ExactInterruptedDiagnosticRow {
            workspace_id,
            diagnostic: DiagnosticRef::from_bytes(id32(0x7b)),
        }),
    ))
}

fn workspace_construction(
    state_root: &Path,
    workspace_id: WorkspaceId,
    current_epoch: &ProgrammaticFabricEpoch,
    activation_authority: Arc<DeltaActivationRuntimeAuthority>,
    command_factory: Arc<ExactProgrammaticCommandRuntimePartsFactory>,
    release_pins: ProgrammaticWorkspaceReleasePins,
    resource_policy_pin: [u8; 32],
    current_input: &'static str,
) -> ProgrammaticWorkspaceConstruction {
    let (ingress, execution, closure) = semantic_catalogs(current_epoch, release_pins);
    let checkpoint_parent = state_root.join("cdf-checkpoints");
    fs::create_dir_all(&checkpoint_parent).expect("create private CDF checkpoint parent");
    fs::set_permissions(&checkpoint_parent, fs::Permissions::from_mode(0o700))
        .expect("protect CDF checkpoint parent");
    let checkpoints = Arc::new(
        SqliteDeltaCdfCheckpointStore::open(&checkpoint_parent.join("checkpoints.sqlite3"))
            .expect("open CDF checkpoint store"),
    );
    ProgrammaticWorkspaceConstruction::try_new(
        workspace_id,
        epoch_builder(*current_epoch.identity(), current_input),
        Arc::clone(current_epoch.observation_publication().table_version_set()),
        activation_authority,
        resource_policy(),
        resource_policy_pin,
        ingress,
        execution,
        closure,
        query_authorization(resource_policy_pin),
        request_limits(),
        result_limits(),
        60_000,
        ProgrammaticDeltaRuntimePorts::new(checkpoints, Arc::new(UnavailableMaintenance)),
        command_factory,
        release_pins,
    )
    .expect("complete workspace construction")
}

#[derive(Debug)]
struct UnavailableMaintenance;

#[async_trait]
impl DeltaMaintenanceSafetyPort for UnavailableMaintenance {
    async fn observe(
        &self,
        _target: &super::delta_exact::ExactDeltaPin,
    ) -> Result<DeltaMaintenanceSafetyEvidence, DeltaMaintenanceAuthorityError> {
        Err(DeltaMaintenanceAuthorityError::new(
            "vertical fixture did not install maintenance evidence",
        ))
    }
}

fn released_find_entities_request(
    workspace_id: &str,
    semantic_request_id: &str,
    looking_for: &str,
    expanded_scopes: bool,
) -> ParsedSemanticRequest {
    let response_projection = expanded_scopes.then(|| {
        serde_json::json!({
            "identity": true,
            "debug": false
        })
    });
    let cost_budget = expanded_scopes.then(|| serde_json::json!({"maximum_rows": 10}));
    let wire = serde_json::json!({
        "specification": "composable semantic CPG fact query",
        "version": "1.3",
        "semantic_request_id": semantic_request_id,
        "workspace_id": workspace_id,
        "freshness_policy": "best_available_snapshot",
        "queries": [{
            "query_id": "find-entities",
            "request": "find code entities",
            "label": null,
            "looking_for": looking_for,
            "return": {"limit": {"maximum_results": 10}}
        }],
        "response_projection": response_projection,
        "cost_budget": cost_budget
    });
    parse_request(&serde_json::to_vec(&wire).expect("released request JSON"))
        .expect("released request parses and canonicalizes")
}

fn production_semantic_backend(
    daemon: &ProgrammaticDaemonComposition,
    workspace: &ProgrammaticWorkspaceRuntime,
) -> ProgrammaticSemanticQueryBackend {
    let ingress = Arc::new(
        ApplicationOwnedSemanticIngressPort::try_released_v1_3(
            id32(0x2f),
            semantic_ingress_limits(),
        )
        .expect("exact released ingress mapping"),
    );
    let authority = workspace
        .query_authorities()
        .resolve(
            workspace
                .admission()
                .admit()
                .expect("current query admission")
                .epoch_id(),
        )
        .expect("current query authority");
    let table_relations = BTreeSet::from([ProgrammaticRelationId::new(QUERY_RELATION)]);
    let ports = ProgrammaticSemanticQueryPorts::try_new(
        *releases().application_release().as_bytes(),
        ingress,
        Arc::new(
            ReleasedV13ProgrammaticScopeAuthorization::try_new(
                id32(0x22),
                authority.execution_catalog(),
                table_relations,
                10_000,
            )
            .expect("released request-independent scope policy"),
        ),
        Arc::new(ExactProgrammaticSnapshotProjection::new()),
    )
    .expect("complete programmatic semantic backend ports");
    ProgrammaticSemanticQueryBackend::try_new(daemon, ports)
        .expect("programmatic semantic backend composition")
}

fn wp37_daemon_config(root: &Path) -> DaemonConfig {
    let state_root = root.join("wp37-daemon-state");
    let runtime_root = root.join("wp37-daemon-runtime");
    let config_root = root.join("wp37-daemon-config");
    for directory in [&state_root, &runtime_root, &config_root] {
        fs::create_dir_all(directory).expect("WP37 private daemon directory");
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
            .expect("WP37 private daemon directory permissions");
    }
    let capability = config_root.join("query.capability");
    fs::write(&capability, b"wp37-production-capability").expect("WP37 query capability");
    fs::set_permissions(&capability, fs::Permissions::from_mode(0o600))
        .expect("WP37 query capability permissions");
    DaemonConfig {
        static_config: StaticConfig {
            state_root,
            runtime_root: runtime_root.clone(),
            config_root,
            socket_endpoint: runtime_root.join("admin.sock"),
            query_socket_endpoint: runtime_root.join("query.sock"),
            query_capability_token_file: PathBuf::from("query.capability"),
            operational_database: PathBuf::from("operational.sqlite3"),
            sandbox_policy: "required-for-untrusted".to_owned(),
            hard_limit_profile: "daemon-default-v1".to_owned(),
            supported_platform_profile: "local-workstation-v1".to_owned(),
        },
        reloadable: ReloadableConfig {
            log_level: "info".to_owned(),
            telemetry_sampling: 0.0,
            soft_query_quota: 4,
            maintenance_schedule: "disabled-during-test".to_owned(),
        },
    }
}

async fn exercise_wp37_real_uds_fastmcp_vertical(
    daemon: &ProgrammaticDaemonComposition,
    backend: Arc<ProgrammaticSemanticQueryBackend>,
    state_root: &Path,
    public_workspace_id: &str,
    request: &ParsedSemanticRequest,
) {
    let config = wp37_daemon_config(state_root);
    let discovery_path = config.static_config.runtime_root.join("daemon.json");
    let admin_socket = config.static_config.socket_endpoint.clone();
    let query_socket = config.static_config.query_socket_endpoint.clone();
    let query_socket_check = query_socket.clone();
    let claims = vec![WorkspaceClaim {
        workspace_id: public_workspace_id.to_owned(),
        repository_id: None,
        worktree_id: None,
        workspace_kind: "programmatic".to_owned(),
        readiness: WorkspaceReadiness::Ready as i32,
        permission_claims: vec!["query".to_owned()],
    }];
    let request_json = String::from_utf8(request.canonical_bytes.clone())
        .expect("released request canonical UTF-8");
    let workspace = public_workspace_id.to_owned();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf();
    let client = async {
        let discovery = wait_for_discovery(&discovery_path, Duration::from_secs(10)).await;
        let status = if discovery.is_ok() {
            Some(
                tokio::task::spawn_blocking(move || {
                    const SCRIPT: &str = r#"
import asyncio
import base64
import json
import sys
from fastmcp import Client
from mcp.types import BlobResourceContents, TextResourceContents
from codefabric_cpg_mcp.server import mcp

async def main():
    request = json.loads(sys.argv[1])
    async with Client(mcp) as client:
        result = await client.call_tool(
            "query_code_graph",
            {"request": request, "delivery": "resource"},
        )
        output = result.structured_content
        assert output is not None
        assert output["execution_state"] == "COMPLETE"
        state_contract = {
            ("AVAILABLE", "COMPLETE"): "complete",
            ("PARTIAL", "PARTIAL"): "partial",
            ("PARTIAL", "INDETERMINATE"): "unknown",
        }
        completion = state_contract[
            (output["availability_state"], output["completeness_state"])
        ]
        assert output["counts"]["fact_count"] == 2
        delivery = output["delivery"]
        assert delivery["mode"] == "resource"
        resource = delivery["result_resource"]
        manifest = await client.read_resource(resource["manifest_uri"])
        assert len(manifest) == 1 and isinstance(manifest[0], TextResourceContents)
        manifest_value = json.loads(manifest[0].text)
        assert manifest_value["completion_state"] == completion
        assert manifest_value["total_rows"] == 2
        relations = resource["subresource_uris"]
        assert relations
        for uri in relations:
            content = await client.read_resource(uri)
            assert len(content) == 1 and isinstance(content[0], BlobResourceContents)
            assert base64.b64decode(content[0].blob)

asyncio.run(main())
"#;
                    Command::new("env")
                        .args([
                            "-u",
                            "VIRTUAL_ENV",
                            "-u",
                            "UV_PROJECT_ENVIRONMENT",
                            "uv",
                            "run",
                            "--frozen",
                            "--project",
                            "codefabric-cpg-mcp",
                            "python",
                            "-c",
                            SCRIPT,
                        ])
                        .arg(request_json)
                        .current_dir(root)
                        .env(
                            "CODEFABRIC_CPG_DAEMON_TARGET",
                            format!("unix://{}", query_socket.display()),
                        )
                        .env("CODEFABRIC_WORKSPACE_ID", workspace)
                        .env("CODEFABRIC_AGENT_INSTANCE_ID", "wp37-fastmcp-agent")
                        .env(
                            "CODEFABRIC_CPG_CAPABILITY_TOKEN",
                            "wp37-production-capability",
                        )
                        .status()
                })
                .await,
            )
        } else {
            None
        };
        let stop = administer(&discovery_path, AdminCommand::Stop).await;
        (discovery, status, stop)
    };
    let serving = serve_with_programmatic_query_backend(
        config,
        backend,
        claims,
        None,
        Arc::clone(daemon.published_results()),
        daemon,
        None,
    );
    let (served, (discovery, status, stop)) = tokio::join!(serving, client);
    discovery.expect("WP37 daemon discovery");
    let process = status
        .expect("WP37 daemon was discovered before launching Python")
        .expect("WP37 Python process task")
        .expect("launch WP37 FastMCP client");
    assert!(process.success(), "WP37 FastMCP client failed: {process}");
    assert!(stop.expect("WP37 daemon stop").accepted);
    let exit = served.expect("WP37 daemon serving vertical");
    assert!(!exit.drained);
    assert!(!admin_socket.exists());
    assert!(!query_socket_check.exists());
}

fn prove_released_scope_budget_and_handoff_causality(
    workspace: &ProgrammaticWorkspaceRuntime,
    request: &ParsedSemanticRequest,
) {
    let authority = workspace
        .query_authorities()
        .resolve(
            workspace
                .admission()
                .admit()
                .expect("current query admission")
                .epoch_id(),
        )
        .expect("current query authority");
    let ingress = ApplicationOwnedSemanticIngressPort::try_released_v1_3(
        id32(0x2f),
        semantic_ingress_limits(),
    )
    .expect("exact released ingress mapping")
    .project(request, workspace, &authority)
    .expect("released request projects into exact epoch ingress");
    let validated = validate_epoch_bound_semantic_ingress(ingress, authority.ingress_catalog())
        .expect("released ingress validates against exact epoch catalog");
    let compiled = compile_epoch_bound_semantic_request(
        &validated,
        authority.execution_catalog(),
        authority.producer_closure(),
    )
    .expect("released ingress compiles with complete handoff");
    let policy = ReleasedV13ProgrammaticScopeAuthorization::try_new(
        id32(0x22),
        authority.execution_catalog(),
        BTreeSet::from([ProgrammaticRelationId::new(QUERY_RELATION)]),
        10_000,
    )
    .expect("released request-independent scope policy");
    let owner = PublishedResultOwner::new(
        workspace.workspace_id(),
        PrincipalId::from_bytes(id16(0x9f)),
    );
    let authorization = policy
        .authorize(
            request,
            owner,
            workspace,
            &authority,
            &compiled.handoff().scopes,
        )
        .expect("exact compiled scope handoff is authorized");
    assert_eq!(
        authorization.max_output_rows(),
        request
            .request
            .cost_budget
            .expect("causal scope proof uses an explicit request budget")
            .maximum_rows,
        "the released request budget must narrow the runtime row capability"
    );

    let mut tampered_scopes = compiled.handoff().scopes.clone();
    let tampered = tampered_scopes
        .iter_mut()
        .find_map(|scope| scope.rows.first_mut())
        .expect("expanded request emits at least one scope row");
    tampered.ordinal = tampered
        .ordinal
        .checked_add(1)
        .expect("fixture scope ordinal remains bounded");
    assert!(
        policy
            .authorize(request, owner, workspace, &authority, &tampered_scopes)
            .is_err(),
        "a compiler handoff row that differs from the request projection must be rejected"
    );
}

async fn execute_released_query(
    backend: &ProgrammaticSemanticQueryBackend,
    daemon: &ProgrammaticDaemonComposition,
    workspace: &ProgrammaticWorkspaceRuntime,
    request: ParsedSemanticRequest,
    seed: u8,
) -> RelationalQueryPublication {
    backend
        .validate_execution_request(&request)
        .expect("released request accepted by programmatic backend");
    let public_workspace = workspace
        .public_workspace_id()
        .expect("public workspace identity");
    let authorization = QueryAuthorization::new(
        b"programmatic-vertical-capability",
        vec![WorkspaceClaim {
            workspace_id: public_workspace.clone(),
            repository_id: None,
            worktree_id: None,
            workspace_kind: "programmatic".to_owned(),
            readiness: WorkspaceReadiness::Ready as i32,
            permission_claims: vec!["query".to_owned()],
        }],
    )
    .expect("query delivery authorization");
    let execution = QueryExecutionContext {
        execution_id: format!("vertical-execution-{seed}"),
        semantic_request_id: request.request.semantic_request_id.clone(),
        mcp_call_id: format!("vertical-call-{seed}"),
    };
    let context = authorization
        .backend_execution_context(
            execution.clone(),
            "programmatic-vertical-agent",
            &public_workspace,
            Arc::clone(daemon.published_results()),
        )
        .expect("backend execution capabilities");
    let artifacts = QueryExecutionArtifactAccumulator::new(execution);
    match backend
        .execute(
            request,
            FreshnessState::Current,
            Cancellation::default(),
            context,
            artifacts,
        )
        .await
    {
        SemanticBackendOutcome::PublishedArrow(success) => success.publication().clone(),
        SemanticBackendOutcome::Failed { error, .. }
        | SemanticBackendOutcome::Cancelled { error, .. } => {
            panic!("programmatic semantic backend failed before Arrow publication: {error}")
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_delta_sqlite_daemon_query_activation_and_process_reopen() {
    let delta_root = TempDir::new().expect("Delta root");
    let state_root = TempDir::new().expect("SQLite state root");
    fs::set_permissions(state_root.path(), fs::Permissions::from_mode(0o700))
        .expect("private SQLite root");
    let command_admin = state_root.path().join("command-admin");
    fs::create_dir_all(&command_admin).expect("command admin root");
    fs::set_permissions(&command_admin, fs::Permissions::from_mode(0o700))
        .expect("private command admin root");

    let workspace_id = WorkspaceId::from_bytes(id16(0x80));
    let initial_epoch_id = EpochId::from_bytes(id16(0x81));
    let successor_epoch_id = EpochId::from_bytes(id16(0x82));
    let third_epoch_id = EpochId::from_bytes(id16(0x83));
    let resource_policy_pin = id32(0x84);
    let release_pins = releases();
    let initial_epoch = durable_epoch(
        &delta_root.path().join("initial-observations"),
        initial_epoch_id,
        0x85,
        "initial-input",
    )
    .await;
    let successor_epoch = durable_epoch(
        &delta_root.path().join("successor-observations"),
        successor_epoch_id,
        0x86,
        "successor-input",
    )
    .await;
    let third_epoch = durable_epoch(
        &delta_root.path().join("third-observations"),
        third_epoch_id,
        0x87,
        "third-input",
    )
    .await;

    let generation_database = state_root.path().join("writer-generations.sqlite3");
    let generations = Arc::new(
        SqliteWriterGenerationStore::open(&generation_database).expect("writer generation store"),
    );
    let initial_lease = LeaseId::from_bytes(id16(0x88));
    let initial_generation = generations
        .allocate_next(workspace_id, initial_lease)
        .expect("seed initial generation");
    let initial_fence = WriterFence {
        lease_id: initial_lease,
        generation: initial_generation,
    };

    let control_path = delta_root.path().join("activation-control");
    fs::create_dir_all(&control_path).expect("activation control root");
    let control_root = Url::from_directory_path(&control_path).expect("control file URL");
    let (control_v0_pin, control_v0_table) =
        provision_activation_control_history(control_root.clone())
            .await
            .expect("provision activation control");
    let (_, _, _, control_assembly) =
        ProgrammaticFabricEpochBuilder::try_new(EpochId::from_bytes(id16(0x8f)), runtime_config())
            .expect("explicit activation-control session recipe")
            .into_assembly_parts();
    let control_session = Arc::new(control_assembly.candidate_state());
    let control_v0 = Arc::new(
        ActivationControlDeltaProvider::try_from_loaded_table(
            Arc::clone(&control_session),
            control_v0_pin,
            control_v0_table,
        )
        .await
        .expect("bind initial control provider"),
    );
    let initial_proof = ProofReceiptRef::from_bytes(id32(0x89));
    let initial_command = activation_command(
        workspace_id,
        ExpectedHead::Empty,
        initial_epoch_id,
        initial_fence,
        0x8a,
        initial_proof,
        resource_policy_pin,
    );
    let initial_pins = activation_pins(&initial_command, &initial_epoch, initial_proof);
    let initial_seed_event = activation_event(initial_command, initial_pins, 0x8b, None, 1);
    let initial_write = control_v0
        .append_exact(
            initial_seed_event.durable_row(),
            Arc::clone(initial_epoch.observation_publication().table_version_set()),
        )
        .await
        .expect("append initial activation row");
    let ControlledDeltaWriteOutcome::Committed(initial_write) = initial_write else {
        panic!("initial activation append did not commit")
    };
    let control_v1_pin = initial_write.committed().clone();
    let control_v1 = Arc::new(
        ActivationControlDeltaProvider::try_from_loaded_table(
            Arc::clone(&control_session),
            control_v1_pin,
            initial_write.into_table(),
        )
        .await
        .expect("bind committed initial control provider"),
    );
    let activation_authority_v1 = Arc::new(DeltaActivationRuntimeAuthority::new(
        workspace_id,
        Arc::clone(&control_v1),
        generations.clone(),
    ));
    let initial_snapshot = activation_authority_v1
        .current_snapshot()
        .await
        .expect("initial exact activation snapshot");
    let initial_event = *initial_snapshot
        .chain
        .head_event()
        .expect("initial selected event");
    assert_eq!(initial_event.pins(), initial_pins);
    assert_eq!(initial_snapshot.active_fence, initial_fence);

    let successor_template = activation_command(
        workspace_id,
        ExpectedHead::Epoch(initial_epoch_id),
        successor_epoch_id,
        WriterFence {
            lease_id: LeaseId::from_bytes(id16(0x79)),
            generation: super::command::WriterGeneration::new(2).expect("second generation"),
        },
        0x8c,
        ProofReceiptRef::from_bytes(id32(0x8d)),
        resource_policy_pin,
    );
    let successor_pins = activation_pins(
        &successor_template,
        &successor_epoch,
        ProofReceiptRef::from_bytes(id32(0x8d)),
    );
    let successor_query_authority = query_authority(
        workspace_id,
        successor_pins,
        Arc::clone(&successor_epoch),
        release_pins,
        resource_policy_pin,
    );
    let candidate_rebuilder = Arc::new(ExactDeltaActivationCommandCandidateRebuilder::new(
        workspace_id,
        move |epoch_id| {
            Ok::<_, std::convert::Infallible>(epoch_builder(epoch_id, "successor-input"))
        },
    ));
    let activation_state = Arc::new(
        SqliteProgrammaticActivationCommandStateStore::open(
            &state_root.path().join("activation-commands.sqlite3"),
            workspace_id,
            candidate_rebuilder,
            control_v1.control_relation().binding().clone(),
            ActivationReconciliationIdentityPolicy::try_new(
                UnknownCommitReason::ReadbackUnavailable,
                id32(0x8e),
                id32(0x8f),
            )
            .expect("reconciliation identity policy"),
        )
        .expect("activation command state store"),
    );
    let command_factory = command_runtime_factory(
        state_root.path(),
        workspace_id,
        initial_event,
        initial_snapshot.active_fence,
        &control_v1,
        resource_policy_pin,
        Arc::clone(&activation_state),
        successor_pins,
        &successor_query_authority,
        "successor-input",
    );
    let factory =
        ProgrammaticWorkspaceRuntimeFactory::new(Arc::new(PublishedArrowResultRegistry::new()));
    let mut daemon = factory
        .build_daemon(
            [workspace_construction(
                state_root.path(),
                workspace_id,
                &initial_epoch,
                Arc::clone(&activation_authority_v1),
                Arc::clone(&command_factory),
                release_pins,
                resource_policy_pin,
                "initial-input",
            )],
            CommandRecoveryPageSize::new(16).expect("recovery page size"),
            NonZeroUsize::new(4).expect("recovery sweep bound"),
        )
        .await
        .expect("cold programmatic daemon construction");

    let initial_workspace = daemon.workspace(workspace_id).expect("initial workspace");
    let slot = WorkspaceSlot::empty(workspace_id);
    assert!(matches!(
        slot.swap(Arc::clone(&initial_workspace)),
        Err(ActiveWorkspaceError::NotInstalled(observed)) if observed == workspace_id
    ));
    assert!(matches!(
        slot.lease(),
        Err(ActiveWorkspaceError::NotInstalled(observed)) if observed == workspace_id
    ));
    let initially_installed = slot
        .install_initial(Arc::clone(&initial_workspace))
        .expect("initial exact active workspace");
    let pinned_before_swap = slot.lease().expect("initial active-workspace lease");
    let retained = slot
        .swap(Arc::clone(&initial_workspace))
        .expect("atomic active-workspace replacement");
    assert!(Arc::ptr_eq(
        pinned_before_swap.workspace(),
        &initially_installed
    ));
    assert!(Arc::ptr_eq(retained.workspace(), &initially_installed));
    assert!(!Arc::ptr_eq(
        slot.lease().expect("replacement lease").workspace(),
        &initially_installed
    ));
    let relation_pin = initial_epoch
        .observation_publication()
        .table_version_set()
        .pin(RELATION_OBSERVATION_RELATION_ID)
        .expect("relation observation pin")
        .clone();
    assert_eq!(
        initial_workspace.delta_runtime().table_version_set_ref(),
        initial_epoch
            .observation_publication()
            .table_version_set_ref()
    );
    let exact_read = initial_workspace
        .delta_runtime()
        .prepare_semantic_read(
            RELATION_OBSERVATION_RELATION_ID,
            ExactDeltaSemanticReadRequest::new(relation_pin.clone(), None, Vec::new(), None),
        )
        .await
        .expect("daemon exact Delta history read");
    assert_eq!(exact_read.output_schema().fields().len(), 7);
    assert!(
        exact_read
            .output_schema()
            .field_with_name("observation_set_id")
            .is_err()
    );
    assert!(
        exact_read
            .output_schema()
            .field_with_name("row_ordinal")
            .is_err()
    );
    let exact_batches = exact_read
        .execute()
        .await
        .expect("execute exact Delta read");
    assert!(
        exact_batches
            .iter()
            .map(RecordBatch::num_rows)
            .sum::<usize>()
            > 0
    );

    let loaded_history = DeltaTableBuilder::from_url(relation_pin.canonical_root().clone())
        .expect("history builder")
        .with_version(relation_pin.version())
        .load()
        .await
        .expect("load exact history");
    let current_view_contract = Arc::clone(
        &initial_epoch
            .relation(&ProgrammaticRelationId::new(
                RELATION_OBSERVATION_RELATION_ID,
            ))
            .expect("current-view relation binding")
            .contract,
    );
    let exact_read_error = prepare_exact_delta_semantic_read(
        loaded_history,
        ExactDeltaSemanticReadRequest::new(relation_pin, None, Vec::new(), None),
        current_view_contract,
        Arc::new(initial_epoch.context().state()),
    )
    .await
    .expect_err("current-view contract cannot stand in for durable history storage");
    assert!(matches!(
        exact_read_error,
        ExactDeltaSemanticReadError::ProviderContract(_)
    ));
    let public_workspace_id = initial_workspace
        .public_workspace_id()
        .expect("public workspace identity");
    let semantic_backend = production_semantic_backend(&daemon, &initial_workspace);
    let initial_request = released_find_entities_request(
        &public_workspace_id,
        "vertical-initial",
        "initial-input",
        false,
    );
    let initial_query = execute_released_query(
        &semantic_backend,
        &daemon,
        &initial_workspace,
        initial_request,
        0x90,
    )
    .await;
    assert_eq!(initial_query.descriptor().epoch_id, initial_epoch_id);
    assert_eq!(initial_query.descriptor().total_rows, 1);
    drop(initial_workspace);

    let command_fence = daemon
        .command_runtime_handle(workspace_id)
        .expect("registered command runtime")
        .fence();
    assert_eq!(command_fence.generation.get(), 2);
    let successor_command = activation_command(
        workspace_id,
        ExpectedHead::Epoch(initial_epoch_id),
        successor_epoch_id,
        command_fence,
        0x8c,
        successor_pins.proof_receipt,
        resource_policy_pin,
    );
    activation_state
        .persist_request(&ActivationCommandRequestMaterial::new(
            ActivationCommandRequestKey::new(successor_command),
            Arc::clone(&successor_epoch),
            successor_pins,
            ActivationEventId::from_bytes(id32(0x91)),
            CompatibilityClassRef::from_bytes(id32(0x92)),
            RetentionPolicyRef::from_bytes(id32(0x93)),
            OperationSelectionRef::from_bytes(id32(0x94)),
            TransactionRef::from_bytes(id32(0x95)),
            control_v1.control_relation().clone(),
        ))
        .await
        .expect("persist exact activation request");
    let activated = daemon
        .submit_command(successor_command)
        .await
        .expect("activation command traverses registered actor");
    assert_eq!(activated.state().kind(), CommandStateKind::Succeeded);
    assert!(matches!(
        activated.state(),
        DurableCommandState::Succeeded {
            result: CommandResult::EpochActivated { epoch, selection },
            ..
        } if epoch == successor_epoch_id
            && selection == OperationSelectionRef::from_bytes(id32(0x94))
    ));
    let successor_workspace = daemon.workspace(workspace_id).expect("successor workspace");
    assert_eq!(
        successor_workspace.admission().active_head(),
        ExpectedHead::Epoch(successor_epoch_id)
    );
    let successor_request = released_find_entities_request(
        &public_workspace_id,
        "vertical-successor",
        "successor-input",
        true,
    );
    prove_released_scope_budget_and_handoff_causality(&successor_workspace, &successor_request);
    let successor_query = execute_released_query(
        &semantic_backend,
        &daemon,
        &successor_workspace,
        successor_request,
        0x96,
    )
    .await;
    assert_eq!(successor_query.descriptor().epoch_id, successor_epoch_id);
    assert_eq!(successor_query.descriptor().total_rows, 2);
    assert_ne!(
        initial_query.descriptor().total_rows,
        successor_query.descriptor().total_rows,
        "typed provider input must causally change the released query result"
    );
    drop(successor_workspace);

    let missing = activation_command(
        workspace_id,
        ExpectedHead::Epoch(successor_epoch_id),
        third_epoch_id,
        command_fence,
        0x97,
        ProofReceiptRef::from_bytes(id32(0x98)),
        resource_policy_pin,
    );
    assert!(daemon.submit_command(missing).await.is_err());

    daemon.shutdown().await.expect("ordered daemon shutdown");
    drop(daemon);
    drop(command_factory);
    drop(activation_state);
    drop(activation_authority_v1);
    drop(control_v1);
    drop(control_v0);
    drop(generations);

    let control_v2_table = DeltaTableBuilder::from_url(control_root.clone())
        .expect("control root")
        .with_version(2)
        .load()
        .await
        .expect("load exact successor control version");
    let control_v2_pin = super::delta_exact::ExactDeltaPin::new(&control_root, 2)
        .expect("exact successor control pin");
    let (_, _, _, restarted_control_assembly) =
        ProgrammaticFabricEpochBuilder::try_new(EpochId::from_bytes(id16(0x9c)), runtime_config())
            .expect("fresh activation-control session recipe")
            .into_assembly_parts();
    let restarted_control = Arc::new(
        ActivationControlDeltaProvider::try_from_loaded_table(
            Arc::new(restarted_control_assembly.candidate_state()),
            control_v2_pin,
            control_v2_table,
        )
        .await
        .expect("fresh-process control provider"),
    );
    let restarted_generations = Arc::new(
        SqliteWriterGenerationStore::open(&generation_database)
            .expect("reopen writer generation store"),
    );
    let restarted_activation = Arc::new(DeltaActivationRuntimeAuthority::new(
        workspace_id,
        Arc::clone(&restarted_control),
        restarted_generations.clone(),
    ));
    let restarted_snapshot = restarted_activation
        .current_snapshot()
        .await
        .expect("reconstruct activation chain after process loss");
    let restarted_event = *restarted_snapshot
        .chain
        .head_event()
        .expect("restarted selected event");
    assert_eq!(restarted_event.pins(), successor_pins);
    assert_eq!(restarted_snapshot.active_fence, command_fence);

    let third_template = activation_command(
        workspace_id,
        ExpectedHead::Epoch(successor_epoch_id),
        third_epoch_id,
        command_fence,
        0x99,
        ProofReceiptRef::from_bytes(id32(0x9a)),
        resource_policy_pin,
    );
    let third_pins = activation_pins(
        &third_template,
        &third_epoch,
        ProofReceiptRef::from_bytes(id32(0x9a)),
    );
    let third_query_authority = query_authority(
        workspace_id,
        third_pins,
        Arc::clone(&third_epoch),
        release_pins,
        resource_policy_pin,
    );
    let restarted_state = Arc::new(
        SqliteProgrammaticActivationCommandStateStore::open(
            &state_root.path().join("activation-commands.sqlite3"),
            workspace_id,
            Arc::new(ExactDeltaActivationCommandCandidateRebuilder::new(
                workspace_id,
                move |epoch_id| {
                    Ok::<_, std::convert::Infallible>(epoch_builder(epoch_id, "third-input"))
                },
            )),
            restarted_control.control_relation().binding().clone(),
            ActivationReconciliationIdentityPolicy::try_new(
                UnknownCommitReason::ReadbackUnavailable,
                id32(0x8e),
                id32(0x8f),
            )
            .expect("reconciliation identity policy"),
        )
        .expect("reopen activation command state store"),
    );
    let restarted_command_factory = command_runtime_factory(
        state_root.path(),
        workspace_id,
        restarted_event,
        restarted_snapshot.active_fence,
        &restarted_control,
        resource_policy_pin,
        Arc::clone(&restarted_state),
        third_pins,
        &third_query_authority,
        "third-input",
    );
    let restarted_factory =
        ProgrammaticWorkspaceRuntimeFactory::new(Arc::new(PublishedArrowResultRegistry::new()));
    let mut restarted = restarted_factory
        .build_daemon(
            [workspace_construction(
                state_root.path(),
                workspace_id,
                &successor_epoch,
                Arc::clone(&restarted_activation),
                restarted_command_factory,
                release_pins,
                resource_policy_pin,
                "successor-input",
            )],
            CommandRecoveryPageSize::new(16).expect("recovery page size"),
            NonZeroUsize::new(4).expect("recovery sweep bound"),
        )
        .await
        .expect("process-style daemon reopen");
    assert_eq!(
        restarted
            .command_runtime_handle(workspace_id)
            .expect("restarted command runtime")
            .fence()
            .generation
            .get(),
        3
    );
    let restarted_workspace = restarted
        .workspace(workspace_id)
        .expect("restarted workspace");
    let restarted_backend = Arc::new(production_semantic_backend(
        &restarted,
        &restarted_workspace,
    ));
    let restarted_request = released_find_entities_request(
        &public_workspace_id,
        "vertical-restarted",
        "successor-input",
        true,
    );
    let restarted_query = execute_released_query(
        restarted_backend.as_ref(),
        &restarted,
        &restarted_workspace,
        restarted_request,
        0x9b,
    )
    .await;
    assert_eq!(restarted_query.descriptor().epoch_id, successor_epoch_id);
    assert_eq!(restarted_query.descriptor().total_rows, 2);
    drop(restarted_workspace);
    let fastmcp_request = released_find_entities_request(
        &public_workspace_id,
        "vertical-fastmcp-uds",
        "successor-input",
        true,
    );
    exercise_wp37_real_uds_fastmcp_vertical(
        &restarted,
        Arc::clone(&restarted_backend),
        state_root.path(),
        &public_workspace_id,
        &fastmcp_request,
    )
    .await;
    drop(restarted_backend);
    restarted
        .shutdown()
        .await
        .expect("restarted ordered shutdown");
}

mod claim013_artifact_tests {
    use std::collections::BTreeSet;

    use serde_json::{Map, Value};

    use super::*;
    use crate::fabric::activation::{
        ActivationRecoveryAttempt, DurableActivationCommit, DurableActivationRow, TableVersionSet,
    };
    use crate::fabric::activation_control_delta::{
        ActivationControlError, ActivationControlReadback,
    };
    use crate::fabric::activation_transaction::{
        ActivatedEpochReceipt, ActivationAdmissionPosture, ActivationAppendContract,
        ActivationAppendOutcome, ActivationEventPort as _, ActivationOperationMarkerOutcome,
        ActivationOperationMarkerPort as _, ActivationReconciliationReason,
        ActivationReconciliationReceiptCache, ActivationReconciliationTicket,
        ActivationRecoveryCoordinator, ActivationRecoveryRequest, ActivationTransactionOutcome,
        ActivationTransactionRequest, ActivationTransactionStage, DurableSelectionKnowledge,
        IdempotentActivationAcknowledgements,
    };
    use crate::fabric::admission::{AdmissionError, FabricAdmissionRuntime};
    use crate::fabric::programmatic_observation_delta::ProgrammaticObservationDeltaPublication;

    const WP33_EXPECTATIONS: &str =
        include_str!("../../contracts/acceptance/relational-fabric-v3/expectations.jsonl");
    const WP33_FIXTURES: &str =
        include_str!("../../contracts/acceptance/relational-fabric-v3/negative-fixtures.jsonl");
    const OBSERVATION_RELATIONS: [&str; 5] = [
        DEPENDENCY_OBSERVATION_RELATION_ID,
        FIELD_OBSERVATION_RELATION_ID,
        PROVENANCE_OBSERVATION_RELATION_ID,
        RELATION_OBSERVATION_RELATION_ID,
        SCHEMA_OBSERVATION_RELATION_ID,
    ];

    fn wp33_row(document: &str, key: &str, expected: &str) -> Value {
        document
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("valid WP33 JSONL row"))
            .find(|row| row[key] == expected)
            .unwrap_or_else(|| panic!("missing WP33 row {key}={expected}"))
    }

    fn object<'a>(value: &'a Value, context: &str) -> &'a Map<String, Value> {
        value
            .as_object()
            .unwrap_or_else(|| panic!("{context} must be an object"))
    }

    fn array<'a>(value: &'a Value, context: &str) -> &'a [Value] {
        value
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_else(|| panic!("{context} must be an array"))
    }

    fn number(value: &Value, context: &str) -> u64 {
        value
            .as_u64()
            .unwrap_or_else(|| panic!("{context} must be an unsigned integer"))
    }

    fn bytes<const N: usize>(value: &Value, context: &str) -> [u8; N] {
        let encoded = value
            .as_str()
            .unwrap_or_else(|| panic!("{context} must be lower-hex text"));
        assert_eq!(encoded.len(), N * 2, "{context} has the wrong width");
        assert!(
            encoded
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "{context} is not lowercase hexadecimal"
        );
        let mut decoded = [0_u8; N];
        for (index, output) in decoded.iter_mut().enumerate() {
            *output = u8::from_str_radix(&encoded[index * 2..index * 2 + 2], 16)
                .unwrap_or_else(|_| panic!("{context} contains invalid hexadecimal"));
        }
        assert!(decoded.iter().any(|byte| *byte != 0), "{context} is zero");
        decoded
    }

    fn lower_hex(bytes: &[u8]) -> String {
        use std::fmt::Write as _;

        let mut encoded = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            write!(&mut encoded, "{byte:02x}").expect("write to String");
        }
        encoded
    }

    fn exact_keys(object: &Map<String, Value>, expected: &[&str]) -> bool {
        object.keys().map(String::as_str).collect::<BTreeSet<_>>()
            == expected.iter().copied().collect::<BTreeSet<_>>()
    }

    fn table_version_binding_relations(value: &Value) -> Result<BTreeSet<String>, String> {
        let binding = value
            .as_object()
            .ok_or_else(|| "table-version reference is literal or sentinel".to_owned())?;
        if !exact_keys(
            binding,
            &[
                "kind",
                "source",
                "constructor",
                "reference_projection",
                "components",
            ],
        ) || binding["kind"] != "runtime_derived_table_version_set"
            || binding["source"] != "sealed_programmatic_observation_delta_publication"
            || binding["constructor"] != "TableVersionSet::try_new"
            || binding["reference_projection"] != "TableVersionSet::reference"
        {
            return Err("table-version binding is not the production derivation".to_owned());
        }
        let components = binding["components"]
            .as_array()
            .ok_or_else(|| "table-version components are not relational records".to_owned())?;
        let mut relations = BTreeSet::new();
        for component in components {
            let component = component
                .as_object()
                .ok_or_else(|| "table-version component is not a record".to_owned())?;
            if !exact_keys(component, &["relation_id", "exact_delta_pin"]) {
                return Err("table-version component carries unrecognized evidence".to_owned());
            }
            let relation_id = component["relation_id"]
                .as_str()
                .ok_or_else(|| "table-version relation is not text".to_owned())?;
            let pin = component["exact_delta_pin"]
                .as_object()
                .ok_or_else(|| "table-version component authors a literal pin".to_owned())?;
            if !exact_keys(pin, &["root", "version"])
                || pin["root"] != "publication_runtime_root"
                || pin["version"] != "publication_exact_version"
            {
                return Err("table-version component authors a literal or sentinel pin".to_owned());
            }
            if !relations.insert(relation_id.to_owned()) {
                return Err("table-version relation is duplicated".to_owned());
            }
        }
        let required = OBSERVATION_RELATIONS
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        if relations != required {
            return Err("table-version binding is missing or adds a relation".to_owned());
        }
        if relations.contains("control.activation_event.v3") {
            return Err("activation-control backend leaked into the epoch vector".to_owned());
        }
        Ok(relations)
    }

    fn assert_runtime_table_version_binding(
        event: &Value,
        publication: &ProgrammaticObservationDeltaPublication,
        control_root: &Url,
    ) {
        let declared = table_version_binding_relations(&event["pins"]["table_versions"])
            .expect("Claim 013 runtime table-version binding");
        let observed = publication
            .table_versions()
            .map(|(relation_id, pin)| {
                assert_eq!(pin.canonical_root().scheme(), "file");
                assert!(pin.version() > 0);
                assert_ne!(pin.canonical_root(), control_root);
                relation_id.to_owned()
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(observed, declared);
        assert!(!observed.contains("control.activation_event.v3"));

        let independently_derived = TableVersionSet::try_new(
            publication
                .table_versions()
                .map(|(relation_id, pin)| (Arc::<str>::from(relation_id), pin.clone())),
        )
        .expect("derive runtime TableVersionSet from the sealed publication");
        assert_eq!(
            independently_derived,
            publication.table_version_set().as_ref().clone()
        );
        assert_eq!(
            independently_derived.reference(),
            publication.table_version_set_ref()
        );
    }

    fn expected_head(value: &Value) -> ExpectedHead {
        let value = object(value, "Claim 013 expected head");
        match value["kind"].as_str() {
            Some("empty") => ExpectedHead::Empty,
            Some("epoch") => ExpectedHead::Epoch(EpochId::from_bytes(bytes(
                &value["epoch"],
                "predecessor epoch",
            ))),
            other => panic!("unsupported Claim 013 expected-head kind {other:?}"),
        }
    }

    struct PreparedActivation {
        candidate: Arc<ProgrammaticFabricEpoch>,
        request: ActivationTransactionRequest,
        row: DurableActivationRow,
        input_value: &'static str,
    }

    fn prepare_activation(
        event: &Value,
        candidate: Arc<ProgrammaticFabricEpoch>,
        control_relation: super::super::activation::ActivationControlRelationPin,
        control_root: &Url,
        input_value: &'static str,
    ) -> PreparedActivation {
        assert_runtime_table_version_binding(
            event,
            candidate.observation_publication(),
            control_root,
        );
        let event_object = object(event, "Claim 013 activation event");
        let command_value = object(&event_object["command"], "Claim 013 FabricCommand");
        let identity = object(&command_value["identity"], "Claim 013 command identity");
        let ownership = object(&command_value["ownership"], "Claim 013 command ownership");
        let command_pins = object(&command_value["pins"], "Claim 013 command pins");
        let payload = object(&command_value["payload"], "Claim 013 command payload");
        assert_eq!(payload["kind"], "ActivateEpoch");

        let fence_value = object(&event_object["execution_fence"], "Claim 013 writer fence");
        let writer_fence = WriterFence {
            lease_id: LeaseId::from_bytes(bytes(&fence_value["lease_id"], "lease ID")),
            generation: super::super::command::WriterGeneration::new(number(
                &fence_value["generation"],
                "writer generation",
            ))
            .expect("nonzero Claim 013 writer generation"),
        };
        let command = FabricCommand {
            identity: CommandIdentity {
                operation_id: OperationId::from_bytes(bytes(
                    &identity["operation_id"],
                    "operation ID",
                )),
                idempotency_key: IdempotencyKey::from_bytes(bytes(
                    &identity["idempotency_key"],
                    "idempotency key",
                )),
            },
            ownership: CommandOwnership {
                workspace_id: WorkspaceId::from_bytes(bytes(
                    &ownership["workspace_id"],
                    "workspace ID",
                )),
                principal_id: PrincipalId::from_bytes(bytes(
                    &ownership["principal_id"],
                    "principal ID",
                )),
                authorization: AuthorizationRef::from_bytes(bytes(
                    &ownership["authorization"],
                    "authorization",
                )),
            },
            expected_head: expected_head(&command_value["expected_head"]),
            writer_fence,
            pins: CommandPins {
                input_release: InputReleaseRef::from_bytes(bytes(
                    &command_pins["input_release"],
                    "input release",
                )),
                program_release: ProgramReleaseRef::from_bytes(bytes(
                    &command_pins["program_release"],
                    "program release",
                )),
                application_release: super::super::command::ApplicationReleaseRef::from_bytes(
                    bytes(&command_pins["application_release"], "application release"),
                ),
                source_authority: super::super::command::SourceAuthorityRef::from_bytes(bytes(
                    &command_pins["source_authority"],
                    "source authority",
                )),
                provider_release: super::super::command::ProviderReleaseRef::from_bytes(bytes(
                    &command_pins["provider_release"],
                    "provider release",
                )),
                source_generation: SourceGeneration::new(number(
                    &command_pins["source_generation"],
                    "source generation",
                )),
                provider_set: ProviderSetRef::from_bytes(bytes(
                    &command_pins["provider_set"],
                    "provider set",
                )),
            },
            resources: ResourceEnvelopeRef::from_bytes(bytes(
                &command_value["resources"],
                "resource envelope",
            )),
            payload: FabricCommandPayload::ActivateEpoch {
                candidate_epoch: EpochId::from_bytes(bytes(
                    &payload["candidate_epoch"],
                    "candidate epoch",
                )),
                proof_receipt: ProofReceiptRef::from_bytes(bytes(
                    &payload["proof_receipt"],
                    "proof receipt",
                )),
            },
        };
        let pins_value = object(&event_object["pins"], "Claim 013 FabricEpochPins");
        let pins = FabricEpochPins {
            epoch: EpochId::from_bytes(bytes(&pins_value["epoch"], "epoch")),
            input_release: InputReleaseRef::from_bytes(bytes(
                &pins_value["input_release"],
                "input release pin",
            )),
            program_release: ProgramReleaseRef::from_bytes(bytes(
                &pins_value["program_release"],
                "program release pin",
            )),
            application_release: super::super::command::ApplicationReleaseRef::from_bytes(bytes(
                &pins_value["application_release"],
                "application release pin",
            )),
            source_authority: super::super::command::SourceAuthorityRef::from_bytes(bytes(
                &pins_value["source_authority"],
                "source authority pin",
            )),
            source_generation: SourceGeneration::new(number(
                &pins_value["source_generation"],
                "source generation pin",
            )),
            provider_release: super::super::command::ProviderReleaseRef::from_bytes(bytes(
                &pins_value["provider_release"],
                "provider release pin",
            )),
            provider_set: ProviderSetRef::from_bytes(bytes(
                &pins_value["provider_set"],
                "provider set pin",
            )),
            table_versions: candidate.observation_publication().table_version_set_ref(),
            overlay_segments: OverlaySegmentSetRef::from_bytes(bytes(
                &pins_value["overlay_segments"],
                "overlay segments",
            )),
            policy_set: PolicySetRef::from_bytes(bytes(&pins_value["policy_set"], "policy set")),
            resource_envelope: ResourceEnvelopeRef::from_bytes(bytes(
                &pins_value["resource_envelope"],
                "resource envelope pin",
            )),
            proof_receipt: ProofReceiptRef::from_bytes(bytes(
                &pins_value["proof_receipt"],
                "proof receipt pin",
            )),
        };
        let attempt = ActivationAttempt::for_test(
            command,
            1,
            ExecutionOwner {
                actor_id: ActorId::from_bytes([0x13; 16]),
                fence: writer_fence,
            },
        );
        let durable = object(&event_object["durable_commit"], "Claim 013 durable commit");
        let compatibility = CompatibilityClassRef::from_bytes(bytes(
            &event_object["compatibility_class"],
            "compatibility class",
        ));
        let retention = RetentionPolicyRef::from_bytes(bytes(
            &event_object["retention_policy"],
            "retention policy",
        ));
        let operation_selection = OperationSelectionRef::from_bytes(bytes(
            &durable["operation_selection"],
            "operation selection",
        ));
        let transaction =
            TransactionRef::from_bytes(bytes(&durable["transaction"], "activation transaction"));
        let row = DurableActivationRow::try_from_attempt(
            ActivationEventId::from_bytes(bytes(&event_object["event_id"], "event ID")),
            attempt,
            event_object["predecessor_event_id"].as_str().map(|_| {
                ActivationEventId::from_bytes(bytes(
                    &event_object["predecessor_event_id"],
                    "predecessor event ID",
                ))
            }),
            ActivationOrdinal::new(number(&event_object["ordinal"], "activation ordinal"))
                .expect("nonzero Claim 013 activation ordinal"),
            pins,
            compatibility,
            retention,
            DurableActivationCommit {
                operation_selection,
                transaction,
            },
        )
        .expect("artifact event is a valid production activation row");
        assert_eq!(
            *row.workspace_id.as_bytes(),
            bytes::<16>(&event_object["workspace_id"], "event workspace")
        );
        assert_eq!(
            *row.operation_id.as_bytes(),
            bytes::<16>(&event_object["operation_id"], "event operation")
        );
        assert_eq!(
            row.predecessor_epoch,
            expected_head(&event_object["predecessor_epoch"])
        );
        let request = ActivationTransactionRequest::try_new(
            attempt,
            Arc::clone(&candidate),
            pins,
            row.event_id,
            compatibility,
            retention,
            operation_selection,
            transaction,
            control_relation,
        )
        .expect("artifact event binds the sealed candidate and production request");
        PreparedActivation {
            candidate,
            request,
            row,
            input_value,
        }
    }

    struct CommittedActivation {
        event: ActivationEvent,
        chain: super::super::activation::ActivationChain,
        provider: Arc<ActivationControlDeltaProvider>,
        readback: ActivationControlReadback,
    }

    async fn load_control_provider(
        root: &Url,
        version: u64,
        session: Arc<datafusion::execution::context::SessionState>,
    ) -> Arc<ActivationControlDeltaProvider> {
        let pin = super::super::delta_exact::ExactDeltaPin::new(root, version)
            .expect("Claim 013 exact activation-control pin");
        let table = DeltaTableBuilder::from_url(root.clone())
            .expect("Claim 013 activation-control builder")
            .with_version(version)
            .load()
            .await
            .expect("Claim 013 exact activation-control table");
        Arc::new(
            ActivationControlDeltaProvider::try_from_loaded_table(session, pin, table)
                .await
                .expect("Claim 013 exact activation-control provider"),
        )
    }

    async fn append_and_readback(
        prepared: &PreparedActivation,
        predecessor: Arc<ActivationControlDeltaProvider>,
        session: Arc<datafusion::execution::context::SessionState>,
    ) -> CommittedActivation {
        let contract = ActivationAppendContract::for_test(
            prepared.request.attempt(),
            prepared.row,
            Arc::clone(
                prepared
                    .candidate
                    .observation_publication()
                    .table_version_set(),
            ),
            predecessor.control_relation().clone(),
        );
        let (event, table_versions, chain) = match predecessor.append_and_readback(contract).await {
            ActivationAppendOutcome::Committed {
                event,
                table_versions,
                chain_after_readback,
            } => (event, table_versions, chain_after_readback),
            other => panic!("Claim 013 activation append was not committed: {other:?}"),
        };
        assert_eq!(event.durable_row(), prepared.row);
        assert_eq!(
            table_versions.as_ref(),
            prepared
                .candidate
                .observation_publication()
                .table_version_set()
                .as_ref()
        );
        let committed_version = predecessor.control_relation().table().version() + 1;
        let committed = load_control_provider(
            predecessor.control_relation().table().canonical_root(),
            committed_version,
            session,
        )
        .await;
        let readback = committed
            .read_workspace(prepared.row.workspace_id, prepared.row.execution_fence)
            .await
            .expect("Claim 013 exact workspace readback");
        let persisted = readback
            .rows()
            .iter()
            .find(|persisted| persisted.row().event_id == prepared.row.event_id)
            .expect("Claim 013 persisted event exists in exact readback");
        assert_eq!(persisted.row(), prepared.row);
        assert_eq!(
            readback
                .table_versions_for_event(prepared.row.event_id)
                .expect("Claim 013 readback carries the complete table vector")
                .reference(),
            prepared
                .candidate
                .observation_publication()
                .table_version_set_ref()
        );
        CommittedActivation {
            event,
            chain,
            provider: committed,
            readback,
        }
    }

    fn recovery_ticket(request: &ActivationTransactionRequest) -> ActivationReconciliationTicket {
        ActivationReconciliationTicket {
            stage: ActivationTransactionStage::DurableAppendReadback,
            reason: ActivationReconciliationReason::OperationMarkerUnknown(
                DiagnosticRef::from_bytes([0x13; 32]),
            ),
            workspace_id: request.command().ownership.workspace_id,
            operation_id: request.command().identity.operation_id,
            candidate_epoch: request.pins().epoch,
            expected_head: request.command().expected_head,
            execution_fence: request.execution_fence(),
            event_id: request.event_id(),
            transaction: request.transaction(),
            operation_selection: request.operation_selection(),
            durable_selection: DurableSelectionKnowledge::Unknown,
            admission_posture: ActivationAdmissionPosture::Closed,
        }
    }

    async fn recover_selected(
        prepared: &PreparedActivation,
        committed: &CommittedActivation,
    ) -> (ActivatedEpochReceipt, Arc<ProgrammaticFabricEpoch>) {
        let admission = Arc::new(
            FabricAdmissionRuntime::recover_unmaterialized_for_reconciliation(&committed.chain)
                .expect("Claim 013 fail-closed admission runtime"),
        );
        assert_eq!(
            admission.admit().unwrap_err(),
            AdmissionError::AdmissionClosed
        );
        let cache = Arc::new(ActivationReconciliationReceiptCache::new(
            prepared.row.workspace_id,
        ));
        assert!(
            cache
                .current_receipt()
                .expect("read empty receipt cache")
                .is_none()
        );
        let selected_epoch = *prepared.candidate.identity();
        let input_value = prepared.input_value;
        let recovery = ActivationRecoveryCoordinator::new(
            Arc::clone(&admission),
            Arc::clone(&committed.provider),
            Arc::new(ExactDeltaProgrammaticEpochRebuilder::new(move |epoch_id| {
                if epoch_id != selected_epoch {
                    return Err("Claim 013 selected another epoch");
                }
                Ok(epoch_builder(epoch_id, input_value))
            })),
            Arc::clone(&cache),
            Arc::new(IdempotentActivationAcknowledgements::new(
                prepared.row.workspace_id,
            )),
        );
        let recovery_attempt = ActivationRecoveryAttempt::for_test(
            prepared.request.attempt(),
            ExecutionOwner {
                actor_id: ActorId::from_bytes([0x14; 16]),
                fence: prepared.row.execution_fence,
            },
        );
        let outcome = recovery
            .recover(
                prepared.request.recovery_request(),
                recovery_ticket(&prepared.request),
                recovery_attempt,
            )
            .await;
        let ActivationTransactionOutcome::Activated(receipt) = outcome else {
            panic!("Claim 013 exact marker recovery did not activate: {outcome:?}")
        };
        assert_eq!(receipt.event, committed.event);
        assert_eq!(receipt.cache.event_id, committed.event.event_id());
        assert_eq!(receipt.acknowledgement.event_id, committed.event.event_id());
        assert_eq!(
            cache
                .current_receipt()
                .expect("read reconciled receipt cache"),
            Some(receipt.cache)
        );
        let admitted = admission.admit().expect("Claim 013 admission reopened");
        let reopened = Arc::clone(admitted.epoch());
        assert_ne!(
            reopened.context().state().session_id(),
            prepared.candidate.context().state().session_id()
        );
        assert_eq!(
            reopened.observation_publication().table_version_set_ref(),
            prepared
                .candidate
                .observation_publication()
                .table_version_set_ref()
        );
        assert_eq!(
            reopened
                .observation_publication()
                .table_version_set()
                .as_ref(),
            prepared
                .candidate
                .observation_publication()
                .table_version_set()
                .as_ref()
        );
        (receipt, reopened)
    }

    fn assert_decoded_outcome(
        expected: &Value,
        event_artifact: &Value,
        prepared: &PreparedActivation,
        committed: &CommittedActivation,
        receipt: ActivatedEpochReceipt,
    ) {
        assert_eq!(
            expected["selected_event_id"],
            lower_hex(receipt.event.event_id().as_bytes())
        );
        assert_eq!(
            expected["selected_epoch"],
            lower_hex(receipt.event.pins().epoch.as_bytes())
        );
        assert_eq!(expected["command"], event_artifact["command"]);
        assert_eq!(expected["fabric_epoch_pins"], event_artifact["pins"]);
        assert_eq!(expected["durable_commit"], event_artifact["durable_commit"]);
        assert_eq!(
            number(
                &expected["backend_observation"]["control_predecessor_version"],
                "Claim 013 expected control predecessor",
            ),
            prepared.request.control_relation().table().version()
        );
        assert_eq!(
            number(
                &expected["backend_observation"]["control_commit_version"],
                "Claim 013 expected control commit",
            ),
            committed.provider.control_relation().table().version()
        );
        assert_eq!(
            number(
                &expected["readback"]["control_commit_version"],
                "Claim 013 expected readback commit",
            ),
            committed.provider.control_relation().table().version()
        );
        assert_eq!(expected["installation"]["state"], "installed");
        assert_eq!(
            expected["receipt_cache_reconciliation"],
            "complete_non_authoritative"
        );
        assert_eq!(expected["acknowledgement"]["state"], "acknowledged");
        assert_eq!(expected["admission_state"], "open");
        assert_eq!(expected["candidate_present_during_reconcile"], false);
        assert_eq!(expected["receipt_cache_authoritative"], false);
        assert_eq!(
            committed
                .readback
                .table_versions_for_event(receipt.event.event_id())
                .expect("Claim 013 outcome table versions")
                .reference(),
            receipt.event.pins().table_versions
        );
    }

    async fn initial_control(
        root: &Url,
    ) -> (
        Arc<datafusion::execution::context::SessionState>,
        Arc<ActivationControlDeltaProvider>,
    ) {
        let (pin, table) = provision_activation_control_history(root.clone())
            .await
            .expect("Claim 013 activation-control history");
        let (_, _, _, assembly) = ProgrammaticFabricEpochBuilder::try_new(
            EpochId::from_bytes([0x71; 16]),
            runtime_config(),
        )
        .expect("Claim 013 control session")
        .into_assembly_parts();
        let session = Arc::new(assembly.candidate_state());
        let provider = Arc::new(
            ActivationControlDeltaProvider::try_from_loaded_table(Arc::clone(&session), pin, table)
                .await
                .expect("Claim 013 initial control provider"),
        );
        (session, provider)
    }

    #[test]
    fn wp38_claim_013_binding_rejects_missing_extra_and_literal_relations() {
        let claim = wp33_row(WP33_EXPECTATIONS, "claim_id", "RFV3-CLAIM-013");
        let event = &claim["complete_input_universe"]["inputs"]["activation_chain"]["events"][0];
        assert_eq!(
            table_version_binding_relations(&event["pins"]["table_versions"])
                .expect("accepted Claim 013 table binding"),
            OBSERVATION_RELATIONS
                .into_iter()
                .map(str::to_owned)
                .collect::<BTreeSet<_>>()
        );

        let mut missing = event["pins"]["table_versions"].clone();
        missing["components"]
            .as_array_mut()
            .expect("components")
            .pop();
        assert!(table_version_binding_relations(&missing).is_err());

        let mut extra = event["pins"]["table_versions"].clone();
        extra["components"]
            .as_array_mut()
            .expect("components")
            .push(serde_json::json!({
                "relation_id": "fact.entity",
                "exact_delta_pin": {
                    "root": "publication_runtime_root",
                    "version": "publication_exact_version"
                }
            }));
        assert!(table_version_binding_relations(&extra).is_err());

        let literal = Value::String("b001b001b001b001b001b001b001b001".to_owned());
        assert!(table_version_binding_relations(&literal).is_err());
        let mut literal_pin = event["pins"]["table_versions"].clone();
        literal_pin["components"][0]["exact_delta_pin"] =
            serde_json::json!({"root": "file:///tmp/fixed", "version": 1});
        assert!(table_version_binding_relations(&literal_pin).is_err());
    }

    #[tokio::test]
    async fn wp38_claim_013_positive_recovers_the_artifact_bound_exact_epoch() {
        let claim = wp33_row(WP33_EXPECTATIONS, "claim_id", "RFV3-CLAIM-013");
        let event = &claim["complete_input_universe"]["inputs"]["activation_chain"]["events"][0];
        let delta = TempDir::new().expect("Claim 013 positive Delta root");
        let observation_root = delta.path().join("observations");
        let control_path = delta.path().join("activation-control");
        fs::create_dir_all(&control_path).expect("Claim 013 control path");
        let control_root = Url::from_directory_path(&control_path).expect("Claim 013 control URL");
        let candidate = durable_epoch(
            &observation_root,
            EpochId::from_bytes(bytes(&event["pins"]["epoch"], "Claim 013 epoch")),
            0x91,
            "initial-input",
        )
        .await;
        let (session, control_v0) = initial_control(&control_root).await;
        let prepared = prepare_activation(
            event,
            candidate,
            control_v0.control_relation().clone(),
            &control_root,
            "initial-input",
        );
        let committed = append_and_readback(&prepared, control_v0, session).await;
        let (receipt, _) = recover_selected(&prepared, &committed).await;
        assert_decoded_outcome(
            &claim["decoded_expectation"]["rows"][0][0],
            event,
            &prepared,
            &committed,
            receipt,
        );
    }

    #[tokio::test]
    async fn wp38_claim_013_causal_new_head_changes_the_recovered_exact_epoch() {
        let causal = wp33_row(WP33_FIXTURES, "fixture_id", "RFV3-FIX-013-C");
        assert_eq!(causal["kind"], "causal");
        assert_eq!(causal["expected_terminal"], "changed");
        let events = array(
            &causal["mutation"]["after"]["events"],
            "Claim 013 causal events",
        );
        assert_eq!(events.len(), 2);
        let delta = TempDir::new().expect("Claim 013 causal Delta root");
        let control_path = delta.path().join("activation-control");
        fs::create_dir_all(&control_path).expect("Claim 013 causal control path");
        let control_root = Url::from_directory_path(&control_path).expect("Claim 013 control URL");
        let first_candidate = durable_epoch(
            &delta.path().join("first-observations"),
            EpochId::from_bytes(bytes(&events[0]["pins"]["epoch"], "first epoch")),
            0x92,
            "initial-input",
        )
        .await;
        let second_candidate = durable_epoch(
            &delta.path().join("second-observations"),
            EpochId::from_bytes(bytes(&events[1]["pins"]["epoch"], "second epoch")),
            0x93,
            "successor-input",
        )
        .await;
        let (session, control_v0) = initial_control(&control_root).await;
        let first = prepare_activation(
            &events[0],
            first_candidate,
            control_v0.control_relation().clone(),
            &control_root,
            "initial-input",
        );
        let first_commit = append_and_readback(&first, control_v0, Arc::clone(&session)).await;
        let second = prepare_activation(
            &events[1],
            second_candidate,
            first_commit.provider.control_relation().clone(),
            &control_root,
            "successor-input",
        );
        let second_commit = append_and_readback(&second, first_commit.provider, session).await;
        assert_ne!(first.row.event_id, second.row.event_id);
        assert_ne!(
            first.row.pins.table_versions,
            second.row.pins.table_versions
        );
        assert_eq!(second_commit.chain.events().len(), 2);
        let (receipt, reopened) = recover_selected(&second, &second_commit).await;
        assert_eq!(*reopened.identity(), second.row.pins.epoch);
        assert_decoded_outcome(
            &causal["expected_decoded"],
            &events[1],
            &second,
            &second_commit,
            receipt,
        );
    }

    #[tokio::test]
    async fn wp38_claim_013_negative_transaction_mismatch_keeps_admission_closed() {
        let claim = wp33_row(WP33_EXPECTATIONS, "claim_id", "RFV3-CLAIM-013");
        let negative = wp33_row(WP33_FIXTURES, "fixture_id", "RFV3-FIX-013-N");
        assert_eq!(negative["kind"], "negative");
        assert_eq!(negative["expected_terminal"], "reject");
        assert_eq!(
            negative["mutation"]["json_pointer"],
            "/events/0/readback/transaction"
        );
        let event = &claim["complete_input_universe"]["inputs"]["activation_chain"]["events"][0];
        let delta = TempDir::new().expect("Claim 013 negative Delta root");
        let control_path = delta.path().join("activation-control");
        fs::create_dir_all(&control_path).expect("Claim 013 negative control path");
        let control_root = Url::from_directory_path(&control_path).expect("Claim 013 control URL");
        let candidate = durable_epoch(
            &delta.path().join("observations"),
            EpochId::from_bytes(bytes(&event["pins"]["epoch"], "negative epoch")),
            0x94,
            "initial-input",
        )
        .await;
        let (session, control_v0) = initial_control(&control_root).await;
        let prepared = prepare_activation(
            event,
            candidate,
            control_v0.control_relation().clone(),
            &control_root,
            "initial-input",
        );
        let committed = append_and_readback(&prepared, control_v0, session).await;
        let contradictory_transaction = TransactionRef::from_bytes(bytes(
            &negative["mutation"]["after"],
            "contradictory readback transaction",
        ));
        assert_ne!(contradictory_transaction, prepared.row.commit.transaction);
        let recovery_request = ActivationRecoveryRequest::try_new(
            prepared.request.attempt(),
            prepared.request.pins(),
            prepared.request.event_id(),
            prepared.request.compatibility(),
            prepared.request.retention(),
            prepared.request.operation_selection(),
            contradictory_transaction,
            prepared.request.control_relation().clone(),
        )
        .expect("construct candidate-free contradictory recovery request");
        let marker_request =
            crate::fabric::activation_transaction::ActivationOperationMarkerRequest {
                workspace_id: prepared.row.workspace_id,
                operation_id: prepared.row.operation_id,
                event_id: prepared.row.event_id,
                expected_head: prepared.row.predecessor_epoch,
                execution_fence: prepared.row.execution_fence,
                active_recovery_fence: prepared.row.execution_fence,
                transaction: contradictory_transaction,
                operation_selection: prepared.row.commit.operation_selection,
                control_relation: prepared.request.control_relation().clone(),
            };
        assert!(matches!(
            committed
                .provider
                .reconcile_operation(&marker_request)
                .await,
            Err(ActivationControlError::MarkerRowDisagreement)
        ));
        assert!(matches!(
            committed
                .provider
                .read_operation_marker(marker_request)
                .await,
            ActivationOperationMarkerOutcome::Unknown { .. }
        ));

        let admission = Arc::new(
            FabricAdmissionRuntime::recover_unmaterialized_for_reconciliation(&committed.chain)
                .expect("negative fail-closed admission runtime"),
        );
        let recovery = ActivationRecoveryCoordinator::new(
            Arc::clone(&admission),
            Arc::clone(&committed.provider),
            Arc::new(ExactDeltaProgrammaticEpochRebuilder::new(move |epoch_id| {
                Ok::<_, std::convert::Infallible>(epoch_builder(epoch_id, "initial-input"))
            })),
            Arc::new(ActivationReconciliationReceiptCache::new(
                prepared.row.workspace_id,
            )),
            Arc::new(IdempotentActivationAcknowledgements::new(
                prepared.row.workspace_id,
            )),
        );
        let mut ticket = recovery_ticket(&prepared.request);
        ticket.transaction = contradictory_transaction;
        let outcome = recovery
            .recover(
                recovery_request,
                ticket,
                ActivationRecoveryAttempt::for_test(
                    prepared.request.attempt(),
                    ExecutionOwner {
                        actor_id: ActorId::from_bytes([0x15; 16]),
                        fence: prepared.row.execution_fence,
                    },
                ),
            )
            .await;
        assert!(matches!(
            outcome,
            ActivationTransactionOutcome::ReconciliationNeeded(ActivationReconciliationTicket {
                stage: ActivationTransactionStage::DurableAppendReadback,
                reason: ActivationReconciliationReason::OperationMarkerUnknown(_),
                admission_posture: ActivationAdmissionPosture::Closed,
                ..
            })
        ));
        assert_eq!(
            admission.admit().unwrap_err(),
            AdmissionError::AdmissionClosed
        );
        assert_eq!(
            negative["expected_decoded"],
            serde_json::json!({
                "error": "ACTIVATION_TRANSACTION_READBACK_MISMATCH",
                "admission_state": "closed"
            })
        );
    }
}
