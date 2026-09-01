---
artifact: implementation-plan
plan_id: codefabric-execution-proved-relational-data-fabric
version: v4
date: 2026-09-01
status: draft
design_path: docs/reviews/interface_design_review_daemon_grpc_fastmcp_boundary_2026-09-01_v4.md
design_version: v4
baseline_commit: f12329f05e3678698ff9a43ec4f69f95f42db12f
working_tree_digest: 25f3d3e36ffb1df4a140133c48235fc5a2e23fe5cea5ce2ec4a0b6584d5130c9
state_path: docs/plans/state/codefabric-execution-proved-relational-data-fabric_v4_state.json
cutover: true
supersedes_on_activation: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v3_2026-08-30.md
---

# CodeFabric execution-proved relational data fabric -- implementation plan v4

This draft successor converts the accepted production daemon, gRPC, and FastMCP boundary review
into one dependency-closed continuation of the relational-data-fabric plan. It preserves the
traceability identities WP29--WP42, DB09--DB14, and L-20--L-55 while replacing their stale
composition, cutover, and certification semantics. WP28 and M01 are deliberately absent: they are
not dependencies, they receive no status, and they contribute no oracle to this plan.

Plan v3 has no trusted packet proving commit to migrate. Its current tree may contain reusable
implementation, tests, and deletion work, but existence and prior state labels confer no v4
completion. This file creates neither
`docs/plans/state/codefabric-execution-proved-relational-data-fabric_v4_state.json` nor a change to
`docs/plans/active-plan.json`. Independent plan audit, approval, and the repository's atomic
activation transaction remain required before execution.

## 1. Outcome and non-goals

### 1.1 Outcome

At completion:

1. A versioned authoritative-suite successor expresses the accepted `FreshActivation` deployment
   profile, lawful genesis, exact activation horizons, clean `codefabric.cpgd.v2` session/resource
   behavior, and revised roadmap order without modifying v2.1 history in place.
2. `CodeFabricV21Release` is the sole compiled production owner of provider relation/field
   descriptors, transformations, application analyses, proof construction, producer closure, and
   all eight semantic request programs. Operational inputs carry only values that genuinely vary.
3. Production construction follows legal phase transitions from `PreEpochWorkspace` through
   `CandidateFabric`, `SealedEpoch`, `SelectedEpochRecord`, and `ActiveWorkspace`. Digests bind
   identity/integrity only; they never substitute for construction, execution, or exact readback.
4. `ProductionStartupCoordinator` and one lifecycle authority drive discovery, health projection,
   handshake, status, validation, query admission, activation, drain, and shutdown. A bound daemon
   may honestly report bootstrapping, but only `Ready` admits semantic work.
5. Empty activation state reaches genesis only through the recovered command actor with
   `ExpectedHead::Empty`. Exact append/readback returns one event, reversible table-version vector,
   writer fence, and control horizon; unknown outcomes reconcile without a blind retry.
6. One atomic slot holds an immutable `Arc<ActiveWorkspace>`. Each accepted query pins its epoch,
   authorization, resource, and result leases until joined terminal cleanup; activation swaps only
   after the exact new authority is reconstructed.
7. Provider descriptors construct Arrow schemas without name heuristics, fake source execution, or
   `Debug`-text fingerprints. Native DataFusion `Expr` and `LogicalPlan` construction, reduced
   authorized child catalogs, and one governed workspace `RuntimeEnv` remain the default.
8. `QueryCoordinator` reserves bounded scheduler, journal, idempotency, result, retention, and task
   capacity before accepting a handle. Query execution remains a `SendableRecordBatchStream` until
   bounded, independently decodable Arrow IPC pages are sealed into one immutable result package.
9. Exact delta-rs versions remain table authority; CodeFabric remains activation, fencing,
   uncertain-outcome reconciliation, maintenance/lease, and atomic workspace-publication authority.
10. One clean `codefabric.cpgd.v2` service owns `Handshake`, `GetStatus`, `GetReference`,
    `ValidateQuery`, `StartQuery`, `WatchQuery`, `CancelQuery`, `ReadResource`, and
    `ReleaseResource`. It has no v1 translator, legacy profile, repeated body authority, overlapping
    watch methods, legacy strings/cursors, or wall-deadline compatibility.
11. Daemon-minted expiring sessions bind peer, daemon generation, principal, workspace, profile,
    operation, handle, and resource authority. `OwnedUnixSocket` guards both endpoints; standard
    Tonic health reports liveness only.
12. FastMCP performs one eager lifespan handshake over one long-lived `grpc.aio` channel, exposes
    exactly four stable tools, reports bounded progress, reads live reference content from Rust,
    and remains strict-Pydantic presentation only. It never becomes an Arrow/DataFusion/Delta or
    semantic JSON processing layer.
13. The displaced ontology/bootstrap/model/generated-schema/dual-epoch authority and the dormant
    permanent predecessor cutover subsystem reach physical zero state after their last target
    consumers cut over. Reusable writer/fence/idempotency/reconciliation primitives move inward.
14. A real source mutation passes through the installed `codefabricd` binary, exact providers,
    native transformations, Delta activation, scheduled query, bounded Arrow pages, generated gRPC,
    and installed FastMCP STDIO package. Restart, cancellation, retention, security, zero-state,
    clean reconstruction, and representative resource behavior pass at one trusted HEAD.

### 1.2 Non-goals

- No legacy behavioral baseline, predecessor comparator, synthetic predecessor deployment, or old
  ontology agreement is required for correctness. Such material may remain labelled history only.
- No WP28/M01 scope, status migration, proving commit, oracle, or completion fiction.
- No arbitrary SQL, public physical table/function names, DataFrame/plan handles, serialized plans,
  or Python-authored semantic catalogs.
- No Python Arrow/DataFusion/Delta processing, whole-relation assembly, dynamic FastMCP catalog,
  dynamic Pydantic model generation, `orjson`, or prose-error parsing.
- No raw Parquet/object listing as Delta state, no cache-selected current epoch, no SQLite semantic
  head, and no hash/digest/count/plan text as semantic proof.
- No unbounded query/event/result map, channel, journal, task set, page, resource, or lease lifetime.
- No live `codefabric.cpgd.v1` compatibility, translator, dual service, old-client fixture gate,
  reflection, compression, keepalive, HTTP/2 tuning, or richer status dependency before exact
  target-only need and interop evidence.
- No new Cargo root, Python processing service, process boundary, or dependency added for code
  organization alone.
- No live authority handoff implementation unless a read-only deployment census discovers a real
  predecessor. That discovery is a design-reopen trigger, not permission to retain dormant code.
- No state creation, plan activation, production implementation, or deletion in this plan-authoring
  turn.

### 1.3 Baseline identity and trust posture

The baseline commit and frontmatter working-tree digest identify the planning snapshot only. The
digest is SHA-256 over `git status --porcelain=v2 -z`; it proves neither correctness nor ownership
of individual changes. No behavioral comparator baseline was taken or is required. The workspace
was already heavily dirty, including substantial successor additions and predecessor deletions, so
every packet must rediscover and reserve its exact impact before editing.

Current-tree inspection established that `codefabricd` calls error-only `daemon::serve`, while
`serve_programmatic` and the real daemon factory have no production caller; first activation is
test-seeded; readiness has multiple owners; query/result state is partly unbounded and whole-result
materialized; the query proto lacks `GetReference` and opaque sessions; FastMCP connects lazily and
synthesizes reference content; and forward-cutover wiring remains reachable. These are planning
facts, not inherited failures that the target must reproduce.

The plan-authoring `just ci-fast` observation exited 1 on a pre-existing formatting diff in
`src/daemon.rs` around programmatic backend construction. It is not a semantic baseline and was not
repaired here. The first code-editing packet must reconcile that dirty region before claiming an
edit-local format gate.

### 1.4 Execution law

- The accepted v4 review is the immediate design authority for this successor. WP33 must issue a
  collision-free versioned authoritative suite before design-bearing code packets begin; it must
  create new artifacts and never edit the declared v2.1 inputs below in place.
- A packet starts only after every named dependency is complete at an ancestral proving commit,
  inputs are fresh, current-tree impact and ownership are reconciled, and any shared-path overlap is
  serialized. Packet numbering is traceability, not execution order.
- A packet completes only when exactly four unique substantive executable oracles pass at its
  proving commit and candidate HEAD: artifact/integrity (`INT`), positive behavior (`BEH`),
  rejection/absence/failure (`NEG`), and operations/recovery/resource/performance (`OPS`). Every
  selector proves a nonzero test count and a committed discriminating fault.
- The active packet/oracle universe is derived from the parsed plan. This version presently yields
  fourteen packets and fifty-six oracles, but no validator or certification artifact may hard-code
  `52`, `56`, WP28, or predecessor-cutover recipe literals.
- A target consumer lands before its predecessor is deleted. Unreachable, deprecated,
  feature-disabled, ignored, or absent from one grep is not zero state. Decommission spans source,
  exports, generated includes, features, targets, packages, recipes, workflows, services, rules,
  fixtures, installed artifacts, and hidden live paths.
- A retained primitive is moved behind a target-owned API and receives a named target consumer.
  Historical wire spellings remain only in explicitly non-live artifacts and may not be generated,
  packaged, selected, or consumed by production code.
- Native library capability is the default. Custom DataFusion/Arrow/Delta, transport, or storage
  behavior records its highest-viable-rung decision, full contract, resource/cancellation proof,
  and exact replan trigger.
- Packet state records status, proving commit, deviations, failed approaches, and blockers only.
  Commands, outputs, selected counts, impact inventories, performance distributions, and evidence
  payloads live in packet-owned versioned artifacts and commits.
- Before activation append, recovery discards the private candidate and leaves admission closed.
  After an unknown append outcome, recovery reads the coherent activation horizon and reconstructs
  it or enters `FailedClosed`; it never retries blindly. After target-format mutation, rollback is
  forward repair through the target command path, never predecessor restoration.

## 2. Source design and declared inputs

The following planning inputs are immutable. WP33 creates new successor-suite files; it does not
alter these paths. Any unplanned change to an input makes this draft stale and requires a revised
plan before activation or continued execution.

| path | sha256 |
|---|---|
| docs/reviews/interface_design_review_daemon_grpc_fastmcp_boundary_2026-09-01_v4.md | b3d9633272a931c511118c2ed639d4560184616a31b9aae38562fa4bfc52d8bc |
| docs/reviews/interface_design_review_daemon_grpc_fastmcp_boundary_2026-08-31_v3.md | 9e53d7fbcad46e718390324e81b0daf15e3dd4a071f8e6d8e89fa9e405edbe4e |
| docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v3_2026-08-30.md | 426b018ba33f8e73a1eacdff5c7fa415b367c855e3d588472d8f43706f0560f1 |
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
| docs/library_ref/rust_grpc_daemon_advanced_reference_tonic_0.14.6.md | 6dd8665f9c33e70181c292b91f6376fd76b12d7e3073a956e48ba9542d9adf32 |
| docs/library_ref/grpcio_python_advanced_reference_1.83.0.md | e01fd5483b679cb62ef09e2c50228ab74eab298c2d559774f1f4c7ddd3320f78 |
| docs/library_ref/protobuf_python_advanced_reference_7.36.0.md | 2b9a2151f25e610ef75a43739b23852fd5faac3b183bbe1c374ff9923001798e |
| docs/library_ref/fastmcp_python_advanced_reference_3.4.7.md | f3c1fc3def7ab14ce09a10b66b06f89f84419525781a368b625ea9d2ff338fb3 |
| docs/library_ref/pydantic_python_advanced_reference_2.13.4.md | 4f66f29a9fde6feed03a0755942db9bb9fb0834f57ff49ab80ab448d65d6a477 |

### 2.1 Planned design-authority evolution

WP33 allocates the next synchronized, collision-free suite version under `SUITE §0` governance.
`2.2.0`/`v2.2` is an illustrative likely allocation, not permission to collide with intervening
work. All seven domain roles and the roadmap receive versioned successor files so suite membership
remains coherent. SUITE, LIFE, FAB, SRV, and RM receive the accepted substantive changes; ONT, GEN,
and QRY carry forward unchanged semantics with explicit successor linkage unless review discovers a
required cross-domain correction.

The successor SRV selects `codefabric.cpgd.v2` as the sole production package. V1 source,
descriptors, allocations, and fixtures remain explicitly non-live history; generated v1 runtime
bindings, services, clients, package payload, translators, and operability tests are decommission
targets. No later packet may restore them to satisfy an old compatibility oracle.

### 2.2 Live pins and library contract

Execution rederives pins from the successor FAB and Cargo/uv metadata. The planning baseline is one
Arrow/Parquet 59.2.0, DataFusion 55.0.0, `object_store` 0.13.2, delta-rs revision `43a0cf10`, Tonic
0.14.6/Prost 0.14.4, grpcio/grpcio-tools 1.83.0, protobuf 7.36.0, FastMCP 3.4.7, and Pydantic
2.13.4 universe. `just stable-graph-check`, `proto-check`, and domain lock checks outrank copied
version text.

Selected capability posture:

- DataFusion provider/catalog/session/expression/logical-plan APIs remain semantic execution
  substrate. `SessionContext::execute_logical_plan` and `DataFrame::execute_stream` retain
  `SendableRecordBatchStream`; no production `collect` boundary is accepted.
- Arrow pages each use a fresh IPC `StreamWriter` and are independently reopened by
  `StreamReader`. A bounded page-local buffer is permitted; a whole-result buffer is not.
- `object_store::buffered::BufWriter` is async while Arrow's writer is synchronous. WP36 must
  compile-probe bounded page encoding followed by create-only object publication and manifest-last
  sealing; if the supported local store cannot provide the required atomicity, it uses a private
  temporary file plus no-replace rename behind `ResultObjectSink`.
- Delta opens exact selected versions, installs the matching object-store mapping, uses high-level
  providers/writes and zero library retry, and preserves application-owned activation,
  reconciliation, and lease-aware maintenance.
- Static generated Tonic/grpcio v2 bindings remain. Tonic health is a liveness service; verified
  UDS peer identity enters request extensions; one long-lived Python channel is recreated and
  re-handshaken after reconnect.
- FastMCP resources are materialized and therefore hard-bounded. Pydantic models remain strict,
  frozen, extra-forbid, and module-scoped with reused adapters and both validation/serialization
  schema snapshots. No `orjson` is added.

### 2.3 Current-tree known impact

Known touch points are discovery seeds, not a must-edit list:

- production composition: `src/bin/codefabricd.rs`, `src/daemon.rs`,
  `src/fabric/programmatic_workspace.rs`, `src/fabric/programmatic_query_backend.rs`, `src/lib.rs`,
  `src/fabric.rs`, Cargo features/targets, and daemon integration tests;
- semantic release/providers: `src/production_provider_recipe.rs`,
  `src/programmatic_derived_analysis.rs`, `src/production_query_recipe.rs`, provider schema/boundary/
  admission/IPC/service modules, auxiliary process protos, and generated bindings;
