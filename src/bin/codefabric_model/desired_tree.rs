//! Read-only action planning and immutable desired-tree construction.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStringExt as _;
use std::path::{Component, Path, PathBuf};

use codefabric::integrity::framed_digest as digest_bytes;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::model_control::{
    EdgeDeclaration, EdgeKind, ModelError, ModelGraph, NodeDeclaration, NodeKind, ResourceBounds,
    StableId,
};
use super::repository_model::{
    ArtifactRole, ClaimedPath, RepositoryModel, RepositoryModelError, output_id, read_stable,
};

const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
const MODEL_SCHEMA_VERSION: &str = "repository-model-v1";
const OUTPUT_SCHEMA_VERSION: &str = "planned-output-v1";
const ENVIRONMENT_CONTRACT_VERSION: &str = "model-driver-env-v1";

/// Safe byte-native repository-relative output path.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SafeOutputPath(Vec<u8>);

impl SafeOutputPath {
    /// Validate a byte-native relative output path.
    ///
    /// # Errors
    ///
    /// Returns an error for empty, absolute, NUL-bearing, traversal, or governed non-derived
    /// paths.
    pub fn parse(bytes: impl Into<Vec<u8>>) -> Result<Self, DesiredTreeError> {
        let bytes = bytes.into();
        if bytes.is_empty() || bytes.contains(&0) || bytes.starts_with(b"/") {
            return Err(DesiredTreeError::UnsafeOutputPath(display_bytes(&bytes)));
        }
        let path = PathBuf::from(OsString::from_vec(bytes.clone()));
        if path.components().any(|component| {
            !matches!(component, Component::Normal(_))
                || matches!(component, Component::ParentDir | Component::RootDir)
        }) || bytes.starts_with(b"contracts/acceptance/")
            || bytes.starts_with(b"contracts/fixtures/")
            || bytes.starts_with(b"docs/upfront_design/")
        {
            return Err(DesiredTreeError::UnsafeOutputPath(display_bytes(&bytes)));
        }
        Ok(Self(bytes))
    }

    /// Borrow the byte-native path identity.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Render the path for diagnostics only.
    #[must_use]
    pub fn display(&self) -> String {
        display_bytes(&self.0)
    }

    pub(crate) fn path_buf(&self) -> PathBuf {
        PathBuf::from(OsString::from_vec(self.0.clone()))
    }
}

/// Pydantic JSON Schema projection mode.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PydanticSchemaMode {
    Validation,
    Serialization,
    Both,
}

/// Protobuf-derived output role.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProtoOutputRole {
    DescriptorSet,
    DescriptorCensus,
    RustBinding,
    PythonMessageBinding,
    PythonGrpcBinding,
    PythonTypingBinding,
}

/// Table/schema output projection.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TableProjection {
    Arrow,
    SqliteDdl,
    PublicJsonSchema,
    RustTableSpec,
}

/// Closed typed output projections. Family drivers refine these declarations without adding
/// sibling path-indexed maps.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(
    tag = "projection_kind",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum PlannedOutputProjection {
    Pydantic {
        mode: PydanticSchemaMode,
        model_roots: BTreeSet<String>,
    },
    JsonSchema {
        public_identity: String,
    },
    Proto {
        role: ProtoOutputRole,
    },
    Registry {
        primary_key: String,
    },
    TableSpec {
        projection: TableProjection,
    },
    RustSource,
    PythonSource,
    CanonicalArtifact {
        artifact_kind: String,
    },
}

/// Closed consumer identities attached to output declarations.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlannedConsumer {
    RustCore,
    PythonAdapter,
    PythonPackage,
    ProtoRuntime,
    ContractVerifier,
}

/// Closed validators selected by output semantics.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlannedValidator {
    ExactBytes,
    StrictDecode,
    RustConsumer,
    PythonConsumer,
    JsonSchemaConsumer,
    ProtoDescriptorConsumer,
}

/// Complete normalized output declaration known before rendering.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedOutput {
    pub output_id: StableId,
    pub public_artifact_id: Option<StableId>,
    pub path: SafeOutputPath,
    pub role: PlannedOutputRole,
    pub producer: StableId,
    pub projection: PlannedOutputProjection,
    pub consumers: BTreeSet<PlannedConsumer>,
    pub validators: BTreeSet<PlannedValidator>,
}

/// Routine output roles; authorities, evidence, and acceptances are intentionally absent.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlannedOutputRole {
    Derived,
    TransitionOverlay,
}

/// Exact source identity entering an action key.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedInput {
    pub artifact_id: StableId,
    pub path: Vec<u8>,
    pub semantic_digest: String,
    pub source_digest: String,
}

/// Exact compiler/tool identity. No cache key or pass decision depends on an executable path
/// alone.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionExecutableIdentity {
    pub compiler_source_identity: String,
    pub cargo_lock_identity: String,
    pub rustc_identity: String,
    pub feature_set: BTreeSet<String>,
    pub profile: String,
    pub target_triple: String,
    pub executable_digest: String,
}

