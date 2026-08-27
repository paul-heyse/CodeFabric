# Delta Lake / `delta-rs` Design-Principle Alignment Manual

## Agent workflow for a model-first, contract-driven, authority-centered, provenance-native transactional data fabric

**Version baseline:** `deltalake` / `deltalake-core` `1.0.0` at git revision `43a0cf10a313e5077c48637ad786a05359136bbb` (pre-release pin); Apache DataFusion `55.0.0`; Apache Arrow / Parquet `59.2.0`; `object_store` `0.13.2`; Rust `1.94.1`, edition 2024.

**Primary audience:** LLM programming agents and human reviewers translating high-level data-fabric requirements into a coherent Delta Lake design that uses `delta-rs` as a durable transactional state layer rather than merely as a file-writing library.

**Source design constitution:** *Model-First, Contract-Driven, Provenance-Native Data Fabric*.

**Companion capability source:** *deltalake Rust 1.0.0 @ 43a0cf10 — DataFusion 55 / Arrow 59 advanced reference*.

---

# 0. Purpose and scope

This document is a **design-alignment manual**, not a general `delta-rs` API reference. The companion Delta reference remains the detailed source for API syntax and feature behavior. This manual answers a different question:

> Given a high-level requirement, how should an agent use Delta Lake and `delta-rs` so that the resulting design advances the full data-fabric principles—explicit meaning, single authority, durable state transitions, provenance closure, reproducibility, contract truthfulness, and controlled lifecycle—rather than merely producing valid Delta tables?

The manual therefore maps each of the 25 data-fabric principles to:

1. the relevant Delta / `delta-rs` abstractions;
2. the correct architectural use of those abstractions;
3. responsibilities that remain application-owned;
4. transaction, snapshot, schema, protocol, storage, maintenance, provenance, interoperability, and testing consequences;
5. evidence an agent must produce before an implementation is considered aligned.

A central premise is that **Delta Lake is strongest when treated as the durable state-transition authority for one logical table**. It is not automatically the authority for cross-table semantics, enterprise governance, authorization, domain models, or multi-table atomicity. Those concerns must remain explicit above the table layer.

## 0.1 Capability-status legend

| Status | Meaning | Agent implication |
|---|---|---|
| **NATIVE AUTHORITY** | Delta directly owns the durable truth of the concept. | Treat Delta transaction-log state/version as the canonical persisted table-state authority; do not duplicate it elsewhere. |
| **NATIVE ENFORCEMENT** | `delta-rs` / Delta protocol validates or enforces a property. | Depend on it only within the documented feature/operation scope and add boundary tests. |
| **NATIVE STATE TRANSITION** | Delta performs an atomic per-table state transition through the transaction log. | Model the operation as `version N + operation -> version N+1`; record before/after identities. |
| **NATIVE OBSERVABILITY** | Delta exposes durable history, operation metrics, CDF, or table metadata that describes behavior. | Capture and connect it to application provenance rather than relying on logs alone. |
| **INTEGRATION CONTRACT** | Delta exposes a provider, log-store, object-store, builder, or DataFusion integration boundary. | Keep backend/execution variation behind that contract and document lifecycle/capability truth. |
| **COMPOSITION PATTERN** | The principle is achieved by combining Delta with Arrow/DataFusion/application models. | Preserve the prescribed boundaries; do not collapse the design into ad hoc procedural code. |
| **APPLICATION OVERLAY** | Delta supplies useful durable artifacts but not the full capability. | Build an explicit application-owned model, registry, policy, provenance graph, or publication manifest. |
| **CAUTION** | The capability is partial, retention-sensitive, version-coupled, operation-specific, or easy to overstate. | Fail closed, record uncertainty, and add compatibility/adversarial tests. |

## 0.2 What Delta should be authoritative for

Within the combined architecture, Delta should be treated as the authoritative owner for the concepts it actually persists and validates:

- the **current durable state of one Delta table**, identified by transaction-log version;
- the table's active `Add` file set and tombstoned `Remove` state for a pinned version;
- the table's persisted **Delta logical schema**, partition columns, table metadata, and configuration at that version;
- the table's **reader/writer protocol** and declared table features;
- per-table transaction ordering and atomic visibility of committed state transitions;
- durable operation history and commit information retained in the transaction log;
- CDF change records, when CDF is enabled and the required range remains within retention;
- constraints and table properties that Delta explicitly enforces for the relevant write/DML path;
- the logical result of `RESTORE` as a new committed table version;
- the durable relationship between data files and transaction-log actions.

Delta should **not** be promoted beyond those boundaries. In particular:

- a Delta checkpoint is a replay optimization, not a separate semantic version;
- a `DeltaTable` in-memory handle is a loaded view, not automatically “latest forever”;
- a `delta-rs` snapshot cache is process-local implementation state, not application authority;
- `object_store` listings are not the table state authority;
- Parquet directory contents are not the table state authority;
- a DataFusion `TableProvider` is a query view over a snapshot, not the durable source of truth.

## 0.3 What remains application-owned

The following are not complete built-in Delta systems and must remain explicit application overlays:

- domain / business semantic models;
- cross-table publication consistency or multi-table atomicity;
- stable application IDs and semantic-version/fingerprint algorithms;
- enterprise catalog identity and governance workflows;
- authentication, authorization, tenancy, and secret management;
- source-to-result provenance graphs across multiple tables/services;
- durable consumer checkpoints beyond CDF retention;
- replay bundles that pin application code/config/function versions;
- maintenance approval workflow and retention policy governance;
- cross-engine compatibility certification;
- stable query-plan fingerprints;
- global cache invalidation and serving-snapshot publication.

## 0.4 Primary architectural distinction: table state vs data-fabric state

A strong architecture distinguishes the two layers explicitly:

```text
application/data-fabric semantic state
    ├─ domain models
    ├─ cross-table publication manifest
    ├─ policy/configuration versions
    ├─ provenance graph
    └─ serving/query snapshot identity
            ↓ pins
Delta table state
    ├─ table URI / identity
    ├─ exact Delta version
    ├─ protocol + table features
    ├─ metadata + schema + properties
    ├─ active Add actions / tombstones
    └─ durable transaction history
            ↓ materializes
Parquet + object-store objects
```

A table version may be sufficient as the durable identity of one table. A data-fabric result that depends on several tables normally requires an **application publication/snapshot manifest containing one exact Delta version per table**.

## 0.5 Source-grounding and derivation rule

This manual is intentionally a **source-grounded synthesis** of the attached design and capability documents rather than an independent re-specification of Delta Lake. Apply the following evidence hierarchy when using it:

1. **Delta capability facts and version-specific API behavior** come from the attached `deltalake` / `delta-rs` advanced reference at the `43a0cf10…` pin.
2. **Architectural requirements and evaluation criteria** come from the attached 25-principle data-fabric constitution.
3. **Organization, agent workflow, pattern-catalogue structure, crosswalk style, and evidence discipline** intentionally parallel the attached DataFusion 55 + Arrow 59 design-principle alignment manual.
4. **The mapping from Delta capabilities to architectural principles is a design derivation made in this manual.** Where Delta supplies only a per-table mechanism—such as exact versioning, history, commit metadata, CDF, or constraints—the manual labels any broader cross-table, governance, provenance, or semantic capability as an application overlay rather than implying native Delta ownership.
5. This document does **not** claim that `43a0cf10…` remains the newest upstream delta-rs revision after the source reference was produced. If the pinned baseline changes, re-verify the capability reference first and then regenerate or audit this alignment manual.

If a future implementation question is not supported by the attached capability reference, the agent should treat that point as unresolved rather than silently filling it from general Delta knowledge.

---

# 1. How an LLM agent should use this manual

## 1.1 Required input

Before selecting Delta APIs, the agent should have a requirement statement that identifies, at minimum:

- the semantic outcome or durable state transition;
- the table(s) affected;
- whether the operation is read-only, append, overwrite, DML, schema/protocol migration, CDF consumption, maintenance, restore, or repair;
- required snapshot/freshness semantics;
- schema and compatibility expectations;
- atomicity and concurrency expectations;
- retention, time-travel, and CDF requirements;
- provenance and reproducibility expectations;
- storage backend and deployment posture;
- query-serving / DataFusion integration needs.

The requirement should not begin by naming low-level `Add` actions, Parquet files, checkpoints, or `LogStore` internals unless those details are genuinely part of the required semantics.

## 1.2 Mandatory review flow

| Step | Agent action | Required output |
|---|---|---|
| 1. **Extract semantic transition** | Separate the intended table-state change or read semantics from storage mechanics. | `SemanticRequirement` + invariants. |
| 2. **Assign authority** | Name the application semantic authority and the exact Delta table/version authority touched. | `AuthorityMap`. |
| 3. **Choose snapshot/freshness semantics** | Latest strict, latest eventual, exact version, as-of timestamp, or metadata-only/lazy. | `SnapshotPolicy`. |
| 4. **Validate schema/protocol/features** | Check table schema, constraints, properties, reader/writer protocol, and operation-specific feature support. | `DeltaTableContract`. |
| 5. **Choose the highest-level operation** | Prefer `DeltaTable` operation builders / provider APIs before low-level log/action code. | `OperationSelectionRecord`. |
| 6. **Define transaction and concurrency posture** | Identify read snapshot, conflict class, idempotency key, retry/reconciliation behavior. | `TransactionContract`. |
| 7. **Define physical/layout policy separately** | Partitioning, target files, stats, optimize/Z-order, vacuum, checkpoint and lazy/eager behavior. | `PhysicalLayoutPolicy`. |
| 8. **Define provenance and reproducibility** | Pin input versions, schema/protocol identity, operation/config/code IDs, commit metadata, output version. | `ProvenanceClosureMap`. |
| 9. **Define retention consequences** | State how vacuum/log/CDF retention affects replay, restore, and consumers. | `RetentionSafetyReview`. |
| 10. **Define serving/query integration** | If DataFusion is involved, bind exact provider snapshot and runtime/object-store state. | `ProviderBindingRecord`. |
| 11. **Derive tests from claims** | Generate tests for state transition, conflict, schema, protocol, retention, cross-engine, and recovery behavior. | `TestEvidenceMatrix`. |
| 12. **Run anti-pattern review** | Reject raw-Parquet bypass, implicit latest semantics, blind retries, metadata theater, and ungoverned vacuum. | `AntiPatternDisposition`. |
| 13. **Produce implementation packet** | Only after the above is coherent, specify crates/modules/builders/configuration/migrations/jobs. | `ImplementationPacket`. |

## 1.3 Stop conditions

The agent should stop at design rather than proceed to code when any of the following remains unresolved:

- “latest” is requested but freshness semantics are not defined;
- a write retry can duplicate or overwrite data and no idempotency/reconciliation strategy exists;
- schema evolution is proposed without a compatibility decision;
- a table declares protocol/features that have not been certified for the intended operation;
- a multi-table result claims atomicity without an application publication manifest or equivalent overlay;
- vacuum retention could invalidate required time travel, restore, or CDF consumption;
- a provider is assumed to refresh automatically after a Delta commit;
- raw Parquet file listing is being used as a substitute for Delta snapshot state;
- commit metadata is being treated as an enforced business constraint;
- column mapping, type widening, deletion vectors, V2 checkpoints, variant, or other advanced features are assumed fully supported without operation-specific evidence;
- a checkpoint file is being treated as the semantic identity instead of the Delta version;
- tests cannot be written to prove the promised state transition and failure behavior.

---

# 2. Canonical architecture and representation map

## 2.1 Preferred state-transition chain

```text
high-level requirement
    ↓
application semantic model
    ├─ TableSpec / SchemaContract
    ├─ SnapshotPolicy
    ├─ WriteSpec / DmlSpec / MaintenanceSpec / CdfSpec
    ├─ PolicySpec / RetentionSpec
    └─ ProvenanceSpec
    ↓ validate / normalize / fingerprint
bound operation
    ├─ table URI + storage identity
    ├─ exact read/current Delta version
    ├─ protocol + feature compatibility
    ├─ schema + properties + constraints
    └─ DataFusion SessionState / LogicalPlan when needed
    ↓ plan / execute
Arrow RecordBatch stream and/or DataFusion execution
    ↓ physical realization
Parquet files + Add/Remove actions
    ↓ optimistic validate-and-commit
Delta transaction log commit
    ↓
new exact Delta version
    ├─ updated schema/protocol/metadata if applicable
    ├─ operation history / commit metadata
    ├─ operation metrics
    └─ CDF changes when enabled
    ↓
application publication / provenance / serving activation
```

## 2.2 Authority and derivation table

| Concept | Preferred authority | Delta/native form | Derived forms that must point back to authority |
|---|---|---|---|
| Domain table meaning | Application `TableSpec` / `SchemaContract` | Delta metadata + `StructType` at a version | Arrow schema, DataFusion provider schema, docs, API schema, migration plan. |
| Durable table state | Delta transaction log | Exact Delta version / snapshot | `DeltaTable`, `Snapshot`, `TableProvider`, active-file report, query provider. |
| Table schema in production | Application schema authority linked to durable Delta schema | `StructType`, `StructField`, protocol metadata | Arrow `Schema`, DataFusion schema, Parquet physical schema. |
| Durable state transition | Application operation spec + Delta transaction | operation builder + committed actions | metrics, history entry, CDF records, result publication. |
| Query snapshot | Application query/serving snapshot | exact Delta version provider | DataFusion `TableProvider`, physical scan, query artifacts. |
| File layout | Delta active Add actions + Parquet files | partition values, stats, data files | layout reports, optimize candidates, file selections. |
| Transaction provenance | Application provenance record | commit info/properties + Delta version | audit views, lineage graph, downstream result references. |
| Incremental change stream | Delta CDF within retention | `_change_type`, `_commit_version`, `_commit_timestamp` | consumer checkpoint, materialized downstream state, audit stream. |
| Storage integration | deployment/storage config | `LogStore` + `ObjectStore` | DataFusion runtime mapping, retry/endpoint diagnostics. |
| Maintenance state | application maintenance policy + current Delta version | optimize/vacuum/restore/filesystem-check builders + metrics | job record, safety approval, benchmarks, retained-version evidence. |

## 2.3 Highest-level operation-selection hierarchy

Prefer the first level that fully preserves semantics:

```text
DeltaTable read/snapshot/time-travel API
    ↓ for query serving
Delta TableProvider / DataFusion integration
    ↓ for ordinary writes
DeltaTable::write / BlindDeltaTable append path
    ↓ for row-level table mutation
DeltaTable delete / update / merge builders
    ↓ for schema/property/protocol migration
create / add_columns / constraints / metadata / add_feature builders
    ↓ for incremental change consumption
scan_cdf
    ↓ for physical maintenance
optimize / vacuum / restore / filesystem_check
    ↓ only for genuinely unsupported needs
kernel transaction / LogStore / low-level actions
```

Agent rule: **do not descend into low-level Delta actions or log-store internals merely for code organization or performance speculation.** Lower levels increase protocol-correctness and compatibility obligations.

---

# Part I — Principle-by-principle Delta alignment

# P1 — Model semantics before implementing behavior

Important table-state meaning should exist as typed application models before it becomes a sequence of Delta builder calls.

### Applicable Delta mechanisms

| Feature family | Native mechanism | Alignment value |
|---|---|---|
| Table contract | Delta metadata, `StructType`, `StructField`, partition columns, properties | Gives a durable persisted form to an application table model. |
| Snapshot semantics | exact version, timestamp travel, latest-state refresh | Allows read meaning to be modeled explicitly. |
| Mutation builders | write, delete, update, merge, restore | Provide typed execution targets for modeled state transitions. |
| Maintenance builders | optimize, vacuum, filesystem check | Allow maintenance intent to be represented separately from ad hoc file operations. |
| CDF | version/timestamp ranges and change types | Gives a typed incremental-change model. |

### Required utilization rules

- Define `TableSpec`, `SnapshotPolicy`, `WriteSpec`, `DmlSpec`, `CdfConsumerSpec`, `MaintenancePolicy`, and `RetentionPolicy` or equivalent explicit models when those semantics recur.
- Compile those models into `DeltaTable` builders through one controlled layer.
- Keep table URI, partitioning, schema mode, save mode, predicate, retention, and protocol-feature policy explicit rather than scattered across call sites.
- Represent exact version vs latest vs as-of-time as an enum/typed policy, not a boolean or convention.
- Model destructive operations separately from physical cleanup; e.g. `DELETE` and `VACUUM` have different semantics.

### Application-owned overlay

Delta supplies the durable table mechanics; it does not define the domain/business model that determines what a table or state transition means.

### Required evidence

- A serializable operation model exists independently of the Rust builder invocation.
- One compiler/orchestrator maps the model to Delta operations.
- Read/write/maintenance code does not independently restate the same domain policy.

### Reject

- Hard-coded `SaveMode`, retention hours, partition columns, or merge clauses scattered across services.
- “Latest” implied by opening a table with no declared freshness contract.
- Direct `Add`/`Remove` construction where a high-level builder expresses the same semantics.

**Primary patterns:** MOD-01–MOD-08, STA-01–STA-05, WRT-01–WRT-04, DML-01–DML-04

---

# P2 — Make models executable, not merely descriptive

A Delta operation model should validate, bind to a table snapshot, compile to the appropriate builder/DataFusion expression, execute, and emit provenance/tests through controlled interpreters.

### Applicable mechanisms

