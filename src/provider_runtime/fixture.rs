//! Neutral real-provider fixture launcher shared by integration proofs.
//!
//! This module owns process, admission, and UDS mechanics for provider-backed fixtures. Review
//! candidate code consumes the resulting application-owned DTOs; it does not own provider launch
//! policy.

use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStringExt as _;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use thiserror::Error;

use crate::identity::{IdentityDomain, encode_public_id};
use crate::pyrefly_service::{
    AcceptedPyreflyRun, PyreflyModuleInput, PyreflyProviderDriver, PyreflyRunRequest,
};
use crate::rustc_service::{
    AcceptedRustcCompilation, RustcObservationService, RustcProtocolPolicy, RustcRunAdmission,
    serve_rustc_uds,
};
use crate::source_image::{DependencyInputBundle, SourceImage, publish_provider_workspace_view};

/// Immutable source-image reference admitted to a real provider fixture run.
#[derive(Clone, Debug)]
pub(crate) struct ProviderSourceBlob {
    pub path: PathBuf,
    pub content_digest: String,
    pub file_id: [u8; 16],
    pub image: SourceImage,
}

/// Compatibility-only consumer port colocated with the production `ProviderRuntime` boundary.
/// Direct process launch remains private to this module until each lane installs its production
/// driver behind `RegisteredSemanticProviderAdapter`.
pub(crate) struct CompatibilityProviderRuntimeDispatch<'a> {
    repository_root: &'a Path,
    state_root: &'a Path,
    workspace_id: [u8; 16],
    analysis_context_id: [u8; 16],
    source_generation: u64,
}

impl<'a> CompatibilityProviderRuntimeDispatch<'a> {
    #[must_use]
    pub(crate) const fn new(
        repository_root: &'a Path,
        state_root: &'a Path,
        workspace_id: [u8; 16],
        analysis_context_id: [u8; 16],
        source_generation: u64,
    ) -> Self {
        Self {
            repository_root,
            state_root,
            workspace_id,
            analysis_context_id,
            source_generation,
        }
    }

    pub(crate) async fn pyrefly(
        &self,
        source: &ProviderSourceBlob,
        ffi_source: &ProviderSourceBlob,
        invalid_source: &ProviderSourceBlob,
    ) -> Result<AcceptedPyreflyRun, ProviderFixtureError> {
        self.pyrefly_with_optional_ffi(source, Some(ffi_source), invalid_source)
            .await
    }

    pub(crate) async fn pyrefly_with_optional_ffi(
        &self,
        source: &ProviderSourceBlob,
        ffi_source: Option<&ProviderSourceBlob>,
        invalid_source: &ProviderSourceBlob,
    ) -> Result<AcceptedPyreflyRun, ProviderFixtureError> {
        execute_pyrefly_provider_runtime(
            self.repository_root,
            self.state_root,
            self.workspace_id,
            self.analysis_context_id,
            self.source_generation,
            source,
            ffi_source,
            invalid_source,
        )
        .await
    }

    pub(crate) async fn rustc(
        &self,
        source: &ProviderSourceBlob,
    ) -> Result<AcceptedRustcCompilation, ProviderFixtureError> {
        run_rustc(
            self.repository_root,
            self.state_root,
            source,
            self.workspace_id,
            self.analysis_context_id,
            self.source_generation,
        )
        .await
    }
}

/// Failure while launching or admitting a real provider fixture.
#[derive(Debug, Error)]
pub(crate) enum ProviderFixtureError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("provider fixture invariant failed: {0}")]
    Invariant(String),
}

fn invariant(error: impl std::fmt::Display) -> ProviderFixtureError {
    ProviderFixtureError::Invariant(error.to_string())
}

fn now_millis() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(i64::MAX)
}

fn digest(bytes: &[u8]) -> String {
    crate::integrity::framed_digest(bytes)
}