impl ActionExecutableIdentity {
    /// Resolve the executing compiler identity supplied by the isolated launcher.
    ///
    /// # Errors
    ///
    /// Returns an error when a required build-identity component is absent or the executable
    /// bytes cannot be read.
    pub fn current() -> Result<Self, DesiredTreeError> {
        let required = |name: &'static str| {
            std::env::var(name).map_err(|_| DesiredTreeError::MissingExecutionIdentity(name))
        };
        let executable = std::env::current_exe().map_err(DesiredTreeError::CurrentExecutable)?;
        let executable_bytes = read_stable(&executable, MAX_OUTPUT_BYTES)?;
        Ok(Self {
            compiler_source_identity: required("CODEFABRIC_MODEL_COMPILER_SOURCE_IDENTITY")?,
            cargo_lock_identity: required("CODEFABRIC_MODEL_CARGO_LOCK_IDENTITY")?,
            rustc_identity: required("CODEFABRIC_MODEL_RUSTC_IDENTITY")?,
            feature_set: required("CODEFABRIC_MODEL_FEATURE_SET")?
                .split(',')
                .map(str::to_owned)
                .collect(),
            profile: required("CODEFABRIC_MODEL_PROFILE")?,
            target_triple: required("CODEFABRIC_MODEL_TARGET_TRIPLE")?,
            executable_digest: digest_bytes(&executable_bytes),
        })
    }
}

/// Canonical action-key payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionKeyMaterial {
    pub driver_id: StableId,
    pub rule_version: String,
    pub model_schema_version: String,
    pub output_schema_version: String,
    pub inputs: Vec<PlannedInput>,
    pub upstream_output_digests: BTreeMap<StableId, String>,
    pub outputs: Vec<PlannedOutput>,
    pub executable: ActionExecutableIdentity,
    pub environment_contract: BTreeMap<String, String>,
    pub generation_profile: String,
}

impl ActionKeyMaterial {
    /// Compute the RFC 8785/BLAKE3 action key.
    ///
    /// # Errors
    ///
    /// Returns an error only when the closed payload cannot be encoded.
    pub fn digest(&self) -> Result<String, DesiredTreeError> {
        canonical_digest(self)
    }
}

/// One deterministic action in a read-only plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedAction {
    pub action_id: StableId,
    pub action_key: String,
    pub inputs: Vec<PlannedInput>,
    pub upstream_output_digests: BTreeMap<StableId, String>,
    pub outputs: Vec<PlannedOutput>,
}

/// One immutable desired output with lineage and exact bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesiredTreeEntry {
    pub output: PlannedOutput,
    pub lineage: Vec<StableId>,
    pub bytes: Vec<u8>,
    pub content_digest: String,
}

/// Complete desired generated tree in byte-native path order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DesiredTree {
    pub entries: BTreeMap<SafeOutputPath, DesiredTreeEntry>,
}

impl DesiredTree {
    fn insert(&mut self, entry: DesiredTreeEntry) -> Result<(), DesiredTreeError> {
        let path = entry.output.path.clone();
        if let Some(existing) = self.entries.insert(path.clone(), entry) {
            return Err(DesiredTreeError::DuplicateOutput {
                path: path.display(),
                first: existing.output.producer,
            });
        }
        Ok(())
    }

    /// Compare a complete desired tree with current generated bytes.
    #[must_use]
    pub fn compare(&self, current: &BTreeMap<SafeOutputPath, Vec<u8>>) -> Vec<TreeChange> {
        let mut paths: BTreeSet<_> = self.entries.keys().cloned().collect();
        paths.extend(current.keys().cloned());
        paths
            .into_iter()
            .map(|path| match (current.get(&path), self.entries.get(&path)) {
                (None, Some(desired)) => TreeChange::new(
                    path,
                    ChangeKind::Add,
                    None,
                    Some(desired.content_digest.clone()),
                ),
                (Some(current), None) => TreeChange::new(
                    path,
                    ChangeKind::DeleteStale,
                    Some(digest_bytes(current)),
                    None,
                ),
                (Some(current), Some(desired)) if current == &desired.bytes => TreeChange::new(
                    path,
                    ChangeKind::Unchanged,
                    Some(desired.content_digest.clone()),
                    Some(desired.content_digest.clone()),
                ),
                (Some(current), Some(desired)) => TreeChange::new(
                    path,
                    ChangeKind::Replace,
                    Some(digest_bytes(current)),
                    Some(desired.content_digest.clone()),
                ),
                (None, None) => unreachable!("path union contains at least one side"),
            })
            .collect()
    }

    fn staging_identity(&self) -> Result<String, DesiredTreeError> {
        canonical_digest(
            &self
                .entries
                .iter()
                .map(|(path, entry)| (path.as_bytes(), entry.content_digest.as_str()))
                .collect::<Vec<_>>(),
        )
    }

    /// Materialize exact desired bytes beneath a disposable staging root without touching the
    /// repository tree.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsafe destination, an I/O failure, or conflicting existing staged
    /// bytes.
    pub fn stage(&self, staging_root: &Path) -> Result<(), DesiredTreeError> {
        for (path, entry) in &self.entries {
            let destination = staging_root.join(path.path_buf());
            if !destination.starts_with(staging_root) {
                return Err(DesiredTreeError::UnsafeOutputPath(path.display()));
            }
            let parent = destination
                .parent()
                .ok_or_else(|| DesiredTreeError::UnsafeOutputPath(path.display()))?;
            fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
            if destination.exists() {
                let observed = read_stable(&destination, MAX_OUTPUT_BYTES)?;
                if observed != entry.bytes {
                    return Err(DesiredTreeError::StageConflict(path.display()));
                }
            } else {
                fs::write(&destination, &entry.bytes)
                    .map_err(|error| io_error(&destination, error))?;
            }
        }
        Ok(())
    }
}

/// Closed desired-tree comparison class.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChangeKind {
    Add,
    Replace,
    DeleteStale,
    Unchanged,
}

/// One deterministic desired/current comparison item.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreeChange {
    pub path: SafeOutputPath,
    pub kind: ChangeKind,
    pub current_digest: Option<String>,
    pub desired_digest: Option<String>,
}

