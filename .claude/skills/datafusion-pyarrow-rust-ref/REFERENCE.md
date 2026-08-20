# DataFusion + Arrow Rust — Detailed Reference

This document is the deep-dive companion to `SKILL.md` in the same directory. SKILL.md carries the core map (version anchors, the five-document topic table, reading strategy, where-to-look routing, and the seven key invariants); this file carries the full **per-document section indexes** (every §/S/C section with its line number), the **cross-document overlap matrix** (who is authoritative per topic), the **decision trees**, the **full 22 operating rules**, and the **CodeFabric project context**.

Cross-references back into the core map are written `SKILL §...`. Read SKILL.md first; reach here once you know which document you need and want its section map, when a routing choice gets hard, or when you need the authoritative-source table for an overlapping topic.

Every doc follows a **catalog-first, then deep-dives** layout, but with different prefixes for the deep-dive headers — disambiguating those prefixes is the single most important skill when grepping (see Operating Rule 20). Deep-dives run 600-2,400 lines and use `## N.M` (DataFusion docs) or `## N.M`/`### N.M` (pyarrow_rust) for subsections; each section closes with anti-patterns + an agent checklist. Load the closing 100-200 lines before drafting code. The **catalog blocks at the top of each doc are themselves a useful map** — they enumerate every subsection topic in 2-5 bullets per section before the deep-dive begins.

## Table of Contents

- **§1 — Per-document section indexes**
  - §1.1 `datafusion_rust.md` (§0-§40)
  - §1.2 `datafusion_planning_rust.md` (§41-§60)
  - §1.3 `datafusion_schemas_rust.md` (S1-S15)
  - §1.4 `datafusion_calculations_rust.md` (C1-C26)
  - §1.5 `pyarrow_rust.md` (§0-§30)
- **§2 — Cross-document overlap matrix** (authoritative source per topic)
- **§3 — Decision trees** (crate · planning surface · UDF kind · column type · read path · write path · plan layer · cross-runtime · slow-query · schema governance · Cargo project)
- **§4 — Operating rules** (full set of 22)
- **§5 — Project context: CodeFabric** (pre-implementation; spec-section → chapter map; the three boundaries)

---

## §1 — Per-document section indexes

### §1.1 `datafusion_rust.md` — section index (§0-§40)

Catalog `1-841`. Deep-dives start at line `876` with `# DataFusion Advanced — 0)` and use `## N.M` subsections. Every section ends with anti-pattern inventory + agent checklist; load the closing 100-200 lines before authoring.

| § | Line | Title | Key subsections |
|---|------|-------|-----------------|
| **0** | 876 | Scope, versioning, mental model | 0.0 version anchors · 0.1 identity · 0.2 value case · 0.3 canonical pipeline · 0.4 minimum vocabulary · 0.5 Arrow memory model · 0.6 lazy vs executed · 0.7-0.10 SQL/DataFrame/in-memory/streaming canonical syntax · 0.11 logical vs physical contract · 0.12 pull streaming · 0.13 runtime/deployment · 0.14 security · 0.15 agent invariants · 0.16 anti-patterns |
| **1** | 1362 | Installation, crate selection, Rust project layout | 1.0 source-of-truth · 1.1 minimal manifest · 1.2 datafusion-vs-subcrates · 1.3 Arrow version alignment · 1.4 feature flags · 1.5-1.6 Tokio + runtime builder · 1.7 workspace layout · 1.8-1.10 release profiles · 1.11 build commands · 1.12 anti-patterns |
| **2** | 1993 | First executable Rust app | 2.0-2.4 SQL-over-CSV, DataFrame-over-CSV, in-memory batches, pretty-print, `Result<()>` return |
| **3** | 2566 | Session model and execution state | `SessionContext` · `SessionState` · `SessionConfig` · `ConfigOptions` · `RuntimeEnv` · `TaskContext` · `ExecutionProps` · table/function registries · memory pools · disk managers · object-store registry · session lifecycle · 3.15 DF-54 extension-type registry |
| **4** | 3344 | Data model: Arrow schemas, arrays, batches | `Schema`/`Field`/`DataType` · `RecordBatch` · `ArrayRef` (`Arc<dyn Array>`) · nullable columns · scalar vs array · `DFSchema` vs Arrow `Schema` · qualifiers · `Arc<Schema>` patterns |
| **5** | 4229 | SQL API | `ctx.sql` · `sql_with_options` · register-before-query · `DataFrame` from SQL · collect/show/stream/write · `SQLOptions` |
| **6** | 4905 | SQL syntax reference | SELECT · WITH/CTE · FROM · joins · WHERE · GROUP BY · HAVING · QUALIFY · ORDER BY · LIMIT · set ops · pipe operators · identifier casing (lower-cased unless quoted) · 6.21 `LATERAL` joins (54) |
| **7** | 5953 | SQL data types ↔ Arrow type mapping | `Utf8View` default · numeric · decimal (max precision 76) · date/time/timestamp/interval · boolean · binary · `arrow_typeof` · `arrow_cast` · string-type config · 7.21 DF-54 comparison coercion |
| **8** | 6776 | DDL and catalog-affecting SQL | `CREATE EXTERNAL TABLE` · CTAS · CREATE/DROP SCHEMA · table options · schema inference vs explicit · in-memory vs custom catalogs |
| **9** | 7848 | DML and write paths | `INSERT INTO` · `COPY ... INTO` · writer options · partitioned output · row-group/file sizing · output file parallelism · safe overwrite/append · `DataSink` |
| **10** | 8860 | DataFrame API | `read_csv`/`read_parquet`/`read_batch`/`table` · filter/select/aggregate/sort/limit/join/union/with_column · collect/show/`execute_stream` · lazy plan builder |
| **11** | 9744 | Expression API (`Expr`) | `col`/`lit` · boolean/arithmetic/comparison methods (`.eq`/`.gt`/`.lt`/`.and`/`.or` — Rust has **no operator overloading** for DF expressions) · casts/aliases/sort exprs · aggregate/scalar function calls · CASE/WHEN · null handling · 11.25 `Operator::Colon` + `InListExpr` (54) |
| **12** | 10667 | Built-in functions catalog | scalar · string · math · date/time · conditional · aggregate · window · array/struct · special · CLI-only helpers · Spark-compat · 12.27 new DF-54 functions (vector/lambda/metadata) |
| **13** | 11905 | Nested data | ARRAY/LIST · STRUCT · maps · `col['field']` / `col[1]` · struct construction · Postgres JSON `->`/`->>` · nested Parquet/Arrow |
| **14** | 12941 | Data sources and file formats | CSV · Parquet · JSON · Avro · Arrow IPC · listing tables · partitioning · `ListingTable` · 14.26 typed `PartitionedFile.extensions` (54) |
| **15** | 13915 | Parquet deep dive | reader · writer · row-groups · stats · page index · bloom filters · column projection · predicate pushdown · encryption · 15.33 DF-54 Parquet changes (nested pushdown, `coerce_int96_tz`, CDC) |
| **16** | 15104 | Object stores + remote locations | `object_store` crate · S3/GCS/Azure/HTTP · runtime registration · URL parsing · IO pool |
| **17** | 15872 | Catalogs / schemas / tables | hierarchy `CatalogProviderList → CatalogProvider → SchemaProvider → TableProvider` · default `datafusion.public` · `information_schema` · custom catalogs · remote metastore caveats |
| **18** | 17094 | Custom `TableProvider` | trait surface · schema · supports_filter_pushdown · scan/insert · projections · partitions · async statistics |
| **19** | 18360 | Logical plans | `LogicalPlan` variants · `LogicalPlanBuilder` · extension nodes · plan traversal · placeholders · logical→physical · policy validation |
| **20** | 19556 | Physical plans + execution operators | `ExecutionPlan` · `PlanProperties` · partitioning · ordering · children · `execute` · streams · metrics · `PhysicalExpr` · resource reservations · 20.27 `ScalarSubqueryExec` (54) |
| **21** | 20753 | Streaming execution model | per-`RecordBatch` streaming · batch size · backpressure · partitioned streams · bounded vs unbounded · pipeline breakers · 21.25 morsel scans + work stealing (54) |
| **22** | 21885 | Query optimizer | analyzer rules · logical optimizer rules · physical optimizer rules · pushdown families · join optimization · statistics · custom rules · 22.29 Top-K + optimizer switches (54) |
| **23** | 23007 | Join algorithms and tuning | hash · sort-merge · nested-loop · semi/anti · join keys · hash vs merge decision · partition + ordering preconditions · 23.29 DF-54 join/exchange improvements |
| **24** | 24275 | User-defined functions (UDF/UDAF/UDWF/UDTF) | `create_udf` · `ScalarUDFImpl` · async UDFs · `AggregateUDFImpl` · `Accumulator`/`GroupsAccumulator` · `WindowUDFImpl` · `TableFunctionImpl` · `Signature` · `Volatility` · registration · 24.23 higher-order fns + lambdas (54) |
| **25** | 25390 | Extending SQL syntax | `ExprPlanner` · `TypePlanner` · `RelationPlanner` · parser wrappers · custom SQL operators · 25.23 `SparkSqlDialect` (sqlparser 0.62) |
| **26** | 26372 | Custom logical and physical operators | extension `LogicalPlan` node · custom `ExecutionPlan` · `PhysicalExpr` · scheduling + metrics + memory accounting · 26.22 `TreeNodeContainer` `Default` bound (54) |
| **27** | 27581 | Configuration system | `ConfigOptions` · `SessionConfig` · runtime config · per-session overrides · option discoverability · 27.28 new DF-54 config options |
| **28** | 28585 | Memory management + spilling | `MemoryPool` (greedy/fair/track-consumers) · `DiskManager` · spill directories · memory reservations · 28.28 NLJ spilling (54) |
| **29** | 29756 | Performance tuning guide | partition count · batch size · repartition · pushdown · stats refresh · PGO (`29.x.x` PGO playbook) · 29.25 `foldhash` hashing (54) |
| **30** | 31062 | Metrics, profiling, explainability | `MetricsSet` · operator metrics · `EXPLAIN [ANALYZE]` · plan display · perf counters |
| **31** | 32406 | CLI as deployment + debug tool | `datafusion-cli` · `.sql` · `\d` · debug commands · introspection |
| **32** | 33192 | Testing and correctness | sqllogictests · golden plans · property + fuzz · differential vs DuckDB/Postgres |
| **33** | 34797 | Error handling and diagnostics | `DataFusionError` taxonomy · plan vs exec errors · backtraces · diagnostic context |
| **34** | 36020 | API stability, upgrades, version migration | DataFusion ⇄ Arrow pair · upgrade procedure · breaking-change catalog · compat tests |
| **35** | 37018 | Architecture and crate organization | top-level `datafusion` re-exports · `datafusion-common` · `datafusion-expr` · `datafusion-physical-plan` · `datafusion-optimizer` · `datafusion-substrait` · `datafusion-proto` · `datafusion-cli` |
| **36** | 38149 | Plan serialization + interoperability | `datafusion-proto` · Substrait · custom serde · plan-hash · interop matrix · 36.26 DF-54 proto decode contexts |
| **37** | 39283 | Distributed + ecosystem integrations | 37.3 Ballista · 37.4 DataFusion Python · 37.5 DataFusion Java · 37.6 DataFusion Comet for Spark · 37.7 API differences · 37.8 ops complexity · 37.9-37.13 compat/risk/checklist/anti-patterns |
| **38** | 40033 | Production deployment patterns in Rust | service profiles · query admission · resource isolation · health/readiness · graceful shutdown |
| **39** | 41239 | Security and governance | SQL allowlists · function allow/deny · tenant isolation · audit · query timeout · result cap |
| **40** | 42365 | Best practices + anti-patterns | dependency · query building · sources · Parquet · runtime · optimizer/extensions · testing |