- `DeltaTableBuilder` for binding snapshot policy.
- DataFusion `Expr` for `replaceWhere`, delete, update, and merge predicates.
- operation builders for writes, DML, schema changes, constraints, maintenance, and restore.
- commit properties / operation metrics / history for derived evidence.
- `scan_cdf` for compiling incremental-consumption specs.

### Required utilization rules

- Give each operation model a validation/binding phase before execution.
- Derive affected table, snapshot version, required columns, protocol features, partition scope, and retention implications from the model.
- Generate dry-run or preflight artifacts where the API supports them, especially vacuum and filesystem repair.
- Produce the provenance record and test cases from the same operation model.
- Treat low-level Delta actions as compiled artifacts, not business specifications.

### Application-owned overlay

Cross-operation orchestration, stable semantic model serialization, versioning, and compilation remain application-owned.

### Required evidence

- One `WriteSpec` can generate validation, execution configuration, provenance fields, and contract tests.
- One `MaintenanceSpec` can generate dry-run, approval, execution, and post-validation steps.

### Reject

- Configuration DTOs that are immediately unpacked into unrelated procedural paths.
- Separate implementations for API, batch job, and replay of the same write semantics.

**Primary patterns:** MOD-02–MOD-08, TXN-01–TXN-06, OBS-01–OBS-06, TST-01–TST-12

---

# P3 — One authoritative owner for every concept

Delta's greatest architectural value is the existence of an exact durable per-table state authority: the transaction log at a specific version.

### Applicable mechanisms

- `DeltaTable::version`, `snapshot`, `load_version`, `update_state`, `update_incremental`.
- protocol/metadata/schema/table configuration in the loaded snapshot.
- active Add actions and tombstones selected by the snapshot.
- table history and commit info.

### Required utilization rules

- Treat `(canonical table identity, exact Delta version)` as the durable identity of a table state.
- Treat `DeltaTable` / `Snapshot` objects as loaded views of that authority, not independent authorities.
- Treat a DataFusion provider as derived from the pinned table state; rebuild when freshness policy changes.
- Treat Parquet/object-store listings as physical materialization, not logical table state.
- If an application `SchemaContract` is the semantic design authority, link its version/fingerprint to the persisted Delta schema; do not let them drift independently.
- For multi-table state, use an application manifest that pins one exact Delta version per table.

### Application-owned overlay

Multi-table publication identity, domain semantics, and cross-table validation require an application authority above Delta.

### Required evidence

- Authority map distinguishes table identity/version, in-memory handles, providers, caches, files, and application publications.
- Every derived provider/cache carries the exact Delta version it derives from.

### Reject

- `HashMap<table, DeltaTable>` with no loaded-version/freshness metadata.
- Directory contents used as active table state.
- Checkpoint filename used as semantic identity.

**Primary patterns:** STA-01–STA-10, MOD-04, QRY-01–QRY-05, OBS-03

---

# P4 — Use explicit conceptual hierarchies to encode shared guarantees and legal variation

Delta has several natural responsibility hierarchies that should remain distinct rather than being collapsed into one “storage” abstraction.

### Native hierarchy

```text
application table specification
    ↓
DeltaTable / loaded snapshot
    ├─ Protocol / table features
    ├─ Metadata / schema / properties / constraints
    └─ active Add / Remove state
          ↓
LogStore — transaction-log correctness / commit visibility
          ↓
ObjectStore — physical object I/O
          ↓
Parquet — data-file representation
```

Query hierarchy:

```text
Delta snapshot
  → Delta TableProvider
    → DataFusion logical/physical plan
      → Arrow RecordBatch stream
```

### Required utilization rules

- Keep `LogStore` and `ObjectStore` responsibilities separate.
- Keep Delta logical schema and Parquet physical schema separate.
- Keep table-state mutation and physical maintenance separate.
- Keep table version, provider snapshot, and query execution state separate.
- Add backend-specific variation at the storage/provider layer rather than branching throughout consumers.

### Required evidence

- Each layer documents invariant responsibilities and permitted variation.
- A new storage backend does not change DML/schema/query consumers.

### Reject

- One universal “DeltaStorage” object owning schema, business policy, auth, object I/O, and query planning.
- Consumers downcasting to S3/Azure/GCS-specific backends for ordinary table behavior.

**Primary patterns:** STO-01–STO-08, QRY-01–QRY-06, SCH-01–SCH-04

---

# P5 — Encode variability behind contracts, not throughout consumers

Cloud/storage/catalog variability belongs behind Delta/object-store/log-store configuration and provider boundaries.

### Applicable mechanisms

- `open_table_with_storage_options` / `DeltaTableBuilder`.
- feature-gated S3, Azure, GCS, HDFS, lakeFS, OpenDAL, Glue, Unity paths.
- `ObjectStore` registry integration with DataFusion.
- `TableProviderBuilder` for query consumers.

### Required utilization rules

- Centralize storage options in typed deployment configuration and emit key/value maps only at the Delta boundary.
- Keep credentials, endpoint quirks, locking/rename configuration, and cloud retries out of table-semantic code.
- Consumers should request a logical table/snapshot, not an S3/GCS-specific implementation.
- Use the Delta provider rather than raw Parquet registration so column mapping, deletion vectors, partition values, schema adaptation, and Delta state are preserved.

### Application-owned overlay

Credential lifecycle, tenant isolation, secret management, and enterprise catalog governance remain outside Delta.

### Required evidence

- Same table operation tests run against local + selected object-store fixtures without semantic branches.
- Storage configuration is centralized and redacted.

### Reject

- `if s3 { ... } else if azure { ... }` throughout DML/query/maintenance code.
- Raw object-store calls for ordinary table reads.

**Primary patterns:** STO-01–STO-10, QRY-02, TST-10

---

# P6 — Separate semantic meaning from execution strategy

Delta operations have semantic intent that should remain stable while file sizing, partition layout, snapshot materialization, scan planning, concurrency, and maintenance strategies vary.

### Semantic vs physical examples

| Semantic meaning | Physical strategy that should remain downstream |
|---|---|
| Append these rows to table T | batch size, target file size, Parquet compression, writer concurrency |
| Replace rows satisfying predicate P | file rewrite grouping, physical plan, row-group sizing |
| Delete rows satisfying predicate P | candidate-file pruning, rewritten file count |
| Read table version N | checkpoint chosen, lazy/eager active-file materialization |
| Preserve current table state while reducing small files | optimize task concurrency, target size, compression |
| Retain rollback capability for 30 days | vacuum traversal mode/concurrency |

### Required utilization rules

- Model save mode, predicate, schema policy, snapshot version, and retention as semantic contracts.
- Keep target file size, row-group size, optimize concurrency, vacuum scan concurrency, and checkpoint selection physical.
- Treat same-version checkpoint adoption as performance/replay change, not semantic state transition.
- Avoid encoding file paths or specific active files in a domain spec unless exact file selection is itself required.

### Required evidence

- Physical tuning can change without changing operation semantic fingerprint.
- Query results at the same exact Delta version remain semantically equivalent across lazy/eager/provider variations.

### Reject

- Domain model says “read checkpoint X” instead of “read Delta version N.”
- Business operation hard-codes optimize task count or Parquet row-group size.

**Primary patterns:** STA-06–STA-09, WRT-05–WRT-08, LAY-01–LAY-08, MNT-01–MNT-06

---

# P7 — Build a shared canonical data fabric

Delta should occupy the durable transactional persistence/state layer while Arrow and DataFusion remain the canonical in-memory/query layers.

```text
application semantic models
        ↓
Delta table contract / snapshot identity
        ↓
DataFusion Expr / LogicalPlan / TableProvider
        ↓
Arrow Schema / RecordBatch stream
        ↓
Parquet data files
        ↕
Delta transaction log / table version
        ↕
object_store / LogStore
```

### Required utilization rules

- Use Arrow `RecordBatch` as the standard write/read batch boundary.
- Use the Delta `TableProvider` for relational reads.
- Use Delta version + schema/protocol as the durable table state boundary.
- Use Parquet as data-file representation, not a parallel table authority.
- Use CDF as the canonical Delta incremental-change boundary when enabled and retention-appropriate.

### Required evidence

- Representation map has explicit conversion/authority points.
- No subsystem-specific row format replaces Arrow in the core path without justification.
- No raw-Parquet bypass exists for a governed Delta table.

### Reject

- Directly scanning `*.parquet` under a Delta root in the normal query path.
- Maintaining a second “active file database” independent of the Delta log.

**Primary patterns:** QRY-01–QRY-08, WRT-01, INT-01–INT-06, STA-01

---

# P8 — Treat the common representation as infrastructure

Delta's durable state model and Arrow's columnar runtime model should be treated as infrastructure rather than convenience layers.

### Applicable mechanisms

- Delta `StructType` / Arrow schema conversion.
- Arrow `RecordBatch` writes and DataFusion batch-stream reads.
- Delta Add-action metadata as Arrow batches for file-layout introspection.
- Parquet writer properties and data-skipping statistics.
- CDF DataFusion execution producing Arrow batches.

### Required utilization rules

- Preserve Arrow-native batches into Delta writes; avoid row-by-row conversion.
- Use Delta file/action metadata rather than re-listing and re-parsing storage objects.
- Maintain meaningful file-level statistics because they are part of query-performance infrastructure.
- Buffer micro-batches to avoid pathological small-file state transitions.
- Keep physical path identity byte/URI-safe; do not hand-normalize action paths.

### Application-owned overlay

Memory budgets, batching policy, file-size SLOs, and workload-specific layout benchmarks are application concerns.

### Required evidence

- Batch/copy boundaries are inventoried.
- Small-file and file-stats quality are observable.
- Performance tests compare representative layout policies.

### Reject

- One Delta transaction per input row.
- Arbitrary file-path string manipulation for transaction-log identity.
- Converting Arrow batches to application row DTOs solely to write them back to Delta.

**Primary patterns:** WRT-01, WRT-05–WRT-08, LAY-01–LAY-08, INT-01–INT-03

---
# P9 — Make provenance intrinsic to every meaningful transformation

Delta provides unusually strong native hooks for durable provenance because every committed table-state change receives a version and commit record.

### Applicable mechanisms

| Feature family | Native mechanism | Alignment value |
|---|---|---|
| Commit identity | exact Delta version | Stable per-table output/state-transition identity. |
| Commit metadata | `CommitProperties` / commit info | Attach application, job, schema, source-version, request, trace, build, and policy references. |
| History | `history()` | Durable operation/audit trail within log retention. |
| CDF | `_commit_version`, `_change_type`, `_commit_timestamp` | Row-level change provenance when enabled. |
| Metrics | write retry metrics; DML/optimize/vacuum/restore/filesystem-check metrics | Operational evidence attached to state transitions/jobs. |
| Snapshot metadata | protocol/schema/table properties/version | Records the contract under which a read/write occurred. |

### Required utilization rules

- Allocate application `operation_id` / `run_id` before execution.
- Record input table versions, semantic model IDs, schema-contract version/fingerprint, code/build ID, policy/config fingerprint, and trace/request IDs in the application provenance record; place compact non-secret references in commit metadata where useful.
- Capture before/after Delta versions for every mutation.
- Capture operation-specific metrics and optimistic-commit retry counts.
- For read-only outputs, record every input table's exact version even when no Delta commit occurs.
- Do not use commit metadata as the only place where critical provenance lives; reference a durable provenance artifact/graph.

### Application-owned overlay

Delta history is table-local and retention-bound. Cross-table lineage, long-term provenance retention, artifact indexing, and source/code/environment closure remain application-owned.

### Required evidence

- Every committed operation can be resolved from output version to application execution record.
- Every read-derived durable result records exact input table versions.
- Provenance survives normal log retention through an application artifact store when required.

### Reject

- “Logs contain enough information to reconstruct it later.”
- Commit metadata containing credentials or large arbitrary payloads.
- Durable result with only a source URI and no source version.

**Primary patterns:** OBS-01–OBS-10, TXN-05, CDF-05, TST-11

---

# P10 — Seek provenance closure

A result should recursively resolve the material facts that produced it. Delta provides an excellent durable table-state node in that graph, but closure requires application composition.

### Preferred closure chain

```text
durable result / publication ID
    ↓
execution / operation ID
    ↓
output Delta table version(s)
    ↓
commit info + operation metrics
    ↓
operation spec + policy/config fingerprints
    ↓
input Delta table versions
    ↓
input schema/protocol fingerprints
    ↓
query / DataFusion planning artifacts when relevant
    ↓
application code/build/dependency environment
```

### Required utilization rules

- Use exact versions, not timestamps alone, as persistent table-state links whenever possible.
- Store requested as-of timestamp **and** resolved version when user semantics are temporal.
- Include schema/protocol/feature identity in replay bundles.
- Treat CDF consumer checkpoints as version identities, not timestamps.
- Preserve application provenance independently of Delta log retention if long-term audit/replay is required.

### Required evidence

- Closure traversal detects missing/expired links explicitly.
- Replay validates that pinned table versions still exist and required files have not been vacuumed.

### Reject

- Assuming `history()` is an indefinite provenance archive.
- CDF timestamp as the sole exactly-once checkpoint.
- Checkpoint Parquet filename used as a replay identity.

**Primary patterns:** OBS-01–OBS-10, STA-03, CDF-03–CDF-06, MNT-07

---

# P11 — Prefer immutable snapshots and explicit state transitions

Delta's core model directly embodies this principle: logical table state advances through immutable committed versions rather than in-place row mutation.

### Applicable mechanisms

- exact version snapshots and time travel;
- `load_version`, `with_version`, `load_with_datetime`;
- append/overwrite/delete/update/merge as new committed versions;
- restore as a **new** version representing a prior logical state;
- optimistic concurrency validation;
- lazy/eager snapshot materialization behind the same logical version.

### Required utilization rules

- Treat reads as pinned to a snapshot for the duration of a logical operation.
- Treat mutations as `before_version + operation -> after_version`.
- Do not mutate one shared `DeltaTable` handle backward/forward across unrelated requests.
- Use exact-version providers for long-running/reproducible queries.
- Treat process-local caches and lazy materialization as derived runtime state.
- Treat same-version checkpoint refresh as identity-neutral.

### Application-owned overlay

For multi-table operations, create an immutable application snapshot/publication containing exact versions for each required table.

### Required evidence

- Concurrency tests prove requests cannot observe mid-operation version drift.
- Re-execution against the same version produces the same logical table rows, subject to external code/query determinism.

### Reject

- “Current table” pointer that changes during a reproducible query.
- Shared mutable handle used for both latest reads and historical time travel.
- Cache contents treated as a new semantic version.

**Primary patterns:** STA-01–STA-10, TXN-01–TXN-06, QRY-04

---

# P12 — Schemas are executable contracts, not documentation

Delta's persisted schema, protocol, table properties, constraints, and write enforcement make schema a first-class executable table contract.

### Applicable mechanisms

| Mechanism | Contract role |
|---|---|
| `StructType` / `StructField` | Persisted Delta logical schema. |
| nullability / decimal / timestamp / nested types | Runtime/write compatibility contract. |
| `SchemaMode::{Merge,Overwrite}` | Explicit schema-evolution modes. |
| constraints / NOT NULL | Enforced invariants on supported write paths. |
| partition columns | Logical + physical layout contract. |
| protocol/table features | Compatibility contract governing readable/writable semantics. |
| column mapping | Logical-to-physical mapping feature with operation-specific restrictions. |
| DataFusion provider adaptation | Preserves logical Delta contract over physical Parquet differences. |

### Required utilization rules

- Define a versioned application `SchemaContract` and compile/validate it against the persisted Delta schema.
- Default writes to strict schema behavior.
- Require explicit approval for `SchemaMode::Merge` or `Overwrite`.
- Keep logical Delta nullability authoritative even where physical Parquet nested nullability is relaxed by Spark; rely on Delta's provider adaptation rather than weakening the logical contract.
- Treat partition-column changes as layout/schema migrations, not write-time options.
- Treat protocol/table-feature changes as compatibility migrations.
- Keep advanced features such as column mapping, type widening, variant, deletion vectors, and nanosecond timestamps behind explicit compatibility tests.

### Application-owned overlay

Semantic field IDs, units, business compatibility policy, schema fingerprints, consumer-impact workflow, and migration approvals remain application-owned.

### Required evidence

- Golden Delta/Arrow/DataFusion/Parquet schema fixtures.
- Strict-write rejection tests.
- Additive/breaking migration classification.
- Cross-engine tests for any advanced feature enabled.
- Nested-nullability and nested-field/partition-name-collision regressions.

### Reject

- Schema inference as production contract.
- “Merge schema” enabled by default for convenience.
- Column metadata assumed to enforce semantics.
- Raw Parquet provider used to bypass Delta logical schema adaptation.

**Primary patterns:** SCH-01–SCH-12, GOV-03–GOV-06, TST-01–TST-04

---

# P13 — Put governance at the authoritative boundary

Delta can enforce important table-level governance at the point where durable state is committed, but it is not a complete authorization system.

### Applicable mechanisms

- schema/type/nullability validation;
- check constraints and NOT NULL;
- `appendOnly` table property;
- CDF enablement and other table properties;
- reader/writer protocol and declared feature validation;
- operation-specific rejection for unsupported column-mapping/features;
- mutation builder boundary for delete/update/merge;
- storage/log-store configuration for safe S3 write semantics.

### Required utilization rules

- Enforce durable data invariants as constraints/schema when Delta supports them.
- Validate protocol/features before writes and DML.
- Fail closed when a table declares unsupported writer features.
- Apply application authorization **before** invoking the mutation builder, and record the policy decision/version in provenance.
- Keep destructive maintenance behind explicit policy/approval gates.
- Require safe locking/conditional-commit semantics for multi-writer object stores.

