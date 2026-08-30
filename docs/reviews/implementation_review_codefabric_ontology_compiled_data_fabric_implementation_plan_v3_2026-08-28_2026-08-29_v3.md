---
artifact: implementation-review
plan_path: docs/plans/codefabric_ontology_compiled_data_fabric_implementation_plan_v3_2026-08-28.md
verdict: changes-required
version: v3
date: 2026-08-29
status: complete
---

# Implementation Review: CodeFabric ontology-compiled data fabric plan v3 — follow-up completeness

## Provenance and Review Scope

This independent, read-only follow-up review assesses whether the actions requested by the v2
implementation review are complete. It uses the v2 report only as the stable `IR-001` through
`IR-011` finding ledger; every closure decision below was re-derived from the current code,
accepted v5 design, immutable v3 plan, execution state, proving ancestry, production callers,
and fresh checks.

The accepted plan baseline is `71a888fed8aae660f97a8bc420f04a039f5aacae`. The prior-review
HEAD `6f6e27987bd97cad44c91e6a238104ede9e6ffe6` and the state-recorded proving commits are
ancestors of review HEAD `9e03e81082234bdf9c44a56eec1a5e9c15d701ef`. The remediation from
the prior-review HEAD to review HEAD changes 61 files with 4,271 insertions and 686 deletions.
Execution state declares WP18-WP27, M05-M08, and DB07-DB12 complete. Fresh `just plan-status`
accepts the schema, declared-input freshness, proving ancestry, and completion metadata.

The worktree was clean at review start. Concurrent documentation work later introduced an
untracked `docs/library_ref/full_data_fabric_design_principles_v2.md`; it is outside this review,
was not modified, and is excluded from diff-hygiene conclusions. Production code, tests, plans,
designs, and execution state were not changed by this review. This report is its only authored
repository change.

The review used hidden-aware textual and structural searches with zero skipped Rust source files,
production call-path tracing, DataFusion 55 `TableProvider`, `ScanArgs`, `StatisticsRequest`, and
subquery traversal contracts, delta-rs exact-version/statistics behavior, and three independent
read-only lenses covering semantic correctness, activation/epoch authority, and library/resource
proof. No performance claim was assessed or required.

## Executive Summary

The follow-up is substantial but incomplete. IR-002, IR-009, and IR-010 are closed. The
remediation now opens exact Delta versions, validates and lowers a bounded relational graph,
digests the complete activation submission, traverses embedded subqueries, installs
content-addressed program packages, releases the operational-store mutex during candidate proof,
persists result-authority pins, and passes the focused cutover/recovery/zero-state suite.

Two certification blockers remain:

1. Authored rule operations and operands still do not causally generate the executable graph;
   the model compiler contains an operation enum, exact operation census, and hand-built graph
   for every current validation family. Its green “causality” test accepts decode rejection of a
   post-generation tamper as proof of authored-to-plan causality.
2. Ordinary fact publication and overlay rebase still advance the serving pointer outside the
   D-15 classifier/opaque-permit route. They can bind new exact providers to a copied old
   `exact_table_set_identity`, while validating with the current binary package rather than the
   active retained epoch.

Six major findings also remain: generated domain-transition policy is not causally enforced;
activation cancellation and crash-spill ownership are incomplete; leases do not reconstruct a
complete executable/result-schema epoch; certification oracles miss those production paths;
some exact graph-shape/alias proof remains incomplete; and `Id16ContractProvider` drops
DataFusion 55 `StatisticsRequest` values before the repaired outer wrapper can forward them.

## Verdict

**Changes required.** The follow-up actions do not establish complete implementation of the
accepted v3 plan. IR-001 and IR-003 remain blockers. IR-004, IR-005, IR-006, IR-007, IR-008, and
IR-011 remain major. IR-002, IR-009, and IR-010 are closed. M06-M08 and DB07-DB10 therefore are
not independently supported as complete, despite healthy execution-state metadata and green
focused gates.

## Gate and Evidence Assessment

