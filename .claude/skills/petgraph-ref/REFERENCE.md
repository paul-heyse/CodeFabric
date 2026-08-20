# petgraph — Detailed Reference

Deep-dive companion to `SKILL.md` in the same directory. SKILL.md carries the core map (version
anchors, the catalog-vs-deep-dive layout, reading strategy, capability routing, and the seven key
invariants); this file carries the **section index** with line numbers, the **capability → API map**,
the **decision trees**, the **20 operating rules**, and the **catalog-only coverage map**.

Cross-references back into the core map are written `SKILL §...`. One document is in scope —
`docs/library_ref/petgraph.md`, 22,805 lines, pinned to **petgraph 0.8.3** — so every `§N.M` below is
a section of that document unless it says otherwise.

Read SKILL.md first. Come here when you know *what you want to compute* (§2), when a modelling or
algorithm choice is genuinely open (§3), or when the thing you are looking for turns out to have no
deep-dive (§5).

## Table of Contents

- **§1 — Section index** (§0-§30; deep-dive line numbers for §0 and §2-§20)
- **§2 — Capability → API map** (outcome → petgraph function/type → section)
- **§3 — Decision trees** (graph type · shortest path · cycles & DAGs · traversal · identity · construction · isomorphism · trait bounds · adaptor vs materialise · Cargo features)
- **§4 — Operating rules** (full set of 20)
- **§5 — Working without a deep-dive** (the topics §1.2 cannot resolve, and how to proceed)

---

## §1 — Section index

Catalog `1-411`; deep-dives start at `412` with `# 0)`. Every deep-dive chapter uses `## N.M`
subsections, opens with a type shape or core-imports block, and closes with an **anti-pattern
inventory** and a **deployment checklist** — those two are omitted from the *Key subsections* column
below because they are present almost everywhere; load them anyway before writing code.

### §1.1 Deep-dive chapters (20)

