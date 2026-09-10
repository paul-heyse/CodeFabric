//! Deterministic Python analysis-context discovery over immutable workspace inputs.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use thiserror::Error;

use crate::analysis_context::{
    AnalysisContext, AnalysisContextCandidate, AnalysisContextDiscoveryPort,
    AnalysisContextDiscoveryRequest, AnalysisContextError, AnalysisContextKind,
    ContextArtifactInput, ContextLookupEvidence, ContextLookupKind, ContextLookupObservation,
    ContextSearchRoot, ContextSearchScope, PythonContextSettings, PythonLanguageVersion,
    PythonModuleBinding,
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
    /// Authoritative workspace-relative bytes; display text never selects a file.
    pub relative_path: Vec<u8>,
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
    /// A registered build-system map takes precedence over filesystem module derivation.
    pub module_map: Vec<PythonModuleBinding>,
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
    /// Missing external bundle evidence does not prevent the native syntax context.
    pub typeshed_bundle_digest: Option<[u8; 32]>,
    pub pyrefly_bundle_digest: Option<[u8; 32]>,
    pub ruff_bundle_digest: [u8; 32],
    pub provider_bundle_version: String,
    pub search_scope: ContextSearchScope,
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
        if request.workspace_id != self.template.workspace_id
            || request.source_generation != self.template.source_generation
        {
            return Err(PythonContextDiscoveryError::terminal(
                "CONTEXT_DISCOVERY_GENERATION_MISMATCH",
                "immutable context inputs cannot be rebound to another workspace or generation",
            ));
        }
        let visible = request
            .source_paths
            .iter()
            .map(Vec::as_slice)
            .collect::<BTreeSet<_>>();
        if self
            .template
            .files
            .iter()
            .any(|file| !visible.contains(file.relative_path.as_slice()))
        {
            return Err(PythonContextDiscoveryError::terminal(
                "CONTEXT_DISCOVERY_VIEW_INCOMPLETE",
                "lane-neutral source inventory omits a configured discovery input",
            ));
        }
        discover_python_context(&self.template)
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

/// Captured PEP 561 marker consumed by the native resolver, never executed as a module.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PythonTypingMarker {
    pub relative_path: Vec<u8>,
    pub digest: String,
    pub contents: Vec<u8>,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub typing_markers: Vec<PythonTypingMarker>,
    pub namespace_package_policy: String,
    pub import_precedence: Vec<String>,
    pub typeshed_bundle_digest: Option<String>,
    pub lockfile_artifacts: Vec<PythonContextArtifact>,
    pub project_config_artifacts: Vec<PythonContextArtifact>,
    /// Captured checker settings not represented by the effective manifest. Absence retains
    /// compatibility with contexts containing no project configuration artifacts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unapplied_checker_settings: Option<Vec<String>>,
    pub pyrefly_bundle_digest: Option<String>,
    pub ruff_bundle_digest: String,
    pub provider_bundle_version: String,
    pub platforms: Vec<String>,
    pub root_bindings: Vec<ContextSearchRoot>,
    pub module_map: Vec<PythonModuleBinding>,
    pub configuration_namespace: Vec<u8>,
    pub configuration_roots: Vec<ContextSearchRoot>,
    pub configuration_policy_identity: [u8; 32],
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
    pub lookup_evidence: Vec<ContextLookupEvidence>,
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
            &self.configuration_dependencies.lookup_evidence,
        )?;
        validate_lookup_closure(
            &self.manifest,
            &self.configuration_dependencies.lookup_evidence,
        )?;
        if dependency_digest != self.configuration_dependencies.dependency_set_digest
            || self.source_generation != self.configuration_dependencies.source_generation
            || self.context.provider_bundle_version != self.manifest.provider_bundle_version
        {
            return Err(PythonContextDiscoveryError::terminal(
                "CONTEXT_DEPENDENCY_SET_MISMATCH",
                "configuration dependency set digest drifted",
            ));
        }
        Ok(())
    }

    /// Project the selected values a provider must install from the identity-bearing manifest.
    ///
    /// # Errors
    /// Rejects inconsistent identities, malformed selected versions or missing root bindings.
    pub fn effective_settings(&self) -> Result<PythonContextSettings, PythonContextDiscoveryError> {
        self.validate()?;
        let manifest = &self.manifest;
        let PythonMinor(major, minor) = parse_minor(&manifest.python_language_version)?;
        let roots = |ids: &[String]| {
            ids.iter()
                .map(|id| {
                    manifest
                        .root_bindings
                        .iter()
                        .find(|root| &root.root_id == id)
                        .cloned()
                        .ok_or_else(|| {
                            PythonContextDiscoveryError::terminal(
                                "CONTEXT_ROOT_UNAUTHORIZED",
                                "selected root has no immutable binding",
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()
        };
        let artifacts = |values: &[PythonContextArtifact]| {
            values
                .iter()
                .map(|artifact| {
                    Ok(ContextArtifactInput {
                        file_id: artifact.file_id.clone(),
                        digest: parse_digest(&artifact.digest)?,
                    })
                })
                .collect::<Result<Vec<_>, PythonContextDiscoveryError>>()
        };
        Ok(PythonContextSettings {
            language_version: PythonLanguageVersion { major, minor },
            platforms: manifest.platforms.clone(),
            module_roots: roots(&manifest.module_roots)?,
            source_roots: roots(&manifest.source_roots)?,
            stub_roots: roots(&manifest.stub_roots)?,
            dependency_roots: roots(&manifest.dependency_roots)?,
            module_map: manifest.module_map.clone(),
            typeshed_bundle_digest: manifest
                .typeshed_bundle_digest
                .as_deref()
                .map(parse_digest)
                .transpose()?,
            pyrefly_bundle_digest: manifest
                .pyrefly_bundle_digest
                .as_deref()
                .map(parse_digest)
                .transpose()?,
            ruff_bundle_digest: parse_digest(&manifest.ruff_bundle_digest)?,
            provider_bundle_version: manifest.provider_bundle_version.clone(),
            configuration_artifacts: artifacts(&manifest.project_config_artifacts)?,
            lock_artifacts: artifacts(&manifest.lockfile_artifacts)?,
        })
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
    let changed = previous.context.analysis_context_id != selected.context.analysis_context_id
        || previous.configuration_dependencies.lookup_evidence
            != selected.configuration_dependencies.lookup_evidence
        || selected
            .configuration_dependencies
            .lookup_evidence
            .iter()
            .any(|evidence| {
                !evidence.scope.is_closed()
                    || matches!(evidence.observation, ContextLookupObservation::Incomplete)
            });
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
    let lookup_evidence = configuration_lookups(request, &files)?;
    let pyproject_file = first_configuration(request, &files, "pyproject.toml");
    let pyrefly_file = first_configuration(request, &files, "pyrefly.toml");
    let pyproject = pyproject_file
        .map(|file| parse_toml(file, "pyproject.toml"))
        .transpose()?;
    let pyrefly = pyrefly_file
        .map(|file| parse_toml(file, "pyrefly.toml"))
        .transpose()?;

    let mut diagnostics = Vec::new();
    if request.typeshed_bundle_digest.is_none() || request.pyrefly_bundle_digest.is_none() {
        diagnostics.push(PythonContextDiagnostic {
            code: "CONTEXT_EXTERNAL_BUNDLE_UNAVAILABLE", terminal: false,
            detail: "native syntax settings are selected; external semantic bundle inputs are unavailable".to_owned(),
        });
    }
    let language_version = resolve_python_version(
        request.workspace_profile.as_ref(),
        pyrefly.as_ref(),
        pyproject.as_ref(),
        &request.deployment,
        &mut diagnostics,
    )?;
    let (lock_artifacts, lock_dependencies) = resolve_lock_artifacts(request, &files)?;
    let configured_roots = configured_package_roots(
        request,
        pyrefly.as_ref(),
        pyproject.as_ref(),
        pyrefly_file,
        pyproject_file,
    )?;
    let (mut manifest, dependencies) = assemble_manifest(
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
    manifest.platforms = selected_platforms(request, pyrefly.as_ref(), pyproject.as_ref())?;
    let configured_dependencies = configured_dependency_roots(
        request,
        pyrefly.as_ref(),
        pyproject.as_ref(),
        pyrefly_file,
        pyproject_file,
    )?;
    if request.registered.dependency_roots.is_empty() {
        manifest.dependency_roots =
            validate_ordered_ids(&configured_dependencies, "dependency roots")?;
    }
    manifest.typing_markers = captured_typing_markers(&files, &manifest)?;
    if !manifest.project_config_artifacts.is_empty() {
        manifest.unapplied_checker_settings = Some(unapplied_checker_settings(
            request,
            pyrefly.as_ref(),
            pyproject.as_ref(),
        ));
    }
    manifest.module_map = discover_module_map(request, &manifest)?;
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
    let dependency_set_digest =
        dependency_set_digest(request.source_generation, &dependencies, &lookup_evidence)?;
    let product = PythonContextDiscoveryProduct {
        manifest,
        canonical_manifest,
        context_manifest_digest: digest_string(&manifest_fingerprint),
        context,
        configuration_dependencies: PythonConfigurationDependencySet {
            source_generation: request.source_generation,
            dependencies,
            lookup_evidence,
            dependency_set_digest,
        },
        diagnostics,
        source_generation: request.source_generation,
    };
    product.validate()?;
    Ok(product)
}

fn captured_typing_markers(
    files: &BTreeMap<Vec<u8>, &PythonDiscoveryFile>,
    manifest: &PythonAnalysisContextManifest,
) -> Result<Vec<PythonTypingMarker>, PythonContextDiscoveryError> {
    let mut markers = Vec::new();
    let mut marker_bytes = 0_usize;
    for file in files
        .values()
        .filter(|file| file.relative_path.ends_with(b"/py.typed"))
    {
        if !manifest
            .dependency_roots
            .iter()
            .chain(&manifest.stub_roots)
            .any(|id| {
                manifest.root_bindings.iter().any(|root| {
                    root.root_id == *id
                        && (root.relative_path == b"."
                            || file
                                .relative_path
                                .strip_prefix(root.relative_path.as_slice())
                                .is_some_and(|suffix| suffix.starts_with(b"/")))
                })
            })
        {
            continue;
        }
        marker_bytes = marker_bytes.saturating_add(file.contents.len() + file.relative_path.len());
        if file.contents.len() > 4096 || marker_bytes > 128 * 1024 || markers.len() >= 4096 {
            return Err(PythonContextDiscoveryError::terminal(
                "CONTEXT_TYPING_MARKER_BOUND",
                "captured typing markers exceed the context bound",
            ));
        }
        markers.push(PythonTypingMarker {
            relative_path: file.relative_path.clone(),
            digest: digest_string(&file.digest),
            contents: file.contents.clone(),
        });
    }
    Ok(markers)
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
            typing_markers: Vec::new(),
            namespace_package_policy: NAMESPACE_PACKAGE_POLICY.to_owned(),
            import_precedence: IMPORT_PRECEDENCE.iter().map(ToString::to_string).collect(),
            typeshed_bundle_digest: request.typeshed_bundle_digest.as_ref().map(digest_string),
            lockfile_artifacts: input.lock_artifacts,
            project_config_artifacts,
            unapplied_checker_settings: None,
            pyrefly_bundle_digest: request.pyrefly_bundle_digest.as_ref().map(digest_string),
            ruff_bundle_digest: digest_string(&request.ruff_bundle_digest),
            provider_bundle_version: request.provider_bundle_version.clone(),
            platforms: Vec::new(),
            root_bindings: authorized_root_bindings(request),
            module_map: Vec::new(),
            configuration_namespace: request.search_scope.namespace.clone(),
            configuration_roots: request.search_scope.ordered_roots.clone(),
            configuration_policy_identity: request.search_scope.policy_identity,
        },
        dependencies,
    ))
}

fn validate_request(
    request: &PythonContextDiscoveryRequest,
) -> Result<(), PythonContextDiscoveryError> {
    request.search_scope.validate()?;
    if request.files.iter().any(|file| {
        let path = file.relative_path.as_slice();
        let namespace = request.search_scope.namespace.as_slice();
        namespace != b"."
            && path != namespace
            && !path
                .strip_prefix(namespace)
                .is_some_and(|suffix| suffix.starts_with(b"/"))
    }) {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_NAMESPACE_INVALID",
            "immutable context input is outside the authorized namespace",
        ));
    }
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
) -> Result<BTreeMap<Vec<u8>, &PythonDiscoveryFile>, PythonContextDiscoveryError> {
    let mut files = BTreeMap::new();
    let mut file_ids = BTreeSet::new();
    for file in inputs {
        if file.file_id.is_empty()
            || !valid_source_path(&file.relative_path)
            || crate::integrity::digest_bytes(&file.contents) != file.digest
        {
            return Err(PythonContextDiscoveryError::terminal(
                "CONTEXT_INPUT_INVALID",
                format!("invalid immutable discovery input {}", file.file_id),
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

const CONFIGURATION_LOOKUPS: [(&str, ContextLookupKind); 6] = [
    (
        "pyproject.toml",
        ContextLookupKind::PythonProjectConfiguration,
    ),
    ("pyrefly.toml", ContextLookupKind::PyreflyConfiguration),
    ("uv.lock", ContextLookupKind::UvLock),
    ("poetry.lock", ContextLookupKind::PoetryLock),
    ("pdm.lock", ContextLookupKind::PdmLock),
    ("Pipfile.lock", ContextLookupKind::PipfileLock),
];

fn first_configuration<'a>(
    request: &PythonContextDiscoveryRequest,
    files: &BTreeMap<Vec<u8>, &'a PythonDiscoveryFile>,
    name: &str,
) -> Option<&'a PythonDiscoveryFile> {
    request.search_scope.ordered_roots.iter().find_map(|root| {
        let path = std::str::from_utf8(&root.relative_path).ok()?;
        files.get(project_path(path, name).as_bytes()).copied()
    })
}

fn configuration_lookups(
    request: &PythonContextDiscoveryRequest,
    files: &BTreeMap<Vec<u8>, &PythonDiscoveryFile>,
) -> Result<Vec<ContextLookupEvidence>, PythonContextDiscoveryError> {
    let mut result = Vec::new();
    for root in &request.search_scope.ordered_roots {
        let path = std::str::from_utf8(&root.relative_path).map_err(|_| {
            PythonContextDiscoveryError::terminal(
                "CONTEXT_NAMESPACE_UNSUPPORTED",
                "Python configuration root is not UTF-8",
            )
        })?;
        for (name, kind) in CONFIGURATION_LOOKUPS {
            let relative_path = project_path(path, name);
            result.push(lookup_evidence(
                request,
                relative_path.as_bytes(),
                kind,
                files.get(relative_path.as_bytes()).copied(),
            ));
        }
    }
    for id in &request.registered.frozen_requirement_file_ids {
        let file = files
            .values()
            .find(|file| &file.file_id == id)
            .ok_or_else(|| {
                PythonContextDiscoveryError::terminal(
                    "CONTEXT_DEPENDENCY_INPUT_MISSING",
                    "registered frozen requirements are unavailable",
                )
            })?;
        result.push(lookup_evidence(
            request,
            file.relative_path.as_slice(),
            ContextLookupKind::FrozenRequirements,
            Some(file),
        ));
    }
    for evidence in &result {
        evidence.validate()?;
    }
    Ok(result)
}

fn lookup_evidence(
    request: &PythonContextDiscoveryRequest,
    relative_path: &[u8],
    kind: ContextLookupKind,
    file: Option<&PythonDiscoveryFile>,
) -> ContextLookupEvidence {
    ContextLookupEvidence {
        scope: request.search_scope.clone(),
        kind,
        relative_path: relative_path.to_vec(),
        observation: file.map_or_else(
            || {
                if request.search_scope.is_closed() {
                    ContextLookupObservation::Absent
                } else {
                    ContextLookupObservation::Incomplete
                }
            },
            |file| ContextLookupObservation::Present {
                file_id: file.file_id.clone(),
                digest: file.digest,
            },
        ),
    }
}

fn validate_lookup_closure(
    manifest: &PythonAnalysisContextManifest,
    evidence: &[ContextLookupEvidence],
) -> Result<(), PythonContextDiscoveryError> {
    let Some(first) = evidence.first() else {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_LOOKUP_CLOSURE_INVALID",
            "configuration search evidence is absent",
        ));
    };
    let mut observed = BTreeSet::new();
    for item in evidence {
        item.validate()?;
        if item.scope.namespace != manifest.configuration_namespace
            || item.scope.ordered_roots != manifest.configuration_roots
            || item.scope.policy_identity != manifest.configuration_policy_identity
            || item.scope != first.scope
            || !observed.insert((item.kind, item.relative_path.clone()))
        {
            return Err(PythonContextDiscoveryError::terminal(
                "CONTEXT_LOOKUP_CLOSURE_INVALID",
                "lookup support has a different namespace or duplicate key",
            ));
        }
    }
    for root in &manifest.configuration_roots {
        let path = std::str::from_utf8(&root.relative_path).map_err(|_| {
            PythonContextDiscoveryError::terminal(
                "CONTEXT_LOOKUP_CLOSURE_INVALID",
                "invalid configuration root",
            )
        })?;
        for (name, kind) in CONFIGURATION_LOOKUPS {
            if !observed.contains(&(kind, project_path(path, name).into_bytes())) {
                return Err(PythonContextDiscoveryError::terminal(
                    "CONTEXT_LOOKUP_CLOSURE_INVALID",
                    "required failed or positive configuration lookup is missing",
                ));
            }
        }
    }
    Ok(())
}

fn authorized_root_bindings(request: &PythonContextDiscoveryRequest) -> Vec<ContextSearchRoot> {
    let mut roots = vec![ContextSearchRoot {
        root_id: request.project_root_id.clone(),
        relative_path: request.project_root_path.as_bytes().to_vec(),
    }];
    roots.extend(
        request
            .registered
            .authorized_roots
            .iter()
            .map(|root| ContextSearchRoot {
                root_id: root.path_id.clone(),
                relative_path: root.relative_path.as_bytes().to_vec(),
            }),
    );
    roots.sort_by(|left, right| left.root_id.cmp(&right.root_id));
    roots.dedup();
    roots
}

fn selected_platforms(
    request: &PythonContextDiscoveryRequest,
    standalone: Option<&toml::Value>,
    project: Option<&toml::Value>,
) -> Result<Vec<String>, PythonContextDiscoveryError> {
    let project = project
        .and_then(|value| value.get("tool"))
        .and_then(|value| value.get("pyrefly"));
    let read = |value: Option<&toml::Value>| -> Result<Vec<String>, PythonContextDiscoveryError> {
        let Some(value) = value.and_then(|value| value.get("python-platform")) else {
            return Ok(Vec::new());
        };
        if let Some(value) = value.as_str() {
            Ok(vec![value.to_owned()])
        } else {
            optional_string_array(Some(value), "python-platform")
        }
    };
    let standalone = read(standalone)?;
    let project = read(project)?;
    if !standalone.is_empty() && !project.is_empty() && standalone != project {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_PLATFORM_CONFLICT",
            "Python platform selections conflict",
        ));
    }
    let mut result = if standalone.is_empty() {
        project
    } else {
        standalone
    };
    if result.is_empty() {
        let tag = request.platform_tag.as_str();
        result.push(
            if tag == "darwin" || tag.starts_with("macos-") {
                "darwin"
            } else if tag == "win32" || tag.starts_with("windows-") {
                "win32"
            } else if tag == "linux" || tag.starts_with("linux-") {
                "linux"
            } else if tag == "all" {
                "all"
            } else {
                return Err(PythonContextDiscoveryError::terminal(
                    "CONTEXT_PLATFORM_UNSUPPORTED",
                    "deployment platform has no explicit Python semantic mapping",
                ));
            }
            .to_owned(),
        );
    }
    if result
        .iter()
        .any(|value| !matches!(value.as_str(), "linux" | "darwin" | "win32" | "all"))
        || result.len() > 1 && result.iter().any(|value| value == "all")
    {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_PLATFORM_UNSUPPORTED",
            "unsupported or contradictory Python platforms",
        ));
    }
    Ok(result)
}

