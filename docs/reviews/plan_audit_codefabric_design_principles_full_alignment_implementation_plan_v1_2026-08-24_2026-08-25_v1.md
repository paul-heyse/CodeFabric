---
artifact: plan-audit
plan_path: docs/plans/codefabric_design_principles_full_alignment_implementation_plan_v1_2026-08-24.md
verdict: needs-redesign
version: v1
date: 2026-08-25
status: complete
---

# Plan Audit: CodeFabric design-principles full alignment implementation plan v1

## Provenance and Scope

This is an independent, read-only audit of the draft implementation plan and
its source design. The only repository mutation made by the audit is this new
report.

The audit evaluated:

- the complete target plan and remediation proposal;
- the 25 data-fabric design principles and the DataFusion 55/Arrow 59
  principle-alignment manual, including its requirement-to-feature routing;
- the governing design corpus, especially `QRY` request, composition,
  ordering, uncertainty, and conformance requirements;
- the current Rust implementation and governance surfaces at
  `f2dfcfe25dbfe46f0ca779a2fc4273787e18a445`;
- Arrow/Parquet 59.2.0 and DataFusion 55.0.0 reference documentation and
  exact installed crate sources; and
- plan structure, declared-input freshness, packet dependencies, gates,
  legacy cutovers, and proof obligations.

All eight declared input digests match the plan. That does not make the plan
executable: the target plan and five principal inputs are untracked, its
recorded dirty-tree description is incomplete, and direct plan validation
fails because the declared state path does not exist.

Current baseline evidence:

- `just ci-fast`: **failed** after 265/265 Rust tests and doctests passed;
  `typos` rejected the `P-n` notation and `nullable` spelling used in the
  plan/proposal.
- `validate_plan(..., verify_declared_inputs=True)`: **failed** with
  `unresolved state_path=docs/plans/state/codefabric-design-principles-full-alignment_v1_state.json`.
- `just artifacts-check`: passed only for the currently active, completed
  DataFusion-upgrade plan; it did not validate this inactive draft.
- All currently named existing recipes resolve. The proposed
  `alignment-detector-check` and `wave-gates` recipes do not exist yet, as
  expected before their owning packets.

Fresh-context design, library, and impact challengers independently reviewed
the plan. Their claims were rechecked against the artifacts, current code, and
pinned crate sources before inclusion below.

## Executive Summary

The plan is not ready to execute. Its target architecture cannot deliver the
claimed full-alignment outcome as written.

The two central blockers are architectural. First, the source design itself
says five principles and DP-051 require amendments to their normative design
homes, but the plan contains no such design work while M14 claims full
alignment. Second, GI-2 and WP62 force every semantic request into a
DataFusion `LogicalPlan`, although `QRY` requires eight request forms,
arbitrary query DAGs, graph paths/patterns, coverage and absence semantics,
and explicitly permits a `LogicalPlan` **and/or** `GraphOperatorPlan` DAG.
The current three-form SQL implementation is not a valid differential oracle
for that contract.

The plan also contains material library and execution defects: IPC is
over-applied to in-process providers; cross-table references are validated too
early; DataFusion 55 extension registration is assigned semantics it does not
provide; deterministic response ordering and durable identity encodings are
under-specified; `EXPLAIN ANALYZE` can create a second execution; requested
statistics have no designed producer/consumer path; oracle governance arrives
after the oracles it is meant to govern; and the dependency graph permits
conflicting work.

These are not localized wording corrections. The accepted target design must
be revised before a replacement implementation plan is produced.

## Readiness Verdict

**Verdict: `needs-redesign`.**

Do not activate or execute this v1 plan. Resolve F-001 and F-002 in the
accepted design, re-derive the conformance register and current-tree baseline,
then issue a new versioned plan that closes the remaining major findings.

## Finding Index

