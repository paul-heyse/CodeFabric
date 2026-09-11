//! Captured Rust workspace preparation, contained Cargo, and accepted Arrow publication.

use std::io::Read as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::analysis_context::rust_context::{
    RustContextDiscoveryOutcome, RustContextDiscoveryRequest, RustContextSelection,
    discover_rust_context,
};
use crate::analysis_context::{
    ContextArtifactInput, ContextFileInput, ContextSearchRoot, ContextSearchScope,
    ContextSearchUniverse, RustToolchainSettings,
};
use crate::cancellation::{Cancellation, StructuredCancellationScope};
use crate::identity::{
    IdentityDomain, decode_public_id, encode_public_id, random_registration_nonce,
};
use crate::integrity::{digest_bytes, frame_digest};
use crate::provider_admission::{ExactProviderLaneRuns, ProviderLaneGap};
use crate::provider_contracts::{
    AdmittedProviderResult, CancellationProbe, ContextIdentity, ProviderContextBinding,
    ProviderLane, ProviderResourceCeilingSpec, ProviderResourceCeilings, ProviderRunBinding,
    ProviderRunIdentity, ProviderScopeIdentity, ProviderSourceBinding, SourceIdentity,
};
use crate::provider_sandbox::{
    CompiledProviderSeccomp, GeneratedSandboxProfile, ProviderSandboxLaunchMaterial,
    ProviderSandboxLauncher, ProviderTrustProfile, SandboxCapabilityMatrix, SandboxMechanism,
};
use crate::relation_ipc::{ContextPin, SourcePin};
use crate::rust_compilation_trust::{
    RustCompilationCancellationSignal, RustCompilationContextPins, RustCompilationInputs,
    RustCompilationPrivatePaths, RustCompilationResourceLimits, RustCompilationRunRequest,
    RustCompilationTrustPolicy, RustExecutableExtensionPolicy, SelectedRustCompilationPreparation,
    compile_rust_metadata_launch_plan, supervise_rust_compilation,
};
use crate::rustc_relation_schema::{RUSTC_PUBLIC_RELEASE, RUSTC_TOOLCHAIN, schema_bundle_digest};
use crate::rustc_service::{
    RustcProtocolPolicy, RustcRunAdmission, TrustQualifiedRustcCompilation,
    UntrustedRustcProviderLifecycle, run_untrusted_rustc_provider_lifecycle,
};
use crate::rustc_source_files::{CapturedRustSourceFile, RustSourceFileManifest};
use crate::semantic_release::ProviderJobInput;
use crate::source_image::{
    DependencyInput, DependencyInputBundle, SourceLanguage, publish_provider_workspace_view,
};
use crate::workspace_registry::WorkspaceRecord;

use super::inputs::ProviderInputs;
use super::{CompiledSemanticRelease, ProductionWorkspaceStartupError, digest16, lower_hex, step};

mod targets;
pub(in crate::fabric) mod toolchain_cache;
pub(in crate::fabric) mod unit_graph;

use toolchain_cache::{FileWitness, ToolchainSelectionKey};

const TOOLCHAIN_IDENTITY: &[u8] =
    include_bytes!("../../../rustc-extractor/toolchain-identity.json");
const MAX_TOOLCHAIN_BYTES: u64 = 8 * 1024 * 1024 * 1024;

fn toolchain_release() -> String {
    format!(
        "rustc-{RUSTC_PUBLIC_RELEASE}-{}",
        RUSTC_TOOLCHAIN.trim_start_matches("nightly-")
    )
}

pub(super) struct RustcOutcome {
    pub source_pin: SourcePin,
    pub context_pin: ContextPin,
    pub admitted: Vec<AdmittedProviderResult>,
    pub progress: Vec<RustTargetProgress>,
    unit_graphs: Vec<unit_graph::SelectedUnitGraph>,
    unselected_graphs: Vec<crate::resource_budget::ChargedValue<unit_graph::CapturedUnitGraph>>,
    runs: Vec<TrustQualifiedRustcCompilation>,
    gap: ProviderLaneGap,
}

pub(super) struct RustTargetProgress {
    pub manifest: Vec<u8>,
    pub target: String,
    pub target_kind: String,
    pub target_platform: Option<String>,
    pub rust_build: Option<crate::fabric::processing_status::ProcessingRustBuildSelection>,
    pub context_id: Option<[u8; 16]>,
    pub state: &'static str,
    pub detail: String,
}

impl RustTargetProgress {
    fn new(target: &targets::CargoTarget, state: &'static str, detail: &str) -> Self {
        let (state, detail) = target
            .build
            .error
            .as_deref()
            .map_or((state, detail), |error| ("unavailable", error));
        Self {
            manifest: target.manifest.clone(),
            target: target.target.name.clone(),
            target_kind: target.target.kind.as_str().to_owned(),
            target_platform: target.target_triple.clone(),
            rust_build: target.build.processing(),
            context_id: None,
            state,
            detail: detail.to_owned(),
        }
    }
}

impl RustcOutcome {
    fn pending_targets(mut self, targets: Vec<targets::CargoTarget>) -> Self {
        self.gap = ProviderLaneGap::Pending;
        self.progress = targets
            .into_iter()
            .map(|target| RustTargetProgress::new(&target, "pending", "semantic_work_pending"))
            .collect();
        self
    }

    fn empty(source_pin: SourcePin, context_pin: ContextPin) -> Self {
        Self {
            source_pin,
            context_pin,
            admitted: Vec::new(),
            progress: Vec::new(),
            unit_graphs: Vec::new(),
            unselected_graphs: Vec::new(),
            runs: Vec::new(),
            gap: ProviderLaneGap::RequiredInputAbsent,
        }
    }

    pub fn observed_relations(
        &self,
    ) -> impl Iterator<Item = crate::rustc_relation_schema::RustcRelation> + '_ {
        self.runs
            .iter()
            .flat_map(|run| &run.accepted().owners)
            .flat_map(|owner| &owner.relations)
            .map(|relation| relation.relation)
    }

    pub fn compilation_units(&self) -> u64 {
        self.runs.len().max(1) as u64
    }

    pub fn owner_units(&self) -> u64 {
        self.runs
            .iter()
            .flat_map(|run| &run.accepted().owners)
            .filter(|owner| {
                owner.relations.iter().any(|relation| {
                    relation.relation == crate::rustc_relation_schema::RustcRelation::PublicItem
                })
            })
            .count()
            .max(1) as u64
    }

    pub fn lane(&self) -> ExactProviderLaneRuns<'_, TrustQualifiedRustcCompilation> {
        if self.runs.is_empty() {
            ExactProviderLaneRuns::Gap(self.gap)
        } else {
            ExactProviderLaneRuns::Accepted(&self.runs)
        }
    }

    pub(super) fn install_unit_graphs(
        &self,
        builder: &mut crate::fabric::programmatic_epoch::ProgrammaticFabricEpochBuilder,
        workspace: [u8; 16],
        generation: u64,
    ) -> Result<(), ProductionWorkspaceStartupError> {
        unit_graph::install(
            builder,
            &self.unit_graphs,
            &self.unselected_graphs,
            workspace,
            generation,
        )
    }
}

