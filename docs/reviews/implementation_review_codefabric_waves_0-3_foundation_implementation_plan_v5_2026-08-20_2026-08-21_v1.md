---
artifact: implementation-review
date: 2026-08-21
version: v1
status: complete
plan_path: docs/plans/codefabric_waves_0-3_foundation_implementation_plan_v5_2026-08-20.md
verdict: changes-required
---

# Implementation review: Wave 0–3 foundation plan v5 completeness

## Provenance and review scope

This read-only review assesses the complete current implementation against the approved v5
plan, its governing design, and its schema-2 execution state. The committed review point is
`5df28cd7f748d4be1a6302ac72c0af82ee19ac13`; the review also evaluates the current WP25
candidate in the worktree because that is the only implementation of the plan's final packet.
The candidate consists of 23 modified entries and five untracked entries, including
`src/fabric/serving.rs`, its governance rule/test, and two mutation-output directories.

The review does not require Ubuntu clean-checkout evidence, in accordance with the user's
accepted assurance deferral. It does not treat DO-01 or DO-02 as missing Wave 0–3 scope; the
plan explicitly assigns those outcomes to Waves 17 and 18.

The method combined plan/state/proving-commit derivation, current-tree inspection, targeted
searches for manual and legacy authority, design and doctrine comparison, DataFusion/Arrow and
delta-rs library-reference checks, focused acceptance tests, and current repository gates. No
production or test code was changed by this review.

## Executive summary

The implementation is close, but the plan is not complete. The execution state has 28 of 29
packets, four of five milestones, and all six decommission batches complete and trusted. WP25
and M04 are correctly still `not_started`; the current serving implementation is an
uncommitted candidate and its mandatory `just ci-fast` gate is red.

The largest issue is not bookkeeping. A `ServingSnapshot` can claim one source generation and
context set while its providers contain rows from another. Exact Delta versions, overlay
identity, and workspace manifest identity are bound, but the required
workspace/context/source-generation row scope is not bound to publication, candidate
activation, or provider scans. The existing test fixture directly demonstrates the gap by
successfully building a generation-2 candidate from batches whose generated
`source_generation` values are all 1.

Three additional gaps prevent WP25 acceptance: arbitrary SQL output and SQLite control capture
are fully materialized outside meaningful result/capture budgets; serving/control projection
topology remains manually enumerated despite the generated Contract IR; and the focused test
uses `MemTable` fixtures rather than proving real exact-version Delta pushdown, plan snapshots,
spill behavior, or cancellation. The current worktree also fails strict Clippy and has no
terminal mutation result.

## Verdict

**Changes required before WP25 or M04 can be marked complete.**

Waves 0–2 and the committed WP19–WP24/WP26 substrate do not require wholesale re-review. The
focused remediation reaches across WP20/WP22/WP24/WP26 only where fact scope must be carried
and bound into the final serving snapshot.

## Gate and evidence assessment

| Evidence | Current result | Assessment |
|---|---:|---|
| `just artifacts-check` | pass; 31 artifact/state tests | current plan/state contracts valid |
| `just plan-status` | pass; no stale inputs or untrusted complete entries | completed-packet provenance trusted |
| `just tracked-target-zero-state-check` | pass; zero tracked/reachable target paths | prior v4 IR-010 remains closed |
| `just compilation-units-check` | pass; four WP06a tests | prior v4 IR-013 remains closed |
| `just contracts-verify` | pass; 64 artifacts, zero warnings | current generated contract tree coherent |
| `just contracts-verify-released` | pass; 64 artifacts, zero warnings | M02 release posture remains green |
| `just governance-scan` | pass; eight rule suites | current structural rules pass |
| `just seed-zero-state-check` | pass | seed/packaging legacy remains absent |
| `just proof-coverage-check` | pass; 60 Tier-A proof atoms | proof graph is current |
| focused WP25 nextest slice | pass; 4/4 | useful but insufficient acceptance coverage |
| `just wave3-integration-check` | pass; 34/34 plus doctests | existing Wave-3 behavior is coherent |
| `just advisory-policy-check` | pass; four exact exceptions, owner M04 | live exception census matches policy; M04 disposition still required |
| `git diff --check` | pass | no whitespace defect |
| `just ci-fast` | **fail** | strict Clippy rejects `wp25_negative_zero_state` as 116 lines |
| WP25 mutation run | incomplete | first run missed 28/82 mutants; strengthened rerun stopped after 63/82 mutants |

