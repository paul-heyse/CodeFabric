---
name: datafusion-pyarrow-rust-ref
description: "Reference navigator for the Rust DataFusion + Arrow stack. SKILL.md maps five deep-dives at `docs/library_ref/`: `datafusion_rust.md` (engine §0-§40), `datafusion_planning_rust.md` (planning §41-§60), `datafusion_schemas_rust.md` (schemas S1-S15), `datafusion_calculations_rust.md` (UDF/UDAF C1-C26), `pyarrow_rust.md` (Arrow crate stack §0-§30); REFERENCE.md (same folder) holds per-doc section indexes, the cross-document overlap matrix, decision trees, and operating rules. Use when Rust touches `use datafusion::`/`use arrow::`/`use arrow_*::`/`use parquet::`/`use object_store::`/`use arrow_flight::`/`pyo3_arrow`, edits `Cargo.toml` for those crates, or authors `SessionContext`/`RecordBatch`/`DataFrame`/`Expr`/`LogicalPlan`/`ExecutionPlan`/`TableProvider`/`ScalarUDFImpl`/`Schema`/`Field`/`DataType`, Substrait/Flight SQL/ADBC, or any `__arrow_c_*` PyCapsule Rust↔Python boundary. Python-side DataFusion/PyArrow → sibling `datafusion-pyarrow-ref`."
allowed-tools: Read, Grep, Glob, Bash
---

# DataFusion + Arrow Rust Reference Navigator

Routes five deep-dive references for the **Rust** DataFusion + Arrow stack. This SKILL.md is the **core map**: version anchors, the five-document topic table, reading strategy, where-to-look routing, and the key invariants. The companion **`REFERENCE.md`** (same folder) carries the per-document section indexes (every §/S/C section with line numbers), the cross-document overlap matrix, the decision trees (crate choice · planning surface · UDF kind · read/write path · cross-runtime · slow-query diagnosis · schema governance · Cargo setup), the full 22 operating rules, and the smartref project context. Reach for REFERENCE.md once you know *which* document you need and want its section map; cross-references back here are written `SKILL §...`.

**Out of scope** (covered elsewhere): Python `datafusion`/`pyarrow` packages → sibling skill **`datafusion-pyarrow-ref`** (`docs/library_ref/datafusion.md`, `pyarrow.md`). Rust `deltalake` / deltalake-on-DataFusion → sibling skill **`deltalake-rust-ref`** (`docs/library_ref/deltalake_rust.md`; the older `Datafusion_Deltalake_*.md` siblings are historical). `duckdb`/`ibis`/`polars-rs` ergonomic comparisons are catalog-only (pa-rust §22). DataFusion **Python** UDF authoring → `datafusion-pyarrow-ref` §16/§33-§35.

---

## Version anchors

* **DataFusion Rust 54.0.0** (released; declares Arrow/Parquet `58.3.0`, edition 2024, MSRV 1.88) — embeddable analytical query engine; SQL + DataFrame APIs over Arrow `RecordBatch`; logical/physical optimizer; custom planner hooks; async streaming. Every public entry point above `LogicalPlan` is `async` (Tokio `rt-multi-thread`, `macros`; `futures` for stream combinators). What changed crossing 53→54 (breaking APIs, semantics, new features, upgrade workflow) → `docs/library_ref/datafusion_54vs53.md`.
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
| What changed 53→54 — breaking APIs, semantic shifts, new-feature adoption, upgrade workflow | `docs/library_ref/datafusion_54vs53.md` (sibling migration spec; per-topic DF-54 deep-dives live in the five docs — see REFERENCE.md §2 "DataFusion 54 additions") |

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

## Project context: smartref

**Rust DataFusion + Arrow is now smartref's core compiled substrate — the project is pivoting hard onto it.** A **23-crate Cargo workspace** under `crates/` (nearly every member depends on `datafusion` + `arrow`) builds the **`smartref_core_native`** PyO3 extension (`crates/smartref_pyo3`, the workspace default-member; `pyo3-arrow` + `arrow-pyarrow` boundary), installed as an editable maturin build (dev profile — never `--release` on the editable refresh) and imported across `src/smartref/adapters/datafusion/`, `shared/graph/`, `shared/solver_math/`, `shared/arrow/schema/`, and `kernel/source_ingress/workbook/`. The Python `datafusion`/`pyarrow` packages still ship on the *other* side of the PyCapsule boundary, but the substrate is Rust. **Workspace pins** (`Cargo.toml`, all centralized in `[workspace.dependencies]` so members stay unified — Rules 6 & 18): `datafusion = "=54.0.0"` (+ `-proto`/`-substrait`/etc., matching these docs' anchor), `arrow` ecosystem `58.3.0` (incl. `arrow-flight` w/ `flight-sql`, `arrow-pyarrow`, `parquet`).

**Crate → reference-document map** (where to read when working in each):

| Crates | Read |
|---|---|
| `smartref_arrow_schema` (foundational — schema specs, fingerprint, Rust/Py parity) | **df-schemas** S1-S15 · **pa-rust** §3/§26 |
| `smartref_catalog`, `smartref_workbook`, `smartref_substrate_context`, `smartref_action_log` (sessions, `TableProvider`, listing/Parquet, `object_store`) | **df-rust** §3/§14/§17/§18 · **df-schemas** S10 · **df-planning** §51 · **pa-rust** §13/§14 |
| `smartref_plan_spec`, `smartref_plan_lint`, `smartref_plan_contracts`, `smartref_cache`, `smartref_substrait`, `smartref_substrate_compiler` (programmatic plans, lints, artifacts, proto/Substrait cache) | **df-planning** §41-§56 · **df-rust** §19/§22/§36 · **pa-rust** §25 |
| `smartref_udf`, `smartref_udf_contracts`, `smartref_solver_kernels`, `smartref_graph_kernels`, `smartref_symbolic_kernels` (UDF/UDAF, COO/CSR + solver/symbolic kernels) | **df-calcs** C1-C13 · **df-rust** §24 · **pa-rust** §5/§7/§17 |
| `smartref_bus` (Arrow Flight), `smartref_pg_adbc` (ADBC / PG-as-source), `smartref_pyo3` (`smartref_core_native` PyCapsule boundary) | **pa-rust** §19/§20/§21 |

(`smartref_pyrefly_query_core`/`smartref_pyrefly_queryd` are the pyrefly type-intelligence crates — no Arrow/DataFusion, out of scope here.)

If a Rust calc crosses back to Python, convert at the `pyo3-arrow` / `arrow_pyarrow` boundary, then structure the `pa.Table`/`pa.RecordBatch` into smartref contracts via cattrs — never via pickle/JSON when a PyCapsule path exists. The solver path still drives external SCIP/Ipopt via Pyomo (see `CLAUDE.md`); `smartref_solver_kernels` handles Arrow-native assembly/projection around that contract (df-calcs C11 route matrix), not in-process MILP. **Rule of thumb:** editing a `.rs` file / `Cargo.toml` / a `pyo3-arrow`-bridged surface → this skill; editing a `.py` file calling `import datafusion`/`import pyarrow` → `datafusion-pyarrow-ref`. Fuller per-crate context: `REFERENCE.md` §5.
