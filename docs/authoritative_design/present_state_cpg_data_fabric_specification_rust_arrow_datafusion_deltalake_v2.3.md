---
artifact: authoritative-design
artifact_id: codefabric-present-state-cpg-data-fabric
suite_id: codefabric-relational-data-fabric
suite_version: 2.3.0
artifact_tag: FAB
artifact_version: 2.3.0
authority_status: current
predecessor_path: docs/authoritative_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v2.2.md
---

# Present-State CPG Data Fabric Specification v2.3

## 0. Authority, identity, and compatibility

The stable artifact ID is `codefabric-present-state-cpg-data-fabric` (`FAB`). This document is
the current normative owner of CodeFabric's Arrow schemas and schema lifecycle, DataFusion
catalog and execution architecture, durable Delta relations, immutable serving epochs, and
fabric publication semantics.

The v2.2 predecessor is immutable release history. Its product behavior remains required unless
this document explicitly replaces the realization mechanism. In particular, v2.3 preserves
present-state facts, canonical application-owned IDs, raw and normalized evidence, explicit
unknowns, owner-scoped replacement, exact query pinning, durable retention, and bounded query
execution. It replaces static registries, generated schema/catalog bundles, mutable current
pointers, bespoke overlay providers, replay/bootstrap authority, and stored green status with
programmatic session authority and executed proof.

V2.3 does not add a FastMCP-aware catalog, provider, table, lease, or result format. FAB continues
to own the sealed internal result package and resource lease; SRV may project a daemon-minted
public handle only after that authoritative state exists and must reauthorize every read/release
through the daemon. Guard continuations and reference completion are QRY/SRV control projections,
not durable fabric relations or alternate epoch authority.

Normative words `MUST`, `MUST NOT`, `SHALL`, `SHOULD`, and `MAY` have their usual requirements
meaning. The synchronized v2.3 suite owns cross-domain meaning as follows:

| Concern | Owner |
|---|---|
| fact meanings and canonical identity | `ONT` |
| exact provider observations and authority | `GEN` |
| schemas, catalog, planning, storage, epoch, and publication | `FAB` |
| source/update/recovery lifecycle | `LIFE` |
| semantic request and response behavior | `QRY` |
| RPC, FastMCP, public delivery, and result resources | `SRV` |

Released public IDs, semantic result meanings, and historical wire allocations are
immutable contracts. The sole production transport is `codefabric.cpgd.v2`; historical v1
runtime bindings and profiles are not compatibility authorities. Current schemas, capabilities,
functions, relations, and proof status
are derived from the admitted epoch; no checked-in census, bundle index, digest ledger, or
generated copy is current authority.

## 1. Purpose and invariant architecture

CodeFabric is a relationally self-describing present-state data fabric:

```text
exact provider Arrow batches + explicit typed inputs
  -> ProgrammaticTransformation values
  -> candidate DataFusion session + derived catalog observations
  -> normalization / authority / derived relations
  -> one immutable DataFusion catalog and SessionState
exact Delta versions + immutable Arrow segments + proof
  -> sealed FabricEpoch
  -> authorized semantic plans
  -> bounded Arrow results
```

The following invariants are mandatory:

1. One programmatically assembled candidate session is semantic authority; execution reads its
   exact batches, typed inputs, and transformations directly.
2. One admitted query holds one `Arc<FabricEpoch>` through terminal delivery.
3. One Arrow 59.2.0 type universe crosses every semantic data boundary.
4. Providers emit typed native observations; canonical and derived facts are separate relations.
5. One `SchemaContract` owns logical and physical meaning at every phase.
6. DataFusion-native expressions and plans are preferred over functions and custom operators.
7. Every durable mutation enters through one idempotent `FabricCommand` actor.
8. One fenced writer may mutate one workspace; concurrent multi-host writers are unsupported.
9. Delta activation events, not SQLite or an in-memory pointer, determine current epoch.
10. Proof, capability, provenance, and governance are query results over exact epoch inputs.
11. One immutable `CompiledSemanticRelease` with one `SuiteIdentity` privately
    and exhaustively constructs every production provider, field, transformation,
    query form, policy, proof, and child-session recipe.
