# CodeFabric design changes recommended for delta-rs `1.0.0` pin `35cfed45…` → `9f922319…`

**Artifact kind:** Design-impact and change-recommendation document  
**Status:** Proposed; not yet normative  
**Reference date:** 2026-08-20  
**Prior delta-rs pin:** `35cfed4545f41c2f483706d29670f7cc2fe7e217`  
**Recommended delta-rs pin:** `9f9223197469897ef05ae4369eb4fd1390174e65`  
**Upstream crate version:** `deltalake` / `deltalake-core` `1.0.0` pre-release line  
**Unchanged ecosystem anchors:** DataFusion `54.0.0`, Arrow/Parquet `58.3.0`, `object_store 0.13.2`  
**New Rust floor:** `1.94.1`  
**Primary affected CodeFabric artifact:** `present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md`

---

## Executive conclusion

Updating CodeFabric from the prior delta-rs pin to `9f922319…` is advisable. The new pin does **not** require a change to CodeFabric's ontology, semantic query model, hot-overlay model, multi-table publication model, or `ServingSnapshot` consistency semantics. Those designs remain sound.

The upgrade does, however, justify several concrete changes to the implementation specification and conformance suite:

1. **Restamp the Rust/dependency baseline** to Rust `1.94.1` and delta-rs `9f922319…`, and move the CodeFabric Rust workspace to Cargo resolver `3`.
2. **Stop treating the Delta kernel as a separately git-pinned dependency.** The new delta-rs pin uses released `buoyant_kernel` / `buoyant_kernel_engine` 0.25.x crates.
3. **Formalize a snapshot-scoped durable-provider cache inside each CodeFabric `ServingSnapshot`.** Build/reuse exact-version Delta DataFusion providers once per active snapshot rather than reconstructing table state per query.
4. **State explicitly that delta-rs materialized-file/snapshot caches are ephemeral execution state, never CodeFabric source of truth.** CodeFabric's exact publication table-version map remains authoritative.
5. **Make checkpoint arrival at the same Delta version identity-neutral.** A newly written checkpoint may change replay mechanics without changing publication identity or CodeFabric freshness.
6. **Separate publication validation from lazy query activation.** The newer lazy snapshot path can defer active-file/stat work; CodeFabric must not mistake cheap provider construction for publication validation.
7. **Keep query-serving Delta handles statistics-capable.** Do not adopt `skip_stats=true` for normal CPG query providers merely because delta-rs can now replay stronger internal stats capabilities on demand.
8. **Add nested-schema `OPTIMIZE` regression cases** covering Spark-style physical nullability and nested fields whose names collide with top-level partition columns.
9. **Treat Delta action paths as opaque URI identities.** Do not manually encode/decode `Add`/`Remove` paths in maintenance code.
10. **Update the table-feature compatibility registry** to recognize `V2Checkpoint` as supported by the pinned delta-rs reader/writer protocol checker, while leaving it disabled by default for CodeFabric-created tables.
11. **Expand performance acceptance testing** to measure snapshot activation/open cost separately from first-query and steady-state query cost, because lazy replay can move work between those phases.

The changes should be released as a synchronized CodeFabric **1.4** design release if the current 1.3 artifacts are treated as immutable released specifications. The externally visible semantic contracts can remain schema-compatible with 1.3; the release change is principally an implementation/data-fabric baseline and conformance update.

---

# 1. Source basis

## 1.1 Current CodeFabric design being assessed

The current CodeFabric 1.3 data-fabric specification establishes the following relevant invariants:

```text
Delta Lake = durable transactional table-state authority

durable publication
  -> exact Delta version for every required table

ServingSnapshot
  -> exact durable base publication and table-version map
  -> consolidated hot overlay
  -> analysis contexts / capabilities / diagnostics / bundle identities

query
  -> lease exactly one immutable ServingSnapshot
  -> execute all planning, DataFusion reads, graph traversal, and source context
     against that same snapshot
```

It also specifies:

- manifest-pinned multi-table MVCC because Delta transactions are per-table;
- one active `ServingSnapshot` pointer per workspace;
- owner-scoped replacement rather than uncontrolled full-table overwrite;
- Arrow/Delta schema bundles and exact table-version publication manifests;
- DataFusion as the query/relational execution plane;
- compaction/Z-order/vacuum as storage maintenance, not semantics;
- snapshot leases as the guard against premature Delta vacuum;
- CDF disabled by default;
- column mapping and type widening disabled by default;
- query-serving statistics/pruning enabled.

These core choices remain valid.

## 1.2 Upstream delta-rs change range

The new target is 20 commits ahead of the previous pin. The changes with architectural or operational relevance are concentrated in a few families:

| Upstream change family | Representative commits | CodeFabric significance |
|---|---|---|
| Lazy snapshot/materialization invariants | `53d4475a…`, `0b83064c…`, `439583af…`, `95ad71d1…`, `84fad0b1…` | **High** |
| Delta kernel 0.25 / default-engine split | `bdcd2526…`, `c19b0d99…`, `fd7e9691…` | **Medium** |
| Spark nested-nullability `OPTIMIZE` fix | `ee55e35f…` | **High for maintenance correctness** |
| Nested field/partition-name collision fix | `9f922319…` | **High for nested maintenance correctness** |
| CDF in-commit timestamps | `7fba644d…` | Low for current CodeFabric because CDF is disabled |
| `V2Checkpoint` protocol support | `3f562682…` | Low/medium compatibility improvement |
| `TimestampNanosNtz` | `7551776c…` | Low because CodeFabric should retain current timestamp profile |
| Action-path URI encoding | `3b1734ba…` | Medium for custom maintenance/log tooling |
| Rust MSRV 1.94.1 | `e8072a63…` | **Required build/tooling change** |
| Python sdist release plumbing | `b2ff9314…` | None for Rust CodeFabric |

