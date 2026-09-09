# CodeFabric pragmatic production implementation plan

Updated 2026-09-08. This is the editable production backlog following [non-production preparation](codefabric_pragmatic_delivery_nonproduction_preparation_plan_2026-09-08.md), derived from the [consolidated review](../reviews/codefabric_pragmatic_product_delivery_consolidated_review_2026-09-08.md) and [selected suite](../spec_index/README.md). [STATUS](../../STATUS.md) records demonstrated behavior and next work. No activation pointer, packet state, proving chain or second audit workflow selects this plan.

Production implementation is the next phase, not work completed by preparation. Keep the canonical tree, integrate coherent changes frequently, preserve existing useful implementation and choose tests by behavior and risk. Do not conduct another process migration before starting outcome 1.

## 1. Product completion boundaries

The first useful release starts one daemon, queries actual provider-derived Python **and** Rust semantics, reports the requested scope still unfinished, reacts to source changes without restart, converges after quiet, and reopens coherent persisted state. Its four forms are `FindEntities`, `FollowRelationships`, `RetrieveFacts`, and `RetrieveSourceContext`. Include resolved references/imports/calls and available types; syntax-only enumeration is insufficient.

The full product additionally includes `FindPaths`, `MatchPattern`, `CombineResults`, `SummarizeFacts`, all composition/partial-success semantics, every selected ONT/GEN fact family and Python/Rust/common analysis, and sustained bounded operation. Explicit unsupported output is truthful staging, not completion. Neither a green tooling suite nor startup alone closes the first release.

Preserve Rust semantic ownership, the four build domains, Arrow/DataFusion/Delta, application-owned identities, raw/normalized facts, source authority, explicit unknown/conflict handling, exact snapshots, publication ownership, authorization and modern presentation. Native Delta owns transaction/log/checkpoint/maintenance behavior.

## 2. Outcome 1 — Restore startup, real query/status and reopen

**First implementation slice.** Investigate and fix activation-control provisioning under the real production resource owner. The retained closeout reports `activation-control-provision: ... local store mutation requires its admitted native runtime`; the old August cached failure is not the current diagnosis. Preparation re-exercises the public startup test and records its actual result in STATUS.

Start at `src/fabric/activation_control_delta.rs`, `programmatic_delta_runtime.rs`, `programmatic_active_workspace_builder.rs`, `programmatic_command_runtime_factory.rs`, `workspace_resources.rs`, `native_resource_policy.rs`, the owned local store and supervisor startup callers. Follow the actual write to its owning runtime; avoid another wrapper/receipt tier. Retain temporary admitted wiring where it makes this transition small and correct. Do not disable the write guard globally or replace the real daemon with test-only bootstrap authority.

1. Reproduce with `just golden --case startup`; inspect the concrete provisioning call and its current owner.
2. Make initial control writes and readback use the correct owned resources, with joined task lifetime and uncertain-write reconciliation.
3. Exercise a lawful empty-root start, status/query through the actual adapter, persisted reopen and a targeted failure at the changed write boundary.
4. Update STATUS with results and unresolved scope, then commit in the canonical tree. Continue outcomes 2–3 without expanding this first fix into universal allocator certification.

Reuse `tests/integration/daemon.rs` and the current `tooling/fastmcp4_modern_client_driver.py`; the golden runner selects existing real startup, Python-serving, reopen and cancellation cases with nonempty nextest selection. Run relevant Rust checks/tests after changes; integrated checks follow when the repair spans domains. Known production failures remain visible until fixed by changed behavior.

**Done:** the actual daemon starts and serves its supported query/status behavior and reopens exact persisted state. This does not imply both-language semantic completeness or full lifecycle correctness.

**Completed 2026-09-09.** Control provisioning, candidate publication and activation append/readback use bounded owned write lanes with joined cleanup and exact reader reconstruction. Completed metadata/error reads release their bookkeeping instead of holding a writer until the serving runtime shuts down. All four existing golden cases and 33 affected store/executor/lost-acknowledgement tests pass; see STATUS. Native allocation receipt estimates are no longer enabled for these production mutations. Remaining proof/resource simplification continues below.