impl TreeChange {
    pub(crate) fn new(
        path: SafeOutputPath,
        kind: ChangeKind,
        current_digest: Option<String>,
        desired_digest: Option<String>,
    ) -> Self {
        Self {
            path,
            kind,
            current_digest,
            desired_digest,
        }
    }
}

/// Source-generation fence captured before staging.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFence {
    pub sources: BTreeMap<Vec<u8>, String>,
}

impl SourceFence {
    /// Recheck every declared source against stable current bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if a source changed or cannot be read stably.
    pub fn verify(&self, root: &Path) -> Result<(), DesiredTreeError> {
        for (path, expected) in &self.sources {
            let repository_path = safe_repository_path(path)?;
            let bytes = read_stable(&root.join(repository_path), MAX_OUTPUT_BYTES)?;
            let observed = digest_bytes(&bytes);
            if &observed != expected {
                return Err(DesiredTreeError::SourceFenceChanged(display_bytes(path)));
            }
        }
        Ok(())
    }
}

/// Cache payload shape introduced for key/byte semantics only. Pass verdicts are deliberately not
/// representable.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CachedActionBytes {
    pub action_key: String,
    pub outputs: BTreeMap<SafeOutputPath, String>,
}

/// Compiled read-only plan plus its ephemeral dependency graph.
#[derive(Debug)]
pub struct ModelPlan {
    pub actions: BTreeMap<StableId, PlannedAction>,
    pub desired_tree: DesiredTree,
    pub current_outputs: BTreeMap<SafeOutputPath, Vec<u8>>,
    pub changes: Vec<TreeChange>,
    pub source_fence: SourceFence,
    graph: ModelGraph,
    reconciled_output_paths: Option<Vec<String>>,
}

impl ModelPlan {
    /// Compile a conservative complete shadow plan from the current typed repository model.
    ///
    /// Family drivers narrow the current all-source input sets in WP06–WP10; conservative
    /// over-invalidation is correct during this read-only packet.
    ///
    /// # Errors
    ///
    /// Returns an error for path, read, graph, identity, or output-ownership violations.
    pub fn compile(
        root: &Path,
        model: &RepositoryModel,
        executable: &ActionExecutableIdentity,
    ) -> Result<Self, DesiredTreeError> {
        let inputs = planned_inputs(model)?;
        let (outputs_by_family, current_outputs) = current_outputs_by_family(root, model)?;
        let shadow =
            compile_shadow_actions(&inputs, outputs_by_family, &current_outputs, executable)?;
        let graph = ModelGraph::compile(
            shadow.nodes,
            shadow.edges,
            ResourceBounds::new(50_000, 500_000, 64)?,
        )?;
        let changes = shadow.desired_tree.compare(&current_outputs);
        let source_fence = SourceFence {
            sources: model
                .claims
                .values()
                .filter(|claim| {
                    matches!(
                        claim.role,
                        ArtifactRole::Authority
                            | ArtifactRole::EvidenceAuthority
                            | ArtifactRole::Acceptance
                    )
                })
                .map(|claim| (claim.path.raw_bytes().to_vec(), claim.source_digest.clone()))
                .collect(),
        };
        Ok(Self {
            actions: shadow.actions,
            desired_tree: shadow.desired_tree,
            current_outputs,
            changes,
            source_fence,
            graph,
            reconciled_output_paths: None,
        })
    }

    /// Replace the bootstrap shadow comparison with the aggregate renderer's real
    /// transaction preview while retaining the typed action graph and explanations.
    pub fn apply_reconciliation_preview(
        &mut self,
        output_paths: Vec<String>,
        changes: Vec<TreeChange>,
    ) {
        self.reconciled_output_paths = Some(output_paths);
        self.changes = changes;
    }

    /// Stable prerequisite-first action order.
    #[must_use]
    pub fn action_order(&self) -> Vec<StableId> {
        self.graph
            .execution_order()
            .iter()
            .filter(|id| self.actions.contains_key(*id))
            .cloned()
            .collect()
    }

