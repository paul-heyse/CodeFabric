//! Deterministic Python analysis-context discovery over immutable workspace inputs.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use thiserror::Error;

use crate::analysis_context::{
    AnalysisContext, AnalysisContextCandidate, AnalysisContextDiscoveryPort,
    AnalysisContextDiscoveryRequest, AnalysisContextError, AnalysisContextKind,
};
use crate::identity::{IdentityDomain, context_set_identity, decode_public_id, encode_public_id};
use crate::snapshot::{SnapshotContextRecord, SnapshotContexts};

const PYTHON_IMPLEMENTATION_PROFILE: &str = "cpython-semantics";
const NAMESPACE_PACKAGE_POLICY: &str = "pep420";
const IMPORT_PRECEDENCE: [&str; 6] = [
    "explicit-stub-roots",
    "workspace-module-roots",
    "workspace-source-roots",
    "authorized-dependency-roots",
    "typeshed-stdlib",
    "typeshed-third-party",
];
const KNOWN_LOCK_FILES: [&str; 4] = ["uv.lock", "poetry.lock", "pdm.lock", "Pipfile.lock"];

/// One immutable file made available to configuration discovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonDiscoveryFile {
    pub file_id: String,
    pub relative_path: String,
    /// Non-authoritative label deliberately excluded from context identity.
    pub display_path: String,
    pub digest: [u8; 32],
    pub contents: Vec<u8>,
}

/// Optional explicit workspace choices with precedence over discovered configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PythonWorkspaceProfile {
    pub python_language_version: Option<String>,
    pub selected_lock_artifact_id: Option<String>,
    pub profile_artifact: Option<PythonContextArtifact>,
}

/// Authorized roots and frozen requirements supplied by workspace registration.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PythonRegisteredInputs {
    pub module_roots: Vec<String>,
    pub source_roots: Vec<String>,
    pub stub_roots: Vec<String>,
    pub dependency_roots: Vec<String>,
    pub frozen_requirement_file_ids: Vec<String>,
    pub authorized_roots: Vec<PythonAuthorizedRoot>,
}

/// Authorized mapping from a discovered workspace-relative root to its application ID.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PythonAuthorizedRoot {
    pub relative_path: String,
    pub path_id: String,
}

/// Deployment-owned version universe and explicit fallback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonDeploymentProfile {
    pub supported_python_versions: Vec<String>,
    pub default_python_version: String,
}

/// One complete, generation-pinned discovery request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonContextDiscoveryRequest {
    pub workspace_id: String,
    pub source_generation: u64,
    pub project_root_path: String,
    pub project_root_id: String,
    pub platform_tag: String,
    pub files: Vec<PythonDiscoveryFile>,
    pub workspace_profile: Option<PythonWorkspaceProfile>,
    pub registered: PythonRegisteredInputs,
    pub deployment: PythonDeploymentProfile,
    pub typeshed_bundle_digest: [u8; 32],
    pub pyrefly_bundle_digest: [u8; 32],
    pub ruff_bundle_digest: [u8; 32],
    pub provider_bundle_version: String,
}

/// Python implementation of the lane-neutral analysis-context discovery port.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonContextDiscoveryAdapter {
    template: PythonContextDiscoveryRequest,
}

impl PythonContextDiscoveryAdapter {
    /// Bind authorized immutable discovery inputs to the shared lane-neutral port.
    #[must_use]
    pub const fn new(template: PythonContextDiscoveryRequest) -> Self {
        Self { template }
    }

    /// Return the complete Python product retained by the lane implementation.
    ///
    /// # Errors
    ///
    /// Fails when the shared request does not name every immutable template input or when
    /// ordinary Python discovery fails.
    pub fn discover_product(
        &self,
        request: &AnalysisContextDiscoveryRequest,
    ) -> Result<PythonContextDiscoveryProduct, PythonContextDiscoveryError> {
        let visible = request
            .source_paths
            .iter()
            .map(Vec::as_slice)
            .collect::<BTreeSet<_>>();
        if self
            .template
            .files
            .iter()
            .any(|file| !visible.contains(file.relative_path.as_bytes()))
        {
            return Err(PythonContextDiscoveryError::terminal(
                "CONTEXT_DISCOVERY_VIEW_INCOMPLETE",
                "lane-neutral source inventory omits a configured discovery input",
            ));
        }
        let mut lane_request = self.template.clone();
        lane_request.workspace_id.clone_from(&request.workspace_id);
        lane_request.source_generation = request.source_generation;
        discover_python_context(&lane_request)
    }
}

impl AnalysisContextDiscoveryPort for PythonContextDiscoveryAdapter {
    type Error = PythonContextDiscoveryError;

    fn discover(
        &self,
        request: &AnalysisContextDiscoveryRequest,
    ) -> Result<Vec<AnalysisContextCandidate>, Self::Error> {
        let product = self.discover_product(request)?;
        Ok(vec![AnalysisContextCandidate {
            context_kind: AnalysisContextKind::Python,
            provider_bundle_version: product.context.provider_bundle_version.clone(),
            compiler_or_language_version: product.context.compiler_or_language_version.clone(),
            configuration_manifest_uri: product.context.configuration_manifest_uri.clone(),
            manifest_fingerprint: Some(product.context.fingerprint_bytes()?),
            active: true,
        }])
    }
}

/// Content-addressed artifact retained by a Python context manifest.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PythonContextArtifact {
    pub file_id: String,
    pub digest: String,
}

/// The complete compatibility-sensitive Python context identity authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PythonAnalysisContextManifest {
    pub context_kind: AnalysisContextKind,
    pub python_language_version: String,
    pub implementation_profile: String,
    pub platform_tag: String,
    pub module_roots: Vec<String>,
    pub source_roots: Vec<String>,
    pub stub_roots: Vec<String>,
    pub dependency_roots: Vec<String>,
    pub namespace_package_policy: String,
    pub import_precedence: Vec<String>,
    pub typeshed_bundle_digest: String,
    pub lockfile_artifacts: Vec<PythonContextArtifact>,
    pub project_config_artifacts: Vec<PythonContextArtifact>,
    pub pyrefly_bundle_digest: String,
    pub ruff_bundle_digest: String,
}

/// Why a configuration artifact participates in invalidation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PythonConfigurationDependencyReason {
    WorkspaceProfile,
    PyreflyConfiguration,
    ProjectMetadata,
    LockSystemCandidate,
    SelectedDependencyLock,
}

