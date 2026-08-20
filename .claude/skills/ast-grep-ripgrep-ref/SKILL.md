---
name: ast-grep-ripgrep-ref
description: "Reference navigator for the two search engines this repository runs — one structural, one textual. SKILL.md maps two deep-dives at `docs/library_ref/`: `ast-grep_0.45.1_advanced_reference.md` (patterns and metavariables, ESQuery selectors and pseudo-classes, the atomic/relational/composite rule object, `outline`, lint-rule YAML and severity, rewrites/transforms/rewriters, languages and injections, JSON and coordinate contracts, a 26-symptom triage playbook, 28 capability matrices; §0-§25) and `ripgrep_advanced_reference_15.2.0_pcre2_10.47.md` (engine selection, fixed strings and case, multiline and dotall, the ignore stack and glob/type precedence, replacements and capture interpolation, JSON, encodings, preprocessors, output separators, match caps, threading and sorting, debugging — then PCRE2 10.47 in depth: lookaround, variable-length lookbehind, backreferences, atomic groups and possessive quantifiers, `\\K`, control verbs, recursion, set algebra, scan-substring assertions, JIT and resource limits — plus agent recipes and three lookup matrices; §0-§49). SKILL.md is the use-case router: fourteen search jobs, each mapped to the capability that answers it in ast-grep, in ripgrep's default Rust engine, or in PCRE2, and to the section that specifies it. REFERENCE.md (same folder) is the mechanical layer: chapter maps with line numbers, a capability-to-section index, the full use-case matrix, cross-tool equivalences, decision trees, exit-code and output contracts, and navigation hazards. Use when a search needs more than a literal — lookaround, backreferences, `-P`/`--engine`, `-U` multiline, `\\p{...}`, capture extraction with `-o`/`-r`; when the ignore stack, globs, or file types decide the answer; when the claim is structural (call sites, impls, `inside`/`has`/`nthChild`) or negative (absence within a stated envelope); when authoring an ast-grep YAML rule, `sgconfig.yml`, an outline extractor, or a codemod; when a version-gated PCRE2 construct must be proven available rather than assumed; or when a search returned nothing and you need the triage path. Doctrine — the three-tier instrument ladder, the mandatory tripwires, coverage-as-declared-artifact — stays in `.claude/skills/_shared/code-intelligence.md`; this skill is the capability surface underneath it."
allowed-tools: Read, Grep, Glob, Bash
---

# ast-grep + ripgrep + PCRE2 Reference Navigator

Routes the two deep-dive references for **searching code**. This SKILL.md is the
**use-case router**: version anchors, the three matching engines and what each
owns, the two-document map, and fourteen *search jobs* — each stating the
question shapes it covers, which instrument answers them, and the sections that
specify the capability. The companion **`REFERENCE.md`** (same folder) is the
mechanical layer: per-document chapter maps with line numbers, the
capability-to-section index, the full use-case matrix, the cross-tool
equivalence tables, six decision trees, the combined exit-code and output
contracts, the navigation hazards, and the operating rules. Reach for
REFERENCE.md once you know *which* capability you need and want its exact
location; cross-references back here are written `SKILL §...`.

**Out of scope — the one handoff.** `.claude/skills/_shared/code-intelligence.md`
owns the **doctrine**: the three-tier instrument ladder (compiler → ast-grep →
rg), the mandatory tripwires before a signature or contract change,
coverage-as-a-declared-artifact, the review/decommission proof directions, and
this repository's two live search traps. That file tells you *what a claim
requires*. This skill tells you *which capability produces it and where it is
specified*. Read them in that order and do not expect either to restate the
other.

Semantic questions — resolved symbol identity, types, callers by identity, trait
implementation resolution, macro expansion — are outside both tools by
construction (`ast-grep §0.2`, `ast-grep §24.22`; `ripgrep` Final Invariant 12,
line 3137: *"ripgrep is a lexical evidence engine, not a semantic code graph"*).
Those escalate to the compiler, an LSP, or CodeFabric's own CPG once it exists.

---

## Version anchors

* **ast-grep 0.45.1** — Tree-sitter parses both the code and the pattern;
  matching, reporting and rewriting happen over syntax nodes. Workspace declares
  edition 2024 and MSRV 1.88.0. **Invoke `ast-grep`, never `sg`**:
  `ast-grep §1.1` gives four reasons, the load-bearing two being that the `sg`
  alias is deprecated in the 0.45 line and that Linux already provides
  `/usr/bin/sg` for changing group execution context — front matter line 15
  names that command `setgroups`. The document is explicit that upstream
  generated pages lag the implementation and keeps a **drift ledger** at
  `ast-grep §24.24`; the installed binary is the final authority on its own
  flags (`ast-grep §25.1`).
* **ripgrep 15.2.0** (released 2026-07-15) — line-oriented, literal-prefiltered,
  ignore-aware recursive search. Two selectable regex engines, chosen globally
  per invocation.
