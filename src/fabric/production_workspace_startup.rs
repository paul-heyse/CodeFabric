//! FreshActivation and exact selected-epoch recovery for one supervisor-owned workspace.
//!
//! This is the production composition root between operational registration, the held OS writer
//! lease, source capture, compiled provider/analysis authority, exact Delta histories, the
//! command actor, and the atomic workspace slot.  It has no predecessor-model input and no
//! process-local semantic fallback.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::num::NonZeroUsize;
use std::os::unix::ffi::OsStringExt as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use datafusion::execution::SessionStateBuilder;
use deltalake::DeltaTableBuilder;
use deltalake::delta_datafusion::planner::DeltaPlanner;
use thiserror::Error;
use url::Url;

use super::activation::{
    ActivationControlRelationPin, ActivationEventId, CompatibilityClassRef, FabricEpochPins,
    OverlaySegmentSetRef, PolicySetRef,
};
use super::activation_control_delta::{
    ActivationControlDeltaProvider, DeltaActivationRuntimeAuthority,
    ExactActivationControlSelection, provision_activation_control_history,
};
use super::command::{
    ActorId, ApplicationReleaseRef, AuthorizationDecision, AuthorizationRef, CommandIdentity,
    CommandOwnership, CommandPins, CommandResult, DiagnosticRef, DurableCommandState, EpochId,
    ExpectedHead, FabricCommand, FabricCommandPayload, IdempotencyKey, InputReleaseRef,
    OperationId, OperationSelectionRef, PrincipalId, ProgramReleaseRef, ProofReceiptRef,
    ProviderReleaseRef, ProviderSetRef, ResourceEnvelopeRef, RetentionPolicyRef,
    SourceAuthorityRef, SourceGeneration, SourceImageSetRef, TransactionRef, UnknownCommitReason,
    WorkspaceId,
};
use super::command_actor::{CommandPortError, FabricCommandActorConfig};
use super::command_record_sqlite::CommandRecoveryPageSize;
use super::command_runtime::{
    FabricCommandRuntime, FabricCommandRuntimeConfig, InterruptedCommitDiagnosticPort,
};
use super::command_runtime_ports::{
    CommandAuthorizationPort, InterruptedCommitDiagnosticQuery,
    InterruptedCommitDiagnosticRelationPort, RelationalCommandSemanticContext,
    RelationalInterruptedCommitDiagnostics,
};
use super::delta_cdf_checkpoint_sqlite::SqliteDeltaCdfCheckpointStore;
use super::delta_exact::ExactDeltaPin;
use super::delta_guarded_maintenance::{
    DeltaMaintenanceAuthorityError, DeltaMaintenanceSafetyEvidence, DeltaMaintenanceSafetyPort,
};
use super::delta_write::ControlledDeltaWriteAssuranceFault;
use super::epoch_runtime::FabricEpochRuntimeConfig;
use super::production_kernel::{
    ActiveWorkspaceError, CompiledSemanticRelease, SelectedEpochRecord, WorkspaceSlot,
};
use super::programmatic_activation_admission::ReleaseOwnedActiveWorkspaceBuilder;
use super::programmatic_activation_command_ports::{
    ActivationCandidateProofRelationsPort, ActivationCommandRequestKey,
    ActivationCommandRequestMaterial,
};
use super::programmatic_activation_command_sqlite::{
    ActivationCommandCandidateRebuilderPort, ActivationReconciliationIdentityPolicy,
    ExactDeltaActivationCommandCandidateRebuilder, SqliteProgrammaticActivationCommandStateStore,
};
use super::programmatic_active_workspace_builder::{
    ProductionActiveWorkspaceBuilder, ProductionActiveWorkspaceConfig,
};
use super::programmatic_command_capability::ProgrammaticCommandCapabilityDisposition;
use super::programmatic_command_runtime_factory::{
    ExactProgrammaticCommandEffectClosure, ProgrammaticActivationCommandEffects,
    ProgrammaticCommandCapabilityGapInput, ProgrammaticNonActivationCommandEffects,
};
use super::programmatic_delta_runtime::ProgrammaticDeltaRuntimePorts;
use super::programmatic_epoch::{ProgrammaticFabricEpoch, ProgrammaticFabricEpochBuilder};
use super::programmatic_observation_delta::ProgrammaticObservationWriteIdentity;
use super::programmatic_query_backend::compiled_query_release_pin;
use super::programmatic_relation_delta::{
    ProgrammaticRelationDeltaLayout, ProgrammaticRelationDeltaPreparation,
};
use super::programmatic_schema::{
    DEPENDENCY_OBSERVATION_RELATION_ID, FIELD_OBSERVATION_RELATION_ID,
    PROVENANCE_OBSERVATION_RELATION_ID, ProgrammaticRelationId, RELATION_OBSERVATION_RELATION_ID,
    SCHEMA_OBSERVATION_RELATION_ID,
};
use super::proof::{
    DeltaActivationCandidateProofRelations, ProofCandidatePins, ProofDeltaWorkspaceRoot,
    ProofDeltaWriteIdentity, persist_proof_relations, provision_proof_relation_histories,
};
use super::published_arrow_result::PublishedArrowResultRegistry;
use super::switchable_activation_authority::SwitchableActivationAuthority;
use super::writer_generation_sqlite::SqliteWriterGenerationStore;
use super::writer_lease::WorkspaceWriterLease;
use crate::cancellation::Cancellation;
use crate::inventory::{InclusionState, InventoryLimits, InventoryWalker};
use crate::operational_store::OperationalStore;
use crate::production_provider_recipe::{
    ExactProviderLaneAuthority, ProductionProviderAuthority, ProductionProviderRuns,
};
use crate::provider_admission::{ExactProviderLaneRuns, ProviderLaneGap};
use crate::provider_contracts::{
    CancellationProbe, ContextIdentity, ProviderContextBinding, ProviderContractError,
    ProviderLane, ProviderResourceCeilingSpec, ProviderResourceCeilings, ProviderRunBinding,
    ProviderRunIdentity, ProviderScopeIdentity, ProviderSourceBinding, SourceIdentity,
};
use crate::provider_native_syntax::{
    ExactPythonSyntaxRunner, InProcessProviderJobs, ProviderNativeSourceImage,
    ProviderNativeSyntaxRun, PythonModuleInput,
};
use crate::relation_ipc::{ContextPin, SourcePin};
use crate::secure_path::{PlatformPath, open_workspace_root};
use crate::semantic_release::ProviderJobInput;
use crate::source_image::{
    CaptureOutcome, CaptureRequest, SourceBlobHolderKind, SourceCapturePolicy, SourceImageStore,
    SourceLanguage, advance_source_generation, current_source_generation,
};
use crate::workspace_registry::WorkspaceRecord;

