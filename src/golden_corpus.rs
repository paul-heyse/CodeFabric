//! Owner-accepted executable golden-corpus authority.
//!
//! The model renderer never writes this corpus or its acceptance. This reader validates the
//! closed manifest and exact profile bytes before any golden answer is used as an oracle.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::contracts::jcs::canonicalize_slice;
use crate::registries::{DURABLE_PUBLICATION_STATE_VALUES, PROVIDER_IDS};
use crate::schema_registry::table_specs;

const REQUIRED_MEMBER_ROOTS: [&str; 3] = ["expected", "scenarios", "workspace"];
pub const LEGACY_CORPUS_ID: &str = "codefabric-golden-v1";
pub const LEGACY_CORPUS_VERSION: &str = "1.0";
pub const RELEASED_CORPUS_ID: &str = "codefabric-golden-v2";
pub const RELEASED_CORPUS_VERSION: &str = "2.0.0";
pub const GATE_B_PROFILE_ID: &str = "gate-b-v1";
pub const LEGACY_CORPUS_DIRECTORY: &str = "tests/golden/codefabric-golden-v1";
pub const RELEASED_CORPUS_DIRECTORY: &str = "tests/golden/codefabric-golden-v2";
pub const CORPUS_INDEX_PATH: &str = "tests/golden/corpus-index.json";
pub(crate) const CORPUS_INDEX_ARTIFACT_ID: &str = "codefabric.golden.corpus-index";
pub(crate) const REQUIRED_EXPECTED_GROUPS: [&str; 11] = [
    "canonical_tables",
    "diagnostics",
    "identities",
    "mcp",
    "provider_observations",
    "publications",
    "queries",
    "rebuild_comparison",
    "rpc",
    "serving_snapshots",
    "source_inventory",
];
pub(crate) const REQUIRED_SCENARIOS: [&str; 16] = [
    "000_clean_bootstrap",
    "010_python_local_edit",
    "020_python_import_surface_change",
    "030_python_parse_failure_and_recovery",
    "040_rust_body_edit",
    "050_rust_public_signature_change",
    "060_rust_compile_failure_and_recovery",
    "070_rename_and_case_change",
    "080_multi_file_logical_save",
    "090_context_change",
    "100_generated_source_change",
    "110_watcher_loss_reconciliation",
    "120_hot_overlay_flush",
    "130_daemon_restart",
    "140_capability_withdrawal",
    "150_source_acl_redaction",
];

const SCENARIO_DOCUMENT: &str = "scenario.json";
const REQUIRED_SCENARIO_EDITS: [&str; 18] = [
    "add-python-import",
    "break-python",
    "break-rust",
    "change-context",
    "change-generated-source",
    "change-rust-signature",
    "drop-watch-hint",
    "flush-overlay",
    "multi-file-save",
    "redact-source",
    "rename-and-case-change",
    "repair-python",
    "repair-rust",
    "replace-python-body",
    "replace-rust-body",
    "rescan",
    "restart-daemon",
    "withdraw-capability",
];

/// Closed executable edit-scenario document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioDefinition {
    pub scenario_id: String,
    pub edits: Vec<String>,
    pub expected_terminal: ScenarioTerminal,
}

