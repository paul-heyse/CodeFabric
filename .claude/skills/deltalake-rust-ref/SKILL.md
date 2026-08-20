---
name: deltalake-rust-ref
description: "Reference navigator for the Rust `deltalake` crate (delta-rs 1.0.0 @ git rev 35cfed45, pre-release pin) on DataFusion 54 + Arrow 58.3. Routes one deep-dive at docs/library_ref/deltalake_rust.md (17,021 lines, 13 chapters, §0 then §2-§13 — no §1): version/feature/compatibility baseline + feature-flag matrix + API stability zones (§0); deployment, cloud object stores (S3/MinIO/R2/Azure/GCS), TLS, storage options, CI (§2); table loading, snapshots, versions, time travel, freshness (§3); Delta↔Arrow schema mapping, types, metadata governance (§4); writing from Arrow batches + DataFusion plans, save/schema modes, replaceWhere, idempotency (§5); reading/querying through DataFusion, TableProvider, pushdown, DeltaScanConfig (§6); exhaustive DataFusion+Arrow integration track — sessions, expressions, batch interop, plan writes, pruning (§7); create-table workflows + Parquet conversion (§8); DML delete/update/merge, upsert/SCD/CDC, conflict + retry (§9); Change Data Feed incremental consumption (§10); constraints, table properties, protocol governance (§11); partitioning, layout, file skipping (§12); optimize, Z-order, vacuum, restore, maintenance (§13). Use when Rust touches `use deltalake::`, `open_table`, `DeltaTable`, `DeltaTableBuilder`, `DeltaOps`, `CreateBuilder`, `WriteBuilder`/`MergeBuilder`/`DeleteBuilder`/`UpdateBuilder`, `OptimizeBuilder`/`VacuumBuilder`/`RestoreBuilder`, `scan_cdf`/`CdfLoadBuilder`, `DeltaScanConfig`/`DeltaSessionContext`/`update_datafusion_session`, `SaveMode`, storage-options maps for `s3://`/`az://`/`gs://` Delta URIs, `_delta_log` semantics, edits `crates/smartref_delta/`, or edits `Cargo.toml` pins for `deltalake`/`delta_kernel`. Core Rust DataFusion/Arrow APIs → sibling `datafusion-pyarrow-rust-ref`; Python `deltalake` package → `datafusion-pyarrow-ref` territory (`docs/library_ref/deltalake.md`)."
allowed-tools: Read, Grep, Glob, Bash
---

# Delta Lake Rust Reference Navigator

Routes one deep-dive reference for the **Rust `deltalake` crate** (delta-rs): `docs/library_ref/deltalake_rust.md`. This SKILL.md is the full map: version anchors, document organization, the per-chapter section index with line numbers, the cross-chapter topic matrix, decision trees, operating rules, and the smartref project context.

## Version anchor

All guidance assumes:

* **`deltalake` 1.0.0 @ git rev `35cfed4545f41c2f483706d29670f7cc2fe7e217`** (pre-release pin on the 1.0.0 line — no formal release notes yet; the checkout's `CHANGELOG.md` is the record, and a formal 1.0.0 release may add changes). The 1.0.0 line is a **semver/stability milestone**: the public API surface the doc covers is unchanged-or-additive vs 0.32.x. Default feature set is **`rustls` only**; everything else (DataFusion, cloud stores, catalogs) is opt-in. The kernel dependency is `delta_kernel` packaged as **`buoyant_kernel` 0.24.0** (git rev `393fbf6…`, features `arrow-58` + `internal-api`).
* **delta-rs workspace baseline (at the pinned rev)**: `rust-version = 1.91.1`, `edition = 2024`, `arrow`/`parquet` **58** (resolved 58.3.0), `datafusion` **54.0.0** (+ `datafusion-datasource`, `-physical-expr-adapter`, `-ffi`, `-proto`), `object_store` **0.13.2**, `tokio` 1. DataFusion and Arrow majors are **not** aligned by design — DF 54.x tracks Arrow 58.x. sqlparser is dual in the tree: 0.61 via delta-core, 0.62 via DF 54. The DF 53→54 substrate change catalog lives at `docs/library_ref/datafusion_54vs53.md`.
* **Async-first**: every load/write/operation is `async` under Tokio (`rt-multi-thread`, `macros`).
* Stack contract: `table URL + storage options → DeltaTableBuilder/open_table → DeltaTable (pinned snapshot of _delta_log state) → {Arrow batch write builders | DataFusion TableProvider registration → SQL/DataFrame | operation builders (create/delete/update/merge/optimize/vacuum/restore/CDF)} → committed Delta version`.

If a snippet uses `DeltaOps(table).write(...)` returning a metrics tuple, pre-0.32 provider internals, `DeltaTableProvider` (removed on the 1.0.0 line), the deprecated `DeltaPhysicalCodec`, or DataFusion ≤53 APIs, treat it as **legacy** and verify against the baseline before adopting.

### Scope

This skill covers the Rust `deltalake` crate surface end-to-end: dependency/feature selection, deployment, table lifecycle, schema governance, reads, writes, DML, CDF, constraints, layout, and maintenance. It does **not** cover:

* Core Rust DataFusion/Arrow APIs in their own right (`SessionContext`, `LogicalPlan`, `Expr`, `RecordBatch`, UDFs, `object_store` internals) — see sibling **`datafusion-pyarrow-rust-ref`** (`datafusion_rust.md`, `datafusion_planning_rust.md`, `datafusion_schemas_rust.md`, `datafusion_calculations_rust.md`, `pyarrow_rust.md`). They appear here only as the integration substrate.
* The **Python** `deltalake` package — `docs/library_ref/deltalake.md`; Python-side DataFusion/PyArrow → sibling **`datafusion-pyarrow-ref`**.
* Adjacent older Delta×DataFusion notes at `docs/library_ref/` (`datafusion_deltalake_advanced_rust_integration.md`, `Datafusion_Deltalake_Constructors_Deepdive.md`, `DataFusion_Delta_Builder_Objects_Glossary_v2.md`, `deltalake_datafusion_integration.md`, `deltalake_datafusionmixins.md`, `datafusion_delta_cache_integration.md`, `datafusion_deltalake_rule_enforcement.md`) — historical/supplementary; `deltalake_rust.md` is the authoritative reference, anchored to the 1.0.0 pinned rev, and wins on conflict.

---

## How the reference document is organized

`docs/library_ref/deltalake_rust.md` is 17,021 lines, organized as 13 chapter-level sections. The chapter sequence is **§0, §2, §3, … §13** — **§1 does not exist** (numbering skips from §0 to §2; never cite "deltalake_rust §1"). Chapters are H1 `# N. <title> — Rust deltalake`-style; subsections are H2 `## N.M <title>`; third level is H3 `### N.M.K`.

**Heading anomalies — disambiguate before grepping:**

1. **§7 uses H1 for its subchapters** (`# 7.0` … `# 7.13`) and H2 for its third level (`## 7.1.1` …). A grep for `^# ` returns §7's subchapters interleaved with real chapters.
2. **Code-block comments masquerade as H1s** in bare greps: lines 17/32/66/243/250 (`# deltalake 1.0.0 @ rev 35cfed45 …`, `# Delta kernel: packaged as buoyant_kernel …`, `# rust-toolchain.toml`, `# Cargo.toml`) and 1025-1040 (`# AWS S3`, `# Azure ADLS …`, `# Google Cloud Storage`, …) are TOML/text-block comments, not headings. Scope chapter greps to `^# [0-9]+\.` and subsection greps to `^## [0-9]+\.[0-9]+ `.

Most chapters follow a recurring internal shape:

| Subsection family | Typical labels |
|-------------------|----------------|
| Mental model | §N.1 (§7.0 for chapter 7) |
| Cargo baseline for the chapter | §N.2 |
| Numbered topical deep-dives with compilable Rust | §N.3 … |
| Error taxonomy / diagnostics | late §N.x |
| Testing matrix | late §N.x |
| Best practices → Anti-patterns → LLM-agent checklist → Value case | final four subsections |

Each chapter closes with reference-style citation links (`[1]: https://docs.rs/...`). The document ends with a **SMARTREF Command Boundary** callout (line 17001) — all smartref Delta mutations flow through `DeltaOperationCommand`/`DeltaOperationReceipt`; direct builder helpers are adapter internals.

**Reading strategy.** Chapters run ~800-1,600 lines. Use the line ranges in the section index below to load only what you need: `Read(file, offset=N, limit=M)`. Before drafting code for a chapter's topic, load that chapter's closing four subsections (anti-patterns + checklist) — they encode the version-specific traps. The per-chapter `§N.2 Cargo baseline` repeats the exact dependency block needed for that chapter's snippets.

---

## deltalake_rust.md — full section index

Line numbers are chapter-start lines in `docs/library_ref/deltalake_rust.md` (17,021 total).

| § | Lines | Title | Key subsections / agent value |
|---|-------|-------|-------------------------------|
| **0** | 1-902 | Version, feature, and compatibility baseline | §0.1 canonical 1.0.0 pinned-rev anchor (rust 1.91.1 / edition 2024 / arrow 58.3 / df 54.0.0 / object_store 0.13.2 / buoyant_kernel 0.24), §0.2 canonical Cargo.toml profiles (local-Arrow / +DataFusion / +S3 / multi-cloud), §0.3 `deltalake` wrapper vs `deltalake-core`, §0.4 exact-pin policy (git rev pins), §0.5 edition+MSRV contract, §0.6 **feature-flag matrix** (34 wrapper flags; `rustls` sole default; core-crate map), §0.7 feature recipes 0.7.1-0.7.8 (local/DF/S3/native-TLS/Azure/GCS/Glue/Unity), §0.8 Cargo.lock + skew controls, §0.9 **API stability risk zones** (DF integration surface churn, `DeltaOps` deprecation, `DeltaTableProvider` removal + codec deprecation, protocol feature table, nanosecond timestamps, variant), §0.10 DF+Arrow alignment matrix (54/58.3, sqlparser duality), §0.11 doc test harness, §0.12 version banner, §0.13 pre-code checklist, §0.14 value case, §0.15 **1.0.0 identity** (semver milestone, git-pin status, kernel = buoyant_kernel 0.24) |
| **2** | 903-2238 | Deployment and project setup | §2.1 deployment mental model, §2.2 cargo install profiles, §2.3 **Tokio runtime contract**, §2.4 URL construction + table loading (local dir vs `s3://`), §2.5 storage-options map design, §2.6 **S3** (feature gate, URL schemes, credentials, safe concurrent writes/locking, unsafe-rename, IAM minimum), §2.7 MinIO/R2/LocalStack (conditional-put, DynamoDB locking), §2.8 Azure (gate/schemes/options), §2.9 GCS, §2.10 TLS selection (rustls vs native), §2.11 DataFusion runtime config, §2.12 Docker, §2.13 env-var configuration, §2.14 CI fixtures, §2.15 production pinning + drift detection, §2.16 deployment config object, §2.17 IAM + secret handling, §2.18 logging/observability, §2.19-2.22 tail, §2.23 OpenDAL storage backends (new at the 1.0.0 rev) |
| **3** | 2239-3736 | Table loading, snapshots, state, and time travel | §3.1 snapshot mental model, §3.3 open latest (`open_table`), §3.4 builder-based loading (`DeltaTableBuilder`), §3.5 loaded vs uninitialized table, §3.6 version inspection, §3.7 refreshing state, §3.8 **load specific version**, §3.9 **time travel by timestamp**, §3.10 snapshot inspection, §3.11 history, §3.12 active file URIs, §3.13 add actions as Arrow batches, §3.14 metadata/protocol validation gate, §3.15 version pinning for reproducibility, §3.16 snapshot caching, §3.17 open-once vs open-per-query, §3.18 avoiding stale state, §3.19 DataFusion implications, §3.20 service registry pattern, §3.21 error taxonomy, §3.22 testing matrix, §3.23-3.26 tail, §3.27 BlindDeltaTable stats-free loading (new at the 1.0.0 rev) |
| **4** | 3737-5139 | Schema, Arrow type mapping, and metadata governance | §4.1 Delta schema mental model, §4.3 canonical Delta schema construction, §4.4 **primitive type catalog**, §4.5 create-from-schema, §4.6 **Arrow schema boundary** (Delta `StructType` ↔ Arrow `Schema`), §4.7 Arrow→Delta validation posture, §4.8 nullability, §4.9-4.10 field metadata, §4.11 table-metadata update, §4.12 add columns, §4.13 enforcement during writes, §4.14 evolution during writes, §4.15 cast safety, §4.16 partition columns, §4.17 decimals, §4.18 **timestamps** (µs canonical; ns experimental), §4.19 binary, §4.20-4.22 structs/lists/maps, §4.23 variant, §4.24 Arrow extension metadata, §4.25 column mapping, §4.26 type widening, §4.27 cross-engine compat matrix, §4.28 inspect loaded schema, §4.29 schema contract object, §4.30 validation helper, §4.31 golden fixtures, §4.32 governance runbook, §4.33-4.36 tail |
| **5** | 5140-6553 | Writing data from Arrow and DataFusion | §5.1 write-path mental model, §5.3 **current API: `DeltaTable::write(batches)` → `WriteBuilder` → `DeltaTable`** (no metrics tuple at the 1.0.0 rev), §5.4 save modes, §5.5 schema modes (enforce/merge/overwrite), §5.6 cast safety, §5.7 partitioned writes, §5.8 **`replaceWhere` predicate overwrite**, §5.9 writing from DataFusion `LogicalPlan`, §5.10 session fallback policy, §5.11 target file size + batch sizing, §5.12 Parquet writer properties, §5.13 commit properties/metadata, §5.14 custom execute handler, §5.15 created-table config during write, §5.16 **idempotent writes** (app-txn patterns), §5.17 retry safety + atomic commit, §5.18 small-file avoidance, §5.19 runtime/object-store wiring before write, §5.20-5.21 end-to-end write functions (batch + plan), §5.22 pre-write validation, §5.23 error taxonomy, §5.24 testing matrix, §5.25-5.28 tail |
| **6** | 6554-7775 | Reading and querying through DataFusion | §6.1 mental model, §6.3 minimal SQL path, §6.4 minimal DataFrame path, §6.5 **`DeltaTable` as `TableProvider`**, §6.6-6.7 explicit/multi-table registration, §6.8-6.9 SQL vs DataFrame paths, §6.10 predicate pushdown, §6.11 projection pushdown, §6.12 partition pruning, §6.13 file/data skipping, §6.14 **`DeltaScanConfig`**, §6.15 provider with file column, §6.16 `DeltaSessionContext`, §6.17 `DeltaRuntimeEnvBuilder`, §6.18 spill config, §6.19 physical-plan diagnostics, §6.20-6.21 joins with `MemTable`/Parquet/CSV providers, §6.22 **object-store registration + duplicate avoidance**, §6.23 alias/catalog naming, §6.24 freshness + provider lifecycle, §6.25 querying pinned versions, §6.26 case sensitivity, §6.27 query-failure diagnostics table, §6.28 metrics, §6.29 security, §6.30 testing matrix, §6.31-6.34 tail, §6.35 next-scan FileSelection/MissingSelectedFilePolicy (new at the 1.0.0 rev) |
| **7** | 7776-9339 | DataFusion + Arrow integration track (exhaustive; **H1 subchapters 7.0-7.13**) | §7.0 integration mental model, §7.1 TableProvider integration (7.1.1-7.1.6: minimal registration, `TableProviderBuilder`, construction cost, **snapshot binding + freshness**, naming, multi-tenant), §7.2 session/runtime (7.2.1-7.2.7: `SessionContext`/`SessionState`/`RuntimeEnv`, **correct `update_datafusion_session`** — idempotent, never overwrites, accidental-override avoidance, custom memory/spill runtime, `DeltaSessionContext`, many-roots), §7.3 SQL path (pushdown-friendly SQL, `EXPLAIN`/`ANALYZE`, compat limits, identifier escaping, UDFs over Delta), §7.4 DataFrame path (`read_table`, safe expression generation from UI/config, compile-time construction), §7.5 **expressions inside Delta operations** (delete/replace-where/update/merge predicates, null + type semantics), §7.6 Arrow batch interop (nulls, decimals, timestamps, dictionary arrays, chunking, **version-skew avoidance**), §7.7 writing from DF plans (query-result→Delta, why `SessionState` matters, CTAS-like materialization, error boundaries), §7.8 file skipping + perf (scan config, projection/partition pruning, stats, small-file effects, target partitions, runtime metrics, benchmark harness), §7.9 end-to-end service skeleton, §7.10-7.13 tail |
| **8** | 9340-10596 | Create-table workflows | §8.1 creation mental model, §8.2 primary construction APIs (`CreateBuilder`), §8.3 empty table, §8.4 create with schema, §8.5 column metadata governance, §8.6 table metadata, §8.7 partitioned creation, §8.8 **table properties at creation**, §8.9 storage options during creation, §8.10 save modes / idempotent creation, §8.11 create-or-replace, §8.12 **convert existing Parquet to Delta**, §8.13 bootstrap from Arrow batches, §8.14 bootstrap from DataFusion query results, §8.15 local/dev/prod differences, §8.16 init idempotency, §8.17 commit properties, §8.18 low-level actions, §8.19 post-creation validation, §8.20 testing matrix, §8.21-8.24 tail, §8.25 kernel-owned checkpoints: add-struct nullability + cross-version reader compat |
| **9** | 10597-11977 | DML: delete, update, and merge | §9.1 DML mental model (rewrite-based, transactional), §9.3 expression imports, §9.4-9.6 **delete** (predicate, all-rows, metrics), §9.7-9.8 **update** (predicate + assignments, metrics), §9.9 safe cast, §9.10 **merge** source→target, §9.11 clause families, §9.12 **clause ordering**, §9.13 source/target aliases, §9.14 upsert pattern, §9.15 SCD patterns, §9.16 CDC ingestion, §9.17 GDPR/right-to-delete, §9.18 merge metrics, §9.19 session-state injection for DML, §9.20 commit properties, §9.21 writer properties for rewrites, §9.22 **conflict detection + retry posture**, §9.23 idempotent merge design, §9.24 duplicate-match avoidance, §9.25 predicate determinism, §9.26 file rewrite behavior, §9.27 append-only + column-mapping limitations, §9.28 DF expression generation, §9.29 production DML service wrapper, §9.30 diagnostics + errors, §9.31 testing matrix, §9.32-9.35 tail |
| **10** | 11978-13062 | Change Data Feed (incremental consumption) | §10.1 CDF mental model, §10.3 enable at creation, §10.4 enable on existing tables, §10.5 **`scan_cdf` API**, §10.6 version-range semantics, §10.7 timestamp-range semantics, §10.8 out-of-range behavior, §10.9 **CDF schema** (`_change_type`/`_commit_version`/`_commit_timestamp`), §10.10 change-type semantics, §10.11 executing CDF as a DF physical plan, §10.12 filtering output, §10.13 **incremental consumer checkpoint**, §10.14 polling loop skeleton, §10.15 incremental downstream materialization, §10.16 simulation-state event sourcing, §10.17 CDC interop, §10.18 **retention/vacuum/history availability**, §10.19 schema evolution + compatibility, §10.20 initial-snapshot + CDF catch-up, §10.21 CDF→downstream Delta, §10.22 validation queries, §10.23 errors, §10.24 testing matrix, §10.25-10.28 tail |
| **11** | 13063-14230 | Constraints, properties, and governance | §11.1 governance mental model, §11.3 **CHECK constraints**, §11.4 naming policy, §11.5 violation behavior, §11.6 drop constraints, §11.7 NOT NULL, §11.8 **table properties**, §11.9 typed property keys, §11.10 append-only tables, §11.11 CDF property governance, §11.12 **protocol inspection** (reader/writer versions), §11.13 feature compatibility across engines, §11.14 add table features, §11.15 metadata governance, §11.16 schema governance, §11.17 domain-specific simulation invariants, §11.18 constraint test harness, §11.19 property migration across versions, §11.20 **governance guard before writes/DML**, §11.21 errors, §11.22 testing matrix, §11.23-11.26 tail, §11.27 strict table-feature validation (declared-but-unsupported features fail the operation) |
| **12** | 14231-15671 | Partitioning, layout, and file skipping | §12.1 physical-layout mental model, §12.3 partition columns, §12.4 low-cardinality partitioning, §12.5 **high-cardinality anti-patterns**, §12.6 partition overwrite vs predicate overwrite, §12.7 pre-validating `replaceWhere` input, §12.8 partition filters, §12.9 file statistics, §12.10 min/max skipping, §12.11 DF predicate pushdown, §12.12 stats-loading pitfalls, §12.13 data-skipping table properties, §12.14 **layout strategy for simulation outputs**, §12.15 scenario/date/run/unit decision table, §12.16 partition evolution risk, §12.17 partition compaction, §12.18 file-size strategy, §12.19 query-pattern-driven design, §12.20 add-actions layout report, §12.21 query-plan validation, §12.22 layout policy object, §12.23 write wrapper with layout policy, §12.24 small-file detector, §12.25 partition-scoped compaction trigger, §12.26 strategy by table type, §12.27 errors, §12.28 testing matrix, §12.29-12.32 tail, §12.33 selective stats materialization (behavior note, 1.0.0 rev) |
| **13** | 15672-17021 | Optimize, compaction, Z-order, and vacuum | §13.1 maintenance mental model, §13.3 small-file problem, §13.4 **optimize compaction**, §13.5 target file sizes, §13.6 **Z-order clustering**, §13.7 partition-scoped optimize, §13.8 optimize with DF session state, §13.9 Parquet writer props for optimize, §13.10 optimize-metrics interpretation, §13.11 scheduling, §13.12 maintenance policy object, §13.13 **vacuum dry run**, §13.14 vacuum execute, §13.15 keep-versions, §13.16 lite vs full vacuum, §13.17 vacuum metrics, §13.18 **time-travel breakage after vacuum**, §13.19 restore before/after maintenance, §13.20 filesystem check, §13.21 operational runbooks, §13.22 end-to-end maintenance function, §13.23 policies by table class, §13.24-13.25 **safety checks before optimize/vacuum**, §13.26 metrics/observability, §13.27 errors, §13.28 testing matrix, §13.29-13.32 tail; closes with the **SMARTREF Command Boundary** callout (line 17001) |

---

## Cross-chapter topic matrix

When a topic spans chapters, this is the routing table. Legend: ✅ authoritative, 🔁 cross-cut.

| Topic | Authoritative | Cross-references |
|-------|---------------|------------------|
| Cargo deps, features, pins, MSRV | **§0** ✅ | §2.2 (deployment profiles), every chapter's §N.2 Cargo baseline 🔁 |
| Cloud object stores, credentials, TLS, URLs | **§2** ✅ | §6.22 (registration duplicates), §7.2.2-7.2.3 (session store mapping), §8.9 (creation-time options) |
| Snapshot model, versions, freshness, time travel | **§3** ✅ | §6.24-6.25 (provider lifecycle / pinned-version queries), §7.1.4 (provider snapshot binding), §13.18-13.19 (vacuum/restore interplay) |
| Delta↔Arrow schema, types, evolution | **§4** ✅ | §5.5-5.6 (write-time schema modes + casts), §8.4-8.5 (creation), §10.19 (CDF evolution), §11.15-11.16 (governance) |
| Writing Arrow batches | **§5** ✅ | §7.6 (batch construction + skew), §8.13 (bootstrap) |
| Writing DataFusion plans / CTAS | **§5.9, §5.21** ✅ | §7.7 (plan-write deep-dive), §8.14 (bootstrap from query) |
| Querying via DataFusion (SQL/DataFrame) | **§6** ✅ | §7.3-7.4 (deep-dive), §10.11 (CDF physical plan) |
| `SessionContext`/`RuntimeEnv`/store registration | **§7.2** ✅ | §2.11, §5.10/§5.19, §6.16-6.18, §9.19, §13.8 🔁 |
| DataFusion `Expr` predicates in operations | **§7.5** ✅ | §5.8 (replaceWhere), §9.4/9.7/9.10 (DML usage), §9.28 (generation), §12.7 (validation) |
| Merge / upsert / CDC / SCD | **§9.10-9.18** ✅ | §10.17 (CDF as CDC source), §9.22-9.25 (correctness discipline) |
| CDF | **§10** ✅ | §11.11 (property governance), §10.18 ↔ §13.13-13.18 (retention vs vacuum) |
| Constraints, properties, protocol features | **§11** ✅ | §8.8 (creation-time properties), §12.13 (skipping properties), §0.9.3 (protocol stability) |
| Partitioning + file skipping | **§12** ✅ | §5.7 (partitioned writes), §6.12-6.13 (query-side pruning), §7.8 (perf track), §8.7 (creation) |
| Optimize / vacuum / restore | **§13** ✅ | §12.17/12.24-12.25 (compaction triggers), §5.11/5.18 (write-side prevention) |
| Idempotency + retries | **§5.16-5.17** ✅ | §8.10/8.16 (creation), §9.22-9.23 (DML), §10.13 (consumer checkpoint) |
| Error taxonomies | per-chapter 🔁 | §3.21, §5.23, §6.27, §9.30, §10.23, §11.21, §12.27, §13.27 |

---

## Decision trees

### "Which chapter do I open first?"

```text
Setting up deps / picking features / verifying versions ........... §0
Configuring storage, cloud, TLS, env, CI, Docker .................. §2
Opening a table / versions / time travel / freshness .............. §3
Defining or validating a schema / type mapping .................... §4
Writing data (batches or plans) ................................... §5  (+§7.6/§7.7)
Querying (SQL / DataFrame / joins / pushdown) ..................... §6  (+§7.3/§7.4/§7.8)
Session wiring, expressions, batch interop deep-dive .............. §7
Creating tables / converting Parquet / bootstrap .................. §8
delete / update / merge / upsert / CDC ............................ §9  (+§7.5)
Incremental change consumption .................................... §10
Constraints / properties / protocol governance .................... §11
Partition + layout design ......................................... §12
Compaction / Z-order / vacuum / restore ........................... §13
```

### "Which write path?"

```text
Table doesn't exist yet?
  ├─ defining fresh → §8.3-§8.8 (CreateBuilder), then write
  ├─ existing Parquet directory → §8.12 (convert)
  └─ from a query result → §8.14 (CTAS-like bootstrap)
Data is Arrow RecordBatches → DeltaTable::write(...).with_save_mode(...) — §5.3-§5.7
Data is a DataFusion plan/DataFrame → plan write with explicit SessionState — §5.9, §5.21, §7.7
Replacing a slice (partition / predicate) → replaceWhere — §5.8, §12.6-§12.7
Row-level conditional change → DML — §9 (delete §9.4 / update §9.7 / merge-upsert §9.10-§9.14)
Need exactly-once / retry safety → §5.16-§5.17 (app-txn), §9.23 (idempotent merge)
```

### "Why is my query slow / scanning too much?"

```text
1. EXPLAIN / EXPLAIN ANALYZE the plan ............... §7.3.3, §6.19
2. Predicate actually pushed down? .................. §6.10, §12.11
3. Projection pruned? ............................... §6.11, §7.8.3
4. Partitions pruned (filter on partition cols)? .... §6.12, §12.8
5. File-level stats skipping effective? ............. §6.13, §12.9-§12.13
6. Too many small files? ............................ §12.24 → optimize §13.4
7. Layout wrong for the query pattern? .............. §12.19, §12.15
8. Runtime metrics + benchmark ...................... §7.8.8-§7.8.9, §6.28
```

---

## Operating rules

1. **Pin exactly**: `deltalake`/`deltalake-core` by **git rev** (`35cfed45…`, the 1.0.0 pre-release line) plus `delta_kernel = { package = "buoyant_kernel", rev = "393fbf6…" }`; `datafusion = "=54.0.0"`, `arrow` family `58.3.0`, `object_store = "0.13.2"`. Never generate examples against older DataFusion/Arrow/provider APIs without marking them legacy (§0.1, §0.4, §0.10).
2. **Feature gates are load-bearing**: default = `rustls` only. Any DataFusion integration requires `features = ["datafusion"]` or imports fail. Pick exactly one TLS posture (`rustls` xor `native-tls`/`s3-native-tls`); enable cloud features (`s3`/`azure`/`gcs`) only for the backends actually used (§0.6-§0.7).
3. **Current write API**: `table.write(batches).with_save_mode(...).await?` returns `DeltaTable` — **no metrics tuple** at the 1.0.0 pinned rev. `DeltaOps` is deprecated; prefer operation-specific builders and record exact builder paths (§5.3, §0.9.2).
4. **`update_datafusion_session` is idempotent and never overwrites** an existing object-store mapping. Use a fresh `SessionContext` per credential domain/endpoint (prod S3 vs MinIO vs tenant); never assume re-registration replaces a stale mapping (§7.2.2-§7.2.3).
5. **A `DeltaTable` is a pinned snapshot**, not a live view. Refresh explicitly for freshness; pin a version for reproducibility; a registered provider binds the snapshot at registration time — re-register after refresh (§3.7, §3.15, §3.18, §7.1.4, §6.24).
6. **Operation predicates are DataFusion `Expr`s** — `col("a").eq(lit(1)).and(...)`; no operator overloading. Mind SQL null semantics in merge/update matches; never splice user/UI strings into predicates — build typed `Expr`s or validate/escape (§7.5, §7.4.3, §9.28).
7. **Merge correctness discipline**: clause order matters; deduplicate the source first (duplicate matches are a correctness bug); keep predicates deterministic (no `now()`-style volatility); design idempotency via app transactions/commit properties (§9.12, §9.23-§9.25, §5.16).
8. **Vacuum destroys time travel and CDF below retention.** Dry-run first, run the §13.24-§13.25 safety checks, and reconcile retention with CDF consumers (§10.18) and restore plans (§13.19) before executing (§13.13-§13.18).
9. **Partition only on low-cardinality columns**; high-cardinality partitioning is the canonical layout anti-pattern. Pre-validate `replaceWhere` inputs; drive layout from query patterns via the §12.15 decision table (§12.4-§12.7, §12.19).
10. **Schema is enforced at write; evolution is opt-in** via explicit schema modes; cast safety is a declared posture, not a default rescue. Timestamps are microsecond-canonical (ns experimental); check the cross-engine matrix before adopting variant/ns/column-mapping features (§4.13-§4.15, §4.18, §4.27, §11.13).
11. **Match Arrow majors across every crate touching Arrow types** — batches built with a mismatched `arrow` version fail at the provider boundary. Re-use the workspace pins; never add a second Arrow version (§7.6.7, §0.10).
12. **Grep the doc with scoped patterns** (`^# [0-9]+\.`, `^## [0-9]+\.[0-9]+ `): §1 doesn't exist, §7 subchapters are H1s, and code-block comments shadow headings (see "How the reference document is organized").

---

## Project context: smartref

The Rust `deltalake` crate is smartref's **durable substrate** — Delta tables are the canonical durable home for relation specs (canonical == durable; the Delta log is the ledger). Workspace pins (`Cargo.toml`) match the doc's §0 anchor exactly: `deltalake` + `deltalake-core` git-rev-pinned to `35cfed45…` (the 1.0.0 pre-release line) with `default-features = false, features = ["rustls", "datafusion", "s3", "gcs", "azure"]`, plus `delta_kernel` (packaged as `buoyant_kernel` **0.24.0**, git rev `393fbf6…`, `arrow-58` + `internal-api`), `object_store 0.13.2` (`aws`/`gcp`/`azure`/`http`), `datafusion = "=54.0.0"`, `arrow 58.3.0`. Cloud features are compiled per ADR 2026-06-03 for the durable-substrate roadmap even where runtime use is local.

**`crates/smartref_delta`** is the integration crate: `DeltaRoot` (URL root + per-relation table URLs + fingerprint), `DeltaTableContract`/`table_contract`, `schema_bridge`, `provider`/`provider_identity`, `write_sink`/`write_strategy`/`plan_write`/`calculation_output`, `cdf`/`cdf_command` (scan + checkpointed consumption), `maintenance`/`maintenance_command` (optimize/vacuum/restore/fsck with metrics), `migration`, `deployment` profiles, `commit_metadata`, `receipt`, and the `DeltaError` taxonomy. Consumers: `smartref_pyo3` (Engine), `smartref_catalog` (`ProviderKind::Delta`, native relation manifest), `smartref_runtime` (Delta catalog registration, live schema provider), `smartref_govern`, `smartref_workbook`, `smartref_planning`.

**Command boundary (mirrors the doc's closing callout):** all smartref Delta mutations enter through `DeltaOperationCommand` (variants: `DeltaWriteBatches`, `DeltaWritePlan`, `DeltaDelete`, `DeltaUpdate`, `DeltaMergeUpsert`, `DeltaReplacePartition`, `DeltaOptimize`, `DeltaVacuum`, `DeltaRestore`, `DeltaFilesystemCheck`, `DeltaCdfConsume`, `DeltaCalculationOutput`, `DeltaMigrationRequest`, …) and return `DeltaOperationReceipt`. PyO3/Python surfaces parse requests into commands; runtime-owned execution invokes the adapter. Direct write/CDF/maintenance helpers are **adapter internals or test fixtures**, never public mutation authority — when adding a mutation path, extend the command vocabulary rather than exposing a builder.

Chapter → smartref surface map:

| Working in | Read |
|---|---|
| `root.rs`, `deployment.rs`, storage options, env config | §2, §0.6-§0.7 |
| `provider.rs`, `provider_identity.rs`, runtime catalog registration | §3, §6, §7.1-§7.2 |
| `write_sink.rs`, `write_strategy.rs`, `plan_write.rs`, `calculation_output.rs` | §5, §7.6-§7.7 |
| `migration.rs`, `table_contract.rs`, `schema_bridge.rs` | §4, §8, §11.15-§11.16 |
| `command.rs` DML variants, `commit_metadata.rs`, `receipt.rs` | §9, §5.13/§5.16-§5.17 |
| `cdf.rs`, `cdf_command.rs`, checkpoint views | §10 |
| `maintenance*.rs`, vacuum/optimize/restore/fsck commands | §13, §12.17-§12.25 |
| governance gates (`smartref_govern`) | §11 |

---

## Related skills

* **`datafusion-pyarrow-rust-ref`** — the core Rust DataFusion + Arrow stack (`SessionContext`, plans, `Expr`, `RecordBatch`, UDFs, schemas, `object_store`, PyCapsule boundary). This skill assumes that substrate; read it for anything DataFusion-side that isn't Delta-specific. The DF 53→54 substrate change catalog is `docs/library_ref/datafusion_54vs53.md`.
* **`datafusion-pyarrow-ref`** — Python `datafusion`/`pyarrow`; the Python `deltalake` package doc (`docs/library_ref/deltalake.md`) lives on that side of the boundary.
* The smartref GUI/data-access rule stands: Python consumes the substrate through `smartref_core_native` over PyCapsule — never `import datafusion` in Python, and never bypass the `DeltaOperationCommand` boundary from Python.