## 3. Outcome 2 — Replace generalized proof execution with compact runtime records

Depends on a working startup boundary; work alongside resource simplification where callers overlap. Inspect `src/semantic_release.rs`, `src/fabric/proof.rs` and `proof/`, `programmatic_schema.rs`, `programmatic_epoch.rs`, `programmatic_observation_delta.rs`, query/publication admission and their consumers.

Use ordinary typed Rust builders, Arrow schemas, DataFusion plans and explicit configuration. Remove mandatory double execution, generalized expectation/fault/proof histories, fixed-point self-description and proof-language dependencies from startup/update/query acceptance. Keep boundary checks for schema, generation/context, identity endpoints, authorization, coverage and publication consistency.

Runtime actions retain compact operation/snapshot/input/provider identities, requested/completed/remainder scope, selected versions, outcome and useful diagnostics. Preserve records required by actual recovery and explanation consumers; bound optional detailed diagnostics. Source edits remain Git changes, without replay bundles or per-edit artifacts.

Replace consumers before deleting code or changing persisted contracts. Classify existing proof data by actual readers: recovery input, explanation, diagnostic, obsolete process. Preserve required durable compatibility or make an explicit forward migration. Update tests of the retired runtime contract with meaningful behavioral replacements; do not delete unrelated failing product assertions.

**Done:** useful supported results publish without generalized proof execution, query completeness remains truthful, and recovery/explanation use compact records. Validate real corpus answers, publication failure/reopen and affected schema/wire compatibility. Track removed production/support code as an observation, not a quota.

## 4. Outcome 3 — Implement the reduced resource contract and shrink native patches

Depends on known startup/publication owners; shares consumers with outcome 2. Inspect `resource_budget.rs`, `provider_admission.rs`, `src/fabric/workspace_resources.rs`, `resource_ownership.rs`, `native_*resource*.rs`, `arrow_result_resource.rs`, child-session resource governance and provider process ownership.

- Share DataFusion memory/spill budgets across current, candidate and leased work.
- Bound application jobs/queues/batches/concurrency/results/retained state and deadlines; keep control headroom.
- Own and join tasks and subprocess groups through cancellation/shutdown; retain correct final-owner lifetime behavior.
- Contain provider processes, monitor RSS/disk headroom, apply backpressure and recover interrupted work.
- Stop claiming universal native pre-allocation admission or immunity from OOM. Avoid copying/replaying native state solely to measure an imagined retained allocation charge.

After callers stop depending on generalized native receipt APIs, classify the local patches package by package. Remove admission-only modifications; retain genuine correctness, cancellation or maintenance fixes until upstream replacement exists. `third_party/native/` origin notes, Cargo path declarations and all locks identify actual selections. Preserve a single Arrow/Parquet/DataFusion/object-store/kernel universe and the existing stable/nightly separation. Compatible dependencies need normal conflict/API checks, not a licensing or source-artifact project.

**Done:** pressure/cancellation tests exercise real owners, budgets and cleanup; native-source changes compile and preserve affected behavior; `just stable-graph-check` and relevant feature/native tests pass. Resource measurements distinguish managed budgets from whole-process RSS.

## 5. Outcome 4 — Real mixed-language semantics through the first four forms

Depends on outcomes 1–3 sufficiently to publish and serve. Reuse `provider_native_syntax.rs`, `source_image/`, `analysis_context.rs`, `production_provider_recipe.rs`, `provider_boundary.rs`, `provider_contracts`, normalization, Pyrefly sidecar, rustc extractor, `production_query_recipe.rs`, `relational_semantic_query.rs`, `query_service.rs` and the actual Arrow/Delta providers.

Make effective Python environments/imports/stubs and Rust Cargo targets/features/compiler inputs part of source/context identity. Preserve contained untrusted compilation and explicitly label any trusted-local profile. Bind application identities to the correct provider seam; never promote provider-local indices to canonical identities.

