# CodeFabric status

Updated 2026-09-08. Current work is in `/home/paul/CodeFabric` on `master`.

## Current handoff

**Non-production preparation is complete.** Follow the [production implementation plan](docs/plans/codefabric_pragmatic_production_implementation_plan.md), starting with outcome 1: restore real daemon startup/query/status/reopen at the activation-control native-runtime boundary. No further process migration or plan activation is required.

The [consolidated review](docs/reviews/codefabric_pragmatic_product_delivery_consolidated_review_2026-09-08.md) and [selected domain documents](docs/spec_index/README.md) define the revised target. All Python/Rust fact families and eight query forms remain scope. First-release delivery, complete-product delivery and preparation readiness are different claims.

## Completed preparation

[Preparation plan](docs/plans/codefabric_pragmatic_delivery_nonproduction_preparation_plan_2026-09-08.md), steps 1–8:

- Consolidated workflow skills and short canonical instructions; library references remain optional API navigation.
- Revised the eight selected domain documents, resource/artifact principles, alignment guidance, roadmap and indexes. Historical predecessors remain unchanged.
- Retired plan activation/state/artifact/oracle/packet/source-bundle machinery and its live callers; kept actual behavioral and build-boundary checks.
- Added proportionate Just commands, CI change routing, truthful hook/config reporting and licensing-free routine dependency policy.
- Prepared independent Python/Rust fixtures, edit sequences, bounded real-product runners, reusable semantic/differential helpers, scale-source generation and measurement tools.
- Created the complete production backlog, including the WP77–WP106 disposition table and detailed first fix.

Execution began at `7479af1`. Preservation commit `84833ca` saved the pre-existing assessment and its two validator additions before retiring that validator. Preparation implementation is commit `79c5d52`. All preparation edits used the canonical tree; existing historical worktrees were left untouched. No subagents or extra worktrees were created for preparation.

## Validation and limits

Checks on the preparation tree, 2026-09-08:

| Check | Result |
|---|---|
| `just tooling-test` | 184 passed; these test tooling and harnesses, not full CPG behavior |
| `just tooling-lint` | Format and lint pass for 30 tooling files |
| `just docs-check`, all changed Markdown local links and scoped spelling | Pass; core navigation checks 17 documents |
| Spec outline fixture tests and current-suite outline | Pass; selects the eight working masters without plan history |
| Shell syntax, environment-contract and tool-version checks | Pass; recipes isolate synthetic inherited contamination |
| `just doctor` | 13 ok, 0 warn, 0 fail; healthy cache/tooling preserved |
| Hook from a subdirectory, installed Codex configuration/rule checks | Pass; valid JSON, STATUS handoff, actual local source selections |
| CI configuration/routing and recipe references | Local checks pass; hosted CI has not been executed here |
| `just policy` | Advisories, bans and sources pass; licensing is outside the routine check |
| Scale-source generator | 10 generated modules per language, 23 source files; workload preparation, not a performance measurement |
| `just golden --case startup --timeout 240` | **Failed:** one real selected test reproduces the retained activation-control provisioning error |
| Production boundary and whitespace | No runtime/native/dependency/wire/runtime-contract changes; `git diff --check` passes |

The real startup test reports `activation-control-provision`: `local store mutation requires its admitted native runtime`. It fails before readiness; this is not skipped or converted to an expected pass. Local ignored observations are in `target/preparation-*.log`, `target/preparation-session-hook.json` and `target/product/golden.json`. They are development observations, not committed proof artifacts. The golden observation was taken at `84833ca` plus the recorded preparation diff; production inputs did not change.

The earlier closeout at `0cc7242` recorded a passing root check and 1,038 root tests passed, 13 failed, two skipped. Those are historical results; preparation did not rerun or claim the full product suite. The August cached baseline is stale.

## Remaining production work

Restore the startup owner, replace generalized runtime proof/resource machinery at its consumers, complete actual semantic contributions from Python and Rust, implement query-relevant processing remainder, live invalidation/publication and quiet convergence, then finish all analyses/forms and sustained bounded operation with safe retention and measured performance.

The mixed-language public-answer adapter, real convergence/rebuild callbacks, obsolete-completion controls and phase-specific runtime telemetry are tied to these production changes and explicitly scheduled in the production plan. Prepared fixtures and harness unit tests do not imply those behaviors work. Existing native patches remain selected until production consumers are replaced and useful fixes preserved.
