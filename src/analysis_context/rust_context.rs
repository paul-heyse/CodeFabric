//! Read-only Rust context preparation. Cargo resolution and execution belong to the extractor lane.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use thiserror::Error;

use super::inputs::{context_path_is_within, valid_context_relative_path};
use super::{
    AnalysisContext, AnalysisContextError, AnalysisContextKind, ContextArtifactInput,
    ContextFileInput, ContextLookupEvidence, ContextLookupKind, ContextLookupObservation,
    ContextSearchScope, RustCfgSetting, RustCompilationSettings, RustEnvironmentSetting,
    RustTargetKind, RustTargetSettings, RustToolchainSettings,
};

/// Explicit caller selections; no missing value is inferred from the daemon's environment.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RustContextSelection {
    pub manifest_path: Option<Vec<u8>>,
    pub cargo_workspace_root: Option<Vec<u8>>,
    pub package_name: Option<String>,
    pub target: Option<RustTargetSettings>,
    pub requested_features: Vec<String>,
    pub default_features: bool,
    pub cfgs: Vec<RustCfgSetting>,
    pub environment: Vec<RustEnvironmentSetting>,
    pub target_triple: Option<String>,
    pub profile: Option<String>,
    pub toolchain: Option<RustToolchainSettings>,
    pub sysroot: Option<ContextArtifactInput>,
    pub dependency_inputs: Option<Vec<ContextArtifactInput>>,
    pub build_inputs: Option<Vec<ContextArtifactInput>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustContextDiscoveryRequest {
    pub workspace_id: String,
    pub source_generation: u64,
    pub provider_bundle_version: String,
    pub files: Vec<ContextFileInput>,
    pub search_scope: ContextSearchScope,
    pub selection: RustContextSelection,
}

/// Preparation limitations are data, not fabricated successful compiler coverage.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RustContextRemainder {
    IncompleteSearch,
    PackageSelectionRequired,
    PackageMetadataInherited,
    TargetSelectionRequired,
    TargetSourceUnavailable,
    TargetTripleRequired,
    ProfileRequired,
    ToolchainRequired,
    SysrootRequired,
    DependencyResolutionRequired,
    BuildInputsRequired,
    CargoMetadataRequired,
    UnsupportedCargoConfiguration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustContextDiscoveryOutcome {
    Prepared(Box<RustContextDiscoveryProduct>),
    Unresolved {
        reasons: Vec<RustContextRemainder>,
        lookup_evidence: Vec<ContextLookupEvidence>,
    },
    Ambiguous {
        manifest_paths: Vec<Vec<u8>>,
        lookup_evidence: Vec<ContextLookupEvidence>,
    },
}

/// Selected settings are ready for job preparation, not permission to claim MIR completeness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustContextDiscoveryProduct {
    pub context: AnalysisContext,
    pub settings: RustCompilationSettings,
    pub canonical_manifest: Vec<u8>,
    pub lookup_evidence: Vec<ContextLookupEvidence>,
    pub remainders: Vec<RustContextRemainder>,
    pub source_generation: u64,
    pub search_scope: ContextSearchScope,
}

