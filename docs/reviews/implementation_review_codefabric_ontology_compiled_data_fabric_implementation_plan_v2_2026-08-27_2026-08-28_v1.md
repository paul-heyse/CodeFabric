---
artifact: implementation-review
plan_path: docs/plans/codefabric_ontology_compiled_data_fabric_implementation_plan_v2_2026-08-27.md
verdict: changes-required
version: v1
date: 2026-08-28
status: complete
---

# Implementation Review: CodeFabric ontology-compiled data fabric plan v2

## Provenance and Review Scope

This independent, read-only review assesses the current implementation against the accepted
v2 plan, accepted v3 design, schema-v2 execution state, current code, legacy dispositions,
library decisions, and executable proof. The implementation-status report at
`docs/reviews/implementation_status_codefabric_ontology_compiled_data_fabric_implementation_plan_v2_2026-08-27_2026-08-28_v1.md`
was used only as a locator; every conclusion below was re-derived from the current tree.

The plan baseline is `eb7a738fa55037b19706fd842737cecad65ffe16`; review HEAD is
`71a888fed8aae660f97a8bc420f04a039f5aacae`, and the baseline is an ancestor of HEAD. The
implementation is a dirty worktree candidate: 103 tracked files differ in the scoped diff,
18 implementation/review paths are untracked, and no packet has a proving commit. The review
did not modify production code, tests, plans, designs, or execution state; this report is its
only repository change.

The method combined plan/design/state reconstruction, three fresh non-overlapping review
lenses, pinned DataFusion 55/Arrow 59 and delta-rs reference checks, Rust call-site and
consumer tracing, hidden-aware `rg`, `ast-grep` structural coverage, and focused packet
oracles. Per the recorded owner waiver, no performance baseline or comparator was run or
treated as required evidence.

## Executive Summary

The candidate contains substantial and generally coherent implementation: one schema-driver
compilation pass, generated per-domain Arrow extension types, an installed DataFusion analyzer,
twenty ontology relations, generated result/control schemas, DataFusion relational validation,
truthful overlay statistics, publication-classified constraints, and an atomic SQLite
acceptance/pointer transaction. Focused WP05, WP07, WP08, WP11, WP13, WP15, and WP17 oracle
quartets all pass.

The target outcome is nevertheless not implemented completely. The two defining M03/M04
claims fail under direct inspection: recursive self-description is only a table census, and
Stage 2b has no production owner route. Its candidate dossier is not integrity-bound to the
frozen snapshot or real proving evidence, and its exactly-once behavior depends on process-local
state that recovery does not restore. The typed rule contracts are descriptive metadata rather
than the authority that drives execution. Additional material gaps remain in analyzer coverage,
lease-version result compatibility, probe decision accountability, phrase-operation
single-sourcing, and DB01 decommissioning.

The passing packet selectors therefore demonstrate useful local behavior, not plan completion.
The execution state correctly remains `executing`: zero packets, milestones, and decommission
batches are complete; all proving commits are null; five declared inputs are stale.

## Verdict

**Changes required.** WP01–WP17, M01–M04, and DB01–DB06 must not be certified complete from
this candidate. The accepted design remains implementable and does not need reopening; the
defects are implementation, integration, decommission, and proof failures.

## Gate and Evidence Assessment

