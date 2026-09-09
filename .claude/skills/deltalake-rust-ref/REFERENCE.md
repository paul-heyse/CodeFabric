# delta-rs 1.0.0 @ `43a0cf10` — Detailed Reference

This is the mechanical layer behind [SKILL.md](SKILL.md): section indexes with verified
line numbers, a symbol index, the pattern→section binding matrix, the principle binding
table, decision trees, and operating rules. Come here when you know *what* you need and
want the exact place to read; use SKILL.md first when you are still classifying the
problem.

**Line-number policy: seek by line, cite by section.** Line numbers appear only in this
file's §1 tables (and the §2 P-table), because line numbers move when a document is
regenerated and section identifiers do not. Every index table is headed by the exact
command that re-derives it — if a `Read(offset)` lands on the wrong heading, re-derive
before trusting anything else in that table.

## Document aliases

Aliases follow `docs/spec_index/library-routing.md` §1 and must stay in sync with it.

| Alias | Document (under `docs/library_ref/`) | Chapters | Lines |
|---|---|---|---:|
| `delta` | `deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md` | §0, §2–§13 (**no §1**); §7 carries h1 sub-chapters §7.0–§7.13 | 17,270 |
| `principles` | `full_data_fabric_design_principles_v2.md` | current P1–P36 guidance | revised |
| `delta-align` | `deltalake_1.0.0_43a0cf10_design_principle_alignment_manual_2026-08-26.md` | §0–§2 · P1–P25 · 16 families / 156 pattern IDs · flows §8–§20 · artifacts §21–§33 · crosswalks §34–§36 · checklists §37–§47 · Parts VII–VIII · App. A–D | 2,903 |

`principles` is shared with the sibling skill `datafusion-pyarrow-rust-ref`; it is one
constitution with two alignment manuals over it. `REFERENCE §N` and `SKILL §…` refer to
this skill's own files, never to a document.

## Table of contents

- §1 — Per-document section indexes (`delta` chapters in §1.1, `delta` symbol index in §1.2, `principles` in §1.3, `delta-align` in §1.4)
- §4 — Operating rules
- §5 — Project context: CodeFabric

---

## §1 — Per-document section indexes

### §1.1 `delta` — chapter index

Re-derive with:

```bash
rg -n '^# [0-9]+(\.[0-9]+)? ' docs/library_ref/deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md
```

**Hazards.**

1. **There is no §1.** The body runs §0, then §2–§13. A "§0–§13" claim is wrong, and any
   citation to §1 of that document is invalid.
2. **14 of the file's 41 `^# ` lines sit inside code fences** — `# AWS S3`,
   `# Cargo.toml`, `# rust-toolchain.toml`, `# builder stage additions…`, and similar
   shell/TOML comments. A bare `rg '^# '` is roughly one-third wrong; a bare
   `just lib-outline` inherits the same noise. Use the anchored pattern above, which
   matches none of them.
3. **§13.29 is used twice** — "Best practices" and, at the end of the file, "Latest-pin
   `OPTIMIZE` interoperability fixes for nested schemas". It is the only duplicated
   section number in the document; disambiguate by title, and this file does.
4. Twelve chapters end with a **latest-pin behavior note** appended after the ordinary
   "Value case" subsection (§3.27, §3.28, §4.37, §5.29, §6.35, §6.36, §8.25, §9.35,
   §10.29, §11.27, §12.33, and the second §13.29). These carry the `43a0cf10`-specific
   corrections and are easy to miss by reading a chapter top-down.

| § | Line | Title |
|---|---:|---|
| §0 | 1 | Version, feature, and compatibility baseline |
| §2 | 1031 | Deployment and project setup in production services |
| §3 | 2367 | Table loading, snapshots, state, and time travel |
| §4 | 3901 | Schema, Arrow type mapping, and metadata governance |
| §5 | 5321 | Writing data from Arrow and DataFusion |
| §6 | 6752 | Reading and querying through DataFusion |
| §7 | 7984 | DataFusion + Arrow integration track (exhaustive) |
| §8 | 9548 | Create-table workflows |
| §9 | 10807 | DML: delete, update, and merge |
| §10 | 12188 | Change Data Feed — incremental consumption |
| §11 | 13292 | Constraints, properties, and governance |
| §12 | 14461 | Partitioning, layout, and file skipping |
| §13 | 15910 | Optimize, compaction, Z-order, and vacuum |