fn discover_module_map(
    request: &PythonContextDiscoveryRequest,
    manifest: &PythonAnalysisContextManifest,
) -> Result<Vec<PythonModuleBinding>, PythonContextDiscoveryError> {
    let mut result = Vec::new();
    let mut bound_files = BTreeSet::new();
    let registered = !request.registered.module_map.is_empty();
    let mut files = request.files.iter().collect::<Vec<_>>();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let mut selected_roots = Vec::new();
    // Bind admitted dependency files relative to their package root before a broad workspace
    // root can rename them. This only assigns input modules; resolver search precedence and
    // the explicit order within each class remain unchanged.
    for id in manifest
        .stub_roots
        .iter()
        .chain(&manifest.dependency_roots)
        .chain(&manifest.module_roots)
        .chain(&manifest.source_roots)
    {
        if !selected_roots.contains(id) {
            selected_roots.push(id.clone());
        }
    }
    // Every captured Python source needs a query binding, including scripts/tests outside
    // import roots. This local fallback changes the input map, never resolver search paths.
    if !registered && !selected_roots.contains(&request.project_root_id) {
        selected_roots.push(request.project_root_id.clone());
    }
    for id in selected_roots {
        let root = manifest
            .root_bindings
            .iter()
            .find(|root| root.root_id == id)
            .ok_or_else(|| {
                PythonContextDiscoveryError::terminal(
                    "CONTEXT_ROOT_UNAUTHORIZED",
                    "selected module root lacks its authorized path",
                )
            })?;
        let path = std::str::from_utf8(&root.relative_path).map_err(|_| {
            PythonContextDiscoveryError::terminal(
                "CONTEXT_NAMESPACE_UNSUPPORTED",
                "module root is not UTF-8",
            )
        })?;
        for file in &files {
            let relative = if matches!(path, "." | "") {
                Some(file.relative_path.as_slice())
            } else {
                file.relative_path
                    .strip_prefix(path.as_bytes())
                    .and_then(|value| value.strip_prefix(b"/"))
            };
            let Some(relative) = relative else { continue };
            let Some((stem, is_stub)) = relative
                .strip_suffix(b".pyi")
                .map(|stem| (stem, true))
                .or_else(|| relative.strip_suffix(b".py").map(|stem| (stem, false)))
            else {
                continue;
            };
            let is_package = stem == b"__init__" || stem.ends_with(b"/__init__");
            let stem = stem.strip_suffix(b"/__init__").unwrap_or(stem);
            // Root order is semantic precedence, not permission to duplicate one file binding.
            // Registered build maps are validated against every applicable authorized root below.
            if !registered && !bound_files.insert(file.file_id.as_str()) {
                continue;
            }
            result.push(PythonModuleBinding {
                module_name: source_module_label(stem),
                file_id: file.file_id.clone(),
                relative_path: file.relative_path.clone(),
                root_id: id.clone(),
                is_stub,
                is_package,
            });
        }
    }
    if registered {
        let candidates = result
            .iter()
            .map(|candidate| {
                (
                    candidate.file_id.as_str(),
                    candidate.relative_path.as_slice(),
                    candidate.root_id.as_str(),
                    candidate.is_stub,
                    candidate.is_package,
                )
            })
            .collect::<BTreeSet<_>>();
        for binding in &request.registered.module_map {
            if binding.module_name.is_empty()
                || !bound_files.insert(binding.file_id.as_str())
                || !candidates.contains(&(
                    binding.file_id.as_str(),
                    binding.relative_path.as_slice(),
                    binding.root_id.as_str(),
                    binding.is_stub,
                    binding.is_package,
                ))
            {
                return Err(PythonContextDiscoveryError::terminal(
                    "CONTEXT_MODULE_MAP_INVALID",
                    "registered module mapping does not resolve to a selected immutable file",
                ));
            }
        }
        result.clone_from(&request.registered.module_map);
    }
    Ok(result)
}