/// Joined owner retained by the daemon after one workspace reaches queryable authority.
pub(crate) struct ProductionWorkspaceStartup {
    command_runtime: FabricCommandRuntime,
    selected_epoch: EpochId,
    fresh_activation: bool,
}

/// Bounded assurance interruption admitted only by the daemon's debug-build configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionWorkspaceStartupAssuranceFault {
    DurableAppendAcknowledgementLostBeforeReadback,
}

impl ProductionWorkspaceStartup {
    #[must_use]
    pub(crate) const fn selected_epoch(&self) -> EpochId {
        self.selected_epoch
    }

    #[must_use]
    pub(crate) const fn fresh_activation(&self) -> bool {
        self.fresh_activation
    }

    pub(crate) async fn shutdown(self) -> Result<(), ProductionWorkspaceStartupError> {
        self.command_runtime
            .shutdown()
            .await
            .map_err(|error| step("command-runtime-shutdown", error))
    }
}

#[derive(Debug, Error)]
#[error("production workspace startup failed during {step}: {detail}")]
pub(crate) struct ProductionWorkspaceStartupError {
    step: &'static str,
    detail: String,
}

fn step(step: &'static str, error: impl std::fmt::Display) -> ProductionWorkspaceStartupError {
    ProductionWorkspaceStartupError {
        step,
        detail: error.to_string(),
    }
}

async fn shutdown_after_startup_error(
    runtime: FabricCommandRuntime,
    primary: ProductionWorkspaceStartupError,
) -> ProductionWorkspaceStartupError {
    match runtime.shutdown().await {
        Ok(()) => primary,
        Err(shutdown) => step(
            "post-startup-command-runtime-cleanup",
            format!("{primary}; joined command-runtime shutdown also failed: {shutdown}"),
        ),
    }
}

fn private_directory(path: &Path) -> Result<(), ProductionWorkspaceStartupError> {
    fs::create_dir_all(path).map_err(|error| step("private-directory-create", error))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| step("private-directory-permissions", error))?;
    let metadata =
        fs::symlink_metadata(path).map_err(|error| step("private-directory-metadata", error))?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(step(
            "private-directory-validation",
            "path is not a private non-symlink directory",
        ));
    }
    Ok(())
}

fn digest32(domain: &[u8], frames: &[&[u8]]) -> [u8; 32] {
    let mut digest = blake3::Hasher::new();
    digest.update(domain);
    for frame in frames {
        digest.update(&(frame.len() as u64).to_be_bytes());
        digest.update(frame);
    }
    *digest.finalize().as_bytes()
}

fn digest16(domain: &[u8], frames: &[&[u8]]) -> [u8; 16] {
    let digest = digest32(domain, frames);
    let mut result = [0_u8; 16];
    result.copy_from_slice(&digest[..16]);
    result
}

fn lower_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

struct DenyMaintenance;

#[async_trait]
impl DeltaMaintenanceSafetyPort for DenyMaintenance {
    async fn observe(
        &self,
        _target: &ExactDeltaPin,
    ) -> Result<DeltaMaintenanceSafetyEvidence, DeltaMaintenanceAuthorityError> {
        Err(DeltaMaintenanceAuthorityError::new(
            "maintenance authority is not installed during daemon startup",
        ))
    }
}

struct ExactInternalAuthorization {
    workspace_id: WorkspaceId,
    principal_id: PrincipalId,
    authorization: AuthorizationRef,
    denial: DiagnosticRef,
}

#[async_trait]
impl CommandAuthorizationPort for ExactInternalAuthorization {
    async fn authorize(
        &self,
        command: &FabricCommand,
        _current_head: ExpectedHead,
    ) -> Result<AuthorizationDecision, CommandPortError> {
        if command.ownership.workspace_id == self.workspace_id
            && command.ownership.principal_id == self.principal_id
            && command.ownership.authorization == self.authorization
        {
            Ok(AuthorizationDecision::Authorized(self.authorization))
        } else {
            Ok(AuthorizationDecision::Denied(self.denial))
        }
    }
}

struct FailClosedInterruptionDiagnostics;

#[async_trait]
impl InterruptedCommitDiagnosticRelationPort for FailClosedInterruptionDiagnostics {
    async fn read_interruption_diagnostic(
        &self,
        _query: InterruptedCommitDiagnosticQuery,
    ) -> Result<Option<DiagnosticRef>, CommandPortError> {
        Ok(None)
    }
}

struct UnavailableProofRelations {
    diagnostic: DiagnosticRef,
}

#[async_trait]
impl ActivationCandidateProofRelationsPort for UnavailableProofRelations {
    async fn observe_candidate(
        &self,
        request: super::activation_transaction::CandidateProofRequest,
    ) -> super::programmatic_activation_command_ports::ActivationCandidateProofObservation {
        super::programmatic_activation_command_ports::ActivationCandidateProofObservation::Unavailable {
            request,
            diagnostic: self.diagnostic,
        }
    }
}