/// One exact configuration dependency and its observed content identity.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PythonConfigurationDependency {
    pub file_id: String,
    pub digest: String,
    pub reason: PythonConfigurationDependencyReason,
}

/// Complete invalidation dependency product for one context discovery.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PythonConfigurationDependencySet {
    pub source_generation: u64,
    pub dependencies: Vec<PythonConfigurationDependency>,
    pub dependency_set_digest: String,
}

/// Bounded context discovery diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonContextDiagnostic {
    pub code: &'static str,
    pub terminal: bool,
    pub detail: String,
}

/// A fully validated Python context discovery result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonContextDiscoveryProduct {
    pub manifest: PythonAnalysisContextManifest,
    pub canonical_manifest: Vec<u8>,
    pub context_manifest_digest: String,
    pub context: AnalysisContext,
    pub configuration_dependencies: PythonConfigurationDependencySet,
    pub diagnostics: Vec<PythonContextDiagnostic>,
    pub source_generation: u64,
}

/// Terminal Python context discovery failures.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PythonContextDiscoveryError {
    #[error("terminal Python context diagnostic: {0:?}")]
    Terminal(PythonContextDiagnostic),
    #[error(transparent)]
    AnalysisContext(#[from] AnalysisContextError),
}

impl PythonContextDiscoveryError {
    fn terminal(code: &'static str, detail: impl Into<String>) -> Self {
        Self::Terminal(PythonContextDiagnostic {
            code,
            terminal: true,
            detail: detail.into(),
        })
    }

    /// Stable diagnostic code suitable for operational status.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Terminal(diagnostic) => diagnostic.code,
            Self::AnalysisContext(_) => "CONTEXT_IDENTITY_INVALID",
        }
    }
}

impl From<crate::identity::IdentityError> for PythonContextDiscoveryError {
    fn from(error: crate::identity::IdentityError) -> Self {
        Self::AnalysisContext(AnalysisContextError::Identity(error))
    }
}

impl PythonContextDiscoveryProduct {
    /// Rebuild the canonical manifest and application-owned identities.
    ///
    /// # Errors
    ///
    /// Rejects drift in the manifest, public context, or dependency set.
    pub fn validate(&self) -> Result<(), PythonContextDiscoveryError> {
        let canonical_manifest = canonical_json(&self.manifest)?;
        let fingerprint = crate::integrity::digest_bytes(&canonical_manifest);
        if canonical_manifest != self.canonical_manifest
            || self.context_manifest_digest != digest_string(&fingerprint)
            || self.context.context_fingerprint != self.context_manifest_digest
            || self.context.context_kind != AnalysisContextKind::Python
            || self.context.compiler_or_language_version != self.manifest.python_language_version
        {
            return Err(PythonContextDiscoveryError::terminal(
                "CONTEXT_IDENTITY_MISMATCH",
                "Python context product does not match its canonical manifest",
            ));
        }
        self.context.validate()?;
        let dependency_digest = dependency_set_digest(
            self.configuration_dependencies.source_generation,
            &self.configuration_dependencies.dependencies,
        )?;
        if dependency_digest != self.configuration_dependencies.dependency_set_digest {
            return Err(PythonContextDiscoveryError::terminal(
                "CONTEXT_DEPENDENCY_SET_MISMATCH",
                "configuration dependency set digest drifted",
            ));
        }
        Ok(())
    }

    /// Build snapshot selection with this context as the sole Python default.
    ///
    /// # Errors
    ///
    /// Returns an error for a malformed workspace or context identity.
    pub fn snapshot_contexts(
        &self,
        capability_partition_digest: String,
    ) -> Result<SnapshotContexts, PythonContextDiscoveryError> {
        self.validate()?;
        let workspace =
            decode_public_id(IdentityDomain::Workspace, None, &self.context.workspace_id)?;
        let context = decode_public_id(
            IdentityDomain::AnalysisContext,
            None,
            &self.context.analysis_context_id,
        )?;
        let set = context_set_identity(workspace, &[context])?;
        Ok(SnapshotContexts {
            context_set_id: encode_public_id(IdentityDomain::ContextSet, None, set.id)?,
            default_python_context_id: Some(self.context.analysis_context_id.clone()),
            default_rust_context_id: None,
            records: vec![SnapshotContextRecord {
                analysis_context_id: self.context.analysis_context_id.clone(),
                context_manifest_digest: self.context_manifest_digest.clone(),
                capability_partition_digest,
            }],
        })
    }
}

/// Semantic families invalidated by a Python project/configuration transition.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PythonContextDependentFamily {
    ModuleResolution,
    CrossModuleReferences,
    Types,
    CallTargets,
}

/// Exact lifecycle decision for an old/new Python context pair.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythonContextInvalidationPlan {
    pub previous_context_id: String,
    pub selected_context_id: String,
    pub source_generation: u64,
    pub source_and_syntax_preserved: bool,
    pub invalidated_families: BTreeSet<PythonContextDependentFamily>,
    pub republish_required: bool,
}

/// Compute the LIFE 7.5 invalidation closure for one rediscovery.
#[must_use]
pub fn plan_python_context_transition(
    previous: &PythonContextDiscoveryProduct,
    selected: &PythonContextDiscoveryProduct,
) -> PythonContextInvalidationPlan {
    let changed = previous.context.analysis_context_id != selected.context.analysis_context_id;
    PythonContextInvalidationPlan {
        previous_context_id: previous.context.analysis_context_id.clone(),
        selected_context_id: selected.context.analysis_context_id.clone(),
        source_generation: selected.source_generation,
        source_and_syntax_preserved: true,
        invalidated_families: if changed {
            [
                PythonContextDependentFamily::ModuleResolution,
                PythonContextDependentFamily::CrossModuleReferences,
                PythonContextDependentFamily::Types,
                PythonContextDependentFamily::CallTargets,
            ]
            .into_iter()
            .collect()
        } else {
            BTreeSet::new()
        },
        republish_required: changed,
    }
}

