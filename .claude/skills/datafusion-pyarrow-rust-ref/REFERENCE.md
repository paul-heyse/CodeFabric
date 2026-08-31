# DataFusion 55 + Arrow 59 — Detailed Reference

This is the mechanical layer behind [SKILL.md](SKILL.md): section indexes with verified
line numbers, the pattern→section binding matrix, the principle binding table, decision
trees, and operating rules. Come here when you know *what* you need and want the exact
place to read; use SKILL.md first when you are still classifying the problem.

**Line-number policy: seek by line, cite by section.** Line numbers appear only in this
file's §1 tables (and the §2 P-table), because line numbers move when a document is
regenerated and section identifiers do not. Every index table is headed by the exact
command that re-derives it — if a `Read(offset)` lands on the wrong heading, re-derive
before trusting anything else in that table.

## Document aliases

Aliases follow `docs/spec_index/library-routing.md` §1 and must stay in sync with it.

| Alias | Document (under `docs/library_ref/`) | Chapters | Lines |
|---|---|---|---:|
| `df` | `datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md` | §0–§40, §40A | 115,587 |
| `df-schema` | same file | S1–S15 | — |
| `df-plan` | same file | §41–§56 | — |
| `df-calc` | same file | C1–C13 | — |
| — | same file, Part III | V1–V6 upgrade gates | — |
| `arrow` | `arrow_rust_59_datafusion55_advanced_reference_2026-08-23.md` | §0–§28 | 34,372 |
| `principles` | `full_data_fabric_design_principles_v2.md` | staticness test + P1–P36 + Y1–Y7 | 2,189 |
| `align` | `datafusion55_arrow59_design_principle_alignment_manual_2026-08-24.md` | §0–§2 · P1–P25 · pattern families · flows §4–§11 · §12–§25 · App. A | 2,294 |

`REFERENCE §N` and `SKILL §…` refer to this skill's own files, never to a document.

## Table of contents

- §1 — Per-document section indexes (`df` in §1.1, `arrow` in §1.2, `principles` in §1.3, `align` in §1.4)
- §2 — Binding matrix: utilization patterns → reference sections; P1–P25 binding table
- §3 — Decision trees (document choice · flow classifier · extension ladder · capability-status evidence · upgrade gates)
- §4 — Operating rules
- §5 — Project context: CodeFabric

---

## §1 — Per-document section indexes

### §1.1 `df` / `df-schema` / `df-plan` / `df-calc` — the comprehensive DataFusion reference

Re-derive with:

```bash
rg -n '^# DataFusion Advanced — |^# Part ' docs/library_ref/datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md
rg -n '^## V[1-6]\) ' docs/library_ref/datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md
```

**Hazard:** the file contains ~250 spurious h1s — authoring slips shaped like `# C1.6 …`
… `# C13.22 …` and `# 37.3 …`–`# 37.13 …`, plus fenced code comments at column 0 — so
bare `rg '^# '` and a bare `just lib-outline` are noisy. Always use the anchored
patterns above. Front matter: doc plan at line 1, documentation map at 128, expansion
order at 954; the body starts at §0 (line 987). Body chapter h1s carry the prefix
`# DataFusion Advanced — `; the titles below omit it.

**Part I (`df` §0–§40, §40A):**

| § | Line | Title |
|---|---:|---|
| §0 | 987 | Scope, versioning, and mental model |
| §1 | 1473 | Installation, crate selection, and Rust project layout |
| §2 | 2104 | First executable Rust app |
| §3 | 2677 | Session model and execution state |
| §4 | 3499 | Data model: Arrow, schemas, arrays, and batches |
| §5 | 4384 | SQL API |
| §6 | 5060 | SQL syntax reference map |
| §7 | 6108 | SQL data types and Arrow type mapping |
| §8 | 6931 | DDL and catalog-affecting SQL |
| §9 | 8003 | DML and write paths |
| §10 | 9015 | DataFrame API |
| §11 | 9899 | Expression API |
| §12 | 10822 | Built-in functions catalog |
| §13 | 12060 | Nested data support |
| §14 | 13096 | Data sources and file formats |
| §15 | 14070 | Parquet deep dive |
| §16 | 15259 | Object stores and remote locations |
| §17 | 16027 | Catalogs, schemas, and tables |
| §18 | 17249 | Custom `TableProvider` |
| §19 | 18535 | Logical plans |
| §20 | 19731 | Physical plans and execution operators |
| §21 | 20971 | Streaming execution model |
| §22 | 22116 | Query optimizer |
| §23 | 23238 | Join algorithms and join tuning |
| §24 | 24506 | User-defined functions |
| §25 | 25621 | Extending SQL syntax |
| §26 | 26603 | Custom logical and physical operators |
| §27 | 27832 | Configuration system |
| §28 | 28836 | Memory management and spilling |
| §29 | 30007 | Performance tuning guide |
| §30 | 31313 | Metrics, profiling, and explainability |
| §31 | 32677 | CLI as deployment and debugging tool |
| §32 | 33463 | Testing and correctness |
| §33 | 35068 | Error handling and diagnostics |
| §34 | 36291 | API stability, upgrades, and version migration |
| §35 | 37289 | Architecture and crate organization |
| §36 | 38420 | Plan serialization and interoperability |
| §37 | 39554 | Distributed and ecosystem integrations |
| §38 | 40304 | Production deployment patterns in Rust |
| §39 | 41510 | Security and governance |
| **§40A** | **42638** | **DataFusion 55 source-verified capability reconciliation** |
| §40 | 43245 | Best practices and anti-patterns |

§40A is the authority for what DataFusion 55 actually ships (`ScanArgs`,
`PhysicalPlanningContext`, `apply_expressions`, `replace_children`, statistics contexts,
dynamic filters, `file_row_index()`, merge-into surfaces, `is_strict`,
`convert_to_state`, spill pluggability, work stealing, self-serialization hooks). It
takes precedence over stale illustrative version strings anywhere in the imported deep
dives.