### §1.2 `datafusion_planning_rust.md` — section index (§41-§60, deep-dives §41-§56 only)

Catalog `1-1125`. Deep-dives start at line `1126` with `# DataFusion Advanced — 41)`. **Sections §57-§60 are catalog-only** (line ranges 868-1097).

| § | Line | Title |
|---|------|-------|
| **41** | 1126 | End-to-end planning lifecycle + phase boundary map (entrypoints / phases / artifact-per-phase / state-per-phase / error-per-phase) · 41.18 DF-54 scalar-subquery lifecycle change |
| **42** | 2414 | SQL planner + binder internals — `sqlparser` AST → `SqlToRel` → `LogicalPlan`; `ContextProvider`/`PlannerContext`; table + column + expression + query-shape binding; planner-extension hooks · 42.21 sqlparser 0.62 / `SparkSqlDialect` (54) |
| **43** | 3945 | Programmatic logical planning with `DataFrame`/`Expr`/`LogicalPlanBuilder`; plan factories; avoiding SQL string generation; parameterized plans; output-schema stability; PlanSpec pattern |
| **44** | 5908 | Plan schema, column identity, aliases, qualifier governance — `DFSchema` vs Arrow `Schema`; relation qualifiers; join-output identity; output-schema contracts; alias preservation through rewrites; schema diff |
| **45** | 7615 | Expression lifecycle — unresolved SQL expression → bound `Expr` → physical expression; coercion; constant folding; alias propagation; physical lowering · 45.21 lambdas (54) · 45.22 field-aware casts (54) |
| **46** | 9165 | Logical plan validation + policy linting before optimization/execution — plan lints (preview-without-LIMIT, unbounded materialization, UDF-in-hot-path, missing partition filter), authorization checks, governance gates |
| **47** | 10917 | Planner metadata — statistics, constraints (PK/UNIQUE/CHECK/NOT NULL), functional dependencies, partitioning, ordering equivalences · 47.4.6 `Arc<Statistics>` contract · 47.24 DF-54 statistics overhaul |
| **48** | 12350 | Analyzer + logical optimizer rule cookbook — type coercion, common-subexpression, projection pushdown, predicate pushdown, decorrelation, constant folding, custom rules · 48.26 `TreeNodeContainer` `Default` bound (54) |
| **49** | 13707 | Physical planning + logical-to-physical lowering map — `PhysicalPlanner` · `DefaultPhysicalPlanner` · operator selection · `PhysicalExpr` lowering · sort planning · 49.30 `ScalarSubqueryExec` (54) · 49.31 `LoweredAggregateBuilder` (54) |
| **50** | 15174 | Physical plan properties — partitioning, ordering, equivalence, boundedness, emission characteristics |
| **51** | 16336 | Scan planning + source pushdown — `TableProvider`, file scans, custom sources, `supports_filter_pushdown`, projection pushdown, statistics-driven pruning · 51.23 DF-54 scan-planning additions |
| **52** | 17881 | Join planning decision model — hash vs merge vs nested-loop selection, broadcast/repartition, semi/anti planning · 52.32 DF-54 join execution changes |
| **53** | 19616 | Streaming topology, boundedness, pipeline-breaker planning |
| **54** | 21128 | Runtime execution planning — partitions, task scheduling, memory reservations, spill discipline · 54.25 file-stream work stealing (54) |
| **55** | 22493 | Planning artifact package — reproducible plan debug bundle (`LogicalPlanArtifacts`, schema-artifact-from-DFSchema, plan-hash) |
| **56** | 24396 | Plan serialization, caching, fingerprints, invalidation — `datafusion-proto` vs Substrait vs SQL/PlanSpec as cache key; invalidation triggers; per-tenant cache · 56.32 DF-54 decode contexts + transport coverage |
| 57 | catalog (868) | Planning diagnostics + error taxonomy (catalog-only) |
| 58 | catalog (920) | Custom planning extensions: SQL ext vs logical ext vs provider pushdown vs physical op (catalog-only) |
| 59 | catalog (973) | LLM-agent planning contract — `PlanSpec`/`ExprSpec` → DataFusion plans (catalog-only) |
| 60 | catalog (1039) | End-to-end governed planning reference architecture (catalog-only) |

### §1.3 `datafusion_schemas_rust.md` — section index (S1-S15)

Catalog `1-790`. Deep-dives start at line `791` with `# DataFusion Advanced — S1)` and use `## S1.M` subsections (e.g., `S1.0 Objective`, `S1.1 Schema vocabulary`, `S1.2 End-to-end schema lifecycle diagram`, `S1.3 Schema surfaces matrix`).

| § | Line | Title |
|---|------|-------|
| **S1** | 791 | Schema lifecycle + invariants across DataFusion — vocabulary, schema-surfaces matrix, source-origin schemas, `DFSchema` qualification, plan-schema propagation, runtime batch validation, sink/output schemas |
| **S2** | 2661 | Schema creation surfaces + factory patterns — `Schema::new` / `Schema::new_with_metadata` / `Field::new` / `Field::new_with_metadata` · type constructors · builders · `Arc<Schema>` discipline |
| **S3** | 4867 | Schema inference, explicit overrides, multi-file drift — Parquet inference, CSV inference, JSON inference, listing-table inference, override merging, drift detection across files in same directory |
| **S4** | 6628 | Naming, identifier normalization, qualifiers, output field names — quoted vs unquoted, case normalization, `catalog.schema.table.column` qualifier, aliasing through rewrites (incl. DF-54 `unalias_nested` metadata retention) |
| **S5** | 8091 | Type compatibility, coercion, schema equality — `matches_arrow_schema`, `has_equivalent_names_and_types`, `datatype_is_logically_equal`, `datatype_is_semantically_equal`, `strip_qualifiers`, `replace_qualifier`, full compatibility framework · S5.3.9 DF-54 comparison vs union coercion · S5.16 DF-54 output-schema shape changes |
| **S6** | 9840 | Schema evolution + migration lifecycle — added/removed/renamed/widened/narrowed fields, compatibility matrix, evolution classifier, metadata preservation through evolution |
| **S7** | 11661 | Schema metadata, Arrow extension types, semantic annotations — `Field.metadata` byte-map discipline, registry, canonical extensions, parameterized extensions, IPC round-trip · S7.20 DF-54 extension-type registry · S7.21 metadata-aware expressions + field-aware casts (54) |
| **S8** | 13391 | Constraints, functional dependencies, defaults, table contracts — `Constraint::PrimaryKey/Unique`, `FunctionalDependencies`, `NOT NULL`, default expressions, contract-attached metadata · S8.7.2 DF-54 statistics API notes |
| **S9** | 14873 | Catalog schema management, remote metastores, `information_schema` — `MemoryCatalogProvider`, custom `SchemaProvider`, remote-metastore patterns (Iceberg/Hive/Glue/Unity), `information_schema` views |
| **S10** | 16807 | Custom `TableProvider` schema adaptation + projection mapping — `schema()` contract, projection indices vs schema, schema-adapter pattern, stream-schema correctness · S10.24 typed `PartitionedFile` extensions (54) |
| **S11** | 18479 | Logical-plan schema propagation + operator output contracts — `LogicalPlan::schema()`, per-operator output schema rules (Filter preserves, Projection rewrites, Join merges, Aggregate replaces), schema invariants under optimization |
| **S12** | 19785 | Nested, partition, virtual-column schemas — struct/list/map nesting, partition columns appended at scan, virtual columns (`__row_id`, `__filename`), partition-key types |
| **S13** | 21256 | View, CTAS, derived-table schema stability — view freezing vs view rebinding, CTAS schema capture, subquery alias schemas, derived-relation guarantees |
| **S14** | 22681 | Schema testing, diagnostics, error cookbook — `Schema::equals_with_options`, golden-schema snapshot, diff reporting, common schema errors (`SchemaError`/`Plan`/`Internal`) and remediation |
| **S15** | 24158 | Schema security, governance, tenant isolation — schema as authorization surface, column-level masking, tenant-scoped catalogs, metadata redaction |

### §1.4 `datafusion_calculations_rust.md` — section index (C1-C26, deep-dives C1-C13 only)