### Application-owned overlay

Authentication, subject/tenant policy, row/column access control, approval workflow, and secret management remain outside Delta and should be enforced in catalog/provider/service layers.

### Required evidence

- Constraint violation tests.
- Unsupported-feature rejection tests.
- Governance bypass tests covering direct builder/service paths.
- Vacuum/restore authorization and approval tests.

### Reject

- Treating table properties as an authorization model.
- Application policy enforced only in UI while direct service calls can mutate the table.
- Disabling protocol checks to “make the write work.”

**Primary patterns:** GOV-01–GOV-10, SCH-06, TXN-04, MNT-07

---

# P14 — Prefer the highest-level extension that preserves the semantics

The Delta equivalent of the DataFusion extension hierarchy is a strong preference for public table/operation APIs before low-level transaction/log/file manipulation.

### Preferred progression

```text
DeltaTable read/load/snapshot API
  > Delta TableProvider
  > DeltaTable::write / BlindDeltaTable append
  > delete/update/merge builders
  > schema/constraint/property/feature builders
  > CDF / optimize / vacuum / restore / filesystem-check builders
  > provider/file-selection configuration
  > kernel transaction APIs
  > LogStore / raw actions / direct object-store mutation
```

### Required utilization rules

- Use `DeltaTable::write` for governed Arrow/DataFusion writes.
- Use `BlindDeltaTable` only when the workload is truly blind append and does not require file-state reads.
- Use DML builders rather than hand-constructing file rewrites.
- Use `FileSelection` for targeted reads rather than constructing raw Parquet scans.
- Use optimize/vacuum/restore builders rather than deleting/replacing files manually.
- Descend to kernel/log-store APIs only with a documented semantic gap and protocol test suite.

### Required evidence

- `OperationSelectionRecord` lists higher-level alternatives reviewed.
- Low-level use has explicit protocol, conflict, compatibility, and recovery tests.

### Reject

- Raw `_delta_log` JSON generation for a feature already supported by a builder.
- Manual tombstone deletion instead of vacuum.
- Raw file rewrite instead of optimize/merge/update/delete.

**Primary patterns:** WRT-01–WRT-08, DML-01–DML-08, MNT-01–MNT-08, EXT-01–EXT-08

---

# P15 — Preserve optimizer visibility

Delta's query performance depends on keeping partition values, file statistics, predicates, schema mapping, and active-file state visible to DataFusion and the Delta provider.

### Applicable mechanisms

- Delta `TableProvider` and `DeltaScanConfig`;
- partition pruning;
- transaction-log min/max/null-count statistics;
- Parquet row-group/page pruning;
- DataFusion `Expr` predicates for DML and `replaceWhere`;
- `FileSelection` with snapshot-owned metadata;
- deletion-vector-aware scan/CDF handling;
- DataFusion 55 physical-expression/schema adaptation.

### Required utilization rules

- Register the Delta provider, not a raw Parquet directory.
- Keep query-serving snapshots statistics-capable; do not use `skip_stats=true` for normal predicated serving.
- Prefer simple DataFusion expressions for predicates rather than opaque custom logic.
- Preserve partition/filter columns and statistics through writes/optimize.
- Use layout/Z-order only after representative `EXPLAIN`/benchmark evidence.
- Let delta-rs own logical-to-physical nested schema adaptation and deletion-vector handling.

### Application-owned overlay

Workload-driven layout policy and benchmark governance remain application responsibilities.

### Required evidence

- `EXPLAIN` proves projection/filter pushdown.
- File-skipping/partition-pruning metrics or benchmarks exist.
- Raw-Parquet bypass is absent from governed paths.

### Reject

- Query service opens Delta with stats deliberately skipped and assumes normal pruning.
- Nested schema adaptation reimplemented in application code.
- File selection interpreted as authority independent of the snapshot.

**Primary patterns:** QRY-01–QRY-10, LAY-01–LAY-10, STA-08

---

# P16 — Treat lifecycle phases as first-class architecture

A durable Delta operation should expose clear phases rather than one opaque “write table” method.

### Recommended lifecycle

```text
declare semantic operation
  ↓
resolve table/storage identity
  ↓
load/pin snapshot
  ↓
validate schema/protocol/features/policy
  ↓
plan predicates / DataFusion execution / file candidates
  ↓
execute Arrow/Parquet work
  ↓
construct candidate actions
  ↓
validate optimistic transaction / reconcile conflicts
  ↓
commit new Delta version
  ↓
refresh/bind output state
  ↓
verify result contract
  ↓
record history/metrics/provenance/publication
```

Maintenance adds preflight/dry-run/approval phases; CDF adds checkpoint/read/apply/checkpoint-commit phases.

### Required utilization rules

- Phase-tag errors and diagnostics.
- Capture the read version before mutation.
- Distinguish data-file production failure from transaction commit failure.
- Reconcile unknown commit outcomes before retry.
- Validate the new version/schema after mutation.
- Keep vacuum physical cleanup downstream of retention approval.

### Required evidence

- Failure injection at load, validation, data execution, commit, refresh, and verification.
- Unknown-commit-outcome test.
- Dry-run/approval tests for destructive maintenance.

### Reject

- One method that opens latest, mutates, retries blindly, vacuums, and returns success.
- Authorization after files have already been rewritten.

**Primary patterns:** TXN-01–TXN-08, MNT-01–MNT-08, CDF-01–CDF-08, OBS-01

---

# P17 — Make intermediate artifacts inspectable and reproducible

Important Delta operations produce or can expose rich intermediate artifacts that should be retained in governed workflows.

### Useful artifacts

- semantic operation spec;
- exact input snapshot pin;
- schema/protocol/table-property snapshot;
- DataFusion logical/physical plan for plan-backed operations;
- partition/file candidate report;
- vacuum dry-run candidate list;
- optimize pre/post layout report;
- CDF range and consumer checkpoint;
- before/after Delta versions;
- operation metrics;
- commit metadata/history entry;
- output schema fingerprint and verification result.

### Required utilization rules

- Persist artifacts appropriate to operation criticality.
- Normalize/redact paths and secrets.
- Preserve raw Delta version IDs even when higher-level semantic publication IDs are used.
- Capture both preflight and execution artifacts for destructive maintenance.

### Application-owned overlay

Artifact storage, retention, redaction, indexing, and upgrade migration are application-owned.

### Required evidence

- Failed operations retain a partial artifact bundle through the failure phase.
- Maintenance approvals can show exactly what was reviewed.

### Reject

- Only final version number is retained for a complex DML/maintenance operation.
- Vacuum executed without saved dry-run evidence in governed environments.

**Primary patterns:** OBS-01–OBS-10, MNT-07, TST-11

---

# P18 — Fingerprint anything whose identity matters

Delta supplies a strong table-state ordinal—the version—but application semantic identity still requires explicit canonicalization and fingerprints.

### Fingerprint candidates

- canonical table URI / logical table ID;
- application `SchemaContract`;
- Delta schema + partition columns + relevant properties;
- protocol + reader/writer feature set;
- multi-table publication manifest;
- write/DML/maintenance spec;
- source-version set;
- DataFusion query/config/function environment;
- deployment/storage profile without secrets.

### Required utilization rules

- Use `(table_id, delta_version)` for exact durable table-state identity.
- Namespace application fingerprints by algorithm/version.
- Include protocol/features and schema fingerprints in compatibility-sensitive caches.
- Do **not** include checkpoint file identity in semantic table-state fingerprints.
- Do not hash debug/display output as a timeless semantic fingerprint.

### Required evidence

- Canonicalization is deterministic across process runs.
- Same-version checkpoint adoption does not change application semantic identity.
- Material schema/protocol change alters the relevant contract fingerprint.

### Reject

- Cache key = table URI only.
- Fingerprint = `_delta_log` checkpoint filename.
- `Debug` representation used as stable protocol identity.

**Primary patterns:** MOD-06, STA-09, SCH-10, OBS-08–OBS-10

---

# P19 — Make reproducibility a normal operating mode

Delta versioning and time travel are powerful reproducibility primitives, but retention and external execution dependencies must be modeled explicitly.

### Required utilization rules

- Persist exact versions for reproducible reads and result derivations.
- When accepting as-of timestamps, persist the resolved version.
- Pin every table participating in a multi-table reproducible computation.
- Record application code/build, DataFusion/Arrow/delta-rs versions, config, functions, and query/model specs.
- Record whether required versions/files remain within retention.
- Preserve versions needed for replay before vacuum; use `keep_versions`/retention policy where applicable.
- Treat CDF consumer state as exact version checkpoints.

### Application-owned overlay

Reproducibility across code changes, multi-table state, external sources, and long retention requires an application replay/provenance system.

### Required evidence

- Replay harness can reopen every pinned input version.
- Vacuum policy is checked against active reproducibility commitments.
- Operation reports why reproduction is partial when dependencies have expired.

### Reject

- Timestamp-only replay when exact version is available.
- Vacuum policy independent of audit/model-replay requirements.

**Primary patterns:** STA-03–STA-05, OBS-01–OBS-10, MNT-05–MNT-07, TST-12

---

# P20 — Be conservative about claimed capabilities

Delta protocol features and operation support are version- and operation-specific. Overclaiming can produce corrupt or non-interoperable state.

### Required utilization rules

- Use protocol checker results as hard compatibility gates.
- Distinguish “feature recognized by reader/writer protocol checker” from “every high-level authoring workflow fully exposed.”
- Treat `V2Checkpoint` support at this pin as protocol compatibility, not proof of a complete V2-checkpoint management API.
- Treat column mapping as partial/operation-sensitive despite writer-set recognition.
- Treat type widening as unsupported for writes unless the documented support changes and is re-certified.
- Treat variant/nanosecond/deletion-vector behavior as advanced compatibility surfaces.
- Treat CDF availability as retention-bound.
- Treat `skip_stats`/lazy replay as a performance posture, not a claim that stats are permanently unavailable.

### Required evidence

- Capability matrix maps table feature × read/write/DML/maintenance operation × engine/version.
- Negative tests for unsupported features.
- Cross-engine fixtures for enabled advanced features.

### Reject

- “Delta supports feature X” without specifying operation and version.
- Disabling protocol validation because current files do not visibly use a declared feature.

**Primary patterns:** GOV-05–GOV-10, SCH-06–SCH-09, INT-05, TST-03

---

# P21 — Separate enforced semantics from advisory metadata

Delta has several distinct semantic classes that must not be conflated.

| Class | Examples | Enforcement posture |
|---|---|---|
| **Enforced** | schema types/nullability; supported constraints; protocol compatibility; append-only restrictions | Delta/runtime validates within documented paths. |
| **Transaction semantics** | version ordering, optimistic conflict validation, Add/Remove state | Delta transaction log owns per-table durability. |
| **Planner/physical metadata** | file stats, partition values, deletion vectors | Used for pruning/execution; not business constraints. |
| **Contractual metadata** | schema-contract version, table properties, feature declarations | Must be interpreted/validated by named consumers. |
| **Lineage metadata** | commit properties, run/source IDs | Provenance references, not enforcement. |
| **Advisory metadata** | descriptions, display metadata | Human/tooling information only. |

### Required utilization rules

- Name the consumer/enforcer for every metadata key.
- Use Delta constraints/schema when correctness depends on enforcement.
- Use commit metadata for lineage references, not authorization or uniqueness enforcement.
- Treat file stats as optimizer inputs, not truth about business rules.

### Required evidence

- Metadata dictionary classifies each key.
- Enforcement tests prove real failures for enforced semantics.

### Reject

- `owner=finance` metadata assumed to prevent unauthorized writes.
- commit property `unique_key=true` assumed to enforce uniqueness.

**Primary patterns:** SCH-05–SCH-07, GOV-01–GOV-04, OBS-05

---

# P22 — Use protocols and canonical boundaries for interoperability

Delta should be used as a protocol-aware transactional table format atop Parquet, while Arrow/DataFusion remain canonical compute boundaries.

### Applicable boundaries

- Delta transaction protocol and table features;
- Parquet data files;
- Arrow schema/RecordBatch in Rust/Python interop;
- DataFusion provider/query integration;
- object-store APIs;
- CDF as incremental table-change protocol;
- optional cloud/catalog integrations.

### Required utilization rules

- Prefer Delta protocol semantics over directory conventions.
- Use the Delta provider for cross-layer reads.
- Use Arrow for in-process/cross-language batch interoperability.
- Run cross-engine read/write tests for any protocol/table feature used.
- Pin coherent delta-rs/DataFusion/Arrow/Parquet versions in Rust public interfaces.
- Maintain protocol/feature compatibility matrices for Spark/Databricks/PyArrow/DataFusion as required.

### Application-owned overlay

Cross-engine certification, protocol upgrade policy, and external catalog governance remain application-owned.

### Required evidence

- Golden tables are readable by every supported engine.
- Protocol upgrades have explicit rollout/rollback notes.

### Reject

- Treating Parquet compatibility as equivalent to Delta compatibility.
- Mixing incompatible Arrow/DataFusion type universes.

**Primary patterns:** INT-01–INT-10, SCH-08, TST-09–TST-12

---

# P23 — Keep state ownership local and explicit

Delta introduces several state scopes that must be named and kept separate.

### State scopes

| State | Owner/scope | Authority relationship |
|---|---|---|
| loaded `DeltaTable` state | handle/service cache | derived view of a specific Delta version |
| kernel snapshot/materialized-file cache | process/table handle | ephemeral implementation state |
| DataFusion provider | query/session registration | derived from a Delta snapshot/version |
| DataFusion runtime object-store mapping | session/runtime | resource/config state, not semantic table authority |
| transaction candidate actions | one mutation attempt | ephemeral until commit |
| CDF consumer checkpoint | consumer | application-owned progress identity |
| maintenance job state | job | application workflow state |
| object-store credentials/options | deployment/tenant | security/resource state |

### Required utilization rules

- Tag cached table/provider state with exact version and config identity.
- Use locks/scopes appropriate to the handle; do not mutate a shared handle across unrelated version semantics.
- Treat snapshot caches as rebuildable.
- Keep CDF checkpoints durable and separate from table state.
- Isolate tenant credentials/runtime mappings appropriately.
- Explicitly invalidate/rebuild providers after a freshness-relevant table refresh.

### Required evidence

- `StateOwnershipMap` covers process/runtime/session/request/transaction/consumer/job scopes.
- Concurrency tests prove no stale provider or cross-tenant object-store leakage.

### Reject

- Global mutable `DeltaTable` serving latest and historical requests.
- Process cache persisted as durable table authority.

**Primary patterns:** STA-06–STA-10, QRY-03–QRY-06, CDF-04, STO-06

---

# P24 — Make observability semantic, not merely operational

Delta can expose table versions, state transitions, conflict retries, maintenance actions, CDF ranges, and schema/protocol drift as first-class semantic telemetry.

### Required semantic observability

- table logical name/ID and exact version;
- before/after version for mutation;
- operation type and semantic spec ID;
- schema/protocol/feature fingerprint;
- input source versions;
- write `num_retries` / conflict evidence;
- DML rows/files touched where metrics provide them;
- optimize files added/removed/skipped/partitions;
- vacuum dry-run/execution candidates/deletions/retention;
- restore target and files re-added/removed;
- filesystem-check missing-file findings;
- CDF start/end/checkpoint version;
- provider/query plan and pruning evidence for reads.

### Required utilization rules

- Join runtime metrics to semantic operation IDs.
- Alert on repeated commit retries, stale providers, schema/protocol drift, lagging CDF consumers, small-file growth, and vacuum retention risk.
- Redact credentials and sensitive paths.
- Distinguish maintenance no-op from failure.

### Application-owned overlay

Metrics backend, tracing, dashboards, alerting, long-term audit storage, and cardinality policy are application concerns.

### Required evidence

- One operation ID correlates traces, Delta versions/history, metrics, and provenance artifact.
- Upgrade benchmarks detect plan/layout/commit-contention drift.

### Reject

- Only latency and error count are observed.
- Version/operation identity omitted from write/maintenance metrics.

**Primary patterns:** OBS-01–OBS-10, MNT-08, QRY-09, TXN-07

---

# P25 — Make testing derive from contracts and invariants

Every advertised Delta behavior should generate tests against the exact pinned stack and enabled table features.

### Required test families

- exact-version/latest/as-of snapshot behavior;
- schema and strict write enforcement;
- protocol/table-feature compatibility;
- optimistic conflict/retry/idempotency/unknown-outcome reconciliation;
- append/overwrite/replaceWhere semantics;
- delete/update/merge correctness and clause ordering;
- CDF version ranges, change types, deletion vectors, timestamps, checkpoints, retention;
- DataFusion provider freshness, projection/filter/partition/file pruning;
- file selection and missing-file policy;
- optimize/Z-order semantic preservation;
- nested-schema optimize/read regressions;
- vacuum dry-run/retention/keep-version/time-travel breakage;
- restore before/after vacuum;
- filesystem-check dry-run/repair behavior;
- storage backends, locking, conditional-put, credentials, endpoint isolation;
- cross-engine protocol/schema feature compatibility;
- dependency/type-universe gates.

### Required utilization rules

- Every `ContractAndCapabilityMatrix` row maps to evidence.
- Test logical table equality before/after optimize.
- Test historical replay before and after vacuum policy boundaries.
- Test unsupported declared features fail even if affected rows/files do not exercise them.
- Test same-version checkpoint refresh is semantically neutral.
- Test commit retry metrics under contention.

### Application-owned overlay