async fn open_activation_authority(
    workspace_root: &Path,
    workspace_id: WorkspaceId,
    generations: Arc<SqliteWriterGenerationStore>,
    assurance_fault: Option<ProductionWorkspaceStartupAssuranceFault>,
) -> Result<Arc<SwitchableActivationAuthority>, ProductionWorkspaceStartupError> {
    let control_path = workspace_root.join("activation-control");
    private_directory(&control_path)?;
    let root = Url::from_directory_path(&control_path).map_err(|()| {
        step(
            "activation-control-root",
            "path is not an absolute file URL",
        )
    })?;
    let (pin, table) = if control_path.join("_delta_log").exists() {
        let discovered = DeltaTableBuilder::from_url(root.clone())
            .map_err(|error| step("activation-control-discovery", error))?
            .load()
            .await
            .map_err(|error| step("activation-control-discovery", error))?;
        let version = discovered.version().ok_or_else(|| {
            step(
                "activation-control-discovery",
                "snapshot has no exact version",
            )
        })?;
        let version =
            u64::try_from(version).map_err(|error| step("activation-control-version", error))?;
        let pin = ExactDeltaPin::new(&root, version)
            .map_err(|error| step("activation-control-pin", error))?;
        let table = DeltaTableBuilder::from_url(root.clone())
            .map_err(|error| step("activation-control-open", error))?
            .with_version(version)
            .load()
            .await
            .map_err(|error| step("activation-control-open", error))?;
        (pin, table)
    } else {
        provision_activation_control_history(root)
            .await
            .map_err(|error| step("activation-control-provision", error))?
    };
    let session = Arc::new(
        SessionStateBuilder::new()
            .with_default_features()
            .with_query_planner(DeltaPlanner::new())
            .build(),
    );
    let mut provider = ActivationControlDeltaProvider::try_from_loaded_table(session, pin, table)
        .await
        .map_err(|error| step("activation-control-provider", error))?;
    if assurance_fault
        == Some(
            ProductionWorkspaceStartupAssuranceFault::DurableAppendAcknowledgementLostBeforeReadback,
        )
    {
        provider = provider.with_assurance_fault(
            ControlledDeltaWriteAssuranceFault::DurableAppendAcknowledgementLostBeforeReadback,
        );
    }
    let provider = Arc::new(provider);
    Ok(Arc::new(SwitchableActivationAuthority::new(Arc::new(
        DeltaActivationRuntimeAuthority::new(workspace_id, provider, generations),
    ))))
}

fn native_source_pin(
    runs: &[ProviderNativeSyntaxRun],
    sources: &[ProviderNativeSourceImage],
) -> SourcePin {
    let mut rows = sources
        .iter()
        .map(|source| {
            (
                source.file_id,
                source.source_generation,
                source.content_digest,
            )
        })
        .collect::<Vec<_>>();
    rows.sort_unstable();
    let mut digest = blake3::Hasher::new();
    digest.update(b"codefabric.native-syntax-workspace-source.v1\0");
    digest.update(&(runs.len() as u64).to_be_bytes());
    for (file_id, generation, content) in rows {
        digest.update(&file_id);
        digest.update(&generation.to_be_bytes());
        digest.update(&content);
    }
    SourcePin(*digest.finalize().as_bytes())
}

fn observation_roots(
    root: &Path,
) -> Result<BTreeMap<ProgrammaticRelationId, Url>, ProductionWorkspaceStartupError> {
    let mut roots = BTreeMap::new();
    for relation in [
        RELATION_OBSERVATION_RELATION_ID,
        FIELD_OBSERVATION_RELATION_ID,
        SCHEMA_OBSERVATION_RELATION_ID,
        DEPENDENCY_OBSERVATION_RELATION_ID,
        PROVENANCE_OBSERVATION_RELATION_ID,
    ] {
        let path = root.join(relation.replace('.', "_"));
        private_directory(&path)?;
        roots.insert(
            ProgrammaticRelationId::new(relation),
            Url::from_directory_path(path)
                .map_err(|()| step("observation-root", "path is not an absolute file URL"))?,
        );
    }
    Ok(roots)
}

struct FreshCandidate {
    candidate: Arc<ProgrammaticFabricEpoch>,
    proof: Arc<dyn ActivationCandidateProofRelationsPort>,
    proof_receipt: ProofReceiptRef,
    pins: FabricEpochPins,
    source_images: SourceImageSetRef,
}

fn inprocess_operational_ceilings() -> Result<ProviderResourceCeilings, ProviderContractError> {
    ProviderResourceCeilings::try_new(ProviderResourceCeilingSpec {
        max_relations: 64,
        max_batches_per_relation: 64,
        max_input_bytes: 16_777_216,
        max_rows: 2_000_000,
        max_bytes: 268_435_456,
        max_diagnostics: 10_000,
        max_work_units: 10_000_000,
        max_wall_millis: 30_000,
        max_visited_nodes: 2_000_000,
        max_traversal_depth: 256,
        max_workers: 4,
        max_retained_revisions: 2,
        cancellation_poll_work_units: 1_024,
        cancellation_ack_millis: 2_000,
    })
}

