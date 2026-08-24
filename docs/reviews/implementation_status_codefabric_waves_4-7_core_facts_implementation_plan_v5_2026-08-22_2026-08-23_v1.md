---
artifact: implementation-status
plan_path: docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v5_2026-08-22.md
state_path: docs/plans/state/codefabric-waves-4-7-core-facts_v5_state.json
version: v1
date: 2026-08-23
status: complete
---

# Implementation Status: CodeFabric Waves 4–7 core facts v5

## Provenance

- Audited plan: `docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v5_2026-08-22.md`.
- Reconciled state: `docs/plans/state/codefabric-waves-4-7-core-facts_v5_state.json`.
- Plan baseline: `3830acade129b2a63a1927ac5e2f4d3ac284f38c`, present and an ancestor of HEAD.
- Audited HEAD: `eb27a5bac12027d5cf8950d847e36efd5305a355`.
- The working tree was materially dirty before this audit. Current uncommitted implementation was treated as evidence of progress, never as a proving commit.
- Repository derivation: `just plan-status — exit 1` because three declared inputs are stale; no completed packet or completion entry is untrusted by ancestry/oracle resolution.
- Focused checks used for judgment:
  - `cargo test --test integration wp — exit 0` (21 passed, including WP27 and the existing Git substrate).
  - `cargo test --lib wp3 — exit 0` (35 passed, covering current WP30–WP39 focused tests).
  - `cargo test --lib wp4 — exit 0` (14 passed, covering current WP41–WP48 focused tests).
  - `cargo test --test integration wp5 — exit 0` (4 passed, covering current WP51–WP53 tests).
  - `just extractor-test — exit 0` (6 passed).
  - `just root-check — exit 0`.
  - `just stable-graph-check — exit 0`.
  - `just model-family-check registry-cbef — exit 0`.
  - `just model-family-check schema — exit 2` because `schema` is not a live family selector.
  - `just model-family-check schemas — exit 100` because one TableSpec/DDL projection test reports 26 declarations versus 24 expected.
  - `just model-check edit — exit 1` because the DesiredTree shadow plan is non-zero.
  - `just adapter-test — exit 1` with 82 passed and the locked STDIO no-daemon startup test failed.
  - Three focused operational-store migration tests each exited 101; details are under blockers below.
  - `just artifacts-check — exit 1` at the model-tooling formatting prerequisite for two Python files.
  - `just features-each — exit 1` was operator-interrupted after the maximal and featureless checks completed; this is not evidence of a code failure or a pass.

## Derived Status Snapshot

```json
{
  "accepted_input_evolutions": [],
  "baseline": {
    "ancestor": true,
    "commit": "3830acade129b2a63a1927ac5e2f4d3ac284f38c",
    "exists": true
  },
  "complete_decommission_batches": [],
  "complete_milestones": [],
  "complete_packets": [
    "WP27",
    "WP28",
    "WP29",
    "WP30",
    "WP31"
  ],
  "declared_input_count": 12,
  "healthy": false,
  "plan_path": "docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v5_2026-08-22.md",
  "stale_inputs": [
    "docs/upfront_design/code_property_graph_present_state_fact_ontology_specification_v1.3.md",
    "docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md",
    "docs/upfront_design/present_state_cpg_fastmcp_serving_specification_v1.3.md"
  ],
  "untrusted_complete_entries": [],
  "untrusted_complete_packets": []
}
```

## Reconciliation Decisions

### Trusted completed packets

WP27–WP31 remain `complete`. Their proving commits are trusted ancestors. Their named acceptance tests pass in the current tree, `root-check` and `stable-graph-check` pass, and the current registry/CBEF family proof passes. Later model-control-plane changes replaced implementation mechanics without invalidating the preserved packet outcomes. The interrupted feature matrix must run again at M05, but it did not produce a code failure and does not by itself reopen an otherwise focused, green historical packet.

### Non-complete packet status