async fn wait_for_socket(path: &Path) -> Result<(), ProviderFixtureError> {
    for _ in 0..500 {
        if path.exists() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Err(invariant(format!(
        "provider socket did not appear at {}",
        path.display()
    )))
}

fn provider_binary(
    repository_root: &Path,
    relative: &str,
) -> Result<PathBuf, ProviderFixtureError> {
    let path = repository_root.join(relative);
    if !path.is_file() {
        return Err(invariant(format!(
            "required provider fixture binary is absent at {}; run the provider gate first",
            path.display()
        )));
    }
    Ok(path)
}

struct FixtureGenerationOracle(u64);

impl crate::provider_runtime::SourceGenerationOracle for FixtureGenerationOracle {
    fn current_generation(
        &self,
        _workspace_id: &str,
    ) -> Result<u64, crate::provider_runtime::ProviderRuntimeError> {
        Ok(self.0)
    }
}

struct UnusedRustSemanticDriver;

impl crate::provider_runtime::SemanticProviderDriver for UnusedRustSemanticDriver {
    fn execute(
        &self,
        _work: crate::provider_runtime::SemanticProviderWork,
        _events: crate::provider_runtime::ProviderEventSink,
        _cancellation: crate::cancellation::Cancellation,
    ) -> Result<
        crate::provider_runtime::ProviderCompletion,
        crate::provider_runtime::ProviderRuntimeError,
    > {
        Err(crate::provider_runtime::ProviderRuntimeError::Adapter {
            code: "unused Rust semantic fixture lane".to_owned(),
        })
    }
}

fn workspace_view_path(
    view: &crate::source_image::ProviderWorkspaceView,
    source: &ProviderSourceBlob,
) -> PathBuf {
    view.workspace_root.join(OsString::from_vec(
        source.image.path.raw_relative_path_bytes.clone(),
    ))
}

struct ProviderViewCleanup(PathBuf);

impl Drop for ProviderViewCleanup {
    fn drop(&mut self) {
        let _ = remove_owned_read_only_tree(&self.0);
    }
}

fn remove_owned_read_only_tree(path: &Path) -> std::io::Result<()> {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(());
    };
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        for entry in fs::read_dir(path)? {
            remove_owned_read_only_tree(&entry?.path())?;
        }
        fs::remove_dir(path)
    } else {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        fs::remove_file(path)
    }
}

