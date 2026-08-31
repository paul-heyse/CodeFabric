---
artifact: implementation-plan
plan_id: codefabric-execution-proved-relational-data-fabric
version: v3
date: 2026-08-30
status: approved
design_path: docs/designs/codefabric_execution_proved_relational_data_fabric_design_v3_2026-08-30.md
design_version: v3
baseline_commit: db67f7cbbd1ce96e7d7a98a790a0a5ef246fbc34
working_tree_digest: caea1e54124eae14cef7828247dac36832cd4959f370d77a66bd79d787b9ac19
state_path: docs/plans/state/codefabric-execution-proved-relational-data-fabric_v3_state.json
cutover: true
supersedes_on_activation: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md
---

# CodeFabric execution-proved relational data fabric -- implementation plan v3

This approved successor realizes accepted design v3 and the sole-current authoritative v2.1
suite. It preserves the v2 product scope and reusable implementation outcomes, but replaces the
invalid bootstrap/model-replay center with exact provider batches, explicit typed inputs, typed
programmatic transformations, plan-derived schemas, self-observed catalogs, and exact Delta
history. The plan is deletion-first at each completed consumer cutover; it is not a clean-slate
license to remove unrelated retained behavior.

This artifact is an immutable execution specification. It deliberately does not create
`docs/plans/state/codefabric-execution-proved-relational-data-fabric_v3_state.json` and does not
change `docs/plans/active-plan.json`. Approval fixes the candidate content; activation remains a
separate governed transaction that validates declared inputs, creates schema-valid state, and
atomically changes the pointer. Plan v2 remains invalidated execution history and must not be
resumed while this candidate is inactive.

## 1. Outcome and non-goals

### 1.1 Outcome

At completion:

1. One production daemon factory builds a `ProgrammaticFabricEpoch` from exact provider-native
   Arrow batches, explicit typed inputs, and `ProgrammaticTransformation` values. No bootstrap
   table list, `ModelMigration` replay, model digest, generated schema registry, or old epoch
   builder can author or select runtime meaning.
2. The admitted provider `SchemaRef` and built DataFusion `LogicalPlan` derive schemas. One
   application `SchemaContract` validates Arrow, qualified logical, physical/storage, IPC,
   streaming, and public-output phases; declared schemas are assertions only.
3. One candidate session derives relation, field, schema, dependency, and provenance observations
   to bounded fixed-point closure. The five self-describing histories are exact-version durable
   Delta relations.
4. Every proof-bearing intermediate needed for restart, audit, incrementality, or provenance is
   classified as Delta history or an immutable Arrow segment selected by the epoch. Execution-only
   buffers stay transient. Delta CDF transports changes, statistics support pruning, and commit
   metadata supplies physical evidence; none is semantic completeness by itself.
5. DataFusion metadata, file-statistics, object-list, and compiled/optimized logical-plan caches
   are bounded, fully authority-keyed optimizations owned by the runtime or epoch. Physical plans,
   results, and semantic-current selection are never cached.
6. One exhaustive `FabricCommand` path, one fenced writer, one exact activation vector, and
   candidate-free recovery converge after success, failure, retry, or unknown outcome. Admission
   reopens only after a freshly reconstructed epoch is installed.
7. Exact providers and application-owned derived producers feed all eight bounded semantic request
   forms through authorized child sessions. The Rust daemon streams immutable Arrow results over
   released UDS gRPC; FastMCP remains presentation-only.
8. Independently authored rows, negative cases, causal faults, released-wire expectations, and
   end-to-end behavior accept correctness. A predecessor comparator is optional diagnostic history,
   never a prerequisite or oracle.
9. Every prior v2 outcome and L-20--L-55 surface has a named successor disposition. Replaced
   bootstrap/model/dual-epoch and attached legacy routes reach file/symbol/feature/recipe/package
   zero state immediately after their last target consumer cuts over; retained wire, identity,
   toolchain, and historical commitments remain explicit.

### 1.2 Non-goals

- No arbitrary SQL, physical table/function name, DataFrame, plan handle, or serialized plan in a
  public request.
- No Python Arrow/DataFusion/Delta processing layer or adapter-owned mutable CPG state.
- No new Cargo root, package, or process boundary for code organization alone.
- No raw Parquet or object listing as Delta state; no provider-local or petgraph index as canonical
  identity.
- No production dual write, bootstrap/model/query fallback, compatibility shim for rejected
  authority, or indefinite comparator archive.
- No persistence of every optimizer intermediate and no unbounded cache.
- No deletion merely because a surface is unrelated to the architectural correction. Each delete
  requires the explicit disposition, replacement, cutover, and proof in §§6--7.
- No state creation, active-plan mutation, packet execution, or production-code edit in the plan-
  authoring turn.

### 1.3 Baseline and current trust posture

The baseline is reconciled HEAD `db67f7cbbd1ce96e7d7a98a790a0a5ef246fbc34`. The frontmatter
working-tree digest is the SHA-256 of the pre-design-v3 porcelain-v2 status stream. The workspace
contains substantial pre-existing staged, modified, and untracked implementation work; the digest
identifies that inventory and proves no semantic claim. Every successor packet must rediscover its
impact before edits and preserve unrelated user work.

The controlling status review found no v2 proving commit, no complete packet, and no complete
decommission batch. It classified WP04--WP05, WP07--WP19, and WP23--WP26 as reusable in progress;
WP01--WP03, WP06, WP20--WP22, and WP27 were invalidated. Focused current-tree checks support
continued development only. This plan does not inherit completion from code existence, green state
integrity, an executor claim, or predecessor oracle output.

### 1.4 Execution law

- A packet may start only when every named dependency is complete at an ancestral proving commit,
  declared inputs are fresh, and current-tree/impact preflight is reconciled. Oracle definitions
  may be implemented by that packet; inactive-plan readiness therefore never requires future
  oracle code to exist.
- A packet may complete only when exactly four unique substantive executable oracles resolve and
  pass at both its proving commit and the candidate HEAD, ordered artifact/integrity contract
  (`INT`), positive behavior (`BEH`), absence/rejection/failure behavior (`NEG`), and operational/
  recovery/performance behavior (`OPS`). `INT` corrupts or omits a governed artifact/dependency;
  `BEH` perturbs an authoritative positive input/output; `NEG` introduces a forbidden route or bad
  input that must be rejected; `OPS` injects restart, cancellation, resource, performance, or
  operational-state failure. Every selector fails on zero selected tests.
- Cut target consumers over first, then delete the predecessor surface in the same packet or its
  attached dependency-closed decommission batch. Unreachable, feature-disabled, deprecated, or
  ignored legacy is not removed.
- A retained primitive is authority-neutral only when a named target consumer uses it and no
  predecessor owner remains selectable. Extract or reshape it before deleting its former module.
- Native DataFusion/Arrow/delta-rs behavior is the default. Custom code requires the selected
  extension-rung justification, complete native contract, resource/cancellation proof, and a
  reopen trigger if the pinned library cannot express the need.
- Schema-v2 state stores only permitted judgment/provenance fields: status, proving commit,
  deviations, failed approaches, and blockers. Exact commands/outputs, selected-test counts,
  changed-path/impact inventories, environment/resource observations, and evidence payloads belong
  in packet-owned versioned artifacts and the proving commit, never embedded in state. A replan
  trigger stops dependent work; it is not patched around.

## 2. Source design and declared inputs

These planning-time digests are immutable. Any change to the accepted design, v2.1 master suite,
doctrine, controlling status review, or selected library alignment manuals makes the candidate
stale and requires explicit input evolution or a revised plan before execution.

| path | sha256 |
|---|---|
| docs/designs/codefabric_execution_proved_relational_data_fabric_design_v3_2026-08-30.md | fc70bb9b356367595fae504dc605f513f8234500fd86eaf46945016c241e4945 |
| docs/reviews/implementation_status_codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29_2026-08-30_v2.md | f3cf8d0c2f1a9fe13741249a1b9298bb6a721508b3b18b51f0f6bf318d0a9db3 |
| docs/library_ref/full_data_fabric_design_principles_v2.md | eb4db97fc9d4522832035002b0a3371e87786971c131a2920ce73af2ef350bd5 |
| docs/authoritative_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.1.md | 97c61518f88f820390ff073582ca9224cac4413cc623c4e6fa4fe4c44a69bda9 |
| docs/authoritative_design/code_property_graph_present_state_fact_ontology_specification_v2.1.md | f16ae052d105dab7ea3e09fcbd33e141e375182e5d4f70da2f22848967643613 |
| docs/authoritative_design/present_state_cpg_fact_generation_specification_python_rust_v2.1.md | 2c990e80ebfdb326e492c2039f1302bdb7e09becf00596866acc53255f1272cb |
| docs/authoritative_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v2.1.md | 4e9878cf311e1574c064f0fb848f7d74ade98b972c5ffa89a4aebabc0f829869 |
| docs/authoritative_design/code_property_graph_semantic_query_specification_v2.1.md | 9a8188f4b4ac1451e65581086c31e75449d5f338949c966e607ec27fb7de822a |
| docs/authoritative_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v2.1.md | cd19380c0589d8f7c5398d28763403d64cd5ac2c72191fded06eb0241ea24869 |
| docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.1.md | 8a3f5bb6112ce8edecc334b46575faba77d34925cc9891770a94ae5fcd58bcf5 |
| docs/authoritative_design/codefabric_2.1_implementation_roadmap_v1.0.md | ff703907513f335eadb6a65df48c3ed92dc7168ecc17c52f1a9a4b2297161cd5 |
| docs/library_ref/datafusion55_arrow59_design_principle_alignment_manual_2026-08-24.md | cfc97d6ea3d963ddf642389434d6762fd70506bb6acb9ed9f12aa13c5fd75726 |
| docs/library_ref/deltalake_1.0.0_43a0cf10_design_principle_alignment_manual_2026-08-26.md | 794a4ecbb38cd90d7ca4506a33c5e8c4b32e209d9a2f9b9429290f96c9af9fc1 |
| docs/library_ref/datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md | 565908b1294aa86772d46cc052a517edd6f5f1115096bf04247143ec09f42a6f |
| docs/library_ref/arrow_rust_59_datafusion55_advanced_reference_2026-08-23.md | 62a9c3f06edebf1807d64802fe82e42dafd76377965dbda61fafd774cdbf5c73 |
| docs/library_ref/deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md | 9ac0717f5f5b401febaed658cca52ca8ce26d336bde54c8e74413d5ff7b01c0c |

### 2.1 Live dependency pins and library contract

The execution baseline must resolve one Arrow/Parquet 59.2.0, DataFusion 55.0.0,
`object_store` 0.13.2, and delta-rs revision
`43a0cf10a313e5077c48637ad786a05359136bbb` universe under Rust 1.95.0. Cargo metadata and
`just stable-graph-check` rederive these values at every dependency-sensitive packet; prose or a
skill reference is not pin authority.

The selected patterns are:

- plan/provider-derived `SchemaRef` and `LogicalPlan::schema()`, with application schema assertions;
- `SessionStateBuilder`, `MemoryCatalogProviderList`, `RuntimeEnvBuilder`,
  `DefaultTableSource`, `TableProvider`, `MemTable`, `ListingTable`, and native logical-plan
  builders before extensions;
- complete expression/tree traversal and full `UserDefinedLogicalNodeCore` / `ExtensionPlanner` /
  `ExecutionPlan` contracts only at the lowest necessary extension rung;
- bounded `DefaultFileStatisticsCache`, `DefaultListFilesCache`, metadata, and epoch LRU caches;
- `DeltaTableBuilder::from_uri(...).with_version(exact)`, `DeltaTableProvider`, writer properties,
  full statistics, one application retry policy of zero, transaction/commit metadata, CDF as
  transport, and guarded retention/vacuum; and
- no raw Parquet scan of a Delta root, `latest` resolution after admission, schema declaration as
  authority, physical/result cache, or cache-selected current state.

For DataFusion RUN-03/RUN-08, every cached logical-plan entry fingerprints the full dependency
closure: transformation/program and output schema; exact source/provider/table-root/version/
relation vector; catalog/schema/table/function/extension-type/registry generations; engine,
application, provider, and compiler releases; analyzer/optimizer/planner rule order; runtime/session
configuration; access/authorization/policy; and resource policy. Any dependency change misses or
invalidates the entry; collision fixtures must produce rejection, never a cross-authority hit.

For Delta STA-03/STA-07/STA-08, TXN-03--TXN-05, OBS-03/OBS-08/OBS-10, and TST-03, every exact
reopen calls pinned-revision `ProtocolChecker::can_read_from` and the kernel
`ensure_operation_supported(Operation::Scan|Cdf)` gate before semantic reads. Every create, append,
DML, CDF property change, optimize, checkpoint, vacuum, or feature change calls
`ProtocolChecker::can_write_to` plus the applicable kernel `ensure_write_supported`/operation gate
before IO. Unsupported declared features or protocol versions reject explicitly; no raw-log,
raw-Parquet, or manual-protocol fallback exists.

### 2.2 Current-tree known impact

Reusable target foundations currently include `src/fabric/programmatic_schema.rs`,
`programmatic_epoch.rs`, `datafusion_cache.rs`, `programmatic_observation_delta.rs`,
`activation_control_delta.rs`, `activation_transaction.rs`, `relational_query_runtime.rs`, and
`command_runtime_manager.rs`. Exact-provider/analysis/query pieces include `relation_ipc*.rs`,
`provider_native_syntax.rs`, `provider_admission.rs`, `pyrefly_service.rs`, `rustc_service.rs`,
`python_derived_analysis.rs`, `rust_mir_derived_analysis.rs`, `common_derived_analysis.rs`,
`graph_program.rs`, and target `*_with_bindings` compiler functions.

Known production and decommission impact spans `src/daemon.rs`, `src/fabric.rs`,
`src/fabric/**`, `src/relational_model/**`, `src/bin/codefabric_model/**`, providers and auxiliary
roots, `src/relational_semantic_query.rs`, `src/query_service.rs`, `src/lib.rs`, Cargo manifests and
locks, Protobuf contracts/generated stubs, the FastMCP adapter, tests, acceptance/governance
contracts, rules, scripts, CI tooling, `justfile`, workflows, and package data. These are discovery
seeds, not a must-touch list. Each packet must run hidden-aware `rg`, structural `ast-grep`, source
outlines, Cargo/uv/package metadata, recipe inventory, and skipped-file accounting before edits.

