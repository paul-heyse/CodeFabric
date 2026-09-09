# CodeFabric: non-production preparation plan

Date: 2026-09-08. Planning baseline: `63e23cb` on `master`.

Source: [finalized pragmatic product-delivery review](../reviews/codefabric_pragmatic_product_delivery_consolidated_review_2026-09-08.md). Scope: the user's request to implement its recommendations **excluding production code changes**, so the repository is ready for production implementation under the new workflow, configuration, design, and plans.

This plan is derived directly from that review and the current repository. It does not use the `impl-plan` skill, its packet template, activation process, digest tables, or proving-commit requirements. It is an ordinary editable Markdown plan. Creating it does not execute it.

## Execution closeout — 2026-09-08

Steps 1–8 are complete under commit `79c5d52` and the accompanying status closeout. [STATUS](../../STATUS.md) records actual validation, the preserved baseline and remaining product failures. The [production plan](codefabric_pragmatic_production_implementation_plan.md) is ready for execution. Production code, dependencies, native patches, wire assets and runtime contracts were deliberately preserved. The real startup test still fails at activation-control native-runtime provisioning; preparation does not claim product readiness.

The sections below retain the preparation scope and rationale. Their proposed actions are now implemented, subject to the explicit runtime-dependent adapters and instrumentation assigned to production work.

## 1. Outcome and scope boundary

Execution ends with one coherent development environment: short current instructions and skills; an updated target design; proportionate commands and CI; usable test and benchmark infrastructure; retired process machinery; a truthful `STATUS.md`; and a concrete follow-on production implementation plan. A new session can start the first production change without another process migration or design-reconciliation cycle.

Preparation readiness is distinct from product readiness. The existing daemon startup failure and unimplemented CPG behavior remain production work. This plan must neither repair those by changing runtime code nor hide them by changing expected results, excluding failures, or claiming the future design already works.

| Included now | Deferred to production implementation |
|---|---|
| All repository skill entrypoints, shared policies, reference routing, and associated instruction text | Daemon, supervisor, provider, analysis, query, storage, resource, or adapter runtime behavior |
| `AGENTS.md`, `CLAUDE.md`, onboarding, repository specification, domain specifications, doctrine, roadmap, indexes, and current planning documents | Connecting Python/Rust providers, implementing queries/status responses, watcher/invalidation/publication changes |
| Development hooks, shell/bootstrap reporting, command definitions, CI routing, test profiles, development configuration, and permission-rule references | Changing deployed operational defaults, permissions, wire behavior, runtime schemas, or persisted graph formats |
| Non-production validators, runners, fixtures, tests of tooling, product test harnesses, and benchmark/report tooling | Removing runtime proof execution/history, changing allocation ownership, fixing activation-control startup, or adding runtime telemetry |
| Removing obsolete plan/artifact orchestration after detaching its consumers | Removing native patches, changing production dependency selections/features, or altering the selected public type universe |
| Preparing test scenarios and the complete production backlog | Claiming those future scenarios pass before the implementation exists |

Classify files by what they do, not just their directory. Some `scripts/` launch production processes; some `contracts/` files are runtime inputs; Cargo/uv manifests contain both development and production settings. Developer tooling can change here, but a change that alters the installed product's behavior belongs in the follow-on plan. Keep production dependency versions, feature selection, native sources, wire descriptors/generated clients, and runtime contract assets unchanged.

Test code and fixtures are in scope, including strictly test-only registration if needed. Prefer external test/tooling files and avoid unnecessary edits under production source directories. Do not rewrite colocated Rust tests merely to remove packet names; rename them when their behavior is next touched. Any test-only change beside production code must leave the production portion unchanged.

Use the canonical tree and small coherent commits. Preserve the pre-existing edits to `.claude/skills/_shared/artifact-schemas.md` and `tooling/ci/artifact_contracts.py`, and the other agent's untracked assessment. During execution, incorporate their useful intent or retire their now-obsolete mechanism explicitly; do not overwrite or discard them as unexplained noise.

## 2. Current coupling that determines the order

The following surfaces were inspected while creating this plan:

| Current coupling | Required preparation |
|---|---|
| `just governance` depends on artifact/status/oracle/dependency and historical zero-state checks; `ci-fast` includes governance | Replace the dependency chain and preserve meaningful checks before removing its modules |
| `authoritative_design_conformance.py` imports `artifact_contracts.py`, validates an approved historical relational plan, enforces synchronized master history, and expects every master filename in `AGENTS.md` | Separate lightweight design navigation from plan approval/history; update its tests and readers together |
| Bootstrap derives suite/pin information from the documentation, reports old baseline guidance, and describes a narrower UDS posture than the project configuration | Update reporting and design discovery together; report actual source selections and environment posture accurately |
| CI separately invokes released-wave/fixture and historical governance recipes | Updating `ci-fast` alone is insufficient; inspect direct workflow calls too |
| Plan validators share helpers with useful checks; `tracked-target-zero-state-check` is implemented inside `artifact_contracts.py` | Move useful small helpers/checks to their proper owner before deleting the old container |
| `gate_filter_census.py` compares live nextest selectors with a committed census | Retain empty-selection protection while removing unnecessary manifest synchronization |
| Performance tooling freezes broad source-path lists and consumes WP65-specific evidence | Preserve measurement/comparison tools; detach ordinary benchmarking from historical proving lineage |
| `.claude/settings.json` includes stale recipe allowances and blanket lockfile edit denial; skills are exposed through shared symlinks | Align development permissions and discovery with the new command surface without changing unrelated sandbox policy |

These are a bounded starting map, not a demand for a new repository census. At execution start, refresh relevant status, references, and recipe dependencies. Do not infer current product failure from the old August cached baseline: the retained closeout evidence identifies activation-control provisioning without the admitted native mutation runtime.

## 3. Execution sequence

| Step | Outcome | Depends on |
|---|---|---|
| 1 | A current handoff and explicit preparation/production boundary | — |
| 2 | Consolidated skills and short agent instructions | 1 |
| 3 | Updated target design and working navigation, detached from old plan authority | 1–2 |
| 4 | Retired process tooling and coherent command dependencies | 2–3 |
| 5 | Aligned development configuration and fresh-session behavior | 2–4 |
| 6 | Ready test/benchmark infrastructure and proportionate CI | 3–5 |
| 7 | Complete production implementation plan and final handoff | 3–6 |
| 8 | Preparation verification and closeout | 1–7 |

Keep tightly coupled reader/configuration changes in the same commit. The sequence is a default, not a new immutable dependency system: small changes can be combined where that avoids an intermediate broken command or instruction. Record relevant progress in `STATUS.md`, not a second state ledger.

## 4. Step 1 — Establish the handoff and preserved baseline

Create root `STATUS.md` with the selected review, this preparation plan, current demonstrated behavior, known product failures, next preparation step, and a place for the subsequent production plan link. Use revision/date/command references for evidence. Record the retained root results as historical: 1,038 passed, 13 failed, two skipped; do not rerun the whole product merely to copy this observation.

Record the actual execution-start commit and working-tree changes with ordinary Git status/diff. Distinguish existing edits from changes made under this plan. Preserve the four build domains, selected toolchains, dependency pins, native patches, generated wire assets, and actual runtime checks.

Add an explicit instruction that this preparation task does not resume WP79 or execute the old packet chain. Point ordinary work toward `STATUS.md` and this plan. Keep the old plan/state as historical input until its live readers are detached in steps 3–4; do not rewrite old packet states as completed or invoke `plan-activate` to retire it.

**Check:** The handoff identifies one next action, an attributable baseline, and the production work that remains. Git diff shows no product change or unrelated cleanup. No extra approval/state artifact is needed.

## 5. Step 2 — Consolidate all skills and agent instructions

### Workflow skills

Create one short `product-delivery` skill for ordinary planning, implementation, relevant validation, and status handoff. It follows the consolidated review's delivery loop and honors task-specific exclusions. Its planning mode produces an editable outcome backlog; it does not create an activation transaction, state JSON, digest registry, or mandatory review cycle.

| Existing skills | Action |
|---|---|
| `impl-plan`, `impl-plan-exec`, `impl-status` | Move useful guidance into `product-delivery`; retire their old entrypoints and mandatory process. Update live references. |
| `plan-audit`, `implementation-review` | Keep one concise optional review entrypoint under `implementation-review`, covering plans or code as requested. Remove compulsory procedural findings and oracle quotas. |
| `integrate-plan-audit` | Retire the separate workflow. Incorporate accepted corrections directly into the selected working document. |
| `design-development` | Keep concise optional guidance for consequential unresolved design choices; use short decisions rather than mandatory dossiers. |
| `library-capability-research`, `lib-leverage` | Consolidate targeted API/capability investigation under `library-capability-research`; remove automatic whole-library surveys and repeated scoring. |
| `skill-eval` | Retain only as an explicit opt-in evaluation task; update references to the new workflow. It is not part of this plan's acceptance. |

