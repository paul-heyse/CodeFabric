---
name: design-development
description: Develop and challenge a target software design before implementation planning. Use for large, cross-module, architectural, migration, or library-sensitive changes where current code may bias the solution.
when_to_use: Use when the desired outcome is understood but the target architecture, library choices, legacy disposition, transition, or proof strategy is not yet settled.
argument-hint: "[topic] [vN]"
allowed-tools: Read, Glob, Grep, Bash, Write, Agent, WebSearch, WebFetch
user-invocable: true
---

# Target Design Development

Produce a versioned design dossier that is independent of the current file
layout, grounded in repository evidence, and explicit about off-the-shelf
capabilities, legacy disposition, transition, and proof.

Read only the shared references needed for this invocation:

- `../_shared/evidence-policy.md`
- `../_shared/code-intelligence.md`
- `../_shared/doctrine-policy.md`
- `../_shared/subagent-orchestration.md`
- the design-dossier section of `../_shared/artifact-schemas.md`

## Invocation

Interpret `$ARGUMENTS` as an optional topic slug and optional version. Infer
missing values from the conversation.

Write:

```text
docs/designs/<topic>_design_vN_<YYYY-MM-DD>.md
```

Never overwrite a prior design version.

## Operating principles

1. **Design the target before designing the migration.** First determine the
   clean target architecture under genuine external constraints. Then map the
   current system onto it.
2. **Current code is evidence, not authority.** Existing modules, abstractions,
   and ownership boundaries must earn preservation.
3. **Capability research is problem-first.** Derive required capabilities
   before selecting libraries or deciding to build custom code.
4. **Design detail is admitted by consequence.** Include detail only when it
   affects architecture, contracts, dependency direction, feasibility,
   correctness, security, reliability, performance, migration, or proof.
5. **Every durable decision has an oracle.** State how correctness will be
   distinguished from plausible-looking implementation.
6. **No silent intermediate architecture.** Temporary dual paths, adapters, or
   migration states require a bounded purpose, owner, exit invariant, and
   decommission condition.

## Procedure

### Phase 1 — Frame the decision

Extract from the conversation and repository:

- desired outcomes and user-visible behavior;
- non-goals;
- external compatibility, data, operational, schedule, and deployment
  constraints;
- measurable quality attributes;
- decisions already settled versus assumptions still open.

Rewrite vague goals into observable outcomes. Do not solve an ambiguity by
quietly choosing the current implementation's behavior.

Record a baseline commit. If the tree is intentionally dirty, record the
working-tree identity and relevant diff instead.

### Phase 2 — Build a bounded current-state model

Read the doctrine when required by `../_shared/doctrine-policy.md`.

Use targeted structural analysis, not exhaustive file dumping, to identify:

- current ownership and sources of truth;
- public and internal contracts;
- data, semantic, control, mutation, and failure flows;
- callers, implementations, construction/serialization sites, and tests;
- custom code surrounding candidate libraries;
- parallel paths, aliases, dual writes, compatibility layers, and dead or
  half-migrated architecture;
- operational constraints and existing proof or observability surfaces.

Capture only facts that affect design. Record each per `evidence-policy.md`
§6: the reproducible command plus its coverage envelope; promote any
invariant asserted twice to a `rules/` governance rule.

### Phase 3 — Derive required capabilities

Before naming a solution, state the capabilities the target needs, including
as applicable:

- semantic/domain representation;
- parsing, compilation, query, storage, transaction, or runtime behavior;
- concurrency and incremental update semantics;
- failure taxonomy and recovery;
- resource ownership and lifecycle;
- observability and provenance;
- security and trust boundaries;
- interoperability and public contracts;
- test and conformance oracles;
- performance and scalability envelopes.

Separate hard requirements from preferences.

### Phase 4 — Research off-the-shelf capabilities

For architecturally material capabilities, either invoke
`/library-capability-research` or delegate a focused library-research subagent.

Research in this order:

1. language and standard-library facilities;
2. already-pinned dependencies;
3. a supported upgrade of an existing dependency;
4. credible new dependencies;
5. small custom implementation only where the prior options fail the target
   constraints.

Verify the pinned version and load-bearing APIs using official documentation
and a minimal import/compile/probe when material. Record `LD-*` decisions
per the LD block in `artifact-schemas.md` §1.

