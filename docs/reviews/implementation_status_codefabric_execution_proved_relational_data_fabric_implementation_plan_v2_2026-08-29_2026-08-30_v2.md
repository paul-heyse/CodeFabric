---
artifact: implementation-status
plan_path: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md
state_path: docs/plans/state/codefabric-execution-proved-relational-data-fabric_v2_state.json
version: v2
date: 2026-08-30
status: complete
---

# Implementation Status: Execution-proved relational data fabric v2

## Provenance

This report reassesses the immutable plan after owner-directed architecture changes made during
execution. It supersedes the judgment in the v1 status report that the original packet outcomes
remained materially valid. The current implementation and owner decisions now establish a
different foundation:

- schemas and relation identities are constructed programmatically from exact provider facts,
  explicit typed inputs, and transformations in the candidate DataFusion session;
- proof-bearing intermediate state is durable through exact-version Delta histories rather than
  replayed bootstrap/model artifacts;
- the selected epoch owns bounded DataFusion metadata, file-statistics, object-list, and logical-
  plan caches, while physical plans and results remain reconstructible and uncached;
- activation recovery starts without a process-local candidate and reconstructs one sealed epoch
  from an exact Delta version vector; and
- final acceptance is first-principles and independently authored. A predecessor comparator is
  optional migration evidence, not acceptance authority or a prerequisite.

The plan and source design were not edited. The execution-state JSON was reconciled only after
current-tree evidence gathering.

Reproducible derivation and focused evidence:

- `just artifacts-check` — exit 0.
- `just plan-status` — exit 0.
- `just v2-authority-cutover-check` — exit 0.
- `cargo nextest run --locked --lib -E 'test(/fabric::programmatic_schema::tests::/) | test(/fabric::programmatic_epoch::tests::/) | test(/fabric::datafusion_cache::tests::/) | test(/relation_ipc::tests::/) | test(/relation_ipc_proto::tests::/) | test(/relation_ipc_wire::tests::/)' --no-tests=fail` — exit 0.
- `just delta-exact-version-reconstruction-check` — exit 0.
- `just fabric-control-recovery-check` — exit 0.
- `git status --short` and `git rev-parse HEAD` — rerun for the exact dirty-tree and HEAD identity;
  all packet proving commits remain null.

No broad repository, feature-matrix, security, performance, or release-certification gate was
run. Focused evidence is sufficient to determine architecture and plan status; it is not release
certification.

## Derived Status Snapshot

```json
{
  "accepted_input_evolutions": [],
  "baseline": {
    "ancestor": true,
    "commit": "7184b86dc80adedc8a2b8d081179fa52d3dfee20",
    "exists": true
  },
  "complete_decommission_batches": [],
  "complete_milestones": [],
  "complete_packets": [],
  "declared_input_count": 25,
  "healthy": true,
  "plan_path": "docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md",
  "stale_inputs": [],
  "untrusted_complete_entries": [],
  "untrusted_complete_packets": []
}
```

This is the verbatim `just plan-status` derivation. `healthy: true` means only that the declared
inputs still match the immutable plan, the baseline is ancestral, and no recorded complete entry
is untrusted. It cannot detect that later owner decisions and current code superseded the design
premise because those decisions correctly live in execution state rather than rewriting the
declared-input table.

## Reconciliation Decisions

### Overall decision: plan and source design invalidated

Plan v2 is no longer an executable statement of the accepted target. This is a major replan, not
a packet-local deviation:

- Design v2 makes the bootstrap metamodel, `ModelMigration` replay, `FabricCompilerRelease`, one
  replayed model authority, and a model-derived `SchemaContract` foundational in D-20, D-21,
  D-31, Stages 1/3/4, and the proof matrix.
- Plan v2 repeats that premise in I-20, I-24, I-31, WP02, WP03, WP05, WP06, WP27, M01, DB01,
  and DB04.