Remove obsolete templates, examples, helper references, and descriptions that would reload the retired process. If an old entrypoint must temporarily communicate its replacement to an already configured client, it may contain only a short redirect; remove it once discovery/reference updates are complete. Do not leave a second executable workflow behind an alias.

### Library reference skills and shared documents

Review every remaining `SKILL.md` and associated `REFERENCE.md`, including:

- `ast-grep-ripgrep-ref`, `canonicalization-lib-ref`, `code-facts-lib-ref`.
- `datafusion-pyarrow-rust-ref`, `deltalake-rust-ref`, `petgraph-ref`.
- `fastmcp-pydantic-ref`, `grpcio-orjson-protobuf-ref`, `rust-grpc-daemon-ref`.
- `gix-notify-ref`, plus the inactive `attrs-cattrs-ref` and `typer-rich-ref` notices.

Retain useful API navigation, provider isolation advice, version-sensitive cautions, and examples. Remove mandatory principle mapping, proof artifacts, source-edit bundles, and workflow escalation from ordinary API use. Do not invent missing reference documents or adopt inactive libraries. Do not change factual API guidance or pins to match an unimplemented future dependency selection.

Reduce `_shared/evidence-policy.md`, `validation-policy.md`, `doctrine-policy.md`, `artifact-schemas.md`, and `code-intelligence.md` to the useful guidance still consumed by the new entrypoints; merge/remove files where that makes the shared source shorter. Keep navigation material available on demand. Remove requirements for universal per-claim automation, four oracles, proving commits, packet test-body policing, typed review registration, historical digest freshness, and automatic invariant promotion. Keep truthful results, relevant checks, meaningful failure propagation, ordinary Git history, and evidence scoped to what it establishes.

### Canonical instructions

Shorten `AGENTS.md`; keep product/build boundaries, the command interface, proportionate validation, source navigation, preservation, and canonical-tree integration. Move infrastructure rationale to a referenced document under `docs/`; preserve its useful content without requiring every session to load it. Align `CLAUDE.md` and the repository specification's instruction, baseline, evidence, and assurance sections. Remove contradictory unconditional pre-edit CI requirements and fixed page/count targets.

**Check:** Read each live entrypoint through its references. All retained skills resolve to existing relevant documents; retired policies are not indirectly loaded. Confirm `.codex/skills` and `.agents/skills` still resolve to the single `.claude/skills` source. A manual walkthrough of a docs task, implementation task, and status request follows the requested scope without activating the old workflow. This is a bounded check, not a comparative skill-evaluation project.

## 6. Step 3 — Update the target design and its readers

Update the selected current domain documents in place with ordinary Git history and a clear revision note. Preserve immutable historical predecessors. Their filename/version history is not a claim that the running product implements the revised target. Do not copy all eight masters solely to change process rules, and do not regenerate runtime assets to make a document change appear implemented.

| Design surface | Required update |
|---|---|
| SUITE and RM | Select the pragmatic target and staged outcomes; remove plan/proving-history activation as development authority; retain full eventual scope and useful semantic governance |
| ONT | Preserve detailed fact families, identity, raw/normalized distinctions, uncertainty and precision. Align any proof-as-semantic-authority language with actual facts and coverage. |
| GEN §§85/88/93–96 and connected clauses | Separate installed support, per-workspace coverage, and release test confidence; keep effective Python/Rust context; move universal candidate semantic proof into proportionate tests |
| FAB §§9.4/13/14 and connected clauses | Keep Arrow/DataFusion/Delta, exact version selection, coherent publication/recovery, and runtime operation records; remove compulsory duplicate execution and reconstructible proof histories; state the reduced resource contract |
| LIFE §§6–8 and connected clauses | Two freshness lanes, conservative invalidation, obsolete-result rejection, scoped remainder, minimal publication ownership, and recovery; advanced invalidation/overlays are not prerequisites for first useful updates |
| QRY, especially §7 | Keep all eight forms and semantic distinctions; specify query-relevant processing/coverage/freshness/precision and historical selection without generalized independent-proof prerequisites |
| SRV | Keep Rust semantic ownership and FastMCP presentation; document status/remainder behavior as a target without altering released wire definitions in this phase |
| Planning contracts D-RT02/D-RT05/LD-RT09 | Incorporate the compact artifact policy and reduced native admission guarantee; preserve the distinction between target change and implemented capability |
| LD-RT08 and storage maintenance guidance | Preserve safe finite retention/native maintenance requirements; make concrete mechanisms depend on actual consumers and measurements |