/// Terminal freshness class asserted by an executable scenario.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScenarioTerminal {
    Current,
    Partial,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusManifest {
    pub corpus_id: String,
    pub corpus_version: String,
    pub corpus_status: CorpusStatus,
    pub coverage_profiles: Vec<CoverageProfile>,
    pub accepted_profile_digests: Vec<String>,
    pub source_archive_digest: String,
    pub workspace_registration_digest: String,
    pub context_manifest_digests: Vec<String>,
    pub provider_bundle_digests: Vec<String>,
    pub model_pack_bundle_digest: String,
    pub ontology_bundle_digest: String,
    pub schema_bundle_digest: String,
    pub derivation_bundle_digest: String,
    pub query_bundle_digest: String,
    pub tool_contract_bundle_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<CorpusSupersedes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance_artifact: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released_candidate_digest: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusSupersedes {
    pub corpus_id: String,
    pub corpus_version: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CorpusStatus {
    Candidate,
    Released,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageProfile {
    pub profile_id: String,
    pub profile_version: String,
    pub profile_status: CorpusStatus,
    pub member_roots: Vec<String>,
    pub file_count: usize,
    pub canonical_digest: String,
    pub owned_requirements: Vec<String>,
    pub acceptance: OwnerAcceptance,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerAcceptance {
    pub accepted_by: String,
    pub accepted_at: String,
    pub acceptance_basis: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CorpusIndexEntry {
    pub(crate) corpus_id: String,
    pub(crate) corpus_version: String,
    pub(crate) corpus_status: CorpusStatus,
    pub(crate) path: String,
    pub(crate) manifest_digest: String,
    pub(crate) profile_id: String,
    pub(crate) profile_digest: String,
    pub(crate) acceptance_digest: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CorpusIndex {
    pub(crate) schema_version: u16,
    pub(crate) artifact_id: String,
    pub(crate) artifact_kind: String,
    pub(crate) version: String,
    pub(crate) status: CorpusStatus,
    pub(crate) current_corpus_id: String,
    pub(crate) current_corpus_version: String,
    pub(crate) entries: Vec<CorpusIndexEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedCoverageProfile {
    pub profile_id: String,
    pub canonical_digest: String,
    pub files: Vec<PathBuf>,
}

/// Independently executed Gate-B artifact identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateBExecution {
    pub profile_digest: String,
    pub artifact_digests: BTreeMap<String, String>,
    pub execution_digest: String,
}

/// Exact owner-accepted coverage attached to the `CORE_SOURCE_V1` completeness claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreSourceCoverage {
    pub precision_profile: &'static str,
    pub coverage_profile_id: String,
    pub coverage_profile_digest: String,
    pub scenario_ids: BTreeSet<String>,
}

#[derive(Debug, Error)]
pub enum CorpusError {
    #[error("golden corpus I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("golden corpus manifest is invalid: {0}")]
    Manifest(String),
    #[error("golden corpus invariant failed: {0}")]
    Invariant(String),
    #[error("Gate-B artifact execution failed: {0}")]
    Artifact(String),
}

fn read(path: &Path) -> Result<Vec<u8>, CorpusError> {
    fs::read(path).map_err(|source| CorpusError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn valid_digest(value: &str) -> bool {
    value.len() == 67
        && value.starts_with("b3:")
        && value[3..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn safe_relative(value: &str) -> Result<&Path, CorpusError> {
    let path = Path::new(value);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(CorpusError::Invariant(format!(
            "unsafe profile member root {value:?}"
        )));
    }
    Ok(path)
}

/// Resolve and validate the immutable corpus selected by the atomic current-version index.
///
/// # Errors
///
/// Returns an error when the index, current entry, manifest identity, acceptance digest, or
/// owner-accepted profile differs from the supported released corpus contract.
pub fn current_released_corpus_root(repository_root: &Path) -> Result<PathBuf, CorpusError> {
    let index_path = repository_root.join(CORPUS_INDEX_PATH);
    let index: CorpusIndex = serde_json::from_slice(&read(&index_path)?)
        .map_err(|error| CorpusError::Manifest(error.to_string()))?;
    if index.schema_version != 1
        || index.artifact_id != CORPUS_INDEX_ARTIFACT_ID
        || index.artifact_kind != "golden-corpus-index"
        || index.version != "1.0"
        || index.status != CorpusStatus::Released
        || index.current_corpus_id != RELEASED_CORPUS_ID
        || index.current_corpus_version != RELEASED_CORPUS_VERSION
    {
        return Err(CorpusError::Invariant(
            "golden corpus current-version index differs".to_owned(),
        ));
    }
    let current = index
        .entries
        .iter()
        .find(|entry| {
            entry.corpus_id == index.current_corpus_id
                && entry.corpus_version == index.current_corpus_version
        })
        .ok_or_else(|| CorpusError::Invariant("current corpus index entry is absent".to_owned()))?;
    if current.corpus_status != CorpusStatus::Released
        || current.path != RELEASED_CORPUS_DIRECTORY
        || current.profile_id != GATE_B_PROFILE_ID
        || !current
            .acceptance_digest
            .as_deref()
            .is_some_and(valid_digest)
    {
        return Err(CorpusError::Invariant(
            "current corpus index entry differs".to_owned(),
        ));
    }
    let corpus_root = repository_root.join(safe_relative(&current.path)?);
    let manifest_bytes = read(&corpus_root.join("corpus-manifest.json"))?;
    if crate::integrity::framed_digest(&manifest_bytes) != current.manifest_digest {
        return Err(CorpusError::Invariant(
            "current corpus manifest digest differs".to_owned(),
        ));
    }
    let manifest: CorpusManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| CorpusError::Manifest(error.to_string()))?;
    let profile = validate_profile(&corpus_root, &current.profile_id)?;
    if manifest.corpus_id != current.corpus_id
        || manifest.corpus_version != current.corpus_version
        || manifest.corpus_status != current.corpus_status
        || manifest.acceptance_digest != current.acceptance_digest
        || profile.canonical_digest != current.profile_digest
    {
        return Err(CorpusError::Invariant(
            "current corpus manifest and index entry differ".to_owned(),
        ));
    }
    Ok(corpus_root)
}

fn collect_files(
    root: &Path,
    relative: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), CorpusError> {
    let directory = root.join(relative);
    let metadata = fs::symlink_metadata(&directory).map_err(|source| CorpusError::Io {
        path: directory.clone(),
        source,
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CorpusError::Invariant(format!(
            "member root {} is not a real directory",
            relative.display()
        )));
    }

    let mut entries = fs::read_dir(&directory)
        .map_err(|source| CorpusError::Io {
            path: directory.clone(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| CorpusError::Io {
            path: directory.clone(),
            source,
        })?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let entry_path = entry.path();
        let entry_metadata = entry.metadata().map_err(|source| CorpusError::Io {
            path: entry_path.clone(),
            source,
        })?;
        if entry.file_type().is_ok_and(|kind| kind.is_symlink()) {
            return Err(CorpusError::Invariant(format!(
                "symlink is not an accepted profile member: {}",
                entry_path.display()
            )));
        }
        let child = relative.join(entry.file_name());
        if entry_metadata.is_dir() {
            collect_files(root, &child, files)?;
        } else if entry_metadata.is_file() {
            files.push(child);
        } else {
            return Err(CorpusError::Invariant(format!(
                "non-file profile member: {}",
                entry_path.display()
            )));
        }
    }
    Ok(())
}

fn digest_files(root: &Path, files: &[PathBuf]) -> Result<String, CorpusError> {
    let mut hasher = crate::integrity::IntegrityHasher::for_domain(
        crate::integrity::IntegrityDomain::GoldenProfile,
    );
    for relative in files {
        let relative = relative.to_str().ok_or_else(|| {
            CorpusError::Invariant(format!("profile path is not UTF-8: {}", relative.display()))
        })?;
        let bytes = read(&root.join(relative))?;
        let path_bytes = relative.as_bytes();
        hasher.update(&(path_bytes.len() as u64).to_be_bytes());
        hasher.update(path_bytes);
        hasher.update(&(bytes.len() as u64).to_be_bytes());
        hasher.update(&bytes);
    }
    hasher.update(&(files.len() as u64).to_be_bytes());
    Ok(crate::integrity::frame_digest(hasher.finalize()))
}

pub(crate) fn compute_required_profile(corpus_root: &Path) -> Result<(usize, String), CorpusError> {
    let mut files = Vec::new();
    for member_root in REQUIRED_MEMBER_ROOTS {
        collect_files(corpus_root, Path::new(member_root), &mut files)?;
    }
    files.sort();
    let digest = digest_files(corpus_root, &files)?;
    Ok((files.len(), digest))
}

fn child_directories(path: &Path) -> Result<BTreeSet<String>, CorpusError> {
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(path)
        .map_err(|source| CorpusError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| CorpusError::Io {
            path: path.to_path_buf(),
            source,
        })?
    {
        if entry
            .file_type()
            .map_err(|source| CorpusError::Io {
                path: entry.path(),
                source,
            })?
            .is_dir()
        {
            names.insert(entry.file_name().to_string_lossy().into_owned());
        }
    }
    Ok(names)
}

fn scenario_definitions(corpus_root: &Path) -> Result<Vec<ScenarioDefinition>, CorpusError> {
    let allowed_edits = REQUIRED_SCENARIO_EDITS
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let mut definitions = Vec::with_capacity(REQUIRED_SCENARIOS.len());
    let mut observed_edits = BTreeSet::new();
    for scenario_id in REQUIRED_SCENARIOS {
        let path = corpus_root
            .join("scenarios")
            .join(scenario_id)
            .join(SCENARIO_DOCUMENT);
        let definition: ScenarioDefinition = serde_json::from_slice(&read(&path)?)
            .map_err(|error| CorpusError::Manifest(error.to_string()))?;
        if definition.scenario_id != scenario_id {
            return Err(CorpusError::Invariant(format!(
                "scenario document identity differs for {scenario_id}"
            )));
        }
        if definition
            .edits
            .iter()
            .any(|edit| !allowed_edits.contains(edit) || !observed_edits.insert(edit.clone()))
        {
            return Err(CorpusError::Invariant(format!(
                "scenario {scenario_id} contains an unknown or duplicate edit"
            )));
        }
        let withdrawal = definition
            .edits
            .iter()
            .any(|edit| edit == "withdraw-capability");
        if (definition.expected_terminal == ScenarioTerminal::Partial) != withdrawal {
            return Err(CorpusError::Invariant(format!(
                "scenario {scenario_id} terminal does not match capability withdrawal"
            )));
        }
        definitions.push(definition);
    }
    if observed_edits != allowed_edits {
        return Err(CorpusError::Invariant(
            "scenario edit census differs from the executable runner".to_owned(),
        ));
    }
    Ok(definitions)
}

/// Load the exact closed executable scenario set.
///
/// # Errors
///
/// Returns an error when a document is absent, malformed, misidentified, duplicates an edit,
/// names an unsupported edit, or disagrees with its expected terminal state.
pub fn load_scenarios(corpus_root: &Path) -> Result<Vec<ScenarioDefinition>, CorpusError> {
    scenario_definitions(corpus_root)
}

/// Load and validate one owner-accepted coverage profile against exact current bytes.
///
/// # Errors
///
/// Returns an error for missing or unreadable corpus members, malformed manifests, path escapes,
/// digest drift, or an unaccepted profile.
#[allow(clippy::too_many_lines)] // One validation pass keeps corpus identity, acceptance, membership, and digest checks visibly coupled.
pub fn validate_profile(
    corpus_root: &Path,
    profile_id: &str,
) -> Result<ValidatedCoverageProfile, CorpusError> {
    let manifest_path = corpus_root.join("corpus-manifest.json");
    let manifest: CorpusManifest = serde_json::from_slice(&read(&manifest_path)?)
        .map_err(|error| CorpusError::Manifest(error.to_string()))?;
    let legacy = manifest.corpus_id == LEGACY_CORPUS_ID
        && manifest.corpus_version == LEGACY_CORPUS_VERSION
        && manifest.corpus_status == CorpusStatus::Candidate
        && manifest.supersedes.is_none()
        && manifest.acceptance_artifact.is_none()
        && manifest.acceptance_digest.is_none()
        && manifest.released_candidate_digest.is_none();
    let released = manifest.corpus_id == RELEASED_CORPUS_ID
        && manifest.corpus_version == RELEASED_CORPUS_VERSION
        && manifest.corpus_status == CorpusStatus::Released
        && manifest.supersedes.as_ref().is_some_and(|supersedes| {
            supersedes.corpus_id == LEGACY_CORPUS_ID
                && supersedes.corpus_version == LEGACY_CORPUS_VERSION
        })
        && manifest.acceptance_artifact.as_deref() == Some("owner-acceptance.json")
        && manifest
            .acceptance_digest
            .as_deref()
            .is_some_and(valid_digest)
        && manifest
            .released_candidate_digest
            .as_deref()
            .is_some_and(valid_digest);
    if !legacy && !released {
        return Err(CorpusError::Invariant(
            "unexpected corpus identity or version".to_owned(),
        ));
    }
    let mut profile_ids = BTreeSet::new();
    if manifest
        .coverage_profiles
        .iter()
        .any(|profile| !profile_ids.insert(profile.profile_id.as_str()))
    {
        return Err(CorpusError::Invariant(
            "coverage profile IDs must be unique".to_owned(),
        ));
    }
    let profile = manifest
        .coverage_profiles
        .iter()
        .find(|profile| profile.profile_id == profile_id)
        .ok_or_else(|| CorpusError::Invariant(format!("unknown coverage profile {profile_id}")))?;
    if (legacy && profile.profile_version != "1.0")
        || (released && profile.profile_version != "2.0")
    {
        return Err(CorpusError::Invariant(format!(
            "coverage profile {profile_id} has the wrong version"
        )));
    }
    if profile.profile_status != CorpusStatus::Released
        || profile.acceptance.accepted_by.trim().is_empty()
        || profile.acceptance.acceptance_basis.trim().is_empty()
        || !manifest
            .accepted_profile_digests
            .contains(&profile.canonical_digest)
    {
        return Err(CorpusError::Invariant(format!(
            "coverage profile {profile_id} lacks owner acceptance"
        )));
    }
    let member_roots = profile
        .member_roots
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if member_roots != REQUIRED_MEMBER_ROOTS.into_iter().collect() {
        return Err(CorpusError::Invariant(format!(
            "coverage profile {profile_id} has the wrong member roots"
        )));
    }
    let expected = child_directories(&corpus_root.join("expected"))?;
    if expected
        != REQUIRED_EXPECTED_GROUPS
            .into_iter()
            .map(str::to_owned)
            .collect()
    {
        return Err(CorpusError::Invariant(
            "expected-output logical group census differs".to_owned(),
        ));
    }
    let scenarios = child_directories(&corpus_root.join("scenarios"))?;
    if scenarios != REQUIRED_SCENARIOS.into_iter().map(str::to_owned).collect() {
        return Err(CorpusError::Invariant(
            "scenario logical group census differs".to_owned(),
        ));
    }
    scenario_definitions(corpus_root)?;
    let mut files = Vec::new();
    for member_root in &profile.member_roots {
        collect_files(corpus_root, safe_relative(member_root)?, &mut files)?;
    }
    files.sort();
    if files.len() != profile.file_count {
        return Err(CorpusError::Invariant(format!(
            "profile file count differs: expected {}, observed {}",
            profile.file_count,
            files.len()
        )));
    }
    let canonical_digest = digest_files(corpus_root, &files)?;
    if canonical_digest != profile.canonical_digest {
        return Err(CorpusError::Invariant(format!(
            "profile digest differs: expected {}, observed {canonical_digest}",
            profile.canonical_digest
        )));
    }
    Ok(ValidatedCoverageProfile {
        profile_id: profile.profile_id.clone(),
        canonical_digest,
        files,
    })
}

/// Execute all eleven accepted Gate-B artifact contracts against live generated authorities.
///
/// This never rewrites an answer. Each answer is strictly decoded and canonicalized, then its
/// externally meaningful assertions are checked against the corpus bytes or current generated
/// registries before its identity participates in the combined execution digest.
///
/// # Errors
///
/// Returns an error when the released profile or any required artifact is absent, malformed,
/// non-canonical, or inconsistent with current generated authority.
pub fn execute_gate_b_artifacts(corpus_root: &Path) -> Result<GateBExecution, CorpusError> {
    let profile = validate_profile(corpus_root, GATE_B_PROFILE_ID)?;
    let mut artifact_digests = BTreeMap::new();
    for group in REQUIRED_EXPECTED_GROUPS {
        let path = corpus_root.join("expected").join(group).join("gate-b.json");
        let canonical = canonicalize_slice(&read(&path)?)
            .map_err(|error| CorpusError::Artifact(error.to_string()))?;
        let value: Value = serde_json::from_slice(&canonical)
            .map_err(|error| CorpusError::Artifact(error.to_string()))?;
        if text_field(&value, "profile")? != GATE_B_PROFILE_ID {
            return Err(CorpusError::Artifact(format!(
                "{group} belongs to a different profile"
            )));
        }
        execute_artifact_contract(corpus_root, group, &value)?;
        artifact_digests.insert(
            group.to_owned(),
            crate::integrity::framed_digest(&canonical),
        );
    }
    let mut combined = crate::integrity::IntegrityHasher::for_domain(
        crate::integrity::IntegrityDomain::GateBExecution,
    );
    combined.update(profile.canonical_digest.as_bytes());
    for (group, digest) in &artifact_digests {
        combined.update(&(group.len() as u64).to_be_bytes());
        combined.update(group.as_bytes());
        combined.update(digest.as_bytes());
    }
    Ok(GateBExecution {
        profile_digest: profile.canonical_digest,
        artifact_digests,
        execution_digest: crate::integrity::frame_digest(combined.finalize()),
    })
}

/// Advertise `CORE_SOURCE_V1` only with the exact released corpus coverage that proved it.
///
/// # Errors
///
/// Returns an error unless Gate B executes successfully and the exact required scenario census is
/// present.
pub fn core_source_v1_coverage(corpus_root: &Path) -> Result<CoreSourceCoverage, CorpusError> {
    let profile = validate_profile(corpus_root, GATE_B_PROFILE_ID)?;
    execute_gate_b_artifacts(corpus_root)?;
    let scenario_ids = child_directories(&corpus_root.join("scenarios"))?;
    if scenario_ids != REQUIRED_SCENARIOS.into_iter().map(str::to_owned).collect() {
        return Err(CorpusError::Artifact(
            "CORE_SOURCE_V1 scenario coverage differs".to_owned(),
        ));
    }
    Ok(CoreSourceCoverage {
        precision_profile: "CORE_SOURCE_V1",
        coverage_profile_id: profile.profile_id,
        coverage_profile_digest: profile.canonical_digest,
        scenario_ids,
    })
}

#[allow(clippy::too_many_lines)] // The closed eleven-family dispatch remains explicit and exhaustive.
fn execute_artifact_contract(
    corpus_root: &Path,
    group: &str,
    value: &Value,
) -> Result<(), CorpusError> {
    let exact_strings = |field: &str| string_array(value, field).map(BTreeSet::from_iter);
    match group {
        "source_inventory" => {
            for field in ["included", "inventory_only"] {
                for relative in string_array(value, field)? {
                    if !corpus_root.join("workspace").join(relative).is_file() {
                        return Err(CorpusError::Artifact(format!(
                            "source inventory member {relative} is absent"
                        )));
                    }
                }
            }
        }
        "provider_observations" => {
            let expected = exact_strings("providers")?;
            let gate_b_providers = BTreeSet::from([
                "ruff-python",
                "rustc-mir",
                "source-substrate",
                "tree-sitter",
            ]);
            let generated = PROVIDER_IDS
                .iter()
                .copied()
                .filter(|provider| gate_b_providers.contains(provider))
                .collect::<BTreeSet<_>>();
            if expected != generated
                || generated != gate_b_providers
                || !bool_field(value, "terminal_manifest_required")?
            {
                return Err(CorpusError::Artifact(
                    "provider observation authority differs".to_owned(),
                ));
            }
        }
        "canonical_tables" => {
            let gate_b_tables = BTreeSet::from([
                "entity",
                "fact_evidence",
                "property_fact",
                "relation",
                "source_annotation",
                "source_file",
                "source_token",
                "syntax_detail",
            ]);
            let generated = table_specs()
                .iter()
                .map(|spec| spec.name)
                .filter(|table| gate_b_tables.contains(table))
                .collect::<BTreeSet<_>>();
            if exact_strings("required_tables")? != generated
                || generated != gate_b_tables
                || text_field(value, "ordering")? != "primary-key"
            {
                return Err(CorpusError::Artifact(
                    "canonical table authority differs".to_owned(),
                ));
            }
        }
        "identities" => require(value, "identity_algorithm", "CBEF-v1")?,
        "diagnostics" => {
            if !bool_field(value, "source_bytes_forbidden")?
                || exact_strings("required_codes")?
                    != BTreeSet::from(["UNAVAILABLE_PARSE", "UNSUPPORTED_CONTENT"])
            {
                return Err(CorpusError::Artifact(
                    "diagnostic contract differs".to_owned(),
                ));
            }
        }
        "publications" => {
            if text_field(value, "publication_state")? != "COMPLETE"
                || !DURABLE_PUBLICATION_STATE_VALUES
                    .iter()
                    .any(|state| state.name == "COMPLETE")
                || !bool_field(value, "atomic_pointer")?
                || !bool_field(value, "delta_versions_pinned")?
            {
                return Err(CorpusError::Artifact(
                    "publication contract differs".to_owned(),
                ));
            }
        }
        "serving_snapshots" => {
            require(value, "freshness_state", "CURRENT")?;
            require(value, "catalog", "codefabric")?;
            if exact_strings("schemas")?
                != BTreeSet::from(["cpg_base", "cpg_control", "cpg_serving"])
            {
                return Err(CorpusError::Artifact(
                    "serving schema census differs".to_owned(),
                ));
            }
        }
        "queries" => {
            if exact_strings("forms")?
                != BTreeSet::from([
                    "find code entities",
                    "follow relationships",
                    "retrieve facts",
                ])
                || text_field(value, "ordering")? != "canonical"
            {
                return Err(CorpusError::Artifact("query contract differs".to_owned()));
            }
        }
        "rpc" => {
            require(value, "transport", "unix-domain-socket")?;
            if !bool_field(value, "deadline_required")?
                || !bool_field(value, "unknown_fields_preserved")?
            {
                return Err(CorpusError::Artifact("RPC contract differs".to_owned()));
            }
        }
        "mcp" => {
            require(value, "transport", "stdio")?;
            if !bool_field(value, "stdout_protocol_only")?
                || exact_strings("delivery_variants")? != BTreeSet::from(["inline", "resource"])
            {
                return Err(CorpusError::Artifact("MCP contract differs".to_owned()));
            }
        }
        "rebuild_comparison" => {
            require(value, "comparison", "canonical-effective-state")?;
            if !bool_field(value, "physical_delta_layout_ignored")?
                || !bool_field(value, "operational_ids_ignored")?
            {
                return Err(CorpusError::Artifact(
                    "rebuild comparison contract differs".to_owned(),
                ));
            }
        }
        _ => {
            return Err(CorpusError::Artifact(format!(
                "unknown artifact group {group}"
            )));
        }
    }
    Ok(())
}

fn text_field<'a>(value: &'a Value, field: &str) -> Result<&'a str, CorpusError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| CorpusError::Artifact(format!("{field} is absent or not text")))
}

fn bool_field(value: &Value, field: &str) -> Result<bool, CorpusError> {
    value
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| CorpusError::Artifact(format!("{field} is absent or not Boolean")))
}

fn string_array<'a>(value: &'a Value, field: &str) -> Result<Vec<&'a str>, CorpusError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| CorpusError::Artifact(format!("{field} is absent or not an array")))?
        .iter()
        .map(|member| {
            member
                .as_str()
                .ok_or_else(|| CorpusError::Artifact(format!("{field} contains a non-text member")))
        })
        .collect()
}

fn require(value: &Value, field: &str, expected: &str) -> Result<(), CorpusError> {
    if text_field(value, field)? == expected {
        Ok(())
    } else {
        Err(CorpusError::Artifact(format!("{field} differs")))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn corpus_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/codefabric-golden-v1")
    }

    fn copy_corpus() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        copy_directory(&corpus_root(), temp.path());
        temp
    }

    fn copy_directory(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let target = destination.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_directory(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    #[test]
    fn wp34_behavioral_acceptance() {
        let validated = validate_profile(&corpus_root(), "gate-b-v1").unwrap();
        assert_eq!(validated.files.len(), 39);
        assert_eq!(
            validated.canonical_digest,
            "b3:45205c097bae69e22afe344003fd356f9a6311714af015c4fce2521179b07dfd"
        );
    }

    #[test]
    fn wp34_negative_zero_state() {
        let temp = copy_corpus();
        fs::write(temp.path().join("expected/queries/gate-b.json"), b"{}\n").unwrap();
        assert!(validate_profile(temp.path(), "gate-b-v1").is_err());
        let missing = copy_corpus();
        fs::remove_file(missing.path().join("workspace/ffi/boundary.py")).unwrap();
        assert!(validate_profile(missing.path(), "gate-b-v1").is_err());

        let extra = copy_corpus();
        fs::write(extra.path().join("workspace/extra.py"), b"pass\n").unwrap();
        assert!(validate_profile(extra.path(), "gate-b-v1").is_err());
    }

    #[test]
    fn wp34_structural_acceptance() {
        let validated = validate_profile(&corpus_root(), "gate-b-v1").unwrap();
        assert_eq!(
            child_directories(&corpus_root().join("expected")).unwrap(),
            REQUIRED_EXPECTED_GROUPS
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
        assert_eq!(
            child_directories(&corpus_root().join("scenarios")).unwrap(),
            REQUIRED_SCENARIOS.into_iter().map(str::to_owned).collect()
        );
        assert!(validated.files.iter().all(|path| {
            REQUIRED_MEMBER_ROOTS
                .iter()
                .any(|root| path.starts_with(root))
        }));
    }

    #[test]
    fn wp34_operational_acceptance() {
        let first = validate_profile(&corpus_root(), "gate-b-v1").unwrap();
        let second = validate_profile(&corpus_root(), "gate-b-v1").unwrap();
        assert_eq!(first, second);
        assert_eq!(
            digest_files(&corpus_root(), &first.files).unwrap(),
            first.canonical_digest
        );
    }

    #[test]
    fn wp40_behavioral_acceptance() {
        let execution = execute_gate_b_artifacts(&corpus_root()).unwrap();
        assert_eq!(execution.artifact_digests.len(), 11);
        assert!(execution.execution_digest.starts_with("b3:"));
        assert_eq!(
            execution.profile_digest,
            validate_profile(&corpus_root(), "gate-b-v1")
                .unwrap()
                .canonical_digest
        );
    }

    #[test]
    fn wp40_structural_acceptance() {
        let execution = execute_gate_b_artifacts(&corpus_root()).unwrap();
        assert_eq!(
            execution
                .artifact_digests
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            REQUIRED_EXPECTED_GROUPS
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
    }

    #[test]
    fn wp40_negative_zero_state() {
        let temp = copy_corpus();
        fs::write(
            temp.path().join("expected/rpc/gate-b.json"),
            br#"{"transport":"tcp","deadline_required":true,"unknown_fields_preserved":true,"profile":"gate-b-v1"}"#,
        )
        .unwrap();
        assert!(execute_gate_b_artifacts(temp.path()).is_err());
    }

    #[test]
    fn wp40_operational_acceptance() {
        let first = execute_gate_b_artifacts(&corpus_root()).unwrap();
        let second = execute_gate_b_artifacts(&corpus_root()).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn wp48_structural_acceptance() {
        let coverage = core_source_v1_coverage(&corpus_root()).unwrap();
        assert_eq!(coverage.precision_profile, "CORE_SOURCE_V1");
        assert_eq!(coverage.coverage_profile_id, "gate-b-v1");
        assert_eq!(coverage.scenario_ids.len(), 16);
        assert_eq!(
            coverage.scenario_ids,
            REQUIRED_SCENARIOS.into_iter().map(str::to_owned).collect()
        );
    }
}