## 3. Target invariants and prior-outcome reconciliation

The packet criteria implement design invariants I-40--I-51, design decisions D-40--D-49, library
decisions LD-30--LD-36, and doctrine P1--P36. No packet may weaken released public behavior while
removing a rejected internal authority.

### 3.1 V2 packet-outcome disposition

| V2 outcome | Status from review | V3 owner | Exact disposition |
|---|---|---|---|
| WP01 transition authority/freeze | invalidated | WP28 | replace sole-v2.0/bootstrap selectors with v2.1 authority and complete successor scope/disposition ledgers |
| WP02 metamodel/compiler/replay | invalidated | WP30 | delete target-facing subsystem; migrate only non-derivable released choices as explicit typed inputs |
| WP03 importer/initial model | invalidated | WP30 | delete importer and every live static-input reader; retain immutable history only |
| WP04 Arrow IPC boundary | reusable in progress | WP34 | retain relation-scoped framing/flow control; rebind schema and exact provider authority and close process interop |
| WP05 immutable epoch/catalog | reusable in progress | WP29, WP31 | complete `ProgrammaticFabricEpoch` production composition and fixed-point catalog/resource proof; delete older epoch path |
| WP06 model-compiled programs | invalidated with reusable core | WP31 | retain typed native lowering/logical cache; replace model inputs and causality with typed inputs/transformations |
| WP07 proof/provenance/capability | reusable in progress | WP32, WP38 | persist proof-bearing histories and replace predecessor-derived expectations with first-principles evidence |
| WP08 Tree-sitter/Ruff | reusable in progress | WP34 | complete exact batch admission, coverage/remainder, incrementality, and exclusive production route |
| WP09 Pyrefly | reusable in progress | WP34 | complete daemon consumption, invalidation, exact pinned-surface proof, and opaque JSON zero state |
| WP10 rustc | reusable in progress | WP34 | complete contained exact API route, stable-key/private seam, trust launcher, and summary/debug bypass zero state |
| WP11 provider integration | reusable in progress | WP34 | complete normalization/authority/conflict/unknown/provenance transformations without replay dependency |
| WP12 graph execution | reusable in progress | WP35, WP36 | prove highest DataFusion rung, application producer closure, resources, cancellation, and causal selection |
| WP13 semantic request forms | reusable in progress | WP36 | close all eight forms/roles and remove static/model-pinned query authority |
| WP14 authorized children | reusable in progress | WP31, WP36 | prove complete table/view/function/extension/store closure and production serving use |
| WP15 public delivery | reusable in progress | WP37 | connect relational runtime to actual UDS service and prove one immutable-result vertical |
| WP16 command path | reusable in progress | WP29, WP37 | supply concrete workspace factory/backends and prove exclusive durable mutation ingress/recovery |
| WP17 Delta/overlays | reusable in progress | WP32 | complete durable-history classification, exact providers, CDF/stats/retention/maintenance, and native overlays |
| WP18 proof/activation/recovery | reusable in progress | WP29, WP32 | compose production authority/ack factory and prove candidate-free exact-vector recovery and swap ordering |
| WP19 lifecycle/resources | reusable in progress | WP37 | compose source truth, invalidation, providers, publication, activation, query, and shared resource domain |
| WP20 release evidence | invalidated | WP38 | replace mandatory comparator evidence with independent first-principles execution; comparison optional |
| WP21 cutover | invalidated | WP41 | implement smallest durable sole-authority fence without restoring comparator/old-binary acceptance authority |
| WP22 evidence freeze | invalidated | WP38 | re-review surviving expectations; remove bootstrap category/DAG; issue independent successor transaction |
| WP23 Python analyses | reusable in progress | WP35 | bind to accepted exact inputs and prove semantics/provenance/unknown/incremental behavior |
| WP24 Rust analyses | reusable in progress | WP35 | bind to accepted rustc relations and prove semantics/provenance/private enrichment/incrementality |
| WP25 common analyses | reusable in progress | WP35 | close cross-language fixed points and one-producer-or-remainder coverage |
| WP26 trust launcher | reusable in progress | WP34 | make launcher exclusive and prove credential/network/process/resource/host/bypass behavior |
| WP27 model-derived schema | invalidated with reusable core | WP31 | retain phase validators/adapters; replace projection with provider/plan-derived contracts and no model bypass |

### 3.2 Durability and cache ledger required before closure

`WP28` creates a machine-readable ledger with one row for every input, provider batch,
transformation output, observation family, proof/coverage/provenance relation, activation/operation
event, immutable Arrow segment, query result, and cache. Each row names owner, lifetime, authority,
durability class, exact selector, reconstruction path, retention, CDF/statistics policy, and
consumer. Missing or duplicate rows fail. `WP32` cannot close until every relation required after
process loss is exact-version durable and every transient/cache row has an executable rebuild path.

### 3.3 Reuse rule

Existing code may be reused only after a current target consumer and target-authority proof are
named. Tests written for an invalidated v2 outcome are development evidence until rebound to a v3
criterion. Compatibility-only released field spellings may remain at the wire projection, but the
internal typed program/input/epoch vocabulary must not preserve a second authority.

## 4. Dependency-closed work packets

### WP28 — Fix successor authority, scope, durability, and disposition ledgers

**Dependencies.** None.

**Target invariants.** I-40, I-46, I-50, I-51; P3, P9--P10, P18, P20--P21, P25--P31, P36.

**Design and library references.** Design §§1.3, 2, 3.1, 3.11, 6--8; SUITE §§0--2; RM §§0--1;
DataFusion/Arrow alignment governance, model, observation, and test patterns; delta-rs alignment
state-identity, transaction, observability, retention, and test patterns.

**Change surface / Preflight / Known Touch.** Before the first edit, reconcile HEAD, declared-input
freshness, active-plan/state identity, v2 invalidation, suite selection, tracked/untracked files,
Cargo/uv graphs, recipe registry, package data, and hidden/skipped candidates. Use structural and
textual searches for bootstrap/model/epoch/schema/current/cache/history/evidence authorities and
compare the result to both ledgers. Known touch:
`contracts/governance/relational-fabric-successor-scope-v3.yaml`,
`contracts/governance/relational-fabric-durability-v3.yaml`,
`tooling/ci/successor_scope.py`, `tooling/ci/test_successor_scope.py`, and `justfile`.

**Required changes.**

1. Materialize an executable authority ledger covering every semantic concept in design §3.1 and
   every implementation surface named by L-20--L-55. Each row names current owner, target owner,
   decision, reason, consumer, cutover packet, deletion packet, positive oracle, negative oracle,
   and historical exclusions.
2. Materialize the durability/cache ledger from §3.2. Require a closed enum for durability class
   (`DELTA_HISTORY`, `IMMUTABLE_ARROW_SEGMENT`, `TRANSIENT_ARROW`, bounded non-authoritative cache),
   exact selector/rebuild/retention/CDF/statistics fields, and one row per discovered producer and
   consumer.
3. Encode the WP01--WP27 successor mapping in a validator so no prior outcome can disappear from
   execution status. Mark reuse as provisional until its v3 consumer and packet oracle pass.
4. Prove the v2.1 eight-master suite is the sole current design and v2.0/v1.3 are traversable
   historical predecessors. Prove v2 state is invalidated and cannot be resumed.
5. Generate legacy selectors from the accepted L-map plus current-tree discovery. Every selector
   records scope and explicit history/vendor/generated exclusions; an unmatched expected surface or
   an unclassified new candidate fails.
6. Add the four packet recipes and tests. Each gate emits structured selected/matched/skipped rows,
   fails on zero selections, and carries a committed bad authority, missing ledger row, stale suite,
   or scope-hole fixture.
7. Add `successor-library-reference-resolution-check` to resolve every zero-padded utilization ID
   against its declared alignment manual and every exact API citation against its declared-input
   digest. Add `successor-oracle-category-contract-check`, `successor-evidence-order-check`, and
   `successor-state-discipline-check` as executable governance validators; each consumes committed
   invalid plan/state fixtures and fails when the named contract is violated.

**Legacy disposition and decommission.** This packet changes no production authority. It replaces
the invalid v2.0 transition selector and comparator-first dependency with the v2.1 authority and
complete successor ledgers. Historical v2 artifacts remain immutable; live v2 execution routing is
denied. DB09--DB14 consume these ledgers and may not silently widen exclusions.

**Acceptance checks.**

Executable oracle: `successor-governance-artifact-integrity-check`
Governed criterion: `PC-WP28-INT`

Executable oracle: `successor-evidence-order-check`
Governed criterion: `PC-WP28-BEH`

Executable oracle: `successor-oracle-category-contract-check`
Governed criterion: `PC-WP28-NEG`

Executable oracle: `successor-state-discipline-check`
Governed criterion: `PC-WP28-OPS`

**Oracle category fault contract.** `INT` corrupts a declared digest, zero-padded pattern reference,
ledger row, or suite/history link; `BEH` proves the issued-evidence dependency precedes every
consumer and changes when the DAG is perturbed; `NEG` rejects a miscategorized, duplicate, or
zero-selection oracle fixture; `OPS` rejects forbidden output/path/evidence fields in schema-v2
state while accepting the same data in packet evidence artifacts.

Completion requires one proving commit containing schema-valid ledgers, exactly 36 L rows, exactly
27 v2 outcome rows, sole-current suite proof, all four governance validators plus
`successor-library-reference-resolution-check`, negative fixtures, and no production-code change.

### WP29 — Compose the real daemon from programmatic epoch authority

**Dependencies.** WP28.

**Target invariants.** I-40, I-43, I-45, I-49; P1--P3, P7--P8, P11, P16, P23, P32--P35.

**Design and library references.** Design §§3.2, 3.9, 3.11 and D-40/D-47/D-49; FAB §§3--5;
LIFE command/activation/recovery contracts; DataFusion/Arrow alignment MOD-1, ARR-1, CAT-1,
RUN-1, INT-1, GOV-1.

**Change surface / Preflight / Known Touch.** Reconcile all constructors and consumers of daemon
workspace/query/command/activation state, process startup and shutdown, test-only factories, and
default implementations. Known touch: `src/daemon.rs`, `src/fabric/programmatic_epoch.rs`,
`src/fabric/command_runtime_manager.rs`, `src/fabric/relational_query_runtime.rs`,
`src/fabric/activation_transaction.rs`, and `tests/integration/daemon.rs`.

**Required changes.**

1. Introduce one concrete production workspace factory that owns the governed `RuntimeEnv`, exact
   provider/input/transformation source, `ProgrammaticFabricEpochBuilder`, durable command ports,
   Delta activation authority, receipt-only reconciliation cache, resource coordinator, authorized
   child-session factory, and relational query runtime.
2. Replace `WorkspaceQueryBackend::default` and production `bootstrap_fabrics` selection with this
   factory. Missing exact inputs, durable roots/versions, credentials, or explicit policies reject
   workspace admission; they never synthesize bootstrap tables or an empty successful backend.
3. Bind every request to one `Arc<ProgrammaticFabricEpoch>` before planning and retain it until
   terminal result/cancellation. Bind every durable operation to one registered runtime manager and
   fenced writer. Shutdown drains admission, commands, streams, processes, and resources in order.
4. Separate construction ports from test probes. A test-only effect/backend may not implement or be
   selected by the production factory. Exhaustively enumerate installed command effects and query
   forms; unknown variants reject.
5. Emit structured startup observations naming exact input/program/provider/application releases,
   table version vector, runtime/session/resource policy, cache limits, activation head, and factory
   identity without treating those observations as acceptance.
6. Add cold-start, multi-workspace isolation, partial-construction rollback, shutdown, cancellation,
   and restart integration cases using real typed inputs and a temporary Delta/SQLite root.

**Legacy disposition and decommission.** This is the positive consumer cutover for L-27, L-35,
L-37--L-40, and L-42. It deliberately leaves predecessor definitions only until WP30 proves every
production consumer moved; no production fallback or dual write is permitted during the interval.

**Acceptance checks.**

Executable oracle: `production-composition-contract-integrity-check`
Governed criterion: `PC-WP29-INT`

Executable oracle: `programmatic-production-composition-check`
Governed criterion: `PC-WP29-BEH`

Executable oracle: `daemon-bootstrap-route-denial-check`
Governed criterion: `PC-WP29-NEG`

Executable oracle: `programmatic-runtime-lifecycle-check`
Governed criterion: `PC-WP29-OPS`

**Oracle category fault contract.** `INT` removes or corrupts a factory/port/configuration contract;
`BEH` changes one typed input and requires the installed plan/result to change; `NEG` attempts
default/bootstrap/missing-input admission and must fail; `OPS` injects partial construction,
cancellation, shutdown, or restart and requires bounded cleanup/recovery.

Completion requires one real daemon cold-start/query/command/restart vertical, an injected typed-
input change that causally changes the installed plan/result, a failing missing-input case, no
production `default`/bootstrap selection, and a proving commit.

### WP30 — Cut over and delete bootstrap, model, generated-schema, and dual-epoch authority

**Dependencies.** WP29.

**Target invariants.** I-40--I-43, I-45, I-49, I-51; P1--P3, P12, P18, P26--P28, P31--P36.

**Design and library references.** Design §§1, 3.1--3.2, 3.7, 6; D-40, D-47, D-49;
L-20--L-24, L-27--L-30, L-35, L-37--L-39, L-54--L-55 as reconciled in §6.

**Change surface / Preflight / Known Touch.** Trace every compiler/build/runtime/test/tool/package
consumer before deletion, including hidden files and generated include paths. Inspect released wire
and identity allocations before changing fields. Known touch: `src/relational_model/mod.rs`,
`src/relational_model/schema.rs`, `src/relational_model/replay.rs`,
`src/relational_model/release.rs`, `src/fabric/epoch.rs`,
`src/fabric/model_migration_command_effect.rs`, `src/fabric/command.rs`,
`src/fabric/command_effect_router.rs`, `src/provider_admission.rs`,
`src/relational_semantic_query.rs`, `src/bin/codefabric_model/main.rs`,
`src/bin/codefabric_model/legacy_model_importer.rs`, `src/schema_registry.rs`, `src/registries.rs`,
`src/lib.rs`, `Cargo.toml`, and `justfile`.