- The currently selected authoritative v2.0 suite also declares replayed model relations and
  model-derived schema authority. `just v2-authority-cutover-check` proves that this suite is
  uniquely selected; it does not prove that its semantics remain accepted.
- The current target code deliberately omits the legacy `model` schema from
  `ProgrammaticFabricEpochBuilder`, constructs relation and schema meaning in
  `ProgrammaticSchemaAssembly`, and historicizes the resulting relation, field, schema,
  dependency, and provenance observations through `programmatic_observation_delta`.

Updating only the implementation plan would leave it in conflict with both design v2 and the
selected authoritative suite. A successor design and suite revision must therefore precede the
successor plan.

### Architecture the successor must make normative

1. **Programmatic session authority.** Exact provider batches and explicit non-derivable policy
   inputs enter one candidate `SessionContext`. Typed `ProgrammaticTransformation` values build
   downstream relations. DataFusion derives schemas from providers and logical plans; declared
   output schemas are assertions checked against the derived result, not independent authority.

2. **Relational self-description without bootstrap.** The live candidate catalog emits relation,
   field, schema, dependency, and provenance observations. Those observations participate in the
   same session and fixed-point closure. There is no `BootstrapMetamodel`, replayed schema
   registry, `ModelEpoch -> SchemaContract` prerequisite, or model digest standing in for rows.

3. **Delta as durable exact state for proof-bearing intermediates.** Stable append-only history
   tables retain exact versions and application transaction markers. The five catalog-
   observation histories and activation-control history already implement this pattern. The
   successor must classify and historicize every additional intermediate relation required for
   activation, restart, incremental recomputation, audit, or provenance closure. Purely
   reconstructible execution buffers remain transient Arrow state; persisting every optimizer
   intermediate is neither required nor desirable.

4. **Exact-version visibility.** A `FabricEpoch` and its activation row name the complete typed
   relation-to-root/version vector. Current views filter exact histories inside the selected
   session. Delta CDF is incremental transport, statistics are pruning evidence, and commit
   metadata is physical evidence; none alone proves semantic completeness.

5. **Explicit cache hierarchy.** DataFusion metadata, Parquet file-statistics, and object-list
   caches have finite memory/TTL bounds. Compiled and optimized logical plans live in an epoch-
   owned bounded cache keyed by the full typed program plus exact epoch, table versions, runtime,
   session, authorization, and resource policy. Physical plans and query results are rebuilt.
   `ActivationReconciliationReceiptCache` caches only a reconstructible receipt and never names
   semantic current.

6. **Candidate-free recovery.** Startup begins admission-closed with no cached candidate,
   reconciles exact Delta operation/activation evidence, reconstructs a fresh sealed session from
   the selected version vector, installs it, reconciles the receipt/acknowledgement, and only then
   reopens admission.

7. **First-principles evidence.** Independent expected rows, causal faults, exact provider API
   facts, released wire contracts, relational invariants, and end-to-end behavior accept the
   final implementation. Existing non-bootstrap expectations may be retained only after review.
   The current `bootstrap_model_semantics` expectation and mandatory WP01/comparator DAG must not
   survive as successor authority.

8. **Direct current APIs and Arrow IPC.** Plan v2's direct-integration decision remains valid.
   The current relation-scoped Arrow IPC frame/ack/coverage/cancellation boundary is reusable and
   should replace opaque provider payloads without a defensive semantic facade.

### Packet reconciliation

Every packet remains unproved because no packet has a proving commit. `in_progress` below means
current implementation is reusable in the successor; `invalidated` means the packet outcome or
dependency graph itself must be replaced.

