---
artifact: implementation-review
plan_path: docs/plans/codefabric_ontology_compiled_data_fabric_implementation_plan_v3_2026-08-28.md
verdict: changes-required
version: v2
date: 2026-08-29
status: complete
---

# Implementation Review: CodeFabric ontology-compiled data fabric plan v3 — remediation candidate

## Provenance and Review Scope

This independent, read-only re-review assesses the implementation of the accepted v3 plan and
v5 design after remediation of the v1 review. It reconciles the current source, schema-v2
execution state, proving-commit ancestry, named packet and milestone checks, legacy
dispositions, and the pinned DataFusion 55, Arrow 59, and delta-rs capabilities. The v1 review
was used only as a stable finding ledger; closure or continued failure was re-derived from the
current tree.

The plan baseline is `71a888fed8aae660f97a8bc420f04a039f5aacae`. Review HEAD is
`6f6e27987bd97cad44c91e6a238104ede9e6ffe6`; the source proving commit is
`47f2074cbd0dd1698b1229dfb16bdb83afc1990c`. Both the baseline and source proving commit are
ancestors of review HEAD. The remediation from the prior review HEAD
`db3797cd12636ea8347f5cb67dd60f40ca91cfa1` through the source proving commit changes 69 files
with 6,366 insertions and 1,713 deletions. The only later commit closes execution-state
documentation. The worktree was clean when this review began.

Execution state declares WP18-WP27, M05-M08, and DB07-DB12 complete. `just plan-status` accepts
the schema, declared-input freshness, proving-commit reachability, and completion metadata.
Those provenance facts are useful, but completion is contradicted by current behavior and by a
required final recipe that is red at HEAD.

The review combined source and consumer tracing, hidden-aware textual search, structural
call-site coverage, exact Delta-provider tracing, DataFusion plan/analyzer traversal review,
and fresh focused execution of the plan's named checks. No performance baseline or performance
test was run: comparative performance is outside the accepted scope, while resource correctness
is explicitly in scope. Production code, tests, plans, designs, and execution state were not
modified. This report is the review's only repository change.

## Executive Summary

The remediation is substantial. Exact-version Delta providers now participate causally in
candidate execution; a normalized relational program is decoded and lowered to ordinary
DataFusion plans; candidate evidence, owner decisions, activation, pointer CAS, and restart
replay have a production orchestration path; a bounded DataFusion runtime exists for candidate
execution; result-authority pins and versioned checksum dispatch are persisted; and the
domain-expression variant census is broader. The focused causality, ID-domain enforcement,
candidate receipt, runtime resource, activation recovery, activation route, and result-authority
checks all pass.

The entirety of the accepted scope is still not implemented. Four defects are certification
blockers:

1. executable program phases are fail-open, and authored ontology operands still do not wholly
   generate the executable graph;
2. a public overlay-rebase path advances the serving snapshot through the raw commit primitive,
   outside the single activation authority;
3. governance walks plans with `LogicalPlan::apply` instead of
   `apply_with_subqueries`, allowing embedded subplans to escape domain and provider checks; and
4. the plan's required final zero-state recipe is red because the committed gate-filter census
   is stale, while several named oracles overstate their causal and failure-boundary coverage.

Major gaps also remain in expression-DAG validation, request replay binding, generated domain
policy, epoch reconstruction, production cancellation/spill ownership, and exact-provider
statistics preservation. These are current-tree defects, not requests to reopen the accepted
DataFusion/Arrow-centered design.

## Verdict

**Changes required.** The implementation must not be certified as the complete v3 plan. IR-002
from the prior review is closed, and the remaining prior findings show meaningful progress, but
IR-001, IR-003, IR-007, and IR-010 are blockers. IR-004 through IR-006 and IR-008, IR-009, and
IR-011 remain or introduce major defects. Consequently M06-M08 and DB07-DB10 are not supported
as complete, and the final certification claimed by WP27 is disproved at HEAD.

## Gate and Evidence Assessment