The reduced resource target must explicitly cover shared DataFusion budgets, bounded application work/state/results, owned task lifetimes, provider containment, measured headroom/backpressure, and recovery. It must explicitly avoid claiming universal allocation pre-admission or immunity from OOM. Runtime actions still leave compact artifacts; source edits use normal Git and proportionate checks.

Revise `full_data_fabric_design_principles_v2.md`, particularly P9/P10/P16/P17/P19/P20/P23/P25/P26/P28/P30/P36, and both DataFusion/Arrow and Delta alignment manuals. Product invariants remain binding; implementation preferences become advisory; generalized proof obligations are removed rather than hidden beneath an override. Preserve technical reference content outside these mandatory workflow clauses.

Update `docs/spec_index/README.md`, `fact-domain-map.md`, `library-routing.md`, `wave-traceability.md`, `contract-census.md`, and `invariants-and-doctrine.md`. Preserve their navigation role. Update README/onboarding and the governing repository specification where they describe the old workflow or current target. Do not rewrite historical reviews and plans or move documents merely because they predate this plan.

### Decouple design navigation from execution certification

Change `tooling/ci/authoritative_design_conformance.py`, its tests, `scripts/spec-outline.sh`, relevant outline tests, and bootstrap's design discovery together as necessary. Keep a small current-document selection in the existing governance document; the index mirrors that selection and remains non-normative. Remove the active-plan import and approved-dossier requirement, synchronized successor issuance as an edit prerequisite, and the requirement to repeat every master filename in `AGENTS.md`.

Retain useful checks for unresolved current links, ambiguous current selection, malformed selected metadata where metadata is still used, and navigator output. They verify navigation, not semantic conformance or product completion. Historical links need remain readable; their contents need not pass current policy schemas.

Distinguish **target dependency/API baseline** from **currently resolved native source selection** in documentation and session output. The existing patched Cargo graph remains installed until production changes replace consumers. If any proposed documentation/config edit is found to feed runtime compilation or serving directly, preserve that executable input and put its migration in the production plan; do not change the product indirectly through data files.

**Check:** Current design links resolve, `just spec-outline` and focused library navigation still work, and relevant navigator/tooling tests pass without an active-plan pointer or proving history. Review the changed normative clauses against the preserved product contract and first-release definition. The documents accurately label implementation gaps.

## 7. Step 4 — Remove process tooling and repair command dependencies

Inspect both Just dependencies and direct callers in CI, scripts, skill references, tests, and docs. Delete an old helper only after remaining useful consumers have moved. Keep the change localized to non-production tooling.

| Surface | Action |
|---|---|
| `tooling/ci/plan_assurance.py`, `real_time_cpg_assurance.py`, `v7_certification.py`, and process-only tests | Retire packet/DAG/oracle/proving-commit/certification orchestration. Preserve actual behavioral scenarios independently of those dispatchers. |
| `artifact_contracts.py` and its tests | Remove plan/review taxonomy, activation/state, digest, and proving-lineage machinery. Move any necessary frontmatter/link parsing and useful tracked-output hygiene to their ordinary tooling owner before deletion. |
| `tooling/ci/native_dependency_artifacts.py`, its tests, `tooling/native-dependencies/` process manifests and recipes | Remove per-edit source bundling/replay enforcement after checking live consumers. Preserve native source origins and active Cargo selections. Runtime-consumed inputs, if any, remain for production migration. |
| `scripts/gate_filter_census.py`, its tests, and committed selector census | Retain direct protection against empty test selection using existing runner failure behavior. Remove the need to synchronize a second manifest of Just commands; keep only a small direct check if still useful. |
| Historical model/legacy/seed checks and performance certification wrappers | Retain concrete build/architecture protections and useful measurements; remove demands to replay historical plan closure or exact source-file evidence freezes. Classify individual assertions before retiring the wrapper. |
| `contracts/governance/plan-overlap-dispositions.yaml`, plan state, and `active-plan.json` | Retire live development authority once readers are gone. Preserve historical material through Git or clearly historical files; do not create successor state JSON. |