**Part II-A (`df-schema` S1–S15, from line 44283):**

| § | Line | Title |
|---|---:|---|
| S1 | 44285 | Schema lifecycle and invariants across DataFusion |
| S2 | 46155 | Schema creation surfaces and factory patterns |
| S3 | 48361 | Schema inference, explicit overrides, and multi-file drift |
| S4 | 50122 | Naming, identifier normalization, qualifiers, and output field names |
| S5 | 51585 | Type compatibility, coercion, and schema equality |
| S6 | 53334 | Schema evolution and migration lifecycle |
| S7 | 55155 | Schema metadata, Arrow extension types, and semantic annotations |
| S8 | 56885 | Constraints, functional dependencies, defaults, and table contracts |
| S9 | 58370 | Catalog schema management, remote metastores, and `information_schema` |
| S10 | 60304 | Custom `TableProvider` schema adaptation and projection mapping |
| S11 | 61978 | Logical-plan schema propagation and operator output contracts |
| S12 | 63284 | Nested, partition, and virtual-column schemas |
| S13 | 64755 | View, CTAS, and derived-table schema stability |
| S14 | 66180 | Schema testing, diagnostics, and error cookbook |
| S15 | 67657 | Schema security, governance, and tenant isolation |

**Part II-B (`df-plan` §41–§56, from line 69045):**

| § | Line | Title |
|---|---:|---|
| §41 | 69047 | End-to-end planning lifecycle and phase boundary map |
| §42 | 70337 | SQL planner and binder internals: `sqlparser` AST → `SqlToRel` → `LogicalPlan` |
| §43 | 71868 | Programmatic logical planning with `DataFrame`, `Expr`, and `LogicalPlanBuilder` |
| §44 | 73831 | Plan schema, column identity, aliases, and qualifier governance |
| §45 | 75538 | Expression lifecycle: unresolved SQL expression → bound `Expr` → physical expression |
| §46 | 77088 | Logical plan validation and policy linting before optimization/execution |
| §47 | 78840 | Planner metadata: statistics, constraints, functional dependencies, partitioning, and ordering |
| §48 | 80295 | Analyzer and logical optimizer rule cookbook |
| §49 | 81652 | Physical planning and logical-to-physical lowering map |
| §50 | 83157 | Physical plan properties: partitioning, ordering, equivalence, boundedness, and emission |
| §51 | 84341 | Scan planning and source pushdown: `TableProvider`, file scans, and custom sources |
| §52 | 85904 | Join planning decision model |
| §53 | 87639 | Streaming topology, boundedness, and pipeline-breaker planning |
| §54 | 89161 | Runtime execution planning: partitions, task scheduling, memory reservations, and spill |
| §55 | 90523 | Planning artifact package: reproducible plan debug bundle |
| §56 | 92426 | Plan serialization, caching, fingerprints, and invalidation |

**Part II-C (`df-calc` C1–C13, from line 94142):**

| § | Line | Title |
|---|---:|---|
| C1 | 94144 | User-defined calculation architecture and decision tree |
| C2 | 96103 | Calculation lifecycle and invariants |
| C3 | 97972 | Function registry, cataloging, and discovery |
| C4 | 99988 | Function package and plugin architecture |
| C5 | 102174 | Signature design and overload resolution |
| C6 | 103631 | Return type, nullability, and metadata inference |
| C7 | 105034 | Null, NaN, infinity, error, and invalid-input semantics |
| C8 | 106615 | Vectorized Arrow implementation patterns |
| C9 | 108281 | Complex conditionality and expression composition |
| C10 | 109642 | Nested and structured return calculations |
| C11 | 110923 | External-library integration strategy |
| C12 | 112275 | Async UDFs for external I/O and services |
| C13 | 113681 | Aggregate UDF state design |

**Part III (upgrade gates, h2s from line 115485).** Note the physical order: **V6 sits
between V4 and V5 in the file** — a "V1–V5" sweep misses the corrections log.

| Gate | Line | Title |
|---|---:|---|
| V1 | 115487 | Compile-time trait gate |
| V2 | 115510 | Schema and source gate |
| V3 | 115523 | Calculation gate |
| V4 | 115540 | Plan/reproducibility gate |
| V6 | 115553 | Source-verified corrections incorporated in this revision |
| V5 | 115584 | Final source-of-truth rule |

### §1.2 `arrow` — the standalone Arrow 59 reference

Re-derive with:

```bash
rg -n '^# [0-9]+\) ' docs/library_ref/arrow_rust_59_datafusion55_advanced_reference_2026-08-23.md
```

and drop any hit at a line ≤ 590 (one stray in-catalog h1 sits at line 427).

**Hazards:** the file is h2-rooted at line 1, so `just lib-outline` starts mid-file. The
preamble topic map promises chapters 29 (security/malformed input) and 30
(PyArrow-to-Rust migration recipes), but the **body ends at §28** — those two exist only
as catalog stubs. Four in-body pointers (~lines 10991, 15554, 16836, 20370) cite retired
predecessor references; the doc's own version matrix and refresh notes supersede them.
The preamble's migration ledger and six subsection titles name predecessor release
versions — never copy those headings verbatim into any tracked file (see REFERENCE §4
rule 4).

**Preamble blocks (h2, lines 1–598):** doc plan (1) · canonical version matrix (8) —
Arrow/Parquet/Flight/Avro 59.2.0, DataFusion 55.0.0, `object_store` 0.13.2,
`pyo3-arrow` 0.19.0 · migration ledger, 58→59 (25) · Parquet/object-store migration
note (73) · Python zero-copy interop migration (91) · combined-stack invariants (105) ·
topic map (148–577) · recommended deep-dive order (578).

**Body chapters (h1 `# N)`, §0–§28):**

