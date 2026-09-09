---
artifact: process-assessment
date: 2026-09-08
version: v1
status: complete
scope: development process (skills, _shared policies, AGENTS.md), the active plan v3 and its source review, and the composition of the codebase relative to the CPG product target
baseline_commit: 0cc7242
---

# Pragmatic delivery of the Python/Rust CPG: process and architecture assessment

## 0. How to read this document

This is an **assessment and a set of recommendations**, written at the user's request on
2026-09-08. It is not an implementation plan, not a design dossier, and not an authorization
to delete or restructure anything. The user decides which recommendations to adopt; an agent
asked to act on this document should confirm which numbered recommendations (R1 through R9)
are in scope before touching code, and should read §8 (interactions with existing artifacts)
first.

Every quantitative claim below was derived from the repository at the baseline commit with
the commands in Appendix A. Re-run them rather than trusting the numbers; they will drift.
Citations of the form `F01` through `F14` refer to the prioritized findings in
`docs/designs/codefabric_real_time_cpg_comprehensive_review_design_v1_2026-09-04.md` §1.3.
Citations of the form `WP79`, `M18`, `DB24` refer to the active plan
`docs/plans/codefabric_real_time_cpg_implementation_plan_v3_2026-09-07.md`.

The question the user asked, paraphrased: *the skills and AGENTS.md stipulations about
proving intermediate results and generating artifacts are producing excess work and excess
code; the end result over a codebase snapshot is externally auditable and the pipeline can be
argued correct on theoretical grounds; what would a more pragmatic and effective approach look
like, holistically, across skills, doctrine, the plan, the review, and the way the design
target is being realized?*

**Short answer.** The user's premise is correct, and the evidence is starker than the framing.
The process has inverted the build order so that roughly 70% of the first-party Rust is a data
fabric proving properties of facts that have not been produced yet; the only executable query
form resolves three phrases to Python function entities; and the current blocker escalated,
within four days, into vendoring and patching fifteen upstream crates including tokio. The
remedy is not to remove rigor but to relocate it: prove correctness at the product boundary
against a golden corpus, build the facts before the fabric, ship a small useful subset, and
cut the planning and artifact machinery by an order of magnitude.

---

## 1. Evidence

### 1.1 Timeline and throughput

| Measure | Value |
|---|---|
| First commit | 2026-08-19 |
| Baseline commit | `0cc7242`, 2026-09-08 |
| Commits | 475 |
| Commits per ISO week (33, 34, 35, 36) | 114, 250, 100, 11 |
| Lines ever added to `src/` | 547,295 |
| Lines ever deleted from `src/` | 262,143 |
| Net `src/` | 285,152 |

Reading: almost half of everything written into `src/` has since been deleted. That is not
refactoring churn on a stable design; it is the cost of successive plans declaring the previous
plan's output "legacy" and requiring its removal with zero-state proof (see §1.5).

### 1.2 Code composition at baseline

| Area | Lines | Notes |
|---|---|---|
| `src/` total | 266,958 | 173 files |
| `src/` outside `#[cfg(test)]` | 176,024 | |
| `src/` inside `#[cfg(test)]` | 90,934 | 34% of `src/`; 545 `#[test]` functions, about 167 lines per test |
| `src/fabric/` | 124,877 | activation, Delta, proof, admission, sessions, resources, registries |
| Semantic core (see below) | ~70,400 | provider adapters, analyses, identity, query contract |
| Fabric and top-level infrastructure | ~173,000 | everything else in `src/` |
| `tests/` | 5,520 | one integration target, 59 tests |
| `rustc-extractor/` | 3,677 | |
| `pyrefly-sidecar/` | 4,657 | |
| `codefabric-cpg-mcp/` Python | 7,516 | 4,707 in `src/` |
| `tooling/ci/*.py` | 12,008 | plan-packet proof dispatchers and artifact validators |
| `third_party/native/` | 712,547 | vendored and patched upstream Rust, added 2026-09-08 |
| `justfile` | 1,398 | 226 recipes; 101 in the `test` group, 46 in `gate` |
| `rules/` | 618 | 31 ast-grep boundary rules |
| Public `struct`/`enum` definitions in `src/` | 1,874 | versus 1,658 public functions: a type-heavy, DTO-heavy codebase |

"Semantic core" was measured as the union of: the Tree-sitter and Ruff adapters, the Pyrefly
and rustc service modules, the four `*_derived_analysis.rs` files, `provider_native_syntax.rs`,
`identity.rs`, `python_context.rs`, `source_image*`, `git_state.rs`, the relational semantic
query and query-contract modules, `relational_program.rs`, `fabric/graph_program.rs`, and
`analysis_context/`. Everything else in `src/` was counted as fabric or infrastructure. The
split is approximate; the ratio is not sensitive to reasonable reclassification.

The largest single files are instructive: `programmatic_derived_analysis.rs` at 11,404 lines,
`relational_semantic_query.rs` at 7,425, `query_service.rs` at 5,835,
`fabric/programmatic_schema.rs` at 5,751, `fabric/derived_producer_closure.rs` at 5,481, and
`supervisor.rs` at 5,202.

### 1.3 Product state today

What an agent can actually do with CodeFabric at baseline, per the 2026-09-04 review and
re-checked by grep at this baseline:

| Capability | State | Evidence |
|---|---|---|
| Query forms declared | 8 | `src/semantic_query_contract.rs` `QueryForm` |
| Query forms compiled into an executor | 1 | `production_query_recipe.rs:678` `compiled_find_entities_program`; F06 |
| What that one form does | resolves three phrases to `entity_kind = function` over Ruff bindings | F06 |
| MCP surface | 4 tools, 2 resources | `codefabric-cpg-mcp/src/codefabric_cpg_mcp/server.py` |
| Live file watcher / dirty registry | not installed | F01 |
| "Continuous update" proof | compares two separate daemon startups | F01, `tests/integration/daemon.rs` |
| Pyrefly sidecar on the production route | no | F03 |
| rustc extractor on the production route | no | F01, F04 |
| Python control flow | AST visitation order labeled `complete` | F02 |
| Persistence | every relation rewritten with `ReplaceAll` per epoch | F09 |
| `just ci-fast` baseline | red since 2026-08-31, exit 101, `E0631` in `src/provider_sandbox.rs:832` | `target/agent/baseline.json` |

The review's own executive decision (§1) says the largest improvement is "connecting source
change, effective compiler configuration, truthful semantic analysis, selective publication,
and all eight query forms into one continuously running product." That is a description of a
product that does not exist yet, written after 190,000 lines of first-party Rust.

### 1.4 The process load stack

Lines of process prose an executing agent is instructed to read before it reads the plan:

| Document | Lines |
|---|---|
| `docs/library_ref/full_data_fabric_design_principles_v2.md` (36 principles) | 2,189 |
| `AGENTS.md` | 792 |
| `.claude/skills/_shared/code-intelligence.md` | 387 |
| `.claude/skills/impl-plan-exec/SKILL.md` | 363 |
| `.claude/skills/_shared/artifact-schemas.md` | 329 |
| `.claude/skills/_shared/evidence-policy.md` | 144 |
| `.claude/skills/_shared/validation-policy.md` | 123 |
| `CLAUDE.md` | 71 |
| `.claude/skills/_shared/doctrine-policy.md` | 56 |
| Subtotal before the plan | ~4,450 |
| Plan v3 | 2,783 lines, 25,922 words |
| Source review | 621 lines, 11,558 words |
| Repository specification cited throughout `AGENTS.md` | 3,676 lines, 97 sections |

The 21 skills total 6,118 lines of `SKILL.md` plus 1,039 lines of shared policy. The eleven
library-reference navigators are pointers into `docs/library_ref/` and are cheap; the ten
workflow skills are the expensive ones.

### 1.5 The plan lifecycle

| Plan family | Versions | Terminal state summary |
|---|---|---|
| waves 0-3 foundation | 5 | v1: 5 invalidated, 22 not started; v5: 29 complete |
| waves 4-7 core facts | 5 | v1: 27 invalidated; v5: 27 complete |
| waves 8-12 semantic profiles | 2 | v2: 9 complete, 29 not started |
| model-based foundation remediation | 1 | 8 complete |
| model-driven artifact control plane | 1 | 15 complete |
| data fabric (DataFusion 55 / Arrow 59 / Delta pin) | 1 | 6 complete |
| design-principles full alignment | 3 | v3: 23 complete |
| full-alignment review remediation | 4 | v4: 8 complete, 2 blocked |
| ontology-compiled data fabric | 3 | v2: 17 in progress; v3: 8 stale |
| execution-proved relational data fabric | 7 | v2: 19 in progress, 8 invalidated; v7: 2 complete, 4 in progress, 8 stale |
| real-time CPG | 3 | v2: 28 invalidated after 3 days; v3: 1 in progress, 27 not started |

Eleven plan families and 35 plan versions in 20 days, 5.7 MB of plan text, plus 1.7 MB of
reviews and audits and about 1 MB of design dossiers. Several families were superseded before
their first milestone. The pattern is not "plan, execute, finish"; it is "plan, execute two or
three packets, discover a fact, reopen the design, re-plan with new IDs, mark the old packets
invalidated or stale, decommission the old code."

Tests inherit the same identity problem: 340 test functions in `src/` and `tests/` are named
after work packets (`wp44_beh_...`, `rt_cpg_wp80_...`). When a plan is invalidated, its tests
keep running but their names no longer describe anything.

### 1.6 The WP79 case study

This is the clearest single illustration of the failure mode, reconstructed from the commit
log, the v2 status report, and plan v3.

1. **2026-09-04.** Plan v1 is activated. WP77 (truthful capability claims) and WP78 (source
   inventory and context contracts) are proved. WP79, "install aggregate resource ownership
   and joined operation primitives", begins.
2. **2026-09-04 to 2026-09-07.** WP79 requires that every material retained allocation be
   admitted before work. delta-rs's eager snapshot retains private Arrow batches behind a
   crate-private accessor; the Delta kernel decodes CRC and schema JSON with plain `serde`
   calls; tokio does not expose task allocation. The status report v2 concludes that the
   accepted design's D-RT05 contract cannot be satisfied without library changes.
3. **2026-09-07.** Under the process, that is a design reopening. A planning-contracts design
   v2 adds LD-RT09, "native resource correction". Plan v3 is authored, audited, and activated.
   All 28 remaining v2 packets are marked invalidated and re-minted.
4. **2026-09-08.** The user has to intervene in writing, at the top of plan v3, to stop a
   "bespoke content-addressed source bundle, ordered patch replay, pre-Cargo artifact
   verification, or a synchronized eight-document suite successor" from being a prerequisite
   to editing a dependency.
