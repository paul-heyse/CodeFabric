# CodeFabric: pragmatic architecture and product delivery review

Date: 2026-09-08. Repository baseline: `0cc7242` on `master`.

This is a recommendation requested by the user, not activation of a replacement design or a claim of implementation completion. It reviews the development rules themselves. Production code, skills, accepted plans, and execution state are unchanged by this review.

## 1. Judgment

**The product objective is sound. The current method makes too much supporting machinery a prerequisite to demonstrating that product. I recommend a substantial simplification of the assurance architecture, a smaller development workflow, and delivery organized around usable Python/Rust graph capabilities. Keep Rust, Arrow, DataFusion, and Delta.**

Your concern is supported by both the written requirements and the current code. The problem is not simply that there are many tests. The requirements have expanded the meaning of correctness into an obligation to build a generalized, self-describing, executable proof system around much of the application. Changes must then maintain that system, its storage, its resource ownership, its documents, and its validators. Some of those requirements create more state and failure boundaries than the product behavior they protect.

The earlier comprehensive review actually recommended a working two-language pipeline and scoped clean recomputation. Its useful priorities became diluted by inherited doctrine, broad prerequisite closure, and the subsequent native-resource amendment. That amendment was explicitly approved; its implementation is not unauthorized work. Nevertheless, its cost and position on the critical path are now good reasons to reconsider the requirement.

I also contributed to the delay. I allowed independent implementation branches to accumulate, treated too many newly discovered gaps as prerequisites, and overinterpreted source-artifact obligations. Those execution choices compounded the written rules. Simplifying the rules will help, but it must be accompanied by smaller changes and earlier integration.

I agree with moving much more verification into snapshot audits, regression tests, and targeted experiments. I would not rely exclusively on auditing final snapshots. A correct snapshot can coexist with missed updates, stale query responses, broken cancellation, or unsafe recovery. The replacement should retain a small set of checks for those behaviors and remove the requirement to continually recertify every intermediate calculation.

## 2. What the repository evidence shows

| Observed requirement or implementation | Why it matters | Recommended disposition |
|---|---|---|
| The active plan has 30 packets, eight milestones, seven decommission batches, and exactly four new oracle names per packet. `plan_assurance.py` enforces the four-oracle shape and rejects single-call aliases. | Acceptance becomes coupled to document structure and test-body shape. A useful existing test may need another wrapper or new assertions to satisfy the packet format. | Choose tests by behavior and risk. Reuse existing tests and parameterized helpers. Retain failure on an accidentally empty test selection. |
| Shared evidence policy says every checkable claim must become an executable check, and repeated invariants must become permanent automation. | There is no cost or consequence threshold. Explanatory statements can expand the permanent maintenance surface. | Automate important, recurring, cheaply detectable failures. Accept code review, types, and bounded reasoning where they are appropriate. |
| The DataFusion/Arrow reference skill imposes a ten-step mapping loop; its alignment manual requires a twelve-step review with named artifact outputs before implementing a material capability. The Delta navigator imposes a similar loop. | An API reference also acts as a design-process mandate, pulling ordinary library integration into another architecture exercise. | Keep exact API references and useful examples. Make alignment checklists advisory and remove the compulsory artifact pipeline. |
| `AGENTS.md` recommends proportionate validation but also requires `ci-fast` before editing; its Tier A covers almost the whole repository. `CLAUDE.md` instead allows using a cached baseline. | Contradictory defaults encourage defensive reruns and inconsistent behavior between agents. | One risk-based rule: use relevant existing evidence and run affected checks; run the complete suite in CI or at an integration boundary. |
| `prove_transformation_execution_contract` creates and executes a second physical plan for nonvolatile transformations. | It adds execution and allocation work to normal candidate construction. Two equal runs do not establish universal determinism or semantic correctness. | Run production calculations once. Move repeated execution and optimization comparisons to tests and an explicit diagnostic mode. |
| Startup provisions observation histories, seals the candidate, constructs proof relations, provisions proof histories, and persists those relations before activation. | Proof is part of the product's transaction and recovery topology, not just development documentation. | Keep commit validation and operation records; sharply reduce mandatory proof storage and re-execution. |
| The activation-proof helper constructs expectation/author/reviewer identities and a `CausalFaultOutcome::Detected` row before calling its evaluator. | In the inspected helper, that row is constructed rather than produced by an executed fault injection. Formal-looking proof records can still provide weaker evidence than their labels suggest. | Put actual fault injection in tests. Production should record the validations it actually performed without representing constructed identities as independent semantic verification. |
| WP79 includes original allocation ownership and preallocation admission inside the native dependency stack; the root selects 15 patched packages from 12 source directories. | A CPG application is carrying part of the maintenance responsibility of a resource-governed execution engine. | Reopen the strength of this resource guarantee. Preserve useful completed work while reducing the required patch surface. |
| WP80–WP82 depend on all of WP79; WP90 depends on advanced invalidation and overlays; WP96 follows retention; full queries in WP98 depend on WP99 and WP90. | Provider delivery, continuous updates, projection optimization, and query completeness wait on work that is not inherently necessary for their first correct implementation. | Separate semantic dependencies from shared-file scheduling constraints and optimization dependencies. |