| § | Line | Title |
|---|---:|---|
| §0 | 599 | Scope, versioning, and mental model — Rust equivalents for PyArrow capabilities |
| §1 | 1175 | Rust crate topology and dependency strategy |
| §2 | 2226 | Installation, Cargo features, and deployment profiles |
| §3 | 3565 | Arrow data model: types, fields, schemas, metadata |
| §4 | 4885 | Buffers, memory ownership, nullability, and zero-copy |
| §5 | 6042 | Arrays, builders, scalars, and chunked data |
| §6 | 7428 | RecordBatch, table-like workflows, and streaming readers |
| §7 | 8671 | Compute kernels: Arrow-level operations |
| §8 | 9824 | Compute expressions and query-style operations |
| §9 | 10995 | CSV, JSON, and line-delimited ingestion |
| §10 | 12209 | IPC, Arrow files, streams, and Feather |
| §11 | 13247 | Parquet core: reading, writing, metadata, and schema mapping |
| §12 | 14399 | Advanced Parquet: async, cloud, predicate pushdown, CDC, bloom filters |
| §13 | 15579 | Dataset-equivalent workflows |
| §14 | 16880 | Filesystem and object-store layer |
| §15 | 17977 | DataFusion SQL and DataFrame API |
| §16 | 19123 | Query planning, optimization, and execution internals |
| §17 | 20390 | Joins, aggregations, windows, and grouped operations |
| §18 | 21617 | UDFs, UDAFs, UDTFs, and extension points |
| §19 | 23141 | Flight RPC and Flight SQL |
| §20 | 24402 | ADBC and database connectivity |
| §21 | 25528 | Python interop: PyArrow, PyCapsule, PyO3, and zero-copy extension modules |
| §22 | 26816 | DataFrame ergonomics: Polars versus DataFusion versus raw Arrow |
| §23 | 27767 | ORC and Avro support |
| §24 | 28756 | CUDA, GPU, device arrays, and DLPack |
| §25 | 29575 | Substrait and portable query plans |
| §26 | 30628 | Extension types and custom logical types |
| §27 | 31799 | Performance engineering and benchmarking |
| §28 | 32926 | Error handling, testing, and compatibility matrix |

### §1.3 `principles` — the design constitution

Re-derive with `rg -n '^# ' docs/library_ref/full_data_fabric_design_principles_v2.md`.

**Citation convention:** the h1 ordinal is **principle number + 1 by construction**
(`# 15. Principle 14 — …`). Always cite by principle number and title ("Principle 14"),
never by the h1 ordinal. Lines for Principles 1–25 are carried by the P-table at the end
of REFERENCE §2.

| Section | Line | Title |
|---|---:|---|
| title | 1 | Model-First, Contract-Driven, Provenance-Native Data Fabric |
| §1 (h2) | 3 | Purpose |
| Principles 1–25 | 21–1123 | see the P-table in REFERENCE §2 |
| §27 | 1124 | The overall architectural pattern |
| §28 | 1167 | Mandatory design questions for an LLM programming agent |
| §29 | 1195 | Anti-patterns agents should actively reject |
| §30 | 1279 | Compact agent design constitution |
| §31 | 1319 | Short form |

### §1.4 `align` — the design-principle alignment manual

Re-derive with
`rg -n '^#{1,2} ' docs/library_ref/datafusion55_arrow59_design_principle_alignment_manual_2026-08-24.md`.

Front matter and workflow:

| Section | Line | Contents |
|---|---:|---|
| §0 | 13 | Purpose and scope; **§0.1 (29) capability-status legend** — NATIVE MODEL / NATIVE ENFORCEMENT / EXTENSION CONTRACT / COMPOSITION PATTERN / APPLICATION OVERLAY / CAUTION; §0.2 (42) library authority boundaries; §0.3 (54) non-goals and explicit gaps |
| §1 | 72 | How an agent uses the manual; §1.1 (74) required input; **§1.2 (88) mandatory 12-step review flow** → Part IV artifact names; **§1.3 (107) stop conditions** |
| §2 | 123 | §2.1 (125) preferred compilation chain; **§2.2 (151) authority and derivation table**; **§2.3 (166) extension-selection hierarchy** |

Part I (line 194): P1–P25, one section per principle, each with the same five members
(mechanisms · required utilization rules · application-owned overlay · required
evidence · reject-list) and a closing `**Primary utilization-pattern references:**` line
into Part II IDs. Per-P lines are in the REFERENCE §2 P-table.

Part II (line 1281): 14 feature families, 150 pattern IDs. Each row =
`ID | Feature(s) | Required leverage | Primary principles | Minimum evidence`.

| Family | Line | IDs | Theme |
|---|---:|---|---|
| MOD | 1287 | MOD-01–08 | semantic modeling and compilation |
| ARR | 1302 | ARR-01–10 | Arrow canonical data plane |
| SCH | 1319 | SCH-01–12 | schema, type, metadata, evolution |
| CAT | 1338 | CAT-01–10 | catalog, provider, table contract |
| EXP | 1355 | EXP-01–12 | expression and calculation |
| LOG | 1374 | LOG-01–10 | logical planning and optimization |
| PHY | 1391 | PHY-01–12 | physical planning and execution contract |
| SRC | 1410 | SRC-01–10 | source, file, Parquet, object store |
| RUN | 1427 | RUN-01–10 | session, runtime, state, cache, resources |
| INT | 1444 | INT-01–10 | interoperability and serialization |
| OBS | 1461 | OBS-01–12 | provenance, observability, reproducibility |
| GOV | 1480 | GOV-01–10 | governance, policy, capability truth |
| EXT | 1497 | EXT-01–10 | extension-level selection |
| TST | 1514 | TST-01–14 | contract-derived testing |

Part III (line 1535): requirement-to-feature decision flows, each ending in
`### Required selections` (literal pattern IDs) and `### Agent questions`.

