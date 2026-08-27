//! Neutral real-provider fixture launcher shared by integration proofs.
//!
//! This module owns process, admission, and UDS mechanics for provider-backed fixtures. Review
//! candidate code consumes the resulting application-owned DTOs; it does not own provider launch
//! policy.

use std::fs;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use thiserror::Error;

use crate::identity::{IdentityDomain, encode_public_id};
use crate::pyrefly_service::{
    AcceptedPyreflyRun, PyreflyModuleInput, PyreflyRunRequest, analyze_pyrefly_uds,
};
use crate::rustc_service::{
    AcceptedRustcCompilation, RustcObservationService, RustcProtocolPolicy, RustcRunAdmission,
    serve_rustc_uds,
};

/// Immutable source-image reference admitted to a real provider fixture run.
#[derive(Clone, Debug)]
pub(crate) struct ProviderSourceBlob {
    pub path: PathBuf,
    pub content_digest: String,
    pub file_id: [u8; 16],
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
        run_pyrefly(
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

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
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

/// Launch the pinned Pyrefly sidecar and return its admitted application DTOs.
#[allow(clippy::too_many_arguments)] // The fixture makes every admission identity and source role explicit at the call site.
async fn run_pyrefly(
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
    let socket = state_root.join("pyrefly.sock");
    let child = Command::new(binary)
        .arg("--serve")
        .env(
            "CODEFABRIC_PYREFLY_ENDPOINT",
            format!("unix://{}", socket.display()),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let _guard = ChildGuard(child);
    wait_for_socket(&socket).await?;
    let workspace_id_text =
        encode_public_id(IdentityDomain::Workspace, None, workspace_id).map_err(invariant)?;
    let context_id_text =
        encode_public_id(IdentityDomain::AnalysisContext, None, analysis_context_id)
            .map_err(invariant)?;
    let context_manifest = b"{\"language\":\"python\",\"profile\":\"PYTHON_SEMANTIC_V1\"}".to_vec();
    let mut manifest_digests = vec![source.content_digest.as_str()];
    if let Some(ffi_source) = ffi_source {
        manifest_digests.push(ffi_source.content_digest.as_str());
    }
    manifest_digests.push(invalid_source.content_digest.as_str());
    let source_manifest_digest = digest(manifest_digests.join(":").as_bytes());
    let mut modules = vec![PyreflyModuleInput {
        module_id: "module:gate-b-python".to_owned(),
        module_name: "golden_pkg.core".to_owned(),
        file_id: encode_public_id(IdentityDomain::SourceFile, None, source.file_id)
            .map_err(invariant)?,
        source_blob_path: source.path.clone(),
        content_digest: source.content_digest.clone(),
    }];
    if let Some(ffi_source) = ffi_source {
        modules.push(PyreflyModuleInput {
            module_id: "module:gate-b-ffi".to_owned(),
            module_name: "ffi.boundary".to_owned(),
            file_id: encode_public_id(IdentityDomain::SourceFile, None, ffi_source.file_id)
                .map_err(invariant)?,
            source_blob_path: ffi_source.path.clone(),
            content_digest: ffi_source.content_digest.clone(),
        });
    }
    modules.push(PyreflyModuleInput {
        module_id: "module:gate-b-python-invalid".to_owned(),
        module_name: "malformed.broken".to_owned(),
        file_id: encode_public_id(IdentityDomain::SourceFile, None, invalid_source.file_id)
            .map_err(invariant)?,
        source_blob_path: invalid_source.path.clone(),
        content_digest: invalid_source.content_digest.clone(),
    });
    let request = PyreflyRunRequest {
        provider_run_id: "run:gate-b-pyrefly".to_owned(),
        workspace_id: workspace_id_text,
        analysis_context_id: context_id_text,
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
    analyze_pyrefly_uds(&socket, &request)
        .await
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
