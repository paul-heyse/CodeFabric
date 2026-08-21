# Evidence and Claim Policy

Read this reference whenever a skill makes repository-wide, API-level,
architectural, decommission, or completion claims.

## 0. Governing principle

**Executable beats derived beats recorded.**

- A claim that can be a check — a test, a `rules/` governance rule, a `just`
  gate, an executable probe — must become one, and is thereafter cited by
  name rather than re-established by hand.
- A fact that can be recomputed from ground truth — git history, file
  digests, the justfile, the generated `contracts/` tree — is derived on
  demand and never hand-transcribed. The derivation contract lives in
  `artifact-schemas.md` §8.
- Hand-written artifacts record only what no tool can produce: decisions,
  rationale, deviations, failed approaches, assumptions, and open blockers.

This section is the single normative home of the principle. Other policies
and skills cite `evidence-policy.md §0` instead of restating it.

## 1. Label the epistemic status of important statements

Keep these categories distinct:

- **Observed fact** — directly supported by code, configuration, test output,
  version metadata, official documentation, or a structural query whose
  coverage envelope was stated and sufficient for the claim.
- **Inference** — a conclusion drawn from observed facts. State the inference
  and the facts that support it.
- **Decision** — a chosen design or execution approach. Record alternatives and
  why the decision was selected.
- **Assumption** — not yet proven. Give it an owner, validation method, and the
  consequence if false.

Do not turn an assumption into plan text that reads like a fact.

## 2. Match evidence strength to the claim

The strongest evidence is proof by construction: promote the claim to an
executable check and cite that check by name (§0). The table below governs
what remains — claims that cannot themselves be a check, or that a check has
not yet been written for.

| Claim | Minimum evidence |
|---|---|
| A file or symbol exists | Current-tree file/symbol lookup |
| A signature or contract has a shape | Definition read, or the compiler-visible interface |
| All callers or implementations are covered | Tier 1 where the language permits — change the symbol and rebuild clean. Otherwise `ast-grep` over the plausible consumer universe plus a stated coverage envelope, recorded as inference |
| A legacy pattern is absent | Zero-hit `ast-grep` rule *and* zero-hit `rg` over a declared scope, plus a green build. One tool alone is not a zero-state proof |
| A library API is available | Pinned-version official docs plus local import/compile probe when material |
| A behavior is correct | A test or reproducible oracle that distinguishes pass from fail |
| A performance claim holds | Representative benchmark with recorded environment and baseline |
| A plan packet is complete | Its named acceptance checks pass at the proving commit and at HEAD; commit ancestry is derived per `artifact-schemas.md` §8 |
| The implementation is complete | All packets, milestones, decommission obligations, and final gates are proved |

Match the instrument to the claim, per the ladder in `code-intelligence.md`.
Text search is not a lesser tool here — it is the only one that sees string
keys, configuration, comments, documentation, and cross-language consumers,
and a decommission claim is incomplete without it. It is simply not, by
itself, proof of caller, implementation, inheritance, or semantic
completeness; a structural query is, within its stated coverage, and the
compiler is, unconditionally, for the languages that have one.

## 3. Coverage is part of the result

An empty result is not evidence of absence unless the query's coverage envelope
is complete for the claim being made.

Before relying on a negative or global result, inspect:

- the candidate set actually searched (`rg --files` with the same filters);
- which files were skipped and why (`rg --debug`,
  `ast-grep --inspect summary` and its `skippedFileCount`);
- the ignore tier in force — hidden files, `.gitignore`, and whether `-u`/`-uu`
  was needed;
- the glob and type scope applied, and whether it excluded a real consumer;
- parse failures and language-mapping gaps in `ast-grep`;
- the tool and grammar version that produced a structural result — a parser
  upgrade can change node kinds, field names, and named/unnamed boundaries, so
  the version is part of the claim, not context around it;
- the residual dynamic, reflective, macro-generated, and re-exported surface
  that no static query reaches;
- whether tests, generated code, tooling, and cross-language consumers were in
  the intended universe.

Qualify partial evidence precisely. An incomplete parse or a filtered candidate
set does not become a global zero-state claim.

## 4. Baseline and staleness

Every design and plan records a baseline as a git ref (plus a working-tree
digest when the tree was dirty). Staleness is derived, never compared by
hand: input freshness and drift over the declared scope come from the
commands in `artifact-schemas.md` §8. The plan's declared-inputs table is
written once at planning time and thereafter only recomputed by tooling —
if a recorded digest no longer matches, that is a derivation result to act
on, not a table to silently re-edit.

Two rules survive any amount of drift:

1. Treat the current repository and verified external API as higher authority
   than illustrative plan detail.
2. Preserve the immutable plan; record execution-time corrections in state.

## 5. Library evidence hierarchy

Use this order:

1. Local manifest, lockfile, and installed/compiled version.
2. Local project reference documentation for that exact version.
3. Official versioned documentation, release notes, and source repository.
4. A minimal executable, import, type, or compilation probe.
5. Secondary material only for orientation, never as the sole API authority.

Record when the runtime version differs from the manifest or lockfile.

## 6. Recording evidence

Two rules replace any ledger, table, or per-claim registry:

1. **A load-bearing claim is recorded as the reproducible command that
   regenerates it, plus its coverage envelope** — never pasted output, never
   a hand-maintained table row with back-references. Anyone re-running the
   command re-derives the claim.
2. **An invariant asserted more than once is promoted** to a test, a `rules/`
   governance rule, or a `just` recipe, and thereafter cited by name — see
   "Durable rules" in `code-intelligence.md`. Re-deriving the same invariant
   by hand in a second artifact or session is a defect, not diligence.

Preserve only the smallest evidence needed to reconstruct the decision.

## 7. Prohibitions

- Do not invent file paths, symbols, commands, library APIs, or test results.
- Do not claim a check passed if it was not run.
- Do not hide a failed check by weakening the gate without a recorded decision.
- Do not hand-transcribe a fact a command derives — a digest, a changed-file
  list, a version pin; cite the deriving command instead.
- Do not claim a decommission is complete while compatibility aliases, imports,
  dual writes, registrations, configuration, or tests still preserve the old
  authority.
- Do not use plan conformity as a substitute for behavioral correctness.