| Flow | Line | Requirement class |
|---|---:|---|
| §4 | 1539 | Schema and data contract |
| §5 | 1580 | Calculation and expression |
| §6 | 1624 | Table, source, and provider |
| §7 | 1664 | Relational plan and query |
| §8 | 1703 | Physical execution and performance |
| §9 | 1742 | Interoperability |
| §10 | 1775 | Governance and policy |
| §11 | 1807 | Provenance, observability, and reproducibility |

Part IV (line 1847): required design artifacts — `SemanticRequirement` (1851),
`AuthorityMap` (1869), `RepresentationMap` (1879), `FeatureUtilizationPlan` (1899),
`ContractAndCapabilityMatrix` (1909), `LifecycleArtifactMap` (1917),
`ProvenanceClosureMap` (1929), `StateOwnershipMap` (1970),
`OptimizerVisibilityReview` (1979), `TestEvidenceMatrix` (1989),
`ExtensionDecisionRecord` (1996).

Part V (line 2020): §23 (2022) principle→pattern crosswalk; §24 (2052) feature-family→
principles→requirement-class crosswalk; §25 (2070) the `functional_building_block` YAML
schema for the next-stage catalogue.

Part VI (line 2100): review checklists §26–§34 (semantic/authority 2102 · Arrow fabric
2110 · schema contract 2119 · calculation/optimizer 2129 · provider/source 2138 ·
logical/physical planning 2148 · governance/state 2158 ·
provenance/observability 2167 · test evidence 2177).

Part VII (line 2189): 14-row anti-pattern → symptom → violation → prescribed-correction
table. Part VIII (2210): compact agent instruction block. Appendix A (2229): A.1 (2233)
19 DataFusion 55 capabilities, A.2 (2257) 14 Arrow 59 capabilities, A.3 (2276) the
exact-pin dependency invariant. Closing maxim at 2292.

---

## §2 — Binding matrix: utilization patterns → reference sections

**Contract:** the `align` Part II row remains authoritative for required leverage,
primary principles, and minimum evidence. This matrix adds only what the manual lacks —
the file and section where each pattern's API surface is documented. A pattern is never
implementable from this table alone: read its Part II row first, then the sections here.
A `—` cell means that document does not cover the pattern's surface; "overlay" means the
capability is application-owned (`align` §0.1 APPLICATION OVERLAY) and the cited
sections cover only the native artifacts it composes.

Bindings were verified against chapter content, not just titles; symbols named in a
theme were located in the cited chapters by search at authoring time.

### MOD — semantic modeling and compilation

| IDs | Theme | df | arrow |
|---|---|---|---|
| MOD-01 | typed models over Arrow schema objects | `df` §4 | `arrow` §3 |
| MOD-02–03 | one owned compiler into `Expr` / `LogicalPlan` | `df` §5, §10–§11 · `df-plan` §42–§43, §45 | — |
| MOD-04–05 | authority map; explicit validate/bind/compile phases | `df-plan` §41; authority map is overlay (`align` §2.2) | — |
| MOD-06 | versioned canonical fingerprints | `df-plan` §56; digest bytes → `canonicalization-lib-ref` | — |
| MOD-07 | tree-derived inventories from `Expr`/`LogicalPlan` | `df` §19 · `df-plan` §43 | — |
| MOD-08 | deterministic naming and stable aliases | `df-schema` S4 | — |

### ARR — Arrow canonical data plane

| IDs | Theme | df | arrow |
|---|---|---|---|
| ARR-01–03 | schema/arrays/`RecordBatch` as the canonical contract | `df` §4 | `arrow` §3, §5–§6 |
| ARR-04–05 | buffers, zero-copy, validity/null semantics | `df-calc` C7 (null semantics in calculations) | `arrow` §4 |
| ARR-06–07 | compute kernels; dictionary/view/nested encodings | `df` §7 | `arrow` §7–§8; encodings in §3, §5 |
| ARR-08 | streams/readers, bounded incremental consumption | `df` §21 | `arrow` §6 |
| ARR-09 | builders, capacity planning, controlled unchecked paths | — | `arrow` §5; `force_validate` feature in §2 |
| ARR-10 | explicit conversion-boundary inventory | — | `arrow` §10, §21 |

### SCH — schema, type, metadata, evolution

| IDs | Theme | df | arrow |
|---|---|---|---|
| SCH-01–02 | `SchemaContract` → Arrow `Schema` → `DFSchema` | `df-schema` S1–S2 · `df-plan` §44 | `arrow` §3 |
| SCH-03 | provider schema snapshot stability | `df` §18 · `df-schema` S10 | — |
| SCH-04 | compatibility modes: exact/contains/merge/equality | `df-schema` S5–S6 | `arrow` §3 |
| SCH-05–06 | metadata semantic classes; extension types | `df-schema` S7 | `arrow` §3, §26 |
| SCH-07 | `Constraints` and `FunctionalDependencies` | `df-schema` S8 · `df-plan` §47 | — |
| SCH-08 | file/partition/virtual/table schema distinction | `df-schema` S12 | — |
| SCH-09 | runtime plan/stream/batch schema validation | `df-schema` S11, S14 | `arrow` §6 |
| SCH-10 | schema fingerprints and versions (overlay) | `df-plan` §56 | — |
| SCH-11 | physical schema adaptation (`PhysicalExprAdapterFactory`) | `df-schema` S6, S10 · `df` §40A | — |
| SCH-12 | IPC/Parquet/FFI schema round trips | `df-schema` S14 | `arrow` §10–§12, §21 |

### CAT — catalog, provider, table contract