| Evidence | Fresh result | Assessment |
|---|---:|---|
| baseline ancestry | pass | accepted baseline exists and is an ancestor of review HEAD |
| `just plan-status` | **fail** | healthy=false; five stale inputs; zero trusted completions |
| `just packet-oracle-check WP05` | pass; exactly four | accepts incomplete phrase/governance authority; see IR-008 |
| `just packet-oracle-check WP07` | pass; exactly four | generated types and republish fixture pass; DB01 recipe still live |
| `just packet-oracle-check WP08` | pass; exactly four | direct comparisons/cast/union pass; composite-expression bypass remains |
| `just packet-oracle-check WP11` | pass; exactly four | selected violations are caught; compiled contracts do not drive execution |
| `just packet-oracle-check WP13` | pass; exactly four | V1/V2 functions pass; live lease-version selection is absent |
| `just packet-oracle-check WP15` | pass; exactly four | statistics, pushdown, and constraint behavior are materially supported |
| `just packet-oracle-check WP17` | pass; exactly four | table-census and in-memory first-activation fixtures pass; M03 does not |
| Stage-2b call-site census | 103 Rust files, 0 skipped | all three `activate_stage2b` calls are test-only; dossier build and serving self-description have no callers |
| retired-name text census | 233 scoped files | retired type/address names are absent from live implementation corpus |
| result-schema structural census | 1 file, 0 skipped | no production `Field::new` call in `semantic_query.rs` |
| worktree/proving-commit trust | **fail** | uncommitted candidate; every packet proving commit is null |

The exact packet runner correctly rejects missing and duplicate oracle definitions, but it does
not establish that each definition proves its governed criterion. In this candidate the same
four-oracle selectors remain green when the central causal link is absent: generated rule
contracts do not control validation, the WP17 resolution does not resolve semantic closure,
and the WP13 runtime never selects V1 for an old lease.

## Finding Index

| ID | Severity | Dimension | Summary |
|---|---|---|---|
| IR-001 | blocker | outcome / architecture / tests | recursive self-description is a table census, not TI-8 closure |
| IR-002 | blocker | operations / integration / security | Stage 2b is test-only and bypassable through generic activation |
| IR-003 | blocker | integrity / recovery / provenance | Stage-2b dossiers and idempotence are not durable or candidate-bound |
| IR-004 | blocker | architecture / correctness | compiled rule contracts do not drive executable validation |
| IR-005 | major | correctness / library use | ID-domain analysis is bypassable through unmodeled expressions |
| IR-006 | major | compatibility / correctness | result schema/checksum selection ignores lease version |
| IR-007 | major | governance / tests / provenance | probe branches are preselected and self-authorized |
| IR-008 | major | architecture / governance / legacy | phrase semantics and promised governance remain duplicated |
| IR-009 | major | legacy / operations | DB01's retired generic-ID recipe remains callable |
| IR-010 | major | gates / provenance / diff hygiene | no plan scope is certifiable from the current worktree |

## Findings

### IR-001 — Recursive self-description is a table census, not TI-8 closure

**Severity:** blocker

**Dimension:** outcome / architecture / tests

**Design and plan references:** design TI-1 and TI-8; plan WP17 required change 2 and
`odf_stage2b_recursive_self_description`.

**Evidence:** `OntologyCatalogResolution` contains only registry-authority IDs, relation name,
row count, field names, and two caller-supplied result strings
(`src/ontology_activation.rs:65-81`). `resolve_ontology_catalog` reads the two bootstrap
relations, enumerates `table_contract` rows, collects each table, and copies those shallow
properties (`src/ontology_activation.rs:195-273`). It never resolves or anti-joins governed
codes, ontology edges, semantic types, table/column/current-result contracts, identity recipes,
phrase/rule bindings, snapshot, publication, or plan. The delivered result digest is checked
only for `b3:` syntax and is never looked up.

The green oracle adds one `table_contract` row, registers a clone of `enum_domain` under the
name `seeded_new_domain`, and asserts only 21 table names plus a nonempty authority set
(`src/ontology_activation.rs:517-582`). It does not seed or discover the required new code
domain and binding. `Stage2bActivationRequest` consumes only a caller-supplied set of table
codes, not the resolution.

**Failure consequence:** semantically disconnected, malformed, empty, or result-unresolvable
ontology relations can satisfy the named self-description gate and proceed toward activation.
The catalog is discoverable, but the data fabric is not recursively self-describing in the
accepted meaning.

**Required remediation:** return an opaque, candidate-bound closure receipt produced from the
leased catalog and delivered result artifact. It must resolve every TI-8 family through typed
joins/anti-joins, bind the exact result artifact to its result-schema/result-field rows, and
record the snapshot/publication/plan identities. Activation must require this receipt.

