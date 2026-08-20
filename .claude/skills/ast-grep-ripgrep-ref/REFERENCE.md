# ast-grep + ripgrep + PCRE2 — Reference Companion

The mechanical layer for `SKILL.md`. That file is the narrative router — three
engines, fourteen search jobs, the seam between the documents. **This file is
where things are**, by line, by flag, by construct, and by question. Come here
once you know *which* capability you need.

The two documents, at `docs/library_ref/`:

- **ast-grep** — `ast-grep_0.45.1_advanced_reference.md` · 8,297 lines ·
  **§0-§25** · 368 `## N.M` subsections · body prefix `# N) `
- **ripgrep** — `ripgrep_advanced_reference_15.2.0_pcre2_10.47.md` · 3,272 lines
  · **§0-§49** · 265 `## N.M` subsections · body prefix `# N) `, except **§0**
  which is `# ripgrep Advanced — 0) `

**Citation convention.** Citations are `ast-grep §N.M` / `ripgrep §N.M`,
matching each document's own numbering. **The alias is never optional** — §0-§25
exist in both documents and mean different things in each (REFERENCE §7 Rule 1).
This file's own sections are always written `REFERENCE §N` so they cannot be
mistaken for a document citation, and cross-references back into the router are
`SKILL §...`. Inside a table explicitly scoped to one document — the
`REFERENCE §2.1`-`§2.5` ast-grep blocks, `REFERENCE §2.6`-`§2.7` for ripgrep,
and the per-engine columns of REFERENCE §3 — a bare `§N.M` means that document.

**Line numbers appear only in REFERENCE §1**, because line numbers move when a
document is regenerated and section numbers do not: seek by line, cite by
section. Section titles sit next to every line number so a drifted section can
be re-found, and if a line looks wrong re-derive the table with
`just lib-outline <file> --view expanded --json=compact`.

| Section | What it is | Reach for it when |
|---|---|---|
| **§1** | Chapter maps with start lines, spans, and subsection ranges | you have a section number, or you need to know where to `Read` |
| **§2** | **Capability → location index** — every CLI flag, pattern construct, rule field and PCRE2 construct | you have a flag or a construct and need where it is *specified* |
| **§3** | The full search use-case matrix, ~90 rows across the fourteen jobs | you have a question and no flag name |
| **§4** | Cross-tool equivalence — what only one engine can express, and the substitute in the others | you know the construct but not whether this tool has it |
| **§5** | Six decision trees | you are choosing between instruments, engines, or scopes |
| **§6** | Exit codes and output contracts, both tools in one table | you are scripting either tool |
| **§7** | Navigation hazards | before searching either file |
| **§8** | Fourteen operating rules | before searching, and before making a claim |
| **§9** | CodeFabric context — governance rules, the outline extractors, the repo traps | before authoring a rule, extractor, or codemod here |

---

## §1 — Document maps

`Line` is the chapter's start line, 1-based, verified for `Read(offset, …)`.
`Lines` is the chapter's own span including its heading. `Subs` is the `## N.M`
range; **`—` means the chapter has no numbered subsections**, which is
deliberate, not a tooling failure. Every row was derived from
`just lib-outline --view expanded --json=compact`, which parses markdown and is
therefore fence-aware — see REFERENCE §7 Rule 4 for why a `grep '^#'` is not.

### §1.1 `ast-grep_0.45.1_advanced_reference.md` — §0-§25

Front matter 1-68 (`## Version / source anchors` at 9, the `[S1]`-`[S26]` URL
anchor list at 29-56, the documentation-drift policy at 58-67).
`# Comprehensive documentation map` — the document's own TOC, rendered as
`## N)` — occupies 69-99. Chapter bodies start at **100**.
`# Primary official references used for this document` is 8210-8278 and
`# Closing operational summary` is 8279-8297.

| § | Line | Lines | Subs | Title |
|---|---:|---:|---|---|
| **§0** | 100 | 119 | 0.0-0.6 | Mental model, capability boundaries, and chooser matrix |
| **§1** | 219 | 119 | 1.0-1.4 | Installation, version verification, executable naming, and shell integration |
| **§2** | 338 | 125 | 2.0-2.7 | CLI topology and global operational conventions |
| **§3** | 463 | 342 | 3.0-3.16 | `ast-grep run` — ad-hoc structural search and one-off rewrite |
| **§4** | 805 | 294 | 4.0-4.14 | `ast-grep scan` — repository rules, linting, CI, and batch codemods |
| **§5** | 1099 | 391 | 5.0-5.16 | `ast-grep outline` — low-token syntax outline for coding agents |
| **§6** | 1490 | 307 | 6.0-6.13 | Pattern language — Tree-sitter parsing, metavariables, selectors, strictness |
| **§7** | 1797 | 503 | 7.0-7.19 | Rule object — atomic, relational, composite, and ESQuery matching |
| **§8** | 2300 | 430 | 8.0-8.17 | Lint-rule YAML — complete finding/reporting/patching/file-selection surface |
| **§9** | 2730 | 233 | 9.0-9.10 | Utility rules — local/global reuse and parameterized utilities |
| **§10** | 2963 | 240 | 10.0-10.11 | Rewrite system — CLI rewrite, `fix`, range expansion, indentation |
| **§11** | 3203 | 274 | 11.0-11.11 | Transformations and rewriters — generated metavariables and recursive rewrites |
| **§12** | 3477 | 143 | 12.0-12.8 | Project configuration — `sgconfig.yml`, discovery, directories, globs |
| **§13** | 3620 | 234 | 13.0-13.9 | Languages — built-ins, aliases, language globs, custom parsers, injections |
| **§14** | 3854 | 288 | 14.0-14.12 | Outline extraction rules — custom items/members and JSON structure contracts |
| **§15** | 4142 | 199 | 15.0-15.11 | Rule testing — valid/invalid cases, snapshots, filtering, test configuration |
| **§16** | 4341 | 195 | 16.0-16.9 | Severity, suppressions, runtime overrides, and CI gating |
| **§17** | 4536 | 228 | 17.0-17.11 | Output contracts — terminal, JSON, SARIF, GitHub annotations, coordinates |
| **§18** | 4764 | 126 | 18.0-18.8 | LSP/editor integration — diagnostics, code actions, rule reload behavior |
| **§19** | 4890 | 487 | 19.0-19.6 | Programmatic APIs — JavaScript/N-API, Python/PyO3, Rust core, WASM |
| **§20** | 5377 | 232 | 20.0-20.13 | Traversal, performance, pruning, concurrency, and scaling |
| **§21** | 5609 | 683 | 21.0-21.25 | Debugging and troubleshooting — a deterministic triage playbook |
| **§22** | 6292 | 369 | 22.0-22.20 | Correctness, safety, trust boundaries, and destructive-operation policy |
| **§23** | 6661 | 670 | 23.0-23.26 | Agentic coding workflows — using ast-grep as a structural tool layer |
| **§24** | 7331 | 519 | 24.0-24.27 | Capability matrices and quick-reference appendices |
| **§25** | 7850 | 360 | 25.0-25.13 | Upgrade policy, source-of-truth hierarchy, and final LLM-agent execution playbook |

**Where the fastest answers are.** `ast-grep §24` (7331-7849) is 28
quick-reference matrices and is usually quicker than the chapter body: `24.2`
`run` options (7361) · `24.3` `outline` options (7385) · `24.4` `scan` control
plane (7406) · `24.5` query-surface choice (7425) · `24.6` strictness (7440) ·
`24.7` metavariables (7453) · `24.8` atomic rules (7465) · `24.9` relational
(7477) · `24.10` composite (7494) · `24.11` ESQuery selectors (7503) · `24.13`
rule top-level field catalog (7540) · `24.14` `sgconfig.yml` catalog (7565) ·
`24.15` outline extractor fields (7590) · `24.16` transforms (7623) · `24.18`
output formats (7664) · `24.19` JSON coordinate contract (7675) · `24.22`
**syntax vs semantic capability** (7712) · `24.24` **documentation-drift
ledger** (7748) · `24.25` version timeline (7793). The daily command card is
`25.9` (8053).

**Most chapters close with a fenced `### Agent checklist`** — 789, 1082, 1474,
2283, 2713, 2948, 3462, 3839, 4125, 4327, 4523, 4877, 5362, 6275, 6644, 7315,
and the final one at 8166. Chapters 6, 10, 17 and 20 have none; §10 ends with
`### Agent anti-patterns` (3191) and §20 with `### Anti-patterns` (5597). Load
the closing block before authoring a rule or codemod.

### §1.2 `ripgrep_advanced_reference_15.2.0_pcre2_10.47.md` — §0-§49

Front matter 1-32 (`## Version / source anchors` at 3 — the release dates and
the CVE boundary live here and no chapter repeats them; `## Feature inventory`
at 13). `# Comprehensive documentation map` occupies 33-87. Chapter bodies start
at **88**. `# 49) Source index` is 3216-3246 and resolves the inline `[RG-*]` /
`[PCRE2-*]` citation tags. The document closes with an unnumbered
`# Final agent invariants` at **3247-3272** — twelve invariants at 3249, 3251,
3253, 3255, 3257, 3259, 3261, 3263, 3265, 3267, 3269, 3271; it is the densest
page in the file.

**§0-§23 are ripgrep. §24-§40 are PCRE2. §41-§48 are agent-facing.**