Primary upstream comparison:

`https://github.com/delta-io/delta-rs/compare/35cfed4545f41c2f483706d29670f7cc2fe7e217...9f9223197469897ef05ae4369eb4fd1390174e65`

---

# 2. Required baseline changes

## REC-DL-01 — Update the canonical CodeFabric Rust / delta-rs baseline

### Current CodeFabric 1.3 contract

The current data-fabric specification pins:

```toml
[workspace]
resolver = "2"

[workspace.package]
edition = "2024"
rust-version = "1.91.1"

[workspace.dependencies]
datafusion = "=54.0.0"
arrow = "=58.3.0"
parquet = "=58.3.0"
object_store = "=0.13.2"

deltalake = {
  git = "https://github.com/delta-io/delta-rs.git",
  rev = "35cfed4545f41c2f483706d29670f7cc2fe7e217",
  default-features = false,
  features = ["rustls", "datafusion", "s3"]
}
```

### Recommended replacement

```toml
[workspace]
resolver = "3"

[workspace.package]
edition = "2024"
rust-version = "1.94.1"

[workspace.dependencies]
datafusion = "=54.0.0"

arrow = "=58.3.0"
arrow-array = "=58.3.0"
arrow-buffer = "=58.3.0"
arrow-schema = "=58.3.0"
arrow-cast = "=58.3.0"
arrow-select = "=58.3.0"
arrow-ord = "=58.3.0"
arrow-string = "=58.3.0"
arrow-row = "=58.3.0"

parquet = { version = "=58.3.0", features = ["arrow", "async", "object_store"] }
object_store = "=0.13.2"

deltalake = {
  git = "https://github.com/delta-io/delta-rs.git",
  rev = "9f9223197469897ef05ae4369eb4fd1390174e65",
  default-features = false,
  features = ["rustls", "datafusion"]
}
```

For the mandatory `local-workstation-v1` CodeFabric profile, I recommend **removing `s3` from the default delta-rs feature set** and exposing it through an explicit future/cloud build feature instead. This does not avoid delta-rs's declared Rust 1.94.1 floor, but it reduces unnecessary AWS dependency/compile surface for the local daemon.

Example CodeFabric feature mapping:

```toml
[features]
default = ["local-workstation"]
local-workstation = []
s3-storage = ["deltalake/s3"]
```

If S3 support is intentionally mandatory even in the local baseline, retain `s3`; this recommendation is dependency hygiene rather than a correctness requirement.

### Why resolver 3

The upstream delta-rs workspace now uses Cargo resolver `3`, and CodeFabric already uses Rust 2024. Resolver 3 is the correct modern workspace posture and participates in Rust-version-aware dependency resolution. It also aligns the application's dependency-resolution model with the pinned upstream stack.

### Toolchain file

```toml
# rust-toolchain.toml
[toolchain]
channel = "1.94.1"
components = ["rustfmt", "clippy", "rust-analyzer"]
```

Update every CI command and developer bootstrap assumption from 1.91.1 to 1.94.1.

---

## REC-DL-02 — Remove any independent git pin for the Delta kernel

At the prior delta-rs revision, the dependency graph contained a separately git-pinned `buoyant_kernel` 0.24 line. The new revision consumes released 0.25.x packages:

```toml
delta_kernel = {
  package = "buoyant_kernel",
  version = "0.25.0,<0.25.100",
  features = ["arrow-58", "internal-api"]
}

delta_kernel_default_engine = {
  package = "buoyant_kernel_engine",
  version = "0.25.0,<0.25.100",
  default-features = false,
  features = ["arrow-58", "rustls"]
}
```

### CodeFabric design rule

CodeFabric SHOULD **not** add those dependencies directly merely because delta-rs uses them. The `deltalake` git pin plus committed `Cargo.lock` should select the matching pair transitively.

Only a CodeFabric crate that intentionally consumes kernel APIs should declare them directly. If such use becomes necessary:

- depend on both `buoyant_kernel` and `buoyant_kernel_engine` where engine facilities are required;
- isolate kernel internals behind one application-owned adapter module;
- do not expose kernel types across CodeFabric public crate boundaries;
- gate the dependency with compile tests against the exact delta-rs SHA.

This change reduces the number of unpublished upstream SHAs CodeFabric must coordinate.

---

## REC-DL-03 — Update generated build/release identity and compatibility fingerprints

The CodeFabric 1.3 `ServingSnapshot` and publication manifests already pin toolchain/provider/schema/bundle identities. The delta-rs revision and Rust toolchain are therefore part of the reproducibility boundary even though they do not alter CPG fact meaning.

The release manifest should record at minimum:

```text
delta_rs_git_rev = 9f9223197469897ef05ae4369eb4fd1390174e65
deltalake_declared_version = 1.0.0-pre-release
rust_version = 1.94.1
datafusion_version = 54.0.0
arrow_version = 58.3.0
parquet_version = 58.3.0
object_store_version = 0.13.2
cargo_lock_digest = ...
```