The two attempted focused commands using the `contracts-tooling` feature selected zero WP25
tests because serving is daemon/data-fabric gated. The corrected default-feature selector ran
all four WP25 tests successfully; the zero-test attempts are reviewer command errors, not
product failures.

## Finding index

| ID | Severity | Dimension | Summary |
|---|---|---|---|
| IR-001 | blocker | identity / correctness / security | snapshot identity is not bound to fact-row scope |
| IR-002 | major | performance / operations / library leverage | result and control capture bypass bounded execution |
| IR-003 | major | architecture / extensibility / maintenance | serving projections retain manual sibling authority |
| IR-004 | major | tests / evidence / library leverage | WP25 does not prove real Delta pushdown or runtime limits |
| IR-005 | major | gates / provenance / diff hygiene | WP25 is unproved, uncommitted, and fails `ci-fast` |

## Findings

### IR-001 — Snapshot identity is not bound to fact-row scope

**Severity:** blocker

**Dimension:** identity / correctness / security

**Design references:** FAB §71.1 Durable base publication, FAB §71.2 Interactive
`ServingSnapshot` activation, FAB §91 `ServingSnapshot`-pinned overlay-aware catalog provider;
plan WP25 invariants I-02, I-05, and I-12.

**Evidence:**

- `PublicationPins` declares `workspace_id`, `source_generation`, and
  `analysis_context_set_id`, but publication manifests checksum complete pinned tables rather
  than selecting or validating their rows against those pins
  (`src/fabric/publication.rs:34`, `src/fabric/publication.rs:392`).
- An owner publication write is checked only against publication and workspace identity
  (`src/fabric/publication.rs:954`). `OwnerMutationRequest` carries neither analysis context nor
  source generation (`src/fabric/mutation.rs:128`). `ValidatedFactBatch` validates against a
  `FactScope` and then retains only table code plus batch, discarding the explicit scope
  (`src/fact_ingest.rs:1141`).
- `SnapshotProviderCatalog::build` opens an exact Delta version and applies the overlay, but
  installs no workspace/context/source-generation filter (`src/fabric/snapshot_catalog.rs:525`).
  `ServingSnapshotCandidate::validate_and_bind` checks publication, overlay, workspace manifest
  identity, versions, schemas, counts, and digests, but not row scope or context-set membership
  (`src/snapshot_runtime.rs:134`).
- `ServingQuerySession` registers those providers directly as `cpg_base`, and serving views
  project them without a scope predicate (`src/fabric/serving.rs:221`,
  `src/fabric/serving.rs:449`). This contradicts FAB §91's explicit requirement that each
  provider bind workspace, context, and source-generation filters below the user view.
- The fixture makes the defect executable: every generated `Int64` column, including
  `source_generation`, is 1 (`src/fabric/serving.rs:1040`), while `candidate` independently puts
  its argument into the manifest (`src/fabric/serving.rs:1061`). The pinned-query test
  successfully activates `candidate(..., 2, 2)` (`src/fabric/serving.rs:1237`). Its manifest
  therefore claims generation 2 over generation-1 rows. The fixture's context record list is
  also empty even though fact rows carry a context ID (`src/fabric/serving.rs:966`).

**Failure consequence:** a validated snapshot can claim one source/context identity while
queries return another fact state. This breaks snapshot determinism and provenance and can
become a cross-context disclosure when multiple analysis contexts coexist in one workspace
fabric. Exact Delta version pinning does not repair the mismatch; it only makes the wrong row
set repeatable.

**Required remediation:** retain a closed scope object on every `ValidatedFactBatch` and owner
mutation, and validate publication writes against the publication's workspace, source
generation, and resolved context-set membership. Bind the selected scope below all
user-controllable catalog/view layers—either with snapshot-scoped `TableProvider` wrappers that
inject typed `Expr` predicates into `scan`, or with a design-owned publication layout that is
already scope-exclusive. Activation must independently reject any provider census whose rows
do not match the manifest scope. Add the scope selection and context-set membership model to
the governing Contract IR rather than reconstructing it from row names.

**Focused re-test:** add two-workspace, two-context, and two-generation publication/snapshot
fixtures. Prove mismatched publication pins and candidate manifests fail; prove raw `cpg_base`
and all `cpg_serving` views return only selected rows; prove an active-pointer swap cannot alter
the leased scope. Run the new `publication_scope`, `snapshot_scope`, and `wp25_scope` tests plus
`just wave3-integration-check`.