| § | Line | Lines | Subs | Title |
|---|---:|---:|---|---|
| **§0** | 88 | 65 | 0.0-0.2 | Scope, release stance, and mental model |
| **§1** | 153 | 52 | 1.0-1.2 | Installation, release binaries, PCRE2 verification, and source builds |
| **§2** | 205 | 32 | 2.0-2.2 | Execution pipeline and performance model |
| **§3** | 237 | 51 | 3.0-3.3 | CLI topology, argument parsing, precedence, and configuration |
| **§4** | 288 | 47 | 4.0-4.4 | Pattern sources and multi-pattern semantics |
| **§5** | 335 | 58 | 5.0-5.4 | Regex engine selection: default vs PCRE2 vs auto |
| **§6** | 393 | 36 | 6.0-6.3 | Literal search, case handling, Unicode, and fixed strings |
| **§7** | 429 | 52 | 7.0-7.5 | Multiline search, dotall, anchors, CRLF, and NUL data |
| **§8** | 481 | 30 | 8.0-8.2 | Whole-word / whole-line wrappers |
| **§9** | 511 | 71 | 9.0-9.5 | Standard output modes, context, headings, color, and only-matching |
| **§10** | 582 | 65 | 10.0-10.4 | Counts, file-list modes, quiet mode, and exit status |
| **§11** | 647 | 36 | 11.0-11.3 | Replacements and capture interpolation |
| **§12** | 683 | 44 | 12.0-12.4 | Recursive traversal, roots, depth, symlinks, and filesystem boundaries |
| **§13** | 727 | 43 | 13.0-13.3 | Automatic filtering and the ignore stack |
| **§14** | 770 | 29 | 14.1-14.4 | ripgrep 15.2 ignore/traversal changes |
| **§15** | 799 | 60 | 15.0-15.5 | Manual filtering: globs, file types, precedence, customization |
| **§16** | 859 | 26 | 16.0-16.2 | Hidden, binary, text, max-filesize, and terminal safety |
| **§17** | 885 | 27 | 17.0-17.3 | Encodings, transcoding, BOMs, raw bytes, and offsets |
| **§18** | 912 | 24 | 18.0-18.2 | Compressed files and preprocessors |
| **§19** | 936 | 75 | 19.0-19.4 | I/O strategy, mmap, threads, buffering, sorting, and performance cliffs |
| **§20** | 1011 | 46 | 20.0-20.4 | JSON output as an agent/programmatic query interface |
| **§21** | 1057 | 39 | 21.0-21.3 | Configuration files, environment, aliases, and override design |
| **§22** | 1096 | 46 | 22.0-22.4 | Debugging search space and execution |
| **§23** | 1142 | 32 | 23.0-23.3 | Shell scripting, filename transport, and deterministic automation |
| **§24** | 1174 | 101 | 24.0-24.4 | PCRE2 integration architecture inside ripgrep 15.2 |
| **§25** | 1275 | 97 | 25.0-25.5 | PCRE2 10.47 syntax and feature map |
| **§26** | 1372 | 52 | 26.0-26.5 | Lookahead, lookbehind, and assertion design |
| **§27** | 1424 | 46 | 27.0-27.3 | Variable-length lookbehind in PCRE2 10.47 |
| **§28** | 1470 | 58 | 28.0-28.4 | Backreferences, named groups, duplicate names, and capture semantics |
| **§29** | 1528 | 58 | 29.0-29.3 | Atomic groups, possessive quantifiers, branch reset, and conditionals |
| **§30** | 1586 | 82 | 30.0-30.4 | Subroutines, recursion, recursion tests, and recursive patterns |
| **§31** | 1668 | 54 | 31.0-31.5 | Backtracking control verbs, `\K`, `\G`, `\R`, and advanced control |
| **§32** | 1722 | 68 | 32.0-32.5 | Unicode/UCP, properties, script runs, case folding, ASCII restrictions |
| **§33** | 1790 | 40 | 33.0-33.2 | PCRE2 10.47 extended character classes and set algebra |
| **§34** | 1830 | 60 | 34.0-34.5 | PCRE2 10.47 scan-substring assertions |
| **§35** | 1890 | 75 | 35.0-35.4 | Pattern-level resource limits and safety hardening |
| **§36** | 1965 | 119 | 36.0-36.6 | JIT behavior, unsupported JIT features, and performance design |
| **§37** | 2084 | 87 | 37.0-37.6 | PCRE2 newline, BSR, multiline, dotall, CRLF, and ripgrep interactions |
| **§38** | 2171 | 37 | 38.0-38.2 | PCRE2 replacement API vs ripgrep `--replace` |
| **§39** | 2208 | 81 | 39.0-39.7 | PCRE2 library capabilities not exposed by ripgrep |
| **§40** | 2289 | 50 | 40.0-40.6 | PCRE2 10.47 security posture and untrusted-pattern policy |
| **§41** | 2339 | 153 | 41.0-41.16 | High-value code-search recipes for programming agents |
| **§42** | 2492 | 159 | 42.0-42.9 | Evidence-oriented search patterns for LLM coding agents |
| **§43** | 2651 | 95 | 43.0-43.12 | Performance engineering and anti-pattern inventory |
| **§44** | 2746 | 111 | 44.0-44.8 | Upgrade guide: outdated ripgrep references → 15.2.0 |
| **§45** | 2857 | 157 | 45.1-45.11 | CLI flag-family lookup matrix |
| **§46** | 3014 | 71 | 46.1-46.2 | PCRE2 10.47 pattern-feature lookup matrix |
| **§47** | 3085 | 44 | 47.1-47.1 | Engine-selection and compatibility matrix |
| **§48** | 3129 | 87 | 48.0-48.5 | Production / agent checklists |
| **§49** | 3216 | 31 | — | Source index |

**Where the fastest answers are.** `ripgrep §45` (2857-3013) is eleven
flag-family tables covering essentially the whole CLI: `45.1` engine and pattern
language (2861) · `45.2` pattern sources (2880) · `45.3` traversal and search
space (2892) · `45.4` ignore controls (2909) · `45.5` globs and types (2923) ·
`45.6` binary/encoding/input (2936) · `45.7` standard output (2951) · `45.8`
summary and existence modes (2978) · `45.9` machine output and filename safety
(2989) · `45.10` diagnostics (2998) · `45.11` **the default-engine-only tuning
caveat** (3008). `ripgrep §46` (3014) is a 39-row PCRE2-feature ×
works-via-`rg -P` matrix, opening with an unnumbered table at 3016-3053.
`ripgrep §47` (3085) is a 22-row engine-compatibility matrix at 3087-3111 plus
the five-step escalation algorithm at `47.1` (3112-3128).

**Chapters are short** — 24 to 159 lines. Read them whole rather than seeking a
subsection.

**Numbering irregularities.** `ripgrep §14` and `§45` have no `.0` subsection.
`§46` and `§47` open with an unnumbered matrix before their first `.1`. `§24.0`
contains five `### Invariant N` sub-blocks (1200, 1204, 1215, 1229, 1233) that
carry the load-bearing PCRE2 wiring facts, `§36.1` carries the
version-under-report caveat as a `###` sub-block (1993), and `§39.7` contains
four `###` sub-blocks (2271, 2275, 2279, 2283).

---

## §2 — Capability → location index

Where a flag, construct or field is *specified*, not merely mentioned. **Use
this instead of grep**: both documents use their own flag names constantly in
examples, so a literal search returns dozens of hits with no signal about which
one is the definition, and each document's matrix chapters mention nearly every
name once, so a grep's last hits are almost always a matrix rather than the
definition.

`+` in the `Also` column marks a second location worth reading. Hazards are
**bolded inline**.

### §2.1 ast-grep — subcommands and traversal

| Symbol | Defined at | Also |
|---|---|---|
| `ast-grep run` (default subcommand) | **§3.0** | equivalence with bare form §2.1 · matrix §24.2 |
| `ast-grep scan` | **§4.0** | rule-loading modes §4.1 · matrix §24.4 |
| `ast-grep outline` | **§5.1** | matrix §24.3 · **exit 0 on empty §5.16** |
| `ast-grep test` | **§15.1** | snapshots §15.4 · `--update-all` §15.4 · `--skip-snapshot-tests` §15.3 · `--include-off` §15.6 |
| `ast-grep new` | **§12.8** | project scaffold §12.1 |
| `ast-grep lsp` | **§18.0** | setup §18.1 · **not a semantic server §18.8** |
| `ast-grep completions` | **§1.2** | — |
| `--globs` | **§2.4** | **later flag wins** · outline §5.10 · scan §4.3 |
| `--no-ignore <layer>` | **§2.3** | six layers: `hidden` `dot` `exclude` `global` `parent` `vcs` · **reaches secrets §22.5** |
| `--follow` | **§2.2** | **symlink loop/traversal risk §22.4** |
| `--stdin` | **§2.5** | **hangs waiting for EOF in a TTY** · requires `--lang` |
| `-j/--threads` | **§3.9** | `0` = heuristic · perf §20.8 · **more is not always faster** |
| `-l/--lang` | **§3.4** | extension inference §2.2 · `languageGlobs` §12.6 · wrong-language triage §21.11 |

### §2.2 ast-grep — matching, output, rewriting

