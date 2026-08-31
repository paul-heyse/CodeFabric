---
artifact: design-dossier
design_id: codefabric-execution-proved-relational-data-fabric
version: v3
date: 2026-08-30
status: accepted
baseline_commit: db67f7cbbd1ce96e7d7a98a790a0a5ef246fbc34
reconciled_head: db67f7cbbd1ce96e7d7a98a790a0a5ef246fbc34
working_tree_digest: caea1e54124eae14cef7828247dac36832cd4959f370d77a66bd79d787b9ac19
primary_scope:
  - docs/authoritative_design/
  - docs/library_ref/full_data_fabric_design_principles_v2.md
  - contracts/
  - src/
  - rustc-extractor/
  - pyrefly-sidecar/
  - codefabric-cpg-mcp/
  - rules/
  - scripts/
  - tooling/ci/
doctrine_path: docs/library_ref/full_data_fabric_design_principles_v2.md
supersedes:
  - docs/designs/codefabric_execution_proved_relational_data_fabric_design_v2_2026-08-29.md
controlling_review:
  - docs/reviews/implementation_status_codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29_2026-08-30_v2.md
---

# CodeFabric execution-proved relational data fabric — target design v3

## 1. Executive decision

CodeFabric keeps the accepted v2 product scope and the implementation outcomes that remain valid,
but replaces one invalid architectural center: bootstrap-and-migration replay is not semantic or
schema authority. The sole target is a programmatically assembled, immutable DataFusion session
whose authority consists of:

1. exact provider-native Arrow `RecordBatch` inputs;
2. explicit typed inputs for meaning that cannot be derived, including released identity/wire
   commitments, policy, compatibility, algorithms, and access/resource choices;
3. typed `ProgrammaticTransformation` values that construct DataFusion expressions and logical
   plans; and
4. relation, field, schema, dependency, and provenance observations derived from what the candidate
   session actually contains.

The output schema of a transformation is obtained from the built logical plan. A declared schema is
a checked assertion, never a source of truth. The candidate describes itself by installing and
observing these relations to a fixed point. There is no `BootstrapMetamodel`, model-migration replay,
`ModelEpoch -> SchemaContract` projection, replayed schema registry, or model digest in the target.

This is an integration of design v2, not a clean-slate scope reduction. Exact providers, relation-
scoped Arrow IPC, application-owned analyses, native-first DataFusion planning, authorized child
catalogs, exact Delta pins, one `FabricCommand` path, fenced activation, semantic requests, UDS
serving, and presentation-only FastMCP remain required. Existing implementations of those outcomes
are reusable when they consume target authority. Compatibility layers and fallbacks that preserve
the rejected authority are not reusable merely because current code depends on them.

### 1.1 Outcome

At completion, one daemon production route builds a `ProgrammaticFabricEpoch` from exact batches,
typed inputs, transformations, and an exact relation-root/version vector; proves it; activates it;
serves every released semantic request through an authorized child session; reconstructs it after
failure without a candidate; and exposes results through the existing released Rust/gRPC/FastMCP
boundary. Delta is durable state for every proof-bearing intermediate required by restart, audit,
incrementality, or provenance. Bounded caches remain re-derivable optimizations. Obsolete bootstrap,
model, generated-schema, dual-epoch, and fallback surfaces are physically absent outside immutable
history.

### 1.2 Non-goals

- No arbitrary SQL, physical table/function name, DataFrame, plan handle, or serialized plan enters
  a public request.
- No Python Arrow/DataFusion/Delta processing layer or adapter-owned mutable semantic state.
- No new Cargo root or package solely for organization.
- No raw Parquet/object-store listing as Delta state, provider-local ID as canonical identity, or
  petgraph index as persistent/public identity.
- No production dual write, bootstrap fallback, legacy query fallback, or indefinite comparator.
- No deletion of retained product behavior, released wire/identity commitments, authority-neutral
  primitives used by the target, or immutable design/plan/review/release history.
- No general dynamic user-authored transformation language. Programmatic transformations are closed
  Rust types and reviewed constructors in the application build.

### 1.3 Current-tree evidence and trust boundary

The controlling independent review invalidated v2 design/plan authority while distinguishing
reusable implementation from completion. Current-tree inspection at the reconciled HEAD found:

- reusable programmatic foundations in `src/fabric/programmatic_schema.rs`,
  `programmatic_epoch.rs`, `datafusion_cache.rs`, `programmatic_observation_delta.rs`,
  `activation_control_delta.rs`, and `activation_transaction.rs`;