| Evidence | Fresh result | Assessment |
|---|---:|---|
| baseline/source proving ancestry | pass | both required commits are ancestors of review HEAD |
| worktree at review start | pass | clean committed candidate |
| `just plan-status` | pass | state shape, declared inputs, and proving-commit metadata are healthy |
| `just ontology-program-causality-check` | pass, 2 tests | proves selected calculation/phrase causality; does not mutate program phase or authored rule operands |
| `just id-domain-plan-enforcement-check` | pass, 4 tests | proves direct-plan cases; does not cover embedded subquery plans |
| `just ontology-candidate-receipt-check` | pass, 4 tests | exact-provider receipts are materially improved |
| `just ontology-runtime-resource-check` | pass, 2 tests | proves selected candidate bounds, not production cancellation/crash spill cleanup or serving deadlines |
| `just ontology-activation-route-check` | pass, 1 test | production coordinator route exists; a separate overlay raw-commit route also exists |
| `just ontology-activation-recovery-check` | pass, 4 unit + 1 integration | proves graceful/recoverable cases, not process-kill boundaries or full request-byte replay collision |
| `just result-authority-lease-check` | pass, 2 tests | proves persisted pin dispatch, not reconstructible epoch execution after restart |
| `just gate-filter-census` | **fail** | committed census omits `ontology_activation_restart_idempotency` from the changed justfile selector |
| `just ontology-datafabric-legacy-zero-state-check` | **fail** | source/structural/test/build stages pass, then the required gate-filter census fails |

The final zero-state run scanned 458 textual candidates, structurally scanned 214 Rust files
with zero skipped files, passed its selected adapter tests, and passed `cargo check
--all-targets`; it nevertheless exits nonzero at the mandatory census stage. The focused green
selectors therefore cannot support a completed WP27 or M08 at review HEAD.

## Finding Index

| ID | Status versus v1 | Severity | Dimension | Summary |
|---|---|---|---|---|
| IR-001 | open, reshaped | blocker | architecture / correctness | program phase and authored semantics are not fail-closed causal authority |
| IR-002 | closed | prior blocker | correctness / integration | candidate closure now executes against exact Delta providers |
| IR-003 | open, narrowed | blocker | integrity / operations | overlay rebase bypasses the sole activation and serving-pointer authority |
| IR-004 | partial | major | correctness / library use | domain enforcement is still handwritten rather than generated total policy |
| IR-005 | partial | major | reliability / resources | bounded runtime exists but production cancel/spill/lock ownership is incomplete |
| IR-006 | open, reshaped | major | compatibility / recovery | lease pins persist but executable epochs are not reconstructible from them |
| IR-007 | open | blocker | assurance / governance | final certification is red and several oracles remain non-causal |
| IR-008 | new | major | correctness / resilience | expression graphs lack cycle, reachability, and exact-role validation |
| IR-009 | new | major | integrity / idempotence | completed activation replay is not bound to the complete submitted request |
| IR-010 | new | blocker | correctness / security | analyzer and serving governance do not traverse embedded subqueries |
| IR-011 | new | major | library use / planning | exact-version wrappers discard authoritative Delta statistics and `ScanArgs` detail |

## Findings

### IR-001 — Program phase and authored semantics are not fail-closed causal authority

**Severity:** blocker  
**Affected scope:** WP19-WP20, M05-M06, DB07, TI-10-TI-11  
**Status versus v1:** open, with substantial structural remediation

The remediation adds normalized plan/expression relations and a generic DataFusion lowerer,
closing much of the earlier flat-catalog defect. The remaining authority chain is still
fail-open. Package decoding accepts any nonempty `execution_phase`
(`src/ontology_relational_program.rs:182-189`), the executor validates bindings only for
`candidate_validation` and `semantic_analysis` (`src/ontology_executor.rs:473-478`), and the
candidate runner executes only `candidate_validation` (`src/ontology_candidate.rs:715-721`).
Its bijection/receipt check is restricted to that filtered subset
(`src/ontology_candidate.rs:815-825`). A correctly resealed package can therefore rename a
required phase to `disabled`: decoding succeeds, execution omits it, and the filtered ledger
does not detect the missing program.

