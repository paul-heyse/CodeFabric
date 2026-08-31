//! Policy-bearing trust boundary for Rust semantic compilation.
//!
//! This module deliberately owns no `rustc_public` or `rustc_private` values. It compiles an
//! application-owned trust policy into the closed inputs consumed by [`ProviderSandboxLauncher`]
//! and records what the launcher must prove. Build scripts and procedural macros are therefore
//! part of the launcher's untrusted-code boundary, not an incidental Cargo implementation detail.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::fs::File;
use std::io::{Read as _, Write as _};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::provider_sandbox::{
    GeneratedSandboxProfile, ProviderLaunchRequest, ProviderProcessGroupChild,
    ProviderProcessLimits, ProviderSandboxLaunchMaterial, ProviderSandboxLauncher,
    ProviderTrustProfile, SandboxCapabilityMatrix, SandboxError, SandboxMechanism,
};

/// Current wire/model version for the Rust compilation trust policy.
pub const RUST_COMPILATION_TRUST_POLICY_VERSION: u32 = 1;
/// Exact launcher implementation named by plans and receipts produced by this module.
pub const RUST_COMPILATION_TRUST_LAUNCHER_ID: &str =
    "codefabric-rust-compilation-trust-launcher-v1";

const MAX_WALL_TIME_MILLIS: u64 = 60 * 60 * 1_000;
const MAX_CAPTURE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ARTIFACT_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const MAX_PROCESS_COUNT: u32 = 512;
const MAX_FILE_COUNT: u64 = 1_000_000;
const MAX_SINGLE_FILE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_CPU_SECONDS: u64 = 60 * 60 * 64;
const MAX_MEMORY_BYTES: u64 = 64 * 1024 * 1024 * 1024;
const MAX_OPEN_FILES: u64 = 4_096;

/// Trust mode is explicit input. Failure to establish untrusted containment never selects local
/// execution.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RustCompilationTrustMode {
    UntrustedSandboxed,
    TrustedLocal,
}

impl RustCompilationTrustMode {
    const fn provider_profile(self) -> ProviderTrustProfile {
        match self {
            Self::UntrustedSandboxed => ProviderTrustProfile::UntrustedSandboxed,
            Self::TrustedLocal => ProviderTrustProfile::TrustedLocal,
        }
    }
}

/// Build scripts and procedural macros either run through the selected launcher or cause a
/// typed rejection. There is no implicit host execution option.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RustExecutableExtensionPolicy {
    ExecuteInsideSelectedLauncher,
    RejectWorkspaceWhenPresent,
}

/// Exact resource contract monitored across the complete compiler process group.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RustCompilationResourceLimits {
    pub wall_time_millis: u64,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub artifact_bytes: u64,
    pub process_count: u32,
    pub file_count: u64,
    pub single_file_bytes: u64,
    pub cpu_seconds: u64,
    pub memory_bytes: u64,
    pub open_files: u64,
}

impl RustCompilationResourceLimits {
    /// Validate that every resource has a finite, enforceable limit.
    ///
    /// # Errors
    ///
    /// Rejects zero or policy-ceiling-exceeding limits and impossible cross-field combinations.
    pub fn validate(self) -> Result<(), RustCompilationTrustError> {
        let finite = self.wall_time_millis > 0
            && self.stdout_bytes > 0
            && self.stderr_bytes > 0
            && self.artifact_bytes > 0
            && self.process_count > 0
            && self.file_count > 0
            && self.single_file_bytes > 0
            && self.cpu_seconds > 0
            && self.memory_bytes > 0
            && self.open_files > 2;
        let within_ceiling = self.wall_time_millis <= MAX_WALL_TIME_MILLIS
            && self.stdout_bytes <= MAX_CAPTURE_BYTES
            && self.stderr_bytes <= MAX_CAPTURE_BYTES
            && self.artifact_bytes <= MAX_ARTIFACT_BYTES
            && self.process_count <= MAX_PROCESS_COUNT
            && self.file_count <= MAX_FILE_COUNT
            && self.single_file_bytes <= MAX_SINGLE_FILE_BYTES
            && self.cpu_seconds <= MAX_CPU_SECONDS
            && self.memory_bytes <= MAX_MEMORY_BYTES
            && self.open_files <= MAX_OPEN_FILES;
        if !finite || !within_ceiling {
            return Err(RustCompilationTrustError::InvalidResourceLimits);
        }
        if self.single_file_bytes > self.artifact_bytes
            || u64::from(self.process_count) >= self.open_files
        {
            return Err(RustCompilationTrustError::InvalidResourceLimits);
        }
        Ok(())
    }

    /// Subset enforced directly by the shared provider launcher. The remaining bounds are owned
    /// by the Rust compilation supervisor and appear independently in receipts.
    #[must_use]
    pub const fn provider_process_limits(self) -> ProviderProcessLimits {
        ProviderProcessLimits {
            cpu_seconds: self.cpu_seconds,
            open_files: self.open_files,
            address_space_bytes: self.memory_bytes,
            output_file_bytes: self.single_file_bytes,
        }
    }
}

/// Versioned closed policy. Enum-valued fields make security-relevant behavior explicit in the
/// policy digest instead of inferring it from a program path or host.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RustCompilationTrustPolicy {
    pub policy_version: u32,
    pub launcher_id: String,
    pub trust_mode: RustCompilationTrustMode,
    pub executable_extensions: RustExecutableExtensionPolicy,
    pub limits: RustCompilationResourceLimits,
    pub clear_inherited_environment: bool,
    pub close_inherited_descriptors: bool,
    pub immutable_input_views: bool,
    pub private_output_view: bool,
    pub offline_dependency_resolution: bool,
    pub isolate_process_group: bool,
    pub terminate_group_on_cancel: bool,
    pub terminate_group_on_timeout: bool,
    pub terminate_group_on_limit: bool,
    pub termination_grace_millis: u64,
}

impl RustCompilationTrustPolicy {
    /// Closed v1 policy for untrusted workspaces.
    #[must_use]
    pub fn untrusted_sandboxed_v1(
        limits: RustCompilationResourceLimits,
        executable_extensions: RustExecutableExtensionPolicy,
    ) -> Self {
        Self::v1(
            RustCompilationTrustMode::UntrustedSandboxed,
            limits,
            executable_extensions,
        )
    }

    /// Explicitly degraded v1 local policy. Compiling a plan for this mode still requires a
    /// separate authorization bound to the policy, workspace, source snapshot, and toolchain.
    #[must_use]
    pub fn trusted_local_v1(
        limits: RustCompilationResourceLimits,
        executable_extensions: RustExecutableExtensionPolicy,
    ) -> Self {
        Self::v1(
            RustCompilationTrustMode::TrustedLocal,
            limits,
            executable_extensions,
        )
    }

    fn v1(
        trust_mode: RustCompilationTrustMode,
        limits: RustCompilationResourceLimits,
        executable_extensions: RustExecutableExtensionPolicy,
    ) -> Self {
        Self {
            policy_version: RUST_COMPILATION_TRUST_POLICY_VERSION,
            launcher_id: RUST_COMPILATION_TRUST_LAUNCHER_ID.into(),
            trust_mode,
            executable_extensions,
            limits,
            clear_inherited_environment: true,
            close_inherited_descriptors: true,
            immutable_input_views: true,
            private_output_view: true,
            offline_dependency_resolution: true,
            isolate_process_group: true,
            terminate_group_on_cancel: true,
            terminate_group_on_timeout: true,
            terminate_group_on_limit: true,
            termination_grace_millis: 2_000,
        }
    }

    /// Validate policy closure and return its canonical digest.
    ///
    /// # Errors
    ///
    /// Rejects unknown versions, partially disabled closure, or invalid resource limits.
    pub fn digest(&self) -> Result<String, RustCompilationTrustError> {
        self.validate()?;
        canonical_digest(self)
    }

    fn validate(&self) -> Result<(), RustCompilationTrustError> {
        self.limits.validate()?;
        if self.policy_version != RUST_COMPILATION_TRUST_POLICY_VERSION
            || self.launcher_id != RUST_COMPILATION_TRUST_LAUNCHER_ID
            || !self.clear_inherited_environment
            || !self.close_inherited_descriptors
            || !self.immutable_input_views
            || !self.private_output_view
            || !self.offline_dependency_resolution
            || !self.isolate_process_group
            || !self.terminate_group_on_cancel
            || !self.terminate_group_on_timeout
            || !self.terminate_group_on_limit
            || self.termination_grace_millis == 0
            || self.termination_grace_millis > 30_000
        {
            return Err(RustCompilationTrustError::OpenPolicy);
        }
        Ok(())
    }
}

/// File identity retained with a canonical input path to detect path replacement before launch.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RustCompilationPathIdentity {
    pub device: u64,
    pub inode: u64,
}

/// Immutable source/dependency inputs and exact executables made visible to the child.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RustCompilationInputs {
    pub workspace_view: PathBuf,
    pub workspace_identity: RustCompilationPathIdentity,
    pub dependency_view: PathBuf,
    pub dependency_identity: RustCompilationPathIdentity,
    pub cargo_executable: PathBuf,
    pub cargo_identity: RustCompilationPathIdentity,
    pub cargo_content_digest: String,
    pub rustc_executable: PathBuf,
    pub rustc_identity: RustCompilationPathIdentity,
    pub rustc_content_digest: String,
    pub extractor_wrapper: PathBuf,
    pub extractor_wrapper_identity: RustCompilationPathIdentity,
    pub extractor_wrapper_content_digest: String,
    pub source_snapshot_digest: String,
    pub dependency_snapshot_digest: String,
    pub toolchain_digest: String,
    pub exact_toolchain_release: String,
}

impl RustCompilationInputs {
    /// Inspect canonical roots and exact toolchain files without importing compiler API types.
    ///
    /// # Errors
    ///
    /// Rejects symlink aliases, overlapping roots, executable escape, malformed digests, and
    /// non-files.
    #[allow(clippy::too_many_arguments)]
    pub fn inspect(
        workspace_view: &Path,
        dependency_view: &Path,
        cargo_executable: &Path,
        rustc_executable: &Path,
        extractor_wrapper: &Path,
        source_snapshot_digest: &str,
        dependency_snapshot_digest: &str,
        toolchain_digest: &str,
        exact_toolchain_release: &str,
    ) -> Result<Self, RustCompilationTrustError> {
        let workspace_view = canonical_unaliased_directory(workspace_view)?;
        let dependency_view = canonical_unaliased_directory(dependency_view)?;
        if roots_overlap(&workspace_view, &dependency_view) {
            return Err(RustCompilationTrustError::InputRootsOverlap);
        }
        let cargo_executable = canonical_unaliased_file(cargo_executable)?;
        let rustc_executable = canonical_unaliased_file(rustc_executable)?;
        let extractor_wrapper = canonical_unaliased_file(extractor_wrapper)?;
        for executable in [&cargo_executable, &rustc_executable, &extractor_wrapper] {
            if !executable.starts_with(&dependency_view) {
                return Err(RustCompilationTrustError::ExecutableEscapesDependencies);
            }
        }
        for digest in [
            source_snapshot_digest,
            dependency_snapshot_digest,
            toolchain_digest,
        ] {
            validate_digest(digest)?;
        }
        validate_identifier(exact_toolchain_release, "toolchain release")?;
        Ok(Self {
            workspace_identity: path_identity(&workspace_view)?,
            dependency_identity: path_identity(&dependency_view)?,
            workspace_view,
            dependency_view,
            cargo_identity: path_identity(&cargo_executable)?,
            cargo_content_digest: sha256_file(&cargo_executable)?,
            cargo_executable,
            rustc_identity: path_identity(&rustc_executable)?,
            rustc_content_digest: sha256_file(&rustc_executable)?,
            rustc_executable,
            extractor_wrapper_identity: path_identity(&extractor_wrapper)?,
            extractor_wrapper_content_digest: sha256_file(&extractor_wrapper)?,
            extractor_wrapper,
            source_snapshot_digest: source_snapshot_digest.into(),
            dependency_snapshot_digest: dependency_snapshot_digest.into(),
            toolchain_digest: toolchain_digest.into(),
            exact_toolchain_release: exact_toolchain_release.into(),
        })
    }

    fn revalidate(&self) -> Result<(), RustCompilationTrustError> {
        if canonical_unaliased_directory(&self.workspace_view)? != self.workspace_view
            || canonical_unaliased_directory(&self.dependency_view)? != self.dependency_view
            || path_identity(&self.workspace_view)? != self.workspace_identity
            || path_identity(&self.dependency_view)? != self.dependency_identity
        {
            return Err(RustCompilationTrustError::InputIdentityChanged);
        }
        for (executable, identity, content_digest) in [
            (
                &self.cargo_executable,
                self.cargo_identity,
                &self.cargo_content_digest,
            ),
            (
                &self.rustc_executable,
                self.rustc_identity,
                &self.rustc_content_digest,
            ),
            (
                &self.extractor_wrapper,
                self.extractor_wrapper_identity,
                &self.extractor_wrapper_content_digest,
            ),
        ] {
            if canonical_unaliased_file(executable)? != *executable
                || !executable.starts_with(&self.dependency_view)
                || path_identity(executable)? != identity
                || sha256_file(executable)? != *content_digest
            {
                return Err(RustCompilationTrustError::InputIdentityChanged);
            }
        }
        Ok(())
    }
}

/// Daemon-owned private output tree. Every mutable compiler path is a direct descendant of the
/// per-run root; the source and dependency roots never double as output locations.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RustCompilationPrivatePaths {
    pub run_root: PathBuf,
    pub run_identity: RustCompilationPathIdentity,
    pub target_root: PathBuf,
    pub target_identity: RustCompilationPathIdentity,
    pub artifact_root: PathBuf,
    pub artifact_identity: RustCompilationPathIdentity,
    pub temporary_root: PathBuf,
    pub temporary_identity: RustCompilationPathIdentity,
    pub home_root: PathBuf,
    pub home_identity: RustCompilationPathIdentity,
    pub control_root: PathBuf,
    pub control_identity: RustCompilationPathIdentity,
    pub stdout_path: PathBuf,
    pub stderr_path: PathBuf,
    pub extractor_socket_path: PathBuf,
}

impl RustCompilationPrivatePaths {
    /// Create a non-reused `0700` output tree under a daemon-owned parent.
    ///
    /// # Errors
    ///
    /// Rejects unsafe run identifiers, symlink/relative parents, and pre-existing run roots.
    pub fn prepare(parent: &Path, run_id: &str) -> Result<Self, RustCompilationTrustError> {
        use rustix::fs::{Mode, OFlags, mkdirat, open, openat};

        validate_identifier(run_id, "run id")?;
        let parent = canonical_unaliased_directory(parent)?;
        validate_private_directory(&parent)?;
        let parent_descriptor = open(
            &parent,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY,
            Mode::empty(),
        )?;
        let run_name = format!("rust-compilation-{run_id}");
        mkdirat(&parent_descriptor, &run_name, Mode::RWXU)?;
        let run_descriptor = openat(
            &parent_descriptor,
            &run_name,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY,
            Mode::empty(),
        )?;
        for child in ["target", "artifacts", "tmp", "home", "control"] {
            mkdirat(&run_descriptor, child, Mode::RWXU)?;
        }
        let run_root = parent.join(run_name);
        Self::inspect(&run_root)
    }