- epoch/lifecycle: programmatic epoch/workspace, activation-control/transaction/admission/command,
  writer lease/generation, admission, epoch runtime, workspace registry, and source lifecycle;
- query/result: `src/query_service.rs`, programmatic query backend, relational query runtime,
  semantic query contracts, child sessions/resource governance/caches, graph/producer closure,
  Arrow result resources, and published result registry;
- public boundary: `contracts/rpc/cpg_query_service.proto`, generated Rust/Python bindings,
  `src/rpc.rs`, query/admin UDS serving, proto tooling/fixtures, and the Python daemon client,
  channel, Arrow resources, server, settings, models, tests, package metadata, and lock;
- decommission/evidence: forward-cutover modules/exports/admin/CLI/recipes, ontology/model/bootstrap
  paths already deleted in the dirty tree, `scripts/model_zero_state_check.sh`, remaining-legacy and
  release-evidence tooling, acceptance contracts, governance rules, justfile, and CI.

Every packet preflight uses `ast-grep outline` for unfamiliar code, structural call/declaration
queries with inspected file counts, and hidden-aware `rg` with `docs/library_ref/**`, `.git/**`, and
`target/**` excluded unless explicitly in scope. Negative zero-state claims report matched,
skipped, excluded, unparsed, and package/build candidates; no empty search stands alone.

## 3. Target invariants and v3 reconciliation

| ID | Invariant |
|---|---|
| I4-01 | One compiled release owns every production semantic constructor; callers supply no alternative production catalog, schema, plan, producer, or proof closure. |
| I4-02 | Only genuinely variable operational/source/policy/access/provider-availability inputs cross the release boundary; absence becomes an explicit gap. |
| I4-03 | Phase types permit only legal transitions and derive downstream values from prior phases; separately constructed digest-equal values are not authority-equivalent. |
| I4-04 | Genesis, mutation, activation, and uncertain-outcome recovery use one fenced command actor; there is no direct production seed or blind append retry. |
| I4-05 | One coherent control-horizon read yields activation event, exact reversible table vector, fence, and proof reference; queries never resolve `latest`. |
| I4-06 | One lifecycle authority drives all readiness/admission projections; bound transport may be bootstrapping, and only `Ready` admits semantic work. |
| I4-07 | One atomic immutable `ActiveWorkspace` slot is installed/swapped; each request retains one old or new epoch and never mixes them. |
| I4-08 | One governed `RuntimeEnv`, native plans, explicit provider contracts, and reduced dependency-closed child catalogs preserve schemas, field identity, authorization, and resource policy. |
| I4-09 | Query capacity, journal, idempotency, tasks, result bytes/pages, leases, tombstones, deadlines, fairness, and cancellation are bounded by one coordinator before acceptance. |
| I4-10 | DataFusion streams to bounded independently decodable Arrow pages; one sealed package owns response, manifest, objects, leases, release, and cleanup. |
| I4-11 | Delta owns table snapshots and commits; CodeFabric owns multi-table epoch selection, fencing, exact readback, reconciliation, active publication, and lease-aware maintenance. |
| I4-12 | One clean `codefabric.cpgd.v2` control/resource contract owns the target; v1 operability, translations, field meanings, and generated runtime bindings are historical only. |
| I4-13 | Kernel peer identity plus an expiring daemon-generation session authorizes every workspace, operation, handle, and resource; body IDs are assertions only. |
| I4-14 | Admin/query sockets are created and removed only through owned path/type/owner/mode/live-probe/device/inode/generation checks. |
| I4-15 | Health means liveness; remaining budgets, typed codes, streaming backpressure, reconnect, resume, cancellation, and joined shutdown remain distinct and explicit. |
| I4-16 | FastMCP eagerly handshakes, exposes exactly four static tools, reads live daemon references, reports bounded progress, and performs presentation only. |
| I4-17 | Current deployment uses FreshActivation and sole target authority; contrary deployed-predecessor evidence reopens a one-shot AuthorityHandoff design. |
| I4-18 | Bootstrap/ontology/model/generated-schema/dual-epoch/default-backend and dormant forward-cutover routes reach multidimensional physical zero state after replacement. |
| I4-19 | Semantic acceptance uses independently authored decoded values, causal input/operand faults, execution, exact durable readback, and failure injection; hashes/counts/text prove only their own narrow properties. |
| I4-20 | The terminal vertical uses the real binary, generated wire, installed adapter package, real source mutation, restart, and one trusted HEAD. |
| I4-21 | Shutdown joins queries, pages, providers, watchers, commands, stores, sockets, writer leases, and daemon lease in owner order; retained/unsealed work has explicit restart state. |
| I4-22 | No performance tuning is accepted without a representative workload, semantic equality oracle, environment, distribution, and resource envelope. |

### 3.1 V3 packet disposition

| Identity | V4 disposition |
|---|---|
| WP28 / M01 | Excluded. No dependency, state, oracle, completion claim, or certification literal. |
| WP29 | Retain ID; replace broad composition with compiled release consumption, phase-typed startup, one lifecycle authority, atomic workspace, and real binary entry. Operational genesis closes in WP32 and the assembled Ready path closes in WP37. |
| WP30 | Retain ID; continue rapid ontology/bootstrap/model/generated-schema cleanup after WP29's target consumer. Activation-specific dual-epoch residue may remain only until WP32 closes DB09. |
| WP31 | Retain ID; make provider/field/transformation/query constructors private and exhaustive, preserve native DataFusion plans, governed runtime, authorized child closure, and bounded caches. |
| WP32 | Retain ID; return one exact `SelectedEpochRecord`, implement lawful genesis and warm recovery, advance activation in process, and remove candidate/default selectors. |
| WP33 | Retain ID; first issue the versioned successor suite and independently reviewed expectations/negative fixtures before dependent implementation claims. |
| WP34 | Retain ID; close exact provider descriptors, IPC, admission, coverage, gaps, and provider trust. |
| WP35 | Retain ID; close all application analyses, normalization/authority/unknown transformations, producer closure, and fixed-point proof. |
| WP36 | Retain ID; close all eight requests, query coordination, streamed page sealing, result packages, retention, and cancellation before transport. |
| WP37 | Retain ID; implement the one real lifecycle-to-UDS/gRPC/FastMCP vertical and delete replaced serving/presentation routes. |
| WP38 | Retain ID; issue and execute first-principles production evidence with causal faults and clean reconstruction. |
| WP39 | Retain ID; purge remaining generated/governance/package/feature/recipe residue and close DB12/DB13. |
| WP40 | Retain ID; rerun post-purge release evidence and representative boundary/resource measurements. |
| WP41 | Retain ID but replace predecessor cutover completely with fresh successor activation, sole-target ownership, forward repair, and dormant handoff deletion. |
| WP42 | Retain ID; derive the active oracle set and certify the revised authority, real vertical, zero state, four domains, and independent review at one HEAD. |

### 3.2 Dependency graph

```text
WP33
  -> WP29 -> WP30
  -> WP31 --------------------+
       +-> WP34 -> WP35 ------+-> WP36
       +-> WP32 --------------+     |
                                     v
WP29 + WP30 + WP31 + WP32 + WP33 + WP34 + WP35 + WP36
                                     |
                                     v
                                    WP37 -> WP38 -> WP39 -> WP40 -> WP41 -> WP42
```

WP32 and WP34/WP35 may proceed in parallel after WP31 only when current-tree impact proves their
public types, generated contracts, recipes, and files do not overlap; otherwise serialize them.
WP29 may complete an honestly bootstrapping real kernel before WP32/WP36, but cannot claim semantic
Ready, lawful production genesis, or a query vertical until those dependencies join in WP37.

## 4. Dependency-closed work packets

### WP33 — Version successor authority and issue independent expectations

**Outcome.** A synchronized authoritative-suite successor records the accepted v4 target, and one
independently reviewed expectation/negative-fixture release defines what later semantic, lifecycle,
wire-v2, resource, security, and zero-state behavior must prove. No implementation output authors
its own expected value.

**Dependencies.** None. WP28 and M01 are not implicit dependencies.

**Target invariants.** I4-01--I4-05, I4-09--I4-20, I4-22; P1--P5, P9--P10, P18--P22,
P25--P31, P36.

**Design and library references.** Accepted design v4 §§0--5 and incorporated v3 §§6--17; current
SUITE §§0, 8--13; LIFE §13; FAB §14; SRV §7; RM §0; Protobuf schema/evolution/testing §§0, 7,
11--13, 26--30, 37, 39, 44; Tonic §§0, 3--8, 34--35, 39, 43; full-data-fabric principles staticness
test and P18/P25/P30.

**Change surface / preflight / known touch.** Before edits, discover the current suite-selection and
version-chain rules, every current domain frontmatter identity, derived spec indexes, plan/state
selection, current expectation/evidence transactions, proto allocation history, and hidden/package
consumers. Known touch: eight new files under `docs/authoritative_design/`, derived
`docs/spec_index/**`, `contracts/acceptance/relational-fabric-v4/**`, focused expectation/review
tooling and tests, and `justfile`. Current v2.1 and prior artifacts are read-only.

**Required changes.**

1. Allocate the next synchronized suite version and copy all seven domain roles plus RM into new,
   noncolliding artifacts. Preserve predecessor links and immutable prior content.
2. Revise SUITE, LIFE, FAB, SRV, and RM for `FreshActivation`, phase-typed startup, lawful
   `ExpectedHead::Empty` genesis, one exact selected horizon, atomic active workspaces, target-only
   forward repair, and the plan order in §3.2. Carry ONT, GEN, and QRY forward coherently unless the
   cross-domain review finds a necessary accepted correction.
3. Define `codefabric.cpgd.v2` as the sole production wire package with the nine v4 methods, clean
   typed messages, opaque binary sessions, one relative budget, one content-bound cursor,
   control-only events, typed resource descriptors, and no v1 translator/profile/operability.
4. Define the minimal inherited supervisor-control record contract for launch-grant registration,
   revocation, generation advance, and acknowledgement. It carries no semantic query/result data
   and has explicit bounds, ordering, replay, expiry, and loss behavior.
5. Issue independently authored typed expectations for provider rows/gaps, transformation and
   analysis outputs, all eight query forms, exact activation/readback, lifecycle projections,
   v2 wire/session/resource behavior, FastMCP projections, recovery, and zero state. Expectations
   may cite design semantics but may not import production modules or generated target output.
6. Issue negative fixtures for omitted provider roles, ambiguous producers, authority leaks,
   invalid phase transitions, incoherent horizons, session and cursor tampering, wrong owner/
   generation/workspace/operation, unsafe sockets, unbounded resources, v1 live selection, and
   forbidden Python semantics.
7. Obtain an independent review of suite causality and every expectation family. Record limitations
   and superseded v3/v1 evidence without making predecessor bytes part of the verdict.

**Legacy disposition and decommission.** Current v2.1 and v1.3/v2.0 suites, v3 review, v3 plan/state,
v1 proto/descriptor allocation history, and old evidence remain immutable non-live history. V1
runtime generation, package payload, client/server operation, translators, and compatibility
fixtures become DB11/DB13 deletion targets. No live artifact is deleted in this packet.

**Acceptance checks.**

Executable oracle: `successor-authority-expectation-integrity-check`
Governed criterion: `PC-WP33-INT`

Executable oracle: `independent-expected-relation-review-check`
Governed criterion: `PC-WP33-BEH`

Executable oracle: `negative-fixture-independence-check`
Governed criterion: `PC-WP33-NEG`

Executable oracle: `expectation-drift-selector-sensitivity-check`
Governed criterion: `PC-WP33-OPS`

**Oracle category fault contract.** `INT` corrupts suite membership, predecessor linkage, a v2
allocation, or expectation provenance; `BEH` changes a controlled semantic input and requires the
independently authored decoded expectation to distinguish it; `NEG` proves a fixture imports target
expected-value code or permits a forbidden authority; `OPS` changes one issued input/selector after
freeze and requires downstream execution to stop rather than silently refresh it.

**Edit-local gates.** Run `just spec-outline` on every changed authoritative artifact, targeted
artifact-contract tests, expectation-tool unit tests, `just authoritative-design-conformance-check`,
and targeted `typos`. Verify v2.1 files are byte-unchanged.

**Packet-local gates.** Add/reshape and run `just successor-authority-expectation-integrity-check`,
`just independent-expected-relation-review-check`, `just negative-fixture-independence-check`, and
`just expectation-drift-selector-sensitivity-check`; each reports nonzero selected cases and its
committed fault.

**Milestone.** Completes M02 after the successor suite and expectation review are the accepted
inputs to all dependent packets.

**Replan triggers.** Stop if synchronized versioning cannot express the successor, the new v2
package cannot own the required functional surface without semantic duplication, the supervisor
grant root cannot be supported by an authorized launcher, or expectation independence cannot be
demonstrated.

**Rollback and recovery.** Before suite selection, remove only unselected new candidate artifacts
and leave v2.1 current. After selection, correct forward with another version; never rewrite the
selected suite or reinstate v1 operability.

**Conditional exemplars.** A `2.2.0` suite and `v2.2` filenames are illustrative only. A rich-status
detail schema is permitted only after a bilateral pinned-version probe; otherwise the target uses
standard status plus one bounded typed trailing-metadata code.

### WP29 — Compose an honest phase-typed production kernel

**Outcome.** The real `codefabricd` entry reaches one production `DaemonKernel` built from
`CodeFabricV21Release`, explicit operational inputs, phase-typed workspace state, one lifecycle
authority, one atomic active-workspace slot, and joined ownership. Before WP32/WP36 close, the
kernel may be honestly bootstrapping; it cannot report semantic Ready or use a test/default backend.

**Dependencies.** WP33.

**Target invariants.** I4-01--I4-07, I4-15, I4-17, I4-20--I4-21; P1--P3, P7--P8, P11,
P16--P17, P23, P27, P32--P35.

**Design and library references.** Design v3 §§6.1--6.5, 6.8 and v4 §§0, 5; successor SUITE/LIFE/
FAB/RM issued by WP33; DataFusion MOD/CAT/RUN/GOV flows; Tonic §§24--28, 37--40; `arc-swap` selected
by design LD3-14.

**Change surface / preflight / known touch.** Trace all callers/constructors of `daemon::serve`,
`serve_programmatic`, `ProgrammaticWorkspaceRuntimeFactory::build_daemon`, programmatic composition,
admission, discovery, health/readiness, shutdown steps, and binary/feature selection. Use structural
call searches and a hidden-aware textual envelope. Known touch: `src/bin/codefabricd.rs`,
`src/daemon.rs`, `src/fabric/programmatic_workspace.rs`, epoch/admission/runtime/command modules,
`src/lib.rs`, `src/fabric.rs`, Cargo targets/features, daemon integration tests, and just recipes.

**Required changes.**

1. Add the immutable `CodeFabricV21Release`, operational-only workspace registry,
   `ProductionDaemonFactory`, `ProductionStartupCoordinator`, `LifecycleAuthority`, atomic
   `WorkspaceSlot<Arc<ActiveWorkspace>>`, and `DaemonKernel` ownership boundaries.