The authored-to-generated chain also remains incomplete. The schema contract carries
`operation_kind` and `ordered_operands`, but the generated `program.rule_binding` omits those
semantics. `ontology_graph.rs` selects hard-coded `ontology.*` rule IDs and manually constructs
their scans, predicates, joins, columns, and aggregates. The generator validates an exact
hard-coded rule census, so an additive authored rule still requires Rust changes. Runtime
cross-validation covers rule/calculation/policy identity, not causal parity with each authored
operation and operand. The named semantic-parity check proves relation presence and counts,
not mutation propagation from authored metadata to executable plan.

**Impact:** required semantics can be silently removed by a digest-valid package change, while
accepted authored metadata can drift from executable behavior. This violates the plan's
fail-closed phase contract, additive self-description, and one acyclic authority chain.

**Required remediation:** define a closed execution-phase enum and reject every unknown or
unconsumed executable row; require an exact one-to-one phase/program/execution/receipt ledger.
Carry authored operation and ordered-operand data into normalized program relations, or remove
the claim that those fields are executable authority. Eliminate hard-coded rule-ID dispatch in
the model compiler and prove a data-only additive rule. Mutate every phase and authored operand
under valid resealing and require a plan/result/diagnostic/receipt change or typed rejection.

### IR-002 — Exact Delta candidate closure is causally implemented

**Severity:** closed prior blocker  
**Affected scope:** WP23, M06, TI-13-TI-14  
**Status versus v1:** closed

The earlier self-derived one-row digest comparison has been replaced. The candidate now opens
each manifest-pinned table through exact-version Delta providers, verifies URI, version,
schema, and checksum, registers the frozen relations, and executes the compiled closure plans
against those providers (`src/ontology_candidate.rs:642-753`). Receipt construction is tied to
the resulting program executions. Focused candidate receipt and Delta-binding tests pass.

This closes the specific v1 finding. Package/epoch reconstruction weaknesses are tracked under
IR-006 rather than used to keep this distinct exact-provider defect artificially open.

### IR-003 — Overlay rebase bypasses the sole activation and serving-pointer authority

**Severity:** blocker  
**Affected scope:** WP24-WP26, M07-M08, DB09, TI-15-TI-16  
**Status versus v1:** open, narrowed to a live alternate mutation path

The production activation coordinator now performs candidate proof, accountable decision, and
ontology activation through one route. However, `OverlayRebaseRequest` is publicly
constructible and carries caller-supplied `candidate_body`
(`src/fabric/overlay.rs:1715-1733`). Public `ConsolidatedOverlay::execute_rebase` builds a
snapshot candidate from that body and calls the raw `commit_fact_snapshot` primitive
(`src/fabric/overlay.rs:1744-1819`).

The ordinary snapshot wrapper first verifies active result authority
(`src/snapshot_runtime.rs:276-343`), while the raw `pub(crate) commit_fact_snapshot`
(`src/snapshot_runtime.rs:346-383`) performs no equivalent activation-authority preservation.
Full Rust call-site coverage finds the overlay path and a Gate-B vertical as raw callers. The
structural rule bans only the exact spelling `pub fn commit_fact_snapshot`; it does not cover
`pub(crate)`, calls to the raw primitive, or `execute_rebase`. The final zero-state token set
also omits both live bypass forms.

**Impact:** a caller can advance serving snapshot state outside the sole activation command and
without a nonconstructible acceptance permit. That defeats the single mutation authority and
allows serving state to diverge from the accepted ontology/result/function/policy epoch.

**Required remediation:** place raw pointer mutation behind a private, nonconstructible
activation permit produced only by the durable acceptance transaction. Route overlay rebases
through the same authority-preserving kernel or make them non-authoritative. Expand structural
governance to calls, visibility variants, and semantic equivalents, then prove every production
pointer advance has exactly one authorized ancestor.

### IR-004 — Domain enforcement remains handwritten rather than generated total policy

**Severity:** major  
**Affected scope:** WP22, M06, DB08, TI-12  
**Status versus v1:** partial