    /// Validate an existing private output tree and derive every egress path from its root.
    ///
    /// # Errors
    ///
    /// Rejects missing, aliased, non-private, or unexpected directory identities.
    pub fn inspect(run_root: &Path) -> Result<Self, RustCompilationTrustError> {
        let run_root = canonical_unaliased_directory(run_root)?;
        validate_private_directory(&run_root)?;
        let target_root = private_child(&run_root, "target")?;
        let artifact_root = private_child(&run_root, "artifacts")?;
        let temporary_root = private_child(&run_root, "tmp")?;
        let home_root = private_child(&run_root, "home")?;
        let control_root = private_child(&run_root, "control")?;
        Ok(Self {
            run_identity: path_identity(&run_root)?,
            target_identity: path_identity(&target_root)?,
            artifact_identity: path_identity(&artifact_root)?,
            temporary_identity: path_identity(&temporary_root)?,
            home_identity: path_identity(&home_root)?,
            control_identity: path_identity(&control_root)?,
            stdout_path: run_root.join("stdout.capture"),
            stderr_path: run_root.join("stderr.capture"),
            extractor_socket_path: control_root.join("extractor.sock"),
            run_root,
            target_root,
            artifact_root,
            temporary_root,
            home_root,
            control_root,
        })
    }

    fn revalidate(&self) -> Result<(), RustCompilationTrustError> {
        if Self::inspect(&self.run_root)? != *self {
            return Err(RustCompilationTrustError::PrivatePathChanged);
        }
        Ok(())
    }
}

/// Exact context consumed by the current rustc extractor wrapper. Values are application-owned
/// pins and identifiers; they do not expose compiler-private types.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RustCompilationContextPins {
    pub provider_run_id: String,
    pub workspace_id: String,
    pub analysis_context_id: String,
    pub source_generation: u64,
    pub context_manifest_digest: String,
    pub resource_profile_id: String,
    pub source_snapshot_manifest_digest: String,
    pub cargo_metadata_digest: String,
    pub cargo_lock_digest: String,
    pub cargo_config_digest: String,
}

impl RustCompilationContextPins {
    fn validate(&self) -> Result<(), RustCompilationTrustError> {
        for (value, field) in [
            (&self.provider_run_id, "provider run id"),
            (&self.workspace_id, "workspace id"),
            (&self.analysis_context_id, "analysis context id"),
            (&self.resource_profile_id, "resource profile id"),
        ] {
            validate_identifier(value, field)?;
        }
        for digest in [
            &self.context_manifest_digest,
            &self.source_snapshot_manifest_digest,
            &self.cargo_metadata_digest,
            &self.cargo_lock_digest,
            &self.cargo_config_digest,
        ] {
            validate_digest(digest)?;
        }
        Ok(())
    }
}

/// Typed Cargo selection. No free-form compiler/Cargo flags cross this trust boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RustCompilationRunRequest {
    pub manifest_relative_path: PathBuf,
    pub package_names: Vec<String>,
    pub feature_names: Vec<String>,
    pub all_targets: bool,
    pub build_scripts_present: bool,
    pub procedural_macros_present: bool,
    pub context: RustCompilationContextPins,
}

impl RustCompilationRunRequest {
    fn validate(&self, workspace_view: &Path) -> Result<PathBuf, RustCompilationTrustError> {
        self.context.validate()?;
        validate_relative_path(&self.manifest_relative_path)?;
        if self
            .manifest_relative_path
            .file_name()
            .and_then(|value| value.to_str())
            != Some("Cargo.toml")
        {
            return Err(RustCompilationTrustError::InvalidManifestPath);
        }
        let manifest = workspace_view.join(&self.manifest_relative_path);
        let manifest = canonical_unaliased_file(&manifest)?;
        if !manifest.starts_with(workspace_view) {
            return Err(RustCompilationTrustError::PathEscape);
        }
        if self.package_names.len() > 256 || self.feature_names.len() > 1_024 {
            return Err(RustCompilationTrustError::InvalidInvocationToken);
        }
        let mut packages = BTreeSet::new();
        for package in &self.package_names {
            validate_cargo_token(package)?;
            if !packages.insert(package) {
                return Err(RustCompilationTrustError::DuplicateInvocationToken);
            }
        }
        let mut features = BTreeSet::new();
        for feature in &self.feature_names {
            validate_cargo_token(feature)?;
            if !features.insert(feature) {
                return Err(RustCompilationTrustError::DuplicateInvocationToken);
            }
        }
        Ok(manifest)
    }
}

/// Separately issued authorization for the degraded trusted-local profile.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TrustedLocalAuthorization {
    pub authorization_id: String,
    pub workspace_id: String,
    pub provider_run_id: String,
    pub policy_digest: String,
    pub source_snapshot_digest: String,
    pub toolchain_digest: String,
}

impl TrustedLocalAuthorization {
    fn validate(
        &self,
        policy_digest: &str,
        inputs: &RustCompilationInputs,
        request: &RustCompilationRunRequest,
    ) -> Result<(), RustCompilationTrustError> {
        validate_identifier(&self.authorization_id, "authorization id")?;
        if self.workspace_id != request.context.workspace_id
            || self.provider_run_id != request.context.provider_run_id
            || self.policy_digest != policy_digest
            || self.source_snapshot_digest != inputs.source_snapshot_digest
            || self.toolchain_digest != inputs.toolchain_digest
        {
            return Err(RustCompilationTrustError::TrustedLocalAuthorizationMismatch);
        }
        Ok(())
    }
}

/// Observed trust state projected into capability and provenance relations.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RustCompilationTrustState {
    ProvedUntrustedContainment,
    DegradedTrustedLocal,
    Unavailable,
}

/// Exact platform/probe row selected by the policy compiler.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RustCompilationPlatformBinding {
    pub trust_mode: RustCompilationTrustMode,
    pub trust_state: RustCompilationTrustState,
    pub sandbox_mechanism: SandboxMechanism,
    pub probe_digest: String,
    pub capability_reason_code: String,
}

/// Closed environment installed after `env_clear`. The map must equal the launch plan exactly;
/// callers cannot append even apparently harmless inherited values.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RustCompilationEnvironment {
    pub variables: BTreeMap<String, String>,
    pub environment_digest: String,
}

impl RustCompilationEnvironment {
    fn build(
        inputs: &RustCompilationInputs,
        paths: &RustCompilationPrivatePaths,
        request: &RustCompilationRunRequest,
        mechanism: SandboxMechanism,
    ) -> Result<Self, RustCompilationTrustError> {
        let layout = ContainedPathLayout::new(inputs, paths, mechanism)?;
        let mut variables = BTreeMap::from([
            (
                "PATH".into(),
                format!("{}:/usr/bin:/bin", layout.toolchain_bin.display()),
            ),
            ("HOME".into(), layout.home_root.display().to_string()),
            ("TMPDIR".into(), layout.temporary_root.display().to_string()),
            ("CARGO_HOME".into(), layout.cargo_home.display().to_string()),
            (
                "RUSTUP_HOME".into(),
                layout.rustup_home.display().to_string(),
            ),
            (
                "CARGO_TARGET_DIR".into(),
                layout.target_root.display().to_string(),
            ),
            ("CARGO_NET_OFFLINE".into(), "true".into()),
            ("CARGO_INCREMENTAL".into(), "0".into()),
            ("CARGO_TERM_COLOR".into(), "never".into()),
            ("RUST_BACKTRACE".into(), "0".into()),
            (
                "RUSTC".into(),
                layout.rustc_executable.display().to_string(),
            ),
            (
                "RUSTC_WRAPPER".into(),
                layout.extractor_wrapper.display().to_string(),
            ),
            (
                "CODEFABRIC_EXTRACTOR_ENDPOINT".into(),
                format!("unix://{}", layout.extractor_socket_path.display()),
            ),
            (
                "CODEFABRIC_PROVIDER_RUN_ID".into(),
                request.context.provider_run_id.clone(),
            ),
            (
                "CODEFABRIC_WORKSPACE_ID".into(),
                request.context.workspace_id.clone(),
            ),
            (
                "CODEFABRIC_ANALYSIS_CONTEXT_ID".into(),
                request.context.analysis_context_id.clone(),
            ),
            (
                "CODEFABRIC_SOURCE_GENERATION".into(),
                request.context.source_generation.to_string(),
            ),
            (
                "CODEFABRIC_CONTEXT_MANIFEST_DIGEST".into(),
                request.context.context_manifest_digest.clone(),
            ),
            (
                "CODEFABRIC_PROVIDER_RESOURCE_PROFILE_ID".into(),
                request.context.resource_profile_id.clone(),
            ),
            (
                "CODEFABRIC_SOURCE_SNAPSHOT_MANIFEST_DIGEST".into(),
                request.context.source_snapshot_manifest_digest.clone(),
            ),
            (
                "CODEFABRIC_CARGO_METADATA_DIGEST".into(),
                request.context.cargo_metadata_digest.clone(),
            ),
            (
                "CODEFABRIC_CARGO_LOCK_DIGEST".into(),
                request.context.cargo_lock_digest.clone(),
            ),
            (
                "CODEFABRIC_CARGO_CONFIG_DIGEST".into(),
                request.context.cargo_config_digest.clone(),
            ),
        ]);
        // A fixed locale avoids diagnostic-dependent behavior without inheriting host locale
        // modules, proxies, credential stores, or agent sockets.
        variables.insert("LC_ALL".into(), "C".into());
        validate_environment_variables(&variables)?;
        let environment_digest = canonical_digest(&variables)?;
        Ok(Self {
            variables,
            environment_digest,
        })
    }

    /// Prove that a candidate environment is exactly this closed environment.
    ///
    /// # Errors
    ///
    /// Rejects credential/proxy/agent variables, unknown keys, changed values, or digest drift.
    pub fn validate_candidate(
        &self,
        candidate: &BTreeMap<String, String>,
    ) -> Result<(), RustCompilationTrustError> {
        validate_environment_variables(candidate)?;
        if candidate != &self.variables || canonical_digest(candidate)? != self.environment_digest {
            return Err(RustCompilationTrustError::EnvironmentNotClosed);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ContainedPathLayout {
    workspace_view: PathBuf,
    toolchain_bin: PathBuf,
    cargo_home: PathBuf,
    rustup_home: PathBuf,
    cargo_executable: PathBuf,
    rustc_executable: PathBuf,
    extractor_wrapper: PathBuf,
    target_root: PathBuf,
    temporary_root: PathBuf,
    home_root: PathBuf,
    extractor_socket_path: PathBuf,
}

impl ContainedPathLayout {
    fn new(
        inputs: &RustCompilationInputs,
        paths: &RustCompilationPrivatePaths,
        mechanism: SandboxMechanism,
    ) -> Result<Self, RustCompilationTrustError> {
        let dependency_relative = |path: &Path| {
            path.strip_prefix(&inputs.dependency_view)
                .map(Path::to_path_buf)
                .map_err(|_| RustCompilationTrustError::ExecutableEscapesDependencies)
        };
        let (workspace_view, dependency_view, output_root) = match mechanism {
            SandboxMechanism::LinuxBubblewrap => (
                PathBuf::from("/workspace"),
                PathBuf::from("/dependencies"),
                PathBuf::from("/output"),
            ),
            SandboxMechanism::DarwinSeatbelt | SandboxMechanism::None => (
                inputs.workspace_view.clone(),
                inputs.dependency_view.clone(),
                paths.run_root.clone(),
            ),
        };
        let translate_dependency = |path: &Path| {
            Ok::<_, RustCompilationTrustError>(dependency_view.join(dependency_relative(path)?))
        };
        Ok(Self {
            workspace_view,
            toolchain_bin: translate_dependency(&inputs.rustc_executable)?
                .parent()
                .ok_or(RustCompilationTrustError::ExecutableEscapesDependencies)?
                .to_path_buf(),
            cargo_home: dependency_view.join("cargo-home"),
            rustup_home: dependency_view.join("rustup-home"),
            cargo_executable: translate_dependency(&inputs.cargo_executable)?,
            rustc_executable: translate_dependency(&inputs.rustc_executable)?,
            extractor_wrapper: translate_dependency(&inputs.extractor_wrapper)?,
            target_root: output_root.join("target"),
            temporary_root: output_root.join("tmp"),
            home_root: output_root.join("home"),
            extractor_socket_path: output_root.join("control/extractor.sock"),
        })
    }
}

/// Cancellation is a process-group operation. The complete sequence is included in the plan
/// digest so an implementation cannot downgrade cancellation to `Child::kill` on one process.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RustCompilationCancellationContract {
    pub create_new_session: bool,
    pub process_group_is_run_scoped: bool,
    pub terminate_entire_group_first: bool,
    pub kill_entire_group_after_grace: bool,
    pub verify_group_empty: bool,
    pub termination_grace_millis: u64,
}

/// Immutable launch plan compiled from policy, platform proof, exact inputs, and run pins.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RustCompilationLaunchPlan {
    launcher_id: String,
    policy_version: u32,
    policy_digest: String,
    plan_digest: String,
    trust_mode: RustCompilationTrustMode,
    platform: RustCompilationPlatformBinding,
    trusted_local_authorization_id: Option<String>,
    sandbox_profile_digest: String,
    workspace_id: String,
    provider_run_id: String,
    source_generation: u64,
    source_snapshot_digest: String,
    dependency_snapshot_digest: String,
    toolchain_digest: String,
    exact_toolchain_release: String,
    path_contract_digest: String,
    host_workspace_view: PathBuf,
    host_workspace_identity: RustCompilationPathIdentity,
    host_dependency_view: PathBuf,
    host_dependency_identity: RustCompilationPathIdentity,
    host_cargo_executable: PathBuf,
    host_cargo_identity: RustCompilationPathIdentity,
    host_cargo_content_digest: String,
    host_rustc_executable: PathBuf,
    host_rustc_identity: RustCompilationPathIdentity,
    host_rustc_content_digest: String,
    host_extractor_wrapper: PathBuf,
    host_extractor_wrapper_identity: RustCompilationPathIdentity,
    host_extractor_wrapper_content_digest: String,
    contained_cargo_executable: PathBuf,
    contained_arguments: Vec<String>,
    output_root: PathBuf,
    environment: RustCompilationEnvironment,
    limits: RustCompilationResourceLimits,
    executable_extensions: RustExecutableExtensionPolicy,
    build_scripts_present: bool,
    procedural_macros_present: bool,
    cancellation: RustCompilationCancellationContract,
}

impl RustCompilationLaunchPlan {
    /// Digest that binds the complete opaque launch plan.
    #[must_use]
    pub fn plan_digest(&self) -> &str {
        &self.plan_digest
    }

