---
name: datafusion-pyarrow-rust-ref
description: "Reference navigator for the Rust DataFusion + Arrow stack. SKILL.md maps five deep-dives at `docs/library_ref/`: `datafusion_rust.md` (engine §0-§40), `datafusion_planning_rust.md` (planning §41-§60), `datafusion_schemas_rust.md` (schemas S1-S15), `datafusion_calculations_rust.md` (UDF/UDAF C1-C26), `pyarrow_rust.md` (Arrow crate stack §0-§30); REFERENCE.md (same folder) holds per-doc section indexes, the cross-document overlap matrix, decision trees, and operating rules. Use when Rust touches `use datafusion::`/`use arrow::`/`use arrow_*::`/`use parquet::`/`use object_store::`/`use arrow_flight::`/`pyo3_arrow`, edits `Cargo.toml` for those crates, or authors `SessionContext`/`RecordBatch`/`DataFrame`/`Expr`/`LogicalPlan`/`ExecutionPlan`/`TableProvider`/`ScalarUDFImpl`/`Schema`/`Field`/`DataType`, Substrait/Flight SQL/ADBC, or any `__arrow_c_*` PyCapsule Rust↔Python boundary. Python-side DataFusion/PyArrow lives in `docs/library_ref/datafusion.md` and `pyarrow.md`, which no skill routes."
allowed-tools: Read, Grep, Glob, Bash
---

# DataFusion + Arrow Rust Reference Navigator

Routes five deep-dive references for the **Rust** DataFusion + Arrow stack. This SKILL.md is the **core map**: version anchors, the five-document topic table, reading strategy, where-to-look routing, and the key invariants. The companion **`REFERENCE.md`** (same folder) carries the per-document section indexes (every §/S/C section with line numbers), the cross-document overlap matrix, the decision trees (crate choice · planning surface · UDF kind · read/write path · cross-runtime · slow-query diagnosis · schema governance · Cargo setup), the full 22 operating rules, and the CodeFabric project context. Reach for REFERENCE.md once you know *which* document you need and want its section map; cross-references back here are written `SKILL §...`.

**Out of scope** (covered elsewhere): Python `datafusion`/`pyarrow` packages — `docs/library_ref/datafusion.md` and `pyarrow.md` cover them, but **no skill routes those two and the design suite gives them no demand** (`SRV §18` states the FastMCP adapter requires neither Arrow nor DataFusion); read them directly if the need arises. Rust `deltalake` / deltalake-on-DataFusion → sibling skill **`deltalake-rust-ref`** (`docs/library_ref/deltalake_rust_1.0.0_9f922319_advanced_reference_2026-08-20.md`). `duckdb`/`ibis`/`polars-rs` ergonomic comparisons are catalog-only (pa-rust §22).

---

## Version anchors

* **DataFusion Rust 54.1.0** (released; declares a caret Arrow/Parquet requirement of `58.3.0`, satisfied by the `58.4.0` baseline; edition 2024, MSRV 1.88) — embeddable analytical query engine; SQL + DataFrame APIs over Arrow `RecordBatch`; logical/physical optimizer; custom planner hooks; async streaming. Every public entry point above `LogicalPlan` is `async` (Tokio `rt-multi-thread`, `macros`; `futures` for stream combinators). A `datafusion_54vs53.md` migration document is referenced by this skill's ancestry but **does not exist in this repository**; per-topic DF-54 behaviour is covered inside the five documents themselves (REFERENCE.md §2, "DataFusion 54 additions").
* **Arrow Rust crate family** — top-level `arrow` re-export + narrow subcrates: `arrow-array`, `-buffer`, `-schema`, `-data`, `-cast`, `-select`, `-ord`, `-string`, `-arith`, `-csv`, `-json`, `-ipc`, `-flight`, `-avro`. Independent crates: `parquet`, `object_store`.
* **Python interop bridges** — `arrow_pyarrow` / `pyo3-arrow` move data Rust↔Python over the C Data / C Stream / PyCapsule protocol (zero-copy). This stack *talks to* PyArrow; it is **not** the Python `pyarrow` package.
* **Stack contract**: `SQL` → `sqlparser AST` → `SqlToRel` (bind/resolve/coerce) → `LogicalPlan` + `Expr` → `AnalyzerRules` → `OptimizerRules` → optimized `LogicalPlan` → `PhysicalPlanner` → `ExecutionPlan` + `PhysicalExpr` → `PhysicalOptimizerRules` → `execute(partition, TaskContext)` → `SendableRecordBatchStream`. DataFrame / `LogicalPlanBuilder` skip the SQL→AST hop and build `LogicalPlan` directly.