`DomainState` and `DomainEffect` now exist and the pinned DataFusion expression/plan variant
census is usefully exhaustive. The semantic policy is nevertheless still custom Rust:
`effect_for_expression` is a handwritten map (`src/domain_conformance.rs:352-392`), comparison
exceptions are embedded in code (`src/domain_conformance.rs:436-440`), and function policies
are name-based tables (`src/domain_conformance.rs:268-287`, `:499-555`). No generated
`DomainOperationPolicy` exists. The generated semantic-analysis program has no executable root,
and the generic relational compiler explicitly delegates that phase back to the analyzer.

`GovernedSession::new_with_runtime` accepts an arbitrary identity string and always installs a
parameterless `DomainConformanceRule` (`src/governed_session.rs:246-297`). Public `seal_plan`
and `seal_sql` methods can turn caller-supplied arbitrary logical plans/SQL into a
`GovernedPlan`; the API is not the closed compiler-only capability described by the design.
The named truth-table test exercises only a same-domain and cross-domain comparison pair, not
the full state-by-effect lattice.

**Impact:** the package identity does not select the semantic policy actually enforced, valid
new operations require Rust edits, and the public sealing boundary is broader than the accepted
opaque compiled-plan ingress.

**Required remediation:** generate one total `DomainOperationPolicy` from ontology metadata,
bind it to the epoch/package identity, and make the analyzer consume that exact artifact.
Represent every pinned state/effect transition explicitly, close plan capability construction,
and execute the complete state-by-effect matrix rather than treating enum exhaustiveness as
semantic proof.

### IR-005 — Production cancellation, spill, and resource ownership remain incomplete

**Severity:** major  
**Affected scope:** WP21, WP23-WP26, M06-M08, TI-18  
**Status versus v1:** partial

Candidate sessions now build an explicit `RuntimeEnv`, bounded memory pool, fair spill pool,
batch/partition settings, output controls, deadline, and cancellation support
(`src/governed_session.rs:246-313`, `:445-515`). This is real remediation. Production
activation uses the ordinary execution method with a never-cancelled default, however, and the
coordinator does not propagate a task/request cancellation token. The daemon holds the global
`OperationalStore` mutex across asynchronous candidate proof
(`src/daemon.rs:629-650`), coupling long-running execution to all store users.

Spill directories use a predictable temporary path without an explicit private-mode creation
contract and are removed only by normal `Drop` (`src/governed_session.rs:266-285`, `:518-521`).
No startup orphan cleanup or process-death proof exists. The focused test starts already
cancelled or with a zero deadline, writes a file manually, and observes normal-drop cleanup; it
does not cancel executing DataFusion work, force real spill, or kill the process. Serving
runtime configuration separately lacks an owned deadline/cancellation token, and stream-drop
tests do not prove deadline termination and resource release.

**Impact:** production work can outlive its owner, block control-plane access, or leave spill
artifacts after abnormal termination even though the recorded profile claims bounded shared
execution.

**Required remediation:** propagate daemon/request cancellation and deadline through candidate
and serving execution, release the global store lock before DataFusion work, create private
per-run spill roots with startup orphan reconciliation, and record typed termination evidence.
Prove in-flight cancellation, deadline expiry, real spill cleanup, process death/restart, memory
consumer release, and zero durable mutation.

### IR-006 — Persisted lease pins do not reconstruct executable epochs

**Severity:** major  
**Affected scope:** WP25-WP26, M07-M08, DB10, TI-17  
**Status versus v1:** open, reshaped

The unpinned-V1-to-ambient-V2 defect was corrected: lease/result authority is now explicit and
checksum dispatch is versioned. The production query path still does not resolve all behavior
from the lease pin. `ServingQuerySession::from_lease` installs the current generated extension
set, analyzer, and functions rather than loading the pinned epoch
(`src/fabric/serving.rs:473-516`); completion consults only the checksum version
(`src/fabric/serving.rs:879-890`). The semantic compiler rebuilds the current binary's ontology
package on each query, and activation likewise builds the current package rather than resolving
a submitted content-addressed artifact.

The activation submission and durable candidate projection retain identities but no package
bytes or reconstructible artifact address. `SnapshotLeaseGuard` requires an in-memory
`Arc<ServingSnapshotCandidate>` created by fresh acquisition; production startup has no lease
rehydration path from persisted pins. The restart/lease oracle lists persisted rows and directly
dispatches checksum versions, but does not reconstruct and execute old and new epochs after a
process restart.