- reusable exact-provider/Arrow/analysis/query pieces in `relation_ipc*.rs`,
  `provider_native_syntax.rs`, `pyrefly_service.rs`, `rustc_service.rs`,
  `python_derived_analysis.rs`, `rust_mir_derived_analysis.rs`,
  `common_derived_analysis.rs`, `graph_program.rs`, and target `*_with_bindings` compiler methods;
- a production composition gap: `src/daemon.rs` still constructs a default query backend and calls
  `bootstrap_fabrics`; target query/command/runtime factories and Delta activation authority have
  isolated definitions but no demonstrated production consumer;
- live predecessor authority in `src/relational_model/{schema,replay,release}.rs`, generated
  `schema_registry` bootstrap, model-migration command/effect surfaces, old epoch builders, replay
  compiler wrappers, legacy provider admission, and model-pinned semantic query paths; and
- first-principles evidence is incomplete because the existing evidence transaction requires a
  `bootstrap_model_semantics` category and a mandatory comparator derived from predecessor shape.

The recorded working-tree digest is the SHA-256 of the pre-design-v3 porcelain-v2 status stream; it
identifies an intentionally dirty shared workspace and is not a correctness claim. Current-tree
evidence and packet oracles outrank this dossier's inventory.

## 2. Constraints and target invariants

### I-40 — Programmatic authority only

Exact batches, explicit typed inputs, and typed transformations are the only semantic construction
inputs. Execution reads them. Bootstrap/replay/schema-registry/model-digest authority is impossible
to select.

### I-41 — Plan-derived schema

The built `LogicalPlan` and admitted provider `SchemaRef` determine schemas. Expected schemas,
fingerprints, and compatibility profiles validate those values; none authors them.

### I-42 — Fixed-point self-description

The candidate derives five observation histories for relations, fields, schemas, dependencies, and
provenance until a complete unchanged iteration. Those histories describe themselves and fail on
missing, duplicate, dangling, or inert rows.

### I-43 — One immutable serving epoch

One admitted query retains one `Arc<ProgrammaticFabricEpoch>` through terminal delivery. The epoch
binds exact source/provider/analysis/program/policy/proof/release identities and exact Delta root/
version pins. A request cannot discover `latest` during planning or execution.

### I-44 — One Arrow/DataFusion type universe

Arrow/Parquet 59.2.0, DataFusion 55.0.0, `object_store` 0.13.2, and the selected delta-rs revision
compose one public type universe. Provider process isolation does not permit semantic type skew at
the Arrow IPC boundary.

### I-45 — One mutation path and candidate-free recovery

Every durable change enters one exhaustive, idempotent `FabricCommand` route under one fenced
writer. Recovery closes admission, owns no candidate, reconciles durable operation/activation
evidence, rebuilds the exactly selected epoch, installs it, reconciles receipt/ack optimization,
then reopens.

### I-46 — Durable proof closure

Every intermediate required after process loss to explain, reproduce, increment, or prove an epoch
is an exact-version Delta relation or immutable Arrow segment selected by the epoch. Transient
execution buffers never masquerade as durable truth.

### I-47 — Bounded caches, no cached authority

Metadata, file-statistics, object-list, and logical-plan caches are bounded and fully version-keyed.
Object-list TTL is a refresh bound, not validity. Physical plans and results are not cached.
Activation receipt cache cannot select state.

### I-48 — Exact provider and explicit analysis authority

Provider-native observations remain raw; normalization and derived analyses remain distinct typed
transformations. Every accepted analysis family has one application producer or explicit
unsupported remainder. Missing output is never a negative fact.

### I-49 — Authorized production composition

The daemon's real source/provider/command/activation/query route constructs the target epoch.
Authorized child sessions expose only allowed catalogs, tables, views, functions, variables,
extensions, runtime configuration, and object stores. Prebound views are rebuilt or recursively
proved closed.

### I-50 — First-principles proof

Correctness is decided by independently authored expected rows, negative cases, causal faults,
released wire expectations, relation invariants, and end-to-end execution. Captures, digests,
self-generated goldens, replay agreement, and comparator agreement are identity/diagnostic evidence
only.

### I-51 — Targeted, physical decommission

Every predecessor surface receives a retain/reshape/replace/delete disposition with a reason,
target consumer, cutover, and oracle. Replaced authority is deleted at the earliest dependency-safe
boundary and completion is file/symbol/feature/recipe/package zero state, not unreachability.

## 3. Target architecture

### 3.1 Authority and representation map

