---
name: integrate-plan-audit
description: Integrate a plan audit into versioned design and implementation-plan artifacts through targeted, traceable edits, preserving stable identifiers and recording a disposition for every finding.
when_to_use: Use after plan-audit and before execution. Use a new artifact version by default. Do not use when the audit verdict is needs-redesign unless creating a new design and plan revision is explicitly within scope.
argument-hint: "[plan-path] [audit-path] [--design path] [--mode new-version|in-place]"
allowed-tools: Read, Glob, Grep, Bash, Write, Edit, WebSearch, WebFetch
user-invocable: true
---

# Plan Audit Integration

Integrate evidence-backed findings without regenerating unaffected prose or
silently changing the accepted mental model.

Read:

- `../_shared/evidence-policy.md`
- `../_shared/code-intelligence.md`
- `../_shared/doctrine-policy.md`
- the audit and integration sections of `../_shared/artifact-schemas.md`

## Inputs and outputs

Interpret `$ARGUMENTS` as:

1. plan path;
2. audit path;
3. optional design path and mode.

Infer the design path from plan frontmatter when omitted.

Default mode is `new-version`.

Outputs may include:

```text
docs/designs/<topic>_design_vN+1_<date>.md
docs/plans/<topic>_implementation_plan_vN+1_<date>.md
```

Only create a new design version when findings change design decisions,
invariants, library decisions, target architecture, or transition architecture.
Otherwise revise only the plan.

Never overwrite prior versions. Never edit execution state as part of audit
integration.

## Non-negotiable rules

1. **Copy, then edit.** In new-version mode, first make an exact copy.
   Confirm the diff is empty with `git diff --no-index <old> <new>`. Every
   later change is a targeted edit.
2. **Stable identifiers are provenance.** Existing `D-*`, `I-*`, `LD-*`,
   `L-*`, `WP*`, `M*`, and `DB*` IDs never change. New items receive the next
   unused ID.
3. **Every finding is dispositioned.** Use exactly one disposition from the
   shared schema.
4. **Validate before trusting the audit.** Re-verify load-bearing current-tree
   and library claims before converting them into specification text.
5. **Edit the root defect, not the audit's wording.** The audit's proposed fix
   is evidence-informed guidance, not an automatic patch.
6. **No hidden re-planning.** If integration requires materially rewriting the
   target architecture, produce a new design version and corresponding new plan
   or stop with `requires-redesign`.
7. **No incidental churn.** Unflagged sections remain byte-for-byte unchanged
   unless a cross-reference or consistency update is required.

## Procedure

### Phase 1 — Parse and classify

Read the design, plan, and audit end-to-end.

Extract every `F-*` finding, severity, category, scope, required resolution, and
revalidation condition.

Classify the likely integration surface:

- plan-only factual or packet correction;
- new/expanded packet or decommission batch;
- design decision/invariant/library/legacy change;
- evidence now stale or contradicted;
- finding too broad for responsible integration.

If the audit verdict is `needs-redesign`, do not disguise redesign as a plan
patch.

### Phase 2 — Create version artifacts

In `new-version` mode:

1. determine the next free version;
2. copy the old artifact byte-for-byte;
3. update only version/date/status/frontmatter lines;
4. confirm the remainder is unchanged before substantive edits.

If design changes, create the design version first, then update the new plan
to reference its exact path and version. Update the plan's declared-inputs
table only when the revision re-baselines it — freshness is derived per
`artifact-schemas.md` §8, not hand-maintained.

Use `in-place` only when the user explicitly requests it or the original is
uncommitted scratch. Record that provenance is weaker.

### Phase 3 — Re-verify load-bearing findings

Re-verify before applying findings about:

- nonexistent/moved files or symbols;
- callers, implementations, construction/serialization sites;
- legacy match sets or zero-state claims;
- library version, API, feature flag, or behavior;
- baseline and repository drift;
- test/build commands;
- doctrine or anti-principle structure.

When current evidence differs:

- **reframe** the finding to the actual defect;
- **reject** it with evidence;
- **defer** it when closure requires new investigation;
- **requires-redesign** when the target itself is invalidated.

Do not re-run broad discovery for findings whose evidence remains current and
load-bearing facts are unchanged.

### Phase 4 — Decide disposition and edit granularity

Allowed dispositions:

- `applied-plan`
- `applied-design`
- `added-packet`
- `added-decommission`
- `covered-by:<finding>`
- `deferred`
- `rejected`
- `requires-redesign`

Defaults are guidance, not automation:

- factual blockers and majors normally apply;
- missing consumers/proof normally expand a packet;
- a distinct workstream normally becomes a new packet;
- optional observations normally defer;
- doctrine regressions and anti-principles must be resolved or marked
  `requires-redesign`.

Use the smallest edit that fixes the root defect. Rewrite a subsection only
when several related surface edits would obscure the model.

### Phase 5 — Apply design edits first

When needed, update:

- decision and invariant sections;
- library decisions and version evidence;
- target contracts/flows;
- legacy disposition matrix;
- transition/cutover/decommission;
- test oracle;
- assumptions, risks, and replan triggers;
- recorded evidence (`evidence-policy.md` §6) and acceptance status.

A changed target decision must propagate to every affected plan packet. Do not
leave the plan referencing superseded design IDs or versions.

### Phase 6 — Apply plan edits

Update as needed:

- frontmatter/design references;
- preflight queries and known-touch entries;
- packet outcome, dependencies, and invariants;
- required changes and legacy disposition;
- named acceptance checks and local gates;
- milestones, decommission batches, and final gate matrix;
- execution sequence;
- plan risks and replan policy.

New packets take fresh `WP` IDs and are placed by dependency in the execution
sequence. New decommission batches take fresh `DB` IDs.

Do not add full code bodies merely because an audit requested more detail.
Prefer a design-bearing contract or stronger proof obligation.

### Phase 7 — Write the integration log

Insert or replace one `## Audit Integration Log` in the revised plan before the
first work packet.

At the top record:

- audit path/version;
- source design and plan versions;
- revised design and plan versions;
- one-sentence revision reason.

For every finding record:

```markdown
- `F-001` — `applied-plan`
  - Finding: ...
  - Resolution: exact section/ID edited.
  - Revalidation: the finding's command, run verbatim, with its exit status.
  - Rationale: why this resolves the root defect.
```

For `covered-by`, `deferred`, `rejected`, or `requires-redesign`, record the
specific reason and next closure condition.

If the design was revised, include a parallel `## Audit Integration Log` or
revision note in the design dossier that identifies affected decisions.

### Phase 8 — Consistency and revalidation

Three passes, cheapest first:

1. **Validate** every revised artifact per `artifact-schemas.md` §8 —
   frontmatter, IDs, cross-references, digests. Do not re-check by hand what
   it covers.
2. **Confirm byte-discipline**: `git diff --no-index <old-version> <new>`
   shows only the intended sections changed.
3. **Judge** what validation cannot see: dependency order and milestone
   membership; every legacy disposition maps to execution; every finding has
   exactly one disposition; doctrine notes exist for new material decisions.

Run the audit's revalidation commands verbatim for all blockers and majors.
Do not claim closure solely because prose was edited.

### Phase 9 — Report

Report:

- output path(s);
- counts by disposition;
- the §8 validation outcome on the revised artifacts;
- whether any blocker/major remains deferred or requires redesign;
- diff size and changed sections;
- whether the revised plan is ready for focused re-audit.

## Stop conditions

Stop and report rather than force integration when:

- the audit exposes a materially different target architecture;
- a load-bearing library capability remains unverified;
- several findings collectively invalidate the design stance;
- the current repository has drifted beyond responsible targeted edits;
- the source artifacts are malformed enough that copy-and-edit cannot preserve
  provenance.

The correct outcome in these cases is a new design-development pass, not a
quietly rewritten plan.