| Packet | Status | What current evidence establishes | Successor disposition and remaining work |
|---|---|---|---|
| WP01 | invalidated | Transition selectors, freeze inputs, recipes, and sole-v2.0 selection now exist. | The selected suite encodes the superseded bootstrap premise. Publish and select a successor design suite; do not certify the current selection as semantic alignment. |
| WP02 | invalidated | `relational_model` implements `BootstrapMetamodel`, compiler releases, migrations, and replay. | Treat the whole target-facing subsystem as legacy. Preserve only genuinely static wire/identity primitives and migrate explicit decisions as ordinary typed inputs where needed. |
| WP03 | invalidated | A legacy importer and independent-evidence artifacts now exist in the dirty tree. | Do not make an importer or initial model corpus a prerequisite. Delete the importer and prove no live static-input reader remains. |
| WP04 | in_progress | Relation-scoped Arrow IPC, generated control messages, flow control, coverage, terminal-state, corruption, and cross-provider protocol code are implemented and focused-tested. | Rebind schema authority to programmatic provider/transform contracts, finish production process interoperability, add proving oracles, and commit. |
| WP05 | in_progress | `ProgrammaticFabricEpoch`, sealed candidate-session assembly, catalog observations, runtime identity, and bounded DataFusion runtime caches exist. | Route production epoch consumers through this builder, close catalog/resource proof, and retire the older model-backed `FabricEpoch` path. |
| WP06 | invalidated | The typed `RelationalProgram` compiler and collision-safe compiled/optimized logical-plan cache are reusable. | Replace model-owned inputs, model-causality oracles, and replay dependencies with provider-fact/explicit-input/transformation causality. Preserve native DataFusion lowering and fresh physical planning. |
| WP07 | in_progress | Proof, provenance, capability, coverage, and producer-closure machinery exists. | Rebind expectations to first-principles evidence and persist the proof-bearing intermediate relations required for activation/restart/audit as exact Delta histories. |
| WP08 | in_progress | Exact Tree-sitter/Ruff provider-native relation producers exist. | Complete production candidate-session ingestion, exact coverage/remainder proof, incremental equivalence, and proving commit. |
| WP09 | in_progress | Pyrefly emits typed relation-scoped Arrow IPC through current pinned surfaces. | Finish daemon consumption, invalidation, exact-surface proof, and removal of target-route JSON/module authority. |
| WP10 | in_progress | rustc public/MIR and selected private facts cross the typed IPC boundary. | Make the contained extractor route exclusive, close exact API/private-boundary proof, and remove debug/summary substitutes. |
| WP11 | in_progress | Provider admission validates typed schema, coverage, and explicit capability state into the programmatic assembly. | Complete normalization/authority/conflict/unknown/provenance transformations and production epoch composition without a model replay dependency. |
| WP12 | in_progress | Native/extension graph program machinery and derived inputs exist. | Prove the highest valid DataFusion rung, extension invariants, resources/cancellation, and causal selection under the programmatic session authority. |
| WP13 | in_progress | Typed relational semantic requests and execution-oriented query transactions exist. | Close all released forms and composition roles, bind them to accepted producers, and remove remaining static/legacy query authority. |
| WP14 | in_progress | Reduced authorized child catalogs, exact pins, allowlists, resource limits, and cross-session logical-plan reuse exist. | Use this route in production serving and prove complete bound-provider/view/function/store closure and non-leakage. |
| WP15 | in_progress | Arrow result packaging/publication plus gRPC/FastMCP resource handling exists. | The daemon still constructs the predecessor `ProductionQueryService`/`WorkspaceQueryBackend`; wire `RelationalQueryRuntime` into the actual UDS service and prove one real vertical. |
| WP16 | in_progress | `FabricCommand`, reducer, actor, journal, fencing, recovery, and effect-port machinery exists. | Supply the production workspace runtime factory and all durable backends, then prove exclusive mutation ingress and recovery. |
| WP17 | in_progress | Exact Delta writes/providers, five stable observation histories, CDF-at-creation, statistics-enabled exact reopen, exact version vectors, and activation-control storage exist. | Generalize the stable-history policy to every proof-bearing intermediate selected by the successor design; finish production publication/retention/maintenance composition. |
| WP18 | in_progress | Delta activation rows, exact marker/readback reconciliation, cold epoch reconstruction, candidate-free restart, receipt cache, and acknowledgement behavior exist. | Compose the concrete production authority/cache/acknowledgement factory and prove daemon swap/admission ordering end to end. |
| WP19 | in_progress | Invalidation, source-wave effects, resource governance, cancellation, and result backpressure mechanisms exist. | Compose gix/safe reads, providers, Delta publication, activation, query runtime, and one shared resource domain in the daemon. |
| WP20 | invalidated | A separately owned decoded expectation corpus exists, but it includes bootstrap semantics and mandatory comparator structure. | Replace with successor first-principles evidence execution; retain only re-reviewed non-bootstrap expectations. Comparison is optional and non-authoritative. |
| WP21 | invalidated | Command, activation, and fencing substrates can support a cutover, but the exact planned legacy-binary ceremony is not implemented. | Define the smallest durable cutover proving one serving/mutation authority and no fallback. Do not restore mandatory comparator/old-binary ceremony. |
| WP22 | invalidated | Independence validators and 25 decoded expectations exist, with explicit target-output exclusion. | Remove bootstrap expectation/dependency and mandatory comparator reconstruction; issue a successor evidence transaction after independent review of surviving expectations. |
| WP23 | in_progress | Typed Python CFG/flow/alias/effect/resource/async derivations exist. | Bind to accepted provider relations in production epochs and prove semantics, provenance, unknowns, and clean/incremental equivalence. |
| WP24 | in_progress | Typed MIR ownership/flow/alias/drop/resource/async/unsafe derivations exist. | Bind to accepted rustc relations and prove semantics, provenance, private enrichment, and incremental equivalence. |
| WP25 | in_progress | Cross-language graph/effect/resource/interprocedural and producer-closure code exists. | Execute fixed points over accepted language-local inputs and prove exactly one producer or an explicit unsupported remainder for every target family. |
| WP26 | in_progress | Untrusted Rust compilation policy, receipt, launcher, and resource/cancellation substrate exists. | Make it the exclusive production route and prove credential/network denial, process-group/resource bounds, host coverage, and bypass zero state. |
| WP27 | invalidated | Generic logical/storage/scan/stream/write schema validation and adaptation code is reusable. | Delete the `ModelEpoch -> SchemaContract` target. Construct contracts directly from provider/transform schemas and prove every Arrow/DataFusion/Delta phase without a model projection bypass. |