| Symbol | Defined at | Also |
|---|---|---|
| `-p/--pattern` | **§3.2** | patterns-are-code §6.0 · vs `-k` §3.3 |
| `-k/--kind` | **§3.3** | ESQuery selectors §7.18 · matrix §24.11 · triage §21.8 |
| `--selector` | **§3.5** | YAML form §6.9 · pattern objects §6.8 |
| `--strictness` | **§3.7** | six modes deep-dive §6.10 · matrix §24.6 · **`template` missing from some upstream pages §24.24** |
| `--debug-query=pattern\|ast\|cst\|sexp` | **§3.6** | **requires explicit `--lang`** · no-match triage §21.2 |
| `--inspect summary\|entity` | **§3.13** | scan §4.11 · **writes to stderr** · `scannedFileCount`/`skippedFileCount` |
| `--json` / `=stream` / `=compact` | **§3.12** | **style needs `=` §2.7** · schema §17.1 · scan extension §17.2 · scaling §20.9 |
| `-A`/`-B`/`-C`, `--heading` | **§3.10** | scan §4.10 · `-C` conflicts with separate before/after |
| `--color` | **§3.11** | **never parse coloured output §2.6** |
| `-r/--rewrite`, `-i`, `-U` | **§3.8** | **`-U` writes files** · lifecycle §4.13 · VCS guard §22.2 |
| `--rule` / `-r` (scan) | **§4.1** Mode B | conflicts with `--config` |
| `--inline-rules` | **§4.1** Mode C | **shell interpolation hazard §22.8** |
| `--filter <regex>` | **§4.1** Mode D | test filtering §15.5 |
| `--config` | **§4.2** | monorepo discovery §12.7 |
| `--report-style rich\|medium\|short` | **§4.4** | human-output decision §24.20 |
| `--format github\|sarif` | **§4.5** | SARIF §17.10 · GitHub §17.9 |
| `--include-metadata` | **§4.6** | **requires JSON mode** · `metadata` field §8.15 |
| `--error=` `--warning=` `--info=` `--hint=` `--off=` | **§4.7** | severity is deployment policy §16.0 · effective severity §16.8 |
| exit behaviour of `scan` | **§4.8** | **1 only on error-severity** · CI triage §21.19 |

### §2.3 ast-grep — `outline` flags

| Symbol | Defined at | Also |
|---|---|---|
| `--items structure\|exports\|imports\|all\|auto` | **§5.3** | **export status is syntax-only** |
| `--view names\|signatures\|digest\|expanded\|auto` | **§5.4** | token-budget rule §5.4 · text shapes §5.11 |
| `--match <regex>` | **§5.5** | **filters items, never members** · Rust regex, case-sensitive |
| `--type <symbolTypes>` | **§5.6** | LSP `SymbolKind` vocabulary · **top-level only** |
| `--pub-members` | **§5.7** | **member is public when the extractor has no `isPublic` rule** |
| `--outline-rules`, `--no-default-outline-rules` | **§5.9** | extractor fields §14 · matrix §24.15 |
| input-type defaults | **§5.2** | dir → `exports`/`names`; file/stdin → `structure`/`digest` |
| outline JSON schema | **§5.12** | **zero-based positions** · differs from run/scan §17.8 |

### §2.4 ast-grep — pattern constructs

| Construct | Defined at | Also |
|---|---|---|
| patterns are parsed as code | **§6.0** | compilation model §6.1 |
| `$NAME` — one named node | **§6.2** | matrix §24.7 · **lowercase names are not metavariables** |
| `$$$` / `$$$NAME` — zero or more | **§6.3** | **cannot be the root pattern** (0.44+) · consumption/anchoring §6.7 · triage §21.6 |
| repeated `$A` — equality constraint | **§6.4** | **syntactic, never symbol identity** · triage §21.5 |
| `$_` / `$_NAME` — non-capturing | **§6.5** | — |
| `$$VAR` — unnamed node/operator | **§6.6** | **`$OP` fails here** · use `--debug-query=cst` |
| pattern object `context` + `selector` + `strictness` | **§6.8** | selector semantics §6.9 · CLI `--selector` §3.5 |
| `cst` `smart` `ast` `relaxed` `signature` `template` | **§6.10** | CLI §3.7 · matrix §24.6 |
| error-recovery is not a contract | **§6.11** | anti-patterns §6.13 |

### §2.5 ast-grep — rule object and rule file

| Field | Defined at | Also |
|---|---|---|
| the three field groups | **§7.0** | matrices §24.8-§24.10 |
| implicit AND vs explicit `all` | **§7.1** | **use `all` when a later sub-rule consumes a capture** |
| `pattern` | **§7.2** | §6.8 for the object form |
| `kind` (exact or ESQuery) | **§7.3** | selectors §7.18 |
| `regex` | **§7.4** | **Rust engine, whole node text, no lookaround or backreferences** · placement §20.4 · DoS posture §22.9 |
| `nthChild` | **§7.5** | one-based, **named siblings only**, `ofRule`, `reverse` |
| `range` | **§7.6** | zero-based, end-exclusive · coordinate safety §22.16 |
| relational model | **§7.7** | `inside` §7.10 · `has` §7.11 · `precedes`/`follows` §7.12 |
| `stopBy: neighbor\|end\|<rule>` | **§7.8** | **`stopBy: end` everywhere is an anti-pattern §4.14** · depth cost §20.3 |
| `field` | **§7.9** | for `inside` and `has` |
| `all` `any` `not` `matches` | **§7.13**-**§7.16** | one-node-at-a-time semantics §7.17 |
| ESQuery `>` `+` `~`, `:has` `:not` `:is` `:nth-child` `:nth-last-child` | **§7.18** | limitations at 2239 · when it beats YAML at 2247 · matrix §24.11 |
| `constraints` | **§8.6** | **evaluated after the match — the negation footgun (2474-2491)** |
| `utils` (local) / `utilDirs` (global) | **§8.7**, **§9.1**-**§9.2** | parameterized §9.4-§9.5 · shadowing §9.6 · **resolution triage §21.17** |
| `message` `severity` `note` `labels` `url` `metadata` | **§8.8**-**§8.15** | **`labels` range requirement (2593)** |
| `files` / `ignores` | **§8.12**-**§8.13** | **not filesystem discovery (2650)** |
| multiple rule documents in one file | **§8.16** | `---` separated |
| `fix` (string) / `FixConfig` | **§10.2**, **§10.4** | expansion §10.5 · indentation §10.7 · **triage §21.15** |
| `transform`: `replace` `substring` `convert` | **§11.3**-**§11.5** | sequential §11.6 · `joinBy` §11.10 · matrix §24.16 |
| `rewriters` / `rewrite` | **§11.7**-**§11.9** | **termination reasoning §22.10** |
| `sgconfig.yml`: `ruleDirs` `testConfigs` `utilDirs` `languageGlobs` `customLanguages` | **§12.3**-**§12.6**, **§13.3** | **untrusted config is code execution §22.3** · matrix §24.14 |
| outline extractor fields: `symbolType` `name` `signature` `isImport` `isExported` `parentRuleIds` `isPublic` | **§14.2**-**§14.8** | design rules §14.11 · matrix §24.15 |
| `languageInjections` | **§13.7** | built-in behaviour §13.8 · **correctness risks §13.9** · triage §21.12 |
| `expandoChar` | **§13.6** | custom parsers §13.5 |

### §2.6 ripgrep — flags

| Flag | Defined at | Also |
|---|---|---|
| `--engine=default\|pcre2\|auto`, `-P/--pcre2` | **§5.0**-**§5.3** | **global to the invocation §4.4** · escalation §47.1 · matrix §47 · `--auto-hybrid-regex` deprecated §44.2 |
| `-F/--fixed-strings` | **§6.0** | **applies to the whole pattern set** · under PCRE2 §24.2 |
| `-s` / `-i` / `-S/--smart-case` | **§6.1** | last-flag-wins §3.1 · config defaults §21.1 |
| `--no-unicode` | **§6.3** | Unicode defaults §6.2 · **`--pcre2-unicode` aliases deprecated §44.3** |
| `-w/--word-regexp`, `-x/--line-regexp` | **§8.0**-**§8.1** | **do not hand-anchor as well §8.2** · `-w` under PCRE2 §24.3 |
| `-e/--regexp`, `-f/--file` | **§4.1**-**§4.2** | **positional args become paths §4.1** · **empty line in a `-f` file matches everywhere §4.2** · `-f -` §4.3 |
| `-U/--multiline`, `--multiline-dotall` | **§7.1**-**§7.2** | **dotall inert without `-U`** · **not the PCRE2 anchor switch §24 Invariant 3** · memory §37.6 |
| `--crlf`, `--null-data` | **§7.4**, **§7.5** | PCRE2 newline interaction §37.1-§37.3 |
| `-A` / `-B` / `-C` | **§9.1** | separators §9.5 |
| `--context-separator`, `--no-context-separator` | **§9.5** | **empty string still prints a blank line; only `--no-context-separator` removes it** |
| `--field-context-separator`, `--field-match-separator` | **§9.5** | **context lines use `-`, match lines use `:` — a parser splitting on `:` misreads context** |
| `-m/--max-count` | **§10.4** | **`-c` reports the *capped* count §10.4** · `-A`/`-C` can print past the cap · exit status unaffected |
| `-o/--only-matching` | **§9.2** | evidence doctrine §42.4 |
| `-n`, `--column`, `-b/--byte-offset`, `--heading`, `-H` | **§9.0** | flag table §45.7 |
| `-p/--pretty`, `--color` | **§9.3**-**§9.4** | **never parse coloured output §43.4** |
| `-c/--count`, `--count-matches` | **§10.0** | flag table §45.8 |
| `-l/--files-with-matches`, `--files-without-match`, `--files` | **§10.1** | candidate set §22.0 · JSON vs `-l` §20.4 |
| `-q/--quiet` | **§10.2** | — |
| exit statuses 0/1/2 | **§10.3** | **do not treat 1 as failure** · scripting §23.2 |
| `-r/--replace`, `$1` / `$name` / `${name}` | **§11.0**-**§11.1** | **output-only, never writes** · PCRE2 pattern + rg printer §11.2, §38.1 |
| `--max-depth`, `-L`, one-filesystem | **§12.1**-**§12.3** | explicit paths override §12.4 |
| the ignore precedence stack | **§13.1** | **nested ignore files override ancestors (705)** |
| `-u` / `-uu` / `-uuu` | **§13.2** | **anti-pattern in normal search §43.3** |
| `--no-ignore-vcs\|-dot\|-parent\|-global\|-exclude\|-files\|-messages` | **§13.3** | full table §45.4 |
| `GIT_CONFIG_GLOBAL` / `GIT_CONFIG_SYSTEM` | **§14.1** | 15.2 change |
| `-g/--glob`, `--iglob`, `--glob-case-insensitive` | **§15.0**-**§15.2** | **`-g dir` ≠ `-g 'dir/**'` §15.1** |
| `-t` / `-T` / `--type-list` / `--type-add` / `--type-clear` | **§15.3**-**§15.4** | **`--type-add` does not persist (798)** · debugging §22.3 |
| `--hidden` | **§16.0** | **pulls in `.git` unless excluded** |
| `--binary` vs `-a/--text` | **§16.1** | binary skipped by default §13.0 |
| `--max-filesize` | **§16.2** | — |
| `-E/--encoding`, `-E none` | **§17.1**-**§17.2** | **transcoded offsets are not file offsets §17.3** |
| `-z/--search-zip`, `--pre`, `--pre-glob` | **§18.0**-**§18.1** | **a preprocessor is arbitrary code — allowlist it §18.2** |
| `--mmap` / `--no-mmap` | **§19.0** | **rotating/truncated files** |
| `-j/--threads` | **§19.1** | `0` = heuristic default · single file/stdin is single-threaded regardless |
| `--sort`, `--sortr` (`none\|path\|modified\|accessed\|created`) | **§19.2** | **every value but `none` forces single-threaded** · `path` orders directory entries, not path strings · **last-flag-wins, despite `--help`** · `created` errors out where unsupported |
| `--line-buffered`, `--block-buffered` | **§19.3** | match density §19.4 |
| `--json` | **§20.0** | arbitrary bytes §20.1 · offsets §20.2 · **agent contract §20.3** |
| `--vimgrep`, `--null`/`-0`, `--path-separator` | **§45.9** | NUL transport §23.1 |
| `RIPGREP_CONFIG_PATH` | **§21.0** | good defaults §21.1 · **override-friendly design §21.2** |
| `--debug`, `--trace` | **§22.1**-**§22.2** | candidate set §22.0 · engine capability §22.4 |
| `--no-messages` | **§43.11** | **suppresses output, not error state — check the exit code** |
| `--regex-size-limit`, `--dfa-size-limit` | **§45.11** | **default engine only; do not bound PCRE2 — §24 Invariant 5, §43.8** |
| `--pcre2-version` | **§36.1** | capability gate §42.6 · **version not inferable from rg's §44.8** · **the string can be LOWER than the linked library — a low report is not proof of absence §36.1, §42.9** |