| Evidence | Fresh result | Assessment |
|---|---:|---|
| baseline, prior-review, and proving ancestry | pass | required commits exist and are ancestors of review HEAD |
| `just plan-status` | pass | state schema, inputs, and proving metadata are healthy; semantic completion is reviewed separately |
| `just artifacts-check` | pass | artifact schemas and plan/state linkage validate |
| `ontology-program-causality-check` | pass, 2/2 | selected generated-package mutations pass; source-IR-to-regenerated-plan causality is not exercised |
| `id-domain-plan-enforcement-check` | pass, 6/6 focused cases | direct and representative embedded-subquery checks pass; generated transition cells are not behaviorally exhausted |
| candidate Delta/receipt checks | pass, 2/2 and 4/4 | exact-version candidate execution and receipt closure remain sound |
| `ontology-runtime-resource-check` | pass, 4/4 | selected bounds and serving cancellation pass; no production activation cancel owner or real spill/kill/restart proof |
| activation route/recovery checks | pass, 1/1 integration and 4/4 unit | durable admin path works; ordinary rebase remains an alternate pointer authority |
| `result-authority-lease-check` | pass, 3/3 | package identities persist; complete provider/result-schema epoch reconstruction is not proved |
| `ontology-datafabric-integration-check` | pass, 4/4 | cutover, predecessor failure, post-cutover publication, and named restart scenario pass |
| `ontology-datafabric-legacy-zero-state-check` | pass | 459 textual candidates and 215 Rust files scanned with zero skipped; its patterns miss equivalent surviving authority |
| `gate-filter-census` | pass | the v2 stale-census blocker is fixed |
| `env RUSTC_WRAPPER= just ci-pr` | incomplete by reviewer interruption | root suite reached 495/495 plus doctests and major four-domain/governance gates; a later redundant long clean-rebuild selector was stopped, so this is not claimed as a completed aggregate result |

Initial sandboxed attempts failed on denied `sccache` or UDS access. Identical focused recipes
were rerun with `RUSTC_WRAPPER=` and, where needed, unrestricted UDS access; those passing reruns
supersede the harness-only failures.

## Finding Index

| ID | Follow-up status | Severity | Dimension | Summary |
|---|---|---|---|---|
| IR-001 | partial, open | blocker | architecture / correctness | authored rule records still do not generate executable semantics |
| IR-002 | closed, no regression | prior blocker | correctness / integration | candidate closure executes against exact Delta providers |
| IR-003 | partial, open | blocker | integrity / operations | ordinary publication remains an alternate pointer authority with stale table pins |
| IR-004 | partial, open | major | correctness / governance | generated domain-transition policy is decoded but not totally enforced |
| IR-005 | partial, open | major | reliability / resources | store locking improved; activation cancellation and crash-spill ownership remain incomplete |
| IR-006 | partial, open | major | compatibility / recovery | retained packages do not reconstruct a complete executable/result-schema epoch |
| IR-007 | partial, open | major | assurance / governance | red census is fixed, but named oracles still bypass material production paths |
| IR-008 | partial, open | major | correctness / resilience | graph safety improved; exact alias closure and required negatives remain incomplete |
| IR-009 | closed | prior major | integrity / idempotence | replay is bound to the complete canonical submission |
| IR-010 | closed | prior blocker | correctness / security | analyzer and serving governance traverse embedded subqueries |
| IR-011 | partial, open | major | library use / planning | exact statistics survive, but Id16 downconverts structured scans |

## Findings

### IR-001 — Authored rule records still do not generate executable semantics

**Severity:** blocker  
**Dimension:** architecture / correctness  
**Design and plan references:** TI-10, TI-11; D-10, D-11, D-13; WP19-WP20, WP23, M05-M06,
DB07