Complete actual Tree-sitter/Ruff source/syntax coverage and Pyrefly/rustc semantic contributions. Normalize source ranges, declarations, bindings, references/imports, types/members and resolved/possible/unresolved calls into actual queryable relations. A call occurrence is not automatically a resolved target. Route the four forms through the installed DataFusion catalog and modern daemon/adapter.

Wire `tests/fixtures/pragmatic_cpg/workspace` and independently justified `expectations.json` into the existing production fixture/modern client. They cover typed Python/Rust functions, imports, direct calls, indirect uncertainty and source positions. Add precise real-answer assertions; do not derive expected answers from the output being tested. The existing public launcher attaches to a registered supervisor; fixture registration/setup can reuse the Rust integration fixture rather than invent a new production authority.

**Done:** a real mixed-language demonstration answers all first-release questions, including at least one semantic observation from each compiler/type provider. Validate the changed provider protocols/schema boundaries and public answers, not just successful process exit or nonzero row counts.

## 6. Outcome 5 — Query-relevant progress and unfinished scope

Build with outcome 4. Reuse actual processing/coverage/publication state in `provider_capability.rs`, query/status paths, lifecycle and adapter projections. Do not create an independent status authority.

Separate installed support, per-snapshot processing coverage and release-test confidence. Every response identifies snapshot and source/context revision, requested/completed scope, pending/failed/cancelled/unsupported/excluded scope, precision, freshness, reason and retry/next action where known. Scope can be language/context/file/owner/family; paginate details. A references query must disclose unprocessed scopes that might contain more matches. If invalidation has not established an exact set, return the conservative larger scope and explain the uncertainty.

Exclude invalidated semantic facts from current-source results. Explicit historical selection retains original generation/coverage; newer disk progress may be observed separately without mutating a pinned snapshot's meaning. Distinguish no-match, complete absence, partial result, unavailable family and truncation.

Extend released wire/projection fields only where required, preserve existing allocations and regenerate clients through normal tooling. Add the differential harness's runtime convergence predicate here: it must inspect relevant completed/remainder scope, never wait a fixed delay and assume completion.

**Done:** empty/partial/failed/unsupported queries and compilation failure/repair provide accurate scoped explanation through the real service. Test that missing providers cannot silently become empty complete results and stale compiler facts cannot remain current.

## 7. Outcome 6 — Live updates and quiet convergence

Depends on source/context identity, scoped progress and owned publication. Inspect watcher/repository input, dirty registry, source-wave command effects, owner replacement, provider scheduling and activation. Keep watcher/gix observations as hints; reread authoritative bytes.

Coalesce edits, reconcile add/delete/rename/context changes, conservatively invalidate affected owners or whole contexts, withdraw old facts/edges and reject obsolete provider completions by generation/context. Publish fast syntax results and slower semantic results without falsely mixing current generations. Use one coordinator and coherent snapshot selection. Fine-grained dependency tracking is an optimization after conservative correctness.

Use `tooling/product/corpus.py` and `edits.json` with actual capture/rebuild/convergence adapters. Compare converged incremental answers to clean rebuilds; preserve semantic identity relationships, positions, ordering and coverage. Include negative dependencies, deletion, rename, failed compilation/repair and configuration changes. Add obsolete completion and cancellation races at the actual boundary; use a controllable test seam only if the public API cannot reliably trigger them.

**Done:** one running daemon observes Python/Rust edits, exposes accurate pending scope, removes obsolete relationships, converges after quiet and equals clean reconstruction. Existing queries retain their pinned snapshot. No restart-per-edit shortcut qualifies.

## 8. Outcome 7 — Complete all analyses and query forms

Build capability by capability on outcomes 4–6, extending expected facts and status alongside each implementation. Detailed ONT/GEN families remain the scope authority; this grouping does not narrow them.

