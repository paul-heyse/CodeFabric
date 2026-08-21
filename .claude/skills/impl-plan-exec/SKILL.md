---
name: impl-plan-exec
description: Execute a versioned implementation plan through dependency-closed work packets with durable state, current-tree impact checks, proportional validation, adaptive replan handling, decommission proof, and final certification.
when_to_use: Use only for an audited or approved plan. Supports long-running and multi-session execution; it does not require all work to fit one context window.
argument-hint: "[plan-path] [--state path] [--packet WPnn]"
allowed-tools: Read, Glob, Grep, Bash, Write, Edit, Agent
disable-model-invocation: true
user-invocable: true
---

# Adaptive Implementation Plan Executor

Execute the accepted design rather than mechanically reproducing a predicted
patch. Keep the immutable plan as specification and the state file as the
authoritative progress record.

Read:

- `../_shared/evidence-policy.md`
- `../_shared/code-intelligence.md`
- `../_shared/validation-policy.md`
- `../_shared/subagent-orchestration.md`
- the plan and execution-state sections of
  `../_shared/artifact-schemas.md`

## Critical operating rules

1. **Plan and design are immutable during execution.** Record progress,
   current-tree corrections, deviations, failed approaches, and blockers in
   state.
2. **Current verified truth outranks illustrative plan detail.** Target
   outcomes, invariants, and accepted design decisions remain authoritative.
3. **Validation is continuous and proportional.** Run edit-local and
   packet-local checks; do not defer all feedback to the end.
4. **A failing check is feedback, not termination.** Diagnose, correct, rerun,
   and continue.
5. **Do not use a tool to certify its own modification.** When editing analysis
   or search tooling — including CodeFabric's own analyzers and query surfaces,
   once they exist — verify with independent source reads, compiler output, and
   tests rather than the tool under change.
6. **No silent redesign.** Adapt implementation details within accepted
   invariants. Record and escalate plan or design invalidation.
7. **No partial cutover disguised as completion.** Immediate consumers,
   attached decommission, and packet proof land together.
8. **Continue independent work around a blocker.** A blocked packet need not
   halt unrelated ready packets.
9. **Persist before context loss.** State must be sufficient for a fresh
   executor to resume without reconstructing the entire session.

## Inputs

Interpret `$ARGUMENTS` as a plan path, optional state path, and optional packet
selection.

Infer the state path from plan frontmatter. The default is the full remaining
plan; `--packet` limits execution to one packet and its required dependencies
only when the user explicitly requests it.

Legacy plans with `S*` scope items may be ingested by grouping them into
dependency-closed execution tasks in state. Do not modify the old plan. Record
that the plan lacks native packet/gate metadata and apply the shared validation
policy conservatively.

## Phase 1 — Preflight and state initialization

### 1. Validate artifacts

Read the design and plan once. Confirm:

- accepted/audited status;
- design path/version;
- baseline commit;
- stable IDs and dependencies;
- state path;
- no unresolved blocker finding in the latest integrated audit.

Validate the artifacts and derive input freshness and packet trust per
`artifact-schemas.md` §8. A stale declared input or an unproven `complete`
packet marks the affected packets `stale` until reconciled. Do not silently
keep old completion state against a changed specification.

### 2. Establish repository state

Derive working-tree status and plan-baseline drift over the declared scope
per `artifact-schemas.md` §8; confirm relevant toolchain versions and
current gate recipes by inspection. Record in state only what is judgment:
baseline failures for required checks, and any unrelated pre-existing
modification that overlaps a packet — as that packet's blocker or deviation.

Do not overwrite or revert unrelated user work. If it overlaps a packet, mark
the overlap and integrate carefully or block that packet.

### 3. Initialize or reconcile state

Create/reconcile the state file using the shared schema (version 2). A
schema-version-1 file is migrated to version 2 on first write
(`artifact-schemas.md` §3).

Set packet readiness from dependencies. A prior `complete` packet remains
trusted only when:

- the §8 derivation shows it proven — its proving commit is in current
  history;
- relevant files/contracts have not changed since proof;
- the plan's declared inputs are fresh;
- its named acceptance checks still apply.

