//! Handwritten model-control core owned by the administrative model binary.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use petgraph::Direction;
use petgraph::algo::{kosaraju_scc, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef as _;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_STABLE_ID_BYTES: usize = 160;

/// A stable, human-reviewable model identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StableId(String);

impl StableId {
    /// Parse a bounded stable identifier.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty, oversized, or non-ASCII identifier, or for an
    /// identifier containing characters outside the closed portable alphabet.
    pub fn parse(value: impl Into<String>) -> Result<Self, ModelError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_STABLE_ID_BYTES
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':' | b'/')
            })
        {
            return Err(ModelError::InvalidStableId(value));
        }
        Ok(Self(value))
    }

    /// Borrow the stable text form.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for StableId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Closed model node families.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeKind {
    /// Native semantic source.
    Source,
    /// Irreducible conformance evidence.
    Evidence,
    /// Accountable human acceptance.
    Acceptance,
    /// One generation or validation action.
    Action,
    /// One derived repository output.
    Output,
    /// One requirement declaration.
    Requirement,
    /// One executable proof collector.
    Oracle,
}

/// Closed dependency-edge families. Edges point from prerequisite to dependent.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeKind {
    /// The dependent reads exact source bytes.
    ReadsExactBytes,
    /// The dependent reads compiled semantic identity.
    ReadsSemanticValue,
    /// An action produces an output.
    Produces,
    /// An implementation declaration satisfies a requirement.
    Implements,
    /// An oracle verifies a requirement or output.
    Verifies,
    /// A package or runtime consumes an output.
    Consumes,
    /// One output packages another output.
    Packages,
    /// One source or output participates in a bundle output.
    Bundles,
    /// A prerequisite change invalidates dependent work.
    Invalidates,
}

impl EdgeKind {
    /// Stable diagnostic code for the edge kind.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ReadsExactBytes => "reads-exact-bytes",
            Self::ReadsSemanticValue => "reads-semantic-value",
            Self::Produces => "produces",
            Self::Implements => "implements",
            Self::Verifies => "verifies",
            Self::Consumes => "consumes",
            Self::Packages => "packages",
            Self::Bundles => "bundles",
            Self::Invalidates => "invalidates",
        }
    }
}

/// Closed output roles known before family-specific payloads land.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputRole {
    /// Canonical or provenance index.
    Index,
    /// Generated Rust source.
    RustSource,
    /// Generated Python source.
    PythonSource,
    /// Schema or table contract.
    Schema,
    /// Protobuf descriptor set.
    DescriptorSet,
    /// Human-reviewable derived report.
    ReviewProjection,
}

/// Stable diagnostic classes shared by CLI and tests.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiagnosticClass {
    /// Stable identifier is malformed.
    InvalidStableId,
    /// A declared resource bound is invalid or exceeded.
    ResourceLimit,
    /// Two nodes share one stable identifier.
    DuplicateNode,
    /// An edge names an absent endpoint.
    UnknownEndpoint,
    /// One typed edge is repeated.
    DuplicateEdge,
    /// An edge is illegal for its endpoint node kinds.
    IllegalEdge,
    /// The prerequisite graph is cyclic.
    DependencyCycle,
    /// Canonical action identity could not be serialized.
    IdentityEncoding,
}

impl DiagnosticClass {
    /// Stable machine code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidStableId => "MODEL_INVALID_STABLE_ID",
            Self::ResourceLimit => "MODEL_RESOURCE_LIMIT",
            Self::DuplicateNode => "MODEL_DUPLICATE_NODE",
            Self::UnknownEndpoint => "MODEL_UNKNOWN_ENDPOINT",
            Self::DuplicateEdge => "MODEL_DUPLICATE_EDGE",
            Self::IllegalEdge => "MODEL_ILLEGAL_EDGE",
            Self::DependencyCycle => "MODEL_DEPENDENCY_CYCLE",
            Self::IdentityEncoding => "MODEL_IDENTITY_ENCODING",
        }
    }
}