Remove or replace `artifacts-check`, `plan-status`, `plan-activate`, `plan-dependency-check`, `oracle-substance-check`, `packet-oracle-check`, real-time packet/milestone/decommission/certification recipes, and other obsolete plan-specific aggregate recipes. Preserve useful direct feature/provider/query tests under behavior-oriented commands. Removing a dispatcher does not remove its tests from ordinary discovery.

Rebuild `governance`, `ci-fast`, and `ci-pr` dependencies around current build, compatibility, structural, and behavioral checks. Retain stable-graph, feature-architecture, Protobuf/interoperability, tool-version consistency, and meaningful structural boundaries. Reassess historical zero-state checks individually: keep a real boundary check, retire archaeology whose only purpose was closing an old packet.

Do not turn the retained production failures into skips or expected successes. Obsolete assertions of retired *development policy* can be removed; assertions about currently implemented production ownership, schemas, or lifecycle remain until their runtime replacement is implemented and exercised.

**Check:** Just parses and lists a coherent command surface; retained tooling imports/tests pass; no executable caller selects retired plan machinery. Search live instructions, tooling, commands, and CI for retired entrypoints and inspect remaining matches. Historical prose may retain historical names. Exercise empty-selector and failing-child behavior in affected runners; do not build a new certification system to check this removal.

## 8. Step 5 — Align development configuration and session behavior

Update the relevant settings and their reference documentation together:

| Surface | Required result |
|---|---|
| `scripts/bootstrap.sh`, `.codex/hooks/session-start.sh`, `.claude/settings.json` hook | A short, nonmutating session report names the current preparation/production handoff, actual environment, and dated check results. Startup does not run CI, activate a plan, install dependencies, or mutate state. |
| `.codex/config.toml`, `.codex/rules/codefabric.rules`, `.claude/settings.json`, `.codex/README.md`, `CLAUDE.md` | Commands and skill discovery match the new workflow. Remove stale allowlist names and blanket development lockfile-edit denials that contradict authorized dependency maintenance; preserve unrelated user-selected approval/sandbox settings. |
| `scripts/repo-shell.sh`, `.envrc`, shell/router helpers | Supported recipes continue to work in fresh non-interactive shells without activation or direnv. Preserve explicit Python-domain selection, inherited-environment cleanup, and secret handling. Correct obsolete documentation references. |
| `.cargo/config.toml`, `bacon.toml`, `.vscode/settings.json`, `.config/nextest.toml` | Preserve working toolchain/cache/target isolation and one continuous Cargo checker. Keep expensive tests/profiling out of the edit loop. Adjust test grouping/timeouts for actual workloads rather than historical packet labels. |
| `scripts/tool-version-contract-check.sh`, `tooling/rust-tool-versions.env`, related environment checks | Keep existing pinned tooling consistent with changed recipe/CI structure. No rolling upgrades or unrelated cache/linker/toolchain redesign. |
| `deny.toml`, dependency-policy recipes, and their instruction/CI callers | Remove licensing approval/audit prerequisites from routine development; retain proportionate compatibility, source/version, duplicate-family, and relevant advisory checks. Do not turn ordinary crate additions into a separate dependency-review project. |

Do not read or print `.envrc.local` or capability tokens. Use synthetic contamination values when exercising environment cleanup. Project-level changes must not edit global agent settings or restart/reconfigure a healthy user service merely to align prose.

Keep sccache and the established incremental/check workflow; the review did not identify them as a reason for a new build architecture. Preserve stable-root/sidecar target sharing and nightly isolation. A stale cached compiler probe warrants only targeted diagnosis if encountered, not routine `cargo clean`.

Replace bootstrap's blanket baseline instruction with “use relevant attributable evidence; run affected checks.” Historical logs can remain available with their revision/date and scope. Distinguish healthy tools, passing preparation checks, and failing product behavior. Do not label a target design pin as the currently resolved upstream source when Cargo selects a local patch.

**Check:** Run `bash -n` for changed shell entrypoints, existing environment-contract and tool-version checks, and the hook under representative startup/subdirectory conditions. Parse its actual output and verify it contains no automatic gate/activation instruction or false current-pass claim. Run `just doctor` and a representative fresh-shell tooling command. Use the installed clients' local configuration/discovery support where available; a simple file parser alone does not demonstrate that a client recognizes a changed key. Do not change client schemas speculatively.

## 9. Step 6 — Prepare tests, measurement, and CI

### A small test command surface