| § | Line | Lines | Title | Key subsections |
|---|---:|---:|---|---|
| **0** | 412 | 744 | Scope, versioning, and the petgraph mental model | 0.1 scope contract · **0.2 released 0.8.3 vs trunk multi-crate rewrite** · 0.4 core vocabulary · **0.5 "weight" means associated data, not cost** · 0.6 directionality · 0.7 index mental model · 0.8 the five representations · **0.9 algorithms target capabilities, not types** · 0.10 DOT mental model · **0.11 what petgraph is not** · 0.12 mental-model table · 0.13 baseline imports · 0.14 baseline modelling patterns · 0.15 production stance |
| **2** | 1156 | 1032 | Graph type decision guide | 2.0 decision primitive · **2.1 representation matrix** · 2.2 `Graph` · 2.3 `StableGraph` · 2.4 `GraphMap` · 2.5 `MatrixGraph` · 2.6 `Csr` · 2.7 sparse vs dense · 2.8 stable vs compact indices · 2.9 external IDs vs `NodeIndex` · 2.10 parallel edges vs simple · 2.11 mutation frequency · 2.12 memory footprint · **2.13 feature-gating / deployment** · **2.14 agent selection algorithm** · 2.15 copy-paste type aliases · **2.17 final decision table** |
| **3** | 2188 | 947 | Core generics and type aliases | 3.0 canonical signatures · 3.1 `N` · 3.2 `E` · 3.3 `Ty` directedness marker · 3.4 `Ix` and the graph-size limit · 3.5 `NodeIndex<Ix>` / `EdgeIndex<Ix>` · 3.6-3.10 shorthand aliases (`DiGraph`/`UnGraph`/`StableDiGraph`/`DiGraphMap`/matrix/`Csr`) · **3.11 `u16` vs `u32` vs `usize`** · 3.12 `Ix` in a public API |
| **4** | 3135 | 1075 | `Graph` deep dive — adjacency-list workhorse | 4.0 type shape · 4.1 `new`/`new_undirected`/`with_capacity`/`from_edges` · 4.2 `add_node`/`add_edge`/`update_edge`/`remove_node`/`remove_edge` · 4.3 `node_weight`/`edge_weight` + indexing · 4.4 `node_indices`/`edges`/`edge_references`/`neighbors` · **4.5 parallel-edge behavior** · **4.6 index invalidation rules** · 4.7 `with_capacity`/`reserve_nodes`/`reserve_edges` · 4.8 performance model · 4.9 domain records → `Graph` · 4.10 mutating payloads mid-traversal · 4.11 error-policy recipes |
| **5** | 4210 | 982 | `StableGraph` deep dive — persistent index use cases | 5.0 type shape · **5.1 stable index contract** · 5.2 holes after deletion · 5.3 core syntax map · 5.4 reusable indices · 5.5 memory and feature-parity cost · 5.6 when stability matters · 5.7 duplicate-edge control · 5.8 migrating from `Graph` · 5.9 pitfall: assuming compact ranges · 5.10 pitfall: `NodeCompactIndexable` · 5.11 pitfall: deletion-heavy loads · 5.12 StableGraph-backed store · 5.13 migration decision table |
| **6** | 5192 | 1086 | `GraphMap` deep dive — map-key node identity | 6.0 type shape · **6.1 node value *is* the identifier** · **6.2 required bounds `Copy + Eq + Hash + Ord`** · 6.3 constructors · 6.4 mutations · 6.5 accessors · 6.6 iteration · 6.7 directed vs undirected · 6.8 no parallel edges, self-loops allowed · 6.9 auto node insert via `add_edge` · 6.10 hasher customization · 6.11 conversions with `Graph` · 6.12 algorithm compatibility · 6.13 parallel iterators · 6.14 best-fit modelling · **6.15 `GraphMap` vs `Graph + HashMap`** |
| **7** | 6278 | 1040 | `MatrixGraph` deep dive — dense representation | 7.0 type shape · 7.1 storage model · **7.2 space complexity and dense suitability** · 7.3 constructors · 7.4 `Null` edge-presence abstraction · 7.5 insertion/lookup tradeoffs · 7.6 no parallel edges · **7.7 large edge weights — box them** · 7.8 when matrix beats adjacency list · 7.9 iteration · 7.10 mutation/deletion caveats · 7.11 capacity · **7.12 `Option<E>` vs `NotZero<E>`** · 7.13 vs `Graph` vs `GraphMap` · 7.14 deployment recipes |
| **8** | 7318 | 1013 | `Csr` deep dive — compressed sparse row | 8.0 type shape · 8.1 CSR mental model · 8.2 constructors · **8.3 `from_sorted_edges` — preferred bulk build** · 8.4 sorted/unique requirement · 8.5 fast outgoing-edge iteration · 8.6 edge-existence lookup · 8.7 mutation cost · 8.8 self-loops yes, parallel edges no · **8.9 undirected CSR stores both directions** · 8.10 index and trait behavior · 8.11 imported edge-list pattern · 8.12 vs the other three · 8.13 construction-performance rules · 8.15 best-fit workloads |
| **9** | 8331 | 1021 | Directedness, edge semantics, and graph invariants | 9.0 imports · **9.1 `Directed`/`Undirected`/`EdgeType`** · **9.2 `Direction::{Outgoing, Incoming}`** · 9.3 neighbor semantics · 9.4 edge-iterator semantics · 9.5 edge orientation when undirected · **9.6 self-loops and parallel edges by family** · **9.7 `reverse()` — in-place** · **9.8 `into_edge_type()` — marker only** · 9.9 direction conversion patterns · 9.10 one-way roads · 9.11 dependency DAGs · 9.12 bidirectional networks · 9.13 multigraph-like data · **9.14 edge-invariant table** · 9.15 modelling decision table |
| **10** | 9352 | 1171 | Node and edge weights as domain data | 10.0 core model · **10.1 payload, not algorithmic cost** · 10.2 zero-sized `()` weights · **10.3 numeric costs vs domain metadata** · 10.4 node payload patterns · 10.5 edge payload patterns · 10.6 borrowing vs cloning · **10.7 `map`** · **10.8 `map_owned`** · **10.9 `filter_map`** · **10.10 `filter_map_owned`** · 10.11 transform decision table · 10.12 large payloads · **10.13 separating topology from business data** · 10.14 `GraphMap` payload caveat · 10.15 `MatrixGraph` payload caveat · 10.17 serialization payload rules |
| **11** | 10523 | 1164 | Indexing, identity, and mutation safety | 11.0 imports · 11.1 `NodeIndex<Ix>` as a graph-local handle · 11.2 `EdgeIndex<Ix>` · 11.3 identity models by family · 11.4 compact vs stable guarantees · **11.5 index invalidation table** · **11.6 slot reuse — stability ≠ generation safety** · 11.7 `NodeIndexable` · 11.8 `NodeCompactIndexable` · 11.9 `EdgeIndexable` · **11.10 `HashMap<DomainId, NodeIndex>`** · 11.11 stable external-ID store · **11.12 anti-pattern: storing handles across removals** · 11.13-11.14 safe mutation while traversing · 11.15 walker state · 11.16 dense arrays vs sparse handles · 11.17 public API handle design · 11.18 conversion/compaction safety |
| **12** | 11687 | 1522 | Construction patterns and graph loading | 12.0 construction taxonomy · 12.1 `new`/`new_undirected`/`with_capacity` · **12.2 `from_edges`** · **12.3 `extend_with_edges`** · 12.4 incremental from domain records · **12.5 deduplicating nodes** · **12.6 deduplicating edges** · **12.7 `try_add_node`/`try_add_edge`/`try_update_edge`** · 12.8 `FromElements` · 12.9 algorithm-output → graph · **12.10 edge lists** · **12.11 adjacency lists** · **12.12 adjacency matrices** · **12.13 DOT / Graphviz import** · **12.14 Graph6** · **12.15 serialized data** · 12.16 capacity planning for large imports · 12.17 per-representation loading · 12.18 error-first builder skeleton · 12.19 loading pipeline decision table |
| **13** | 13209 | 1197 | Traversal system — visitors, walkers, graph traits | 13.0 imports · 13.1 `visit` mental model · 13.2 minimal trait stack · **13.3 `Dfs`** · **13.4 `Bfs`** · **13.5 `DfsPostOrder`** · **13.6 `Topo`** · **13.7 `depth_first_search` event/callback API** · 13.8 `Walker`/`WalkerIter` · 13.9 why walkers do not hold the borrow · 13.10 detached neighbor walkers · 13.11 direction-aware traversal · **13.12 `NodeFiltered`** · **13.13 `EdgeFiltered`** · **13.14 `Reversed` / `UndirectedAdaptor`** · 13.15 custom graph compatibility · **13.16 traversal mutation-safety table** · **13.17 traversal selection table** |
| **14** | 14406 | 1271 | Trait-based graph abstraction | 14.0 imports · 14.1 `GraphBase` · 14.2 `GraphProp` · 14.3 `GraphRef` · 14.4 `IntoNeighbors` · 14.5 `IntoNeighborsDirected` · 14.6 `IntoEdges` · 14.7 `IntoEdgesDirected` · 14.8 `IntoEdgeReferences` · 14.9 `IntoNodeIdentifiers` · 14.10 `IntoNodeReferences` · 14.11 `Data` · 14.12 `DataMap` · 14.13 `DataMapMut` · 14.14 `NodeCount`/`EdgeCount` · 14.15 `NodeIndexable`/`NodeCompactIndexable`/`EdgeIndexable` · **14.16 which type implements which trait** · **14.17 trait-bound recipes** · 14.18 generic bounds vs concrete `Graph` APIs · **14.19 bound-selection cheat sheet** |
| **15** | 15677 | 1313 | Algorithm catalog — shortest paths | 15.0 imports · **15.1 edge weights vs algorithm costs** · 15.2 numeric trait taxonomy · **15.3 negative-weight policy** · **15.4 `dijkstra`** · **15.5 `bidirectional_dijkstra`** · **15.6 `astar`** · **15.7 `bellman_ford`** · **15.8 `find_negative_cycle`** · **15.9 `spfa`** · **15.10 `floyd_warshall`** · **15.11 `johnson`** · **15.12 `parallel_johnson`** (rayon) · **15.13 `k_shortest_path`** · 15.14 predecessor reconstruction · 15.15 all-pairs vs single-source vs single-pair · **15.16 decision table** · **15.17 output matrix** · 15.18 trait compatibility · 15.19 cost-closure recipes · 15.22 final agent rules |
| **16** | 16990 | 1181 | Algorithm catalog — connectivity, components, cycles, DAGs | 16.0 imports · **16.1 decision table** · **16.2 `connected_components`** · **16.3 `has_path_connecting`** · **16.4 `kosaraju_scc`** · **16.5 `tarjan_scc`** · **16.6 SCC interpretation rules** · **16.7 `condensation`** · **16.8 `is_cyclic_directed`** · **16.9 `is_cyclic_undirected`** · **16.10 `toposort`** · **16.11 `Topo`** · 16.12 DAG modelling conventions · **16.13 `acyclic::Acyclic`** · 16.14 `Acyclic` vs `toposort` · 16.15 SCC condensation workflow · **16.16 cycle error handling** · 16.17 topological position metadata · 16.18 trait-bound matrix · 16.19 DAG production recipes · **16.20 decision table** |
| **17** | 18171 | 1119 | Algorithm catalog — spanning trees, cuts, bridges, articulation | 17.0 imports · **17.1 selection table** · 17.2 edge-weight ordering requirements · **17.3 `min_spanning_tree` (Kruskal)** · **17.4 `min_spanning_tree_prim`** · **17.5 `FromElements` to build the result graph** · **17.6 `bridges`** · **17.7 bridge multigraph caveat** · **17.8 `articulation_points`** · **17.9 `UnionFind`** · 17.10 UnionFind as component builder · 17.11 manual Kruskal · 17.12 built-in vs manual · 17.13 bridges vs MST edges · 17.14 articulation points vs bridges · 17.15 use cases · 17.16 trait-bound matrix · **17.17 decision table** |
| **18** | 19290 | 1047 | Algorithm catalog — matching, flow, cliques, coloring | 18.0 imports · **18.1 decision table** · 18.2 matching model · **18.3 `greedy_matching`** · **18.4 `maximum_matching`** · **18.5 `Matching<G>` output API** · 18.6 matching deployment · 18.7 max-flow capacity model · **18.8 `dinics`** · **18.9 `ford_fulkerson`** · 18.10 flow output interpretation · 18.11 flow modelling recipes · **18.12 flow limitations** · **18.13 `maximal_cliques`** · 18.14 clique cost controls · **18.15 `dsatur_coloring`** · 18.16 coloring output validation · **18.17 capacity and optimization-model boundaries** · 18.18 trait-bound matrix · 18.19 use cases |
| **19** | 20337 | 1471 | Algorithm catalog — isomorphism and subgraph matching | 19.0 imports · **19.1 syntactic vs semantic isomorphism** · 19.2 trait-bound contract · **19.3 multigraph restriction** · **19.4 `is_isomorphic`** · **19.5 `is_isomorphic_matching`** · 19.6 node/edge matching closures · **19.7 `is_isomorphic_subgraph`** · **19.8 `is_isomorphic_subgraph_matching`** · **19.9 `subgraph_isomorphisms_iter`** · 19.10 exact vs domain-equivalence matching · **19.11 induced-subgraph semantics — critical caveat** · 19.12 directedness/shape compatibility · **19.13 pre-filtering and pruning** · 19.14 graph preparation · 19.15-19.17 use cases (workflow · chemistry-like · typed pattern detection) · **19.18 performance caveats** · **19.19 API decision matrix** · 19.20 trait-bound matrix |
| **20** | 21808 | 998 | Algorithm catalog — graph analytics and specialized routines | 20.0 imports · **20.1 decision table** · **20.2 `page_rank`** · **20.3 `greedy_feedback_arc_set`** · **20.4 `steiner_tree`** · **20.5 `tred` — `dag_to_toposorted_adjacency_list` + `dag_transitive_reduction_closure`** · **20.6 `dominators::simple_fast` / `Dominators`** · 20.7 analytics workflows · **20.8 limitations by routine** · **20.9 parallel availability behind `rayon`** · 20.10 trait-bound and input-shape matrix · 20.11 use cases |