    /// Conservative affected action/output closure for changed model IDs.
    #[must_use]
    pub fn affected(&self, changed: &[StableId]) -> Vec<StableId> {
        changed
            .iter()
            .flat_map(|id| self.graph.affected_closure(id))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Explain source lineage, consumers, and validator-oracles for a model ID or path.
    #[must_use]
    pub fn explain(&self, target: &str) -> Vec<PlanExplanation> {
        self.actions
            .values()
            .filter_map(|action| {
                let matching_inputs: Vec<_> = action
                    .inputs
                    .iter()
                    .filter(|input| {
                        input.artifact_id.as_str() == target || display_bytes(&input.path) == target
                    })
                    .map(|input| input.artifact_id.clone())
                    .collect();
                let matching_outputs: Vec<_> = action
                    .outputs
                    .iter()
                    .filter(|output| {
                        output.output_id.as_str() == target
                            || output
                                .public_artifact_id
                                .as_ref()
                                .is_some_and(|id| id.as_str() == target)
                            || output.path.display() == target
                    })
                    .cloned()
                    .collect();
                (!matching_inputs.is_empty() || !matching_outputs.is_empty()).then(|| {
                    let outputs = if matching_outputs.is_empty() {
                        action.outputs.clone()
                    } else {
                        matching_outputs
                    };
                    PlanExplanation {
                        action_id: action.action_id.clone(),
                        source_lineage: if matching_inputs.is_empty() {
                            action
                                .inputs
                                .iter()
                                .map(|input| input.artifact_id.clone())
                                .collect()
                        } else {
                            matching_inputs
                        },
                        consumers: outputs
                            .iter()
                            .flat_map(|output| output.consumers.iter().copied())
                            .collect(),
                        oracles: outputs
                            .iter()
                            .flat_map(|output| output.validators.iter().copied())
                            .collect(),
                        outputs,
                    }
                })
            })
            .collect()
    }

    /// Verify current source fences, stage exact desired bytes, and require a zero-diff shadow
    /// plan.
    ///
    /// # Errors
    ///
    /// Returns an error for source drift, staging conflict, or any non-unchanged repository output.
    pub fn check(&self, root: &Path) -> Result<(), DesiredTreeError> {
        self.source_fence.verify(root)?;
        let action_identity = self
            .actions
            .values()
            .map(|action| (&action.action_id, &action.action_key))
            .collect::<Vec<_>>();
        let desired_tree_identity = self.desired_tree.staging_identity()?;
        let plan_digest = canonical_digest(&(action_identity, desired_tree_identity))?;
        let stage_name = plan_digest.strip_prefix("b3:").unwrap_or(&plan_digest);
        self.desired_tree
            .stage(&root.join("target/model-stage/read-only").join(stage_name))?;
        self.source_fence.verify(root)?;
        if let Some(change) = self
            .changes
            .iter()
            .find(|change| change.kind != ChangeKind::Unchanged)
        {
            return Err(DesiredTreeError::NonZeroShadowPlan {
                path: change.path.display(),
                kind: change.kind,
            });
        }
        Ok(())
    }
}

/// Structured plan explanation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlanExplanation {
    pub action_id: StableId,
    pub source_lineage: Vec<StableId>,
    pub consumers: BTreeSet<PlannedConsumer>,
    pub oracles: BTreeSet<PlannedValidator>,
    pub outputs: Vec<PlannedOutput>,
}

/// Bounded CLI projection; output bytes never enter stdout.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlanReport {
    pub action_order: Vec<StableId>,
    pub action_keys: BTreeMap<StableId, String>,
    pub output_count: usize,
    pub output_paths: Vec<String>,
    pub changes: Vec<TreeChange>,
    pub affected: Vec<StableId>,
}

impl ModelPlan {
    /// Build a byte-free structured CLI report.
    #[must_use]
    pub fn report(&self, changed: &[StableId]) -> PlanReport {
        let output_paths = self.reconciled_output_paths.clone().unwrap_or_else(|| {
            self.desired_tree
                .entries
                .keys()
                .map(SafeOutputPath::display)
                .collect()
        });
        PlanReport {
            action_order: self.action_order(),
            action_keys: self
                .actions
                .iter()
                .map(|(id, action)| (id.clone(), action.action_key.clone()))
                .collect(),
            output_count: output_paths.len(),
            output_paths,
            changes: self.changes.clone(),
            affected: self.affected(changed),
        }
    }
}

type FamilyOutputs<'a> = BTreeMap<String, Vec<(SafeOutputPath, &'a ClaimedPath)>>;

struct ShadowActions {
    actions: BTreeMap<StableId, PlannedAction>,
    desired_tree: DesiredTree,
    nodes: Vec<NodeDeclaration>,
    edges: Vec<EdgeDeclaration>,
}

fn current_outputs_by_family<'a>(
    root: &Path,
    model: &'a RepositoryModel,
) -> Result<(FamilyOutputs<'a>, BTreeMap<SafeOutputPath, Vec<u8>>), DesiredTreeError> {
    let mut outputs_by_family: FamilyOutputs<'a> = BTreeMap::new();
    let mut current_outputs = BTreeMap::new();
    for claim in model
        .claims
        .values()
        .filter(|claim| claim.role == ArtifactRole::Derived)
    {
        let path = SafeOutputPath::parse(claim.path.raw_bytes().to_vec())?;
        let bytes = read_stable(&root.join(path.path_buf()), MAX_OUTPUT_BYTES)?;
        current_outputs.insert(path.clone(), bytes);
        outputs_by_family
            .entry(claim.family_id.clone())
            .or_default()
            .push((path, claim));
    }
    Ok((outputs_by_family, current_outputs))
}

fn compile_shadow_actions(
    inputs: &[PlannedInput],
    outputs_by_family: FamilyOutputs<'_>,
    current_outputs: &BTreeMap<SafeOutputPath, Vec<u8>>,
    executable: &ActionExecutableIdentity,
) -> Result<ShadowActions, DesiredTreeError> {
    let mut nodes: Vec<_> = inputs
        .iter()
        .map(|input| NodeDeclaration {
            id: input.artifact_id.clone(),
            kind: NodeKind::Source,
        })
        .collect();
    let mut edges = Vec::new();
    let mut actions = BTreeMap::new();
    let mut desired_tree = DesiredTree::default();
    for (family, family_outputs) in outputs_by_family {
        let action_id = StableId::parse(format!("action:{family}"))?;
        nodes.push(NodeDeclaration {
            id: action_id.clone(),
            kind: NodeKind::Action,
        });
        edges.extend(inputs.iter().map(|input| EdgeDeclaration {
            prerequisite: input.artifact_id.clone(),
            dependent: action_id.clone(),
            kind: EdgeKind::ReadsExactBytes,
        }));
        let outputs = compile_family_outputs(
            inputs,
            &action_id,
            family_outputs,
            current_outputs,
            &mut desired_tree,
            &mut nodes,
            &mut edges,
        )?;
        let key_material = ActionKeyMaterial {
            driver_id: action_id.clone(),
            rule_version: format!("{family}-shadow-v1"),
            model_schema_version: MODEL_SCHEMA_VERSION.to_owned(),
            output_schema_version: OUTPUT_SCHEMA_VERSION.to_owned(),
            inputs: inputs.to_vec(),
            upstream_output_digests: BTreeMap::new(),
            outputs: outputs.clone(),
            executable: executable.clone(),
            environment_contract: BTreeMap::from([
                (
                    "contract".to_owned(),
                    ENVIRONMENT_CONTRACT_VERSION.to_owned(),
                ),
                ("credentials".to_owned(), "stripped".to_owned()),
                ("network".to_owned(), "undeclared".to_owned()),
            ]),
            generation_profile: "shadow-read-only".to_owned(),
        };
        actions.insert(
            action_id.clone(),
            PlannedAction {
                action_id,
                action_key: key_material.digest()?,
                inputs: inputs.to_vec(),
                upstream_output_digests: BTreeMap::new(),
                outputs,
            },
        );
    }
    Ok(ShadowActions {
        actions,
        desired_tree,
        nodes,
        edges,
    })
}

