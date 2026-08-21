---
name: plan-audit
description: Independently audit an implementation plan and its source design for factual accuracy, target-design quality, library grounding, impact completeness, legacy disposition, proof quality, doctrine alignment, and executability.
when_to_use: Use before execution, after material repository drift, or whenever a plan may preserve legacy architecture, over-specify implementation detail, or depend on uncertain library capabilities.
argument-hint: "[plan-path] [--focus categories]"
allowed-tools: Read, Glob, Grep, Bash, Write, Agent, WebSearch, WebFetch
disallowed-tools: Edit
user-invocable: true
context: fork
background: false
agent: general-purpose
---

# Design and Plan Audit

Review the plan adversarially but constructively. Judge whether the chosen
target and execution strategy are correct and provable, not whether the plan
merely follows its template.

The repository is read-only except for writing a new audit report.

Read:

- `../_shared/evidence-policy.md`
- `../_shared/code-intelligence.md`
- `../_shared/doctrine-policy.md`
- `../_shared/validation-policy.md`
- the audit-finding section of `../_shared/artifact-schemas.md`

Infer the source design and state paths from plan frontmatter.

## Output and versioning

Write:

```text
docs/reviews/plan_audit_<plan-slug>_<YYYY-MM-DD>_vN.md
```

Frontmatter per `artifact-schemas.md` §7 (`artifact: plan-audit`,
`plan_path`, `verdict`, `version`, `date`, `status`).

Never overwrite, rename, or edit a prior audit. Reference earlier audits and
state what the new audit re-evaluates.

Use stable finding IDs `F-001`, `F-002`, ... within this audit.

Verdict:

```text
ready | ready-with-corrections | needs-revision | needs-redesign
```

## Audit stance

- Evidence outranks plan assertions.
- Current code and verified pinned APIs outrank illustrative plan detail.
- Do not reopen design for stylistic preference.
- Do reopen it when codebase evidence, library capability, target invariants,
  doctrine, migration economics, security, reliability, performance, or
  operational behavior materially invalidate the decision.
- Respect YAGNI, but do not use YAGNI to excuse a broken consumer, missing
  cutover, unproved failure path, or permanent legacy duplication.
- Grade depth by risk and scope.

## Procedure

### Phase 1 — Ingest artifacts and provenance

Read the full plan and source design once. Extract:

- baseline and declared inputs (freshness derived per
  `artifact-schemas.md` §8);
- outcomes, non-goals, design decisions, invariants, and library decisions;
- work packets, dependencies, milestones, and decommission batches;
- preflight queries and known-touch claims;
- local, packet, milestone, and final gates;
- risks, assumptions, replan triggers, and rollback;
- prior audit and integration provenance.

Run the §8 validation on the plan, design, and state, and convert failures
into findings. Do not hand-validate structure it already checks.

### Phase 2 — Establish current truth and staleness

Derive drift from the recorded baseline over the relevant scope per
`artifact-schemas.md` §8. Then verify load-bearing:

- files, symbols, contracts, import/export paths, and configuration;
- callers, implementations, subclasses, constructors, serialization, and
  persistence sites;
- current library versions/features and APIs;
- tests, build commands, CI gates, and baseline failures;
- legacy targets and alternative paths.

Inspect structural-query coverage before global or negative conclusions.
Record stale assumptions as findings, not silent corrections.

### Phase 3 — Audit the target design

Evaluate:

1. **Outcome fit** — Does the target solve the stated problem without expanding
   into unrelated objectives?
2. **Clean-sheet quality** — Would this be preferred absent the current
   implementation? Is any retained legacy shape justified by a genuine
   constraint?
3. **Responsibilities and dependency direction** — Are ownership, sources of
   truth, contracts, data/control/mutation flows, and resource lifecycles
   coherent?
4. **Failure and operational design** — Are failure semantics, recovery,
   observability, provenance, security, performance, and deployment concerns
   sufficient for the risk?
5. **Alternatives** — Were materially simpler or more library-native designs
   omitted?
6. **Test oracle** — Could an implementation be proven correct, or would review
   rely on plausibility?
7. **Doctrine** — Does the target advance or maintain relevant principles and
   avoid anti-principles?

Apply the explicit challenge:

> Would this still be the preferred architecture if the existing
> implementation did not exist?

A negative answer without a documented external constraint is a major or
blocker finding.

### Phase 4 — Audit library and platform decisions

For every load-bearing `LD-*` decision:

- confirm manifest/lockfile and resolved version;
- verify official API and feature availability;
- inspect feature flags, platform limits, maturity, and upgrade requirements;
- assess whether the wrapper/custom code is justified;
- identify current custom code that the selected capability should displace;
- check lifecycle, failure, observability, performance, and interoperability;
- consider language-native, existing-dependency, upgrade, new-dependency, and
  small-custom alternatives.

Run a minimal probe when a material API or behavior remains uncertain.
Fabricated or version-incompatible APIs are blockers.

### Phase 5 — Audit work-packet executability

Structural conformance is already covered by the §8 validation; judge each
packet on five dimensions:

1. **Outcome and closure** — the outcome is observable, dependencies are
   complete and acyclic, and the packet is dependency-closed or has an
   explicit milestone boundary.
2. **Proof strength** — the named acceptance checks are executable and
   actually *prove* the outcome across behavior, structure, negative state,
   and operations as applicable. A check that merely runs code without
   distinguishing pass from fail is a finding.
3. **Discovery sufficiency** — the preflight query would discover the real
   change surface; design/invariant/library references are correct.
4. **Consumer coverage** — immediate consumers and cross-language boundaries
   are covered by the packet or an explicit milestone.
5. **Proportionality** — required changes avoid speculative implementation
   detail; gates are proportional; replan triggers distinguish adaptation,
   plan revision, and design reopening; rollback is adequate; packet size is
   neither a one-file microtask nor an untestable omnibus.

Flag plans that spend context predicting helper bodies while omitting contracts,
proof, or impact discovery.

### Phase 6 — Audit legacy and transition

For every `L-*` disposition and decommission target:

- disposition is explicit and justified;
- all old consumers, registrations, exports, data paths, aliases, tests, and
  documentation are accounted for;
- temporary encapsulation is bounded;
- dual reads/writes/ownership have an exit invariant;
- atomic versus incremental cutover is intentionally chosen;
- decommission prerequisites are safe and complete;
- positive target evidence and negative legacy zero-state evidence exist;
- executable governance prevents reintroduction when warranted.

A plan that leaves permanent duplicate authority or compatibility ambiguity is
not ready.

### Phase 7 — Audit proof and validation

Check that validation is layered rather than entirely deferred:

- edit-local checks catch cheap errors;
- every packet has focused proof;
- milestones test first interactions;
- final gates are discovered from all affected toolchains;
- baseline failures are measured;
- failures are treated as feedback;
- subagents are allowed and required to prove their packets;
- broad final checks do not replace targeted behavioral or negative proof.

Ensure tests prove semantics rather than only importing or exercising lines.
Look for missing property, differential, contract, migration, concurrency,
recovery, performance, or failure-injection tests where the design requires
them.

### Phase 8 — Audit sequence and parallelism

Verify:

- foundation and contracts precede consumers;
- packets that share contracts/files are not falsely parallelized;
- parallel writers require disjoint files or worktree isolation;
- milestones occur before risk compounds;
- decommission occurs as soon as safe, not only at the end;
- independent work can continue when a packet blocks;
- execution state and resume behavior are defined.

### Phase 9 — Fresh challenger

For high-risk plans, delegate one fresh-context design challenger and, when
useful, one focused impact or library specialist. Give them artifacts and
bounded questions, not the lead auditor's conclusions.

The lead adjudicates and records only evidence-backed findings.

### Phase 10 — Synthesize findings

Use this finding format:

```markdown
### F-001 — <title>

**Severity:** blocker | major | minor | observation
**Category:** factuality | design | library | impact | legacy | proof |
sequence | doctrine | operations | context-efficiency
**Scope:** <design IDs and/or packet IDs>
**Finding:** <one falsifiable claim and its evidence — specific code,
command, artifact section, or official API>
**Required resolution:** <exact artifact change or re-design>
**Revalidation:** <an executable command whose success closes the finding>
```

**`Revalidation:` must be an executable command** — a test, recipe,
`ast-grep`/`rg` query, or probe — never prose. Integration runs it verbatim.

Do not create multiple findings for symptoms of one root defect. Do not bury
execution blockers in prose.

### Phase 11 — Write the report

Use:

```markdown
# Plan Audit: <title>

## Provenance and Scope
## Executive Summary
## Readiness Verdict
## Finding Index
## Findings
## Target-Design Assessment
## Library Capability Assessment
## Work-Packet and Impact Assessment
## Legacy, Transition, and Decommission Assessment
## Proof and Validation Assessment
## Doctrine and Anti-Principle Assessment
## Top Required Changes
## Re-Audit Scope
```

The finding index lists ID, severity, category, scope, and disposition status
(initially `open`).

## Severity and verdict rules

- **Blocker:** execution would likely fail, violate a target invariant, use an
  unavailable API, corrupt/lose authority, or cannot be proven safe.
- **Major:** plan may execute but risks substantial incorrectness, permanent
  legacy duplication, architectural regression, or missing proof.
- **Minor:** localized ambiguity or improvement that should be corrected but
  does not invalidate the plan.
- **Observation:** evidence or optional follow-up, not required for readiness.

Verdict:

- `ready`: no open blockers or majors.
- `ready-with-corrections`: no blockers; majors are narrow and mechanically
  correctable.
- `needs-revision`: packet, impact, transition, or proof structure requires
  material plan change.
- `needs-redesign`: target architecture, library decision, or invariant is
  materially invalidated.

## Quality requirements

- Every finding has evidence and an executable closure test.
- Every global/negative claim accounts for coverage.
- Every library finding is version-grounded.
- Recommendations modify a named design/plan section or add a bounded packet.
- The audit may recommend deleting or replacing legacy architecture even when
  the original plan preserved it.
- The report is concise enough that integration can disposition every finding
  without redoing the audit.
