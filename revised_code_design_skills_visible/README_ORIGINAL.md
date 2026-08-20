# Revised Code Design and Implementation Skill Suite

This package replaces a plan-centric, end-of-run validation workflow with a
design-and-proof workflow suitable for current long-running coding agents.

## Included skills

| Skill | Purpose |
|---|---|
| `design-development` | Develop and challenge the target architecture before planning implementation. |
| `library-capability-research` | Research problem-first, version-grounded off-the-shelf capabilities. |
| `impl-plan` | Convert an accepted design into dependency-closed work packets and proof obligations. |
| `plan-audit` | Independently audit the design and plan for correctness, library leverage, legacy bias, and executability. |
| `integrate-plan-audit` | Integrate audit findings through versioned, traceable edits. |
| `impl-plan-exec` | Execute work packets adaptively with proportional validation and durable state. |
| `impl-status` | Reconstruct progress and resume safely without modifying the immutable plan. |
| `implementation-review` | Review the resulting code in a fresh context against the design, plan, and actual behavior. |
| `lib-leverage` | Perform a deep review of one architecturally important dependency. |
| `skill-eval` | Evaluate the skill suite itself using fresh-session comparative trials. |

## Core workflow

```text
design-development
        |
        +-- library-capability-research (as needed)
        v
impl-plan
        v
plan-audit
        v
integrate-plan-audit
        v
impl-plan-exec <----> impl-status
        v
implementation-review
        v
focused remediation + re-review
```

`lib-leverage` is an optional deep dive before or during design. `skill-eval`
is a maintenance workflow for improving the skills themselves.

## Artifact model

The suite deliberately separates immutable specifications from mutable state:

```text
docs/designs/<topic>_design_vN_<date>.md
docs/plans/<topic>_implementation_plan_vN_<date>.md
docs/plans/state/<plan-slug>_state.json
docs/reviews/plan_audit_<plan-slug>_<date>_vN.md
docs/reviews/implementation_status_<plan-slug>_<date>_vN.md
docs/reviews/implementation_review_<plan-slug>_<date>_vN.md
```

Design and plan files are versioned specifications. Execution progress,
failed approaches, proving checks, and deviations belong in the state file.
Reviews are append-only provenance artifacts.

## Installation

Copy `.claude/skills/` into the repository's `.claude/skills/` directory.
Keep the `_shared` directory adjacent to the individual skill directories.

Archive the old `impl-plan-review` skill or rename it before installing
`impl-status`; the new skill intentionally no longer edits the plan.

## Project-specific references expected by this suite

The skills preserve the existing SmartRef reference structure and expect these
files when they are available:

- `docs/library_ref/semantic_design_principles_holistic.md`
- `docs/library_ref/mcp_code_intel_usage.md`
- `docs/library_ref/mcp_code_intel_for_skills.md`
- `docs/library_ref/improvement_criteria.md`

If a reference is absent, the invoking skill must state the limitation rather
than silently replacing project doctrine with generic advice.

## Important behavioral differences from the previous suite

1. Quality checks run at edit-local, work-packet, milestone, and final
   boundaries. A failing check is feedback, not a stopping condition.
2. Plans no longer predict a full patch. They declare outcomes, invariants,
   dependency order, change-surface evidence, and proof obligations.
3. Exact implementation snippets are conditional and limited to
   design-bearing contracts or fragile external APIs.
4. Every current component receives an explicit legacy disposition:
   preserve, reshape, temporarily encapsulate, replace, delete, or defer.
5. Parallel implementation is limited to independent packets with disjoint
   files or isolated worktrees.
6. The implementation reviewer judges the actual code and behavior, not merely
   whether predicted files changed.