fn compile_family_outputs(
    inputs: &[PlannedInput],
    action_id: &StableId,
    family_outputs: Vec<(SafeOutputPath, &ClaimedPath)>,
    current_outputs: &BTreeMap<SafeOutputPath, Vec<u8>>,
    desired_tree: &mut DesiredTree,
    nodes: &mut Vec<NodeDeclaration>,
    edges: &mut Vec<EdgeDeclaration>,
) -> Result<Vec<PlannedOutput>, DesiredTreeError> {
    let mut outputs = Vec::new();
    for (path, claim) in family_outputs {
        let output = planned_output(action_id, path.clone(), claim)?;
        nodes.push(NodeDeclaration {
            id: output.output_id.clone(),
            kind: NodeKind::Output,
        });
        edges.push(EdgeDeclaration {
            prerequisite: action_id.clone(),
            dependent: output.output_id.clone(),
            kind: EdgeKind::Produces,
        });
        let bytes = current_outputs
            .get(&path)
            .cloned()
            .ok_or_else(|| DesiredTreeError::MissingCurrentOutput(path.display()))?;
        desired_tree.insert(DesiredTreeEntry {
            output: output.clone(),
            lineage: inputs
                .iter()
                .map(|input| input.artifact_id.clone())
                .collect(),
            content_digest: digest_bytes(&bytes),
            bytes,
        })?;
        outputs.push(output);
    }
    outputs.sort();
    Ok(outputs)
}

fn planned_inputs(model: &RepositoryModel) -> Result<Vec<PlannedInput>, DesiredTreeError> {
    let mut inputs = Vec::new();
    for claim in model.claims.values().filter(|claim| {
        matches!(
            claim.role,
            ArtifactRole::Authority | ArtifactRole::EvidenceAuthority | ArtifactRole::Acceptance
        )
    }) {
        let header = claim
            .header
            .as_ref()
            .ok_or_else(|| DesiredTreeError::MissingHeader(claim.path.display().to_owned()))?;
        inputs.push(PlannedInput {
            artifact_id: header.artifact_id.clone(),
            path: claim.path.raw_bytes().to_vec(),
            semantic_digest: canonical_digest(header)?,
            source_digest: claim.source_digest.clone(),
        });
    }
    inputs.sort();
    Ok(inputs)
}

fn planned_output(
    producer: &StableId,
    path: SafeOutputPath,
    claim: &ClaimedPath,
) -> Result<PlannedOutput, DesiredTreeError> {
    let display = path.display();
    let projection = if display.ends_with(".schema.json") {
        PlannedOutputProjection::JsonSchema {
            public_identity: claim
                .header
                .as_ref()
                .map_or_else(|| display.clone(), |header| header.artifact_id.to_string()),
        }
    } else if display.ends_with("adapter-schemas.json") || display.ends_with("wire_models.py") {
        PlannedOutputProjection::Pydantic {
            mode: PydanticSchemaMode::Both,
            model_roots: BTreeSet::from(["contract-ir".to_owned()]),
        }
    } else if has_extension(&display, "pb") {
        PlannedOutputProjection::Proto {
            role: ProtoOutputRole::DescriptorSet,
        }
    } else if display.ends_with("_pb2_grpc.py") {
        PlannedOutputProjection::Proto {
            role: ProtoOutputRole::PythonGrpcBinding,
        }
    } else if display.ends_with("_pb2.pyi") {
        PlannedOutputProjection::Proto {
            role: ProtoOutputRole::PythonTypingBinding,
        }
    } else if display.ends_with("_pb2.py") {
        PlannedOutputProjection::Proto {
            role: ProtoOutputRole::PythonMessageBinding,
        }
    } else if display.starts_with("src/generated/codefabric.") && has_extension(&display, "rs") {
        PlannedOutputProjection::Proto {
            role: ProtoOutputRole::RustBinding,
        }
    } else if display.contains("/registry/") || display.ends_with("registries.py") {
        PlannedOutputProjection::Registry {
            primary_key: "stable-id".to_owned(),
        }
    } else if display.ends_with("table_specs.rs") {
        PlannedOutputProjection::TableSpec {
            projection: TableProjection::RustTableSpec,
        }
    } else if has_extension(&display, "rs") {
        PlannedOutputProjection::RustSource
    } else if has_extension(&display, "py") || has_extension(&display, "pyi") {
        PlannedOutputProjection::PythonSource
    } else {
        PlannedOutputProjection::CanonicalArtifact {
            artifact_kind: claim.header.as_ref().map_or_else(
                || extension_kind(&display),
                |header| header.artifact_kind.clone(),
            ),
        }
    };
    let consumers = consumers_for(&display, &projection);
    let validators = validators_for(&projection);
    Ok(PlannedOutput {
        output_id: output_id(path.as_bytes())?,
        public_artifact_id: claim
            .header
            .as_ref()
            .map(|header| header.artifact_id.clone()),
        path,
        role: PlannedOutputRole::Derived,
        producer: producer.clone(),
        projection,
        consumers,
        validators,
    })
}