* **PCRE2 10.47** (released 2025-10-21; CVE-2025-58050 fix included,
  `ripgrep §40.0`) — reached only through `rg -P` / `--engine=pcre2`. **The
  PCRE2 version cannot be inferred from the ripgrep version** — ripgrep may
  bundle it or link it dynamically. Probe it (`ripgrep §1.0`, `ripgrep §44.8`):

  ```bash
  rg --version          # ripgrep itself, and whether +PCRE2 was compiled in
  rg --pcre2-version    # the version it was COMPILED AGAINST
  ```

  **That second number is a floor, not a measurement.** It comes from the PCRE2
  headers at build time, so a dynamically-linked binary can be running a newer
  library than it names — a build here reports `PCRE2 10.45` while the
  10.47-only returned-capture recursion of `ripgrep §30.2` matches and its
  control correctly fails. A version *at or above* what you need is good
  evidence; a version *below* it is no evidence at all. For anything
  version-gated — returned-capture recursion (`ripgrep §30.2`), scan-substring
  assertions (`ripgrep §34`), extended classes (`ripgrep §33`) — **feature-probe
  with a control** and record the probe result as the capability fact
  (`ripgrep §42.9`, `ripgrep §36.1`, `ripgrep §42.7`).

This repository's session-bootstrap block prints the installed versions live. Do
not quote a version from this file or from either reference when the context
block has told you the truth.

---

## Three engines, and who owns what

There are **three** matchers here, not two. Conflating the second and third is
the single most common error this skill exists to prevent.

```text
   ast-grep  ──────────────  Tree-sitter parse -> structural match
                             sees: node kinds, fields, relations, positions
                             blind to: bytes that are not a parse

   rg (default)  ──────────  Rust regex -> linear-time, literal-prefiltered
                             sees: every byte, in any file, in any language
                             blind to: structure, and to lookaround/backrefs

   rg -P  ─────────────────  PCRE2 10.47 -> backtracking
                             sees: everything the default sees, plus assertions
                             blind to: structure — and to its own worst case
```

| Engine | Owns | Explicitly does **not** own |
|---|---|---|
| **ast-grep** | grammar structure, ancestor/descendant/sibling relations, positional constraints, captures that enforce equality, syntax-preserving rewrite, durable repository rules | resolved symbols, inferred types, call graph, module resolution, macro semantics (`ast-grep §0.2`) · lookaround and backreferences *at any complexity* (see below) |
| **rg default** | literal-prefiltered scanning at scale, linear-time guarantee, Unicode properties, the ignore stack, file typing, JSON evidence | arbitrary lookaround, backreferences (`ripgrep §5.1`, line 360) |
| **rg -P (PCRE2)** | lookahead/lookbehind, backreferences, `\K`, control verbs, recursion/subroutines, set algebra, scan-substring assertions | the linear-time guarantee · a sandbox — pattern-level limits are defence in depth only (`ripgrep §35.2`, `ripgrep §40.4`, `ripgrep §43.12`) |

**The load-bearing consequence.** ast-grep's `regex:` atomic rule runs the
**Rust** engine over node text — the reference states outright that it
"intentionally lacks constructs such as lookaround/backreferences"
(`ast-grep §7.4`, line 1918). So *lookaround and backreferences are unavailable
inside an ast-grep rule at any level of YAML complexity*. Needing them is not a
reason to write a more elaborate rule; it is a signal to either express the
requirement structurally (`REFERENCE.md §4` gives the equivalence for each
construct) or compose the two tools:

```bash
# structure narrows the candidate set, PCRE2 applies the assertion
ast-grep run -l rust -p 'fn $N($$$A) -> $R { $$$B }' --json=stream . \
  | jq -r '.file' | sort -u \
  | xargs rg -P --no-heading -n '(?<!async )fn \w+'
```

`ast-grep §23.14` is the document's own chapter on combining the two.

---

## The two documents and how to read them

Both live at `docs/library_ref/`. Both share one skeleton: front matter → a
`# Comprehensive documentation map` TOC → `# N)` chapter bodies → lookup
matrices → checklists → a source index.

| Alias | Path (`docs/library_ref/`) | Lines | Chapters | `## N.M` subs | Body prefix |
|---|---|------:|---|---:|---|
| **ast-grep** | `ast-grep_0.45.1_advanced_reference.md` | 8,297 | **§0-§25** | 368 | `# N) ` |
| **ripgrep** | `ripgrep_advanced_reference_15.2.0_pcre2_10.47.md` | 3,272 | **§0-§49** | 265 | `# N) `, except **§0** = `# ripgrep Advanced — 0) ` |

| Role | ast-grep | ripgrep |
|---|---|---|
| front matter (`## Version / source anchors`) | 1-68 | 1-32 |
| `# Comprehensive documentation map` (the TOC) | 69-99 | 33-87 |
| chapter bodies begin | **100** | **88** |
| the lookup-matrix chapters | **§24** (7331-7849, 28 matrices) | **§45**/**§46**/**§47** (2857-3128) |
| the agent chapters | **§23** (24 workflows) | **§41** (17 recipes) · **§42** (evidence doctrine) |
| the anti-pattern inventory | **§25.12** (8146) | **§43** (13 named) |
| checklists | per-chapter `### Agent checklist`, final at 8166 | **§48** (six checklists) |
| source index | 8210-8278 | 3216-3246 |
| epilogue | `# Closing operational summary` 8279-8297 | `# Final agent invariants` 3247-3272 |

**Reading strategy.** Find the section in `REFERENCE.md §1`, then
`Read(offset, limit)`. ast-grep chapters run 119-683 lines and nearly all close
with a fenced `### Agent checklist` — **load the closing 20-40 lines before
authoring a rule or a codemod.** ripgrep chapters are short (24-159 lines), so
read whole chapters. When you only need a flag or a construct, `ripgrep §45`/
`ripgrep §46` and `ast-grep §24` are faster than either chapter body.