**Focused executable re-test:** add
`odf_stage2b_self_description_rejects_unresolved_result_and_broken_edges` with one valid
additive domain and corruptions for every authority family, then run:

```bash
cargo nextest run --locked --lib -E 'test(/odf_stage2b_(recursive_self_description|self_description_rejects_unresolved_result_and_broken_edges)/)' --no-tests=fail
```

### IR-002 — Stage 2b is test-only and bypassable through generic activation

**Severity:** blocker

**Dimension:** operations / integration / security

**Design and plan references:** design Stage 2b atomicity and TI-8; plan WP17 outcome,
required change 5, and M03.

**Evidence:** an `ast-grep` call-site search over 103 Rust files with zero skipped files finds
the only three `activate_stage2b` calls in `snapshot_runtime.rs`'s `#[cfg(test)]` module
(`src/snapshot_runtime.rs:1592`, `:1618`, `:1632`). `OntologyCandidateDossier::build` and
`ServingQuerySession::resolve_ontology_self_description` have no callers. No daemon,
coordinator, administrative command, or publication orchestration can execute the governed
flow.

Meanwhile the public generic `ServingSnapshotRuntime::activate` accepts an initial snapshot
without ontology acceptance (`src/snapshot_runtime.rs:300-330`). The Gate-B vertical uses that
path (`src/gate_b_candidate/vertical.rs:815-826`), and the overlay rebase path uses it for later
snapshots (`src/fabric/overlay.rs:1809-1820`). No candidate classification prevents an ontology
fingerprint-moving initial candidate from bypassing Stage 2b.

**Failure consequence:** the Stage-2b outcome is not operationally reachable through its
required owner, while a less-governed pointer path remains callable. The presence of an API and
unit tests does not implement the sole activation authority.

**Required remediation:** wire one authorized daemon/coordinator route that freezes the
candidate, executes the complete gate set, obtains accountable owner acceptance, and calls the
Stage-2b transaction. Make generic activation unable to activate an unaccepted Stage-2b
candidate; retain it only for candidate classes whose authority permits it.

**Focused executable re-test:** add `odf_daemon_stage2b_activation_owner_route` and an
unauthorized/generic-bypass negative case, then run the exact integration selector and repeat
the structural census to prove one production owner and no bypass.

### IR-003 — Stage-2b dossiers and idempotence are not durable or candidate-bound

**Severity:** blocker

**Dimension:** integrity / recovery / provenance

**Design and plan references:** design F-2, F-6, TI-8, Stage-2b atomicity; plan WP17 required
changes 1, 4, and 5.

**Evidence:** `OntologyCandidateDossier` has public fields. `build` verifies table presence and
only the shape of five `b3:` strings (`src/ontology_activation.rs:276-340`); `activate` does not
recompute `dossier_digest` and again checks proof strings only for key/prefix/length
(`src/ontology_activation.rs:361-407`). Runtime binding compares just `(table_code,
delta_version)` (`src/snapshot_runtime.rs:372-396`), omitting workspace, publication, snapshot,
manifest digest, table URI, schema digest, content digest, and actual gate receipts. The passing
fixture fabricates arbitrary proof and dossier digests (`src/snapshot_runtime.rs:1513-1530`).

Exactly-once retry depends on caller-owned `OntologyActivationState`
(`src/ontology_activation.rs:83-89`, `:392-395`). The SQLite transaction can commit before the
`AfterSqlCommitBeforeMemorySwap` fault (`src/snapshot_runtime.rs:501-667`), but the caller state
is assigned only after the function succeeds (`src/snapshot_runtime.rs:452`). `recover` restores
only the serving candidate, not ontology versions or acceptance (`src/snapshot_runtime.rs:673-725`).
The WP17 fault test starts from an empty runtime with no predecessor or active lease and covers
only the six pre-commit ontology faults (`src/snapshot_runtime.rs:1580-1602`).

