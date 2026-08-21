---
name: implementation-review
description: Independently review the implemented code against the accepted design, plan, execution state, actual behavior, library decisions, legacy cutover, and proof evidence. Produces findings without modifying code.
when_to_use: Use after implementation or a major milestone, after fixes to prior review findings, or before declaring a plan complete.
argument-hint: "[plan-path] [--focus dimensions] [--base git-ref]"
allowed-tools: Read, Glob, Grep, Bash, Write, Agent, WebSearch, WebFetch
disallowed-tools: Edit
user-invocable: true
context: fork
background: false
agent: general-purpose
---

# Implementation Review

Review the resulting system, not the author's intentions and not merely the
plan's predicted file list. Reconstruct correctness from code, behavior,
artifacts, and evidence in a fresh context.

The repository is read-only except for a new review report.

Read:

- `../_shared/evidence-policy.md`
- `../_shared/code-intelligence.md`
- `../_shared/validation-policy.md`
- `../_shared/doctrine-policy.md`
- the implementation-review section of
  `../_shared/artifact-schemas.md`

Infer the design and state paths from plan frontmatter.

## Output

Write:

```text
docs/reviews/implementation_review_<plan-slug>_<YYYY-MM-DD>_vN.md
```

Frontmatter per `artifact-schemas.md` §7 (`artifact: implementation-review`,
`plan_path`, `verdict`, `version`, `date`, `status`).

Never overwrite an earlier review. Reference prior reviews and identify the
findings being re-reviewed.

Use stable IDs `IR-001`, `IR-002`, ...

Verdict:

```text
approved | approved-with-minor-findings | changes-required |
design-invalidated
```

## Review independence

Do not accept the executor's summary as proof. Use it to locate claimed
evidence, then verify the code and checks.

Do not require literal conformity with illustrative snippets or predicted
files. A cleaner implementation is acceptable when it satisfies the design,
invariants, behavior, and proof. Conversely, full file-manifest conformity is
not proof of correctness.

Do not modify code. Findings go to the implementation author for remediation.

## Procedure

### Phase 1 — Establish review scope and provenance

Read the accepted design, plan, state, latest audit/integration log, and
relevant prior implementation reviews.

Derive packet trust, per-packet changed files, and drift per
`artifact-schemas.md` §8 before judgment. A `complete` packet with no
proving commit in history is itself a finding, not a fact to work around.

Determine:

- baseline and final/current refs;
- actual diff and unrelated changes;
- completed packets, deviations, failed approaches, and blockers;
- residual baseline failures;
- design decisions, invariants, library decisions, and legacy dispositions.

If the state claims completion but required packets/gates are missing, record a
finding rather than silently expanding scope.

### Phase 2 — Inspect the actual change

Use the diff as an index, not the entire review universe.

Inspect:

- changed/created/deleted code and tests;
- consumers and implementation surfaces of changed contracts;
- relevant unchanged code that participates in behavior;
- public exports, configuration, registration, serialization, persistence,
  generated code, and cross-language boundaries;
- old paths and compatibility residue;
- operational and diagnostic surfaces.

Use `ast-grep` for relational and global claims, `rg` for string, config, and
cross-language residue, and state the coverage envelope for any negative claim.

### Phase 3 — Review outcome and invariant conformance

For each design outcome and target invariant:

- identify the code and behavior that implement it;
- name the executable oracle that proves it — a test id, `rules/` rule id,
  `just` recipe, or probe command;
- verify failure and edge behavior;
- verify no contradictory path remains;
- record unexplained deviations.

These rows become the report's Outcome and Invariant Matrix. **A row with no
executable oracle is itself an `IR-` finding (dimension: tests) — a test
gap — never a prose attestation that the invariant holds.**

The implementation is defective when the plan's mechanics landed but the
design outcome did not.

### Phase 4 — Review correctness

Assess as applicable:

- input and state invariants;
- boundary parsing/validation and illegal states;
- error taxonomy and propagation;
- transaction and mutation semantics;
- concurrency, ordering, cancellation, retry, and idempotency;
- resource ownership, cleanup, and leaks;
- cache invalidation and incremental behavior;
- persistence, migration, compatibility, and rollback;
- cross-language type and memory boundaries;
- determinism and reproducibility;
- security, authorization, trust, and data exposure;
- behavior under invalid, partial, empty, large, and failure inputs.

Prefer falsifiable examples and targeted tests over stylistic commentary.

### Phase 5 — Review architecture and doctrine

Verify:

- dependency direction and module boundaries;
- single source of semantic/durable truth;
- ports/adapters and vendor isolation;
- staged compilation/artifact boundaries where relevant;
- stable identity and provenance;
- command/query and durable/temporal-state separation;
- generic runtime versus special-case paths;
- observability as structured data;
- public contract declaration/versioning;
- executable governance.