fn parse_digest(value: &str) -> Result<[u8; 32], PythonContextDiscoveryError> {
    if !valid_digest(value) {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_INPUT_INVALID",
            "invalid artifact digest",
        ));
    }
    let mut output = [0; 32];
    for (slot, pair) in output
        .iter_mut()
        .zip(value.as_bytes()[3..].as_chunks::<2>().0)
    {
        let pair = std::str::from_utf8(pair).expect("validated hexadecimal ASCII");
        *slot = u8::from_str_radix(pair, 16).expect("validated hexadecimal digits");
    }
    Ok(output)
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
    files: &BTreeMap<Vec<u8>, &PythonDiscoveryFile>,
) -> Result<
    (
        Vec<PythonContextArtifact>,
        Vec<PythonConfigurationDependency>,
    ),
    PythonContextDiscoveryError,
> {
    let mut candidates = KNOWN_LOCK_FILES
        .iter()
        .filter_map(|name| first_configuration(request, files, name))
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

fn configured_dependency_roots(
    request: &PythonContextDiscoveryRequest,
    pyrefly: Option<&toml::Value>,
    pyproject: Option<&toml::Value>,
    pyrefly_file: Option<&PythonDiscoveryFile>,
    pyproject_file: Option<&PythonDiscoveryFile>,
) -> Result<Vec<String>, PythonContextDiscoveryError> {
    let mut selected = None;
    for (document, file) in [
        (pyrefly, pyrefly_file),
        (
            pyproject
                .and_then(|value| value.get("tool"))
                .and_then(|value| value.get("pyrefly")),
            pyproject_file,
        ),
    ] {
        let Some(document) = document else { continue };
        for field in ["site-package-path", "site_package_path"] {
            let Some(value) = document.get(field) else {
                continue;
            };
            let paths = optional_string_array(Some(value), field)?
                .iter()
                .map(|path| authorized_root_id(request, path, configuration_origin(file)))
                .collect::<Result<Vec<_>, _>>()?;
            if selected.as_ref().is_some_and(|previous| *previous != paths) {
                return Err(PythonContextDiscoveryError::terminal(
                    "CONTEXT_ROOTS_CONFLICT",
                    "Pyrefly configurations select different site-package roots",
                ));
            }
            selected = Some(paths);
        }
    }
    Ok(selected.unwrap_or_default())
}

fn configured_package_roots(
    request: &PythonContextDiscoveryRequest,
    pyrefly: Option<&toml::Value>,
    pyproject: Option<&toml::Value>,
    pyrefly_file: Option<&PythonDiscoveryFile>,
    pyproject_file: Option<&PythonDiscoveryFile>,
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
    let resolve = |paths: Vec<String>, base: &str| {
        paths
            .iter()
            .map(|path| authorized_root_id(request, path, base))
            .collect::<Result<Vec<_>, _>>()
    };
    let standalone = resolve(standalone, configuration_origin(pyrefly_file))?;
    let project_pyrefly = resolve(project_pyrefly, configuration_origin(pyproject_file))?;
    if !standalone.is_empty() && !project_pyrefly.is_empty() && standalone != project_pyrefly {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_ROOTS_CONFLICT",
            "pyrefly.toml and [tool.pyrefly] select different search paths",
        ));
    }
    if !standalone.is_empty() {
        Ok(standalone)
    } else if !project_pyrefly.is_empty() {
        Ok(project_pyrefly)
    } else {
        resolve(
            setuptools_package_roots(pyproject)?,
            configuration_origin(pyproject_file),
        )
    }
}