### IR-002 — Result and control capture bypass bounded execution

**Severity:** major

**Dimension:** performance / operations / library leverage

**Design references:** FAB §98 DataFusion runtime policy, FAB §110 Plan artifact bundle, plan
WP25 bounded-session outcome and operational acceptance; holistic principles 22 and 28.

**Evidence:** `ServingRuntimeConfig` bounds the DataFusion memory pool and spill directory but
defines no result row/byte/batch limit or operational-capture budget
(`src/fabric/serving.rs:90`). `execute_plan` calls DataFusion `collect`, retaining every output
batch, and then `concat_batches`, allocating another complete result representation solely to
compute a checksum (`src/fabric/serving.rs:291`). The SQLite path first collects every matching
row as `Vec<Vec<rusqlite::types::Value>>`, then creates another column-oriented Arrow
representation and `MemTable` (`src/fabric/serving.rs:539`, `src/fabric/serving.rs:580`). These
allocations are outside the large-operator reservations governed by the configured DataFusion
pool.

The same issue occurs earlier in snapshot construction. `SnapshotProviderCatalog::build`
collects and concatenates each complete wrapped provider to derive primary-key/content evidence
(`src/fabric/snapshot_catalog.rs:546`), then `validate_provider_record` collects the same
provider again (`src/fabric/snapshot_catalog.rs:775`). `provider_batch` executes those scans
through a fresh default `SessionContext` with no service memory pool
(`src/fabric/snapshot_catalog.rs:707`). Snapshot activation therefore performs two full-table
materializations per pinned table even though publication already computed the governed row
count and checksum.

The pinned DataFusion reference is explicit: `collect` buffers total output and is unsuitable
for arbitrary service SQL; `execute_stream` is the service/export primitive and provides
backpressure and cancellation (`datafusion_rust` §10.16, §10.18, and §21). Its memory reference
also notes that normal `RecordBatch` values and transient allocations are not fully accounted
by `MemoryPool` (`datafusion_rust` §28). Merely observing a `GreedyMemoryPool` therefore does
not make these two materialization paths bounded.

**Failure consequence:** candidate construction scales as two full reads of every pinned table,
and a read-only query or large operational table can exhaust process memory despite the
advertised service limit. Checksum construction can nearly double materialized memory. Spill
settings do not protect any of these paths.

**Required remediation:** compute primary-key/content evidence once at the publication boundary
and persist it in the typed publication manifest; exact version plus schema/manifest identity
should let snapshot construction bind a provider without re-reading table contents. If a
content scan remains required, perform one bounded streaming validation pass, not two
`collect`/concatenate passes. Expose a `SendableRecordBatchStream`, or place an explicit
generated row/byte/batch result budget before any bounded collector. Hash ordered output
incrementally, or use a bounded spill-aware checksum path when order-independent hashing is
required; do not concatenate all output. Capture SQLite rows directly into bounded Arrow
batches, reserve their memory as a named DataFusion `MemoryConsumer`, and fail closed at the
contract budget. Prefer DataFusion's built-in consumer tracking, and document whether
`FairSpillPool` is required for queries with multiple spillable operators.

**Focused re-test:** prove snapshot construction performs no fact-table scan after publication
(or exactly one bounded streaming pass if the design retains revalidation); force a result over
each row/byte/batch limit; force an operational capture over its budget; stream a result under
backpressure; and drop a stream to prove cancellation. Assert stable failure classes, released
reservations, bounded temporary storage, and no `collect`/`concat_batches` path in the
arbitrary-query service or candidate-construction path.

### IR-003 — Serving projections retain manual sibling authority

**Severity:** major

**Dimension:** architecture / extensibility / maintenance

**Design references:** FAB §13.12 Operational read-only views, FAB §92 Stable serving views,
plan WP25 structural acceptance, holistic principles 10, 14, 26, and 31.

**Evidence:** the current WP25 design correction correctly adds a closed generated
`workspace_scope` to the schema Contract IR. The runtime registry already exposes
`operational_table_specs()` (`src/schema_registry.rs:415`). Nevertheless:

- `CONTROL_TABLES` repeats the nine projected table names in runtime code
  (`src/fabric/serving.rs:38`);
