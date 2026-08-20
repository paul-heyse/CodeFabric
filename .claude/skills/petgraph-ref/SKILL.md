---
name: petgraph-ref
description: "Reference navigator for Rust petgraph 0.8.3. SKILL.md maps `docs/library_ref/petgraph.md` (22,805 lines): a catalog of §0-§30 followed by deep-dives for §0 and §2-§20 — graph-type selection (`Graph`/`StableGraph`/`GraphMap`/`MatrixGraph`/`Csr`), generics and index types, directedness and edge invariants, weights as domain data, index identity and mutation safety, construction and loading, the `visit` traversal/trait system, and the algorithm catalog (shortest paths · connectivity/SCC/cycles/DAGs · spanning trees/bridges/articulation · matching/flow/cliques/coloring · isomorphism · analytics). REFERENCE.md (same folder) holds the section index with line numbers, the capability→API map, decision trees, operating rules, and a catalog-only coverage map. Use when Rust touches `use petgraph::`/`Graph<`/`DiGraph`/`UnGraph`/`StableGraph`/`GraphMap`/`MatrixGraph`/`Csr`/`NodeIndex`/`EdgeIndex`/`petgraph::algo::`/`petgraph::visit::`/`petgraph::dot::`/`Dfs`/`Bfs`/`Topo`/`toposort`/`dijkstra`/`astar`/`tarjan_scc`/`condensation`/`min_spanning_tree`/`is_isomorphic`, or when editing a `petgraph` dependency or feature list. Arrow/DataFusion analytics → sibling `datafusion-pyarrow-rust-ref`; Delta storage → `deltalake-rust-ref`."
allowed-tools: Read, Grep, Glob, Bash
---

# petgraph Reference Navigator

Routes the deep-dive reference for **Rust petgraph** — in-memory graph data structures, the trait-based `visit` abstraction, and the `algo` catalog. This SKILL.md is the **core map**: version anchors, the document's catalog-vs-deep-dive layout, reading strategy, capability routing, and the key invariants. The companion **`REFERENCE.md`** (same folder) carries the full section index with line numbers, the **capability → API map** (what you are trying to achieve → the petgraph function or type that does it → where it is documented), the decision trees (graph type · shortest path · SCC/cycle · traversal · identity · construction · isomorphism · trait bounds · adaptor-vs-materialize · features), the 20 operating rules, and the **catalog-only coverage map** (§1.2 and §5). Reach for REFERENCE.md as soon as you know *what you want to compute*; cross-references back here are written `SKILL §...`.

**Out of scope** (covered elsewhere): columnar analytics, query planning, and anything Arrow-shaped → sibling **`datafusion-pyarrow-rust-ref`**. Delta Lake storage → **`deltalake-rust-ref`**. Source parsing and fact extraction → **`code-facts-lib-ref`**. Filesystem watching and Git state → **`gix-notify-ref`**. petgraph is a data-structure crate: it holds a graph in memory and runs algorithms over it. It is **not** a graph database, a persistence layer, a NetworkX-style batteries-included toolkit, or a solver (`§0.11`).

---

## Version anchors

* **petgraph 0.8.3** — the released docs.rs line, and the default implementation target. Declare `petgraph = "0.8.3"` (`§0.2`, `§2.13`).
* **Trunk is mid-rewrite.** The repository is transitioning to a new **multi-crate architecture**; the previous release lives on the `0.8` branch. The document's rule is explicit: *"Never assume trunk APIs are stable unless user pins a git revision"* — do not take a `{ git = ... }` dependency, and do not lift API shapes from trunk docs (`§0.2`).
* **Features.** Default: `graphmap`, `stable_graph`, `matrix_graph`, `std`. Optional: `serde-1`, `rayon`, `dot_parser`, `generate`, `unstable`. **`rayon` requires `std`**, and `default-features = false` for `no_std` also switches off the three graph families — re-enable the ones you need explicitly (`§2.13`).
* **Five graph representations, one algorithm surface.** `Graph` (adjacency list, compact indices) · `StableGraph` (adjacency list, indices survive removal) · `GraphMap` (the node *value* is the identifier) · `MatrixGraph` (dense adjacency matrix) · `Csr` (compressed sparse row, static/sparse). Algorithms are written against **traits**, so most of `algo` works across all five — but not all, and the trait-bound matrices per chapter are the authority (`§0.8`, `§0.9`, `§2.1`, `§14.16`).
* **Vocabulary trap.** A "weight" in petgraph is **arbitrary associated data**, not necessarily a numeric cost. Shortest-path cost comes from a numeric edge weight *or* an explicit cost closure; the two are separate concepts (`§0.5`, `§10.1`, `§15.1`).