Sources: [active plan §§1.4, 3 and WP79–WP106](../plans/codefabric_real_time_cpg_implementation_plan_v3_2026-09-07.md), [evidence policy §§0, 2, 6](../../.claude/skills/_shared/evidence-policy.md), [AGENTS.md, Assurance tiers and Invariants for agents](../../AGENTS.md), [CLAUDE.md, Session start](../../CLAUDE.md), [plan assurance](../../tooling/ci/plan_assurance.py), [transformation execution](../../src/fabric/programmatic_schema.rs), [startup](../../src/fabric/production_workspace_startup.rs), [activation proof helper](../../src/fabric/proof.rs), and [Cargo patches](../../Cargo.toml).

The additional library-workflow requirements are in the [DataFusion/Arrow skill](../../.claude/skills/datafusion-pyarrow-rust-ref/SKILL.md), [alignment manual §1.2](../library_ref/datafusion55_arrow59_design_principle_alignment_manual_2026-08-24.md), and [Delta skill](../../.claude/skills/deltalake-rust-ref/SKILL.md).

The retained closeout run reports 1,038 passing root tests, 13 failures, and two skipped cases. The failures depend on production activation-control provisioning outside its required native mutation runtime. This is a concrete example of strong local checks coexisting with an unusable production startup path. It does not mean those passing tests are worthless. See [retained test log](../../target/native-closeout-root-tests.log); this review did not rerun it.

A tracked-file census also found 28 Python files and 12,008 lines under `tooling/ci`, and 39 plan documents totaling 91,738 lines. These counts include tests and historical documents; they are indicators of maintenance surface, not measurements of wasted effort. Vendored upstream source size is not counted as code we authored. No reliable percentage of development time lost to process can be inferred from these counts.

## 3. Preserve the product contract and make partial progress useful

The following are worth keeping as product requirements:

- One Rust daemon owns the graph, update scheduling, query execution, and current snapshot selection. Python remains a thin presentation layer; the existing sidecar and extractor boundaries isolate unstable provider APIs.
- Arrow is the in-memory and provider data boundary. DataFusion performs relational transformations and queries. Delta provides durable versioned relations. There is no need for a second graph database.
- Raw observations, normalized facts, and application-derived analyses remain distinguishable. Application IDs do not become compiler-local IDs.
- Queries pin a coherent snapshot. Invalidated semantics are never presented as current. Empty output is distinguished from incomplete processing or unresolved semantics.
- Provider context is effective: Python search roots/stubs/settings and Rust target/features/cfg/toolchain must change actual analysis, not merely its recorded hash.
- Publication, recovery, source disclosure, and compiler containment have clear ownership. A failed compile must not revive stale compiler facts.