/// Explicit resource bounds for graph compilation and diagnostic projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceBounds {
    /// Maximum number of nodes.
    pub max_nodes: usize,
    /// Maximum number of typed edges.
    pub max_edges: usize,
    /// Maximum node or edge items retained in one diagnostic witness.
    pub max_diagnostic_items: usize,
}

impl ResourceBounds {
    /// Construct non-zero bounds.
    ///
    /// # Errors
    ///
    /// Returns an error when any bound is zero.
    pub const fn new(
        max_nodes: usize,
        max_edges: usize,
        max_diagnostic_items: usize,
    ) -> Result<Self, ModelError> {
        if max_nodes == 0 || max_edges == 0 || max_diagnostic_items == 0 {
            return Err(ModelError::InvalidBounds);
        }
        Ok(Self {
            max_nodes,
            max_edges,
            max_diagnostic_items,
        })
    }
}

/// One node declaration before graph compilation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodeDeclaration {
    /// Stable identifier.
    pub id: StableId,
    /// Closed family.
    pub kind: NodeKind,
}

/// One prerequisite-to-dependent edge declaration.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct EdgeDeclaration {
    /// Prerequisite node.
    pub prerequisite: StableId,
    /// Dependent node.
    pub dependent: StableId,
    /// Typed dependency role.
    pub kind: EdgeKind,
}

/// One deterministic edge in a cycle witness.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct WitnessEdge {
    /// Prerequisite stable ID.
    pub prerequisite: StableId,
    /// Dependent stable ID.
    pub dependent: StableId,
    /// Edge kind.
    pub kind: EdgeKind,
}

/// Bounded deterministic strongly-connected-component projection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CycleWitness {
    /// Sorted stable IDs retained in the witness.
    pub nodes: Vec<StableId>,
    /// Sorted internal typed edges retained in the witness.
    pub edges: Vec<WitnessEdge>,
    /// Whether resource bounds omitted additional witness items.
    pub truncated: bool,
}

/// Model compilation failure.
#[derive(Debug, Error)]
pub enum ModelError {
    /// Stable identifier is malformed.
    #[error("invalid stable model identifier: {0}")]
    InvalidStableId(String),
    /// Resource bounds must be non-zero.
    #[error("model resource bounds must be non-zero")]
    InvalidBounds,
    /// Graph size exceeds a declared bound.
    #[error("model {resource} count {observed} exceeds limit {limit}")]
    ResourceLimit {
        /// Resource name.
        resource: &'static str,
        /// Observed count.
        observed: usize,
        /// Accepted limit.
        limit: usize,
    },
    /// Two nodes have one ID.
    #[error("duplicate model node: {0}")]
    DuplicateNode(StableId),
    /// An edge endpoint is absent.
    #[error("unknown {role} endpoint {id} for {kind}")]
    UnknownEndpoint {
        /// `prerequisite` or `dependent`.
        role: &'static str,
        /// Missing stable ID.
        id: StableId,
        /// Edge kind.
        kind: &'static str,
    },
    /// The exact same typed edge was declared twice.
    #[error("duplicate {kind} edge from {prerequisite} to {dependent}")]
    DuplicateEdge {
        /// Prerequisite stable ID.
        prerequisite: StableId,
        /// Dependent stable ID.
        dependent: StableId,
        /// Edge kind.
        kind: &'static str,
    },
    /// Edge endpoint kinds violate the closed graph schema.
    #[error("illegal {kind} edge from {prerequisite_kind:?} to {dependent_kind:?}")]
    IllegalEdge {
        /// Prerequisite node kind.
        prerequisite_kind: NodeKind,
        /// Dependent node kind.
        dependent_kind: NodeKind,
        /// Edge kind.
        kind: &'static str,
    },
    /// Dependency cycle with bounded typed witness.
    #[error("model dependency cycle")]
    DependencyCycle(CycleWitness),
    /// Canonical identity serialization failed.
    #[error("canonical model identity encoding failed: {0}")]
    IdentityEncoding(serde_json::Error),
}