async fn build_fresh_candidate(
    state_root: &Path,
    operational_database: &Path,
    record: &WorkspaceRecord,
    release: &CompiledSemanticRelease,
    fence: super::command::WriterFence,
) -> Result<FreshCandidate, ProductionWorkspaceStartupError> {
    let workspace_id = WorkspaceId::from_bytes(record.workspace_id);
    let workspace_root = state_root
        .join("fabric")
        .join(lower_hex(&record.workspace_id));
    private_directory(&workspace_root)?;
    let mut store = OperationalStore::open(operational_database)
        .map_err(|error| step("operational-store-open", error))?;
    let mut generation = current_source_generation(&store, record.workspace_id)
        .map_err(|error| step("source-generation-read", error))?;
    if generation == 0 {
        generation = advance_source_generation(&mut store, record.workspace_id, 0)
            .map_err(|error| step("source-generation-genesis", error))?;
    }
    let root = open_workspace_root(&mut store, record.workspace_id)
        .map_err(|error| step("workspace-root-open", error))?;
    let inventory = InventoryWalker::new(InventoryLimits::default())
        .walk_and_persist(&root, &mut store, generation, &Cancellation::default())
        .map_err(|error| step("source-inventory", error))?;
    let source_images = SourceImageSetRef::from_bytes(digest32(
        b"codefabric.source-image-set.v1\0",
        &[
            &record.workspace_id,
            &generation.to_be_bytes(),
            &inventory.digest,
        ],
    ));
    let analysis_context = digest32(
        b"codefabric.native-analysis-context.v1\0",
        &[&record.context_fingerprint, &inventory.digest],
    );
    let semantic_environment = digest32(
        b"codefabric.native-semantic-environment.v1\0",
        &[
            release.suite().as_str().as_bytes(),
            &record.authorization_fingerprint,
        ],
    );
    let mut image_store = SourceImageStore::open(
        &workspace_root.join("source-blobs"),
        SourceCapturePolicy::default(),
    )
    .map_err(|error| step("source-image-store", error))?;
    let mut sources = Vec::new();
    let mut module_paths = Vec::new();
    for item in inventory.records.iter().filter(|item| {
        item.language == Some("python") && item.inclusion == InclusionState::Included
    }) {
        let holder_id = digest16(
            b"codefabric.provider-source-holder.v1\0",
            &[
                &item.path.raw_relative_path_bytes,
                &generation.to_be_bytes(),
            ],
        );
        let path = PlatformPath::from_raw_relative_bytes(
            item.path.platform_code,
            item.path.raw_relative_path_bytes.clone(),
        )
        .map_err(|error| step("source-path", error))?;
        let request = CaptureRequest {
            workspace_id: record.workspace_id,
            source_generation: generation,
            change_token: generation,
            path,
            language: SourceLanguage::Python,
            holder_kind: SourceBlobHolderKind::ProviderRun,
            holder_id,
        };
        if let CaptureOutcome::Published(image) = image_store
            .capture(&mut store, &request)
            .map_err(|error| step("source-image-capture", error))?
        {
            sources.push(
                ProviderNativeSourceImage::try_from(image.as_ref())
                    .map_err(|error| step("provider-source-image", error))?,
            );
            module_paths.push(PathBuf::from(OsString::from_vec(
                item.path.raw_relative_path_bytes.clone(),
            )));
        }
    }
    drop(store);

    let epoch_id = EpochId::from_bytes(digest16(
        b"codefabric.fresh-activation.epoch.v1\0",
        &[
            &record.workspace_id,
            &generation.to_be_bytes(),
            &inventory.digest,
        ],
    ));
    let builder =
        ProgrammaticFabricEpochBuilder::try_new(epoch_id, FabricEpochRuntimeConfig::default())
            .map_err(|error| step("epoch-builder", error))?;
    let mut runner =
        ExactPythonSyntaxRunner::new().map_err(|error| step("native-provider-open", error))?;
    let mut native_runs = Vec::with_capacity(sources.len());
    for (index, (source, module_path)) in sources.iter().zip(&module_paths).enumerate() {
        let revision =
            u64::try_from(index + 1).map_err(|error| step("native-provider-revision", error))?;
        let tree_run = digest16(
            b"codefabric.tree-sitter-provider-run.v1\0",
            &[&source.file_id, &revision.to_be_bytes()],
        );
        let ruff_run = digest16(
            b"codefabric.ruff-provider-run.v1\0",
            &[&source.file_id, &revision.to_be_bytes()],
        );
        let source_binding = ProviderSourceBinding::try_new(
            SourceIdentity::try_new(format!(
                "codefabric.source.{}.{}",
                lower_hex(&source.file_id),
                source.source_generation
            ))
            .map_err(|error| step("provider-source-identity", error))?,
            source.file_id,
            source.source_generation,
            source.content_digest,
        )
        .map_err(|error| step("provider-source-binding", error))?;
        let context_binding = ProviderContextBinding::try_new(
            ContextIdentity::try_new(format!(
                "codefabric.context.{}",
                lower_hex(&analysis_context)
            ))
            .map_err(|error| step("provider-context-identity", error))?,
            analysis_context,
            semantic_environment,
        )
        .map_err(|error| step("provider-context-binding", error))?;
        let scope = ProviderScopeIdentity::try_new(format!(
            "codefabric.source-scope.{}",
            lower_hex(&source.file_id)
        ))
        .map_err(|error| step("provider-scope", error))?;
        let (_tree_cancel_owner, tree_cancellation) = CancellationProbe::pair(1_024)
            .map_err(|error| step("tree-sitter-cancellation", error))?;
        let (_ruff_cancel_owner, ruff_cancellation) =
            CancellationProbe::pair(1_024).map_err(|error| step("ruff-cancellation", error))?;
        let operational_ceilings = inprocess_operational_ceilings()
            .map_err(|error| step("in-process-provider-ceilings", error))?;
        let deadline = Instant::now() + Duration::from_secs(30);
        let tree_prepared = release
            .providers()
            .prepare_job(
                release.policy(),
                ProviderJobInput {
                    lane: ProviderLane::TreeSitter,
                    source: source_binding.clone(),
                    context: context_binding.clone(),
                    run: ProviderRunBinding::try_new(
                        ProviderRunIdentity::try_new(format!(
                            "codefabric.tree-sitter-run.{}",
                            lower_hex(&tree_run)
                        ))
                        .map_err(|error| step("tree-sitter-run-identity", error))?,
                        tree_run,
                    )
                    .map_err(|error| step("tree-sitter-run-binding", error))?,
                    scope: scope.clone(),
                    requested_families: release
                        .providers()
                        .families(ProviderLane::TreeSitter)
                        .map_err(|error| step("tree-sitter-family-program", error))?
                        .into_iter()
                        .map(|family| (family, 1))
                        .collect(),
                    operational_ceilings,
                    deadline,
                    cancellation: tree_cancellation,
                },
            )
            .map_err(|error| step("tree-sitter-job", error))?;
        let ruff_prepared = release
            .providers()
            .prepare_job(
                release.policy(),
                ProviderJobInput {
                    lane: ProviderLane::Ruff,
                    source: source_binding,
                    context: context_binding,
                    run: ProviderRunBinding::try_new(
                        ProviderRunIdentity::try_new(format!(
                            "codefabric.ruff-run.{}",
                            lower_hex(&ruff_run)
                        ))
                        .map_err(|error| step("ruff-run-identity", error))?,
                        ruff_run,
                    )
                    .map_err(|error| step("ruff-run-binding", error))?,
                    scope,
                    requested_families: release
                        .providers()
                        .families(ProviderLane::Ruff)
                        .map_err(|error| step("ruff-family-program", error))?
                        .into_iter()
                        .map(|family| (family, 1))
                        .collect(),
                    operational_ceilings,
                    deadline,
                    cancellation: ruff_cancellation,
                },
            )
            .map_err(|error| step("ruff-job", error))?;
        let jobs = InProcessProviderJobs::try_new(tree_prepared.job(), ruff_prepared.job())
            .map_err(|error| step("in-process-provider-jobs", error))?;
        let module_name = format!("codefabric_source_{}", lower_hex(&source.file_id));
        let run = runner
            .run_full(
                jobs,
                revision,
                source,
                PythonModuleInput {
                    module_name: &module_name,
                    module_path,
                },
            )
            .map_err(|error| step("native-provider-run", error))?;
        release
            .providers()
            .admit(tree_prepared, run.tree_sitter_result().clone())
            .map_err(|error| step("tree-sitter-admission", error))?;
        release
            .providers()
            .admit(ruff_prepared, run.ruff_result().clone())
            .map_err(|error| step("ruff-admission", error))?;
        native_runs.push(run);
    }
    let native_pin = native_source_pin(&native_runs, &sources);
    let requested_native = u64::try_from(native_runs.len()).unwrap_or(u64::MAX).max(1);
    let external_source_pin = SourcePin(digest32(
        b"codefabric.external-provider-source.v1\0",
        &[&record.workspace_id, &inventory.digest],
    ));
    let external_context_pin = ContextPin(digest32(
        b"codefabric.external-provider-context.v1\0",
        &[&record.context_fingerprint],
    ));
    let authority = ProductionProviderAuthority::try_new(
        ExactProviderLaneAuthority::try_new(
            native_pin,
            ContextPin(analysis_context),
            requested_native,
        )
        .map_err(|error| step("native-provider-authority", error))?,
        ExactProviderLaneAuthority::try_new(external_source_pin, external_context_pin, 1)
            .map_err(|error| step("pyrefly-provider-authority", error))?,
        ExactProviderLaneAuthority::try_new(external_source_pin, external_context_pin, 1)
            .map_err(|error| step("rustc-provider-authority", error))?,
        1,
    )
    .map_err(|error| step("provider-authority", error))?;
    let native_lane = if native_runs.is_empty() {
        ExactProviderLaneRuns::Gap(ProviderLaneGap::RequiredInputAbsent)
    } else {
        ExactProviderLaneRuns::Accepted(&native_runs)
    };
    let outcome = release
        .admit_and_compose_production_relations(
            builder,
            authority,
            ProductionProviderRuns::new(
                native_lane,
                ExactProviderLaneRuns::Gap(ProviderLaneGap::RequiredInputAbsent),
                ExactProviderLaneRuns::Gap(ProviderLaneGap::RequiredInputAbsent),
            ),
        )
        .map_err(|error| step("provider-derived-composition", error))?;
    let (derived, _) = outcome.into_parts();
    let (builder, _, _) = derived.into_parts();
    let observation_root = workspace_root
        .join("epochs")
        .join(lower_hex(epoch_id.as_bytes()))
        .join("observations");
    private_directory(&observation_root)?;
    let targets = builder
        .provision_observation_histories(observation_roots(&observation_root)?)
        .await
        .map_err(|error| step("observation-history-provision", error))?;
    let activation_operation = OperationId::from_bytes(digest16(
        b"codefabric.fresh-activation.operation.v1\0",
        &[epoch_id.as_bytes(), &inventory.digest],
    ));
    let transaction = TransactionRef::from_bytes(digest32(
        b"codefabric.fresh-activation.transaction.v1\0",
        &[
            epoch_id.as_bytes(),
            fence.lease_id.as_bytes(),
            &fence.generation.get().to_be_bytes(),
        ],
    ));
    let relation_root = workspace_root
        .join("epochs")
        .join(lower_hex(epoch_id.as_bytes()))
        .join("relations");
    private_directory(&relation_root)?;
    let candidate = Arc::new(
        builder
            .seal(
                ProgrammaticObservationWriteIdentity::new(
                    epoch_id,
                    activation_operation,
                    fence.generation,
                    transaction,
                ),
                targets,
                ProgrammaticRelationDeltaPreparation::Genesis(
                    ProgrammaticRelationDeltaLayout::try_new(
                        Url::from_directory_path(relation_root).map_err(|()| {
                            step("relation-root", "path is not an absolute file URL")
                        })?,
                    )
                    .map_err(|error| step("relation-layout", error))?,
                ),
            )
            .await
            .map_err(|error| step("epoch-seal", error))?,
    );

    let input_release = InputReleaseRef::from_bytes(digest32(
        b"codefabric.input-release.v1\0",
        &[release.suite().as_str().as_bytes()],
    ));
    let program_release = ProgramReleaseRef::from_bytes(digest32(
        b"codefabric.program-release.v1\0",
        &[release.suite().as_str().as_bytes()],
    ));
    let application_release =
        ApplicationReleaseRef::from_bytes(compiled_query_release_pin(release));
    let source_authority = SourceAuthorityRef::from_bytes(inventory.digest);
    let provider_release = ProviderReleaseRef::from_bytes(digest32(
        b"codefabric.provider-release.v1\0",
        &[release.suite().as_str().as_bytes()],
    ));
    let provider_set = ProviderSetRef::from_bytes(digest32(
        b"codefabric.provider-set.v1\0",
        &[
            native_pin.0.as_slice(),
            &analysis_context,
            &semantic_environment,
        ],
    ));
    let overlay_segments = OverlaySegmentSetRef::from_bytes(digest32(
        b"codefabric.overlay-segment-set.empty.v1\0",
        &[],
    ));
    let policy_set = PolicySetRef::from_bytes(digest32(
        b"codefabric.policy-set.local-workstation.v1\0",
        &[&record.authorization_fingerprint],
    ));
    let resources = ResourceEnvelopeRef::from_bytes(digest32(
        b"codefabric.resource-envelope.local-workstation.v1\0",
        &[&record.workspace_id],
    ));
    let proof_candidate = ProofCandidatePins {
        epoch: epoch_id,
        input_release,
        program_release,
        application_release,
        source_authority,
        source_generation: SourceGeneration::new(generation),
        source_images,
        provider_release,
        provider_set,
        table_versions: candidate.table_version_set_ref(),
        overlay_segments,
        policy_set,
        resource_envelope: resources,
    };
    let proof_relations = release
        .prove_activation_candidate(proof_candidate)
        .map_err(|error| step("activation-proof", error))?;
    let proof_receipt = proof_relations
        .receipt()
        .map_err(|error| step("activation-proof-receipt", error))?;
    let proof_root_path = workspace_root
        .join("epochs")
        .join(lower_hex(epoch_id.as_bytes()))
        .join("proof");
    private_directory(&proof_root_path)?;
    let proof_root = ProofDeltaWorkspaceRoot::try_new(
        workspace_id,
        Url::from_directory_path(proof_root_path)
            .map_err(|()| step("proof-root", "path is not an absolute file URL"))?,
    )
    .map_err(|error| step("proof-root", error))?;
    let proof_targets = provision_proof_relation_histories(proof_root)
        .await
        .map_err(|error| step("proof-history-provision", error))?;
    let proof_set = TransactionRef::from_bytes(digest32(
        b"codefabric.proof-set.v1\0",
        &[epoch_id.as_bytes(), proof_receipt.as_bytes()],
    ));
    let session = Arc::new(candidate.context().state());
    let publication = persist_proof_relations(
        Arc::clone(&session),
        proof_targets,
        ProofDeltaWriteIdentity {
            operation_id: activation_operation,
            writer_generation: fence.generation,
            proof_set_id: proof_set,
        },
        &proof_relations,
    )
    .await
    .map_err(|error| step("proof-history-persist", error))?;
    let integrity_diagnostic = DiagnosticRef::from_bytes(digest32(
        b"codefabric.proof-integrity-diagnostic.v1\0",
        &[epoch_id.as_bytes()],
    ));
    let proof: Arc<dyn ActivationCandidateProofRelationsPort> =
        Arc::new(DeltaActivationCandidateProofRelations::new(
            publication,
            session,
            Some(proof_receipt),
            None,
            integrity_diagnostic,
        ));
    Ok(FreshCandidate {
        candidate,
        proof,
        proof_receipt,
        pins: FabricEpochPins {
            epoch: epoch_id,
            input_release,
            program_release,
            application_release,
            source_authority,
            source_generation: SourceGeneration::new(generation),
            provider_release,
            provider_set,
            table_versions: proof_candidate.table_versions,
            overlay_segments,
            policy_set,
            resource_envelope: resources,
            proof_receipt,
        },
        source_images,
    })
}

