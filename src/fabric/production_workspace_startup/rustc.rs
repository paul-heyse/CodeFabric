//! Captured Rust workspace preparation, contained Cargo, and accepted Arrow publication.

use std::collections::BTreeMap;
use std::io::Read as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
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
use crate::identity::{IdentityDomain, decode_public_id, encode_public_id};
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

use super::inputs::PreparedSourceInputs;
use super::{CompiledSemanticRelease, ProductionWorkspaceStartupError, digest16, lower_hex, step};

mod targets;

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
    runs: Vec<TrustQualifiedRustcCompilation>,
    gap: ProviderLaneGap,
}

pub(super) struct RustTargetProgress {
    pub manifest: Vec<u8>,
    pub target: String,
    pub target_kind: String,
    pub context_id: Option<[u8; 16]>,
    pub state: &'static str,
    pub detail: String,
}

impl RustcOutcome {
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
}

pub(super) fn run(
    root: &Path,
    release: &CompiledSemanticRelease,
    inputs: &PreparedSourceInputs,
    record: &WorkspaceRecord,
    cancellation: &Cancellation,
    work: super::PublicationWork<'_>,
) -> Result<RustcOutcome, ProductionWorkspaceStartupError> {
    let super::PublicationWork { scope, stage, .. } = work;
    let inventory = inputs.inventory_for_language(SourceLanguage::Rust)?;
    let mut outcome = RustcOutcome {
        source_pin: SourcePin(inventory.identity()),
        context_pin: ContextPin(record.context_fingerprint),
        admitted: Vec::new(),
        progress: Vec::new(),
        runs: Vec::new(),
        gap: ProviderLaneGap::RequiredInputAbsent,
    };
    if inventory.selected_files().next().is_none() {
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
    if stage == super::PublicationStage::Source {
        outcome.gap = ProviderLaneGap::Pending;
        outcome.progress = targets
            .into_iter()
            .map(|target| RustTargetProgress {
                manifest: target.manifest,
                target: target.target.name,
                target_kind: target.target.kind.as_str().to_owned(),
                context_id: None,
                state: "pending",
                detail: "semantic_work_pending".to_owned(),
            })
            .collect();
        return Ok(outcome);
    }
    let mut contexts = Vec::new();
    for target in targets {
        let mut progress = RustTargetProgress {
            manifest: target.manifest.clone(),
            target: target.target.name.clone(),
            target_kind: target.target.kind.as_str().to_owned(),
            context_id: None,
            state: "unavailable",
            detail: String::new(),
        };
        let available = prepare_and_run(
            root,
            release,
            inputs,
            record,
            inventory.clone(),
            &target,
            scope,
            cancellation.clone(),
        );
        match available {
            Ok((context_pin, admitted, runs)) => {
                progress.context_id = Some(admitted.job().context().analysis_context_id());
                progress.state = if runs.is_empty() {
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
    Ok(outcome)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)] // One owned capture-to-provider transaction.
fn prepare_and_run(
    root: &Path,
    release: &CompiledSemanticRelease,
    inputs: &PreparedSourceInputs,
    record: &WorkspaceRecord,
    inventory: crate::resource_budget::ChargedValue<
        crate::provider_contracts::ProviderSourceInventory,
    >,
    target: &targets::CargoTarget,
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
    let capabilities = SandboxCapabilityMatrix::probe_current_host();
    if !capabilities
        .row(ProviderTrustProfile::UntrustedSandboxed)
        .is_some_and(|row| row.available)
    {
        return Err(step("rust-containment", "contained compiler unavailable"));
    }
    // Toolchain bytes are transient, bounded, and shared by the published immutable view.
    let bundle_memory = crate::inventory::reserve_memory(inputs.budget(), MAX_TOOLCHAIN_BYTES * 2)
        .map_err(|error| step("rust-toolchain-memory", error))?;
    let (mut dependencies, host) = toolchain_inputs(&cancellation)?;
    let workspace_id = public_id(IdentityDomain::Workspace, record.workspace_id)?;
    let files = captured_files(inputs)?;
    let source_manifest = RustSourceFileManifest {
        workspace_id: workspace_id.clone(),
        source_generation: inventory.source_generation(),
        files: files
            .iter()
            .map(|file| {
                Ok((
                    String::from_utf8(file.relative_path.clone())
                        .map_err(|error| step("rust-source-path", error))?,
                    CapturedRustSourceFile {
                        file_id: file.file_id.clone(),
                        content_digest: file.digest,
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>, ProductionWorkspaceStartupError>>()?,
    };
    dependencies.push(dependency(
        "source-files.json",
        serde_json::to_vec(&source_manifest)
            .map_err(|error| step("rust-source-manifest", error))?,
        false,
    ));
    let dependencies = DependencyInputBundle::pin(dependencies)
        .map_err(|error| step("rust-dependency-bundle", error))?;
    let selection = initial_selection(&files, &host, target)?;
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
            universe: if inputs.capture()?.dispositions().iter().any(|entry| {
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
    let run_id = digest16(
        b"codefabric.rust-provider-run.v1\0",
        &[&inventory.identity(), &context_pin],
    );
    let images = inputs.capture()?.images().iter().collect::<Vec<_>>();
    let view = publish_provider_workspace_view(
        root,
        &lower_hex(&run_id),
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
    drop(bundle_memory);
    let limits = RustCompilationResourceLimits {
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
    let metadata_paths = RustCompilationPrivatePaths::prepare(&view.output_root, "metadata")
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
        .map_err(|error| step("rust-metadata-result", error))?;
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
                        max_workers: 16,
                        max_retained_revisions: 1,
                        cancellation_poll_work_units: 1_024,
                        cancellation_ack_millis: 2_000,
                    },
                )
                .map_err(|error| step("rust-ceilings", error))?,
                deadline: Instant::now() + Duration::from_secs(120),
                cancellation: CancellationProbe::from_cancellation(cancellation, 1024)
                    .map_err(|error| step("rust-cancellation", error))?,
                resource_budget: inputs
                    .budget()
                    .operation(run_id, inputs.budget().policy())
                    .map_err(|error| step("rust-budget", error))?,
            },
        )
        .map_err(|error| step("rust-provider-job", error))?;
    let identity: serde_json::Value = serde_json::from_slice(TOOLCHAIN_IDENTITY)
        .map_err(|error| step("rust-toolchain-identity", error))?;
    let paths = RustCompilationPrivatePaths::prepare(&view.output_root, "compiler")
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
        .child("rust-compiler")
        .map_err(|error| step("rust-task-scope", error))?;
    tokio::runtime::Handle::current().block_on(async {
        let result = Box::pin(run_untrusted_rustc_provider_lifecycle(
            UntrustedRustcProviderLifecycle {
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

fn toolchain_inputs(
    cancellation: &Cancellation,
) -> Result<(Vec<DependencyInput>, String), ProductionWorkspaceStartupError> {
    let selected = std::process::Command::new("rustup")
        .args(["which", "--toolchain", RUSTC_TOOLCHAIN, "rustc"])
        .output()
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
    let root = rustc
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| step("rust-toolchain-location", "missing sysroot"))?;
    let version = std::process::Command::new(&rustc)
        .arg("-vV")
        .output()
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
    for path in ["bin/cargo", "bin/rustc", "lib"] {
        collect_toolchain(
            root,
            &root.join(path),
            &mut entries,
            &mut bytes,
            cancellation,
        )?;
    }
    let extractor = std::env::var_os("CODEFABRIC_RUSTC_EXTRACTOR_BIN")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe().ok().and_then(|path| {
                path.parent()
                    .map(|parent| parent.join("codefabric-rustc-extractor"))
            })
        })
        .ok_or_else(|| {
            step(
                "rust-extractor-location",
                "extractor executable unavailable",
            )
        })?;
    entries.push(dependency(
        "extractor/codefabric-rustc-extractor",
        std::fs::read(&extractor).map_err(|error| step("rust-extractor-location", error))?,
        true,
    ));
    entries.push(dependency("toolchain/bin/codefabric-rustc-extractor", b"#!/bin/sh\nexport LD_LIBRARY_PATH=/dependencies/toolchain/lib\nexec /dependencies/extractor/codefabric-rustc-extractor \"$@\"\n".to_vec(), true));
    Ok((entries, host))
}

fn collect_toolchain(
    root: &Path,
    path: &Path,
    entries: &mut Vec<DependencyInput>,
    bytes: &mut u64,
    cancellation: &Cancellation,
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
            )?;
        }
    } else if metadata.is_file() {
        if metadata.len() > MAX_TOOLCHAIN_BYTES.saturating_sub(*bytes) || entries.len() >= 100_000 {
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
        let mut captured = Vec::new();
        std::fs::File::open(&exact)
            .map_err(|error| step("rust-toolchain-file", error))?
            .take(MAX_TOOLCHAIN_BYTES.saturating_sub(*bytes) + 1)
            .read_to_end(&mut captured)
            .map_err(|error| step("rust-toolchain-file", error))?;
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

fn initial_selection(
    files: &[ContextFileInput],
    host: &str,
    selected: &targets::CargoTarget,
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
    let dependency_inputs = files
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
    let build_inputs = files
        .iter()
        .filter(|file| file.relative_path.ends_with(b"build.rs"))
        .map(|file| ContextArtifactInput {
            file_id: file.file_id.clone(),
            digest: file.digest,
        })
        .collect();
    Ok(RustContextSelection {
        manifest_path: Some(selected.manifest.clone()),
        package_name: Some(selected.package.clone()),
        target: Some(selected.target.clone()),
        default_features: true,
        target_triple: Some(host.to_owned()),
        profile: Some("dev".into()),
        toolchain: Some(RustToolchainSettings {
            release: toolchain_release(),
            commit_hash: identity["rustc_commit_hash"]
                .as_str()
                .unwrap_or_default()
                .into(),
            artifact_digest: digest_bytes(TOOLCHAIN_IDENTITY),
        }),
        sysroot: Some(ContextArtifactInput {
            file_id: "sysroot:selected".into(),
            digest: digest_bytes(TOOLCHAIN_IDENTITY),
        }),
        dependency_inputs: Some(dependency_inputs),
        build_inputs: Some(build_inputs),
        ..RustContextSelection::default()
    })
}

fn captured_files(
    inputs: &PreparedSourceInputs,
) -> Result<Vec<ContextFileInput>, ProductionWorkspaceStartupError> {
    inputs
        .capture()?
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