impl ModelError {
    /// Return the stable diagnostic class.
    #[must_use]
    pub const fn diagnostic_class(&self) -> DiagnosticClass {
        match self {
            Self::InvalidStableId(_) => DiagnosticClass::InvalidStableId,
            Self::InvalidBounds | Self::ResourceLimit { .. } => DiagnosticClass::ResourceLimit,
            Self::DuplicateNode(_) => DiagnosticClass::DuplicateNode,
            Self::UnknownEndpoint { .. } => DiagnosticClass::UnknownEndpoint,
            Self::DuplicateEdge { .. } => DiagnosticClass::DuplicateEdge,
            Self::IllegalEdge { .. } => DiagnosticClass::IllegalEdge,
            Self::DependencyCycle(_) => DiagnosticClass::DependencyCycle,
            Self::IdentityEncoding(_) => DiagnosticClass::IdentityEncoding,
        }
    }
}

/// Immutable compiled graph with stable execution order.
#[derive(Debug)]
pub struct ModelGraph {
    graph: DiGraph<NodeDeclaration, EdgeKind>,
    indices: BTreeMap<StableId, NodeIndex>,
    order: Vec<StableId>,
}

impl ModelGraph {
    /// Compile closed declarations into a bounded acyclic graph.
    ///
    /// Edges are oriented prerequisite to dependent. Distinct edge kinds may connect
    /// the same pair; an exact duplicate typed edge is rejected.
    ///
    /// # Errors
    ///
    /// Returns a stable typed error for bounds, duplicate IDs/edges, missing endpoints,
    /// or a dependency cycle.
    pub fn compile(
        mut nodes: Vec<NodeDeclaration>,
        mut edges: Vec<EdgeDeclaration>,
        bounds: ResourceBounds,
    ) -> Result<Self, ModelError> {
        enforce_limit("node", nodes.len(), bounds.max_nodes)?;
        enforce_limit("edge", edges.len(), bounds.max_edges)?;
        nodes.sort_by(|left, right| left.id.cmp(&right.id));
        edges.sort();

        let mut graph = DiGraph::with_capacity(nodes.len(), edges.len());
        let mut indices = BTreeMap::new();
        for node in nodes {
            if indices.contains_key(&node.id) {
                return Err(ModelError::DuplicateNode(node.id));
            }
            let id = node.id.clone();
            indices.insert(id, graph.add_node(node));
        }

        let mut previous: Option<&EdgeDeclaration> = None;
        for edge in &edges {
            if previous == Some(edge) {
                return Err(ModelError::DuplicateEdge {
                    prerequisite: edge.prerequisite.clone(),
                    dependent: edge.dependent.clone(),
                    kind: edge.kind.code(),
                });
            }
            previous = Some(edge);
            let prerequisite = indices.get(&edge.prerequisite).copied().ok_or_else(|| {
                ModelError::UnknownEndpoint {
                    role: "prerequisite",
                    id: edge.prerequisite.clone(),
                    kind: edge.kind.code(),
                }
            })?;
            let dependent = indices.get(&edge.dependent).copied().ok_or_else(|| {
                ModelError::UnknownEndpoint {
                    role: "dependent",
                    id: edge.dependent.clone(),
                    kind: edge.kind.code(),
                }
            })?;
            let prerequisite_kind = graph[prerequisite].kind;
            let dependent_kind = graph[dependent].kind;
            if !legal_edge(prerequisite_kind, dependent_kind, edge.kind) {
                return Err(ModelError::IllegalEdge {
                    prerequisite_kind,
                    dependent_kind,
                    kind: edge.kind.code(),
                });
            }
            graph.add_edge(prerequisite, dependent, edge.kind);
        }

        if toposort(&graph, None).is_err() {
            return Err(ModelError::DependencyCycle(cycle_witness(
                &graph,
                bounds.max_diagnostic_items,
            )));
        }
        let order = deterministic_topological_order(&graph);
        Ok(Self {
            graph,
            indices,
            order,
        })
    }

    /// Return the stable prerequisite-first order.
    #[must_use]
    pub fn execution_order(&self) -> &[StableId] {
        &self.order
    }