### Milestone reconciliation

| Milestone | Status | Reconciliation |
|---|---|---|
| M01 | invalidated | Replace “replayed model foundation” with a programmatic candidate-session and exact-history foundation. |
| M02 | in_progress | Exact provider and language-analysis components exist but lack successor dependencies, production composition, gates, and proving commits. |
| M03 | in_progress | Query, child-session, result-resource, and IPC components exist; production daemon delivery remains incomplete. |
| M04 | in_progress | Durable observation/activation/recovery mechanisms and focused proof exist; production composition and complete proof-history coverage remain. |
| M05 | invalidated | Replace predecessor-comparator/frozen-binary acceptance with first-principles evidence plus the smallest proved sole-authority cutover. |
| M06 | not_started | Total purge remains mandatory but must use the successor inventory and dependencies. |

### Decommission reconciliation

- **Invalidated and replaced:** DB01, DB04, DB07, and DB08. DB01 must become direct
  bootstrap/model/importer removal; DB04 must preserve no schema/model bootstrap authority; the
  mandatory comparator-archive phases in DB07/DB08 collapse into optional historical-evidence
  disposition and final purge.
- **Still required but not started:** DB02, DB03, DB05, and DB06. Provider/analysis, serving/
  storage/query, governance/routing, and feature/package/dependency legacy remain live beside the
  successor modules.

## Blockers and Invalidated Assumptions

1. **Normative corpus conflict.** The selected v2.0 authoritative suite, design v2, and plan v2
   still require replayed model/bootstrap authority. A future executor following them would
   regress the code.