| Concept | Authority | Derived/runtime form | Never authority |
|---|---|---|---|
| provider observation | exact provider batch + coverage trailer | registered Arrow relation | opaque JSON, DTO census |
| non-derivable meaning | closed explicit typed input | input relation / constructor argument | generated registry |
| transformation meaning | `ProgrammaticTransformation` | DataFusion `Expr`/`LogicalPlan` | SQL string, plan text |
| relation schema | admitted `SchemaRef` or `LogicalPlan::schema()` | `DFSchema`, storage/output mappings | declared output schema |
| catalog contents | installed candidate session | five observation histories | bootstrap table list |
| table state | canonical Delta root + exact version | delta-rs provider | raw Parquet/listing/cache |
| multi-relation epoch | activation event exact vector | sealed epoch/session | mutable pointer/SQLite row |
| current epoch | unique valid activation-chain head | atomic `Arc` handle | timestamp/latest/cache |
| correctness | independent execution result | proof rows/receipt | digest/comparator/capture |
| released compatibility | immutable wire/identity contract | negotiated projection | internal legacy route |

### 3.2 D-40 — Programmatic assembly contract

`ProgrammaticSchemaAssembly` receives a non-empty set of exact provider batches, explicit inputs,
and transformations. Each transformation contains semantic ID/version, typed input relation IDs,
resource class, determinism/order policy, a plan-building function, and an output-schema assertion.
Construction parses loose inputs once into closed Rust types; exhaustive matching prevents unknown
variants from silently executing.

The assembler validates relation identity, schema, batch lengths, nullability, keys, dependencies,
and source/provider/release pins before registration. It topologically orders transformations,
rejects cycles unless the transformation explicitly selects DataFusion's bounded recursive
semantics, builds the plan, compares the actual plan schema to the assertion, executes under the
candidate session, validates every batch, and emits operation provenance.

### 3.3 D-41 — Candidate catalog and fixed-point observations

The builder begins with a fresh `SessionStateBuilder`, one governed `RuntimeEnv`,
`MemoryCatalogProviderList`, catalogs, and role-specific schemas. It installs only explicit object
stores, providers, functions, analyzer/optimizer rules, query/extension planners, and typed
transformations. Registration handles never escape the builder.

After each installation pass it derives:

- `system.programmatic_relation_observation`;
- `system.programmatic_field_observation`;
- `system.programmatic_schema_observation`;
- `system.programmatic_dependency_observation`; and
- `system.programmatic_provenance_observation`.

The observations are installed and observed recursively until an iteration adds no row. Closure
requires exact coverage of installed relations and dependencies; a hard iteration/row/resource
bound yields `unknown`/rejection, never partial success. These five histories are durable because
restart, audit, proof, and self-description depend on them.

### 3.4 D-42 — Schema lifecycle and DataFusion planning

One application `SchemaContract` ties admitted Arrow schema to qualified `DFSchema`, provider,
logical-plan, physical/storage, IPC, batch, and public-output forms. It owns field identity/order,
nullability, nested types, dictionary/extension policy, fixed-width IDs, casts, qualifiers,
projection/filter/statistics remaps, Delta column mapping/deletion-vector adaptation, and boundary
validation.

Compilation takes the first sufficient rung: Arrow kernel; native DataFusion expression/operator;
transparent expression builder; precise UDF/table function/provider; planner hook; logical
extension; then custom physical plan. A custom node exposes every expression/child, supports
rewrites and child replacement, recomputes properties, propagates `PhysicalPlanningContext`,
handles statistics/reservations/spill/cancellation/state reset, and validates output. Code
organization or speculative performance never justifies a lower rung.

### 3.5 D-43 — Exact provider batches and Arrow IPC

Tree-sitter/Ruff remain stable-root adapters; Pyrefly remains the pinned sidecar; rustc public plus
the narrow private seam remain the dated-nightly extractor. Provider objects and borrowed types do
not cross adapters. Relation-scoped Arrow IPC carries one schema/dictionary scope per stream under
a bounded control envelope; Protobuf carries control only. Every stream terminates with coverage,
remainder, diagnostic, and trust/resource status. Corruption, cancellation, version/schema mismatch,
or missing trailer produces explicit incomplete/unknown state.

### 3.6 D-44 — Derived analyses and semantic query

Python CFG/flow, Rust MIR-derived ownership/flow, common graph/effect/resource, and interprocedural
algorithms are separately versioned application transformations. Native relational operators and
`RecursiveQuery` are used where adequate; petgraph remains a bounded private implementation detail.

All eight public semantic request forms compile from typed request relations inside an authorized
child session. Public views are recompiled there or recursively checked for every bound table,
function, extension, variable, and object-store dependency. Query results preserve exact epoch,
provenance, completeness, unknown, deterministic ordering, and resource semantics. FastMCP remains
presentation-only.

### 3.7 D-45 — Delta table, transaction, and durability contracts