pub(super) fn run(
    root: &Path,
    release: &CompiledSemanticRelease,
    inputs: &ProviderInputs<'_>,
    record: &WorkspaceRecord,
    cancellation: &Cancellation,
    work: super::PublicationWork<'_>,
) -> Result<RustcOutcome, ProductionWorkspaceStartupError> {
    let super::PublicationWork {
        scope,
        stage,
        resources,
        ..
    } = work;
    let inventory = inputs.inventory_for_language(SourceLanguage::Rust)?;
    let mut outcome = RustcOutcome::empty(
        SourcePin(inventory.identity()),
        ContextPin(record.context_fingerprint),
    );
    if inventory.selected_files().next().is_none() {
        resources
            .rust_unit_graph_cache()
            .lock()
            .map_err(|error| step("cargo-unit-graph-cache-owner", error))?
            .clear();
        resources
            .rust_toolchain_cache()
            .lock()
            .map_err(|error| step("rust-toolchain-cache-owner", error))?
            .clear();
        return Ok(outcome);
    }
    if !cfg!(target_os = "linux") {
        outcome.gap = ProviderLaneGap::TrustUnavailable;
        return Ok(outcome);
    }
    let targets = match targets::discover(&captured_files(inputs)?) {
        Ok(targets) => targets,
        Err(error) => {
            tracing::warn!(%error, "Rust target discovery did not complete");
            outcome.gap = ProviderLaneGap::ProviderFailure;
            return Ok(outcome);
        }
    };
    let targets = match selected_toolchain(cancellation) {
        Ok((_, host)) => targets::resolve_host(targets, &host),
        Err(_) => targets, // Preserve requested scope; each semantic preparation reports its failure.
    };
    if stage == super::PublicationStage::Source {
        outcome.unselected_graphs = resources
            .rust_unit_graph_cache()
            .lock()
            .map_err(|error| step("cargo-unit-graph-cache-owner", error))?
            .unselected();
        return Ok(outcome.pending_targets(targets));
    }
    let mut contexts = Vec::new();
    let toolchain = OnceLock::new();
    let completed = super::context_workers::map(
        &targets,
        resources
            .scheduler()
            .native_cpu_observation()
            .capacity
            .min(2),
        inputs.budget(),
        |target| {
            let mut graphs = Vec::new();
            let available = if cancellation.is_cancelled() {
                Err(step(
                    "rust-context-cancelled",
                    "obsolete context was not started",
                ))
            } else {
                prepare_and_run(
                    root,
                    release,
                    inputs,
                    record,
                    inventory.clone(),
                    target,
                    &toolchain,
                    &mut graphs,
                    resources,
                    scope,
                    cancellation.clone(),
                )
            };
            (available, graphs)
        },
    )?;
    for (target, (available, graphs)) in targets.iter().zip(completed) {
        outcome.unit_graphs.extend(graphs);
        let mut progress = RustTargetProgress::new(target, "unavailable", "");
        match available {
            Ok((context_pin, admitted, runs)) => {
                progress.context_id = Some(admitted.job().context().analysis_context_id());
                progress.state = if runs.is_empty()
                    || runs.iter().any(|run| {
                        run.trust_proof().terminal().terminal_state
                            == crate::rust_compilation_trust::RustCompilationTerminalState::CompilerFailed
                    })
                {
                    "unavailable"
                } else {
                    "processed"
                };
                progress.detail = admitted.result().gaps().first().map_or_else(
                    || format!("{:?}", admitted.result().terminal()),
                    |gap| {
                        format!(
                            "{}: {:?}: {}",
                            gap.family().as_str(),
                            gap.cause(),
                            gap.detail()
                        )
                    },
                );
                if !runs.is_empty() {
                    contexts.push((admitted.job().context().analysis_context_id(), context_pin));
                }
                outcome.admitted.push(admitted);
                outcome.runs.extend(runs);
            }
            Err(error) if error.step == "rust-process-join" => return Err(error),
            Err(error) => {
                progress.detail = error.to_string();
                tracing::warn!(%error, target = %target.target.name, "Rust semantic scope did not complete");
            }
        }
        outcome.progress.push(progress);
    }
    outcome.gap = ProviderLaneGap::ProviderFailure;
    if !contexts.is_empty() {
        outcome.context_pin = crate::provider_admission::rustc_context_set_pin(contexts)
            .map_err(|error| step("rust-context-set", error))?;
    }
    resources
        .rust_unit_graph_cache()
        .lock()
        .map_err(|error| step("cargo-unit-graph-cache-owner", error))?
        .replace(&outcome.unit_graphs);
    Ok(outcome)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)] // One owned capture-to-provider transaction.
fn prepare_and_run(
    root: &Path,
    release: &CompiledSemanticRelease,
    inputs: &ProviderInputs<'_>,
    record: &WorkspaceRecord,
    inventory: crate::resource_budget::ChargedValue<
        crate::provider_contracts::ProviderSourceInventory,
    >,
    target: &targets::CargoTarget,
    shared_toolchain: &SharedToolchain,
    unit_graphs: &mut Vec<unit_graph::SelectedUnitGraph>,
    resources: &crate::fabric::workspace_resources::ProductionWorkspaceResources,
    scope: &StructuredCancellationScope,
    cancellation: Cancellation,
) -> Result<
    (
        ContextPin,
        AdmittedProviderResult,
        Vec<TrustQualifiedRustcCompilation>,
    ),
    ProductionWorkspaceStartupError,