Fixture generation, cross-engine CI, long-running object-store integration, and release gates remain application-owned.

### Reject

- Tests only verify that builders return `Ok`.
- Maintenance tests ignore logical table equivalence.
- No negative/adversarial transaction tests.

**Primary patterns:** TST-01–TST-14 and every pattern family's evidence column.

---
# Part II — Delta capability-utilization pattern catalogue

The following stable identifiers allow future functional building blocks to map to concrete Delta utilization patterns rather than vague statements such as “use Delta Lake.” Selecting a pattern means accepting its contract, lifecycle, provenance, compatibility, and evidence obligations.

# 3. Semantic modeling and authority patterns

## MOD — Semantic modeling and table authority

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
|---|---|---|---|---|
| MOD-01 | Application `TableSpec` | Model logical table purpose, schema authority, partitioning, required properties/features, retention class, and serving posture before creation. | P1, P3, P12 | Spec serialization + validation tests. |
| MOD-02 | `SnapshotPolicy` | Represent `LatestStrict`, `LatestEventual`, `PinnedVersion`, `AsOfTime`, and `MetadataFirst` explicitly. | P1, P11, P19, P23 | Policy→load behavior tests. |
| MOD-03 | `WriteSpec` / `DmlSpec` | Represent save mode, predicate, schema policy, idempotency, provenance, and expected result transition. | P1, P2, P16 | Spec→builder golden tests. |
| MOD-04 | Authority map | Distinguish application semantic authority, exact Delta version, loaded handle, provider, checkpoint, cache, and files. | P3, P11 | Authority/staleness audit. |
| MOD-05 | Operation lifecycle model | Declare resolve→pin→validate→plan→execute→commit→verify→observe phases. | P2, P16 | Phase failure-injection tests. |
| MOD-06 | Versioned fingerprints | Fingerprint application specs/contracts/publication manifests; use Delta version as table-state identity, not a replacement for semantic fingerprints. | P10, P18, P19 | Canonicalization tests. |
| MOD-07 | Multi-table publication model | Pin one exact Delta version per table for coherent cross-table state; never imply native multi-table atomicity. | P3, P10, P11, P19 | Publication consistency/reopen tests. |
| MOD-08 | Operation selection record | Document why high-level table/operation APIs are sufficient or why lower-level kernel/log APIs are required. | P14, P20 | Review + compatibility test plan. |

## STA — Table loading, snapshots, freshness, and time travel

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
|---|---|---|---|---|
| STA-01 | `DeltaTableBuilder::load`, `open_table*` | Load table state through Delta, not filesystem enumeration. | P3, P7, P11 | Open/latest fixture tests. |
| STA-02 | `version`, `get_latest_version` | Distinguish loaded local version from backing-store latest. | P3, P23, P24 | stale-state tests. |
| STA-03 | `with_version` / `load_version` | Use exact versions for reproducible reads and query/provider pins. | P10, P11, P19 | historical replay tests. |
| STA-04 | timestamp travel | Resolve as-of timestamp to exact version and persist both. | P10, P19 | timestamp→version tests. |
| STA-05 | `update_state` / `update_incremental` | Refresh latest explicitly according to freshness policy; never use DML `update()` as refresh. | P3, P16, P23 | incremental/full refresh tests. |
| STA-06 | `without_files` / lazy snapshot | Use metadata-first/lazy posture only when appropriate; expect first file-dependent operation to pay replay cost. | P6, P23 | activation vs first-read benchmarks. |
| STA-07 | `with_skip_stats` | Use stats-free loading only where no query pruning/data operation depends on resident stats; query-serving default remains stats-capable. | P15, P20 | no-stats/query behavior tests. |
| STA-08 | snapshot-native file discovery | Let snapshot APIs own active-file discovery and metadata; do not persist internal file caches as authority. | P3, P11, P23 | cache-mismatch/replay tests. |
| STA-09 | same-version checkpoint refresh | Treat new checkpoint adoption at the same Delta version as identity-neutral replay optimization. | P6, P11, P18 | same-version equivalence test. |
| STA-10 | immutable request pin | Use separate/pinned handles/providers for historical or long-running requests instead of mutating shared latest handles. | P11, P23 | concurrency isolation tests. |

# 4. Schema, protocol, constraints, and governance patterns

## SCH — Schema and table-contract utilization

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
|---|---|---|---|---|
| SCH-01 | `StructType::try_new`, `StructField` | Compile the application schema authority into validated Delta logical schema. | P1, P3, P12 | Delta schema golden fixture. |
| SCH-02 | Arrow schema mapping | Use Arrow schema/RecordBatch as runtime boundary while keeping Delta schema as persisted table contract. | P7, P8, P12 | Delta↔Arrow round trips. |
| SCH-03 | nullability/types/decimals/timestamps | Define exact type/nullability/timezone/precision policy; fail closed on drift. | P12, P20 | type/null/decimal/timestamp matrix. |
| SCH-04 | nested structures | Use struct/list/map only under explicit query/evolution policy; test nested physical/logical adaptation. | P12, P22 | nested round-trip + Spark-optional regression. |
| SCH-05 | field/table metadata | Classify metadata as contractual/governance/lineage/advisory and name consumers. | P9, P21 | metadata registry/round-trip tests. |
| SCH-06 | `SchemaMode` | Default strict; use Merge/Overwrite only as explicit governed migrations. | P12, P13, P20 | strict/merge/overwrite tests. |
| SCH-07 | constraints / NOT NULL | Encode enforceable invariants at the table boundary where Delta supports them. | P12, P13, P21 | violation + existing-data validation tests. |
| SCH-08 | protocol + table features | Inspect reader/writer protocol and fail unsupported declared features wholesale. | P12, P20, P22 | unsupported-feature matrix. |
| SCH-09 | advanced feature compatibility | Treat column mapping, variant, timestamp nanos, deletion vectors, V2 checkpoints, type widening as explicit certification surfaces. | P20, P22 | cross-engine feature fixtures. |
| SCH-10 | schema/protocol fingerprint | Fingerprint schema, partition columns, relevant properties, protocol, and features for caches/provenance. | P10, P18, P19 | deterministic fingerprint tests. |
| SCH-11 | logical/physical schema adaptation | Let Delta provider own column mapping and nested physical nullability adaptation; do not bypass with raw Parquet. | P5, P12, P15 | provider vs raw-file regression tests. |
| SCH-12 | schema migration workflow | Treat add columns, metadata updates, constraint changes, protocol/feature changes as explicit versioned migrations. | P11, P16, P17 | before/after contract tests. |

## GOV — Protocol, feature, policy, and mutation governance

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
|---|---|---|---|---|
| GOV-01 | Check constraints | Place stable row invariants in Delta when expressible and supported. | P13, P21 | invalid write/DML rejection. |
| GOV-02 | append-only property | Use to enforce append-only table classes; do not rely on naming convention. | P13, P21 | mutation rejection tests. |
| GOV-03 | CDF property | Enable only through governed migration with retention/consumer plan. | P13, P16 | enable + consumer compatibility tests. |
| GOV-04 | application authorization before builder execution | Enforce user/tenant/write authority outside Delta at the service/provider boundary and record policy version. | P13 | bypass-path tests. |
| GOV-05 | protocol checker | Reject unsupported reader/writer features even if touched data does not use them. | P20 | declared-unsupported-feature tests. |
| GOV-06 | operation-specific feature gates | Enforce stricter restrictions for schema evolution, optimize, CDF, etc. on advanced feature tables. | P20, P21 | operation×feature matrix. |
| GOV-07 | feature migration | Use add-feature/property builders only with compatibility matrix and rollback analysis. | P13, P20, P22 | staged cross-engine migration. |
| GOV-08 | retention governance | Bind vacuum/log/CDF retention to audit/replay/consumer obligations. | P10, P13, P19 | retention safety gate. |
| GOV-09 | destructive-operation approval | Require policy/preflight/approval for vacuum, restore downgrade, filesystem repair, broad DML. | P13, P16 | approval/bypass tests. |
| GOV-10 | capability truth registry | Record read/write/DML/CDF/maintenance support by feature and exact dependency pin. | P20, P25 | claim-to-test traceability. |

# 5. Transaction, write, and DML patterns

## TXN — Transactions, optimistic concurrency, and commit semantics

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
|---|---|---|---|---|
| TXN-01 | exact read snapshot | Record the Delta version against which a mutation is planned. | P11, P16 | before-version assertion. |
| TXN-02 | optimistic validate-and-commit | Treat conflict validation as part of the transaction contract; do not emulate row locks. | P11, P16, P20 | conflicting-writer tests. |
| TXN-03 | idempotency key / operation ID | Define deterministic retry identity before writing. | P9, P16, P19 | duplicate retry tests. |
| TXN-04 | unknown commit outcome reconciliation | Reload latest/history and determine whether operation committed before retrying. | P16, P20 | injected timeout-after-commit test. |
| TXN-05 | commit properties | Attach compact provenance references without secrets. | P9, P10, P21 | history lookup tests. |
| TXN-06 | before/after version contract | Every mutation reports or records input and committed versions. | P9, P11, P24 | version transition assertions. |
| TXN-07 | `num_retries` telemetry | Capture optimistic retry count as contention signal. | P24 | contention test emits retries. |
| TXN-08 | storage commit safety | Configure S3 locking / conditional writes / safe backend semantics before multi-writer production use. | P5, P13, P20 | backend concurrency fixture. |

## WRT — Write and append utilization

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
|---|---|---|---|---|
| WRT-01 | `DeltaTable::write` | Canonical governed Arrow/DataFusion write boundary. | P7, P14 | append/read-back test. |
| WRT-02 | `SaveMode` | Select Append/Overwrite/ErrorIfExists/Ignore explicitly from semantic intent. | P1, P16 | mode semantics tests. |
| WRT-03 | `replaceWhere` | Use bounded predicate replacement for regenerable slices; validate all input rows satisfy predicate. | P2, P16, P20 | predicate violation + retry tests. |
| WRT-04 | DataFusion `LogicalPlan` + SessionState | Persist query results using the same planning/runtime state that produced the plan; require session state in production. | P6, P7, P23 | UDF/object-store/session tests. |
| WRT-05 | target file size | Treat file size as physical policy; benchmark table class. | P6, P8, P15 | file-size distribution benchmark. |
| WRT-06 | write batch / Parquet properties | Tune row groups/compression only with workload evidence and compatibility tests. | P6, P8, P22 | read/write benchmark + cross-engine test. |
| WRT-07 | micro-batch buffering | Avoid one-row/one-input-object commits and pathological small files. | P8, P15, P24 | small-file SLO tests. |
| WRT-08 | `BlindDeltaTable` | Use metadata-only append handle only for truly blind append workloads; keep read/DML paths on `DeltaTable`. | P6, P14, P20 | append-only capability tests. |

## DML — Delete, update, merge, and row-level mutation

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
|---|---|---|---|---|
| DML-01 | delete builder | Use deterministic predicate; model delete-all as explicit destructive operation. | P1, P13, P16 | predicate/all-row tests. |
| DML-02 | update builder | Compile assignments/predicate from typed spec; validate casts and constraints. | P1, P12, P16 | update result + invalid cast tests. |
| DML-03 | merge builder | Model target/source aliases, match conditions, clause order, and assignments explicitly. | P1, P2, P16 | clause ordering fixtures. |
| DML-04 | idempotent merge/upsert | Require stable business/source keys and retry semantics. | P16, P19 | retry equivalence tests. |
| DML-05 | duplicate match policy | Prevent ambiguous multi-match source semantics before merge. | P20, P25 | duplicate-source negative tests. |
| DML-06 | session-state injection | Use governed DataFusion session/runtime for expression planning/execution. | P15, P23 | missing-UDF/runtime tests. |
| DML-07 | rewrite/file metrics | Observe files/rows rewritten to quantify physical cost without changing semantic contract. | P6, P24 | metrics assertions. |
| DML-08 | append-only/feature restrictions | Validate table properties/protocol before DML and fail closed. | P13, P20 | restricted-table tests. |

# 6. Change-data, layout, and maintenance patterns

## CDF — Change Data Feed utilization

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
|---|---|---|---|---|
| CDF-01 | CDF enablement | Enable through governed property migration with retention/consumer design. | P13, P16 | before/after property tests. |
| CDF-02 | `scan_cdf` | Consume changes via Delta's CDF API, not transaction-log scraping. | P7, P14, P22 | change-type fixtures. |
| CDF-03 | exact version range | Use start/end versions as primary reproducibility/order boundary. | P10, P19 | exact-range replay tests. |
| CDF-04 | durable consumer checkpoint | Persist last successfully applied source version outside Delta source table. | P10, P23 | crash/restart tests. |
| CDF-05 | `_commit_version` + change type | Build external sink idempotency from version/change/business key; timestamp is secondary. | P9, P19 | duplicate reprocessing tests. |
| CDF-06 | in-commit timestamp | Use ICT-aware emitted timestamp for temporal metadata/filtering while retaining version as authoritative order. | P19, P20 | ICT/fallback tests. |
| CDF-07 | deletion-vector-aware CDF | Validate insert/delete semantics for DV changes; do not reconstruct DVs manually. | P15, P20, P22 | DV CDF fixtures. |
| CDF-08 | retention guard | Block vacuum or alert when consumers require versions outside planned retention. | P10, P13, P19 | lag/vacuum safety tests. |
| CDF-09 | initial snapshot + catch-up | Define race-free baseline snapshot/version then consume CDF strictly after it. | P11, P16, P19 | concurrent-write initialization test. |
| CDF-10 | schema evolution | Split/migrate consumer handling across breaking schema changes; additive changes require compatibility tests. | P12, P20 | cross-version CDF schema tests. |

## LAY — Partitioning, file layout, statistics, and pruning

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
|---|---|---|---|---|
| LAY-01 | partition columns | Select stable low/medium-cardinality query-pruning dimensions; treat as table contract. | P6, P12, P15 | partition pruning benchmark. |
| LAY-02 | partition filters | Use Delta partition-filter APIs for metadata/file selection rather than parsing paths. | P5, P15 | filter/path fixtures. |
| LAY-03 | Add-action stats | Inspect file size, partition values, min/max/null stats through snapshot metadata. | P8, P15, P24 | layout report tests. |
| LAY-04 | data skipping | Preserve/use stats for common filters; do not overstate exact row filtering. | P15, P20 | EXPLAIN/file-read benchmark. |
| LAY-05 | small-file SLO | Define target file count/size by table class and trigger compaction from evidence. | P8, P24 | detector tests. |
| LAY-06 | query-pattern-driven layout | Choose partition/Z-order based on workload evidence, not generic rules. | P6, P15 | pre/post benchmark. |
| LAY-07 | selective stats/lazy materialization | Separate activation cost from first-query cost and keep query-serving stats policy explicit. | P6, P23 | open/first/steady-state benchmarks. |
| LAY-08 | file path identity | Treat Add/Remove paths as Delta/object-store URI identities; let delta-rs encode spaces/escapes. | P3, P8, P22 | path round-trip tests. |
| LAY-09 | `FileSelection` | Target exact snapshot-known files for repair/quality/maintenance reads; retain snapshot metadata and missing-file policy. | P5, P15, P20 | missing/duplicate/out-of-root tests. |
| LAY-10 | deletion vectors | Treat DV metadata as part of snapshot/file semantics and let provider/CDF own application. | P3, P15, P22 | read/CDF DV tests. |

## MNT — Optimize, vacuum, restore, checkpoint, and repair

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
|---|---|---|---|---|
| MNT-01 | optimize compact | Use for small-file reduction while asserting logical table equality before/after. | P6, P14, P25 | equality + file-count tests. |
| MNT-02 | Z-order | Use only after workload benchmark shows file-skipping benefit. | P6, P15 | before/after query benchmark. |
| MNT-03 | partition-scoped optimize | Prefer closed/inactive partitions; avoid hot files subject to conflicting mutation. | P16, P20 | concurrent append/overwrite tests. |
| MNT-04 | vacuum dry run | Make dry run + candidate review a standard preflight for governed tables. | P16, P17, P25 | dry-run evidence test. |
| MNT-05 | vacuum retention / keep versions | Bind physical cleanup to time-travel/CDF/replay commitments. | P10, P19 | retained-version reopen tests. |
| MNT-06 | vacuum full/lite/concurrency | Treat scan mode/concurrency as physical strategy; benchmark large partition trees. | P6, P23, P24 | deep-partition benchmark. |
| MNT-07 | restore | Treat restore as new committed version; verify required historical files exist and protocol downgrade policy is explicit. | P11, P16, P19 | pre/post-vacuum restore tests. |
| MNT-08 | filesystem check | Use for incident repair with dry-run/approval; never as normal cleanup or data-loss concealment. | P13, P16, P24 | missing-file fixture. |
| MNT-09 | checkpoint semantics | Treat checkpoints as replay accelerators; logical identity stays Delta version. | P6, P11, P18 | same-version checkpoint equivalence. |
| MNT-10 | nested optimize regressions | Test Spark physical optionality and nested-field/partition-name collision at every upgrade. | P12, P25 | dedicated golden fixtures. |

# 7. Storage, query, provenance, interoperability, and testing patterns

