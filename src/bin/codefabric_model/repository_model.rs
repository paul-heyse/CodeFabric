//! Byte-safe, bounded repository discovery and family claiming for the model compiler.

use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
use std::ffi::OsString;
use std::fs;
use std::io::Read as _;
use std::os::unix::ffi::OsStrExt as _;
#[cfg(test)]
use std::os::unix::ffi::OsStringExt as _;
use std::os::unix::fs::MetadataExt as _;
#[cfg(test)]
use std::path::PathBuf;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::model_control::{
    ModelError, ModelGraph, NodeDeclaration, NodeKind, ResourceBounds, StableId,
};

const MAX_HEADER_BYTES: usize = 1_048_576;
const MAX_DIAGNOSTIC_BYTES: usize = 512;

/// Fixed model-discovery bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InventoryBounds {
    /// Maximum directory depth below one fixed root.
    pub max_depth: usize,
    /// Maximum filesystem entries across all fixed roots.
    pub max_entries: usize,
    /// Maximum total bytes read from claimed regular files.
    pub max_total_bytes: u64,
    /// Maximum retained diagnostics.
    pub max_diagnostics: usize,
}

impl Default for InventoryBounds {
    fn default() -> Self {
        Self {
            max_depth: 32,
            max_entries: 20_000,
            max_total_bytes: 256 * 1024 * 1024,
            max_diagnostics: 32,
        }
    }
}

/// Native repository path that never uses display text as identity.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RepositoryPath {
    raw_bytes: Vec<u8>,
    display: String,
    display_is_lossy: bool,
}

impl RepositoryPath {
    fn from_relative(path: &Path) -> Result<Self, RepositoryModelError> {
        if path.is_absolute()
            || path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(RepositoryModelError::UnsafePath(
                path.to_string_lossy().into_owned(),
            ));
        }
        let display = path.to_string_lossy();
        Ok(Self {
            raw_bytes: path.as_os_str().as_bytes().to_vec(),
            display_is_lossy: matches!(display, std::borrow::Cow::Owned(_)),
            display: display.into_owned(),
        })
    }

    /// Display-only path rendering.
    #[must_use]
    pub fn display(&self) -> &str {
        &self.display
    }

    /// Whether display rendering replaced non-UTF-8 bytes.
    #[must_use]
    pub const fn display_is_lossy(&self) -> bool {
        self.display_is_lossy
    }
}

/// Read-only Git/worktree classification. Multiple states may apply to one path.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitPathState {
    /// Path exists in the index.
    Tracked,
    /// Index differs from HEAD.
    Staged,
    /// Worktree differs from the index.
    WorktreeModified,
    /// Path is not tracked and not ignored.
    Untracked,
    /// Path matches Git ignore rules.
    Ignored,
    /// Index contains conflict stages.
    Conflicted,
    /// Tracked path is absent from current filesystem bytes.
    Deleted,
    /// Filesystem-only fallback supplied the candidate.
    FilesystemOnly,
}

/// Detached selected-worktree topology.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorktreeTopology {
    /// Current worktree path bytes when non-bare.
    pub work_dir: Option<Vec<u8>>,
    /// Per-worktree Git directory bytes.
    pub git_dir: Option<Vec<u8>>,
    /// Common Git directory bytes.
    pub common_dir: Option<Vec<u8>>,
    /// Whether the selected worktree is linked.
    pub linked_worktree: bool,
    /// Whether Git acceleration was available.
    pub git_available: bool,
}

impl WorktreeTopology {
    fn filesystem_only(root: &Path) -> Self {
        Self {
            work_dir: Some(root.as_os_str().as_bytes().to_vec()),
            git_dir: None,
            common_dir: None,
            linked_worktree: false,
            git_available: false,
        }
    }
}

/// The four governed write-policy roles.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactRole {
    /// Normative or machine authority.
    Authority,
    /// Independent conformance evidence.
    EvidenceAuthority,
    /// Explicit owner acceptance.
    Acceptance,
    /// Routine generated output.
    Derived,
    /// Deliberately ignored support/cache file within a fixed root.
    Ignored,
}

/// Native parser selected by a family rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeParser {
    Json,
    Yaml,
    JsonLines,
    MarkdownHeader,
    CommentHeader,
    Opaque,
}

/// Closed family claim policy. It classifies conventions, never member IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimPolicy {
    DesignSources,
    ContractTree,
    EvidenceRules,
    ProtoTooling,
    RustGenerated,
    AdapterContractViews,
    AdapterRpcBindings,
}

/// How an owning family derives output paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputConvention {
    None,
    ClosedGeneratedRoot,
    NativeSemanticName,
    MixedTransition,
}

/// Independent validators required by a family boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidatorKind {
    StableBytes,
    StrictHeader,
    NativeSyntax,
    ConsumerLoad,
}

/// Fixed per-family discovery bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FamilyBudget {
    pub max_depth: usize,
    pub max_file_bytes: usize,
}

const SOURCE_BUDGET: FamilyBudget = FamilyBudget {
    max_depth: 32,
    max_file_bytes: 16 * 1024 * 1024,
};
const BINARY_BUDGET: FamilyBudget = FamilyBudget {
    max_depth: 32,
    max_file_bytes: 64 * 1024 * 1024,
};
const SOURCE_VALIDATORS: &[ValidatorKind] = &[
    ValidatorKind::StableBytes,
    ValidatorKind::StrictHeader,
    ValidatorKind::NativeSyntax,
];
const GENERATED_VALIDATORS: &[ValidatorKind] =
    &[ValidatorKind::StableBytes, ValidatorKind::ConsumerLoad];

/// Closed, member-independent family declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FamilyRule {
    /// Stable family ID.
    pub family_id: &'static str,
    /// Closed repository root.
    pub root: &'static str,
    /// Complete accepted suffix set for this root.
    pub suffixes: &'static [&'static str],
    /// Native header parser.
    pub parser: NativeParser,
    /// Convention-only role classifier.
    pub claim_policy: ClaimPolicy,
    /// Ordinary output naming policy.
    pub output_convention: OutputConvention,
    /// Family-local resource bounds.
    pub budget: FamilyBudget,
    /// Validators supplied by this family.
    pub validators: &'static [ValidatorKind],
}

