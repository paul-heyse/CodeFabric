//! Captured Python inputs to a contained provider and the production catalog boundary.

use std::os::unix::ffi::OsStrExt as _;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::cancellation::Cancellation;
use crate::identity::{IdentityDomain, encode_public_id};
use crate::provider_admission::{
    ExactProviderLaneRuns, ProviderLaneGap, pyrefly_source_pin_from_modules,
};
use crate::provider_contracts::{
    AdmittedProviderResult, CancellationProbe, ProviderContextBinding, ProviderLane,
    ProviderResourceCeilingSpec, ProviderResourceCeilings, ProviderRunBinding, ProviderRunIdentity,
    ProviderScopeIdentity, ProviderSourceBinding, ProviderTerminalStatus, SourceIdentity,
};
use crate::pyrefly_service::{AcceptedPyreflyRun, PyreflyModuleInput, PyreflyWorkspaceInput};
use crate::relation_ipc::SourcePin;
use crate::resource_budget::ChargedValue;
use crate::semantic_release::ProviderJobInput;
use crate::source_image::{DependencyInputBundle, SourceLanguage, publish_provider_workspace_view};

use super::inputs::PreparedSourceInputs;
use super::{CompiledSemanticRelease, ProductionWorkspaceStartupError, digest16, lower_hex, step};

pub(super) struct PyreflyOutcome {
    pub source_pin: SourcePin,
    pub requested_units: u64,
    pub admitted: Option<AdmittedProviderResult>,
    runs: Vec<AcceptedPyreflyRun>,
    gap: ProviderLaneGap,
}