12. Callers supply operational workspace inputs only; they cannot inject a
    catalog, transformation set, query program, proof closure, release vector,
    or alternative production constructor.

Arrow/DataFusion/Delta are the data plane. SQLite owns reconstructible temporal queues, retry
state, leases, and command progress only. Python never owns Arrow transformations, DataFusion
plans, Delta state, or mutable CPG truth.

## 2. Exact platform baseline

### 2.1 Canonical dependency baseline

The v2.3 fabric baseline is exact:

| Surface | Required identity |
|---|---|
| Rust toolchain floor | 1.95.0, edition 2024 |
| Arrow and Parquet | 59.2.0 |
| DataFusion | 55.0.0 |
| `object_store` | 0.13.2 |
| `deltalake` / `deltalake-core` | 1.0.0 at Git revision `43a0cf10a313e5077c48637ad786a05359136bbb` |
| stable provider-side Arrow roots | 59.2.0 |

The manifests and committed lockfiles are the executable version authority. A resolved second
Arrow/Parquet/DataFusion/object-store universe is incompatible. Local workstation authority does
not include an S3 implementation; the explicit `s3-storage` feature is required for it.

### 2.2 Responsibility split

- Arrow owns columnar values, schemas, record batches, IPC, kernels, and Parquet interop.
- DataFusion owns catalogs, qualified logical schemas, expressions, logical and physical plans,
  optimization, provider scans, execution streams, resource reservations, and metrics.
- delta-rs owns Delta transaction-log interpretation, snapshots, protocol/features, file
  adaptation, table commits, and its DataFusion scan path.
- CodeFabric owns domain identity, schema meaning, authority resolution, exact epoch selection,
  multi-table visibility, writer fencing, retry policy, proof, authorization, and public meaning.

Raw Parquet listings are never Delta table state. A DataFusion plan or `EXPLAIN` string is never
semantic identity. Arrow field metadata is annotation unless a named consumer and fault prove
that it enforces a contract.

## 3. Relational namespaces and authority

Every epoch contains role-separated schemas in one catalog graph:

| Namespace | Contents |
|---|---|
| `input` | explicit non-derivable identity, compatibility, policy, algorithm, query, and oracle inputs |
| `program` | typed normalization, authority, derivation, query, policy, and proof transformations |
| `raw` | exact provider-native observations, coverage, remainders, diagnostics, and run provenance |
| `canonical` | reconciled facts, conflicts, explicit unknowns, and normalized identities |
| `derived` | application-owned graph/dataflow/effect/resource/summary outputs with algorithm and precision |
| `public` | authorized stable semantic views and result projections |
| `proof` | expectations, violations, coverage, causal mutations, provenance closure, and receipts |
| `system` | derived catalog/runtime/capability/query/update/lease observations |
| `_storage` | internal exact-version Delta and immutable-segment providers; never public |

Relation and field identity comes from admitted schemas and explicit stable identity inputs.
Current catalog contents come from live `information_schema` and runtime observation. Function,
provider, capability, dependency,
and extension inventories are derived from installation and compilation; they are not authored
lists.

Canonical identifiers are application-owned 16-byte values with stable public encodings. The
logical Arrow representation is `FixedSizeBinary(16)` plus released typed identity metadata. Provider
local IDs, DataFusion plan node identities, file ordinals, petgraph `NodeIndex`, and Delta file
paths are never canonical identity.

## 4. `FabricEpoch` and sealed catalog ownership

One immutable `FabricEpoch` owns:

```text
fabric_epoch_id
programmatic_assembly_id, transformation_set identity, and exact release vector
source generation and source inventory identity
provider runs, versions, coverage, and capability result
exact Delta table root/version pairs
immutable overlay segment identities
policy and AccessScope inputs
proof receipt and provenance-closure result
sealed SessionState and catalog graph
one governed RuntimeEnv and resource profile
function, analyzer, optimizer, and extension implementations
activation event, writer generation, and retention class
```

