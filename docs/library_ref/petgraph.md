# Petgraph advanced technical doc plan — feature-category catalog

Below is a structured topic map for a deep-dive technical document on Rust’s **petgraph** library, modeled after the uploaded Cyclopts advanced-doc template. 

Version anchor: the current docs.rs “latest” page is **petgraph 0.8.3**; the public repo also warns that the trunk branch is transitioning toward a new multi-crate architecture, so the documentation should explicitly distinguish **released 0.8.x behavior** from trunk/development behavior. ([Docs.rs][1])

---

## 0) Scope, versioning, and the petgraph mental model

* What petgraph is: Rust graph data structures + algorithms + Graphviz/DOT output.
* Core model: nodes, edges, node weights, edge weights, directionality, indices, and graph traits.
* Released docs vs trunk architecture transition.
* What “weights” mean in petgraph: arbitrary associated data, not necessarily numeric costs.
* How petgraph differs from a graph database, NetworkX-style high-level toolkit, and domain-specific graph engines.

---

## 1) Installation, Cargo features, and deployment surface

* `cargo add petgraph` and `Cargo.toml` dependency patterns.
* Default features: `graphmap`, `stable_graph`, `matrix_graph`, `std`.
* Optional features: `serde-1`, `rayon`, `dot_parser`, `generate`, `unstable`.
* `no_std` story via disabling `std`.
* MSRV / Rust-version policy.
* Deployment patterns for libraries, CLIs, WASM/no-std targets, and serialization-heavy apps. ([Docs.rs][2])

---

## 2) Graph type decision guide

* `Graph<N, E, Ty, Ix>`: general adjacency-list graph.
* `StableGraph<N, E, Ty, Ix>`: adjacency-list graph with stable unrelated indices across removals.
* `GraphMap<N, E, Ty>`: node identifiers are the node values themselves, backed by hash-map-like storage.
* `MatrixGraph<N, E, ...>`: adjacency-matrix representation for dense graphs.
* `Csr<N, E, Ty, Ix>`: compressed sparse row graph, optimized for sparse adjacency iteration.
* Decision matrix: sparse vs dense, stable indices vs compact indices, external IDs vs internal `NodeIndex`, parallel edges vs no parallel edges, mutation frequency, memory footprint. ([Docs.rs][2])

---

## 3) Core generics and type aliases

* `N`: node weight type.
* `E`: edge weight type.
* `Ty`: `Directed` vs `Undirected`.
* `Ix`: index storage type and graph-size limit.
* `DefaultIx`, `NodeIndex<Ix>`, `EdgeIndex<Ix>`.
* Shorthand aliases: `Graph`, `DiGraph`, `UnGraph`, `StableDiGraph`, `StableUnGraph`, `DiGraphMap`, `UnGraphMap`, matrix aliases.
* When to choose `u16`, `u32`, `usize` index types.

---

## 4) `Graph` deep dive: adjacency-list workhorse

* Constructors: `Graph::new`, `Graph::new_undirected`, `Graph::with_capacity`, `Graph::from_edges`.
* Mutations: `add_node`, `try_add_node`, `add_edge`, `try_add_edge`, `update_edge`, `remove_node`, `remove_edge`.
* Accessors: `node_weight`, `node_weight_mut`, `edge_weight`, `edge_weight_mut`, indexing syntax.
* Iterators: `node_indices`, `edge_indices`, `neighbors`, `edges`, `edge_references`, `node_weights`.
* Parallel-edge behavior and `update_edge` as duplicate-edge control.
* Index invalidation rules on removals.
* Capacity management and performance implications. ([Docs.rs][3])

---

## 5) `StableGraph` deep dive: persistent index use cases

* Stable index contract and what “unrelated indices” means.
* Holes/gaps after deletions.
* Memory and feature-parity tradeoffs.
* When stable indices matter: external references, long-lived IDs, UI selections, incremental algorithms.
* Migration from `Graph` to `StableGraph`.
* Pitfalls: assuming compact index ranges, relying on `NodeCompactIndexable`, deletion-heavy workloads. ([Docs.rs][4])

---

## 6) `GraphMap` deep dive: map-key node identity

* Node values as identifiers.
* Required node bounds: `Copy`, `Eq`, `Hash`, `Ord`.
* `DiGraphMap` and `UnGraphMap`.
* Constant-time edge-existence testing.
* No parallel edges; self-loops allowed.
* Automatic node insertion through `add_edge`.
* Hasher customization and deterministic/fast hashing choices.
* Best fit: small copyable node IDs, enum keys, integer IDs, string-like interned IDs. ([Docs.rs][5])

---

## 7) `MatrixGraph` deep dive: dense graph representation

* Adjacency matrix storage model.
* Space complexity and dense-graph suitability.
* `Null` edge-presence abstraction: `Option<E>` vs sentinel strategies such as `NotZero`.
* Fast edge insertion/lookup tradeoffs.
* No parallel edges.
* Large edge-weight warning and boxing strategy.
* Dense graph algorithms and when matrix form beats adjacency lists. ([Docs.rs][6])

---

## 8) `Csr` deep dive: compressed sparse row graphs

* CSR mental model: sparse adjacency matrix.
* `with_nodes`, `from_sorted_edges`, sorted/unique edge requirement.
* Fast outgoing-edge iteration.
* Self-loops allowed; no parallel edges.
* Construction cost and row-major insertion behavior.
* Undirected CSR convention: store both `(u, v)` and `(v, u)`.
* Best fit: mostly static sparse graphs, traversal-heavy workloads, imported edge lists. ([Docs.rs][7])

---

## 9) Directedness, edge semantics, and graph invariants

* `Directed`, `Undirected`, `EdgeType`, `Direction::{Incoming, Outgoing}`.
* Neighbor semantics for directed vs undirected graphs.
* Self-loops and parallel edges by graph type.
* Edge orientation in undirected graphs.
* `reverse`, `into_edge_type`, and direction conversion patterns.
* Modeling one-way roads, dependency DAGs, bidirectional networks, multigraph-like data.

---

## 10) Node and edge weights as domain data

* Weight terminology and arbitrary associated data.
* Modeling zero-sized weights with `()`.
* Numeric costs vs domain metadata.
* `map`, `map_owned`, `filter_map`, `filter_map_owned`.
* Borrowing vs cloning weights.
* Best practices for large payloads: arena IDs, `Arc`, `Box`, external stores.
* Separating graph topology from business data.

---

## 11) Indexing, identity, and mutation safety

* `NodeIndex` / `EdgeIndex` as lightweight handles.
* Stable vs compact index guarantees.
* Index invalidation after removal.
* `NodeIndexable`, `NodeCompactIndexable`, `EdgeIndexable`.
* External ID maps: `HashMap<DomainId, NodeIndex>`.
* Anti-patterns: storing `NodeIndex` from a mutable `Graph` across removals without a stability strategy.
* Safe mutation while traversing: detached walkers and traversal patterns.

---

## 12) Construction patterns and graph loading

* `new`, `with_capacity`, `from_edges`, `extend_with_edges`.
* `FromElements` and algorithm-output-to-graph patterns.
* Building from domain records.
* Deduplicating nodes and edges.
* Loading from edge lists, adjacency lists, matrices, DOT, Graph6, and serialized data.
* Capacity planning for large imports.
* Error-first construction with `try_add_node`, `try_add_edge`, `try_update_edge`.

---

## 13) Traversal system: visitors, walkers, and graph traits

* `visit` module mental model.
* `Dfs`, `Bfs`, `DfsPostOrder`, `Topo`.
* `depth_first_search` callback API.
* `Walker` and `WalkerIter`.
* Why walkers do not borrow the graph for the whole traversal.
* Minimum trait set for custom graph compatibility: `GraphBase`, `IntoNeighbors`, `Visitable`.
* Direction-aware traversal and filtered traversal. ([Docs.rs][8])

---

## 14) Trait-based graph abstraction

* `GraphBase`, `GraphProp`, `GraphRef`.
* `IntoNeighbors`, `IntoNeighborsDirected`.
* `IntoEdges`, `IntoEdgesDirected`, `IntoEdgeReferences`.
* `IntoNodeIdentifiers`, `IntoNodeReferences`.
* `Data`, `DataMap`, `DataMapMut`.
* `NodeCount`, `EdgeCount`.
* Writing generic functions over graph traits.
* Which graph types implement which traits.
* Tradeoffs: generic algorithms vs concrete `Graph`-only APIs. ([Docs.rs][8])

---

## 15) Algorithm catalog: shortest paths

* `dijkstra`
* `bidirectional_dijkstra`
* `astar`
* `bellman_ford`
* `find_negative_cycle`
* `floyd_warshall`
* `johnson`
* `parallel_johnson`
* `spfa`
* `k_shortest_path`
* Cost closures, numeric bounds, negative weights, all-pairs vs single-source, early exit, predecessor reconstruction.
* Best-practice decision table: Dijkstra vs A* vs Bellman-Ford vs Floyd-Warshall vs Johnson. ([Docs.rs][9])

---

## 16) Algorithm catalog: connectivity, components, cycles, DAGs

* `connected_components`
* `has_path_connecting`
* `kosaraju_scc`
* `tarjan_scc`
* `condensation`
* `is_cyclic_directed`
* `is_cyclic_undirected`
* `toposort`
* `Topo`
* `acyclic::Acyclic`
* DAG modeling, topological position, cycle errors, SCC condensation workflows. ([Docs.rs][9])

---

## 17) Algorithm catalog: spanning trees, cuts, bridges, articulation

* `min_spanning_tree`
* `min_spanning_tree_prim`
* `bridges`
* articulation points module
* `UnionFind`
* Edge-weight requirements and ordering.
* Building an MST result graph with `FromElements`.
* Practical use cases: network design, clustering, infrastructure planning.

---

## 18) Algorithm catalog: matching, flow, cliques, coloring

* `greedy_matching`
* `maximum_matching`
* `Matching`
* `dinics`
* `ford_fulkerson`
* `maximal_cliques`
* `dsatur_coloring`
* Capacity models, matching outputs, clique enumeration costs, coloring heuristics.
* When petgraph algorithms are enough vs when to use specialized optimization crates. ([Docs.rs][9])

---

## 19) Algorithm catalog: isomorphism and subgraph matching

* `is_isomorphic`
* `is_isomorphic_matching`
* `is_isomorphic_subgraph`
* `is_isomorphic_subgraph_matching`
* `subgraph_isomorphisms_iter`
* Node/edge matching closures.
* Exact matching vs domain-equivalence matching.
* Performance caveats and pruning strategies.
* Use cases: pattern detection, chemistry, workflow graph comparison. ([Docs.rs][9])

---

## 20) Algorithm catalog: graph analytics and specialized routines

* `page_rank`
* `greedy_feedback_arc_set`
* `steiner_tree`
* transitive reduction/closure via `tred`
* dominators for control-flow graphs.
* Analytics-oriented workflows and limitations.
* Parallel algorithm availability behind `rayon`. ([Docs.rs][9])

---

## 21) Graph adaptors and filtered views

* `Reversed`
* `UndirectedAdaptor`
* `NodeFiltered`
* `EdgeFiltered`
* Reversed edge references.
* Filter traits and closure-based graph views.
* When to use adaptors instead of materializing a new graph.
* Common patterns: run an algorithm on a reversed graph, restrict traversal to active nodes, hide disabled edges.

---

## 22) Graph operators and transformations

* `operator::complement`
* `condensation`
* `map`, `filter_map`, owned variants.
* Edge-type conversion and graph cloning strategies.
* Topology-preserving vs topology-changing transformations.
* Building derived graphs for algorithms or visualization. ([Docs.rs][10])

---

## 23) Visualization and DOT / Graphviz workflows

* `dot::Dot`
* `Dot::new`
* `Dot::with_config`
* `Dot::with_attr_getters`
* `Config::{EdgeNoLabel, NodeNoLabel, ...}`
* Styling nodes and edges with custom attribute getters.
* Writing `.dot` files and rendering with Graphviz.
* `dot_parser` feature for importing DOT into `Graph` / `StableGraph`.
* Best practices for debug visualization and CI artifacts. ([Docs.rs][2])

---

## 24) Serialization, import/export, and interoperability

* `serde-1` feature for `Graph`, `StableGraph`, `GraphMap`.
* Graph6 support for undirected graphs.
* DOT import with `dot_parser`.
* Raw topology export patterns.
* Stable file formats vs internal serde formats.
* Versioning serialized graphs across crate upgrades.
* Interop with `serde_json`, `bincode`, `ron`, databases, and domain-specific schemas. ([Docs.rs][2])

---

## 25) Parallelism and performance engineering

* `rayon` feature scope.
* Parallel iterators and parallel algorithms where available.
* Big-O by graph type.
* Memory layout: adjacency list vs matrix vs CSR.
* Index type choice and cache behavior.
* Preallocation with `with_capacity`, `reserve_nodes`, `reserve_edges`.
* Avoiding heavy weights inside graph storage.
* Benchmarking recipes with representative topology, density, mutation rate, and algorithm mix.

---

## 26) `no_std`, embedded, and constrained environments

* Disabling `std`.
* Which features require `std`.
* Data-structure choices for memory-bounded targets.
* Avoiding heap-heavy weights.
* Serialization and allocation constraints.
* Suitability boundaries for embedded graph workloads. ([Docs.rs][2])

---

## 27) Error handling, panics, and fallible APIs

* Panicking APIs: `add_edge`, indexing, some matrix operations.
* Fallible APIs: `try_add_node`, `try_add_edge`, `try_update_edge`.
* Algorithm error types: cycle errors, negative-cycle errors, graph-specific errors.
* When to use `Option` accessors vs indexing syntax.
* Production best practices: validate indices, prefer fallible APIs on untrusted input, wrap domain errors.

---

## 28) Testing and verification patterns

* Golden tests for graph construction.
* Property tests for traversal and algorithm invariants.
* Snapshotting DOT output.
* Testing stable-index behavior.
* Testing serialization round trips.
* Cross-checking algorithms on tiny known graphs.
* Fuzzing imported edge lists / DOT inputs.
* Benchmark tests for graph size and density thresholds.

---

## 29) Common recipes and “copy-paste” implementation patterns

* Build a graph from domain records.
* Maintain `HashMap<ExternalId, NodeIndex>`.
* Run Dijkstra and reconstruct paths.
* Run topological sort for dependency ordering.
* Detect cycles before accepting an edge.
* Compute SCCs and build condensation graph.
* Create an MST graph from `min_spanning_tree`.
* Export styled DOT.
* Serialize and deserialize a `StableGraph`.
* Use `GraphMap` for enum/integer node IDs.
* Use CSR for static sparse traversal.

---

## 30) Best-practice decision tables

* Which graph type should I use?
* Which shortest-path algorithm should I use?
* Should I store domain objects directly as weights?
* Should I use `GraphMap` or `Graph + HashMap`?
* Should I choose `Graph` or `StableGraph`?
* When should I enable `serde-1`, `rayon`, `dot_parser`, `generate`, or `unstable`?
* What APIs are safe for untrusted data?

---

## Suggested deep-dive order

Start with **Sections 0–4** to lock down the mental model, installation, graph type selection, and core `Graph` syntax. Then move to **Sections 11–15** for indexing, traversal, traits, and shortest paths. After that, expand the algorithm catalog and finish with performance, serialization, DOT, and production best practices.

[1]: https://docs.rs/petgraph/ "petgraph - Rust"
[2]: https://docs.rs/petgraph/latest/petgraph/ "petgraph - Rust"
[3]: https://docs.rs/petgraph/latest/petgraph/graph/struct.Graph.html "Graph in petgraph::graph - Rust"
[4]: https://docs.rs/petgraph/latest/petgraph/stable_graph/struct.StableGraph.html "StableGraph in petgraph::stable_graph - Rust"
[5]: https://docs.rs/petgraph/latest/petgraph/graphmap/struct.GraphMap.html "GraphMap in petgraph::graphmap - Rust"
[6]: https://docs.rs/petgraph/latest/petgraph/matrix_graph/struct.MatrixGraph.html "MatrixGraph in petgraph::matrix_graph - Rust"
[7]: https://docs.rs/petgraph/latest/petgraph/csr/struct.Csr.html "Csr in petgraph::csr - Rust"
[8]: https://docs.rs/petgraph/latest/petgraph/visit/index.html "petgraph::visit - Rust"
[9]: https://docs.rs/petgraph/latest/petgraph/algo/index.html "petgraph::algo - Rust"
[10]: https://docs.rs/petgraph/latest/petgraph/operator/index.html "petgraph::operator - Rust"

# 0) Scope, versioning, and the petgraph mental model

## 0.1 Scope contract

`petgraph` is a **Rust graph data-structure + graph-algorithm crate**, not a graph database, not a query engine, and not a Python-style exploratory graph environment. Its core scope is:

```text
in-memory graph topology
+ typed node weights
+ typed edge weights
+ directed / undirected edge semantics
+ node / edge indices
+ traversal traits
+ reusable algorithms
+ Graphviz DOT output/import support
```

The public crate docs define petgraph as a graph data-structure library with multiple graph types, graph algorithms, and Graphviz output support; nodes and edges can carry arbitrary associated data, and edges can be directed or undirected. ([Docs.rs][1])

Agent rule: treat petgraph as a **typed graph kernel** embedded inside Rust applications. Do not model it as a service, storage engine, graph query planner, Cypher/Gremlin executor, OLAP engine, or distributed graph runtime.

---

## 0.2 Versioning anchor: released crate vs trunk

Use the **released docs.rs version** as the default implementation target. The current docs.rs page is for **petgraph 0.8.3**, and the GitHub README says the `trunk` branch is transitioning to a new multi-crate layout/new architecture, while the previous release is available under the `0.8` branch. ([Docs.rs][2]) ([GitHub][3])

Recommended agent dependency stance:

```toml
[dependencies]
petgraph = "0.8.3"
```

Avoid this unless intentionally tracking unreleased architecture changes:

```toml
# Avoid for production docs/code unless explicitly requested.
petgraph = { git = "https://github.com/petgraph/petgraph" }
```

Documentation rule:

```text
Default target: petgraph 0.8.x released API.
Mention trunk only as future/development architecture.
Never assume trunk APIs are stable unless user pins a git revision.
```

---

## 0.3 Crate value proposition

Petgraph’s value case is **zero-runtime, strongly typed, in-process graph modeling**:

```text
Rust-native ownership model
+ compile-time node/edge payload types
+ multiple topology representations
+ reusable graph algorithms
+ trait-driven algorithm compatibility
+ no external service dependency
+ no query language runtime
+ no Python interpreter / GIL / object-dict graph overhead
```

The README positions petgraph as “fast, flexible graph data structures and algorithms in Rust,” supporting directed and undirected graphs with arbitrary node and edge data; it also highlights multiple graph types, built-in/extensible algorithms, and DOT/Graphviz visualization support. ([GitHub][3])

---

## 0.4 Core graph vocabulary

Petgraph graph state is conceptually:

```rust
Graph<N, E, Ty, Ix>
```

Where:

```text
N  = node weight type
E  = edge weight type
Ty = edge direction type marker: Directed | Undirected
Ix = integer index storage type, default u32
```

The `Graph` docs define `Graph<N, E, Ty, Ix>` as an adjacency-list graph; `N` and `E` are associated node/edge data called weights, `Ty` determines directedness, and `Ix` determines maximum graph size. ([Docs.rs][4])

Minimal canonical syntax:

```rust
use petgraph::Graph;
use petgraph::graph::{NodeIndex, EdgeIndex};

let mut g: Graph<&'static str, u32> = Graph::new();

let a: NodeIndex = g.add_node("A");
let b: NodeIndex = g.add_node("B");

let e: EdgeIndex = g.add_edge(a, b, 7);

assert_eq!(g[a], "A");
assert_eq!(g[e], 7);
assert_eq!(g.node_count(), 2);
assert_eq!(g.edge_count(), 1);
```

Interpretation:

```text
"A" = node weight
7   = edge weight
a   = opaque node handle into this graph
b   = opaque node handle into this graph
e   = opaque edge handle into this graph
```

---

## 0.5 “Weight” means associated data, not necessarily cost

In petgraph, **weight** means payload/associated data stored on a node or edge. It does **not** automatically mean graph-theoretic numeric edge cost. The docs explicitly call `N` and `E` associated data / weights and say the associated data can be arbitrary type. ([Docs.rs][4])

Use `()` for topology-only graphs:

```rust
use petgraph::graph::UnGraph;

let g = UnGraph::<(), ()>::from_edges([
    (0, 1),
    (1, 2),
    (2, 0),
]);
```

Use domain structs for semantic payloads:

```rust
use petgraph::Graph;

#[derive(Debug, Clone)]
struct Service {
    name: String,
    tier: String,
}

#[derive(Debug, Clone)]
struct Dependency {
    protocol: String,
    latency_ms: u32,
}

let mut g: Graph<Service, Dependency> = Graph::new();

let api = g.add_node(Service {
    name: "api".into(),
    tier: "frontend".into(),
});

let db = g.add_node(Service {
    name: "postgres".into(),
    tier: "storage".into(),
});

g.add_edge(api, db, Dependency {
    protocol: "tcp".into(),
    latency_ms: 4,
});
```

When an algorithm needs a numeric cost, you provide a closure that extracts/converts edge weights:

```rust
use petgraph::algo::dijkstra;

let distances = dijkstra(&g, api, Some(db), |edge_ref| {
    edge_ref.weight().latency_ms
});
```

Agent rule:

```text
Do not infer E: numeric.
Do not assume edge_weight == algorithmic distance.
Do not overload “weight” in docs without clarifying payload vs cost.
```

---

## 0.6 Directionality model

Directedness is a **type-level graph property**, not a per-edge runtime flag:

```rust
use petgraph::{Graph, Directed, Undirected};

type Di = Graph<&'static str, (), Directed>;
type Un = Graph<&'static str, (), Undirected>;

let directed: Di = Graph::new();
let undirected: Un = Graph::new_undirected();
```

For `Graph`, `Graph::new()` constructs a directed graph; `Graph::new_undirected()` constructs an undirected graph. ([Docs.rs][4])

Directed neighbor semantics:

```text
neighbors(a) on Directed   => outgoing neighbors
neighbors(a) on Undirected => all adjacent neighbors
neighbors_directed(a, Incoming | Outgoing) => explicit direction-aware traversal
```

The docs define `neighbors` and `neighbors_directed` with exactly these directed/undirected semantics. ([Docs.rs][4])

Agent rule:

```text
If graph semantics require asymmetric dependency, ownership, control-flow, one-way routing:
    use Directed.
If graph semantics require symmetric adjacency, friendship, undirected roads, physical links:
    use Undirected.
Do not simulate undirected graphs by inserting reciprocal directed edges unless algorithm semantics require directed multiedges.
```

---

## 0.7 Index mental model

For `Graph`, nodes and edges live behind compact indices:

```rust
use petgraph::graph::{NodeIndex, EdgeIndex};

let n: NodeIndex = g.add_node("payload");
let e: EdgeIndex = g.add_edge(n, n, ());
```

The `Graph` docs state that node/edge indices are compact intervals; for `n` nodes, node indices range from `0` to `n - 1`. Adding nodes/edges keeps indices stable, but removing nodes/edges may shift other indices because the last node/edge can move into the removed slot. ([Docs.rs][4])

Index safety rules:

```text
Graph:
    add_node/add_edge       => existing indices stable
    remove_node/remove_edge => some other index may be invalidated/shifted

StableGraph:
    removals do not invalidate unrelated node/edge indices

GraphMap:
    node identity is the node value itself, not NodeIndex

MatrixGraph:
    index-space constrained by matrix representation

Csr:
    optimized static sparse adjacency representation
```

Use `StableGraph` when storing node handles outside the graph across removals. Its docs state that unrelated node/edge indices are not invalidated when items are removed. ([Docs.rs][5])

Agent rule:

```text
If indices escape the graph object and graph topology mutates by deletion:
    prefer StableGraph
else if graph is append-only / rebuilt per phase:
    Graph is usually simpler/faster
```

---

## 0.8 Primary graph representations

Petgraph is not one graph type; it is a family of graph representations with different invariants.

### `Graph<N, E, Ty, Ix>`

Adjacency-list default workhorse:

```rust
use petgraph::Graph;

let mut g = Graph::<&str, i32>::new();
```

Properties:

```text
compact NodeIndex / EdgeIndex
parallel edges allowed
fast insertion
O(|V| + |E|) space
removal may shift indices
best default for mutable sparse graphs
```

The `Graph` docs state O(|V| + |E|) space, fast node/edge insertion, efficient search/algorithms, local edge lookup/removal cost, and parallel-edge allowance for `add_edge`. ([Docs.rs][4])

### `StableGraph<N, E, Ty, Ix>`

Adjacency-list graph with stable unrelated indices:

```rust
use petgraph::stable_graph::StableGraph;

let mut g = StableGraph::<&str, ()>::new();
```

Use when:

```text
external code stores NodeIndex / EdgeIndex
nodes/edges are deleted
index stability matters more than compactness
```

### `GraphMap<N, E, Ty>`

Associative graph where node values are node IDs:

```rust
use petgraph::graphmap::DiGraphMap;

let mut g = DiGraphMap::<&str, i32>::new();
g.add_edge("api", "db", 4);
```

Properties:

```text
node value N is identifier
N: Copy + Eq + Hash + Ord
constant-time edge-existence testing
no parallel edges
self-loops allowed
nodes auto-inserted by add_edge
```

The `GraphMap` docs state that it uses an associative array of node weights, requires `N` to be copyable/hashable/orderable as a node identifier, allows constant-time edge-existence checks, disallows parallel edges, and allows self-loops. ([Docs.rs][6])

### `MatrixGraph<N, E, Ty, Null, Ix>`

Adjacency-matrix graph:

```rust
use petgraph::matrix_graph::DiMatrix;

let mut g = DiMatrix::<&str, i32>::with_capacity(4);
```

Use when:

```text
graph is dense
edge existence lookup dominates
node count is bounded
O(V²) storage is acceptable
```

The `matrix_graph` module describes `MatrixGraph` as backed by an adjacency matrix and includes `NotZero` / `Nullable` machinery for edge-null representation. ([Docs.rs][7])

---

## 0.9 Trait mental model: algorithms target capabilities, not always concrete types

Petgraph’s `visit` module is the core abstraction layer for generic graph algorithms:

```rust
use petgraph::visit::{
    GraphBase,
    IntoNeighbors,
    IntoEdges,
    IntoNodeIdentifiers,
    Visitable,
};
```

The `visit` docs describe graph traits and traversals; `IntoNeighbors`-style traits produce iterators using the `IntoIterator` pattern, and basic visitors such as `Dfs`, `Bfs`, `DfsPostOrder`, and `Topo` use walker methods that borrow the graph only for each `.next()` call. ([Docs.rs][8])

Minimal generic traversal shape:

```rust
use petgraph::visit::{IntoNeighbors, Visitable};

fn reachable_count<G>(graph: G, start: G::NodeId) -> usize
where
    G: IntoNeighbors + Visitable,
    G::NodeId: Copy,
{
    let mut dfs = petgraph::visit::Dfs::new(graph, start);
    let mut count = 0;

    while let Some(_nx) = dfs.next(graph) {
        count += 1;
    }

    count
}
```

Algorithm design rule:

```text
Prefer trait bounds for reusable library code.
Prefer concrete Graph / StableGraph APIs for app-local mutation-heavy code.
Check each algorithm signature; not every algorithm is fully trait-generic.
```

The `algo` module docs state that most petgraph algorithms live there, simple DFS/BFS live in `visit`, and a stated goal is gradual migration toward graph-trait-based algorithms; some algorithms still require `Graph`. ([Docs.rs][2])

---

## 0.10 DOT / Graphviz mental model

Petgraph includes debug-oriented Graphviz DOT rendering:

```rust
use petgraph::dot::{Dot, Config};

println!("{:?}", Dot::with_config(&g, &[Config::EdgeNoLabel]));
```

`Dot` implements output to Graphviz `.dot` format for graph types implementing `IntoEdgeReferences + IntoNodeReferences`; its docs warn formatting/options are simple, intended mostly for debugging, and exact output may change. ([Docs.rs][9])

Agent rule:

```text
Use DOT for debug visualization, docs, diagnostics, and CI artifacts.
Do not treat exact DOT string formatting as a stable wire protocol.
For golden tests, normalize DOT or assert semantic graph structure instead.
```

---

## 0.11 What petgraph is not

### Not a graph database

A graph database manages persisted graph records, query execution, indexing, transactions, and graph query languages. For example, Neo4j documents a property-graph database model with nodes and relationships as database entities. ([Graph Database & Analytics][10])

Petgraph provides:

```text
in-memory Rust collections
typed node/edge payloads
algorithms
DOT output
```

Petgraph does not provide:

```text
persistent storage engine
ACID transactions
Cypher / GQL / Gremlin query language
query planner
secondary indexes over node properties
multi-user concurrency model
server runtime
auth / clustering / replication
```

Deployment implication:

```text
Use petgraph inside an app/service.
Use a graph DB when graph persistence + graph query language + transactional multi-user access are primary requirements.
```

### Not NetworkX

NetworkX is a Python package for creation, manipulation, and study of complex networks. ([networkx.org][11])

Petgraph differs operationally:

```text
petgraph:
    Rust
    static payload types
    explicit indices / trait bounds
    compiled binary/library dependency
    performance-oriented in-process graph kernel

NetworkX:
    Python
    dynamic object model
    exploratory/data-science ergonomics
    broad algorithm/IO ecosystem
    excellent notebook/prototyping UX
```

Migration heuristic:

```text
Prototype graph idea in NetworkX if Python/notebook iteration dominates.
Implement production Rust graph core in petgraph if static typing, embedding, latency, memory control, or Rust integration dominates.
```

### Not a domain-specific graph engine

Temporal graph engines, graph ML frameworks, graph stream processors, RDF triple stores, and graph databases encode domain semantics above “nodes + edges.” Raphtory, for example, describes itself as an open-source temporal graph analytics engine for building, querying, and analyzing time-evolving graphs in Python. ([Pometry Docs][12])

Petgraph does not natively model:

```text
time windows
valid-time / transaction-time edge histories
property graph labels
RDF triples
SPARQL
GraphQL graph APIs
distributed graph partitions
GPU graph kernels
GNN tensors
incremental materialized views
```

Use petgraph when the domain layer can be modeled by your own Rust structs:

```rust
#[derive(Clone, Debug)]
struct TemporalEdge {
    valid_from_ms: i64,
    valid_to_ms: Option<i64>,
    relation: &'static str,
}
```

Use a domain engine when those semantics are the platform’s core responsibility.

---

## 0.12 Agent-ready “mental model table”

| Concept                 | Petgraph interpretation              | Syntax anchor                | Common mistake                                     |
| ----------------------- | ------------------------------------ | ---------------------------- | -------------------------------------------------- |
| Node                    | Vertex slot in graph topology        | `let n = g.add_node(weight)` | Treating node weight as stable ID in `Graph`       |
| Edge                    | Connection between node handles      | `g.add_edge(a, b, weight)`   | Assuming no parallel edges in `Graph`              |
| Node weight             | Arbitrary node payload               | `Graph<NodeData, EdgeData>`  | Assuming numeric “weight”                          |
| Edge weight             | Arbitrary edge payload               | `edge_ref.weight()`          | Assuming cost unless algorithm closure says so     |
| Directedness            | Type-level graph property            | `Graph<N,E,Directed>`        | Mixing directed/undirected semantics at runtime    |
| Node ID                 | Usually `NodeIndex<Ix>`              | `NodeIndex::new(i)`          | Reusing across graph instances blindly             |
| Stable ID               | Use `StableGraph` or external ID map | `StableGraph<N,E>`           | Holding `Graph` indices across deletion            |
| Algorithm compatibility | Trait-bound-dependent                | `IntoNeighbors + Visitable`  | Assuming every graph type supports every algorithm |
| Visualization           | DOT debug output                     | `Dot::with_config(...)`      | Treating exact DOT text as stable API              |

---

## 0.13 Baseline imports for agents

```rust
use petgraph::{
    Directed,
    Undirected,
    Direction,
    Graph,
};

use petgraph::graph::{
    NodeIndex,
    EdgeIndex,
    DiGraph,
    UnGraph,
};

use petgraph::stable_graph::{
    StableGraph,
    StableDiGraph,
    StableUnGraph,
};

use petgraph::graphmap::{
    DiGraphMap,
    UnGraphMap,
};

use petgraph::algo::{
    dijkstra,
    astar,
    toposort,
    kosaraju_scc,
    min_spanning_tree,
};

use petgraph::visit::{
    Dfs,
    Bfs,
    IntoNeighbors,
    Visitable,
};

use petgraph::dot::{
    Dot,
    Config,
};
```

---

## 0.14 Baseline modeling patterns

### Pattern A: topology-only graph

```rust
use petgraph::graph::UnGraph;

let g = UnGraph::<(), ()>::from_edges([
    (0, 1),
    (1, 2),
    (2, 0),
]);
```

Use for:

```text
reachability
connectivity
cycle detection
MST over unit weights
simple graph algorithms
```

### Pattern B: domain payload graph

```rust
use petgraph::Graph;

#[derive(Clone, Debug)]
struct Package {
    name: String,
    version: String,
}

#[derive(Clone, Debug)]
struct Requires {
    semver_req: String,
}

let mut deps: Graph<Package, Requires> = Graph::new();
```

Use for:

```text
dependency graphs
workflow DAGs
compiler IR graphs
service dependency maps
asset lineage
```

### Pattern C: external ID map + `Graph`

```rust
use std::collections::HashMap;
use petgraph::Graph;
use petgraph::graph::NodeIndex;

let mut g = Graph::<String, ()>::new();
let mut ids: HashMap<String, NodeIndex> = HashMap::new();

fn intern(
    g: &mut Graph<String, ()>,
    ids: &mut HashMap<String, NodeIndex>,
    id: &str,
) -> NodeIndex {
    if let Some(&ix) = ids.get(id) {
        ix
    } else {
        let ix = g.add_node(id.to_owned());
        ids.insert(id.to_owned(), ix);
        ix
    }
}
```

Use for:

```text
large non-Copy IDs
string IDs
database primary keys
UUIDs
domain IDs needing compact NodeIndex for algorithms
```

### Pattern D: `GraphMap` for copyable node IDs

```rust
use petgraph::graphmap::DiGraphMap;

let mut g = DiGraphMap::<u64, u32>::new();

g.add_edge(10, 20, 5);
g.add_edge(20, 30, 3);

assert!(g.contains_edge(10, 20));
```

Use for:

```text
u32/u64 IDs
small enums
copyable interned IDs
constant-time edge lookup
no parallel edges
```

---

## 0.15 Production stance summary

```text
Pin petgraph version.
Default to released 0.8.x docs.
Pick graph representation first.
Treat weights as arbitrary payloads.
Separate domain IDs from NodeIndex unless using GraphMap.
Choose StableGraph when deletion + escaped indices exist.
Use trait bounds for reusable algorithms.
Use DOT for diagnostics, not stable serialization.
Use graph DB / temporal engine / NetworkX only when their platform semantics dominate.
```

[1]: https://docs.rs/petgraph/?utm_source=chatgpt.com "petgraph - Rust"
[2]: https://docs.rs/petgraph/latest/petgraph/algo/index.html "petgraph::algo - Rust"
[3]: https://github.com/petgraph/petgraph "GitHub - petgraph/petgraph: Graph data structure library for Rust. · GitHub"
[4]: https://docs.rs/petgraph/latest/petgraph/graph/struct.Graph.html "Graph in petgraph::graph - Rust"
[5]: https://docs.rs/petgraph/latest/petgraph/stable_graph/struct.StableGraph.html "StableGraph in petgraph::stable_graph - Rust"
[6]: https://docs.rs/petgraph/latest/petgraph/graphmap/struct.GraphMap.html "GraphMap in petgraph::graphmap - Rust"
[7]: https://docs.rs/petgraph/latest/petgraph/matrix_graph/index.html "petgraph::matrix_graph - Rust"
[8]: https://docs.rs/petgraph/latest/petgraph/visit/index.html "petgraph::visit - Rust"
[9]: https://docs.rs/petgraph/latest/petgraph/dot/struct.Dot.html "Dot in petgraph::dot - Rust"
[10]: https://neo4j.com/docs/getting-started/appendix/graphdb-concepts/?utm_source=chatgpt.com "Graph database concepts - Getting Started"
[11]: https://networkx.org/?utm_source=chatgpt.com "NetworkX — NetworkX documentation"
[12]: https://www.raphtory.com/?utm_source=chatgpt.com "Raphtory Documentation | Pometry Docs"


# 2) Graph type decision guide — petgraph

Format follows the uploaded advanced-reference style. 

Version target: **petgraph 0.8.3** released API. Petgraph exposes five concrete graph families: `Graph`, `StableGraph`, `GraphMap`, `MatrixGraph`, and `Csr`; the crate docs explicitly frame them as different memory-layout / index-stability / lookup-speed tradeoffs. ([Docs.rs][1])

---

## 2.0 Decision primitive

Choose graph representation by answering, in order:

```text
1. Are node identities external values or internal handles?
2. Will node/edge indices escape the graph and survive deletions?
3. Is topology sparse, dense, or static sparse?
4. Are parallel edges required?
5. Is edge-existence lookup hot?
6. Is mutation frequent after construction?
7. Are algorithms needed that require specific trait impls / concrete Graph support?
8. Is memory footprint dominated by topology, edge weights, node weights, or index width?
```

Core selection:

```text
Default mutable sparse graph                    => Graph
Deletion + long-lived NodeIndex / EdgeIndex     => StableGraph
Copy/hashable node IDs as graph identity         => GraphMap
Dense graph / fast adjacency matrix semantics   => MatrixGraph
Mostly static sparse graph / fast row traversal  => Csr
```

---

## 2.1 Representation matrix

| Requirement                             |                           `Graph` |                       `StableGraph` |                           `GraphMap` |                       `MatrixGraph` |                                               `Csr` |
| --------------------------------------- | --------------------------------: | ----------------------------------: | -----------------------------------: | ----------------------------------: | --------------------------------------------------: |
| Sparse graph                            |                         Excellent |                           Excellent |                                 Good |                   Poor unless dense |                                           Excellent |
| Dense graph                             |                        Acceptable |                          Acceptable |                           Acceptable |                           Excellent |                                 Usually not primary |
| Mutable construction                    |                         Excellent |                           Excellent |                                 Good |                                Good |                    Limited / construction-sensitive |
| Frequent deletion                       |        Compact but index-shifting |                                Best |                          Good by key |                             Depends |                                            Poor fit |
| Stable unrelated indices after deletion |                                No |                                 Yes |                    N/A, key identity |                Not primary contract |                                          Static-ish |
| Parallel edges                          |                               Yes |       Yes-like adjacency-list model |                                   No |  No practical matrix duplicate edge |                                                  No |
| Self-loops                              |                               Yes |                                 Yes |                                  Yes | Yes / representation-dependent APIs |                                                 Yes |
| Edge-existence hot path                 |              Local adjacency scan |                Local adjacency scan |         Constant-time edge existence |            Matrix-style fast lookup | Good outgoing traversal, not general mutable lookup |
| External node IDs                       | Use side `HashMap<ID, NodeIndex>` |   Use side `HashMap<ID, NodeIndex>` |                               Native |              Use side map / indices |                                         Use indices |
| Memory                                  |                        `O(V + E)` | `O(V + E)` plus gaps after deletion | `O(V + E)` plus hash/key duplication |                             `O(V²)` |                                          `O(V + E)` |
| Best value case                         |                 General workhorse |                      Stable handles |                  Keyed simple graphs |                     Dense adjacency |                             Static sparse traversal |

Petgraph documents `Graph` and `StableGraph` as adjacency-list graphs using `O(|V| + |E|)` space; `GraphMap` uses a combined adjacency-list / sparse-adjacency-matrix representation with constant-time edge-existence testing; `MatrixGraph` uses `O(|V²|)` space; and `Csr` uses `O(|V| + |E|)` space with fast outgoing-edge iteration. ([Docs.rs][2])

---

## 2.2 `Graph<N, E, Ty, Ix>` — general adjacency-list graph

### Type shape

```rust
use petgraph::Graph;
use petgraph::{Directed, Undirected};
use petgraph::graph::{DiGraph, UnGraph, NodeIndex, EdgeIndex};

type G1 = Graph<&'static str, u32>;                 // Directed, default Ix
type G2 = Graph<&'static str, u32, Directed>;       // explicit directed
type G3 = Graph<&'static str, u32, Undirected>;     // explicit undirected
type G4 = Graph<&'static str, u32, Directed, u16>;  // smaller index type

type DG = DiGraph<&'static str, u32>;
type UG = UnGraph<&'static str, u32>;
```

`Graph<N, E, Ty, Ix>` is an adjacency-list graph; `N` and `E` are arbitrary node/edge weights, `Ty` chooses directed vs undirected edges, and `Ix` sets the graph index integer type / maximum size. ([Docs.rs][2])

### Core syntax

```rust
use petgraph::Graph;

let mut g = Graph::<&str, u32>::new();

let a = g.add_node("api");
let b = g.add_node("db");

let e = g.add_edge(a, b, 5);

assert_eq!(g[a], "api");
assert_eq!(g[e], 5);
assert!(g.contains_edge(a, b));
```

### Mutation semantics

```rust
let e1 = g.add_edge(a, b, 10);       // always inserts new edge
let e2 = g.add_edge(a, b, 20);       // parallel edge allowed

let e3 = g.update_edge(a, b, 30);    // update existing edge if present; insert otherwise
```

`Graph::add_edge` is `O(1)` and allows parallel duplicate edges; use `update_edge` to avoid duplicate logical edges between the same endpoints. `update_edge` performs local adjacency work proportional to the relevant connected edge count. ([Docs.rs][2])

### Index contract

```text
Graph index contract:
    add_node/add_edge       => existing indices remain stable
    remove_node/remove_edge => may shift another node/edge into removed slot
```

`Graph` keeps compact node/edge index intervals; removals may shift the last node/edge into the removed slot, while additions keep indices stable. ([Docs.rs][2])

Bad long-lived-index pattern:

```rust
// Bad if graph may delete nodes after this handle escapes.
let user_handle = g.add_node("user");
// later: g.remove_node(...)
// user_handle might no longer identify the original semantic node.
```

Safe deployment patterns:

```text
Append-only phase graph                     => Graph
Build graph, run algorithms, discard         => Graph
Need multiedges / duplicate relationships    => Graph
Need maximum method/API coverage             => Graph
Need compact indices after deletion          => Graph, but rebuild external maps after removals
```

### External-ID pattern

```rust
use std::collections::HashMap;
use petgraph::Graph;
use petgraph::graph::NodeIndex;

struct GraphStore {
    graph: Graph<String, ()>,
    by_id: HashMap<String, NodeIndex>,
}

impl GraphStore {
    fn intern(&mut self, id: &str) -> NodeIndex {
        if let Some(&ix) = self.by_id.get(id) {
            ix
        } else {
            let ix = self.graph.add_node(id.to_owned());
            self.by_id.insert(id.to_owned(), ix);
            ix
        }
    }

    fn add_dep(&mut self, from: &str, to: &str) {
        let a = self.intern(from);
        let b = self.intern(to);
        self.graph.update_edge(a, b, ());
    }
}
```

Advisory:

```text
Use Graph + HashMap<ID, NodeIndex> when:
    ID is String / UUID / non-Copy domain object
    algorithms prefer compact NodeIndex
    parallel edges may matter
    stable deletion is not required, or maps are rebuilt after deletion
```

---

## 2.3 `StableGraph<N, E, Ty, Ix>` — adjacency list with stable unrelated indices

### Type shape

```rust
use petgraph::stable_graph::{StableGraph, StableDiGraph, StableUnGraph};
use petgraph::{Directed, Undirected};

type SG1 = StableGraph<&'static str, u32>;
type SG2 = StableGraph<&'static str, u32, Directed>;
type SG3 = StableGraph<&'static str, u32, Undirected, u32>;

type SDG = StableDiGraph<&'static str, u32>;
type SUG = StableUnGraph<&'static str, u32>;
```

`StableGraph` is adjacency-list based and does not invalidate unrelated node/edge indices when items are removed; deletion creates gaps because not every index in the numeric interval remains valid. ([Docs.rs][3])

### Core syntax

```rust
use petgraph::stable_graph::StableGraph;

let mut g = StableGraph::<&str, ()>::new();

let a = g.add_node("a");
let b = g.add_node("b");
let c = g.add_node("c");

g.add_edge(a, b, ());
g.remove_node(b);

// a and c remain valid handles to their original nodes.
// b is invalid/removed.
assert_eq!(g.node_weight(a), Some(&"a"));
assert_eq!(g.node_weight(c), Some(&"c"));
assert_eq!(g.node_weight(b), None);
```

### Deployment value case

```text
StableGraph solves:
    UI selection handles
    persisted in-memory handles
    incremental graph editing
    long-lived references in side tables
    deletion-heavy workflows where semantic handles must survive
```

### Tradeoff contract

```text
StableGraph gains:
    stable unrelated NodeIndex / EdgeIndex across removals

StableGraph pays:
    possible gaps in index space
    possible extra memory after deletions
    some feature/method parity lag versus Graph
```

The petgraph docs explicitly say stable graphs avoid index invalidation across removals, while noting possible extra memory usage and that `StableGraph` is still missing some methods compared with `Graph`. ([Docs.rs][3])

### Correct iteration over valid nodes

Do not assume `0..node_bound` is all valid after deletion.

```rust
use petgraph::visit::IntoNodeIdentifiers;

for node in g.node_identifiers() {
    println!("{:?} => {:?}", node, g.node_weight(node));
}
```

Agent rule:

```text
StableGraph:
    valid nodes     => iterate graph APIs
    invalid holes   => expected after deletions
    raw integer loop => unsafe assumption unless checked with node_weight/is_valid-like APIs
```

---

## 2.4 `GraphMap<N, E, Ty>` — node identifiers are node values

### Type shape

```rust
use petgraph::graphmap::{GraphMap, DiGraphMap, UnGraphMap};
use petgraph::{Directed, Undirected};

type GM1 = GraphMap<u64, u32, Directed>;
type GM2 = GraphMap<u64, u32, Undirected>;

type DGM = DiGraphMap<u64, u32>;
type UGM = UnGraphMap<u64, u32>;
```

`GraphMap` uses node weights as identifiers in an associative-array-backed representation; node type `N` must be `Copy`, hash/equality-capable, and orderable, because it is duplicated in the data structure and used as the hash-table key. ([Docs.rs][4])

### Core syntax

```rust
use petgraph::graphmap::DiGraphMap;

let mut g = DiGraphMap::<u64, u32>::new();

g.add_edge(10, 20, 5);      // nodes 10 and 20 inserted/known by value
g.add_edge(20, 30, 7);

assert!(g.contains_node(10));
assert!(g.contains_edge(10, 20));
assert_eq!(g.edge_weight(10, 20), Some(&5));
```

### Identity model

```text
Graph:
    semantic node id != NodeIndex
    NodeIndex assigned internally
    side map often needed

GraphMap:
    semantic node id == N
    no standalone NodeIndex identity required by user
    graph APIs work directly with node keys
```

### Hard constraints

```text
N requirements:
    Copy
    Eq
    Hash
    Ord

Good N:
    u32
    u64
    usize
    small enum
    interned symbol ID
    newtype around integer

Bad N:
    String
    Vec<_>
    large struct
    non-Copy domain payload
```

### Parallel-edge contract

`GraphMap` disallows parallel edges but allows self-loops. Converting from `Graph` to `GraphMap` can merge nodes with the same weight and retain only the last parallel edge, losing original node/edge indices. ([Docs.rs][4])

### Safe newtype pattern

```rust
use petgraph::graphmap::DiGraphMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct PackageId(u32);

let mut deps = DiGraphMap::<PackageId, ()>::new();

let a = PackageId(1);
let b = PackageId(2);

deps.add_edge(a, b, ());
```

Deployment advisory:

```text
Use GraphMap when:
    node IDs are already compact Copy keys
    no parallel edges required
    edge existence checks are hot
    you want direct contains_edge(a, b) by domain key
    algorithm trait coverage needed by GraphMap is sufficient

Avoid GraphMap when:
    node payload is large / non-Copy
    duplicate/parallel edges carry distinct meaning
    you need stable EdgeIndex handles
    you need separate node payload and node identity
```

---

## 2.5 `MatrixGraph<N, E, S, Ty, Null, Ix>` — dense adjacency matrix

### Type shape

```rust
use petgraph::matrix_graph::{MatrixGraph, DiMatrix, UnMatrix};
use petgraph::{Directed, Undirected};

type M1 = MatrixGraph<&'static str, u32>; // defaults include Directed-ish aliases/module defaults
type DM = DiMatrix<&'static str, u32>;
type UM = UnMatrix<&'static str, u32>;
```

Actual struct shape:

```rust
pub struct MatrixGraph<N, E, S = RandomState, Ty = Directed, Null = Option<E>, Ix = u16>
```

`MatrixGraph` is backed by an adjacency matrix; `Null` represents edge absence and defaults to `Option<E>`, while `NotZero<E>` can use a sentinel-style value for missing edges. It uses `O(|V²|)` space, provides fast edge insertion and amortized node insertion, stores undirected graphs using only the lower triangular matrix, and recommends boxing large edge weights because the backing array stores edge weights. ([Docs.rs][5])

### Dense-graph value case

```text
MatrixGraph optimizes:
    dense edge presence
    predictable adjacency matrix storage
    fast edge lookup/insert by pair
    graph algorithms where dense topology dominates

MatrixGraph penalizes:
    sparse topology
    large node count
    large E payload stored per matrix slot/null wrapper
```

### Core syntax

```rust
use petgraph::matrix_graph::DiMatrix;

let mut g = DiMatrix::<&str, u32>::with_capacity(4);

let a = g.add_node("a");
let b = g.add_node("b");

g.add_edge(a, b, 42);

assert_eq!(g.edge_weight(a, b), Some(&42));
```

### Large edge-weight advisory

Prefer indirection for heavy edge payloads:

```rust
use petgraph::matrix_graph::DiMatrix;

#[derive(Debug)]
struct LargeEdgePayload {
    bytes: Vec<u8>,
    metadata: String,
}

let mut g = DiMatrix::<&str, Box<LargeEdgePayload>>::with_capacity(512);
```

Rule:

```text
If E is large and V² storage exists:
    Box<E> / Arc<E> / external edge table
else:
    matrix slots can dominate memory catastrophically
```

### Density heuristic

```text
Let V = node count
Let E = edge count
Directed full matrix slots ≈ V * V
Undirected stored slots ≈ V * (V + 1) / 2
Adjacency-list slots ≈ V + E

Use MatrixGraph when:
    E is a large fraction of V²
    edge-existence lookup is dominant
    V is bounded and reasonably small
    memory budget includes matrix capacity
```

---

## 2.6 `Csr<N, E, Ty, Ix>` — compressed sparse row graph

### Type shape

```rust
use petgraph::csr::Csr;
use petgraph::{Directed, Undirected};

type C1 = Csr<(), ()>;
type C2 = Csr<&'static str, u32, Directed>;
type C3 = Csr<&'static str, u32, Undirected, u32>;
```

`Csr` is a compressed sparse row / sparse adjacency matrix graph with arbitrary node and edge weights; it uses `O(|V| + |E|)` space, allows self-loops, disallows parallel edges, and provides fast outgoing-edge iteration. ([Docs.rs][6])

### Construction syntax: fixed node count

```rust
use petgraph::csr::Csr;

let g = Csr::<u8, ()>::with_nodes(5);

assert_eq!(g.node_count(), 5);
assert_eq!(g.edge_count(), 0);
assert_eq!(g[0], 0);
```

`Csr::with_nodes(n)` creates `n` nodes and requires `N: Default` so each node receives a default weight. ([Docs.rs][6])

### Construction syntax: sorted unique edges

```rust
use petgraph::csr::Csr;
use petgraph::prelude::*;

let edges = [
    (0.into(), 1.into(), 10),
    (0.into(), 2.into(), 20),
    (1.into(), 2.into(), 30),
];

let g = Csr::<(), i32>::from_sorted_edges(&edges)
    .expect("edges must be sorted and unique by (u, v)");
```

`Csr::from_sorted_edges` requires edges sorted and unique by endpoint pair `(u, v)` order. ([Docs.rs][6])

### Deployment value case

```text
Use Csr when:
    graph is sparse
    topology is mostly static after construction
    outgoing-neighbor scans dominate
    graph is imported from sorted edge list
    compact traversal representation matters

Avoid Csr when:
    arbitrary insertion/deletion dominates
    parallel edges required
    unsorted streaming edge ingestion must remain incremental
    many algorithms require mutation-heavy Graph APIs
```

---

## 2.7 Sparse vs dense decision

### Sparse default

```text
E << V²  => Graph / StableGraph / GraphMap / Csr
```

Use adjacency-list/CSR family when most possible node pairs do not have edges.

Recommended defaults:

```text
Sparse + mutable + internal indices     => Graph
Sparse + mutable + stable handles       => StableGraph
Sparse + keyed + simple edges           => GraphMap
Sparse + static + row traversal         => Csr
```

### Dense default

```text
E ≈ V²  => MatrixGraph
```

Use matrix family when most node pairs are edges or edge-existence lookup by pair dominates.

Anti-pattern:

```text
MatrixGraph with V = 1_000_000
    => conceptual V² slots
    => impossible memory profile for ordinary deployments
```

---

## 2.8 Stable indices vs compact indices

### Compact index semantics: `Graph`

```rust
use petgraph::graph::Graph;

let mut g = Graph::<&str, ()>::new();
let a = g.add_node("a");
let b = g.add_node("b");
let c = g.add_node("c");

g.remove_node(b);

// Some node may move into b's numeric slot.
// Do not assume c's old index still points to "c".
```

`Graph` removals can invalidate the last node/edge index by moving it into the removed slot. ([Docs.rs][2])

### Stable unrelated index semantics: `StableGraph`

```rust
use petgraph::stable_graph::StableGraph;

let mut g = StableGraph::<&str, ()>::new();
let a = g.add_node("a");
let b = g.add_node("b");
let c = g.add_node("c");

g.remove_node(b);

assert_eq!(g.node_weight(a), Some(&"a"));
assert_eq!(g.node_weight(c), Some(&"c"));
assert_eq!(g.node_weight(b), None);
```

Decision:

```text
Index escapes + deletion exists:
    StableGraph

Index local to algorithm/build phase:
    Graph

Need dense valid index range:
    Graph, or rebuild/remap StableGraph

Need semantic ID not index:
    GraphMap or external map
```

---

## 2.9 External IDs vs internal `NodeIndex`

### `Graph + side map`

Use when domain ID is not `Copy` or node payload differs from identity.

```rust
use std::collections::HashMap;
use petgraph::Graph;
use petgraph::graph::NodeIndex;

#[derive(Debug)]
struct NodePayload {
    display_name: String,
    kind: String,
}

type Id = String;

struct Model {
    g: Graph<NodePayload, ()>,
    by_id: HashMap<Id, NodeIndex>,
}
```

Pros:

```text
separates identity from payload
supports String/UUID IDs
supports parallel edges
supports rich node weights
better algorithm compatibility
```

Cons:

```text
must maintain side map
Graph deletion may invalidate side map unless StableGraph or remap
```

### `GraphMap`

Use when ID is the node.

```rust
use petgraph::graphmap::UnGraphMap;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct UserId(u64);

let mut follows = UnGraphMap::<UserId, ()>::new();
```

Pros:

```text
direct domain-key APIs
constant-time edge-existence testing
no NodeIndex side map
simple sparse graph model
```

Cons:

```text
N must be Copy + Eq + Hash + Ord
node data cannot be large non-Copy payload
no parallel edges
node/edge index semantics not the same as Graph
```

---

## 2.10 Parallel edges vs simple graph semantics

| Need                                                   | Choose                                                    |
| ------------------------------------------------------ | --------------------------------------------------------- |
| Multiple distinct relationships between same endpoints | `Graph` / usually `StableGraph`                           |
| At most one edge per endpoint pair                     | `GraphMap`, `MatrixGraph`, `Csr`, or `Graph::update_edge` |
| Convert multigraph to simple graph                     | Use `update_edge` or aggregate edge weights               |
| Preserve every edge event                              | Avoid `GraphMap` / `Csr` / matrix duplicate collapse      |

`Graph::add_edge` permits parallel duplicate edges; `GraphMap` and `Csr` explicitly disallow parallel edges, and `GraphMap::from_graph` warns that duplicate node weights merge and only the last parallel edge is kept. ([Docs.rs][2])

Aggregation pattern:

```rust
use petgraph::Graph;
use petgraph::graph::NodeIndex;

fn add_or_accumulate(
    g: &mut Graph<&str, u32>,
    a: NodeIndex,
    b: NodeIndex,
    delta: u32,
) {
    if let Some(e) = g.find_edge(a, b) {
        *g.edge_weight_mut(e).unwrap() += delta;
    } else {
        g.add_edge(a, b, delta);
    }
}
```

---

## 2.11 Mutation frequency guide

### Build once, query many

```text
Recommended:
    Csr         if sparse + outgoing scans
    MatrixGraph if dense + edge lookup
    Graph       if algorithm coverage / construction simplicity dominates
```

### Frequent insertions

```text
Recommended:
    Graph
    StableGraph
    GraphMap for keyed simple graphs
```

### Frequent deletions

```text
Recommended:
    StableGraph if handles escape
    GraphMap if keyed simple graph and deletions by key
    Graph if no external index persistence and compactness desirable
```

### Streaming unsorted edges

```text
Recommended:
    Graph + side map
    GraphMap if keys are Copy and no parallel edges
Avoid:
    Csr::from_sorted_edges unless you sort/deduplicate batch first
```

---

## 2.12 Memory-footprint guide

### Index width

`Ix` controls index integer type for index-based graph types; docs list allowed values `u8`, `u16`, `u32`, and `usize`, with `u32` default. ([Docs.rs][1])

```rust
use petgraph::Graph;
use petgraph::Directed;

type Small = Graph<(), (), Directed, u16>;
type Huge = Graph<(), (), Directed, usize>;
```

Guidance:

```text
u16:
    small graphs
    memory-sensitive embedded / dense metadata
    hard upper bound acceptable

u32:
    default
    most production graphs

usize:
    very large graphs
    platform-word-size overhead accepted
```

### Weight footprint

```text
N large:
    Graph/StableGraph store one node weight per node
    GraphMap duplicates node key N in multiple places; avoid large N
    MatrixGraph stores edge-slot backing array; avoid large E directly

E large:
    Graph/StableGraph store one per edge
    MatrixGraph may allocate many nullable edge slots
    Csr stores one per actual edge
```

Large-payload strategy:

```rust
use std::sync::Arc;
use petgraph::Graph;

#[derive(Debug)]
struct BigNode {
    blob: Vec<u8>,
}

let mut g = Graph::<Arc<BigNode>, u32>::new();
```

External-store strategy:

```rust
use petgraph::Graph;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct NodeRowId(u32);

let mut g = Graph::<NodeRowId, u32>::new();
// actual payloads stored in Vec<BigNode> / database / arena
```

---

## 2.13 Feature-gating / deployment checklist

Default petgraph features enable `graphmap`, `stable_graph`, `matrix_graph`, and `std`; optional features include `serde-1`, `rayon`, `dot_parser`, `unstable`, and `generate`, with `rayon` requiring `std`. ([Docs.rs][1])

### Full standard deployment

```toml
[dependencies]
petgraph = "0.8.3"
```

### Serialization deployment

```toml
[dependencies]
petgraph = { version = "0.8.3", features = ["serde-1"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

Serialization support applies to `Graph`, `StableGraph`, and `GraphMap` under `serde-1`. ([Docs.rs][1])

### Parallel deployment

```toml
[dependencies]
petgraph = { version = "0.8.3", features = ["rayon"] }
rayon = "1"
```

### `no_std`-leaning deployment

```toml
[dependencies]
petgraph = { version = "0.8.3", default-features = false }
```

Disabling `std` enables `no_std` contexts, but default graph-family features are also disabled unless explicitly re-enabled. ([Docs.rs][1])

---

## 2.14 Agent selection algorithm

```text
function choose_petgraph_type(req):
    if req.node_identity == "copy_hash_ord_key" and !req.parallel_edges:
        if req.edge_existence_lookup_hot or req.wants_keyed_api:
            return GraphMap

    if req.density == "dense":
        if req.node_count_bounded and memory_ok(V^2, E_size):
            return MatrixGraph

    if req.topology == "static_sparse":
        if req.outgoing_iteration_hot and sorted_unique_edges_available:
            return Csr

    if req.indices_escape and req.deletions_possible:
        return StableGraph

    return Graph
```

Secondary overrides:

```text
Need parallel edges:
    force Graph / StableGraph

Need compact valid index interval:
    prefer Graph; avoid StableGraph after deletions

Need no duplicate edge per pair:
    GraphMap / MatrixGraph / Csr, or Graph::update_edge

Need large non-Copy domain node IDs:
    Graph/StableGraph + HashMap<ID, NodeIndex>

Need maximum method and algorithm compatibility:
    start with Graph

Need deletion + external handles:
    start with StableGraph
```

---

## 2.15 Copy-paste type aliases for large codebases

```rust
use petgraph::{Directed, Undirected};
use petgraph::graph::Graph;
use petgraph::stable_graph::StableGraph;
use petgraph::graphmap::GraphMap;
use petgraph::matrix_graph::MatrixGraph;
use petgraph::csr::Csr;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct NodeId(pub u32);

#[derive(Clone, Debug)]
pub struct NodeData {
    pub label: String,
}

#[derive(Clone, Debug)]
pub struct EdgeData {
    pub kind: String,
    pub cost: u32,
}

// General sparse directed graph.
pub type WorkGraph = Graph<NodeData, EdgeData, Directed, u32>;

// Stable-handle sparse graph.
pub type EditingGraph = StableGraph<NodeData, EdgeData, Directed, u32>;

// Keyed simple graph.
pub type KeyGraph = GraphMap<NodeId, EdgeData, Directed>;

// Dense graph; consider Box<EdgeData> if EdgeData is large.
pub type DenseGraph = MatrixGraph<NodeData, EdgeData>;

// Static sparse graph.
pub type StaticSparseGraph = Csr<NodeData, EdgeData, Directed, u32>;
```

---

## 2.16 Anti-pattern inventory

```text
Anti-pattern:
    GraphMap<String, E>
Reason:
    GraphMap requires Copy node keys; use Graph + HashMap<String, NodeIndex>.

Anti-pattern:
    Store Graph NodeIndex in database, then delete nodes over time.
Reason:
    Graph removals can shift indices; use StableGraph or stable external IDs.

Anti-pattern:
    MatrixGraph for sparse million-node topology.
Reason:
    O(V²) matrix memory.

Anti-pattern:
    Csr for online unsorted edge insertion/deletion.
Reason:
    CSR is optimized for sparse row traversal, not arbitrary mutable graph editing.

Anti-pattern:
    Convert Graph with duplicate node weights / parallel edges to GraphMap blindly.
Reason:
    GraphMap conversion can merge duplicate node weights and keep only the last parallel edge.

Anti-pattern:
    Treat edge weight as algorithm cost automatically.
Reason:
    algorithms usually take cost closures; E is arbitrary payload.
```

---

## 2.17 Final decision table

| Workload                                           | Best first choice                | Why                                                          |
| -------------------------------------------------- | -------------------------------- | ------------------------------------------------------------ |
| Dependency DAG, build once, topo/Dijkstra/SCC      | `Graph`                          | Default API coverage, sparse, simple mutation                |
| Interactive editor with deletions and selections   | `StableGraph`                    | Stable unrelated handles across removals                     |
| Social graph keyed by `UserId(u64)`, no multiedges | `GraphMap`                       | Node key is identity, hot `contains_edge`                    |
| Dense similarity matrix / complete weighted graph  | `MatrixGraph`                    | Matrix representation matches topology                       |
| Imported sorted sparse edge list, traversal-heavy  | `Csr`                            | Compact sparse row layout, fast outgoing iteration           |
| Multigraph event stream                            | `Graph`                          | Parallel edges preserved                                     |
| Large UUID node IDs + algorithms                   | `Graph`/`StableGraph` + side map | Avoid `GraphMap` key constraints; compact indices            |
| Deletion-heavy graph + external references         | `StableGraph`                    | Avoid index shifting                                         |
| Sparse graph + memory-sensitive static analysis    | `Csr` or `Graph<u16/u32>`        | Pick CSR for static traversal; shrink index width if bounded |

[1]: https://docs.rs/petgraph/latest/petgraph/ "petgraph - Rust"
[2]: https://docs.rs/petgraph/latest/petgraph/graph/struct.Graph.html "Graph in petgraph::graph - Rust"
[3]: https://docs.rs/petgraph/latest/petgraph/stable_graph/struct.StableGraph.html "StableGraph in petgraph::stable_graph - Rust"
[4]: https://docs.rs/petgraph/latest/petgraph/graphmap/struct.GraphMap.html "GraphMap in petgraph::graphmap - Rust"
[5]: https://docs.rs/petgraph/latest/petgraph/matrix_graph/struct.MatrixGraph.html "MatrixGraph in petgraph::matrix_graph - Rust"
[6]: https://docs.rs/petgraph/latest/petgraph/csr/struct.Csr.html "Csr in petgraph::csr - Rust"


# 3) Core generics and type aliases — petgraph

Target: **petgraph 0.8.3 released API**. Petgraph’s shared generic model is `N`, `E`, `Ty`, and, for index-backed graph types, `Ix`; `N`/`E` are node/edge “weights” meaning arbitrary associated data, `Ty` controls directedness, and `Ix` controls index storage size / graph-size limit. ([Docs.rs][1])

---

## 3.0 Canonical generic signatures

```rust
use petgraph::{Directed, Undirected};
use petgraph::graph::{Graph, NodeIndex, EdgeIndex, DefaultIx};
use petgraph::stable_graph::StableGraph;
use petgraph::graphmap::GraphMap;
use petgraph::matrix_graph::MatrixGraph;
use petgraph::csr::Csr;
```

Core released signatures:

```rust
// Adjacency-list workhorse.
pub struct Graph<N, E, Ty = Directed, Ix = DefaultIx>;

// Stable-index adjacency-list graph.
pub struct StableGraph<N, E, Ty = Directed, Ix = DefaultIx>;

// Keyed graph; no Ix because node ID is N.
pub struct GraphMap<N, E, Ty, S = std::collections::hash_map::RandomState>;

// Dense matrix graph.
pub struct MatrixGraph<
    N,
    E,
    S = std::collections::hash_map::RandomState,
    Ty = Directed,
    Null = Option<E>,
    Ix = u16,
>;

// Compressed sparse row graph.
pub struct Csr<N = (), E = (), Ty = Directed, Ix = DefaultIx>;
```

`Graph` is declared as `Graph<N, E, Ty = Directed, Ix = DefaultIx>`; `GraphMap` adds a hasher parameter `S` and uses `N` as the node identifier; `MatrixGraph` adds hasher `S` plus `Null` edge-presence representation and defaults `Ix` to `u16`; `Csr` defaults node/edge weights to `()` and `Ix` to `DefaultIx`. ([Docs.rs][2])

---

## 3.1 `N`: node weight type

`N` is the **node-associated data type**, not necessarily the node’s stable external ID.

```rust
use petgraph::Graph;

#[derive(Debug, Clone)]
struct Service {
    name: String,
    tier: String,
}

let mut g: Graph<Service, ()> = Graph::new();

let api = g.add_node(Service {
    name: "api".into(),
    tier: "frontend".into(),
});
```

Valid `N` patterns:

```text
Topology-only node             => ()
Small scalar payload            => u32 / i64 / bool
Borrowed static label           => &'static str
Owned domain payload            => struct NodeData { ... }
External storage handle         => NodeRowId(u32)
Shared heavy payload            => Arc<NodeData>
```

`N` and `E` are called “weights” in petgraph and are generally arbitrary associated data; algorithms that need costs commonly accept closures translating weights into algorithm-specific costs. Some graph types impose extra bounds, especially `GraphMap`, where `N` must serve as a hash-table key. ([Docs.rs][1])

### Node payload vs node identity

For `Graph` / `StableGraph`:

```rust
use std::collections::HashMap;
use petgraph::Graph;
use petgraph::graph::NodeIndex;

#[derive(Debug)]
struct NodeData {
    label: String,
}

type ExternalId = String;

struct Model {
    g: Graph<NodeData, ()>,
    by_id: HashMap<ExternalId, NodeIndex>,
}
```

For `GraphMap`:

```rust
use petgraph::graphmap::DiGraphMap;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct UserId(u64);

let mut g = DiGraphMap::<UserId, ()>::new();

g.add_edge(UserId(1), UserId(2), ());
```

Agent rule:

```text
Graph / StableGraph:
    N = payload
    NodeIndex<Ix> = graph-local handle
    external ID usually lives in side map or payload

GraphMap:
    N = payload + identity + key
    N must be Copy + Eq + Hash + Ord
```

`GraphMap` explicitly uses node weights `N` as node identifiers, duplicates them internally, requires suitability as hash keys plus ordering, and does not create standalone node indices. ([Docs.rs][3])

---

## 3.2 `E`: edge weight type

`E` is the **edge-associated data type**, not automatically numeric cost.

```rust
use petgraph::Graph;

#[derive(Debug, Clone)]
struct Dependency {
    protocol: &'static str,
    latency_ms: u32,
}

let mut g: Graph<&'static str, Dependency> = Graph::new();

let api = g.add_node("api");
let db = g.add_node("db");

g.add_edge(api, db, Dependency {
    protocol: "tcp",
    latency_ms: 4,
});
```

Algorithm-cost extraction:

```rust
use petgraph::algo::dijkstra;

let dist = dijkstra(&g, api, Some(db), |edge| {
    edge.weight().latency_ms
});
```

Edge-weight design patterns:

```text
Topology-only edge             => ()
Unit-cost edge                  => ()
Numeric cost directly           => u32 / f64 / ordered wrapper
Domain relationship             => struct EdgeData { kind, cost, flags }
Large payload                   => Box<EdgeData> / Arc<EdgeData> / external ID
Multiple semantic channels      => enum EdgeKind { DependsOn, Calls, Owns }
```

Some algorithms impose weight bounds through their signatures or cost closures; for example, petgraph’s docs call out `min_spanning_tree` requiring edge weights that implement `PartialOrd`. ([Docs.rs][1])

---

## 3.3 `Ty`: directedness type marker

`Ty` is a **type-level directedness marker**:

```rust
use petgraph::{Directed, Undirected};
use petgraph::Graph;

type DepGraph = Graph<&'static str, (), Directed>;
type LinkGraph = Graph<&'static str, (), Undirected>;
```

`Ty` implements petgraph’s `EdgeType` trait:

```rust
pub trait EdgeType {
    fn is_directed() -> bool;
}
```

`Directed` and `Undirected` implement `EdgeType`; the trait determines whether a graph has directed edges. ([Docs.rs][4])

### Constructor syntax

```rust
use petgraph::Graph;

let directed = Graph::<&str, ()>::new();
let undirected = Graph::<&str, ()>::new_undirected();
```

### Type aliases usually preferred

```rust
use petgraph::graph::{DiGraph, UnGraph};

let dg: DiGraph<&str, ()> = DiGraph::new();
let ug: UnGraph<&str, ()> = UnGraph::new_undirected();
```

Agent rule:

```text
Use Directed when:
    dependency graph
    call graph
    control-flow graph
    ownership/lineage graph
    one-way route graph

Use Undirected when:
    symmetric adjacency
    physical connectivity
    friendship-like relation
    co-occurrence graph
```

### Runtime edge direction enum

`Direction` is **not** the graph’s directedness type. It is a runtime query direction used by APIs such as `neighbors_directed`:

```rust
use petgraph::Direction::{Incoming, Outgoing};

let incoming = g.neighbors_directed(db, Incoming);
let outgoing = g.neighbors_directed(api, Outgoing);
```

`Direction` has variants `Outgoing = 0` and `Incoming = 1`; `Outgoing` means outward from the current node and `Incoming` means inbound to the current node. ([Docs.rs][5])

---

## 3.4 `Ix`: index storage type and graph-size limit

`Ix` is the unsigned integer type used inside node/edge index handles.

```rust
use petgraph::Graph;
use petgraph::Directed;

type TinyGraph = Graph<(), (), Directed, u16>;
type DefaultGraph = Graph<(), (), Directed>;      // Ix = DefaultIx = u32
type HugeGraph = Graph<(), (), Directed, usize>;
```

Petgraph exposes `Ix` so users can control node/edge index size and memory footprint; allowed values are `u8`, `u16`, `u32`, and `usize`, with `u32` being the general default. ([Docs.rs][1])

`DefaultIx` is a type alias for `u32`; docs state `u32` is the default to reduce graph data size and improve common-case performance, and `DefaultIx` is used for node/edge indices in `Graph` and `StableGraph` and node indices in `Csr`. ([Docs.rs][6])

### `IndexType` contract

```rust
use petgraph::graph::IndexType;

fn raw_index<Ix: IndexType>(ix: Ix) -> usize {
    ix.index()
}
```

`IndexType` is an unsafe trait for unsigned integer types used for node and edge indices; it requires `new(usize)`, `index() -> usize`, and `max()`, and is implemented for integer types including `u8` and `u16`. ([Docs.rs][7])

Agent rule:

```text
Do not implement IndexType for custom types.
Use built-in Ix choices: u8, u16, u32, usize.
Treat Ix as storage/capacity policy, not semantic ID policy.
```

---

## 3.5 `NodeIndex<Ix>` and `EdgeIndex<Ix>`

### Node index

```rust
use petgraph::graph::{NodeIndex, DefaultIx};

let n0: NodeIndex = NodeIndex::new(0);              // NodeIndex<DefaultIx>
let n1: NodeIndex<DefaultIx> = NodeIndex::new(1);
let raw: usize = n1.index();
```

`NodeIndex<Ix = DefaultIx>` is the node identifier type for index-backed graph families; it has `new`, `index`, and `end` associated functions/methods and implements common traits including `Copy`, `Eq`, `Hash`, `Ord`, and serde traits when enabled. ([Docs.rs][8])

### Edge index

```rust
use petgraph::graph::{EdgeIndex, DefaultIx};

let e0: EdgeIndex = EdgeIndex::new(0);              // EdgeIndex<DefaultIx>
let e1: EdgeIndex<DefaultIx> = EdgeIndex::new(1);
let raw: usize = e1.index();

let sentinel = EdgeIndex::<DefaultIx>::end();
```

`EdgeIndex<Ix = DefaultIx>` is the edge identifier type; it exposes `new`, `index`, and `end`, where `end` is an invalid `EdgeIndex` used internally to denote absence/end of adjacency list. ([Docs.rs][9])

### Type-safety implication

```rust
use petgraph::Graph;
use petgraph::Directed;
use petgraph::graph::NodeIndex;

type G32 = Graph<(), (), Directed, u32>;
type G16 = Graph<(), (), Directed, u16>;

let mut g32: G32 = Graph::new();
let mut g16: G16 = Graph::new();

let a32: NodeIndex<u32> = g32.add_node(());
let a16: NodeIndex<u16> = g16.add_node(());

// a32 and a16 are different Rust types.
// Do not mix NodeIndex<u32> with Graph<_,_,_,u16>.
```

Agent rule:

```text
NodeIndex<Ix> / EdgeIndex<Ix> are graph-local typed handles.
They are not globally meaningful.
They are not portable across graph instances.
They are not stable across Graph deletion.
They must use the same Ix as the graph.
```

---

## 3.6 Shorthand aliases: `Graph`

### Directed graph alias

```rust
use petgraph::graph::DiGraph;

let mut g: DiGraph<&str, u32> = DiGraph::new();
```

Definition:

```rust
pub type DiGraph<N, E, Ix = DefaultIx> = Graph<N, E, Directed, Ix>;
```

`DiGraph` is a `Graph` with directed edges; an edge from `1` to `2` is distinct from an edge from `2` to `1`. ([Docs.rs][10])

### Undirected graph alias

```rust
use petgraph::graph::UnGraph;

let mut g: UnGraph<&str, u32> = UnGraph::new_undirected();
```

Definition:

```rust
pub type UnGraph<N, E, Ix = DefaultIx> = Graph<N, E, Undirected, Ix>;
```

`UnGraph` is a `Graph` with undirected edges; an edge between `1` and `2` is equivalent to an edge between `2` and `1`. ([Docs.rs][11])

### Alias with custom `Ix`

```rust
use petgraph::graph::DiGraph;

type SmallDiGraph<N, E> = DiGraph<N, E, u16>;

let mut g: SmallDiGraph<&str, ()> = DiGraph::new();
```

---

## 3.7 Shorthand aliases: `StableGraph`

### Stable directed graph

```rust
use petgraph::stable_graph::StableDiGraph;

let mut g: StableDiGraph<&str, ()> = StableDiGraph::new();
```

Definition:

```rust
pub type StableDiGraph<N, E, Ix = DefaultIx> =
    StableGraph<N, E, Directed, Ix>;
```

`StableDiGraph` is a `StableGraph` with directed edges; an edge from `1` to `2` is distinct from an edge from `2` to `1`. ([Docs.rs][12])

### Stable undirected graph

```rust
use petgraph::stable_graph::StableUnGraph;

let mut g: StableUnGraph<&str, ()> = StableUnGraph::new_undirected();
```

Definition:

```rust
pub type StableUnGraph<N, E, Ix = DefaultIx> =
    StableGraph<N, E, Undirected, Ix>;
```

`StableUnGraph` is a `StableGraph` with undirected edges; an edge between `1` and `2` is equivalent to an edge between `2` and `1`. ([Docs.rs][13])

---

## 3.8 Shorthand aliases: `GraphMap`

### Directed graph map

```rust
use petgraph::graphmap::DiGraphMap;

let mut g: DiGraphMap<u64, u32> = DiGraphMap::new();
g.add_edge(10, 20, 5);
```

Definition:

```rust
pub type DiGraphMap<N, E, S = std::collections::hash_map::RandomState> =
    GraphMap<N, E, Directed, S>;
```

`DiGraphMap` is a directed `GraphMap`; an edge from `1` to `2` is distinct from an edge from `2` to `1`. ([Docs.rs][14])

### Undirected graph map

```rust
use petgraph::graphmap::UnGraphMap;

let mut g: UnGraphMap<u64, u32> = UnGraphMap::new();
g.add_edge(10, 20, 5);
```

Definition:

```rust
pub type UnGraphMap<N, E, S = std::collections::hash_map::RandomState> =
    GraphMap<N, E, Undirected, S>;
```

`UnGraphMap` is an undirected `GraphMap`; an edge between `1` and `2` is equivalent to an edge between `2` and `1`. ([Docs.rs][15])

### Custom hasher alias

```rust
use std::collections::hash_map::RandomState;
use petgraph::graphmap::DiGraphMap;

type KeyGraph<N, E> = DiGraphMap<N, E, RandomState>;
```

Agent note:

```text
GraphMap has no Ix.
GraphMap index/identity policy is N + hasher S.
Use S only when hash policy is part of performance/security contract.
```

---

## 3.9 Shorthand aliases: matrix graph

### Directed matrix

```rust
use petgraph::matrix_graph::DiMatrix;

let mut g: DiMatrix<&str, u32> = DiMatrix::with_capacity(32);
```

Definition:

```rust
pub type DiMatrix<
    N,
    E,
    S = std::collections::hash_map::RandomState,
    Null = Option<E>,
    Ix = u16,
> = MatrixGraph<N, E, S, Directed, Null, Ix>;
```

`DiMatrix` is a `MatrixGraph` with directed edges. ([Docs.rs][16])

### Undirected matrix

```rust
use petgraph::matrix_graph::UnMatrix;

let mut g: UnMatrix<&str, u32> = UnMatrix::with_capacity(32);
```

Definition:

```rust
pub type UnMatrix<
    N,
    E,
    S = std::collections::hash_map::RandomState,
    Null = Option<E>,
    Ix = u16,
> = MatrixGraph<N, E, S, Undirected, Null, Ix>;
```

`UnMatrix` is a `MatrixGraph` with undirected edges. ([Docs.rs][17])

### Dense-graph payload caution

```rust
use petgraph::matrix_graph::DiMatrix;

#[derive(Debug)]
struct BigEdge {
    bytes: Vec<u8>,
}

type Dense = DiMatrix<&'static str, Box<BigEdge>>;
```

`MatrixGraph` uses an adjacency matrix, stores edge weights in the flattened backing array, uses `O(|V²|)` space, and the docs recommend boxing large edge weights. ([Docs.rs][18])

---

## 3.10 `Csr` aliases and practical type definitions

`Csr` has no common `DiCsr`/`UnCsr` aliases in the public docs section analogous to `DiGraph`; use local aliases:

```rust
use petgraph::csr::Csr;
use petgraph::{Directed, Undirected};
use petgraph::graph::DefaultIx;

type DiCsr<N = (), E = (), Ix = DefaultIx> = Csr<N, E, Directed, Ix>;
type UnCsr<N = (), E = (), Ix = DefaultIx> = Csr<N, E, Undirected, Ix>;
```

`Csr<N = (), E = (), Ty = Directed, Ix = DefaultIx>` is a compressed sparse row graph; its parameters are node/edge weights, edge type, and index type, using `O(|V| + |E|)` space, allowing self-loops, disallowing parallel edges, and optimizing outgoing-edge iteration. ([Docs.rs][19])

---

## 3.11 `Ix` choice guide: `u16`, `u32`, `usize`

### `u16`

Use for bounded small graphs:

```rust
use petgraph::graph::DiGraph;

type SmallGraph<N, E> = DiGraph<N, E, u16>;
```

Value case:

```text
smaller index fields
better cache density
explicit hard size ceiling
good for embedded / many small graphs / matrix graphs
```

Risk:

```text
graph can exceed Ix capacity
fallible APIs or construction validation needed
not safe for unbounded imports
```

### `u32` / `DefaultIx`

Use for ordinary production graphs:

```rust
use petgraph::graph::{DiGraph, DefaultIx};

type AppGraph<N, E> = DiGraph<N, E, DefaultIx>;
```

Value case:

```text
petgraph common default
smaller than usize on 64-bit platforms
large enough for most in-memory graphs
best compatibility with default aliases
```

`DefaultIx = u32`; docs explicitly say this default reduces graph data size and improves common-case performance. ([Docs.rs][6])

### `usize`

Use for very large graphs or host-index alignment:

```rust
use petgraph::graph::Graph;
use petgraph::Directed;

type HugeGraph<N, E> = Graph<N, E, Directed, usize>;
```

Value case:

```text
maximum platform-native index range
interop with usize-heavy APIs
avoid u32 ceiling for extremely large graphs
```

Cost:

```text
larger index storage on 64-bit
worse cache density
not portable as stable serialized layout across 32/64-bit targets
```

### `u8`

Available, but usually only for tiny fixed graphs/tests:

```rust
use petgraph::graph::UnGraph;

type Tiny<N, E> = UnGraph<N, E, u8>;
```

`IndexType` is implemented for `u8` and `u16`, and petgraph’s root docs list `u8`, `u16`, `u32`, and `usize` as allowed `Ix` values. ([Docs.rs][7])

---

## 3.12 Public API stability rule for `Ix`

Bad public API:

```rust
use petgraph::graph::NodeIndex;

pub struct Handle {
    pub node: NodeIndex, // silently means NodeIndex<DefaultIx>
}
```

Better public API:

```rust
use petgraph::graph::{NodeIndex, DefaultIx};

pub type AppIx = DefaultIx;
pub type AppNode = NodeIndex<AppIx>;

pub struct Handle {
    pub node: AppNode,
}
```

Generic public API:

```rust
use petgraph::graph::{IndexType, NodeIndex};

pub struct Handle<Ix: IndexType = petgraph::graph::DefaultIx> {
    pub node: NodeIndex<Ix>,
}
```

Agent rule:

```text
If graph type is public:
    export a project-level Ix alias.

If serialized handles exist:
    avoid usize unless platform dependency is acceptable.

If graph type is internal:
    use DefaultIx unless measured memory pressure or capacity pressure exists.
```

---

## 3.13 Type alias cookbook

### General mutable directed graph

```rust
use petgraph::graph::DiGraph;

#[derive(Clone, Debug)]
pub struct NodeData {
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct EdgeData {
    pub cost: u32,
}

pub type AppGraph = DiGraph<NodeData, EdgeData>;
```

### Stable editing graph

```rust
use petgraph::stable_graph::StableDiGraph;
use petgraph::graph::DefaultIx;

pub type EditGraph<N, E> = StableDiGraph<N, E, DefaultIx>;
```

### Memory-tight graph

```rust
use petgraph::graph::UnGraph;

pub type SmallUndirected<N, E> = UnGraph<N, E, u16>;
```

### Keyed simple graph

```rust
use petgraph::graphmap::DiGraphMap;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SymbolId(pub u32);

pub type SymbolGraph<E> = DiGraphMap<SymbolId, E>;
```

### Dense matrix graph

```rust
use petgraph::matrix_graph::DiMatrix;

pub type DenseCostMatrix<N> = DiMatrix<N, u32>;
pub type DensePayloadMatrix<N, E> = DiMatrix<N, Box<E>>;
```

### Static sparse CSR graph

```rust
use petgraph::csr::Csr;
use petgraph::Directed;

pub type StaticSparse<N, E> = Csr<N, E, Directed>;
```

---

## 3.14 Generic function patterns

### Concrete graph API

```rust
use petgraph::graph::{DiGraph, NodeIndex};

fn add_dep(
    g: &mut DiGraph<String, u32>,
    from: NodeIndex,
    to: NodeIndex,
    cost: u32,
) {
    g.update_edge(from, to, cost);
}
```

Use when:

```text
code is app-local
mutation APIs matter
Graph-specific methods matter
minimal generic complexity desired
```

### Direction-generic but representation-specific

```rust
use petgraph::{Directed, Undirected};
use petgraph::graph::Graph;
use petgraph::EdgeType;

fn node_len<N, E, Ty, Ix>(g: &Graph<N, E, Ty, Ix>) -> usize
where
    Ty: EdgeType,
    Ix: petgraph::graph::IndexType,
{
    g.node_count()
}
```

### Trait-generic traversal

```rust
use petgraph::visit::{IntoNeighbors, Visitable};
use petgraph::visit::Dfs;

fn reachable_count<G>(graph: G, start: G::NodeId) -> usize
where
    G: IntoNeighbors + Visitable,
    G::NodeId: Copy,
{
    let mut dfs = Dfs::new(graph, start);
    let mut count = 0;

    while let Some(_) = dfs.next(graph) {
        count += 1;
    }

    count
}
```

Petgraph’s graph types implement graph/visit traits such as `GraphBase`, `IntoNeighbors`, `IntoEdges`, `NodeIndexable`, `Visitable`, and related traits depending on representation; the docs emphasize algorithms operate on compatible graph traits, while concrete graph types vary in feature coverage. ([Docs.rs][2])

---

## 3.15 Anti-pattern inventory

```text
Anti-pattern:
    Confuse N with NodeIndex.
Fix:
    N is payload/key depending on graph family; NodeIndex<Ix> is graph-local handle.

Anti-pattern:
    Use GraphMap<String, E>.
Fix:
    GraphMap requires Copy node identifiers; use Graph + HashMap<String, NodeIndex>.

Anti-pattern:
    Store NodeIndex without recording Ix.
Fix:
    export AppIx and AppNode aliases.

Anti-pattern:
    Use usize indices for serialized public formats by default.
Fix:
    prefer u32/DefaultIx unless huge graph range required.

Anti-pattern:
    Use MatrixGraph<E = BigStruct> directly.
Fix:
    Box/Arc large edge payloads.

Anti-pattern:
    Treat Directed/Undirected as runtime flags.
Fix:
    choose Ty at type level; use Direction only for incoming/outgoing traversal.

Anti-pattern:
    Assume every algorithm accepts every graph type.
Fix:
    inspect trait bounds / concrete requirements; start with Graph for broadest method coverage.
```

---

## 3.16 Final agent selection rules

```text
N:
    Use () for topology-only.
    Use compact IDs for external stores.
    Use structs for domain payloads.
    Use Copy+Eq+Hash+Ord only when choosing GraphMap.

E:
    Use () for unweighted topology.
    Use numeric when algorithm cost is exactly edge payload.
    Use structs when edge payload is semantic.
    Use Box/Arc/external handles for large payloads, especially MatrixGraph.

Ty:
    Directed for asymmetric relations.
    Undirected for symmetric relations.
    Prefer Di*/Un* aliases over manually spelling Ty in application code.

Ix:
    u16 for bounded small/memory-tight graphs.
    DefaultIx/u32 for normal production.
    usize for extremely large/platform-native graph indexing.
    Keep Ix consistent across Graph, NodeIndex, EdgeIndex, and public handles.

Aliases:
    DiGraph / UnGraph for normal sparse graphs.
    StableDiGraph / StableUnGraph for deletion-stable handles.
    DiGraphMap / UnGraphMap for key-identity simple graphs.
    DiMatrix / UnMatrix for dense graphs.
    Local DiCsr / UnCsr aliases for CSR if needed.
```

[1]: https://docs.rs/petgraph/ "petgraph - Rust"
[2]: https://docs.rs/petgraph/latest/petgraph/graph/struct.Graph.html "Graph in petgraph::graph - Rust"
[3]: https://docs.rs/petgraph/latest/petgraph/graphmap/struct.GraphMap.html "GraphMap in petgraph::graphmap - Rust"
[4]: https://docs.rs/petgraph/latest/petgraph/trait.EdgeType.html "EdgeType in petgraph - Rust"
[5]: https://docs.rs/petgraph/latest/petgraph/enum.Direction.html "Direction in petgraph - Rust"
[6]: https://docs.rs/petgraph/latest/petgraph/graph/type.DefaultIx.html "DefaultIx in petgraph::graph - Rust"
[7]: https://docs.rs/petgraph/latest/petgraph/graph/trait.IndexType.html "IndexType in petgraph::graph - Rust"
[8]: https://docs.rs/petgraph/latest/petgraph/graph/struct.NodeIndex.html "NodeIndex in petgraph::graph - Rust"
[9]: https://docs.rs/petgraph/latest/petgraph/graph/struct.EdgeIndex.html "EdgeIndex in petgraph::graph - Rust"
[10]: https://docs.rs/petgraph/latest/petgraph/graph/type.DiGraph.html "DiGraph in petgraph::graph - Rust"
[11]: https://docs.rs/petgraph/latest/petgraph/graph/type.UnGraph.html "UnGraph in petgraph::graph - Rust"
[12]: https://docs.rs/petgraph/latest/petgraph/stable_graph/type.StableDiGraph.html "StableDiGraph in petgraph::stable_graph - Rust"
[13]: https://docs.rs/petgraph/latest/petgraph/stable_graph/type.StableUnGraph.html "StableUnGraph in petgraph::stable_graph - Rust"
[14]: https://docs.rs/petgraph/latest/petgraph/graphmap/type.DiGraphMap.html "DiGraphMap in petgraph::graphmap - Rust"
[15]: https://docs.rs/petgraph/latest/petgraph/graphmap/type.UnGraphMap.html "UnGraphMap in petgraph::graphmap - Rust"
[16]: https://docs.rs/petgraph/latest/i686-pc-windows-msvc/petgraph/matrix_graph/type.DiMatrix.html "DiMatrix in petgraph::matrix_graph - Rust"
[17]: https://docs.rs/petgraph/latest/petgraph/matrix_graph/type.UnMatrix.html "UnMatrix in petgraph::matrix_graph - Rust"
[18]: https://docs.rs/petgraph/latest/petgraph/matrix_graph/struct.MatrixGraph.html "MatrixGraph in petgraph::matrix_graph - Rust"
[19]: https://docs.rs/petgraph/latest/petgraph/csr/struct.Csr.html "Csr in petgraph::csr - Rust"


# 4) `Graph` deep dive — adjacency-list workhorse

Format follows the uploaded advanced-reference style. 

`Graph<N, E, Ty, Ix>` is petgraph’s default adjacency-list graph: arbitrary node weights `N`, arbitrary edge weights `E`, directedness marker `Ty`, index type `Ix`, `O(|V| + |E|)` topology space, fast node/edge insertion, efficient traversal/algorithm support, and `Send`/`Sync` when `N` and `E` are. ([Docs.rs][1])

---

## 4.0 Type shape

```rust id="9sx37j"
use petgraph::Graph;
use petgraph::{Directed, Undirected};
use petgraph::graph::{DiGraph, UnGraph, NodeIndex, EdgeIndex, DefaultIx};

pub struct Graph<N, E, Ty = Directed, Ix = DefaultIx>;
```

Canonical aliases:

```rust id="vrv067"
type DG<N, E, Ix = DefaultIx> = DiGraph<N, E, Ix>;
type UG<N, E, Ix = DefaultIx> = UnGraph<N, E, Ix>;

type RawDirected<N, E, Ix = DefaultIx> = Graph<N, E, Directed, Ix>;
type RawUndirected<N, E, Ix = DefaultIx> = Graph<N, E, Undirected, Ix>;
```

Use `Graph` when:

```text id="i82ncs"
sparse topology
mutation-heavy construction
parallel edges may matter
compact NodeIndex / EdgeIndex ranges useful
maximum petgraph API/algorithm compatibility desired
indices do not need stability across deletion
```

Avoid plain `Graph` when:

```text id="27bi30"
NodeIndex / EdgeIndex escape graph and must survive deletion  => StableGraph
node key is Copy + Eq + Hash + Ord and no parallel edges       => GraphMap
dense adjacency dominates                                     => MatrixGraph
static sparse row traversal dominates                          => Csr
```

---

## 4.1 Constructors

### `Graph::new()`

Directed graph constructor.

```rust id="pn4x6q"
use petgraph::Graph;

let mut g = Graph::<&str, u32>::new();

let a = g.add_node("api");
let b = g.add_node("db");

g.add_edge(a, b, 5);
```

`Graph::new()` creates a directed graph and is a convenience constructor for `Graph<N, E, Directed>`; the docs recommend `Graph::with_capacity` or `Graph::default` when constructing generically over all graph type parameters. ([Docs.rs][1])

### `Graph::new_undirected()`

Undirected graph constructor.

```rust id="wv6dgj"
use petgraph::Graph;
use petgraph::Undirected;

let mut g = Graph::<&str, u32, Undirected>::new_undirected();

let a = g.add_node("a");
let b = g.add_node("b");

g.add_edge(a, b, 1);
```

Prefer alias form:

```rust id="91pjfx"
use petgraph::graph::UnGraph;

let mut g: UnGraph<&str, u32> = UnGraph::new_undirected();
```

`Graph::new_undirected()` creates an undirected graph and has the same “convenience constructor” caveat as `new()`. ([Docs.rs][1])

### `Graph::with_capacity(nodes, edges)`

Preallocated constructor.

```rust id="0mu9hp"
use petgraph::Graph;

let expected_nodes = 10_000;
let expected_edges = 50_000;

let mut g = Graph::<String, u32>::with_capacity(expected_nodes, expected_edges);

assert!(g.capacity().0 >= expected_nodes);
assert!(g.capacity().1 >= expected_edges);
```

Use for bulk import / known graph size:

```text id="jv54pl"
CSV edge list import
database snapshot import
compiler/module dependency graph
service graph from inventory
large one-shot construction before algorithms
```

`with_capacity(nodes, edges)` creates a graph with estimated node and edge capacity, while `capacity()` returns the current node and edge capacities. ([Docs.rs][1])

### `Graph::from_edges(iterable)`

Construct graph from edge tuples; node weights default; nodes auto-created to cover edge endpoints.

```rust id="6o0vyd"
use petgraph::Graph;

let g = Graph::<(), i32>::from_edges([
    (0, 1),
    (0, 2),
    (1, 2),
]);
```

Weighted edge tuples:

```rust id="xgdbik"
use petgraph::Graph;

let g = Graph::<(), u32>::from_edges([
    (0, 1, 10),
    (0, 2, 20),
    (1, 2, 30),
]);
```

Index-based form after explicit node construction:

```rust id="neqgrg"
use petgraph::Graph;

let mut g = Graph::<(), u32>::new();

let a = g.add_node(());
let b = g.add_node(());
let c = g.add_node(());

g.extend_with_edges([
    (a, b, 10),
    (b, c, 20),
]);
```

`from_edges` and `extend_with_edges` require defaultable node weights, set `N` to defaults for auto-created nodes, accept weighted or default edge-weight inputs, and insert nodes automatically to match edge endpoints. ([Docs.rs][1])

Agent rule:

```text id="70qvea"
Use from_edges when:
    node payload is ()
    node payload has acceptable Default
    endpoint IDs are compact numeric indices
    fast topology bootstrap matters

Avoid from_edges when:
    nodes need rich domain payloads at construction
    external IDs are strings/UUIDs
    edge list is untrusted and Ix limits matter
```

---

## 4.2 Mutation APIs

### `add_node(weight) -> NodeIndex<Ix>`

```rust id="x9e6qe"
use petgraph::Graph;
use petgraph::graph::NodeIndex;

let mut g = Graph::<String, ()>::new();

let n: NodeIndex = g.add_node("api".to_owned());

assert_eq!(&g[n], "api");
```

`add_node` inserts a node in `O(1)`, returns its new `NodeIndex<Ix>`, and panics if the graph has reached the maximum node count for its index type. ([Docs.rs][1])

### `try_add_node(weight) -> Result<NodeIndex<Ix>, GraphError>`

```rust id="8q3hwm"
use petgraph::Graph;

let mut g = Graph::<String, ()>::new();

let n = g
    .try_add_node("api".to_owned())
    .expect("node index capacity exceeded");
```

`try_add_node` is the fallible form; it returns `GraphError::NodeIxLimit` when the graph is at the node limit for its index type. ([Docs.rs][1])

Deployment rule:

```text id="0xzpax"
Use add_node:
    trusted bounded graph
    default u32/usize Ix
    panics treated as programmer error

Use try_add_node:
    untrusted imports
    small Ix such as u8/u16
    service boundary / recoverable failure path
```

### `add_edge(a, b, weight) -> EdgeIndex<Ix>`

```rust id="ftx5sp"
use petgraph::Graph;
use petgraph::graph::EdgeIndex;

let mut g = Graph::<&str, u32>::new();

let a = g.add_node("a");
let b = g.add_node("b");

let e: EdgeIndex = g.add_edge(a, b, 7);

assert_eq!(g[e], 7);
```

`add_edge` inserts an edge in `O(1)`, returns a new `EdgeIndex<Ix>`, panics if either endpoint is invalid, panics if the edge index limit is reached, and explicitly allows parallel duplicate edges. ([Docs.rs][1])

### `try_add_edge(a, b, weight) -> Result<EdgeIndex<Ix>, GraphError>`

```rust id="dnbf7k"
use petgraph::Graph;

let mut g = Graph::<&str, u32>::new();

let a = g.add_node("a");
let b = g.add_node("b");

let e = g
    .try_add_edge(a, b, 7)
    .expect("invalid endpoint or edge index capacity exceeded");
```

`try_add_edge` is the fallible insertion form; possible errors include invalid endpoint nodes and edge-index capacity exhaustion. ([Docs.rs][1])

### `update_edge(a, b, weight) -> EdgeIndex<Ix>`

Duplicate-edge control.

```rust id="c1munq"
use petgraph::Graph;

let mut g = Graph::<&str, u32>::new();

let a = g.add_node("a");
let b = g.add_node("b");

let e1 = g.update_edge(a, b, 10);
let e2 = g.update_edge(a, b, 20);

assert_eq!(e1, e2);
assert_eq!(g[e1], 20);
assert_eq!(g.edge_count(), 1);
```

`update_edge` adds or updates an edge: if an edge from `a` to `b` already exists, its weight is updated; otherwise a new edge is inserted. It returns the affected edge index and runs in `O(e')`, where `e'` is the number of edges connected to `a`, and also to `b` for undirected graphs. ([Docs.rs][1])

Fallible variant:

```rust id="lhsgx4"
let e = g.try_update_edge(a, b, 30)?;
```

`try_update_edge` has the same add-or-update semantics and can fail for invalid endpoints or edge-index capacity exhaustion. ([Docs.rs][1])

Agent rule:

```text id="j3w2g5"
Use add_edge:
    multigraph semantics
    every relationship/event is distinct
    parallel edges carry independent payloads

Use update_edge:
    simple directed/undirected graph semantics
    one logical edge per endpoint pair
    latest value / accumulated value should overwrite existing edge
```

### `remove_node(a) -> Option<N>`

```rust id="l6hgdc"
use petgraph::Graph;

let mut g = Graph::<&str, ()>::new();

let a = g.add_node("a");
let b = g.add_node("b");
let c = g.add_node("c");

let removed = g.remove_node(b);

assert_eq!(removed, Some("b"));
```

`remove_node` removes the node if present and returns its node weight. It invalidates the removed index and may also invalidate the last node index because the last node shifts into the removed slot; edge indices are invalidated as if each incident edge were removed. Complexity is local to affected edges, including incident edges and edges touching the displaced node. ([Docs.rs][1])

### `remove_edge(e) -> Option<E>`

```rust id="l333x4"
use petgraph::Graph;

let mut g = Graph::<&str, u32>::new();

let a = g.add_node("a");
let b = g.add_node("b");
let e = g.add_edge(a, b, 7);

let removed = g.remove_edge(e);

assert_eq!(removed, Some(7));
```

`remove_edge` removes an edge if present and returns its edge weight; besides the removed edge index, it invalidates the last edge index because the last edge moves into the removed slot. ([Docs.rs][1])

---

## 4.3 Accessors

### Safe optional access

```rust id="b4m7iy"
use petgraph::Graph;

let mut g = Graph::<String, u32>::new();

let a = g.add_node("api".to_owned());
let b = g.add_node("db".to_owned());
let e = g.add_edge(a, b, 5);

assert_eq!(g.node_weight(a).map(String::as_str), Some("api"));
assert_eq!(g.edge_weight(e), Some(&5));
```

Mutable safe access:

```rust id="ps3ktk"
if let Some(node) = g.node_weight_mut(a) {
    node.push_str("-v2");
}

if let Some(weight) = g.edge_weight_mut(e) {
    *weight += 1;
}
```

`node_weight` / `node_weight_mut` and `edge_weight` / `edge_weight_mut` return `Option` and return `None` when the given node or edge does not exist. ([Docs.rs][1])

### Indexing syntax

```rust id="pbn5gk"
assert_eq!(&g[a], "api-v2");
assert_eq!(g[e], 6);

g[a] = "api-v3".to_owned();
g[e] = 9;
```

Indexing with `graph[node_index]` accesses node weights; indexing with `graph[edge_index]` accesses edge weights. Indexing panics when the node or edge does not exist. ([Docs.rs][1])

Agent rule:

```text id="tdfk3k"
Use node_weight / edge_weight:
    external/untrusted indices
    indices may be stale after deletion
    production error path required

Use indexing syntax:
    index was just returned by this graph
    deletion has not occurred
    panic is acceptable as invariant violation
```

### Edge endpoint access

```rust id="u5wav0"
if let Some((source, target)) = g.edge_endpoints(e) {
    println!("{:?} -> {:?}", source, target);
}
```

`edge_endpoints(e)` returns the source and target node indices for an edge or `None` if the edge does not exist. ([Docs.rs][1])

### Simultaneous mutable access: `index_twice_mut`

```rust id="cah78x"
use petgraph::{Graph, Incoming};
use petgraph::visit::Dfs;

let mut gr = Graph::<f64, f64>::new();

let a = gr.add_node(0.0);
let b = gr.add_node(0.0);
let c = gr.add_node(0.0);

gr.add_edge(a, b, 3.0);
gr.add_edge(b, c, 2.0);
gr.add_edge(c, b, 1.0);

let mut dfs = Dfs::new(&gr, a);

while let Some(node) = dfs.next(&gr) {
    let mut edges = gr.neighbors_directed(node, Incoming).detach();

    while let Some(edge) = edges.next_edge(&gr) {
        let (node_weight, edge_weight) = gr.index_twice_mut(node, edge);
        *node_weight += *edge_weight;
    }
}
```

`index_twice_mut` indexes the graph by two distinct node/edge indices, returning two mutable references; it panics if the indices are equal or out of bounds. The docs demonstrate this with a detached neighbor walker during DFS. ([Docs.rs][1])

---

## 4.4 Iterators

### `node_indices()`

```rust id="tbzal1"
for n in g.node_indices() {
    println!("{:?}: {:?}", n, g.node_weight(n));
}
```

`node_indices()` returns an iterator over node indices. ([Docs.rs][1])

### `edge_indices()`

```rust id="qfi18e"
for e in g.edge_indices() {
    let (a, b) = g.edge_endpoints(e).unwrap();
    println!("{:?}: {:?} -> {:?}, weight={:?}", e, a, b, g.edge_weight(e));
}
```

`edge_indices()` returns an iterator over edge indices. ([Docs.rs][1])

### `node_weights()`, `node_weights_mut()`

```rust id="awjv78"
for node in g.node_weights() {
    println!("{node:?}");
}

for node in g.node_weights_mut() {
    // mutate node payloads in index order
}
```

Node-weight iterators yield weights in node-index order, with immutable and mutable variants. ([Docs.rs][1])

### `edge_weights()`, `edge_weights_mut()`

```rust id="959i17"
for w in g.edge_weights() {
    println!("{w:?}");
}

for w in g.edge_weights_mut() {
    *w += 1;
}
```

Edge-weight iterators yield weights in edge-index order, with immutable and mutable variants. ([Docs.rs][1])

### `neighbors(a)`

```rust id="11s4ph"
for neighbor in g.neighbors(a) {
    println!("a -> {:?}", neighbor);
}
```

`neighbors(a)` returns `NodeIndex<Ix>` values. On directed graphs it means outgoing neighbors from `a`; on undirected graphs it means all adjacent neighbors. Missing nodes produce an empty iterator. ([Docs.rs][1])

Direction-explicit:

```rust id="urffsj"
use petgraph::Direction::{Incoming, Outgoing};

for src in g.neighbors_directed(a, Incoming) {
    println!("incoming from {:?}", src);
}

for dst in g.neighbors_directed(a, Outgoing) {
    println!("outgoing to {:?}", dst);
}
```

For directed graphs, `neighbors_directed(a, Outgoing)` iterates edges from `a`; `Incoming` iterates edges to `a`; for undirected graphs it is equivalent to all neighbors. Directed neighbor order is reverse insertion order for edges. ([Docs.rs][1])

### `edges(a)`

```rust id="v2n9us"
use petgraph::visit::EdgeRef;

for edge in g.edges(a) {
    println!(
        "{:?} -> {:?}, edge={:?}, weight={:?}",
        edge.source(),
        edge.target(),
        edge.id(),
        edge.weight()
    );
}
```

`edges(a)` returns `EdgeReference<E, Ix>` values; on directed graphs it means outgoing edges from `a`, and on undirected graphs all connected edges. Directed edge order is reverse insertion order. ([Docs.rs][1])

Direction-explicit edge iteration:

```rust id="zywmwe"
use petgraph::Direction::Incoming;
use petgraph::visit::EdgeRef;

for edge in g.edges_directed(a, Incoming) {
    println!("incoming edge {:?}: {:?}", edge.id(), edge.weight());
}
```

`edges_directed` yields `EdgeReference<E, Ix>` and respects directed/undirected semantics documented by `Direction`. ([Docs.rs][1])

### `edge_references()`

```rust id="ugviby"
use petgraph::visit::EdgeRef;

for edge in g.edge_references() {
    println!(
        "edge {:?}: {:?} -> {:?}, weight={:?}",
        edge.id(),
        edge.source(),
        edge.target(),
        edge.weight()
    );
}
```

`edge_references()` iterates all edges in indexed order and yields `EdgeReference<E, Ix>`. ([Docs.rs][1])

---

## 4.5 Parallel-edge behavior

`Graph::add_edge` always inserts a new edge, even if an edge between the same endpoints already exists. Petgraph’s docs explicitly state that `Graph` allows parallel duplicate edges and recommend `update_edge` when duplicates should be avoided. ([Docs.rs][1])

### Multigraph/event semantics

```rust id="v5xtkc"
use petgraph::Graph;

let mut g = Graph::<&str, &'static str>::new();

let a = g.add_node("client");
let b = g.add_node("server");

let e1 = g.add_edge(a, b, "connect");
let e2 = g.add_edge(a, b, "retry");
let e3 = g.add_edge(a, b, "disconnect");

assert_ne!(e1, e2);
assert_ne!(e2, e3);
assert_eq!(g.edge_count(), 3);
```

Use `add_edge` when:

```text id="n8eqwp"
multiple distinct relationships exist
edge payloads are event records
edge multiplicity is semantically meaningful
algorithm must see every parallel edge
```

### Simple-graph overwrite semantics

```rust id="h7ej41"
use petgraph::Graph;

let mut g = Graph::<&str, u32>::new();

let a = g.add_node("a");
let b = g.add_node("b");

g.update_edge(a, b, 1);
g.update_edge(a, b, 2);
g.update_edge(a, b, 3);

assert_eq!(g.edge_count(), 1);
assert_eq!(g.edge_weight(g.find_edge(a, b).unwrap()), Some(&3));
```

Use `update_edge` when:

```text id="8s5rm1"
endpoint pair identifies the relationship
latest value should replace previous value
one edge per pair is required
deduplication cost O(local degree) is acceptable
```

### Accumulating duplicate edge values

```rust id="usf8w6"
use petgraph::Graph;
use petgraph::graph::NodeIndex;

fn add_or_accumulate(
    g: &mut Graph<&str, u32>,
    a: NodeIndex,
    b: NodeIndex,
    delta: u32,
) {
    if let Some(e) = g.find_edge(a, b) {
        *g.edge_weight_mut(e).unwrap() += delta;
    } else {
        g.add_edge(a, b, delta);
    }
}
```

`find_edge` and `contains_edge` use local adjacency work proportional to edges connected to the relevant endpoint(s), so they are not constant-time global pair lookups. ([Docs.rs][1])

---

## 4.6 Index invalidation rules

Graph indices are compact and graph-local. Additions preserve existing indices; removals may shift the last node/edge into the removed slot. The docs state this explicitly for graph indices and explain that compact index intervals simplify algorithms. ([Docs.rs][1])

### Addition-safe handles

```rust id="c9mqtj"
let a = g.add_node("a");
let b = g.add_node("b");

let old_a = a;
let old_b = b;

let c = g.add_node("c");
let e = g.add_edge(a, c, ());

// old_a and old_b still identify the same nodes.
assert_eq!(g.node_weight(old_a), Some(&"a"));
assert_eq!(g.node_weight(old_b), Some(&"b"));
```

### Removal-unsafe handles

```rust id="2shx9m"
let a = g.add_node("a");
let b = g.add_node("b");
let c = g.add_node("c");

g.remove_node(b);

// b is removed.
// c may have moved into b's old numeric slot.
// old c handle may no longer be valid for "c".
```

Agent rule:

```text id="9gn5z3"
Never store Graph NodeIndex / EdgeIndex across deletion unless:
    you can prove deletion cannot affect the handle, or
    you immediately validate with node_weight/edge_weight, or
    you rebuild/remap side tables after deletion.

Use StableGraph when:
    external handles must remain valid across unrelated removals.
```

### Deletion-safe side-map strategy

```rust id="q42wuz"
use std::collections::HashMap;
use petgraph::Graph;
use petgraph::graph::NodeIndex;

fn rebuild_index<N, E>(
    g: &Graph<N, E>,
    key_of: impl Fn(&N) -> String,
) -> HashMap<String, NodeIndex> {
    let mut out = HashMap::new();

    for n in g.node_indices() {
        out.insert(key_of(&g[n]), n);
    }

    out
}
```

Use after any deletion if side maps point into `Graph`.

---

## 4.7 Capacity management

### Initial preallocation

```rust id="8m95vt"
use petgraph::Graph;

let mut g = Graph::<(), ()>::with_capacity(1_000_000, 5_000_000);
```

### Runtime reservation

```rust id="3862v6"
g.reserve_nodes(100_000);
g.reserve_edges(500_000);
```

`reserve_nodes` / `reserve_edges` reserve capacity for at least the additional requested nodes/edges and may reserve more to avoid frequent reallocations; both can panic if capacity overflows `usize`. ([Docs.rs][1])

### Exact reservation

```rust id="kqdweq"
g.reserve_exact_nodes(100_000);
g.reserve_exact_edges(500_000);
```

`reserve_exact_nodes` / `reserve_exact_edges` reserve minimum capacity for exactly the additional requested items and do nothing if current capacity is sufficient; the docs prefer non-exact `reserve_*` when future insertions are expected. ([Docs.rs][1])

### Shrinking

```rust id="irjlsx"
g.shrink_to_fit_nodes();
g.shrink_to_fit_edges();
g.shrink_to_fit();
```

The shrink methods reduce underlying node and/or edge collection capacity as much as possible. ([Docs.rs][1])

### Capacity telemetry

```rust id="ae0rhb"
let (node_cap, edge_cap) = g.capacity();
println!("capacity nodes={node_cap}, edges={edge_cap}");
```

Deployment rules:

```text id="g64e7u"
Bulk import:
    use with_capacity or reserve_* before insertion

Long-running service:
    reserve in batches; avoid exact reservations on hot growth path

Memory reclamation after large temporary graph:
    drop graph entirely if possible
    otherwise shrink_to_fit after deletion/clear phase

Many small graphs:
    choose smaller Ix where safe
    use with_capacity for known tiny upper bounds
```

---

## 4.8 Performance model

Core asymptotics for `Graph`:

```text id="6lkb5b"
Space:
    O(|V| + |E|)

node_count / edge_count:
    O(1)

add_node:
    O(1)

add_edge:
    O(1)

contains_edge / find_edge:
    O(e') local adjacency scan

update_edge:
    O(e') local adjacency scan; inserts if absent

remove_node:
    O(affected local edges); invalidates compact indices

remove_edge:
    O(affected local edge lists); invalidates compact edge index

node_indices / edge_indices:
    linear over compact index ranges

neighbors / edges:
    linear over local degree
```

The docs state `Graph` uses `O(|V| + |E|)` space, offers fast node/edge insert, and uses local edge-count complexity for lookup/removal; `node_count` and `edge_count` are `O(1)`, `add_node` and `add_edge` are `O(1)`, and `update_edge`, `contains_edge`, and `find_edge` depend on a local edge-count measure. ([Docs.rs][1])

Agent performance rules:

```text id="g864cm"
Hot edge-existence lookup by pair:
    Graph may be too slow for high-degree nodes.
    Consider GraphMap or maintain HashMap<(NodeIndex, NodeIndex), EdgeIndex>.

High duplicate-edge insertion throughput:
    add_edge is O(1), update_edge is O(local degree).
    Use add_edge if multiedges are acceptable.

High deletion + escaped handles:
    Graph invalidation cost is semantic hazard.
    Use StableGraph.

Known large import:
    preallocate nodes/edges.

Dense graph:
    Graph edge lookup is local-list-based.
    MatrixGraph may match workload better.
```

---

## 4.9 Construction recipe: domain records → `Graph`

```rust id="f2cmjv"
use std::collections::HashMap;
use petgraph::Graph;
use petgraph::graph::NodeIndex;

#[derive(Clone, Debug)]
struct Service {
    id: String,
    tier: String,
}

#[derive(Clone, Debug)]
struct Calls {
    protocol: String,
    p95_ms: u32,
}

#[derive(Clone, Debug)]
struct CallRecord {
    from: String,
    to: String,
    protocol: String,
    p95_ms: u32,
}

struct ServiceGraph {
    g: Graph<Service, Calls>,
    by_id: HashMap<String, NodeIndex>,
}

impl ServiceGraph {
    fn with_capacity(nodes: usize, edges: usize) -> Self {
        Self {
            g: Graph::with_capacity(nodes, edges),
            by_id: HashMap::with_capacity(nodes),
        }
    }

    fn intern(&mut self, id: &str) -> NodeIndex {
        if let Some(&ix) = self.by_id.get(id) {
            return ix;
        }

        let ix = self.g.add_node(Service {
            id: id.to_owned(),
            tier: "unknown".to_owned(),
        });

        self.by_id.insert(id.to_owned(), ix);
        ix
    }

    fn add_or_update_call(&mut self, rec: CallRecord) {
        let from = self.intern(&rec.from);
        let to = self.intern(&rec.to);

        self.g.update_edge(from, to, Calls {
            protocol: rec.protocol,
            p95_ms: rec.p95_ms,
        });
    }
}
```

Use this pattern when:

```text id="y9tuq7"
domain IDs are String/UUID/non-Copy
payload differs from graph-local identity
one logical edge per pair is desired
bulk import size known
petgraph algorithms need compact NodeIndex
```

---

## 4.10 Traversal while mutating payloads

Detached neighbor walkers avoid borrowing the graph for the entire iterator, enabling mutation patterns during traversal. Petgraph docs explicitly recommend `.neighbors(a).detach()` / `.neighbors_directed(a, dir).detach()` for walkers that do not borrow from the graph. ([Docs.rs][1])

```rust id="yvqlky"
use petgraph::{Graph, Incoming};
use petgraph::visit::Dfs;

let mut g = Graph::<u32, u32>::new();

let a = g.add_node(0);
let b = g.add_node(0);

let e = g.add_edge(a, b, 5);

let mut dfs = Dfs::new(&g, a);

while let Some(node) = dfs.next(&g) {
    let mut incoming = g.neighbors_directed(node, Incoming).detach();

    while let Some(edge) = incoming.next_edge(&g) {
        let (node_w, edge_w) = g.index_twice_mut(node, edge);
        *node_w += *edge_w;
    }
}
```

Agent rule:

```text id="17t5yr"
Iterator borrows graph:
    cannot freely mutate graph topology/payloads inside loop.

Detached walker:
    can query next step using graph each time.

index_twice_mut:
    use for simultaneous mutable node/edge access.
```

---

## 4.11 Error-policy recipes

### Panic-oriented internal graph

```rust id="g739qv"
fn build_internal_graph() -> Graph<&'static str, u32> {
    let mut g = Graph::new();

    let a = g.add_node("a");
    let b = g.add_node("b");

    g.add_edge(a, b, 1);

    g
}
```

Use when invariants are controlled by code.

### Fallible import graph

```rust id="b6h31p"
use petgraph::Graph;
use petgraph::graph::NodeIndex;

fn add_checked_edge<N, E>(
    g: &mut Graph<N, E>,
    a: NodeIndex,
    b: NodeIndex,
    weight: E,
) -> Result<(), petgraph::graph::GraphError> {
    g.try_add_edge(a, b, weight)?;
    Ok(())
}
```

Use when input is external/untrusted or `Ix` is intentionally small.

---

## 4.12 Best-practice deployment checklist

```text id="zjpjo8"
Representation:
    Choose Graph for general mutable sparse graphs.

Construction:
    Use with_capacity for known bulk sizes.
    Use from_edges only when default node weights are acceptable.
    Use side maps for external IDs.

Mutation:
    Use add_edge for multigraph semantics.
    Use update_edge for simple-graph overwrite semantics.
    Use try_* APIs for untrusted input / small Ix.

Access:
    Use Option accessors at boundaries.
    Use indexing only for internal invariants.
    Use edge_endpoints for edge-index to node-index resolution.

Iteration:
    Use node_indices / edge_indices for index-oriented passes.
    Use node_weights / edge_weights for payload-only passes.
    Use neighbors for local topology.
    Use edges / edge_references when edge IDs/endpoints/weights are required.
    Import EdgeRef trait for source/target/id/weight methods.

Index safety:
    Additions keep indices stable.
    Removals may shift last node/edge.
    Rebuild side maps after deletion or use StableGraph.

Performance:
    Preallocate.
    Avoid update_edge/find_edge on very high-degree hot paths if pair lookup dominates.
    Consider GraphMap for constant-time keyed edge existence.
    Consider MatrixGraph for dense topology.
    Consider Csr for static sparse traversal.
```

Final rule:

```text id="id4tp7"
Graph is the default petgraph workhorse:
    adjacency list
    compact indices
    fast insertion
    parallel-edge capable
    broad algorithm support

Graph is not deletion-stable:
    use StableGraph when external handles must survive removals
```

[1]: https://docs.rs/petgraph/latest/petgraph/graph/struct.Graph.html "Graph in petgraph::graph - Rust"


# 5) `StableGraph` deep dive — persistent index use cases

Format follows the uploaded advanced-reference style. 

`StableGraph<N, E, Ty, Ix>` is petgraph’s adjacency-list graph variant for **index-stability under removals**: arbitrary node weights `N`, arbitrary edge weights `E`, directedness marker `Ty`, index type `Ix`, `O(|V| + |E|)` storage, fast insertion, local-cost edge lookup/removal, and `Send`/`Sync` when weights are. The decisive contract: removing a node or edge invalidates that item’s index, but does **not** invalidate unrelated node/edge indices. ([Docs.rs][1])

---

## 5.0 Type shape

```rust id="p6xd8k"
use petgraph::stable_graph::{
    StableGraph,
    StableDiGraph,
    StableUnGraph,
};

use petgraph::{Directed, Undirected};
use petgraph::graph::{NodeIndex, EdgeIndex, DefaultIx};

pub struct StableGraph<N, E, Ty = Directed, Ix = DefaultIx>;

pub type StableDiGraph<N, E, Ix = DefaultIx> =
    StableGraph<N, E, Directed, Ix>;

pub type StableUnGraph<N, E, Ix = DefaultIx> =
    StableGraph<N, E, Undirected, Ix>;
```

`StableDiGraph` is the directed alias and `StableUnGraph` is the undirected alias; the module-level docs state that `stable_graph` depends on the `stable_graph` crate feature, which is part of petgraph’s default feature set in normal installs. ([Docs.rs][2])

Canonical constructors:

```rust id="pm6rxx"
use petgraph::stable_graph::{StableDiGraph, StableUnGraph};

let mut dg: StableDiGraph<&str, u32> = StableDiGraph::new();

let mut ug: StableUnGraph<&str, u32> =
    StableUnGraph::with_capacity(128, 512);
```

---

## 5.1 Stable index contract

### StableGraph removal rule

```text id="7pxb5m"
remove_node(a):
    invalidates node index a
    invalidates incident edge indices
    does not invalidate unrelated node indices

remove_edge(e):
    invalidates edge index e
    does not invalidate unrelated edge indices
```

The docs say `remove_node(a)` invalidates `a` but no other node index; incident edge indices are invalidated as if each incident edge were removed. `remove_edge(e)` invalidates `e` but no other edge index. ([Docs.rs][1])

Concrete demonstration:

```rust id="xnaqow"
use petgraph::stable_graph::StableDiGraph;

let mut g = StableDiGraph::<&str, ()>::new();

let a = g.add_node("a");
let b = g.add_node("b");
let c = g.add_node("c");

let ab = g.add_edge(a, b, ());
let bc = g.add_edge(b, c, ());

assert_eq!(g.node_weight(a), Some(&"a"));
assert_eq!(g.node_weight(b), Some(&"b"));
assert_eq!(g.node_weight(c), Some(&"c"));

let removed_b = g.remove_node(b);

assert_eq!(removed_b, Some("b"));
assert_eq!(g.node_weight(b), None);

// Unrelated node handles still point at the same semantic nodes.
assert_eq!(g.node_weight(a), Some(&"a"));
assert_eq!(g.node_weight(c), Some(&"c"));

// Incident edges are gone.
assert_eq!(g.edge_weight(ab), None);
assert_eq!(g.edge_weight(bc), None);
```

### Meaning of “unrelated indices”

```text id="6pu5jz"
unrelated node index:
    node index not equal to removed node
    still points to same node after removal

unrelated edge index:
    edge index not removed directly
    edge index not incident to a removed node
    still points to same edge after removal
```

Contrast with `Graph`: `Graph` uses compact node/edge index intervals and removals may move the last node/edge into the removed slot, invalidating another index. `StableGraph` intentionally avoids that compaction on removal. ([Docs.rs][3])

---

## 5.2 Holes/gaps after deletions

`StableGraph` keeps node and edge indices in an interval `0..m`, but not all indices in that interval are valid after deletions; gaps are formed by removed items. ([Docs.rs][1])

```rust id="dg45e4"
use petgraph::stable_graph::StableDiGraph;
use petgraph::graph::NodeIndex;

let mut g = StableDiGraph::<&str, ()>::new();

let a = g.add_node("a"); // likely NodeIndex(0)
let b = g.add_node("b"); // likely NodeIndex(1)
let c = g.add_node("c"); // likely NodeIndex(2)

g.remove_node(b);

// Do not treat every integer between 0 and node_bound as valid.
assert_eq!(g.node_weight(a), Some(&"a"));
assert_eq!(g.node_weight(b), None);
assert_eq!(g.node_weight(c), Some(&"c"));
```

Correct traversal:

```rust id="bx3n3n"
for n in g.node_indices() {
    println!("{:?}: {:?}", n, g.node_weight(n).unwrap());
}

for e in g.edge_indices() {
    println!("{:?}: {:?}", e, g.edge_weight(e).unwrap());
}
```

Incorrect compact-index assumption:

```rust id="4xxdu2"
// Bad for StableGraph after deletions:
for raw in 0..g.node_count() {
    let n = NodeIndex::new(raw);
    // raw may not correspond to a valid node.
}
```

`NodeIndexable::node_bound()` returns an upper bound suitable for bitmap sizing, not necessarily the number of live nodes; `NodeCompactIndexable` is the stricter trait saying node identifiers correspond exactly to `0..node_bound()` with no holes. ([Docs.rs][4])

Agent rule:

```text id="zzpjh7"
StableGraph:
    node_count()      = live node count
    edge_count()      = live edge count
    node_bound()      = upper bound over index space
    node_indices()    = valid live node IDs
    edge_indices()    = valid live edge IDs
    NodeIndex::new(i) = just a handle-shaped value, not proof of validity
```

---

## 5.3 Core syntax map

### Construction

```rust id="vy2x0k"
use petgraph::stable_graph::{StableDiGraph, StableUnGraph};

let mut dg = StableDiGraph::<String, u32>::new();

let mut ug = StableUnGraph::<String, u32>::with_capacity(
    10_000, // estimated nodes
    50_000, // estimated edges
);
```

`StableGraph::new()` creates a directed stable graph; `with_capacity(nodes, edges)` creates a graph with estimated node/edge capacity. ([Docs.rs][1])

### Mutation

```rust id="h82xpe"
let a = dg.add_node("api".to_owned());
let b = dg.add_node("db".to_owned());

let e = dg.add_edge(a, b, 5);

dg.update_edge(a, b, 7); // update existing edge if found, insert otherwise
```

`add_node` is `O(1)` and panics at index-capacity exhaustion; `try_add_node` returns `GraphError::NodeIxLimit`. `add_edge` is `O(1)`, panics for missing endpoints or edge-index exhaustion, and allows parallel duplicate edges; `try_add_edge` returns `GraphError::NodeMissed` or `GraphError::EdgeIxLimit`. `update_edge` / `try_update_edge` add-or-update and run in local-degree time. ([Docs.rs][1])

Fallible boundary form:

```rust id="g7ars4"
use petgraph::graph::GraphError;

fn add_checked(
    g: &mut StableDiGraph<String, u32>,
    a: petgraph::graph::NodeIndex,
    b: petgraph::graph::NodeIndex,
    w: u32,
) -> Result<(), GraphError> {
    g.try_add_edge(a, b, w)?;
    Ok(())
}
```

### Accessors

```rust id="z3q2pv"
if let Some(node) = dg.node_weight(a) {
    println!("node = {node}");
}

if let Some(node) = dg.node_weight_mut(a) {
    node.push_str("-v2");
}

if let Some(weight) = dg.edge_weight(e) {
    println!("edge weight = {weight}");
}

if let Some(weight) = dg.edge_weight_mut(e) {
    *weight += 1;
}
```

`node_weight`, `node_weight_mut`, `edge_weight`, and `edge_weight_mut` return `Option`; indexing syntax exists but panics if the node or edge does not exist. ([Docs.rs][1])

Safe boundary rule:

```text id="2vrhlx"
Use Option accessors:
    user-provided indices
    stale handles possible
    deletion has occurred
    handles cross subsystem boundaries

Use indexing syntax:
    handle was just produced by this graph
    deletion did not intervene
    panic means internal invariant violation
```

---

## 5.4 Holes and reusable indices

Stable index does **not** mean monotonically increasing index forever. It means unrelated existing indices are preserved. Removed slots may later be reused for newly inserted nodes/edges, so a stale removed index can become valid again but refer to a new entity.

```rust id="0ejjjk"
use petgraph::stable_graph::StableDiGraph;

let mut g = StableDiGraph::<&str, ()>::new();

let old = g.add_node("old");
g.remove_node(old);

assert_eq!(g.node_weight(old), None);

let new = g.add_node("new");

// Do not rely on old != new.
// A removed slot may be reused.
if old == new {
    assert_eq!(g.node_weight(old), Some(&"new"));
}
```

Agent rule:

```text id="hx5tvv"
StableGraph preserves live unrelated handles.
StableGraph does not make removed handles permanently invalid.
For stale-handle detection across deletion/reinsertion:
    add generation/version IDs in node payload or external handle table.
```

Generational handle pattern:

```rust id="iibzmz"
use petgraph::stable_graph::StableDiGraph;
use petgraph::graph::NodeIndex;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Gen(u64);

#[derive(Clone, Debug)]
struct NodeData {
    generation: Gen,
    label: String,
}

#[derive(Copy, Clone, Debug)]
struct StableHandle {
    ix: NodeIndex,
    generation: Gen,
}

struct Store {
    g: StableDiGraph<NodeData, ()>,
    next_gen: u64,
}

impl Store {
    fn insert(&mut self, label: String) -> StableHandle {
        let gen = Gen(self.next_gen);
        self.next_gen += 1;

        let ix = self.g.add_node(NodeData { generation: gen, label });
        StableHandle { ix, generation: gen }
    }

    fn get(&self, h: StableHandle) -> Option<&NodeData> {
        let node = self.g.node_weight(h.ix)?;
        (node.generation == h.generation).then_some(node)
    }
}
```

Use this pattern when removed node handles can be cached by UI/frontends, background jobs, plugins, or async tasks.

---

## 5.5 Memory and feature-parity tradeoffs

`StableGraph` uses adjacency-list storage and `O(|V| + |E|)` space, but deletions form holes, so capacity/index-bound can exceed live counts; docs also state `StableGraph` is still missing a few methods compared with `Graph`. ([Docs.rs][1])

### Tradeoff matrix

| Dimension                   | `Graph`                              | `StableGraph`                                 |
| --------------------------- | ------------------------------------ | --------------------------------------------- |
| Add node/edge               | Fast                                 | Fast                                          |
| Remove node/edge            | Compacts by moving last item         | Leaves unrelated indices stable               |
| Node index interval         | Compact live interval                | Interval may contain holes                    |
| `NodeCompactIndexable`      | Implemented                          | Not implemented                               |
| Long-lived external handles | Dangerous across deletion            | Primary use case                              |
| Memory after many deletions | Compact after removals               | May retain holes/capacity                     |
| Method parity               | Broadest                             | Mostly similar, but docs note missing methods |
| Conversion                  | Into `StableGraph` preserves indices | Into `Graph` compacts/reindexes               |

`NodeCompactIndexable` requires hole-free `0..node_bound()` node identifiers; the implementor list includes `Graph`, `Csr`, and `GraphMap`, but not `StableGraph`, matching StableGraph’s deletion-hole model. ([Docs.rs][5])

### Memory hygiene

```rust id="e9gjg8"
g.shrink_to_fit_nodes();
g.shrink_to_fit_edges();
g.shrink_to_fit();
```

`StableGraph` exposes capacity, reservation, exact reservation, and shrink methods for node/edge backing storage. ([Docs.rs][1])

Memory advisory:

```text id="1t5dog"
If workload is:
    create many nodes
    delete many nodes
    retain small live set
    never rebuild/compact

Then:
    StableGraph may keep sparse index space/capacity.
    Consider periodic compaction into Graph and rebuilding handles/maps,
    or application-level free-list/generation policy,
    or partitioned graphs per lifecycle epoch.
```

---

## 5.6 When stable indices matter

### External references

```rust id="wuyyiv"
use std::collections::HashMap;
use petgraph::stable_graph::StableDiGraph;
use petgraph::graph::NodeIndex;

struct Model {
    graph: StableDiGraph<String, ()>,
    by_external_id: HashMap<String, NodeIndex>,
}
```

Use when:

```text id="iiw9ah"
external table stores NodeIndex
node is selected in UI
task/job references graph node
edge/node handle crosses API boundary
subsystems mutate graph concurrently by phase/ownership discipline
```

### Long-lived IDs inside app process

```rust id="6pjx0h"
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct WidgetNode(NodeIndex);

struct Selection {
    selected: Option<WidgetNode>,
}
```

`StableGraph` supports handles that survive unrelated deletions; still validate before use because the selected node itself may have been removed.

### UI selections

```rust id="u7li0p"
fn render_selected(
    g: &StableDiGraph<String, ()>,
    selected: Option<NodeIndex>,
) {
    if let Some(ix) = selected {
        match g.node_weight(ix) {
            Some(label) => println!("selected: {label}"),
            None => println!("selection no longer exists"),
        }
    }
}
```

### Incremental algorithms

Use StableGraph when algorithm state caches `NodeIndex` across graph edits:

```text id="v4mh54"
cached reachability sets
incremental topological order
dirty-node queue
layout coordinates keyed by NodeIndex
incremental SCC/cluster metadata
UI graph layout positions
```

Example metadata keyed by stable handles:

```rust id="62hoao"
use std::collections::HashMap;
use petgraph::stable_graph::StableDiGraph;
use petgraph::graph::NodeIndex;

#[derive(Default, Clone, Debug)]
struct Layout {
    x: f32,
    y: f32,
}

let mut graph = StableDiGraph::<String, ()>::new();
let mut layout: HashMap<NodeIndex, Layout> = HashMap::new();

let a = graph.add_node("a".into());
layout.insert(a, Layout { x: 10.0, y: 20.0 });

// unrelated deletions do not corrupt layout[a]
```

---

## 5.7 Parallel edges and duplicate-edge control

Like `Graph`, `StableGraph::add_edge` allows parallel duplicate edges; use `update_edge` to enforce one logical edge per endpoint pair. ([Docs.rs][1])

```rust id="chx82b"
use petgraph::stable_graph::StableDiGraph;

let mut g = StableDiGraph::<&str, u32>::new();

let a = g.add_node("a");
let b = g.add_node("b");

let e1 = g.add_edge(a, b, 1);
let e2 = g.add_edge(a, b, 2);

assert_ne!(e1, e2);
assert_eq!(g.edge_count(), 2);
```

Duplicate-control form:

```rust id="m01zwf"
let e1 = g.update_edge(a, b, 1);
let e2 = g.update_edge(a, b, 2);

assert_eq!(e1, e2);
assert_eq!(g.edge_weight(e1), Some(&2));
```

Agent rule:

```text id="ixt95b"
StableGraph solves index stability.
It does not automatically make graph simple.
Use add_edge for multigraph semantics.
Use update_edge for one-edge-per-pair semantics.
```

---

## 5.8 Migration from `Graph` to `StableGraph`

### Mechanical type-alias migration

Before:

```rust id="hfc7fi"
use petgraph::graph::DiGraph;

pub type AppGraph = DiGraph<NodeData, EdgeData>;
```

After:

```rust id="3rdp34"
use petgraph::stable_graph::StableDiGraph;

pub type AppGraph = StableDiGraph<NodeData, EdgeData>;
```

Node and edge handles remain `NodeIndex<Ix>` / `EdgeIndex<Ix>`, so many call sites continue to compile if they use the alias.

### Direct conversion preserving indices

```rust id="t47qi9"
use petgraph::graph::DiGraph;
use petgraph::stable_graph::StableDiGraph;

let mut g = DiGraph::<&str, u32>::new();

let a = g.add_node("a");
let b = g.add_node("b");
let e = g.add_edge(a, b, 7);

let sg: StableDiGraph<&str, u32> = g.into();

assert_eq!(sg.node_weight(a), Some(&"a"));
assert_eq!(sg.edge_weight(e), Some(&7));
```

`From<Graph<N, E, Ty, Ix>> for StableGraph<N, E, Ty, Ix>` runs in `O(|V| + |E|)` and preserves the same node and edge indices as the original graph. ([Docs.rs][1])

### Compacting back to `Graph`

```rust id="xjtj40"
use petgraph::graph::DiGraph;
use petgraph::stable_graph::StableDiGraph;

let sg = StableDiGraph::<String, ()>::new();

let compact: DiGraph<String, ()> = sg.into();
```

`From<StableGraph<N, E, Ty, Ix>> for Graph<N, E, Ty, Ix>` runs in `O(|V| + |E|)` and compacts node/edge indices into hole-free intervals, so prior `NodeIndex` / `EdgeIndex` side tables must be rebuilt. ([Docs.rs][3])

### Migration checklist

```text id="v9akj0"
1. Replace graph type alias:
       DiGraph<N,E> -> StableDiGraph<N,E>
       UnGraph<N,E> -> StableUnGraph<N,E>

2. Audit loops:
       for i in 0..node_count()
       NodeIndex::new(i)
       Vec indexed by node.index()

3. Replace compact assumptions:
       iterate node_indices()
       use node_bound-sized bitmaps only with validity checks
       use HashMap<NodeIndex, T> or secondary remap for live nodes

4. Audit algorithms:
       remove NodeCompactIndexable bounds unless truly needed
       use NodeIndexable + validity-aware iteration where possible
       convert to Graph for compact-only algorithms if acceptable

5. Audit side maps:
       stale removed handles may be reused
       add generation tokens if clients cache handles

6. Audit deletion-heavy phases:
       decide whether periodic compaction/rebuild is needed
```

---

## 5.9 Pitfall: assuming compact index ranges

Bad:

```rust id="v6z2s0"
use petgraph::graph::NodeIndex;

let mut values = vec![0; g.node_count()];

for i in 0..g.node_count() {
    let n = NodeIndex::new(i);
    values[i] = compute_for_node(&g, n); // invalid if n is a hole
}
```

Good, if output keyed by stable handles:

```rust id="cf3w93"
use std::collections::HashMap;
use petgraph::graph::NodeIndex;

let mut values: HashMap<NodeIndex, u32> = HashMap::new();

for n in g.node_indices() {
    values.insert(n, compute_for_node(&g, n));
}
```

Good, if dense temporary algorithm array required:

```rust id="e3b6x8"
use std::collections::HashMap;
use petgraph::graph::NodeIndex;

let live: Vec<NodeIndex> = g.node_indices().collect();

let dense_id: HashMap<NodeIndex, usize> =
    live.iter()
        .copied()
        .enumerate()
        .map(|(i, n)| (n, i))
        .collect();

let mut values = vec![0u32; live.len()];

for &n in &live {
    let i = dense_id[&n];
    values[i] = compute_for_node(&g, n);
}
```

Agent rule:

```text id="09pj01"
StableGraph NodeIndex.index():
    suitable as stable sparse key
    not suitable as dense Vec offset after deletions unless Vec sized by node_bound and holes handled
```

---

## 5.10 Pitfall: relying on `NodeCompactIndexable`

Bad generic bound for StableGraph-compatible algorithm:

```rust id="ztm88c"
use petgraph::visit::NodeCompactIndexable;

fn compact_only<G>(g: &G)
where
    G: NodeCompactIndexable,
{
    // Assumes NodeId exactly maps to 0..node_bound().
}
```

This excludes `StableGraph` because `NodeCompactIndexable` requires no holes, while StableGraph explicitly permits holes after deletions. ([Docs.rs][5])

More StableGraph-compatible shape:

```rust id="bl2kyw"
use petgraph::visit::{IntoNodeIdentifiers, NodeIndexable};

fn sparse_index_space<G>(g: G)
where
    G: IntoNodeIdentifiers + NodeIndexable,
    G::NodeId: Copy,
{
    // node_bound can size bitmap-like structures,
    // but iterate live nodes via node_identifiers().
    let upper = g.node_bound();

    for n in g.node_identifiers() {
        let raw = g.to_index(n);
        assert!(raw < upper);
    }
}
```

Rule:

```text id="46e78q"
Use NodeCompactIndexable only when:
    algorithm genuinely needs dense live node IDs 0..node_bound()

Use NodeIndexable + IntoNodeIdentifiers when:
    sparse index space with holes is acceptable

Use remapping when:
    algorithm internally needs dense arrays
```

---

## 5.11 Pitfall: deletion-heavy workloads

StableGraph is ideal for preserving handles, but deletion-heavy workloads can create sparse index spaces and free-slot churn.

Risk profile:

```text id="cw6q18"
high creation rate
high deletion rate
long-running process
large side tables keyed by NodeIndex.index()
removed handles cached externally
no compaction window
```

Mitigations:

```text id="w2x1o1"
A. Generation handles:
       detect stale removed-and-reused indices

B. Periodic compaction:
       convert StableGraph -> Graph or rebuild StableGraph
       rebuild every side map / handle table

C. Epoch partitioning:
       create new graph per session/window/batch
       retire old graph wholesale

D. Dense remap per algorithm run:
       live NodeIndex -> dense usize
       output mapped back to NodeIndex

E. Store external IDs in payload:
       recover semantic identity even if handle table rebuilt
```

Compaction recipe:

```rust id="t2l5vb"
use petgraph::graph::DiGraph;
use petgraph::stable_graph::StableDiGraph;
use petgraph::visit::EdgeRef;

#[derive(Clone, Debug)]
struct NodeData {
    id: String,
}

#[derive(Clone, Debug)]
struct EdgeData {
    kind: String,
}

fn compact_rebuild(
    stable: StableDiGraph<NodeData, EdgeData>,
) -> DiGraph<NodeData, EdgeData> {
    // Built-in conversion compacts, but old NodeIndex values are no longer valid.
    let compact: DiGraph<NodeData, EdgeData> = stable.into();
    compact
}
```

Use custom rebuild when you must rebuild external maps at the same time:

```rust id="ci8qfo"
use std::collections::HashMap;
use petgraph::stable_graph::StableDiGraph;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;

#[derive(Clone, Debug)]
struct NodeData {
    id: String,
}

#[derive(Clone, Debug)]
struct EdgeData {
    kind: String,
}

fn compact_with_map(
    old: &StableDiGraph<NodeData, EdgeData>,
) -> (DiGraph<NodeData, EdgeData>, HashMap<String, NodeIndex>) {
    let mut new = DiGraph::<NodeData, EdgeData>::with_capacity(
        old.node_count(),
        old.edge_count(),
    );

    let mut old_to_new = HashMap::<NodeIndex, NodeIndex>::new();
    let mut by_id = HashMap::<String, NodeIndex>::new();

    for old_n in old.node_indices() {
        let payload = old.node_weight(old_n).unwrap().clone();
        let id = payload.id.clone();
        let new_n = new.add_node(payload);

        old_to_new.insert(old_n, new_n);
        by_id.insert(id, new_n);
    }

    for edge in old.edge_references() {
        let a = old_to_new[&edge.source()];
        let b = old_to_new[&edge.target()];
        new.add_edge(a, b, edge.weight().clone());
    }

    (new, by_id)
}
```

---

## 5.12 StableGraph-backed store pattern

```rust id="lg8n2j"
use std::collections::HashMap;
use petgraph::stable_graph::StableDiGraph;
use petgraph::graph::NodeIndex;

#[derive(Clone, Debug)]
struct NodeData {
    id: String,
    generation: u64,
    label: String,
}

#[derive(Clone, Debug)]
struct EdgeData {
    kind: String,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct Handle {
    ix: NodeIndex,
    generation: u64,
}

struct Store {
    graph: StableDiGraph<NodeData, EdgeData>,
    by_id: HashMap<String, Handle>,
    next_generation: u64,
}

impl Store {
    fn new() -> Self {
        Self {
            graph: StableDiGraph::new(),
            by_id: HashMap::new(),
            next_generation: 0,
        }
    }

    fn insert_node(&mut self, id: String, label: String) -> Handle {
        let generation = self.next_generation;
        self.next_generation += 1;

        let ix = self.graph.add_node(NodeData {
            id: id.clone(),
            generation,
            label,
        });

        let handle = Handle { ix, generation };
        self.by_id.insert(id, handle);
        handle
    }

    fn get(&self, handle: Handle) -> Option<&NodeData> {
        let node = self.graph.node_weight(handle.ix)?;
        (node.generation == handle.generation).then_some(node)
    }

    fn remove(&mut self, id: &str) -> Option<NodeData> {
        let handle = self.by_id.remove(id)?;
        let node = self.graph.node_weight(handle.ix)?;

        if node.generation != handle.generation {
            return None;
        }

        self.graph.remove_node(handle.ix)
    }

    fn add_edge_by_id(&mut self, from: &str, to: &str, kind: String) -> Option<()> {
        let from = self.by_id.get(from).copied()?;
        let to = self.by_id.get(to).copied()?;

        self.get(from)?;
        self.get(to)?;

        self.graph.update_edge(from.ix, to.ix, EdgeData { kind });
        Some(())
    }
}
```

Value case:

```text id="p0cxx2"
StableGraph gives:
    NodeIndex stability for live nodes
    cheap node-handle keys for layout/cache/metadata
    safe deletion without global remap on every edit

Generation layer gives:
    stale-handle detection
    removed-slot reuse safety
    API-boundary robustness
```

---

## 5.13 Migration decision table

| Existing `Graph` behavior                    | Migration target                   | Required fix                          |
| -------------------------------------------- | ---------------------------------- | ------------------------------------- |
| Handles never escape; graph rebuilt per run  | Stay `Graph`                       | None                                  |
| Handles stored in UI selection               | `StableGraph`                      | Validate with `node_weight`           |
| Handles stored in side maps across deletion  | `StableGraph`                      | Add generation if stale reuse matters |
| Code uses `for i in 0..node_count()`         | `StableGraph` possible             | Replace with `node_indices()`         |
| Code uses `Vec<T>` indexed by `node.index()` | `StableGraph` risky                | Use `HashMap<NodeIndex,T>` or remap   |
| Generic algo requires `NodeCompactIndexable` | Usually not StableGraph-compatible | Relax bound or remap                  |
| Need compact graph for final algorithm       | Convert to `Graph`                 | Rebuild handle maps                   |
| Deletion rate extremely high                 | Maybe `StableGraph` + compaction   | Add lifecycle strategy                |

---

## 5.14 Production deployment checklist

```text id="v29xbj"
Use StableGraph when:
    NodeIndex / EdgeIndex is stored outside immediate scope
    graph supports deletion
    UI/caches/metadata reference graph elements
    incremental algorithms keep node state
    app cannot afford full side-map rebuild after every deletion

Do not assume:
    node_count == node_bound
    0..node_count maps to valid NodeIndex
    NodeIndex.index() is dense
    removed handles stay invalid forever
    StableGraph implements NodeCompactIndexable
    StableGraph has perfect Graph method parity

Prefer:
    node_indices() / edge_indices()
    node_weight() / edge_weight() at boundaries
    generation tokens for public handles
    HashMap<NodeIndex, T> for sparse metadata
    live-node remapping for dense algorithm arrays
    periodic compaction for long-running deletion-heavy graphs
```

Final rule:

```text id="8z0w90"
Graph optimizes compactness.
StableGraph optimizes handle continuity.
Choose StableGraph only when that continuity is part of the program contract.
```

[1]: https://docs.rs/petgraph/latest/petgraph/stable_graph/struct.StableGraph.html "StableGraph in petgraph::stable_graph - Rust"
[2]: https://docs.rs/petgraph/latest/petgraph/stable_graph/index.html "petgraph::stable_graph - Rust"
[3]: https://docs.rs/petgraph/latest/petgraph/graph/struct.Graph.html "Graph in petgraph::graph - Rust"
[4]: https://docs.rs/petgraph/latest/petgraph/visit/trait.NodeIndexable.html "NodeIndexable in petgraph::visit - Rust"
[5]: https://docs.rs/petgraph/latest/petgraph/visit/trait.NodeCompactIndexable.html "NodeCompactIndexable in petgraph::visit - Rust"


# 6) `GraphMap` deep dive — map-key node identity

Format follows the uploaded advanced-reference style. 

`GraphMap<N, E, Ty, S>` is petgraph’s **keyed simple-graph representation**: node values `N` are the node identifiers; edge IDs are endpoint pairs `(N, N)`; storage combines adjacency-list and sparse-adjacency-matrix ideas; space is `O(|V| + |E|)`; edge-existence testing is constant-time; parallel edges are disallowed; self-loops are allowed. ([Docs.rs][1])

---

## 6.0 Type shape

```rust
use petgraph::graphmap::{GraphMap, DiGraphMap, UnGraphMap};
use petgraph::{Directed, Undirected};

pub struct GraphMap<N, E, Ty, S = std::collections::hash_map::RandomState>
where
    S: std::hash::BuildHasher;
```

Source-level aliases:

```rust
pub type UnGraphMap<N, E, S = RandomState> =
    GraphMap<N, E, Undirected, S>;

pub type DiGraphMap<N, E, S = RandomState> =
    GraphMap<N, E, Directed, S>;
```

`UnGraphMap` treats `(a, b)` and `(b, a)` as the same undirected edge; `DiGraphMap` treats `a -> b` and `b -> a` as distinct directed edges. ([Docs.rs][2])

Canonical imports:

```rust
use petgraph::graphmap::{DiGraphMap, UnGraphMap};
use petgraph::Direction::{Incoming, Outgoing};
```

---

## 6.1 Core identity model: node value = node identifier

`GraphMap` does **not** allocate `NodeIndex<Ix>` handles. The node key `N` is the node ID, the node weight, and the lookup key. Internally, the implementation stores node keys in maps and edge keys as endpoint pairs. ([Docs.rs][2])

```rust
use petgraph::graphmap::DiGraphMap;

let mut g = DiGraphMap::<u64, u32>::new();

g.add_edge(10, 20, 5);

assert!(g.contains_node(10));
assert!(g.contains_node(20));
assert!(g.contains_edge(10, 20));

assert_eq!(g.edge_weight(10, 20), Some(&5));
```

Mental model:

```text
Graph / StableGraph:
    graph-local identity = NodeIndex<Ix>
    semantic ID usually stored in payload or side map

GraphMap:
    graph-local identity = N
    semantic ID should be the node value itself
    edge identity = (N, N)
```

Trait view: `GraphMap` implements `GraphBase` with `NodeId = N` and `EdgeId = (N, N)`. ([Docs.rs][1])

---

## 6.2 Required node bounds: `Copy + Eq + Hash + Ord`

The `GraphMap` docs require node type `N` to be copyable, usable as a hash-table key, and orderable; source defines `NodeTrait` as `Copy + Ord + Hash`, and `Ord` implies equality semantics through Rust’s ordering trait hierarchy. ([Docs.rs][1])

Good node-key types:

```rust
u8
u16
u32
u64
usize

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct UserId(u64);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
enum State {
    Start,
    Parsing,
    Done,
}
```

Bad node-key types:

```text
String              // not Copy
Vec<T>              // not Copy
large struct         // duplicated internally; bad memory locality
domain payload       // GraphMap key should be identity, not large data
Arc<T>               // Copy bound fails
Uuid type            // only OK if Copy + Ord + Hash; many UUID types are Copy, verify crate
```

Recommended string-like strategy: intern string IDs to compact copyable symbols.

```rust
use petgraph::graphmap::DiGraphMap;
use std::collections::HashMap;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct SymbolId(u32);

struct Interner {
    ids: HashMap<String, SymbolId>,
    names: Vec<String>,
}

impl Interner {
    fn intern(&mut self, s: &str) -> SymbolId {
        if let Some(&id) = self.ids.get(s) {
            return id;
        }

        let id = SymbolId(self.names.len() as u32);
        self.names.push(s.to_owned());
        self.ids.insert(s.to_owned(), id);
        id
    }
}

let mut g = DiGraphMap::<SymbolId, u32>::new();
```

Agent rule:

```text
Use GraphMap<N, E> only when:
    N is the stable semantic ID
    N is small and Copy
    N has total ordering
    no separate rich node payload is needed inside the graph
```

---

## 6.3 Constructors and capacity

### `new()`

```rust
use petgraph::graphmap::DiGraphMap;

let g = DiGraphMap::<u64, ()>::new();
```

`GraphMap::new()` creates an empty map-backed graph; the default hasher parameter is `RandomState` under `std`. ([Docs.rs][1])

### `with_capacity(nodes, edges)`

```rust
use petgraph::graphmap::DiGraphMap;

let mut g = DiGraphMap::<u64, u32>::with_capacity(10_000, 50_000);

let (node_cap, edge_cap) = g.capacity();
assert!(node_cap >= 10_000);
assert!(edge_cap >= 50_000);
```

`with_capacity` creates a `GraphMap` with estimated node and edge capacity; `capacity()` reports current node and edge capacity. ([Docs.rs][1])

### `with_capacity_and_hasher(nodes, edges, hasher)`

```rust
use std::collections::hash_map::RandomState;
use petgraph::graphmap::DiGraphMap;

let hasher = RandomState::new();

let g = DiGraphMap::<u64, u32, RandomState>::with_capacity_and_hasher(
    10_000,
    50_000,
    hasher,
);
```

`with_capacity_and_hasher` constructs a `GraphMap` with estimated capacity and a specified `BuildHasher`. ([Docs.rs][1])

### `from_edges(iterable)`

```rust
use petgraph::graphmap::UnGraphMap;

let g = UnGraphMap::<_, ()>::from_edges([
    (0u32, 1u32),
    (0u32, 2u32),
    (1u32, 2u32),
]);
```

Weighted edge form:

```rust
use petgraph::graphmap::DiGraphMap;

let g = DiGraphMap::<_, u32>::from_edges([
    (10u64, 20u64, 5u32),
    (20u64, 30u64, 7u32),
]);
```

`from_edges` takes node values directly from the edge list, uses provided edge weights or default edge weights, and automatically inserts nodes required by edges. ([Docs.rs][1])

---

## 6.4 Mutations

### `add_node(n) -> N`

```rust
use petgraph::graphmap::DiGraphMap;

let mut g = DiGraphMap::<u32, ()>::new();

let id = g.add_node(42);

assert_eq!(id, 42);
assert!(g.contains_node(42));
```

`add_node` inserts node `n` and returns `n`. Duplicate node insertion is idempotent from a graph-topology perspective. ([Docs.rs][1])

### `add_edge(a, b, weight) -> Option<E>`

```rust
use petgraph::graphmap::DiGraphMap;

let mut g = DiGraphMap::<u32, &'static str>::new();

assert_eq!(g.add_edge(1, 2, "first"), None);
assert_eq!(g.add_edge(1, 2, "second"), Some("first"));

assert_eq!(g.edge_count(), 1);
assert_eq!(g.edge_weight(1, 2), Some(&"second"));
```

`add_edge` inserts missing endpoint nodes automatically. If the edge did not previously exist, it returns `None`; if the edge already existed, it updates the edge weight and returns `Some(old_weight)`. ([Docs.rs][1])

Important semantic difference from `Graph`:

```text
Graph::add_edge:
    always inserts a new edge
    parallel edges possible

GraphMap::add_edge:
    inserts or updates exactly one endpoint-pair edge
    returns old edge weight if overwritten
    no parallel edges
```

### `remove_edge(a, b) -> Option<E>`

```rust
use petgraph::graphmap::UnGraphMap;

let mut g = UnGraphMap::<u32, i32>::new();

g.add_edge(1, 2, -1);

assert_eq!(g.remove_edge(2, 1), Some(-1)); // undirected symmetry
assert_eq!(g.edge_count(), 0);
```

`remove_edge` removes the edge from `a` to `b` and returns the edge weight, or `None` if no edge exists. In an undirected map, reverse endpoint order identifies the same edge. ([Docs.rs][1])

### `remove_node(n) -> bool`

```rust
use petgraph::graphmap::DiGraphMap;

let mut g = DiGraphMap::<u32, ()>::new();

g.add_edge(1, 2, ());
g.add_edge(1, 3, ());

assert!(g.remove_node(1));
assert!(!g.contains_node(1));
assert!(!g.contains_edge(1, 2));
assert!(!g.contains_edge(1, 3));
```

`remove_node` removes node `n`, returns `true` if it existed, and runs in `O(V)` because incident edges with other nodes must be removed. ([Docs.rs][1])

### `clear()`

```rust
g.clear();

assert_eq!(g.node_count(), 0);
assert_eq!(g.edge_count(), 0);
```

`clear` removes all nodes and edges. ([Docs.rs][1])

---

## 6.5 Accessors

### Node presence

```rust
assert!(g.contains_node(1));
assert!(!g.contains_node(999));
```

`contains_node(n)` returns whether the node is present. ([Docs.rs][1])

### Edge presence

```rust
assert!(g.contains_edge(1, 2));
assert!(!g.contains_edge(2, 1)); // for directed graph
```

`contains_edge(a, b)` returns whether the edge connecting `a` and `b` is present; the representation supports constant-time edge-existence testing. ([Docs.rs][1])

### Edge weight

```rust
if let Some(w) = g.edge_weight(1, 2) {
    println!("weight={w}");
}

if let Some(w) = g.edge_weight_mut(1, 2) {
    *w += 1;
}
```

`edge_weight(a, b)` and `edge_weight_mut(a, b)` return immutable/mutable references to the edge weight, or `None` if the edge does not exist. ([Docs.rs][1])

### Indexing by endpoint pair

```rust
use petgraph::graphmap::DiGraphMap;

let mut g = DiGraphMap::<u32, u32>::new();

g.add_edge(1, 2, 10);

assert_eq!(g[(1, 2)], 10);

g[(1, 2)] = 99;
assert_eq!(g.edge_weight(1, 2), Some(&99));
```

`GraphMap` implements indexing and mutable indexing by endpoint pair `(N, N)` to access edge weights. ([Docs.rs][1])

Agent rule:

```text
Use edge_weight / edge_weight_mut:
    endpoint pair may be absent
    input is user-provided
    safe error path required

Use graph[(a, b)]:
    edge existence is an invariant
    panic on missing edge is acceptable
```

---

## 6.6 Iteration

### Nodes

```rust
for n in g.nodes() {
    println!("node={n:?}");
}
```

`nodes()` returns an iterator over node values; item type is `N`. ([Docs.rs][1])

### Neighbors

```rust
for dst in g.neighbors(1) {
    println!("1 -> {dst}");
}
```

`neighbors(a)` returns nodes reachable by an edge starting from `a`; for directed graphs this means outgoing neighbors, while for undirected graphs this means all adjacent nodes. Missing nodes produce an empty iterator. ([Docs.rs][1])

Direction-explicit:

```rust
use petgraph::Direction::{Incoming, Outgoing};

for dst in g.neighbors_directed(1, Outgoing) {
    println!("outgoing to {dst}");
}

for src in g.neighbors_directed(1, Incoming) {
    println!("incoming from {src}");
}
```

`neighbors_directed(a, dir)` respects incoming/outgoing direction on directed graphs and is equivalent to `neighbors(a)` for undirected graphs. ([Docs.rs][1])

### Edges adjacent to a node

```rust
for (a, b, w) in g.edges(1) {
    println!("{a:?} -> {b:?}: {w:?}");
}
```

`edges(a)` returns `(N, N, &E)` tuples for edges starting from `a`; directed graphs return outgoing edges and undirected graphs return all adjacent edges. ([Docs.rs][1])

Direction-explicit edges:

```rust
for (a, b, w) in g.edges_directed(1, Incoming) {
    println!("{a:?} -> {b:?}: {w:?}");
}
```

`edges_directed(a, dir)` returns `(N, N, &E)` and has direction-specific endpoint orientation rules. ([Docs.rs][1])

### All edges

```rust
for (a, b, w) in g.all_edges() {
    println!("{a:?} -> {b:?}: {w:?}");
}

for (_a, _b, w) in g.all_edges_mut() {
    *w += 1;
}
```

`all_edges()` returns all edges with weights in arbitrary order, item type `(N, N, &E)`; `all_edges_mut()` returns `(N, N, &mut E)`. ([Docs.rs][1])

---

## 6.7 Directed vs undirected semantics

### `DiGraphMap`

```rust
use petgraph::graphmap::DiGraphMap;

let mut g = DiGraphMap::<&str, i32>::new();

g.add_edge("x", "y", 1);

assert!(g.contains_edge("x", "y"));
assert!(!g.contains_edge("y", "x"));
```

For directed maps, `a -> b` and `b -> a` are different edges. The docs’ directed example asserts exactly that after adding `"x" -> "y"`, reverse edge lookup fails. ([Docs.rs][1])

### `UnGraphMap`

```rust
use petgraph::graphmap::UnGraphMap;

let mut g = UnGraphMap::<&str, i32>::new();

g.add_edge("x", "y", 1);

assert!(g.contains_edge("x", "y"));
assert!(g.contains_edge("y", "x"));

assert_eq!(g.remove_edge("y", "x"), Some(1));
```

For undirected maps, an edge between `a` and `b` is equivalent regardless of endpoint order. ([Docs.rs][2])

---

## 6.8 No parallel edges; self-loops allowed

`GraphMap` is a **simple graph/digraph representation** with at most one edge per endpoint pair; self-loops are allowed. ([Docs.rs][1])

### Duplicate edge update

```rust
use petgraph::graphmap::DiGraphMap;

let mut g = DiGraphMap::<u32, u32>::new();

assert_eq!(g.add_edge(1, 2, 10), None);
assert_eq!(g.add_edge(1, 2, 20), Some(10));

assert_eq!(g.edge_count(), 1);
assert_eq!(g[(1, 2)], 20);
```

### Self-loop

```rust
use petgraph::graphmap::DiGraphMap;

let mut g = DiGraphMap::<u32, &'static str>::new();

g.add_edge(1, 1, "self");

assert!(g.contains_edge(1, 1));
assert_eq!(g.edge_weight(1, 1), Some(&"self"));
```

Use `GraphMap` when:

```text
one logical relationship per endpoint pair
edge overwrite semantics are correct
self-loops are meaningful or acceptable
parallel-edge preservation is not required
```

Avoid `GraphMap` when:

```text
edge multiplicity carries meaning
events are modeled as multiple edges
two endpoints may have multiple relationship types simultaneously
edge identity must be independent of endpoint pair
```

For multiple relationship types, encode multiplicity inside `E`:

```rust
use petgraph::graphmap::DiGraphMap;

#[derive(Debug, Default)]
struct EdgeKinds {
    calls: bool,
    owns: bool,
    monitors: bool,
}

let mut g = DiGraphMap::<u32, EdgeKinds>::new();

g.add_edge(1, 2, EdgeKinds { calls: true, ..Default::default() });

if let Some(e) = g.edge_weight_mut(1, 2) {
    e.monitors = true;
}
```

---

## 6.9 Automatic node insertion through `add_edge`

`GraphMap::add_edge(a, b, weight)` inserts nodes `a` and/or `b` if they are not already present. ([Docs.rs][1])

```rust
use petgraph::graphmap::DiGraphMap;

let mut g = DiGraphMap::<u64, u32>::new();

assert_eq!(g.node_count(), 0);

g.add_edge(10, 20, 5);

assert_eq!(g.node_count(), 2);
assert!(g.contains_node(10));
assert!(g.contains_node(20));
```

Deployment implications:

```text
Pros:
    edge-list ingestion is simple
    no explicit intern/side-map layer required
    missing endpoint bootstrap is automatic

Cons:
    typo/key error silently creates a new node
    no validation hook before node creation
    input normalization must occur before add_edge
```

Validation wrapper:

```rust
use std::collections::HashSet;
use petgraph::graphmap::DiGraphMap;

fn add_checked_edge(
    g: &mut DiGraphMap<u64, u32>,
    allowed: &HashSet<u64>,
    a: u64,
    b: u64,
    w: u32,
) -> Result<Option<u32>, &'static str> {
    if !allowed.contains(&a) || !allowed.contains(&b) {
        return Err("unknown node id");
    }

    Ok(g.add_edge(a, b, w))
}
```

---

## 6.10 Hasher customization

`GraphMap` has a fourth generic parameter `S` implementing `BuildHasher`; under `std`, the default is `RandomState`, and constructors include `with_capacity_and_hasher`. ([Docs.rs][1])

### Default hasher policy

Rust’s standard `HashMap` default hashing is selected for HashDoS resistance, uses random seeding, and the current default algorithm is SipHash 1-3, though the standard library explicitly reserves the right to change it; other hashers can outperform it for small integer keys or long strings but may not protect against HashDoS. ([Rust Documentation][3])

Policy table:

| Workload                             | Hasher stance                                             |
| ------------------------------------ | --------------------------------------------------------- |
| Untrusted external keys              | Keep `RandomState`                                        |
| Public network service               | Keep `RandomState` unless risk-reviewed                   |
| Trusted integer IDs, hot lookup path | Consider custom fast non-cryptographic hasher             |
| Reproducible benchmark fixtures      | Use fixed deterministic hasher                            |
| Persistent serialized hash order     | Do not depend on hash-map iteration order                 |
| `no_std` build                       | Provide explicit `S` if default `RandomState` unavailable |

### Custom hasher type alias

```rust
use std::collections::hash_map::RandomState;
use petgraph::graphmap::GraphMap;
use petgraph::Directed;

type MyGraphMap<N, E, S> = GraphMap<N, E, Directed, S>;
type DefaultKeyGraph<N, E> = GraphMap<N, E, Directed, RandomState>;
```

### Deterministic hasher skeleton for trusted integer keys

```rust
use std::hash::{BuildHasher, Hasher};
use petgraph::graphmap::DiGraphMap;

#[derive(Clone, Default)]
struct DeterministicBuildHasher;

#[derive(Default)]
struct DeterministicHasher {
    state: u64,
}

impl Hasher for DeterministicHasher {
    fn write(&mut self, bytes: &[u8]) {
        // Tiny FNV-1a-like example. Use only for trusted keys / tests.
        let mut h = self.state ^ 0xcbf29ce484222325;
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        self.state = h;
    }

    fn write_u64(&mut self, i: u64) {
        self.state = i.wrapping_mul(0x9E3779B97F4A7C15);
    }

    fn finish(&self) -> u64 {
        self.state
    }
}

impl BuildHasher for DeterministicBuildHasher {
    type Hasher = DeterministicHasher;

    fn build_hasher(&self) -> Self::Hasher {
        DeterministicHasher::default()
    }
}

type FastTrustedGraph = DiGraphMap<u64, u32, DeterministicBuildHasher>;

let mut g = FastTrustedGraph::with_capacity_and_hasher(
    10_000,
    50_000,
    DeterministicBuildHasher,
);

g.add_edge(1, 2, 7);
```

Agent rule:

```text
Default RandomState:
    safe default
    randomized
    HashDoS-aware
    not deterministic by seed

Custom deterministic hasher:
    good for tests / trusted data / reproducible perf
    bad for hostile keys unless collision-resistance reviewed

Fast integer hasher:
    measure before adopting
    document threat model
    keep hasher type in public alias if graph type is public
```

---

## 6.11 Conversions with `Graph`

### `into_graph<Ix>()`

```rust
use petgraph::graphmap::DiGraphMap;
use petgraph::graph::Graph;

let mut gm = DiGraphMap::<u64, u32>::new();
gm.add_edge(10, 20, 5);

let g: Graph<u64, u32> = gm.into_graph();
```

`into_graph` converts a `GraphMap` into a `Graph`; resulting `Graph` node/edge indices have nothing in common with `GraphMap` node values, and the `GraphMap` node values become node weights in the resulting `Graph`. The conversion is average `O(|V| + |E|)` and can panic if the target index type cannot fit the graph. ([Docs.rs][1])

### `from_graph(graph)`

```rust
use petgraph::graph::DiGraph;
use petgraph::graphmap::DiGraphMap;

let mut g = DiGraph::<u64, u32>::new();

let a = g.add_node(10);
let b = g.add_node(20);

g.add_edge(a, b, 5);

let gm = DiGraphMap::<u64, u32>::from_graph(g);
```

`from_graph` creates a corresponding `GraphMap`, but nodes with identical weights are merged, only the last parallel edge is kept, and original `Graph` node/edge indices are lost; use it only when node weights are distinct and there are no parallel edges. ([Docs.rs][1])

Migration rule:

```text
GraphMap -> Graph:
    okay when algorithms require Graph
    rebuild NodeIndex-based side maps
    Graph node weights = original N keys

Graph -> GraphMap:
    safe only if node weights are unique IDs
    unsafe for multigraphs
    unsafe if NodeIndex identity matters
```

---

## 6.12 Algorithm compatibility and trait behavior

`GraphMap` implements many petgraph visitor/data traits, including `GraphBase`, `GraphProp`, `IntoNeighbors`, `IntoNeighborsDirected`, `IntoEdges`, `IntoEdgesDirected`, `IntoEdgeReferences`, `IntoNodeIdentifiers`, `IntoNodeReferences`, `NodeIndexable`, `NodeCompactIndexable`, `Visitable`, and count traits. ([Docs.rs][1])

Generic traversal:

```rust
use petgraph::visit::{IntoNeighbors, Visitable};
use petgraph::visit::Dfs;

fn reachable_count<G>(graph: G, start: G::NodeId) -> usize
where
    G: IntoNeighbors + Visitable,
    G::NodeId: Copy,
{
    let mut dfs = Dfs::new(graph, start);
    let mut count = 0;

    while let Some(_) = dfs.next(graph) {
        count += 1;
    }

    count
}

use petgraph::graphmap::DiGraphMap;

let mut g = DiGraphMap::<u64, ()>::new();
g.add_edge(1, 2, ());
g.add_edge(2, 3, ());

assert_eq!(reachable_count(&g, 1), 3);
```

Agent rule:

```text
GraphMap works well with trait-generic read/traversal algorithms.
Concrete Graph-only algorithms may require conversion via into_graph().
After conversion, Graph NodeIndex values are new and unrelated to original N keys.
```

---

## 6.13 Parallel iterators

`GraphMap` exposes parallel node and edge iterators behind the `rayon` feature: `par_nodes`, `par_all_edges`, and `par_all_edges_mut`. Their item types are `N`, `(N, N, &E)`, and `(N, N, &mut E)` respectively; returned order is arbitrary. ([Docs.rs][1])

Cargo:

```toml
[dependencies]
petgraph = { version = "0.8.3", features = ["rayon"] }
rayon = "1"
```

Usage:

```rust
use petgraph::graphmap::DiGraphMap;

let mut g = DiGraphMap::<u64, u32>::new();

g.add_edge(1, 2, 10);
g.add_edge(2, 3, 20);

#[cfg(feature = "rayon")]
{
    use rayon::prelude::*;

    let total: u32 = g
        .par_all_edges()
        .map(|(_a, _b, w)| *w)
        .sum();

    assert_eq!(total, 30);
}
```

Rule:

```text
Use parallel GraphMap iteration when:
    graph is large
    per-edge/node work is nontrivial
    arbitrary order is acceptable
    N/E satisfy Send/Sync bounds

Avoid when:
    deterministic iteration order is required
    work per item is tiny
    mutation requires cross-edge coordination
```

---

## 6.14 Best-fit modeling patterns

### Integer ID graph

```rust
use petgraph::graphmap::DiGraphMap;

type ServiceId = u32;
type Cost = u32;

let mut calls = DiGraphMap::<ServiceId, Cost>::new();

calls.add_edge(10, 20, 4);
calls.add_edge(20, 30, 7);
```

Fit:

```text
small copyable service IDs
simple dependency/call graph
fast contains_edge hot path
no rich node payload in graph
```

### Enum-state graph

```rust
use petgraph::graphmap::DiGraphMap;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
enum State {
    Start,
    Authenticated,
    Failed,
    Done,
}

let mut sm = DiGraphMap::<State, &'static str>::new();

sm.add_edge(State::Start, State::Authenticated, "login_ok");
sm.add_edge(State::Start, State::Failed, "login_fail");
sm.add_edge(State::Authenticated, State::Done, "finish");
```

Fit:

```text
finite-state machines
control states
enum node identifiers
compact key identity
```

### Interned string graph

```rust
use petgraph::graphmap::UnGraphMap;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct InternedStr(u32);

let mut g = UnGraphMap::<InternedStr, ()>::new();
```

Fit:

```text
string-heavy domain
copyable symbol IDs
separate string table
GraphMap key requirements satisfied
```

---

## 6.15 GraphMap vs `Graph + HashMap<ID, NodeIndex>`

| Requirement                                        | `GraphMap`          | `Graph + HashMap`                   |
| -------------------------------------------------- | ------------------- | ----------------------------------- |
| Node identity is small copyable key                | Excellent           | Good                                |
| Node identity is `String`/large payload            | Bad                 | Excellent                           |
| Need rich node payload separate from ID            | Weak                | Excellent                           |
| Need parallel edges                                | No                  | Yes                                 |
| Need constant-time edge-existence by endpoint pair | Built-in            | Add side edge map or scan adjacency |
| Need `NodeIndex` handles                           | No                  | Yes                                 |
| Need broad concrete `Graph` APIs                   | Convert             | Native                              |
| Need simple graph semantics                        | Native              | Use `update_edge` or side map       |
| Need external ID lookup                            | Native if ID is `N` | Side map                            |

Decision:

```text
Choose GraphMap:
    N is already the domain ID
    N is Copy + Ord + Hash
    graph is simple
    contains_edge(a,b) is hot
    node payload is unnecessary or external

Choose Graph + HashMap:
    node IDs are strings/UUIDs/non-Copy
    nodes need rich payloads
    parallel edges matter
    algorithms/APIs expect Graph
    NodeIndex metadata tables matter
```

---

## 6.16 Deployment checklist

```text
Cargo:
    default features include graphmap in normal petgraph builds
    disable default features only if you intentionally manage feature surface
```

```toml
[dependencies]
petgraph = "0.8.3"
```

For `serde`:

```toml
[dependencies]
petgraph = { version = "0.8.3", features = ["serde-1"] }
serde = { version = "1", features = ["derive"] }
```

`GraphMap` has serde implementations behind the `serde-1` feature; serialization uses the same format as standard `Graph` and requires clone support for the graph in the serialization implementation. ([Docs.rs][1])

For `rayon`:

```toml
[dependencies]
petgraph = { version = "0.8.3", features = ["rayon"] }
rayon = "1"
```

---

## 6.17 Anti-pattern inventory

```text
Anti-pattern:
    DiGraphMap<String, E>
Problem:
    String is not Copy.
Fix:
    intern strings to SymbolId(u32), or use Graph + HashMap<String, NodeIndex>.

Anti-pattern:
    Store rich user object as N.
Problem:
    N is duplicated and must be Copy + Ord + Hash.
Fix:
    use UserId(u64) as N and store rich data externally.

Anti-pattern:
    Model multiple event records by repeated add_edge(a,b,event).
Problem:
    add_edge overwrites existing edge and returns old weight.
Fix:
    use Graph for multiedges, or store Vec<Event> as E.

Anti-pattern:
    Convert Graph with duplicate node weights to GraphMap.
Problem:
    duplicate node weights merge; parallel edges collapse.
Fix:
    ensure unique node weights and no parallel edges first.

Anti-pattern:
    Depend on default hasher iteration/order reproducibility.
Problem:
    RandomState is randomized and hash algorithm is not stable contract.
Fix:
    use explicit deterministic hasher for tests, or sort exported data.

Anti-pattern:
    Let add_edge create nodes from unvalidated external input.
Problem:
    typos create nodes automatically.
Fix:
    validate IDs before edge insertion.
```

---

## 6.18 Final agent rules

```text
GraphMap is best when:
    node key is the node
    key is small, Copy, Ord, Hash
    graph is simple: one edge per pair
    edge-existence lookup is hot
    endpoint-pair indexing is ergonomic
    external side maps are undesirable

GraphMap is wrong when:
    node payload is large/non-Copy
    multiple edges per pair are required
    NodeIndex is required
    Graph-specific algorithms/APIs dominate
    deletion-heavy rich payload model needs stable handles

Default stance:
    integer IDs / enum states / interned string symbols => GraphMap
    strings / UUID side data / rich payloads / multiedges => Graph or StableGraph
```

[1]: https://docs.rs/petgraph/latest/petgraph/graphmap/struct.GraphMap.html "GraphMap in petgraph::graphmap - Rust"
[2]: https://docs.rs/petgraph/latest/src/petgraph/graphmap.rs.html "graphmap.rs - source"
[3]: https://doc.rust-lang.org/beta/std/collections/struct.HashMap.html?utm_source=chatgpt.com "HashMap in std::collections - Rust"


# 7) `MatrixGraph` deep dive — dense graph representation

Format follows the uploaded advanced-reference style. 

`MatrixGraph<N, E, S, Ty, Null, Ix>` is petgraph’s adjacency-matrix graph family: arbitrary node weights `N`, arbitrary edge weights `E`, directedness marker `Ty`, edge-presence wrapper `Null`, node-index type `Ix`, `O(|V|²)` topology storage, fast edge insertion/lookup in the no-reallocation case, and dense-graph-oriented traversal/algorithm behavior. The implementation is backed by a flattened 2D array; undirected graphs store only the lower triangular matrix; large edge weights should be boxed because the backing array stores edge-weight slots. ([Docs.rs][1])

---

## 7.0 Type shape

```rust
use petgraph::matrix_graph::{
    MatrixGraph,
    DiMatrix,
    UnMatrix,
    NotZero,
};

use petgraph::{Directed, Undirected};
use std::collections::hash_map::RandomState;
```

Core source-level shape:

```rust
pub struct MatrixGraph<
    N,
    E,
    S = RandomState,
    Ty = Directed,
    Null: Nullable<Wrapped = E> = Option<E>,
    Ix = DefaultIx,
>;
```

Matrix-local `DefaultIx` is `u16`, not the graph module’s normal `u32`; the matrix module source explicitly defines its own `type DefaultIx = u16` because adjacency-matrix backing size grows with the square of the node bound. ([Docs.rs][2])

Aliases:

```rust
pub type DiMatrix<N, E, S = RandomState, Null = Option<E>, Ix = DefaultIx> =
    MatrixGraph<N, E, S, Directed, Null, Ix>;

pub type UnMatrix<N, E, S = RandomState, Null = Option<E>, Ix = DefaultIx> =
    MatrixGraph<N, E, S, Undirected, Null, Ix>;
```

The source defines `DiMatrix` and `UnMatrix` as directed/undirected `MatrixGraph` aliases with default `Null = Option<E>` and matrix-local `Ix = DefaultIx`. ([Docs.rs][2])

---

## 7.1 Adjacency matrix storage model

Mental model:

```text
Directed MatrixGraph:
    conceptual slots = V * V
    slot(a, b)       = edge a -> b, or Null sentinel

Undirected MatrixGraph:
    conceptual slots = V * (V + 1) / 2
    slot(a, b)       = edge between a and b, normalized into lower triangle

Stored array:
    Vec<Null>
    flattened 2D matrix
    no per-node adjacency Vec
    no independent EdgeIndex
    edge identity = endpoint pair (NodeIndex<Ix>, NodeIndex<Ix>)
```

Source confirms `MatrixGraph` stores `node_adjacencies: Vec<Null>`, `node_capacity`, node storage, edge count, and type/index markers. ([Docs.rs][2])

Canonical type:

```rust
use petgraph::matrix_graph::DiMatrix;

let mut g = DiMatrix::<&str, u32>::with_capacity(4);

let a = g.add_node("a");
let b = g.add_node("b");

g.add_edge(a, b, 10);

assert!(g.has_edge(a, b));
assert_eq!(*g.edge_weight(a, b), 10);
```

---

## 7.2 Space complexity and dense suitability

`MatrixGraph` uses `O(|V|²)` space and is designed for dense graphs; docs explicitly describe fast edge insertion, amortized node insertion, and efficient dense-graph search/algorithms. ([Docs.rs][2])

Decision rule:

```text
Let V = node count
Let E = edge count

Sparse:
    E << V²
    prefer Graph / StableGraph / GraphMap / Csr

Dense:
    E is large fraction of V²
    prefer MatrixGraph

Lookup-heavy:
    frequent has_edge(a,b) / edge_weight(a,b)
    MatrixGraph likely beats adjacency-list scans

Traversal-heavy sparse:
    MatrixGraph scans matrix-style storage
    Csr / Graph usually better
```

Approximate storage pressure:

```text
Directed slots:
    V²

Undirected slots:
    V * (V + 1) / 2

Slot payload:
    size_of::<Null>()

Default Null:
    Option<E>

Sentinel Null:
    NotZero<E>, when E: Zero
```

Hard warning:

```text
V = 10_000 directed
    slots ≈ 100,000,000

E = u64, Null = Option<u64>
    slot size likely >= 8 bytes, often maybe larger due layout
    matrix backing alone can approach hundreds of MB to GB

V = 1_000_000
    impossible ordinary MatrixGraph deployment
```

---

## 7.3 Constructors

### `new()`

```rust
use petgraph::matrix_graph::DiMatrix;

let g = DiMatrix::<&str, u32>::new();
assert_eq!(g.node_count(), 0);
assert_eq!(g.edge_count(), 0);
```

`MatrixGraph::new()` is the directed convenience constructor; docs recommend `with_capacity` or `default` for constructors generic over all type parameters. ([Docs.rs][1])

### `new_undirected()`

```rust
use petgraph::matrix_graph::UnMatrix;

let g = UnMatrix::<&str, u32>::new_undirected();
assert!(!g.is_directed());
```

`new_undirected()` creates an undirected matrix graph and has the same convenience-constructor caveat. ([Docs.rs][1])

### `with_capacity(node_capacity)`

```rust
use petgraph::matrix_graph::DiMatrix;

let mut g = DiMatrix::<&str, u32>::with_capacity(512);
```

Unlike `Graph::with_capacity(nodes, edges)`, `MatrixGraph::with_capacity` accepts **node capacity only**, because edge capacity is implied by node capacity squared/triangular matrix shape. The source constructs backing storage sized from `node_capacity` and extends matrix capacity for the final node slot when capacity is nonzero. ([Docs.rs][2])

### `with_capacity_and_hasher(node_capacity, hasher)`

```rust
use petgraph::matrix_graph::DiMatrix;
use std::collections::hash_map::RandomState;

let g = DiMatrix::<&str, u32, RandomState>::with_capacity_and_hasher(
    512,
    RandomState::new(),
);
```

Use when node-storage hashing policy matters for deterministic tests or specialized node-storage behavior.

### `from_edges(iterable)`

```rust
use petgraph::matrix_graph::MatrixGraph;

let g = MatrixGraph::<(), i32>::from_edges([
    (0, 1),
    (0, 2),
    (1, 2),
]);
```

Weighted edge form:

```rust
use petgraph::matrix_graph::MatrixGraph;

let g = MatrixGraph::<(), i32>::from_edges([
    (0, 1, 10),
    (0, 2, 20),
    (1, 2, 30),
]);
```

`from_edges` sets node weights to `Default`, accepts weighted or default edge weights, and automatically inserts nodes to match edge endpoints. ([Docs.rs][1])

---

## 7.4 Null edge-presence abstraction

`Null` is the matrix slot wrapper that encodes edge presence/absence. Default:

```rust
Null = Option<E>
```

Meaning:

```text
None    => no edge
Some(E) => edge present with weight E
```

The `Nullable` trait is sealed and implemented for `Option<T>` and `NotZero<T>`; external crates cannot implement custom `Nullable` types. ([Docs.rs][3])

### Default `Option<E>` strategy

```rust
use petgraph::matrix_graph::DiMatrix;

type Dense = DiMatrix<&'static str, u32>; // Null = Option<u32>
```

Use when:

```text
E has no reserved sentinel
zero / default value is a valid edge weight
clarity > slot-size optimization
edge payload is non-numeric or enum/struct
```

### Sentinel `NotZero<E>` strategy

`NotZero<T>` replaces the default `Option<E>` sentinel and uses the edge value’s zero value as absence; the edge weight type must implement `Zero`, and standard non-zero integer types such as `NonZeroU32` do not need this wrapper because `Option<NonZeroU32>` already has an optimized layout. ([Docs.rs][4])

```rust
use petgraph::matrix_graph::{MatrixGraph, NotZero};
use petgraph::Directed;
use std::collections::hash_map::RandomState;

type DenseCost =
    MatrixGraph<&'static str, u32, RandomState, Directed, NotZero<u32>>;

let mut g = DenseCost::with_capacity(256);

let a = g.add_node("a");
let b = g.add_node("b");

// Do not use zero as a valid edge weight with NotZero<u32>.
g.add_edge(a, b, 7);
```

Sentinel contract:

```text
NotZero<E>:
    absence encoded by E::zero()
    zero cannot safely represent a semantic edge weight
    E must implement matrix_graph::Zero
    good for numeric positive-cost dense graphs

Option<E>:
    absence encoded structurally
    all E values remain valid edge weights
    usually safer default
```

Practical recommendation:

```text
Use Option<E> unless:
    matrix memory is a measured bottleneck
    E is numeric
    zero is semantically impossible/reserved
    tests assert that zero-weight edges are rejected/avoided
```

---

## 7.5 Edge insertion and lookup tradeoffs

### `add_node(weight) -> NodeIndex<Ix>`

```rust
use petgraph::matrix_graph::DiMatrix;

let mut g = DiMatrix::<&str, u32>::with_capacity(4);

let a = g.add_node("a");
let b = g.add_node("b");
```

`add_node` is documented as `O(1)` and panics if the graph has reached the maximum number of nodes for its index type; `try_add_node` returns `MatrixError::NodeIxLimit`. ([Docs.rs][1])

### `update_edge(a, b, weight) -> Option<E>`

```rust
let old = g.update_edge(a, b, 10);
assert_eq!(old, None);

let old = g.update_edge(a, b, 20);
assert_eq!(old, Some(10));
```

`update_edge` overwrites or inserts the endpoint-pair edge and returns the previous weight if any. It is `O(1)` best case and `O(|V|²)` worst case when the matrix must be reallocated. ([Docs.rs][2])

### `try_update_edge(a, b, weight) -> Result<Option<E>, MatrixError>`

```rust
let previous = g.try_update_edge(a, b, 30)?;
```

Use at API boundaries; `MatrixError::NodeMissed` is returned if endpoints do not exist. ([Docs.rs][2])

### `add_edge(a, b, weight)`

```rust
g.add_edge(a, b, 40);
```

`add_edge` inserts a new endpoint-pair edge, but panics if endpoints do not exist or if an edge already exists from `a` to `b`; docs explicitly state `MatrixGraph` does not allow duplicate/parallel edges and recommend `update_edge` to avoid duplicate-edge panics. ([Docs.rs][2])

Agent rule:

```text
Prefer update_edge / try_update_edge:
    normal mutation path
    idempotent build/import
    duplicate endpoint pair should overwrite

Use add_edge:
    invariant-checking path
    duplicate edge means programmer error
```

### `has_edge(a, b) -> bool`

```rust
if g.has_edge(a, b) {
    let w = g.edge_weight(a, b);
    println!("{w}");
}
```

`has_edge` checks matrix presence by endpoint pair and returns `false` if the edge slot is absent or out of current capacity; source computes a linearized matrix position and checks whether the stored `Null` is non-null. ([Docs.rs][2])

### `edge_weight(a, b) -> &E`

```rust
let weight: &u32 = g.edge_weight(a, b);
```

Panicking accessor. Use only when edge existence is an invariant.

### `get_edge_weight(a, b) -> Option<&E>`

```rust
if let Some(w) = g.get_edge_weight(a, b) {
    println!("edge weight = {w}");
}
```

Safe optional accessor.

### Mutable forms

```rust
*g.edge_weight_mut(a, b) += 1;

if let Some(w) = g.get_edge_weight_mut(a, b) {
    *w += 1;
}
```

`edge_weight` / `edge_weight_mut` panic if no endpoint-pair edge exists; `get_edge_weight` / `get_edge_weight_mut` return `None` for absent edges. ([Docs.rs][1])

---

## 7.6 No parallel edges

Matrix slot identity:

```text
edge id = (source NodeIndex, target NodeIndex)
one matrix slot per endpoint pair
therefore no parallel edges
```

Duplicate insertion behavior:

```rust
use petgraph::matrix_graph::DiMatrix;

let mut g = DiMatrix::<&str, u32>::with_capacity(4);

let a = g.add_node("a");
let b = g.add_node("b");

g.add_edge(a, b, 1);

// Panics: duplicate edge.
//// g.add_edge(a, b, 2);

// Safe overwrite:
assert_eq!(g.update_edge(a, b, 2), Some(1));
```

Use matrix graph when:

```text
one logical relationship per endpoint pair
edge overwrite semantics are acceptable
edge existence is naturally Boolean-per-pair
```

Avoid matrix graph when:

```text
parallel edge events must be preserved
multiple relationship records per pair are distinct
edge identity must be independent of endpoints
multigraph algorithms required
```

Encode multiplicity inside `E` when endpoint pair remains the edge identity:

```rust
use petgraph::matrix_graph::DiMatrix;

#[derive(Clone, Debug, Default)]
struct EdgeBundle {
    call_count: u64,
    total_latency_ms: u64,
}

let mut g = DiMatrix::<&str, EdgeBundle>::with_capacity(4);

let a = g.add_node("api");
let b = g.add_node("db");

g.update_edge(a, b, EdgeBundle::default());

let edge = g.edge_weight_mut(a, b);
edge.call_count += 1;
edge.total_latency_ms += 4;
```

---

## 7.7 Large edge-weight warning and boxing strategy

Docs explicitly recommend boxing large edge weights because the flattened backing array stores edge weights/edge slots. ([Docs.rs][2])

Bad dense payload:

```rust
use petgraph::matrix_graph::DiMatrix;

#[derive(Clone, Debug)]
struct LargeEdge {
    payload: Vec<u8>,
    metadata: String,
}

type BadDense = DiMatrix<&'static str, LargeEdge>;
```

Better:

```rust
use petgraph::matrix_graph::DiMatrix;

#[derive(Clone, Debug)]
struct LargeEdge {
    payload: Vec<u8>,
    metadata: String,
}

type BetterDense = DiMatrix<&'static str, Box<LargeEdge>>;
```

Shared payload strategy:

```rust
use petgraph::matrix_graph::DiMatrix;
use std::sync::Arc;

#[derive(Debug)]
struct EdgePayload {
    metadata: String,
}

type SharedDense = DiMatrix<&'static str, Arc<EdgePayload>>;
```

External store strategy:

```rust
use petgraph::matrix_graph::DiMatrix;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct EdgeRowId(u32);

type DenseWithExternalEdges = DiMatrix<&'static str, EdgeRowId>;
```

Decision:

```text
E small numeric:
    store E directly

E large struct:
    Box<E> or Arc<E>

E shared/cached:
    Arc<E>

E stored in DB/arena:
    EdgeId / RowId / index handle

E optional and zero impossible:
    consider NotZero<E>

E optional and all values valid:
    keep Option<E>
```

---

## 7.8 Dense graph algorithms and when matrix beats adjacency lists

Matrix form wins when the graph workload is dominated by endpoint-pair operations:

```text
hot has_edge(a,b)
hot edge_weight(a,b)
dense all-pairs algorithms
complete or near-complete weighted relation
bounded node count
edge density high enough that O(V²) storage is acceptable
```

Adjacency list wins when the graph workload is dominated by sparse traversal:

```text
E << V²
neighbors(a) should inspect degree(a), not row width V
memory footprint is constrained
edge insertions are sparse and unbounded
node count can grow large
parallel edges required
```

Dense use cases:

```text
similarity matrix
distance matrix
complete graph over small entity set
dense transition cost table
dense compatibility matrix
game-state pair relation
all-pairs reachability/cost experiments
bounded finite-state graph with many transitions
```

Sparse anti-cases:

```text
web graph
social graph with millions of nodes
dependency graph
call graph
road network
knowledge graph
streaming event multigraph
```

Rule of thumb:

```text
If average degree ≈ V:
    MatrixGraph likely appropriate

If average degree ≪ V:
    Graph / Csr likely appropriate

If you cannot bound V:
    avoid MatrixGraph

If you cannot afford V² slots:
    avoid MatrixGraph
```

---

## 7.9 Iteration APIs

### Neighbors

```rust
for n in g.neighbors(a) {
    println!("neighbor = {:?}", n);
}
```

`neighbors(a)` returns outgoing neighbors for directed graphs and all adjacent neighbors for undirected graphs; missing nodes yield an empty iterator. ([Docs.rs][1])

Directed variant:

```rust
use petgraph::Direction::{Incoming, Outgoing};

for dst in g.neighbors_directed(a, Outgoing) {
    println!("a -> {:?}", dst);
}

for src in g.neighbors_directed(a, Incoming) {
    println!("{:?} -> a", src);
}
```

`neighbors_directed` and `edges_directed` are available for directed matrix graphs; `Outgoing` means edges from `a`, and `Incoming` means edges to `a`. ([Docs.rs][1])

### Edges adjacent to node

```rust
for (src, dst, weight) in g.edges(a) {
    println!("{:?} -> {:?}: {:?}", src, dst, weight);
}
```

`edges(a)` yields `(NodeIndex<Ix>, NodeIndex<Ix>, &E)` and follows directed/undirected semantics analogous to `neighbors`. ([Docs.rs][1])

### Trait iteration

`MatrixGraph` implements core graph/visit traits including `GraphBase`, `GraphProp`, `IntoNeighbors`, `IntoEdges`, `IntoEdgesDirected`, `IntoNodeIdentifiers`, `IntoNodeReferences`, `NodeCount`, `EdgeCount`, `NodeIndexable`, `Visitable`, and `GetAdjacencyMatrix`. ([Docs.rs][1])

Generic algorithm shape:

```rust
use petgraph::visit::{IntoNeighbors, Visitable};
use petgraph::visit::Dfs;

fn reachable_count<G>(graph: G, start: G::NodeId) -> usize
where
    G: IntoNeighbors + Visitable,
    G::NodeId: Copy,
{
    let mut dfs = Dfs::new(graph, start);
    let mut count = 0;

    while let Some(_) = dfs.next(graph) {
        count += 1;
    }

    count
}
```

---

## 7.10 Mutation and deletion caveats

### Node removal

```rust
let removed_node_weight = g.remove_node(a);
```

`remove_node` is `O(V)` because edges involving the removed node must be cleared, and it panics if the node does not exist. ([Docs.rs][2])

### Edge removal

```rust
let old = g.try_remove_edge(a, b);

if let Some(weight) = old {
    println!("removed {weight:?}");
}
```

`remove_edge` panics if endpoints or edge are absent; `try_remove_edge` returns `Option<E>`. ([Docs.rs][2])

Boundary rule:

```text
External / untrusted endpoint pairs:
    try_update_edge
    try_remove_edge
    get_edge_weight

Internal invariant path:
    update_edge
    remove_edge
    edge_weight indexing/accessor
```

---

## 7.11 Capacity strategy

### Preallocate by node bound

```rust
use petgraph::matrix_graph::DiMatrix;

let max_nodes = 512;
let mut g = DiMatrix::<&str, u32>::with_capacity(max_nodes);
```

Rationale:

```text
with_capacity(V):
    allocates matrix backing for node capacity
    avoids O(V²) reallocations during edge updates
    appropriate when node upper bound is known
```

`update_edge`, `add_edge`, and add-or-update operations are `O(1)` best case but `O(|V|²)` worst case when the matrix must be reallocated; preallocating the node capacity avoids repeated matrix growth. ([Docs.rs][2])

### Node-count bound discipline

```text
MatrixGraph deployment must have:
    known maximum V
    memory budget for V² slots
    explicit capacity planning
    measured slot type size
```

### Bad deployment

```rust
// Bad: sparse unbounded import into MatrixGraph.
let mut g = DiMatrix::<String, BigEdge>::new();

for rec in unbounded_stream {
    let a = g.add_node(rec.from);
    let b = g.add_node(rec.to);
    g.update_edge(a, b, rec.edge);
}
```

### Better for bounded dense workload

```rust
use petgraph::matrix_graph::DiMatrix;

let n = 256;
let mut g = DiMatrix::<u32, f64>::with_capacity(n);

let nodes: Vec<_> = (0..n).map(|i| g.add_node(i as u32)).collect();

for i in 0..n {
    for j in 0..n {
        if i != j {
            g.update_edge(nodes[i], nodes[j], compute_cost(i, j));
        }
    }
}
```

---

## 7.12 `Option<E>` vs `NotZero<E>` decision table

| Requirement                             |                      Use `Option<E>` | Use `NotZero<E>` |
| --------------------------------------- | -----------------------------------: | ---------------: |
| Zero is valid edge weight               |                                  Yes |               No |
| Edge payload is struct/enum             |                                  Yes |       Usually no |
| Numeric positive weights only           |                           Acceptable |             Good |
| Memory pressure from `Option<E>` layout |                                Maybe |   Good candidate |
| Simplicity / clarity                    |                                 Best |      More subtle |
| Untrusted data may contain zero         |                                Safer |   Validate first |
| Standard `NonZero*` edge type           | `Option<NonZero*>` already efficient |       Not needed |

Example with `NonZeroU32`:

```rust
use petgraph::matrix_graph::DiMatrix;
use std::num::NonZeroU32;

type DensePositive = DiMatrix<&'static str, NonZeroU32>;

let mut g = DensePositive::with_capacity(128);

let a = g.add_node("a");
let b = g.add_node("b");

g.add_edge(a, b, NonZeroU32::new(7).unwrap());
```

Docs state that standard non-zero types such as `NonZeroU32` do not require `NotZero`; keep the default `Null` type argument in that case. ([Docs.rs][4])

---

## 7.13 `MatrixGraph` vs `Graph` vs `GraphMap`

| Requirement               |                `MatrixGraph` |                        `Graph` |                  `GraphMap` |
| ------------------------- | ---------------------------: | -----------------------------: | --------------------------: |
| Dense topology            |                         Best | OK but adjacency-list overhead |   OK for keyed simple graph |
| Sparse topology           |                         Poor |                   Best default | Good for keyed simple graph |
| Edge existence by pair    |             Fast matrix slot |           Local adjacency scan |    Constant-time map lookup |
| Parallel edges            |                           No |                            Yes |                          No |
| Edge identity             |     `(NodeIndex, NodeIndex)` |                    `EdgeIndex` |                    `(N, N)` |
| Node identity             |                  `NodeIndex` |                    `NodeIndex` |                node key `N` |
| Memory                    |                      `O(V²)` |                     `O(V + E)` |                  `O(V + E)` |
| Large edge payload        | Box/Arc strongly recommended |                Direct often OK |             Direct often OK |
| Node count bound required |         Strongly recommended |                             No |                          No |

Selection rule:

```text
MatrixGraph:
    bounded V
    dense E
    hot pair lookup
    no multiedges
    edge payload small or boxed

Graph:
    mutable sparse graph
    multiedges
    broad algorithm support
    compact adjacency-list storage

GraphMap:
    node IDs are Copy keys
    simple graph
    hot contains_edge(a,b)
```

---

## 7.14 Deployment recipes

### Dense numeric cost matrix

```rust
use petgraph::matrix_graph::DiMatrix;

type CostGraph = DiMatrix<u32, f64>;

let mut g = CostGraph::with_capacity(256);
```

Use for:

```text
complete weighted digraph
similarity/cost table
all-pairs optimization
bounded state transitions
```

### Dense positive integer matrix with sentinel

```rust
use petgraph::matrix_graph::{MatrixGraph, NotZero};
use petgraph::Directed;
use std::collections::hash_map::RandomState;

type PositiveCostGraph =
    MatrixGraph<u32, u32, RandomState, Directed, NotZero<u32>>;

let mut g = PositiveCostGraph::with_capacity(512);
```

Use only when `0` is reserved for “no edge.”

### Dense graph with large edge payloads

```rust
use petgraph::matrix_graph::UnMatrix;

#[derive(Debug)]
struct Relation {
    metadata: String,
    samples: Vec<u8>,
}

type DenseRelations = UnMatrix<u32, Box<Relation>>;

let mut g = DenseRelations::with_capacity(128);
```

### Bounded finite-state graph

```rust
use petgraph::matrix_graph::DiMatrix;

#[derive(Clone, Debug)]
enum State {
    Start,
    Auth,
    Done,
}

#[derive(Clone, Debug)]
enum Transition {
    Login,
    Finish,
}

let mut g = DiMatrix::<State, Transition>::with_capacity(16);

let start = g.add_node(State::Start);
let auth = g.add_node(State::Auth);
let done = g.add_node(State::Done);

g.update_edge(start, auth, Transition::Login);
g.update_edge(auth, done, Transition::Finish);
```

---

## 7.15 Anti-pattern inventory

```text
Anti-pattern:
    MatrixGraph for sparse million-node graph.
Problem:
    O(V²) storage.

Anti-pattern:
    MatrixGraph<E = LargeStruct>.
Problem:
    flattened backing array stores edge slots.
Fix:
    Box<E>, Arc<E>, or external EdgeId.

Anti-pattern:
    add_edge used during idempotent imports.
Problem:
    panics on duplicate endpoint pair.
Fix:
    update_edge / try_update_edge.

Anti-pattern:
    NotZero<u32> with zero-weight semantic edges.
Problem:
    zero is absence sentinel.
Fix:
    use Option<u32> or NonZeroU32.

Anti-pattern:
    unbounded streaming construction.
Problem:
    matrix reallocations can be O(V²), memory explodes.
Fix:
    precompute/bound V or use Graph/Csr.

Anti-pattern:
    need multigraph semantics.
Problem:
    one matrix slot per endpoint pair.
Fix:
    use Graph, or store multiplicity bundle inside E.
```

---

## 7.16 Final agent rules

```text
Use MatrixGraph when:
    graph is dense
    node count is bounded
    edge existence / edge lookup by pair is hot
    one edge per endpoint pair is correct
    memory budget includes V² edge slots
    edge payload is small or boxed

Do not use MatrixGraph when:
    graph is sparse
    node count is unbounded
    parallel edges are required
    edge payload is large and stored inline
    streaming incremental topology is primary
    memory footprint must scale with E rather than V²

Default Null:
    Option<E>

Sentinel optimization:
    NotZero<E> only when E: Zero and zero is impossible as real edge weight

Capacity:
    call with_capacity(max_nodes)
    avoid repeated O(V²) growth

Mutation:
    update_edge for normal add/overwrite
    add_edge only when duplicate edge is a bug
    try_* methods at input boundaries
```

[1]: https://docs.rs/petgraph/latest/petgraph/matrix_graph/struct.MatrixGraph.html "MatrixGraph in petgraph::matrix_graph - Rust"
[2]: https://docs.rs/petgraph/latest/src/petgraph/matrix_graph.rs.html "matrix_graph.rs - source"
[3]: https://docs.rs/petgraph/latest/petgraph/matrix_graph/trait.Nullable.html "Nullable in petgraph::matrix_graph - Rust"
[4]: https://docs.rs/petgraph/latest/petgraph/matrix_graph/struct.NotZero.html "NotZero in petgraph::matrix_graph - Rust"


# 8) `Csr` deep dive — compressed sparse row graphs

Format follows the uploaded advanced-reference style. 

`Csr<N, E, Ty, Ix>` is petgraph’s **compressed sparse row** graph: sparse adjacency-matrix representation, arbitrary node weights `N`, arbitrary edge weights `E`, directedness marker `Ty`, index type `Ix`, `O(|V| + |E|)` space, self-loops allowed, no parallel edges, and fast outgoing-edge iteration. ([Docs.rs][1])

---

## 8.0 Type shape

```rust id="p7b3js"
use petgraph::csr::Csr;
use petgraph::{Directed, Undirected};
use petgraph::graph::{DefaultIx, NodeIndex};

pub struct Csr<N = (), E = (), Ty = Directed, Ix = DefaultIx>;
```

`Csr` parameters:

```text id="x3clv7"
N  = node weight type
E  = edge weight type
Ty = Directed | Undirected
Ix = index type controlling maximum graph size
```

Docs define `Csr` as:

```rust id="kqb1xs"
pub struct Csr<N = (), E = (), Ty = Directed, Ix = DefaultIx> { /* private fields */ }
```

and describe it as a compressed sparse row sparse adjacency matrix graph, parameterized by node/edge weights, edge type, and index type. ([Docs.rs][1])

Local aliases:

```rust id="i1z4kb"
use petgraph::csr::Csr;
use petgraph::{Directed, Undirected};
use petgraph::graph::DefaultIx;

type DiCsr<N = (), E = (), Ix = DefaultIx> =
    Csr<N, E, Directed, Ix>;

type UnCsr<N = (), E = (), Ix = DefaultIx> =
    Csr<N, E, Undirected, Ix>;
```

---

## 8.1 CSR mental model: sparse adjacency matrix

CSR conceptual layout:

```text id="akpmi5"
nodes:
    dense node index interval

rows:
    one row per source node

row offsets:
    offset range for each node's outgoing adjacency

columns:
    neighbor node indices for each outgoing edge

edge weights:
    edge weights aligned with columns

edge identity:
    row source + column target + position in row
```

Csr stores sparse adjacency rows instead of per-node linked edge lists or a full matrix. Value case:

```text id="5iz5aq"
MatrixGraph:
    O(V²) slots
    fast endpoint-pair matrix lookup
    dense graph fit

Csr:
    O(V + E) slots
    compact sparse adjacency
    fast outgoing row iteration
    static/imported sparse graph fit

Graph:
    O(V + E) adjacency lists
    mutable workhorse
    easier arbitrary insertion/deletion semantics
```

Petgraph docs explicitly call CSR a sparse adjacency matrix graph using `O(|V| + |E|)` space and optimized for fast outgoing-edge iteration. ([Docs.rs][1])

---

## 8.2 Constructors

### `Csr::new()`

```rust id="os9tjh"
use petgraph::csr::Csr;

let g = Csr::<(), ()>::new();

assert_eq!(g.node_count(), 0);
assert_eq!(g.edge_count(), 0);
```

`new()` creates an empty `Csr`. ([Docs.rs][1])

### `Csr::with_nodes(n)`

```rust id="lu6z9c"
use petgraph::csr::Csr;

let g = Csr::<u8, ()>::with_nodes(5);

assert_eq!(g.node_count(), 5);
assert_eq!(g.edge_count(), 0);

assert_eq!(g[0.into()], 0);
assert_eq!(g[4.into()], 0);
```

`with_nodes(n)` creates `n` nodes and requires `N: Default` because each node gets a default weight. ([Docs.rs][1])

Agent rule:

```text id="mc358o"
Use with_nodes(n) when:
    node count is known
    node weights can be Default
    graph will be populated by NodeIndex endpoints
    imported topology uses compact numeric node IDs
```

Node-weight mutation after construction:

```rust id="dbaxmq"
use petgraph::csr::Csr;
use petgraph::graph::NodeIndex;

let mut g = Csr::<String, ()>::with_nodes(3);

g[NodeIndex::new(0)] = "api".to_owned();
g[NodeIndex::new(1)] = "db".to_owned();
g[NodeIndex::new(2)] = "cache".to_owned();
```

`Csr` implements indexing and mutable indexing for node weights by `NodeIndex<Ix>`. ([Docs.rs][1])

---

## 8.3 `from_sorted_edges`: preferred bulk construction

### Unweighted sorted edge list

```rust id="i1e5h3"
use petgraph::csr::Csr;
use petgraph::prelude::*;

let edges = [
    (0, 1),
    (0, 2),
    (1, 0),
    (1, 2),
    (1, 3),
    (2, 0),
    (3, 1),
];

let g = Csr::<(), ()>::from_sorted_edges(&edges)
    .expect("edges must be sorted and unique by (u, v)");

assert_eq!(g.node_count(), 4);
```

### Weighted sorted edge list

```rust id="255q6e"
use petgraph::csr::Csr;
use petgraph::prelude::*;

let edges = [
    (0, 1, 10u32),
    (0, 2, 20u32),
    (1, 2, 30u32),
];

let g = Csr::<(), u32>::from_sorted_edges(&edges)
    .expect("sorted unique weighted edges required");
```

`from_sorted_edges` creates a `Csr` from a sorted edge sequence; edges must be sorted and unique by Rust’s default pair order `(u, v)` with `u` first, and construction runs in `O(|V| + |E|)` time. ([Docs.rs][1])

Required sort order:

```text id="osyp9j"
valid order:
    (0, 1)
    (0, 2)
    (1, 0)
    (1, 2)
    (1, 3)
    (2, 0)

invalid:
    (1, 0)
    (0, 2)   // source row decreased
```

Dedup/sort recipe:

```rust id="h2j99c"
use petgraph::csr::Csr;

fn build_csr(mut edges: Vec<(usize, usize, u32)>) -> Csr<(), u32> {
    edges.sort_by_key(|&(u, v, _w)| (u, v));

    // Keep first edge per pair. Alternative: aggregate weights before dedup.
    edges.dedup_by_key(|edge| (edge.0, edge.1));

    Csr::<(), u32>::from_sorted_edges(&edges)
        .expect("sort + dedup should satisfy Csr requirements")
}
```

Weighted aggregation before build:

```rust id="w1adgd"
use std::collections::BTreeMap;
use petgraph::csr::Csr;

fn aggregate_to_csr(raw: impl IntoIterator<Item = (usize, usize, u32)>)
    -> Csr<(), u32>
{
    let mut agg = BTreeMap::<(usize, usize), u32>::new();

    for (u, v, w) in raw {
        *agg.entry((u, v)).or_insert(0) += w;
    }

    let edges: Vec<_> = agg
        .into_iter()
        .map(|((u, v), w)| (u, v, w))
        .collect();

    Csr::<(), u32>::from_sorted_edges(&edges)
        .expect("BTreeMap emits sorted unique keys")
}
```

Agent rule:

```text id="ktdtvx"
Prefer from_sorted_edges for:
    batch import
    edge-list files
    mostly static sparse graph
    deterministic build
    large graph construction

Avoid raw add_edge loop when:
    edge list is already available
    edge count is large
    batch sort/dedup is cheap relative to incremental insertion
```

---

## 8.4 Sorted/unique requirement

Formal requirement:

```text id="jx7904"
for all consecutive edges:
    (u_i, v_i) < (u_{i+1}, v_{i+1})

no duplicate:
    same (u, v) endpoint pair cannot appear twice
```

Failure type:

```rust id="49ylwr"
use petgraph::csr::{Csr, EdgesNotSorted};

let result: Result<Csr<(), ()>, EdgesNotSorted> =
    Csr::<(), ()>::from_sorted_edges(&[(1, 0), (0, 1)]);

assert!(result.is_err());
```

Docs state `from_sorted_edges` returns `Result<Self, EdgesNotSorted>` and requires sorted unique edges. ([Docs.rs][1])

Best-practice import pipeline:

```text id="ri5as4"
raw edges
    -> normalize endpoints
    -> validate node ID bounds / remap external IDs to dense NodeIndex
    -> sort by (source, target)
    -> aggregate/dedup duplicate pairs
    -> from_sorted_edges
    -> run traversal/algorithm workload
```

---

## 8.5 Fast outgoing-edge iteration

CSR’s core advantage is contiguous row slices for outgoing adjacency.

### `out_degree(a) -> usize`

```rust id="gpgxfg"
use petgraph::csr::Csr;
use petgraph::graph::NodeIndex;

let g = Csr::<(), ()>::from_sorted_edges(&[
    (0, 1),
    (0, 2),
    (1, 2),
]).unwrap();

let a = NodeIndex::new(0);

assert_eq!(g.out_degree(a), 2);
```

`out_degree(a)` computes in `O(1)` and panics if node `a` does not exist. ([Docs.rs][1])

### `neighbors_slice(a) -> &[NodeIndex<Ix>]`

```rust id="3m4eab"
use petgraph::graph::NodeIndex;

let a = NodeIndex::new(0);

let neighbors = g.neighbors_slice(a);

assert_eq!(neighbors, &[NodeIndex::new(1), NodeIndex::new(2)]);
```

`neighbors_slice(a)` computes in `O(1)` and returns the outgoing neighbor slice for a node; it panics if the node does not exist. ([Docs.rs][1])

### `edges_slice(a) -> &[E]`

```rust id="jcg8j6"
use petgraph::csr::Csr;
use petgraph::graph::NodeIndex;

let g = Csr::<(), u32>::from_sorted_edges(&[
    (0, 1, 10),
    (0, 2, 20),
    (1, 2, 30),
]).unwrap();

let weights = g.edges_slice(NodeIndex::new(0));

assert_eq!(weights, &[10, 20]);
```

`edges_slice(a)` computes in `O(1)` and returns the edge-weight slice aligned with `neighbors_slice(a)`. ([Docs.rs][1])

### Slice zip pattern

```rust id="4gwhll"
use petgraph::graph::NodeIndex;

let row = NodeIndex::new(0);

for (&dst, weight) in g.neighbors_slice(row).iter().zip(g.edges_slice(row)) {
    println!("{:?} -> {:?}: {:?}", row, dst, weight);
}
```

This is the high-performance CSR row idiom:

```text id="5gfxbc"
neighbors_slice(a)[i] corresponds to edges_slice(a)[i]
row is contiguous
no iterator allocation
ideal for traversal kernels
```

### `edges(a)`

```rust id="aqv6ql"
use petgraph::visit::EdgeRef;
use petgraph::graph::NodeIndex;

for edge in g.edges(NodeIndex::new(0)) {
    println!(
        "{:?} -> {:?}: {:?}",
        edge.source(),
        edge.target(),
        edge.weight()
    );
}
```

`edges(a)` returns an iterator over all edges of `a`; for directed graphs these are outgoing edges, and for undirected graphs all connected edges. It panics if `a` does not exist. ([Docs.rs][1])

---

## 8.6 Edge existence lookup

```rust id="elat6d"
use petgraph::graph::NodeIndex;

let a = NodeIndex::new(0);
let b = NodeIndex::new(2);

if g.contains_edge(a, b) {
    println!("edge exists");
}
```

`contains_edge(a, b)` computes in `O(log |V|)` time according to the docs and panics if source node `a` does not exist. ([Docs.rs][1])

Interpretation:

```text id="utx6hq"
Csr is not primarily a pair-lookup structure.
Csr is primarily a row-traversal structure.
For extremely hot contains_edge(a,b):
    GraphMap or MatrixGraph may be better.
For scanning all outgoing edges of a:
    Csr is excellent.
```

---

## 8.7 Mutation APIs and construction cost

### `add_node(weight) -> NodeIndex<Ix>`

```rust id="6chjtr"
use petgraph::csr::Csr;

let mut g = Csr::<String, u32>::new();

let a = g.add_node("api".to_owned());
let b = g.add_node("db".to_owned());
```

`add_node` adds a node with the given weight and returns its node index. ([Docs.rs][1])

### `add_edge(a, b, weight) -> bool`

```rust id="b3lv2a"
use petgraph::csr::Csr;
use petgraph::graph::NodeIndex;

let mut g = Csr::<(), u32>::with_nodes(3);

let added = g.add_edge(NodeIndex::new(0), NodeIndex::new(1), 10);

assert!(added);
assert!(g.contains_edge(NodeIndex::new(0), NodeIndex::new(1)));
```

`add_edge` adds an edge and returns `true` if the edge was added; it requires `E: Clone`, panics if endpoints are out of bounds, and docs state that adding all edges in row-major order costs `O(|V| · |E|)` for the whole operation. ([Docs.rs][1])

Duplicate edge behavior:

```rust id="h9e6g0"
let first = g.add_edge(NodeIndex::new(0), NodeIndex::new(1), 10);
let second = g.add_edge(NodeIndex::new(0), NodeIndex::new(1), 20);

assert!(first);
assert!(!second); // no parallel edge inserted
```

### `try_add_edge(a, b, weight) -> Result<bool, CsrError>`

```rust id="wcobqn"
use petgraph::csr::{Csr, CsrError};
use petgraph::graph::NodeIndex;

let mut g = Csr::<(), u32>::with_nodes(2);

let result: Result<bool, CsrError> =
    g.try_add_edge(NodeIndex::new(0), NodeIndex::new(1), 7);

assert_eq!(result, Ok(true));
```

`try_add_edge` is the fallible edge insertion API; it returns `true` when an edge was added and can return `CsrError::IndicesOutBounds` when endpoints are out of bounds. ([Docs.rs][1])

### Row-major insertion advisory

```text id="xm9n4k"
If incrementally adding edges to Csr:
    add in row-major order:
        source asc
        target asc within source

But:
    even row-major add_edge loop is documented as O(V * E) for whole insertion set.
    from_sorted_edges is O(V + E).
```

Construction choice:

```text id="7vhba3"
Large batch:
    sort/dedup + from_sorted_edges

Small incremental patch:
    try_add_edge may be acceptable

Heavy online mutation:
    use Graph / GraphMap, then convert/rebuild Csr for traversal phase
```

---

## 8.8 Self-loops allowed; no parallel edges

Docs state directly: self-loops are allowed; no parallel edges. ([Docs.rs][1])

### Self-loop

```rust id="9fgx2x"
use petgraph::csr::Csr;
use petgraph::graph::NodeIndex;

let mut g = Csr::<(), ()>::with_nodes(2);

assert!(g.add_edge(NodeIndex::new(0), NodeIndex::new(0), ()));
assert!(g.contains_edge(NodeIndex::new(0), NodeIndex::new(0)));
```

### No parallel edge

```rust id="is1q5l"
use petgraph::csr::Csr;
use petgraph::graph::NodeIndex;

let mut g = Csr::<(), u32>::with_nodes(2);

assert!(g.add_edge(NodeIndex::new(0), NodeIndex::new(1), 10));
assert!(!g.add_edge(NodeIndex::new(0), NodeIndex::new(1), 20));

assert_eq!(g.edge_count(), 1);
```

Modeling multiplicity inside `E`:

```rust id="fw54mc"
use petgraph::csr::Csr;

#[derive(Clone, Debug, Default)]
struct EdgeAgg {
    count: u64,
    total_weight: u64,
}

// Aggregate duplicates before building CSR.
let raw = [(0usize, 1usize, 5u64), (0, 1, 7)];
```

Rule:

```text id="gpzw2a"
Use Csr when:
    endpoint pair identifies edge
    duplicate pairs should be deduplicated or aggregated
    self-loop support is acceptable

Avoid Csr when:
    parallel edges are semantic records
    edge insertion order/event multiplicity matters
    edge identity must be separate from endpoint pair
```

---

## 8.9 Undirected CSR convention: store both directions

For undirected CSR construction, docs explicitly require that edges be present in both directions: `(u, v)` requires `(v, u)` also in the sequence. ([Docs.rs][1])

### Correct undirected construction

```rust id="uryv7r"
use petgraph::csr::Csr;
use petgraph::Undirected;

let edges = [
    (0, 1),
    (1, 0),
    (1, 2),
    (2, 1),
];

let g = Csr::<(), (), Undirected>::from_sorted_edges(&edges)
    .expect("sorted unique symmetric edges required");
```

### Symmetrization helper

```rust id="fypq3u"
use petgraph::csr::Csr;
use petgraph::Undirected;

fn undirected_csr_from_edges(
    raw: impl IntoIterator<Item = (usize, usize)>,
) -> Csr<(), (), Undirected> {
    let mut edges = Vec::<(usize, usize)>::new();

    for (u, v) in raw {
        edges.push((u, v));

        if u != v {
            edges.push((v, u));
        }
    }

    edges.sort();
    edges.dedup();

    Csr::<(), (), Undirected>::from_sorted_edges(&edges)
        .expect("symmetrized edges are sorted unique")
}
```

Weighted undirected symmetrization:

```rust id="es75ul"
use petgraph::csr::Csr;
use petgraph::Undirected;

fn undirected_weighted_csr(
    raw: impl IntoIterator<Item = (usize, usize, u32)>,
) -> Csr<(), u32, Undirected> {
    let mut edges = Vec::<(usize, usize, u32)>::new();

    for (u, v, w) in raw {
        edges.push((u, v, w));

        if u != v {
            edges.push((v, u, w));
        }
    }

    edges.sort_by_key(|&(u, v, _)| (u, v));
    edges.dedup_by_key(|edge| (edge.0, edge.1));

    Csr::<(), u32, Undirected>::from_sorted_edges(&edges)
        .expect("symmetrized weighted edges are sorted unique")
}
```

Agent rule:

```text id="n6mgax"
Undirected Csr is not built from one edge per unordered pair.
It expects adjacency rows for both endpoint directions.
For self-loop (u,u):
    add once.
For non-self edge (u,v):
    add both (u,v) and (v,u).
```

---

## 8.10 Index and trait behavior

`Csr` implements graph traits including `Data`, `GraphBase`, `GraphProp`, `IntoNeighbors`, `IntoEdges`, `IntoEdgeReferences`, `IntoNodeIdentifiers`, `IntoNodeReferences`, `NodeCount`, `EdgeCount`, `NodeIndexable`, `NodeCompactIndexable`, `Visitable`, and `GetAdjacencyMatrix`. ([Docs.rs][1])

Important implications:

```text id="d4qi3q"
Csr has compact node IDs:
    implements NodeCompactIndexable

Csr supports traversal traits:
    usable in many trait-generic algorithms

Csr is read/traversal-oriented:
    no remove_node
    no remove_edge
    no update_edge
    clear_edges exists
```

`GetAdjacencyMatrix` for `&Csr` computes a bitmap adjacency matrix using `FixedBitSet`; this is derived on demand by `.adjacency_matrix()`, not the primary storage layout. ([Docs.rs][1])

Generic traversal example:

```rust id="fw6gzr"
use petgraph::visit::{Dfs, IntoNeighbors, Visitable};

fn reachable_count<G>(graph: G, start: G::NodeId) -> usize
where
    G: IntoNeighbors + Visitable,
    G::NodeId: Copy,
{
    let mut dfs = Dfs::new(graph, start);
    let mut count = 0;

    while let Some(_) = dfs.next(graph) {
        count += 1;
    }

    count
}
```

---

## 8.11 Imported edge-list deployment pattern

### External string IDs → compact CSR

```rust id="s9sf4h"
use std::collections::HashMap;
use petgraph::csr::Csr;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct DenseId(usize);

fn intern(
    ids: &mut HashMap<String, usize>,
    labels: &mut Vec<String>,
    label: &str,
) -> usize {
    if let Some(&id) = ids.get(label) {
        id
    } else {
        let id = labels.len();
        ids.insert(label.to_owned(), id);
        labels.push(label.to_owned());
        id
    }
}

fn build_from_string_edges(
    raw: impl IntoIterator<Item = (String, String, u32)>,
) -> (Csr<String, u32>, Vec<String>) {
    let mut ids = HashMap::<String, usize>::new();
    let mut labels = Vec::<String>::new();
    let mut edges = Vec::<(usize, usize, u32)>::new();

    for (src, dst, weight) in raw {
        let u = intern(&mut ids, &mut labels, &src);
        let v = intern(&mut ids, &mut labels, &dst);
        edges.push((u, v, weight));
    }

    edges.sort_by_key(|&(u, v, _)| (u, v));
    edges.dedup_by_key(|edge| (edge.0, edge.1));

    let mut csr = Csr::<String, u32>::from_sorted_edges(&edges)
        .expect("sorted/deduped edges");

    for (i, label) in labels.iter().enumerate() {
        csr[i.into()] = label.clone();
    }

    (csr, labels)
}
```

Deployment rule:

```text id="j74mpf"
Csr construction wants dense numeric node IDs.
For external IDs:
    remap external ID -> usize
    preserve reverse Vec for labels/payload
    build sorted unique edge list
```

---

## 8.12 CSR vs Graph vs GraphMap vs MatrixGraph

| Requirement                |                `Csr` |     `Graph` | `GraphMap` |                      `MatrixGraph` |               |                    |
| -------------------------- | -------------------: | ----------: | ---------: | ---------------------------------: | ------------- | ------------------ |
| Mostly static sparse graph |                 Best |        Good |       Good |                               Poor |               |                    |
| Fast outgoing row scan     |                 Best |        Good |       Good | Matrix-row scan possible but dense |               |                    |
| Online arbitrary insertion |                 Weak |        Best |       Good |                 Bounded dense only |               |                    |
| Remove nodes/edges         | Not primary / absent |         Yes | Yes by key |           Limited matrix semantics |               |                    |
| Parallel edges             |                   No |         Yes |         No |                                 No |               |                    |
| Node identity              |      compact indices | `NodeIndex` |    key `N` |                        `NodeIndex` |               |                    |
| Edge existence lookup      |               `O(log |           V |    )` docs |               local adjacency scan | constant-time | matrix-slot lookup |
| Dense graph                |          Not primary |          OK |         OK |                               Best |               |                    |
| Imported sorted edge list  |            Excellent |        Good |       Good |                        Usually not |               |                    |
| Memory scaling             |             `O(V+E)` |    `O(V+E)` |   `O(V+E)` |                            `O(V²)` |               |                    |

Selection:

```text id="x2rtgb"
Choose Csr:
    static sparse graph
    imported edge list
    repeated outgoing-neighbor traversal
    no multiedges
    dense compact node IDs

Choose Graph:
    mutable sparse graph
    incremental edits
    multiedges
    broad API

Choose GraphMap:
    node IDs are Copy keys
    hot contains_edge(a,b)
    simple graph

Choose MatrixGraph:
    dense graph
    bounded V
    hot pair lookup
```

---

## 8.13 Construction-performance rules

```text id="53n2tk"
Best:
    from_sorted_edges:
        O(V + E)
        requires sorted unique edges
        ideal for batch/import

Acceptable:
    with_nodes + row-major try_add_edge:
        documented whole-operation O(V * E)
        OK for small patch sets
        not ideal for large batch

Poor:
    unsorted incremental add_edge at scale
        sort/dedup first instead
```

Docs state `from_sorted_edges` computes in `O(|V| + |E|)`, while adding all edges in row-major order with `add_edge` / `try_add_edge` costs `O(|V| · |E|)` for the whole operation. ([Docs.rs][1])

---

## 8.14 Boundary-safe API choices

```text id="1h8ofx"
Input is trusted/internal:
    add_edge
    contains_edge
    edges
    neighbors_slice
    edges_slice

Input is external/untrusted:
    validate NodeIndex raw values
    use try_add_edge
    avoid panicking accessors
    check node_count bounds before NodeIndex::new
```

Safe raw-index wrapper:

```rust id="zgvbx0"
use petgraph::csr::{Csr, CsrError};
use petgraph::graph::NodeIndex;

fn try_add_raw_edge<N, E: Clone, Ty, Ix>(
    g: &mut Csr<N, E, Ty, Ix>,
    u: usize,
    v: usize,
    w: E,
) -> Result<bool, CsrError>
where
    Ty: petgraph::EdgeType,
    Ix: petgraph::graph::IndexType,
{
    let a = NodeIndex::<Ix>::new(u);
    let b = NodeIndex::<Ix>::new(v);
    g.try_add_edge(a, b, w)
}
```

---

## 8.15 Best-fit workloads

```text id="h9b2jt"
Ideal:
    PageRank-like repeated outgoing scans
    graph analytics over static sparse topology
    imported web/link graph
    static dependency graph after preprocessing
    finite graph generated from sorted edge list
    offline batch algorithms
    graph6-style undirected topology workloads

Acceptable:
    small incremental edits before traversal
    sparse finite-state graph
    simple graph with no duplicate edge semantics

Bad:
    online graph editor
    deletion-heavy mutable graph
    event multigraph
    dense graph
    unknown/unbounded node count
    edge-existence lookup dominates more than neighbor scans
```

---

## 8.16 Anti-pattern inventory

```text id="jsz0px"
Anti-pattern:
    Csr from unsorted raw edge stream.
Problem:
    from_sorted_edges requires sorted unique edges.
Fix:
    sort_by_key((u,v)) + dedup/aggregate.

Anti-pattern:
    Undirected Csr with only (u,v), not (v,u).
Problem:
    docs require both directions for undirected construction.
Fix:
    symmetrize non-self edges.

Anti-pattern:
    Repeated large-scale add_edge build.
Problem:
    documented O(V * E) whole-operation cost even in row-major order.
Fix:
    from_sorted_edges.

Anti-pattern:
    Parallel edge event log in Csr.
Problem:
    no parallel edges.
Fix:
    Graph, or aggregate duplicates into E.

Anti-pattern:
    Use Csr for deletion-heavy online workload.
Problem:
    CSR is static/traversal-oriented.
Fix:
    Graph/StableGraph during mutation; rebuild Csr for analysis.

Anti-pattern:
    Hot pair lookup workload.
Problem:
    Csr contains_edge is not its main strength.
Fix:
    GraphMap or MatrixGraph depending density/key model.

Anti-pattern:
    External String IDs directly as node identity.
Problem:
    Csr uses compact NodeIndex-style nodes.
Fix:
    remap external IDs to dense usize, store labels separately or as node weights.
```

---

## 8.17 Final agent rules

```text id="60fnvg"
Use Csr when:
    graph is sparse
    graph is mostly static
    edge list can be sorted/deduplicated
    outgoing row traversal is the hot path
    no parallel edges required
    compact dense node IDs are available
    memory must scale as O(V + E)

Construct Csr by:
    with_nodes(n) for small/manual construction
    from_sorted_edges(&edges) for bulk construction

Guarantee before from_sorted_edges:
    sorted by (u, v)
    unique by (u, v)
    node weights can be Default
    undirected edges are symmetric: (u,v) and (v,u)

Avoid Csr when:
    arbitrary mutation dominates
    deletion is required
    multiedges are required
    dense graph pair lookup dominates
    node IDs are rich external values without remapping

Core value case:
    compact sparse row storage
    O(1) row-slice access
    fast outgoing-edge scans
    ideal for imported static sparse graphs
```

[1]: https://docs.rs/petgraph/latest/petgraph/csr/struct.Csr.html "Csr in petgraph::csr - Rust"


# 9) Directedness, edge semantics, and graph invariants — petgraph

Format follows the uploaded advanced-reference style. 

Petgraph models directedness at the **graph type level** via `Directed` / `Undirected` and `EdgeType`; traversal direction is expressed at runtime via `Direction::{Outgoing, Incoming}`. `EdgeType::is_directed()` tells whether a graph has directed edges; `Direction::Outgoing` means outward from the current node and `Direction::Incoming` means inbound to the current node. ([Docs.rs][1])

---

## 9.0 Core imports

```rust
use petgraph::{
    Directed,
    Undirected,
    Direction,
    EdgeType,
};

use petgraph::Direction::{
    Incoming,
    Outgoing,
};

use petgraph::graph::{
    Graph,
    DiGraph,
    UnGraph,
    NodeIndex,
    EdgeIndex,
};

use petgraph::stable_graph::{
    StableDiGraph,
    StableUnGraph,
};

use petgraph::graphmap::{
    DiGraphMap,
    UnGraphMap,
};

use petgraph::matrix_graph::{
    DiMatrix,
    UnMatrix,
};

use petgraph::csr::Csr;
```

---

## 9.1 Type-level directedness: `Directed`, `Undirected`, `EdgeType`

Type shape:

```rust
use petgraph::{Directed, Undirected, EdgeType};
use petgraph::graph::Graph;

type DependencyGraph<N, E> = Graph<N, E, Directed>;
type LinkGraph<N, E> = Graph<N, E, Undirected>;
```

`EdgeType`:

```rust
pub trait EdgeType {
    fn is_directed() -> bool;
}
```

`Directed` and `Undirected` implement `EdgeType`; the trait is petgraph’s type-level directedness switch. It is not object-safe / dyn-compatible. ([Docs.rs][1])

Generic directedness inspection:

```rust
use petgraph::EdgeType;

fn describe_ty<Ty: EdgeType>() -> &'static str {
    if Ty::is_directed() {
        "directed"
    } else {
        "undirected"
    }
}

assert_eq!(describe_ty::<petgraph::Directed>(), "directed");
assert_eq!(describe_ty::<petgraph::Undirected>(), "undirected");
```

Graph generic over direction:

```rust
use petgraph::{EdgeType, Graph};

fn edge_semantics<N, E, Ty, Ix>(g: &Graph<N, E, Ty, Ix>) -> &'static str
where
    Ty: EdgeType,
    Ix: petgraph::graph::IndexType,
{
    if g.is_directed() {
        "ordered endpoint pairs"
    } else {
        "unordered adjacency relation"
    }
}
```

Agent rule:

```text
Directedness is not a per-edge runtime field.
Directedness is graph-level type parameter Ty.
Use Direction only to choose Incoming/Outgoing traversal direction.
```

---

## 9.2 Runtime traversal direction: `Direction::{Outgoing, Incoming}`

`Direction`:

```rust
#[repr(usize)]
pub enum Direction {
    Outgoing = 0,
    Incoming = 1,
}
```

`Outgoing` is an outward edge from the current node; `Incoming` is an inbound edge to the current node. `Direction::opposite()` flips direction; `Direction::index()` returns `0` for `Outgoing`, `1` for `Incoming`. ([Docs.rs][2])

```rust
use petgraph::Direction::{Incoming, Outgoing};

assert_eq!(Outgoing.opposite(), Incoming);
assert_eq!(Incoming.opposite(), Outgoing);

assert_eq!(Outgoing.index(), 0);
assert_eq!(Incoming.index(), 1);
```

Direction-array idiom:

```rust
use petgraph::Direction::{Incoming, Outgoing};

let mut counts = [0usize; 2];

counts[Outgoing.index()] += 1;
counts[Incoming.index()] += 1;
```

Direction parameterization pattern:

```rust
use petgraph::Direction;
use petgraph::graph::{Graph, NodeIndex};

fn degree_in_direction<N, E, Ty, Ix>(
    g: &Graph<N, E, Ty, Ix>,
    n: NodeIndex<Ix>,
    dir: Direction,
) -> usize
where
    Ty: petgraph::EdgeType,
    Ix: petgraph::graph::IndexType,
{
    g.neighbors_directed(n, dir).count()
}
```

---

## 9.3 Neighbor semantics: directed vs undirected

For `Graph`, `neighbors(a)` means outgoing neighbors on directed graphs and all adjacent neighbors on undirected graphs. `neighbors_directed(a, dir)` means outgoing or incoming neighbors on directed graphs; for undirected graphs it is equivalent to `neighbors(a)`. Missing nodes produce empty iterators. ([Docs.rs][3])

```rust
use petgraph::graph::DiGraph;
use petgraph::Direction::{Incoming, Outgoing};

let mut g = DiGraph::<&str, ()>::new();

let a = g.add_node("a");
let b = g.add_node("b");
let c = g.add_node("c");

g.add_edge(a, b, ());
g.add_edge(c, a, ());

let outgoing: Vec<_> = g.neighbors_directed(a, Outgoing).collect();
let incoming: Vec<_> = g.neighbors_directed(a, Incoming).collect();

assert_eq!(outgoing, vec![b]);
assert_eq!(incoming, vec![c]);
```

Undirected semantics:

```rust
use petgraph::graph::UnGraph;
use petgraph::Direction::{Incoming, Outgoing};

let mut g = UnGraph::<&str, ()>::new_undirected();

let a = g.add_node("a");
let b = g.add_node("b");

g.add_edge(a, b, ());

let n1: Vec<_> = g.neighbors(a).collect();
let n2: Vec<_> = g.neighbors_directed(a, Outgoing).collect();
let n3: Vec<_> = g.neighbors_directed(a, Incoming).collect();

assert_eq!(n1, n2);
assert_eq!(n2, n3);
```

Agent rule:

```text
Directed Graph:
    neighbors(a) == outgoing neighbors from a
    neighbors_directed(a, Outgoing) == outgoing
    neighbors_directed(a, Incoming) == incoming
    neighbors_undirected(a) == both outgoing then incoming

Undirected Graph:
    neighbors(a) == all adjacent nodes
    neighbors_directed(a, Outgoing) == all adjacent nodes
    neighbors_directed(a, Incoming) == all adjacent nodes
```

`neighbors_undirected(a)` returns all neighbors connected in either direction; for directed graphs, outgoing neighbors are listed before incoming neighbors, each in reverse edge-addition order. ([Docs.rs][3])

---

## 9.4 Edge iterator semantics

For `Graph::edges(a)`, directed graphs yield outgoing edges from `a`; undirected graphs yield all edges connected to `a`. `edges_directed(a, dir)` yields directed incoming/outgoing edges for directed graphs; for undirected graphs, `Outgoing` yields connected edges with `a` as source and `Incoming` yields connected edges with `a` as target in the returned references. ([Docs.rs][3])

```rust
use petgraph::graph::DiGraph;
use petgraph::visit::EdgeRef;
use petgraph::Direction::{Incoming, Outgoing};

let mut g = DiGraph::<&str, u32>::new();

let a = g.add_node("a");
let b = g.add_node("b");
let c = g.add_node("c");

g.add_edge(a, b, 10);
g.add_edge(c, a, 20);

for e in g.edges_directed(a, Outgoing) {
    assert_eq!(e.source(), a);
}

for e in g.edges_directed(a, Incoming) {
    assert_eq!(e.target(), a);
}
```

Undirected orientation in returned `EdgeReference`:

```rust
use petgraph::graph::UnGraph;
use petgraph::visit::EdgeRef;
use petgraph::Direction::{Incoming, Outgoing};

let mut g = UnGraph::<&str, u32>::new_undirected();

let a = g.add_node("a");
let b = g.add_node("b");

g.add_edge(a, b, 7);

for e in g.edges_directed(a, Outgoing) {
    assert_eq!(e.source(), a);
}

for e in g.edges_directed(a, Incoming) {
    assert_eq!(e.target(), a);
}
```

Agent rule:

```text
Undirected graph still has endpoint ordering internally/for references.
Do not interpret EdgeRef::source()/target() in an undirected graph as semantic direction.
Use it only as iteration orientation / endpoint normalization artifact.
```

---

## 9.5 Edge orientation in undirected graphs

Undirected edge semantics:

```text
semantic relation:
    edge(a, b) == edge(b, a)

internal/reference orientation:
    APIs may orient edge references relative to query direction
    add_edge(a, b, w) stores endpoints in an order
    edges_directed can emit references with a as source or target
```

`Graph::edges_directed` documentation explicitly says that for undirected graphs, `Outgoing` returns connected edges with the queried node as source, while `Incoming` returns connected edges with the queried node as target; ordering is based on the order endpoints were listed when adding the edge. ([Docs.rs][3])

Endpoint normalization helper:

```rust
use petgraph::graph::NodeIndex;

fn unordered_pair<Ix: petgraph::graph::IndexType>(
    a: NodeIndex<Ix>,
    b: NodeIndex<Ix>,
) -> (NodeIndex<Ix>, NodeIndex<Ix>) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}
```

Use this when building undirected-edge maps:

```rust
use std::collections::HashMap;
use petgraph::graph::{UnGraph, NodeIndex};

let mut edge_meta: HashMap<(NodeIndex, NodeIndex), String> = HashMap::new();

let mut g = UnGraph::<&str, ()>::new_undirected();

let a = g.add_node("a");
let b = g.add_node("b");

g.add_edge(a, b, ());
edge_meta.insert(unordered_pair(a, b), "physical link".to_owned());
```

---

## 9.6 Self-loops and parallel edges by graph type

| Graph family  |                                                                 Self-loops | Parallel edges | Edge identity                                     |
| ------------- | -------------------------------------------------------------------------: | -------------: | ------------------------------------------------- |
| `Graph`       |         Allowed by API / generated arbitrary graphs may include self-loops |            Yes | `EdgeIndex<Ix>`                                   |
| `StableGraph` |         Allowed by API / generated arbitrary graphs may include self-loops |            Yes | stable-ish `EdgeIndex<Ix>` for unrelated removals |
| `GraphMap`    |                                                                        Yes |             No | endpoint pair `(N, N)`                            |
| `MatrixGraph` | Endpoint-pair slot supports diagonal usage by API shape; no duplicate slot |             No | endpoint pair `(NodeIndex, NodeIndex)`            |
| `Csr`         |                                                                        Yes |             No | CSR row/column endpoint pair                      |

`Graph::add_edge` and `StableGraph::add_edge` insert a new edge and allow parallel duplicate edges; both provide `update_edge` to add-or-update a single logical endpoint-pair edge. `GraphMap` explicitly disallows parallel edges but allows self-loops; `MatrixGraph::add_edge` panics if an edge already exists and says duplicate edges are not allowed; `Csr` explicitly allows self-loops and disallows parallel edges. ([Docs.rs][3])

Parallel edge demo:

```rust
use petgraph::graph::DiGraph;

let mut g = DiGraph::<&str, &'static str>::new();

let a = g.add_node("a");
let b = g.add_node("b");

let e1 = g.add_edge(a, b, "call");
let e2 = g.add_edge(a, b, "owns");

assert_ne!(e1, e2);
assert_eq!(g.edge_count(), 2);
```

Simple-edge overwrite demo:

```rust
use petgraph::graph::DiGraph;

let mut g = DiGraph::<&str, u32>::new();

let a = g.add_node("a");
let b = g.add_node("b");

let e1 = g.update_edge(a, b, 10);
let e2 = g.update_edge(a, b, 20);

assert_eq!(e1, e2);
assert_eq!(g.edge_count(), 1);
assert_eq!(g[e1], 20);
```

GraphMap overwrite semantics:

```rust
use petgraph::graphmap::DiGraphMap;

let mut g = DiGraphMap::<u64, u32>::new();

assert_eq!(g.add_edge(1, 2, 10), None);
assert_eq!(g.add_edge(1, 2, 20), Some(10));
assert_eq!(g.edge_count(), 1);
```

`GraphMap::add_edge` inserts missing endpoint nodes; for a directed graph, the edge is from `a` to `b`; repeated insertion updates the edge data and returns the old weight. ([Docs.rs][4])

---

## 9.7 `reverse`: in-place edge direction reversal

`Graph::reverse(&mut self)` reverses the direction of all edges. ([Docs.rs][3])

```rust
use petgraph::graph::DiGraph;

let mut g = DiGraph::<&str, ()>::new();

let a = g.add_node("a");
let b = g.add_node("b");

g.add_edge(a, b, ());

assert!(g.contains_edge(a, b));
assert!(!g.contains_edge(b, a));

g.reverse();

assert!(!g.contains_edge(a, b));
assert!(g.contains_edge(b, a));
```

StableGraph also exposes `reverse` in its method surface. ([Docs.rs][5])

Agent rule:

```text
reverse():
    in-place topology orientation flip
    preserves node/edge weights
    useful for reverse reachability, reverse dependency queries, backward dataflow

Use adapter instead when:
    original orientation must remain unchanged
    temporary algorithm view is enough
```

---

## 9.8 `into_edge_type`: type-level conversion without edge adjustment

`Graph::into_edge_type<NewTy>(self)` converts a `Graph<N, E, Ty, Ix>` into `Graph<N, E, NewTy, Ix>` in `O(1)`, but it performs **no edge adjustments**; docs explicitly warn that you may need to remove or add edges after conversion. ([Docs.rs][3])

```rust
use petgraph::graph::{DiGraph, UnGraph};
use petgraph::Undirected;

let mut dg = DiGraph::<&str, ()>::new();

let a = dg.add_node("a");
let b = dg.add_node("b");

dg.add_edge(a, b, ());

let ug: UnGraph<&str, ()> = dg.into_edge_type::<Undirected>();
```

Critical semantic warning:

```text
Directed -> Undirected:
    a -> b becomes an undirected adjacency between a and b
    reciprocal pairs may become parallel edges if both a->b and b->a existed in Graph
    no deduplication is performed

Undirected -> Directed:
    stored endpoint order becomes directed edge orientation
    semantic direction may be arbitrary unless you intentionally encoded endpoint order
    no reciprocal edges are added
```

Deduplicate after directed-to-undirected conversion:

```rust
use std::collections::HashSet;
use petgraph::graph::{DiGraph, UnGraph, EdgeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Undirected;

fn directed_to_simple_undirected<N: Clone, E: Clone>(
    dg: DiGraph<N, E>,
) -> UnGraph<N, E> {
    let mut ug: UnGraph<N, E> = dg.into_edge_type::<Undirected>();

    let mut seen = HashSet::new();
    let mut remove = Vec::<EdgeIndex>::new();

    for e in ug.edge_references() {
        let a = e.source();
        let b = e.target();

        let key = if a <= b { (a, b) } else { (b, a) };

        if !seen.insert(key) {
            remove.push(e.id());
        }
    }

    for e in remove {
        ug.remove_edge(e);
    }

    ug
}
```

---

## 9.9 Direction conversion patterns

### Pattern A: temporary reverse traversal without mutating topology

Use `Incoming` traversal:

```rust
use petgraph::graph::DiGraph;
use petgraph::Direction::Incoming;

fn predecessors<N, E>(
    g: &DiGraph<N, E>,
    n: petgraph::graph::NodeIndex,
) -> Vec<petgraph::graph::NodeIndex> {
    g.neighbors_directed(n, Incoming).collect()
}
```

Use when:

```text
single-node reverse query
predecessor scan
avoid graph mutation
avoid clone/reverse allocation
```

### Pattern B: in-place whole-graph reverse

```rust
g.reverse();
```

Use when:

```text
phase shift: all future traversals should use reversed graph
original orientation no longer needed
edge indices/weights should remain in same graph object
```

### Pattern C: cloned reverse for dual orientation

```rust
let mut reverse = g.clone();
reverse.reverse();
```

Use when:

```text
both original and reversed traversals are needed repeatedly
graph clone cost is acceptable
```

### Pattern D: type conversion only

```rust
let undirected = directed.into_edge_type::<petgraph::Undirected>();
```

Use when:

```text
direction semantics are intentionally discarded
you accept no automatic edge deduplication
you will normalize after conversion if needed
```

### Pattern E: semantic symmetric expansion

```rust
use petgraph::graph::DiGraph;

fn add_bidirectional<N, E: Clone>(
    g: &mut DiGraph<N, E>,
    a: petgraph::graph::NodeIndex,
    b: petgraph::graph::NodeIndex,
    w: E,
) {
    g.update_edge(a, b, w.clone());
    g.update_edge(b, a, w);
}
```

Use when:

```text
algorithm requires Directed graph
logical relation is bidirectional
incoming/outgoing direction still matters computationally
```

---

## 9.10 Modeling one-way roads

Canonical type:

```rust
use petgraph::graph::DiGraph;

#[derive(Clone, Debug)]
struct Intersection {
    id: u64,
}

#[derive(Clone, Debug)]
struct RoadSegment {
    meters: u32,
    speed_limit_kph: u32,
}

type RoadGraph = DiGraph<Intersection, RoadSegment>;
```

Construction:

```rust
let mut roads = RoadGraph::new();

let a = roads.add_node(Intersection { id: 1 });
let b = roads.add_node(Intersection { id: 2 });

roads.add_edge(a, b, RoadSegment {
    meters: 120,
    speed_limit_kph: 50,
});
```

Rules:

```text
one-way road:
    Directed edge a -> b

two-way road with same metadata:
    add a -> b and b -> a

two-way road with asymmetric metadata:
    store different edge weights per direction

do not use Undirected if:
    turn restrictions, slope, travel time, or legal direction are asymmetric
```

---

## 9.11 Modeling dependency DAGs

Canonical type:

```rust
use petgraph::graph::DiGraph;

#[derive(Clone, Debug)]
struct Package {
    name: String,
}

#[derive(Clone, Debug)]
struct Requires {
    kind: &'static str,
}

type DependencyDag = DiGraph<Package, Requires>;
```

Direction convention:

```text
recommended:
    dependency edge dependent -> prerequisite

Meaning:
    A -> B means A requires B
    outgoing(A) = direct prerequisites of A
    incoming(B) = direct dependents of B
```

Syntax:

```rust
use petgraph::Direction::{Incoming, Outgoing};

let mut g = DependencyDag::new();

let app = g.add_node(Package { name: "app".into() });
let serde = g.add_node(Package { name: "serde".into() });

g.update_edge(app, serde, Requires { kind: "normal" });

let prerequisites: Vec<_> = g.neighbors_directed(app, Outgoing).collect();
let dependents: Vec<_> = g.neighbors_directed(serde, Incoming).collect();

assert_eq!(prerequisites, vec![serde]);
assert_eq!(dependents, vec![app]);
```

DAG invariant check:

```rust
use petgraph::algo::is_cyclic_directed;

assert!(!is_cyclic_directed(&g));
```

Agent rule:

```text
Pick one dependency orientation and freeze it in docs:
    dependent -> prerequisite
or:
    prerequisite -> dependent

Then define:
    outgoing means ...
    incoming means ...

Most bugs in dependency graphs come from silent orientation reversal.
```

---

## 9.12 Modeling bidirectional networks

### Option A: `UnGraph` for symmetric topology

```rust
use petgraph::graph::UnGraph;

#[derive(Clone, Debug)]
struct Router {
    id: u64,
}

#[derive(Clone, Debug)]
struct Link {
    capacity_gbps: u32,
}

type Network = UnGraph<Router, Link>;
```

Use when:

```text
link exists symmetrically
edge payload is direction-invariant
algorithms treat adjacency as undirected
```

### Option B: `DiGraph` with reciprocal edges

```rust
use petgraph::graph::DiGraph;

type Network = DiGraph<Router, Link>;

fn add_symmetric_link(
    g: &mut Network,
    a: NodeIndex,
    b: NodeIndex,
    link: Link,
) {
    g.update_edge(a, b, link.clone());
    g.update_edge(b, a, link);
}
```

Use when:

```text
algorithm expects directed edges
direction-specific metrics may diverge later
incoming/outgoing degree semantics matter
you need one-way failure modeling
```

### Option C: `GraphMap` for copy-key simple networks

```rust
use petgraph::graphmap::UnGraphMap;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct RouterId(u32);

type NetworkMap = UnGraphMap<RouterId, u32>;
```

Use when:

```text
node ID is Copy key
no parallel links
edge existence checks are hot
no rich node payload required inside graph
```

---

## 9.13 Modeling multigraph-like data

Use `Graph` / `StableGraph` if parallel edges are semantic:

```rust
use petgraph::graph::DiGraph;

#[derive(Clone, Debug)]
enum Relationship {
    Calls,
    Owns,
    Monitors,
}

let mut g = DiGraph::<&str, Relationship>::new();

let api = g.add_node("api");
let db = g.add_node("db");

g.add_edge(api, db, Relationship::Calls);
g.add_edge(api, db, Relationship::Monitors);

assert_eq!(g.edges_connecting(api, db).count(), 2);
```

Use bundled edge weight if graph type disallows parallel edges:

```rust
use petgraph::graphmap::DiGraphMap;

#[derive(Default, Clone, Debug)]
struct RelationshipSet {
    calls: bool,
    owns: bool,
    monitors: bool,
}

let mut g = DiGraphMap::<u32, RelationshipSet>::new();

g.add_edge(1, 2, RelationshipSet {
    calls: true,
    ..Default::default()
});

if let Some(e) = g.edge_weight_mut(1, 2) {
    e.monitors = true;
}
```

Decision:

```text
Need distinct edge identity/history:
    Graph / StableGraph + add_edge

Need one edge per endpoint pair with accumulated state:
    Graph::update_edge
    GraphMap edge bundle
    MatrixGraph edge bundle
    Csr aggregate before construction
```

---

## 9.14 Graph-family edge invariant table

| Invariant                       |           `Graph` |     `StableGraph` |                  `GraphMap` |                        `MatrixGraph` |                                   `Csr` |
| ------------------------------- | ----------------: | ----------------: | --------------------------: | -----------------------------------: | --------------------------------------: |
| Directed/undirected type marker |               Yes |               Yes |                         Yes |                                  Yes |                                     Yes |
| Runtime direction traversal     |               Yes |               Yes |                         Yes |                                  Yes | Trait-dependent / outgoing-row-oriented |
| Parallel edges                  |               Yes |               Yes |                          No |                                   No |                                      No |
| Self-loops                      |               Yes |               Yes |                         Yes |  endpoint-pair diagonal by API shape |                                     Yes |
| Edge ID                         |       `EdgeIndex` |       `EdgeIndex` |                    `(N, N)` |             `(NodeIndex, NodeIndex)` |                         row/column pair |
| Add edge duplicates             | inserts duplicate | inserts duplicate |          updates old weight | `add_edge` panics; update overwrites |      add returns `false` / no duplicate |
| Best duplicate-control API      |     `update_edge` |     `update_edge` | native `add_edge` overwrite | `update_edge` / `add_or_update_edge` |                 sort+dedup before build |

---

## 9.15 Modeling decision table

| Domain                         | Recommended graph               | Direction convention                                            |
| ------------------------------ | ------------------------------- | --------------------------------------------------------------- |
| One-way roads                  | `DiGraph`                       | road segment `from -> to`                                       |
| Two-way roads, symmetric cost  | `UnGraph`                       | no semantic source/target                                       |
| Two-way roads, asymmetric cost | `DiGraph` with reciprocal edges | separate weights per direction                                  |
| Dependency DAG                 | `DiGraph`                       | freeze convention: dependent→dependency or dependency→dependent |
| Build pipeline order           | `DiGraph`                       | prerequisite→dependent often natural for topo output            |
| Bidirectional physical network | `UnGraph`                       | edge is symmetric link                                          |
| Directed network flow          | `DiGraph`                       | capacity edge source→sink                                       |
| Multigraph event stream        | `Graph` / `StableGraph`         | each event is its own edge                                      |
| Simple keyed relation          | `GraphMap`                      | endpoint pair is unique edge                                    |
| Dense compatibility matrix     | `MatrixGraph`                   | endpoint pair slot                                              |
| Static sparse adjacency        | `Csr`                           | row source -> column target                                     |

---

## 9.16 Anti-pattern inventory

```text
Anti-pattern:
    Use Undirected for one-way relationships.
Problem:
    incoming/outgoing semantics collapse.
Fix:
    use Directed.

Anti-pattern:
    Use Directed reciprocal edges but then assume one logical edge.
Problem:
    algorithms see two edges.
Fix:
    use Undirected or normalize output.

Anti-pattern:
    Convert Directed -> Undirected with into_edge_type and assume deduplication.
Problem:
    into_edge_type performs no edge adjustment.
Fix:
    deduplicate after conversion.

Anti-pattern:
    Treat source/target of undirected EdgeReference as semantic direction.
Problem:
    orientation is API/reference artifact.
Fix:
    normalize endpoint pair or ignore source/target order.

Anti-pattern:
    Use GraphMap / MatrixGraph / Csr for parallel-edge event logs.
Problem:
    they represent one edge per endpoint pair.
Fix:
    use Graph / StableGraph or aggregate events inside E.

Anti-pattern:
    Reverse graph by changing interpretation only.
Problem:
    APIs still follow stored orientation.
Fix:
    use Incoming traversal, reverse(), or a reversed adaptor.

Anti-pattern:
    Unclear dependency edge direction.
Problem:
    every incoming/outgoing query becomes ambiguous.
Fix:
    document convention in type alias and helper names.
```

---

## 9.17 Deployment checklist

```text
Before choosing graph type:
    define whether relation is symmetric
    define whether endpoint order is semantic
    define whether self-loops are valid
    define whether parallel edges are valid
    define whether duplicate edge insertion should overwrite, reject, or append
    define whether algorithms require directed or undirected semantics

For directed graphs:
    document incoming/outgoing meaning
    use helper functions named successors/predecessors or dependencies/dependents
    prefer Direction constants over booleans

For undirected graphs:
    never expose source/target as semantic direction
    normalize endpoint pairs for external maps
    use UnGraph/UnGraphMap/UnMatrix when symmetry is structural

For multigraphs:
    choose Graph/StableGraph
    use add_edge
    use edges_connecting to inspect all endpoint-pair edges

For simple graphs:
    choose update_edge or simple graph family
    test duplicate insertion behavior
```

Final rule:

```text
Directedness is a type contract.
Direction is a traversal parameter.
Parallel-edge behavior is graph-family-specific.
Undirected edge orientation is not domain direction.
```

[1]: https://docs.rs/petgraph/latest/petgraph/trait.EdgeType.html "EdgeType in petgraph - Rust"
[2]: https://docs.rs/petgraph/latest/petgraph/enum.Direction.html "Direction in petgraph - Rust"
[3]: https://docs.rs/petgraph/latest/petgraph/graph/struct.Graph.html "Graph in petgraph::graph - Rust"
[4]: https://docs.rs/petgraph/latest/petgraph/graphmap/struct.GraphMap.html "GraphMap in petgraph::graphmap - Rust"
[5]: https://docs.rs/petgraph/latest/petgraph/stable_graph/struct.StableGraph.html "StableGraph in petgraph::stable_graph - Rust"


# 10) Node and edge weights as domain data

Format follows the uploaded advanced-reference style. 

`Graph<N, E, Ty, Ix>` is parameterized by associated node data `N` and edge data `E`, called **weights**. In petgraph terminology, “weight” means arbitrary associated payload, not necessarily numeric cost; `N` and `E` can be domain structs, IDs, `()`, numeric costs, boxes, shared pointers, or external-store handles. ([Docs.rs][1])

---

## 10.0 Core model

```rust id="hp6q3w"
use petgraph::Graph;

pub struct Graph<N, E, Ty = petgraph::Directed, Ix = petgraph::graph::DefaultIx>;
```

Interpretation:

```text id="b2ip0y"
N = node weight type = payload attached to each node
E = edge weight type = payload attached to each edge

"weight" != necessarily numeric
"weight" == associated data
```

Minimal payload graph:

```rust id="oynpp0"
use petgraph::Graph;

let mut g = Graph::<&'static str, u32>::new();

let api = g.add_node("api"); // node weight: &'static str
let db = g.add_node("db");

let edge = g.add_edge(api, db, 4); // edge weight: u32

assert_eq!(g[api], "api");
assert_eq!(g[edge], 4);
```

`Graph` stores node and edge weights, supports mutable access to both, and exposes weight access through `node_weight`, `node_weight_mut`, `edge_weight`, `edge_weight_mut`, plus indexing syntax. ([Docs.rs][1])

---

## 10.1 Weight terminology: domain payload, not algorithmic cost

Bad assumption:

```text id="b54lwj"
edge weight E == shortest-path cost
```

Correct model:

```text id="vc4sf4"
edge weight E == edge payload
algorithm cost == value extracted from E by closure
```

Domain edge payload:

```rust id="y0pn4f"
use petgraph::Graph;
use petgraph::algo::dijkstra;

#[derive(Clone, Debug)]
struct Dependency {
    protocol: &'static str,
    p95_latency_ms: u32,
    critical: bool,
}

let mut g = Graph::<&str, Dependency>::new();

let api = g.add_node("api");
let db = g.add_node("db");

g.add_edge(api, db, Dependency {
    protocol: "tcp",
    p95_latency_ms: 4,
    critical: true,
});

// Algorithmic cost extracted from domain metadata.
let dist = dijkstra(&g, api, Some(db), |edge| {
    edge.weight().p95_latency_ms
});
```

Agent rule:

```text id="gde6vd"
Do not force numeric E unless numeric edge cost is the only edge data.
Prefer domain-rich E when application logic needs relation metadata.
Extract numeric costs at algorithm call sites.
```

---

## 10.2 Zero-sized weights with `()`

Use `()` when topology is the data:

```rust id="26feqc"
use petgraph::graph::DiGraph;

let mut g = DiGraph::<(), ()>::new();

let a = g.add_node(());
let b = g.add_node(());

g.add_edge(a, b, ());
```

Use cases:

```text id="oy4de2"
reachability
cycle detection
topological sort
SCC computation
connectivity
unit-cost graph
graph shape tests
benchmark topology without payload pressure
```

`from_edges` / `extend_with_edges` can fill node weights and edge weights with defaults when omitted, making `Graph::<(), ()>::from_edges(...)` the terse topology-only constructor path. ([Docs.rs][1])

```rust id="otayhd"
use petgraph::Graph;

let g = Graph::<(), ()>::from_edges([
    (0, 1),
    (1, 2),
    (2, 0),
]);

assert_eq!(g.node_count(), 3);
assert_eq!(g.edge_count(), 3);
```

Unit-cost Dijkstra:

```rust id="z03bzg"
use petgraph::algo::dijkstra;
use petgraph::Graph;

let g = Graph::<(), ()>::from_edges([(0, 1), (1, 2)]);

let distances = dijkstra(
    &g,
    petgraph::graph::NodeIndex::new(0),
    None,
    |_| 1u32,
);
```

Agent rule:

```text id="5m5cwq"
Use () for N when:
    node identity is only NodeIndex
    no node metadata needed

Use () for E when:
    edge existence is sufficient
    algorithm cost is constant
    relation has no metadata
```

---

## 10.3 Numeric costs vs domain metadata

### Numeric-cost edge graph

```rust id="kl4je1"
use petgraph::graph::DiGraph;

type CostGraph = DiGraph<&'static str, u32>;

let mut g = CostGraph::new();

let a = g.add_node("a");
let b = g.add_node("b");

g.add_edge(a, b, 10);
```

Good when:

```text id="7zv12z"
only edge cost matters
edge label/type/provenance unnecessary
algorithms dominate graph use
```

### Domain-edge graph

```rust id="v7ai7x"
use petgraph::graph::DiGraph;

#[derive(Clone, Debug)]
struct EdgeData {
    cost: u32,
    kind: EdgeKind,
    source: &'static str,
}

#[derive(Clone, Debug)]
enum EdgeKind {
    Calls,
    Owns,
    Monitors,
}

type DomainGraph = DiGraph<&'static str, EdgeData>;
```

Good when:

```text id="d9fwhr"
edge relation type matters
cost is one field among many
debug output needs provenance
filtering/subgraph generation uses metadata
serialization/export preserves business semantics
```

Cost closure:

```rust id="5xgnt7"
let cost = |edge: petgraph::graph::EdgeReference<'_, EdgeData>| {
    edge.weight().cost
};
```

Agent rule:

```text id="rcpr4f"
If business logic names the relation:
    use struct/enum E

If algorithm logic alone names the relation:
    numeric E or () may be enough

If future filters/exports/debugging need context:
    do not throw context away by storing only numeric costs
```

---

## 10.4 Node payload patterns

### Rich node payload

```rust id="jzimq8"
#[derive(Clone, Debug)]
struct Service {
    id: String,
    tier: String,
    owner: String,
}
```

```rust id="q6jotm"
use petgraph::graph::DiGraph;

let mut g = DiGraph::<Service, ()>::new();

let api = g.add_node(Service {
    id: "api".into(),
    tier: "frontend".into(),
    owner: "platform".into(),
});
```

### External ID as payload

```rust id="fvirqc"
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct ServiceRowId(u32);

use petgraph::graph::DiGraph;

let mut g = DiGraph::<ServiceRowId, ()>::new();
let api = g.add_node(ServiceRowId(17));
```

### Side-map identity + rich external store

```rust id="ht2nz9"
use std::collections::HashMap;
use petgraph::graph::{DiGraph, NodeIndex};

#[derive(Clone, Debug)]
struct Service {
    id: String,
    owner: String,
}

struct Store {
    graph: DiGraph<ServiceRowId, ()>,
    by_external_id: HashMap<String, NodeIndex>,
    services: Vec<Service>,
}
```

Agent rule:

```text id="zbn6h1"
Put rich node structs directly in N when:
    graph owns domain data
    graph is serialization/debug boundary
    node_count is moderate
    cloning/mapping cost is acceptable

Put compact handles in N when:
    payload is large
    payload is shared across graphs
    payload lives in DB/arena/cache
    algorithms dominate and payload rarely needed
```

---

## 10.5 Edge payload patterns

### Boolean/topology-only edge

```rust id="udrgl0"
type G = petgraph::graph::DiGraph<NodeData, ()>;
```

### Numeric cost edge

```rust id="u6ye16"
type G = petgraph::graph::DiGraph<NodeData, u32>;
```

### Edge enum

```rust id="7qdvhl"
#[derive(Clone, Debug)]
enum Relation {
    DependsOn,
    Calls,
    Owns,
}
```

### Edge struct

```rust id="f1k68w"
#[derive(Clone, Debug)]
struct RelationData {
    relation: Relation,
    cost: u32,
    source_file: Option<String>,
    confidence: f32,
}
```

### Aggregated multirelation bundle

```rust id="s9gbgt"
#[derive(Clone, Debug, Default)]
struct RelationBundle {
    calls: bool,
    owns: bool,
    monitors: bool,
    call_count: u64,
}
```

Use bundle-style `E` when using simple graph families (`GraphMap`, `MatrixGraph`, `Csr`) or `Graph::update_edge` semantics:

```rust id="0h4tdd"
use petgraph::graphmap::DiGraphMap;

let mut g = DiGraphMap::<u32, RelationBundle>::new();

g.add_edge(1, 2, RelationBundle {
    calls: true,
    ..Default::default()
});

if let Some(e) = g.edge_weight_mut(1, 2) {
    e.monitors = true;
}
```

---

## 10.6 Borrowing vs cloning weights

### Borrowing accessors

```rust id="1eps70"
if let Some(node) = g.node_weight(api) {
    println!("{node:?}");
}

if let Some(edge) = g.edge_weight(edge_id) {
    println!("{edge:?}");
}
```

`node_weight` and `edge_weight` return `Option<&N>` / `Option<&E>`; mutable variants return `Option<&mut N>` / `Option<&mut E>`. ([Docs.rs][1])

### Mutable in-place edit

```rust id="eg2wvc"
if let Some(service) = g.node_weight_mut(api) {
    service.owner = "runtime".to_owned();
}

if let Some(dep) = g.edge_weight_mut(edge_id) {
    dep.p95_latency_ms += 1;
}
```

### Payload-only iteration

```rust id="c4mo7u"
for node in g.node_weights() {
    println!("{node:?}");
}

for edge in g.edge_weights() {
    println!("{edge:?}");
}
```

`node_weights` / `edge_weights` yield immutable access to weights in node-index / edge-index order, and mutable variants yield mutable access in the same index order. ([Docs.rs][1])

### Clone only at ownership boundary

```rust id="sc80lb"
let owned_node: NodeData = g
    .node_weight(api)
    .expect("valid node")
    .clone();
```

Agent rule:

```text id="sbs5qz"
Borrow weights inside algorithms/traversals.
Mutate in place via *_weight_mut.
Clone only when data must outlive graph borrow.
Avoid cloning large N/E inside hot loops.
Use map_owned when consuming graph avoids cloning.
```

---

## 10.7 `map`: borrowed transform, same topology

Signature:

```rust id="qq4z4y"
pub fn map<'a, F, G, N2, E2>(
    &'a self,
    node_map: F,
    edge_map: G,
) -> Graph<N2, E2, Ty, Ix>
where
    F: FnMut(NodeIndex<Ix>, &'a N) -> N2,
    G: FnMut(EdgeIndex<Ix>, &'a E) -> E2;
```

`Graph::map` creates a new graph by transforming node and edge weights; the resulting graph has the same structure and graph indices as the original. ([Docs.rs][1])

Example: domain graph → algorithm-cost graph, cloning node IDs only:

```rust id="euj3qw"
use petgraph::graph::DiGraph;

#[derive(Clone, Debug)]
struct Service {
    name: String,
}

#[derive(Clone, Debug)]
struct Dependency {
    latency_ms: u32,
    protocol: &'static str,
}

let mut domain = DiGraph::<Service, Dependency>::new();

let api = domain.add_node(Service { name: "api".into() });
let db = domain.add_node(Service { name: "db".into() });

domain.add_edge(api, db, Dependency {
    latency_ms: 4,
    protocol: "tcp",
});

let cost_graph: DiGraph<String, u32> = domain.map(
    |_node_ix, svc| svc.name.clone(),
    |_edge_ix, dep| dep.latency_ms,
);
```

Use `map` when:

```text id="ywu314"
source graph must remain available
transform can borrow source weights
cloning selected fields is acceptable
index compatibility must be preserved
```

---

## 10.8 `map_owned`: consuming transform, move weights

Signature:

```rust id="rhxjcz"
pub fn map_owned<F, G, N2, E2>(
    self,
    node_map: F,
    edge_map: G,
) -> Graph<N2, E2, Ty, Ix>
where
    F: FnMut(NodeIndex<Ix>, N) -> N2,
    G: FnMut(EdgeIndex<Ix>, E) -> E2;
```

`Graph::map_owned` consumes the source graph, maps owned node/edge weights, and preserves the same structure and graph indices. ([Docs.rs][1])

Example: strip metadata without cloning big strings:

```rust id="82w52l"
use petgraph::graph::DiGraph;

#[derive(Debug)]
struct Service {
    id: String,
    owner: String,
}

#[derive(Debug)]
struct Dependency {
    cost: u32,
    provenance: String,
}

let projected: DiGraph<String, u32> = domain.map_owned(
    |_ix, service| service.id,
    |_ix, dep| dep.cost,
);
```

Use `map_owned` when:

```text id="6w0pi1"
source graph no longer needed
want to move fields out of N/E
avoid cloning large String/Vec payloads
building downstream algorithm/export graph
```

---

## 10.9 `filter_map`: borrowed transform + subgraph

Signature:

```rust id="xf5p4s"
pub fn filter_map<'a, F, G, N2, E2>(
    &'a self,
    node_map: F,
    edge_map: G,
) -> Graph<N2, E2, Ty, Ix>
where
    F: FnMut(NodeIndex<Ix>, &'a N) -> Option<N2>,
    G: FnMut(EdgeIndex<Ix>, &'a E) -> Option<E2>;
```

`filter_map` maps nodes/edges and drops any node or edge mapped to `None`; nodes are mapped first, then `edge_map` is called only for edges whose endpoints remain. The result is a subgraph of the original; if no nodes are removed, node indices remain compatible, and if neither nodes nor edges are removed, all graph indices match the original. ([Docs.rs][1])

Example: keep only production services and critical dependencies:

```rust id="jbtbmm"
use petgraph::graph::DiGraph;

#[derive(Clone, Debug)]
struct Service {
    name: String,
    env: Env,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Env {
    Prod,
    Dev,
}

#[derive(Clone, Debug)]
struct Dependency {
    critical: bool,
    latency_ms: u32,
}

let prod_critical: DiGraph<String, u32> = domain.filter_map(
    |_ix, svc| {
        if svc.env == Env::Prod {
            Some(svc.name.clone())
        } else {
            None
        }
    },
    |_ix, dep| {
        if dep.critical {
            Some(dep.latency_ms)
        } else {
            None
        }
    },
);
```

Index warning:

```text id="slscxw"
filter_map may remove nodes.
If nodes are removed:
    old NodeIndex values are not generally compatible.
If no nodes are removed but edges are filtered:
    node indices stay compatible.
If nothing is filtered:
    node and edge indices match original.
```

---

## 10.10 `filter_map_owned`: consuming transform + subgraph

Signature:

```rust id="f071mk"
pub fn filter_map_owned<F, G, N2, E2>(
    self,
    node_map: F,
    edge_map: G,
) -> Graph<N2, E2, Ty, Ix>
where
    F: FnMut(NodeIndex<Ix>, N) -> Option<N2>,
    G: FnMut(EdgeIndex<Ix>, E) -> Option<E2>;
```

`filter_map_owned` consumes the source graph, can drop nodes/edges by returning `None`, maps nodes first, and calls the edge closure only for edges whose endpoints remain. The result is a subgraph with the same compatibility caveats as `filter_map`. ([Docs.rs][1])

Example: consume graph and keep only cheap dependencies:

```rust id="00avb9"
let cheap: DiGraph<String, u32> = domain.filter_map_owned(
    |_ix, svc| Some(svc.name),
    |_ix, dep| {
        if dep.latency_ms <= 10 {
            Some(dep.latency_ms)
        } else {
            None
        }
    },
);
```

Use `filter_map_owned` when:

```text id="wvpznt"
source graph can be consumed
filtering creates derived graph
large fields should be moved, not cloned
subgraph extraction is one-way pipeline
```

---

## 10.11 Transform decision table

| Operation          | Borrows source? | Consumes source? | Can change weight types? | Can drop nodes/edges? | Preserves indices?                          |
| ------------------ | --------------: | ---------------: | -----------------------: | --------------------: | ------------------------------------------- |
| `map`              |             Yes |               No |                      Yes |                    No | Yes                                         |
| `map_owned`        |              No |              Yes |                      Yes |                    No | Yes                                         |
| `filter_map`       |             Yes |               No |                      Yes |                   Yes | Only under documented no-removal conditions |
| `filter_map_owned` |              No |              Yes |                      Yes |                   Yes | Only under documented no-removal conditions |

Rule:

```text id="frhj12"
Use map:
    projection without topology change

Use map_owned:
    projection without topology change, avoid clones

Use filter_map:
    borrowed subgraph extraction

Use filter_map_owned:
    consuming subgraph extraction, avoid clones
```

---

## 10.12 Best practices for large payloads

### Direct payload: simplest, but can be heavy

```rust id="h8pddk"
#[derive(Clone, Debug)]
struct LargeNode {
    blob: Vec<u8>,
    metadata: String,
}

type G = petgraph::graph::DiGraph<LargeNode, ()>;
```

Use direct payload when:

```text id="kp4ff0"
graph owns data
payload count moderate
clone/serialize cost acceptable
graph is domain object, not just topology kernel
```

### `Box<T>`: unique heap allocation

`Box<T>` uniquely owns a heap allocation of type `T`; `Box::new(x)` allocates memory on the heap and places `x` into it, except for zero-sized types. ([Rust Documentation][2])

```rust id="c3gidj"
use petgraph::graph::DiGraph;

#[derive(Debug)]
struct LargeEdge {
    payload: Vec<u8>,
    metadata: String,
}

type G = DiGraph<u32, Box<LargeEdge>>;
```

Use `Box<T>` when:

```text id="h5fhis"
payload is large
unique ownership is correct
you want smaller graph slot size
payload rarely accessed
no shared ownership needed
```

### `Arc<T>`: shared immutable payload

`Arc<T>` is an atomically reference-counted pointer that provides shared heap ownership; cloning an `Arc` creates another pointer to the same allocation and increments the reference count. It does not make inner data itself mutable or thread-safe; mutation requires interior mutability or copy-on-write mechanisms. ([Rust Documentation][3])

```rust id="zr8aoe"
use std::sync::Arc;
use petgraph::graph::DiGraph;

#[derive(Debug)]
struct NodePayload {
    blob: Vec<u8>,
    label: String,
}

type G = DiGraph<Arc<NodePayload>, ()>;

let shared = Arc::new(NodePayload {
    blob: vec![0; 1024],
    label: "shared".into(),
});
```

Use `Arc<T>` when:

```text id="u2fwiw"
payload shared across graphs
payload shared with external caches
graph cloned/mapped often
read-mostly payload
cross-thread sharing required and T: Send + Sync
```

### Arena/external-store ID

```rust id="hg205r"
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct NodeId(u32);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct EdgeId(u32);

type G = petgraph::graph::DiGraph<NodeId, EdgeId>;

struct Arena {
    nodes: Vec<NodePayload>,
    edges: Vec<EdgePayload>,
}
```

Use external handles when:

```text id="w2mxaw"
payload is huge
payload has independent lifecycle
graph algorithms rarely need payload
multiple graph views share same objects
payload lives in DB/arena/cache
graph should be cheap to clone/project
```

---

## 10.13 Separating topology from business data

Recommended layered architecture:

```text id="5a6bwk"
Graph topology layer:
    petgraph graph with compact node/edge payloads
    NodeIndex / EdgeIndex / lightweight IDs
    algorithm-friendly structure

Domain data layer:
    Vec / arena / database / ECS / cache
    rich mutable business objects
    stable external IDs
    serialization/versioning policy

Mapping layer:
    external ID -> NodeIndex
    NodeIndex -> external ID / arena ID
    EdgeIndex -> edge record ID when needed
```

Concrete store:

```rust id="vlvlhq"
use std::collections::HashMap;
use petgraph::graph::{DiGraph, NodeIndex, EdgeIndex};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct ServiceId(u32);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct DependencyId(u32);

#[derive(Clone, Debug)]
struct Service {
    external_id: String,
    owner: String,
}

#[derive(Clone, Debug)]
struct Dependency {
    protocol: String,
    p95_latency_ms: u32,
}

struct ServiceGraph {
    graph: DiGraph<ServiceId, DependencyId>,
    service_by_external_id: HashMap<String, NodeIndex>,
    services: Vec<Service>,
    dependencies: Vec<Dependency>,
}

impl ServiceGraph {
    fn dependency_cost(&self, e: EdgeIndex) -> Option<u32> {
        let dep_id = *self.graph.edge_weight(e)?;
        Some(self.dependencies[dep_id.0 as usize].p95_latency_ms)
    }
}
```

Algorithm closure with external store:

```rust id="0pxewy"
use petgraph::algo::dijkstra;

let distances = dijkstra(
    &store.graph,
    start,
    None,
    |edge| {
        let dep_id = *edge.weight();
        store.dependencies[dep_id.0 as usize].p95_latency_ms
    },
);
```

Value case:

```text id="zpixup"
topology remains small
payload lifecycle independent
algorithms traverse compact data
large business objects avoid cloning
multiple derived graphs can share one data store
serialization can version graph topology separately from domain records
```

---

## 10.14 `GraphMap` payload caveat

`GraphMap<N, E, Ty>` uses `N` as the node identity and requires it to be copyable, hashable/orderable, and duplicated in the structure, so it is a poor fit for large node payloads. Use copyable IDs as `N` and store rich node data externally. ([Docs.rs][4])

```rust id="pm3qnx"
use petgraph::graphmap::DiGraphMap;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct ServiceId(u32);

type G = DiGraphMap<ServiceId, DependencyId>;
```

Rule:

```text id="40v8mu"
GraphMap N:
    identity key
    small Copy ID

Graph / StableGraph N:
    arbitrary payload or compact handle
```

---

## 10.15 `MatrixGraph` payload caveat

`MatrixGraph` stores edge slots in an adjacency matrix and docs recommend boxing large edge weights because the backing array stores edge weights; direct large `E` can multiply memory pressure by matrix slot count. ([Docs.rs][1])

Preferred dense graph edge payload:

```rust id="db3r8h"
use petgraph::matrix_graph::DiMatrix;

#[derive(Debug)]
struct LargeRelation {
    metadata: String,
    samples: Vec<u8>,
}

type Dense = DiMatrix<u32, Box<LargeRelation>>;
```

Rule:

```text id="h74vyy"
MatrixGraph + large E:
    Box<E>
    Arc<E>
    EdgeId into external store

MatrixGraph + small numeric E:
    store directly
```

---

## 10.16 Borrowing-pattern recipes

### Read-only traversal over domain payloads

```rust id="56anzn"
use petgraph::visit::EdgeRef;

for edge in g.edge_references() {
    let dep = edge.weight();
    println!(
        "{:?} -> {:?}: {:?}",
        edge.source(),
        edge.target(),
        dep
    );
}
```

### Mutate all edge costs

```rust id="ie3wku"
for dep in g.edge_weights_mut() {
    dep.p95_latency_ms += 1;
}
```

### Mutate node payload by handle

```rust id="wwfcsg"
fn mark_owner(
    g: &mut petgraph::graph::DiGraph<Service, Dependency>,
    n: petgraph::graph::NodeIndex,
    owner: String,
) {
    if let Some(svc) = g.node_weight_mut(n) {
        svc.owner = owner;
    }
}
```

### Extract detached report without owning graph

```rust id="tresv1"
#[derive(Debug)]
struct EdgeReport {
    latency_ms: u32,
    critical: bool,
}

let report: Vec<EdgeReport> = g
    .edge_weights()
    .map(|dep| EdgeReport {
        latency_ms: dep.p95_latency_ms,
        critical: dep.critical,
    })
    .collect();
```

---

## 10.17 Serialization/deployment payload rules

Cargo:

```toml id="n7x39c"
[dependencies]
petgraph = { version = "0.8.3", features = ["serde-1"] }
serde = { version = "1", features = ["derive"] }
```

Payload requirements:

```text id="ffcxq2"
To serialize Graph<N,E>:
    N: Serialize
    E: Serialize

To deserialize:
    N: Deserialize
    E: Deserialize

If using Arc<T> / Box<T> / IDs:
    ensure chosen pointer/ID strategy serializes as desired
```

Stable file-format advisory:

```text id="xu5r8d"
Do not expose raw petgraph serde as long-term public protocol unless version-pinned.
Prefer domain schema:
    nodes: [{id, payload}]
    edges: [{source_id, target_id, payload}]
Then rebuild petgraph topology on load.
```

---

## 10.18 Anti-pattern inventory

```text id="6m24kc"
Anti-pattern:
    Treat E as shortest-path cost everywhere.
Problem:
    E is arbitrary payload; algorithms use closures.
Fix:
    store domain E; extract cost per algorithm.

Anti-pattern:
    Use String node IDs directly as GraphMap N.
Problem:
    GraphMap N must be Copy and is duplicated.
Fix:
    intern strings or use Graph + HashMap.

Anti-pattern:
    Clone large N/E inside map/filter hot path.
Problem:
    expensive payload copies.
Fix:
    use map_owned, Box/Arc, or external IDs.

Anti-pattern:
    Store large E directly in MatrixGraph.
Problem:
    adjacency matrix edge slots amplify memory.
Fix:
    Box<E>, Arc<E>, or EdgeId.

Anti-pattern:
    Put all business state inside graph weights.
Problem:
    algorithms, clones, serialization, and transformation become heavy.
Fix:
    store compact IDs in graph; keep business data in arena/store.

Anti-pattern:
    Use () but later need provenance/debugging.
Problem:
    topology-only graph loses context.
Fix:
    use lightweight enum/ID payload from start if provenance matters.

Anti-pattern:
    Use Arc<T> expecting mutable thread-safe inner T.
Problem:
    Arc shares ownership; mutation needs Mutex/RwLock/Atomic/COW.
Fix:
    choose Arc<Mutex<T>>, Arc<RwLock<T>>, or immutable Arc<T>.
```

---

## 10.19 Deployment checklist

```text id="nbq88u"
Choose N:
    () for topology-only
    small Copy ID for external payload store
    rich struct for graph-owned domain node
    Arc<T> for shared read-mostly node payload
    Box<T> for unique heap-owned large node payload

Choose E:
    () for unweighted/topology-only edges
    numeric for pure algorithm cost
    struct/enum for relation metadata
    bundle for simple graph with multiple flags/counters
    Box/Arc/EdgeId for large edge payloads

Choose transform:
    map              => borrow source, preserve topology/indices
    map_owned        => consume source, move weights, preserve topology/indices
    filter_map       => borrow source, produce subgraph
    filter_map_owned => consume source, produce subgraph

Optimize:
    avoid clones in hot paths
    store compact IDs for large payloads
    pre-project domain graph into algorithm graph when algorithm repeats
    separate topology from business data when lifecycle/serialization/performance differ
```

Final rule:

```text id="qw6m7p"
Petgraph weights are payload slots.
Use them intentionally:
    () for pure topology
    numeric for pure cost
    structs/enums for domain semantics
    IDs/Box/Arc for large or shared data
    map/filter_map families for topology-preserving or subgraph transformations
```

[1]: https://docs.rs/petgraph/latest/petgraph/graph/struct.Graph.html "Graph in petgraph::graph - Rust"
[2]: https://doc.rust-lang.org/std/boxed/struct.Box.html "Box in std::boxed - Rust"
[3]: https://doc.rust-lang.org/std/sync/struct.Arc.html "Arc in std::sync - Rust"
[4]: https://docs.rs/petgraph/latest/petgraph/graphmap/struct.GraphMap.html "GraphMap in petgraph::graphmap - Rust"

# 11) Indexing, identity, and mutation safety — petgraph

Format follows the uploaded advanced-reference style. 

Petgraph index safety is a **graph-family contract**, not a universal property of `NodeIndex` / `EdgeIndex`. `Graph` gives compact indices but removals may shift other indices; `StableGraph` preserves unrelated live indices across removals; `GraphMap` uses node values as identity; `MatrixGraph` and `Csr` use endpoint/index-position semantics. `Graph` docs explicitly state additions keep indices stable, while node/edge removals may shift the last node/edge into the removed slot. ([Docs.rs][1])

---

## 11.0 Core imports

```rust id="h54ais"
use std::collections::HashMap;

use petgraph::graph::{
    Graph,
    DiGraph,
    UnGraph,
    NodeIndex,
    EdgeIndex,
    DefaultIx,
    IndexType,
};

use petgraph::stable_graph::{
    StableGraph,
    StableDiGraph,
    StableUnGraph,
};

use petgraph::graphmap::{
    DiGraphMap,
    UnGraphMap,
};

use petgraph::visit::{
    Dfs,
    Bfs,
    Walker,
    NodeIndexable,
    NodeCompactIndexable,
    EdgeIndexable,
    IntoNodeIdentifiers,
    IntoNeighbors,
    Visitable,
};

use petgraph::Direction::{
    Incoming,
    Outgoing,
};
```

---

## 11.1 `NodeIndex<Ix>` as lightweight graph-local handle

`NodeIndex<Ix = DefaultIx>` is petgraph’s node identifier for index-backed graph families. It is `Copy`, hashable/orderable, exposes `new(usize)`, `index() -> usize`, and `end()`, and is used by indexing implementations for `Graph`, `StableGraph`, and `MatrixGraph`; indexing panics if the node does not exist. ([Docs.rs][2])

```rust id="gsdm5b"
use petgraph::graph::{DiGraph, NodeIndex};

let mut g = DiGraph::<&str, ()>::new();

let api: NodeIndex = g.add_node("api");
let db: NodeIndex = g.add_node("db");

g.add_edge(api, db, ());

assert_eq!(api.index(), 0);
assert_eq!(g[api], "api");
```

Semantic contract:

```text id="qodv3j"
NodeIndex<Ix>:
    small Copy handle
    graph-local
    typed by Ix
    not globally unique
    not self-validating
    not a domain ID
    not stable across all graph families/mutations
```

Bad interpretation:

```text id="czufwx"
NodeIndex(7) == database ID 7
NodeIndex(7) remains valid forever
NodeIndex(7) can be used in any graph
NodeIndex(7) proves node exists
```

Correct boundary validation:

```rust id="qwhj9o"
fn get_label<'a>(
    g: &'a DiGraph<String, ()>,
    n: NodeIndex,
) -> Option<&'a str> {
    g.node_weight(n).map(String::as_str)
}
```

---

## 11.2 `EdgeIndex<Ix>` as lightweight graph-local edge handle

`EdgeIndex<Ix = DefaultIx>` is the edge identifier for index-backed adjacency-list graphs; it exposes `new`, `index`, and `end`, and `Graph` / `StableGraph` support indexing by `EdgeIndex` to access edge weights, panicking if the edge does not exist. ([Docs.rs][3])

```rust id="u5gzle"
use petgraph::graph::{DiGraph, EdgeIndex};

let mut g = DiGraph::<&str, u32>::new();

let a = g.add_node("a");
let b = g.add_node("b");

let e: EdgeIndex = g.add_edge(a, b, 10);

assert_eq!(e.index(), 0);
assert_eq!(g[e], 10);
```

Safe edge access:

```rust id="yy7pzf"
fn edge_cost(g: &DiGraph<&str, u32>, e: EdgeIndex) -> Option<u32> {
    g.edge_weight(e).copied()
}
```

Edge endpoint validation:

```rust id="86vzgq"
if let Some((source, target)) = g.edge_endpoints(e) {
    println!("{source:?} -> {target:?}");
}
```

`Graph::edge_weight`, `edge_weight_mut`, and `edge_endpoints` all return `Option`, giving the safe boundary path when handles may be stale or user-provided. ([Docs.rs][1])

---

## 11.3 Identity models by graph family

| Graph family  | Node identity   | Edge identity                          |            Stable after deletion? |                  Compact live IDs? |
| ------------- | --------------- | -------------------------------------- | --------------------------------: | ---------------------------------: |
| `Graph`       | `NodeIndex<Ix>` | `EdgeIndex<Ix>`                        |  No, removals may shift last item |                                Yes |
| `StableGraph` | `NodeIndex<Ix>` | `EdgeIndex<Ix>`                        | Unrelated live indices stay valid |                 No, holes possible |
| `GraphMap`    | node value `N`  | endpoint pair `(N, N)`                 |   Key-based; no `NodeIndex` model | Trait-level compact mapping exists |
| `MatrixGraph` | `NodeIndex<Ix>` | endpoint pair `(NodeIndex, NodeIndex)` |           Representation-specific |                       Index-backed |
| `Csr`         | `NodeIndex<Ix>` | row/column endpoint pair               |               Mostly static model |                                Yes |

`StableGraph` docs state it does not invalidate unrelated node or edge indices when items are removed, while `Graph` docs state removals may shift other indices; `NodeCompactIndexable` docs define the no-hole condition as node identifiers corresponding exactly to `0..node_bound()`. ([Docs.rs][4])

---

## 11.4 Compact vs stable index guarantees

### `Graph`: compact, not deletion-stable

```rust id="hz36oa"
use petgraph::graph::DiGraph;

let mut g = DiGraph::<&str, ()>::new();

let a = g.add_node("a"); // NodeIndex(0)
let b = g.add_node("b"); // NodeIndex(1)
let c = g.add_node("c"); // NodeIndex(2)

g.remove_node(b);

// b is removed.
// c may have moved into b's old slot.
// old c may be invalid or no longer point to "c".
```

`Graph::remove_node` invalidates the removed node index and may also invalidate the last node index because the last node adopts the removed node’s index; `remove_edge` similarly invalidates the removed edge index and may invalidate the last edge index. ([Docs.rs][1])

### `StableGraph`: stable unrelated live handles, holes possible

```rust id="3ekb30"
use petgraph::stable_graph::StableDiGraph;

let mut g = StableDiGraph::<&str, ()>::new();

let a = g.add_node("a");
let b = g.add_node("b");
let c = g.add_node("c");

g.remove_node(b);

assert_eq!(g.node_weight(a), Some(&"a"));
assert_eq!(g.node_weight(b), None);
assert_eq!(g.node_weight(c), Some(&"c"));
```

`StableGraph::remove_node` invalidates the removed node index and incident edges but not other node indices; this is the “unrelated indices” guarantee. ([Docs.rs][4])

---

## 11.5 Index invalidation table

| Operation          | `Graph` node handles             | `Graph` edge handles                          | `StableGraph` node handles                 | `StableGraph` edge handles |
| ------------------ | -------------------------------- | --------------------------------------------- | ------------------------------------------ | -------------------------- |
| `add_node`         | existing stable                  | existing stable                               | existing stable                            | existing stable            |
| `add_edge`         | existing stable                  | existing stable                               | existing stable                            | existing stable            |
| `remove_node(n)`   | `n` invalid; last node may shift | incident edges invalid; edge shifts may occur | only `n` invalid                           | incident edges invalid     |
| `remove_edge(e)`   | node handles stable              | `e` invalid; last edge may shift              | node handles stable                        | only `e` invalid           |
| compaction/rebuild | all remapped                     | all remapped                                  | all remapped if converted to compact graph | all remapped               |

The `Graph` page documents stable additions and shifting removals; `StableGraph` documents preservation of unrelated indices. ([Docs.rs][1])

---

## 11.6 Removed-slot reuse: stability is not generation safety

`StableGraph` preserves unrelated live handles, but a removed index can later become valid again for a different node/edge if the implementation reuses vacant slots. Treat `NodeIndex` alone as insufficient for stale-handle detection at API boundaries.

```rust id="iab61v"
use petgraph::stable_graph::StableDiGraph;

let mut g = StableDiGraph::<&str, ()>::new();

let old = g.add_node("old");
g.remove_node(old);

assert_eq!(g.node_weight(old), None);

let new = g.add_node("new");

// old == new is allowed by the stable-slot model.
// If equal, stale old handle now points to a different semantic entity.
if old == new {
    assert_eq!(g.node_weight(old), Some(&"new"));
}
```

Generation handle pattern:

```rust id="obzlqu"
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableDiGraph;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Generation(u64);

#[derive(Clone, Debug)]
struct NodeData {
    generation: Generation,
    label: String,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct NodeHandle {
    ix: NodeIndex,
    generation: Generation,
}

struct Store {
    graph: StableDiGraph<NodeData, ()>,
    next_generation: u64,
}

impl Store {
    fn insert(&mut self, label: String) -> NodeHandle {
        let generation = Generation(self.next_generation);
        self.next_generation += 1;

        let ix = self.graph.add_node(NodeData {
            generation,
            label,
        });

        NodeHandle { ix, generation }
    }

    fn get(&self, h: NodeHandle) -> Option<&NodeData> {
        let node = self.graph.node_weight(h.ix)?;
        (node.generation == h.generation).then_some(node)
    }
}
```

Deployment rule:

```text id="ow3n2w"
StableGraph protects unrelated live handles.
Generation IDs protect against stale removed-and-reused handles.
Use both when handles cross UI/API/task boundaries.
```

---

## 11.7 `NodeIndexable`: sparse index-space abstraction

`NodeIndexable` is the trait for graphs whose `NodeId`s map to integer indices; required methods are `node_bound()`, `to_index(NodeId) -> usize`, and `from_index(usize) -> NodeId`, where `node_bound()` is an upper bound suitable for bitmap sizing and `from_index` requires a valid graph value. It is implemented for `Graph`, `StableGraph`, `Csr`, `MatrixGraph`, `GraphMap`, and adapters. ([Docs.rs][5])

```rust id="l149tq"
use petgraph::visit::NodeIndexable;

fn make_node_bitmap<G>(g: &G) -> Vec<bool>
where
    G: NodeIndexable,
{
    vec![false; g.node_bound()]
}
```

Safe sparse-node loop:

```rust id="7wqdbs"
use petgraph::visit::{IntoNodeIdentifiers, NodeIndexable};

fn mark_live_nodes<G>(g: G) -> Vec<bool>
where
    G: IntoNodeIdentifiers + NodeIndexable,
    G::NodeId: Copy,
{
    let mut seen = vec![false; g.node_bound()];

    for n in g.node_identifiers() {
        seen[g.to_index(n)] = true;
    }

    seen
}
```

Agent rule:

```text id="q0fysp"
NodeIndexable:
    node_bound is an upper bound
    to_index maps valid NodeId to usize
    from_index requires valid index value
    holes may exist unless NodeCompactIndexable is also required
```

---

## 11.8 `NodeCompactIndexable`: dense no-hole node IDs

`NodeCompactIndexable` extends `NodeIndexable + NodeCount` and guarantees graph node identifiers map to indices in a range without holes; docs state node identifiers correspond exactly to `0..self.node_bound()`. Implementors include `Graph`, `Csr`, and `GraphMap`, but not `StableGraph`. ([Docs.rs][6])

Dense-array algorithm:

```rust id="x2m6li"
use petgraph::visit::{IntoNodeIdentifiers, NodeCompactIndexable};

fn dense_node_values<G>(g: G) -> Vec<usize>
where
    G: IntoNodeIdentifiers + NodeCompactIndexable,
    G::NodeId: Copy,
{
    let mut out = vec![0; g.node_bound()];

    for n in g.node_identifiers() {
        out[g.to_index(n)] = 1;
    }

    out
}
```

StableGraph-compatible alternative:

```rust id="ee1q0l"
use std::collections::HashMap;
use petgraph::visit::{IntoNodeIdentifiers, NodeIndexable};

fn sparse_node_values<G>(g: G) -> HashMap<G::NodeId, usize>
where
    G: IntoNodeIdentifiers + NodeIndexable,
    G::NodeId: Copy + Eq + std::hash::Hash,
{
    let mut out = HashMap::new();

    for n in g.node_identifiers() {
        out.insert(n, g.to_index(n));
    }

    out
}
```

Agent rule:

```text id="n4wdsn"
Use NodeCompactIndexable only when:
    dense Vec indexed by to_index is semantically required
    graph family cannot have holes
    StableGraph support is intentionally excluded

Use NodeIndexable + IntoNodeIdentifiers when:
    holes are acceptable
    StableGraph support matters
    bitmaps can be sized by node_bound but live nodes are iterated explicitly
```

---

## 11.9 `EdgeIndexable`: edge-index abstraction

`EdgeIndexable` maps graph edge IDs to integer indices and exposes `edge_bound()`, `to_index(EdgeId)`, and `from_index(usize)`, with `edge_bound()` documented as an upper bound suitable for bitmap sizing and `from_index` requiring a valid graph value. Implementors include `Graph`, `StableGraph`, `GraphMap`, and adapter types. ([Docs.rs][7])

```rust id="nxp6cy"
use petgraph::visit::EdgeIndexable;

fn make_edge_bitmap<G>(g: &G) -> Vec<bool>
where
    G: EdgeIndexable,
{
    vec![false; g.edge_bound()]
}
```

Edge-ID caveat:

```text id="l6lbz9"
Graph / StableGraph:
    EdgeId = EdgeIndex<Ix>

GraphMap:
    EdgeId = (N, N)

MatrixGraph:
    endpoint-pair style APIs dominate

Csr:
    no EdgeIndexable implementor in current trait list
```

Use `EdgeIndexable` for generic edge bitmaps only when the graph type supports it; otherwise, use edge references / endpoint pairs as keys.

---

## 11.10 External ID maps: `HashMap<DomainId, NodeIndex>`

Canonical app graph:

```rust id="jqr0vh"
use std::collections::HashMap;
use petgraph::graph::{DiGraph, NodeIndex};

#[derive(Clone, Debug)]
struct NodeData {
    id: String,
    label: String,
}

#[derive(Clone, Debug)]
struct EdgeData {
    kind: String,
}

struct Model {
    graph: DiGraph<NodeData, EdgeData>,
    by_id: HashMap<String, NodeIndex>,
}
```

Interning pattern:

```rust id="r5urac"
impl Model {
    fn intern(&mut self, id: &str) -> NodeIndex {
        if let Some(&ix) = self.by_id.get(id) {
            return ix;
        }

        let ix = self.graph.add_node(NodeData {
            id: id.to_owned(),
            label: id.to_owned(),
        });

        self.by_id.insert(id.to_owned(), ix);
        ix
    }

    fn add_or_update_edge(&mut self, from: &str, to: &str, kind: String) {
        let a = self.intern(from);
        let b = self.intern(to);

        self.graph.update_edge(a, b, EdgeData { kind });
    }
}
```

Deletion hazard with `Graph`:

```rust id="j73p32"
impl Model {
    fn remove_node_by_id_bad_for_graph(&mut self, id: &str) {
        if let Some(ix) = self.by_id.remove(id) {
            self.graph.remove_node(ix);

            // BUG: Graph may have shifted another node into ix.
            // All entries in by_id may now be stale.
        }
    }
}
```

Safe deletion options:

```text id="eecbwb"
Option A:
    use StableGraph so unrelated NodeIndex values survive deletion

Option B:
    use Graph but rebuild all maps after every deletion

Option C:
    avoid deletions; mark nodes inactive in payload

Option D:
    convert/delete in batch, rebuild graph and maps
```

Rebuild map after `Graph` deletion:

```rust id="g9t8uu"
use std::collections::HashMap;
use petgraph::graph::{DiGraph, NodeIndex};

fn rebuild_by_id(
    graph: &DiGraph<NodeData, EdgeData>,
) -> HashMap<String, NodeIndex> {
    let mut by_id = HashMap::with_capacity(graph.node_count());

    for ix in graph.node_indices() {
        by_id.insert(graph[ix].id.clone(), ix);
    }

    by_id
}
```

---

## 11.11 Stable external-ID store

```rust id="yp3m72"
use std::collections::HashMap;
use petgraph::stable_graph::StableDiGraph;
use petgraph::graph::NodeIndex;

#[derive(Clone, Debug)]
struct NodeData {
    id: String,
    label: String,
}

#[derive(Clone, Debug)]
struct EdgeData {
    kind: String,
}

struct StableModel {
    graph: StableDiGraph<NodeData, EdgeData>,
    by_id: HashMap<String, NodeIndex>,
}

impl StableModel {
    fn remove_node_by_id(&mut self, id: &str) -> Option<NodeData> {
        let ix = self.by_id.remove(id)?;
        self.graph.remove_node(ix)
    }

    fn get(&self, id: &str) -> Option<&NodeData> {
        let ix = *self.by_id.get(id)?;
        self.graph.node_weight(ix)
    }
}
```

Why this works:

```text id="3g07i0"
StableGraph deletion:
    removed node's index invalid
    unrelated node indices remain valid
    side map entries for other nodes remain correct
```

`StableGraph` docs explicitly state unrelated node/edge indices are not invalidated by removals. ([Docs.rs][4])

---

## 11.12 Anti-pattern: storing `Graph` handles across removals

Bad:

```rust id="9m34gs"
use petgraph::graph::{DiGraph, NodeIndex};

struct Selection {
    node: Option<NodeIndex>,
}

let mut g = DiGraph::<&str, ()>::new();

let a = g.add_node("a");
let b = g.add_node("b");
let c = g.add_node("c");

let mut selection = Selection { node: Some(c) };

g.remove_node(b);

// selection.node may no longer point to "c".
```

Correct strategies:

```text id="u221zy"
If selection must survive unrelated deletion:
    StableGraph

If using Graph:
    store domain ID, not NodeIndex
    rebuild NodeIndex from HashMap after mutation
    clear invalidated selections after deletion
```

Domain-ID selection:

```rust id="6xl9aj"
struct Selection {
    id: Option<String>,
}

fn selected_node(
    model: &Model,
    selection: &Selection,
) -> Option<NodeIndex> {
    let id = selection.id.as_ref()?;
    model.by_id.get(id).copied()
}
```

StableGraph selection:

```rust id="ctgpil"
use petgraph::stable_graph::StableDiGraph;
use petgraph::graph::NodeIndex;

struct StableSelection {
    node: Option<NodeIndex>,
}

fn render_selection<N, E>(
    g: &StableDiGraph<N, E>,
    selection: &StableSelection,
) {
    if let Some(ix) = selection.node {
        if g.node_weight(ix).is_some() {
            // still valid
        }
    }
}
```

---

## 11.13 Safe mutation while traversing: borrow model

Iterator APIs like `neighbors(a)` borrow the graph while the iterator lives, limiting mutation inside the loop. Petgraph provides detached walkers via `.detach()` for neighbor traversals, and the docs state detached walkers do not borrow the graph. The generic `Walker` trait is described as traversal state where part of traversal info is supplied to each next call, allowing graph traversals that do not hold a graph borrow. ([Docs.rs][1])

Borrowing iterator: read-only traversal

```rust id="qk8e2h"
for neighbor in g.neighbors(a) {
    println!("{neighbor:?}");
    // Cannot freely mutate g here because iterator borrows g.
}
```

Detached walker: mutation-capable traversal pattern

```rust id="fjcu0f"
let mut neighbors = g.neighbors(a).detach();

while let Some((edge, node)) = neighbors.next(&g) {
    // walker does not hold a graph borrow between calls
    if let Some(w) = g.edge_weight_mut(edge) {
        *w += 1;
    }

    if let Some(label) = g.node_weight_mut(node) {
        label.push_str("_visited");
    }
}
```

Detached directed walker:

```rust id="d1zo6j"
let mut incoming = g.neighbors_directed(a, Incoming).detach();

while let Some((edge, pred)) = incoming.next(&g) {
    println!("incoming edge {edge:?} from {pred:?}");
}
```

`Graph` and `StableGraph` docs both point to `.neighbors(a).detach()`, `.neighbors_directed(a, dir).detach()`, and `.neighbors_undirected(a).detach()` as walkers that do not borrow from the graph. ([Docs.rs][1])

---

## 11.14 Safe mutation while traversing: topology mutation discipline

Detached walker prevents borrow conflicts; it does **not** automatically make topology mutation semantically safe. Removing nodes/edges while walking can invalidate pending handles depending on graph family.

Safe payload-only mutation:

```rust id="v7o0ok"
let mut walk = g.neighbors(a).detach();

while let Some((edge, node)) = walk.next(&g) {
    if let Some(w) = g.edge_weight_mut(edge) {
        *w += 1;
    }

    if let Some(nw) = g.node_weight_mut(node) {
        *nw += 1;
    }
}
```

Safer topology mutation: collect first, mutate second

```rust id="s3cmqc"
let to_remove: Vec<_> = g
    .edges(a)
    .filter_map(|edge| {
        use petgraph::visit::EdgeRef;
        (*edge.weight() == 0).then_some(edge.id())
    })
    .collect();

for e in to_remove {
    g.remove_edge(e);
}
```

Graph deletion with side-map rebuild:

```rust id="qrkpe5"
let nodes_to_remove: Vec<_> = g
    .node_indices()
    .filter(|&n| g[n].starts_with("tmp_"))
    .collect();

for n in nodes_to_remove {
    g.remove_node(n);
}

// If using Graph + external maps:
by_id = rebuild_by_id(&g);
```

StableGraph deletion with validation:

```rust id="cqswls"
let nodes_to_remove: Vec<_> = stable
    .node_indices()
    .filter(|&n| stable[n].starts_with("tmp_"))
    .collect();

for n in nodes_to_remove {
    stable.remove_node(n);
}

// Unrelated NodeIndex handles remain valid.
// Removed handles must still be validated before use.
```

Rule:

```text id="aw92fo"
Payload mutation during detached traversal:
    generally safe with Option accessors

Edge removal during traversal:
    collect EdgeIndex first, remove after pass

Node removal during traversal:
    collect NodeIndex first, remove after pass

Graph + side maps:
    rebuild maps after removal

StableGraph:
    validate removed/stale handles; generation IDs if public handles exist
```

---

## 11.15 `Dfs`, `Bfs`, and walker state

`Walker<Context>` exposes `walk_next(context)` and `iter(context)`, and petgraph implements it for `Bfs`, `Dfs`, `DfsPostOrder`, and `Topo`; the docs say walkers can avoid holding a borrow of the graph they traverse. ([Docs.rs][8])

DFS traversal:

```rust id="2jrzi6"
use petgraph::visit::Dfs;

let mut dfs = Dfs::new(&g, start);

while let Some(nx) = dfs.next(&g) {
    println!("visit {nx:?}");
}
```

Mutation with traversal state:

```rust id="g737zz"
use petgraph::visit::Dfs;

let mut dfs = Dfs::new(&g, start);

while let Some(nx) = dfs.next(&g) {
    if let Some(node) = g.node_weight_mut(nx) {
        node.visited = true;
    }
}
```

Topology-mutation caution:

```text id="hiol0o"
Dfs/Bfs traversal state stores discovered/visited node IDs.
If Graph deletion shifts NodeIndex values during traversal:
    traversal state can become semantically invalid.

If topology must mutate during traversal:
    prefer StableGraph
    or collect mutations and apply after traversal
    or restart traversal after each topology edit
```

---

## 11.16 Dense arrays, sparse handles, and remapping

### Compact graph dense array

```rust id="6hbwhk"
use petgraph::visit::{IntoNodeIdentifiers, NodeCompactIndexable};

fn compact_scores<G>(g: G) -> Vec<u32>
where
    G: IntoNodeIdentifiers + NodeCompactIndexable,
    G::NodeId: Copy,
{
    let mut score = vec![0; g.node_bound()];

    for n in g.node_identifiers() {
        score[g.to_index(n)] += 1;
    }

    score
}
```

### StableGraph-safe dense remap

```rust id="7x5tvh"
use std::collections::HashMap;
use petgraph::visit::IntoNodeIdentifiers;

fn remap_live_nodes<G>(g: G) -> HashMap<G::NodeId, usize>
where
    G: IntoNodeIdentifiers,
    G::NodeId: Copy + Eq + std::hash::Hash,
{
    g.node_identifiers()
        .enumerate()
        .map(|(dense, node)| (node, dense))
        .collect()
}
```

StableGraph-safe vector algorithm:

```rust id="ue2037"
let live: Vec<_> = stable.node_indices().collect();

let dense: HashMap<_, _> = live
    .iter()
    .copied()
    .enumerate()
    .map(|(i, n)| (n, i))
    .collect();

let mut scores = vec![0u32; live.len()];

for &n in &live {
    let i = dense[&n];
    scores[i] = stable.neighbors(n).count() as u32;
}
```

Rule:

```text id="s7jzbg"
NodeIndex.index() as Vec offset:
    OK for Graph/Csr/GraphMap under compact trait constraints
    unsafe assumption for StableGraph after deletion
    acceptable for StableGraph only with Vec sized by node_bound and hole handling
```

---

## 11.17 Public API handle design

Bad public handle:

```rust id="fghdfs"
use petgraph::graph::NodeIndex;

pub struct PublicNode {
    pub ix: NodeIndex,
}
```

Problems:

```text id="87k346"
does not encode graph instance
does not encode generation
does not encode Ix policy explicitly
does not prevent stale/reused handle
ambiguous after compaction/rebuild
```

Better internal-only handle:

```rust id="m4ky6j"
use petgraph::graph::{NodeIndex, DefaultIx};

pub type AppIx = DefaultIx;
pub type AppNodeIndex = NodeIndex<AppIx>;

pub(crate) struct InternalNode {
    ix: AppNodeIndex,
}
```

Better public handle:

```rust id="x214rw"
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct PublicNodeId {
    raw: u64,        // domain ID or generated ID
    generation: u64,
}
```

Mapping layer:

```rust id="3nznwq"
use std::collections::HashMap;
use petgraph::graph::NodeIndex;

struct HandleMap {
    public_to_node: HashMap<PublicNodeId, NodeIndex>,
    node_to_public: HashMap<NodeIndex, PublicNodeId>,
}
```

Production rule:

```text id="4s8din"
Expose domain IDs publicly.
Keep NodeIndex private unless caller is graph-internal.
If exposing NodeIndex-like handles, add graph identity + generation.
```

---

## 11.18 Conversion and compaction safety

`From<Graph<...>> for StableGraph<...>` preserves node and edge indices and runs in `O(|V| + |E|)`; `From<StableGraph<...>> for Graph<...>` compacts holes and only preserves indices if the stable graph had no vacancies. ([Docs.rs][1])

Graph → StableGraph:

```rust id="r4qjxi"
use petgraph::graph::DiGraph;
use petgraph::stable_graph::StableDiGraph;

let mut g = DiGraph::<&str, ()>::new();

let a = g.add_node("a");
let b = g.add_node("b");

g.add_edge(a, b, ());

let stable: StableDiGraph<&str, ()> = g.into();

assert_eq!(stable.node_weight(a), Some(&"a"));
```

StableGraph → Graph compaction hazard:

```rust id="1conpl"
use petgraph::graph::DiGraph;
use petgraph::stable_graph::StableDiGraph;

let mut stable = StableDiGraph::<&str, ()>::new();

let a = stable.add_node("a");
let b = stable.add_node("b");
let c = stable.add_node("c");

stable.remove_node(b);

let compact: DiGraph<&str, ()> = stable.into();

// old NodeIndex values from stable are not generally valid for compact.
```

Rule:

```text id="uxw1w6"
After StableGraph -> Graph:
    discard old NodeIndex/EdgeIndex side maps
    rebuild maps from node/edge payload/domain IDs
```

---

## 11.19 Anti-pattern inventory

```text id="l5cb81"
Anti-pattern:
    Store Graph NodeIndex in UI selection while allowing node deletion.
Problem:
    Graph removal may shift another node into removed slot.
Fix:
    StableGraph, domain ID selection, or rebuild selection after deletion.

Anti-pattern:
    HashMap<DomainId, NodeIndex> with Graph + remove_node without rebuild.
Problem:
    side map may point to wrong nodes.
Fix:
    StableGraph or rebuild all map entries.

Anti-pattern:
    Use NodeIndex::new(i) from user input and index graph directly.
Problem:
    constructed handle is not proof of existence.
Fix:
    validate with node_weight / edge_weight.

Anti-pattern:
    Use NodeIndex.index() as stable persistent ID.
Problem:
    graph-local and mutation-dependent.
Fix:
    persistent domain ID + map.

Anti-pattern:
    Require NodeCompactIndexable in generic algorithm unnecessarily.
Problem:
    excludes StableGraph and hole-capable graphs.
Fix:
    use NodeIndexable + IntoNodeIdentifiers, or remap live nodes.

Anti-pattern:
    Remove nodes while Dfs/Bfs over Graph is in progress.
Problem:
    traversal state can hold shifted/stale NodeIndex values.
Fix:
    collect mutations, apply after traversal; or use StableGraph and validate.

Anti-pattern:
    Expose EdgeIndex as public API ID.
Problem:
    edge indices shift in Graph and are graph-local.
Fix:
    public edge ID / generation / payload ID.
```

---

## 11.20 Production deployment checklist

```text id="n7b7jk"
Choose identity strategy:
    internal algorithm only:
        NodeIndex / EdgeIndex acceptable

    external domain IDs:
        HashMap<DomainId, NodeIndex>

    deletion + external references:
        StableGraph + optional generation handles

    public API:
        expose domain IDs, not raw NodeIndex

Choose graph:
    Graph:
        compact, fast, broad API
        safe for append-only or rebuild-after-delete

    StableGraph:
        safe unrelated handles across deletion
        holes possible
        stale removed handles still need validation/generation

Choose generic bounds:
    NodeCompactIndexable:
        dense arrays, no holes required

    NodeIndexable:
        bitmap upper bound, holes possible

    IntoNodeIdentifiers:
        live node iteration

    EdgeIndexable:
        edge bitmap/index abstraction where available

Mutation while traversing:
    payload-only:
        detached walkers or traversal state + Option accessors

    topology mutation:
        collect first, mutate second
        or StableGraph + validation
        or restart traversal after edits

Boundary safety:
    never index with user-provided handles directly
    validate with node_weight / edge_weight
    clear or revalidate cached handles after removal
```

Final rule:

```text id="nw36tq"
NodeIndex and EdgeIndex are handles, not identities.
Graph gives compactness.
StableGraph gives live-handle continuity.
Domain IDs give persistence.
Generation IDs give stale-handle safety.
```

[1]: https://docs.rs/petgraph/latest/petgraph/graph/struct.Graph.html "Graph in petgraph::graph - Rust"
[2]: https://docs.rs/petgraph/latest/petgraph/graph/struct.NodeIndex.html "NodeIndex in petgraph::graph - Rust"
[3]: https://docs.rs/petgraph/latest/petgraph/graph/struct.EdgeIndex.html "EdgeIndex in petgraph::graph - Rust"
[4]: https://docs.rs/petgraph/latest/petgraph/stable_graph/struct.StableGraph.html "StableGraph in petgraph::stable_graph - Rust"
[5]: https://docs.rs/petgraph/latest/petgraph/visit/trait.NodeIndexable.html "NodeIndexable in petgraph::visit - Rust"
[6]: https://docs.rs/petgraph/latest/petgraph/visit/trait.NodeCompactIndexable.html "NodeCompactIndexable in petgraph::visit - Rust"
[7]: https://docs.rs/petgraph/latest/petgraph/visit/trait.EdgeIndexable.html "EdgeIndexable in petgraph::visit - Rust"
[8]: https://docs.rs/petgraph/latest/petgraph/visit/trait.Walker.html "Walker in petgraph::visit - Rust"


# 12) Construction patterns and graph loading — petgraph

Format follows the uploaded advanced-reference style. 

Target: **petgraph 0.8.3 released API**. This section covers construction APIs, import pipelines, bulk loading, edge/node deduplication, `FromElements`, algorithm-output-to-graph patterns, and error-first construction. Petgraph exposes several graph families and supports arbitrary node/edge weights, directed/undirected edges, Graphviz DOT output, `serde` feature-gated serialization, `graph6`, and optional DOT parser dependencies through the `dot_parser` feature. ([Docs.rs][1])

---

## 12.0 Construction API taxonomy

```text id="ctlwwm"
Low-level incremental:
    Graph::new
    Graph::new_undirected
    Graph::with_capacity
    add_node
    add_edge
    update_edge
    try_add_node
    try_add_edge
    try_update_edge

Bulk topology:
    Graph::from_edges
    Graph::extend_with_edges
    Csr::from_sorted_edges
    GraphMap::from_edges
    MatrixGraph::from_edges

Element stream:
    FromElements::from_elements
    data::Element::{Node, Edge}
    ElementIterator::filter_elements

Algorithm output:
    algo iterators producing data::Element
    FromElements::from_elements(output)

Format import/export:
    graph6
    DOT output via dot::Dot
    DOT parser feature / dot-parser dependency
    serde-1 serialized graph values
    custom edge-list / adjacency-list / matrix schemas
```

Core distinction:

```text id="f4mlay"
Construction from known topology:
    preallocate / batch / sort / deduplicate

Construction from external records:
    validate / normalize / intern IDs / choose graph representation

Construction from algorithm outputs:
    prefer FromElements when output iterator is Element<N,E>

Construction from persistent files:
    prefer stable domain schema over raw NodeIndex serialization
```

---

## 12.1 Constructors: `new`, `new_undirected`, `with_capacity`

### Directed default: `Graph::new`

```rust id="ru28n6"
use petgraph::Graph;

let mut g = Graph::<&str, u32>::new();

let a = g.add_node("api");
let b = g.add_node("db");

g.add_edge(a, b, 4);
```

`Graph::new()` creates an empty directed graph; `Graph<N,E,Ty,Ix>` is an adjacency-list graph where `N` and `E` are arbitrary node/edge weights, `Ty` selects directedness, and `Ix` controls index size. `add_node` and `add_edge` are `O(1)` and `Graph` uses `O(|V| + |E|)` space. ([Docs.rs][2])

### Undirected constructor: `Graph::new_undirected`

```rust id="lts14n"
use petgraph::Graph;
use petgraph::Undirected;

let mut g = Graph::<&str, u32, Undirected>::new_undirected();

let a = g.add_node("a");
let b = g.add_node("b");

g.add_edge(a, b, 1);
```

Alias-preferred form:

```rust id="w6e33a"
use petgraph::graph::UnGraph;

let mut g: UnGraph<&str, u32> = UnGraph::new_undirected();
```

`UnGraph<N,E,Ix>` is an alias for `Graph<N,E,Undirected,Ix>`, and an edge between `1` and `2` is equivalent to an edge between `2` and `1`. ([Docs.rs][3])

### Capacity constructor: `Graph::with_capacity(nodes, edges)`

```rust id="ga0cgu"
use petgraph::graph::DiGraph;

let estimated_nodes = 100_000;
let estimated_edges = 1_000_000;

let mut g = DiGraph::<String, u32>::with_capacity(
    estimated_nodes,
    estimated_edges,
);
```

Use `with_capacity` when import cardinalities are known or cheaply estimated:

```text id="0ly5v4"
CSV/Parquet edge-list import
database snapshot import
package dependency graph
service inventory graph
compiler module graph
one-shot algorithm graph
```

Capacity rule:

```text id="dtyrzy"
Graph:
    with_capacity(nodes, edges)

StableGraph:
    with_capacity(nodes, edges)

GraphMap:
    with_capacity(nodes, edges)

MatrixGraph:
    with_capacity(node_capacity)

Csr:
    with_nodes(node_count)
    from_sorted_edges(sorted_unique_edges)
```

The feature and graph-family docs define multiple graph types with different representation tradeoffs; `Graph`, `StableGraph`, `GraphMap`, and `MatrixGraph` are default-enabled graph families, and `Csr` is available in the crate API. ([Docs.rs][1])

---

## 12.2 `from_edges`: compact topology bootstrap

### `Graph::<(), ()>::from_edges`

```rust id="b61vib"
use petgraph::Graph;

let g = Graph::<(), ()>::from_edges([
    (0, 1),
    (1, 2),
    (2, 0),
]);

assert_eq!(g.node_count(), 3);
assert_eq!(g.edge_count(), 3);
```

### Weighted edge tuples

```rust id="isocfr"
use petgraph::Graph;

let g = Graph::<(), u32>::from_edges([
    (0, 1, 10),
    (1, 2, 20),
    (2, 0, 30),
]);
```

### Undirected topology

```rust id="ztio5f"
use petgraph::graph::UnGraph;

let g = UnGraph::<(), ()>::from_edges([
    (0, 1),
    (1, 2),
    (2, 0),
]);
```

`from_edges` is shown in the crate overview as a terse way to create a graph from numeric endpoint pairs; omitted node/edge weights are defaulted, and the overview imports `FromElements` because algorithm outputs and element streams can also construct graphs. ([Docs.rs][1])

Use `from_edges` when:

```text id="c8pq0h"
node IDs are compact numeric positions
node weights can be Default / ()
edge weights are direct tuple fields or Default
topology is small/medium or test fixture
index assignment from endpoint integers is acceptable
```

Avoid `from_edges` when:

```text id="h1f1cj"
external IDs are String/UUID
node weights are rich domain structs unavailable at default construction
edge list is untrusted and Ix overflow should be fallible
dedup/validation required before insertion
stable external ID mapping must be preserved explicitly
```

---

## 12.3 `extend_with_edges`: add edge tuples to existing graph

```rust id="mwsp2p"
use petgraph::Graph;

let mut g = Graph::<&str, u32>::new();

let api = g.add_node("api");
let db = g.add_node("db");
let cache = g.add_node("cache");

g.extend_with_edges([
    (api, db, 4),
    (api, cache, 1),
]);
```

Pattern:

```text id="hsrrpp"
1. create graph
2. add rich node weights explicitly
3. keep returned NodeIndex values
4. extend edges with NodeIndex endpoint tuples
```

Use `extend_with_edges` when node payload construction is separate from edge construction:

```rust id="glhc73"
use petgraph::graph::DiGraph;
use petgraph::graph::NodeIndex;

#[derive(Clone, Debug)]
struct Service {
    name: String,
    tier: String,
}

let mut g = DiGraph::<Service, u32>::with_capacity(3, 2);

let nodes: Vec<NodeIndex> = [
    Service { name: "api".into(), tier: "frontend".into() },
    Service { name: "db".into(), tier: "storage".into() },
    Service { name: "cache".into(), tier: "storage".into() },
]
.into_iter()
.map(|svc| g.add_node(svc))
.collect();

g.extend_with_edges([
    (nodes[0], nodes[1], 4),
    (nodes[0], nodes[2], 1),
]);
```

---

## 12.4 Incremental construction from domain records

### Record model

```rust id="x7yyq5"
#[derive(Clone, Debug)]
struct ServiceRecord {
    id: String,
    tier: String,
}

#[derive(Clone, Debug)]
struct DependencyRecord {
    from: String,
    to: String,
    protocol: String,
    p95_ms: u32,
}

#[derive(Clone, Debug)]
struct ServiceNode {
    id: String,
    tier: String,
}

#[derive(Clone, Debug)]
struct DependencyEdge {
    protocol: String,
    p95_ms: u32,
}
```

### `Graph + HashMap<DomainId, NodeIndex>`

```rust id="8ves6o"
use std::collections::HashMap;
use petgraph::graph::{DiGraph, NodeIndex};

struct Builder {
    graph: DiGraph<ServiceNode, DependencyEdge>,
    by_id: HashMap<String, NodeIndex>,
}

impl Builder {
    fn new(expected_nodes: usize, expected_edges: usize) -> Self {
        Self {
            graph: DiGraph::with_capacity(expected_nodes, expected_edges),
            by_id: HashMap::with_capacity(expected_nodes),
        }
    }

    fn intern_service(&mut self, id: &str, tier: &str) -> NodeIndex {
        if let Some(&ix) = self.by_id.get(id) {
            return ix;
        }

        let ix = self.graph.add_node(ServiceNode {
            id: id.to_owned(),
            tier: tier.to_owned(),
        });

        self.by_id.insert(id.to_owned(), ix);
        ix
    }

    fn add_dependency(&mut self, rec: DependencyRecord) {
        let from = self.intern_service(&rec.from, "unknown");
        let to = self.intern_service(&rec.to, "unknown");

        self.graph.update_edge(from, to, DependencyEdge {
            protocol: rec.protocol,
            p95_ms: rec.p95_ms,
        });
    }
}
```

Use this pattern when:

```text id="zfalff"
domain IDs are strings / UUIDs / non-Copy
node payload differs from ID
duplicate records should update or aggregate one edge
petgraph algorithms need compact NodeIndex handles
GraphMap bounds are unsuitable
```

`Graph::update_edge` adds or updates an edge and avoids parallel duplicates, while `Graph::add_edge` always adds a new edge and explicitly allows parallel duplicate edges; choose according to edge semantics. ([Docs.rs][2])

---

## 12.5 Deduplicating nodes

### Interning strategy

```rust id="xy1cwn"
use std::collections::HashMap;
use petgraph::graph::{DiGraph, NodeIndex};

fn intern<K, N, E>(
    graph: &mut DiGraph<N, E>,
    map: &mut HashMap<K, NodeIndex>,
    key: K,
    make_node: impl FnOnce(&K) -> N,
) -> NodeIndex
where
    K: Eq + std::hash::Hash + Clone,
{
    if let Some(&ix) = map.get(&key) {
        ix
    } else {
        let ix = graph.add_node(make_node(&key));
        map.insert(key, ix);
        ix
    }
}
```

### Sort/group strategy for batch imports

```rust id="9vf5y7"
use std::collections::HashMap;
use petgraph::graph::{DiGraph, NodeIndex};

fn build_node_map(
    graph: &mut DiGraph<String, ()>,
    ids: impl IntoIterator<Item = String>,
) -> HashMap<String, NodeIndex> {
    let mut unique: Vec<String> = ids.into_iter().collect();
    unique.sort();
    unique.dedup();

    let mut by_id = HashMap::with_capacity(unique.len());

    for id in unique {
        let ix = graph.add_node(id.clone());
        by_id.insert(id, ix);
    }

    by_id
}
```

Node-dedup rule:

```text id="102lo9"
Streaming load:
    HashMap interner

Batch load:
    sort + dedup IDs first
    preallocate exact/estimated node capacity
    add nodes in deterministic order if reproducible NodeIndex assignment matters

Stable persisted identity:
    never rely solely on NodeIndex
    keep external ID in payload or side map
```

---

## 12.6 Deduplicating edges

### `Graph::update_edge`: one edge per endpoint pair

```rust id="d6df1o"
let edge = graph.update_edge(from, to, DependencyEdge {
    protocol: "tcp".to_owned(),
    p95_ms: 4,
});
```

Use when:

```text id="vxoeqp"
latest record wins
endpoint pair is the logical edge identity
parallel edges are not wanted
local adjacency scan cost is acceptable
```

### Accumulate duplicate edge records

```rust id="hrznb2"
use petgraph::graph::{DiGraph, NodeIndex};

#[derive(Clone, Debug, Default)]
struct EdgeAgg {
    count: u64,
    total_p95_ms: u64,
}

fn add_or_accumulate(
    g: &mut DiGraph<String, EdgeAgg>,
    a: NodeIndex,
    b: NodeIndex,
    p95_ms: u32,
) {
    if let Some(e) = g.find_edge(a, b) {
        let agg = g.edge_weight_mut(e).unwrap();
        agg.count += 1;
        agg.total_p95_ms += p95_ms as u64;
    } else {
        g.add_edge(a, b, EdgeAgg {
            count: 1,
            total_p95_ms: p95_ms as u64,
        });
    }
}
```

### Pre-aggregate with `BTreeMap`

```rust id="0f5atv"
use std::collections::BTreeMap;

fn aggregate_edges(
    raw: impl IntoIterator<Item = (usize, usize, u32)>,
) -> Vec<(usize, usize, u32)> {
    let mut agg = BTreeMap::<(usize, usize), u32>::new();

    for (u, v, w) in raw {
        *agg.entry((u, v)).or_insert(0) += w;
    }

    agg.into_iter()
        .map(|((u, v), w)| (u, v, w))
        .collect()
}
```

Edge-dedup rule:

```text id="qdrg5e"
Graph + add_edge:
    preserves duplicate/parallel edges

Graph + update_edge:
    overwrites one edge per pair

GraphMap:
    add_edge overwrites and returns old weight

MatrixGraph:
    update_edge overwrites; add_edge panics on duplicate

Csr:
    requires sorted unique pairs for from_sorted_edges
```

---

## 12.7 Error-first construction: `try_add_node`, `try_add_edge`, `try_update_edge`

### Fallible node insertion

```rust id="yl6y1a"
use petgraph::graph::{DiGraph, GraphError};

fn insert_node(
    g: &mut DiGraph<String, ()>,
    label: String,
) -> Result<petgraph::graph::NodeIndex, GraphError> {
    g.try_add_node(label)
}
```

`try_add_node` returns `GraphError::NodeIxLimit` when the graph has reached the maximum node count for its index type; `add_node` panics in that case. ([Docs.rs][2])

### Fallible edge insertion

```rust id="zi04tw"
use petgraph::graph::{DiGraph, GraphError, NodeIndex};

fn insert_edge(
    g: &mut DiGraph<String, u32>,
    a: NodeIndex,
    b: NodeIndex,
    weight: u32,
) -> Result<(), GraphError> {
    g.try_add_edge(a, b, weight)?;
    Ok(())
}
```

`try_add_edge` is the non-panicking endpoint/capacity-checked form of edge insertion, while `add_edge` panics for missing endpoints or edge-index limit exhaustion. ([Docs.rs][2])

### Fallible add-or-update

```rust id="b54h8w"
use petgraph::graph::{DiGraph, GraphError, NodeIndex};

fn upsert_edge(
    g: &mut DiGraph<String, u32>,
    a: NodeIndex,
    b: NodeIndex,
    weight: u32,
) -> Result<petgraph::graph::EdgeIndex, GraphError> {
    g.try_update_edge(a, b, weight)
}
```

Use fallible APIs when:

```text id="50ftws"
input is external/untrusted
Ix is small: u8/u16
service should return structured error instead of panic
graph size limits are part of validation
endpoint NodeIndex values are decoded from user data
```

Boundary pattern:

```rust id="nool96"
#[derive(Debug)]
enum BuildError {
    UnknownNode(String),
    Graph(petgraph::graph::GraphError),
}

impl From<petgraph::graph::GraphError> for BuildError {
    fn from(e: petgraph::graph::GraphError) -> Self {
        BuildError::Graph(e)
    }
}
```

---

## 12.8 `FromElements`: element-stream construction

`data::Element<N,E>` has two variants: `Node { weight: N }` and `Edge { source: usize, target: usize, weight: E }`. A sequence of elements implicitly assigns node indices by node appearance order, and edge source/target fields refer to those indices. `FromElements::from_elements` builds a graph from an iterator of `Element`s; implementations exist for `Graph`, `StableGraph`, and `GraphMap` under their feature gates. ([Docs.rs][4])

### Manual element stream

```rust id="rxkq96"
use petgraph::data::{Element, FromElements};
use petgraph::graph::DiGraph;

let elements = vec![
    Element::Node { weight: "a" },
    Element::Node { weight: "b" },
    Element::Node { weight: "c" },
    Element::Edge { source: 0, target: 1, weight: 10 },
    Element::Edge { source: 1, target: 2, weight: 20 },
];

let g: DiGraph<&str, u32> = DiGraph::from_elements(elements);

assert_eq!(g.node_count(), 3);
assert_eq!(g.edge_count(), 2);
```

### Element-stream filtering

`ElementIterator::filter_elements` adapts an iterator of `Element`s, calls a predicate with mutable references to node/edge weights, removes elements when the predicate returns `false`, and adapts edge source/target indices after node removals. ([Docs.rs][5])

```rust id="dovhkv"
use petgraph::data::{Element, ElementIterator, FromElements};
use petgraph::graph::DiGraph;

let elements = vec![
    Element::Node { weight: 1 },
    Element::Node { weight: 2 },
    Element::Node { weight: 3 },
    Element::Edge { source: 0, target: 1, weight: 10 },
    Element::Edge { source: 1, target: 2, weight: 20 },
];

let filtered = elements.into_iter().filter_elements(|element| {
    match element {
        Element::Node { weight } => *weight >= 2,
        Element::Edge { weight, .. } => *weight >= 10,
    }
});

let g: DiGraph<i32, i32> = DiGraph::from_elements(filtered);
```

Agent rule:

```text id="cqra50"
Use FromElements when:
    algorithm returns Element stream
    transform pipeline naturally emits nodes then edges
    source/target indices refer to element-order node indices
    index adaptation after filtering is desired

Avoid FromElements when:
    source IDs are domain IDs needing interning
    edge endpoints are NodeIndex values from an existing graph
    validation needs custom error reporting
```

---

## 12.9 Algorithm-output-to-graph patterns

### Minimum spanning tree → graph

```rust id="feq1c9"
use petgraph::algo::min_spanning_tree;
use petgraph::data::FromElements;
use petgraph::graph::UnGraph;

let g = UnGraph::<i32, i32>::from_edges([
    (0, 1, 10),
    (1, 2, 5),
    (0, 2, 100),
]);

let mst: UnGraph<i32, i32> =
    UnGraph::from_elements(min_spanning_tree(&g));
```

`min_spanning_tree` returns an iterator of graph elements and is intended to be consumed by `FromElements`; its docs state it computes a minimum spanning forest using Kruskal’s algorithm with runtime `O(|E| log |E|)`, and the resulting graph has all vertices of the input with identical node indices plus `|V| - c` edges for `c` components. ([Docs.rs][6])

### Algorithm-output pipeline with element filtering

```rust id="qvz2cu"
use petgraph::algo::min_spanning_tree;
use petgraph::data::{Element, ElementIterator, FromElements};
use petgraph::graph::UnGraph;

let mst_filtered = min_spanning_tree(&g).filter_elements(|element| {
    match element {
        Element::Node { .. } => true,
        Element::Edge { weight, .. } => *weight <= 50,
    }
});

let mst: UnGraph<i32, i32> = UnGraph::from_elements(mst_filtered);
```

Rule:

```text id="h49dvh"
Algorithm returns Element<N,E>:
    FromElements::from_elements(output)

Algorithm returns node/edge sets:
    build manually with capacity + map/remap

Algorithm returns distances/maps:
    usually not graph topology; store separately or create derived graph intentionally
```

---

## 12.10 Loading from edge lists

### Compact numeric edge list

```rust id="s5fwjq"
use petgraph::graph::DiGraph;

fn load_numeric_edges(edges: &[(usize, usize, u32)]) -> DiGraph<(), u32> {
    DiGraph::<(), u32>::from_edges(edges.iter().copied())
}
```

### External string edge list

```rust id="jkviyj"
use std::collections::HashMap;
use petgraph::graph::{DiGraph, NodeIndex};

fn load_string_edges<I>(records: I) -> (DiGraph<String, u32>, HashMap<String, NodeIndex>)
where
    I: IntoIterator<Item = (String, String, u32)>,
{
    let mut g = DiGraph::<String, u32>::new();
    let mut by_id = HashMap::<String, NodeIndex>::new();

    for (src, dst, w) in records {
        let a = if let Some(&ix) = by_id.get(&src) {
            ix
        } else {
            let ix = g.add_node(src.clone());
            by_id.insert(src, ix);
            ix
        };

        let b = if let Some(&ix) = by_id.get(&dst) {
            ix
        } else {
            let ix = g.add_node(dst.clone());
            by_id.insert(dst, ix);
            ix
        };

        g.update_edge(a, b, w);
    }

    (g, by_id)
}
```

### Batch edge list → CSR

```rust id="lgdwcf"
use petgraph::csr::Csr;

fn load_csr(mut edges: Vec<(usize, usize, u32)>) -> Csr<(), u32> {
    edges.sort_by_key(|&(u, v, _)| (u, v));
    edges.dedup_by_key(|edge| (edge.0, edge.1));

    Csr::<(), u32>::from_sorted_edges(&edges)
        .expect("sorted unique edge list")
}
```

`Csr::from_sorted_edges` requires edges sorted and unique by `(u, v)` pair and computes in `O(|V| + |E|)` time, making it the preferred large sparse static edge-list loader. ([Docs.rs][7])

---

## 12.11 Loading from adjacency lists

### Numeric adjacency list

```rust id="2gvx1i"
use petgraph::graph::DiGraph;

fn from_adjacency(adjacency: &[Vec<usize>]) -> DiGraph<(), ()> {
    let mut g = DiGraph::<(), ()>::with_capacity(
        adjacency.len(),
        adjacency.iter().map(Vec::len).sum(),
    );

    let nodes: Vec<_> = (0..adjacency.len())
        .map(|_| g.add_node(()))
        .collect();

    for (src, dsts) in adjacency.iter().enumerate() {
        for &dst in dsts {
            g.update_edge(nodes[src], nodes[dst], ());
        }
    }

    g
}
```

### Weighted adjacency list

```rust id="2d9y5p"
use petgraph::graph::DiGraph;

fn from_weighted_adjacency(adjacency: &[Vec<(usize, u32)>]) -> DiGraph<(), u32> {
    let edge_count = adjacency.iter().map(Vec::len).sum();

    let mut g = DiGraph::<(), u32>::with_capacity(adjacency.len(), edge_count);

    let nodes: Vec<_> = (0..adjacency.len())
        .map(|_| g.add_node(()))
        .collect();

    for (src, dsts) in adjacency.iter().enumerate() {
        for &(dst, w) in dsts {
            g.update_edge(nodes[src], nodes[dst], w);
        }
    }

    g
}
```

Adjacency-list rule:

```text id="8j75up"
If adjacency list is sparse and mutable:
    Graph

If adjacency list is sorted unique and static:
    flatten to edge list -> Csr::from_sorted_edges

If node identifiers are already Copy keys:
    GraphMap
```

---

## 12.12 Loading from adjacency matrices

### Dense Boolean matrix → `Graph`

```rust id="jz8px3"
use petgraph::graph::DiGraph;

fn from_bool_matrix(m: &[Vec<bool>]) -> DiGraph<(), ()> {
    let n = m.len();
    let edge_estimate = m.iter().flatten().filter(|&&x| x).count();

    let mut g = DiGraph::<(), ()>::with_capacity(n, edge_estimate);
    let nodes: Vec<_> = (0..n).map(|_| g.add_node(())).collect();

    for i in 0..n {
        for j in 0..n {
            if m[i][j] {
                g.add_edge(nodes[i], nodes[j], ());
            }
        }
    }

    g
}
```

### Dense weighted matrix → `MatrixGraph`

```rust id="kz0y4n"
use petgraph::matrix_graph::DiMatrix;

fn from_dense_cost_matrix(m: &[Vec<Option<u32>>]) -> DiMatrix<(), u32> {
    let n = m.len();

    let mut g = DiMatrix::<(), u32>::with_capacity(n);
    let nodes: Vec<_> = (0..n).map(|_| g.add_node(())).collect();

    for i in 0..n {
        for j in 0..n {
            if let Some(w) = m[i][j] {
                g.update_edge(nodes[i], nodes[j], w);
            }
        }
    }

    g
}
```

Matrix rule:

```text id="l8c8z3"
Sparse matrix:
    scan nonzeros -> edge list -> Graph/Csr

Dense matrix:
    MatrixGraph

Need multiedges:
    matrix input cannot represent parallel edges directly
```

`MatrixGraph` is backed by an adjacency matrix, uses `O(|V|²)` space, and is suited to dense graphs and fast endpoint-pair edge operations. ([Docs.rs][1])

---

## 12.13 Loading DOT / Graphviz

### DOT output

```rust id="ob0oqv"
use petgraph::dot::{Dot, Config};
use petgraph::graph::DiGraph;

let mut g = DiGraph::<&str, &str>::new();

let a = g.add_node("api");
let b = g.add_node("db");

g.add_edge(a, b, "tcp");

println!("{:?}", Dot::with_config(&g, &[Config::EdgeNoLabel]));
```

`dot::Dot` outputs Graphviz `.dot` text for compatible graph types; its docs describe it as simple DOT output, mostly intended for debugging, and warn that exact output may change. ([Docs.rs][8])

### DOT parser feature

Cargo:

```toml id="ccx2zh"
[dependencies]
petgraph = { version = "0.8.3", features = ["dot_parser"] }
```

The `dot_parser` feature pulls in the `dot-parser` and `dot-parser-macros` dependencies and requires `std`; the README/crates metadata describe DOT import/export support, while the `dot` module itself is the output module. ([Docs.rs][9])

Practical advisory:

```text id="zqp0e4"
DOT is excellent for:
    visualization
    debug artifacts
    small test fixtures
    manual graph inspection

DOT is weak as a production interchange:
    exact Dot output may change
    Graphviz DOT has broad syntax surface
    attributes need domain mapping
    parser feature/dependency is optional

Production import:
    parse DOT -> domain records -> validate -> build graph
```

---

## 12.14 Loading Graph6

### `FromGraph6`

```rust id="hwzuck"
use petgraph::graph::UnGraph;
use petgraph::graph6::FromGraph6;

let g: UnGraph<(), ()> =
    UnGraph::from_graph6_string("D?{".to_owned());
```

`FromGraph6` converts a graph6 string into an undirected graph; implementors include `Graph<(),(),Undirected,Ix>`, `StableGraph<(),(),Undirected,Ix>`, `Csr<(),(),Undirected,Ix>`, and `GraphMap<Ix,(),Undirected,S>` under relevant feature gates. ([Docs.rs][10])

### Raw graph6 representation

```rust id="9zgx9w"
use petgraph::graph6::from_graph6_representation;
use petgraph::graph::DefaultIx;

let (order, edges) =
    from_graph6_representation::<DefaultIx>("D?{".to_owned());

println!("nodes={order}, edges={edges:?}");
```

`from_graph6_representation` converts a graph6 string into graph order plus edges usable to construct an undirected graph. ([Docs.rs][11])

### Graph6 export

```rust id="6qxt1q"
use petgraph::graph6::get_graph6_representation;

let s = get_graph6_representation(&g);
```

`get_graph6_representation` converts any graph implementing `GetAdjacencyMatrix + IntoNodeIdentifiers` into a graph6 string. ([Docs.rs][12])

Graph6 rule:

```text id="n4gvru"
Use graph6 when:
    undirected unlabeled topology is enough
    node/edge weights are ()
    compact graph exchange/test fixtures matter

Do not use graph6 when:
    node labels/weights must be preserved
    edge weights must be preserved
    directed graphs are required
    domain schema is richer than topology
```

---

## 12.15 Loading serialized data

Cargo:

```toml id="wc5u4v"
[dependencies]
petgraph = { version = "0.8.3", features = ["serde-1"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

`Graph` implements `Serialize` / `Deserialize` behind `serde-1` when `N`, `E`, `Ty`, and `Ix` satisfy serde bounds; the feature page shows `serde-1` enabling `serde` and `serde_derive`. ([Docs.rs][2])

```rust id="ouq8t4"
use petgraph::graph::DiGraph;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct NodeData {
    id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EdgeData {
    cost: u32,
}

fn roundtrip(g: &DiGraph<NodeData, EdgeData>) -> serde_json::Result<DiGraph<NodeData, EdgeData>> {
    let json = serde_json::to_string(g)?;
    serde_json::from_str(&json)
}
```

Production serialization rule:

```text id="j0rfw1"
Internal cache / version-pinned binary:
    raw petgraph serde may be acceptable

Public long-term format:
    prefer domain schema:
        nodes: [{id, payload}]
        edges: [{source_id, target_id, payload}]
    rebuild petgraph on load

Reason:
    NodeIndex is graph-local
    Graph vs StableGraph has different index guarantees
    cross-version representation should be explicitly owned by application
```

Domain schema loader:

```rust id="vx98ft"
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use petgraph::graph::{DiGraph, NodeIndex};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct NodeRow {
    id: String,
    tier: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EdgeRow {
    source: String,
    target: String,
    cost: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct GraphFile {
    nodes: Vec<NodeRow>,
    edges: Vec<EdgeRow>,
}

fn from_file_model(file: GraphFile) -> Result<DiGraph<NodeRow, u32>, String> {
    let mut g = DiGraph::<NodeRow, u32>::with_capacity(
        file.nodes.len(),
        file.edges.len(),
    );

    let mut by_id = HashMap::<String, NodeIndex>::with_capacity(file.nodes.len());

    for node in file.nodes {
        if by_id.contains_key(&node.id) {
            return Err(format!("duplicate node id: {}", node.id));
        }

        let id = node.id.clone();
        let ix = g.add_node(node);
        by_id.insert(id, ix);
    }

    for edge in file.edges {
        let source = *by_id
            .get(&edge.source)
            .ok_or_else(|| format!("unknown source: {}", edge.source))?;

        let target = *by_id
            .get(&edge.target)
            .ok_or_else(|| format!("unknown target: {}", edge.target))?;

        g.update_edge(source, target, edge.cost);
    }

    Ok(g)
}
```

---

## 12.16 Capacity planning for large imports

### Pre-scan planning

```text id="3iqzd3"
If input supports pre-scan:
    count nodes
    count edges
    estimate duplicates
    choose graph type
    choose Ix
    choose capacity
```

### Strategy table

| Input                          | Recommended graph       | Planning                               |
| ------------------------------ | ----------------------- | -------------------------------------- |
| unsorted streaming edge list   | `Graph` / `GraphMap`    | reserve estimate; intern IDs           |
| sorted static sparse edge list | `Csr`                   | sort/dedup first; `from_sorted_edges`  |
| dense matrix                   | `MatrixGraph`           | `with_capacity(n)`                     |
| deletion-heavy editable graph  | `StableGraph`           | `with_capacity(nodes, edges)`          |
| multigraph event log           | `Graph`                 | use `add_edge`, not `update_edge`      |
| simple keyed graph             | `GraphMap`              | `with_capacity(nodes, edges)`          |
| domain file schema             | `Graph` / `StableGraph` | add nodes first, validate edges second |

### Import sizing code

```rust id="grnf3o"
use petgraph::graph::DiGraph;

fn graph_with_import_capacity<N, E>(
    expected_nodes: usize,
    expected_edges: usize,
) -> DiGraph<N, E> {
    DiGraph::with_capacity(expected_nodes, expected_edges)
}
```

### `Ix` guard

```rust id="lbxo2t"
use petgraph::graph::DiGraph;

type SmallGraph<N, E> = DiGraph<N, E, u16>;

fn ensure_ix_capacity(nodes: usize, edges: usize) -> Result<(), String> {
    if nodes > u16::MAX as usize {
        return Err("too many nodes for u16 graph".to_owned());
    }

    if edges > u16::MAX as usize {
        return Err("too many edges for u16 graph".to_owned());
    }

    Ok(())
}
```

`DefaultIx` is `u32`, chosen to reduce graph data size and improve common-case performance; smaller index types can improve memory locality but impose hard graph-size ceilings. ([Docs.rs][13])

---

## 12.17 Representation-specific loading recipes

### `Graph`: general mutable sparse import

```rust id="2wrtz4"
let mut g = petgraph::graph::DiGraph::<NodeData, EdgeData>::with_capacity(n, e);
```

Best for:

```text id="jmwzpe"
mutable sparse graph
parallel edges possible
broad method/algorithm compatibility
domain records with rich payload
```

### `StableGraph`: editable graph with stable handles

```rust id="v37lop"
let mut g = petgraph::stable_graph::StableDiGraph::<NodeData, EdgeData>::with_capacity(n, e);
```

Best for:

```text id="0e3ls4"
external NodeIndex maps survive unrelated deletions
UI selections
incremental algorithms
long-lived handles
```

`StableGraph` does not invalidate unrelated node/edge indices when items are removed, but holes can form and some methods lag `Graph` parity. ([Docs.rs][14])

### `GraphMap`: key-identity simple import

```rust id="xs334r"
use petgraph::graphmap::DiGraphMap;

let mut g = DiGraphMap::<u64, u32>::with_capacity(n, e);

for (src, dst, weight) in records {
    g.add_edge(src, dst, weight);
}
```

Best for:

```text id="kxkgbs"
node ID is Copy + Hash + Ord
no parallel edges
edge existence lookup is hot
automatic node insertion acceptable
```

`GraphMap` uses node values as mapping keys and is an associative-array graph representation.

### `MatrixGraph`: dense matrix import

```rust id="6id460"
use petgraph::matrix_graph::DiMatrix;

let mut g = DiMatrix::<NodeData, EdgeData>::with_capacity(node_count);
```

Best for:

```text id="aflrlk"
dense graph
bounded node count
hot endpoint-pair lookup
one edge per endpoint pair
```

### `Csr`: sorted static sparse import

```rust id="t1ev5d"
use petgraph::csr::Csr;

let g = Csr::<(), u32>::from_sorted_edges(&sorted_unique_edges)?;
```

Best for:

```text id="wwsfcm"
mostly static sparse graph
fast outgoing-edge traversal
large imported edge lists
no parallel edges
```

---

## 12.18 Error-first builder skeleton

```rust id="8nol4j"
use std::collections::HashMap;
use petgraph::graph::{DiGraph, NodeIndex, GraphError};

#[derive(Clone, Debug)]
struct NodeData {
    id: String,
}

#[derive(Clone, Debug)]
struct EdgeData {
    cost: u32,
}

#[derive(Debug)]
enum BuildError {
    DuplicateNode(String),
    UnknownSource(String),
    UnknownTarget(String),
    Graph(GraphError),
}

impl From<GraphError> for BuildError {
    fn from(e: GraphError) -> Self {
        BuildError::Graph(e)
    }
}

fn build_checked(
    nodes: Vec<NodeData>,
    edges: Vec<(String, String, EdgeData)>,
) -> Result<DiGraph<NodeData, EdgeData>, BuildError> {
    let mut g = DiGraph::with_capacity(nodes.len(), edges.len());
    let mut by_id = HashMap::<String, NodeIndex>::with_capacity(nodes.len());

    for node in nodes {
        if by_id.contains_key(&node.id) {
            return Err(BuildError::DuplicateNode(node.id));
        }

        let id = node.id.clone();
        let ix = g.try_add_node(node)?;
        by_id.insert(id, ix);
    }

    for (source, target, payload) in edges {
        let a = *by_id
            .get(&source)
            .ok_or_else(|| BuildError::UnknownSource(source.clone()))?;

        let b = *by_id
            .get(&target)
            .ok_or_else(|| BuildError::UnknownTarget(target.clone()))?;

        g.try_update_edge(a, b, payload)?;
    }

    Ok(g)
}
```

Use this when:

```text id="wt07ri"
input is untrusted
duplicate node IDs should be rejected
unknown edge endpoints should be rejected
Ix limit should be a returned error
duplicate edge policy is explicit
```

---

## 12.19 Loading pipeline decision table

| Source                          | Primary pipeline                                          | Best graph target                                       |
| ------------------------------- | --------------------------------------------------------- | ------------------------------------------------------- |
| Compact edge tuples             | `from_edges`                                              | `Graph`, `UnGraph`, `GraphMap`, `MatrixGraph`           |
| Rich domain records             | `HashMap<ID, NodeIndex>` + `add_node` + `try_update_edge` | `Graph` / `StableGraph`                                 |
| Large sorted sparse edge list   | sort/dedup + `Csr::from_sorted_edges`                     | `Csr`                                                   |
| Large unsorted sparse edge list | interner + `Graph`, or sort/dedup + `Csr`                 | `Graph` / `Csr`                                         |
| Adjacency list                  | flatten to edges or direct add loop                       | `Graph` / `Csr`                                         |
| Dense matrix                    | scan cells + `MatrixGraph::with_capacity`                 | `MatrixGraph`                                           |
| DOT                             | DOT parser/domain records + validate                      | `Graph` / `StableGraph`                                 |
| graph6                          | `FromGraph6` / `from_graph6_representation`               | undirected `Graph` / `StableGraph` / `Csr` / `GraphMap` |
| serde raw graph                 | `serde_json` / `bincode` with `serde-1`                   | same graph type                                         |
| algorithm output                | `FromElements::from_elements`                             | `Graph` / `StableGraph` / `GraphMap`                    |

---

## 12.20 Anti-pattern inventory

```text id="qow6ht"
Anti-pattern:
    Use from_edges for string IDs.
Problem:
    from_edges expects compact endpoint positions / node defaults.
Fix:
    intern IDs with HashMap.

Anti-pattern:
    Use Graph::add_edge for simple graph imports.
Problem:
    parallel duplicate edges accumulate.
Fix:
    use update_edge / try_update_edge, GraphMap, MatrixGraph, or Csr dedup.

Anti-pattern:
    Load Csr from unsorted edge stream.
Problem:
    from_sorted_edges requires sorted unique pairs.
Fix:
    sort_by_key((u,v)) + dedup/aggregate first.

Anti-pattern:
    Use raw petgraph serde as long-term public file format.
Problem:
    graph-local indices and crate representation become protocol details.
Fix:
    use domain node/edge schema.

Anti-pattern:
    Build MatrixGraph from unbounded sparse records.
Problem:
    O(V²) storage and reallocations.
Fix:
    Graph/Csr.

Anti-pattern:
    Ignore Ix limits on u8/u16 graphs.
Problem:
    add_node/add_edge can panic or try_* returns errors.
Fix:
    prevalidate counts and use try_*.

Anti-pattern:
    DOT as authoritative production import schema.
Problem:
    visualization format, attributes need mapping, exact Dot output may change.
Fix:
    parse DOT into validated domain records or use explicit schema.

Anti-pattern:
    Graph + side map + deletion without remap.
Problem:
    Graph removals may shift indices.
Fix:
    StableGraph or rebuild side maps.
```

---

## 12.21 Production deployment checklist

```text id="o8cpvj"
Before loading:
    choose graph representation
    choose directedness
    choose duplicate-edge policy
    choose external-ID policy
    choose Ix type
    estimate nodes/edges
    define error type

For domain records:
    validate IDs
    deduplicate nodes
    add all nodes first when strict endpoint validation required
    add/update edges second
    preserve external IDs in payload or side map

For large imports:
    preallocate
    use batch sort/dedup where possible
    prefer Csr for static sparse traversal
    avoid MatrixGraph unless dense and bounded
    use try_* at service boundaries

For algorithm outputs:
    use FromElements if output is Element stream
    preserve/remap indices intentionally

For persistence:
    serde-1 for internal/cache use
    explicit domain schema for long-term/public files
    graph6 for undirected topology-only exchange
    DOT for visualization/debug or carefully validated import
```

Final rule:

```text id="r6mrdz"
Construction is where graph invariants become real:
    ID mapping
    duplicate policy
    edge direction
    node payload ownership
    index stability
    capacity limits
    serialization contract

Make those explicit in the builder API, not implicit in scattered add_edge calls.
```

[1]: https://docs.rs/petgraph/?utm_source=chatgpt.com "petgraph - Rust"
[2]: https://docs.rs/petgraph/latest/petgraph/graph/struct.Graph.html?utm_source=chatgpt.com "Graph in petgraph::graph - Rust"
[3]: https://docs.rs/petgraph/latest/petgraph/graph/type.UnGraph.html?utm_source=chatgpt.com "UnGraph in petgraph::graph - Rust"
[4]: https://docs.rs/petgraph/latest/petgraph/data/enum.Element.html "Element in petgraph::data - Rust"
[5]: https://docs.rs/petgraph/latest/petgraph/data/trait.ElementIterator.html "ElementIterator in petgraph::data - Rust"
[6]: https://docs.rs/petgraph/latest/petgraph/algo/min_spanning_tree/fn.min_spanning_tree.html?utm_source=chatgpt.com "min_spanning_tree in petgraph::algo"
[7]: https://docs.rs/petgraph/latest/petgraph/graph6/index.html?utm_source=chatgpt.com "petgraph::graph6 - Rust"
[8]: https://docs.rs/petgraph/latest/petgraph/dot/struct.Dot.html?utm_source=chatgpt.com "Dot in petgraph::dot - Rust"
[9]: https://docs.rs/crate/petgraph/latest/features "petgraph 0.8.3 - Docs.rs"
[10]: https://docs.rs/petgraph/latest/petgraph/graph6/trait.FromGraph6.html "FromGraph6 in petgraph::graph6 - Rust"
[11]: https://docs.rs/petgraph/latest/petgraph/graph6/fn.from_graph6_representation.html "from_graph6_representation in petgraph::graph6 - Rust"
[12]: https://docs.rs/petgraph/latest/petgraph/graph6/fn.get_graph6_representation.html?utm_source=chatgpt.com "get_graph6_representation in petgraph::graph6 - Rust"
[13]: https://docs.rs/petgraph/latest/petgraph/graph/index.html?utm_source=chatgpt.com "petgraph::graph - Rust"
[14]: https://docs.rs/petgraph/latest/petgraph/stable_graph/struct.StableGraph.html?utm_source=chatgpt.com "StableGraph in petgraph::stable_graph - Rust"

# 13) Traversal system — visitors, walkers, and graph traits

Format follows the uploaded advanced-reference style. 

`petgraph::visit` is the traversal abstraction layer: concrete traversal state machines (`Dfs`, `Bfs`, `DfsPostOrder`, `Topo`), event/callback traversal (`depth_first_search` + `DfsEvent`), graph-view adaptors (`Reversed`, `NodeFiltered`, `EdgeFiltered`, `UndirectedAdaptor`), and traits that let algorithms target graph capabilities rather than one concrete graph type. The module index lists these traversal structs, filter/reversal adaptors, and core traits such as `GraphBase`, `IntoNeighbors`, `IntoNeighborsDirected`, `IntoEdges`, `Visitable`, `NodeIndexable`, and `NodeCompactIndexable`. ([Docs.rs][1])

---

## 13.0 Core imports

```rust id="slz1tx"
use petgraph::visit::{
    // Traversal state machines
    Dfs,
    Bfs,
    DfsPostOrder,
    Topo,

    // Callback DFS
    depth_first_search,
    DfsEvent,
    Control,

    // Walker abstraction
    Walker,
    WalkerIter,

    // Core graph traits
    GraphBase,
    GraphRef,
    IntoNeighbors,
    IntoNeighborsDirected,
    IntoEdges,
    IntoEdgesDirected,
    IntoNodeIdentifiers,
    Visitable,

    // Edge/node reference traits
    EdgeRef,
    NodeRef,

    // Filtering/adaptors
    NodeFiltered,
    EdgeFiltered,
    Reversed,
    UndirectedAdaptor,
};

use petgraph::Direction::{Incoming, Outgoing};
use petgraph::graph::{DiGraph, NodeIndex};
```

---

## 13.1 Visit module mental model

```text id="w989vu"
visit module =
    traversal state objects
    + callback traversal
    + graph capability traits
    + graph adaptors/views
    + visitor maps / indexable abstractions
```

Primary architecture:

```text id="2yhin6"
Traversal state:
    Dfs
    Bfs
    DfsPostOrder
    Topo

Callback traversal:
    depth_first_search
    DfsEvent
    Control / ControlFlow

Generic graph capability:
    GraphBase
    IntoNeighbors
    IntoNeighborsDirected
    IntoEdges
    Visitable

Borrow-decoupled stepping:
    Walker
    WalkerIter
    .next(&graph)
    .walk_next(context)

View/adaptor traversal:
    Reversed
    NodeFiltered
    EdgeFiltered
    UndirectedAdaptor
```

Agent rule:

```text id="edlxnq"
Use concrete traversal structs when:
    you need iterative traversal state
    you need to interleave traversal with payload mutation
    you need pause/resume/reset/move_to behavior

Use callback DFS when:
    you need DFS events
    you need edge classification
    you need early break/prune semantics

Use graph traits when:
    writing reusable algorithms over Graph / StableGraph / GraphMap / Csr / MatrixGraph / adaptors
```

---

## 13.2 Minimal graph-trait stack

### `GraphBase`

`GraphBase` defines graph-local node and edge identifier types; `NodeId` and `EdgeId` must be `Copy + PartialEq`. ([Docs.rs][2])

```rust id="677l8w"
use petgraph::visit::GraphBase;

fn ids_are_copy<G: GraphBase>(node: G::NodeId, edge: G::EdgeId) {
    let _node2 = node;
    let _edge2 = edge;
}
```

Mental model:

```text id="k0vb0b"
GraphBase:
    NodeId type
    EdgeId type
    no traversal by itself
    foundation trait for graph algorithms
```

### `IntoNeighbors`

`IntoNeighbors` provides `neighbors(self, a) -> Iterator<Item = NodeId>`; for directed graphs, neighbors are targets of outgoing edges from `a`, while for undirected graphs they are other endpoints connected to `a`. ([Docs.rs][3])

```rust id="7rvsrj"
use petgraph::visit::IntoNeighbors;

fn out_degree_like<G>(graph: G, a: G::NodeId) -> usize
where
    G: IntoNeighbors,
{
    graph.neighbors(a).count()
}
```

### `Visitable`

`Visitable` creates and resets a visitor map for node-discovery tracking; it requires `GraphBase` and has associated type `Map: VisitMap<NodeId>`. ([Docs.rs][4])

```rust id="ts9e3m"
use petgraph::visit::Visitable;

fn new_visit_map<G: Visitable>(g: &G) -> G::Map {
    g.visit_map()
}
```

### Minimal traversal-compatible bound

```rust id="lt5hk2"
use petgraph::visit::{IntoNeighbors, Visitable};

fn requires_basic_traversal<G>(_g: G)
where
    G: IntoNeighbors + Visitable,
{
}
```

Rule:

```text id="sxy7dj"
Minimum for DFS/BFS-style custom traversal:
    GraphBase    => NodeId / EdgeId
    IntoNeighbors => adjacency expansion
    Visitable    => visited/discovered map

For direction-aware traversal:
    add IntoNeighborsDirected

For edge-weight-aware traversal:
    add IntoEdges / IntoEdgesDirected + EdgeRef
```

---

## 13.3 `Dfs`: preorder depth-first traversal

`Dfs<N, VM>` stores a `Vec<N>` stack and a discovered map; it emits nodes in DFS preorder, starts from one node, traverses reachable nodes only, is non-recursive, and does not itself borrow the graph, allowing mutable graph access between `next(&graph)` calls. The docs explicitly warn traversal may not behave correctly if nodes are removed during iteration and may not necessarily visit nodes/edges added during iteration. ([Docs.rs][5])

### Basic syntax

```rust id="nli3vl"
use petgraph::graph::DiGraph;
use petgraph::visit::Dfs;

let mut g = DiGraph::<&str, ()>::new();

let a = g.add_node("a");
let b = g.add_node("b");
let c = g.add_node("c");

g.add_edge(a, b, ());
g.add_edge(b, c, ());

let mut dfs = Dfs::new(&g, a);

while let Some(nx) = dfs.next(&g) {
    println!("visit {nx:?}: {}", g[nx]);
}
```

### Payload mutation during DFS

```rust id="mg3l25"
use petgraph::graph::DiGraph;
use petgraph::visit::Dfs;

let mut g = DiGraph::<u32, ()>::new();

let a = g.add_node(0);
let b = g.add_node(0);

g.add_edge(a, b, ());

let mut dfs = Dfs::new(&g, a);

while let Some(nx) = dfs.next(&g) {
    g[nx] += 1;
}

assert_eq!(g[a], 1);
assert_eq!(g[b], 1);
```

### `Dfs` control methods

```rust id="q7x9y4"
let mut dfs = Dfs::empty(&g); // visitor map, empty stack
dfs.move_to(a);               // keep discovered map; restart stack from a

while let Some(nx) = dfs.next(&g) {
    println!("{nx:?}");
}

dfs.reset(&g);                // clear visit state
dfs.move_to(b);
```

`Dfs::new` uses the graph’s visitor map and seeds the stack; `empty` creates a traversal with no stack; `move_to` keeps the discovered map but clears the visit stack and restarts from a node; `reset` clears visit state; `next` requires `IntoNeighbors<NodeId = N>` and returns the next node or `None`. ([Docs.rs][5])

Use `Dfs` when:

```text id="9rxqkn"
preorder discovery order required
recursive DFS would risk stack overflow
you need mutable access between traversal steps
you need pause/resume traversal state
you only need nodes, not edge classifications
```

---

## 13.4 `Bfs`: breadth-first traversal

`Bfs<N, VM>` stores a `VecDeque<N>` queue and a discovered map; traversal starts at one node, visits reachable nodes, is non-recursive, and does not itself borrow the graph, enabling mutable graph access between `next(&graph)` calls. It has the same mutation caveat as `Dfs`: removing nodes during iteration can break correctness, and newly added nodes/edges may not be visited. ([Docs.rs][6])

### Basic syntax

```rust id="wwvwb7"
use petgraph::graph::DiGraph;
use petgraph::visit::Bfs;

let mut g = DiGraph::<&str, ()>::new();

let a = g.add_node("a");
let b = g.add_node("b");
let c = g.add_node("c");

g.add_edge(a, b, ());
g.add_edge(a, c, ());

let mut bfs = Bfs::new(&g, a);

while let Some(nx) = bfs.next(&g) {
    println!("visit {nx:?}: {}", g[nx]);
}
```

### BFS payload mutation

```rust id="iwmxv0"
use petgraph::visit::Bfs;

let mut bfs = Bfs::new(&g, a);

while let Some(nx) = bfs.next(&g) {
    if let Some(weight) = g.node_weight_mut(nx) {
        // safe payload mutation
        *weight = "visited";
    }
}
```

`Bfs::new` requires `GraphRef + Visitable`; `Bfs::next` requires `IntoNeighbors<NodeId = N>` and returns the next node or `None`. ([Docs.rs][6])

Use `Bfs` when:

```text id="khf8f3"
shortest unweighted hop expansion
level-order traversal
flood fill / reachability by increasing distance
frontier-style exploration
```

---

## 13.5 `DfsPostOrder`: postorder DFS

`DfsPostOrder<N, VM>` stores a stack, discovered map, and finished map; it is a non-recursive DFS that emits nodes in postorder, meaning each node is emitted after all descendants have been emitted. Traversal starts at a given node and only visits reachable nodes. ([Docs.rs][7])

### Basic syntax

```rust id="1mnh8b"
use petgraph::graph::DiGraph;
use petgraph::visit::DfsPostOrder;

let mut g = DiGraph::<&str, ()>::new();

let a = g.add_node("a");
let b = g.add_node("b");
let c = g.add_node("c");

g.add_edge(a, b, ());
g.add_edge(b, c, ());

let mut dfs_po = DfsPostOrder::new(&g, a);

while let Some(nx) = dfs_po.next(&g) {
    println!("postorder {nx:?}: {}", g[nx]);
}
```

Use `DfsPostOrder` when:

```text id="2a8lju"
children-before-parent processing
dependency cleanup
reverse topological-ish traversal over reachable region
postorder forest logic
cycle-tolerant DFS order where Topo is inappropriate
```

Agent rule:

```text id="fybv9c"
Dfs:
    emits on discovery

DfsPostOrder:
    emits after descendants

Topo:
    emits topological order for DAG-like reachable nodes / graph
```

---

## 13.6 `Topo`: topological traversal

`Topo<N, VM>` performs a topological order traversal. It only visits nodes that are not part of cycles, so for graphs with possible cycles the docs recommend `DfsPostOrder` or SCC algorithms such as `kosaraju_scc`; `Topo::new` requires `IntoNodeIdentifiers + IntoNeighborsDirected + Visitable`, while `Topo::next` requires `IntoNeighborsDirected + Visitable`. The docs also state the only way to know whether the graph had a complete topological order is to run the full traversal and ensure every node was visited. ([Docs.rs][8])

### Basic syntax

```rust id="clp40p"
use petgraph::graph::DiGraph;
use petgraph::visit::Topo;

let mut g = DiGraph::<&str, ()>::new();

let compile = g.add_node("compile");
let test = g.add_node("test");
let deploy = g.add_node("deploy");

g.add_edge(compile, test, ());
g.add_edge(test, deploy, ());

let mut topo = Topo::new(&g);

while let Some(nx) = topo.next(&g) {
    println!("topo {}", g[nx]);
}
```

### Detect incomplete traversal

```rust id="ck84zw"
use petgraph::visit::Topo;

let mut topo = Topo::new(&g);
let mut visited = 0usize;

while let Some(_nx) = topo.next(&g) {
    visited += 1;
}

if visited != g.node_count() {
    // cycle or graph structure prevents complete topological traversal
}
```

### Seeded topological traversal

```rust id="h7zuqy"
let mut topo = Topo::with_initials(&g, [compile]);

while let Some(nx) = topo.next(&g) {
    println!("{}", g[nx]);
}
```

`Topo::with_initials` creates a traversal from supplied initial nodes and ignores nodes with incoming edges. ([Docs.rs][8])

Use `Topo` when:

```text id="p60lfg"
DAG scheduling
build order
dependency order
pipeline execution
topological layer processing
```

Avoid `Topo` when:

```text id="havjcv"
cycles are expected and must be reported precisely
you need edge classification
you need complete traversal on cyclic graph
```

---

## 13.7 `depth_first_search`: callback/event API

`depth_first_search(graph, starts, visitor)` is a recursive DFS helper. It takes a graph implementing `IntoNeighbors + Visitable`, a start-node iterator, and a visitor callback `FnMut(DfsEvent<NodeId>) -> C`, where `C: ControlFlow`. It emits discovery/finish events for reachable nodes and edge-classification events for reachable edges; `Control::Continue`, `Control::Break`, and `Control::Prune` can control traversal, and pruning during `Finish` panics. ([Docs.rs][9])

### Event variants

```rust id="5lqzae"
pub enum DfsEvent<N> {
    Discover(N, Time),
    TreeEdge(N, N),
    BackEdge(N, N),
    CrossForwardEdge(N, N),
    Finish(N, Time),
}
```

`DfsEvent` includes discovery, tree edge, back edge, cross/forward edge, and finish events; `Finish` means all edges from a node have been reported. ([Docs.rs][10])

### Basic event logging

```rust id="rkj9rs"
use petgraph::graph::DiGraph;
use petgraph::visit::{depth_first_search, DfsEvent};

let mut g = DiGraph::<&str, ()>::new();

let a = g.add_node("a");
let b = g.add_node("b");

g.add_edge(a, b, ());

depth_first_search(&g, Some(a), |event| {
    match event {
        DfsEvent::Discover(n, t) => {
            println!("discover {:?} at {:?}", n, t);
        }
        DfsEvent::TreeEdge(u, v) => {
            println!("tree edge {:?}->{:?}", u, v);
        }
        DfsEvent::BackEdge(u, v) => {
            println!("back edge {:?}->{:?}", u, v);
        }
        DfsEvent::CrossForwardEdge(u, v) => {
            println!("cross/forward edge {:?}->{:?}", u, v);
        }
        DfsEvent::Finish(n, t) => {
            println!("finish {:?} at {:?}", n, t);
        }
    }
});
```

### Early break on goal

```rust id="4w0fj5"
use petgraph::visit::{depth_first_search, Control, DfsEvent};

let goal = b;

let found = depth_first_search(&g, Some(a), |event| {
    if let DfsEvent::Discover(n, _) = event {
        if n == goal {
            return Control::Break(n);
        }
    }

    Control::Continue
});

assert_eq!(found.break_value(), Some(goal));
```

### Prune current node

```rust id="g49gw4"
use petgraph::visit::{depth_first_search, Control, DfsEvent};

depth_first_search(&g, Some(a), |event| {
    match event {
        DfsEvent::Discover(n, _) if should_skip_subtree(n) => Control::Prune,
        _ => Control::Continue,
    }
});
```

Agent rule:

```text id="uzvykh"
Use depth_first_search when:
    you need tree/back/cross-forward edge classification
    you need discover/finish timestamps
    you need callback-controlled pruning/breaking
    event stream is more useful than simple node stream

Use Dfs when:
    node preorder alone is enough
    you need explicit mutable graph access between steps
```

---

## 13.8 `Walker` and `WalkerIter`

`Walker<Context>` is a traversal-state trait where part of the traversal information is supplied manually to each `walk_next(context)` call; docs state this enables traversals that do not hold a borrow of the graph. It has an associated `Item`, required `walk_next`, and provided `iter(context)` method that creates `WalkerIter` when `Context: Clone`. ([Docs.rs][11])

```rust id="vnrxmz"
pub trait Walker<Context> {
    type Item;

    fn walk_next(&mut self, context: Context) -> Option<Self::Item>;

    fn iter(self, context: Context) -> WalkerIter<Self, Context>
    where
        Self: Sized,
        Context: Clone;
}
```

`WalkerIter<W, C>` wraps a walker and context into an `Iterator`; it exposes `context`, `inner_ref`, and `inner_mut`. ([Docs.rs][12])

### Iterator wrapping

```rust id="uu3w7s"
use petgraph::visit::{Dfs, Walker};

let dfs = Dfs::new(&g, a);

for nx in dfs.iter(&g) {
    println!("{:?}", nx);
}
```

### Direct walker stepping

```rust id="t4otih"
use petgraph::visit::{Dfs, Walker};

let mut dfs = Dfs::new(&g, a);

while let Some(nx) = dfs.walk_next(&g) {
    println!("{:?}", nx);
}
```

Use direct `.next(&graph)` / `.walk_next(&graph)` when:

```text id="vuobpx"
you need mutation between steps
you need explicit control loop
you need to inspect/update traversal state
```

Use `.iter(&graph)` when:

```text id="x852me"
read-only iterator pipeline is enough
context clone is cheap
standard Iterator adapters are convenient
```

---

## 13.9 Why walkers do not borrow the graph for the whole traversal

Petgraph traversal structs store traversal state separately from graph data:

```text id="mvoozl"
Dfs:
    stack
    discovered map

Bfs:
    queue
    discovered map

DfsPostOrder:
    stack
    discovered map
    finished map

Topo:
    topological traversal state
```

`Dfs` and `Bfs` docs explicitly say the traversal object does not itself borrow the graph, which allows retaining mutable graph access during traversal; they also warn that removing nodes can break traversal correctness and added nodes/edges may not be visited. ([Docs.rs][5])

Safe payload mutation pattern:

```rust id="ucofi1"
use petgraph::visit::Dfs;

let mut dfs = Dfs::new(&g, start);

while let Some(nx) = dfs.next(&g) {
    if let Some(weight) = g.node_weight_mut(nx) {
        weight.visited = true;
    }
}
```

Unsafe topology-mutation pattern:

```rust id="slqzax"
let mut dfs = Dfs::new(&g, start);

while let Some(nx) = dfs.next(&g) {
    // Risky for Graph:
    // node removals can invalidate traversal state.
    // g.remove_node(nx);
}
```

Correct topology mutation pattern:

```rust id="4986b0"
let mut dfs = Dfs::new(&g, start);
let mut remove_later = Vec::new();

while let Some(nx) = dfs.next(&g) {
    if should_remove(nx) {
        remove_later.push(nx);
    }
}

for nx in remove_later {
    g.remove_node(nx);
}
```

Agent rule:

```text id="zc0p10"
Walkers avoid borrow conflicts.
Walkers do not eliminate graph-mutation semantic hazards.

Safe:
    mutate node/edge weights
    collect IDs for later mutation
    inspect graph state per step

Risky:
    remove nodes during traversal
    rely on newly added edges/nodes being visited
    mutate topology of Graph while traversal state contains NodeIndex values
```

---

## 13.10 Detached neighbor walkers

Graph neighbor iterators expose `.detach()` to create a walker that does not borrow the graph; the `Neighbors` docs state detached walkers are used to step through a node’s edge list and do not borrow from the graph, allowing edge walking mixed with mutating graph weights. ([Docs.rs][13])

```rust id="ojc45o"
use petgraph::Direction::Incoming;

let mut incoming = g.neighbors_directed(node, Incoming).detach();

while let Some((edge, neighbor)) = incoming.next(&g) {
    if let Some(edge_weight) = g.edge_weight_mut(edge) {
        *edge_weight += 1;
    }

    if let Some(node_weight) = g.node_weight_mut(neighbor) {
        node_weight.marked = true;
    }
}
```

Use detached neighbor walkers when:

```text id="vabjvh"
iterating local adjacency
mutating node/edge weights during adjacency scan
avoiding borrow checker conflicts
processing edge IDs plus neighbor IDs
```

Avoid topology removal inside detached-walker loop unless carefully validated:

```text id="ua6zf6"
Graph:
    removing edges/nodes can disturb pending edge-list traversal

StableGraph:
    unrelated handles survive, but removed handles still invalid
```

---

## 13.11 Direction-aware traversal

### Directed neighbor expansion

```rust id="gd8y4x"
use petgraph::visit::IntoNeighborsDirected;
use petgraph::Direction::{Incoming, Outgoing};

fn count_dir<G>(g: G, n: G::NodeId, dir: petgraph::Direction) -> usize
where
    G: IntoNeighborsDirected,
{
    g.neighbors_directed(n, dir).count()
}
```

`IntoNeighborsDirected` extends `IntoNeighbors` and provides `neighbors_directed(self, node, Direction)`; directed/outgoing returns targets of edges from the node, directed/incoming returns sources of edges to the node, and undirected graphs return other endpoints of connected edges. ([Docs.rs][14])

### Reverse reachability with `Incoming`

```rust id="pyuo5d"
use petgraph::Direction::Incoming;
use petgraph::graph::DiGraph;

fn immediate_dependents<N, E>(
    g: &DiGraph<N, E>,
    dependency: NodeIndex,
) -> Vec<NodeIndex> {
    g.neighbors_directed(dependency, Incoming).collect()
}
```

### Direction-parameterized DFS-like traversal

```rust id="v2ofj7"
use petgraph::visit::{IntoNeighborsDirected, Visitable};
use petgraph::Direction;

fn directional_reachable<G>(
    graph: G,
    start: G::NodeId,
    dir: Direction,
) -> Vec<G::NodeId>
where
    G: IntoNeighborsDirected + Visitable,
    G::NodeId: Copy + PartialEq,
{
    let mut seen = graph.visit_map();
    let mut stack = vec![start];
    let mut out = Vec::new();

    while let Some(n) = stack.pop() {
        if seen.visit(n) {
            out.push(n);
            for m in graph.neighbors_directed(n, dir) {
                stack.push(m);
            }
        }
    }

    out
}
```

Note: the example uses `VisitMap::visit`, so production code must import `petgraph::visit::VisitMap`.

Agent rule:

```text id="isy7ix"
For directed graphs:
    Outgoing traversal = successors
    Incoming traversal = predecessors

For undirected graphs:
    direction parameter does not change semantic neighbor set

For reverse queries:
    prefer Incoming traversal or Reversed adaptor over mutating graph with reverse()
```

---

## 13.12 Filtered traversal with `NodeFiltered`

`NodeFiltered<G, F>` is a node-filtering graph adaptor; it stores the graph and a node predicate closure. The constructor `NodeFiltered::from_fn(graph, filter)` accepts `F: Fn(G::NodeId) -> bool`, and the adaptor implements many graph traits including `GraphBase`, `IntoNeighbors`, `IntoNeighborsDirected`, `IntoNodeIdentifiers`, `NodeIndexable`, and `Visitable`, making it usable by traversal APIs. ([Docs.rs][15])

```rust id="6l6npg"
use petgraph::visit::{Dfs, NodeFiltered};

let active = |n: NodeIndex| {
    g.node_weight(n)
        .map(|node| node.active)
        .unwrap_or(false)
};

let view = NodeFiltered::from_fn(&g, active);

let mut dfs = Dfs::new(&view, start);

while let Some(nx) = dfs.next(&view) {
    println!("active reachable: {:?}", nx);
}
```

Use `NodeFiltered` when:

```text id="pdi2xf"
nodes have active/deleted/visible flags
algorithm should ignore inactive nodes
you want view semantics without materializing subgraph
node predicate depends only on NodeId / graph access captured by closure
```

Caveat:

```text id="m5cgjy"
NodeFiltered filters node visibility.
Ensure start node passes predicate, or traversal may produce no useful work.
For edge predicates, use EdgeFiltered.
For expensive predicates, precompute flags/sets.
```

---

## 13.13 Filtered traversal with `EdgeFiltered`

`EdgeFiltered<G, F>` is an edge-filtering graph adaptor; the filter implements `FilterEdge`, and closures `Fn(G::EdgeRef) -> bool` already implement that trait. The filter can use edge source, target, ID, and weight to decide inclusion. The adaptor implements traversal traits such as `IntoNeighbors`, `IntoNeighborsDirected`, `IntoEdges`, `IntoEdgesDirected`, `IntoEdgeReferences`, `IntoNodeIdentifiers`, and `Visitable`. ([Docs.rs][16])

```rust id="t75ah4"
use petgraph::visit::{Dfs, EdgeFiltered, EdgeRef};

let view = EdgeFiltered::from_fn(&g, |edge| {
    edge.weight().enabled && edge.weight().latency_ms <= 10
});

let mut dfs = Dfs::new(&view, start);

while let Some(nx) = dfs.next(&view) {
    println!("reachable through enabled low-latency edges: {:?}", nx);
}
```

Use `EdgeFiltered` when:

```text id="bofv6h"
edge weights encode enabled/disabled state
cost threshold filters traversal
relation type determines traversability
temporary policy view is needed
materializing filtered graph is too expensive
```

Caveat:

```text id="6d4anp"
EdgeFiltered filters edges at traversal/view layer.
It does not remove edges from original graph.
If multiple algorithms reuse same filter, name the adaptor or helper function.
```

---

## 13.14 Reversed and undirected views

The visit module includes `Reversed`, which reverses edge orientation for traversal, and `UndirectedAdaptor`, which removes edge direction for traversal. The module index lists these adaptors and corresponding reversed edge-reference iterators. ([Docs.rs][1])

Reverse-view pattern:

```rust id="7m56s6"
use petgraph::visit::{Dfs, Reversed};

let reversed = Reversed(&g);
let mut dfs = Dfs::new(&reversed, target);

while let Some(nx) = dfs.next(&reversed) {
    println!("can reach target from {:?}", nx);
}
```

Undirected-view pattern:

```rust id="b42jb2"
use petgraph::visit::{Dfs, UndirectedAdaptor};

let undirected = UndirectedAdaptor(&g);
let mut dfs = Dfs::new(&undirected, start);

while let Some(nx) = dfs.next(&undirected) {
    println!("weakly connected {:?}", nx);
}
```

Use views when:

```text id="y3x9ks"
you need temporary traversal semantics
you do not want to clone or mutate topology
algorithm accepts graph traits
```

Use materialization when:

```text id="yxf72z"
algorithm requires concrete Graph
view lifetime/closure complexity becomes noisy
derived graph must be serialized/exported
multiple passes amortize construction cost
```

---

## 13.15 Custom graph compatibility

Minimum custom graph design target:

```text id="90cqac"
GraphBase:
    define NodeId / EdgeId

GraphRef:
    if graph reference is copyable

IntoNeighbors:
    expose neighbor iteration

Visitable:
    create/reset VisitMap

Optional:
    IntoNeighborsDirected
    IntoEdges / IntoEdgesDirected
    IntoNodeIdentifiers
    NodeIndexable / NodeCompactIndexable
```

Skeleton:

```rust id="9jwfc1"
use petgraph::visit::{
    GraphBase,
    IntoNeighbors,
    Visitable,
    VisitMap,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct MyNode(u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct MyEdge(u32);

struct MyGraph {
    adj: Vec<Vec<MyNode>>,
}

// GraphBase: define IDs.
impl GraphBase for MyGraph {
    type NodeId = MyNode;
    type EdgeId = MyEdge;
}

// Prefer implementing traits for &MyGraph in real code if borrowing matters.
// This is illustrative; exact iterator lifetimes often require wrapper types.
```

Practical recommendation:

```text id="z4hrzj"
Before implementing custom graph traits:
    consider wrapping Graph / StableGraph / GraphMap
    consider NodeFiltered / EdgeFiltered / Reversed adaptors
    consider newtype with delegated trait impls

Implement custom graph traits only when:
    topology storage is external/custom
    zero-copy traversal over external format matters
    graph is virtual/generated
    domain graph cannot be materialized efficiently
```

Trait target by algorithm:

```text id="vau77g"
Dfs / Bfs:
    IntoNeighbors + Visitable

depth_first_search:
    IntoNeighbors + Visitable

Topo:
    IntoNodeIdentifiers + IntoNeighborsDirected + Visitable

directional traversal:
    IntoNeighborsDirected

edge-aware traversal:
    IntoEdges / IntoEdgesDirected + EdgeRef

dense array algorithms:
    NodeIndexable or NodeCompactIndexable
```

---

## 13.16 Traversal mutation safety table

| Operation during traversal   |                                  `Dfs` / `Bfs` state safety | Recommended pattern                          |
| ---------------------------- | ----------------------------------------------------------: | -------------------------------------------- |
| Mutate node weight           |                                                        Good | `node_weight_mut(nx)` inside loop            |
| Mutate edge weight           |                                   Good if edge handle known | detached neighbor walker or collect edge IDs |
| Add node                     |                                  Traversal may not visit it | add after traversal or restart               |
| Add edge                     |                                  Traversal may not visit it | add after traversal or restart               |
| Remove edge                  |                            Risky if walker state touches it | collect then remove                          |
| Remove node in `Graph`       |                                      Dangerous; index shift | collect then remove or use `StableGraph`     |
| Remove node in `StableGraph` | Safer for unrelated handles, still invalidates removed node | collect/validate handles                     |
| Filter traversal             |                                                        Good | `NodeFiltered` / `EdgeFiltered` view         |

---

## 13.17 Traversal selection table

| Need                             | Use                                            | Why                                        |
| -------------------------------- | ---------------------------------------------- | ------------------------------------------ |
| Reachable nodes, preorder        | `Dfs`                                          | non-recursive preorder DFS                 |
| Reachable nodes, level order     | `Bfs`                                          | queue/frontier traversal                   |
| Children before parents          | `DfsPostOrder`                                 | DFS postorder                              |
| DAG execution/build order        | `Topo`                                         | topological traversal                      |
| Discover/finish times            | `depth_first_search`                           | emits timed events                         |
| Edge classification              | `depth_first_search`                           | tree/back/cross-forward events             |
| Stop when goal found             | `depth_first_search` + `Control::Break`        | callback control                           |
| Skip subtree                     | `depth_first_search` + `Control::Prune`        | prune edges from current node              |
| Mutate payloads during traversal | `Dfs` / `Bfs` / detached walkers               | traversal state does not hold graph borrow |
| Direction-specific reachability  | `IntoNeighborsDirected` / `neighbors_directed` | incoming/outgoing                          |
| Filter by active nodes           | `NodeFiltered`                                 | graph view                                 |
| Filter by edge weights           | `EdgeFiltered`                                 | graph view                                 |
| Reverse traversal                | `Incoming` or `Reversed`                       | no topology mutation                       |

---

## 13.18 Anti-pattern inventory

```text id="spx6t0"
Anti-pattern:
    Remove Graph nodes during Dfs/Bfs traversal.
Problem:
    traversal state stores NodeIndex values; Graph removals may shift indices.
Fix:
    collect nodes, remove after traversal; or use StableGraph and validate.

Anti-pattern:
    Expect Dfs/Bfs to visit nodes/edges added during traversal.
Problem:
    docs warn added nodes/edges may not be visited.
Fix:
    restart traversal or use explicit work queue policy.

Anti-pattern:
    Use Topo on cyclic graph and assume full traversal.
Problem:
    Topo only visits nodes not part of cycles; full-order detection requires counting visits.
Fix:
    use toposort for error reporting or SCC/DfsPostOrder for cyclic graphs.

Anti-pattern:
    Materialize filtered graph for one traversal.
Problem:
    avoidable allocation/cloning.
Fix:
    NodeFiltered / EdgeFiltered.

Anti-pattern:
    Use IntoNeighbors when edge weight filter is required.
Problem:
    neighbor iterator loses edge-ref context.
Fix:
    use IntoEdges / EdgeFiltered.

Anti-pattern:
    Treat WalkerIter as mutation-friendly.
Problem:
    iterator context borrow/clone may be less convenient than explicit next(&graph).
Fix:
    use direct while-let loop for mutation-heavy traversal.

Anti-pattern:
    Implement custom graph traits before trying adaptors.
Problem:
    unnecessary trait complexity.
Fix:
    use Reversed / NodeFiltered / EdgeFiltered / wrapper around Graph.
```

---

## 13.19 Deployment checklist

```text id="ms2qz5"
For application traversal:
    Dfs for preorder DFS
    Bfs for breadth-first levels
    DfsPostOrder for descendant-first processing
    Topo for DAG traversal
    depth_first_search for events/classification/control

For safe mutation:
    mutate weights during traversal
    do not remove Graph nodes during traversal
    collect topology changes and apply after traversal
    use detached walkers for local adjacency + payload mutation

For generic libraries:
    require the narrowest trait set
    IntoNeighbors + Visitable for basic traversal
    IntoNeighborsDirected for direction control
    IntoEdges for edge-weight-aware algorithms
    NodeIndexable only when index maps are needed
    NodeCompactIndexable only when dense no-hole IDs are required

For filtered/reversed traversal:
    prefer adaptors over cloned/materialized graphs
    document filter predicates
    precompute expensive filter state
    use EdgeFiltered when predicate depends on edge weight/source/target/id
```

Final rule:

```text id="p1z6rr"
Traversal state is separate from graph storage.
That design gives borrow flexibility.
It does not give unrestricted topology-mutation safety.
Use the narrowest traversal API that matches the algorithm contract.
```

[1]: https://docs.rs/petgraph/latest/petgraph/visit/index.html "petgraph::visit - Rust"
[2]: https://docs.rs/petgraph/latest/petgraph/visit/trait.GraphBase.html "GraphBase in petgraph::visit - Rust"
[3]: https://docs.rs/petgraph/latest/petgraph/visit/trait.IntoNeighbors.html "IntoNeighbors in petgraph::visit - Rust"
[4]: https://docs.rs/petgraph/latest/petgraph/visit/trait.Visitable.html "Visitable in petgraph::visit - Rust"
[5]: https://docs.rs/petgraph/latest/petgraph/visit/struct.Dfs.html "Dfs in petgraph::visit - Rust"
[6]: https://docs.rs/petgraph/latest/petgraph/visit/struct.Bfs.html "Bfs in petgraph::visit - Rust"
[7]: https://docs.rs/petgraph/latest/petgraph/visit/struct.DfsPostOrder.html "DfsPostOrder in petgraph::visit - Rust"
[8]: https://docs.rs/petgraph/latest/petgraph/visit/struct.Topo.html "Topo in petgraph::visit - Rust"
[9]: https://docs.rs/petgraph/latest/petgraph/visit/fn.depth_first_search.html "depth_first_search in petgraph::visit - Rust"
[10]: https://docs.rs/petgraph/latest/petgraph/visit/enum.DfsEvent.html "DfsEvent in petgraph::visit - Rust"
[11]: https://docs.rs/petgraph/latest/petgraph/visit/trait.Walker.html "Walker in petgraph::visit - Rust"
[12]: https://docs.rs/petgraph/latest/petgraph/visit/struct.WalkerIter.html "WalkerIter in petgraph::visit - Rust"
[13]: https://docs.rs/petgraph/latest/petgraph/graph/struct.Neighbors.html?utm_source=chatgpt.com "Neighbors in petgraph::graph - Rust"
[14]: https://docs.rs/nopetgraph/latest/petgraph/visit/trait.IntoNeighborsDirected.html "IntoNeighborsDirected in petgraph::visit - Rust"
[15]: https://docs.rs/petgraph/latest/petgraph/visit/struct.NodeFiltered.html "NodeFiltered in petgraph::visit - Rust"
[16]: https://docs.rs/petgraph/latest/petgraph/visit/struct.EdgeFiltered.html "EdgeFiltered in petgraph::visit - Rust"


# 14) Trait-based graph abstraction — petgraph

Format follows the uploaded advanced-reference style. 

Petgraph’s trait layer is centered in `petgraph::visit`: graph traits define **capabilities** such as node identity, directedness, neighbor access, edge-reference access, node/edge counts, weight access, visit-map construction, and indexability. The module docs state that `Into*` traits produce iterators from shared graph references, `Dfs` / `Bfs` / `DfsPostOrder` / `Topo` use walker methods that only borrow the graph for each `.next()` call, and the minimum trait set for visitors is `GraphBase + IntoNeighbors + Visitable`. ([Docs.rs][1])

---

## 14.0 Core imports

```rust
use petgraph::visit::{
    GraphBase,
    GraphProp,
    GraphRef,

    IntoNeighbors,
    IntoNeighborsDirected,
    IntoEdges,
    IntoEdgesDirected,
    IntoEdgeReferences,
    IntoNodeIdentifiers,
    IntoNodeReferences,

    Data,
    NodeCount,
    EdgeCount,
    NodeIndexable,
    NodeCompactIndexable,
    EdgeIndexable,

    Visitable,
    VisitMap,
    EdgeRef,
    NodeRef,
};

use petgraph::data::{
    DataMap,
    DataMapMut,
};

use petgraph::Direction::{Incoming, Outgoing};
use petgraph::graph::{Graph, DiGraph, UnGraph, NodeIndex, EdgeIndex};
use petgraph::stable_graph::{StableGraph, StableDiGraph, StableUnGraph};
use petgraph::graphmap::{DiGraphMap, UnGraphMap};
use petgraph::matrix_graph::{DiMatrix, UnMatrix};
use petgraph::csr::Csr;
```

Trait-group mental model:

```text
identity:
    GraphBase

graph property:
    GraphProp

copyable graph reference:
    GraphRef

adjacency:
    IntoNeighbors
    IntoNeighborsDirected

edge traversal:
    IntoEdges
    IntoEdgesDirected
    IntoEdgeReferences

node traversal:
    IntoNodeIdentifiers
    IntoNodeReferences

weights:
    Data
    DataMap
    DataMapMut

counts:
    NodeCount
    EdgeCount

indexability:
    NodeIndexable
    NodeCompactIndexable
    EdgeIndexable

traversal state:
    Visitable
    VisitMap
```

---

## 14.1 `GraphBase`: node/edge identity foundation

`GraphBase` defines the associated identifier types for a graph: `NodeId` and `EdgeId`, each `Copy + PartialEq`. It is the base trait under most other graph-traversal traits. ([Docs.rs][2])

```rust
pub trait GraphBase {
    type EdgeId: Copy + PartialEq;
    type NodeId: Copy + PartialEq;
}
```

Generic identity-only function:

```rust
use petgraph::visit::GraphBase;

fn same_node<G>(a: G::NodeId, b: G::NodeId) -> bool
where
    G: GraphBase,
{
    a == b
}
```

Identity examples:

```text
Graph<N,E,Ty,Ix>:
    NodeId = NodeIndex<Ix>
    EdgeId = EdgeIndex<Ix>

StableGraph<N,E,Ty,Ix>:
    NodeId = NodeIndex<Ix>
    EdgeId = EdgeIndex<Ix>

GraphMap<N,E,Ty,S>:
    NodeId = N
    EdgeId = (N, N)

MatrixGraph<N,E,...,Ix>:
    NodeId = NodeIndex<Ix>
    EdgeId = (NodeIndex<Ix>, NodeIndex<Ix>)

Csr<N,E,Ty,Ix>:
    NodeId = Ix
    EdgeId = usize
```

The `GraphBase` implementor list in rustdoc shows these associated identifier shapes for `Graph`, `StableGraph`, `GraphMap`, `MatrixGraph`, and `Csr`. ([Docs.rs][2])

Agent rule:

```text
Use GraphBase when:
    algorithm only needs graph-local ID types
    no adjacency access required
    no weights required
    no counts required

Do not assume:
    NodeId == NodeIndex
    EdgeId == EdgeIndex
    graph families share handle semantics
```

---

## 14.2 `GraphProp`: directedness at trait level

`GraphProp` extends `GraphBase`, exposes an associated `EdgeType`, and provides `is_directed()`. Its purpose is to describe whether a graph’s edges are directed or undirected. ([Docs.rs][3])

```rust
pub trait GraphProp: GraphBase {
    type EdgeType: petgraph::EdgeType;

    fn is_directed(&self) -> bool { ... }
}
```

Generic directedness inspection:

```rust
use petgraph::visit::GraphProp;

fn graph_kind<G>(graph: &G) -> &'static str
where
    G: GraphProp,
{
    if graph.is_directed() {
        "directed"
    } else {
        "undirected"
    }
}
```

Adaptor note:

```text
Reversed<G>:
    preserves underlying EdgeType

UndirectedAdaptor<G>:
    exposes EdgeType = Undirected
```

The `GraphProp` implementor list shows `Reversed` preserving the graph’s edge type and `UndirectedAdaptor` exposing `Undirected`. ([Docs.rs][3])

---

## 14.3 `GraphRef`: copyable reference wrapper contract

`GraphRef` is a marker trait: `Copy + GraphBase`. It represents a copyable reference-like graph value. `&G` implements `GraphRef` when `G: GraphBase`, which is why many `Into*` trait methods consume `self` but are normally called on `&graph`. ([Docs.rs][4])

```rust
pub trait GraphRef: Copy + GraphBase { }
```

Pattern:

```rust
use petgraph::visit::IntoNeighbors;

fn count_neighbors<G>(graph: G, node: G::NodeId) -> usize
where
    G: IntoNeighbors,
{
    graph.neighbors(node).count()
}

// Call with &graph:
let count = count_neighbors(&g, start);
```

Agent rule:

```text
Most generic traversal functions should accept graph: G
where G is usually &GraphType.

Do:
    fn f<G>(g: G, n: G::NodeId) where G: IntoNeighbors

Call:
    f(&graph, node)

Avoid:
    requiring owned Graph unless algorithm consumes topology
```

---

## 14.4 `IntoNeighbors`: node adjacency without edge weights

`IntoNeighbors` requires `GraphRef` and exposes `neighbors(self, a) -> Iterator<Item = NodeId>`. For directed graphs, neighbors are targets of outgoing edges from `a`; for undirected graphs, neighbors are the other endpoints connected to `a`. ([Docs.rs][5])

```rust
pub trait IntoNeighbors: GraphRef {
    type Neighbors: Iterator<Item = Self::NodeId>;

    fn neighbors(self, a: Self::NodeId) -> Self::Neighbors;
}
```

Reachability skeleton:

```rust
use petgraph::visit::{IntoNeighbors, Visitable, VisitMap};

fn reachable_nodes<G>(graph: G, start: G::NodeId) -> Vec<G::NodeId>
where
    G: IntoNeighbors + Visitable,
    G::NodeId: Copy,
{
    let mut seen = graph.visit_map();
    let mut stack = vec![start];
    let mut out = Vec::new();

    while let Some(n) = stack.pop() {
        if seen.visit(n) {
            out.push(n);

            for m in graph.neighbors(n) {
                stack.push(m);
            }
        }
    }

    out
}
```

Use when:

```text
edge weights not needed
edge IDs not needed
simple reachability / DFS / BFS / flood-fill
directed default should mean outgoing traversal
```

---

## 14.5 `IntoNeighborsDirected`: incoming/outgoing adjacency

`IntoNeighborsDirected` gives direction-aware neighbor iteration. For directed graphs, `Outgoing` yields targets of edges from `a`, and `Incoming` yields sources of edges to `a`; for undirected graphs, either direction yields the other endpoints of connected edges. ([Docs.rs][6])

```rust
use petgraph::visit::IntoNeighborsDirected;
use petgraph::Direction;

fn degree_in<G>(graph: G, node: G::NodeId) -> usize
where
    G: IntoNeighborsDirected,
{
    graph.neighbors_directed(node, Direction::Incoming).count()
}

fn degree_out<G>(graph: G, node: G::NodeId) -> usize
where
    G: IntoNeighborsDirected,
{
    graph.neighbors_directed(node, Direction::Outgoing).count()
}
```

Direction-parametric traversal:

```rust
use petgraph::visit::{IntoNeighborsDirected, Visitable, VisitMap};
use petgraph::Direction;

fn directional_reachable<G>(
    graph: G,
    start: G::NodeId,
    dir: Direction,
) -> Vec<G::NodeId>
where
    G: IntoNeighborsDirected + Visitable,
    G::NodeId: Copy,
{
    let mut seen = graph.visit_map();
    let mut stack = vec![start];
    let mut out = Vec::new();

    while let Some(n) = stack.pop() {
        if seen.visit(n) {
            out.push(n);

            for m in graph.neighbors_directed(n, dir) {
                stack.push(m);
            }
        }
    }

    out
}
```

Use when:

```text
reverse reachability
predecessor/successor queries
dependency/dependent split
incoming/outgoing degree logic
edge orientation is semantically important
```

---

## 14.6 `IntoEdges`: edge references adjacent to node

`IntoEdges` extends `IntoEdgeReferences + IntoNeighbors` and yields edge references for a node. For directed graphs, `edges(a)` yields all edges from `a`; for undirected graphs, all connected edges. It is the edge-weight/edge-ID-aware extension of `IntoNeighbors`. ([Docs.rs][7])

```rust
pub trait IntoEdges: IntoEdgeReferences + IntoNeighbors {
    type Edges: Iterator<Item = Self::EdgeRef>;

    fn edges(self, a: Self::NodeId) -> Self::Edges;
}
```

Sum outgoing numeric weights:

```rust
use petgraph::visit::{IntoEdges, EdgeRef};

fn sum_outgoing_u32<G>(graph: G, node: G::NodeId) -> u32
where
    G: IntoEdges,
    G::EdgeRef: EdgeRef<Weight = u32>,
{
    graph.edges(node).map(|e| *e.weight()).sum()
}
```

Collect adjacent edge IDs:

```rust
use petgraph::visit::{IntoEdges, EdgeRef};

fn adjacent_edge_ids<G>(graph: G, node: G::NodeId) -> Vec<G::EdgeId>
where
    G: IntoEdges,
{
    graph.edges(node).map(|e| e.id()).collect()
}
```

Use when:

```text
need edge weights
need edge IDs
need source/target/weight in one iterator
algorithm is local-neighborhood edge-aware
```

---

## 14.7 `IntoEdgesDirected`: direction-aware edge references

`IntoEdgesDirected` exposes edge references in a specified `Direction`; it is the edge-reference equivalent of `IntoNeighborsDirected`. The visit implementation matrix marks it available for `Graph`, `StableGraph`, `GraphMap`, and `MatrixGraph`, but not universally for every graph family. ([Docs.rs][8])

```rust
use petgraph::visit::{IntoEdgesDirected, EdgeRef};
use petgraph::Direction;

fn incoming_edge_ids<G>(graph: G, node: G::NodeId) -> Vec<G::EdgeId>
where
    G: IntoEdgesDirected,
{
    graph
        .edges_directed(node, Direction::Incoming)
        .map(|e| e.id())
        .collect()
}
```

Use when:

```text
need incoming edge weights
need outgoing edge weights
need edge IDs with direction
need predecessor-edge metadata
```

---

## 14.8 `IntoEdgeReferences`: all-edge iteration

`IntoEdgeReferences` requires `Data + GraphRef`, exposes an associated `EdgeRef`, and returns an iterator over all edge references in the graph. Its `EdgeRef` must provide `source`, `target`, `weight`, and `id` through the `EdgeRef` trait. ([Docs.rs][9])

```rust
pub trait IntoEdgeReferences: Data + GraphRef {
    type EdgeRef: EdgeRef<
        NodeId = Self::NodeId,
        EdgeId = Self::EdgeId,
        Weight = Self::EdgeWeight,
    >;

    type EdgeReferences: Iterator<Item = Self::EdgeRef>;

    fn edge_references(self) -> Self::EdgeReferences;
}
```

Generic all-edge dump:

```rust
use petgraph::visit::{IntoEdgeReferences, EdgeRef};

fn edge_triples<G>(graph: G) -> Vec<(G::NodeId, G::NodeId, G::EdgeId)>
where
    G: IntoEdgeReferences,
{
    graph
        .edge_references()
        .map(|e| (e.source(), e.target(), e.id()))
        .collect()
}
```

Generic edge-weight fold:

```rust
use petgraph::visit::{IntoEdgeReferences, EdgeRef};

fn total_edge_cost<G>(graph: G) -> u64
where
    G: IntoEdgeReferences,
    G::EdgeRef: EdgeRef<Weight = u32>,
{
    graph
        .edge_references()
        .map(|e| *e.weight() as u64)
        .sum()
}
```

Use when:

```text
global edge scan
edge export
edge filtering
edge count by predicate
serialization into custom schema
```

---

## 14.9 `IntoNodeIdentifiers`: all-node IDs

`IntoNodeIdentifiers` requires `GraphRef` and returns an iterator over the graph’s `NodeId`s. Implementors include `Graph`, `StableGraph`, `GraphMap`, `MatrixGraph`, and adaptors such as `NodeFiltered`, `EdgeFiltered`, `Reversed`, and `UndirectedAdaptor`. ([Docs.rs][10])

```rust
pub trait IntoNodeIdentifiers: GraphRef {
    type NodeIdentifiers: Iterator<Item = Self::NodeId>;

    fn node_identifiers(self) -> Self::NodeIdentifiers;
}
```

Generic node scan:

```rust
use petgraph::visit::IntoNodeIdentifiers;

fn collect_nodes<G>(graph: G) -> Vec<G::NodeId>
where
    G: IntoNodeIdentifiers,
{
    graph.node_identifiers().collect()
}
```

StableGraph-safe array-remap:

```rust
use std::collections::HashMap;
use petgraph::visit::IntoNodeIdentifiers;

fn dense_remap<G>(graph: G) -> HashMap<G::NodeId, usize>
where
    G: IntoNodeIdentifiers,
    G::NodeId: Eq + std::hash::Hash,
{
    graph
        .node_identifiers()
        .enumerate()
        .map(|(dense, n)| (n, dense))
        .collect()
}
```

Use when:

```text
all live node IDs needed
StableGraph holes must be respected
algorithm should not assume NodeIndex::new(0..node_count)
```

---

## 14.10 `IntoNodeReferences`: all-node IDs plus weights

`IntoNodeReferences` returns node-reference values; its associated `NodeRef` must provide `NodeId` and `Weight`, and `node_references()` yields an iterator over those references. ([Docs.rs][11])

```rust
use petgraph::visit::{IntoNodeReferences, NodeRef};

fn node_weight_pairs<G>(graph: G) -> Vec<(G::NodeId, G::NodeWeight)>
where
    G: IntoNodeReferences,
    G::NodeWeight: Clone,
{
    graph
        .node_references()
        .map(|nr| (nr.id(), nr.weight().clone()))
        .collect()
}
```

Borrowed reporting:

```rust
use petgraph::visit::{IntoNodeReferences, NodeRef};

fn print_nodes<G>(graph: G)
where
    G: IntoNodeReferences,
    G::NodeWeight: std::fmt::Debug,
{
    for node in graph.node_references() {
        println!("{:?}: {:?}", node.id(), node.weight());
    }
}
```

Use when:

```text
node export needs IDs + weights
node filtering/reporting needs payloads
generic code should avoid concrete node_weight methods
```

---

## 14.11 `Data`: associated node/edge weight types

`Data` extends `GraphBase` and defines `NodeWeight` and `EdgeWeight`. Rustdoc lists `Graph`, `StableGraph`, `GraphMap`, `MatrixGraph`, and `Csr` as implementors with `NodeWeight = N` and `EdgeWeight = E`. ([Docs.rs][12])

```rust
pub trait Data: GraphBase {
    type NodeWeight;
    type EdgeWeight;
}
```

Generic associated-weight declaration:

```rust
use petgraph::visit::Data;

fn weight_type_names<G>()
where
    G: Data,
{
    let _ = std::any::type_name::<G::NodeWeight>();
    let _ = std::any::type_name::<G::EdgeWeight>();
}
```

Use when:

```text
trait bounds need to name node/edge weight types
no actual weight lookup is needed
combined with IntoNodeReferences / IntoEdgeReferences
```

---

## 14.12 `DataMap`: immutable weight lookup

`DataMap` extends `Data` and exposes `node_weight(id) -> Option<&NodeWeight>` and `edge_weight(id) -> Option<&EdgeWeight>`. It is explicitly documented as access to node and edge weights / associated data. ([Docs.rs][13])

```rust
use petgraph::data::DataMap;

fn get_node_weight<G>(
    graph: &G,
    node: G::NodeId,
) -> Option<&G::NodeWeight>
where
    G: DataMap,
{
    graph.node_weight(node)
}

fn get_edge_weight<G>(
    graph: &G,
    edge: G::EdgeId,
) -> Option<&G::EdgeWeight>
where
    G: DataMap,
{
    graph.edge_weight(edge)
}
```

Use when:

```text
direct ID -> weight lookup needed
safe Option boundary required
algorithm has node/edge IDs already
generic function should work across graph families implementing DataMap
```

---

## 14.13 `DataMapMut`: mutable weight lookup

`DataMapMut` is the mutable counterpart to `DataMap`; the data module documents it as mutable access to node and edge weights. ([Docs.rs][14])

```rust
use petgraph::data::DataMapMut;

fn bump_node_u32<G>(graph: &mut G, node: G::NodeId)
where
    G: DataMapMut<NodeWeight = u32>,
{
    if let Some(w) = graph.node_weight_mut(node) {
        *w += 1;
    }
}

fn bump_edge_u32<G>(graph: &mut G, edge: G::EdgeId)
where
    G: DataMapMut<EdgeWeight = u32>,
{
    if let Some(w) = graph.edge_weight_mut(edge) {
        *w += 1;
    }
}
```

Use when:

```text
topology stays unchanged
payload mutation is generic
graph family should remain abstract
caller already has IDs
```

Avoid when:

```text
topology mutation required
node/edge insertion/removal required
concrete Graph methods are needed
```

---

## 14.14 `NodeCount` and `EdgeCount`

`NodeCount` extends `GraphBase` and exposes `node_count(&self) -> usize`; it is documented as “a graph with a known node count.” ([Docs.rs][15])

```rust
use petgraph::visit::{NodeCount, EdgeCount};

fn size<G>(graph: &G) -> (usize, usize)
where
    G: NodeCount + EdgeCount,
{
    (graph.node_count(), graph.edge_count())
}
```

`EdgeCount` is the analogous known-edge-count trait, listed in the visit module’s trait inventory and implementation matrix. ([Docs.rs][1])

Use when:

```text
preallocating output
sanity checking graph size
choosing algorithm branch by density
statistics/reporting
```

Density gate:

```rust
use petgraph::visit::{NodeCount, EdgeCount};

fn is_dense_enough_for_matrix<G>(graph: &G, threshold: f64) -> bool
where
    G: NodeCount + EdgeCount,
{
    let n = graph.node_count();

    if n == 0 {
        return false;
    }

    let max_directed_edges = (n as f64) * (n as f64);
    let density = graph.edge_count() as f64 / max_directed_edges;

    density >= threshold
}
```

---

## 14.15 `NodeIndexable`, `NodeCompactIndexable`, `EdgeIndexable`

`NodeIndexable` maps graph node IDs to integer indices and exposes an upper bound suitable for bitmap sizing; `NodeCompactIndexable` adds the stronger “range without holes” guarantee; `EdgeIndexable` similarly maps edge IDs to integer indices. The visit module’s trait list explicitly distinguishes `NodeIndexable` from `NodeCompactIndexable` and describes the compact version as node IDs mapping to indices “in a range without holes.” ([Docs.rs][1])

Sparse-safe bitmap:

```rust
use petgraph::visit::{IntoNodeIdentifiers, NodeIndexable};

fn live_node_bitmap<G>(graph: G) -> Vec<bool>
where
    G: IntoNodeIdentifiers + NodeIndexable,
    G::NodeId: Copy,
{
    let mut seen = vec![false; graph.node_bound()];

    for n in graph.node_identifiers() {
        seen[graph.to_index(n)] = true;
    }

    seen
}
```

Dense-only algorithm:

```rust
use petgraph::visit::{IntoNodeIdentifiers, NodeCompactIndexable};

fn compact_scores<G>(graph: G) -> Vec<u32>
where
    G: IntoNodeIdentifiers + NodeCompactIndexable,
    G::NodeId: Copy,
{
    let mut scores = vec![0; graph.node_bound()];

    for n in graph.node_identifiers() {
        scores[graph.to_index(n)] += 1;
    }

    scores
}
```

Edge bitmap:

```rust
use petgraph::visit::EdgeIndexable;

fn edge_bitmap<G>(graph: &G) -> Vec<bool>
where
    G: EdgeIndexable,
{
    vec![false; graph.edge_bound()]
}
```

Rule:

```text
Use NodeIndexable:
    bitmap upper bound
    holes allowed
    StableGraph-compatible

Use NodeCompactIndexable:
    dense Vec indexed by node ID
    holes not allowed
    StableGraph excluded

Use EdgeIndexable:
    edge bitmap/index mapping
    not universal across graph families
```

The visit implementation table marks `NodeCompactIndexable` and `EdgeIndexable` as non-universal across graph types, while `GraphBase`, `GraphProp`, `NodeCount`, `NodeIndexable`, `EdgeCount`, `Data`, node/edge references, neighbor/edge traversal, `Visitable`, and `GetAdjacencyMatrix` are broadly implemented across the listed graph families. ([Docs.rs][1])

---

## 14.16 Which graph types implement which core traits

The visit module includes an implementation matrix for `Graph`, `StableGraph`, `GraphMap`, `MatrixGraph`, `Csr`, and `List`. For the main five graph families, the practical matrix is: ([Docs.rs][1])

| Trait                   | `Graph` | `StableGraph` | `GraphMap` | `MatrixGraph` | `Csr` |
| ----------------------- | ------: | ------------: | ---------: | ------------: | ----: |
| `GraphBase`             |     yes |           yes |        yes |           yes |   yes |
| `GraphProp`             |     yes |           yes |        yes |           yes |   yes |
| `NodeCount`             |     yes |           yes |        yes |           yes |   yes |
| `NodeIndexable`         |     yes |           yes |        yes |           yes |   yes |
| `NodeCompactIndexable`  |     yes |            no |        yes |           yes |   yes |
| `EdgeCount`             |     yes |           yes |        yes |           yes |   yes |
| `EdgeIndexable`         |     yes |           yes |        yes |            no |    no |
| `Data`                  |     yes |           yes |        yes |           yes |   yes |
| `IntoNodeIdentifiers`   |     yes |           yes |        yes |           yes |   yes |
| `IntoNodeReferences`    |     yes |           yes |        yes |           yes |   yes |
| `IntoEdgeReferences`    |     yes |           yes |        yes |           yes |   yes |
| `IntoNeighbors`         |     yes |           yes |        yes |           yes |   yes |
| `IntoNeighborsDirected` |     yes |           yes |        yes |           yes |    no |
| `IntoEdges`             |     yes |           yes |        yes |           yes |   yes |
| `IntoEdgesDirected`     |     yes |           yes |        yes |           yes |    no |
| `Visitable`             |     yes |           yes |        yes |           yes |   yes |

Deployment rule:

```text
Need maximum generic compatibility:
    Graph usually easiest

Need StableGraph support:
    avoid NodeCompactIndexable assumptions

Need Csr support:
    avoid IntoNeighborsDirected / IntoEdgesDirected bounds

Need MatrixGraph support:
    avoid EdgeIndexable bounds

Need GraphMap support:
    remember NodeId = N and EdgeId = (N, N)
```

---

## 14.17 Writing generic functions: trait-bound recipes

### Recipe A: reachable count

Minimum bound: `IntoNeighbors + Visitable`.

```rust
use petgraph::visit::{IntoNeighbors, Visitable, VisitMap};

fn reachable_count<G>(graph: G, start: G::NodeId) -> usize
where
    G: IntoNeighbors + Visitable,
    G::NodeId: Copy,
{
    let mut seen = graph.visit_map();
    let mut stack = vec![start];
    let mut count = 0;

    while let Some(n) = stack.pop() {
        if seen.visit(n) {
            count += 1;

            for m in graph.neighbors(n) {
                stack.push(m);
            }
        }
    }

    count
}
```

### Recipe B: direction-aware reachable set

Minimum bound: `IntoNeighborsDirected + Visitable`.

```rust
use petgraph::visit::{IntoNeighborsDirected, Visitable, VisitMap};
use petgraph::Direction;

fn reachable_in_direction<G>(
    graph: G,
    start: G::NodeId,
    dir: Direction,
) -> Vec<G::NodeId>
where
    G: IntoNeighborsDirected + Visitable,
    G::NodeId: Copy,
{
    let mut seen = graph.visit_map();
    let mut stack = vec![start];
    let mut out = Vec::new();

    while let Some(n) = stack.pop() {
        if seen.visit(n) {
            out.push(n);

            for m in graph.neighbors_directed(n, dir) {
                stack.push(m);
            }
        }
    }

    out
}
```

### Recipe C: all-edge export

Minimum bound: `IntoEdgeReferences`.

```rust
use petgraph::visit::{IntoEdgeReferences, EdgeRef};

#[derive(Debug)]
struct EdgeRow<N, E> {
    source: N,
    target: N,
    edge_id: E,
}

fn export_edge_ids<G>(graph: G) -> Vec<EdgeRow<G::NodeId, G::EdgeId>>
where
    G: IntoEdgeReferences,
{
    graph
        .edge_references()
        .map(|e| EdgeRow {
            source: e.source(),
            target: e.target(),
            edge_id: e.id(),
        })
        .collect()
}
```

### Recipe D: all-node export with weights

Minimum bound: `IntoNodeReferences`.

```rust
use petgraph::visit::{IntoNodeReferences, NodeRef};

fn export_nodes<G>(graph: G) -> Vec<(G::NodeId, G::NodeWeight)>
where
    G: IntoNodeReferences,
    G::NodeWeight: Clone,
{
    graph
        .node_references()
        .map(|n| (n.id(), n.weight().clone()))
        .collect()
}
```

### Recipe E: payload mutation without topology mutation

Minimum bound: `DataMapMut`.

```rust
use petgraph::data::DataMapMut;

fn mark_node<G>(graph: &mut G, n: G::NodeId)
where
    G: DataMapMut<NodeWeight = bool>,
{
    if let Some(flag) = graph.node_weight_mut(n) {
        *flag = true;
    }
}
```

### Recipe F: compact dense vector algorithm

Minimum bound: `IntoNodeIdentifiers + NodeCompactIndexable`.

```rust
use petgraph::visit::{IntoNodeIdentifiers, NodeCompactIndexable};

fn dense_node_positions<G>(graph: G) -> Vec<usize>
where
    G: IntoNodeIdentifiers + NodeCompactIndexable,
    G::NodeId: Copy,
{
    let mut positions = vec![usize::MAX; graph.node_bound()];

    for n in graph.node_identifiers() {
        let i = graph.to_index(n);
        positions[i] = i;
    }

    positions
}
```

---

## 14.18 Generic trait bounds vs concrete `Graph` APIs

### Generic algorithm value case

```text
Pros:
    works across Graph / StableGraph / GraphMap / MatrixGraph / Csr when bounds match
    works across adaptors: Reversed, NodeFiltered, EdgeFiltered, UndirectedAdaptor
    expresses exact capability requirements
    enables zero-copy views
    supports library-level algorithms

Costs:
    trait bounds verbose
    compile errors more abstract
    mutation/build APIs usually unavailable
    graph-family edge/index semantics differ
    some traits are not dyn-compatible
    not all graph types implement all directed/index traits
```

The `visit` docs state the traits are composable but mainly operate through shared references, and that the trait system is intentionally loosely coupled with some missing traits. ([Docs.rs][1])

### Concrete `Graph` value case

```text
Pros:
    simplest syntax
    broadest method surface
    explicit add/remove/update APIs
    compact NodeIndex / EdgeIndex semantics
    easiest app-local mutation
    easier diagnostics

Costs:
    excludes StableGraph/GraphMap/Csr/MatrixGraph callers
    cannot operate directly on filtered/reversed views
    less reusable as library API
```

The crate overview describes petgraph’s concrete graph types as optimized for different tradeoffs and notes that some types, especially `Graph`, expose the fullest method and algorithm support while others may not implement the full feature set. ([Docs.rs][16])

---

## 14.19 Bound-selection cheat sheet

```text
Need only node IDs:
    IntoNodeIdentifiers

Need node IDs + node weights:
    IntoNodeReferences

Need all edges:
    IntoEdgeReferences

Need neighbors only:
    IntoNeighbors

Need incoming/outgoing neighbors:
    IntoNeighborsDirected

Need local edge refs:
    IntoEdges

Need incoming/outgoing edge refs:
    IntoEdgesDirected

Need visitor map:
    Visitable

Need node count:
    NodeCount

Need edge count:
    EdgeCount

Need node/edge weight associated types:
    Data

Need safe immutable weight lookup by ID:
    DataMap

Need safe mutable weight lookup by ID:
    DataMapMut

Need node ID -> usize map:
    NodeIndexable

Need dense no-hole node indexing:
    NodeCompactIndexable

Need edge ID -> usize map:
    EdgeIndexable

Need graph directedness:
    GraphProp
```

---

## 14.20 Anti-pattern inventory

```text
Anti-pattern:
    Require Graph<N,E> when only neighbors are needed.
Fix:
    use IntoNeighbors + Visitable.

Anti-pattern:
    Require NodeCompactIndexable for StableGraph-compatible code.
Problem:
    StableGraph can have holes.
Fix:
    use NodeIndexable + IntoNodeIdentifiers or dense remap.

Anti-pattern:
    Use IntoNeighbors when edge weights are needed.
Fix:
    use IntoEdges or IntoEdgesDirected.

Anti-pattern:
    Use IntoEdgeReferences when only edge count is needed.
Fix:
    use EdgeCount.

Anti-pattern:
    Write generic function over owned G when &G is enough.
Fix:
    design function as fn f<G>(g: G, ...) where G: IntoNeighbors; call with &graph.

Anti-pattern:
    Expect GraphMap NodeId to be NodeIndex.
Problem:
    GraphMap NodeId = node key N.
Fix:
    write bounds using G::NodeId, not NodeIndex.

Anti-pattern:
    Expect Csr to satisfy direction-aware edge/neighbor traits.
Problem:
    implementation matrix does not mark Csr for IntoNeighborsDirected / IntoEdgesDirected.
Fix:
    use outgoing-only IntoNeighbors/IntoEdges or choose Graph/Csr-specific logic.

Anti-pattern:
    Use DataMapMut expecting topology mutation.
Problem:
    DataMapMut is for mutable weights, not add/remove nodes/edges.
Fix:
    use concrete graph APIs or construction traits.
```

---

## 14.21 Production deployment checklist

```text
For app-local code:
    prefer concrete Graph / StableGraph APIs
    keep type aliases clear
    avoid generic complexity unless reuse matters

For library algorithms:
    start with the narrowest trait bound
    add traits only when methods require them
    test with Graph + StableGraph + GraphMap where feasible
    test Csr separately if direction-aware bounds are avoided
    test adaptors if filter/reverse views are expected

For index-heavy algorithms:
    prefer NodeIndexable over NodeCompactIndexable unless no-hole indexing is required
    document StableGraph compatibility
    remap live nodes for dense arrays

For edge-weight algorithms:
    use IntoEdges / IntoEdgeReferences + EdgeRef
    constrain EdgeRef::Weight only as tightly as required
    extract numeric cost with closures when possible

For mutation:
    use DataMapMut for weight mutation
    use concrete graph type for topology mutation
    do not pretend traversal traits can add/remove graph structure
```

Final rule:

```text
Petgraph traits describe graph capabilities.
Use the narrowest capability set:
    more generic = more reusable, more verbose
    more concrete = simpler, more powerful, less reusable
```

[1]: https://docs.rs/petgraph/latest/petgraph/visit/index.html "petgraph::visit - Rust"
[2]: https://docs.rs/petgraph/latest/petgraph/visit/trait.GraphBase.html "GraphBase in petgraph::visit - Rust"
[3]: https://docs.rs/petgraph/latest/petgraph/visit/trait.GraphProp.html "GraphProp in petgraph::visit - Rust"
[4]: https://docs.rs/nopetgraph/latest/petgraph/visit/trait.GraphRef.html "GraphRef in petgraph::visit - Rust"
[5]: https://docs.rs/petgraph/latest/petgraph/visit/trait.IntoNeighbors.html "IntoNeighbors in petgraph::visit - Rust"
[6]: https://docs.rs/petgraph/latest/petgraph/visit/trait.IntoNeighborsDirected.html "IntoNeighborsDirected in petgraph::visit - Rust"
[7]: https://docs.rs/nopetgraph/latest/petgraph/visit/trait.IntoEdges.html "IntoEdges in petgraph::visit - Rust"
[8]: https://docs.rs/petgraph/latest/petgraph/visit/trait.IntoEdgesDirected.html "IntoEdgesDirected in petgraph::visit - Rust"
[9]: https://docs.rs/petgraph/latest/petgraph/visit/trait.IntoEdgeReferences.html "IntoEdgeReferences in petgraph::visit - Rust"
[10]: https://docs.rs/petgraph/latest/petgraph/visit/trait.IntoNodeIdentifiers.html "IntoNodeIdentifiers in petgraph::visit - Rust"
[11]: https://docs.rs/petgraph/latest/petgraph/visit/trait.IntoNodeReferences.html "IntoNodeReferences in petgraph::visit - Rust"
[12]: https://docs.rs/petgraph/latest/petgraph/visit/trait.Data.html "Data in petgraph::visit - Rust"
[13]: https://docs.rs/petgraph/latest/petgraph/data/trait.DataMap.html "DataMap in petgraph::data - Rust"
[14]: https://docs.rs/petgraph/latest/petgraph/data/index.html "petgraph::data - Rust"
[15]: https://docs.rs/petgraph/latest/petgraph/visit/trait.NodeCount.html "NodeCount in petgraph::visit - Rust"
[16]: https://docs.rs/petgraph/?utm_source=chatgpt.com "petgraph - Rust"


# 15) Algorithm catalog: shortest paths — petgraph

Format follows the uploaded advanced-reference style. 

Version target: **petgraph 0.8.3 released API**. The `petgraph::algo` module re-exports shortest-path functions including `dijkstra`, `bidirectional_dijkstra`, `astar`, `bellman_ford`, `find_negative_cycle`, `floyd_warshall`, `johnson`, `parallel_johnson`, `spfa`, and `k_shortest_path`; docs note that most algorithms live in `algo`, while simple DFS/BFS traversal lives in `visit`, and some algorithms are still being migrated toward trait-generic implementations. ([Docs.rs][1])

---

## 15.0 Core imports

```rust id="82vf61"
use petgraph::algo::{
    dijkstra,
    bidirectional_dijkstra,
    astar,
    bellman_ford,
    find_negative_cycle,
    floyd_warshall,
    johnson,
    spfa,
    k_shortest_path,
};

#[cfg(feature = "rayon")]
use petgraph::algo::parallel_johnson;

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::{EdgeRef, NodeIndexable};
```

Cargo baseline:

```toml id="4a12px"
[dependencies]
petgraph = "0.8.3"
```

For `parallel_johnson`:

```toml id="vtx0zh"
[dependencies]
petgraph = { version = "0.8.3", features = ["rayon"] }
rayon = "1"
```

The `rayon` feature enables the `rayon` dependency and rayon support through `hashbrown` and `indexmap`; `parallel_johnson` is documented as a Johnson implementation that parallelizes Dijkstra calls with `rayon`. ([Docs.rs][2])

---

## 15.1 Cost model: edge weights vs algorithm costs

Petgraph shortest-path functions generally accept an **edge-cost closure**:

```rust id="pq1f1s"
|edge_ref| -> K
```

Do not assume graph edge weight `E` is already the shortest-path cost. Petgraph graph weights are arbitrary domain data; shortest-path algorithms call the cost closure to convert edge references into numeric/path-measure values.

```rust id="6y900m"
use petgraph::graph::DiGraph;
use petgraph::algo::dijkstra;
use petgraph::visit::EdgeRef;

#[derive(Clone, Debug)]
struct Link {
    latency_ms: u32,
    enabled: bool,
}

let mut g = DiGraph::<&str, Link>::new();

let api = g.add_node("api");
let db = g.add_node("db");

g.add_edge(api, db, Link {
    latency_ms: 4,
    enabled: true,
});

let distances = dijkstra(&g, api, None, |edge| {
    edge.weight().latency_ms
});
```

Rule:

```text id="d41v9p"
E = domain payload
K = algorithm path measure
edge_cost: EdgeRef<E> -> K

Never encode all domain edge semantics into K.
Extract K at algorithm call site.
```

---

## 15.2 Numeric trait taxonomy

Petgraph signatures use several measure traits:

```text id="ng2s13"
Measure:
    dijkstra
    bidirectional_dijkstra
    astar
    k_shortest_path

BoundedMeasure:
    floyd_warshall
    johnson
    parallel_johnson
    spfa

FloatMeasure:
    bellman_ford
    find_negative_cycle
```

Operational interpretation:

```text id="l81sr5"
Measure:
    non-negative shortest-path arithmetic

BoundedMeasure:
    finite/infinite-style all-pairs arithmetic

FloatMeasure:
    floating-point negative-weight-capable Bellman-Ford family

K: Copy:
    common requirement for returned/stored path measures
```

Concrete safe starting choices:

```rust id="s7agey"
// Non-negative integer weights:
u32
u64
usize

// Negative integer weights for Johnson/Floyd/SPFA-style APIs:
i32
i64

// Bellman-Ford / find_negative_cycle:
f32
f64
```

Algorithm signatures expose these bounds directly: `dijkstra`, `bidirectional_dijkstra`, `astar`, and `k_shortest_path` require `K: Measure + Copy`; `floyd_warshall`, `johnson`, and `spfa` require bounded-measure-style cost types, while `bellman_ford` and `find_negative_cycle` require `EdgeWeight: FloatMeasure`. ([Docs.rs][3])

---

## 15.3 Negative-weight policy

| Algorithm                | Negative edge weights |                     Negative cycles | Notes                                     |
| ------------------------ | --------------------: | ----------------------------------: | ----------------------------------------- |
| `dijkstra`               |                    No |                       Not supported | edge costs must be non-negative           |
| `bidirectional_dijkstra` |                    No |                       Not supported | non-negative, single source-target        |
| `astar`                  |                    No |                       Not supported | edge and heuristic estimates non-negative |
| `k_shortest_path`        |                    No |                       Not supported | non-negative kth-distance variant         |
| `bellman_ford`           |                   Yes |        Returns `Err(NegativeCycle)` | single-source all destinations            |
| `find_negative_cycle`    |                   Yes | Finds reachable negative cycle path | source-reachable only                     |
| `spfa`                   |                   Yes |        Returns `Err(NegativeCycle)` | single-source all destinations            |
| `floyd_warshall`         |                   Yes |        Returns `Err(NegativeCycle)` | all pairs                                 |
| `johnson`                |                   Yes |        Returns `Err(NegativeCycle)` | all pairs, sparse-oriented                |
| `parallel_johnson`       |                   Yes |        Returns `Err(NegativeCycle)` | rayon-parallel all pairs                  |

Docs explicitly state non-negative edge-cost requirements for Dijkstra, bidirectional Dijkstra, A*, and kth-shortest path; Bellman-Ford, SPFA, Floyd-Warshall, and Johnson allow positive or negative edge weights but reject negative cycles. ([Docs.rs][3])

---

## 15.4 `dijkstra` — single-source shortest distances

Signature:

```rust id="k0l3bz"
pub fn dijkstra<G, F, K>(
    graph: G,
    start: G::NodeId,
    goal: Option<G::NodeId>,
    edge_cost: F,
) -> HashMap<G::NodeId, K>
where
    G: IntoEdges + Visitable,
    G::NodeId: Eq + Hash,
    F: FnMut(G::EdgeRef) -> K,
    K: Measure + Copy;
```

Semantics:

```text id="0qceuk"
Input:
    graph
    start node
    optional goal
    non-negative edge cost closure

Output:
    HashMap<NodeId, K>
    maps reachable NodeId -> shortest distance from start

Early exit:
    if goal = Some(node), terminates once goal's cost is calculated

No predecessor/path output:
    distances only
```

Docs define `dijkstra` as computing shortest path lengths from `start` to every reachable node, returning a `HashMap<NodeId, K>`; if `goal` is supplied, the algorithm terminates when that goal cost is calculated; complexity is `O((|V| + |E|) log |V|)` time and `O(|V| + |E|)` auxiliary space. ([Docs.rs][3])

### Basic syntax

```rust id="jpmdhl"
use petgraph::algo::dijkstra;
use petgraph::graph::DiGraph;

let mut g = DiGraph::<&str, u32>::new();

let a = g.add_node("a");
let b = g.add_node("b");
let c = g.add_node("c");

g.add_edge(a, b, 2);
g.add_edge(b, c, 3);
g.add_edge(a, c, 10);

let dist = dijkstra(&g, a, None, |edge| *edge.weight());

assert_eq!(dist[&a], 0);
assert_eq!(dist[&b], 2);
assert_eq!(dist[&c], 5);
```

### Early-exit single target

```rust id="gj3ebu"
let dist_to_c = dijkstra(&g, a, Some(c), |edge| *edge.weight());

assert_eq!(dist_to_c.get(&c), Some(&5));
```

Use `dijkstra` when:

```text id="1dwnjo"
single source
all reachable distances needed
non-negative edge costs
path reconstruction not required
multiple targets from one source possible
```

Avoid `dijkstra` when:

```text id="5ihucv"
negative edge weights exist
only one source-target path is needed and heuristic available
predecessor/path output is required
all-pairs distances required
```

---

## 15.5 `bidirectional_dijkstra` — single source-target cost

Signature:

```rust id="xnb1mu"
pub fn bidirectional_dijkstra<G, F, K>(
    graph: G,
    start: G::NodeId,
    goal: G::NodeId,
    edge_cost: F,
) -> Option<K>
where
    G: Visitable + IntoEdgesDirected,
    G::NodeId: Eq + Hash,
    F: FnMut(G::EdgeRef) -> K,
    K: Measure + Copy;
```

Semantics:

```text id="eclfbl"
Input:
    graph
    start
    goal
    non-negative edge cost closure

Output:
    Some(total_cost)
    None if no path found

Path:
    not returned

Predecessors:
    not returned
```

Docs state bidirectional Dijkstra computes shortest path length from start to target, has the same asymptotic time complexity as standard Dijkstra, often explores roughly half the nodes by searching from both ends, and is especially useful for long paths in sparse graphs; regular Dijkstra may be preferable for multiple goals or close source/goal. ([Docs.rs][4])

```rust id="g2enbp"
use petgraph::algo::bidirectional_dijkstra;
use petgraph::graph::DiGraph;

let result = bidirectional_dijkstra(&g, a, c, |edge| *edge.weight());

assert_eq!(result, Some(5));
```

Use when:

```text id="rxj3hp"
single source-target distance
non-negative weights
graph supports IntoEdgesDirected
sparse graph
path likely long relative to graph size
distance only is sufficient
```

Avoid when:

```text id="0j99vb"
path nodes needed
multiple goals needed
negative edges exist
graph type lacks IntoEdgesDirected
```

---

## 15.6 `astar` — goal-directed single path

Signature:

```rust id="wg3ef4"
pub fn astar<G, F, H, K, IsGoal>(
    graph: G,
    start: G::NodeId,
    is_goal: IsGoal,
    edge_cost: F,
    estimate_cost: H,
) -> Option<(K, Vec<G::NodeId>)>
where
    G: IntoEdges + Visitable,
    IsGoal: FnMut(G::NodeId) -> bool,
    G::NodeId: Eq + Hash,
    F: FnMut(G::EdgeRef) -> K,
    H: FnMut(G::NodeId) -> K,
    K: Measure + Copy;
```

Semantics:

```text id="u152uc"
Input:
    start
    is_goal(node) -> bool
    edge_cost(edge) -> K
    estimate_cost(node) -> K

Output:
    Some((total_cost, path_nodes))
    None if no goal reached

Requirements:
    edge costs non-negative
    heuristic estimates non-negative
    heuristic admissible for guaranteed optimality
```

Docs state A* returns total path cost plus the path from start to finish; `is_goal` defines the finish condition, edge costs must be non-negative, and the heuristic must be admissible—never overestimating true remaining cost—to guarantee the actual shortest path. ([Docs.rs][5])

```rust id="gcuhk1"
use petgraph::algo::astar;
use petgraph::graph::DiGraph;

let result = astar(
    &g,
    a,
    |node| node == c,
    |edge| *edge.weight(),
    |_node| 0u32, // zero heuristic == Dijkstra-like behavior
);

let (cost, path) = result.expect("path exists");

assert_eq!(cost, 5);
assert_eq!(path, vec![a, b, c]);
```

Heuristic pattern:

```rust id="iek9gp"
#[derive(Clone, Debug)]
struct Pos {
    x: i32,
    y: i32,
}

fn manhattan(a: &Pos, b: &Pos) -> u32 {
    (a.x.abs_diff(b.x) + a.y.abs_diff(b.y)) as u32
}

let result = astar(
    &grid_graph,
    start,
    |n| n == goal,
    |edge| edge.weight().cost,
    |n| manhattan(&grid_graph[n], &grid_graph[goal]),
);
```

Use when:

```text id="5hrgdl"
single path needed
goal known or goal predicate available
non-negative edge costs
admissible heuristic available
path reconstruction needed
```

Avoid when:

```text id="q5zqkn"
all distances from source needed
heuristic unavailable and dijkstra output is enough
negative weights exist
heuristic may overestimate and optimality matters
```

---

## 15.7 `bellman_ford` — single-source with negative edges + predecessors

Signature:

```rust id="hetpv3"
pub fn bellman_ford<G>(
    g: G,
    source: G::NodeId,
) -> Result<Paths<G::NodeId, G::EdgeWeight>, NegativeCycle>
where
    G: NodeCount + IntoNodeIdentifiers + IntoEdges + NodeIndexable,
    G::EdgeWeight: FloatMeasure;
```

Semantics:

```text id="2fjsip"
Input:
    graph whose edge weights are the path costs
    source

Output:
    Ok(Paths { distances, predecessors })
    Err(NegativeCycle) if graph contains negative cycle

Negative weights:
    permitted

Cost closure:
    no closure; edge weight type itself is the cost
```

Docs state `bellman_ford` computes shortest paths from `source` to all nodes, permits negative edge costs, returns an error if a negative-weight cycle exists, and returns `Paths` containing distance and predecessor vectors indexed by graph node indices; complexity is `O(|V||E|)` time and `O(|V|)` auxiliary space. ([Docs.rs][6])

```rust id="8uuqmm"
use petgraph::algo::bellman_ford;
use petgraph::graph::DiGraph;
use petgraph::visit::NodeIndexable;

let mut g = DiGraph::<(), f32>::new();

let a = g.add_node(());
let b = g.add_node(());
let c = g.add_node(());

g.add_edge(a, b, 2.0);
g.add_edge(b, c, -1.0);
g.add_edge(a, c, 5.0);

let paths = bellman_ford(&g, a).expect("no negative cycle");

assert_eq!(paths.distances[g.to_index(c)], 1.0);
assert_eq!(paths.predecessors[g.to_index(c)], Some(b));
```

Use when:

```text id="y6jq1p"
single source
negative edge weights possible
predecessor reconstruction needed
edge weight itself is numeric cost
graph has no negative cycles
```

Avoid when:

```text id="fe3slb"
edge cost must be derived from domain E via closure
non-negative graph can use faster Dijkstra
all-pairs needed
large graph and performance is critical
```

---

## 15.8 `find_negative_cycle` — reachable negative-cycle diagnostic

Signature:

```rust id="ss3ddr"
pub fn find_negative_cycle<G>(
    g: G,
    source: G::NodeId,
) -> Option<Vec<G::NodeId>>
where
    G: NodeCount + IntoNodeIdentifiers + IntoEdges + NodeIndexable + Visitable,
    G::EdgeWeight: FloatMeasure;
```

Semantics:

```text id="z8a5th"
Input:
    graph
    source

Output:
    Some(Vec<NodeId>) negative cycle path reachable from source
    None if no reachable negative cycle found

Cost:
    graph edge weight itself
```

Docs state this function searches for a negative cycle reachable from `source` using Bellman-Ford logic, returns one path of `NodeId`s when found, and has `O(|V||E|)` time and `O(|V|)` auxiliary space. ([Docs.rs][7])

```rust id="3wfoj4"
use petgraph::algo::find_negative_cycle;
use petgraph::graph::DiGraph;

let mut g = DiGraph::<(), f32>::new();

let a = g.add_node(());
let b = g.add_node(());
let c = g.add_node(());

g.add_edge(a, b, 1.0);
g.add_edge(b, c, -3.0);
g.add_edge(c, a, 1.0);

if let Some(cycle) = find_negative_cycle(&g, a) {
    println!("negative cycle: {cycle:?}");
}
```

Use when:

```text id="gkcrma"
Bellman-Ford/SPFA/Floyd/Johnson returns NegativeCycle
debugging bad weight data
need an actual cycle path, not just error marker
negative cycle only matters if reachable from source
```

---

## 15.9 `spfa` — queue-based single-source with negative edges

Signature:

```rust id="f3nfah"
pub fn spfa<G, F, K>(
    graph: G,
    source: G::NodeId,
    edge_cost: F,
) -> Result<Paths<G::NodeId, K>, NegativeCycle>
where
    G: IntoEdges + IntoNodeIdentifiers + NodeIndexable,
    F: FnMut(G::EdgeRef) -> K,
    K: BoundedMeasure + Copy;
```

Semantics:

```text id="7byz6f"
Input:
    graph
    source
    edge_cost closure

Output:
    Ok(Paths { distances, predecessors })
    Err(NegativeCycle)

Negative weights:
    permitted

Negative cycles:
    rejected

Cost closure:
    yes
```

Docs describe SPFA as computing shortest path lengths from `source` to all other nodes with positive or negative edge weights but no negative cycles; it returns `Paths` on success or `NegativeCycle` on error, has worst-case `O(|V||E|)` time, and is generally assumed average-case `O(|E|)` with `O(|V|)` auxiliary space. ([Docs.rs][8])

```rust id="pv4g6h"
use petgraph::algo::spfa;
use petgraph::graph::DiGraph;
use petgraph::visit::NodeIndexable;

#[derive(Clone, Debug)]
struct Edge {
    delta: i32,
}

let mut g = DiGraph::<(), Edge>::new();

let a = g.add_node(());
let b = g.add_node(());
let c = g.add_node(());

g.add_edge(a, b, Edge { delta: 3 });
g.add_edge(b, c, Edge { delta: -1 });

let paths = spfa(&g, a, |edge| edge.weight().delta)
    .expect("no negative cycle");

assert_eq!(paths.distances[g.to_index(c)], 2);
assert_eq!(paths.predecessors[g.to_index(c)], Some(b));
```

Use when:

```text id="8l8uy2"
single source
negative edge costs possible
cost must be closure-derived from edge payload
predecessors needed
practical sparse graphs may benefit
```

Avoid when:

```text id="u0de3z"
worst-case guarantees dominate
all-pairs distances needed
non-negative weights allow Dijkstra
```

---

## 15.10 `floyd_warshall` — all-pairs, dense/simple API

Signature:

```rust id="qi57hl"
pub fn floyd_warshall<G, F, K>(
    graph: G,
    edge_cost: F,
) -> Result<HashMap<(G::NodeId, G::NodeId), K>, NegativeCycle>
where
    G: NodeCompactIndexable + IntoEdgeReferences + IntoNodeIdentifiers + GraphProp,
    G::NodeId: Eq + Hash,
    F: FnMut(G::EdgeRef) -> K,
    K: BoundedMeasure + Copy;
```

Semantics:

```text id="p47lyc"
Input:
    graph
    edge_cost closure

Output:
    Ok(HashMap<(source, target), distance>)
    Err(NegativeCycle)

Problem class:
    all-pairs shortest paths

Negative weights:
    permitted

Negative cycles:
    rejected
```

Docs state Floyd-Warshall computes all-pairs shortest path lengths in a weighted graph with positive or negative edge weights but no negative cycles, returns a `HashMap<(NodeId, NodeId), K>`, has `O(|V|³)` time and `O(|V|²)` auxiliary space, and recommends Johnson for sparse graphs. ([Docs.rs][9])

```rust id="3vyfdj"
use petgraph::algo::floyd_warshall;
use petgraph::graph::DiGraph;

let all_pairs = floyd_warshall(&g, |edge| *edge.weight())
    .expect("no negative cycle");

let d_ac = all_pairs.get(&(a, c)).copied();
```

Use when:

```text id="cvpk6n"
all-pairs distances required
graph is small or dense
negative edges possible
simple implementation preferred
NodeCompactIndexable available
```

Avoid when:

```text id="ct8q12"
graph is large and sparse
StableGraph holes matter and NodeCompactIndexable bound fails
only one source or one target needed
predecessor/path reconstruction required
```

---

## 15.11 `johnson` — all-pairs, sparse-oriented

Signature:

```rust id="ra9g30"
pub fn johnson<G, F, K>(
    graph: G,
    edge_cost: F,
) -> Result<HashMap<(G::NodeId, G::NodeId), K>, NegativeCycle>
where
    G: IntoEdges + IntoNodeIdentifiers + NodeIndexable + Visitable,
    G::NodeId: Eq + Hash,
    F: FnMut(G::EdgeRef) -> K,
    K: BoundedMeasure + Copy + Sub<K, Output = K>;
```

Semantics:

```text id="34s31s"
Input:
    graph
    edge_cost closure

Output:
    Ok(HashMap<(source, target), distance>)
    Err(NegativeCycle)

Problem class:
    all-pairs shortest paths

Negative weights:
    permitted

Negative cycles:
    rejected

Graph density:
    better than Floyd-Warshall on sparse graphs
    slower on dense graphs
```

Docs state Johnson computes all-pairs shortest path lengths with positive or negative weights but no negative cycles; the implementation’s time complexity is `O(|V||E| log |V| + |V|² log |V|)`, auxiliary space is `O(|V|² + |V||E|)`, it is faster than Floyd-Warshall on sparse graphs and slower on dense graphs, and if the sparse graph is guaranteed non-negative, running Dijkstra several times is preferable. ([Docs.rs][10])

```rust id="h3ydn9"
use petgraph::algo::johnson;
use petgraph::graph::DiGraph;

let all_pairs = johnson(&g, |edge| *edge.weight())
    .expect("no negative cycle");

let d_ac = all_pairs.get(&(a, c)).copied();
```

Use when:

```text id="07qg40"
all-pairs distances needed
graph is sparse
negative weights possible
no negative cycles
NodeIndexable + Visitable available
```

Avoid when:

```text id="2xybwp"
dense graph favors Floyd-Warshall
non-negative sparse graph can use repeated Dijkstra
paths/predecessors required
memory O(V² + VE) unacceptable
```

---

## 15.12 `parallel_johnson` — rayon-parallel all-pairs Johnson

Signature:

```rust id="hxt9o3"
pub fn parallel_johnson<G, F, K>(
    graph: G,
    edge_cost: F,
) -> Result<HashMap<(G::NodeId, G::NodeId), K>, NegativeCycle>
where
    G: IntoEdges + IntoNodeIdentifiers + NodeIndexable + Visitable + Sync,
    G::NodeId: Eq + Hash + Send,
    F: Fn(G::EdgeRef) -> K + Sync,
    K: BoundedMeasure + Copy + Sub<K, Output = K> + Send + Sync;
```

Semantics:

```text id="qg5ya9"
Same problem class as johnson:
    all-pairs shortest paths

Parallelism:
    Dijkstra calls parallelized with rayon

Requires:
    rayon feature
    graph Sync
    NodeId Send
    cost closure Fn + Sync
    K Send + Sync
```

Docs state `parallel_johnson` is available under the `rayon` feature and parallelizes the Dijkstra calls; it computes all-pairs distances with positive or negative weights but no negative cycles. ([Docs.rs][11])

```rust id="hy04po"
#[cfg(feature = "rayon")]
{
    use petgraph::algo::parallel_johnson;

    let all_pairs = parallel_johnson(&g, |edge| *edge.weight())
        .expect("no negative cycle");
}
```

Use when:

```text id="k4y0l3"
all-pairs distances needed
graph is large enough for parallelism overhead
graph/cost closure are Sync
K is Send + Sync
rayon feature enabled
```

Avoid when:

```text id="8eudr4"
small graph
single-thread deterministic benchmarking desired
cost closure captures non-Sync state
non-negative sparse graph can run custom parallel Dijkstra more directly
```

---

## 15.13 `k_shortest_path` — kth shortest distance per node

Signature:

```rust id="h41efr"
pub fn k_shortest_path<G, F, K>(
    graph: G,
    start: G::NodeId,
    goal: Option<G::NodeId>,
    k: usize,
    edge_cost: F,
) -> HashMap<G::NodeId, K>
where
    G: IntoEdges + Visitable + NodeCount + NodeIndexable,
    G::NodeId: Eq + Hash,
    F: FnMut(G::EdgeRef) -> K,
    K: Measure + Copy;
```

Semantics:

```text id="ul9b3h"
Input:
    graph
    start
    optional goal
    k
    non-negative edge cost closure

Output:
    HashMap<NodeId, K>
    maps node -> kth shortest path length from start

Early exit:
    if goal = Some(node), terminates once goal's kth cost is calculated

Path:
    not returned
```

Docs state `k_shortest_path` computes the length of the kth shortest path from `start` to every reachable node, requires non-negative edge costs, supports optional goal early termination, and has `O(k|E| log(k|E|))` time and `O(|V| + k|E|)` auxiliary space. ([Docs.rs][12])

```rust id="lwe9cl"
use petgraph::algo::k_shortest_path;

let second = k_shortest_path(&g, a, None, 2, |edge| *edge.weight());

if let Some(cost) = second.get(&c) {
    println!("2nd shortest distance to c = {cost}");
}
```

Use when:

```text id="4aqkc6"
kth distance is required
non-negative weights
distance-only output is enough
alternate routes matter
```

Avoid when:

```text id="c66qpn"
actual kth path nodes required
negative weights exist
k is large and memory O(kE) is too high
all paths / path enumeration required
```

---

## 15.14 Predecessor reconstruction

### A*: built-in path output

A* directly returns `(cost, Vec<NodeId>)` when a goal is found. ([Docs.rs][5])

```rust id="t5v7mf"
let (cost, path) = astar(
    &g,
    a,
    |n| n == c,
    |edge| *edge.weight(),
    |_| 0u32,
).expect("path exists");

assert_eq!(path.first(), Some(&a));
assert_eq!(path.last(), Some(&c));
```

### Bellman-Ford / SPFA: `Paths { distances, predecessors }`

`Paths<NodeId, EdgeWeight>` contains public `distances: Vec<EdgeWeight>` and `predecessors: Vec<Option<NodeId>>`. Bellman-Ford docs say the vectors are indexed by graph node indices, and the struct docs expose those fields directly. ([Docs.rs][6])

```rust id="g2i9sp"
use petgraph::visit::NodeIndexable;

fn reconstruct_path<G>(
    graph: &G,
    predecessors: &[Option<G::NodeId>],
    source: G::NodeId,
    target: G::NodeId,
) -> Option<Vec<G::NodeId>>
where
    G: NodeIndexable,
    G::NodeId: Copy + Eq,
{
    let mut path = Vec::new();
    let mut cur = target;

    loop {
        path.push(cur);

        if cur == source {
            path.reverse();
            return Some(path);
        }

        let i = graph.to_index(cur);
        cur = predecessors.get(i).copied().flatten()?;
    }
}
```

Usage:

```rust id="4p01dh"
let paths = spfa(&g, a, |edge| edge.weight().delta)
    .expect("no negative cycle");

let path_to_c = reconstruct_path(&g, &paths.predecessors, a, c)
    .expect("reachable");
```

### Dijkstra: distance-only in petgraph API

Petgraph’s `dijkstra` returns only a `HashMap<NodeId, K>`, not predecessors. Use A* with zero heuristic for single-target path output, Bellman-Ford/SPFA when predecessors are needed, or implement a custom Dijkstra that stores predecessor links. ([Docs.rs][3])

---

## 15.15 All-pairs vs single-source vs single-pair

| Need                                                 | Prefer                   |
| ---------------------------------------------------- | ------------------------ |
| source → all reachable, non-negative                 | `dijkstra`               |
| source → one goal, non-negative, no path needed      | `bidirectional_dijkstra` |
| source → one goal, non-negative, path needed         | `astar`                  |
| source → all, negative edges, predecessors           | `bellman_ford` or `spfa` |
| reachable negative-cycle diagnostic                  | `find_negative_cycle`    |
| all pairs, dense/small graph, negative edges allowed | `floyd_warshall`         |
| all pairs, sparse graph, negative edges allowed      | `johnson`                |
| all pairs, sparse/large and parallelism helpful      | `parallel_johnson`       |
| kth distance from source                             | `k_shortest_path`        |

---

## 15.16 Best-practice decision table

| Graph/cost condition                        | Need                            | Algorithm                | Rationale                                   |
| ------------------------------------------- | ------------------------------- | ------------------------ | ------------------------------------------- |
| Non-negative weights                        | distances from one source       | `dijkstra`               | simplest single-source distance API         |
| Non-negative weights                        | one source-target cost only     | `bidirectional_dijkstra` | often less exploration on sparse long paths |
| Non-negative weights + admissible heuristic | one source-target path          | `astar`                  | returns path and cost; heuristic-guided     |
| Negative weights, no negative cycles        | source distances + predecessors | `bellman_ford`           | classical negative-weight single-source     |
| Negative weights + closure-derived costs    | source distances + predecessors | `spfa`                   | closure support; practical sparse cases     |
| Negative-cycle debugging                    | cycle path                      | `find_negative_cycle`    | returns reachable negative cycle            |
| All pairs, dense/small                      | distance matrix/map             | `floyd_warshall`         | simple O(V³), O(V²)                         |
| All pairs, sparse + negative weights        | distance map                    | `johnson`                | sparse-oriented all-pairs                   |
| All pairs, sparse + CPU parallelism         | distance map                    | `parallel_johnson`       | rayon-parallel Johnson                      |
| kth shortest distance, non-negative         | distance only                   | `k_shortest_path`        | kth distance API, not path list             |

---

## 15.17 Algorithm output matrix

| Algorithm                | Output                                                |  Predecessors |           Path nodes |     Early exit | Negative-cycle error |
| ------------------------ | ----------------------------------------------------- | ------------: | -------------------: | -------------: | -------------------: |
| `dijkstra`               | `HashMap<NodeId, K>`                                  |            No |                   No |    goal option |                   No |
| `bidirectional_dijkstra` | `Option<K>`                                           |            No |                   No |     fixed goal |                   No |
| `astar`                  | `Option<(K, Vec<NodeId>)>`                            | internal only |                  Yes | goal predicate |                   No |
| `bellman_ford`           | `Result<Paths<NodeId, EdgeWeight>, NegativeCycle>`    |           Yes | reconstruct manually |             No |                  Yes |
| `find_negative_cycle`    | `Option<Vec<NodeId>>`                                 |           N/A |  negative cycle path |            N/A |           diagnostic |
| `spfa`                   | `Result<Paths<NodeId, K>, NegativeCycle>`             |           Yes | reconstruct manually |             No |                  Yes |
| `floyd_warshall`         | `Result<HashMap<(NodeId, NodeId), K>, NegativeCycle>` |            No |                   No |             No |                  Yes |
| `johnson`                | `Result<HashMap<(NodeId, NodeId), K>, NegativeCycle>` |            No |                   No |             No |                  Yes |
| `parallel_johnson`       | same as `johnson`                                     |            No |                   No |             No |                  Yes |
| `k_shortest_path`        | `HashMap<NodeId, K>`                                  |            No |                   No |    goal option |                   No |

---

## 15.18 Trait-compatibility notes

```text id="qq1toq"
dijkstra:
    G: IntoEdges + Visitable

bidirectional_dijkstra:
    G: Visitable + IntoEdgesDirected

astar:
    G: IntoEdges + Visitable

bellman_ford:
    G: NodeCount + IntoNodeIdentifiers + IntoEdges + NodeIndexable
    EdgeWeight: FloatMeasure

find_negative_cycle:
    G: NodeCount + IntoNodeIdentifiers + IntoEdges + NodeIndexable + Visitable
    EdgeWeight: FloatMeasure

floyd_warshall:
    G: NodeCompactIndexable + IntoEdgeReferences + IntoNodeIdentifiers + GraphProp

johnson:
    G: IntoEdges + IntoNodeIdentifiers + NodeIndexable + Visitable

parallel_johnson:
    johnson bounds + Sync/Send requirements

spfa:
    G: IntoEdges + IntoNodeIdentifiers + NodeIndexable

k_shortest_path:
    G: IntoEdges + Visitable + NodeCount + NodeIndexable
```

These bounds come from the public rustdoc signatures; key deployment consequences are: `bidirectional_dijkstra` needs direction-aware edge access, `floyd_warshall` requires `NodeCompactIndexable` and therefore excludes `StableGraph`, and `parallel_johnson` requires `rayon` plus thread-safety bounds. ([Docs.rs][3])

---

## 15.19 Cost-closure recipes

### Direct numeric edge weight

```rust id="awlxds"
let dist = dijkstra(&g, start, None, |edge| *edge.weight());
```

### Struct field

```rust id="fgov6v"
let dist = dijkstra(&g, start, None, |edge| {
    edge.weight().latency_ms
});
```

### Filter by assigning large cost

```rust id="8o7d23"
let dist = dijkstra(&g, start, None, |edge| {
    if edge.weight().enabled {
        edge.weight().cost
    } else {
        u32::MAX / 4
    }
});
```

Better for hard exclusion:

```text id="kcfbql"
Use EdgeFiltered view when an edge should not exist for traversal.
Use large cost only when edge remains valid but undesirable.
```

### A* zero heuristic

```rust id="copavt"
let result = astar(
    &g,
    start,
    |n| n == goal,
    |edge| *edge.weight(),
    |_| 0u32,
);
```

Zero heuristic is the safe fallback when no admissible heuristic is known; it preserves optimality but gives up goal-directed pruning.

---

## 15.20 Deployment and performance guidance

```text id="e52ou3"
For repeated queries from one source:
    run dijkstra once
    reuse HashMap distances

For repeated one-pair queries with no heuristic:
    bidirectional_dijkstra

For repeated one-pair queries with geometry/domain heuristic:
    astar

For repeated all-pairs queries on static graph:
    precompute floyd_warshall / johnson result
    cache HashMap<(NodeId, NodeId), K>
    ensure memory budget O(V²)

For dynamic graph:
    avoid caching all-pairs across mutations unless invalidation strategy exists

For negative weights:
    validate no negative cycle
    choose bellman_ford/spfa/johnson/floyd_warshall by query shape

For parallel all-pairs:
    enable rayon
    ensure cost closure is pure/read-only/Sync
```

---

## 15.21 Anti-pattern inventory

```text id="ryjuny"
Anti-pattern:
    Use dijkstra with negative edge costs.
Problem:
    docs require non-negative edge costs.
Fix:
    bellman_ford, spfa, johnson, floyd_warshall.

Anti-pattern:
    Use astar with overestimating heuristic and expect optimality.
Problem:
    admissibility is required for actual shortest path.
Fix:
    prove heuristic <= true remaining cost; else use zero heuristic/dijkstra.

Anti-pattern:
    Use bellman_ford with domain edge struct expecting cost closure.
Problem:
    bellman_ford uses EdgeWeight directly and requires FloatMeasure.
Fix:
    use spfa with cost closure, or map graph edge weights to numeric costs.

Anti-pattern:
    Use floyd_warshall on sparse huge graph.
Problem:
    O(V³) time and O(V²) space.
Fix:
    johnson or repeated dijkstra.

Anti-pattern:
    Use johnson on dense graph by default.
Problem:
    docs say faster than Floyd on sparse, slower on dense.
Fix:
    floyd_warshall for dense/small graphs.

Anti-pattern:
    Expect dijkstra to return paths.
Problem:
    returns distance map only.
Fix:
    astar for single path, spfa/bellman_ford for predecessor vectors, or custom Dijkstra.

Anti-pattern:
    Use k_shortest_path expecting k explicit paths.
Problem:
    returns kth distance map, not path sequences.
Fix:
    implement/enlist path-enumeration algorithm if actual paths required.

Anti-pattern:
    Use parallel_johnson with mutable captured state in cost closure.
Problem:
    closure must be Fn + Sync; mutation breaks thread-safety.
Fix:
    precompute immutable data or use thread-safe read-only captures.
```

---

## 15.22 Final agent rules

```text id="gujyzx"
Single source, non-negative:
    dijkstra

Single pair, non-negative, cost only:
    bidirectional_dijkstra

Single pair, non-negative, path needed:
    astar

Single source, negative weights:
    bellman_ford if edge weight is cost
    spfa if cost closure is needed

Negative-cycle diagnostics:
    find_negative_cycle

All pairs, dense/small:
    floyd_warshall

All pairs, sparse:
    johnson

All pairs, sparse + CPU parallelism:
    parallel_johnson + rayon

Kth distance:
    k_shortest_path

Path reconstruction:
    astar returns path
    bellman_ford/spfa return predecessor vectors
    dijkstra/johnson/floyd/k_shortest return distances only
```

Final invariant:

```text id="jsd3ht"
Choose shortest-path algorithm by:
    query shape: single-pair / single-source / all-pairs / kth-distance
    weight domain: non-negative / negative / negative-cycle risk
    output need: distance / predecessor / explicit path
    graph density: sparse / dense
    deployment mode: sequential / rayon-parallel / cached precompute
```

[1]: https://docs.rs/petgraph/latest/petgraph/algo/index.html "petgraph::algo - Rust"
[2]: https://docs.rs/crate/petgraph/latest/features "petgraph 0.8.3 - Docs.rs"
[3]: https://docs.rs/petgraph/latest/petgraph/algo/dijkstra/fn.dijkstra.html "dijkstra in petgraph::algo::dijkstra - Rust"
[4]: https://docs.rs/petgraph/latest/petgraph/algo/dijkstra/fn.bidirectional_dijkstra.html "bidirectional_dijkstra in petgraph::algo::dijkstra - Rust"
[5]: https://docs.rs/petgraph/latest/petgraph/algo/astar/fn.astar.html "astar in petgraph::algo::astar - Rust"
[6]: https://docs.rs/petgraph/latest/petgraph/algo/bellman_ford/fn.bellman_ford.html "bellman_ford in petgraph::algo::bellman_ford - Rust"
[7]: https://docs.rs/petgraph/latest/petgraph/algo/bellman_ford/fn.find_negative_cycle.html "find_negative_cycle in petgraph::algo::bellman_ford - Rust"
[8]: https://docs.rs/petgraph/latest/petgraph/algo/spfa/fn.spfa.html "spfa in petgraph::algo::spfa - Rust"
[9]: https://docs.rs/petgraph/latest/petgraph/algo/floyd_warshall/fn.floyd_warshall.html "floyd_warshall in petgraph::algo::floyd_warshall - Rust"
[10]: https://docs.rs/petgraph/latest/petgraph/algo/johnson/fn.johnson.html "johnson in petgraph::algo::johnson - Rust"
[11]: https://docs.rs/petgraph/latest/petgraph/algo/johnson/fn.parallel_johnson.html "parallel_johnson in petgraph::algo::johnson - Rust"
[12]: https://docs.rs/petgraph/latest/petgraph/algo/k_shortest_path/fn.k_shortest_path.html "k_shortest_path in petgraph::algo::k_shortest_path - Rust"


# 16) Algorithm catalog — connectivity, components, cycles, DAGs

Format follows the uploaded advanced-reference style. 

`petgraph::algo` contains most petgraph algorithms; simple DFS/BFS-style traversal is in `visit`, while connectivity/cycle/SCC/topological-sort functions live in `algo`. Some algorithms are generic over graph traits, while a few still require concrete `Graph`. ([Docs.rs][1])

---

## 16.0 Core imports

```rust
use petgraph::algo::{
    connected_components,
    has_path_connecting,
    kosaraju_scc,
    tarjan_scc,
    condensation,
    is_cyclic_directed,
    is_cyclic_undirected,
    toposort,
};

use petgraph::algo::Cycle;

use petgraph::visit::{
    Topo,
    DfsPostOrder,
    DfsSpace,
    IntoNeighbors,
    IntoNeighborsDirected,
    IntoNodeIdentifiers,
    IntoEdgeReferences,
    NodeCompactIndexable,
    NodeIndexable,
    Visitable,
    EdgeRef,
};

use petgraph::acyclic::{
    Acyclic,
    TopologicalPosition,
};

use petgraph::graph::{
    Graph,
    DiGraph,
    UnGraph,
    NodeIndex,
};

use petgraph::stable_graph::{
    StableDiGraph,
    StableUnGraph,
};

use petgraph::{Directed, Undirected};
```

---

## 16.1 Decision table

| Need                                     | Use                    | Output                               | Cycle handling                    |
| ---------------------------------------- | ---------------------- | ------------------------------------ | --------------------------------- |
| Count weak components                    | `connected_components` | `usize`                              | treats directed graph weakly      |
| Check reachability                       | `has_path_connecting`  | `bool`                               | no cycle report                   |
| Strongly connected components, iterative | `kosaraju_scc`         | `Vec<Vec<NodeId>>`                   | SCCs expose cycles/groups         |
| Strongly connected components, recursive | `tarjan_scc`           | `Vec<Vec<NodeId>>`                   | SCCs expose cycles/groups         |
| Collapse SCCs into component graph       | `condensation`         | `Graph<Vec<N>, E, Ty, Ix>`           | optional DAG cleanup              |
| Directed cycle predicate                 | `is_cyclic_directed`   | `bool`                               | recursive                         |
| Undirected cycle predicate               | `is_cyclic_undirected` | `bool`                               | always treats graph as undirected |
| Full topological order                   | `toposort`             | `Result<Vec<NodeId>, Cycle<NodeId>>` | `Err(Cycle)`                      |
| Streaming topological traversal          | `Topo`                 | iterator-like traversal              | skips nodes in cycles             |
| Enforce DAG invariant during mutation    | `acyclic::Acyclic`     | wrapper graph                        | rejects cycle-creating edges      |

---

## 16.2 `connected_components` — weak connected component count

Signature:

```rust
pub fn connected_components<G>(g: G) -> usize
where
    G: NodeCompactIndexable + IntoEdgeReferences;
```

`connected_components` returns the number of connected components; for directed graphs, it returns the number of **weakly** connected components, meaning edge direction is ignored for component grouping. Complexity is amortized `O(|E| + |V| log |V|)` time and `O(|V|)` auxiliary space. ([Docs.rs][2])

```rust
use petgraph::algo::connected_components;
use petgraph::graph::UnGraph;

let g = UnGraph::<(), ()>::from_edges([
    (0, 1),
    (1, 2),
    (3, 4),
]);

assert_eq!(connected_components(&g), 2);
```

Directed weak-component example:

```rust
use petgraph::algo::connected_components;
use petgraph::graph::DiGraph;

let g = DiGraph::<(), ()>::from_edges([
    (0, 1),
    (2, 1), // direction ignored for weak component count
    (3, 4),
]);

assert_eq!(connected_components(&g), 2);
```

Use when:

```text
need only component count
weak connectivity is sufficient
graph has compact node IDs
no component membership vectors needed
```

Avoid when:

```text
need actual component membership
need strongly connected components
using StableGraph with holes, because NodeCompactIndexable is required
```

---

## 16.3 `has_path_connecting` — reachability predicate

Signature:

```rust
pub fn has_path_connecting<G>(
    g: G,
    from: G::NodeId,
    to: G::NodeId,
    space: Option<&mut DfsSpace<G::NodeId, G::Map>>,
) -> bool
where
    G: IntoNeighbors + Visitable;
```

`has_path_connecting` checks whether a path exists from `from` to `to`; if both node IDs are equal, it returns `true`. An optional `DfsSpace` can be passed to reuse traversal workspace instead of allocating fresh state. ([Docs.rs][3])

```rust
use petgraph::algo::has_path_connecting;
use petgraph::graph::DiGraph;

let g = DiGraph::<(), ()>::from_edges([
    (0, 1),
    (1, 2),
]);

let a = NodeIndex::new(0);
let c = NodeIndex::new(2);

assert!(has_path_connecting(&g, a, c, None));
assert!(!has_path_connecting(&g, c, a, None));
```

Workspace reuse:

```rust
use petgraph::algo::{has_path_connecting, DfsSpace};

let mut space = DfsSpace::new(&g);

for (from, to) in [(a, c), (c, a)] {
    let reachable = has_path_connecting(&g, from, to, Some(&mut space));
    println!("{from:?} -> {to:?}: {reachable}");
}
```

Use when:

```text
boolean reachability is enough
single source-target query
allocation reuse matters across many queries
edge weights/costs irrelevant
```

Avoid when:

```text
path nodes are required
distance/cost required
all reachable nodes required
cycle diagnostics required
```

---

## 16.4 `kosaraju_scc` — strongly connected components, iterative two-pass

Signature:

```rust
pub fn kosaraju_scc<G>(g: G) -> Vec<Vec<G::NodeId>>
where
    G: IntoNeighborsDirected + Visitable + IntoNodeIdentifiers;
```

`kosaraju_scc` computes strongly connected components using Kosaraju’s algorithm. It returns `Vec<Vec<NodeId>>`; node order inside each SCC is arbitrary, while SCC order is postorder / reverse topological sort. For undirected graphs, SCCs are simply connected components. The implementation is iterative and performs two passes over nodes. ([Docs.rs][4])

```rust
use petgraph::algo::kosaraju_scc;
use petgraph::graph::DiGraph;

let mut g = DiGraph::<&str, ()>::new();

let a = g.add_node("a");
let b = g.add_node("b");
let c = g.add_node("c");
let d = g.add_node("d");

g.add_edge(a, b, ());
g.add_edge(b, a, ());
g.add_edge(b, c, ());
g.add_edge(c, d, ());
g.add_edge(d, c, ());

let sccs = kosaraju_scc(&g);

for component in &sccs {
    println!("{component:?}");
}
```

Use when:

```text
need SCC membership
prefer iterative SCC implementation
graph supports IntoNeighborsDirected
want SCCs in reverse-topological component order
stack-depth safety matters
```

Avoid when:

```text
graph type lacks IntoNeighborsDirected
need one-pass recursive Tarjan behavior
need immediate callback streaming of SCCs
```

---

## 16.5 `tarjan_scc` — strongly connected components, recursive one-pass

Signature:

```rust
pub fn tarjan_scc<G>(g: G) -> Vec<Vec<G::NodeId>>
where
    G: IntoNodeIdentifiers + IntoNeighbors + NodeIndexable;
```

`tarjan_scc` computes strongly connected components using Tarjan’s algorithm. It returns `Vec<Vec<NodeId>>`; node order inside each component is arbitrary, while SCC order is postorder / reverse topological sort. For an undirected graph, SCCs are connected components. The implementation is recursive and performs one pass over nodes. ([Docs.rs][5])

```rust
use petgraph::algo::tarjan_scc;
use petgraph::graph::DiGraph;

let sccs = tarjan_scc(&g);

let cyclic_components: Vec<_> = sccs
    .iter()
    .filter(|component| component.len() > 1)
    .collect();
```

Self-loop-aware cyclic SCC classification:

```rust
use petgraph::visit::EdgeRef;

fn component_has_cycle<N, E>(
    g: &DiGraph<N, E>,
    component: &[NodeIndex],
) -> bool {
    if component.len() > 1 {
        return true;
    }

    let n = component[0];

    g.edges(n).any(|e| e.target() == n)
}
```

Use when:

```text
need SCC membership
graph has NodeIndexable support
one-pass recursive SCC is acceptable
component order should be reverse-topological
```

Avoid when:

```text
very deep graph may risk recursion depth
graph type supports Kosaraju bounds more naturally
need non-recursive implementation
```

---

## 16.6 SCC interpretation rules

```text
Strongly connected component:
    maximal set of nodes where every node can reach every other node

DAG condition:
    directed graph is acyclic iff every SCC has size 1 and no self-loop

Cycle-relevant SCC:
    component.len() > 1
    OR single node has self-loop

Condensation:
    each SCC becomes one node
    resulting simple SCC graph is a DAG if self-loops/multiedges are removed
```

Use SCCs for:

```text
cycle grouping
module/package dependency collapse
compiler strongly connected module groups
workflow loop detection
service dependency strongly connected clusters
deadlock/circular-reference analysis
```

---

## 16.7 `condensation` — collapse SCCs into component nodes

Signature:

```rust
pub fn condensation<N, E, Ty, Ix>(
    g: Graph<N, E, Ty, Ix>,
    make_acyclic: bool,
) -> Graph<Vec<N>, E, Ty, Ix>
where
    Ty: EdgeType,
    Ix: IndexType;
```

`condensation` consumes a concrete `Graph`, condenses every strongly connected component into a single node, and returns `Graph<Vec<N>, E, Ty, Ix>`. Each output node weight is a `Vec<N>` containing original node weights for that SCC. If `make_acyclic` is `true`, self-loops and multi-edges are ignored, guaranteeing an acyclic output. Complexity is `O(|V| + |E|)` time and `O(|V| + |E|)` auxiliary space. ([Docs.rs][6])

```rust
use petgraph::algo::condensation;
use petgraph::graph::DiGraph;

let mut g = DiGraph::<&str, &str>::new();

let a = g.add_node("a");
let b = g.add_node("b");
let c = g.add_node("c");

g.add_edge(a, b, "a->b");
g.add_edge(b, a, "b->a");
g.add_edge(b, c, "b->c");

let condensed = condensation(g, true);

for component_weight in condensed.node_weights() {
    println!("SCC: {component_weight:?}");
}
```

`make_acyclic` policy:

```text
make_acyclic = false:
    preserves self-loops / multiedges created by SCC collapse
    useful for diagnostics and edge provenance counts

make_acyclic = true:
    removes self-loops and duplicate inter-component edges
    output guaranteed acyclic
    useful for DAG pipeline after SCC collapse
```

Important constraint:

```text
condensation requires concrete Graph<N,E,Ty,Ix>
not generic G
consumes the input graph
moves node weights into Vec<N>
moves edge weights into output edges
```

SCC condensation workflow:

```rust
use petgraph::algo::{condensation, toposort};
use petgraph::graph::DiGraph;

fn component_dag<N, E>(
    g: DiGraph<N, E>,
) -> DiGraph<Vec<N>, E> {
    condensation(g, true)
}

let dag = component_dag(g);
let order = toposort(&dag, None).expect("make_acyclic=true produces DAG");
```

---

## 16.8 `is_cyclic_directed` — directed cycle predicate

Signature:

```rust
pub fn is_cyclic_directed<G>(g: G) -> bool
where
    G: IntoNodeIdentifiers + IntoNeighbors + Visitable;
```

`is_cyclic_directed` returns `true` if a directed graph contains a cycle. The implementation is recursive; docs recommend `toposort` as an alternative if recursion is undesirable. Complexity is `O(|V| + |E|)` time and `O(|V|)` auxiliary space. ([Docs.rs][7])

```rust
use petgraph::algo::is_cyclic_directed;
use petgraph::graph::DiGraph;

let cyclic = DiGraph::<(), ()>::from_edges([
    (0, 1),
    (1, 2),
    (2, 0),
]);

assert!(is_cyclic_directed(&cyclic));

let dag = DiGraph::<(), ()>::from_edges([
    (0, 1),
    (1, 2),
]);

assert!(!is_cyclic_directed(&dag));
```

Use when:

```text
boolean directed-cycle check is enough
recursive implementation acceptable
no cycle witness needed
no topological order needed
```

Prefer `toposort` when:

```text
need topological order if acyclic
need iterative implementation
need Cycle<NodeId> error witness
```

---

## 16.9 `is_cyclic_undirected` — undirected cycle predicate

Signature:

```rust
pub fn is_cyclic_undirected<G>(g: G) -> bool
where
    G: NodeIndexable + IntoEdgeReferences;
```

`is_cyclic_undirected` returns `true` if the input graph contains a cycle and always treats the input graph as undirected, even if the graph type is directed. Complexity is amortized `O(|E|)` time and `O(|V|)` auxiliary space. ([Docs.rs][8])

```rust
use petgraph::algo::is_cyclic_undirected;
use petgraph::graph::UnGraph;

let tree = UnGraph::<(), ()>::from_edges([
    (0, 1),
    (1, 2),
    (1, 3),
]);

assert!(!is_cyclic_undirected(&tree));

let cycle = UnGraph::<(), ()>::from_edges([
    (0, 1),
    (1, 2),
    (2, 0),
]);

assert!(is_cyclic_undirected(&cycle));
```

Directed input still treated as undirected:

```rust
use petgraph::algo::is_cyclic_undirected;
use petgraph::graph::DiGraph;

let directed = DiGraph::<(), ()>::from_edges([
    (0, 1),
    (2, 1),
    (2, 0),
]);

assert!(is_cyclic_undirected(&directed));
```

Use when:

```text
cycle means undirected graph-theoretic cycle
edge orientation must be ignored
spanning-tree / forest validation
physical-network cycle detection
```

Avoid when:

```text
directed feedback loop semantics matter
DAG validation needed
topological sort needed
```

---

## 16.10 `toposort` — full topological order or cycle error

Signature:

```rust
pub fn toposort<G>(
    g: G,
    space: Option<&mut DfsSpace<G::NodeId, G::Map>>,
) -> Result<Vec<G::NodeId>, Cycle<G::NodeId>>
where
    G: IntoNeighborsDirected + IntoNodeIdentifiers + Visitable;
```

`toposort` performs an iterative topological sort of a directed graph. On success, it returns a vector of nodes in topological order where every node appears before its successors. On failure, it returns `Err(Cycle<NodeId>)`; self-loops are cycles. Optional `DfsSpace` can reuse traversal workspace. Complexity is `O(|V| + |E|)` time and `O(|V|)` auxiliary space. ([Docs.rs][9])

```rust
use petgraph::algo::toposort;
use petgraph::graph::DiGraph;

let mut g = DiGraph::<&str, ()>::new();

let parse = g.add_node("parse");
let typecheck = g.add_node("typecheck");
let codegen = g.add_node("codegen");

g.add_edge(parse, typecheck, ());
g.add_edge(typecheck, codegen, ());

let order = toposort(&g, None).expect("DAG expected");

let labels: Vec<_> = order.iter().map(|&n| g[n]).collect();

assert_eq!(labels, vec!["parse", "typecheck", "codegen"]);
```

Cycle handling:

```rust
use petgraph::algo::{toposort, Cycle};

match toposort(&g, None) {
    Ok(order) => {
        println!("topological order: {order:?}");
    }
    Err(cycle) => {
        let node = cycle.node_id();
        println!("cycle detected near node {node:?}");
    }
}
```

Workspace reuse:

```rust
use petgraph::algo::{toposort, DfsSpace};

let mut space = DfsSpace::new(&g);

for _ in 0..10 {
    let order = toposort(&g, Some(&mut space));
    println!("{order:?}");
}
```

Use when:

```text
need full DAG ordering
need cycle error instead of boolean
need iterative cycle check
need self-loop detection as cycle
```

Avoid when:

```text
graph may be cyclic and partial order over cyclic groups is acceptable
SCC grouping is required
streaming traversal enough via Topo
```

---

## 16.11 `Topo` — streaming topological traversal

`Topo` is a topological-order traversal visitor. It only visits nodes that are not part of cycles, so it is appropriate for true DAG traversal; for graphs with possible cycles, docs recommend `DfsPostOrder` or SCC algorithms such as `kosaraju_scc`. ([Docs.rs][10])

```rust
use petgraph::visit::Topo;
use petgraph::graph::DiGraph;

let mut topo = Topo::new(&g);

while let Some(node) = topo.next(&g) {
    println!("ready: {}", g[node]);
}
```

Detect incomplete traversal:

```rust
use petgraph::visit::Topo;

let mut topo = Topo::new(&g);
let mut visited = 0usize;

while let Some(_node) = topo.next(&g) {
    visited += 1;
}

if visited != g.node_count() {
    println!("cycle or non-DAG region detected");
}
```

Use `Topo` when:

```text
need streaming/pull-based topological order
graph is expected to be DAG
want traversal object rather than Vec<NodeId>
payload mutation between steps is useful
```

Prefer `toposort` when:

```text
need all-or-error result
must detect cycle explicitly
want Vec<NodeId>
want Cycle<NodeId> error
```

---

## 16.12 DAG modeling conventions

Two common conventions:

```text
Convention A: dependent -> prerequisite
    outgoing(node) = prerequisites
    incoming(node) = dependents
    toposort order may list dependents before prerequisites depending edge semantics

Convention B: prerequisite -> dependent
    outgoing(node) = unlocked dependents
    incoming(node) = prerequisites
    toposort order naturally gives build/execution order
```

Build-order graph: prerequisite → dependent.

```rust
use petgraph::graph::DiGraph;
use petgraph::algo::toposort;

let mut g = DiGraph::<&str, ()>::new();

let parse = g.add_node("parse");
let typecheck = g.add_node("typecheck");
let codegen = g.add_node("codegen");

g.add_edge(parse, typecheck, ());
g.add_edge(typecheck, codegen, ());

let order = toposort(&g, None).unwrap();
let names: Vec<_> = order.into_iter().map(|n| g[n]).collect();

assert_eq!(names, vec!["parse", "typecheck", "codegen"]);
```

Dependency-query graph: dependent → prerequisite.

```rust
use petgraph::Direction::{Incoming, Outgoing};

let prerequisites: Vec<_> = g.neighbors_directed(package, Outgoing).collect();
let dependents: Vec<_> = g.neighbors_directed(package, Incoming).collect();
```

Agent rule:

```text
Freeze edge direction in type alias/docs.
Name helper functions by semantics:
    prerequisites_of
    dependents_of
    successors_of
    predecessors_of
Do not expose raw incoming/outgoing in domain APIs unless graph orientation is obvious.
```

---

## 16.13 `acyclic::Acyclic` — runtime-enforced DAG wrapper

`Acyclic<G>` wraps a directed acyclic graph and exposes an API that maintains the acyclicity invariant: no edge addition can create a cycle. It dynamically maintains topological order when edges are added using the Pierce-Kelly dynamic topological sort algorithm; worst case can be linear in vertex count, but docs describe it as fast in practice, especially on sparse graphs. Good inner graph candidates are `DiGraph` and `StableDiGraph`. ([Docs.rs][11])

```rust
use petgraph::acyclic::Acyclic;
use petgraph::data::Build;
use petgraph::graph::DiGraph;

let mut dag: Acyclic<DiGraph<&str, ()>> = Acyclic::new();

let parse = dag.add_node("parse");
let typecheck = dag.add_node("typecheck");

let edge = dag
    .try_add_edge(parse, typecheck, ())
    .expect("edge does not create cycle");
```

Cycle-rejecting edge insertion:

```rust
let result = dag.try_add_edge(typecheck, parse, ());

assert!(result.is_err());
```

`Acyclic::try_add_edge` returns the added edge ID or `AcyclicEdgeError` if the edge would create a cycle, a self-loop, or the underlying graph addition failed. `try_update_edge` adds or updates while rejecting cycle/self-loop creation. Docs recommend the dedicated `try_add_edge` / `try_update_edge` methods because generic `Build::update_edge` panics if a cycle would be created, while generic `Build::add_edge` returns `None` when edge addition cannot occur. ([Docs.rs][11])

Wrapping an existing graph:

```rust
use petgraph::acyclic::Acyclic;
use petgraph::graph::DiGraph;

let graph = DiGraph::<&str, ()>::from_edges([
    (0, 1),
    (1, 2),
]);

let dag = Acyclic::try_from_graph(graph)
    .expect("input graph is acyclic");
```

`Acyclic::try_from_graph` wraps a graph into an acyclic graph, returning a `Cycle<NodeId>` if the graph is cyclic. `DiGraph` and `StableDiGraph` also implement `TryFrom` for this wrapper. ([Docs.rs][11])

Topological position API:

```rust
let pos = dag.get_position(parse);
let node = dag.at_position(pos);
let ordered_nodes: Vec<_> = dag.nodes_iter().collect();
```

`Acyclic` exposes `get_position`, `at_position`, `nodes_iter`, `range`, `inner`, and `into_inner`; `TopologicalPosition` defines a total order over nodes, and positions are not necessarily contiguous. ([Docs.rs][11])

---

## 16.14 `Acyclic` vs `toposort`

| Requirement                                          | Prefer                                        |
| ---------------------------------------------------- | --------------------------------------------- |
| One-time validation of static graph                  | `toposort` or `Acyclic::try_from_graph`       |
| Need topological order vector                        | `toposort`                                    |
| Need streaming topological traversal                 | `Topo`                                        |
| Need graph API that rejects cycle-creating mutations | `Acyclic<G>`                                  |
| Need long-lived editable DAG with stable handles     | `Acyclic<StableDiGraph<N,E>>`                 |
| Need simple non-enforcing graph + occasional checks  | `DiGraph` + `toposort` / `is_cyclic_directed` |

Pattern: enforcing build API.

```rust
use petgraph::acyclic::Acyclic;
use petgraph::data::Build;
use petgraph::graph::DiGraph;

#[derive(Debug)]
enum DagBuildError<N> {
    Cycle(N),
}

fn add_dependency<N, E>(
    dag: &mut Acyclic<DiGraph<N, E>>,
    prerequisite: NodeIndex,
    dependent: NodeIndex,
    edge: E,
) -> Result<(), String> {
    dag.try_add_edge(prerequisite, dependent, edge)
        .map(|_| ())
        .map_err(|e| format!("edge would violate DAG invariant: {e:?}"))
}
```

---

## 16.15 SCC condensation workflow

Workflow:

```text
directed graph
    -> kosaraju_scc / tarjan_scc for component membership
    -> condensation(graph, make_acyclic)
    -> toposort(condensed_dag)
    -> process SCC groups in dependency order
```

Concrete:

```rust
use petgraph::algo::{condensation, kosaraju_scc, toposort};
use petgraph::graph::DiGraph;

let sccs = kosaraju_scc(&g);

let condensed = condensation(g, true);

let order = toposort(&condensed, None)
    .expect("condensation(..., true) removes self-loops/multiedges and yields DAG-like output");

for component_node in order {
    let original_node_weights = &condensed[component_node];
    println!("process SCC: {original_node_weights:?}");
}
```

Interpretation:

```text
SCC node weight:
    Vec<N>
    all original node weights inside one strongly connected component

Condensed edge:
    original edge payload E moved to inter-component edge

make_acyclic=true:
    suitable for toposort / DAG scheduling

make_acyclic=false:
    suitable for diagnostics preserving self-loop/multiedge evidence
```

`condensation` consumes concrete `Graph<N,E,Ty,Ix>`, returns `Graph<Vec<N>,E,Ty,Ix>`, and with `make_acyclic=true` ignores self-loops and multi-edges to guarantee acyclicity. ([Docs.rs][6])

---

## 16.16 Cycle error handling

`toposort` returns `Result<Vec<NodeId>, Cycle<NodeId>>`; docs state self-loops are also cycles. `Cycle<NodeId>` is the error carrier used by topological sorting and acyclic wrapping APIs. ([Docs.rs][9])

```rust
use petgraph::algo::toposort;

fn topo_or_cycle<N, E>(
    g: &DiGraph<N, E>,
) -> Result<Vec<NodeIndex>, NodeIndex> {
    match toposort(g, None) {
        Ok(order) => Ok(order),
        Err(cycle) => Err(cycle.node_id()),
    }
}
```

Cycle-response patterns:

```text
fail-fast build:
    return error immediately

diagnostic grouping:
    run kosaraju_scc / tarjan_scc
    report SCCs with len > 1 or self-loop

safe scheduling:
    condensation(graph, true)
    schedule component DAG instead of individual nodes

interactive editor:
    wrap graph in Acyclic<StableDiGraph<...>>
    reject invalid edge before graph state changes
```

---

## 16.17 Topological position and incremental DAG metadata

`Acyclic::get_position(node)` returns a `TopologicalPosition`; `nodes_iter()` returns nodes ordered by position, and `range(..)` returns ordered nodes within a position range. `TopologicalPosition` defines a total order over graph nodes but positions may not form a contiguous interval. ([Docs.rs][11])

```rust
use petgraph::acyclic::{Acyclic, TopologicalPosition};
use petgraph::data::Build;
use petgraph::graph::DiGraph;

let mut dag: Acyclic<DiGraph<&str, ()>> = Acyclic::new();

let a = dag.add_node("a");
let b = dag.add_node("b");

dag.try_add_edge(a, b, ()).unwrap();

let pos_a = dag.get_position(a);
let pos_b = dag.get_position(b);

assert!(pos_a < pos_b);

let ordered: Vec<_> = dag.nodes_iter().collect();
```

Use topological positions for:

```text
incremental scheduling
DAG UI layout
partial-range processing
stable topological comparisons
online DAG mutation validation
```

Caveat:

```text
TopologicalPosition is an ordering token.
Do not assume positions are dense integers.
Use nodes_iter/range rather than manual numeric iteration.
```

---

## 16.18 Algorithm trait-bound matrix

| Function/type          | Key bounds / concrete type requirement                           |
| ---------------------- | ---------------------------------------------------------------- |
| `connected_components` | `NodeCompactIndexable + IntoEdgeReferences`                      |
| `has_path_connecting`  | `IntoNeighbors + Visitable`                                      |
| `kosaraju_scc`         | `IntoNeighborsDirected + Visitable + IntoNodeIdentifiers`        |
| `tarjan_scc`           | `IntoNodeIdentifiers + IntoNeighbors + NodeIndexable`            |
| `condensation`         | concrete `Graph<N,E,Ty,Ix>`                                      |
| `is_cyclic_directed`   | `IntoNodeIdentifiers + IntoNeighbors + Visitable`                |
| `is_cyclic_undirected` | `NodeIndexable + IntoEdgeReferences`                             |
| `toposort`             | `IntoNeighborsDirected + IntoNodeIdentifiers + Visitable`        |
| `Topo`                 | `IntoNodeIdentifiers + IntoNeighborsDirected + Visitable`        |
| `Acyclic<G>`           | `G: Visitable`; mutation requires `Build` and node-index support |

`visit` docs list graph traversal traits and implementation coverage; they also state that visitors minimally require `GraphBase`, `IntoNeighbors`, and `Visitable`, and that topological visitor/traversal types use walker-style methods that borrow the graph only per step. ([Docs.rs][12])

---

## 16.19 DAG production recipes

### Recipe A: validate static DAG

```rust
use petgraph::algo::toposort;

let order = toposort(&g, None)
    .map_err(|cycle| format!("cycle near {:?}", cycle.node_id()))?;
```

### Recipe B: enforce DAG at mutation time

```rust
use petgraph::acyclic::Acyclic;
use petgraph::data::Build;
use petgraph::stable_graph::StableDiGraph;

type EditableDag<N, E> = Acyclic<StableDiGraph<N, E>>;

let mut dag: EditableDag<&str, ()> = Acyclic::new();

let a = dag.add_node("a");
let b = dag.add_node("b");

dag.try_add_edge(a, b, ())?;
```

### Recipe C: accept cycles but schedule SCC groups

```rust
use petgraph::algo::{condensation, toposort};

let component_graph = condensation(g, true);
let component_order = toposort(&component_graph, None).unwrap();

for component in component_order {
    let members = &component_graph[component];
    process_strongly_connected_group(members);
}
```

### Recipe D: boolean check before expensive work

```rust
use petgraph::algo::is_cyclic_directed;

if is_cyclic_directed(&g) {
    return Err("dependency graph contains a cycle");
}
```

---

## 16.20 Best-practice decision table

| Workload                               | Recommended API                                | Reason                         |
| -------------------------------------- | ---------------------------------------------- | ------------------------------ |
| Count weak components                  | `connected_components`                         | no membership allocation       |
| Need reachability bool                 | `has_path_connecting`                          | cheap DFS predicate            |
| Need all SCC groups, iterative         | `kosaraju_scc`                                 | no recursion                   |
| Need all SCC groups, one pass          | `tarjan_scc`                                   | one-pass recursive             |
| Need DAG from cyclic directed graph    | `condensation(..., true)`                      | SCC collapse + acyclic cleanup |
| Need cycle bool only                   | `is_cyclic_directed` / `is_cyclic_undirected`  | direct predicate               |
| Need topological order or error        | `toposort`                                     | full order + `Cycle` error     |
| Need pull-based topo traversal         | `Topo`                                         | streaming visitor              |
| Need invariant-preserving editable DAG | `Acyclic<DiGraph>` / `Acyclic<StableDiGraph>`  | rejects invalid edges          |
| Need DAG UI/order positions            | `Acyclic::get_position`, `nodes_iter`, `range` | dynamic topological order      |

---

## 16.21 Anti-pattern inventory

```text
Anti-pattern:
    Use connected_components expecting strong components in directed graph.
Problem:
    connected_components returns weak components for directed graphs.
Fix:
    kosaraju_scc or tarjan_scc.

Anti-pattern:
    Use is_cyclic_undirected for DAG validation.
Problem:
    it ignores edge direction.
Fix:
    is_cyclic_directed or toposort.

Anti-pattern:
    Use Topo and assume it detects/report cycles.
Problem:
    Topo only visits nodes not part of cycles.
Fix:
    count visited nodes or use toposort.

Anti-pattern:
    Use toposort on cyclic graph and discard Err.
Problem:
    no valid node-level topological order exists.
Fix:
    run SCC algorithm or condensation.

Anti-pattern:
    Run condensation expecting generic graph support.
Problem:
    condensation consumes concrete Graph<N,E,Ty,Ix>.
Fix:
    convert/materialize to Graph first if needed.

Anti-pattern:
    Use Acyclic but mutate inner graph directly.
Problem:
    bypasses acyclicity invariant.
Fix:
    mutate through Acyclic API; use inner() read-only or into_inner only at boundary.

Anti-pattern:
    Use Acyclic::Build::update_edge and expect Result.
Problem:
    docs state generic Build::update_edge panics on cycle creation.
Fix:
    use try_add_edge / try_update_edge.

Anti-pattern:
    Assume topological positions are contiguous numeric ranks.
Problem:
    TopologicalPosition total order need not be contiguous.
Fix:
    use nodes_iter/range/at_position.
```

---

## 16.22 Deployment checklist

```text
Connectivity:
    connected_components for weak count
    SCC algorithms for directed cycle groups
    has_path_connecting for boolean reachability

Cycle validation:
    is_cyclic_directed for quick recursive bool
    toposort for iterative order-or-error
    SCC for rich diagnostics
    Acyclic for mutation-time invariant enforcement

DAG modeling:
    choose edge orientation explicitly
    prerequisite -> dependent for natural build order
    dependent -> prerequisite for dependency lookup ergonomics
    document incoming/outgoing semantics

SCC workflow:
    find SCCs with kosaraju_scc/tarjan_scc
    classify cyclic SCCs
    optionally condense graph
    toposort condensed DAG
    process SCC groups in topological order

Production:
    use DfsSpace reuse in hot repeated toposort/has_path checks
    prefer StableDiGraph inside Acyclic when UI/API handles survive deletion
    never expose raw topological position as dense integer
    preserve original domain IDs in node weights before condensation
```

Final rule:

```text
Connectivity APIs answer “can/are connected?”
SCC APIs answer “which nodes mutually depend?”
Cycle APIs answer “is this graph cyclic?”
Topological APIs answer “what order is valid if acyclic?”
Acyclic wrapper answers “how do we prevent cycles by construction?”
```

[1]: https://docs.rs/petgraph/latest/petgraph/algo/index.html "petgraph::algo - Rust"
[2]: https://docs.rs/petgraph/latest/petgraph/algo/fn.connected_components.html?utm_source=chatgpt.com "connected_components in petgraph::algo - Rust"
[3]: https://docs.rs/petgraph/latest/petgraph/algo/fn.has_path_connecting.html?utm_source=chatgpt.com "has_path_connecting in petgraph::algo - Rust"
[4]: https://docs.rs/nopetgraph/latest/petgraph/algo/fn.kosaraju_scc.html?utm_source=chatgpt.com "kosaraju_scc in petgraph::algo - Rust"
[5]: https://docs.rs/nopetgraph/latest/petgraph/algo/fn.tarjan_scc.html?utm_source=chatgpt.com "tarjan_scc in petgraph::algo - Rust"
[6]: https://docs.rs/petgraph/latest/petgraph/algo/fn.condensation.html "condensation in petgraph::algo - Rust"
[7]: https://docs.rs/petgraph/latest/petgraph/algo/fn.is_cyclic_directed.html "is_cyclic_directed in petgraph::algo - Rust"
[8]: https://docs.rs/petgraph/latest/petgraph/algo/fn.is_cyclic_undirected.html "is_cyclic_undirected in petgraph::algo - Rust"
[9]: https://docs.rs/petgraph/latest/petgraph/algo/fn.toposort.html "toposort in petgraph::algo - Rust"
[10]: https://docs.rs/petgraph/latest/petgraph/visit/struct.Topo.html?utm_source=chatgpt.com "Topo in petgraph::visit - Rust"
[11]: https://docs.rs/petgraph/latest/petgraph/acyclic/struct.Acyclic.html "Acyclic in petgraph::acyclic - Rust"
[12]: https://docs.rs/petgraph/latest/petgraph/visit/index.html "petgraph::visit - Rust"


# 17) Algorithm catalog — spanning trees, cuts, bridges, articulation

Format follows the uploaded advanced-reference style. 

Target: **petgraph 0.8.3 released API**. This section covers minimum spanning tree/forest algorithms, bridge detection, articulation-point detection, and `UnionFind` / disjoint-set patterns.

---

## 17.0 Core imports

```rust
use petgraph::algo::{
    min_spanning_tree,
    min_spanning_tree_prim,
};

use petgraph::algo::bridges::bridges;
use petgraph::algo::articulation_points::articulation_points;

use petgraph::data::FromElements;

use petgraph::graph::{
    Graph,
    UnGraph,
    DiGraph,
    NodeIndex,
    EdgeIndex,
};

use petgraph::unionfind::UnionFind;

use petgraph::visit::{
    EdgeRef,
    IntoEdgeReferences,
    IntoNodeReferences,
    NodeIndexable,
};
```

`min_spanning_tree` and `min_spanning_tree_prim` are re-exported from `petgraph::algo`, while `bridges` and `articulation_points` live in their submodules and are also part of the `algo` module inventory. ([Docs.rs][1])

---

## 17.1 Algorithm selection table

| Need                                                     | Use                                | Graph model                         | Output                       |
| -------------------------------------------------------- | ---------------------------------- | ----------------------------------- | ---------------------------- |
| Minimum spanning forest over possibly disconnected graph | `min_spanning_tree`                | treated as undirected               | iterator of `Element<N,E>`   |
| Minimum spanning tree over connected graph               | `min_spanning_tree_prim`           | treated as undirected and connected | iterator of `Element<N,E>`   |
| Find critical edges in simple undirected graph           | `bridges`                          | simple undirected                   | iterator of edge references  |
| Find critical vertices                                   | `articulation_points`              | graph connectivity cut vertices     | `HashSet<NodeId>`            |
| Maintain connected components manually                   | `UnionFind`                        | dense integer elements `0..n`       | disjoint-set representatives |
| Build MST graph value                                    | `FromElements::from_elements(...)` | output element stream               | `Graph` / `UnGraph`          |

`min_spanning_tree` uses Kruskal’s algorithm, treats input as undirected, returns a minimum spanning forest, and is designed to be consumed by `FromElements`; `min_spanning_tree_prim` uses Prim’s algorithm, treats input as undirected and connected, and can be wrong if that connected/undirected assumption is false. ([Docs.rs][2])

---

## 17.2 Edge-weight requirements and ordering

### MST weight constraints

```text
min_spanning_tree:
    NodeWeight: Clone
    EdgeWeight: Clone + PartialOrd
    Graph: IntoNodeReferences + IntoEdgeReferences + NodeIndexable

min_spanning_tree_prim:
    EdgeWeight: PartialOrd
    Graph: IntoNodeReferences + IntoEdgeReferences
```

The `min_spanning_tree` signature requires cloned node and edge weights plus `PartialOrd` for edge ordering; `min_spanning_tree_prim` requires only `EdgeWeight: PartialOrd` and the graph reference traits listed by its signature. ([Docs.rs][2])

### Practical ordering hazards

```text
PartialOrd:
    accepts f32/f64
    NaN can make comparisons non-total
    MST behavior with incomparable values is hazardous

Recommended production edge weights:
    integer costs: u32, u64, i64
    fixed-point decimal wrapper
    ordered-float wrapper if floats are required
    domain struct with extracted numeric MST graph
```

Bad float pattern:

```rust
use petgraph::graph::UnGraph;
use petgraph::algo::min_spanning_tree;
use petgraph::data::FromElements;

let mut g = UnGraph::<(), f64>::new_undirected();

// Avoid NaN in MST edge weights.
g.extend_with_edges([
    (0, 1, 1.0),
    (1, 2, f64::NAN),
]);

let _mst = UnGraph::<_, _>::from_elements(min_spanning_tree(&g));
```

Safer integer-cost projection:

```rust
use petgraph::graph::UnGraph;

#[derive(Clone, Debug)]
struct Link {
    distance_m: u32,
    capacity_mbps: u32,
}

let mut domain = UnGraph::<&str, Link>::new_undirected();

let cost_graph: UnGraph<&str, u32> = domain.map(
    |_node_ix, node| *node,
    |_edge_ix, link| link.distance_m,
);
```

Agent rule:

```text
For MST:
    edge payload must be orderable by desired optimization metric.
If domain edge has many fields:
    project to a numeric/orderable edge-weight graph first,
    or make E itself encode the chosen ordering.
```

---

## 17.3 `min_spanning_tree` — Kruskal minimum spanning forest

Signature shape:

```rust
pub fn min_spanning_tree<G>(g: G) -> MinSpanningTree<G>
where
    G::NodeWeight: Clone,
    G::EdgeWeight: Clone + PartialOrd,
    G: IntoNodeReferences + IntoEdgeReferences + NodeIndexable;
```

Semantics:

```text
Input:
    graph treated as undirected

Algorithm:
    Kruskal

Output:
    iterator producing minimum spanning forest elements

Disconnected graph:
    supported
    returns one MST per connected component

Result graph:
    all original vertices
    identical node indices
    |V| - c edges, where c = connected component count

Complexity:
    time O(|E| log |E|)
    auxiliary space O(|V| + |E|)
```

These semantics are documented directly on `min_spanning_tree`; the docs emphasize that it actually returns a **minimum spanning forest**, not just one tree, and that `FromElements` is the intended graph-building adapter. ([Docs.rs][2])

### Basic MST graph construction

```rust
use petgraph::algo::min_spanning_tree;
use petgraph::data::FromElements;
use petgraph::graph::UnGraph;

let mut g = UnGraph::<&str, u32>::new_undirected();

let a = g.add_node("a");
let b = g.add_node("b");
let c = g.add_node("c");
let d = g.add_node("d");

g.extend_with_edges([
    (a, b, 2),
    (b, c, 1),
    (c, d, 5),
    (a, d, 4),
    (b, d, 7),
]);

let mst: UnGraph<&str, u32> =
    UnGraph::from_elements(min_spanning_tree(&g));

assert_eq!(mst.node_count(), g.node_count());
assert_eq!(mst.edge_count(), g.node_count() - 1);
```

### Minimum spanning forest for disconnected graph

```rust
use petgraph::algo::{connected_components, min_spanning_tree};
use petgraph::data::FromElements;
use petgraph::graph::UnGraph;

let g = UnGraph::<(), u32>::from_edges([
    (0, 1, 10),
    (1, 2, 5),
    (3, 4, 1),
]);

let forest: UnGraph<(), u32> =
    UnGraph::from_elements(min_spanning_tree(&g));

let c = connected_components(&g);

assert_eq!(forest.node_count(), g.node_count());
assert_eq!(forest.edge_count(), g.node_count() - c);
```

### Value case

```text
Use min_spanning_tree when:
    graph may be disconnected
    you want minimum spanning forest
    edge weights are cloneable + partially ordered
    node indices should match original graph
    Kruskal-style sort-by-edge-weight is acceptable
```

---

## 17.4 `min_spanning_tree_prim` — Prim minimum spanning tree

Signature shape:

```rust
pub fn min_spanning_tree_prim<G>(g: G) -> MinSpanningTreePrim<G>
where
    G::EdgeWeight: PartialOrd,
    G: IntoNodeReferences + IntoEdgeReferences;
```

Semantics:

```text
Input:
    graph treated as undirected
    graph treated as connected

Algorithm:
    Prim

Disconnected graph:
    not full forest
    result contains edges for an arbitrary minimum spanning tree of a single component

Result graph:
    all original vertices
    identical node indices
    |V| - 1 edges if connected

Complexity:
    time O(|E| log |V|)
    auxiliary space O(|V| + |E|)
```

Docs explicitly warn that Prim’s MST implementation can be wrong if the graph is not truly undirected and that it assumes the graph is connected; for disconnected graphs, it returns only one component’s tree edges. ([Docs.rs][3])

### Basic Prim MST

```rust
use petgraph::algo::min_spanning_tree_prim;
use petgraph::data::FromElements;
use petgraph::graph::UnGraph;

let g = UnGraph::<(), u32>::from_edges([
    (0, 1, 2),
    (1, 2, 1),
    (0, 2, 10),
]);

let mst: UnGraph<(), u32> =
    UnGraph::from_elements(min_spanning_tree_prim(&g));

assert_eq!(mst.node_count(), g.node_count());
assert_eq!(mst.edge_count(), g.node_count() - 1);
```

### Connectivity guard

```rust
use petgraph::algo::{connected_components, min_spanning_tree_prim};
use petgraph::data::FromElements;
use petgraph::graph::UnGraph;

fn prim_mst_if_connected<N, E>(
    g: &UnGraph<N, E>,
) -> Option<UnGraph<N, E>>
where
    N: Clone,
    E: Clone + PartialOrd,
{
    if connected_components(g) != 1 {
        return None;
    }

    Some(UnGraph::from_elements(min_spanning_tree_prim(g)))
}
```

Use `min_spanning_tree_prim` when:

```text
connected undirected graph is guaranteed
Prim behavior is desired
edge-count result should be |V| - 1
minimum forest is not needed
```

Prefer `min_spanning_tree` when:

```text
graph may be disconnected
forest semantics are required
input undirectedness/connectivity is not fully trusted
```

---

## 17.5 Building MST result graphs with `FromElements`

`min_spanning_tree` and `min_spanning_tree_prim` return iterators of graph elements; docs say to use `FromElements` to create a graph from the resulting iterator. `Element` streams preserve all vertices and MST/forest edges, with identical node indices in the resulting graph. ([Docs.rs][2])

### Canonical pattern

```rust
use petgraph::algo::min_spanning_tree;
use petgraph::data::FromElements;
use petgraph::graph::UnGraph;

let mst: UnGraph<_, _> =
    UnGraph::from_elements(min_spanning_tree(&g));
```

### Extract sorted MST weights

```rust
use petgraph::algo::min_spanning_tree;
use petgraph::data::FromElements;
use petgraph::graph::UnGraph;

let mst: UnGraph<&str, u32> =
    UnGraph::from_elements(min_spanning_tree(&g));

let mut weights: Vec<_> = mst.edge_weights().copied().collect();
weights.sort();

println!("MST weights = {weights:?}");
```

### Keep original domain node indices stable

```rust
let mst: UnGraph<&str, u32> =
    UnGraph::from_elements(min_spanning_tree(&g));

// Same node index addresses original node weights.
for n in g.node_indices() {
    assert_eq!(g[n], mst[n]);
}
```

Agent rule:

```text
MST iterator is not itself a graph.
Use FromElements when:
    you want a petgraph graph result
    you want node indices preserved
    downstream algorithms expect Graph/UnGraph

Use iterator directly when:
    only MST edges/weights are needed
    avoiding graph allocation matters
```

---

## 17.6 `bridges` — critical edge detection

Signature shape:

```rust
pub fn bridges<G>(graph: G) -> impl Iterator<Item = G::EdgeRef>
where
    G: IntoNodeIdentifiers + IntoNeighbors + NodeIndexable + IntoEdgeReferences;
```

Semantics:

```text
Input:
    simple undirected graph

Output:
    iterator of edge references
    each edge reference identifies an edge whose removal increases connected-component count
    edge order arbitrary

Complexity:
    time O(|V| + |E|)
    auxiliary space O(|V|)
```

The `bridges` function is documented as finding all bridges in a **simple undirected graph**; it returns an iterator of edge references with arbitrary order and runs in linear time with linear-in-nodes auxiliary space. ([Docs.rs][4])

### Basic bridge detection

```rust
use petgraph::algo::bridges::bridges;
use petgraph::graph::UnGraph;
use petgraph::visit::EdgeRef;

let mut g = UnGraph::<&str, ()>::new_undirected();

let a = g.add_node("a");
let b = g.add_node("b");
let c = g.add_node("c");

let ab = g.add_edge(a, b, ());
let bc = g.add_edge(b, c, ());

let bridge_edges: Vec<_> =
    bridges(&g).map(|edge_ref| edge_ref.id()).collect();

assert_eq!(bridge_edges.len(), 2);
assert!(bridge_edges.contains(&ab));
assert!(bridge_edges.contains(&bc));
```

### Convert bridge refs to domain rows

```rust
use petgraph::algo::bridges::bridges;
use petgraph::visit::EdgeRef;

#[derive(Debug)]
struct BridgeRow<N> {
    edge_id: petgraph::graph::EdgeIndex,
    source: N,
    target: N,
}

let rows: Vec<_> = bridges(&g)
    .map(|e| BridgeRow {
        edge_id: e.id(),
        source: g[e.source()].clone(),
        target: g[e.target()].clone(),
    })
    .collect();
```

Use `bridges` when:

```text
simple undirected graph
critical links / cut edges matter
network resilience analysis
tree/backbone extraction
failure analysis
```

Avoid or preprocess when:

```text
parallel edges exist
directed edge semantics matter
edge removal should respect capacities/flows
graph is not simple
```

---

## 17.7 Bridge semantics and multigraph caveat

Bridge definition in this API is for **simple undirected graphs**. In multigraph-like data, two parallel edges between the same endpoints mean removing one edge may not disconnect anything, so using `bridges` directly on `Graph` with parallel edges can produce results that do not match domain intuition unless the graph is simplified first. ([Docs.rs][4])

Simplify endpoint-pair multigraph before bridge analysis:

```rust
use std::collections::HashSet;
use petgraph::graph::{UnGraph, NodeIndex};
use petgraph::visit::EdgeRef;

fn simple_undirected_projection<N: Clone, E>(
    g: &UnGraph<N, E>,
) -> UnGraph<N, ()> {
    let mut simple = UnGraph::<N, ()>::with_capacity(
        g.node_count(),
        g.edge_count(),
    );

    let mut old_to_new = Vec::new();

    for n in g.node_indices() {
        old_to_new.push(simple.add_node(g[n].clone()));
    }

    let mut seen = HashSet::<(usize, usize)>::new();

    for e in g.edge_references() {
        let a = e.source().index();
        let b = e.target().index();

        let key = if a <= b { (a, b) } else { (b, a) };

        if seen.insert(key) {
            simple.add_edge(
                NodeIndex::new(key.0),
                NodeIndex::new(key.1),
                (),
            );
        }
    }

    simple
}
```

Agent rule:

```text
Before bridge analysis:
    ensure graph is undirected
    ensure graph is simple
    normalize/deduplicate parallel endpoint pairs
```

---

## 17.8 `articulation_points` — critical vertex detection

Signature shape:

```rust
pub fn articulation_points<G>(g: G) -> HashSet<G::NodeId>
where
    G: IntoNodeReferences + IntoEdges + NodeIndexable + GraphProp,
    G::NodeWeight: Clone,
    G::EdgeWeight: Clone + PartialOrd,
    G::NodeId: Eq + Hash;
```

Semantics:

```text
Input:
    graph accepted by trait bounds
    practical target: undirected connectivity articulation analysis

Output:
    HashSet<NodeId>
    each node is a cut vertex / articulation point

Complexity:
    time O(|V| + |E|)
    auxiliary space O(|V|)
```

The function is documented as finding articulation points using Tarjan’s algorithm and returning a `HashSet` of node IDs; its example uses `UnGraph` and returns the middle node of a three-node path as the articulation point. The rustdoc’s argument line says “directed graph,” but the articulation-point definition and example are undirected connectivity oriented; for production, prefer `UnGraph` / simple undirected projections unless you have tested directed input semantics for your domain. ([Docs.rs][5])

### Basic articulation point detection

```rust
use petgraph::algo::articulation_points::articulation_points;
use petgraph::graph::UnGraph;

let mut g = UnGraph::<&str, ()>::new_undirected();

let a = g.add_node("A");
let b = g.add_node("B");
let c = g.add_node("C");

g.add_edge(a, b, ());
g.add_edge(b, c, ());

let points = articulation_points(&g);

assert!(points.contains(&b));
assert!(!points.contains(&a));
assert!(!points.contains(&c));
```

### Convert articulation IDs to labels

```rust
use petgraph::algo::articulation_points::articulation_points;

let labels: Vec<_> = articulation_points(&g)
    .into_iter()
    .map(|node| g[node])
    .collect();

println!("cut vertices: {labels:?}");
```

Use `articulation_points` when:

```text
critical nodes / cut vertices matter
removing a facility/router/junction could split network
undirected connectivity resilience is the target
node-failure risk analysis
```

Avoid or preprocess when:

```text
directed reachability cut semantics are required
node capacities/flows are required
graph has domain-specific failure semantics
parallel edges/self-loops should be normalized first
```

---

## 17.9 `UnionFind` — disjoint-set data structure

`UnionFind<K>` tracks disjoint-set membership for elements indexed `0..n-1`; `K` must be an unsigned integer index type implementing petgraph’s `IndexType`. It supports representative lookup, path-compressing mutable lookup, set union, dynamic addition via `new_set`, capacity management, fallible variants, and `into_labeling`. Its docs quote inverse-Ackermann amortized operation time. ([Docs.rs][6])

### Basic construction

```rust
use petgraph::unionfind::UnionFind;

let mut uf: UnionFind<u32> = UnionFind::new(5);

assert_eq!(uf.len(), 5);
assert!(!uf.equiv(0, 1));

assert!(uf.union(0, 1));
assert!(uf.equiv(0, 1));

assert!(!uf.union(0, 1)); // already unified
```

### Fallible boundary methods

```rust
use petgraph::unionfind::UnionFind;

let mut uf: UnionFind<u32> = UnionFind::new(3);

assert_eq!(uf.try_find(10), None);
assert_eq!(uf.try_equiv(0, 10), Err(10));
assert_eq!(uf.try_union(0, 10), Err(10));
```

`try_find`, `try_equiv`, and `try_union` return `None` / `Err` instead of panicking on out-of-bounds indices; `union`, `find`, and `equiv` panic on out-of-bounds. ([Docs.rs][6])

### Dynamic growth

```rust
use petgraph::unionfind::UnionFind;

let mut uf: UnionFind<u32> = UnionFind::new_empty();

let a = uf.new_set();
let b = uf.new_set();

assert_eq!(a, 0);
assert_eq!(b, 1);

uf.union(a, b);
assert!(uf.equiv(a, b));
```

`new_set` adds a new disjoint set at the end and returns its index; docs state it takes amortized `O(1)` time. ([Docs.rs][6])

### Capacity planning

```rust
use petgraph::unionfind::UnionFind;

let mut uf: UnionFind<u32> = UnionFind::with_capacity(10_000);

for _ in 0..1_000 {
    uf.new_set();
}

assert!(uf.capacity() >= 10_000);
```

`with_capacity`, `reserve`, `reserve_exact`, `try_reserve`, `try_reserve_exact`, `shrink_to_fit`, and `shrink_to` provide the usual vector-like capacity controls. ([Docs.rs][6])

---

## 17.10 UnionFind as connected-component builder

Use `UnionFind` when you need component labels, custom connectivity rules, or incremental connectivity outside a full graph traversal.

```rust
use petgraph::unionfind::UnionFind;

let n = 6usize;
let edges = [
    (0u32, 1u32),
    (1u32, 2u32),
    (3u32, 4u32),
];

let mut uf = UnionFind::<u32>::new(n);

for (a, b) in edges {
    uf.union(a, b);
}

assert!(uf.equiv(0, 2));
assert!(uf.equiv(3, 4));
assert!(!uf.equiv(0, 3));

let labels = uf.into_labeling();
```

Component grouping:

```rust
use std::collections::HashMap;
use petgraph::unionfind::UnionFind;

fn groups_from_unionfind(mut uf: UnionFind<u32>) -> HashMap<u32, Vec<u32>> {
    let mut groups = HashMap::<u32, Vec<u32>>::new();

    for i in 0..uf.len() {
        let i = i as u32;
        let root = uf.find_mut(i);
        groups.entry(root).or_default().push(i);
    }

    groups
}
```

Rule:

```text
UnionFind works on dense integer IDs.
For petgraph NodeIndex:
    map NodeIndex.index() -> K
    use NodeIndexable when writing generic code
    beware StableGraph holes if using raw index ranges
```

---

## 17.11 Manual Kruskal with `UnionFind`

Use this when you need custom edge filtering, custom tie-breaking, or output metadata beyond `FromElements`.

```rust
use petgraph::graph::{UnGraph, EdgeIndex};
use petgraph::unionfind::UnionFind;
use petgraph::visit::{EdgeRef, NodeIndexable};

fn kruskal_edge_ids<N, E>(
    g: &UnGraph<N, E>,
) -> Vec<EdgeIndex>
where
    E: PartialOrd,
{
    let mut edges: Vec<_> = g.edge_references().collect();

    edges.sort_by(|a, b| {
        a.weight()
            .partial_cmp(b.weight())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut uf = UnionFind::<u32>::new(g.node_bound());
    let mut selected = Vec::new();

    for e in edges {
        let a = g.to_index(e.source()) as u32;
        let b = g.to_index(e.target()) as u32;

        if uf.union(a, b) {
            selected.push(e.id());
        }
    }

    selected
}
```

Production fixes:

```text
If E can be NaN/incomparable:
    reject edge set before sort
    use ordered wrapper
    return Result instead of unwrap_or

If graph has holes:
    build dense NodeIndex -> u32 remap instead of node_bound raw indexing

If graph is directed:
    normalize to undirected semantics first
```

---

## 17.12 `min_spanning_tree` vs manual `UnionFind`

| Requirement                             |                     Built-in MST |     Manual UnionFind |
| --------------------------------------- | -------------------------------: | -------------------: |
| Standard MST/forest                     |                             Best |            More code |
| Preserve node indices in result graph   |           Yes via `FromElements` |       Must implement |
| Need custom edge filter                 | Use pre-filtered graph or manual |                 Best |
| Need custom tie-breaker                 |                           Harder |                 Best |
| Need selected original `EdgeIndex` list |                         Indirect |                 Best |
| Need disconnected forest                |              `min_spanning_tree` |   Supported if coded |
| Need connected-only MST                 |         `min_spanning_tree_prim` | Supported if guarded |
| Need diagnose skipped edges             |                           Manual |                 Best |

---

## 17.13 Bridges vs MST edges

```text
Bridge:
    edge that appears in every spanning forest/tree of its component
    removal increases connected component count

MST edge:
    selected by weight optimization
    may not be structurally mandatory

Relationship:
    every bridge in a weighted connected component must be present in every spanning tree,
    therefore also in every MST/forest for that component
```

Practical check:

```rust
use std::collections::HashSet;
use petgraph::algo::bridges::bridges;
use petgraph::algo::min_spanning_tree;
use petgraph::data::FromElements;
use petgraph::graph::UnGraph;
use petgraph::visit::EdgeRef;

let bridge_ids: HashSet<_> =
    bridges(&g).map(|e| e.id()).collect();

let mst: UnGraph<_, _> =
    UnGraph::from_elements(min_spanning_tree(&g));
```

Caveat:

```text
MST result graph preserves node indices,
but edge indices in the result graph are not the original graph's EdgeIndex values.
If original EdgeIndex preservation is required, implement manual Kruskal.
```

---

## 17.14 Articulation points vs bridges

| Cut object           | Function              | Domain meaning                                  |
| -------------------- | --------------------- | ----------------------------------------------- |
| Edge cut of size 1   | `bridges`             | critical link / cable / road / pipe             |
| Vertex cut of size 1 | `articulation_points` | critical station / router / junction / facility |

Infrastructure interpretation:

```text
bridge edge:
    failure of this link disconnects graph

articulation point:
    failure of this node disconnects graph

MST:
    cheapest connectivity backbone, not redundancy analysis by itself
```

Combined resilience report:

```rust
use petgraph::algo::bridges::bridges;
use petgraph::algo::articulation_points::articulation_points;
use petgraph::visit::EdgeRef;

let critical_links: Vec<_> =
    bridges(&g).map(|e| (e.source(), e.target())).collect();

let critical_nodes: Vec<_> =
    articulation_points(&g).into_iter().collect();
```

---

## 17.15 Practical use cases

### Network design

```text
Problem:
    connect all sites with minimum total cable/fiber/road cost

Use:
    min_spanning_tree for disconnected site clusters
    min_spanning_tree_prim only when connectedness is guaranteed
    bridges for unavoidable links
    articulation_points for critical facilities
```

```rust
use petgraph::algo::min_spanning_tree;
use petgraph::data::FromElements;
use petgraph::graph::UnGraph;

#[derive(Clone, Debug)]
struct Site {
    name: String,
}

type Cost = u32;

let backbone: UnGraph<Site, Cost> =
    UnGraph::from_elements(min_spanning_tree(&candidate_links));
```

### Clustering / single-linkage style workflows

```text
Problem:
    build a low-cost forest connecting nearby points

Use:
    MST as hierarchy/backbone
    cut high-weight MST edges to form clusters
```

```rust
let mst: UnGraph<Point, u32> =
    UnGraph::from_elements(min_spanning_tree(&distance_graph));

let threshold = 1_000;

let cluster_forest = mst.filter_map(
    |_ix, point| Some(point.clone()),
    |_ix, &dist| (dist <= threshold).then_some(dist),
);
```

### Infrastructure planning / resilience

```text
Problem:
    identify fragile roads/pipes/cables/stations

Use:
    bridges = critical edges
    articulation_points = critical nodes
    MST = minimal-cost connectivity baseline
    compare actual network against MST + redundancy edges
```

---

## 17.16 Algorithm trait-bound matrix

| API                      | Key bounds / concrete expectations                                                                                                         |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `min_spanning_tree`      | `NodeWeight: Clone`, `EdgeWeight: Clone + PartialOrd`, `IntoNodeReferences + IntoEdgeReferences + NodeIndexable`                           |
| `min_spanning_tree_prim` | `EdgeWeight: PartialOrd`, `IntoNodeReferences + IntoEdgeReferences`                                                                        |
| `bridges`                | `IntoNodeIdentifiers + IntoNeighbors + NodeIndexable + IntoEdgeReferences`                                                                 |
| `articulation_points`    | `IntoNodeReferences + IntoEdges + NodeIndexable + GraphProp`, node/edge weights cloneable, edge weights `PartialOrd`, node IDs `Eq + Hash` |
| `UnionFind<K>`           | `K: IndexType`, elements indexed `0..n-1`                                                                                                  |

These are the public rustdoc bounds for the released APIs; they matter because `StableGraph` holes, `GraphMap` node IDs, and custom graph types may or may not satisfy the exact bound set you select. ([Docs.rs][2])

---

## 17.17 Production decision table

| Workload                                      | Recommended primitive                     | Advisory                               |
| --------------------------------------------- | ----------------------------------------- | -------------------------------------- |
| Standard MST over possibly disconnected graph | `min_spanning_tree`                       | Returns forest                         |
| Connected graph MST                           | `min_spanning_tree_prim`                  | Validate connectedness first           |
| Need MST graph object                         | `FromElements`                            | Use `UnGraph::from_elements(...)`      |
| Need original edge IDs in MST                 | Manual Kruskal + `UnionFind`              | Built-in result graph has new edge IDs |
| Need critical links                           | `bridges`                                 | Simple undirected graph only           |
| Need critical vertices                        | `articulation_points`                     | Prefer undirected/simple input         |
| Need custom connected components              | `UnionFind`                               | Dense integer ID domain required       |
| Need sparse infrastructure resilience report  | `bridges` + `articulation_points`         | Normalize graph first                  |
| Need clustering backbone                      | `min_spanning_tree` then threshold/filter | MST edges define hierarchy             |

---

## 17.18 Anti-pattern inventory

```text
Anti-pattern:
    Run min_spanning_tree_prim on disconnected graph.
Problem:
    docs say it returns only one component's tree edges.
Fix:
    use min_spanning_tree or validate connected_components == 1.

Anti-pattern:
    Use directed graph semantics for MST.
Problem:
    MST functions treat input as undirected; Prim docs warn wrong result if not truly undirected.
Fix:
    build/convert a semantically undirected graph.

Anti-pattern:
    Use f64 weights with NaN in MST.
Problem:
    EdgeWeight only requires PartialOrd; NaN is incomparable.
Fix:
    reject NaN or use ordered wrapper/integer fixed-point.

Anti-pattern:
    Expect MST result edge indices to match original graph edge indices.
Problem:
    FromElements builds a new graph.
Fix:
    implement manual Kruskal collecting original EdgeIndex values.

Anti-pattern:
    Run bridges on multigraph-like data.
Problem:
    API is for simple undirected graphs.
Fix:
    simplify/deduplicate endpoint pairs first.

Anti-pattern:
    Treat articulation_points as directed dominator/cut analysis.
Problem:
    articulation-point analysis is connectivity cut-vertex logic, not flow/dominator semantics.
Fix:
    use dominators/flow-specific algorithms for directed control-flow or s-t cuts.

Anti-pattern:
    Use UnionFind with raw StableGraph node_bound loops after deletions.
Problem:
    holes can make raw index range include invalid nodes.
Fix:
    build dense live-node remap.
```

---

## 17.19 Deployment checklist

```text
Before MST:
    ensure graph is semantically undirected
    choose forest vs connected-tree algorithm
    validate connectedness if using Prim
    validate edge-weight ordering
    reject NaN/incomparable weights
    decide whether original EdgeIndex preservation matters

Before bridge/articulation analysis:
    simplify graph if multiedges exist
    use undirected graph representation where possible
    decide whether self-loops are meaningful/noise
    produce domain labels for reports

For UnionFind:
    choose K = u32 unless tiny/huge domain dictates otherwise
    preallocate with new(n) or with_capacity
    use try_* at external boundaries
    remap graph nodes to dense integers when graph indices are sparse
```

Final rule:

```text
MST answers:
    cheapest connectivity backbone

Bridges answer:
    which links are structurally mandatory

Articulation points answer:
    which nodes are structurally mandatory

UnionFind answers:
    which dense-indexed elements are in the same component
```

[1]: https://docs.rs/petgraph/latest/petgraph/algo/index.html?utm_source=chatgpt.com "petgraph::algo - Rust"
[2]: https://docs.rs/petgraph/latest/petgraph/algo/min_spanning_tree/fn.min_spanning_tree.html "min_spanning_tree in petgraph::algo::min_spanning_tree - Rust"
[3]: https://docs.rs/petgraph/latest/petgraph/algo/min_spanning_tree/fn.min_spanning_tree_prim.html "min_spanning_tree_prim in petgraph::algo::min_spanning_tree - Rust"
[4]: https://docs.rs/petgraph/latest/petgraph/algo/bridges/fn.bridges.html "bridges in petgraph::algo::bridges - Rust"
[5]: https://docs.rs/petgraph/latest/petgraph/algo/articulation_points/fn.articulation_points.html "articulation_points in petgraph::algo::articulation_points - Rust"
[6]: https://docs.rs/petgraph/latest/petgraph/unionfind/struct.UnionFind.html "UnionFind in petgraph::unionfind - Rust"


# 18) Algorithm catalog — matching, flow, cliques, coloring

Format follows the uploaded advanced-reference style. 

Target: **petgraph 0.8.x released API**. The `algo` module re-exports `greedy_matching`, `maximum_matching`, `Matching`, `dinics`, `ford_fulkerson`, `maximal_cliques`, and `dsatur_coloring`; the module also notes that algorithms are gradually migrating toward graph-trait-based implementations, while some still have graph-type-specific constraints. ([Docs.rs][1])

---

## 18.0 Core imports

```rust
use petgraph::algo::{
    greedy_matching,
    maximum_matching,
    Matching,
    dinics,
    ford_fulkerson,
    maximal_cliques,
    dsatur_coloring,
};

use petgraph::graph::{
    Graph,
    DiGraph,
    UnGraph,
    NodeIndex,
    EdgeIndex,
};

use petgraph::visit::{
    EdgeRef,
    NodeIndexable,
    EdgeIndexable,
};

use std::collections::{HashMap, HashSet};
```

---

## 18.1 Decision table

| Problem                      | Petgraph API       | Graph shape                     | Output                                  | Optimization status               |
| ---------------------------- | ------------------ | ------------------------------- | --------------------------------------- | --------------------------------- |
| Fast valid matching          | `greedy_matching`  | treated undirected              | `Matching<G>`                           | heuristic only                    |
| Maximum-cardinality matching | `maximum_matching` | treated undirected              | `Matching<G>`                           | exact maximum                     |
| Inspect matching             | `Matching`         | matching result                 | mates, nodes, edges                     | query wrapper                     |
| Max flow                     | `dinics`           | directed weighted network       | `(max_flow, edge_flows)`                | exact max flow                    |
| Max flow, small/sparse       | `ford_fulkerson`   | directed weighted network       | `(max_flow, edge_flows)`                | exact max flow                    |
| Enumerate maximal cliques    | `maximal_cliques`  | undirected / symmetric directed | `Vec<HashSet<NodeId>>`                  | exhaustive                        |
| Heuristic graph coloring     | `dsatur_coloring`  | undirected, loop-free           | `(HashMap<NodeId, usize>, color_count)` | heuristic, not guaranteed minimum |

The maximum-flow module states that Dinic’s and Ford-Fulkerson/Edmonds-Karp compute the same maximum flow value but may assign edge flows differently; it also notes Dinic’s is generally faster, especially on dense, unit-capacity, and bipartite graphs, while Ford-Fulkerson may be preferable on small or sparse graphs. ([Docs.rs][2])

---

## 18.2 Matching model

A graph matching is a set of edges with no shared endpoint. Petgraph’s matching algorithms treat the input graph **as undirected**, regardless of directed graph orientation; `greedy_matching` guarantees only a valid matching, while `maximum_matching` computes a maximum matching. ([Docs.rs][3])

```text
matching invariant:
    no node appears in more than one matched edge

valid matching:
    all selected edges are pairwise endpoint-disjoint

maximum matching:
    valid matching with maximum possible number of matched edges

perfect matching:
    every graph node is matched
```

---

## 18.3 `greedy_matching` — fast heuristic matching

Signature:

```rust
pub fn greedy_matching<G>(graph: G) -> Matching<G>
where
    G: Visitable + IntoNodeIdentifiers + NodeIndexable + IntoNeighbors,
    G::NodeId: Eq + Hash,
    G::EdgeId: Eq + Hash;
```

`greedy_matching` computes a matching using an unspecified greedy heuristic; the input graph is treated as undirected, the output is guaranteed only to be a valid matching, and the documented complexity is `O(|V| + |E|)` time with `O(|V|)` auxiliary space. ([Docs.rs][3])

```rust
use petgraph::algo::greedy_matching;
use petgraph::graph::UnGraph;

let mut graph = UnGraph::<(), ()>::new_undirected();

let a = graph.add_node(());
let b = graph.add_node(());
let c = graph.add_node(());
let d = graph.add_node(());

graph.extend_with_edges([
    (a, b),
    (b, c),
    (c, d),
]);

let matching = greedy_matching(&graph);

assert!(matching.len() <= graph.node_count() / 2);
```

Use when:

```text
large graph
matching quality can be approximate
valid endpoint-disjoint set is enough
speed matters more than optimality
preprocessing / warm-start / heuristic scheduling
```

Avoid when:

```text
maximum cardinality is required
benchmarking exact matching quality
unmatched-node count is part of correctness
```

---

## 18.4 `maximum_matching` — exact maximum-cardinality matching

Signature:

```rust
pub fn maximum_matching<G>(graph: G) -> Matching<G>
where
    G: Visitable + NodeIndexable + IntoNodeIdentifiers + IntoEdges;
```

`maximum_matching` computes a maximum matching using Gabow’s algorithm; the input is treated as undirected, the documented time complexity is `O(|V|³)`, auxiliary space is `O(|V| + |E|)`, and the docs note that a better-complexity algorithm may be used in the future. ([Docs.rs][4])

```rust
use petgraph::algo::maximum_matching;
use petgraph::graph::UnGraph;

let mut graph = UnGraph::<(), ()>::new_undirected();

let a = graph.add_node(());
let b = graph.add_node(());
let c = graph.add_node(());
let d = graph.add_node(());
let e = graph.add_node(());
let f = graph.add_node(());

graph.extend_with_edges([
    (a, b),
    (a, c),
    (b, c),
    (b, d),
    (c, e),
    (d, e),
    (d, f),
]);

let matching = maximum_matching(&graph);

assert!(matching.contains_edge(a, b));
assert!(matching.contains_edge(c, e));
assert_eq!(matching.mate(d), Some(f));
assert_eq!(matching.mate(f), Some(d));
```

Use when:

```text
maximum cardinality is correctness-critical
graph size is moderate
O(V³) is acceptable
matching is structural, not weighted
```

Avoid when:

```text
weighted matching required
bipartite assignment with costs required
very large graph with strict latency budget
incremental/dynamic matching required
```

---

## 18.5 `Matching<G>` — matching output API

`Matching<G>` is the result wrapper for petgraph matching algorithms. It exposes `mate(node)`, `edges()`, `nodes()`, `contains_edge(a,b)`, `contains_node(node)`, `len()`, `is_empty()`, and `is_perfect()`; `edges()` reports matched endpoint pairs once, treating the graph as undirected. ([Docs.rs][5])

```rust
use petgraph::algo::{maximum_matching, Matching};
use petgraph::graph::UnGraph;

let matching = maximum_matching(&graph);

for (a, b) in matching.edges() {
    println!("matched {a:?} -- {b:?}");
}

for n in matching.nodes() {
    println!("matched node {n:?}");
}

if let Some(mate) = matching.mate(a) {
    println!("a is matched with {mate:?}");
}

assert_eq!(matching.contains_edge(a, b), matching.mate(a) == Some(b));
```

Perfect matching check:

```rust
if matching.is_perfect() {
    println!("every node is matched");
}
```

Rules:

```text
matching.len():
    number of matched edges

matching.nodes():
    all matched endpoints

matching.mate(n):
    matched counterpart or None

matching.is_perfect():
    true iff every node is incident to a matching edge
```

---

## 18.6 Matching deployment patterns

### Pairing workers with jobs, unweighted feasibility

```rust
use petgraph::graph::UnGraph;
use petgraph::algo::maximum_matching;

#[derive(Clone, Debug)]
enum Vertex {
    Worker(u32),
    Job(u32),
}

let mut g = UnGraph::<Vertex, ()>::new_undirected();

let w1 = g.add_node(Vertex::Worker(1));
let w2 = g.add_node(Vertex::Worker(2));
let j1 = g.add_node(Vertex::Job(1));
let j2 = g.add_node(Vertex::Job(2));

g.extend_with_edges([
    (w1, j1),
    (w1, j2),
    (w2, j2),
]);

let matching = maximum_matching(&g);
```

### Maximum matching is not assignment optimization

```text
Petgraph maximum_matching:
    maximizes number of matched edges
    does not minimize cost
    does not maximize weight
    does not enforce bipartite partitions

For weighted assignment:
    use specialized assignment/min-cost-flow/LP/MIP crate
    or model as min-cost max-flow in a solver that supports costs
```

---

## 18.7 Maximum-flow capacity model

`dinics` and `ford_fulkerson` take the graph edge weight itself as capacity; there is no capacity closure. Both return `(max_flow, Vec<EdgeWeight>)`, and the flow vector is indexed by the graph’s edge indices. Both require a directed weighted network and `EdgeWeight: PositiveMeasure + Sub<Output = EdgeWeight>`; `dinics` also warns it can panic when edge weights are not comparable through `PartialOrd` semantics. ([Docs.rs][6])

```text
capacity graph:
    NodeWeight = arbitrary node payload
    EdgeWeight = capacity type
    edge weight must be positive-measure-like
    edge weight also represents flow value type

output:
    max_flow: EdgeWeight
    flows: Vec<EdgeWeight>
    flows[edge_index] = assigned flow on that edge
```

Recommended capacity types:

```text
u32 / u64:
    integral capacities
    safe default

usize:
    graph-size/platform-adjacent quantities

f64:
    avoid unless comparability/precision tested
    reject NaN
```

If edge payload is domain-rich, project to a capacity graph:

```rust
use petgraph::graph::DiGraph;

#[derive(Clone, Debug)]
struct Pipe {
    capacity_lps: u32,
    material: String,
}

let capacity_graph: DiGraph<&str, u32> = domain_graph.map(
    |_node_ix, node| *node,
    |_edge_ix, pipe| pipe.capacity_lps,
);
```

---

## 18.8 `dinics` — max flow with level graphs and blocking flows

Signature:

```rust
pub fn dinics<G>(
    network: G,
    source: G::NodeId,
    destination: G::NodeId,
) -> (G::EdgeWeight, Vec<G::EdgeWeight>)
where
    G: NodeCount
        + EdgeCount
        + IntoEdgesDirected
        + EdgeIndexable
        + NodeIndexable
        + Visitable,
    G::EdgeWeight: Sub<Output = G::EdgeWeight> + PositiveMeasure;
```

`dinics` computes maximum flow from `source` to `destination` in a directed graph with positive edge capacities, builds successive BFS level graphs and DFS blocking flows, returns maximum flow plus per-edge flow vector, and has documented general time complexity `O(|V|²|E|)` plus better unit-capacity bounds. ([Docs.rs][6])

```rust
use petgraph::Graph;
use petgraph::algo::dinics;
use petgraph::visit::{EdgeRef, EdgeIndexable};

let mut graph = Graph::<&str, u32>::new();

let s = graph.add_node("s");
let a = graph.add_node("a");
let b = graph.add_node("b");
let t = graph.add_node("t");

let e_sa = graph.add_edge(s, a, 10);
let e_sb = graph.add_edge(s, b, 5);
let e_at = graph.add_edge(a, t, 7);
let e_bt = graph.add_edge(b, t, 8);

let (max_flow, flows) = dinics(&graph, s, t);

println!("max_flow={max_flow}");

for edge in graph.edge_references() {
    let idx = graph.to_index(edge.id());
    println!(
        "{:?} -> {:?}: capacity={}, flow={}",
        edge.source(),
        edge.target(),
        edge.weight(),
        flows[idx],
    );
}
```

Use `dinics` when:

```text
max-flow value required
flow assignment per edge required
graph is dense, unit-capacity, bipartite, or moderately large
directed network model is natural
capacities are non-negative/positive and comparable
```

---

## 18.9 `ford_fulkerson` — Edmonds-Karp max flow

Signature:

```rust
pub fn ford_fulkerson<G>(
    network: G,
    source: G::NodeId,
    destination: G::NodeId,
) -> (G::EdgeWeight, Vec<G::EdgeWeight>)
where
    G: NodeCount
        + EdgeCount
        + IntoEdgesDirected
        + EdgeIndexable
        + NodeIndexable
        + DataMap
        + Visitable,
    G::EdgeWeight: Sub<Output = G::EdgeWeight> + PositiveMeasure;
```

`ford_fulkerson` is documented as the Ford-Fulkerson algorithm in the Edmonds-Karp variation for weighted directed graphs; it returns maximum flow plus per-edge flow vector, with time complexity `O(|V||E|²)` and auxiliary space `O(|V| + |E|)`. ([Docs.rs][7])

```rust
use petgraph::Graph;
use petgraph::algo::ford_fulkerson;

let (max_flow, flows) = ford_fulkerson(&graph, s, t);

assert!(max_flow <= 15);
assert_eq!(flows.len(), graph.edge_count());
```

Use `ford_fulkerson` when:

```text
graph is small
graph is sparse
simple max-flow baseline needed
Dinic overhead not justified
cross-checking max-flow result in tests
```

Prefer `dinics` when:

```text
graph is larger
graph is dense
unit-capacity network
bipartite-style network
performance matters
```

---

## 18.10 Flow output interpretation

```rust
use petgraph::visit::{EdgeRef, EdgeIndexable};

let (max_flow, flow_by_edge) = dinics(&network, source, sink);

for e in network.edge_references() {
    let edge_id = e.id();
    let i = network.to_index(edge_id);

    let cap = *e.weight();
    let flow = flow_by_edge[i];

    assert!(flow <= cap);

    println!(
        "{:?}->{:?}: {flow}/{cap}",
        e.source(),
        e.target()
    );
}
```

Rules:

```text
flow vector:
    indexed by EdgeIndexable::to_index(edge_id)
    length tracks edge_bound / edge index layout
    tied to graph used in algorithm call

Do not:
    mutate graph before interpreting flow vector
    interpret flow vector against another graph
    assume edge IDs survive Graph deletions
```

---

## 18.11 Flow modeling recipes

### Single-source/sink network

```rust
type FlowNet = DiGraph<&'static str, u32>;

let (max_flow, flows) = dinics(&net, source, sink);
```

### Super-source / super-sink

```rust
let super_source = net.add_node("super_source");
let super_sink = net.add_node("super_sink");

for producer in producers {
    net.add_edge(super_source, producer, producer_capacity[&producer]);
}

for consumer in consumers {
    net.add_edge(consumer, super_sink, consumer_demand[&consumer]);
}

let (max_flow, flows) = dinics(&net, super_source, super_sink);
```

### Bipartite matching via max flow

```text
source -> left partition:
    capacity 1

left -> right compatibility edges:
    capacity 1

right partition -> sink:
    capacity 1

max_flow:
    maximum bipartite matching cardinality
```

Use petgraph max-flow for this when you need the flow formulation and not just `maximum_matching`.

---

## 18.12 Flow limitations

```text
Petgraph max-flow APIs:
    maximum s-t flow value
    per-edge flow vector
    capacities are edge weights
    no capacity closure
    no lower bounds
    no costs
    no min-cost max-flow
    no multi-commodity flow
    no explicit residual graph output
    no LP/MIP constraints
```

Leave petgraph when:

```text
min-cost flow required
assignment with costs required
capacity lower bounds required
integer programming constraints required
multi-commodity flow required
time-expanded scheduling with side constraints required
industrial-scale optimization / infeasibility proofs required
```

---

## 18.13 `maximal_cliques` — enumerate all maximal cliques

Signature:

```rust
pub fn maximal_cliques<G>(g: G) -> Vec<HashSet<G::NodeId>>
where
    G: GetAdjacencyMatrix + IntoNodeIdentifiers + IntoNeighbors,
    G::NodeId: Eq + Hash;
```

`maximal_cliques` finds all maximal cliques in an undirected graph using the Bron-Kerbosch algorithm with pivoting; it also works on symmetric directed graphs when `(u, v)` implies `(v, u)`. The output is `Vec<HashSet<NodeId>>`; time complexity is `O(3^(|V|/3))`, and auxiliary space is `O(|V|² + |V|k)`, where `k` is the number of maximal cliques and can itself be exponential. ([Docs.rs][8])

```rust
use petgraph::algo::maximal_cliques;
use petgraph::graph::UnGraph;

let mut g = UnGraph::<i32, ()>::from_edges([
    (0, 1),
    (0, 2),
    (1, 2),
    (2, 3),
]);

g.add_node(4);

let cliques = maximal_cliques(&g);

for clique in cliques {
    println!("{clique:?}");
}
```

Use when:

```text
all maximal cliques are needed
graph is small/moderate
worst-case exponential enumeration is acceptable
undirected simple graph semantics hold
```

Avoid when:

```text
large dense graph
only maximum clique needed
only clique count/approximation needed
directed nonsymmetric graph
streaming output required
memory cannot hold Vec<HashSet<NodeId>>
```

---

## 18.14 Clique enumeration cost controls

```text
Before maximal_cliques:
    filter nodes
    filter edges
    restrict to connected component/subgraph
    remove isolated nodes if irrelevant
    cap graph size
    timebox/cancel at outer layer if needed
```

Subgraph projection pattern:

```rust
use petgraph::graph::UnGraph;
use petgraph::algo::maximal_cliques;

let active = g.filter_map(
    |_ix, node| node.active.then_some(node.id),
    |_ix, edge| edge.enabled.then_some(()),
);

let cliques = maximal_cliques(&active);
```

Operational rule:

```text
maximal_cliques is output-sensitive and exponential.
Do not run blindly on unbounded user-provided graphs.
```

---

## 18.15 `dsatur_coloring` — heuristic graph coloring

Signature:

```rust
pub fn dsatur_coloring<G>(graph: G) -> (HashMap<G::NodeId, usize>, usize)
where
    G: IntoEdges + IntoNodeIdentifiers + Visitable + NodeIndexable,
    G::NodeId: Eq + Hash;
```

`dsatur_coloring` applies the DSatur heuristic to color a non-weighted undirected graph; it is explicitly documented as a heuristic that does not necessarily return a minimum coloring, requires an undirected graph without loops, and returns a `HashMap<NodeId, usize>` plus the number of colors used. The documented complexity is `O((|V| + |E|) log |V|)` time and `O(|V| + |E|)` auxiliary space. ([Docs.rs][9])

```rust
use petgraph::algo::dsatur_coloring;
use petgraph::graph::UnGraph;

let mut graph = UnGraph::<(), ()>::new_undirected();

let a = graph.add_node(());
let b = graph.add_node(());
let c = graph.add_node(());
let d = graph.add_node(());

graph.extend_with_edges([
    (a, b),
    (b, c),
    (c, d),
    (d, a),
]);

let (colors, color_count) = dsatur_coloring(&graph);

assert!(color_count <= 2);
assert_ne!(colors[&a], colors[&b]);
assert_ne!(colors[&b], colors[&c]);
```

Use when:

```text
valid coloring is enough
near-minimal heuristic is acceptable
register/resource/time-slot assignment is heuristic
undirected loop-free conflict graph
fast coloring needed
```

Avoid when:

```text
minimum coloring is correctness-critical
weighted coloring required
list coloring required
precolored vertices required
loop-containing graph
domain constraints exceed simple graph coloring
```

---

## 18.16 Coloring output validation

```rust
use petgraph::visit::EdgeRef;

fn validate_coloring<N, E>(
    g: &UnGraph<N, E>,
    colors: &HashMap<NodeIndex, usize>,
) -> bool {
    for edge in g.edge_references() {
        let a = edge.source();
        let b = edge.target();

        if a == b {
            return false;
        }

        if colors.get(&a) == colors.get(&b) {
            return false;
        }
    }

    true
}
```

Rules:

```text
colors[node] = assigned color index
color_count = number of colors used
color indices are implementation output, not stable semantic labels
renumber/sort if deterministic presentation required
```

---

## 18.17 Capacity and optimization-model boundaries

### Matching

```text
Petgraph matching:
    greedy valid matching
    exact maximum-cardinality matching
    no weighted matching
    no bipartite-cost assignment
    no stable matching/preferences
    no dynamic matching maintenance
```

Use specialized optimization crates/solvers when:

```text
edge weights/profits matter
assignment costs matter
constraints beyond endpoint-disjointness exist
incremental updates require maintained solution
proof of optimality/certificates required
```

### Flow

```text
Petgraph flow:
    max s-t flow
    positive capacities as edge weights
    edge-flow vector result
    no costs/lower bounds/multi-commodity constraints
```

Use specialized crates/solvers when:

```text
min-cost max-flow
transportation problem
lower/upper bounds
side constraints
integer programming
multi-source/multi-sink with costs
industrial scheduling/planning
```

### Cliques

```text
Petgraph cliques:
    all maximal cliques
    exponential worst-case
    memory returns Vec<HashSet<NodeId>>
```

Use specialized crates/solvers when:

```text
maximum clique only on large graph
branch-and-bound tuning required
parallel clique enumeration required
streaming enumeration required
constraint-specific pruning required
```

### Coloring

```text
Petgraph coloring:
    DSatur heuristic
    valid coloring, not guaranteed chromatic minimum
```

Use specialized crates/solvers when:

```text
minimum coloring required
list coloring / weighted coloring
precolored constraints
timetabling-style constraints
proof of optimality needed
```

---

## 18.18 Algorithm trait-bound matrix

| API                | Key bounds / constraints                                                                                                                |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------- |
| `greedy_matching`  | `Visitable + IntoNodeIdentifiers + NodeIndexable + IntoNeighbors`; graph treated undirected                                             |
| `maximum_matching` | `Visitable + NodeIndexable + IntoNodeIdentifiers + IntoEdges`; graph treated undirected                                                 |
| `Matching`         | `GraphBase`; methods need `NodeIndexable` / `NodeCount` depending method                                                                |
| `dinics`           | `NodeCount + EdgeCount + IntoEdgesDirected + EdgeIndexable + NodeIndexable + Visitable`; edge weights `PositiveMeasure + Sub`           |
| `ford_fulkerson`   | `NodeCount + EdgeCount + IntoEdgesDirected + EdgeIndexable + NodeIndexable + DataMap + Visitable`; edge weights `PositiveMeasure + Sub` |
| `maximal_cliques`  | `GetAdjacencyMatrix + IntoNodeIdentifiers + IntoNeighbors`; undirected/symmetric directed                                               |
| `dsatur_coloring`  | `IntoEdges + IntoNodeIdentifiers + Visitable + NodeIndexable`; undirected loop-free                                                     |

These bounds are from the public function signatures and matter for graph-family compatibility; for example, max-flow functions need `EdgeIndexable`, `IntoEdgesDirected`, and `NodeIndexable`, while clique enumeration requires adjacency-matrix access via `GetAdjacencyMatrix`. ([Docs.rs][6])

---

## 18.19 Practical use cases

### Matching: task pairing

```text
Use:
    maximum_matching

Model:
    undirected compatibility graph
    worker/job vertices
    edge = feasible pairing

Output:
    matched pairs via matching.edges()
```

### Flow: network throughput

```text
Use:
    dinics

Model:
    directed capacity graph
    source = supply
    sink = demand
    edge weight = capacity

Output:
    max_flow
    per-edge flow allocation
```

### Cliques: fully compatible groups

```text
Use:
    maximal_cliques

Model:
    undirected compatibility graph
    edge = pairwise compatibility

Output:
    maximal groups where every pair is compatible
```

### Coloring: resource conflict assignment

```text
Use:
    dsatur_coloring

Model:
    undirected conflict graph
    edge = cannot share same resource/time/register
    color = assigned slot/resource/register
```

---

## 18.20 Anti-pattern inventory

```text
Anti-pattern:
    Use greedy_matching and assume maximum cardinality.
Problem:
    greedy only guarantees valid matching.
Fix:
    maximum_matching.

Anti-pattern:
    Use maximum_matching for weighted assignment.
Problem:
    maximum cardinality, not min-cost/max-weight.
Fix:
    specialized assignment/min-cost-flow/optimization solver.

Anti-pattern:
    Use flow algorithms on domain edge structs.
Problem:
    capacity is EdgeWeight directly; no closure.
Fix:
    map/project graph to numeric capacity weights.

Anti-pattern:
    Interpret max-flow output vector after graph mutation.
Problem:
    flow vector is indexed by graph edge indices from algorithm call.
Fix:
    inspect immediately or map to stable domain edge IDs.

Anti-pattern:
    Run maximal_cliques on unbounded user graph.
Problem:
    exponential output/time/memory.
Fix:
    prefilter, cap size, use specialized solver.

Anti-pattern:
    Use dsatur_coloring as proof of chromatic number.
Problem:
    DSatur function is documented as heuristic, not minimum.
Fix:
    exact coloring solver when minimum is required.

Anti-pattern:
    Run dsatur_coloring on graph with self-loops.
Problem:
    docs require loop-free undirected graph.
Fix:
    reject/self-loop-clean before coloring.

Anti-pattern:
    Use directed graph for matching/coloring/cliques without normalizing.
Problem:
    APIs treat matching/coloring/cliques as undirected-style problems.
Fix:
    build UnGraph or symmetric directed representation.
```

---

## 18.21 Production checklist

```text
Before matching:
    decide greedy vs exact maximum
    normalize to undirected compatibility graph
    decide whether weights matter
    validate output with Matching APIs

Before flow:
    project capacity to numeric EdgeWeight
    reject negative / non-comparable / NaN capacities
    keep graph immutable until flow vector interpreted
    choose dinics by default; benchmark ford_fulkerson on small/sparse graphs

Before clique enumeration:
    bound graph size
    ensure undirected/symmetric relation
    prefilter inactive nodes/edges
    expect exponential output

Before coloring:
    remove/reject self-loops
    use undirected conflict graph
    validate coloring if graph generated from untrusted data
    do not claim optimal color count
```

Final rule:

```text
Petgraph is enough when:
    standard graph algorithm output is the product
    constraints match the documented algorithm model
    graph sizes fit documented complexity
    heuristic output is acceptable where documented

Use specialized optimization crates/solvers when:
    weights, costs, constraints, certificates, dynamic updates, or exact optimality exceed petgraph's algorithm contract
```

[1]: https://docs.rs/petgraph/latest/petgraph/algo/index.html "petgraph::algo - Rust"
[2]: https://docs.rs/petgraph/latest/petgraph/algo/maximum_flow/index.html?utm_source=chatgpt.com "petgraph::algo::maximum_flow - Rust"
[3]: https://docs.rs/petgraph/latest/petgraph/algo/matching/fn.greedy_matching.html "greedy_matching in petgraph::algo::matching - Rust"
[4]: https://docs.rs/petgraph/latest/petgraph/algo/matching/fn.maximum_matching.html?search=&utm_source=chatgpt.com "\"\" Search - Rust"
[5]: https://docs.rs/petgraph/latest/petgraph/algo/matching/struct.Matching.html?utm_source=chatgpt.com "Matching in petgraph::algo::matching - Rust"
[6]: https://docs.rs/petgraph/latest/petgraph/algo/maximum_flow/fn.dinics.html "dinics in petgraph::algo::maximum_flow - Rust"
[7]: https://docs.rs/petgraph/latest/petgraph/algo/maximum_flow/fn.ford_fulkerson.html "ford_fulkerson in petgraph::algo::maximum_flow - Rust"
[8]: https://docs.rs/petgraph/latest/petgraph/algo/maximal_cliques/fn.maximal_cliques.html "maximal_cliques in petgraph::algo::maximal_cliques - Rust"
[9]: https://docs.rs/petgraph/latest/petgraph/algo/coloring/fn.dsatur_coloring.html "dsatur_coloring in petgraph::algo::coloring - Rust"


# 19) Algorithm catalog — isomorphism and subgraph matching

Format follows the uploaded advanced-reference style. 

Petgraph’s isomorphism APIs live under `petgraph::algo::isomorphism` and use the **VF2 algorithm**. The module exposes structural graph isomorphism, semantic node/edge matching, induced subgraph isomorphism, and an iterator over subgraph-isomorphism mappings. All documented isomorphism APIs warn that the input graphs should **not** be multigraphs. ([Docs.rs][1])

---

## 19.0 Core imports

```rust
use petgraph::algo::isomorphism::{
    is_isomorphic,
    is_isomorphic_matching,
    is_isomorphic_subgraph,
    is_isomorphic_subgraph_matching,
    subgraph_isomorphisms_iter,
};

use petgraph::graph::{
    Graph,
    DiGraph,
    UnGraph,
    NodeIndex,
};

use petgraph::visit::{
    NodeCompactIndexable,
    EdgeCount,
    DataMap,
    GetAdjacencyMatrix,
    GraphProp,
    IntoEdgesDirected,
};

use petgraph::Directed;
use petgraph::Undirected;
```

Core decision:

```text
same graph shape only, ignore weights:
    is_isomorphic

same graph shape + node/edge semantic equivalence:
    is_isomorphic_matching

pattern graph occurs as induced subgraph of target, ignore weights:
    is_isomorphic_subgraph

pattern graph occurs as induced subgraph of target + semantic equivalence:
    is_isomorphic_subgraph_matching

need all/first actual mappings:
    subgraph_isomorphisms_iter
```

---

## 19.1 Mental model: syntactic vs semantic isomorphism

```text
syntactic isomorphism:
    topology only
    node weights ignored
    edge weights ignored
    graph directedness must match
    adjacency structure must match

semantic isomorphism:
    topology must match
    node_match(&N0, &N1) must be true for mapped nodes
    edge_match(&E0, &E1) must be true for mapped edges
```

Petgraph’s `is_isomorphic` checks only graph structure with VF2, while `is_isomorphic_matching` checks structure plus node/edge-weight predicates. Both require matching directedness through `GraphProp<EdgeType = G0::EdgeType>` on the second graph. ([Docs.rs][2])

---

## 19.2 Trait-bound contract

The isomorphism functions are intentionally trait-generic, but the bound set is strict:

```text
required for structural isomorphism:
    NodeCompactIndexable
    EdgeCount
    GetAdjacencyMatrix
    GraphProp
    IntoNeighborsDirected

additional for semantic matching:
    DataMap
    IntoEdgesDirected
    node_match: FnMut(&G0::NodeWeight, &G1::NodeWeight) -> bool
    edge_match: FnMut(&G0::EdgeWeight, &G1::EdgeWeight) -> bool
```

`is_isomorphic` requires `NodeCompactIndexable + EdgeCount + GetAdjacencyMatrix + GraphProp + IntoNeighborsDirected`; matching variants additionally require `DataMap` and `IntoEdgesDirected` so node and edge weights can be read for semantic predicates. ([Docs.rs][2])

Practical graph-family implications:

```text
Graph:
    good default

UnGraph / DiGraph aliases:
    good default

GraphMap:
    often usable if trait bounds satisfied; node IDs are node weights/keys

MatrixGraph:
    adjacency-matrix representation can satisfy adjacency requirements

Csr:
    trait coverage may be limiting for directed matching APIs

StableGraph:
    often excluded by NodeCompactIndexable after deletions / hole semantics
```

Agent rule:

```text
If isomorphism APIs fail trait bounds:
    materialize/compact into Graph<N,E,Ty,Ix>
    or remap live StableGraph nodes into a fresh Graph
    or avoid APIs requiring NodeCompactIndexable
```

---

## 19.3 Multigraph restriction

All documented isomorphism functions say the graphs should not be multigraphs. In petgraph terms, this is especially relevant for `Graph` / `StableGraph`, because `add_edge` permits parallel edges. ([Docs.rs][2])

Bad input model:

```rust
use petgraph::graph::DiGraph;

let mut g = DiGraph::<&str, ()>::new();

let a = g.add_node("a");
let b = g.add_node("b");

g.add_edge(a, b, ());
g.add_edge(a, b, ()); // parallel edge: avoid for VF2 isomorphism APIs
```

Safe simple-graph construction:

```rust
use petgraph::graph::DiGraph;

let mut g = DiGraph::<&str, ()>::new();

let a = g.add_node("a");
let b = g.add_node("b");

g.update_edge(a, b, ()); // one logical endpoint-pair edge
g.update_edge(a, b, ()); // overwrites/updates, no duplicate
```

Graph family choice:

```text
Need simple graph by construction:
    GraphMap
    MatrixGraph
    Csr

Using Graph / StableGraph:
    use update_edge, not add_edge
    or deduplicate before matching
```

---

## 19.4 `is_isomorphic` — structural full-graph isomorphism

Signature shape:

```rust
pub fn is_isomorphic<G0, G1>(g0: G0, g1: G1) -> bool
where
    G0: NodeCompactIndexable
        + EdgeCount
        + GetAdjacencyMatrix
        + GraphProp
        + IntoNeighborsDirected,
    G1: NodeCompactIndexable
        + EdgeCount
        + GetAdjacencyMatrix
        + GraphProp<EdgeType = G0::EdgeType>
        + IntoNeighborsDirected;
```

Semantics:

```text
Input:
    two graphs with same directedness type

Checks:
    topology only
    node weights ignored
    edge weights ignored

Output:
    true if bijection exists between all nodes preserving adjacency

Restriction:
    non-multigraph input expected
```

Docs: `is_isomorphic` returns `true` if two graphs are isomorphic, using VF2 and matching only syntactic graph structure. ([Docs.rs][2])

Example: same path, different labels.

```rust
use petgraph::algo::isomorphism::is_isomorphic;
use petgraph::graph::UnGraph;

let g0 = UnGraph::<&str, ()>::from_edges([
    (0, 1),
    (1, 2),
]);

let g1 = UnGraph::<&str, ()>::from_edges([
    (10, 11),
    (11, 12),
]);

assert!(is_isomorphic(&g0, &g1));
```

Example: different topology.

```rust
use petgraph::algo::isomorphism::is_isomorphic;
use petgraph::graph::UnGraph;

let path = UnGraph::<(), ()>::from_edges([
    (0, 1),
    (1, 2),
]);

let triangle = UnGraph::<(), ()>::from_edges([
    (0, 1),
    (1, 2),
    (2, 0),
]);

assert!(!is_isomorphic(&path, &triangle));
```

Use when:

```text
topology identity matters
payloads should be ignored
graph canonicalization/checking test fixtures
workflow structure equivalence
control-flow shape comparison
```

Avoid when:

```text
labels/types/edge kinds matter
node/edge weights encode semantics
pattern is smaller than target
all mappings are needed
```

---

## 19.5 `is_isomorphic_matching` — full-graph isomorphism with node/edge predicates

Signature shape:

```rust
pub fn is_isomorphic_matching<G0, G1, NM, EM>(
    g0: G0,
    g1: G1,
    node_match: NM,
    edge_match: EM,
) -> bool
where
    G0: NodeCompactIndexable
        + EdgeCount
        + DataMap
        + GetAdjacencyMatrix
        + GraphProp
        + IntoEdgesDirected,
    G1: NodeCompactIndexable
        + EdgeCount
        + DataMap
        + GetAdjacencyMatrix
        + GraphProp<EdgeType = G0::EdgeType>
        + IntoEdgesDirected,
    NM: FnMut(&G0::NodeWeight, &G1::NodeWeight) -> bool,
    EM: FnMut(&G0::EdgeWeight, &G1::EdgeWeight) -> bool;
```

Docs: `is_isomorphic_matching` performs VF2 graph isomorphism over both topology and semantic node/edge-weight predicates. ([Docs.rs][3])

Example: exact labels.

```rust
use petgraph::algo::isomorphism::is_isomorphic_matching;
use petgraph::graph::UnGraph;

let mut g0 = UnGraph::<&str, &str>::new_undirected();
let a0 = g0.add_node("service");
let b0 = g0.add_node("db");
g0.update_edge(a0, b0, "tcp");

let mut g1 = UnGraph::<&str, &str>::new_undirected();
let a1 = g1.add_node("service");
let b1 = g1.add_node("db");
g1.update_edge(a1, b1, "tcp");

let ok = is_isomorphic_matching(
    &g0,
    &g1,
    |n0, n1| n0 == n1,
    |e0, e1| e0 == e1,
);

assert!(ok);
```

Example: domain-equivalence labels.

```rust
#[derive(Clone, Debug)]
struct Service {
    kind: &'static str,
    region: &'static str,
}

#[derive(Clone, Debug)]
struct Link {
    protocol: &'static str,
    encrypted: bool,
}

let equivalent = is_isomorphic_matching(
    &g0,
    &g1,
    |a: &Service, b: &Service| {
        a.kind == b.kind
        // region deliberately ignored
    },
    |a: &Link, b: &Link| {
        a.protocol == b.protocol && a.encrypted == b.encrypted
    },
);
```

Use when:

```text
same topology required
payload equivalence matters
node labels / kinds / atom types / workflow steps matter
edge labels / bond types / relation kinds matter
```

Avoid when:

```text
only topology matters
pattern graph should match inside target graph
predicate is expensive and no prefiltering exists
```

---

## 19.6 Node and edge matching closures

Closure contracts:

```text
node_match:
    (&G0::NodeWeight, &G1::NodeWeight) -> bool

edge_match:
    (&G0::EdgeWeight, &G1::EdgeWeight) -> bool
```

Exact semantic equality:

```rust
|a, b| a == b
```

Ignore payloads in matching API:

```rust
|_, _| true
```

Coarse category equivalence:

```rust
|a: &Service, b: &Service| a.kind == b.kind
```

Tolerance equivalence:

```rust
|a: &Metric, b: &Metric| {
    (a.value - b.value).abs() <= 0.001
}
```

Edge-direction-aware semantic matching is **not** directly represented in the closure arguments; the closure receives edge weights, while topology/orientation feasibility is handled by the VF2 algorithm through directed adjacency. The internal source retrieves edge weights using `edges_directed(..., Outgoing)` when semantic edge matching is enabled. ([Docs.rs][4])

Performance rule:

```text
Keep closures:
    pure
    deterministic
    cheap
    allocation-free
    side-effect-free where possible

Avoid:
    DB lookups
    string normalization per call
    regex compilation per call
    logging inside predicate
    mutation-dependent behavior
```

Precompute expensive equivalence:

```rust
#[derive(Clone, Debug)]
struct NodeData {
    raw_label: String,
    normalized_kind: u32,
}

let ok = is_isomorphic_matching(
    &g0,
    &g1,
    |a: &NodeData, b: &NodeData| a.normalized_kind == b.normalized_kind,
    |_, _| true,
);
```

---

## 19.7 `is_isomorphic_subgraph` — structural induced-subgraph existence

Signature shape:

```rust
pub fn is_isomorphic_subgraph<G0, G1>(g0: G0, g1: G1) -> bool
where
    G0: NodeCompactIndexable
        + EdgeCount
        + GetAdjacencyMatrix
        + GraphProp
        + IntoNeighborsDirected,
    G1: NodeCompactIndexable
        + EdgeCount
        + GetAdjacencyMatrix
        + GraphProp<EdgeType = G0::EdgeType>
        + IntoNeighborsDirected;
```

Semantics:

```text
Input:
    g0 = pattern graph
    g1 = target graph

Checks:
    whether g0 is isomorphic to a node-induced subgraph of g1

Output:
    true / false

Payloads:
    ignored
```

Petgraph clarifies that “subgraph” in these APIs means a **node-induced subgraph**; edge-induced subgraph isomorphisms are not directly supported, and non-induced subgraph matching corresponds more closely to monomorphism terminology. ([Docs.rs][5])

Example: path pattern in triangle target.

```rust
use petgraph::algo::isomorphism::is_isomorphic_subgraph;
use petgraph::graph::UnGraph;

let pattern = UnGraph::<(), ()>::from_edges([
    (0, 1),
    (1, 2),
]);

let target = UnGraph::<(), ()>::from_edges([
    (0, 1),
    (1, 2),
    (2, 0),
]);

// For induced subgraph semantics, a 3-node path is NOT an induced
// subgraph of a triangle on those same 3 nodes because the extra edge exists.
assert!(!is_isomorphic_subgraph(&pattern, &target));
```

Example: path pattern inside larger target where induced node set works.

```rust
let pattern = UnGraph::<(), ()>::from_edges([
    (0, 1),
    (1, 2),
]);

let target = UnGraph::<(), ()>::from_edges([
    (0, 1),
    (1, 2),
    (3, 4),
]);

assert!(is_isomorphic_subgraph(&pattern, &target));
```

Use when:

```text
pattern topology only
target may be larger
node-induced semantics are correct
existence boolean is enough
```

Avoid when:

```text
edge-induced or non-induced matching required
labels/weights matter
actual mapping required
target graph is a multigraph
```

---

## 19.8 `is_isomorphic_subgraph_matching` — semantic induced-subgraph existence

Signature shape:

```rust
pub fn is_isomorphic_subgraph_matching<G0, G1, NM, EM>(
    g0: G0,
    g1: G1,
    node_match: NM,
    edge_match: EM,
) -> bool
where
    G0: NodeCompactIndexable
        + EdgeCount
        + DataMap
        + GetAdjacencyMatrix
        + GraphProp
        + IntoEdgesDirected,
    G1: NodeCompactIndexable
        + EdgeCount
        + DataMap
        + GetAdjacencyMatrix
        + GraphProp<EdgeType = G0::EdgeType>
        + IntoEdgesDirected,
    NM: FnMut(&G0::NodeWeight, &G1::NodeWeight) -> bool,
    EM: FnMut(&G0::EdgeWeight, &G1::EdgeWeight) -> bool;
```

Docs: this function returns `true` if `g0` is isomorphic to a subgraph of `g1`, using VF2 over both graph structure and node/edge-weight predicates. ([Docs.rs][6])

Pattern detection example:

```rust
use petgraph::algo::isomorphism::is_isomorphic_subgraph_matching;
use petgraph::graph::DiGraph;

#[derive(Clone, Debug)]
struct Step {
    kind: &'static str,
}

#[derive(Clone, Debug)]
struct Flow {
    kind: &'static str,
}

let mut pattern = DiGraph::<Step, Flow>::new();
let p_extract = pattern.add_node(Step { kind: "extract" });
let p_transform = pattern.add_node(Step { kind: "transform" });
pattern.update_edge(p_extract, p_transform, Flow { kind: "data" });

let mut workflow = DiGraph::<Step, Flow>::new();
let w_extract = workflow.add_node(Step { kind: "extract" });
let w_transform = workflow.add_node(Step { kind: "transform" });
let w_load = workflow.add_node(Step { kind: "load" });

workflow.update_edge(w_extract, w_transform, Flow { kind: "data" });
workflow.update_edge(w_transform, w_load, Flow { kind: "data" });

let found = is_isomorphic_subgraph_matching(
    &pattern,
    &workflow,
    |a, b| a.kind == b.kind,
    |a, b| a.kind == b.kind,
);

assert!(found);
```

Use when:

```text
pattern labels matter
edge kinds matter
target graph is larger
node-induced semantics match domain
boolean existence is enough
```

Avoid when:

```text
need all matches
need first mapping
need non-induced matching
need edge-induced matching
```

---

## 19.9 `subgraph_isomorphisms_iter` — mappings for semantic induced-subgraph matches

Signature shape:

```rust
pub fn subgraph_isomorphisms_iter<'a, G0, G1, NM, EM>(
    g0: &'a G0,
    g1: &'a G1,
    node_match: &'a mut NM,
    edge_match: &'a mut EM,
) -> Option<impl Iterator<Item = Vec<usize>> + 'a>
where
    G0: 'a
        + NodeCompactIndexable
        + EdgeCount
        + DataMap
        + GetAdjacencyMatrix
        + GraphProp
        + IntoEdgesDirected,
    G1: 'a
        + NodeCompactIndexable
        + EdgeCount
        + DataMap
        + GetAdjacencyMatrix
        + GraphProp<EdgeType = G0::EdgeType>
        + IntoEdgesDirected,
    NM: 'a + FnMut(&G0::NodeWeight, &G1::NodeWeight) -> bool,
    EM: 'a + FnMut(&G0::EdgeWeight, &G1::EdgeWeight) -> bool;
```

This API returns `None` when no subgraph isomorphism exists; otherwise it returns an iterator over mappings. Each mapping is a `Vec<usize>`; the internal state stores mappings from `G0` node indices to `G1` node indices, with `usize::MAX` used internally for unmapped entries, and the iterator item type is `Vec<usize>`. ([Docs.rs][7])

Basic mapping extraction:

```rust
use petgraph::algo::isomorphism::subgraph_isomorphisms_iter;
use petgraph::graph::UnGraph;
use petgraph::visit::NodeIndexable;

let pattern = UnGraph::<&str, ()>::from_edges([
    (0, 1),
]);

let target = UnGraph::<&str, ()>::from_edges([
    (0, 1),
    (2, 3),
]);

let mut node_match = |a: &&str, b: &&str| a == b;
let mut edge_match = |_: &(), _: &()| true;

if let Some(iter) = subgraph_isomorphisms_iter(
    &pattern,
    &target,
    &mut node_match,
    &mut edge_match,
) {
    for mapping in iter {
        // mapping[pattern.to_index(pattern_node)] = target node index as usize
        println!("mapping = {mapping:?}");
    }
}
```

Mapping-to-`NodeIndex` helper for `Graph`-style targets:

```rust
use petgraph::graph::NodeIndex;
use petgraph::visit::NodeIndexable;

fn decode_mapping<G0, G1>(
    g0: &G0,
    g1: &G1,
    mapping: &[usize],
) -> Vec<(G0::NodeId, G1::NodeId)>
where
    G0: NodeCompactIndexable,
    G1: NodeCompactIndexable,
{
    g0
        .node_identifiers()
        .map(|n0| {
            let i0 = g0.to_index(n0);
            let i1 = mapping[i0];
            (n0, g1.from_index(i1))
        })
        .collect()
}
```

Use when:

```text
need actual match location(s)
need all candidate mappings
pattern detection should return bindings
downstream code highlights matched subgraph
domain needs proof/certificate of match
```

Avoid when:

```text
boolean existence is enough
target is large and number of matches may explode
all matches cannot fit time/memory budget
non-induced matching is required
```

---

## 19.10 Exact matching vs domain-equivalence matching

### Exact payload equality

```rust
let ok = is_isomorphic_matching(
    &g0,
    &g1,
    |n0, n1| n0 == n1,
    |e0, e1| e0 == e1,
);
```

Use when:

```text
node labels must match exactly
edge labels must match exactly
graphs are normalized into canonical payloads
```

### Domain-equivalence matching

```rust
let ok = is_isomorphic_matching(
    &g0,
    &g1,
    |a: &Service, b: &Service| {
        a.kind == b.kind
            && a.version_major == b.version_major
    },
    |a: &Dependency, b: &Dependency| {
        a.protocol == b.protocol
            && a.required == b.required
    },
);
```

Use when:

```text
payloads differ but are semantically equivalent
version ranges / categories / classes matter
exact raw value equality is too strict
```

### Topology-only through matching API

```rust
let ok = is_isomorphic_matching(
    &g0,
    &g1,
    |_, _| true,
    |_, _| true,
);
```

Prefer `is_isomorphic` unless you need matching API shape for generic code.

---

## 19.11 Induced-subgraph semantics: critical caveat

Petgraph’s subgraph isomorphism APIs use **node-induced subgraph** semantics. If `g0` maps to a set of nodes in `g1`, every edge among those mapped target nodes must correspond to an edge in `g0`; extra target edges among the chosen node set can break the match. Edge-induced subgraph isomorphism is not directly supported. ([Docs.rs][5])

Implications:

```text
Pattern path A-B-C:
    does not match triangle X-Y-Z as induced 3-node subgraph
    because target has extra X-Z edge

Pattern triangle:
    can match triangle target

Non-induced path search:
    not directly supported by these APIs
    requires custom matcher / graph transformation / monomorphism-capable library
```

Workaround for non-induced relationship:

```text
Option A:
    transform target by deleting edges irrelevant to the pattern predicate

Option B:
    encode “extra edges allowed” as a different search problem

Option C:
    use specialized subgraph monomorphism/isomorphism library

Option D:
    implement custom backtracking over candidates with edge-presence lower-bound checks only
```

---

## 19.12 Directedness and graph-shape compatibility

The second graph must have the same `EdgeType` as the first graph in all documented isomorphism signatures. Directed graphs are matched with directed graphs; undirected graphs are matched with undirected graphs. ([Docs.rs][2])

```rust
use petgraph::graph::{DiGraph, UnGraph};
use petgraph::algo::isomorphism::is_isomorphic;

let dg = DiGraph::<(), ()>::from_edges([(0, 1)]);
let ug = UnGraph::<(), ()>::from_edges([(0, 1)]);

// This is a type-level mismatch in the API bounds:
// is_isomorphic(&dg, &ug)
```

Conversion strategy:

```rust
use petgraph::Undirected;

let ug_from_dg = dg.into_edge_type::<Undirected>();
```

Caveat:

```text
into_edge_type does not deduplicate edges
directed reciprocal edges can become parallel edges
isomorphism APIs expect non-multigraphs
deduplicate after conversion if needed
```

---

## 19.13 Pre-filtering and pruning strategies

Subgraph isomorphism is a known NP-hard problem, and practical performance depends heavily on reducing candidate mappings before invoking exhaustive search. ([arXiv][8])

Cheap prefilters:

```text
Full isomorphism:
    node_count equal
    edge_count equal
    directedness equal
    degree sequence equal
    node-label histogram equal
    edge-label histogram equal
    connected component count equal if applicable

Subgraph isomorphism:
    pattern.node_count <= target.node_count
    pattern.edge_count <= target.edge_count
    every pattern node label count <= target label count
    every pattern degree lower bound satisfiable by candidate target node
    connected-component constraints checked early
```

Degree histogram prefilter:

```rust
use std::collections::HashMap;
use petgraph::visit::IntoNodeIdentifiers;

fn degree_histogram<N, E, Ty, Ix>(
    g: &petgraph::Graph<N, E, Ty, Ix>,
) -> HashMap<usize, usize>
where
    Ty: petgraph::EdgeType,
    Ix: petgraph::graph::IndexType,
{
    let mut hist = HashMap::new();

    for n in g.node_indices() {
        *hist.entry(g.neighbors(n).count()).or_insert(0) += 1;
    }

    hist
}
```

Label histogram prefilter:

```rust
use std::collections::HashMap;
use petgraph::graph::Graph;

fn node_label_histogram<N, E, Ty, Ix, K>(
    g: &Graph<N, E, Ty, Ix>,
    key: impl Fn(&N) -> K,
) -> HashMap<K, usize>
where
    Ty: petgraph::EdgeType,
    Ix: petgraph::graph::IndexType,
    K: Eq + std::hash::Hash,
{
    let mut hist = HashMap::new();

    for n in g.node_weights() {
        *hist.entry(key(n)).or_insert(0) += 1;
    }

    hist
}
```

Candidate-filter closure:

```rust
let mut node_match = |p: &PatternNode, t: &TargetNode| {
    p.kind == t.kind
        && p.min_degree <= t.degree_hint
        && p.required_flags.is_subset(t.flags)
};
```

Rule:

```text
Put cheap, selective predicates into node_match.
Precompute expensive predicate inputs into node/edge weights.
Do not allocate or normalize inside node_match / edge_match.
```

---

## 19.14 Graph preparation for isomorphism APIs

### Deduplicate edge pairs

```rust
use std::collections::HashSet;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;

fn simple_projection<N: Clone, E: Clone>(
    g: &DiGraph<N, E>,
) -> DiGraph<N, E> {
    let mut out = DiGraph::<N, E>::with_capacity(g.node_count(), g.edge_count());

    let nodes: Vec<_> = g.node_indices()
        .map(|n| out.add_node(g[n].clone()))
        .collect();

    let mut seen = HashSet::new();

    for e in g.edge_references() {
        let key = (e.source().index(), e.target().index());

        if seen.insert(key) {
            out.add_edge(nodes[key.0], nodes[key.1], e.weight().clone());
        }
    }

    out
}
```

### Compact `StableGraph` before matching

```rust
use petgraph::stable_graph::StableDiGraph;
use petgraph::graph::DiGraph;
use petgraph::visit::EdgeRef;

fn compact_stable<N: Clone, E: Clone>(
    sg: &StableDiGraph<N, E>,
) -> DiGraph<N, E> {
    let mut out = DiGraph::<N, E>::with_capacity(
        sg.node_count(),
        sg.edge_count(),
    );

    let mut old_to_new = std::collections::HashMap::new();

    for n in sg.node_indices() {
        let new = out.add_node(sg[n].clone());
        old_to_new.insert(n, new);
    }

    for e in sg.edge_references() {
        let a = old_to_new[&e.source()];
        let b = old_to_new[&e.target()];
        out.add_edge(a, b, e.weight().clone());
    }

    out
}
```

Reason:

```text
isomorphism APIs require NodeCompactIndexable.
StableGraph can have holes.
A compact Graph projection is the safest compatibility adapter.
```

---

## 19.15 Use case: workflow graph comparison

Model:

```rust
#[derive(Clone, Debug)]
struct Step {
    kind: &'static str,
    normalized_config_hash: u64,
}

#[derive(Clone, Debug)]
struct DataEdge {
    medium: &'static str,
}
```

Full workflow equivalence:

```rust
use petgraph::algo::isomorphism::is_isomorphic_matching;
use petgraph::graph::DiGraph;

fn same_workflow_shape(
    a: &DiGraph<Step, DataEdge>,
    b: &DiGraph<Step, DataEdge>,
) -> bool {
    is_isomorphic_matching(
        a,
        b,
        |x, y| {
            x.kind == y.kind
                && x.normalized_config_hash == y.normalized_config_hash
        },
        |x, y| x.medium == y.medium,
    )
}
```

Pattern detection:

```rust
fn contains_extract_transform(
    pattern: &DiGraph<Step, DataEdge>,
    workflow: &DiGraph<Step, DataEdge>,
) -> bool {
    is_isomorphic_subgraph_matching(
        pattern,
        workflow,
        |p, w| p.kind == w.kind,
        |p, w| p.medium == w.medium,
    )
}
```

Value case:

```text
CI regression:
    workflow graph should not structurally change

Template matching:
    ETL pattern present in larger workflow

Normalization:
    compare config hashes, not raw config text
```

---

## 19.16 Use case: chemistry-like graph matching

Model:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
enum Element {
    C,
    N,
    O,
    H,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Bond {
    Single,
    Double,
    Aromatic,
}

type Molecule = petgraph::graph::UnGraph<Element, Bond>;
```

Molecule exact isomorphism:

```rust
use petgraph::algo::isomorphism::is_isomorphic_matching;

fn same_molecule(a: &Molecule, b: &Molecule) -> bool {
    is_isomorphic_matching(
        a,
        b,
        |ea, eb| ea == eb,
        |ba, bb| ba == bb,
    )
}
```

Substructure search:

```rust
use petgraph::algo::isomorphism::is_isomorphic_subgraph_matching;

fn contains_substructure(pattern: &Molecule, target: &Molecule) -> bool {
    is_isomorphic_subgraph_matching(
        pattern,
        target,
        |p, t| p == t,
        |p, t| p == t,
    )
}
```

Caveat:

```text
Chemical substructure search may require:
    non-induced matching
    aromaticity normalization
    charge/isotope/stereochemistry semantics
    multiple bond interpretations
    domain-specific pruning

Petgraph is useful for small/medium exact experiments.
Specialized cheminformatics toolkits are better for production chemistry search.
```

---

## 19.17 Use case: typed pattern detection

Pattern graph:

```rust
#[derive(Clone, Debug)]
enum NodeKind {
    Source,
    Transform,
    Sink,
}

#[derive(Clone, Debug)]
enum EdgeKind {
    Data,
    Control,
}

type TypedGraph = petgraph::graph::DiGraph<NodeKind, EdgeKind>;
```

Detection:

```rust
use petgraph::algo::isomorphism::subgraph_isomorphisms_iter;
use petgraph::visit::NodeIndexable;

fn find_pattern(
    pattern: &TypedGraph,
    target: &TypedGraph,
) -> Vec<Vec<(petgraph::graph::NodeIndex, petgraph::graph::NodeIndex)>> {
    let mut node_match = |a: &NodeKind, b: &NodeKind| a == b;
    let mut edge_match = |a: &EdgeKind, b: &EdgeKind| a == b;

    let Some(iter) = subgraph_isomorphisms_iter(
        pattern,
        target,
        &mut node_match,
        &mut edge_match,
    ) else {
        return Vec::new();
    };

    iter.map(|mapping| {
        pattern
            .node_indices()
            .map(|p_node| {
                let target_raw = mapping[pattern.to_index(p_node)];
                let t_node = target.from_index(target_raw);
                (p_node, t_node)
            })
            .collect()
    })
    .collect()
}
```

Value case:

```text
static analysis:
    detect known anti-pattern subgraphs

security:
    detect forbidden flow motifs

data lineage:
    find template pipeline fragments

workflow engines:
    find canonical sub-DAG shapes
```

---

## 19.18 Performance caveats

```text
Worst-case:
    combinatorial / exponential search behavior possible
    subgraph isomorphism is NP-hard
    number of mappings may be huge

Memory:
    adjacency matrix support required by trait bounds
    mapping iterator yields Vec<usize> per match
    collecting all mappings can explode

Closures:
    called frequently
    must be cheap
    precompute normalized fields

Multigraphs:
    not supported by documented assumptions

StableGraph:
    compact before matching if holes exist

Subgraph semantics:
    induced only
    extra target edges between mapped nodes can reject match
```

Pruning priority:

```text
1. reject by node/edge counts
2. reject by directedness
3. reject by simple graph / multigraph check
4. reject by node-label histogram
5. reject by edge-label histogram
6. reject by degree constraints
7. restrict target candidate set
8. run VF2 matcher
9. stop after first match when boolean is enough
```

Boolean-first rule:

```text
Use is_isomorphic_subgraph_matching when:
    existence is enough

Use subgraph_isomorphisms_iter when:
    actual mappings are required

Do not enumerate all mappings:
    just to answer true/false
```

---

## 19.19 API decision matrix

| Need                                        | API                               | Matches weights? | Returns mapping? | Pattern smaller than target? |
| ------------------------------------------- | --------------------------------- | ---------------: | ---------------: | ---------------------------: |
| Same topology                               | `is_isomorphic`                   |               No |               No |                           No |
| Same topology + semantic weights            | `is_isomorphic_matching`          |              Yes |               No |                           No |
| Pattern topology exists in target           | `is_isomorphic_subgraph`          |               No |               No |                          Yes |
| Pattern + semantic weights exists in target | `is_isomorphic_subgraph_matching` |              Yes |               No |                          Yes |
| Actual pattern mappings                     | `subgraph_isomorphisms_iter`      |              Yes |              Yes |                          Yes |

---

## 19.20 Trait-bound matrix

| API                               | Key bounds                                                                                   |
| --------------------------------- | -------------------------------------------------------------------------------------------- |
| `is_isomorphic`                   | `NodeCompactIndexable + EdgeCount + GetAdjacencyMatrix + GraphProp + IntoNeighborsDirected`  |
| `is_isomorphic_matching`          | same plus `DataMap + IntoEdgesDirected`; node/edge closures                                  |
| `is_isomorphic_subgraph`          | same structural bounds as `is_isomorphic`                                                    |
| `is_isomorphic_subgraph_matching` | same semantic bounds as `is_isomorphic_matching`                                             |
| `subgraph_isomorphisms_iter`      | references to both graphs; semantic closures by mutable reference; returns optional iterator |

All five APIs require the target graph’s `EdgeType` to equal the pattern/source graph’s `EdgeType`; this is encoded as `GraphProp<EdgeType = G0::EdgeType>` on `G1`. ([Docs.rs][2])

---

## 19.21 Anti-pattern inventory

```text
Anti-pattern:
    Use is_isomorphic on labeled graphs and expect labels to matter.
Problem:
    topology only.
Fix:
    is_isomorphic_matching.

Anti-pattern:
    Use subgraph API expecting non-induced matching.
Problem:
    petgraph subgraph means node-induced subgraph.
Fix:
    transform graph, implement monomorphism search, or use specialized library.

Anti-pattern:
    Run matching APIs on Graph with parallel edges.
Problem:
    docs say graphs should not be multigraphs.
Fix:
    use update_edge / deduplicate / GraphMap / MatrixGraph / Csr.

Anti-pattern:
    Use StableGraph with holes directly.
Problem:
    NodeCompactIndexable bound excludes hole semantics.
Fix:
    compact into Graph first.

Anti-pattern:
    Put expensive normalization inside node_match.
Problem:
    closure called many times in backtracking search.
Fix:
    precompute normalized fields.

Anti-pattern:
    Collect all mappings from huge target.
Problem:
    output count may explode.
Fix:
    stop early, cap count, prefilter, or specialized engine.

Anti-pattern:
    Ignore directedness.
Problem:
    graph EdgeType must match.
Fix:
    convert intentionally and deduplicate if needed.

Anti-pattern:
    Treat returned Vec<usize> as NodeIndex values blindly.
Problem:
    mapping vector stores target node indices by compact index.
Fix:
    decode with target.from_index(mapping[pattern.to_index(node)]).
```

---

## 19.22 Deployment checklist

```text
Before isomorphism:
    choose full-graph vs subgraph
    choose topology-only vs semantic predicates
    ensure graph directedness matches
    ensure graph is not multigraph
    ensure NodeCompactIndexable compatibility
    prefilter by counts/histograms/degrees
    precompute normalized node/edge fields

For Graph / StableGraph:
    use update_edge to keep simple graph
    compact StableGraph before matching
    avoid deleting nodes during matching

For pattern matching:
    remember induced-subgraph semantics
    choose boolean API unless mappings are required
    cap mapping enumeration in production
    make closures cheap and deterministic

For domain search:
    encode domain equivalence explicitly
    validate results with decoded mappings
    use specialized engines for large-scale, non-induced, chemical, RDF/property-graph, or constraint-rich matching
```

Final rule:

```text
Isomorphism answers:
    “are these graph structures equivalent?”

Semantic isomorphism answers:
    “are these graph structures equivalent under domain predicates?”

Subgraph isomorphism answers:
    “does this pattern appear as a node-induced subgraph?”

Mapping iterator answers:
    “where exactly does this pattern appear?”
```

[1]: https://docs.rs/petgraph/latest/petgraph/algo/isomorphism/index.html "petgraph::algo::isomorphism - Rust"
[2]: https://docs.rs/petgraph/latest/petgraph/algo/isomorphism/fn.is_isomorphic.html "is_isomorphic in petgraph::algo::isomorphism - Rust"
[3]: https://docs.rs/petgraph/latest/petgraph/algo/isomorphism/fn.is_isomorphic_matching.html "is_isomorphic_matching in petgraph::algo::isomorphism - Rust"
[4]: https://docs.rs/petgraph/latest/src/petgraph/algo/isomorphism.rs.html "isomorphism.rs - source"
[5]: https://docs.rs/petgraph/latest/petgraph/algo/isomorphism/fn.is_isomorphic_subgraph.html "is_isomorphic_subgraph in petgraph::algo::isomorphism - Rust"
[6]: https://docs.rs/petgraph/latest/petgraph/algo/isomorphism/fn.is_isomorphic_subgraph_matching.html "is_isomorphic_subgraph_matching in petgraph::algo::isomorphism - Rust"
[7]: https://docs.rs/petgraph/latest/petgraph/algo/isomorphism/fn.subgraph_isomorphisms_iter.html "subgraph_isomorphisms_iter in petgraph::algo::isomorphism - Rust"
[8]: https://arxiv.org/abs/1906.03420?utm_source=chatgpt.com "GSI: GPU-friendly Subgraph Isomorphism"


# 20) Algorithm catalog — graph analytics and specialized routines

Format follows the uploaded advanced-reference style. 

This section covers `page_rank`, `greedy_feedback_arc_set`, `steiner_tree`, transitive reduction/closure via `algo::tred`, dominators for control-flow graphs, analytics workflows, and the limited `rayon`-gated parallel surface. The `algo` module re-exports `page_rank`, `greedy_feedback_arc_set`, `steiner_tree`, `parallel_johnson`, and contains `dominators` and `tred` submodules. ([Docs.rs][1])

---

## 20.0 Core imports

```rust
use petgraph::algo::{
    page_rank,
    greedy_feedback_arc_set,
    steiner_tree,
    is_cyclic_directed,
    toposort,
};

use petgraph::algo::tred::{
    dag_to_toposorted_adjacency_list,
    dag_transitive_reduction_closure,
};

use petgraph::algo::dominators::{
    simple_fast,
    Dominators,
};

use petgraph::graph::{
    Graph,
    DiGraph,
    UnGraph,
    NodeIndex,
    EdgeIndex,
    DefaultIx,
};

use petgraph::stable_graph::StableGraph;

use petgraph::visit::{
    EdgeRef,
    IntoNeighbors,
    NodeIndexable,
};
```

Cargo baseline:

```toml
[dependencies]
petgraph = "0.8.3"
```

Rayon-gated surface:

```toml
[dependencies]
petgraph = { version = "0.8.3", features = ["rayon"] }
rayon = "1"
```

The `rayon` feature enables the `rayon` dependency plus rayon support in `hashbrown` and `indexmap`; it is not enabled by default. ([Docs.rs][2])

---

## 20.1 Decision table

| Need                               | API                       | Graph shape                                   | Output                           | Key limitation                                     |
| ---------------------------------- | ------------------------- | --------------------------------------------- | -------------------------------- | -------------------------------------------------- |
| Node importance / centrality proxy | `page_rank`               | directed graph                                | `Vec<D>` rank by node index      | fixed-iteration PageRank, no convergence tolerance |
| Remove feedback edges to make DAG  | `greedy_feedback_arc_set` | directed graph                                | iterator of edge refs            | heuristic, not minimum                             |
| Approximate Steiner tree           | `steiner_tree`            | connected `UnGraph`                           | `StableGraph<N,E,Undirected,Ix>` | Kou approximation, constrained type bounds         |
| DAG transitive closure/reduction   | `tred::*`                 | DAG, topologically sorted adjacency-list form | `(reduction, closure)`           | DAG-only, specialized input format                 |
| Control-flow dominance             | `dominators::simple_fast` | rooted directed control-flow graph            | `Dominators<NodeId>`             | root-reachable dominance relation                  |
| Parallel all-pairs shortest path   | `parallel_johnson`        | sparse weighted graph                         | all-pairs distance map           | behind `rayon`, not part of this analytics set     |

---

## 20.2 `page_rank` — iterative node ranking

Signature:

```rust
pub fn page_rank<G, D>(
    graph: G,
    damping_factor: D,
    nb_iter: usize,
) -> Vec<D>
where
    G: NodeCount + IntoEdges + NodeIndexable,
    D: UnitMeasure + Copy;
```

`page_rank` computes PageRank scores for every node in a directed graph. It takes a damping factor in `0.0..=1.0`, a fixed iteration count, and returns a `Vec` mapping each node index to its rank; it panics if the damping factor is outside `[0, 1]`. The docs list time complexity as `O(n|V|²|E|)` where `n` is the iteration count, and auxiliary space as `O(|V| + |E|)`. ([Docs.rs][3])

### Basic syntax

```rust
use petgraph::Graph;
use petgraph::algo::page_rank;

let mut g: Graph<(), usize> = Graph::new();

let a = g.add_node(());
let b = g.add_node(());
let c = g.add_node(());
let d = g.add_node(());
let e = g.add_node(());

g.extend_with_edges([
    (a, b),
    (a, d),
    (b, c),
    (b, d),
]);

let damping_factor = 0.85_f64;
let iterations = 20;

let ranks: Vec<f64> = page_rank(&g, damping_factor, iterations);

for node in g.node_indices() {
    println!("{node:?}: rank={}", ranks[node.index()]);
}
```

### Type guidance

```text
D = f32:
    lower memory
    faster on some targets
    less precision

D = f64:
    default recommendation
    better numerical stability
    common analytics choice
```

### Deployment rules

```text
Use page_rank when:
    graph is directed
    relative node importance is useful
    fixed-iteration approximation is acceptable
    node-index-addressed Vec output is acceptable

Avoid / wrap when:
    convergence tolerance is required
    personalization vector is required
    dangling-node behavior must be domain-customized
    graph has holey index semantics and output Vec indexing is untested
```

### Stable output wrapper

```rust
use petgraph::Graph;
use petgraph::algo::page_rank;
use petgraph::graph::NodeIndex;

fn ranked_nodes<N, E>(
    g: &Graph<N, E>,
    damping: f64,
    iterations: usize,
) -> Vec<(NodeIndex, f64)> {
    let ranks = page_rank(g, damping, iterations);

    let mut rows: Vec<_> = g
        .node_indices()
        .map(|n| (n, ranks[n.index()]))
        .collect();

    rows.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    rows
}
```

Agent rule:

```text
PageRank output is index-addressed.
Immediately convert Vec ranks into domain rows:
    NodeIndex -> DomainId -> rank
Do not expose raw rank vector as public API.
```

---

## 20.3 `greedy_feedback_arc_set` — heuristic cycle-breaking edge set

Signature:

```rust
pub fn greedy_feedback_arc_set<G>(
    g: G,
) -> impl Iterator<Item = G::EdgeRef>
where
    G: IntoEdgeReferences + GraphProp<EdgeType = Directed> + NodeCount,
    G::NodeId: GraphIndex;
```

A feedback arc set is a set of directed edges whose removal makes the graph acyclic. Petgraph’s implementation uses a greedy heuristic, does not guarantee a minimum feedback arc set, ignores node/edge weights, always includes self-loops, and latest source docs list `O(|V| + |E|)` time and auxiliary space. ([Docs.rs][4])

### Basic syntax

```rust
use petgraph::{
    algo::{greedy_feedback_arc_set, is_cyclic_directed},
    graph::EdgeIndex,
    stable_graph::StableGraph,
    visit::EdgeRef,
};

let mut g: StableGraph<(), ()> = StableGraph::from_edges([
    (0, 1),
    (1, 2),
    (2, 3),
    (3, 4),
    (4, 5),
    (5, 0),
    (4, 1),
    (1, 3),
]);

assert!(is_cyclic_directed(&g));

let fas: Vec<EdgeIndex> =
    greedy_feedback_arc_set(&g).map(|e| e.id()).collect();

for edge_id in fas {
    g.remove_edge(edge_id);
}

assert!(!is_cyclic_directed(&g));
```

### Edge-removal safety pattern

```rust
use petgraph::algo::greedy_feedback_arc_set;
use petgraph::visit::EdgeRef;

let remove: Vec<_> = greedy_feedback_arc_set(&g)
    .map(|edge_ref| edge_ref.id())
    .collect();

for edge_id in remove {
    g.remove_edge(edge_id);
}
```

Reason:

```text
Collect first:
    iterator borrows graph
    removal mutates graph
    EdgeRef lifetime cannot coexist with mutation
```

### What it does *not* optimize

```text
Not minimum feedback arc set.
Not weighted feedback arc set.
Not domain-priority cycle breaking.
Not stable/reproducible edge preference unless graph iteration order is controlled.
Does not use edge/node weights.
```

### Weighted cycle-breaking recipe

```rust
use petgraph::algo::greedy_feedback_arc_set;
use petgraph::visit::EdgeRef;

#[derive(Clone, Debug)]
struct EdgeData {
    remove_cost: u32,
    relation: &'static str,
}

let candidate_edges: Vec<_> = greedy_feedback_arc_set(&g)
    .map(|e| {
        let id = e.id();
        let cost = e.weight().remove_cost;
        (id, cost)
    })
    .collect();

// Optional second-stage domain choice:
let mut remove = candidate_edges;
remove.sort_by_key(|(_id, cost)| *cost);
```

Agent rule:

```text
Use greedy_feedback_arc_set for:
    fast cycle-breaking heuristic
    “make this directed graph acyclic enough” workflows
    dependency graph repair suggestions
    visualization/layout cleanup

Use specialized optimization when:
    minimum feedback arc set is required
    weighted removal costs matter
    domain constraints decide which edge may be removed
```

---

## 20.4 `steiner_tree` — approximate terminal-connecting tree

Signature:

```rust
pub fn steiner_tree<N, E, Ix>(
    graph: &UnGraph<N, E, Ix>,
    terminals: &[NodeIndex<Ix>],
) -> StableGraph<N, E, Undirected, Ix>
where
    N: Default + Clone + Eq + Hash + Debug,
    E: Copy + Eq + Ord + Measure + BoundedMeasure,
    Ix: IndexType;
```

`steiner_tree` computes a Steiner tree in an undirected connected graph for a slice of terminal nodes using Kou’s algorithm. It returns a `StableGraph` containing the selected nodes and edges, and docs list time complexity as `O(|S| |V|²)` where `|S|` is terminal count. ([Docs.rs][5])

### Basic syntax

```rust
use petgraph::algo::steiner_tree::steiner_tree;
use petgraph::graph::UnGraph;

let mut graph = UnGraph::<(), i32>::new_undirected();

let a = graph.add_node(());
let b = graph.add_node(());
let c = graph.add_node(());
let d = graph.add_node(());
let e = graph.add_node(());
let f = graph.add_node(());

graph.extend_with_edges([
    (a, b, 7),
    (a, f, 6),
    (b, c, 1),
    (b, f, 5),
    (c, d, 1),
    (c, e, 3),
    (d, e, 1),
    (d, f, 4),
    (e, f, 10),
]);

let terminals = vec![a, c, e, f];

let tree = steiner_tree(&graph, &terminals);

let total_weight: i32 = tree.edge_weights().copied().sum();
```

### Type-bound implications

```text
N:
    Default + Clone + Eq + Hash + Debug

E:
    Copy + Eq + Ord
    Measure + BoundedMeasure
    no floats unless ordering/measure bounds are satisfied safely
    use integer / fixed-point weights by default

Input graph:
    UnGraph<N,E,Ix>
    connected graph expected by algorithm contract
```

### Deployment rules

```text
Use steiner_tree when:
    only subset of nodes are mandatory terminals
    extra non-terminal nodes may reduce total connection cost
    approximate/heuristic Steiner solution is acceptable
    graph is undirected and connected
    integer-like ordered edge costs are available

Avoid / use specialized solver when:
    exact Steiner optimum is required
    directed Steiner tree is required
    prize-collecting Steiner variants are required
    side constraints / capacities / budgets exist
    terminal count or graph size makes O(|S||V|²) too expensive
```

### Network design pattern

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash, Default)]
struct Site {
    name: String,
}

type Cost = u32;

let backbone = steiner_tree(&candidate_network, &required_sites);
```

Value case:

```text
Steiner tree:
    connect required terminals
    allow optional intermediate nodes
    cheaper than MST over all nodes when many nodes are optional
```

---

## 20.5 `tred`: transitive reduction and closure for DAGs

The `tred` module computes transitive reduction and transitive closure of a **directed acyclic graph**. The module docs define transitive closure as adding reachability edges and transitive reduction as the minimal edge set preserving the same transitive closure; transitive reduction is well-defined for acyclic graphs only. ([Docs.rs][6])

### Concept model

```text
DAG G:
    A -> B -> C
    A -> C

Transitive closure:
    contains A -> C because path A -> B -> C exists

Transitive reduction:
    removes A -> C if A -> B -> C already preserves reachability
```

### API pipeline

```text
1. Verify / compute topological order:
       toposort(&g, None)?

2. Convert DAG into toposorted adjacency-list format:
       dag_to_toposorted_adjacency_list(&g, &toposort)

3. Compute reduction and closure:
       dag_transitive_reduction_closure(&toposorted_list)

4. Map toposorted indices back through revmap/toposort as needed.
```

`dag_to_toposorted_adjacency_list` requires a DAG plus a supplied topological order, strips node/edge weights, replaces node indices by topological-rank indices, stores neighbors in topological order, returns `(UnweightedList<Ix>, Vec<Ix>)`, and runs in `O(|V| + |E|)` time and auxiliary space. ([Docs.rs][7])

### Basic pipeline

```rust
use petgraph::algo::toposort;
use petgraph::algo::tred::{
    dag_to_toposorted_adjacency_list,
    dag_transitive_reduction_closure,
};
use petgraph::graph::DiGraph;
use petgraph::visit::IntoNeighbors;

let mut g = DiGraph::<&str, ()>::new();

let top = g.add_node("top");
let first = g.add_node("first child");
let second = g.add_node("second child");

g.extend_with_edges([
    (top, first),
    (first, second),
    (top, second), // transitive edge
]);

let order = toposort(&g, None).expect("DAG required");

let (toposorted_graph, revmap) =
    dag_to_toposorted_adjacency_list(&g, &order);

let (reduction, closure) =
    dag_transitive_reduction_closure(&toposorted_graph);

// Example: children of top in topological-order representation.
let top_rank = revmap[top.index()];
let reduced_children: Vec<_> =
    reduction.neighbors(top_rank).collect();
```

`dag_transitive_reduction_closure` takes the specialized topologically sorted adjacency-list representation and returns a pair `(transitive_reduction, transitive_closure)`; its docs cite an algorithm with worst-case `O(|V|³)` behavior but improved behavior on some graph classes, and `O(|E|)` auxiliary space. ([Docs.rs][8])

### Deployment rules

```text
Use tred when:
    graph is a DAG
    reachability preservation matters
    redundant edges should be removed
    closure edges are needed for reachability acceleration
    weights are irrelevant to reduction/closure

Avoid tred when:
    graph may contain cycles
    edge weights/labels must be preserved directly
    transitive reduction of cyclic graph is required
    graph is huge and closure size may explode
```

### Common use cases

```text
Dependency graphs:
    remove redundant direct dependency edges

Workflow DAGs:
    simplify visualization while preserving reachability

Partial orders:
    compute Hasse-diagram-like reduction

Access-control inheritance:
    closure = all inherited permissions / relationships
```

---

## 20.6 Dominators for control-flow graphs

The `dominators` module computes dominance relations for rooted directed control-flow graphs. A node `A` dominates node `B` if every path from root `R` to `B` contains `A`; strict dominance excludes equality; immediate dominator is the closest strict dominator. ([Docs.rs][9])

### `simple_fast`

Signature:

```rust
pub fn simple_fast<G>(
    graph: G,
    root: G::NodeId,
) -> Dominators<G::NodeId>
where
    G: IntoNeighbors + Visitable,
    G::NodeId: Eq + Hash;
```

`simple_fast` implements the Cooper et al. “Simple, Fast Dominance Algorithm”; docs list `O(|V|²)` time and `O(|V| + |E|)` auxiliary space, while noting the authors found it faster in practice on control-flow graphs up to roughly 30,000 nodes despite slower theoretical complexity than Lengauer–Tarjan. ([Docs.rs][10])

### Basic syntax

```rust
use petgraph::algo::dominators::simple_fast;
use petgraph::graph::DiGraph;

let mut cfg = DiGraph::<&str, ()>::new();

let entry = cfg.add_node("entry");
let branch = cfg.add_node("branch");
let then_block = cfg.add_node("then");
let else_block = cfg.add_node("else");
let merge = cfg.add_node("merge");

cfg.extend_with_edges([
    (entry, branch),
    (branch, then_block),
    (branch, else_block),
    (then_block, merge),
    (else_block, merge),
]);

let doms = simple_fast(&cfg, entry);

assert_eq!(doms.root(), entry);
assert_eq!(doms.immediate_dominator(entry), None);
assert_eq!(doms.immediate_dominator(branch), Some(entry));
assert_eq!(doms.immediate_dominator(merge), Some(branch));
```

### `Dominators` API

`Dominators<N>` exposes `root`, `immediate_dominator`, `strict_dominators`, `dominators`, and `immediately_dominated_by`; unreachable nodes return `None` for dominator iterators / immediate dominator, and the root has no immediate dominator. ([Docs.rs][11])

```rust
let idom = doms.immediate_dominator(merge);

if let Some(iter) = doms.dominators(merge) {
    let chain: Vec<_> = iter.collect();
    println!("dominators of merge: {chain:?}");
}

if let Some(iter) = doms.strict_dominators(merge) {
    let strict: Vec<_> = iter.collect();
    println!("strict dominators: {strict:?}");
}

let children: Vec<_> =
    doms.immediately_dominated_by(branch).collect();
```

### Control-flow graph rules

```text
Root:
    entry block / function entry / artificial start node

Edges:
    control transfer edges

Unreachable nodes:
    not dominated by root in useful sense
    APIs return None for unreachable-node dominator queries

Direction:
    forward CFG edges for dominators
    reverse CFG edges for post-dominators
```

### Post-dominator pattern

```rust
use petgraph::visit::Reversed;
use petgraph::algo::dominators::simple_fast;

// Add/choose an exit node, then analyze reversed CFG.
let post_doms = simple_fast(Reversed(&cfg), exit);
```

Agent rule:

```text
Dominators:
    entry-rooted forward CFG

Post-dominators:
    exit-rooted reversed CFG

If multiple exits:
    add synthetic exit
    connect real exits -> synthetic exit
    run reversed dominators from synthetic exit
```

---

## 20.7 Analytics-oriented workflows

### Ranking workflow

```text
directed graph
    -> page_rank
    -> Vec rank by node index
    -> convert to domain rows
    -> sort descending
    -> export/report
```

```rust
let ranks = page_rank(&g, 0.85_f64, 50);

let mut rows: Vec<_> = g
    .node_indices()
    .map(|n| (g[n].clone(), ranks[n.index()]))
    .collect();

rows.sort_by(|a, b| {
    b.1.partial_cmp(&a.1)
        .unwrap_or(std::cmp::Ordering::Equal)
});
```

### DAG cleanup workflow

```text
directed dependency graph
    -> if cyclic:
           greedy_feedback_arc_set or SCC diagnostics
    -> toposort
    -> tred reduction
    -> simplified DAG for visualization/scheduling
```

```rust
if is_cyclic_directed(&g) {
    let remove: Vec<_> =
        greedy_feedback_arc_set(&g).map(|e| e.id()).collect();

    for edge in remove {
        g.remove_edge(edge);
    }
}
```

### Terminal-network workflow

```text
undirected weighted connected graph
    + required terminals
    -> steiner_tree
    -> approximate connecting subnetwork
    -> compute total cost
    -> validate terminal coverage
```

```rust
let tree = steiner_tree(&network, &terminals);
let cost: u32 = tree.edge_weights().copied().sum();
```

### Control-flow workflow

```text
CFG
    -> dominators from entry
    -> immediate dominator tree
    -> loop / SSA / dominance-frontier downstream logic
```

```rust
let doms = simple_fast(&cfg, entry);

for node in cfg.node_indices() {
    println!("{:?} idom={:?}", node, doms.immediate_dominator(node));
}
```

---

## 20.8 Limitations by routine

| Routine                   | Limitation                                                                              |   |                          |
| ------------------------- | --------------------------------------------------------------------------------------- | - | ------------------------ |
| `page_rank`               | fixed iteration count; damping panics outside `[0,1]`; no tolerance/personalization API |   |                          |
| `greedy_feedback_arc_set` | heuristic; ignores weights; not minimum FAS                                             |   |                          |
| `steiner_tree`            | undirected connected `UnGraph`; Kou approximation; strict type bounds                   |   |                          |
| `tred`                    | DAG-only; strips weights in intermediate representation; closure can be large           |   |                          |
| `dominators::simple_fast` | rooted reachability only; `O(                                                           | V | ²)`; not Lengauer–Tarjan |
| `parallel_johnson`        | rayon-only; shortest-path all-pairs, not general graph analytics parallelism            |   |                          |

---

## 20.9 Parallel algorithm availability behind `rayon`

The `algo` module re-exports `parallel_johnson`, and the crate feature page shows `rayon` as an optional feature that enables the `rayon` dependency plus rayon support in backing collections. ([Docs.rs][1])

```toml
[dependencies]
petgraph = { version = "0.8.3", features = ["rayon"] }
rayon = "1"
```

```rust
#[cfg(feature = "rayon")]
use petgraph::algo::parallel_johnson;
```

Operational reality:

```text
Rayon feature gives:
    parallel_johnson
    rayon-enabled backing collection support
    some parallel iterator APIs on selected graph families

Rayon feature does not imply:
    page_rank is parallel
    greedy_feedback_arc_set is parallel
    steiner_tree is parallel
    tred is parallel
    dominators are parallel
```

Agent rule:

```text
Do not assume petgraph analytics are broadly parallel.
Check function availability and feature gates.
For large-scale parallel analytics, evaluate graph-processing crates/frameworks specialized for parallel graph workloads.
```

---

## 20.10 Trait-bound and input-shape matrix

| API                                | Input/bounds                                                                                           | Output                           |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------ | -------------------------------- |
| `page_rank`                        | `NodeCount + IntoEdges + NodeIndexable`; `D: UnitMeasure + Copy`                                       | `Vec<D>`                         |
| `greedy_feedback_arc_set`          | directed graph, `IntoEdgeReferences + GraphProp<EdgeType=Directed> + NodeCount`, node IDs `GraphIndex` | iterator of edge refs            |
| `steiner_tree`                     | `&UnGraph<N,E,Ix>`; `N: Default+Clone+Eq+Hash+Debug`; `E: Copy+Eq+Ord+Measure+BoundedMeasure`          | `StableGraph<N,E,Undirected,Ix>` |
| `dag_to_toposorted_adjacency_list` | DAG + topological order; `GraphBase + IntoNeighborsDirected + NodeCompactIndexable + NodeCount`        | `(UnweightedList<Ix>, Vec<Ix>)`  |
| `dag_transitive_reduction_closure` | `&List<E,Ix>` in specialized topological format                                                        | `(reduction, closure)`           |
| `dominators::simple_fast`          | `IntoNeighbors + Visitable`; `NodeId: Eq + Hash`                                                       | `Dominators<NodeId>`             |

---

## 20.11 Practical use cases

### PageRank

```text
web/link graph scoring
dependency influence scoring
citation-like network importance
service-call centrality approximation
knowledge graph entity prominence
```

### Feedback arc set

```text
cycle-breaking for DAG layout
dependency graph repair suggestions
workflow graph cleanup
make directed cyclic graph schedulable enough for heuristic display
```

### Steiner tree

```text
network design over required terminals
infrastructure backbone approximation
minimum-cost subnet connecting required sites
supply-chain / transit / cable routing approximations
```

### Transitive reduction/closure

```text
DAG edge minimization
dependency simplification
partial-order Hasse diagram
reachability cache construction
permission/inheritance closure
```

### Dominators

```text
compiler control-flow analysis
SSA construction prerequisites
control-dependence analysis
CFG simplification
post-dominator analysis via reversed CFG
```

---

## 20.12 Anti-pattern inventory

```text
Anti-pattern:
    Treat page_rank output as domain-keyed map.
Problem:
    output is Vec by node index.
Fix:
    convert immediately to Vec<(DomainId, rank)> or HashMap<DomainId, rank>.

Anti-pattern:
    Use page_rank with damping outside [0,1].
Problem:
    documented panic.
Fix:
    validate parameter at API boundary.

Anti-pattern:
    Use greedy_feedback_arc_set expecting minimum cycle-breaking set.
Problem:
    greedy heuristic only.
Fix:
    specialized FAS optimizer / ILP / domain-specific solver.

Anti-pattern:
    Use greedy_feedback_arc_set for weighted edge-removal costs.
Problem:
    ignores node/edge weights.
Fix:
    post-process or use weighted optimization.

Anti-pattern:
    Use steiner_tree for directed or exact Steiner variants.
Problem:
    API is undirected connected graph + Kou approximation.
Fix:
    specialized Steiner solver.

Anti-pattern:
    Run tred on cyclic graph.
Problem:
    transitive reduction well-defined only for DAGs in this module.
Fix:
    toposort first; condense SCCs if cycles are expected.

Anti-pattern:
    Expect tred to preserve weights.
Problem:
    toposorted adjacency-list conversion strips node/edge weights.
Fix:
    maintain side maps or reconstruct weighted graph manually.

Anti-pattern:
    Use dominators on graph without a meaningful root.
Problem:
    dominance is root-relative.
Fix:
    define entry/synthetic root.

Anti-pattern:
    Use dominators expecting information about unreachable nodes.
Problem:
    unreachable nodes return None for immediate dominator/dominator iterators.
Fix:
    precompute reachable set or add synthetic root edges intentionally.
```

---

## 20.13 Production checklist

```text
Before page_rank:
    choose f32/f64
    validate damping in [0,1]
    choose iteration count
    convert rank Vec to domain-keyed output
    snapshot test rank ordering if used in product logic

Before greedy_feedback_arc_set:
    confirm directed graph
    decide whether heuristic is acceptable
    collect edge IDs before removal
    run is_cyclic_directed after removal if acyclicity is required
    document that weights are ignored

Before steiner_tree:
    confirm UnGraph
    confirm connectedness
    validate terminals are present
    ensure edge weights are ordered integer/fixed-point costs
    validate terminal coverage in output tree

Before tred:
    run toposort
    pass topological order to dag_to_toposorted_adjacency_list
    treat reduction/closure as unweighted structural artifacts
    map indices back through revmap/toposort

Before dominators:
    choose root
    decide forward dominators vs post-dominators
    add synthetic root/exit for multi-entry/multi-exit graphs
    handle unreachable nodes explicitly
```

Final rule:

```text
page_rank:
    rank nodes

greedy_feedback_arc_set:
    choose heuristic edges to remove for acyclicity

steiner_tree:
    approximate cheapest subnetwork connecting terminals

tred:
    compute DAG reachability closure and redundant-edge reduction

dominators:
    compute root-relative control-flow dominance

rayon:
    optional, limited, not general analytics parallelization
```

[1]: https://docs.rs/petgraph/latest/petgraph/algo/index.html "petgraph::algo - Rust"
[2]: https://docs.rs/crate/petgraph/latest/features "petgraph 0.8.3 - Docs.rs"
[3]: https://docs.rs/petgraph/latest/petgraph/algo/page_rank/fn.page_rank.html "page_rank in petgraph::algo::page_rank - Rust"
[4]: https://docs.rs/petgraph/latest/src/petgraph/algo/feedback_arc_set.rs.html "feedback_arc_set.rs - source"
[5]: https://docs.rs/petgraph/latest/petgraph/algo/steiner_tree/fn.steiner_tree.html "steiner_tree in petgraph::algo::steiner_tree - Rust"
[6]: https://docs.rs/petgraph/latest/petgraph/algo/tred/index.html "petgraph::algo::tred - Rust"
[7]: https://docs.rs/petgraph/latest/petgraph/algo/tred/fn.dag_to_toposorted_adjacency_list.html "dag_to_toposorted_adjacency_list in petgraph::algo::tred - Rust"
[8]: https://docs.rs/petgraph/latest/petgraph/algo/tred/fn.dag_transitive_reduction_closure.html "dag_transitive_reduction_closure in petgraph::algo::tred - Rust"
[9]: https://docs.rs/petgraph/latest/petgraph/algo/dominators/index.html "petgraph::algo::dominators - Rust"
[10]: https://docs.rs/petgraph/latest/petgraph/algo/dominators/fn.simple_fast.html "simple_fast in petgraph::algo::dominators - Rust"
[11]: https://docs.rs/petgraph/latest/petgraph/algo/dominators/struct.Dominators.html "Dominators in petgraph::algo::dominators - Rust"