/// Discover one deterministic Python analysis context without process-environment inference.
///
/// # Errors
///
/// Returns a terminal diagnostic for malformed inputs, ambiguous locks, conflicting versions,
/// or unsupported configuration. No error path guesses a context.
pub fn discover_python_context(
    request: &PythonContextDiscoveryRequest,
) -> Result<PythonContextDiscoveryProduct, PythonContextDiscoveryError> {
    decode_public_id(IdentityDomain::Workspace, None, &request.workspace_id)?;
    validate_request(request)?;
    let files = validate_files(&request.files)?;
    let pyproject_path = project_path(&request.project_root_path, "pyproject.toml");
    let pyrefly_path = project_path(&request.project_root_path, "pyrefly.toml");
    let pyproject_file = files.get(&pyproject_path).copied();
    let pyrefly_file = files.get(&pyrefly_path).copied();
    let pyproject = pyproject_file
        .map(|file| parse_toml(file, "pyproject.toml"))
        .transpose()?;
    let pyrefly = pyrefly_file
        .map(|file| parse_toml(file, "pyrefly.toml"))
        .transpose()?;

    let mut diagnostics = Vec::new();
    let language_version = resolve_python_version(
        request.workspace_profile.as_ref(),
        pyrefly.as_ref(),
        pyproject.as_ref(),
        &request.deployment,
        &mut diagnostics,
    )?;
    let (lock_artifacts, lock_dependencies) = resolve_lock_artifacts(request, &files)?;
    let configured_roots = configured_package_roots(request, pyrefly.as_ref(), pyproject.as_ref())?;
    let (manifest, dependencies) = assemble_manifest(
        ManifestAssembly {
            request,
            language_version: &language_version,
            pyrefly_file,
            pyproject_file,
            lock_artifacts,
            lock_dependencies,
            configured_roots,
        },
        &mut diagnostics,
    )?;
    let canonical_manifest = canonical_json(&manifest)?;
    let manifest_fingerprint = crate::integrity::digest_bytes(&canonical_manifest);
    let context = AnalysisContext::new_from_manifest_fingerprint(
        &request.workspace_id,
        AnalysisContextKind::Python,
        request.provider_bundle_version.clone(),
        language_version,
        manifest_fingerprint,
        true,
    )?;
    let dependency_set_digest = dependency_set_digest(request.source_generation, &dependencies)?;
    let product = PythonContextDiscoveryProduct {
        manifest,
        canonical_manifest,
        context_manifest_digest: digest_string(&manifest_fingerprint),
        context,
        configuration_dependencies: PythonConfigurationDependencySet {
            source_generation: request.source_generation,
            dependencies,
            dependency_set_digest,
        },
        diagnostics,
        source_generation: request.source_generation,
    };
    product.validate()?;
    Ok(product)
}

struct ManifestAssembly<'a> {
    request: &'a PythonContextDiscoveryRequest,
    language_version: &'a str,
    pyrefly_file: Option<&'a PythonDiscoveryFile>,
    pyproject_file: Option<&'a PythonDiscoveryFile>,
    lock_artifacts: Vec<PythonContextArtifact>,
    lock_dependencies: Vec<PythonConfigurationDependency>,
    configured_roots: Vec<String>,
}

fn assemble_manifest(
    input: ManifestAssembly<'_>,
    diagnostics: &mut Vec<PythonContextDiagnostic>,
) -> Result<
    (
        PythonAnalysisContextManifest,
        Vec<PythonConfigurationDependency>,
    ),
    PythonContextDiscoveryError,
> {
    let request = input.request;
    let mut project_config_artifacts = Vec::new();
    let mut dependencies = Vec::new();
    if let Some(profile) = &request.workspace_profile
        && let Some(artifact) = &profile.profile_artifact
    {
        validate_artifact(artifact, "workspace profile")?;
        project_config_artifacts.push(artifact.clone());
        dependencies.push(PythonConfigurationDependency {
            file_id: artifact.file_id.clone(),
            digest: artifact.digest.clone(),
            reason: PythonConfigurationDependencyReason::WorkspaceProfile,
        });
    }
    for (file, reason) in [
        (
            input.pyrefly_file,
            PythonConfigurationDependencyReason::PyreflyConfiguration,
        ),
        (
            input.pyproject_file,
            PythonConfigurationDependencyReason::ProjectMetadata,
        ),
    ] {
        if let Some(file) = file {
            let artifact = file_artifact(file);
            project_config_artifacts.push(artifact.clone());
            dependencies.push(PythonConfigurationDependency {
                file_id: artifact.file_id,
                digest: artifact.digest,
                reason,
            });
        }
    }
    dependencies.extend(input.lock_dependencies);
    dependencies.sort();
    dependencies.dedup();
    let module_root_candidates = if request.registered.module_roots.is_empty() {
        input.configured_roots.clone()
    } else {
        request.registered.module_roots.clone()
    };
    let source_root_candidates = if request.registered.source_roots.is_empty() {
        input.configured_roots
    } else {
        request.registered.source_roots.clone()
    };
    let module_roots = roots_or_default(
        &module_root_candidates,
        &request.project_root_id,
        "module roots",
        diagnostics,
    )?;
    let source_roots = roots_or_default(
        &source_root_candidates,
        &request.project_root_id,
        "source roots",
        diagnostics,
    )?;
    Ok((
        PythonAnalysisContextManifest {
            context_kind: AnalysisContextKind::Python,
            python_language_version: input.language_version.to_owned(),
            implementation_profile: PYTHON_IMPLEMENTATION_PROFILE.to_owned(),
            platform_tag: request.platform_tag.clone(),
            module_roots,
            source_roots,
            stub_roots: validate_ordered_ids(&request.registered.stub_roots, "stub roots")?,
            dependency_roots: validate_ordered_ids(
                &request.registered.dependency_roots,
                "dependency roots",
            )?,
            namespace_package_policy: NAMESPACE_PACKAGE_POLICY.to_owned(),
            import_precedence: IMPORT_PRECEDENCE.iter().map(ToString::to_string).collect(),
            typeshed_bundle_digest: digest_string(&request.typeshed_bundle_digest),
            lockfile_artifacts: input.lock_artifacts,
            project_config_artifacts,
            pyrefly_bundle_digest: digest_string(&request.pyrefly_bundle_digest),
            ruff_bundle_digest: digest_string(&request.ruff_bundle_digest),
        },
        dependencies,
    ))
}