2. Replace broad synchronized construction DTOs with legal phase aggregates. Transitions derive
   catalogs, proof, exact selection, and query authority rather than accepting separately assembled
   digest-equal values.
3. Model `Configured -> DaemonLeased -> WriterFenced -> CommandRecovered -> GenesisRequired |
   SelectedEpochRecovered -> ... -> Ready -> Draining -> Stopped`, including `FailedClosed`.
   WP29 implements the honest states and ports; WP32 owns operational genesis/exact horizon, and
   WP37 proves the joined Ready path.
4. Route `codefabricd` to the production factory. Remove `ProgrammaticCompositionRequired` from the
   normal production path. Missing roots, policy, provider availability, writer authority, or grant
   control fail closed with no leaked socket, lease, task, store, or partial workspace.
5. Bind admin/query endpoints only after service construction. During bootstrapping, handshake,
   status, and reference may respond from lifecycle/release authority; validate/start return one
   typed unavailable result without consulting an uninstalled query backend.
6. Ensure every accepted query will pin one immutable active workspace; no query uses a mutable
   bundle of epoch/catalog/vector/proof fields. Existing queries survive a later atomic swap under
   their old leases.
7. Implement joined shutdown ownership order and replace placeholder-success steps with evidence
   from the actual owner. Writer and daemon singleton leases release last.

**Legacy disposition and decommission.** This is the positive consumer for the error-only
`daemon::serve`, broad `ProgrammaticWorkspaceConstruction`, hardcoded Ready/default freshness, and
test-only production selection. Their deletion occurs in WP30 after this packet's real binary and
bootstrapping cases pass. Activation-specific transitional scaffolding can remain only as named
DB09 debt for WP32.

**Acceptance checks.**

Executable oracle: `production-composition-contract-integrity-check`
Governed criterion: `PC-WP29-INT`

Executable oracle: `programmatic-production-composition-check`
Governed criterion: `PC-WP29-BEH`

Executable oracle: `daemon-bootstrap-route-denial-check`
Governed criterion: `PC-WP29-NEG`

Executable oracle: `programmatic-runtime-lifecycle-check`
Governed criterion: `PC-WP29-OPS`

**Oracle category fault contract.** `INT` removes a phase/release/lifecycle/ownership dependency;
`BEH` launches the actual binary and observes its one production kernel and causal lifecycle
projection; `NEG` attempts default, test, missing-input, direct-seed, or false-Ready selection;
`OPS` faults partial construction, endpoint bind, task start, drain, and restart and proves bounded
owner cleanup.

**Edit-local gates.** `just root-fmt`, targeted daemon library/integration tests, `just root-check`,
targeted Clippy for touched targets, and structural production-caller/readiness rules.

**Packet-local gates.** Run the four named packet recipes independently and then through the packet
oracle harness. A process launch, not a direct factory call, supplies the BEH/OPS evidence.

**Milestone.** Contributes to M03; it does not close semantic Ready or the production vertical.

**Replan triggers.** Stop if phase ownership requires caller-supplied semantic values, one lifecycle
cannot drive every projection, atomic workspace installation cannot retain old query leases, or the
production binary can only start through an injected backend.

**Rollback and recovery.** A failure before activation leaves admission closed, joins partial
owners, removes only owned sockets, and releases writer/daemon leases in reverse order. No fallback
server is selected.

**Conditional exemplars.** Existing `ArcSwapOption<ProgrammaticFabricEpoch>` is a migration seed,
not the prescribed public type. The final slot may use `ArcSwapOption<ActiveWorkspace>` or an
equivalent proven atomic abstraction without changing lifecycle authority.

### WP30 — Cut over and rapidly delete displaced ontology/bootstrap authority

**Outcome.** After WP29 supplies the target production consumer, bootstrap/model/compiler/
ontology/generated-schema/default-backend authority is physically absent from live builds and
packages. Already-deleted dirty-tree work is preserved and verified, never recreated for baseline
comparison.

**Dependencies.** WP29.

**Target invariants.** I4-01--I4-03, I4-06, I4-17--I4-19; P1--P3, P12, P18, P26--P28,
P31--P36.

**Design and library references.** Design v3 §§3.1, 3.9, 11.2--11.4, 13.2, 14.8 and v4 §§0, 4--5;
successor SUITE/ONT/GEN/FAB/LIFE/RM; L-20--L-24, L-27--L-30, L-35, L-37--L-39, L-54--L-55.

**Change surface / preflight / known touch.** Reconcile current deletions and all remaining
consumers before any delete. Scope includes `src/ontology_*`, `src/bin/codefabric_model/**`,
`src/generated/model*`, `src/relational_model/**`, old epoch/model-migration/schema-registry/
registries/provider/query routes, `contracts/generated/model/**`, ontology/model bundles and
registries, adapter package data, tooling/scripts/rules/tests/recipes/features/targets, ignored live
sources, and installed wheel/sdist contents. Preserve unrelated user edits.

**Required changes.**

1. Move only released identity primitives, target schema-phase validation, canonicalization, wire
   allocation history, and authority-neutral writer/fence/reconciliation primitives behind named
   target-owned consumers.
2. Delete the model compiler/importer/replay, bootstrap table/schema construction, ontology bundle/
   candidate/program authority, generated semantic arrays/registries, live artifact readers,
   model-migration command/effects/admin route, and default/empty/debug query backends.
3. Delete the model bin/feature/dependencies, model tooling, recipes, CI/package edges, adapter
   registries/query-form data, obsolete rules/snapshots, fixtures, and tests whose only purpose is
   the rejected authority.
4. Remove old `daemon::serve` error-only routing and test-seeded composition helpers after the WP29
   binary path owns bootstrapping. No deprecated wrapper, alias, feature, or test helper may keep
   production selection reachable.
5. Preserve only the narrow activation-era primitives WP32 still consumes. Record each by symbol,
   target consumer, and WP32 deletion/reshape action; DB09 cannot close while an old epoch/current
   selector remains.
6. Update tests to construct explicit inputs, release-owned recipes, and legal phase values. Delete
   replay/comparator tests rather than translating them into a new static registry.
7. Prove multidimensional zero state with path, text, syntax, Cargo, Python package, generated
   include, recipe, workflow, service, rule, fixture, ignored-source, and installed-artifact
   inventories. History exclusions are exact, non-importable paths.

**Legacy disposition and decommission.** Executes the bulk of DB09 and preserves prior deletions.
Historical designs/plans/reviews, released wire allocations, canonical known-answer vectors, and
explicit tombstones remain. Activation-specific dual-epoch residue is not called complete; WP32
removes or reshapes it before DB09 exits.

**Acceptance checks.**

Executable oracle: `bootstrap-model-decommission-integrity-check`
Governed criterion: `PC-WP30-INT`

Executable oracle: `compiled-release-consumer-cutover-check`
Governed criterion: `PC-WP30-BEH`

Executable oracle: `bootstrap-ontology-authority-zero-state-check`
Governed criterion: `PC-WP30-NEG`

Executable oracle: `programmatic-model-free-restart-check`
Governed criterion: `PC-WP30-OPS`

**Oracle category fault contract.** `INT` removes a retained primitive mapping or disposition;
`BEH` proves target consumers still construct and reach honest lifecycle states with only compiled
release/explicit inputs; `NEG` reintroduces a legacy file/symbol/feature/target/package/recipe/
selector; `OPS` rebuilds/packages/restarts with historical/model bytes unavailable.

**Edit-local gates.** Run format/check on each touched build domain, focused target-consumer tests,
`just model-zero-state-check`, `just remaining-legacy-zero-state-check`, targeted governance-rule
tests, and package inventory checks. Never run a mutating cleanup recipe as a gate dependency.

**Packet-local gates.** Run all four packet recipes with nonzero candidate/skipped/exclusion output;
the negative fixture places one live legacy route outside the history class and must fail.

**Milestone.** Contributes to M03 and DB09; final DB09 closure waits for WP32's activation residue
disposition.

**Replan triggers.** Stop if a supposedly legacy value remains non-derivable and behaviorally
required, a released public contract depends on the old internal meaning, or current-tree ownership
cannot distinguish concurrent user work from deletion targets.

**Rollback and recovery.** Restore only task-authored candidate deletions that lack a target
consumer; never restore the predecessor subsystem wholesale. A failed packet leaves the production
kernel bootstrapping/fail-closed, not on a legacy fallback.

**Conditional exemplars.** Renaming the surviving programmatic epoch to `FabricEpoch` is optional
and occurs only after all old epoch aliases are gone. Historical paths listed by exact suffix remain
excluded from zero-state scanning but may not be imported by build/runtime/package code.

### WP31 — Close compiled release, schema, catalog, and DataFusion authority

**Outcome.** `CodeFabricV21Release` privately and exhaustively constructs provider/field contracts,
typed transformations, application-analysis declarations, proof/producer closure, and all eight
query programs. One governed DataFusion runtime and plan/provider-derived schemas feed root and
dependency-closed authorized child sessions; bounded caches remain optimization only.

**Dependencies.** WP29, WP30, and WP33.

**Target invariants.** I4-01--I4-03, I4-07--I4-08, I4-11, I4-18--I4-19; P1--P8,
P11--P16, P18--P20, P23, P27, P30, P32--P35.

**Design and library references.** Design v3 §§6.2--6.3, 7.1--7.3, 13.3, 14.2 and LD3-01,
LD3-05--LD3-06, LD3-13; successor ONT/GEN/FAB/QRY; DataFusion/Arrow alignment schema, provider,
plan, governance, provenance, and test flows; DataFusion schema/catalog/planning §§14--19,
41--49, S1--S15; Arrow schemas/metadata §§3, 10; Delta provider/exact-state §§3, 6--7.

**Change surface / preflight / known touch.** Outline and trace constructors/callers for production
provider/query recipes, programmatic transformations/epoch/schema/observations, child sessions,
catalog/provider/view wrappers, runtime env, logical-plan caches, and every schema fingerprint.
Known touch: `src/production_provider_recipe.rs`, `src/production_query_recipe.rs`,
`src/programmatic_derived_analysis.rs`, `src/fabric/programmatic_{schema,epoch,workspace}.rs`,
`src/fabric/child_session.rs`, resource governance, datafusion cache, observation/proof modules,
semantic query contracts, tests, rules, and just recipes.

**Required changes.**

1. Make `CodeFabricV21Release` the only public production constructor. Provider availability,
   source/repository identity, explicit policy/access/resource limits, roots, and credentials are
   its variable inputs; schemas, programs, closures, and producer functions are private outputs.
2. Replace name-substring semantic inference with exhaustive provider relation enums and
   `ProviderRelationDescriptor` field-role matches. Adding a relation/field without a role must fail
   compilation or release construction.
3. Construct one Arrow `SchemaRef` from each descriptor, use it for IPC/admission, and use
   `LogicalPlan::schema()`/provider schema for derived phases. Remove fake `pass\n` source execution,
   `Debug` datatype hashing, duplicate schema identity, and declaration-as-authority.
4. Lower transformations and the eight query programs into native `Expr`/`LogicalPlanBuilder`
   operations. Select a UDF, table function, logical node, or physical node only when the next
   higher native rung cannot preserve accepted semantics; record and test the full contract.
5. Build one governed `RuntimeEnv` per workspace authority domain with explicit memory, spill/temp,
   object-store, scheduler, and cache policy. Build one root epoch session and fresh reduced child
   sessions for each authorization closure.
6. Reconstruct transitive provider/view dependency closure in each child. Retain
   `IdentityPreservingViewTable` only while an executable native-type fault proves it is necessary
   across analyzed, optimized, physical, and batch schemas.
7. Keep metadata, file-statistics, object-list, and logical-plan caches bounded and keyed by the
   complete authority/policy/configuration closure. Never cache physical plans, results, semantic
   current selection, or authorization-derived providers across incompatible closures.
8. Construct query authority from the actual sealed/reopened epoch. A caller cannot inject an
   alternative query catalog, producer map, schema census, or test closure into production.

**Legacy disposition and decommission.** Begins DB10. Delete static/provider-name schemas,
generated kind/capability/producer/query registries, model-pinned compilers, parent-session name
filters, SQL/name bypasses, duplicate schema hashes, and cache-as-authority routes after target
consumers pass. Preserve application-owned provider/view wrappers only for proved contracts.

**Acceptance checks.**

Executable oracle: `datafusion-contract-matrix-integrity-check`
Governed criterion: `PC-WP31-INT`

Executable oracle: `datafusion-plan-schema-cache-check`
Governed criterion: `PC-WP31-BEH`

Executable oracle: `caller-defined-semantic-authority-denial-check`
Governed criterion: `PC-WP31-NEG`

Executable oracle: `datafusion-cache-resource-operations-check`
Governed criterion: `PC-WP31-OPS`

**Oracle category fault contract.** `INT` adds a provider field/query form without exhaustive
release ownership; `BEH` mutates a typed operand/input and requires decoded plan output/schema to
change; `NEG` injects an alternative caller catalog, leaked transitive provider, fake schema run,
name heuristic, or cache-selected authority; `OPS` varies cache pressure/runtime resources and
requires bounded eviction with identical decoded semantics.

**Edit-local gates.** Focused Rust format/check/test/Clippy, DataFusion contract tests,
`just stable-graph-check`, targeted governance rules, and a compile probe for every selected native
or extension API.

**Packet-local gates.** Run the four named recipes independently. The positive oracle compares
decoded rows/schema/metadata, not plan text; the negative child-catalog fixture retains an
unauthorized provider through a view and must fail.

**Milestone.** Contributes to M04 and opens WP32/WP34.

**Replan triggers.** Stop if provider schema requires executing a provider, release programs cannot
be reconstructed from compiled code plus explicit inputs, native DataFusion loses required field
identity/authorization closure, or the selected extension rung cannot implement its complete
resource/schema/statistics contract.

**Rollback and recovery.** A failed candidate session is dropped before publication. Cache loss
must cause recomputation, never semantic loss; authorization failure destroys the child session and
leaves the root epoch unchanged.

**Conditional exemplars.** `IdentityPreservingViewTable` and custom extension nodes are temporary
or permanent only with their named failing native control. DataFrame APIs may assist construction,
but terminal execution must retain the stream and exact schema authority.

### WP32 — Implement lawful genesis, exact activation, and reconstruction

**Outcome.** A recovered pre-epoch command actor can create the first epoch from an empty head,
return one coherent `SelectedEpochRecord`, install/swap exact active authority, and reconstruct the
same selected version after restart without a process-local candidate, cache, SQLite head, or
`latest` lookup.

**Dependencies.** WP29, WP30, WP31, and WP33. It may execute in parallel with WP34/WP35 only under
the overlap rule in §3.2.

**Target invariants.** I4-03--I4-07, I4-11, I4-17--I4-19, I4-21; P3, P9--P12, P16--P18,
P20--P21, P23--P25, P27, P30, P32--P35.

**Design and library references.** Design v3 §§6.3--6.8, 7.6, 11.1--11.2, 13.3, 14.1,
14.3--14.4; successor FAB/LIFE; Delta alignment state/read/write/transaction/query/maintenance/
provenance flows and App. B/D; delta-rs exact loading §§3, 5--7, 9--13.