### §1.2 Catalog-only sections (11) — no deep-dive exists

These are catalogued but never deep-dived. Do not search for a `# N)` chapter; read the catalog
bullets for *what exists*, then follow §5 below to where the material is actually covered.

| § | Catalog line | Title | Real coverage |
|---|---:|---|---|
| **1** | 19 | Installation, Cargo features, and deployment surface | **§2.13** carries the whole feature matrix and four `Cargo.toml` profiles |
| **21** | 272 | Graph adaptors and filtered views | **§13.12-§13.14**, **§14.3** — all four adaptors are deep-dived there |
| **22** | 285 | Graph operators and transformations | **§10.7-§10.11** (`map`/`filter_map` family), **§16.7** (`condensation`), **§9.8** (`into_edge_type`); `operator::complement` is **catalog-only** |
| **23** | 296 | Visualization and DOT / Graphviz workflows | **§0.10** (mental model + `Dot::with_config`), **§0.13** (imports), **§12.13** (DOT *import*); `Dot::with_attr_getters` styling is **catalog-only** |
| **24** | 310 | Serialization, import/export, interoperability | **§12.15** (loading serialized data), **§10.17** (payload rules), **§2.13** (`serde-1` profile), **§12.14** (Graph6) |
| **25** | 322 | Parallelism and performance engineering | **§20.9** (rayon availability), **§15.12** (`parallel_johnson`), **§6.13** (parallel iterators), **§4.8**/**§7.2**/**§8.13** (per-type performance), **§4.7**/**§12.16** (capacity) |
| **26** | 335 | `no_std`, embedded, constrained environments | **§2.13** (`default-features = false` and what it switches off) |
| **27** | 346 | Error handling, panics, fallible APIs | **§12.7** (`try_*` APIs), **§4.11** (error-policy recipes), **§16.16** (cycle errors), **§12.18** (error-first builder) |
| **28** | 356 | Testing and verification patterns | no single home — **§0.10** (do not golden-test DOT), plus each chapter's anti-pattern inventory |
| **29** | 369 | Common recipes and copy-paste patterns | distributed: **§4.9**, **§11.10**, **§15.14**, **§16.19**, **§17.5**, **§12.17** |
| **30** | 385 | Best-practice decision tables | the per-chapter tables: **§2.17**, **§13.17**, **§15.16**, **§16.20**, **§17.17**, **§19.19**, **§20.1** |

---

## §2 — Capability → API map

**Start here when you know the outcome you want but not the call.** Rows are phrased as goals; the
middle column is the petgraph surface that achieves it; the right column is where it is documented.
The *Watch* notes are the failure that most often turns a correct-looking call into a wrong answer —
they are compressed from the chapter anti-pattern inventories.

### Getting a graph into memory

| I need to… | Use | Where | Watch |
|---|---|---|---|
| create an empty directed / undirected graph | `Graph::new()` · `Graph::new_undirected()` · aliases `DiGraph`/`UnGraph` | 4.1, 3.6 | directedness is a **type parameter**, not a flag (9.1) |
| pre-size a graph I am about to fill | `Graph::with_capacity(n, e)` · `reserve_nodes` · `reserve_edges` | 4.7, 12.16 | the dominant cost on large imports (12.16) |
| bootstrap topology from a literal edge list | `Graph::from_edges(...)` | 12.2 | node indices are implied by the tuple values; no dedupe |
| add edges to an existing graph in bulk | `extend_with_edges(...)` | 12.3 | same implied-index semantics as `from_edges` |
| add nodes/edges one at a time | `add_node(w)` · `add_edge(a, b, w)` | 4.2 | `add_edge` twice creates **two** edges on `Graph` (4.5) |
| add an edge but replace it if it exists | `update_edge(a, b, w)` | 4.2, 12.6 | this is the deduplication control — not `add_edge` |
| fail instead of panic on a full or invalid graph | `try_add_node` · `try_add_edge` · `try_update_edge` | 12.7, 4.11 | the plain forms **panic**; use `try_*` on untrusted input (Rule 14) |
| build from domain records, deduplicating as I go | incremental build + `HashMap<DomainId, NodeIndex>` | 12.4-12.6, 4.9 | dedupe nodes *and* edges; they are separate steps |
| load an edge list / adjacency list / adjacency matrix | loader recipes | 12.10 / 12.11 / 12.12 | pick the representation first (2.14), then the loader (12.17) |
| import DOT / Graphviz | `dot_parser` feature | 12.13 | feature-gated; output styling is a different topic (§5) |
| import Graph6 | Graph6 loaders | 12.14 | undirected graphs only |
| rehydrate a serialized graph | `serde-1` feature | 12.15, 10.17 | `Graph`, `StableGraph`, `GraphMap` only; version your format (10.17) |
| build a static sparse graph as fast as possible | `Csr::from_sorted_edges(...)` | 8.3, 8.4 | edges **must** be sorted and unique; undirected CSR needs both directions (8.9) |
| build the output of an algorithm back into a graph | `FromElements` | 12.8, 17.5 | the standard way to turn `min_spanning_tree` into a real graph |

### Referring to nodes safely

| I need to… | Use | Where | Watch |
|---|---|---|---|
| hold a reference to a node across time | `NodeIndex<Ix>` + external `HashMap` | 11.1, 11.10-11.11 | a bare `NodeIndex` is **not** stable across `remove_node` (11.5) |
| keep indices valid after deletions | `StableGraph` | 5.1, 5.6 | slots are reused later — stability ≠ generation safety (11.6) |
| use my own IDs as the node identity | `GraphMap` (node value *is* the key) | 6.1, 6.2 | needs `Copy + Eq + Hash + Ord`; no parallel edges (6.8) |
| decide between `GraphMap` and `Graph` + a map | comparison | 6.15, 2.9 | `GraphMap` trades payload richness for key ergonomics (10.14) |
| look up a weight | `node_weight` / `edge_weight` (+ `_mut`), or index syntax | 4.3 | index syntax **panics** on a stale handle; the accessors return `Option` |
| test for / find an edge | `contains_edge` · `find_edge` | 4.3, 8.6 | on `Csr` and `MatrixGraph` the cost model differs sharply (8.6, 7.5) |
| iterate everything | `node_indices` · `edge_indices` · `node_weights` · `edge_references` | 4.4 | `edge_references` gives `EdgeRef` with source/target/weight together |
| iterate a node's neighbours | `neighbors` · `neighbors_directed` · `edges` · `edges_directed` | 4.4, 9.3, 9.4 | on an **undirected** graph, direction arguments are ignored (9.3, 9.5) |
| count | `node_count` · `edge_count` | 4.4, 14.14 | on `StableGraph` these exclude holes — do not use them as an index bound (5.9) |
| mutate while traversing | detached walkers | 13.9-13.10, 11.13-11.14 | the borrow model is why `Dfs` does not hold the graph (13.9) |

### Walking the graph yourself

| I need to… | Use | Where | Watch |
|---|---|---|---|
| visit reachable nodes depth-first | `Dfs::new(&g, start)` | 13.3 | preorder; visits only what is reachable from `start` |
| visit reachable nodes breadth-first | `Bfs::new(&g, start)` | 13.4 | BFS order ≠ shortest path when edges are weighted (15.4) |
| process children before parents | `DfsPostOrder` | 13.5 | the usual choice for bottom-up folds over a DAG |
| stream nodes in dependency order | `Topo::new(&g)` | 13.6, 16.11 | silently yields nothing for the cyclic part — use `toposort` if you need the error (16.14) |
| react to traversal *events* (tree edge, back edge, finish) | `depth_first_search(...)` with a callback | 13.7 | the event API is how you detect back edges yourself |
| drive a traversal as an iterator | `Walker` / `WalkerIter` | 13.8 | `.iter(&g)` adapts a walker into an iterator |
| traverse only part of the graph | `NodeFiltered` · `EdgeFiltered` | 13.12, 13.13 | a **view**, not a copy — no allocation (Rule 4) |
| traverse against the arrows | `Reversed` | 13.14 | also the standard trick for "who depends on me?" |
| treat a directed graph as undirected for one algorithm | `UndirectedAdaptor` | 13.14 | different from `into_edge_type`, which changes the type marker (9.8) |
| pick between the traversal types | selection table | 13.17 | mutation-safety table is 13.16 |

### Reachability, components, cycles, ordering

| I need to… | Use | Where | Watch |
|---|---|---|---|
| know whether B is reachable from A | `has_path_connecting` | 16.3 | predicate only; use `astar`/`dijkstra` if you want the path |
| count weakly connected components | `connected_components` | 16.2 | **weak** — ignores direction. Not the same as SCC (16.6) |
| find strongly connected components | `tarjan_scc` (one pass) · `kosaraju_scc` (two pass) | 16.5, 16.4 | output ordering differs; read 16.6 before interpreting |
| collapse each SCC into a single node | `condensation` | 16.7, 16.15 | the result is a DAG — the standard pre-step for cyclic input |
| test for a cycle | `is_cyclic_directed` · `is_cyclic_undirected` | 16.8, 16.9 | predicates only; they do not tell you *which* cycle |
| order a DAG, or learn it is not a DAG | `toposort(&g, None)` | 16.10 | returns `Err(Cycle)` — this is the diagnostic form (16.16) |
| keep a graph acyclic by construction | `acyclic::Acyclic` | 16.13, 16.14 | rejects cycle-creating edges at insert; cheaper than re-running `toposort` |
| break cycles so the graph becomes a DAG | `greedy_feedback_arc_set` | 20.3 | heuristic — not a minimum feedback arc set (20.8) |
| find a negative cycle | `find_negative_cycle` | 15.8 | reachable-from-source only |

### Distances and paths

Ten algorithms, one per subsection in §15; the decision table is **15.16** and the output matrix
(what each one actually returns) is **15.17**.

| I need to… | Use | Where | Watch |
|---|---|---|---|
| distances from one source, non-negative weights | `dijkstra` | 15.4 | **rejects nothing** — negative weights give wrong answers silently (15.3) |
| the cost between one specific pair | `bidirectional_dijkstra` | 15.5 | cost only; still non-negative-only |
| one path to a goal, with a heuristic | `astar` | 15.6 | an inadmissible heuristic breaks optimality |
| distances with negative edges | `bellman_ford` · `spfa` | 15.7, 15.9 | both also detect negative cycles; `spfa` is queue-based |
| all-pairs on a dense graph | `floyd_warshall` | 15.10 | simple API, `O(V³)` |
| all-pairs on a sparse graph | `johnson` · `parallel_johnson` | 15.11, 15.12 | `parallel_johnson` needs the **`rayon`** feature (20.9) |
| the k-th shortest distance per node | `k_shortest_path` | 15.13 | distances per node, not k enumerated paths |
| the actual path, not just the cost | predecessor reconstruction | 15.14 | `dijkstra` returns a distance map; `bellman_ford` returns predecessors |
| cost from a rich edge payload | a cost closure | 15.19, 15.1 | this is the answer to "my edge weight is a struct" (Rule 1) |

### Structure: trees, cuts, critical elements

| I need to… | Use | Where | Watch |
|---|---|---|---|
| a minimum spanning tree/forest | `min_spanning_tree` (Kruskal) · `min_spanning_tree_prim` | 17.3, 17.4 | Kruskal returns a **forest** on a disconnected graph |
| turn that result into a graph | `FromElements` | 17.5 | the result is an element stream, not a graph |
| find edges whose removal disconnects the graph | `bridges` | 17.6 | parallel edges mean a bridge may not be reported (17.7) |
| find vertices whose removal disconnects the graph | `articulation_points` | 17.8 | related to but not the same as bridges (17.14) |
| union/find over disjoint sets | `UnionFind` | 17.9, 17.10 | also the building block for a manual Kruskal (17.11) |
| a tree connecting a chosen set of terminals | `steiner_tree` | 20.4 | **approximate** (20.8) |

### Assignment, flow, grouping, colouring

| I need to… | Use | Where | Watch |
|---|---|---|---|
| pair items up, quickly | `greedy_matching` | 18.3 | heuristic |
| pair items up, optimally | `maximum_matching` | 18.4, 18.5 | maximum **cardinality**, not maximum weight |
| route maximum flow | `dinics` · `ford_fulkerson` | 18.8, 18.9 | read 18.7 on the capacity model and 18.12 on what flow cannot express |
| enumerate all maximal cliques | `maximal_cliques` | 18.13 | exponential in the worst case — bound it (18.14) |
| assign non-conflicting labels | `dsatur_coloring` | 18.15, 18.16 | heuristic; validate the output |
| solve a real optimisation problem | — | **18.17** | explicit boundary: petgraph is not a solver; take the model elsewhere |

### Comparing graphs and finding patterns

| I need to… | Use | Where | Watch |
|---|---|---|---|
| test two graphs for the same shape | `is_isomorphic` | 19.4 | structure only, ignores weights |
| ...respecting node/edge semantics | `is_isomorphic_matching` | 19.5, 19.6 | you supply the two matching closures |
| test whether a pattern occurs | `is_isomorphic_subgraph` | 19.7 | **induced** subgraph — read 19.11, this is the classic wrong assumption |
| ...respecting semantics | `is_isomorphic_subgraph_matching` | 19.8 | same induced caveat |
| enumerate where a pattern occurs | `subgraph_isomorphisms_iter` | 19.9 | the only one that yields mappings; pre-filter first (19.13) |
| choose among the five | API decision matrix | 19.19 | multigraphs are **not supported** by this family (19.3) |

### Ranking, reduction, dominance

| I need to… | Use | Where | Watch |
|---|---|---|---|
| rank nodes by link structure | `page_rank` | 20.2 | iterative; convergence is your parameter |
| compute the minimal edge set with the same reachability | `dag_to_toposorted_adjacency_list` → `dag_transitive_reduction_closure` | 20.5 | **DAG only**; a three-step pipeline through `toposort`, then map indices back |
| compute the transitive closure | same pipeline, closure output | 20.5 | both come out of the one call |
| find dominators in a control-flow graph | `dominators::simple_fast(g, root)` | 20.6 | rooted directed graphs; `O(V²)` but fast in practice |

### Deriving a new graph from an old one

| I need to… | Use | Where | Watch |
|---|---|---|---|
| change weights, keep topology | `map` (borrowed) · `map_owned` (consuming) | 10.7, 10.8 | decision table is 10.11 |
| drop nodes/edges while transforming | `filter_map` · `filter_map_owned` | 10.9, 10.10 | dropping a node drops its edges |
| flip every edge | `reverse()` | 9.7 | in place, rewrites edges |
| change only the directedness marker | `into_edge_type()` | 9.8 | **does not touch edges** — the pair 9.7/9.8 is a classic confusion |
| collapse cycles | `condensation` | 16.7 | see *Reachability, components, cycles, ordering* above |
| avoid materialising at all | an adaptor | 13.12-13.14 | prefer this unless you need to keep the result (Rule 4) |

### Getting the graph back out

| I need to… | Use | Where | Watch |
|---|---|---|---|
| render for debugging | `Dot::new(&g)` · `Dot::with_config(&g, &[Config::EdgeNoLabel])` | 0.10, 0.13 | debug rendering; **do not golden-test the string** (Rule 7) |
| style nodes/edges in DOT | `Dot::with_attr_getters` | **catalog §23 only** | no deep-dive exists — see §5 |
| persist and reload | `serde-1` | 12.15, 10.17, 2.13 | `Graph`/`StableGraph`/`GraphMap`; version the format |
| export raw topology | edge iteration + your own writer | 4.4, 10.17 | a stable external format is your job, not the crate's |

---

## §3 — Decision trees

Ten choices petgraph forces early. Each ends in a citation; the citation is the authority.

### Which graph type?

```
Node identity is my own value (enum, u32, &'static str) and I never want NodeIndex?
  -> GraphMap<N, E, Ty>            requires Copy + Eq + Hash + Ord   (2.4, 6.1, 6.2)
     ... no parallel edges, payload lives outside the graph          (6.8, 10.14)
I remove nodes/edges and must keep handles valid across removals?
  -> StableGraph                    indices survive; slots reused    (2.3, 5.1, 11.6)
Dense graph (edges ~ V²), or I need O(1) edge-existence lookup?
  -> MatrixGraph                    box large weights                (2.5, 7.2, 7.7)
Static sparse graph, built once, iterated hot?
  -> Csr + from_sorted_edges        sorted + unique required         (2.6, 8.3, 8.4)
Otherwise
  -> Graph                          the default; parallel edges ok   (2.2, 4.0)
Still unsure
  -> run the selection algorithm 2.14, then confirm against 2.17
```

### Which shortest-path algorithm?

```
All-pairs?
  dense graph   -> floyd_warshall                                    (15.10)
  sparse graph  -> johnson  (+ rayon: parallel_johnson)              (15.11, 15.12)
Single pair, and I have a distance heuristic?
  -> astar                                                           (15.6)
Single pair, no heuristic, non-negative weights?
  -> bidirectional_dijkstra                                          (15.5)
Single source?
  weights are non-negative        -> dijkstra                        (15.4)
  weights may be negative         -> bellman_ford or spfa            (15.7, 15.9)
  I only need to know if a negative cycle exists -> find_negative_cycle (15.8)
Need the k-th shortest distance per node?
  -> k_shortest_path                                                 (15.13)
Need the path, not the cost?
  -> reconstruct from predecessors                                   (15.14)
My "weight" is a struct, not a number?
  -> pass a cost closure; do not change the edge type                (15.19, 15.1)
Confirm against the decision table 15.16 and the output matrix 15.17.
```

### Cycles, ordering, and DAGs

```
Just need a yes/no?
  -> is_cyclic_directed / is_cyclic_undirected                       (16.8, 16.9)
Need an order, and a cycle is an error?
  -> toposort(&g, None)  -> Err(Cycle) carries the diagnostic        (16.10, 16.16)
Need an order, streaming, and cycles are simply out of scope?
  -> Topo   ... note it yields nothing for the cyclic part           (16.11, 16.14)
Need to prevent cycles from ever existing?
  -> acyclic::Acyclic wrapper, checked at insert                     (16.13)
Graph is cyclic and the algorithm needs a DAG?
  -> tarjan_scc -> condensation -> run on the condensation           (16.5, 16.7, 16.15)
Graph must become a DAG and I accept losing edges?
  -> greedy_feedback_arc_set   (heuristic, not minimum)              (20.3, 20.8)
```

### Which traversal?

```
Reachability / visit order from a start node
  preorder            -> Dfs                                         (13.3)
  level order         -> Bfs                                         (13.4)
  children first      -> DfsPostOrder                                (13.5)
  dependency order    -> Topo                                        (13.6)
I need to observe edges classified (tree / back / cross) as I go
  -> depth_first_search with an event callback                       (13.7)
I want an iterator rather than a stepping API
  -> Walker::iter(&g) / WalkerIter                                   (13.8)
I need to mutate the graph during traversal
  -> detached walkers; understand the borrow model first             (13.9, 13.10, 11.13)
I want to traverse a subset, or against edge direction
  -> NodeFiltered / EdgeFiltered / Reversed  (views, not copies)     (13.12-13.14)
Confirm against the selection table 13.17 and mutation-safety table 13.16.
```

### How do I refer to a node over time?

```
Within one uninterrupted borrow, no mutation?
  -> NodeIndex is fine                                               (11.1)
The graph is mutated, but only by adding?
  -> NodeIndex is still stable (additions never invalidate)          (11.5)
Nodes are removed, and I hold handles across the removal?
  Graph        -> handles are INVALID; last node was swapped in      (4.6, 11.5)
  StableGraph  -> handles survive, but slots are later reused        (5.1, 11.6)
  ... so a stale handle can silently address a different live node
Handles must survive removals AND be meaningful to my domain?
  -> own the mapping: HashMap<DomainId, NodeIndex>, rebuilt on compaction (11.10, 11.11)
Handles cross a public API boundary?
  -> do not expose NodeIndex; expose your own opaque ID              (11.17, 3.12)
```

### How do I build/load it?

```
Topology is a literal in code?
  -> Graph::from_edges / extend_with_edges                           (12.2, 12.3)
Records arrive one at a time, and IDs repeat?
  -> incremental build + dedupe nodes then edges                     (12.4-12.6)
  -> update_edge, not add_edge, if repeats mean "replace"            (12.6, 4.2)
Input is untrusted or size-bounded?
  -> try_add_node / try_add_edge / try_update_edge, error-first builder (12.7, 12.18)
Input is a file format?
  edge list / adjacency list / adjacency matrix -> 12.10 / 12.11 / 12.12
  DOT  -> dot_parser feature                                          (12.13)
  Graph6 -> undirected only                                           (12.14)
  serde -> serde-1 feature                                            (12.15)
Building a Csr?
  -> sort and dedupe edges first; from_sorted_edges                   (8.3, 8.4)
Large import?
  -> size it up front: with_capacity / reserve_*                      (12.16, 4.7)
```

### Which isomorphism API?

```
Do I need the mappings, or just a yes/no?
  mappings  -> subgraph_isomorphisms_iter                            (19.9)
  yes/no    -> continue
Whole graph, or pattern-inside-graph?
  whole graph:
    structure only        -> is_isomorphic                           (19.4)
    weights matter        -> is_isomorphic_matching + closures       (19.5, 19.6)
  pattern inside graph:
    structure only        -> is_isomorphic_subgraph                  (19.7)
    weights matter        -> is_isomorphic_subgraph_matching         (19.8)
Before any of these:
  - "subgraph" here means INDUCED subgraph -- read 19.11 first
  - multigraphs are not supported                                    (19.3)
  - pre-filter by degree/label/size; the search is exponential       (19.13, 19.18)
```

### What trait bounds does my generic function need?

```
I only walk neighbours
  -> G: IntoNeighbors                                                (14.4)
...and I need direction
  -> G: IntoNeighborsDirected                                        (14.5)
I need edge weights while walking
  -> G: IntoEdges  (or IntoEdgesDirected)                            (14.6, 14.7)
I iterate every node / every edge
  -> IntoNodeIdentifiers / IntoNodeReferences / IntoEdgeReferences   (14.8-14.10)
I keep a visited set
  -> + Visitable                                                     (13.2)
I look weights up by id
  -> + Data + DataMap (DataMapMut to write)                          (14.11-14.13)
I index into a side array by node
  -> + NodeIndexable   (+ NodeCompactIndexable only if no holes)     (14.15, 5.10)
I need counts
  -> + NodeCount / EdgeCount                                         (14.14)
Assemble from the recipes 14.17 and the cheat sheet 14.19; check the
implementation matrix 14.16 before assuming a type qualifies.
```

### Adaptor or materialise?

```
I want to run one algorithm on a modified view
  -> adaptor: Reversed / UndirectedAdaptor / NodeFiltered / EdgeFiltered  (13.12-13.14)
     zero allocation, composes, works because algorithms are trait-generic (0.9)
I need to keep the derived graph, mutate it, or return it
  -> materialise: map / filter_map / condensation / FromElements     (10.7-10.10, 16.7, 12.8)
I want a different edge-type marker but the same edges
  -> into_edge_type()  (NOT reverse())                               (9.8, 9.7)
```

### Which Cargo features?

```
Default (graphmap + stable_graph + matrix_graph + std) covers most work.   (2.13)
Persisting graphs?          -> serde-1     (Graph, StableGraph, GraphMap)  (2.13, 12.15)
Parallel algorithms?        -> rayon       (requires std)                  (2.13, 20.9)
Importing DOT?              -> dot_parser                                  (2.13, 12.13)
Generating test graphs?     -> generate                                    (2.13)
Embedded / no_std?          -> default-features = false
                               ... this also disables the three graph
                               families -- re-enable what you need         (2.13)
Never: petgraph = { git = "..." } -- trunk is mid-rewrite                   (0.2)
```

---

## §4 — Operating rules

The full set. Rules 1-8 are modelling, 9-14 are correctness, 15-20 are navigation and versioning.

1. **"Weight" is domain data, not cost.** `N` and `E` are arbitrary types; a node weight can be a
   whole record. Numeric algorithms take cost from a numeric `E` *or* from a closure you pass — so
   never flatten a rich edge payload into a bare `f64` just to run `dijkstra`. Write
   `|e| e.weight().distance` instead. (`§0.5`, `§10.1`, `§10.3`, `§15.1`, `§15.19`)

2. **Separate topology from business data when payloads are large.** Storing a heavy struct inline
   makes every clone, `map`, and matrix cell expensive — `MatrixGraph` in particular allocates per
   cell, so box large edge weights. Keep an index into your own store when the payload is big.
   (`§10.12`, `§10.13`, `§7.7`)

3. **Choose the representation before writing code, and re-check it later.** The five differ on
   parallel edges, index stability, node identity, mutation cost, and memory, and switching later is
   a migration (`§5.8`). Run the selection algorithm at `§2.14`, then confirm against `§2.17`.

4. **Prefer an adaptor to a materialised copy.** `Reversed`, `UndirectedAdaptor`, `NodeFiltered` and
   `EdgeFiltered` are trait-level views over a borrowed graph — they allocate nothing and compose.
   Materialise only when you need to keep or mutate the derived graph. (`§13.12`-`§13.14`, `§14.3`)

5. **Write algorithms against traits, not `&Graph`.** `where G: IntoEdges + Visitable` makes a
   function work on all five representations *and* on adaptors; taking `&Graph<N, E>` locks you out
   of both. Use the recipes at `§14.17` and the cheat sheet at `§14.19`. (`§0.9`, `§14.18`)

6. **Directedness is type-level, direction is runtime.** `Directed`/`Undirected` are `EdgeType`
   markers chosen at the type; `Direction::{Outgoing, Incoming}` is chosen per call. On an
   undirected graph the direction argument is ignored and `neighbors` returns both sides. (`§9.1`,
   `§9.2`, `§9.3`, `§9.5`)

7. **`reverse()` and `into_edge_type()` are not the same operation.** `reverse()` rewrites every
   edge in place; `into_edge_type()` changes only the type marker and leaves edge endpoints
   untouched. Picking the wrong one produces a graph that type-checks and means something else.
   (`§9.7`, `§9.8`, `§9.9`)

8. **Parallel edges and self-loops differ by family — know which contract you are under.**
   `Graph`/`StableGraph` allow parallel edges, so `add_edge` twice yields two edges and `update_edge`
   is the dedupe control. `GraphMap`, `MatrixGraph` and `Csr` have no parallel edges. Downstream
   algorithms inherit the difference: `bridges` under-reports on multigraphs (`§17.7`) and the
   isomorphism family rejects them outright (`§19.3`). (`§4.5`, `§9.6`, `§9.14`)

9. **`Graph::remove_node` invalidates indices, and not only the removed one.** The last node is
   swapped into the freed slot, so an unrelated node's index changes silently. Any `NodeIndex` you
   stored is suspect after a removal. (`§4.6`, `§11.5`, `§11.12`)

10. **`StableGraph` gives index stability, not generation safety.** Indices survive removals, but
    freed slots are reused by later insertions — a handle kept across a remove-then-add can address
    a live, different node with no error. If that matters, carry a generation counter yourself.
    (`§5.1`, `§5.4`, `§11.6`)

11. **Own the mapping between your IDs and petgraph's.** `HashMap<DomainId, NodeIndex>` is the
    standard pattern, and it must be rebuilt whenever you compact or rebuild the graph. Do not leak
    `NodeIndex` — or `Ix` — across a public API boundary. (`§11.10`, `§11.11`, `§11.17`, `§3.12`)

12. **Do not treat `node_count()` as an index bound on `StableGraph`.** It excludes holes, so the
    valid index space is larger than the count. Equivalently, do not assume
    `NodeCompactIndexable`. (`§5.9`, `§5.10`, `§11.8`)

13. **`dijkstra` does not reject negative weights — it returns wrong answers.** Negative edges
    require `bellman_ford` or `spfa`; `find_negative_cycle` is the diagnostic. Decide the
    negative-weight policy at modelling time, not when results look odd. (`§15.3`, `§15.7`-`§15.9`)

14. **Panicking APIs are the default; the fallible ones are opt-in.** `add_edge`, index syntax and
    several matrix operations panic. On untrusted or size-unbounded input use `try_add_node` /
    `try_add_edge` / `try_update_edge`, prefer the `Option`-returning accessors over `g[idx]`, and
    build behind the error-first skeleton. (`§12.7`, `§12.18`, `§4.11`, catalog `§27`)

15. **"Subgraph" in the isomorphism family means *induced* subgraph.** A pattern that appears as a
    non-induced subgraph will not be found. This is the most common wrong assumption in `§19`; read
    `§19.11` before using any of the five APIs. (`§19.7`-`§19.9`, `§19.11`)

16. **Several algorithms are heuristics or approximations — do not present their output as optimal.**
    `greedy_matching`, `dsatur_coloring`, `greedy_feedback_arc_set` and `steiner_tree` are all
    inexact, and `maximum_matching` maximises **cardinality**, not weight. `§18.17` states the
    boundary outright: petgraph is not an optimisation solver. (`§18.3`, `§18.15`, `§20.3`, `§20.4`,
    `§20.8`)

17. **DOT output is debug rendering, not a stable format.** The crate's own docs warn that options
    are simple and exact output may change. Use it for diagnostics and CI artifacts; for golden
    tests, normalise the output or assert graph structure instead. (`§0.10`)

18. **Eleven of the thirty-one catalogued sections have no deep-dive.** §1 and §21-§30 exist only as
    catalog bullets at lines 19 and 272-396. Do not hunt for a `# 23)` chapter — consult `§1.2`
    above for where each topic is really covered, and `§5` for the two that are genuinely
    documented nowhere else.

19. **Target released 0.8.3; treat trunk as unpublished.** The repository is mid-transition to a
    multi-crate architecture and the document's rule is explicit — *"Never assume trunk APIs are
    stable unless user pins a git revision."* Do not take a git dependency, and do not lift API
    shapes from trunk documentation. (`§0.2`)

20. **Feature flags change the API surface, not just the binary size.** `rayon` requires `std`;
    `serde-1` covers only `Graph`, `StableGraph` and `GraphMap`; and `default-features = false` for
    `no_std` also switches off `graphmap`, `stable_graph` and `matrix_graph` — re-enable what you
    need explicitly. Check `§2.13` before assuming a type or algorithm is available.

---

## §5 — Working without a deep-dive

`§1.2` maps each catalog-only section to where its material actually lives. Two topics survive that
mapping with **no coverage anywhere in the document** beyond a single catalog bullet:

| Topic | Catalog bullet | What to do |
|---|---|---|
| **DOT styling** — `Dot::with_attr_getters`, per-node/per-edge attribute closures | line 301 | `§0.10` gives the mental model and `Dot::with_config` with `Config::EdgeNoLabel`; `§0.13` gives the import. For attribute getters, go to docs.rs — and remember Rule 17 before investing in the output shape. |
| **`operator::complement`** | line 287 | mentioned once and never again. Neighbouring transforms (`map`/`filter_map` at `§10.7`-`§10.10`, `condensation` at `§16.7`) are the documented derivation surface. |

Three further gaps are softer — the catalog promises a treatment the deep-dives only supply in
pieces:

* **Testing (`§28`)** has no home chapter. What exists is spread across the per-chapter
  anti-pattern inventories, plus `§0.10` (do not golden-test DOT) and `§19.14` (graph preparation).
  Build the test plan from the invariants in `§4` above rather than looking for `§28`.
* **Benchmarking and big-O by graph type (`§25`)** is real but scattered: `§4.8` (`Graph`), `§7.2`
  (`MatrixGraph` space), `§8.13` (`Csr` construction), `§2.12` (memory footprint), `§19.18`
  (isomorphism cost), `§18.14` (clique cost). There is no single complexity table.
* **Recipes (`§29`)** are all present, but each inside its own chapter — `§4.9` (domain records),
  `§11.10` (external ID map), `§15.14` (path reconstruction), `§16.19` (DAG recipes), `§17.5` (MST
  to graph), `§12.17` (per-representation loading). The catalog bullet list at line 369 is a usable
  checklist of what to go find.

When a topic is catalog-only, the document's own `## Suggested deep-dive order` (line 397) is still
worth reading: it states which chapters the author considered foundational, which is a reasonable
proxy for where the missing material was expected to land.