**Required changes.**

1. Move only application identity encoders, released wire helpers, `FabricEpochId`, schema phase
   validators, and runtime configuration proven authority-neutral into inward target-owned modules.
   Make target consumers compile before deleting their former owners.
2. Delete `src/relational_model/**`, the model importer, target-facing `src/bin/codefabric_model/**`
   and `tooling/model/**`, the `model-compiler` bin/feature/recipe graph, bootstrap schema/table
   construction, installed/resealed ontology bundle authority, and live registry/model-artifact
   readers. Preserve foreign Protobuf generation only in its independent tooling boundary.
3. Delete old `FabricEpochBuilder`/`FabricEpoch`, dual epoch handles/pins, replay compiler wrappers,
   `SchemaContractModelRows::from_model_epoch`, and every `ModelEpoch -> SchemaContract` path.
   Rename the surviving type to the sole `FabricEpoch` only if doing so does not preserve an
   ambiguous compatibility alias.
4. Delete `ApplyModelMigration`, its effect/ports/events/admin route, and runtime model/compiler
   release identifiers. If an externally released field spelling cannot yet be removed, isolate it
   in a versioned compatibility projection mapped to the program/application release vector and
   prove no internal branch consumes its old meaning.
5. Delete legacy provider-admission overloads and model-pinned query compilers after exact
   programmatic admission and query consumers compile. No deprecated wrapper, feature gate, trait
   alias, fallback branch, or test helper may keep the route selectable.
6. Remove generated model/schema/identity parallel arrays once explicit typed inputs and observed
   plan/provider schemas pass equivalence and causal-load-bearing proof. Released Protobuf stubs,
   allocation tombstones, and immutable history are excluded by exact path, not broad directories.
7. Update tests to construct exact batches/typed inputs/transformations directly. Delete tests and
   fixtures whose only purpose is the rejected authority; do not translate them into new replay
   fixtures.

**Legacy disposition and decommission.** Completes DB09 and the early portions of DB12/DB13.
L-20--L-24, L-27, L-29, L-54, and model-owned L-55 become physical deletion targets; L-28/L-30/
L-35/L-37--L-39 retain only reshaped programmatic consumers. V2.0/v1.3 designs, invalidated plans/
state/reviews, released allocations, and tombstone evidence remain historical and cannot be runtime
inputs.

**Acceptance checks.**

Executable oracle: `bootstrap-model-decommission-integrity-check`
Governed criterion: `PC-WP30-INT`

Executable oracle: `bootstrap-model-consumer-cutover-check`
Governed criterion: `PC-WP30-BEH`

Executable oracle: `bootstrap-model-dual-authority-zero-state-check`
Governed criterion: `PC-WP30-NEG`

Executable oracle: `programmatic-model-free-restart-check`
Governed criterion: `PC-WP30-OPS`

**Oracle category fault contract.** `INT` removes a required disposition/deletion selector or
retained-primitive mapping; `BEH` proves every target consumer still constructs and queries from
typed programmatic inputs; `NEG` reintroduces any bootstrap/model/importer/old-epoch/migration
symbol, feature, recipe, package, or selection route and must fail; `OPS` restarts without model
artifacts and requires deterministic recovery.

Completion requires positive cold construction and query from programmatic inputs, compilation of
all target consumers, deletion of named files/modules/features/recipes/package edges, a history-
aware zero-state scan with no unclassified skips, an attempted legacy selection that fails, and a
proving commit.

### WP31 — Close plan-derived schema, fixed-point catalog, child sessions, and DataFusion caches

**Dependencies.** WP30.

**Target invariants.** I-41--I-44, I-47, I-49; P4--P8, P12--P18, P23--P24, P27, P32--P35.

**Design and library references.** Design §§3.2--3.4, 3.6, 4; D-40--D-42, D-44, D-46;
LD-30--LD-33; DataFusion/Arrow alignment SCH-1, CAT-1, LOG-1, PHY-1, RUN-1, INT-1,
OBS-1, EXT-1, TST-1.

**Change surface / Preflight / Known Touch.** Inventory every schema constructor, provider
registration, transformation compiler, observation producer, custom logical/physical node, child
catalog clone/rebuild, cache key, cache owner, and public/session escape. Known touch:
`src/schema_contract.rs`, `src/fabric/programmatic_schema.rs`,
`src/fabric/programmatic_epoch.rs`, `src/fabric/datafusion_cache.rs`,
`src/fabric/child_session.rs`, `src/fabric/child_session/resource_governance.rs`,
`src/relational_program.rs`, and `src/fabric/graph_program.rs`.

**Required changes.**

1. Make admitted provider `SchemaRef` and built `LogicalPlan::schema()` the only schema sources.
   Validate field IDs/order/types/nullability, nested/dictionary/extension metadata, qualifiers,
   projection/filter/statistics remaps, storage mappings, IPC, batch, stream, and public output
   through one `SchemaContract`; an expected schema mismatch rejects.
2. Close `ProgrammaticTransformation` as typed application constructors with semantic ID/version,
   exact input relation IDs, resource/determinism/order policy, and provenance. Topologically order
   them; use bounded native recursive semantics only when declared; reject cycles and inert inputs.
3. Install and derive the five programmatic observation histories until a complete unchanged
   iteration. Reject missing/duplicate/dangling observations, incomplete dependency closure,
   non-self-description, and iteration/row/resource overflow with explicit unknown/rejection.
4. For every operation, record the highest viable rung: Arrow kernel, native expression/operator,
   transparent builder, UDF/table function/provider, planner hook, logical extension, custom
   physical node. Custom nodes expose all expressions/children, support rewrites and child
   replacement, recompute properties/statistics/partitioning/order, propagate physical planning
   context, reservations/spill/cancellation, and reset per-execution state.
5. Build authorized child sessions from fresh restricted catalogs. Rebuild views in the child or
   recursively validate complete bound table/function/extension/object-store/runtime-variable
   closure. Reject prebound parent providers and internal names not in the capability relation.
6. Bound metadata, file-statistics, object-list, and compiled/optimized logical-plan caches by
   entries/bytes and exact authority keys. Object-list TTL is 30 seconds maximum refresh, not
   validity. Prove collision handling and cross-session logical-plan reuse; rebuild physical plans
   and results per execution. Emit cache/resource observations and make eviction semantics-neutral.

**Legacy disposition and decommission.** Completes target replacement for L-28--L-29, L-35--L-36,
L-39, and L-41. Any remaining replay wrapper, declared-schema author, parent-provider leak,
unbounded/default cache, physical/result cache, or cache-selected current route is deleted in this
packet, not deferred as compatibility.

**Acceptance checks.**

Executable oracle: `datafusion-contract-matrix-integrity-check`
Governed criterion: `PC-WP31-INT`

Executable oracle: `datafusion-plan-schema-cache-check`
Governed criterion: `PC-WP31-BEH`

Executable oracle: `authorized-child-schema-rejection-check`
Governed criterion: `PC-WP31-NEG`

Executable oracle: `datafusion-cache-resource-operations-check`
Governed criterion: `PC-WP31-OPS`

**Oracle category fault contract.** `INT` corrupts a schema/plan/dependency/rung contract or omits
RUN-03/RUN-08 fingerprint material; `BEH` proves plan-derived schemas, fixed-point observations,
native rung choice, and valid cross-session logical reuse; `NEG` injects a mismatched schema,
unauthorized child dependency, cache collision, or parent provider and requires rejection; `OPS`
exercises bounded eviction, fresh physical plans, cancellation, spill, and per-execution reset.

Completion requires native-plan inspection, schema fault cases at every phase, fixed-point closure
including self-observation, child-catalog leak denial, bounded eviction/collision tests, two fresh
physical plans for one cached logical plan, no result cache, and a proving commit.

### WP32 — Close Delta proof histories, exact activation, and candidate-free recovery

**Dependencies.** WP30. This packet may execute in parallel with WP31 because its known-touch set
owns Delta histories and activation/recovery while WP31 owns DataFusion schema/catalog/cache
closure; shared public types must already be fixed by WP30.

**Target invariants.** I-43, I-45--I-47; P3, P7--P11, P16--P20, P23--P24, P32--P34.

**Design and library references.** Design §§3.3, 3.7--3.9, 4; D-41, D-45--D-47; LD-34--LD-35;
delta-rs alignment MOD-1, STA-1, IO-1, TXN-1, MUT-1, CDF-1, STR-1, MET-1, LOG-1,
LAY-1, OBS-1, SEC-1, TST-1.

**Change surface / Preflight / Known Touch.** Inventory every Delta URI/table builder, version
resolution, raw Parquet/listing path, write/retry/transaction marker, observation/proof/coverage/
provenance producer, CDF cursor, statistics adapter, activation row, current selector, receipt
cache, recovery branch, maintenance/retention command, and object-store registration. Known touch:
`src/fabric/delta_exact.rs`, `src/fabric/delta_write.rs`,
`src/fabric/programmatic_observation_delta.rs`, `src/fabric/activation_control_delta.rs`,
`src/fabric/activation_transaction.rs`, `src/fabric/command_delta.rs`,
`src/fabric/proof.rs`, and `src/fabric/retention_command_effect.rs`.

**Required changes.**

1. Close the WP28 durability ledger. Persist the five programmatic observation families and every
   additional coverage, capability, proof, provenance, derived-analysis, operation, activation, or
   source intermediate required after process loss as stable append-only Delta history. Leave
   execution-only buffers transient and prove their deterministic rebuild.
2. Canonicalize every Delta root as a workspace-owned object-store URL and select every table with
   one exact version. Build a typed, canonical, reversible relation-to-root/version vector; bind its
   digest plus source/provider/program/policy/proof/release identities into the activation row and
   epoch. No runtime path resolves `latest` after admission.
3. Use delta-rs table providers, never raw Parquet files or cached object listings, for semantic
   reads. Preserve full table/file/column statistics and adapt projection/filter/column-mapping/
   deletion-vector semantics through `SchemaContract`. Treat missing statistics as unknown cost,
   not empty data.
4. Create required table properties, including CDF, at table creation. Append with typed writers,
   field metadata intact, target file size/rows/compression explicitly chosen, one application
   transaction identity, operation/commit metadata, and application retry count zero. On uncertain
   commit, inspect exact transaction/commit evidence before retrying.
5. Implement CDF checkpoints as exact `(table_root, from_version, through_version, consumer)`
   transport cursors with retention preconditions, gap detection, replay/idempotence, schema
   evolution handling, and fallback exact reconstruction. CDF/commit metadata/statistics cannot
   certify semantic completeness.
6. Append/read back one fenced activation event only after all exact table versions and proof rows
   exist. Derive the unique current head from chain validity. Reject split brain, missing parent,
   wrong vector/digest, stale fence, partial history, or ambiguous head.
7. Start recovery admission-closed with no process-local candidate. Reconcile durable command and
   activation evidence, reconstruct a fresh sealed epoch from the exact vector, install it, then
   reconcile receipt/ack and reopen. The bounded `ActivationReconciliationReceiptCache` may save a
   receipt only; clearing, corrupting, or replaying it cannot change selected state.
8. Route optimize/checkpoint/vacuum/retention through explicit administrative commands with active-
   lease/pin/CDF-reader/proof-history safeguards, dry-run visibility, and postcondition validation.

**Legacy disposition and decommission.** Completes the durable replacement for L-35--L-38 and the
state portions of L-40. Delete custom mutable snapshot/current manifests, raw Delta-directory
Parquet/listing authority, duplicated SQLite semantic current, automatic write retries, candidate-
required recovery, and cache-selected activation. Keep SQLite temporal queue/lease/command progress
and released public snapshot projection only in their bounded roles.

**Acceptance checks.**

Executable oracle: `delta-durability-protocol-integrity-check`
Governed criterion: `PC-WP32-INT`

Executable oracle: `delta-exact-reconstruction-v3-check`
Governed criterion: `PC-WP32-BEH`

Executable oracle: `activation-receipt-nonauthority-check`
Governed criterion: `PC-WP32-NEG`

Executable oracle: `candidate-free-recovery-check`
Governed criterion: `PC-WP32-OPS`

**Oracle category fault contract.** `INT` corrupts a durability row, exact selector, protocol/
feature fingerprint, or transaction identity; `BEH` proves exact-version read/write/CDF/history and
reconstruction with supported features; `NEG` injects unsupported reader/writer features, raw
listing/latest/current-pointer selection, or receipt-only authority and requires rejection; `OPS`
injects unknown commits, CDF gaps, cache loss, split heads, process loss, and guarded maintenance.

Completion requires exact reopen after process loss, CDF gap/fallback proof, full-statistics
inspection, uncertain-commit reconciliation, split-head/fence/cache faults, guarded maintenance,
raw-listing/current-pointer negative inventory, and a proving commit.

### WP33 — Issue and independently review successor expectations and negative fixtures

**Dependencies.** WP28.

**Target invariants.** I-40, I-48, I-50; P9--P10, P18--P22, P25, P27, P29--P30, P36.

**Design and library references.** Design §3.10 and D-48; design §§7--8; SUITE proof/release
contracts; controlling status-review evidence order; DataFusion/Arrow TST-01, TST-03, TST-07,
TST-11, and TST-14; delta-rs TST-03, TST-08, TST-11, and TST-12; exact API authorities declared
in §2.

**Change surface / Preflight / Known Touch.** Inventory every expectation, golden, comparator,
capture, digest, count, fixture, fault, claim, evidence transaction, and acceptance DAG. Trace each
expected value to an authoring source and prove it does not import target/predecessor output or
production expected-value code. Known touch:
`contracts/acceptance/relational-fabric-v3/expectations.jsonl`,
`contracts/acceptance/relational-fabric-v3/negative-fixtures.jsonl`,
`contracts/acceptance/relational-fabric-v3/evidence-issuance.json`,
`tooling/ci/successor_evidence_issuance.py`,
`tooling/ci/test_successor_evidence_issuance.py`, and `justfile`.

**Required changes.**