| IDs | Theme | df | arrow |
|---|---|---|---|
| CAT-01–02 | catalog→schema→table hierarchy; registration | `df` §17 · `df-schema` S9 | — |
| CAT-03–04 | `TableProvider` contract; backend snapshot/mapping | `df` §18 · `df-schema` S10 | — |
| CAT-05 | `supports_filters_pushdown` truthfulness | `df` §18 · `df-plan` §51 | — |
| CAT-06 | `scan_with_args` / `ScanArgs` | `df` §18, §40A · `df-schema` S10 · `df-plan` §47 | — |
| CAT-07 | `StatisticsRequest` and provider statistics | `df` §40A · `df-plan` §47, §51 | — |
| CAT-08 | provider DML methods and write posture | `df` §9, §18 | — |
| CAT-09 | `TableFunction` / UDTF | `df` §24 | `arrow` §18 |
| CAT-10 | backend-specific isolation inside the provider | `df` §18 | — |

### EXP — expression and calculation

| IDs | Theme | df | arrow |
|---|---|---|---|
| EXP-01–02 | built-ins and transparent expression builders first | `df` §11–§12 · `df-calc` C9 | `arrow` §8 |
| EXP-03 | `ScalarUDFImpl` for true custom kernels | `df` §24 · `df-calc` C1, C8 | `arrow` §18 |
| EXP-04–05 | signature/coercion; volatility, `is_strict`, nullability | `df-calc` C5–C7 · `df` §40A | — |
| EXP-06–07 | UDF optimizer hooks; conditional/short-circuit declaration | `df` §24 · `df-calc` C1–C2, C7 | — |
| EXP-08 | higher-order functions and lambdas | `df-calc` C1, C3 · `df` §40A · `df-plan` §45 | — |
| EXP-09 | aggregate state, `GroupsAccumulator`, `convert_to_state` | `df-calc` C13 · `df` §24, §40A | — |
| EXP-10 | window UDF / `PartitionEvaluator` | `df` §24 · `df-calc` C2, C4 | — |
| EXP-11 | output field metadata and deterministic aliasing | `df-calc` C6 · `df-schema` S4 | — |
| EXP-12 | async scalar UDFs | `df-calc` C12 | — |

### LOG — logical planning and optimization

| IDs | Theme | df | arrow |
|---|---|---|---|
| LOG-01 | all entry paths converge on `LogicalPlan` | `df` §19 · `df-plan` §41–§43 | — |
| LOG-02–03 | `PlannerContext`; `ContextProvider` | `df-plan` §42 | — |
| LOG-04 | analyzer and logical optimizer | `df` §22 · `df-plan` §48 | — |
| LOG-05 | `TreeNode` traversal and transforms | `df` §19 · `df-plan` §43 | — |
| LOG-06 | plan metadata: schema, constraints, FDs, statistics | `df-plan` §44, §47 | — |
| LOG-07 | logical-plan policy validation | `df-plan` §46 | — |
| LOG-08 | `LogicalPlan::Extension` | `df` §26 · `df-plan` §49 | — |
| LOG-09 | proto/Substrait logical artifacts | `df` §36 | `arrow` §25 |
| LOG-10 | DML logical models including merge | `df` §9, §40A | — |

### PHY — physical planning and execution contract

| IDs | Theme | df | arrow |
|---|---|---|---|
| PHY-01 | `PhysicalPlanningContext` | `df` §3, §40A · `df-plan` §49 | — |
| PHY-02–03 | `ExecutionPlan` contract; `PlanProperties` | `df` §20, §26 · `df-plan` §50 | `arrow` §16 |
| PHY-04–06 | `apply_expressions`; `replace_children`; invariants/state reset | `df` §40A, §26 · plan-invariant checking in §19 | — |
| PHY-07–08 | distribution and ordering requirements | `df-plan` §50 · `df` §40A | — |
| PHY-09 | bottom-up statistics propagation | `df-plan` §47 · `df` §40A | — |
| PHY-10 | dynamic filters and pruning builders | `df` §40A, §15, §22 · `df-plan` §51 | `arrow` §12 |
| PHY-11 | metrics, memory reservations, spill | `df` §28, §30, §40A · `df-plan` §54 | — |
| PHY-12 | physical-plan serialization hooks | `df` §36, §40A · `df-plan` §56 | — |

### SRC — source, file, Parquet, object store

| IDs | Theme | df | arrow |
|---|---|---|---|
| SRC-01 | `RuntimeEnv` object-store registry | `df` §16 | `arrow` §14 |
| SRC-02 | `FileSource` / `FileScanConfig` / `DataSourceExec` | `df` §14 · `df-plan` §51 | `arrow` §13 |
| SRC-03 | projection/filter/limit pushdown with residuals | `df` §18 · `df-plan` §51 | `arrow` §12 |
| SRC-04 | Parquet row-group/page/bloom/row-filter pruning | `df` §15 | `arrow` §11–§12 |
| SRC-05 | sort pushdown, Top-K, dynamic early stopping | `df` §22, §29 · `df-plan` §51, §53 | — |
| SRC-06 | `file_row_index()` and virtual columns | `df` §40A · `df-schema` S12 | — |
| SRC-07 | per-file schema evolution adapters | `df-schema` S6, S10 | — |
| SRC-08 | reader factories and metadata caches | `df` §15 | `arrow` §12 |
| SRC-09 | file-stream work stealing and output partitioning | `df` §21, §40A · `df-plan` §54 | — |
| SRC-10 | CSV/JSON/Avro ingress and sink boundaries | `df` §14 | `arrow` §9, §23 |

### RUN — session, runtime, state, cache, resources

| IDs | Theme | df | arrow |
|---|---|---|---|
| RUN-01–05 | scope taxonomy; `SessionContext`/`SessionState`/`RuntimeEnv`/`TaskContext` | `df` §3 | — |
| RUN-06–07 | `ExecutionProps` vs `PhysicalPlanningContext` scope split | `df` §3, §40A | — |
| RUN-08 | cache entries with dependency fingerprints | `df-plan` §56 | — |
| RUN-09 | memory reservations, spill, bounded channels | `df` §28 · `df-plan` §54 | — |
| RUN-10 | environment snapshot for reproducibility | `df-plan` §55 | — |

