---
artifact: implementation-review
plan_path: docs/plans/codefabric_ontology_compiled_data_fabric_implementation_plan_v3_2026-08-28.md
verdict: changes-required
version: v1
date: 2026-08-29
status: complete
---

# Implementation Review: CodeFabric ontology-compiled data fabric plan v3

## Provenance and Review Scope

This independent, read-only review assesses the current implementation against the accepted
v3 plan, accepted v5 design, schema-v2 execution state, current code, legacy dispositions,
pinned-library decisions, and executable proof. The prior v2 implementation review was used
only as a defect-history locator; every conclusion below was re-derived from the current tree.

The plan baseline is `71a888fed8aae660f97a8bc420f04a039f5aacae`; review HEAD is
`db3797cd12636ea8347f5cb67dd60f40ca91cfa1`, and the baseline is an ancestor of HEAD. The
baseline-to-HEAD implementation diff contains 229 files, 50,766 insertions, and 17,399
deletions. The worktree was clean when review began. Execution state declares WP18-WP27,
M05-M08, and DB07-DB12 complete and records a distinct proving commit for every packet.

The method combined plan/design/state reconstruction, current-tree source and consumer
inspection, hidden-aware textual search, structural call-site coverage, exact Delta-provider
tracing, DataFusion plan/analyzer tracing, and fresh focused review checks. The pinned
DataFusion 55/Arrow 59 and delta-rs references were applied to distinguish library-supported
target behavior from custom substitutes. In accordance with the accepted plan's non-goals and
the owner's explicit direction, no performance baseline or performance test was run or treated
as required evidence.

The review did not modify production code, tests, plans, designs, or execution state. This
report is its only repository change.

## Executive Summary

The implementation makes material progress and corrects several defects from the v2 candidate.
It now has committed packet evidence, reproducible Arrow IPC program packaging, a governed
DataFusion session, an installed analyzer rule, opaque candidate receipts, exact-version Delta
provider construction, durable candidate/decision/activation records, an administrative
activation command, retry-safe ontology-pointer CAS behavior, and authority pins on new serving
leases. Focused named checks and plan-state trust checks all pass.

The accepted outcome is nevertheless not implemented in full. The central Arrow program is a
flat operation catalog whose executable meaning remains in operation-specific Rust validators;
the declared operands, policies, and diagnostics do not generically compile the validation
plans. Candidate “semantic closure” compares self-derived digest strings in one-row memory
tables and does not execute closure over the exact Delta providers it opens. The production
admin command activates only the ontology pointer: candidate proof and owner decision creation
remain test-only, and serving-pointer activation occurs in a separate caller-driven
transaction. The analyzer has an exhaustive enum match but not the specified domain-state and
domain-effect algebra. Resource governance lacks memory, time, cancellation, and spill
enforcement. Legacy leases have no authority pin and silently default to the new checksum
version.

The green oracles therefore establish useful local mechanics but not the causal production
invariants their names claim. Completion in execution state and proving-commit ancestry are
trustworthy provenance facts; they do not overcome the implementation defects below.

## Verdict

**Changes required.** WP19-WP27, M06-M08, and DB07-DB10 must not be certified complete from
this candidate. Three blocker findings defeat the unified executable-authority, semantic
closure, and atomic production-cutover outcomes. Four major findings leave analyzer semantics,
resource governance, lease compatibility, and final assurance incomplete. The accepted v5
design remains coherent; these are implementation, integration, decommission, and proof
failures rather than evidence that the design must be reopened.

## Gate and Evidence Assessment

| Evidence | Fresh result | Assessment |
|---|---:|---|
| baseline ancestry | pass | accepted baseline is an ancestor of review HEAD |
| worktree at review start | pass | clean committed candidate |
| execution-state proving commits | pass | WP18-WP27 each name a reachable proving commit |
| `just plan-status` | pass | state schema, ancestry, declared inputs, and completion metadata are healthy |
| `just artifacts-check` | pass before report | artifact schemas and repository review inventory are structurally healthy |
| `just ontology-program-causality-check` | pass, 2 tests | phrase operands are causal; rule operands/operators are not mutated or proved causal |
| `just ontology-self-description-check` | pass, 2 tests | proves package census/resealing, not semantic closure over governed relations |
| `just ontology-activation-route-check` | pass, 1 test | proves ontology-pointer command behavior, not candidate proof or serving-pointer cutover |
| `just result-authority-lease-check` | pass, 2 tests | new-lease metadata persists; old lease executes through an unproved V2 default |
| `just ontology-runtime-resource-check` | pass, 1 test | proves output-row rejection only; no bounded runtime, deadline, cancellation, or spill |
| production proof/decision call-site census | fail | all candidate-proving and owner-decision construction/persistence callers are test-only |
| normalized program-relation census | fail | no scan/filter/project/join/aggregate/set/expression node-and-edge relations exist |
| exact Delta closure trace | fail | providers are built and inspected, then discarded before closure execution |
| legacy/decommission structural scan | partial | retired names are absent, but equivalent handwritten semantic and pointer authorities remain |

