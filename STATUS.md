# CodeFabric status

Updated 2026-09-09. Current work is in `/home/paul/CodeFabric` on `master`.

## Current handoff

**Production implementation is active. Outcomes 1–3 are implemented for the current Linux workflow; outcome 4 is in progress and outcomes 5–8 remain open.** Follow the [production implementation plan](docs/plans/codefabric_pragmatic_production_implementation_plan.md). The four existing golden cases pass. Pyrefly now honors selected Python version/platform settings through its real sidecar protocol; daemon publication still needs semantic-provider integration. These checks do not establish mixed-language or full-product completion.

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

Catalog sealing now uses a single schema/identity/dependency validation pass against the published observations. The iterative self-comparison and iteration policy are removed; row/byte limits remain. This also rejects a catalog changed after publication rather than accepting two matching later scans. The Rust check, 30 affected catalog/publication/reopen tests and all four golden scenarios pass on 2026-09-09. Existing observation histories remain for exact reopen and explanation; their retirement requires replacing those consumers. This slice is committed as `55fdd7b`.

Outcome 2 is complete. The unused activation proof adapters, nine-history reader/writer, generalized expectation/fault/proof engine and proof-qualified provider-capability API are removed. Ordinary provider coverage/remainder reports and exact activation/reconciliation remain. Transformation installation now checks plans, schemas, dependencies, volatility and ordering without executing result sets. Actual relation reads enforce rows across partitions and observed memory/spill limits, including stream completion; authorized child views retain those limits and physical-plan reset clears counters. No intermediate result collection/checksum or repeated execution establishes admission.

Validation on 2026-09-09: 69 affected provider/admission/reconciliation/catalog/child-view/producer tests and all four golden daemon scenarios pass. The isolated `data-fabric` compile and `just feature-architecture-check data-fabric` pass. Its existing native dependency expectations were aligned with `arrow-json` and `buoyant_kernel`; no dependency was changed or downgraded. All 20 feature-architecture tooling tests pass. Structural rule fixtures pass (30), but `just governance-scan` still reports two unchanged direct source reads at `source_image.rs:1805` and `pyrefly_service.rs:1241`. These remain open for source/provider boundary work; the full suite is not claimed green.

The resource and source-read work described below supersedes those earlier findings. Next is actual Python/Rust semantic-provider integration, scoped progress and live updates. Retained catalog histories are recovery/explanation inputs; reducing redundant history writes belongs to outcome 8. All full-product fact families and query forms remain required.

The [consolidated review](docs/reviews/codefabric_pragmatic_product_delivery_consolidated_review_2026-09-08.md) and [selected domain documents](docs/spec_index/README.md) define the revised target. All Python/Rust fact families and eight query forms remain scope. First-release delivery, complete-product delivery and preparation readiness are different claims.

## Outcome 3 — reduced resources implemented for the current Linux runtime

On 2026-09-09, production execution stopped constructing generalized native allocation
owners/policies. Native worker/thread/job envelopes, cancellation, joined runtime cleanup,
owned-store mutation leases and shared DataFusion memory/spill pools remain. The receipt
forks and their dedicated allocator harnesses are removed from the tree; Cargo selects the
same upstream Arrow/Parquet 59.2.0, Tokio 1.53.1, Buoyant kernel/engine/derive versions and
exact delta-rs revision. No dependency was downgraded. Git retains the previous sources.

Pinned provider blobs now use one bounded, no-follow regular-file read and the caller's
expected content digest. Symlinks in any component, oversized files and FIFOs are rejected.
Both previously reported direct source-read findings are fixed.

Validation: stable library compile, `just stable-graph-check`, `just governance-scan`
and all four `just golden --timeout 360` cases pass against upstream dependencies. The
focused resource/provider/maintenance run passed 64 cases; its real Pyrefly shutdown case
then passed with the freshly verified sidecar binary supplied (65 affected cases total).
All 28 selected feature-architecture/change-routing tooling tests pass. Native checkpoint,
vacuum protection, exact reopen, cancellation, writer reconciliation and control headroom
are among the passing cases. The cleanup is committed as `200b306`; all four affected
compatibility integration tests also pass.