**Change surface / preflight / known touch.** Trace `current_snapshot`, `head_event`,
`ActivationHeadMissing`, `ExpectedHead::Empty`, direct seed functions, writer generations, command
identity, activation append/readback, exact version reopen, admission close/swap/reopen, CDF gaps,
maintenance, and SQLite state. Known touch: programmatic workspace/epoch/activation modules,
activation control/transaction/admission/ports/SQLite, command actor/runtime/manager/effects,
writer lease/generation, epoch runtime/admission, Delta adapters, tests, and recipes.

**Required changes.**

1. Construct `PreEpochWorkspace` after daemon/writer lease and command recovery. An empty exact
   activation-control read yields `GenesisRequired`, not an error, inferred empty fabric, or direct
   seed permission.
2. Build/prove the candidate from release-owned providers/transformations, then submit one
   `ActivateGenesis { expected_head: Empty, ... }` through the same command actor used later.
   Duplicate delivery converges by normalized command identity.
3. Append once with zero library retry, read back one coherent control horizon, and construct
   `SelectedEpochRecord { event, table_version_vector, writer_fence, control_horizon,
   proof_reference }`. No field may be independently selected or recomputed from a receipt.
4. Reopen every Delta table at the selected exact version, validate read protocol/features,
   rebuild the release-owned session/query authority, verify schema/proof/producer closure by
   execution, and atomically install with admission closed until lifecycle Ready.
5. Implement in-process activation: close new admission, preserve old query leases, revalidate
   head/fence, append/read back once, advance horizon, build/install new active workspace, reopen
   admission, then acknowledge.
6. Inject faults after close, append, readback, horizon advance, swap, reopen, and acknowledge.
   Unknown append outcomes read the horizon and reconstruct or enter `FailedClosed`; no blind retry
   and no early admission.
7. Prove two versions with different decoded rows, restart selecting the older activation-selected
   version, two successive activations, and deletion of SQLite/cache state without semantic-head
   change. CDF/checkpoint gaps emit explicit rebuild/fail-closed state.
8. Remove remaining old epoch/current selectors, candidate-required recovery, receipt authority,
   direct mutation, raw Delta listing/Parquet authority, and activation-specific DB09 debt. Protect
   selected versions from vacuum/retention while epoch/result leases exist.

**Legacy disposition and decommission.** Completes DB09 and the activation portion of DB11. Writer
lease, expected-head checks, command idempotency, reconciliation, and exact Delta adapters are
retained inward. Old epoch types, dual handles, model/current pointers, direct seed/mutation,
candidate caches, and receipt selectors are deleted.

**Acceptance checks.**

Executable oracle: `delta-durability-protocol-integrity-check`
Governed criterion: `PC-WP32-INT`

Executable oracle: `delta-exact-reconstruction-v4-check`
Governed criterion: `PC-WP32-BEH`

Executable oracle: `activation-receipt-nonauthority-check`
Governed criterion: `PC-WP32-NEG`

Executable oracle: `candidate-free-recovery-check`
Governed criterion: `PC-WP32-OPS`

**Oracle category fault contract.** `INT` corrupts one event/vector/fence/horizon/proof binding or
unsupported Delta feature declaration; `BEH` proves decoded rows come from the selected exact
version, including an older one; `NEG` attempts receipt/cache/SQLite/latest/direct-seed authority;
`OPS` injects every activation boundary failure, cache loss, CDF gap, restart, and maintenance race.

**Edit-local gates.** Focused Rust format/check/test/Clippy, exact delta-rs compile probes,
`just stable-graph-check`, activation/command/lease tests, and temp-root restart cases.

**Packet-local gates.** Run all four recipes and report exact selected versions and decoded relation
assertions. The candidate-free oracle must delete non-authoritative temporal/cache state before
reopening.

**Milestone.** Contributes to M04 and closes DB09 with WP30.

**Replan triggers.** Stop if one control read cannot return a coherent event/vector/fence/horizon,
recovery requires a process-local candidate/catalog clone, the pinned Delta protocol gates cannot
validate the operation, or deployment requires distributed/multi-host fencing.

**Rollback and recovery.** Before append, discard the candidate. After append uncertainty, read and
reconcile. After target-format mutation, repair forward through the command actor. Existing queries
retain old leases; new queries wait for the atomic install.

**Conditional exemplars.** High-level `DeltaTableBuilder::from_url(...).with_version(v).load()`,
`update_datafusion_session`, `table_provider`, and bounded CDF builders are preferred exact-pin
paths; concrete names must be compile-probed against revision `43a0cf10`.

### WP34 — Close exact provider descriptors, IPC, admission, and trust

**Outcome.** Native syntax, Pyrefly, and rustc lanes emit exact application-owned relation batches
or explicit typed gaps. One exhaustive descriptor supplies schema/roles/identity/coverage/
provenance, and all-lanes preflight plus transactional admission installs only trusted batches.

**Dependencies.** WP31 and WP33.

**Target invariants.** I4-01--I4-03, I4-08, I4-18--I4-20; P1--P10, P12--P16, P18--P20,
P23, P26--P28, P30, P32--P35.

**Design and library references.** Design v3 §§6.2, 7.1, 13.4, 14.2 and LD3-01; successor ONT/GEN/
FAB; DataFusion provider/schema/interop flows CAT/SRC/SCH/INT/TST; Arrow arrays/schema/IPC §§3--6,
10; provider library references for tree-sitter, Ruff, Pyrefly, and rustc/MIR.

**Change surface / preflight / known touch.** Trace provider enum definitions, schema carriers,
name inference, field-role matches, IPC framing, service/process launch, coverage/remainder rows,
admission preparation/commit, and generated provider protos. Known touch:
`src/production_provider_recipe.rs`, native/Pyrefly/rustc relation schema and provider boundary/
admission modules, relation IPC/service, sidecar/extractor services and auxiliary roots, provider
protos/bindings, tests, rules, and recipes.

**Required changes.**

1. Define exact descriptors for every relation and field: native symbol/lane, Arrow field,
   canonical/provider-local identity role, coordinate/raw-kind/diagnostic/retention/provenance role,
   and required/optional coverage.
2. Delete semantic inference from field/relation names, fake `pass\n` schema runs, `Debug` datatype
   fingerprints, opaque/cold JSON, DTO mirrors, and duplicate schema declarations.
3. Preserve raw and normalized kinds and explicit source coordinates. Provider-local keys remain
   provenance; unsupported canonical seams emit capability gaps rather than counterfeit identity.
4. Keep relation-scoped Arrow IPC with explicit schema/limits/sequence/completion/checksum framing.
   Rust/Python process fixtures are independently generated and test truncation, extra/missing
   relations, wrong schema, duplicates, and cancellation.
5. Perform all-lanes prepare/validate before transactional session registration. Required absence,
   optional absence, provider failure, compile failure, and new raw kinds produce the exact typed
   remainder/capability behavior from the successor GEN authority.
6. Bind admitted batches to the actual release/provider/source/input identities and prove no test
   batch, alternate schema registry, or uncontained process launcher can enter production.

**Legacy disposition and decommission.** Executes provider portions of DB10. Remove old admission
overloads, opaque payloads, static provider/kind/capability registries, schema carriers, summaries/
debug substitutes, uncontained launchers, and duplicate fixtures after exact lanes pass. Preserve
process isolation, generated provider control contracts, Arrow IPC, canonical ID primitives, and
explicit gaps.

**Acceptance checks.**

Executable oracle: `provider-relation-descriptor-contract-check`
Governed criterion: `PC-WP34-INT`

Executable oracle: `exact-provider-batch-check`
Governed criterion: `PC-WP34-BEH`

Executable oracle: `provider-gap-schema-shortcut-rejection-check`
Governed criterion: `PC-WP34-NEG`

Executable oracle: `relation-ipc-provider-operations-check`
Governed criterion: `PC-WP34-OPS`

**Oracle category fault contract.** `INT` omits a descriptor role/coverage row or corrupts IPC
contract identity; `BEH` independently supplies exact provider facts and decodes admitted rows;
`NEG` adds a field without a role, supplies fake/empty success, wrong schema, name heuristic, or
provider-local canonical key; `OPS` faults truncation, process loss, backpressure, cancellation,
partial lanes, and restart cleanup.

**Edit-local gates.** Format/check/test each touched domain, shared-wire provider interoperation tests,
extractor/sidecar identity gates, targeted governance rules, and stable graph checks when manifests
or generated bindings change.

**Packet-local gates.** Run the four packet recipes with exact relation/field coverage counts and
committed invalid fixtures. Counts prove coverage only; decoded values and gaps prove behavior.

**Milestone.** Contributes to M04 and DB10.

**Replan triggers.** Stop if a schema cannot be constructed without provider execution, a required
provider-native field cannot be represented in Arrow, canonical identity cannot be obtained without
an explicit downgrade, or an auxiliary process cannot meet bounded IPC/cancellation behavior.

**Rollback and recovery.** No lane registers until all prepared lanes validate. On provider/process
failure, discard prepared batches, terminate process groups, emit the governed gap, and leave the
previous active workspace unchanged.

**Conditional exemplars.** Bounded `MemTable` remains appropriate for admitted provider batches;
larger/durable products follow the successor FAB classification rather than forcing every provider
through one storage form.

### WP35 — Close transformations, application analyses, and proof closure

**Outcome.** Every normalized, authority/conflict, unknown/remainder, graph, and application-derived
relation has exactly one release-owned producer or an explicit unsupported remainder. Candidate
catalog observation reaches bounded fixed-point closure and proof is executed over actual rows.

**Dependencies.** WP31, WP33, and WP34.

**Target invariants.** I4-01--I4-03, I4-08, I4-18--I4-20; P1--P16, P18--P20, P23--P25,
P27--P28, P30, P32--P35.

**Design and library references.** Design v3 §§6.2, 7.2--7.3, 13.3, 14.2; successor ONT/GEN/FAB;
DataFusion calculation/plan/provenance flows EXP/LOG/OBS/TST; Arrow kernels/compute §§7--8;
petgraph reference only where graph semantics require a transient graph rung.

**Change surface / preflight / known touch.** Map every `ProgrammaticTransformation`, analysis
producer, normalization/authority/unknown rule, graph program, closure resolver, fixed-point
observation, proof row, and old procedural/generated producer. Known touch:
`src/programmatic_derived_analysis.rs`, Python/Rust/common analysis modules, graph program,
derived-producer closure, programmatic observations/epoch, production query recipe inputs, tests,
rules, acceptance fixtures, and just recipes.

**Required changes.**

1. Make all transformation and analysis constructors private exhaustive methods of the compiled
   release; a caller can supply neither a producer closure nor alternative program.
2. Express normalization, provider authority/conflict retention, explicit unknown/remainder, and
   derived calculations as typed native DataFusion plans at the highest viable rung.
3. Preserve provider-native plus normalized values, provenance, conflicts, multi-candidate facts,
   and capability gaps. Missing output never becomes an empty proof of absence.
4. Resolve producer dependencies from the installed candidate catalog. Each required family has
   exactly one producer; zero or multiple producers emits the governed gap/error and blocks a false
   proved epoch.
5. Derive relation, field, schema, dependency, and provenance observations to bounded fixed point
   from the actual session. Execute proof queries; do not accept generated censuses, expected row
   counts, digest equality, or plan text as semantic closure.
6. Retain a transient petgraph/application graph only where native relational recursion cannot
   express the bounded analysis, with canonical identities as weights and no persisted graph index.
7. Inject operand, authority, missing-provider, conflict, unknown, and producer ambiguity faults and
   require decoded derived/proof relations to change or reject exactly as specified.

**Legacy disposition and decommission.** Executes analysis/graph portions of DB10. Delete generated
producer registries, procedural duplicate analyses, summaries/debug substitutes, persisted graph
indices, replay compilers, fixed crosswalks, and old model pins. Preserve the smallest proved
transient graph kernel and application-owned typed transformations.

**Acceptance checks.**

Executable oracle: `analysis-producer-contract-integrity-check`
Governed criterion: `PC-WP35-INT`

Executable oracle: `analysis-producer-semantic-check`
Governed criterion: `PC-WP35-BEH`

Executable oracle: `ambiguous-producer-empty-success-rejection-check`
Governed criterion: `PC-WP35-NEG`

Executable oracle: `analysis-fixed-point-resource-check`
Governed criterion: `PC-WP35-OPS`

**Oracle category fault contract.** `INT` breaks a producer/dependency/proof mapping; `BEH` mutates
one typed operand/authority input and distinguishes decoded derived rows; `NEG` supplies zero/multiple
producers, missing provider output, or forbidden empty success; `OPS` varies iteration/resource
bounds, cancellation, and restart while preserving deterministic semantic closure.

**Edit-local gates.** Focused Rust format/check/test/Clippy, DataFusion plan/schema tests, graph
rung tests where applicable, and targeted expectation/causal-fault checks.

**Packet-local gates.** Run all four recipes. Each selected producer and remainder family is
reported; the positive oracle uses WP33 expectations and the negative oracle proves no zero-case
selector passes silently.

**Milestone.** Contributes to M04 and DB10.

**Replan triggers.** Stop if a released analysis cannot be reconstructed from compiled release plus
explicit inputs, native DataFusion cannot express required semantics at an auditable rung, graph
identity requires provider-local indices, or fixed-point closure lacks a finite governed bound.

**Rollback and recovery.** Failed analysis/proof keeps the candidate private and emits explicit
capability/proof state. It never publishes partial derived authority or falls back to a predecessor
producer.

**Conditional exemplars.** Petgraph SCC/shortest-path/recursion kernels are selected only by a
documented semantic need; a native recursive DataFusion plan remains preferred where it preserves
identity, bounds, and optimizer visibility.

### WP36 — Close query coordination, streamed pages, and result packages

**Outcome.** All eight release-owned requests execute through authorized DataFusion child sessions
under one bounded `QueryCoordinator`. Execution remains streamed into independently decodable Arrow
pages and one immutable result package; idempotency, events, retention, cancellation, and restart
state share the same authority before any transport adapter is added.

**Dependencies.** WP31, WP32, WP33, WP34, and WP35.

**Target invariants.** I4-01--I4-02, I4-05--I4-11, I4-15, I4-19--I4-22; P3, P6--P11,
P13--P18, P20--P25, P27, P30, P32--P35.

**Design and library references.** Design v3 §§7.2--7.5, 8, 9.6, 13.3--13.4, 14.6, 15 and
LD3-01--LD3-06, LD3-13; design v4 §§1, 2; successor QRY/FAB/LIFE/SRV; DataFusion plan/execution/
governance flows LOG/PHY/RUN/INT/GOV/OBS/TST; Arrow IPC §§5--6, 10, 28; object-store 0.13.2 API;
Tonic streaming/backpressure §§19--20 only as a downstream constraint.

**Change surface / preflight / known touch.** Trace every semantic request compiler/backend,
freshness barrier, handle/session/idempotency/task/event/result map, spawn, event vector, batch
collection, IPC buffer, published-result registry, resource lease/read/release, restart recovery,
and cache. Known touch: `src/query_service.rs`, programmatic query backend, relational query runtime,
query artifacts/contracts/ingress, child sessions/resource governance, datafusion cache, graph/
producer closure, Arrow result resource/published result, tests, rules, and just recipes.