1. Independently author decoded typed expectations for exact provider facts, transformations,
   derived analyses, all eight semantic forms, Delta exact-version/protocol behavior, activation/
   recovery, authorization, resource terminals, security denial, released wire projection, and
   clean/incremental equivalence. Every row names source anchor, governing clause, complete input
   universe, exact pins, ordering/null/unknown/provenance semantics, limitation, and future
   consumer/oracle.
2. Require a reviewer distinct from the expectation author and implementation owner to accept or
   reject every row before WP34 can start. Store immutable author/reviewer identity and disposition
   in the versioned issuance artifact; neither target execution nor predecessor comparison may
   supply the verdict.
3. Author at least one semantic causal fixture and one absence/rejection/failure fixture per claim
   family before consumer implementation. Faults change authoritative inputs, provider batches,
   transformations, schemas/plans, coverage, exact versions/features, authorization, resource,
   protocol, or public output. Digest/count/text-only perturbations qualify only for an integrity
   claim.
4. Delete `bootstrap_model_semantics`, mandatory WP01/comparator/replay-agreement/model-digest
   edges, and producer-generated expected values from the current issuance DAG. Preserve old
   transactions only as labelled history. A predecessor comparator may remain optional diagnostic
   input but deleting it must leave issuance and later acceptance verdicts unchanged.
5. Add executable validators for claim completeness, author/reviewer independence, frozen
   expectation/fixture identity, consumer dependency order, zero selected tests, and target/
   predecessor import attempts. WP34--WP37 dependencies must include WP33; a fixture mutation after
   review invalidates issuance and reopens its consumer.
6. Do not execute unfinished production behavior here. Freeze expected artifacts and negative
   fixtures for later WP38 execution; observations from WP38 may add evidence only after a newly
   discovered claim returns through independent issuance/review.

**Legacy disposition and decommission.** This is the positive P30 replacement for v2 WP22 and the
issuance portion of L-44/L-45/L-49. It removes bootstrap/comparator authority from the current
evidence DAG before provider consumers, while preserving immutable historical evidence and optional
non-gating diagnostics until WP39 proves their deletion.

**Acceptance checks.**

Executable oracle: `successor-evidence-transaction-integrity-check`
Governed criterion: `PC-WP33-INT`

Executable oracle: `successor-expected-behavior-review-check`
Governed criterion: `PC-WP33-BEH`

Executable oracle: `successor-negative-fixture-independence-check`
Governed criterion: `PC-WP33-NEG`

Executable oracle: `successor-evidence-issuance-readiness-check`
Governed criterion: `PC-WP33-OPS`

**Oracle category fault contract.** `INT` corrupts a claim/fixture digest, dependency, or review
record; `BEH` changes one independently expected decoded value and detects the mismatch; `NEG`
imports target/predecessor output or supplies a forbidden/bad case that must be rejected; `OPS`
tests issuance/review ordering, zero selection, and immutable re-entry after a changed claim.

Completion requires reviewed expected rows and pre-authored negative/causal fixtures for every
claim family, no bootstrap/model/comparator pass/fail edge, WP34--WP37 dependency enforcement, a
successful issuance run with predecessor artifacts unavailable, and a proving commit.

### WP34 — Complete exact provider batches, Arrow IPC, admission, and Rust trust

**Dependencies.** WP31, WP32, and WP33.

**Target invariants.** I-40--I-44, I-46, I-48; P3--P9, P12--P13, P20, P22--P24, P31--P35.

**Design and library references.** Design §§3.2, 3.5, 3.7 and D-40/D-43/D-45; LD-30, LD-34,
LD-36; GEN provider/API/coverage contracts; Arrow alignment ARR-1, SCH-1, INT-1, OBS-1, TST-1;
exact Tree-sitter, Ruff, Pyrefly, and rustc reference APIs cited by GEN.

**Change surface / Preflight / Known Touch.** Reinventory every provider/API capability claim,
borrowed/vendor type escape, opaque payload, schema source, IPC producer/consumer, admission
overload, coverage/remainder branch, process trust path, generated control message, and legacy JSON
fallback across all four build domains. Known touch: `src/relation_ipc.rs`,
`src/relation_ipc_proto.rs`, `src/relation_ipc_wire.rs`, `src/provider_native_syntax.rs`,
`src/provider_admission.rs`, `src/tree_sitter_adapter.rs`, `src/ruff_adapter/mod.rs`,
`src/pyrefly_service.rs`, `src/rustc_service.rs`, `pyrefly-sidecar/src/server.rs`,
`rustc-extractor/src/wrapper.rs`, and `contracts/rpc/provider_control.proto`.

**Required changes.**

1. Emit application-owned provider-native `RecordBatch` relations directly from exact pinned
   Tree-sitter/Ruff, Pyrefly Query/TSP/module-resolver/selected Glean/LSP, and `rustc_public` plus
   the smallest dated-nightly private stable-key/source/borrowck seam. Preserve raw kind and
   normalized kind separately; no borrowed provider object crosses an adapter/process boundary.
2. Carry one Arrow schema/dictionary scope and bounded batch sequence per semantic stream. Keep
   Protobuf control-only. Validate protocol/provider/application releases, run/job/relation/program/
   schema IDs, source/context pins, schema/fixed-width IDs/metadata, checksums, credits, sequence,
   byte/row/deadline limits, and terminal state before admission.
3. End every requested scope with completed, intentional remainder, unknown, diagnostics, evidence,
   capability, trust, and resource status. Require requested minus completed/intentional remainder
   to be empty for closed coverage. Missing trailer, corruption, cancellation, or version/schema
   mismatch yields explicit incomplete/unknown and deletes partial semantic admission.
4. Make one programmatic provider-admission API exclusive. It validates exact schemas and batches,
   installs raw relations, and supplies typed normalization/authority/conflict/unknown/provenance
   transformations. Conflicting evidence remains queryable; an unresolved conflict does not become
   a single chosen fact.
5. Complete affected-module/file invalidation and clean/incremental equivalence for every provider.
   Advertised capability is the join of exact API support, requested/completed coverage, admission,
   producer closure, provenance, and proof; compiled code or an enum variant is insufficient.
6. Make the Rust untrusted-compilation launcher the only production build-script/proc-macro route.
   Deny credentials/network by default, bound process group/CPU/memory/output/time, validate source/
   toolchain/context, cover supported hosts, and kill/reap descendants on cancellation.
7. Remove opaque semantic JSON, one-row payloads, defensive mirrors, static kind inventories,
   `OwnedMirItem`/debug summaries, uncontained extractor bypasses, and old admission overloads after
   their exact consumers pass.

**Legacy disposition and decommission.** Completes L-30--L-34 and the provider portions of DB10.
Process/revision isolation, released control wire, source-coordinate capture, and lifecycle ports
are retained/reshaped; payload mirrors, static registries, summary substitutes, JSON fallbacks, and
parallel admission are deleted.

**Acceptance checks.**

Executable oracle: `provider-ipc-contract-integrity-check`
Governed criterion: `PC-WP34-INT`

Executable oracle: `exact-provider-batch-check`
Governed criterion: `PC-WP34-BEH`

Executable oracle: `provider-admission-exclusivity-check`
Governed criterion: `PC-WP34-NEG`

Executable oracle: `provider-trust-coverage-remainder-check`
Governed criterion: `PC-WP34-OPS`

**Oracle category fault contract.** `INT` corrupts schema/control/sequence/checksum/coverage
contracts or exact API/pin identity; `BEH` proves exact provider-native Arrow rows and clean/
incremental equality; `NEG` attempts opaque JSON, old admission, private bypass, corrupt stream, or
missing terminal and requires rejection; `OPS` exercises credits, byte/row/deadline bounds,
cancellation, launcher containment, descendant cleanup, and explicit remainder accounting.

Completion requires real cross-process streams, exact schema/API assertions, flow-control and
process/resource behavior, corrupt/missing-terminal/private-bypass faults, clean/incremental
provider equivalence, opaque-payload/admission zero state, and a proving commit.

### WP35 — Close all application-owned derived-analysis producers

**Dependencies.** WP34.

**Target invariants.** I-42--I-43, I-46, I-48; P1--P6, P9--P10, P14--P17, P20--P24,
P27--P30, P32--P35.

**Design and library references.** Design §3.6 and D-44; ONT derived-family and unknown contracts;
GEN authority/coverage rules; DataFusion/Arrow alignment LOG-1, PHY-1, RUN-1, OBS-1, EXT-1, TST-1;
petgraph reference only for proved irreducible bounded kernels.

**Change surface / Preflight / Known Touch.** Enumerate every accepted derived family, provider vs
application owner, input relation, fixed-point edge, algorithm/precision/completeness/witness field,
producer registration, persisted intermediate, unsupported remainder, old procedural producer, and
petgraph identity. Known touch: `src/python_derived_analysis.rs`,
`src/rust_mir_derived_analysis.rs`, `src/common_derived_analysis.rs`,
`src/fabric/derived_producer_closure.rs`, `src/fabric/graph_program.rs`,
`src/relational_program.rs`, and `src/core_facts.rs`.

**Required changes.**

1. Bind Python owner-local CFG, reaching definitions, alias/points-to, effects/resources, async/
   coroutine, and uncertainty derivations to accepted exact provider relations through typed
   transformations. Preserve occurrence/entity separation and explicit unknowns.
2. Bind Rust MIR ownership/borrow/move/drop/flow/alias/resource/async/unsafe derivations to exact
   public/private compiler relations. Private enrichment is evidence with its own release and
   remainder; it never silently substitutes when unavailable.
3. Close common graph, call-resolution, hierarchy, effect/resource, and bounded interprocedural
   fixed points. Choose the highest DataFusion rung operation by operation. Any petgraph kernel is
   transient, bounded, Arrow-in/Arrow-out, keyed by canonical external IDs, and behind a complete
   typed logical extension or explicitly opaque planning-time provider.
4. For every output row record application algorithm/version, precision, input exact vector,
   completeness, support facts or witness paths, and provenance closure. Persist only intermediates
   named durable by the WP28/WP32 ledger.
5. Enforce exactly one accepted producer per family or one explicit unsupported remainder. Reject
   duplicate producers, missing owners, undeclared algorithms, orphan dependencies, unsupported
   output presented as empty, and provider-owned application judgment.
6. Replace procedural/current JSON derivations, persisted graph indices, generated producer
   registries, and replay wrappers only after typed producer consumers pass. Delete obsolete tests
   rather than preserving parallel derived answers.

**Legacy disposition and decommission.** Completes the analysis portions of L-28/L-30/L-41/L-49
and DB10. Provider-native inputs remain distinct. Authority-neutral graph algorithms may remain
only under the exact bounded contract; generated registries, persisted indices, and procedural
parallel producers are deleted.

**Acceptance checks.**

Executable oracle: `analysis-producer-contract-integrity-check`
Governed criterion: `PC-WP35-INT`

Executable oracle: `analysis-producer-semantic-check`
Governed criterion: `PC-WP35-BEH`

Executable oracle: `analysis-causal-fault-check`
Governed criterion: `PC-WP35-NEG`

Executable oracle: `analysis-fixed-point-resource-check`
Governed criterion: `PC-WP35-OPS`

**Oracle category fault contract.** `INT` corrupts producer ownership, algorithm/version, input
vector, precision, witness, or provenance declarations; `BEH` proves independently expected Python,
Rust, and common derived rows; `NEG` changes a causal input or introduces duplicate/missing/
unsupported-as-empty/procedural producers and must discriminate or reject; `OPS` exercises bounded
fixed-point convergence, cancellation, transient-kernel resources, and incremental re-execution.

Completion requires independently asserted rows for every accepted family, precision/unknown/
provenance proof, fixed-point convergence and resource cancellation, changed-input causal deltas,
one-producer-or-remainder closure, procedural/index/registry zero state, and a proving commit.

### WP36 — Close semantic request programs and authorized graph/query execution

**Dependencies.** WP35.

**Target invariants.** I-41--I-44, I-47--I-50; P2--P8, P12--P17, P20, P22--P24,
P27--P30, P32--P35.

**Design and library references.** Design §§3.4, 3.6, 3.9 and D-42/D-44/D-47; QRY §§1--9;
DataFusion/Arrow alignment CAT-1, LOG-1, PHY-1, RUN-1, INT-1, GOV-1, EXT-1, TST-1.

**Change surface / Preflight / Known Touch.** Inventory all eight public forms, composition roles,
request/result schemas, query compilers, graph operation rungs, static crosswalks, capability
booleans, SQL/table/function-name escapes, model/epoch pins, bypass execution, child-session
bindings, ordering/pagination/truncation/unknown behavior, and result projection. Known touch:
`src/relational_semantic_query.rs`, `src/fabric/relational_query_runtime.rs`,
`src/fabric/child_session.rs`, `src/query_service.rs`, `src/semantic_query.rs`,
`src/fabric/published_arrow_result.rs`, and `contracts/rpc/cpg_query_service.proto`.

**Required changes.**

1. Represent lookup, bounded neighborhood, path/witness, composition, relation slice, explanation,
   capability/reference, and administration/status requests as strict typed relations. Compile them
   with closed `ProgrammaticTransformation` constructors against one pinned admitted epoch.
2. Preserve released public meaning, IDs, ordering, pagination, limits, redaction, unknown/
   capability-gap, explanation, and structured error behavior. A compatibility-only old field name
   maps at the public projection and cannot select a model/bootstrap/compiler branch.
3. Execute bounded graph operations at the highest valid DataFusion rung and expose their complete
   expression/child/resource/cancellation semantics. Prove deterministic results across partition
   and batch layout and no persisted provider-local graph identity.
4. Create a fresh authorized child for every request/lease. Expose only authorized capabilities and
   recursively closed bound dependencies. Deny arbitrary SQL, internal names, parent catalog/store/
   function leakage, unbounded plans, and cross-principal logical-cache collisions.
5. Return explicit no-match, unresolved, unavailable, partial, or limited outcomes. Negative claims
   require explicit negative facts or complete relevant coverage; empty output cannot prove absence.
6. Delete fixed query-form crosswalks, package/model capability registries, model epoch pins,
   predecessor `SemanticQuery` planners, and bypass execution after every released form is served by
   the relational runtime.