- `build_serving_schema` repeats the four table-code/view-name pairs
  (`src/fabric/serving.rs:449`), even though the plan requires view eligibility to follow
  `materialization_role` only
  (`docs/plans/codefabric_waves_0-3_foundation_implementation_plan_v5_2026-08-20.md:3866`);
- `install_derived_control_views` manually owns two view names and their column lists
  (`src/fabric/serving.rs:721`); and
- the Contract-IR validator repeats a hard-coded required table-name set rather than validating
  a declared projection role (`src/contracts/schema_models.rs:392`). It also checks that scope
  column names exist but not that workspace columns are non-null binary IDs or that child and
  parent join-key types are compatible (`src/contracts/schema_models.rs:343`).

**Failure consequence:** adding or renaming an eligible table or control projection requires
coordinated edits to the IR, validator, generator, runtime constants, derived-view code, and
tests. A model change can compile while its projection is silently omitted, and malformed
scope joins can remain representable. This is the same manual-authority churn the remediation
plan was intended to remove.

**Required remediation:** extend the schema Contract IR with closed serving and control
projection records: stable view name, source table identity, availability wave, projection
role, explicit derived projection where needed, and typed workspace-scope contract. Generate
the four Wave-3 serving specs, control-source census, and derived control projections. Continue
deriving hidden-column exclusion and enum joins from field metadata. Make type/nullability/key
compatibility validation model-based. Runtime code should iterate generated projection specs;
it should not own table-name, table-code, or column crosswalks.

**Focused re-test:** mutate the model with one valid eligible table/view and prove generation
and runtime discovery change without editing `serving.rs`; reject duplicate view names,
incompatible join keys, nullable/non-ID workspace columns, and unowned projections. Add a
zero-state rule for the superseded constants and table-code/view-name literals.

### IR-004 — WP25 does not prove real Delta pushdown or runtime limits

**Severity:** major

**Dimension:** tests / evidence / library leverage

**Design references:** FAB §112.4 DataFusion tests, FAB §110 Plan artifact bundle, plan WP25
behavioral and operational acceptance
(`docs/plans/codefabric_waves_0-3_foundation_implementation_plan_v5_2026-08-20.md:3855`);
holistic principles 27 and 30.

**Evidence:** all four WP25 tests build their source catalog with
`SnapshotProviderCatalog::from_batches_for_snapshot_tests`, whose providers are `MemTable`s
with fabricated Delta version 1 (`src/fabric/snapshot_catalog.rs:453`). The behavioral test
only checks that plan strings contain `Filter` and `Projection`
(`src/fabric/serving.rs:1190`); that does not show either expression reached a real
exact-version Delta/Parquet scan. Repository search finds no committed logical or physical
plan snapshot and no `insta` assertion for WP25. The operational test observes configured
memory/spill values and three hand-built counters but does not force spill, reject an
over-limit query, cancel execution, or consume DataFusion's physical metrics
(`src/fabric/serving.rs:1453`).

The delta-rs reference requires an actual local Delta table, pinned provider, filtered and
projected queries, and `EXPLAIN` in its integration matrix; its diagnostics specifically say
to verify that the Delta provider appears and projection/filter pushdown reaches the scan
(`deltalake_rust` §6.19, §6.30, and §7.8). DataFusion's plan-artifact guidance recommends
version-pinned logical and physical golden snapshots (`datafusion_planning_rust` §55).

**Failure consequence:** the named WP25 oracle passes even if serving never uses the production
Delta provider, if pushdown stops at a `ViewTable`, or if memory/spill/cancellation behavior is
non-functional. The emitted `execution_metrics` map proves only post-hoc counters chosen by
CodeFabric, not operator-level execution evidence.

**Required remediation:** construct the WP25 conformance fixture through the production path:
local Delta tables, publication, `SnapshotProviderCatalog::build`, candidate activation, lease,
and `ServingQuerySession`. Record normalized, version-pinned unoptimized/optimized/physical
plan snapshots. Assert projected scan width, pushed/residual filter placement, exact Delta
versions, and provider reuse. Add forced low-memory spill-or-stable-rejection and cancellation
tests, and capture DataFusion `MetricsSet`/`EXPLAIN ANALYZE` evidence including spill bytes where
available.

**Focused re-test:** run a new `wp25_delta_serving_acceptance` oracle, review the committed
logical/physical snapshots, run `just snapshots-review` in non-mutating review mode, and rerun
`just wave3-integration-check`.

### IR-005 — WP25 is unproved, uncommitted, and fails the mandatory gate