    /// Provider-run identity used to join compiler protocol and launcher observations.
    #[must_use]
    pub fn provider_run_id(&self) -> &str {
        &self.provider_run_id
    }

    /// Explicit trust mode selected at policy compilation.
    #[must_use]
    pub const fn trust_mode(&self) -> RustCompilationTrustMode {
        self.trust_mode
    }

    /// Produce the request owned by the shared semantic-provider launcher.
    ///
    /// On Darwin and trusted-local profiles the contained executable is its canonical host path.
    /// Bubblewrap plans name the `/dependencies` mount path; the shared launcher must validate the
    /// corresponding host identity before entering the namespace.
    #[must_use]
    fn provider_launch_request(&self) -> ProviderLaunchRequest {
        ProviderLaunchRequest {
            host_executable: self.host_cargo_executable.clone(),
            contained_executable: self.contained_cargo_executable.clone(),
            arguments: self.contained_arguments.clone(),
            environment: self.environment.variables.clone(),
            output_root: self.output_root.clone(),
            limits: self.limits.provider_process_limits(),
        }
    }

    fn launch_captured(
        &self,
        launcher: &ProviderSandboxLauncher,
        sandbox_profile: &GeneratedSandboxProfile,
        material: ProviderSandboxLaunchMaterial<'_>,
    ) -> Result<ProviderProcessGroupChild, RustCompilationTrustError> {
        self.verify_digest()?;
        self.environment
            .validate_candidate(&self.environment.variables)?;
        if sandbox_profile.trust_profile != self.trust_mode.provider_profile()
            || sandbox_profile.mechanism != self.platform.sandbox_mechanism
            || sandbox_profile.output_root != self.output_root
            || sandbox_profile.sha256_digest != self.sandbox_profile_digest
            || sandbox_profile.sha256_digest != sha256_bytes(&sandbox_profile.bytes)
        {
            return Err(RustCompilationTrustError::SandboxProfileMismatch);
        }
        Ok(launcher.launch_captured(&self.provider_launch_request(), sandbox_profile, material)?)
    }

    /// Recompute the plan digest, detecting any mutation between admission and launch.
    ///
    /// # Errors
    ///
    /// Rejects any changed policy, path, environment, resource, command, or platform binding.
    pub fn verify_digest(&self) -> Result<(), RustCompilationTrustError> {
        let mut unsigned = self.clone();
        unsigned.plan_digest.clear();
        if canonical_digest(&unsigned)? != self.plan_digest {
            return Err(RustCompilationTrustError::LaunchPlanDigestMismatch);
        }
        self.environment
            .validate_candidate(&self.environment.variables)?;
        for (path, identity) in [
            (&self.host_workspace_view, self.host_workspace_identity),
            (&self.host_dependency_view, self.host_dependency_identity),
        ] {
            if canonical_unaliased_directory(path)? != *path || path_identity(path)? != identity {
                return Err(RustCompilationTrustError::InputIdentityChanged);
            }
        }
        for (path, identity, content_digest) in [
            (
                &self.host_cargo_executable,
                self.host_cargo_identity,
                &self.host_cargo_content_digest,
            ),
            (
                &self.host_rustc_executable,
                self.host_rustc_identity,
                &self.host_rustc_content_digest,
            ),
            (
                &self.host_extractor_wrapper,
                self.host_extractor_wrapper_identity,
                &self.host_extractor_wrapper_content_digest,
            ),
        ] {
            if canonical_unaliased_file(path)? != *path
                || path_identity(path)? != identity
                || sha256_file(path)? != *content_digest
            {
                return Err(RustCompilationTrustError::InputIdentityChanged);
            }
        }
        Ok(())
    }
}

/// Compile a fail-closed launcher plan. This function has no fallback branch: a missing probe,
/// changed path, open environment, or absent trusted-local grant is an error row, not host Cargo.
///
/// # Errors
///
/// Returns a typed closure/trust/path/authorization error before any process is spawned.
#[allow(clippy::too_many_arguments)]
pub fn compile_rust_compilation_launch_plan(
    policy: &RustCompilationTrustPolicy,
    capabilities: &SandboxCapabilityMatrix,
    sandbox_profile: &GeneratedSandboxProfile,
    inputs: &RustCompilationInputs,
    paths: &RustCompilationPrivatePaths,
    request: &RustCompilationRunRequest,
    trusted_local_authorization: Option<&TrustedLocalAuthorization>,
) -> Result<RustCompilationLaunchPlan, RustCompilationTrustError> {
    let policy_digest = policy.digest()?;
    inputs.revalidate()?;
    paths.revalidate()?;
    let manifest = request.validate(&inputs.workspace_view)?;
    if roots_overlap(&inputs.workspace_view, &paths.run_root)
        || roots_overlap(&inputs.dependency_view, &paths.run_root)
    {
        return Err(RustCompilationTrustError::OutputOverlapsInput);
    }
    if sandbox_profile.trust_profile != policy.trust_mode.provider_profile()
        || sandbox_profile.workspace_view != inputs.workspace_view
        || sandbox_profile.dependency_root != inputs.dependency_view
        || sandbox_profile.output_root != paths.run_root
        || sandbox_profile.sha256_digest != sha256_bytes(&sandbox_profile.bytes)
    {
        return Err(RustCompilationTrustError::SandboxProfileMismatch);
    }
    let capability = capabilities
        .row(policy.trust_mode.provider_profile())
        .ok_or(RustCompilationTrustError::ContainmentUnavailable)?;
    validate_digest(&capability.probe_digest)?;
    let (trust_state, authorization_id) = match policy.trust_mode {
        RustCompilationTrustMode::UntrustedSandboxed => {
            if trusted_local_authorization.is_some() {
                return Err(RustCompilationTrustError::UnexpectedTrustedLocalAuthorization);
            }
            if !capability.available
                || capability.mechanism == SandboxMechanism::None
                || capability.mechanism != sandbox_profile.mechanism
                || capability.reason_code != "SANDBOX_PROVED"
            {
                return Err(RustCompilationTrustError::ContainmentUnavailable);
            }
            (RustCompilationTrustState::ProvedUntrustedContainment, None)
        }
        RustCompilationTrustMode::TrustedLocal => {
            if !capability.available
                || capability.mechanism != SandboxMechanism::None
                || sandbox_profile.mechanism != SandboxMechanism::None
            {
                return Err(RustCompilationTrustError::TrustedLocalProfileMismatch);
            }
            let authorization = trusted_local_authorization
                .ok_or(RustCompilationTrustError::TrustedLocalAuthorizationRequired)?;
            authorization.validate(&policy_digest, inputs, request)?;
            (
                RustCompilationTrustState::DegradedTrustedLocal,
                Some(authorization.authorization_id.clone()),
            )
        }
    };
    if policy.executable_extensions == RustExecutableExtensionPolicy::RejectWorkspaceWhenPresent
        && (request.build_scripts_present || request.procedural_macros_present)
    {
        return Err(RustCompilationTrustError::ExecutableExtensionRejected);
    }

    let layout = ContainedPathLayout::new(inputs, paths, sandbox_profile.mechanism)?;
    let relative_manifest = manifest
        .strip_prefix(&inputs.workspace_view)
        .map_err(|_| RustCompilationTrustError::PathEscape)?;
    let contained_manifest = layout.workspace_view.join(relative_manifest);
    let mut package_names = request.package_names.clone();
    package_names.sort();
    let mut feature_names = request.feature_names.clone();
    feature_names.sort();
    let mut contained_arguments = vec![
        "check".into(),
        "--locked".into(),
        "--offline".into(),
        "--manifest-path".into(),
        contained_manifest.display().to_string(),
    ];
    for package in package_names {
        contained_arguments.extend(["--package".into(), package]);
    }
    if !feature_names.is_empty() {
        contained_arguments.extend(["--features".into(), feature_names.join(",")]);
    }
    if request.all_targets {
        contained_arguments.push("--all-targets".into());
    }
    let environment =
        RustCompilationEnvironment::build(inputs, paths, request, sandbox_profile.mechanism)?;
    let path_contract_digest = canonical_digest(&(inputs, paths))?;
    let mut plan = RustCompilationLaunchPlan {
        launcher_id: RUST_COMPILATION_TRUST_LAUNCHER_ID.into(),
        policy_version: policy.policy_version,
        policy_digest,
        plan_digest: String::new(),
        trust_mode: policy.trust_mode,
        platform: RustCompilationPlatformBinding {
            trust_mode: policy.trust_mode,
            trust_state,
            sandbox_mechanism: capability.mechanism,
            probe_digest: capability.probe_digest.clone(),
            capability_reason_code: capability.reason_code.clone(),
        },
        trusted_local_authorization_id: authorization_id,
        sandbox_profile_digest: sandbox_profile.sha256_digest.clone(),
        workspace_id: request.context.workspace_id.clone(),
        provider_run_id: request.context.provider_run_id.clone(),
        source_generation: request.context.source_generation,
        source_snapshot_digest: inputs.source_snapshot_digest.clone(),
        dependency_snapshot_digest: inputs.dependency_snapshot_digest.clone(),
        toolchain_digest: inputs.toolchain_digest.clone(),
        exact_toolchain_release: inputs.exact_toolchain_release.clone(),
        path_contract_digest,
        host_workspace_view: inputs.workspace_view.clone(),
        host_workspace_identity: inputs.workspace_identity,
        host_dependency_view: inputs.dependency_view.clone(),
        host_dependency_identity: inputs.dependency_identity,
        host_cargo_executable: inputs.cargo_executable.clone(),
        host_cargo_identity: inputs.cargo_identity,
        host_cargo_content_digest: inputs.cargo_content_digest.clone(),
        host_rustc_executable: inputs.rustc_executable.clone(),
        host_rustc_identity: inputs.rustc_identity,
        host_rustc_content_digest: inputs.rustc_content_digest.clone(),
        host_extractor_wrapper: inputs.extractor_wrapper.clone(),
        host_extractor_wrapper_identity: inputs.extractor_wrapper_identity,
        host_extractor_wrapper_content_digest: inputs.extractor_wrapper_content_digest.clone(),
        contained_cargo_executable: layout.cargo_executable,
        contained_arguments,
        output_root: paths.run_root.clone(),
        environment,
        limits: policy.limits,
        executable_extensions: policy.executable_extensions,
        build_scripts_present: request.build_scripts_present,
        procedural_macros_present: request.procedural_macros_present,
        cancellation: RustCompilationCancellationContract {
            create_new_session: true,
            process_group_is_run_scoped: true,
            terminate_entire_group_first: true,
            kill_entire_group_after_grace: true,
            verify_group_empty: true,
            termination_grace_millis: policy.termination_grace_millis,
        },
    };
    plan.plan_digest = canonical_digest(&plan)?;
    plan.verify_digest()?;
    Ok(plan)
}

/// Why the supervisor terminated a complete compiler process group.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RustCompilationCancellationReason {
    Requested,
    WallTimeExceeded,
    ResourceLimitExceeded,
    LeaderExitedWithDescendants,
}

/// Cloneable run-wide cancellation edge shared by the compiler protocol service and supervisor.
///
/// A Cargo invocation can contain many concurrent rustc compilation units. The protocol service
/// first sends cooperative cancellation to every active unit, then marks this signal so the sole
/// launcher escalates against the complete Cargo process group.
#[derive(Clone, Debug, Default)]
pub struct RustCompilationCancellationSignal {
    requested: Arc<AtomicBool>,
}

impl RustCompilationCancellationSignal {
    /// Request run-wide process-group cancellation. Repeated requests are idempotent.
    pub fn request(&self) {
        self.requested.store(true, Ordering::Release);
    }

    /// Whether cancellation has been requested for this run.
    #[must_use]
    pub fn is_requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }
}

/// Observable group actions. Receipt order is security-relevant.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RustCompilationProcessGroupAction {
    TerminateGroup,
    WaitGrace,
    KillGroup,
    VerifyGroupEmpty,
}

/// Adapter boundary for safe platform process-group control. Implementations must operate on the
/// run-scoped process group, never just the Cargo child PID.
pub trait RustCompilationProcessGroupControl {
    fn terminate_group(&mut self) -> std::io::Result<()>;
    fn wait_group_empty(&mut self, timeout: Duration) -> std::io::Result<bool>;
    fn kill_group(&mut self) -> std::io::Result<()>;
}

impl RustCompilationProcessGroupControl for ProviderProcessGroupChild {
    fn terminate_group(&mut self) -> std::io::Result<()> {
        ProviderProcessGroupChild::terminate_group(self)
    }

    fn wait_group_empty(&mut self, timeout: Duration) -> std::io::Result<bool> {
        ProviderProcessGroupChild::wait_group_empty(self, timeout)
    }

    fn kill_group(&mut self) -> std::io::Result<()> {
        ProviderProcessGroupChild::kill_group(self)
    }
}

/// Receipt proving the cancellation escalation sequence against one launch-plan digest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RustCompilationCancellationReceipt {
    pub launcher_id: String,
    pub plan_digest: String,
    pub reason: RustCompilationCancellationReason,
    pub actions: Vec<RustCompilationProcessGroupAction>,
    pub group_empty: bool,
    pub receipt_digest: String,
}

impl RustCompilationCancellationReceipt {
    fn verify_digest(&self) -> Result<(), RustCompilationTrustError> {
        let mut unsigned = self.clone();
        unsigned.receipt_digest.clear();
        if canonical_digest(&unsigned)? != self.receipt_digest {
            return Err(RustCompilationTrustError::CancellationReceiptMismatch);
        }
        Ok(())
    }
}