The change requires regeneration of the CodeFabric canonical build/deployment bundle digest. It does **not** require new ontology IDs, query phrase IDs, fact preimages, or schema bundle IDs unless an unrelated schema change is made at the same time.

---

# 3. Recommended durable-snapshot architecture refinements

## REC-DL-04 — Keep CodeFabric `ServingSnapshot` as semantic truth; treat delta-rs `Snapshot` as an execution mechanism

This distinction should become explicit in the normative data-fabric specification because both layers now use the word “snapshot.”

### CodeFabric snapshot

```text
ServingSnapshot
  = durable publication + exact table-version map
  + hot overlay
  + source generation
  + analysis contexts
  + capabilities / diagnostics
  + exact interpretation bundle digests
```

It is the only query pin.

### delta-rs snapshot

```text
delta-rs Snapshot / EagerSnapshot
  = storage-engine representation of one Delta table version
  + log replay/materialization/cache state
```

It is an implementation object for reading one durable table.

### Normative rule to add

> A delta-rs `Snapshot`, `EagerSnapshot`, materialized-file cache, checkpoint selection, or DataFusion `TableProvider` SHALL NOT independently define CodeFabric current-state identity. CodeFabric current-state identity is defined only by the leased `ServingSnapshot` and its exact durable Delta table-version map plus overlay identity.

This prevents an attractive but incorrect simplification where the newer delta-rs snapshot model would be allowed to substitute for CodeFabric's multi-table/overlay consistency object.

---

## REC-DL-05 — Build one immutable durable-provider set per CodeFabric `ServingSnapshot`

The current data-fabric architecture already creates an overlay-aware DataFusion catalog. The new delta-rs snapshot model makes it more attractive to make the **durable provider set itself snapshot-scoped and reusable**.

Recommended implementation shape:

```text
ServingSnapshot
  ├─ durable publication metadata
  ├─ DeltaBaseCatalog
  │    ├─ table_code -> exact delta_version
  │    ├─ table_code -> Arc<dyn TableProvider>
  │    └─ table_code -> table-root / schema identity diagnostics
  ├─ consolidated hot overlay
  ├─ capability index
  └─ diagnostics index
```

Conceptual Rust abstraction:

```rust
struct DurableTableProviderHandle {
    table_code: TableCode,
    delta_version: u64,
    provider: Arc<dyn datafusion::catalog::TableProvider>,
}

struct DeltaBaseCatalog {
    publication_id: PublicationId,
    providers: BTreeMap<TableCode, DurableTableProviderHandle>,
}
```

The application abstraction should store the DataFusion provider and CodeFabric identity metadata, **not expose or serialize delta-rs internal snapshot/cache types**.

### Activation flow

```text
1. Read immutable durable publication manifest.
2. For each required table, resolve exact Delta version from publication_table.
3. Construct an exact-version delta-rs table/snapshot/provider.
4. Register the provider in the candidate snapshot's private DataFusion catalog.
5. Wrap it with the CodeFabric overlay-aware provider.
6. Run activation integrity checks.
7. Freeze the catalog/provider set.
8. Atomically activate the new ServingSnapshot Arc.
9. All query leases reuse those exact provider objects.
```

### Why this is beneficial

- avoids reopening/replaying the Delta log independently for every semantic query;
- aligns provider lifetime exactly with CodeFabric snapshot lease lifetime;
- makes the table-version map and provider set impossible to drift within one query;
- exploits delta-rs's snapshot-aware provider path;
- allows newer lazy active-file/stat replay to happen on demand while preserving exact-version identity.

The current delta-rs `TableProviderBuilder` can accept a `Snapshot` directly, and when a snapshot is supplied, provider construction does not need to rediscover the table version.

---

## REC-DL-06 — Treat delta-rs materialized-file caches as ephemeral and rebuildable

The latest delta-rs changes introduce an internal `SnapshotIdentity` covering:

```text
table root
Delta version
checkpoint version
protocol
metadata
```

Materialized file state is reused only if it matches the snapshot and the requested stats capability. Identity-less or mismatched caches are rejected/ignored.

CodeFabric should deliberately **not duplicate this internal cache state into SQLite, Delta control tables, or `ServingSnapshot` wire metadata**.

Add this rule:

> Delta file-materialization caches are non-authoritative process-local accelerators. They MAY be retained by delta-rs/provider objects for the lifetime of a leased `ServingSnapshot`, but SHALL be reconstructible solely from the table root, pinned Delta version, and storage configuration. They SHALL NOT participate in semantic digests, publication equality, or query completeness proofs.

### Application-level cache key

If CodeFabric has its own provider/cache map, key it at a higher level by:

```text
workspace_id
publication_id
table_code
delta_table_root_identity
delta_version
schema_bundle_digest
```

Do not key by “latest” or by table name alone.

---

## REC-DL-07 — Make same-version checkpoint refresh identity-neutral

The latest snapshot implementation can rebuild a snapshot at the **same Delta version** if a newly created checkpoint becomes available after the snapshot was originally loaded.

For CodeFabric, this leads to an important explicit rule:

> Checkpoint creation or adoption at an already pinned Delta version is a physical replay optimization and SHALL NOT create a new durable publication, source generation, semantic snapshot identity, or freshness generation by itself.