/// Fixed family registry. It names representation roots and suffixes, never members.
pub const FAMILY_RULES: &[FamilyRule] = &[
    FamilyRule {
        family_id: "upfront-design",
        root: "docs/upfront_design",
        suffixes: &[".DS_Store", ".md"],
        parser: NativeParser::MarkdownHeader,
        claim_policy: ClaimPolicy::DesignSources,
        output_convention: OutputConvention::None,
        budget: SOURCE_BUDGET,
        validators: SOURCE_VALIDATORS,
    },
    FamilyRule {
        family_id: "contracts",
        root: "contracts",
        suffixes: &[".ebnf", ".json", ".jsonl", ".md", ".proto", ".sql", ".yaml"],
        parser: NativeParser::Opaque,
        claim_policy: ClaimPolicy::ContractTree,
        output_convention: OutputConvention::MixedTransition,
        budget: BINARY_BUDGET,
        validators: SOURCE_VALIDATORS,
    },
    FamilyRule {
        family_id: "ast-grep-rules",
        root: "rules",
        suffixes: &[".md", ".yml"],
        parser: NativeParser::Yaml,
        claim_policy: ClaimPolicy::EvidenceRules,
        output_convention: OutputConvention::None,
        budget: SOURCE_BUDGET,
        validators: SOURCE_VALIDATORS,
    },
    FamilyRule {
        family_id: "ast-grep-rule-tests",
        root: "rule-tests",
        suffixes: &[".md", ".yml"],
        parser: NativeParser::Yaml,
        claim_policy: ClaimPolicy::EvidenceRules,
        output_convention: OutputConvention::None,
        budget: SOURCE_BUDGET,
        validators: SOURCE_VALIDATORS,
    },
    FamilyRule {
        family_id: "proto-tooling",
        root: "tooling/proto",
        suffixes: &[".json", ".md", ".pb", ".py", ".pyc", ".rs"],
        parser: NativeParser::Opaque,
        claim_policy: ClaimPolicy::ProtoTooling,
        output_convention: OutputConvention::NativeSemanticName,
        budget: BINARY_BUDGET,
        validators: GENERATED_VALIDATORS,
    },
    FamilyRule {
        family_id: "rust-generated",
        root: "src/generated",
        suffixes: &[".rs"],
        parser: NativeParser::Opaque,
        claim_policy: ClaimPolicy::RustGenerated,
        output_convention: OutputConvention::ClosedGeneratedRoot,
        budget: SOURCE_BUDGET,
        validators: GENERATED_VALIDATORS,
    },
    FamilyRule {
        family_id: "adapter-contract-views",
        root: "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts",
        suffixes: &[".json", ".py", ".pyc"],
        parser: NativeParser::Opaque,
        claim_policy: ClaimPolicy::AdapterContractViews,
        output_convention: OutputConvention::NativeSemanticName,
        budget: SOURCE_BUDGET,
        validators: GENERATED_VALIDATORS,
    },
    FamilyRule {
        family_id: "adapter-rpc-bindings",
        root: "codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated",
        suffixes: &[".py", ".pyc", ".pyi"],
        parser: NativeParser::Opaque,
        claim_policy: ClaimPolicy::AdapterRpcBindings,
        output_convention: OutputConvention::NativeSemanticName,
        budget: SOURCE_BUDGET,
        validators: GENERATED_VALIDATORS,
    },
];

/// Stable source header retained from a family-native artifact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ArtifactHeader {
    /// Stable artifact ID.
    pub artifact_id: StableId,
    /// Native artifact kind.
    pub artifact_kind: String,
    /// Public artifact version.
    pub version: String,
    /// Compatible suite major.
    pub compatible_suite_major: u64,
    /// Closed status text retained for later owner policy.
    pub status: String,
}

/// One exact current-file claim.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ClaimedPath {
    /// Byte-native repository path.
    pub path: RepositoryPath,
    /// Claiming family.
    pub family_id: String,
    /// Governance role.
    pub role: ArtifactRole,
    /// Selected parser.
    pub parser: NativeParser,
    /// Header for self-identifying source artifacts.
    pub header: Option<ArtifactHeader>,
    /// Exact current-byte BLAKE3 identity.
    pub source_digest: String,
    /// Exact current byte length.
    pub byte_length: u64,
    /// Detached Git classifications.
    pub git_states: BTreeSet<GitPathState>,
}

/// Read-only lineage explanation for one modeled claim or graph node.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelExplanation {
    /// Stable graph node when the claim participates in dependency planning.
    pub node_id: Option<StableId>,
    /// Exact claimed source or output path.
    pub claim: ClaimedPath,
    /// Stable prerequisite closure, including this node when present.
    pub prerequisites: Vec<StableId>,
    /// Stable affected closure, including this node when present.
    pub affected: Vec<StableId>,
}

/// Stable bounded inventory diagnostic.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InventoryDiagnostic {
    /// Stable automation class.
    pub class: &'static str,
    /// Affected path when available.
    pub path: Option<RepositoryPath>,
    /// Bounded explanatory text.
    pub message: String,
}

/// Functional repository model with an ephemeral petgraph index.
#[derive(Debug)]
pub struct RepositoryModel {
    /// Claimed current paths in raw-byte order.
    pub claims: BTreeMap<Vec<u8>, ClaimedPath>,
    /// Self-identifying authority/evidence/acceptance artifacts.
    pub artifacts: BTreeMap<StableId, Vec<u8>>,
    /// Derived output paths.
    pub outputs: BTreeMap<StableId, Vec<u8>>,
    /// Selected worktree topology.
    pub topology: WorktreeTopology,
    /// Detached Git classification census, including absent tracked paths.
    pub classifications: BTreeMap<GitPathState, usize>,
    /// Bounded diagnostics.
    pub diagnostics: Vec<InventoryDiagnostic>,
    graph: ModelGraph,
}