The packet runner and named recipes correctly enforce selector presence and ordinary test
success. The weakness is semantic: the selected tests often prove that metadata exists, is
digest-bound, or can be persisted, while the accepted design requires that the metadata
causally controls production planning, execution, and activation.

## Finding Index

| ID | Severity | Dimension | Summary |
|---|---|---|---|
| IR-001 | blocker | architecture / correctness / library use | Arrow program metadata does not generically compile or control validation |
| IR-002 | blocker | correctness / integrity / integration | candidate closure compares self-derived digests instead of exact Delta semantics |
| IR-003 | blocker | operations / integrity / security | production proof, accountable decision, and serving cutover are not one owner route |
| IR-004 | major | correctness / library use | analyzer census is exhaustive but required domain lattice/effects are absent |
| IR-005 | major | reliability / resources / operations | resource envelope is output-only and the DataFusion runtime remains unbounded |
| IR-006 | major | compatibility / correctness | legacy leases silently select V2 rather than a lease-pinned V1 authority |
| IR-007 | major | tests / governance / legacy | final oracles and zero-state checks accept non-causal replacement authorities |

## Findings

### IR-001 — Arrow program metadata does not generically compile or control validation

**Severity:** blocker

**Dimension:** architecture / correctness / library use

**Design and plan references:** design D-10 and D-11, TI-10 and TI-11; plan WP19, WP20,
M05, M06, and DB07.

**Evidence:** the package generator emits a flat `program.rule_operation` relation with an
operation kind and repeated operands (`src/bin/codefabric_model/schema_driver.rs:4039-4126`). A
source/test census finds none of the design's normalized relational-plan and expression forms:
scan, filter, project, join, aggregate, set, column, literal, binary, call, case, cast, plan
edge, or expression edge.

`OntologyProgramCompiler::validation_operations` verifies catalog strings and converts
`calculation.native_operation` into `NativeValidationOperation`
(`src/ontology_executor.rs:587-635`). It does not lower each operation's ordered relation and
column operands, policy, or diagnostic into a `LogicalPlan`. Execution then matches
`lowered.native` and calls operation-specific functions
(`src/ontology_rules.rs:541-591`). Those functions independently own table codes, field names,
join shapes, cardinality rules, owner rules, and diagnostic strings; for example,
`validate_primary_keys` and `validate_governed_codes` construct their own plans
(`src/ontology_rules.rs:42-135`). `ExternalReferential` is an explicit no-op because a separate
compiler still owns that behavior (`src/ontology_rules.rs:558-562`).

The named causality recipe says it mutates compiled operators and phrase operands, but its two
selected tests only execute phrase-operand variation. The rule portion asserts that fields are
nonempty and operand ordinals are ordered (`src/ontology_executor.rs:759-784`); no rule
operator or operand is changed and no plan/result change is observed.

**Failure mode and consequence:** the Arrow package is descriptive metadata rather than the
single executable semantic authority. Changing a relation/column operand, policy, diagnostic,
or adding an operation can leave runtime validation unchanged. New ontology operations still
require edits to the Rust dispatcher and custom functions, defeating ontology-driven additive
extensibility and permitting metadata/behavior drift.

**Required remediation:** emit the accepted normalized node/edge program relations, bind and
type-check them against governed schemas and the calculation catalog, and lower them through
one generic DataFusion compiler. Every program field that claims semantic authority must either
change the generated `Expr`/`LogicalPlan`, diagnostic, receipt identity, or be removed. Delete
the operation-specific validation dispatch and parallel handwritten rule census.