### §2.7 PCRE2 10.47 constructs (reachable only through `rg -P`)

| Construct | Defined at | Also |
|---|---|---|
| core atoms, quantifiers, groups, inline options | **§25.0**-**§25.2** | boundary assertions §25.3 · 10.47 delta §25.5 · matrix §46 |
| `(?=…)` / `(?!…)` lookahead | **§26.0**-**§26.1** | conjunction recipe §41.1 · assertion design §26.4 |
| `(?<=…)` / `(?<!…)` lookbehind | **§26.2**-**§26.3** | **bounded only; 255-char cap with no rg setter §27.1-§27.2, §39.2** |
| `\1`, `\k<name>`, `(?P<n>…)`, duplicate names | **§28.0**-**§28.2** | pattern vs replacement capture §28.3 · **backtracking risk §28.4** · recipes §41.5-§41.6 |
| `(?>…)` atomic group | **§29.0** | assertion atomicity §26.5 |
| `*+` `++` `?+` possessive quantifiers | **§29.1** | catastrophic families §36.6 |
| `(?\|…)` branch reset | **§29.2** | recipe §41.15 |
| `(?(…)…\|…)` conditionals | **§29.3** | — |
| `(?R)`, `(?&name)` recursion and subroutines | **§30.0** | **not a parser replacement §30.1** · **10.47 returned captures §30.2, version gate at 1562** · resource risk §30.4 |
| `\K` | **§31.0** | as a lookbehind substitute §27.3 · recipe §41.3 |
| `(*SKIP)(*F)` | **§31.1** | recipe §41.7 |
| `(*PRUNE)` `(*COMMIT)` `(*THEN)` `(*ACCEPT)` `(*FAIL)` | **§31.2**-**§31.3** | matrix §46 |
| `\G`, `\R` | **§31.4**-**§31.5** | BSR and newline interaction §37.3 |
| `\p{…}` `\P{…}`, UCP, script runs `(*script_run:…)` | **§32.0**, **§32.4** | **UTF+UCP are on by default under `rg -P` §24 Invariant 2** · Unicode 16 data §32.1 · `\w` semantics §32.3 · ASCII restrictions §32.5 |
| `(?[…])` Perl extended classes / set algebra | **§33.0** | **UTS#18 `[A&&[B]]` needs a compile option ripgrep does not expose §33.1, §39.1** |
| `(*scs:…)` scan-substring assertions | **§34.0**-**§34.1** | when it helps §34.2 · **overkill §34.3** · JIT §34.4 · **security §34.5** |
| `(*LIMIT_MATCH=)` `(*LIMIT_DEPTH=)` `(*LIMIT_HEAP=)` | **§35.0**-**§35.1** | **lower only, never raise** · **not a sandbox §35.2** |
| `(*NO_JIT)` and optimization-control verbs | **§35.3**-**§35.4** | JIT policy §36.0 |
| `(?x)` extended mode, escape cheat-sheet | **§46.2**, **§46.1** | **bare `\x` is no longer NUL shorthand (2934)** |
| capabilities with no ripgrep flag at all | **§39** | callouts §39.5 · substitution API §39.6 · `pcre2_next_match()` §39.7 · **decision rule at 2283** |
| **proving a version-gated construct is actually available** | **§42.9** | feature-probe with a control · **`--pcre2-version` under-reports §36.1** · metadata §42.7 |

---

## §3 — The search use-case matrix

The exhaustive form of `SKILL §`Search jobs. One row per question shape. `—`
means the engine cannot express it at all; a parenthesised entry means it can,
badly. The `Prefer` column is the decision, not a summary, and always carries
the alias because it names both engines.

### §3.1 Inventory and navigation

| Question | ast-grep | ripgrep / PCRE2 | Prefer, and why |
|---|---|---|---|
| What is in this directory? | `outline <dir>` → `exports`/`names` §5.2 | `rg --files` §10.1 | **outline** — names, not paths |
| What does this file define? | `outline <file>` → `structure`/`digest` §5.2 | — | **outline**; do not read the body |
| What does this module import? | `outline --items imports` §5.3 | `rg '^\s*(use\|import) '` | **outline** for real import nodes; `rg` for generated or dynamic imports |
| What are this type's methods and fields? | `outline --match '^T$' --view expanded` §5.4-§5.5 | — | **outline** — but `--match` selects the *item*, never a member |
| Only structs and enums, please | `outline --type struct,enum` §5.6 | — | **outline**; `--type` is top-level only |
| Only the public members | `outline --view expanded --pub-members` §5.7 | — | **outline**, knowing "public" defaults to true when the extractor is silent |
| What changed structurally in my diff? | `git diff --name-only -z \| xargs -0 ast-grep outline` §5.13 | — | **outline**, NUL-delimited |
| How many files would be searched? | `run … --inspect summary` §3.13 | `rg --files \| wc -l` §22.0 | either — this is the coverage envelope |
| Map a markdown corpus | `outline --outline-rules …` §5.9, §14 | `rg '^#'` | **outline** — `rg` cannot tell a heading from a fenced `#` comment |

### §3.2 Declarations

| Question | ast-grep | ripgrep / PCRE2 | Prefer, and why |
|---|---|---|---|
| Where is `Name` defined? | `outline --match '^Name$'` §5.5, or `-p`/`-k` §3.2-§3.3 | `rg -w 'Name'` §8.0; recipe §41.0 | **outline** if you know the subtree; **`rg -w`** across languages |
| Every function declaration | `-k function_item` §3.3 | `rg '^\s*fn \w+'` | **ast-grep** — no false hits in comments or strings |
| Every impl of a trait | `-p 'impl Trait for $T { $$$ }'` §3.2 | — | **ast-grep** — `rg` cannot bound the block |
| Every subclass of a base | `-p 'class $C(Base): $$$'` §3.2 | — | **ast-grep** |
| A declaration in a language with no grammar | — | `rg` §6.0 | **rg**; ast-grep has no parser to offer |
| A declaration built at runtime | — | `rg` §41.4 | **rg** — no AST node exists |
| A key in TOML/JSON/YAML | `-p` with that language §3.4 | `rg` §41.12 | **ast-grep** when nesting matters; **rg** for a flat lookup |
| Extract just the declared name | `--json` captures §17.6 | `rg -o -P 'class \K\w+'` §41.3, §31.0 | **PCRE2 `\K`** for a one-liner; **ast-grep** JSON when it feeds a tool |

### §3.3 Usages and call sites