**Impact:** immutable metadata says which epoch a lease selected, but a restarted process can
execute that lease with the current binary's program, policy, function set, and analyzer. The
same lease can therefore change semantics without changing identity.

**Required remediation:** persist a content-addressed, retention-governed package/policy/
function artifact reference for every accepted epoch; implement a fail-closed resolver and
startup lease rehydration; and derive the entire query session from the resolved pin. Execute
simultaneous old/new leases before and after restart and rollback, rejecting missing artifacts
rather than falling back to current behavior.

### IR-007 — Final certification is red and several oracles remain non-causal

**Severity:** blocker  
**Affected scope:** WP27, M08, DB07-DB12, TI-10-TI-19  
**Status versus v1:** open

The mandatory final aggregate fails because `justfile` now includes
`ontology_activation_restart_idempotency` in `ontology-activation-recovery-check`, while
`tooling/ci/gate-filter-census.json` omits it. `scripts/gate_filter_census.py` correctly rejects
the mismatch. Therefore `just ontology-datafabric-legacy-zero-state-check` is red at HEAD and
the state claim that WP27/M08 are complete is false even before semantic findings are applied.

The assurance gaps are broader than the stale census. The causality test finds an execution
phase row but never mutates the phase. Recovery uses graceful daemon stop and same-process fault
injection rather than process termination at each transaction seam. Concurrency tests submit
the same candidate with request-key variation rather than competing candidate bytes. Four
named integration selectors share one cached scenario. Governance misses the overlay bypass,
hard-coded model-compiler rule dispatch, and embedded-subquery traversal defect.

**Impact:** green local selectors and trusted state metadata overclaim end-to-end causality,
crash safety, sole authority, and legacy zero state.

**Required remediation:** fix product defects first, then strengthen each oracle with an
independent causal mutation and the real production boundary. Add kill/restart seam tests,
different-candidate CAS races, complete subquery negatives, and semantic structural rules.
Update the gate-filter census only after the final selector set is stable, then rerun the full
plan finalization matrix at one fresh proving commit.

### IR-008 — Expression graphs lack cycle, reachability, and exact-role validation

**Severity:** major  
**Affected scope:** WP19-WP20, M05-M06, TI-10-TI-11  
**Status:** new

The relational decoder checks dangling expression edges and plan-node cycles, but does not
check expression-DAG cycles (`src/ontology_relational_program.rs:416-487`). Expression
compilation recursively follows edges with no active set, depth bound, or topological ordering
(`src/ontology_relational_program.rs:514-633`). Edge selection takes expected roles while
silently ignoring extra roles, and executor closure accepts referenced calculations as a subset
of the catalog rather than requiring exact executable closure.

**Impact:** a digest-valid two-node expression cycle can pass decoding and recurse until stack
overflow. Dead expressions, unused calculations, extra roles, or misplaced aliases can change
package identity without affecting execution, weakening both safety and causal receipts.

**Required remediation:** validate expression acyclicity, bounded depth/count, exact node-kind
role/arity/alias contracts, and reachability of every executable plan/expression/calculation
row. Compile iteratively or with an explicit active set. Add resealed cycle, dead-node, unused
calculation, extra-role, illegal-alias, and positive shared-DAG fixtures.

### IR-009 — Completed activation replay is not bound to the complete request

**Severity:** major  
**Affected scope:** WP24-WP26, M07-M08, TI-16  
**Status:** new

Activation submission contains mutable manifest, package/body, blob, and retention inputs
(`src/ontology_activation.rs:31-37`). The coordinator attempts completed replay before deriving
and authenticating those candidate inputs (`src/ontology_activation.rs:124-131`).
`replay_completed_ontology_activation` verifies workspace and publication identity but not a
canonical digest of the complete submitted request (`src/operational_store.rs:2144-2190`).

**Impact:** reusing a completed request key/publication with altered package, manifest, blob
list, or retention inputs returns the previous success instead of the TI-16 mandated
same-key/different-bytes collision. The reply is idempotent only for a partial identity.

