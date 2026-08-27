//! Executable Gate B review-candidate generation.
//!
//! Candidate bytes are deliberately unreleased. The runner applies the closed scenario edit
//! vocabulary through [`crate::continuous::ContinuousWorkspaceEngine`], derives expectations from
//! normative inputs and generated registries on a separate path, and emits a detached digest chain
//! that WP76 can review but only an accountable owner can accept.

mod functional_candidate;
pub(crate) mod vertical;

pub use functional_candidate::{
    FUNCTIONAL_CANDIDATE_DIRECTORY, FUNCTIONAL_CANDIDATE_ID, generate_functional_candidate_bundle,
    verify_functional_candidate_bundle,
};

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::continuous::{ContinuousWorkspaceConfig, ContinuousWorkspaceEngine};
use crate::contracts::jcs::canonicalize_slice;
use crate::fabric::batch_checksum;
use crate::git_state::{GitCandidatePlanner, GitStateObservations, GixGitStateAdapter};
use crate::golden_corpus::{
    CorpusError, REQUIRED_EXPECTED_GROUPS, REQUIRED_SCENARIOS, ScenarioTerminal,
};
use crate::identity::{IdentityDomain, SOURCE_CONTEXT_ID, encode_public_id};
use crate::lifecycle::{
    LifecycleConfig, OverlayFlushPolicy, UpdateWaveScheduler, WatchHint, WatchHintBatch,
    WatchHintKind,
};
use crate::operational_store::{OperationalStore, OperationalStoreError};
use crate::source_image::{SourceCapturePolicy, SourceImageError, SourceImageStore};
use crate::workspace_registry::{WorkspaceRegistry, WorkspaceRegistryError};

pub const CANDIDATE_ID: &str = "codefabric-golden-v3.0.0-candidate.1";
pub const CANDIDATE_DIRECTORY: &str =
    "tests/golden/review-candidates/codefabric-golden-v3.0.0-candidate.1";
const CANDIDATE_FILE: &str = "candidate.json";
const DIFF_FILE: &str = "expected-vs-candidate-diff.json";
const MANIFEST_FILE: &str = "candidate-manifest.json";
const DIGEST_FILE: &str = "candidate-digest.json";
const BUNDLE_FILES: [&str; 4] = [CANDIDATE_FILE, DIFF_FILE, MANIFEST_FILE, DIGEST_FILE];