/// Only settings actually consumed by discovery and installed by the sidecar count as applied.
/// Unknown checker settings qualify the context instead of being silently replaced by defaults.
fn unapplied_checker_settings(
    request: &PythonContextDiscoveryRequest,
    standalone: Option<&toml::Value>,
    project: Option<&toml::Value>,
) -> Vec<String> {
    let mut result = BTreeSet::new();
    if request
        .workspace_profile
        .as_ref()
        .is_some_and(|p| p.profile_artifact.is_some())
    {
        result.insert("workspace-profile-artifact".to_owned());
    }
    for (prefix, document) in [
        ("pyrefly.toml", standalone),
        (
            "tool.pyrefly",
            project
                .and_then(|p| p.get("tool"))
                .and_then(|p| p.get("pyrefly")),
        ),
    ] {
        if let Some(document) = document {
            checker_settings_remainder(prefix, document, false, &mut result);
        }
    }
    if let Some(project) = project.and_then(|p| p.get("project")) {
        for key in ["dependencies", "optional-dependencies"] {
            if let Some(value) = project.get(key)
                && !value.as_array().is_some_and(Vec::is_empty)
                && !value.as_table().is_some_and(toml::Table::is_empty)
            {
                result.insert(format!("project.{key}"));
            }
        }
        if project.get("requires-python").is_some_and(|v| !v.is_str()) {
            result.insert("project.requires-python".to_owned());
        }
    }
    result.into_iter().collect()
}

