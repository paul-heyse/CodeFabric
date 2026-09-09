//! Owned context inputs shared without importing a provider or a daemon.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::AnalysisContextError;

/// One authorized root, in the precedence order selected by context discovery.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextSearchRoot {
    pub root_id: String,
    pub relative_path: Vec<u8>,
}

/// Whether the source owner proved its complete selected discovery universe.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextSearchUniverse {
    Closed { inventory_identity: [u8; 32] },
    Incomplete { observed_identity: [u8; 32] },
}

/// The complete selection inputs of a positive or failed configuration lookup.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextSearchScope {
    pub namespace: Vec<u8>,
    pub ordered_roots: Vec<ContextSearchRoot>,
    pub policy_identity: [u8; 32],
    pub universe: ContextSearchUniverse,
}

impl ContextSearchScope {
    /// Validate the authorized namespace and ordered roots, without reading disk.
    ///
    /// # Errors
    /// Rejects missing identity, repeated roots and paths outside the selected namespace.
    pub fn validate(&self) -> Result<(), AnalysisContextError> {
        let mut ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        let inventory_identity = match self.universe {
            ContextSearchUniverse::Closed { inventory_identity } => inventory_identity,
            ContextSearchUniverse::Incomplete { observed_identity } => observed_identity,
        };
        if !valid_context_relative_path(&self.namespace)
            || self.ordered_roots.is_empty()
            || self.policy_identity == [0; 32]
            || inventory_identity == [0; 32]
            || self.ordered_roots.iter().any(|root| {
                root.root_id.is_empty()
                    || !valid_context_relative_path(&root.relative_path)
                    || !context_path_is_within(&root.relative_path, &self.namespace)
                    || !ids.insert(&root.root_id)
                    || !paths.insert(&root.relative_path)
            })
        {
            return Err(AnalysisContextError::InvalidSearchScope);
        }
        Ok(())
    }

    #[must_use]
    pub const fn is_closed(&self) -> bool {
        matches!(self.universe, ContextSearchUniverse::Closed { .. })
    }
}

/// Closed configuration lookup categories; not an interpreted predicate language.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextLookupKind {
    PythonProjectConfiguration,
    PyreflyConfiguration,
    UvLock,
    PoetryLock,
    PdmLock,
    PipfileLock,
    FrozenRequirements,
    CargoManifest,
    CargoLock,
    CargoConfiguration,
    RustToolchain,
    RustBuildScript,
}

/// Observed result of one search. Only a closed universe can prove absence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextLookupObservation {
    Present { file_id: String, digest: [u8; 32] },
    Absent,
    Incomplete,
}

/// Immutable support for a lookup, retaining failed candidates and root order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextLookupEvidence {
    pub scope: ContextSearchScope,
    pub kind: ContextLookupKind,
    pub relative_path: Vec<u8>,
    pub observation: ContextLookupObservation,
}

impl ContextLookupEvidence {
    /// Validate support before admitting or persisting it.
    ///
    /// # Errors
    /// Rejects an escaping path, contradictory absence or an empty present identity.
    pub fn validate(&self) -> Result<(), AnalysisContextError> {
        self.scope.validate()?;
        if !valid_context_relative_path(&self.relative_path)
            || !context_path_is_within(&self.relative_path, &self.scope.namespace)
            || matches!(self.observation, ContextLookupObservation::Absent)
                && !self.scope.is_closed()
            || matches!(&self.observation, ContextLookupObservation::Present { file_id, .. } if file_id.is_empty())
        {
            return Err(AnalysisContextError::InvalidLookupEvidence);
        }
        Ok(())
    }
}

/// Immutable file supplied by the authorized inventory/capture owner.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextFileInput {
    pub file_id: String,
    pub relative_path: Vec<u8>,
    pub digest: [u8; 32],
    pub contents: Vec<u8>,
}

/// Content-addressed selected artifact, independent of provider-native types.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextArtifactInput {
    pub file_id: String,
    pub digest: [u8; 32],
}

