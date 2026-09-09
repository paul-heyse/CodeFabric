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
| `principles` | `full_data_fabric_design_principles_v2.md` | current P1–P36 guidance | revised |
| `align` | `datafusion55_arrow59_design_principle_alignment_manual_2026-08-24.md` | §0–§2 · P1–P25 · pattern families · flows §4–§11 · §12–§25 · App. A | revised |

`REFERENCE §N` and `SKILL §…` refer to this skill's own files, never to a document.

## Table of contents

- §1 — Per-document section indexes (`df` in §1.1, `arrow` in §1.2, `principles` in §1.3, `align` in §1.4)
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

## Current principles and optional alignment

The current P1–P36 guidance is in `docs/library_ref/full_data_fabric_design_principles_v2.md`. Alignment manuals are optional capability references. Their former workflow/artifact matrices are retired; use current headings with `just lib-outline`. No principle crosswalk or exhaustive API survey is required before implementation. Preserve native API details from the indexes above and verify current Cargo source selections when using them.