### INT — interoperability and serialization

| IDs | Theme | df | arrow |
|---|---|---|---|
| INT-01 | Arrow IPC files and streams | — | `arrow` §10 |
| INT-02 | Parquet as durable interchange | `df` §15 | `arrow` §11–§12 |
| INT-03–04 | C Data / C Stream / PyCapsule / `pyo3-arrow` | — | `arrow` §21 |
| INT-05 | Flight and Flight SQL | — | `arrow` §19 |
| INT-06 | DataFusion native proto plan transport | `df` §36 | — |
| INT-07 | Substrait cross-engine plans | `df` §36 | `arrow` §25 |
| INT-08 | `RecordBatchReader` / `SendableRecordBatchStream` boundaries | `df` §21 | `arrow` §6 |
| INT-09 | extension-type negotiation and fallback | `df-schema` S7 | `arrow` §3, §26 |
| INT-10 | pinned compatibility matrix at every boundary | `df` §1, §34 + gates V1–V6 | `arrow` §2, §28 |

### OBS — provenance, observability, reproducibility

| IDs | Theme | df | arrow |
|---|---|---|---|
| OBS-01–03 | planning/execution artifact bundle; plan capture | `df-plan` §55 · `df` §19–§20 | — |
| OBS-04 | `EXPLAIN` / `EXPLAIN ANALYZE` | `df` §30 | `arrow` §16 |
| OBS-05 | metadata provenance references | `df-schema` S7 | `arrow` §3 |
| OBS-06 | execution identity via `TaskContext`/tracing | `df` §3, §30 | — |
| OBS-07 | dependency environment record | `df-plan` §55 · `df` §34 | `arrow` §2 |
| OBS-08 | source/provider snapshot identity | `df` §18; Delta versions → `deltalake-rust-ref` | — |
| OBS-09–10 | semantic fingerprints; reproducibility status (overlay) | `df-plan` §55–§56 | — |
| OBS-11 | operator/provider metrics semantics | `df` §30 · `df-plan` §54 | — |
| OBS-12 | provenance closure traversal (overlay) | native artifacts from `df-plan` §55 | — |

### GOV — governance, policy, capability truth

| IDs | Theme | df | arrow |
|---|---|---|---|
| GOV-01–02 | namespace/table visibility; row/column enforcement | `df` §17–§18, §39 · `df-schema` S15 | — |
| GOV-03 | logical-plan policy validation | `df-plan` §46 | — |
| GOV-04 | function registry allowlist | `df-calc` C3 | — |
| GOV-05 | scoped credentials and resources | `df` §16, §27–§28 · `df-schema` S15 | `arrow` §14 |
| GOV-06–07 | capability truth table; metadata class registry (overlay, `align` §0.1) | `df` §18 · `df-schema` S7 | — |
| GOV-08 | policy version/fingerprint in dependencies | `df-plan` §56 | — |
| GOV-09 | write/DML authority at the provider boundary | `df` §9, §39 | — |
| GOV-10 | audit-ready decision records | `df` §30, §39 | — |

### EXT — extension-level selection (the ladder, `align` §2.3)

| IDs | Theme | df | arrow |
|---|---|---|---|
| EXT-01 | built-in Arrow kernel | — | `arrow` §7 |
| EXT-02–03 | built-in expression; reusable builder library | `df` §11–§12 · `df-calc` C9 | `arrow` §8 |
| EXT-04–05 | scalar/async/higher-order UDF; UDAF/UDWF/UDTF | `df` §24 · `df-calc` C1, C8, C12–C13 | `arrow` §18 |
| EXT-06 | `TableProvider` / `FileSource` / `ObjectStore` | `df` §14, §16, §18 | `arrow` §14 |
| EXT-07 | SQL expression/type/relation planner hooks | `df` §25 | — |
| EXT-08 | `LogicalPlan::Extension` + `ExtensionPlanner` | `df` §26 · `df-plan` §49 | — |
| EXT-09 | custom `ExecutionPlan` / `PhysicalExpr` | `df` §20, §26 | — |
| EXT-10 | custom `QueryPlanner` / `PhysicalPlanner` | `df` §26 · `df-plan` §41, §49 | — |

### TST — contract-derived testing

| IDs | Theme | df | arrow |
|---|---|---|---|
| TST-01 | schema contract tests | `df-schema` S14 · `df` §32 | `arrow` §3 |
| TST-02–04 | provider/pushdown/schema-adaptation harnesses | `df` §18, §32 · `df-schema` S10, S14 | — |
| TST-05 | UDF semantic tests | `df` §32 · `df-calc` C2, C5–C7 | — |
| TST-06 | optimizer equivalence tests | `df` §22, §32 | — |
| TST-07 | physical property/invariant tests | `df` §26, §32 · `df-plan` §50 | — |
| TST-08 | serialization round-trip tests | `df` §32, §36 | — |
| TST-09 | Arrow protocol interoperability tests | — | `arrow` §28; fixtures across §10–§12, §21 |
| TST-10 | resource/state tests: spill, cancellation, reset | `df` §28, §32 | — |
| TST-11 | plan/metric semantic snapshots | `df` §30, §32 | — |
| TST-12 | governance bypass tests | `df` §32, §39 | — |
| TST-13 | fuzz and malformed-input tests | `df` §32 | `arrow` §28; `force_validate` in §2 |
| TST-14 | version/dependency migration matrix | `df` §1, §34 + gates V1–V6 | `arrow` §2, §28 |

### The P1–P25 binding table

Principle titles are shared verbatim between `principles` and `align` Part I. Cite the
principle number, never the `principles` h1 ordinal (which is number + 1). To go from a
principle to its patterns, use `align` §23 (line 2022) — do not reconstruct that mapping
here; then resolve pattern IDs through the family blocks above.