**§7 sub-chapters** (h1s, not h2s — they are siblings of the chapter heading in the
markdown tree, so an h2-only outline misses them):

| § | Line | Title |
|---|---:|---|
| §7.0 | 8008 | Integration mental model |
| §7.1 | 8052 | `TableProvider` integration |
| §7.2 | 8220 | DataFusion session/runtime integration |
| §7.3 | 8372 | SQL API path |
| §7.4 | 8543 | DataFrame API path |
| §7.5 | 8675 | DataFusion expressions inside Delta operations |
| §7.6 | 8800 | Arrow batch interoperability |
| §7.7 | 8999 | Writing from DataFusion plans |
| §7.8 | 9131 | File skipping, pruning, and performance |
| §7.9 | 9330 | End-to-end service skeleton |
| §7.10 | 9399 | Production best practices |
| §7.11 | 9450 | Anti-patterns |
| §7.12 | 9469 | LLM-agent checklist |
| §7.13 | 9500 | Value case |

The document carries 401 fence-aware h2 subsections. They are not tabulated here — zoom
with `just lib-outline <path> --view names` once you know the chapter, and use §1.2 below
to go straight from a symbol to its subsection.

### §1.2 `delta` — symbol index

Each row cites the subsection where the symbol is actually exercised, verified by
occurrence **inside that subsection's line range** rather than by whole-file grep. A `+`
lists a secondary subsection worth reading with it. Symbols marked **absent** do not
occur anywhere in `delta`: the surrounding contract is cited instead, and the symbol
itself must be verified against the pinned source.

**Loading, snapshots, freshness, time travel (§3)**

| Symbol | Cited to | Note |
|---|---|---|
| `open_table` | §3.3 | + §2.4 for URL construction |
| `open_table_with_storage_options` | §3.3 | + §2.5 storage-options map design |
| `DeltaTableBuilder` | §3.4 | + §3.9 for the timestamp form |
| `with_storage_options` | §3.3 | + §3.4 builder form |
| `with_allow_http` | §3.4 | only occurrence |
| `with_version` | §3.4 | pinning at build time; §3.8 for post-load |
| `load_version` | §3.8 | exact-version reload |
| `load_with_datetime` | §3.9 | resolve, then persist the resolved version |
| `get_latest_version` | §3.6 | backing-store latest ≠ loaded version |
| `update_incremental` | §3.7 | + §3.18 avoiding stale state |
| `without_files` | §3.4 | + §3.28 lazy replay cost |
| `with_skip_stats` | §3.4 | + §12.12 stats-loading pitfalls |
| `DeltaTableState` | §3.1 | + §3.10 snapshot inspection |
| `snapshot()` | §3.10 | + §3.13 add actions as Arrow batches |
| `EagerSnapshot` | §3.28 | lazy/eager replay and cache identity |
| `BlindDeltaTable` | §3.27 | stats-free blind-append handle only |
| `history()` | §3.11 | retention-bound audit view |
| `add_actions_table` | §3.13 | Arrow view of the active file set |
| `get_files_by_partitions` | §3.12 | + §12.8 partition filters |
| `get_active_add_actions_by_partitions` | §3.12 | only occurrence |
| `PartitionFilter` | §3.12 | + §12.8, §13.7 |
| `metadata()` | §3.10 | + §3.14 metadata/protocol validation gate |
| `protocol()` | §3.10 | + §11.12 protocol inspection |
| `DeltaTableConfig` | §12.12 | + §12.33 selective stats materialization |
| error handling taxonomy | §3.21 | per-chapter taxonomies also at §5.23, §9.30, §10.23, §11.21, §12.27, §13.27 |
| `DeltaTableError` | §0.11 | the type is named in the doc-test harness; §4.25 carries `UnsupportedColumnMapping` |

**Schema, types, metadata (§4)**