**Severity:** major

**Dimension:** gates / provenance / diff hygiene

**Design references:** plan WP25 packet-local gates and M04 exit; schema-2 execution-state
contract.

**Evidence:** the authoritative state records WP25 as `not_started` with no proving commit
(`docs/plans/state/codefabric-waves-0-3-foundation_v5_state.json:659`) and M04 likewise
(`docs/plans/state/codefabric-waves-0-3-foundation_v5_state.json:724`). This is correct:
`src/fabric/serving.rs` and its governance files are untracked,
while generated contracts, design, schema runtime, and snapshot catalog changes are modified.
`just ci-fast` currently stops at strict Clippy because `wp25_negative_zero_state` is 116 lines,
over the repository's 100-line limit (`src/fabric/serving.rs:1333`).

The first complete mutation census produced 28 missed, 33 caught, 19 unviable, and two timeout
outcomes over 82 mutants (`mutants.out.old/outcomes.json`). Tests were strengthened afterward,
but the replacement run was stopped with only 63 mutants classified (44 caught and 19
unviable, plus the baseline success) in `mutants.out/outcomes.json`. That partial run is
encouraging but cannot establish zero surviving mutants. Both mutation directories remain
untracked worktree output.

**Failure consequence:** neither the packet nor the Wave-3 milestone has current proof at an
ancestor commit, the mandatory routine gate is red, and the risk-triggered test-strength
obligation has no terminal result.

**Required remediation:** resolve IR-001–IR-004, factor the oversized negative oracle without
reducing assertions, rerun the scoped mutation census to terminal completion, and remove or
archive transient mutation output outside governed source. Run the complete WP25/M04 matrix,
including `just ci-fast`, `just wave3-integration-check`, `just contracts-verify`, the relevant
feature matrix, plan snapshots, and the M04-owned advisory review. Commit the proved candidate,
then record the WP25 and M04 proving commits in schema-2 state.

**Focused re-test:** `just ci-fast`; `just wave3-integration-check`; `just contracts-verify`;
`just features-each`; terminal `just mutants-file src/fabric/serving.rs`; `just
advisory-policy-check`; `just artifacts-check`; and `just plan-status` at the final proving
commit.

## Outcome and invariant matrix

| Outcome or invariant | Assessment | Evidence |
|---|---|---|
| schema-2 plan/proving-commit trust | conforming for completed packets | `just artifacts-check`; `just plan-status` |
| tracked build-output zero state | conforming | `just tracked-target-zero-state-check` |
| first-class compilation units | conforming | `just compilation-units-check` |
| Wave 0 four-domain foundation | conforming at proving commits | M01 complete; plan-status trusted |
| Wave 1 released model/contract foundation | conforming | 64 released artifacts, zero warnings |
| Wave 2 source-instance control plane | conforming at proving commit | M03 complete; no current finding reopens its topology |
| WP19–WP24/WP26 Wave-3 substrate | substantially conforming | current 34-test Wave-3 gate; scope binding reopened by IR-001 only |
| leased exact-version provider reuse | conforming | WP26 oracles; pointer-identity WP25 test |
| snapshot scope identity | non-conforming | IR-001 |
| bounded query/control execution | non-conforming | IR-002 |
| model-driven serving projections | partially conforming | generated fields/scopes, but IR-003 manual topology remains |
| real Delta pushdown/plan evidence | non-conforming | IR-004 |
| WP25 packet completion | non-conforming | state not started; `ci-fast` red |
| M04 Wave-3 exit | non-conforming | WP25 and final evidence open |
| Ubuntu clean-checkout | user-deferred assurance | explicitly not a blocker |
| DO-01 / DO-02 | correctly deferred | Waves 17 and 18, outside this plan's product completion |

## Architecture and doctrine assessment

The committed foundation now follows the model-based architecture far more faithfully than
the v4 review point. It has a typed catalog and compilation-unit graph, generated schema and
operational-store contracts, dual identities, a single Proto FDS, generated adapter models,
closed bundle models, immutable exact-version provider sets, and schema-2 proof state. The
prior v4 IR-010–IR-013 findings are closed by current executable evidence.

The WP25 candidate also makes sound library-native choices: `SQLOptions` owns the coarse
read-only statement policy; `ViewTable` and logical plans own derived views; the bounded
`RuntimeEnv` replaces DataFusion's unbounded default; immutable catalog/schema providers reject
mutation; and leases retain pointer-identical snapshot providers.

