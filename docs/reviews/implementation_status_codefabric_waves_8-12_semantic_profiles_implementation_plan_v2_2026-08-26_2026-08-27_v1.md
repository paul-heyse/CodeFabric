---
artifact: implementation-status
plan_path: docs/plans/codefabric_waves_8-12_semantic_profiles_implementation_plan_v2_2026-08-26.md
state_path: docs/plans/state/codefabric-waves-8-12-semantic-profiles_v2_state.json
version: v1
date: 2026-08-27
status: complete
---

# Implementation Status: CodeFabric Waves 8–12 semantic profiles v2

## Provenance

- Plan assessed: `docs/plans/codefabric_waves_8-12_semantic_profiles_implementation_plan_v2_2026-08-26.md`.
- State reconciled: `docs/plans/state/codefabric-waves-8-12-semantic-profiles_v2_state.json`.
- Baseline `ea13ca41617dce93ea76349f34bfbd5739f7a5a2` exists and is an ancestor of audited HEAD `43680b8`.
- The implementation worktree was materially dirty at audit intake. Its uncommitted WP07 code and generated-contract changes are progress evidence only; they are not a proving commit.
- Derived and schema checks: `just plan-status — exit 0`; `just artifacts-check — exit 0`; `just model-repro-check — exit 0`.
- Focused current-tree revalidation, all through the plan's four-oracle selector with Cargo nextest `--no-fail-fast`:
  - `just packet-oracle-check WP01 — exit 0`.
  - `just packet-oracle-check WP36 — exit 0`.
  - `just packet-oracle-check WP02 — exit 0`.
  - `just packet-oracle-check WP03 — exit 0`.
  - `just packet-oracle-check WP04 — exit 1`: three oracles passed; `py_import_export_fixture_conformance` failed because a frozen canonical-entity census expected 3 rows after the additive WP07 projection produced 7.
  - `just packet-oracle-check WP05 — exit 0`.
  - `just packet-oracle-check WP06 — exit 0`.
  - `just packet-oracle-check WP07 — exit 1`: all four planned WP07 oracle definitions are absent.
- No wave-wide or repository-wide test suite was run. This preserves the user-directed cadence: focused tests per item, repository-wide gates at the end of the wave. The earlier WP06 no-fail-fast full-suite discovery and focused repairs remain recorded in execution state; they are not represented here as a current all-suite pass.

## Derived Status Snapshot

```json
{
  "accepted_input_evolutions": [],
  "baseline": {
    "ancestor": true,
    "commit": "ea13ca41617dce93ea76349f34bfbd5739f7a5a2",
    "exists": true
  },
  "complete_decommission_batches": [],
  "complete_milestones": [],
  "complete_packets": [
    "WP01",
    "WP36",
    "WP02",
    "WP03"
  ],
  "declared_input_count": 20,
  "healthy": true,
  "plan_path": "docs/plans/codefabric_waves_8-12_semantic_profiles_implementation_plan_v2_2026-08-26.md",
  "stale_inputs": [],
  "untrusted_complete_entries": [],
  "untrusted_complete_packets": []
}
```

`healthy: true` is the derivation result for input freshness, ancestry, and the entries still recorded complete. It is not a statement that WP07, M01, or the full Waves 8–12 plan is complete.

## Reconciliation Decisions

### Packets

| Packet(s) | Reconciled status | What is proved | What remains | Original instructions |
|---|---|---|---|---|
| WP01, WP36, WP02, WP03 | complete | Proving commits are in current ancestry; declared inputs are fresh; each packet's four named oracles passes in the current WP07 worktree. | Re-run only if later work changes their contracts or an M01 gate exposes a regression. | Valid. |
| WP04 | stale | Three of four current oracles pass; import/export distinction, dynamic-unknown behavior, and replacement behavior remain focused-green. | Repair `py_import_export_fixture_conformance` so its entity assertion proves WP04's import/export identity set without freezing the size of the shared additive entity table; then pass all four oracles at HEAD. | Valid. This is a proof-harness drift defect, not evidence that the target design is invalid. |
| WP05 | stale | Its own four named callable/call-site oracles pass at HEAD. | WP04 is a declared dependency and is stale. Re-run WP05's selector after WP04 is repaired before restoring trusted completion. | Valid. No WP05 behavioral regression was observed. |
| WP06 | stale | Its own four CFG oracles pass at HEAD. | WP06 depends transitively on WP04 through WP05. Re-run WP06's selector after dependency trust is restored. | Valid. No WP06 behavioral regression was observed. |
| WP07 | in_progress | The current worktree contains a compiled application-owned dataflow module, the single `PY_OWNER_REACHING_DEFS_V1` registry entry, generated table/ingest projections for the new dataflow families, canonical projection and validation, aggregation-derived Python profile state, named unavailable-provider children, and a parse-unavailable semantic boundary. Model reproduction passes, and adjacent WP03/WP05/WP06 behavior remains focused-green. | There is no proving commit. The four required oracles `py_defuse_fixture_conformance`, `py_semantic_profile_partial_parity`, `py_parse_error_capability_gap_falsification`, and `wave8_integration_operational_gate` are not defined. `wave8-integration-check` still selects only WP02–WP06 oracles. Exact def-use/merge semantics, publication-authority rejection, parse-error source/CST retention plus semantic withdrawal, formal profile closure, incremental-versus-clean equality, and the Wave 8 operational gate therefore remain unproved. | Valid. No resource-envelope or design replan trigger has fired. Finish the existing packet; do not weaken its first-principles oracles into row-count or digest checks. |
| WP08–WP14 and WP38 | not_started | The shared provider substrate from WP36 exists, but no Wave 9 packet-specific implementation evidence was found. | All Wave 9 packet outcomes, proving commits, and M02 remain. WP08 is not ready while M01 is open. | Valid after M01. |
| WP15–WP28 | not_started | Pre-existing extractor substrate is not plan-specific proof for these Wave 10–11 outcomes. | All packet outcomes, proving commits, M03, and M04 remain. | Valid when their dependencies are satisfied. |
| WP29–WP35 and WP37 | not_started | Inherited Gate B seams are recorded as obligations, not accepted proof of Wave 12 closure. | All reconciliation, completeness, context, FFI, derivation, decommission, profile-revalidation, and M05 work remains. | Valid after M02 and M04. |