These preserve the useful substance of ONT/GEN, QRY §7 “Evidence, unknowns, absence, and provenance,” LIFE §6 “Update pipeline and analysis lanes,” and the [comprehensive review §2.1](../designs/codefabric_real_time_cpg_comprehensive_review_design_v1_2026-09-04.md).

“Full CPG” should mean complete implementation of the declared Python/Rust fact families, with explicit limits of static analysis. It cannot mean exact prediction of arbitrary dynamic execution. Missing implementation must remain distinguishable from genuine language uncertainty. Delivering some families first is a delivery stage, not a redefinition of the eventual target.

### A practical freshness and completeness contract

Each response should identify its captured source/context revision and published graph snapshot. It should also expose processing status by the smallest scope the provider can honestly identify: language, context, file/module/owner, and fact family.

Keep separate dimensions:

| Dimension | Examples |
|---|---|
| Processing | queued, running, completed, failed, cancelled |
| Coverage | complete for requested scope, partial, indeterminate, unsupported |
| Freshness | current for the requested revision, superseded, awaiting processing |
| Semantic precision | exact observation, conservative possible set, heuristic, unresolved |
| Cause and action | compile error, missing dependency, resource limit, retry scheduled, implementation missing |

For example: “Syntax for `a.py` is current; type and call resolution for its Python context are running; Rust compilation failed for target X; these relationship results omit the listed pending scopes.” Pending scope details may be paginated. If invalidation is still being calculated, report that fact and conservatively name the larger potentially affected scope. Do not invent a precise file list or completion percentage from unavailable provider telemetry.

The current-source query policy should exclude invalidated semantic facts. An explicitly requested historical snapshot may remain queryable with its own revision. A pending-work observation can report newer disk changes without mutating the meaning of an already pinned graph snapshot.

This makes incomplete processing a useful, first-class product result. It avoids making one slow semantic provider block all available syntax or unrelated graph queries.

Separate implementation support, per-snapshot coverage, and test evidence. A release can implement a fact family while one workspace job fails to produce it. Successful CI does not complete that job; conversely, the job need not rerun the release's independent semantic corpus before its facts are usable. Supported operations should come from installed implementations, coverage from actual processing, and confidence in the implementation from appropriate tests and review.

## 4. Separate runtime validation, operation records, and software assurance

The current design connects these concerns too tightly. They need different costs and execution schedules.

| Mechanism | Normal operation | Tests, audits, or diagnostics |
|---|---|---|
| Typed inputs and boundary validation | Required: valid IDs, schemas, source/context binding, provider termination, generation checks, lawful publication | Fault and malformed-input cases |
| Graph integrity | Validate changed partitions and their affected references; check replacements and coverage | Full-snapshot scans, larger randomized edit sequences |
| Semantic correctness | Execute reviewed algorithms; report scope and precision | Independent expected graphs, focused properties, provider comparisons, algorithm review |
| Determinism and optimizer equivalence | Versioned implementation and valid dependency/cache keys | Repeated runs, reordered inputs, optimized/unoptimized comparisons |
| Runtime artifacts | Small records of actual operations and their results | Detailed intermediate capture when diagnosing a problem |
| Recovery and concurrency | State machine, single publication owner, exact readback, owned task lifetimes | Deliberate crash, race, timeout, and cancellation tests |

**Honor the runtime-artifact requirement at the operation boundary.** A source capture, provider run, analysis batch, publication, or query should leave a structured result/event with an operation ID, parent operation, source/context references, implementation version, affected scope, terminal outcome, gaps/diagnostics, and output references. Reuse Delta commit metadata and a compact operation history where appropriate. A query response is itself a result artifact; retention of its operation record can be bounded.

This does not require a separate Delta table or durable artifact for every function, allocation, schema adaptation, or intermediate plan node. Record detailed logical/physical plans and intermediate batches on demand or for selected failures. Preserve enough inputs and version information to reproduce an investigation within the supported retention window. State explicitly when old inputs have expired; do not promise indefinite reproducibility.