| Question | ast-grep | ripgrep / PCRE2 | Prefer, and why |
|---|---|---|---|
| Every call to a free function | `-p 'foo($$$A)'` §6.3 | `rg -w 'foo'` | **ast-grep** — `rg` also matches the definition and the docs |
| Every method call on any receiver | `-p '$R.foo($$$A)'` §6.2 | — | **ast-grep** |
| Calls with exactly one argument | `-p 'foo($X)'` §6.2 | — | **ast-grep**; `$X` is one node, deliberately |
| Calls with zero arguments | `-p 'foo()'` §6.2 | — | **ast-grep** |
| A name used as a value, not called | `-k identifier` + `not: { inside: call_expression }` §7.15 | (`rg -P 'foo(?!\s*\()'` §26.1) | **ast-grep** — the negative lookahead cannot see a multi-line call |
| Callers of a macro-generated function | — | `rg` §42.8 | **rg**, and declare it as residue |
| A symbol referenced from another language | — | `rg -t py -t toml` §15.3 | **rg** — structure stops at the language boundary |
| A string-keyed registry entry | — | `rg -F '"key"'` §6.0 | **rg**; no AST edge exists |
| Is this symbol dead? | tier 2 only | tier 3 only | **neither alone** — see `_shared/code-intelligence.md` |

### §3.4 Structural context

| Question | ast-grep | ripgrep / PCRE2 | Prefer, and why |
|---|---|---|---|
| Calls *inside* a given impl/class | `inside:` + `stopBy: end` §7.10, §7.8 | — | **ast-grep** — text search has no ancestor |
| A declaration that *has* a given child | `has:` (+ `field:`) §7.11, §7.9 | — | **ast-grep** |
| The statement before/after another | `precedes` / `follows` §7.12 | (`-U` with a two-line pattern §7.1) | **ast-grep** — formatting-independent |
| The nth argument or field | `nthChild` §7.5 or `:nth-child` §7.18 | — | **ast-grep**; one-based, named siblings |
| The last variant of an enum | `nthChild: { position: 1, reverse: true }` §7.5 | — | **ast-grep** |
| Every decorated definition | `has: { kind: decorator }` §7.11 | `rg -B1 '^\s*def '` §9.1 | **ast-grep** — context lines are a guess |
| A node kind relationship, tersely | `-k 'a > b'`, `:has`, `:is` §7.18 | — | **ast-grep**; often shorter than the YAML |
| Two conditions where one captures for the other | explicit `all:` §7.1 | — | **ast-grep**, and field order is otherwise unspecified |
| Balanced delimiters | `has`/`inside` — a real parse | (`(?R)` §30.0) | **ast-grep** — `ripgrep §30.1` says recursion is not a parser |

### §3.5 Negation

| Question | ast-grep | ripgrep / PCRE2 | Prefer, and why |
|---|---|---|---|
| X not followed by Y | `not:` §7.15, `:not` §7.18 | `(?!…)` §26.1 | **ast-grep** when Y is a node; **PCRE2** when Y is bytes on the same line |
| X not preceded by Y | `follows:` + `not:` §7.12, §7.15 | `(?<=…)`/`(?<!…)` §26.2-§26.3 | **PCRE2**, but the lookbehind must be bounded (`ripgrep §27.1`) |
| Lines with A but not B | `all: [ …, not: … ]` §7.13 | `rg 'A' \| rg -v 'B'`, or `(?!.*B)` §26.1 | **the pipe** — cheaper and needs no `-P` |
| Constructs missing an attribute | `not: { has: … }` §7.15, §7.11 | — | **ast-grep** |
| Exclude a whole region from matching | `ignores:` §8.13, `not: { inside: … }` §7.15 | `(*SKIP)(*F)` §31.1, recipe §41.7 | **ast-grep** — the verb is fragile and PCRE2-only |
| Refine a match after it succeeded | `constraints` §8.6 | — | **ast-grep** — and constraints run **after**, so they cannot express `not` |

### §3.6 Repetition and co-location

| Question | ast-grep | ripgrep / PCRE2 | Prefer, and why |
|---|---|---|---|
| The same expression on both sides | repeated `$A` §6.4 | `\1` §28.0 | **ast-grep** — subtree equality, not text equality |
| A duplicated word | — | `\b(\w+)\s+\1\b` §41.5 | **PCRE2** — prose has no AST |
| Matching quote delimiters | — | `(?<q>['"]).*?\k<q>` §28.1, §41.6 | **PCRE2** |
| Repeated delimiter, several alternatives | — | branch reset `(?\|…)` §29.2, §41.15 | **PCRE2** |
| A and B on one line, either order | `all:` + relational §7.13 | double lookahead §41.1 | **staged narrowing** (`rg -l` then `rg`) — `ripgrep §42.1`, no `-P` needed |
| A near B but not on one line | `precedes`/`follows` §7.12 | `-U` §7.1, or `-C` §9.1 | **ast-grep** — proximity in lines is not proximity in structure |
| The same identifier in two files | — | `rg -o … \| sort \| uniq -d` | **rg + shell**; neither engine crosses files |

### §3.7 Text with no AST

| Question | ast-grep | ripgrep / PCRE2 | Prefer, and why |
|---|---|---|---|
| An exact literal containing regex metacharacters | `regex:` with escaping §7.4 | `-F` §6.0 | **`rg -F`** |
| An error message or log format | — | `rg -F` §6.0 | **rg** |
| A comment or docstring | (strictness ignores comments §6.10) | `rg` | **rg** — the structural engines discard trivia by design |
| A whole word | `regex: '^word$'` on node text §7.4 | `-w` §8.0 | **`rg -w`** |
| A whole line | — | `-x` §8.1 | **`rg -x`** |
| Several alternative literals | `any:` §7.14 | repeatable `-e` §4.1 | **`rg -e`**; note positionals then become paths |
| A long list of patterns from a file | — | `-f file` §4.2 | **rg**, and check for blank lines |
| Case-insensitively, but only if lowercase | — | `-S/--smart-case` §6.1 | **rg** |
| A Unicode script or category | `regex:` (the Rust engine supports `\p{…}`) §7.4 | `\p{…}` §32.0, recipe §41.13 | **rg** — both engines have it, `rg` reaches more files |
| Anything in a generated tree | `--no-ignore` §2.3 | `-u`/`-uu` §13.2 | **rg**, scoped to the path |

### §3.8 Multi-line

| Question | ast-grep | ripgrep / PCRE2 | Prefer, and why |
|---|---|---|---|
| A construct spanning lines | any pattern — nodes ignore line breaks §6.0 | `-U` §7.1 | **ast-grep** — no `-U`, no dotall, no anchor reasoning |
| Text block between two delimiters | — | `-U --multiline-dotall` §7.2, recipes §41.9-§41.10 | **rg**, scoped tightly |
| `.` should cross a newline | — | `--multiline-dotall` §7.2 | **rg** — **inert without `-U`** |
| `^`/`$` inside a multi-line haystack | — | §7.3, §37.5 | **rg**, after reading `ripgrep §24` Invariant 3 |
| CRLF files | — | `--crlf` §7.4, `(*CRLF)` §37.2 | **rg** |
| NUL-separated records | — | `--null-data` §7.5 | **rg** |

### §3.9 Engine selection

| Question | ast-grep | ripgrep / PCRE2 | Prefer, and why |
|---|---|---|---|
| Do I need `-P` at all? | n/a | §5.1 vs §5.2, matrix §46-§47, algorithm §47.1 | **default first** — `ripgrep §43.0` |
| Can I mix engines across patterns? | n/a | **no** — §4.4 | one engine per invocation |
| Is PCRE2 even compiled in? | n/a | `rg --pcre2-version` §36.1, gate §42.6 | probe, never assume |
| Which PCRE2 version do I have? | n/a | §44.8 — **not inferable from rg's version** | probe |
| Is this 10.47-only construct actually usable? | n/a | feature-probe with a control §42.9 | **feature-probe** — `--pcre2-version` can report lower than the linked library (`ripgrep §36.1`) |
| Should I use `--engine=auto`? | n/a | §5.3 | **no for evidence work** — it hides the choice (`ripgrep §42.5`) |
| Can I bound a runaway PCRE2 pattern? | n/a | `(*LIMIT_*)` §35.1 — **not** `--regex-size-limit` §24 Inv. 5 | pattern verbs, and they only lower |
| Is PCRE2 safe for untrusted patterns? | n/a | **no** — §40.2-§40.6 | process boundary; offer the default engine instead |

### §3.10 Scope and coverage

| Question | ast-grep | ripgrep / PCRE2 | Prefer, and why |
|---|---|---|---|
| Restrict to a subtree | `--globs 'src/**'` §2.4 | `-g 'src/**'` §15.0 | either — **`-g src` does not work** (`ripgrep §15.1`) |
| Restrict to a language | `-l` §3.4 | `-t` §15.3 | `-t` filters files; `-l` chooses a parser — not the same thing |
| Add a file type | `languageGlobs` §12.6 | `--type-add` §15.4 | **rg**, but `--type-add` does not persist (798) |
| Include hidden files | `--no-ignore hidden` §2.3 | `--hidden` §16.0 | either; exclude `.git` |
| Ignore `.gitignore` | `--no-ignore vcs` §2.3 | `-u` §13.2 | either — **scoped to a path**; both reach `.envrc.local` |
| Include binary files | — | `-uuu` / `-a` §13.2, §16.1 | **rg**; ast-grep needs a parse |
| Search inside archives | — | `-z` §18.0 | **rg** |
| Search a non-text source | — | `--pre` §18.1 | **rg** — allowlist the preprocessor (`ripgrep §18.2`) |
| A UTF-16 or latin1 file | — | `-E` §17.1 | **rg**; offsets become transcoded offsets (`ripgrep §17.3`) |
| Why was this file skipped? | `--inspect entity` §3.13 | `rg --debug` §22.1 | either — both name the reason |
| What was actually scanned? | `--inspect summary` §3.13 | `rg --files` §22.0 | cite the number with any negative claim |

### §3.11 Consuming results