### Milestones and decommission batches

- M01 remains `not_started` as a gate: WP04–WP06 are stale, WP07 is incomplete, the WP07 oracles are absent from `wave8-integration-check`, and none of the M01 wave-conclusion gates has been run on a candidate commit.
- M02–M05 remain `not_started`; their prerequisite waves have not begun.
- DB01 and DB03 remain `not_started` because their provider-path and typed-observation prerequisites are later packets.
- DB02 is `in_progress`: WP01 established coherent generated observation-schema authority, `handwritten_observation_schema_falsification` is current-green, and model reproduction is current-green. Completion still requires the governed `semantic-provider-legacy-zero-state-check` at M01 and a proving commit; that repository-wide negative gate was intentionally not pulled forward into this item-level audit.

## Blockers and Invalidated Assumptions

No external blocker and no invalidated target-design or library decision was found. Execution cannot safely advance to WP08 for two internal reasons:

1. WP04's required current-tree behavioral oracle fails, which makes WP04 stale and breaks dependency trust for WP05 and WP06 even though their own focused oracles pass.
2. WP07 has substantial coherent implementation but lacks every named acceptance oracle, an operational Wave 8 selector, packet-local proof, and a proving commit.

The predecessor Gate B semantic seam remains open exactly as the approved plan and state record it. The user's explicit sequencing approval permits this plan to execute; it does not convert that predecessor obligation into completion evidence. Those inherited seams remain assigned to their later owning packets.

## Recommended Resume Order

1. Repair the stale WP04 proof without simply changing `3` to `7`: make the fixture compare the intended module/import/export canonical identities or a derived WP04 subset, so additive WP07 entity families cannot invalidate it while missing or duplicate WP04 entities still fail it.
2. Run `just packet-oracle-check WP04`, then WP05 and WP06. Restore `complete` only if all three selectors pass at HEAD.
3. Finish WP07's four first-principles oracles: exact reaching-definition/def-use rows and merge provenance; aggregation-derived `PARTIAL` with named Pyrefly gaps; parse-error withdrawal/source+CST/gap behavior plus rejection of unauthorized derived-row publication; and incremental continuous-wave equality with a clean rebuild.
4. Add the four WP07 oracles to the packet selector and the non-recursive Wave 8 selector, then pass `just packet-oracle-check WP07`.
5. At the Wave 8 conclusion, run the M01 packet-local and repository-wide gates named by the plan, including `wave8-integration-check`, Python `rebuild-equivalence-check`, `root-ci-fast`, model/property/fault/observability proof, DB02 zero state, and the Wave 8 benchmark. Run Cargo nextest scopes with `--no-fail-fast`.
6. Commit the dependency-closed WP07/M01 result, record proving commits, and only then make WP08 ready.

## Exact Next Action

At `src/ruff_adapter/semantic.rs` in `py_import_export_fixture_conformance`, replace the frozen whole-table `projection.batch(9).num_rows() == 3` assertion with a semantic assertion over the WP04-owned module/import/export entity identities that still detects omission or duplication while permitting additive WP07 dataflow entities. Then run, in order:

```text
just packet-oracle-check WP04
just packet-oracle-check WP05
just packet-oracle-check WP06
```

Do not resume WP07 or run the Wave 8 repository-wide gates until those dependency proofs are current-green.

## State Reconciliation Summary

- Overall state remains `executing`.
- `current_packet` now points to reopened WP04; WP07 remains recorded `in_progress` so its uncommitted work is not lost or misrepresented.
- WP01, WP36, WP02, and WP03 remain `complete`.
- WP04 is `stale` from a named-oracle failure; WP05 and WP06 are `stale` by dependency trust.
- WP07 remains `in_progress`; all later packets remain `not_started`.
- DB02 moved to `in_progress`; DB01 and DB03 remain `not_started`.
- M01–M05 remain `not_started`.
- The stale `next_action` that still named WP06 implementation was replaced with the exact WP04 repair and dependency revalidation sequence.
- No plan or design artifact was edited.