/// Run the pinned Pyrefly sidecar through `ProviderRuntime` and return admitted application DTOs.
#[allow(clippy::too_many_lines)]
// One fixture transaction keeps view publication, runtime admission, journal proof, and result transfer adjacent.
#[allow(clippy::too_many_arguments)] // The fixture makes every admission identity and source role explicit at the call site.
async fn execute_pyrefly_provider_runtime(
    repository_root: &Path,
    state_root: &Path,
    workspace_id: [u8; 16],
    analysis_context_id: [u8; 16],
    source_generation: u64,
    source: &ProviderSourceBlob,
    ffi_source: Option<&ProviderSourceBlob>,
    invalid_source: &ProviderSourceBlob,
) -> Result<AcceptedPyreflyRun, ProviderFixtureError> {
    let binary = provider_binary(repository_root, "target/debug/codefabric-pyrefly-sidecar")?;
    let mut images = vec![&source.image];
    if let Some(ffi_source) = ffi_source {
        images.push(&ffi_source.image);
    }
    images.push(&invalid_source.image);
    let provider_run_id = "50595245464c5950524f564944455230";
    let provider_view_state = state_root.join("pyrefly-provider-view");
    let _provider_view_cleanup = ProviderViewCleanup(provider_view_state.clone());
    let view = publish_provider_workspace_view(
        &provider_view_state,
        provider_run_id,
        &images,
        &DependencyInputBundle::empty(),
        crate::pyrefly_service::SANDBOX_PROFILE_DIGEST,
    )
    .map_err(invariant)?;
    let workspace_id_text =
        encode_public_id(IdentityDomain::Workspace, None, workspace_id).map_err(invariant)?;
    let context_id_text =
        encode_public_id(IdentityDomain::AnalysisContext, None, analysis_context_id)
            .map_err(invariant)?;
    let context_manifest = b"{\"language\":\"python\",\"profile\":\"PYTHON_SEMANTIC_V1\"}".to_vec();
    let source_manifest_digest = crate::integrity::frame_digest(view.manifest_digest);
    let mut modules = vec![PyreflyModuleInput {
        module_id: "module:gate-b-python".to_owned(),
        module_name: "golden_pkg.core".to_owned(),
        file_id: encode_public_id(IdentityDomain::SourceFile, None, source.file_id)
            .map_err(invariant)?,
        source_blob_path: workspace_view_path(&view, source),
        content_digest: source.content_digest.clone(),
    }];
    if let Some(ffi_source) = ffi_source {
        modules.push(PyreflyModuleInput {
            module_id: "module:gate-b-ffi".to_owned(),
            module_name: "ffi.boundary".to_owned(),
            file_id: encode_public_id(IdentityDomain::SourceFile, None, ffi_source.file_id)
                .map_err(invariant)?,
            source_blob_path: workspace_view_path(&view, ffi_source),
            content_digest: ffi_source.content_digest.clone(),
        });
    }
    modules.push(PyreflyModuleInput {
        module_id: "module:gate-b-python-invalid".to_owned(),
        module_name: "malformed.broken".to_owned(),
        file_id: encode_public_id(IdentityDomain::SourceFile, None, invalid_source.file_id)
            .map_err(invariant)?,
        source_blob_path: workspace_view_path(&view, invalid_source),
        content_digest: invalid_source.content_digest.clone(),
    });
    let request = PyreflyRunRequest {
        provider_run_id: provider_run_id.to_owned(),
        workspace_id: workspace_id_text.clone(),
        analysis_context_id: context_id_text.clone(),
        canonical_workspace_id: workspace_id,
        canonical_analysis_context_id: analysis_context_id,
        source_generation,
        context_manifest,
        source_snapshot_lease_id: "lease:gate-b-pyrefly".to_owned(),
        source_manifest_digest,
        modules,
        requested_capability_codes: vec![90, 110],
        deadline_unix_ms: now_millis() + 120_000,
        output_schema_bundle_digest: digest(include_bytes!(
            "../../contracts/schema/schema-contract-ir.json"
        )),
    };
    let driver = Arc::new(
        PyreflyProviderDriver::new(binary, state_root.join("pyrefly-supervisor"))
            .map_err(invariant)?,
    );
    let adapters = crate::provider_runtime::SemanticProviderAdapterRegistry::new(
        driver.clone(),
        Arc::new(UnusedRustSemanticDriver),
    )
    .map_err(invariant)?;
    let operational = crate::operational_store::OperationalStore::open(
        &state_root.join("pyrefly-provider-runtime.sqlite"),
    )
    .map_err(invariant)?;
    let journal: Arc<dyn crate::provider_runtime::ProviderRunJournal> = Arc::new(
        crate::provider_runtime::OperationalProviderRunJournal::new(operational),
    );
    let generation: Arc<dyn crate::provider_runtime::SourceGenerationOracle> =
        Arc::new(FixtureGenerationOracle(source_generation));
    let dispatch = crate::provider_runtime::ProviderRuntimeDispatch::semantic(
        &[0x50; 16],
        &adapters,
        &generation,
        &journal,
    )
    .map_err(invariant)?;
    let scope = crate::provider_runtime::ProviderScope {
        scope_kind: 6,
        scope_id: "PPPPPPPPPPPPPPPP".to_owned(),
    };
    let input_bytes = request
        .modules
        .iter()
        .map(|module| fs::metadata(&module.source_blob_path).map(|value| value.len()))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum();
    let lease_blobs = request
        .modules
        .iter()
        .map(|module| crate::provider_runtime::ProviderBlobReference {
            blob_id: format!("blob:{}", &module.content_digest[3..35]),
            content_digest: module.content_digest.clone(),
            byte_length: fs::metadata(&module.source_blob_path)
                .map(|value| value.len())
                .unwrap_or_default(),
            read_only_uri: format!("file://{}", module.source_blob_path.display()),
        })
        .collect();
    let job = crate::provider_runtime::ProviderJob {
        provider_run_id: provider_run_id.to_owned(),
        workspace_id: workspace_id_text.clone(),
        analysis_context_id: context_id_text,
        source_generation,
        source_snapshot_lease: Some(crate::provider_runtime::ProviderSourceSnapshotLease {
            lease_id: request.source_snapshot_lease_id.clone(),
            workspace_id: workspace_id_text,
            source_generation,
            source_manifest_digest: request.source_manifest_digest.clone(),
            expires_at_unix_ms: request.deadline_unix_ms,
            blobs: lease_blobs,
        }),
        requested_capability_codes: request.requested_capability_codes.clone(),
        scopes: vec![scope.clone()],
        priority_class: 1,
        resource_estimate: Some(crate::provider_runtime::ProviderResourceEstimate {
            input_bytes,
            expected_output_bytes: 16 * 1024 * 1024,
            cpu_weight: 2,
            memory_mib: 1024,
        }),
        deadline_unix_ms: request.deadline_unix_ms,
        supersession_key: crate::provider_runtime::ProviderJob::semantic_supersession_key(
            &scope,
            "PYTHON_SEMANTIC",
        ),
        required_bundle_digests: vec![request.output_schema_bundle_digest.clone()],
        required_schema_digests: vec![crate::integrity::framed_digest(&view.manifest_digest)],
        idempotency_key: format!("pyrefly:{provider_run_id}"),
        resource_profile_id: "sidecar-semantic-standard".to_owned(),
        sandbox_profile_digest: crate::pyrefly_service::SANDBOX_PROFILE_DIGEST.to_owned(),
        direct_work: crate::provider_runtime::ProviderDirectWork::SemanticProcess(
            crate::provider_runtime::SemanticProviderWork {
                provider_id: "pyrefly-python".to_owned(),
                capability_family: "PYTHON_SEMANTIC".to_owned(),
                workspace_view: view,
                trust_profile: crate::provider_sandbox::ProviderTrustProfile::UntrustedSandboxed,
                invocation_manifest: PyreflyProviderDriver::invocation_manifest(&request)
                    .map_err(invariant)?,
            },
        ),
    };
    let mut accepted = dispatch
        .submit("pyrefly-python", job)
        .await
        .map_err(invariant)?;
    let terminal = loop {
        let event = accepted
            .events
            .recv()
            .await
            .ok_or_else(|| invariant("Pyrefly ProviderRuntime event stream closed"))?;
        match event {
            crate::provider_runtime::ProviderEvent::Completed { state, .. } => break Ok(state),
            crate::provider_runtime::ProviderEvent::Failed { state, code, .. } => {
                break Err(format!("{state:?}: {code}"));
            }
            _ => {}
        }
    };
    let terminal = terminal.map_err(|terminal| {
        let detail = driver
            .take_result(provider_run_id)
            .and_then(Result::err)
            .unwrap_or_else(|| "no lane diagnostic".to_owned());
        invariant(format!(
            "Pyrefly ProviderRuntime terminal was {terminal}; {detail}"
        ))
    })?;
    if terminal != crate::registries::ProviderRunState::Succeeded {
        return Err(invariant(format!(
            "Pyrefly ProviderRuntime terminal state was {terminal:?}"
        )));
    }
    driver
        .take_result(provider_run_id)
        .ok_or_else(|| invariant("Pyrefly ProviderRuntime result is absent"))?
        .map_err(invariant)
}