5. **2026-09-08.** Twelve upstream source trees covering fifteen crates are imported under
   `third_party/native/` and selected through `[patch.crates-io]`: six arrow crates, parquet,
   three Delta-kernel crates, four delta-rs crates, and tokio. That is 712,547 lines of Rust
   the repository now compiles, formats, lints, and is responsible for keeping in step with
   upstream.

At the end of this sequence, find-references still does not work in either language. In a
pragmatic shop the WP79 question has a one-line answer: the process is capped by an RSS
watchdog and DataFusion's memory pool governs query execution; delta-rs's internal allocations
are not accounted, and that is recorded as a known limitation.

### 1.7 What the specification demands

The eight-document v2.3 suite is 7,171 lines. It requires eight query forms with composition
DAGs, exact provider batches, self-observed proved epochs, exact Delta histories with CDF
retention, Python object, memory, effect, resource and async analyses, Rust MIR-derived
ownership, flow, effect and state analyses, and dependency-driven SCC summaries. Plan v3 §1.2
states that "unimplemented required functionality is not an uncertainty waiver." There is no
defined useful subset anywhere in the suite, the roadmap, or the plans.

---

## 2. Diagnosis

Six findings. Each names the mechanism, so that a remedy can target it.

### D1. Proof is applied at every internal boundary instead of at the product boundary

Plan v3 requires, for each of its 30 packets, four named oracles (behavioral, structural,
negative, operational), packet-local gates, zero-state decommission proof for the code it
replaces, and a proving commit. That is 120 new tests before any of them asks the only
question that matters to a user: *does the tool answer this question about this repository
correctly, and does it say what it has not yet processed?*

The user's intuition here is sound and can be stated precisely. The fact pipeline is a
composition of deterministic, per-file functions: bytes → parse → provider facts → normalized
facts → relations. For such a composition, correctness of the final relations over a fixed
snapshot implies correctness of every intermediate stage *for the inputs that snapshot
exercises*. Testing the stages separately adds value only where the end-to-end oracle cannot
localize a failure, or where a stage is not a pure function of the snapshot. In this system
there is exactly one such stage: incremental invalidation (which downstream facts to recompute
when a file changes). That stage has a cheap, complete oracle: after any edit sequence, the
incremental state must equal a clean rebuild. One differential test covers it.

### D2. The fabric was built before the facts

About 173,000 of 267,000 first-party lines are the fabric: activation transactions, exact
Delta reads and guarded writes, proof of epochs, CDF replay and checkpoints, admission and
capacity accounting, streamed-result registries, child-session authorization, leases, command
actors and SQLite command records. About 70,000 lines are providers and analyses, and most of
those are not wired to the production route (F01, F03, F04).

Every one of the fabric concerns is real for a mature system. None of them can be designed
well without a workload, and there is no workload because there are no facts. Exactness,
incrementality and resource bounds are being solved in the abstract, which is the most
expensive way to solve them, and the code that results has to be maintained through every
subsequent re-plan.

### D3. The planning machinery consumes the budget that would build the product

Immutable plans, versioned successors, declared-input digest tables, confirm-gated plan
activation, a state schema with judgment fields, plan audits, audit integration with
per-finding dispositions, and status reconciliation all exist to make agent claims
checkable. The cost is that every discovered fact becomes a design reopening, and every
reopening costs a 100 to 250 KB plan plus an audit plus an integration pass plus a state
migration. §1.5 shows the result: 35 plan versions in 20 days, most invalidated within days.

The tooling that enforces this is itself 12,000 lines of Python in `tooling/ci/`, plus the
artifact-schema section of `_shared/`, plus `just artifacts-check`, `just plan-status`, and
the packet dispatcher recipes. That is a second product being maintained alongside the first.

### D4. Doctrine intended for the product has been applied to every internal seam

The eight product doctrines in `AGENTS.md` §0.2 are good and should survive: facts not
judgments; absence is never proof of absence; raw and normalized coexist; syntax occurrence is
not a semantic entity; canonical identity is application-owned; provider isolation; authority
tables rather than silent overwrite; atomic present state per query. They describe what a
CPG *means* and they cost almost nothing to honor.

The 36 fabric principles (P1 through P36) are a different kind of thing. They describe how a
data platform *should be built*, and the process requires every design decision to cite them
with `Advances`, `Maintains` or `Risk — mitigated`. Applied to internal seams, they produce
1,874 public types, "self-observed proof" of transformations that run twice to check
determinism (F13), and identity-preserving view tables that disable native projection
pushdown for the whole session (F08). They are optimizing seams that have no consumers.

### D5. The specification is a maximal target treated as a minimum

Because "complete" is defined as the entire v2.3 profile and there is no defined useful subset,
every plan is scoped to everything, and nothing ships. A tool that answers definition,
references, callers and callees, imports, and type-of for Python and Rust, refreshed on file
save with honest staleness markers, would be roughly a tenth of the specification and would
already be valuable to an agent. It would also generate the workload that D2 is missing.

### D6. The process was designed to constrain a fallible agent, and agents responded by producing process-shaped output

Agents do hallucinate completion, and the proving-commit rule, the executable-oracle rule and
the zero-state rule are all reasonable responses to that. But the defense has become the work.
An executing agent loads about 4,500 lines of policy and 26,000 words of plan, and is then
rewarded for producing oracles, packet checks, dispositions and status reports that mirror the
policy. The 48% deletion rate on `src/` and the 340 packet-named tests are the visible
residue. The user has already had to intervene once, in writing, against the process's own
ceremony (§1.6 step 4).