| Family group | Completion work and validation |
|---|---|
| Source/lexical/syntax, generated/expanded/lowered | Complete source capture, coordinates, raw/normalized types, unsupported/binary/oversized treatment and generated provenance |
| Entities, symbols, modules, types, members, calls | Complete binding/import resolution, object models, callable/type identity, ambiguity and environment/context effects in both languages |
| Python CFG and core dataflow | Replace sequential approximations with actual branch/loop/evaluation/exception control flow and correct definitions/uses/reaching facts |
| Python advanced analysis | Object/heap/memory, effects, exceptions, resource lifetimes, async/concurrency, closures/capture and program-point states with explicit precision |
| Rust MIR-derived analysis | CFG/accesses, moves/borrows/ownership/lifetimes, dataflow/memory, effects/resources/async/capture and program-point facts; compiler observations remain distinct from inference |
| Common graph and summaries | Bounded projections, SCC/reachability/dominance/post-dominance/control-dependence/loops, interprocedural summaries and convergence/unknown propagation |
| Remaining forms | FindPaths, MatchPattern, CombineResults, SummarizeFacts plus composition DAGs, objective aggregates, negative clauses, partial success, limits and deterministic ordering |
| Graph query behavior | Demand-rooted traversal, correct witnesses and cycles, bounds/cancellation, stable canonical identity; use DataFusion/native capabilities before custom extension code |
| Presentation | One canonical daemon-authored response, guarded input, authorized references/resources, pagination and modern cancellation without duplicate Python state |

Inspect `python_derived_analysis.rs`, `rust_mir_derived_analysis.rs`, `common_derived_analysis.rs`, relevant provider/query modules and per-family consumers. State algorithm invariants and precision; use examples and targeted property/differential tests where useful, rather than every technique for every family. A supported-family gap is a remaining task with a concrete cause; do not indefinitely hide it behind a generic partial label.

**Done:** every selected family and all eight forms have real behavior and meaningful corpus coverage, including partial/unknown semantics. First-release completion does not close this outcome.

## 9. Outcome 8 — Sustained operation, native maintenance and performance

Start operational essentials during earlier outcomes, then extend workloads after the first useful release. Bound all retained work/state/results, preserve task ownership and provider containment, and exercise cancellation, restart, interrupted writes, leased reads and recovery across domains.

Classify durable versus recomputable data. Reuse unchanged exact relation versions where useful; avoid rewriting all histories or persisting all intermediates. Implement safe native Delta maintenance and finite retention/reclamation protecting current/history/leased versions and any actual CDF consumer watermark. A temporary disk cap/backpressure policy supports an interactive milestone but does not complete unattended operation.

Measure real startup/reopen, query latency, edit-to-syntax including detection/debounce, semantic convergence, backlog, peak memory, retained disk and recovery. Add the missing runtime timestamps/counters needed to separate these phases. Prepared `product-bench` reports whole-scenario wall time and missing metrics honestly; it cannot claim daemon-only timings or CPG performance from the synthetic hashing benchmark. Use repeatable mixed-language small/medium/large workloads, machine/revision/settings and multiple samples. The proposed 100 ms/500 ms numbers are hypotheses until measured and deliberately selected, not frozen pass thresholds.

Optimize demonstrated bottlenecks: restore safe native projection/optimizer visibility, truthful statistics, bounded streaming/Arrow operations. Add precise invalidation when conservative rebuild cost warrants it; overlays only when measured publication latency requires them; durable CDF only for a concrete downstream consumer needing intervals/recovery. Conditional mechanisms are explicit deferred choices, not mandatory infrastructure before useful queries.

**Done:** measured workloads demonstrate sustained finite state, correct retention/reopen, effective cancellation/recovery and useful responsiveness with scoped remainder. Report workload limits rather than a universal real-time or OOM guarantee.

## 10. Consumer-first removal and final integration

Remove obsolete proof/compiler/native-resource/fallback modules as their replacements are exercised. Retain actual semantic tests and useful native fixes; retire assertions about behavior deliberately removed with a concrete replacement. Use Git/references and affected checks, without new decommission registries or historical closure replay.