| ID | Severity | Category | Scope | Status |
|---|---|---|---|---|
| F-001 | blocker | doctrine | design §6, GI-10, M14 | open |
| F-002 | blocker | design | GI-2, WP62, M11, DB10 | open |
| F-003 | blocker | factuality | plan frontmatter, WP54 | open |
| F-004 | major | factuality | CONF, WP54–WP72 | open |
| F-005 | major | design | GI-3, WP59, WP60, DB11 | open |
| F-006 | major | design | WP57, WP59, publication | open |
| F-007 | major | library | LD-04, WP58 | open |
| F-008 | major | design | WP62, WP64, LD-03, LD-09 | open |
| F-009 | major | library | LD-06, WP65 | open |
| F-010 | major | library | LD-05, WP58 | open |
| F-011 | major | proof | WP54–WP72, WP70 | open |
| F-012 | major | sequence | WP56, WP62–WP69 | open |
| F-013 | major | doctrine | WP55, GI-1 | open |
| F-014 | major | proof | WP72, AC-G-79 | open |
| F-015 | major | operations | WP71, Gate B | open |
| F-016 | major | impact | baseline, WP54 | open |

## Findings

### F-001 — “Full alignment” omits required normative design amendments

**Severity:** blocker  
**Category:** doctrine  
**Scope:** remediation proposal §6, plan §1.1, GI-10, M14  
**Finding:** The proposal explicitly says P2, P10, P14, P19, and P21 are
“not resolved by implementation alone” and routes amendments to
`SUITE`/`FAB`/`QRY`/`LIFE`; DP-051 remains routed to `GEN §13`. No packet
changes or accepts those normative homes. Nevertheless, §1.1 and M14 claim
full resolution of all 124 findings and full alignment with all 25
principles. GI-10 cannot make an omitted normative decision executable, and
the plan's “block only the raising packet” rule permits M14 to certify while
those decisions remain unresolved.  
**Required resolution:** Reopen the source design. Accept versioned amendments
in every routed normative home and make their digests prerequisites of the
implementation plan and M14, including an explicit `GEN §13` disposition for
DP-051. Alternatively narrow the outcome and certification claims to the
implemented mechanism subset. Add a traceability gate mapping every principle
and finding to an accepted normative clause, implementation packet, and
executable proof.  
**Revalidation:** `just design-principle-traceability-check`

### F-002 — The LogicalPlan-only target cannot implement the governing query contract

**Severity:** blocker  
**Category:** design  
**Scope:** GI-2, WP62, M11, DB10; `QRY §§4.2–4.10, 15–17, 21, 30, 33, 106–107`  
**Finding:** `QRY` requires eight request forms, arbitrary acyclic
composition, prior-result roles, transitive traversal, bounded/shortest paths,
conjunctive graph patterns, fan-in/fan-out, explicit uncertainty/unknown and
coverage/absence semantics, and deterministic output. Its compilation model
is `DataFusion LogicalPlan and/or GraphOperatorPlan DAG`. GI-2 instead makes a
DataFusion `LogicalPlan` the only internal representation. WP62 specifies only
filter/projection/fetch binding and validates parity with the current SQL
path. Current `src/semantic_query.rs` implements only three forms, so that
differential can faithfully reproduce an already partial implementation while
still violating `QRY`. The packet itself admits that an inexpressible form
would reopen design, but defers that known design question until execution.  
**Required resolution:** Accept a query-compiler design before replanning. It
must define typed semantic IR, phrase binding, typed result roles, DAG
scheduling, all eight lowerings, graph/path/pattern execution ownership,
coverage/negative-proof effects, deterministic ordering, and the boundary
between DataFusion relational plans and application graph operators. Split
WP62 into dependency-closed packets and advertise unsupported forms until the
full conformance gate passes.  
**Revalidation:** `just semantic-query-conformance-check`

### F-003 — The draft cannot pass its governing artifact validator