**Required changes.**

1. Compile all eight typed request forms only through the sealed/reopened epoch's release recipe.
   Strictly canonicalize/validate request bytes and derive the full normalized operation identity;
   no public SQL, plan, table/function name, or caller-supplied query catalog exists.
2. Add one `QueryCoordinator` that owns global/workspace/principal admission and fairness, queued/
   running capacity, journal bytes/events, idempotency, tasks, deadlines, cancellation, result
   bytes/pages, leases, expiry, terminal replay, and tombstones. Reserve all necessary capacity
   before returning an accepted handle.
3. Bind idempotency to every meaning-bearing operation field. Exact replay returns the original
   acceptance; any changed field returns typed conflict. Entries expire under one observable policy.
4. Derive freshness from lifecycle event watermark, source generation, activation head, selected
   epoch, and remaining budget. Remove `FreshnessBarrier::default()` and RPC/backend-owned competing
   barriers.
5. Keep control events to `SnapshotPinned`, coalesced `Progress`, `ResultReady`, and `Terminal`.
   Allocate sequence after progress coalescing, reserve terminal capacity, bind the opaque cursor to
   content/principal/generation/profile/expiry, and never place response bytes in the journal.
6. Compile-probe `execute_logical_plan -> execute_stream` and consume the
   `SendableRecordBatchStream` under cancellation/deadline/memory accounting. Forbid production
   `collect`, `collect_partitioned`, unbounded `Vec<RecordBatch>`, and whole-result IPC bytes.
7. Encode each bounded page with a fresh Arrow IPC stream writer and independently reopen it. Use a
   page-local bounded buffer, then create-only object publication and manifest-last seal; if local
   object-store semantics cannot provide atomic create, use a private temporary file plus no-replace
   rename behind `ResultObjectSink`.
8. Publish one `ResultPackage` owning the canonical semantic response envelope, ordered manifest,
   pages, schema/coverage/provenance, leases, release/tombstone, and cleanup. A small bounded JSON row
   projection derives from that sealed package; it is not a second result authority.
9. Prove cancellation during freshness, planning, execution, page encoding/write, seal, and read;
   slow consumers remain within the declared pool plus bounded buffers. Restart reopens sealed
   terminal/packages or marks unsealed work `LOST`; it never reruns under another epoch.

**Legacy disposition and decommission.** Completes query/result portions of DB10 and prepares DB11.
Delete fixed query crosswalks, old model pins, default backend, parent-catalog leaks, independent
maps/vectors, one-chunk assumptions, complete IPC `Arc<[u8]>`, whole-result joins, unbounded queues,
and result bytes in events. Retain one bounded package store and authorized DataFusion execution.

**Acceptance checks.**

Executable oracle: `query-result-package-contract-integrity-check`
Governed criterion: `PC-WP36-INT`

Executable oracle: `scheduled-streamed-semantic-query-check`
Governed criterion: `PC-WP36-BEH`

Executable oracle: `query-admission-materialization-bypass-rejection-check`
Governed criterion: `PC-WP36-NEG`

Executable oracle: `query-retention-cancellation-restart-check`
Governed criterion: `PC-WP36-OPS`

**Oracle category fault contract.** `INT` corrupts normalized operation, cursor, page, manifest,
package, or retention bindings; `BEH` runs all eight forms and independently decodes expected
pages/response; `NEG` bypasses coordinator capacity, child authorization, canonical validation, or
introduces whole-result materialization/unbounded ownership; `OPS` faults every stage, slow reads,
expiry/release races, and restart.

**Edit-local gates.** Focused Rust format/check/test/Clippy, exact DataFusion/Arrow/object-store
compile probes, targeted governance rules (`production-query-streaming-only` and
`bounded-query-coordinator-only`), stable graph checks, and resource-bound tests.

**Packet-local gates.** Run all four packet recipes. Vary partition count, batch size, and page size
while comparing decoded ordered rows, schema, coverage, and provenance; record peak resident/page/
journal/task capacity.

**Milestone.** Completes M04 with WP31/WP32/WP34/WP35 and closes DB10.

**Replan triggers.** Stop if DataFusion cannot stream the selected plan with required identity/
ordering, bounded pages cannot be atomically sealed on a supported local store without whole-result
copying, one coordinator cannot reserve capacity before acceptance, or FastMCP acquires a genuinely
streaming resource contract that changes the page model.

**Rollback and recovery.** Partial pages/manifests remain private and are removed by the owning
query. A failed execution reaches one terminal state, releases permits, and never publishes a
package. Sealed packages remain immutable through release/expiry race policy.

**Conditional exemplars.** `object_store::buffered::BufWriter` does not directly satisfy Arrow's
synchronous writer; do not force that composition. A bounded memory page followed by
`put_opts(..., PutMode::Create)` is acceptable only with manifest-last atomic publication and the
declared memory bound.

### WP37 — Deliver the real daemon, gRPC v2, and FastMCP vertical

**Outcome.** The actual `codefabricd` process serves the clean `codefabric.cpgd.v2` contract over
owned UDS endpoints, authenticates supervisor grants and daemon sessions, exposes honest lifecycle
and bounded resources, and is consumed by one eager-session installed FastMCP STDIO package. One
real source-to-FastMCP vertical proves the assembled target; v1 and shortcut serving are absent.

**Dependencies.** WP29, WP30, WP31, WP32, WP33, WP34, WP35, and WP36.

**Target invariants.** I4-01--I4-22; P1--P36.

**Design and library references.** Design v3 §§6.1, 6.4, 6.8, 8--10, 13.4, 14.4--14.7 and
LD3-07--LD3-12, LD3-14 as amended by design v4 §§0--5; successor LIFE/SRV/FAB/QRY; Tonic §§0,
6--8, 12--29, 34--40, 43; grpcio §§8--10, 13--19, 21, 23, 26--30, 35--37; Protobuf §§4--18,
26--30, 37--44; FastMCP §§4--14, 21--22, 29--30, 32--35; Pydantic §§5--10, 16--21, 26, 34,
36, 40, 48.

**Change surface / preflight / known touch.** Trace proto source, descriptor/baseline/census/blob,
Rust/Python generators and bindings, every service/client method, metadata/interceptor, peer
credential, socket bind/unlink, daemon/admin startup, health, errors, deadlines, chunks, Python
channel/client/result/resource/server/settings/models, FastMCP tools/resources/lifespan/progress,
STDIO/package tests, service/package configuration, and v1 consumers. Known touch includes
`contracts/rpc/cpg_query_service*.proto`, `tooling/proto/**`, generated outputs, `src/rpc.rs`,
`src/daemon.rs`, `src/query_service.rs`, the full adapter domain, integration tests, rules, package
metadata/lock, Cargo manifest/lock, justfile, and CI.

**Required changes.**

1. Complete the production lifecycle/atomic workspace integration from WP29/WP32. Every discovery,
   handshake, status, validate/start admission, activation, drain, and shutdown projection reads the
   same lifecycle authority; only `Ready` opens coordinator admission.
2. Add one `codefabric.cpgd.v2` proto and perform source, descriptor, Rust generation, Python
   generation, service/client, package, and test changes in one atomic transaction. Implement the
   nine v4 RPCs and standard health; do not register v1 or a translator.
3. Implement the minimal inherited supervisor-control socketpair and single-use launch grants.
   Handshake consumes a binary bootstrap capability and mints an expiring session bound to peer,
   daemon/supervisor generation, principal, workspaces, operations, profiles, host bounds,
   revocation generation, and expiry.
4. Carry verified UDS peer identity in Tonic request extensions and reauthorize every method,
   query, handle, and resource. Body correlation IDs grant no authority. Reserve transport capacity
   for handshake/status/cancel/release so heavy streams cannot starve control.
5. Add `OwnedUnixSocket` for admin and query endpoints: private no-symlink parent, type/owner/mode
   checks, live probe, stale-only unlink under lease, final permissions, recorded device/inode/
   generation, and replacement-inode-safe shutdown unlink.
6. Serve health as process/service liveness only. Derive one remaining-budget model through Python
   timeout, gRPC deadline, queue/freshness/execution/write/read/cleanup. Map outer errors by standard
   status plus the probed typed detail/metadata code; never parse or expose prose/secrets.
7. Implement `WatchQuery` at-least-once resume and `ReadResource` bounded chunk/range streaming.
   Dropping a watch stops observation, not work; cancellation uses `CancelQuery`; reconnect creates
   one channel, re-handshakes, and watches the accepted query without resubmitting.
8. Build FastMCP lifespan with strict settings, one channel/stub/session manager, bounded readiness
   wait, handshake/profile/reference-index validation, and unconditional close. Honest
   `BOOTSTRAPPING` may yield; incompatibility/authentication/transport failure may not.
9. Register exactly four tools and bounded manifest/page/reference resources. Use
   `Context.report_progress`; keep MCP call, RPC attempt, daemon query, epoch, package, resource, and
   lease IDs distinct. Python forwards bytes and validates presentation only.
10. Launch the real binary and installed adapter package for a real workspace source mutation:
    exact provider batches/gaps -> transformations/proof -> Delta genesis/activation -> atomic
    workspace -> scheduled query -> streamed package -> v2 UDS -> FastMCP response/resource.
11. Delete v1 runtime bindings/services/clients/package payload/operability tests, overlapping
    Stream/Attach and Read/ReleaseResult routes, repeated body authority, lazy handshake, local
    reference synthesis, one-chunk joins, default/in-process backends, blind socket removal, static
    adapter registries, and Python semantic processing.

**Legacy disposition and decommission.** Completes DB11 and the public portion of DB13. Preserve v1
proto/descriptor/allocation fixtures only in an exact non-live historical class. Preserve generated
v2 bindings, strict models, presentation helpers, and bounded source/lifecycle/result mechanisms.
No old client/server execution is a retained capability.

**Acceptance checks.**

Executable oracle: `public-lifecycle-wire-contract-integrity-check`
Governed criterion: `PC-WP37-INT`

Executable oracle: `lifecycle-production-vertical-check`
Governed criterion: `PC-WP37-BEH`

Executable oracle: `session-uds-presentation-boundary-rejection-check`
Governed criterion: `PC-WP37-NEG`

Executable oracle: `resource-cancellation-recovery-check`
Governed criterion: `PC-WP37-OPS`

**Oracle category fault contract.** `INT` corrupts v2 descriptor/generation/session/event/resource/
error/lifecycle contracts; `BEH` executes the real four-domain source-to-FastMCP path and decodes
WP33 expectations; `NEG` covers wrong UID/grant/session/generation/workspace/operation/owner,
cursor tamper, unsafe socket, v1 selection, Python semantics, static reference, and STDOUT leakage;
`OPS` covers bootstrapping, reconnect/watch, slow reads, cancellation at every layer, expiry/
release, socket replacement, shutdown, and restart reconstruction.

**Edit-local gates.** Rust and adapter format/check/lint/tests, `proto-check`, `proto-repro-check`,
real UDS interop, generated descriptor assertions, targeted rules (`owned-uds-lifecycle-only`,
`daemon-production-composition-only`, `adapter-live-reference-only`), `adapter-stdio-test`, package
inspection, and stable graph/feature gates when dependencies change.

**Packet-local gates.** Run the four packet recipes against actual subprocesses. The positive gate
must launch `codefabricd` and the installed adapter package; `ProbeService`, injected Rust backends,
and Python stub daemons are permitted only as lower-layer tests and cannot satisfy completion.

**Milestone.** Completes M05 with WP29/WP30, closes DB11, and opens release evidence.

**Replan triggers.** Stop if the supervisor grant root cannot be safely delivered by a supported
launcher, UDS peer identity cannot reach async authorization, the v2 service duplicates semantic
authority, FastMCP cannot bound materialized resources, or the real four-domain process topology
cannot meet cancellation/shutdown ownership.

**Rollback and recovery.** Failed startup leaves admission closed and removes only owned endpoints.
Old daemon-generation sessions fail; reconnect re-handshakes. Accepted unsealed work becomes
`LOST`, sealed resources follow lease policy, and no v1/default service is selected.

**Conditional exemplars.** Adopt `tonic-health` after its exact compile probe. Adopt
`tonic-types`/`grpcio-status` only if a bilateral runtime probe justifies richer typed details;
otherwise fixed bounded binary/ASCII trailing metadata plus standard status is the target.

### WP38 — Issue and execute first-principles successor evidence

**Outcome.** One reviewed evidence transaction proves the target through independently authored
decoded values, causal faults, exact durable readback, real process boundaries, clean
reconstruction, and bounded failure behavior. Predecessor output, hashes, counts, and plan text do
not decide semantic acceptance.

**Dependencies.** WP37.

**Target invariants.** I4-01--I4-22; P9--P10, P18--P22, P24--P30, P36.

**Design and library references.** Design v3 §§14--15, 17 and v4 §5; successor SUITE proof/release
contracts; DataFusion/Arrow and Delta TST families selected by WP31--WP37; Tonic §39.10, grpcio §26,
Protobuf §37, FastMCP §30, Pydantic §48.

**Change surface / preflight / known touch.** Inventory all expectations, goldens, comparators,
digests/counts, captures, fixtures, fault injectors, evidence DAG/transaction, selector, decoder,
release recipe, and test filter. Trace expected-value provenance and whether any imports target or
historical implementation output. Known touch: `contracts/acceptance/relational-fabric-v4/**`,
release/evidence tooling and tests, integration/vertical harnesses, just recipes, and CI evidence
jobs.

**Required changes.**

1. Freeze WP33 expectations/review identities and map each release claim to typed input, decoded
   expected output, negative case, causal fault, owning packet oracle, and limitation.
2. Execute provider, transformation, analysis, all eight query, activation, lifecycle, v2 wire,
   session/security, resource, FastMCP, recovery, and zero-state claims through WP37's production
   processes.
3. Compare decoded rows/schema/order/null/unknown/coverage/provenance, typed control states, exact
   table versions/horizon, and strict public projections. Digest/count assertions remain secondary
   integrity/limit checks.
4. Create one discriminating causal fault per claim family by changing an authoritative input,
   provider batch, operand, producer, plan/schema, authorization, activation vector, resource bound,
   session/event/cursor, or result. Text-only changes do not qualify for semantic claims.
5. Run clean reconstruction after deleting non-authoritative temporal/cache/result scratch state;
   rerun exact providers from source/input pins and prove the same semantic outcome without comparing
   hashes alone.
6. Make historical v1/v3 comparator/evidence bytes physically unavailable and prove the verdict is
   identical. Preserve them afterwards only as immutable non-live history.
7. Record deferred platform, performance, scheduled, or deep-assurance limitations honestly; no
   unsupported capability is reported green.

**Legacy disposition and decommission.** Begins DB12. Delete producer-generated goldens,
bootstrap/model expectations, mandatory comparators, old client/server interop, digest/count-only
semantic gates, and predecessor agreement from the live evidence DAG after v4 evidence passes.