**Evidence.** Package decoding now closes `execution_phase` to `candidate_validation` and
`semantic_analysis` and rejects incompatible roots (`src/ontology_relational_program.rs:201-215`).
That fixes the prior arbitrary-phase defect. The authority chain is still not data-driven:
`OntologyRuleOperationKind` enumerates every current operation in
`src/bin/codefabric_model/schema_driver.rs:513-547`; the schema driver requires that exact
hard-coded census at `:1376-1427`; and `build_graph` selects those enum values and manually
constructs every scan, predicate, join, aggregate, and projection in
`src/bin/codefabric_model/schema_driver/ontology_graph.rs:381-402` and `:533-1248`.
`ordered_operands` affect `rule_semantics_identity`, but do not drive graph construction.

The focused causality helper mutates the already-generated package, then treats decode rejection
as success (`src/ontology_executor.rs:760-782`). Its operand mutations do not regenerate the
paired graph from changed Schema Contract IR (`:946-975`). The governance rule bans selected
runtime match shapes but not `rule(ir, OntologyRuleOperationKind::...)` plus manual graph
construction (`rules/ontology-operation-dispatch-generic.yml`).

**Failure mode.** A valid authored semantic change is not sufficient to change the executable
plan. Adding an operation or altering operand meaning requires a Rust compiler edit; a package
tamper is rejected because identities disagree, which proves integrity but not causality. This
contradicts TI-11 and makes DB07 a spelling-based rather than behavioral decommission.

**Required remediation.** Make typed authored operation and operand relations sufficient input
to one generic graph compiler. Remove the exact Rust operation census and operation-specific
graph-building branches. Preserve the integrity identity, but separately prove that a valid
source-IR mutation followed by model regeneration changes the intended plan, result, or typed
diagnostic.

**Focused re-test.** Extend `ontology-program-causality-check` with data-only Schema Contract IR
mutants for every supported operation/operand, rerun the model compiler, and require changed
logical plans and semantic results. Add a correctly modeled new rule without Rust edits and a
structural zero-state check covering equivalent enum/call-site dispatch.

### IR-002 — Exact Delta candidate execution remains closed

**Status:** closed; no regression  
**Design and plan references:** LD-13; WP23, WP26

Exact table handles reopen with the requested delta-rs version and verify it after load
(`src/fabric/snapshot_catalog.rs:144-176`). Candidate catalog construction preserves those
providers (`:737-770`), and candidate execution rechecks URI, version, schema, and content before
compiling and executing the closure (`src/ontology_candidate.rs:682-708`, `:733-776`). Fresh
candidate-Delta checks pass. IR-011 concerns later planning metadata, not exact-version authority.

### IR-003 — Ordinary publication remains an alternate pointer authority

**Severity:** blocker  
**Dimension:** integrity / operations  
**Design and plan references:** TI-14-TI-16; D-14, D-15; WP24-WP26, M07-M08, DB09-DB10

**Evidence.** The raw `commit_fact_snapshot` method is now private
(`src/snapshot_runtime.rs:354-383`), which is a real improvement. Public
`commit_ordinary_fact_snapshot` still reaches it after comparing a caller-carried
`ResultAuthorityPin` to the active row (`:284-343`). Overlay rebase builds new exact providers
and a new snapshot, then calls that public route (`src/fabric/overlay.rs:1752-1820`).

`ServingSnapshotCandidate::build` correctly overwrites publication and table evidence from the
new provider catalog but retains the separately supplied result-authority pin
(`src/snapshot_runtime.rs:107-148`). Neither candidate binding nor serving session construction
recomputes `exact_table_set_identity` from that provider set (`:156-229`;
`src/fabric/serving.rs:571-589`). The ordinary route therefore proves only that the copied pin
equals the old active pin. In addition, ordinary publication validation builds the current
binary package rather than resolving the active retained epoch
(`src/fabric/publication.rs:806-837`).

**Failure mode.** A fact publication can serve new exact table versions under an old exact-table
authority identity, with validation semantics supplied by a newer current binary. This is both
an alternate pointer mutation route and a stale-authority labeling defect. The green route and
zero-state checks do not inspect `commit_ordinary_fact_snapshot` or cross-bind the pin to actual
providers.