Catalog `1-1255`. Deep-dives start at line `1256` with `# DataFusion Advanced — C1)`. **Sections C14-C26 are catalog-only** (line ranges 642-1241). Note: within a C-section, larger sub-parts also use H1 headings of the form `# CN.M Title` (e.g. `# C1.19`).

| § | Line | Title |
|---|------|-------|
| **C1** | 1256 | User-defined calculation architecture + decision tree — placement matrix (`Expr` / built-in / scalar UDF / async UDF / UDAF / UDWF / UDTF / custom logical / custom physical / external preprocessing) by output cardinality × evaluation phase × cost × statefulness × determinism × optimizer visibility × distributed portability × security · C1.19 higher-order UDFs + lambdas (54) · C1.20 implementing `HigherOrderUDFImpl` (54) |
| **C2** | 3215 | Calculation lifecycle + invariants — define→class→signature→return→implement→register→expose→document→version→authorize→test→profile→deploy→deprecate; hard invariants (registered before planning, signature⇄coercion, volatility⇄semantics, scalar row-count preserved, UDAF state schema agreement, UDWF frame contracts, UDTF planning-cost limits); `CalculationSpec`/`FunctionManifest`/`SignatureSpec`/`ReturnFieldSpec`/`NullPolicy`/`VolatilityPolicy` artifacts · C2.12 DF-54 upstream built-in contract changes |
| **C3** | 5084 | Function registry, cataloging, discovery — DataFusion runtime registry vs product catalog vs tenant policy vs documentation registry vs version registry; canonical name + aliases + family + signature + cost class + security class; collision rules; semantic versioning per function · C3.29 higher-order registry + discovery (54) |
| **C4** | 7100 | Function package + plugin architecture — `calc-core`/`calc-arrow-kernels`/`calc-functions`/`calc-aggregates`/`calc-window`/`calc-table-functions`/`calc-manifest`/`calc-tests` crate layout; `FunctionPackage` trait; deterministic registration order; static vs dynamic plugin model |
| **C5** | 9286 | Signature design + overload resolution — exact / coercible / comparable / numeric / string / variadic; named-argument ergonomics; overload strategy; coercion hazard catalog; `SignatureSpec` manifest |
| **C6** | 10743 | Return type, nullability, metadata inference — `return_type(arg_types)` vs `return_field_from_args(args)`; nullability modes; metadata-emitting UDFs; return-shape patterns; runtime validation against declared field · C6.16 extension-type-aware return fields (54) |
| **C7** | 12146 | Null / NaN / infinity / error / invalid-input semantics — planning vs execution errors; per-row error rows vs scalar errors; audited struct return; diagnostic side-tables; NaN/inf policy; zero-row batch handling · C7.19 optimizer-controlled filter order (54) |
| **C8** | 13727 | Vectorized Arrow implementation patterns — `ColumnarValue` dispatch · downcast helpers · unary/binary/ternary/variadic input patterns · list/struct in/out · string dispatch · dictionary input support · output builder cookbook · Arrow compute kernel composition · `DataType` dispatch · production scalar UDF wrapper |
| **C9** | 15393 | Complex conditionality + expression composition — `CASE WHEN` vs scalar UDF · safe arithmetic · bucketization · piecewise functions · conditional aggregates/windows · nested-field conditional extraction · rule-table joins instead of giant CASE · generated-Expr builder · alias stability through composition |
| **C10** | 16754 | Nested + structured return calculations — UDFs returning `StructArray`/`ListArray`/`MapArray`/fixed-size list embeddings · nested diagnostics records · functions over nested inputs · schema evolution for nested returns |
| **C11** | 18035 | External-library integration strategy — Rust crates vs PyO3 vs Python subprocess vs WASM vs FFI vs Substrait export to external engine; SciPy/SymPy case taxonomy; Rust-alternative mapping; security + observability for external calls |
| **C12** | 19387 | Async UDFs for external I/O and services — `ScalarUDF::async_invoke` model · external-call semantics · authentication + secret handling · timeout/retry/circuit-breaker · concurrency + rate controls · caching · cancellation · `runtime` posture · resource-control manifest |
| **C13** | 20793 | Aggregate UDAF state design — `Accumulator` lifecycle (init/update/merge/evaluate/state); state-design taxonomy; distributed readiness; memory accounting; `GroupsAccumulator`; weighted average / geometric mean / online variance / top-k / approx-quantile / domain-quality examples; `AggregateUDFImpl` advanced API · C13.22 `LoweredAggregateBuilder` lowering (54) |
| C14 | catalog (642) | Window UDF frame semantics (catalog-only) |
| C15 | catalog (685) | Table UDFs + parameterized relations (catalog-only) |
| C16 | catalog (726) | Optimizer interaction for custom calculations (catalog-only) |
| C17 | catalog (769) | Function authorization, allowlists, risk classes (catalog-only) |
| C18 | catalog (817) | Function observability + diagnostics (catalog-only) |
| C19 | catalog (864) | Testing strategy for custom calculations (catalog-only) |
| C20 | catalog (918) | External reference + differential validation (catalog-only) |
| C21 | catalog (965) | Calculation DSL / semantic calculation specs (catalog-only) |
| C22 | catalog (1014) | Domain-specific calculation libraries (catalog-only) |
| C23 | catalog (1059) | Stateful, cached, memoized calculations (catalog-only) |
| C24 | catalog (1102) | Calculation performance engineering (catalog-only) |
| C25 | catalog (1149) | Distributed execution + portability (catalog-only) |
| C26 | catalog (1197) | Generated-code discipline for LLM agents (catalog-only) |

### §1.5 `pyarrow_rust.md` — section index (§0-§30, deep-dives §0-§28)

Catalog `1-473`. Deep-dives start at line `474` with `# 0)` — note the **prefix is just `# N)`**, with no "DataFusion Advanced —" prefix. **Sections §29-§30 are catalog-only** (line ranges 423-471).

| § | Line | Title |
|---|------|-------|
| **0** | 474 | Scope, versioning, mental model — PyArrow is a C++ wrapper; the Rust equivalent is a **crate stack**. Coverage key (Direct/Close/Partial/Gap) for every PyArrow capability family. Version anchors for `arrow`/`parquet`/`datafusion`/`object_store`/`arrow-flight`/`pyo3-arrow`. |
| **1** | 1050 | Rust crate topology + dependency strategy — `arrow` top-level vs subcrates; independent crates (`parquet`/`arrow-flight`/`arrow-avro`); SemVer alignment; workspace patterns for apps / libs / Python extensions / query engines |
| **2** | 2101 | Installation, Cargo features, deployment profiles — feature flags by crate; minimal binary / data-lake / Python-ext / server profiles; pinning |
| **3** | 3440 | Arrow data model — `DataType` families · `Field` (name/type/nullable/metadata) · `Schema` (ordered fields + metadata) · nested-types-include-fields · canonical extension types · view types · decimal precision |
| **4** | 4760 | Buffers, memory ownership, nullability, zero-copy — `Buffer`/`MutableBuffer`/`BufferBuilder`/`NullBuffer`/`ScalarBuffer`/`Bytes`/`Arc<Buffer>` · null = validity bitmap · slice/clone is cheap (Arc bump) · zero-copy boundary |
| **5** | 5917 | Arrays, builders, scalars, chunked data — `ArrayRef` (`Arc<dyn Array>`) · concrete arrays (`PrimitiveArray<T>`/`StringArray`/`BooleanArray`/`ListArray`/`StructArray`/`DictionaryArray`/`RunEndArray`) · `*Builder` constructors · `ScalarValue` · downcast pattern |
| **6** | 7303 | RecordBatch, table-like workflows, streaming readers — `RecordBatch::try_new` · `RecordBatchReader` trait · `RecordBatchIterator` · streaming `Stream<Item=Result<RecordBatch>>` from DataFusion · Polars / DataFusion as Table-like wrappers |
| **7** | 8546 | Compute kernels — `arrow::compute` · `arrow-arith` · `arrow-cast` · `arrow-select` · `arrow-ord` · `arrow-string` · selection/take/filter/sort/cast · arithmetic · comparison · string ops · aggregation |
| **8** | 9699 | Compute expressions + query-style operations — DataFusion `Expr` as the Rust expression language (no analogous `pyarrow.compute.Expression` outside DataFusion); evaluating physical expressions; building selection predicates · 8.27 DF-54 vector/nested/metadata functions |
| **9** | 10870 | CSV, JSON, line-delimited ingestion — `arrow_csv::ReaderBuilder` / `WriterBuilder` · `arrow_json::ReaderBuilder` (line-delimited) · type inference vs explicit schema · streaming reads · escape/quote/delimiter configuration |
| **10** | 12084 | IPC, Arrow files, streams, Feather — `arrow_ipc::reader::StreamReader/FileReader` · `arrow_ipc::writer::StreamWriter/FileWriter` · IPC v2 (Feather) · zero-copy mmap read · compression options |
| **11** | 13122 | Parquet core — `parquet::file::reader::SerializedFileReader` · `parquet::arrow::ArrowReader/ArrowWriter` · `ParquetReadOptions`/`ParquetWriterProperties` · metadata · schema mapping |
| **12** | 14274 | Advanced Parquet — async `ParquetRecordBatchStreamBuilder` · cloud (object_store integration) · predicate/projection pushdown · row-filter API · column-statistics-driven pruning · bloom filters · CDC patterns · 12.25 DF-54 Parquet integration notes (CDC config, nested pushdown, `coerce_int96_tz`) |
| **13** | 15454 | Dataset-equivalent workflows — `ListingTable` (DataFusion) as the canonical multi-file dataset substitute · partition discovery · file-format dispatch · vs PyArrow `pyarrow.dataset.dataset(...)` · 13.29 typed `PartitionedFile.extensions` (54) |
| **14** | 16755 | Filesystem + object_store layer — `object_store::ObjectStore` trait · `LocalFileSystem` · `InMemory` · `AmazonS3Builder` · `GoogleCloudStorageBuilder` · `MicrosoftAzureBuilder` · `HttpBuilder` · pagination · retry · credential providers |
| **15** | 17852 | DataFusion SQL + DataFrame API — bridge into datafusion_rust.md §5-§11 from the Arrow-substrate perspective; what DataFusion gives you over raw Arrow compute |
| **16** | 18998 | Query planning, optimization, execution internals (from Arrow-substrate view) — links into datafusion_rust §19-§22 and datafusion_planning_rust §41-§56 · 16.26 DF-54 execution + plan-shape notes (scalar subqueries, foldhash, repartition coalescing, work stealing) |
| **17** | 20265 | Joins, aggregations, windows, grouped operations — what Arrow gives you (no native joins; via DataFusion) · `arrow::compute::sort_to_indices` · `arrow-row` for fast multi-column sort · DataFusion joins/aggs/windows |
| **18** | 21492 | UDFs / UDAFs / UDTFs + extension points — bridge into datafusion_calculations_rust C1-C13 from the Arrow-substrate angle; non-DataFusion compute extension via custom kernels · 18.28 higher-order UDFs + SQL lambdas (54) |
| **19** | 23016 | Flight RPC + Flight SQL — `arrow_flight::FlightClient`/`FlightServiceServer` · `FlightDescriptor`/`Ticket`/`FlightInfo` · streaming endpoints · handshake/auth · middleware · Flight SQL (`flight-sql` feature) |
| **20** | 24277 | ADBC + database connectivity — `adbc_core` driver model · Flight SQL driver · DataFusion-backed driver · vs ODBC/JDBC bridges |
| **21** | 25403 | Python interop: PyArrow, PyCapsule, PyO3, zero-copy extension modules — `arrow_pyarrow` (PyArrow-managed) vs `pyo3-arrow` (PyCapsule-first); `__arrow_c_schema__`/`__arrow_c_array__`/`__arrow_c_stream__` producer/consumer; building `maturin`-packaged extensions |
| **22** | 26691 | DataFrame ergonomics — Polars vs DataFusion vs raw Arrow; decision matrix for when to pick each |
| **23** | 27642 | ORC + Avro — `orc-rust` / `datafusion-orc`; `arrow-avro` (now the DF-54 Avro datasource basis) |
| **24** | 28631 | CUDA / GPU / device arrays / DLPack — Arrow C Device interface · `cudarc` (optional) · `dlpack-rs` (optional); DLPack has no null-bitmap semantics — fallback to CPU or mask-aware tensor |
| **25** | 29450 | Substrait + portable query plans — `datafusion-substrait`; `NamedStruct` bytes; `ExtendedExpression` bytes; cross-runtime exchange |
| **26** | 30503 | Extension types + custom logical types — `ExtensionType` registry; `UnknownExtensionType` fallback; round-tripping through IPC; Python ↔ Rust extension agreement · 26.22 DF-54 extension-type registry + field-aware casts |
| **27** | 31674 | Performance engineering + benchmarking — `criterion` benches; allocator (jemalloc/mimalloc); SIMD; profiling (perf/flamegraph); reproducible benchmarks |
| **28** | 32801 | Error handling, testing, compatibility matrix — `ArrowError` / `DataFusionError` / `ParquetError` taxonomy; sqllogictests; cross-runtime fixtures with PyArrow; compat.toml; fuzz local smoke |
| 29 | catalog (423) | Security, safety, malformed input model (catalog-only) |
| 30 | catalog (437) | PyArrow-to-Rust migration recipes (catalog-only) |