> {
    if let Some(error) = &target.build.error {
        return Err(step("rust-context-configuration", error));
    }
    let capabilities = SandboxCapabilityMatrix::probe_current_host();
    if !capabilities
        .row(ProviderTrustProfile::UntrustedSandboxed)
        .is_some_and(|row| row.available)
    {
        return Err(step(
            "rust-containment",
            format!(
                "contained compiler unavailable: {:?}",
                capabilities
                    .row(ProviderTrustProfile::UntrustedSandboxed)
                    .map(|row| &row.unmet_requirements),
            ),
        ));
    }
    // One immutable capture lease serves every target in this publication pass. The workspace
    // retains compatible captured inputs; Cargo and extractor outputs are still fresh per run.
    // A failed capture is also shared, so each requested target keeps its own failure scope.
    let toolchain = shared_toolchain
        .get_or_init(|| {
            resources
                .rust_toolchain_cache()
                .lock()
                .map_err(|error| step("rust-toolchain-cache-owner", error))?
                .acquire(&cancellation)
        })
        .as_ref()
        .map_err(|error| step("rust-toolchain", error))?;
    let cpu = resources
        .native_cpu_allocation(&cancellation)
        .map_err(|error| step("rust-cpu-admission", error))?;
    let workers = cpu
        .workers()
        .get()
        .try_into()
        .expect("bounded native worker profile");
    let mut dependencies = toolchain.dependencies.entries.clone();
    let workspace_id = public_id(IdentityDomain::Workspace, record.workspace_id)?;
    let files = captured_files(inputs)?;
    let source_manifest = RustSourceFileManifest {
        workspace_id: workspace_id.clone(),
        source_generation: inventory.source_generation(),
        files: files
            .iter()
            .map(|file| {
                (
                    file.relative_path.clone(),
                    CapturedRustSourceFile {
                        file_id: file.file_id.clone(),
                        content_digest: file.digest,
                    },
                )
            })
            .collect(),
    };
    dependencies.push(dependency(
        "source-files.json",
        serde_json::to_vec(&source_manifest)
            .map_err(|error| step("rust-source-manifest", error))?,
        false,
    ));
    let dependencies = DependencyInputBundle::pin(dependencies)
        .map_err(|error| step("rust-dependency-bundle", error))?;
    let selection = initial_selection(&files, target, toolchain, workers)?;
    let product = discover_rust_context(&RustContextDiscoveryRequest {
        workspace_id: workspace_id.clone(),
        source_generation: inventory.source_generation(),
        provider_bundle_version: "codefabric-rust-compiler-v1".into(),
        files,
        search_scope: ContextSearchScope {
            namespace: b".".to_vec(),
            ordered_roots: vec![ContextSearchRoot {
                root_id: "workspace-root".into(),
                relative_path: b".".to_vec(),
            }],
            policy_identity: record.authorization_fingerprint,
            universe: if inputs.capture().dispositions().iter().any(|entry| {
                !matches!(
                    entry.disposition(),
                    crate::source_image::InventoryCaptureDisposition::Captured { .. }
                )
            }) {
                ContextSearchUniverse::Incomplete {
                    observed_identity: inputs.inventory.identity(),
                }
            } else {
                ContextSearchUniverse::Closed {
                    inventory_identity: inputs.inventory.identity(),
                }
            },
        },
        selection,
    })
    .map_err(|error| step("rust-context", error))?;
    let RustContextDiscoveryOutcome::Prepared(product) = product else {
        return Err(step("rust-context", format!("{product:?}")));
    };
    let context_pin = product
        .context
        .fingerprint_bytes()
        .map_err(|error| step("rust-context-pin", error))?;
    let view_id = digest16(
        b"codefabric.rust-provider-run.v1\0",
        &[&inventory.identity(), &context_pin],
    );
    let images = inputs.capture().images().iter().collect::<Vec<_>>();
    let view = publish_provider_workspace_view(
        root,
        &lower_hex(&view_id),
        record.workspace_id,
        inventory.source_generation(),
        &images,
        &dependencies,
    )
    .map_err(|error| step("rust-source-view", error))?;
    let compilation_inputs = RustCompilationInputs::inspect(
        &view.workspace_root,
        &view.dependency_root,
        &view.dependency_root.join("toolchain/bin/cargo"),
        &view.dependency_root.join("toolchain/bin/rustc"),
        &view
            .dependency_root
            .join("toolchain/bin/codefabric-rustc-extractor"),
        &frame_digest(inventory.identity()),
        &frame_digest(dependencies.manifest_digest),
        &frame_digest(digest_bytes(TOOLCHAIN_IDENTITY)),
        &toolchain_release(),
    )
    .map_err(|error| step("rust-compilation-inputs", error))?;
    drop(dependencies);
    let limits = RustCompilationResourceLimits {
        cpu_workers: workers,
        wall_time_millis: 120_000,
        stdout_bytes: 64 * 1024 * 1024,
        stderr_bytes: 64 * 1024 * 1024,
        artifact_bytes: 4 * 1024 * 1024 * 1024,
        process_count: 256,
        file_count: 100_000,
        single_file_bytes: 1024 * 1024 * 1024,
        cpu_seconds: 1800,
        memory_bytes: 16 * 1024 * 1024 * 1024,
        open_files: 4096,
    };
    let policy = RustCompilationTrustPolicy::untrusted_sandboxed_v1(
        limits,
        RustExecutableExtensionPolicy::ExecuteInsideSelectedLauncher,
    );
    let resource_budget = inputs.provider_operation_budget()?;
    let run_id = resource_budget.owner().id;
    let mut request = RustCompilationRunRequest {
        preparation: SelectedRustCompilationPreparation::from_discovered(&product)
            .map_err(|error| step("rust-selected-preparation", error))?,
        build_scripts_present: product
            .settings
            .build_inputs
            .as_ref()
            .is_none_or(|inputs| !inputs.is_empty()),
        procedural_macros_present: false,
        context: RustCompilationContextPins {
            provider_run_id: format!("codefabric.rustc-run.{}", lower_hex(&run_id)),
            workspace_id: workspace_id.clone(),
            analysis_context_id: product.context.analysis_context_id.clone(),
            source_generation: inventory.source_generation(),
            context_manifest_digest: product.context.context_fingerprint.clone(),
            resource_profile_id: "compiler-semantic-standard".into(),
            source_snapshot_manifest_digest: frame_digest(inventory.identity()),
            cargo_metadata_digest: frame_digest(digest_bytes(b"")),
            cargo_lock_digest: product.settings.lock_artifacts.first().map_or_else(
                || frame_digest(digest_bytes(b"")),
                |input| frame_digest(input.digest),
            ),
            cargo_config_digest: frame_digest(context_pin),
        },
    };
    let seccomp =
        CompiledProviderSeccomp::compile().map_err(|error| step("rust-seccomp", error))?;
    // Input/context identity is stable across retries; writable compiler output is not.
    // A fresh attempt must never reuse a previous subprocess's private output tree.
    let attempt =
        lower_hex(&random_registration_nonce().map_err(|error| step("rust-attempt-id", error))?);
    let metadata_paths =
        RustCompilationPrivatePaths::prepare(&view.output_root, &format!("metadata-{attempt}"))
            .map_err(|error| step("rust-metadata-output", error))?;
    let metadata_profile = profile(&compilation_inputs, &metadata_paths)?;
    let metadata_plan = compile_rust_metadata_launch_plan(
        &policy,
        &capabilities,
        &metadata_profile,
        &compilation_inputs,
        &metadata_paths,
        &request,
    )
    .map_err(|error| step("rust-metadata-plan", error))?;
    let terminal = supervise_rust_compilation(
        &metadata_plan,
        &metadata_paths,
        &ProviderSandboxLauncher::new(capabilities.clone()),
        &metadata_profile,
        ProviderSandboxLaunchMaterial::LinuxSeccomp(&seccomp),
        &RustCompilationCancellationSignal::default().with_scope_cancellation(cancellation.clone()),
    )
    .map_err(|error| step("rust-metadata", error))?;
    let metadata = metadata_plan
        .read_metadata(&metadata_paths, &terminal)
        .map_err(|error| {
            let observed = terminal.terminal();
            step(
                "rust-metadata-result",
                format!(
                    "{error}; state={:?}, exit={:?}, limit={:?}, elapsed_ms={}, process_group_empty={}",
                    observed.terminal_state,
                    observed.exit_code,
                    observed.exceeded_limit,
                    observed.usage.wall_time_millis,
                    observed.process_group_empty,
                ),
            )
        })?;
    request.context.cargo_metadata_digest = frame_digest(digest_bytes(&metadata));
    request.preparation = request
        .preparation
        .with_cargo_metadata(&compilation_inputs, &metadata)
        .and_then(|preparation| {
            preparation.with_source_file_manifest(
                &compilation_inputs,
                &view.dependency_root.join("source-files.json"),
            )
        })
        .map_err(|error| step("rust-metadata-binding", error))?;
    let graph_paths =
        RustCompilationPrivatePaths::prepare(&view.output_root, &format!("unit-graph-{attempt}"))
            .map_err(|error| step("rust-unit-graph-output", error))?;
    let graph_profile = profile(&compilation_inputs, &graph_paths)?;
    let graph_plan = crate::rust_compilation_trust::compile_rust_unit_graph_launch_plan(
        &policy,
        &capabilities,
        &graph_profile,
        &compilation_inputs,
        &graph_paths,
        &request,
    )
    .map_err(|error| step("rust-unit-graph-plan", error))?;
    let graph_terminal = supervise_rust_compilation(
        &graph_plan,
        &graph_paths,
        &ProviderSandboxLauncher::new(capabilities.clone()),
        &graph_profile,
        ProviderSandboxLaunchMaterial::LinuxSeccomp(&seccomp),
        &RustCompilationCancellationSignal::default().with_scope_cancellation(cancellation.clone()),
    )
    .map_err(|error| step("rust-unit-graph", error))?;
    let graph = graph_plan
        .read_unit_graph(&graph_paths, &graph_terminal)
        .map_err(|error| step("rust-unit-graph-result", error))?;
    unit_graphs.push(unit_graph::SelectedUnitGraph {
        captured: unit_graph::capture(graph, inputs.budget())?,
        context: decode_public_id(
            IdentityDomain::AnalysisContext,
            None,
            &product.context.analysis_context_id,
        )
        .map_err(|error| step("rust-unit-graph-context", error))?,
        run: run_id,
    });
    let mut context_charge = crate::inventory::reserve_memory(inputs.budget(), 4096)
        .map_err(|error| step("rust-context-memory", error))?;
    let context = ProviderContextBinding::try_new(
        ContextIdentity::try_new(product.context.analysis_context_id.clone())
            .map_err(|error| step("rust-context-id", error))?,
        decode_public_id(
            IdentityDomain::AnalysisContext,
            None,
            &product.context.analysis_context_id,
        )
        .map_err(|error| step("rust-context-id", error))?,
        context_pin,
        context_pin,
    )
    .map_err(|error| step("rust-provider-context", error))?;
    let retained = context
        .memory_bytes()
        .map_err(|error| step("rust-context-memory", error))?;
    context_charge
        .shrink(crate::resource_budget::ResourceAmounts {
            memory_bytes: 4096_u64
                .checked_sub(retained)
                .ok_or_else(|| step("rust-context-memory", "context exceeds reservation"))?,
            ..Default::default()
        })
        .map_err(|error| step("rust-context-memory", error))?;
    let prepared = release
        .providers()
        .prepare_job(
            release.policy(),
            ProviderJobInput {
                lane: ProviderLane::Rustc,
                source: ProviderSourceBinding::from_inventory(
                    SourceIdentity::try_new(
                        request.context.source_snapshot_manifest_digest.clone(),
                    )
                    .map_err(|error| step("rust-source-id", error))?,
                    inventory,
                ),
                context: context_charge.into_charged_value(context),
                run: ProviderRunBinding::try_new(
                    ProviderRunIdentity::try_new(request.context.provider_run_id.clone())
                        .map_err(|error| step("rust-run-id", error))?,
                    run_id,
                )
                .map_err(|error| step("rust-run-id", error))?,
                scope: ProviderScopeIdentity::try_new("workspace")
                    .map_err(|error| step("rust-scope", error))?,
                requested_families: release
                    .providers()
                    .families(ProviderLane::Rustc)
                    .map_err(|error| step("rust-families", error))?
                    .into_iter()
                    .map(|family| (family, 1))
                    .collect(),
                operational_ceilings: ProviderResourceCeilings::try_new(
                    ProviderResourceCeilingSpec {
                        max_relations: 128,
                        max_batches_per_relation: 65_536,
                        max_input_bytes: 64 * 1024 * 1024,
                        max_rows: 4_000_000,
                        max_bytes: 512 * 1024 * 1024,
                        max_diagnostics: 20_000,
                        max_work_units: 20_000_000,
                        max_wall_millis: 120_000,
                        max_visited_nodes: 4_000_000,
                        max_traversal_depth: 512,
                        max_workers: workers,
                        max_retained_revisions: 1,
                        cancellation_poll_work_units: 1_024,
                        cancellation_ack_millis: 2_000,
                    },
                )
                .map_err(|error| step("rust-ceilings", error))?,
                deadline: Instant::now() + Duration::from_secs(120),
                cancellation: CancellationProbe::from_cancellation(cancellation, 1024)
                    .map_err(|error| step("rust-cancellation", error))?,
                resource_budget,
            },
        )
        .map_err(|error| step("rust-provider-job", error))?;
    let identity: serde_json::Value = serde_json::from_slice(TOOLCHAIN_IDENTITY)
        .map_err(|error| step("rust-toolchain-identity", error))?;
    let paths =
        RustCompilationPrivatePaths::prepare(&view.output_root, &format!("compiler-{attempt}"))
            .map_err(|error| step("rust-output", error))?;
    let profile = profile(&compilation_inputs, &paths)?;
    let admission = RustcRunAdmission {
        provider_run_id: request.context.provider_run_id.clone(),
        workspace_id,
        analysis_context_id: product.context.analysis_context_id.clone(),
        canonical_workspace_id: record.workspace_id,
        canonical_analysis_context_id: prepared.job().context().analysis_context_id(),
        source_generation: request.context.source_generation,
        context_manifest_digest: request.context.context_manifest_digest.clone(),
        source_snapshot_manifest_digest: request.context.source_snapshot_manifest_digest.clone(),
        resource_profile_id: request.context.resource_profile_id.clone(),
    };
    let child = scope
        .child(&format!("rust-compiler-{}", lower_hex(&run_id)))
        .map_err(|error| step("rust-task-scope", error))?;
    tokio::runtime::Handle::current().block_on(async {
        let result = Box::pin(run_untrusted_rustc_provider_lifecycle(
            UntrustedRustcProviderLifecycle {
                cpu_lease: Some(cpu),
                task_scope: child.clone(),
                provider_job: prepared.job(),
                trust_policy: &policy,
                sandbox_capabilities: &capabilities,
                sandbox_profile: &profile,
                compilation_inputs: &compilation_inputs,
                private_paths: &paths,
                compilation_request: &request,
                protocol_policy: RustcProtocolPolicy {
                    daemon_build: format!("codefabric {}", env!("CARGO_PKG_VERSION")),
                    output_schema_bundle_digest: schema_bundle_digest(),
                    sandbox_profile_digest: profile.sha256_digest.clone(),
                    extractor_build: identity["extractor"].as_str().unwrap_or_default().into(),
                    rustc_version: RUSTC_PUBLIC_RELEASE.into(),
                    rustc_commit: identity["rustc_commit_hash"]
                        .as_str()
                        .unwrap_or_default()
                        .into(),
                    toolchain_identity_digest: frame_digest(digest_bytes(TOOLCHAIN_IDENTITY)),
                    supported_feature_bits: 0,
                    provider_deadline_unix_ms: i64::try_from(
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis(),
                    )
                    .unwrap_or(i64::MAX - 120_000)
                        + 120_000,
                },
                run_admission: admission,
                allowed_uid: rustix::process::getuid().as_raw(),
                launch_material: ProviderSandboxLaunchMaterial::LinuxSeccomp(&seccomp),
            },
        ))
        .await;
        child
            .cancel_and_join(Duration::from_secs(10))
            .await
            .map_err(|error| step("rust-process-join", error))?;
        let result = result.map_err(|error| step("rust-provider", error))?;
        let runs = result.compilations().to_vec();
        let admitted = release
            .providers()
            .admit(prepared, result.into_result())
            .map_err(|error| step("rust-result-admission", error))?;
        Ok((ContextPin(context_pin), admitted, runs))
    })
}