**Failure consequence:** stale or fabricated proof digests can be accepted, a dossier can be
replayed onto a different candidate with coincident versions, and restart after durable commit
cannot return the required idempotent no-op. It may instead collide on durable inserts or
attempt another acceptance.

**Required remediation:** make dossiers opaque and derive them from the frozen candidate,
trusted proving artifacts, and the IR-001 closure receipt. Persist the full ontology activation
record in the same SQLite transaction; recover and reconcile it before retry. Bind acceptance
to an authenticated/accountable owner boundary and exact candidate identity.

**Focused executable re-test:** add
`odf_stage2b_rejects_unbound_or_stale_dossier` and
`odf_stage2b_postcommit_restart_retry_idempotent`; include mutated proofs/workspace/manifest,
an existing predecessor with active leases, every fault point, reopen/recover, and a retry that
leaves exactly one acceptance and pointer generation.

### IR-004 — Compiled rule contracts do not drive executable validation

**Severity:** blocker

**Dimension:** architecture / correctness

**Design and plan references:** design D-09, TI-7, TI-9; plan WP11 required changes 1-3 and
`odf_compiled_rule_contract_census`.

**Evidence:** `CompiledRuleOperationKind` and `CompiledRuleContract` are generated and exposed
(`src/compiled_ontology.rs:85-108`; `src/ontology_rules.rs:18-21`), but production execution
never dispatches on them. `validate_compiled_ontology_rules` directly calls ten handwritten
validators in a fixed sequence (`src/ontology_rules.rs:545-557`). Those functions own table
codes, column names, semantic-authority mappings, and rule-specific strings. The compiled rule
contracts are consumed only to emit `rule_contract` ontology rows.

The structural oracle merely asserts that eleven distinct operation names exist in Contract IR
(`tooling/ci/test_ontology_compiled_data_fabric.py:251-254`). The WP11 selector therefore passes
without a causal connection between a compiled contract and the validation plan it supposedly
selects.

**Failure consequence:** changing, deleting, or adding a governed rule contract does not
change runtime enforcement. Metadata and executable behavior can drift while publication gates
remain green, defeating the plan's single executable ontology authority.

**Required remediation:** give each compiled rule typed input/output/diagnostic operands
sufficient for exhaustive lowering; iterate the generated contracts and dispatch every
`operation_kind` into ordinary DataFusion plans. Remove the parallel handwritten semantic
census. Rust should orchestrate execution and diagnostics, not own a second rule table.

**Focused executable re-test:** add a causal mutation fixture for every operation kind and a
census proving each compiled contract lowers and executes exactly once, then run:

```bash
just packet-oracle-check WP11
just ontology-relational-closure-check
just governance-scan
```

### IR-005 — ID-domain analysis is bypassable through unmodeled expressions

**Severity:** major

**Dimension:** correctness / library use

**Design and plan references:** design D-02, TI-2; plan WP08 universal analyzer requirement.

**Evidence:** the implementation uses the correct DataFusion 55 seam: one
`DomainConformanceRule` is installed in the serving `SessionState`, and
`SessionState::optimize` runs analyzer rules before logical optimization. The rule itself is not
fail-closed. `validate_expression` handles only `BinaryExpr`, `InList`, `Cast`, and `TryCast`,
while `expression_domain` recognizes only alias, column, literal, cast, and try-cast
(`src/domain_conformance.rs:45-119`). Every other expression returns `None`; binary enforcement
explicitly accepts `(None, None)` (`src/domain_conformance.rs:142-152`). `Union` is the only set
alignment handled.

The current tests cover direct comparison, direct IN-list, cast, and union
(`src/domain_conformance.rs:233-292`). CASE/scalar-function wrappers, BETWEEN, subquery/set
comparisons, joins, and metadata-erasing composite outputs are not covered.

**Failure consequence:** wrapping workspace and repository IDs in separately unmodeled
expressions can erase both domains from this analyzer and permit a cross-domain comparison.
The plan's universal-ingress claim is therefore false even though the analyzer is correctly
installed.