| Symbol | Cited to | Note |
|---|---|---|
| `StructType` | §4.3 | canonical Delta schema construction |
| `StructField` | §4.3 | + §8.4 create-table form |
| primitive type catalog | §4.4 | one table for every Delta primitive |
| Arrow schema boundary | §4.6 | + §4.7 validation posture |
| nullable fields | §4.8 | + §4.37 nested physical optionality |
| field metadata | §4.9 | + §4.10 update, §8.5 at creation |
| `with_metadata` | §8.5 | table/field metadata at creation; §4.9–§4.11 to update |
| `add_columns` | §4.12 | additive migration |
| schema enforcement on write | §4.13 | strict default |
| schema evolution on write | §4.14 | + §5.5 `SchemaMode` |
| decimal types | §4.17 | precision/scale policy |
| timestamp types | §4.18 | timezone and ntz policy |
| structs / lists / maps | §4.20–§4.22 | nested containers |
| variant type | §4.23 | + §11.27 feature validation |
| `TableFeatures` | §11.27 | + §4.23 |
| Arrow extension metadata | §4.24 | annotation, not enforcement |
| `columnMapping` | §4.25 | + §11.27 operation restrictions |
| type widening | §4.26 | + §11.27 `typeWidening` |
| cross-engine compatibility matrix | §4.27 | the certification surface |
| schema contract object | §4.29 | + §4.30 validation helper |
| golden schema fixtures | §4.31 | + §4.32 governance runbook |

**Writes (§5)**

| Symbol | Cited to | Note |
|---|---|---|
| `DeltaTable::write` | §5.3 | the current write surface |
| `WriteBuilder` | §5.3 | + §5.1 write-path mental model |
| `DeltaOps` | §5 (chapter preamble) | **deprecated at this rev** in favour of `DeltaTable::` methods; §8.2 repeats it for creation |
| `SaveMode` | §5.4 | + §8.10 for creation idempotency |
| `SchemaMode` | §5.5 | + §4.14 |
| cast safety | §5.6 | + §9.9 for update/merge |
| partitioned writes | §5.7 | + §12.3 |
| `replaceWhere` | §5.8 | + §12.6–§12.7 pre-validating input |
| DataFusion `LogicalPlan` write | §5.9 | + §7.7 writing from plans |
| session fallback policy | §5.10 | require explicit session state in production |
| `with_target_file_size` | §5.11 | + §12.18 file-size strategy |
| `with_writer_properties` | §5.12 | + §9.21, §13.9 |
| `WriterProperties` | §5.12 | Parquet writer knobs |
| `CommitProperties` | §5.13 | + §8.17, §9.20 |
| `with_commit_properties` | §5.13 | + §8.17 creation form |
| `with_max_retries` | **absent** | not documented in `delta`; nearest contract is §5.17 retry safety and §9.22 conflict/retry posture. Verify against the pinned source. |
| `with_application_transaction` | **absent** | not documented in `delta`; idempotency contract is §5.16, §9.23. Verify against the pinned source. |
| `with_custom_execute_handler` | §5.14 | + §9.7, §11.7 |
| idempotent write patterns | §5.16 | + §9.23 idempotent merge design |
| retry safety / atomic commit | §5.17 | unknown-outcome reconciliation lives here |
| small-file avoidance | §5.18 | + §12.24 detector |
| action-path URI encoding | §5.29 | latest-pin hardening; never hand-encode paths |

**Query and provider (§6, §7)**

| Symbol | Cited to | Note |
|---|---|---|
| `DeltaTable` as `TableProvider` | §6.5 | + §7.1.1 minimal registration |
| `TableProviderBuilder` | §7.1.2 | + §6.36 schema adaptation |
| `DeltaScanConfig` | §6.14 | + §7.8.2 performance view |
| `file_column` | §6.15 | diagnostics/provenance only |
| `DeltaSessionContext` | §6.16 | + §7.2.6 |
| `DeltaRuntimeEnvBuilder` | §6.17 | + §6.18 spill configuration |
| `update_datafusion_session` | §7.2.2 | + §6.22 avoiding duplicate object-store registration |
| `register_object_store` | §2.11 | only occurrence |
| predicate pushdown | §6.10 | + §12.11, §7.8 |
| projection pushdown | §6.11 | + §7.8.3 |
| partition pruning | §6.12 | + §7.8.4, §12.8 |
| file/data skipping | §6.13 | + §12.10 min/max skipping |
| freshness and provider lifecycle | §6.24 | providers do not auto-refresh |
| querying pinned versions | §6.25 | + §3.15 |
| `EXPLAIN` / `EXPLAIN ANALYZE` | §7.3.3 | + §6.19 physical plan diagnostics |
| `with_session_state` | §5.9 | + §9.19 DML, §13.8 optimize |
| `FileSelection` | §6.35 | latest-pin targeted file reads |
| `MissingSelectedFilePolicy` | §6.35 | fail-closed vs skip policy |
| Delta-aware expression/schema adaptation | §6.36 | column mapping + DV handled by delta-rs |