| Question | ast-grep | ripgrep / PCRE2 | Prefer, and why |
|---|---|---|---|
| Just the matched text | `--json` `.text` §17.1 | `-o` §9.2 | **`rg -o`** for a shell one-liner |
| One captured field | `--json` `.metaVariables` §17.6 | `-o -r '$1'` §11.0-§11.1 | **`rg -r`** for text; **ast-grep** JSON for a tool |
| A named capture | `$NAME` in the pattern §6.2 | `(?P<n>…)` + `-r '$n'` §11.1 | either |
| Machine-readable stream | `--json=stream` §17.4 | `--json` §20.0 | both; **`--json=stream` needs the `=`** (`ast-grep §2.7`) |
| Byte-exact positions | `range.byteOffset` §17.3 | `--json` offsets §20.2 | either; ast-grep positions are **zero-based** |
| Filenames only | — | `-l` §10.1 | **rg** |
| Filenames into `xargs` | — | `-l --null` + `xargs -0` §23.1 | **rg**; never split on newline |
| Counts | — | `-c`, `--count-matches` §10.0 | **rg** |
| An editor jump list | `--json` §17.1 | `--vimgrep` §45.9 | either |
| CI annotations | `scan --format github\|sarif` §4.5 | — | **ast-grep** |

### §3.12 Changing code

| Question | ast-grep | ripgrep / PCRE2 | Prefer, and why |
|---|---|---|---|
| Preview a rename | `run -p … -r …` §3.8 | `rg -r` §11.0 | **ast-grep** — `rg -r` only reformats output |
| Apply a rename | `run … -i` then `-U` §3.8 | **cannot** §11.0 | **ast-grep** |
| A conditional codemod | YAML `fix` §10.2 | — | **ast-grep** |
| Edit beyond the matched node | `FixConfig` expansion §10.4-§10.5 | — | **ast-grep** |
| Derive the replacement text | `transform` §11.3-§11.5 | — | **ast-grep** |
| Rewrite each element of a list | `rewriters` + `rewrite` §11.7-§11.9 | — | **ast-grep**; prove termination (`ast-grep §22.10`) |
| Delete a node | empty `fix` §10.3 | — | **ast-grep** |
| Is this codemod safe to bulk-apply? | risk classes §10.10, VCS guard §22.2, idempotence §22.18 | — | **ast-grep** — and test before `-U` |

### §3.13 Recurring checks

| Question | ast-grep | ripgrep / PCRE2 | Prefer, and why |
|---|---|---|---|
| A one-off repository invariant | `scan --inline-rules` §4.1 | — | **ast-grep** |
| An invariant that must persist | `rules/` + `sgconfig.yml` §8, §12.3 | — | **ast-grep** |
| Make it fail CI | `severity: error` §16.1, `--error=<id>` §4.7 | exit 1 §10.3 | **ast-grep** — exit 1 means error-severity fired |
| Run one rule family | `--filter '^security\.'` §4.1 | — | **ast-grep** |
| Test the rule | `valid`/`invalid` + snapshots §15.2, §15.4 | — | **ast-grep**; cover the negative space (`ast-grep §22.19`) |
| Silence a false positive | suppression comments §16.3-§16.5 | — | **ast-grep**; `ast-grep §16.6` unused-suppression, `ast-grep §16.7` no-suppress-all |
| Reuse a sub-rule | `utils` §9.1, `utilDirs` §9.2, parameterized §9.4 | — | **ast-grep** |

### §3.14 Debugging and performance

| Question | ast-grep | ripgrep / PCRE2 | Prefer, and why |
|---|---|---|---|
| The pattern matches nothing | `--debug-query=cst` §3.6, four-step §21.2 | `rg --debug` §22.1 | **inspect the parse before complicating the query** |
| It matches too much | §21.4 | narrow the scope first §42.1 | scope, then strictness, then structure |
| Playground works, CLI does not | §21.3 | — | **ast-grep** — version/grammar mismatch |
| It is slow | scan fewer files §20.1, kind pruning §20.2, depth §20.3 | literal extraction §2.2, §36.5 | **narrow the candidate set first** in both |
| The regex is pathological | the Rust engine has no catastrophic backtracking §22.9 | possessive/atomic §29.0-§29.1, families to avoid §36.6 | **the default engine**, if it can express it |
| Output ordering is unstable | `-j 1` §3.9 | `-j 1` §19.1, or `--sort path` §19.2 | either — in rg any non-`none` sort already forces single-threaded |
| I need to cap output per file | — | `-m NUM` §10.4 | **rg**, but never together with `-c` (`ripgrep §10.4`) |
| Too much output | `--json=stream` §20.9 | §19.4, §43.6 | stream, or reduce to `-l`/`-c` |

---

## §4 — Cross-tool equivalence

### §4.1 PCRE2-only constructs and their structural substitutes

Every row's left column is unavailable in ast-grep **and** in ripgrep's default
engine. When you cannot or should not reach for `-P`, the middle column is the
reformulation.

| PCRE2 construct | Structural equivalent in ast-grep | Notes |
|---|---|---|
| `(?!Y)` negative lookahead | `not: { … }` `ast-grep §7.15` · `:not(…)` `ast-grep §7.18` | exact, and it crosses line breaks |
| `(?=Y)` positive lookahead | `has:` `ast-grep §7.11` · `precedes:` `ast-grep §7.12` | `has` descends, `precedes` is sibling-ordered |
| `(?<=Y)` lookbehind | `follows:` `ast-grep §7.12` · `inside:` `ast-grep §7.10` | removes the 255-char bound (`ripgrep §27.2`) entirely |
| `\1` / `\k<n>` backreference | repeated capturing `$A` `ast-grep §6.4` | **subtree** equality, not text equality |
| `\K` | `pattern.selector` `ast-grep §6.9` · `--selector` `ast-grep §3.5` | selects the reported node rather than trimming a match |
| `(*SKIP)(*F)` region exclusion | `not: { inside: … }` `ast-grep §7.15` · rule `ignores:` `ast-grep §8.13` | the verb is order-dependent; the rule is not |
| `(?R)` / `(?&name)` recursion | `has:`/`inside:` over the real parse `ast-grep §7.10`-`§7.11` | `ripgrep §30.1`: recursion "is not a parser replacement" |
| `(?\|…)` branch reset | `any:` `ast-grep §7.14` | ast-grep captures do not renumber |
| `(*scs:…)` scan-substring | `constraints` on a captured metavariable `ast-grep §8.6` | constraints run after the match, which is the same idea |
| `(?[A-[B]])` set algebra | `all:`/`not:` over `regex:` `ast-grep §7.13`, `§7.15` | ast-grep's `regex:` is the Rust engine (`ast-grep §7.4`) |
| `(?(cond)a\|b)` conditional | `any:` of two fully-specified `all:` branches `ast-grep §7.14` | — |
| `\G`, `(*ACCEPT)`, `(*PRUNE)` | no equivalent | backtracking-engine control has no structural analogue |

### §4.2 ast-grep-only capabilities, and the least-bad text approximation

| ast-grep capability | Text approximation | Why the approximation fails |
|---|---|---|
| ancestor containment `ast-grep §7.10` | `rg -U` over a hand-written block regex | block boundaries are not regular; nesting breaks it |
| descendant `has:` (+ `field:`) `ast-grep §7.11` | `rg -A n` `ripgrep §9.1` | a line count is not a subtree |
| positional `nthChild` `ast-grep §7.5` | counting commas | comments, nested calls and line breaks all defeat it |
| variable-arity `$$$ARGS` `ast-grep §6.3` | `\(([^)]*)\)` | fails on a nested call in any argument |
| kind-aware matching `-k` `ast-grep §3.3` | word boundaries | a keyword in a string or comment still matches |
| syntax-preserving rewrite `ast-grep §10` | `sed` | reindentation and capture reinsertion are manual |
| trivia policy via strictness `ast-grep §6.10` | none | you cannot ask a regex to ignore comments |
| a tested, versioned invariant `ast-grep §15` | a script and a convention | no snapshot, no negative-space coverage |

### §4.3 ripgrep-only capabilities

| Capability | Section | Why ast-grep cannot |
|---|---|---|
| bytes in a file with no grammar | `ripgrep §6.0` | no parser exists |
| comments, docstrings, prose | `ripgrep §6.0` | strictness discards trivia (`ast-grep §6.10`) |
| binary and archive contents | `ripgrep §16.1`, `§18.0` | not parseable |
| non-UTF-8 encodings | `ripgrep §17.1` | Tree-sitter expects decoded text |
| preprocessed sources (PDF, DB dumps) | `ripgrep §18.1` | — |
| the candidate-file listing itself | `ripgrep §10.1` `--files` | `outline` is per-file |
| whole-line and count modes | `ripgrep §8.1`, `§10.0` | ast-grep reports nodes, not lines |

### §4.4 Composition pipelines

When neither tool alone suffices. `ast-grep §23.14` is the document's own
chapter on this.

```text
structure narrows, text confirms
  ast-grep run … --json=stream | jq -r .file | sort -u | xargs rg -P '<assert>'

text narrows, structure confirms
  rg -l --null '<literal>' | xargs -0 ast-grep run -p '<pattern>'

structure locates, compiler proves
  ast-grep run -p '…' --json=stream   ->  candidate ranges
  edit / rename / delete              ->  cargo check enumerates the rest

analyzer locates, structure edits
  external tool reports a source range
  ->  ast-grep `range:` rule selects the node there   (ast-grep 7.6)
  ->  ast-grep fix applies the edit                   (ast-grep 10.2)
```

---

## §5 — Decision trees

ASCII arrows only inside these fences.

### Which instrument?

