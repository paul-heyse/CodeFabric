//! `FreshActivation` and exact selected-epoch recovery for one supervisor-owned workspace.
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
    ProviderReleaseRef, ProviderSetRef, RetentionPolicyRef, SourceAuthorityRef, SourceGeneration,
    SourceImageSetRef, TransactionRef, UnknownCommitReason, WorkspaceId,
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
use super::production_kernel::{
    ActiveWorkspaceError, CompiledSemanticRelease, SelectedEpochRecord, WorkspaceSlot,
};
use super::programmatic_activation_admission::ReleaseOwnedActiveWorkspaceBuilder;
use super::programmatic_activation_command_ports::{
    ActivationCommandRequestKey, ActivationCommandRequestMaterial,
};
use super::activation_transaction::PublishedCandidateValidation;
use super::programmatic_activation_command_sqlite::{
    ActivationCommandCandidateRebuilderPort, ActivationReconciliationIdentityPolicy,
    ExactDeltaActivationCommandCandidateRebuilder, SqliteProgrammaticActivationCommandStateStore,
};
use super::programmatic_active_workspace_builder::ProductionActiveWorkspaceBuilder;
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
use super::published_arrow_result::PublishedArrowResultRegistry;
use super::workspace_resources::ProductionWorkspaceResources;
use super::writer_generation_sqlite::SqliteWriterGenerationStore;
use super::writer_lease::WorkspaceWriterLease;
use crate::cancellation::StructuredCancellationScope;
use crate::operational_store::OperationalStore;
use crate::production_provider_recipe::{
    ExactProviderLaneAuthority, ProductionProviderAuthority, ProductionProviderRuns,
};
use crate::provider_admission::{ExactProviderLaneRuns, ProviderLaneGap};
use crate::provider_contracts::{
    CancellationProbe, ProviderContractError, ProviderLane, ProviderResourceCeilingSpec,
    ProviderResourceCeilings, ProviderRunBinding, ProviderRunIdentity, ProviderScopeIdentity,
    ProviderSourceBinding, SourceIdentity,
};
use crate::provider_native_syntax::{
    ExactPythonSyntaxRunner, InProcessProviderJobs, ProviderNativeSourceImage,
    ProviderNativeSyntaxRun, PythonModuleInput,
};
use crate::relation_ipc::{ContextPin, SourcePin};
use crate::semantic_release::ProviderJobInput;
use crate::source_image::{SourceLanguage, advance_source_generation, current_source_generation};
use crate::workspace_registry::WorkspaceRecord;

mod input_observations;
mod inputs;

/// Joined owner retained by the daemon after one workspace reaches queryable authority.
pub(crate) struct ProductionWorkspaceStartup {
    command_runtime: FabricCommandRuntime,
    selected_epoch: EpochId,
    fresh_activation: bool,
    resources: ProductionWorkspaceResources,
    task_scope: StructuredCancellationScope,
}

/// Bounded assurance interruption admitted only by the daemon's debug-build configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionWorkspaceStartupAssuranceFault {
    DurableAppendAcknowledgementLostBeforeReadback,
}

impl ProductionWorkspaceStartup {
    pub(crate) fn resources(&self) -> &ProductionWorkspaceResources {
        &self.resources
    }

    #[must_use]
    pub(crate) const fn selected_epoch(&self) -> EpochId {
        self.selected_epoch
    }

    #[must_use]
    pub(crate) const fn fresh_activation(&self) -> bool {
        self.fresh_activation
    }

