---
artifact: implementation-status
plan_path: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md
state_path: docs/plans/state/codefabric-execution-proved-relational-data-fabric_v2_state.json
version: v1
date: 2026-08-30
status: in_progress
---

# Implementation Status: Execution-proved relational data fabric v2

## Provenance

- Plan assessed: `docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md`.
- State reconciled: `docs/plans/state/codefabric-execution-proved-relational-data-fabric_v2_state.json`.
- Active-plan routing selects this exact plan. Baseline `7184b86dc80adedc8a2b8d081179fa52d3dfee20` exists and is an ancestor of audited HEAD `db67f7c`.
- The worktree contained 53 modified and 58 untracked paths at audit intake. These changes predate this status artifact and span both target implementation and other owner-directed work. They are current-tree progress, not packet proving commits.
- Derivation and artifact checks:
  - `just plan-status` — exit 0; inputs are fresh, the baseline is ancestral, and no packet, milestone, or decommission batch is recorded complete.
  - `just artifacts-check` — exit 1 before artifact validation because Ruff would reformat `tooling/ci/test_relational_fabric_transition.py` at line 306.
  - Direct non-mutating execution of `tooling/ci/artifact_contracts.py artifacts-check` — exit 0; the plan/state schemas and declared inputs validate once the formatting prerequisite is bypassed diagnostically.
- Focused current-tree checks:
  - `just root-check` — exit 0 for default and featureless stable-root graphs. The default graph emitted one unused-import warning in `src/fabric/command_record_sqlite.rs`.
  - `just authoritative-design-conformance-check` — 7 passed.
  - `tooling/ci/test_relational_fabric_transition.py` — 15 passed.
  - Direct `relational_fabric_transition.py authority-cutover-check` — exit 0 and selected one eight-master v2 suite with byte-identical predecessor history.
  - `just plan-dependency-check` — exit 0: 27 packets and no disjoint-phase overlap. The parameterized command written in the plan currently fails because the recipe accepts no positional plan argument; the executable API must be reconciled.
  - `just governance-scan` — exit 1 because new result/resource public error prefixes are absent from the public error registry.
- Oracle resolution is far behind implementation: the plan names 211 unique `just` recipes; 38 currently resolve and 173 do not. An absent recipe is not a failed implementation by itself, but no packet can be complete until every named acceptance oracle for that packet exists and passes at a proving commit and at HEAD.
- No broad test, `ci-fast`, `ci-pr`, feature matrix, extractor aggregate, sidecar aggregate, security, performance, or release-certification run was performed. This audit answers implementation status, not release readiness, and follows the owner's direction not to repeat broad validation unnecessarily.

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

`healthy: true` is limited to declared-input freshness, baseline ancestry, and trust checking of entries currently labeled complete. It is not a behavioral or completion result.

## Reconciliation Decisions

The prior state materially underreported implementation progress. The reconciled state is:

- 23 packets `in_progress`: WP01, WP02, WP04–WP19, and WP23–WP27.
- 4 packets `not_started`: WP03, WP20, WP21, and WP22.
- 0 packets `complete`; none has a proving commit.
- M01–M06 and DB01–DB08 remain `not_started` as acceptance states.

`in_progress` means that packet-specific implementation is present in the current tree. It does not mean dependencies are accepted, the production route is complete, or the packet's proof obligations exist.

### Foundation, model, protocol, catalog, compiler, and proof