fn validate_request(
    request: &PythonContextDiscoveryRequest,
) -> Result<(), PythonContextDiscoveryError> {
    if request.project_root_id.is_empty()
        || request.platform_tag.is_empty()
        || request.provider_bundle_version.is_empty()
        || !valid_relative_path(&request.project_root_path)
    {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_REQUEST_INVALID",
            "project root, platform, and provider bundle must be explicit",
        ));
    }
    let mut paths = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for root in &request.registered.authorized_roots {
        if !valid_relative_path(&root.relative_path)
            || root.path_id.is_empty()
            || !paths.insert(root.relative_path.as_str())
            || !ids.insert(root.path_id.as_str())
        {
            return Err(PythonContextDiscoveryError::terminal(
                "CONTEXT_ROOTS_INVALID",
                "authorized root mappings contain an invalid path, ID, or duplicate",
            ));
        }
    }
    Ok(())
}

fn validate_files(
    inputs: &[PythonDiscoveryFile],
) -> Result<BTreeMap<String, &PythonDiscoveryFile>, PythonContextDiscoveryError> {
    let mut files = BTreeMap::new();
    let mut file_ids = BTreeSet::new();
    for file in inputs {
        if file.file_id.is_empty()
            || !valid_relative_path(&file.relative_path)
            || crate::integrity::digest_bytes(&file.contents) != file.digest
        {
            return Err(PythonContextDiscoveryError::terminal(
                "CONTEXT_INPUT_INVALID",
                format!("invalid immutable discovery input {}", file.relative_path),
            ));
        }
        if files.insert(file.relative_path.clone(), file).is_some()
            || !file_ids.insert(file.file_id.clone())
        {
            return Err(PythonContextDiscoveryError::terminal(
                "CONTEXT_INPUT_CONFLICT",
                "duplicate discovery path or file identity",
            ));
        }
    }
    Ok(files)
}

fn parse_toml(
    file: &PythonDiscoveryFile,
    kind: &str,
) -> Result<toml::Value, PythonContextDiscoveryError> {
    let text = std::str::from_utf8(&file.contents).map_err(|_| {
        PythonContextDiscoveryError::terminal(
            "CONTEXT_CONFIG_INVALID",
            format!("{kind} is not UTF-8"),
        )
    })?;
    toml::from_str(text).map_err(|error| {
        PythonContextDiscoveryError::terminal(
            "CONTEXT_CONFIG_INVALID",
            format!("{kind} could not be parsed: {error}"),
        )
    })
}

fn resolve_python_version(
    profile: Option<&PythonWorkspaceProfile>,
    pyrefly: Option<&toml::Value>,
    pyproject: Option<&toml::Value>,
    deployment: &PythonDeploymentProfile,
    diagnostics: &mut Vec<PythonContextDiagnostic>,
) -> Result<String, PythonContextDiscoveryError> {
    let supported = supported_versions(deployment)?;
    if let Some(version) = profile.and_then(|value| value.python_language_version.as_deref()) {
        return supported_exact(version, &supported, "workspace profile");
    }

    let standalone = pyrefly.map(pyrefly_version).transpose()?.flatten();
    let project = pyproject
        .and_then(|document| document.get("tool"))
        .and_then(|tool| tool.get("pyrefly"))
        .map(pyrefly_version)
        .transpose()?
        .flatten();
    if let (Some(left), Some(right)) = (&standalone, &project)
        && left != right
    {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_VERSION_CONFLICT",
            "pyrefly.toml and [tool.pyrefly] select different Python versions",
        ));
    }
    if let Some(version) = standalone.or(project) {
        return supported_exact(&version, &supported, "Pyrefly configuration");
    }

    let requires_python = pyproject
        .and_then(|document| document.get("project"))
        .and_then(|project| project.get("requires-python"))
        .and_then(toml::Value::as_str);
    if let Some(specifier) = requires_python {
        if !specifier_supported(specifier) {
            return Err(PythonContextDiscoveryError::terminal(
                "CONTEXT_VERSION_UNKNOWN",
                format!("unsupported requires-python specifier {specifier}"),
            ));
        }
        let candidates = supported
            .iter()
            .filter(|(minor, _)| version_satisfies(**minor, specifier).unwrap_or(false))
            .map(|(_, value)| value.clone())
            .collect::<Vec<_>>();
        if candidates.len() == 1 {
            return Ok(candidates[0].clone());
        }
        let fallback = supported_exact(
            &deployment.default_python_version,
            &supported,
            "deployment default",
        )?;
        let fallback_minor = parse_minor(&fallback)?;
        if candidates.contains(&fallback) && version_satisfies(fallback_minor, specifier)? {
            diagnostics.push(defaulted_diagnostic(format!(
                "requires-python {specifier} admits multiple supported minors; selected deployment default {fallback}"
            )));
            return Ok(fallback);
        }
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_VERSION_UNKNOWN",
            format!("requires-python {specifier} selects no unique supported Python minor"),
        ));
    }

    let fallback = supported_exact(
        &deployment.default_python_version,
        &supported,
        "deployment default",
    )?;
    diagnostics.push(defaulted_diagnostic(format!(
        "no explicit Python version was discovered; selected deployment default {fallback}"
    )));
    Ok(fallback)
}

fn pyrefly_version(document: &toml::Value) -> Result<Option<String>, PythonContextDiscoveryError> {
    let mut values = [
        document.get("python-version").and_then(toml::Value::as_str),
        document.get("python_version").and_then(toml::Value::as_str),
        document
            .get("environment")
            .and_then(|value| value.get("python-version"))
            .and_then(toml::Value::as_str),
        document
            .get("environment")
            .and_then(|value| value.get("python_version"))
            .and_then(toml::Value::as_str),
    ]
    .into_iter()
    .flatten()
    .map(ToOwned::to_owned)
    .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    match values.as_slice() {
        [] => Ok(None),
        [value] => Ok(Some(value.clone())),
        _ => Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_VERSION_CONFLICT",
            "one Pyrefly configuration contains conflicting Python-version aliases",
        )),
    }
}

fn resolve_lock_artifacts(
    request: &PythonContextDiscoveryRequest,
    files: &BTreeMap<String, &PythonDiscoveryFile>,
) -> Result<
    (
        Vec<PythonContextArtifact>,
        Vec<PythonConfigurationDependency>,
    ),
    PythonContextDiscoveryError,
