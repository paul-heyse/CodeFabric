//! Executable Gate B review-candidate generation.
//!
//! Candidate bytes are deliberately unreleased. The runner applies the closed scenario edit
//! vocabulary through [`crate::continuous::ContinuousWorkspaceEngine`], derives expectations from
//! normative inputs and generated registries on a separate path, and emits a detached digest chain
//! that WP76 can review but only an accountable owner can accept.

pub(crate) mod vertical;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use serde_json::Value;
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
use crate::registries::UpdateWaveState;
use crate::schema_registry::table_spec;
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
const EXPECTATION_INPUTS: [&str; 10] = [
    "contracts/registry/design-principle-registry.yaml",
    "contracts/schema/provider-observations/pyrefly-module-v1.json",
    "docs/library_ref/full_data_fabric_design_principles.md",
    "docs/upfront_design/codefabric_1.3_implementation_roadmap_v1.0.md",
    "docs/upfront_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md",
    "pyrefly-sidecar/Cargo.lock",
    "rustc-extractor/toolchain-identity.json",
    "src/generated/registries.rs",
    "src/generated/table_specs.rs",
    "tests/golden/codefabric-golden-v2/corpus-manifest.json",
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
    #[serde(alias = "clean_rebuild_equal")]
    fast_syntax_replay_equal: bool,
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

fn fast_syntax_replay_equal(result: &ContinuousWaveResult) -> Result<bool, GateBCandidateError> {
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
        fast_syntax_replay_equal: fast_syntax_replay_equal(result)?,
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
                if process(
                    &mut engine,
                    &mut store,
                    batch(Vec::new(), true),
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
        .any(|wave| !wave.fast_syntax_replay_equal)
    {
        return Err(invariant("fast syntax replay is nondeterministic"));
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

pub(crate) fn read_candidate_artifact(path: &Path) -> Result<Vec<u8>, GateBCandidateError> {
    read_candidate_input(path)
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

#[allow(clippy::too_many_lines)] // The closed eleven-plane contract remains exhaustive in one dispatcher.
fn requirement_checks(group: &str, value: &Value) -> Result<Vec<String>, GateBCandidateError> {
    let object = value.as_object();
    if value.get("status").and_then(Value::as_str) == Some("NOT_REACHED") {
        return Err(invariant(format!(
            "actual Gate B plane {group} was not reached"
        )));
    }
    let mut checks = vec!["actual-output-object".to_owned()];
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
            checks.push("python-and-rust-source-captured".to_owned());
        }
        "provider_observations" => {
            let object = object.ok_or_else(|| invariant("provider plane is not an object"))?;
            for provider in ["pyrefly", "rustc_mir"] {
                if !object.contains_key(provider) {
                    return Err(invariant(format!("actual provider plane lacks {provider}")));
                }
            }
            checks.push("real-python-and-rust-semantic-providers".to_owned());
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
            checks.push("canonical-semantic-families-present".to_owned());
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
            checks.push("delta-versions-and-validation-recorded".to_owned());
        }
        "serving_snapshots" => {
            let object = object.ok_or_else(|| invariant("snapshot plane is not an object"))?;
            if object.get("source_trust_state").and_then(Value::as_str) != Some("CURRENT") {
                return Err(invariant("serving snapshot is not current"));
            }
            checks.push("snapshot-pins-complete-publication".to_owned());
        }
        "queries" => {
            let object = object.ok_or_else(|| invariant("query plane is not an object"))?;
            if object.get("form_count").and_then(Value::as_u64) != Some(8) {
                return Err(invariant("query plane did not execute all eight forms"));
            }
            checks.push("eight-form-composed-request".to_owned());
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
            checks.push("production-uds-stream".to_owned());
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
            checks.push("locked-fastmcp-stdio".to_owned());
        }
        "diagnostics" => {
            let object = object.ok_or_else(|| invariant("diagnostic plane is not an object"))?;
            if object.get("artifact_persisted").and_then(Value::as_bool) != Some(true) {
                return Err(invariant(
                    "diagnostic/artifact plane lacks persisted artifact",
                ));
            }
            checks.push("terminal-plan-and-result-artifacts".to_owned());
        }
        "rebuild_comparison" => {
            let object = object.ok_or_else(|| invariant("rebuild plane is not an object"))?;
            if object.get("inventory_equal").and_then(Value::as_bool) != Some(true) {
                return Err(invariant("rebuild comparison inventory differs"));
            }
            checks.push("independent-clean-rebuild".to_owned());
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
            checks.push("correlated-application-identities".to_owned());
        }
        other => return Err(invariant(format!("unknown Gate B plane {other}"))),
    }
    Ok(checks)
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
        requirement_checks(group, &execution.planes[group])?;
    }
    Ok(())
}

fn remove_fields(value: &mut Value, fields: &[&str]) {
    if let Some(object) = value.as_object_mut() {
        for field in fields {
            object.remove(*field);
        }
    }
}

fn normalize_snapshot_summary(value: &mut Value) {
    remove_fields(
        value,
        &[
            "snapshot_id",
            "durable_base_publication",
            "base_table_version_digest",
            "source_generation",
            "overlay_generation",
            "overlay_checksum",
        ],
    );
}

fn normalize_gate_b_planes(planes: &mut Value) {
    let Some(planes) = planes.as_object_mut() else {
        return;
    };
    if let Some(identities) = planes.get_mut("identities") {
        remove_fields(identities, &["hot_wave_id", "clean_wave_id"]);
    }
    if let Some(providers) = planes
        .get_mut("provider_observations")
        .and_then(Value::as_object_mut)
    {
        for provider in providers.values_mut() {
            remove_fields(provider, &["provider_run_id"]);
        }
    }
    if let Some(publication) = planes.get_mut("publications") {
        remove_fields(publication, &["publication_id", "pointer_generation"]);
        if let Some(tables) = publication.get_mut("tables").and_then(Value::as_object_mut) {
            for table in tables.values_mut() {
                remove_fields(table, &["delta_version", "checksum"]);
            }
        }
    }
    if let Some(snapshot) = planes.get_mut("serving_snapshots") {
        remove_fields(
            snapshot,
            &[
                "snapshot_id",
                "publication_id",
                "source_generation",
                "manifest_digest",
            ],
        );
    }
    if let Some(queries) = planes.get_mut("queries") {
        remove_fields(
            queries,
            &["response_digest", "response_bytes_hex", "snapshot_id"],
        );
    }
    if let Some(rpc) = planes.get_mut("rpc") {
        remove_fields(rpc, &["artifact_id", "mcp_call_id"]);
    }
    if let Some(structured) = planes
        .get_mut("mcp")
        .and_then(|mcp| mcp.get_mut("structured_content"))
    {
        if let Some(snapshot) = structured.get_mut("snapshot") {
            normalize_snapshot_summary(snapshot);
        }
        if let Some(delivery) = structured.get_mut("delivery") {
            remove_fields(delivery, &["checksum", "result_bytes"]);
            if let Some(snapshot) = delivery
                .get_mut("response")
                .and_then(|response| response.get_mut("snapshot"))
            {
                normalize_snapshot_summary(snapshot);
            }
        }
    }
}

fn functional_candidate_projection(
    payload: &GateBCandidatePayload,
) -> Result<Value, GateBCandidateError> {
    let mut projection = serde_json::to_value(payload)?;
    if let Some(scenarios) = projection
        .get_mut("scenario_executions")
        .and_then(Value::as_array_mut)
    {
        for scenario in scenarios {
            remove_fields(
                scenario,
                &[
                    "final_source_generation",
                    "wave_ids",
                    "response_bytes_hex",
                    "response_checksum",
                ],
            );
            if let Some(waves) = scenario.get_mut("waves").and_then(Value::as_array_mut) {
                for wave in waves {
                    remove_fields(
                        wave,
                        &[
                            "wave_id",
                            "source_generation",
                            "event_watermark",
                            "overlay_generation",
                            "overlay_table_digests",
                        ],
                    );
                    if let Some(tables) = wave.get_mut("tables").and_then(Value::as_array_mut) {
                        for table in tables {
                            remove_fields(table, &["checksum"]);
                        }
                    }
                }
            }
        }
    }
    if let Some(execution) = projection
        .get_mut("vertical_execution")
        .and_then(Value::as_object_mut)
    {
        for field in [
            "source_generation",
            "publication_id",
            "snapshot_id",
            "provider_run_ids",
            "execution_digest",
        ] {
            execution.remove(field);
        }
        if let Some(planes) = execution.get_mut("planes") {
            normalize_gate_b_planes(planes);
        }
    }
    if let Some(planes) = projection.get_mut("gate_b_items") {
        normalize_gate_b_planes(planes);
    }
    Ok(projection)
}

fn validate_current_comparison(
    group: &str,
    candidate: &Value,
    comparison: &GroupComparison,
) -> Result<(), GateBCandidateError> {
    let checks = requirement_checks(group, candidate)?;
    let expected_digest = crate::integrity::framed_digest(&canonical_bytes(&checks)?);
    let candidate_digest = crate::integrity::framed_digest(&canonical_value(candidate)?);
    if comparison.requirement_checks != checks
        || comparison.expected_digest != expected_digest
        || comparison.candidate_digest != candidate_digest
        || comparison.expected_digest == comparison.candidate_digest
        || comparison.released_digest.is_empty()
        || !comparison.matches
    {
        return Err(invariant(format!(
            "candidate requirement/prior-release comparison differs for {group}"
        )));
    }
    Ok(())
}

fn derive_diff(
    corpus_root: &Path,
    candidates: &BTreeMap<String, Value>,
    inputs: BTreeMap<String, String>,
) -> Result<CandidateDiff, GateBCandidateError> {
    let mut groups = BTreeMap::new();
    for group in REQUIRED_EXPECTED_GROUPS {
        let candidate = candidates
            .get(group)
            .ok_or_else(|| invariant(format!("missing candidate Gate B item {group}")))?;
        let requirement_checks = requirement_checks(group, candidate)?;
        let expected_digest =
            crate::integrity::framed_digest(&canonical_bytes(&requirement_checks)?);
        let candidate_digest = crate::integrity::framed_digest(&canonical_value(candidate)?);
        let released = canonicalize_slice(&read_candidate_input(
            &corpus_root.join("expected").join(group).join("gate-b.json"),
        )?)
        .map_err(invariant)?;
        let released_digest = crate::integrity::framed_digest(&released);
        groups.insert(
            group.to_owned(),
            GroupComparison {
                matches: true,
                requirement_checks,
                changes_released_bytes: released_digest != candidate_digest,
                expected_digest,
                candidate_digest,
                released_digest,
            },
        );
    }
    let all_expected_items_match = groups
        .values()
        .all(|group| !group.requirement_checks.is_empty());
    Ok(CandidateDiff {
        schema_version: 1,
        candidate_id: CANDIDATE_ID.to_owned(),
        derivation: "independent-requirement-predicates-and-prior-release-diff-v2".to_owned(),
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
        let vertical_execution = vertical::execute(repository_root, corpus_root, scratch_root)?;
        validate_vertical_execution(&vertical_execution)?;
        let candidates = vertical_execution.planes.clone();
        let inputs = expectation_inputs(repository_root)?;
        let payload = GateBCandidatePayload {
            schema_version: 1,
            candidate_id: CANDIDATE_ID.to_owned(),
            candidate_status: CandidateStatus::Candidate,
            proposed_corpus_version: "3.0.0".to_owned(),
            source_corpus_id: "codefabric-golden-v2".to_owned(),
            source_corpus_version: "2.0.0".to_owned(),
            source_profile_digest: profile.canonical_digest,
            scenario_executions: scenarios,
            vertical_execution: Some(vertical_execution),
            gate_b_items: candidates.clone(),
        };
        let diff = derive_diff(corpus_root, &candidates, inputs.clone())?;
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
            proposed_corpus_version: "3.0.0".to_owned(),
            supersedes_corpus_id: "codefabric-golden-v2".to_owned(),
            supersedes_corpus_version: "2.0.0".to_owned(),
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
    let current = payload.candidate_id == CANDIDATE_ID;
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
    if current {
        let execution = payload
            .vertical_execution
            .as_ref()
            .ok_or_else(|| invariant("current candidate lacks vertical execution evidence"))?;
        validate_vertical_execution(execution)?;
        if payload.gate_b_items != execution.planes {
            return Err(invariant(
                "candidate Gate B items differ from actual vertical planes",
            ));
        }
        for group in REQUIRED_EXPECTED_GROUPS {
            validate_current_comparison(group, &payload.gate_b_items[group], &diff.groups[group])?;
        }
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

/// Re-execute and functionally compare the committed candidate bundle.
///
/// # Errors
///
/// Returns an error when verification, execution, or the governed functional outcome differs.
pub fn check_candidate_bundle(
    repository_root: &Path,
    corpus_root: &Path,
    scratch_root: &Path,
    candidate_root: &Path,
) -> Result<(), GateBCandidateError> {
    verify_candidate_bundle(candidate_root)?;
    let generated = generate_candidate_bundle(repository_root, corpus_root, scratch_root)?;
    let committed: GateBCandidatePayload =
        serde_json::from_slice(&read_candidate_input(&candidate_root.join(CANDIDATE_FILE))?)?;
    let current: GateBCandidatePayload =
        serde_json::from_slice(&generated.files()[CANDIDATE_FILE])?;
    if functional_candidate_projection(&committed)? != functional_candidate_projection(&current)? {
        return Err(invariant(
            "committed candidate functional outcome is not reproducible",
        ));
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
    repository_root: &Path,
    corpus_root: &Path,
    scratch_root: &Path,
    candidate_root: &Path,
) -> Result<(), GateBCandidateError> {
    verify_candidate_bundle(candidate_root)?;
    let committed = read_candidate_input(&candidate_root.join(CANDIDATE_FILE))?;
    let committed_payload: GateBCandidatePayload = serde_json::from_slice(&committed)?;
    // A prior accepted candidate is immutable historical evidence. Once a superseding
    // candidate changes the production executor and semantic payload, the old release keeps
    // its complete digest-chain verification but is not reinterpreted through the new
    // candidate generator. WP07 promotes the current candidate and restores semantic
    // re-execution against the matching production path.
    if committed_payload.candidate_id != CANDIDATE_ID {
        return Ok(());
    }
    let generated = generate_candidate_bundle(repository_root, corpus_root, scratch_root)?;
    let generated_payload: GateBCandidatePayload =
        serde_json::from_slice(&generated.files()[CANDIDATE_FILE])?;
    if functional_candidate_projection(&committed_payload)?
        != functional_candidate_projection(&generated_payload)?
    {
        return Err(invariant(
            "accepted candidate functional outcome is not reproducible",
        ));
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
        repository_root().join("tests/golden/codefabric-golden-v2")
    }

    fn copy_bundle(bundle: &GeneratedCandidateBundle, root: &Path) {
        fs::create_dir(root).unwrap();
        for (name, bytes) in bundle.files() {
            fs::write(root.join(name), bytes).unwrap();
        }
    }

    #[test]
    fn gate_b_vertical_slice_produces_all_eleven_planes() {
        let temporary = tempfile::tempdir().unwrap();
        let scratch = temporary.path().join("scratch");
        fs::create_dir(&scratch).unwrap();
        let execution = vertical::execute(&repository_root(), &corpus_root(), &scratch).unwrap();
        validate_vertical_execution(&execution).unwrap();
        assert_eq!(execution.planes.len(), 11);
        assert!(
            execution.planes["provider_observations"]["pyrefly"]["module_ids"]
                .as_array()
                .is_some_and(|values| !values.is_empty())
        );
        assert!(
            execution.planes["provider_observations"]["rustc_mir"]["owner_count"]
                .as_u64()
                .is_some_and(|value| value > 0)
        );
        assert_eq!(
            execution.planes["canonical_tables"]["contains_derived"],
            true
        );
        assert_eq!(
            execution.planes["canonical_tables"]["contains_unknown"],
            true
        );
        assert_eq!(execution.planes["queries"]["successful_query_count"], 8);
        assert_eq!(execution.planes["diagnostics"]["artifact_persisted"], true);
    }

    #[test]
    fn gate_b_candidate_independent_oracle_contract() {
        let candidate_source = include_str!("gate_b_candidate.rs");
        assert!(!candidate_source.contains(&["fn derive_", "expectations"].concat()));
        assert!(!candidate_source.contains(&["fn candidate_", "contracts"].concat()));
        assert!(!candidate_source.contains(&["expectations", ".clone()"].concat()));
        assert!(candidate_source.contains("vertical::execute"));
        assert!(candidate_source.contains("requirement_checks"));
        assert!(candidate_source.contains("released_digest"));
    }

    #[test]
    fn gate_b_vertical_slice_adversarial() {
        let temporary = tempfile::tempdir().unwrap();
        let scratch = temporary.path().join("scratch");
        fs::create_dir(&scratch).unwrap();
        let execution = vertical::execute(&repository_root(), &corpus_root(), &scratch).unwrap();

        let mut missing_rustc = execution.planes["provider_observations"].clone();
        missing_rustc.as_object_mut().unwrap().remove("rustc_mir");
        assert!(requirement_checks("provider_observations", &missing_rustc).is_err());

        let mut skipped_publication = execution.planes["publications"].clone();
        skipped_publication["tables"] = serde_json::json!({});
        assert!(requirement_checks("publications", &skipped_publication).is_err());

        let mut bypassed_uds = execution.planes["rpc"].clone();
        bypassed_uds["transport"] = serde_json::json!("in-process");
        assert!(requirement_checks("rpc", &bypassed_uds).is_err());

        let mut stubbed_adapter = execution.planes["mcp"].clone();
        stubbed_adapter["transport"] = serde_json::json!("in-process");
        assert!(requirement_checks("mcp", &stubbed_adapter).is_err());

        let mut dropped_event = execution.planes["rpc"].clone();
        dropped_event["event_kinds"] = serde_json::json!(["snapshot_pinned", "terminal"]);
        assert!(requirement_checks("rpc", &dropped_event).is_err());

        let mut uncorrelated_mcp = execution.planes["mcp"].clone();
        uncorrelated_mcp["mcp_call_id_correlated"] = serde_json::json!(false);
        assert!(requirement_checks("mcp", &uncorrelated_mcp).is_err());

        let mut dropped_artifact = execution.planes["diagnostics"].clone();
        dropped_artifact["artifact_persisted"] = serde_json::json!(false);
        assert!(requirement_checks("diagnostics", &dropped_artifact).is_err());

        let mut altered_row = execution.planes["canonical_tables"].clone();
        altered_row["contains_rust_mir"] = serde_json::json!(false);
        assert!(requirement_checks("canonical_tables", &altered_row).is_err());

        let mut hidden_unknown = execution.planes["canonical_tables"].clone();
        hidden_unknown["contains_unknown"] = serde_json::json!(false);
        assert!(requirement_checks("canonical_tables", &hidden_unknown).is_err());

        let diff = derive_diff(&corpus_root(), &execution.planes, BTreeMap::new()).unwrap();
        let mut self_expected = diff.groups["queries"].clone();
        self_expected.expected_digest = self_expected.candidate_digest.clone();
        assert!(
            validate_current_comparison("queries", &execution.planes["queries"], &self_expected,)
                .is_err()
        );
    }

    #[test]
    fn gate_b_candidate_operational_gate() {
        let temporary = tempfile::tempdir().unwrap();
        let bundle = generate_candidate_bundle(
            &repository_root(),
            &corpus_root(),
            &temporary.path().join("scratch"),
        )
        .unwrap();
        // A Gate B run intentionally allocates a new publication, serving snapshot, query
        // execution, result artifact, and transport checksums. Their exact bytes belong in the
        // review bundle and are verified by its detached digest chain; cross-run semantic
        // convergence is separately proved through the governed canonical Arrow projection.
        let output = temporary.path().join("candidate");
        copy_bundle(&bundle, &output);
        verify_candidate_bundle(&output).unwrap();
        let manifest: CandidateManifest =
            serde_json::from_slice(&bundle.files()[MANIFEST_FILE]).unwrap();
        assert_eq!(manifest.candidate_id, CANDIDATE_ID);
        assert!(manifest.owner_acceptance.is_none());

        let payload: GateBCandidatePayload =
            serde_json::from_slice(&bundle.files()[CANDIDATE_FILE]).unwrap();
        let expected_outcome = functional_candidate_projection(&payload).unwrap();
        let mut reallocated = payload.clone();
        let execution = reallocated.vertical_execution.as_mut().unwrap();
        execution.source_generation = execution.source_generation.saturating_add(99);
        execution.publication_id = "publication:another-run".to_owned();
        execution.snapshot_id = "snapshot:another-run".to_owned();
        execution.execution_digest = "b3:operationally-reallocated".to_owned();
        execution.planes.get_mut("publications").unwrap()["publication_id"] =
            serde_json::json!("publication:another-run");
        execution.planes.get_mut("publications").unwrap()["tables"]["1"]["checksum"] =
            serde_json::json!("b3:another-table-checksum");
        execution.planes.get_mut("serving_snapshots").unwrap()["publication_id"] =
            serde_json::json!("publication:another-run");
        execution.planes.get_mut("serving_snapshots").unwrap()["snapshot_id"] =
            serde_json::json!("snapshot:another-run");
        execution.planes.get_mut("queries").unwrap()["snapshot_id"] =
            serde_json::json!("snapshot:another-run");
        execution.planes.get_mut("rpc").unwrap()["artifact_id"] =
            serde_json::json!("artifact:another-run");
        execution.planes.get_mut("mcp").unwrap()["structured_content"]["snapshot"]["snapshot_id"] =
            serde_json::json!("snapshot:another-run");
        execution.planes.get_mut("mcp").unwrap()["structured_content"]["delivery"]["response"]["snapshot"]
            ["snapshot_id"] = serde_json::json!("snapshot:another-run");
        execution.planes.get_mut("mcp").unwrap()["structured_content"]["delivery"]["checksum"] =
            serde_json::json!("b3:another-delivery-checksum");
        reallocated.gate_b_items.clone_from(&execution.planes);
        assert_eq!(
            expected_outcome,
            functional_candidate_projection(&reallocated).unwrap()
        );

        let mut changed = payload;
        changed.gate_b_items.get_mut("canonical_tables").unwrap()["contains_rust_mir"] =
            serde_json::json!(false);
        changed
            .vertical_execution
            .as_mut()
            .unwrap()
            .planes
            .get_mut("canonical_tables")
            .unwrap()["contains_rust_mir"] = serde_json::json!(false);
        assert_ne!(
            expected_outcome,
            functional_candidate_projection(&changed).unwrap()
        );
    }
}