The builder starts from a fresh `SessionStateBuilder`, `MemoryCatalogProviderList`,
`MemoryCatalogProvider`, and role-specific `MemorySchemaProvider`s. It explicitly installs the
runtime, object stores, providers, functions, analyzer/optimizer rules, query planner, and
extension planners. Only the builder retains registration handles. Published code receives a
query/inspection facade and cannot register, deregister, or obtain a raw mutable context.

An epoch is rejected unless input/program/catalog/schema/provider/function/policy/proof closure is exact.
An accepted query clones the current `Arc<FabricEpoch>` once. It never discovers `latest`,
refreshes a Delta handle, or consults a mutable global registry while planning or executing.

### 4.1 Programmatic assembly and self-description

`ProgrammaticSchemaAssembly` accepts exact provider batches, explicit typed inputs, and typed
transformations. Each `ProgrammaticTransformation` declares its semantic identity, input relation
IDs, plan builder, deterministic ordering contract, resource class, and expected output schema.
The plan builder constructs the actual DataFusion logical plan; `plan.schema()` is the schema
authority and the expected schema is a failing assertion over it.

The builder repeatedly installs then observes the candidate catalog until the observation set is
unchanged. It emits five typed histories: `system.programmatic_relation_observation`,
`system.programmatic_field_observation`, `system.programmatic_schema_observation`,
`system.programmatic_dependency_observation`, and
`system.programmatic_provenance_observation`. A later iteration may describe these observation
relations themselves. Closure is reached only when every installed relation, field, schema,
dependency, and provenance edge is observed and no new row appears. A bootstrap metamodel,
`ModelEpoch -> SchemaContract` projection, replayed schema registry, or model digest is prohibited.

## 5. Executable `SchemaContract`

### 5.1 Ownership and contents

For every accepted relation, one `SchemaContract` is derived from the admitted Arrow schema,
explicit field/type/key/representation inputs, and an exact physical binding. A transformation's
declared output schema is checked against the plan-derived schema and cannot author it. The contract
owns:

- source schema identity and Arrow `SchemaRef`;
- qualified DataFusion `DFSchema` and qualifier policy;
- logical, provider, storage, and restored output types;
- logical-to-storage and storage-to-logical casts;
- projection, filter, column, and statistics index maps;
- nullability, nested-child, dictionary, decimal, timestamp/timezone, and map/list/struct rules;
- fixed-width and extension-metadata requirements;
- Delta column mapping and deletion-vector adaptation;
- key, ordering, partitioning, and constraint metadata allowed after proof; and
- explicit empty-stream schema behavior.

### 5.2 Phase contract

The contract is checked at each boundary:

```text
provider ingress
  -> analyzed logical plan
  -> optimized logical plan
  -> initial physical plan
  -> optimized physical plan
  -> stream construction
  -> every RecordBatch
  -> write sink / IPC / result artifact
```

The actual batch schema MUST equal the stream schema and the planned output contract. Wrong-width
IDs, missing required metadata, illegal nulls, changed nested children, reordered/unmapped
columns, or provider declaration/batch mismatch fail before publication or delivery.

Delta `BINARY` storage does not redefine a logical fixed-size identifier. Native DataFusion
projections/casts/views restore logical meaning. At most one generic transparent provider
adapter may exist for an irreducible storage seam; it MUST preserve optimizer visibility and
projection/filter/statistics mapping. A domain-specific wrapper may be removed only in the same
change that proves the generic replacement on the real Delta route.

## 6. Provider boundary and Arrow IPC

Provider adapters emit one typed relation at a time. Each row set includes exact provider
version, source/context pins, provider-local identity, coordinates, run identity, and provenance.
Coverage and unknown remainder are relations, not an omitted batch. Application-built CFG,
dataflow, alias, ownership, effect, resource, and summary facts use `derived`, never `raw`.

The control plane multiplexes relation-scoped IPC streams:

```text
open(relation_id, stream_id, schema_fingerprint, source/context pins)
  -> one Arrow IPC schema and stream-local dictionary scope
  -> ordered IPC messages under bounded flow-control acknowledgements
  -> ipc_end
  -> coverage/remainder/diagnostic trailer
  -> terminal(stream_id, status)
```

