# Optional Enforcement and Subagent Configuration

This file is intentionally advisory. Repository-specific commands should be
implemented only after inspecting the actual manifests, CI, and team workflow.

## Deterministic checks worth scripting

### Plan/design lint

Validate:

- YAML frontmatter and required fields;
- stable ID uniqueness and references;
- packet dependency acyclicity;
- milestone and decommission prerequisites;
- plan/design/state path and digest consistency;
- required acceptance and gate sections;
- every cutover has a negative proof;
- every design legacy disposition maps to a packet or batch.

### Packet completion

A script can accept `WPnn` and check:

- dependencies complete;
- required command results recorded;
- acceptance evidence present;
- no open blocker;
- decommission proof present when applicable;
- state timestamps/digests updated.

### Stop-time completion

A `Stop` hook can reject a "complete" claim when:

- ready/in-progress required packets remain;
- a milestone or decommission batch is open;
- final gates are missing/failed;
- state and plan digests disagree;
- unresolved blocker/major implementation-review findings remain.

Keep the hook's output concise and actionable.

### Post-edit feedback

A `PostToolUse` or `PostToolBatch` hook can run only very fast checks:

- changed-file formatter;
- parser/syntax check;
- narrow lint;
- generated-file drift check.

Do not run the full suite after every edit.

## Subagent definition guidance

A read-only reviewer should omit editing tools and have a detailed review
description. A packet implementer should receive only the tools and skills
needed for implementation and local proof.

Parallel writing agents should use worktree isolation. The lead session owns
merge order and milestone/final checks.

## Suggested hard permission boundaries

Use project permission settings or hooks—not skill prose alone—to prevent:

- automatic invocation of side-effecting workflows;
- edits to accepted design/plan artifacts during execution;
- destructive Git or filesystem commands;
- code edits by review agents;
- bypassing required completion scripts;
- writes outside assigned worktrees for parallel agents.

## Context-management hooks

Before compaction or session end, a hook or standing instruction can require:

- state file updated;
- current packet and next action set;
- failed approaches persisted;
- last command/failure summarized;
- uncommitted change identity recorded.

After compaction, reload the active packet, design references, and state rather
than the entire planning history.