**Required remediation.** Route ordinary candidates through D-15's common classifier and a
nonconstructible permit into one durable kernel. Derive candidate class from the full predecessor
manifest difference, validate with the active retained policy/package, and recompute/cross-bind
the exact-table identity and resulting authority to the actual provider tuple before pointer CAS.

**Focused re-test.** Require a changed fact publication with a copied predecessor exact-table pin
to reject; require the classified/permit route to succeed with a recomputed pin; prove binary
package drift cannot change the active policy; and structurally prove that only the permitted
kernel can move the serving pointer.

### IR-004 — Generated domain policy is not total runtime authority

**Severity:** major  
**Dimension:** correctness / governance  
**Design and plan references:** TI-12; D-12; WP22-WP23, M06, DB08

**Evidence.** The remediation generates and package-binds expression operations, a 20-cell
state/effect transition table, and comparison pairs. `DomainOperationPolicy::from_package`
validates the 37-expression census and total transition shape
(`src/domain_conformance.rs:53-192`). Capability construction and plan sealing are materially
narrower.

The analyzer retrieves each transition at `src/domain_conformance.rs:587-605`, but consults its
`allowed` value generically only for `Opaque`. Declared false cells for `Domain/Produce` and
`Domain/ExplicitErase` are not enforced through the table; other behavior remains encoded in
handwritten expression branches. `odf_generated_domain_policy_total_truth_table` merely reads
and asserts the 20 values (`:1081-1113`), while the named session truth-table exercises one
same-domain acceptance and one cross-domain rejection
(`src/governed_session.rs:715-738`).

**Failure mode.** Generated policy can change without causally changing analyzer behavior, so the
package is not the single enforcement authority and a supposedly forbidden non-opaque
state/effect cell can be accepted by handwritten logic.

**Required remediation.** Apply `allowed` and declared output state for every state/effect cell,
leaving handwritten code only to infer inputs and enforce expression-specific structural
preconditions. Remove duplicate semantic decisions from branches.

**Focused re-test.** Execute plans covering all 20 generated transitions, mutate each valid
policy cell through a resealed package, and require analyzer behavior to follow the mutation.
Keep the pinned expression/plan census and bypass negatives.

### IR-005 — Activation cancellation and crash-spill ownership remain incomplete

**Severity:** major  
**Dimension:** reliability / resources  
**Design and plan references:** TI-18; D-11, D-15; WP20-WP22, WP26, M06-M08

**Evidence.** The daemon now releases the global `OperationalStore` mutex before asynchronous
proof and reacquires it only for commit (`src/daemon.rs:631-655`). Serving has real in-flight
cancellation/deadline and reservation-release coverage. Private `0700` spill directories and
dead-PID reconciliation also exist.

Activation nevertheless creates a local `Cancellation` at `src/daemon.rs:645-650`; no production
path owns or calls `cancel()` on that handle. The sequential admin loop awaits proof, so client
disconnect, daemon stop, or drain cannot signal the in-flight activation. Spill recovery tests
construct a fake dead-PID directory rather than force a real spilling subprocess to die. The
named “no mutation” candidate resource test does not open an operational store.

**Failure mode.** An expensive activation proof can continue after its request owner disappears,
and real process death during spill has no end-to-end cleanup proof. The resource contract is
present but not owned across the production lifecycle.

**Required remediation.** Give activation a request/task-owned cancellation token linked to
disconnect, shutdown, deadline, and task drop; guarantee termination and resource release before
commit. Exercise actual spilling in an isolated process and reconcile after forced death.

**Focused re-test.** Cancel a live daemon activation from each production owner boundary and
assert bounded termination, released memory/spill, and unchanged durable candidate/decision/
pointer state. Kill a genuinely spilling child and verify safe startup cleanup.

### IR-006 — Retained packages do not reconstruct a complete executable epoch

**Severity:** major  
**Dimension:** compatibility / recovery  
**Design and plan references:** TI-16-TI-17; D-15, D-16; WP25-WP26, M07-M08, DB10

**Evidence.** Content-addressed package installation and authenticated load are real
(`src/ontology_program.rs:412-550`), activation installs before commit
(`src/ontology_activation.rs:192-209`), and `ServingQuerySession::from_lease` resolves the
retained package and derives its compiler/policy (`src/fabric/serving.rs:535-589`).