    /// Return the direct affected closure, including the changed node.
    #[must_use]
    pub fn affected_closure(&self, changed: &StableId) -> Vec<StableId> {
        self.closure(changed, Direction::Outgoing)
    }

    /// Return the prerequisite closure, including the selected node.
    #[must_use]
    pub fn prerequisite_closure(&self, dependent: &StableId) -> Vec<StableId> {
        self.closure(dependent, Direction::Incoming)
    }

    /// Whether the compiled graph contains a stable node.
    #[must_use]
    pub fn contains(&self, id: &StableId) -> bool {
        self.indices.contains_key(id)
    }

    fn closure(&self, selected: &StableId, direction: Direction) -> Vec<StableId> {
        let Some(start) = self.indices.get(selected).copied() else {
            return Vec::new();
        };
        let mut pending = vec![start];
        let mut visited = BTreeSet::new();
        while let Some(node) = pending.pop() {
            if !visited.insert(self.graph[node].id.clone()) {
                continue;
            }
            let mut adjacent: Vec<_> = self.graph.neighbors_directed(node, direction).collect();
            adjacent.sort_by(|left, right| self.graph[*right].id.cmp(&self.graph[*left].id));
            pending.extend(adjacent);
        }
        visited.into_iter().collect()
    }
}

const fn legal_edge(prerequisite: NodeKind, dependent: NodeKind, edge: EdgeKind) -> bool {
    match edge {
        EdgeKind::ReadsExactBytes | EdgeKind::ReadsSemanticValue => {
            matches!(
                prerequisite,
                NodeKind::Source | NodeKind::Evidence | NodeKind::Acceptance | NodeKind::Output
            ) && matches!(dependent, NodeKind::Action | NodeKind::Oracle)
        }
        EdgeKind::Produces => {
            matches!(prerequisite, NodeKind::Action) && matches!(dependent, NodeKind::Output)
        }
        EdgeKind::Implements => {
            matches!(prerequisite, NodeKind::Source | NodeKind::Output)
                && matches!(dependent, NodeKind::Requirement)
        }
        EdgeKind::Verifies => {
            matches!(prerequisite, NodeKind::Requirement | NodeKind::Output)
                && matches!(dependent, NodeKind::Oracle)
        }
        EdgeKind::Consumes => {
            matches!(prerequisite, NodeKind::Source | NodeKind::Output)
                && matches!(dependent, NodeKind::Action | NodeKind::Oracle)
        }
        EdgeKind::Packages => {
            matches!(prerequisite, NodeKind::Output) && matches!(dependent, NodeKind::Output)
        }
        EdgeKind::Bundles => {
            matches!(prerequisite, NodeKind::Source | NodeKind::Output)
                && matches!(dependent, NodeKind::Output)
        }
        EdgeKind::Invalidates => {
            matches!(prerequisite, NodeKind::Action | NodeKind::Output)
                && matches!(dependent, NodeKind::Action | NodeKind::Output)
        }
    }
}

fn enforce_limit(resource: &'static str, observed: usize, limit: usize) -> Result<(), ModelError> {
    if observed > limit {
        Err(ModelError::ResourceLimit {
            resource,
            observed,
            limit,
        })
    } else {
        Ok(())
    }
}

fn deterministic_topological_order(graph: &DiGraph<NodeDeclaration, EdgeKind>) -> Vec<StableId> {
    let mut indegrees: BTreeMap<NodeIndex, usize> = graph
        .node_indices()
        .map(|node| {
            (
                node,
                graph.neighbors_directed(node, Direction::Incoming).count(),
            )
        })
        .collect();
    let mut ready: BTreeSet<(StableId, NodeIndex)> = indegrees
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(node, _)| (graph[*node].id.clone(), *node))
        .collect();
    let mut order = Vec::with_capacity(graph.node_count());
    while let Some((id, node)) = ready.pop_first() {
        order.push(id);
        let mut dependents: Vec<_> = graph
            .neighbors_directed(node, Direction::Outgoing)
            .collect();
        dependents.sort_by(|left, right| graph[*left].id.cmp(&graph[*right].id));
        for dependent in dependents {
            let count = indegrees
                .get_mut(&dependent)
                .expect("compiled graph contains every node");
            *count -= 1;
            if *count == 0 {
                ready.insert((graph[dependent].id.clone(), dependent));
            }
        }
    }
    debug_assert_eq!(order.len(), graph.node_count());
    order
}