Otherwise mark it `stale` and revalidate or re-open it.

Set `next_action` before implementation begins.

### 4. Staleness and impact probe

For each soon-ready packet, verify load-bearing current facts:

- the symbols/files the packet names still exist;
- planned library API/version remains valid;
- contract consumers and implementations have not materially drifted;
- legacy targets and match sets still exist as expected;
- packet remains dependency-closed.

Put corrections into the packet's authoritative state context.

## Phase 2 — Decompose execution without re-planning

Create an execution task per work packet, or combine only when the plan itself
shows the packets cannot be left coherent separately.

Use dependency relationships from the plan. Attach each `DB*` decommission
batch to the earliest task where its prerequisites become true. Do not postpone
safe deletion solely for convenience.

A task description should contain:

- outcome and target invariants;
- dependencies already proved;
- design/library references;
- current verified facts and staleness corrections;
- preflight discovery queries and any session-verified known-touch files;
- named acceptance checks;
- local gates;
- decommission and negative proof;
- replan triggers.

Load only the active packet and referenced design sections during execution.
Do not repeatedly reload the entire plan.

## Phase 3 — Execute the packet loop

For each ready packet in dependency/risk order:

### Step 1 — Mark and checkpoint

Set packet `in_progress`, `current_packet`, current HEAD, start time, and
`next_action` in state before edits.

### Step 2 — Run packet preflight

Execute the packet's current-tree discovery obligations. Before load-bearing
interface changes, re-run the structural tripwires from
`../_shared/code-intelligence.md`.

Update the expected change surface in state. Do not edit the plan.

### Step 3 — Evaluate replan triggers

Classify any discrepancy:

#### Implementation adaptation

The mechanism/file/helper differs but outcomes, contracts, library decision,
invariants, migration, and proof remain unchanged.

- adapt;
- record a packet deviation;
- continue.

#### Plan revision required

Packet boundaries, dependencies, sequence, impact surface, decommission timing,
or proof obligations materially change, but target design remains valid.

- record the exact plan deviation and discovered obligation;
- execute only when the revised packet can still be left coherent and the
  change is bounded;
- otherwise mark affected packets `blocked` and continue independent work;
- recommend a new plan version after the session.

#### Design reopening required

A target invariant, public contract, architecture, library decision, or
transition design is invalidated.

- do not silently implement a different design;
- mark dependent packets `invalidated`;
- record evidence and a design-reopening request;
- continue only unrelated packets.

### Step 4 — Implement coherently

Implement the packet end-to-end:

- target contract and behavior;
- immediate consumers and adapters;
- cross-language or generated surfaces;
- tests and governance;
- observability/failure/resource behavior;
- attached legacy deletion and cutover.

Do not stop at scaffolding, TODOs, placeholder errors, compatibility aliases,
or one side of a dual-path migration unless the accepted design explicitly
requires a bounded intermediate state.

Keep refactors proportional. Refactor outside the packet only when necessary to
satisfy a target invariant or direct compile/test break, and record it.

### Step 5 — Run edit-local validation

After coherent micro-units, run the cheapest relevant checks:

- syntax/parse/import/compile smoke;
- changed-file formatter/lint;
- narrow type diagnostics;
- directly affected unit or contract test;
- minimal pinned-library probe.

Fix introduced failures before they spread. Avoid repository-wide gates while
the tree is intentionally incomplete.

### Step 6 — Run packet-local proof

Before completion, run every packet-local obligation:

- behavioral tests;
- structural/contract checks;
- immediate-consumer build/type checks;
- negative legacy checks;
- migration/boundary tests;
- operational or failure proof;
- package/crate/module checks.

A packet is not complete because files exist. Every named acceptance check
must be run and green.

If a check fails, diagnose and iterate. Add a failed approach to state when the
failure teaches a future executor not to repeat a path.

### Step 7 — Verify decommission

For attached legacy work, prove both:

- target route/authority is present and used;
- old symbol/import/export/registration/configuration/data path/alias/match set
  has reached the required zero state with complete evidence.

If zero-state evidence is partial, do not mark decommission complete.

### Step 8 — Complete and persist