Recovery remains dependent on ambient state. `SnapshotLeaseManager::rehydrate` and
`ServingSnapshotRuntime::recover` require caller-supplied in-memory
`Arc<ServingSnapshotCandidate>` values (`src/snapshot_runtime.rs:545-596`, `:898-1004`); their
production call census is empty and tests carry/rebuild candidates with current code. The
integration restart case verifies durable lease metadata but does not reconstruct and execute
simultaneous old/new query sessions after a real restart.

`ResultAuthorityPin` also omits D-16's result-contract-set identity, schema generation/version,
and public-wire contract version (`src/snapshot.rs:120-131`). Runtime result-field projection
continues through the current generated schema registry, while lease construction checks only
program, function, policy, phrase/query-form, and checksum identities. Forward rollback builds
the current binary package rather than selecting the retained predecessor package.

**Failure mode.** After binary or schema evolution, an old lease can execute current result-field
semantics under an unchanged old pin; startup cannot reconstruct the provider/session graph from
durable identity alone; and “rollback” cannot faithfully reactivate predecessor semantics.

**Required remediation.** Persist a complete content-addressed epoch manifest covering exact
providers/tables, program, functions, policy, result schema/contract set, checksum, wire version,
and extension registry. Reconstruct snapshots and sessions solely from durable records and
retained artifacts, and make forward rollback select that predecessor epoch.

**Focused re-test.** Start two epochs with intentionally different result schemas/policies,
restart in a binary whose current generated schema differs, rehydrate from identities without
retained Arcs, execute both leases, and prove epoch-specific results/checksums. Then forward-
activate the retained predecessor and repeat.

### IR-007 — Certification is green-shaped but not complete

**Severity:** major  
**Dimension:** assurance / governance  
**Design and plan references:** TI-10-TI-19; WP27, M08; plan final gate matrix

**Evidence.** The v2 stale gate-filter census is fixed, and the focused recipe suite is green.
The long aggregate run also completed the full 495-test root suite, doctests, stable graph,
extractor, sidecar, adapter, governance, artifact, plan, policy, and multiple packet selectors
before the reviewer stopped a repeated long clean-rebuild selector. This is strong broad
regression evidence, but not semantic closure.

Named oracles still bypass the failure surfaces documented above: the causality test mutates a
generated package rather than authored IR; the forwarding spy bypasses `Id16ContractProvider`;
route/zero-state checks ignore the ordinary pointer path; policy “truth table” reads rows rather
than executing all cells; restart tests retain or rebuild candidate authority; resource tests do
not own production activation cancellation or kill a real spiller. Release certification also
checks recipe registration rather than independently executing and interpreting all claimed
semantics.

**Failure mode.** A fresh proving commit can be entirely green while the mechanisms named by
IR-001, IR-003, IR-004, IR-005, IR-006, IR-008, and IR-011 remain incomplete. State completion
therefore overclaims what the evidence proves.

**Required remediation.** Repair the mechanisms first, then make each named oracle cross the
real production boundary and falsify the corresponding defect. Reconcile plan state only after
the complete final matrix passes at one proving commit and an independent review confirms the
result.

**Focused re-test.** Add the exact causal, pointer/pin, policy, cancellation, spill-kill,
restart/rollback, graph-shape, and Id16 structured-scan probes specified by their findings; run
WP19-WP27, M06-M08, DB07-DB10, zero-state, plan/artifact, and repository gates at committed HEAD.

### IR-008 — Graph safety improved, but exact shape closure remains incomplete

**Severity:** major  
**Dimension:** correctness / resilience  
**Design and plan references:** TI-11, TI-13-TI-14; D-10-D-11, D-13-D-14; WP20, WP23, M06

**Evidence.** The relational decoder now enforces plan/expression acyclicity, reachability,
count/depth bounds, edge roles/arity, child aliases, and calculation closure
(`src/ontology_relational_program.rs:438-524`, `:647-716`). This closes the stack-overflow and
most dead-row risk from v2.