| Packet | Reconciled status | Current-tree evidence | What remains |
|---|---|---|---|
| WP01 | in_progress | Eight v2 masters are present and uniquely selected; the transition validator proves authority selection and has 15 focused fixtures; the active-plan pointer selects v2. | Publish the compiled legacy selector and freeze inputs; wire every WP01 recipe; complete or explicitly disposition the external predecessor-transition receipts; restore governance/artifact gates; obtain a proving commit. |
| WP02 | in_progress | `src/relational_model/{schema,release,replay}.rs` implements the bootstrap metamodel, `FabricCompilerRelease`, typed model rows, migrations, replay, release handoff, and bootstrap closure; the module is exported and compiles. | Supply accepted production migrations and release inputs, close replay/reconstruction/cross-release/staticness oracles, prove legacy-input isolation, satisfy WP22, and commit. |
| WP03 | not_started | Generic `ModelMigration` and `ModelDecision` types exist as WP02 substrate. No one-time importer, accepted initial migration corpus, released-allocation bijection, or independent row-level review artifact was found. | Implement and independently accept the importer and complete initial relational model after WP02 and WP22. |
| WP04 | in_progress | `src/relation_ipc.rs`, provider boundary changes, Pyrefly/rustc relation schemas, Protobuf control additions, and adapter Arrow-resource code establish substantial typed relation-stream and result-stream substrate. | Prove the complete cross-process framing contract, opaque-payload rejection, truncation/interleaving/cancellation/backpressure behavior, exact Arrow universe, production interoperability, and all named gates after WP03. |
| WP05 | in_progress | `src/fabric/epoch.rs` and `src/fabric/provider.rs` implement immutable epoch construction, model binding, provider adaptation, sealed access, and model-program execution; provider admission uses new schema contracts. | Construct a production model-only catalog from accepted model/schema rows, close provider/catalog/resource/mutation-zero-state contracts, and prove no raw mutable session or legacy catalog authority remains. |
| WP06 | in_progress | `src/relational_program.rs` implements typed model-owned relations/fields, native DataFusion expression and logical-plan compilation, functions, observations, and execution through epoch/child-session paths. | Cover every required normalization, authority, unknown, derivation, semantic, policy, and proof composition; prove model causality, intrinsic runtime closure, plan visibility, and zero opaque/SQL/static-plan authority. |
| WP07 | in_progress | `src/fabric/proof.rs`, capability relations, producer-closure substrate, and epoch proof integration implement substantial executable proof/provenance/capability machinery. | WP22 must independently own expectations; then close provenance, coverage, causality, unknown, governance, and terminal proof relations with producer-authored-expectation rejection and packet gates. |

### Exact providers and provider integration

| Packet | Reconciled status | Current-tree evidence | What remains |
|---|---|---|---|
| WP08 | in_progress | `src/provider_native_syntax.rs` emits 25 typed Tree-sitter/Ruff relation families, including source/run pins, raw kinds, diagnostics, remainders, and incremental changed ranges, against direct pinned APIs. | Connect the relation set to the accepted production epoch route; close exact-API, Arrow-conformance, opaque-payload zero-state, incremental behavior, and independent boundary oracles. |
| WP09 | in_progress | The Pyrefly sidecar now emits relation-scoped Arrow IPC for typed context/type/member/import/call/navigation/diagnostic/coverage/remainder families, with an application-owned schema in `src/pyrefly_relation_schema.rs`. | Prove the full pinned Pyrefly surface matrix and environment invalidation, consume all streams through the daemon production route, remove target-route module JSON authority, and add packet gates. |
| WP10 | in_progress | The nightly extractor and `src/rustc_relation_schema.rs` now emit typed compilation/item/type/instance/MIR/CFG/call/access/diagnostic/remainder relations using current rustc surfaces. | Complete the exact public/private authority boundary, run every untrusted extraction through accepted WP26 containment, remove summary/debug substitutes on the target route, and pass extractor/API/protocol gates. |
| WP11 | in_progress | `src/provider_admission.rs`, capability code, relation IPC, and schema adaptation compose accepted relation streams into validated typed provider inputs with explicit coverage. | Wire complete normalization/authority/conflict/unknown/provenance/capability plans into production epoch construction and prove requested/completed coverage, stale-current rejection, honest statistics, and exact provider closure. |

### Derived analysis, semantic planning, authorization, and public delivery

| Packet | Reconciled status | Current-tree evidence | What remains |
|---|---|---|---|
| WP12 | in_progress | `src/fabric/graph_program.rs` implements model-selected native and extension graph operations; language-local and common derived-analysis modules supply substantial graph inputs. | Prove the highest valid DataFusion rung per operation, complete logical/physical extension contracts, child/context forwarding, reset/repetition/statistics/resource/cancellation behavior, causal selection, and no persisted petgraph identity. |
| WP13 | in_progress | `src/relational_semantic_query.rs` and `src/fabric/relational_query_runtime.rs` implement typed request relations, semantic compilation, producer-closure checks, and execution-oriented query transactions. | Close all eight released forms and composition roles against accepted producers; wire the daemon public request route; prove deterministic bounded planning and remove static crosswalk/physical/SQL authority. |
| WP14 | in_progress | `src/fabric/child_session.rs` and its resource-governance module build reduced catalogs, table grants, model-program execution, pins, registry allowlists, and bounded resources. | Use the child path in production serving, recursively prove bound-view authority, install fresh allowlisted registries/stores/functions, seal the epoch session, close leakage/policy tests, and register its new public errors. |
| WP15 | in_progress | Arrow result packaging/publication modules, gRPC control additions, Python Arrow-resource handling, and FastMCP/client changes implement much of the dynamic-result lifecycle. | Wire `RelationalQueryRuntime` into the actual daemon query service; prove a real FastMCP-to-daemon vertical, reference/status/capability delivery, authorization, package contents, and absence of packaged semantic authority. The current daemon references only parts of the result-resource layer. |