**A proving commit is mandatory.** Commit the packet's coherent work — the
commit message carries the implementation summary — and record the commit as
`proving_commit` before setting `complete`. Without one the packet stays
`in_progress` with a blocker naming the gap. There is no working-tree-digest
fallback.

Update state (schema version 2 — judgment fields only) with:

- deviations worth a future executor's attention;
- failed approaches that teach what not to repeat;
- remaining risks and blockers.

Changed files, check outcomes, acceptance evidence, and decommission proof
are never stored: they are re-derived from the proving commit and by
re-running the packet's named checks (`artifact-schemas.md` §8).

Set packet `complete`, release dependent packets to `ready`, and update
`next_action`.

## Subagent execution

Delegate only packets satisfying the shared criteria.

### Parallel writers

Use isolated worktrees for simultaneous writers. Do not run two agents against
overlapping files or the same contract boundary. The lead owns merge order and
conflict resolution.

### Handoff

Use the compact handoff from `../_shared/subagent-orchestration.md`. Generic
constraints belong in the subagent definition.

### Validation ownership

Subagents must run edit-local and packet-local checks. Their return must
include the checks run — name plus exit status for each. Do not instruct
them to "implement only" or disregard their tests.

After merge, the lead:

- inspects the diff;
- reconciles state;
- reruns integration-sensitive packet checks;
- runs the next milestone when ready.

A subagent's completion claim is evidence to inspect, not automatic state.

## Phase 4 — Integration milestones

When all prerequisite packets for `M*` are complete:

1. mark the milestone in progress;
2. run its cross-packet behavioral, integration, differential, recovery,
   migration, performance, or end-to-end gates;
3. fix defects in the owning packet(s);
4. re-open packet state when a milestone reveals incomplete work;
5. record the final milestone evidence.

Do not defer a high-value milestone until all unrelated packets are complete.

## Phase 5 — Completeness audit

After all non-invalidated packets and milestones, run the §8 validation and
derivation first — artifact conformance, packet trust, input freshness —
then judge only what derivation cannot see:

- every outcome and invariant is satisfied per the accepted design, not
  merely the packet text;
- every design `L-*` disposition is satisfied;
- no unplanned duplicate authority, compatibility shims, TODOs, placeholder
  errors, or old patterns remain (structural search with a stated coverage
  envelope);
- all discovered obligations are closed or explicitly blocked.

Fix gaps before final gates when possible.

## Phase 6 — Final gate matrix

Run the plan's final gates in dependency-aware order. A typical order is:

1. formatting and syntax;
2. static analysis/type checks;
3. language/package builds;
4. focused unit/contract tests;
5. integration/cross-language/end-to-end tests;
6. governance, architecture, and decommission rules;
7. required migration, security, or performance gates.

Use repository-defined commands. Do not invent a universal command sequence.

For each failure:

- compare to baseline;
- assign ownership;
- fix and rerun the smallest affected gate;
- rerun broader gates whose validity the fix could affect.

Do not suppress errors, loosen tests, or narrow scope merely to report green.

## Phase 7 — Completion or durable handoff

Set overall state:

- `completed` only when all required packets, milestones, batches, and final
  gates are proved;
- `blocked` when unresolved blockers/design invalidation remain;
- `executing` when deliberately ending after a bounded packet selection.

Write a completion summary containing:

- design/plan/state versions and final HEAD;
- the derived change summary per `artifact-schemas.md` §8 (proving commits
  and their diffs — not a hand-built file list);
- packets/milestones/batches complete;
- final gate outcomes (recipe name + exit status) and baseline residuals;
- plan deviations;
- legacy removed and any explicitly deferred residue;
- open risks and next action.

Before compaction, interruption, or session end, persist current packet,
commands, failures, discoveries, and next action. Never rely on conversational
memory as the only resume mechanism.

## Prohibited completion shortcuts

- Marking a packet complete without its local proof.
- Treating a pre-existing file as evidence that planned behavior landed.
- Accepting a structural query with an unstated or filtered coverage envelope
  as global decommission proof.
- Leaving old and new authorities active without an accepted bounded
  transition.
- Rewriting the immutable plan to match implementation.
- Claiming final success while required checks were skipped or only subagent
  summaries were reviewed.