---

## The reference document

`docs/library_ref/petgraph.md` — **22,805 lines**, one document, **catalog-first then deep-dives**:

| Block | Lines | Contents |
|---|---|---|
| **Catalog** | 1-411 | `## N)` blocks enumerating **§0-§30** in 5-11 bullets each — the complete feature surface, including topics that never get a deep-dive. `## Suggested deep-dive order` closes it at 397. |
| **Deep-dives** | 412-22805 | `# N) Title` chapters for **§0 and §2-§20 only** — 20 chapters, 744-1,522 lines each, uniform `## N.M` subsections. |

**Eleven of the thirty-one catalogued sections have no deep-dive: §1 and §21-§30.** That covers installation/features, adaptors, operators, DOT/Graphviz output, serialization, parallelism, `no_std`, error handling, testing, recipes, and the decision tables. This is by far the most important navigational fact about this document — searching for a `# 23)` chapter will not find one. Most of that material *is* present, scattered through the deep-dives; **REFERENCE.md §1.2 maps every catalog-only section to where it is actually covered**, and **§5** handles the residue — the two topics (DOT styling via `Dot::with_attr_getters`, and `operator::complement`) that genuinely appear nowhere but a catalog bullet, plus the three the deep-dives only supply in pieces.

**Reading strategy.** Locate the section in REFERENCE.md §1, then `Read(offset, limit)`. Chapters open with a `N.0` type shape / core-imports block and close with an **anti-pattern inventory + deployment checklist** — load the closing 60-100 lines before drafting code; they are where the failure modes live. The algorithm chapters (§15-§20) are organized **one subsection per function**, with the function name in the heading, so REFERENCE.md §2 can take you straight to it. Several chapters end in a **decision table** (`§2.17`, `§13.17`, `§15.16`, `§16.20`, `§17.17`, `§19.19`, `§30` in catalog) — read the table before the prose when you are choosing rather than implementing. Navigation is otherwise easy here: uniform `## N.M` numbering, only one `#`-comment decoy inside a fence, 26% blank lines.

---

## Where do I look?

Routing by **what you are trying to achieve**, not by API name. The full map — ~90 outcomes to specific functions — is REFERENCE.md **§2**.