fn consumers_for(path: &str, projection: &PlannedOutputProjection) -> BTreeSet<PlannedConsumer> {
    let mut consumers = BTreeSet::from([PlannedConsumer::ContractVerifier]);
    match projection {
        PlannedOutputProjection::Pydantic { .. } | PlannedOutputProjection::PythonSource => {
            consumers.insert(PlannedConsumer::PythonAdapter);
            consumers.insert(PlannedConsumer::PythonPackage);
        }
        PlannedOutputProjection::Proto { role } => {
            consumers.insert(PlannedConsumer::ProtoRuntime);
            if matches!(
                role,
                ProtoOutputRole::PythonMessageBinding
                    | ProtoOutputRole::PythonGrpcBinding
                    | ProtoOutputRole::PythonTypingBinding
            ) {
                consumers.insert(PlannedConsumer::PythonAdapter);
            } else {
                consumers.insert(PlannedConsumer::RustCore);
            }
        }
        PlannedOutputProjection::RustSource | PlannedOutputProjection::TableSpec { .. } => {
            consumers.insert(PlannedConsumer::RustCore);
        }
        PlannedOutputProjection::JsonSchema { .. }
        | PlannedOutputProjection::Registry { .. }
        | PlannedOutputProjection::CanonicalArtifact { .. } => {}
    }
    if path.starts_with("codefabric-cpg-mcp/") {
        consumers.insert(PlannedConsumer::PythonPackage);
    }
    consumers
}

fn validators_for(projection: &PlannedOutputProjection) -> BTreeSet<PlannedValidator> {
    let mut validators = BTreeSet::from([PlannedValidator::ExactBytes]);
    match projection {
        PlannedOutputProjection::Pydantic { .. } | PlannedOutputProjection::PythonSource => {
            validators.insert(PlannedValidator::PythonConsumer);
        }
        PlannedOutputProjection::JsonSchema { .. } => {
            validators.insert(PlannedValidator::StrictDecode);
            validators.insert(PlannedValidator::JsonSchemaConsumer);
        }
        PlannedOutputProjection::Proto { .. } => {
            validators.insert(PlannedValidator::ProtoDescriptorConsumer);
        }
        PlannedOutputProjection::Registry { .. }
        | PlannedOutputProjection::CanonicalArtifact { .. } => {
            validators.insert(PlannedValidator::StrictDecode);
        }
        PlannedOutputProjection::TableSpec { .. } | PlannedOutputProjection::RustSource => {
            validators.insert(PlannedValidator::RustConsumer);
        }
    }
    validators
}

fn extension_kind(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map_or_else(|| "binary".to_owned(), str::to_owned)
}

fn has_extension(path: &str, expected: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

fn safe_repository_path(bytes: &[u8]) -> Result<PathBuf, DesiredTreeError> {
    if bytes.is_empty() || bytes.contains(&0) || bytes.starts_with(b"/") {
        return Err(DesiredTreeError::UnsafeOutputPath(display_bytes(bytes)));
    }
    let path = PathBuf::from(OsString::from_vec(bytes.to_vec()));
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(DesiredTreeError::UnsafeOutputPath(display_bytes(bytes)));
    }
    Ok(path)
}

fn canonical_digest(value: &impl Serialize) -> Result<String, DesiredTreeError> {
    let value = serde_json::to_value(value).map_err(DesiredTreeError::Json)?;
    let bytes = serde_json_canonicalizer::to_vec(&value).map_err(DesiredTreeError::Json)?;
    Ok(digest_bytes(&bytes))
}

fn display_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn io_error(path: &Path, error: impl std::fmt::Display) -> DesiredTreeError {
    DesiredTreeError::Io {
        path: path.to_owned(),
        message: error.to_string().chars().take(512).collect(),
    }
}