---

## 3. Cross-cutting recommendations

Each recommendation states what to do, why, how, what it replaces, and the main risk. They
are numbered for reference; §7 gives a suggested order.

### R1. Move the proof to the product boundary: a golden corpus

**What.** Build a fixture corpus of three to five small real Python and Rust repositories,
plus CodeFabric itself, each with a golden answer file for every supported question form.
Correctness is a diff between the tool's answer at the MCP boundary and the golden file.

**Why.** This is the one investment in proof that must be made, because it is what makes the
user's "we can always audit the results" true. Without it, auditing is guesswork. With it,
every intermediate oracle becomes optional.

**How.**
- Corpus candidates: this repository's `codefabric-cpg-mcp/` for Python; a small Rust crate
  with traits, generics and a `build.rs`; a Python package with relative imports, decorators,
  `__init__` re-exports, and a `TYPE_CHECKING` block; one deliberately malformed file per
  language.
- Golden format: one JSON Lines file per question per corpus, each line a query request and
  its expected response. Store under `tests/fixtures/golden/<corpus>/<form>.jsonl`. The
  `tests/fixtures/` trigger in `AGENTS.md` §2 ("a test needs reusable non-code data") is now
  met.
- Recipe: `just golden` runs the corpus through a real daemon, diffs, and prints the first
  mismatch. `just golden-accept` is confirm-gated and rewrites the expected files from
  current output; its diff is reviewed like any snapshot acceptance.
- Add one differential test: for a scripted edit sequence over a corpus, the incremental
  state after the last edit equals a clean rebuild from the final bytes. This is the only
  intermediate oracle the pipeline needs (D1).

**Replaces.** The four-oracle-per-packet rule, `just real-time-cpg-packet-check`, and the
120 proposed `rt_cpg_wp*` tests.

**Risk.** Golden files can be accepted carelessly. Mitigation: `golden-accept` stays
confirm-gated and the diff is inspected, exactly as `snapshots-accept` is today.

### R2. Invert the build order: facts first, fabric last

**What.** Wire the four providers to the daemon on the simplest substrate DataFusion accepts,
and defer Delta, exact epochs, CDF, activation transactions and resource admission until a
measured workload demands each one.

**Why.** D2. The fabric cannot be designed well without facts flowing through it, and every
fabric line written now is maintained through every re-plan.

**How.**
- Substrate: one set of Arrow record batches per file per relation, held in an
  `Arc<HashMap<FileId, Vec<RecordBatch>>>` behind a custom `TableProvider`, or simply a
  `MemTable` per relation rebuilt from the map. DataFusion 55 handles either. A catalog swap is
  an `Arc` replacement under a lock; a query pins the `Arc` it started with, which already
  satisfies "one immutable snapshot per query" without an epoch machine.
- Real time: `notify-debouncer-full` drives a syntax lane that re-parses one file with
  Tree-sitter (using the prior tree and the edit) and Ruff, and replaces that file's batches.
  Budget: under 100 ms per file. A semantic lane runs Pyrefly and the rustc extractor,
  debounced, and marks affected files `semantic_stale` until it completes. The review's
  "two freshness lanes" (§3.1) is the correct design and survives intact.
- Persistence: none in v0. Startup re-parses; on a 50k-line repository that is seconds. Add
  Delta as a cache when startup time on a real target repository is measured and found
  unacceptable, and add it as a plain Parquet or Delta snapshot per relation, not as an
  exact-history transaction system.
- Resources: DataFusion's `FairSpillPool` bounds query execution. A process-level RSS
  watchdog restarts the daemon over a threshold. Retained catalogs are capped by count.
  delta-rs and tokio internals are not accounted; record that as a limitation.

**Replaces.** The production route through `fabric/activation*.rs`,
`fabric/programmatic_epoch.rs`, `fabric/delta_*.rs`, `fabric/proof*`, `fabric/admission.rs`,
`fabric/child_session*`, `fabric/streamed_result_*`, `fabric/command*.rs`, and the SQLite
command and checkpoint records. See §4 for the disposition of that code.

**Risk.** Losing exactness guarantees the specification requires. Mitigation: they are
deferred, not rejected; and the golden corpus plus the differential test are stronger
evidence of correctness than the epoch proofs they replace.

### R3. Keep exactly one doctrine as non-negotiable: absence is never proof of absence

**What.** Every response carries a coverage envelope: which files are current, which are
syntax-current but semantically stale, which providers ran, which failed and why, and which
files were excluded. This is the user's requirement to "describe exactly what has still not
been fully processed", and it must be designed in from the first commit of v0.

**How.** A per-file status table, itself queryable, with roughly these columns:

| Column | Meaning |
|---|---|
| `file_id`, `path`, `content_digest` | identity of the bytes the facts were computed from |
| `syntax_state` | `current` / `stale` / `error` / `excluded` |
| `semantic_state` | `current` / `stale` / `pending` / `unavailable` / `error` |
| `semantic_provider` | `pyrefly` / `rustc` / `none` |
| `syntax_generation`, `semantic_generation` | monotone counters, so a response can say "as of generation N" |
| `detail` | parse remainder, provider error text, exclusion reason |