**Focused executable re-test:** mutate every supported operation kind and every operand field
independently and require a corresponding plan, result, diagnostic, or receipt change. Add an
additive operation using only data/package changes. Strengthen structural governance to reject
`match lowered.native`, `NativeValidationOperation`, and operation-specific validator
functions, then rerun the package-reproducibility, calculation-catalog, causality, relational
closure, and governance selectors.

### IR-002 — Candidate closure compares self-derived digests instead of exact Delta semantics

**Severity:** blocker

**Dimension:** correctness / integrity / integration

**Design and plan references:** design D-13 and D-14, TI-13 and TI-14; plan WP23, M06,
and DB08.

**Evidence:** candidate construction derives `expected_bindings` from the same package,
publication, session, and exact-table metadata under review, then initializes
`observed_bindings` as an exact clone (`src/ontology_candidate.rs:636-706`). Each “closure” plan
is a one-row `MemTable` containing family ID, observed identity, and expected identity, filtered
only on string inequality (`src/ontology_candidate.rs:477-507`). `execute` runs one such digest
comparison per bootstrap member (`src/ontology_candidate.rs:725-765`). It does not compile or
execute joins/anti-joins across governed code, ontology edge, semantic type, table/column,
result, identity, phrase, rule, publication, snapshot, or plan relations.

`open_frozen_catalog` correctly constructs providers from the exact publication
(`src/ontology_candidate.rs:709-723`), but `execute` does not accept, register, or reference
that catalog. The integration helper opens the catalog, checks provider count and `file://`
URIs, discards it, and then calls the independent digest-comparison execution
(`tests/integration/ontology_datafabric_cutover.rs:355-377`). Negative tests inject a test-only
observed-identity override rather than corrupting a relation while preserving valid package
digests.

**Failure mode and consequence:** any internally resealed package whose member census and
digests are self-consistent can obtain closure receipts even when its governed semantic
relations are disconnected or contradictory. The receipts are opaque and canonical, but they
attest to identity equality rather than the design's semantic closure over the candidate's
exact Delta state.

**Required remediation:** register the exact-version Delta providers in the candidate's
governed session and compile the bootstrap-discovered closure programs into real DataFusion
plans over those providers. Derive each receipt from the exhausted plan result and exact
provider/table-version authority. Remove the injected observed-binding map as a proof
mechanism.

**Focused executable re-test:** for every closure family, corrupt a real Arrow/Delta relation,
recompute all package/member digests, and require the relevant semantic gate to fail with the
governed diagnostic. Prove that changing exact Delta content changes or invalidates the
receipt, while unrelated provider ordering does not. The test must not pass an observed digest
override.

### IR-003 — Production proof, accountable decision, and serving cutover are not one owner route

**Severity:** blocker

**Dimension:** operations / integrity / security

**Design and plan references:** design D-15 through D-17, TI-15, TI-16, and TI-19; plan WP24,
WP25, WP26, M07, M08, DB09, and DB10.

**Evidence:** structural and textual call-site coverage finds all calls to
`CandidateClosureRunner::new`/`new_for_epoch`, `persist_proved_ontology_candidate`,
`OntologyOwnerDecision::new`, and `record_ontology_owner_decision` inside `#[cfg(test)]` modules
or the integration test. No production daemon/coordinator route creates and persists a PROVED
candidate or authenticates and records the owner decision.

The production `ActivateCandidate` command accepts existing candidate and decision identities,
resolves them, and advances the ontology activation pointer
(`src/daemon.rs:629-649`). `OntologyOwnerDecision::new` explicitly assumes the caller has
already authenticated the owner, but accepts any nonempty owner string
(`src/operational_store.rs:153-189`); persistence verifies candidate state and policy, not actor
authority (`src/operational_store.rs:1580-1660`).

Serving activation is a different transaction. Public, doc-hidden
`ServingSnapshotRuntime::commit_fact_snapshot` accepts a caller-built snapshot candidate and
advances the serving snapshot pointer (`src/snapshot_runtime.rs:260-327`). The end-to-end test
demonstrates the split: it directly proves/persists the candidate and decision, invokes the
daemon command, stops the daemon, reopens the store, manually builds a serving candidate, and
then calls `commit_fact_snapshot` (`tests/integration/ontology_datafabric_cutover.rs:469-477`,
`:545-592`).