fn public_id(
    domain: IdentityDomain,
    bytes: [u8; 16],
) -> Result<String, ProductionWorkspaceStartupError> {
    encode_public_id(domain, None, bytes).map_err(|error| step("rust-canonical-id", error))
}

fn profile(
    inputs: &RustCompilationInputs,
    paths: &RustCompilationPrivatePaths,
) -> Result<GeneratedSandboxProfile, ProductionWorkspaceStartupError> {
    GeneratedSandboxProfile::generate(
        ProviderTrustProfile::UntrustedSandboxed,
        SandboxMechanism::LinuxBubblewrap,
        &inputs.workspace_view,
        &inputs.dependency_view,
        &paths.run_root,
    )
    .map_err(|error| step("rust-sandbox-profile", error))
}

fn dependency(path: &str, bytes: Vec<u8>, executable: bool) -> DependencyInput {
    DependencyInput {
        raw_relative_path_bytes: path.as_bytes().to_vec(),
        digest: digest_bytes(&bytes),
        bytes: Arc::from(bytes),
        mode: if executable { 0o500 } else { 0o400 },
    }
}

type SharedToolchain = OnceLock<
    Result<crate::resource_budget::ChargedValue<ToolchainInputs>, ProductionWorkspaceStartupError>,
>;

struct ToolchainInputs {
    dependencies: DependencyInputBundle,
    host: String,
    runtime_artifacts: Vec<ContextArtifactInput>,
    selection: ToolchainSelectionKey,
    witnesses: Vec<FileWitness>,
}