Catalog metadata is often reconstructible from the installed typed schema and plans. It need not always be separately historicized, reopened, compared, and proved. Changes to that rule require amending FAB §9.4 “Durability classification and exact reconstruction,” FAB §13 “Resources, observability, and proof,” LIFE §7 “Validation and candidate construction,” and GEN §§85/88. Merely changing a skill would leave the production obligation intact.

### Where your theoretical argument is useful

Use types and short algorithm/protocol arguments for properties they can establish. Examples include owner-scoped replacement by anti-join/union, convergence of monotone transfer functions over a finite lattice, and publication through one immutable manifest. Document assumptions, then test their implementation and boundaries. Deletions across versions still require invalidation; monotonicity within one analysis run does not solve that problem.

A test provides evidence for its cases. It does not generally prove a property for every input. Conversely, a type-enforced invariant does not need a duplicate runtime proof table. The doctrine's assertion that executing twice decides determinism is too strong. P25's allowance for proof by construction is useful and should be reflected consistently in the skills.

Snapshot audits should combine two kinds of evidence: incremental-versus-clean comparisons to catch invalidation defects, and independently expected facts to catch algorithms that are identically wrong in both paths. Retain targeted lifecycle tests because a later snapshot cannot reveal every stale response or unsafe intermediate transition.

## 5. Simplify the architecture around a working pipeline

The preferred path remains:

```text
source changes -> capture/context -> Python and Rust providers
  -> owned Arrow facts -> Rust analyses and DataFusion transformations
  -> changed partitions plus coverage -> exact Delta snapshot publication
  -> pinned DataFusion query -> useful facts and explicit processing remainder
```

### Keep responsibilities small

The update scheduler coalesces events and prioritizes work. Provider adapters translate native observations. Analysis functions compute semantics. DataFusion handles relational algebra. The publisher validates and selects committed state. The query service reports facts, coverage, and freshness. Supporting abstractions should earn their place by simplifying one of these responsibilities.

Keep a versioned, immutable application configuration and explicit schema/algorithm identities. Do not require every behavior to become a general-purpose compiled release/proof program. Ordinary typed Rust enums, functions, and builders are often the simplest executable definition. A single owner does not require a generic registry or another compiler layer.

Use DataFusion heavily where it fits: filtering, projection, normalization, joins, owner replacement, aggregation, and query composition. Use ordinary Rust or petgraph for control-flow construction and graph algorithms where that is clearer or more efficient. They must consume and emit the same Arrow-based graph representation. This preserves the data fabric without forcing an algorithm into awkward relational machinery solely to satisfy a library-utilization rule.

### Start with conservative recomputation

Capture edits, invalidate conservatively, and recompute an affected file, module, or compiler context. Retain full-context recomputation as the correctness reference. Narrow dependency tracking only when a real workload shows that broader invalidation is too expensive. Unknown or missing dependency information widens work rather than allowing a stale cache hit.

The original review already recommends this in §3.4. Apply it to delivery order: a working watcher with conservative recomputation does not need every interprocedural analysis and exact negative-lookup dependency before it can be useful.

### Keep coherent publication; reduce storage amplification

Retain the application-level manifest that selects an exact vector of relation versions. Delta's documented transactions are table-level, so removing application coordination would not provide coherent multi-table graph snapshots. [Delta Lake FAQ](https://docs.delta.io/delta-faq/)

Use stable table roots, reuse unchanged versions, and batch small updates. Persist graph facts, coverage, input references, and the records needed for recovery. Avoid rewriting every relation or creating a new table hierarchy for every update.

Start with correct replacements at a practical granularity. Add durable overlays only when measured publication latency justifies the extra read, compaction, retention, and recovery paths. Add CDF only for an actual downstream catch-up consumer; the local update already knows which work it performed. The original review §3.4 explicitly allows this.