impl RepositoryModel {
    /// Compile a bounded read-only model. The legacy suite manifest is not an input.
    ///
    /// # Errors
    ///
    /// Returns a classified path, filesystem, parser, resource, Git, or graph failure.
    pub fn discover(
        root: &Path,
        bounds: InventoryBounds,
        use_gix: bool,
    ) -> Result<Self, RepositoryModelError> {
        let git = if use_gix {
            super::model_git_state::inventory(root).unwrap_or_else(|_| GitInventory {
                topology: WorktreeTopology::filesystem_only(root),
                states: BTreeMap::new(),
            })
        } else {
            GitInventory {
                topology: WorktreeTopology::filesystem_only(root),
                states: BTreeMap::new(),
            }
        };

        let mut claims = BTreeMap::new();
        let mut total_entries = 0_usize;
        let mut total_bytes = 0_u64;
        let mut case_keys = BTreeMap::<Vec<u8>, Vec<u8>>::new();
        for rule in FAMILY_RULES {
            let absolute_root = root.join(rule.root);
            if !absolute_root.exists() {
                continue;
            }
            walk_family(
                root,
                &absolute_root,
                rule,
                0,
                bounds,
                &git.states,
                &mut total_entries,
                &mut total_bytes,
                &mut case_keys,
                &mut claims,
            )?;
        }

        let mut artifacts = BTreeMap::new();
        let mut outputs = BTreeMap::new();
        let mut nodes = Vec::new();
        for (raw_path, claim) in &claims {
            match claim.role {
                ArtifactRole::Authority
                | ArtifactRole::EvidenceAuthority
                | ArtifactRole::Acceptance => {
                    let header = claim.header.as_ref().ok_or_else(|| {
                        RepositoryModelError::MissingHeader(claim.path.display().to_owned())
                    })?;
                    if artifacts
                        .insert(header.artifact_id.clone(), raw_path.clone())
                        .is_some()
                    {
                        return Err(RepositoryModelError::DuplicateArtifact(
                            header.artifact_id.to_string(),
                        ));
                    }
                    nodes.push(NodeDeclaration {
                        id: header.artifact_id.clone(),
                        kind: match claim.role {
                            ArtifactRole::Authority => NodeKind::Source,
                            ArtifactRole::EvidenceAuthority => NodeKind::Evidence,
                            ArtifactRole::Acceptance => NodeKind::Acceptance,
                            ArtifactRole::Derived | ArtifactRole::Ignored => unreachable!(),
                        },
                    });
                }
                ArtifactRole::Derived => {
                    let output_id = output_id(raw_path)?;
                    outputs.insert(output_id.clone(), raw_path.clone());
                    nodes.push(NodeDeclaration {
                        id: output_id,
                        kind: NodeKind::Output,
                    });
                }
                ArtifactRole::Ignored => {}
            }
        }
        let graph = ModelGraph::compile(
            nodes,
            Vec::new(),
            ResourceBounds::new(
                bounds.max_entries,
                bounds.max_entries.saturating_mul(8).max(1),
                bounds.max_diagnostics,
            )?,
        )?;
        let classifications = if git.topology.git_available {
            governed_classifications(&git.states)
        } else {
            BTreeMap::from([(GitPathState::FilesystemOnly, claims.len())])
        };
        Ok(Self {
            claims,
            artifacts,
            outputs,
            topology: git.topology,
            classifications,
            diagnostics: Vec::new(),
            graph,
        })
    }

    /// Semantic identity that deliberately excludes Git acceleration/classification.
    ///
    /// # Errors
    ///
    /// Returns an encoding error only if the normalized view cannot be serialized.
    pub fn semantic_digest(&self) -> Result<String, RepositoryModelError> {
        #[derive(Serialize)]
        struct SemanticClaim<'a> {
            path: &'a [u8],
            family_id: &'a str,
            role: ArtifactRole,
            header: &'a Option<ArtifactHeader>,
            source_digest: &'a str,
            byte_length: u64,
        }
        let claims: Vec<_> = self
            .claims
            .iter()
            .map(|(path, claim)| SemanticClaim {
                path,
                family_id: &claim.family_id,
                role: claim.role,
                header: &claim.header,
                source_digest: &claim.source_digest,
                byte_length: claim.byte_length,
            })
            .collect();
        canonical_digest(&claims)
    }

    /// Stable serializable view. Petgraph-local indices are intentionally absent.
    ///
    /// # Errors
    ///
    /// Returns an encoding error only if the view cannot be serialized.
    pub fn summary(&self) -> Result<RepositorySummary, RepositoryModelError> {
        Ok(RepositorySummary {
            claim_count: self.claims.len(),
            artifact_count: self.artifacts.len(),
            output_count: self.outputs.len(),
            ignored_count: self
                .claims
                .values()
                .filter(|claim| claim.role == ArtifactRole::Ignored)
                .count(),
            diagnostic_count: self.diagnostics.len(),
            semantic_digest: self.semantic_digest()?,
            topology: self.topology.clone(),
            classifications: self.classifications.clone(),
            execution_order: self
                .graph
                .execution_order()
                .iter()
                .map(ToString::to_string)
                .collect(),
        })
    }

    /// Explain a stable artifact ID or display path.
    #[must_use]
    pub fn explain(&self, target: &str) -> Vec<ModelExplanation> {
        self.claims
            .iter()
            .filter_map(|(path, claim)| {
                let node_id = claim
                    .header
                    .as_ref()
                    .map(|header| header.artifact_id.clone())
                    .or_else(|| {
                        (claim.role == ArtifactRole::Derived)
                            .then(|| output_id(path))
                            .transpose()
                            .ok()
                            .flatten()
                    });
                let matches = claim.path.display() == target
                    || claim
                        .header
                        .as_ref()
                        .is_some_and(|header| header.artifact_id.as_str() == target)
                    || node_id.as_ref().is_some_and(|id| id.as_str() == target);
                matches.then(|| {
                    let graph_node = node_id.filter(|id| self.graph.contains(id));
                    ModelExplanation {
                        prerequisites: graph_node
                            .as_ref()
                            .map_or_else(Vec::new, |id| self.graph.prerequisite_closure(id)),
                        affected: graph_node
                            .as_ref()
                            .map_or_else(Vec::new, |id| self.graph.affected_closure(id)),
                        node_id: graph_node,
                        claim: claim.clone(),
                    }
                })
            })
            .collect()
    }
}