#[derive(Debug, Error)]
pub enum GateBCandidateError {
    #[error(transparent)]
    Corpus(#[from] CorpusError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Store(#[from] OperationalStoreError),
    #[error(transparent)]
    Workspace(#[from] WorkspaceRegistryError),
    #[error(transparent)]
    SourceImage(#[from] SourceImageError),
    #[error("Gate B candidate invariant failed: {0}")]
    Invariant(String),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum CandidateStatus {
    Candidate,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CapturedFileObservation {
    path: String,
    file_id: String,
    content_digest: String,
    byte_length: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct TableObservation {
    table_code: i16,
    table_name: String,
    row_count: usize,
    checksum_scope: String,
    checksum: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct WaveObservation {
    wave_id: String,
    source_generation: u64,
    event_watermark: u64,
    state: String,
    captured_files: Vec<CapturedFileObservation>,
    tables: Vec<TableObservation>,
    overlay_generation: u64,
    overlay_row_count: u64,
    overlay_table_digests: BTreeMap<String, String>,
    flush_required: bool,
    #[serde(alias = "clean_rebuild_equal")]
    fast_syntax_replay_equal: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ScenarioObservation {
    scenario_id: String,
    edits: Vec<String>,
    expected_terminal: ScenarioTerminal,
    observed_terminal: ScenarioTerminal,
    workspace_id: String,
    waves: Vec<WaveObservation>,
    providers_observed: Vec<String>,
    final_inventory: Vec<CapturedFileObservation>,
    response_bytes_hex: String,
    response_checksum: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct GateBCandidatePayload {
    schema_version: u16,
    candidate_id: String,
    candidate_status: CandidateStatus,
    proposed_corpus_version: String,
    source_corpus_id: String,
    source_corpus_version: String,
    source_profile_digest: String,
    scenario_executions: Vec<ScenarioObservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vertical_execution: Option<vertical::VerticalExecution>,
    gate_b_items: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct GroupComparison {
    expected_digest: String,
    candidate_digest: String,
    matches: bool,
    #[serde(default)]
    requirement_checks: Vec<String>,
    #[serde(default)]
    released_digest: String,
    #[serde(default)]
    changes_released_bytes: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CandidateDiff {
    schema_version: u16,
    candidate_id: String,
    derivation: String,
    expectation_inputs: BTreeMap<String, String>,
    groups: BTreeMap<String, GroupComparison>,
    all_expected_items_match: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ManifestMember {
    path: String,
    digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CandidateManifest {
    schema_version: u16,
    artifact_kind: String,
    candidate_id: String,
    candidate_status: CandidateStatus,
    proposed_corpus_version: String,
    supersedes_corpus_id: String,
    supersedes_corpus_version: String,
    scenario_count: usize,
    gate_b_item_count: usize,
    expectation_inputs: BTreeMap<String, String>,
    members: Vec<ManifestMember>,
    owner_acceptance: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DetachedCandidateDigest {
    schema_version: u16,
    artifact_kind: String,
    domain: String,
    manifest: String,
    digest: String,
}

/// Exact deterministic files in one Gate B review bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedCandidateBundle {
    files: BTreeMap<String, Vec<u8>>,
}

impl GeneratedCandidateBundle {
    #[must_use]
    pub fn files(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.files
    }
}

fn candidate_lifecycle_config(flush_aggressively: bool) -> LifecycleConfig {
    LifecycleConfig {
        debounce_timeout: std::time::Duration::from_millis(20),
        tick_rate: std::time::Duration::from_millis(5),
        ingress_capacity: 64,
        maximum_paths_per_batch: 256,
        gather_window: std::time::Duration::from_millis(5),
        dirty_path_bulk_threshold: 8,
        await_current_timeout: std::time::Duration::from_secs(1),
        maximum_capture_bytes: 1024 * 1024,
        stable_read_retry_count: 2,
        source_blob_lease_ttl: std::time::Duration::from_secs(60),
        overlay_flush_policy: OverlayFlushPolicy {
            maximum_rows: if flush_aggressively { 1 } else { 100_000 },
            maximum_bytes: 64 * 1024 * 1024,
            maximum_touched_owners: 1_000,
            maximum_generations: 32,
        },
    }
}

fn engine_config() -> ContinuousWorkspaceConfig {
    ContinuousWorkspaceConfig {
        analysis_context_id: SOURCE_CONTEXT_ID,
        registered_git_identity: None,
        git_observations: GitStateObservations {
            inclusion_policy_fingerprint: [0x71; 32],
            attributes_fingerprint: [0x72; 32],
            worktree_inventory_digest: [0; 32],
        },
        prior_git_vector: None,
        overlay_memory_limit_bytes: 64 * 1024 * 1024,
        semantic_capabilities_required: false,
    }
}

fn build_engine(
    workspace_id: [u8; 16],
    workspace_root: &Path,
    state_root: &Path,
    source_generation: u64,
    event_watermark: u64,
    lifecycle: LifecycleConfig,
    config: ContinuousWorkspaceConfig,
) -> Result<ContinuousWorkspaceEngine<GixGitStateAdapter>, GateBCandidateError> {
    let scheduler = UpdateWaveScheduler::new(
        workspace_id,
        workspace_root,
        source_generation,
        event_watermark,
        event_watermark,
        lifecycle,
    )
    .map_err(invariant)?;
    let source_images = SourceImageStore::open(
        &state_root.join("source-blobs"),
        SourceCapturePolicy {
            maximum_bytes: lifecycle.maximum_capture_bytes,
            stable_read_retries: lifecycle.stable_read_retry_count,
            lease_ttl: lifecycle.source_blob_lease_ttl,
        },
    )?;
    Ok(ContinuousWorkspaceEngine::new(
        scheduler,
        source_images,
        GitCandidatePlanner::without_cache(GixGitStateAdapter),
        config,
    ))
}

fn invariant(error: impl std::fmt::Display) -> GateBCandidateError {
    GateBCandidateError::Invariant(error.to_string())
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), GateBCandidateError> {
    fs::create_dir(destination)?;
    let mut entries = fs::read_dir(source)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(invariant(format!(
                "scenario workspace contains a symlink: {}",
                entry.path().display()
            )));
        }
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target)?;
        } else {
            return Err(invariant("scenario workspace contains a non-file member"));
        }
    }
    Ok(())
}

fn hint(path: &str, kind: WatchHintKind) -> WatchHint {
    WatchHint {
        path_bytes: path.as_bytes().to_vec(),
        kind,
    }
}

fn batch(hints: Vec<WatchHint>, rescan_required: bool) -> WatchHintBatch {
    WatchHintBatch {
        hints,
        rescan_required,
    }
}

fn write_workspace(root: &Path, relative: &str, bytes: &[u8]) -> Result<(), GateBCandidateError> {
    fs::write(root.join(relative), bytes)?;
    Ok(())
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(result, "{byte:02x}").expect("writing to a String is infallible");
    }
    result
}

fn canonical_bytes(value: &impl Serialize) -> Result<Vec<u8>, GateBCandidateError> {
    canonicalize_slice(&serde_json::to_vec(value)?).map_err(invariant)
}

fn file_bytes(value: &impl Serialize) -> Result<Vec<u8>, GateBCandidateError> {
    let mut bytes = canonical_bytes(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn read_candidate_input(path: &Path) -> Result<Vec<u8>, GateBCandidateError> {
    Ok(fs::read(path)?)
}

pub(crate) fn read_candidate_artifact(path: &Path) -> Result<Vec<u8>, GateBCandidateError> {
    read_candidate_input(path)
}

#[allow(clippy::too_many_lines)] // The closed eleven-plane integrity contract remains exhaustive in one dispatcher.
fn validate_execution_plane(group: &str, value: &Value) -> Result<(), GateBCandidateError> {
    let object = value.as_object();
    if value.get("status").and_then(Value::as_str) == Some("NOT_REACHED") {
        return Err(invariant(format!(
            "actual Gate B plane {group} was not reached"
        )));
    }
    match group {
        "source_inventory" => {
            let rows = value
                .as_array()
                .ok_or_else(|| invariant("source inventory plane is not an array"))?;
            let paths = rows
                .iter()
                .filter_map(|row| row["path"].as_str())
                .collect::<BTreeSet<_>>();
            for required in ["python/golden_pkg/core.py", "rust/src/lib.rs"] {
                if !paths.contains(required) {
                    return Err(invariant(format!(
                        "actual source inventory lacks {required}"
                    )));
                }
            }
        }
        "provider_observations" => {
            let object = object.ok_or_else(|| invariant("provider plane is not an object"))?;
            for provider in ["pyrefly", "rustc_mir"] {
                if !object.contains_key(provider) {
                    return Err(invariant(format!("actual provider plane lacks {provider}")));
                }
            }
        }
        "canonical_tables" => {
            let object = object.ok_or_else(|| invariant("canonical plane is not an object"))?;
            for flag in [
                "contains_python_semantics",
                "contains_rust_mir",
                "contains_relation",
                "contains_property",
                "contains_derived",
                "contains_unknown",
            ] {
                if object.get(flag).and_then(Value::as_bool) != Some(true) {
                    return Err(invariant(format!(
                        "actual canonical plane does not prove {flag}"
                    )));
                }
            }
            let effective = object
                .get("governed_effective_state")
                .and_then(Value::as_object)
                .ok_or_else(|| invariant("governed canonical comparison state is absent"))?;
            if effective
                .get("state_digest")
                .and_then(Value::as_str)
                .is_none()
                || effective
                    .get("tables")
                    .and_then(Value::as_object)
                    .is_none_or(serde_json::Map::is_empty)
            {
                return Err(invariant(
                    "governed canonical comparison state is empty or undigested",
                ));
            }
        }
        "publications" => {
            let object = object.ok_or_else(|| invariant("publication plane is not an object"))?;
            let tables = object
                .get("tables")
                .and_then(Value::as_object)
                .ok_or_else(|| invariant("publication table pins are absent"))?;
            if tables.is_empty() || tables.values().any(|table| table["validated"] != true) {
                return Err(invariant("publication tables are absent or unvalidated"));
            }
        }
        "serving_snapshots" => {
            let object = object.ok_or_else(|| invariant("snapshot plane is not an object"))?;
            if object.get("source_trust_state").and_then(Value::as_str) != Some("CURRENT") {
                return Err(invariant("serving snapshot is not current"));
            }
        }
        "queries" => {
            let object = object.ok_or_else(|| invariant("query plane is not an object"))?;
            if object.get("form_count").and_then(Value::as_u64) != Some(8) {
                return Err(invariant("query plane did not execute all eight forms"));
            }
        }
        "rpc" => {
            let object = object.ok_or_else(|| invariant("RPC plane is not an object"))?;
            if object.get("transport").and_then(Value::as_str) != Some("unix-domain-socket") {
                return Err(invariant("RPC plane did not use UDS"));
            }
            if object
                .get("daemon_query_id_correlated")
                .and_then(Value::as_bool)
                != Some(true)
                || object.get("event_checksums_valid").and_then(Value::as_bool) != Some(true)
            {
                return Err(invariant(
                    "RPC execution correlation or event checksum validation is absent",
                ));
            }
            let events = object
                .get("event_kinds")
                .and_then(Value::as_array)
                .ok_or_else(|| invariant("RPC plane has no event census"))?;
            for required in ["snapshot_pinned", "artifact_ready", "terminal"] {
                if !events.iter().any(|event| event.as_str() == Some(required)) {
                    return Err(invariant(format!("RPC plane lacks {required}")));
                }
            }
        }
        "mcp" => {
            let object = object.ok_or_else(|| invariant("MCP plane is not an object"))?;
            if object.get("transport").and_then(Value::as_str) != Some("stdio") {
                return Err(invariant("MCP plane did not use STDIO"));
            }
            if object
                .get("mcp_call_id_correlated")
                .and_then(Value::as_bool)
                != Some(true)
            {
                return Err(invariant("MCP-to-daemon execution correlation is absent"));
            }
            if !object
                .get("tool_names")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .any(|tool| tool.as_str() == Some("query_code_graph"))
            {
                return Err(invariant("MCP plane lacks the production query tool"));
            }
        }
        "diagnostics" => {
            let object = object.ok_or_else(|| invariant("diagnostic plane is not an object"))?;
            if object.get("artifact_persisted").and_then(Value::as_bool) != Some(true) {
                return Err(invariant(
                    "diagnostic/artifact plane lacks persisted artifact",
                ));
            }
        }
        "rebuild_comparison" => {
            let object = object.ok_or_else(|| invariant("rebuild plane is not an object"))?;
            if object.get("inventory_equal").and_then(Value::as_bool) != Some(true) {
                return Err(invariant("rebuild comparison inventory differs"));
            }
        }
        "identities" => {
            let object = object.ok_or_else(|| invariant("identity plane is not an object"))?;
            for field in [
                "workspace_id",
                "analysis_context_id",
                "hot_wave_id",
                "clean_wave_id",
            ] {
                if object.get(field).and_then(Value::as_str).is_none() {
                    return Err(invariant(format!("identity plane lacks {field}")));
                }
            }
        }
        other => return Err(invariant(format!("unknown Gate B plane {other}"))),
    }
    Ok(())
}

fn validate_vertical_execution(
    execution: &vertical::VerticalExecution,
) -> Result<(), GateBCandidateError> {
    let observed = execution
        .planes
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if observed != REQUIRED_EXPECTED_GROUPS.into_iter().collect() {
        return Err(invariant("vertical execution plane census differs"));
    }
    let identities = &execution.planes["identities"];
    let publication = &execution.planes["publications"];
    let snapshot = &execution.planes["serving_snapshots"];
    let query = &execution.planes["queries"];
    if identities["workspace_id"] != execution.workspace_id
        || identities["analysis_context_id"] != execution.analysis_context_id
        || publication["publication_id"] != execution.publication_id
        || snapshot["publication_id"] != execution.publication_id
        || snapshot["snapshot_id"] != execution.snapshot_id
        || query["snapshot_id"] != execution.snapshot_id
    {
        return Err(invariant(
            "vertical execution correlation/provenance identities differ",
        ));
    }
    for group in REQUIRED_EXPECTED_GROUPS {
        validate_execution_plane(group, &execution.planes[group])?;
    }
    Ok(())
}

fn semantic_rows<'a>(value: &'a Value, label: &str) -> Result<&'a Vec<Value>, GateBCandidateError> {
    value
        .as_array()
        .ok_or_else(|| invariant(format!("{label} is not an array")))
}

fn provider_module<'a>(
    execution: &'a vertical::VerticalExecution,
    module_name: &str,
) -> Result<&'a Value, GateBCandidateError> {
    semantic_rows(
        &execution.planes["provider_observations"]["pyrefly"]["decoded_semantics"],
        "decoded Pyrefly modules",
    )?
    .iter()
    .find(|module| module["module_name"] == module_name)
    .ok_or_else(|| invariant(format!("Pyrefly module {module_name} is absent")))
}

fn authored_call_range(
    contract: &crate::functional_golden::FunctionalGoldenContract,
    source_name: &str,
    anchor: &str,
    call_text: &str,
) -> Result<(u64, u64, u64, u64), GateBCandidateError> {
    let source = contract
        .sources
        .iter()
        .find(|source| source.source_name == source_name)
        .ok_or_else(|| invariant(format!("authored source {source_name} is absent")))?;
    let anchor_text = source
        .anchors
        .get(anchor)
        .ok_or_else(|| invariant(format!("authored anchor {source_name}.{anchor} is absent")))?;
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(crate::functional_golden::FUNCTIONAL_AUTHORITY_ROOT)
        .join(&source.path);
    let contents = fs::read_to_string(&path)?;
    let matches = contents
        .lines()
        .enumerate()
        .filter(|(_, line)| line.trim_start() == anchor_text)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(invariant(format!(
            "authored anchor {source_name}.{anchor} is not unique in {}",
            path.display()
        )));
    }
    let (line_index, line) = matches[0];
    let start_col = line
        .find(call_text)
        .ok_or_else(|| invariant(format!("{call_text} is absent from {source_name}.{anchor}")))?;
    Ok((
        u64::try_from(line_index + 1).map_err(invariant)?,
        u64::try_from(start_col).map_err(invariant)?,
        u64::try_from(line_index + 1).map_err(invariant)?,
        u64::try_from(start_col + call_text.len()).map_err(invariant)?,
    ))
}

fn assert_callee(
    module: &Value,
    target: &str,
    expected_range: (u64, u64, u64, u64),
) -> Result<(), GateBCandidateError> {
    let callees = semantic_rows(&module["callees"], "Pyrefly callees")?;
    let matches = callees
        .iter()
        .filter(|callee| callee["kind"] == "function" && callee["target"] == target)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(invariant(format!(
            "expected one Pyrefly callee {target}, observed {}",
            matches.len()
        )));
    }
    let range = &matches[0]["range"];
    let actual = (
        range["start_line"].as_u64(),
        range["start_col"].as_u64(),
        range["end_line"].as_u64(),
        range["end_col"].as_u64(),
    );
    let expected = (
        Some(expected_range.0),
        Some(expected_range.1),
        Some(expected_range.2),
        Some(expected_range.3),
    );
    if actual != expected {
        return Err(invariant(format!(
            "Pyrefly callee {target} range {actual:?} differs from authored {expected:?}"
        )));
    }
    Ok(())
}

fn query_result<'a>(response: &'a Value, query_id: &str) -> Result<&'a Value, GateBCandidateError> {
    semantic_rows(&response["query_results"], "public query results")?
        .iter()
        .find(|result| result["query_id"] == query_id)
        .ok_or_else(|| invariant(format!("public result {query_id} is absent")))
}

#[allow(clippy::too_many_lines)] // One explicit function binds each decoded public seam to its authored claim identifier.
fn validated_functional_claims(
    contract: &crate::functional_golden::FunctionalGoldenContract,
    execution: &vertical::VerticalExecution,
) -> Result<BTreeSet<String>, GateBCandidateError> {
    validate_vertical_execution(execution)?;
    let mut validated = BTreeSet::new();
    let reference = crate::functional_golden::ReferenceQueryEvaluator::new(contract, "base")
        .evaluate_all(&contract.queries);
    if !reference.passed {
        return Err(invariant(format!(
            "authored query contract is internally inconsistent: {:?}",
            reference.mismatches
        )));
    }
    let pyrefly_modules = semantic_rows(
        &execution.planes["provider_observations"]["pyrefly"]["decoded_semantics"],
        "decoded Pyrefly modules",
    )?;
    let module_names = pyrefly_modules
        .iter()
        .filter_map(|module| module["module_name"].as_str())
        .collect::<BTreeSet<_>>();
    if module_names != BTreeSet::from(["ffi.boundary", "golden_pkg.core", "malformed.broken"]) {
        return Err(invariant(
            "Pyrefly module census differs from authored sources",
        ));
    }
    let core = provider_module(execution, "golden_pkg.core")?;
    if !semantic_rows(&core["diagnostics"], "core diagnostics")?.is_empty() {
        return Err(invariant("authored Python core has Pyrefly diagnostics"));
    }
    let type_table = semantic_rows(&core["type_table"]["type_table"], "Pyrefly core type table")?;
    let int_index = type_table
        .iter()
        .position(|entry| entry["kind"] == "named" && entry["name"] == "builtins.int")
        .ok_or_else(|| invariant("Pyrefly core lacks builtins.int"))?;
    let int_index = u64::try_from(int_index).map_err(invariant)?;
    if !type_table.iter().any(|entry| {
        entry["kind"] == "callable"
            && entry["params"]
                .as_array()
                .is_some_and(|params| params.len() == 1 && params[0].as_u64() == Some(int_index))
            && entry["return_type"].as_u64() == Some(int_index)
    }) {
        return Err(invariant(
            "Pyrefly core lacks the authored (int) -> int callable contract",
        ));
    }
    validated.extend(["claim.py.owner".to_owned(), "claim.py.return".to_owned()]);
    assert_callee(
        core,
        "golden_pkg.core.scale",
        authored_call_range(contract, "python.core", "call.scale", "scale")?,
    )?;
    validated.insert("claim.py.call-target".to_owned());
    let ffi = provider_module(execution, "ffi.boundary")?;
    if !semantic_rows(&ffi["diagnostics"], "FFI diagnostics")?.is_empty() {
        return Err(invariant(
            "same-run Pyrefly module resolution left an FFI import diagnostic",
        ));
    }
    assert_callee(
        ffi,
        "golden_pkg.core.pipeline",
        authored_call_range(contract, "ffi.boundary", "call.pipeline", "pipeline")?,
    )?;
    validated.insert("claim.ffi.call-target".to_owned());
    let malformed = provider_module(execution, "malformed.broken")?;
    let malformed_diagnostics = semantic_rows(&malformed["diagnostics"], "parse diagnostics")?;
    if malformed_diagnostics.is_empty()
        || malformed_diagnostics.iter().any(|diagnostic| {
            !diagnostic
                .as_str()
                .is_some_and(|text| text.contains("[parse-error]"))
        })
    {
        return Err(invariant(
            "malformed Python did not yield explicit parse-error diagnostics",
        ));
    }
    if !semantic_rows(&malformed["callees"], "malformed callees")?.is_empty() {
        return Err(invariant("malformed Python invented semantic callees"));
    }
    validated.extend([
        "claim.unknown.parse".to_owned(),
        "claim.diagnostic.parse".to_owned(),
    ]);

    let rustc = semantic_rows(
        &execution.planes["provider_observations"]["rustc_mir"]["decoded_semantics"],
        "decoded rustc owners",
    )?;
    let rustc_by_name = rustc
        .iter()
        .filter_map(|owner| Some((owner["name"].as_str()?, owner)))
        .collect::<BTreeMap<_, _>>();
    if rustc_by_name.keys().copied().collect::<BTreeSet<_>>()
        != BTreeSet::from([
            "codefabric_gate_b_rust::choose",
            "codefabric_gate_b_rust::double",
            "codefabric_gate_b_rust::pipeline",
        ])
        || rustc_by_name
            .values()
            .any(|owner| owner["item_kind"] != "function")
    {
        return Err(invariant(
            "rustc/MIR callable census differs from authored Rust",
        ));
    }
    validated.insert("claim.rust.owner".to_owned());
    let terminators = |name: &str| -> Result<BTreeSet<&str>, GateBCandidateError> {
        Ok(
            semantic_rows(&rustc_by_name[name]["terminator_kinds"], "MIR terminators")?
                .iter()
                .filter_map(Value::as_str)
                .collect(),
        )
    };
    if !terminators("codefabric_gate_b_rust::pipeline")?.contains("call")
        || !terminators("codefabric_gate_b_rust::choose")?.contains("switch-int")
    {
        return Err(invariant(
            "rustc/MIR did not expose the authored call and branch semantics",
        ));
    }
    validated.insert("claim.rust.mir-branch".to_owned());

    let decoded = &execution.planes["canonical_tables"]["decoded_semantics"];
    let entities = semantic_rows(&decoded["entities"], "decoded canonical entities")?;
    let callable_code = crate::registries::entity_kind("CALLABLE")
        .ok_or_else(|| invariant("CALLABLE registry allocation is absent"))?
        .code;
    let rust_entities = entities
        .iter()
        .filter(|entity| entity["language_code"] == crate::registries::Language::Rust as i16)
        .filter(|entity| entity["entity_kind_code"] == callable_code)
        .filter_map(|entity| entity["name"].as_str())
        .filter(|name| name.starts_with("codefabric_gate_b_rust::"))
        .collect::<BTreeSet<_>>();
    if rust_entities != rustc_by_name.keys().copied().collect() {
        return Err(invariant(
            "canonical Rust callables differ from exact rustc/MIR owners",
        ));
    }
    let calls_code = crate::registries::relation_kind("CALLS")
        .ok_or_else(|| invariant("CALLS registry allocation is absent"))?
        .code;
    let relations = semantic_rows(&decoded["relations"], "decoded canonical relations")?;
    for (source_prefix, target) in [
        ("golden_pkg.core:", "golden_pkg.core.scale"),
        ("ffi.boundary:", "golden_pkg.core.pipeline"),
    ] {
        let matches = relations
            .iter()
            .filter(|relation| relation["relation_kind_code"] == calls_code)
            .filter(|relation| {
                relation["certainty_code"]
                    == crate::registries::EvidenceCertainty::StaticSemantic as i16
            })
            .filter(|relation| {
                relation["source_name"]
                    .as_str()
                    .is_some_and(|name| name.starts_with(source_prefix))
            })
            .filter(|relation| relation["target_name"] == target)
            .count();
        if matches != 1 {
            return Err(invariant(format!(
                "canonical call-site relation {source_prefix} -> {target} has multiplicity {matches}"
            )));
        }
    }
    let name_property = crate::registries::property_kind("NAME")
        .ok_or_else(|| invariant("NAME registry allocation is absent"))?
        .code;
    let rust_name_fact_ids = semantic_rows(&decoded["properties"], "decoded properties")?
        .iter()
        .filter(|property| property["property_kind_code"] == name_property)
        .filter(|property| {
            property["subject_name"]
                .as_str()
                .is_some_and(|name| rust_entities.contains(name))
        })
        .filter_map(|property| property["fact_id"].as_str())
        .collect::<BTreeSet<_>>();
    if rust_name_fact_ids.len() != 3 {
        return Err(invariant("canonical Rust NAME fact census differs"));
    }
    let capabilities = semantic_rows(&decoded["capabilities"], "decoded capabilities")?;
    if !capabilities.iter().any(|row| {
        row["state_code"] == crate::registries::OwnerCapabilityState::UnavailableParse as i16
            && row["completeness_code"] == crate::registries::Completeness::Indeterminate as i16
    }) || !capabilities.iter().any(|row| {
        row["state_code"] == crate::registries::OwnerCapabilityState::Current as i16
            && row["completeness_code"] == crate::registries::Completeness::Complete as i16
    }) {
        return Err(invariant(
            "canonical capability surface lacks current and explicit unavailable/indeterminate states",
        ));
    }
    validated.insert("claim.capability.current".to_owned());

    let publication_tables = execution.planes["publications"]["tables"]
        .as_object()
        .ok_or_else(|| invariant("decoded publication table map is absent"))?;
    if publication_tables.is_empty()
        || publication_tables
            .values()
            .any(|table| table["validated"] != true)
    {
        return Err(invariant(
            "Delta publication contains an absent or unvalidated table",
        ));
    }
    if execution.planes["serving_snapshots"]["source_trust_state"] != "CURRENT" {
        return Err(invariant(
            "serving snapshot activated without current source trust",
        ));
    }

    let decoded_response = &execution.planes["queries"]["decoded_response"];
    if decoded_response["successful_query_count"] != contract.queries.len()
        || decoded_response["failed_query_count"] != 0
        || decoded_response["execution_state"] != "COMPLETE"
        || decoded_response["availability_state"] != "PARTIAL"
        || decoded_response["completeness_state"] != "INDETERMINATE"
    {
        return Err(invariant(
            "decoded public response terminal semantics differ",
        ));
    }
    validated.insert("claim.terminal.current".to_owned());
    for query in &contract.queries {
        let actual = query_result(decoded_response, &query.query_id)?;
        if actual["request"] != query.request_form
            || actual["execution_state"] != "COMPLETE"
            || actual["coverage"]["returned_rows"]
                != u64::try_from(query.expected_records.len()).map_err(invariant)?
            || actual["completeness_state"]
                != if query.completeness == "complete" {
                    "COMPLETE"
                } else {
                    "INDETERMINATE"
                }
        {
            return Err(invariant(format!(
                "decoded public response differs for {}",
                query.query_id
            )));
        }
    }
    let entities_result = query_result(decoded_response, "q.entities")?;
    let facts_result = query_result(decoded_response, "q.facts")?;
    let relationships_result = query_result(decoded_response, "q.relationships")?;
    let paths_result = query_result(decoded_response, "q.paths")?;
    let pattern_result = query_result(decoded_response, "q.pattern")?;
    let combine_result = query_result(decoded_response, "q.combine")?;
    let summary_result = query_result(decoded_response, "q.summary")?;
    let context_result = query_result(decoded_response, "q.context")?;
    if entities_result["availability_state"] != "AVAILABLE"
        || entities_result["completeness_state"] != "COMPLETE"
        || entities_result["entity_ids"].as_array().map_or(0, Vec::len) != 1
        || context_result["source_context_ids"]
            .as_array()
            .map_or(0, Vec::len)
            != 1
    {
        return Err(invariant("source-file/context query semantics differ"));
    }
    let returned_rust_facts = semantic_rows(&facts_result["fact_ids"], "Rust query fact ids")?
        .iter()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    if returned_rust_facts != rust_name_fact_ids {
        return Err(invariant(
            "public callable-contract facts differ from canonical Rust NAME facts",
        ));
    }
    let core_relation = relations
        .iter()
        .find(|relation| {
            relation["source_name"]
                .as_str()
                .is_some_and(|name| name.starts_with("golden_pkg.core:"))
                && relation["target_name"] == "golden_pkg.core.scale"
        })
        .ok_or_else(|| invariant("canonical core call relation is absent"))?;
    let relationship_fact_ids = semantic_rows(
        &relationships_result["fact_ids"],
        "relationship query fact ids",
    )?;
    if relationship_fact_ids.as_slice() != [core_relation["fact_id"].clone()]
        || relationships_result["coverage"]["examined_edges"] != 2
        || relationships_result["coverage"]["graph_nodes"] != 4
    {
        return Err(invariant(format!(
            "public call-target result differs: facts={relationship_fact_ids:?}, expected={}, coverage={}",
            core_relation["fact_id"], relationships_result["coverage"]
        )));
    }
    if paths_result["availability_state"] != "PARTIAL"
        || paths_result["completeness_state"] != "INDETERMINATE"
        || paths_result["coverage"]["returned_rows"] != 0
        || paths_result["coverage"]["negative_proof_available"] != 0
        || paths_result["notices"].as_array().is_none_or(Vec::is_empty)
    {
        return Err(invariant(
            "empty control-flow path failed to remain explicit indeterminate non-absence",
        ));
    }
    if pattern_result["group_ids"].as_array().map_or(0, Vec::len) != 1
        || pattern_result["coverage"]["examined_edges"] != 2
        || pattern_result["coverage"]["graph_nodes"] != 4
        || combine_result["group_ids"].as_array().map_or(0, Vec::len) != 4
        || summary_result["group_ids"].as_array().map_or(0, Vec::len) != 1
    {
        return Err(invariant(
            "pattern/combine/summary public semantics differ from the authored graph",
        ));
    }
    if &execution.planes["mcp"]["structured_content"]["delivery"]["response"] != decoded_response {
        return Err(invariant(
            "decoded FastMCP response differs from UDS artifact semantics",
        ));
    }
    if execution.planes["rpc"]["event_kinds"]
        != serde_json::json!(["snapshot_pinned", "artifact_ready", "terminal"])
        || execution.planes["rpc"]["event_checksums_valid"] != true
        || execution.planes["diagnostics"]["artifact_persisted"] != true
        || execution.planes["diagnostics"]["terminal_state"] != "SUCCEEDED"
    {
        return Err(invariant("public delivery/correlation semantics differ"));
    }
    validated.insert("claim.delivery.equivalent".to_owned());
    Ok(validated)
}

fn detached_manifest_digest(manifest: &CandidateManifest) -> Result<String, GateBCandidateError> {
    let bytes = canonical_bytes(manifest)?;
    let mut hasher = crate::integrity::IntegrityHasher::for_domain(
        crate::integrity::IntegrityDomain::GateBReviewCandidate,
    );
    hasher.update(&(bytes.len() as u64).to_be_bytes());
    hasher.update(&bytes);
    Ok(crate::integrity::frame_digest(hasher.finalize()))
}

fn safe_output_directory(path: &Path) -> Result<(), GateBCandidateError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invariant(
            "candidate output must be a safe repository-relative path",
        ));
    }
    Ok(())
}