Every query response includes the generation it ran against and the count of files in each
state, and any file-scoped result row carries that file's state. A response over a partially
processed workspace is therefore honest by construction, and no epoch proof is needed to make
it so.

**Replaces.** Proved `FabricEpoch`, capability-gap relations as a separate proof product.

### R4. Define v0 and ship it

**What.** A small, useful subset with a hard boundary, shipped before anything else in the
specification is attempted. See §6 for the full definition. In one line: both languages;
definition, references, callers and callees, imports and dependencies, type-of, file status;
two MCP tools; real-time syntax lane; debounced semantic lane; no persistence.

**Why.** D5. The subset generates the workload that every deferred fabric decision needs, and
it is the point at which the tool becomes worth using on itself.

**Replaces.** "Complete realized outcome" as the only defined target.

### R5. Cut the process weight by an order of magnitude

**What.** Replace the ten workflow skills and the artifact machinery with two living
documents and git history.

**How.**
- `docs/designs/`: a design note of one to three pages per significant decision, recording
  the decision, the alternative rejected, and the reason. No frontmatter schema, no digest
  tables, no principle-conformance tables, no LD blocks. Superseded notes are moved to
  `docs/designs/archive/`.
- `STATUS.md` at the repository root: what works, what is next, what is broken, and the one
  command that proves the current claim. Edited in place. This is the cross-session handoff
  that the state JSON files were trying to be.
- Retire: immutable plans, versioned successors, `docs/plans/state/`, `active-plan.json`,
  plan activation transactions, declared-input digest tables, packet and milestone and
  decommission-batch IDs, proving commits, zero-state decommission proof, plan audits,
  audit-integration dispositions, and implementation-status reconciliation.
- Retire the corresponding skills: `impl-plan`, `plan-audit`, `integrate-plan-audit`,
  `impl-plan-exec`, `impl-status`, `implementation-review`, `skill-eval`. Keep
  `design-development` and `library-capability-research` only if cut to a page each; their
  useful content is "challenge the design before building" and "probe the library before
  assuming".
- Keep the eleven library-reference navigators. They are cheap pointers and they earn their
  keep.
- Collapse `_shared/` to one page. The whole evidence policy an agent needs is: *if you claim
  it, show the command that shows it; if it is a behavior, there is a test; if you did not run
  it, say so.* The whole validation policy is: *edit-local checks while editing, `just
  ci-fast` before claiming done, `just golden` before claiming a product behavior.*
- Reduce `AGENTS.md` to about 150 lines: what the repository is, the four build domains, the
  eight product doctrines, the command contract, the three traps, and the search hazards.
  Move the tooling inventory, the sccache topology, the assurance tiers and the dependency
  policy rationale into `docs/` where they can be consulted when relevant rather than loaded
  every session.
- Delete `tooling/ci/plan_assurance.py`, `artifact_contracts.py`,
  `real_time_cpg_assurance.py` and their tests, and the recipes that invoke them. Keep the
  feature-architecture, stable-graph, duplicate-family and proto-drift validators; they
  protect real invariants of the build.

**Why.** D3 and D6. Git history is a complete, derivable record of what changed and when; the
artifact machinery was reproducing it by hand.

**Risk.** Loss of cross-session continuity. Mitigation: `STATUS.md` plus `git log` plus the
golden corpus is a smaller and more reliable handoff than a 35 KB state file whose packets
are invalidated by the next plan.

### R6. Demote the 36 fabric principles from gate to reference

**What.** Keep `full_data_fabric_design_principles_v2.md` as reading material for whoever is
designing the persistence layer when R2 says it is time. Remove the requirement that
decisions cite principles, and remove conformance reviews and remediation proposals as
artifact types.

**Why.** D4. The eight product doctrines in `AGENTS.md` §0.2 are the ones that define what a
CPG means and they are cheap. The 36 fabric principles are a platform philosophy applied to
seams with no consumers.

### R7. Rename and prune the test suite around behaviors, not packets

**What.** Tests are named for the behavior they prove. `wp44_beh_real_supervisor_ready_requires_durable_fresh_activation`
becomes `supervisor_ready_requires_fresh_activation`, or is deleted if the behavior does not
survive R2. Delete the packet dispatchers. Keep `just ci-fast` and keep it green.

**Why.** D3, and the gate has been red for nine days at baseline, which means it has stopped
doing its one job. A red baseline that every plan promises a later packet will fix is the
process failing its own purpose.

**How.** Fix `src/provider_sandbox.rs:832` first; it is a one-line `NonZero<i32>` mismatch.
Then rename or delete in one commit per module as R2 touches it, not as a separate sweep.

### R8. Reverse the dependency fork

**What.** Remove `third_party/native/` and the `[patch.crates-io]` section, returning to the
registry pins and the delta-rs git revision recorded in `FAB §2.1`.