**Required remediation:** define and propagate a domain lattice through every domain-preserving
DataFusion expression and set-comparison form. Reject unsupported metadata-erasing forms
fail-closed. Exercise the installed serving analyzer through SQL, semantic native plans, joins,
subqueries, set operations, literals, and binder delegation—not only direct rule invocation.

**Focused executable re-test:** add `odf_nested_expression_cross_domain_rejection` covering
CASE/coalesce, BETWEEN, IN-subquery, set comparisons, joins, same-domain controls, and unknown
extensions, then run WP08's packet selector and `just id-domain-extension-check`.

### IR-006 — Result schema/checksum selection ignores lease version

**Severity:** major

**Dimension:** compatibility / correctness

**Design and plan references:** design D-04, TI-3, TI-5; plan WP13 required change 3 and DB03.

**Evidence:** serving always computes `result_checksum_v2` and always records the global V2
constant (`src/fabric/serving.rs:877-881`, `:973`). The version-dispatching
`result_checksum_for_version` is used only by unit tests (`src/fabric/result_checksum.rs:220-240`).
Semantic result schema IDs are globally generated target IDs rather than selected from a
lease-pinned result authority. The WP13 conformance test checks eight-form success and
determinism, not concurrent old/new lease routing.

**Failure consequence:** an old lease cannot continue its V1/current schema and checksum after
the V2 authority lands. The implementation keeps a V1 verifier but does not implement bounded
coexistence or the governed result-boundary transaction required by the plan.

**Required remediation:** pin a result-schema authority/version in the leased manifest, select
the generated schema from that pin, and route emission through
`result_checksum_for_version`. Old and new leases must remain independently stable across
activation and restart.

**Focused executable re-test:** hold an old lease while activating the target result authority,
acquire a new lease, and prove old=V1/current and new=V2/target before and after restart. Add the
matrix to `query-determinism-check`, `query-form-contract-check`, and WP13's selector.

### IR-007 — Probe branches are preselected and self-authorized

**Severity:** major

**Dimension:** governance / tests / provenance

**Design and plan references:** design probe suite and LD decisions; plan WP02 outcome and
required changes 1-4.

**Evidence:** probe branch/fallback values are hardcoded before execution
(`scripts/ontology_fabric_probe_suite.py:17-87`). Several commands do not test the capability
they select: PR-3a runs already-selected flat-span tests instead of a Delta struct round-trip;
PR-5 runs ID lowering rather than Parquet extension-metadata persistence; PR-6 runs decoration
shape rather than unused-left-join elimination. The test helper automatically runs the suite
and calls `record_reviewed_decision`; that function hardcodes reviewer
`plan-owner-v2-implementation-authorization` and a rationale
(`scripts/ontology_fabric_probe_suite.py:168-199`). Environment identity is only a hash of the
repository path, and fixture identity is the lockfile digest.

No decision transaction exists in the execution state, and `just plan-status` reports no
accepted input evolutions. Generic authorization to implement the plan is not an accountable
review of each observation.

**Failure consequence:** architecture branches appear independently reviewed without evidence
for the library behavior they govern. Downstream packets validate a target-only file created by
their own test process rather than a durable, accountable state transaction.

**Required remediation:** implement the exact PR capability probes, emit observations only,
and separate owner review into an externally supplied schema-v2 state transaction. Bind real
environment/session/workload/fixture evidence and make downstream packets reject missing,
stale, or unreviewed state. Preserve only the explicit PR-7 performance waiver.

**Focused executable re-test:** after adding real probes, run `just probe-suite` without creating
a decision; prove `just packet-oracle-check WP02` fails until an independently recorded owner
transaction exists, then prove pin/config/report drift invalidates it.

### IR-008 — Phrase semantics and promised governance remain duplicated

**Severity:** major

**Dimension:** architecture / governance / legacy

**Design and plan references:** design D-08/D-09, TI-7, TI-9; plan WP05 and DB05.