**Legacy disposition and decommission.** Completes query portions of L-23/L-26/L-39/L-41/L-42 and
DB10/DB11. Released request/response schemas remain under L-43. Static registries, model pins,
legacy planners, SQL/name escapes, and parent-session leaks are deleted.

**Acceptance checks.**

Executable oracle: `semantic-request-contract-integrity-check`
Governed criterion: `PC-WP36-INT`

Executable oracle: `semantic-request-program-check`
Governed criterion: `PC-WP36-BEH`

Executable oracle: `query-unknown-negative-proof-check`
Governed criterion: `PC-WP36-NEG`

Executable oracle: `graph-query-resource-operations-check`
Governed criterion: `PC-WP36-OPS`

**Oracle category fault contract.** `INT` corrupts a request/result schema, role, bound, ordering,
or dependency contract; `BEH` proves all eight forms through authorized children with exact public
meaning; `NEG` attempts SQL/name/parent-catalog/store/function/model-pin/bypass access or an absence
claim without complete evidence and must reject; `OPS` varies partition/batch layout and exercises
limits, pagination, cancellation, truncation, and graph-resource cleanup.

Completion requires all eight forms through real authorized children, exact public ordering/error/
unknown behavior, graph plan inspection, adversarial catalog/function/store/auth/cache isolation,
legacy planner/pin/crosswalk zero state, and a proving commit.

### WP37 — Complete lifecycle, resource, daemon, UDS, and FastMCP production delivery

**Dependencies.** WP29, WP30, WP31, WP32, WP34, WP35, and WP36.

**Target invariants.** I-43--I-50; P3, P7--P11, P13, P16--P17, P20--P24, P32--P35.

**Design and library references.** Design §§3.7--3.11 and D-45--D-49; LIFE full command/update/
publication/activation/lease/fence/recovery invariants; SRV full daemon/UDS/stream/presentation
boundary; DataFusion/Arrow RUN-1/INT-1/OBS-1/GOV-1 and Delta TXN-1/LOG-1/OBS-1/SEC-1.

**Change surface / Preflight / Known Touch.** Trace real startup through repository discovery, safe
source reads, watcher loss/rescan, invalidation, provider orchestration, command journal/effects,
Delta publication/activation/recovery, query admission, result resource, gRPC UDS, Python adapter,
shutdown, and restart. Reconcile all resource domains and public/protocol compatibility. Known
touch: `src/daemon.rs`, `src/continuous.rs`, `src/continuous/coordinator.rs`,
`src/fabric/command_runtime.rs`, `src/fabric/command_actor.rs`,
`src/fabric/command_runtime_manager.rs`, `src/fabric/source_wave_command_effect.rs`,
`src/fabric/published_arrow_result.rs`, `src/query_service.rs`,
`codefabric-cpg-mcp/src/codefabric_cpg_mcp/server.py`,
`codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/client.py`, and
`tests/integration/daemon.rs`.

**Required changes.**

1. Route repository/worktree identity, descriptor-relative authoritative reads, source images,
   ignore/attribute/path semantics, watcher debounce/loss/rescan, and invalidation into exact typed
   input/provider batches. Missing/changed/lost inputs create explicit gaps and rebuild work.
2. Route every durable source wave, provider publication, compaction, retention, rollback,
   activation, and administration operation through the one exhaustive idempotent command actor and
   concrete effect router. Remove direct storage/current mutations and test/migration effects from
   the production registry.
3. Enforce one shared resource domain across source reads, providers/subprocesses, DataFusion
   execution, Delta writes, result streams, and FastMCP delivery: admission, memory, CPU/time,
   concurrency, byte/row limits, flow control, cancellation, process-group cleanup, fairness, and
   terminal accounting.
4. Publish only proved epochs, swap one atomic handle, preserve per-request epoch/authorization
   leases, and coordinate retention/maintenance with active pins. Restart/reboot follows the
   candidate-free exact-vector recovery contract before UDS admission.
5. Wire the actual UDS gRPC service to `RelationalQueryRuntime` and immutable Arrow result resources.
   Validate peer/workspace/request/session identities, deadlines/cancellation, frame sequence,
   checksums, resource cleanup, reconnect, and error mapping. No debug/in-process/backend shortcut
   serves production.
6. Keep FastMCP STDIO protocol-only and presentation-only. It uses real generated stubs and strict
   public models, streams resources with bounded acknowledgement/cancellation, and derives live
   reference/capability/status from daemon results. It holds no Arrow/DataFusion/Delta processing,
   schema registry, model registry, fingerprint authority, or mutable CPG state.
7. Add one real end-to-end scenario from repository/source change through exact providers,
   transformations, durable proof/activation, authorized query, UDS delivery, and FastMCP response;
   cover cancellation, process loss, watcher rescan, partial provider, backpressure, and restart.

**Legacy disposition and decommission.** Completes positive cutover for L-25--L-26, L-35--L-40,
L-42--L-43 and DB11. Delete default/empty backends, direct mutation/publication/current paths,
mutable snapshot authority, in-process query shortcuts, packaged adapter registries/fingerprints,
and Python semantic state after the real vertical passes. Preserve released `.proto`/public models,
strict presentation helpers, and bounded lifecycle/source-truth mechanisms.

**Acceptance checks.**

Executable oracle: `public-lifecycle-wire-contract-integrity-check`
Governed criterion: `PC-WP37-INT`

Executable oracle: `lifecycle-production-vertical-check`
Governed criterion: `PC-WP37-BEH`

Executable oracle: `fastmcp-presentation-boundary-check`
Governed criterion: `PC-WP37-NEG`

Executable oracle: `resource-cancellation-recovery-check`
Governed criterion: `PC-WP37-OPS`

**Oracle category fault contract.** `INT` corrupts command/wire/result-resource/identity/terminal
contracts; `BEH` proves the real source-to-provider-to-Delta-to-authorized-query-to-UDS/FastMCP
vertical; `NEG` introduces direct mutation, debug/default serving, Python semantic state, invalid
peer/workspace identity, or protocol-only STDOUT violation and requires rejection; `OPS` injects
watcher loss, process loss, backpressure, cancellation, shutdown, reconnect, and restart recovery.

Completion requires one real four-domain public vertical, source-loss/rescan and restart recovery,
command exclusivity, resource/backpressure/cancellation cleanup, strict wire compatibility, no
default/debug/Python-state route, and a proving commit.

### WP38 — Issue and execute first-principles successor evidence

**Dependencies.** WP37.

**Target invariants.** I-40--I-51; P9--P10, P18--P22, P24--P30, P36.

**Design and library references.** Design §3.10 and D-48; design §§6.3, 7--8; SUITE proof and
release contracts; DataFusion/Arrow and delta-rs alignment TST-1 plus the exact behavioral,
failure, observability, security, and reconstruction rows used by WP31--WP37.

**Change surface / Preflight / Known Touch.** Inventory every expectation, golden, comparator,
capture, digest, count, fault, proof row, producer, decoder, evidence transaction, evidence DAG,
acceptance recipe, and test filter. Trace how each expected value was authored and whether it
imports target/predecessor outputs or implementation modules. Known touch:
`contracts/acceptance/relational-fabric-v1/expectations.jsonl`,
`contracts/acceptance/relational-fabric-v1/evidence-transaction.json`,
`tooling/ci/relational_fabric_evidence.py`,
`tooling/ci/test_relational_fabric_evidence.py`, `src/gate_b_candidate/vertical.rs`,
`src/golden_corpus.rs`, and `justfile`.

**Required changes.**

1. Independently review every surviving non-bootstrap expectation against design v3 and v2.1
   public meaning. Re-author typed input rows, provider facts, transformation results, analysis
   rows, semantic responses, Delta/version/activation outcomes, security denials, resource
   terminals, and released-wire projections without importing target output or calling production
   expected-value code.
2. Delete the `bootstrap_model_semantics` category, mandatory WP01/comparator dependency, replay
   agreement, model digest, and old-binary output as acceptance inputs. A predecessor comparison may
   be retained only as optional labelled diagnostic evidence, excluded from every pass/fail DAG and
   removable without changing the verdict.
3. Issue a new append-only successor evidence transaction recording author/reviewer independence,
   source design clauses, decoded expected rows, negative cases, limitations, superseded evidence,
   and exact claim-to-oracle mapping. Preserve the old transaction as historical, not current.
4. For each accepted claim create a causal fault that changes the authoritative input, provider
   batch, transformation, plan/schema, coverage, proof history, version vector, authorization,
   resource, protocol, or output behavior and makes the owning oracle fail. Text-only/digest/count
   changes do not qualify unless the claim is itself structural identity.
5. Execute evidence through the production composition from WP37. Compare decoded typed/public
   values with explicit ordering/null/unknown/provenance semantics. Prove that removing comparator
   bytes and all predecessor executables leaves identical acceptance results.
6. Separate development readiness from release certification. Record every deferred platform,
   performance, scheduled, or deep-assurance limitation; no unsupported domain becomes a green
   capability.

**Legacy disposition and decommission.** Replaces L-44/L-45/L-49 evidence machinery while
preserving immutable released allocations, KATs, independently valid decisions, and historical
transactions. Producer-generated goldens, bootstrap expectations, mandatory comparator DAGs,
artifact-count acceptance, and predecessor agreement gates are deleted after the successor
transaction passes.

**Acceptance checks.**

Executable oracle: `production-evidence-input-integrity-check`
Governed criterion: `PC-WP38-INT`

Executable oracle: `first-principles-production-behavior-check`
Governed criterion: `PC-WP38-BEH`

Executable oracle: `causal-fault-discrimination-check`
Governed criterion: `PC-WP38-NEG`

Executable oracle: `production-evidence-recovery-operations-check`
Governed criterion: `PC-WP38-OPS`

**Oracle category fault contract.** `INT` mutates a frozen WP33 expectation/fixture/review/input
identity and must invalidate execution; `BEH` compares decoded production rows/public results to
the independently issued positive expectations; `NEG` executes every causal/rejection fixture and
proves comparator/predecessor absence cannot change the verdict; `OPS` runs restart, cache-loss,
resource, security, clean/incremental, and recovery evidence without re-authoring expectations.

Completion requires a reviewed successor transaction, decoded independent rows for every release
claim, a discriminating causal fault per claim family, zero bootstrap/model/comparator acceptance
edges, a successful run with predecessor evidence physically unavailable, and a proving commit.

### WP39 — Purge remaining generated authority, governance, tooling, and package residue

**Dependencies.** WP38.

**Target invariants.** I-40--I-41, I-48, I-50--I-51; P3, P18, P21, P25--P31, P36.

**Design and library references.** Design §§6--7; reconciled L-20--L-55 map in §6; SUITE
historical/release/governance boundaries; repository four-domain and package architecture in
`AGENTS.md`.

**Change surface / Preflight / Known Touch.** Regenerate the WP28 selector inventory from current
HEAD and inspect every matched, skipped, generated, ignored, vendored, hidden, build-script,
include, feature, bin, recipe, workflow, package-data, fixture, rule, and documentation routing
surface. Prove retained exclusions exactly. Known touch:
`contracts/generated/model/manifest.json`, `contracts/governance/relational-fabric-legacy-freeze.json`,
`contracts/governance/relational-fabric-legacy-selectors.json`,
`tooling/ci/relational_fabric_transition.py`, `rules/relational-fabric.yml`, `justfile`,
`Cargo.toml`, `codefabric-cpg-mcp/pyproject.toml`, `.github/workflows/ci.yml`,
`AGENTS.md`, and `docs/spec_index/README.md`.

**Required changes.**

1. Execute every deletion still open in the L-map and DB09--DB13 after re-proving its target
   consumer. Delete generated semantic registries, bundles, model/provider kind/schema/result/table/
   identity authorities, manifests/censuses/fingerprints, static query tables, current-status
   products, importer/comparator caches, and their loaders.
2. Replace predecessor model/artifact/census/error/property/gate scripts and textual/count detectors
   with the smallest intent-level v3 behavior, causal, relational, exact-version, and zero-state
   gates. Retain structural rules only for real build/process/wire/security boundaries or labelled
   residue proof.
3. Remove retired Cargo features/binaries/dependencies/build edges, Python modules/package data/
   dependencies, Just recipes, workflow steps, scripts, rules/rule fixtures, fuzz targets/corpora,
   snapshots/goldens, and tests once no retained target or released compatibility consumer uses
   them. Re-lock each affected domain and prove the exact four-root boundary.
4. Update current documentation/navigation to v2.1 programmatic authority and remove live guidance
   that instructs agents or operators to generate/replay/bootstrap model authority. Preserve v2.0,
   v1.3, prior designs/plans/states/reviews, released allocations/tombstones, and accepted historical
   evidence as explicitly non-live history.
5. Require multi-engine negative proof: compiler/build/package/recipe enumeration, ast-grep
   construction/call rules, hidden-aware fixed-string and semantic searches, generated/include
   tracing, dependency/feature trees, installed wheel/sdist census, and all skipped-file accounting.
   A path allowlist alone cannot prove absence.
6. Remove the transition selectors and freeze products themselves only after a newly generated
   inventory proves no matched live residue and the permanent v3 negative oracles cover every
   forbidden authority class. Keep only the compact L-disposition ledger and historical exclusions.

**Legacy disposition and decommission.** Completes DB09--DB13 and L-20--L-42/L-45--L-51/L-54--
L-55 deletions not already closed. L-43/L-44/L-52/L-53 remain preserved or reshaped as the map
states. No compatibility module, dormant feature, deprecated recipe, ignored package file, or
test-only legacy implementation survives.

**Acceptance checks.**

Executable oracle: `legacy-disposition-artifact-integrity-check`
Governed criterion: `PC-WP39-INT`

Executable oracle: `retained-target-post-purge-behavior-check`
Governed criterion: `PC-WP39-BEH`

Executable oracle: `remaining-legacy-zero-state-check`
Governed criterion: `PC-WP39-NEG`

Executable oracle: `post-purge-package-build-operations-check`
Governed criterion: `PC-WP39-OPS`

**Oracle category fault contract.** `INT` omits an L/DB disposition, target consumer, history
exclusion, or deletion selector; `BEH` reruns retained production behavior after physical purge;
`NEG` reintroduces any forbidden file/symbol/feature/recipe/rule/workflow/package/dependency or
unclassified skip and must fail; `OPS` rebuilds/relocks/packages all four roots and retires
transition selectors without restoring legacy.