---

## The five reference documents

All live at `docs/library_ref/`. Each is **catalog-first** (a top block enumerates every subsection in 2-5 bullets) **then deep-dives**. The deep-dive H1 prefix differs per doc — **disambiguate it before grepping** (a grep for `# DataFusion Advanced — N)` returns 85 hits across four files; scope to one file at a time).

| Doc | Path (`docs/library_ref/`) | Lines | Deep-dive prefix | Scope (deep-dive range) |
|-----|------|------:|------------------|-------|
| **df-rust** | `datafusion_rust.md` | 43,381 | `# DataFusion Advanced — N) ` | **§0-§40** — full Rust DataFusion engine: install, sessions, Arrow data model, SQL, DataFrame, `Expr`, built-ins, sources, Parquet, object stores, catalogs, `TableProvider`, logical/physical plans, optimizer, joins, UDFs, custom SQL/operators, config, memory, perf, metrics, CLI, testing, errors, architecture, distributed, deploy, security. |
| **df-planning** | `datafusion_planning_rust.md` | 26,109 | `# DataFusion Advanced — N) ` | **§41-§60** (deep-dives §41-§56) — planning lifecycle: phase-boundary map, SQL binder internals, programmatic plan construction, qualifier governance, expression lifecycle, plan lint, planner metadata, optimizer cookbook, physical lowering, scan/join/streaming planning, plan artifacts + serialization/caching/fingerprints. |
| **df-schemas** | `datafusion_schemas_rust.md` | 25,543 | `# DataFusion Advanced — SN) ` | **S1-S15** — schema lifecycle: vocabulary, factories, inference vs explicit + drift, naming/qualifiers, type compatibility/coercion/equality, evolution + migration, extension types + metadata, constraints/FDs, catalog + `information_schema`, `TableProvider` adaptation, plan-schema propagation, nested/partition/virtual columns, view/CTAS stability, testing, security. |
| **df-calcs** | `datafusion_calculations_rust.md` | 22,583 | `# DataFusion Advanced — CN) ` | **C1-C26** (deep-dives C1-C13) — UDF/UDAF/UDWF/UDTF subsystem: placement decision tree, lifecycle, registry/discovery, package architecture, signature/overload, return type/nullability, null/NaN/inf semantics, vectorized Arrow patterns, conditional composition, nested returns, external-lib integration, async UDFs, UDAF state. Catalog-only C14-C26: window frames, table UDFs, optimizer interaction, authz, observability, testing, DSL, domain libs, perf, distributed. |
| **pa-rust** | `pyarrow_rust.md` | 34,247 | `# N) ` *(no "DataFusion Advanced" prefix)* | **§0-§30** (deep-dives §0-§28) — Arrow crate stack as the PyArrow-feature→Rust-crate map: data model, buffers/zero-copy, arrays/builders/scalars, RecordBatch/streaming, compute kernels, CSV/JSON/IPC/Parquet IO, dataset/object_store, DataFusion bridge, joins/aggs/windows, UDFs, Flight RPC/SQL, ADBC, Python interop/PyCapsule, Polars-vs-DataFusion, ORC/Avro, CUDA/DLPack, Substrait, extension types, perf, errors/testing. |

