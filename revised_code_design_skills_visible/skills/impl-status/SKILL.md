---
name: impl-status
description: Reconstruct and reconcile implementation progress for a versioned plan using the codebase, execution state, proving commits, and checks. Produces a separate status report and safe next action without editing the plan.
when_to_use: Use before resuming a long-running plan, after interrupted or parallel execution, after repository drift, or when execution state may no longer reflect current truth.
argument-hint: "[plan-path] [--state path]"
allowed-tools: Read, Glob, Grep, Bash, Write, Agent
disallowed-tools: Edit
user-invocable: true
context: fork
background: false
agent: general-purpose
---

# Implementation Status and Resume Audit

Determine what is truly complete, stale, blocked, or ready. This is not a code
quality review and does not modify the immutable design or plan.

The repository is read-only except for:

- a versioned implementation-status report;
- reconciliation of the plan's execution-state JSON.

Read:

- `../_shared/evidence-policy.md`
- `../_shared/code-intelligence.md`
- `../_shared/validation-policy.md`
- the execution-state section of `../_shared/artifact-schemas.md`

## Outputs

Write:

```text
docs/reviews/implementation_status_<plan-slug>_<YYYY-MM-DD>_vN.md
```

Update the state file only after evidence gathering. Never embed status into the
plan or check boxes in the plan.

## Status model

Packet statuses:

```text
not_started | ready | in_progress | blocked | complete | stale | invalidated
```

Milestones and decommission batches use the same model where applicable.

A prior `complete` status is not permanent. It remains trusted only when:

- the proving commit is in current history;
- the relevant code has not changed since proof;
- governing design/plan digests have not invalidated it;
- its dependencies remain valid;
- the recorded checks remain applicable;
- later packets did not reintroduce the legacy path or violate the invariant.

## Procedure

### Phase 1 — Ingest artifacts and state

Read the design, plan, and existing state. Extract:

- artifact versions/digests and baseline;
- packet dependencies and outcomes;
- milestones and decommission batches;
- acceptance evidence and checks;
- prior deviations, failed approaches, blockers, and discovered obligations;
- current and proving commits.

If state is absent, initialize an in-memory model from the plan; do not write it
until evidence has been gathered.

### Phase 2 — Detect specification and repository drift

Compare:

- stored plan/design digests to current files;
- baseline and proving commits to current history;
- current HEAD and working tree;
- relevant files/symbols/contracts since packet proof;
- library/toolchain versions and feature flags;
- tests/gates whose command or scope changed.

Mark the affected state candidates `stale` until re-evaluated. Do not invalidate
unrelated packets merely because HEAD advanced.

### Phase 3 — Verify packet evidence

Use the cheapest decisive evidence first.

For each packet:

1. Verify target contracts/behavior exist.
2. Verify immediate consumers and integrations use the target.
3. Verify attached legacy/decommission targets remain absent.
4. Verify recorded checks actually correspond to current code and current
   commands.
5. Re-run focused proof only when:
   - code changed;
   - prior evidence is missing/ambiguous;
   - a dependency changed;
   - a later change could reintroduce the defect;
   - the packet was never independently proved.

Do not require every predicted file to change. Judge the packet's outcome and
acceptance evidence.

### Phase 4 — Verify milestones and cross-packet effects

Check whether milestone prerequisites remain complete and whether later work
invalidated cross-packet behavior.

Pay particular attention to:

- contracts changed after consumers were proved;
- cross-language bindings or generated code;
- migrations and persisted data;
- concurrency/resource ownership;
- final removal of compatibility or dual-authority paths;
- governance rules that may have been weakened or bypassed.

Re-open the owning packet(s) when a milestone reveals incomplete work.

### Phase 5 — Reconcile legacy state

For each design legacy disposition and `DB*` batch:

- identify current target authority;
- verify old symbols, routes, imports, exports, registrations, config, data
  paths, aliases, tests, and documentation as required;
- inspect coverage for zero-state claims;
- check whether later edits reintroduced the old path.

A decommission batch is complete only when prerequisites and negative proof
both hold.

### Phase 6 — Determine status and next action

Assign each packet:

- `complete` — all current acceptance evidence holds;
- `stale` — previously complete but proof is no longer trustworthy;
- `invalidated` — target design/contract/library decision no longer holds;
- `blocked` — an external or upstream condition prevents completion;
- `in_progress` — coherent work exists but one or more obligations remain;
- `ready` — dependencies complete and no blocker;
- `not_started` — no plan-specific implementation evidence.

Distinguish `in_progress` from `not_started` using functional evidence, not the
mere existence of a pre-existing file.

Select the next action by dependency and risk:

1. repair invalidated/stale foundational packets;
2. finish in-progress dependency-closed packets;
3. clear blockers with the largest downstream fan-out;
4. execute ready packets;
5. run due milestones/decommission;
6. final gates and implementation review.

### Phase 7 — Reconcile the state file

Update:

- current HEAD and overall status;
- packet/milestone/batch statuses;
- current evidence and checks;
- stale/invalidated reasons;
- discovered obligations;
- blockers and failed approaches;
- exact `next_action`;
- timestamp.

Preserve prior evidence and failed approaches. Append corrections rather than
deleting provenance unless an entry is demonstrably corrupt; then record the
correction.

### Phase 8 — Write the report

Use:

```markdown
# Implementation Status: <plan>

## Provenance and Current Repository State
## Executive Summary
## Specification and Repository Drift
## Packet Status Matrix
## Milestone Status
## Decommission Status
## Baseline and Gate Status
## Discovered Obligations and Deviations
## Blockers and Invalidated Assumptions
## Recommended Resume Order
## Exact Next Action
## State Reconciliation Summary
```

For each non-complete packet, state:

- what is proved;
- what remains;
- exact evidence;
- whether the original packet instructions remain valid;
- focused completion/revalidation steps.

## Quality requirements

- Do not edit the plan or design.
- Do not trust `complete` solely because an earlier status said so.
- Do not rerun the entire repository suite when a focused proof settles status.
- Do not carry forward stale file paths or APIs.
- Do not mark a packet incomplete merely because an illustrative file was not
  modified.
- Do not write gap-closure instructions that silently alter the target design.
- Preserve enough state for a fresh executor to resume immediately.
