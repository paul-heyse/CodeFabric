# Fact-domain map

The suite's strongest structural property is that **the same fact domains appear, in near-
parallel order, in four of the six domain specifications**: the ontology says what a fact *is*, fact generation
says how it is *produced* and by which provider, the data fabric gives it a *table*, and the
query spec gives it *phrases*. The lifecycle spec assigns it to an update *lane*.

None of that correspondence is written down in the specs — `rg '§'` finds two section-level
cross-references in the whole suite. This file is that correspondence.

See [`README.md §2`](./README.md#2-citation-convention) for the tag convention. Library
shorthands used in the last column are expanded in
[`library-routing.md §1`](./library-routing.md#1-reference-shorthands).

## 1. Core domain matrix

Ordered by `ONT` Part I. Python and Rust profile sections are folded into the row for the core
domain they extend, tagged `Py`/`Rs`.

| Domain | ONT | GEN | FAB (table) | QRY | Provider / library |
|---|---|---|---|---|---|
| Source and lexical | §5 | §15 Py · §35 Rs | §17 `source_file` · §18 `source_token` · §19 `source_annotation` | §50 | `ruff` §3 source coordinates, §4 lexer/tokens, §6 trivia · `ts` §6 text input |
| Syntax | §6 | §16 Py · §35 Rs | §20 `syntax_detail` | §51 | `ts` §7 CST model, §20 error recovery, §22 static node types, **§45 Python** · `ruff` §5 typed AST |
| Semantic identity | §7 | §17 Py · §36 Rs | §21 `semantic_detail` | §52 · §81 Rs | `ruff` §8 semantic model · `pyrefly` §25 semantic identity · `mir` §7 item discovery, §37 stable identifiers |
| Scope, binding, name resolution | §8 · §33 Py scope · §34 Py binding | §18 | §22 `scope_detail` · §23 `binding_detail` · §24 `reference_detail` | §52 · §71 Py | `ruff` §8 `ruff_python_semantic` (scopes, bindings, references) |
| Module, import, export, dependency | §9 | §19 | §25 `module_import_detail` | §53 | `ruff` §8 import semantics · `pyrefly` §4 module identity, §26 imports/exports/re-exports · `mir` §5 cargo metadata, §40 dependency graph |
| Types | §10 · §35 Py · §47 Rs | §20 Py · §37 Rs | §26 `type_detail` · §27 `type_fact_detail` | §54 · §72 Py · §83 Rs | `pyrefly` §7 type model, §8 narrowing, §10 type-table extraction · `mir` §16 types/generics/normalization |
| Members and object model | §11 · §36 Py · §53 Rs traits | §21 Py · §42 Rs | §28 `member_relation_detail` | §55 · §73 Py · §89 Rs | `pyrefly` §12 class attributes/properties, §27 inheritance/MRO/protocols · `mir` §23 trait dispatch/vtables |
| Callable contracts | §12 · §45 Rs decl properties | §22 | §29 `callable_detail` · §30 `parameter_detail` | §56 | `ruff` §5 typed AST · `pyrefly` §11 call/callee extraction · `mir` §8 `Body` anatomy, §9 locals/arguments |
| Call sites | §13 | §23 Py · §41 Rs | §31 `call_site_detail` · §32 `call_argument_detail` | §57 · §74 Py · §88 Rs | `pyrefly` §11 `get_callees_with_location` · `mir` §21 direct call edges, §22 fn pointers/closures/indirect |
| Dispatch and executable instances | §14 · §52 Rs · §53 Rs | §23 Py · §41 Rs · §42 Rs | §33 `call_target_detail` · §53 `rust_instance` · §57 `rust_vtable_entry` | §57 · §74 Py · §88 Rs · §89 Rs | `pyrefly` §13 qualified targets/subtype, §27 dispatch · `mir` §20 `Instance::resolve`, §23 vtables/over-approximation |
| Control flow | §15 · §16 derived | §24 Py · §39 Rs | §34 `cfg_graph` · §35 `cfg_node_detail` · §36 `cfg_edge_detail` | §58 | `ruff` §5 (CodeFabric builds its own Python CFG) · `mir` §10 basic blocks/CFG, §12 terminators/unwind edges, §25 MIR CFG → CPG |
| Values and computation | §17 | §25 Py · §38 Rs | §37 `value_detail` · §38 `operation_detail` | §59 | `mir` §13 operands/constants/moves, §15 rvalues |
| Definition/use and dataflow | §18 | §25 Py · §45 Rs | §39 `dataflow_event_detail` | §59 | `mir` §26 read/write/move/copy edges, §30 reaching definitions/def-use |
| Abstract memory and state locations | §19 · §49 Rs places | §26 Py · §40 Rs | §40 `memory_location_detail` · §41 `access_path_component` · §42 `memory_access_detail` | §60 · §85 Rs | `mir` §14 places and projections, §28 place abstraction/access paths |
| Alias and points-to | §20 | §26 Py · §46 Rs | §40–§42 + `FAB §88` fixed point | §60 | `mir` §28 alias domains · `pg` traversal chapters for the constraint graph |
| Program-point state | §21 · §50 Rs MIR transitions | §44 Rs | §43 `program_state_detail` | §61 · §86 Rs | `mir` §11 statements/state transitions, §29 move paths and initialization |
| Effects | §22 | §27 Py · effect sites across §§47–50 Rs | §44 `effect_detail` | §62 | model packs (`GEN AC-G-38`) — no library covers this; `mir` §34 unsafe/FFI supplies Rust evidence |
| Exceptional flow | §23 | §28 Py | §45 `exception_detail` | §63 | `mir` §12 unwind edges, §24 drop glue |
| Resource lifetime | §24 · §42 Py context managers | §29 Py · §47 Rs | §46 `resource_event_detail` | §64 · §79 Py · §91 Rs | `mir` §24 drop glue, §27 ownership edges |
| Async and concurrency | §25 · §43 Py · §56 Rs | §30 Py · §48 Rs | §47 `async_event_detail` | §65 · §80 Py · §92 Rs | `pyrefly` §7 type model for awaitables · `mir` §32 closures/async/coroutines |
| Closures and capture | §26 | §31 Py · §48 Rs | §48 `capture_detail` | §66 | `mir` §22 callable operands, §32 captures |
| Generated and lowered code | §27 · §54 Rs macros | §32 Py · §43 Rs | §49 `generated_detail` · §58 `rust_macro_expansion` | §67 · §90 Rs | `mir` §17 source spans/macro expansion, §19 generic vs monomorphized MIR |
| Generics and specialization | §28 · §46 Rs | §37 Rs | §26 `type_detail` · §53 `rust_instance` | §67 · §82 Rs | `pyrefly` §27 generics · `mir` §16 generics, §19 monomorphization |
| Objective graph-analysis facts | §29 · §60 projections | §53–§64 | §60 `derived_component` · §83–§88 calculations | §68 | `pg` §§2–20 graph types and algorithm catalog · `df` custom operators |
| Objective structural metrics | §30 | §65 | §61 `metric` | §68 | `pg` analytics chapters · `df` UDAF (`FAB §79`) |
| Interprocedural summaries | §31 | §66 | §62 `callable_summary` · §89 fixed point | §69 | `mir` §35 interprocedural summaries · `pg` SCC/condensation |
| Explicit unknowns | §32 · `AC-G-73` | §33 Py · §51 Rs · §84 materialization rules | §59 `unknown_detail` | §70 | `pyrefly` §28 dynamic Python/uncertainty · `mir` §23 dynamic over-approximation, §58 capability gaps |

### 1.1 Rust-only domains

| Domain | ONT | GEN | FAB | QRY | Library |
|---|---|---|---|---|---|
| Rust source-semantic entities | §44 | §36 | §21 `semantic_detail` | §81 | `mir` §7 crate and item discovery |
| MIR bodies and structure | §48 | §38 | §51 `rust_mir_body` · §52 `rust_mir_local` | §84 | `mir` §8 `Body`, §9 locals, §18 visitor APIs |
| Ownership, loans, regions | §51 | §44 | §54 `rust_loan` · §55 `rust_region` · §56 `rust_move_path` | §87 | `mir` §27 borrows/references/ownership edges, §29 move paths |
| Drop and destruction | §55 | §47 | §46 `resource_event_detail` | §91 | `mir` §24 drop glue, shims, intrinsics |
| Unsafe, FFI, inline assembly | §57 | §50 | — (properties on `entity`/`property_fact`) | §93 | `mir` §34 unsafe operations, FFI, inline asm |
| Constants, statics, CTFE | §58 | §49 | — (properties) | §94 | `mir` §33 constants, statics, CTFE |

### 1.2 Python-only domains

| Domain | ONT | GEN | FAB | QRY | Library |
|---|---|---|---|---|---|
| Dynamic semantics | §38 | §33 | §50 `python_dynamic_detail` | §75 | `pyrefly` §28 dynamic Python and uncertainty modeling |
| Decorators | §39 | §22 | §29 `callable_detail` | §76 | `ruff` §5 typed AST · `pyrefly` §11 decorator-aware call targets |
| Pattern matching | §40 | §24 | §34–§36 CFG tables | §77 | `ruff` §5 typed AST |
| Comprehensions | §41 | §24 | §34–§36 CFG tables | §78 | `ruff` §5 typed AST |

### 1.3 Ontology and contract plane

`FAB §6.3` serves the governed vocabulary and recursive contract metadata under
`cpg_ontology`. These are bundle-pinned dimensions, not present-state source facts, so they
do not belong to one language lane.

| Plane | FAB relations | Governing source | Primary consumer |
|---|---|---|---|
| Vocabulary | `enum_domain`, `entity_kind`, `entity_family`, `relation_kind`, `relation_family`, `property_kind`, `fact_kind`, `provider_raw_kind`, `id_domain` | ontology/enum/provider registries and ID-domain Contract IR | compiled semantic plans, serving decoration, publication validation |
| Recursive contracts | `ontology_term`, `ontology_edge`, `registry_authority`, `semantic_type_binding`, `table_contract`, `column_contract`, `result_schema`, `result_field`, `identity_recipe`, `phrase_binding`, `rule_contract` | one `SchemaContractCompilation` output | catalog-only discovery, result shaping, relational integrity gates |

Raw-provider-kind identity is scoped by provider, raw catalog, raw namespace, and native
numeric code. ID columns resolve through `id_domain`; result fields resolve through
`result_schema`/`result_field`; N:M ontology membership resolves only through
`ontology_edge`.

## 2. Lifecycle lanes

`LIFE` Part VI (§93–§99) assigns every domain to one of four update lanes. The lane determines
what is republished on an edit and how fast.

| Lane | LIFE § | Domains it owns | Provider |
|---|---|---|---|
| Fast syntax lane | §94 | source and lexical, syntax | Tree-sitter (incremental where safe), Ruff lexer |
| Python semantic lane | §95 | semantic identity, scopes/bindings, modules, types, members, callables, call sites, dispatch, CFG, dataflow — Python side | Ruff + Pyrefly sidecar |
| Rust semantic lane | §96 | the same domains plus MIR, places, ownership, drop, unsafe, CTFE — Rust side | `rustc_public` extractor |
| Owner-local derived lane | §97 | graph-analysis facts, structural metrics, alias/points-to within an owner | petgraph, DataFusion custom operators |
| Interprocedural derived lane | §98 | interprocedural summaries, effects, transitive facts | SCC fixed point over the call graph |

`LIFE §93 Pipeline overview` states the ordering; `LIFE §99 Validation stages` gates
activation. Nothing may skip a lane: a domain's facts become current only when its lane
republishes.

## 3. Relation-name join — `ONT` Part VII ↔ `GEN` Part VI

`ONT` Part VII (§69–§81) is the de-facto relation registry: 13 grouped fences containing ~154
`UPPER_SNAKE` relation names. `GEN` Part VI (§67–§79) is a matrix keyed **on the same names**,
one row per relation, giving the Python source, the Rust source, and the reconciliation rule.
They are the tightest join in the corpus and neither document mentions the other.

| Relation group | ONT § | GEN § | FAB carrier |
|---|---|---|---|
| Structural | §69 | §67 · §67A source and lexical | `relation` (§15), `entity` (§14) |
| Symbol and binding | §70 | §68 | §22–§24 scope/binding/reference detail |
| Module and dependency | §71 | §69 | §25 `module_import_detail` |
| Type | §72 | §70 | §26–§27 type detail |
| Member | §73 | §71 · §71A Python · §71B Rust | §28 `member_relation_detail` |
| Invocation | §74 | §72 | §31–§33 call site/argument/target |
| Control-flow | §75 | §73 | §34–§36 cfg tables |
| Dataflow | §76 | §74 | §39 `dataflow_event_detail` |
| Memory | §77 | §75 | §40–§42 memory tables |
| Ownership/lifetime | §78 | §76 | §54–§56 loan/region/move-path |
| Effect | §79 | §77 · §77A exceptional · §77B resource · §77C async · §77D closure · §77E program-point | §44–§48 |
| Generated/lowered | §80 | §78 | §49 · §58 |
| Derived graph | §81 | §79 | §60 `derived_component` |

Note the letter-suffixed `GEN` sections — `§67A`, `§71A`, `§71B`, `§77A`–`§77E`. They are real
sections, easy to miss when reading a numeric range.

## 4. Delta table registry

Six catalog namespaces, defined once at **`FAB §6.3`** and used nowhere else in the suite:
`cpg_control` · `cpg_base` · `cpg_python` · `cpg_rust` · `cpg_derived` · `cpg_serving`.

**62 named tables.** `FAB` gives one section per table — 12 control-plane under `FAB §13`
(§13.8 defines two, `serving_snapshot_manifest` and `active_snapshot`), 4 universal core
(§14, §15, §16.1, §16.2), and 46 detail, extension and derived tables across §17–§62.
`FAB` Appendix A restates the full set as a topological dependency order — use it when deciding
creation or publication order.

### 4.1 Control plane — `FAB §13`

`§13.1 workspace` · `§13.2 common_repository` · `§13.3 analysis_context` ·
`§13.4 analysis_context_set` · `§13.5 publication` · `§13.6 publication_table` ·
`§13.7 current_publication` · `§13.8 serving_snapshot_manifest` and `active_snapshot` ·
`§13.9 owner` · `§13.10 capability_status` · `§13.11 diagnostic` ·
`§13.12 Operational read-only views`

The durable backing store for operational state is embedded SQLite in WAL mode
(`LIFE AC-G-27`, `LIFE §130`) — **no crate is named anywhere in the suite**; see the
[gap register](./README.md#74-library-coverage-gaps).

### 4.2 Universal graph core — `FAB` Part IV

`§14 entity` · `§15 relation` · `§16 First-class property facts and evidence` (defines
`property_fact` and `fact_evidence`)

These four are the canonical fact substrate. Everything else is a detail table keyed to them.

### 4.3 Detail tables by fact domain

| Part | FAB §§ | Tables |
|---|---|---|
| IV — Universal Graph Tables | §17–§25 | `source_file` `source_token` `source_annotation` `syntax_detail` `semantic_detail` `scope_detail` `binding_detail` `reference_detail` `module_import_detail` |
| VI — Types, Members, Calls, Control Flow | §26–§36 | `type_detail` `type_fact_detail` `member_relation_detail` `callable_detail` `parameter_detail` `call_site_detail` `call_argument_detail` `call_target_detail` `cfg_graph` `cfg_node_detail` `cfg_edge_detail` |
| VII — Values, Dataflow, Memory, State | §37–§43 | `value_detail` `operation_detail` `dataflow_event_detail` `memory_location_detail` `access_path_component` `memory_access_detail` `program_state_detail` |
| VIII — Effects, Exceptions, Resources, Async, Generated | §44–§49 | `effect_detail` `exception_detail` `resource_event_detail` `async_event_detail` `capture_detail` `generated_detail` |
| IX — Python and Rust Extension Tables | §50–§58 | `python_dynamic_detail` · `rust_mir_body` `rust_mir_local` `rust_instance` `rust_loan` `rust_region` `rust_move_path` `rust_vtable_entry` `rust_macro_expansion` |
| X — Unknowns, Derived, Metrics, Summaries | §59–§62 | `unknown_detail` `derived_component` `metric` `callable_summary` |

## 5. Serving surface — what a query actually reaches

Queries never touch the tables above directly. `FAB` Part XV defines the stable surface:

| Surface | FAB § | Contents |
|---|---|---|
| Overlay-aware catalog provider | §91 | `ServingSnapshot`-pinned; merges durable Delta base with the consolidated hot overlay |
| Stable serving views | §92 | 23 `cpg_serving.*` views — `aliases` `async_relations` `call_graph` `callable_summaries` `calls` `cfg_edges` `cfg_nodes` `def_use` `effects` `entities` `exceptions` `files` `generated` `members` `memory_accesses` `metrics` `relations` `resources` `symbols` `syntax` `types` `unknowns` `value_flow` |
| Table functions | §93 | `cpg_neighbors` · `cpg_reachable` · `cpg_source_context` · `cpg_owner_facts` |
| Query-planning policy | §94 | what the planner may and may not do against the pinned snapshot |

Scalar and aggregate UDFs live at `FAB §78`/`§79`: `cpg_id_set_union`, `cpg_fact_checksum`,
`cpg_flags_or`. The custom physical operators — `CpgGraphTraverse`, `CpgStrongComponents`,
`CpgDominators`, `CpgPostDominators`, `CpgControlDependence`, `CpgNaturalLoops`,
`CpgReachingDefinitions`, `CpgLiveness`, `CpgPointsTo`, `CpgSummaryFixpoint` — are specified at
`FAB §81`/`§82` with execution requirements at `FAB §90`.

**Which engine owns which derived fact** is settled by `FAB §79A Derivation registry and
single-authority matrix`, with the materialization decision at `FAB AC-G-42`. `FAB §82` is
explicit that the data fabric's physical graph representation is Arrow CSR, **not** petgraph —
petgraph lives on the fact-generation side (`GEN §52 Petgraph role`). Do not carry a petgraph
type across that boundary.

## 6. Reconciliation — where provider observations become canonical facts

No provider writes a canonical row. `GEN` Part VII is the algorithm surface, `FAB AC-G-37` is
the contract:

| Concern | GEN § | Authority |
|---|---|---|
| Range reconciliation | §80 | Tree-sitter vs Ruff source ranges |
| Declaration reconciliation | §81 | per-fact-family authority tables at `GEN §5` |
| Type reconciliation | §82 | Pyrefly over Ruff (Python); `rustc_public` (Rust) |
| Call-target reconciliation | §83 | exact / resolved / possible / unknown remainder |
| Explicit unknown materialization | §84 | `ONT AC-G-73` |
| Capability reporting | §85 | `GEN AC-G-36` |

Conflicting evidence is retained in `fact_evidence` (`FAB §16`) and diagnostics
(`FAB §13.11`); it is never silently dropped.

## 5. Compiled-program path

For every row above, authored ontology/contract data is projected into a normalized Arrow
program member, lowered through the single DataFusion compiler, and evaluated against exact-
version factual tables. This does not change fact meaning or provider authority. `ONT`'s
*Compiled ontology-program projection*, `GEN`'s *Provider boundary for compiled ontology
programs*, and `FAB`'s *Arrow/DataFusion ontology-program and activation authority* are the
normative amendment anchors.