Use the existing adapter dev environment for Python tooling tests; do not create a root Python project. Keep one stable-root Rust integration-test target. Select a few clear commands and update their callers in the same change:

| Command | Status and purpose |
|---|---|
| `just tooling-lint` | Proposed replacement name for `governance-tooling-lint`; covers the retained/new Python tooling rather than plan governance |
| `just tooling-test *args` | Proposed focused tooling/harness tests using existing dev dependencies; no production certification implication |
| `just docs-check *args` | Proposed lightweight spelling/local-link/navigation checks for current changed documents; no review taxonomy, digest registry, or all-history conformance |
| `just golden *args` | Proposed real daemon/MCP product corpus runner with explicit case selection, useful failures, and nonempty selection |
| `just root-check-fast`, `root-test-incremental`, domain gates, `governance-scan`, `stable-graph-check`, `proto-check`, `ci-fast` | Existing commands retained or simplified where needed; their actual coverage remains explicit |

These are implementation targets, not commands claimed to exist today. Reuse an existing equivalent helper where it makes the change smaller; avoid maintaining duplicate commands with different meanings.

### Product corpus and differential harness

Prepare compact Python/Rust fixture programs and independently justified expected facts/answer fragments under the existing test fixture hierarchy. Reuse `tests/fixtures/`, `tooling/fastmcp4_modern_client_driver.py`, `tests/integration/`, and adapter test helpers where appropriate. Keep the harness outside the runtime packages and use the current public service/transport boundary.

The initial fixture set should support first-release entity/source, references/imports/calls, available types, and processing-status questions. Include semantic ambiguity and missing inputs. Prepare scripted edit/delete/rename/context-change sequences and a reusable clean/incremental comparator; extend it later with randomized cases as runtime capability lands. Include scenarios for obsolete completions, compilation failure/repair, cancellation, and restart where the public surface can exercise them.

Do not force future internal schemas into the current wire contract. Where the current API cannot express or observe a target behavior, record the precise scenario and required API/instrumentation work in the production plan. Implement the runner and comparisons that can be prepared now; leave runtime hooks and service additions to production work.

Compare semantic identity relationships, source positions, meaningful ordering, and coverage. Normalize only incidental values that are not under test. Never normalize a missing provider into an empty complete result. Keep fixture expectations independent of the runtime output used to investigate them. Snapshot acceptance remains an explicit reviewed operation using existing mechanics, without a new approval service.

The product command must attempt the real product. With the current startup blocker it may fail, and that failure is expected to remain visible. Harness unit tests may use controlled child processes/responses to test timeout, cleanup, comparison, selection, and error propagation; those results are labeled harness tests. Do not use fake daemon output, skipped target cases, or `xfail` as a product pass. An empty selection, missing prerequisite, or all-unavailable product run must not report success.

Retain useful existing production tests and their assertions. Remove tests devoted solely to retired plan policy with that policy. Avoid mass renaming, broadly increasing timeouts, or modifying expected native behavior to get green results without runtime changes.

### Measurement preparation

Update `tooling/benchmarks/semantic_profile_bench.py`, `fastmcp4_release_benchmark.py`, and their reporting/validation helpers as needed. Preserve workload execution and useful comparisons; remove WP65/proving-lineage/source-freeze prerequisites from ordinary measurement. Keep records of workload, machine, revision, settings, and samples.

Prepare repeatable mixed-language workloads for startup/reopen, query latency, edit-to-syntax, semantic convergence, backlog, memory, disk, and recovery. Measure end-to-end time including detection/debounce where observable. Missing runtime telemetry remains a named production task; do not add telemetry to the daemon in this phase or invent measurements. Proposed 100 ms/500 ms timings are hypotheses, not acceptance thresholds. A smoke test validates the benchmark runner, not product performance.

### CI routing and integration

Update `.github/workflows/ci.yml`, Just aggregates, test profiles, and their tooling checks together. Use change routing based on actual consumers:

- Documentation/skill-only changes run relevant documentation/configuration checks.
- Tooling and harness changes run their tests and affected consumer checks.
- Shared shell, Cargo/feature, wire, or test-environment changes select the affected build domains; do not accidentally exclude them as “just tooling.”
- Production changes in the later phase retain domain and product checks appropriate to their behavior. Scheduled/manual integration remains available.

Remove direct historical wave/plan-certification invocations as well as indirect Just dependencies. Preserve the substantive compatibility and boundary cases those aggregates covered. Keep supported platform and toolchain coverage rather than changing platforms to avoid failures. Keep caches as acceleration, mutating commands explicit, and secrets confined.

