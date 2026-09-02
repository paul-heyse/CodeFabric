---
artifact: implementation-status
plan_path: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v4_2026-09-01.md
state_path: docs/plans/state/codefabric-execution-proved-relational-data-fabric_v4_state.json
version: v1
date: 2026-09-01
status: complete
---

# Implementation Status: Execution-proved relational data fabric v4

## Provenance

This report reconstructs the approved v4 plan from the immutable plan, its schema-v2 execution
state, proving commits, the current source/contract tree, named packet oracles, and direct source
inspection. It is a review-only status reconciliation: no plan, design, production code, contract,
or oracle was changed while producing it.

The audited HEAD is `4f2c676d3c2ebb1b6e43b25f0efb847bcde9f888`; baseline
`6a76b5cff3d84e8249e5bedaa52a17f2abb816dd` exists and is ancestral. Before the status artifacts
were written, the shared working tree contained 57 modified tracked files and no untracked or
deleted files. Those changes span WP32 exact activation/reconstruction, WP34 provider relation IPC
and admission, and WP35 analysis/proof closure. Working-tree behavior proves progress, not packet
completion: WP32, WP34, and WP35 have no proving commit.

Current-tree evidence gathered for this reconciliation:

- `just artifacts-check` — exit 0; Ruff and all 15 artifact-contract tests pass.
- `just plan-status` — exit 0; the verbatim derivation is below.
- `just root-check-fast` — exit 0. The large unused/dead-code warning set corroborates that several
  WP32/WP35 release-owned paths are implemented but not yet joined into production composition.
- `git diff --check` — exit 0.
- WP33 — all four named recipes exit 0.
- WP29 — INT, NEG, and OPS exit 0; `programmatic-production-composition-check` exits 4 because its
  two selected library tests were removed and Nextest selected zero tests. The production binary
  integration selector still exists, but the recipe stops at the empty first selector.
- WP30 — all four named recipes exit 1 because the static activation-residue manifest still
  requires `ProgrammaticWorkspaceReleasePins`, which current WP32 work has removed.
- WP31 — all four named recipes exit 0.
- WP32 — INT, NEG, and OPS exit 0; `delta-exact-reconstruction-v4-check` exits 4 because no
  `wp32_beh_` test exists.
- WP34 — all four named recipes exit 0 across the stable root, Pyrefly sidecar, rustc extractor,
  generated Protobuf, and exact relation-stream boundaries.
- WP35 — INT, BEH, and NEG exit 0; `analysis-fixed-point-resource-check` exits 4 because no
  `wp35_ops_` test exists.
- Direct recipe census confirms all four WP36 recipes are absent; WP37 lacks
  `session-uds-presentation-boundary-rejection-check`; WP38 lacks
  `clean-reconstruction-evidence-check`; WP39 lacks two recipes; all four WP40 and WP41 recipes are
  absent; and WP42 lacks `relational-fabric-v4-certification`.

No full `ci-fast`, full stable-root test suite, clean-build matrix, benchmark, fresh activation, or
release certification was run. The focused packet failures and absent dependency-closed surfaces
already determine status; running terminal certification now could not establish completion.

## Derived Status Snapshot

The following is the verbatim `just plan-status` result after state reconciliation:

```json
{
  "accepted_input_evolutions": [],
  "baseline": {
    "ancestor": true,
    "commit": "6a76b5cff3d84e8249e5bedaa52a17f2abb816dd",
    "exists": true
  },
  "complete_decommission_batches": [],
  "complete_milestones": [
    "M02"
  ],
  "complete_packets": [
    "WP33",
    "WP31"
  ],
  "declared_input_count": 23,
  "healthy": true,
  "plan_path": "docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v4_2026-09-01.md",
  "stale_inputs": [],
  "untrusted_complete_entries": [],
  "untrusted_complete_packets": []
}
```

`healthy: true` means the baseline, declared inputs, and recorded completion ancestry are coherent.
It does not mean the plan is complete. Only WP33 and WP31 currently satisfy the state-level
completion contract.

## Reconciliation Decisions

### Overall decision

The v4 plan remains the correct execution authority and does not require a design replan. The tree
is materially ahead of the previous state: WP32, WP34, and WP35 are active, and WP34 appears ready
for an atomic proving commit. The plan is nevertheless far from terminal completion. WP29/WP30
have stale acceptance contracts at the current tree, WP32/WP35 each lack one substantive oracle
and production closure, and WP36 through WP42 have not landed as dependency-closed v4 packets.

