# Reconciling the pragmatic CodeFabric delivery assessments

Date: 2026-09-08. Comparison baseline: `59aab15` on `master`; both source assessments use implementation baseline `0cc7242`.

This document assesses the other agent's recommendations and specifies what should be incorporated into the original assessment. It does not amend either assessment, activate a replacement plan, or authorize a code deletion. The current task changes only this document.

The two inputs are:

- **Original assessment:** [CodeFabric: pragmatic architecture and product delivery review](codefabric_pragmatic_product_delivery_review_2026-09-08.md), referred to below as **A**.
- **Other assessment:** [Pragmatic delivery of the Python/Rust CPG: process and architecture assessment](process_assessment_pragmatic_cpg_delivery_2026-09-08_v1.md), referred to below as **B**. Its recommendations are numbered R1–R9.

## 1. Recommendation

**Use A as the architectural foundation, strengthen its delivery and process recommendations with B's concrete proposals, and correct B's assurance and migration assumptions before incorporating them.**

B is right to demand a sharply defined first useful release, a much smaller default instruction set, a living handoff document, tests named after behaviors, and an end to maintaining plan machinery as a second product. Those recommendations make the original assessment more actionable. A's proposal to make the existing workflows optional is probably insufficient on its own: leaving all their entrypoints and supporting rules intact makes it too easy to resume the same cycle.

The main disagreement is substantial. B recommends a memory-only first product, removing Delta and much of the existing runtime, then reconsidering them after measurement. That is a new implementation direction with its own migration cost. It would defer an explicitly selected part of the user's product, and several proposed deletions remove useful lifecycle guarantees along with excessive proof machinery. I recommend **a smaller production path through the existing Rust/Arrow/DataFusion/Delta system**, with a narrow useful release and aggressive removal of unnecessary assurance dependencies as each replacement lands.

B's claim that a golden corpus and one differential test can replace essentially all other correctness checks is also too strong. Product-level tests should become the primary demonstration of value, while selected boundary and lifecycle tests protect failures a final answer cannot reveal. This does not justify restoring four new oracles per work packet.

The combined position should therefore be more decisive about retiring the compulsory process, more concrete about the first release, and more selective about architectural deletion than either a blanket preservation or a wholesale restart.

## 2. What B adds, and how it differs

Many of B's recommendations reinforce A rather than introduce new ideas. Both already favor honest incompleteness, early Python/Rust provider integration, independent semantic fixtures, clean-versus-incremental comparisons, simpler resource requirements, removal of packet-shaped assurance, and measured optimization. The useful differences are these:

| B's contribution | Difference from A | Recommended integration |
|---|---|---|
| An explicit v0 question list using four existing query forms, with advanced forms visibly unsupported | A specifies staged exits but does not give the first release an equally concrete question menu | Add the menu, preserve the existing wire surface, and retain an explicit backlog for the full graph and remaining forms |
| A root `STATUS.md` with working behavior, failures, next work, and verification commands | A calls for a small backlog and truthful evidence without selecting one simple handoff location | Adopt one living status/backlog document instead of a parallel plan-state system |
| Retiring workflow skills instead of merely relaxing each one | A's skill-by-skill reform may leave too much habitual ceremony available | Consolidate routine execution into one short workflow; retain concise, optional design/review guidance and targeted library references |
| Short `AGENTS.md`, with infrastructure explanations moved to references | A identifies contradictions but is less explicit about reducing always-loaded context | Adopt progressive disclosure; keep actionable rules and navigation in the entrypoint |
| Behavior-based test names and pruning as modules change | A removes dispatchers but does not explicitly address packet identities embedded in test names | Adopt opportunistically; keep substantive scenarios and existing helpers |
| A named product test command with reviewable expected answers | A proposes an independent corpus but does not specify its command or result format | Adopt a small product harness, without making every answer a full-response snapshot or every edit run the entire corpus |
| A memory-backed catalog swap as the first production substrate | A retains simplified Delta publication from the first integrated release | Retain immutable snapshot selection as an implementation idea; do not adopt memory-only production or defer Delta by default |
| Removing the complete native fork and broad fabric module groups | A recommends reducing patches and abstractions as their consumers are replaced | Adopt the urgency to reduce maintenance, but preserve A's compiling, behavior-preserving removal order |
| Quantitative churn and code-composition analysis | A gives a smaller, more qualified maintenance census | Use it to locate expensive areas, not to infer a percentage of wasted code, elapsed effort, or causation |

