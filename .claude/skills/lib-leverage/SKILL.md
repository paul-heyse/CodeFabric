---
name: lib-leverage
description: Deeply review one current or proposed library for underused capabilities, architecture fit, custom-code displacement, version upgrades, and outcome impact using the project's O01-O18 criteria.
when_to_use: Use when a dependency is architecturally central, substantial custom code surrounds it, a version upgrade may change the design, or the code appears to fight the library's intended model.
argument-hint: "[library] [--scope path] [--focus outcomes] [--depth surface|moderate|deep]"
allowed-tools: Read, Glob, Grep, Bash, Write, Agent, WebSearch, WebFetch
user-invocable: true
context: fork
background: false
agent: general-purpose
---

# Deep Library Leverage Review

Assess whether the project uses a specific library in the most effective way
for the accepted or candidate target architecture. This is deeper and
library-specific; use `/library-capability-research` when the problem has not
yet selected a candidate library.

Read:

- `docs/library_ref/improvement_criteria.md`
- `../_shared/evidence-policy.md`
- `../_shared/code-intelligence.md`
- relevant design dossier and `LD-*` decisions when present.

If the improvement-criteria file is absent, state the limitation and do not
silently replace its definitions.

## Output

Write:

```text
docs/reviews/lib_leverage_<library>_<YYYY-MM-DD>_vN.md
```

Frontmatter per `artifact-schemas.md` §7 (`artifact: lib-leverage`,
`library`, `version`, `date`, `status`).

Never overwrite a prior review.

## Review stance

- Evaluate relevant capabilities, not the library's entire marketing surface.
- Ground recommendations in the resolved version and concrete code.
- Maximize lifecycle leverage, not dependency usage for its own sake.
- Respect the accepted target architecture. Do not preserve the current
  architecture merely because it exists.
- Recommend a hard pivot when wrappers/custom infrastructure fight the
  library, duplicate its authority, or prevent target invariants.
- A small custom implementation may be preferable when it is more inspectable,
  testable, operationally integrated, and stable for the requirement.

## Procedure

### Phase 1 — Establish version and design context

Identify from all relevant manifests and lockfiles:

- declared and resolved versions;
- enabled features/extras;
- optional/platform-specific paths;
- current upgrade constraints;
- local reference documentation;
- design decisions that selected or rejected the library.

Verify official docs and source for the resolved version. If the review depends
on a newer feature, treat it as an upgrade opportunity with migration cost.

### Phase 2 — Map current usage and custom infrastructure

Use structural search to inventory:

- imports and entry points;
- wrappers, adapters, helper layers, and type conversions;
- configuration and feature flags;
- transaction, concurrency, resource, cache, and error handling;
- serialization/interoperability boundaries;
- tests and observability;
- hand-rolled implementations of library capabilities;
- anti-patterns and workarounds;
- cross-language consumers.

Sample by impact, not arbitrary file count. For deep mode, cover the complete
declared scope and inspect query coverage.

### Phase 3 — Model the relevant feature surface

Group only capabilities that could plausibly serve the codebase, such as:

- core data/model APIs;
- advanced execution/query/algorithm features;
- incremental, transactional, or concurrency behavior;
- validation/type/schema facilities;
- resource lifecycle and failure handling;
- observability, diagnostics, and provenance;
- extension/plugin interfaces;
- performance, batching, pushdown, caching, and parallelism;
- interoperability and persistence;
- testing and deterministic modes;
- maintenance and migration utilities.

For each capability, verify version availability and material limitations.

### Phase 4 — Identify leverage opportunities

A candidate opportunity must have:

- a concrete current site or architectural concern;
- a verified library capability;
- a target-state application;
- custom code displaced, simplified, or made safer;
- a plausible proof strategy.

High-value patterns include:

- custom code reimplementing a native capability;
- wrappers adding no stable project policy;
- conversions that defeat zero-copy or pushdown behavior;
- manual lifecycle/transaction logic the library can own;
- old workarounds for fixed limitations;
- feature flags or newer versions that remove architectural constraints;
- separate project abstractions duplicating a library's canonical model;
- missing observability or testing facilities already provided by the library.

Do not call every unused feature an opportunity.

### Phase 5 — Assess O01-O18 outcomes

Use the project definitions. For each opportunity, list only outcomes with
`moderate` or `strong` impact.

Also assess:

- effort: `small | medium | large`;
- risk: `low | medium | high`;
- confidence: `high | medium | low`;
- migration shape;
- target-design impact;
- custom code deleted/retained;
- version/feature prerequisite;
- focused validation.

Do not force-fit governance, security, or performance benefits.

### Phase 6 — Challenge architecture fit

For each major wrapper or custom layer, decide:

```text
retain | narrow | replace-with-library | replace-library | delete |
revisit-target-design
```

Ask:

- Does the layer protect a stable domain contract or only mirror the vendor?
- Does it normalize failure/observability/lifecycle policy?
- Does it make replacement feasible in practice?
- Does it preserve duplicate truth or block library-native optimization?
- Would the layer exist in a clean-sheet design?
- Does adopting a deeper library primitive change packet or migration design?

A recommendation may replace a foundational current choice when evidence shows
it is inconsistent with the accepted target or materially inferior. Record the
design consequence rather than hiding it as a "quick win."

### Phase 7 — Verify material claims

Use minimal executable probes for uncertain:

- imports and signatures;
- feature flags;
- schema/type conversion;
- query/optimizer behavior;
- transaction or concurrency semantics;
- performance-critical behavior;
- cross-language interoperability.

Do not publish aspirational API snippets.

### Phase 8 — Prioritize

Rank by value density and dependency order:

1. correctness/architecture blockers;
2. capabilities that delete large custom surfaces;
3. low-effort, high-confidence leverage;
4. upgrades that unlock target design;
5. performance/scalability improvements with representative evidence;
6. optional ergonomics.

Separate recommendations appropriate for the current plan from follow-up
design work.

### Phase 9 — Write the review

Use:

```markdown
# Library Leverage Review: <library>

## Provenance and Version Baseline
## Executive Summary
## Target-Architecture Role
## Current Usage Maturity
## Relevant Capability Surface
## Opportunity Index
## High-Priority Opportunities
## Medium-Priority Opportunities
## Low-Priority Opportunities
## Custom Infrastructure Disposition
## Anti-Patterns and Library-Fighting Code
## Version Upgrade Opportunities
## O01-O18 Outcome Coverage
## Recommended Design/Plan Integration
## Validation Probes and Open Questions
## Evidence and Reproduction Commands
```

Each opportunity:

```markdown
### LL-01 — <title>

**Capability and version basis:** ...
**Current code:** ...
**Target application:** ...
**Custom code disposition:** ...
**Architecture effect:** ...
**Outcome criteria:** ...
**Effort / risk / confidence:** ...
**Migration:** ...
**Proof:** ...
```

## Quality requirements

- Every opportunity references specific code and verified capability.
- No recommendation based solely on "newer" or "more idiomatic."
- No percentage-of-full-library metric; measure relevant capability coverage.
- No pass-through wrapper preserved without a concrete reason.
- No architecture pivot hidden as a local refactor.
- No performance recommendation without a representative workload or a
  planned benchmark.
- Say when the library is already well leveraged.
