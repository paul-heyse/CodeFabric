---
name: impl-plan
description: Convert an accepted target design into a versioned implementation plan of dependency-closed work packets, change-surface evidence, legacy cutovers, replan triggers, and layered proof obligations.
when_to_use: Use after design-development has accepted a target design, or for a bounded change whose architecture and library choices are already settled and evidenced.
argument-hint: "[design-path] [vN]"
allowed-tools: Read, Glob, Grep, Bash, Write, Agent, WebSearch, WebFetch
user-invocable: true
---

# Implementation Plan Generator

Generate an execution specification from an accepted design. The plan states
what must become true, why, in what dependency order, and how it will be
proved. It does not attempt to predict the full patch.

Read:

- `../_shared/evidence-policy.md`
- `../_shared/code-intelligence.md`
- `../_shared/doctrine-policy.md`
- `../_shared/validation-policy.md`
- `../_shared/subagent-orchestration.md`
- the implementation-plan section of `../_shared/artifact-schemas.md`

## Inputs and output

Interpret `$ARGUMENTS` as a design path and optional plan version. If no design
path is supplied, infer it from the conversation and repository. A plan may be
generated without a separate dossier only for a bounded change whose outcomes,
contracts, library choices, legacy disposition, and proof strategy are already
settled; capture those decisions in the plan rather than silently inventing
them.

Write:

```text
docs/plans/<topic>_implementation_plan_vN_<YYYY-MM-DD>.md
```

Never overwrite a prior plan version.

## Planning detail admission rule

Include implementation detail only when it affects at least one of:

1. architecture, ownership, dependency direction, or a public contract;
2. dependency order or blast radius;
3. feasibility of a pinned external API;
4. correctness, security, reliability, performance, or operations;
5. acceptance, migration, rollback, cutover, or decommission proof.

Leave local algorithms, helper names, exact control flow, and ordinary
boilerplate to execution unless they carry one of those consequences.

Use a signature, schema, state transition, or short pseudocode exemplar instead
of a full implementation body. No code exemplar is required by default.

## Procedure

### Phase 1 — Ingest and validate the design

Read the design dossier end-to-end and extract:

- accepted outcomes and non-goals;
- target invariants and governing decisions;
- library decisions and verified version basis;
- legacy disposition matrix;
- transition/cutover design;
- test oracle and conformance strategy;
- risks, assumptions, and design-level replan triggers.

Verify that the dossier is accepted. Do not plan around unresolved blockers.
Record the baseline commit and the declared-inputs table — one `path |
sha256` row per document the plan depends on (`shasum -a 256`), computed
once now and thereafter recomputed only by tooling per
`artifact-schemas.md` §8.

If current repository drift may invalidate the design, perform a bounded
staleness probe before planning and record the result.

### Phase 2 — Map the impact surface

Use structural code intelligence for load-bearing interfaces and legacy
targets. Identify:

- direct and transitive consumers;
- Protocol/trait implementations and subclasses;
- construction, structuring, serialization, persistence, and fixture sites;
- public exports, registration, configuration, and generated-code surfaces;
- tests, governance rules, and operational tooling;
- Python/Rust or native-extension boundaries;
- old paths that must reach zero.

Do not pretend every downstream file is certain before implementation exists.
The plan carries the question, not a frozen answer:

- **Preflight query:** the exact current-tree commands or rules the executor
  runs before editing to discover the change surface.
- **Known touch (verified this session):** optionally, the files whose
  involvement follows directly from evidence gathered in this planning
  session.

Global and negative claims require complete evidence coverage.

### Phase 3 — Form dependency-closed work packets

Create stable packet IDs `WP01`, `WP02`, ...

A packet is dependency-closed when it includes the contracts and immediate
consumers needed to leave the affected subsystem coherent and testable.

Group work by dependency and coherent proof boundary, not by one file, one
conversation bullet, or one design section. Combine tightly coupled contract
and consumer changes. Split independent workstreams.

Each packet must contain:

```markdown
## WP01 — <title>

### Outcome
### Dependencies
### Target Invariants
### Design and Library References
### Change Surface
#### Preflight Query
#### Known Touch (verified this session)
### Required Changes
### Legacy Disposition and Decommission
### Acceptance Checks
#### Behavioral
#### Structural
#### Negative / Zero-State
#### Operational
### Edit-Local Gates
### Packet-Local Gates
### Integration Milestone
### Replan Triggers
### Rollback or Recovery
### Design-Bearing Contracts and Exemplars (conditional)
```

A packet outcome must be observable. "Implement the new module" is not enough.

Every acceptance-check item is a named executable check — a test name, a
`rules/` rule id, a `just` recipe, or a probe command. A packet is
**complete** when these checks pass at its proving commit and at HEAD
(`artifact-schemas.md` §2); prose that cannot be run is not an acceptance
check.

### Phase 4 — Carry legacy decisions into execution

For every design `L-*` disposition:

- map it to one or more packets or a cross-packet decommission batch;
- name the old symbol, module, route, authority, registration, data, or pattern;
- state when deletion becomes safe;
- include positive target-state and negative old-state proof;
- prohibit silent compatibility aliases, dual writes, or duplicate authority
  unless the design explicitly authorizes a bounded transition.

Use `DB01`, `DB02`, ... for deletion that becomes safe only after several
packets. State prerequisites and mechanized exit invariants.

### Phase 5 — Define integration milestones

Use `M01`, `M02`, ... where packets first interact or risk accumulates.
Milestones should be few and meaningful. Examples:

- a new contract and all adapters compile and pass contract tests;
- a new semantic path handles a representative end-to-end flow;
- migration reads old and new persisted forms before final cutover;
- the old authority reaches zero and governance prohibits reintroduction.

List the packets, required evidence, and milestone gates.

### Phase 6 — Build the layered gate matrix

Discover gates from `just --list`; the justfile is the gate registry. When a
needed gate has no recipe, the plan proposes adding one rather than
embedding raw flags.

For each packet and milestone, select the smallest relevant checks from
`../_shared/validation-policy.md`. Then define the final matrix — a list of
`just` recipes — across all affected toolchains.

Record baseline failures rather than assuming a clean repository. Do not make
a Python-only final gate for mixed Python/Rust scope.

### Phase 7 — Define replan behavior

Packet-level replan triggers must cover material uncertainty, including:

- a planned library API is absent or behaviorally incompatible;
- the current consumer surface is materially larger or different;
- the packet cannot be left dependency-closed;
- a target invariant cannot be satisfied with the planned mechanism;
- migration requires unbounded dual authority;
- security, performance, reliability, or operational evidence invalidates the
  approach.

Distinguish:

- **implementation adaptation:** stays within accepted design/invariants and is
  recorded in execution state;
- **plan revision:** packet boundaries, sequence, or proof obligations change;
- **design reopening:** architecture, public contract, library decision, or
  target invariant changes.

### Phase 8 — Independent plan challenge

For large plans, delegate bounded lenses:

- impact/file-surface completeness;
- packet dependency and sequence;
- library/API grounding;
- test/proof and operational completeness;
- legacy cutover and clean-sheet consistency.

The lead synthesizes. Do not paste raw subagent output into the plan.

### Phase 9 — Write the plan and initialize state contract

Write the plan using the shared schema. Include:

- design path/version, baseline, and the declared-inputs table;
- immutable work-packet specification;
- state-file path (state contract: schema version 2);
- execution sequence covering all packets, milestones, and decommission
  batches;
- explicit plan risks and replan policy.

Forbidden in the body: digest tables beyond the declared-inputs table,
hand-built traceability matrices, and restamp procedures
(`artifact-schemas.md` §2).

Do not create or update execution state unless the user also asks to begin
execution.

## Quality requirements

- Every packet traces to design decisions/invariants.
- Every load-bearing library use traces to a verified `LD-*` decision.
- Every material legacy surface has an execution disposition.
- Every cutover has an exit invariant and negative proof.
- Every packet is coherent enough for local validation.
- File predictions are limited to session-verified known-touch entries;
  everything else is a preflight query.
- Exemplars are conditional and design-bearing.
- The sequence is a dependency graph, not a prose wish list.
- The final gate matrix covers all affected languages and integration
  boundaries.
- The plan is compact enough that an executor can load one packet at a time.