## 3. Disposition of all nine recommendations

| Recommendation in B | Disposition | What the combined assessment should say |
|---|---|---|
| **R1 — Move proof to the product boundary** | Adopt with substantive corrections | Build a small independent semantic corpus and exercise the real daemon/MCP route. Reuse a differential harness across many edit cases. Keep focused tests where they cover distinct material risks. Remove the four-oracle quota, not all intermediate tests. |
| **R2 — Facts first, fabric last** | Adopt the priority; reject the proposed default migration | Make useful facts and queries the organizing objective. Remove generalized proof and advanced storage prerequisites. Keep the selected data fabric, a minimal coherent publication boundary, and recovery. A memory-only experiment is justified only by a specific uncertainty, not as a second production architecture. |
| **R3 — Coverage envelope from the outset** | Adopt and expand | Make processing status queryable and include query-relevant completeness in every response. File status is a useful view, but context, fact family, semantic precision, failed work, and uncertain invalidation scope must remain expressible. Preserve all substantive product doctrines. |
| **R4 — Define v0 and ship it** | Adopt | State exactly which questions work for both languages, through the existing service. Unimplemented families and forms remain an explicit delivery backlog. Include minimal Delta persistence and restart behavior rather than adopting B's no-persistence boundary. |
| **R5 — Cut process weight** | Adopt the consolidation, qualify the mechanics | Replace compulsory plan/audit/activation/state cycles with one living status/backlog and short decisions where needed. Reduce always-loaded instructions. Remove enforcement consumers together with their rules. Avoid new page-count quotas, indiscriminate validator deletion, and automatic full CI for documentation work. |
| **R6 — Demote fabric principles** | Adopt as a classification, not a blanket waiver | Keep semantic correctness, ownership, coherent snapshots, and the selected stack binding. Make implementation preferences and conformance scoring advisory; remove generalized proof obligations explicitly. Amend normative domain clauses as well as skills. |
| **R7 — Behavior-based tests; healthy baseline** | Adopt, correcting the stated blocker | Rename tests when touching their behavior, remove packet dispatchers, and keep integration failures visible. Start from the retained current startup failure, not the August cached compiler error. |
| **R8 — Reverse the dependency fork** | Adopt reduction as an objective; reject unconditional removal | Freeze expansion, revise the resource contract, retain useful fixes, replace consumers, then remove patches no longer needed. Reconcile lockfiles and validate the real consumer. Do not remove the fork first or assume existing locks already select upstream sources. |
| **R9 — Simple commands instead of procedural completion claims** | Adopt the interface; reject the universal gate | Provide obvious product and integration commands with bounded, attributable results. A passing corpus supports the behaviors it exercises. Use affected checks during editing and integrated checks for integrated/release claims; report failures and untested scope. |

## 4. Corrections needed before integrating B

### 4.1 Correct final output does not prove all intermediate stages correct

B's D1 and §5 infer intermediate correctness from correct final relations or golden answers. That implication does not hold in general. A projection can discard an incorrectly calculated field; a deduplication can hide erroneous duplicates; a later bug can compensate for an earlier one. Correct answers to selected questions establish neither the correctness of every stored graph fact nor of every intermediate transformation.

This matters particularly because the product is an inspectable full CPG. Some intermediate facts are themselves public query results. Expectations for those exposed facts are product tests, even if a high-level callers query does not use them.

