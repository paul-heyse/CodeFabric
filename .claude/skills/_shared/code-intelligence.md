# Repository Research and Structural Proof

Use this reference for repository research and for any claim about what the
code contains, who calls what, or what has reached zero.

Two tools do this work: `ast-grep` for syntax-aware structure and `rg` (ripgrep)
for text. Both are installed. Their capabilities are documented in depth at:

- `docs/library_ref/ast-grep_0.45.1_advanced_reference.md` (§0-§25)
- `docs/library_ref/ripgrep_advanced_reference_15.2.0_pcre2_10.47.md` (§0-§49)

Consult those for syntax and flag semantics rather than guessing. Section
citations below (`ast-grep §6.0`, `ripgrep §13.1`) point into them. Both
documents number chapters `# N)` and subsections `## N.M`, and nine chapter
numbers collide between them, so the alias is never optional.

The `ast-grep-ripgrep-ref` skill navigates both: which capability answers a given
search job, and where it is specified. Reach for it when the search needs more
than a literal.

Invoke it as `ast-grep`, never `sg`. The `sg` shim is deprecated in the 0.45
line, prints a warning banner that corrupts `--json` and piped output, and
collides with the Unix `setgroups` command on many systems.

The installed binary is the final authority on its own flags. The reference
tracks a fast-moving tool and is explicit that generated documentation pages
lag the implementation, so confirm an unfamiliar flag with `--help` rather than
from prose.

## Two axes

These tools serve two different jobs, and conflating them wastes effort:

- **Navigation** — cheaply finding out what exists and where, without reading
  whole files. Optimise for token cost.
- **Proof** — establishing that a claim about the code is true. Optimise for
  coverage and certainty.

Navigation is `outline` first. Proof is the instrument ladder.

## Instrument ladder (proof)

Three tiers. Each is stronger as *proof* and narrower in *reach* than the one
below it.

| Tier | Instrument | What it proves | Reach |
|---|---|---|---|
| 1 | Compiler / type checker — `cargo check`, `uv run ruff`, later Pyrefly | Caller coverage **by construction**: remove or rename the symbol, rebuild clean, and absence is proved | Rust strong; Python partial |
| 2 | `ast-grep` | Structural facts: call sites, impls, subclasses, decorators, signatures, member access | Any tree-sitter language, parsed per file |
| 3 | `rg` | Literal presence and absence: strings, comments, config, docs, cross-language, dynamic and generated references | Everything, including what has no AST |

**Escalate down for breadth, up for proof.** A tier-3 zero-hit is not caller
completeness. A tier-1 clean rebuild is. Label every claim with the highest
tier that confirmed it.

Tier 3 stays mandatory even when tier 1 passes: string-keyed registries,
serialized names, configuration, and cross-language consumers survive
compilation untouched.

A live example from this repository. Asking whether `codefabric.main` is dead
code:

```bash
ast-grep run -l python -p 'main($$$A)' src/     # zero call sites
rg -n -w 'main' --type-add 'cfg:*.toml' -t cfg -t py .
#   src/codefabric/__init__.py:1:def main() -> None:
#   pyproject.toml:18:codefabric = "codefabric:main"
```

Tier 2 alone says "no callers — safe to delete". Tier 3 finds the console-script
entry point, a string no AST query can see, and tier 1 confirms it is
load-bearing (`uv run codefabric` prints). Structural absence is not absence.

The tool's own guidance agrees with this shape: use ast-grep to answer *what
syntax has this structure*, and compiler/type systems to answer *what program
entity does this syntax mean* (`ast-grep §0.2`, `§24.22`).

## Navigation: search, outline, targeted read

`ast-grep outline` produces a syntax-aware table of contents without reading
file bodies. Reach for it before opening a file to find out what is in it.

```text
1. narrow candidate files (rg -l, or a path you already know)
2. outline the directory's public surface
3. narrow by symbol name or type
4. read only the ranges that matter
5. use run / YAML rules for precise structural facts
6. escalate to the compiler only when identity or types matter
```

Defaults change with input type, which is deliberate: a directory gives a cheap
public map, a file gives a richer local digest.

| Input | Default `--items` | Default `--view` |
|---|---|---|
| directory | `exports` | `names` |
| file or stdin | `structure` | `digest` |

