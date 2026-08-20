# Migration Guide

## Mapping from the previous six-skill suite

| Previous skill | Revised disposition |
|---|---|
| `impl-plan` | Split into `design-development` for target architecture and revised `impl-plan` for execution packets. |
| `impl-plan-exec` | Replaced by adaptive packet execution with durable state and layered validation. |
| `impl-plan-review` | Renamed/refocused as `impl-status`; it no longer edits the plan. |
| `plan-audit` | Expanded to audit the source design, clean-sheet quality, library decisions, packet closure, proof, and legacy bias. |
| `integrate-plan-audit` | Retained with copy-then-edit provenance, but can now revise design and plan separately. |
| `lib-leverage` | Retained as a deep single-library review; complemented by problem-first `library-capability-research`. |

New capabilities:

- `implementation-review`
- `skill-eval`

## Recommended rollout

### 1. Install shared references and non-writing skills

Install:

- `_shared/`
- `design-development`
- `library-capability-research`
- `plan-audit`
- `implementation-review`
- `lib-leverage`
- `skill-eval`

Use them on one representative architectural change and compare their artifacts
with the previous workflow.

### 2. Adopt the new plan format

Install revised `impl-plan`. New plans use:

- design dossiers;
- `WP*` work packets;
- `M*` milestones;
- `DB*` decommission batches;
- external execution state.

Existing `S*`/`D*` plans can remain immutable. The new executor can ingest them
as legacy format, but their missing packet/gate metadata reduces confidence.
For high-risk unfinished plans, generate a new plan version from the accepted
design rather than translating mechanically.

### 3. Replace execution and status skills

Install:

- revised `impl-plan-exec`
- `impl-status`

Archive or rename the old `impl-plan-review` to prevent accidental invocation.
Do not retain both commands under similar names.

### 4. Add deterministic enforcement

After the skill workflow is stable, add repository-specific scripts/hooks for:

- artifact schema and stable-ID validation;
- dependency-cycle checks;
- state/plan digest checks;
- changed-file format/syntax feedback;
- architecture and legacy rules;
- stop-time completion evidence.

Keep repository commands outside generic skills; derive them from manifests and
CI.

### 5. Evaluate before full replacement

Use `skill-eval` across:

- one bounded change;
- one cross-module contract change;
- one hard architectural replacement;
- one mixed Python/Rust change;
- one partial-plan resume.

Adopt the suite when it reduces defect escape and implementation rework without
unacceptable planning overhead.

## Frontmatter decisions

Side-effecting skills are manual-only:

- `impl-plan-exec`
- `integrate-plan-audit`
- `skill-eval`

Read/research-heavy skills use forked contexts where helpful. A fork keeps
investigation transcripts out of the main context; independent reviewers should
still use fresh named subagents when author bias matters.

`allowed-tools` pre-approves tools during invocation; it is not a complete
security boundary. Use project permissions, subagent definitions, and hooks for
hard restrictions.

## Suggested companion subagents

The skills work without custom subagent files, but the following focused
profiles improve consistency:

- `repo-architect` — read-only architecture and impact evidence.
- `library-researcher` — official docs, versions, and probes.
- `design-challenger` — fresh-context clean-sheet critique.
- `packet-implementer` — dependency-closed implementation plus local proof.
- `implementation-reviewer` — read-only outcome/correctness review.

Put generic behavioral rules in these definitions. Keep packet prompts limited
to current facts, boundaries, acceptance evidence, and local gates.

## Artifact retention

Treat these as append-only provenance:

- accepted/superseded design versions;
- plan versions;
- plan audits;
- implementation-status reports;
- implementation reviews;
- skill evaluations.

Execution state is mutable but should preserve prior evidence, deviations,
failed approaches, and corrections.

## First plan conversion checklist

- [ ] Target design exists and is accepted.
- [ ] Every material current surface has a legacy disposition.
- [ ] Load-bearing library APIs are pinned and verified.
- [ ] Work packets are dependency-closed.
- [ ] Change surfaces are tiered by confidence.
- [ ] Every packet has local proof and replan triggers.
- [ ] Integration milestones are placed where risk first combines.
- [ ] Final gates cover every affected toolchain.
- [ ] Plan and state paths are separate.
- [ ] Fresh-context implementation review is scheduled.