The replacement argument is simpler: use independent expected facts and answers for the behavior being delivered; use types and short algorithm arguments for invariants they establish; add local tests where they cheaply expose a consequential failure. Stop requiring ceremonial proof at every seam. A useful product test can cover several stages without certifying all of them separately.

### 4.2 Semantic extraction is not generally independent per file

B §5 says provider extraction does not depend on other files. Syntax parsing can often be isolated that way; semantic extraction cannot. Changing an imported Python declaration can alter another file's types or resolved calls without changing that file. Rust configuration, dependency metadata, macro expansion, and crate contents affect compiler results.

An “effective context” can represent those inputs, but then it must actually capture their changing dependency state. A fixed configuration label does not make file bytes a sufficient cache key. This is also why a per-file replacement map alone does not solve semantic invalidation.

Preserve A's conservative module/context invalidation first. Narrow it only where useful. Rust's own incremental compilation design tracks query dependencies and stable identities rather than treating semantic results as independent file functions; this supports the distinction, not a claim that CodeFabric already has the same machinery. [Rust compiler development guide: incremental compilation](https://rustc-dev-guide.rust-lang.org/queries/incremental-compilation-in-detail.html)

### 4.3 One differential harness is valuable; one final comparison is insufficient

Incremental-versus-clean equality checks consistency between two execution paths. Both can share the same semantic error. A comparison only after the final edit can also miss stale responses served during intermediate states, an old provider job overwriting a newer result, or a cancelled operation continuing to mutate storage.

Use one reusable harness with cases for edits, removals, renames, context changes, previously missing imports, delayed completions, and selected failure/restart boundaries. Compare relevant intermediate states as well as convergence after quiet. Keep independent expected facts alongside these comparisons. Expand cases when defects or new behavior justify them; do not turn this list into a new fixed quota.

Provider failure also has both correctness and coverage implications. It is an incomplete result only if stale facts are excluded or explicitly historical, affected dependents are handled, and the response tells the truth about coverage. Merely setting a file's status to `unavailable` does not ensure those properties.

### 4.4 Snapshot coherence is an operational mechanism, not evidence paperwork

B's immutable catalog pointer is a useful simplification if its referenced tables, source identities, and coverage are immutable and selected together. Replacing one `Arc` does not itself ensure this if a table beneath it remains mutable or relations are updated independently. Separate syntax and semantic generations also need compatibility and invalidation rules.

Keep the small concept: one selected snapshot descriptor binds facts, coverage, source/context inputs, and exact table versions. Remove the requirement for a generalized proof product around that descriptor. Delta provides table-level transactions; application coordination still has a purpose when the graph spans tables. [Delta Lake FAQ: transaction scope](https://docs.delta.io/delta-faq/)

B proposes removing durable exactness and later describes this as only a change in order. It is also a change to first-release behavior and storage architecture. Given the user's explicit Delta-based target and existing implementation, the combined assessment should not adopt it without an independent reason that outweighs the migration cost.

### 4.5 A memory pool and RSS watchdog are not a complete resource contract

B is right to question allocation-by-allocation accounting throughout fifteen patched packages. Its proposed replacement is too terse. A sampled watchdog reacts after memory growth; a retained-catalog count does not bound catalog bytes, and active queries may retain old catalogs after a registry drops them. Query memory accounting does not automatically cover provider processes, source captures, results, retained input batches, and native library work.

Keep A's smaller explicit operating contract: shared query budgets and spill, bounded application queues/jobs/buffers/results, bounded retained state, joined task ownership, provider containment, measured headroom, backpressure, and host containment where available. State that this does not guarantee rejection before every native allocation or immunity from OOM. DataFusion exposes workload-dependent batch and concurrency settings; tuning those is useful, but is not a proof of process-wide allocation coverage. [DataFusion configuration](https://datafusion.apache.org/user-guide/configs.html)

This is a real reduction in guarantee, not merely a testing-policy change. LD-RT09 and its production consumers need a coherent amendment. Keep actual lifecycle fixes from the merged work while removing mechanisms whose only purpose was the stronger guarantee.

### 4.6 The proposed immediate repair and fork-removal recipe are stale or incomplete

The following checks were read-only; no new build or product test was run for this comparison.

| B's assertion | Evidence checked in this comparison | Correction |
|---|---|---|
| Fix `provider_sandbox.rs:832` first for an `E0631` mismatch; the baseline has been red for nine days | [Cached baseline](../../target/agent/baseline.json) identifies `f12329f` on August 31. Current [provider sandbox](../../src/provider_sandbox.rs) uses `as_raw_nonzero().get()`. The [retained root check](../../target/native-closeout-root-check.log) completed successfully. | The old cached report is not a current failure diagnosis or evidence of continuous failure over that interval. |
| That one-line fix restores the practical baseline | The [retained root test run](../../target/native-closeout-root-tests.log) reports 1,038 passing, 13 failing, and two skipped tests. Failures identify activation-control provisioning without the admitted native mutation runtime. | Restore that real startup path under the selected resource contract, then rerun relevant integration checks. These retained results do not establish a current full-CI pass. |
| The lock already records upstream registry selections, so removing patches needs no resolution work | Current [Cargo.lock](../../Cargo.lock) entries for patched Arrow, Tokio, and Delta packages lack upstream `source` fields; [Cargo.toml](../../Cargo.toml) selects local paths. | Restore intended upstream identities deliberately and reconcile affected locks. Stable-graph validation alone is not a compile or runtime compatibility check. |
| Remove the native fork first, archive consumers later | [Owned local store](../../src/fabric/owned_local_store.rs), [workspace native execution](../../src/fabric/workspace_native_execution.rs), and related consumers enforce the patched ownership contract. B itself acknowledges these dependencies. | Replace consumers and their guarantees before dropping the APIs they compile against. Avoid temporary stubs that turn required work into apparent success. |

B's narrow query-executor observation is supported by the inspected `compiled_released_form_programs` route in [production query recipe](../../src/production_query_recipe.rs): it currently constructs the find-entities program from available Ruff bindings. That static finding is useful. It does not establish that the broken daemon currently serves that behavior, or refresh every older F01–F14 finding by association.

### 4.7 Use quantitative evidence without turning it into an unsupported causal claim

B's census highlights an unusually large support surface and frequent plan succession. That strengthens the case for simplification. However:

- Classifying a directory as “fabric/infrastructure” does not classify its contents as proof overhead. Query coordination, storage, ownership, and recovery include useful product behavior.
- Lines added/deleted measure churn, not why it occurred. They cannot by themselves establish that successive plans caused every deletion.
- The Appendix A `#[cfg(test)]` counter classifies the remainder of a file after a matching marker; it is an estimate, not a Rust module parser. Test counts and lines per test include helpers and formatting effects.
- Vendored upstream lines are not lines authored by the agents. Maintenance risk depends on local changes, interfaces, update frequency, and active consumers, not just checkout size.
- A syntax deadline of 100 ms, a 500 ms debounce, and “seconds” of startup for 50k lines are proposed operating targets or estimates. Neither report measures those claims for the proposed product.

Keep the maintenance diagnosis. Do not import “70% is proving facts that do not exist,” a claimed waste percentage, or an implied schedule into the combined conclusion. Measure actual end-to-end latency, including detection and debounce, alongside processing time before setting release thresholds. Rust semantics may require longer convergence; an absolute instruction never to attempt fast semantic refresh would unnecessarily abandon the user's performance objective.

## 5. Concrete improvements to integrate into A

### 5.1 Define a first usable release without changing the eventual target

Add B's question menu to A §8, with these meanings and limits:

| First-release behavior | Required distinction |
|---|---|
| Find declarations/entities and show source | Declaration enumeration can use syntax; resolving a use to its definition may require semantics |
| Follow references, imports, callers, and callees | Syntactic occurrences and call sites are not resolved semantic relationships. Possible or unresolved targets must be labeled accordingly |
| Retrieve available types and other supported facts | Use actual Pyrefly/rustc results with effective context; missing semantic support is explicit |
| Inspect workspace and scoped processing status | Report affected scope/family, failures, pending work, exclusions, and result completeness |
| Edit Python and Rust while the same daemon remains running | Invalidate conservatively, publish fresh available facts, reject obsolete completions, and converge after quiet |
| Restart and resume useful graph access | Reopen a coherent persisted selection, reconcile current source state, and report any pending refresh |

Use the existing `FindEntities`, `FollowRelationships`, `RetrieveFacts`, and `RetrieveSourceContext` forms. Preserve existing validation/reference tools and released wire contracts unless they demonstrably obstruct delivery. Tool count is not a complexity metric worth optimizing on its own.

The first accepted release should include real semantic contributions from both languages. Syntax-only milestones are useful progress, not fulfillment of semantic references/calls. Keep `FindPaths`, `MatchPattern`, `CombineResults`, `SummarizeFacts`, and the remaining Python/Rust analysis families visible as unfinished scope. Do not describe them as completed merely because the system can return `unsupported`.

### 5.2 Make status a queryable product relation and a useful response

Adopt B's status-first demonstration and per-file view. Implement it as a presentation over the same processing records that determine what facts can be served. Do not create a second status authority whose flags can drift from actual publication.

Extend the view beyond a pair of generation counters: include context, fact family, input revision, processing outcome, coverage, freshness, precision, and cause. These are logical requirements, not a demand for a new schema compiler or separate table for each dimension.

For a references query, workspace counts alone cannot explain whether missing files might contain additional references. Return the relevant incomplete scope, with paginated detail when necessary. If the scheduler cannot yet identify that scope, say that context-wide invalidation is pending. Avoid manufacturing a precise list or percentage. Preserve A's distinction between installed support, actual processing coverage, and release test confidence.

### 5.3 Give the product corpus a small operational shape

Add a `just golden`-style entrypoint, or an equivalently clear existing recipe, to A §10. Begin with tiny independently understood Python/Rust fixtures and a few real repository smoke cases. CodeFabric itself is a useful dogfooding target, not a requirement to author golden answers for its entire graph before delivering the first feature.

Store requests and expected semantic facts/answer fragments in a simple reviewable format. Normalize incidental output such as temporary roots or opaque operation IDs only where they are not the behavior under test. Preserve ordering where the contract defines it; preserve source spans, identities, coverage, and errors when those are what makes an answer correct. Whole-response snapshots otherwise risk turning harmless telemetry changes into continual golden churn.

Expected output should be independently justified. An acceptance command can propose updates; inspecting an automatically produced diff is not, by itself, an independent oracle. Reuse the repository's existing snapshot acceptance mechanics if appropriate instead of building another approval system. Selective fixtures and direct assertions remain valid alternatives to golden files.

Keep a focused edit loop and a small integrated product scenario. One convenient command may run several kinds of checks; the command's name does not make its coverage universal.

### 5.4 Replace the process topology, not just its wording

Strengthen A §7 with these concrete recommendations:

1. **One living handoff/backlog.** Use `STATUS.md`, or one clearly selected equivalent, for what works, what fails, the next usable slice, remaining target capabilities, and material open decisions. Record relevant commands/results with revision and date, including limitations. Do not duplicate this into judgment-heavy plan JSON and another status report.
2. **One ordinary execution workflow.** Consolidate routine plan/execute/status guidance into a short delivery loop. Keep optional design and independent review guidance for consequential changes. Remove automatic audit-integration and activation steps. Transitional skill entrypoints can redirect to the smaller workflow; they must not reload the retired policies.
3. **Short entrypoint instructions.** Keep build-domain boundaries, product invariants, command selection, preservation/integration rules, and navigation in `AGENTS.md`. Move detailed tooling rationale to referenced documentation. Treat B's 150-line target as an editing aid, not another gate.
4. **References remain references.** Retain useful pinned API navigators, but remove their compulsory doctrine-mapping loops and mandatory report outputs. The original assessment correctly identifies that these navigators are not all merely cheap pointers today.
5. **History stops controlling execution.** Retire active plan selection and dispatchers once their useful responsibilities have moved. Archive by supersession and relevance, not by document date. Do not move every older file and break live reference paths solely to achieve a tidy directory.

A small, manually maintained status file can also become stale. The remedy is to tie its claims to concrete behavior and update it when that behavior changes, not to recreate a schema-enforced status product. A completed documentation change should not require a full product gate, and an already authorized implementation should not require repeated permission for each numbered recommendation.

B §8 reports that authoring its assessment also required adding a new artifact type. The current pre-existing diff indeed adds `process-assessment` to [artifact schemas](../../.claude/skills/_shared/artifact-schemas.md) and [artifact validation](../../tooling/ci/artifact_contracts.py). Those are small edits, but they illustrate the coupling to remove: an ordinary requested review should not need a new production/tooling schema category. This comparison preserves those other-agent edits unchanged.

### 5.5 Retire tests and modules by behavior, not historical identity

Incorporate B's opportunistic test renaming into A §9. Remove packet prefixes as modules are changed; name the retained scenario for its failure or behavior. Do not schedule a repository-wide rename before product work. A test is neither useful nor useless because its packet was superseded.

Apply the same principle to code deletion. A Git reference preserves source bytes, but not a working future integration. The cost of reconnecting an archived module still exists. Remove unused machinery promptly after the active path stops needing it, in small compiling changes on the canonical tree. Keep ordinary cancellation, resource release, coherent source/query selection, and necessary authorization/disclosure checks. Do not remove whole module prefixes merely because some of their functions served proof machinery.

The combined assessment should be firmer than A about actually completing removals: once a replacement works and references are detached, delete the obsolete implementation and dispatcher in that change. Do not leave it as a permanently compiled fallback. That preserves progress while reducing the maintenance surface the user is concerned about.

## 6. What the combined assessment must retain from A

B omits or weakens several points that remain important:

- **Artifacts of runtime actions.** Capture/provider/analysis/publication/query operations should leave compact structured outcomes and input/output references. Retention may be bounded and detailed intermediates optional. Removing per-edit source bundles does not remove the user's runtime-artifact requirement.
- **Full product semantics.** Keep application-owned identity, raw/normalized distinctions, provider isolation, authority/conflict handling, source-context binding, and coherent query snapshots. B's R3 title says to keep only one doctrine, while D4/R5 retain eight; reconcile this in favor of preserving the substantive product guarantees.
- **Runtime enforcement versus software assurance.** Remove mandatory semantic fault campaigns, double execution, and reconstructed proof histories from ordinary publication. Retain the actual boundary validation and ownership that make the operation correct.
- **A precise resource tradeoff.** Reduced allocation assurance must be stated as a changed operating contract and reflected in code, not silently inferred from passing golden tests.
- **Same-tree integration.** Continue small changes in the canonical tree with one integration owner. This directly addresses the user's concern about accumulated independent work; neither archive tags nor better reports substitute for frequent integration.
- **Coordinated authority changes.** Skills alone cannot relax FAB/LIFE/GEN/SUITE/QRY runtime requirements. Change the operative rules and their enforcing code together, without another synchronized copy of every design document merely to revise a development policy.

## 7. Recommended combined sequence

This is an ordering recommendation, not another packet plan or a calendar estimate.

1. **Make one bounded policy and contract correction.** Establish the first release, preserved semantic guarantees, reduced resource guarantee, compact runtime operation records, and short workflow. Update conflicting live rules and remove their compulsory enforcement. Select the living status/backlog. Do not expand this into rewriting the whole documentation corpus.
2. **Restore the existing real daemon path.** Address the retained activation-control/native-runtime failure under that contract. Demonstrate startup, a useful query/status response, and coherent reopen. Reduce native patches only after their application dependencies are replaced; validate affected locks and builds together.
3. **Deliver the first useful two-language queries.** Connect provider output with effective context to Arrow relations, ordinary Delta publication, and the existing query service. Add independent fixtures for the supported questions as each lands. Integrate Python and Rust contributions early instead of completing a large Python-only product first.
4. **Make updates and remainder observable in the same process.** Introduce the watcher, conservative invalidation, stale-result rejection, and a quiet-window clean comparison. Exercise edit/delete/failure/repair/restart behavior. Add status expectations before claiming a query complete.
5. **Dogfood, extend, and simplify continuously.** Use real repositories, complete the remaining fact families and forms, remove obsolete code with each replacement, and measure update/query latency, convergence, backlog, memory, and disk. Add fine-grained invalidation, overlays, CDF consumers, and advanced maintenance where an observed need justifies them. Basic bounded operation and retention cannot wait for unattended use.

The practical improvement over A is a clearer first release and a more decisive reduction in process entrypoints. The practical improvement over B is avoiding a second architecture migration and retaining the small operational guarantees that make incomplete, continuously changing graph results trustworthy.

## 8. Exact changes recommended for the original assessment

| Section of A | Recommended revision |
|---|---|
| §1 Judgment | State that consolidating the workflow is preferable to merely making ten existing workflows optional. Add that a memory-only restart is not the recommended path. |
| §2 Evidence | Add qualified observations about plan churn, packet-named tests, and review-to-validator coupling. Retain the distinction between support-code size and demonstrated waste. Use current-attributed failure evidence. |
| §3 Product contract | Add B's queryable per-file status view and the concrete first-release question meanings; retain context/family scope and precision. |
| §4 Assurance separation | Add the explicit counterexample to “correct final output implies correct intermediates.” Clarify that public graph facts also belong to product-level testing. |
| §5 Architecture | Acknowledge immutable catalog selection as a useful simplification inside the fabric. Preserve minimal exact Delta publication and recovery; reject deferring the whole storage contract. |
| §6 Resources | Explicitly reject RSS watchdog plus catalog count as the entire replacement contract. Add consumer-first fork removal and lockfile reconciliation. |
| §7 Workflow | Add one living `STATUS.md`/backlog, consolidated routine execution guidance, shorter `AGENTS.md`, and removal of automatic workflow chaining. |
| §8 Delivery | Add the concrete first usable release from §5.1 here, with both semantic providers, same-process updates, and restart behavior. Mark later forms/families as unfinished scope. |
| §9 Supporting code | Add behavior-based test naming, retirement of inactive dispatchers, and deletion in the same change as a proven replacement. Reject prefix-based archival and calendar-based document moves. |
| §10 Assurance | Specify the small product command, independent expectations, selective normalization, and one reusable differential harness with multiple meaningful cases. Keep affected checks and targeted lifecycle tests. |
| §11 Transition | Use the sequence in §7 here; include removing contradictory validators/entrypoints rather than leaving another advisory override above them. |

The recommended combined assessment should supersede the two recommendations as the selected advice once reconciled. It should not create a permanent third review cycle: the next substantive deliverable should be the agreed policy/contract correction followed by a working product slice.

## Scope and validation of this comparison

Both assessment documents were read in full. Material disagreement checks covered the current production query route, source/context preparation, the native store and execution boundary, Cargo source selections, relevant agent rules, the other agent's two policy/validator additions, and retained closeout logs. Historical implementation evidence was not represented as a new test run. The comparison does not recertify every implementation-status claim in either report or measure performance for a proposed architecture.

External primary references support only the limited transaction, compiler-dependency, and tuning observations linked above; they do not establish compatibility with this repository's patched dependency graph. Validation for this documentation-only change consists of local-link checks, spelling, and whitespace checks. Neither source review, production code, active plan/state, nor pre-existing policy changes were modified.