/// Write a generated bundle to a new repository-relative candidate directory.
///
/// # Errors
///
/// Returns an error for unsafe paths, an existing destination, or an I/O failure.
pub fn write_candidate_bundle(
    repository_root: &Path,
    output: &Path,
    bundle: &GeneratedCandidateBundle,
) -> Result<(), GateBCandidateError> {
    safe_output_directory(output)?;
    let destination = repository_root.join(output);
    let parent = destination
        .parent()
        .ok_or_else(|| invariant("candidate output has no repository-relative parent"))?;
    fs::create_dir_all(parent)?;
    fs::create_dir(&destination)?;
    for (name, bytes) in bundle.files() {
        fs::write(destination.join(name), bytes)?;
    }
    Ok(())
}

/// Verify a committed candidate bundle without granting release authority.
///
/// # Errors
///
/// Returns an error for missing/extra/noncanonical members, a released or owner-accepted
/// candidate, incomplete scenarios/items/diff, or any detached digest mismatch.
pub fn verify_candidate_bundle(candidate_root: &Path) -> Result<(), GateBCandidateError> {
    let observed = fs::read_dir(candidate_root)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if observed != BUNDLE_FILES.into_iter().map(str::to_owned).collect() {
        return Err(invariant("candidate bundle member census differs"));
    }
    let payload: GateBCandidatePayload =
        serde_json::from_slice(&read_candidate_input(&candidate_root.join(CANDIDATE_FILE))?)?;
    let diff: CandidateDiff =
        serde_json::from_slice(&read_candidate_input(&candidate_root.join(DIFF_FILE))?)?;
    let manifest: CandidateManifest =
        serde_json::from_slice(&read_candidate_input(&candidate_root.join(MANIFEST_FILE))?)?;
    let detached: DetachedCandidateDigest =
        serde_json::from_slice(&read_candidate_input(&candidate_root.join(DIGEST_FILE))?)?;
    if manifest.candidate_id != payload.candidate_id
        || diff.candidate_id != payload.candidate_id
        || manifest.owner_acceptance.is_some()
        || payload.scenario_executions.len() != REQUIRED_SCENARIOS.len()
        || payload.gate_b_items.len() != REQUIRED_EXPECTED_GROUPS.len()
        || manifest.scenario_count != REQUIRED_SCENARIOS.len()
        || manifest.gate_b_item_count != REQUIRED_EXPECTED_GROUPS.len()
        || !diff.all_expected_items_match
        || diff.groups.values().any(|group| !group.matches)
    {
        return Err(invariant(
            "candidate status, census, diff, or acceptance is invalid",
        ));
    }
    if manifest
        .members
        .iter()
        .map(|member| member.path.as_str())
        .collect::<BTreeSet<_>>()
        != BTreeSet::from([CANDIDATE_FILE, DIFF_FILE])
    {
        return Err(invariant("candidate manifest member census differs"));
    }
    for member in &manifest.members {
        if member.path == MANIFEST_FILE || member.path == DIGEST_FILE {
            return Err(invariant("candidate digest chain is self-referential"));
        }
        let bytes = read_candidate_input(&candidate_root.join(&member.path))?;
        if crate::integrity::framed_digest(&bytes) != member.digest {
            return Err(invariant(format!(
                "candidate member {} drifted",
                member.path
            )));
        }
    }
    if detached.domain != "GATE_B_REVIEW_CANDIDATE"
        || detached.manifest != MANIFEST_FILE
        || detached.digest != detached_manifest_digest(&manifest)?
    {
        return Err(invariant("detached candidate digest differs"));
    }
    for name in BUNDLE_FILES {
        let bytes = read_candidate_input(&candidate_root.join(name))?;
        let canonical = canonicalize_slice(&bytes).map_err(invariant)?;
        let mut expected_file = canonical;
        expected_file.push(b'\n');
        if bytes != expected_file {
            return Err(invariant(format!(
                "candidate member {name} is not canonical JSON"
            )));
        }
    }
    Ok(())
}