/// Selected Python language version, not a host interpreter probe.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PythonLanguageVersion {
    pub major: u16,
    pub minor: u16,
}

/// An explicit module candidate, preserving root order and stub/source coexistence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PythonModuleBinding {
    pub module_name: String,
    pub file_id: String,
    pub relative_path: Vec<u8>,
    pub root_id: String,
    pub is_stub: bool,
    pub is_package: bool,
}

/// Effective values to install in a Python provider, never merely a context hash.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PythonContextSettings {
    pub language_version: PythonLanguageVersion,
    /// Selected Pyrefly platform values; `all` remains an explicit selection.
    pub platforms: Vec<String>,
    pub module_roots: Vec<ContextSearchRoot>,
    pub source_roots: Vec<ContextSearchRoot>,
    pub stub_roots: Vec<ContextSearchRoot>,
    pub dependency_roots: Vec<ContextSearchRoot>,
    pub module_map: Vec<PythonModuleBinding>,
    /// `None` means unavailable semantic input, never an empty installed bundle.
    pub typeshed_bundle_digest: Option<[u8; 32]>,
    pub pyrefly_bundle_digest: Option<[u8; 32]>,
    pub ruff_bundle_digest: [u8; 32],
    pub provider_bundle_version: String,
    pub configuration_artifacts: Vec<ContextArtifactInput>,
    pub lock_artifacts: Vec<ContextArtifactInput>,
}

/// Cargo target selected by the caller's authorized build profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RustTargetKind {
    Library,
    Binary,
    Example,
    Test,
    Benchmark,
    ProcMacro,
}

impl RustTargetKind {
    /// Stable application vocabulary shared by context manifests and processing scope.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Library => "library",
            Self::Binary => "binary",
            Self::Example => "example",
            Self::Test => "test",
            Self::Benchmark => "benchmark",
            Self::ProcMacro => "proc_macro",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RustTargetSettings {
    pub name: String,
    pub kind: RustTargetKind,
    pub crate_root: Vec<u8>,
}

/// One explicitly selected conditional-compilation option.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RustCfgSetting {
    pub name: String,
    pub value: Option<String>,
}

/// Exact compiler artifact identity, distinct from the human toolchain channel.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RustToolchainSettings {
    pub release: String,
    pub commit_hash: String,
    pub artifact_digest: [u8; 32],
}

/// Explicit build-environment input selected by the authorized profile, never host inference.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RustEnvironmentSetting {
    pub name: String,
    pub value: String,
}

/// Effective Rust compilation values. Missing resolution is explicit, not an empty success.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RustCompilationSettings {
    pub package_name: String,
    pub package_version: String,
    pub manifest_path: Vec<u8>,
    pub edition: String,
    pub target: RustTargetSettings,
    pub requested_features: Vec<String>,
    pub default_features: bool,
    pub cfgs: Vec<RustCfgSetting>,
    pub environment: Vec<RustEnvironmentSetting>,
    pub target_triple: String,
    pub profile: String,
    pub toolchain: RustToolchainSettings,
    pub sysroot: ContextArtifactInput,
    pub provider_bundle_version: String,
    pub configuration_artifacts: Vec<ContextArtifactInput>,
    pub lock_artifacts: Vec<ContextArtifactInput>,
    pub dependency_inputs: Option<Vec<ContextArtifactInput>>,
    pub build_inputs: Option<Vec<ContextArtifactInput>>,
    /// Configured compiler flags retained for the contained Cargo invocation, never a shell.
    pub configured_rustflags: Option<Vec<String>>,
}

pub(super) fn valid_context_relative_path(path: &[u8]) -> bool {
    !path.is_empty()
        && path[0] != b'/'
        && !path.contains(&0)
        && !path.contains(&b'\\')
        && (path == b"."
            || path
                .split(|byte| *byte == b'/')
                .all(|part| !part.is_empty() && part != b".." && part != b"."))
}

pub(super) fn context_path_is_within(path: &[u8], root: &[u8]) -> bool {
    root == b"."
        || path == root
        || path
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with(b"/"))
}