Look for accidental architecture introduced during execution:

- pass-through wrappers;
- new utility dumping grounds;
- compatibility aliases;
- duplicate models/mappings;
- deep reach-through;
- special-case branches;
- hidden side writes;
- temporal workflow state becoming domain truth.

Apply relevant doctrine and anti-principles only; avoid ceremonial scoring.

### Phase 6 — Review library leverage

For each load-bearing library decision:

- verify the implementation uses the selected pinned capability;
- assess whether use is idiomatic and lifecycle-correct;
- check feature flags, error behavior, transactions, concurrency, and resource
  management;
- look for custom code the decision intended to delete;
- look for wrappers that obscure or fight the library;
- verify upgrade/migration assumptions;
- run a focused probe when behavior is unclear.

If the implementation proves the design's library decision was wrong, use
verdict `design-invalidated` rather than forcing a local patch.

### Phase 7 — Review legacy cutover

For every `L-*` and `DB*`:

- verify the target authority is active;
- verify old imports, exports, registrations, aliases, routes, writes, reads,
  persisted forms, config, tests, and docs are removed as required;
- inspect structural coverage for zero-state claims;
- verify governance prevents reintroduction;
- ensure temporary migration paths have not become permanent.

Any remaining duplicate authority is at least major and often blocker-level.

### Phase 8 — Review test and proof quality

Inspect tests, not only results.

Ask:

- Does the test fail under the old or defective behavior?
- Does it assert semantics rather than implementation trivia?
- Are boundary, negative, failure, and recovery cases covered?
- Are property, differential, contract, migration, concurrency, performance, or
  end-to-end tests used where the design requires them?
- Are mocks hiding the integration being claimed?
- Are flaky timing assumptions or weak assertions present?
- Do structural rules have positive and negative fixtures?
- Do final gates cover all affected languages and boundaries?
- Does every design invariant have a named oracle? A missing oracle is a
  test-gap finding.

Run focused checks to verify disputed behavior. Reuse existing gate evidence
when it is current and sufficient; do not rerun expensive suites merely for
ceremony.

### Phase 9 — Review operations and supportability

Assess:

- structured diagnostics, logs, metrics, traces, and provenance;
- actionable error messages and failure categories;
- startup/shutdown, cleanup, recovery, and degraded behavior;
- configuration and deployment;
- performance budgets and representative benchmarks;
- maintainability of generated or derived artifacts;
- operator and developer debugging surfaces.

Scale depth to the subsystem's operational risk.

### Phase 10 — Independent review lenses

For large changes, delegate fresh, read-only lenses:

- correctness/failure/concurrency;
- architecture/doctrine/legacy;
- library/API/performance;
- tests/security/operations.

Avoid overlapping generic reviewers. The lead de-duplicates findings and owns
severity.

### Phase 11 — Findings and verdict

Use:

```markdown
### IR-001 — <title>

**Severity:** blocker | major | minor | observation
**Dimension:** outcome | correctness | architecture | library | legacy |
tests | security | performance | operations | diff-hygiene
**Design/Plan refs:** <IDs>
**Evidence:** <code, command, behavior>
**Failure mode:** <what goes wrong>
**Remediation:** <concrete change, not a full patch>
**Focused re-test:** <an executable command whose success proves closure>
```

Severity:

- **Blocker:** data/correctness/security failure, target invariant violation,
  unavailable required capability, unsafe migration, or incomplete authority
  cutover.
- **Major:** substantial defect, architecture regression, missing important
  proof, or operational risk.
- **Minor:** localized maintainability, clarity, or low-risk proof issue.
- **Observation:** optional follow-up or evidence note.

Verdict:

- `approved`: no open findings requiring code change.
- `approved-with-minor-findings`: only minor/observational follow-up.
- `changes-required`: any blocker/major, or cumulative minors that undermine
  confidence.
- `design-invalidated`: correct remediation requires changing accepted design
  or a load-bearing library decision.

### Phase 12 — Write the report

Use:

```markdown
# Implementation Review: <plan>

## Provenance and Review Scope
## Executive Summary
## Verdict
## Gate and Evidence Assessment
## Finding Index
## Findings
## Outcome and Invariant Matrix
## Architecture and Doctrine Assessment
## Library Leverage Assessment
## Legacy and Decommission Assessment
## Test and Operational Assessment
## Plan Deviations and Diff Hygiene
## Required Remediation Order
## Focused Re-Review Scope
```

## Quality requirements

- Every blocker/major has reproducible evidence and a focused re-test.
- Findings target root causes and are de-duplicated.
- Style preferences are not elevated to correctness findings.
- The reviewer may praise or accept a beneficial plan deviation.
- No claim of global absence without complete evidence.
- No code edits, auto-fixes, or hidden plan changes.
- A follow-up review re-tests unresolved IDs rather than starting from the
  author's assertion that they were fixed.