The WP30 failure is specifically not a reason to restore `ProgrammaticWorkspaceReleasePins`.
Removal is the beneficial target change; the residue ledger and zero-state oracle must evolve to
record and enforce that removal.

### Packets

| Packet | Status | Established now | Remaining scope and instruction validity |
|---|---|---|---|
| WP33 | complete | Proving commit `5787862` is ancestral and all four independent authority/expectation recipes pass at the current tree. | Preserve the frozen successor authority and independent expectations. Instructions remain valid. |
| WP29 | stale | Proving commit `fe4e5b7` is ancestral; INT, NEG, OPS, and the production-binary integration test remain green. | Rebind the behavior oracle to substantive extant successor behavior. Do not restore the two deleted test seams. Completion cannot remain current while a named oracle selects zero tests. |
| WP30 | stale | Proving commit `917a081` is ancestral and the predecessor authority remains physically displaced. | Update the residue ledger/validator to record and positively reject the removed `ProgrammaticWorkspaceReleasePins`; rerun all four recipes. The decommission direction remains valid. |
| WP31 | complete | Proving commit `06a3112` is ancestral and all four schema/plan/cache/child-authority recipes pass at the current tree. | Preserve compiled release and DataFusion authority while downstream packets join it. Instructions remain valid. |
| WP32 | in_progress | Selected-event readback, receipt nonauthority, candidate-free recovery, and atomic-workspace work exists; INT, NEG, and OPS pass. | Complete exact reconstruction for every selected provider/canonical/derived/proof relation, a concrete release-owned rebuild, and an older selected version. Add a substantive WP32-BEH test, then record a proving commit. |
| WP34 | in_progress | The four descriptor, exact-batch, negative-gap, and IPC operations recipes all pass across all three Rust build domains and generated contracts. | Isolate the shared working-tree surface, rerun the quartet, and create one atomic proving commit. Until then it is packet-ready, not complete. Instructions remain valid. |
| WP35 | in_progress | Released producer-census/closure/proof work is present; INT, BEH, and NEG pass. | Join the release-owned analysis/proof closure into production, remove displaced caller-composition paths, add a substantive aggregate/fixed-point OPS test, and record a proving commit. Instructions remain valid. |
| WP36 | not_started | Some predecessor query/result implementation exists, but no v4 packet oracle exists and the current path still contains competing process-local maps and whole-result collection. | Implement the durable coordinator, streamed page sealing, immutable result packages, retention, cancellation, and all four named recipes after WP32/WP35. Instructions remain valid and are not yet ready. |
| WP37 | not_started | Older admin/v1/FastMCP scaffolding and three recipes exist. They do not prove the target v2 vertical: the daemon still lacks the joined lifecycle/query service and the Python adapter remains presentation over the older boundary. | After WP36, implement the real supervisor, public gRPC v2 UDS service, session/grant boundary, and presentation-only FastMCP delivery; replace placeholder recipes and add the missing NEG recipe. Instructions remain valid. |
| WP38 | not_started | Earlier evidence tooling exists, but the v4 clean-reconstruction recipe and completed WP37 production vertical do not. | Execute first-principles v4 evidence only after WP37; add the missing OPS recipe. Instructions remain valid but dependencies are unmet. |
| WP39 | not_started | Two older post-purge recipes exist. | After WP38, implement the v4 surface inventory and remaining-live-authority zero state, then close package/feature residue. Instructions remain valid but dependencies are unmet. |
| WP40 | not_started | No v4 packet recipe exists. | Add and execute the release matrix, post-purge behavior, independent history-comparator rejection, and daemon boundary benchmark after WP39. |
| WP41 | not_started | No v4 fresh-activation recipe exists. | Execute a fresh successor activation, prove sole target authority and reconciliation, and delete dormant handoff only after WP40. |
| WP42 | not_started | Three certification-support recipes pre-exist, but the v4 certification recipe and trusted candidate HEAD do not. | Add `relational-fabric-v4-certification` and execute all 56 packet oracles, zero state, four domains, and independent review at one trusted HEAD after WP41. |

### Milestones and decommission batches