Safe retention remains necessary before unattended use. A first interactive slice may use conservative retention with an explicit disk cap and backpressure. That is not a completed long-running retention solution. Do not implement an application Delta file-deletion planner to avoid the native API's limitations.

## 6. Reconsider the native-resource amendment explicitly

**The highest-cost requirement to reconsider is rejection before essentially every relevant native allocation, with shared receipts for original retained backing throughout the dependency stack.** This is materially stronger than an application with bounded concurrency, a shared DataFusion pool, bounded buffers, and measured resource headroom.

My recommendation is an initial operational contract comprising:

- A shared DataFusion memory/spill budget across active queries and updates.
- Bounded provider jobs, native work submission, channels, source/value sizes, IPC batches, results, caches, and retained snapshots.
- Clear ownership of application buffers and tasks; no accidental multiplication of per-epoch budgets.
- Existing subprocess containment, cooperative cancellation, and joined cleanup for work that can be joined. A timeout notification is not a claim that an uninterruptible task has terminated.
- Measured daemon/provider RSS and disk use, headroom, and admission backpressure.
- Host/process containment as a backstop where supported, with restart and unknown-outcome recovery. It may terminate work; it is not graceful per-allocation rejection.

This deliberately **does not promise that every library allocation is pre-admitted or that the process cannot exhaust memory**. Input caps alone also do not prove safety against compressed expansion. A stronger malicious-input or hard-memory contract needs appropriate decoder limits, isolation, or selected native changes. It cannot be claimed from this simpler profile.

DataFusion's own tuning guidance treats partition count, batch size, memory pools, and spill as workload-dependent tradeoffs. It supports measuring a bounded operating profile; it does not establish CodeFabric-wide allocator coverage. [DataFusion configuration and tuning](https://datafusion.apache.org/user-guide/configs.html)

LD-RT09 and WP79 must be amended if this recommendation is accepted. If the existing strict guarantee remains non-negotiable, the native work remains substantial engineering; it cannot honestly be eliminated by relabeling tests.

Do not discard the merged work wholesale. Freeze further expansion of the fork, identify which changes fix demonstrated semantic or lifecycle defects, and keep those. Remove unnecessary first-party dependencies on the stronger resource APIs in small, compiling changes, then reduce patches where the replacement behavior is exercised. Track upstream origins normally in Git. Dependency changes merit version/conflict and compatibility checks, not a new licensing or source-artifact approval program.

For the immediate startup failure, either finish the minimum admitted-operation wiring under the current contract, or replace that boundary coherently under the revised contract. Simply disabling the store's guard would leave an inconsistent implementation.

## 7. Replace the compulsory workflow with a short delivery loop

The default loop should be:

**Choose one observable capability → inspect its immediate code path → implement and integrate it → run relevant checks → exercise it through the daemon → fix concrete defects → continue.**

Design and independent review remain tools for consequential decisions, not mandatory stations for every change. This aligns with DORA's emphasis on small batches, short-lived development branches, and fast integration feedback. It does not require remote deployment or publishing from this workspace. [DORA: trunk-based development](https://dora.dev/capabilities/trunk-based-development/), [DORA: working in small batches](https://dora.dev/capabilities/working-in-small-batches/)