impl PyreflyOutcome {
    pub fn lane(&self) -> ExactProviderLaneRuns<'_, AcceptedPyreflyRun> {
        if self.runs.is_empty() {
            ExactProviderLaneRuns::Gap(self.gap)
        } else {
            ExactProviderLaneRuns::Accepted(&self.runs)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(super) enum StartupPyreflyError {
    #[error(transparent)]
    Provider(#[from] crate::pyrefly_service::PyreflyServiceError),
    #[error("Pyrefly process cleanup did not join: {0}")]
    Join(String),
}

/// Called inside the source operation's owned blocking task. The existing daemon runtime
/// drives UDS traffic; the blocking task retains input leases through complete extraction.
/// The workspace scope retains the contained checker across compatible generations.
pub(super) fn run(
    workspace_root: &Path,
    release: &CompiledSemanticRelease,
    inputs: &PreparedSourceInputs,
    context: &ChargedValue<ProviderContextBinding>,
    context_manifest: &[u8],
    cancellation: Cancellation,
    work: super::PublicationWork<'_>,
) -> Result<PyreflyOutcome, ProductionWorkspaceStartupError> {
    let super::PublicationWork {
        resources, stage, ..
    } = work;
    let images = inputs
        .capture()?
        .images()
        .iter()
        .filter(|image| image.language == SourceLanguage::Python)
        .collect::<Vec<_>>();
    let ids = images
        .iter()
        .map(|image| {
            encode_public_id(IdentityDomain::SourceFile, None, image.file_id)
                .map_err(|error| step("pyrefly-file-id", error))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let generation = inputs.inventory.source_generation();
    let source_pin = pyrefly_source_pin_from_modules(
        generation,
        images
            .iter()
            .zip(&ids)
            .map(|(image, id)| (id.as_str(), image.file_id, image.digest)),
    );
    let mut outcome = PyreflyOutcome {
        source_pin,
        requested_units: images.len().max(1) as u64,
        admitted: None,
        runs: Vec::new(),
        gap: ProviderLaneGap::RequiredInputAbsent,
    };
    if images.is_empty() {
        tokio::runtime::Handle::current()
            .block_on(
                resources
                    .pyrefly_cache()
                    .lock()
                    .map_err(|error| step("pyrefly-cache-owner", error))?
                    .retire(),
            )
            .map_err(|error| step("pyrefly-empty-retirement", error))?;
        return Ok(outcome);
    }
    if stage == super::PublicationStage::Source {
        outcome.gap = ProviderLaneGap::Pending;
        return Ok(outcome);
    }
    // A complete checker inventory is required. Never call a truncated batch complete.
    if images.len() > crate::pyrefly_service::inventory_stream::MAX_MODULES_PER_RUN
        || images
            .iter()
            .try_fold(0_u64, |total, image| total.checked_add(image.byte_length))
            .is_none_or(|total| {
                total > crate::pyrefly_service::inventory_stream::MAX_SOURCE_BYTES_PER_RUN
            })
        || images.iter().any(|image| {
            image.byte_length
                > crate::pyrefly_service::inventory_stream::MAX_SOURCE_BYTES_PER_MODULE
        })
    {
        outcome.gap = ProviderLaneGap::ResourceLimit;
        return Ok(outcome);
    }
    let executable = crate::fabric::provider_deployment::ProviderExecutable::Pyrefly
        .selected_path()
        .ok();
    let Some(executable) = executable.and_then(|path| std::fs::canonicalize(path).ok()) else {
        outcome.gap = ProviderLaneGap::ProviderFailure;
        tracing::warn!("Pyrefly executable unavailable; Python semantic scope remains incomplete");
        return Ok(outcome);
    };
    let run_id = digest16(
        b"codefabric.startup.pyrefly-run.v1\0",
        &[&source_pin.0, &context.effective_input_identity()],
    );
    // A stable read-only mount contains only daemon-published Python input views. The
    // checker copies each verified generation into its own writable native workspace.
    let provider_root = workspace_root.join("pyrefly-workspace");
    super::private_directory(&provider_root)?;
    let view = publish_provider_workspace_view(
        &provider_root,
        "pyrefly-workspace-service.v1",
        inputs.inventory.workspace_id(),
        generation,
        &images,
        &DependencyInputBundle::empty(),
    )
    .map_err(|error| step("pyrefly-source-view", error))?;
    let input_root = provider_root.join("provider-views");
    let mounted_view = view
        .workspace_root
        .strip_prefix(&input_root)
        .map_err(|error| step("pyrefly-mounted-input", error))?;
    let modules = images
        .iter()
        .zip(&ids)
        .map(|(image, id)| {
            let binding = context
                .module_for_file(image.file_id)
                .ok_or_else(|| step("pyrefly-module", "captured file lacks a context module"))?;
            let relative = Path::new(std::ffi::OsStr::from_bytes(&binding.relative_path));
            Ok(PyreflyModuleInput {
                module_id: id.clone(),
                file_id: id.clone(),
                module_name: binding.qualified_name.clone(),
                source_blob_path: view.workspace_root.join(relative),
                provider_source_blob_path: Path::new("/workspace")
                    .join(mounted_view)
                    .join(relative),
                content_digest: format!("b3:{}", lower_hex(&image.digest)),
            })
        })
        .collect::<Result<Vec<_>, ProductionWorkspaceStartupError>>()?;
    let input = PyreflyWorkspaceInput {
        workspace_id: encode_public_id(
            IdentityDomain::Workspace,
            None,
            inputs.inventory.workspace_id(),
        )
        .map_err(|error| step("pyrefly-workspace-id", error))?,
        canonical_workspace_id: inputs.inventory.workspace_id(),
        context_manifest: context_manifest.to_vec(),
        source_snapshot_lease_id: format!(
            "capture:{generation}:{}",
            lower_hex(&inputs.inventory.identity())
        ),
        changed_module_ids: ids,
        modules,
    };
    let prepared = release
        .providers()
        .prepare_job(
            release.policy(),
            ProviderJobInput {
                lane: ProviderLane::Pyrefly,
                source: ProviderSourceBinding::from_inventory(
                    SourceIdentity::try_new(format!(
                        "codefabric.python-input.{}",
                        lower_hex(&source_pin.0)
                    ))
                    .map_err(|error| step("pyrefly-source-identity", error))?,
                    inputs.inventory.clone(),
                ),
                context: context.clone(),
                run: ProviderRunBinding::try_new(
                    ProviderRunIdentity::try_new(format!(
                        "codefabric.pyrefly-run.{}",
                        lower_hex(&run_id)
                    ))
                    .map_err(|error| step("pyrefly-run-identity", error))?,
                    run_id,
                )
                .map_err(|error| step("pyrefly-run-binding", error))?,
                scope: ProviderScopeIdentity::try_new(format!(
                    "codefabric.python-scope.{}",
                    lower_hex(&inputs.inventory.workspace_id())
                ))
                .map_err(|error| step("pyrefly-scope", error))?,
                requested_families: release
                    .providers()
                    .families(ProviderLane::Pyrefly)
                    .map_err(|error| step("pyrefly-families", error))?
                    .into_iter()
                    .map(|family| (family, outcome.requested_units))
                    .collect(),
                operational_ceilings: ProviderResourceCeilings::try_new(
                    ProviderResourceCeilingSpec {
                        max_relations: 64,
                        max_batches_per_relation: 65_536,
                        max_input_bytes:
                            crate::pyrefly_service::inventory_stream::MAX_SOURCE_BYTES_PER_RUN,
                        max_rows: 4_000_000,
                        max_bytes: 512 * 1024 * 1024,
                        max_diagnostics: 20_000,
                        max_work_units: 20_000_000,
                        max_wall_millis: 120_000,
                        max_visited_nodes: 4_000_000,
                        max_traversal_depth: 512,
                        max_workers: 16,
                        max_retained_revisions: 1,
                        cancellation_poll_work_units: 1_024,
                        cancellation_ack_millis: 2_000,
                    },
                )
                .map_err(|error| step("pyrefly-ceilings", error))?,
                deadline: Instant::now() + Duration::from_secs(120),
                cancellation: CancellationProbe::from_cancellation(cancellation, 1_024)
                    .map_err(|error| step("pyrefly-cancellation", error))?,
                resource_budget: inputs
                    .budget()
                    .operation(run_id, inputs.budget().policy())
                    .map_err(|error| step("pyrefly-owner", error))?,
            },
        )
        .map_err(|error| step("pyrefly-job", error))?;
    let result = tokio::runtime::Handle::current().block_on(
        resources
            .pyrefly_cache()
            .lock()
            .map_err(|error| step("pyrefly-cache-owner", error))?
            .analyze(
                prepared.job(),
                input,
                executable,
                &input_root,
                view.output_root,
            ),
    );
    match result {
        Ok(result) => {
            outcome.gap = match result.result().terminal() {
                ProviderTerminalStatus::Cancelled => ProviderLaneGap::Cancelled,
                ProviderTerminalStatus::TimedOut => ProviderLaneGap::TimedOut,
                ProviderTerminalStatus::Oversized => ProviderLaneGap::ResourceLimit,
                ProviderTerminalStatus::Unknown => ProviderLaneGap::Unsupported,
                _ => ProviderLaneGap::ProviderFailure,
            };
            if let Some(run) = result.accepted() {
                outcome.runs.push(run.clone());
            }
            outcome.admitted = Some(
                release
                    .providers()
                    .admit(prepared, result.result().clone())
                    .map_err(|error| step("pyrefly-result-admission", error))?,
            );
        }
        Err(StartupPyreflyError::Join(error)) => return Err(step("pyrefly-process-join", error)),
        Err(StartupPyreflyError::Provider(error)) => {
            use crate::pyrefly_service::PyreflyRunGap;
            outcome.gap = match error.run_gap() {
                Some(PyreflyRunGap::ResourceLimit) => ProviderLaneGap::ResourceLimit,
                Some(PyreflyRunGap::Cancelled) => ProviderLaneGap::Cancelled,
                Some(PyreflyRunGap::TimedOut) => ProviderLaneGap::TimedOut,
                Some(PyreflyRunGap::TrustUnavailable) => ProviderLaneGap::TrustUnavailable,
                Some(PyreflyRunGap::PreparationUnavailable) => ProviderLaneGap::Unsupported,
                _ => ProviderLaneGap::ProviderFailure,
            };
            tracing::warn!(%error, "Python semantic provider did not complete");
        }
    }
    Ok(outcome)
}