**Severity:** blocker  
**Category:** factuality  
**Scope:** plan frontmatter, §1.3, WP54  
**Finding:** The plan declares
`state_path: docs/plans/state/codefabric-design-principles-full-alignment_v1_state.json`
and says no state exists “by design.” The current artifact validator requires
all declared `*_path` values to resolve; direct validation fails on that path.
Pointing `active-plan.json` at the draft before creating state would therefore
make governance invalid, while creating execution state during planning
conflicts with the implementation-plan workflow. The activation sequence is
not atomic or currently representable.  
**Required resolution:** Reconcile the artifact contract before approval:
either permit absent state only for inactive draft/audited plans and require
it atomically at activation, or create a non-execution planning-state artifact
defined by the schema. Specify and test the exact activation transaction so
there is no active-plan interval with an unresolved state path.  
**Revalidation:** `python3 -c 'from pathlib import Path; from tooling.ci.artifact_contracts import ROOT, validate_plan; validate_plan(ROOT, Path("docs/plans/codefabric_design_principles_full_alignment_implementation_plan_v1_2026-08-24.md"), verify_declared_inputs=True)'`

### F-004 — The 124-finding worklist was not re-derived and contains a false blocker premise

**Severity:** major  
**Category:** factuality  
**Scope:** conformance register, proposal §4, WP54–WP72  
**Finding:** The plan says drift moved line numbers “but not findings” after
spot-checking five anchors. That is false for at least DP-022. The conformance
register says `rg -c 'INSERT INTO update_wave' --glob '*.rs' .` returned zero,
but `git show d89cc90:src/lifecycle.rs` contains production inserts into both
`update_wave` and `update_wave_item`; HEAD still does, and the continuous
engine calls the persisted scheduler. WP66 therefore plans already-existing
work from a blocker premise that was false even at the register's stated
baseline. A five-anchor check cannot support packetization of 124 findings.  
**Required resolution:** Re-run every detector against the chosen baseline
and current tree. Publish a superseding register classifying every DP item as
open, partial, closed, invalid, or changed, with coverage-qualified detector
evidence. Regenerate proposal §4 and the packet-to-finding map from that
register before replanning.  
**Revalidation:** `just alignment-detector-check`

### F-005 — IPC-only transport is the wrong clean-sheet boundary and misstates alignment

**Severity:** major  
**Category:** design  
**Scope:** GI-3, WP59, WP60, DB11; ALIGN INT-01/INT-08; ARROW §10  
**Finding:** Tree-sitter and Ruff are in-process Rust adapters, yet GI-3 and
WP59/WP60 require every provider fact to serialize to IPC and decode again.
The principles and alignment manual distinguish Arrow's in-memory
`RecordBatch` boundary from serialized process/interchange protocols. The
universal IPC path adds copies, compression/backpressure work, and malformed
wire states without an interoperability boundary. WP59 also treats alignment
as protocol metadata and requires misaligned chunks to be rejected. Arrow
59.2 `StreamDecoder` defaults `require_alignment=false` and safely copies an
unaligned receive buffer; alignment is a local memory-address property, not
an encoded wire guarantee. Arbitrary chunk splitting is valid, and
`finish()` is required to detect truncation.  
**Required resolution:** Make validated `RecordBatch` streams the canonical
provider contract. Use bounded in-memory streams for in-process adapters and
validated `StreamDecoder` IPC only for rustc/Pyrefly process boundaries; both
must converge immediately above the same validator/reconciler. Keep
validation enabled, require `finish()`, and accept valid unaligned buffers via
the default copy path unless a benchmark and allocator contract justify a
stricter local policy.  
**Revalidation:** `just provider-protocol-check`

### F-006 — Cross-table referential integrity is enforced at the wrong lifecycle phase

**Severity:** major  
**Category:** design  
**Scope:** WP57, WP59, publication activation; `FAB §§66, 71.1`  
**Finding:** WP57 makes generated foreign keys “enforced,” while WP59 places
one cross-table validator above `ValidatedFactBatch::validate`. A partial
provider batch cannot determine whether a reference resolves against an
unchanged durable row, a co-arriving row, a replacement, or a tombstoned
target. Current publication code correctly has a candidate-publication
validation phase after constructing the effective table set. WP57 is also
not dependency-closed: its headline enforcement outcome is deferred to
WP59, and its own acceptance checks do not prove referential rejection.  
**Required resolution:** Keep row-local type, shape, and key checks at ingest.
Generate FK contracts in WP57, but enforce them over the complete candidate
effective snapshot after owner replacements/tombstones and before publication
CAS/activation. Move the runtime outcome and adversarial proof wholly to that
packet or merge the packets.  
**Revalidation:** `just publication-referential-integrity-check`