**Acceptance checks.**

Executable oracle: `production-evidence-input-integrity-check`
Governed criterion: `PC-WP38-INT`

Executable oracle: `first-principles-production-behavior-check`
Governed criterion: `PC-WP38-BEH`

Executable oracle: `causal-fault-discrimination-check`
Governed criterion: `PC-WP38-NEG`

Executable oracle: `clean-reconstruction-evidence-check`
Governed criterion: `PC-WP38-OPS`

**Oracle category fault contract.** `INT` changes a frozen expectation/review/input identity;
`BEH` compares real decoded production outcomes to independent expectations; `NEG` executes causal
and rejection fixtures and proves history absence leaves the verdict unchanged; `OPS` performs
clean reconstruction, cache/temporal loss, security/resource, cancellation, and restart evidence.

**Edit-local gates.** Evidence-tool format/lint/unit tests, targeted production vertical cases,
schema validation, selector nonzero checks, and artifact-contract validation.

**Packet-local gates.** Run all four packet recipes with structured claim/selection/fault output.
The harness must refuse an expectation generated by importing the implementation or decoding its
output into the expected file.

**Milestone.** Contributes to M06 and opens purge.

**Replan triggers.** Stop if an expectation cannot be authored independently, a causal fault does
not discriminate the claim, clean reconstruction requires historical/runtime cache state, or the
real process vertical cannot expose a required observation without adding semantic authority to the
transport.

**Rollback and recovery.** Preserve the last accepted transaction as immutable history; issue a
new version for corrected expectations. Never rewrite an accepted expected value to make current
implementation pass.

**Conditional exemplars.** A legacy comparison may be run locally as a labelled diagnostic only
when its bytes are absent from every pass/fail dependency. It is never required and is deleted from
release automation.

### WP39 — Purge remaining generated, governance, package, and recipe residue

**Outcome.** Every displaced live authority, generated projection, compatibility runtime, feature,
target, dependency, package entry, recipe, workflow, rule, fixture, service configuration, and
ignored source is either physically absent or belongs to one exact retained class with a live target
consumer. Clean builds/packages expose only the successor surface.

**Dependencies.** WP30 and WP38.

**Target invariants.** I4-01--I4-02, I4-12, I4-16--I4-20; P3, P9--P10, P18, P20--P21,
P25--P31, P36.

**Design and library references.** Design v3 §§11.2--11.3, 14.8 and v4 §§0, 4--5; successor SUITE
legacy/release rules; DB09--DB13; repository package/feature/test architecture in AGENTS.md.

**Change surface / preflight / known touch.** Start from a fresh tracked/untracked/ignored/source/
syntax/Cargo/uv/package/recipe/workflow/service/rule/generated-include inventory. Compare against all
L-20--L-55 and DB09--DB13 rows and current wheel/sdist/Cargo target contents. Known touch: legacy
and evidence tooling, acceptance/governance contracts, Cargo/uv manifests and locks, package data,
generated outputs, rules/snapshots, scripts, justfile, CI, service configuration, tests, and files
already deleted in the dirty tree.

**Required changes.**

1. Delete remaining generated model/schema/identity/result/table/package authorities, stale static
   adapter references, v1 runtime bindings/clients/servers, old descriptor-generation targets,
   bootstrap expectations, comparator DAGs, detector/count registries, and obsolete governance
   scripts/rules/fixtures.
2. Remove retired Rust bins/features/dependencies/build edges, Python dependencies/modules/package
   data, recipes/workflow jobs, service units, fuzz/snapshot/corpus entries, and installed artifacts
   after reachability proves no retained consumer.
3. Preserve only current v2 generation plus explicitly historical v1 proto/descriptors/allocations;
   ensure history cannot be imported, generated, compiled, installed, or selected.
4. Keep negative-oracle implementations only in exact test/tooling paths that cannot enter runtime
   or packages. Replace broad directory exclusions with named class/path rules.
5. Update remaining-zero-state and package-build tooling to report paths, syntax/imports, Cargo
   targets/features, Python payload, generated includes, recipes, workflows, services, rules/tests,
   ignored live sources, skips, parse failures, overlaps, and unmatched candidates.
6. Reproduce v2 descriptors/bindings through the sole generator, rebuild all four domains and the
   adapter wheel/sdist from the purged tree, and inspect contents. Do not regenerate or execute v1.

**Legacy disposition and decommission.** Completes DB12 and DB13 except the dormant forward-cutover
family reserved for WP41 after sole-target activation proof. DB09--DB11 deletions are reverified.
Immutable history remains exact and non-live; no broad `docs/**` exclusion substitutes for path
classification.

**Acceptance checks.**

Executable oracle: `post-purge-surface-inventory-integrity-check`
Governed criterion: `PC-WP39-INT`

Executable oracle: `retained-target-post-purge-behavior-check`
Governed criterion: `PC-WP39-BEH`

Executable oracle: `remaining-live-authority-zero-state-check`
Governed criterion: `PC-WP39-NEG`

Executable oracle: `post-purge-package-build-operations-check`
Governed criterion: `PC-WP39-OPS`

**Oracle category fault contract.** `INT` leaves an unclassified/overlapping/skipped candidate or
invalid retained-class link; `BEH` reruns retained production behavior after purge; `NEG` injects
one live legacy/v1/generated route in every inventory dimension; `OPS` clean-builds and inspects all
domains/packages/generated v2 artifacts with historical inputs unavailable.

**Edit-local gates.** Targeted tooling/unit tests, governance rules, `just remaining-legacy-zero-state-check`,
`just stable-graph-check`, `proto-check`, `proto-repro-check`, feature/target checks, adapter package
tests, and exact package-content inspection.

**Packet-local gates.** Run the four recipes from a clean temporary build/package output while
preserving the user's main target tree. Every negative fixture must be discovered and no selector
may pass with zero candidates.

**Milestone.** Contributes to M06 and closes DB12/DB13.

**Replan triggers.** Stop if a retained historical artifact remains a build/runtime/package input,
a dependency or generated target cannot be attributed to a successor consumer, or zero-state
coverage cannot account for ignored/unparsed/skipped paths without reaching secrets/vendor output.

**Rollback and recovery.** Restore only task-owned deletions that break a named successor consumer;
record the missing disposition and replan that row. Never restore a family merely to make an old
recipe or fixture pass.

**Conditional exemplars.** A generated v1 descriptor may remain as immutable bytes for allocation
history under a non-live path; no v1 generator, runtime binding, service, client, wheel payload, or
interop test remains.

### WP40 — Re-execute post-purge release and resource evidence

**Outcome.** The purged successor passes the complete semantic, security, recovery, package, and
representative performance matrix at one candidate HEAD. Measurements characterize the target and
permit tuning only when a predeclared resource envelope and semantic equality justify it.

**Dependencies.** WP39.

**Target invariants.** I4-01--I4-22; P9--P10, P18--P25, P27--P30, P36.

**Design and library references.** Design v3 §§14--15, 17 and v4 §5; successor SUITE release gate;
DataFusion/Arrow TST-01--TST-14, Delta release gate App. D, Tonic performance/testing §§38--39,
grpcio performance/testing §§25--29, FastMCP/Pydantic performance/testing sections.

**Change surface / preflight / known touch.** Inventory every release aggregate, test filter,
environment record, performance workload, semantic equality oracle, resource metric, threshold,
retry, and platform limitation. Confirm each final command still selects the purged v4 surface and
does not invoke v1/WP28/predecessor recipes. Known touch: release/evidence/performance tooling,
contracts, tests, justfile, CI, and report artifacts.

**Required changes.**

1. Re-execute all WP29--WP39 packet oracles from the purged tree with WP33 expectations and record
   per-command exit code, selected count, fault result, environment, and exact candidate HEAD.
2. Run semantic mixed-language, v2 wire, session/UDS security, cancellation/retention, clean
   reconstruction, package, feature, and four-domain gates independently before any aggregate.
3. Add `daemon-boundary-bench` for cold genesis, warm exact recovery, transport bind,
   bootstrapping-to-Ready, eight-form execution, first page, throughput, peak RSS, page/batch policy,
   fairness/queue accuracy, cancellation, reconnect/handshake, result-store IO, and drain time.
4. Declare representative source/query/concurrency/result workloads, supported environment,
   repetitions/distribution, warm/cold conditions, resource envelope, and decoded semantic equality
   before measuring. A single timing or cumulative cache ratio is not evidence.
5. Compare the old collect-based code only if still available as an isolated diagnostic; it is not
   required and cannot enter acceptance. Target limits are judged against the declared functional
   envelope.
6. Tune page sizes, spill, retention, compression, HTTP/2, keepalive, or storage thresholds only
   when measurement shows a specific bottleneck and all semantic/resource/recovery oracles remain
   green. Otherwise retain safe defaults.
7. Emit one versioned release-evidence matrix with limitations; no digest/status/proving-commit row
   replaces underlying command and behavioral evidence.

**Legacy disposition and decommission.** Revalidates DB09--DB13 and detaches stale release/
benchmark/comparator routes. No production code is restored for measurement. Temporary benchmark
fixtures are bounded, non-packaged, and not semantic authorities.

**Acceptance checks.**

Executable oracle: `release-evidence-matrix-integrity-check`
Governed criterion: `PC-WP40-INT`

Executable oracle: `post-purge-release-behavior-check`
Governed criterion: `PC-WP40-BEH`

Executable oracle: `history-comparator-independence-check`
Governed criterion: `PC-WP40-NEG`

Executable oracle: `daemon-boundary-bench`
Governed criterion: `PC-WP40-OPS`

**Oracle category fault contract.** `INT` removes a selected command/fault/environment/limitation
from the matrix; `BEH` reruns decoded production semantics after purge; `NEG` makes history/v1/
comparator bytes unavailable and injects a stale selector; `OPS` violates one declared bound or
semantic equality under representative load and must block tuning/release.

**Edit-local gates.** Evidence/performance-tool unit tests, deterministic workload validation,
targeted domain checks, and review of measurement output for environment/distribution/resource/
semantic fields.

**Packet-local gates.** Run all four recipes independently, then the release matrix aggregate. The
benchmark is non-mutating to source and uses isolated temp roots; report rather than hide unsupported
platform/workload cases.

**Milestone.** Completes M06 and opens FreshActivation.

**Replan triggers.** Stop if representative load exceeds the accepted resource envelope, the
stream/page/session topology is the cause, a tuning requires semantic duplication/unbounded state,
or the supported environment cannot reproduce the workload.

**Rollback and recovery.** Revert only unproved tuning to the last semantically green configuration.
Measurements remain versioned evidence with their environment; they are never rewritten to match a
new result.

**Conditional exemplars.** HTTP/2 windows, keepalive, compression, multipart thresholds, page size,
spill, and TTL are intentionally unspecified until this packet measures them.

### WP41 — Execute fresh successor activation and prove sole target authority

**Outcome.** A clean target installation creates/reconstructs one workspace through FreshActivation,
owns daemon/writer/UDS/activation authority before and after restart, mutates forward only, repairs
unknown outcomes, and contains no dormant predecessor handoff/cutover machinery.

**Dependencies.** WP40.

**Target invariants.** I4-03--I4-07, I4-11--I4-21; P3, P9--P11, P16--P18, P20--P25,
P27--P30, P32--P35.

**Design and library references.** Design v3 §§11.1--11.3, 13.5, 14.8 and v4 §§0, 4--5;
successor SUITE/LIFE/FAB/RM FreshActivation contracts; Delta transaction/recovery/maintenance flows;
Tonic UDS/shutdown §§22--24, 37, 40.

**Change surface / preflight / known touch.** Perform a read-only deployment census for installed
packages/binaries, services, runtime dirs/sockets, daemon/writer leases, activation heads, configs,
features/targets, recipes, and live processes. Trace every `forward_cutover`,
`ForwardCutover`, `CutoverStatus`, `cutover-status`, `with_forward_cutover`, predecessor release/
revocation/rollback/bridge/reboot selector across source, tests, contracts, tooling, rules, hidden
paths, and packages. Known touch: both forward-cutover modules, daemon/admin/CLI imports and fields,
command-factory wiring, manifests/journals, tests/fixtures/recipes/certification, and service config.

**Required changes.**

1. Confirm the authorized profile: no deployed predecessor owns or owned the workspace UDS, writer
   lease, production package/service, or activation head. Contrary evidence stops this packet and
   reopens a separate one-shot AuthorityHandoff design.
2. Install only the target production binary/package/service/config. Prove no bootstrap/ontology/
   default/v1/predecessor backend, binary, feature, package, recipe, or service can be selected.
3. Start from an empty head and prove exactly one command-actor genesis, honest bootstrapping,
   exact activation readback, Ready, v2 session/query/resource behavior, and joined shutdown.
4. Restart and prove the same selected exact epoch, sole daemon/writer/socket ownership, invalid old
   daemon sessions, target-only reconnect/handshake, and no duplicate genesis.
5. Inject unknown command/append/readback outcomes and prove admission remains closed until coherent
   reconciliation. After one target-format mutation, inject failure and repair forward through the
   target command/activation path.
6. Prove a live foreign socket is never removed, a proven stale target socket is recovered, and a
   replacement inode survives shutdown. Prove a second target instance cannot acquire daemon/writer
   authority.
7. Delete `src/fabric/forward_cutover.rs`, `src/forward_cutover_controller.rs`, exports/imports,
   `with_forward_cutover`, cutover admin/CLI status, predecessor release/revoke/reboot/rollback/
   bridge vocabulary, manifests/journals, fixtures, recipes, selectors, and certification clauses.
8. Run multidimensional zero state for all target-only and handoff classes while retaining exact
   historical design/plan/review/allocation artifacts as non-live evidence.

**Legacy disposition and decommission.** Completes DB14. Narrow writer lease, generation,
expected-head, command idempotency, physical observation, reconciliation, and fail-closed repair
primitives remain target-owned. No predecessor executable or synthetic deployment is created to
satisfy a test.

**Acceptance checks.**

Executable oracle: `fresh-activation-contract-integrity-check`
Governed criterion: `PC-WP41-INT`

Executable oracle: `fresh-activation-target-authority-check`
Governed criterion: `PC-WP41-BEH`

Executable oracle: `fresh-activation-zero-state-check`
Governed criterion: `PC-WP41-NEG`

Executable oracle: `fresh-activation-reconciliation-check`
Governed criterion: `PC-WP41-OPS`

**Oracle category fault contract.** `INT` corrupts profile/ownership/horizon/zero-state evidence;
`BEH` proves empty-head genesis, target query, mutation, restart, and sole ownership; `NEG` injects a
selectable old/v1/default/handoff route or second owner; `OPS` faults command/activation/socket/
shutdown/restart and proves fail-closed reconciliation plus forward repair.

**Edit-local gates.** Focused Rust/adapter tests for touched cleanup, deployment inventory tests,
owned-socket/session/restart gates, zero-state rules, package/service inspection, and all four domain
format/check/test gates affected by deletion.