struct SelectedToolchainInputs {
    key: ToolchainSelectionKey,
    linker: Vec<DependencyInput>,
    runtime_artifacts: Vec<ContextArtifactInput>,
}

fn capture_toolchain(
    budget: &crate::resource_budget::ResourceBudget,
    cancellation: &Cancellation,
    selection: SelectedToolchainInputs,
) -> Result<crate::resource_budget::ChargedValue<ToolchainInputs>, ProductionWorkspaceStartupError>
{
    let started = Instant::now();
    let mut capacity = crate::inventory::reserve_memory(budget, MAX_TOOLCHAIN_BYTES * 2)
        .map_err(|error| step("rust-toolchain-memory", error))?;
    let toolchain = toolchain_inputs(cancellation, selection)?;
    if !toolchain.unchanged(cancellation)? {
        return Err(step(
            "rust-toolchain-file",
            "installed toolchain changed during capture",
        ));
    }
    let retained = toolchain.memory_bytes()?;
    capacity
        .shrink(crate::resource_budget::ResourceAmounts {
            memory_bytes: (MAX_TOOLCHAIN_BYTES * 2)
                .checked_sub(retained)
                .ok_or_else(|| step("rust-toolchain-memory", "captured bundle exceeds capacity"))?,
            ..Default::default()
        })
        .map_err(|error| step("rust-toolchain-memory", error))?;
    tracing::info!(
        input_bytes = retained,
        files = toolchain.dependencies.entries.len(),
        elapsed_ms = started.elapsed().as_millis(),
        digest = %frame_digest(toolchain.dependencies.manifest_digest),
        "captured shared Rust toolchain inputs"
    );
    Ok(capacity.into_charged_value(toolchain))
}