## STO — LogStore, ObjectStore, cloud, and deployment

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
|---|---|---|---|---|
| STO-01 | `LogStore` | Treat as table-scoped transaction-log consistency/commit boundary, distinct from raw object I/O. | P4, P5 | backend commit smoke tests. |
| STO-02 | `ObjectStore` | Use as physical I/O abstraction; do not infer table state from listings. | P3, P5 | list-vs-snapshot tests. |
| STO-03 | typed storage config | Centralize region/endpoint/TLS/locking/client options; emit string map only at boundary. | P1, P5, P23 | config validation/redaction. |
| STO-04 | S3 safe write config | Use DynamoDB locking or supported conditional semantics for multi-writer deployment; unsafe rename only under proven single writer. | P13, P20 | concurrency fixtures. |
| STO-05 | TLS feature posture | Select rustls/native TLS deliberately; avoid accidental mixed profiles. | P20, P22 | build matrix. |
| STO-06 | tenant/session credential isolation | Scope object-store mappings and credentials; fresh sessions where mappings differ. | P13, P23 | cross-tenant isolation tests. |
| STO-07 | OpenDAL | Use for long-tail backends where appropriate; do not let it silently replace native backend semantics. | P5, P22 | per-scheme fixtures. |
| STO-08 | cloud feature minimization | Enable only deployed backends/features in production binaries. | P20, P23 | feature-profile CI. |
| STO-09 | canonical table URI | Normalize/record stable table identity; all writers must agree on root semantics. | P3, P18 | URI equivalence/locking tests. |
| STO-10 | secret hygiene | Never log storage options/commit metadata secrets; redact key classes. | P13, P24 | redaction tests. |

## QRY — DataFusion serving and Delta provider integration

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
|---|---|---|---|---|
| QRY-01 | Delta `TableProvider` | Register Delta provider, never raw Parquet folder, for governed reads. | P7, P14, P15 | provider plan smoke. |
| QRY-02 | `update_datafusion_session` | Register table-root object store into intended runtime; remember it does not overwrite an existing mapping. | P5, P23 | endpoint/mapping tests. |
| QRY-03 | provider version binding | Record exact Delta version behind every registered provider. | P3, P11, P24 | stale-provider tests. |
| QRY-04 | rebuild on refresh | Refresh Delta state and rebuild/re-register provider when freshness policy requires new version. | P11, P23 | N→N+1 provider tests. |
| QRY-05 | exact-version provider | Use pinned provider for reproducible/long-running queries. | P10, P19 | replay query tests. |
| QRY-06 | `SessionState` / runtime | Preserve object-store/UDF/config/spill state for plan-backed writes/DML/optimize. | P6, P23 | missing-session negative tests. |
| QRY-07 | projection/predicate/partition pushdown | Generate explicit columns and pushdown-friendly expressions; verify with EXPLAIN. | P15, P24 | plan snapshots + results. |
| QRY-08 | file column | Use source file path only for diagnostics/provenance and protect sensitive paths. | P9, P13 | leakage tests. |
| QRY-09 | query semantic metrics | Record table versions, plan identity, pruning, rows, latency, spill. | P24 | trace correlation. |
| QRY-10 | Delta-aware schema/DV adaptation | Let delta-rs own physical expression adaptation and DV filtering. | P12, P15 | Spark/DV regression tests. |

## OBS — Provenance, history, observability, and reproducibility

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
|---|---|---|---|---|
| OBS-01 | operation artifact bundle | Capture spec, pins, contract, plan/preflight, before/after versions, metrics, verification, errors. | P9, P10, P17 | complete/partial bundle tests. |
| OBS-02 | `history()` | Use as durable table-local audit source within retention; copy/index externally for long retention. | P9, P10 | history lookup tests. |
| OBS-03 | table/version identity | Represent durable table state as canonical table identity + exact Delta version. | P3, P10, P18 | identity resolution tests. |
| OBS-04 | commit properties | Store compact lineage references and build/config/source pins. | P9, P21 | history/provenance join tests. |
| OBS-05 | operation metrics | Persist DML/maintenance metrics with operation ID and versions. | P9, P24 | metrics schema tests. |
| OBS-06 | retry/contention metrics | Track `num_retries` and conflict/reconciliation outcomes. | P24 | contention tests. |
| OBS-07 | CDF provenance | Persist source version range, consumer checkpoint, schema/feature context. | P9, P10 | replay tests. |
| OBS-08 | schema/protocol fingerprint | Record contract fingerprint at every governed read/write result. | P10, P18, P19 | drift detection. |
| OBS-09 | environment record | Record delta-rs/DataFusion/Arrow/Parquet/Rust/build/config versions for reproducibility. | P10, P19 | environment replay check. |
| OBS-10 | closure status | Explicitly report missing/expired versions/files/log/CDF ranges rather than guessing. | P10, P19 | expired-retention test. |

## INT — Interoperability and compatibility

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
|---|---|---|---|---|
| INT-01 | Arrow `RecordBatch` | Use as canonical Rust/Python/data-plane batch boundary. | P7, P8, P22 | Arrow round trips. |
| INT-02 | Parquet | Use as Delta-managed data file format; do not expose it as alternate table authority. | P3, P7, P22 | Delta+Parquet cross-engine fixtures. |
| INT-03 | DataFusion 55 integration | Keep one coherent Arrow/DataFusion type universe and use Delta provider/write plan integration. | P7, P22 | cargo-tree + query/write tests. |
| INT-04 | CDF external CDC mapping | Map Delta changes to downstream sinks with version-based idempotency. | P9, P22 | crash/replay sink tests. |
| INT-05 | table-feature matrix | Certify read/write/DML/CDF/maintenance by engine and exact table feature. | P20, P22 | compatibility CI. |
| INT-06 | column mapping | Use only when certified and operation restrictions are understood; preserve logical/physical distinction. | P12, P20, P22 | Spark/DataFusion/Delta fixtures. |
| INT-07 | variant / advanced types | Isolate behind explicit compatibility profiles. | P20, P22 | multi-engine type tests. |
| INT-08 | V2 checkpoint | Treat supported protocol recognition separately from authoring/operational workflow support. | P20, P22 | reader/feature gate tests. |
| INT-09 | Python Arrow interop | Align with the documented Arrow/PyO3/pyo3-arrow universe when sharing buffers. | P8, P22 | Python/Rust zero-copy fixtures. |
| INT-10 | dependency matrix | Pin delta-rs/DataFusion/Arrow/Parquet/object_store/Rust and reject incompatible duplicate public type universes. | P18, P19, P22, P25 | CI dependency gate. |

## EXT — Lowest-necessary Delta extension level

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
|---|---|---|---|---|
| EXT-01 | high-level load/provider | Use for ordinary read/query needs. | P14 | API review. |
| EXT-02 | high-level write/DML builder | Use for ordinary state changes. | P14 | contract suite. |
| EXT-03 | metadata/schema/feature builder | Use for governed migrations. | P14 | migration tests. |
| EXT-04 | CDF/maintenance builder | Use for incremental/physical lifecycle operations. | P14 | operation tests. |
| EXT-05 | `FileSelection` | Use for targeted file reads while preserving snapshot metadata. | P14, P15 | selection tests. |
| EXT-06 | `BlindDeltaTable` | Use only for true blind append performance specialization. | P14, P20 | capability boundary tests. |
| EXT-07 | kernel transaction APIs | Use only when public builder cannot express required state transition. | P14, P20 | protocol/concurrency suite. |
| EXT-08 | LogStore/raw actions/object-store manipulation | Last resort; requires explicit protocol correctness, recovery, and compatibility design. | P14, P20 | exhaustive negative/recovery tests. |

## TST — Contract-derived Delta testing

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
|---|---|---|---|---|
| TST-01 | schema contract tests | Exact/nullability/nested/metadata/evolution/constraint fixtures. | P12, P25 | unit + integration. |
| TST-02 | snapshot/version tests | Latest/stale/pinned/as-of/lazy/same-version-checkpoint behavior. | P11, P19, P25 | local + object-store fixtures. |
| TST-03 | protocol/feature tests | Supported/unsupported declared feature matrix by operation. | P20, P25 | negative feature fixtures. |
| TST-04 | write-mode tests | Append/overwrite/error/ignore/replaceWhere/schema/cast behavior. | P16, P25 | result + failure assertions. |
| TST-05 | transaction/concurrency tests | Conflicts, retries, unknown outcome, idempotency, storage locking. | P16, P20, P25 | concurrent workers/fault injection. |
| TST-06 | DML tests | Delete/update/merge clause order, duplicate matching, rewritten files, constraints. | P25 | adversarial source/target fixtures. |
| TST-07 | CDF tests | versions, change types, ICT, DVs, checkpoint/replay, retention, schema evolution. | P19, P25 | incremental replay harness. |
| TST-08 | DataFusion provider tests | exact version, refresh/rebuild, pushdown/pruning, file selection, schema adaptation. | P15, P25 | EXPLAIN + result tests. |
| TST-09 | maintenance tests | optimize semantic equality, Z-order benchmark, vacuum dry-run/retention, restore, filesystem check. | P17, P25 | maintenance harness. |
| TST-10 | storage/backend tests | S3 lock/conditional writes, MinIO/LocalStack/Azure/GCS selected profiles, auth failures. | P5, P25 | backend CI. |
| TST-11 | provenance/observability tests | history/commit props/metrics/closure/retry fields. | P9, P10, P24, P25 | artifact join tests. |
| TST-12 | reproducibility/retention tests | reopen pins, replay, vacuum boundary, expired-version diagnostics. | P10, P19, P25 | replay CI. |
| TST-13 | path/physical compatibility tests | URI spaces/percent encoding, nested optionality, partition-name collisions. | P12, P22, P25 | golden files. |
| TST-14 | dependency/upgrade matrix | exact pin compile, duplicate-type gate, cross-engine fixtures, plan/performance drift. | P18, P22, P25 | release CI. |

---
# Part III — Requirement-to-feature decision flows

These flows are the operational bridge between high-level requirements and the Delta utilization catalogue. For a material design, the agent should select every applicable flow, record the decisions, and reference the chosen pattern IDs.

# 8. Table creation and schema-contract flow

```text
Does the requirement introduce a new durable logical table?
    ├─ no → bind existing table identity + exact schema/protocol contract
    └─ yes
        ↓
Define application TableSpec + SchemaContract
  fields / types / nullability / metadata
  partition columns
  constraints
  required properties/features
  retention class
  compatibility matrix
        ↓
Compile to Delta StructType / metadata / properties
        ↓
Do advanced table features appear?
    ├─ no → create with conservative protocol/features
    └─ yes → certify every reader/writer/DML/maintenance engine first
        ↓
Create table through high-level builder
        ↓
Reopen exact committed version
        ↓
Validate schema + protocol + properties + partitioning
        ↓
Register Delta provider and run golden write/read fixture
```

### Required selections

- `MOD-01`, `SCH-01`–`SCH-05`, `SCH-08`–`SCH-10`.
- `GOV-01`–`GOV-03` as applicable.
- `INT-05` for any advanced feature.
- `TST-01`, `TST-03`, `TST-14`.

### Agent questions

1. What is the application semantic schema authority?
2. Which Delta schema/properties are enforced vs annotations?
3. Are partition columns logically stable enough to become part of the durable table contract?
4. Which table features are required now vs merely possible later?
5. What cross-engine consumers must read/write the resulting table?
6. What retention/time-travel class applies from day one?

# 9. Read, snapshot, and freshness flow

```text
What does “read table” mean?
    ├─ exact reproducible state → with_version / load_version
    ├─ user “as of” time       → resolve timestamp, persist resolved version
    ├─ latest strict           → compare loaded vs latest, refresh before bind
    ├─ latest eventual         → refresh according to bounded-staleness policy
    └─ metadata-only           → metadata-first/lazy posture
        ↓
Pin exact version for operation lifetime
        ↓
Validate protocol/schema/features
        ↓
Does read require relational execution?
    ├─ no → snapshot/add-action/table APIs
    └─ yes → build/register Delta TableProvider for exact version
        ↓
Record resolved version + contract fingerprint
```

### Required selections

- `MOD-02`, `STA-01`–`STA-10`.
- `QRY-01`–`QRY-05` when using DataFusion.
- `OBS-03`, `OBS-08`.
- `TST-02`, `TST-08`.

### Agent questions

1. Is the operation allowed to observe a new version after it begins?
2. Is the loaded handle already known to be at the required version?
3. Could lazy loading merely shift cost to first query?
4. Are statistics required for normal query pruning?
5. Does a cached provider carry the same exact version as the requested snapshot?

# 10. Append/write flow

```text
What is the semantic write class?
    ├─ immutable append             → SaveMode::Append
    ├─ full deterministic replace  → SaveMode::Overwrite
    ├─ bounded slice regeneration  → Overwrite + replaceWhere
    ├─ create-once artifact         → ErrorIfExists
    └─ bootstrap-if-absent          → Ignore only with explicit caller semantics
        ↓
Define exact schema policy
    ├─ strict (default)
    ├─ Merge (approved additive evolution)
    └─ Overwrite (controlled rebuild/migration)
        ↓
Define idempotency / retry identity
        ↓
Define physical file-size / partition policy separately
        ↓
Choose input
    ├─ Arrow RecordBatch(es) → DeltaTable::write
    ├─ DataFusion LogicalPlan → with_input_plan + exact SessionState
    └─ true blind append      → BlindDeltaTable after capability review
        ↓
Validate table protocol/features + input schema/predicate
        ↓
Execute files → optimistic validate/commit
        ↓
On ambiguous failure: reconcile history/version before retry
        ↓
Record before/after versions + metrics + provenance
```

### Required selections

- `MOD-03`, `TXN-01`–`TXN-08`, `WRT-01`–`WRT-08`.
- `SCH-03`, `SCH-06`, `SCH-08`.
- `OBS-01`, `OBS-04`–`OBS-06`.
- `TST-04`, `TST-05`.

### Agent questions

1. Is append actually idempotent under retry?
2. Can the write outcome be recognized if the commit response is lost?
3. Is schema merge truly a migration, or a convenience workaround?
4. Does `replaceWhere` input provably satisfy the predicate?
5. Is the workload truly blind append, or will a later step need active-file state?
6. Are file size and partition choices semantic or merely physical?

# 11. Delete/update/merge flow

```text
What row-level transition is required?
    ├─ remove rows by deterministic predicate → delete
    ├─ change columns by predicate            → update
    ├─ reconcile source/target keyed state    → merge
    └─ replace complete deterministic slice   → reconsider replaceWhere first
        ↓
Bind exact target snapshot/version
        ↓
Validate append-only / protocol / column-mapping / feature restrictions
        ↓
Compile predicates/assignments as DataFusion Expr
        ↓
For merge:
  source identity + alias
  target alias
  match condition
  ordered clauses
  duplicate-source match policy
        ↓
Define idempotency + conflict retry posture
        ↓
Execute rewrite + optimistic commit
        ↓
Verify resulting rows + constraints + schema
        ↓
Record before/after versions + rewrite metrics
```

### Required selections

- `DML-01`–`DML-08`, `TXN-01`–`TXN-07`.
- `GOV-01`, `GOV-02`, `GOV-05`, `GOV-06`.
- `TST-05`, `TST-06`.

### Agent questions

1. Could `replaceWhere` express the operation more simply and reproducibly?
2. Is the predicate deterministic and type-correct?
3. Can a source row match more than one target row or vice versa?
4. What happens if another writer removes one of the files selected for rewrite?
5. Are row constraints revalidated after the mutation?

# 12. Schema/protocol migration flow

```text
What is changing?
    ├─ field metadata only
    ├─ add nullable fields
    ├─ nullability / NOT NULL relaxation
    ├─ table properties
    ├─ constraints
    ├─ protocol/table feature
    ├─ column mapping
    └─ breaking type/name/layout change
        ↓
Classify compatibility:
  backward / forward / additive / breaking / unsupported
        ↓
Inspect exact reader/writer feature support at pinned delta-rs rev
        ↓
Inspect operation-specific restrictions
        ↓
Certify every required external engine
        ↓
Choose high-level migration builder or table rebuild
        ↓
Commit new version
        ↓
Reopen and validate contract fingerprint
        ↓
Rebuild dependent providers/caches and publish new contract version
```

### Required selections

- `SCH-06`–`SCH-12`, `GOV-05`–`GOV-10`, `INT-05`–`INT-08`.
- `TST-01`, `TST-03`, `TST-14`.

### Agent questions

1. Does protocol recognition actually imply the intended operation supports the feature?
2. Can the change be represented as an additive migration instead of a breaking rewrite?
3. Do CDF consumers span the schema change?
4. Does column mapping make the operation unsupported even if normal reads work?
5. What rollback is possible after a protocol upgrade?

# 13. CDF / incremental-consumption flow

```text
Is incremental row-change consumption required?
    ├─ no → use exact snapshot reads
    └─ yes
        ↓
Confirm CDF is enabled and retention satisfies maximum consumer lag
        ↓
Choose baseline
    ├─ exact starting version
    └─ initial full snapshot at version N, then CDF from N+1
        ↓
Read exact version range with scan_cdf
        ↓
Interpret _change_type semantics
        ↓
Use _commit_version as authoritative ordering/checkpoint identity
        ↓
Use _commit_timestamp / inCommitTimestamp only as temporal metadata/filter
        ↓
Apply downstream changes idempotently
        ↓
Persist consumer checkpoint only after downstream commit succeeds
        ↓
Before vacuum: validate every consumer checkpoint against retention
```

### Required selections

- `CDF-01`–`CDF-10`, `GOV-03`, `GOV-08`.
- `OBS-07`, `OBS-10`.
- `TST-07`, `TST-12`.

### Agent questions

1. Can the consumer be offline longer than Delta/CDF retention?
2. Is a durable downstream audit stream required beyond source retention?
3. How are update preimage/postimage handled?
4. Are deletion vectors enabled, and are DV changes covered by fixtures?
5. Does a schema migration split the consumer's range into incompatible eras?