Do not maximize dependency count. Prefer the option with the best combination
of capability fit, inspectability, operability, testability, ecosystem health,
and total maintenance cost.

### Phase 5 — Develop material alternatives

Develop at least two materially different designs for large architectural
work. One may be the existing-direction evolution, but at least one must be a
clean-sheet alternative.

For each alternative, compare:

- target-invariant satisfaction;
- conceptual integrity and doctrine;
- library leverage;
- change surface and migration complexity;
- failure and operational behavior;
- testability and observability;
- performance/scalability;
- lock-in and reversibility;
- legacy retained or deleted;
- principal risks.

Do not generate artificial alternatives that differ only in naming or file
placement.

### Phase 6 — Select and specify the target

Define:

- component responsibilities and dependency direction;
- stable contracts and semantic identities;
- data/control/mutation flows;
- ownership of durable state, temporal state, and scarce resources;
- boundaries around external libraries and vendors;
- failure categories and recovery semantics;
- observability, provenance, and support surfaces;
- security and trust boundaries;
- performance-critical paths and budgets;
- extension points and executable governance.

Prefer diagrams, signatures, schemas, state machines, or short pseudocode only
when they carry a design decision. Do not write implementation bodies.

Mint a stable ID only for items the plan or another artifact will reference
(`artifact-schemas.md`, intro rule).

### Phase 7 — Perform the clean-sheet and legacy challenge

Ask explicitly:

> Would this still be the preferred design if the current implementation did
> not exist?

Build the Legacy Disposition Matrix over a generated inventory, not
open-ended re-reading: enumerate the material surfaces with
`ast-grep outline <scope> --items exports` (plus `rg --files` over non-code
surfaces in scope) and cite the generating command in the dossier. Every
surface in that inventory receives one disposition:

```text
preserve | reshape | encapsulate-temporarily | replace | delete | defer
```

A retained surface must have a concrete justification. Recommend a hard pivot
when adapting the legacy architecture would preserve duplicate authority,
forbidden dependency direction, permanent compatibility layers, domain truth
inside infrastructure/UI/workflow objects, repeated special cases, or custom
reimplementation of a verified library capability.

For temporary encapsulation, state the exit invariant and latest safe removal
point.

### Phase 8 — Design the transition

Only after the target is stable, map current to target:

- foundation and contract sequence;
- data/schema/protocol migration;
- atomic versus incremental cutover;
- compatibility commitments that are genuinely external;
- rollback/recovery;
- decommission order;
- intermediate states and how they remain coherent;
- observability required to judge the transition.

Avoid long-lived dual authority. If an atomic cutover is safer, say so.

### Phase 9 — Define proof before planning

Specify the test oracle and conformance strategy:

- unit, property, differential, contract, integration, migration, concurrency,
  recovery, performance, or end-to-end proof as applicable;
- structural governance and dependency rules;
- positive target-state evidence;
- negative legacy zero-state evidence;
- baseline comparison;
- representative workloads and failure injection.

A design is not ready when its success criterion is merely "the new modules
exist."

### Phase 10 — Independent challenge

For a large design, delegate at least these independent lenses when useful:

- repository/impact analyst;
- library capability researcher;
- quality/operations analyst;
- fresh-context design challenger.

The challenger must look for legacy-preserving bias, unnecessary abstraction,
unproven library assumptions, missing failure behavior, and a simpler target.

The lead resolves findings and owns the final design. Do not paste subagent
reports into the dossier.

### Phase 11 — Write and decide

Write the dossier using the shared schema — six core sections, with an
optional section added only when it carries a decision. Its final section
must state one:

- `accepted for implementation planning`;
- `accepted with named assumptions to validate in plan preflight`;
- `not ready — design blockers remain`.

A dossier is not accepted while any load-bearing library API is unverified, a
target invariant conflicts with the transition, or a material legacy surface
has no disposition.

## Quality bar

The dossier must allow a planner unfamiliar with the conversation to answer:

- What must become true?
- Why is this architecture preferred?
- Which current architecture is intentionally abandoned?
- Which library capabilities are used and at what verified version?
- How does the system move from current to target?
- How will correctness and decommission be proved?
- What evidence would force the design to be reopened?