/// Terminate and verify the entire run-scoped process group.
///
/// # Errors
///
/// Rejects a plan-digest mismatch and fails when any signal/wait operation fails or descendants
/// survive escalation.
pub fn cancel_rust_compilation_process_group<C: RustCompilationProcessGroupControl>(
    plan: &RustCompilationLaunchPlan,
    admitted_plan_digest: &str,
    reason: RustCompilationCancellationReason,
    control: &mut C,
) -> Result<RustCompilationCancellationReceipt, RustCompilationTrustError> {
    plan.verify_digest()?;
    if admitted_plan_digest != plan.plan_digest {
        return Err(RustCompilationTrustError::CancellationBindingMismatch);
    }
    control.terminate_group()?;
    let mut actions = vec![RustCompilationProcessGroupAction::TerminateGroup];
    let grace = Duration::from_millis(plan.cancellation.termination_grace_millis);
    actions.push(RustCompilationProcessGroupAction::WaitGrace);
    if !control.wait_group_empty(grace)? {
        control.kill_group()?;
        actions.push(RustCompilationProcessGroupAction::KillGroup);
        if !control.wait_group_empty(grace)? {
            return Err(RustCompilationTrustError::ProcessGroupSurvived);
        }
    }
    actions.push(RustCompilationProcessGroupAction::VerifyGroupEmpty);
    let mut receipt = RustCompilationCancellationReceipt {
        launcher_id: plan.launcher_id.clone(),
        plan_digest: plan.plan_digest.clone(),
        reason,
        actions,
        group_empty: true,
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = canonical_digest(&receipt)?;
    Ok(receipt)
}

/// Resource whose attempted use caused supervised termination.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RustCompilationLimitKind {
    WallTime,
    StdoutBytes,
    StderrBytes,
    ArtifactBytes,
    ProcessCount,
    FileCount,
    SingleFileBytes,
    CpuTime,
    Memory,
    OpenFiles,
}

/// Terminal state observed by the trust launcher, distinct from semantic provider capability.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RustCompilationTerminalState {
    Succeeded,
    CompilerFailed,
    Cancelled,
    TimedOut,
    ResourceLimit,
}

/// Strength of the operating-system accounting attached to one terminal observation.
///
/// A sampled observation is useful for explicitly degraded `TRUSTED_LOCAL` execution, but cannot
/// establish the complete process-group budget required for untrusted workspaces.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RustCompilationAccountingQuality {
    KernelComplete,
    SampledDegraded,
}

/// Bounded observed usage. Captured/output values may equal but never exceed the admitted limits.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RustCompilationObservedUsage {
    pub wall_time_millis: u64,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub artifact_bytes: u64,
    pub peak_process_count: u32,
    pub file_count: u64,
    pub largest_file_bytes: u64,
    pub cpu_seconds: u64,
    pub peak_memory_bytes: u64,
    pub peak_open_files: u64,
}

impl RustCompilationObservedUsage {
    fn within(self, limits: RustCompilationResourceLimits) -> bool {
        self.wall_time_millis <= limits.wall_time_millis
            && self.stdout_bytes <= limits.stdout_bytes
            && self.stderr_bytes <= limits.stderr_bytes
            && self.artifact_bytes <= limits.artifact_bytes
            && self.peak_process_count <= limits.process_count
            && self.file_count <= limits.file_count
            && self.largest_file_bytes <= limits.single_file_bytes
            && self.cpu_seconds <= limits.cpu_seconds
            && self.peak_memory_bytes <= limits.memory_bytes
            && self.peak_open_files <= limits.open_files
    }

    fn exceeds(
        self,
        limit: RustCompilationLimitKind,
        limits: RustCompilationResourceLimits,
    ) -> bool {
        match limit {
            RustCompilationLimitKind::WallTime => self.wall_time_millis > limits.wall_time_millis,
            RustCompilationLimitKind::StdoutBytes => self.stdout_bytes > limits.stdout_bytes,
            RustCompilationLimitKind::StderrBytes => self.stderr_bytes > limits.stderr_bytes,
            RustCompilationLimitKind::ArtifactBytes => self.artifact_bytes > limits.artifact_bytes,
            RustCompilationLimitKind::ProcessCount => {
                self.peak_process_count > limits.process_count
            }
            RustCompilationLimitKind::FileCount => self.file_count > limits.file_count,
            RustCompilationLimitKind::SingleFileBytes => {
                self.largest_file_bytes > limits.single_file_bytes
            }
            RustCompilationLimitKind::CpuTime => self.cpu_seconds > limits.cpu_seconds,
            RustCompilationLimitKind::Memory => self.peak_memory_bytes > limits.memory_bytes,
            RustCompilationLimitKind::OpenFiles => self.peak_open_files > limits.open_files,
        }
    }
}

/// Actual terminal observation supplied by the supervisor after output/resource accounting.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RustCompilationTerminalObservation {
    pub terminal_state: RustCompilationTerminalState,
    pub exit_code: Option<i32>,
    pub exceeded_limit: Option<RustCompilationLimitKind>,
    pub usage: RustCompilationObservedUsage,
    pub accounting_quality: RustCompilationAccountingQuality,
    /// Number of complete process-group observations incorporated into the maxima.
    pub process_sample_count: u64,
    pub process_group_empty: bool,
    /// Digest of the retained bounded stdout prefix. An over-limit attempt is represented by
    /// `usage.stdout_bytes == limit + 1` while the retained file remains exactly at the limit.
    pub stdout_digest: String,
    /// Digest of the retained bounded stderr prefix, with the same over-limit convention.
    pub stderr_digest: String,
    /// Complete terminal output-tree manifest, absent when accounting stopped at an overage.
    pub output_manifest_digest: Option<String>,
}

/// Typed launcher receipt. It proves execution through an admitted plan, not correctness of the
/// compiler's semantic facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RustCompilationLauncherReceipt {
    launcher_id: String,
    policy_digest: String,
    plan_digest: String,
    trust_mode: RustCompilationTrustMode,
    trust_state: RustCompilationTrustState,
    sandbox_mechanism: SandboxMechanism,
    probe_digest: String,
    sandbox_profile_digest: String,
    workspace_id: String,
    provider_run_id: String,
    source_snapshot_digest: String,
    toolchain_digest: String,
    terminal: RustCompilationTerminalObservation,
    cancellation_receipt_digest: Option<String>,
    receipt_digest: String,
}

impl RustCompilationLauncherReceipt {
    /// Digest that identifies this immutable receipt.
    #[must_use]
    pub fn receipt_digest(&self) -> &str {
        &self.receipt_digest
    }

    /// Supervisor-owned terminal observation.
    #[must_use]
    pub const fn terminal(&self) -> &RustCompilationTerminalObservation {
        &self.terminal
    }

    /// Digest of the full process-group cancellation receipt when escalation was required.
    #[must_use]
    pub fn cancellation_receipt_digest(&self) -> Option<&str> {
        self.cancellation_receipt_digest.as_deref()
    }

    /// Close a launch with actual bounded observations.
    ///
    /// # Errors
    ///
    /// Rejects inconsistent exit/terminal states, over-limit captured output, missing group
    /// cancellation evidence, or malformed artifact digests.
    fn close(
        plan: &RustCompilationLaunchPlan,
        terminal: RustCompilationTerminalObservation,
        cancellation: Option<&RustCompilationCancellationReceipt>,
    ) -> Result<Self, RustCompilationTrustError> {
        plan.verify_digest()?;
        if !terminal.process_group_empty {
            return Err(RustCompilationTrustError::ProcessGroupSurvived);
        }
        for digest in [
            Some(&terminal.stdout_digest),
            Some(&terminal.stderr_digest),
            terminal.output_manifest_digest.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            validate_digest(digest)?;
        }
        if plan.trust_mode == RustCompilationTrustMode::UntrustedSandboxed
            && terminal.accounting_quality != RustCompilationAccountingQuality::KernelComplete
        {
            return Err(RustCompilationTrustError::CompleteAccountingUnavailable);
        }
        let requires_cancellation = matches!(
            terminal.terminal_state,
            RustCompilationTerminalState::Cancelled
                | RustCompilationTerminalState::TimedOut
                | RustCompilationTerminalState::ResourceLimit
        );
        let state_is_consistent = match terminal.terminal_state {
            RustCompilationTerminalState::Succeeded => {
                terminal.exit_code == Some(0) && terminal.exceeded_limit.is_none()
            }
            RustCompilationTerminalState::CompilerFailed => {
                terminal.exit_code.is_some_and(|exit_code| exit_code != 0)
                    && terminal.exceeded_limit.is_none()
            }
            RustCompilationTerminalState::Cancelled => terminal.exceeded_limit.is_none(),
            RustCompilationTerminalState::TimedOut => {
                terminal.exceeded_limit == Some(RustCompilationLimitKind::WallTime)
            }
            RustCompilationTerminalState::ResourceLimit => terminal
                .exceeded_limit
                .is_some_and(|limit| limit != RustCompilationLimitKind::WallTime),
        };
        let usage_is_consistent = match terminal.terminal_state {
            RustCompilationTerminalState::Succeeded
            | RustCompilationTerminalState::CompilerFailed
            | RustCompilationTerminalState::Cancelled => terminal.usage.within(plan.limits),
            RustCompilationTerminalState::TimedOut => terminal
                .usage
                .exceeds(RustCompilationLimitKind::WallTime, plan.limits),
            RustCompilationTerminalState::ResourceLimit => terminal
                .exceeded_limit
                .is_some_and(|limit| terminal.usage.exceeds(limit, plan.limits)),
        };
        if matches!(
            terminal.terminal_state,
            RustCompilationTerminalState::Succeeded
                | RustCompilationTerminalState::CompilerFailed
                | RustCompilationTerminalState::Cancelled
        ) && !terminal.usage.within(plan.limits)
        {
            return Err(RustCompilationTrustError::UnboundedObservedUsage);
        }
        if !state_is_consistent || !usage_is_consistent {
            return Err(RustCompilationTrustError::InconsistentTerminalObservation);
        }
        let cancellation_receipt_digest = match (requires_cancellation, cancellation) {
            (false, None) => None,
            (true, Some(receipt))
                if receipt.plan_digest == plan.plan_digest
                    && receipt.group_empty
                    && receipt.verify_digest().is_ok() =>
            {
                Some(receipt.receipt_digest.clone())
            }
            _ => return Err(RustCompilationTrustError::CancellationReceiptMismatch),
        };
        let mut receipt = Self {
            launcher_id: plan.launcher_id.clone(),
            policy_digest: plan.policy_digest.clone(),
            plan_digest: plan.plan_digest.clone(),
            trust_mode: plan.trust_mode,
            trust_state: plan.platform.trust_state,
            sandbox_mechanism: plan.platform.sandbox_mechanism,
            probe_digest: plan.platform.probe_digest.clone(),
            sandbox_profile_digest: plan.sandbox_profile_digest.clone(),
            workspace_id: plan.workspace_id.clone(),
            provider_run_id: plan.provider_run_id.clone(),
            source_snapshot_digest: plan.source_snapshot_digest.clone(),
            toolchain_digest: plan.toolchain_digest.clone(),
            terminal,
            cancellation_receipt_digest,
            receipt_digest: String::new(),
        };
        receipt.receipt_digest = canonical_digest(&receipt)?;
        Ok(receipt)
    }