**Packet-local gates.** Run the four new FreshActivation recipes. Remove/retire stale
`fenced-authority-cutover-v3-check`, predecessor reboot/revocation, and rollback-to-predecessor
recipes from every active aggregate rather than changing their fixtures to pass.

**Milestone.** Contributes to M07 and closes DB14.

**Replan triggers.** Any real deployed predecessor, cross-host/distributed writer ownership,
unreconstructible target-format mutation, or inability to prove sole endpoint/package/service
ownership reopens design. It does not authorize retaining the dormant controller.

**Rollback and recovery.** Before target mutation, remove the failed target install and retry from
clean FreshActivation state. After mutation, repair forward only. Unknown outcomes read exact
target authority before acting.

**Conditional exemplars.** A future AuthorityHandoff is a separately accepted temporary outer
controller removed after use. No source module or feature for it exists in this FreshActivation
release.

### WP42 — Certify the complete successor at one trusted HEAD

**Outcome.** One trusted HEAD has fresh declared inputs, ancestral proving commits for every active
packet, closed milestones/decommission batches, all derived packet oracles, the real terminal
vertical, four-domain/package proof, target-only zero state, representative resource evidence, and
an independent implementation-review acceptance. Only then may the plan state become complete.

**Dependencies.** WP41.

**Target invariants.** I4-01--I4-22; P1--P36.

**Design and library references.** Accepted design v4 and incorporated v3 target; successor
authoritative suite and RM; all selected library alignment/release/test gates; repository evidence,
state, review, and completion policies.

**Change surface / preflight / known touch.** Recompute declared-input freshness, plan/state/active
identity, packet DAG, proving-commit ancestry, oracle extraction, milestone/DB status, current diff,
package/build graphs, zero-state inventories, environment, and every final recipe implementation.
Known touch: artifact/state/dependency/certification tooling and tests, release evidence, justfile,
CI, final review artifact, and only defects discovered by final execution.

**Required changes.**

1. Derive active packets and their four criteria/oracles from this plan. Reject missing/duplicate/
   zero-selection/miscategorized recipes, non-ancestral proving commits, WP28/M01, hard-coded count
   literals, and stale predecessor/v1 cutover gates.
2. Rerun every WP29--WP42 oracle at the exact candidate HEAD. Preserve individual exit codes,
   selections, faults, and evidence even when an aggregate exists.
3. Rerun authoritative-suite conformance, artifact/state/dependency integrity, stable root,
   extractor, sidecar, adapter, proto-v2 generation, governance, feature matrix, dependency/supply
   chain, package/wheel/sdist, clean reconstruction, security/resource, FreshActivation, zero-state,
   and performance-limit gates.
4. Rerun the real source-mutation -> `codefabricd` -> v2 grpc.aio -> installed FastMCP STDIO
   vertical, cancellation, shutdown, restart, exact reconstruction, and live reference change.
5. Obtain an independent `implementation-review` against design v4, successor suite, this plan,
   state, implementation, library decisions, decommission, and actual behavior. Resolve every
   blocking finding at a new trusted HEAD and rerun affected plus terminal gates.
6. Prove all DB09--DB14 retained/history exclusions are exact and non-live, and that old ontology/
   bootstrap/model/default/v1/forward-cutover authority cannot build, install, bind, serve, write,
   query, or influence acceptance.
7. Mark complete only through the schema-valid state transaction after every condition is true.
   Proving-commit/state hashes attest lineage/integrity, not behavior by themselves.

**Legacy disposition and decommission.** Certifies DB09--DB14 and L-20--L-55. Historical suites,
contracts, plans, states, reviews, allocations, and independently valid KATs remain immutable and
detached. No release-mandatory deletion or proof may be deferred.

**Acceptance checks.**

Executable oracle: `successor-provenance-state-integrity-check`
Governed criterion: `PC-WP42-INT`

Executable oracle: `relational-fabric-v4-certification`
Governed criterion: `PC-WP42-BEH`

Executable oracle: `successor-final-zero-state-check`
Governed criterion: `PC-WP42-NEG`

Executable oracle: `successor-four-domain-release-check`
Governed criterion: `PC-WP42-OPS`

**Oracle category fault contract.** `INT` corrupts plan/state/input/proving ancestry/oracle
derivation; `BEH` perturbs one authoritative source/operand and requires the real terminal vertical
to distinguish it; `NEG` injects one live legacy/v1/handoff/default route in each inventory plane;
`OPS` reruns clean builds/packages, restart/reconstruction, cancellation/shutdown, and representative
resource limits across supported domains.

**Edit-local gates.** Any final repair runs the smallest affected focused gates first, then the
entire terminal matrix. Documentation/state-only updates run artifact/schema/dependency validation
without treating it as behavioral certification.

**Packet-local gates.** Run the four named recipes at one clean candidate HEAD. The certification
recipe invokes, rather than summarizes, the real semantic and operational gates and requires the
independent review result.

**Milestone.** Completes M07 and the plan.

**Replan triggers.** Any unresolved design/implementation review finding, stale input, non-ancestral
proof, unclassified residue, unsupported functional capability, real predecessor discovery, or
resource-envelope failure prevents completion and reopens the owning packet/design boundary.

**Rollback and recovery.** Certification is repeatable and non-mutating except the final governed
state transaction. A failed run leaves the plan executing and records the blocker; it never edits
evidence, exclusions, thresholds, or status to manufacture success.

**Conditional exemplars.** `ci-pr` may aggregate routine gates, but terminal evidence retains each
underlying result. Scheduled/deep assurance is release-mandatory only where the successor suite
declares it; unsupported cases remain explicit limitations rather than green placeholders.

## 5. Milestones

Milestones are derived barriers, not substitutes for packet completion. Their proving commit is the
latest ancestral packet commit at which every exit condition is rerun. M01 does not exist in v4.

### M02 — Successor design authority and expectations are frozen

**Dependencies.** WP33.

**Exit.** One synchronized successor suite incorporates design v4; `codefabric.cpgd.v2` is the sole
target wire authority; independently reviewed expectations and negative fixtures are immutable;
v2.1/v1/v3 inputs remain byte-stable non-live history; WP28/M01 and old-client operability have no
dependency edge.

### M03 — Production kernel exists and displaced bootstrap authority is removed

**Dependencies.** WP29 and WP30.

**Exit.** The real binary reaches one honest phase-typed kernel and one lifecycle authority without
a default/test backend; target consumers own retained primitives; ontology/bootstrap/model/compiler/
generated-schema authority is absent; only explicitly enumerated activation residue remains for
WP32.

### M04 — Compiled semantic, provider, exact epoch, query, and result authority are closed

**Dependencies.** WP31, WP32, WP34, WP35, and WP36.

**Exit.** The compiled release owns exhaustive descriptors/programs/producers; exact provider gaps,
native plans, governed runtime/children, lawful genesis, exact version reconstruction, activation,
all eight requests, bounded coordination, streamed Arrow pages, and one result package pass their
behavioral/failure/resource gates. DB09 and DB10 are closed.

### M05 — Real daemon-to-FastMCP delivery is proved

**Dependencies.** WP37.

**Exit.** Actual `codefabricd`, owned UDS, supervisor grants/sessions, `codefabric.cpgd.v2`, generated
grpc.aio client, and installed FastMCP STDIO package pass one source-to-resource vertical,
bootstrapping/readiness, security, cancellation, reconnect, shutdown, and restart. V1 runtime and
shortcut serving are absent; DB11 is closed.

### M06 — Independent evidence, total purge, and measured release candidate are accepted

**Dependencies.** WP38, WP39, and WP40.

**Exit.** Independently authored decoded expectations, causal faults, clean reconstruction,
history/comparator independence, multidimensional zero state, clean four-domain/package builds, and
representative boundary/resource measurements pass at one candidate HEAD. DB12 and DB13 are closed.

### M07 — Fresh activation and final certification are complete

**Dependencies.** WP41 and WP42.

**Exit.** Empty-head FreshActivation, sole target ownership, target-only restart/forward repair,
unknown-outcome reconciliation, forward-cutover zero state, the derived packet oracle set, terminal
vertical, all final gates, and independent implementation review pass at one trusted HEAD. DB14 is
closed and schema-v2 state is complete.

## 6. Successor L-20--L-55 disposition map

Every row has one target consumer and one negative deletion owner. Existing dirty-tree deletions
remain provisional until the named target/zero-state proof passes. V1 operability is not retained.

| ID | V4 disposition and retained outcome | Replacement / deletion owner | Positive proof | Negative proof |
|---|---|---|---|---|
| L-20 | Delete model-compiler binary/feature/readers; retain independent Protobuf v2 tooling. | WP29 consumer; WP30/WP39 delete. | `programmatic-production-composition-check` | `bootstrap-ontology-authority-zero-state-check` |
| L-21 | Delete DesiredTree/model sync/repro/release tooling; retain successor suite/evidence. | WP33 authority; WP30/WP39 delete. | `successor-authority-expectation-integrity-check` | `remaining-live-authority-zero-state-check` |
| L-22 | Replace static live registries with explicit typed inputs; preserve exact history only. | WP29/WP31 consume; WP30 deletes readers. | `compiled-release-consumer-cutover-check` | `bootstrap-ontology-authority-zero-state-check` |
| L-23 | Delete generated model/provider-kind/manifest products; derive observations live. | WP31/WP36 replace; WP30/WP39 delete. | `datafusion-plan-schema-cache-check` | `remaining-live-authority-zero-state-check` |
| L-24 | Delete generated encoder/domain/model/bundle/registry/result/table authorities; retain intrinsic encoders inward. | WP31/WP36 consume; WP30/WP39 delete. | `datafusion-contract-matrix-integrity-check` | `remaining-live-authority-zero-state-check` |
| L-25 | Replace live v1 query wire with clean generated `codefabric.cpgd.v2`; retain v1 allocation history only. | WP37 replaces/deletes runtime; WP39 verifies packages. | `public-lifecycle-wire-contract-integrity-check` | `session-uds-presentation-boundary-rejection-check` |
| L-26 | Delete adapter fingerprints/static schemas/query registries; use daemon live reference/resources. | WP37 replaces; WP39 deletes payload. | `lifecycle-production-vertical-check` | `remaining-live-authority-zero-state-check` |
| L-27 | Delete installed/resealed ontology bundle/candidate authority. | WP29 consumes release; WP30 deletes. | `programmatic-production-composition-check` | `bootstrap-ontology-authority-zero-state-check` |
| L-28 | Retain native relational semantics; delete replay/generated catalogs. | WP31/WP35 replace; WP30/WP39 delete. | `analysis-producer-semantic-check` | `caller-defined-semantic-authority-denial-check` |
| L-29 | Replace schema/current registries with descriptors, plan/provider schemas, observations, and exact selected record. | WP31/WP32 replace; WP30 deletes. | `datafusion-plan-schema-cache-check` | `activation-receipt-nonauthority-check` |
| L-30 | Replace procedural projection/opaque payload with exact raw batches and typed transforms. | WP34/WP35 replace/delete. | `exact-provider-batch-check` | `provider-gap-schema-shortcut-rejection-check` |
| L-31 | Retain Tree-sitter/Ruff adapters/coordinates; delete mirrors/static kinds/opaque fields. | WP34. | `exact-provider-batch-check` | `provider-gap-schema-shortcut-rejection-check` |
| L-32 | Retain Pyrefly isolation/backpressure; replace opaque JSON/module authority with exact relations/gaps. | WP34. | `relation-ipc-provider-operations-check` | `provider-gap-schema-shortcut-rejection-check` |
| L-33 | Retain dated-nightly rustc isolation; replace summaries/local identity with exact relations and explicit downgrade. | WP34. | `exact-provider-batch-check` | `provider-gap-schema-shortcut-rejection-check` |
| L-34 | Retain bounded provider process lifecycle; delete claimed sandbox/static inventory/uncontained launch routes. | WP34/WP37. | `relation-ipc-provider-operations-check` | `session-uds-presentation-boundary-rejection-check` |
| L-35 | Retain exact Delta/providers/sessions/streams; replace model epoch/schema/current. | WP31/WP32/WP36; WP30 deletes old. | `delta-exact-reconstruction-v4-check` | `activation-receipt-nonauthority-check` |
| L-36 | Replace bespoke row consolidation with native plans or fully conforming extension. | WP31/WP35. | `datafusion-plan-schema-cache-check` | `caller-defined-semantic-authority-denial-check` |
| L-37 | Replace mutable snapshot manifests with `SelectedEpochRecord` and atomic active workspace. | WP29/WP32. | `delta-exact-reconstruction-v4-check` | `activation-receipt-nonauthority-check` |
| L-38 | Retain SQLite temporal queue/session/idempotency state only; Delta activation is semantic head. | WP32/WP36. | `candidate-free-recovery-check` | `activation-receipt-nonauthority-check` |
| L-39 | Retain eight bounded request meanings; delete fixed crosswalks/model pins/package planners/bypasses. | WP31/WP36. | `scheduled-streamed-semantic-query-check` | `query-admission-materialization-bypass-rejection-check` |
| L-40 | Retain central daemon/source/invalidation/fairness/security; replace parallel lifecycle/direct mutations. | WP29/WP32/WP37. | `lifecycle-production-vertical-check` | `session-uds-presentation-boundary-rejection-check` |
| L-41 | Delete persisted graph DTO/registry/index authority; retain only a proved transient kernel. | WP35/WP36. | `analysis-producer-semantic-check` | `ambiguous-producer-empty-success-rejection-check` |
| L-42 | Retain FastMCP topology/strict models; delete Python semantic state/static catalogs/whole joins. | WP37/WP39. | `lifecycle-production-vertical-check` | `session-uds-presentation-boundary-rejection-check` |
| L-43 | Targeted change: v1 public operability is not retained. Preserve only immutable allocations/history; v2 is sole production contract. | WP33 defines; WP37 deletes runtime; WP39 verifies. | `public-lifecycle-wire-contract-integrity-check` | `remaining-live-authority-zero-state-check` |
| L-44 | Preserve canonicalization KATs, independent expectations, decisions, and allocations; detach predecessor answers. | WP38/WP40. | `first-principles-production-behavior-check` | `history-comparator-independence-check` |
| L-45 | Replace producer goldens/counts/comparator acceptance with independent decoded rows/faults. | WP38/WP39. | `causal-fault-discrimination-check` | `history-comparator-independence-check` |
| L-46 | Delete obsolete detector/count governance; retain doctrine history. | WP39. | `post-purge-surface-inventory-integrity-check` | `remaining-live-authority-zero-state-check` |
| L-47 | Replace generated-authority gates with behavior/exact-version/resource/zero-state gates. | WP38--WP42. | `release-evidence-matrix-integrity-check` | `successor-final-zero-state-check` |
| L-48 | Retain intent-level Just/CI/four-domain isolation; delete retired jobs/edges. | WP39/WP42. | `successor-four-domain-release-check` | `remaining-live-authority-zero-state-check` |
| L-49 | Retain behavioral/provider/protocol/KAT tests rebound to v4; delete digest/replay/text acceptance. | WP33/WP38/WP40. | `first-principles-production-behavior-check` | `history-comparator-independence-check` |
| L-50 | Replace current suite with versioned successor; preserve prior suites as history. | WP33. | `successor-authority-expectation-integrity-check` | `negative-fixture-independence-check` |
| L-51 | Retain spec index as derived navigation only; update to successor and delete stale current claims. | WP33/WP39. | `successor-authority-expectation-integrity-check` | `remaining-live-authority-zero-state-check` |
| L-52 | Preserve designs/plans/states/reviews/released census as immutable detached history. | WP39--WP42. | `release-evidence-matrix-integrity-check` | `successor-final-zero-state-check` |
| L-53 | Preserve exact locks/toolchains/four roots; remove unused dependency/target edges. | WP39/WP42. | `successor-four-domain-release-check` | `remaining-live-authority-zero-state-check` |
| L-54 | Remove live dirty ontology/schema epoch readers; preserve exact historical artifacts only. | WP29 consumes typed inputs; WP30 deletes readers. | `compiled-release-consumer-cutover-check` | `bootstrap-ontology-authority-zero-state-check` |
| L-55 | Retain minimal intrinsic identity encoders; replace generated/current arrays/registries. | WP31 consumes; WP30/WP39 delete. | `datafusion-contract-matrix-integrity-check` | `remaining-live-authority-zero-state-check` |