impl ToolchainInputs {
    fn unchanged(
        &self,
        cancellation: &Cancellation,
    ) -> Result<bool, ProductionWorkspaceStartupError> {
        for witness in &self.witnesses {
            if cancellation.is_cancelled() {
                return Err(step(
                    "rust-toolchain-cancelled",
                    "cancelled during deployment validation",
                ));
            }
            if !witness.unchanged() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn memory_bytes(&self) -> Result<u64, ProductionWorkspaceStartupError> {
        let mut bytes = std::mem::size_of::<Self>() as u64
            + self.host.capacity() as u64
            + self.dependencies.entries.capacity() as u64
                * std::mem::size_of::<DependencyInput>() as u64
            + self.runtime_artifacts.capacity() as u64
                * std::mem::size_of::<ContextArtifactInput>() as u64
            + self.witnesses.capacity() as u64 * std::mem::size_of::<FileWitness>() as u64
            + self
                .witnesses
                .iter()
                .map(FileWitness::memory_bytes)
                .sum::<u64>()
            + (self.selection.root.capacity()
                + self.selection.extractor.capacity()
                + self.selection.host.capacity()) as u64;
        for entry in &self.dependencies.entries {
            bytes = bytes
                .checked_add(entry.bytes.len() as u64)
                .and_then(|bytes| {
                    bytes.checked_add(entry.raw_relative_path_bytes.capacity() as u64 + 64)
                })
                .ok_or_else(|| step("rust-toolchain-memory", "bundle memory size overflow"))?;
        }
        for artifact in &self.runtime_artifacts {
            bytes = bytes
                .checked_add(artifact.file_id.capacity() as u64)
                .ok_or_else(|| step("rust-toolchain-memory", "artifact memory size overflow"))?;
        }
        Ok(bytes)
    }
}

fn selected_toolchain(
    cancellation: &Cancellation,
) -> Result<(PathBuf, String), ProductionWorkspaceStartupError> {
    let mut command = std::process::Command::new("rustup");
    command.args(["which", "--toolchain", RUSTC_TOOLCHAIN, "rustc"]);
    let selected = crate::fabric::provider_deployment::command_output(command, cancellation)
        .map_err(|error| step("rust-toolchain-location", error))?;
    if !selected.status.success() {
        return Err(step(
            "rust-toolchain-location",
            "selected dated nightly is unavailable",
        ));
    }
    let rustc = PathBuf::from(
        String::from_utf8(selected.stdout)
            .map_err(|error| step("rust-toolchain-location", error))?
            .trim(),
    );
    let mut command = std::process::Command::new(&rustc);
    command.arg("-vV");
    let version = crate::fabric::provider_deployment::command_output(command, cancellation)
        .map_err(|error| step("rust-toolchain-version", error))?;
    let version =
        String::from_utf8(version.stdout).map_err(|error| step("rust-toolchain-version", error))?;
    let identity: serde_json::Value = serde_json::from_slice(TOOLCHAIN_IDENTITY)
        .map_err(|error| step("rust-toolchain-identity", error))?;
    if !version.lines().any(|line| {
        line == format!(
            "commit-hash: {}",
            identity["rustc_commit_hash"].as_str().unwrap_or_default()
        )
    }) {
        return Err(step(
            "rust-toolchain-version",
            "installed compiler differs from selected extractor",
        ));
    }
    let host = version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or_else(|| step("rust-toolchain-host", "missing compiler host"))?
        .to_owned();
    let root = rustc
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| step("rust-toolchain-location", "missing sysroot"))?
        .to_owned();
    Ok((root, host))
}

fn select_toolchain_inputs(
    cancellation: &Cancellation,
) -> Result<SelectedToolchainInputs, ProductionWorkspaceStartupError> {
    if cancellation.is_cancelled() {
        return Err(step(
            "rust-toolchain-cancelled",
            "cancelled before deployment selection",
        ));
    }
    let (root, host) = selected_toolchain(cancellation)?;
    let root =
        std::fs::canonicalize(root).map_err(|error| step("rust-toolchain-location", error))?;
    let extractor = crate::fabric::provider_deployment::ProviderExecutable::RustcExtractor
        .selected_path()
        .map_err(|error| step("rust-extractor-location", error))?;
    let mut linker = Vec::new();
    let runtime_artifacts = host_c_compiler_inputs(&mut linker, cancellation)?;
    let linker =
        DependencyInputBundle::pin(linker).map_err(|error| step("rust-host-linker", error))?;
    Ok(SelectedToolchainInputs {
        key: ToolchainSelectionKey {
            root,
            host,
            extractor,
            linker_manifest: linker.manifest_digest,
        },
        linker: linker.entries,
        runtime_artifacts,
    })
}

fn toolchain_inputs(
    cancellation: &Cancellation,
    selected: SelectedToolchainInputs,
) -> Result<ToolchainInputs, ProductionWorkspaceStartupError> {
    let root = &selected.key.root;
    let mut entries = Vec::new();
    entries.push(dependency(
        "cargo-home/config.toml",
        b"[net]\noffline = true\n".to_vec(),
        false,
    ));
    entries.push(dependency(
        "rustup-home/selected-toolchain",
        RUSTC_TOOLCHAIN.as_bytes().to_vec(),
        false,
    ));
    let mut bytes = 0;
    let mut witnesses = Vec::new();
    for path in crate::fabric::provider_deployment::COMPILER_INPUT_ROOTS {
        collect_toolchain(
            root,
            &root.join(path),
            &mut entries,
            &mut bytes,
            cancellation,
            &mut witnesses,
        )?;
    }
    let witness = FileWitness::read(&selected.key.extractor)?;
    let executable = capture_file(
        &selected.key.extractor,
        &witness,
        MAX_TOOLCHAIN_BYTES.saturating_sub(bytes),
        cancellation,
    )?;
    witnesses.push(witness);
    entries.push(dependency(
        "extractor/codefabric-rustc-extractor",
        executable,
        true,
    ));
    entries.push(dependency("toolchain/bin/codefabric-rustc-extractor", b"#!/bin/sh\nexport LD_LIBRARY_PATH=/dependencies/toolchain/lib\nexec /dependencies/extractor/codefabric-rustc-extractor \"$@\"\n".to_vec(), true));
    entries.extend(selected.linker);
    let input_bytes = entries
        .iter()
        .map(|entry| entry.bytes.len() as u64)
        .sum::<u64>();
    if input_bytes > MAX_TOOLCHAIN_BYTES {
        return Err(step(
            "rust-toolchain-size",
            "complete toolchain input budget exceeded",
        ));
    }
    Ok(ToolchainInputs {
        dependencies: DependencyInputBundle::pin(entries)
            .map_err(|error| step("rust-toolchain-bundle", error))?,
        host: selected.key.host.clone(),
        runtime_artifacts: selected.runtime_artifacts,
        selection: selected.key,
        witnesses,
    })
}

fn host_c_compiler_inputs(
    entries: &mut Vec<DependencyInput>,
    cancellation: &Cancellation,
) -> Result<Vec<ContextArtifactInput>, ProductionWorkspaceStartupError> {
    let mut artifacts = Vec::new();
    // /usr is the selected read-only runtime image, but Debian's cc alias crosses
    // /etc/alternatives. Capture the selected executable, without exposing /etc.
    if let Ok(compiler) = std::fs::canonicalize("/usr/bin/cc") {
        if !compiler.starts_with("/usr") {
            return Err(step(
                "rust-host-linker",
                "system compiler escapes the selected runtime image",
            ));
        }
        let witness = FileWitness::read(Path::new("/usr/bin/cc"))?;
        let captured = capture_file(&compiler, &witness, 32 * 1024 * 1024, cancellation)?;
        artifacts.push(ContextArtifactInput {
            file_id: "toolchain:host-c-compiler".to_owned(),
            digest: digest_bytes(&captured),
        });
        let mut command = std::process::Command::new(&compiler);
        command.env_clear().arg("-print-libgcc-file-name");
        let libraries = crate::fabric::provider_deployment::command_output(command, cancellation)
            .map_err(|error| step("rust-host-linker-search", error))?;
        if !libraries.status.success() {
            return Err(step(
                "rust-host-linker-search",
                "system compiler did not identify its runtime libraries",
            ));
        }
        let library = String::from_utf8(libraries.stdout)
            .map_err(|error| step("rust-host-linker-search", error))?;
        let library = Path::new(library.trim());
        let prefix = library.parent().and_then(Path::to_str).ok_or_else(|| {
            step(
                "rust-host-linker-search",
                "system compiler returned no library prefix",
            )
        })?;
        if !library.is_file()
            || !library.starts_with("/usr")
            || !prefix
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"/._-".contains(&byte))
        {
            return Err(step(
                "rust-host-linker-search",
                "compiler runtime libraries escape the selected image or have an unsupported path",
            ));
        }
        let wrapper =
            format!("#!/bin/sh\nexec /dependencies/toolchain/bin/cc-driver -B{prefix}/ \"$@\"\n")
                .into_bytes();
        artifacts.push(ContextArtifactInput {
            file_id: "toolchain:host-c-compiler-search".to_owned(),
            digest: digest_bytes(&wrapper),
        });
        entries.push(dependency("toolchain/bin/cc-driver", captured, true));
        entries.push(dependency("toolchain/bin/cc", wrapper, true));
        if !witness.unchanged() || cancellation.is_cancelled() {
            return Err(step(
                "rust-host-linker",
                "system compiler changed or selection was cancelled",
            ));
        }
    }
    Ok(artifacts)
}