/// Compose one workspace, recover its command journal, and establish exact semantic authority.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn start_production_workspace(
    state_root: &Path,
    operational_database: &Path,
    record: &WorkspaceRecord,
    release: Arc<CompiledSemanticRelease>,
    slot: Arc<WorkspaceSlot>,
    generations: Arc<SqliteWriterGenerationStore>,
    writer_lease: WorkspaceWriterLease,
    assurance_fault: Option<ProductionWorkspaceStartupAssuranceFault>,
) -> Result<ProductionWorkspaceStartup, ProductionWorkspaceStartupError> {
    let workspace_id = WorkspaceId::from_bytes(record.workspace_id);
    if slot.workspace_id() != workspace_id || writer_lease.workspace_id() != workspace_id {
        return Err(step(
            "workspace-binding",
            "slot or writer lease was substituted",
        ));
    }
    let workspace_root = state_root
        .join("fabric")
        .join(lower_hex(&record.workspace_id));
    private_directory(&workspace_root)?;
    let activation = open_activation_authority(
        &workspace_root,
        workspace_id,
        Arc::clone(&generations),
        assurance_fault,
    )
    .await?;
    let selection = activation
        .current_selection()
        .await
        .map_err(|error| step("activation-selection", error))?;
    let admission = match &selection {
        ExactActivationControlSelection::GenesisRequired(_) => Arc::new(
            super::admission::FabricAdmissionRuntime::fresh_genesis(workspace_id),
        ),
        ExactActivationControlSelection::Selected(selected) => Arc::new(
            super::admission::FabricAdmissionRuntime::recover_unmaterialized_for_reconciliation(
                selected.chain(),
            )
            .map_err(|error| step("selected-admission-recovery", error))?,
        ),
    };
    let published_results = Arc::new(PublishedArrowResultRegistry::new());
    let checkpoint_store = Arc::new(
        SqliteDeltaCdfCheckpointStore::open(&workspace_root.join("cdf-checkpoints.sqlite3"))
            .map_err(|error| step("cdf-checkpoint-store", error))?,
    );
    let delta_ports =
        ProgrammaticDeltaRuntimePorts::new(checkpoint_store, Arc::new(DenyMaintenance));
    let active_config = ProductionActiveWorkspaceConfig::bounded_local_workstation()
        .map_err(|error| step("active-workspace-resource-policy", error))?;
    let active_builder: Arc<dyn ReleaseOwnedActiveWorkspaceBuilder> =
        Arc::new(ProductionActiveWorkspaceBuilder::new(
            Arc::clone(&release),
            active_config,
            Arc::clone(&admission),
            Arc::clone(&published_results),
            delta_ports,
            Arc::clone(&generations) as Arc<dyn super::writer_lease::DurableWriterGenerationPort>,
            Arc::clone(&activation),
        ));

    let fresh = match &selection {
        ExactActivationControlSelection::GenesisRequired(genesis) => {
            if genesis.workspace_id() != workspace_id
                || genesis.writer_fence() != writer_lease.fence()
            {
                return Err(step(
                    "genesis-authority",
                    "empty head has another writer fence",
                ));
            }
            Some(
                build_fresh_candidate(
                    state_root,
                    operational_database,
                    record,
                    release.as_ref(),
                    writer_lease.fence(),
                )
                .await?,
            )
        }
        ExactActivationControlSelection::Selected(selected) => {
            let selected_record = SelectedEpochRecord::from_exact_readback(selected);
            let active = active_builder
                .rebuild_selected(selected_record, selected.chain())
                .await
                .map_err(|error| step("selected-workspace-rebuild", error))?;
            admission
                .install_reconciled_selected_head(
                    selected.event(),
                    selected.chain(),
                    Arc::clone(active.runtime().epoch()),
                    selected.control_horizon().active_recovery_fence(),
                )
                .map_err(|error| step("selected-admission-install", error))?;
            slot.install_initial(active)
                .map_err(|error| step("selected-workspace-install", error))?;
            None
        }
    };

    let candidate_rebuilder: Arc<dyn ActivationCommandCandidateRebuilderPort> = Arc::new(
        ExactDeltaActivationCommandCandidateRebuilder::new(workspace_id, |epoch_id| {
            ProgrammaticFabricEpochBuilder::try_new(epoch_id, FabricEpochRuntimeConfig::default())
        }),
    );
    let control_binding = activation.current().control_relation().binding().clone();
    let identity_policy = ActivationReconciliationIdentityPolicy::try_new(
        UnknownCommitReason::ReadbackUnavailable,
        digest32(
            b"codefabric.activation-reconciliation-diagnostic-namespace.v1\0",
            &[workspace_id.as_bytes()],
        ),
        digest32(
            b"codefabric.activation-reconciliation-evidence-namespace.v1\0",
            &[workspace_id.as_bytes()],
        ),
    )
    .map_err(|error| step("activation-identity-policy", error))?;
    let state_store = Arc::new(
        SqliteProgrammaticActivationCommandStateStore::open(
            &workspace_root.join("activation-commands.sqlite3"),
            workspace_id,
            candidate_rebuilder,
            control_binding,
            identity_policy,
        )
        .map_err(|error| step("activation-command-state", error))?,
    );
    let proof: Arc<dyn ActivationCandidateProofRelationsPort> = fresh.as_ref().map_or_else(
        || {
            Arc::new(UnavailableProofRelations {
                diagnostic: DiagnosticRef::from_bytes(digest32(
                    b"codefabric.proof-publication-required.v1\0",
                    &[workspace_id.as_bytes()],
                )),
            }) as Arc<dyn ActivationCandidateProofRelationsPort>
        },
        |fresh| Arc::clone(&fresh.proof),
    );
    let gaps = |family: &'static [u8]| {
        ProgrammaticCommandCapabilityGapInput::new(
            ProgrammaticCommandCapabilityDisposition::Unavailable,
            DiagnosticRef::from_bytes(digest32(
                b"codefabric.command-capability-gap.v1\0",
                &[workspace_id.as_bytes(), family],
            )),
        )
    };
    let non_activation = ProgrammaticNonActivationCommandEffects::try_new(
        gaps(b"source-wave"),
        gaps(b"relation-publication"),
        gaps(b"rollback"),
        gaps(b"compaction"),
        gaps(b"retention"),
        gaps(b"administration"),
    )
    .map_err(|error| step("command-effect-gaps", error))?;
    let effects = ExactProgrammaticCommandEffectClosure::new(
        ProgrammaticActivationCommandEffects::new(
            Arc::clone(&state_store) as Arc<_>,
            proof,
            DiagnosticRef::from_bytes(digest32(
                b"codefabric.activation-proof-integrity.v1\0",
                &[workspace_id.as_bytes()],
            )),
            Arc::clone(&active_builder),
        ),
        non_activation,
    )
    .build_for_daemon(
        workspace_id,
        Arc::clone(&admission),
        Arc::downgrade(&slot),
        Arc::clone(&activation),
    );
    let principal_id = PrincipalId::from_bytes(digest16(
        b"codefabric.internal-command-principal.v1\0",
        &[workspace_id.as_bytes()],
    ));
    let authorization = AuthorizationRef::from_bytes(digest32(
        b"codefabric.internal-command-authorization.v1\0",
        &[workspace_id.as_bytes(), &record.authorization_fingerprint],
    ));
    let command_authorization: Arc<dyn CommandAuthorizationPort> =
        Arc::new(ExactInternalAuthorization {
            workspace_id,
            principal_id,
            authorization,
            denial: DiagnosticRef::from_bytes(digest32(
                b"codefabric.internal-command-denial.v1\0",
                &[workspace_id.as_bytes()],
            )),
        });
    let semantics = Arc::new(RelationalCommandSemanticContext::new(
        workspace_id,
        Arc::clone(&activation) as Arc<_>,
        command_authorization,
    ));
    let actor_id = ActorId::from_bytes(digest16(
        b"codefabric.fabric-command-actor.v1\0",
        &[
            workspace_id.as_bytes(),
            &writer_lease.fence().generation.get().to_be_bytes(),
        ],
    ));
    let runtime_config = FabricCommandRuntimeConfig::new(
        state_root.join("writer-authority"),
        generations.database_path(),
        workspace_root.join("fabric-commands.sqlite3"),
        workspace_id,
        writer_lease.fence().lease_id,
        actor_id,
        FabricCommandActorConfig::default(),
    );
    let command_runtime = FabricCommandRuntime::start_with_held_authority(
        runtime_config,
        Arc::clone(&generations),
        writer_lease,
        semantics,
        effects,
    )
    .map_err(|error| step("command-runtime-start", error))?;
    let post_startup = async {
        let diagnostics = RelationalInterruptedCommitDiagnostics::new(
            workspace_id,
            Arc::new(FailClosedInterruptionDiagnostics),
        );
        let recovery = command_runtime
            .recover_and_open_bounded(
                CommandRecoveryPageSize::new(128).expect("128 is a valid recovery page"),
                NonZeroUsize::new(8).expect("eight recovery sweeps is nonzero"),
                &diagnostics as &dyn InterruptedCommitDiagnosticPort,
            )
            .await
            .map_err(|error| step("command-runtime-recovery", error))?;
        if !matches!(
            recovery.state(),
            super::command_runtime::FabricCommandStartupRecoveryState::Ready
        ) {
            return Err(step(
                "command-runtime-recovery",
                "bounded recovery retained a nonterminal obligation",
            ));
        }

        let selected = if let Some(fresh) = fresh {
            let operation_id = OperationId::from_bytes(digest16(
                b"codefabric.fresh-activation.operation.v1\0",
                &[
                    fresh.candidate.identity().as_bytes(),
                    fresh.source_images.as_bytes(),
                ],
            ));
            let command = FabricCommand {
                identity: CommandIdentity {
                    operation_id,
                    idempotency_key: IdempotencyKey::from_bytes(digest32(
                        b"codefabric.fresh-activation.idempotency.v1\0",
                        &[operation_id.as_bytes()],
                    )),
                },
                ownership: CommandOwnership {
                    workspace_id,
                    principal_id,
                    authorization,
                },
                expected_head: ExpectedHead::Empty,
                writer_fence: command_runtime.fence(),
                pins: CommandPins {
                    input_release: fresh.pins.input_release,
                    program_release: fresh.pins.program_release,
                    application_release: fresh.pins.application_release,
                    source_authority: fresh.pins.source_authority,
                    source_generation: fresh.pins.source_generation,
                    provider_release: fresh.pins.provider_release,
                    provider_set: fresh.pins.provider_set,
                },
                resources: fresh.pins.resource_envelope,
                payload: FabricCommandPayload::ActivateEpoch {
                    candidate_epoch: fresh.pins.epoch,
                    proof_receipt: fresh.proof_receipt,
                },
            };
            let material = ActivationCommandRequestMaterial::new(
                ActivationCommandRequestKey::new(command),
                Arc::clone(&fresh.candidate),
                fresh.pins,
                ActivationEventId::from_bytes(digest32(
                    b"codefabric.fresh-activation.event.v1\0",
                    &[
                        operation_id.as_bytes(),
                        fresh.candidate.table_version_set_ref().as_bytes(),
                    ],
                )),
                CompatibilityClassRef::from_bytes(digest32(
                    b"codefabric.compatibility-class.v1\0",
                    &[release.suite().as_str().as_bytes()],
                )),
                RetentionPolicyRef::from_bytes(digest32(
                    b"codefabric.retention-policy.v1\0",
                    &[workspace_id.as_bytes()],
                )),
                OperationSelectionRef::from_bytes(digest32(
                    b"codefabric.activation-operation-selection.v1\0",
                    &[operation_id.as_bytes()],
                )),
                TransactionRef::from_bytes(digest32(
                    b"codefabric.activation-transaction.v1\0",
                    &[
                        operation_id.as_bytes(),
                        command_runtime.fence().lease_id.as_bytes(),
                    ],
                )),
                ActivationControlRelationPin::new(
                    activation.current().control_relation().table().clone(),
                    activation.current().control_relation().binding().clone(),
                ),
            );
            state_store
                .persist_request(&material)
                .await
                .map_err(|error| step("activation-request-persist", error))?;
            let completed = command_runtime
                .handle()
                .submit(command)
                .await
                .map_err(|error| step("fresh-activation-command", error))?;
            let completed = if matches!(
                completed.state(),
                DurableCommandState::AwaitingReconciliation { .. }
            ) {
                let reconciliation = command_runtime
                    .recover_and_open_bounded(
                        CommandRecoveryPageSize::new(128).expect("128 is a valid recovery page"),
                        NonZeroUsize::new(8).expect("eight recovery sweeps is nonzero"),
                        &diagnostics as &dyn InterruptedCommitDiagnosticPort,
                    )
                    .await
                    .map_err(|error| step("fresh-activation-reconciliation", error))?;
                match reconciliation.state() {
                    super::command_runtime::FabricCommandStartupRecoveryState::Ready => {
                        command_runtime
                            .handle()
                            .submit(command)
                            .await
                            .map_err(|error| step("fresh-activation-terminal-readback", error))?
                    }
                    super::command_runtime::FabricCommandStartupRecoveryState::Pending {
                        operation_id,
                        obligation,
                    } => {
                        return Err(step(
                            "fresh-activation-reconciliation",
                            format!(
                                "exact durable evidence remained ambiguous for operation \
                                 {operation_id:?}: {obligation:?}"
                            ),
                        ));
                    }
                }
            } else {
                completed
            };
            match completed.state() {
                DurableCommandState::Succeeded {
                    result: CommandResult::EpochActivated { epoch, .. },
                    ..
                } if epoch == fresh.pins.epoch => (epoch, true),
                state => {
                    return Err(step(
                        "fresh-activation-command",
                        format!("unexpected terminal state {state:?}"),
                    ));
                }
            }
        } else {
            let active = slot
                .lease()
                .map_err(|error| step("selected-workspace-observe", error))?;
            (active.workspace().selection().epoch_id(), false)
        };
        slot.lease().map_err(|error: ActiveWorkspaceError| {
            step("active-workspace-final-readback", error)
        })?;
        Ok::<_, ProductionWorkspaceStartupError>(selected)
    }
    .await;
    let (selected_epoch, fresh_activation) = match post_startup {
        Ok(selected) => selected,
        Err(primary) => return Err(shutdown_after_startup_error(command_runtime, primary).await),
    };
    Ok(ProductionWorkspaceStartup {
        command_runtime,
        selected_epoch,
        fresh_activation,
    })
}