```
Is the question "what does this program mean" (types, resolved callers)?
  -> neither tool. Compiler / LSP / CPG.        (ast-grep 0.2, 24.22)
Is it "what is here" (inventory, surface, structure)?
  -> ast-grep outline                            (ast-grep 5)
Is it "what syntax has this shape"?
  -> ast-grep run -k   if a kind/relation suffices        (ast-grep 3.3)
  -> ast-grep run -p   if concrete syntax or captures matter  (ast-grep 3.2)
  -> ast-grep scan     if it needs relations or must persist  (ast-grep 4)
Is it "what bytes are present" (strings, comments, config, cross-language)?
  -> rg                                          (ripgrep 6, 15)
Is it a negative claim?
  -> all three tiers. See _shared/code-intelligence.md
```

### Which regex engine?

```
Can the default engine express it?            (ripgrep 5.1, matrix 46)
  yes -> use it. Linear time, literal prefilter.
  no  -> is the missing piece lookaround, a backreference, \K, a verb,
         recursion, set algebra, or a scan-substring assertion?
           yes -> can the requirement be restated structurally?  (REFERENCE 4.1)
                    yes -> ast-grep. Exact, and no backtracking.
                    no  -> rg -P, and:
                             probe   rg --pcre2-version        (ripgrep 36.1, 42.6)
                             scope   -g / -t before the pattern    (ripgrep 15)
                             bound   (*LIMIT_MATCH=) on large input  (ripgrep 35.1)
                             record  engine + version with the result (ripgrep 42.5)
           no  -> re-read ripgrep 46. The construct may not exist at all.
Is the pattern from an untrusted source?
  -> do not use PCRE2. ripgrep 40.5 makes the default engine the safer
     request language.
```

### How far do I widen the ignore stack?

```
Did the file appear in the candidate set?
  rg --files            /  ast-grep run --inspect summary
  no -> why not?
        rg --debug      /  ast-grep run --inspect entity
        gitignored?      -> --no-ignore-vcs   /  --no-ignore vcs
        hidden?          -> --hidden          /  --no-ignore hidden
        wrong type/glob? -> fix -t / -g       /  fix -l / --globs
        too large?       -> --max-filesize                  (ripgrep 16.2)
        binary?          -> -a / --binary                   (ripgrep 16.1)
  yes -> the problem is the pattern, not the scope. Go to the triage tree.

Never widen globally. -uu and --no-ignore vcs reach .envrc.local.
Scope the widening to a path.               (ast-grep 22.5, ripgrep 43.3)
```

### Nothing came back — triage

```
ast-grep                                  ripgrep
  1. is --lang explicit?      (21.2)        1. is the file in rg --files?  (22.0)
  2. --debug-query=cst        (3.6)         2. rg --debug for skip reason  (22.1)
  3. try one minimal real file (21.2)       3. is the engine right?        (22.4)
  4. add context + selector   (6.8)         4. -F to rule out regex parse  (6.0)
  5. is $$$ needed for arity? (6.3)         5. does the pattern need -U?   (7.1)
  6. is a repeated $A forcing equality? (6.4)  6. under -P, does it contain
  7. is strictness too strong? (6.10)          \n without -U? silent fail  (5.4)
  8. exit 1 from run means NO MATCH, not error  (3.14)
```

### Is this codemod ready to apply?

```
Is the rule tested with valid AND invalid cases?        (ast-grep 15.2)
  -> no: stop.
Does the negative space have coverage -- near misses, already-migrated code,
   commented-out code, nested occurrences, list boundaries?  (ast-grep 22.19)
  -> no: stop.
Is the fix snapshotted?                                  (ast-grep 15.4)
Is the working tree clean?                               (ast-grep 22.2)
Is the transformation idempotent?                        (ast-grep 22.18)
Which risk class?                                        (ast-grep 10.10)
  A structural substitution -> scan -i, then -U
  B syntax restructuring    -> scan -i only, review every hunk
  C semantic migration      -> human review per site; -U is not appropriate
Then: git diff --check, formatter, typecheck, tests.     (ast-grep 4.9)
```

### Which outline view?

```
unknown subtree            -> outline <dir>       (dir default: exports/names)
known file, unknown shape  -> outline <file>      (file default: structure/digest)
known API, want signatures -> --view signatures
known type, want members   -> --match '^T$' --view expanded
dependency direction       -> --items imports --view signatures
feeding another tool       -> --json=stream, keep the ranges   (ast-grep 5.12)
     rendered NNNN: is 1-based; JSON range.start.line is 0-based
```

---

## §6 — Exit codes and output contracts

`_shared/code-intelligence.md` carries the policy; this is the mechanical
statement with citations.

| Command | 0 | 1 | 2 |
|---|---|---|---|
| `ast-grep run` | matched | **no match** | — (`ast-grep §3.14`) |
| `ast-grep scan` | no error-severity finding | an **error-severity** rule fired | — (`ast-grep §4.8`) |
| `ast-grep outline` | completed, **including an empty outline** | — | fatal read/parse/config error (`ast-grep §5.16`) |
| `ast-grep test` | every selected case passed | — **never returns 1** | `3` filter selected nothing · `4` assertion or snapshot failure · `6` missing `testDir` · `8` unparseable rule/config (`ast-grep §15.11`) |
| `rg` | matched | no match | error (`ripgrep §10.3`) |

Two consequences that scripts get wrong. A clean zero from `ast-grep run` is
**exit 1**, so `set -e` will kill the script — but do not invert that into "any
nonzero means no matches": capture stderr and distinguish real errors
(`ast-grep §3.14`). And `rg --no-messages` suppresses the *output* of errors,
not the error state — always read the exit code (`ripgrep §43.11`).

| Output contract | ast-grep | ripgrep |
|---|---|---|
| structured mode | `--json`, `=compact`, `=stream` — **style attaches with `=`** (`ast-grep §2.7`) | `--json`, one message per line (`ripgrep §20.0`) |
| match schema | `ast-grep §17.1`; `scan` adds `ruleId`/`severity`/`message`/`note`/`labels` (`ast-grep §17.2`) | begin/match/context/end/summary (`ripgrep §20.0`) |
| captures | `metaVariables.single` / `.multi` / `.transformed` (`ast-grep §17.6`) | `--json` submatches (`ripgrep §20.2`) |
| coordinates | **zero-based** line/column plus byte offsets (`ast-grep §17.3`, `ast-grep §24.19`) | byte offsets; **transcoded input shifts them** (`ripgrep §17.3`) |
| non-UTF-8 bytes | — | explicitly encoded in JSON (`ripgrep §20.1`) |
| outline JSON | one `OutlineFile` per line under `=stream`; differs from run/scan by design (`ast-grep §5.12`, `ast-grep §17.8`) | — |
| CI formats | `--format github`, `--format sarif` (`ast-grep §17.9`, `ast-grep §17.10`) | — |
| stability policy | `ast-grep §17.11` — pin what you parse | `ripgrep §20.3` — the recommended agent contract |
| diagnostics channel | `--inspect` writes to **stderr** so stdout stays clean (`ast-grep §3.13`) | `--debug`/`--trace` to stderr (`ripgrep §22.1`) |

---

## §7 — Navigation hazards

Read these before searching either file.

**1. Never write a bare `§N`.** Both documents number chapters `# N)` and
subsections `## N.M`, and §0-§25 exist in both. The collisions:

| § | ast-grep | ripgrep |
|---|---|---|
| §0 | mental model and capability boundaries | scope and release stance |
| §5 | `outline` | regex engine selection |
| §7 | the rule object | multiline, dotall, anchors, CRLF |
| §13 | languages, injections, custom parsers | the ignore stack |
| §20 | traversal, performance, pruning | JSON output |
| §22 | correctness, safety, trust boundaries | debugging search space |
| §23 | agentic coding workflows | shell scripting and filename transport |
| §24 | capability matrices | PCRE2 integration architecture |
| §25 | upgrade policy and final playbook | PCRE2 syntax and feature map |

Always carry the alias. When reading someone else's citation, work out which
document it was written against before trusting it —
`_shared/code-intelligence.md` carried nine ripgrep citations against a
*different* ripgrep reference until this skill was written.

**2. Grep gives you mentions, not definitions.** Both documents use their own
names constantly: `outline` appears on 172 lines of the ast-grep reference,
`--json` on 41, `strictness` on 25, `stopBy` on 20; `PCRE2` appears on 281 lines
of the ripgrep reference and `-P` on 106. The tail chapters — matrices,
anti-patterns, checklists — mention nearly every name once, so a grep's last
hits are almost always a checklist rather than the definition. **Use
REFERENCE §2.**

**3. Each document renders its own table of contents as `##` headings,
shape-identical to body subsections, and the TOC titles have drifted.**
ast-grep's TOC is lines 69-99 (`## N)`), its bodies are `# N)`; ripgrep's TOC is
33-87, bodies from 88. Confirmed title mismatches: **5 in ast-grep** (§21, §22,
§23, §24, §25 — e.g. TOC "Debugging and observability" vs body "Debugging and
troubleshooting — a deterministic triage playbook") and **13 in ripgrep** (§0,
§4, §6, §7, §8, §15, §16, §17, §22, §23, §32, §38, §44). **Cite the body
heading.**