**Reading strategy.** Find the section in REFERENCE.md's per-doc index (SKILL §indexes there), then `Read(offset, limit)`. Deep-dives run 600-2,400 lines with `## N.M` subsections; each closes with an anti-pattern inventory + agent checklist — **load the closing 100-200 lines before drafting code**. The catalog block at the top of each doc is itself a usable map. Catalog-only sections (df-planning §57-§60, df-calcs C14-C26, pa-rust §29-§30) have no deep-dive by design — read the catalog for *what* exists, then derive from the adjacent deep-dive.

---

## Where do I look?

| Question | Doc |
|---|---|
| What *is* this Arrow value — buffers, arrays, builders, scalars, compute kernels, raw IO (CSV/JSON/IPC/Parquet), object_store | **pa-rust** |
| How a query / plan / `Expr` is built, optimized, executed | **df-rust** (API surface) + **df-planning** (lifecycle, plan-time decisions) |
| What a schema means — `DFSchema` vs Arrow `Schema`, coercion, evolution, constraints, governance | **df-schemas** |
| How a custom calculation works — UDF/UDAF/UDWF/UDTF, vectorized bodies, external math (SciPy/SymPy/native) | **df-calcs** |
| SQL grammar, DDL/DML, built-in functions, sessions, config, file sources, joins, `TableProvider`, memory/spill | **df-rust** |
| Flight RPC/SQL, ADBC, Python interop / PyCapsule, Substrait, CUDA/DLPack | **pa-rust** |
| What changed 53→54 — breaking APIs, semantic shifts, new-feature adoption, upgrade workflow | **No migration document exists here** — `datafusion_54vs53.md` is absent. Per-topic DF-54 deep-dives live in the five docs; see REFERENCE.md §2 "DataFusion 54 additions" |

For deeper routing — the full cross-document overlap matrix (which doc is *authoritative* per topic) and the eight decision trees (crate · planning surface · UDF kind · column type · read path · write path · plan layer · slow-query · schema governance · Cargo project) — see **`REFERENCE.md`**.

---

## Key invariants

The seven that prevent the most errors; the full set of **22 operating rules** is in `REFERENCE.md`.

1. **Rust DataFusion is a Rust crate; the PyArrow side is the C Data / PyCapsule boundary**, not the Python interpreter. Never round-trip data via pickle/JSON/pandas when a `__arrow_c_stream__` / `__arrow_c_array__` path exists. (pa-rust §21; df-rust §37.4)
2. **DataFrame is lazy; `.collect()` / `.execute_stream()` / `show()` / `write_*()` are terminal.** Transforms return a new lazy `DataFrame`; terminals materialize. Bound execution explicitly; never `.collect()` an unbounded source. (df-rust §0.6, §10, §21)
3. **Arrow containers (`Arc<Schema>`, `Arc<dyn Array>`, `Arc<RecordBatch>`) are reference-counted and effectively immutable** — clone shares cheaply (Arc bump); mutation is *replacement* via builders. **Nulls are validity bitmaps, never sentinels** (`-1`/`NaN`/`""`); UDFs propagate nulls per declared `Volatility`/null-policy. (pa-rust §3-§5; df-calcs C7)
4. **Rust `Expr` has no operator overloading** — write `col("a").gt(lit(0)).and(col("b").lt(lit(10)))`, not `>` / `&`. Same for `eq`/`gt_eq`/`lt`/`or`/`is_null`/`between`. (df-rust §11)
5. **Match the Arrow major version across every crate that touches Arrow types** (`datafusion`, `parquet`, `arrow-flight`, your code) — re-export `datafusion::arrow` when unsure; a `RecordBatch` from `arrow 58.x` ≠ one from `57.x`. `#[tokio::main(flavor = "multi_thread")]` is the canonical entry point. (df-rust §1, §34)
6. **`DFSchema` (plan-side: Arrow schema + relation qualifiers + ambiguity rules) ≠ Arrow `Schema` (runtime `RecordBatch`, Arrow only).** They convert both ways, but the qualifier is lost crossing one direction — the most common confusion. (df-rust §4; df-schemas S1)
7. **Schema is a contract artifact, not a derived value.** Fingerprint with `sha256(schema.serialize())`; declare at source registration; treat `Field.metadata` as governance/provenance, never application state. Plan-hash snapshots (unoptimized + optimized `LogicalPlan`) before any optimizer-affecting change. (df-schemas S1/S7/S14; df-planning §55-§56)