Close baseline Rust/lint/behavior regressions as production code is touched. Run the affected domain gates and integrated checks for the final assembled product, including providers, wire/adapter, golden mixed-language answers, updates, reopen, failure/retention and measured operation. Do not equate passing tooling, declarations, source hashes or test names with implemented behavior. STATUS records delivered capability and remaining scope; no terminal certification machinery is reinstated.

## 11. Reconciliation of WP77–WP106

Old identifiers are navigation only. Preserve semantic and operational obligations; replace proof/process-only obligations and condition optional optimization on actual need.

| Historical work | Current disposition |
|---|---|
| WP77 compiled boundary/capability | Outcomes 2/5: ordinary typed builders, support/coverage separation; retire generalized release/proof compiler |
| WP78 source/context support | Outcomes 4–6: authoritative inventory, effective contexts and dependency gaps |
| WP79 aggregate resources | Outcomes 1/3: owned startup and reduced resource contract; no universal allocator receipt requirement |
| WP80 syntax | Outcome 4 and full-family completion in 7 |
| WP81 Pyrefly | Outcome 4 effective semantic provider, outcome 7 remaining family coverage |
| WP82 rustc containment/extractor | Outcomes 3/4/7, preserving trust and private identity seam distinctions |
| WP83 normalization | Outcome 4 actual two-language Arrow/Delta facts |
| WP84 Python CFG/dataflow | Outcome 7 actual evaluation/control semantics |
| WP85 Python advanced analyses | Outcome 7 all listed Python families |
| WP86 Rust MIR analyses | Outcome 7 all listed Rust families |
| WP87 common graph/summaries | Outcome 7 bounded graph and interprocedural algorithms |
| WP88 watcher/reconciliation | Outcome 6 live owned input loop |
| WP89 invalidation/withdrawals | Outcome 6 conservative correct invalidation; precision optimized in 8 |
| WP90 continuous publication | Outcome 6 two-speed updates and coherent snapshots |
| WP91 durability/selective histories | Outcome 8 retain/recompute by real consumer and reuse exact versions |
| WP92 native overlays | Outcome 8 conditional on measured publication need; not first-release prerequisite |
| WP93 durable CDF | Outcome 8 conditional on a concrete downstream consumer, with interval recovery |
| WP94 native maintenance amendment | Outcome 8 safe native APIs/fixes; no source-artifact certification |
| WP95 finite retention/reclamation | Outcome 8 mandatory for sustained operation, protect actual leases/consumers |
| WP96 projection optimization | Outcome 8 preserve schema/field identity and native optimizer correctness |
| WP97 library capabilities/cost | Outcomes 4/8 native mechanisms and measured improvement; no leverage survey quota |
| WP98 eight query forms | Outcomes 4/7 with composition/partial-success semantics |
| WP99 bounded graph queries | Outcome 7 correct demand/witness/limit behavior |
| WP100 canonical modern response | Outcomes 4/5/7 daemon semantics, adapter presentation |
| WP101 dependency-aware proof | Replace with outcome 2 compact records/boundary validation and outcomes 6/7 targeted differential tests |
| WP102 installed real-time product | Outcomes 4–8 real mixed-language corpus and live demonstration |
| WP103 cancellation/crash/recovery | Outcomes 1/3/6/8 with actual ownership and failure behavior |
| WP104 preregistered targets | Outcome 8 measurements and deliberate service objectives; remove preregistration/source freezes |
| WP105 decommission/regressions | Outcome 10 consumer-first cleanup with meaningful replacement tests |
| WP106 terminal certification | Outcome 10 integrated behavioral completion and honest STATUS; retire proving-chain ceremony |

## 12. Next action

Outcome 1 is complete. Continue outcome 2 by replacing generalized activation proof histories with compact candidate validation records, then remove remaining generalized proof execution and resource consumers. Resolve engineering choices directly in this editable plan when new evidence matters. Another design, plan review or status-artifact cycle is not a prerequisite.