| I want to… | Go to |
|---|---|
| pick a graph type, or find out whether I picked wrong | **§2** (matrix at `2.1`, selection algorithm at `2.14`, final table at `2.17`) |
| understand why my `NodeIndex` now points at the wrong node | **§11** (invalidation table `11.5`, slot reuse `11.6`) · **§4.6** · **§5.1** |
| key nodes by my own domain ID instead of `NodeIndex` | **§6** (`GraphMap`) · **§11.10-§11.11** (external ID map) · **§6.15** for the trade |
| build a graph from records, an edge list, a matrix, DOT, or Graph6 | **§12** (`12.4`-`12.6` dedupe, `12.10`-`12.15` loaders, `12.7` fallible APIs) |
| find shortest paths / costs | **§15** — ten algorithms, one per subsection; decision table `15.16`, output matrix `15.17` |
| detect cycles, order a DAG, or find strongly connected components | **§16** (`is_cyclic_*`, `toposort`, `tarjan_scc`/`kosaraju_scc`, `condensation`, `Acyclic`) |
| find critical edges/vertices or a spanning tree | **§17** (`bridges`, `articulation_points`, `min_spanning_tree`, `UnionFind`) |
| match, route flow, find cliques, or colour a graph | **§18** (`maximum_matching`, `dinics`, `ford_fulkerson`, `maximal_cliques`, `dsatur_coloring`) |
| test whether two graphs — or a pattern and a graph — are the same shape | **§19** (`is_isomorphic*`, `subgraph_isomorphisms_iter`; read `19.11` on *induced* semantics first) |
| rank nodes, break cycles, reduce a DAG, or compute dominators | **§20** (`page_rank`, `greedy_feedback_arc_set`, `steiner_tree`, `tred`, `dominators::simple_fast`) |
| walk a graph myself, with filtering or reversal | **§13** (`Dfs`/`Bfs`/`DfsPostOrder`/`Topo`, `depth_first_search` events, `NodeFiltered`/`EdgeFiltered`/`Reversed`) |
| write a function generic over graph types | **§14** (trait-by-trait, bound recipes `14.17`, cheat sheet `14.19`, implementation matrix `14.16`) |
| decide what to store in a node or edge, and transform it later | **§10** (payload patterns `10.4`-`10.5`, `map`/`filter_map` family `10.7`-`10.11`) |
| get directedness, self-loops, or parallel edges right | **§9** (invariant table `9.14`, modelling table `9.15`) |
| emit DOT, enable `serde`, or turn on `rayon` | catalog **§23** / **§24** / **§25** — *no deep-dive*; see REFERENCE.md **§1.2** for where each is really covered |

---

## Key invariants

The seven that prevent the most errors; the full set of **20 operating rules** is in `REFERENCE.md` §4.

1. **A "weight" is domain data, not a cost.** `N` and `E` are arbitrary associated types. Numeric algorithms take the cost either from a numeric `E` or from an explicit closure — so a rich edge payload is fine as long as you pass `|e| e.weight().distance`. Conflating the two is the single most common petgraph modelling error. (`§0.5`, `§10.1`, `§10.3`, `§15.1`, `§15.19`)
2. **`NodeIndex` is a graph-local handle, and `Graph::remove_node` invalidates it.** Removal swaps the last node into the freed slot, so *some other* node's index silently changes. `StableGraph` preserves indices across removals — but it reuses slots later, so **stability is not generation safety**; a stale handle can point at a live, wrong node. Store external IDs in a `HashMap<DomainId, NodeIndex>` and rebuild after compaction. (`§4.6`, `§5.1`, `§11.5`, `§11.6`, `§11.12`)
3. **Algorithms target traits, not concrete types.** Write `where G: IntoEdges + Visitable` rather than taking `&Graph<N, E>`, and your function works on all five representations plus adaptors. Which type implements which trait is `§14.16`; the bound recipes are `§14.17`. (`§0.9`, `§14`)
4. **Adaptors are views, not copies.** `Reversed`, `UndirectedAdaptor`, `NodeFiltered`, `EdgeFiltered` wrap a graph reference and re-implement the traits — run an algorithm on a reversed or filtered graph without materialising one. Materialise only when you need to *keep* the derived graph. (`§13.12`-`§13.14`, `§14.3`)
5. **Directedness is type-level; traversal direction is runtime.** `Directed`/`Undirected` are `EdgeType` markers fixed at the type; `Direction::{Outgoing, Incoming}` selects at call time. `reverse()` rewrites edges in place, while `into_edge_type()` changes only the marker and leaves edges untouched — they are not interchangeable. (`§9.1`, `§9.2`, `§9.7`, `§9.8`)
6. **Parallel edges and self-loops vary by family.** `Graph`/`StableGraph` accept parallel edges — `add_edge` twice gives you two edges; `update_edge` is the dedupe control. `GraphMap`, `MatrixGraph` and `Csr` have no parallel edges. Algorithms that assume simple graphs (isomorphism, bridges) inherit the caveat. (`§4.5`, `§9.6`, `§17.7`, `§19.3`)
7. **DOT output is debug rendering, not a wire format.** The crate's own docs warn that formatting is simple and *"exact output may change"*. Use it for diagnostics and CI artifacts; never golden-test the string — normalise it or assert graph structure instead. (`§0.10`)