### F-007 — DataFusion 55 does not provide LD-04’s claimed extension semantics

**Severity:** major  
**Category:** library  
**Scope:** LD-04, WP58; DFREF S7; ARROW §26  
**Finding:** LD-04 claims DataFusion's registry makes planning and
`cast_to_type` honor `codefabric.id16`. In DataFusion 55, `DFExtensionType`
explicitly says the current customizable behavior is pretty-printing. Arrow's
`ExtensionType` validates/serializes metadata; DataFusion needs a separate
`DFExtensionType` plus registration factory. The pinned `cast_to` path
compares `DataType`, calls Arrow cast compatibility, creates an `Expr::Cast`,
and never consults the extension registry. A trait-existence compile probe can
therefore pass while the semantic claim is false. WP58's metadata-only
fallback also contradicts §1.1, GI-8, and the plan's own design-reopening
policy.  
**Required resolution:** Redesign the contract as an application-enforced
Arrow extension-metadata type over `FixedSizeBinary(16)`. Specify where
metadata is preserved, deliberately reattached, or rejected through
projection/cast/schema evolution. Add a separate DataFusion registration only
for behavior DF55 actually supports. If engine-enforced planning semantics
remain required, add an application planner/validator layer and prove it; do
not treat failure as a packet-local deviation.  
**Revalidation:** `just id16-extension-contract-check`

### F-008 — Ordering, result identity, and plan identity lack durable semantic contracts

**Severity:** major  
**Category:** design  
**Scope:** WP62, WP64, LD-03, LD-09; `QRY §33`  
**Finding:** WP62 implements fetch but omits the proposal's canonical
`SortExpr`; sorting encoded rows later for a checksum cannot make delivered
rows deterministic, and fetch before canonical sort can select different
subsets. WP64 calls `arrow-row` bytes canonical without defining schema
encoding, row framing/count, null/sort options, duplicate multiplicity,
Map-key order, float/NaN policy, zero-column batches, extension metadata, or
algorithm-version invalidation. Sorted rows form a multiset, not the stated
set. `arrow-row` does not promise a cross-version durable encoding. LD-09
likewise labels plan canonicalization “verified” without choosing a plan
phase or defining node/expression/table/function/parameter bytes. Finally,
query identity omits bound request parameters, so distinct predicates can
collide if the plan fingerprint is a reusable parameterized template.  
**Required resolution:** Add a versioned identity design. Specify canonical
per-form response sorting before offset/fetch; `ResultChecksumV1` schema and
length-framed multiset bytes with memory bounds and Arrow-version policy; a
precise plan-fingerprint phase/encoding; and separate plan-template identity
from semantic query identity containing canonical bound `QuerySpec` or
parameter bytes.  
**Revalidation:** `just query-determinism-check`

### F-009 — Persisting EXPLAIN ANALYZE can attribute a second execution to the first

**Severity:** major  
**Category:** library  
**Scope:** LD-06, WP65  
**Finding:** DataFusion 55's `AnalyzeExec::execute` runs every input partition,
consumes and discards all result batches, then produces the annotated plan.
Running native `EXPLAIN ANALYZE` after serving results would execute the query
again, so its metrics do not belong to the execution ID or rows/checksum
delivered to the caller. DataFusion already exposes
`DisplayableExecutionPlan::with_metrics`/`with_full_metrics(...).pgjson(...)`
for rendering metrics from the exact physical plan instance after execution.  
**Required resolution:** Keep ordinary `EXPLAIN` as a planning artifact, but
replace persisted `EXPLAIN ANALYZE` with a rendering of the served physical
plan's collected metrics. Explicitly prohibit diagnostic re-execution and
define partial artifact behavior for stream drop, cancellation, and failure.  
**Revalidation:** `just query-artifact-single-execution-check`

### F-010 — WP58 cites requested-statistics leverage without designing the path