**Required remediation:** canonicalize and persist the full submitted request digest before
replay lookup, compare it on every retry, and reject any same-key/different-bytes request before
returning the prior result. Add one mutation test for every request field, including reordered
but canonically equivalent maps/lists where the contract allows them.

### IR-010 — Governance does not traverse embedded subqueries

**Severity:** blocker  
**Affected scope:** WP22-WP23, M06, DB08, TI-12-TI-14  
**Status:** new

The domain analyzer traverses with `LogicalPlan::apply`
(`src/domain_conformance.rs:114-132`), and serving provider/extension allowlisting uses the same
method (`src/fabric/serving.rs:1909-1961`). In DataFusion 55, `apply_with_subqueries` is the
native traversal that descends through embedded `ScalarSubquery`, `Exists`, `InSubquery`, and
related plans. Existing tests cover direct expressions and set operations, not governed
violations inside embedded subplans.

**Impact:** a physically valid nested subquery can contain cross-domain comparisons,
unauthorized providers, forbidden functions, or extension nodes that the outer-plan traversal
never visits. This is a fail-open semantic and serving-policy bypass at an authorized ingress.

**Required remediation:** use DataFusion 55's subquery-aware traversal in both analyzer and
serving allowlist, with one shared helper so the two policies cannot drift. Add negative scalar,
`EXISTS`, `IN`, correlated, and set-comparison subqueries containing cross-domain values,
unauthorized providers/functions, and extension nodes, plus positive same-domain cases.

### IR-011 — Exact-version wrappers discard authoritative Delta statistics and `ScanArgs` detail

**Severity:** major  
**Affected scope:** WP21, WP23, M06, TI-13, LD-13  
**Status:** new

The catalog correctly reopens exact Delta versions and configures statistics loading, and the
inner provider exposes those statistics. The final `EffectiveStatisticsProvider` replaces them
with sparse manifest-derived row/null counts (`src/fabric/snapshot_catalog.rs:768-777`,
`:1056-1079`), losing Delta total-size, min/max, and retained column precision. Its scan path
also downconverts structured `ScanArgs` to the legacy arguments and discards
`StatisticsRequest` (`src/fabric/snapshot_catalog.rs:294-311`).

**Impact:** the final governed provider no longer preserves the exact Delta provider's planning
contract required by LD-13. This can produce materially different planning and makes manifest
statistics a parallel authority. No comparative performance claim is needed to establish this
contract violation.

**Required remediation:** preserve or conservatively merge inner Delta statistics for overlay
generation zero, authenticate manifest row counts without replacing valid transaction-log
statistics, recompute only overlay-affected values with honest precision, and forward
structured `ScanArgs` intact. Compare direct exact-provider and final-provider statistics and
exercise `StatisticsRequest` explicitly.

## Outcome and Invariant Matrix

| Target | Status | Current evidence |
|---|---|---|
| TI-10 Arrow-native compiled authority | **partial** | normalized program exists; authored rule semantics and expression closure remain incomplete |
| TI-11 DataFusion causal execution | **not met** | unknown phases skip execution; authored operand and dead-row causality are incomplete |
| TI-12 fail-closed semantic planning | **not met** | handwritten policy and non-recursive subquery traversal remain |
| TI-13 semantic self-description | **partial** | exact-provider closure is real; hard-coded rule census and package reconstruction constrain additivity |
| TI-14 candidate-bound proof | **partial** | receipts bind real execution, but filtered phases and graph closure weaken completeness |
| TI-15 one activation command | **not met** | coordinator exists, but overlay rebase retains a raw serving-pointer path |
| TI-16 durable idempotence/recovery | **partial** | CAS/restart path exists; complete request-byte collision is absent |
| TI-17 lease-scoped compatibility | **not met** | pins persist; complete executable epoch is rebuilt from current binary state |
| TI-18 bounded shared execution | **partial** | candidate bounds exist; production cancel/spill/serving ownership is incomplete |
| TI-19 accountable decisions | **supported with caveat** | observations and decisions are distinct; alternate pointer authority still defeats final acceptance exclusivity |
| DB07 duplicate semantic/phrase authority | **not met** | model compiler retains hard-coded rule semantics |
| DB08 governed-execution bypasses | **not met** | embedded subqueries bypass analysis/allowlisting |
| DB09 activation/proof duplicates | **not met** | overlay raw commit is an alternate serving mutation authority |
| DB10 global result selection/self-authorization | **not met** | checksum pin improved, but current-binary package/policy/functions remain ambient |
| DB11 temporary comparison authority | **supported** | no live authoritative comparison route found in reviewed scope |
| DB12 obsolete master root/wording | **supported** | live authoritative root cutover remains structurally present |