**Evidence:** `entity_kind_codes`, `relation_kind_codes`, and `property_kind_codes` retain large
handwritten phrase/contract match tables (`src/semantic_query.rs:1194-1322`). The generated
`SEMANTIC_OPERATION_SPECS` covers only the narrow certainty-condition operations, and
`compiled_condition_predicate` consumes only column/operator/operands while ignoring the
compiled null policy, output role, and diagnostic contract (`src/semantic_query.rs:1335-1352`).

The WP05 source-text oracles check symbol presence rather than bidirectional registry/compiler
census. A hidden-aware search across `rules/` and `rule-tests/` finds neither of the two promised
governance rules: no bare compiler predicate integer, and no phrase arm without a registry ID.
`model-no-raw-governed-code-or-flag` is a different model-compiler boundary rule.

**Failure consequence:** registry changes do not fully define compiler behavior; adding or
renaming a phrase still requires handwritten runtime edits, and operation diagnostics/null
semantics can drift from the compiled authority while WP05 stays green.

**Required remediation:** compile all governed phrase semantics into closed typed operations
and dispatch them generically. Add both plan-promised structural rules with positive and
negative fixtures and make the coverage oracle compare registry-marked bindings in both
directions.

**Focused executable re-test:** add a registry-only mutation that changes a binding and proves
both relational and graph paths follow it, then run:

```bash
just packet-oracle-check WP05
just semantic-query-conformance-check
just governance-scan
```

### IR-009 — DB01's retired generic-ID recipe remains callable

**Severity:** major

**Dimension:** legacy / operations

**Design and plan references:** plan WP07 and DB01.

**Evidence:** the live source/contract/upfront-design name census finds no
`codefabric.id16` or `Id16Extension`, which is good implementation progress. However,
`id16-extension-contract-check` remains in the public command contract
(`justfile:142-146`) and in the gate-filter census
(`tooling/ci/gate-filter-census.json:10`). DB01 explicitly requires that recipe not resolve and
that `id-domain-extension-check` be its successor.

**Failure consequence:** operators and automation can continue invoking a retired generic-ID
assurance surface, preserving duplicate operational authority and misleading completion
evidence.

**Required remediation:** remove the recipe and census entry, update any callers, and retain
only the per-domain successor. Rename or retire remaining test identities if they imply generic
ID authority.

**Focused executable re-test:** prove the old recipe is absent from `just --list` and all live
tooling, then run `just id-domain-extension-check` and `just gate-filter-census`.

### IR-010 — No plan scope is certifiable from the current worktree

**Severity:** major

**Dimension:** gates / provenance / diff hygiene

**Design and plan references:** plan packet proving-commit contract, milestones M01-M04,
DB01-DB06, and final gate matrix.

**Evidence:** `just plan-status` reports `healthy: false`, zero complete packets/milestones/
decommission batches, and stale inputs for the accepted v3 design, Schema Contract IR,
ontology-relation registry, phrase registry, and query-form contract. Every WP01-WP17 packet
is `in_progress` with `proving_commit: null`; state has no accepted input evolutions. The
candidate spans 103 tracked and 18 untracked paths. HEAD itself contains no proving
implementation commit beyond the accepted plan activation point.

The state is truthful and should remain so. Passing local packet selectors are progress
evidence only; they cannot provide two-reference comparison, commit ancestry, input freshness,
or a rerunnable completion point.

**Failure consequence:** no reviewer can reproduce or attribute a packet, milestone,
decommission, or plan-completion claim from repository history. The final gate matrix is not a
certification of this candidate.

**Required remediation:** close IR-001 through IR-009 first. Then create dependency-closed
proving commits, record the planned input evolutions through the owning packet transactions,
rerun the exact packet and milestone gates at those commits, and only then advance state. Do not
rewrite immutable planning-time hashes.

**Focused executable re-test:** at the intended proving HEAD, run `just plan-status`,
`just artifacts-check`, `just plan-dependency-check`, every final non-performance gate, and
`just ci-pr`; require zero stale inputs and trusted completion entries before re-review.

## Outcome and Invariant Matrix