Do not make every ordinary docs edit run the full product. Equally, do not redefine `ci-fast` as tooling-only or suppress its existing product failures. Publish preparation/tooling and product outcomes distinctly. If branch-required status names depend on external hosting settings, retain compatible check names in the workflow and document the external follow-up; do not claim to have changed remote settings from this local plan.

**Check:** Tooling/harness tests pass, including failure propagation and cleanup. Validate CI routing against representative docs-only, tooling-only, shared-environment, and production path sets. Run relevant retained boundary/feature checks when their execution configuration changed. Exercise `just golden` enough to verify real startup/transport invocation and record its actual product failure or success. Do not require future product scenarios to pass in order to finish preparation.

## 10. Step 7 — Produce the production plan and final documentation

Create `docs/plans/codefabric_pragmatic_production_implementation_plan.md` as an editable outcome backlog, using the new documentation and workflow directly. It is a deliverable of executing this preparation plan, not another prerequisite to approving this plan. Do not resume the old packet chain or generate successor plan-state JSON.

The production plan must include affected runtime surfaces, true dependencies, useful existing tests/fixtures, expected user-visible behavior, material transition risks, and validation appropriate to each outcome. Detail the first implementation slice; retain later work at capability level. Carry existing work forward rather than scheduling a clean-room rewrite.

Required production backlog, in delivery order:

1. Restore real startup/query/status/reopen under a coherent resource boundary, beginning with the retained activation-control/native-runtime failure. Preserve temporary existing admitted wiring only where it enables a small correct transition.
2. Replace mandatory double execution, generalized expectation/fault/proof histories, and proof-language dependencies with actual validation and compact runtime operation records.
3. Implement the reduced resource operating contract, replace unnecessary consumers of patched APIs, retain useful native fixes, then reduce patches and reconcile production locks/type compatibility.
4. Connect effective Python and Rust providers to Arrow/Delta and the first-release entity, fact, relationship, and source queries, with real semantic contributions from both languages.
5. Implement queryable scoped status, coverage, freshness, precision, causes, exclusions, historical selection, and relevant processing remainder.
6. Deliver watcher/coalescing, conservative context invalidation, two freshness lanes, obsolete-result rejection, compatible incremental publication, and quiet convergence.
7. Complete the remaining Python/Rust/common analyses and all eight query forms, including composition and partial-success semantics.
8. Deliver sustained-operation requirements: bounded retained work/state, useful telemetry, recovery/cancellation, safe finite retention/reclamation, and measured performance. Add precise dependency tracking, overlays, CDF consumers, or advanced maintenance only where the need justifies the mechanism.
9. Remove obsolete runtime modules, fallback paths, and tests of superseded behavior with their exercised replacements. Extend independent fixtures and lifecycle/differential scenarios as each behavior lands.

Preserve all selected ONT/GEN families. Reconcile WP77–WP106 obligations into the new outcomes: carry semantic and operational requirements forward, replace proof/process-only requirements, and state triggers for conditional optimization work. Use a concise table in the production plan; no new traceability compiler or mandatory oracle count. Do not drop remaining families behind an indefinite “partial” label.

Update the implementation roadmap to point at the new sequence. `STATUS.md` links the production plan, records completed preparation and remaining product limitations, and identifies its first actionable task. README/onboarding should explain how to start development, run relevant checks, inspect the target design, and interpret product failures under the new workflow. Document fixture, benchmark, and expected-output maintenance beside their existing tooling.

**Check:** Starting from `AGENTS.md`, an executor can find the current design, the production plan, the first task, relevant commands, and known failures without opening a historical packet report. Every review recommendation has either been completed here as preparation or has an explicit production disposition. Creating a plan or editing a decision no longer requires another policy migration.

## 11. Step 8 — Verify readiness and close preparation

Run the smallest combined set justified by the actual non-production diff. Reuse successful earlier checks if no relevant inputs changed. Expected verification includes:

- Documentation spelling/local links, current design navigation, and reference resolution.
- Just parsing/listing, retained tooling lint/tests, and the new harness's own tests.
- Changed shell syntax, fresh-shell environment checks, hook/configuration behavior, and skill discovery.
- Changed CI routing plus any build/feature/wire checks affected by configuration changes.
- A real product-harness invocation with its actual outcome reported; known production failures remain named.
- Git diff review against the recorded execution-start state to confirm no production behavior, dependency selection, wire/runtime asset, or native-source change has entered the preparation work. Attribute any concurrent edits separately.