**Create-table (§8)**

| Symbol | Cited to | Note |
|---|---|---|
| `CreateBuilder` | §8.2 | primary construction APIs |
| create from Delta schema | §8.4 | + §4.5 |
| partitioned table creation | §8.7 | + §12.3 |
| table properties | §8.8 | + §11.8–§11.9 typed keys |
| `enableChangeDataFeed` | §8.8 | + §10.3 enable at creation |
| `appendOnly` | §8.8 | + §11.10 |
| storage options at creation | §8.9 | + §2.5 |
| create-or-replace | §8.11 | + §8.16 initialization idempotency |
| `convert_to_delta` | §8.12 | Parquet-directory conversion |
| bootstrap from Arrow batches | §8.13 | + §8.14 from query results |
| low-level actions | §8.18 | the bottom of the ladder |
| `create_checkpoint` | §8.25 | checkpoint writing is kernel-owned |
| `checkpoints::` | §8.25 | same subsection |

**DML (§9)**

| Symbol | Cited to | Note |
|---|---|---|
| `DeleteBuilder` | §9.4 | + §9.5 delete-all, §9.6 metrics |
| `DeleteMetrics` | §9.6 | + §9.4 |
| `UpdateBuilder` | §9.7 | + §9.8 metrics |
| `UpdateMetrics` | §9.8 | + §9.7 |
| merge source into target | §9.10 | + §9.11 clause families, §9.12 ordering |
| `MergeMetrics` | §9.18 | + §9.10 |
| upsert pattern | §9.14 | + §9.23 idempotent merge design |
| duplicate match avoidance | §9.24 | ambiguous multi-match is a design error |
| session-state injection for DML | §9.19 | + §7.5 expressions inside operations |
| conflict detection / retry posture | §9.22 | + §5.17 |
| file rewrite behavior | §9.26 | physical cost, not semantic change |
| append-only / column-mapping limits | §9.27 | fail closed before DML |

**CDF (§10)**

| Symbol | Cited to | Note |
|---|---|---|
| enable CDF at creation | §10.3 | + §10.4 on existing tables |
| `scan_cdf` | §10.5 | the only supported change API |
| `CdfLoadBuilder` | §10.5 | + §10.12 filtering output |
| version range semantics | §10.6 | + §10.8 out-of-range behavior |
| timestamp range semantics | §10.7 | version stays authoritative |
| CDF schema | §10.9 | `_change_type`, `_commit_version`, `_commit_timestamp` |
| `_change_type` | §10.9 | + §10.10 change-type semantics |
| `_commit_version` | §10.9 | + §10.22 validation queries |
| `_commit_timestamp` | §10.9 | secondary to version |
| incremental consumer checkpoint | §10.13 | persist outside the source table |
| retention, vacuum, history availability | §10.18 | the closure boundary |
| initial snapshot + catch-up | §10.20 | race-free baseline |
| `inCommitTimestamp` | §10.29 | latest-pin ICT support + fallback |

**Governance, constraints, protocol (§11)**

| Symbol | Cited to | Note |
|---|---|---|
| check constraints | §11.3 | + §11.4 naming, §11.5 violation behavior |
| `Constraint` | §11.3 | + §11.21 error taxonomy |
| NOT NULL constraints | §11.7 | existing-data validation matters |
| table properties | §11.8 | + §11.9 typed property keys |
| `TableProperty` | §11.9 | typed key surface |
| `logRetentionDuration` | §11.9 | + §3.11 history availability |
| `deletedFileRetentionDuration` | §11.9 | vacuum retention floor |
| `Protocol` | §11.12 | protocol inspection |
| `min_reader_version` / `min_writer_version` | §11.12 | + §11.20 governance guard |
| `reader_features` / `writer_features` | §11.12 | declared ≠ supported |
| feature compatibility across engines | §11.13 | the certification matrix |
| `add_feature` | §11.14 | + §11.19 property migration |
| governance guard before writes/DML | §11.20 | the fail-closed gate |
| `ProtocolChecker` | §11.27 | strict declared-feature validation |
| `timestampNtz` / `typeWidening` / `v2Checkpoint` / `deletionVectors` | §11.27 | latest-pin strict validation |

