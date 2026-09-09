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

> Target revised 2026-09-08 under the consolidated pragmatic delivery review. These are target contracts, not a claim of implemented behavior; see [current status](../../STATUS.md). Historical predecessors are unchanged.

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
typed Rust/DataFusion execution and scoped processing coverage.

V2.3 does not add a FastMCP-aware catalog, provider, table, lease, or result format. FAB continues
to own the sealed internal result package and resource lease; SRV may project a daemon-minted
public handle only after that authoritative state exists and must reauthorize every read/release
through the daemon. Guard continuations and reference completion are QRY/SRV control projections,
not durable fabric relations or alternate epoch authority.

Normative words `MUST`, `MUST NOT`, `SHALL`, `SHOULD`, and `MAY` have their usual requirements
meaning. The selected working suite owns cross-domain meaning as follows:

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
functions, relations, and processing status
are derived from the admitted epoch; no checked-in census, bundle index, digest ledger, or
generated copy is current authority.

## 1. Purpose and invariant architecture

CodeFabric is a present-state Arrow/DataFusion/Delta data fabric:

```text
exact provider Arrow batches + explicit typed inputs
  -> ProgrammaticTransformation values
  -> candidate DataFusion session + derived catalog observations
  -> normalization / authority / derived relations
  -> one immutable DataFusion catalog and SessionState
exact Delta versions + optional immutable Arrow segments + coverage
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
10. Query-relevant processing coverage and compact provenance describe actual work for the snapshot.
11. Typed Rust builders own provider, schema and query construction. No generalized semantic release compiler is required.
12. Public callers provide bounded semantic requests and authorized workspace inputs, never arbitrary physical catalogs or plans.

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
  multi-table visibility, writer fencing, retry policy, authorization, and public meaning.

Raw Parquet listings are never Delta table state. A DataFusion plan or `EXPLAIN` string is never
semantic identity. Arrow field metadata is annotation unless a named consumer and fault prove
that it enforces a contract.

## 3. Relational namespaces and authority

Every epoch contains role-separated schemas in one catalog graph:

| Namespace | Contents |
|---|---|
| `input` | explicit non-derivable identity, compatibility, policy, algorithm, query, and oracle inputs |
| `program` | typed normalization, authority, derivation, query and policy transformations |
| `raw` | exact provider-native observations, coverage, remainders, diagnostics, and run provenance |
| `canonical` | reconciled facts, conflicts, explicit unknowns, and normalized identities |
| `derived` | application-owned graph/dataflow/effect/resource/summary outputs with algorithm and precision |
| `public` | authorized stable semantic views and result projections |
| `system` | derived catalog/runtime/capability/query/update/lease observations |
| `_storage` | internal exact-version Delta and immutable-segment providers; never public |

Relation and field identity comes from admitted schemas and explicit stable identity inputs.
Current catalog contents come from live `information_schema` and runtime observation. Function,
provider and extension inventories describe installed support; workspace capability and coverage come from processing. Ordinary code declarations are allowed.

Canonical identifiers are application-owned 16-byte values with stable public encodings. The
logical Arrow representation is `FixedSizeBinary(16)` plus released typed identity metadata. Provider
local IDs, DataFusion plan node identities, file ordinals, petgraph `NodeIndex`, and Delta file
paths are never canonical identity.

## 4. `FabricEpoch` and sealed catalog ownership

One immutable snapshot owns source/context generation, provider versions and scoped coverage, exact Delta root/version pairs, any immutable segments, schema/authorization configuration, and a sealed DataFusion catalog. One shared workspace RuntimeEnv supplies memory/spill resources across candidates and leased snapshots. A query clones the selected snapshot once and never discovers latest tables during execution.

Typed Rust builders install actual providers, functions and plans. Observe schemas from these objects when useful; compare declared boundary schemas where needed. Do not require a generalized semantic release compiler, fixed-point self-description, five mandatory observation histories or independent proof receipts to construct a snapshot. Published query facades cannot mutate registration state.

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
- key, ordering, partitioning, and constraint metadata justified by the actual provider contract; and
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

Typed Rust builders use DataFusion `Expr` and `LogicalPlan` values for normalization, authority, derivation and queries. Prefer expressions and native relational plans, then functions, then a custom bounded extension only for a concrete capability gap. Record consequential decisions in ordinary code/review notes; do not emit an extension-choice relation for every use. Public requests never supply arbitrary SQL or physical plan text.

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

Advertise uniqueness/nullability only when enforced by the provider or established by the input contract; test those claims at that boundary.
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

Use native Delta checkpoint, compaction and vacuum APIs subject to actual supported capabilities. Preserve logical data/schema and exact snapshots referenced by current state, history policy or live leases. Test maintenance/reopen and retention safety at changed boundaries. Do not reimplement the Delta log engine or compare every table against an independent semantic proof before routine compaction.

### 9.4 Durability classification and exact reconstruction

Persist schema/provider/configuration identities and compact operation/publication outcomes needed to reconstruct selected state. Catalog introspection can be computed on demand. Generalized observation histories are optional diagnostics with bounded retention, not durable semantic authority or a publication prerequisite.

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
chain; forks, missing predecessors, multiple heads, incompatible snapshot metadata, or incompatible compiler
releases fail closed.

An empty chain is lawful. Genesis uses the same command actor and activation
path with `ExpectedHead::Empty`; it is never seeded through a test helper,
direct Delta write, default backend, or separate bootstrap authority.

### 11.3 Ordered activation

The only valid activation order is:

```text
stage -> validate boundaries -> build and seal candidate
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
catalog, coverage, vector, or readiness flag. Recovery selects an exact retained
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

Share one workspace DataFusion budget and spill manager across current/candidate/leased work. Bound application queues, tasks, batches, retained state, query/result bytes, concurrency and deadlines. Own/join work through cancellation and contain external providers. Measure RSS/disk headroom, apply backpressure and recover interrupted operations. This does not promise universal allocation pre-admission or immunity from OOM.

Runtime actions leave compact records containing operation/snapshot/source/context/provider identifiers, requested/completed/remainder scope, exact table versions, timing, terminal outcome and diagnostics. Avoid secrets and unbounded event detail. Detailed traces/intermediates are diagnostic options, not mandatory permanent history. Git records source edits.

Ordinary runtime checks enforce schemas, snapshot identity, publication ownership and scoped coverage. Release tests establish confidence in algorithms; no executable prover must succeed on every epoch before support can be advertised. Distinguish installed support, current processing progress and test confidence.

## 14. Lifecycle, reconstruction, and fresh activation

Source bytes remain authoritative. Conservative invalidation and owner replacement are acceptable; reject stale-generation provider output and publish current coverage/remainder honestly. Immutable snapshots and exact table selection remain mandatory.

Routine reopen reads the selected publication record and exact persisted versions, reconstructs the session and resumes interrupted work. A deliberate clean rebuild reruns providers and analyses for audit/differential tests. It is not a mandatory startup or update step. An empty store uses the same owned publication path for genesis. Recovery reconciles unknown writes and installs one coherent selected snapshot before admission; do not silently revive legacy mutation authority.

## 15. Executable acceptance obligations

Validate relevant Arrow/schema/provider boundaries, exact Delta versions, one publication owner, atomic snapshot pinning, retention/recovery and real query results. Use ordinary focused checks and integrated release tests according to risk. A navigation check, source digest or passing process ledger does not establish semantics. Historical recipe lists are not required proof quotas.