    /// Recompute the receipt digest.
    pub fn verify_digest(&self) -> Result<(), RustCompilationTrustError> {
        let mut unsigned = self.clone();
        unsigned.receipt_digest.clear();
        if canonical_digest(&unsigned)? != self.receipt_digest {
            return Err(RustCompilationTrustError::LauncherReceiptDigestMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct ProcessGroupSample {
    process_count: u32,
    cpu_millis: u64,
    memory_bytes: u64,
    open_files: u64,
}

struct ProcessGroupObserver {
    ps_path: &'static Path,
    lsof_path: &'static Path,
}

impl ProcessGroupObserver {
    fn probe() -> Result<Self, RustCompilationTrustError> {
        #[cfg(target_os = "macos")]
        let (ps_path, lsof_path) = (Path::new("/bin/ps"), Path::new("/usr/sbin/lsof"));
        #[cfg(target_os = "linux")]
        let (ps_path, lsof_path) = (Path::new("/bin/ps"), Path::new("/usr/bin/lsof"));
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        return Err(RustCompilationTrustError::ProcessAccountingUnavailable);

        if !ps_path.is_file() || !lsof_path.is_file() {
            return Err(RustCompilationTrustError::ProcessAccountingUnavailable);
        }
        Ok(Self { ps_path, lsof_path })
    }

    const fn quality(&self) -> RustCompilationAccountingQuality {
        // `ps`/`lsof` values are real observations, but sampling can miss short-lived descendants.
        // Consequently this backend is intentionally insufficient for UNTRUSTED_SANDBOXED.
        RustCompilationAccountingQuality::SampledDegraded
    }

    fn sample(
        &self,
        process_group_id: i32,
    ) -> Result<ProcessGroupSample, RustCompilationTrustError> {
        let group = process_group_id.to_string();
        #[cfg(target_os = "macos")]
        let ps_group_selector = "-g";
        #[cfg(target_os = "linux")]
        let ps_group_selector = "--pgroup";
        let ps = bounded_command_output(
            self.ps_path,
            &["-o", "pid=,pgid=,rss=,time=", ps_group_selector, &group],
            4 * 1024 * 1024,
        )?;
        if !ps.status.success() {
            return Err(RustCompilationTrustError::ProcessAccountingFailed);
        }
        let text = std::str::from_utf8(&ps.bytes)
            .map_err(|_| RustCompilationTrustError::ProcessAccountingFailed)?;
        let mut process_count = 0_u32;
        let mut cpu_millis = 0_u64;
        let mut memory_bytes = 0_u64;
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let columns = line.split_whitespace().collect::<Vec<_>>();
            if columns.len() != 4 {
                return Err(RustCompilationTrustError::ProcessAccountingFailed);
            }
            let observed_group = columns[1]
                .parse::<i32>()
                .map_err(|_| RustCompilationTrustError::ProcessAccountingFailed)?;
            if observed_group != process_group_id {
                return Err(RustCompilationTrustError::ProcessAccountingFailed);
            }
            let rss_kib = columns[2]
                .parse::<u64>()
                .map_err(|_| RustCompilationTrustError::ProcessAccountingFailed)?;
            process_count = process_count
                .checked_add(1)
                .ok_or(RustCompilationTrustError::ProcessAccountingFailed)?;
            memory_bytes = memory_bytes
                .checked_add(rss_kib.saturating_mul(1024))
                .ok_or(RustCompilationTrustError::ProcessAccountingFailed)?;
            cpu_millis = cpu_millis
                .checked_add(parse_ps_time_millis(columns[3])?)
                .ok_or(RustCompilationTrustError::ProcessAccountingFailed)?;
        }
        if process_count == 0 {
            return Err(RustCompilationTrustError::ProcessAccountingFailed);
        }

        let lsof = bounded_command_output(
            self.lsof_path,
            &["-n", "-P", "-a", "-g", &group, "-Fpf"],
            8 * 1024 * 1024,
        )?;
        if !lsof.status.success() && lsof.status.code() != Some(1) {
            return Err(RustCompilationTrustError::ProcessAccountingFailed);
        }
        let open_files = lsof
            .bytes
            .split(|byte| *byte == b'\n')
            .filter(|line| line.first() == Some(&b'f'))
            .try_fold(0_u64, |count, _| {
                count
                    .checked_add(1)
                    .ok_or(RustCompilationTrustError::ProcessAccountingFailed)
            })?;
        if open_files == 0 {
            return Err(RustCompilationTrustError::ProcessAccountingFailed);
        }
        Ok(ProcessGroupSample {
            process_count,
            cpu_millis,
            memory_bytes,
            open_files,
        })
    }
}

struct BoundedCommandOutput {
    status: ExitStatus,
    bytes: Vec<u8>,
}

fn bounded_command_output(
    program: &Path,
    arguments: &[&str],
    maximum_bytes: u64,
) -> Result<BoundedCommandOutput, RustCompilationTrustError> {
    let mut child = Command::new(program)
        .args(arguments)
        .env_clear()
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or(RustCompilationTrustError::ProcessAccountingFailed)?;
    let mut bytes = Vec::new();
    stdout
        .by_ref()
        .take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum_bytes {
        let _ = child.kill();
        let _ = child.wait();
        return Err(RustCompilationTrustError::ProcessAccountingFailed);
    }
    let status = child.wait()?;
    Ok(BoundedCommandOutput { status, bytes })
}

fn parse_ps_time_millis(value: &str) -> Result<u64, RustCompilationTrustError> {
    let (days, clock) = value
        .split_once('-')
        .map_or((0_u64, value), |(days, clock)| {
            (days.parse::<u64>().unwrap_or(u64::MAX), clock)
        });
    if days == u64::MAX {
        return Err(RustCompilationTrustError::ProcessAccountingFailed);
    }
    let fields = clock.split(':').collect::<Vec<_>>();
    let (hours, minutes, seconds) = match fields.as_slice() {
        [minutes, seconds] => (0_u64, *minutes, *seconds),
        [hours, minutes, seconds] => (
            hours
                .parse::<u64>()
                .map_err(|_| RustCompilationTrustError::ProcessAccountingFailed)?,
            *minutes,
            *seconds,
        ),
        _ => return Err(RustCompilationTrustError::ProcessAccountingFailed),
    };
    let minutes = minutes
        .parse::<u64>()
        .map_err(|_| RustCompilationTrustError::ProcessAccountingFailed)?;
    let (whole_seconds, fractional) = seconds.split_once('.').unwrap_or((seconds, "0"));
    let whole_seconds = whole_seconds
        .parse::<u64>()
        .map_err(|_| RustCompilationTrustError::ProcessAccountingFailed)?;
    if fractional.len() > 3 || !fractional.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(RustCompilationTrustError::ProcessAccountingFailed);
    }
    let fractional_millis = fractional
        .parse::<u64>()
        .map_err(|_| RustCompilationTrustError::ProcessAccountingFailed)?
        * 10_u64.pow(u32::try_from(3_usize.saturating_sub(fractional.len())).unwrap_or(0));
    days.checked_mul(86_400_000)
        .and_then(|value| value.checked_add(hours.saturating_mul(3_600_000)))
        .and_then(|value| value.checked_add(minutes.saturating_mul(60_000)))
        .and_then(|value| value.checked_add(whole_seconds.saturating_mul(1_000)))
        .and_then(|value| value.checked_add(fractional_millis))
        .ok_or(RustCompilationTrustError::ProcessAccountingFailed)
}

#[derive(Debug)]
struct CaptureResult {
    observed_bytes: u64,
    stream_digest: String,
}

struct BoundedCapture {
    observed_bytes: Arc<AtomicU64>,
    overflowed: Arc<AtomicBool>,
    handle: JoinHandle<Result<CaptureResult, std::io::Error>>,
}

fn spawn_bounded_capture<R: std::io::Read + Send + 'static>(
    mut source: R,
    mut destination: File,
    maximum_bytes: u64,
) -> BoundedCapture {
    let observed_bytes = Arc::new(AtomicU64::new(0));
    let overflowed = Arc::new(AtomicBool::new(false));
    let thread_bytes = Arc::clone(&observed_bytes);
    let thread_overflowed = Arc::clone(&overflowed);
    let handle = thread::spawn(move || {
        let mut retained_hasher = Sha256::new();
        let mut buffer = [0_u8; 16 * 1024];
        loop {
            let already_observed = thread_bytes.load(Ordering::Acquire);
            let remaining = maximum_bytes.saturating_sub(already_observed);
            let maximum_read = usize::try_from(remaining.saturating_add(1))
                .unwrap_or(usize::MAX)
                .min(buffer.len());
            let read = source.read(&mut buffer[..maximum_read])?;
            if read == 0 {
                break;
            }
            let read_u64 = u64::try_from(read).unwrap_or(u64::MAX);
            let total = already_observed.saturating_add(read_u64);
            thread_bytes.store(total, Ordering::Release);
            let retained = usize::try_from(remaining).unwrap_or(usize::MAX).min(read);
            if retained > 0 {
                destination.write_all(&buffer[..retained])?;
                retained_hasher.update(&buffer[..retained]);
            }
            if total > maximum_bytes {
                thread_overflowed.store(true, Ordering::Release);
                break;
            }
        }
        destination.sync_all()?;
        Ok(CaptureResult {
            observed_bytes: thread_bytes.load(Ordering::Acquire),
            stream_digest: format!("sha256:{}", hex_bytes(&retained_hasher.finalize())),
        })
    });
    BoundedCapture {
        observed_bytes,
        overflowed,
        handle,
    }
}

#[derive(Clone, Debug, Serialize)]
struct OutputManifestEntry {
    path_hex: String,
    size_bytes: u64,
    content_digest: String,
}

#[derive(Clone, Debug, Default)]
struct OutputTreeObservation {
    artifact_bytes: u64,
    file_count: u64,
    largest_file_bytes: u64,
    manifest_entries: Vec<OutputManifestEntry>,
}

fn observe_output_tree(
    paths: &RustCompilationPrivatePaths,
    hash_contents: bool,
    limits: RustCompilationResourceLimits,
) -> Result<OutputTreeObservation, RustCompilationTrustError> {
    use std::os::unix::fs::FileTypeExt as _;

    paths.revalidate()?;
    let mut observation = OutputTreeObservation::default();
    let mut pending = vec![paths.run_root.clone()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            let file_type = metadata.file_type();
            if file_type.is_symlink() {
                return Err(RustCompilationTrustError::OutputPathViolation);
            }
            if file_type.is_dir() {
                let daemon_baseline = [
                    &paths.target_root,
                    &paths.artifact_root,
                    &paths.temporary_root,
                    &paths.home_root,
                    &paths.control_root,
                ]
                .contains(&&path);
                if !daemon_baseline {
                    observation.file_count = observation
                        .file_count
                        .checked_add(1)
                        .ok_or(RustCompilationTrustError::OutputAccountingOverflow)?;
                    if observation.file_count > limits.file_count {
                        return Ok(observation);
                    }
                }
                pending.push(path);
                continue;
            }
            if path == paths.stdout_path || path == paths.stderr_path {
                continue;
            }
            if file_type.is_socket() && path == paths.extractor_socket_path {
                continue;
            }
            if !file_type.is_file() {
                return Err(RustCompilationTrustError::OutputPathViolation);
            }
            observation.file_count = observation
                .file_count
                .checked_add(1)
                .ok_or(RustCompilationTrustError::OutputAccountingOverflow)?;
            observation.artifact_bytes = observation
                .artifact_bytes
                .checked_add(metadata.len())
                .ok_or(RustCompilationTrustError::OutputAccountingOverflow)?;
            observation.largest_file_bytes = observation.largest_file_bytes.max(metadata.len());
            if observation.file_count > limits.file_count
                || observation.artifact_bytes > limits.artifact_bytes
                || observation.largest_file_bytes > limits.single_file_bytes
            {
                return Ok(observation);
            }
            if hash_contents {
                let relative = path
                    .strip_prefix(&paths.run_root)
                    .map_err(|_| RustCompilationTrustError::OutputPathViolation)?;
                let digest = sha256_file(&path)?;
                let after = fs::metadata(&path)?;
                if after.dev() != metadata.dev()
                    || after.ino() != metadata.ino()
                    || after.len() != metadata.len()
                    || after.modified()? != metadata.modified()?
                {
                    return Err(RustCompilationTrustError::OutputChangedDuringAccounting);
                }
                observation.manifest_entries.push(OutputManifestEntry {
                    path_hex: hex_bytes(relative.as_os_str().as_bytes()),
                    size_bytes: metadata.len(),
                    content_digest: digest,
                });
            }
        }
    }
    observation
        .manifest_entries
        .sort_by(|left, right| left.path_hex.cmp(&right.path_hex));
    Ok(observation)
}

#[derive(Clone, Copy, Debug)]
enum SupervisionDecision {
    Exited(ExitStatus),
    Cancelled,
    TimedOut,
    ResourceLimit(RustCompilationLimitKind),
    LeaderExitedWithDescendants,
}

/// Execute and supervise one admitted Cargo run, returning the only constructible launcher
/// receipt path.
///
/// The supervisor captures bounded stdout/stderr, samples the complete launcher-owned process
/// group, observes the private output tree, and always verifies group emptiness before closing a
/// receipt. Current `ps`/`lsof` accounting is explicitly sampled; therefore untrusted plans fail
/// before spawn until a kernel-complete group-accounting backend is installed. Separately
/// authorized `TRUSTED_LOCAL` plans may execute and remain visibly degraded in their receipt.
///
/// # Errors
///
/// Rejects plan/path drift, pre-launch cancellation, incomplete untrusted accounting, any
/// observation failure, or a process group that survives escalation.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn supervise_rust_compilation(
    plan: &RustCompilationLaunchPlan,
    paths: &RustCompilationPrivatePaths,
    launcher: &ProviderSandboxLauncher,
    sandbox_profile: &GeneratedSandboxProfile,
    material: ProviderSandboxLaunchMaterial<'_>,
    cancellation: &RustCompilationCancellationSignal,
) -> Result<RustCompilationLauncherReceipt, RustCompilationTrustError> {
    plan.verify_digest()?;
    paths.revalidate()?;
    if paths.run_root != plan.output_root {
        return Err(RustCompilationTrustError::PrivatePathChanged);
    }
    if cancellation.is_requested() {
        return Err(RustCompilationTrustError::CancellationBeforeLaunch);
    }
    let observer = ProcessGroupObserver::probe()?;
    let accounting_quality = observer.quality();
    if plan.trust_mode == RustCompilationTrustMode::UntrustedSandboxed
        && accounting_quality != RustCompilationAccountingQuality::KernelComplete
    {
        return Err(RustCompilationTrustError::CompleteAccountingUnavailable);
    }
    let stdout_file = open_capture_file(&paths.stdout_path)?;
    let stderr_file = open_capture_file(&paths.stderr_path)?;
    let mut child = plan.launch_captured(launcher, sandbox_profile, material)?;
    let stdout = child
        .take_stdout()
        .ok_or(RustCompilationTrustError::MissingCapturePipe)?;
    let stderr = child
        .take_stderr()
        .ok_or(RustCompilationTrustError::MissingCapturePipe)?;
    let stdout_capture = spawn_bounded_capture(stdout, stdout_file, plan.limits.stdout_bytes);
    let stderr_capture = spawn_bounded_capture(stderr, stderr_file, plan.limits.stderr_bytes);

    let started = Instant::now();
    let mut usage = RustCompilationObservedUsage {
        wall_time_millis: 0,
        stdout_bytes: 0,
        stderr_bytes: 0,
        artifact_bytes: 0,
        peak_process_count: 0,
        file_count: 0,
        largest_file_bytes: 0,
        cpu_seconds: 0,
        peak_memory_bytes: 0,
        peak_open_files: 0,
    };
    let mut sample_count = 0_u64;

    let decision = loop {
        usage.wall_time_millis = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        usage.stdout_bytes = stdout_capture.observed_bytes.load(Ordering::Acquire);
        usage.stderr_bytes = stderr_capture.observed_bytes.load(Ordering::Acquire);
        if cancellation.is_requested() {
            break SupervisionDecision::Cancelled;
        }
        if stdout_capture.overflowed.load(Ordering::Acquire) {
            break SupervisionDecision::ResourceLimit(RustCompilationLimitKind::StdoutBytes);
        }
        if stderr_capture.overflowed.load(Ordering::Acquire) {
            break SupervisionDecision::ResourceLimit(RustCompilationLimitKind::StderrBytes);
        }
        let output = observe_output_tree(paths, false, plan.limits)?;
        usage.artifact_bytes = usage.artifact_bytes.max(output.artifact_bytes);
        usage.file_count = usage.file_count.max(output.file_count);
        usage.largest_file_bytes = usage.largest_file_bytes.max(output.largest_file_bytes);
        if usage.exceeds(RustCompilationLimitKind::ArtifactBytes, plan.limits) {
            break SupervisionDecision::ResourceLimit(RustCompilationLimitKind::ArtifactBytes);
        }
        if usage.exceeds(RustCompilationLimitKind::FileCount, plan.limits) {
            break SupervisionDecision::ResourceLimit(RustCompilationLimitKind::FileCount);
        }
        if usage.exceeds(RustCompilationLimitKind::SingleFileBytes, plan.limits) {
            break SupervisionDecision::ResourceLimit(RustCompilationLimitKind::SingleFileBytes);
        }

        if let Some(status) = child.try_wait()? {
            if child.wait_group_empty(Duration::ZERO)? {
                break SupervisionDecision::Exited(status);
            }
            break SupervisionDecision::LeaderExitedWithDescendants;
        }

        let sample = observer.sample(child.process_group_id())?;
        sample_count = sample_count.saturating_add(1);
        usage.peak_process_count = usage.peak_process_count.max(sample.process_count);
        usage.cpu_seconds = usage.cpu_seconds.max(sample.cpu_millis.div_ceil(1_000));
        usage.peak_memory_bytes = usage.peak_memory_bytes.max(sample.memory_bytes);
        usage.peak_open_files = usage.peak_open_files.max(sample.open_files);
        if usage.exceeds(RustCompilationLimitKind::ProcessCount, plan.limits) {
            break SupervisionDecision::ResourceLimit(RustCompilationLimitKind::ProcessCount);
        }
        if usage.exceeds(RustCompilationLimitKind::CpuTime, plan.limits) {
            break SupervisionDecision::ResourceLimit(RustCompilationLimitKind::CpuTime);
        }
        if usage.exceeds(RustCompilationLimitKind::Memory, plan.limits) {
            break SupervisionDecision::ResourceLimit(RustCompilationLimitKind::Memory);
        }
        if usage.exceeds(RustCompilationLimitKind::OpenFiles, plan.limits) {
            break SupervisionDecision::ResourceLimit(RustCompilationLimitKind::OpenFiles);
        }
        if usage.wall_time_millis > plan.limits.wall_time_millis {
            break SupervisionDecision::TimedOut;
        }
        thread::sleep(Duration::from_millis(100));
    };

    let cancellation_receipt = match decision {
        SupervisionDecision::Exited(_) => None,
        SupervisionDecision::Cancelled => Some(cancel_rust_compilation_process_group(
            plan,
            &plan.plan_digest,
            RustCompilationCancellationReason::Requested,
            &mut child,
        )?),
        SupervisionDecision::TimedOut => Some(cancel_rust_compilation_process_group(
            plan,
            &plan.plan_digest,
            RustCompilationCancellationReason::WallTimeExceeded,
            &mut child,
        )?),
        SupervisionDecision::ResourceLimit(_) => Some(cancel_rust_compilation_process_group(
            plan,
            &plan.plan_digest,
            RustCompilationCancellationReason::ResourceLimitExceeded,
            &mut child,
        )?),
        SupervisionDecision::LeaderExitedWithDescendants => {
            Some(cancel_rust_compilation_process_group(
                plan,
                &plan.plan_digest,
                RustCompilationCancellationReason::LeaderExitedWithDescendants,
                &mut child,
            )?)
        }
    };
    if !child.wait_group_empty(Duration::from_millis(
        plan.cancellation.termination_grace_millis,
    ))? {
        return Err(RustCompilationTrustError::ProcessGroupSurvived);
    }
    let exit_status = match decision {
        SupervisionDecision::Exited(status) => Some(status),
        _ => child.wait().ok(),
    };

    let stdout = stdout_capture
        .handle
        .join()
        .map_err(|_| RustCompilationTrustError::CaptureThreadFailed)??;
    let stderr = stderr_capture
        .handle
        .join()
        .map_err(|_| RustCompilationTrustError::CaptureThreadFailed)??;
    usage.wall_time_millis = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    usage.stdout_bytes = stdout.observed_bytes;
    usage.stderr_bytes = stderr.observed_bytes;
    let final_output = observe_output_tree(paths, true, plan.limits)?;
    usage.artifact_bytes = usage.artifact_bytes.max(final_output.artifact_bytes);
    usage.file_count = usage.file_count.max(final_output.file_count);
    usage.largest_file_bytes = usage
        .largest_file_bytes
        .max(final_output.largest_file_bytes);
    let final_manifest_complete = final_output.artifact_bytes <= plan.limits.artifact_bytes
        && final_output.file_count <= plan.limits.file_count
        && final_output.largest_file_bytes <= plan.limits.single_file_bytes;
    let output_manifest_digest = final_manifest_complete
        .then(|| canonical_digest(&final_output.manifest_entries))
        .transpose()?;
    let (terminal_state, exceeded_limit) = match decision {
        SupervisionDecision::Exited(status) if status.success() => {
            (RustCompilationTerminalState::Succeeded, None)
        }
        SupervisionDecision::Exited(_) => (RustCompilationTerminalState::CompilerFailed, None),
        SupervisionDecision::Cancelled | SupervisionDecision::LeaderExitedWithDescendants => {
            (RustCompilationTerminalState::Cancelled, None)
        }
        SupervisionDecision::TimedOut => (
            RustCompilationTerminalState::TimedOut,
            Some(RustCompilationLimitKind::WallTime),
        ),
        SupervisionDecision::ResourceLimit(limit) => {
            (RustCompilationTerminalState::ResourceLimit, Some(limit))
        }
    };
    RustCompilationLauncherReceipt::close(
        plan,
        RustCompilationTerminalObservation {
            terminal_state,
            exit_code: exit_status.and_then(|status| status.code()),
            exceeded_limit,
            usage,
            accounting_quality,
            process_sample_count: sample_count,
            process_group_empty: true,
            stdout_digest: stdout.stream_digest,
            stderr_digest: stderr.stream_digest,
            output_manifest_digest,
        },
        cancellation_receipt.as_ref(),
    )
}