`output_alias` remains nullable in generated project/aggregate relations; decode rejects only an
empty present alias, and lowering accepts `None` (`src/ontology_relational_program.rs:600-613`,
`:916-924`, `:974-995`) even though current generators always provide aliases for those output
roles. Focused negatives cover an expression cycle and an illegal child alias, but the complete
v2 fixture set—missing required alias, dead node/expression, unused calculation, extra role,
depth/count boundary, and positive shared DAG—is not independently present.

**Failure mode.** A digest-valid graph can omit an alias that the current authored profile treats
as deterministic identity material, and the named closure check does not demonstrate every
required rejection/positive sharing case.

**Required remediation.** Make alias presence/absence exact per node and output role, not merely
nonempty when present, and complete the adversarial graph fixture matrix.

**Focused re-test.** Add resealed missing/extra alias, dead row, unused calculation, extra role,
exact count/depth limit, over-limit, and shared-DAG fixtures to
`ontology-candidate-receipt-check` or a narrower graph-closure recipe.

### IR-009 — Complete activation submission replay is closed

**Status:** closed  
**Design and plan references:** TI-14, TI-16; D-14-D-15; WP24-WP26

The complete `OntologyCandidateSubmission` is serialized and JCS-canonicalized before replay
identity is derived (`src/ontology_activation.rs:42-56`, `:165-173`). The digest is persisted and
compared before completed replay in `src/operational_store.rs`. The integration test accepts a
canonically equivalent reserialization and rejects same-key changes to rollback retention,
source blobs, manifest body, and publication
(`tests/integration/ontology_datafabric_cutover.rs:620-679`).

### IR-010 — Embedded-subquery governance is closed

**Status:** closed  
**Design and plan references:** TI-12; D-12; WP22, DB08

Domain analysis uses `LogicalPlan::apply_with_subqueries`
(`src/domain_conformance.rs:335-352`), and serving provider/function/extension governance does
the same (`src/fabric/serving.rs:2114-2184`). Under DataFusion 55 this traversal visits embedded
subquery plans. Tests cover scalar, `EXISTS`, `IN`, and correlated subqueries for domain checks,
and unauthorized providers in scalar/`EXISTS`/`IN` subqueries for serving. A shared helper and
some deeper nested-function/extension cases would improve maintenance, but no current bypass was
found.

### IR-011 — Id16 still drops structured statistics requests

**Severity:** major  
**Dimension:** library use / planning  
**Design and plan references:** LD-13; WP23, WP26; DataFusion 55 CAT-06/CAT-07 and delta-rs exact
provider/statistics contracts

**Evidence.** Exact Delta statistics are now retained and authenticated instead of replaced, and
`EffectiveStatisticsProvider` plus `OverlayIdentityProvider` explicitly forward `ScanArgs`
(`src/fabric/snapshot_catalog.rs:254-317`, `:330-374`, `:747-770`). The final exact provider is,
however, first wrapped by `Id16ContractProvider` (`src/fabric.rs:877-918`). Its `TableProvider`
implementation defines legacy `scan` and `statistics`, but not `scan_with_args`
(`src/fabric.rs:995-1041`). DataFusion 55's default `scan_with_args` downconverts to `scan`,
preserving projection/filter/limit while discarding `StatisticsRequest`.

The green structured-forwarding test constructs `EffectiveStatisticsProvider` directly over a
spy (`src/fabric/snapshot_catalog.rs:1728-1790`), bypassing `Id16ContractProvider`; it therefore
cannot detect the production loss. The overlay-effective provider's explicit refusal to answer
query-aware statistics for a rewritten plan is honest and is not this defect.

**Failure mode.** Query-aware statistics requests never reach the exact Delta provider through
the production wrapper chain, so DataFusion planning loses supported information despite outer
forwarding and retained table-level statistics.

**Required remediation.** Implement `scan_with_args` on `Id16ContractProvider`, transform only
the storage-typed filters it owns, preserve every other structured argument including statistics
requests, and reattach the Id16 output schema to the returned plan.