Example:

```text
Before:
  entity table version = 421
  snapshot replay = JSON commits after checkpoint 400

Later maintenance:
  checkpoint 421 is created

After refresh:
  entity table version = 421
  snapshot replay = checkpoint 421

CodeFabric semantic identity:
  unchanged
```

A new `ServingSnapshot` may still be constructed for operational reasons, but its logical durable-base content digest should compare equal if no Delta table version or overlay content changed.

### Why this matters

Without this rule, background checkpoint creation could spuriously:

- advance freshness generations;
- invalidate query caches;
- generate meaningless publication churn;
- create false “state changed” notifications.

---

## REC-DL-08 — Separate lazy provider construction from publication validation

The newer delta-rs snapshot path can postpone active-file and statistics materialization. That is useful for query-serving latency and memory, but it creates a potential architecture mistake:

```text
cheap provider construction != durable publication validation
```

CodeFabric 1.3 already says a `ServingSnapshot` is built off-path, validated, then activated. Preserve and strengthen that requirement.

### Publication validation MUST still establish

```text
exact requested Delta table version exists
schema digest matches schema registry
protocol/features are compatible
table metadata/partition contract is correct
publication table checksums/counts pass
owner/relation integrity queries pass
all required tables are present in the publication manifest
cross-table publication invariants pass
```

Where publication integrity requires enumerating active files or reading fact rows, the validator must explicitly perform that work. It must not infer correctness because the delta-rs provider object was constructed successfully.

### Query activation MAY remain lazy

Once durable publication validation has succeeded, the `ServingSnapshot` provider set may defer file/stat replay until a query needs it.

This separation preserves correctness while taking advantage of the newer lazy engine behavior.

---

# 4. Statistics and pruning policy

## REC-DL-09 — Keep `skip_stats=false` for CodeFabric query-serving providers

The latest delta-rs internal logic can replay active adds with stronger statistics capabilities when an internal operation explicitly requires them, even if the resident materialized cache was created without stats. This is useful hardening, but it should **not** be interpreted as a reason to disable statistics on the primary query path.

The public `DeltaTableConfig::skip_stats` contract still states that a normal predicated query using an instance whose cache has no stats may scan every file; partition pruning is separate.

### CodeFabric access profiles

Add an explicit implementation profile table:

| Access profile | `require_files` / materialization posture | `skip_stats` | Purpose |
|---|---|---:|---|
| `QUERY_SERVING` | exact-version provider; lazy replay permitted | **false** | normal semantic/DataFusion queries |
| `PUBLICATION_METADATA` | metadata-first / lazy | may be true only if no pruning/data scan is performed | schema/protocol/table-version validation |
| `APPEND_ONLY_WRITER` | metadata-first where safe | may be true | writes that do not inspect existing files |
| `VACUUM_FILESYSTEM_CHECK` | operation-specific | true may be appropriate | maintenance without query pruning |
| `OPTIMIZE_DML` | active files/stats as required by operation | false/default unless upstream operation owns stronger replay | rewrite maintenance |

### Normative rule

> A CodeFabric query-visible Delta provider SHALL NOT be created with a stats-skipping configuration unless the query planner can prove that no data-skipping predicate will be required and the performance regression is explicitly accepted.

This aligns with the current Data Fabric §98 requirement that metadata/file/statistics caching and Parquet pruning be enabled.

---

# 5. Maintenance and nested-schema correctness

## REC-DL-10 — Add two mandatory `OPTIMIZE` nested-schema regressions

The new pin fixes two concrete failures in the Delta/DataFusion scan path.

### Case A — Spark-style nested physical nullability

Logical Delta schema:

```text
meta: STRUCT<int_id: STRING NOT NULL> NULLABLE
```

Physical Parquet written by Spark:

```text
meta.int_id physically optional
```

Prior failure mode:

```text
Cannot cast nullable struct field 'int_id' to non-nullable field
```

The new pin relaxes nested nullability for physical read/adaptation and then restores the strict logical Delta schema after the scan. Actual null data that violates the logical contract still fails validation.

### Case B — nested field name equals top-level partition column

Example schema:

```text
date STRING                      -- top-level partition column
properties STRUCT<date: STRING> -- ordinary nested field
```

The current tip ensures only the top-level `date` is treated as a partition-field candidate. The nested `properties.date` must retain its normal string representation through scan and `OPTIMIZE`.

### Why CodeFabric needs these tests

The CodeFabric schema registry already includes bounded `List`, `Map`, and `Struct` payloads. Even if the current most frequently optimized tables are flat, the maintenance subsystem is generic across Delta tables and should certify the nested cases once rather than encode table-specific exceptions.

Add to Data Fabric §112.3:

```text
- optimize Spark-style physically nullable nested fields under stricter Delta logical schema;
- optimize table with nested field name equal to top-level partition column;
- assert pre/post optimize logical schema digest equality;
- assert pre/post optimize row/content digest equality;
- assert no nested dictionary/partition coercion leakage.
```

---

## REC-DL-11 — Keep CodeFabric schema policy strict; do not weaken nullability to accommodate Parquet

The new Delta-aware adapter fixes the physical/logical mismatch at the proper layer. Therefore CodeFabric should **not** respond by making nested fields nullable in its canonical schema merely to improve interoperability.

Maintain the existing rule:

```text
Delta logical schema = authoritative CodeFabric durable table contract
Arrow batches = must conform to the logical contract
Parquet physical nullability = storage encoding detail handled by delta-rs adapter
```

This is an upstream fix that reduces the need for application workarounds.

---

# 6. Delta action-path handling

## REC-DL-12 — Treat `Add.path` / `Remove.path` / CDF action paths as opaque Delta URI paths

The latest pin fixes serialization of spaces in Delta transaction-log action paths:

```text
part 0.parquet -> part%200.parquet
```

while preserving:

```text
/  path hierarchy
=  Hive partition delimiter
```

and preserving compatibility with already encoded Spark paths and legacy unencoded spaces.

### CodeFabric rule

If CodeFabric maintenance code ever inspects Delta file actions directly:

```text
- use delta-rs Path/URL/object-store facilities;
- never construct transaction-log action paths by string concatenation;
- never decode an action path for display and reuse the display string as identity;
- never compare CodeFabric source-path byte identity to Delta Parquet action paths;
- keep source-path ontology identity and storage-file identity as separate domains.
```

This is particularly important because CodeFabric already has a sophisticated byte-safe source-path contract. That contract governs **source files inside analyzed workspaces**, not Delta's own Parquet/log object paths.

No ontology change is required.

---

# 7. Table-feature compatibility

## REC-DL-13 — Recognize `V2Checkpoint`, but do not enable it by default

The pinned delta-rs `ProtocolChecker` now includes:

```text
Reader features:
  ...
  V2Checkpoint

Writer features:
  ...
  V2Checkpoint
```

### Recommended CodeFabric change

Update any delta-rs capability/feature compatibility table in the Data Fabric implementation notes and conformance fixtures so that a table declaring `V2Checkpoint` is not rejected **solely because the selected delta-rs build does not understand the feature**.

### Do not change table-creation defaults yet

CodeFabric-owned tables should continue to use the existing checkpoint policy unless a separate benchmark/design exercise proves value from V2 checkpoint authoring.

Reason:

- protocol-checker recognition is not the same as a fully designed CodeFabric V2 checkpoint maintenance policy;
- the current upgrade does not require V2 checkpoints for correctness;
- CodeFabric's durable publication identity is table-version based and independent of checkpoint format.

Proposed policy:

```text
v2Checkpoint read compatibility: ALLOWED
v2Checkpoint existing-table write compatibility: ALLOWED_BY_LIBRARY
CodeFabric create/enable by default: NO
CodeFabric maintenance rollout: BENCHMARK_AND_CONFORMANCE_REQUIRED
```

---

# 8. CDF and timestamp changes: no current design change

## 8.1 In-commit timestamps

The latest delta-rs CDF path now uses `CommitInfo.inCommitTimestamp` when present and falls back to ordinary `CommitInfo.timestamp`.

CodeFabric 1.3 explicitly creates durable tables with **CDF disabled by default** and does not use CDF as the CPG lifecycle/update mechanism. Therefore:

```text
CodeFabric source freshness: unchanged
CodeFabric publication ordering: unchanged
ServingSnapshot identity: unchanged
owner update lifecycle: unchanged
```

If a future CodeFabric profile adopts CDF for external replication, checkpointing should still be by Delta version rather than timestamp.

## 8.2 `TimestampNanosNtz`

The new pin adds timezone-less nanosecond timestamp support under the existing `nanosecond-timestamps` feature.

CodeFabric should **not enable that feature merely because the type now exists**. Continue using the canonical timestamp precision/type profile defined by the schema registry unless a concrete fact-domain requirement for nanoseconds is established and cross-engine round trips are certified.

No schema migration is recommended.

---

# 9. Performance and observability changes

## REC-DL-14 — Split Delta activation/replay metrics from query metrics

The new lazy snapshot architecture can move work from “open/activate table” to “first operation requiring active files/stats.” A single end-to-end query latency metric can hide that shift.

Add metrics around the durable-provider lifecycle:

```text
delta_snapshot_open_ms
delta_provider_build_ms
delta_provider_activation_count
delta_first_scan_ms
delta_first_predicated_scan_ms
delta_table_version
delta_checkpoint_version_if_available   -- diagnostic only, not semantic identity
delta_active_file_count_when_materialized
delta_materialization_reason            -- QUERY | VALIDATION | DML | OPTIMIZE | CONFLICT_CHECK
delta_stats_policy_class                 -- diagnostic abstraction, not upstream private enum
```

Do not depend on private upstream enum/type names in telemetry contracts. Map them into CodeFabric-owned diagnostic categories.

### Acceptance benchmark matrix

For representative small/medium/large CodeFabric tables, capture:

| Metric | Old pin | New pin | Gate |
|---|---:|---:|---:|
| daemon durable-base activation latency | | | no material regression |
| activation peak RSS | | | should improve or remain bounded |
| first filtered query latency | | | bounded |
| steady-state filtered query p50/p95 | | | no material regression |
| unfiltered full scan | | | no material regression |
| owner replacement conflict-check latency | | | bounded |
| optimize nested table | | | correctness + timing |
| table reopen after checkpoint creation | | | same logical version/result |

The new pin should be accepted based on actual CodeFabric table shapes, not upstream synthetic benchmarks alone.

---

## REC-DL-15 — Add a first-query warm-up option, but do not require it globally