Completion requires zero unclassified live matches, no skipped candidate, clean generated/package/
feature/recipe/dependency inventories, exactly justified historical exclusions, retired transition
selectors, all retained target consumers still passing, and a proving commit.

### WP40 — Re-execute the post-purge release evidence matrix

**Dependencies.** WP39.

**Target invariants.** I-40--I-51; P1--P36.

**Design and library references.** Design §§7--10; all v2.1 masters; DataFusion/Arrow and delta-rs
alignment behavioral, transaction, recovery, observability, security, performance, and test
patterns; all prior packet criteria.

**Change surface / Preflight / Known Touch.** Reconcile the proving-commit chain, exact dependency
graphs, current suite/plan/state, evidence transaction, released contracts, golden sources,
durability ledger, all packet recipes, performance baselines, and platform limitations after the
purge. Known touch: `contracts/acceptance/relational-fabric-v3/release-evidence.json`,
`tooling/ci/relational_fabric_release.py`, `tooling/ci/test_relational_fabric_release.py`,
`tests/integration/data_fabric_upgrade.rs`, and `justfile`.

**Required changes.**

1. Re-run every semantic provider, transformation, analysis, query, lifecycle, Delta, activation,
   recovery, authorization, resource, public-wire, and legacy-absence claim against the post-purge
   tree. Record exact commands, selected tests, inputs, environment, elapsed/resource observations,
   outputs, limitations, and proving HEAD in one immutable release-evidence record.
2. Prove clean reconstruction from source plus exact released inputs and exact Delta histories, and
   prove incremental execution reaches identical typed rows, unknowns, provenance, exact vectors,
   activation head, and public outputs for the golden scenarios. Vary partitions, batch boundaries,
   restarts, CDF gaps, and cache state.
3. Exercise authorization/catalog isolation, credentials/network denial, unsafe source/build paths,
   resource exhaustion, flow control, cancellation, partial/corrupt providers, uncertain Delta
   commits, stale fences, split activation chains, retention guards, and adapter protocol errors.
4. Prove full provenance closure from every served row to exact source/provider/input/
   transformation/application/table/release/proof/expectation identities. Missing or cyclic closure
   rejects activation or capability; it never becomes a release limitation silently.
5. Measure declared performance/resource envelopes on the supported local-workstation deployment.
   Report regression and uncertainty honestly; do not convert a missing benchmark/platform into a
   passing release claim.
6. Verify comparator bytes, v2 executables, legacy selectors, and all invalidated state can be absent
   without changing the evidence matrix. This packet may create release records and focused test/
   gate code, but must not restore production legacy.

**Legacy disposition and decommission.** Supplies positive post-purge proof for all L dispositions
and closes DB14's live-read detachment. Immutable historical artifacts remain outside runtime/build/
package/acceptance inputs. Any newly discovered legacy candidate reopens WP39 rather than gaining an
exception here.

**Acceptance checks.**

Executable oracle: `release-evidence-record-integrity-check`
Governed criterion: `PC-WP40-INT`

Executable oracle: `release-evidence-matrix-v3-check`
Governed criterion: `PC-WP40-BEH`

Executable oracle: `security-resource-release-rejection-check`
Governed criterion: `PC-WP40-NEG`

Executable oracle: `clean-incremental-recovery-performance-check`
Governed criterion: `PC-WP40-OPS`

**Oracle category fault contract.** `INT` corrupts the immutable release record, proving HEAD,
input/pin/environment identity, or provenance link; `BEH` reruns the complete post-purge semantic
matrix against frozen WP33 expectations; `NEG` injects authorization, credential, unsafe-source,
provider, protocol, retention, fence, and provenance failures and requires rejection; `OPS` varies
clean/incremental, partition, batch, restart, CDF-gap, cache, resource, and performance conditions.

Completion requires a trusted post-purge HEAD, independently decoded expectations, all causal
faults, clean/incremental equality, security/resource/recovery failure matrix, complete provenance,
declared limitations, no predecessor dependency, and a proving commit.

### WP41 — Execute the durable fenced forward-only production cutover

**Dependencies.** WP40.

**Target invariants.** I-43, I-45--I-46, I-49--I-51; P3, P10--P11, P16, P18--P20,
P23--P25, P32--P34, P36.

**Design and library references.** Design §§3.7, 3.9, 6.3 and D-45/D-47; LIFE fencing,
activation, admission, restart, rollback, and recovery invariants; Delta transaction/state identity
patterns.

**Change surface / Preflight / Known Touch.** Inventory deployment identity, daemon binary/version,
service/launch configuration, writer lease, UDS binding, activation chain, cutover journal, rollback
command, restart/reboot recovery, legacy executable/package availability, and operator status. Known
touch: `src/fabric/activation_command_effect.rs`, `src/fabric/rollback_command_effect.rs`,
`src/fabric/administration_command_effect.rs`, `src/daemon.rs`,
`contracts/governance/relational-fabric-cutover-v3.json`, and `scripts/bootstrap.sh`.

**Required changes.**

1. Implement a durable idempotent state machine with explicit identities and evidence:
   `TARGET_PROVED -> PREDECESSOR_FENCED -> TARGET_SERVING -> TARGET_MUTATING -> COMPLETE`, plus
   failure/unknown reconciliation. State advance is one fenced command/Delta activation fact, never
   a mutable status flag or process-local receipt.
2. Before target serving, mechanically deny the exact predecessor binary/package from binding the
   workspace UDS, acquiring the writer lease, serving, or mutating across process restart and host
   reboot. Prove the denial with the actual deployment supervisor/configuration; a source search or
   stopped process is insufficient.
3. Between `PREDECESSOR_FENCED` and the first target-format mutation, rollback may discard target
   staging and re-enable only the exact frozen predecessor under an explicit command. After
   `TARGET_MUTATING`, rollback is forward repair through target commands/activation; the predecessor
   can never regain authority.
4. Reconcile crash/timeout/unknown outcome at every transition from durable fence, command,
   transaction, activation, and supervisor evidence. Duplicate execution converges; contradictory
   evidence closes admission and requires an administrative repair command.
5. Prove one serving authority, one writer, one UDS owner, one current activation head, and one
   programmatic epoch before and after restart/reboot. Delete any temporary bridge/fallback toggle
   once `COMPLETE` is read back.
6. Emit operator-readable status derived from durable events and observed deployment state, with
   exact limitations and remediation. It cannot author the cutover result.

**Legacy disposition and decommission.** Completes the authority-revocation part of DB14. It does
not require a predecessor comparator or preserve a live old binary after completion. Immutable
release/history bytes may remain archived, but no service/package/feature/command can execute them.

**Acceptance checks.**

Executable oracle: `cutover-event-contract-integrity-check`
Governed criterion: `PC-WP41-INT`

Executable oracle: `fenced-authority-cutover-v3-check`
Governed criterion: `PC-WP41-BEH`

Executable oracle: `predecessor-restart-revocation-check`
Governed criterion: `PC-WP41-NEG`

Executable oracle: `unknown-cutover-reconciliation-check`
Governed criterion: `PC-WP41-OPS`

**Oracle category fault contract.** `INT` corrupts a transition identity, fence, command,
activation, or supervisor observation; `BEH` executes the durable idempotent cutover and permitted
pre-mutation rollback; `NEG` attempts predecessor bind/serve/write or post-mutation revival across
restart/reboot and must fail; `OPS` injects crash/timeout/unknown at every transition and requires
durable convergence or admission-closed repair.

Completion requires transition/readback proof, injected crash/unknown at every edge, actual
restart/reboot denial of predecessor bind/serve/write, pre-mutation rollback proof, post-mutation
forward-only denial, temporary-control zero state, and a proving commit.

### WP42 — Certify the complete successor at one trusted HEAD

**Dependencies.** WP41.

**Target invariants.** I-40--I-51; P1--P36; every governed criterion PC-WP28-BEH through
PC-WP41-OPS.

**Design and library references.** Accepted design v3, all eight v2.1 masters, both selected
library alignment manuals, every packet proving record, M01--M06, DB09--DB14, and the final gate
matrix in §8.

**Change surface / Preflight / Known Touch.** Freeze candidate HEAD; verify clean tracked/untracked
state or explicitly classified user artifacts, ancestral proving commits, declared-input freshness,
active plan/state identity, suite chain, all ledger rows, all oracle definitions/selectors, release
evidence, cutover completion, four build roots, locks, features, packages, and historical exclusions.
Known touch: `docs/plans/state/codefabric-execution-proved-relational-data-fabric_v3_state.json`,
`contracts/acceptance/relational-fabric-v3/release-evidence.json`, `justfile`, and
`tooling/ci/packet_oracles.py`.

**Required changes.**

1. Re-run all 52 predecessor packet oracles from WP28--WP41 through the exact packet selector and
   verify each still maps to one governed criterion, resolves to substantive code, selects nonzero
   tests, demonstrates its negative fixture, and records the same trusted HEAD.
2. Close M01--M06 and DB09--DB14 only from current evidence. Validate all 27 v2 outcome mappings,
   all 36 L dispositions, the complete authority/durability/cache ledgers, no stale selector or
   unexplained exclusion, and no complete state entry without an ancestral proving commit.
3. Run final semantic, Delta/recovery, public compatibility, four-domain Tier A, exact graph,
   feature isolation, package, governance, generated-contract, documentation, and legacy-zero-state
   gates from a fresh supported shell. Run conditional/deep gates required by touched risk and list
   anything deferred; deferred release-mandatory evidence prevents completion.
4. Reconstruct from a clean process/target-independent source checkout plus released inputs and exact
   durable histories. Verify the exact production binary/package starts, recovers, serves, cancels,
   restarts, and remains sole authority without legacy archives or caches.
5. Generate a final immutable certification record containing HEAD, environment, input/pin/lock
   identities, command/output digests, elapsed/resource summaries, limitations, state/proving-commit
   derivation, and independent reviewer disposition. The record reports evidence; it cannot mark its
   own state complete.
6. After independent implementation review returns no blocking finding, record WP42, DB14, and M06
   proving commits/status. Preserve plan/design/review/history; do not rewrite this plan.

**Legacy disposition and decommission.** Certifies, but does not substitute for, physical deletion.
Any live L-20--L-55 violation, old authority route, stale package/feature/recipe, historical runtime
reader, or missing proof reopens its owning packet. No final-gate allowlist may waive target scope.

**Acceptance checks.**

Executable oracle: `successor-provenance-state-integrity-check`
Governed criterion: `PC-WP42-INT`

Executable oracle: `relational-fabric-v3-certification`
Governed criterion: `PC-WP42-BEH`

Executable oracle: `successor-final-zero-state-check`
Governed criterion: `PC-WP42-NEG`

Executable oracle: `successor-four-domain-release-check`
Governed criterion: `PC-WP42-OPS`

**Oracle category fault contract.** `INT` corrupts declared inputs, proving ancestry, permitted
state fields, packet evidence, or final certification provenance; `BEH` certifies the complete
successor at one trusted HEAD; `NEG` reintroduces any legacy/live-history route, stale selector,
missing fault, or release-mandatory deferral and must fail; `OPS` performs clean four-domain build,
reconstruction, start/serve/cancel/restart, feature isolation, and risk-triggered release gates.

Completion requires all 56 v3 packet oracles green at one trusted HEAD, M01--M06 and DB09--DB14
closed with ancestral proving commits, independent review acceptance, no release-mandatory deferral,
and a final WP42 proving commit.

## 5. Milestones

Milestones are derived release barriers, not packet substitutes. Their proving commit is the latest
ancestral packet commit at which every exit condition below is rerun and accepted.

### M01 — Successor authority and scope are closed

**Dependencies.** WP28.

**Exit.** The v2.1 suite is sole current authority; v2.0/v1.3 and v2 plan/state are historical or
invalidated; 27 prior outcomes and 36 L dispositions are complete; authority, durability, cache,
selector, history-exclusion, and oracle ledgers are schema-valid; all WP28 gates and faults pass.

### M02 — Production is programmatic and rejected authority is absent

**Dependencies.** WP29 and WP30.

**Exit.** The real daemon cold-starts one programmatic workspace/runtime and no default/bootstrap
backend; exact typed inputs causally affect the plan/result; model/replay/importer/generated-schema/
old-epoch/model-migration/model-pinned routes and attached files/features/recipes/package edges are
physically absent; DB09 closes.

### M03 — DataFusion and Delta state closure are proved

**Dependencies.** WP31 and WP32.

**Exit.** Plan/provider-derived schema, five fixed-point observations, authorized child closure,
native extension rungs, bounded caches, complete durability classification, exact-version Delta
history, CDF/stats/transaction policy, one activation chain, and candidate-free recovery pass all
faults. No cache, `latest`, raw listing, or receipt can select semantic state.

### M04 — Exact provider-to-public production delivery is proved

**Dependencies.** WP34, WP35, WP36, and WP37.

**Exit.** Exact provider IPC/admission/trust, every derived producer or explicit remainder, all
eight semantic forms, graph rung selection, repository lifecycle, command/resource authority, UDS
delivery, and presentation-only FastMCP pass one real four-domain vertical plus negative cases.
DB10 and DB11 close.

### M05 — Independent release evidence and total targeted purge are accepted

**Dependencies.** WP38, WP39, and WP40.

**Exit.** A reviewed first-principles evidence transaction, one causal fault per claim family,
post-purge clean/incremental equality, security/resource/recovery/provenance evidence, complete
L-map proof, zero unclassified legacy, clean package/feature/recipe/dependency inventories, and no
predecessor/comparator dependency pass. DB12 and DB13 close and DB14 is detached from live reads.

### M06 — Fenced cutover and final certification are complete

**Dependencies.** WP41 and WP42.

**Exit.** The predecessor is mechanically unable to bind/serve/write across restart/reboot; target
mutation is forward-only; all 56 packet oracles, final gates, exact reconstruction, four build
domains, state/proving-commit derivation, independent implementation review, and declared release
evidence pass at one trusted HEAD. DB14 closes.

## 6. Successor L-20--L-55 disposition map