IR-001 violates stable semantic identity and trust-boundary doctrine because the manifest and
served row set can disagree. IR-003 violates declarative single-sourcing and additive
extensibility because new projections require edits to runtime crosswalks. IR-002 and IR-004
weaken resource lifecycle, observability, and testability. These are targeted seam defects,
not evidence for a different crate topology or a new service.

## Library leverage assessment

The implementation should retain its use of exact-version delta-rs providers,
`SnapshotProviderCatalog`, `ViewTable`, `SQLOptions`, `RuntimeEnvBuilder`, and Arrow field
metadata. The next correction should use more of the libraries rather than add custom loops:

- DataFusion `execute_stream` / `SendableRecordBatchStream` for service output, with stream drop
  as cancellation and natural backpressure;
- `MemoryConsumer`/`MemoryReservation` and consumer-tracking pools for non-operator capture;
- provider-level typed `Expr` scope injection below views;
- DataFusion physical `MetricsSet` and `EXPLAIN ANALYZE` for spill/row/operator evidence;
- exact-version Delta providers and scan plans for pushdown proof; and
- version-pinned `insta` plan snapshots rather than substring assertions.

No timing benchmark is required. All recommended evidence is functional: scope, limits,
streaming, cancellation, plan shape, spill/rejection, and deterministic artifacts.

## Legacy and decommission assessment

All six v5 decommission batches are recorded complete. Current targeted checks independently
confirm the tracked-target, compilation-unit/manual-output, and seed zero states. No old
orjson, dual-protoc, independent adapter-schema authority, or seed native-extension path needs
to be restored.

The only worktree hygiene issue is transient mutation output. It is not a product authority,
but it must not be committed as source or cited as successful evidence. The new
`workspace_scope` design/model change is a legitimate WP25 correction; once the full scope and
projection model is settled, regenerate dependent artifacts programmatically and record the
accepted design deviation in WP25 state.

## Test and operational assessment

The current focused tests meaningfully prove immutable catalogs, statement rejection, hidden
columns, generated enum-name projection, a barrier-synchronized pointer swap, and current
control-store consistency labeling. Those are valuable and should remain.

They need four additions: adversarial scope isolation, bounded streaming/capture, production
Delta pushdown and plan goldens, and functional spill/cancellation. Mutation testing should be
rerun only after those root causes are fixed; the partial rerun must not be extrapolated to a
passing census.

## Plan deviations and diff hygiene

The candidate adds two design clarifications to FAB: generated operational workspace scopes
and the four-view Wave-3 conformance subset. Both are directionally correct. The first is
incomplete until scope typing and fact-provider binding cover the whole snapshot identity; the
second should be expressed as generated projection data rather than only prose plus runtime
literals.

`git diff --check` is green. Current contract generation is coherent and released verification
has zero warnings. The source diff is nevertheless not completion evidence until its untracked
files are committed, transient outputs removed, strict Clippy passes, and state records a
proving commit.

## Required remediation order

1. Close IR-001 by freezing the row-scope model and its publication/provider ownership; this is
   the only blocker and can change the shape of the remaining work.
2. Extend Contract IR for serving/control projections and explicit query/capture resource
   budgets; regenerate all dependent artifacts.
3. Replace unbounded collection with bounded streaming/capture and provider-level scope
   enforcement.
4. Rebuild WP25 acceptance on the production local-Delta path with plan snapshots,
   pushdown, spill/rejection, cancellation, and cross-scope negatives.
5. Factor the oversized test and finish the scoped mutation census with explicit dispositions.
6. Run the full packet/milestone matrix, review the four M04-owned advisory exceptions, commit,
   and reconcile WP25/M04 proving commits.

## Focused re-review scope

A focused re-review need not reopen unrelated Wave 0–2 packets. It should verify:

- publication-to-manifest-to-provider row-scope closure across WP20/WP22/WP24/WP26/WP25;
- absence of manual serving/control projection crosswalks;
- bounded streaming and operational capture under low resource limits;
- real exact-version Delta projection/filter pushdown and committed plan snapshots;
- cancellation and spill-or-stable-rejection behavior;
- terminal mutation evidence with no unexplained survivors/timeouts;
- green `ci-fast`, Wave-3, feature, contract, governance, advisory, artifact, and plan-status
  gates at the WP25/M04 proving commit.