**Failure mode and consequence:** the nominal owner route cannot create the prerequisites it
requires and does not atomically make the accepted ontology authoritative for serving. A caller
can advance serving state through a separate low-level path, and a claimed owner identity is
not authenticated at the decision boundary. A successful admin response therefore does not
mean queries or newly issued leases observe the target epoch.

**Required remediation:** implement one production administrative orchestration that seals and
proves the candidate, authenticates the accountable owner decision, and atomically commits the
acceptance record, ontology authority, serving pointer, and result/function/policy authority.
Make low-level serving-pointer mutation private and require an opaque activation permit derived
from the accepted transaction. Recovery must reconstruct the committed epoch before readiness
or reply replay.

**Focused executable re-test:** drive only the production daemon/admin boundary from candidate
submission through query. Do not call the store, candidate runner, decision constructor, or
snapshot runtime directly. Assert that the one command creates exactly one proof, decision,
acceptance, ontology pointer, and serving pointer; that an immediate query/new lease sees the
target; that an unauthorized owner fails; and that process termination before/after commit
recovers and replays without another generation.

### IR-004 — Analyzer census is exhaustive but required domain lattice/effects are absent

**Severity:** major

**Dimension:** correctness / library use

**Design and plan references:** design D-12 and TI-12; plan WP22 and M06.

**Evidence:** the implemented `DomainState` is `Untracked | Exact(String) | Predicate`, not the
accepted `Domain(id) | Neutral | Bottom | Opaque` lattice. `DomainEffect` declares broad labels
but is not consumed anywhere outside its definition (`src/domain_conformance.rs:10-31`). The
DataFusion expression and logical-plan matches are syntactically exhaustive, which is useful
upgrade protection, but many required valid forms are rejected wholesale rather than evaluated
through explicit transitions: `CASE`, scalar functions, window functions, grouping sets,
unnest, higher-order functions, lambda forms, ID-bearing `IN` subqueries, and set comparisons
(`src/domain_conformance.rs:238-323`).

The named truth-table coverage exercises a small same-domain/cross-domain comparison surface;
the census test proves variant names and match completeness, not the specified state/effect
algebra.

**Failure mode and consequence:** unsafe cross-domain plans generally fail closed, but valid
plans required by the accepted target are rejected and composite expressions do not have one
inspectable semantic transition authority. Exhaustive syntax coverage is being reported as
exhaustive semantic enforcement.

**Required remediation:** implement the exact domain-state lattice and generated effect
catalog, including null/Bottom behavior, opaque values, comparison/predicate outputs, CASE and
coalesce joins, scalar/aggregate/window function contracts, subquery output binding, set
alignment, join keys, casts, and list-valued child expressions. Derive diagnostics from the
same transition record.

**Focused executable re-test:** execute the complete state-by-effect matrix across every pinned
DataFusion expression and plan family, with valid and invalid cases for CASE/coalesce, null,
functions, aggregates, windows, joins, IN/subquery, set comparison, unions, casts, and nested
list children. Retain the compile-time enum census as upgrade evidence, not semantic proof.

### IR-005 — Resource envelope is output-only and the DataFusion runtime remains unbounded

**Severity:** major

**Dimension:** reliability / resources / operations

**Design and plan references:** design D-18 and TI-18; plan WP21, M06, and M08.

**Evidence:** `GateResourceEnvelope` contains only maximum output rows, bytes, batches, and
checksum-encoding bytes (`src/ontology_gate.rs:16-46`). It has no memory-pool limit, execution
deadline, cancellation token, spill policy/path, spill cleanup contract, or bounded session
configuration. `GovernedSession::new` builds a default-feature `SessionState` and
`SessionContext` without a bounded `RuntimeEnv` or memory pool
(`src/governed_session.rs:160-202`). The focused resource test lowers only the output-row limit
and checks deterministic failure; it does not exhaust or cancel a real candidate operation or
prove durable state remains unchanged.

**Failure mode and consequence:** a candidate can consume unbounded planning/execution memory,
run without a governed deadline or cancellation path, and spill according to ambient defaults.
Output caps take effect only after resources have already been consumed. The recorded envelope
identity therefore overstates the boundedness of the execution environment.