Every durable relation has a typed table contract: canonical URI, logical/Delta schema, features,
partition/layout policy, CDF posture, retention class, serving statistics policy, write mode,
authorization, and exact-version reconstruction. The five observation histories, operation and
activation control, provider coverage/remainders, canonical/derived facts, proof violations,
expectation evaluation, and provenance-closure rows use Delta whenever restart/audit/incremental/
provenance behavior depends on them.

Reads use delta-rs `TableProvider` at one exact loaded snapshot/version; never raw Parquet. Serving
loads full statistics. CDF uses `scan_cdf` over explicit version ranges as transport, persists a
consumer checkpoint, and is retention-guarded; it does not select state. Commit properties carry
compact operation/input/schema/program/release/provenance references without secrets, but exact
Delta versions and the epoch manifest remain authority.

Every exact-version reopen performs both pinned-revision compatibility layers before semantic
use: delta-rs `ProtocolChecker::can_read_from` plus the kernel
`ensure_operation_supported(Operation::Scan|Cdf)` gate. Every create, append, DML, CDF-property,
optimize, checkpoint, vacuum, or feature-changing path performs
`ProtocolChecker::can_write_to` (which first requires read compatibility) plus the applicable
kernel `ensure_write_supported`/operation gate before mutation. Unsupported declared features,
reader/writer versions, or operation-specific features produce an explicit unsupported capability
or rejected command; no raw-log, raw-Parquet, or manual protocol fallback is permitted.

Writes use the highest public builder that preserves semantics, the epoch `SessionState`, explicit
application transaction identity, and application-owned retries with library retries disabled.
Unknown outcomes reload durable markers/history before retry. Optimize/maintenance is physical,
proves logical equality, and uses controlled reconciliation; vacuum requires dry-run plus lease and
retained-version closure.

### 3.8 D-46 — Cache and resource ownership

One epoch runtime owns bounded memory, private quota-limited spill, object stores, and caches:

- DataFusion metadata and file-statistics caches have entry and byte bounds and exact source keys;
- object-list cache has entry/byte bounds plus a finite 30-second refresh TTL;
- a bounded epoch LRU stores only compiled and optimized logical plans, keyed by full transformation
  program and output-schema fingerprints; exact source/provider/table-root/version/relation vector;
  catalog/schema/table/function/extension-type/registry generations; DataFusion, Arrow, Parquet,
  delta-rs, object-store, application, provider, and compiler releases; analyzer/optimizer/planner
  rule-set order; runtime/session configuration; access scope; authorization/policy; and resource
  policy;
- no physical plan or result cache exists; and
- `ActivationReconciliationReceiptCache` stores only re-derivable receipt/ack state.

Admission assigns CPU, memory, spill, rows, bytes, time, output, and concurrency budgets.
Cancellation reaches provider processes, DataFusion streams, graph work, results, and leases.

### 3.9 D-47 — Production composition, activation, and recovery

The daemon constructs one registered workspace runtime factory containing exact source/provider/
analysis inputs, programmatic assembly, Delta activation authority, epoch rebuilder, query runtime,
and command manager. No `WorkspaceQueryBackend::default` plus bootstrap initialization is a valid
production path.

Activation order is: stage exact writes; prove; seal candidate; close admission and drain; recheck
predecessor/fence; append and read back activation event; atomic epoch swap; reconcile receipt cache;
reopen; acknowledge. Queries holding predecessor leases finish; new queries cannot start on it after
durable selection.

Recovery runs before query or mutation admission and with no candidate installed. It reconciles the
operation marker and activation chain, derives the unique selected vector, opens exact Delta state,
rebuilds the sealed session, installs it, reconciles receipt/ack state, then opens admission. Fork,
missing predecessor, incompatible feature/schema/release, missing retained version, or invalid proof
fails closed. Rollback after target mutation is a new forward command/activation, never legacy
revival.

### 3.10 D-48 — First-principles proof

Before any provider, analysis, query, or lifecycle consumer packet may claim acceptance, an
evidence-issuance packet reissues the transaction without `bootstrap_model_semantics`. An author
independent of the implementation authors the expected typed relation/result, source anchor,
governing contract, complete input universe, exact pins, and negative/causal fixture for every
claim family; a separate reviewer accepts or rejects each row. The issued artifacts and fixtures
are immutable consumer inputs. This preserves P30's requirement that expected behavior exist
before the behavior under test.

After the complete production vertical exists, a later evidence-execution packet runs those frozen
expectations and fixtures through provider, normalization, analysis, query, activation/recovery,
authorization, resource, security, compatibility, and clean/incremental routes. It may record
observed evidence and limitations but cannot re-author expected values or weaken negative fixtures.
Any newly discovered claim first returns to independent issuance/review before its consumer can
close.