fn open_capture_file(path: &Path) -> Result<File, RustCompilationTrustError> {
    use std::os::unix::fs::OpenOptionsExt as _;

    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true).mode(0o600);
    Ok(options.open(path)?)
}

/// Capability projection derived from the selected platform row and a real launcher receipt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RustCompilationCapabilityRow {
    pub workspace_id: String,
    pub provider_run_id: String,
    pub trust_mode: RustCompilationTrustMode,
    pub trust_state: RustCompilationTrustState,
    pub available: bool,
    pub degraded: bool,
    pub reason_code: String,
    pub policy_digest: String,
    pub platform_probe_digest: String,
    pub launcher_receipt_digest: String,
}

/// Provenance projection retained with raw rustc facts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RustCompilationProvenanceRow {
    pub workspace_id: String,
    pub provider_run_id: String,
    pub source_generation: u64,
    pub source_snapshot_digest: String,
    pub dependency_snapshot_digest: String,
    pub toolchain_digest: String,
    pub exact_toolchain_release: String,
    pub policy_digest: String,
    pub plan_digest: String,
    pub platform_probe_digest: String,
    pub sandbox_profile_digest: String,
    pub launcher_receipt_digest: String,
    pub trusted_local_authorization_id: Option<String>,
    pub build_scripts_policy: RustExecutableExtensionPolicy,
    pub procedural_macros_policy: RustExecutableExtensionPolicy,
}

/// Derive public relational projections from a real, matching receipt.
///
/// # Errors
///
/// Rejects claimed-only or cross-plan receipts.
pub fn project_rust_compilation_trust_rows(
    plan: &RustCompilationLaunchPlan,
    receipt: &RustCompilationLauncherReceipt,
) -> Result<(RustCompilationCapabilityRow, RustCompilationProvenanceRow), RustCompilationTrustError>
{
    plan.verify_digest()?;
    receipt.verify_digest()?;
    if receipt.plan_digest != plan.plan_digest
        || receipt.policy_digest != plan.policy_digest
        || receipt.provider_run_id != plan.provider_run_id
        || receipt.source_snapshot_digest != plan.source_snapshot_digest
        || receipt.toolchain_digest != plan.toolchain_digest
    {
        return Err(RustCompilationTrustError::LauncherReceiptMismatch);
    }
    let available = matches!(
        receipt.terminal.terminal_state,
        RustCompilationTerminalState::Succeeded | RustCompilationTerminalState::CompilerFailed
    );
    let reason_code = match receipt.terminal.terminal_state {
        RustCompilationTerminalState::Succeeded => "RUST_COMPILATION_COMPLETED",
        RustCompilationTerminalState::CompilerFailed => "RUST_COMPILATION_FAILED_TYPED_GAP",
        RustCompilationTerminalState::Cancelled => "RUST_COMPILATION_CANCELLED",
        RustCompilationTerminalState::TimedOut => "RUST_COMPILATION_WALL_TIME_LIMIT",
        RustCompilationTerminalState::ResourceLimit => "RUST_COMPILATION_RESOURCE_LIMIT",
    };
    Ok((
        RustCompilationCapabilityRow {
            workspace_id: plan.workspace_id.clone(),
            provider_run_id: plan.provider_run_id.clone(),
            trust_mode: plan.trust_mode,
            trust_state: plan.platform.trust_state,
            available,
            degraded: plan.platform.trust_state == RustCompilationTrustState::DegradedTrustedLocal,
            reason_code: reason_code.into(),
            policy_digest: plan.policy_digest.clone(),
            platform_probe_digest: plan.platform.probe_digest.clone(),
            launcher_receipt_digest: receipt.receipt_digest.clone(),
        },
        RustCompilationProvenanceRow {
            workspace_id: plan.workspace_id.clone(),
            provider_run_id: plan.provider_run_id.clone(),
            source_generation: plan.source_generation,
            source_snapshot_digest: plan.source_snapshot_digest.clone(),
            dependency_snapshot_digest: plan.dependency_snapshot_digest.clone(),
            toolchain_digest: plan.toolchain_digest.clone(),
            exact_toolchain_release: plan.exact_toolchain_release.clone(),
            policy_digest: plan.policy_digest.clone(),
            plan_digest: plan.plan_digest.clone(),
            platform_probe_digest: plan.platform.probe_digest.clone(),
            sandbox_profile_digest: plan.sandbox_profile_digest.clone(),
            launcher_receipt_digest: receipt.receipt_digest.clone(),
            trusted_local_authorization_id: plan.trusted_local_authorization_id.clone(),
            build_scripts_policy: plan.executable_extensions,
            procedural_macros_policy: plan.executable_extensions,
        },
    ))
}

#[derive(Debug, Error)]
pub enum RustCompilationTrustError {
    #[error("Rust compilation resource limits are invalid or effectively unbounded")]
    InvalidResourceLimits,
    #[error("Rust compilation trust policy is not closed")]
    OpenPolicy,
    #[error("input roots overlap")]
    InputRootsOverlap,
    #[error("compiler executable escapes the immutable dependency view")]
    ExecutableEscapesDependencies,
    #[error("immutable input identity changed before launch")]
    InputIdentityChanged,
    #[error("private output path changed before launch")]
    PrivatePathChanged,
    #[error("private output overlaps an immutable input")]
    OutputOverlapsInput,
    #[error("path is not canonical, absolute, and unaliased")]
    NonCanonicalPath,
    #[error("path is not the required file or directory kind")]
    WrongPathKind,
    #[error("private output path is not daemon-owned and mode 0700")]
    NonPrivateOutput,
    #[error("path escapes its authorized root")]
    PathEscape,
    #[error("manifest path must be a relative Cargo.toml path")]
    InvalidManifestPath,
    #[error("identifier is malformed: {0}")]
    InvalidIdentifier(&'static str),
    #[error("digest is malformed")]
    InvalidDigest,
    #[error("Cargo selection token is malformed")]
    InvalidInvocationToken,
    #[error("Cargo selection token is duplicated")]
    DuplicateInvocationToken,
    #[error("sandbox profile differs from admitted paths, trust mode, or bytes")]
    SandboxProfileMismatch,
    #[error("proved untrusted containment is unavailable")]
    ContainmentUnavailable,
    #[error("trusted-local authorization is required")]
    TrustedLocalAuthorizationRequired,
    #[error("trusted-local authorization does not bind this exact run")]
    TrustedLocalAuthorizationMismatch,
    #[error("trusted-local profile does not use the explicit degraded mechanism")]
    TrustedLocalProfileMismatch,
    #[error("trusted-local authorization was supplied to an untrusted plan")]
    UnexpectedTrustedLocalAuthorization,
    #[error("build script or procedural macro is rejected by policy")]
    ExecutableExtensionRejected,
    #[error("environment contains a forbidden or unknown variable")]
    ForbiddenEnvironmentVariable,
    #[error("environment is not the exact admitted closed map")]
    EnvironmentNotClosed,
    #[error("launch plan digest changed")]
    LaunchPlanDigestMismatch,
    #[error("cancellation does not bind the admitted launch plan")]
    CancellationBindingMismatch,
    #[error("compiler process-group descendants survived kill escalation")]
    ProcessGroupSurvived,
    #[error("kernel-complete process-group accounting is unavailable for untrusted execution")]
    CompleteAccountingUnavailable,
    #[error("process-group accounting tools are unavailable on this host")]
    ProcessAccountingUnavailable,
    #[error("process-group accounting failed or returned malformed observations")]
    ProcessAccountingFailed,
    #[error("run-wide cancellation was requested before the provider was launched")]
    CancellationBeforeLaunch,
    #[error("bounded stdout/stderr capture pipe was not created")]
    MissingCapturePipe,
    #[error("bounded stdout/stderr capture thread failed")]
    CaptureThreadFailed,
    #[error("provider output contains a symlink or unauthorized special file")]
    OutputPathViolation,
    #[error("provider output accounting overflowed")]
    OutputAccountingOverflow,
    #[error("provider output changed while its terminal manifest was being accounted")]
    OutputChangedDuringAccounting,
    #[error("captured usage exceeds the admitted bound")]
    UnboundedObservedUsage,
    #[error("terminal state, exit status, and limit observation disagree")]
    InconsistentTerminalObservation,
    #[error("cancellation receipt is missing or belongs to another run")]
    CancellationReceiptMismatch,
    #[error("launcher receipt digest changed")]
    LauncherReceiptDigestMismatch,
    #[error("launcher receipt does not bind the admitted plan")]
    LauncherReceiptMismatch,
    #[error(transparent)]
    Sandbox(#[from] SandboxError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Rustix(#[from] rustix::io::Errno),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

fn canonical_unaliased_directory(path: &Path) -> Result<PathBuf, RustCompilationTrustError> {
    canonical_unaliased(path, true)
}

fn canonical_unaliased_file(path: &Path) -> Result<PathBuf, RustCompilationTrustError> {
    canonical_unaliased(path, false)
}

fn canonical_unaliased(path: &Path, directory: bool) -> Result<PathBuf, RustCompilationTrustError> {
    if !path.is_absolute() || fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(RustCompilationTrustError::NonCanonicalPath);
    }
    let canonical = fs::canonicalize(path)?;
    if canonical != path {
        return Err(RustCompilationTrustError::NonCanonicalPath);
    }
    let metadata = fs::metadata(path)?;
    if (directory && !metadata.is_dir()) || (!directory && !metadata.is_file()) {
        return Err(RustCompilationTrustError::WrongPathKind);
    }
    Ok(canonical)
}

fn path_identity(path: &Path) -> Result<RustCompilationPathIdentity, RustCompilationTrustError> {
    let metadata = fs::metadata(path)?;
    Ok(RustCompilationPathIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

fn validate_private_directory(path: &Path) -> Result<(), RustCompilationTrustError> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_dir()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(RustCompilationTrustError::NonPrivateOutput);
    }
    Ok(())
}

fn private_child(
    run_root: &Path,
    child_name: &'static str,
) -> Result<PathBuf, RustCompilationTrustError> {
    let expected = run_root.join(child_name);
    let child = canonical_unaliased_directory(&expected)?;
    validate_private_directory(&child)?;
    if child.parent() != Some(run_root) {
        return Err(RustCompilationTrustError::PathEscape);
    }
    Ok(child)
}

fn roots_overlap(left: &Path, right: &Path) -> bool {
    left == right || left.starts_with(right) || right.starts_with(left)
}

fn validate_relative_path(path: &Path) -> Result<(), RustCompilationTrustError> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(RustCompilationTrustError::InvalidManifestPath);
    }
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err(RustCompilationTrustError::PathEscape);
        };
        let Some(value) = value.to_str() else {
            return Err(RustCompilationTrustError::InvalidManifestPath);
        };
        if value.is_empty() || value.contains('\0') {
            return Err(RustCompilationTrustError::InvalidManifestPath);
        }
    }
    Ok(())
}

fn validate_identifier(value: &str, field: &'static str) -> Result<(), RustCompilationTrustError> {
    if value.is_empty()
        || value.len() > 256
        || value.starts_with('-')
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'-' | b'_' | b'.' | b':' | b'@' | b'+' | b'(' | b')' | b' '
                )
        })
    {
        return Err(RustCompilationTrustError::InvalidIdentifier(field));
    }
    Ok(())
}