## 7. Decommission batches

Each batch closes only after positive target behavior and multidimensional negative zero-state pass
at the same HEAD. Historical exclusions are exact paths/classes and have no live reader.

### DB09 — Delete bootstrap, model, ontology, generated-schema, and dual-epoch authority

**Dependencies.** WP29 target kernel; executed by WP30 and completed by WP32.

**Disposition.** Remove model compiler/importer/replay/tooling/features, bootstrap/ontology bundles
and schemas, generated semantic registries/arrays, model migration, default query/composition,
old epoch/current selectors, dual handles, direct seed/mutation, and attached tests/recipes/package
edges. Move only intrinsic identity, schema-phase, writer/fence/idempotency/reconciliation primitives
to target owners.

**Exit.** WP29/WP30/WP32 positive and negative oracles pass; model-free clean build/restart and exact
empty-head genesis pass; no old selector remains outside non-live history.

### DB10 — Delete legacy provider, projection, analysis, graph, query, and result authority

**Dependencies.** WP31 and WP33; executed by WP34--WP36.

**Disposition.** Remove name/fake-source schemas, opaque/JSON provider payloads, old admission,
static kinds/capabilities/producers/query registries, procedural duplicate analyses, persisted graph
indices, fixed crosswalks/model pins, parent-session leaks, independent/unbounded query maps, result
bytes in events, one-chunk/whole-result IPC, and parallel caches/fixtures.

**Exit.** Exact provider/gap, producer closure, all-eight-query, authorized child, bounded
coordinator, streamed-page/package, cancellation, retention, and restart oracles pass; no alternate
provider/analysis/query/result route exists.

### DB11 — Delete legacy serving, v1 wire, socket, lifecycle, and adapter routes

**Dependencies.** WP32 and WP36; executed by WP37.

**Disposition.** Remove v1 generated runtime/service/client/package/operability, Stream/Attach and
Read/ReleaseResult routes, repeated body authority, legacy fields/cursors/deadlines, blind socket
removal, split readiness, default/in-process serving, lazy handshake, local reference synthesis,
static package registries, Python semantic/Arrow processing, and unbounded delivery paths.

**Exit.** The actual v2 daemon/client/installed FastMCP vertical, session/UDS denial matrix,
resource/cancellation/reconnect/shutdown/restart proof, and package zero state pass. V1 artifacts
remain only immutable non-live allocation history.

### DB12 — Delete predecessor evidence and generated governance authority

**Dependencies.** WP38; executed and closed by WP39.

**Disposition.** Preserve independent KATs/expectations/decisions/history. Delete bootstrap/model/v1
operability expectations, mandatory comparators, producer goldens, digest/count semantic acceptance,
generated authority censuses, obsolete detector/property/gate scripts/rules/fixtures, and stale
current-suite/index selectors.

**Exit.** Independent v4 evidence/causal faults pass with history unavailable; zero-state and
governance scans find no live predecessor acceptance edge.

### DB13 — Delete retired features, targets, dependencies, recipes, workflows, and package edges

**Dependencies.** DB09--DB12; executed and closed by WP39.

**Disposition.** Remove every retired Cargo/Python/build/generated/package/service/recipe/workflow/
rule/test/fuzz/snapshot/corpus edge after reachability. Re-lock and regenerate only through the
surviving exact target toolchains.

**Exit.** Exact Cargo/uv/target/feature/package/recipe/workflow/service/generated inventories,
clean four-domain builds, v2 proto reproduction, adapter wheel/sdist inspection, and hidden/skipped
accounting pass.

### DB14 — Detach history and delete dormant handoff/cutover authority

**Dependencies.** DB09--DB13 and WP40; executed by WP41 and certified by WP42.

**Disposition.** Preserve immutable history/allocations/tombstones without live readers. Delete
forward-cutover modules, controller wiring, admin/CLI status, predecessor release/revocation/reboot/
rollback/bridge vocabulary, manifests/fixtures/recipes/selectors, and every ability to build or
select a predecessor/handoff path. Retain target-only leases, generation, expected-head,
reconciliation, physical observations, and forward repair.

**Exit.** FreshActivation owns binary/package/service/UDS/writer/activation before and after restart;
unknown outcomes reconcile; target mutation repairs forward; dormant handoff and every other legacy
class are zero; final certification passes.

## 8. Final gate matrix

Packet recipes supplement repository gates; they do not alias them. An aggregate preserves each
underlying exit code, nonzero selection, discriminating fault, and evidence artifact.

| Gate family | Exact commands at WP42 | Required truth |
|---|---|---|
| Design/artifact/state | `just authoritative-design-conformance-check`; `just artifacts-check`; `just plan-status`; `just plan-dependency-check docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v4_2026-09-01.md` | Successor suite is sole current, inputs fresh, v4 active state schema-valid, proving commits ancestral, DAG closed, WP28/M01 absent. |
| Packet proof | `just successor-all-packet-oracles-check`; parsed `just packet-oracle-check <WP>` for WP29--WP42 | Four substantive categories per active packet, nonzero selection/fault, no hard-coded count or stale recipe. |
| Composition/lifecycle | `just programmatic-production-composition-check`; `just programmatic-runtime-lifecycle-check`; `just fresh-activation-target-authority-check` | Real binary, one lifecycle, lawful exact authority, target-only activation/restart. |
| DataFusion/Arrow/Delta | `just datafusion-plan-schema-cache-check`; `just exact-provider-batch-check`; `just analysis-producer-semantic-check`; `just scheduled-streamed-semantic-query-check`; `just delta-exact-reconstruction-v4-check`; `just candidate-free-recovery-check` | Exhaustive compiled release, native authorized execution, exact gaps/rows, bounded pages/packages, exact durable selection/recovery. |
| Wire/session/UDS/FastMCP | `just public-lifecycle-wire-contract-integrity-check`; `just lifecycle-production-vertical-check`; `just session-uds-presentation-boundary-rejection-check`; `just resource-cancellation-recovery-check`; `just proto-check`; `just proto-repro-check`; `just adapter-stdio-test` | Sole v2 contract, generated Rust/Python parity, grants/sessions/owned sockets, four tools/live references, real installed-package vertical. |
| Evidence/reconstruction | `just first-principles-production-behavior-check`; `just causal-fault-discrimination-check`; `just clean-reconstruction-evidence-check`; `just history-comparator-independence-check` | Independent decoded values/faults and clean rebuild decide semantics; history is unnecessary. |
| Purge/activation | `just remaining-live-authority-zero-state-check`; `just post-purge-package-build-operations-check`; `just fresh-activation-zero-state-check`; `just fresh-activation-reconciliation-check`; `just successor-final-zero-state-check` | DB09--DB14 and L-20--L-55 physically closed, sole target ownership and forward repair. |
| Performance/resources | `just daemon-boundary-bench`; `just release-evidence-matrix-integrity-check` | Representative workloads meet declared envelope with semantic equality and recorded limitations; no folklore tuning. |
| Stable root | `just root-fmt`; `just root-check`; `just root-clippy`; `just root-test-rust`; `just root-doctest`; `just features-no-default`; `just features-each`; `just stable-graph-check` | Formatting, compiler/lints, ordinary plus doc tests, feature isolation, exact dependency universe. |
| Extractor | `just extractor-fmt`; `just extractor-check`; `just extractor-test`; `just extractor-identity` | Exact dated-nightly identity/private seam and provider behavior. |
| Sidecar | `just sidecar-fmt`; `just sidecar-check`; `just sidecar-test`; `just sidecar-identity` | Exact pinned Pyrefly sidecar identity/API/process behavior. |
| Adapter/package | `just adapter-ci-fast`; `just adapter-wheel-test`; `just adapter-stdio-test` | Python types/tests/package/STDIO protocol and no semantic/Arrow processing. |
| Repository release | `just governance`; `just ci-pr`; `just successor-four-domain-release-check`; `just relational-fabric-v4-certification` | Complete supported release matrix and independent review at one trusted HEAD. |

## 9. Execution order, parallelism, and overlap control

1. Independently audit this draft. On approval, activate it through a fresh v4 schema-v2 state
   transaction; migrate no v3 completion.
2. Execute WP33 and freeze the versioned successor suite plus expectations.
3. Execute WP29, then WP30 immediately so the positive kernel consumer is followed by rapid
   ontology/bootstrap/model decommission.
4. Execute WP31. After its public contract freezes, WP32 and WP34 may run in parallel only with
   disjoint path reservations; WP35 follows WP34. Any shared type/recipe/generated artifact forces
   serialization.
5. Execute WP36 after WP32 and WP35, then run the joined M04 gate.
6. Execute WP37 as one production vertical. Internal implementation order is lifecycle/workspace,
   coordinator/results, v2 descriptor, supervisor/session, owned UDS/health/errors, Python client,
   FastMCP, then full process proof; none is separately complete.
7. Execute WP38, WP39, and WP40 in order. Evidence precedes purge; purge precedes post-purge
   measurement/certification.
8. Execute WP41 FreshActivation and delete dormant handoff machinery only after target-only proof.
9. Execute WP42 and the independent implementation review at one trusted HEAD.

Before every packet, record `git status --short --untracked-files=all`, current HEAD/input freshness,
structural/textual candidate coverage, Cargo/uv/package/recipe impact, shared-path reservations, and
unrelated dirty changes. Do not stage, reset, overwrite, or attribute other work. A packet that
discovers overlap stops or serializes before edits; a separate worktree does not waive shared
external runtime/socket/target concerns.

## 10. Packet evidence, commits, and state discipline

- Each packet has one proving commit containing its implementation, tests, oracle recipes,
  committed positive/negative/fault fixtures, evidence artifact, and no unrelated user changes.
- The evidence artifact records candidate HEAD, dependency commits, input identities, changed and
  impacted paths, skipped/excluded/unparsed candidates, exact commands/exit codes, selected counts,
  fault results, environment/resources, limitations, and recovery observations.
- State records only the schema-permitted judgment/provenance fields. It never stores command
  transcripts, hashes as behavioral claims, test counts, benchmarks, or arbitrary evidence payloads.
- A green artifact/state/dependency validator proves structure and lineage only. A green proto
  generator proves derivation and Rust/Python descriptor equivalence only. Neither can satisfy a
  behavioral packet criterion.
- If a packet changes after its proving commit, rerun its four oracles and all dependent milestone/
  final gates at the new candidate HEAD; update state only through the governed transaction.
- No packet claims completion from current-tree code, a previous state label, a test-only direct
  factory, `ProbeService`, injected backend, Python stub daemon, selector count, or legacy agreement.

## 11. Replan triggers and risk controls

| Trigger | Required action |
|---|---|
| Provider schema requires executing a provider or semantic name heuristics. | Reopen compiled descriptor/provider boundary; do not retain fake source execution. |
| Release programs/producers cannot be reconstructed from compiled code plus explicit inputs. | Reopen semantic ownership; do not persist caller catalogs or restore generated registries. |
| One read cannot return coherent activation event/vector/fence/horizon. | Reopen activation storage contract; do not combine independent latest values or trust receipts. |
| Recovery needs a process-local candidate, stored catalog clone, cache, or digest equality. | Reopen durability classification; remain fail-closed. |
| Native DataFusion cannot preserve field identity/order/authorization closure at the selected rung. | Produce the failing native control and review the next extension rung; no parallel evaluator. |
| Bounded page encoding/publication cannot atomically seal on a supported store without whole-result copy. | Reopen result-sink/page topology; do not raise memory limits or materialize the result. |
| Supervisor grant/session root cannot be supported by an authorized launcher. | Reopen local authentication/launch design; do not fall back to reusable env/argv/repository tokens. |
| V2 control/resource contract cannot express a required functional capability without semantic duplication. | Version the design/package; do not restore v1 or add opaque JSON/`Any` semantics. |
| FastMCP gains a genuinely streaming resource API or no longer materializes resource values. | Reassess page/resource boundary with executable library evidence. |
| A real deployed predecessor is discovered. | Stop FreshActivation and design a separate one-shot AuthorityHandoff; do not activate dormant controller code. |
| Multi-host/distributed writers or cross-user/network transport enter scope. | Reopen fencing, identity, transport security, and deployment design; local UDS assumptions do not extend. |
| Representative workloads violate the declared resource/latency envelope. | Reopen topology/policy using measured attribution; no folklore tuning or weakened semantic bounds. |
| An input, successor spec, expectation, packet dependency, or proving commit drifts. | Stop dependent execution and issue the governed version/replan; never silently refresh hashes. |
| Zero-state has skipped/unparsed/unclassified/overlapping candidates or reaches secret/vendor paths. | Correct the coverage envelope and rerun; neither broad exclusion nor empty grep is acceptance. |

Top risks are dirty-tree ownership collision, a false bootstrapping/Ready conflation, hidden
whole-result buffering, coordinator fragmentation, async/sync object-store writer mismatch,
session authority leakage into body IDs, blind socket deletion, target-v2 semantic duplication,
historical exclusions becoming live, and certification that validates records instead of behavior.
The packet dependencies and four-category faults are the controls; they may not be waived locally.

## 12. Activation and completion boundary

This draft is ready for independent `plan-audit`; it is not active or executable state. Approval
does not itself change the current suite, plan pointer, or state. Activation must:

1. verify this file and every declared input;
2. verify the accepted design v4 and supersession chain;
3. create a fresh schema-v2 state at the declared v4 path with WP29--WP42 only;
4. create no WP28/M01 entry and migrate no v3 completion/proving commit;
5. atomically point the active plan to this exact artifact; and
6. record activation evidence without claiming implementation behavior.

Implementation is complete only after M02--M07 and DB09--DB14 close, every derived packet oracle
and final matrix gate passes at one trusted HEAD, the real v2 binary-to-installed-FastMCP vertical
and FreshActivation/restart/zero-state proof pass, representative resource evidence meets its
declared envelope, and the independent implementation review is accepted. Only the governed state
transaction may then mark v4 complete.