fn extractor_environment(
    command: &mut Command,
    socket: &Path,
    workspace_id: &str,
    context_id: &str,
    source_generation: u64,
) {
    let fixed_digest = digest(b"gate-b-rustc-fixture");
    command
        .env(
            "CODEFABRIC_EXTRACTOR_ENDPOINT",
            format!("unix://{}", socket.display()),
        )
        .env("CODEFABRIC_PROVIDER_RUN_ID", "run:gate-b-rustc")
        .env("CODEFABRIC_WORKSPACE_ID", workspace_id)
        .env("CODEFABRIC_ANALYSIS_CONTEXT_ID", context_id)
        .env(
            "CODEFABRIC_SOURCE_GENERATION",
            source_generation.to_string(),
        )
        .env("CODEFABRIC_CONTEXT_MANIFEST_DIGEST", &fixed_digest)
        .env(
            "CODEFABRIC_PROVIDER_RESOURCE_PROFILE_ID",
            "compiler-semantic-standard",
        )
        .env("CODEFABRIC_SOURCE_SNAPSHOT_MANIFEST_DIGEST", &fixed_digest)
        .env("CODEFABRIC_CARGO_METADATA_DIGEST", &fixed_digest)
        .env("CODEFABRIC_CARGO_LOCK_DIGEST", &fixed_digest)
        .env("CODEFABRIC_CARGO_CONFIG_DIGEST", &fixed_digest);
}