**Citations are always alias-qualified.** Both documents number chapters `# N)`
and subsections `## N.M`, and nine chapter numbers mean different things in
each. A bare `§N` is ambiguous; write `ast-grep §5.4` or `ripgrep §5.2`. The
collision table is `REFERENCE.md §7` Rule 1.

---

## Search jobs

Fourteen jobs, ordered by *what you are looking for* (1-9) then *how you
constrain and consume it* (10-14). Each names the question shapes it covers, the
capability that answers them, and the trap. `REFERENCE.md §3` is the exhaustive
row-level matrix behind these; this is the narrative.

### 1. You are mapping a tree or a file you have not read

*"What is in `src/query/`?" · "What does this 2,000-line file define?" · "What
does this module import?"*

`ast-grep outline` is the answer and it is cheap. It emits a syntax-aware table
of contents without reading bodies (`ast-grep §5`). **Its defaults change with
input type, deliberately** (`ast-grep §5.2`): a directory gives
`--items exports --view names` (a small public map), a file gives
`--items structure --view digest` (signatures plus member-name groups). Pick the
smallest view that answers the planning question (`ast-grep §5.4`, and the
token-budget rule at line 1187).

```bash
ast-grep outline src                                    # public surface, cheapest
ast-grep outline src --items imports --view signatures  # dependency direction
ast-grep outline src/planner.rs --match '^Planner$' --type struct --view expanded
```

**Traps.** `--match` and `--type` filter **top-level items, not members**
(`ast-grep §5.5`, `ast-grep §5.6`) — to reach a method, select its parent item
and expand. Export/public classification is syntax-only, and `--pub-members`
treats a member as public whenever the extractor has no `isPublic` rule
(`ast-grep §5.7`). `outline` resolves nothing, follows no re-export chain, and
builds no index (`ast-grep §5.0`); `ast-grep §5.14` names the cases where it
must not be your only discovery tool. Its exit code is **0 even for an empty
outline** — emptiness is data, not failure (`ast-grep §5.16`).

Markdown has no bundled extractors, so this repository ships two under
`tooling/ast-grep/outline/` and wraps them: `spec-outline` for the h2-rooted
design specs, `lib-outline` for the h1-rooted references — **including the two
this skill routes**. Custom extractor authoring is `ast-grep §14`.

### 2. You are looking for a declaration

*"Where is `SnapshotId` defined?" · "Which files declare a `TableProvider`
impl?"*

If you know roughly where it lives, `outline --match '^Name$' --type struct,enum`
is the cheapest hit. If you do not, a structural pattern is next:
`ast-grep run -p` when the shape matters, `-k` when a node kind or ESQuery
relation is sufficient (`ast-grep §3.2`, `ast-grep §3.3`, and the decision rule
at line 523).

```bash
ast-grep run -l rust -k 'function_item > identifier' src/
ast-grep run -l python -p 'class $C($BASE): $$$' .
```

Drop to `rg` when the grammar cannot help: a language with no parser, a name
constructed at runtime, or a declaration living in TOML/JSON/proto.
`ripgrep §41.0` is the token-boundary definition recipe; `-w` is the boundary
primitive (`ripgrep §8.0`).

### 3. You are looking for usages and call sites

*"Who calls `publish_snapshot`?" · "Every `.await` on this client" · "Is this
symbol dead?"*

`ast-grep run -p` with an arity-correct pattern. `$X` matches **one node**; an
argument list needs `$$$ARGS` (`ast-grep §6.2`, `ast-grep §6.3`). A
multi-metavariable cannot be the root pattern (`ast-grep §6.3`, line 1584).

```bash
ast-grep run -l rust -p 'publish_snapshot($$$A)' src/
ast-grep run -l rust -p '$R.publish_snapshot($$$A)' src/
```

**This is tier 2 and it is not caller completeness.** String-keyed registries,
generated code, dynamic dispatch and cross-language consumers survive a
structural search untouched — the ripgrep document's own framing of the gap is
`ripgrep §42.8`, "Evidence completeness vs grep completeness". A dead-code claim
needs all three tiers; that policy is in `_shared/code-intelligence.md`, not
here.

### 4. You are looking for a construct defined by its context

*"Every `todo!()` inside an `impl`" · "Calls in the constructor only" · "The
decorated definitions" · "The second argument"*

This is the class of question text search structurally cannot express, and where
ast-grep's relational rules earn their cost (`ast-grep §7.7` through
`ast-grep §7.12`). Read a relational rule as
`TARGET relation SURROUNDING-MATCH`: the relation narrows the target, it does
not change what is reported.

- `inside` / `has` (`ast-grep §7.10`, `ast-grep §7.11`), each taking `field` to
  pin the grammar field (`ast-grep §7.9`)
- `precedes` / `follows` (`ast-grep §7.12`)
- `stopBy: neighbor | end | <rule>` — the traversal stop policy
  (`ast-grep §7.8`); `stopBy: end` everywhere when a direct-child relation
  suffices is a named anti-pattern (`ast-grep §4.14`, `ast-grep §20.3`)
- `nthChild` — one-based, counts **named** siblings, takes `ofRule` and
  `reverse` (`ast-grep §7.5`)