fn collect_toolchain(
    root: &Path,
    path: &Path,
    entries: &mut Vec<DependencyInput>,
    bytes: &mut u64,
    cancellation: &Cancellation,
    witnesses: &mut Vec<FileWitness>,
) -> Result<(), ProductionWorkspaceStartupError> {
    if cancellation.is_cancelled() {
        return Err(step(
            "rust-toolchain-cancelled",
            "cancelled during toolchain capture",
        ));
    }
    let exact = std::fs::canonicalize(path).map_err(|error| step("rust-toolchain-file", error))?;
    if !exact.starts_with(root) {
        return Err(step(
            "rust-toolchain-file",
            "toolchain symlink escapes selected root",
        ));
    }
    let metadata = std::fs::metadata(&exact).map_err(|error| step("rust-toolchain-file", error))?;
    if witnesses.len() >= crate::fabric::provider_deployment::MAX_COMPILER_INPUT_ENTRIES {
        return Err(step(
            "rust-toolchain-size",
            "toolchain entry budget exceeded",
        ));
    }
    let witness = FileWitness::read(path)?;
    if !witness.matches_metadata(&metadata) {
        return Err(step(
            "rust-toolchain-file",
            "toolchain changed before capture",
        ));
    }
    witnesses.push(witness.clone());
    if metadata.is_dir() {
        if path
            .components()
            .count()
            .saturating_sub(root.components().count())
            > 64
        {
            return Err(step(
                "rust-toolchain-directory",
                "toolchain directory depth exceeded",
            ));
        }
        for entry in
            std::fs::read_dir(path).map_err(|error| step("rust-toolchain-directory", error))?
        {
            collect_toolchain(
                root,
                &entry
                    .map_err(|error| step("rust-toolchain-directory", error))?
                    .path(),
                entries,
                bytes,
                cancellation,
                witnesses,
            )?;
        }
    } else if metadata.is_file() {
        if metadata.len() > MAX_TOOLCHAIN_BYTES.saturating_sub(*bytes)
            || entries.len() >= crate::fabric::provider_deployment::MAX_COMPILER_INPUT_ENTRIES
        {
            return Err(step(
                "rust-toolchain-size",
                "toolchain input budget exceeded",
            ));
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|error| step("rust-toolchain-path", error))?
            .to_str()
            .ok_or_else(|| step("rust-toolchain-path", "non-UTF-8 toolchain path"))?;
        let captured = capture_file(
            &exact,
            &witness,
            MAX_TOOLCHAIN_BYTES.saturating_sub(*bytes),
            cancellation,
        )?;
        *bytes = bytes.saturating_add(captured.len() as u64);
        if *bytes > MAX_TOOLCHAIN_BYTES || captured.len() as u64 != metadata.len() {
            return Err(step(
                "rust-toolchain-file",
                "toolchain changed during bounded capture",
            ));
        }
        entries.push(dependency(
            &format!("toolchain/{relative}"),
            captured,
            metadata.permissions().mode() & 0o111 != 0,
        ));
    } else {
        return Err(step(
            "rust-toolchain-file",
            "unsupported toolchain file kind",
        ));
    }
    Ok(())
}