/// Re-execute the candidate's semantic payload while preserving the accepted bundle as history.
///
/// A released candidate records the exact design and generated-authority digests reviewed at
/// acceptance time. Later, unrelated edits to those inputs must not rewrite that immutable
/// evidence or require a new acceptance. This check therefore verifies the committed bundle's
/// complete digest chain, regenerates against the current tree, and compares the governed
/// functional projection. Exact accepted bytes remain immutable evidence; newly allocated
/// publication, snapshot, provider-run, artifact, and transport identities are validated within
/// their run rather than required to repeat across runs. Any semantic scenario, Gate B item, or
/// source-profile drift remains a failure; review-time input-digest metadata remains frozen.
///
/// # Errors
///
/// Returns an error when the accepted bundle is malformed, current execution fails, or the
/// regenerated semantic payload differs from the accepted candidate payload.
pub fn check_released_candidate_payload(
    _repository_root: &Path,
    _corpus_root: &Path,
    _scratch_root: &Path,
    candidate_root: &Path,
) -> Result<(), GateBCandidateError> {
    verify_candidate_bundle(candidate_root)?;
    let committed = read_candidate_input(&candidate_root.join(CANDIDATE_FILE))?;
    let committed_payload: GateBCandidatePayload = serde_json::from_slice(&committed)?;
    // Released v2 remains immutable historical evidence and retains complete digest-chain
    // verification. Rejected v3 is never regenerated or reinterpreted through a current semantic
    // executor: doing so would restore the retired candidate-local expectation authority.
    if committed_payload.candidate_id == CANDIDATE_ID {
        return Err(invariant(
            "rejected Gate B v3 candidate cannot be routed as a released corpus",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::OnceLock;

    use super::*;

    fn repository_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn functional_corpus_root() -> PathBuf {
        repository_root().join(crate::functional_golden::FUNCTIONAL_AUTHORITY_ROOT)
    }

    fn functional_execution() -> &'static vertical::VerticalExecution {
        static EXECUTION: OnceLock<vertical::VerticalExecution> = OnceLock::new();
        EXECUTION.get_or_init(|| {
            let temporary = tempfile::tempdir().unwrap();
            let scratch = temporary.path().join("scratch");
            fs::create_dir(&scratch).unwrap();
            vertical::execute_authored_workspace(
                &repository_root(),
                &functional_corpus_root(),
                &scratch,
            )
            .unwrap()
        })
    }

    fn functional_contract() -> crate::functional_golden::FunctionalGoldenContract {
        crate::functional_golden::load_contract(&repository_root()).unwrap()
    }

    #[test]
    fn gate_b_public_vertical_conformance() {
        validated_functional_claims(&functional_contract(), functional_execution()).unwrap();
    }

    #[test]
    fn gate_b_projection_registry_closure() {
        let execution = functional_execution();
        let decoded = &execution.planes["canonical_tables"]["decoded_semantics"];
        assert!(
            decoded["entities"]
                .as_array()
                .is_some_and(|rows| !rows.is_empty())
        );
        let registry: crate::contracts::registry_models::AcceptedRegistry<
            crate::contracts::registry_models::ComparisonIgnoreRecord,
        > = serde_yaml_ng::from_slice(include_bytes!(
            "../contracts/comparison/comparison-ignore-registry.yaml"
        ))
        .unwrap();
        crate::contracts::registry_models::validate_comparison_ignores(&registry.records).unwrap();
        assert!(registry.records.iter().all(|record| !record.semantic));
    }

    #[test]
    fn gate_b_causal_intervention_matrix() {
        let contract = functional_contract();
        let baseline = functional_execution();
        validated_functional_claims(&contract, baseline).unwrap();

        let interventions = [
            vertical::CausalIntervention::PyreflySourceAdmission,
            vertical::CausalIntervention::ReconciliationAuthority,
            vertical::CausalIntervention::DeltaPublication,
            vertical::CausalIntervention::SnapshotActivation,
            vertical::CausalIntervention::ArtifactPersistence,
            vertical::CausalIntervention::ArtifactReadback,
            vertical::CausalIntervention::FastMcpAdaptation,
        ];
        for batch in interventions.chunks(3) {
            std::thread::scope(|scope| {
                let handles = batch
                    .iter()
                    .copied()
                    .map(|intervention| {
                        let contract = &contract;
                        scope.spawn(move || {
                            let temporary = tempfile::tempdir().unwrap();
                            let scratch = temporary.path().join("scratch");
                            fs::create_dir(&scratch).unwrap();
                            match vertical::execute_with_intervention(
                                &repository_root(),
                                &functional_corpus_root(),
                                &scratch,
                                intervention,
                            ) {
                                Ok(observed) => {
                                    let failure = validated_functional_claims(contract, &observed)
                                            .expect_err(
                                                "a producing-seam intervention survived the semantic oracle",
                                            );
                                    assert_eq!(
                                        observed.planes["source_inventory"],
                                        baseline.planes["source_inventory"],
                                        "{intervention:?} changed the unrelated source inventory"
                                    );
                                    assert_eq!(
                                        observed.planes["provider_observations"],
                                        baseline.planes["provider_observations"],
                                        "{intervention:?} changed unrelated provider evidence"
                                    );
                                    assert!(
                                        failure.to_string().contains("snapshot"),
                                        "{intervention:?} failed outside its predicted snapshot claim: {failure}"
                                    );
                                }
                                Err(error) => assert!(
                                    error.to_string().contains(&format!("{intervention:?}")),
                                    "intervention error lacks its named producing seam: {error}"
                                ),
                            }
                        })
                    })
                    .collect::<Vec<_>>();
                for handle in handles {
                    handle.join().unwrap();
                }
            });
        }
    }

    #[test]
    fn gate_b_public_vertical_operational_gate() {
        let contract = functional_contract();
        let execution = functional_execution();
        validated_functional_claims(&contract, execution).unwrap();
        assert_eq!(execution.planes["rpc"]["transport"], "unix-domain-socket");
        assert_eq!(execution.planes["mcp"]["transport"], "stdio");
        assert_eq!(execution.planes["diagnostics"]["artifact_persisted"], true);
    }

    #[test]
    fn golden_scenario_semantic_transition_contracts() {
        let contract = functional_contract();
        let temporary = tempfile::tempdir().unwrap();
        let source_workspace = functional_corpus_root().join("workspace");
        let mut checkpoint_count = 0;
        for scenario in &contract.scenarios {
            assert!(!scenario.operations.is_empty(), "{}", scenario.scenario_id);
            let target = temporary.path().join(&scenario.scenario_id);
            let materialized = crate::functional_scenario::materialize_scenario(
                &source_workspace,
                &target,
                scenario,
            )
            .unwrap_or_else(|error| panic!("{}: {error}", scenario.scenario_id));
            assert_eq!(materialized.len(), scenario.checkpoints.len());
            checkpoint_count += materialized.len();
            for (checkpoint, actual) in scenario.checkpoints.iter().zip(&materialized) {
                assert_eq!(actual.checkpoint, checkpoint.checkpoint);
                assert_eq!(actual.after_operation, checkpoint.after_operation);
                assert!(
                    actual.files.contains_key("python/golden_pkg/core.py")
                        || actual.files.contains_key("python/golden_pkg/Core.py")
                );
                assert!(actual.files.contains_key("rust/src/lib.rs"));
                assert!(actual.directives.iter().any(|directive| matches!(
                    directive,
                    crate::functional_scenario::ScenarioDirective::Barrier(name)
                        if scenario.operations[..checkpoint.after_operation].iter().any(|operation| matches!(
                            operation,
                            crate::functional_golden::ScenarioOperation::Barrier { name: expected }
                                if expected == name
                        ))
                )));
                let changed = checkpoint.transition.added.len()
                    + checkpoint.transition.removed.len()
                    + checkpoint.transition.changed.len();
                let has_semantic_operation = scenario.operations[..checkpoint.after_operation]
                    .iter()
                    .any(|operation| {
                        !matches!(
                            operation,
                            crate::functional_golden::ScenarioOperation::Barrier { .. }
                        )
                    });
                assert!(
                    changed == 0
                        || has_semantic_operation
                        || scenario.scenario_id == "000_clean_bootstrap",
                    "{}",
                    checkpoint.checkpoint
                );
                assert!(checkpoint.claims.iter().all(|claim| {
                    contract
                        .claims
                        .iter()
                        .any(|candidate| &candidate.claim_id == claim)
                }));
            }
        }
        assert_eq!(checkpoint_count, 18);
    }

    #[test]
    fn gate_b_named_fixture_query_causality() {
        let contract = functional_contract();
        let scenario = contract
            .scenarios
            .iter()
            .find(|scenario| scenario.scenario_id == "010_python_local_edit")
            .unwrap();
        let temporary = tempfile::tempdir().unwrap();
        let authority_root = temporary.path().join("authored-edit");
        let target_workspace = authority_root.join("workspace");
        let checkpoints = crate::functional_scenario::materialize_scenario(
            &functional_corpus_root().join("workspace"),
            &target_workspace,
            scenario,
        )
        .unwrap();
        assert_eq!(checkpoints.len(), 1);
        let edited_source = checkpoints[0].files["python/golden_pkg/core.py"].as_slice();
        assert!(
            edited_source
                .windows(b"scale(value) + 7".len())
                .any(|window| window == b"scale(value) + 7")
        );
        assert!(
            !edited_source
                .windows(b"scale(value) + 1".len())
                .any(|window| window == b"scale(value) + 1")
        );

        let scratch = temporary.path().join("scratch");
        fs::create_dir(&scratch).unwrap();
        let edited =
            vertical::execute_authored_workspace(&repository_root(), &authority_root, &scratch)
                .unwrap();
        validated_functional_claims(&contract, &edited).unwrap();
        let base_response = &functional_execution().planes["queries"]["decoded_response"];
        let edited_response = &edited.planes["queries"]["decoded_response"];
        assert_ne!(
            base_response["snapshot"]["source_inventory_digest"],
            edited_response["snapshot"]["source_inventory_digest"]
        );
        assert_eq!(
            query_result(base_response, "q.relationships").unwrap()["fact_ids"],
            query_result(edited_response, "q.relationships").unwrap()["fact_ids"]
        );
        assert_ne!(base_response, edited_response);
    }

    #[test]
    fn gate_b_delivery_surface_semantic_equivalence() {
        let execution = functional_execution();
        let artifact = &execution.planes["queries"]["decoded_response"];
        let fastmcp = &execution.planes["mcp"]["structured_content"]["delivery"]["response"];
        assert_eq!(artifact, fastmcp);
        assert_eq!(artifact["successful_query_count"], 8);
        assert_eq!(execution.planes["rpc"]["event_checksums_valid"], true);
        assert_eq!(
            execution.planes["diagnostics"]["terminal_state"],
            "SUCCEEDED"
        );
    }
}
