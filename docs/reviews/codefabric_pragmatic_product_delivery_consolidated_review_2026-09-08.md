# CodeFabric: finalized review for pragmatic product delivery

Date: 2026-09-08. Status: **final consolidated recommendations for execution planning**. Repository baseline: `b981cc6` on `master`; implementation observations below refer to `0cc7242` unless stated otherwise.

This document is the single recommendation set for the forthcoming execution plan. It incorporates the [original assessment](codefabric_pragmatic_product_delivery_review_2026-09-08.md), the [other agent's assessment](process_assessment_pragmatic_cpg_delivery_2026-09-08_v1.md), and the decisions in the [reconciliation review](pragmatic_cpg_delivery_review_reconciliation_2026-09-08.md). Those documents remain supporting history; readers do not need to reconcile their competing proposals before planning from this review.

The review covers the product architecture, runtime artifacts and assurance, resource requirements, skills and agent instructions, implementation workflow, supporting-code removal, and delivery of the full Python/Rust CPG. It defines a useful first release as an intermediate outcome. Producing this review does not itself modify the active design, plan, policies, or implementation.

## 1. Final assessment

**Keep the Rust daemon and Arrow/DataFusion/Delta architecture. Simplify the runtime's assurance machinery, replace the compulsory planning and certification cycle with a short delivery workflow, and organize implementation around useful graph queries over real Python and Rust code.**

Progress has been impeded by several reinforcing mechanisms. Broad proof requirements became production state and execution dependencies. Every design discovery could reopen a large plan and its validators. Strong native allocation guarantees expanded the dependency patch surface. Provider and query delivery waited behind storage, resource, and assurance work that was not inherently necessary for their first correct implementation. Long-lived independent workstreams and delayed integration compounded these problems.

The remedy requires changes to both the software and the instructions that shape it. Editing skills alone leaves mandatory runtime proof machinery intact; deleting runtime checks alone leaves the process that regenerates that machinery intact. Conversely, preserving every existing abstraction would retain the maintenance burden, while removing the entire fabric would incur another architecture migration.

The selected approach is incremental simplification of the actual production path. Keep mechanisms that establish correct facts, coherent publication, truthful incompleteness, and owned operation lifetimes. Remove generalized proof systems, duplicate execution, plan-shaped test requirements, and abstractions that have no necessary product consumer.

### Evidence and its limits

| Observation from the source assessments | Implication for this review |
|---|---|
| Plan v3 has 30 packets with four proposed new oracles each, enforced by packet tooling | Tests must be selected for behavior and risk, without a fixed number or required test-body shape |
| Shared policy promotes checkable claims and repeated invariants into permanent checks | Add a consequence, recurrence, and maintenance-cost threshold; types and reasoned review remain legitimate evidence |
| Library navigators and alignment manuals impose additional doctrine and artifact workflows | Separate API guidance from compulsory design-process requirements |
| Nonvolatile transformations are executed a second time during candidate construction | Move determinism and optimizer comparisons out of ordinary execution |
| Startup provisions observation/proof histories; an activation helper constructs a detected-fault record rather than deriving it from an executed fault injection in that helper | Retain actual runtime validation and operation outcomes; move independent semantic assurance into tests and diagnostics |
| The root selects 15 patched native packages from 12 source directories | Reduce the strength and cost of the allocation contract deliberately, then reduce its patch surface |
| The retained root run reports 1,038 passing tests, 13 failures, and two skipped cases; failures identify activation-control provisioning outside the required native mutation runtime | Restore the real startup path early; local test success does not establish a usable product |
| Large tooling/document inventories, frequent plan succession, and packet-named tests | Reduce the active maintenance surface and procedural coupling; these counts do not quantify wasted effort |

Relevant implementation sources include [transformation execution](../../src/fabric/programmatic_schema.rs), [production startup](../../src/fabric/production_workspace_startup.rs), [activation proof](../../src/fabric/proof.rs), [Cargo selection](../../Cargo.toml), and the [retained root test log](../../target/native-closeout-root-tests.log). These are attributed observations, not a new test run.

The old cached August 31 `E0631` report is not the current repair target. The retained closeout root check passed; startup-dependent tests failed for the native mutation-runtime boundary. Similarly, vendored upstream lines are not agent-authored lines, and source churn alone does not establish why code was deleted. The recommendation rests on concrete dependencies and failure mechanisms, not a claimed percentage of unnecessary code.

## 2. Product contract to preserve

The target remains a full, inspectable code property graph for Python and Rust, updated as quickly as practical and served with precise disclosure of incomplete processing. “Full” means implementation of the selected fact and analysis families, with explicit static-analysis limits. It does not mean predicting arbitrary dynamic execution exactly. Missing implementation must remain distinguishable from inherent uncertainty or a failed workspace job.

Preserve these requirements:

- **Rust owns semantics.** One workspace daemon owns source capture, provider scheduling, graph state, query execution, publication, and capability status. The supervisor and launcher own process lifecycle. FastMCP remains presentation only.
- **Keep the four build domains.** The stable root, dated-nightly rustc extractor, pinned Pyrefly sidecar, and Python adapter continue to isolate their toolchains and dependencies. Do not add crates merely to organize source files.
- **Keep the selected data fabric.** Arrow carries provider and relational data; DataFusion executes relational transformations and queries; Delta stores durable versioned relations. Do not create a second graph database or replace the production target with a memory-only prototype.
- **Emit facts and mechanically derived facts.** Do not introduce evaluative judgments such as refactoring safety or risk verdicts. Preserve the existing exclusions for git history, execution coverage, and environment inventory as graph domains. Operational telemetry about the daemon remains necessary and is a different concern.
- **Preserve semantic distinctions.** Raw and normalized kinds coexist. Syntax occurrences differ from semantic entities. Call sites remain first-class facts. Canonical identity is application-owned rather than copied from provider-local IDs.
- **Use effective provider context.** Python roots, dependencies, stubs, settings, and language/platform configuration must affect analysis. Rust target, features, cfg, toolchain, dependencies, and compiler inputs must affect extraction. A recorded digest alone does not make configuration effective.
- **Keep provider isolation and authority rules.** Application-owned adapters contain unstable provider types. Conflicts retain their evidence and use the defined authority policy; unresolved conflict remains unknown or multi-candidate.
- **Keep coherent present-state queries.** Each query selects immutable compatible facts, coverage, and source/context references. Invalidated semantic facts cannot silently remain current. Missing output cannot imply absence.
- **Keep operational ownership.** Publication has one owner. Cancellation, resource release, source disclosure, and provider containment remain real behavior. A failed compile cannot revive stale compiler facts.

The eventual scope includes syntax, declarations, bindings, references, types, calls, control/data flow, and the selected Python object/memory/effect/resource/async and Rust MIR-derived ownership/flow/effect/state analyses, together with common derived families and all eight query forms. The current ontology and generation specifications retain the detailed semantic inventory; the execution plan must carry forward that inventory while changing its assurance and delivery dependencies.

## 3. Define a first useful release

The first release must be useful through the existing daemon and MCP service, include real semantic contributions from both Python and Rust, update without restarting, and reopen persisted graph state. It is not the completion boundary for the full target.

| User question or operation | Existing form or surface | Required behavior |
|---|---|---|
| Find declarations/entities | `FindEntities` | Return actual provider-derived entities; distinguish declaration enumeration from resolving a use to a definition |
| Find references, imports/dependents, callers, and callees | `FollowRelationships` | Distinguish syntactic occurrences and call sites from resolved semantic relationships; label possible and unresolved targets |
| Retrieve types and other supported facts | `RetrieveFacts` | Use effective Pyrefly/rustc analysis where required; disclose unavailable or unimplemented families |
| Show source around an entity or occurrence | `RetrieveSourceContext` | Return source from the selected source image with correct locations and disclosure rules |
| Inspect workspace/file processing | Existing status surface and queryable status relations | Explain current, pending, failed, excluded, and unsupported scope and its effect on results |
| Edit, add, rename, or delete source | Same running daemon | Invalidate affected facts, expose available fresh results, reject obsolete completions, and converge after quiet |
| Restart | Existing daemon lifecycle | Reopen a coherent persisted selection, reconcile current source/context state, and expose pending refresh |

Keep existing validation/reference tools and released wire contracts unless a concrete change requires versioning. Reducing the number of tools is not an independent objective.

`FindPaths`, `MatchPattern`, `CombineResults`, `SummarizeFacts`, and remaining graph families stay visible as unfinished delivery scope. Returning an explicit unsupported result is correct first-release behavior; it is not implementation completion. Syntax-only milestones are useful progress, but cannot satisfy claims about semantic references or resolved calls.

The early demonstration is a small mixed-language repository: start one daemon, inspect real facts and status, edit Python and Rust, observe pending scopes, retrieve changed results, delete a definition and remove obsolete relationships, introduce and repair a compile error, observe convergence, and restart into the selected persisted state. Extend this demonstration as families are added rather than building a comprehensive demonstration framework first.

## 4. Make incomplete processing part of the product

Maintain queryable processing records and derive the public status view from the same records used to decide which facts may be served. Do not add an independent status authority that can drift from publication.

Every response identifies the captured source/context revision and selected graph snapshot. Expose status at the smallest scope that can be stated honestly: language, context, file/module/owner, and fact family.

| Dimension | Meaning and examples |
|---|---|
| Processing | queued, running, completed, failed, cancelled |
| Coverage | complete for the requested scope, partial, indeterminate, unsupported; include exclusions |
| Freshness | current for the selected revision, superseded, awaiting processing |
| Semantic precision | exact observation, conservative possible set, heuristic, unresolved |
| Cause and next action | compile error, missing dependency, resource limit, implementation missing, retry scheduled |

A per-file view with syntax and semantic generations is useful, but generations alone do not capture these distinctions. For example, syntax may be current while type/call resolution for the entire Python context is running. A Rust target may have failed while independent syntax results remain available.

Query completeness must reflect the requested behavior. A workspace count of pending files is insufficient for a references query if those files could contain additional references. Return the relevant incomplete scope and offer paginated detail. If invalidation has not yet established the affected set, report a conservative larger scope and that uncertainty. Do not manufacture a precise file list or completion percentage from missing telemetry.

Exclude invalidated semantics from current-source results. An explicitly selected historical snapshot may remain queryable with its own revision and coverage. New pending-work observations can describe later disk changes without changing the meaning of an already pinned graph snapshot. The request must distinguish the snapshot being answered from any newer state being observed.

Keep three independent concepts: installed implementation support, actual per-snapshot processing coverage, and confidence established by release tests. Passing CI does not complete a workspace job. Completing a workspace job does not require rerunning the release corpus before publishing its facts.

## 5. Simplify the production architecture

The target production path is:

```text
source changes -> capture and effective context -> provider jobs
  -> owned Arrow facts -> Rust analyses and DataFusion transformations
  -> changed relations plus coverage -> exact Delta publication
  -> pinned DataFusion query -> facts, provenance, and processing remainder
```

### 5.1 Keep responsibilities concrete

The scheduler coalesces and prioritizes work. Adapters translate provider observations. Analysis functions compute semantics. DataFusion performs relational algebra. The publisher validates and selects committed state. The query service returns results with coverage and freshness.

Prefer ordinary typed Rust enums, functions, builders, and immutable configuration for these responsibilities. Keep schema and algorithm identities where they support compatibility, invalidation, or reproduction. Do not require every behavior to become a general-purpose compiled release/proof program, registry, or self-describing execution language.

Use DataFusion for filtering, projection, normalization, joins, replacement, aggregation, and query composition. Use Rust and petgraph for control-flow construction and graph algorithms where they are clearer or more efficient, consuming and emitting the same Arrow-based graph representation. Preserve native projection/filter pushdown and optimizer capabilities when their semantics fit. Library utilization is a means to a better implementation, not a conformance score.

### 5.2 Deliver two update lanes with conservative invalidation

Introduce a fast syntax lane using Tree-sitter/Ruff and a scheduled semantic lane using Pyrefly/rustc. Coalesce changes, reuse valid provider state where supported, and discard results from obsolete source/context generations. Provider failure and cancellation must propagate into coverage and dependent facts.

Start with file, module, or compiler-context recomputation where that is the smallest trustworthy unit. Semantic extraction is not generally a pure function of one file: imports, dependency metadata, macro expansion, configuration, and other source files affect it. A context identity must account for the actual changing inputs. Rust's dependency-based incremental design illustrates why per-file bytes alone are insufficient. [Rust compiler development guide](https://rustc-dev-guide.rust-lang.org/queries/incremental-compilation-in-detail.html)

Keep full-context recomputation as a correctness reference. Unknown dependencies widen invalidation. Add more precise tracking, including negative lookups and interprocedural dependencies, when it materially improves the workload; do not require every advanced analysis before a working watcher and conservative update loop.

### 5.3 Keep a small coherent publication boundary

Retain one immutable snapshot descriptor selecting compatible source/context references, facts, coverage, and exact relation versions. An immutable catalog pointer is a useful implementation technique only when the referenced state is also immutable. Replacing an `Arc` does not make mutable tables or independently updated relations coherent.

Commit relation changes, then select the exact committed version vector through the single publication owner. Queries must not silently read each table's latest version independently. Delta transactions are table-level; the application still needs to select a coherent multi-relation graph. [Delta Lake FAQ](https://docs.delta.io/delta-faq/)

Keep enough publication state for exact reopen and recovery from uncertain outcomes. Remove the requirement for a generalized proof product around every selected snapshot. Reuse unchanged table versions, stable table roots, and practical batching; avoid rewriting all relations or creating a new hierarchy for each source change.

### 5.4 Defer advanced storage mechanisms until their consumer exists

Use correct replacement at a practical owner/partition/context granularity first. Add durable overlays when measured publication latency justifies their read, compaction, and recovery complexity. Add CDF for an actual catch-up consumer; the local update already knows which work it performed. Ordinary query optimization must not wait for unrelated retention or maintenance packets.

Preserve native Delta transaction, log, and maintenance authority. Do not implement a custom file-deletion planner to work around a library limitation. Safe finite retention and reclamation remain required for sustained use. An initial interactive slice can use conservative retention with a disk cap and backpressure; it must not claim unattended operation is complete on that basis.

## 6. Separate runtime validation, runtime artifacts, and software assurance

These concerns need different execution schedules and costs.

| Concern | Ordinary runtime | Tests, audits, or explicit diagnostics |
|---|---|---|
| Boundary validity | Check IDs, schema, input/context binding, provider outcomes, and generation compatibility | Malformed inputs, failed providers, mismatches |
| Graph integrity | Validate changed partitions, affected references, replacements, and coverage | Broader snapshot integrity scans and semantic fixtures |
| Calculation semantics | Execute the reviewed algorithm once and state precision | Independent expected facts, focused properties, provider comparisons |
| Determinism/optimizer equivalence | Maintain valid inputs, identities, and cache dependencies | Repeated runs, input reordering, optimized/unoptimized comparisons |
| Operation history | Record actual operations and outcomes | Capture detailed plans and intermediates when investigating |
| Publication/lifecycle | Single owner, coherent selection, bounded task lifetimes, recovery state | Crash, uncertain commit, cancellation, delayed completion, and restart cases |

Remove mandatory second execution of nonvolatile transformations from normal candidate construction. Remove production generation of supposed independent expectation/reviewer/fault evidence when it does not represent an independently executed verification. Keep the validations that actually run and record their real outcomes.

Catalog metadata reconstructible from installed typed schemas and plans need not always be durably written, reopened, compared, and proved. Persist what is needed for semantics, invalidation, publication, recovery, and supported reproduction. Compute optional descriptive metadata on demand where practical.

### Runtime actions still leave artifacts

A capture, provider run, analysis batch, publication, or query leaves a compact structured result/event containing an operation ID, parent operation when applicable, source/context and implementation references, scope, outcome, gaps/diagnostics, and output references. Reuse Delta commit metadata and a compact operation history rather than inventing another event platform.

A query response is a result artifact; its retained operation record can be bounded. Detailed logical/physical plans and intermediate batches are captured on demand or for selected failures. Preserve enough retained inputs and versions to reproduce an investigation within the documented retention window; state when they have expired. An interrupted operation must remain interrupted or unknown until recovery establishes its outcome.

This requirement applies to actions of the running system. Ordinary Git history, code review, dependency pins, and proportionate checks govern edits to source code. Do not require per-edit content-addressed bundles, patch replay, pre-Cargo artifact verification, or synchronized copies of the design suite. Nor does every internal function call or allocation require its own durable artifact.

## 7. Adopt a smaller explicit resource contract

Replace the requirement to reject before essentially every relevant native allocation, with original backing receipts throughout the dependency stack, with an application operating contract comprising:

- One shared DataFusion memory/spill budget across updates and queries, without multiplying budgets per epoch.
- Bounded provider concurrency, native submissions, queues, input/value sizes, IPC batches, result buffers, caches, and retained snapshots.
- Explicit ownership and release of application buffers, query results, tasks, and retained generations, including state still held by active queries.
- Provider process containment, cooperative cancellation, and joined cleanup where work can be joined. A timeout response does not establish that uninterruptible work has stopped.
- Measured daemon/provider RSS and disk use, operating headroom, backpressure, and overload behavior.
- Host/process containment where supported as a backstop, with restart and uncertain-outcome recovery.

This deliberately does **not** guarantee admission before every library allocation or immunity from OOM. A sampled RSS watchdog reacts after growth. A catalog count does not bound catalog bytes. A query pool does not cover every provider or retained-input allocation. Compressed expansion may require decoder limits or isolation beyond simple input caps. Stronger guarantees need specific implementation work and cannot be claimed from this profile.

Use the library's supported resource and tuning mechanisms first. DataFusion exposes workload-dependent concurrency and batch settings; they support measurement and tuning, not a proof of process-wide allocator coverage. [DataFusion configuration](https://datafusion.apache.org/user-guide/configs.html)

### Reduce the native fork after replacing its consumers

Freeze expansion for the superseded allocation guarantee. Retain changes that address demonstrated semantic, ownership, cancellation, or recovery defects. Replace first-party dependencies on unnecessary allocation instrumentation in small compiling changes, then remove the patches that have no remaining required consumer.

Restore intended upstream identities deliberately and reconcile all affected locks. Current path-selected entries do not already identify registry/git sources merely because versions match. Check the resolved Arrow/DataFusion/Delta type universe, affected builds, relevant compatibility cases, and the real application consumer together. Removing the patch table and running only a graph validator is insufficient.

Amend D-RT05/LD-RT09 and their production consumers coherently. The startup repair must establish a valid operation boundary under the revised contract; simply disabling the owned-store guard would leave contradictory ownership behavior. A temporary use of existing admitted-operation wiring is acceptable if it restores the integrated path while consumers are simplified, without requiring completion of the entire strict allocation program first.

Routine new crates need proportionate API, version, feature, and compatibility checks. Do not add a licensing review or source-artifact approval program. Downgrades, dependency conflicts, and changes to the selected public type universe warrant specific investigation. Normal source origins and lockfile changes remain reviewable in Git.

## 8. Use smaller, stronger software assurance

### 8.1 Build a small independent product corpus

Provide one obvious product command, such as `just golden`, backed by the real daemon and MCP route. Reuse existing runners and fixtures where useful. Start with compact Python/Rust programs whose expected facts can be understood independently; add a few real repository smoke cases and use CodeFabric itself for dogfooding. Do not require golden answers for an entire large repository before the first feature can land.

Exercise both direct graph facts and useful query answers. Relevant examples include branches, loops, exceptions, imports and re-exports, rebinding, dynamic calls, traits, moves, borrows, drops, configuration differences, and broken compilation. Add cases with delivered behavior and discovered defects, without a fixed quota per family or change.

Store requests and expected facts or answer fragments in a simple reviewable format. JSON Lines is suitable where it is convenient; direct assertions and existing snapshots are equally valid. Normalize incidental temporary paths or opaque operation IDs only when they are not under test. Preserve contract-defined order, identity relationships, source locations, coverage, and error semantics. Avoid full-response snapshots that create maintenance churn from unrelated telemetry.

Expected results must be justified independently of the implementation producing them. An acceptance command can propose an expected-output update; reviewing that diff does not automatically make it an independent semantic oracle. Reuse existing snapshot acceptance mechanics where helpful. Do not introduce another approval workflow merely to maintain the corpus.

### 8.2 Reuse one differential harness across meaningful cases

Compare incremental state with clean recomputation after scripted and randomized edit sequences, using the same effective source/context inputs and comparison semantics. Include removals, renames, changed configuration, newly resolvable imports, delayed provider results, and selected cancellation/failure boundaries. Observe relevant intermediate responses as well as convergence after quiet.

Clean and incremental paths can share the same bug, so retain independent expected facts. A final converged snapshot also cannot show every stale response served earlier, unsafe cancellation, or failed restart. Keep targeted lifecycle tests where those risks exist.

One reusable harness can replace repeated scaffolding across many tests. One test case cannot establish correctness for all histories, and its existence is not grounds for deleting distinct failure scenarios.

### 8.3 Use reasoning and local tests proportionately

Types and short algorithm/protocol arguments can establish useful properties. Examples include owner-scoped anti-join/union replacement, immutable snapshot selection, and convergence under stated finite-lattice/monotonicity assumptions. Record the assumptions that matter, then test their implementation and boundaries. Monotonicity within one analysis run does not solve invalidation across deletions.

Correct final output does not imply every intermediate stage is correct: projection can hide an incorrect field, deduplication can hide duplicates, and errors can compensate. Some intermediate facts are themselves public CPG results and deserve product expectations. Conversely, an invariant already enforced by construction does not need a duplicate runtime proof table.

Replace “every checkable claim must become a check” with automation chosen by consequence, recurrence, detection cost, and maintenance cost. Two equal executions do not establish universal determinism. Fuzzing, fault injection, mutation testing, and deeper concurrency tools remain available for boundaries where they provide useful evidence; they are not prerequisites for every routine change. Independent harnesses and fault testing are valuable without reproducing an entire database engine's assurance investment. [SQLite testing practice](https://www.sqlite.org/testing.html)

### 8.4 Select validation by the claim being made

| Change or claim | Normal validation |
|---|---|
| Documentation or comments | Relevant spelling, links, formatting, and whitespace |
| Local implementation/refactor | Affected build/check and existing relevant tests; add a regression for meaningful changed behavior |
| New graph family/query behavior | Independent expected facts/answers, provider-to-query integration, missing-input behavior |
| Invalidation | Clean/incremental comparisons including removal and newly visible dependencies |
| Publication, storage, or recovery | Exact reopen, uncertain outcomes, relevant crash/cancellation/retention boundaries |
| Shared native dependency or feature | Affected source/feature compatibility, one public type universe, relevant builds and real consumers |
| Performance strategy | Representative before/after workload, profiles where useful, correctness checks for the changed strategy |
| Integrated/release readiness | Relevant integrated suites, mixed-language product scenario, selected failure/recovery cases, and supported workload measurements |

Keep `just ci-fast` as an understandable integration command after retiring its obsolete process dependencies. Keep the product command focused on observable product behavior. Neither command replaces all evidence for all claims. Use affected checks while editing; run the integrated set at integration/release boundaries when relevant. Do not require full CI before every edit or to finish a documentation task.

Retain nonempty test selection and honest failure propagation. Record command, tested revision, relevant configuration/environment, date, result, and limitations. Store historical reports normally; reassess their applicability when relevant code or inputs change. Do not call stale evidence current, a testless selector a pass, or nextest alone the complete Rust test result when doctests matter.

Fix integration blockers early. A known failure can be reported without blocking unrelated reversible work, but it cannot be hidden behind a distant cleanup packet or a blanket completion claim.

## 9. Replace the compulsory development workflow

The ordinary workflow becomes:

**Choose one observable outcome → inspect the immediate path → implement and integrate a small change → run relevant checks → exercise the product behavior → fix concrete defects → update the handoff and continue.**

Design and independent review remain available for consequential changes. They are not mandatory stations after every edit or every previous review. Small batches and frequent integration reduce the cost of coordinating independently evolving work. [DORA: small batches](https://dora.dev/capabilities/working-in-small-batches/), [DORA: trunk-based development](https://dora.dev/capabilities/trunk-based-development/)

### 9.1 One living handoff and one evolving plan

Use root `STATUS.md` for:

- Working behavior demonstrated through the product.
- Known failures and material limitations, with attributable evidence.
- The next usable slice and its immediate blockers.
- Remaining target capabilities and a link to the current execution backlog.
- Significant decisions or risks that the next session must understand.

The forthcoming execution plan should be a concise, Git-tracked, editable backlog of outcomes. `STATUS.md` points to it and summarizes present behavior; it does not duplicate every task or maintain a second completion ledger. Keep brief decision notes for changed semantics, durable formats, trust boundaries, or material performance tradeoffs. Ordinary plan corrections happen in place, with Git preserving history.

Retire the independent authority of plan-state JSON, active-plan selection transactions, declared-input digest tables, proving-commit chains, milestone/decommission machinery, and mandatory status reconstruction. Version released wire/storage contracts when compatibility requires it; do not require a new version of the entire plan/design suite for an ordinary implementation correction.

A living document can become stale. Update it when relevant behavior changes and expose its evidence date. Do not solve that problem by recreating a general-purpose status validator or maintaining multiple synchronized status artifacts.

### 9.2 Consolidate skill entrypoints

One short routine execution workflow should own the delivery loop, including lightweight planning and handoff updates. Existing skill names may briefly redirect to it for discoverability; they must not load the retired process behind the redirect.

| Current surface | Final disposition |
|---|---|
| `impl-plan`, `impl-plan-exec`, `impl-status` | Consolidate routine planning, execution, and handoff guidance. A plan or status request can produce its requested document without activating another state machine. |
| `plan-audit`, `implementation-review` | Consolidate concise, optional independent review guidance around behavior and material risk. Missing ceremonial fields are not automatic findings. |
| `integrate-plan-audit` | Retire as a separate compulsory workflow; apply accepted corrections directly to the selected working document. |
| `design-development` | Retain optional, concise design guidance for unsettled consequential decisions. No full design dossier or fresh alternative survey for an ordinary correction. |
| `library-capability-research`, `lib-leverage` | Consolidate targeted library investigation. Consult exact local sources and probe concrete uncertainty; do not require a full capability survey before routine use. |
| `skill-eval` | Keep outside the delivery chain and invoke only for an explicit workflow-evaluation task. It is not a prerequisite for simplifying these instructions. |
| Library reference navigators | Retain useful API navigation and examples; remove automatic doctrine-mapping, artifact, and stop-condition workflows. |
| Shared policies | Reduce common guidance to truthful claims, proportionate checks, preservation/integration, and relevant navigation. Remove repeated mandatory proof and artifact taxonomies. |

This is consolidation of the workflow itself, not merely making ten large workflows optional while leaving all their enforcement active. No fixed page count, tenfold reduction target, or recipe count becomes a new acceptance gate.

### 9.3 Reduce always-loaded instructions

Keep `AGENTS.md` focused on the product/build boundaries, selected command interface, risk-based validation, preservation and integration rules, source navigation, and material search/environment traps. Move the tooling inventory, sccache topology, assurance rationale, and other reference material into linked documentation. Preserve working build/toolchain conventions while shortening their entrypoint explanation.

Align `CLAUDE.md`, skill discovery, and session/bootstrap context with the same guidance. Remove stale active-plan claims and contradictory baseline rules from all live instruction sources. API references should be consulted when needed, not loaded wholesale before ordinary work. Ordinary reviews should not require adding a new artifact category to a validator.

### 9.4 Integrate frequently in the canonical tree

Continue in the single canonical working tree. Use small coherent commits and preserve pre-existing work. If agents are used within the applicable session rules, give them bounded tasks with clear file ownership and one integration owner; do not allow independent long-lived interface designs to accumulate in separate worktrees.

Integrate and exercise changes as they become usable. Do not hold working implementation until a separate certification or documentation phase. Existing authorization should not become repeated approval requests for ordinary implementation choices. Git history preserves the changes; it does not replace meaningful validation of their behavior.

## 10. Remove supporting code as replacements land

Remove unnecessary machinery promptly once the replacement works and its consumers no longer need it. Preserve behavior, not historical module or packet identities. Avoid both broad deletion by filename prefix and permanently compiled obsolete fallback paths.

| Surface | Required disposition |
|---|---|
| `tooling/ci/native_dependency_artifacts.py`, associated tests, manifest consumers, and recipes | Retire the rejected source-bundle/replay prerequisite; retain ordinary source/version selection checks. |
| `plan_assurance.py`, `real_time_cpg_assurance.py`, plan/artifact portions of `artifact_contracts.py`, and plan-specific certification layers | Remove Markdown-to-proof orchestration, fixed oracle/test-body enforcement, and activation/state machinery. Move any useful non-plan validation to its ordinary owner before deleting the old container. |
| Packet dispatchers and packet-named tests | Remove dispatchers. Retain useful scenarios and helpers; rename tests for behavior as their modules change. Do not start a separate repository-wide rename campaign. |
| `fabric/programmatic_schema.rs` | Remove compulsory duplicate execution. Preserve typed schema/plan enforcement and useful tests. |
| `fabric/proof.rs`, `fabric/proof/delta_history.rs`, and startup proof scheduling | Replace generalized expectations/fault/proof history with actual candidate validation and compact outcomes. Move real fault campaigns to tests. |
| `fabric/programmatic_observation_delta.rs` | Remove mandatory persistence of reconstructible descriptive observations unless an actual recovery, provenance, or invalidation consumer needs them. |
| `semantic_release.rs` and associated release-compiler machinery | Keep necessary immutable configuration, schema/algorithm identity, and typed program selection. Remove production test-expectation and generalized proof-language dependencies. |
| Native resource adapters and `third_party/native` | Replace consumers of the superseded guarantee, retain demonstrated fixes, then remove unnecessary patches and reconcile locks. |
| Old plans, states, review pointers, instruction references, and recipes | Stop selecting runtime gates or workflows from historical artifacts. Preserve history and useful navigation; archive by supersession, not by date. |

Retain build-domain isolation, generated Protobuf interoperability/drift checks, stable-graph and feature-architecture validation, useful structural rules, provider adapters, application-owned IDs, source capture, query ownership, exact version selection, cancellation, and recovery. These protect real behavior. They should not be deleted merely because they sit beside excessive assurance code.

A Git commit or tag preserves source bytes, not the future cost of reintegration. Remove an obsolete module in the same change as its exercised replacement when practical. Check affected references, configuration, generators, tests, and recipes with ordinary searches and builds; do not create another exhaustive decommission-proof product.

## 11. Change the operative design and instructions together

Classify doctrine into three categories:

1. **Product invariants remain binding:** factual semantics, explicit uncertainty, effective context, identity, authority, isolation, coherent snapshots, and owned lifetimes.
2. **Implementation preferences become advisory:** staticness heuristics, library capability mappings, generalized self-description, and conformance scores. Use them when they simplify the product.
3. **Superseded obligations are removed explicitly:** per-clause executable proof, compulsory repeated execution, independent semantic proof on every candidate, mandatory historical storage of reconstructible metadata, and allocation-by-allocation native admission as the default contract.

Editing only a skill would leave contradictory domain requirements in force. The execution plan must coordinate these surfaces:

| Authority surface | Required change |
|---|---|
| [AGENTS.md](../../AGENTS.md), [CLAUDE.md](../../CLAUDE.md), [repository specification](../rust_core_python_interface_repository_specification_2026-08-20.md), and session/bootstrap guidance | One current instruction source, short entrypoints, proportional validation, ordinary plan evolution, and frequent integration |
| [Shared skill policies](../../.claude/skills/_shared/) and workflow entrypoints | Consolidation described in §9, including removal of artifact/activation enforcement consumers |
| [Data-fabric principles v2](../library_ref/full_data_fabric_design_principles_v2.md), particularly P9/P10/P16/P17/P19/P20/P23/P25/P26/P28/P30/P36 | Preserve useful intent, remove universal proof/artifact interpretations, and make implementation preferences advisory |
| [DataFusion/Arrow alignment manual](../library_ref/datafusion55_arrow59_design_principle_alignment_manual_2026-08-24.md) and [Delta alignment manual](../library_ref/deltalake_1.0.0_43a0cf10_design_principle_alignment_manual_2026-08-26.md) | Remove mandatory multi-step mapping, report outputs, and stop conditions from ordinary API use |
| FAB §9.4 “Durability classification and exact reconstruction,” §13 “Resources, observability, and proof,” and §14 | Reduce proof-history/double-execution obligations while retaining durable graph state, coherent reopen, and meaningful reconstruction checks |
| LIFE §§6–8, including “Update pipeline and analysis lanes” and “Validation and candidate construction” | Publish available compatible facts with honest remainder, simplify candidate validation, preserve publication ownership and recovery |
| GEN §§85/88 and §§93–96 | Separate installed support, runtime coverage, and release assurance; select tests by material risk rather than universal clause coverage |
| Connected SUITE/QRY requirements, including QRY §7 “Evidence, unknowns, absence, and provenance” | Retain semantic provenance and completeness; remove generalized independent-proof prerequisites from ordinary serving |
| Planning contracts D-RT02/D-RT05 and LD-RT09, active plan/state selectors, and gate dispatchers | Adopt the reduced proof/resource contract and outcome-based delivery order; retire inherited process authority |
| LD-RT08 and maintenance/reclamation consumers | Reassess against actual storage and retention needs; resource-contract simplification does not itself implement safe maintenance |
| `docs/spec_index/` and design navigation | Point to one current target and historical predecessors without treating indexes as normative or stale masters as coequal targets |

The domain tags above refer to the current [design corpus](../authoritative_design/), as navigated by [the specification index](../spec_index/README.md). Preserve the detailed ONT/GEN semantic target while correcting its assurance and delivery rules. Change released external contracts only when actual compatibility behavior changes. A development-policy correction does not require synchronized copies of all eight master documents.

Make the initial policy correction bounded: enough to remove contradictory execution obligations and select the new delivery contract. Do not rewrite the entire documentation corpus before restoring the product. Runtime changes follow in coherent increments under that contract; the documentation must describe any real transition rather than prematurely claim the old mechanism is gone.

## 12. Delivery order and preservation of remaining scope

The following order guides the execution plan; it is not a calendar estimate or a replacement packet taxonomy.

| Stage | Observable outcome | Relevant existing scope to reuse or split |
|---|---|---|
| Correct policy and contract | One selected recommendation set, short workflow, living handoff, reduced proof/resource obligations, and no contradictory compulsory gate chain | Shared policies, agent instructions, domain clauses, D-RT02/D-RT05/LD-RT09, process tooling |
| Restore integrated startup | Real daemon starts, reports status, answers a useful request, and coherently reopens persisted state | Immediate WP79 application integration; existing activation, storage, serving, and cancellation work |
| Serve useful facts from both languages | Effective Python/Rust provider output reaches Arrow/Delta and the first-release entity/fact/relationship/source queries | WP77–WP83 as needed for truthful capability/context/provider boundaries; useful parts of WP98/WP100 |
| Update in one running process | Source/context edits invalidate correctly; fresh syntax and pending semantics are visible; stale jobs cannot overwrite newer state; quiet results converge | WP88/WP90 and the necessary portions of WP89/WP91; start with conservative recomputation |
| Complete the selected CPG and query surface | Remaining Python/Rust/common families, all eight query forms, and composition with explicit precision and partial-success behavior | WP84–WP87 and remaining WP98–WP100; graph execution work from WP99 as required |
| Make sustained workloads fast and reliable | Measured latency/convergence improvements, bounded backlog/resources, safe finite retention, cancellation, recovery, and operational visibility | WP92–WP97 and WP99/WP103–WP104 where their actual consumers or measurements justify them; basic safety accompanies earlier stages |

Apply installed product testing from the first useful behavior rather than waiting for WP102. Simplify WP101-style proof scheduling where it obstructs ordinary execution. Remove obsolete code as replacements land instead of waiting for WP105; residual quality work still needs closure. Replace WP106-style replay of the packet history with release readiness over the demonstrated product.

Separate true semantic dependencies from shared-file scheduling constraints. Python and Rust work do not inherently need serial delivery because they touch the same integration file. Provider installation need not wait for every native allocation patch; useful query optimization need not wait for all maintenance work. Conversely, available syntax does not remove the need for correct semantic invalidation or effective compiler context.

Keep the complete family/query inventory in the execution backlog. Classify advanced optimization work by actual need and trigger rather than deleting it silently. Finite retention, bounded operation, and correctness are mandatory before claiming sustained operation; a particular overlay, CDF consumer, or optimization is conditional on the need it serves.

## 13. Measure product progress and know when to stop adding support work

Track working graph families and query behaviors on real repositories, correctness of coverage/remainder, edit-to-syntax latency, semantic convergence after quiet, query latency, backlog age, startup/reopen time, memory, disk, and recovery behavior. Include both languages and failure cases.

Measure end-to-end latency from observed source changes, including detection and debounce, as well as internal processing time. Retain workload, machine, revision, relevant settings, and samples for comparisons. Treat 100 ms syntax refresh, a 500 ms debounce, or a particular startup time as hypotheses until measured on a defined workload. Do not abandon fast Rust semantics categorically; pursue the fastest correct convergence the actual provider permits.

Use before/after measurements and profiles for performance changes. Avoid extra source-file digest freezes or preregistration procedures for ordinary exploration. Release thresholds need a defined supported workload; they are not universal promises for arbitrary repositories.

Track integration delay and obvious time spent maintaining assurance when evaluating the new workflow. Artifact count, packet count, test count, lines deleted, and a target reduction percentage are not product completion metrics. Remove support work when it no longer enables a user outcome, protects a specific consequential failure boundary, or supports a demonstrated operational need.

The first release is complete when its stated questions, two-language semantic contributions, live updates, truthful remainder, and persisted restart work through the actual service under the declared operating limits. The full target is complete only when the remaining selected families/forms and sustained-operation requirements are delivered, with limitations stated accurately. Process reform is complete when retired rules and validators no longer govern routine work, not merely when a new paragraph recommends ignoring them.

## 14. Instructions for deriving the execution plan

Use this document as the consolidated recommendation source. The plan must cover **all** of the following, not just the first product slice:

- Preserved product semantics and complete remaining Python/Rust family/query scope (§§2–4, 12).
- Simplified analyses, update scheduling, publication, storage, and recovery (§5).
- Separation of actual runtime validation/artifacts from software assurance (§6).
- The reduced resource contract and consumer-first native patch reduction (§7).
- Product fixtures, differential/lifecycle testing, and proportionate validation (§8).
- Workflow/skill consolidation, short instructions, the living handoff, and same-tree integration (§9).
- Removal of obsolete code, tests, dispatchers, and artifact/plan machinery (§10).
- Coordinated changes to operative design, policies, and navigation (§11).
- Sustained operation, measured optimization, and honest completion criteria (§§12–13).

For each planned outcome, record the affected surfaces, genuine dependencies, relevant validation, and any material transition risk. Give immediate work implementation detail; keep later work at capability level until it becomes actionable. Existing tests and commands may satisfy multiple outcomes. Do not reproduce fixed oracle quotas, proving-commit requirements, whole-worktree digest gates, or separate activation/state machinery to execute their own retirement.

The selected decisions are settled for planning: retain the Rust/Arrow/DataFusion/Delta architecture; preserve all substantive product doctrines; ship a useful subset before full completion; relax universal native allocation admission explicitly; use independent product evidence plus targeted lifecycle checks; consolidate the process; and delete obsolete machinery through exercised replacements. A memory-only production restart, wholesale fork removal before consumer changes, a single universal test, and calendar-based archival are not part of this recommendation set.

Derive the plan directly. Another review or design cycle is warranted only for a newly discovered consequential uncertainty that this document does not resolve. Ordinary implementation choices can be made within these boundaries without repeatedly requesting approval for already authorized work.

## Source scope and document validation

The consolidation uses the two source assessments and their completed reconciliation. The selected production paths and manifests have no new working-tree edits relative to that comparison; the pre-existing additions to the process-assessment artifact schema/validator remain separate. Implementation observations and test counts above are inherited, attributed evidence, not fresh production certification.

The [active plan](../plans/codefabric_real_time_cpg_implementation_plan_v3_2026-09-07.md), [planning-contract amendment](../designs/codefabric_real_time_cpg_planning_contracts_design_v2_2026-09-07.md), and [comprehensive design review](../designs/codefabric_real_time_cpg_comprehensive_review_design_v1_2026-09-04.md) supply the existing scope and change surfaces. They are inputs to the forthcoming correction, not a reason to reintroduce the workflow this review retires.

The external primary sources linked above support limited engineering observations already used in the source reviews. They do not establish compatibility with the repository's patched dependencies. This documentation-only change is checked for local links, spelling, and whitespace. No production test, benchmark, policy change, plan activation, or source-code change is performed by publishing this consolidated review.
