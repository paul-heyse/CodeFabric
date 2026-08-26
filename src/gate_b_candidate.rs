//! Executable Gate B review-candidate generation.
//!
//! Candidate bytes are deliberately unreleased. The runner applies the closed scenario edit
//! vocabulary through [`crate::continuous::ContinuousWorkspaceEngine`], derives expectations from
//! normative inputs and generated registries on a separate path, and emits a detached digest chain
//! that WP76 can review but only an accountable owner can accept.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::continuous::{
    ContinuousWaveResult, ContinuousWorkspaceConfig, ContinuousWorkspaceEngine,
};
use crate::contracts::jcs::canonicalize_slice;
use crate::fabric::batch_checksum;
use crate::git_state::{GitCandidatePlanner, GitStateObservations, GixGitStateAdapter};
use crate::golden_corpus::{
    CorpusError, REQUIRED_EXPECTED_GROUPS, REQUIRED_SCENARIOS, ScenarioDefinition,
    ScenarioTerminal, load_scenarios, validate_profile,
};
use crate::identity::{IdentityDomain, SOURCE_CONTEXT_ID, encode_public_id};
use crate::lifecycle::{
    FastSyntaxFactOutput, FastSyntaxReconciler, FreshnessState, LifecycleConfig,
    OverlayFlushPolicy, UpdateWaveScheduler, WatchHint, WatchHintBatch, WatchHintKind,
    recover_workspace,
};
use crate::operational_store::{OperationalStore, OperationalStoreError};
use crate::registries::{PROVIDER_IDS, UpdateWaveState};
use crate::schema_registry::{table_spec, table_specs};
use crate::source_image::{SourceCapturePolicy, SourceImageError, SourceImageStore};
use crate::workspace_registry::{WorkspaceRegistry, WorkspaceRegistryError};

pub const CANDIDATE_ID: &str = "codefabric-golden-v2.0.0-candidate.1";
pub const CANDIDATE_DIRECTORY: &str =
    "tests/golden/review-candidates/codefabric-golden-v2.0.0-candidate.1";
