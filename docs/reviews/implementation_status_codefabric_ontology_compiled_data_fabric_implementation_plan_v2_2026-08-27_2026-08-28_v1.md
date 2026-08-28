---
artifact: implementation-status
plan_path: docs/plans/codefabric_ontology_compiled_data_fabric_implementation_plan_v2_2026-08-27.md
state_path: docs/plans/state/codefabric-ontology-compiled-data-fabric_v2_state.json
version: v1
date: 2026-08-28
status: complete
---

# Implementation Status: CodeFabric ontology-compiled data fabric plan v2

## Provenance

This reconciliation reviews the accepted v2 implementation plan against its accepted
v3 target design, execution state, current tree, named packet oracles, and proving-commit
requirements. The plan baseline is
`eb7a738fa55037b19706fd842737cecad65ffe16`; current HEAD is
`71a888fed8aae660f97a8bc420f04a039f5aacae`, and the baseline is an ancestor of HEAD.

The implementation is present only in the dirty working tree. Reproduce that fact with
`git status --short --branch`; no packet has a proving commit. The focused status checks
were:

- `just plan-status` — exit 1.
- `env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp pytest -q tooling/ci/test_ontology_fabric_stage0.py tooling/ci/test_ontology_compiled_data_fabric.py` — exit 1; 66 passed and 2 failed.
- `just model-repro-check` — exit 0; the aggregate DesiredTree reproduced at identity
  `b3:4927035f82ca78def5d70f5aa6574420e311b1953da85fe66e033afc766f1635`.
- `rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' -g '!target/**' 'codefabric\.id16|cpg_base\.enum_catalog|\bmodel_field\b|Statistics::new_unknown' src contracts tooling scripts docs/upfront_design docs/spec_index justfile` — the only live retired-ID hits are in `tooling/model/schema_consumer.rs`; the statistics hit is the oracle's negative assertion.
- `ast-grep run -l rust -p 'activate_stage2b($$$A)' src tests --inspect summary` — zero call sites over 103 scanned Rust files, with 0 skipped files.
- `rg -n 'ontology|DomainConformance|ResultChecksumV2|Stage2b|stage2b|id_domain' tests src -g '*.rs'` and direct reads of the new modules — no Rust behavioral tests exercise the new ontology, analyzer, or activation surfaces.

No performance measurement or performance gate was run. This follows the explicit owner
waiver already recorded in execution state.

## Derived Status Snapshot

The following is the `artifact-schemas.md` section 8 derivation from
`just plan-status`, reproduced verbatim:

```text
{
  "accepted_input_evolutions": [],
  "baseline": {
    "ancestor": true,
    "commit": "eb7a738fa55037b19706fd842737cecad65ffe16",
    "exists": true
  },
  "complete_decommission_batches": [],
  "complete_milestones": [],
  "complete_packets": [],
  "declared_input_count": 13,
  "healthy": false,
  "plan_path": "docs/plans/codefabric_ontology_compiled_data_fabric_implementation_plan_v2_2026-08-27.md",
  "stale_inputs": [
    "contracts/schema/schema-contract-ir.json",
    "contracts/registry/phrase-registry.yaml",
    "contracts/query/query-form-contract.json"
  ],
  "untrusted_complete_entries": [],
  "untrusted_complete_packets": []
}
error: recipe `plan-status` failed on line 719 with exit code 1
```

The derived result is controlling: zero packets, milestones, and decommission batches
are complete. The stale inputs are intentional implementation surfaces, but they are not
accepted input evolutions until an owning packet has a trusted proving commit.

## Reconciliation Decisions

All packets are reconciled to `in_progress`. The current tree contains coherent
packet-specific implementation, so `not_started` would understate progress; none can be
`complete` because every `proving_commit` is null, declared-input freshness is red, and
the acceptance evidence is incomplete. No target-design or pinned-library decision is
invalidated.