---

## Project context: CodeFabric

**Pre-implementation.** There is no DataFusion or Arrow code yet — one Cargo package, one
library crate, and a seed whose only job was to prove the toolchain. So these five docs are read
*against the specifications*, not against existing code, and the mapping that matters is
**spec section → doc**, not crate → doc.

The planned workspace baseline is fixed by the data-fabric specification, not by a current
`Cargo.toml`: `FAB §2.1 Canonical workspace baseline` pins `datafusion = "=54.1.0"`, the `arrow`
family and `parquet` at `=58.4.0`, `object_store = "=0.13.2"`, `edition = "2024"` and
`rust-version = "1.94.1"` under Cargo resolver `3`. `FAB §2.2` states the alignment invariant those pins serve — one Arrow
major/minor family, one matching Parquet family, one DataFusion family, one `object_store`
family, one pinned delta-rs revision — and requires CI to reject duplicates that cross public
type boundaries. Read the pins from the spec; do not assume a lockfile.

**Spec section → reference-document map.** `docs/spec_index/library-routing.md` §3 is the full
table; the load-bearing rows:

| Spec section | Read |
|---|---|
| `FAB §7` canonical physical types · `§10`-`§11` schema metadata and registry | **pa-rust** §3 · **df-schemas** S1-S15 |
| `FAB §63`-`§66` provider-observation to Arrow, batch size, builders, validation | **pa-rust** §5/§6/§10 · **df-rust** §4 |
| `FAB §77` Arrow kernel catalog | **pa-rust** §7/§8 |
| `FAB §78`-`§79` scalar and aggregate UDFs · `§79A` derivation registry | **df-rust** §24 · **df-calcs** C1-C3 (esp. C1.7 placement matrix, C1.12 manifest) |
| `FAB §81`-`§82` custom logical operators, custom physical graph representation | **df-rust** §19/§26 · **pa-rust** §4 — CSR over Arrow buffers, **not** petgraph |
| `FAB §83`-`§90` reachability, SCC, dominators, dataflow, summaries, execution requirements | **df-rust** §20/§21/§28 · **df-planning** §41-§56 |
| `FAB §91`-`§94` snapshot-pinned overlay catalog, serving views, table functions, planning policy | **df-rust** §17/§18/§22/§24 · **df-planning** |
| `FAB §95`-`§98` partitioning, Z-order, Parquet writer, runtime policy | **pa-rust** §11/§12 · **df-rust** §27/§28 |
| `QRY AC-G-46` typed `PlanSpec` · `AC-G-52` cost model · `AC-G-57` plan cache | **df-rust** §19/§22/§28 · **df-planning** |

**Two boundaries the specs draw that this stack must not cross.** `FAB §82` puts the physical
graph representation in Arrow CSR buffers and keeps petgraph on the fact-generation side
(`GEN §52`) — do not carry a petgraph type across. `SRV §18` states the FastMCP adapter does not
require Arrow or DataFusion at all; the Python side is Pydantic and orjson only, so nothing here
belongs in the adapter process.

**The Python-side siblings are unrouted.** `docs/library_ref/datafusion.md` and
`docs/library_ref/pyarrow.md` exist but no skill routes them, and the design suite gives them no
demand — read them directly if a Python-side question ever arises.

**Rule of thumb:** editing a `.rs` file, `Cargo.toml`, or the PyO3 boundary → this skill. Delta
tables and the `_delta_log` → `deltalake-rust-ref`. Graph algorithms in fact generation →
`petgraph-ref`. Fuller context: `REFERENCE.md` §5.
