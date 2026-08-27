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
| `principles` | `full_data_fabric_design_principles.md` | Principles 1–25 + §27–§31 | 1,327 |
| `delta-align` | `deltalake_1.0.0_43a0cf10_design_principle_alignment_manual_2026-08-26.md` | §0–§2 · P1–P25 · 16 families / 156 pattern IDs · flows §8–§20 · artifacts §21–§33 · crosswalks §34–§36 · checklists §37–§47 · Parts VII–VIII · App. A–D | 2,903 |

`principles` is shared with the sibling skill `datafusion-pyarrow-rust-ref`; it is one
constitution with two alignment manuals over it. `REFERENCE §N` and `SKILL §…` refer to
this skill's own files, never to a document.

## Table of contents

- §1 — Per-document section indexes (`delta` chapters in §1.1, `delta` symbol index in §1.2, `principles` in §1.3, `delta-align` in §1.4)
- §2 — Binding matrix: utilization patterns → `delta` sections; P1–P25 binding table
- §3 — Decision trees (document choice · flow classifier · operation ladder · capability-status evidence · release gate)
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

### §1.3 `principles` — the design constitution

Re-derive with `rg -n '^# ' docs/library_ref/full_data_fabric_design_principles.md`.

**Citation convention:** the h1 ordinal is **principle number + 1 by construction**
(`# 15. Principle 14 — …`). Always cite by principle number and title ("Principle 14"),
never by the h1 ordinal. Lines for Principles 1–25 are carried by the P-table at the end
of REFERENCE §2.

| Section | Line | Title |
|---|---:|---|
| title | 1 | Model-First, Contract-Driven, Provenance-Native Data Fabric |
| Principles 1–25 | 21–1123 | see the P-table in REFERENCE §2 |
| §27 | 1124 | The overall architectural pattern |
| §28 | 1167 | Mandatory design questions for an LLM programming agent |
| §29 | 1195 | Anti-patterns agents should actively reject |
| §30 | 1279 | Compact agent design constitution |
| §31 | 1319 | Short form |

### §1.4 `delta-align` — the Delta design-principle alignment manual

Re-derive with
`rg -n '^#{1,2} ' docs/library_ref/deltalake_1.0.0_43a0cf10_design_principle_alignment_manual_2026-08-26.md`
— the file has no `#`-prefixed lines inside code fences, so the bare pattern is safe here
(133 headings, fence-aware and raw counts agree).

Front matter and workflow:

| Section | Line | Contents |
|---|---:|---|
| §0 | 15 | Purpose and scope; **§0.1 (31) capability-status legend — 8 statuses**; §0.2 (44) what Delta is authoritative for; §0.3 (68) what stays application-owned; **§0.4 (85) table state vs data-fabric state**; §0.5 (110) source-grounding and derivation rule |
| §1 | 124 | §1.1 (126) required input; **§1.2 (143) mandatory 13-step review flow**; **§1.3 (161) 12 stop conditions** |
| §2 | 180 | §2.1 (182) preferred state-transition chain; **§2.2 (216) authority and derivation table**; **§2.3 (231) highest-level operation-selection hierarchy** |