fn governed_classifications(
    states: &BTreeMap<Vec<u8>, BTreeSet<GitPathState>>,
) -> BTreeMap<GitPathState, usize> {
    let roots: Vec<Vec<u8>> = FAMILY_RULES
        .iter()
        .map(|rule| format!("{}/", rule.root).into_bytes())
        .collect();
    let mut classifications = BTreeMap::new();
    for (path, path_states) in states {
        if roots.iter().any(|root| path.starts_with(root)) {
            for state in path_states {
                *classifications.entry(*state).or_insert(0) += 1;
            }
        }
    }
    classifications
}

/// Stable CLI summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RepositorySummary {
    pub claim_count: usize,
    pub artifact_count: usize,
    pub output_count: usize,
    pub ignored_count: usize,
    pub diagnostic_count: usize,
    pub semantic_digest: String,
    pub topology: WorktreeTopology,
    pub classifications: BTreeMap<GitPathState, usize>,
    pub execution_order: Vec<String>,
}

/// Temporary comparison classes for the authored legacy catalog.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShadowClass {
    MatchedArtifact,
    MatchedOutput,
    LegacyTopologyPending,
    MissingCurrentPath,
    ModelOnlyPath,
}

/// Shadow parity report. It is never a model input.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ShadowReport {
    pub classes: BTreeMap<ShadowClass, usize>,
    pub missing_paths: Vec<String>,
    pub mismatches: Vec<ShadowMismatch>,
}

/// One explainable temporary legacy-parity mismatch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ShadowMismatch {
    pub target: String,
    pub class: ShadowClass,
    pub detail: String,
}

impl ShadowReport {
    /// Whether every legacy artifact/output path has current model coverage.
    #[must_use]
    pub fn path_parity(&self) -> bool {
        self.missing_paths.is_empty()
    }
}

/// Compare a completed model to the temporary authored catalog.
///
/// # Errors
///
/// Returns a bounded I/O or typed JSON error. The catalog is read only after model compilation.
pub fn compare_legacy_catalog(
    root: &Path,
    model: &RepositoryModel,
) -> Result<ShadowReport, RepositoryModelError> {
    let catalog_path = root.join("contracts/manifests/suite-manifest.json");
    let bytes = read_stable(&catalog_path, MAX_HEADER_BYTES * 8)?;
    let catalog: LegacyCatalog =
        serde_json::from_slice(&bytes).map_err(|error| RepositoryModelError::HeaderParse {
            path: "contracts/manifests/suite-manifest.json".to_owned(),
            message: bounded(error),
        })?;
    let mut classes = BTreeMap::new();
    let mut expected = BTreeSet::new();
    let mut missing_paths = Vec::new();
    let mut mismatches = Vec::new();
    for artifact in catalog.artifacts {
        let path = artifact.authority_path.into_bytes();
        expected.insert(path.clone());
        let class = match model.claims.get(&path) {
            Some(claim) if claim.role == ArtifactRole::Derived => ShadowClass::MatchedOutput,
            Some(_) => ShadowClass::MatchedArtifact,
            None => {
                let display = String::from_utf8_lossy(&path).into_owned();
                missing_paths.push(display.clone());
                mismatches.push(ShadowMismatch {
                    target: display.clone(),
                    class: ShadowClass::MissingCurrentPath,
                    detail: format!("legacy artifact path is absent from current model: {display}"),
                });
                ShadowClass::MissingCurrentPath
            }
        };
        *classes.entry(class).or_insert(0) += 1;
    }
    for derivation in catalog.derivations {
        mismatches.push(ShadowMismatch {
            target: derivation.derivation_id.clone(),
            class: ShadowClass::LegacyTopologyPending,
            detail: format!(
                "legacy derivation {} remains a parity oracle until its family driver describes typed inputs and outputs",
                derivation.derivation_id
            ),
        });
        *classes
            .entry(ShadowClass::LegacyTopologyPending)
            .or_insert(0) += 1;
        for output in derivation.outputs {
            let path = output.path.into_bytes();
            expected.insert(path.clone());
            let class = if model.claims.contains_key(&path) {
                ShadowClass::MatchedOutput
            } else {
                let display = String::from_utf8_lossy(&path).into_owned();
                missing_paths.push(display.clone());
                mismatches.push(ShadowMismatch {
                    target: display.clone(),
                    class: ShadowClass::MissingCurrentPath,
                    detail: format!("legacy output path is absent from current model: {display}"),
                });
                ShadowClass::MissingCurrentPath
            };
            *classes.entry(class).or_insert(0) += 1;
        }
    }
    for (path, claim) in &model.claims {
        if claim.role != ArtifactRole::Ignored && !expected.contains(path) {
            *classes.entry(ShadowClass::ModelOnlyPath).or_insert(0) += 1;
            mismatches.push(ShadowMismatch {
                target: claim.path.display().to_owned(),
                class: ShadowClass::ModelOnlyPath,
                detail: format!(
                    "current model path is not represented by the temporary legacy catalog: {}",
                    claim.path.display()
                ),
            });
        }
    }
    missing_paths.sort();
    missing_paths.dedup();
    mismatches.sort_by(|left, right| {
        (&left.target, left.class, &left.detail).cmp(&(&right.target, right.class, &right.detail))
    });
    Ok(ShadowReport {
        classes,
        missing_paths,
        mismatches,
    })
}

#[derive(Deserialize)]
struct LegacyCatalog {
    artifacts: Vec<LegacyArtifact>,
    derivations: Vec<LegacyDerivation>,
}

#[derive(Deserialize)]
struct LegacyArtifact {
    authority_path: String,
}

#[derive(Deserialize)]
struct LegacyDerivation {
    derivation_id: String,
    outputs: Vec<LegacyOutput>,
}

#[derive(Deserialize)]
struct LegacyOutput {
    path: String,
}