`--items` selects the top-level class: `structure`, `exports`, `imports`,
`all`, `auto`. `--view` selects detail: `names`, `signatures`, `digest`,
`expanded`, `auto`. `--match <regex>` and `--type <symbolTypes>` filter
**top-level items only** — neither reaches members, so filter to the parent
item and then expand it. `--pub-members` keeps only public members, and treats
a member as public when the extractor has no rule to say otherwise.

Pick the smallest view that answers the question:

```bash
ast-grep outline crates --items exports              # unknown subtree -> names
ast-grep outline crates/cf-query/src/lib.rs          # known file -> digest
ast-grep outline src --items imports --view signatures
ast-grep outline src/x.rs --match '^Planner$' --type class --view expanded
```

Do not read a 2,000-line file to discover its type names.

`outline` does **not** resolve references, infer types, follow re-export
chains, build a call graph, or persist an index. Export and public
classification is syntax-only. It is a local summary, not a symbol database
(`ast-grep §5.0`, `§23.1`).

## Mandatory tripwires

Before each of these, run the stated inquiry. Patterns are code, not text
(`ast-grep §6.0`); `$X` captures one node and `$$$A` captures zero or more
(`§6.2`–`§6.3`).

| Before you... | Required inquiry |
|---|---|
| Change a function or method signature | Tier 2 for call sites — `ast-grep run -l rust -p '$R.method($$$A)'` — then tier 1: change it and let the compiler enumerate the rest |
| Add or change a trait/Protocol method | `ast-grep run -l rust -p 'impl TraitName for $T { $$$ }'` for every implementor, then read each one's current interface |
| Change a base-class contract | `ast-grep run -l python -p 'class $C(BaseName): $$$'` for subclasses, plus tier 2 for construction sites |
| Add or remove a contract field | Constructors, deserialization, factories, fixtures, and direct consumers — tier 2 for field access, tier 3 for the serialized name |
| Delete a symbol/module/rule | Tier 2 for references, tier 3 for string and config residue, then tier 1: delete it and prove the build is clean. All three, or it is not a zero-state proof |
| Claim a repository-wide invariant | An executable rule — `ast-grep scan --inline-rules` for a one-off, a rule in `ruleDirs` when it must persist. Not a hand-tallied search |
| Use an existing API in a design exemplar | Definition read at the pinned current version, per the library evidence hierarchy in `evidence-policy.md` §5 |

## Structural queries

Start with the least complicated construct that expresses the requirement
(`ast-grep §0.5`):

```text
exact node kind -> code pattern -> pattern + metavariables
  -> pattern object (context + selector) -> atomic YAML rule
  -> relational/composite rule -> utilities -> transform + fix
```

Complex YAML is not inherently more precise than a clear pattern. Every extra
relational traversal is another assumption to validate.

**`-p` or `-k`.** Use `--kind`/`-k` when node type or relationship is
sufficient; use `--pattern`/`-p` when concrete syntax, captures, repeated
metavariable equality, or rewrite interpolation matters (`§3.3`). `-k` accepts
ESQuery-style selectors and pseudo-classes:

```bash
ast-grep run -l rust -k 'function_item > identifier' crates/
ast-grep run -l rust -k unsafe_block crates/
```

**Rule model** for anything `-p`/`-k` cannot express (`ast-grep §7`):

- **Atomic** — `pattern`, `kind`, `regex`, `nthChild`, `range`. `regex` runs
  the Rust engine over node text: no lookaround, no backreferences.
- **Relational** — `inside`, `has`, `precedes`, `follows`, each accepting
  `stopBy: neighbor|end|<rule>` and (for `inside`/`has`) `field`. These answer
  what text search cannot: every call *inside* a given impl, every decorated
  definition, every field access within a constructor.
- **Composite** — `all`, `any`, `not`, `matches`. Use explicit `all` when one
  sub-rule captures a metavariable a later one consumes; field evaluation order
  is otherwise not guaranteed (`§7.1`).

A one-off relational query, no config file required:

```bash
ast-grep scan --inline-rules '
id: unfinished-impl
language: rust
severity: error
message: trait impl left unfinished
rule:
  pattern: todo!()
  inside:
    kind: impl_item
    stopBy: end
' src/
```

**Strictness** has six modes — `cst`, `smart` (default), `ast`, `relaxed`,
`signature`, `template` (`§3.7`). Do not reflexively pick the strictest: choose
the minimum invariants the claim actually depends on. Too strict yields false
negatives after harmless reformatting; too permissive conflates distinct
constructs.