| Outcome / invariant | Assessment | Evidence or gap |
|---|---|---|
| F-1 typed eight-form query plans | partial | eight-form execution passes; phrase authority and result-version routing remain split |
| F-2 atomic pinned publication/snapshot | partial | generic SQLite CAS is sound locally; Stage-2b owner/recovery path is incomplete |
| F-3 explicit unknown/capability gaps | supported | no contrary regression found in reviewed scope |
| F-4 deterministic checksums/rebuild | partial | V2 and V1 verifier KATs pass; lease selection and proving commit are absent |
| F-5 canonical typed identities | partial | generated domains are present; analyzer is not exhaustive |
| F-6 provenance closure | fail | dossiers accept synthetic proof strings and do not bind the exact candidate |
| TI-1 normalized twenty-relation ontology plane | partial | twenty generated relations exist; semantic recursive closure is not proved |
| TI-2 universal per-domain enforcement | fail | correct DataFusion seam, bypassable expression coverage |
| TI-3 one logical type vocabulary | partial | generated storage/result/control types exist; old/new result surfaces are global |
| TI-4 logical structure classification | supported | classification and span/pruning focused tests pass |
| TI-5 generated result authorities | partial | generated schemas/lists exist; current-result and lease-version closure fail |
| TI-6 truthful statistics/constraints | supported | focused statistics, adversarial pushdown, and constraint gates pass |
| TI-7 one compiled authority | fail | rule and phrase execution retain sibling handwritten authorities |
| TI-8 catalog-only recursive resolution | fail | table enumeration is not authority/contract/provenance resolution |
| TI-9 closed typed executable operations | fail | compiled rule contracts do not causally drive their DataFusion plans |
| Stage-2b atomic activation | fail | no production owner; unbound dossier; restart idempotence absent |
| DB01 | fail | names are retired, public legacy recipe remains |
| DB02 | candidate-complete | generated row shapes and dual-list zero state are materially present; commit proof absent |
| DB03 | partial | typed result lists present; old/new result-boundary transaction absent |
| DB04 | candidate-complete | retired address absent from live implementation envelope; commit proof absent |
| DB05 | fail | handwritten phrase semantics and promised rules remain |
| DB06 | partial | list-valued membership is absent; executable rule authority remains duplicated |

## Architecture and Doctrine Assessment

The implementation moves strongly toward the accepted architecture: schema and vocabulary are
compiled once, Arrow is the runtime contract, DataFusion owns relational plans, Delta versions
are manifest-pinned, and catalogs are frozen per lease. These align with staged compilation,
functional-core/imperative-shell separation, generic runtime, and durable present-state truth.

The remaining defects violate the load-bearing doctrine rather than cosmetic style:

- Principle 10 (declarative single-sourcing) and Principle 18 (generic runtime) are violated by
  handwritten rule and phrase dispatch parallel to generated contracts.
- Principle 12 (illegal states unrepresentable), Principle 20 (unified transactions), and
  Principle 24 (idempotency) are violated by public, caller-constructible dossiers and
  process-local Stage-2b recovery state.
- Principles 25 and 27 (reproducibility and provenance) are violated by synthetic proof strings,
  self-recorded probe decisions, and absent proving commits.
- Principles 30 and 31 (testability and executable governance) are violated when exact packet
  selectors pass despite absent causal connections.

No accepted design premise was invalidated. The corrective direction is to finish the compiled,
ontology-driven architecture already selected—not to add another runtime authority.

## Library Leverage Assessment

The pinned stack is correctly retained: DataFusion 55.0.0, Arrow/Parquet 59.2.0,
`object_store` 0.13.2, and delta-rs revision `43a0cf10...`. The implementation appropriately
uses `SessionStateBuilder`, `MemoryExtensionTypeRegistry`, Arrow extension metadata,
DataFusion joins/anti-joins/aggregates, exact-version Delta providers, frozen catalogs,
structured `ScanArgs`, truthful pushdown/statistics, and application-validated PK constraints.
No custom SQL IR or raw-Parquet table-state bypass was found.