---

## §2 — Cross-document overlap matrix

Legend: ✅ authoritative, 🔁 cross-cut/summary, — not covered.
"datafusion_rust" abbreviated `df-rust`, `df-planning` for planning, `df-schemas`, `df-calcs`, `pa-rust`.

### Arrow data model — types, fields, schemas, metadata, buffers, nulls

| Topic | df-rust | df-planning | df-schemas | df-calcs | pa-rust | Authoritative |
|-------|--------:|-------------|-----------|----------|---------|----------------|
| `DataType`/`Field`/`Schema` factory | §4 🔁 | §44 🔁 | **S2** ✅ | — | **§3** ✅ | **pa-rust §3** general Arrow Rust; **df-schemas S2** DataFusion-contract factories |
| `Field` metadata byte-map + extension types | — | — | **S7** ✅ | — | §3, §26 ✅ | **df-schemas S7** governance + invariants; **pa-rust §26** registry + IPC round-trip |
| Nullability, validity bitmaps, `NullBuffer` | §4 🔁 | — | S5 🔁 | C7 ✅ | **§4** ✅ | **pa-rust §4** for the storage model; **df-calcs C7** for null-in-UDF semantics |
| `Buffer`/`MutableBuffer`/`BufferBuilder` zero-copy | — | — | — | C8 🔁 | **§4** ✅ | **pa-rust §4** |
| `ArrayRef` / `Arc<dyn Array>` / downcast | §4 🔁 | — | S2 🔁 | C8 ✅ | **§5** ✅ | **pa-rust §5** for the array layer; **df-calcs C8** for UDF-body downcast patterns |
| `ScalarValue` | §4 🔁 | §45 ✅ | — | C8 ✅ | §5 ✅ | **df-planning §45** for plan-side scalars; **pa-rust §5** for array-side scalars |
| Nested types (`StructArray`/`ListArray`/`MapArray`/dictionary/run-end) | §13 🔁 | — | **S12** ✅ | C10 ✅ | §3, §5 ✅ | **pa-rust §3+§5** physical; **df-schemas S12** DataFusion plan view; **df-calcs C10** UDF return |
| Decimal precision, view types, canonical extensions | §7 🔁 | — | S7 ✅ | — | **§3** ✅ | **pa-rust §3** |

### Schema lifecycle — across DataFusion

| Topic | df-rust | df-planning | df-schemas | df-calcs | pa-rust | Authoritative |
|-------|--------:|-------------|-----------|----------|---------|----------------|
| `DFSchema` vs Arrow `Schema` | §4 ✅ | §44 ✅ | **S1** ✅ | — | — | **df-schemas S1** unified lifecycle; **df-planning §44** plan-side qualifier governance |
| Schema inference + multi-file drift | §14 🔁 | — | **S3** ✅ | — | §13 🔁 | **df-schemas S3** |
| Qualifier governance + naming + ambiguity | §4 🔁 | **§44** ✅ | **S4** ✅ | — | — | **df-planning §44** + **df-schemas S4** |
| Type coercion, compatibility, equality helpers | §7 🔁 | §45 ✅ | **S5** ✅ | C5 🔁 | — | **df-schemas S5** is canonical; **df-planning §45** for coercion-during-expression-lifecycle; **df-calcs C5** signature-side |
| Schema evolution + migration policy | — | — | **S6** ✅ | — | — | **df-schemas S6** |
| Constraints / FDs / defaults | — | **§47** ✅ | **S8** ✅ | — | — | **df-schemas S8** for catalog-side; **df-planning §47** for planner-metadata use |
| Catalog + `information_schema` | §17 ✅ | — | **S9** ✅ | — | — | **df-schemas S9** + **df-rust §17** |
| Plan schema propagation / operator output rules | §19 🔁 | §44 ✅ | **S11** ✅ | — | — | **df-schemas S11** is canonical; **df-planning §44** for alias-preservation through rewrites |
| Schema testing + golden snapshots + diff | §32 🔁 | §55 ✅ | **S14** ✅ | — | §28 🔁 | **df-schemas S14**; **df-planning §55** for plan-artifact-level fingerprint capture |
| Schema as authz surface / tenant isolation | §39 🔁 | — | **S15** ✅ | C17 🔁 | — | **df-schemas S15** |

### Session / runtime / config

| Topic | df-rust | df-planning | pa-rust | Authoritative |
|-------|--------:|-------------|---------|----------------|
| `SessionContext`/`SessionState`/`SessionConfig` | **§3** ✅ | §41 🔁 | §15 🔁 | **df-rust §3** |
| `TaskContext`/`ExecutionProps`/`RuntimeEnv` | **§3** ✅ | §41 🔁 | — | **df-rust §3** |
| `ConfigOptions` / runtime config | §27 ✅ | §41 🔁 | — | **df-rust §27** |
| `MemoryPool` / `DiskManager` / spilling | **§28** ✅ | §54 ✅ | — | **df-rust §28** for pools; **df-planning §54** for plan-time memory reservation |
| Tokio runtime layout + `#[tokio::main]` | **§1** ✅ | — | §2 🔁 | **df-rust §1** |

### SQL surface

| Topic | df-rust | df-planning | pa-rust | Authoritative |
|-------|--------:|-------------|---------|----------------|
| `ctx.sql` / `sql_with_options` / `SQLOptions` | **§5** ✅ | §41 🔁 | §15 🔁 | **df-rust §5** |
| SQL grammar reference | **§6** ✅ | — | — | **df-rust §6** |
| SQL type ↔ Arrow type mapping | **§7** ✅ | — | — | **df-rust §7** |
| DDL (`CREATE EXTERNAL TABLE`, CTAS) | **§8** ✅ | — | — | **df-rust §8** |
| DML (`INSERT`, `COPY`) | **§9** ✅ | — | — | **df-rust §9** |
| `SqlToRel` / `sqlparser` AST | §0 🔁 | **§42** ✅ | — | **df-planning §42** |
| Custom SQL syntax (`ExprPlanner`/`TypePlanner`/`RelationPlanner`) | **§25** ✅ | §42 🔁 | — | **df-rust §25** API surface; **df-planning §42** binder integration |

### DataFrame / `Expr` / programmatic planning

| Topic | df-rust | df-planning | pa-rust | Authoritative |
|-------|--------:|-------------|---------|----------------|
| `DataFrame` (lazy plan builder) | **§10** ✅ | §43 ✅ | §22 🔁 | **df-rust §10** API; **df-planning §43** as plan-construction strategy |
| `Expr` API + Rust operator non-overloading | **§11** ✅ | §45 ✅ | §8 🔁 | **df-rust §11** general; **df-planning §45** lifecycle |
| `LogicalPlanBuilder` programmatic path | §19 ✅ | **§43** ✅ | — | **df-planning §43** |
| Plan factories + PlanSpec/`Expr`Spec | — | **§43, §59** ✅ | — | **df-planning §43 + §59** |
| Built-in functions catalog | **§12** ✅ | — | — | **df-rust §12** |
| Nested data access (`col['field']`/`col[1]`) | **§13** ✅ | — | — | **df-rust §13** |

