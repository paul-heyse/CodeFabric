# Artifact Schemas

These schemas define the contract among design, planning, execution, status,
audit, and review skills. Preserve stable identifiers and version artifacts
rather than rewriting history.

Three rules govern every schema below (`evidence-policy.md` §0):

- **Artifacts record judgment.** Decisions, rationale, deviations, failed
  approaches, assumptions, and blockers belong in artifacts. Facts a command
  can recompute — digests, changed-file lists, check outcomes — do not; §8
  defines how they are derived.
- **One ID rule.** Mint a stable ID (`D-`, `I-`, `L-`, `LD-`, `WP`, `M`,
  `DB`, `F-`, `IR-`) only when another artifact or section references the
  item. There are no mandatory ID registries; an item nothing references
  needs no ID. Once minted, an ID never changes and is never reused.
- **Acceptance evidence is executable.** Wherever a schema asks for evidence,
  proof, revalidation, or a re-test, it means a named executable check — a
  test name, a `rules/` rule id, a `just` recipe, or a probe command — not
  prose.

## 1. Design dossier

Path:

```text
docs/designs/<topic>_design_vN_<YYYY-MM-DD>.md
```

Required frontmatter:

```yaml
---
artifact: design-dossier
design_id: <topic>
version: vN
date: YYYY-MM-DD
status: draft|accepted|superseded
baseline_commit: <git-ref>
primary_scope:
  - <paths-or-modules>
doctrine_path: docs/library_ref/semantic_design_principles_holistic.md
---
```

When the baseline tree was dirty, record the tree identity in a separate
`working_tree_digest` key. (A `+working-tree-<digest>` suffix inside
`baseline_commit` appears in older dossiers and is tolerated there; do not
produce it.)

Core body — six sections, each carrying a decision:

1. Executive decision
2. Constraints and target invariants
3. Target architecture — contracts, ownership, flows, and library/platform
   decisions
4. Alternatives and clean-sheet challenge
5. Transition, cutover, and legacy disposition
6. Proof strategy — the oracles and checks that will prove the target

Everything else — current-state evidence, failure/security/performance
analysis, risk registers, observability — is optional: add a section only
when it carries a decision or a load-bearing constraint. The dossier ends
with a single acceptance line: `accepted`, `accepted-with-named-assumptions`
(each assumption labeled per `evidence-policy.md` §1), or `not-ready` with
the blocking items.

### The LD block

The single definition of a library decision record, referenced by the
design-development and library-capability-research skills:

```markdown
### LD-NN — <library> <decision>

**Decision:** adopt | wrap | upgrade | build | reject | retain-current
**Version basis:** <exact pinned version / feature set the evidence used>
**Displaces:** <custom code displaced or retained, with paths>
**Risk:** <key risk and its mitigation>
**Validation:** <executable check that must pass before or during execution>
```

## 2. Implementation plan

Path:

```text
docs/plans/<topic>_implementation_plan_vN_<YYYY-MM-DD>.md
```

Required frontmatter (the validator checks required keys only; additional
keys are permitted):

```yaml
---
artifact: implementation-plan
plan_id: <topic>
version: vN
date: YYYY-MM-DD
status: draft|audited|approved|superseded
design_path: <path>
design_version: vN
baseline_commit: <git-ref>
state_path: docs/plans/state/<plan-slug>_state.json
cutover: true|false
---
```

Body:

1. Outcome and non-goals
2. Source design and declared inputs — one `path | sha256` table listing the
   documents the plan depends on, computed once at planning time and
   thereafter recomputed only by tooling (§8); never hand-updated
3. Global target invariants
4. Work packets (`WP01`, `WP02`, ...)
5. Integration milestones (`M01`, ...)
6. Cross-packet decommission batches (`DB01`, ...)
7. Final gate matrix — a list of `just` recipes; if a needed gate has no
   recipe, the plan proposes adding one rather than embedding raw flags
8. Execution sequence
9. Plan risks and replan policy

Each work packet contains:

- outcome;
- dependencies;
- target invariants and design references;
- **preflight query** — the exact commands the executor runs before editing
  to discover the change surface, plus optionally the known-touch files
  verified in the planning session. No frozen must-touch/likely-touch
  manifests: the plan carries the question, the executor derives the answer
  at execution time;
- required changes and legacy disposition;
- **acceptance checks** — named executable checks (behavioral, structural,
  negative, operational as relevant). A packet is **complete** when these
  checks pass at its proving commit and at HEAD, and at no other time;
- edit-local and packet-local gates;
- integration milestone;
- replan triggers and rollback/recovery.

Forbidden in the plan body: digest tables beyond the declared-inputs table,
hand-built traceability matrices, restamp procedures, and any other record a
command in §8 derives. (Traceability generated from the `contracts/`
registries is future tooling work, not a hand-maintained table.)

## 3. Execution state

Path:

```text
docs/plans/state/<plan-slug>_state.json
```

Schema version 2 — judgment fields only:

```json
{
  "schema_version": 2,
  "plan_path": "...",
  "design_path": "...",
  "baseline_commit": "...",
  "status": "not_started",
  "current_packet": null,
  "packets": {
    "WP01": {
      "status": "not_started",
      "proving_commit": null,
      "deviations": [],
      "failed_approaches": [],
      "blockers": []
    }
  },
  "milestones": {},
  "decommission_batches": {},
  "baseline_failures": [],
  "discovered_obligations": [],
  "plan_deviations": [],
  "next_action": null,
  "updated_at": "..."
}
```

Packet statuses:

```text
not_started | ready | in_progress | blocked | complete | stale | invalidated
```

Rules:

- **`proving_commit` is mandatory for `complete`.** Commit the packet's work
  and record the commit before setting the status; a packet without one stays
  `in_progress` with a blocker naming the gap. There is no working-tree
  fallback.
- **Derived facts are never stored.** Changed files, check outcomes,
  acceptance evidence, digests, and pre-existing-modification records are
  recomputed on demand (§8). A state file that stores them is malformed.
- A previously `complete` packet remains trusted only while its proving
  commit is in current history and its named acceptance checks still pass —
  both derivable, neither taken on faith.
- `schema_version: 1` files are read-only history. A skill that needs to
  write one migrates it to version 2 first: drop the derivable fields, keep
  the judgment fields.

## 4. Audit findings

Use stable IDs `F-001`, `F-002`, ... across one audit version.

Each finding contains:

```markdown
### F-001 — <title>

**Severity:** blocker | major | minor | observation
**Category:** factuality | design | library | impact | legacy | proof |
sequence | doctrine | operations | context-efficiency
**Scope:** design D-03; plan WP04
**Finding:** <claim and its evidence, per evidence-policy.md §1>
**Required resolution:** ...
**Revalidation:** <an executable command whose success closes the finding>
```

`Revalidation:` MUST be an executable command — a test, recipe, `ast-grep`/
`rg` query, or probe — never prose. Integration runs it; a follow-up audit
re-runs it.

An audit verdict is:

```text
ready | ready-with-corrections | needs-revision | needs-redesign
```

## 5. Audit integration disposition

Every finding receives exactly one disposition:

```text
applied-plan | applied-design | added-packet | added-decommission |
covered-by:<finding> | deferred | rejected | requires-redesign
```

The integration log records the exact edit, rationale, and the revalidation
command's outcome.

## 6. Implementation review findings

Use stable IDs `IR-001`, `IR-002`, ...

Each finding contains: severity, dimension, evidence, remediation, and a
focused re-test that is an executable command.

Every row of the review's outcome/invariant matrix names the executable
oracle (test id, `rules/` rule id, `just` recipe, or probe) that proves it.
A row with no oracle is itself an `IR-` finding — a test gap — never a prose
attestation that the invariant holds.

Verdict:

```text
approved | approved-with-minor-findings | changes-required | design-invalidated
```

## 7. Review-artifact frontmatter

All reports under `docs/reviews/` share the common required keys and add the
per-type keys below. Unknown `artifact` values are schema violations — a new
report type gets a row here before first use.

Common required keys: `artifact`, `date`, `version` (`vN`), and
`status: draft|complete|superseded`.

| `artifact` | Additional required keys |
|---|---|
| `plan-audit` | `plan_path`; `verdict: ready\|ready-with-corrections\|needs-revision\|needs-redesign` |
| `implementation-review` | `plan_path`; `verdict: approved\|approved-with-minor-findings\|changes-required\|design-invalidated` |
| `implementation-status` | `plan_path`, `state_path` |
| `library-capability-research` | `topic` |
| `lib-leverage` | `library` |
| `skill-eval` | — |

## 8. Validation and derivation

This section owns the enforcement contract for everything above. Skills cite
it as `artifact-schemas.md §8` rather than restating commands.

**Validated** (structure that must conform):

- frontmatter presence, required keys, and status/verdict vocabularies
  (§§1, 2, 3, 7);
- ID uniqueness within an artifact;
- resolvability of every `*_path` frontmatter reference;
- recomputation of any declared digest — frontmatter `X_digest`/`X_path`
  pairs and declared-inputs table rows;
- state-file conformance to the schema-version-2 shape, including the
  absence of derived-fact fields.

**Derived** (facts recomputed from ground truth, never stored or
hand-compared):

| Fact | Command |
|---|---|
| Input freshness | `shasum -a 256 <path>` vs. the declared-inputs table |
| Drift since baseline | `git diff --stat <baseline_commit>..HEAD -- <scope>` |
| Packet changed files | `git show --stat --format= <proving_commit>` |
| Packet trust | `git cat-file -e <proving_commit>` and `git merge-base --is-ancestor <proving_commit> HEAD` |
| Check outcomes | re-run the named check; record recipe name + exit status |

**Absent on purpose — add when phase 2 lands.** Two recipes consolidate this
section: `just artifacts-check` (the validations) and `just plan-status`
(the derivations). They are planned, not yet implemented. Until they land,
run the commands above directly; never hand-transcribe their outputs into an
artifact. When they land, this paragraph is replaced by their names and the
skills need no edits.

Artifacts predating the 2026-08-20 schema revision are grandfathered: they
are validated only to the extent their vintage permits (the burn-down list
lives with the phase-2 validator). Never write a new artifact that needs
grandfathering.