The predecessor comparator may remain temporarily as a historical diagnostic for preserved overlap;
it cannot gate semantic correctness and is deleted after its explicit retention window. Released
wire interoperability and independently authored fixture behavior, not predecessor output, decide
compatibility.

### 3.11 D-49 — State ownership

| State | Owner/lifetime | Authority | Reconstruction/invalidation |
|---|---|---|---|
| source image | workspace generation | source bytes + descriptor-safe capture | recapture exact generation |
| provider batch | provider run | exact batch + terminal coverage | rerun exact provider/context |
| transformation result | candidate build | typed program over exact inputs | re-execute deterministically |
| proof-bearing history | Delta table/version | transaction log | open exact root/version |
| epoch selection | activation chain | unique valid head | reconcile durable chain |
| installed epoch | daemon `Arc` | selected exact vector | rebuild before admission |
| queue/lease/retry progress | coordinator/SQLite | temporal only | reconcile from durable domain state |
| cache entry | epoch/runtime | never | exact-key check/re-derive/evict |
| public result | result lease | immutable bytes + epoch/provenance | rerun while retained inputs exist |

## 4. Library decisions

### LD-30 — Arrow 59.2.0 is the semantic data boundary

**Decision:** adopt Arrow as the sole in-process and cross-process semantic columnar boundary.

**Version basis:** Arrow/Parquet 59.2.0; exact API authority
`docs/library_ref/arrow_rust_59_datafusion55_advanced_reference_2026-08-23.md`; utilization patterns
ARR-01–ARR-10, SCH-09, SCH-12, INT-01, and INT-08.

**Displaces:** row DTOs, opaque semantic payloads, one-row JSON blobs, and schema-less provider
handoffs; application-owned `SchemaRef`, typed arrays, `RecordBatch`, bounded streams, kernels, and
relation-scoped IPC remain.

**Risk:** schema/dictionary drift or unbounded stream ownership can make typed bytes ambiguous;
every batch validates against the admitted stream/plan schema and one bounded dictionary scope.

**Validation:** `provider-ipc-contract-integrity-check`, `exact-provider-batch-check`, and phase-
specific schema fault fixtures.

### LD-31 — DataFusion 55.0.0 is the catalog/compiler/execution engine

**Decision:** adopt DataFusion as the native catalog, logical compiler, optimizer, physical planner,
and execution engine; custom extensions are permitted only at the first sufficient rung.

**Version basis:** DataFusion 55.0.0 against Arrow/Parquet 59.2.0; exact API authority
`docs/library_ref/datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md`;
patterns CAT-01–CAT-07, LOG-01–LOG-08, PHY-01–PHY-11, RUN-03, RUN-08, and GOV-01–GOV-10.

**Displaces:** hand-written catalogs, procedural relational execution, partial custom nodes, and
opaque planning; typed programmatic transformations remain the application semantic model while
plans and observations are derived runtime forms.

**Risk:** incomplete extension contracts or hidden dependencies can defeat optimization,
authorization, reproducibility, and cancellation; native plans and full dependency traversal are
mandatory.

**Validation:** `datafusion-contract-matrix-integrity-check`, native plan inspection,
`datafusion-plan-schema-cache-check`, and extension rewrite/resource fault tests.

### LD-32 — Schemas are derived and validated across phases

**Decision:** retain an application `SchemaContract` as a validator while provider `SchemaRef` and
`LogicalPlan::schema()` remain native runtime authority.

**Version basis:** Arrow/DataFusion versions in LD-30/LD-31; SCH-01–SCH-12 and the exact DataFusion
schema-lifecycle S1–S14 API chapters.

**Displaces:** model-replayed or declared-schema authority and phase-local schema registries; the
contract retains semantic IDs, compatibility, physical mappings, and assertions only.

**Risk:** qualifier, nested/dictionary/extension, projection, or storage drift can pass one phase
and fail another; every boundary is checked through the same contract.

**Validation:** `datafusion-contract-matrix-integrity-check`, `datafusion-plan-schema-cache-check`,
and adversarial mismatch fixtures for every schema phase.

### LD-33 — Highest viable extension level is mandatory

**Decision:** require built-in expressions/operators before UDFs, providers, planner hooks, logical
extensions, or custom physical nodes.

**Version basis:** DataFusion 55 exact API authority; EXT-01–EXT-10, LOG-08, PHY-01–PHY-11, and
TST-01/TST-03/TST-07.

**Displaces:** bespoke operators and partial physical adapters where a native rung expresses the
semantics; irreducible extensions retain the complete DataFusion node/planner contract.

**Risk:** a low-level extension may hide expressions, children, statistics, resource ownership, or
rewrite semantics; every rung choice is inspectable and causal.