> {
    let mut candidates = KNOWN_LOCK_FILES
        .iter()
        .filter_map(|name| {
            files
                .get(&project_path(&request.project_root_path, name))
                .copied()
        })
        .collect::<Vec<_>>();
    let by_id = files
        .values()
        .map(|file| (file.file_id.as_str(), *file))
        .collect::<BTreeMap<_, _>>();
    for file_id in &request.registered.frozen_requirement_file_ids {
        let file = by_id.get(file_id.as_str()).copied().ok_or_else(|| {
            PythonContextDiscoveryError::terminal(
                "CONTEXT_DEPENDENCY_INPUT_MISSING",
                format!("registered frozen requirements {file_id} are unavailable"),
            )
        })?;
        candidates.push(file);
    }
    candidates.sort_by(|left, right| left.file_id.cmp(&right.file_id));
    candidates.dedup_by(|left, right| left.file_id == right.file_id);
    let selected_id = request
        .workspace_profile
        .as_ref()
        .and_then(|profile| profile.selected_lock_artifact_id.as_deref());
    let selected = match (candidates.as_slice(), selected_id) {
        ([], None) => None,
        ([], Some(file_id)) => {
            return Err(PythonContextDiscoveryError::terminal(
                "CONTEXT_LOCK_SELECTION_INVALID",
                format!("selected dependency lock {file_id} is unavailable"),
            ));
        }
        ([only], None) => Some(*only),
        ([..], None) => {
            return Err(PythonContextDiscoveryError::terminal(
                "CONTEXT_LOCK_CONFLICT",
                "multiple lock systems are present and no workspace profile selects one",
            ));
        }
        (_, Some(file_id)) => Some(
            candidates
                .iter()
                .find(|file| file.file_id == file_id)
                .copied()
                .ok_or_else(|| {
                    PythonContextDiscoveryError::terminal(
                        "CONTEXT_LOCK_SELECTION_INVALID",
                        format!("selected dependency lock {file_id} is not a recognized candidate"),
                    )
                })?,
        ),
    };
    let mut dependencies = candidates
        .iter()
        .map(|file| PythonConfigurationDependency {
            file_id: file.file_id.clone(),
            digest: digest_string(&file.digest),
            reason: PythonConfigurationDependencyReason::LockSystemCandidate,
        })
        .collect::<Vec<_>>();
    if let Some(file) = selected {
        dependencies.push(PythonConfigurationDependency {
            file_id: file.file_id.clone(),
            digest: digest_string(&file.digest),
            reason: PythonConfigurationDependencyReason::SelectedDependencyLock,
        });
    }
    Ok((
        selected.into_iter().map(file_artifact).collect(),
        dependencies,
    ))
}

fn configured_package_roots(
    request: &PythonContextDiscoveryRequest,
    pyrefly: Option<&toml::Value>,
    pyproject: Option<&toml::Value>,
) -> Result<Vec<String>, PythonContextDiscoveryError> {
    let standalone = pyrefly
        .map(pyrefly_search_paths)
        .transpose()?
        .unwrap_or_default();
    let project_pyrefly = pyproject
        .and_then(|document| document.get("tool"))
        .and_then(|tool| tool.get("pyrefly"))
        .map(pyrefly_search_paths)
        .transpose()?
        .unwrap_or_default();
    if !standalone.is_empty() && !project_pyrefly.is_empty() && standalone != project_pyrefly {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_ROOTS_CONFLICT",
            "pyrefly.toml and [tool.pyrefly] select different search paths",
        ));
    }
    let configured = if !standalone.is_empty() {
        standalone
    } else if !project_pyrefly.is_empty() {
        project_pyrefly
    } else {
        setuptools_package_roots(pyproject)?
    };
    configured
        .iter()
        .map(|path| authorized_root_id(request, path))
        .collect()
}

fn pyrefly_search_paths(
    document: &toml::Value,
) -> Result<Vec<String>, PythonContextDiscoveryError> {
    let hyphenated = optional_string_array(document.get("search-path"), "search-path")?;
    let underscored = optional_string_array(document.get("search_path"), "search_path")?;
    if !hyphenated.is_empty() && !underscored.is_empty() && hyphenated != underscored {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_ROOTS_CONFLICT",
            "Pyrefly search-path aliases disagree",
        ));
    }
    Ok(if hyphenated.is_empty() {
        underscored
    } else {
        hyphenated
    })
}

fn setuptools_package_roots(
    pyproject: Option<&toml::Value>,
) -> Result<Vec<String>, PythonContextDiscoveryError> {
    let value = pyproject
        .and_then(|document| document.get("tool"))
        .and_then(|tool| tool.get("setuptools"))
        .and_then(|setuptools| setuptools.get("packages"))
        .and_then(|packages| packages.get("find"))
        .and_then(|find| find.get("where"));
    optional_string_array(value, "tool.setuptools.packages.find.where")
}

fn optional_string_array(
    value: Option<&toml::Value>,
    field: &str,
) -> Result<Vec<String>, PythonContextDiscoveryError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values = value.as_array().ok_or_else(|| {
        PythonContextDiscoveryError::terminal(
            "CONTEXT_CONFIG_INVALID",
            format!("{field} must be an ordered string array"),
        )
    })?;
    let result = values
        .iter()
        .map(|value| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                PythonContextDiscoveryError::terminal(
                    "CONTEXT_CONFIG_INVALID",
                    format!("{field} contains a non-string root"),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if result.iter().collect::<BTreeSet<_>>().len() != result.len() {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_ROOTS_CONFLICT",
            format!("{field} contains duplicate roots"),
        ));
    }
    Ok(result)
}

fn authorized_root_id(
    request: &PythonContextDiscoveryRequest,
    configured_path: &str,
) -> Result<String, PythonContextDiscoveryError> {
    if !valid_relative_path(configured_path) {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_ROOTS_INVALID",
            format!("configured package root {configured_path} escapes the project"),
        ));
    }
    let path = if matches!(configured_path, "" | ".") {
        request.project_root_path.clone()
    } else {
        project_path(&request.project_root_path, configured_path)
    };
    if path == request.project_root_path {
        return Ok(request.project_root_id.clone());
    }
    request
        .registered
        .authorized_roots
        .iter()
        .find(|root| root.relative_path == path)
        .map(|root| root.path_id.clone())
        .ok_or_else(|| {
            PythonContextDiscoveryError::terminal(
                "CONTEXT_ROOT_UNAUTHORIZED",
                format!("configured package root {path} has no authorized path identity"),
            )
        })
}

fn roots_or_default(
    roots: &[String],
    default: &str,
    name: &str,
    diagnostics: &mut Vec<PythonContextDiagnostic>,
) -> Result<Vec<String>, PythonContextDiscoveryError> {
    if roots.is_empty() {
        diagnostics.push(PythonContextDiagnostic {
            code: "CONTEXT_ROOTS_DEFAULTED",
            terminal: false,
            detail: format!("{name} defaulted to the registered project root"),
        });
        return Ok(vec![default.to_owned()]);
    }
    validate_ordered_ids(roots, name)
}