**Focused re-test.** Place a structured-scan spy beneath `Id16ContractProvider`, then exercise the
complete production wrapper stack and require identical projection, transformed filters, limit,
and `StatisticsRequest` values plus exact statistics.

## Outcome and Invariant Matrix

| Target | Status | Current evidence |
|---|---|---|
| TI-10 Arrow-native compiled authority | **partial** | Arrow program/package exists; operation-specific compiler authority remains |
| TI-11 DataFusion causal execution | **not met** | authored operations/operands do not generate the executable graph |
| TI-12 fail-closed semantic planning | **partial** | subquery traversal is fixed; generated transition policy is not totally causal |
| TI-13 semantic self-description | **partial** | closure is real, but a new rule still requires Rust graph/compiler edits |
| TI-14 candidate-bound proof | **partial** | candidate receipts improved; ordinary snapshots can carry stale exact-table authority |
| TI-15 one activation command | **not met** | ordinary publication/rebase remains a public pointer route without classifier/permit |
| TI-16 durable idempotence/recovery | **partial** | submission replay is closed; pointer/session reconstruction still needs ambient candidates |
| TI-17 lease-scoped compatibility | **not met** | result-schema/wire/provider epoch is not fully pinned or reconstructible |
| TI-18 bounded shared execution | **partial** | bounds and serving cancellation exist; activation cancel and crash spill are incomplete |
| TI-19 accountable decisions | **supported with caveat** | observation/decision separation is sound; alternate pointer authority bypasses acceptance exclusivity |
| DB07 duplicate semantic/phrase authority | **not met** | equivalent operation census and manual graph authority remain in the model compiler |
| DB08 governed-execution bypasses | **not met** | generated transition policy is not the total behavioral authority |
| DB09 activation/proof duplicates | **not met** | ordinary fact commit is a second pointer authority |
| DB10 global result/self-authorization | **not met** | complete result schema/wire/provider epoch remains current-binary or caller supplied |
| DB11 temporary comparison authority | **supported** | no live temporary comparison authority found in reviewed implementation scope |
| DB12 obsolete master root/wording | **supported** | no live obsolete master ownership found in reviewed implementation scope |

## Architecture and Doctrine Assessment

The accepted design remains valid. Native DataFusion relational plans, analyzer hooks,
`apply_with_subqueries`, bounded runtimes, exact delta-rs snapshots, Arrow IPC contracts,
content-addressed packages, SQLite CAS, and lease-local dispatch are the correct architectural
direction. No redesign, custom physical operator, custom UDF, or new storage transaction layer
is warranted.

The remaining defects are consolidation failures. Manual operation graph construction violates
declarative single-source and executable-model doctrine. The ordinary pointer route violates one
mutation authority and least privilege. A decoded-but-noncausal transition table and current-
binary result schemas violate generated authority and immutable epoch semantics. Unowned
cancellation violates explicit lifecycle. Green oracles that bypass the relevant wrapper or
production caller violate executable-governance doctrine.

## Library Leverage Assessment

Correct library use to preserve:

- DataFusion 55 native `Expr`/`LogicalPlanBuilder`, anti/semi joins, aggregates, analyzer rules,
  `apply_with_subqueries`, execution streams, memory pools, and disk manager;
- delta-rs exact `with_version` reopen and loaded-version verification;
- authoritative Delta provider statistics with honest overlay precision;
- Arrow 59 extension metadata and deterministic IPC/canonical row encodings; and
- application-owned activation, identity, and cross-table SQLite governance.

Remaining underuse is narrow and concrete: authored relations do not replace custom graph
construction; `Id16ContractProvider` does not implement DataFusion 55 structured scan forwarding;
and runtime cancellation is not connected to the production activation owner. The fixes should
use the existing pinned APIs rather than introduce replacement abstractions.

## Legacy and Decommission Assessment