## Architecture and Doctrine Assessment

The candidate is now recognizably aligned with the accepted architecture: Arrow relations
carry program data, DataFusion owns ordinary logical and physical execution, exact Delta
versions are registered as relation providers, SQLite owns durable activation state, and
receipts/decisions/pins are distinct canonical identities. The v1 remediation should be
preserved.

The open defects are nevertheless violations of the architecture's decisive doctrines. A
fail-open phase and hard-coded generator semantics break executable-model and declarative
single-source principles. Raw overlay pointer mutation breaks single mutation authority and
least privilege. Non-recursive subquery governance breaks closed authorized ingress. Ambient
current-binary session construction breaks lease-scoped immutable authority. Incomplete
resource ownership breaks explicit lifecycle and failure semantics. Stale gate state breaks
executable governance.

The appropriate correction is further consolidation around the accepted model: one authored
ontology-to-program chain, one generic DataFusion compiler/analyzer policy, one subquery-aware
governance traversal, one content-addressed epoch resolver, and one activation permit. Adding
another compatibility shim or duplicate validation layer would worsen the authority problem.

## Library Leverage Assessment

Correct use should be retained:

- native DataFusion `Expr`, `LogicalPlanBuilder`, aggregates, semi/anti joins, analyzer hooks,
  execution streams, and bounded runtime components;
- exact delta-rs `with_version` reopening and loaded-version verification;
- Delta protocol/feature rejection for unsupported CDF, deletion-vector, type-widening, and
  reader/writer features;
- application transaction identity with `CommitProperties::with_max_retries(0)`;
- Arrow IPC/canonical encodings and the domain-aware `Id16ContractProvider` metadata/literal
  adapter; and
- structured argument forwarding already demonstrated by `OverlayIdentityProvider`.

Material underuse or substitution remains:

- `LogicalPlan::apply_with_subqueries` is not used where the pinned engine explicitly exposes
  it for semantic and provider traversal;
- authored operation/operand records do not wholly displace custom model-compiler rule logic;
- exact Delta statistics are loaded and then replaced rather than preserved/merged;
- structured `ScanArgs`, including `StatisticsRequest`, are downconverted by the final wrapper;
- runtime cancellation/deadline facilities are not owned end to end by production candidate
  and serving requests; and
- accepted package/function/policy artifacts are not resolved into sessions from immutable
  lease pins.

No custom UDF, physical operator, or new storage transaction mechanism is recommended. Native
DataFusion/Arrow/delta-rs capabilities already fit the required corrections.

## Legacy and Decommission Assessment

| Batch | Assessment | Current surviving authority or contradiction |
|---|---|---|
| DB07 | fail | hard-coded `ontology.*` rule dispatch and manual graph construction |
| DB08 | fail | handwritten domain policy and non-recursive subquery traversal |
| DB09 | fail | public overlay rebase reaches raw serving snapshot commit |
| DB10 | fail | lease checksum is explicit, but current-binary package/policy/function selection remains |
| DB11 | pass on reviewed scope | no live temporary comparison authority identified |
| DB12 | pass on reviewed scope | obsolete master root and wording remain absent from live authority |

Retired tokens and v1's native validator enum are substantially removed. That progress does not
establish behavioral zero state while equivalent authority survives behind the generator,
overlay, analyzer, and session-construction paths. Governance must prove unique replacement
authority, not merely absence of old spellings.

## Test and Operational Assessment