For very hot tables where p99 first-query latency matters, CodeFabric MAY warm selected exact-version providers during `ServingSnapshot` activation with a bounded metadata/cheap query that forces the required file/stat state into the provider's normal execution path.

Do this only for tables proven hot by measurement.

Recommended policy:

```text
cold/seldom queried extension tables:
  leave lazy

entity / relation / high-frequency typed detail tables:
  benchmark optional warm-up

control tables:
  eager cost is usually negligible
```

Warm-up is a performance policy, not part of snapshot correctness.

---

# 10. Conformance and fault-injection additions

## REC-DL-16 — Expand the delta-rs upgrade gate

The current Data Fabric §112 tests should be extended with the following upgrade-specific cases.

### 10.1 Snapshot/cache behavior

```text
[ ] Exact Delta version produces identical CodeFabric logical content with and without a checkpoint at that same version.
[ ] Provider rebuilt after same-version checkpoint creation returns identical row/checksum result.
[ ] Provider/snapshot cache from version N is never reused as version N+1 state.
[ ] Table root mismatch cannot reuse a cached provider/snapshot.
[ ] Daemon restart reconstructs providers solely from publication manifest + Delta versions.
```

### 10.2 Lazy/eager equivalence

```text
[ ] Lazy exact-version provider and fully materialized/eager read return identical rows.
[ ] Metadata-first load and normal load return identical protocol/schema/table metadata.
[ ] First predicated scan after lazy activation returns the same result as steady-state scan.
```

### 10.3 Stats policy

```text
[ ] QUERY_SERVING provider retains pruning capability.
[ ] A deliberately stats-skipped query test demonstrates the expected loss of file pruning and is never used as production default.
[ ] Partition pruning remains correct independent of file stats.
```

### 10.4 `OPTIMIZE`

```text
[ ] Spark-style nested nullability fixture.
[ ] Nested field name matches top-level partition column.
[ ] Logical schema digest identical before/after optimize.
[ ] Publication-pinned old version remains queryable until normal retention/vacuum policy allows removal.
```

### 10.5 Delta action paths

```text
[ ] data-file path containing a space is serialized/reopened correctly.
[ ] Hive partition delimiters remain intact.
[ ] already percent-encoded Spark-style path round-trips without double encoding.
```

### 10.6 Protocol features

```text
[ ] V2Checkpoint-declaring fixture passes generic feature compatibility.
[ ] unsupported feature such as identityColumns/typeWidening still fails closed where unsupported.
[ ] CodeFabric-owned table creation does not enable V2Checkpoint implicitly.
```

---

# 11. Changes to specific CodeFabric specification sections

The following is the recommended propagation map if these recommendations are integrated into the synchronized suite.

| Current 1.3 section | Recommended change | Semantic/API compatibility |
|---|---|---|
| Data Fabric §2 Source basis/version anchors | replace delta SHA and Rust floor | implementation-only |
| Data Fabric §2.1 workspace baseline | resolver `3`, Rust 1.94.1, new SHA; optionally remove default `s3` | implementation-only |
| Data Fabric §2.2 version-alignment invariant | add kernel 0.25/default-engine split as transitive identity | implementation-only |
| Data Fabric §12 durable publication / ServingSnapshot | explicitly distinguish CodeFabric `ServingSnapshot` from delta-rs `Snapshot`; make checkpoint identity-neutral | clarifying, no wire change |
| Data Fabric §12/overlay catalog implementation | add snapshot-scoped immutable `DeltaBaseCatalog` / provider set | internal architecture refinement |
| Data Fabric §67 table creation | recognize V2 checkpoint support but retain current checkpoint default | compatibility-only |
| Data Fabric §68–70 mutation | no change to delete+append normative model | none |
| Data Fabric §97 writer policy | no required change | none |
| Data Fabric §98 DataFusion runtime | add provider lifecycle/access profiles; keep stats/pruning enabled | internal architecture refinement |
| Data Fabric §100 compaction | add nested-schema optimize regressions | correctness test |
| Data Fabric §101 vacuum | no algorithm change; clarify checkpoint files do not define publication identity | clarification |
| Data Fabric §111 metrics | add Delta provider/snapshot activation/replay metrics | observability only |
| Data Fabric §112 Delta/DataFusion tests | add upgrade-specific cases in §10 above | conformance only |
| Data Fabric AC-G-23 leases/vacuum | no lease-model change | none |
| Lifecycle spec | update Rust/build baseline only if it repeats the suite toolchain; no freshness-state change | none |
| Semantic Query spec | no change | fully compatible |
| Ontology spec | no change | fully compatible |
| FastMCP serving spec | no behavior change; only synchronized release/digest metadata if issuing 1.4 | fully compatible |
| Suite Governance/Manifest | new dependency pin/build digest; register conformance additions | release metadata |

---

# 12. Proposed normative text additions

The following language can be propagated nearly verbatim into the next Data Fabric release.

## 12.1 Delta engine snapshot boundary

> **Delta engine snapshots are not CodeFabric serving snapshots.** A delta-rs `Snapshot`, `EagerSnapshot`, checkpoint, materialized-file cache, or DataFusion provider represents one physical Delta table-version execution view. The sole CodeFabric current-state query pin remains one leased `ServingSnapshot`, which owns the exact multi-table Delta version map plus the consolidated hot overlay and interpretation metadata. Delta engine objects are reconstructible accelerators and SHALL NOT independently define fact freshness, completeness, publication identity, or semantic snapshot identity.