| Current skill or rule | Replacement behavior |
|---|---|
| `design-development` | Use for changed semantics, durable formats, trust boundaries, or major performance tradeoffs. Produce the shortest decision note that resolves the question; no mandatory clean-sheet alternative for routine work. |
| `library-capability-research` and `lib-leverage` | Target a concrete API uncertainty, defect, or performance opportunity. Consult exact local sources first. Do not survey all capabilities before using an established library. |
| `impl-plan` | Maintain a small ordered backlog of user-visible outcomes. Give detail to the next slice; keep later work at capability level. No fixed four-oracle quota or repeated twenty-heading packet template. |
| `plan-audit` | Optional challenge of risky sequencing or architecture. Missing procedural fields are not automatically product blockers. |
| `integrate-plan-audit` | Apply accepted corrections directly to the current working design/backlog. Git preserves history. No routine copy/version/activation transaction. |
| `impl-plan-exec` | Integrate each small usable change into the canonical tree. Stop scope expansion when it ceases to be necessary for that behavior; record the follow-up. |
| `impl-status` | Answer what works through the real product, what fails, and what is next. Reuse attributable test reports; do not rerun everything just to reconstruct status. |
| `implementation-review` | Review changed behavior and material risks. Add a finding for a missing test when it leaves meaningful uncertainty, not merely because a contract clause has no named oracle. |
| Library reference navigators | Keep their API guidance. Remove the automatic escalation from an ordinary API edit into full doctrine/plan conformance. |
| `skill-eval` | Use when workflow behavior is itself being deliberately evaluated, not as a prerequisite for product delivery. |

Specific policy changes:

1. Replace “every checkable claim must become a check” with “automate failures whose consequence, recurrence, and detection cost justify maintaining the check.”
2. Retain substantive tests, truthful reporting, and no empty test selections. Remove test-body policing as a substitute for reviewing assertions.
3. Allow ordinary Git-tracked designs and plans to evolve. Version released wire/storage contracts when compatibility changes; do not version the whole design suite for an implementation correction.
4. Keep test reports as historical observations tied to a commit, command, environment, and result. Recompute current validity when relevant code or inputs change. Historical results are not made more truthful by prohibiting their storage.
5. Remove the unconditional pre-edit full-CI rule. Keep an affected-code check loop and an integrated baseline whose failures are visible.
6. Retire repeated doctrine scores and mandatory per-finding ceremony. Preserve decisions and genuine unresolved risks in concise prose.
7. Use the single canonical working tree requested here. Delegate only bounded tasks with disjoint file ownership and one integration owner. No independently evolving interface designs or long-lived agent branches.

The normal review result should be an actionable correction or a named residual risk. Another review cycle is justified by changed semantics, unresolved high-consequence findings, or new evidence—not by the existence of the previous review.

## 8. Reorder delivery around observable product progress

Preserve the eventual Python/Rust family coverage and all eight query forms. Split their implementation from advanced optimization and universal assurance prerequisites.

| Delivery stage | User-visible exit | Existing scope to reuse or split |
|---|---|---|
| Restore an integrated baseline | The real daemon starts, opens its graph, answers a useful request, and reopens the selected snapshot after restart. | Immediate WP79 integration, existing activation and serving work. |
| Query real facts from both languages | Actual Python and contained Rust provider output reaches Arrow/Delta and can be inspected through entity, fact, and relationship queries with coverage. | WP80–WP83 plus useful parts of WP98/WP100. Start provider families incrementally. |
| Update without restarting | Same daemon handles edits/additions/deletions and context changes; fresh syntax and pending semantics are visible; after edits stop, results converge with a clean rebuild. | WP88/WP90 and minimal WP89/WP91. Conservative context invalidation and ordinary publication suffice initially. |
| Complete the graph and query semantics | Python CFG/dataflow, Rust MIR-derived families, common analyses, all eight forms, and partial-success composition operate with explicit precision. | WP84–WP87, remaining WP98–WP100. Python and Rust work need not serialize solely because they share an integration file. |
| Make real workloads fast and sustainable | Measured improvements in update/query latency, bounded backlog, safe finite retention, and reliable cancellation/recovery. | WP92–WP97/WP99/WP103–WP104 where demonstrated workload needs them. Basic resource and disk safety accompany earlier stages. |

Apply WP102-style installed behavior tests from the first stage instead of waiting until nearly the end. Simplify WP101's runtime proof scheduling at the point where duplicate execution obstructs the pipeline. Remove obsolete code as replacement routes land; reserve WP105 for residual cleanup rather than treating deletion certification as a product of its own. WP106 becomes a release readiness check over the demonstrated product, not a replay of the entire history of work packets.