When a pattern does not match, inspect the parse before making it more complex:
`--debug-query=cst` shows named and unnamed nodes, `=ast` shows named only.

## Coverage is a declared artifact

State the scope you searched. Do not assume it.

An empty result proves nothing until you know what was in the candidate set:

```bash
rg --files -g '!docs/library_ref/**'      # candidate set, before searching
rg --debug 'pattern' src 2>&1 | grep -i ignore   # per-file skip reasons
rg --type-list | grep -E '^(py|rust):'    # what -t actually selects
ast-grep run -l rust -p '...' --inspect summary .   # scannedFileCount / skippedFileCount
```

`--inspect summary` writes `scannedFileCount=N,skippedFileCount=M` to stderr,
keeping stdout clean for `--json`. That pair is the coverage envelope; cite it
when making a negative claim.

### Exit codes differ by subcommand

Scripted zero-state proofs depend on this and it is not uniform:

| Command | 0 | 1 | 2 |
|---|---|---|---|
| `ast-grep run` | matched | **no match** | — |
| `ast-grep scan` | no error-severity finding | error-severity finding fired | — |
| `ast-grep outline` | completed, **including an empty outline** | — | fatal read/parse/config error |
| `rg` | matched | no match | error |

So a clean zero from `run` is exit 1, not failure — but do not invert that into
"any nonzero means no matches"; capture stderr and distinguish real errors
(`ast-grep §3.14`, `§5.16`). An empty `outline` is data, not a signal.

`--json` takes its style with `=`. Write `--json=stream`, never `--json stream`
— the bare word parses as a path (`§2.7`).

### This repository's traps

Both are live today and both silently shrink results:

- **`.claude/` is hidden, so default `rg` cannot see the skills.** Searching
  for a term that appears in both `.claude/skills/` and a tracked directory
  returns only the tracked hits — which reads exactly like "found them all".
  Compare `rg --files | grep -c '^\.claude/'` (zero) against
  `rg --files --hidden -g '!.git/**' | grep -c '^\.claude/'`. Use
  `--hidden -g '!.git/**'` whenever skills, settings, or any dotfile are in
  scope (`ripgrep §16.0`).
- **`docs/library_ref/` is ~9 MB of prose**, several files over 1 MB, and it
  mentions most identifiers you will ever search for. Exclude it with
  `-g '!docs/library_ref/**'` unless the question is *about* the references.

**Widening the ignore stack is a scoped debugging move, not a default.**
`.venv/` is gitignored and therefore invisible — correct for "our code"
questions, wrong for "what does the installed package actually do". `rg -uu`
takes the candidate set from 59 files to ~8,470, and `ast-grep --no-ignore
hidden --no-ignore vcs` does the equivalent. Both then reach `.env`-style
files, caches, local tooling state, vendored dependencies, and secrets —
this repository keeps a capability token in `.envrc.local` (`ast-grep §22.5`).
Widen deliberately, scoped to a path, and never as a standing default.

Treat the output the same way: matched source text, captures, and outline
signatures are source-code data. Do not ship broad `--json` output somewhere
external merely because it is structured (`§22.6`).

### Known blind spots — declare, do not paper over

- **ast-grep**: language is inferred from file extension; a bare pattern
  fragment may not parse without `context`/`selector` (`§6.8`); `$X` matches a
  single node, so it will not span an argument list; `$$OP` is needed to
  capture unnamed operator/punctuation nodes; a multi-metavariable cannot be
  the root pattern; repeated `$A` silently enforces syntactic equality.
- **rg**: the default engine has no lookaround or backreferences — use
  `-P`/`--engine` (`ripgrep §5.1`, `§5.0`); `-g dir` does not mean `dir/` —
  write `-g 'dir/**'` (`ripgrep §15.1`); later matching globs win
  (`ripgrep §15.0`); manual `-g` overrides sit *above* the ignore stack while
  `-t` type filters are a separate layer the reference does not place in it
  (`ripgrep §13.1`, `§15.3`); hidden and binary files are skipped by default
  (`ripgrep §13.0`, `§16.1`).
- **Both**: dynamic dispatch, `getattr`, macro-generated code, trait objects,
  re-exports, and string-keyed registries are invisible to structure. Name this
  residue in the claim instead of implying it was covered.