# 14. Query-serving / DataFusion flow

```text
What table versions should this query see?
        ↓
Resolve application query/serving snapshot manifest
        ↓
For each Delta table:
  load exact version
  validate schema/protocol/features
  prepare intended DataFusion RuntimeEnv/object store
  build exact-version Delta TableProvider
        ↓
Register providers under stable logical names
        ↓
Compile SQL/DataFrame/PlanSpec
        ↓
Preserve projection/filter/partition visibility
        ↓
EXPLAIN / execute
        ↓
Record table versions + plan/config fingerprints + pruning/metrics
```

### Required selections

- `QRY-01`–`QRY-10`, `STA-03`, `MOD-07`.
- `LAY-01`–`LAY-07`.
- `OBS-01`, `OBS-03`, `OBS-08`, `OBS-09`.
- `TST-08`.

### Agent questions

1. Does the application require a coherent version set across multiple tables?
2. Can any registered provider be stale relative to the serving snapshot?
3. Are object-store mappings tenant/environment correct and immutable for the query?
4. Is `skip_stats` disabling expected pruning?
5. Is the query bypassing Delta-aware schema/deletion-vector behavior?

# 15. File-selection / targeted-repair read flow

```text
Do you need only a known subset of active files?
    ├─ no → ordinary Delta provider scan
    └─ yes
        ↓
Derive selected files from snapshot-owned Add/file APIs
        ↓
Choose MissingSelectedFilePolicy
    ├─ Error  → deterministic exact file set
    └─ Ignore → maintenance workflow tolerating no-longer-active selected files
        ↓
Build FileSelection / with_adds / with_file_paths
        ↓
Keep snapshot as metadata/deletion-vector/partition/statistics authority
        ↓
Execute targeted read
        ↓
Record snapshot version + selected logical file identities
```

### Required selections

- `LAY-09`, `LAY-10`, `QRY-10`, `EXT-05`.
- `TST-08`, `TST-13`.

### Agent questions

1. Are paths active in the pinned snapshot?
2. Should missing files fail or be ignored?
3. Are absolute paths under the table root?
4. Is a targeted scan being incorrectly treated as an alternative table state?

# 16. Optimize / layout-maintenance flow

```text
Is there measured layout pain?
    ├─ no → do not optimize by habit
    └─ yes
        ↓
Measure file count/size + representative query performance
        ↓
Choose maintenance intent
    ├─ compact small files
    └─ Z-order selected columns after benchmark rationale
        ↓
Prefer closed/inactive partition scope
        ↓
Inject production SessionState/runtime
        ↓
Execute optimize with target file/layout policy
        ↓
Validate logical row/schema equality
        ↓
Record files added/removed/skipped + before/after version
        ↓
Benchmark queries again
```

### Required selections

- `LAY-03`–`LAY-07`, `MNT-01`–`MNT-03`, `MNT-10`.
- `QRY-06`, `OBS-05`.
- `TST-09`, `TST-13`.

### Agent questions

1. Is the target partition still receiving conflicting DML?
2. Does Z-order have a benchmark-proven workload benefit?
3. Does optimize preserve exact logical Delta schema and data?
4. Are the two latest nested-schema regressions in the upgrade suite?

# 17. Vacuum / retention flow

```text
Why is physical cleanup required?
        ↓
Resolve table class + business/audit/replay/CDF retention commitments
        ↓
Enumerate pinned versions / long readers / CDF checkpoints
        ↓
Choose retention + keep_versions + mode
        ↓
Run dry run
        ↓
Review candidate scope/count and approve
        ↓
Execute vacuum
        ↓
Reopen latest + required retained versions
        ↓
Verify CDF/replay consumers remain valid
        ↓
Record retention policy + candidates/deletions + approval ID
```

### Required selections

- `GOV-08`, `GOV-09`, `MNT-04`–`MNT-06`.
- `OBS-01`, `OBS-05`, `OBS-10`.
- `TST-09`, `TST-12`.

### Agent questions

1. Which exact old versions still have contractual value?
2. Could any long reader still reference tombstoned files?
3. Are CDF consumers caught up?
4. Is retention enforcement being disabled, and if so what exceptional governance permits it?
5. Does the application understand that vacuum can make old versions unrestorable/unreadable?

# 18. Restore / incident-repair flow

```text
What is the incident?
    ├─ logical bad state with files retained → restore target version/time
    ├─ missing active files                 → filesystem check preflight
    └─ storage corruption / unsupported loss → escalate; do not conceal with automatic repair
        ↓
Pin current version + target/reference state
        ↓
Dry-run / verify target files and protocol compatibility
        ↓
Require authorization / incident ID
        ↓
Execute restore or repair builder
        ↓
Commit new version if state changes
        ↓
Validate latest query + schema/protocol + retained history
        ↓
Record degraded semantics if missing files were ignored
```

### Required selections

- `MNT-07`, `MNT-08`, `GOV-09`.
- `OBS-01`, `OBS-05`, `OBS-10`.
- `TST-09`.

### Agent questions

1. Have vacuumed files made exact restore impossible?
2. Is protocol downgrade allowed and certified?
3. Would `ignore_missing_files=true` produce a knowingly degraded result?
4. Should filesystem repair remove missing file references, or should the incident remain fail-closed?

# 19. Storage/backend flow

```text
Select canonical table URI and backend
        ↓
Choose native backend feature where available
        ↓
Choose TLS posture
        ↓
Resolve credentials/workload identity outside semantic table spec
        ↓
For S3-style multi-writer:
  configure locking or supported conditional commit semantics
        ↓
Build typed storage configuration → Delta options map
        ↓
Open table and register same object store into intended DataFusion runtime
        ↓
Run backend smoke + concurrency + negative credential tests
```

### Required selections

- `STO-01`–`STO-10`, `TXN-08`.
- `TST-10`, `TST-14`.

### Agent questions

1. Do all writers use exactly the same table root identity?
2. Is unsafe rename enabled anywhere outside proven single-writer/test scope?
3. Could a reused DataFusion runtime already contain a stale/wrong object-store mapping?
4. Are secrets excluded from logs and commit metadata?
5. Is OpenDAL being used because a native backend is unavailable, or accidentally shadowing a native path?

# 20. Provenance / reproducibility flow

```text
Before operation:
  allocate execution / operation / publication identity
  resolve exact input table versions
  resolve schema/protocol/config/code fingerprints
        ↓
During planning/execution:
  retain semantic spec + DataFusion plan/preflight artifacts as applicable
        ↓
At commit/result:
  record output Delta version(s), commit refs, metrics, CDF effects
        ↓
Link output to input versions and semantic/environment artifacts
        ↓
Evaluate retention/replay viability
        ↓
Mark provenance closure and reproducibility status
```

### Required selections

- `OBS-01`–`OBS-10`, `MOD-06`, `MOD-07`.
- `STA-03`, `TXN-05`, `TXN-06`.
- `TST-11`, `TST-12`.

### Agent questions

1. Can the durable result recursively resolve all Delta versions that produced it?
2. Is application provenance retained longer than Delta log history if required?
3. Can every required input version still be reopened?
4. Are same-version checkpoint/cache changes correctly excluded from semantic identity?

---

# Part IV — Required agent design artifacts

The following artifacts operationalize the design constitution for Delta-backed systems. For small/local workflows some may be compact, but important production state transitions should make these decisions explicit.

# 21. `SemanticRequirement`

```yaml
semantic_requirement:
  id: stable requirement id
  objective: externally meaningful read or state transition
  tables:
    - logical_table_id: ...
      access: read | append | overwrite | dml | schema_migration | maintenance | cdf
  snapshot_semantics: latest_strict | latest_eventual | pinned | as_of | metadata_first
  output_contract: ...
  invariants:
    - machine-testable invariant
  atomicity_scope: per_table | application_publication
  retention_expectations: ...
  non_semantic_preferences:
    - latency, file size, concurrency, compression
  prohibited_shortcuts:
    - raw parquet bypass, blind retry, unapproved vacuum, etc.
```

# 22. `AuthorityMap`

| Concept | Authority | Mutable by | Derived representations | Staleness/invalidation | Provenance identity |
|---|---|---|---|---|---|
| table semantic contract | application `TableSpec` / `SchemaContract` | governance workflow | Delta metadata/schema, Arrow schema, provider schema | contract fingerprint mismatch | table/spec ID + version |
| durable table state | Delta transaction log | committed Delta operations | `DeltaTable`, snapshot, provider, file report | loaded version differs | table ID + Delta version |
| multi-table state | application publication manifest | publication coordinator | provider set/query snapshot | any table version mismatch | publication ID/fingerprint |
| CDF consumer progress | consumer checkpoint store | consumer commit | next version range | checkpoint lag/retention | source table ID + version |
| maintenance policy | application policy | governance workflow | optimize/vacuum builder config | policy version change | maintenance policy ID |

# 23. `DeltaTableContract`

```yaml
delta_table_contract:
  logical_table_id: ...
  canonical_uri_id: ...
  schema:
    contract_version: ...
    fingerprint: ...
    fields: ...
  partition_columns: [...]
  constraints: [...]
  properties:
    append_only: ...
    cdf_enabled: ...
    retention: ...
  protocol:
    min_reader_version: ...
    min_writer_version: ...
    reader_features: [...]
    writer_features: [...]
  advanced_feature_posture:
    column_mapping: disabled | certified | restricted
    deletion_vectors: ...
    v2_checkpoint: ...
    variant: ...
    type_widening: unsupported | certified
  supported_operations:
    read: ...
    write: ...
    dml: ...
    cdf: ...
    optimize: ...
    vacuum: ...
  cross_engine_matrix: ...
```

# 24. `SnapshotPolicy`

```yaml
snapshot_policy:
  mode: latest_strict | latest_eventual | pinned_version | as_of_time | metadata_first
  requested_version: ...
  requested_as_of: ...
  resolved_version: ...
  staleness_budget: ...
  require_stats: true
  allow_lazy_files: ...
  provider_rebuild_on_refresh: true
  persist_resolved_pin: true
```

# 25. `TransactionContract`

```yaml
transaction_contract:
  operation_id: ...
  input_delta_version: ...
  mutation_class: append | overwrite | replace_where | delete | update | merge | migration | restore
  idempotency:
    deterministic_key: ...
    retry_safe: ...
    duplicate_detection: ...
  conflict_posture:
    expected_conflicts: ...
    retry_limit_policy: ...
  ambiguous_commit_reconciliation:
    inspect_history: true
    operation_reference: ...
  provenance_commit_properties: [...]
  expected_output:
    new_version: one committed successor/retried successor
    verification: ...
```

# 26. `FeatureUtilizationPlan`

| Requirement/building block | Selected pattern IDs | Native Delta features | Application overlay | Why highest viable level | Key contracts | Evidence |
|---|---|---|---|---|---|---|
| reproducible read | STA-03, QRY-05, OBS-03 | exact version + provider | publication/query snapshot | preserves Delta authority and DataFusion pruning | version/schema/protocol | replay + EXPLAIN |
| bounded rerun | WRT-03, TXN-03, TXN-04 | overwrite + replaceWhere | operation/idempotency model | simpler than row merge when whole slice regenerates | predicate + retry | predicate/retry tests |
| incremental projection | CDF-03–CDF-09 | CDF | durable consumer checkpoint | native change semantics | version order + retention | crash/replay tests |

# 27. `ContractAndCapabilityMatrix`

| Claim | Semantic class | Exact/partial/unsupported | Owner | Consumer/enforcer | Failure consequence | Test |
|---|---|---|---|---|---|---|
| exact table state at version N | native authority | exact | Delta log | snapshot/provider | wrong reproducibility if false | pinned read fixture |
| column mapping write | protocol + operation capability | operation-specific | delta-rs | write/DML builder | corrupt/incompatible table if overclaimed | feature matrix |
| v2Checkpoint declared | protocol compatibility | recognized | delta-rs protocol checker | reader/writer gate | false incompatibility if absent | feature fixture |
| commit metadata `schema_contract_version` | lineage metadata | advisory/reference | application | provenance resolver | lineage gap | history join test |
| file statistics | planner metadata | exact/inexact/absent by file/action | Delta/provider | DataFusion pruning | performance drift, not business correctness | pruning test |

# 28. `LifecycleArtifactMap`

| Phase | Input | Delta/native artifact | Application artifact | Gate | Failure class |
|---|---|---|---|---|---|
| declare | requirement | none | operation spec | semantic validation | `declaration.*` |
| resolve | table spec | URI/log store | resolved table identity | authorization/storage policy | `resolution.*` |
| snapshot | table identity | exact snapshot/version | snapshot pin | freshness + retention | `snapshot.*` |
| validate | snapshot + spec | schema/protocol/properties | contract report | feature/policy gate | `contract.*` |
| plan | validated op | DataFusion plan / candidate files | plan/preflight artifact | predicate/layout safety | `planning.*` |
| execute data | plan/input | Arrow/Parquet + candidate actions | execution telemetry | resource/schema checks | `execution.*` |
| commit | candidate transition | transaction-log commit | transaction record | optimistic validation | `commit.*` |
| verify | new version | reopened snapshot | verification report | output contract | `verification.*` |
| observe/publish | verified version | history/metrics/CDF | provenance/publication | closure/repro status | `publication.*` |

# 29. `ProvenanceClosureMap`

```yaml
provenance:
  operation_id: ...
  publication_id: ...
  semantic_spec:
    id: ...
    version: ...
    fingerprint: ...
  inputs:
    - table_id: ...
      delta_version: ...
      schema_fingerprint: ...
      protocol_fingerprint: ...
  output:
    table_id: ...
    delta_version: ...
    commit_reference: ...
  planning:
    datafusion_plan_artifact: ...
    config_fingerprint: ...
  environment:
    deltalake_rev: 43a0cf10a313e5077c48637ad786a05359136bbb
    datafusion: 55.0.0
    arrow: 59.2.0
    parquet: 59.2.0
    object_store: 0.13.2
    rust: 1.94.1
    cargo_lock_fingerprint: ...
  observations:
    operation_metrics: ...
    write_retry_count: ...
  retention:
    replay_versions_still_available: ...
    cdf_range_still_available: ...
  reproducibility:
    exact_table_inputs_pinned: ...
    environment_recorded: ...
    missing_links: [...]
```

# 30. `StateOwnershipMap`

| State | Scope | Owner | Mutable? | Lifetime | Authority relationship | Refresh/reset | Concurrency/invalidation |
|---|---|---|---|---|---|---|---|
| loaded table state | service cache/table handle | table registry | yes via refresh/load | handle lifetime | derived from exact Delta version | `update_*` / reload | version-tagged |
| snapshot file/stat cache | process/table internals | delta-rs | yes/rebuildable | process | non-authoritative | replay/rebuild | identity/capability checked |
| provider | session/query snapshot | provider registry | immutable view | registration/query | derived from Delta snapshot | rebuild on version change | exact-version key |
| transaction actions | request/operation | mutation executor | yes until commit | one attempt | non-authoritative candidate | discard/retry | operation scoped |
| CDF checkpoint | consumer | checkpoint store | yes | consumer lifetime | application progress authority | monotonic commit | exactly-once protocol |
| maintenance approval | job | governance workflow | controlled | job/audit retention | application policy state | immutable after execution | approval ID |
| object-store mapping | runtime/session | runtime owner | controlled | session/runtime | resource state only | replace/new context | tenant/env isolated |

# 31. `MaintenanceSafetyReview`

```yaml
maintenance_safety:
  table_id: ...
  loaded_version: ...
  operation: optimize | zorder | vacuum | restore | filesystem_check
  target_scope: ...
  active_writer_assessment: ...
  cdf_consumer_checkpoints: ...
  pinned_versions_required: [...]
  retention_policy: ...
  dry_run:
    required: ...
    artifact: ...
  semantic_equality_check: ...
  rollback_posture: ...
  approval_id: ...
  post_validation: ...
```

# 32. `TestEvidenceMatrix`

| Contract ID | Claim | Positive tests | Negative/adversarial tests | Concurrency/fault tests | Cross-engine tests | Upgrade tests | CI gate |
|---|---|---|---|---|---|---|---|

Every row in `ContractAndCapabilityMatrix` must map to at least one evidence row.

# 33. `OperationSelectionRecord`

```yaml
operation_selection:
  requirement: ...
  candidates_reviewed:
    - DeltaTable load/provider
    - DeltaTable write
    - BlindDeltaTable
    - delete/update/merge
    - schema/constraint/property/feature builders
    - CDF
    - optimize/vacuum/restore/filesystem-check
    - FileSelection
    - kernel transaction APIs
    - LogStore/raw actions
  selected_level: ...
  why_higher_levels_are_insufficient: ...
  protocol_risk_added: ...
  concurrency_risk_added: ...
  required_tests: ...
```

---
# Part V — Crosswalks for future functional building blocks

# 34. Principle-to-pattern crosswalk