fn cycle_witness(graph: &DiGraph<NodeDeclaration, EdgeKind>, max_items: usize) -> CycleWitness {
    let mut cyclic_components: Vec<Vec<NodeIndex>> = kosaraju_scc(graph)
        .into_iter()
        .filter(|component| {
            component.len() > 1
                || component
                    .first()
                    .is_some_and(|node| graph.edges_connecting(*node, *node).next().is_some())
        })
        .collect();
    cyclic_components
        .sort_by(|left, right| component_min_id(graph, left).cmp(component_min_id(graph, right)));
    let component = cyclic_components
        .first()
        .expect("toposort cycle must have one cyclic SCC");
    let component_set: BTreeSet<_> = component.iter().copied().collect();
    let mut all_nodes: Vec<_> = component
        .iter()
        .map(|node| graph[*node].id.clone())
        .collect();
    all_nodes.sort();
    let mut all_edges: Vec<_> = graph
        .edge_references()
        .filter(|edge| {
            component_set.contains(&edge.source()) && component_set.contains(&edge.target())
        })
        .map(|edge| WitnessEdge {
            prerequisite: graph[edge.source()].id.clone(),
            dependent: graph[edge.target()].id.clone(),
            kind: *edge.weight(),
        })
        .collect();
    all_edges.sort();
    let truncated = all_nodes.len() > max_items || all_edges.len() > max_items;
    all_nodes.truncate(max_items);
    all_edges.truncate(max_items);
    CycleWitness {
        nodes: all_nodes,
        edges: all_edges,
        truncated,
    }
}

fn component_min_id<'a>(
    graph: &'a DiGraph<NodeDeclaration, EdgeKind>,
    component: &[NodeIndex],
) -> &'a StableId {
    component
        .iter()
        .map(|node| &graph[*node].id)
        .min()
        .expect("SCC is non-empty")
}

/// Compiler build inputs that determine an isolated Cargo output root.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompilerBuildIdentity {
    /// BLAKE3 digest over the exact compiler source set and Cargo manifest.
    pub compiler_source_digest: String,
    /// Exact `rustc -vV` identity.
    pub rustc_identity: String,
    /// Exact Cargo.lock digest.
    pub cargo_lock_digest: String,
    /// Exact sorted Cargo feature set.
    pub feature_set: BTreeSet<String>,
    /// Cargo profile.
    pub profile: String,
    /// Rust target triple.
    pub target_triple: String,
}

impl CompilerBuildIdentity {
    /// Compute the canonical BLAKE3 identity.
    ///
    /// # Errors
    ///
    /// Returns an error if the typed record cannot be serialized canonically.
    pub fn digest(&self) -> Result<String, ModelError> {
        canonical_digest(self)
    }

    /// Resolve a dedicated target root from the complete build identity.
    ///
    /// # Errors
    ///
    /// Returns an error if the identity cannot be encoded.
    pub fn target_dir(&self, target_root: &Path) -> Result<PathBuf, ModelError> {
        let digest = self.digest()?;
        let path_component = digest.strip_prefix("b3:").unwrap_or(&digest);
        Ok(target_root.join("model-builds").join(path_component))
    }
}

/// Built executable identity used by external-driver action keys.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DriverExecutableIdentity {
    /// Compiler/build identity.
    pub build: CompilerBuildIdentity,
    /// BLAKE3 digest over exact executable bytes.
    pub executable_digest: String,
}

impl DriverExecutableIdentity {
    /// Compute the canonical combined action identity.
    ///
    /// # Errors
    ///
    /// Returns an error if the typed identity cannot be serialized canonically.
    pub fn digest(&self) -> Result<String, ModelError> {
        canonical_digest(self)
    }
}