- **Version is part of the claim.** A structural result is only valid for the
  ast-grep and grammar version that produced it — a parser upgrade can change
  node kinds, field names, and named/unnamed boundaries (`ast-grep §22.15`). Record the
  tool version alongside any structural claim you expect to survive.

### Navigating the design specs

`docs/upfront_design/` holds six normative specs, ~650 KB. Do not read them to
find a section — outline them:

```bash
spec-outline                                  # every spec, by section (28 KB)
spec-outline <spec>.md --match '^36\.' --view expanded    # one section + subsections
spec-outline <spec>.md --json=compact         # machine-readable, with line ranges
```

`scripts/spec-outline.sh` wraps `ast-grep outline` with a project extractor
(`tooling/ast-grep/outline/specs.yml`) that markdown has no bundled equivalent
for. Items are `## N. Section`, members are `### N.N Subsection`; output is one
line per section with the line number to seek to. `# Part N` groupings and h4
are not represented — outline exposes only item plus direct member.

Section numbers drift when a spec is revised. Confirm a citation with
`spec-outline <spec>.md --match '^N\.'` before relying on it; four citations in
`CLAUDE.md` were stale by 60+ sections when this extractor was first run.

The library references in `docs/library_ref/` need the sibling `lib-outline`
(`tooling/ast-grep/outline/library-ref.yml`), because they are h1-rooted —
chapters are `#`, subsections are `##`:

```bash
lib-outline                                   # every reference, by chapter
lib-outline <ref>.md --match '^Appendix M' --view expanded   # one chapter + subsections
```

`spec-outline`'s h2/h3 mapping matches no chapter in those files and promotes
their subsections to items, producing a flat wall that omits whole appendix
ranges; each script now refuses the other's tree rather than emitting that.
Prefer either over `rg` for headings there — both parse markdown, so a
`# Cargo.toml` line inside a fenced block cannot masquerade as a heading, and a
document's own table of contents nests under its map instead of colliding with
real sections. `--match` filters items, not members.

## Durable rules

This is the default destination for any invariant asserted more than once:
per `evidence-policy.md` §0, promote it to an executable rule and cite the
rule id thereafter, rather than re-deriving it each session or re-asserting
it per artifact. `sgconfig.yml` at the repository root:

```yaml
ruleDirs: [rules]
testConfigs:
  - testDir: rule-tests
```

`ast-grep scan` then runs the full ruleset and **exits 1** when an
`error`-severity rule fires, so an invariant becomes a gate. Severity is
deployment policy, not a property of the finding: `--error=<id>`, `--off=<id>`,
and friends re-rank rules per run, and `--filter <regex>` runs a subset.
`--format github|sarif` emits CI annotations.

Rules are testable, and a rule that only has passing examples is not tested.
`ast-grep test --update-all` accepts snapshots into `rule-tests/__snapshots__/`;
a later `ast-grep test` exits 0 only if behavior is unchanged. Cover the
negative space (`§22.19`): near-misses, already-migrated code, commented-out
code, nested occurrences, and boundary positions in a list. A rule correct on
its positive examples can still be unsafe.

This is the mechanism behind "structural governance rules" in
`validation-policy.md`.

## Usage by phase

### Design and planning

Establish current boundaries, ownership, and inventories; consumer and
implementation surfaces; duplicate or parallel implementations; legacy-pattern
extent; likely test and governance surfaces; and whether a proposed abstraction
already exists. `outline --items exports` over a subtree answers most of the
inventory questions before any file is read.

Do not enumerate the whole repository when a bounded subsystem answers the
question.

### Execution

Re-run the impact probe immediately before changing a load-bearing interface.
Plans are evidence-backed hypotheses, not a licence to skip current-tree
verification.

### Review and decommission

Use both directions of proof:

- **positive** — the target symbol, contract, behavior, or route exists and is
  used;
- **negative** — the old symbol, route, registration, alias, and authority have
  reached zero *within a stated coverage envelope*, confirmed at tier 1 where
  the language permits.

Prefer `--json` on both tools when the result backs a recorded claim
(`evidence-policy.md` §6), so the record cites a reproducible command rather
than a pasted screenful.

Do not use a tool to certify its own modification. When changing analysis or
search tooling — including CodeFabric's own analyzers, once they exist — verify
with independent source reads, compiler output, and tests.
