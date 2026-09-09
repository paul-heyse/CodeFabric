# CodeFabric status

Updated 2026-09-08. Canonical tree: `/home/paul/CodeFabric`, branch `master`.

## Current work

Execute the [non-production preparation plan](docs/plans/codefabric_pragmatic_delivery_nonproduction_preparation_plan_2026-09-08.md) directly. The [consolidated review](docs/reviews/codefabric_pragmatic_product_delivery_consolidated_review_2026-09-08.md) selects the target. Production changes are excluded from this task; do not resume the old packet chain.

Execution started at `7479af1`. Commit `84833ca` preserves the pre-existing process assessment and its two validator additions before retiring that machinery. Their review remains history; ordinary reviews will no longer require a validator artifact type.

## Demonstrated behavior and limitations

The four build domains, provider implementations, data-fabric infrastructure, and presentation shell exist. Full Python/Rust graph delivery is not complete. The retained closeout at `0cc7242` recorded a passing root check and root nextest results of 1,038 passed, 13 failed, two skipped. These are historical observations, not a new run.

The startup-dependent failures report `activation-control-provision`: `local store mutation requires its admitted native runtime`. Repair belongs to the production phase. The August cached compiler error is not the current diagnosis. Retained local log: `target/native-closeout-root-tests.log` (ignored, not a portable CI input).

## Preparation progress

Instructions/skills, selected target design and navigation, process-tooling retirement, Just/CI routing, development configuration, corpus/harness and production backlog have been prepared. Focused validation is in progress; no preparation completion claim yet.

The first tooling run found one obsolete composed-guard expectation after removing its historical dispatcher; its dead composition helper and tests were retired. Retained behavioral checks are unchanged. Current navigation/local-link check passes for 17 core documents.

## Production handoff

Follow the [production implementation plan](docs/plans/codefabric_pragmatic_production_implementation_plan.md). Its first task is coherent daemon startup and useful query/status/reopen under the revised resource contract. Remaining scope includes real semantic providers for both languages, all selected graph families and eight forms, live updates, truthful remainder, compact operation records, resource simplification, and sustained operation.