## 12.2 Delta provider lifetime

> For each active `ServingSnapshot`, the daemon SHOULD build one immutable set of exact-version Delta `TableProvider`s for the durable base and reuse those providers for all leases on that snapshot. Providers SHALL be discarded when their owning `ServingSnapshot` becomes unreferenced. A provider from one publication/version SHALL NOT be rebound to another publication by mutating its underlying table state.

## 12.3 Delta cache persistence

> Materialized-file and statistics caches internal to delta-rs SHALL remain process-local, non-authoritative, and rebuildable. CodeFabric SHALL NOT serialize such caches as durable current-state truth. Durable recovery requires only the publication manifest, exact table-version map, table roots/storage configuration, schema bundle, and normal CodeFabric operational recovery state.

## 12.4 Checkpoint identity

> The addition, replacement, or later discovery of a Delta checkpoint for an already pinned table version does not change the logical content identity of that table version. Checkpoint choice is a replay optimization and SHALL NOT by itself advance publication generation, source generation, fact generation, or CodeFabric freshness.

## 12.5 Query statistics profile

> Query-serving Delta providers SHALL retain file-statistics/data-skipping capability. `skip_stats=true` MAY be used only by explicitly designated metadata/maintenance/append-only access profiles that do not rely on predicated file skipping. Internal delta-rs replay of stronger stats capabilities does not relax this public query-serving rule.

## 12.6 Physical nested schema adaptation

> Delta logical schema remains authoritative even when Parquet physical nested-field nullability is looser. CodeFabric SHALL rely on the pinned delta-rs/DataFusion schema-adaptation layer rather than weakening canonical schema nullability or scanning Delta-owned Parquet paths through an independent raw-Parquet provider.

## 12.7 Delta action paths

> Transaction-log data-file action paths are Delta/storage URI identities and SHALL be created, encoded, decoded, and compared through delta-rs/object-store path facilities. Display strings SHALL NOT become durable action-path identity. CodeFabric source-path identity is a separate ontology domain and SHALL NOT be conflated with Delta data-file paths.

---

# 13. What should deliberately remain unchanged

The upgrade should **not** be used as an excuse to redesign stable parts of CodeFabric.

## 13.1 Multi-table publication remains necessary

Delta transactions are still atomic per table, not across the full CPG table family. Therefore the current manifest-pinned multi-table publication design remains necessary.

Do not replace:

```text
publication_id -> exact version per required Delta table
```

with “latest version of every table.”

## 13.2 Hot overlay remains necessary

The new delta-rs lazy snapshot model is about reading one durable Delta table version. It does not replace CodeFabric's hot overlay for sub-publication current edits.

Keep:

```text
ServingSnapshot = durable publication + consolidated hot overlay
```

## 13.3 Snapshot leases and vacuum model remain valid

The newer snapshot cache/checkpoint semantics do not weaken the need to keep every Delta version reachable from:

- `current_publication`;
- active `ServingSnapshot`;
- non-expired query/artifact leases;
- crash-recovery publication holds;
- minimum retention.

The existing AC-G-23 design is still correct.

## 13.4 Owner-replacement protocol remains valid

The update does not materially change the tradeoff behind CodeFabric's normative delete-owner-rows then append-replacement approach. `MERGE` remains an optional optimization under the existing criteria.

## 13.5 CDF remains unnecessary for core CPG freshness

CodeFabric already derives freshness from source inventory/watcher/Git interpretation and provider update waves. Delta CDF should not be introduced into that path merely because its timestamp handling improved.

## 13.6 No ontology or query-language expansion

None of the delta-rs changes creates a new CPG fact class or semantic query capability. The ontology/query specs should not gain storage-engine concepts such as:

```text
V2_CHECKPOINT
DELTA_SNAPSHOT_CACHE
IN_COMMIT_TIMESTAMP
PARQUET_NULLABILITY_ADAPTER
```

Those are implementation/operational facts, not program facts.

---

# 14. Rollout sequence

## Wave A — dependency/toolchain restamp

```text
1. Pin delta-rs 9f9223197469897ef05ae4369eb4fd1390174e65.
2. Move Rust toolchain and rust-version to 1.94.1.
3. Move workspace resolver to 3.
4. Re-resolve and commit Cargo.lock.
5. Verify one DataFusion 54 / Arrow 58 / Parquet 58 / object_store 0.13.2 universe.
6. Confirm buoyant_kernel / buoyant_kernel_engine resolve on the 0.25.x line.
7. Run cargo check / clippy / nextest / cargo-deny / cargo-audit under the new toolchain.
```

## Wave B — unchanged-behavior regression

```text
1. Clean-build CodeFabric data-fabric tests.
2. Rebuild golden CPG corpus.
3. Compare durable table logical digests to the old pin.
4. Compare query canonical responses to the old pin.
5. Confirm publication/ServingSnapshot behavior is byte/logically equivalent where expected.
6. Benchmark open/activation/query/maintenance metrics.
```

## Wave C — adopt provider-lifetime refinement

```text
1. Introduce snapshot-scoped DeltaBaseCatalog/provider cache abstraction.
2. Build exact-version providers during candidate ServingSnapshot construction.
3. Reuse providers for all query leases on that ServingSnapshot.
4. Keep delta-rs internal caches opaque/non-persistent.
5. Add same-version checkpoint identity test.
```