Control frames may interleave stream IDs. Heterogeneous schemas MUST NOT share one Arrow IPC
stream, and semantic rows MUST NOT be encoded in Protobuf control messages. Duplicate or
out-of-order frames, schema mismatch, dictionary corruption, truncation, cancellation, missing
trailer, or terminal-before-EOS produces a typed partial/unknown result. A corrupt or
incompatible stream never yields an empty relation interpreted as absence.

Released Protobuf control continues to own handshake, authentication, accepted handles,
deadlines, progress, flow control, cancellation, errors, and terminal status.

## 7. DataFusion compilation and extension policy

Generic compilers consume explicit input/request/proof relations and typed
`ProgrammaticTransformation` values, then construct DataFusion `Expr` and `LogicalPlan` values.
They cover catalog assembly, normalization/authority/unknown selection, derivation, semantic
queries, policy, and proof. Typed inputs name semantic IDs and bindings; they do not contain SQL,
physical table names, Rust display strings, or general bytecode.

The compiler uses this per-operation ladder:

1. built-in Arrow/DataFusion expression or kernel;
2. native projection/filter/join/semi-join/anti-join/union/window/aggregate/sort/limit or bounded
   `RecursiveQuery`;
3. scalar, aggregate, or window UDF with exact volatility/return semantics;
4. planning-time table function/provider when scalar arguments fully name its inputs;
5. higher-order UDF only for actual collection/lambda semantics;
6. typed logical extension with relational children for a proved irreducible operation.

Each compiled use emits an observed extension-selection row naming the chosen rung and rejected
higher rungs. No graph family receives a blanket custom-extension designation.

A surviving `UserDefinedLogicalNodeCore` exposes all expressions and children, supports
expression/input rewrite, has stable equality/hash and output schema, and has an
`ExtensionPlanner` installed in every relevant session. Its `ExecutionPlan` MUST:

- forward the supplied `PhysicalPlanningContext`;
- visit and replace every owned root physical expression;
- implement child replacement, required `with_new_children`, and property recomputation;
- reset mutable execution state for repeated or recursive execution;
- declare child statistics requests and compute honest exact/inexact/unknown statistics;
- preserve partitioning, ordering, equivalence, and physical invariants after optimizer rewrites;
- validate partitions, reserve/account memory, honor cancellation, and bound input/output; and
- emit deterministic batches satisfying its `SchemaContract`.

Petgraph is private to one bounded execution call. Canonical external IDs enter and leave the
operator; `NodeIndex` and graph storage do not persist or cross a public boundary.

## 8. Honest providers, constraints, and statistics

Every non-native provider has one structured `plan_scan(ScanArgs)` path. It accepts projection,
filters, limit, and any caller-supplied statistics requests without lossy down-conversion. It
reports filter pushdown as exact, inexact, or unsupported and leaves residual filters to
DataFusion as required. It reports partitioning, ordering, functional dependencies,
constraints, and ordinary plan/provider statistics only after an independent oracle proves the
claim for the exact epoch.

DataFusion 55 `StatisticsRequest` is transport vocabulary; DataFusion itself neither produces
nor consumes a query-aware feature. V2.3 does not fabricate one. The initial fabric uses honest
ordinary `Statistics`/precision and native pruning while forwarding supplied requests. A future
query-aware feature requires, together, a typed program-selected producer, provider response mapping,
optimizer consumer, precision rules, request-sensitive cache identity, and an observable plan
oracle.

Uniqueness/nullability may become optimizer metadata only after independent relational proof.
Foreign keys, checks, authority, and access policy remain executable invariants; DataFusion
metadata is not enforcement.

## 9. Durable Delta relations

### 9.1 Writes and retry ownership

All durable writes execute under `FabricCommand` through the pinned write builders with the exact
epoch `SessionState`, `SessionFallbackPolicy::RequireSessionState`, operation ID, writer
generation, application transaction marker, schema contract, and
`CommitProperties::with_max_retries(0)`. Missing or incompatible session state fails closed.

