---
name: skill-eval
description: Evaluate one or more coding-agent skills with fresh-session comparative trials, measuring invocation quality, artifact correctness, context efficiency, implementation outcomes, and review-detected defects.
when_to_use: Use after revising skills, before replacing a team workflow, or when plans are verbose, stale, under-diligenced, poorly executed, or inconsistently invoked.
argument-hint: "[skill-or-suite] [--cases path] [--mode invocation|artifact|execution|full]"
allowed-tools: Read, Glob, Grep, Bash, Write, Agent
disable-model-invocation: true
user-invocable: true
---

# Skill Evaluation

Treat skill design as an empirical engineering problem. Compare realistic fresh
runs, not the apparent quality of the prose.

Read:

- `../_shared/evidence-policy.md`
- `../_shared/artifact-schemas.md`

## Output

Write a versioned report:

```text
docs/reviews/skill_eval_<suite-or-skill>_<YYYY-MM-DD>_vN.md
```

Place reusable evaluation cases under:

```text
docs/agent_evals/<suite-or-skill>/cases/
```

(The directory is created on first use; it is absent until an evaluation has
produced cases.)

Do not modify production code merely to make an evaluation pass. Use a fixture,
temporary branch, or isolated worktree for implementation trials.

## Evaluation dimensions

Evaluate separately:

1. **Invocation precision**
   - triggers on appropriate prompts;
   - does not trigger on adjacent/simple prompts;
   - argument handling is correct;
   - manual-only skills are not invoked automatically.

2. **Artifact quality**
   - factual grounding and evidence coverage;
   - target design quality;
   - library API/version correctness;
   - legacy disposition;
   - packet dependency closure;
   - proof and replan quality;
   - context-efficient level of detail;
   - schema and provenance compliance.

3. **Execution quality**
   - first-pass packet coherence;
   - edit-local and packet-local feedback use;
   - defects caught before final gates;
   - plan deviations handled correctly;
   - legacy removal completeness;
   - resume from state;
   - independent review verdict and finding severity.

4. **Efficiency**
   - input/output tokens where available;
   - tool calls and repeated reads;
   - raw evidence dumped into main context;
   - plan word/line count;
   - irrelevant predicted detail;
   - number of re-plans or implementation reversals.

5. **Reliability**
   - consistency across repeated fresh runs;
   - sensitivity to repository drift;
   - handling of partial structural evidence;
   - behavior when tests fail;
   - handling of missing docs or unavailable library APIs.

## Case design

Build a representative corpus, not one showcase prompt.

Include as applicable:

1. bounded bug fix with no architecture change;
2. local additive feature;
3. cross-module contract change;
4. Python/Rust or native boundary change;
5. hard architectural replacement;
6. library-native capability replacing custom code;
7. data/schema/protocol migration;
8. decommission requiring global zero-state proof;
9. plan seeded with a stale API or missing consumer;
10. partially implemented plan resumed after repository drift;
11. plan with unnecessary legacy accommodation;
12. simple request where the heavy skill should not trigger.

Each case defines:

- repository/ref or fixture;
- prompt and supplied artifacts;
- expected skill invocation;
- must-discover facts;
- target invariants;
- known traps;
- acceptable alternatives;
- objective checks;
- human/reviewer rubric.

Avoid evaluation cases whose "correct" answer depends only on matching one
preferred implementation.

## Comparative protocol

For each case, use fresh contexts. Leftover authoring context invalidates the
comparison.

Run as applicable:

- baseline without the skill;
- current skill;
- revised skill;
- optional ablation with a major instruction removed.

Keep model, repository ref, permissions, and tool availability constant.

For manual skills, invoke explicitly. For invocation tests, expose the skill and
observe whether it is selected without naming it.

Use isolated worktrees for writing/execution trials. Do not let one run's edits,
state, or memory contaminate another.

Repeat high-variance cases enough to distinguish a pattern from a single run.

## Objective checks

Run the checks that already exist before inventing any:

- artifact schema, frontmatter, ID, and path validation per
  `artifact-schemas.md` §8;
- declared gates: run the `just` recipes the artifact names, recording
  recipe name + exit status;
- plan freshness and packet trust, where a state file was produced: the
  derivations in `artifact-schemas.md` §8;
- compilation/tests via the repository's gate recipes.

Bespoke checks with no automation yet (run by hand, scripted per evaluation
when worth it):

- invented/nonexistent file or symbol references (`rg`/`ast-grep` over the
  artifact's citations);
- dependency-cycle detection across packet dependencies;
- pinned library API probes;
- missing consumer/implementation sites;
- global claims made on partial evidence;
- legacy zero-state;
- diff size and unrelated churn;
- artifact word/line count.

Keep deterministic checks separate from qualitative reviewer scoring.

## Reviewer rubric

Use a fresh reviewer blind to which variant produced the artifact when
possible.

Score 1–5:

- problem framing;
- current-state accuracy;
- target architecture;
- clean-sheet/legacy judgment;
- library leverage;
- migration and cutover;
- proof strategy;
- plan executability;
- context economy;
- provenance and resumability;
- implementation correctness;
- reviewability.

Require evidence for low and high scores.

## Metrics

Recommended metrics include:

- false-positive/false-negative invocation rate;
- unsupported API claims;
- missing impact sites;
- percentage of legacy targets actually removed;
- packet first-pass gate success;
- defects found only at final review;
- implementation-time design reversals;
- unexplained plan deviations;
- resume success without full re-analysis;
- review blocker/major count;
- plan tokens or words per work packet;
- detail later found irrelevant;
- total tool calls and duplicate reads;
- wall time and token use when available.

Do not optimize token count at the expense of correctness. Track value per token
and defect escape rate.

## Procedure

1. Read the target skill(s) and supporting references.
2. Select or create a balanced case set.
3. Freeze repository refs, environment, and scoring rubric.
4. Run fresh comparative trials.
5. Apply deterministic checks.
6. Run blinded or fresh-context qualitative review.
7. Analyze failures by root cause:
   - missing instruction;
   - conflicting instruction;
   - excessive instruction/context dilution;
   - bad trigger description;
   - unavailable tool;
   - poor artifact schema;
   - model variance;
   - repository-specific assumption.
8. Recommend the smallest skill/reference/hook change that addresses the
   failure.
9. Re-run affected cases and check for regressions on simple cases.
10. Write the report and preserve raw run references outside the skill body.

## Report structure

```markdown
# Skill Evaluation: <suite>

## Evaluation Configuration
## Case Matrix
## Executive Summary
## Invocation Results
## Artifact Quality Results
## Execution and Review Results
## Efficiency and Reliability Results
## Per-Case Findings
## Root-Cause Analysis
## Recommended Skill Changes
## Regression Risks
## Re-Test Results
## Adoption Verdict
```

Adoption verdict:

```text
adopt | adopt-with-corrections | continue-trial | reject
```

## Quality requirements

- Fresh sessions for every comparison.
- Same environment across variants.
- Objective checks and qualitative judgments are not conflated.
- No conclusion from one hand-picked case.
- No skill change accepted without re-testing affected and adjacent cases.
- Preserve failed approaches and surprising results; they are the highest-value
  evidence for future skill revisions.