**Validation:** `datafusion-contract-matrix-integrity-check`, `graph-rung-conformance-v3-check`, and
child-rewrite, statistics, cancellation, and zero-selected-test faults.

### LD-34 — delta-rs 1.0.0 at `43a0cf10a313e5077c48637ad786a05359136bbb`

**Decision:** adopt delta-rs for protocol-governed per-table durable state, exact-version providers,
writes, CDF, history, and guarded maintenance; keep cross-table activation and retry policy in the
application.

**Version basis:** `deltalake` 1.0.0 at exact revision
`43a0cf10a313e5077c48637ad786a05359136bbb`; exact API authority
`docs/library_ref/deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md`;
STA-03, STA-07, STA-08, QRY-01–QRY-10, TXN-03–TXN-05, CDF-01–CDF-10, OBS-03, OBS-08,
OBS-10, and TST-03.

**Displaces:** raw Parquet/listing table authority, mutable current manifests, automatic
application retries, and cache-selected snapshots; typed Delta builders and exact providers remain.

**Risk:** unsupported declared table features or protocol versions can make a snapshot unreadable
or a mutation unsafe; every reopen invokes `ProtocolChecker::can_read_from` and the kernel scan/CDF
gate, and every mutation invokes `ProtocolChecker::can_write_to` plus the kernel write/operation
gate before IO.

**Validation:** `delta-durability-protocol-integrity-check`, `delta-exact-reconstruction-v3-check`,
and TST-03 supported/unsupported feature fixtures across reads, writes, CDF, and maintenance.

### LD-35 — Object storage and SQLite remain subordinate

**Decision:** retain `object_store` for physical IO and SQLite for reconstructible temporal control,
never semantic state selection.

**Version basis:** `object_store` 0.13.2 and `rusqlite` 0.40.2 under the exact root graph; local
workstation excludes active AWS implementation and `s3-storage` is the explicit remote capability.

**Displaces:** object listing as table state and SQLite semantic-current/snapshot authority;
queue/lease/command progress and access-scoped store registries remain.

**Risk:** stale listings or temporal rows can masquerade as current state and credentials can leak
across sessions; Delta snapshots remain authoritative and stores/credentials are scope-bound.

**Validation:** `delta-durability-protocol-integrity-check`, cache-mismatch/restart fixtures, exact
feature-graph checks, and credential-isolation tests.

### LD-36 — Existing provider, graph, gRPC, and FastMCP process boundaries remain

**Decision:** retain the existing provider/toolchain, bounded graph-kernel, UDS gRPC, and FastMCP
process boundaries without adding a compatibility facade or native Python extension.

**Version basis:** the exact provider pins and toolchains in GEN v2.1, Tonic 0.14.6/Prost 0.14.4,
grpcio 1.83.0/Protobuf 7.36.0, and FastMCP 3.4.7/Pydantic 2.13.4.

**Displaces:** opaque semantic JSON, borrowed provider escape, in-process/debug serving, adapter
semantic state, and parallel transport authority; owned Arrow relations, control-only Protobuf,
immutable result resources, and presentation remain.

**Risk:** a process boundary can hide schema, resource, cancellation, trust, or compatibility
failure; exact control identities, bounded streams, terminal accounting, and generated-wire
interoperability are required.

**Validation:** `provider-ipc-contract-integrity-check`, `public-lifecycle-wire-contract-integrity-check`,
and real four-domain cancellation/restart/interop faults.

## 5. Alternatives and clean-sheet challenge

### A — Repair bootstrap/model replay

Rejected. It retains a second schema/semantic authority, makes output declarations causally inert,
and recreates the failing `bootstrap_model_semantics` evidence loop.

### B — Programmatic session authority (selected)

Selected because runtime types, plans, dependencies, and observations are causally load-bearing;
provider batches remain exact; explicit meaning remains typed; and current structure is derived
from the session instead of synchronized with it.

### C — Rust types/code alone

Rejected as sole representation. Closed Rust constructors are the safe authoring boundary, but
Arrow relations and DataFusion plans remain necessary for queryable self-description, governance,
provenance, and execution.

### D — Raw SQL or serialized plan programs

Rejected. They expose physical names, weaken type/construction safety, complicate authorization,
and couple identity to engine serialization. SQL rendering and plan text remain diagnostics.

### E — Custom engine/providers for all semantics

Rejected. It hides structure from DataFusion, duplicates planner/runtime duties, and discards
best-in-class native pruning, optimization, resource, and Delta integration.

### F — Preserve both old and new routes for safety

Rejected. A fallback is a second authority and bypasses target proof. Safety comes from frozen-input
comparison before cutover, fenced rollback boundaries, exact durable recovery, and forward repair.