pub(super) struct GitInventory {
    pub(super) topology: WorktreeTopology,
    pub(super) states: BTreeMap<Vec<u8>, BTreeSet<GitPathState>>,
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the bounded recursive walk keeps one auditable accounting and claiming path"
)]
fn walk_family(
    repository_root: &Path,
    directory: &Path,
    rule: &FamilyRule,
    depth: usize,
    bounds: InventoryBounds,
    git_states: &BTreeMap<Vec<u8>, BTreeSet<GitPathState>>,
    total_entries: &mut usize,
    total_bytes: &mut u64,
    case_keys: &mut BTreeMap<Vec<u8>, Vec<u8>>,
    claims: &mut BTreeMap<Vec<u8>, ClaimedPath>,
) -> Result<(), RepositoryModelError> {
    if depth > bounds.max_depth.min(rule.budget.max_depth) {
        return Err(RepositoryModelError::ResourceLimit("directory-depth"));
    }
    let mut entries = fs::read_dir(directory)
        .map_err(|error| io_error(directory, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error(directory, error))?;
    entries.sort_by(|left, right| {
        left.file_name()
            .as_bytes()
            .cmp(right.file_name().as_bytes())
    });
    for entry in entries {
        *total_entries = total_entries.saturating_add(1);
        if *total_entries > bounds.max_entries {
            return Err(RepositoryModelError::ResourceLimit("entry-count"));
        }
        let absolute = entry.path();
        let metadata =
            fs::symlink_metadata(&absolute).map_err(|error| io_error(&absolute, error))?;
        if metadata.file_type().is_symlink() {
            return Err(RepositoryModelError::Symlink(
                absolute.to_string_lossy().into_owned(),
            ));
        }
        if metadata.is_dir() {
            walk_family(
                repository_root,
                &absolute,
                rule,
                depth + 1,
                bounds,
                git_states,
                total_entries,
                total_bytes,
                case_keys,
                claims,
            )?;
            continue;
        }
        if !metadata.is_file() {
            return Err(RepositoryModelError::SpecialFile(
                absolute.to_string_lossy().into_owned(),
            ));
        }
        let relative = absolute.strip_prefix(repository_root).map_err(|_| {
            RepositoryModelError::UnsafePath(absolute.to_string_lossy().into_owned())
        })?;
        let path = RepositoryPath::from_relative(relative)?;
        let suffix = matching_suffix(&path.raw_bytes, rule.suffixes)
            .ok_or_else(|| RepositoryModelError::UnclaimedPath(path.display().to_owned()))?;
        let claim_role = role_for(rule.claim_policy, &path.raw_bytes, suffix);
        validate_claim_path(&path, claim_role)?;
        register_case_key(case_keys, &path)?;
        let bytes = read_stable(
            &absolute,
            usize::try_from(bounds.max_total_bytes)
                .unwrap_or(usize::MAX)
                .min(rule.budget.max_file_bytes),
        )?;
        *total_bytes = total_bytes.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        if *total_bytes > bounds.max_total_bytes {
            return Err(RepositoryModelError::ResourceLimit("total-bytes"));
        }
        let parser = parser_for(&path.raw_bytes, rule.parser);
        let header = if matches!(
            claim_role,
            ArtifactRole::Authority | ArtifactRole::EvidenceAuthority | ArtifactRole::Acceptance
        ) {
            Some(parse_header(&path, parser, &bytes, claim_role)?)
        } else {
            None
        };
        let mut states = git_states.get(&path.raw_bytes).cloned().unwrap_or_default();
        if states.is_empty() {
            states.insert(GitPathState::FilesystemOnly);
        }
        claims.insert(
            path.raw_bytes.clone(),
            ClaimedPath {
                path,
                family_id: rule.family_id.to_owned(),
                role: claim_role,
                parser,
                header,
                source_digest: format!("b3:{}", blake3::hash(&bytes).to_hex()),
                byte_length: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                git_states: states,
            },
        );
    }
    Ok(())
}

fn validate_claim_path(
    path: &RepositoryPath,
    role: ArtifactRole,
) -> Result<(), RepositoryModelError> {
    if path.display_is_lossy() && role != ArtifactRole::Ignored {
        Err(RepositoryModelError::NonUtf8Path(path.display().to_owned()))
    } else {
        Ok(())
    }
}

fn register_case_key(
    case_keys: &mut BTreeMap<Vec<u8>, Vec<u8>>,
    path: &RepositoryPath,
) -> Result<(), RepositoryModelError> {
    let case_key = path
        .raw_bytes
        .iter()
        .map(u8::to_ascii_lowercase)
        .collect::<Vec<_>>();
    if let Some(previous) = case_keys.insert(case_key, path.raw_bytes.clone())
        && previous != path.raw_bytes
    {
        return Err(RepositoryModelError::CaseCollision {
            first: String::from_utf8_lossy(&previous).into_owned(),
            second: path.display().to_owned(),
        });
    }
    Ok(())
}

fn matching_suffix<'a>(path: &[u8], suffixes: &'a [&str]) -> Option<&'a str> {
    suffixes
        .iter()
        .copied()
        .filter(|suffix| path.ends_with(suffix.as_bytes()))
        .max_by_key(|suffix| suffix.len())
}

fn role_for(policy: ClaimPolicy, path: &[u8], suffix: &str) -> ArtifactRole {
    if path.ends_with(b".DS_Store")
        || path
            .windows(b"/__pycache__/".len())
            .any(|part| part == b"/__pycache__/")
        || suffix == ".pyc"
        || path.ends_with(b"/README.md")
        || path.ends_with(b"/CHANGELOG.md")
        || path.ends_with(b"/__init__.py")
    {
        return ArtifactRole::Ignored;
    }
    match policy {
        ClaimPolicy::DesignSources => ArtifactRole::Authority,
        ClaimPolicy::EvidenceRules => ArtifactRole::EvidenceAuthority,
        ClaimPolicy::RustGenerated | ClaimPolicy::AdapterRpcBindings => ArtifactRole::Derived,
        ClaimPolicy::ProtoTooling => {
            if path.ends_with(b"/compatibility-baseline.json") {
                ArtifactRole::Acceptance
            } else if path.ends_with(b"/descriptor-census.json")
                || path.ends_with(b"/production-descriptor.pb")
                || path.ends_with(b"/toolchain-identity.json")
            {
                ArtifactRole::Derived
            } else {
                ArtifactRole::Ignored
            }
        }
        ClaimPolicy::AdapterContractViews => {
            if path.ends_with(b".json")
                || path.ends_with(b"/registries.py")
                || path.ends_with(b"/wire_models.py")
            {
                ArtifactRole::Derived
            } else {
                ArtifactRole::Ignored
            }
        }
        ClaimPolicy::ContractTree => contract_role(path),
    }
}