Whole-process RSS sampling now gates source capture, native data operations and scheduled
query admission/checkpoints. Initial workstation thresholds pause new data work at 3 GiB
and resume at 2.5 GiB; control/cancellation/cleanup remain available. Current and peak-sampled
RSS are separate from DataFusion reservations and spill, and observation failure is explicit.
Nineteen RSS/scheduler/executor tests, the isolated data-fabric feature check and all four real
golden cases pass with this behavior. RSS is sampled on Linux; other platforms report it
unavailable, not zero. Sampling cannot guarantee immunity from OOM. Provider containment,
shared pools, work/result bounds and physical disk headroom remain in place. Full mixed-language
provider wiring and sustained retention/performance are outcomes 4–8, still open.

## Outcome 4 — selected Pyrefly context preparation

Pyrefly 1.2.0's Query ignored configured runtime selection by retaining default system
information. A narrow local dependency fix now creates handles with the selected file
configuration's system information. The sidecar keeps the same pinned sibling dependencies;
its handshake identifies the configured-context implementation so an old binary is rejected.
A null bundle selection uses the pinned embedded bundles; an explicit mismatching bundle is
rejected. External dependency/stub roots and project configuration preparation remain open.

The deployed context path now accepts supported contexts. Source blobs are read once with
bounded size, no-follow components, regular-file and exact digest checks before checker-state
mutation. The same bytes drive checker input and result positions. Source substitution,
symlinks and FIFOs cannot silently change a selected input.

Validation on 2026-09-09: all 29 sidecar tests and `just sidecar-check` (compile and strict
Clippy) pass. All 10 root Pyrefly service tests pass with the rebuilt sidecar binary. The
real-process test now analyzes a version-dependent call through UDS and validates the Arrow
answer `module.current` for Python 3.14 before joined shutdown. Sidecar tests independently
cover 3.13/3.14 and Linux/Darwin selection, deletion/recreation and rejected source mutation.
This demonstrates the provider boundary, not daemon catalog publication or containment.
The Linux sandbox follow-up below replaces its previously unimplemented policy path. Outcomes 4–8 remain open.

## Linux provider containment

The production launcher now consumes an application-compiled, sealed seccomp descriptor.
The Linux-only `seccompiler` 0.5.0 dependency compiles architecture-aware filters; existing
versions are unchanged. The filter permits Unix sockets and normal threads while rejecting
network socket families, namespace/process-group escape and privileged kernel operations.
Bubblewrap supplies the private filesystem/PID/network view, and delegated cgroups retain
whole-process-tree memory/CPU/process accounting and cleanup. A missing host prerequisite
continues to produce an unavailable containment result.

The live Linux probe uses the same Bubblewrap arguments and descriptor-closing shell as
production. All 12 sandbox tests and all 39 affected sandbox/Pyrefly/rustc service tests pass
on 2026-09-09. A real confined worker starts threads and a descendant; kernel usage is
observed and the complete tree is killed and joined. Strict source-read governance passes.
The root library Clippy command completes with the existing broad warning backlog; two
new missing-error-section warnings were corrected. This is containment infrastructure for
provider integration, not completed daemon semantics.

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

Complete actual semantic contributions from Python and Rust, implement query-relevant processing remainder, live invalidation/publication and quiet convergence, then finish all analyses/forms and sustained bounded operation with safe retention and measured performance.

The mixed-language public-answer adapter, real convergence/rebuild callbacks, obsolete-completion controls and phase-specific runtime telemetry are tied to these production changes and explicitly scheduled in the production plan. Prepared fixtures and harness unit tests do not imply those behaviors work. Native receipt forks have been removed; the new Pyrefly context fix remains a selected local dependency.