## 6. Transition and decommission design

### 6.1 Transition law

Prior v2 outcomes are presumed in scope. A surface is deleted only when it is a prior elimination
target, embodies the invalid bootstrap/replay/dual-authority decision, or becomes a bypass/duplicate
under the target. Each disposition records a named reason, target consumer, positive cutover oracle,
and negative zero-state oracle. Authority-neutral primitives are extracted or reshaped before their
owning predecessor module is deleted.

### 6.2 Surface disposition

| Surface/outcome | Decision | Reason and target cutover | Proof obligation |
|---|---|---|---|
| `SchemaContract`, Arrow/DF schema maps | reshape/retain | consume admitted/plan-derived schema, not `ModelEpoch` | phase-schema + no model projection |
| relation-scoped IPC and exact providers | retain/complete | target canonical batch boundary | interop, corruption, coverage, exclusive admission |
| application analyses and `*_with_bindings` compiler paths | retain/complete | typed transformation consumers | producer closure + no replay wrapper |
| programmatic schema/epoch/cache modules | retain/complete | target foundation already aligned | production consumer + contract tests |
| Delta observation/activation/recovery modules | retain/complete | exact durable histories and candidate-free recovery | exact reopen + fault matrix |
| command/query/runtime managers | reshape/compose | install registered production factory | daemon vertical + no default bootstrap |
| public gRPC/JSON/identity contracts | retain | released Class 1 commitments | regeneration/interop/compatibility |
| `src/relational_model/{schema,replay,release}.rs` | delete | rejected replay authority | target consumer cutover + file/symbol zero state |
| bootstrap workspace and generated `schema_registry` | delete | second schema/catalog authority | cold start without inputs + no table/files/includes |
| legacy importer/model migration event path | delete | no migration log in target | no command/effect/proto/internal route |
| old `FabricEpochBuilder/FabricEpoch` and dual epoch pins | replace/delete | one programmatic epoch identity | type/consumer zero state |
| replay compiler wrappers and `from_model_epoch` projection | delete | bypass plan-derived schema/typed bindings | call-site and symbol zero state |
| legacy provider admission overload | delete | one exact programmatic admission | compile-time route exclusivity |
| model-pinned semantic query path | delete | request compiles against admitted epoch | end-to-end query + symbol zero state |
| generated semantic registries/bundles/censuses | delete when last consumer cuts over | derived/cache artifacts cannot be authority | hidden-aware inventory + clean build |
| obsolete Cargo features/binaries/dependencies/recipes/rules/tests | delete when owner disappears | non-selectability must be physical | metadata/list/search/skipped-file proof |
| `bootstrap_model_semantics` evidence and mandatory comparator DAG | replace/delete | circular expectation | independent claims + causal faults |
| v2 state, design, plan, review, released/tombstone history | retain historical | immutable Class 1 evidence | excluded from live authority routing |

### 6.3 Cutover and rollback

Target consumers cut over before their predecessor implementation is removed, but both never own
production mutation/serving simultaneously. Before target mutation, rollback may restart the exact
frozen predecessor after target state is discarded. After any target-format mutation, recovery is a
forward `FabricCommand`/activation only. The predecessor binary is mechanically denied binding,
serving, and writing across restart/reboot before target mutation authority advances.

## 7. Proof strategy

Proof is layered:

1. construction/type proof for closed inputs, legal transitions, and exhaustive variants;
2. relational schema/catalog/dependency/provenance/authority/coverage invariants;
3. independent provider, transformation, analysis, query, Delta, activation, security, resource,
   public-wire, clean-rebuild, and incremental-equivalence behavior;
4. causal faults for every acceptance family;
5. structural/textual/compiler/build evidence for cross-language residue and legacy absence; and
6. aggregate four-domain and feature/release gates at one trusted HEAD.

The successor plan assigns exact named recipes. Each packet oracle must fail on zero selected tests
or a committed negative fixture. A check that only proves existence, digest equality, row count,
execution capture, or agreement with predecessor output cannot close a semantic clause.

## 8. P1–P36 disposition

Status means the target treatment, not current implementation conformance.