**Required remediation:** construct each candidate/serving epoch with an explicit DataFusion
`RuntimeEnv`, bounded memory pool, deadline/cancellation propagation, controlled spill path and
cleanup, batch/partition configuration, and a versioned resource-profile identity. Keep output
and checksum limits as additional terminal bounds. No performance baseline is required.

**Focused executable re-test:** independently trigger memory exhaustion, row/byte/batch limits,
deadline, cancellation, spill creation/cleanup, and checksum budget. For each failure, prove a
stable taxonomy and zero candidate/decision/pointer mutation, then prove restart leaves no
orphaned spill authority.

### IR-006 — Legacy leases silently select V2 rather than a lease-pinned V1 authority

**Severity:** major

**Dimension:** compatibility / correctness

**Design and plan references:** design D-16 and TI-17; plan WP25, WP26, M07, M08, and DB10.

**Evidence:** the lease matrix explicitly asserts that the old lease has no result-authority
pin, while the new lease has a V2 pin (`src/snapshot_runtime.rs:1865-1958`). Serving selects a
lease pin, falls back to a manifest pin, and then defaults an absent authority to
`ResultChecksumV2` (`src/fabric/serving.rs:879-896`). Thus the unpinned old lease also executes
V2, contrary to the required coexistence of legacy V1/current authority and new V2 authority.
The lease matrix persists and counts metadata but does not execute the same query through both
leases or compare their versioned results.

**Failure mode and consequence:** an old lease can change result identity after activation
without changing its snapshot/lease identity. Compatibility and rollback behavior depend on an
ambient default rather than the immutable lease authority the design requires.

**Required remediation:** deterministically decode legacy manifests/leases to an explicit
versioned V1 authority, or migrate them to a replacement epoch before serving. Query execution
must select result, function, policy, query-form, and program authority solely from the lease's
resolved immutable pin; absence must not mean “latest.”

**Focused executable re-test:** hold simultaneous old and new leases across activation and
restart, execute identical queries through both, and assert the expected V1/V2 checksum and
result bytes plus stable function/policy/program identities. Repeat after rollback and reject
an unresolved authority instead of defaulting it.

### IR-007 — Final oracles and zero-state checks accept non-causal replacement authorities

**Severity:** major

**Dimension:** tests / governance / legacy

**Design and plan references:** plan WP27, M08, DB07-DB10, and final gate matrix; design TI-10
through TI-19.

**Evidence:** the causality recipe's description exceeds its assertions: rule metadata is only
checked for presence/order (`justfile:89-92`; `src/ontology_executor.rs:759-784`). The
self-description oracle accepts an additive resealed relation without proving its governed
semantic connections. The activation integration test stitches production transport to direct
test-only proof, decision, and serving-pointer calls. The lease oracle checks pin persistence
but never runs an old/new query. The resource oracle checks one output limit.

The structural rule intended to prohibit operation-specific dispatch bans only matches on
`operation_kind.as_str()` and `calculation_id.as_str()`
(`rules/ontology-operation-dispatch-generic.yml:1-11`); dispatch through `lowered.native`
passes. Retired symbol/name scans are green, but equivalent legacy authorities survive as
`NativeValidationOperation`, the handwritten validators, public `commit_fact_snapshot`, and
the global V2 default.

**Failure mode and consequence:** state can declare DB07-DB10 and M08 complete while the same
semantic, activation, and result-selection duplication survives under replacement names. The
final assurance packet certifies implementation shape and happy-path metadata, not the
behavioral absence of the retired authorities.

**Required remediation:** rewrite each final oracle around an independently observable causal
intervention and production owner boundary. Governance must recognize semantic equivalents,
not only retired tokens. Require source/call-site coverage plus compiler proof for negative
claims, and make every zero-state batch demonstrate that the replacement authority is both
unique and production-reachable.

**Focused executable re-test:** after remediating IR-001-IR-006, rerun the strengthened
causality, exact-Delta closure, black-box activation, old/new query, resource-failure, and
semantic zero-state checks. Deliberately reintroduce one duplicate dispatcher, pointer caller,
and ambient result default and prove each oracle fails.

## Outcome and Invariant Matrix