| P | Title | `principles` line (h1 ordinal) | `align` line |
|---|---|---:|---:|
| P1 | Model semantics before implementing behavior | 21 (§2) | 196 |
| P2 | Make models executable, not merely descriptive | 93 (§3) | 240 |
| P3 | One authoritative owner for every concept | 134 (§4) | 283 |
| P4 | Use explicit conceptual hierarchies to encode shared guarantees and legal variation | 183 (§5) | 326 |
| P5 | Encode variability behind contracts, not throughout consumers | 244 (§6) | 368 |
| P6 | Separate semantic meaning from execution strategy | 280 (§7) | 411 |
| P7 | Build a shared canonical data fabric | 336 (§8) | 453 |
| P8 | Treat the common representation as infrastructure | 386 (§9) | 495 |
| P9 | Make provenance intrinsic to every meaningful transformation | 416 (§10) | 537 |
| P10 | Seek provenance closure | 472 (§11) | 581 |
| P11 | Prefer immutable snapshots and explicit state transitions | 508 (§12) | 623 |
| P12 | Schemas are executable contracts, not documentation | 549 (§13) | 664 |
| P13 | Put governance at the authoritative boundary | 602 (§14) | 713 |
| P14 | Prefer the highest-level extension that preserves the semantics | 637 (§15) | 756 |
| P15 | Preserve optimizer visibility | 681 (§16) | 800 |
| P16 | Treat lifecycle phases as first-class architecture | 715 (§17) | 845 |
| P17 | Make intermediate artifacts inspectable and reproducible | 778 (§18) | 890 |
| P18 | Fingerprint anything whose identity matters | 806 (§19) | 932 |
| P19 | Make reproducibility a normal operating mode | 846 (§20) | 974 |
| P20 | Be conservative about claimed capabilities | 881 (§21) | 1017 |
| P21 | Separate enforced semantics from advisory metadata | 913 (§22) | 1061 |
| P22 | Use protocols and canonical boundaries for interoperability | 960 (§23) | 1103 |
| P23 | Keep state ownership local and explicit | 985 (§24) | 1147 |
| P24 | Make observability semantic, not merely operational | 1041 (§25) | 1192 |
| P25 | Make testing derive from contracts and invariants | 1076 (§26) | 1234 |

---

## §3 — Decision trees

### 3a. Which document answers this?

```text
Is the question "how should this be designed / which principle applies"?
  -> principles (the constitution) and align Part I     (align P-table above)
Is the question "which features should this outcome use"?
  -> align Part III flow, then Part II rows, then REFERENCE §2 bindings
Is the question a DataFusion API/behavior question?
  -> df / df-schema / df-plan / df-calc                  (index §1.1)
Is the question an Arrow-crate, encoding, or protocol question?
  -> arrow                                               (index §1.2)

Overlap topics — who wins:
  Parquet     -> df §15 for DataFusion-integrated reads/config and pruning behavior;
                 arrow §11-§12 for reader/writer crate mechanics, async, bloom, CDC
  UDFs        -> df §24 + df-calc for the DataFusion function system;
                 arrow §18 for the Arrow-side extension-point view
  IPC/streams -> arrow §10 for framing/compression/Feather;
                 df §21 for streaming execution semantics
  object store-> df §16 for DataFusion registration/session wiring;
                 arrow §14 for the object_store crate itself
  Every interop protocol (Flight, ADBC, C Data/PyCapsule, Substrait) -> arrow;
  df §36 only for DataFusion's own proto plan serialization.
```

### 3b. Outcome → flow classifier (`align` Part III)

```text
Does the outcome define or change what a table/field MEANS?        -> align §4  (schema)
Does it compute a new value, predicate, or aggregate?              -> align §5  (calculation)
Does it read from / write to a new place or source shape?          -> align §6  (provider/source)
Does it change what queries can express or how they are compiled?  -> align §7  (plan/query)
Does it change how execution runs: speed, memory, partitioning?    -> align §8  (physical)
Does it cross a process/language/engine/file boundary?             -> align §9  (interop)
Does it decide who may see or do something?                        -> align §10 (governance)
Does it explain, reproduce, or audit what happened?                -> align §11 (provenance)

Most real outcomes hit 2-4 flows; run each one and union the
Required-selections IDs. Then resolve IDs through REFERENCE §2.
```

### 3c. Extension ladder (`align` §2.3), bound to sections

```text
Arrow built-in kernel                      arrow §7          EXT-01
  -> DataFusion built-in SQL/Expr          df §11-§12        EXT-02
  -> application expression builder        df-calc C9        EXT-03
  -> Scalar/Async/HigherOrder UDF          df §24 · C1/C8/C12  EXT-04
  -> AggregateUDF/WindowUDF/UDTF/provider  df §24 · C13 · df §18  EXT-05/-06
  -> SQL planner hooks                     df §25            EXT-07
  -> LogicalPlan::Extension                df §26 · df-plan §49  EXT-08
  -> custom ExecutionPlan/PhysicalExpr     df §20, §26       EXT-09
  -> custom QueryPlanner                   df §26 · df-plan §41, §49  EXT-10

Take the FIRST level that fully preserves the required semantics
(principles Principle 14; align §2.3). Each step down must be justified
in an ExtensionDecisionRecord (align Part IV).
```

### 3d. Capability-status legend → required evidence (`align` §0.1)

```text
NATIVE MODEL        -> use the native object; evidence = boundary type/schema tests
NATIVE ENFORCEMENT  -> rely only inside documented scope; evidence = scope-edge tests
EXTENSION CONTRACT  -> you own the implementation; evidence = full contract suite
                       (the pattern's Minimum-evidence column, align Part II)
COMPOSITION PATTERN -> keep the prescribed boundaries; evidence = seam tests proving
                       the composition was not collapsed into procedural logic
APPLICATION OVERLAY -> build the owned model/registry/policy layer; evidence = the
                       overlay's own tests plus links back to native artifacts
CAUTION             -> treat as advisory/version-coupled; evidence = pinned-version
                       tests and recorded uncertainty
```