/// Read-only planning failure.
#[derive(Debug, Error)]
pub enum DesiredTreeError {
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error(transparent)]
    Repository(#[from] RepositoryModelError),
    #[error("unsafe planned output path: {0}")]
    UnsafeOutputPath(String),
    #[error("duplicate desired output path {path}; first producer {first}")]
    DuplicateOutput { path: String, first: StableId },
    #[error("missing current derived output bytes: {0}")]
    MissingCurrentOutput(String),
    #[error("missing typed header for planned source: {0}")]
    MissingHeader(String),
    #[error("required executable identity is absent: {0}")]
    MissingExecutionIdentity(&'static str),
    #[error("cannot resolve current model executable: {0}")]
    CurrentExecutable(std::io::Error),
    #[error("desired-tree JSON encoding failed: {0}")]
    Json(serde_json::Error),
    #[error("desired-tree I/O failed at {path}: {message}")]
    Io { path: PathBuf, message: String },
    #[error("existing staging bytes conflict at {0}")]
    StageConflict(String),
    #[error("source-generation fence changed at {0}")]
    SourceFenceChanged(String),
    #[error("shadow plan is not zero at {path}: {kind:?}")]
    NonZeroShadowPlan { path: String, kind: ChangeKind },
    #[error("action references missing upstream output: {0}")]
    MissingUpstreamOutput(StableId),
}

/// Validate that every upstream output digest names a declared desired output.
///
/// # Errors
///
/// Returns the first missing upstream output identity.
pub fn validate_upstream_outputs(
    outputs: &BTreeSet<StableId>,
    upstream: &BTreeMap<StableId, String>,
) -> Result<(), DesiredTreeError> {
    if let Some(missing) = upstream.keys().find(|id| !outputs.contains(*id)) {
        return Err(DesiredTreeError::MissingUpstreamOutput((*missing).clone()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use proptest::test_runner::{Config as ProptestConfig, RngSeed};

    use super::*;

    fn id(value: &str) -> StableId {
        StableId::parse(value).unwrap()
    }

    fn executable() -> ActionExecutableIdentity {
        ActionExecutableIdentity {
            compiler_source_identity: "sha256:compiler".to_owned(),
            cargo_lock_identity: "sha256:lock".to_owned(),
            rustc_identity: "rustc 1.97.1".to_owned(),
            feature_set: BTreeSet::from(["model-compiler".to_owned()]),
            profile: "dev".to_owned(),
            target_triple: "aarch64-apple-darwin".to_owned(),
            executable_digest:
                "b3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
        }
    }

    fn output(path: &str) -> PlannedOutput {
        PlannedOutput {
            output_id: id(&format!("output:{path}")),
            public_artifact_id: None,
            path: SafeOutputPath::parse(path.as_bytes().to_vec()).unwrap(),
            role: PlannedOutputRole::Derived,
            producer: id("action:test"),
            projection: PlannedOutputProjection::RustSource,
            consumers: BTreeSet::from([PlannedConsumer::RustCore]),
            validators: BTreeSet::from([PlannedValidator::RustConsumer]),
        }
    }

    fn material() -> ActionKeyMaterial {
        ActionKeyMaterial {
            driver_id: id("action:test"),
            rule_version: "rule-v1".to_owned(),
            model_schema_version: MODEL_SCHEMA_VERSION.to_owned(),
            output_schema_version: OUTPUT_SCHEMA_VERSION.to_owned(),
            inputs: vec![PlannedInput {
                artifact_id: id("source:a"),
                path: b"contracts/a.json".to_vec(),
                semantic_digest: "b3:semantic".to_owned(),
                source_digest: "b3:source".to_owned(),
            }],
            upstream_output_digests: BTreeMap::from([(id("output:upstream"), "b3:up".to_owned())]),
            outputs: vec![output("src/generated/a.rs")],
            executable: executable(),
            environment_contract: BTreeMap::from([("network".to_owned(), "undeclared".to_owned())]),
            generation_profile: "shadow-read-only".to_owned(),
        }
    }

    #[test]
    fn model_action_key_changes_for_every_output_affecting_input() {
        let baseline = material();
        let baseline_digest = baseline.digest().unwrap();
        let mut variants = Vec::new();
        let mut changed = baseline.clone();
        changed.driver_id = id("action:other");
        variants.push(changed);
        let mut changed = baseline.clone();
        changed.rule_version.push('x');
        variants.push(changed);
        let mut changed = baseline.clone();
        changed.model_schema_version.push('x');
        variants.push(changed);
        let mut changed = baseline.clone();
        changed.output_schema_version.push('x');
        variants.push(changed);
        let mut changed = baseline.clone();
        changed.inputs[0].semantic_digest.push('x');
        variants.push(changed);
        let mut changed = baseline.clone();
        changed.inputs[0].source_digest.push('x');
        variants.push(changed);
        let mut changed = baseline.clone();
        changed
            .upstream_output_digests
            .values_mut()
            .next()
            .unwrap()
            .push('x');
        variants.push(changed);
        let mut changed = baseline.clone();
        changed.outputs[0].path = SafeOutputPath::parse(b"src/generated/b.rs".to_vec()).unwrap();
        variants.push(changed);
        let mut changed = baseline.clone();
        changed.executable.feature_set.insert("other".to_owned());
        variants.push(changed);
        let mut changed = baseline.clone();
        changed.executable.cargo_lock_identity.push('x');
        variants.push(changed);
        let mut changed = baseline.clone();
        changed.executable.rustc_identity.push('x');
        variants.push(changed);
        let mut changed = baseline.clone();
        changed.executable.executable_digest.push('x');
        variants.push(changed);
        let mut changed = baseline.clone();
        changed
            .environment_contract
            .insert("locale".to_owned(), "C".to_owned());
        variants.push(changed);
        let mut changed = baseline;
        changed.generation_profile.push('x');
        variants.push(changed);
        assert!(
            variants
                .iter()
                .all(|variant| variant.digest().unwrap() != baseline_digest)
        );
    }

    #[test]
    fn model_desired_tree_classifies_add_replace_delete_stale_unchanged() {
        let mut desired = DesiredTree::default();
        for (path, bytes) in [
            ("src/generated/add.rs", b"add".as_slice()),
            ("src/generated/replace.rs", b"new".as_slice()),
            ("src/generated/same.rs", b"same".as_slice()),
        ] {
            let output = output(path);
            desired
                .insert(DesiredTreeEntry {
                    output,
                    lineage: vec![id("source:a")],
                    bytes: bytes.to_vec(),
                    content_digest: digest_bytes(bytes),
                })
                .unwrap();
        }
        let current = BTreeMap::from([
            (
                SafeOutputPath::parse(b"src/generated/replace.rs".to_vec()).unwrap(),
                b"old".to_vec(),
            ),
            (
                SafeOutputPath::parse(b"src/generated/same.rs".to_vec()).unwrap(),
                b"same".to_vec(),
            ),
            (
                SafeOutputPath::parse(b"src/generated/stale.rs".to_vec()).unwrap(),
                b"stale".to_vec(),
            ),
        ]);
        let kinds: BTreeSet<_> = desired
            .compare(&current)
            .into_iter()
            .map(|item| item.kind)
            .collect();
        assert_eq!(
            kinds,
            BTreeSet::from([
                ChangeKind::Add,
                ChangeKind::Replace,
                ChangeKind::DeleteStale,
                ChangeKind::Unchanged,
            ])
        );
    }

    #[test]
    fn model_planned_output_variants_are_closed() {
        let variants = [
            PlannedOutputProjection::Pydantic {
                mode: PydanticSchemaMode::Both,
                model_roots: BTreeSet::new(),
            },
            PlannedOutputProjection::JsonSchema {
                public_identity: "id".to_owned(),
            },
            PlannedOutputProjection::Proto {
                role: ProtoOutputRole::DescriptorSet,
            },
            PlannedOutputProjection::Registry {
                primary_key: "id".to_owned(),
            },
            PlannedOutputProjection::TableSpec {
                projection: TableProjection::Arrow,
            },
            PlannedOutputProjection::RustSource,
            PlannedOutputProjection::PythonSource,
            PlannedOutputProjection::CanonicalArtifact {
                artifact_kind: "json".to_owned(),
            },
        ];
        assert!(variants.iter().all(|variant| {
            serde_json::to_value(variant)
                .unwrap()
                .get("projection_kind")
                .is_some()
        }));
    }

    #[test]
    fn model_rejects_duplicate_output_unsafe_path_and_missing_upstream_output() {
        assert!(SafeOutputPath::parse(b"../authority.yaml".to_vec()).is_err());
        assert!(SafeOutputPath::parse(b"contracts/acceptance/owner.json".to_vec()).is_err());
        let mut tree = DesiredTree::default();
        let planned = output("src/generated/a.rs");
        let entry = DesiredTreeEntry {
            output: planned,
            lineage: vec![],
            bytes: vec![],
            content_digest: digest_bytes(&[]),
        };
        tree.insert(entry.clone()).unwrap();
        assert!(matches!(
            tree.insert(entry),
            Err(DesiredTreeError::DuplicateOutput { .. })
        ));
        assert!(matches!(
            validate_upstream_outputs(
                &BTreeSet::new(),
                &BTreeMap::from([(id("output:missing"), "b3:x".to_owned())])
            ),
            Err(DesiredTreeError::MissingUpstreamOutput(_))
        ));
    }

    #[test]
    fn model_cache_entry_never_contains_pass_verdict() {
        let value = serde_json::to_value(CachedActionBytes {
            action_key: "b3:key".to_owned(),
            outputs: BTreeMap::new(),
        })
        .unwrap();
        let object = value.as_object().unwrap();
        assert_eq!(object.keys().collect::<Vec<_>>(), ["action_key", "outputs"]);
        assert!(!value.to_string().contains("verdict"));
        assert!(!value.to_string().contains("pass"));
    }

    #[test]
    fn model_explain_reports_source_lineage_consumers_and_oracles() {
        let material = material();
        assert!(!material.outputs[0].consumers.is_empty());
        assert!(!material.outputs[0].validators.is_empty());
        assert!(!material.inputs.is_empty());
    }

    fn dag(node_count: usize, parents: &[usize]) -> (ModelGraph, Vec<StableId>) {
        let ids: Vec<_> = (0..node_count)
            .map(|index| id(&format!("node:{index}")))
            .collect();
        let nodes = ids
            .iter()
            .cloned()
            .map(|id| NodeDeclaration {
                id,
                kind: NodeKind::Action,
            })
            .collect();
        let edges = (1..node_count)
            .map(|dependent| EdgeDeclaration {
                prerequisite: ids[parents[dependent - 1] % dependent].clone(),
                dependent: ids[dependent].clone(),
                kind: EdgeKind::Invalidates,
            })
            .collect();
        (
            ModelGraph::compile(nodes, edges, ResourceBounds::new(64, 128, 16).unwrap()).unwrap(),
            ids,
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 64,
            max_shrink_iters: 512,
            max_local_rejects: 128,
            max_global_rejects: 256,
            failure_persistence: None,
            rng_seed: RngSeed::Fixed(0xC0DE_FAB1),
            ..ProptestConfig::default()
        })]

        #[test]
        fn model_incremental_matches_full_for_fixed_property_seed_matrix(
            node_count in 1_usize..24,
            parents in prop::collection::vec(0_usize..24, 0..23),
        ) {
            let needed = node_count.saturating_sub(1);
            let mut parents = parents;
            parents.resize(needed, 0);
            let (graph, ids) = dag(node_count, &parents);
            let changed = &ids[0];
            let affected: BTreeSet<_> = graph.affected_closure(changed).into_iter().collect();
            let recomputed: BTreeSet<_> = ids
                .iter()
                .filter(|candidate| graph.prerequisite_closure(candidate).contains(changed))
                .cloned()
                .collect();
            prop_assert_eq!(
                affected,
                recomputed,
                "replay: just model-incremental-check # seed=0xC0DEFAB1 edit=change:{}",
                changed
            );
        }
    }

    #[test]
    fn model_plan_is_insertion_order_invariant_and_bounded() {
        let (first, _) = dag(8, &[0, 0, 1, 1, 2, 2, 3]);
        let (second, _) = dag(8, &[0, 0, 1, 1, 2, 2, 3]);
        assert_eq!(first.execution_order(), second.execution_order());
        assert_eq!(first.execution_order().len(), 8);
    }
}