### Durable mutation, Delta, activation, and lifecycle

| Packet | Reconciled status | Current-tree evidence | What remains |
|---|---|---|---|
| WP16 | in_progress | `FabricCommand`, reducer transitions, actor, SQLite journal, writer lease/generation, recovery runtime/manager, complete effect-router interfaces, and typed command-family effects are implemented with extensive unit tests. | Provide production `WorkspaceFabricCommandRuntimeFactory`, semantic-context, interruption-diagnostic, resolver/commit/marker backends for every family; route every daemon mutation through it; prove exclusive ingress, fencing, recovery, temporal-store limits, Miri, and mutation-bypass zero state. Most durable effect ports currently have only test probes. |
| WP17 | in_progress | `delta_exact.rs` reconstructs exact-version providers; `delta_write.rs` performs real session-bound zero-retry Delta writes; `effective_view.rs` compiles optimizer-visible native overlays; real local Delta tests exist. | Add model-selected production table/bootstrap mappings, relation-publication and compaction backends, exact overlay/retention integration, schema restoration at Delta boundaries, conflict reconciliation, and all Delta/transaction/equivalence/zero-state gates. |
| WP18 | in_progress | Activation event/chain, admission closure, child sessions, command effect, transaction ordering, fault/recovery state machines, and cache-swap contracts exist. | Implement durable production authority/event/operation-marker/cache/acknowledgement ports and daemon epoch swap. The activation-control relation still needs an exact Delta control pin, canonical transaction marker, decodable durable activation rows/evidence, concrete selection/transaction rows, and a model-derived single-table physical mapping before exact readback and restart recovery can be truthful. |
| WP19 | in_progress | `src/continuous/invalidation.rs`, lifecycle changes, source-wave command effects, resource governance, cancellation, and result backpressure provide substantial mechanisms. | Compose authoritative gix/safe-file reads, watcher invalidation, providers, command publication, activation, and one shared DataFusion resource runtime in the daemon; then prove clean/incremental equivalence with live legacy inputs unavailable. |

### Independent evidence, release/cutover, language analyses, trust, and schema lifecycle

| Packet | Reconciled status | Current-tree evidence | What remains |
|---|---|---|---|
| WP20 | not_started | Existing predecessor golden/security/benchmark assets are not WP20 evidence because WP22 has not frozen and accepted the target corpus. | Re-execute the independently accepted corpus against the completed target and produce causal, semantic, security, performance, package, and old/new release evidence without authoring expectations. |
| WP21 | not_started | No target `LEGACY_AUTHORITATIVE -> NEW_READ_ONLY -> NEW_MUTATING -> LEGACY_RETIRED` production state machine or exact-old-binary revocation boundary was found. Predecessor ontology cutover code is not successor proof. | Implement and prove fenced drain, read-only activation, restart/host-reboot revocation, crash reconciliation, one serving/mutation authority, no fallback, and final cutover readiness after WP20. |
| WP22 | not_started | The v2 suite states independence doctrine, but no WP22-owned decoded expectation set, owner acceptance, frozen comparator executable/worktree, comparison contract, isolation harness, or named WP22 recipes exists. Existing golden files are unaccepted candidates. | Freeze and independently accept the exact comparator and every model/provider/query/public/security/activation expectation before any later implementation can be accepted. Ensure target outputs are not used to author the corpus. |
| WP23 | in_progress | `src/python_derived_analysis.rs` implements typed application-owned Python CFG, reaching definitions, liveness, value/alias/effect/resource/async relations and explicit unknown handling. | Integrate accepted WP08/WP09/WP11 inputs into production epochs; prove provenance/authority, exact flow semantics, clean/incremental equivalence, and no Ruff/Pyrefly misattribution. |
| WP24 | in_progress | `src/rust_mir_derived_analysis.rs` implements typed MIR-derived ownership, flow, alias, drop/resource, async, unsafe/FFI, control-flow, and unknown relations. | Integrate accepted WP10/WP11 relations and private enrichment; prove provenance/authority, exact flow behavior, incremental equivalence, extractor closure, and no rustc-native misattribution. |
| WP25 | in_progress | `src/common_derived_analysis.rs` and `src/fabric/derived_producer_closure.rs` implement cross-language graph/effect/resource/interprocedural relations and producer-closure checks. | Complete and execute fixed points over accepted language-local outputs; prove every accepted ontology/query family has exactly one producer or explicit unsupported remainder, and close judgment-zero-state gates. |
| WP26 | in_progress | `src/rust_compilation_trust.rs`, provider sandboxing, and rustc-service cancellation/resource hooks implement a large policy/receipt/launch substrate. | Make this the only production route for every untrusted Rust extraction; prove immutable inputs/private outputs, credential/network denial, bounded process groups/resources, fail-closed host coverage, explicit `TRUSTED_LOCAL`, and host-compiler bypass zero state. |
| WP27 | in_progress | `src/schema_contract.rs` implements logical/storage/scan/stream/write schemas, mappings, model IDs/roles, metadata preservation, casts, adapters, validation, and integration with provider admission and epoch providers. | Add the production `ModelEpoch -> SchemaContractModelRows` projection and model-selected physical bindings; prove closure through IPC, Delta/Parquet reopen, scanning, physical planning, streams, batches, and sinks; remove adaptation bypasses and pass lifecycle gates. |