**Layout and maintenance (§12, §13)**

| Symbol | Cited to | Note |
|---|---|---|
| partition columns | §12.3 | + §12.4 low-cardinality guidance |
| file statistics | §12.9 | + §12.10 min/max skipping |
| stats-loading pitfalls | §12.12 | + §12.33 selective stats materialization |
| table properties for data skipping | §12.13 | tunables, not guarantees |
| add-actions layout report | §12.20 | diagnostics |
| small-file detector | §12.24 | + §12.25 compaction trigger |
| `OptimizeType` | §12.17 | + §13.4, §13.6 |
| `OptimizeBuilder` | §13.4 | + §13.5 target file sizes |
| `z_order` | §13.6 | + §13.12 maintenance policy object |
| partition-scoped optimize | §13.7 | avoid hot partitions |
| optimize with session state | §13.8 | + §13.9 writer properties |
| `with_dry_run` | §13.13 | vacuum preflight |
| `VacuumBuilder` | §13.14 | + §13.13 dry run |
| `with_retention_period` | §13.14 | + §13.15 keep versions |
| `with_enforce_retention_duration` | §13.14 | disabling it is a governed decision |
| `keep_versions` | §13.15 | pin protection |
| `VacuumMode` | §13.16 | lite vs full |
| `VacuumMetrics` | §13.17 | + §13.13–§13.16 |
| time-travel breakage after vacuum | §13.18 | + §10.18 for CDF |
| `RestoreBuilder` | §13.19 | restore is a new committed version |
| `filesystem_check` | §13.20 | incident repair only |
| `FileSystemCheckBuilder` | §13.26 | only occurrence |
| safety checks before optimize / vacuum | §13.24–§13.25 | the approval preflight |
| nested `OPTIMIZE` interoperability fixes | §13.29 (second) | duplicate section number — see §1.1 hazard 3 |

**Deployment and storage (§0, §2)**

| Symbol | Cited to | Note |
|---|---|---|
| canonical baseline `1.0.0` @ `43a0cf10…` | §0.1 | + §0.15 current identity |
| `deltalake` vs `deltalake-core` | §0.3 | crate-selection decision |
| feature flag matrix | §0.6 | + §0.7 selection recipes |
| dependency-skew controls | §0.8 | + §2.15 drift detection |
| API stability risk zones | §0.9 | what may move under the pin |
| DataFusion + Arrow alignment requirements | §0.10 | one type universe |
| net change from `9f922319…` | §0.16 | the only place `num_retries` appears |
| `num_retries` | §0.16 | write metrics now expose retry count |
| `LogStore` | §2.1 | transaction-log consistency boundary |
| `ObjectStore` | §2.1 | physical I/O only, never table state |
| URL construction and table loading | §2.4 | + §5.29 path encoding |
| storage-options map design | §2.5 | + §2.16 deployment config object |
| `AWS_S3_LOCKING_PROVIDER` | §2.6 | multi-writer S3 safety |
| TLS selection | §2.10 | rustls vs native-tls |
| DataFusion runtime configuration | §2.11 | + §7.2 |
| CI fixtures | §2.14 | + §2.7 MinIO/R2/LocalStack |
| IAM and secret handling | §2.17 | + §2.18 logging/observability |
| `opendal` / OpenDAL backends | §2.23 | new at the pinned rev |

## Current principles and optional alignment

The current P1–P36 guidance is in `docs/library_ref/full_data_fabric_design_principles_v2.md`. Alignment manuals are optional capability references. Their former workflow/artifact matrices are retired; use current headings with `just lib-outline`. No principle crosswalk or exhaustive API survey is required before implementation. Preserve native API details from the indexes above and verify current Cargo source selections when using them.