CodeFabric owns retries and unknown-outcome reconciliation. A conflict returns to the command
actor, which reads the durable application marker and committed version before deciding whether
another attempt is legal. The pinned retrying `OptimizeBuilder`, retrying DML helpers, and hidden
automatic rebase are forbidden on the command-owned path. Compaction uses controlled zero-retry
write primitives.

### 9.2 Exact reads and the single-selector rule

An epoch registers a Delta provider through exactly one recipe:

1. a previously loaded snapshot whose table root and version are compared to the epoch pin,
   supplied with the query session and no version selector; or
2. a log store plus exact `with_table_version` and query session, with no supplied snapshot.

Supplying both snapshot and table version is forbidden: at the pinned revision a supplied
snapshot is used directly and `table_version` is consulted only when no snapshot exists.
`DeltaTable::table_provider()` may already carry the loaded snapshot, so appending a contradictory
version selector is not proof of pinning. The observed provider root/version is recorded and
checked before registration.

The kernel-backed delta-rs provider and `DeltaScanExec` own transaction-log/file adaptation.
CodeFabric's `SchemaContract` owns restoration of application logical meaning. Canonical tables
are never opened through raw Parquet listings.

### 9.3 Physical layout and maintenance

Partitioning, clustering, Z-ordering, target file size, compaction, checkpoints, and vacuum are
physical policies derived from measured workload and retention facts. They never change logical
identity. Optimize/compaction must prove relation equality before successor activation. Vacuum
starts with a dry run and protects every version, segment, application/provider release, expectation,
rollback, query, and result lease.

### 9.4 Durability classification and exact reconstruction

Delta is the durable authority for every proof-bearing relation needed after process loss. The
five programmatic observation histories, provider coverage/remainder, canonical/derived state,
proof violations and provenance closure, operation markers, epoch manifests, and activation events
are Delta relations whenever restart, audit, incremental invalidation, or provenance resolution
depends on them. Each table enables CDF where incremental transport is required, records exact
schema/protocol and commit metadata, loads full statistics for serving, and is reopened at one
explicit table root/version.

Intermediate `RecordBatch` values used only within a deterministic transformation, optimizer
scratch, physical plans, streams, and result buffers remain transient Arrow state. They are not
silently promoted to durable truth. A typed `DurabilityClass` attached to the producing operation
selects `DELTA_HISTORY`, `IMMUTABLE_ARROW_SEGMENT`, or `TRANSIENT_ARROW`; an unclassified
proof-bearing output rejects epoch construction.

CDF transports changes between exact versions; it never selects state. Consumers persist an exact
version checkpoint outside the source table and enforce retention closure. Commit properties
record operation, input vector, schema/program/release IDs, and provenance references, but are
physical evidence rather than semantic correctness. Exact table versions and the epoch manifest
remain the authority.

## 10. Effective state and immutable overlays

Interactive freshness may use immutable Arrow segments staged through `object_store`. A segment
is authoritative only after durable bytes, schema, source/provider provenance, and checksum are
validated and pinned by an epoch. Unpersisted process memory is never serving authority.

Effective owner-scoped state is a typed programmatic native plan:

```text
overlay replacement rows
UNION ALL
base exact-version rows
  ANTI JOIN replaced owner/relation keys
  ANTI JOIN owner/relation tombstones
```

Conflict selection and latest-within-the-epoch behavior use visible native windows/joins. Base
and segments remain separately registered in `_storage`; public views expose only the canonical
effective relation. Bespoke concatenate/take consolidation, hidden row conversion, and custom
overlay semantics are prohibited.

Consolidation writes a new exact base, constructs a segment-free candidate epoch, proves logical
row/provenance/unknown/public-query equality, and only then activates it. No query observes an
intermediate rebase.

## 11. `FabricCommand`, fencing, publication, and activation

### 11.1 One mutation path

Every source wave, provider publication, programmatic schema/transformation change, owner replacement/deletion,
compaction, rollback, activation, and retention action is an exhaustive `FabricCommand` with:

```text
operation_id, workspace_id, authorization, expected predecessor,
writer generation, application/transformation/source/provider pins,
resource envelope, typed command payload, and transaction contract
```