The v2 disposition is preserved unless the `V3 treatment` cell says **targeted change**. A changed
replacement mechanism does not silently change the retained product outcome. `Positive proof`
proves the target consumer; `Negative proof` proves the predecessor cannot remain a second answer.

| ID | V3 treatment relative to v2 | Named reason and retained outcome | Replacement / cutover / deletion | Positive proof | Negative proof |
|---|---|---|---|---|---|
| L-20 | **unchanged: delete** | The model-compiler bin/feature authored generated current state; foreign Protobuf generation is independently retained. | Programmatic production composition in WP29; delete `src/bin/codefabric_model/**`, model feature/bin/readers in WP30. | `programmatic-production-composition-check` | `bootstrap-model-file-zero-state-check` |
| L-21 | **unchanged: delete** | DesiredTree/model sync/repro/family/release tooling duplicates self-observed catalogs and first-principles proof. | WP28 ledgers replace transition accounting; WP30 deletes model tooling, WP39 removes residual jobs/recipes. | `successor-authority-scope-check` | `legacy-feature-recipe-package-check` |
| L-22 | **targeted change: replace live use, preserve immutable history** | Static registries are not a migration prerequisite; only released/non-derivable decisions remain explicit typed inputs or Class 1 history. | Admit reviewed typed inputs in WP29; delete every build/runtime/tool reader in WP30; retain exact historical paths under L-44/L-52. | `programmatic-schema-causality-check` | `bootstrap-model-consumer-cutover-check` |
| L-23 | **unchanged: delete** | Generated model/provider-kind/manifest products duplicate fixed-point observations, live catalog queries, and release evidence. | WP31/WP36 supply runtime observations/queries; WP30 removes model products and WP39 removes residual manifests. | `authorized-child-closure-check` | `generated-authority-zero-state-check` |
| L-24 | **unchanged: delete** | Generated encoders/domains/model/bundle/kind/registry/result/table arrays are parallel semantic authorities. | Retain only released low-level encoders as explicit inward primitives in WP30; plan/provider schemas and typed inputs replace the rest. | `plan-derived-schema-check` | `generated-authority-zero-state-check` |
| L-25 | **unchanged: reshape** | Released `.proto` authority and generated interop remain; committed stubs/descriptors are derivable compatibility products, not semantics. | WP34 validates control-only IPC; WP37 regenerates/compares real Rust/Python stubs and package contents; delete stale caches only. | `relation-ipc-v3-conformance-check` | `legacy-feature-recipe-package-check` |
| L-26 | **unchanged: delete** | Adapter fingerprints, model/schema aggregates, artifact indexes, and generated query tables duplicate daemon catalog/capability authority. | WP36/WP37 deliver live references/status through daemon; WP39 deletes package data/loaders. | `fastmcp-presentation-boundary-check` | `generated-authority-zero-state-check` |
| L-27 | **unchanged: replace, then delete** | Installed/resealed ontology bundle and candidate gate remain rejected package authority; accepted behavior stays. | WP29 programmatic epoch/activation replaces bundle selection; WP30 deletes ontology package/bundle/candidate modules and routes. | `programmatic-session-authority-check` | `bootstrap-model-file-zero-state-check` |
| L-28 | **unchanged: reshape; replacement mechanism targeted** | Relational semantics and native DataFusion lowering are reusable; hard-coded/generated/replay catalogs are not. | WP31 binds typed transformations; WP35 completes application producers; delete replay/census wrappers in WP30/WP39. | `datafusion-extension-contract-check` | `dual-epoch-symbol-zero-state-check` |
| L-29 | **unchanged: replace, then delete** | Schema/current registries are a second answer; truly static wire primitives remain inward. | Five catalog observations and plan/provider-derived `SchemaContract` in WP31; split retained primitives then delete registry modules in WP30. | `plan-derived-schema-check` | `bootstrap-model-file-zero-state-check` |
| L-30 | **unchanged: replace** | Procedural projection/cold payloads conflate raw provider facts and application meaning. | Exact raw batches in WP34 plus typed normalization/analysis transformations in WP31/WP35; delete old ingest/projection/JSON paths. | `exact-provider-batch-check` | `provider-admission-exclusivity-check` |
| L-31 | **unchanged: reshape** | Tree-sitter/Ruff process/API adapters and coordinates remain useful; mirrors/static kinds/opaque fields do not. | Direct exact API Arrow producers cut over in WP34; delete defensive mirrors and static/opaque payloads there. | `exact-provider-batch-check` | `provider-admission-exclusivity-check` |
| L-32 | **unchanged: reshape** | Pyrefly revision isolation, validation, backpressure, cancellation remain; opaque payload/module authority is incomplete. | Exact Query/TSP/resolver/selected Glean/LSP relation streams and affected-module invalidation in WP34; delete JSON/parallel routes. | `relation-ipc-v3-conformance-check` | `provider-trust-coverage-remainder-check` |
| L-33 | **unchanged: reshape** | Dated-nightly isolation/control remains; summaries/provider-local identity cannot satisfy exact facts. | Exact `rustc_public` plus narrow private enrichment and Arrow IPC in WP34; delete `OwnedMirItem`/debug/bypass routes. | `exact-provider-batch-check` | `provider-trust-coverage-remainder-check` |
| L-34 | **unchanged: reshape** | Job/sandbox/transport lifecycle remains; hard-coded inventory and claimed sandbox digest are not capability/trust proof. | Exclusive exact admission and untrusted compilation launcher in WP34; delete legacy overload/uncontained paths. | `provider-trust-coverage-remainder-check` | `provider-admission-exclusivity-check` |
| L-35 | **unchanged: reshape; replacement mechanism targeted** | Exact Delta providers, mutation spine, sessions, and streams remain; model-backed epoch/schema/current do not. | WP29 programmatic composition, WP31 schema/session/cache, WP32 exact Delta vector; delete old epoch/current/overlay owners in WP30/WP32. | `delta-exact-reconstruction-v3-check` | `dual-epoch-symbol-zero-state-check` |
| L-36 | **unchanged: replace** | Bespoke concatenate/take/row consolidation hides optimizer semantics and duplicates native plans. | Native anti-join/union/window/provider views or fully conforming extension in WP31; delete bespoke overlay/consolidation in WP31/WP32. | `datafusion-extension-contract-check` | `remaining-legacy-zero-state-check` |
| L-37 | **unchanged: replace; replacement mechanism targeted** | Mutable/duplicated snapshot manifests cannot select exact multi-table state. | Programmatic epoch plus activation exact root/version vector in WP29/WP32; retain public snapshot projection, delete internal authority. | `candidate-free-recovery-check` | `activation-receipt-nonauthority-check` |
| L-38 | **unchanged: replace** | SQLite may own queue/retry/lease progress but not semantic history/current. | Delta operation/activation histories and derived head in WP32/WP37; delete ontology candidate/package/current-pointer tables. | `durable-proof-history-check` | `remaining-legacy-zero-state-check` |
| L-39 | **unchanged: replace; replacement mechanism targeted** | Public forms/bounds remain; fixed crosswalks, model pins, package planning, and bypasses are parallel query authority. | Programmatic request transformations and authorized children in WP31/WP36; delete old planners/pins/registries there. | `semantic-request-program-check` | `semantic-query-authority-check` |
| L-40 | **unchanged: reshape** | Central daemon, source truth, invalidation, cancellation, fairness, and security remain valid. | One programmatic runtime/command/resource domain in WP29/WP37; delete direct durable mutations and predecessor lifecycle routes. | `lifecycle-production-vertical-check` | `resource-cancellation-v3-check` |
| L-41 | **unchanged: delete, with prior narrow kernel condition** | Persisted graph DTO/registry/index authority is invalid; only a proved bounded transient kernel may remain. | Highest DataFusion rung in WP31/WP35/WP36; canonical Arrow boundary for any irreducible kernel; delete indices/registries. | `graph-rung-conformance-v3-check` | `analysis-producer-closure-v3-check` |
| L-42 | **unchanged: reshape** | FastMCP topology and strict public models remain; packaged registries and Python semantic state do not. | Live daemon catalog/result resources in WP36/WP37; delete fingerprints/registries/state/package data in WP37/WP39. | `fastmcp-presentation-boundary-check` | `semantic-delivery-v3-check` |
| L-43 | **unchanged: preserve** | Released RPC/request/response/status/source/admin meaning is Class 1 compatibility authority, not internal static semantics. | Regenerate/compare and version/tombstone through WP34/WP37/WP38; no deletion unless compatibility policy independently authorizes it. | `released-wire-expectation-check` | `semantic-query-authority-check` |
| L-44 | **unchanged: preserve, narrow live use** | Released allocations, canonicalization KATs, independent expectations, and accepted decisions remain; predecessor-derived answers do not. | WP38 independently re-reviews expectations; WP40 proves no production/acceptance reader needs invalid history. | `released-wire-expectation-check` | `comparator-independence-check` |
| L-45 | **targeted change: replace** | Producer goldens/counts and mandatory comparator cannot accept programmatic authority; independent rows/faults retain semantic claims. | WP38 issues successor evidence and deletes bootstrap/comparator DAG; WP39 removes residual machinery. | `first-principles-evidence-check` | `comparator-independence-check` |
| L-46 | **unchanged: delete** | V1 principle/detector/count mapping is obsolete textual governance; doctrine prose stays historical. | WP28 v3 relational/causal ledger, WP38 faults, WP39 delete detectors/baselines/alignment scripts. | `successor-governance-negative-fixture-check` | `governance-selector-retirement-check` |
| L-47 | **unchanged: replace** | Generated-authority model/artifact/census/property gates prove the wrong object. | Packet behavior/causal/relational/exact-version/zero-state gates WP28--WP40; remove predecessor scripts/rules/recipes in WP39. | `release-evidence-matrix-v3-check` | `governance-selector-retirement-check` |
| L-48 | **unchanged: reshape** | Intent-level Just/CI, four-domain isolation, feature and stable-graph proof remain; retired jobs/edges do not. | Add v3 recipes/selectors across packets; remove retired features/jobs/deps/workflows and re-lock in WP39. | `successor-four-domain-release-check` | `legacy-feature-recipe-package-check` |
| L-49 | **unchanged: reshape; expectation mechanism targeted** | Behavioral/provider/protocol/KAT tests remain; digest/count/replay/static-text acceptance does not. | Rebind retained tests to v3 consumers; author typed/fault/recovery/resource/delivery proof in WP31--WP40; delete obsolete tests. | `causal-fault-discrimination-check` | `comparator-independence-check` |
| L-50 | **unchanged: replace, governance cutover already authored** | High-level behavior remains; only one current suite may describe programmatic authority. | V2.1 eight-master suite selected/validated in WP28; preserve v2.0/v1.3 as chained history. | `successor-authority-scope-check` | `successor-governance-negative-fixture-check` |
| L-51 | **unchanged: replace** | Spec index is disposable navigation and never runtime/build authority. | Route to sole-current v2.1 and generate/validate from masters where practical in WP28/WP39; delete stale current claims. | `successor-disposition-coverage-check` | `governance-selector-retirement-check` |
| L-52 | **unchanged: preserve, detach live reads** | Designs/plans/states/reviews/released census are immutable history and allocation/tombstone evidence. | WP39 marks/routes history; WP40 proves runtime/build/package/acceptance independence; never delete for runtime cleanup. | `release-evidence-matrix-v3-check` | `remaining-legacy-zero-state-check` |
| L-53 | **unchanged: preserve/reshape exact graphs** | Locks/toolchains/Protobuf tooling/four roots remain justified; unused edges must still disappear. | Re-lock exact target dependencies in WP39; final stable/four-domain proof WP42. | `successor-four-domain-release-check` | `legacy-feature-recipe-package-check` |
| L-54 | **targeted change: remove live use early, retain immutable history only** | Dirty ontology/schema epoch artifacts are not migration/replay prerequisites; overwriting user history remains forbidden. | Re-express reviewed non-derivable choices as typed inputs in WP29; delete live readers in WP30; retain only exact historical artifacts under L-52. | `programmatic-schema-causality-check` | `bootstrap-model-consumer-cutover-check` |
| L-55 | **unchanged: replace; replacement mechanism targeted** | Minimal released identity encoders remain intrinsic; generated/current recipe arrays and runtime registries are parallel authority. | Explicit typed identity inputs and programmatic bindings cut over in WP29/WP31; delete arrays/registries in WP30/WP39. | `programmatic-schema-causality-check` | `generated-authority-zero-state-check` |

Every row is also present in the executable WP28 ledger with exact current selectors, target
consumer symbols, cutover/deletion packet, history exclusions, and fault fixture. A change to any
row is a plan deviation and replan input, not an executor-local exception.

## 7. Decommission batches

Each batch is dependency-closed and completes only after its positive target consumer and negative
zero-state evidence pass at one HEAD. Deletion includes files, symbols, constructors, features,
recipes, rules, workflow jobs, fixtures, generated products, package data, dependency edges, and
installed artifacts. Historical exclusions must be exact and non-live.

### DB09 — Delete bootstrap, model, importer, generated-schema, and dual-epoch authority

**Dependencies.** WP29 positive composition; executed and closed by WP30.

**Disposition.** Complete L-20--L-24, L-27, L-29, model-owned L-35/L-37/L-39, L-54, and L-55
runtime deletion. Extract proven released identity/wire/schema-phase/runtime primitives first;
delete relational model replay/release/schema, model compiler/importer/tooling/live inputs,
bootstrap schema/tables, ontology bundle/candidate authority, old epoch types/handles/pins,
model-to-schema projection, model migration command/effect/admin route, legacy admission/query
wrappers, generated semantic arrays, and their tests/features/recipes/package edges.

**Exit.** `programmatic-production-composition-check`, `bootstrap-model-consumer-cutover-check`,
`bootstrap-model-file-zero-state-check`, `dual-epoch-symbol-zero-state-check`, and
`model-migration-route-denial-check` pass; target consumers compile and run; no unclassified live
match or skip remains; released/historical exclusions cannot be imported by runtime/build/package.

### DB10 — Delete legacy provider, projection, analysis, graph, and query authority

**Dependencies.** WP31 and WP32; executed across WP34--WP36 and closed at M04.