**Severity:** major  
**Category:** library  
**Scope:** LD-05, WP58; ALIGN CAT-05–CAT-07  
**Finding:** `ScanArgs.statistics_requests` is a vocabulary threaded from a
`TableScan`; an application optimizer must request the statistics and the
returned physical plan must expose/consume them. Current overlay code
explicitly ignores the requests. WP58 does not name a request-producing rule,
the returned-plan representation, a consumer, cost/staleness policy, or the
relationship between replacement-batch and effective-table statistics. Its
claim that a materialized overlay gives an exact row count can overstate the
effective relation after base subtraction, tombstones, and replacements.  
**Required resolution:** Either narrow WP58 to truthful table-wide
`statistics()` and explicitly decline CAT-07, or design the complete
request/response/consumer path. Exactness must apply to the effective
relation; otherwise report `Inexact` or `Absent`, with one
`ColumnStatistics` entry per field and no planning-time I/O.  
**Revalidation:** `just provider-statistics-contract-check`

### F-011 — Oracle governance lands after, and would invalidate, its own proving oracles

**Severity:** major  
**Category:** proof  
**Scope:** WP54–WP72, WP70, M14  
**Finding:** WP54–WP69 can close using today's presence-only oracle test,
which accepts any literal occurrence. Alias detection, zero-match selector
protection, and acceptance-reference enforcement do not land until WP70.
WP70 then says this immutable plan's 76 `wp54_*`–`wp72_*` declarations are
the first population requiring per-oracle `AC-G-NN` references, but the plan
does not provide those mappings and not every plan-local criterion has an
AC-G owner. WP72 compounds the issue: its gates select legacy `wp48` and
`wp49`–`wp53` tests and omit any selector for the four promised `wp72_*`
oracles. A dead test can satisfy plan status without running.  
**Required resolution:** Move oracle schema, governed criterion mapping,
alias detection, and `--no-tests=fail` selector validation into WP54 and make
every later packet depend on it. Revise every oracle declaration before plan
approval using plan-criterion IDs mapped to AC-G, design, or conformance
authority as applicable. Add explicit selectors for each packet's four
oracles, including WP72, to the packet and milestone gates.  
**Revalidation:** `just oracle-substance-check`

### F-012 — Normative dependencies permit conflicting and semantically premature work

**Severity:** major  
**Category:** sequence  
**Scope:** WP56, WP62–WP69; §8 dependency graph  
**Finding:** The plan declares dependency edges normative and permits parallel
interleaving, while the linear default is only advisory. Several branches
share files and contracts without ordering: WP56 and WP69 both change
`repository_model.rs` and generated model authorities; WP62, WP65, and WP67
all change `query_service.rs` and request/artifact semantics; WP67 can harden
the boundary before the new query vertical/artifacts exist; and WP68 promises
production-surface adapter tests without depending on WP63's production
activation. The edge list therefore permits conflicts and proofs against the
wrong contract generation.  
**Required resolution:** Recompute the graph from contract ownership and
known-touch intersections. At minimum order WP56 before WP69, the accepted
WP62/M11 query contract before WP67, WP65 before boundary artifact/lease
hardening where they share schemas, and WP63 before WP68. Split shared-file
packets where genuine parallelism is desired.  
**Revalidation:** `just plan-dependency-check`

### F-013 — WP55 conflates application identity with integrity and security hashing

**Severity:** major  
**Category:** doctrine  
**Scope:** WP55, GI-1; `GEN §13`  
**Finding:** WP55 claims every digest construction belongs in
`crate::identity`, but its discovery searches only domain byte literals,
`digest_bytes`, and `blake3::Hasher`. Current coverage finds 124
`blake3::Hasher`/`blake3::hash` references outside the stated authority and
generated code, including keyed security tokens, content-integrity checks,
model tooling, lifecycle, publication, and artifacts. The proposed rule bans
only `Hasher`, so one-shot `blake3::hash` can bypass it. More fundamentally,
canonical semantic identity, integrity digests, and keyed authentication are
different authorities and threat models; centralizing all of them under
identity is not P3 alignment.  
**Required resolution:** Inventory all hashing structurally and classify each
domain by semantic identity, integrity, cache key, or security/MAC purpose.
Create narrow authorities and API boundaries for those purposes, then make
the generated registry own only application identities/fingerprints it can
correctly specify. Cover constructor, one-shot, imported, and renamed calls in
the zero-state rule.  
**Revalidation:** `just digest-domain-contract-check`