fn validate_cargo_token(value: &str) -> Result<(), RustCompilationTrustError> {
    if value.is_empty()
        || value.len() > 128
        || value.starts_with('-')
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'-' | b'_' | b'.' | b'+' | b'/' | b':' | b'?')
        })
    {
        return Err(RustCompilationTrustError::InvalidInvocationToken);
    }
    Ok(())
}

fn validate_digest(value: &str) -> Result<(), RustCompilationTrustError> {
    let suffix = value
        .strip_prefix("b3:")
        .or_else(|| value.strip_prefix("sha256:"))
        .ok_or(RustCompilationTrustError::InvalidDigest)?;
    if suffix.len() != 64
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(RustCompilationTrustError::InvalidDigest);
    }
    Ok(())
}

fn validate_environment_variables(
    variables: &BTreeMap<String, String>,
) -> Result<(), RustCompilationTrustError> {
    const ALLOWED: [&str; 24] = [
        "PATH",
        "HOME",
        "TMPDIR",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "CARGO_TARGET_DIR",
        "CARGO_NET_OFFLINE",
        "CARGO_INCREMENTAL",
        "CARGO_TERM_COLOR",
        "RUST_BACKTRACE",
        "RUSTC",
        "RUSTC_WRAPPER",
        "CODEFABRIC_EXTRACTOR_ENDPOINT",
        "CODEFABRIC_PROVIDER_RUN_ID",
        "CODEFABRIC_WORKSPACE_ID",
        "CODEFABRIC_ANALYSIS_CONTEXT_ID",
        "CODEFABRIC_SOURCE_GENERATION",
        "CODEFABRIC_CONTEXT_MANIFEST_DIGEST",
        "CODEFABRIC_PROVIDER_RESOURCE_PROFILE_ID",
        "CODEFABRIC_SOURCE_SNAPSHOT_MANIFEST_DIGEST",
        "CODEFABRIC_CARGO_METADATA_DIGEST",
        "CODEFABRIC_CARGO_LOCK_DIGEST",
        "CODEFABRIC_CARGO_CONFIG_DIGEST",
        "LC_ALL",
    ];
    for (name, value) in variables {
        let upper = name.to_ascii_uppercase();
        let sensitive = upper.contains("PROXY")
            || upper.starts_with("AWS_")
            || upper.starts_with("AZURE_")
            || upper.starts_with("GOOGLE_")
            || upper.starts_with("GCP_")
            || upper.starts_with("GITHUB_")
            || upper.starts_with("GITLAB_")
            || upper.starts_with("SSH_")
            || upper.starts_with("GPG_")
            || upper.starts_with("DOCKER_")
            || upper.starts_with("VAULT_")
            || upper == "KUBECONFIG"
            || upper.ends_with("_TOKEN")
            || upper.ends_with("_SECRET")
            || upper.ends_with("_PASSWORD")
            || upper.ends_with("_CREDENTIAL")
            || upper.ends_with("_CREDENTIALS")
            || upper.ends_with("_AGENT_PID")
            || upper.ends_with("_AUTH_SOCK");
        if !ALLOWED.contains(&name.as_str())
            || sensitive
            || name.contains('=')
            || value.contains('\0')
            || value.contains('\n')
            || value.contains('\r')
        {
            return Err(RustCompilationTrustError::ForbiddenEnvironmentVariable);
        }
    }
    Ok(())
}

fn canonical_digest(value: &impl Serialize) -> Result<String, RustCompilationTrustError> {
    Ok(sha256_bytes(&serde_json_canonicalizer::to_vec(value)?))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(71);
    encoded.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        write!(encoded, "{byte:02x}").expect("writing to a String is infallible");
    }
    encoded
}