### Plans, optimizer, execution

| Topic | df-rust | df-planning | pa-rust | Authoritative |
|-------|--------:|-------------|---------|----------------|
| End-to-end planning lifecycle | §0 🔁 | **§41** ✅ | §16 🔁 | **df-planning §41** |
| `LogicalPlan` variants + traversal | **§19** ✅ | §41 🔁 | — | **df-rust §19** |
| `ExecutionPlan` + `PhysicalExpr` + `PlanProperties` | **§20** ✅ | **§49, §50** ✅ | §16 🔁 | **df-rust §20** API; **df-planning §49+§50** lowering + properties |
| Streaming + boundedness + pipeline breakers | **§21** ✅ | **§53** ✅ | — | **df-rust §21** mechanics; **df-planning §53** plan-time decisions |
| Optimizer rule families (analyzer/logical/physical) | **§22** ✅ | **§48** ✅ | — | **df-rust §22** API; **df-planning §48** rule cookbook |
| Plan validation, lints, governance gates | §32 🔁 | **§46** ✅ | — | **df-planning §46** |
| Planner metadata (stats/constraints/FDs/partitioning/ordering) | §22 🔁 | **§47** ✅ | — | **df-planning §47** |
| Scan planning + provider pushdown | §15 🔁 | **§51** ✅ | §13 🔁 | **df-planning §51** |
| Join planning decision model | **§23** ✅ | **§52** ✅ | §17 🔁 | **df-rust §23** algorithm internals; **df-planning §52** plan-time decision |
| Runtime execution planning (partitions/scheduling/memory/spill) | §28 🔁 | **§54** ✅ | — | **df-planning §54** |
| Plan artifact bundle (logical+optimized+physical+hash) | §30 🔁 | **§55** ✅ | — | **df-planning §55** |
| Plan serialization + caching + fingerprints + invalidation | **§36** ✅ | **§56** ✅ | §25 🔁 | **df-planning §56** for cache-key + invalidation; **df-rust §36** for serialization surface |
| `EXPLAIN [ANALYZE]` + metrics | **§30** ✅ | §55 🔁 | — | **df-rust §30** |

### TableProvider / catalogs / sources

| Topic | df-rust | df-planning | df-schemas | pa-rust | Authoritative |
|-------|--------:|-------------|-----------|---------|----------------|
| Catalog/schema/table hierarchy | **§17** ✅ | — | **S9** ✅ | — | **df-rust §17** API; **df-schemas S9** governance |
| Custom `TableProvider` (trait/scan/schema/pushdown) | **§18** ✅ | §51 🔁 | **S10** ✅ | §13 🔁 | **df-rust §18** API; **df-planning §51** planner integration; **df-schemas S10** schema-adapter discipline |
| `ListingTable` (multi-file partitioned dataset) | §14 ✅ | — | — | **§13** ✅ | **df-rust §14** + **pa-rust §13** |
| Sources: CSV / JSON / Parquet / Avro / Arrow IPC | **§14** ✅ | — | — | §9-§12 ✅ | **df-rust §14** integration; **pa-rust §9-§12** raw Arrow Rust IO |
| Parquet read/write/metadata | **§15** ✅ | — | — | **§11-§12** ✅ | **pa-rust §11+§12** for the crate; **df-rust §15** for DataFusion integration |
| Object stores (S3/GCS/Azure/HTTP/Local) | **§16** ✅ | — | — | **§14** ✅ | **pa-rust §14** for `object_store` API; **df-rust §16** for `SessionContext` registration |

### UDF / UDAF / UDWF / UDTF / external integration

| Topic | df-rust | df-calcs | pa-rust | Authoritative |
|-------|--------:|----------|---------|----------------|
| Calculation placement decision tree | §24 🔁 | **C1** ✅ | §18 🔁 | **df-calcs C1** |
| Calculation lifecycle + invariants | §24 🔁 | **C2** ✅ | — | **df-calcs C2** |
| Function registry + catalog + discovery | §17 🔁 | **C3** ✅ | — | **df-calcs C3** |
| Function package + plugin architecture | §35 🔁 | **C4** ✅ | §1 🔁 | **df-calcs C4** |
| `Signature` + overload resolution + coercion | §24 ✅ | **C5** ✅ | — | **df-calcs C5** governance; **df-rust §24** API |
| Return-type + nullability + metadata inference | §24 🔁 | **C6** ✅ | — | **df-calcs C6** |
| Null/NaN/inf/error/invalid-input semantics in UDFs | §33 🔁 | **C7** ✅ | — | **df-calcs C7** |
| Vectorized Arrow implementation in UDFs | §24 🔁 | **C8** ✅ | §7 🔁 | **df-calcs C8** |
| Conditional + expression composition | §11 🔁 | **C9** ✅ | §8 🔁 | **df-calcs C9** |
| Nested + structured return UDFs | §13 🔁 | **C10** ✅ | §5 🔁 | **df-calcs C10** |
| External-library integration (Rust crate / PyO3 / subprocess / WASM / FFI / Substrait) | §37 🔁 | **C11** ✅ | §21 ✅ | **df-calcs C11** for the route matrix; **pa-rust §21** for the PyArrow PyCapsule path specifically |
| Async UDFs for external I/O + services | §24 🔁 | **C12** ✅ | — | **df-calcs C12** |
| UDAF state design + `Accumulator` lifecycle | §24 🔁 | **C13** ✅ | — | **df-calcs C13** |
| Volatility (immutable/stable/volatile) | **§24** ✅ | C2 ✅ | — | **df-rust §24** API; **df-calcs C2** invariants |

### Compute kernels + expressions

| Topic | df-rust | pa-rust | Authoritative |
|-------|--------:|---------|----------------|
| `arrow::compute` kernel surface | — | **§7** ✅ | **pa-rust §7** |
| `arrow-arith`/`arrow-cast`/`arrow-select`/`arrow-ord`/`arrow-string` | — | **§7** ✅ | **pa-rust §7** |
| Selection/take/filter/sort/cast at Arrow level | — | **§7** ✅ | **pa-rust §7** |
| DataFusion `Expr` as Rust expression language | **§11** ✅ | §8 🔁 | **df-rust §11** |
| Building physical predicates from `Expr` | §20 ✅ | §8 🔁 | **df-rust §20** |
| `arrow-row` for fast multi-column sort | — | **§17** ✅ | **pa-rust §17** |

### Python interop / FFI / cross-runtime exchange

| Topic | df-rust | pa-rust | Authoritative |
|-------|--------:|---------|----------------|
| Arrow C Data / C Stream / `__arrow_c_*` PyCapsule | §37.4 🔁 | **§21** ✅ | **pa-rust §21** |
| `arrow_pyarrow` (PyArrow-managed) | — | **§21** ✅ | **pa-rust §21** |
| `pyo3-arrow` (PyCapsule-first) | — | **§21** ✅ | **pa-rust §21** |
| Building maturin-packaged Rust extensions | — | **§21** ✅ | **pa-rust §21** |
| DataFusion Python bridge | **§37.4** ✅ | §22 🔁 | **df-rust §37.4** |
| DLPack / Arrow C Device (CUDA/GPU) | — | **§24** ✅ | **pa-rust §24** |
| Substrait + portable plans | **§36** ✅ | **§25** ✅ | **df-rust §36** for `datafusion-substrait`; **pa-rust §25** for cross-runtime exchange |

### Flight RPC / Flight SQL / ADBC

| Topic | df-rust | pa-rust | Authoritative |
|-------|--------:|---------|----------------|
| `arrow-flight` RPC | — | **§19** ✅ | **pa-rust §19** |
| Flight SQL (`flight-sql` feature) | — | **§19** ✅ | **pa-rust §19** |
| ADBC + driver model | — | **§20** ✅ | **pa-rust §20** |

### Distributed / ecosystem

| Topic | df-rust | pa-rust | Authoritative |
|-------|--------:|---------|----------------|
| Ballista (distributed execution) | **§37.3** ✅ | — | **df-rust §37.3** |
| DataFusion Comet (Spark acceleration) | **§37.6** ✅ | — | **df-rust §37.6** |
| DataFusion Java/Python subprojects | **§37.4, §37.5** ✅ | — | **df-rust §37** |
| Polars vs DataFusion vs raw Arrow DataFrame ergonomics | — | **§22** ✅ | **pa-rust §22** |

### Performance / observability / production / security

| Topic | df-rust | df-planning | pa-rust | Authoritative |
|-------|--------:|-------------|---------|----------------|
| Performance tuning guide | **§29** ✅ | §54 🔁 | **§27** ✅ | **df-rust §29** for DataFusion; **pa-rust §27** for Arrow-substrate bench/profiling |
| Metrics + EXPLAIN ANALYZE | **§30** ✅ | §55 🔁 | — | **df-rust §30** |
| CLI (`datafusion-cli`) | **§31** ✅ | — | — | **df-rust §31** |
| Testing + correctness (sqllogictests/golden/diff) | **§32** ✅ | §55 🔁 | **§28** ✅ | **df-rust §32** + **pa-rust §28** |
| Error taxonomy (`DataFusionError`/`ArrowError`/`ParquetError`) | **§33** ✅ | §57 (catalog) | **§28** ✅ | **df-rust §33** + **pa-rust §28** |
| API stability + upgrade procedure | **§34** ✅ | — | — | **df-rust §34** |
| Production deployment patterns | **§38** ✅ | — | — | **df-rust §38** |
| Security + governance + SQL/function allowlists | **§39** ✅ | §57-§58 (catalog) | §29 (catalog) | **df-rust §39** |
| Best practices + anti-patterns | **§40** ✅ | — | — | **df-rust §40** |
| ORC + Avro | §14 🔁 | — | **§23** ✅ | **pa-rust §23** |

### DataFusion 54 additions