| Batch | Assessment | Surviving authority or contradiction |
|---|---|---|
| DB07 | fail | operation enum/census and manual per-family graph construction remain equivalent semantic authority |
| DB08 | fail | generated transition cells do not totally decide analyzer behavior |
| DB09 | fail | ordinary fact publication/rebase reaches pointer mutation outside the classified permit route |
| DB10 | fail | complete provider/result-schema/wire epoch is not lease-pinned and restart-reconstructible |
| DB11 | pass on reviewed scope | no live temporary dual-comparison authority found |
| DB12 | pass on reviewed scope | obsolete master root/wording remains absent from live implementation authority |

The hidden-aware zero-state checker is valuable but presently proves selected spellings and call
shapes, not unique replacement authority. It must be expanded only after the behavioral routes
above are removed or consolidated.

## Test and Operational Assessment

The current suite is broad and mostly healthy. It credibly proves exact-version candidate reads,
canonical complete-request replay, representative nested-subquery rejection, SQLite CAS/replay,
content-addressed package integrity, selected resource bounds, and cutover integration. The full
root suite passed 495 tests during review, and focused plan recipes were green.

Its decisive weakness is oracle placement. Several tests instantiate an outer wrapper over a
spy, inspect generated rows without executing their policy, retain in-memory candidates across
“restart,” or mutate generated bytes without regenerating from authored input. Those tests can
remain as local checks, but their recipe names and certification role must be narrowed until
production-boundary falsification exists.

No comparative performance evidence was required. The resource findings concern termination,
cleanup, and durable-state correctness, not throughput.

## Plan Deviations and Diff Hygiene

The remediation diff is concentrated on the plan's ontology, data-fabric, activation, serving,
governance, and state surfaces. Concurrent documentation changes are explicitly excluded and
untouched. No unrelated production mutation or destructive action was performed by the review.

Material deviations that should have prevented final state closure are:

1. authored rule records remain descriptive/identity input around a hand-built Rust graph;
2. ordinary publication bypasses the common activation classifier/permit and can carry a stale
   exact-table pin;
3. generated transition policy is not the total runtime decision table;
4. activation cancellation and crash-spill cleanup lack production ownership/proof;
5. restart and rollback do not reconstruct a complete provider/result-schema epoch;
6. graph alias closure and its required adversarial matrix remain incomplete; and
7. the Id16 wrapper still downconverts DataFusion 55 structured scans.

## Required Remediation Order

1. **Close the pointer/authority bypass (IR-003, IR-006).** Consolidate all candidate classes
   behind the D-15 classifier/permit, bind actual exact providers, and define the full durable
   epoch/result-schema authority.
2. **Finish authored-to-executable causality (IR-001).** Replace per-operation graph construction
   with the generic typed compiler and prove valid source-IR mutations through regeneration.
3. **Make policy and graph closure exact (IR-004, IR-008).** Enforce every generated transition
   and complete node/edge/alias/limit fixtures.
4. **Complete production resource ownership (IR-005).** Connect activation cancellation and
   prove real spill/process-death cleanup with no durable semantic mutation.
5. **Preserve the full DataFusion provider contract (IR-011).** Forward `ScanArgs` through Id16
   and test the actual wrapper chain.
6. **Rebuild certification evidence (IR-007).** Move every oracle to the real production boundary,
   rerun the complete matrix at one fresh proving commit, reconcile state, then request another
   independent review.

## Focused Re-Review Scope

A subsequent review can remain focused on:

- source-IR-to-regenerated-plan/result causality and removal of equivalent operation dispatch;
- the sole classified permit-bearing pointer mutation route and exact-table cross-binding;
- generated 20-cell policy behavior and complete graph-shape negatives;
- request-owned activation cancellation and real spill/kill/restart cleanup;
- durable old/new provider, package, result-schema, checksum, wire, restart, and rollback epochs;
- Id16 `ScanArgs`/`StatisticsRequest` propagation through the production wrapper chain; and
- strengthened named oracles, DB07-DB10 zero state, state reconciliation, and one completed final
  gate matrix at committed HEAD.

IR-002, IR-009, and IR-010 need only regression verification unless those surfaces change.