fn contract_role(path: &[u8]) -> ArtifactRole {
    if path.starts_with(b"contracts/fixtures/") {
        ArtifactRole::EvidenceAuthority
    } else if path.starts_with(b"contracts/generated/")
        || path.starts_with(b"contracts/bundles/")
        || path.starts_with(b"contracts/toolchain/")
        || path.starts_with(b"contracts/adapter/") && path.ends_with(b".schema.json")
        || path.starts_with(b"contracts/query/") && path.ends_with(b".schema.json")
        || path.starts_with(b"contracts/schema/") && !path.ends_with(b"schema-contract-ir.json")
        || path.starts_with(b"contracts/manifests/")
            && !path.ends_with(b"deployment-profile.schema.json")
    {
        ArtifactRole::Derived
    } else {
        ArtifactRole::Authority
    }
}

fn parser_for(path: &[u8], fallback: NativeParser) -> NativeParser {
    if path.ends_with(b".json") {
        NativeParser::Json
    } else if path.ends_with(b".yaml") || path.ends_with(b".yml") {
        NativeParser::Yaml
    } else if path.ends_with(b".jsonl") {
        NativeParser::JsonLines
    } else if path.ends_with(b".md") {
        NativeParser::MarkdownHeader
    } else if path.ends_with(b".ebnf") || path.ends_with(b".proto") {
        NativeParser::CommentHeader
    } else {
        fallback
    }
}

fn parse_header(
    path: &RepositoryPath,
    parser: NativeParser,
    bytes: &[u8],
    role: ArtifactRole,
) -> Result<ArtifactHeader, RepositoryModelError> {
    if role == ArtifactRole::EvidenceAuthority
        && (path.raw_bytes.starts_with(b"contracts/fixtures/")
            || path.raw_bytes.starts_with(b"rules/")
            || path.raw_bytes.starts_with(b"rule-tests/"))
    {
        return synthetic_evidence_header(path);
    }
    if role == ArtifactRole::Acceptance && path.raw_bytes.ends_with(b"/compatibility-baseline.json")
    {
        return synthetic_acceptance_header(path);
    }
    let fields = match parser {
        NativeParser::Json => json_header(path, bytes)?,
        NativeParser::Yaml => yaml_header(path, bytes)?,
        NativeParser::JsonLines => jsonl_header(path, bytes)?,
        NativeParser::MarkdownHeader | NativeParser::CommentHeader => text_header(path, bytes)?,
        NativeParser::Opaque => {
            return Err(RepositoryModelError::MissingHeader(
                path.display().to_owned(),
            ));
        }
    };
    if !fields.contains_key("artifact_id") && path.raw_bytes.ends_with(b".schema.json") {
        return schema_header_from_id(path, &fields);
    }
    header_from_fields(path, &fields)
}

fn synthetic_acceptance_header(
    path: &RepositoryPath,
) -> Result<ArtifactHeader, RepositoryModelError> {
    let slug = path
        .display()
        .trim_end_matches(".json")
        .replace(['/', '_'], "-");
    Ok(ArtifactHeader {
        artifact_id: StableId::parse(format!("acceptance:{slug}"))?,
        artifact_kind: "compatibility-acceptance".to_owned(),
        version: "1.0".to_owned(),
        compatible_suite_major: 1,
        status: "accepted".to_owned(),
    })
}

fn schema_header_from_id(
    path: &RepositoryPath,
    fields: &BTreeMap<String, serde_json::Value>,
) -> Result<ArtifactHeader, RepositoryModelError> {
    let schema_id = fields
        .get("$id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| RepositoryModelError::HeaderField {
            path: path.display().to_owned(),
            field: "$id".to_owned(),
        })?;
    let relative = schema_id
        .strip_prefix("https://codefabric.dev/contracts/")
        .ok_or_else(|| RepositoryModelError::HeaderField {
            path: path.display().to_owned(),
            field: "$id".to_owned(),
        })?
        .trim_end_matches(".json")
        .replace('/', ".");
    Ok(ArtifactHeader {
        artifact_id: StableId::parse(format!("codefabric.{relative}"))?,
        artifact_kind: "json-schema".to_owned(),
        version: "1.0".to_owned(),
        compatible_suite_major: 1,
        status: "released".to_owned(),
    })
}

fn synthetic_evidence_header(
    path: &RepositoryPath,
) -> Result<ArtifactHeader, RepositoryModelError> {
    let slug = path
        .display()
        .trim_end_matches(".json")
        .trim_end_matches(".yml")
        .trim_end_matches(".md")
        .replace(['/', '_'], "-");
    Ok(ArtifactHeader {
        artifact_id: StableId::parse(format!("evidence:{slug}"))?,
        artifact_kind: "evidence".to_owned(),
        version: "1.0".to_owned(),
        compatible_suite_major: 1,
        status: "accepted".to_owned(),
    })
}

fn json_header(
    path: &RepositoryPath,
    bytes: &[u8],
) -> Result<BTreeMap<String, serde_json::Value>, RepositoryModelError> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|error| RepositoryModelError::HeaderParse {
            path: path.display().to_owned(),
            message: bounded(error),
        })?;
    value.as_object().map_or_else(
        || {
            Err(RepositoryModelError::MissingHeader(
                path.display().to_owned(),
            ))
        },
        |object| {
            Ok(object
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect())
        },
    )
}

fn yaml_header(
    path: &RepositoryPath,
    bytes: &[u8],
) -> Result<BTreeMap<String, serde_json::Value>, RepositoryModelError> {
    let value: serde_yaml_ng::Value =
        serde_yaml_ng::from_slice(bytes).map_err(|error| RepositoryModelError::HeaderParse {
            path: path.display().to_owned(),
            message: bounded(error),
        })?;
    let json = serde_json::to_value(value).map_err(RepositoryModelError::Json)?;
    json.as_object().map_or_else(
        || {
            Err(RepositoryModelError::MissingHeader(
                path.display().to_owned(),
            ))
        },
        |object| {
            Ok(object
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect())
        },
    )
}