| Topic | df-rust | df-planning | df-schemas | df-calcs | pa-rust | Authoritative |
|-------|--------:|-------------|-----------|----------|---------|----------------|
| Higher-order UDFs + SQL lambdas (`HigherOrderUDFImpl`, `Expr::Lambda`) | §24.23 🔁 | §45.21 🔁 | — | **C1.19-C1.20, C3.29** ✅ | §18.28 🔁 | **df-calcs C1.19-C1.20** |
| Extension-type registry + field-aware casts (`with_metadata`/`cast_to_type`) | §3.15 🔁 | §45.22 🔁 | **S7.20-S7.21** ✅ | C6.16 🔁 | §26.22 🔁 | **df-schemas S7.20-S7.21** |
| Physical uncorrelated scalar subqueries (`ScalarSubqueryExec`) | §20.27 🔁 | **§41.18, §49.30** ✅ | — | — | §16.26 🔁 | **df-planning §49.30** |
| Typed `PartitionedFile.extensions` (`FileExtensions` map) | **§14.26** ✅ | §51.23 🔁 | S10.24 🔁 | — | §13.29 🔁 | **df-rust §14.26** |
| Statistics contract (`Arc<Statistics>`, pruning relocation, NDV) | §20 🔁 | **§47.4.6, §47.24** ✅ | S8.7.2 🔁 | — | — | **df-planning §47.24** |
| Morsel-driven scans + file-stream work stealing | **§21.25** ✅ | §54.25 🔁 | — | — | §16.26 🔁 | **df-rust §21.25** |
| New DF-54 configuration options | **§27.28** ✅ | — | — | — | — | **df-rust §27.28** |
| Aggregate lowering (`LoweredAggregateBuilder`) | §30 🔁 | §49.31 🔁 | — | **C13.22** ✅ | — | **df-calcs C13.22** |
| Proto decode contexts (`from_bytes_with_ctx`, `PhysicalPlanDecodeContext`) | **§36.26** ✅ | §56.32 🔁 | — | — | — | **df-rust §36.26** |
| Comparison-coercion semantics (numeric-preferring, `type_union_coercion`) | §7.21 ✅ | — | **S5.3.9** ✅ | C2.12 🔁 | — | **df-schemas S5.3.9** governance; **df-rust §7.21** SQL surface |
| Full 53→54 migration review spec (all breaking changes + upgrade workflow) | — | — | — | — | — | **Absent** — `datafusion_54vs53.md` is cited by this skill's ancestry but does not exist here (§5) |

---

## §3 — Decision trees

### Pick the right Rust crate dependency

```
Need only Arrow buffers/arrays/types without compute?
  -> arrow-array + arrow-buffer + arrow-schema + arrow-data         (pa-rust §1)
Need Arrow compute (cast/select/sort/filter/string/arith)?
  -> arrow + features                                              (pa-rust §1, §7)
Need Parquet read/write?
  -> parquet (+ arrow feature for Arrow integration)                (pa-rust §11)
  -> + async feature for ParquetRecordBatchStreamBuilder           (pa-rust §12)
  -> + object_store feature for cloud Parquet                     (pa-rust §12, §14)
Need cloud storage (S3/GCS/Azure)?
  -> object_store                                                  (pa-rust §14; df-rust §16)
Need Arrow Flight or Flight SQL?
  -> arrow-flight (+ flight-sql feature)                          (pa-rust §19)
Need a query engine over Arrow?
  -> datafusion + tokio["rt-multi-thread", "macros"]              (df-rust §1)
Need Substrait?
  -> datafusion-substrait                                          (df-rust §36; pa-rust §25)
Need wire-format plan serialization?
  -> datafusion-proto                                              (df-rust §36)
Need to expose extension nodes over FFI?
  -> datafusion-ffi                                                (df-rust §35-§37)
Need to bridge Rust ↔ Python via Arrow?
  -> arrow_pyarrow (PyArrow-managed) OR pyo3-arrow (PyCapsule-first) (pa-rust §21)
Arrow version must agree across Parquet + DataFusion + your crate.
  -> Use datafusion::arrow re-export OR pin exact versions          (df-rust §1)
```

### Pick the right planning surface

```
End-user SQL (untrusted)?
  -> ctx.sql + SQLOptions allowlist (df-rust §5, §39)
Generated SQL from typed spec?
  -> avoid SQL string assembly; build LogicalPlan directly         (df-planning §43)
Application-driven query (typed)?
  -> DataFrame API (df-rust §10) for fluent path
  -> LogicalPlanBuilder (df-rust §19, df-planning §43) for plan templates
Need to inspect/transform a plan before execution?
  -> ctx.state().create_logical_plan(sql) -> state.optimize(&plan) (df-planning §41, §55)
Need a stable plan-hash for caching?
  -> LogicalPlanArtifacts + sha256 of display_indent_schema        (df-planning §55, §56)
Custom DSL → plan?
  -> Build LogicalPlan directly via LogicalPlanBuilder + Expr      (df-planning §43, §59 catalog)
```

### Implement a calculation (UDF/UDAF/UDWF/UDTF)

```
Expressible with built-in scalar functions or Expr composition?
  -> Use df-rust §11-§12 functions; avoid UDF                     (df-calcs C1)
Element-wise transform/filter over a List/LargeList column?
  -> SQL lambda + array_transform / array_filter / array_any_match (df-calcs C1.19)
  -> Custom HigherOrderUDFImpl only when built-ins can't compose   (df-calcs C1.20, C3.29)
Row-wise deterministic transform?
  -> create_udf or ScalarUDFImpl                                  (df-rust §24; df-calcs C5-C8)
  -> Signature via Signature::exact / coercible / variadic         (df-calcs C5)
  -> volatility = Volatility::Immutable unless really stable/volatile (df-calcs C2)
Needs external I/O (DB lookup, network call)?
  -> async ScalarUDF                                              (df-calcs C12)
  -> Add timeout/retry/circuit-breaker, secret handling, caching  (df-calcs C12)
Group-wise aggregation?
  -> AggregateUDFImpl + Accumulator                               (df-rust §24; df-calcs C13)
  -> Use GroupsAccumulator when grouped + perf-critical          (df-calcs C13)
  -> Make state mergeable for distributed/partial-agg semantics  (df-calcs C13)
Per-row over window/partition/frame?
  -> WindowUDFImpl                                                (df-rust §24; df-calcs C14 catalog)
Relation from literal args (table function)?
  -> TableFunctionImpl returning a TableProvider                  (df-rust §24; df-calcs C15 catalog)
Returns nested types (Struct/List/Map)?
  -> Use return_field_from_args + StructBuilder/ListBuilder       (df-calcs C6, C10)
Needs SciPy/SymPy/native lib?
  -> See df-calcs C11 route matrix:
       Rust crate (preferred) | PyO3 in-process | subprocess | WASM | FFI | Substrait-out
Vectorize the Arrow body, not row-by-row.
  -> ColumnarValue dispatch + downcast + builders                  (df-calcs C8)
  -> Use arrow::compute kernels inside the UDF                    (pa-rust §7)
Null/NaN/inf handling?
  -> Decide null-policy at signature; respect Arrow validity      (df-calcs C7)
Register before planning; volatility must match true semantics.
```

### Define a column type (Arrow Rust)

```
Primitive: Int8/16/32/64, UInt*, Float32/64, Boolean                (pa-rust §3)
String: Utf8 (Utf8View default for new DDL in DF 53+)               (df-rust §7; pa-rust §3)
Binary: Binary / LargeBinary / BinaryView                            (pa-rust §3)
Temporal: Timestamp(unit, tz), Date32/64, Time32/64, Duration       (pa-rust §3)
Decimal: Decimal128(p, s), Decimal256(p, s); max precision = 76     (df-rust §7; pa-rust §3)
Nested:
  - Struct: DataType::Struct(Fields)                                 (pa-rust §3, §5)
  - List: DataType::List(Field) / LargeList / FixedSizeList         (pa-rust §3, §5)
  - Map: DataType::Map(Field, keys_sorted)                          (pa-rust §3, §5)
  - Dictionary: DataType::Dictionary(idx_type, val_type)             (pa-rust §3, §5)
  - RunEnd: DataType::RunEndEncoded(run_end_t, val_t)               (pa-rust §3, §5)
Extension type (semantic annotation):
  -> arrow_schema::extension::ExtensionType + Field metadata        (df-schemas S7; pa-rust §26)
  -> DF-54 session-level registry: ExtensionTypeRegistry            (df-schemas S7.20)
View types (when DF + Arrow upgraded):
  -> Utf8View / BinaryView / ListView / LargeListView                (pa-rust §3)
Always wrap shared schemas as Arc<Schema>; immutable-replace fields.
```

### Choose a read path

```
DataFrame from single Parquet:    ctx.read_parquet("...")           (df-rust §10, §15)
SQL over single Parquet:          ctx.register_parquet + ctx.sql    (df-rust §5, §15)
Async Parquet from object_store:  ParquetRecordBatchStreamBuilder    (pa-rust §12)
Listing/partitioned Parquet:      ListingTable + ListingTableConfig (df-rust §14; pa-rust §13)
CSV:                              ctx.read_csv | arrow_csv::Reader  (df-rust §14; pa-rust §9)
JSON (line-delimited):            ctx.read_json | arrow_json::Reader (df-rust §14; pa-rust §9)
Arrow IPC stream/file:            ctx.read_arrow | arrow_ipc::reader (df-rust §14; pa-rust §10)
Avro:                             arrow-avro + DataFusion AvroExec   (pa-rust §23; df-rust §14)
ORC:                              orc-rust + datafusion-orc          (pa-rust §23)
Remote (S3/GCS/Azure/HTTP):       ctx.runtime_env().register_object_store(...) (df-rust §16)
In-memory RecordBatch:            MemTable::try_new(schema, partitions) + ctx.register_table
                                  OR DataFrame::from_record_batches  (df-rust §3)
Cross-runtime via Arrow C Stream: ctx.read_table on TableProvider that wraps the stream
                                  OR pyo3-arrow `__arrow_c_stream__` consumer (pa-rust §21)
```