    pub(crate) async fn shutdown(self) -> Result<(), ProductionWorkspaceStartupError> {
        // A failed native join must not release the writer lease in-process. In that case
        // FabricCommandRuntime's fail-closed Drop keeps the OS fence until process teardown.
        self.task_scope
            .cancel_and_join(Duration::from_secs(2))
            .await
            .map_err(|error| step("workspace-operation-join", error))?;
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

async fn open_activation_authority(
    workspace_root: &Path,
    workspace_id: WorkspaceId,
    generations: Arc<SqliteWriterGenerationStore>,
    assurance_fault: Option<ProductionWorkspaceStartupAssuranceFault>,
    resources: &ProductionWorkspaceResources,
    task_scope: &StructuredCancellationScope,
) -> Result<Arc<DeltaActivationRuntimeAuthority>, ProductionWorkspaceStartupError> {
    let control_path = workspace_root.join("activation-control");
    let root = Url::from_directory_path(&control_path).map_err(|()| {
        step(
            "activation-control-root",
            "path is not an absolute file URL",
        )
    })?;
    let session = Arc::new(
        SessionStateBuilder::new()
            .with_default_features()
            .with_runtime_env(
                resources
                    .native()
                    .control_runtime_with_registry(Arc::clone(
                        &resources.native().runtime_env().object_store_registry,
                    ))
                    .map_err(|error| step("activation-control-resources", error))?,
            )
            .with_query_planner(DeltaPlanner::new())
            .build(),
    );
    let (pin, table) = if control_path.join("_delta_log").exists() {
        let discovered = super::delta_exact::session_delta_table_builder(root.clone(), &session)
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
        let table = super::delta_exact::session_delta_table_builder(root.clone(), &session)
            .map_err(|error| step("activation-control-open", error))?
            .with_version(version)
            .load()
            .await
            .map_err(|error| step("activation-control-open", error))?;
        (pin, table)
    } else {
        let executor = resources
            .native_execution(task_scope)
            .map_err(|error| step("activation-control-executor", error))?;
        let provision_root = root.clone();
        let provision_session = Arc::clone(&session);
        let version = executor
            .run_mutation(
                "activation-control-provision",
                crate::resource_budget::ResourceClass::Control,
                Instant::now() + Duration::from_secs(120),
                move |_, _| async move {
                    let (pin, _table) =
                        provision_activation_control_history(provision_root, &provision_session)
                            .await?;
                    Ok::<_, super::activation_control_delta::ActivationControlError>(pin.version())
                },
            )
            .await
            .map_err(|error| step("activation-control-provision", error))?;
        // The native runtime and its writes have joined. Only the exact version crosses
        // that boundary; reconstruct the long-lived reader on the serving runtime.
        let pin = ExactDeltaPin::new(&root, version)
            .map_err(|error| step("activation-control-pin", error))?;
        let table = super::delta_exact::session_delta_table_builder(root, &session)
            .map_err(|error| step("activation-control-open", error))?
            .with_version(version)
            .load()
            .await
            .map_err(|error| step("activation-control-open", error))?;
        (pin, table)
    };
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
    Ok(Arc::new(DeltaActivationRuntimeAuthority::new(
        workspace_id,
        provider,
        generations,
    ).with_execution(
        resources.native_execution(task_scope)
            .map_err(|error| step("activation-executor", error))?,
    )))
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
    validation: Arc<PublishedCandidateValidation>,
    proof_receipt: ProofReceiptRef,
    pins: FabricEpochPins,
    source_images: SourceImageSetRef,
}

// Only owned identifiers and exact version vectors leave the publishing runtime.
// Delta tables, sessions, executors and streams are reconstructed after it joins.
struct FreshCandidatePublication {
    table_versions: Arc<super::activation::TableVersionSet>,
    pins: FabricEpochPins,
    source_images: SourceImageSetRef,
}

impl super::native_execution_lane::output_seal::Sealed for FreshCandidatePublication {}
impl super::native_execution_lane::NativeLaneOutput for FreshCandidatePublication {
    fn validate_retained_owner(
        &self,
        _: &crate::resource_budget::ResourceBudget,
    ) -> Result<(), crate::resource_budget::ResourceBudgetError> {
        Ok(())
    }
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

struct FreshNativeSource {
    builder: ProgrammaticFabricEpochBuilder,
    workspace_root: PathBuf,
    generation: u64,
    epoch_id: EpochId,
    source_images: SourceImageSetRef,
    inventory_digest: [u8; 32],
    analysis_context: [u8; 32],
    semantic_environment: [u8; 32],
    native_pin: SourcePin,
}

/// One owned blocking operation carries source capture, parser owners, and exact admission.
/// A dropped async observer cannot detach the native work or release its running reservation.
fn build_fresh_native_source(
    state_root: &Path,
    operational_database: &Path,
    record: &WorkspaceRecord,
    release: &CompiledSemanticRelease,
    workspace_resources: &ProductionWorkspaceResources,
    cancellation: crate::cancellation::Cancellation,
) -> Result<FreshNativeSource, ProductionWorkspaceStartupError> {
    crate::process_memory::admit(crate::resource_budget::ResourceClass::Data)
        .map_err(|error| step("source-memory-headroom", error))?;
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
    drop(store);
    let prepared_inputs = inputs::capture_inputs(
        &workspace_root,
        operational_database,
        record,
        generation,
        workspace_resources.budget().clone(),
        cancellation.clone(),
        workspace_resources.source_blob_disk().clone(),
    )?;
    let inventory_digest = prepared_inputs.inventory.identity();
    let context_product = inputs::discover_python_inputs(&prepared_inputs, record)?;
    let prepared_context = inputs::provider_context(&context_product)?;
    let source_images = SourceImageSetRef::from_bytes(digest32(
        b"codefabric.source-image-set.v1\0",
        &[
            &record.workspace_id,
            &generation.to_be_bytes(),
            &inventory_digest,
        ],
    ));
    let analysis_context = prepared_context.context_fingerprint();
    let semantic_environment = prepared_context.semantic_environment_id();
    let source_count = prepared_inputs.capture()?.images().len();
    let working_bytes =
        prepared_inputs
            .capture()?
            .images()
            .iter()
            .try_fold(8192_u64, |sum, image| {
                sum.checked_add(8192)
                    .and_then(|bytes| {
                        bytes.checked_add(image.path.raw_relative_path_bytes.len() as u64 * 2)
                    })
                    .ok_or_else(|| step("provider-container-memory", "size overflow"))
            })?;
    let _working = crate::inventory::reserve_memory(workspace_resources.budget(), working_bytes)
        .map_err(|error| step("provider-container-memory", error))?;
    let mut sources = Vec::with_capacity(source_count);
    let mut module_paths = Vec::with_capacity(source_count);
    for image in prepared_inputs
        .capture()?
        .images()
        .iter()
        .filter(|image| image.language == SourceLanguage::Python)
    {
        sources.push(
            ProviderNativeSourceImage::try_from(image)
                .map_err(|error| step("provider-source-image", error))?,
        );
        module_paths.push(PathBuf::from(OsString::from_vec(
            image.path.raw_relative_path_bytes.clone(),
        )));
    }

    let epoch_id = EpochId::from_bytes(digest16(
        b"codefabric.fresh-activation.epoch.v1\0",
        &[
            &record.workspace_id,
            &generation.to_be_bytes(),
            &inventory_digest,
        ],
    ));
    let builder = ProgrammaticFabricEpochBuilder::try_new_governed(
        epoch_id,
        workspace_resources.config().epoch_runtime().clone(),
        workspace_resources.native().clone(),
    )
    .map_err(|error| step("epoch-builder", error))?;
    let mut runner = None;
    let mut native_runs = Vec::with_capacity(sources.len());
    let mut admitted_runs = Vec::with_capacity(sources.len().saturating_mul(2));
    for (index, (source, module_path)) in sources.iter().zip(&module_paths).enumerate() {
        if cancellation.is_cancelled() {
            return Err(step(
                "native-provider-cancelled",
                "cancelled before provider run",
            ));
        }
        let revision =
            u64::try_from(index + 1).map_err(|error| step("native-provider-revision", error))?;
        let tree_run = digest16(
            b"codefabric.tree-sitter-provider-run.v1\0",
            &[
                &source.file_id,
                &revision.to_be_bytes(),
                &inventory_digest,
                &prepared_context.effective_input_identity(),
            ],
        );
        let ruff_run = digest16(
            b"codefabric.ruff-provider-run.v1\0",
            &[
                &source.file_id,
                &revision.to_be_bytes(),
                &inventory_digest,
                &prepared_context.effective_input_identity(),
            ],
        );
        let source_binding = ProviderSourceBinding::try_file(
            SourceIdentity::try_new(format!(
                "codefabric.source.{}.{}",
                lower_hex(&source.file_id),
                source.source_generation
            ))
            .map_err(|error| step("provider-source-identity", error))?,
            record.workspace_id,
            source.file_id,
            source.source_generation,
            source.content_digest,
        )
        .map_err(|error| step("provider-source-binding", error))?;
        let context_binding = prepared_context.clone();
        let scope = ProviderScopeIdentity::try_new(format!(
            "codefabric.source-scope.{}",
            lower_hex(&source.file_id)
        ))
        .map_err(|error| step("provider-scope", error))?;
        let tree_cancellation = CancellationProbe::from_cancellation(cancellation.clone(), 1_024)
            .map_err(|error| step("tree-sitter-cancellation", error))?;
        let ruff_cancellation = CancellationProbe::from_cancellation(cancellation.clone(), 1_024)
            .map_err(|error| step("ruff-cancellation", error))?;
        let tree_budget = workspace_resources
            .budget()
            .operation(tree_run, workspace_resources.budget().policy())
            .map_err(|error| step("tree-sitter-resource-owner", error))?;
        let ruff_budget = workspace_resources
            .budget()
            .operation(ruff_run, workspace_resources.budget().policy())
            .map_err(|error| step("ruff-resource-owner", error))?;
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
                    resource_budget: tree_budget,
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
                    resource_budget: ruff_budget,
                },
            )
            .map_err(|error| step("ruff-job", error))?;
        let jobs = InProcessProviderJobs::try_new(tree_prepared.job(), ruff_prepared.job())
            .map_err(|error| step("in-process-provider-jobs", error))?;
        if runner.is_none() {
            runner = Some(
                ExactPythonSyntaxRunner::new(jobs)
                    .map_err(|error| step("native-provider-open", error))?,
            );
        }
        let module_name = &prepared_context
            .module_for_file(source.file_id)
            .ok_or_else(|| {
                step(
                    "effective-module-map",
                    "selected source has no qualified module mapping",
                )
            })?
            .qualified_name;
        let run = runner
            .as_mut()
            .expect("native runner initialized")
            .run_full(
                jobs,
                revision,
                source,
                PythonModuleInput {
                    module_name,
                    module_path,
                },
            )
            .map_err(|error| step("native-provider-run", error))?;
        admitted_runs.push(
            release
                .providers()
                .admit(tree_prepared, run.tree_sitter_result().clone())
                .map_err(|error| step("tree-sitter-admission", error))?,
        );
        admitted_runs.push(
            release
                .providers()
                .admit(ruff_prepared, run.ruff_result().clone())
                .map_err(|error| step("ruff-admission", error))?,
        );
        native_runs.push(run);
    }
    let native_pin = native_source_pin(&native_runs, &sources);
    let requested_native = u64::try_from(native_runs.len()).unwrap_or(u64::MAX).max(1);
    let external_source_pin = SourcePin(digest32(
        b"codefabric.external-provider-source.v1\0",
        &[&record.workspace_id, &inventory_digest],
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
    let (mut builder, _, _) = derived.into_parts();
    input_observations::install_input_observations(
        &mut builder,
        &prepared_inputs.inventory,
        &admitted_runs,
    )?;
    // Registered batches own their buffers; source leases are no longer needed after providers join.
    prepared_inputs.release()?;
    Ok(FreshNativeSource {
        builder,
        workspace_root,
        generation,
        epoch_id,
        source_images,
        inventory_digest,
        analysis_context,
        semantic_environment,
        native_pin,
    })
}

fn source_operation_metadata_bytes(
    state_root: &Path,
    operational_database: &Path,
    record: &WorkspaceRecord,
) -> Result<u64, ProductionWorkspaceStartupError> {
    let parts = [
        state_root.as_os_str().as_encoded_bytes(),
        operational_database.as_os_str().as_encoded_bytes(),
        record.administrative_key.as_slice(),
        record.root_path_bytes.as_slice(),
        record.root_path_display.as_bytes(),
        record.root_directory_file_identity.as_slice(),
        record.case_sensitivity_mode.as_bytes(),
        record.created_at.as_bytes(),
        record.updated_at.as_bytes(),
    ];
    parts
        .into_iter()
        .chain(
            record
                .allowed_source_disclosure_rules
                .iter()
                .map(|rule| rule.as_bytes()),
        )
        .try_fold(128 * 1024_u64, |sum, bytes| {
            sum.checked_add(bytes.len() as u64 * 4 + 128)
                .ok_or_else(|| step("source-operation-memory", "size overflow"))
        })
}

async fn build_fresh_candidate(
    state_root: &Path,
    operational_database: &Path,
    record: &WorkspaceRecord,
    release: &Arc<CompiledSemanticRelease>,
    fence: super::command::WriterFence,
    workspace_resources: &ProductionWorkspaceResources,
    task_scope: &StructuredCancellationScope,
) -> Result<FreshCandidate, ProductionWorkspaceStartupError> {
    let guard = workspace_resources
        .budget()
        .try_reserve(
            crate::resource_budget::ResourceClass::Data,
            crate::resource_budget::ResourceAmounts {
                memory_bytes: source_operation_metadata_bytes(
                    state_root,
                    operational_database,
                    record,
                )?,
                running_jobs: 1,
                ..crate::resource_budget::ResourceAmounts::default()
            },
        )
        .map_err(|error| step("source-operation-admission", error))?;
    let source_scope = task_scope
        .child("source-providers")
        .map_err(|error| step("source-operation-scope", error))?;
    let operation_state_root = state_root.to_owned();
    let operation_database = operational_database.to_owned();
    let operation_record = record.clone();
    let operation_release = Arc::clone(release);
    let operation_resources = workspace_resources.clone();
    let operation = source_scope
        .spawn_blocking_owned("capture-and-providers", guard, move |cancellation| {
            build_fresh_native_source(
                &operation_state_root,
                &operation_database,
                &operation_record,
                &operation_release,
                &operation_resources,
                cancellation,
            )
        })
        .await
        .map_err(|error| step("source-operation-start", error))?;
    let source = operation
        .wait()
        .await
        .map_err(|error| step("source-operation-join", error))??;
    let publish_release = Arc::clone(release);
    let publish_record = record.clone();
    let publish_resources = workspace_resources.clone();
    let publication = workspace_resources
        .native_execution(task_scope)
        .map_err(|error| step("candidate-publish-executor", error))?
        .run_mutation(
            "candidate-publish",
            crate::resource_budget::ResourceClass::Data,
            Instant::now() + Duration::from_secs(120),
            move |_, _| async move {
                publish_fresh_candidate(
                    source,
                    &publish_record,
                    &publish_release,
                    fence,
                    &publish_resources,
                )
                .await
            },
        )
        .await
        .map_err(|error| step("candidate-publish", error))?;
    let candidate = Arc::new(
        ProgrammaticFabricEpochBuilder::try_new_governed(
            publication.pins.epoch,
            workspace_resources.config().epoch_runtime().clone(),
            workspace_resources.native().clone(),
        )
        .map_err(|error| step("candidate-reader", error))?
        .reopen(publication.table_versions)
        .await
        .map_err(|error| step("candidate-readback", error))?,
    );
    let proof_receipt = publication.pins.proof_receipt;
    let integrity_diagnostic = DiagnosticRef::from_bytes(digest32(
        b"codefabric.candidate-validation-diagnostic.v1\0",
        &[publication.pins.epoch.as_bytes()],
    ));
    let validation = Arc::new(PublishedCandidateValidation::for_published(
        WorkspaceId::from_bytes(record.workspace_id),
        publication.pins,
        &candidate,
        integrity_diagnostic,
    ).map_err(|error| step("candidate-validation", error))?);
    Ok(FreshCandidate {
        candidate,
        validation,
        proof_receipt,
        pins: publication.pins,
        source_images: publication.source_images,
    })
}

async fn publish_fresh_candidate(
    source: FreshNativeSource,
    record: &WorkspaceRecord,
    release: &Arc<CompiledSemanticRelease>,
    fence: super::command::WriterFence,
    workspace_resources: &ProductionWorkspaceResources,
) -> Result<FreshCandidatePublication, ProductionWorkspaceStartupError> {
    let workspace_id = WorkspaceId::from_bytes(record.workspace_id);
    let FreshNativeSource {
        builder,
        workspace_root,
        generation,
        epoch_id,
        source_images,
        inventory_digest,
        analysis_context,
        semantic_environment,
        native_pin,
    } = source;
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
        &[epoch_id.as_bytes(), &inventory_digest],
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
    let source_authority = SourceAuthorityRef::from_bytes(inventory_digest);
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
    let resources = workspace_resources.policy_ref();
    // Compact identity for this published candidate. The wire field retains its
    // historical name, but no proof language or independent histories are executed.
    let table_versions = candidate.table_version_set_ref();
    let proof_receipt = ProofReceiptRef::from_bytes(digest32(
        b"codefabric.published-candidate-record.v1\0",
        &[
            workspace_id.as_bytes(), epoch_id.as_bytes(),
            input_release.as_bytes(), program_release.as_bytes(),
            application_release.as_bytes(), source_authority.as_bytes(),
            &generation.to_be_bytes(), source_images.as_bytes(),
            provider_release.as_bytes(), provider_set.as_bytes(),
            table_versions.as_bytes(), overlay_segments.as_bytes(),
            policy_set.as_bytes(), resources.as_bytes(),
        ],
    ));
    Ok(FreshCandidatePublication {
        table_versions: Arc::clone(candidate.table_version_set()),
        pins: FabricEpochPins {
            epoch: epoch_id,
            input_release,
            program_release,
            application_release,
            source_authority,
            source_generation: SourceGeneration::new(generation),
            provider_release,
            provider_set,
            table_versions,
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
    workspace_resources: ProductionWorkspaceResources,
    task_scope: StructuredCancellationScope,
) -> Result<ProductionWorkspaceStartup, ProductionWorkspaceStartupError> {
    let guard = workspace_resources
        .budget()
        .try_reserve(
            crate::resource_budget::ResourceClass::Control,
            crate::resource_budget::ResourceAmounts {
                memory_bytes: source_operation_metadata_bytes(
                    state_root,
                    operational_database,
                    record,
                )?,
                running_jobs: 1,
                ..crate::resource_budget::ResourceAmounts::default()
            },
        )
        .map_err(|error| step("startup-operation-admission", error))?;
    let state_root = state_root.to_owned();
    let operational_database = operational_database.to_owned();
    let record = record.clone();
    let startup_scope = task_scope
        .child_control("workspace-startup")
        .map_err(|error| step("startup-operation-scope", error))?;
    // The registry-owned future retains the lease if its caller drops the observation while
    // native source or Delta work is active. Intent to cancel is not writer-lock release.
    let operation = startup_scope
        .spawn_async_owned(
            "compose",
            crate::cancellation::TaskCancellationMode::Cooperative,
            guard,
            async move {
                compose_production_workspace(
                    &state_root,
                    &operational_database,
                    &record,
                    release,
                    slot,
                    generations,
                    writer_lease,
                    assurance_fault,
                    workspace_resources,
                    task_scope,
                )
                .await
            },
        )
        .await
        .map_err(|error| step("startup-operation-start", error))?;
    operation
        .wait()
        .await
        .map_err(|error| step("startup-operation-join", error))?
}

#[allow(clippy::too_many_arguments)]
async fn compose_production_workspace(
    state_root: &Path,
    operational_database: &Path,
    record: &WorkspaceRecord,
    release: Arc<CompiledSemanticRelease>,
    slot: Arc<WorkspaceSlot>,
    generations: Arc<SqliteWriterGenerationStore>,
    writer_lease: WorkspaceWriterLease,
    assurance_fault: Option<ProductionWorkspaceStartupAssuranceFault>,
    workspace_resources: ProductionWorkspaceResources,
    task_scope: StructuredCancellationScope,
) -> Result<ProductionWorkspaceStartup, ProductionWorkspaceStartupError> {
    let workspace_id = WorkspaceId::from_bytes(record.workspace_id);
    if slot.workspace_id() != workspace_id || writer_lease.workspace_id() != workspace_id {
        return Err(step(
            "workspace-binding",
            "slot or writer lease was substituted",
        ));
    }
    // Physical directory ownership is established before any Delta engine or
    // mutation lease exists. This join retains the original store through a
    // dropped observer and preserves partial-failure charges for retry.
    workspace_resources
        .bootstrap_local_store_owned(&task_scope)
        .await
        .map_err(|error| step("workspace-physical-bootstrap", error))?;
    let workspace_root = state_root
        .join("fabric")
        .join(lower_hex(&record.workspace_id));
    let activation = open_activation_authority(
        &workspace_root,
        workspace_id,
        Arc::clone(&generations),
        assurance_fault,
        &workspace_resources,
        &task_scope,
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
    let published_results = Arc::new(PublishedArrowResultRegistry::new(
        workspace_resources.budget().clone(),
    ));
    let checkpoint_store = Arc::new(
        SqliteDeltaCdfCheckpointStore::open(&workspace_root.join("cdf-checkpoints.sqlite3"))
            .map_err(|error| step("cdf-checkpoint-store", error))?,
    );
    let delta_ports =
        ProgrammaticDeltaRuntimePorts::new(checkpoint_store, Arc::new(DenyMaintenance));
    let active_config = workspace_resources.config().clone();
    let active_builder: Arc<dyn ReleaseOwnedActiveWorkspaceBuilder> =
        Arc::new(ProductionActiveWorkspaceBuilder::new(
            Arc::clone(&release),
            active_config,
            Arc::clone(&admission),
            Arc::clone(&published_results),
            delta_ports,
            Arc::clone(&activation),
            workspace_resources.clone(),
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
                    &release,
                    writer_lease.fence(),
                    &workspace_resources,
                    &task_scope,
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

    let candidate_resources = workspace_resources.clone();
    let candidate_rebuilder: Arc<dyn ActivationCommandCandidateRebuilderPort> = Arc::new(
        ExactDeltaActivationCommandCandidateRebuilder::new(workspace_id, move |epoch_id| {
            ProgrammaticFabricEpochBuilder::try_new_governed(
                epoch_id,
                candidate_resources.config().epoch_runtime().clone(),
                candidate_resources.native().clone(),
            )
        }),
    );
    let control_binding = activation
        .current_control()
        .map_err(|error| step("activation-control-state", error))?
        .control_relation()
        .binding()
        .clone();
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
    let validation = fresh.as_ref().map_or_else(
        || Arc::new(PublishedCandidateValidation::unavailable(
            DiagnosticRef::from_bytes(digest32(
                b"codefabric.published-candidate-required.v1\0",
                &[workspace_id.as_bytes()],
            )),
        )),
        |fresh| Arc::clone(&fresh.validation),
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
            validation,
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
            let activation_control = activation
                .current_control()
                .map_err(|error| step("activation-control-state", error))?;
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
                    activation_control.control_relation().table().clone(),
                    activation_control.control_relation().binding().clone(),
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
                    .map_err(|error| {
                        step("fresh-activation-reconciliation", format!(
                            "{error}; preceding command state: {:?}", completed.state(),
                        ))
                    })?;
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
        resources: workspace_resources,
        task_scope,
    })
}