fn jsonl_header(
    path: &RepositoryPath,
    bytes: &[u8],
) -> Result<BTreeMap<String, serde_json::Value>, RepositoryModelError> {
    let line = bytes
        .split(|byte| *byte == b'\n')
        .find(|line| !line.is_empty())
        .ok_or_else(|| RepositoryModelError::MissingHeader(path.display().to_owned()))?;
    json_header(path, line)
}

fn text_header(
    path: &RepositoryPath,
    bytes: &[u8],
) -> Result<BTreeMap<String, serde_json::Value>, RepositoryModelError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| RepositoryModelError::NonUtf8Content(path.display().to_owned()))?;
    let mut fields = BTreeMap::new();
    for line in text.lines().take(24) {
        let normalized = line
            .trim()
            .trim_start_matches("//")
            .trim_start_matches("(*")
            .trim_end_matches("*)")
            .trim()
            .trim_matches('*')
            .trim();
        let Some((key, value)) = normalized.split_once(':') else {
            continue;
        };
        let key = key
            .trim()
            .trim_matches('*')
            .trim()
            .to_ascii_lowercase()
            .replace([' ', '-'], "_");
        let value = value
            .trim()
            .trim_start_matches('*')
            .trim()
            .trim_matches('`');
        match key.as_str() {
            "artifact_id" | "artifact_kind" | "version" | "status" => {
                fields.insert(key, serde_json::Value::String(value.to_owned()));
            }
            "specification_version" | "roadmap_version" => {
                fields.insert(
                    "version".to_owned(),
                    serde_json::Value::String(value.to_owned()),
                );
            }
            "compatible_suite_major" => {
                if let Ok(number) = value.parse::<u64>() {
                    fields.insert(key, serde_json::Value::from(number));
                }
            }
            _ => {}
        }
    }
    if path.raw_bytes.starts_with(b"docs/upfront_design/") {
        fields
            .entry("compatible_suite_major".to_owned())
            .or_insert_with(|| serde_json::Value::from(1_u64));
    }
    if fields.is_empty() {
        Err(RepositoryModelError::MissingHeader(
            path.display().to_owned(),
        ))
    } else {
        Ok(fields)
    }
}

fn header_from_fields(
    path: &RepositoryPath,
    fields: &BTreeMap<String, serde_json::Value>,
) -> Result<ArtifactHeader, RepositoryModelError> {
    let string = |key: &str| {
        fields
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| RepositoryModelError::HeaderField {
                path: path.display().to_owned(),
                field: key.to_owned(),
            })
    };
    let artifact_id = StableId::parse(string("artifact_id")?)?;
    let artifact_kind = normalize_kind(&string("artifact_kind")?);
    let version = string("version")?;
    let compatible_suite_major = fields
        .get("compatible_suite_major")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| RepositoryModelError::HeaderField {
            path: path.display().to_owned(),
            field: "compatible_suite_major".to_owned(),
        })?;
    let status = normalize_kind(&string("status")?);
    Ok(ArtifactHeader {
        artifact_id,
        artifact_kind,
        version,
        compatible_suite_major,
        status,
    })
}

fn normalize_kind(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '_'], "-")
        .replace("normative-suite-manifest", "normative-document")
        .replace("released-normative-specification", "released")
}

fn output_id(path: &[u8]) -> Result<StableId, RepositoryModelError> {
    StableId::parse(format!("output:{}", blake3::hash(path).to_hex())).map_err(Into::into)
}

fn read_stable(path: &Path, maximum: usize) -> Result<Vec<u8>, RepositoryModelError> {
    let before = fs::metadata(path).map_err(|error| io_error(path, error))?;
    if before.len() > u64::try_from(maximum).unwrap_or(u64::MAX) {
        return Err(RepositoryModelError::ResourceLimit("file-bytes"));
    }
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| io_error(path, error))?;
    let file = fs::File::from(descriptor);
    let opened = file.metadata().map_err(|error| io_error(path, error))?;
    if !opened.is_file() || !same_file_version(&before, &opened) {
        return Err(RepositoryModelError::SourceChanged(
            path.to_string_lossy().into_owned(),
        ));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
    file.take(u64::try_from(maximum).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| io_error(path, error))?;
    if bytes.len() > maximum {
        return Err(RepositoryModelError::ResourceLimit("file-bytes"));
    }
    let after = fs::metadata(path).map_err(|error| io_error(path, error))?;
    if !same_file_version(&opened, &after) {
        return Err(RepositoryModelError::SourceChanged(
            path.to_string_lossy().into_owned(),
        ));
    }
    Ok(bytes)
}

fn same_file_version(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
}

fn canonical_digest(value: &impl Serialize) -> Result<String, RepositoryModelError> {
    let value = serde_json::to_value(value).map_err(RepositoryModelError::Json)?;
    let bytes = serde_json_canonicalizer::to_vec(&value).map_err(RepositoryModelError::Json)?;
    Ok(format!("b3:{}", blake3::hash(&bytes).to_hex()))
}

fn bounded(message: impl std::fmt::Display) -> String {
    let mut message = message.to_string();
    if message.len() > MAX_DIAGNOSTIC_BYTES {
        message.truncate(MAX_DIAGNOSTIC_BYTES);
    }
    message
}

fn io_error(path: &Path, error: impl std::fmt::Display) -> RepositoryModelError {
    RepositoryModelError::Io {
        path: path.to_string_lossy().into_owned(),
        message: bounded(error),
    }
}