Existing recipes available for relevant checks include `doctor`, `environment-contract-check`, `tool-version-contract-check`, `governance-scan`, `stable-graph-check`, `proto-check`, and domain checks. `environment-regression` currently includes product tests; it is not a cheap substitute for focused preparation checks. Use the simplified/new tooling commands only after they have been implemented. Do not run old artifact/plan certification to certify its own removal.

Preparation is complete when all of these are true:

1. One current handoff, one target design, and the new workflow are discoverable from fresh-session entrypoints.
2. All retained skill and instruction documents agree on scope, proportionate validation, ordinary plan evolution, runtime artifacts, and canonical-tree integration.
3. Current design documents express the full product target and selected resource/assurance reductions; their readers do not require old plan activation or synchronized reissuance.
4. Retired process modules/recipes have no live consumers; useful test, compatibility, and build checks remain callable.
5. Development environment and CI changes are exercised, with no speculative global toolchain/sandbox changes.
6. Product and measurement harnesses are usable and honest about missing runtime behavior; their preparation checks pass.
7. The production plan covers all deferred recommendations, preserves full family/query scope, and begins with a concrete startup/product task.
8. All changes under this plan are integrated and committed in the canonical tree; the final handoff states exactly which checks passed and which product failures remain.

Do not require a green future product to declare the environment ready for its implementation. Do not describe preparation as completion of the product. Correct newly introduced non-production failures here; a necessary runtime fix stays in the production plan and remains visible.

## 12. Coverage of the consolidated review

| Review recommendations | Completed by this plan | Deferred production counterpart |
|---|---|---|
| §§2–4: full target, first release, honest remainder | Updated domain/serving docs, status semantics, independent fixture expectations, complete backlog | Provider/query/status implementation |
| §5: simpler analyses, updates, publication, storage | Revised architecture/roadmap and prepared incremental/recovery scenarios | Runtime simplification, scheduling, publication, maintenance |
| §6: validation, runtime artifacts, software assurance | Doctrine/policy/spec corrections and removal of development proof machinery | Runtime duplicate execution/proof-history removal and operation records |
| §7: resource contract and native dependency reduction | Explicit target/limits, routine dependency policy, source-selection documentation, removal of edit-artifact tooling | Allocation/admission consumers, native patch removal, production locks |
| §8: product corpus, differential/lifecycle checks, proportionate validation | Fixtures, harnesses, tooling tests, runner semantics, relevant CI | New runtime hooks/observability and passing future product cases |
| §9: skills, instructions, handoff, integration | Steps 1–2 and 5, including all skill/reference documents and session consumers | Continued use during implementation |
| §10: supporting-code removal | Non-production validators, dispatchers, obsolete policy tests, history selection | Runtime modules and obsolete runtime assertions removed with replacements |
| §11: coordinated authority changes | Domain/repository/doctrine/index changes and their tooling readers | Actual product adoption of the revised contract |
| §§12–14: ordered delivery, measurement, complete execution scope | Prepared benchmarks, readiness criteria, complete production plan and roadmap | Full product delivery and demonstrated performance |

The only excluded implementation category is production change. Non-production work needed to prepare or verify a deferred behavior remains in scope, while a truthful failing product scenario is not an excuse to weaken its expectation.

## 13. Handling discoveries without restarting the process

Adapt file groupings or helper choices in place when new references are found. Preserve the outcome and scope. A validator that mixes useful checks with retired policy should be split, not preserved wholesale or deleted blindly. A new test or helper needs only enough structure to support its concrete consumer.

If a supposed tooling edit changes product behavior, keep that portion unchanged and add the precise production task; finish independent preparation. If that prevents a preparation check from passing, report the dependency rather than faking a pass. Reopen the selected design only for a substantive unresolved product decision, not because an old artifact validator rejects the new workflow.

Historical names and references may remain in clearly historical documents. The final live-reference check targets instructions, executable tooling, selected navigation, CI, and current plans. It does not require erasing repository history.

## Planning validation

This plan was based on read-only inspection of the consolidated review, current command/CI wiring, skill inventory, design navigation/validation, bootstrap/hooks, development settings, and representative test/benchmark helpers. No preparation changes or product tests were executed while authoring it. The plan document is checked for spelling, local-link validity, and whitespace; its future checks are not represented as completed work.