One workspace actor authorizes and serializes commands. Duplicate operation IDs with identical
meaning return the prior terminal result; mismatched duplicates conflict. Production,
administrative, importer, maintenance, and test routes have no second durable writer.

An OS-backed workspace lease and monotonically increasing durable writer generation are acquired
before any target write and checked at every durable boundary. A duplicate daemon or stale
generation fails before a domain write. SQLite may record temporal progress but cannot select
semantic current.

### 11.2 Multi-table visibility

Delta provides atomicity per table, not a cross-table transaction. CodeFabric commits and
validates all component versions first, then appends immutable `fabric_epoch` and
`activation_event` control rows naming the complete exact set. Orphaned component versions are
unreachable candidates. Current is the unique valid head of the predecessor-linked activation
chain; forks, missing predecessors, multiple heads, invalid proof, or incompatible compiler
releases fail closed.

An empty chain is lawful. Genesis uses the same command actor and activation
path with `ExpectedHead::Empty`; it is never seeded through a test helper,
direct Delta write, default backend, or separate bootstrap authority.

### 11.3 Ordered activation

The only valid activation order is:

```text
stage -> prove -> build and seal candidate
      -> close new admissions and establish barrier
      -> revalidate predecessor and writer fence
      -> append and read back activation event
      -> atomically install/swap Arc<ActiveWorkspace>
      -> reconcile temporal cache
      -> reopen admission
      -> acknowledge
```

Existing query/result leases remain on the predecessor. No new query may observe it after durable
selection. A crash before selection leaves the predecessor current. A crash after selection but
before swap terminates serving; restart reconstructs and installs the selected epoch before
opening admission. Recovery reads application markers and the activation chain; it never guesses
or selects by timestamp.

The installed value is the complete phase-typed `ActiveWorkspace`: exact
`FabricEpoch`, `SelectedEpochRecord`, query authority, authorized child-session
factory, admission runtime, resource coordinator, activation authority, and
command/source lifecycle handles. There is no independently swappable epoch,
catalog, proof, vector, or readiness flag. Recovery selects an exact retained
target epoch or issues a corrective forward epoch through `FabricCommand`; it
never revives a legacy writer.

## 12. Authorization and bound-plan closure

Each public request receives a reduced child catalog/session for one `AccessScopeId`. It begins
with a fresh catalog graph and explicitly installs only allowed schemas, tables, functions,
extensions, variables, metadata, runtime options, and planners. It uses a fresh allowlisted
object-store registry; shared memory/spill resources do not imply a shared store registry.
Blind `SessionStateBuilder::new_from_existing` is prohibited.

A `ViewTable` contains a pre-bound logical plan. Therefore catalog-name filtering alone is not
authorization. Public views are compiled from typed programmatic expressions inside the child session.
A precompiled view is accepted only after recursive verification of every bound table-provider
Arc, nested/subquery view, scalar/aggregate/window/table function implementation, extension node,
variable, and object-store URL. Unknown nodes fail before physical planning.

Row/column/table/function/operation policy is compiled before execution. Unauthorized objects
are absent from resolution, cost/statistics, errors, and public information schema. Explicit
public metadata views are preferred. Redaction happens before projection, artifact creation,
logging, or diagnostics.

## 13. Resources, observability, and proof

One epoch `RuntimeEnv` owns a bounded memory pool, private quota-limited spill, batch size,
target partitions, object-store registrations, and caches. Query/update admission assigns CPU,
memory, spill, row, byte, time, and concurrency budgets. Cancellation reaches DataFusion streams,
provider processes, graph work, artifact creation, and leases. Slow consumers produce
backpressure rather than unbounded buffering.

### 13.1 Bounded caches are never authority