fn validate_ordered_ids(
    values: &[String],
    name: &str,
) -> Result<Vec<String>, PythonContextDiscoveryError> {
    if values.iter().any(String::is_empty)
        || values.iter().collect::<BTreeSet<_>>().len() != values.len()
    {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_ROOTS_INVALID",
            format!("{name} contain an empty or duplicate identity"),
        ));
    }
    Ok(values.to_vec())
}

fn supported_versions(
    deployment: &PythonDeploymentProfile,
) -> Result<BTreeMap<PythonMinor, String>, PythonContextDiscoveryError> {
    let mut supported = BTreeMap::new();
    for version in &deployment.supported_python_versions {
        let minor = parse_minor(version)?;
        if supported.insert(minor, normalized_minor(minor)).is_some() {
            return Err(PythonContextDiscoveryError::terminal(
                "CONTEXT_DEPLOYMENT_PROFILE_INVALID",
                "deployment profile contains duplicate Python minors",
            ));
        }
    }
    if supported.is_empty() {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_DEPLOYMENT_PROFILE_INVALID",
            "deployment profile has no supported Python versions",
        ));
    }
    Ok(supported)
}

fn supported_exact(
    version: &str,
    supported: &BTreeMap<PythonMinor, String>,
    authority: &str,
) -> Result<String, PythonContextDiscoveryError> {
    let minor = parse_minor(version)?;
    supported.get(&minor).cloned().ok_or_else(|| {
        PythonContextDiscoveryError::terminal(
            "CONTEXT_VERSION_UNSUPPORTED",
            format!(
                "{authority} selected unsupported Python {}",
                normalized_minor(minor)
            ),
        )
    })
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PythonMinor(u16, u16);

fn parse_minor(value: &str) -> Result<PythonMinor, PythonContextDiscoveryError> {
    let parts = value.trim().split('.').collect::<Vec<_>>();
    if !matches!(parts.as_slice(), [_, _] | [_, _, "0"]) {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_VERSION_UNKNOWN",
            format!("Python version {value} is not an exact 3.<minor> version"),
        ));
    }
    let (version, _) = parse_version_bound(value)?;
    if version.0 != 3 {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_VERSION_UNKNOWN",
            format!("Python version {value} is not an exact 3.<minor> version"),
        ));
    }
    Ok(version)
}

fn parse_version_bound(value: &str) -> Result<(PythonMinor, usize), PythonContextDiscoveryError> {
    let parts = value.trim().split('.').collect::<Vec<_>>();
    if !matches!(parts.as_slice(), [_] | [_, _] | [_, _, "0"]) {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_VERSION_UNKNOWN",
            format!("Python version bound {value} is unsupported"),
        ));
    }
    let major = parts[0].parse::<u16>().map_err(|_| {
        PythonContextDiscoveryError::terminal(
            "CONTEXT_VERSION_UNKNOWN",
            format!("Python version bound {value} has an invalid major"),
        )
    })?;
    let minor = parts
        .get(1)
        .map_or(Ok(0), |minor| minor.parse::<u16>())
        .map_err(|_| {
            PythonContextDiscoveryError::terminal(
                "CONTEXT_VERSION_UNKNOWN",
                format!("Python version bound {value} has an invalid minor"),
            )
        })?;
    Ok((PythonMinor(major, minor), parts.len()))
}

fn normalized_minor(version: PythonMinor) -> String {
    format!("{}.{}", version.0, version.1)
}

fn specifier_supported(specifier: &str) -> bool {
    specifier
        .split(',')
        .map(str::trim)
        .all(|clause| parse_specifier_clause(clause).is_ok())
}

fn version_satisfies(
    version: PythonMinor,
    specifier: &str,
) -> Result<bool, PythonContextDiscoveryError> {
    specifier
        .split(',')
        .map(str::trim)
        .try_fold(true, |accepted, clause| {
            let (operator, required, wildcard, precision) = parse_specifier_clause(clause)?;
            let matches = match operator {
                "==" if wildcard && precision == 1 => version.0 == required.0,
                "==" => version == required,
                ">=" => version >= required,
                ">" => version > required,
                "<=" => version <= required,
                "<" => version < required,
                "~=" => {
                    version >= required
                        && if precision >= 3 {
                            version == required
                        } else {
                            version.0 == required.0
                        }
                }
                _ => false,
            };
            Ok(accepted && matches)
        })
}

fn parse_specifier_clause(
    clause: &str,
) -> Result<(&str, PythonMinor, bool, usize), PythonContextDiscoveryError> {
    let operator = [">=", "<=", "==", "~=", ">", "<"]
        .into_iter()
        .find(|operator| clause.starts_with(operator))
        .ok_or_else(|| {
            PythonContextDiscoveryError::terminal(
                "CONTEXT_VERSION_UNKNOWN",
                format!("unsupported requires-python clause {clause}"),
            )
        })?;
    let mut value = clause[operator.len()..].trim();
    let wildcard = value.ends_with(".*");
    if wildcard {
        value = &value[..value.len() - 2];
    }
    if wildcard && operator != "==" {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_VERSION_UNKNOWN",
            format!("wildcard is unsupported for {operator}"),
        ));
    }
    let (version, precision) = parse_version_bound(value)?;
    Ok((operator, version, wildcard, precision))
}

fn file_artifact(file: &PythonDiscoveryFile) -> PythonContextArtifact {
    PythonContextArtifact {
        file_id: file.file_id.clone(),
        digest: digest_string(&file.digest),
    }
}

fn validate_artifact(
    artifact: &PythonContextArtifact,
    kind: &str,
) -> Result<(), PythonContextDiscoveryError> {
    if artifact.file_id.is_empty() || !valid_digest(&artifact.digest) {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_INPUT_INVALID",
            format!("{kind} artifact is malformed"),
        ));
    }
    Ok(())
}

fn dependency_set_digest(
    source_generation: u64,
    dependencies: &[PythonConfigurationDependency],
) -> Result<String, PythonContextDiscoveryError> {
    #[derive(Serialize)]
    struct DigestView<'a> {
        source_generation: u64,
        dependencies: &'a [PythonConfigurationDependency],
    }
    let bytes = canonical_json(&DigestView {
        source_generation,
        dependencies,
    })?;
    Ok(digest_string(&crate::integrity::digest_bytes(&bytes)))
}