2. **Dual compiled authorities.** `relational_model`, `BootstrapMetamodel`, model migration
   commands, the model-backed `FabricEpoch`, and model-derived schema tests remain compiled and
   exported beside `ProgrammaticFabricEpoch`.
3. **Production composition gap.** New epoch, query, activation, Delta, command, and lifecycle
   components are mostly isolated. The daemon still serves through predecessor query/bootstrap
   paths and does not install the complete successor workspace runtime.
4. **Incomplete durable-proof classification.** Five catalog-observation histories and the
   activation-control relation are durable; the successor has not yet named every intermediate
   proof/coverage/provenance/derived relation that must survive restart or support audit.
5. **Evidence corpus drift.** Existing independent expectations contain bootstrap semantics and
   a mandatory comparator dependency. They cannot accept the successor without a new review
   transaction.
6. **No completion-grade commit evidence.** Every packet proving commit is null. Green focused
   checks support continued development only.

## Recommended Resume Order

1. Publish a versioned successor target design that explicitly replaces D-20, I-20, I-24,
   I-31, D-31, bootstrap stages, model replay, and mandatory predecessor evidence.
2. Revise the eight authoritative v2.0 masters as a versioned successor suite so the one selected
   current authority matches the accepted programmatic-session/Delta-history design.
3. Create a successor implementation plan by remapping reusable WP04–WP05, WP07–WP19, and
   WP23–WP26 work. Preserve stable IDs only where the outcome remains semantically identical;
   record explicit replacements for invalidated packets.
4. Make production composition the first implementation cut: select `ProgrammaticFabricEpoch`,
   `RelationalQueryRuntime`, exact Delta activation/recovery, command runtime, and shared resource
   authority in the daemon.
5. Cut over all consumers, then execute early negative proof and deletion of
   `BootstrapMetamodel`, model replay/importer/compiler authority, model-to-schema projection,
   old `FabricEpoch`, and related features/tests/tooling.
6. Complete the Delta durable-history matrix for proof-bearing intermediates, exact version-vector
   activation, retention/CDF/checkpoint policy, statistics, and restart reconstruction.
7. Issue a successor independent evidence transaction from first principles, run causal and
   end-to-end proofs against the final target, and use predecessor comparison only if it cheaply
   explains a meaningful behavioral difference.
8. Finish remaining provider/query/lifecycle integration, decommission batches, packet proving
   commits, milestones, and final gates.

## Exact Next Action

Use `design-development` to create the successor target design and authoritative-suite delta
before invoking `impl-plan`. The design must contain an explicit old-to-new decision map for:

- bootstrap/model replay -> programmatic provider/input/transformation relations;
- model-derived schema contract -> derived-and-validated session schemas;
- materialized model/plan artifacts -> exact Delta histories for proof-bearing relations plus
  bounded reconstructible caches;
- temporal cache -> receipt-only reconciliation cache;
- early frozen predecessor evidence -> independently authored first-principles acceptance; and
- delayed model purge -> early consumer cutover followed by total bootstrap/dual-epoch zero state.

Do not resume a packet under plan v2 or change the active pointer again until that successor
design and plan are approved.

## State Reconciliation Summary

- Overall status is `invalidated`; `current_packet` is null.
- Invalidated packets: WP01, WP02, WP03, WP06, WP20, WP21, WP22, and WP27.
- Reusable in-progress packets: WP04, WP05, WP07–WP19, and WP23–WP26.
- M01 and M05 are invalidated; M02–M04 are in progress; M06 remains not started.
- DB01, DB04, DB07, and DB08 are invalidated; DB02, DB03, DB05, and DB06 remain not started.
- New obligations record successor design/suite authority, durable intermediate proof histories,
  first-principles evidence, and bootstrap/dual-epoch zero state.
- The exact next action now points to successor design and plan creation rather than further
  execution of plan v2.
- The immutable plan, source design, and production code were not edited by this status audit.
