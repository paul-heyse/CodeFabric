# Subagent Orchestration

Use subagents to isolate evidence gathering, protect the implementation
context, and parallelize genuinely independent work. Do not use them merely to
increase agent count.

## 1. Choose the right context

- **Fork**: use when the side task benefits from the current conversation and
  prompt-cache continuity, but its tool transcript should stay out of the main
  context.
- **Fresh named subagent**: use for independent research, adversarial review,
  or a specialist that should not inherit the author's framing.
- **Inline lead execution**: use for tightly coupled implementation, ongoing
  architectural judgment, and cross-packet integration.
- **Agent team**: reserve for separable workstreams that genuinely need
  peer-to-peer coordination. Do not use for a single tightly coupled pipeline.

## 2. Design and planning lenses

For large or cross-language work, parallelize independent evidence lenses:

1. **Repository architect** — current boundaries, ownership, dependency
   direction, semantic/control/data flows, and architectural anomalies.
2. **Impact analyst** — callers, implementations, construction sites,
   persistence/serialization, tests, and decommission surface.
3. **Library researcher** — required capabilities, pinned versions, official
   APIs, upgrade options, and executable probes.
4. **Quality and operations analyst** — current oracles, tests, failures,
   observability, security, lifecycle, performance, and migration concerns.
5. **Design challenger** — clean-sheet alternative, legacy-preserving bias,
   accidental complexity, and simpler designs.

The lead owns synthesis. Do not ask several agents to independently produce
overlapping final architectures unless comparing alternatives is the explicit
task.

## 3. Implementation delegation criteria

Delegate a work packet only when all are true:

- its architecture and contracts are settled;
- dependencies are already landed;
- it is dependency-closed or has an explicit milestone boundary;
- its acceptance evidence and local gates are concrete;
- it touches disjoint files, or it runs in an isolated worktree;
- it does not require continuous cross-packet design judgment.

Parallel writers should use isolated worktrees. The lead merges deliberately,
resolves conflicts, and runs integration checks.

## 4. Handoff packet

A handoff must be self-contained but compact:

```markdown
Task: WP03 — <title>

Outcome:
<what must be true>

Authoritative current facts:
- <verified fact>
- <stale-plan correction>

Dependencies already landed:
- <packet/contract>

Read first:
- <specific files or symbols>

Change surface:
- Preflight query: <commands to run before editing>
- Known-touch (verified): <optional; only files verified this session>

Required changes and legacy disposition:
- ...

Acceptance checks (named, executable):
- Behavioral: <test / recipe / probe>
- Structural: ...
- Negative: ...
- Operational: ...

Local gates:
- ...

Constraints:
- No scaffolding, placeholders, silent compatibility layers, or unrelated refactors.
- Re-verify current-tree facts before load-bearing edits.
- Adapt implementation detail when needed, but do not silently change target invariants.
- Record any replan trigger or plan discrepancy.

Return:
- Files changed/created/deleted.
- What was implemented.
- Acceptance checks run: name + exit status for each.
- Decommission proof.
- Deviations, failed approaches, blockers, and remaining risks.
```

Generic instructions belong in the subagent definition or shared reference, not
repeated in every prompt.

## 5. Review independence

Use a fresh context for implementation review. Give the reviewer:

- the accepted design;
- the implementation plan;
- execution state and deviations;
- the baseline and final diff;
- test/gate evidence.

Do not give the review agent the implementation author's private narrative or
ask it to defend the chosen patch. It should reconstruct correctness from the
artifacts and code.

## 6. Context discipline

- Return synthesized evidence, not raw file dumps.
- Limit each researcher to a question and bounded scope.
- Prefer one strong subagent result over several redundant ones.
- Persist durable facts and failed approaches in artifacts/state before
  compaction or session rollover.
- Do not re-run broad discovery already captured in an authoritative handoff
  unless the current tree contradicts it.