**Disposition.** Complete L-28, L-30--L-36, L-39, and L-41 provider/analysis/query deletion.
Retain exact API/process isolation, Arrow IPC, lifecycle ports, canonical IDs, and the smallest
proved transient graph kernel. Delete opaque/cold JSON, DTO mirrors, static kind/capability/
producer/query registries, summaries/debug substitutes, old admission, procedural derivations,
persisted graph indices, replay compilers, fixed query crosswalks/model pins, parent-session leaks,
SQL/name bypasses, and parallel tests/fixtures.

**Exit.** WP34--WP36 oracles pass, every family has exact coverage and one producer or explicit
remainder, all eight forms serve through authorized children, and multi-engine zero-state proof
finds no opaque payload, old admission/producer/planner, persisted index, or uncontained launcher.

### DB11 — Delete legacy storage, snapshot, mutation, lifecycle, serving, and adapter routes

**Dependencies.** WP32 and WP36; executed and closed by WP37.

**Disposition.** Complete L-25--L-26, L-35--L-40, and L-42 serving/runtime deletion while
preserving released wire meaning and bounded source/lifecycle/resource mechanisms. Delete bespoke
overlay/consolidation, raw Parquet/listing Delta authority, mutable snapshot/current manifests,
SQLite semantic current, direct durable mutation/publication, default/empty/debug query backends,
candidate-required recovery, unbounded/authority caches, in-process serving shortcuts, adapter
registries/fingerprints/package state, and Python semantic processing.

**Exit.** WP32/WP37 oracles pass one real source-to-FastMCP vertical plus restart/cancellation/
resource/protocol faults; exact version vector and activation head are unique; no direct mutation,
default backend, raw listing, snapshot pointer, Python semantic owner, or packaged registry remains.

### DB12 — Delete predecessor evidence and generated governance authority

**Dependencies.** WP38; executed and closed by WP39.

**Disposition.** Complete L-23--L-24, L-26, L-44--L-47, L-49--L-51, and residual L-55 cleanup.
Preserve immutable independently valid KAT/allocation/decision/history records. Delete bootstrap
expectations, mandatory comparator DAG, producer-generated goldens, digest/count acceptance,
generated model/schema/identity/result/table/package authorities, v1 detector registries and count
mappings, obsolete model/artifact/census/error/property/gate scripts/rules/fixtures, stale current
suite/index/agent routing, and transitional freeze/selectors once permanent negative gates cover
their classes.

**Exit.** WP38/WP39 first-principles, comparator-independence, generated-authority, governance, and
zero-state oracles pass; a causal fault exists for every claim family; v2.1 is sole current; the
acceptance verdict is identical with predecessor/comparator/history bytes unavailable.

### DB13 — Delete retired features, binaries, dependencies, recipes, workflows, and package edges

**Dependencies.** DB09, DB10, DB11, and DB12; executed and closed by WP39.

**Disposition.** Complete L-20/L-25/L-48/L-53 packaging and graph decisions. Remove every retired
Cargo bin/feature/dependency/build edge, Python module/dependency/package-data entry, generated stub
cache not justified by foreign builds, Just recipe, workflow step, script, rule, fuzz target/corpus,
snapshot, fixture, and installed artifact after reachability proves zero retained consumers.
Regenerate locks and descriptors only through the surviving exact toolchain.

**Exit.** `legacy-feature-recipe-package-check`, exact Cargo/uv/package inventories,
`stable-graph-check`, feature isolation, Protobuf reproduction/interop, recipe/workflow census, clean
build/package inspection, and hidden/skipped-file accounting pass across all four roots.

### DB14 — Detach immutable history, revoke predecessor authority, and certify total purge

**Dependencies.** DB09--DB13 and WP40; executed across WP40--WP42.

**Disposition.** Complete L-43--L-44/L-52--L-54 live-read separation without deleting released
contracts, allocations/tombstones, accepted KATs/decisions, prior suites/designs/plans/states/
reviews, or exact toolchain history. Remove any runtime/build/package/acceptance link to them except
explicit current compatibility validation. Revoke predecessor executable bind/serve/write authority,
delete temporary cutover controls after readback, and retain no live comparator/archive reader.

**Exit.** Post-purge release evidence passes with history/comparator/old executable unavailable;
WP41 proves predecessor restart/reboot revocation and forward-only target mutation; WP42 proves all
56 oracles, milestones, batches, exact history exclusions, clean reconstruction, and independent
review at one trusted HEAD.

## 8. Final gate matrix

Packet oracles are mandatory in addition to, not aliases for, the repository gates below. A final
recipe may aggregate commands but must preserve individual exit codes, selected-test counts,
structured evidence, and negative fixtures. `artifacts-check` and `plan-status` prove governance and
commit/input trust; neither proves behavior.

| Gate family | Exact commands at WP42 | What must be true |
|---|---|---|
| Artifact and suite authority | `just authoritative-design-conformance-check`; `just artifacts-check`; `just plan-status`; `just plan-dependency-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v3_2026-08-30.md` | v2.1 sole-current chain, declared inputs fresh or accepted evolution, schema-valid state, ancestral proving commits, closed DAG |
| Packet proof | `just successor-all-packet-oracles-check`; each `just packet-oracle-check WP28` through `WP42` | exactly four unique substantive oracles/criteria per packet, nonzero selection, demonstrated negative fixture, same trusted HEAD |
| Programmatic authority and schema | `just programmatic-production-composition-check`; `just programmatic-session-authority-check`; `just plan-derived-schema-check`; `just authorized-child-closure-check` | exact inputs/transformations/plan-derived schema/fixed-point observations/child closure, no bootstrap/model/cache second answer |
| DataFusion execution and caches | `just datafusion-plan-schema-cache-check`; `just datafusion-extension-contract-check`; `just graph-rung-conformance-v3-check` | highest viable rung, complete extension contracts, bounded exact-key caches, fresh physical plan/result, deterministic resource-aware execution |
| Delta durability and recovery | `just durable-proof-history-check`; `just delta-exact-reconstruction-v3-check`; `just candidate-free-recovery-check`; `just activation-receipt-nonauthority-check` | complete durable ledger, exact root/version vector, provider/CDF/stats/transaction policy, unique activation head, cache-independent recovery |
| Provider and analysis | `just exact-provider-batch-check`; `just relation-ipc-v3-conformance-check`; `just provider-trust-coverage-remainder-check`; `just analysis-producer-closure-v3-check` | exact provider APIs and Arrow IPC, closed coverage/remainders, exclusive trust route, one application producer or explicit unsupported remainder |
| Query, lifecycle, and public delivery | `just semantic-request-program-check`; `just semantic-query-authority-check`; `just lifecycle-production-vertical-check`; `just semantic-delivery-v3-check`; `just fastmcp-presentation-boundary-check` | all eight forms, authorized child execution, one command/resource/epoch route, real UDS/FastMCP presentation-only delivery |
| Independent release evidence | `just first-principles-evidence-check`; `just causal-fault-discrimination-check`; `just comparator-independence-check`; `just release-evidence-matrix-v3-check`; `just clean-incremental-equivalence-v3-check`; `just provenance-closure-release-check` | independently authored decoded rows, causal faults, no comparator dependency, clean/incremental equality, full provenance and limitations |
| Cutover and zero state | `just remaining-legacy-zero-state-check`; `just generated-authority-zero-state-check`; `just legacy-feature-recipe-package-check`; `just fenced-authority-cutover-v3-check`; `just predecessor-restart-revocation-check`; `just post-mutation-forward-only-check` | every L/DB deletion physical, clean package/graph/recipe inventory, old binary cannot bind/serve/write, target mutation forward-only |
| Stable root | `just root-fmt`; `just root-check`; `just root-clippy`; `just root-test-rust`; `just root-doctest`; `just features-no-default`; `just features-each`; `just stable-graph-check` | formatting, compiler/lints, ordinary tests plus doctests, feature isolation, exact dependency universe |
| Extractor | `just extractor-fmt`; `just extractor-check`; `just extractor-test`; `just extractor-identity`; `just semantic-sandbox-host-matrix-check` | dated-nightly exact identity/API/private seam, contained trust/resource behavior, supported-host coverage |
| Pyrefly sidecar | `just sidecar-fmt`; `just sidecar-check`; `just sidecar-test`; `just sidecar-policy` | exact pinned sidecar graph and relation/protocol/policy behavior |
| Python adapter and wire | `just adapter-lint`; `just adapter-type`; `just adapter-test`; `just adapter-stdio-test`; `just adapter-wheel-test`; `just provider-protocol-check` | strict public models, real stubs/UDS protocol, STDOUT discipline, package census, no semantic state |
| Repository aggregate and conditional risk | `just ci-pr`; plus `just coverage`, `just miri-seeds`, `just mutants-file <touched-path>`, `just fuzz <touched-target>`, `just udeps`, `just msrv`, and performance recipes when triggered | Tier A passes; every risk-triggered Tier B/C gate is run or explicitly documented as non-release and independently accepted |
| Final certification | `just relational-fabric-v3-certification`; `just successor-four-domain-release-check`; `just successor-provenance-state-closure-check` | one trusted HEAD, exact environment/input/output record, M01--M06 and DB09--DB14 complete, independent implementation review accepted |

No final gate may use an old plan packet, model/bootstrap recipe, predecessor comparison, file count,
digest agreement, or state label as semantic acceptance. A renamed implementation gate must update
this immutable plan through governed input evolution or a successor plan; it cannot be silently
substituted.

## 9. Execution order, parallelism, and overlap control

The dependency spine is:

```text
WP28 -> WP29 -> WP30 -> {WP31, WP32}
WP31 + WP32 -> WP34 -> WP35 -> WP36
WP29 + WP30 + WP31 + WP32 + WP34 + WP35 + WP36 -> WP37
WP37 -> WP38 -> WP39 -> WP40 -> WP41 -> WP42
```

Only WP31 and WP32 are intentionally unordered. Their known-touch sets are disjoint at planning
time: WP31 owns DataFusion schema/catalog/child/cache modules; WP32 owns Delta history/activation/
recovery modules. If impact discovery finds a shared file, generated contract, public type, or
recipe, either serialize the packets or add an explicit phase/resource overlap disposition before
editing. WP34 inventory may begin while WP31/WP32 run, but implementation and completion wait for
both. No other file disjointness permits dependency bypass.

Execution status may expose only packets whose declared dependencies are complete and trusted.
Milestones and DBs close in this order:

```text
M01
  -> M02 / DB09
  -> M03
  -> M04 / DB10 / DB11
  -> M05 / DB12 / DB13 / live-read portion of DB14
  -> M06 / DB14
```

## 10. Packet evidence, commits, and state discipline

For each packet, the executor must:

1. capture pre-edit HEAD, status digest, declared-input freshness, exact dependency/pin graph, and
   current impact inventory;
2. record changed/added/deleted/unexpected paths and reconcile them against known touch before the
   first semantic edit;
3. implement target consumers and attached deletion as one dependency-closed change;
4. run the four packet oracles, their negative fixtures, required lower gates, and targeted
   compiler/package proof;
5. inspect the diff and make one packet proving commit whose tree contains all evidence-producing
   code and fixtures;
6. rerun the packet selector at that commit, then record commit, outputs, deviations, failures, and
   limitations in state; and
7. rerun affected descendant/aggregate gates after any later overlapping change. A failing current
   oracle reopens the packet regardless of recorded status.

Uncommitted code is progress, not completion. A proving commit must exist and be an ancestor of the
current certification HEAD. Accepted input evolution records provenance and review; it cannot
retroactively bless behavior or bypass a design/replan trigger.

## 11. Replan triggers and risk controls

Stop dependent execution and create a reviewed deviation/replan when any of these occurs:

- a v2.1 master, accepted design decision, doctrine principle, released public meaning, exact
  dependency pin, or plan declared input changes;
- a required provider fact is unavailable from the exact pinned public/private API, or a planned
  provider/app authority assignment is false;
- DataFusion 55 cannot express a required transformation at a rung that preserves optimizer,
  schema, resource, cancellation, and child-authorization contracts;
- a Delta/table intermediate marked transient is required after process loss for restart, audit,
  incrementality, provenance, or proof, or an item marked durable has no retention/reconstruction
  policy;
- multi-host writers, remote coordination, distributed fencing, new storage backend semantics, or a
  new process/Cargo/package boundary becomes required;
- a released client actually requires removed internal model/bootstrap behavior rather than only
  released protocol meaning;
- current-tree impact reveals an L surface without a disposition, a retained primitive without a
  target consumer, a historical exclusion with a live reader, or a deletion that would remove an
  unrelated v2 outcome;
- an oracle cannot distinguish a causal fault, selects zero tests, depends on target-generated
  expected output, or requires a predecessor comparator to pass;
- a required final platform/security/performance/recovery gate cannot run or fails at the candidate
  HEAD; or
- rollback after target-format mutation would require restoring the predecessor rather than
  forward repair.

Known risks are incomplete dirty-tree composition, exact-provider/private-API drift, custom
DataFusion node incompleteness, Delta uncertain-commit/retention mistakes, catalog leakage through
prebound views, cache-key omission, evidence circularity, and deletion collateral. The controls are
respectively per-packet reconciliation, exact compile/API tests, native-first rung proof, exact
transaction/history oracles, recursive child closure, typed full keys plus causal collision tests,
independent evidence/faults, and the complete outcome/L ledgers with positive-before-negative proof.

## 12. Activation and completion boundary

The approved plan is ready for a separate activation transaction only when:

1. all declared inputs still match;
2. v2 state remains invalidated with no current packet;
3. the v2.1 conformance check proves exactly eight current masters and their complete predecessor
   chains;
4. structural validation proves WP28--WP42, M01--M06, DB09--DB14, an acyclic dependency graph,
   exactly four globally unique oracle/criterion pairs per packet, and exactly 36 L rows;
5. the future state path does not exist and the active pointer has not been changed out of band; and
6. an independent plan audit accepts executability, library grounding, proof quality, impact
   completeness, and targeted legacy disposition.

Activation creates state; it does not execute WP28 or assert implementation readiness. Completion
exists only after WP42 and M06/DB14 are independently accepted at one trusted HEAD. Development may
be safe to continue after earlier milestones, but release certification cannot be inferred from
focused gates, state health, packet counts, or a partially green matrix.