impl RustContextDiscoveryProduct {
    /// Verify settings, context identity and the still-required Cargo resolution boundary.
    ///
    /// # Errors
    /// Rejects changed settings, dropped preparation limitations or conflicting lookup evidence.
    pub fn validate(&self) -> Result<(), RustContextDiscoveryError> {
        self.context.validate()?;
        self.search_scope.validate()?;
        let canonical = canonical_manifest(&self.search_scope, &self.settings)?;
        if canonical != self.canonical_manifest
            || self.context.fingerprint_bytes()? != crate::integrity::digest_bytes(&canonical)
            || self.context.context_kind != AnalysisContextKind::Rust
            || self.context.provider_bundle_version != self.settings.provider_bundle_version
            || self.context.compiler_or_language_version != self.settings.toolchain.release
            || !self
                .remainders
                .contains(&RustContextRemainder::CargoMetadataRequired)
        {
            return Err(RustContextDiscoveryError::InvalidInput(
                "prepared context identity or resolution boundary drifted",
            ));
        }
        let mut observed = BTreeSet::new();
        for evidence in &self.lookup_evidence {
            evidence.validate()?;
            if evidence.scope != self.search_scope
                || !observed.insert((evidence.kind, evidence.relative_path.clone()))
            {
                return Err(RustContextDiscoveryError::InvalidInput(
                    "lookup scope drifted",
                ));
            }
        }
        if !required_lookup_keys(&self.search_scope).is_subset(&observed) {
            return Err(RustContextDiscoveryError::InvalidInput(
                "required configuration lookup was omitted",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum RustContextDiscoveryError {
    #[error(transparent)]
    Context(#[from] AnalysisContextError),
    #[error("invalid immutable Rust context input: {0}")]
    InvalidInput(&'static str),
    #[error("invalid Cargo configuration: {0}")]
    InvalidToml(String),
    #[error("Rust context canonicalization failed: {0}")]
    Canonical(String),
}

/// Select one package from captured Cargo inputs and prepare explicit compiler settings.
///
/// # Errors
/// Rejects mismatched bytes, escaping paths, duplicate identities and malformed Cargo TOML.
/// Missing/ambiguous metadata returns an owned unresolved outcome rather than guessed settings.
pub fn discover_rust_context(
    request: &RustContextDiscoveryRequest,
) -> Result<RustContextDiscoveryOutcome, RustContextDiscoveryError> {
    request.search_scope.validate()?;
    let files = validate_inputs(request)?;
    let lookup_evidence = observe_lookups(request, &files)?;
    let mut candidates = Vec::new();
    for (path, file) in &files {
        if path.rsplit(|byte| *byte == b'/').next() != Some(b"Cargo.toml".as_slice()) {
            continue;
        }
        let document = parse_toml(file)?;
        let Some(package) = document.get("package") else {
            continue;
        };
        if request
            .selection
            .manifest_path
            .as_ref()
            .is_some_and(|selected| selected != path)
            || request
                .selection
                .package_name
                .as_deref()
                .is_some_and(|selected| {
                    package.get("name").and_then(toml::Value::as_str) != Some(selected)
                })
        {
            continue;
        }
        candidates.push((*file, document));
    }
    if candidates.len() > 1 {
        return Ok(RustContextDiscoveryOutcome::Ambiguous {
            manifest_paths: candidates
                .iter()
                .map(|(file, _)| file.relative_path.clone())
                .collect(),
            lookup_evidence,
        });
    }
    let Some((manifest, document)) = candidates.pop() else {
        return Ok(RustContextDiscoveryOutcome::Unresolved {
            reasons: vec![RustContextRemainder::PackageSelectionRequired],
            lookup_evidence,
        });
    };
    let mut remainders = Vec::new();
    let Some(settings) = prepare_settings(request, &files, manifest, &document, &mut remainders)?
    else {
        return Ok(RustContextDiscoveryOutcome::Unresolved {
            reasons: remainders,
            lookup_evidence,
        });
    };
    if !request.search_scope.is_closed() {
        remainders.push(RustContextRemainder::IncompleteSearch);
    }
    // TOML preparation does not establish Cargo's resolved unit graph, feature unification,
    // build-script outputs or proc-macro closure. WP82 must supply that independent authority.
    remainders.push(RustContextRemainder::CargoMetadataRequired);
    if settings.dependency_inputs.is_none() {
        remainders.push(RustContextRemainder::DependencyResolutionRequired);
    }
    if settings.build_inputs.is_none() {
        remainders.push(RustContextRemainder::BuildInputsRequired);
    }
    remainders.sort();
    remainders.dedup();
    let canonical_manifest = canonical_manifest(&request.search_scope, &settings)?;
    let fingerprint = crate::integrity::digest_bytes(&canonical_manifest);
    let context = AnalysisContext::new_from_manifest_fingerprint(
        &request.workspace_id,
        AnalysisContextKind::Rust,
        request.provider_bundle_version.clone(),
        settings.toolchain.release.clone(),
        fingerprint,
        true,
    )?;
    Ok(RustContextDiscoveryOutcome::Prepared(Box::new(
        RustContextDiscoveryProduct {
            context,
            settings,
            canonical_manifest,
            lookup_evidence,
            remainders,
            source_generation: request.source_generation,
            search_scope: request.search_scope.clone(),
        },
    )))
}

fn validate_inputs(
    request: &RustContextDiscoveryRequest,
) -> Result<BTreeMap<Vec<u8>, &ContextFileInput>, RustContextDiscoveryError> {
    if request.provider_bundle_version.is_empty() {
        return Err(RustContextDiscoveryError::InvalidInput(
            "missing provider identity",
        ));
    }
    let mut files = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for file in &request.files {
        if file.file_id.is_empty()
            || !valid_context_relative_path(&file.relative_path)
            || !context_path_is_within(&file.relative_path, &request.search_scope.namespace)
            || crate::integrity::digest_bytes(&file.contents) != file.digest
            || !ids.insert(&file.file_id)
            || files.insert(file.relative_path.clone(), file).is_some()
        {
            return Err(RustContextDiscoveryError::InvalidInput(
                "path, identity or content mismatch",
            ));
        }
    }
    Ok(files)
}

fn observe_lookups(
    request: &RustContextDiscoveryRequest,
    files: &BTreeMap<Vec<u8>, &ContextFileInput>,
) -> Result<Vec<ContextLookupEvidence>, RustContextDiscoveryError> {
    let mut keys = required_lookup_keys(&request.search_scope);
    // Package manifests outside the ordered ancestor search roots remain actual positive inputs.
    for path in files.keys().filter(|path| path.ends_with(b"Cargo.toml")) {
        keys.insert((ContextLookupKind::CargoManifest, path.clone()));
    }
    let mut evidence = Vec::new();
    for (kind, relative_path) in keys {
        let observation = files.get(&relative_path).map_or_else(
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
        );
        let item = ContextLookupEvidence {
            scope: request.search_scope.clone(),
            kind,
            relative_path,
            observation,
        };
        item.validate()?;
        evidence.push(item);
    }
    Ok(evidence)
}

fn required_lookup_keys(scope: &ContextSearchScope) -> BTreeSet<(ContextLookupKind, Vec<u8>)> {
    let mut keys = BTreeSet::new();
    for root in &scope.ordered_roots {
        for (name, kind) in [
            (b"Cargo.toml".as_slice(), ContextLookupKind::CargoManifest),
            (b"Cargo.lock".as_slice(), ContextLookupKind::CargoLock),
            (
                b".cargo/config".as_slice(),
                ContextLookupKind::CargoConfiguration,
            ),
            (
                b".cargo/config.toml".as_slice(),
                ContextLookupKind::CargoConfiguration,
            ),
            (
                b"rust-toolchain".as_slice(),
                ContextLookupKind::RustToolchain,
            ),
            (
                b"rust-toolchain.toml".as_slice(),
                ContextLookupKind::RustToolchain,
            ),
            (b"build.rs".as_slice(), ContextLookupKind::RustBuildScript),
        ] {
            keys.insert((kind, joined_path(&root.relative_path, name)));
        }
    }
    keys
}

fn effective_package_string(
    package: &toml::Value,
    field: &str,
    default: &str,
    files: &BTreeMap<Vec<u8>, &ContextFileInput>,
    manifest: &ContextFileInput,
) -> Result<Option<String>, RustContextDiscoveryError> {
    let Some(value) = package.get(field) else {
        return Ok(Some(default.to_owned()));
    };
    if let Some(value) = value.as_str() {
        return Ok(Some(value.to_owned()));
    }
    if value.get("workspace").and_then(toml::Value::as_bool) != Some(true) {
        return Ok(None);
    }
    let mut selected = None;
    for file in files.values() {
        let Some(parent) = file.relative_path.strip_suffix(b"Cargo.toml") else {
            continue;
        };
        if !manifest.relative_path.starts_with(parent) {
            continue;
        }
        let document = parse_toml(file)?;
        if let Some(value) = document
            .get("workspace")
            .and_then(|workspace| workspace.get("package"))
            .and_then(|package| package.get(field))
            .and_then(toml::Value::as_str)
            && selected
                .as_ref()
                .is_none_or(|(length, _)| parent.len() > *length)
        {
            selected = Some((parent.len(), value.to_owned()));
        }
    }
    Ok(selected.map(|(_, value)| value))
}

fn prepare_settings(
    request: &RustContextDiscoveryRequest,
    files: &BTreeMap<Vec<u8>, &ContextFileInput>,
    manifest: &ContextFileInput,
    document: &toml::Value,
    remainders: &mut Vec<RustContextRemainder>,
) -> Result<Option<RustCompilationSettings>, RustContextDiscoveryError> {
    let selection = &request.selection;
    let package = document.get("package").expect("selected package document");
    let name = package.get("name").and_then(toml::Value::as_str);
    let version = effective_package_string(package, "version", "0.0.0", files, manifest)?;
    let edition = effective_package_string(package, "edition", "2015", files, manifest)?;
    if name.is_none() || version.is_none() || edition.is_none() {
        remainders.push(RustContextRemainder::PackageMetadataInherited);
    }
    if selection.target.is_none() {
        remainders.push(RustContextRemainder::TargetSelectionRequired);
    }
    if selection
        .target_triple
        .as_ref()
        .is_none_or(String::is_empty)
    {
        remainders.push(RustContextRemainder::TargetTripleRequired);
    }
    if selection.profile.as_ref().is_none_or(String::is_empty) {
        remainders.push(RustContextRemainder::ProfileRequired);
    }
    if selection.toolchain.is_none() {
        remainders.push(RustContextRemainder::ToolchainRequired);
    }
    if selection.sysroot.is_none() {
        remainders.push(RustContextRemainder::SysrootRequired);
    }
    if !remainders.is_empty() {
        return Ok(None);
    }
    let target = selection.target.clone().expect("validated selected target");
    if target.name.is_empty()
        || !valid_context_relative_path(&target.crate_root)
        || selection
            .cargo_workspace_root
            .as_deref()
            .is_some_and(|root| !valid_context_relative_path(root))
    {
        return Err(RustContextDiscoveryError::InvalidInput(
            "invalid selected target",
        ));
    }
    if !files.contains_key(&target.crate_root) {
        remainders.push(RustContextRemainder::TargetSourceUnavailable);
        return Ok(None);
    }
    let toolchain = selection
        .toolchain
        .clone()
        .expect("validated toolchain selection");
    if toolchain.release.is_empty()
        || toolchain.commit_hash.is_empty()
        || toolchain.artifact_digest == [0; 32]
    {
        return Err(RustContextDiscoveryError::InvalidInput(
            "incomplete toolchain identity",
        ));
    }
    let target_triple = selection
        .target_triple
        .clone()
        .expect("validated target triple");
    let (configuration_artifacts, lock_artifacts, configured_rustflags) =
        configuration_inputs(request, files, manifest, &target_triple, remainders)?;
    let mut requested_features = selection.requested_features.clone();
    if requested_features.iter().any(String::is_empty)
        || selection.cfgs.iter().any(|cfg| cfg.name.is_empty())
    {
        return Err(RustContextDiscoveryError::InvalidInput(
            "empty feature or cfg selection",
        ));
    }
    requested_features.sort();
    requested_features.dedup();
    Ok(Some(RustCompilationSettings {
        package_name: name.expect("validated package name").to_owned(),
        package_version: version.expect("validated package version"),
        manifest_path: manifest.relative_path.clone(),
        cargo_workspace_root: selection.cargo_workspace_root.clone(),
        edition: edition.expect("validated edition"),
        crate_types: selected_crate_types(document, &target)?,
        target,
        requested_features,
        default_features: selection.default_features,
        cfgs: selection.cfgs.clone(),
        environment: selection.environment.clone(),
        target_triple,
        profile: selection.profile.clone().expect("validated profile"),
        toolchain,
        sysroot: selection.sysroot.clone().expect("validated sysroot"),
        provider_bundle_version: request.provider_bundle_version.clone(),
        configuration_artifacts,
        lock_artifacts,
        dependency_inputs: selection.dependency_inputs.clone(),
        build_inputs: selection.build_inputs.clone(),
        configured_rustflags,
    }))
}

fn selected_crate_types(
    document: &toml::Value,
    target: &RustTargetSettings,
) -> Result<Vec<String>, RustContextDiscoveryError> {
    let (table, default) = match target.kind {
        RustTargetKind::Library => (document.get("lib"), "lib"),
        RustTargetKind::ProcMacro => (document.get("lib"), "proc-macro"),
        RustTargetKind::Example => (
            document
                .get("example")
                .and_then(toml::Value::as_array)
                .and_then(|targets| {
                    targets.iter().find(|candidate| {
                        candidate.get("name").and_then(toml::Value::as_str)
                            == Some(target.name.as_str())
                    })
                }),
            "bin",
        ),
        RustTargetKind::Binary | RustTargetKind::Test | RustTargetKind::Benchmark => (None, "bin"),
    };
    let Some(selected) = table.and_then(|table| table.get("crate-type")) else {
        return Ok(vec![default.to_owned()]);
    };
    let selected = selected
        .as_array()
        .ok_or(RustContextDiscoveryError::InvalidInput(
            "crate-type must be an array",
        ))?;
    if selected.is_empty() || selected.len() > 64 {
        return Err(RustContextDiscoveryError::InvalidInput(
            "empty or excessive crate-type selection",
        ));
    }
    selected
        .iter()
        .map(|value| match value.as_str() {
            Some(
                kind @ ("lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "proc-macro" | "bin"),
            ) => Ok(kind.to_owned()),
            _ => Err(RustContextDiscoveryError::InvalidInput(
                "unknown crate-type selection",
            )),
        })
        .collect::<Result<BTreeSet<_>, _>>()
        .map(|selected| selected.into_iter().collect())
}

type ConfigurationInputs = (
    Vec<ContextArtifactInput>,
    Vec<ContextArtifactInput>,
    Option<Vec<String>>,
);

fn configuration_inputs(
    request: &RustContextDiscoveryRequest,
    files: &BTreeMap<Vec<u8>, &ContextFileInput>,
    manifest: &ContextFileInput,
    target_triple: &str,
    remainders: &mut Vec<RustContextRemainder>,
) -> Result<ConfigurationInputs, RustContextDiscoveryError> {
    let mut configurations = vec![artifact(manifest)];
    let mut locks = Vec::new();
    let mut build_flags = None;
    let mut target_flags = None;
    let mut incomplete_flags = false;
    // Cargo merges ancestor arrays before more specific arrays. The supplied search order
    // is most specific first; no daemon-home configuration participates implicitly.
    // https://doc.rust-lang.org/cargo/reference/config.html#hierarchical-structure
    for root in request.search_scope.ordered_roots.iter().rev() {
        for name in [
            b"Cargo.lock".as_slice(),
            b"Cargo.toml",
            b"rust-toolchain",
            b"rust-toolchain.toml",
            b"build.rs",
        ] {
            let Some(file) = files.get(&joined_path(&root.relative_path, name)) else {
                continue;
            };
            if name == b"Cargo.lock" {
                locks.push(artifact(file));
                continue;
            }
            if file.file_id != manifest.file_id {
                configurations.push(artifact(file));
            }
        }
        // The extensionless name wins when both candidates exist. Both lookups are retained.
        let file = files
            .get(&joined_path(&root.relative_path, b".cargo/config"))
            .or_else(|| files.get(&joined_path(&root.relative_path, b".cargo/config.toml")));
        let Some(file) = file else { continue };
        configurations.push(artifact(file));
        let document = parse_toml(file)?;
        incomplete_flags |= document.get("include").is_some()
            || document
                .get("target")
                .and_then(toml::Value::as_table)
                .is_some_and(|targets| targets.keys().any(|key| key.starts_with("cfg(")));
        merge_flags(
            document
                .get("build")
                .and_then(|value| value.get("rustflags")),
            &mut build_flags,
        )?;
        merge_flags(
            document
                .get("target")
                .and_then(|value| value.get(target_triple))
                .and_then(|value| value.get("rustflags")),
            &mut target_flags,
        )?;
    }
    if incomplete_flags {
        remainders.push(RustContextRemainder::UnsupportedCargoConfiguration);
    }
    let environment = &request.selection.environment;
    let explicit_flags = environment
        .iter()
        .find(|value| value.name == "CARGO_ENCODED_RUSTFLAGS")
        .map(|value| {
            value
                .value
                .split('\u{1f}')
                .filter(|part| !part.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .or_else(|| {
            environment
                .iter()
                .find(|value| value.name == "RUSTFLAGS")
                .map(|value| value.value.split_whitespace().map(str::to_owned).collect())
        });
    let flags = if let Some(flags) = explicit_flags {
        Some(flags)
    } else if incomplete_flags {
        None
    } else {
        Some(target_flags.or(build_flags).unwrap_or_default())
    };
    Ok((configurations, locks, flags))
}

fn merge_flags(
    value: Option<&toml::Value>,
    merged: &mut Option<Vec<String>>,
) -> Result<(), RustContextDiscoveryError> {
    let Some(value) = value else { return Ok(()) };
    if let Some(value) = value.as_str() {
        *merged = Some(value.split_whitespace().map(str::to_owned).collect());
    } else if let Some(values) = value.as_array() {
        let flags = merged.get_or_insert_with(Vec::new);
        for value in values {
            flags.push(
                value
                    .as_str()
                    .ok_or(RustContextDiscoveryError::InvalidInput(
                        "non-string rustflags",
                    ))?
                    .to_owned(),
            );
        }
    } else {
        return Err(RustContextDiscoveryError::InvalidInput(
            "invalid rustflags shape",
        ));
    }
    Ok(())
}

fn canonical_manifest(
    scope: &ContextSearchScope,
    settings: &RustCompilationSettings,
) -> Result<Vec<u8>, RustContextDiscoveryError> {
    // Scope closure and generation belong to support evidence, not semantic environment identity.
    let value = serde_json::json!({"settings": settings, "namespace": scope.namespace,
        "ordered_roots": scope.ordered_roots, "policy_identity": scope.policy_identity});
    crate::contracts::jcs::canonicalize_value(&value)
        .map_err(|error| RustContextDiscoveryError::Canonical(error.to_string()))
}

fn artifact(file: &ContextFileInput) -> ContextArtifactInput {
    ContextArtifactInput {
        file_id: file.file_id.clone(),
        digest: file.digest,
    }
}

fn parse_toml(file: &ContextFileInput) -> Result<toml::Value, RustContextDiscoveryError> {
    let text = std::str::from_utf8(&file.contents)
        .map_err(|_| RustContextDiscoveryError::InvalidInput("non-UTF8 Cargo configuration"))?;
    toml::from_str(text)
        .map_err(|error: toml::de::Error| RustContextDiscoveryError::InvalidToml(error.to_string()))
}

fn joined_path(root: &[u8], name: &[u8]) -> Vec<u8> {
    if root == b"." {
        return name.to_vec();
    }
    let mut result = root.to_vec();
    result.push(b'/');
    result.extend_from_slice(name);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_context::{ContextSearchRoot, ContextSearchUniverse, RustTargetKind};
    use crate::identity::{IdentityDomain, encode_public_id};

    fn input(path: &[u8], contents: &[u8]) -> ContextFileInput {
        ContextFileInput {
            file_id: format!("file:{}", String::from_utf8_lossy(path)),
            relative_path: path.to_vec(),
            digest: crate::integrity::digest_bytes(contents),
            contents: contents.to_vec(),
        }
    }

    fn request() -> RustContextDiscoveryRequest {
        RustContextDiscoveryRequest {
            workspace_id: encode_public_id(IdentityDomain::Workspace, None, [9; 16]).unwrap(),
            source_generation: 5, provider_bundle_version: "rust-providers-1".to_owned(),
            files: vec![
                input(b"Cargo.toml", b"[package]\nname='sample'\nversion='0.1.0'\nedition='2024'\n[lib]\npath='src/lib.rs'\n"),
                input(b"src/lib.rs", b"pub fn value() -> u32 { 1 }\n"),
                input(b"Cargo.lock", b"version = 4\n"),
            ],
            search_scope: ContextSearchScope {
                namespace: b".".to_vec(), ordered_roots: vec![ContextSearchRoot {
                    root_id: "path:root".to_owned(), relative_path: b".".to_vec(),
                }], policy_identity: [8; 32], universe: ContextSearchUniverse::Closed { inventory_identity: [7; 32] },
            },
            selection: RustContextSelection {
                target: Some(RustTargetSettings { name: "sample".to_owned(), kind: RustTargetKind::Library, crate_root: b"src/lib.rs".to_vec() }),
                requested_features: vec!["one".to_owned()], default_features: true,
                cfgs: vec![RustCfgSetting { name: "test_mode".to_owned(), value: None }],
                target_triple: Some("x86_64-unknown-linux-gnu".to_owned()), profile: Some("dev".to_owned()),
                toolchain: Some(RustToolchainSettings { release: "nightly-2026-08-18".to_owned(), commit_hash: "compiler-commit".to_owned(), artifact_digest: [4; 32] }),
                sysroot: Some(ContextArtifactInput { file_id: "sysroot:approved".to_owned(), digest: [3; 32] }),
                ..RustContextSelection::default()
            },
        }
    }

    fn prepared(request: &RustContextDiscoveryRequest) -> Box<RustContextDiscoveryProduct> {
        let RustContextDiscoveryOutcome::Prepared(product) =
            discover_rust_context(request).unwrap()
        else {
            panic!("expected prepared context with explicit resolution remainders")
        };
        product.validate().unwrap();
        product
    }

    #[test]
    fn rust_context_discovers_exact_library_and_example_linkage() {
        let mut request = request();
        let before = prepared(&request);
        assert_eq!(before.settings.crate_types, ["lib"]);
        let manifest = "[package]\nname='sample'\nversion='0.1.0'\nedition='2024'\n[lib]\ncrate-type=['rlib', 'cdylib', 'rlib']\n[[example]]\nname='sample'\npath='src/lib.rs'\ncrate-type=['staticlib']\n";
        request.files[0] = input(b"Cargo.toml", manifest.as_bytes());
        let library = prepared(&request);
        assert_eq!(library.settings.crate_types, ["cdylib", "rlib"]);
        assert_ne!(
            before.context.analysis_context_id,
            library.context.analysis_context_id
        );
        request.selection.target.as_mut().unwrap().kind = RustTargetKind::Example;
        assert_eq!(prepared(&request).settings.crate_types, ["staticlib"]);
        request.selection.target.as_mut().unwrap().kind = RustTargetKind::Binary;
        assert_eq!(prepared(&request).settings.crate_types, ["bin"]);
        request.selection.target.as_mut().unwrap().kind = RustTargetKind::Library;
        for invalid in ["[]", "['unknown']", "'rlib'", "[3]"] {
            request.files[0] = input(
                b"Cargo.toml",
                format!("[package]\nname='sample'\nversion='0.1.0'\n[lib]\ncrate-type={invalid}\n")
                    .as_bytes(),
            );
            assert!(discover_rust_context(&request).is_err(), "{invalid}");
        }
    }

    #[test]
    fn rust_context_settings_are_causal_without_claiming_cargo_resolution() {
        let mut request = request();
        let before = prepared(&request);
        assert_eq!(before.settings.package_name, "sample");
        assert_eq!(before.settings.edition, "2024");
        assert_eq!(before.settings.requested_features, ["one"]);
        assert!(
            before
                .remainders
                .contains(&RustContextRemainder::CargoMetadataRequired)
        );
        assert!(
            before
                .remainders
                .contains(&RustContextRemainder::DependencyResolutionRequired)
        );
        request.files[1] = input(b"src/lib.rs", b"pub fn value() -> u32 { 2 }\n");
        request.source_generation += 1;
        request.search_scope.universe = ContextSearchUniverse::Closed {
            inventory_identity: [6; 32],
        };
        let source_changed = prepared(&request);
        assert_eq!(
            source_changed.context.analysis_context_id,
            before.context.analysis_context_id
        );
        assert_eq!(source_changed.settings, before.settings);
        let absence = source_changed
            .lookup_evidence
            .iter()
            .find(|item| item.relative_path == b".cargo/config.toml")
            .unwrap();
        assert_eq!(absence.observation, ContextLookupObservation::Absent);
        request.files.push(input(
            b".cargo/config.toml",
            b"[target.x86_64-unknown-linux-gnu]\nrustflags=['--cfg', 'configured']\n",
        ));
        let configured = prepared(&request);
        assert_eq!(
            configured.settings.configured_rustflags,
            Some(vec!["--cfg".to_owned(), "configured".to_owned()])
        );
        assert_ne!(
            configured.context.analysis_context_id,
            source_changed.context.analysis_context_id
        );
        request.selection.requested_features.push("two".to_owned());
        let features = prepared(&request);
        assert_ne!(
            features.context.analysis_context_id,
            configured.context.analysis_context_id
        );
        request
            .selection
            .toolchain
            .as_mut()
            .unwrap()
            .artifact_digest = [22; 32];
        assert_ne!(
            prepared(&request).context.analysis_context_id,
            features.context.analysis_context_id
        );
        let mut forged = configured;
        forged.remainders.clear();
        assert!(forged.validate().is_err());
    }

    #[test]
    fn rust_flags_obey_selection_precedence_and_keep_unresolved_config_explicit() {
        let mut request = request();
        request.files.push(input(b".cargo/config", b"[build]\nrustflags=['--cfg','ignored_build']\n[target.x86_64-unknown-linux-gnu]\nrustflags=['--cfg','selected_target']\n"));
        request.files.push(input(
            b".cargo/config.toml",
            b"[build]\nrustflags=['--cfg','ignored_extension']\n",
        ));
        let product = prepared(&request);
        assert_eq!(
            product.settings.configured_rustflags,
            Some(vec!["--cfg".to_owned(), "selected_target".to_owned()])
        );
        assert_eq!(
            product
                .lookup_evidence
                .iter()
                .filter(|item| item.kind == ContextLookupKind::CargoConfiguration
                    && matches!(item.observation, ContextLookupObservation::Present { .. }))
                .count(),
            2
        );
        request.files[3] = input(
            b".cargo/config",
            b"[target.'cfg(unix)']\nrustflags=['--cfg','conditional']\n",
        );
        let unresolved = prepared(&request);
        assert!(unresolved.settings.configured_rustflags.is_none());
        assert!(
            unresolved
                .remainders
                .contains(&RustContextRemainder::UnsupportedCargoConfiguration)
        );
        request.selection.environment.push(RustEnvironmentSetting {
            name: "CARGO_ENCODED_RUSTFLAGS".to_owned(),
            value: "--cfg\u{1f}authorized_env".to_owned(),
        });
        let explicit = prepared(&request);
        assert_eq!(
            explicit.settings.configured_rustflags,
            Some(vec!["--cfg".to_owned(), "authorized_env".to_owned()])
        );
        let mut omitted = explicit;
        omitted
            .lookup_evidence
            .retain(|item| item.relative_path != b"Cargo.lock");
        assert!(omitted.validate().is_err());
    }

    #[test]
    fn rust_context_preserves_ambiguous_inherited_and_incomplete_selection() {
        let mut request = request();
        request.files.push(input(
            b"other/Cargo.toml",
            b"[package]\nname='other'\nversion='0.2.0'\n",
        ));
        assert!(
            matches!(discover_rust_context(&request).unwrap(), RustContextDiscoveryOutcome::Ambiguous { manifest_paths, .. } if manifest_paths.len() == 2)
        );
        request.selection.package_name = Some("sample".to_owned());
        assert_eq!(prepared(&request).settings.package_name, "sample");
        request.selection.target = None;
        assert!(
            matches!(discover_rust_context(&request).unwrap(), RustContextDiscoveryOutcome::Unresolved { reasons, .. } if reasons.contains(&RustContextRemainder::TargetSelectionRequired))
        );
        request = self::request();
        request.files[0] = input(
            b"Cargo.toml",
            b"[package]\nname='sample'\nversion.workspace=true\n",
        );
        assert!(
            matches!(discover_rust_context(&request).unwrap(), RustContextDiscoveryOutcome::Unresolved { reasons, .. } if reasons.contains(&RustContextRemainder::PackageMetadataInherited))
        );
        request = self::request();
        request.search_scope.universe = ContextSearchUniverse::Incomplete {
            observed_identity: [10; 32],
        };
        let product = prepared(&request);
        assert!(
            product
                .remainders
                .contains(&RustContextRemainder::IncompleteSearch)
        );
        assert!(
            product
                .lookup_evidence
                .iter()
                .any(|item| item.observation == ContextLookupObservation::Incomplete)
        );
        assert!(
            !product
                .lookup_evidence
                .iter()
                .any(|item| item.observation == ContextLookupObservation::Absent)
        );
    }
}