## Wave D — maintenance hardening

```text
1. Add nested nullability optimize fixture.
2. Add nested partition-name collision fixture.
3. Add action-path encoding fixture.
4. Add V2Checkpoint compatibility fixture.
5. Re-run optimize/vacuum/lease safety corpus.
```

## Wave E — synchronized design release

If CodeFabric 1.3 is immutable/released:

```text
1. Produce synchronized suite 1.4.
2. Update Data Fabric source basis and implementation contracts.
3. Update suite governance/release manifest and bundle digests.
4. Restamp other synchronized documents to 1.4 without changing semantic bodies unless required.
5. Generate new conformance trace artifacts.
6. Preserve 1.3 as historical released baseline.
```

---

# 15. Acceptance criteria

The delta-rs upgrade is accepted only when all of the following are true:

```text
Dependency / build
[ ] exact delta-rs SHA = 9f9223197469897ef05ae4369eb4fd1390174e65
[ ] Rust 1.94.1 clean build
[ ] resolver 3 lockfile committed
[ ] DataFusion exactly 54.0.0
[ ] Arrow family exactly 58.3.0 where directly pinned
[ ] Parquet exactly 58.3.0
[ ] object_store exactly 0.13.2
[ ] no incompatible duplicate Arrow/DataFusion type universes
[ ] buoyant_kernel / buoyant_kernel_engine on compatible 0.25.x line

Semantic equivalence
[ ] clean-build CPG logical digests match expected corpus
[ ] canonical query responses unchanged for unchanged source
[ ] publication/overlay freshness semantics unchanged
[ ] negative-proof/completeness semantics unchanged

Snapshot/provider
[ ] exact-version provider reuse scoped to one ServingSnapshot
[ ] no provider drift to later table version
[ ] same-version checkpoint does not change logical publication identity
[ ] daemon restart reconstructs providers without persisted delta-rs caches

Performance
[ ] activation memory within target
[ ] first-query latency within target
[ ] steady-state query p50/p95 within target
[ ] owner replacement latency within target
[ ] optimize/vacuum maintenance within target

Correctness hardening
[ ] nested-nullability optimize fixture passes
[ ] nested-name/partition-column collision fixture passes
[ ] action paths with spaces round-trip
[ ] V2Checkpoint feature fixture passes compatibility gate
[ ] unsupported features still fail closed

Maintenance safety
[ ] old publication remains queryable while leased
[ ] vacuum respects CodeFabric lease/version reachability
[ ] optimize preserves logical content digest
```

---

# 16. Recommended final design stance

The most useful architectural consequence of the new delta-rs pin is not a new Delta feature to expose. It is a **cleaner separation of identity from materialization**:

```text
CodeFabric identity:
  source generation
  durable publication
  exact Delta table versions
  hot overlay
  analysis contexts
  capability/completeness state

Delta execution materialization:
  checkpoint selected
  log replay path
  active-file cache
  stats cache
  lazy/eager replay
  DataFusion physical schema adaptation
```

The first category is durable semantic/current-state truth. The second category is an interchangeable execution strategy.

The latest delta-rs implementation now enforces that separation more carefully inside its own snapshot subsystem. CodeFabric should take advantage of that by making its provider/cache lifecycle more explicitly `ServingSnapshot`-scoped, while **not allowing the storage engine's cache/checkpoint state to leak upward into CPG identity or freshness semantics**.

That is the principal design improvement I recommend from this upgrade.

---

# Upstream references

- Current pinned commit: https://github.com/delta-io/delta-rs/commit/9f9223197469897ef05ae4369eb4fd1390174e65
- Prior-to-current comparison: https://github.com/delta-io/delta-rs/compare/35cfed4545f41c2f483706d29670f7cc2fe7e217...9f9223197469897ef05ae4369eb4fd1390174e65
- Current workspace dependencies: https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/Cargo.toml
- Current `deltalake-core` dependencies/features: https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/crates/core/Cargo.toml
- Snapshot implementation: https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/crates/core/src/kernel/snapshot/mod.rs
- Table-provider builder: https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/crates/core/src/delta_datafusion/table_provider.rs
- Protocol feature checker: https://github.com/delta-io/delta-rs/blob/9f9223197469897ef05ae4369eb4fd1390174e65/crates/core/src/kernel/transaction/protocol.rs
- Spark nested-nullability fix: https://github.com/delta-io/delta-rs/commit/ee55e35f2a7444f50af42f38917c25d5f102626d
- Lazy snapshot replay capability: https://github.com/delta-io/delta-rs/commit/84fad0b1bd861367a94e144cd253323559c9d3bd
- Action path encoding fix: https://github.com/delta-io/delta-rs/commit/3b1734ba2e89599cb386c5f86a8d9983eec61062
- V2 checkpoint protocol support: https://github.com/delta-io/delta-rs/commit/3f562682c5a9dd55693b7f7bbd2a2f749fdf38e5
- CDF in-commit timestamp support: https://github.com/delta-io/delta-rs/commit/7fba644dbc89d2b6b4fe8403b42fd79e327597b7
- Rust 1.94.1 MSRV change: https://github.com/delta-io/delta-rs/commit/e8072a63e827e518bcdb88433fc98a49cf0b1d7e