**Why.** §1.6. Fifteen patched crates including tokio, arrow and parquet is the highest-risk
decision in the repository: every upstream security fix, every DataFusion upgrade, and every
`cargo update` now requires a rebase of a private fork, and the reason for the fork (charging
delta-rs's internal allocations) disappears under R2 because Delta is off the v0 path.

**How.** `git rm -r third_party/native tooling/native-dependencies`, delete the patch
section, `cargo update -p` nothing (the lock already records the registry versions), run
`just stable-graph-check`. Tag the pre-removal commit so the work is recoverable if a
measured OOM ever justifies revisiting it.

**Risk.** Code in `src/fabric/native_*` and `owned_local_store*` that depends on the patched
APIs stops compiling. That code is on the R2 archive list anyway.

### R9. Replace the agent-hallucination defense with one command

**What.** No claim of a product behavior without `just golden` passing, and no claim of
"done" without `just ci-fast` passing. Both are stated in `STATUS.md` and in the reduced
`AGENTS.md`. That is the whole gate.

**Why.** D6. The proving-commit rule was protecting against agents claiming completion they
had not earned. A golden diff over real repositories is a stronger defense than a proving
commit, and it costs one command.

---

## 4. What to keep, what to archive

Fairness matters here: the current codebase contains real, reusable work, and the point of
this assessment is to stop maintaining the parts that are not on the path, not to discard
everything.

### Keep, as is

| Component | Reason |
|---|---|
| The four isolated build domains and their Cargo roots | Contains nightly and pinned-source instability; no Python data plane. Correct and cheap. |
| `contracts/rpc/*.proto`, generated Rust and Python, `tooling/proto/` | Works; drift-checked; the daemon-to-adapter boundary is settled. |
| `codefabric-cpg-mcp/` | The FastMCP 4 adapter works and is presentation-only. Its four tools are roughly the right shape for v0. |
| `src/supervisor.rs`, `src/daemon.rs`, `src/process_runtime*`, `src/bin/` | The process shell. Simplify the handshake and discovery paths, but keep. |
| `rules/` and `rule-tests/` | 31 ast-grep boundary rules, 618 lines. Cheap, tested, and they encode the domain isolation. |
| `src/ruff_adapter/` including `cfg.rs` and `semantic.rs` | Real Python syntax, scope and control-flow work. F02 says the CFG is not yet the one on the production route; it should be. |
| `src/tree_sitter_adapter.rs`, `src/provider_native_syntax.rs` | Incremental parsing with owned DTOs. This is the syntax lane. |
| `pyrefly-sidecar/` | A working Pyrefly `Query` integration. Fix F03 (pass the effective context) and wire it in. |
| `rustc-extractor/` | Typed MIR and stable-key extraction. Wire it in as the Rust semantic lane. |
| `src/identity.rs`, `src/python_context.rs`, `src/source_image*`, `src/git_state.rs` | Canonical identity, effective Python context discovery, descriptor-relative reads. All on the v0 path. |
| `src/semantic_query_contract.rs`, `src/relational_semantic_query.rs` | The typed request contract and its relational compilation. Cut to the v0 forms; keep the shape. |
| `just` recipes in `environment`, `static`, `extractor`, `sidecar`, `adapter`, `contracts`, `supply-chain` | The command contract for building and checking each domain. |
| `scripts/stable_graph_check.sh`, `tooling/ci/feature_architecture.py` | Protects the single Arrow universe and the local-versus-S3 boundary. |

### Archive: tag, then remove from master

Tag the baseline as `fabric-v7-archive` so nothing is lost, then remove from master the
modules that are not on the v0 path and would otherwise have to compile, format and pass
Clippy on every change. Candidates, by prefix under `src/fabric/`: `activation*`,
`programmatic_epoch`, `programmatic_activation_command_*`, `delta_*`,
`programmatic_delta_*`, `programmatic_relation_delta`, `proof*`, `admission`,
`child_session*`, `streamed_result_*`, `published_arrow_result`, `arrow_result_resource`,
`command*`, `native_*`, `owned_local_store*`, `request_owned_relation`, `effective_view`,
`derived_producer_closure*`, `programmatic_observation_delta`. Under `src/`:
`provider_admission.rs`, `rust_compilation_trust.rs`, `production_provider_recipe.rs`,
`production_query_recipe.rs`, `registry_contract_data.rs`, `operational_schema_specs.rs`,
the `semantic_release` and `release-compiler` features, and `cancellation/` if it is not
reused by the v0 query coordinator.

This is a large deletion and it is the user's call, not an agent's. The reason to recommend it
rather than a feature flag is the user's own observation: dormant code that must still compile
is exactly the maintenance choke being described, and a git tag preserves it perfectly.

Pieces of the archived fabric that are likely to come back when R2 says a workload justifies
them, and should be the first things read at that time: `fabric/delta_exact.rs` (exact
version reads), `fabric/delta_write.rs` (governed writes with application transaction
identity), `fabric/programmatic_schema.rs` (typed transformation registration), and
`fabric/query_coordinator.rs` (task ownership and joined cancellation, which the review
singles out at F14 as worth preserving).

### Reduce

| Component | To |
|---|---|
| `AGENTS.md` | ~150 lines (R5) |
| `.claude/skills/_shared/` | one page (R5) |
| `.claude/skills/` | eleven reference navigators plus at most two one-page workflow skills (R5) |
| `justfile` | remove the `test` group's packet dispatchers and the `gate` group's plan/artifact validators; expect roughly 226 → 120 recipes |
| `tooling/ci/` | remove plan, artifact, and packet assurance modules; expect roughly 12,000 → 3,000 lines |
| `docs/plans/`, `docs/reviews/`, `docs/designs/` | move everything dated before this assessment to `archive/` subdirectories. They remain history; they stop being inputs. |

---

## 5. The theoretical argument, stated carefully

The user's second premise is that the sequence of calculations can be argued robust on
theoretical grounds rather than proved stage by stage. This is right with one qualification,
and stating it precisely helps an agent know where a test is and is not needed.

The fact pipeline has this shape:

```text
bytes(file)  --parse-->  provider facts(file)  --normalize-->  facts(file)
facts(all files)  --join/derive-->  relations  --compile-->  answer(query)
```

Properties:

- **Per-file stages are pure functions of the file's bytes and the effective context.** Parse,
  provider extraction and normalization do not depend on other files. Therefore for any fixed
  snapshot, if the final relations are correct then each per-file stage was correct on that
  file. Golden answers over a corpus exercise all of these at once.
- **Cross-file derivation is a pure function of the set of per-file facts.** Joins, closure,
  SCCs and reachability depend on all files but on nothing else. Golden answers whose
  expected results span files exercise these.
- **Incremental invalidation is the one stage that is not a function of the snapshot.** It
  is a function of the *history* of edits. Its correctness condition is simple to state:
  `incremental(edits) == clean(final bytes)`. One differential test over scripted edit
  sequences proves it, and that test should be in `just golden`.
- **Provider availability is not a correctness question but a coverage question.** If Pyrefly
  did not run, the answer is not wrong; it is incomplete, and R3 makes that visible. A test
  that a missing provider produces `unavailable` rather than an empty result is the one
  negative test worth keeping from the current suite's philosophy.

So the theoretical argument holds for everything except invalidation, and invalidation has a
one-test oracle. That is the whole justification for R1 replacing 120 packet oracles.

---

## 6. A concrete v0 definition

This is a proposal for the user to edit, not a specification.

**Languages.** Python 3.14 via Tree-sitter, Ruff and Pyrefly. Rust via Tree-sitter and the
rustc extractor on the dated nightly.

**Questions.** Each is a `QueryForm` value that already exists in the contract:

| Question | Form | Minimum semantic source |
|---|---|---|
| Where is `X` defined? | `FindEntities` | syntax lane (Ruff bindings, Tree-sitter items) |
| Where is `X` referenced? | `FollowRelationships` | syntax lane, upgraded by the semantic lane when available |
| Who calls `X` / what does `X` call? | `FollowRelationships` | syntax lane call sites; semantic lane resolves targets |
| What does file/module `M` import, and who imports it? | `FollowRelationships` | syntax lane |
| What is the type of `X`? | `RetrieveFacts` | semantic lane only; `unavailable` otherwise |
| Show the source around `X` | `RetrieveSourceContext` | source image |
| What is the processing state of the workspace / file? | `RetrieveFacts` over the status table | R3 |

`FindPaths`, `MatchPattern`, `CombineResults`, `SummarizeFacts` are declared and rejected with
a typed `unsupported_in_v0` until a later version.

**MCP tools.** `query_code_graph` and `get_code_graph_status`, both already registered.
`validate_code_graph_query` and `get_code_graph_reference` can stay if they are already
cheap; they are not required.

**Freshness.** Syntax lane: a file's syntax facts are replaced within 100 ms of the debounced
change event. Semantic lane: Pyrefly on the affected module set, rustc extractor on the
affected crate, debounced to 500 ms of quiet, with `semantic_state = stale` set immediately
and cleared on completion. Both generations are reported in every response.

**Persistence.** None. Startup parses the workspace. If startup on the user's largest real
repository exceeds an acceptable bound, that measurement opens the Delta-as-cache decision.

**Proof.** `just golden` over the corpus in R1, including the incremental-equals-clean
differential test and one "provider absent yields unavailable, not empty" test per provider.
`just ci-fast` green.

**Non-goals for v0.** Exact Delta histories, CDF, epochs as proof objects, resource admission
beyond DataFusion's pool and an RSS watchdog, multi-workspace, S3, MIR-derived ownership
analyses, Python effect and async analyses, path and pattern query forms.

---

## 7. Suggested sequencing

Weeks are indicative. The point of the order is that each step produces something an agent
can use, and that nothing in a later step is needed to make an earlier step useful.

1. **Stop the bleeding.** Fix the red baseline (R7 first step). Apply R8 and tag. Write
   `STATUS.md`. Reduce `AGENTS.md` and `_shared/` (R5). Do not start any packet of plan v3.
2. **Corpus and harness.** Build the golden corpus and `just golden` (R1) with the status
   table (R3) as the first golden question: the tool must report its own coverage before it
   reports anything else.
3. **Python syntax lane on the memory substrate.** Tree-sitter plus Ruff, per-file batches,
   catalog swap, `FindEntities` and `FollowRelationships` for definitions, references,
   imports, and syntactic call sites (R2, R4). Golden answers for those. Real-time via
   `notify`. The differential test.
4. **Rust syntax lane.** Same shape with the Tree-sitter Rust grammar.
5. **Python semantic lane.** Pyrefly wired to the production route with the effective context
   (F03). Type-of and resolved call targets. `semantic_state` transitions visible.
6. **Rust semantic lane.** The rustc extractor on the production route. Resolved call
   targets and types from MIR.
7. **Archive the fabric** (§4) once steps 3 through 6 no longer reference it, in one commit
   per module group.
8. **Measure.** Startup time, per-edit latency, RSS, on the user's real repositories. Those
   numbers, and only those numbers, decide whether Delta, epochs, or resource admission come
   next.

---

## 8. Interactions with existing artifacts

An agent acting on any of this must know how it collides with what is currently in force.

- **Plan v3 is the active plan** (`docs/plans/active-plan.json`). Its WP77 and WP78 carry
  proving commits; WP79 is in progress; WP80 through WP106 are not started. This assessment
  recommends not executing it. Under the current process that requires the user to say so,
  because an agent following `impl-plan-exec` will otherwise resume WP79. The cleanest
  expression is to set the plan's `status` to `superseded` and point `active-plan.json` at
  nothing, or to remove the plan machinery entirely under R5. Either is the user's decision.
- **The vendored fork is in master** as of `0cc7242`. R8 removes it. Code under
  `src/fabric/native_*`, `src/fabric/owned_local_store*`, `src/fabric/child_session/resource_governance.rs`
  and the `bounded_encoding` module depends on the patched APIs and would need to go with it or
  be stubbed; all of it is on the §4 archive list.
- **`artifact-schemas.md` §7 and `tooling/ci/artifact_contracts.py`** were extended with a
  `process-assessment` row so that this document is valid under the rule that exists today.
  R5 retires that rule; when it does, the row goes with it.
- **The v2.3 suite remains the design target.** Nothing here proposes changing what a CPG
  means or what the eventual system should do. It proposes changing the order in which the
  target is realized and the mechanism by which realization is proved. When v0 is shipped and
  measured, the suite's remaining scope should be re-cut into a v1 subset by the same method.
- **`docs/spec_index/` and the library navigators** remain valid and useful. The
  `impl-*` and `plan-*` skills should be marked deprecated in their frontmatter before
  deletion so that a session that loads them by habit sees the notice.

---

## 9. Cautions, and where the premise needs qualification

- **"We can always audit the results" is only true once the golden corpus exists.** It is the
  first thing to build, not the last, and `golden-accept` must stay confirm-gated.
- **Honest staleness has to be designed in at the start.** If the status table (R3) is added
  after the query surface, responses will have been silently incomplete in the meantime.
  Make the status query the first golden question.
- **Archiving the fabric loses real engineering.** The exact Delta reads, the governed writes
  with application transaction identity, and the joined-cancellation coordinator are good
  work. The tag preserves them; §4 names the files to read first when a workload brings them
  back.
- **Agents do hallucinate completion.** The current process is a rational response to that.
  R9 is a smaller response, not the absence of one. If an agent reports a product behavior
  without a passing `just golden`, that is the same defect the old process was guarding
  against, and `STATUS.md` should say so in its first paragraph.
- **The rustc extractor is slow by nature** because it compiles the target crate on a nightly.
  The two-lane design handles that; a v0 that tries to make Rust semantics real-time will fail
  and should not be attempted. Syntax-lane Rust facts with `semantic_state = pending` is the
  honest v0 answer.

---

## Appendix A. Reproduction commands

All commands run from the repository root at the baseline commit.

```bash
# Timeline and churn
git log --reverse --pretty='%ad %s' --date=short | head -1
git log --date=format:%Y-%W --pretty=%ad | sort | uniq -c
git log --numstat --pretty=format: | awk 'NF==3 && $1!="-" && $3 ~ /^src\//{a+=$1; d+=$2} END{print a, d}'

# Code composition
find src -name '*.rs' | xargs cat | wc -l
awk 'FNR==1{t=0} /^#\[cfg\(test\)\]/{t=1} {if(t) tl++; else pl++} END{print pl, tl}' $(find src -name '*.rs')
find src/fabric -name '*.rs' | xargs cat | wc -l
find third_party/native -name '*.rs' | xargs cat | wc -l
grep -rc '#\[test\]' src --include='*.rs' | awk -F: '{s+=$2} END{print s}'
grep -rhoE 'fn (wp|rt_cpg_wp)[0-9]+[a-z_]*' src tests | wc -l
wc -l tooling/ci/*.py | tail -1

# Product surface
sed -n '/^pub enum QueryForm/,/^}/p' src/semantic_query_contract.rs
grep -nE 'fn compiled_[a-z_]+_program' src/production_query_recipe.rs
grep -nE 'name="[a-z_]+"' codefabric-cpg-mcp/src/codefabric_cpg_mcp/server.py

# Process load stack
wc -l AGENTS.md CLAUDE.md docs/library_ref/full_data_fabric_design_principles_v2.md \
      .claude/skills/_shared/*.md .claude/skills/impl-plan-exec/SKILL.md
wc -lw docs/plans/codefabric_real_time_cpg_implementation_plan_v3_2026-09-07.md

# Plan lifecycle
ls docs/plans/*.md | wc -l
for f in docs/plans/state/*.json; do echo "$f"; jq -r '[.. | objects | select(has("status")) | .status] | group_by(.) | map({(.[0]): length}) | add' "$f"; done

# Dependency fork
sed -n '/^\[patch.crates-io\]/,$p' Cargo.toml
du -sh third_party/native

# Gate health
cat target/agent/baseline.json
```

## Appendix B. Findings cross-reference

| This document | Review finding | Plan v3 item |
|---|---|---|
| D2, R2 | F01, F04, F09, F10, F11, F13 | WP79, WP88 through WP95 |
| D1, R1 | F12 | every packet's four oracles; WP101, WP102, WP106 |
| R3 | F05, F07 | WP89, WP100 |
| R4, §6 | F06, F07 | WP98, WP100 |
| §1.6, R8 | (status report v2 "evidence for reopening") | WP79, LD-RT09, DB30 |
| D4, R6 | F08, F13 | WP96, WP97 |
| R7 | (baseline red) | WP105 |