| Packet | What is proved now | What remains before completion | Instructions still valid? |
|---|---|---|---|
| WP01 | Gate-filter census assertions and five of six promoted library tests execute successfully. | Repair the duplicate `candidate_source_id` schema failure; rerun the promoted selector and checksum KATs; create a proving commit. | Yes, except the owner-waived performance anchor. |
| WP02 | The eight PR-1…PR-7 contract entries, pinned identities, fallbacks, and target-only report path exist. | Run genuine probe behaviors, produce content-addressed reports, record reviewed decisions, and prove dependent-packet drift rejection. No report files are currently present. | Yes, except PR-7 performance measurement is owner-waived. |
| WP03 | One compiled-model/generation path exists and `just model-repro-check` passes. | Prove exact pre/post schema fingerprint equality and downstream single-consumer closure, then commit. | Yes. |
| WP04 | Generated row shapes replace the 29 hand-written public `*Row` definitions in `fact_ingest.rs`. | Add an encoding/replay equivalence test that compares batches and publication digest, then commit. | Yes. |
| WP05 | Registry-backed phrase operations and generated semantic operation specs exist; literal predicates were refactored. | Execute relational-versus-graph parity and governance seeded-negative tests; reconcile the changed phrase registry through a proving commit. | Yes. |
| WP06 | `SessionStateBuilder`, extension registry, analyzer seam, and centralized field validation are present and compile. | The required serving-equivalence behavior currently fails on duplicate field construction; fix and rerun the packet gates. | Yes. |
| WP07 | The Contract IR and generated code enumerate per-domain extensions and retire `codefabric.id16` in the main runtime. | Migrate `tooling/model/schema_consumer.rs`, execute per-domain round trips and republish migration, reconcile the changed Contract IR, and commit. | Yes. |
| WP08 | `DomainConformanceRule` is installed in serving session construction. | Add executable same-domain, cross-domain, cast, `IN`, set-operation, all-ingress, and idempotence tests; prove the former gate's zero state. | Yes. |
| WP09 | The Contract IR has exactly twenty ontology relations and generated ontology/dimension builders exist. | Execute batch parity, FK closure, ontology-term/edge completeness, and unchanged-version tests against real batches. | Yes. |
| WP10 | `cpg_ontology` catalog registration, namespace constants, and decoration-plan code exist. | Execute frozen-catalog, namespace resolution, decorated projection, and plan-shape behavior; reprove serving equivalence. | Yes. |
| WP11 | Eleven typed rule contracts exist and publication calls `validate_compiled_ontology_rules`. | Add seeded violation tests for every operation family, including property one-of and relational membership closure. | Yes. |
| WP12 | Logical structure classifications and the selected flat source-span lowering are recorded. | Execute partially populated span rejection and production pruning/result parity. | Yes. |
| WP13 | Generated result schemas and ResultChecksumV2/V1 dispatch code exist. | Add eight-form delivered-batch conformance, typed-list wire equivalence, V2 KATs, V1 continuity, and result-version selection tests; reconcile the query-form input. | Yes. |
| WP14 | All 27 operational projections carry logical types and the capture code has fixed-width builders. | Execute typed capture, wrong-width fail-closed, timestamp, cross-namespace join, and pre/post value-equivalence tests. | Yes. |
| WP15 | Statistics composition code exists; the four promoted overlay tests pass. | Execute the mutation-class precision matrix, adversarial pushdown falsification, and uniqueness-gated constraint classification. | Yes; no performance comparison is required. |
| WP16 | The suite/index amendments, waves reconciliation review, and waves-state disposition are present. | Restore artifact freshness and run artifact/dependency validation on the reconciled state and review. | Yes. |
| WP17 | Dynamic ontology discovery, candidate dossier checks, and the durable activation transaction exist as APIs. | Wire or explicitly expose the activation owner, then execute complete-candidate, six-fault rollback, acceptance, pointer-CAS, retry-idempotence, active-lease, and dimension-version-stability tests. `activate_stage2b` currently has no call site. | Yes, except the performance-comparator clause is owner-waived. |

The packet table is based on the named focused run plus current-tree consumer tracing.
The 66 green plan-specific oracle nodes are useful structural progress evidence, but most read source
text and do not execute the behavior named by their acceptance criteria. They therefore
cannot substitute for the behavioral work listed above.

Milestones M01–M04 are `in_progress`: constituent implementations exist, but their packet
prerequisites are not complete and no milestone proof has run. DB01–DB06 are also
`in_progress`: several negative source assertions are green, but the required dual-tool,
compiler, behavioral, and proving-commit closure is absent. DB01 specifically still has a
live legacy consumer in `tooling/model/schema_consumer.rs`.

## Blockers and Invalidated Assumptions

There is no external blocker and no invalidated target-design or library decision. The
current completion blockers are implementation/proof obligations:

1. `datafusion_55_serving_equivalence` fails with DataFusion
   `DuplicateUnqualifiedField { name: "candidate_source_id" }` at
   `src/fabric/serving.rs:3656`.
2. Declared-input freshness is red for the Contract IR, phrase registry, and query-form
   contract. The plan is immutable; these require governed input-evolution disposition
   plus trusted packet commits, not digest rewriting.
3. All packet and milestone proving commits are absent. Current working-tree behavior is
   progress evidence only.
4. The plan's new oracle names mostly map to source-text assertions. The strongest new
   runtime surfaces—domain analysis, ontology rule execution, typed control capture,
   ResultChecksumV2, and Stage-2b activation—lack behavior-distinguishing tests.
5. `tooling/model/schema_consumer.rs` remains a live `codefabric.id16` producer.
6. Stage-2b activation exists as an uncalled API and lacks the required atomicity/fault
   proof and accountable owner-acceptance execution.

## Recommended Resume Order

1. Repair the WP01/WP06 duplicate-field regression and rerun only
   `datafusion_55_serving_equivalence`, followed by WP01's promoted selector.
2. Replace or augment shallow plan-specific oracle nodes with Rust behavioral tests for WP06–WP17,
   starting with the domain analyzer, ontology rules, activation transaction,
   ResultChecksumV2, and typed capture.
3. Migrate the schema-consumer binary from `codefabric.id16` to generated per-domain
   metadata and rerun its focused compatibility check.
4. Execute the correctness portions of PR-1…PR-7, write target-only reports, and record
   reviewed branch decisions. Keep PR-7 performance measurement waived.
5. Re-run each dependency-closed packet's four substantive oracles and packet-local
   gates, then create and record proving commits in dependency order.
6. Reconcile the three changed declared inputs through governed planned-input evolution;
   rerun `just plan-status`, milestones, decommission checks, and final non-performance
   gates. Request the independent `implementation-review` only after those are green.

## Exact Next Action

Fix the duplicate unqualified `candidate_source_id` introduced in the serving plan used by
`fabric::serving::tests::datafusion_55_serving_equivalence`, add a focused assertion that
the projected schema has unique field names, and rerun:

```bash
direnv exec . cargo nextest run --locked --lib -E 'test(fabric::serving::tests::datafusion_55_serving_equivalence)' --no-tests=fail
```

Do not proceed to broad repository validation or performance work from this status
checkpoint.

## State Reconciliation Summary

The schema-v2 state now records WP01–WP17, M01–M04, and DB01–DB06 as `in_progress`;
preserves all prior deviations and failed approaches; keeps every proving commit null;
records the five discovered obligations above; leaves the overall status `executing` and
the current packet at WP01; and sets the exact next action to the serving-equivalence
repair. No plan or design artifact was edited.