The main under-leverage is architectural: DataFusion's analyzer and logical plans are the right
mechanisms, but the domain lattice and compiled rule dispatcher are incomplete. Delta's exact
versions are recorded, but Stage-2b tests use `MemTable` fixtures and fabricated version maps
rather than a real frozen multi-table Delta candidate/restart path. Result-version selection is
global instead of derived from the lease's pinned authority. These are application integration
gaps, not missing library features.

## Legacy and Decommission Assessment

The broad hidden-aware text census supports real progress: `codefabric.id16`,
`Id16Extension`, `cpg_base.enum_catalog`, and production `Statistics::new_unknown` are absent
from the declared live implementation envelope; no production `Field::new` remains in the
result-shaping module; ontology memberships are normalized rather than list-valued.

DB01 is still open because the public recipe survives. DB05 is open because handwritten phrase
semantic tables and the two promised governance rules survive/are absent respectively. DB03
cannot close until old/new lease result selection is proved. DB02 and DB04 appear materially
implemented but remain uncertified without their proving commits; DB06 remains partial until
compiled rule contracts become the executable authority.

## Test and Operational Assessment

The new Rust tests are materially stronger than the status report's earlier snapshot: direct
domain checks, ontology violation rejection, typed control capture, generated result execution,
statistics composition, adversarial pushdown, SQLite fault rollback, and deterministic retry all
execute. The packet selector's exact-four and zero-selection safeguards are useful.

The dominant problem is assertion strength and causal coverage. WP17 starts from an empty
runtime instead of preserving a predecessor and leases, retries only in one process, and does
not perform later ordinary fact publications. WP11 never changes a compiled contract to prove
it owns behavior. WP13 never holds simultaneous old/new leases. WP05 source tests check symbol
presence instead of the promised registry/runtime bijection and rule activation. WP02's test
creates its own reviewed decision.

Operationally, no production Stage-2b route or post-commit ontology recovery exists. The
generic serving transaction remains a useful substrate, but M03 requires the governed owner
path and restart reconciliation above it.

## Plan Deviations and Diff Hygiene

The only accepted scope deviation is the explicit owner waiver of performance baselining and
comparison. This review preserves that waiver and makes no performance finding.

The state records planned changes to five declared inputs, but none is yet accepted through a
trusted packet transaction. That is expected during execution and is not permission to call
the inputs fresh. The large dirty worktree and untracked generated/implementation files also
prevent meaningful two-reference diff attribution. No unrelated user changes were modified by
this review.

## Required Remediation Order

1. Close IR-001 through IR-003 as one Stage-2b trust vertical: typed closure receipt, opaque
   candidate-bound dossier, single production owner, durable recovery, predecessor/lease fault
   proof, and exact retry semantics.
2. Close IR-004 and IR-008 by making compiled rule and phrase contracts the causal executable
   authority; add mutation/registry-only proofs.
3. Close IR-005 and IR-006 with exhaustive analyzer-domain propagation and lease-pinned
   result-schema/checksum selection.
4. Close IR-007 and IR-009: genuine observation-only probes plus accountable state decisions,
   and removal of the retired generic-ID command surface.
5. Re-run dependency-closed packet gates, create proving commits, accept planned input
   evolutions, close DB/milestone state, and execute the final non-performance matrix.

## Focused Re-Review Scope

Re-review can remain narrow if remediation follows the order above. It should re-open WP02,
WP05, WP07-WP08, WP11, WP13, WP17, M02-M04, DB01, DB03, DB05, and DB06, plus any shared files
changed to introduce the durable activation record. WP03-WP04, WP06, WP09-WP10, WP12, WP14,
and WP15 need only regression evidence unless their authority or schema surfaces change.

Approval requires fresh committed evidence at the proving HEAD: all cited focused tests, real
Delta-backed Stage-2b activation/restart evidence, zero stale inputs, trusted packet ancestry,
closed legacy recipes/rules, and the plan's final non-performance gates.