### Choose a write path

```
DataFrame → Parquet (single file):     df.write_parquet(...).await          (df-rust §10, §15)
DataFrame → partitioned Parquet:       df.write_parquet(..., options).await (df-rust §9, §15)
DataFrame → CSV / JSON:                df.write_csv(...) / df.write_json(...) (df-rust §10, §14)
DataFrame → Arrow IPC:                 df.write_arrow_ipc(...)               (df-rust §10)
DataFrame → registered table:          ctx.execute("INSERT INTO ... SELECT ...") (df-rust §9)
Outside DataFusion (raw Arrow):        ArrowWriter / AsyncArrowWriter         (pa-rust §11, §12)
Outside DataFusion partitioned:        Manually chunk + write per partition + emit hive paths (pa-rust §13)
IPC stream:                            StreamWriter / FileWriter              (pa-rust §10)
Object-store sink:                     ObjectStore::put_multipart + buffered writer (pa-rust §14)
```

### Choose a plan layer for a change

```
Adding a function / aggregate / window?               -> df-calcs C1-C13; df-rust §24
Changing how SQL parses?                              -> df-rust §25; df-planning §42
Changing how a logical operator is built?             -> df-rust §19; df-planning §43
Changing how a logical plan is rewritten/optimized?   -> df-rust §22; df-planning §48
Adding planner metadata (stats/constraints/FDs)?       -> df-rust §22; df-planning §47
Changing what physical operator is chosen?            -> df-rust §20; df-planning §49
Changing how a join is planned?                        -> df-rust §23; df-planning §52
Adding/altering streaming bounds + pipeline breakers? -> df-rust §21; df-planning §53
Adding partition/scheduling/memory policy?            -> df-rust §28; df-planning §54
Adding a new TableProvider / source / pushdown?       -> df-rust §18; df-planning §51; df-schemas S10
Custom logical or physical node?                       -> df-rust §26
Plan serialization / cache / fingerprint?              -> df-rust §36; df-planning §55-§56
```

### Cross-runtime via Arrow (Rust ↔ Python ↔ other)

```
Producer in Rust, consumer in Python?
  -> Expose Rust side as `__arrow_c_stream__` producer via pyo3-arrow (pa-rust §21)
  -> Python `datafusion.SessionContext.from_arrow(producer)` consumes directly (`docs/library_ref/datafusion.md`, unrouted -- and note SRV §18 and SRV §6 invariant 3 forbid this path in CodeFabric; see §5)
Producer in Python (PyArrow), consumer in Rust?
  -> Use arrow_pyarrow::PyArrowType to ingest pa.Table/pa.RecordBatch (pa-rust §21)
  -> Or accept any `__arrow_c_stream__`-implementing object via PyCapsule
Producer + consumer both in Rust, separate processes?
  -> Arrow IPC stream over a pipe or socket (pa-rust §10)
  -> Arrow Flight if remote (pa-rust §19)
Need a stable, language-neutral plan?
  -> Substrait via datafusion-substrait                            (df-rust §36; pa-rust §25)
Need DB connectivity from Rust?
  -> ADBC drivers (Flight SQL / DataFusion / vendor)               (pa-rust §20)
Never pickle / JSON / serde across the Rust ↔ PyArrow boundary if Arrow C Data path works.
```

### Diagnose a slow query

```
Cost location?               EXPLAIN ANALYZE + MetricsSet           (df-rust §30; df-planning §55)
Plan vs exec?                Compare unoptimized/optimized/physical plans (df-planning §41, §55)
Parquet read slow?           Confirm projection + predicate pushdown + bloom (df-rust §15; pa-rust §12)
Object store slow?           IO pool sizing + retry/backoff                (df-rust §16; pa-rust §14)
UDF slow?                    Move to arrow::compute kernels inside body    (df-calcs C8; pa-rust §7)
                             Or to built-in via Expr composition           (df-calcs C9)
Wrong join algorithm?        Inspect physical plan; tune statistics        (df-rust §23; df-planning §52)
Backpressure / unbounded?    Check pipeline breakers + boundedness          (df-rust §21; df-planning §53)
Memory pressure?             Set memory_pool limit + spill dir              (df-rust §28; df-planning §54)
Too many tiny partitions?    Tune batch_size + repartition                   (df-rust §29)
Repeated similar queries?    Plan-cache via PlanCacheKey + fingerprint      (df-planning §56)
```

### Compare/govern schemas

```
Strict equality including metadata?  Schema::equals_with_options(opts.with_metadata(true)) (df-schemas S5, S14)
Logical equivalence (no metadata)?    matches_arrow_schema / has_equivalent_names_and_types (df-schemas S5)
Multi-file Parquet drift?             Reconcile via SchemaAdapter or unify-then-cast        (df-schemas S3, S10)
Plan-output schema diff?              LogicalPlan::schema() before/after rewrite           (df-schemas S11, S14)
Stable fingerprint for cache key?     sha256(schema.serialize() bytes)                       (df-schemas S14; df-planning §55, §56)
Cross all layers (Arrow ↔ DFSchema ↔ Parquet ↔ ListingTable)?  Schema lifecycle diagram     (df-schemas S1.2)
Evolution policy (added/removed/widened/narrowed)?  Compatibility classifier               (df-schemas S6)
```

### Setting up a Cargo project

```
Minimal binary (SQL CSV demo)?
  -> [dep] datafusion, tokio["rt-multi-thread","macros"]            (df-rust §1, §2)
Library / SDK?
  -> Avoid Arrow version bleed; re-export datafusion::arrow         (df-rust §1)
Workspace with kernels + sources + query + cli?
  -> Multi-crate workspace template                                  (df-rust §1.7-1.10; pa-rust §1)
Python extension (PyO3 + maturin + Arrow)?
  -> pyo3 + pyo3-arrow + arrow + tokio["rt-multi-thread"]            (pa-rust §1-§2, §21)
Server profile?
  -> tokio + tower/axum + arrow-flight + datafusion + object_store   (df-rust §38; pa-rust §1-§2)
Match Arrow version across all crates that depend on Arrow types directly.
```

---

## §4 — Operating rules

1. **Rust DataFusion is a Rust crate; PyArrow-side calls cross the C Data / PyCapsule boundary, not the Python interpreter.** When in doubt about *what* a value is, reach for pa-rust; about *how a query is built*, df-rust + df-planning; about *what schemas mean*, df-schemas; about *how custom math works*, df-calcs. (df-rust §0; pa-rust §0)

2. **DataFrame is lazy; `.collect()` / `.execute_stream()` / `df.show()` / `df.write_*()` are terminal.** Transforms return `DataFrame` (a new lazy node); terminals return `Vec<RecordBatch>` / `SendableRecordBatchStream` / `()`. Bound execution explicitly and avoid `.collect()` on unbounded sources. (df-rust §0.6, §10)

3. **Arrow Rust containers are reference-counted and effectively immutable.** `Arc<Schema>`, `Arc<dyn Array>`, `Arc<RecordBatch>` share cheaply via clone; mutation is *replacement* via builders (`SchemaBuilder`, `StringBuilder`, `ListBuilder`, etc.) that produce new values. Treat schemas and arrays as values, not as mutable state. (pa-rust §3-§5; df-schemas S2)

4. **Nulls are validity bitmaps, not sentinels.** Use `Array.is_null(i)` / `Array.nulls()`. Never encode missingness with `-1`/`NaN`/`""`. UDFs must propagate nulls per declared `Volatility`/null-policy. (pa-rust §4-§5; df-calcs C7)

5. **Rust `Expr` has no operator overloading for booleans/comparisons.** Write `col("a").gt(lit(0)).and(col("b").lt(lit(10)))` — Rust will not let you write `col("a") > lit(0) & col("b") < lit(10)`. The same applies to `eq`/`not_eq`/`gt`/`gt_eq`/`lt`/`lt_eq`/`and`/`or`/`is_null`/`is_not_null`/`between`. (df-rust §11)

6. **Match Arrow versions across every crate that touches Arrow types directly.** `datafusion`, `parquet`, `arrow-flight`, your code — they must agree on the major Arrow version. When in doubt re-export through `datafusion::arrow` to avoid type-mismatch errors at the boundary. (df-rust §1, §34)

7. **`#[tokio::main(flavor = "multi_thread")]` is the canonical entry point.** DataFusion is async end-to-end above `LogicalPlan`. Spawning DataFusion work from a `current_thread` runtime leads to executor stalls on blocking IO. (df-rust §1, §3)

8. **Streaming vs materializing has the same dichotomy as the Python side.** `df.collect()` materializes to `Vec<RecordBatch>` — fine for bounded preview, dangerous for unbounded. `df.execute_stream().await?` returns `SendableRecordBatchStream` — use for writes, transport, long results. (df-rust §10, §21)

9. **Cross-runtime exchange uses Arrow C Data / C Stream / PyCapsule, not pickle / JSON / pandas DataFrame.** Implement `__arrow_c_stream__` / `__arrow_c_array__` / `__arrow_c_schema__` on the producer; the consumer (DataFusion Python, Rust via `pyo3-arrow`, any Arrow runtime) takes it zero-copy. (pa-rust §21; df-rust §37.4)

10. **Schema is a contract artifact, not a derived value.** Fingerprint with `sha256(schema.serialize() bytes)`. Declare schemas at source registration, validate in CI, and treat `Field.metadata` as governance/provenance (never as application state). The schema lifecycle diagram in df-schemas S1.2 is the canonical model. (df-schemas S1, S7, S14; df-planning §55)

11. **`DFSchema` vs Arrow `Schema` is the most common confusion.** Plans / `Expr` / SQL binding live on `DFSchema` (Arrow schema + relation qualifiers + ambiguity rules). Runtime `RecordBatch` carries `Arc<Schema>` (Arrow only). They convert in both directions but they are *not* the same type, and the qualifier is lost crossing one way. (df-rust §4; df-schemas S1)

12. **Plan-hash snapshots before any optimizer-affecting change.** Capture unoptimized + analyzed + optimized `LogicalPlan` (and the physical plan when relevant) via `LogicalPlanArtifacts` and diff on upgrades / rule changes / function-registry changes / stats refreshes. Plan-cache keys include all of these dimensions. (df-planning §55-§56)