**Hazard — the §1.2 output names are not all Part IV artifacts.** Five names appear
*only* in the §1.2 table and have no template anywhere in the manual:
`PhysicalLayoutPolicy`, `RetentionSafetyReview`, `ProviderBindingRecord`,
`AntiPatternDisposition`, `ImplementationPacket`. Conversely five Part IV artifacts are
never named in §1.2: `FeatureUtilizationPlan`, `ContractAndCapabilityMatrix`,
`LifecycleArtifactMap`, `StateOwnershipMap`, `MaintenanceSafetyReview` (the nearest
counterpart of §1.2's `RetentionSafetyReview`). Eight names are shared. Treat §1.2 as the
*workflow* and Part IV as the *templates*; do not assume a §1.2 output has a schema to
fill in.

**Part I (line 257): P1–P25**, one section per principle. Each carries `### Applicable
mechanisms` (or a principle-specific variant such as `### Native hierarchy`,
`### Preferred closure chain`, `### Fingerprint candidates`), `### Required utilization
rules`, often `### Application-owned overlay`, `### Required evidence`, `### Reject`, and
closes with a `**Primary patterns:**` line into Part II IDs — all 25 have one. Per-P
lines are in the REFERENCE §2 P-table.

**Part II (line 1310): 16 feature families, 156 pattern IDs.** Each row =
`ID | Feature(s) | Required leverage | Primary principles | Minimum evidence`.

| Family | Line | IDs | Theme |
|---|---:|---|---|
| MOD | 1316 | MOD-01–08 | semantic modeling and table authority |
| STA | 1329 | STA-01–10 | loading, snapshots, freshness, time travel |
| SCH | 1346 | SCH-01–12 | schema and table-contract utilization |
| GOV | 1363 | GOV-01–10 | protocol, feature, policy, mutation governance |
| TXN | 1380 | TXN-01–08 | transactions, optimistic concurrency, commit semantics |
| WRT | 1393 | WRT-01–08 | write and append utilization |
| DML | 1406 | DML-01–08 | delete, update, merge, row-level mutation |
| CDF | 1421 | CDF-01–10 | Change Data Feed utilization |
| LAY | 1436 | LAY-01–10 | partitioning, layout, statistics, pruning |
| MNT | 1451 | MNT-01–10 | optimize, vacuum, restore, checkpoint, repair |
| STO | 1468 | STO-01–10 | LogStore, ObjectStore, cloud, deployment |
| QRY | 1483 | QRY-01–10 | DataFusion serving and provider integration |
| OBS | 1498 | OBS-01–10 | provenance, history, observability, reproducibility |
| INT | 1513 | INT-01–10 | interoperability and compatibility |
| EXT | 1528 | EXT-01–08 | lowest-necessary Delta extension level |
| TST | 1541 | TST-01–14 | contract-derived Delta testing |

Section headings group them: §3 (1314) semantic modeling · §4 (1344) schema/protocol/
governance · §5 (1378) transaction/write/DML · §6 (1419) change-data/layout/maintenance ·
§7 (1466) storage/query/provenance/interop/testing.

**Part III (line 1561): 13 requirement-to-feature decision flows**, each a decision tree
ending in `### Required selections` (literal pattern IDs) and `### Agent questions`.

| Flow | Line | Requirement class |
|---|---:|---|
| §8 | 1565 | Table creation and schema contract |
| §9 | 1611 | Read, snapshot, and freshness |
| §10 | 1647 | Append/write |
| §11 | 1696 | Delete/update/merge |
| §12 | 1741 | Schema/protocol migration |
| §13 | 1785 | CDF / incremental consumption |
| §14 | 1827 | Query serving / DataFusion |
| §15 | 1866 | File selection / targeted repair read |
| §16 | 1900 | Optimize / layout maintenance |
| §17 | 1939 | Vacuum / retention |
| §18 | 1977 | Restore / incident repair |
| §19 | 2013 | Storage/backend |
| §20 | 2047 | Provenance / reproducibility |

**Part IV (line 2083): 13 required design artifacts** — `SemanticRequirement` (2087),
`AuthorityMap` (2108), `DeltaTableContract` (2118), `SnapshotPolicy` (2155),
`TransactionContract` (2170), `FeatureUtilizationPlan` (2193),
`ContractAndCapabilityMatrix` (2201), `LifecycleArtifactMap` (2211),
`ProvenanceClosureMap` (2225), `StateOwnershipMap` (2267), `MaintenanceSafetyReview`
(2279), `TestEvidenceMatrix` (2300), `OperationSelectionRecord` (2307).

**Part V (line 2331):** §34 (2333) principle→pattern crosswalk; §35 (2363) feature-family→
principles→requirement-class crosswalk; §36 (2385) preparation for the future
functional-building-block catalogue.

**Part VI (line 2417): 11 review checklists** — §37 semantic/authority (2419) · §38
snapshot/freshness (2428) · §39 schema-protocol contract (2439) · §40 transaction/write
(2452) · §41 DML (2466) · §42 CDF (2477) · §43 query/provider (2490) · §44 layout/
maintenance (2501) · §45 storage/deployment (2516) · §46 provenance/reproducibility
(2529) · §47 test evidence (2541).

**Part VII (line 2559):** 20-row anti-pattern → Delta symptom → constitutional violation →
prescribed-correction table. **Part VIII (2586):** compact agent instruction block.

**Appendix A (2612): version-specific leverage** — A.1 coordinated dependency universe
(2616) · A.2 DataFusion 55 / Arrow 59 migration (2629) · A.3 snapshot-native file
discovery and replay-safe lightweight snapshots (2637) · A.4 same-version checkpoint
adoption (2651) · A.5 `BlindDeltaTable` (2666) · A.6 deletion-vector-aware CDF (2680) ·
A.7 in-commit timestamp support (2693) · A.8 `FileSelection` and
`MissingSelectedFilePolicy` (2701) · A.9 nested physical nullability and partition-name
collision fixes (2709) · A.10 write retry metrics (2722) · A.11 full-vacuum scan
improvements (2737) · A.12 `mergeSchema` non-nullable fix (2745) · A.13 S3 option cleanup
(2753) · A.14 action-path URI encoding (2761) · A.15 `V2Checkpoint` protocol recognition
(2769) · A.16 strict declared-feature validation (2777) · A.17 OpenDAL backend family
(2785) · A.18 Python interop alignment (2793).

**Appendix B (2803): Delta authority matrix** — 16 artifacts scored on semantic
authority, durability, version identity, whether they can change without a Delta version
change, and correct architectural use. This is the fastest answer to "is X the
authority?" and has no DataFusion-manual counterpart.

**Appendix C (2826):** recommended table-class policy defaults (6 classes × write posture,
CDF, optimize, vacuum, retention emphasis). **Appendix D (2841):** compact release gate —
8 checklist blocks (dependency, snapshot/authority, schema/protocol, transaction, CDF,
query, maintenance, interop, provenance). Closing maxim at 2901.

---

## §2 — Binding matrix: utilization patterns → `delta` sections

**Contract:** the `delta-align` Part II row remains authoritative for required leverage,
primary principles, and minimum evidence. This matrix adds only what the manual lacks —
the file and section where each pattern's API surface is documented. A pattern is never
implementable from this table alone: read its Part II row first, then the sections here.
"overlay" means the capability is application-owned (`delta-align` §0.1 APPLICATION
OVERLAY) and the cited sections cover only the native artifacts it composes. The
`sibling` column routes to `datafusion-pyarrow-rust-ref` when the surface is
DataFusion/Arrow-side rather than Delta-side.

Bindings were verified against subsection content, not just titles.

### MOD — semantic modeling and table authority

| IDs | Theme | `delta` | sibling |
|---|---|---|---|
| MOD-01 | application `TableSpec` | overlay; compiles into §4.29, §8.1–§8.2 | — |
| MOD-02 | `SnapshotPolicy` as an explicit type | overlay over §3.3–§3.9, §3.15 | — |
| MOD-03 | `WriteSpec` / `DmlSpec` | overlay over §5.4–§5.5, §9.1 | — |
| MOD-04 | authority map | overlay; `delta-align` App. B is the reference table | — |
| MOD-05 | operation lifecycle model | overlay over §5.22, §11.20 | — |
| MOD-06 | versioned fingerprints | §3.15, §4.29–§4.30 | digest bytes → `canonicalization-lib-ref` |
| MOD-07 | multi-table publication model | overlay — Delta atomicity is per table (§5.17) | — |
| MOD-08 | operation selection record | §8.2, §8.18 (the API ladder's two ends) | — |

### STA — loading, snapshots, freshness, time travel

| IDs | Theme | `delta` | sibling |
|---|---|---|---|
| STA-01 | `DeltaTableBuilder::load`, `open_table*` | §3.3–§3.4 | — |
| STA-02 | loaded version vs backing-store latest | §3.6 | — |
| STA-03 | `with_version` / `load_version` | §3.8, §3.15 | — |
| STA-04 | timestamp travel | §3.9 | — |
| STA-05 | `update_state` / `update_incremental` | §3.7, §3.18 | — |
| STA-06 | `without_files` / lazy snapshot | §3.4, §3.28 | — |
| STA-07 | `with_skip_stats` | §3.4, §3.27, §12.12, §12.33 | — |
| STA-08 | snapshot-native file discovery | §3.10, §3.12–§3.13, §3.28 | — |
| STA-09 | same-version checkpoint refresh | §3.28, §8.25 | — |
| STA-10 | immutable request pin | §3.16–§3.17, §3.20, §6.25 | — |

### SCH — schema and table contract

| IDs | Theme | `delta` | sibling |
|---|---|---|---|
| SCH-01 | `StructType::try_new`, `StructField` | §4.3 | — |
| SCH-02 | Arrow schema as runtime boundary | §4.6–§4.7 | `arrow` §3 |
| SCH-03 | nullability, types, decimals, timestamps | §4.4, §4.8, §4.17–§4.19 | — |
| SCH-04 | nested structures | §4.20–§4.22, §4.37 | `arrow` §3 |
| SCH-05 | field/table metadata classes | §4.9–§4.11, §8.5–§8.6 | — |
| SCH-06 | `SchemaMode` | §5.5, §4.14 | — |
| SCH-07 | constraints / NOT NULL | §11.3, §11.7 | — |
| SCH-08 | protocol + table features | §11.12, §11.27 | — |
| SCH-09 | advanced feature compatibility | §4.23, §4.25–§4.27, §11.13, §11.27 | — |
| SCH-10 | schema/protocol fingerprint | overlay over §4.29–§4.30, §11.12 | — |
| SCH-11 | logical/physical schema adaptation | §6.36, §4.37 | `df-schema` S10 for the adapter contract |
| SCH-12 | schema migration workflow | §4.12, §4.32, §11.19 | — |

### GOV — protocol, feature, policy, mutation governance

| IDs | Theme | `delta` | sibling |
|---|---|---|---|
| GOV-01 | check constraints | §11.3–§11.5 | — |
| GOV-02 | append-only property | §11.10 | — |
| GOV-03 | CDF property | §11.11, §10.3–§10.4 | — |
| GOV-04 | authorization before builder execution | overlay; service boundary in §6.29 | `df` §39 |
| GOV-05 | protocol checker | §11.12, §11.27 | — |
| GOV-06 | operation-specific feature gates | §11.13, §11.20, §11.27 | — |
| GOV-07 | feature migration | §11.14, §11.19 | — |
| GOV-08 | retention governance | §13.15, §13.18, §10.18 | — |
| GOV-09 | destructive-operation approval | §13.20, §13.24–§13.25 | — |
| GOV-10 | capability truth registry | overlay over §4.27, §11.13 | — |

### TXN — transactions, concurrency, commit semantics

| IDs | Theme | `delta` | sibling |
|---|---|---|---|
| TXN-01 | exact read snapshot for a mutation | §3.15, §9.22 | — |
| TXN-02 | optimistic validate-and-commit | §5.17, §9.22 | — |
| TXN-03 | idempotency key / operation ID | §5.16, §9.23 | — |
| TXN-04 | unknown commit outcome reconciliation | §5.17, §5.23 | — |
| TXN-05 | commit properties | §5.13, §8.17, §9.20 | — |
| TXN-06 | before/after version contract | §3.6, §3.11 | — |
| TXN-07 | `num_retries` telemetry | §0.16 (the only occurrence) | — |
| TXN-08 | storage commit safety | §2.5–§2.6 | — |

### WRT — write and append

| IDs | Theme | `delta` | sibling |
|---|---|---|---|
| WRT-01 | `DeltaTable::write` | §5.3 | — |
| WRT-02 | `SaveMode` | §5.4, §8.10 | — |
| WRT-03 | `replaceWhere` | §5.8, §12.6–§12.7 | — |
| WRT-04 | `LogicalPlan` + `SessionState` | §5.9–§5.10, §7.7 | `df` §19, §3 |
| WRT-05 | target file size | §5.11, §12.18 | — |
| WRT-06 | write batch / Parquet properties | §5.12, §9.21 | `arrow` §11 |
| WRT-07 | micro-batch buffering | §5.18, §12.24 | — |
| WRT-08 | `BlindDeltaTable` | §3.27 | — |

### DML — delete, update, merge

| IDs | Theme | `delta` | sibling |
|---|---|---|---|
| DML-01 | delete builder | §9.4–§9.6 | — |
| DML-02 | update builder | §9.7–§9.9 | — |
| DML-03 | merge builder | §9.10–§9.13 | — |
| DML-04 | idempotent merge/upsert | §9.14, §9.23 | — |
| DML-05 | duplicate match policy | §9.24 | — |
| DML-06 | session-state injection | §9.19, §7.5 | `df` §3 |
| DML-07 | rewrite/file metrics | §9.18, §9.26 | — |
| DML-08 | append-only / feature restrictions | §9.27, §11.10 | — |

### CDF — Change Data Feed

| IDs | Theme | `delta` | sibling |
|---|---|---|---|
| CDF-01 | CDF enablement | §10.3–§10.4 | — |
| CDF-02 | `scan_cdf` | §10.5 | — |
| CDF-03 | exact version range | §10.6, §10.8 | — |
| CDF-04 | durable consumer checkpoint | §10.13 | — |
| CDF-05 | `_commit_version` + change type | §10.9–§10.10 | — |
| CDF-06 | in-commit timestamp | §10.29, §10.9 | — |
| CDF-07 | deletion-vector-aware CDF | §10.29, §11.27 | — |
| CDF-08 | retention guard | §10.18 | — |
| CDF-09 | initial snapshot + catch-up | §10.20 | — |
| CDF-10 | CDF schema evolution | §10.19 | — |

### LAY — partitioning, layout, statistics, pruning

| IDs | Theme | `delta` | sibling |
|---|---|---|---|
| LAY-01 | partition columns as table contract | §12.3–§12.4, §4.16 | — |
| LAY-02 | partition filters, not path parsing | §12.8, §3.12 | — |
| LAY-03 | Add-action stats | §12.9, §3.13, §12.20 | — |
| LAY-04 | data skipping without overstatement | §12.10–§12.11, §6.13 | `df` §15 |
| LAY-05 | small-file SLO | §12.24, §12.18 | — |
| LAY-06 | query-pattern-driven layout | §12.15, §12.19 | — |
| LAY-07 | selective stats / lazy materialization | §12.12, §12.33, §3.28 | — |
| LAY-08 | file path identity | §5.29 | — |
| LAY-09 | `FileSelection` | §6.35 | — |
| LAY-10 | deletion vectors | §11.27, §6.36 | — |

### MNT — optimize, vacuum, restore, checkpoint, repair

| IDs | Theme | `delta` | sibling |
|---|---|---|---|
| MNT-01 | optimize compact | §13.4–§13.5 | — |
| MNT-02 | Z-order | §13.6 | — |
| MNT-03 | partition-scoped optimize | §13.7 | — |
| MNT-04 | vacuum dry run | §13.13 | — |
| MNT-05 | vacuum retention / keep versions | §13.14–§13.15, §13.18 | — |
| MNT-06 | vacuum full/lite/concurrency | §13.16 | — |
| MNT-07 | restore | §13.19 | — |
| MNT-08 | filesystem check | §13.20 | — |
| MNT-09 | checkpoint semantics | §8.25, §3.28 | — |
| MNT-10 | nested optimize regressions | §13.29 (second — REFERENCE §1.1 hazard 3), §4.37 | — |

### STO — LogStore, ObjectStore, cloud, deployment

| IDs | Theme | `delta` | sibling |
|---|---|---|---|
| STO-01 | `LogStore` as commit boundary | §2.1 | — |
| STO-02 | `ObjectStore` as physical I/O only | §2.1, §6.22 | `arrow` §14 |
| STO-03 | typed storage config | §2.5, §2.16 | — |
| STO-04 | S3 safe write config | §2.6 | — |
| STO-05 | TLS feature posture | §2.10 | — |
| STO-06 | tenant/session credential isolation | §2.17, §7.1.6 | — |
| STO-07 | OpenDAL backends | §2.23 | — |
| STO-08 | cloud feature minimization | §0.6–§0.7, §2.2 | — |
| STO-09 | canonical table URI | §2.4, §5.29 | — |
| STO-10 | secret hygiene | §2.17–§2.18 | — |

### QRY — DataFusion serving and provider integration

| IDs | Theme | `delta` | sibling |
|---|---|---|---|
| QRY-01 | Delta `TableProvider`, never raw Parquet | §6.5, §7.1 | `df` §17–§18 |
| QRY-02 | `update_datafusion_session` | §7.2.2, §6.22 | `df` §16 |
| QRY-03 | provider version binding | §6.25, §7.1.4 | — |
| QRY-04 | rebuild on refresh | §6.24, §7.1.4 | — |
| QRY-05 | exact-version provider | §6.25 | — |
| QRY-06 | `SessionState` / runtime preservation | §6.16–§6.18, §7.2 | `df` §3, §27 |
| QRY-07 | projection/predicate/partition pushdown | §6.10–§6.13, §7.8 | `df-plan` §51 |
| QRY-08 | file column for diagnostics only | §6.15 | — |
| QRY-09 | query semantic metrics | §6.28, §7.8.8 | `df` §30 |
| QRY-10 | Delta-aware schema/DV adaptation | §6.36 | `df-schema` S10 |

### OBS — provenance, history, observability, reproducibility

| IDs | Theme | `delta` | sibling |
|---|---|---|---|
| OBS-01 | operation artifact bundle | overlay over §3.11, §5.13 | `df-plan` §55 |
| OBS-02 | `history()` within retention | §3.11 | — |
| OBS-03 | table identity + exact version | §3.6, §3.15 | — |
| OBS-04 | commit properties as lineage refs | §5.13, §8.17, §9.20 | — |
| OBS-05 | operation metrics | §9.6, §9.8, §9.18, §13.10, §13.17 | — |
| OBS-06 | retry/contention metrics | §0.16 | — |
| OBS-07 | CDF provenance | §10.6, §10.13 | — |
| OBS-08 | schema/protocol fingerprint per result | §4.29–§4.30, §11.12 | — |
| OBS-09 | environment record | §0.1, §0.15, §2.15 | `arrow` §2 |
| OBS-10 | closure status, explicitly reported | §10.8, §13.18 | — |

### INT — interoperability and compatibility

| IDs | Theme | `delta` | sibling |
|---|---|---|---|
| INT-01 | Arrow `RecordBatch` boundary | §7.6 | `arrow` §5–§6 |
| INT-02 | Parquet as Delta-managed format only | §5.12, §8.12 | `arrow` §11–§12 |
| INT-03 | DataFusion 55 integration | §0.10, §7.0–§7.2 | `df` §1, §3 |
| INT-04 | CDF → external CDC sinks | §10.17, §10.21 | — |
| INT-05 | table-feature matrix by engine | §4.27, §11.13, §11.27 | — |
| INT-06 | column mapping | §4.25, §9.27, §11.27 | — |
| INT-07 | variant / advanced types | §4.23 | `arrow` §26 |
| INT-08 | V2 checkpoint | §8.25, §11.27 | — |
| INT-09 | Python Arrow interop | §7.6.7 | `arrow` §21 |
| INT-10 | pinned dependency matrix | §0.1–§0.4, §0.8, §0.10, §2.15 | `df` §1, §34 + gates V1–V6 |

### EXT — lowest-necessary extension level (the ladder, `delta-align` §2.3)

| IDs | Theme | `delta` |
|---|---|---|
| EXT-01 | high-level load / provider | §3.3, §6.5 |
| EXT-02 | high-level write / DML builder | §5.3 · §9.4, §9.7, §9.10 |
| EXT-03 | metadata / schema / feature builder | §4.12, §8.6, §11.14 |
| EXT-04 | CDF / maintenance builder | §10.5 · §13.4, §13.13, §13.19 |
| EXT-05 | `FileSelection` | §6.35 |
| EXT-06 | `BlindDeltaTable` | §3.27 |
| EXT-07 | kernel transaction APIs | §8.25 (kernel-owned checkpoint), §8.18 |
| EXT-08 | LogStore / raw actions / object-store manipulation | §2.1, §8.18 |

### TST — contract-derived testing

| IDs | Theme | `delta` |
|---|---|---|
| TST-01 | schema contract tests | §4.31, §11.18 |
| TST-02 | snapshot/version tests | §3.22 |
| TST-03 | protocol/feature tests | §11.22, §11.27 |
| TST-04 | write-mode tests | §5.24 |
| TST-05 | transaction/concurrency tests | §5.24, §9.22 |
| TST-06 | DML tests | §9.31 |
| TST-07 | CDF tests | §10.24 |
| TST-08 | DataFusion provider tests | §6.30 |
| TST-09 | maintenance tests | §13.28 |
| TST-10 | storage/backend tests | §2.7, §2.14 |
| TST-11 | provenance/observability tests | §6.28, §13.26 |
| TST-12 | reproducibility/retention tests | §3.22, §13.18 |
| TST-13 | path/physical compatibility tests | §5.29, §4.37, §13.29 (second) |
| TST-14 | dependency/upgrade matrix | §0.8, §0.16, §2.15 |

### The P1–P25 binding table

Principle titles are shared verbatim between `principles` and `delta-align` Part I. Cite
the principle number, never the `principles` h1 ordinal (which is number + 1). To go from
a principle to its patterns use `delta-align` §34 (line 2333) — do not reconstruct that
mapping here; then resolve pattern IDs through the family blocks above.

| P | Title | `principles` line | `delta-align` line |
|---|---|---:|---:|
| P1 | Model semantics before implementing behavior | 21 | 259 |
| P2 | Make models executable, not merely descriptive | 93 | 301 |
| P3 | One authoritative owner for every concept | 134 | 339 |
| P4 | Use explicit conceptual hierarchies to encode shared guarantees and legal variation | 183 | 378 |
| P5 | Encode variability behind contracts, not throughout consumers | 244 | 430 |
| P6 | Separate semantic meaning from execution strategy | 280 | 466 |
| P7 | Build a shared canonical data fabric | 336 | 502 |
| P8 | Treat the common representation as infrastructure | 386 | 545 |
| P9 | Make provenance intrinsic to every meaningful transformation | 416 | 584 |
| P10 | Seek provenance closure | 472 | 628 |
| P11 | Prefer immutable snapshots and explicit state transitions | 508 | 677 |
| P12 | Schemas are executable contracts, not documentation | 549 | 718 |
| P13 | Put governance at the authoritative boundary | 602 | 768 |
| P14 | Prefer the highest-level extension that preserves the semantics | 637 | 813 |
| P15 | Preserve optimizer visibility | 681 | 855 |
| P16 | Treat lifecycle phases as first-class architecture | 715 | 899 |
| P17 | Make intermediate artifacts inspectable and reproducible | 778 | 957 |
| P18 | Fingerprint anything whose identity matters | 806 | 1001 |
| P19 | Make reproducibility a normal operating mode | 846 | 1041 |
| P20 | Be conservative about claimed capabilities | 881 | 1074 |
| P21 | Separate enforced semantics from advisory metadata | 913 | 1104 |
| P22 | Use protocols and canonical boundaries for interoperability | 960 | 1138 |
| P23 | Keep state ownership local and explicit | 985 | 1179 |
| P24 | Make observability semantic, not merely operational | 1041 | 1219 |
| P25 | Make testing derive from contracts and invariants | 1076 | 1264 |

---

## §3 — Decision trees

### 3a. Which document answers this?

```text
Is the question "how should this be designed / which principle applies"?
  -> principles (the constitution) and delta-align Part I    (P-table above)
Is the question "which Delta capabilities should this outcome use"?
  -> delta-align Part III flow, then Part II rows, then REFERENCE §2 bindings
Is the question "is X the authority for this concept?"
  -> delta-align App. B (authority matrix) and §0.2-§0.4
Is the question a delta-rs API/behavior question?
  -> delta                                                   (index §1.1, symbols §1.2)

Delta <-> datafusion-pyarrow-rust-ref, who wins:
  Delta TableProvider, DeltaScanConfig, provider freshness, file selection,
  Delta-aware schema/DV adaptation, writing a plan INTO a table -> delta §6-§7
  SessionContext/SessionState/RuntimeEnv semantics, Expr construction,
  LogicalPlan/ExecutionPlan, optimizer, UDFs, statistics contracts,
  the object_store crate itself, Arrow arrays/kernels/IPC        -> the sibling
  Parquet: file-format mechanics -> sibling (arrow §11-§12);
           Delta-managed writer properties and stats -> delta §5.12, §12.9
```

### 3b. Outcome → flow classifier (`delta-align` Part III)

```text
Does the outcome introduce a new durable logical table or change its contract?
                                                              -> §8  (creation/schema)
Does it read a table, and what does "read" mean here?         -> §9  (snapshot/freshness)
Does it add rows or replace a slice?                          -> §10 (append/write)
Does it mutate rows in place?                                 -> §11 (delete/update/merge)
Does it change schema, properties, protocol, or features?     -> §12 (migration)
Does it consume changes incrementally?                        -> §13 (CDF)
Does it serve queries through DataFusion?                     -> §14 (query serving)
Does it read specific known files for repair or quality work? -> §15 (file selection)
Does it change physical layout without changing meaning?      -> §16 (optimize)
Does it physically delete old files?                          -> §17 (vacuum/retention)
Does it roll a table back or repair an incident?              -> §18 (restore/repair)
Does it change storage backend, credentials, or deployment?   -> §19 (storage)
Does it explain, reproduce, or audit what happened?           -> §20 (provenance)

Most real outcomes hit 2-4 flows; run each one and union the
Required-selections IDs. Then resolve IDs through REFERENCE §2.
```

### 3c. Operation-selection ladder (`delta-align` §2.3), bound to sections

```text
DeltaTable read / snapshot / time travel      delta §3.3-§3.9    EXT-01
  -> Delta TableProvider / DataFusion         delta §6.5, §7.1   EXT-01
  -> DeltaTable::write / BlindDeltaTable      delta §5.3, §3.27  EXT-02/-06
  -> delete / update / merge builders         delta §9.4-§9.13   EXT-02
  -> create / add_columns / constraints /
     metadata / add_feature builders          delta §8.2, §4.12,
                                              §11.3, §11.14      EXT-03
  -> scan_cdf                                 delta §10.5        EXT-04
  -> optimize / vacuum / restore /
     filesystem_check                         delta §13.4, §13.13,
                                              §13.19, §13.20     EXT-04
  -> FileSelection targeted reads             delta §6.35        EXT-05
  -> kernel transaction APIs                  delta §8.25        EXT-07
  -> LogStore / raw actions / object store     delta §2.1, §8.18  EXT-08

Take the FIRST level that fully preserves the required semantics
(Principle 14; delta-align §2.3). Descending is never justified by code
organization or performance speculation, and each step down must be
recorded in an OperationSelectionRecord (delta-align §33).
```

### 3d. Capability-status legend → required evidence (`delta-align` §0.1)

Eight statuses — note this is **not** the sibling manual's six-status legend, and the two
must not be substituted for one another.

```text
NATIVE AUTHORITY        -> Delta owns the durable truth; do not duplicate it.
                           evidence = identity/version resolution tests
NATIVE ENFORCEMENT      -> rely only inside the documented feature/operation scope.
                           evidence = scope-edge and violation tests
NATIVE STATE TRANSITION -> model as version N + operation -> version N+1.
                           evidence = before/after version assertions
NATIVE OBSERVABILITY    -> durable history/metrics/CDF describing behavior.
                           evidence = artifact-join tests against app provenance
INTEGRATION CONTRACT    -> a provider/log-store/object-store/builder boundary.
                           evidence = lifecycle + capability-truth tests
COMPOSITION PATTERN     -> Delta + Arrow/DataFusion/application models together.
                           evidence = seam tests proving the boundaries survived
APPLICATION OVERLAY     -> Delta supplies artifacts, not the capability.
                           evidence = the overlay's own tests + links to native artifacts
CAUTION                 -> partial, retention-sensitive, version-coupled, or
                           operation-specific. evidence = fail-closed behavior,
                           pinned-version tests, recorded uncertainty
```

### 3e. Release-gate walk (`delta-align` App. D, line 2841)

The Delta analogue of an upgrade-gate sweep. Run it whenever the pin, the protocol
surface, or a maintenance policy changes.

```text
DEPENDENCY        -> exact rev, DF/Arrow/object_store pins, no duplicate type universes
                     (repo: just stable-graph-check)
SNAPSHOT/AUTHORITY-> exact version recorded per input; provider/cache keys carry it;
                     same-version checkpoint is identity-neutral; multi-table results
                     use a publication manifest
SCHEMA/PROTOCOL   -> schema fingerprint, reader/writer features, operation-specific
                     feature matrix, nested optionality + partition-name collisions
TRANSACTION       -> idempotency/retry model, unknown-commit reconciliation,
                     num_retries telemetry, backend commit safety
CDF               -> DV fixtures, ICT/fallback, consumer checkpoint by version,
                     retention guard covering every consumer
QUERY             -> Delta provider not raw Parquet, session/runtime mapping,
                     provider refresh/pin tests, EXPLAIN/pruning regressions
MAINTENANCE       -> optimize logical equivalence, vacuum dry-run/retention/keep-version,
                     deep-partition benchmark, restore + filesystem-check incident tests
INTEROP           -> certified engine/feature set, path-encoding fixture, Python interop
PROVENANCE        -> before/after versions and spec/config/code/environment IDs,
                     no secrets in commit metadata, closure/replay check
```

---

## §4 — Operating rules

1. **Seek by line, cite by section.** Line numbers live only in REFERENCE §1/§2 tables;
   citations everywhere else use `delta §N` / `delta-align §N` / `App. X` identifiers.
2. **`delta` has no §1.** The spine is §0, §2–§13. A citation to §1 of that document is
   invalid, and a "§0–§13" range claim is wrong.
3. **Use the anchored heading pattern** from §1.1. Fourteen of the file's `^# ` lines are
   shell/TOML comments inside code fences, so a bare `rg '^# '` or a bare `just
   lib-outline` over `delta` is roughly one-third noise.
4. **`delta` §13.29 is ambiguous by construction.** Two subsections share the number;
   always disambiguate by title ("Best practices" vs the latest-pin `OPTIMIZE`
   interoperability note).
5. **Read the latest-pin note before trusting a chapter.** Twelve chapters append a
   `43a0cf10`-specific correction after their Value case (§3.27–§3.28, §4.37, §5.29,
   §6.35–§6.36, §8.25, §9.35, §10.29, §11.27, §12.33, §13.29-second). They outrank the
   ordinary prose above them.
6. **Cite `principles` by principle number and title, never by h1 ordinal** — the ordinal
   is principle + 1 by construction.
7. **The predecessor `9f922319…` reference is never a current API authority.** It exists
   only for historical comparison; `delta` §0.16 is the reviewed net-change map between
   the two revisions.
8. **Version pins come from `FAB §2.1` and the session context**, never from a document.
   In particular `delta-align`'s banner states Rust `1.94.1` — that is delta-rs's own
   minimum, not the repository floor, which is `1.95.0` (set by the Ruff provider train).
   Do not copy `1.94.1` into a tracked file as the CodeFabric pin.
9. **A pattern ID is never implementable without its `delta-align` Part II row.** The row
   carries required leverage, principles, and minimum evidence; REFERENCE §2 carries only
   the reading locations.
10. **Honor `delta-align` §1.3 stop conditions.** Twelve of them; if any holds, the work
    stays at design. "Latest" with no freshness semantics and a retryable write with no
    reconciliation strategy are the two that recur most.
11. **`delta-align` §1.2 outputs are not all Part IV templates** (see §1.4's hazard). When
    a step names an artifact with no template, produce the content the step asks for and
    say so — do not invent a schema and present it as the manual's.
12. **Absence degradation.** If a routed document is missing, say so explicitly, continue
    with the documents that exist, and never reconstruct pattern IDs, principle numbers,
    or section claims from memory.
13. **Untracked-document drift rule.** If a line/title probe mismatches any §1 table,
    re-derive that document's spine with the embedded command before trusting anything
    else cited against it; report the drift.
14. **`just lib-outline <path> --view names` before reading a `delta` chapter** —
    chapters run 1,000–2,000 lines across 401 subsections; zoom first, or jump straight
    from §1.2.
15. **Non-Delta DataFusion and Arrow work routes to `datafusion-pyarrow-rust-ref`**
    (tree 3a arbitrates); fact providers → `code-facts-lib-ref`; graph analytics →
    `petgraph-ref`; canonical JSON and digests → `canonicalization-lib-ref`.
16. **A symbol marked absent in §1.2 is absent from the document, not from the library.**
    `with_max_retries` and `with_application_transaction` are both used in this repository
    and neither is documented in `delta`; verify such symbols against the pinned source
    and never fabricate a section citation for them.
17. **Evidence over restatement.** When this skill's guidance and a cited section seem to
    disagree, open the section; if the disagreement survives, the document wins and this
    skill needs a fix — flag it.

---

## §5 — Project context: CodeFabric

**Source map** (where these patterns land in this repository):

| Path | Role | Dominant families |
|---|---|---|
| `src/fabric/snapshot_catalog.rs` | exact-version provider construction and caching | STA, QRY-03–05, MOD-07 |
| `src/fabric/overlay.rs` | effective overlay provider, explicit statistics disposition | QRY, LAY-03–04, SCH-11 |
| `src/fabric/serving.rs` | query evidence, pruning, resources, plan diagnostics | QRY-06–09, LAY, OBS |
| `src/fabric/mutation.rs` | governed commits: `CommitProperties`, app transaction, retries=0 | TXN-01–07, WRT, DML |
| `src/fabric/publication.rs` | multi-table publication and snapshot activation | MOD-07, OBS-03, STA-03 |
| `src/fabric/result_checksum.rs` | durable result identity over query output | MOD-06, OBS-08 |
| `src/contracts/models.rs` | wire models carrying table/version identity | SCH-10, OBS-03 |
| `src/bin/codefabric_model/aggregate_driver.rs` | model-compiler side Delta usage | MOD, SCH |
| `scripts/stable_graph_check.sh` | exact revision, feature, and type-universe enforcement | INT-10, STO-08, TST-14 |

**Constraints** (repository invariants these documents serve):

- **Coordination is application-owned.** `CommitProperties::default().with_max_retries(0)`
  in `src/fabric/mutation.rs`: CodeFabric owns retries, application transactions,
  predecessor checks, and unknown-outcome reconciliation (TXN-02–04, TXN-07).
- **A query pins one immutable snapshot** and exact Delta versions per table; a
  `DeltaTable` handle or provider is a pinned view, never "latest forever"
  (STA-03/-10, MOD-07, QRY-03; `delta-align` App. B).
- **Never bypass the transaction log** with raw Parquet reads or object-store listings
  (QRY-01, STO-02; `delta-align` Part VII row 1).
- **Query-serving handles retain full statistics.** Metadata-only and `without_files`
  profiles are separate, deliberate constructions (STA-06/-07, LAY-07).
- **Reopen validation fails closed** on unapproved CDF, deletion vectors, type widening,
  protocol, or table features (GOV-05/-06, SCH-08, INT-05).
- **Local-workstation authority excludes `deltalake-aws` and the AWS SDK**; only the
  `s3-storage` feature activates that implementation, and kernel-forced latent
  `object_store` cloud features are reported, not mistaken for runtime authority
  (STO-08, INT-10; `scripts/stable_graph_check.sh`).
- **Vacuum is destructive to retained versions.** Dry-run plus reference/lease safety
  remain mandatory; this skill does not authorize production vacuum orchestration
  (MNT-04–06, GOV-08/-09; `delta-align` §31 `MaintenanceSafetyReview`).
- Python/FastMCP remains presentation-only — no Delta, Arrow, or DataFusion processing in
  the adapter (`AGENTS.md` invariant 7).

**Spec anchors.** Cite specs as `FAB §N` etc. by section number, never line number.
`FAB §2.1` is the pin authority; the atomic-present-state doctrine (owner-scoped
replacement plus manifest-pinned multi-table MVCC) is what MOD-07 and OBS-03 implement
here, and `LIFE §§157–159` lists the consistency, performance, and failure invariants any
incremental-update design must preserve. `docs/spec_index/library-routing.md` maps spec
sections to chapters here.

**Boundary discipline.** Non-Delta DataFusion/Arrow → `datafusion-pyarrow-rust-ref`; fact
providers → `code-facts-lib-ref`; graph analytics → `petgraph-ref`; canonical JSON and
digests → `canonicalization-lib-ref`; the adapter wire → `grpcio-orjson-protobuf-ref` and
`fastmcp-pydantic-ref`.