fn canonical_digest(value: &impl Serialize) -> Result<String, ModelError> {
    let value = serde_json::to_value(value).map_err(ModelError::IdentityEncoding)?;
    let bytes = serde_json_canonicalizer::to_vec(&value).map_err(ModelError::IdentityEncoding)?;
    Ok(format!("b3:{}", blake3::hash(&bytes).to_hex()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> StableId {
        StableId::parse(value).unwrap()
    }

    fn node(value: &str) -> NodeDeclaration {
        node_kind(value, NodeKind::Action)
    }

    fn node_kind(value: &str, kind: NodeKind) -> NodeDeclaration {
        NodeDeclaration {
            id: id(value),
            kind,
        }
    }

    fn edge(from: &str, to: &str, kind: EdgeKind) -> EdgeDeclaration {
        EdgeDeclaration {
            prerequisite: id(from),
            dependent: id(to),
            kind,
        }
    }

    fn bounds() -> ResourceBounds {
        ResourceBounds::new(32, 64, 8).unwrap()
    }

    fn build_identity(rustc: &str, feature: &str) -> CompilerBuildIdentity {
        CompilerBuildIdentity {
            compiler_source_digest:
                "b3:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_owned(),
            rustc_identity: rustc.to_owned(),
            cargo_lock_digest:
                "b3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            feature_set: BTreeSet::from([feature.to_owned()]),
            profile: "dev".to_owned(),
            target_triple: "aarch64-apple-darwin".to_owned(),
        }
    }

    #[test]
    fn model_graph_order_is_insertion_invariant() {
        let nodes = vec![
            node_kind("output:c", NodeKind::Output),
            node_kind("source:a", NodeKind::Source),
            node("action:b"),
        ];
        let edges = vec![
            edge("action:b", "output:c", EdgeKind::Produces),
            edge("source:a", "action:b", EdgeKind::ReadsSemanticValue),
        ];
        let first = ModelGraph::compile(nodes.clone(), edges.clone(), bounds()).unwrap();
        let second = ModelGraph::compile(
            nodes.into_iter().rev().collect(),
            edges.into_iter().rev().collect(),
            bounds(),
        )
        .unwrap();
        assert_eq!(first.execution_order(), second.execution_order());
        assert_eq!(
            first.execution_order(),
            &[id("source:a"), id("action:b"), id("output:c")]
        );
        assert_eq!(
            first.affected_closure(&id("source:a")),
            vec![id("action:b"), id("output:c"), id("source:a")]
        );
        assert_eq!(
            first.prerequisite_closure(&id("output:c")),
            vec![id("action:b"), id("output:c"), id("source:a")]
        );
    }

    #[test]
    fn model_cycles_project_stable_nodes_and_typed_edges() {
        let cases = [
            (vec![node("a")], vec![edge("a", "a", EdgeKind::Invalidates)]),
            (
                vec![node("b"), node("a")],
                vec![
                    edge("b", "a", EdgeKind::Invalidates),
                    edge("a", "b", EdgeKind::Invalidates),
                ],
            ),
            (
                vec![node("c"), node("a"), node("b")],
                vec![
                    edge("c", "a", EdgeKind::Invalidates),
                    edge("a", "b", EdgeKind::Invalidates),
                    edge("b", "c", EdgeKind::Invalidates),
                ],
            ),
        ];
        for (nodes, edges) in cases {
            let error = ModelGraph::compile(nodes, edges, bounds()).unwrap_err();
            assert_eq!(error.diagnostic_class().code(), "MODEL_DEPENDENCY_CYCLE");
            let ModelError::DependencyCycle(witness) = error else {
                unreachable!();
            };
            assert!(witness.nodes.windows(2).all(|pair| pair[0] < pair[1]));
            assert!(witness.edges.windows(2).all(|pair| pair[0] < pair[1]));
            assert!(!witness.edges.is_empty());
        }
    }

    #[test]
    fn model_parallel_edge_kinds_are_preserved_and_exact_duplicates_fail() {
        let graph = ModelGraph::compile(
            vec![
                node_kind("a", NodeKind::Source),
                node_kind("b", NodeKind::Action),
            ],
            vec![
                edge("a", "b", EdgeKind::ReadsExactBytes),
                edge("a", "b", EdgeKind::ReadsSemanticValue),
            ],
            bounds(),
        )
        .unwrap();
        assert_eq!(graph.execution_order(), &[id("a"), id("b")]);

        let error = ModelGraph::compile(
            vec![node("a"), node("b")],
            vec![
                edge("a", "b", EdgeKind::Invalidates),
                edge("a", "b", EdgeKind::Invalidates),
            ],
            bounds(),
        )
        .unwrap_err();
        assert_eq!(error.diagnostic_class(), DiagnosticClass::DuplicateEdge);
    }

    #[test]
    fn model_graph_rejects_illegal_edges_duplicates_and_cycles() {
        let illegal = ModelGraph::compile(
            vec![
                node_kind("source:a", NodeKind::Source),
                node_kind("source:b", NodeKind::Source),
            ],
            vec![edge("source:a", "source:b", EdgeKind::Produces)],
            bounds(),
        )
        .unwrap_err();
        assert_eq!(illegal.diagnostic_class(), DiagnosticClass::IllegalEdge);

        let duplicate = ModelGraph::compile(
            vec![node("action:a"), node("action:b")],
            vec![
                edge("action:a", "action:b", EdgeKind::Invalidates),
                edge("action:a", "action:b", EdgeKind::Invalidates),
            ],
            bounds(),
        )
        .unwrap_err();
        assert_eq!(duplicate.diagnostic_class(), DiagnosticClass::DuplicateEdge);

        let cycle = ModelGraph::compile(
            vec![node("action:a"), node("action:b")],
            vec![
                edge("action:a", "action:b", EdgeKind::Invalidates),
                edge("action:b", "action:a", EdgeKind::Invalidates),
            ],
            bounds(),
        )
        .unwrap_err();
        assert_eq!(cycle.diagnostic_class(), DiagnosticClass::DependencyCycle);
    }

    #[test]
    fn model_diagnostic_classes_are_stable_and_bounded() {
        assert_eq!(
            DiagnosticClass::DependencyCycle.code(),
            "MODEL_DEPENDENCY_CYCLE"
        );
        let error = ModelGraph::compile(
            vec![node("a")],
            vec![edge("a", "a", EdgeKind::Invalidates)],
            ResourceBounds::new(2, 2, 1).unwrap(),
        )
        .unwrap_err();
        let ModelError::DependencyCycle(witness) = error else {
            unreachable!();
        };
        assert!(witness.nodes.len() <= 1);
        assert!(witness.edges.len() <= 1);
    }

    #[test]
    fn model_action_identity_distinguishes_feature_and_toolchain_builds() {
        let first = build_identity("rustc-a", "model-compiler");
        let feature_changed = build_identity("rustc-a", "model-compiler,canonical-json");
        let toolchain_changed = build_identity("rustc-b", "model-compiler");
        let mut source_changed = first.clone();
        source_changed.compiler_source_digest =
            "b3:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_owned();
        assert_ne!(first.digest().unwrap(), feature_changed.digest().unwrap());
        assert_ne!(first.digest().unwrap(), toolchain_changed.digest().unwrap());
        assert_ne!(first.digest().unwrap(), source_changed.digest().unwrap());
    }

    #[test]
    fn model_feature_distinct_compilers_do_not_share_executable_path() {
        let first = build_identity("rustc-a", "model-compiler");
        let second = build_identity("rustc-a", "model-compiler,future-driver");
        assert_ne!(
            first.target_dir(Path::new("target")).unwrap(),
            second.target_dir(Path::new("target")).unwrap()
        );
        let executable_a = DriverExecutableIdentity {
            build: first,
            executable_digest:
                "b3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
        };
        let mut executable_b = executable_a.clone();
        executable_b.executable_digest =
            "b3:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_owned();
        assert_ne!(
            executable_a.digest().unwrap(),
            executable_b.digest().unwrap()
        );
    }
}