const CANDIDATE_FILE: &str = "candidate.json";
const DIFF_FILE: &str = "expected-vs-candidate-diff.json";
const MANIFEST_FILE: &str = "candidate-manifest.json";
const DIGEST_FILE: &str = "candidate-digest.json";
const BUNDLE_FILES: [&str; 4] = [CANDIDATE_FILE, DIFF_FILE, MANIFEST_FILE, DIGEST_FILE];
const EXPECTATION_INPUTS: [&str; 5] = [
    "contracts/registry/design-principle-registry.yaml",
    "docs/upfront_design/codefabric_1.3_implementation_roadmap_v1.0.md",
    "docs/upfront_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md",
    "src/generated/registries.rs",
    "src/generated/table_specs.rs",
];
const GATE_B_PROVIDERS: [&str; 4] = [
    "ruff-python",
    "rustc-mir",
    "source-substrate",
    "tree-sitter",
];
const GATE_B_TABLES: [&str; 8] = [
    "entity",
    "fact_evidence",
    "property_fact",
    "relation",
    "source_annotation",
    "source_file",
    "source_token",
    "syntax_detail",
];

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
    clean_rebuild_equal: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ScenarioResponse {
    scenario_id: String,
    terminal: ScenarioTerminal,
    final_source_generation: u64,
    wave_ids: Vec<String>,
    row_count: u64,
    checksums: Vec<String>,
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
    gate_b_items: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct GroupComparison {
    expected_digest: String,
    candidate_digest: String,
    matches: bool,
    released_digest: String,
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

#[derive(Default)]
struct ScenarioAccumulator {
    waves: Vec<WaveObservation>,
    providers: BTreeSet<String>,
    inventory: BTreeMap<String, CapturedFileObservation>,
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

fn framed(bytes: [u8; 32]) -> String {
    crate::integrity::frame_digest(bytes)
}

fn candidate_table_checksum(
    table_name: &str,
    row_count: usize,
    source_generation: u64,
    raw_checksum: [u8; 32],
) -> (String, String) {
    if matches!(table_name, "capability_status" | "fact_evidence") {
        let mut hasher = crate::integrity::IntegrityHasher::for_domain(
            crate::integrity::IntegrityDomain::GateBExecution,
        );
        hasher.update(&(table_name.len() as u64).to_be_bytes());
        hasher.update(table_name.as_bytes());
        hasher.update(&(row_count as u64).to_be_bytes());
        hasher.update(&source_generation.to_be_bytes());
        (
            "OPERATIONAL_IDENTITIES_NORMALIZED".to_owned(),
            framed(hasher.finalize()),
        )
    } else {
        ("CANONICAL_BATCH".to_owned(), framed(raw_checksum))
    }
}

fn fast_state(
    outputs: &[FastSyntaxFactOutput],
) -> Result<BTreeMap<(Vec<u8>, i16), String>, GateBCandidateError> {
    let mut state = BTreeMap::new();
    for output in outputs {
        for (table_code, batch) in &output.canonical.batches {
            state.insert(
                (output.path_bytes.clone(), *table_code),
                framed(batch_checksum(batch.batch()).map_err(invariant)?),
            );
        }
    }
    Ok(state)
}

fn clean_rebuild_equal(result: &ContinuousWaveResult) -> Result<bool, GateBCandidateError> {
    let mut wave = result.wave.clone();
    wave.state = UpdateWaveState::FastAnalyzing;
    let rebuilt = FastSyntaxReconciler::default()
        .reconcile_wave(
            &wave,
            result.overlay.analysis_context_id(),
            &BTreeMap::new(),
        )
        .map_err(invariant)?;
    Ok(fast_state(&result.fast_outputs)? == fast_state(&rebuilt)?)
}

fn observe_wave(
    result: &ContinuousWaveResult,
    accumulator: &mut ScenarioAccumulator,
) -> Result<(), GateBCandidateError> {
    let mut captured_files = Vec::new();
    for item in &result.wave.items {
        let path = String::from_utf8(item.path_bytes.clone()).map_err(invariant)?;
        if let Some(source) = item.captured.as_deref() {
            let file_id = encode_public_id(IdentityDomain::SourceFile, None, source.file_id)
                .map_err(invariant)?;
            let observation = CapturedFileObservation {
                path: path.clone(),
                file_id,
                content_digest: framed(source.digest),
                byte_length: source.byte_length,
            };
            accumulator.inventory.insert(path, observation.clone());
            captured_files.push(observation);
            accumulator.providers.insert("source-substrate".to_owned());
        } else {
            accumulator.inventory.remove(&path);
        }
    }

    let mut tables = Vec::new();
    for output in &result.fast_outputs {
        accumulator.providers.insert("tree-sitter".to_owned());
        if output.ruff_python.is_some() {
            accumulator.providers.insert("ruff-python".to_owned());
        }
        for (table_code, batch) in &output.canonical.batches {
            let spec = table_spec(*table_code)
                .ok_or_else(|| invariant(format!("unknown table code {table_code}")))?;
            let (checksum_scope, checksum) = candidate_table_checksum(
                spec.name,
                batch.batch().num_rows(),
                result.wave.source_generation,
                batch_checksum(batch.batch()).map_err(invariant)?,
            );
            tables.push(TableObservation {
                table_code: *table_code,
                table_name: spec.name.to_owned(),
                row_count: batch.batch().num_rows(),
                checksum_scope,
                checksum,
            });
        }
    }
    tables.sort_by(|left, right| {
        (&left.table_name, left.table_code, &left.checksum).cmp(&(
            &right.table_name,
            right.table_code,
            &right.checksum,
        ))
    });
    let overlay_table_digests = result
        .overlay
        .tables()
        .map(|table| {
            let name = table_spec(table.table_code()).map_or_else(
                || table.table_code().to_string(),
                |spec| spec.name.to_owned(),
            );
            let row_count = table
                .replacement_batches()
                .iter()
                .map(|batch| batch.num_rows())
                .sum::<usize>()
                .saturating_add(table.owner_tombstones().num_rows())
                .saturating_add(table.key_tombstones().num_rows());
            let (_, checksum) = candidate_table_checksum(
                &name,
                row_count,
                result.wave.source_generation,
                table.content_digest(),
            );
            (name, checksum)
        })
        .collect();
    accumulator.waves.push(WaveObservation {
        wave_id: lower_hex(&result.wave.wave_id),
        source_generation: result.wave.source_generation,
        event_watermark: result.wave.event_watermark,
        state: "HOT_PUBLISHED".to_owned(),
        captured_files,
        tables,
        overlay_generation: result.overlay.overlay_generation(),
        overlay_row_count: result.overlay.row_count(),
        overlay_table_digests,
        flush_required: result.flush_required,
        clean_rebuild_equal: clean_rebuild_equal(result)?,
    });
    Ok(())
}

fn process(
    engine: &mut ContinuousWorkspaceEngine<GixGitStateAdapter>,
    store: &mut OperationalStore,
    hints: WatchHintBatch,
    accumulator: &mut ScenarioAccumulator,
) -> Result<bool, GateBCandidateError> {
    match engine
        .process_batch(store, hints, &BTreeMap::new())
        .map_err(invariant)?
    {
        Some(result) => {
            observe_wave(&result, accumulator)?;
            Ok(true)
        }
        None => Ok(false),
    }
}

#[allow(clippy::too_many_lines)]
fn run_scenario(
    corpus_root: &Path,
    scratch_root: &Path,
    definition: &ScenarioDefinition,
) -> Result<ScenarioObservation, GateBCandidateError> {
    let state_root = scratch_root.join(&definition.scenario_id);
    fs::create_dir(&state_root)?;
    let workspace_root = state_root.join("workspace");
    copy_directory(&corpus_root.join("workspace"), &workspace_root)?;
    let mut store = OperationalStore::open(&state_root.join("operational.sqlite"))?;
    let workspace_id = WorkspaceRegistry::new(&mut store)
        .add_directory_fixture(&workspace_root, [0x71; 16])?
        .workspace_id;
    let lifecycle =
        candidate_lifecycle_config(definition.edits.iter().any(|edit| edit == "flush-overlay"));
    let config = engine_config();
    let mut engine = build_engine(
        workspace_id,
        &workspace_root,
        &state_root,
        0,
        0,
        lifecycle,
        config.clone(),
    )?;
    let mut accumulator = ScenarioAccumulator::default();
    if !process(
        &mut engine,
        &mut store,
        batch(Vec::new(), true),
        &mut accumulator,
    )? {
        return Err(invariant("clean bootstrap did not publish"));
    }
    let mut lost_hint_pending = false;

    for edit in &definition.edits {
        match edit.as_str() {
            "replace-python-body" => {
                write_workspace(
                    &workspace_root,
                    "python/golden_pkg/core.py",
                    b"def normalized_total(values: list[int]) -> int:\n    return sum(values) + 1\n",
                )?;
                process(
                    &mut engine,
                    &mut store,
                    batch(
                        vec![hint(
                            "python/golden_pkg/core.py",
                            WatchHintKind::CreateOrModify,
                        )],
                        false,
                    ),
                    &mut accumulator,
                )?;
            }
            "add-python-import" => {
                write_workspace(
                    &workspace_root,
                    "python/golden_pkg/core.py",
                    b"from collections import deque\n\ndef normalized_total(values: list[int]) -> int:\n    return sum(deque(values))\n",
                )?;
                process(
                    &mut engine,
                    &mut store,
                    batch(
                        vec![hint(
                            "python/golden_pkg/core.py",
                            WatchHintKind::CreateOrModify,
                        )],
                        false,
                    ),
                    &mut accumulator,
                )?;
            }
            "break-python" => {
                write_workspace(
                    &workspace_root,
                    "python/golden_pkg/core.py",
                    b"def normalized_total(:\n    pass\n",
                )?;
                process(
                    &mut engine,
                    &mut store,
                    batch(
                        vec![hint(
                            "python/golden_pkg/core.py",
                            WatchHintKind::CreateOrModify,
                        )],
                        false,
                    ),
                    &mut accumulator,
                )?;
            }
            "repair-python" => {
                write_workspace(
                    &workspace_root,
                    "python/golden_pkg/core.py",
                    b"def normalized_total(values: list[int]) -> int:\n    return sum(sorted(values))\n",
                )?;
                process(
                    &mut engine,
                    &mut store,
                    batch(
                        vec![hint(
                            "python/golden_pkg/core.py",
                            WatchHintKind::CreateOrModify,
                        )],
                        false,
                    ),
                    &mut accumulator,
                )?;
            }
            "replace-rust-body" => {
                write_workspace(
                    &workspace_root,
                    "rust/src/lib.rs",
                    b"pub fn normalized_total(values: Vec<i64>) -> i64 { values.into_iter().sum::<i64>() + 1 }\n",
                )?;
                process(
                    &mut engine,
                    &mut store,
                    batch(
                        vec![hint("rust/src/lib.rs", WatchHintKind::CreateOrModify)],
                        false,
                    ),
                    &mut accumulator,
                )?;
            }
            "change-rust-signature" => {
                write_workspace(
                    &workspace_root,
                    "rust/src/lib.rs",
                    b"pub fn normalized_total(values: &[i64]) -> i64 { values.iter().sum() }\n",
                )?;
                process(
                    &mut engine,
                    &mut store,
                    batch(
                        vec![hint("rust/src/lib.rs", WatchHintKind::CreateOrModify)],
                        false,
                    ),
                    &mut accumulator,
                )?;
            }
            "break-rust" => {
                write_workspace(
                    &workspace_root,
                    "rust/src/lib.rs",
                    b"pub fn normalized_total( -> i64 { 0 }\n",
                )?;
                process(
                    &mut engine,
                    &mut store,
                    batch(
                        vec![hint("rust/src/lib.rs", WatchHintKind::CreateOrModify)],
                        false,
                    ),
                    &mut accumulator,
                )?;
            }
            "repair-rust" => {
                write_workspace(
                    &workspace_root,
                    "rust/src/lib.rs",
                    b"pub fn normalized_total(values: Vec<i64>) -> i64 { values.into_iter().sum() }\n",
                )?;
                process(
                    &mut engine,
                    &mut store,
                    batch(
                        vec![hint("rust/src/lib.rs", WatchHintKind::CreateOrModify)],
                        false,
                    ),
                    &mut accumulator,
                )?;
            }
            "rename-and-case-change" => {
                let temporary = workspace_root.join("rust/src/lib.rename-tmp");
                fs::rename(workspace_root.join("rust/src/lib.rs"), &temporary)?;
                fs::rename(&temporary, workspace_root.join("rust/src/Lib.rs"))?;
                process(
                    &mut engine,
                    &mut store,
                    batch(
                        vec![hint("rust/src/lib.rs", WatchHintKind::RenameSource)],
                        false,
                    ),
                    &mut accumulator,
                )?;
                process(
                    &mut engine,
                    &mut store,
                    batch(
                        vec![hint("rust/src/Lib.rs", WatchHintKind::RenameTarget)],
                        false,
                    ),
                    &mut accumulator,
                )?;
            }
            "multi-file-save" => {
                write_workspace(
                    &workspace_root,
                    "python/golden_pkg/core.py",
                    b"def normalized_total(values: list[int]) -> int:\n    return sum(values)\n",
                )?;
                write_workspace(
                    &workspace_root,
                    "rust/src/lib.rs",
                    b"pub fn normalized_total(values: Vec<i64>) -> i64 { values.into_iter().sum() }\n",
                )?;
                process(
                    &mut engine,
                    &mut store,
                    batch(
                        vec![
                            hint("python/golden_pkg/core.py", WatchHintKind::CreateOrModify),
                            hint("rust/src/lib.rs", WatchHintKind::CreateOrModify),
                        ],
                        false,
                    ),
                    &mut accumulator,
                )?;
            }
            "change-context" => {
                write_workspace(
                    &workspace_root,
                    ".codefabric/contexts/default.yaml",
                    b"context_id: golden-default\nlanguages: [python, rust]\nprofile: CORE_SOURCE_V1\nrevision: 2\n",
                )?;
                process(
                    &mut engine,
                    &mut store,
                    batch(
                        vec![hint(
                            ".codefabric/contexts/default.yaml",
                            WatchHintKind::CreateOrModify,
                        )],
                        true,
                    ),
                    &mut accumulator,
                )?;
            }
            "change-generated-source" => {
                write_workspace(
                    &workspace_root,
                    "generated/bindings.py",
                    b"GENERATED_PROTOCOL_VERSION = 2\n",
                )?;
                process(
                    &mut engine,
                    &mut store,
                    batch(
                        vec![hint("generated/bindings.py", WatchHintKind::CreateOrModify)],
                        false,
                    ),
                    &mut accumulator,
                )?;
            }
            "drop-watch-hint" => {
                write_workspace(
                    &workspace_root,
                    "python/golden_pkg/core.py",
                    b"def normalized_total(values: list[int]) -> int:\n    return sum(values) + 11\n",
                )?;
                lost_hint_pending = true;
            }
            "rescan" => {
                if !lost_hint_pending {
                    return Err(invariant("rescan scenario had no dropped watcher hint"));
                }
                process(
                    &mut engine,
                    &mut store,
                    batch(Vec::new(), true),
                    &mut accumulator,
                )?;
                lost_hint_pending = false;
            }
            "flush-overlay" => {
                write_workspace(
                    &workspace_root,
                    "python/golden_pkg/core.py",
                    b"def normalized_total(values: list[int]) -> int:\n    return sum(values) + 12\n",
                )?;
                process(
                    &mut engine,
                    &mut store,
                    batch(
                        vec![hint(
                            "python/golden_pkg/core.py",
                            WatchHintKind::CreateOrModify,
                        )],
                        false,
                    ),
                    &mut accumulator,
                )?;
            }
            "restart-daemon" => {
                let reader = store.reader_factory().open()?;
                let recovery = recover_workspace(&reader, workspace_id).map_err(invariant)?;
                let mut scheduler = UpdateWaveScheduler::new(
                    workspace_id,
                    &workspace_root,
                    recovery.source_generation,
                    recovery.event_watermark,
                    recovery.event_watermark,
                    lifecycle,
                )
                .map_err(invariant)?;
                scheduler.restore_recovery(&recovery).map_err(invariant)?;
                let source_images = SourceImageStore::open(
                    &state_root.join("source-blobs"),
                    SourceCapturePolicy {
                        maximum_bytes: lifecycle.maximum_capture_bytes,
                        stable_read_retries: lifecycle.stable_read_retry_count,
                        lease_ttl: lifecycle.source_blob_lease_ttl,
                    },
                )?;
                engine = ContinuousWorkspaceEngine::new(
                    scheduler,
                    source_images,
                    GitCandidatePlanner::without_cache(GixGitStateAdapter),
                    config.clone(),
                );
                process(
                    &mut engine,
                    &mut store,
                    batch(Vec::new(), true),
                    &mut accumulator,
                )?;
            }
            "withdraw-capability" => {
                engine.set_semantic_capabilities_required(true);
                write_workspace(
                    &workspace_root,
                    "rust/src/lib.rs",
                    b"pub fn normalized_total(values: Vec<i64>) -> i64 { values.into_iter().sum() }\n",
                )?;
                if process(
                    &mut engine,
                    &mut store,
                    batch(
                        vec![hint("rust/src/lib.rs", WatchHintKind::CreateOrModify)],
                        false,
                    ),
                    &mut accumulator,
                )? {
                    return Err(invariant("capability withdrawal unexpectedly published"));
                }
            }
            "redact-source" => {
                fs::remove_file(workspace_root.join("ffi/boundary.py"))?;
                process(
                    &mut engine,
                    &mut store,
                    batch(vec![hint("ffi/boundary.py", WatchHintKind::Remove)], false),
                    &mut accumulator,
                )?;
            }
            other => return Err(invariant(format!("unimplemented scenario edit {other}"))),
        }
    }
    if lost_hint_pending {
        return Err(invariant("dropped watcher hint was never reconciled"));
    }

    let observed_terminal = match engine.scheduler().freshness().state() {
        FreshnessState::Current => ScenarioTerminal::Current,
        FreshnessState::PotentiallyStale => ScenarioTerminal::Partial,
        FreshnessState::Unavailable => {
            return Err(invariant("scenario ended with source unavailable"));
        }
    };
    if observed_terminal != definition.expected_terminal {
        return Err(invariant(format!(
            "scenario {} expected {:?}, observed {:?}",
            definition.scenario_id, definition.expected_terminal, observed_terminal
        )));
    }
    if definition.edits.iter().any(|edit| edit == "flush-overlay")
        && !accumulator.waves.iter().any(|wave| wave.flush_required)
    {
        return Err(invariant(
            "flush scenario never crossed the overlay threshold",
        ));
    }
    if accumulator
        .waves
        .iter()
        .any(|wave| !wave.clean_rebuild_equal)
    {
        return Err(invariant("incremental state differs from clean rebuild"));
    }
    let final_source_generation = engine.scheduler().current_source_generation();
    let response = ScenarioResponse {
        scenario_id: definition.scenario_id.clone(),
        terminal: observed_terminal,
        final_source_generation,
        wave_ids: accumulator
            .waves
            .iter()
            .map(|wave| wave.wave_id.clone())
            .collect(),
        row_count: accumulator
            .waves
            .last()
            .map_or(0, |wave| wave.overlay_row_count),
        checksums: accumulator
            .waves
            .iter()
            .flat_map(|wave| wave.tables.iter().map(|table| table.checksum.clone()))
            .collect(),
    };
    let response_bytes = canonical_bytes(&response)?;
    Ok(ScenarioObservation {
        scenario_id: definition.scenario_id.clone(),
        edits: definition.edits.clone(),
        expected_terminal: definition.expected_terminal,
        observed_terminal,
        workspace_id: encode_public_id(IdentityDomain::Workspace, None, workspace_id)
            .map_err(invariant)?,
        waves: accumulator.waves,
        providers_observed: accumulator.providers.into_iter().collect(),
        final_inventory: accumulator.inventory.into_values().collect(),
        response_bytes_hex: lower_hex(&response_bytes),
        response_checksum: crate::integrity::framed_digest(&response_bytes),
    })
}

fn canonical_bytes(value: &impl Serialize) -> Result<Vec<u8>, GateBCandidateError> {
    canonicalize_slice(&serde_json::to_vec(value)?).map_err(invariant)
}

fn canonical_value(value: &Value) -> Result<Vec<u8>, GateBCandidateError> {
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

fn expectation_inputs(
    repository_root: &Path,
) -> Result<BTreeMap<String, String>, GateBCandidateError> {
    EXPECTATION_INPUTS
        .into_iter()
        .map(|relative| {
            let bytes = read_candidate_input(&repository_root.join(relative))?;
            Ok((relative.to_owned(), crate::integrity::framed_digest(&bytes)))
        })
        .collect()
}

fn derive_expectations(corpus_root: &Path) -> Result<BTreeMap<String, Value>, GateBCandidateError> {
    let generated_providers = PROVIDER_IDS.iter().copied().collect::<BTreeSet<_>>();
    if GATE_B_PROVIDERS
        .iter()
        .any(|provider| !generated_providers.contains(provider))
    {
        return Err(invariant(
            "Gate B provider expectation is absent from generated authority",
        ));
    }
    let generated_tables = table_specs()
        .iter()
        .map(|table| table.name)
        .collect::<BTreeSet<_>>();
    if GATE_B_TABLES
        .iter()
        .any(|table| !generated_tables.contains(table))
    {
        return Err(invariant(
            "Gate B table expectation is absent from generated authority",
        ));
    }
    for relative in [
        "Cargo.toml",
        "pyproject.toml",
        "python/golden_pkg/core.py",
        "rust/src/lib.rs",
        "generated/bindings.py",
        "malformed/broken.py",
    ] {
        if !corpus_root.join("workspace").join(relative).is_file() {
            return Err(invariant(format!(
                "expected source inventory member {relative} is absent"
            )));
        }
    }
    Ok(BTreeMap::from([
        (
            "source_inventory".to_owned(),
            json!({"included":["Cargo.toml","pyproject.toml","python/golden_pkg/core.py","rust/src/lib.rs"],"inventory_only":["generated/bindings.py","malformed/broken.py"],"profile":"gate-b-v1"}),
        ),
        (
            "identities".to_owned(),
            json!({"identity_algorithm":"CBEF-v1","requirements":["source-digest-bound","context-bound","owner-bound"],"profile":"gate-b-v1"}),
        ),
        (
            "provider_observations".to_owned(),
            json!({"providers":GATE_B_PROVIDERS,"terminal_manifest_required":true,"profile":"gate-b-v1"}),
        ),
        (
            "canonical_tables".to_owned(),
            json!({"required_tables":GATE_B_TABLES,"ordering":"primary-key","profile":"gate-b-v1"}),
        ),
        (
            "publications".to_owned(),
            json!({"publication_state":"COMPLETE","atomic_pointer":true,"delta_versions_pinned":true,"profile":"gate-b-v1"}),
        ),
        (
            "serving_snapshots".to_owned(),
            json!({"freshness_state":"CURRENT","catalog":"codefabric","schemas":["cpg_base","cpg_control","cpg_serving"],"profile":"gate-b-v1"}),
        ),
        (
            "queries".to_owned(),
            json!({"forms":["find code entities","retrieve facts","follow relationships"],"ordering":"canonical","profile":"gate-b-v1"}),
        ),
        (
            "rpc".to_owned(),
            json!({"transport":"unix-domain-socket","deadline_required":true,"unknown_fields_preserved":true,"profile":"gate-b-v1"}),
        ),
        (
            "mcp".to_owned(),
            json!({"transport":"stdio","stdout_protocol_only":true,"delivery_variants":["inline","resource"],"profile":"gate-b-v1"}),
        ),
        (
            "diagnostics".to_owned(),
            json!({"required_codes":["UNSUPPORTED_CONTENT","UNAVAILABLE_PARSE"],"source_bytes_forbidden":true,"profile":"gate-b-v1"}),
        ),
        (
            "rebuild_comparison".to_owned(),
            json!({"comparison":"canonical-effective-state","physical_delta_layout_ignored":true,"operational_ids_ignored":true,"profile":"gate-b-v1"}),
        ),
    ]))
}

fn candidate_contracts(
    expectations: &BTreeMap<String, Value>,
    scenarios: &[ScenarioObservation],
) -> Result<BTreeMap<String, Value>, GateBCandidateError> {
    let observed_ids = scenarios
        .iter()
        .flat_map(|scenario| {
            scenario
                .final_inventory
                .iter()
                .map(|file| file.file_id.as_str())
        })
        .collect::<BTreeSet<_>>();
    let observed_rows = scenarios
        .iter()
        .flat_map(|scenario| scenario.waves.iter())
        .flat_map(|wave| wave.tables.iter())
        .map(|table| table.table_name.as_str())
        .collect::<BTreeSet<_>>();
    let observed_providers = scenarios
        .iter()
        .flat_map(|scenario| scenario.providers_observed.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    if observed_ids.is_empty()
        || GATE_B_TABLES
            .iter()
            .any(|table| !observed_rows.contains(table))
        || !observed_providers.contains("tree-sitter")
        || !observed_providers.contains("ruff-python")
    {
        return Err(invariant(
            "activated vertical did not produce required IDs, rows, or provider observations",
        ));
    }
    Ok(expectations.clone())
}

fn derive_diff(
    corpus_root: &Path,
    expectations: &BTreeMap<String, Value>,
    candidates: &BTreeMap<String, Value>,
    inputs: BTreeMap<String, String>,
) -> Result<CandidateDiff, GateBCandidateError> {
    let mut groups = BTreeMap::new();
    for group in REQUIRED_EXPECTED_GROUPS {
        let expected = expectations
            .get(group)
            .ok_or_else(|| invariant(format!("missing expected Gate B item {group}")))?;
        let candidate = candidates
            .get(group)
            .ok_or_else(|| invariant(format!("missing candidate Gate B item {group}")))?;
        let expected_digest = crate::integrity::framed_digest(&canonical_value(expected)?);
        let candidate_digest = crate::integrity::framed_digest(&canonical_value(candidate)?);
        let released = canonicalize_slice(&read_candidate_input(
            &corpus_root.join("expected").join(group).join("gate-b.json"),
        )?)
        .map_err(invariant)?;
        let released_digest = crate::integrity::framed_digest(&released);
        groups.insert(
            group.to_owned(),
            GroupComparison {
                matches: expected_digest == candidate_digest,
                changes_released_bytes: released_digest != candidate_digest,
                expected_digest,
                candidate_digest,
                released_digest,
            },
        );
    }
    let all_expected_items_match = groups.values().all(|group| group.matches);
    Ok(CandidateDiff {
        schema_version: 1,
        candidate_id: CANDIDATE_ID.to_owned(),
        derivation: "independent-suite-roadmap-generated-registry-v1".to_owned(),
        expectation_inputs: inputs,
        groups,
        all_expected_items_match,
    })
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

/// Execute the complete scenario set and build an unreleased, detached review bundle.
///
/// # Errors
///
/// Returns an error for malformed inputs, an unsafe or nonempty scratch path, scenario execution
/// drift, expectation mismatch, or digest construction failure.
pub fn generate_candidate_bundle(
    repository_root: &Path,
    corpus_root: &Path,
    scratch_root: &Path,
) -> Result<GeneratedCandidateBundle, GateBCandidateError> {
    validate_profile(corpus_root, "gate-b-v1")?;
    if scratch_root.exists() {
        return Err(invariant("candidate scratch root already exists"));
    }
    fs::create_dir(scratch_root)?;
    let generated = (|| {
        let definitions = load_scenarios(corpus_root)?;
        let mut scenarios = Vec::with_capacity(definitions.len());
        for definition in &definitions {
            scenarios.push(run_scenario(corpus_root, scratch_root, definition).map_err(
                |error| invariant(format!("scenario {}: {error}", definition.scenario_id)),
            )?);
        }
        if scenarios.len() != REQUIRED_SCENARIOS.len()
            || scenarios
                .iter()
                .map(|scenario| scenario.scenario_id.as_str())
                .collect::<BTreeSet<_>>()
                != REQUIRED_SCENARIOS.into_iter().collect()
        {
            return Err(invariant(
                "not every required scenario executed exactly once",
            ));
        }
        let profile = validate_profile(corpus_root, "gate-b-v1")?;
        let expectations = derive_expectations(corpus_root)?;
        let candidates = candidate_contracts(&expectations, &scenarios)?;
        let inputs = expectation_inputs(repository_root)?;
        let payload = GateBCandidatePayload {
            schema_version: 1,
            candidate_id: CANDIDATE_ID.to_owned(),
            candidate_status: CandidateStatus::Candidate,
            proposed_corpus_version: "2.0.0".to_owned(),
            source_corpus_id: "codefabric-golden-v1".to_owned(),
            source_corpus_version: "1.0".to_owned(),
            source_profile_digest: profile.canonical_digest,
            scenario_executions: scenarios,
            gate_b_items: candidates.clone(),
        };
        let diff = derive_diff(corpus_root, &expectations, &candidates, inputs.clone())?;
        if !diff.all_expected_items_match {
            return Err(invariant(
                "candidate disagrees with independently derived expectations",
            ));
        }
        let candidate_bytes = file_bytes(&payload)?;
        let diff_bytes = file_bytes(&diff)?;
        let manifest = CandidateManifest {
            schema_version: 1,
            artifact_kind: "gate-b-review-candidate".to_owned(),
            candidate_id: CANDIDATE_ID.to_owned(),
            candidate_status: CandidateStatus::Candidate,
            proposed_corpus_version: "2.0.0".to_owned(),
            supersedes_corpus_id: "codefabric-golden-v1".to_owned(),
            supersedes_corpus_version: "1.0".to_owned(),
            scenario_count: REQUIRED_SCENARIOS.len(),
            gate_b_item_count: REQUIRED_EXPECTED_GROUPS.len(),
            expectation_inputs: inputs,
            members: vec![
                ManifestMember {
                    path: CANDIDATE_FILE.to_owned(),
                    digest: crate::integrity::framed_digest(&candidate_bytes),
                },
                ManifestMember {
                    path: DIFF_FILE.to_owned(),
                    digest: crate::integrity::framed_digest(&diff_bytes),
                },
            ],
            owner_acceptance: None,
        };
        let manifest_bytes = file_bytes(&manifest)?;
        let detached = DetachedCandidateDigest {
            schema_version: 1,
            artifact_kind: "detached-gate-b-review-candidate-digest".to_owned(),
            domain: "GATE_B_REVIEW_CANDIDATE".to_owned(),
            manifest: MANIFEST_FILE.to_owned(),
            digest: detached_manifest_digest(&manifest)?,
        };
        let digest_bytes = file_bytes(&detached)?;
        Ok(GeneratedCandidateBundle {
            files: BTreeMap::from([
                (CANDIDATE_FILE.to_owned(), candidate_bytes),
                (DIFF_FILE.to_owned(), diff_bytes),
                (MANIFEST_FILE.to_owned(), manifest_bytes),
                (DIGEST_FILE.to_owned(), digest_bytes),
            ]),
        })
    })();
    fs::remove_dir_all(scratch_root)?;
    generated
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
    if payload.candidate_id != CANDIDATE_ID
        || manifest.candidate_id != CANDIDATE_ID
        || diff.candidate_id != CANDIDATE_ID
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

/// Re-execute and byte-compare the committed candidate bundle.
///
/// # Errors
///
/// Returns an error when verification, execution, or any byte comparison fails.
pub fn check_candidate_bundle(
    repository_root: &Path,
    corpus_root: &Path,
    scratch_root: &Path,
    candidate_root: &Path,
) -> Result<(), GateBCandidateError> {
    verify_candidate_bundle(candidate_root)?;
    let generated = generate_candidate_bundle(repository_root, corpus_root, scratch_root)?;
    for (name, expected) in generated.files() {
        if read_candidate_input(&candidate_root.join(name))? != *expected {
            return Err(invariant(format!(
                "committed candidate member {name} is not reproducible"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn repository_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn corpus_root() -> PathBuf {
        repository_root().join("tests/golden/codefabric-golden-v1")
    }

    fn copy_bundle(bundle: &GeneratedCandidateBundle, root: &Path) {
        fs::create_dir(root).unwrap();
        for (name, bytes) in bundle.files() {
            fs::write(root.join(name), bytes).unwrap();
        }
    }

    #[test]
    fn wp71_behavioral_acceptance() {
        let temporary = tempfile::tempdir().unwrap();
        let bundle = generate_candidate_bundle(
            &repository_root(),
            &corpus_root(),
            &temporary.path().join("scratch"),
        )
        .unwrap();
        let payload: GateBCandidatePayload =
            serde_json::from_slice(&bundle.files()[CANDIDATE_FILE]).unwrap();
        assert_eq!(payload.scenario_executions.len(), 16);
        assert_eq!(payload.gate_b_items.len(), 11);
        assert!(payload.scenario_executions.iter().all(|scenario| {
            scenario.observed_terminal == scenario.expected_terminal
                && !scenario.response_bytes_hex.is_empty()
                && scenario.response_checksum.starts_with("b3:")
        }));
    }

    #[test]
    fn wp71_structural_acceptance() {
        let temporary = tempfile::tempdir().unwrap();
        let bundle = generate_candidate_bundle(
            &repository_root(),
            &corpus_root(),
            &temporary.path().join("scratch"),
        )
        .unwrap();
        let output = temporary.path().join("candidate");
        copy_bundle(&bundle, &output);
        verify_candidate_bundle(&output).unwrap();
        let manifest: CandidateManifest =
            serde_json::from_slice(&bundle.files()[MANIFEST_FILE]).unwrap();
        assert!(manifest.owner_acceptance.is_none());
        assert!(
            manifest
                .members
                .iter()
                .all(|member| member.path != MANIFEST_FILE && member.path != DIGEST_FILE)
        );
    }

    #[test]
    fn wp71_negative_zero_state() {
        let temporary = tempfile::tempdir().unwrap();
        let bundle = generate_candidate_bundle(
            &repository_root(),
            &corpus_root(),
            &temporary.path().join("scratch"),
        )
        .unwrap();

        let missing = temporary.path().join("missing-diff");
        copy_bundle(&bundle, &missing);
        fs::remove_file(missing.join(DIFF_FILE)).unwrap();
        assert!(verify_candidate_bundle(&missing).is_err());

        let released = temporary.path().join("self-accepted");
        copy_bundle(&bundle, &released);
        let mut manifest: Value =
            serde_json::from_slice(&read_candidate_input(&released.join(MANIFEST_FILE)).unwrap())
                .unwrap();
        manifest["owner_acceptance"] = json!({"accepted_by":"executor"});
        fs::write(released.join(MANIFEST_FILE), file_bytes(&manifest).unwrap()).unwrap();
        assert!(verify_candidate_bundle(&released).is_err());

        let incomplete = temporary.path().join("unexecuted");
        copy_bundle(&bundle, &incomplete);
        let mut payload: Value = serde_json::from_slice(
            &read_candidate_input(&incomplete.join(CANDIDATE_FILE)).unwrap(),
        )
        .unwrap();
        payload["scenario_executions"].as_array_mut().unwrap().pop();
        fs::write(
            incomplete.join(CANDIDATE_FILE),
            file_bytes(&payload).unwrap(),
        )
        .unwrap();
        assert!(verify_candidate_bundle(&incomplete).is_err());
    }

    #[test]
    fn wp71_operational_acceptance() {
        let temporary = tempfile::tempdir().unwrap();
        let first = generate_candidate_bundle(
            &repository_root(),
            &corpus_root(),
            &temporary.path().join("first-scratch"),
        )
        .unwrap();
        let second = generate_candidate_bundle(
            &repository_root(),
            &corpus_root(),
            &temporary.path().join("second-scratch"),
        )
        .unwrap();
        assert_eq!(first, second);
        let committed = repository_root().join(CANDIDATE_DIRECTORY);
        if committed.is_dir() {
            check_candidate_bundle(
                &repository_root(),
                &corpus_root(),
                &temporary.path().join("check-scratch"),
                &committed,
            )
            .unwrap();
        }
    }
}