fn capture_file(
    path: &Path,
    witness: &FileWitness,
    maximum_bytes: u64,
    cancellation: &Cancellation,
) -> Result<Vec<u8>, ProductionWorkspaceStartupError> {
    let mut file = std::fs::File::open(path).map_err(|error| step("rust-toolchain-file", error))?;
    if !file
        .metadata()
        .is_ok_and(|value| value.is_file() && value.len() <= maximum_bytes)
        || !witness.matches_file(&file)
    {
        return Err(step(
            "rust-toolchain-file",
            "toolchain file changed or exceeds its bound",
        ));
    }
    let mut bytes = Vec::new();
    let mut chunk = vec![0_u8; 64 * 1024];
    loop {
        if cancellation.is_cancelled() {
            return Err(step(
                "rust-toolchain-cancelled",
                "cancelled during toolchain read",
            ));
        }
        let count = file
            .read(&mut chunk)
            .map_err(|error| step("rust-toolchain-file", error))?;
        if count == 0 {
            break;
        }
        if bytes.len() as u64 + count as u64 > maximum_bytes {
            return Err(step(
                "rust-toolchain-file",
                "toolchain read exceeds its bound",
            ));
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    if !witness.matches_file(&file) || !witness.unchanged() {
        return Err(step(
            "rust-toolchain-file",
            "toolchain changed during bounded capture",
        ));
    }
    Ok(bytes)
}

fn initial_selection(
    files: &[ContextFileInput],
    selected: &targets::CargoTarget,
    toolchain: &ToolchainInputs,
    workers: u16,
) -> Result<RustContextSelection, ProductionWorkspaceStartupError> {
    let identity: serde_json::Value = serde_json::from_slice(TOOLCHAIN_IDENTITY)
        .map_err(|error| step("rust-toolchain-identity", error))?;
    // Captured inputs are the only material available to Cargo. Contained metadata must
    // still resolve the complete locked graph before these inputs authorize compilation.
    let dependency_roots = files
        .iter()
        .filter(|file| file.relative_path != selected.manifest)
        .filter_map(|file| file.relative_path.strip_suffix(b"/Cargo.toml"))
        .collect::<Vec<_>>();
    let mut dependency_inputs: Vec<_> = files
        .iter()
        .filter(|file| {
            dependency_roots.iter().any(|root| {
                file.relative_path
                    .strip_prefix(*root)
                    .is_some_and(|suffix| suffix.starts_with(b"/"))
            })
        })
        .map(|file| ContextArtifactInput {
            file_id: file.file_id.clone(),
            digest: file.digest,
        })
        .collect();
    dependency_inputs.extend_from_slice(&toolchain.runtime_artifacts);
    let cargo_workspace_root = if let Some(path) = &selected.build.inherited_workspace {
        let manifest = files
            .iter()
            .find(|file| file.relative_path == *path)
            .ok_or_else(|| {
                step(
                    "rust-context-configuration",
                    "inherited workspace manifest is not captured",
                )
            })?;
        if !dependency_inputs
            .iter()
            .any(|artifact| artifact.file_id == manifest.file_id)
        {
            dependency_inputs.push(ContextArtifactInput {
                file_id: manifest.file_id.clone(),
                digest: manifest.digest,
            });
        }
        let root = path
            .strip_suffix(b"Cargo.toml")
            .expect("captured manifest path");
        Some(if root.is_empty() {
            b".".to_vec()
        } else {
            root.strip_suffix(b"/").unwrap_or(root).to_vec()
        })
    } else {
        None
    };
    let build_inputs = targets::build_inputs(files)?;
    Ok(RustContextSelection {
        // Cargo exports NUM_JOBS to build scripts. Pin the actual granted width as selected
        // build environment before deriving a context; resource policy alone is not identity.
        environment: vec![crate::analysis_context::RustEnvironmentSetting {
            name: "CARGO_BUILD_JOBS".into(),
            value: workers.to_string(),
        }],
        manifest_path: Some(selected.manifest.clone()),
        cargo_workspace_root,
        package_name: Some(selected.package.clone()),
        target: Some(selected.target.clone()),
        requested_features: selected
            .build
            .features
            .clone()
            .ok_or_else(|| step("rust-context-configuration", "features are unavailable"))?,
        default_features: selected.build.default_features.ok_or_else(|| {
            step(
                "rust-context-configuration",
                "default feature selection is unavailable",
            )
        })?,
        target_triple: Some(
            selected
                .target_triple
                .as_deref()
                .filter(|value| *value != "host-tuple")
                .unwrap_or(&toolchain.host)
                .to_owned(),
        ),
        profile: selected.build.profile.clone(),
        toolchain: Some(RustToolchainSettings {
            release: toolchain_release(),
            commit_hash: identity["rustc_commit_hash"]
                .as_str()
                .unwrap_or_default()
                .into(),
            artifact_digest: digest_bytes(TOOLCHAIN_IDENTITY),
        }),
        sysroot: Some(ContextArtifactInput {
            file_id: "toolchain:captured-inputs".into(),
            digest: toolchain.dependencies.manifest_digest,
        }),
        dependency_inputs: Some(dependency_inputs),
        build_inputs: Some(build_inputs),
        ..RustContextSelection::default()
    })
}

fn captured_files(
    inputs: &ProviderInputs<'_>,
) -> Result<Vec<ContextFileInput>, ProductionWorkspaceStartupError> {
    inputs
        .capture()
        .images()
        .iter()
        .map(|image| {
            Ok(ContextFileInput {
                file_id: public_id(IdentityDomain::SourceFile, image.file_id)?,
                relative_path: image.path.raw_relative_path_bytes.clone(),
                digest: image.digest,
                contents: image.bytes.to_vec(),
            })
        })
        .collect::<Result<Vec<_>, ProductionWorkspaceStartupError>>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn captured_toolchain(root: &Path) -> ToolchainInputs {
        let mut entries = Vec::new();
        let mut witnesses = Vec::new();
        collect_toolchain(
            root,
            &root.join("lib"),
            &mut entries,
            &mut 0,
            &Cancellation::default(),
            &mut witnesses,
        )
        .unwrap();
        ToolchainInputs {
            dependencies: DependencyInputBundle::pin(entries).unwrap(),
            host: "x86_64-unknown-linux-gnu".to_owned(),
            runtime_artifacts: Vec::new(),
            selection: ToolchainSelectionKey {
                root: root.to_owned(),
                host: "x86_64-unknown-linux-gnu".to_owned(),
                extractor: root.join("extractor"),
                linker_manifest: [0; 32],
            },
            witnesses,
        }
    }

    fn context_for_toolchain(toolchain: &ToolchainInputs, generation: u64, workers: u16) -> String {
        let files = [
            (
                "Cargo.toml",
                "[package]\nname = 'sample'\nversion = '0.1.0'\nedition = '2024'\n",
            ),
            ("src/lib.rs", "pub fn leaf() {}\n"),
            (
                "Cargo.lock",
                "version = 4\n[[package]]\nname = 'sample'\nversion = '0.1.0'\n",
            ),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (path, contents))| ContextFileInput {
            file_id: format!("file:{index:032x}"),
            relative_path: path.as_bytes().to_vec(),
            digest: digest_bytes(contents.as_bytes()),
            contents: contents.as_bytes().to_vec(),
        })
        .collect::<Vec<_>>();
        let selected = targets::discover(&files).unwrap().remove(0);
        let selection = initial_selection(&files, &selected, toolchain, workers).unwrap();
        let product = discover_rust_context(&RustContextDiscoveryRequest {
            workspace_id: format!("workspace:{:032x}", 1),
            source_generation: generation,
            provider_bundle_version: "codefabric-rust-compiler-v1".into(),
            files,
            search_scope: ContextSearchScope {
                namespace: b".".to_vec(),
                ordered_roots: vec![ContextSearchRoot {
                    root_id: "workspace-root".into(),
                    relative_path: b".".to_vec(),
                }],
                policy_identity: [1; 32],
                universe: ContextSearchUniverse::Closed {
                    inventory_identity: [2; 32],
                },
            },
            selection,
        })
        .unwrap();
        let RustContextDiscoveryOutcome::Prepared(product) = product else {
            panic!("expected captured context")
        };
        product.context.analysis_context_id.clone()
    }

    #[test]
    fn rust_context_tracks_captured_toolchain_bytes_without_generation_or_host_path_drift() {
        let first_root = tempfile::tempdir().unwrap();
        let second_root = tempfile::tempdir().unwrap();
        for root in [first_root.path(), second_root.path()] {
            std::fs::create_dir(root.join("lib")).unwrap();
            std::fs::write(root.join("lib/runtime.so"), b"first runtime").unwrap();
        }
        let first = captured_toolchain(first_root.path());
        let relocated = captured_toolchain(second_root.path());
        let before = context_for_toolchain(&first, 1, 2);
        assert_eq!(before, context_for_toolchain(&relocated, 2, 2));
        assert_ne!(before, context_for_toolchain(&relocated, 2, 3));
        std::fs::write(second_root.path().join("lib/runtime.so"), b"other runtime").unwrap();
        let changed = captured_toolchain(second_root.path());
        assert_ne!(before, context_for_toolchain(&changed, 2, 2));
        // The original captured view remains exact after the installed input changes.
        assert_eq!(&*relocated.dependencies.entries[0].bytes, b"first runtime");
        assert_eq!(before, context_for_toolchain(&relocated, 3, 2));
        assert!(first.memory_bytes().unwrap() >= b"first runtime".len() as u64);
    }
}