### F-014 — DataFusion set difference does not prove exact effective-state equality

**Severity:** major  
**Category:** proof  
**Scope:** WP72, AC-G-79  
**Finding:** WP72 says “DataFusion set-difference queries” prove convergence
without defining schema equality, distinct-versus-bag semantics, duplicate
counts, null/NaN behavior, or extension metadata. A distinct difference loses
multiplicity. DataFusion 55's `except(..., false)` deduplicates, while its
`is_all=true` path is not a general multiplicity-counting bag-difference
oracle. Bidirectional set difference can miss `{a,a,b}` versus `{a,b,b}`.
Rows alone also do not prove schema/metadata equality.  
**Required resolution:** Define AC-G-79 equality as exact versioned schema
fingerprint equality plus exact bag equality. For governed-key tables, prove
key uniqueness and compare full rows/counts; otherwise group canonical rows
with multiplicities or use the corrected checksum contract. Include base,
tombstone, and overlay effective-state construction on both sides.  
**Revalidation:** `just rebuild-equivalence-check`

### F-015 — Gate B requires an unmodeled accountable human acceptance action

**Severity:** major  
**Category:** operations  
**Scope:** WP71, Gate B  
**Finding:** WP71 says expected outputs are produced by the activated vertical
and “accepted by the owner,” but it has no candidate-generation phase,
reviewable diff, explicit pause, authorization artifact, signer/owner record,
or acceptance command. The same implementation can generate expected and
actual outputs and bless its own defect. Its rollback also treats released,
owner-accepted bytes as ordinary fixture-local edits, contrary to versioned
acceptance.  
**Required resolution:** Split candidate generation from accountable owner
review. Make owner acceptance a blocking external checkpoint captured in a
versioned artifact with candidate digest, authority, decision, and source
specification. Require an independent derivation/review oracle. Corrections to
released answers must create a superseding corpus version, never silently
revert accepted bytes.  
**Revalidation:** `just gate-b-owner-acceptance-check`

### F-016 — The dirty baseline and WP54 rollback are not trustworthy

**Severity:** major  
**Category:** impact  
**Scope:** baseline, WP54  
**Finding:** The plan records dirty digest `849b…` and says it covers only two
artifact-registration edits. The current tracked diff also contains
DataFusion-skill/routing edits, a deleted legacy reference, and
`seed_zero_state_check.sh`; several existed before the plan's mtime. The
recorded digest no longer matches the tree, and WP54 does not disposition all
changed/untracked/deleted paths. The plan also instructs deletion of an
untracked `skills/` duplicate and claims rollback is “revert the commit.” Git
cannot recover an untracked directory; it is already absent, so its removal
cannot be attributed from Git history. The fresh `ci-fast` baseline is red on
the plan/proposal's own typos.  
**Required resolution:** Recapture the complete planning tree and classify
every dirty path as plan-owned, separately owned, or excluded. Record a new
digest after the inputs and intended baseline edits are tracked. Require owner
disposition and a recoverable archive/manifest before deleting any untracked
tree. Record and clear the current Typos failures before any proving commit.  
**Revalidation:** `just audit-baseline-check`

## Target-Design Assessment

The proposal has strong goals—generated authority, truthful runtime states,
provenance closure, thin protocol adapters, and executable contract tests—but
the plan converts several of them into invariants before the underlying design
questions are settled. The largest example is the query engine: preserving
the current SQL-shaped relational implementation as a LogicalPlan-only target
is not the clean-sheet architecture implied by the semantic-query contract.
A typed semantic DAG with relational and graph execution backends is the
smaller coherent design because it directly represents the required request
forms and their dependencies.

Likewise, Arrow should be the canonical in-memory fact representation, not a
mandate to serialize every in-process call. Referential integrity belongs to
candidate publication state, not partial ingress. These corrections simplify
responsibility boundaries while improving correctness.