fn checker_settings_remainder(
    prefix: &str,
    document: &toml::Value,
    environment: bool,
    result: &mut BTreeSet<String>,
) {
    let Some(table) = document.as_table() else {
        result.insert(prefix.to_owned());
        return;
    };
    for (key, value) in table {
        let applied = match key.as_str() {
            "python-version" | "python_version" => value.is_str(),
            "python-platform" if !environment => value.is_str() || value.is_array(),
            "search-path" | "search_path" | "site-package-path" | "site_package_path"
                if !environment =>
            {
                value.is_array()
            }
            "environment" if !environment => {
                checker_settings_remainder(&format!("{prefix}.environment"), value, true, result);
                true
            }
            _ => false,
        };
        if !applied {
            result.insert(format!("{prefix}.{key}"));
        }
    }
}

fn configuration_origin(file: Option<&PythonDiscoveryFile>) -> &str {
    file.and_then(|file| {
        std::str::from_utf8(&file.relative_path)
            .ok()?
            .rsplit_once('/')
            .map(|(parent, _)| parent)
    })
    .unwrap_or(".")
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
    configuration_root: &str,
) -> Result<String, PythonContextDiscoveryError> {
    if !valid_relative_path(configured_path) {
        return Err(PythonContextDiscoveryError::terminal(
            "CONTEXT_ROOTS_INVALID",
            format!("configured package root {configured_path} escapes the project"),
        ));
    }
    let path = if matches!(configured_path, "" | ".") {
        configuration_root.to_owned()
    } else {
        project_path(configuration_root, configured_path)
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
    lookup_evidence: &[ContextLookupEvidence],
) -> Result<String, PythonContextDiscoveryError> {
    #[derive(Serialize)]
    struct DigestView<'a> {
        source_generation: u64,
        dependencies: &'a [PythonConfigurationDependency],
        lookup_evidence: &'a [ContextLookupEvidence],
    }
    let bytes = canonical_json(&DigestView {
        source_generation,
        dependencies,
        lookup_evidence,
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

// A non-Unicode source path still needs a direct checker input. This reversible label
// is not an import-resolution claim: the provider's path and application file ID remain
// authoritative. Ordinary Unicode import names retain their established spelling.
fn source_module_label(stem: &[u8]) -> String {
    use std::fmt::Write as _;

    if let Ok(name) = std::str::from_utf8(stem) {
        return name.replace('/', ".");
    }
    let mut name = String::with_capacity(stem.len() * 3);
    for byte in stem {
        if *byte == b'/' {
            name.push('.');
        } else {
            write!(name, "%{byte:02X}").expect("writing to String is infallible");
        }
    }
    name
}

fn valid_source_path(path: &[u8]) -> bool {
    !path.is_empty()
        && !path.contains(&0)
        && path
            .split(|byte| *byte == b'/')
            .all(|part| !part.is_empty() && part != b"." && part != b"..")
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
            relative_path: path.as_bytes().to_vec(),
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
            typeshed_bundle_digest: Some([0x31; 32]),
            pyrefly_bundle_digest: Some([0x32; 32]),
            ruff_bundle_digest: [0x33; 32],
            provider_bundle_version: "python-providers-v1".to_owned(),
            search_scope: ContextSearchScope {
                namespace: b".".to_vec(),
                ordered_roots: vec![ContextSearchRoot {
                    root_id: "path:project-root".to_owned(),
                    relative_path: b".".to_vec(),
                }],
                policy_identity: [0x34; 32],
                universe: crate::analysis_context::ContextSearchUniverse::Closed {
                    inventory_identity: [0x35; 32],
                },
            },
        }
    }

    #[test]
    fn captured_site_packages_bind_native_modules_and_typing_markers() {
        let mut request = base_request();
        request.files.push(file(
            "pyrefly.toml",
            "file:checker",
            "site-package-path=['vendor']\n",
        ));
        request
            .registered
            .authorized_roots
            .push(PythonAuthorizedRoot {
                relative_path: "vendor".into(),
                path_id: "path:vendor".into(),
            });
        request.files.push(file(
            "vendor/external/__init__.py",
            "file:external",
            "def selected() -> int: return 1\n",
        ));
        request
            .files
            .push(file("vendor/external/py.typed", "file:typed", "partial\n"));
        let first = discover_python_context(&request).unwrap();
        assert_eq!(first.manifest.dependency_roots, ["path:vendor"]);
        assert_eq!(first.manifest.typing_markers[0].contents, b"partial\n");
        assert!(
            first
                .manifest
                .module_map
                .iter()
                .any(|module| module.file_id == "file:external"
                    && module.module_name == "external"
                    && module.root_id == "path:vendor")
        );
        assert!(
            first
                .manifest
                .unapplied_checker_settings
                .as_ref()
                .unwrap()
                .is_empty()
        );
        request.files.retain(|file| file.file_id != "file:typed");
        let removed = discover_python_context(&request).unwrap();
        assert_ne!(
            first.context.analysis_context_id,
            removed.context.analysis_context_id
        );
        assert!(removed.manifest.typing_markers.is_empty());
        request.files.retain(|file| file.file_id != "file:checker");
        request.files.push(file(
            "pyrefly.toml",
            "file:checker",
            "site-package-path=['../outside']\n",
        ));
        assert!(discover_python_context(&request).is_err());
    }

    #[test]
    fn captured_checker_settings_are_applied_or_retain_their_exact_remainder() {
        let mut request = base_request();
        request.files.push(file(
            "pyrefly.toml",
            "file:pyrefly",
            "python-version='3.13'\npython-platform='win32'\nsearch-path=['src']\n",
        ));
        let supported = discover_python_context(&request).unwrap();
        assert_eq!(supported.manifest.unapplied_checker_settings, Some(vec![]));
        assert_eq!(supported.manifest.python_language_version, "3.13");
        assert_eq!(supported.manifest.platforms, ["win32"]);
        request.files.pop();
        request.files.push(file("pyrefly.toml", "file:pyrefly", "python-version='3.13'\nunknown-feature=true\n[environment]\npython_version='3.13'\ninterpreter='private-selection'\n"));
        let unsupported = discover_python_context(&request).unwrap();
        assert_eq!(
            unsupported.manifest.unapplied_checker_settings,
            Some(vec![
                "pyrefly.toml.environment.interpreter".to_owned(),
                "pyrefly.toml.unknown-feature".to_owned(),
            ])
        );
        assert_ne!(
            supported.context.analysis_context_id,
            unsupported.context.analysis_context_id
        );
        assert_ne!(
            supported.manifest.project_config_artifacts,
            unsupported.manifest.project_config_artifacts
        );
        request.files.pop();
        request
            .files
            .push(file("pyrefly.toml", "file:pyrefly", "python-version=314\n"));
        let invalid = discover_python_context(&request).unwrap();
        assert_eq!(
            invalid.manifest.unapplied_checker_settings,
            Some(vec!["pyrefly.toml.python-version".to_owned()])
        );
    }

    #[test]
    fn configured_import_roots_preserve_sources_outside_their_search_path() {
        let mut request = base_request();
        request
            .files
            .push(file("standalone.py", "file:standalone", "import pkg\n"));
        let product = discover_python_context(&request).unwrap();
        assert_eq!(product.manifest.module_roots, ["path:src-root"]);
        assert_eq!(product.manifest.source_roots, ["path:src-root"]);
        let standalone = product
            .manifest
            .module_map
            .iter()
            .find(|m| m.file_id == "file:standalone")
            .unwrap();
        assert_eq!(standalone.module_name, "standalone");
        assert_eq!(standalone.root_id, "path:project-root");
        let package = product
            .manifest
            .module_map
            .iter()
            .find(|m| m.file_id == "file:package")
            .unwrap();
        assert_eq!(package.module_name, "pkg");
        assert_eq!(package.root_id, "path:src-root");
        assert_eq!(product.manifest.module_map.len(), 2);
    }

    #[test]
    fn raw_source_paths_and_root_initializers_keep_distinct_input_bindings() {
        let mut request = base_request();
        let mut raw = file("placeholder.py", "file:raw", "def leaf(): pass\n");
        raw.relative_path = b"dir-\xff/module%?# \\.py".to_vec();
        raw.display_path = "same display".to_owned();
        let mut unicode = file(
            "dir-�/module%?# \\.py",
            "file:unicode",
            "def leaf(): pass\n",
        );
        unicode.display_path.clone_from(&raw.display_path);
        request.files.extend([
            raw,
            unicode,
            file("__init__.py", "file:init", "def root(): pass\n"),
        ]);
        let initial = discover_python_context(&request).unwrap();
        for file in request
            .files
            .iter()
            .filter(|file| file.relative_path.ends_with(b".py"))
        {
            let binding = initial
                .manifest
                .module_map
                .iter()
                .find(|m| m.file_id == file.file_id)
                .unwrap();
            assert_eq!(binding.relative_path, file.relative_path);
        }
        let root = initial
            .manifest
            .module_map
            .iter()
            .find(|m| m.file_id == "file:init")
            .unwrap();
        assert_eq!(root.module_name, "__init__");
        assert!(root.is_package);
        for file in &mut request.files {
            file.display_path = "different display".to_owned();
        }
        assert_eq!(
            initial.manifest,
            discover_python_context(&request).unwrap().manifest
        );
        request.files.last_mut().unwrap().relative_path = b"../outside.py".to_vec();
        assert_eq!(
            discover_python_context(&request).unwrap_err().code(),
            "CONTEXT_INPUT_INVALID"
        );
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
                .map(|file| file.relative_path.clone())
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
        namespace
            .registered
            .authorized_roots
            .push(PythonAuthorizedRoot {
                relative_path: "namespace".to_owned(),
                path_id: "path:namespace-root".to_owned(),
            });
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
            "unapplied_checker_settings",
            "pyrefly_bundle_digest",
            "ruff_bundle_digest",
            "provider_bundle_version",
            "platforms",
            "root_bindings",
            "module_map",
            "configuration_namespace",
            "configuration_roots",
            "configuration_policy_identity",
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

    #[test]
    fn rt_cpg_wp78_integrity() {
        let request = base_request();
        let product = discover_python_context(&request).unwrap();
        let settings = product.effective_settings().unwrap();
        assert_eq!(
            settings.language_version,
            PythonLanguageVersion {
                major: 3,
                minor: 12
            }
        );
        assert_eq!(settings.platforms, ["darwin"]);
        assert_eq!(settings.module_roots[0].relative_path, b"src");
        assert_eq!(settings.module_map[0].module_name, "pkg");
        assert_eq!(settings.module_map[0].relative_path, b"src/pkg/__init__.py");
        assert!(settings.module_map[0].is_package);
        let context_id = decode_public_id(
            IdentityDomain::AnalysisContext,
            None,
            &product.context.analysis_context_id,
        )
        .unwrap();
        assert_eq!(context_id.len(), 16);
        let fingerprint = product.context.fingerprint_bytes().unwrap();
        assert_eq!(fingerprint.len(), 32);
        assert_ne!(context_id.as_slice(), &fingerprint[..16]);
        assert_eq!(product.configuration_dependencies.lookup_evidence.len(), 6);
        let absent = product
            .configuration_dependencies
            .lookup_evidence
            .iter()
            .find(|item| item.kind == ContextLookupKind::PyreflyConfiguration)
            .unwrap();
        assert_eq!(absent.observation, ContextLookupObservation::Absent);
        assert_eq!(absent.scope, request.search_scope);
        let mut missing = product.clone();
        missing.configuration_dependencies.lookup_evidence.remove(1);
        missing.configuration_dependencies.dependency_set_digest = dependency_set_digest(
            missing.source_generation,
            &missing.configuration_dependencies.dependencies,
            &missing.configuration_dependencies.lookup_evidence,
        )
        .unwrap();
        assert!(
            missing.validate().is_err(),
            "resealing a digest cannot restore omitted failed lookup evidence"
        );

        let mut changed = request.clone();
        changed.files[1] = file("src/pkg/__init__.py", "file:package", "VALUE = 99\n");
        changed.source_generation += 1;
        changed.search_scope.universe = crate::analysis_context::ContextSearchUniverse::Closed {
            inventory_identity: [91; 32],
        };
        let source_only = discover_python_context(&changed).unwrap();
        assert_eq!(
            source_only.context.context_fingerprint,
            product.context.context_fingerprint
        );
        assert_eq!(source_only.effective_settings().unwrap(), settings);

        changed.files.push(file(
            "pyrefly.toml",
            "file:pyrefly",
            "python-platform='linux'\n",
        ));
        let configured = discover_python_context(&changed).unwrap();
        assert_eq!(
            configured.effective_settings().unwrap().platforms,
            ["linux"]
        );
        assert_ne!(
            configured.context.analysis_context_id,
            source_only.context.analysis_context_id
        );
        assert!(plan_python_context_transition(&source_only, &configured).republish_required);
        assert!(configured.configuration_dependencies.lookup_evidence.iter().any(|item| {
            item.kind == ContextLookupKind::PyreflyConfiguration &&
                matches!(&item.observation, ContextLookupObservation::Present { file_id, .. } if file_id == "file:pyrefly")
        }));
    }

    #[test]
    fn python_effective_context_root_order_stubs_lock_and_negative_scope() {
        let mut request = base_request();
        request
            .registered
            .authorized_roots
            .push(PythonAuthorizedRoot {
                relative_path: "stubs".to_owned(),
                path_id: "path:stubs".to_owned(),
            });
        request.registered.stub_roots = vec!["path:stubs".to_owned()];
        request
            .files
            .push(file("stubs/pkg.pyi", "file:stub", "VALUE: str\n"));
        request
            .files
            .push(file("uv.lock", "file:lock", "version = 1\n"));
        let before = discover_python_context(&request).unwrap();
        let settings = before.effective_settings().unwrap();
        assert!(settings.module_map[0].is_stub);
        assert_eq!(settings.module_map[0].module_name, "pkg");
        assert!(!settings.module_map[1].is_stub);
        assert_eq!(
            settings.lock_artifacts[0].digest,
            crate::integrity::digest_bytes(b"version = 1\n")
        );
        request.files[3] = file("uv.lock", "file:lock", "version = 2\n");
        let lock_changed = discover_python_context(&request).unwrap();
        assert_ne!(
            lock_changed.context.analysis_context_id,
            before.context.analysis_context_id
        );
        assert_ne!(
            lock_changed.effective_settings().unwrap().lock_artifacts,
            settings.lock_artifacts
        );
        request.registered.module_roots = vec!["path:src-root".to_owned(), "path:stubs".to_owned()];
        let ordered = discover_python_context(&request).unwrap();
        request.registered.module_roots.reverse();
        let reversed = discover_python_context(&request).unwrap();
        assert_ne!(
            ordered.effective_settings().unwrap().module_roots,
            reversed.effective_settings().unwrap().module_roots
        );
        assert_ne!(
            ordered.context.analysis_context_id,
            reversed.context.analysis_context_id
        );
        request.search_scope.universe =
            crate::analysis_context::ContextSearchUniverse::Incomplete {
                observed_identity: [73; 32],
            };
        let incomplete = discover_python_context(&request).unwrap();
        assert!(
            incomplete
                .configuration_dependencies
                .lookup_evidence
                .iter()
                .any(|item| item.observation == ContextLookupObservation::Incomplete)
        );
        assert!(
            !incomplete
                .configuration_dependencies
                .lookup_evidence
                .iter()
                .any(|item| item.observation == ContextLookupObservation::Absent)
        );
        assert!(plan_python_context_transition(&incomplete, &incomplete).republish_required);
    }

    #[test]
    fn python_effective_context_overlapping_roots_bind_each_file_once() {
        let mut request = base_request();
        request
            .registered
            .authorized_roots
            .push(PythonAuthorizedRoot {
                relative_path: "src/pkg".to_owned(),
                path_id: "path:nested".to_owned(),
            });
        request
            .files
            .push(file("src/pkg/deep/mod.py", "file:nested", "VALUE = 2\n"));
        request.registered.module_roots = vec![
            request.project_root_id.clone(),
            "path:src-root".to_owned(),
            "path:nested".to_owned(),
        ];
        let outer = discover_python_context(&request).unwrap();
        let outer_settings = outer.effective_settings().unwrap();
        assert_eq!(outer_settings.module_map.len(), 2);
        let outer_binding = outer_settings
            .module_map
            .iter()
            .find(|binding| binding.file_id == "file:nested")
            .unwrap();
        assert_eq!(outer_binding.module_name, "src.pkg.deep.mod");
        assert_eq!(outer_binding.root_id, request.project_root_id);

        request.registered.module_roots.reverse();
        let inner = discover_python_context(&request).unwrap();
        let inner_settings = inner.effective_settings().unwrap();
        assert_eq!(inner_settings.module_map.len(), 2);
        let inner_binding = inner_settings
            .module_map
            .iter()
            .find(|binding| binding.file_id == "file:nested")
            .unwrap();
        assert_eq!(inner_binding.module_name, "deep.mod");
        assert_eq!(inner_binding.root_id, "path:nested");
        assert_ne!(
            outer.context.analysis_context_id,
            inner.context.analysis_context_id
        );
        assert_eq!(
            inner_settings
                .module_map
                .iter()
                .map(|binding| &binding.file_id)
                .collect::<BTreeSet<_>>()
                .len(),
            inner_settings.module_map.len()
        );

        // Precedence is across root classes as well as within an ordered class.
        request.registered.stub_roots = vec![request.project_root_id.clone()];
        let stub_first = discover_python_context(&request)
            .unwrap()
            .effective_settings()
            .unwrap();
        assert_eq!(
            stub_first
                .module_map
                .iter()
                .find(|binding| binding.file_id == "file:nested")
                .unwrap()
                .module_name,
            "src.pkg.deep.mod"
        );
    }

    #[test]
    fn python_effective_context_registered_map_preserves_override_without_ambiguity() {
        let mut request = base_request();
        request.registered.module_roots =
            vec!["path:src-root".to_owned(), request.project_root_id.clone()];
        request.registered.module_map = vec![PythonModuleBinding {
            module_name: "registered.package".to_owned(),
            file_id: "file:package".to_owned(),
            relative_path: b"src/pkg/__init__.py".to_vec(),
            root_id: request.project_root_id.clone(),
            is_stub: false,
            is_package: true,
        }];
        let selected = discover_python_context(&request)
            .unwrap()
            .effective_settings()
            .unwrap();
        assert_eq!(selected.module_map, request.registered.module_map);
        for mutation in 0..3 {
            let mut invalid = request.clone();
            match mutation {
                0 => {
                    let mut duplicate = invalid.registered.module_map[0].clone();
                    duplicate.module_name = "different.name".to_owned();
                    duplicate.root_id = "path:src-root".to_owned();
                    invalid.registered.module_map.push(duplicate);
                }
                1 => invalid.registered.module_map[0].relative_path = b"different.py".to_vec(),
                _ => invalid.registered.module_map[0].root_id = "path:unselected".to_owned(),
            }
            assert_eq!(
                discover_python_context(&invalid).unwrap_err().code(),
                "CONTEXT_MODULE_MAP_INVALID"
            );
        }
    }

    #[test]
    fn python_configuration_search_order_uses_selected_config_relative_roots() {
        let mut request = base_request();
        request.project_root_path = "app".to_owned();
        request.project_root_id = "path:app".to_owned();
        request
            .registered
            .authorized_roots
            .push(PythonAuthorizedRoot {
                path_id: "path:app-src".to_owned(),
                relative_path: "app/src".to_owned(),
            });
        request.search_scope.ordered_roots.insert(
            0,
            ContextSearchRoot {
                root_id: "path:app".to_owned(),
                relative_path: b"app".to_vec(),
            },
        );
        request.files.push(file("app/pyproject.toml", "file:app-config",
            "[project]\nrequires-python='>=3.13,<3.14'\n[tool.setuptools.packages.find]\nwhere=['src']\n"));
        request
            .files
            .push(file("app/src/mod.py", "file:app-mod", "VALUE = 4\n"));
        let nested = discover_python_context(&request).unwrap();
        let settings = nested.effective_settings().unwrap();
        assert_eq!(settings.language_version.minor, 13);
        assert_eq!(settings.module_roots[0].relative_path, b"app/src");
        assert_eq!(settings.module_map[0].module_name, "mod");
        assert_eq!(nested.configuration_dependencies.lookup_evidence.len(), 12);
        request.search_scope.ordered_roots.reverse();
        let root_first = discover_python_context(&request).unwrap();
        assert_eq!(
            root_first
                .effective_settings()
                .unwrap()
                .language_version
                .minor,
            12
        );
        assert_eq!(
            root_first.effective_settings().unwrap().module_roots[0].relative_path,
            b"src"
        );
        assert_ne!(
            nested.context.analysis_context_id,
            root_first.context.analysis_context_id
        );
    }

    #[test]
    fn python_syntax_context_does_not_fabricate_external_bundle_identity() {
        let mut request = base_request();
        request.typeshed_bundle_digest = None;
        request.pyrefly_bundle_digest = None;
        let syntax = discover_python_context(&request).unwrap();
        let settings = syntax.effective_settings().unwrap();
        assert!(settings.typeshed_bundle_digest.is_none());
        assert!(settings.pyrefly_bundle_digest.is_none());
        assert_eq!(settings.ruff_bundle_digest, request.ruff_bundle_digest);
        assert_eq!(settings.language_version.minor, 12);
        assert_eq!(settings.module_map[0].module_name, "pkg");
        assert!(syntax.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "CONTEXT_EXTERNAL_BUNDLE_UNAVAILABLE" && !diagnostic.terminal
        }));
        request.typeshed_bundle_digest = Some([15; 32]);
        request.pyrefly_bundle_digest = Some([16; 32]);
        let semantic = discover_python_context(&request).unwrap();
        assert_ne!(
            syntax.context.analysis_context_id,
            semantic.context.analysis_context_id
        );
        assert_eq!(
            semantic
                .effective_settings()
                .unwrap()
                .typeshed_bundle_digest,
            Some([15; 32])
        );
    }
}