/// Stable repository-model failure classes.
#[derive(Debug, Error)]
pub enum RepositoryModelError {
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error(transparent)]
    Json(serde_json::Error),
    #[error("unsafe repository path: {0}")]
    UnsafePath(String),
    #[error("unclaimed path in governed root: {0}")]
    UnclaimedPath(String),
    #[error("symlink in governed root: {0}")]
    Symlink(String),
    #[error("special file in governed root: {0}")]
    SpecialFile(String),
    #[error("case-colliding governed paths: {first} and {second}")]
    CaseCollision { first: String, second: String },
    #[error("non-UTF-8 governed path requires explicit family policy: {0}")]
    NonUtf8Path(String),
    #[error("non-UTF-8 header content: {0}")]
    NonUtf8Content(String),
    #[error("missing artifact header: {0}")]
    MissingHeader(String),
    #[error("missing header field {field} in {path}")]
    HeaderField { path: String, field: String },
    #[error("invalid header in {path}: {message}")]
    HeaderParse { path: String, message: String },
    #[error("duplicate artifact ID: {0}")]
    DuplicateArtifact(String),
    #[error("inventory exceeded {0}")]
    ResourceLimit(&'static str),
    #[error("source changed during stable read: {0}")]
    SourceChanged(String),
    #[error("repository I/O failed at {path}: {message}")]
    Io { path: String, message: String },
    #[error("gix repository classification is unavailable")]
    GitUnavailable,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "codefabric-model-{name}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("contracts/identity")).unwrap();
        root
    }

    fn authority_yaml(id: &str) -> String {
        format!(
            "artifact_id: {id}\nartifact_kind: yaml-contract\nversion: \"1.0\"\ncompatible_suite_major: 1\nstatus: released\n"
        )
    }

    #[test]
    fn model_gix_failure_falls_back_without_semantic_drift() {
        let root = temp_root("fallback");
        fs::write(
            root.join("contracts/identity/example.yaml"),
            authority_yaml("codefabric.identity.example"),
        )
        .unwrap();
        let gix_requested =
            RepositoryModel::discover(&root, InventoryBounds::default(), true).unwrap();
        let fallback = RepositoryModel::discover(&root, InventoryBounds::default(), false).unwrap();
        assert_eq!(
            gix_requested.semantic_digest().unwrap(),
            fallback.semantic_digest().unwrap()
        );
        assert!(!gix_requested.topology.git_available);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn model_inventory_rejects_symlink_escape_case_collision_and_unclaimed_paths() {
        let root = temp_root("negative");
        fs::write(
            root.join("contracts/identity/Example.yaml"),
            authority_yaml("codefabric.identity.example-a"),
        )
        .unwrap();
        let mut case_keys = BTreeMap::new();
        register_case_key(
            &mut case_keys,
            &RepositoryPath::from_relative(Path::new("contracts/identity/Example.yaml")).unwrap(),
        )
        .unwrap();
        let error = register_case_key(
            &mut case_keys,
            &RepositoryPath::from_relative(Path::new("contracts/identity/example.yaml")).unwrap(),
        )
        .unwrap_err();
        assert!(
            matches!(error, RepositoryModelError::CaseCollision { .. }),
            "unexpected error: {error:?}"
        );
        fs::remove_file(root.join("contracts/identity/Example.yaml")).unwrap();
        fs::write(root.join("contracts/identity/unclaimed.txt"), b"x").unwrap();
        assert!(matches!(
            RepositoryModel::discover(&root, InventoryBounds::default(), false),
            Err(RepositoryModelError::UnclaimedPath(_))
        ));
        fs::remove_file(root.join("contracts/identity/unclaimed.txt")).unwrap();
        std::os::unix::fs::symlink("/", root.join("contracts/identity/escape.yaml")).unwrap();
        assert!(matches!(
            RepositoryModel::discover(&root, InventoryBounds::default(), false),
            Err(RepositoryModelError::Symlink(_))
        ));
        fs::remove_file(root.join("contracts/identity/escape.yaml")).unwrap();
        let non_utf8 = PathBuf::from(OsString::from_vec(
            b"contracts/identity/non-\xff.yaml".to_vec(),
        ));
        let non_utf8 = RepositoryPath::from_relative(&non_utf8).unwrap();
        assert!(matches!(
            validate_claim_path(&non_utf8, ArtifactRole::Authority),
            Err(RepositoryModelError::NonUtf8Path(_))
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn model_graph_indices_never_serialize() {
        let root = temp_root("serialization");
        fs::write(
            root.join("contracts/identity/example.yaml"),
            authority_yaml("codefabric.identity.example"),
        )
        .unwrap();
        let model = RepositoryModel::discover(&root, InventoryBounds::default(), false).unwrap();
        let serialized = serde_json::to_string(&model.summary().unwrap()).unwrap();
        assert!(!serialized.contains("NodeIndex"));
        assert!(!serialized.contains("node_index"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn model_inventory_diagnostics_stay_within_budgets() {
        let root = temp_root("budget");
        for index in 0..4 {
            fs::write(
                root.join(format!("contracts/identity/{index}.yaml")),
                authority_yaml(&format!("codefabric.identity.{index}")),
            )
            .unwrap();
        }
        assert!(matches!(
            RepositoryModel::discover(
                &root,
                InventoryBounds {
                    max_entries: 2,
                    ..InventoryBounds::default()
                },
                false,
            ),
            Err(RepositoryModelError::ResourceLimit("entry-count"))
        ));
        assert!(bounded("x".repeat(2048)).len() <= MAX_DIAGNOSTIC_BYTES);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn model_family_rules_keep_handwritten_support_out_of_the_derived_write_set() {
        assert_eq!(
            role_for(
                ClaimPolicy::ProtoTooling,
                b"tooling/proto/generate.py",
                ".py"
            ),
            ArtifactRole::Ignored
        );
        assert_eq!(
            role_for(
                ClaimPolicy::ProtoTooling,
                b"tooling/proto/production-descriptor.pb",
                ".pb"
            ),
            ArtifactRole::Derived
        );
        assert_eq!(
            role_for(
                ClaimPolicy::AdapterContractViews,
                b"codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/identity.py",
                ".py"
            ),
            ArtifactRole::Ignored
        );
        assert_eq!(
            role_for(
                ClaimPolicy::AdapterContractViews,
                b"codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/wire_models.py",
                ".py"
            ),
            ArtifactRole::Derived
        );
        assert!(FAMILY_RULES.iter().all(|rule| {
            !rule.validators.is_empty()
                && rule.budget.max_depth > 0
                && rule.budget.max_file_bytes > 0
        }));
    }
}