- the ESQuery form of the same ideas, often shorter: `>`, `+`, `~`, and the
  0.42+ pseudo-classes `:has`, `:not`, `:is`, `:nth-child`, `:nth-last-child`
  (`ast-grep §7.18`, matrix `ast-grep §24.11`)

```bash
ast-grep scan --inline-rules '
id: unfinished-impl
language: rust
severity: error
message: trait impl left unfinished
rule:
  pattern: todo!()
  inside: { kind: impl_item, stopBy: end }
' src/
```

**Trap.** Field evaluation order within one rule object is not guaranteed. If
one sub-rule captures a metavariable that a later one consumes, use explicit
`all:` to get ordered evaluation (`ast-grep §7.1`, line 1854).

### 5. You are looking for a construct defined by what it *lacks*

*"Handlers with no `#[instrument]`" · "`unwrap()` outside tests" · "`use` lines
that are not `pub use`"*

Two routes, and they are not equivalent.

**Structural negation** — `not` (`ast-grep §7.15`), `:not`
(`ast-grep §7.18`), or `not: { inside: ... }`. This is exact, because it negates
a parse.

**PCRE2 negative lookahead/lookbehind** — `(?!...)`, `(?<!...)`
(`ripgrep §26.1`, `ripgrep §26.3`). Only `rg -P`; the default engine cannot
(`ripgrep §5.1`). Lookbehind must be bounded, and the bound is **255 characters
with no ripgrep flag to raise it** (`ripgrep §27.1`, `ripgrep §27.2`); `\K` is
the practical substitute (`ripgrep §27.3`, `ripgrep §31.0`).

**The trap that costs the most time.** `constraints` are evaluated *after* the
primary match, so they cannot rewrite `not` semantics — the reference calls this
the "negation footgun" (`ast-grep §8.6`, lines 2474-2491). And remember the
three engines above: you cannot reach for lookahead from inside an ast-grep rule
at all.

### 6. You are looking for two things co-located

*"Lines mentioning both `Delta` and `commit`, in either order" · "A call whose
receiver is also matched"*

With PCRE2, conjunction on one line is a double lookahead — `ripgrep §41.1` is
the worked recipe. Structurally, it is `all:` plus a relational rule
(`ast-grep §7.13`), which is exact across line breaks and formatting where the
regex is not.