| Principle | Primary Delta utilization patterns |
|---|---|
| P1 — Model semantics before implementing behavior | MOD-01–MOD-08, STA-01–STA-05, WRT-01–WRT-04, DML-01–DML-04 |
| P2 — Make models executable, not merely descriptive | MOD-02–MOD-08, TXN-01–TXN-06, OBS-01–OBS-06, TST-01–TST-12 |
| P3 — One authoritative owner for every concept | MOD-04, MOD-07, STA-01–STA-10, QRY-01–QRY-05, OBS-03 |
| P4 — Explicit conceptual hierarchies | STO-01–STO-08, QRY-01–QRY-06, SCH-01–SCH-04 |
| P5 — Variability behind contracts | STO-01–STO-10, QRY-01–QRY-02, INT-05, TST-10 |
| P6 — Separate semantics from execution | STA-06–STA-09, WRT-05–WRT-08, LAY-01–LAY-08, MNT-01–MNT-06 |
| P7 — Shared canonical data fabric | QRY-01–QRY-10, WRT-01, STA-01, INT-01–INT-06 |
| P8 — Common representation as infrastructure | WRT-01, WRT-05–WRT-08, LAY-01–LAY-08, INT-01–INT-03 |
| P9 — Intrinsic provenance | OBS-01–OBS-10, TXN-05–TXN-07, CDF-05, MNT-08 |
| P10 — Provenance closure | OBS-01–OBS-10, STA-03–STA-04, CDF-03–CDF-06, MOD-07, MNT-05 |
| P11 — Immutable snapshots and explicit transitions | STA-01–STA-10, TXN-01–TXN-06, QRY-03–QRY-05, MNT-07 |
| P12 — Schema as executable contract | SCH-01–SCH-12, GOV-01–GOV-07, QRY-10, TST-01–TST-04 |
| P13 — Governance at authoritative boundary | GOV-01–GOV-10, TXN-08, MNT-04–MNT-08, STO-04, STO-06 |
| P14 — Highest-level extension | EXT-01–EXT-08, WRT-01–WRT-08, DML-01–DML-08, MNT-01–MNT-08 |
| P15 — Preserve optimizer visibility | QRY-01–QRY-10, LAY-01–LAY-10, STA-07, SCH-11 |
| P16 — Lifecycle phases | MOD-05, TXN-01–TXN-08, WRT-01–WRT-04, DML-01–DML-08, CDF-01–CDF-10, MNT-01–MNT-08 |
| P17 — Inspectable/reproducible intermediates | OBS-01–OBS-10, MNT-04, MNT-07–MNT-08, QRY-09, TST-11 |
| P18 — Fingerprint important identity | MOD-06–MOD-07, STA-09, SCH-10, OBS-03, OBS-08–OBS-09, STO-09 |
| P19 — Reproducibility normal mode | STA-03–STA-05, MOD-07, OBS-01–OBS-10, CDF-03–CDF-06, MNT-05, TST-12 |
| P20 — Conservative capability claims | SCH-08–SCH-09, GOV-05–GOV-10, TXN-02–TXN-04, CDF-06–CDF-10, EXT-06–EXT-08 |
| P21 — Enforced vs advisory metadata | SCH-05–SCH-07, GOV-01–GOV-05, TXN-05, OBS-04 |
| P22 — Protocol/canonical interoperability | INT-01–INT-10, SCH-08–SCH-11, STO-01–STO-07, CDF-02, TST-13–TST-14 |
| P23 — Explicit state ownership | STA-05–STA-10, QRY-02–QRY-06, CDF-04, STO-03, STO-06, TXN-01 |
| P24 — Semantic observability | OBS-01–OBS-10, TXN-06–TXN-07, QRY-09, LAY-03–LAY-05, MNT-08 |
| P25 — Contract-derived testing | TST-01–TST-14 and every feature pattern's evidence column |

# 35. Delta feature-family-to-principle crosswalk

| Delta capability family | Primary principles advanced | Typical functional building blocks |
|---|---|---|
| Delta transaction log / version | P3, P9, P10, P11, P16, P18, P19, P24 | durable state, publication, replay, audit |
| `DeltaTable` loading / snapshot / time travel | P3, P11, P16, P19, P23 | current reads, historical reads, replay, serving pins |
| schema / metadata / properties | P1, P3, P12, P13, P21, P22, P25 | table contracts, migrations, validation |
| protocol / table features | P12, P13, P20, P22, P25 | compatibility, feature rollout, cross-engine governance |
| Arrow writes / `WriteBuilder` | P2, P6, P7, P8, P11, P16, P19 | append, overwrite, materialization, ingestion |
| optimistic transaction / commit properties | P9, P11, P16, P19, P20, P24 | concurrency, retries, provenance, idempotency |
| delete/update/merge | P1, P2, P11, P13, P16, P20, P25 | corrections, upserts, privacy deletes, current state |
| CDF | P7, P9, P10, P11, P16, P19, P22, P24 | incremental materialization, cache invalidation, CDC |
| Delta TableProvider / DataFusion integration | P5, P6, P7, P12, P15, P23, P24, P25 | query serving, SQL/DataFrame, plan-backed writes |
| partitioning / stats / file skipping | P6, P8, P15, P20, P24, P25 | layout, query performance, pruning |
| `FileSelection` / active-file metadata | P3, P5, P15, P20, P23 | repair scans, quality checks, targeted maintenance |
| optimize / Z-order | P6, P14, P15, P16, P24, P25 | compaction, layout improvement |
| vacuum / restore | P10, P11, P13, P16, P17, P19, P24, P25 | retention, cleanup, rollback, incident recovery |
| filesystem check | P13, P16, P17, P24, P25 | corruption detection and controlled repair |
| checkpoint / lazy snapshot internals | P6, P11, P18, P23 | replay performance, activation latency |
| LogStore / ObjectStore / cloud features | P4, P5, P13, P20, P22, P23 | deployment, storage portability, safe commits |
| history / metrics / commit info | P9, P10, P17, P24, P25 | audit, lineage, observability, contention analysis |

# 36. Preparation for future functional-building-block catalogue

Future functional building blocks can map directly to the pattern IDs above:

```yaml
functional_building_block:
  id: ...
  semantic_purpose: ...
  table_authorities:
    - logical_table_id: ...
      required_snapshot_semantics: ...
  input_contracts: ...
  output_state_transition: ...
  lifecycle_phases: ...
  selected_delta_patterns:
    - MOD-..
    - STA-..
    - SCH/GOV/TXN/WRT/DML/CDF/LAY/MNT/STO/QRY/OBS/INT/EXT/TST-..
  atomicity_scope: ...
  legal_variation: ...
  protocol_feature_requirements: ...
  provenance_outputs: ...
  retention_consequences: ...
  state_and_resource_scope: ...
  interoperability_boundaries: ...
  test_evidence: ...
```

This preserves a crucial separation: **the functional catalogue says what the fabric must do; this manual says how Delta capabilities should be used to realize it without violating the data-fabric constitution.**

---

# Part VI — Comprehensive agent review checklist

# 37. Semantic and authority review

- [ ] The table/state-transition meaning exists as a typed application model or the design explains why one is unnecessary.
- [ ] One semantic authority is named for the table contract and one exact Delta version is named for persisted table state.
- [ ] Loaded `DeltaTable`, snapshot, provider, cache, checkpoint, Parquet files, and object-store listings are explicitly classified as authority or derived state.
- [ ] Multi-table consistency uses an application version manifest/publication rather than implied Delta atomicity.
- [ ] A checkpoint is never treated as a semantic table version.
- [ ] Every cache/provider carries the exact Delta version and relevant contract/config fingerprints.

# 38. Snapshot and freshness review

- [ ] Read policy is one of latest strict / latest eventual / pinned version / as-of time / metadata-first.
- [ ] As-of timestamps are resolved and persisted as exact versions.
- [ ] Long-running or reproducible queries pin versions for their full lifetime.
- [ ] Shared handles are not mutated across incompatible freshness/time-travel semantics.
- [ ] Provider rebuild rules are explicit after table refresh.
- [ ] Lazy/without-files mode is treated as a performance posture, not a guarantee that file state will never be materialized.
- [ ] Query-serving statistics policy is explicit and normally keeps pruning-capable stats.
- [ ] Same-version checkpoint adoption is identity-neutral.

# 39. Schema/protocol contract review

- [ ] Application schema authority, Delta `StructType`, Arrow schema, provider schema, and Parquet physical schema have explicit relationships.
- [ ] Types, nullability, decimal precision/scale, timestamp semantics, nested structure, partition columns, and metadata classes are defined.
- [ ] Strict schema is the default write posture.
- [ ] Merge/overwrite schema changes are explicit migrations.
- [ ] Constraints and NOT NULL are used where Delta can enforce required invariants.
- [ ] Reader/writer protocol and declared features are validated before every governed operation class.
- [ ] Operation-specific feature restrictions are recorded.
- [ ] Column mapping, variant, deletion vectors, V2 checkpoints, nanosecond timestamps, and type widening have explicit certification posture.
- [ ] Nested logical non-null / physical optional Parquet interop is tested.
- [ ] Nested field names matching top-level partition-column names are tested.

# 40. Transaction and write review

- [ ] Every mutation has an input version, operation ID, idempotency/retry model, expected output contract, and before/after version record.
- [ ] Save mode is explicit.
- [ ] Schema mode is explicit if enabled.
- [ ] Cast policy is explicit.
- [ ] `replaceWhere` rows are prevalidated against the predicate.
- [ ] DataFusion plan writes preserve the exact intended `SessionState` / runtime configuration.
- [ ] File sizing/partitioning/compression are classified as physical policy, not hidden semantics.
- [ ] Blind append uses `BlindDeltaTable` only when no file-state reads/DML are required.
- [ ] Unknown commit outcomes are reconciled before retry.
- [ ] Write retry metrics are captured.
- [ ] Multi-writer S3-style deployments have safe locking/conditional commit semantics.

# 41. DML review

- [ ] Delete/update/merge predicates are deterministic and type-correct.
- [ ] Merge aliases and clause ordering are explicit.
- [ ] Duplicate source/target matches have a defined policy.
- [ ] Append-only and advanced-feature restrictions are checked first.
- [ ] DataFusion session/runtime state is injected where required.
- [ ] Rewritten-file/row metrics are observed.
- [ ] Constraints and schema are revalidated after the mutation.
- [ ] Retry behavior cannot duplicate or corrupt the intended logical transition.

# 42. CDF review

- [ ] CDF enablement is governed and retention-aware.
- [ ] Consumer starts from an exact version or race-free initial-snapshot boundary.
- [ ] `_commit_version` is the canonical ordering/checkpoint identity.
- [ ] `_commit_timestamp` / in-commit timestamp is treated as temporal metadata, not exact identity.
- [ ] Consumer checkpoint is durable and updated only after downstream success.
- [ ] Duplicate replay is safe/idempotent.
- [ ] Deletion-vector CDF semantics are fixture-tested where applicable.
- [ ] Schema evolution across the consumed range has an explicit compatibility path.
- [ ] Vacuum safety checks include every CDF consumer checkpoint.
- [ ] A longer-lived audit/event layer exists if consumers can exceed source retention.

# 43. Query/provider review

- [ ] Delta provider is used instead of raw Parquet registration.
- [ ] Provider exact version is recorded.
- [ ] Runtime object-store mapping is correct and not silently stale.
- [ ] Projection/predicate/partition pruning is verified with `EXPLAIN`/benchmarks.
- [ ] Query-serving stats are not disabled accidentally.
- [ ] Delta-aware column mapping, nested schema adaptation, and deletion vectors remain inside the provider.
- [ ] File path diagnostic columns are access-controlled/redacted.
- [ ] Multi-table queries bind to one coherent application publication/version set.

# 44. Layout and maintenance review

- [ ] Partition columns are selected from query patterns and remain stable/low-enough cardinality.
- [ ] File statistics are retained/observed for important filter columns.
- [ ] Small-file thresholds and target sizes are table-class specific.
- [ ] Optimize is triggered by measured need, not schedule alone.
- [ ] Z-order has workload benchmark evidence.
- [ ] Optimize targets closed/inactive partitions when possible.
- [ ] Logical row/schema equality is verified before/after optimize.
- [ ] Vacuum always evaluates audit/replay/CDF/long-reader retention requirements.
- [ ] Governed vacuum performs dry run and retains review evidence.
- [ ] Required versions are protected/reopened after vacuum.
- [ ] Restore behavior before/after vacuum is understood.
- [ ] Filesystem check is treated as incident repair, not routine cleanup.

# 45. Storage/deployment review

- [ ] One canonical table root identity is used by all writers.
- [ ] `LogStore` and `ObjectStore` responsibilities are separate.
- [ ] Storage options are centralized in typed configuration.
- [ ] Secrets never enter logs, metrics, or commit metadata.
- [ ] TLS posture is deliberate.
- [ ] Only required cloud/backend features are enabled.
- [ ] S3 multi-writer safety is proven.
- [ ] Test/prod object-store endpoints cannot share a stale runtime mapping accidentally.
- [ ] Tenant credentials/object stores are correctly isolated.
- [ ] OpenDAL is used deliberately for long-tail stores and not assumed equivalent to native backend behavior.

# 46. Provenance/reproducibility review

- [ ] Operation identity is allocated before execution.
- [ ] Every read/write records exact input table versions.
- [ ] Every mutation records output version and commit reference.
- [ ] Schema/protocol/config/code/environment fingerprints are linked.
- [ ] Commit properties contain compact lineage references but no secrets.
- [ ] Delta history is not the only long-term provenance store if retention requirements exceed log retention.
- [ ] Reproducibility status records whether every required version/file still exists.
- [ ] Same-version checkpoint/cache changes do not alter semantic identity.
- [ ] A durable result can recursively resolve its input Delta versions and operation/spec/environment artifacts.

# 47. Test-evidence review

- [ ] Every capability claim maps to tests.
- [ ] Exact-version/latest/time-travel semantics are tested.
- [ ] Strict schema and constraint failures are tested.
- [ ] Protocol/table-feature negative cases are tested.
- [ ] Concurrent mutation conflicts and unknown commit outcomes are fault-injected.
- [ ] DML clause/duplicate-match edge cases are tested.
- [ ] CDF deletion-vector/ICT/replay/retention cases are tested.
- [ ] Provider refresh/pruning/schema adaptation is tested.
- [ ] Optimize semantic equality and nested-schema regressions are tested.
- [ ] Vacuum/restore/retention breakage is tested.
- [ ] Storage backend concurrency and credential failures are tested.
- [ ] Cross-engine compatibility is tested for every advanced feature used.
- [ ] Dependency/type-universe drift is a CI gate.

---

# Part VII — Anti-pattern diagnosis and prescribed correction

| Anti-pattern | Delta symptom | Why it violates the constitution | Prescribed correction |
|---|---|---|---|
| Raw-Parquet bypass | query scans all files under table root directly | ignores transaction-log authority, tombstones, DVs, column mapping, schema adaptation | register Delta `TableProvider` |
| “Latest forever” handle | service opens once and never refreshes | loaded snapshot silently becomes stale | explicit freshness policy + version checks + provider rebuild |
| Timestamp-only identity | result records “as of 10:00” but not version | weaker replay and ordering | persist resolved exact Delta version |
| Checkpoint as semantic identity | replay cache keys use checkpoint file | checkpoint can appear at same version | key by table ID + Delta version; checkpoint diagnostic only |
| Duplicate authorities | external active-file DB disagrees with Delta log | independent drift | derive file inventory from exact snapshot |
| Blind append misuse | `BlindDeltaTable` later used for reads/DML assumptions | capability boundary violated | use full `DeltaTable` when file state matters |
| Blind write retry | append retried after timeout with no reconciliation | duplicates possible | operation ID + history/version reconciliation |
| Schema merge by convenience | every write enables `SchemaMode::Merge` | schema authority drifts through data arrival | strict default + governed migration |
| Protocol optimism | table feature recognized, assumed fully supported everywhere | operation-specific incompatibility/corruption risk | capability matrix by operation + engine |
| Column-mapping overclaim | normal reads work, so all schema/DML/optimize assumed safe | operation restrictions remain | certify exact operation; fail closed |
| Metadata theater | commit/table metadata claims security/invariant | metadata is not authorization/constraint | enforce in policy/constraint layer |
| Maintenance as semantics | optimize/Z-order file layout encoded in domain model | physical strategy contaminates meaning | separate `MaintenancePolicy` |
| Vacuum as performance tuning | vacuum scheduled to “speed queries” | physical deletion mainly affects storage/retention | optimize for layout; vacuum for retention cleanup |
| Ungoverned vacuum | short retention run without consumer/pin checks | destroys replay/time travel/CDF | dry run + retention safety + keep-version review |
| Repair hides data loss | filesystem check auto-removes missing active files | normalizes corruption without incident record | dry run, alert, explicit incident authorization |
| Provider auto-refresh assumption | query still sees version N after table at N+1 | provider snapshot is derived state | rebuild/re-register according to freshness policy |
| Stats disabled in serving | `skip_stats` used for regular filtered queries | pruning visibility/performance lost | query profile with stats; metadata-only profile separate |
| Manual action path encoding | custom `%20`/space transformations | identity/double-encoding bugs | use delta-rs/object-store URI/path handling |
| Multi-table atomicity fiction | several Delta commits called “one transaction” | Delta atomicity is per table | application publication manifest/activation layer |
| History as infinite lineage store | provenance depends only on retained Delta log | retention eventually breaks closure | external provenance artifact/index |

---

# Part VIII — Compact LLM-agent instruction block