DataFusion metadata, file-statistics, and object-list caches have explicit entry/byte bounds;
object-list entries additionally use a finite 30-second refresh TTL. TTL controls refresh cost,
not semantic validity: every entry is keyed by exact object/table identity and version and is
discarded on mismatch. The epoch owns a bounded LRU only for compiled and optimized logical plans,
keyed by the full program identity, epoch and relation-root/version vector, runtime/session
configuration, access scope, authorization/policy identity, and resource policy. Physical plans and
query results are never cached. `ActivationReconciliationReceiptCache` stores only a reconstructible
receipt/ack optimization and cannot select an epoch or substitute for durable activation evidence.

Metrics and traces identify epoch, source generation, query/command, stage, provider, table
versions, resource reservations, spill, cancellation, and terminal state without exposing
credentials, source bytes, internal SQL, or unredacted paths. Metrics diagnose execution; they do
not establish semantic correctness.

Proof relations contain exact inputs, independently authored expectations, violations, coverage,
provenance closure, causal mutants, resource outcomes, and `pass`/`fail`/`unknown`. Missing input
or incomplete coverage is `unknown`, never pass. Capability begins unknown and is advertised only
when its executable prover succeeds for the exact epoch. A stored receipt is useful only with the
rows and identities from which it is re-executable.

## 14. Lifecycle, reconstruction, and fresh activation

Source bytes remain authoritative; watcher and gix observations are hints/accelerators. Each
accepted source generation produces immutable source images, provider runs, derived facts,
candidate Delta versions/segments, proof, and an activation event through `FabricCommand`.
Superseded or stale-generation results cannot commit.

Cold reconstruction reads the exact explicit inputs, reruns exact providers and analyses,
reconstructs typed transformations, opens the exact relation-root/version vector, rederives the
activation head, builds one sealed session, and compares logical facts/public results with
incremental state. Recovery closes admission and holds no candidate while reconciling durable
operation/activation evidence; it installs the selected rebuilt epoch before reconciling the
receipt/ack cache and reopening. Generated predecessor bundles, bootstrap/replay paths, and static
registries are physically absent from this proof.

The selected deployment profile is target-only `FreshActivation`. One
`PreEpochWorkspace` owns command recovery and lawful genesis. A candidate is
normalized, proved, durably published, selected, read back at one exact control
horizon, and converted into one `ActiveWorkspace` before admission. Incoherent
roots, versions, activation events, writer fences, or control horizons fail
closed. Repair is forward-only; runtime fallback, predecessor comparison,
rollback-to-predecessor, and dual writes are prohibited. A real deployed
predecessor, if ever discovered, requires a separate accepted handoff design.

## 15. Executable acceptance obligations

The following named commands are normative proof obligations:

| Contract | Required executable oracle |
|---|---|
| one Arrow universe and relation-scoped IPC | `just relational-arrow-boundary-check`; `just arrow-universe-check` |
| logical/physical schema meaning | `just relational-schema-lifecycle-check`; `just schema-phase-boundary-check` |
| immutable epoch catalog | `just fabric-epoch-construction-check`; `just relational-catalog-closure-check` |
| provider scan and honest metadata | `just table-provider-contract-check`; `just provider-statistics-contract-check` |
| native-first compilation | `just semantic-plan-conformance-check`; `just plan-visibility-check` |
| graph rung and physical duties | `just graph-extension-conformance-check`; `just graph-execution-contract-check` |
| authorized bound plans | `just access-catalog-isolation-check`; `just authorized-view-bound-authority-check` |
| Delta exact version and schema | `just delta-exact-version-reconstruction-check`; `just delta-provider-contract-check` |
| one mutation path and writer | `just fabric-single-mutation-path-check`; `just single-writer-fence-check` |
| activation ordering and recovery | `just fabric-activation-recovery-check`; `just activation-fault-matrix-check` |
| epoch/resource reconstruction | `just durable-epoch-reconstruction-check`; `just resource-governance-check` |
| proof/provenance/capability | `just fabric-epoch-proof-closure-check`; `just provenance-closure-check` |
| public end-to-end semantics | `just semantic-delivery-vertical-check`; `just independent-semantic-oracle-check` |

A v2.3 release is nonconforming if any required oracle is absent, skipped, self-authored by the
production path it tests, or nonzero at the proving revision. Digest, file presence, plan text,
row count, or execution capture alone is not acceptance.
