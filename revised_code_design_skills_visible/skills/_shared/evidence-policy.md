# Evidence and Claim Policy

Read this reference whenever a skill makes repository-wide, API-level,
architectural, decommission, or completion claims.

## 1. Label the epistemic status of important statements

Keep these categories distinct:

- **Observed fact** — directly supported by code, configuration, test output,
  version metadata, official documentation, or a complete structural query.
- **Inference** — a conclusion drawn from observed facts. State the inference
  and the facts that support it.
- **Decision** — a chosen design or execution approach. Record alternatives and
  why the decision was selected.
- **Assumption** — not yet proven. Give it an owner, validation method, and the
  consequence if false.

Do not turn an assumption into plan text that reads like a fact.

## 2. Match evidence strength to the claim

| Claim | Minimum evidence |
|---|---|
| A file or symbol exists | Current-tree file/symbol lookup |
| A signature or contract has a shape | Definition read, type query, or compiler-visible interface |
| All callers or implementations are covered | Relational query over the plausible consumer universe with complete coverage |
| A legacy pattern is absent | Complete structural/rule query or an appropriate repository-wide zero-hit fallback |
| A library API is available | Pinned-version official docs plus local import/compile probe when material |
| A behavior is correct | A test or reproducible oracle that distinguishes pass from fail |
| A performance claim holds | Representative benchmark with recorded environment and baseline |
| A plan packet is complete | All declared acceptance evidence and local gates are satisfied |
| The implementation is complete | All packets, milestones, decommission obligations, and final gates are proved |

Text search is useful evidence for literal strings and a fallback for
structural-tool coverage failures. It is not, by itself, proof of caller,
implementation, inheritance, or semantic completeness.

## 3. Coverage is part of the result

An empty result is not evidence of absence unless the query's coverage envelope
is complete for the claim being made.

Before relying on a negative or global result, inspect:

- selected files/modules;
- unprocessed or failed files;
- language/configuration coverage;
- unresolved symbols;
- graph evidence state;
- parser/index diagnostics;
- whether tests, generated code, tooling, and cross-language consumers were in
  the intended universe.

Qualify partial evidence precisely. Do not convert `partial_current` or an
incomplete parse into a global zero-state claim.

## 4. Baseline and staleness

Every design and plan records a baseline commit or an explicit uncommitted
working-tree identity. Before execution or review:

1. Compare the current tree with the baseline over the design/plan change
   surface.
2. Record drift that invalidates file, symbol, dependency, or API assumptions.
3. Treat the current repository and verified external API as higher authority
   than illustrative plan detail.
4. Preserve the immutable plan; record execution-time corrections in state.

## 5. Library evidence hierarchy

Use this order:

1. Local manifest, lockfile, and installed/compiled version.
2. Local project reference documentation for that exact version.
3. Official versioned documentation, release notes, and source repository.
4. A minimal executable, import, type, or compilation probe.
5. Secondary material only for orientation, never as the sole API authority.

Record when the runtime version differs from the manifest or lockfile.

## 6. Evidence ledger

For design, audit, and review work, maintain a compact ledger:

| ID | Claim | Status | Evidence | Coverage/limits | Used by |
|---|---|---|---|---|---|
| E-01 | ... | observed/inferred/assumed | path, symbol, command, doc | ... | D-03, WP02 |

Do not paste raw search output into the artifact. Preserve only the smallest
evidence needed to reconstruct the decision.

## 7. Prohibitions

- Do not invent file paths, symbols, commands, library APIs, or test results.
- Do not claim a check passed if it was not run.
- Do not hide a failed check by weakening the gate without a recorded decision.
- Do not claim a decommission is complete while compatibility aliases, imports,
  dual writes, registrations, configuration, or tests still preserve the old
  authority.
- Do not use plan conformity as a substitute for behavioral correctness.
