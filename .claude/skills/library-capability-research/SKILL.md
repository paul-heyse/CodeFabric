---
name: library-capability-research
description: Research the best off-the-shelf capabilities for a target design, grounded in repository requirements, pinned versions, official documentation, and executable probes. Use before committing to custom architecture or a library API.
when_to_use: Use when a design depends on storage, query, parsing, compilation, concurrency, serialization, orchestration, UI, solver, testing, or other ecosystem capabilities that may already exist.
argument-hint: "[topic-or-capability] [--scope path] [--design path]"
allowed-tools: Read, Glob, Grep, Bash, Write, Agent, WebSearch, WebFetch
user-invocable: true
context: fork
background: false
---

# Library Capability Research

Answer a capability decision, not "what features does library X have?" Start
from the target requirements, compare viable ways to satisfy them, and produce
version-grounded decisions that a design and implementation plan can rely on.

Read:

- `../_shared/evidence-policy.md`
- `../_shared/code-intelligence.md`
- the LD block in `../_shared/artifact-schemas.md` §1 and the
  evidence-recording rules in `../_shared/evidence-policy.md` §6

When a design path is supplied, read only the relevant requirements,
constraints, invariants, and open library decisions.

## Output

Write a versioned report:

```text
docs/reviews/library_capability_<topic>_<YYYY-MM-DD>_vN.md
```

Frontmatter per `artifact-schemas.md` §7
(`artifact: library-capability-research`, `topic`, `version`, `date`,
`status`).

If the invoking design workflow requests inline output, return the same
structured findings to the parent rather than duplicating a report.

Never overwrite a prior report.

## Procedure

### 1. State the capability question

Express the decision without assuming a library:

- required behavior;
- non-functional constraints;
- integration boundaries;
- version, language, platform, deployment, and licensing constraints;
- required proof;
- what custom code or architecture could be displaced.

Separate must-haves from preferences and future possibilities.

### 2. Inventory the actual environment

Inspect all relevant manifests and lockfiles, not only `pyproject.toml`:

- Python package and tool configuration;
- Cargo workspace/crate manifests and lockfile;
- JavaScript/package-manager manifests where present;
- system/native build declarations;
- CI and deployment configuration;
- existing local library reference documents.

Record:

- declared version range;
- resolved/installed version;
- enabled features/extras;
- language/runtime compatibility;
- current wrappers and usage sites;
- version skew or optional-path differences.

Use structural search for current imports, calls, wrappers, and hand-rolled
alternatives.

### 3. Build the candidate set

Consider, in order:

1. language or standard-library capability;
2. already-pinned library capability;
3. upgrade of an existing library;
4. credible new library or service;
5. narrowly scoped custom implementation;
6. no change, when the requirement does not justify new machinery.

Do not compare candidates that fail a hard constraint. Do not add candidates
solely to make the table look comprehensive.

### 4. Verify capabilities

For each load-bearing candidate:

- use official versioned documentation, release notes, and source repository;
- verify availability in the actual pinned/resolved version;
- identify feature flags, optional extras, platform restrictions, and maturity;
- inspect error, transaction, concurrency, resource-lifecycle, and
  interoperability behavior;
- run a minimal import, type, compile, or behavior probe when documentation
  alone leaves material uncertainty;
- note upgrade and migration implications.

Secondary sources may suggest a lead but are not API authority.

A feature that exists only in a newer release is an `upgrade` decision, not an
available pinned-version capability.

### 5. Compare lifecycle fit

Evaluate each candidate against:

- semantic correctness and target-invariant fit;
- API stability and inspectability;
- performance/scalability for representative workloads;
- failure and recovery behavior;
- resource ownership and operational model;
- observability and diagnostics;
- testability and deterministic behavior;
- security and trust;
- cross-language and ecosystem interoperability;
- lock-in, reversibility, and ecosystem health;
- total custom code and long-term maintenance;
- migration risk.

Do not treat fewer lines of code as sufficient if the dependency creates an
opaque or operationally mismatched subsystem.

### 6. Decide

For each required capability, record one decision:

```text
adopt | wrap | upgrade | build | reject | retain-current
```

Record each decision per the LD block in `artifact-schemas.md` §1 — the
single definition of the `LD-*` record. Add the target design role and
integration consequences when the block's fields do not already carry them.

A wrapper is justified only when it protects a stable project contract,
normalizes vendor behavior, adds material policy/observability, or makes
replacement feasible. Do not preserve pass-through wrappers by default.

### 7. Challenge the custom-build option

When recommending custom code, state why the best library/native alternatives
fail. When recommending a dependency, state why a small in-repo implementation
would be worse.

For existing custom infrastructure, ask whether it exists because the library
once lacked a feature that is now available.

### 8. Write the report

Use this structure:

```markdown
# Library Capability Research: <topic>

## Decision Summary
## Capability Requirements
## Environment and Version Baseline
## Current Code and Custom Infrastructure
## Candidate Matrix
## Verified Capability Findings
## Library Decisions
## Custom Code Displaced or Retained
## Upgrade and Migration Implications
## Risks and Open Validation
## Recommended Design Integration
## Evidence and Reproduction Commands
```

The candidate matrix should include capability fit, version basis, effort,
risk, operational fit, custom code displaced, and decision.

## Quality requirements

- No API claim without pinned-version evidence.
- No recommendation without a concrete project application site.
- No "use library X" conclusion that omits ownership, failure, operational, or
  migration behavior.
- No assumption that the latest release is compatible with the repository.
- No forced adoption: `build` or `retain-current` is valid when justified.
- No exhaustive feature catalog. Research only features relevant to the
  capability question.
- Clearly distinguish verified behavior from a planned probe.
