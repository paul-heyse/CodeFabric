# CodeFabric status

Updated 2026-09-09. Current work is in `/home/paul/CodeFabric` on `master`.

## Current handoff

**Production implementation is active; outcome 1 is complete and outcome 2 is in progress.** Follow the [production implementation plan](docs/plans/codefabric_pragmatic_production_implementation_plan.md). All four existing golden cases pass: startup, Python serving, persisted reopen and cancellation. All 33 affected store/executor and lost-acknowledgement recovery tests pass. New activation uses compact published-candidate validation instead of nine proof histories. Outcomes 2–8 remain open; these tests do not establish mixed-language semantic completeness.

Current production work, 2026-09-08–09:

- Initial control-table creation runs with the workspace write lease and joined cleanup; exact-version readback reconstructs the serving reader after the native runtime joins.
- The production executor retains bounded work, deadlines, store ownership and cleanup without enabling the generalized native allocation receipt policy. The first attempt with that policy panicked on a roughly 2 GiB schema-decode estimate for the small activation schema.
- Executor commit `3a41bd0` passes its focused real-store test: an error after writing preserves durable data, joined cleanup releases the lease, unowned writes remain rejected, and a subsequent owned write succeeds. Candidate publication now uses the same executor; only identifiers/version records cross the joined runtime boundary, and serving readers reopen those exact versions.
- Commit `41c30dc` owns candidate publication and reconstructs exact readers after join. The final activation append/readback now uses the same bounded control lane.
- Completed head/error/range/list reads release their pending-operation entries. Previously these entries survived until host-runtime shutdown and prevented a joined writer lease from releasing. Cancelled unfinished native IO still retains its entry through runtime join.
- `just root-check-fast` passes for the startup/resource changes. `just golden --timeout 360` passes all four real cases on 2026-09-09. Focused store/executor regressions and lost-acknowledgement recovery pass: 33 tests, 1,021 unrelated tests filtered out. Generalized runtime proof machinery, provider completeness and live-update gaps remain production work.
- uv was correctly upgraded on the host. Commit `fe5615f` aligns the environment manifest and all three CI setup sites with 0.12.11. Both tool-version checks pass; refreshed session context reports 13 ok, no warnings or failures.

Outcome 1 is committed as `a940930`. Outcome 2's first slice (`b4f03f1`) removes the activation proof evaluator and its nine newly written histories. The existing activation record retains exact input/provider/source/table references and an opaque candidate identity in the compatible `proof_receipt` field. Validation rejects substituted workspace, source generation and table versions. All four golden cases still pass, including the assertion that no proof directory is created; five focused candidate/factory tests pass. Historical proof data is preserved but no longer required by new activations.

The frozen unimplemented-analysis gate is removed (`e674454`). Actual typed census/composition checks still require producer or explicit-remainder coverage; a family no longer has to retain a fixed unavailable status to be admitted. The Rust check and three real producer/census/remainder tests pass.

Proof-program construction is removed from the release model in both production and tests. Its definitions, fault/expectation compiler and legacy fixture validator are deleted. Useful source examples and target fact assertions are preserved in `tests/fixtures/pragmatic_cpg/analysis_cases.json`; actual source-to-syntax/remainder tests consume the examples, while their unfinished semantic assertions remain explicitly pending. All four golden cases, 17 affected release/provider tests and `just feature-architecture-check release-compiler` pass. Producer-result validation now returns the existing execution directly instead of copying its rows into a proof wrapper. Its binding, coverage, cancellation and resource checks remain; the Rust check and 14 affected tests pass. This slice is committed as `1cf9c72`.

Catalog sealing now uses a single schema/identity/dependency validation pass against the published observations. The iterative self-comparison and iteration policy are removed; row/byte limits remain. This also rejects a catalog changed after publication rather than accepting two matching later scans. The Rust check, 30 affected catalog/publication/reopen tests and all four golden scenarios pass on 2026-09-09. Existing observation histories remain for exact reopen and explanation; their retirement requires replacing those consumers. Next: continue consumer-first proof/resource simplification, then connect actual semantic provider runs and live updates. Outcomes 2–8 remain open.

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

Replace generalized runtime proof/resource machinery at its consumers, complete actual semantic contributions from Python and Rust, implement query-relevant processing remainder, live invalidation/publication and quiet convergence, then finish all analyses/forms and sustained bounded operation with safe retention and measured performance.

The mixed-language public-answer adapter, real convergence/rebuild callbacks, obsolete-completion controls and phase-specific runtime telemetry are tied to these production changes and explicitly scheduled in the production plan. Prepared fixtures and harness unit tests do not imply those behaviors work. Existing native patches remain selected until production consumers are replaced and useful fixes preserved.
