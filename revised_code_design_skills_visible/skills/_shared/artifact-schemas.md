# Artifact Schemas

These schemas define the contract among design, planning, execution, status,
audit, and review skills. Preserve stable identifiers and version artifacts
rather than rewriting history.

## 1. Design dossier

Path:

```text
docs/designs/<topic>_design_vN_<YYYY-MM-DD>.md
```

Required frontmatter:

```yaml
---
artifact: design-dossier
design_id: <topic>
version: vN
date: YYYY-MM-DD
status: draft|accepted|superseded
baseline_commit: <git-ref-or-working-tree-id>
primary_scope:
  - <paths-or-modules>
doctrine_path: docs/library_ref/semantic_design_principles_holistic.md
---
```

Required body:

1. Executive decision
2. Problem, outcomes, and non-goals
3. Constraints and measurable quality attributes
4. Current-state evidence and architecture
5. Target architecture
6. Target invariants, contracts, ownership, and flows
7. Library/platform capability decisions
8. Alternatives and decision rationale
9. Clean-sheet challenge
10. Legacy disposition matrix
11. Transition, cutover, rollback, and decommission
12. Failure, security, resource lifecycle, observability, and performance
13. Test oracle and conformance strategy
14. Risks, assumptions, and design-level replan triggers
15. Acceptance decision and open blockers
16. Evidence ledger

Use stable IDs:

- `D-01` design decisions
- `I-01` target invariants
- `A-01` assumptions
- `R-01` risks
- `LD-01` library decisions
- `L-01` legacy dispositions

## 2. Implementation plan

Path:

```text
docs/plans/<topic>_implementation_plan_vN_<YYYY-MM-DD>.md
```

Required frontmatter:

```yaml
---
artifact: implementation-plan
plan_id: <topic>
version: vN
date: YYYY-MM-DD
status: draft|audited|approved|superseded
design_path: <path>
design_version: vN
baseline_commit: <git-ref-or-working-tree-id>
state_path: docs/plans/state/<plan-slug>_state.json
cutover: true|false
---
```

Required body:

1. Outcome and non-goals
2. Source design and governing decisions
3. Current baseline and staleness boundary
4. Global target invariants
5. Library decisions carried into execution
6. Work packets (`WP01`, `WP02`, ...)
7. Integration milestones (`M01`, `M02`, ...)
8. Cross-packet decommission batches (`DB01`, ...)
9. Final gate matrix
10. Execution sequence
11. Completion checklist
12. Plan risks and replan policy

Each work packet contains:

- outcome;
- dependencies;
- target invariants;
- design references;
- change surface: must-touch, likely-touch, and execution-preflight discovery;
- required changes;
- legacy disposition/decommission;
- acceptance evidence: behavioral, structural, negative, operational;
- edit-local and packet-local gates;
- integration milestone;
- replan triggers;
- rollback/recovery;
- design-bearing contracts/exemplars only when necessary.

## 3. Execution state

Path:

```text
docs/plans/state/<plan-slug>_state.json
```

Minimum shape:

```json
{
  "schema_version": 1,
  "plan_path": "...",
  "plan_digest": "...",
  "design_path": "...",
  "design_digest": "...",
  "baseline_commit": "...",
  "current_head": "...",
  "status": "not_started",
  "current_packet": null,
  "packets": {
    "WP01": {
      "status": "not_started",
      "dependencies": [],
      "started_at": null,
      "completed_at": null,
      "proving_commit": null,
      "changed_files": [],
      "acceptance_evidence": [],
      "checks": [],
      "deviations": [],
      "failed_approaches": [],
      "blockers": []
    }
  },
  "milestones": {},
  "decommission_batches": {},
  "baseline_failures": [],
  "discovered_obligations": [],
  "plan_deviations": [],
  "failed_approaches": [],
  "next_action": null,
  "updated_at": "..."
}
```

Packet statuses:

```text
not_started | ready | in_progress | blocked | complete | stale | invalidated
```

A previously complete packet remains trusted only when its proving commit is
in current history, its relevant code and governing contract have not changed,
and its checks remain applicable.

## 4. Audit findings

Use stable IDs `F-001`, `F-002`, ... across one audit version.

Each finding contains:

```markdown
### F-001 — <title>

**Severity:** blocker | major | minor | observation
**Category:** factuality | design | library | impact | legacy | proof |
sequence | doctrine | operations | context-efficiency
**Scope:** design D-03; plan WP04
**Claim:** ...
**Evidence:** ...
**Impact:** ...
**Required resolution:** ...
**Revalidation:** ...
```

An audit verdict is:

```text
ready | ready-with-corrections | needs-revision | needs-redesign
```

## 5. Audit integration disposition

Every finding receives exactly one disposition:

```text
applied-plan | applied-design | added-packet | added-decommission |
covered-by:<finding> | deferred | rejected | requires-redesign
```

The integration log records the exact edit, rationale, and re-verification.

## 6. Implementation review findings

Use stable IDs `IR-001`, `IR-002`, ...

Each finding contains severity, dimension, evidence, affected behavior,
design/plan references, remediation, and a focused re-test.

Verdict:

```text
approved | approved-with-minor-findings | changes-required | design-invalidated
```