| Principle | Status | V3 disposition |
|---|---|---|
| P1 Model semantics before implementing behavior | advances | exact typed inputs and `ProgrammaticTransformation` are the semantic model execution reads |
| P2 Make models executable, not merely descriptive | advances | transformations directly build executable `Expr`/`LogicalPlan`; observations are derived |
| P3 One authoritative owner for every concept | advances | authority map in §3.1 removes bootstrap/schema/activation/cache duplicates |
| P4 Explicit conceptual hierarchies | maintains | native catalog/schema/table/function and provider/analysis roles remain explicit |
| P5 Variability behind contracts | maintains | provider/process/storage variation remains behind application ports and native providers |
| P6 Separate semantic meaning from execution strategy | advances | typed transformation intent is distinct from DataFusion physical planning and Delta layout |
| P7 Shared canonical data fabric | maintains | Arrow/DataFusion/Delta remain the one composed fabric |
| P8 Common representation as infrastructure | maintains | `SchemaRef`, `RecordBatch`, `LogicalPlan`, exact Delta versions cross boundaries |
| P9 Provenance intrinsic | advances | each transformation and Delta write emits provenance; five histories include provenance |
| P10 Provenance closure | advances | durable exact-version histories and resolver are activation prerequisites |
| P11 Immutable snapshots and explicit transitions | advances | sealed epochs, exact vectors, activation events, and candidate-free recovery |
| P12 Schemas are executable contracts | advances | native derived schemas are validated across every phase; declarations cannot author them |
| P13 Governance at authoritative boundary | maintains | child catalogs, providers, command actor, and Delta writes enforce policy with denied cases |
| P14 Highest-level extension | advances | LD-33 selects the full native-first ladder per operation |
| P15 Preserve optimizer/validator visibility | advances | typed plans, tree traversal, native operators, and explicit custom-node duties preserve structure |
| P16 Lifecycle phases first-class | advances | assembly, proof, activation, recovery, and failure phases are explicit |
| P17 Reconstruct intermediates by re-execution | advances | deterministic transformations reconstruct; only necessary proof histories persist |
| P18 Fingerprint for identity, never correctness | advances | exact IDs key cache/provenance; independent execution proves semantics |
| P19 Prove reproducibility by re-execution | advances | clean/incremental rebuild and exact-version reopen are required behaviors |
| P20 Advertise only proved capabilities | maintains | capability begins unknown and joins exact coverage/proof before publication |
| P21 Enforced vs advisory metadata | maintains | metadata consumers/enforcers and fault tests determine class |
| P22 Protocol/canonical interoperability | maintains | Arrow IPC and released Protobuf/JSON boundaries remain explicit |
| P23 Local, explicit state ownership | advances | §3.11 and bounded cache design name owner/lifetime/authority/rebuild |
| P24 Semantic observability | advances | exact vectors, plan/provider/pruning/resource/activation observations are structured |
| P25 Every clause names an oracle | advances | plan assigns four discriminating packet oracles and final gates |
| P26 Declare only what cannot change | advances | explicit inputs are limited to Class 1/non-derivable choices; live catalogs are derived |
| P27 Every declaration causally load-bearing | advances | changing a typed input/transformation changes plan/result; inert rows reject closure |
| P28 Compute change, never declare it | advances | dependencies/observations/invalidation/current head are queries over exact states |
| P29 Relational validation over typed model | advances | catalog/proof/authority/coverage invariants are DataFusion relations; residue is labelled weaker |
| P30 Independent expectations | advances | bootstrap comparator category is removed; expected rows and faults are authored independently |
| P31 Eliminate forget-only synchronization | advances | bootstrap registries/censuses disappear; unavoidable wire copies retain regeneration closure |
| P32 Validate by construction | advances | closed input/transformation/command/durability variants and exhaustive matching prevent illegal states |
| P33 Functional core, imperative shell | advances | deterministic batch/plan transformations are pure; IO/commit/process/recovery stay in the shell |
| P34 One mutation path; idempotent/replayable commands | advances | one fenced command actor owns all durable changes and reconciliation |
| P35 Inward acyclic dependency structure | maintains | semantic types remain inward; provider/storage/transport adapters depend on them |
| P36 Governance is executable | advances | causal negative fixtures and named gates classify enforcement honestly |

## 9. Risks and reopen triggers

- Reopen if a required transformation cannot be represented by the selected DataFusion extension
  ladder without hiding semantics or violating resource/cancellation contracts.
- Reopen the durability classification if restart/audit/incremental/provenance proof requires an
  intermediate currently marked transient.
- Reopen Delta deployment if multi-host mutation becomes required; the current design proves only
  one fenced writer per workspace.
- Reopen compatibility if a released client requires a removed internal route rather than the
  released protocol meaning.
- Reopen provider authority if an exact pinned API cannot produce a claimed fact family.
- Replan, rather than patch the artifact, if v2.1 masters, doctrine, exact dependency pins, or any
  accepted design decision changes.

## 10. Acceptance

This design is accepted as the sole target for successor planning. Acceptance means the programmatic
session architecture, v2.1 suite, library decisions, proof model, and targeted decommission
dispositions are fixed inputs to plan v3. It does not certify current implementation, authorize
plan activation, create execution state, or waive any named oracle.