13. **Prefer built-ins → `Expr` composition → vectorized UDF → external integration, in that order.** Reach for PyO3 / WASM / subprocess only when neither built-in nor `arrow::compute` covers the algorithm, and document the choice in the route manifest. (df-calcs C1, C11)

14. **`Volatility` is execution-time semantics, not documentation.** `Volatility::Immutable` enables constant folding and CSE; `Stable` enables some folding within a query; `Volatile` blocks all. Lying here causes correctness bugs in the optimizer. (df-rust §24; df-calcs C2)

15. **UDAF state must agree between `state()` and `merge_batch()`** in field order, types, and nullability — otherwise distributed/partial aggregation silently corrupts. Snapshot the state schema in C2's `CalculationSpec` and assert in tests. (df-calcs C2, C13)

16. **Use `ListingTable` for multi-file/partitioned datasets in DataFusion; never glob manually and `UNION ALL`.** Naïve glob defeats partition pruning, predicate pushdown, column-stat-driven row-group pruning, and bloom filters. (df-rust §14; pa-rust §13)

17. **`object_store` is the canonical filesystem abstraction.** Register stores on the `RuntimeEnv` once; resolve URLs (`s3://`/`gs://`/`az://`/`file://`/`http://`) through it. Do not handcraft AWS/GCS/Azure HTTP calls. (df-rust §16; pa-rust §14)

18. **Tokio + Arrow + DataFusion all carry public types in their crate version.** A `RecordBatch` from `arrow 58.x` is not interchangeable with `RecordBatch` from `arrow 57.x` even though field structure looks identical. Either match versions exactly or convert at the boundary using IPC / C Data. (df-rust §1, §34)

19. **Catalog-only sections (df-planning §57-§60, df-calcs C14-C26, pa-rust §29-§30) are deliberately catalog-only.** The catalog block describes the surface; read it for *what* exists and *how it relates to neighboring deep-dives* and do not search for a missing deep-dive section. If you need a deep-dive on one of those catalog topics, derive from the adjacent deep-dive (e.g., for df-calcs C14 window-UDF-frame, use C13 UDAF state design + df-rust §24 UDWF authoring + df-rust §12 built-in window-function catalog).

20. **`# DataFusion Advanced — N)` prefix is shared by datafusion_rust, datafusion_planning_rust, datafusion_schemas_rust, datafusion_calculations_rust.** A grep for that prefix returns 41 + 16 + 15 + 13 = 85 hits across four files. **Always disambiguate by file path before reading**, and remember that pyarrow_rust deep-dives use `# N) Title` (no "DataFusion Advanced —" prefix). When `grep`-ing for a section header, scope the search to one file at a time.

21. **`information_schema` is product surface, not infrastructure.** Catalog/function/UDF discovery exposed there is part of your tenant-facing contract and must be gated by authorization rules. (df-rust §17; df-schemas S9, S15; df-calcs C3, C17)

22. **Custom `TableProvider::scan` must return an `ExecutionPlan` whose stream schema matches `schema()` after projection.** This is the single most common runtime error in custom sources. Use a `SchemaAdapter` to bridge declared vs file-physical schema differences. (df-rust §18; df-schemas S10)

---

## §5 — Project context: CodeFabric

**Pre-implementation.** There is no DataFusion or Arrow code in this repository yet. The shape
is one Cargo package and one library crate, with a seed whose only job was to prove the
toolchain; `RM W0` plans the four build domains (stable Rust daemon/data plane, nightly rustc
extractor, Pyrefly sidecar, Python FastMCP adapter) and `RM W3` is the first wave that puts real
Arrow, Delta, and DataFusion code on disk.

Consequence for using these five documents: read them **against the specifications**, not against
existing code. The mapping that matters is spec section → chapter, not crate → chapter.

### Version pins come from the spec, not a lockfile

`FAB §2.1 Canonical workspace baseline` is the normative source:

* `datafusion = "=54.1.0"`;
* `arrow`, `arrow-array`, `-buffer`, `-schema`, `-cast`, `-select`, `-ord`, `-string`, `-row` and
  `parquet` all at `=58.4.0` (`parquet` with features `arrow`, `async`, `object_store`);
* `object_store = "=0.13.2"`;
* `deltalake` at git rev `9f9223197469897ef05ae4369eb4fd1390174e65`, `default-features = false`,
  features `rustls`, `datafusion` — **a pinned pre-release revision**, so `FAB §2` requires
  compile-testing generated code against that exact rev before adoption. Object-store backends are
  feature-gated (`s3-storage`), not default;
* `edition = "2024"`, `rust-version = "1.94.1"`, `resolver = "3"`;
* utility crates `tokio`, `futures`, `url`, `tracing`, `serde`, `serde_json`, `blake3`.

`FAB §2.2` states the alignment invariant: one Arrow major/minor family, one matching Parquet
family, one DataFusion family, one `object_store` family, one pinned delta-rs revision — and CI
SHALL reject duplicate versions that cross public type boundaries. `RM W0` work package 1 is
where that CI check is built.

These pins move. They were `58.3.0` / `54.0.0` until recently. Read `FAB §2.1` rather than
trusting a number quoted here or in the skill description.

**No `datafusion_54vs53.md` exists in this repository.** Earlier revisions of this skill cited it
as the DF 53→54 migration catalog; per-topic DF-54 behaviour is covered inside the five documents
themselves — see §2 "DataFusion 54 additions" above.

### Spec section → reference-document map

Full table in `docs/spec_index/library-routing.md` §3. The load-bearing rows:

| Spec section | Role | Read |
|---|---|---|
| `FAB §7` canonical physical types and identity · `FAB §10`-`§11` schema metadata conventions and schema registry | The `TableSpec` contract every other table depends on | **df-schemas** S1-S15 · **pa-rust** §3 (data model) / §26 (extension types) · Rule 10 (schema as contract) |
| `FAB §13` control-plane schemas · `FAB §91` snapshot-pinned overlay-aware catalog provider · `FAB §92`-`§93` stable serving views and table functions | Catalog, `TableProvider`, the 23 `cpg_serving.*` views, four table functions | **df-rust** §3 (sessions) / §17 (catalogs) / §18 (`TableProvider`) / §24 (UDTF) · **df-planning** §51 |
| `FAB §63`-`§66` provider-observation to Arrow, batch size, builder policy, batch validation | The ingestion boundary — provider DTOs become `RecordBatch` here | **pa-rust** §5 (arrays/builders) / §6 (`RecordBatch`) / §10 (IPC) · **df-rust** §4 |
| `FAB §77` Arrow kernel catalog · `FAB §78`-`§79` scalar and aggregate UDFs · `FAB §79A` derivation registry and single-authority matrix | Which engine owns which derived fact | **pa-rust** §7/§8 (kernels) · **df-rust** §24 · **df-calcs** C1-C3, esp. C1.7 multi-axis placement matrix and C1.12 calculation manifest |
| `FAB §80`-`§82` relationally expressible derived facts, custom logical operators, custom physical graph representation | `CpgGraphTraverse`, `CpgStrongComponents`, `CpgDominators`, … | **df-rust** §19 (logical plans) / §26 (custom operators) · **pa-rust** §4 (buffers) — the physical graph is **Arrow CSR**, not petgraph |
| `FAB §83`-`§90` reachability, SCC, dominators, control dependence, reaching definitions, points-to, summary fixed point, execution requirements | Memory, spill, cancellation, streaming, `PlanProperties` obligations | **df-rust** §20 (physical plans) / §21 (streaming) / §28 (memory and spilling) · **df-planning** §41-§56 |
| `FAB §94`-`§98` query-planning policy, partitioning, Z-order, Parquet writer policy, runtime policy | Layout and runtime tuning | **df-rust** §22 (optimizer) / §27 (configuration) / §28 · **pa-rust** §11-§12 (Parquet) |
| `QRY AC-G-46` typed internal `PlanSpec` · `AC-G-52` cost model and hard limits · `AC-G-57` plan cache contract | The query compiler's lowering target | **df-rust** §19/§22/§28 · **df-planning** |

### Boundary discipline

Three boundaries the specifications draw, all of which constrain this stack:

**Petgraph does not cross into the fabric.** `FAB §82` is explicit that the custom physical graph
representation is CSR over Arrow buffers; petgraph belongs to fact generation (`GEN §52 Petgraph
role`, routed by the `petgraph-ref` sibling). A `petgraph::graph::DiGraph` must not appear in a
`TableProvider`, an `ExecutionPlan`, or any type on the fabric's public surface.

**The adapter process holds no Arrow.** `SRV §18 Framework and package posture` states the Python
FastMCP adapter requires neither Arrow nor DataFusion — it is Pydantic, pydantic-settings, a gRPC
client, and orjson. `SRV §6` invariant 3 makes it a hard system invariant: *"Python never becomes
an Arrow/DataFusion processing layer."* There is therefore **no PyCapsule / `pyo3-arrow` data path
in this architecture**; the Rust↔Python boundary is Protobuf over a Unix domain socket
(`SRV §8`-`§9`), not zero-copy Arrow. Chapters **pa-rust §21** (PyO3 interop), **§19**-**§20**
(Flight, ADBC) and **§25** (Substrait) have no consumer in the design suite — read them only if a
future decision creates one.

**No provider writes a canonical row.** Reconciliation is `FAB AC-G-37`, fed by `GEN` Part VII
(§80-§85). Arrow batches arriving from a provider are *observations*; turning them into canonical
`entity`/`relation`/`property_fact` rows is the reconciliation engine's job, not the ingestion
path's.

**Rule of thumb:** editing a `.rs` file, a `Cargo.toml`, or anything on the fabric's Arrow /
DataFusion surface → this skill. Delta tables, `_delta_log`, publication and vacuum →
`deltalake-rust-ref`. Graph algorithms during fact generation → `petgraph-ref`. Watching the
filesystem or reading Git state → `gix-notify-ref`.