The remediation suite provides credible proof of exact-provider execution, receipt sealing,
selected relational lowering, SQLite CAS/replay mechanics, and explicit checksum pins. Its
weakness is boundary completeness. Several recipes prove one representative or metadata case
while their names claim total causality, production cancellation, crash recovery, or full epoch
compatibility.

The most consequential missing tests are: mutation of `execution_phase`; data-only additive
authored rules; expression cycles/dead rows/extra roles; nested subquery governance; complete
same-key/different-request replay collision; in-flight cancellation and deadline; real spill
and process-death cleanup; different-candidate concurrent activation; restart reconstruction of
simultaneous old/new leases; and exact statistics/`ScanArgs` preservation. The recovery vertical
uses graceful process control and shares one cached scenario among multiple selectors, reducing
fault independence.

Operational readiness is also blocked by the global store mutex held across proof, lack of
content-addressed epoch rehydration, and the alternate overlay pointer path. These are lifecycle
correctness issues. They do not require a performance baseline.

## Plan Deviations and Diff Hygiene

The remediation diff is focused on the plan's ontology/data-fabric surfaces, and no unrelated
destructive or performance-baselining work was identified. The baseline and proving commits
are ancestral, source is committed, and the state file is current. The report itself is the
only review-time worktree change.

The material unrecorded deviations are:

1. arbitrary nonempty execution phases are accepted rather than closed and exhaustively
   consumed;
2. authored operation/operand semantics still terminate in hard-coded generator logic;
3. domain policy is handwritten and public sealing is broader than compiler-only ingress;
4. subquery plans are not traversed by governance;
5. overlay rebase remains a second serving-pointer mutation authority;
6. accepted epochs are metadata-pinned but not artifact-resolved after restart;
7. replay equality covers only part of the submitted request;
8. resource profiles are not propagated through production cancellation/spill lifecycle;
9. final wrappers replace exact Delta statistics and structured scan requests; and
10. execution state was closed while a mandatory final recipe was red.

These deviations change accepted invariants and should have prevented WP27/M08 completion.

## Required Remediation Order

1. **Close authority and governance bypasses (IR-003, IR-010).** Put every serving-pointer
   mutation behind one nonconstructible activation permit and recurse through all embedded
   subquery plans in analyzer and serving allowlisting.
2. **Make the compiled program fail closed (IR-001, IR-008).** Close the phase enum, enforce
   exact program/expression closure, carry authored operands causally, and remove hard-coded
   rule dispatch.
3. **Bind activation and leases to complete immutable inputs (IR-009, IR-006).** Persist the
   complete request digest and content-addressed executable epoch, implement restart resolution,
   and remove current-binary fallbacks.
4. **Finish semantic and resource consolidation (IR-004, IR-005).** Generate the domain policy,
   close plan capability construction, propagate cancellation/deadline, shorten store lock
   ownership, and make spill lifecycle crash-safe.
5. **Preserve exact-provider planning contracts (IR-011).** Merge rather than replace Delta
   statistics and forward structured `ScanArgs`.
6. **Rebuild certification evidence (IR-007).** Add the causal/fault/concurrency cases above,
   update gate-filter/hash-like inventories only after implementation and selectors stabilize,
   then rerun WP18-WP27, milestone, decommission, plan-status, artifact, and repository gates at
   one fresh proving commit.

## Focused Re-Review Scope

If remediation remains focused, a subsequent review can be limited to:

- program phase/graph closure, authored-to-generated causality, and removal of rule-ID dispatch;
- subquery-aware domain/provider governance and the generated epoch-bound domain policy;
- overlay/serving-pointer call-site closure and activation-permit construction;
- complete request replay collision and content-addressed package/epoch rehydration;
- candidate and serving cancellation/deadline/spill/process-death behavior without a global
  store lock across execution;
- exact Delta statistics and structured `ScanArgs` preservation;
- simultaneous old/new lease query execution across restart and rollback; and
- strengthened final oracles, gate-filter census, execution-state reconciliation, and fresh
  proving commits.

No performance baseline is required. Re-review should rerun the affected packet selectors,
`just ontology-datafabric-legacy-zero-state-check`, `just plan-status`, artifact validation, and
the repository-level validation matrix only after the implementation scope is complete.