The original packet outcomes remain materially valid. No DataFusion, Arrow, delta-rs, or code-facts API finding requires a target-design replan. Two execution qualifications now apply:

1. WP02 and later code was developed before WP22's required evidence freeze, so it is provisional implementation and cannot be used to author or justify the expected results that will later accept it.
2. The plan's parameterized `plan-dependency-check <path>` command does not match the current no-argument recipe. The gate implementation must support the documented contract or another versioned plan integration must formally disposition the mismatch; silently using a different command is not completion proof.

### Milestones

| Milestone | Status | Why it is not complete |
|---|---|---|
| M01 | not_started | WP01 is incomplete, WP03 and WP22 have not started, WP02/WP04–WP07/WP27 have no proving commits or complete gates, and `relational-model-foundation-check` is absent. |
| M02 | not_started | Exact-provider and language-analysis code exists, but WP08–WP11/WP23/WP24/WP26 are dependency-unaccepted and unproved; `exact-provider-fabric-check` is absent. |
| M03 | not_started | Query/graph/child/result components exist, but WP12–WP15/WP25 are not wired and proved end to end; `semantic-delivery-vertical-check` is absent. |
| M04 | not_started | Command/Delta/activation/lifecycle components exist, but the production durable ports and daemon composition are incomplete; `durable-epoch-reconstruction-check` is absent. |
| M05 | not_started | WP20 and WP21 have not started; no independent release dossier or target cutover state machine exists. |
| M06 | not_started | No decommission batch is complete and live legacy authorities remain extensive. |

### Decommission batches

DB01–DB08 remain `not_started`. This is not a conservative label hiding a mostly complete purge:

- The WP03 importer and WP22 archive do not yet exist, so DB01 cannot execute.
- Legacy provider/projection/graph, serving/storage/mutation/query/adapter paths remain live beside target components, so DB02–DB03 have not begun as zero-state batches.
- `src/bin/codefabric_model/**`, `src/generated/**`, and `contracts/generated/**` still contain 56 files, and predecessor registry/schema/ontology/model modules remain exported and consumed. DB04 has not begun.
- Predecessor governance, Gate B/model tooling, generated authority checks, static adapter contracts, and old routing remain present. DB05 has not begun.
- The `model-compiler` feature/binary and its dependency/package edges remain. DB06 has not begun.
- History/comparator retention and final expiry prerequisites do not exist yet. DB07–DB08 have not begun.

The indicative legacy counts above are not substitutes for DB coverage proof. WP01's required complete selector/inventory relation is absent, so the exhaustive purge universe is still unknown.

## Blockers and Invalidated Assumptions

No target-design assumption is currently invalidated. The plan is executable, but completion is blocked by six cross-cutting conditions:

1. **Evidence-first order was bypassed.** WP22 has not started even though implementation consumers are far advanced. The current code can be retained, but every expected result must now be authored independently from accepted design/upstream/released-contract evidence and every dependent packet must be revalidated after WP22 acceptance.
2. **There is no completion-grade commit evidence.** All 23 active packets are dirty-tree progress with `proving_commit: null`. No milestone or decommission batch can inherit trust from them.
3. **WP01's transition authority is incomplete.** The v2 selection works, but `relational-fabric-legacy-selectors.json` and `relational-fabric-legacy-freeze.json` are absent, required recipes are unwired, and the predecessor supersession/overlap work remains partly uncommitted.
4. **Production composition trails component implementation.** The command family, Delta writer, activation transaction, query runtime, and derived analyses have substantial isolated implementations, but the daemon does not yet supply the complete durable backends and sole-route composition required by WP15–WP19.
5. **The acceptance surface is mostly absent.** 173 of 211 unique plan recipe identities do not resolve. The current governance scan also fails on unregistered new public errors, and `just artifacts-check` is blocked by one format defect.
6. **Legacy removal has not started.** Target modules currently coexist with the generated/static predecessor system. Until the M01–M05 cutovers and DB01–DB08 zero-state gates run, coexistence is transition progress, not v2 alignment.

## Recommended Resume Order

1. Finish WP01 rather than adding more later-packet code: publish the complete legacy inventory selector/freeze inputs, wire the transition recipes already implemented in `tooling/ci/relational_fabric_transition.py`, restore the public-error and artifact-format gates, close the owner-directed predecessor transition disposition, and create a WP01 proving commit.
2. Execute WP22 immediately after WP01. Freeze the comparator and decoded independent expectations without reading target outputs as authoring authority; add its four core acceptance/isolation gates and obtain independent acceptance.
3. Close M01 in dependency order: WP02, WP03, WP04, WP27, WP05, WP06, and WP07. Use the existing component code, but add accepted production model migrations, model-to-schema projection, real catalog construction, and executable packet oracles. Run DB01 immediately after M01 and the importer rollback decision.
4. Close M02: finish WP08/WP09/WP26, then WP10, WP11, WP23, and WP24 with real process-boundary and production-epoch proof.
5. Close M03: finish WP12, then WP25, WP13, WP14, and WP15; prove one real FastMCP-to-daemon semantic vertical over an authorized child catalog and Arrow result resource.
6. Close M04: finish WP16's production runtime factory/effect backends, WP17's model-selected Delta relations, WP18's durable activation-control relation and cache swap, and WP19's lifecycle/resource composition.
7. Execute WP20 against WP22's immutable evidence, then WP21's fenced cutover and M05. Do not treat predecessor cutover tests as successor evidence.
8. Run DB02–DB07 only after `LEGACY_RETIRED`, observing each batch's prerequisites and consumer-before-authority deletion order. Run DB08 only after every comparator/rollback/retention commitment expires, then perform final certification.

## Exact Next Action

Complete the bounded WP01 slice already started:

1. Publish `contracts/governance/relational-fabric-legacy-selectors.json` and `contracts/governance/relational-fabric-legacy-freeze.json` with exact design/plan binding and complete inventory coverage.
2. Add the WP01 `just` entry points around `tooling/ci/relational_fabric_transition.py`, including authority selection, inventory, disposition coverage, freeze, and old-current-authority zero state; make the documented plan-path form of `plan-dependency-check` executable.
3. Register the new public result/resource error codes and format the transition fixture so `just governance-scan` and `just artifacts-check` reach and pass their intended validators.
4. Run the full WP01 packet-local set, commit the dependency-closed result, record its proving commit, and only then begin WP22 acceptance.

Do not label WP02 or any later packet complete from the current working tree, even if its focused unit tests pass.

## State Reconciliation Summary

- Overall state remains `executing`; `current_packet` remains WP01.
- WP01, WP02, WP04–WP19, and WP23–WP27 are now recorded `in_progress` because packet-specific current-tree implementation exists.
- WP03, WP20, WP21, and WP22 remain `not_started`.
- Every in-progress packet has `proving_commit: null` and a blocker stating that its current implementation is provisional.
- M01–M06 and DB01–DB08 remain `not_started`.
- The existing owner-directed activation override remains recorded, and a second deviation now records that implementation advanced before WP22's independent evidence freeze.
- Three discovered obligations now record the accepted-model/schema projection, activation-control relation, and production runtime-composition closure needed by the current component set.
- `next_action` now directs execution to finish WP01 and then WP22 rather than continuing unsequenced later-packet implementation.
- No plan, design, or production-code artifact was edited by this status assessment.