The strongest early acceptance scenario is a small mixed-language repository: start one daemon; inspect real graph facts; edit Python and Rust; observe pending scopes; retrieve changed facts without restart; delete a definition and verify old edges disappear; introduce a compile error and verify a truthful gap; repair it and observe convergence; restart and recover the selected snapshot. Extend that scenario family by family.

A first usable slice is not the final product. Maintain an explicit remaining-family list so advanced semantics do not disappear behind a permanently “partial” label.

## 9. Reduce supporting code selectively

Use deletion to simplify the active product path, not as a separate exhaustive archaeology project.

| Surface | Action and preservation condition |
|---|---|
| `tooling/ci/native_dependency_artifacts.py`, its tests and retired recipe | Remove once references are detached. The user already rejected this source-packaging prerequisite. Keep normal Cargo source/version checks. |
| `plan_assurance.py`, `real_time_cpg_assurance.py`, `artifact_contracts.py`, plan-specific certification layers | Retire Markdown-to-proof-graph orchestration and fixed oracle-shape enforcement. Keep ordinary test runners and valuable graph/feature/protocol checks. Do not delete their behavioral test cases merely because their dispatcher is retired. |
| `fabric/programmatic_schema.rs` repeated transformation execution | Remove from normal updates after the contract changes. Preserve schema enforcement and move equivalence/determinism scenarios into tests or diagnostics. |
| `fabric/proof.rs` and `fabric/proof/delta_history.rs` | Separate necessary candidate validation from generalized expectation/fault/proof history. Preserve actual source/version/reference checks and operation outcomes. |
| `fabric/programmatic_observation_delta.rs` | Reconsider mandatory storage of reconstructible catalog observations. Retain data required for graph provenance, recovery, and invalidation. |
| `semantic_release.rs` proof and authority machinery | Keep the useful immutable configuration and typed program selection. Move test expectations and fault campaigns out of the production capability path; avoid maintaining a generalized proof language. |
| Native resource adapters and `third_party/native` | Preserve ordinary ownership and actual bug fixes. Reduce allocation-by-allocation instrumentation only together with explicit resource-contract changes and tested consumers. |
| Old plans, reports, and stale agent instructions | Keep historical material discoverable through Git/archive navigation. One current product specification and backlog should guide work. Historical documents must not continue to select live gates. |

This is not a recommendation to remove type-safe IDs, provider adapters, the scheduler, leases, exact version pins, or the publication owner. Those mechanisms protect facts and lifecycle behavior the user directly depends on.

## 10. A smaller, stronger assurance strategy

Use an independent semantic corpus as the main correctness investment. Start with compact hand-understandable Python/Rust programs: branches, loops, exceptions, imports, rebinding, dynamic calls, traits, moves, borrows, drops, and broken compilation. Expected graph fragments should be reviewed separately from their production builder. Add cases when a defect is found.

Run randomized edit sequences and compare incremental output with clean recomputation. Include deletion, rename, configuration changes, newly resolvable imports, cancelled work, and delayed provider results. Sample real repositories periodically. These checks exercise the behavior you actually want to trust.

| Change | Normal evidence |
|---|---|
| Local implementation or refactor | Affected build/check and relevant existing tests; add a regression when the behavior warrants one. |
| New graph family | Expected graph fixtures plus provider-to-query integration and explicit missing-input behavior. |
| Invalidation change | Clean/incremental comparison including removal and newly visible dependencies. |
| Storage/publication/recovery change | Exact reopen, uncertain commit, crash-boundary, and lease cases. |
| Shared native dependency change | Affected compatibility tests, one resolved type universe, and the real application consumer. |
| Performance change | Representative before/after workload and profiles; correctness checks for the changed execution strategy. |
| Release candidate | Integrated suites, mixed-language product scenario, selected failure/recovery tests, and supported workload measurements. |