### 3e. Upgrade-gate walk (Part III, V1–V6)

```text
V1 compile-time trait gate   -> traits/methods compile against the exact pins
V2 schema and source gate    -> schema, scan, and source behavior verified at runtime
V3 calculation gate          -> UDF/UDAF semantics (incl. convert_to_state) verified
V4 plan/reproducibility gate -> plan shape, fingerprints, and artifacts verified
V6 corrections log           -> what the source-verification pass changed; read it
                                BEFORE trusting older prose (sits between V4 and V5)
V5 final source-of-truth rule-> exact-source probe beats any illustrative prose
```

---

## §4 — Operating rules

1. **Seek by line, cite by section.** Line numbers live only in REFERENCE §1/§2 tables;
   citations everywhere else use `alias §N` / `S/C/V` identifiers.
2. **Use anchored heading patterns only** (the re-derive commands in §1). `df` has ~250
   spurious h1s and fenced `#`-comments; `arrow` is h2-rooted and has one stray
   in-catalog h1 at line 427.
3. **Cite `principles` by principle number and title, never by h1 ordinal** — the
   ordinal is principle + 1 by construction.
4. **Never copy the predecessor-release version strings** from `arrow`'s migration
   ledger or its six legacy-comparison subsection titles into any tracked file.
   `scripts/data_fabric_old_authority_check.sh` (run by
   `just data-fabric-old-authority-check`) greps all of `.claude/skills` for them.
   Paraphrase, as this file does.
5. **`df` §40A and the V1–V6 gates outrank imported prose.** Both API documents carry
   imported deep dives with stale illustrative version strings; the source-verified
   reconciliation and the gates are the authority when they disagree.
6. **A pattern ID is never implementable without its `align` Part II row.** The row
   carries required leverage, principles, and minimum evidence; REFERENCE §2 carries
   only the reading locations.
7. **Honor `align` §1.3 stop conditions.** If any stop condition holds, the work stays
   at design; do not proceed to code and "fix it later".
8. **Untracked-document drift rule.** If a line/title probe mismatches any §1 table,
   re-derive that document's spine with the embedded command before trusting anything
   else cited against it; report the drift.
9. **Absence degradation.** If a routed document is missing, say so explicitly, continue
   with the documents that exist, and never reconstruct pattern IDs, principle numbers,
   or section claims from memory.
10. **`just lib-outline <path> --view names` before reading a large chapter** — chapters
    in both API documents run 800–2,000 lines; zoom to the subsection first.
11. **Version pins come from FAB §2.1 and the session context**, never from examples in
    any reference (`df` §0/§1 and `arrow`'s version matrix restate them but are not the
    pin authority).
12. **The `arrow` body ends at §28.** Chapters 29–30 exist only in its topic map; a
    citation to them is invalid.
13. **Overlap topics resolve by tree 3a**, not by whichever document was open.
14. **Delta anything** (snapshots, exact versions, writes, transactions, checkpoints,
    optimize, vacuum) routes to `deltalake-rust-ref`; provider-side integration points
    stay here (`df` §17–§18).
15. **Evidence over restatement.** When this skill's guidance and a cited section seem
    to disagree, open the section; if the disagreement survives, the document wins and
    this skill needs a fix — flag it.

---

## §5 — Project context: CodeFabric

**Source map** (where these patterns land in this repository):

| Path | Role | Dominant families |
|---|---|---|
| `src/fabric/snapshot_catalog.rs` | identity provider wrapper, exact provider caching | CAT, SCH, RUN |
| `src/fabric/overlay.rs` | effective overlay provider, explicit statistics disposition | CAT-05–07, PHY-09, GOV |
| `src/fabric/serving.rs` | query evidence, pruning, resources, cancellation, plan diagnostics | LOG, PHY, OBS, SRC |
| `src/fabric/mutation.rs`, `src/fabric/publication.rs` | durable checksum consumers | OBS, MOD-06, SCH-10 |
| `rustc-extractor/src/wrapper.rs` | Arrow IPC producer boundary | INT-01, ARR, SCH-12 |
| `scripts/stable_graph_check.sh` | exact family/source/feature enforcement | INT-10, TST-14 |

**Constraints** (repository invariants these documents serve):

- One Arrow/Parquet/DataFusion public type universe across the stable root and the
  extractor IPC boundary (`INT-10`, `TST-14`; enforced by `stable_graph_check.sh`).
- `RowConverter` output participates in durable application checksums; never refresh a
  checksum merely because a library version changed (`MOD-06`, `OBS-09`; gate V3).
- A query pins one immutable snapshot and exact Delta versions; plan text and metrics
  are diagnostics, not semantic identity (`OBS-08`, Principle 11; `deltalake-rust-ref`).
- The application-owned `TableProvider` wrappers must make DataFusion 55 structured scan
  and statistics behavior explicit (`CAT-06`, `CAT-07`; `df` §40A).
- Python/FastMCP remains presentation-only — no Arrow/DataFusion processing in the
  adapter (`AGENTS.md` invariant 7).

**Spec anchors.** Cite specs as `FAB §N` etc. by section number, never line number.
`docs/spec_index/library-routing.md` maps spec sections to chapters here (e.g.
`FAB §77` → `arrow` §7–§8; `FAB §78` → `df` §24 + `df-calc` C1; `FAB §91` → `df`
§17–§18). The manual's Part IV artifacts are what fabric design documents should carry
for data-fabric work.

**Boundary discipline.** Fact providers → `code-facts-lib-ref`; Delta storage →
`deltalake-rust-ref`; graph analytics → `petgraph-ref`; canonical JSON/digests →
`canonicalization-lib-ref`; the adapter wire → `grpcio-orjson-protobuf-ref` and
`fastmcp-pydantic-ref`.