/// Launch the pinned compiler extractor and return its admitted application DTOs.
#[allow(clippy::too_many_lines)] // One compiler transaction keeps invocation, stream, and terminal validation ordered.
async fn run_rustc(
    repository_root: &Path,
    state_root: &Path,
    source: &ProviderSourceBlob,
    workspace_id: [u8; 16],
    analysis_context_id: [u8; 16],
    source_generation: u64,
) -> Result<AcceptedRustcCompilation, ProviderFixtureError> {
    provider_binary(
        repository_root,
        "target/extractor/debug/codefabric-rustc-extractor",
    )?;
    let identity_bytes = fs::read(repository_root.join("rustc-extractor/toolchain-identity.json"))?;
    let identity: Value = serde_json::from_slice(&identity_bytes)?;
    let workspace_id_text =
        encode_public_id(IdentityDomain::Workspace, None, workspace_id).map_err(invariant)?;
    let context_id_text =
        encode_public_id(IdentityDomain::AnalysisContext, None, analysis_context_id)
            .map_err(invariant)?;
    let fixed_digest = digest(b"gate-b-rustc-fixture");
    let policy = RustcProtocolPolicy {
        daemon_build: "codefabricd-gate-b".to_owned(),
        output_schema_bundle_digest: digest(include_bytes!(
            "../../contracts/schema/schema-contract-ir.json"
        )),
        sandbox_profile_digest: fixed_digest.clone(),
        extractor_build: identity["extractor"]
            .as_str()
            .ok_or_else(|| invariant("extractor identity is absent"))?
            .to_owned(),
        rustc_version: identity["rustc_release"]
            .as_str()
            .ok_or_else(|| invariant("rustc release is absent"))?
            .to_owned(),
        rustc_commit: identity["rustc_commit_hash"]
            .as_str()
            .ok_or_else(|| invariant("rustc commit is absent"))?
            .to_owned(),
        toolchain_identity_digest: digest(&identity_bytes),
        supported_feature_bits: 0,
        provider_deadline_unix_ms: now_millis() + 120_000,
    };
    let admission = RustcRunAdmission {
        provider_run_id: "run:gate-b-rustc".to_owned(),
        workspace_id: workspace_id_text.clone(),
        analysis_context_id: context_id_text.clone(),
        canonical_workspace_id: workspace_id,
        canonical_analysis_context_id: analysis_context_id,
        source_generation,
        context_manifest_digest: fixed_digest.clone(),
        source_snapshot_manifest_digest: fixed_digest,
        resource_profile_id: "compiler-semantic-standard".to_owned(),
    };
    let (service, mut accepted) =
        RustcObservationService::new(policy, admission).map_err(invariant)?;
    let socket = state_root.join("rustc.sock");
    let allowed_uid = fs::metadata(state_root)?.uid();
    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let server_socket = socket.clone();
    let server = tokio::spawn(async move {
        serve_rustc_uds(&server_socket, allowed_uid, service, async {
            let _ = shutdown_receiver.await;
        })
        .await
    });
    wait_for_socket(&socket).await?;
    let wrapper = repository_root.join("scripts/run_rustc_extractor.sh");
    if !source.path.is_file() {
        return Err(invariant(
            "Rust provider fixture has no immutable source blob",
        ));
    }
    let source = source.path.clone();
    let output_directory = state_root.join("rust-output");
    fs::create_dir(&output_directory)?;
    let status = tokio::task::spawn_blocking(move || {
        let rustc = Command::new("rustup")
            .args(["which", "--toolchain", "nightly-2026-08-18", "rustc"])
            .output()?;
        if !rustc.status.success() {
            return Err(std::io::Error::other(format!(
                "rustup could not resolve the pinned rustc: {}",
                String::from_utf8_lossy(&rustc.stderr)
            )));
        }
        let rustc = PathBuf::from(String::from_utf8_lossy(&rustc.stdout).trim());
        let sysroot_library = rustc
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| std::io::Error::other("pinned rustc has no sysroot parent"))?
            .join("lib");
        let mut command = Command::new(wrapper);
        command
            .arg(rustc)
            .arg(source)
            .args([
                "--crate-name=codefabric_gate_b_rust",
                "--crate-type=lib",
                "--edition=2024",
                "--emit=metadata",
            ])
            .arg(format!("--out-dir={}", output_directory.display()))
            .env("DYLD_LIBRARY_PATH", &sysroot_library)
            .env("LD_LIBRARY_PATH", &sysroot_library)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        extractor_environment(
            &mut command,
            &socket,
            &workspace_id_text,
            &context_id_text,
            source_generation,
        );
        command.output()
    })
    .await
    .map_err(invariant)??;
    if !status.status.success() {
        let _ = shutdown_sender.send(());
        let _ = server.await;
        return Err(invariant(format!(
            "real rustc extractor compilation failed: {}",
            String::from_utf8_lossy(&status.stderr)
        )));
    }
    let compilation = tokio::time::timeout(Duration::from_secs(30), accepted.recv())
        .await
        .map_err(|_| invariant("rustc accepted stream timed out"))?
        .ok_or_else(|| invariant("rustc service closed without an accepted compilation"))?;
    let _ = shutdown_sender.send(());
    server.await.map_err(invariant)?.map_err(invariant)?;
    Ok(compilation)
}