| Entry | Status | Reconciliation |
|---|---|---|
| M02 | complete | WP33 authority and expectations have an ancestral proving commit and current green oracles. |
| M03 | stale | WP29/WP30 target code and proving commits exist, but their current named acceptance contracts are stale. |
| M04 | in_progress | WP31 is complete, WP32/WP34/WP35 are active, and WP36 has not started. |
| M05 | not_started | WP37's real daemon-to-FastMCP vertical is absent. |
| M06 | not_started | WP38–WP40 dependencies are not closed. |
| M07 | not_started | WP41 fresh activation and WP42 certification are absent. |
| DB09 | in_progress | WP30 decommission remains real; WP32 must finish exact activation residue disposition and revalidation. |
| DB10 | in_progress | WP34/WP35 cleanup is active; WP36 query/result decommission has not started. |
| DB11 | not_started | The v1 serving/lifecycle/adapter replacement belongs to unstarted WP37. |
| DB12 | not_started | Requires completed first-principles evidence and WP39 purge. |
| DB13 | not_started | Requires DB09–DB12 and WP39 package/feature cleanup. |
| DB14 | not_started | Requires post-purge evidence, fresh activation, and final certification. |

## Blockers and Invalidated Assumptions

1. **WP29/WP30 evidence drift invalidates current completion, not the target architecture.** WP29's
   behavior filter selects deleted tests; WP30's static ledger requires a symbol that the target
   correctly removed. Neither failure authorizes legacy restoration.
2. **WP32 is not exact across the whole epoch yet.** Current durability/reopen work covers the
   provisioned observation histories, but not the complete provider/canonical/derived/proof version
   closure or an older selected epoch through the production rebuild port.
3. **WP34 lacks proof lineage only.** Its named packet evidence is green, but no uncommitted packet
   can satisfy the proving-commit contract.
4. **WP35 is not production-closed.** Substantial released analysis/decoded proof code remains
   unused from production, and its OPS selector is empty.
5. **WP36 is the first wholly absent capability packet.** Query coordination, page streaming,
   durable result packages, retention, and restart-safe cancellation remain predecessor-shaped;
   all four v4 recipes are absent.
6. **WP37 is not a real v2 transport vertical.** Existing v1/admin/presentation tests cannot prove
   the accepted lifecycle-to-UDS/gRPC/FastMCP boundary, and its boundary-rejection recipe is absent.
7. **Terminal evidence cannot be inferred.** WP38–WP42 are dependency-blocked implementation work,
   with 16 named recipes absent across WP38–WP42. No fresh activation or certification claim is
   currently supportable.

There is no external blocker and no invalidated target-design premise. The immediate defects are
bounded implementation and assurance work already represented by the plan.

## Recommended Resume Order

1. Repair and revalidate the stale WP29/WP30 assurance contracts without restoring removed
   authority.
2. Atomically commit the packet-ready WP34 surface and record its proving commit.
3. Finish and prove WP35's production analysis/proof closure.
4. Finish and prove WP32's complete exact-version reconstruction and concrete rebuild path.
5. Implement WP36, then WP37 as the real joined daemon/gRPC/FastMCP vertical.
6. Execute WP38, WP39, and WP40 in plan order.
7. Execute WP41 fresh activation and WP42 certification at one trusted HEAD.

WP34 may be checkpointed before the longer WP32/WP35 completion work because its dependencies
(WP31 and WP33) are complete and its current packet quartet is green. Do not label it complete
until its atomic commit is ancestral and the named checks are rerun from that state.

## Exact Next Action

Retarget `programmatic-production-composition-check` to substantive current successor behavior;
update `tooling/ci/wp30_activation_residue.json` and its validator/tests so the removed
`ProgrammaticWorkspaceReleasePins` is recorded as disposed and positively forbidden; rerun all
eight named WP29/WP30 recipes; and record target-aligned revalidation without restoring deleted
authority. Then isolate WP34's owned paths, rerun its four recipes, and create its proving commit.

## State Reconciliation Summary

The schema-v2 state was reconciled from its committed WP32-only execution view to the current tree.
Overall status remains `executing`; `current_packet` is WP29 for the required evidence repair. WP33
and WP31 remain complete. WP29, WP30, and M03 are stale; WP32, WP34, WP35, M04, DB09, and DB10 are
in progress; WP36–WP42, M05–M07, and DB11–DB14 are not started. Seven durable obligations and the
exact next action were added. No proving commit was invented, no derived check output or dirty-tree
digest was stored in state, and no beneficial architectural deletion was reversed.