## Library Capability Assessment

The pinned Arrow/DataFusion choices are appropriate, but four plan decisions
overstate or under-specify their capabilities:

- DataFusion `LogicalPlanBuilder` is suitable for relational portions of the
  semantic compiler, not a demonstrated replacement for graph/path operators.
- DataFusion 55 extension registration resolves metadata-backed formatter
  behavior; it does not automatically add cast, optimizer, or execution
  semantics.
- `DisplayableExecutionPlan` can capture metrics from the served plan;
  `EXPLAIN ANALYZE` should not be rerun for governed provenance.
- `arrow-row` and DataFusion plan encodings are useful building blocks inside
  an application-versioned identity contract, not themselves durable
  canonical formats.

Arrow 59's `StreamDecoder`, fallible constructors, `RecordBatch` validation,
DataFusion provider pushdown/statistics APIs, and physical metrics should be
used, but only at the boundaries and semantic scope their contracts support.

## Work-Packet and Impact Assessment

WP62 is an omnibus design placeholder rather than an executable packet. WP57
claims an outcome deferred to WP59. WP70 retroactively governs already-closed
proofs. WP71 contains an unmodeled human decision. WP72's named gates do not
currently select its oracles. The normative dependency graph permits shared
contract/file edits in parallel even though the linearized example happens to
serialize them.

Before replacement packetization, re-derive the 124-finding register, build a
current symbol/consumer/persistence map, and partition work around accepted
contracts and lifecycle ownership—not around the current modules alone.

## Legacy, Transition, and Decommission Assessment

DB10–DB12 name useful negative end states, but their safety depends on the
redesign above. DB10 cannot delete the SQL path until all supported query forms
have a trustworthy target oracle. DB11 should delete bespoke fact DTOs without
forcing IPC onto same-process adapters. DB12 must follow the corrected
authority taxonomy rather than centralizing unrelated hashes. Untracked
artifact deletion also needs recoverable handling; a Git revert is not a
rollback for bytes Git never owned.

## Proof and Validation Assessment

The plan correctly asks for behavioral, structural, negative, and operational
oracles, but many names precede executable substance. Proof must be layered
from the first packet: non-vacuous selectors and criterion mappings first;
focused semantic contracts per packet; interaction tests at milestones; and
the full gate matrix at certification. A green broad gate cannot substitute
for eight-form query conformance, single-execution metrics, candidate-state FK
validation, duplicate-sensitive convergence, or accountable golden-answer
acceptance.

## Doctrine and Anti-Principle Assessment

The target advances P1/P3/P9/P20/P25 in intent, but currently violates the
same doctrine in execution:

- declaring routed normative work “fully aligned” is an unsupported claim;
- forcing one library representation onto graph semantics and one transport
  onto all boundaries is extension-level and protocol overreach;
- treating metadata registration as enforcement violates the advisory versus
  enforced distinction;
- a checksum or fingerprint without a versioned semantic contract creates a
  second authority; and
- self-generated golden answers and presence-only oracles are not tests
  derived from contracts.

## Top Required Changes

1. Amend and accept the routed normative design homes; narrow M14 if any
   remain unresolved.
2. Replace the LogicalPlan-only query target with an accepted typed semantic
   DAG design spanning all eight forms and relational/graph execution.
3. Re-derive all 124 conformance findings and the complete dirty baseline.
4. Correct the provider, publication-integrity, extension-type, metrics,
   statistics, checksum, and plan-identity designs using the pinned APIs'
   actual semantics.
5. Move oracle governance to the foundation, repair dependencies, and model
   the Gate B owner checkpoint explicitly.
6. Resolve the inactive-plan/state validator contract before activation.

## Re-Audit Scope

A replacement design and plan should be re-audited only after:

- every F-001/F-002 design decision is accepted in a versioned artifact;
- a superseding, detector-backed conformance register exists;
- the plan validates with fresh declared inputs and a trustworthy baseline;
- all 16 findings have an explicit disposition;
- the named revalidation recipes are defined and non-vacuous; and
- the revised dependency graph, decommission exits, and M14 evidence are
  regenerated from those decisions.