| Target | Status | Review evidence |
|---|---|---|
| TI-10 Arrow-native ontology program | **not met** | reproducible package exists, but normalized plan/expression relations do not |
| TI-11 generic DataFusion compiler/catalog | **not met** | flat catalog maps to a Rust enum and handwritten validators |
| TI-12 sealed exhaustive semantic analyzer | **partially met** | ingress and enum census exist; required lattice/effects do not |
| TI-13 honest compiled semantic closure | **not met** | self-derived digest equality replaces relation closure |
| TI-14 opaque candidate-bound evidence | **partially met** | receipts are opaque/canonical but attest to shallow inputs |
| TI-15 one durable activation command | **not met** | production command advances only the ontology pointer |
| TI-16 idempotent atomic cutover/recovery | **partially met** | ontology CAS replay works; serving cutover is a separate transaction |
| TI-17 lease-scoped result/function/policy authority | **not met** | new pins persist; old leases use ambient V2 default |
| TI-18 bounded execution environment | **not met** | output caps exist; memory/time/cancel/spill controls do not |
| TI-19 observation separated from accountable decision | **not met** | records are separate, but no authenticated production decision route exists |
| DB07 duplicate semantic/phrase authority | **not met** | operation-specific Rust validation remains authoritative |
| DB08 governed execution bypasses | **not met** | exact providers and program operands can be bypassed by shallow execution |
| DB09 activation/proof duplicates | **not met** | proof and serving pointer remain separately callable/test-wired |
| DB10 global result selection/self-authorization | **not met** | absent pin selects global V2; owner identity is caller-asserted |
| DB11 temporary comparison authority | supported | no live comparator path was identified in scoped code |
| DB12 obsolete design root/wording | supported | authoritative design root and wording cutover are structurally present |

## Architecture and Doctrine Assessment

The implementation follows the intended direction in several important respects: immutable
Arrow batches and IPC artifacts carry program data; DataFusion plans and expressions perform
relational work; Delta versions are opened as table authority rather than used as mutation
authority; SQLite owns control-plane transactions; receipts and decision bytes are
canonicalized; and query leases are the right locus for versioned serving authority.

The unresolved findings violate the design corpus's decisive doctrine rather than incidental
style. Descriptive program DTOs unpacked into procedural validators conflict with model-first,
executable-model, single-authority, declarative single-sourcing, staged-compilation, and generic
runtime principles. The split owner route conflicts with least privilege, unified mutation,
explicit lifecycle, and idempotent recovery. Output-only limits conflict with explicit resource
ownership and failure semantics. Name-based zero-state proof conflicts with executable
governance and the rule that derived representations must be causally tied to their authority.

No architectural benefit was found that justifies these deviations. The clean remediation is
to complete the accepted consolidation into Arrow program data, generic DataFusion lowering,
exact-provider closure, one SQLite-owned activation command, and lease-resolved authority—not
to add another compatibility layer.

## Library Leverage Assessment

### Correctly leveraged capabilities

- Arrow IPC and canonical Arrow encodings provide reproducible program and result artifacts.
- DataFusion `Expr`, `LogicalPlan`, analyzer hooks, physical-plan execution, and metrics are used
  in production paths rather than reimplemented wholesale.
- Exact delta-rs table versions are opened and verified, with provider schemas/statistics
  retained for serving.
- SQLite remains the correct activation authority; delta-rs transaction history is not
  misused as a cross-table control plane.
- New lease authority pins and versioned checksum dispatch establish the right compatibility
  mechanism, even though the legacy branch is incomplete.

### Material underuse or substitution

- DataFusion's logical-plan model is not used as the normalized ontology program authority;
  operation-specific Rust functions substitute for generic lowering.
- Exact Delta `TableProvider`s are built but not registered into the candidate closure session.
- The analyzer hook is installed, but the accepted domain lattice and effects are not modeled.
- DataFusion runtime environment, memory-pool, cancellation, and spill configuration are not
  brought under the governed resource profile.
- DataFusion function/catalog extensibility remains less authoritative than the Rust native
  operation enum.

These are correctness and authority issues, not requests for speculative library adoption or
performance work.

## Legacy and Decommission Assessment

The implementation removes many retired names, old recipe surfaces, and obsolete design-root
references. DB11 and DB12 have credible current-tree support. DB07-DB10 do not meet behavioral
zero state: their old responsibilities survive behind renamed structures and generic-looking
entry points.