| Packet | Status | What is proved now | Remaining scope and exact evidence | Plan validity |
|---|---|---|---|---|
| WP32 | in_progress | All four named WP32 unit oracles pass; registry/CBEF family and root checks pass. | Repair the 26-versus-24 schema projection failure; reconcile the non-zero `model-plan`; clear migration and adapter regressions so packet gates can pass; run the full WP32 matrix and record a proving commit. | Valid, with execution correction: use live selector `schemas`, not stale `schema`. |
| WP33 | in_progress | Admission policy, capability algebra, provider disposition census, deterministic source-lane replay, and operational tests pass. | Add `wave4-integration-check`; bind the complete four named oracles; prove malformed/oversized/binary/excluded/cancelled/unknown cases and every raw-kind disposition; wait for WP32. | Valid. |
| WP34 | in_progress | The owner-accepted golden candidate, expected groups, scenarios, exact-byte checks, and tamper/missing/extra negatives exist and pass. | Complete M05 dependency, independent KAT/census proof, and packet proving commit; keep answers outside renderer outputs. | Valid. |
| WP35 | in_progress | Daemon protocol validation, nightly wrapper, real `rustc_public` golden extraction, deterministic typed Arrow output, handshake, and cancellation tests pass. | Complete M05/WP34 dependencies, full `extractor-ci-fast` and protocol/toolchain/model-provenance packet proof, then commit. GEN input evolution remains stale until that commit. | Valid and extended by the recorded input evolution. |
| WP36 | in_progress | Rustc Arrow observations reconcile into validated canonical batches; schema/payload drift is rejected atomically in focused tests. | Complete provider precedence/conflict/property/state-digest cases, invalid-batch atomicity, and DB07 structural/compiler zero-state; then commit. | Valid. |
| WP37 | in_progress | Deterministic full/incremental `SYNTAX_TREE_V1` equality and missing/cross-owner negatives pass. | Finish registered execution, projection persistence, provenance, invalidation, independent query, and model graph explanation. | Valid. |
| WP38 | in_progress | The three Wave-5 forms validate and execute canonically; unknown-field, duplicate, and budget negatives pass. | Add independent request/response KATs, snapshot-handle/error/state-digest coverage, registry phrase resolution, and model query-contract proof. | Valid. |
| WP39 | in_progress | Rust UDS handshake, accepted handles, streaming, lease-token artifacts, cancellation, deadlines, checksums, and limits pass focused tests. | Fix locked STDIO startup/EOF behavior without a live daemon; complete descriptor/client/wheel proof and canonical result identity. SRV input evolution remains stale until a trusted commit. | Valid and extended by the recorded input evolution. |
| WP40 | not_started | No WP40-specific integration recipe or executable Gate-B proof exists. Prerequisite components and fixtures do not prove the integrated outcome. | Add and pass `wave5-integration-check` and `gate-b-check`; execute all eleven artifacts over accepted answers with cache-disabled/full equivalence and required domain gates. | Valid. |
| WP41 | in_progress | Watch-hint normalization, overflow widening, native delivery, and clean shutdown tests pass. | Complete M06 dependency, platform/fallback equivalence, resource cleanup, and packet proof. | Valid. |
| WP42 | in_progress | Dirty coalescing and current-byte-fenced persisted update-wave scheduling tests pass. | Complete burst/backpressure/retry/restart/source-drift proof and DB09 migration/no-emission proof. | Valid. |
| WP43 | in_progress | Deterministic dependency closure, cycle handling, atomic replacement, and persistence round trip pass. | Fix the migration regression caused by the new table; prove rollback/crash, stale-fact non-exposure, and full-rebuild differential. | Valid. |
| WP44 | in_progress | Incremental parser equality and clean canonical fact equality pass. | Complete edit-sequence/error/overflow/source-drift/fallback corpus, overlay publication, and strict-current negatives. | Valid. |
| WP45 | in_progress | Freshness-barrier concurrency and strict-current service rejection pass; no temporary freshness-shim text was found in the declared source scope. | Complete formal transition/restart proof and DB08 structural zero-state after dependencies. | Valid. |
| WP46 | in_progress | Structural-pressure flush policy has a focused passing test. | Implement/prove complete rebase/flush execution, idempotent failure/retry/crash, visibility, reader isolation, and rebuild equality. | Valid. |
| WP47 | in_progress | Historical in-flight restart, unfinished-path recovery, and rescan fencing tests pass. | Complete cold/warm/kill-point/corrupt-state/idempotency/stale-result/resource-cleanup corpus. | Valid. |
| WP48 | in_progress | A semantic incremental-versus-rebuild comparator test and the wider golden scenario corpus exist. | Add `rebuild-equivalence-check` and `wave6-integration-check`; execute the full core edit corpus; bind exact `CORE_SOURCE_V1` coverage and advertisement. | Valid. |
| WP49 | in_progress | The existing Git adapter provides byte-safe repository/worktree topology and detached DTOs; current integration tests pass. | After M07, add explicit WP49 proof for all repository states, gix-disabled equivalence, boundary leakage, and a proving commit. | Valid; substantial prior substrate is reused. |
| WP50 | in_progress | Git-native inventory and authoritative current-byte integration provide coherent plan-specific implementation evidence. | Complete the eight-class inclusion/fallback corpus, incomplete-topology widening, M07/WP49 dependencies, and proving commit. | Valid; substantial prior substrate is reused. |
| WP51 | in_progress | Advisory status/index candidates and fresh-vector fencing pass a focused integration test. | Add `git-parity-check`; prove accepted isolated-save scan behavior and forced fallback equality; commit. | Valid. |
| WP52 | in_progress | HEAD-tree transition candidates and stale-vector identity rejection pass a focused integration test. | Complete branch/index/rename/submodule/topology/concurrent-change corpus and exact fallback equality. | Valid. |
| WP53 | in_progress | Bounded L1/L2 caches, stale/corrupt misses, strategy selection, and generic degradation pass focused tests. | Fix cache-table migration compatibility; add `wave7-integration-check`; prove gix/cache-disabled/full-rebuild equivalence, eviction, restart, and linked-worktree cases. | Valid. |