Fuzzing, allocation-failure injection, and mutation testing are valuable for selected risky boundaries. They need not be attached to every parser edit or contract clause. SQLite's testing practice demonstrates the value of independent harnesses, fault injection, and optimization comparisons; it does not imply that CodeFabric should reproduce a database engine's entire assurance investment before delivering code intelligence. [How SQLite Is Tested](https://www.sqlite.org/testing.html)

Measure product progress by working fact families and queries, freshness/remainder correctness, edit-to-syntax latency, semantic convergence after a quiet window, query latency, backlog age, RSS/disk use, and recoverability. Track commits/integration delay and time spent maintaining assurance when deciding whether the new workflow helps. Test count, artifact count, and packet count are not product-completion metrics.

Keep benchmark records with the workload, machine, software revision, and raw samples. Use them to evaluate changes, without making a source-file digest freeze or a new preregistration procedure necessary for ordinary performance exploration. Reserve fixed acceptance thresholds for a defined release workload; the comprehensive review's latency numbers are proposals, not established measurements.

## 11. How to make this change without another process project

Make one coordinated policy/design revision, then immediately restore the working product path. The revision should state the preserved semantic guarantees, the simplified runtime validation/artifact policy, the chosen resource guarantee, and the new delivery order. Apply it to the active documents and the skills that enforce them in the same change. Do not add another layer of overriding prose while leaving contradictory validators active.

The concrete authority surfaces are:

- `AGENTS.md` and the governing repository specification's baseline, evidence, and test-tier rules; align the `CLAUDE.md` shim with them.
- Shared skill evidence/validation/doctrine/artifact policies and the workflow entrypoints above.
- The DataFusion/Arrow and Delta alignment manuals' mandatory mapping, artifact, and stop-condition rules; editing only their navigator skills would leave another source of the same obligations.
- Data-fabric doctrine P9/P10/P16/P17/P19/P20/P23/P25/P26/P28/P30/P36, preserving their useful intent while removing total per-clause and per-execution proof obligations.
- FAB §§9.4/13/14, LIFE §§6–8, GEN §§85/88/93–96, and the connected suite/query proof requirements. Keep factual coverage and transactional integrity separate from release assurance.
- Planning-contracts D-RT02/D-RT05 and LD-RT09, followed by the active backlog and its gate dispatchers. LD-RT08 maintenance should be judged separately against the reduced storage/consumer needs.

Keep released external contracts stable unless a concrete product change requires versioning. There is no architectural need for a synchronized copy of all eight master documents merely to correct an internal development rule.

The next implementation objective should be: **one integrated Rust daemon that can start, expose real Python and Rust graph facts, report pending work precisely, and remain running as those facts change.** Supporting work should either enable that objective, protect a specific material failure boundary, or wait until an observed workload justifies it.

## Review scope and limits

This assessment inspected the active plan and state, the complete incorporated comprehensive review and planning amendments, the seven main workflow skill entrypoints and shared policies, the relevant library navigators, root `AGENTS.md`/`CLAUDE.md`, selected current FAB/GEN/QRY/LIFE clauses, and representative production/tooling paths. Source searches covered the tracked application, supporting tooling, and named native selection; they were not a whole-program dead-code proof. No production tests, benchmarks, code changes, or automatic plan activation were performed for this review.

The implementation examples were verified in the current checkout. Historical review findings were used for the design's rationale, not assumed to describe every current function unchanged. External sources above support limited engineering/library observations; they do not select a dependency upgrade or prove compatibility with CodeFabric's patched graph.

To reproduce the main observations: inspect `git status --short`, `git rev-parse HEAD`, the active plan's dependency lines, `Cargo.toml`'s patch table, `prove_transformation_execution_contract`, `evaluate_compiled_activation_candidate`, and `build_fresh_candidate`; enumerate tracked paths with `git ls-files` for the bounded size census. The referenced retained test output is evidence from the completed closeout run, not a new execution during this assessment.