| Decommission batch | Assessment | Surviving authority |
|---|---|---|
| DB07 | fail | handwritten validators plus `NativeValidationOperation` |
| DB08 | fail | closure execution ignores exact providers and declared operands |
| DB09 | fail | proof/decision creation and serving-pointer commit are separate authorities |
| DB10 | fail | global V2 fallback and caller-asserted owner identity |
| DB11 | pass on reviewed scope | no live temporary comparator found |
| DB12 | pass on reviewed scope | obsolete root/wording removed from live authority |

Zero-state evidence must demonstrate absence of the behavior and ownership pattern, not only
absence of a legacy token.

## Test and Operational Assessment

The suite is broad, fast enough for focused review, and deterministic in the checks rerun here.
It has good canonicalization, package reproducibility, SQLite replay, exact-provider readback,
and metadata-persistence coverage. The principal defect is oracle construction: several tests
use the implementation's own derived identities as both expected and observed values, or wire
low-level internal calls around a narrow production transport segment.

The final integration scenario is therefore not a black-box production proof. It uses real
Delta tables and the real daemon command, which is valuable, but candidate proof, owner
decision, and serving cutover are supplied through direct Rust calls. Graceful daemon stop and
restart does not substitute for process termination at the transaction seams. The four named
integration assertions share one cached scenario, so they do not independently establish each
failure boundary.

Operationally, the durable ontology-pointer transaction and request-key replay are promising.
Certification must wait until that transaction owns the serving authority transition and its
prerequisites, and until readiness/recovery proves the same epoch visible to queries.

## Plan Deviations and Diff Hygiene

The committed candidate has strong provenance hygiene: the baseline is ancestral, the tree was
clean, packet proving commits exist, and the state schema/declared inputs are current. The large
229-file diff is consistent with the plan's cross-cutting design and authoritative-design
updates; no unrelated destructive or performance-baselining work was identified in the
reviewed scope.

The material deviations are semantic and unrecorded:

1. normalized plan/expression relations were replaced by flat rule-operation metadata;
2. generic compilation was replaced by a Rust native-operation dispatcher;
3. exact semantic closure was replaced by self-derived digest comparison;
4. one production activation route was split into ontology and serving transactions;
5. the specified domain lattice/effects were replaced by a smaller reject-oriented checker;
6. the bounded execution environment was reduced to terminal output limits; and
7. legacy lease resolution was replaced by a V2 default.

Those changes alter accepted outcomes and should have triggered state adaptation or plan/design
reconciliation before completion was recorded.

## Required Remediation Order

1. **Restore executable program authority (IR-001).** Define and generate the normalized
   node/edge relations, build the generic binder/lowerer, and remove handwritten dispatch.
2. **Make closure semantic and exact-provider-bound (IR-002).** Execute the compiled closure
   over exact Delta providers and derive receipts from those results.
3. **Unify the production owner route (IR-003).** Put proof, authenticated decision,
   acceptance, ontology authority, and serving authority behind one atomic command and private
   permit.
4. **Complete analyzer and runtime governance (IR-004, IR-005).** Implement the domain
   state/effect algebra and bounded DataFusion environment.
5. **Resolve lease compatibility (IR-006).** Make both legacy and new authorities explicit and
   query-proven.
6. **Rebuild final assurance and zero-state proof (IR-007).** Use causal mutations,
   black-box production tests, crash recovery, and semantic structural checks.
7. Reconcile execution state only after the strengthened packet and milestone evidence passes
   at a fresh proving commit.

## Focused Re-Review Scope

A focused re-review may be limited to the following if no unrelated implementation changes are
introduced:

- normalized program schemas, package generation, generic compiler, and removal of native
  operation dispatch;
- exact-provider candidate closure and independently corrupted relation fixtures;
- daemon/admin proof-decision-activation orchestration, atomic serving transition, actor
  authorization, and crash recovery;
- domain lattice/effect implementation and complete expression/plan transition tests;
- governed `RuntimeEnv`, memory/deadline/cancellation/spill behavior, and durable no-mutation
  proof;
- simultaneous legacy/new lease query execution and rollback;
- strengthened governance/zero-state rules, execution-state reconciliation, and fresh proving
  commits.

The re-review need not introduce or require a performance baseline. It should rerun the
affected packet oracles, the black-box cutover vertical, governance/zero-state checks,
`just plan-status`, `just artifacts-check`, and the repository-level validation matrix required
by the plan after all remediation is complete.