### Milestones and decommission batches

- M05–M08 remain `not_started`: none has its required dependency closure and named integration recipes green.
- DB07 is `in_progress`: `SyntheticCanonicalIngest` has no text hits in the declared source/test/contract/tooling scope and production reconciliation exists, but WP36 and structural/compiler zero-state proof are incomplete.
- DB08 is `in_progress`: no freshness-shim text hit was found and strict-current admission uses the barrier in focused tests, but WP45 and structural proof are incomplete.
- DB09 is `in_progress`: the registry marks historical states decode-only and recovery logic handles restart/retirement, but WP42 migration and no-emission proof are incomplete.

## Blockers and Invalidated Assumptions

No packet is externally `blocked`, and no target-design decision is `invalidated`. The following implementation defects or unproved conditions prevent forward completion:

1. Three declared design inputs are stale. They are assigned as planned input evolutions to WP32, WP35, and WP39 and cannot become accepted until those packets have trusted proving commits.
2. `just model-family-check schemas` fails `model_tablespec_projects_equivalent_arrow_json_schema_and_ddl` with 26 generated declarations versus 24 expected.
3. `just model-check edit` fails because the DesiredTree shadow plan is non-zero; `just model-plan` is the reproducible read-only derivation.
4. `operational_store::tests::wp13_operational_acceptance` fails because the migration-backup assertion still searches for `pre-migration-v6` after schema version advanced to 8.
5. `operational_store::tests::wp14_operational_schema_v1_migrates_to_current` and `source_image::tests::wp16_operational_schema_v2_migrates_to_v3` fail because `operational_dependency_edge` already exists when the migration tries to create it. The same migration chain must cover `git_candidate_cache` exactly once.
6. `just adapter-test` fails the locked STDIO startup test because adapter lifespan eagerly connects to `/tmp/codefabric.sock`; the process exits 1 when the daemon is absent.
7. `just artifacts-check` stops at Ruff formatting drift in `tooling/model/proto_contract.py` and `tooling/model/validate_staged_schemas.py`.
8. The stable recipes `wave4-integration-check`, `wave5-integration-check`, `gate-b-check`, `rebuild-equivalence-check`, `wave6-integration-check`, `git-parity-check`, and `wave7-integration-check` do not exist yet.
9. WP32–WP39 and WP41–WP53 have no proving commits. Passing focused tests in a dirty tree cannot produce `complete` status.

## Recommended Resume Order

1. Finish WP32 as the dependency-closed current packet. Repair the schema projection and operational migration regressions; reconcile model outputs through confirmed `model-sync`; prevent ahead-of-sequence WP39 adapter work from breaking Tier A; rerun WP32 and repository gates; commit and record the proving commit.
2. Finish WP33 and M05, including the stable Wave-4 integration recipe and complete raw-kind disposition proof.
3. Follow the accepted Wave-5 dependency chain WP34 → WP35 → WP36/DB07 → WP37 → WP38 → WP39 → WP40 → M06. Existing partial work reduces implementation effort but does not remove dependency or proof obligations.
4. Follow WP41 → WP42/DB09 → WP43 → WP44 → WP45/DB08 → WP46 → WP47 → WP48 → M07.
5. Follow WP49 → WP50 → WP51 → WP52 → WP53 → M08, reusing the existing Git substrate and adding the missing equivalence/integration proofs.
6. Run the final gate matrix and an independent implementation review only after all packets, milestones, and decommission batches have trusted proving commits.

## Exact Next Action

Finish WP32 without starting or claiming WP33:

1. Correct the schema-family assertion so `just model-family-check schemas` passes and retain `schemas` as the live selector in execution state.
2. Repair schema versions 6–8 and prior-schema fixtures so `operational_dependency_edge` and `git_candidate_cache` are created exactly once and backup assertions are version-current.
3. Restore the locked adapter STDIO no-daemon startup/EOF invariant or otherwise remove that out-of-sequence regression without discarding the in-progress WP39 work.
4. Reconcile the read-only `just model-plan` changes through confirmed `model-sync`, inspect the diff, and require a zero-action model plan.
5. Run `cargo test --lib wp3`, `just model-family-check registry-cbef`, `just model-family-check schemas`, `just model-check edit`, `just root-test`, and `just ci-fast`.
6. Commit the dependency-closed WP32 result and record its proving commit in state before advancing to WP33.

## State Reconciliation Summary

- Overall state remains `executing`; `current_packet` remains WP32.
- WP27–WP31 remain `complete`.
- WP32–WP39 and WP41–WP53 are now `in_progress`, reflecting coherent current-tree implementation evidence without proving commits.
- WP40 remains `not_started` because no packet-specific integration implementation or recipe exists.
- M05–M08 remain `not_started`.
- DB07–DB09 are now `in_progress`.
- Three planned design-input evolutions and five newly discovered implementation obligations were appended without deleting prior provenance.
- The plan and design artifacts were not edited.