Prefer the cheap two-pass pipeline when the corpus is large: `rg -l` for one
term, feed the file list to the second search. `ripgrep §42.1` ("staged
narrowing") is the document's own recommendation and avoids `-P` entirely.

### 7. You are looking for a *repeated* token

*"`x == x`" · "duplicated word in prose" · "paired quote delimiters" · "the same
identifier on both sides"*

Two mechanisms, and the difference is real.

- **ast-grep**: a repeated captured metavariable enforces equality —
  `-p '$A == $A'` matches `x == x` and `(a+b) == (a+b)` but not `x == y`
  (`ast-grep §6.4`). This is **syntactic** equality, never resolved-symbol
  identity. `$_`-prefixed names are non-capturing and impose no such constraint
  (`ast-grep §6.5`).
- **PCRE2**: backreferences `\1` and `\k<name>` (`ripgrep §28.0`,
  `ripgrep §28.2`). `ripgrep §41.5` is the duplicate-token recipe,
  `ripgrep §41.6` and `ripgrep §28.1` the paired-delimiter one. Backreferences
  defeat the literal prefilter and create backtracking risk (`ripgrep §28.4`).

Balanced delimiters look like a third case but are not: PCRE2 recursion
`(?R)`/`(?&name)` exists (`ripgrep §30`) and the document says outright that it
**is not a parser replacement** (`ripgrep §30.1`). If you need balance, you need
a parse — use `has`/`inside`.

### 8. You are looking for literal text with no AST

*"This error message" · "A config key" · "A docstring" · "The name in a
`[project.scripts]` entry" · "Anything in generated output"*

`rg`, and usually with the regex turned off. `-F/--fixed-strings` makes the
whole pattern set literal (`ripgrep §6.0`); `-w`/`-x` wrap word and line
boundaries without hand-anchoring, which `ripgrep §8.2` tells you not to do.
Multiple alternatives are repeatable `-e` (`ripgrep §4.1`) or `-f` a pattern
file (`ripgrep §4.2`).

**Traps.** Once `-e` or `-f` appears, every positional argument is a **path**,
not a pattern (`ripgrep §4.1`, line 304). An **empty line in a `-f` file is an
empty regex and matches everywhere** (`ripgrep §4.2`, line 312). `-f -` consumes
stdin, so stdin cannot also be the haystack (`ripgrep §4.3`).

Comments and docstrings are a special case: ast-grep's `relaxed` and `signature`
strictness modes ignore comments entirely (`ast-grep §6.10`), so a
comment-targeted question is a `rg` question by construction.

### 9. You are looking for a pattern that spans lines

*"A struct definition and its first field" · "A `match` arm across a line break"
· "An import block"*

Structurally this is a non-question: ast-grep matches nodes, and a node does not
care where the line breaks fall. Reach for `rg -U` only when there is no parse
to lean on.

`ripgrep §7.0` separates three concepts that are routinely confused — multiline
search (`-U`), dotall (`--multiline-dotall`), and anchor behaviour.
`--multiline-dotall` has **no effect without `-U`** (`ripgrep §7.2`, line 458;
`ripgrep §24` Invariant 4).

**The correction the document goes out of its way to make.** `-U` is a
*searcher* permission to consider a multi-line haystack. It is **not** the PCRE2
multiline-anchor switch — ripgrep 15.2 always enables PCRE2 multiline-anchor
mode regardless (`ripgrep §24` Invariant 3, lines 1128-1141), and
`ripgrep §44.4` exists specifically to correct older references that claim
otherwise. Under PCRE2, a pattern containing `\n` **without `-U` silently fails
to match**, with no diagnostic (`ripgrep §5.4`, lines 387-392). Repository-wide
multiline dotall is a named anti-pattern (`ripgrep §43.1`), and multiline forces
reading to EOF (`ripgrep §37.6`, `ripgrep §19.3`).

---

### 10. You are choosing the regex engine

Default first, always. `ripgrep §0.2` frames the whole tool as "fast default,
fancy opt-in", and `ripgrep §43.0` names `-P` for every search as anti-pattern
zero.

Escalate only for a construct the default cannot express — the full list is
`ripgrep §5.1` (what default supports) against `ripgrep §5.2` (what triggers
PCRE2), the 39-row feature matrix is `ripgrep §46`, the 22-row
engine-compatibility matrix is `ripgrep §47`, and `ripgrep §47.1` is a five-step
escalation algorithm you can follow mechanically. Selection is **global to the
invocation**: you cannot mix engines across `-e` patterns (`ripgrep §4.4`).

`--engine=auto` exists but makes the choice implicit and unauditable
(`ripgrep §5.3`, lines 383-385); for evidence work, state the engine
(`ripgrep §42.5` determinism template). `--auto-hybrid-regex` is deprecated
compatibility spelling — use `--engine=auto` (`ripgrep §44.2`, line 2652).

**Do not confuse the size knobs.** `--regex-size-limit` and `--dfa-size-limit`
are passed to the *Rust* matcher builder; the 15.2 PCRE2 path does not consume
them (`ripgrep §24` Invariant 5, line 1148; restated `ripgrep §45.11`,
`ripgrep §43.8`). PCRE2's own limits are the pattern-level verbs
`(*LIMIT_MATCH=)`, `(*LIMIT_DEPTH=)`, `(*LIMIT_HEAP=)` (`ripgrep §35.1`) — and
`ripgrep §35.2` is explicit that they are not a sandbox.

### 11. You are scoping the candidate set, and proving what you scoped

*"Did it search the file I meant?" · "Why is nothing coming back?" · "Can I
claim this is everywhere?"*

Narrow the file set **before** complicating the matcher — the first optimisation
in both documents (`ast-grep §20.1`, `ripgrep §42.1`).

| Lever | ripgrep | ast-grep |
|---|---|---|
| path globs | `-g` / `--iglob`, later wins (`ripgrep §15.0`-`§15.2`) | `--globs`, later wins (`ast-grep §2.4`) |
| language / type | `-t`/`-T`/`--type-list`/`--type-add` (`ripgrep §15.3`-`§15.4`) | `-l/--lang`; `languageGlobs` (`ast-grep §3.4`, `ast-grep §12.6`) |
| ignore layers | precedence stack (`ripgrep §13.1`) · `-u`/`-uu`/`-uuu` (`ripgrep §13.2`) · `--no-ignore-*` (`ripgrep §13.3`) | `--no-ignore hidden\|dot\|exclude\|global\|parent\|vcs` (`ast-grep §2.3`) |
| hidden files | `--hidden` (`ripgrep §16.0`) | `--no-ignore hidden` (`ast-grep §2.3`) |
| symlinks | `-L` (`ripgrep §12.2`) | `--follow` (`ast-grep §2.2`) |
| coverage proof | `rg --files` · `--debug` · `--trace` (`ripgrep §22.0`-`§22.2`) | `--inspect summary\|entity`, to **stderr** (`ast-grep §3.13`, `ast-grep §4.11`) |
| worker threads | `-j NUM` (`ripgrep §19.1`) | `-j NUM` (`ast-grep §3.9`) |
| deterministic order | `--sort path` (`ripgrep §19.2`) | `-j 1` (`ast-grep §3.9`) |
| cap output per file | `-m NUM` (`ripgrep §10.4`) | — |

**Traps.** `-g dir` does not mean the directory — write `-g 'dir/**'`
(`ripgrep §15.1`). `--type-add` does not mutate a global registry; persist it in
a config file (`ripgrep §15.4`, line 798). Nested `.gitignore` files override
their ancestors (`ripgrep §13.1`, line 705). `--hidden` pulls in `.git` object
data unless you exclude it (`ripgrep §16.0`). And in ast-grep, a rule's
`files:`/`ignores:` fields are **not** filesystem discovery — they filter after
traversal (`ast-grep §8.13`, line 2650).

`--inspect summary` writes `scannedFileCount=N,skippedFileCount=M` to stderr
while stdout stays clean for `--json`. That pair is the coverage envelope; cite
it with any negative claim. Binary and hidden files are skipped by default in
both tools (`ripgrep §13.0`, `ripgrep §16.1`), which is a silent narrowing, not
an error.

**Widening the ignore stack is a scoped debugging move, never a default** —
`rg -uu` and `ast-grep --no-ignore vcs` both reach `.envrc.local`, which holds
this repository's capability token (`ast-grep §22.5`; `ripgrep §43.3`).

### 12. You are extracting and consuming what you found

*"Just the matched identifier" · "Feed this to `jq`" · "Hand these filenames to
`xargs` safely"*

- **one field, not the line** — `-o/--only-matching` (`ripgrep §9.2`, doctrine
  `ripgrep §42.4`)
- **reshape the output** — `-r/--replace` with `$1` / `$name` / `${name}`
  (`ripgrep §11.0`-`§11.1`); under PCRE2 the pattern is PCRE2's but the printer
  is still ripgrep's (`ripgrep §11.2`, `ripgrep §38.1`)
- **machine consumption** — `rg --json` (`ripgrep §20`, agent contract
  `ripgrep §20.3`) · `ast-grep --json=stream` with the match schema, capture
  data and coordinate contract at `ast-grep §17.1`, `ast-grep §17.6`,
  `ast-grep §17.3`
- **filenames across a pipe** — `-l` plus `--null`/`-0`, then `xargs -0`
  (`ripgrep §23.1`)
- **counts and existence** — `-c`, `--count-matches`, `-l`,
  `--files-without-match`, `-q` (`ripgrep §10.0`-`§10.2`)
- **cap the volume** — `-m/--max-count`, per file, not per run
  (`ripgrep §10.4`)
- **retune the delimiters** — `--context-separator`,
  `--field-match-separator` and friends (`ripgrep §9.5`), for humans only

**Traps.** `rg -r` is **output-only — it never writes to the file**
(`ripgrep §11.0`). **`-m` silently caps `-c`**: a file with three matching lines
reports `3` under `rg -c` and `2` under `rg -m 2 -c`, so never combine a cap
with a count you intend to use as evidence (`ripgrep §10.4`). Context lines are
printed with `-` where match lines use `:` (`ripgrep §9.5`), so a parser that
splits on `:` and assumes every line matched will misread every context line. `ast-grep`'s `--json` takes its style with `=`:
`--json=stream`, never `--json stream`, because the bare word parses as a path
(`ast-grep §2.7`). Never parse coloured terminal output as an API
(`ripgrep §43.4`; `ast-grep §2.6`). ast-grep JSON coordinates are zero-based
with byte offsets — preserve them as machine coordinates rather than
re-deriving from displayed text (`ast-grep §17.3`, `ast-grep §22.16`), and note
that `outline`'s rendered `NNNN:` prefix is 1-based while its JSON
`range.start.line` is 0-based.

### 13. You are changing what you found

Only ast-grep writes. Escalate in this order and stop at the first that
suffices:

```text
run -p ... -r ...            preview a one-off        (ast-grep 3.8)
  -> run ... -i              review interactively     (ast-grep 3.8)
  -> YAML fix: string        durable, testable        (ast-grep 10.2)
  -> FixConfig               range expansion          (ast-grep 10.4)
  -> transform + rewriters   generated metavariables  (ast-grep 11.3-11.9)
```

Classify the risk before applying: Class A structural substitution, Class B
syntax restructuring, Class C semantic migration (`ast-grep §10.10`). `-U` is
never the first move — `ast-grep §4.13` is the ten-step production rule
lifecycle and `ast-grep §22.2` requires a VCS guard before any bulk rewrite.
Generated text must be reparsed (`ast-grep §22.11`), the codemod should be
idempotent (`ast-grep §22.18`), and formatting ownership belongs to the
formatter, not the fix (`ast-grep §22.12`).

For text-level renames where structure genuinely cannot help, `rg -r` gives you
a **preview only**; the edit itself is a separate, deliberate step.

### 14. You are making it recurring — or debugging why it failed

**Recurring.** An invariant you will re-check is a rule, not a search.
`ast-grep §8` is the full YAML surface (`id`, `rule`, `constraints`, `utils`,
`message`, `severity`, `note`, `labels`, `files`, `ignores`, `metadata`);
`ast-grep §12` is `sgconfig.yml` discovery and `ruleDirs`; `ast-grep §9` is
utilities and the 0.42+ parameterized form; `ast-grep §15` is `valid`/`invalid`
cases and snapshots; `ast-grep §16` makes severity a **deployment** decision
separable from the rule (`--error=<id>`, `--off=<id>`, `--filter`). `scan` exits
1 only on an **error-severity** finding (`ast-grep §4.8`). A rule with only
passing examples is untested — cover the negative space (`ast-grep §22.19`).

`ast-grep test` does not use the grep-like contract: it **never returns 1**, and
signals `3` (filter selected nothing), `4` (assertion or snapshot failure), `6`
(missing `testDir`) or `8` (unparseable rule) — check zero-versus-nonzero, not a
specific code (`ast-grep §15.11`). The trap there is silent success: **a test
file naming a rule `id` that does not exist exits `0`** with
`0 passed; 0 failed`, so a typo deletes the test rather than failing it. Assert
on the number of cases executed when a suite must be non-empty.

**Debugging.** Both documents have a triage chapter; use them instead of making
the query more complicated.

| Symptom | Go to |
|---|---|
| pattern matches nothing | `ast-grep §21.2` four-step: force `--lang`, then `--debug-query=cst`, then a minimal real file, then add `context`/`selector` |
| too many matches | `ast-grep §21.4` |
| repeated metavariable misbehaves | `ast-grep §21.5` · `$$$` over/under-consumes `ast-grep §21.6` |
| comments/trivia change the result | `ast-grep §21.7` — reconsider strictness (`ast-grep §3.7`, `ast-grep §6.10`) |
| `kind`/ESQuery does not match | `ast-grep §21.8` |
| a file was silently skipped | `ast-grep §21.10` · `ripgrep §22.1` (`--debug`) |
| wrong language chosen for the file | `ast-grep §21.11` |
| works in `run`, fails in project `scan` | `ast-grep §21.9` |
| CI exit code surprises you | `ast-grep §21.19` · `ripgrep §10.3` |
| the regex engine lacks the construct | `ripgrep §22.4`, `ripgrep §47` |

`--debug-query` takes `pattern`, `ast`, `cst`, `sexp` and needs an explicit
`--lang` (`ast-grep §3.6`). Use `cst` when punctuation or an unnamed operator is
the culprit — that is also when you need `$$OP` rather than `$OP`
(`ast-grep §6.6`).

---

## The seam: rules that live between the two documents

Six places where reading one document reliably produces the wrong answer.

1. **Lookaround and backreferences are unreachable from ast-grep.** Its `regex:`
   rule is the Rust engine (`ast-grep §7.4`, line 1918); the constructs live
   only behind `rg -P` (`ripgrep §26`, `ripgrep §28`). The fix is a structural
   reformulation or a two-tool pipeline, never a bigger YAML rule.
   Equivalences: `REFERENCE.md §4`.
2. **Both tools have an ignore stack, and they are not the same stack.**
   ripgrep's precedence is `ripgrep §13.1` and its escape hatches are
   `-u`/`-uu`/`-uuu`; ast-grep's vocabulary is
   `--no-ignore hidden|dot|exclude|global|parent|vcs` (`ast-grep §2.3`).
   Explicit globs override ordinary ignore routing in both, and later globs win
   in both.
3. **Only one of them writes.** `ast-grep -U` / `scan -U` mutates files
   (`ast-grep §3.8`, `ast-grep §4.9`); `rg -r` only reformats output
   (`ripgrep §11.0`). A pipeline that assumes the wrong one is either a no-op or
   an unreviewed bulk edit.
4. **Exit codes disagree by subcommand, and one of them inverts.**
   `ast-grep run` returns **1** for a clean no-match, `scan` returns 1 only for
   an error-severity finding, `outline` returns 0 for an empty result and 2 for
   a fatal error, `test` never returns 1 at all (it uses 3/4/6/8), and `rg`
   returns 1 for no match and 2 for an error (`ast-grep §3.14`,
   `ast-grep §4.8`, `ast-grep §5.16`, `ast-grep §15.11`; `ripgrep §10.3`). Full
   table: `REFERENCE.md §6`.
5. **Size limits do not compose.** `--regex-size-limit`/`--dfa-size-limit` bound
   the Rust engine only (`ripgrep §24` Invariant 5); PCRE2 is bounded by
   pattern-level verbs (`ripgrep §35`); ast-grep's cost model is file count and
   traversal depth, controlled by scope and pruning (`ast-grep §20.1`,
   `ast-grep §20.3`), not by a regex knob.
6. **Version is part of the claim in both.** A structural result is valid only
   for the parser version that produced it (`ast-grep §22.15`), and a PCRE2
   pattern is valid only for a linked library new enough for its constructs
   (`ripgrep §42.7`). Record both alongside any result you expect to survive.

---

## Key invariants

1. **`ast-grep`, never `sg`.** Deprecated in 0.45; `/usr/bin/sg` already exists
   on Linux as the group-execution command (`ast-grep §1.1`, and front matter
   line 15). The further claim that the shim's warning banner corrupts `--json`
   output is **not made by this reference** — it comes from
   `_shared/code-intelligence.md`; do not attribute it here.
2. **Patterns are code, not wildcards.** `$X` is one named node, `$$$A` is zero
   or more in a list position, `$$OP` captures an unnamed operator, `$_X` is
   non-capturing, and a repeated `$A` silently enforces syntactic equality.
   (`ast-grep §6.2`-`§6.6`)
3. **Choose strictness from the invariants the claim depends on, not from
   habit.** Six modes — `cst`, `smart` (default), `ast`, `relaxed`, `signature`,
   `template`. Too strict yields false negatives after harmless reformatting;
   too permissive conflates distinct constructs. (`ast-grep §3.7`,
   `ast-grep §6.10`)
4. **Default engine first.** Escalate to `-P` only for a construct
   `ripgrep §5.1` cannot express, and record the escalation. (`ripgrep §0.2`,
   `ripgrep §43.0`, `ripgrep §47.1`)
5. **`-U` is a searcher permission, not the PCRE2 anchor mode**, and
   `--multiline-dotall` is inert without it. A PCRE2 pattern containing `\n`
   without `-U` fails silently. (`ripgrep §7.2`, `ripgrep §24` Invariants 3-4,
   `ripgrep §5.4`, `ripgrep §44.4`)
6. **`--regex-size-limit` and `--dfa-size-limit` do not bound PCRE2.**
   (`ripgrep §24` Invariant 5, `ripgrep §43.8`, `ripgrep §45.11`)
7. **PCRE2 10.47 is not a sandbox.** Pattern-level limits are defence in depth;
   untrusted patterns need a process boundary, and the safer request language is
   the default engine. (`ripgrep §35.2`, `ripgrep §40.2`-`§40.5`,
   `ripgrep §43.12`)
8. **`rg -r` never writes to a file; `ast-grep -U` does.** (`ripgrep §11.0`;
   `ast-grep §3.8`)
9. **An empty result is only evidence once you state the candidate set.**
   `--inspect summary` for ast-grep, `rg --files` / `--debug` for ripgrep.
   Binary and hidden files are skipped by default in both. (`ast-grep §3.13`;
   `ripgrep §13.0`, `ripgrep §22.0`-`§22.1`, `ripgrep §42.8`)
10. **`--no-messages` suppresses output, not error state** — always inspect the
    exit code. (`ripgrep §43.11`)
11. **A low `--pcre2-version` is not proof a construct is missing.** The string
    is the compile-time floor, not the linked runtime. Feature-probe with a
    control before rejecting a version-gated pattern. (`ripgrep §36.1`,
    `ripgrep §42.9`)
12. **Syntactic equality is not symbol identity, and a syntax match is not a
    confirmed semantic fact.** (`ast-grep §6.4`, `ast-grep §0.2`,
    `ast-grep §4.14`)
13. **The installed binary is the final authority on its own flags.** ast-grep's
    own documentation-drift ledger is `ast-grep §24.24`; confirm an unfamiliar
    flag with `--help`, and when the reference is silent, close the gap rather
    than routing around it. (`ast-grep §25.1`; `ripgrep §1.0`)

---

## Navigation hazards

Two that bite immediately; the full set is `REFERENCE.md §7`.

- **A bare `§N` is ambiguous.** Nine chapter numbers exist in both documents
  with different meanings. Always write `ast-grep §N.M` or `ripgrep §N.M`, and
  when reading someone else's citation work out which document it was written
  against before trusting it.
- **Each document renders its own table of contents as `##` headings,
  shape-identical to body subsections**, and the TOC titles have drifted from
  the body titles — 5 mismatches in ast-grep (`ast-grep §21`-`§25`), 13 in
  ripgrep. Cite the **body** heading. Use `just lib-outline`, which parses
  markdown and is fence-aware; a hand-rolled `grep '^#'` over-reports by 49
  lines in ast-grep and 4 in ripgrep.

---

## Project context: CodeFabric

Unlike the pure-library navigators, this one is project-anchored: the repository
runs both tools in its gates and ships custom extractors for them.

**Structural governance is a gate.** `sgconfig.yml` sets `ruleDirs: [rules]` and
`testConfigs: [{ testDir: rule-tests, snapshotDir: __snapshots__ }]`. Four
boundary rules live in `rules/` — `deltalake-boundary-only`,
`gix-boundary-only`, `no-framework-internal-contract-imports`,
`no-pyrefly-public-api` — each with a matching `rule-tests/*-test.yml`.
`just governance-scan` runs `ast-grep test --skip-snapshot-tests` then
`ast-grep scan` with five `--globs` exclusions for the generated trees, and is
wired into `just ci-fast`. Adding an invariant means adding a rule and its
negative-space tests (`ast-grep §15.8`, `ast-grep §22.19`), not a hand-tallied
search.

The existing rules are worth reading as exemplars: they combine `kind` with
`regex` under `any:`/`all:` and use file-level `files:`/`ignores:` to carve out
the boundary module — the pattern `ast-grep §8.12`-`§8.13` describes.

**Two custom outline extractors.** `tooling/ast-grep/outline/specs.yml` and
`library-ref.yml` map markdown headings to outline items and members, with shape
tests beside them. They are loaded through `scripts/spec-outline.sh` and
`scripts/lib-outline.sh` (`just spec-outline`, `just lib-outline`) because
**markdown is a built-in language and `outlineRules` is only a
`customLanguages` field** — there is no `sgconfig.yml` registration path, so
`--outline-rules` must be passed on every invocation. `specs.yml` maps
`## N. Section` to items and `### N.N` to members; `library-ref.yml` is
h1-rooted for the references, including the two this skill routes. Each script
refuses the other's tree rather than emit a misleading flat outline. If you
change either extractor, `ast-grep §14` is the field reference and the
`.test.sh` files pin the properties that matter.

**The two live search traps**, both of which silently shrink results:

- `.claude/` is hidden, so a default `rg` cannot see the skills — use
  `--hidden -g '!.git/**'` when skills, settings or any dotfile are in scope
  (`ripgrep §16.0`).
- `docs/library_ref/` is ~9 MB of prose that mentions nearly every identifier
  you will search for. Exclude it with `-g '!docs/library_ref/**'` unless the
  question is *about* the references — as it is whenever you are using this
  skill.

**Widening is scoped, never standing.** `rg -uu` and `ast-grep --no-ignore vcs`
both reach `.envrc.local`, which holds `CODEFABRIC_CPG_CAPABILITY_TOKEN`. Scope
the widening to a path (`ast-grep §22.5`, `ast-grep §22.6`).

**Rule of thumb:** a question about *what syntax has this shape* → ast-grep. A
question about *what bytes are present anywhere* → `rg`. A question about *what
this program means* → neither; escalate. Which capability, and where it is
specified: `REFERENCE.md`.