fn canonical_json<T: Serialize>(value: &T) -> Result<Vec<u8>, PythonContextDiscoveryError> {
    let value = serde_json::to_value(value).map_err(|error| {
        PythonContextDiscoveryError::terminal("CONTEXT_CANONICALIZATION_FAILED", error.to_string())
    })?;
    crate::contracts::jcs::canonicalize_value(&value).map_err(|error| {
        PythonContextDiscoveryError::terminal("CONTEXT_CANONICALIZATION_FAILED", error.to_string())
    })
}

fn defaulted_diagnostic(detail: String) -> PythonContextDiagnostic {
    PythonContextDiagnostic {
        code: "CONTEXT_DEFAULTED",
        terminal: false,
        detail,
    }
}

fn project_path(root: &str, name: &str) -> String {
    if matches!(root, "" | ".") {
        name.to_owned()
    } else {
        format!("{root}/{name}")
    }
}

fn valid_relative_path(path: &str) -> bool {
    path == "."
        || (!path.is_empty()
            && !path.starts_with('/')
            && !path.contains('\\')
            && path
                .split('/')
                .all(|component| !matches!(component, "" | "." | "..")))
}

fn valid_digest(value: &str) -> bool {
    value.strip_prefix("b3:").is_some_and(|payload| {
        payload.len() == 64
            && payload
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn digest_string(value: &[u8; 32]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(67);
    output.push_str("b3:");
    for byte in value {
        write!(output, "{byte:02x}").expect("writing to String is infallible");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_id() -> String {
        encode_public_id(IdentityDomain::Workspace, None, [0x21; 16]).unwrap()
    }

    fn file(path: &str, file_id: &str, contents: &str) -> PythonDiscoveryFile {
        PythonDiscoveryFile {
            file_id: file_id.to_owned(),
            relative_path: path.to_owned(),
            display_path: format!("display/{path}"),
            digest: crate::integrity::digest_bytes(contents.as_bytes()),
            contents: contents.as_bytes().to_vec(),
        }
    }

    fn base_request() -> PythonContextDiscoveryRequest {
        PythonContextDiscoveryRequest {
            workspace_id: workspace_id(),
            source_generation: 7,
            project_root_path: ".".to_owned(),
            project_root_id: "path:project-root".to_owned(),
            platform_tag: "macos-aarch64".to_owned(),
            files: vec![
                file(
                    "pyproject.toml",
                    "file:pyproject",
                    "[project]\nname='fixture'\nrequires-python='>=3.12,<3.13'\n[tool.setuptools.packages.find]\nwhere=['src']\n",
                ),
                file("src/pkg/__init__.py", "file:package", "VALUE = 1\n"),
            ],
            workspace_profile: None,
            registered: PythonRegisteredInputs {
                authorized_roots: vec![PythonAuthorizedRoot {
                    relative_path: "src".to_owned(),
                    path_id: "path:src-root".to_owned(),
                }],
                ..PythonRegisteredInputs::default()
            },
            deployment: PythonDeploymentProfile {
                supported_python_versions: vec!["3.12".to_owned(), "3.13".to_owned()],
                default_python_version: "3.13".to_owned(),
            },
            typeshed_bundle_digest: [0x31; 32],
            pyrefly_bundle_digest: [0x32; 32],
            ruff_bundle_digest: [0x33; 32],
            provider_bundle_version: "python-providers-v1".to_owned(),
        }
    }

    #[test]
    fn py_context_discovery_conformance() {
        assert!(version_satisfies(PythonMinor(3, 13), ">=3.12,<4").unwrap());
        assert!(!version_satisfies(PythonMinor(3, 13), "==3.12.*").unwrap());
        assert!(!version_satisfies(PythonMinor(3, 13), "~=3.12.0").unwrap());
        let base = base_request();
        let pyproject_only = discover_python_context(&base).unwrap();
        assert_eq!(pyproject_only.manifest.python_language_version, "3.12");
        assert_eq!(pyproject_only.manifest.module_roots, ["path:src-root"]);
        assert_eq!(
            pyproject_only.manifest.namespace_package_policy,
            NAMESPACE_PACKAGE_POLICY
        );
        assert!(pyproject_only.context.validate().is_ok());
        let shared_request = AnalysisContextDiscoveryRequest {
            workspace_id: base.workspace_id.clone(),
            source_generation: base.source_generation,
            source_paths: base
                .files
                .iter()
                .map(|file| file.relative_path.as_bytes().to_vec())
                .collect(),
        };
        let candidates = PythonContextDiscoveryAdapter::new(base)
            .discover(&shared_request)
            .unwrap();
        let materialized =
            crate::analysis_context::materialize_discovered_contexts(&shared_request, candidates)
                .unwrap();
        assert_eq!(
            materialized[0].analysis_context_id,
            pyproject_only.context.analysis_context_id
        );

        let mut pyrefly = base_request();
        pyrefly.files[0] = file(
            "pyproject.toml",
            "file:pyproject",
            "[project]\nname='fixture'\n",
        );
        pyrefly.files.push(file(
            "pyrefly.toml",
            "file:pyrefly",
            "python-version='3.13'\nsearch-path=['src']\n",
        ));
        let pyrefly = discover_python_context(&pyrefly).unwrap();
        assert_eq!(pyrefly.manifest.python_language_version, "3.13");
        assert_eq!(pyrefly.manifest.project_config_artifacts.len(), 2);

        let mut multi_lock = base_request();
        multi_lock
            .files
            .push(file("uv.lock", "file:uv-lock", "version = 1\n"));
        multi_lock
            .files
            .push(file("poetry.lock", "file:poetry-lock", "package = []\n"));
        multi_lock.workspace_profile = Some(PythonWorkspaceProfile {
            python_language_version: None,
            selected_lock_artifact_id: Some("file:uv-lock".to_owned()),
            profile_artifact: Some(PythonContextArtifact {
                file_id: "file:workspace-profile".to_owned(),
                digest: digest_string(&[0x44; 32]),
            }),
        });
        let multi_lock = discover_python_context(&multi_lock).unwrap();
        assert_eq!(
            multi_lock.manifest.lockfile_artifacts,
            [PythonContextArtifact {
                file_id: "file:uv-lock".to_owned(),
                digest: digest_string(&crate::integrity::digest_bytes(b"version = 1\n")),
            }]
        );
        assert_eq!(
            multi_lock
                .configuration_dependencies
                .dependencies
                .iter()
                .filter(|dependency| {
                    dependency.reason == PythonConfigurationDependencyReason::LockSystemCandidate
                })
                .count(),
            2
        );

        let mut namespace = base_request();
        namespace.registered.module_roots = vec!["path:namespace-root".to_owned()];
        namespace.registered.source_roots = vec!["path:namespace-root".to_owned()];
        let namespace = discover_python_context(&namespace).unwrap();
        assert_eq!(namespace.manifest.module_roots, ["path:namespace-root"]);
        assert_eq!(
            namespace.manifest.import_precedence,
            IMPORT_PRECEDENCE.map(str::to_owned)
        );
    }

    #[test]
    fn py_context_manifest_identity_parity() {
        let request = base_request();
        let product = discover_python_context(&request).unwrap();
        let value = serde_json::to_value(&product.manifest).unwrap();
        let keys = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let expected = [
            "context_kind",
            "python_language_version",
            "implementation_profile",
            "platform_tag",
            "module_roots",
            "source_roots",
            "stub_roots",
            "dependency_roots",
            "namespace_package_policy",
            "import_precedence",
            "typeshed_bundle_digest",
            "lockfile_artifacts",
            "project_config_artifacts",
            "pyrefly_bundle_digest",
            "ruff_bundle_digest",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(keys, expected);

        for key in expected
            .iter()
            .copied()
            .filter(|key| *key != "context_kind")
        {
            let mut changed = value.clone();
            let field = &mut changed[key];
            match field {
                serde_json::Value::String(value) => value.push_str("-changed"),
                serde_json::Value::Array(values) => {
                    values.push(serde_json::json!({"identity_probe": key}));
                }
                _ => panic!("manifest field {key} has an untested representation"),
            }
            let canonical = crate::contracts::jcs::canonicalize_value(&changed).unwrap();
            let fingerprint = crate::integrity::digest_bytes(&canonical);
            let changed_context = AnalysisContext::new_from_manifest_fingerprint(
                &request.workspace_id,
                AnalysisContextKind::Python,
                request.provider_bundle_version.clone(),
                product.manifest.python_language_version.clone(),
                fingerprint,
                true,
            )
            .unwrap();
            assert_ne!(
                changed_context.analysis_context_id, product.context.analysis_context_id,
                "manifest field {key} did not affect identity"
            );
        }
        let rust_kind = AnalysisContext::new_from_manifest_fingerprint(
            &request.workspace_id,
            AnalysisContextKind::Rust,
            request.provider_bundle_version.clone(),
            product.manifest.python_language_version.clone(),
            product.context.fingerprint_bytes().unwrap(),
            true,
        )
        .unwrap();
        assert_ne!(
            rust_kind.analysis_context_id,
            product.context.analysis_context_id
        );

        let repeated = discover_python_context(&request).unwrap();
        assert_eq!(repeated, product);
        assert_eq!(
            AnalysisContext::from_json(&serde_json::to_vec(&product.context).unwrap()).unwrap(),
            product.context
        );
        let mut display_only = request;
        for file in &mut display_only.files {
            file.display_path.push_str("-renamed-for-display");
        }
        assert_eq!(
            discover_python_context(&display_only)
                .unwrap()
                .context
                .analysis_context_id,
            product.context.analysis_context_id
        );
    }

    #[test]
    fn py_context_guess_rejection_falsification() {
        let mut locks = base_request();
        locks
            .files
            .push(file("uv.lock", "file:uv-lock", "version = 1\n"));
        locks
            .files
            .push(file("poetry.lock", "file:poetry-lock", "package = []\n"));
        assert_eq!(
            discover_python_context(&locks).unwrap_err().code(),
            "CONTEXT_LOCK_CONFLICT"
        );

        let mut conflicting = base_request();
        conflicting.files[0] = file(
            "pyproject.toml",
            "file:pyproject",
            "[project]\nname='fixture'\n[tool.pyrefly]\npython-version='3.12'\n",
        );
        conflicting.files.push(file(
            "pyrefly.toml",
            "file:pyrefly",
            "python-version='3.13'\n",
        ));
        assert_eq!(
            discover_python_context(&conflicting).unwrap_err().code(),
            "CONTEXT_VERSION_CONFLICT"
        );

        let mut invalid = base_request();
        invalid.files[0] = file(
            "pyproject.toml",
            "file:pyproject",
            "[project\nrequires-python='>=3.12'\n",
        );
        assert_eq!(
            discover_python_context(&invalid).unwrap_err().code(),
            "CONTEXT_CONFIG_INVALID"
        );
    }

    #[test]
    fn py_context_invalidation_operational_gate() {
        let before_request = base_request();
        let before = discover_python_context(&before_request).unwrap();
        let mut after_request = before_request.clone();
        after_request.files[0] = file(
            "pyproject.toml",
            "file:pyproject",
            "[project]\nname='fixture'\nrequires-python='>=3.13,<3.14'\n[tool.setuptools.packages.find]\nwhere=['src']\n",
        );
        let after = discover_python_context(&after_request).unwrap();
        let transition = plan_python_context_transition(&before, &after);
        assert!(transition.source_and_syntax_preserved);
        assert!(transition.republish_required);
        assert_eq!(transition.source_generation, before.source_generation);
        assert_eq!(
            transition.invalidated_families,
            [
                PythonContextDependentFamily::ModuleResolution,
                PythonContextDependentFamily::CrossModuleReferences,
                PythonContextDependentFamily::Types,
                PythonContextDependentFamily::CallTargets,
            ]
            .into_iter()
            .collect()
        );
        assert_ne!(
            transition.previous_context_id,
            transition.selected_context_id
        );

        let snapshot = after.snapshot_contexts(digest_string(&[0x55; 32])).unwrap();
        let previous_snapshot = before
            .snapshot_contexts(digest_string(&[0x55; 32]))
            .unwrap();
        assert_ne!(snapshot.context_set_id, previous_snapshot.context_set_id);
        assert_eq!(
            snapshot.default_python_context_id.as_deref(),
            Some(after.context.analysis_context_id.as_str())
        );
        assert_eq!(
            snapshot.records[0].context_manifest_digest,
            after.context_manifest_digest
        );

        let unchanged = plan_python_context_transition(&after, &after);
        assert!(!unchanged.republish_required);
        assert!(unchanged.invalidated_families.is_empty());
    }
}