fn sha256_file(path: &Path) -> Result<String, RustCompilationTrustError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    Ok(format!("sha256:{}", hex_bytes(&digest)))
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(HEX[usize::from(*byte >> 4)]));
        output.push(char::from(HEX[usize::from(*byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::os::unix::fs::{DirBuilderExt as _, symlink};

    use tempfile::TempDir;

    use super::*;
    use crate::provider_sandbox::{SandboxCapabilityMatrix, SandboxProbeObservation};

    struct Harness {
        _root: TempDir,
        inputs: RustCompilationInputs,
        paths: RustCompilationPrivatePaths,
        request: RustCompilationRunRequest,
        capabilities: SandboxCapabilityMatrix,
        profile: GeneratedSandboxProfile,
    }

    fn digest(byte: u8) -> String {
        format!("b3:{}", format!("{byte:02x}").repeat(32))
    }

    fn limits() -> RustCompilationResourceLimits {
        RustCompilationResourceLimits {
            wall_time_millis: 30_000,
            stdout_bytes: 1024 * 1024,
            stderr_bytes: 1024 * 1024,
            artifact_bytes: 32 * 1024 * 1024,
            process_count: 32,
            file_count: 10_000,
            single_file_bytes: 8 * 1024 * 1024,
            cpu_seconds: 120,
            memory_bytes: 1024 * 1024 * 1024,
            open_files: 128,
        }
    }

    fn observation(all_pass: bool) -> SandboxProbeObservation {
        SandboxProbeObservation {
            mechanism: SandboxMechanism::DarwinSeatbelt,
            executable_path: "/usr/bin/sandbox-exec".into(),
            executable_version: "darwin-seatbelt-system".into(),
            owned_by_root: true,
            executable_mode: 0o755,
            setuid: false,
            behavior: [
                "launch-confined",
                "leased-workspace-read-allowed",
                "workspace-write-denied",
                "out-of-root-write-denied",
                "live-workspace-read-denied",
                "credential-read-denied",
                "git-read-denied",
                "network-denied",
                "inherited-fd-read-denied",
                "child-process-contained",
                "resource-limit-enforceable",
                "cleanup-escape-denied",
                "output-write-allowed",
            ]
            .into_iter()
            .map(|name| (name.into(), all_pass))
            .collect(),
        }
    }

    fn create_private_directory(path: &Path) {
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        builder.create(path).unwrap();
    }

    fn create_executable(path: &Path) {
        fs::write(path, b"fixture executable").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    fn harness(mode: RustCompilationTrustMode) -> Harness {
        harness_with_cargo(mode, b"fixture executable")
    }

    fn harness_with_cargo(mode: RustCompilationTrustMode, cargo_program: &[u8]) -> Harness {
        let root = tempfile::tempdir().unwrap();
        let root_path = fs::canonicalize(root.path()).unwrap();
        let workspace = root_path.join("workspace");
        let dependencies = root_path.join("dependencies");
        let private_parent = root_path.join("private");
        fs::create_dir(&workspace).unwrap();
        fs::create_dir(&dependencies).unwrap();
        create_private_directory(&private_parent);
        fs::write(
            workspace.join("Cargo.toml"),
            b"[package]\nname='fixture'\nversion='0.0.0'\n",
        )
        .unwrap();
        let toolchain_bin = dependencies.join("toolchain/bin");
        fs::create_dir_all(&toolchain_bin).unwrap();
        fs::create_dir(dependencies.join("cargo-home")).unwrap();
        fs::create_dir(dependencies.join("rustup-home")).unwrap();
        let cargo = toolchain_bin.join("cargo");
        let rustc = toolchain_bin.join("rustc");
        let wrapper = toolchain_bin.join("codefabric-rustc-extractor");
        for executable in [&cargo, &rustc, &wrapper] {
            create_executable(executable);
        }
        fs::write(&cargo, cargo_program).unwrap();
        fs::set_permissions(&cargo, fs::Permissions::from_mode(0o700)).unwrap();
        let inputs = RustCompilationInputs::inspect(
            &workspace,
            &dependencies,
            &cargo,
            &rustc,
            &wrapper,
            &digest(1),
            &digest(2),
            &digest(3),
            "rustc-1.100.0-nightly-2026-08-18",
        )
        .unwrap();
        let paths = RustCompilationPrivatePaths::prepare(&private_parent, "run-1").unwrap();
        let request = RustCompilationRunRequest {
            manifest_relative_path: "Cargo.toml".into(),
            package_names: vec!["fixture".into()],
            feature_names: vec!["semantic".into()],
            all_targets: true,
            build_scripts_present: true,
            procedural_macros_present: true,
            context: RustCompilationContextPins {
                provider_run_id: "provider-run-1".into(),
                workspace_id: "workspace-1".into(),
                analysis_context_id: "analysis-1".into(),
                source_generation: 7,
                context_manifest_digest: digest(4),
                resource_profile_id: "rust-compiler-medium".into(),
                source_snapshot_manifest_digest: digest(5),
                cargo_metadata_digest: digest(6),
                cargo_lock_digest: digest(7),
                cargo_config_digest: digest(8),
            },
        };
        let capabilities = SandboxCapabilityMatrix::evaluate(&observation(true));
        let (provider_profile, mechanism) = match mode {
            RustCompilationTrustMode::UntrustedSandboxed => (
                ProviderTrustProfile::UntrustedSandboxed,
                SandboxMechanism::DarwinSeatbelt,
            ),
            RustCompilationTrustMode::TrustedLocal => {
                (ProviderTrustProfile::TrustedLocal, SandboxMechanism::None)
            }
        };
        let profile = GeneratedSandboxProfile::generate(
            provider_profile,
            mechanism,
            &inputs.workspace_view,
            &inputs.dependency_view,
            &paths.run_root,
        )
        .unwrap();
        Harness {
            _root: root,
            inputs,
            paths,
            request,
            capabilities,
            profile,
        }
    }

    fn untrusted_policy() -> RustCompilationTrustPolicy {
        RustCompilationTrustPolicy::untrusted_sandboxed_v1(
            limits(),
            RustExecutableExtensionPolicy::ExecuteInsideSelectedLauncher,
        )
    }

    fn compile_untrusted(harness: &Harness) -> RustCompilationLaunchPlan {
        compile_rust_compilation_launch_plan(
            &untrusted_policy(),
            &harness.capabilities,
            &harness.profile,
            &harness.inputs,
            &harness.paths,
            &harness.request,
            None,
        )
        .unwrap()
    }

    fn compile_trusted(
        harness: &Harness,
        limits: RustCompilationResourceLimits,
    ) -> RustCompilationLaunchPlan {
        let policy = RustCompilationTrustPolicy::trusted_local_v1(
            limits,
            RustExecutableExtensionPolicy::ExecuteInsideSelectedLauncher,
        );
        let authorization = trusted_authorization(&policy, harness);
        compile_rust_compilation_launch_plan(
            &policy,
            &harness.capabilities,
            &harness.profile,
            &harness.inputs,
            &harness.paths,
            &harness.request,
            Some(&authorization),
        )
        .unwrap()
    }

    fn trusted_authorization(
        policy: &RustCompilationTrustPolicy,
        harness: &Harness,
    ) -> TrustedLocalAuthorization {
        TrustedLocalAuthorization {
            authorization_id: "trusted-local-grant-1".into(),
            workspace_id: harness.request.context.workspace_id.clone(),
            provider_run_id: harness.request.context.provider_run_id.clone(),
            policy_digest: policy.digest().unwrap(),
            source_snapshot_digest: harness.inputs.source_snapshot_digest.clone(),
            toolchain_digest: harness.inputs.toolchain_digest.clone(),
        }
    }

    fn usage() -> RustCompilationObservedUsage {
        RustCompilationObservedUsage {
            wall_time_millis: 500,
            stdout_bytes: 20,
            stderr_bytes: 10,
            artifact_bytes: 4_096,
            peak_process_count: 4,
            file_count: 12,
            largest_file_bytes: 1_024,
            cpu_seconds: 1,
            peak_memory_bytes: 32 * 1024 * 1024,
            peak_open_files: 16,
        }
    }

    fn success_observation() -> RustCompilationTerminalObservation {
        RustCompilationTerminalObservation {
            terminal_state: RustCompilationTerminalState::Succeeded,
            exit_code: Some(0),
            exceeded_limit: None,
            usage: usage(),
            accounting_quality: RustCompilationAccountingQuality::KernelComplete,
            process_sample_count: 1,
            process_group_empty: true,
            stdout_digest: digest(90),
            stderr_digest: digest(91),
            output_manifest_digest: Some(digest(9)),
        }
    }

    #[test]
    fn untrusted_plan_is_closed_probe_bound_and_deterministic() {
        let harness = harness(RustCompilationTrustMode::UntrustedSandboxed);
        let first = compile_untrusted(&harness);
        let second = compile_untrusted(&harness);

        assert_eq!(first, second);
        assert_eq!(
            first.platform.trust_state,
            RustCompilationTrustState::ProvedUntrustedContainment
        );
        assert_eq!(first.platform.capability_reason_code, "SANDBOX_PROVED");
        assert!(first.contained_arguments.contains(&"--offline".into()));
        assert!(first.contained_arguments.contains(&"--locked".into()));
        assert_eq!(
            first.environment.variables.get("CARGO_NET_OFFLINE"),
            Some(&"true".into())
        );
        assert!(!first.environment.variables.contains_key("HTTP_PROXY"));
        assert_eq!(
            first.provider_launch_request().limits,
            limits().provider_process_limits()
        );
        let provider_request = first.provider_launch_request();
        assert_eq!(
            provider_request.host_executable,
            first.host_cargo_executable
        );
        assert_eq!(
            provider_request.contained_executable,
            first.contained_cargo_executable
        );
        assert_eq!(provider_request.environment, first.environment.variables);
        first.verify_digest().unwrap();
    }

    #[test]
    fn linux_contained_layout_translates_only_after_host_identity_binding() {
        let harness = harness(RustCompilationTrustMode::UntrustedSandboxed);
        let layout = ContainedPathLayout::new(
            &harness.inputs,
            &harness.paths,
            SandboxMechanism::LinuxBubblewrap,
        )
        .unwrap();

        assert_eq!(
            layout.cargo_executable,
            Path::new("/dependencies/toolchain/bin/cargo")
        );
        assert!(harness.inputs.cargo_executable.is_file());
        assert_ne!(layout.cargo_executable, harness.inputs.cargo_executable);
    }

    #[test]
    fn unavailable_untrusted_containment_never_falls_back() {
        let harness = harness(RustCompilationTrustMode::UntrustedSandboxed);
        let unavailable = SandboxCapabilityMatrix::evaluate(&observation(false));
        let error = compile_rust_compilation_launch_plan(
            &untrusted_policy(),
            &unavailable,
            &harness.profile,
            &harness.inputs,
            &harness.paths,
            &harness.request,
            None,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            RustCompilationTrustError::ContainmentUnavailable
        ));
    }

    #[test]
    fn trusted_local_requires_exact_separate_authorization_and_stays_degraded() {
        let harness = harness(RustCompilationTrustMode::TrustedLocal);
        let policy = RustCompilationTrustPolicy::trusted_local_v1(
            limits(),
            RustExecutableExtensionPolicy::ExecuteInsideSelectedLauncher,
        );
        let missing = compile_rust_compilation_launch_plan(
            &policy,
            &harness.capabilities,
            &harness.profile,
            &harness.inputs,
            &harness.paths,
            &harness.request,
            None,
        )
        .unwrap_err();
        assert!(matches!(
            missing,
            RustCompilationTrustError::TrustedLocalAuthorizationRequired
        ));

        let mut wrong = trusted_authorization(&policy, &harness);
        wrong.toolchain_digest = digest(99);
        assert!(matches!(
            compile_rust_compilation_launch_plan(
                &policy,
                &harness.capabilities,
                &harness.profile,
                &harness.inputs,
                &harness.paths,
                &harness.request,
                Some(&wrong),
            )
            .unwrap_err(),
            RustCompilationTrustError::TrustedLocalAuthorizationMismatch
        ));

        let authorization = trusted_authorization(&policy, &harness);
        let plan = compile_rust_compilation_launch_plan(
            &policy,
            &harness.capabilities,
            &harness.profile,
            &harness.inputs,
            &harness.paths,
            &harness.request,
            Some(&authorization),
        )
        .unwrap();
        assert_eq!(
            plan.platform.trust_state,
            RustCompilationTrustState::DegradedTrustedLocal
        );
        assert_eq!(
            plan.trusted_local_authorization_id.as_deref(),
            Some("trusted-local-grant-1")
        );
    }

    #[test]
    fn policy_closure_and_all_resource_limits_are_enforced() {
        let mut open = untrusted_policy();
        open.close_inherited_descriptors = false;
        assert!(matches!(
            open.digest().unwrap_err(),
            RustCompilationTrustError::OpenPolicy
        ));

        let mut invalid = limits();
        invalid.stdout_bytes = 0;
        assert!(matches!(
            invalid.validate().unwrap_err(),
            RustCompilationTrustError::InvalidResourceLimits
        ));
        invalid = limits();
        invalid.process_count = MAX_PROCESS_COUNT + 1;
        assert!(matches!(
            invalid.validate().unwrap_err(),
            RustCompilationTrustError::InvalidResourceLimits
        ));
    }

    #[test]
    fn credential_proxy_agent_and_unknown_environment_are_rejected() {
        let harness = harness(RustCompilationTrustMode::UntrustedSandboxed);
        let plan = compile_untrusted(&harness);
        for name in [
            "AWS_SECRET_ACCESS_KEY",
            "HTTPS_PROXY",
            "SSH_AUTH_SOCK",
            "GITHUB_TOKEN",
            "UNREVIEWED_VALUE",
        ] {
            let mut hostile = plan.environment.variables.clone();
            hostile.insert(name.into(), "host-value".into());
            assert!(
                plan.environment.validate_candidate(&hostile).is_err(),
                "{name}"
            );
        }
    }

    #[test]
    fn hostile_manifest_tokens_and_executable_extensions_fail_before_launch() {
        let mut harness = harness(RustCompilationTrustMode::UntrustedSandboxed);
        harness.request.manifest_relative_path = "../Cargo.toml".into();
        assert!(matches!(
            compile_rust_compilation_launch_plan(
                &untrusted_policy(),
                &harness.capabilities,
                &harness.profile,
                &harness.inputs,
                &harness.paths,
                &harness.request,
                None,
            )
            .unwrap_err(),
            RustCompilationTrustError::PathEscape
        ));

        harness.request.manifest_relative_path = "Cargo.toml".into();
        harness.request.package_names = vec!["--target-dir=/tmp/escape".into()];
        assert!(matches!(
            compile_rust_compilation_launch_plan(
                &untrusted_policy(),
                &harness.capabilities,
                &harness.profile,
                &harness.inputs,
                &harness.paths,
                &harness.request,
                None,
            )
            .unwrap_err(),
            RustCompilationTrustError::InvalidInvocationToken
        ));

        harness.request.package_names = vec!["fixture".into()];
        let reject = RustCompilationTrustPolicy::untrusted_sandboxed_v1(
            limits(),
            RustExecutableExtensionPolicy::RejectWorkspaceWhenPresent,
        );
        assert!(matches!(
            compile_rust_compilation_launch_plan(
                &reject,
                &harness.capabilities,
                &harness.profile,
                &harness.inputs,
                &harness.paths,
                &harness.request,
                None,
            )
            .unwrap_err(),
            RustCompilationTrustError::ExecutableExtensionRejected
        ));
    }

    #[test]
    fn private_output_symlink_escape_and_changed_input_identity_are_rejected() {
        let symlink_harness = harness(RustCompilationTrustMode::UntrustedSandboxed);
        fs::remove_dir(&symlink_harness.paths.target_root).unwrap();
        symlink(
            &symlink_harness.inputs.workspace_view,
            &symlink_harness.paths.target_root,
        )
        .unwrap();
        assert!(RustCompilationPrivatePaths::inspect(&symlink_harness.paths.run_root).is_err());

        let identity_harness = harness(RustCompilationTrustMode::UntrustedSandboxed);
        fs::remove_file(identity_harness.inputs.workspace_view.join("Cargo.toml")).unwrap();
        fs::remove_dir(&identity_harness.inputs.workspace_view).unwrap();
        fs::create_dir(&identity_harness.inputs.workspace_view).unwrap();
        fs::write(
            identity_harness.inputs.workspace_view.join("Cargo.toml"),
            b"[package]\nname='fixture'\nversion='0.0.0'\n",
        )
        .unwrap();
        assert!(matches!(
            compile_rust_compilation_launch_plan(
                &untrusted_policy(),
                &identity_harness.capabilities,
                &identity_harness.profile,
                &identity_harness.inputs,
                &identity_harness.paths,
                &identity_harness.request,
                None,
            )
            .unwrap_err(),
            RustCompilationTrustError::InputIdentityChanged
        ));
    }

    struct FakeProcessGroup {
        waits: VecDeque<bool>,
        terminated: usize,
        killed: usize,
    }

    impl RustCompilationProcessGroupControl for FakeProcessGroup {
        fn terminate_group(&mut self) -> std::io::Result<()> {
            self.terminated += 1;
            Ok(())
        }

        fn wait_group_empty(&mut self, _timeout: Duration) -> std::io::Result<bool> {
            Ok(self.waits.pop_front().unwrap_or(false))
        }

        fn kill_group(&mut self) -> std::io::Result<()> {
            self.killed += 1;
            Ok(())
        }
    }

    #[test]
    fn cancellation_is_plan_bound_and_escalates_the_complete_group() {
        let harness = harness(RustCompilationTrustMode::UntrustedSandboxed);
        let plan = compile_untrusted(&harness);
        let mut group = FakeProcessGroup {
            waits: VecDeque::from([false, true]),
            terminated: 0,
            killed: 0,
        };
        let receipt = cancel_rust_compilation_process_group(
            &plan,
            &plan.plan_digest,
            RustCompilationCancellationReason::WallTimeExceeded,
            &mut group,
        )
        .unwrap();
        assert_eq!(group.terminated, 1);
        assert_eq!(group.killed, 1);
        assert_eq!(
            receipt.actions,
            vec![
                RustCompilationProcessGroupAction::TerminateGroup,
                RustCompilationProcessGroupAction::WaitGrace,
                RustCompilationProcessGroupAction::KillGroup,
                RustCompilationProcessGroupAction::VerifyGroupEmpty,
            ]
        );

        let mut never_called = FakeProcessGroup {
            waits: VecDeque::new(),
            terminated: 0,
            killed: 0,
        };
        assert!(matches!(
            cancel_rust_compilation_process_group(
                &plan,
                &digest(55),
                RustCompilationCancellationReason::Requested,
                &mut never_called,
            )
            .unwrap_err(),
            RustCompilationTrustError::CancellationBindingMismatch
        ));
        assert_eq!(never_called.terminated, 0);
    }

    #[test]
    fn surviving_process_group_fails_without_a_false_receipt() {
        let harness = harness(RustCompilationTrustMode::UntrustedSandboxed);
        let plan = compile_untrusted(&harness);
        let mut group = FakeProcessGroup {
            waits: VecDeque::from([false, false]),
            terminated: 0,
            killed: 0,
        };
        assert!(matches!(
            cancel_rust_compilation_process_group(
                &plan,
                &plan.plan_digest,
                RustCompilationCancellationReason::Requested,
                &mut group,
            )
            .unwrap_err(),
            RustCompilationTrustError::ProcessGroupSurvived
        ));
    }

    #[test]
    fn sampled_accounting_fails_closed_before_an_untrusted_spawn() {
        let harness = harness(RustCompilationTrustMode::UntrustedSandboxed);
        let plan = compile_untrusted(&harness);
        let launcher = ProviderSandboxLauncher::new(harness.capabilities.clone());
        let error = supervise_rust_compilation(
            &plan,
            &harness.paths,
            &launcher,
            &harness.profile,
            ProviderSandboxLaunchMaterial::DarwinProfile(Path::new("/not-consumed")),
            &RustCompilationCancellationSignal::default(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            RustCompilationTrustError::CompleteAccountingUnavailable
        ));
        assert!(!harness.paths.stdout_path.exists());
        assert!(!harness.paths.stderr_path.exists());
    }

    #[test]
    fn real_trusted_local_supervisor_observes_outputs_artifacts_and_group() {
        let harness = harness_with_cargo(
            RustCompilationTrustMode::TrustedLocal,
            br#"#!/bin/sh
mkdir -p "$CARGO_TARGET_DIR"
printf artifact > "$CARGO_TARGET_DIR/result.bin"
printf hello
printf problem >&2
sleep 1
exit 0
"#,
        );
        let plan = compile_trusted(&harness, limits());
        let launcher = ProviderSandboxLauncher::new(harness.capabilities.clone());
        let receipt = supervise_rust_compilation(
            &plan,
            &harness.paths,
            &launcher,
            &harness.profile,
            ProviderSandboxLaunchMaterial::None,
            &RustCompilationCancellationSignal::default(),
        )
        .unwrap();

        receipt.verify_digest().unwrap();
        assert_eq!(
            receipt.terminal.terminal_state,
            RustCompilationTerminalState::Succeeded
        );
        assert_eq!(
            receipt.terminal.accounting_quality,
            RustCompilationAccountingQuality::SampledDegraded
        );
        assert!(receipt.terminal.process_sample_count > 0);
        assert!(receipt.terminal.usage.peak_process_count > 0);
        assert_eq!(receipt.terminal.usage.stdout_bytes, 5);
        assert_eq!(receipt.terminal.usage.stderr_bytes, 7);
        assert!(receipt.terminal.usage.artifact_bytes >= 8);
        assert_eq!(fs::read(&harness.paths.stdout_path).unwrap(), b"hello");
        assert_eq!(fs::read(&harness.paths.stderr_path).unwrap(), b"problem");
        let (capability, _) = project_rust_compilation_trust_rows(&plan, &receipt).unwrap();
        assert!(capability.available);
        assert!(capability.degraded);
    }

    #[test]
    fn real_supervisor_terminates_group_at_bounded_stdout_prefix() {
        let harness = harness_with_cargo(
            RustCompilationTrustMode::TrustedLocal,
            br#"#!/bin/sh
printf 0123456789abcdef
sleep 30
"#,
        );
        let mut constrained = limits();
        constrained.stdout_bytes = 8;
        let plan = compile_trusted(&harness, constrained);
        let launcher = ProviderSandboxLauncher::new(harness.capabilities.clone());
        let receipt = supervise_rust_compilation(
            &plan,
            &harness.paths,
            &launcher,
            &harness.profile,
            ProviderSandboxLaunchMaterial::None,
            &RustCompilationCancellationSignal::default(),
        )
        .unwrap();

        assert_eq!(
            receipt.terminal.terminal_state,
            RustCompilationTerminalState::ResourceLimit
        );
        assert_eq!(
            receipt.terminal.exceeded_limit,
            Some(RustCompilationLimitKind::StdoutBytes)
        );
        assert_eq!(receipt.terminal.usage.stdout_bytes, 9);
        assert_eq!(fs::metadata(&harness.paths.stdout_path).unwrap().len(), 8);
        assert!(receipt.cancellation_receipt_digest.is_some());
        assert!(receipt.terminal.process_group_empty);
    }

    #[test]
    fn in_place_executable_mutation_invalidates_the_admitted_plan() {
        let harness = harness(RustCompilationTrustMode::TrustedLocal);
        let plan = compile_trusted(&harness, limits());
        let identity_before = path_identity(&harness.inputs.cargo_executable).unwrap();
        fs::write(&harness.inputs.cargo_executable, b"changed in place").unwrap();
        assert_eq!(
            path_identity(&harness.inputs.cargo_executable).unwrap(),
            identity_before
        );
        assert!(matches!(
            plan.verify_digest().unwrap_err(),
            RustCompilationTrustError::InputIdentityChanged
        ));
    }

    #[test]
    fn launcher_receipt_and_relational_projections_are_deterministic_and_bound() {
        let harness = harness(RustCompilationTrustMode::UntrustedSandboxed);
        let plan = compile_untrusted(&harness);
        let first =
            RustCompilationLauncherReceipt::close(&plan, success_observation(), None).unwrap();
        let second =
            RustCompilationLauncherReceipt::close(&plan, success_observation(), None).unwrap();
        assert_eq!(first, second);
        first.verify_digest().unwrap();

        let (capability, provenance) = project_rust_compilation_trust_rows(&plan, &first).unwrap();
        assert!(capability.available);
        assert!(!capability.degraded);
        assert_eq!(provenance.launcher_receipt_digest, first.receipt_digest);
        assert_eq!(
            provenance.build_scripts_policy,
            RustExecutableExtensionPolicy::ExecuteInsideSelectedLauncher
        );
    }

    #[test]
    fn over_limit_or_inconsistent_terminal_observation_cannot_issue_receipt() {
        let harness = harness(RustCompilationTrustMode::UntrustedSandboxed);
        let plan = compile_untrusted(&harness);
        let mut over = success_observation();
        over.usage.stdout_bytes = plan.limits.stdout_bytes + 1;
        assert!(matches!(
            RustCompilationLauncherReceipt::close(&plan, over, None).unwrap_err(),
            RustCompilationTrustError::UnboundedObservedUsage
        ));

        let mut inconsistent = success_observation();
        inconsistent.exit_code = Some(1);
        assert!(matches!(
            RustCompilationLauncherReceipt::close(&plan, inconsistent, None).unwrap_err(),
            RustCompilationTrustError::InconsistentTerminalObservation
        ));
    }
}