> **Use Delta Lake as the durable per-table transactional state authority, not as a directory of Parquet files.** Every meaningful read should resolve to an explicit snapshot policy and exact Delta version; every meaningful mutation should be modeled as `version N + typed operation -> committed version N+1`, with optimistic conflict handling, verification, provenance, and retry/reconciliation semantics.
>
> **Keep application semantics above Delta and compile them into Delta operations.** Define typed table, snapshot, write, DML, CDF, maintenance, retention, and provenance models. Prefer `DeltaTable` loading/provider/operation builders, `BlindDeltaTable` only for true blind appends, and public CDF/maintenance APIs before kernel transactions, log-store internals, or raw actions. Do not manually scan or mutate Parquet files under a Delta root for governed behavior.
>
> **Assign authority precisely.** The Delta transaction log at an exact version owns one table's durable state. In-memory `DeltaTable`/snapshot/provider objects, file/stat caches, checkpoints, and Parquet/object-store listings are derived execution state. A newly available checkpoint at the same Delta version does not create a new semantic state. Multi-table atomicity/serving consistency requires an application publication manifest pinning exact versions for all tables.
>
> **Treat schema, constraints, protocol, and table features as executable contracts.** Keep an application `SchemaContract` linked to the persisted Delta schema; default writes to strict schema; make merge/overwrite schema changes explicit migrations; validate protocol/features before every governed operation; fail closed on unsupported declared features. Treat column mapping, deletion vectors, variant, V2 checkpoints, nanosecond timestamps, type widening, and other advanced features as operation-specific compatibility surfaces, not blanket capabilities.
>
> **Separate semantic transition from physical strategy.** Save mode, predicate, schema policy, source/target identity, snapshot version, and retention obligations are semantic. File sizes, row groups, compression, lazy/eager materialization, checkpoint selection, optimize concurrency, Z-order, and vacuum traversal are physical policies. Same semantic state should remain invariant across physical tuning.
>
> **Preserve DataFusion/Arrow visibility.** Use Arrow `RecordBatch` as the data plane and the Delta `TableProvider` as the query boundary. Do not register raw Parquet folders for Delta tables. Preserve stats/partition metadata for query-serving pruning, pass the correct `SessionState` into plan-backed writes/DML/optimize, rebuild providers when snapshot freshness changes, and let delta-rs own deletion-vector, column-mapping, and nested physical-schema adaptation.
>
> **Make concurrency and retry semantics explicit.** Delta uses optimistic concurrency. Assign operation/idempotency identity before execution, record the input version, distinguish pre-commit failures from unknown commit outcomes, reconcile history/latest version before retrying ambiguous writes, and capture commit retry metrics. Configure safe locking/conditional commit semantics for multi-writer object stores.
>
> **Make provenance and reproducibility native to the flow.** Record exact input versions, output version, schema/protocol fingerprints, semantic model/config/code/environment identity, DataFusion plan artifacts when relevant, commit references, operation metrics, and retention status. Commit metadata should carry compact lineage references, not replace a durable provenance graph. CDF consumers should checkpoint `_commit_version`, not timestamps.
>
> **Treat retention and maintenance as governed lifecycle operations.** Optimize and Z-order change physical layout while preserving logical table state and require equality/benchmark evidence. Vacuum physically destroys old-file availability and must respect readers, pinned versions, restore commitments, audit/replay requirements, and CDF checkpoints. Governed vacuum should dry-run first. Restore is a new committed version; filesystem check is incident repair, not normal cleanup.
>
> **Be conservative about capability claims.** Protocol recognition does not prove every authoring/maintenance path is supported. Unknown/unsupported is safer than false confidence. Maintain an operation×feature×engine compatibility matrix and derive tests directly from every claim.
>
> **Do not implement until the design states:** semantic authority, exact snapshot/freshness policy, table contract, transaction/idempotency behavior, feature compatibility, physical layout policy, retention consequences, provenance closure, state ownership, operation-selection level, and test evidence.

---

# Appendix A — Version-specific leverage map for `43a0cf10` / DataFusion 55 / Arrow 59

This appendix highlights capabilities and changes that materially affect how the general patterns above should be applied to the pinned environment.

## A.1 Coordinated dependency universe

| Component | Pinned target | Design implication |
|---|---:|---|
| `deltalake` / `deltalake-core` | `1.0.0` @ `43a0cf10a313e5077c48637ad786a05359136bbb` | Treat exact git revision as source/API baseline until a stable Rust 1.0 tag is adopted. |
| DataFusion | `55.0.0` | Provider/planner/statistics/custom execution code must follow DF55 contracts. |
| Arrow / Parquet | `59.2.0` | Maintain one public type universe across Delta/DataFusion/application code. |
| `object_store` | `0.13.2` | Central storage abstraction and DataFusion runtime mapping. |
| Rust | `1.94.1`, edition 2024 | Build/toolchain contract and CI fingerprint. |
| `buoyant_kernel` / engine | `0.25.x`, Arrow 59 features | Kernel implementation dependency; do not expose internals as application authority. |

**Required use:** `INT-10`, `OBS-09`, `TST-14`.

## A.2 DataFusion 55 / Arrow 59 migration

The pin is aligned to DataFusion 55 and Arrow/Parquet 59. The reference documents corresponding upstream changes in custom `ExecutionPlan` expression traversal, physical planning context, statistics APIs, plan-codec converter arguments, external-table locations, and scalar-list coercion.

**Design implication:** application code that merely uses the high-level Delta provider/builders benefits from the integration automatically. Any direct custom DataFusion extension code must also comply with the companion DataFusion 55 / Arrow 59 design manual and test matrix.

**Patterns:** `QRY-01`–`QRY-10`, `INT-03`, `TST-08`, `TST-14`.

## A.3 Snapshot-native file discovery and replay-safe lightweight snapshots

The pinned line moves active-file discovery behind the native snapshot abstraction and hardens lazy/materialized cache identity. Metadata-first / `without_files` / `skip_stats` states can replay required file information later rather than relying on unsafe persisted process caches.

**Design leverage:**

- use `SnapshotPolicy` explicitly;
- treat materialized-file/stat caches as ephemeral and rebuildable;
- benchmark activation separately from first file-dependent query;
- keep query-serving stats policy explicit;
- never persist delta-rs file caches as application state.

**Patterns:** `STA-06`–`STA-09`, `LAY-07`, `TST-02`.

## A.4 Same-version checkpoint adoption

A checkpoint can become available after a process already loaded the same Delta version. The snapshot may rebuild at that **same logical version** to use the new checkpoint.

**Design leverage:**

```text
semantic identity = canonical table ID + Delta version
checkpoint version/file = diagnostic/replay implementation detail
```

Do not advance application publication/freshness generation solely because a new checkpoint file appears for an unchanged Delta version.

**Patterns:** `STA-09`, `MNT-09`, `OBS-03`, `TST-02`.

## A.5 `BlindDeltaTable`

`BlindDeltaTable` provides a metadata-only, stats-free handle optimized for append-only write workloads and intentionally excludes read/file-state semantics.

**Use when:**

- append is truly blind;
- schema/protocol/properties are sufficient;
- no merge/delete/update/query file discovery is needed in the same abstraction.

**Do not:** generalize it into the default `DeltaTable` replacement.

**Patterns:** `WRT-08`, `EXT-06`, `TST-04`.

## A.6 Deletion-vector-aware CDF

CDF now handles same-file deletion-vector changes to derive inserted/deleted row sets and uses Parquet row selections rather than naïve full-file masks.

**Design leverage:**

- rely on native CDF/DV semantics;
- add DV insert/delete/restore fixtures;
- do not manually infer DV deltas from Add/Remove pairs;
- keep `_commit_version` as consumer identity.

**Patterns:** `CDF-07`, `LAY-10`, `TST-07`.

## A.7 In-commit timestamp support

CDF timestamp-range filtering and emitted `_commit_timestamp` prefer `CommitInfo.inCommitTimestamp` when present and otherwise fall back to the ordinary commit timestamp.

**Design leverage:** timestamps are improved temporal metadata but **version remains the authoritative CDF ordering/checkpoint key**.

**Patterns:** `CDF-03`, `CDF-06`, `OBS-07`.

## A.8 `FileSelection` and `MissingSelectedFilePolicy`

The next-scan surface can restrict scans to explicit snapshot-known files while retaining all Delta metadata semantics such as deletion vectors, partition values, stats, and column mapping.

**Design leverage:** use for deterministic file-set re-reads, repair/quality scans, and targeted maintenance. Use `Error` for exact file-set requirements and `Ignore` only for maintenance workflows that intentionally tolerate files becoming inactive.

**Patterns:** `LAY-09`, `EXT-05`, `TST-08`, `TST-13`.

## A.9 Nested physical nullability and partition-name collision fixes

The Delta provider now adapts Spark-written nested Parquet fields that are physically optional while the Delta logical field remains non-nullable, and avoids mistaking nested fields for top-level partition columns merely because their names match.

**Design leverage:**

- retain strict logical Delta schema;
- rely on provider physical adaptation;
- add mandatory read/OPTIMIZE golden regressions;
- reject raw-Parquet query paths for governed Delta tables.

**Patterns:** `SCH-04`, `SCH-11`, `QRY-10`, `MNT-10`, `TST-13`.

## A.10 Write retry metrics

Write metrics now include `num_retries`, exposing optimistic commit contention directly.

**Design leverage:** ingest as a first-class semantic/operational metric:

```text
0   → normal uncontended commit
>0  → optimistic conflict/retry occurred
```

Trend by table, operation class, partition/slice, writer cohort, and latency.

**Patterns:** `TXN-07`, `OBS-06`, `TST-05`.

## A.11 Full-vacuum scan improvements

Full vacuum now uses hierarchical traversal, configurable/concurrent leaf scanning, and distinct orphan-scan errors, materially changing large-partition-tree performance assumptions.

**Design leverage:** re-benchmark vacuum mode/concurrency rather than carrying older wall-time/memory assumptions; concurrency remains physical policy, not retention semantics.

**Patterns:** `MNT-06`, `OBS-05`, `TST-09`.

## A.12 `mergeSchema` non-nullable fix

The pin fixes a merge-schema failure involving non-nullable fields.

**Design leverage:** remove legacy workarounds only after the desired schema-evolution semantics are validated; this fix does **not** justify enabling `SchemaMode::Merge` by default.

**Patterns:** `SCH-06`, `TST-01`, `TST-04`.

## A.13 S3 option cleanup

Delta-specific S3 options now focus more tightly on locking/rename semantics while general S3 configuration is normalized through `object_store` keys.

**Design leverage:** centralize canonical object-store option names and avoid application dependencies on removed/internal delta-specific option fields.

**Patterns:** `STO-03`, `STO-04`, `TST-10`.

## A.14 Action-path URI encoding

Delta action paths encode literal spaces as `%20`, preserve `/` and Hive `=`, remain compatible with older unencoded-space paths, and avoid double-encoding already percent-encoded paths.

**Design leverage:** treat Add/Remove/CDF action paths as opaque Delta/object-store URI identities; never round-trip display-decoded strings back into transaction identity.

**Patterns:** `LAY-08`, `TST-13`.

## A.15 `V2Checkpoint` protocol recognition

`TableFeature::V2Checkpoint` is recognized in supported reader and writer feature sets at this pin.

**Design leverage:** generic protocol compatibility no longer fails solely due to `v2Checkpoint`; however, do not imply a complete high-level checkpoint authoring/management surface without separate evidence.

**Patterns:** `SCH-08`, `SCH-09`, `INT-08`, `GOV-10`, `TST-03`.

## A.16 Strict declared-feature validation

The 1.0.0 line validates declared table features as a whole. Unsupported declared reader/writer features fail the operation even if the touched rows/files do not visibly exercise them.

**Design leverage:** this strongly supports the constitution's “unknown is preferable to falsely known” rule. Do not weaken protocol validation based on local inspection of touched data.

**Patterns:** `GOV-05`, `GOV-06`, `SCH-08`, `TST-03`.

## A.17 OpenDAL backend family

The wrapper exposes OpenDAL-backed long-tail storage services with unambiguous `opendal+<service>` schemes and avoids clobbering native backends for colliding schemes.

**Design leverage:** preserve backend variability behind storage contracts; prefer native S3/Azure/GCS backends for their primary production paths and use OpenDAL deliberately where its service coverage is the reason for adoption.

**Patterns:** `STO-07`, `TST-10`.

## A.18 Python interop alignment

The documented Python package line aligns to Arrow 59, DataFusion execution 55, `pyo3-arrow 0.19.0`, and PyO3 0.29.

**Design leverage:** if Rust and Python exchange Arrow objects directly, treat the exact interoperability version set as a tested protocol boundary rather than assuming all Arrow Python/Rust combinations are zero-copy compatible.

**Patterns:** `INT-09`, `INT-10`, `TST-14`.

---

# Appendix B — Delta authority matrix

| Artifact / object | Semantic authority? | Durable? | Version identity | Can change without Delta version change? | Correct architectural use |
|---|---:|---:|---|---:|---|
| Delta transaction-log table state | **Yes — per table** | yes | exact Delta version | no | persisted table state authority |
| application multi-table publication | **Yes — application scope** | application-defined | publication ID/fingerprint | application-defined | coherent cross-table serving/replay authority |
| loaded `DeltaTable` | no | no | exposes loaded version | internal state may refresh | handle/view of authority |
| kernel snapshot | no independently | no/process | Delta version + internal identity | checkpoint/cache details can change | execution/replay view |
| materialized file/stat cache | no | no | tied to snapshot identity | yes | rebuildable optimization |
| checkpoint file | no | yes physical artifact | checkpoint-associated version | yes, can appear later | replay accelerator |
| DataFusion `TableProvider` | no | no | should record pinned version | no if immutable provider; replace on refresh | query view |
| active Parquet files on storage | no independently | yes physical objects | selected by snapshot | storage can contain tombstones/orphans | physical realization only |
| object-store listing | no | transient observation | none | yes | storage diagnostic only |
| Add/Remove action path | part of Delta state | yes in log | transaction version | no for committed action | opaque file identity in protocol |
| table schema/protocol/properties | yes as persisted table contract | yes | Delta version | only via new commit/version | durable operational contract |
| commit metadata | provenance artifact | yes within log retention | commit version | no | lineage/reference, not enforcement |
| `history()` result | derived durable audit view | retention-bound | commit versions | history can expire | table-local audit/provenance |
| CDF | derived native change stream | retention-bound | `_commit_version` | range availability can expire | incremental consumption |
| optimize layout | no semantic-state authority | physical/durable files + new version | resulting Delta version | physical representation changes | maintenance |
| vacuum result | physical cleanup | durable deletion | no new semantic row state implied | yes relative to old file availability | retention/storage lifecycle |

---

# Appendix C — Recommended table-class policy defaults

These are design starting points, not universal defaults; table-specific requirements override them.

| Table class | Write posture | CDF | Optimize | Vacuum | Reproducibility/retention emphasis |
|---|---|---|---|---|---|
| immutable run/event facts | append / create-once | optional | periodic compaction | conservative | strong version pins; long audit history |
| current-state dimensions | merge/update | useful | after heavy DML; possible Z-order on key | moderate | restore and bad-merge recovery matter |
| raw/bronze ingestion | append | often useful | frequent if micro-batch heavy | based on reprocessing/audit policy | source replay may require long retention |
| derived materialization | overwrite/replaceWhere/append by contract | optional | workload-driven | can be more aggressive if fully reproducible | exact source pins allow regeneration |
| ephemeral staging | create/overwrite | usually off | rarely | aggressive after job completion | isolate from durable tables |
| audit/legal archive | append-only | optional/external event log | conservative | very conservative or disabled | retention dominates storage savings |

---

# Appendix D — Compact release gate for this baseline

```text
DEPENDENCY
[ ] deltalake/deltalake-core rev = 43a0cf10a313e5077c48637ad786a05359136bbb
[ ] DataFusion = 55.0.0
[ ] Arrow/Parquet = 59.2.0
[ ] object_store = 0.13.2
[ ] Rust = 1.94.1 / edition 2024
[ ] cargo tree reviewed for incompatible duplicate public type universes

SNAPSHOT/AUTHORITY
[ ] exact Delta version recorded for every query/write input
[ ] provider/cache keys include exact version
[ ] same-version checkpoint does not change semantic publication identity
[ ] multi-table results use publication manifest/version map

SCHEMA/PROTOCOL
[ ] schema fingerprint validated
[ ] protocol/reader/writer features validated
[ ] operation-specific advanced-feature matrix passes
[ ] nested physical optionality + partition-name collision regressions pass

TRANSACTION
[ ] idempotency/retry model documented
[ ] unknown commit reconciliation test passes
[ ] num_retries telemetry captured
[ ] S3/multi-writer commit safety passes backend test

CDF
[ ] DV-aware CDF fixtures pass if applicable
[ ] ICT timestamp/fallback fixtures pass
[ ] consumer checkpoint uses exact version
[ ] retention guard covers all consumers

QUERY
[ ] Delta provider used, not raw Parquet
[ ] DataFusion SessionState/runtime mapping correct
[ ] provider refresh/pin tests pass
[ ] EXPLAIN/pruning regression tests pass

MAINTENANCE
[ ] optimize logical-equivalence tests pass
[ ] vacuum dry-run/retention/keep-version tests pass
[ ] deep-partition full-vacuum benchmark reviewed
[ ] restore/filesystem-check incident tests pass

INTEROP
[ ] supported external engines read/write certified feature set
[ ] path spaces/percent encoding fixture passes
[ ] Python Arrow interop tested if used

PROVENANCE
[ ] before/after versions, spec/config/code/environment IDs recorded
[ ] commit metadata contains no secrets
[ ] provenance closure/replay check passes
```

---

# Closing maxim

> **Use Delta to make table state explicit and versioned; use Arrow/DataFusion to compute over that state; make every mutation a governed transition; and preserve enough contract, provenance, retention, and evidence to explain and reproduce the state forever.**