**4. Fenced `#` lines masquerade as headings.** A bare `rg '^#'` returns 613
lines from the ast-grep reference of which **49 are shell comments or
sample-file content inside code fences** — including
`## Structural code navigation` at 6703, which sits inside a sample `AGENTS.md`
in `ast-grep §23.2` and looks exactly like a real subsection. ripgrep has **4**
(1364, 1367, 1370 inside `ripgrep §27.3`'s shell block, and 2507 inside
`ripgrep §42.7`'s YAML block). `just lib-outline` parses markdown and is
fence-aware; a hand-rolled `grep` is not. Note also that the rendered `NNNN:`
prefix is 1-based (feed it to `Read`) while `range.start.line` in `--json` is
0-based.

**5. Absence is not proof of absence — for flags, and for versions.** Both
documents say the same thing: **the installed binary is the final authority on
its own flags** (`ast-grep §25.1`; `ripgrep §1.0`), and ast-grep keeps an
explicit **documentation-drift ledger** at `ast-grep §24.24` for the places
where upstream pages lag its implementation. Two concrete instances have already
been worked through here.

*Flags.* Four ripgrep flags had **zero mentions** in this reference —
`--context-separator`, `-m/--max-count`, `--sort`/`--sortr`, and the literal
`-j/--threads`. They were not missing from the tool, only from the prose. They
are now documented at `ripgrep §9.5`, `§10.4`, `§19.2` and `§19.1`
respectively, written against `rg --help` for the pinned 15.2.0 build and
checked against its observed behavior. `ast-grep test`'s exit contract was
likewise unwritten and is now `ast-grep §15.11`. When you next find a gap,
close it the same way — help text plus an executed probe — rather than
recording the gap and moving on.

*Versions.* The stronger case is `rg --pcre2-version`, which prints the version
ripgrep was **compiled against**, not the shared library it is running on. A
build here reports `PCRE2 10.45` while the 10.47-only returned-capture recursion
of `ripgrep §30.2` matches and its control correctly fails. So a version report
**at or above** the requirement is good evidence; a report **below** it is no
evidence at all. Gate on a feature probe with a control (`ripgrep §42.9`), never
on a low version string (`ripgrep §36.1`).

**6. Numbering is not uniform.** `ripgrep §14` and `ripgrep §45` have no `.0`
subsection; `ripgrep §46` and `ripgrep §47` open with an unnumbered matrix
before their first `.1`; `ripgrep §47` has exactly one subsection.
`ripgrep §0`'s body heading is `# ripgrep Advanced — 0) …`, unlike every other
chapter. In ast-grep, §6, §10, §17 and §20 have no `### Agent checklist` even
though the other 22 do.

**7. Some facts live above §0.** Both front matters carry version statements no
chapter repeats. ast-grep lines 9-16: the 0.45.1 / edition-2024 / MSRV-1.88.0
anchor, the **distribution-surface caveat** that GitHub's Releases UI still
labels 0.45.0 "Latest", and the `sg` deprecation. ripgrep lines 3-11: the two
release dates, the statement that release artifacts may bundle an older PCRE2
while packaged builds link the system one, and the CVE-2025-58050
security-boundary block. Read lines 1-60 for any version question a chapter does
not settle.

**8. A structural or lexical result is version-scoped.** A parser upgrade can
change node kinds, field names, and the named/unnamed boundary, so an ast-grep
result is only valid for the grammar that produced it (`ast-grep §22.15`). A
PCRE2 pattern using a 10.47 construct is only valid for a library that new, and
the ripgrep version does not tell you which one is linked (`ripgrep §42.7`,
`ripgrep §44.8`). Record tool and version alongside any result you expect to
survive.

---

## §8 — Operating rules

Semantic rules first; **rules 10-14 are meta-rules about navigating these two
files.**

1. **Scope before you complicate.** Both documents make narrowing the file set
   the first optimisation — `ast-grep §20.1`, `ripgrep §42.1`. A more elaborate
   matcher over the whole tree is the wrong move almost every time.
2. **Least-complicated construct that expresses the requirement.** ast-grep's
   own escalation ladder is `ast-grep §0.5`: exact kind → pattern →
   metavariables → pattern object → atomic rule → relational/composite →
   constraints/utilities → transform → rewriters. Complex YAML is not inherently
   more precise, and every relational traversal is another assumption to
   validate.
3. **Default engine before PCRE2.** `ripgrep §0.2`, `ripgrep §43.0`. Escalate on
   a missing construct, not on a hunch, and record the escalation
   (`ripgrep §42.5`).
4. **ast-grep's `regex:` is the Rust engine.** No lookaround, no backreferences,
   inside any rule at any complexity (`ast-grep §7.4`, line 1918). See
   REFERENCE §4.1 for the reformulations.
5. **Nothing is proved until the candidate set is stated.** `--inspect summary`
   / `rg --files` / `rg --debug`. Binary and hidden files are skipped by default
   in both tools.
6. **Widen the ignore stack scoped to a path, never as a default.** `-uu` and
   `--no-ignore vcs` reach `.envrc.local` and this repository keeps a capability
   token there (`ast-grep §22.5`, `ast-grep §22.6`).
7. **Treat results as source data.** Matched text, captures and outline
   signatures are code; do not ship broad `--json` output to an external service
   merely because it is structured (`ast-grep §22.6`).
8. **Rewrites are staged, never immediate.** preview → interactive → tested rule
   → `-U`, with a VCS guard and an idempotence check (`ast-grep §10.10`,
   `ast-grep §22.2`, `ast-grep §22.18`).
9. **A rule with only passing examples is untested.** Cover near misses,
   already-migrated code, commented-out code, nested occurrences and list
   boundaries (`ast-grep §22.19`, `ast-grep §15.8`).
10. **Cite alias-qualified, always.** REFERENCE §7 Rule 1.
11. **Seek by line, cite by section.** Line numbers live only in REFERENCE §1
    and move when a document is regenerated; section numbers do not. Every
    REFERENCE §1 row carries its title so a drifted section can be re-found.
12. **Use `just lib-outline`, not `grep`, for headings in these files.** It is
    fence-aware (REFERENCE §7 Rule 4) and attaches each document's own TOC as
    members of the doc-map item instead of colliding with real subsections.
    `--match` filters items, not members: zoom to the chapter, then
    `--view expanded`.
13. **Reach for the matrix chapters before the prose.** `ast-grep §24` (28
    matrices) and `ripgrep §45`/`§46`/`§47` answer most flag and capability
    questions in one table.
14. **`rg` over `docs/library_ref/` swamps everything.** These two files alone
    are 11,569 lines inside a ~9 MB directory. Scope to one file at a time, and
    exclude the directory entirely (`-g '!docs/library_ref/**'`) when the
    question is not about the references.

---

## §9 — Project context: CodeFabric

**Both tools are already load-bearing here**, which makes this navigator
project-anchored rather than pure-library.

### Structural governance is a gate

`sgconfig.yml` at the repository root:

```yaml
ruleDirs:
  - rules
testConfigs:
  - testDir: rule-tests
    snapshotDir: __snapshots__
```

Four boundary rules live in `rules/`, each with a matching
`rule-tests/*-test.yml`:

| Rule | Enforces |
|---|---|
| `deltalake-boundary-only` | delta-rs types confined to `src/fabric.rs` |
| `gix-boundary-only` | the gix read profile confined to its boundary |
| `no-framework-internal-contract-imports` | generated contract internals stay unimported |
| `no-pyrefly-public-api` | the sidecar's unstable-library seam |

`just governance-scan` runs `ast-grep test --skip-snapshot-tests` and then
`ast-grep scan` with five `--globs` exclusions for the generated trees, and it
is wired into `just ci-fast`. The existing rules are worth reading as exemplars:
they combine `kind` with `regex` under `any:`/`all:`, and use file-level
`files:`/`ignores:` to carve out the boundary module — exactly the shape
`ast-grep §8.12`-`§8.13` describes. Adding an invariant means adding a rule plus
its negative-space tests, not a hand-tallied search.

### Two custom outline extractors

`tooling/ast-grep/outline/specs.yml` and `library-ref.yml` are markdown outline
extractors, each with a `.test.sh` beside it pinning the properties that matter.
They are loaded through `scripts/spec-outline.sh` and `scripts/lib-outline.sh`
(`just spec-outline`, `just lib-outline`) because **markdown is a built-in
language and `outlineRules` is only a `customLanguages` field** — there is no
`sgconfig.yml` registration path, so `--outline-rules` must be passed on every
invocation.

| Extractor | Item | Member | Corpus |
|---|---|---|---|
| `specs.yml` | `## N. Section` | `### N.N Subsection` | `docs/upfront_design/` (h2-rooted) |
| `library-ref.yml` | `# Chapter` | `## Subsection` | `docs/library_ref/` (h1-rooted) |

Each script refuses the other's tree rather than emit a misleading flat outline.
`selector: section` is load-bearing in both — without it the matched node is the
heading line alone, which contains no subsections, so members can never attach.
`ast-grep §14` is the field reference if you change either.

**The two documents this skill routes are navigated by `lib-outline`**, and
REFERENCE §1 of this file was generated from it.

### The two live search traps

- **`.claude/` is hidden.** A default `rg` cannot see the skills, so a term
  present in both `.claude/skills/` and a tracked directory returns only the
  tracked hits — which reads exactly like completeness. Use
  `--hidden -g '!.git/**'` whenever skills, settings or any dotfile are in scope
  (`ripgrep §16.0`).
- **`docs/library_ref/` is ~9 MB of prose** mentioning nearly every identifier
  you will ever search for. Exclude it with `-g '!docs/library_ref/**'` unless
  the question is about the references.

### Where the doctrine lives

`.claude/skills/_shared/code-intelligence.md` owns the three-tier instrument
ladder, the mandatory tripwires, coverage-as-a-declared-artifact, and the
design/execution/review phase guidance. This file owns the capability surface.
Neither restates the other; when they touch the same fact — exit codes, the
repository traps — this file carries the citation and that file carries the
rule.

**Do not use a tool to certify its own modification.** When changing search or
analysis tooling — including these extractors and the governance rules — verify
with independent source reads, compiler output, and tests.
