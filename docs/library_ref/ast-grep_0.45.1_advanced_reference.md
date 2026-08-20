# ast-grep 0.45.1 — advanced technical reference / structural search, lint, rewrite, outline, and API catalog for LLM coding agents

This is a release-pinned, operational reference for **ast-grep 0.45.1**. It is written for coding agents and engineers that need to choose between structural search, repository linting, codemods, low-token source navigation, editor diagnostics, custom Tree-sitter languages, or direct library APIs without confusing syntax-aware matching with compiler-level semantics.

The document preserves the existing ast-grep reference's practical pattern — **contract → operational meaning → knobs → examples → caveats → debugging** — while expanding the coverage to the full 0.45.x surface, including the capabilities added after the earlier reference: `outline`, `run --kind`, ESQuery selectors and pseudo-classes, parameterized utility rules, `template` strictness, Markdown and Dart support, transforms/rewriters, injected languages, suppressions, snapshot testing, structured JSON contracts, and JavaScript/Python/Rust/WASM APIs.

---

## Version / source anchors

**Release / implementation anchor:** **`0.45.1` (2026-08-07)** is present in the published Rust package line, and the upstream workspace `Cargo.toml` declares **`0.45.1`**, Rust edition 2024, and **MSRV Rust 1.88.0**. The workspace includes the dedicated `ast-grep-outline` crate alongside core/config/dynamic/language/LSP crates.

**Distribution-surface caveat:** as of **2026-08-19**, GitHub's Releases UI still labels **0.45.0 (2026-07-23)** as “Latest,” while 0.45.1 crates and multiple platform-specific npm binary packages are already published. Some aggregate package/documentation pages also lag by a patch release. For automation, always verify the actually installed binary with `ast-grep --version` and `ast-grep --help` rather than inferring its feature surface from one registry or release page.

**Canonical executable:** use **`ast-grep`**. The historical `sg` executable is deprecated in the 0.45 line, and `sg` also collides with a standard Unix/Linux `setgroups` command on many systems.

### Source-of-truth hierarchy

| Priority | Source | Use |
|---:|---|---|
| 1 | installed `ast-grep --help` / subcommand help | exact flags supported by the binary that will execute |
| 2 | upstream 0.45.x source + changelog | exact implementation/version changes and fixes |
| 3 | current official CLI reference | command contracts, flag interactions, output behavior |
| 4 | current YAML/reference pages | rule/config/fix/transform/outline schema semantics |
| 5 | current guides | mental models, workflows, caveats, examples |
| 6 | language/API package documentation | bindings, dynamic-language, host API contracts |
| 7 | playground/catalog examples | pattern prototyping, not a substitute for version-pinned CLI behavior |

### Primary anchors used throughout

* [S1] Project repository: https://github.com/ast-grep/ast-grep
* [S2] Workspace `Cargo.toml`: https://github.com/ast-grep/ast-grep/blob/main/Cargo.toml
* [S3] Changelog: https://github.com/ast-grep/ast-grep/blob/main/CHANGELOG.md
* [S4] CLI reference: https://ast-grep.github.io/reference/cli.html
* [S5] `run`: https://ast-grep.github.io/reference/cli/run.html
* [S6] `scan`: https://ast-grep.github.io/reference/cli/scan.html
* [S7] `outline`: https://ast-grep.github.io/reference/cli/outline.html
* [S8] `test`: https://ast-grep.github.io/reference/cli/test.html
* [S9] `new`: https://ast-grep.github.io/reference/cli/new.html
* [S10] Pattern syntax: https://ast-grep.github.io/guide/pattern-syntax.html
* [S11] Rule object: https://ast-grep.github.io/reference/rule.html
* [S12] Lint rules: https://ast-grep.github.io/guide/project/lint-rule.html
* [S13] `sgconfig.yml`: https://ast-grep.github.io/reference/sgconfig.html
* [S14] Rewrite guide: https://ast-grep.github.io/guide/rewrite-code.html
* [S15] Fix reference: https://ast-grep.github.io/reference/yaml/fix.html
* [S16] Transformation reference: https://ast-grep.github.io/reference/yaml/transformation.html
* [S17] Outline guide: https://ast-grep.github.io/guide/outline-code.html
* [S18] Outline extractor fields: https://ast-grep.github.io/reference/outline-rule-fields
* [S19] 0.42 release / parameterized utilities: https://ast-grep.github.io/blog/new-ver-42.html
* [S20] Language reference: https://ast-grep.github.io/reference/languages.html
* [S21] Editor integration: https://ast-grep.github.io/guide/tools/editors.html
* [S22] JSON mode: https://ast-grep.github.io/guide/tools/json.html
* [S23] JavaScript API: https://ast-grep.github.io/guide/api-usage/js-api.html
* [S24] Python API: https://ast-grep.github.io/guide/api-usage/py-api.html
* [S25] Rust `ast-grep-core`: https://docs.rs/ast-grep-core/
* [S26] Multi-option interactive fixes: https://ast-grep.github.io/blog/interactive-demo

### Documentation-drift policy

The official documentation is actively generated and occasionally internally inconsistent with the newest implementation. Two concrete 0.45.x examples matter for agents:

* the current `run` CLI reference accepts `--strictness template`, while the separate rule-object reference still enumerates only the five older modes;
* the built-in language table has lagged release/source additions such as restored **Dart** support and **Markdown** support.

Where such discrepancies exist, this reference says so explicitly. Do **not** normalize disagreements by guessing. The installed binary remains the final operational authority.

---

# Comprehensive documentation map

## 0) Mental model, capability boundaries, and chooser matrix
## 1) Installation, version verification, executable naming, and shell integration
## 2) CLI topology and global operational conventions
## 3) `ast-grep run` — ad-hoc structural search and one-off rewrite
## 4) `ast-grep scan` — repository rules, linting, CI, and batch codemods
## 5) `ast-grep outline` — low-token syntax outline for coding agents
## 6) Pattern language — Tree-sitter parsing, metavariables, selectors, strictness
## 7) Rule object — atomic, relational, composite, and ESQuery matching
## 8) Lint-rule YAML — complete finding/reporting/patching/file-selection surface
## 9) Utility rules — local/global reuse and parameterized utilities
## 10) Rewrite system — CLI rewrite, `fix`, range expansion, indentation
## 11) Transformations and rewriters — generated metavariables and recursive rewrites
## 12) Project configuration — `sgconfig.yml`, discovery, directories, globs
## 13) Languages — built-ins, aliases, language globs, custom parsers, injections
## 14) Outline extraction rules — custom items/members and JSON structure contracts
## 15) Rule testing — valid/invalid cases, snapshots, filtering, test configuration
## 16) Severity, suppressions, runtime overrides, and CI gating
## 17) Output contracts — terminal, JSON, SARIF, GitHub annotations, coordinates
## 18) LSP/editor integration — diagnostics, code actions, rule reload behavior
## 19) Programmatic APIs — JavaScript/N-API, Python/PyO3, Rust core, WASM
## 20) Traversal, performance, pruning, concurrency, and scaling
## 21) Debugging and observability — query parse, discovery, matcher isolation
## 22) Correctness traps, version-sensitive behavior, and security considerations
## 23) Agent-first workflows — navigation, querying, refactoring, verification
## 24) Capability and decision matrices
## 25) Upgrade checklist and final LLM-agent playbook

---

# 0) Mental model, capability boundaries, and chooser matrix

## 0.0 One-sentence model

**ast-grep parses both code and structural patterns with Tree-sitter, then matches, reports, and optionally rewrites syntax nodes according to structural rules.**

This gives it a different operating envelope from plain text search, an LSP, or a compiler query engine.

## 0.1 What ast-grep is exceptionally good at

* **syntax-aware search**: find calls, declarations, fields, control-flow constructs, import forms, attributes, decorators, generic syntax, or combinations of AST relations;
* **structural refactoring**: preserve captured syntax while changing the surrounding construct;
* **repository lint rules**: version-controlled YAML checks with severity, messages, labels, suppressions, tests, and fixes;
* **repeatable codemods**: tested rules plus deterministic text-template patches;
* **cross-file high-throughput scanning**: Rust implementation, file-level parallelism, ignore/glob handling, rule pruning;
* **low-token code navigation**: `outline` exposes top-level items, imports/exports, and direct members without reading complete files;
* **editor diagnostics**: the built-in LSP can publish rule diagnostics and quick fixes;
* **embedding**: JavaScript, Python, Rust, and WASM APIs expose parsing/traversal/matching/edit primitives.

## 0.2 What ast-grep does *not* intrinsically know

ast-grep is a **syntax engine**, not a semantic compiler database. A match does not inherently know:

* resolved symbol identity;
* inferred or compiler-checked type;
* overload resolution;
* module/package resolution;
* trait/interface implementation resolution;
* macro expansion semantics;
* call graph edges;
* runtime control/data flow;
* cross-file reference identity;
* whether two equal-looking identifiers refer to the same declaration.

A YAML rule can infer syntactic context, and host applications can combine ast-grep ranges with compiler/LSP facts, but those semantic facts are not created merely by parsing an AST.

### Operational implication for coding agents

Use ast-grep to answer **“what syntax has this structure?”**. Use compiler/LSP/CPG/type-analysis systems to answer **“what program entity does this syntax mean?”**. Often the optimal pipeline is:

```text
cheap file narrowing
  -> ast-grep structural candidate search
  -> semantic confirmation only for candidates
  -> ast-grep rewrite or host-generated edit
  -> formatter/typecheck/test validation
```

## 0.3 Tree-sitter AST/CST terminology

Tree-sitter produces a concrete syntax tree containing both:

* **named nodes** — meaningful grammar constructs such as `call_expression`, `identifier`, `function_item`;
* **unnamed nodes** — tokens such as operators, punctuation, and some keywords.

ast-grep documentation often says **AST** for the parsed syntax tree generally. This matters because:

* `$VAR` normally captures a **named** node;
* `$$VAR` can capture an **unnamed** node;
* strictness modes decide how much named/unnamed/text detail participates in matching;
* `--debug-query=cst` is necessary when punctuation/operator structure is the source of a mismatch.

## 0.4 Capability chooser

| Goal | Preferred surface | Why |
|---|---|---|
| find one structural pattern now | `ast-grep run -p` | no project scaffold |
| find one node kind / ESQuery shape | `ast-grep run -k` | avoids constructing code pattern |
| preview a one-off codemod | `run -p ... -r ...` | immediate diff/interactive path |
| maintain many repo checks | `ast-grep scan` + `sgconfig.yml` | reusable rules, severity, CI, fixes |
| execute one YAML rule without project | `scan -r rule.yml` | full rule language, isolated |
| prototype a YAML rule inline | `scan --inline-rules ...` | no file creation needed |
| map a directory's public syntax surface | `ast-grep outline <dir>` | low-token exported-name map |
| inspect class/struct members | `outline <file> --match X --view expanded` | signatures and line ranges |
| discover imports | `outline --items imports` | syntax-only dependency direction |
| persistent editor lint | `ast-grep lsp` | diagnostics + code actions |
| verify rule behavior | `ast-grep test` | valid/invalid + snapshot assertions |
| custom procedural analysis | JS/Python/Rust API | arbitrary host computation |
| browser/edge structural parsing | WASM package | no native binary requirement |
| semantic references/types | compiler/LSP/CPG, not ast-grep alone | syntax is insufficient |

## 0.5 Search surface escalation ladder

Start with the least complicated construct that can express the requirement:

```text
exact node kind
  -> code pattern
  -> pattern + metavariables
  -> pattern object (context + selector)
  -> atomic YAML rule
  -> relational/composite rule
  -> constraints / utilities
  -> transform + fix
  -> rewriters
  -> host API / semantic tool composition
```

This ordering is important. Complex YAML is not inherently more precise than a clear structural pattern. Every extra relational traversal or reusable indirection increases the number of assumptions an agent must validate.

## 0.6 0.40–0.45 features that materially change older references

| Version | Capability / change | Agent significance |
|---|---|---|
| 0.39 | ESQuery-style `kind`; `template` strictness | compact selector queries; text-oriented template matching |
| 0.40 | SARIF; file-rule improvements | stronger CI integration |
| 0.40.5 | case-insensitive rule file filters | more robust platform/path policy |
| 0.41 | WASM; rule id can default from filename | browser embedding; simpler rules |
| 0.42 | parameterized global utilities; `:has`, `:not`, `:is`, `:nth-child`; injected-language LSP diagnostics | reusable rule abstractions and richer selectors |
| 0.42.1 | Dart support restored | language coverage |
| 0.42.3 | `:nth-last-child` | positional selectors |
| 0.43 | ESQuery selector accepted by `run --kind`; Markdown support | fast CLI selector authoring; docs/code analysis |
| 0.44 | **`outline` alpha**; streaming outline extraction; single-traversal optimization; metavariable correctness fixes | major coding-agent navigation feature |
| 0.44.1 | custom-language outline rules; more bundled outline coverage | extension to custom grammars |
| 0.45 | `sg` deprecation; ambient TS module outline support; ignore behavior fixes | executable migration + outline fidelity |
| 0.45.1 | root metavariable/comment correctness; C# multiline outline signature fix | patch-level correctness |

---

# 1) Installation, version verification, executable naming, and shell integration

## 1.0 Canonical installation choices

Common supported installation routes include:

```bash
# Cargo: explicit Rust-native installation
cargo install ast-grep --locked

# Homebrew (macOS/Linux)
brew install ast-grep

# npm CLI package
npm install -g @ast-grep/cli

# Python-distributed CLI package
pip install ast-grep-cli
```

Additional packaging exists through platforms such as MacPorts and Nix. Package freshness can vary.

### Deployment rule

For a reproducible developer/CI environment, pin the package version in the environment provisioning layer and assert the binary at startup:

```bash
ast-grep --version
ast-grep --help
```

Do not assume `cargo`, Homebrew, npm, and pip publish the same patch at the same instant.

## 1.1 `ast-grep`, not `sg`

Use:

```bash
ast-grep --help
ast-grep run --help
```

Avoid new automation that invokes:

```bash
sg ...
```

Reasons:

1. the `sg` alias/executable is deprecated in the 0.45 line;
2. Linux commonly already provides `/usr/bin/sg` for changing group execution context;
3. shell PATH changes can silently invoke the wrong program;
4. explicit `ast-grep` is self-documenting in agent command traces.

If a human wants a short alias, make it a **shell alias under their control**, not a repository contract.

## 1.2 Shell completions

```bash
ast-grep completions bash
ast-grep completions zsh
ast-grep completions fish
ast-grep completions powershell
ast-grep completions elvish
```

The command writes a completion script to stdout. Persistence is shell/distribution-specific.

Ephemeral examples:

```bash
# bash
source <(ast-grep completions bash)

# zsh
source <(ast-grep completions zsh)

# fish
ast-grep completions fish | source
```

## 1.3 Rust/MSRV note for source builds and embedding

The current upstream 0.45.1 workspace declares:

```text
edition = 2024
rust-version = 1.88.0
```

This is relevant when:

* building the CLI from source;
* depending directly on `ast-grep-core` / config/language crates;
* vendoring ast-grep into a Rust workspace with an older toolchain;
* producing reproducible cross-platform binaries.

A prebuilt CLI package does not force the consumer's repository itself to use that Rust edition.

## 1.4 Installation verification harness

Recommended for tool bootstrap scripts:

```bash
set -euo pipefail

command -v ast-grep >/dev/null
ast-grep --version
ast-grep --help >/dev/null
ast-grep run --help >/dev/null
ast-grep scan --help >/dev/null
ast-grep outline --help >/dev/null
```

If `outline` is missing, you do **not** have the expected modern 0.44+ CLI surface.

---

# 2) CLI topology and global operational conventions

## 2.0 Current command inventory

The current CLI exposes:

```text
ast-grep run          ad-hoc structural search/rewrite (default subcommand)
ast-grep outline      syntax-aware file/directory outline
ast-grep scan         project rules / lint / batch rewrite
ast-grep test         rule test and snapshot runner
ast-grep new          scaffold project/rule/test/util
ast-grep lsp          language server
ast-grep completions  shell completion generation
ast-grep help         command help
```

The earlier reference's statement that the current CLI was only `run/scan/test/new/lsp/completions/help` is no longer true because `outline` was added in 0.44.

## 2.1 `run` is the default subcommand

These are equivalent:

```bash
ast-grep -p 'foo($X)' src/
ast-grep run -p 'foo($X)' src/
```

For automation and agent logs, prefer the explicit `run` form when readability matters; for interactive shell use, the abbreviated default is convenient.

## 2.2 Common input/traversal conventions

Across the search-like commands, expect the same broad file-routing model:

* path arguments default to `.` where applicable;
* language is normally inferred from filename extension;
* `--lang` can force a language and is required for stdin when no filename exists;
* ignore files and hidden-file policy apply by default;
* `--globs` can include/exclude and overrides ordinary ignore routing;
* `--follow` opts into symlink traversal with loop/broken-link checks;
* `--no-ignore` selectively disables ignore layers;
* `-j/--threads 0` means ast-grep chooses a heuristic thread count.

## 2.3 Ignore-layer vocabulary

The repeatable `--no-ignore <FILE_TYPE>` accepts:

| Value | Effect |
|---|---|
| `hidden` | include hidden files/directories |
| `dot` | do not respect `.ignore` files |
| `exclude` | do not respect repository manual excludes such as `.git/info/exclude` |
| `global` | do not respect global VCS ignores such as Git `core.excludesFile` |
| `parent` | do not respect ignore files in parent directories |
| `vcs` | do not respect VCS ignore files; implies parent behavior for VCS ignore files, but `.ignore` remains separate |

Example:

```bash
ast-grep run -p 'foo($X)' \
  --no-ignore hidden \
  --no-ignore vcs \
  .
```

## 2.4 `--globs`: explicit route override

```bash
ast-grep run -p 'foo($X)' \
  --globs 'src/**/*.ts' \
  --globs '!src/**/__generated__/**' \
  .
```

Rules:

* syntax follows `.gitignore`-style glob conventions;
* prefix `!` excludes;
* multiple flags are allowed;
* when multiple globs match, **later flags win**;
* explicit globs override normal ignore behavior.

## 2.5 StdIn is not a filename

For `run` and `outline`, stdin cannot provide extension-based language inference:

```bash
cat file.py | ast-grep run --stdin --lang python -p 'print($X)'
cat file.ts | ast-grep outline --stdin --lang ts
```

Operational footgun: invoking a stdin mode in an interactive TTY without piping data can appear to hang because it is waiting for EOF.

## 2.6 Human vs machine output

Prefer:

```text
human inspection      -> default rich/text output
large shell pipeline  -> --json=stream
small structured use  -> --json or --json=compact
GitHub CI annotations -> scan --format github
code scanning         -> scan --format sarif
```

Do not parse colored human output as an API.

## 2.7 JSON style syntax footgun

Where the CLI uses an **optional value** for `--json`, style must be attached with `=`:

```bash
ast-grep run -p 'foo($X)' --json=stream .
```

Do not write:

```bash
ast-grep run -p 'foo($X)' --json stream .
```

because `stream` can be parsed as a path/positional argument.

---

# 3) `ast-grep run` — ad-hoc structural search and one-off rewrite

## 3.0 Contract

```bash
ast-grep run [OPTIONS] <--pattern <PATTERN>|--kind <KIND>> [PATHS]...
```

`PATHS` defaults to `.`.

Modern `run` therefore has **two primary matcher entry points**:

1. `--pattern/-p`: parse code-shaped pattern syntax;
2. `--kind/-k`: match an AST kind or an ESQuery-style selector.

## 3.1 When to use `run`

Use `run` when the query is:

* ad hoc;
* one matcher at a time;
* not dependent on a project ruleset;
* useful as a candidate-discovery step for an agent;
* a small one-off codemod;
* being debugged before promotion into a YAML rule.

Promote to `scan` when the requirement needs reusable relational/composite logic, rule metadata, tests, severity, suppressions, or a durable fix policy.

## 3.2 Pattern mode: `-p/--pattern`

```bash
ast-grep run -p 'console.log($ARG)' src/
```

The pattern is **code**, not a regex. `$ARG` is a structural metavariable.

Variable-arity example:

```bash
ast-grep run -p 'console.log($$$ARGS)' src/
```

`$$$ARGS` matches zero or more nodes in a list-like grammar position.

## 3.3 Kind/selector mode: `-k/--kind`

Exact Tree-sitter kind:

```bash
ast-grep run -k call_expression -l ts src/
```

ESQuery-style selector:

```bash
ast-grep run -k 'call_expression > identifier' -l ts src/
```

This is especially valuable when an agent already knows the grammar shape from `--debug-query=cst` or Tree-sitter tooling and does not need a code-shaped pattern.

### Decision rule: `--pattern` vs `--kind`

Use `--kind` when node **type/relationship** is sufficient. Use `--pattern` when concrete syntax, captures, repeated metavariable equality, or rewrite interpolation matters.

## 3.4 Language selection: `-l/--lang`

```bash
ast-grep run -l python -p 'print($$$ARGS)' .
ast-grep run -l rust   -p 'Some($X)' crates/
ast-grep run -l ts     -k 'class_declaration > identifier' src/
```

For path input, extension inference can normally choose the parser. Explicit language is recommended when:

* debugging the query parse;
* file extensions are non-standard;
* using stdin;
* a language glob can map an extension unexpectedly;
* a custom language is registered.

## 3.5 `--selector`: extract the actual node from a larger pattern

Sometimes the snippet you want cannot be parsed validly in isolation. Give Tree-sitter enough context, then choose the sub-node kind to become the matcher:

```bash
ast-grep run \
  -l dart \
  -p 'void _() { debugPrint($$$ARGS); }' \
  --selector call_expression \
  lib/
```

Equivalent durable YAML uses a pattern object:

```yaml
rule:
  pattern:
    context: 'void _() { debugPrint($$$ARGS); }'
    selector: call_expression
```

## 3.6 Query parsing introspection: `--debug-query`

Requires explicit `--lang`.

```bash
ast-grep run -l ts -p 'a?.b()' --debug-query=pattern
ast-grep run -l ts -p 'a?.b()' --debug-query=ast
ast-grep run -l ts -p 'a?.b()' --debug-query=cst
ast-grep run -l ts -p 'a?.b()' --debug-query=sexp
```

Use:

| Format | Best for |
|---|---|
| `pattern` | ast-grep's internal pattern/metavariable interpretation |
| `ast` | named node kinds only |
| `cst` | named + unnamed nodes; punctuation/operators |
| `sexp` | compact shape comparison |

**Agent rule:** if a pattern does not match, inspect its parse **before** making it more complex.

## 3.7 Match strictness

```bash
ast-grep run -l ts -p 'foo($X)' --strictness smart .
```

Current CLI values:

| Mode | Practical meaning |
|---|---|
| `cst` | exact syntax-tree detail; all nodes participate |
| `smart` | default; ignores source-trivial target nodes while retaining useful pattern structure |
| `ast` | match named AST nodes |
| `relaxed` | AST-oriented match with comments ignored |
| `signature` | structural named-node signature; comments and source text are ignored |
| `template` | similar to `smart`, but **text is matched while node kinds are ignored** |

`template` is a modern option added in 0.39. Some older generated rule-reference pages still omit it even though the current CLI accepts it.

### Strictness advice

Do not mechanically choose the “strictest” mode. Choose the **minimum invariants the refactor actually depends on**. Overly strict patterns create false negatives after formatting or harmless syntax variation; overly permissive patterns can conflate semantically distinct constructs.

## 3.8 One-off rewrite: `-r/--rewrite`

```bash
ast-grep run \
  -l js \
  -p 'var $X = $Y' \
  -r 'let $X = $Y' \
  src/
```

Without an apply flag, treat the result as a proposed edit/diff surface.

Review interactively:

```bash
ast-grep run -l js -p 'var $X = $Y' -r 'let $X = $Y' -i src/
```

Apply all:

```bash
ast-grep run -l js -p 'var $X = $Y' -r 'let $X = $Y' -U src/
```

For durable or multi-condition codemods, encode the matcher and `fix` in YAML and test it before `scan -U`.

## 3.9 Threads: `-j/--threads`

```bash
ast-grep run -p 'foo($X)' -j 8 .
```

`0` is the heuristic/default mode. Manual values are useful for:

* CI CPU quotas;
* avoiding competition with other workers;
* benchmarking;
* I/O-constrained network filesystems.

Do not assume more threads always improve throughput.

## 3.10 Context lines and headings

Context flags are:

```text
-A, --after <NUM>    lines after each match
-B, --before <NUM>   lines before each match
-C, --context <NUM>  symmetric context around each match
```

Examples:

```bash
ast-grep run -p 'dangerous($X)' --context 2 .
ast-grep run -p 'dangerous($X)' --after 3 --before 1 .
ast-grep run -p 'foo($X)' --heading=always .
```

`-C/--context` conflicts with separately specifying before/after context.

Heading modes:

```text
auto | always | never
```

## 3.11 Color

```bash
ast-grep run -p 'foo($X)' --color=never .
ast-grep run -p 'foo($X)' --color=always .
```

Values:

```text
auto | always | ansi | never
```

Machine pipelines should normally use JSON rather than relying on ANSI suppression alone.

## 3.12 JSON output

```bash
# pretty JSON array
ast-grep run -p 'Some($A)' --json .

# one match object per line
ast-grep run -p 'Some($A)' --json=stream .

# compact array
ast-grep run -p 'Some($A)' --json=compact .
```

Core match data includes source text, file, range/offset information, surrounding lines, language, captures, and replacement data when applicable. The full schema is covered in section 17.

JSON conflicts with interactive edit mode.

## 3.13 `--inspect`: debug file discovery

```bash
ast-grep run -p 'foo($X)' --inspect summary .
ast-grep run -p 'foo($X)' --inspect entity .
```

Inspection is emitted to **stderr** so structured stdout can remain clean:

```bash
ast-grep run -p 'foo($X)' --json=stream --inspect entity . \
  2>ast-grep-inspect.log \
  | jq -c '.file'
```

## 3.14 Exit-code posture

`run` follows grep-like result semantics: a successful match run and a successful no-match run are distinct statuses, and actual CLI/configuration failures should be treated separately. In modern ast-grep, the intended scripting contract is **match → 0, no match → 1**, with unrelated errors no longer collapsed into the no-result status.

Agent code should still avoid writing logic that treats **every** nonzero result as “no matches.” Capture stderr and distinguish execution errors.

## 3.15 `run` recipes

### Find zero-or-more-argument calls

```bash
ast-grep run -l ts -p 'fetch($$$ARGS)' src/
```

### Find identifiers directly under calls

```bash
ast-grep run -l ts -k 'call_expression > identifier' src/
```

### Limit to selected paths

```bash
ast-grep run -l python -p 'print($$$ARGS)' \
  --globs 'src/**/*.py' \
  --globs '!src/generated/**' .
```

### Pipe one buffer

```bash
cat src/example.ts \
  | ast-grep run --stdin -l ts -p 'console.log($$$ARGS)' --json=stream
```

### Safer codemod workflow

```bash
# 1. enumerate
ast-grep run -l js -p 'oldApi($$$ARGS)' src/

# 2. preview replacement
ast-grep run -l js -p 'oldApi($$$ARGS)' -r 'newApi($$$ARGS)' src/

# 3. review interactively
ast-grep run -l js -p 'oldApi($$$ARGS)' -r 'newApi($$$ARGS)' -i src/

# 4. validate externally
git diff --check
npm test
```

## 3.16 `run` failure modes

* pattern is parsed into the wrong root node;
* pattern fragment is invalid without context;
* wrong language selected by extension;
* `$VAR` is expected to match multiple nodes;
* repeated captured metavariable unexpectedly enforces equality;
* comments/punctuation make strictness too strong;
* a required operator is unnamed and needs `$$OP` in YAML/pattern context;
* ignores/globs exclude the file before matching;
* JSON style passed without `=`;
* stdin is waiting for EOF;
* syntactic equality is mistaken for resolved-symbol equality.

### Agent checklist

```text
[ ] Can this be an exact kind/ESQuery query instead of a pattern?
[ ] Is the language explicit while debugging?
[ ] Did --debug-query show the expected root and child kinds?
[ ] Is variable arity represented with $$$NAME?
[ ] Is repeated metavariable equality intentional?
[ ] Is strictness chosen from required invariants, not habit?
[ ] Are globs/ignores verified with --inspect when results are surprising?
[ ] Is a rewrite previewed before -U?
[ ] Is semantic validation required after the syntactic candidate set?
```

---

# 4) `ast-grep scan` — repository rules, linting, CI, and batch codemods

## 4.0 Contract

```bash
ast-grep scan [OPTIONS] [PATHS]...
```

`PATHS` defaults to `.`.

`scan` is the multi-rule/project execution engine. It adds rule loading, severity, structured diagnostics, fixes, suppression semantics, CI formats, and repository configuration on top of the same structural matcher core.

## 4.1 Rule-loading modes

### Mode A — project rules

```bash
ast-grep scan
```

Loads rules under directories configured by `sgconfig.yml`.

### Mode B — one rule file

```bash
ast-grep scan --rule rules/no-eval.yml src/
# short form
ast-grep scan -r rules/no-eval.yml src/
```

Useful for isolated debugging and ad-hoc full-YAML execution. `--rule` conflicts with project `--config` routing.

### Mode C — inline rule documents

```bash
ast-grep scan --inline-rules "$(cat <<'YAML'
id: no-alert
language: JavaScript
rule:
  pattern: alert($MSG)
severity: warning
message: Avoid alert.
YAML
)" src/
```

Multiple rule documents can be separated by `---`.

### Mode D — filtered project subset

```bash
ast-grep scan --filter '^security\.' .
```

`--filter` matches rule IDs by regex and is useful for CI slices or debugging a rule family.

## 4.2 Config discovery

Normal project operation starts from `sgconfig.yml`. Use:

```bash
ast-grep scan --inspect summary
```

to verify which project root/config was actually resolved.

Explicit config:

```bash
ast-grep scan --config tooling/ast-grep/sgconfig.yml
```

## 4.3 Traversal controls

`scan` shares the familiar routing surface:

```bash
ast-grep scan --follow .
ast-grep scan --no-ignore hidden --no-ignore vcs .
ast-grep scan --globs 'src/**' --globs '!src/generated/**' .
ast-grep scan -j 8 .
```

Use path selection to reduce work **before** adding complicated matcher optimizations.

## 4.4 Human report styles

```bash
ast-grep scan --report-style rich
ast-grep scan --report-style medium
ast-grep scan --report-style short
```

Broadly:

* `rich`: context-rich terminal diagnostics;
* `medium`: condensed diagnostic detail;
* `short`: minimal location/severity/message style.

## 4.5 CI formats

GitHub annotation format:

```bash
ast-grep scan --format github
```

SARIF:

```bash
ast-grep scan --format sarif > ast-grep.sarif
```

Use SARIF when integrating with a code-scanning ingestion pipeline; use `github` when direct workflow annotations are sufficient.

## 4.6 JSON

```bash
ast-grep scan --json
ast-grep scan --json=stream
ast-grep scan --json=compact
```

Add rule metadata:

```bash
ast-grep scan --json=stream --include-metadata
```

`--include-metadata` requires JSON mode.

## 4.7 Runtime severity overrides

Current scan supports:

```text
--error
--warning
--info
--hint
--off
```

Specific rule IDs are passed with `=` and flags can be repeated:

```bash
ast-grep scan \
  --error=security.no-eval \
  --error=security.no-sql-injection

ast-grep scan --off=noisy.experimental.rule
```

A flag without a rule ID applies the severity override broadly:

```bash
ast-grep scan --warning
```

This is valuable for separating **rule definition** from **deployment policy**. A rule can be informational locally but gating in a specific CI workflow.

## 4.8 Severity and exit behavior

The important modern distinction is not merely “any match vs no match”; **error-level findings are the intended CI-gating severity**. Warnings/info/hints can report findings without being used as hard failure policy.

Therefore, for CI:

```text
rule finding semantics    -> YAML rule
organizational gate       -> severity + CLI override
process failure semantics -> scan exit code
```

Do not infer gate policy from “a rule matched” alone.

## 4.9 Applying fixes

Interactive:

```bash
ast-grep scan -i
```

Apply all eligible fixes:

```bash
ast-grep scan -U
```

For a repository-wide codemod, a robust sequence is:

```bash
ast-grep test
ast-grep scan --json=stream > /tmp/findings.ndjson
ast-grep scan -i
# or after review:
ast-grep scan -U
git diff --check
<formatter>
<typecheck>
<tests>
```

## 4.10 Context around diagnostics

`scan` exposes the same long/short context controls:

```text
-A, --after <NUM>
-B, --before <NUM>
-C, --context <NUM>
```

```bash
ast-grep scan --context 2
ast-grep scan --before 1 --after 3
```

These are primarily human-output controls. Prefer structured ranges in machine pipelines.

## 4.11 Scan inspection

```bash
ast-grep scan --inspect summary
ast-grep scan --inspect entity
```

Use inspection to answer:

* which config/project was discovered;
* how many files/rules were considered;
* why an entity was included/skipped;
* which effective severity a rule received.

Because inspect goes to stderr, preserve clean machine output:

```bash
ast-grep scan --json=stream --inspect entity \
  2> ast-grep-inspect.log \
  | jq -c '{file, ruleId, range}'
```

## 4.12 Built-in suppression-related rules

Modern ast-grep includes built-in support around suppression hygiene, notably:

* `unused-suppression` — find suppression comments that suppress nothing;
* `no-suppress-all` — forbid blanket suppressions when policy requires rule-specific exceptions.

Full behavior is in section 16.

## 4.13 A production rule lifecycle

```text
1. author a minimal matcher with run
2. inspect query parse
3. move to YAML only when needed
4. isolate with scan -r
5. add valid/invalid tests
6. add snapshot when reporting/fix spans matter
7. run against representative repository paths
8. set severity policy
9. enable CI output
10. only then enable automatic -U use
```

## 4.14 Scan anti-patterns

* a monolithic `any:` containing unrelated rule concerns;
* global `stopBy: end` everywhere when a neighbor/direct-child relation suffices;
* using `regex` to recreate a parser inside a syntax matcher;
* broad file scanning when language/path scope is known;
* enabling `-U` before snapshotting/validating fixes;
* using warning/info findings as if their mere presence necessarily means CI should fail;
* ignoring `--inspect` when config discovery is ambiguous in a monorepo;
* treating syntax-only findings as confirmed semantic bugs.

### Agent checklist

```text
[ ] Is project mode actually needed, or will run / scan -r be clearer?
[ ] Is sgconfig discovery deterministic from the invocation CWD?
[ ] Can path/language scope remove most files before parsing?
[ ] Are rules independently testable?
[ ] Are metavariable-dependent checks ordered with all: where needed?
[ ] Is severity a deployment policy rather than accidental YAML default?
[ ] Are fixes tested before bulk application?
[ ] Is CI output selected for the consuming system?
[ ] Is --inspect available in troubleshooting scripts?
```


---

# 5) `ast-grep outline` — low-token syntax outline for coding agents

## 5.0 Why `outline` is strategically important

`outline` is the most consequential new ast-grep capability for coding-agent workflows in the 0.44–0.45 line. It produces a syntax-aware **table of contents** for source without requiring the consumer to read full file bodies.

It is intended for real use but remains a comparatively new surface: the project describes the command as usable while language coverage and extraction rules can still evolve from feedback.

### What it returns conceptually

Two levels:

1. **items** — top-level declarations/edges such as imports, functions, classes, structs, interfaces, modules, enums;
2. **members** — direct syntactic children of items such as methods, fields, constructors, enum variants, or module children.

Items can have import/export flags. Members can have public/private information where the language extractor can determine it syntactically.

### What it does *not* do

`outline` does **not**:

* resolve references;
* infer types;
* follow re-export chains;
* associate receiver methods semantically across arbitrary constructs;
* build a call graph;
* create a persistent index.

It is a **local syntax summary**, not a semantic symbol database.

## 5.1 Contract

```bash
ast-grep outline [OPTIONS] [PATHS]...
```

Paths default to `.`.

## 5.2 Default behavior depends on input type

| Input | Default `--items` | Default `--view` | Intended task |
|---|---|---|---|
| one file | `structure` | `digest` | understand local structure + members |
| directory | `exports` | `names` | map public surface cheaply |
| stdin | `structure` | `digest` | summarize piped source |

If input mixes files and directories, directory defaults apply to the command.

This default selection is deliberately agent-friendly: **directory scan → small public map; file scan → richer local digest**.

## 5.3 `--items`: choose top-level entry class

```bash
ast-grep outline src/parser.ts --items structure
ast-grep outline src           --items exports
ast-grep outline src/parser.ts --items imports
ast-grep outline src/parser.ts --items all
```

Values:

| Value | Meaning |
|---|---|
| `auto` | choose based on file/directory/stdin |
| `structure` | local top-level structure excluding items marked import |
| `exports` | entries marked exported/public by extractor |
| `imports` | entries marked import/dependency |
| `all` | all extracted top-level items, including import/export edges |

Export classification is **syntax-only**. It can recognize constructs such as `export`, `pub`, or language-specific public syntax when extractors support them, but it does not resolve a re-export across modules.

## 5.4 `--view`: control token/detail level

```bash
ast-grep outline src --view names
ast-grep outline src/parser.ts --view signatures
ast-grep outline src/parser.ts --view digest
ast-grep outline src/parser.ts --view expanded
```

| View | Output | Recommended use |
|---|---|---|
| `names` | names grouped by symbol type | directory map / lowest-token navigation |
| `signatures` | one signature/source line per item | API skim |
| `digest` | signatures + compact member-name groups | default file understanding |
| `expanded` | item + direct-member signatures/lines | targeted symbol inspection |
| `auto` | directory→names; file/stdin→digest | default |

### Agent token-budget rule

Use the smallest view that answers the planning question:

```text
unknown subtree -> names
known file, unknown structure -> digest
known API surface -> signatures
known type, need methods/fields -> expanded + --match
```

Do not read a 2,000-line file merely to discover its class names.

## 5.5 `--match`: top-level regex filter

```bash
ast-grep outline src/parser.ts --match Parser
```

Properties:

* Rust regular expression;
* case-sensitive by default;
* applied to top-level **name, signature, and first source line**;
* invalid regex is a CLI error;
* **does not directly filter members**.

This distinction matters. To find a method name directly, `outline --match methodName` may not work if the method is only a member; first select its parent item or use structural `run`/semantic tooling.

## 5.6 `--type`: top-level symbol-type filter

```bash
ast-grep outline crates --type struct,enum,interface
ast-grep outline src/parser.ts --match Parser --type class
```

Values are lower-camel symbol names compatible with LSP `SymbolKind` vocabulary, such as:

```text
file module namespace package class method property field constructor enum
interface function variable constant key enumMember struct operator typeParameter
```

`--type` selects **top-level items only**. Asking for `--type method` does not magically enumerate class methods when they exist only as members.

## 5.7 `--pub-members`

```bash
ast-grep outline src/lib.rs --view expanded --pub-members
```

In member-bearing views, keep only members classified as public/external.

Important default: if an extractor does **not** provide an `isPublic` rule, the member is treated as public. Thus this flag is only as precise as the language extractor's syntax model.

## 5.8 Language, config, and stdin

```bash
ast-grep outline src -l rust
ast-grep outline src -c tooling/sgconfig.yml
cat src/parser.ts | ast-grep outline --stdin -l ts
```

`outline` consults project config for custom language registration and custom language outline rules.

## 5.9 Custom extractor loading

Add extractors to bundled/default ones:

```bash
ast-grep outline src --outline-rules tooling/my-outline.yml
```

Replace bundled extractors entirely:

```bash
ast-grep outline src \
  --no-default-outline-rules \
  --outline-rules tooling/my-outline.yml
```

Each outline rule file is a **stream of YAML documents**, separated with `---`.

## 5.10 Traversal and concurrency

`outline` also supports:

```bash
--globs <GLOB>
--follow
--no-ignore <layer>
-j, --threads <NUM>
```

Examples:

```bash
ast-grep outline src \
  --globs '**/*.rs' \
  --globs '!**/generated/**'

ast-grep outline src -j 8
```

0.44-era implementation work moved outline extraction to a streaming iterator/single traversal, and 0.44.1 tightened queue bounds. The design is intentionally suitable for large tree scans rather than constructing an unconstrained all-files intermediate representation first.

## 5.11 Text output shape

Names:

```text
src/parser.ts
class: Parser
function: parseRule, parsePattern
```

Signatures:

```text
src/parser.ts
12: export function parseRule(...)
40: export class Parser
```

Digest:

```text
src/parser.ts
12: export function parseRule(...)
40: export class Parser
    methods: parse, recover
```

Expanded:

```text
src/parser.ts
12: export function parseRule(...)
40: export class Parser
44:   parse(...)
73:   recover(...)
```

For direct file/stdin input, an empty selection prints an explicit `nothing found` block. Directory walking suppresses empty files; a fully empty directory/mixed selection gets one command-level `nothing found` message.

## 5.12 JSON output

```bash
ast-grep outline src --json
ast-grep outline src --json=compact
ast-grep outline src --json=stream
```

`stream` emits **one file object per line**.

Conceptual schema:

```ts
type OutlineJsonOutput = OutlineFile[]

interface OutlineFile {
  path: string
  language: string
  items: OutlineItem[]
}

interface OutlineEntry {
  symbolType: string
  name: string
  range: {
    byteOffset: { start: number; end: number }
    start: { line: number; column: number }
    end: { line: number; column: number }
  }
  signature: string
  astKind: string
}

interface OutlineItem extends OutlineEntry {
  role: 'item'
  isImport: boolean
  isExported: boolean
  members?: OutlineMember[]
}

interface OutlineMember extends OutlineEntry {
  role: 'member'
  isPublic: boolean
}
```

Positions are zero-based. Byte offsets and line/column positions should be preserved as machine coordinates rather than re-derived from displayed text.

## 5.13 Coding-agent recipes

### Map an unfamiliar directory

```bash
ast-grep outline src --items exports
```

### Limit to structural declarations

```bash
ast-grep outline src --type class,struct,interface,enum,function
```

### Zoom into one type without opening full source

```bash
ast-grep outline src/parser.ts \
  --match '^Parser$' \
  --type class \
  --view expanded
```

### Inspect dependencies

```bash
ast-grep outline src --items imports --view signatures
```

### Review structural consequences of local edits

```bash
git diff --name-only --diff-filter=ACMR -z HEAD \
  | xargs -0 -r ast-grep outline --items exports
```

The NUL-delimited form preserves filenames safely. Filter non-source paths further when the changed set spans generated/assets files.

### Build a machine-readable symbol prefilter

```bash
ast-grep outline src \
  --items all \
  --json=stream \
  | jq -c '. as $f | .items[] | {file: $f.path, name, symbolType}'
```

`--json=stream` emits one `OutlineFile` object per line; preserve its enclosing `.path` when flattening items.

## 5.14 When outline should *not* be your only discovery tool

Use `rg`/text search in parallel when searching:

* comments/docstrings;
* configuration strings;
* dynamically generated names;
* arbitrary literals not represented by outline extractors.

Use `run`/`scan` when searching:

* statements or expressions;
* internal call sites;
* nested constructs beyond direct outlined members;
* syntax relations.

Use compiler/LSP/CPG when searching:

* definitions/references by identity;
* type implementations;
* callers/callees;
* cross-module semantic dependency.

## 5.15 Version-sensitive outline notes

The outline command landed as an alpha surface in 0.44 and has evolved rapidly:

* 0.44: initial CLI, streaming file JSON, extraction performance work;
* 0.44.1: more bundled rules and custom-language outline-rule loading;
* 0.45.0: ambient TypeScript module support;
* 0.45.1: C# multiline member signature correctness improvement.

If exact extraction fidelity is critical, add project-specific fixture tests around the text/JSON shape you depend on instead of assuming all language grammars expose identical concepts.

## 5.16 Exit semantics

`outline` deliberately differs from grep-like search exit behavior:

```text
0  command completed successfully, including an empty outline
2  fatal read, parse, or configuration error
```

Invalid CLI arguments are rejected by clap. Therefore an empty structural result is **data**, not a failure signal. Automation that needs to distinguish “nothing found” from “could not analyze input” should branch on output content plus the exit code rather than treating every empty result as an error.

### Agent checklist

```text
[ ] Is the question local syntax structure rather than semantic identity?
[ ] For a directory, can default exports/names answer it cheaply?
[ ] For a file, can digest avoid reading the body?
[ ] Does --match target an item, not a member?
[ ] Does --type target top-level entries only?
[ ] Is public/export status syntax-only acceptable?
[ ] Are JSON ranges preserved if another tool will consume the result?
[ ] Is custom extraction needed for a custom grammar/project idiom?
[ ] If outline is incomplete, is the fallback run/rg/compiler-based search explicit?
```

---

# 6) Pattern language — Tree-sitter parsing, metavariables, selectors, strictness

## 6.0 First principle: patterns are parsed as code

A pattern such as:

```text
console.log($ARG)
```

is converted into a Tree-sitter parse for the target language and then interpreted as a structural matcher. It is **not** textual wildcard syntax layered on regex.

Therefore:

* grammar validity matters;
* parser version matters;
* surrounding context can matter;
* an identifier in one grammar position may parse as a different node kind elsewhere;
* syntax error recovery is not a stable substitute for a valid pattern.

## 6.1 Pattern compilation mental model

Conceptually:

```text
pattern text
  -> preprocess metavariable markers for parser compatibility
  -> Tree-sitter parse using target grammar
  -> identify effective pattern node
  -> optional selector extracts sub-node
  -> convert metavariable-shaped nodes/tokens into match wildcards
  -> structural matcher
```

The practical consequence: when a pattern surprises you, inspect **what was parsed** rather than reasoning only from what the source string looks like.

## 6.2 Single metavariable: `$NAME`

A standard metavariable:

* starts with `$`;
* is followed by uppercase letters, `_`, and digits under ast-grep's metavariable naming rules;
* matches one named syntax node;
* captures the matched syntax for reuse unless it is a non-capturing underscore variable.

Examples:

```text
$A
$ARG
$META_VAR
$META1
$_
$_TEMP
```

Avoid lowercase names such as `$value`; they are not ordinary ast-grep metavariables.

### One node means one node

```bash
ast-grep run -p 'console.log($ARG)' -l js .
```

matches a call shape with one argument node. It does not mean “arbitrary argument list.”

## 6.3 Multi metavariable: `$$$` / `$$$NAME`

Use for zero-or-more consecutive syntax nodes in a list-like position:

```bash
ast-grep run -p 'console.log($$$ARGS)' -l js .
```

This can match:

```js
console.log()
console.log(x)
console.log(a, b, c)
```

Named form captures the sequence for fixes/JSON:

```text
$$$ARGS
```

Unnamed form:

```text
$$$
```

### Root restriction in modern versions

0.44 tightened correctness by **rejecting a multi-metavariable as the root pattern**. A bare root like:

```text
$$$A
```

is not a meaningful unconstrained pattern. Put a multi metavariable inside a surrounding construct/list position.

## 6.4 Repeated captured metavariables enforce equality

```bash
ast-grep run -p '$A == $A' -l js .
```

matches structurally repeated equal capture text/subtrees such as:

```js
x == x
(a + b) == (a + b)
```

but not:

```js
x == y
```

This is a powerful constraint, but it is still **syntactic equality**, not resolved-symbol identity.

## 6.5 Non-capturing metavariables: `$_...`

Names beginning with `_` are non-capturing:

```text
$_
$_ARG
$_FUNC
```

Benefits:

* repeated occurrences do not impose same-capture equality;
* no capture bookkeeping when you do not need the value;
* communicates that a wildcard is intentionally throwaway.

Example:

```bash
ast-grep run -p '$_FUNC($_FUNC)' -l js .
```

The two appearances need not denote identical syntax because the variable is non-capturing.

## 6.6 Unnamed-node capture: `$$VAR`

Tree-sitter operators/punctuation are often unnamed nodes. To capture one:

```yaml
rule:
  kind: binary_expression
  has:
    field: operator
    pattern: $$OP
```

Using `$OP` there can fail because ordinary metavariables target named nodes.

Use `--debug-query=cst` to see unnamed tokens.

## 6.7 Multi-metavariable consumption / anchoring

A `$$$NAME` in a sequence can consume multiple nodes. The surrounding pattern constrains where consumption ends. To avoid surprises:

* put a clear structural token/node **after** the multi capture when a suffix matters;
* avoid multiple unconstrained multi captures adjacent to each other unless you have verified parse/match behavior;
* test empty, one-item, and many-item cases.

## 6.8 Pattern objects: make parsing explicit

YAML `pattern` can be a string:

```yaml
pattern: console.log($ARG)
```

or an object:

```yaml
pattern:
  context: 'class A { $FIELD = $INIT }'
  selector: field_definition
  strictness: smart
```

Use the object form when:

* the desired fragment is not valid as a standalone file;
* the grammar parses the fragment ambiguously;
* you need a particular inner node to be the actual target;
* per-pattern strictness should be explicit.

### JSON pair example

This fragment may not parse as the intended standalone JSON node:

```text
"a": 123
```

Provide context:

```yaml
pattern:
  context: '{ "a": 123 }'
  selector: pair
```

## 6.9 `selector` semantics

A selector identifies the node kind inside the parsed pattern context that becomes the actual pattern target.

Fast workflow:

```bash
ast-grep run -l <lang> -p '<context>' --debug-query=cst
ast-grep run -l <lang> -p '<context>' --selector <kind> <paths>
```

Durable rule:

```yaml
rule:
  pattern:
    context: '<context>'
    selector: <kind>
```

## 6.10 Strictness deep dive

### `cst`

Treat complete syntax detail as meaningful. Use when punctuation/token structure is genuinely part of the contract.

Risk: fragile across formatting/trivia variation.

### `smart` — default

Balanced code-pattern behavior: preserve important pattern structure while ignoring target-side source-trivial nodes that should not ordinarily defeat a match.

Use first for most code-shaped patterns.

### `ast`

Restrict matching to named AST nodes. Useful when grammar-level constructs matter but token-level details do not.

### `relaxed`

AST-oriented matching with comments ignored. Appropriate when comment presence should never decide a match.

### `signature`

Ignores comments and source text, emphasizing structural node shape/kinds. Useful for highly structural signatures but can be much more permissive about identifier/literal text.

### `template`

Modern mode described by the CLI as similar to `smart`, but matching **text while ignoring node kinds**. Use when template text alignment matters more than exact grammar-kind identity.

Because some generated reference pages have not caught up with this sixth mode, test a version-pinned fixture if it is central to a production rule.

## 6.11 Invalid-code/error recovery is not a portability contract

Tree-sitter can recover from syntax errors, which sometimes makes incomplete patterns appear to work. That is useful for exploration but dangerous for durable rules because parser upgrades can change error-recovery shape.

Prefer:

```yaml
pattern:
  context: <valid code>
  selector: <desired node>
```

over depending on an `ERROR` parse when production correctness matters.

## 6.12 Pattern authoring protocol for agents

```text
1. identify language
2. write the smallest valid example
3. run --debug-query=cst
4. identify actual target kind/fields
5. choose -k if kind/relationship is sufficient
6. otherwise add metavariables
7. test zero/one/many sequence cases
8. test expected false positives
9. if fragment is ambiguous, add context + selector
10. only then add relational/composite YAML logic
```

## 6.13 Pattern anti-patterns

* treating `$X` as regex `.*`;
* using a bare root `$$$X`;
* lowercase metavariable names;
* relying on malformed syntax when valid context is available;
* repeated `$A` when equality was not intended;
* expecting `$OP` to capture an unnamed operator;
* using strictness changes to compensate for a wrongly parsed root;
* assuming identical source text implies identical semantic symbol.

---

# 7) Rule object — atomic, relational, composite, and ESQuery matching

## 7.0 The rule object model

A rule object has fields from three groups:

**Atomic**

```text
pattern
kind
regex
nthChild
range
```

**Relational**

```text
inside
has
precedes
follows
```

**Composite**

```text
all
any
not
matches
```

A rule needs at least one positive matching component. The fields present in one rule object are logically combined, but **field evaluation order should not be relied upon**. When capture ordering matters, use explicit `all:`.

## 7.1 Implicit field AND vs explicit `all`

These express a similar logical conjunction:

```yaml
rule:
  pattern: this.foo
  inside:
    kind: class_body
```

```yaml
rule:
  all:
    - pattern: this.foo
    - inside:
        kind: class_body
```

But explicit `all` has a crucial property: it supplies **ordered sub-rule evaluation**, so an earlier rule can capture a metavariable before a later relation consumes it.

### Rule

If `$A` captured in condition 1 is referenced by condition 2, write:

```yaml
all:
  - <capture A>
  - <use A>
```

Do not rely on YAML/object field insertion order.

## 7.2 Atomic: `pattern`

String:

```yaml
rule:
  pattern: console.log($ARG)
```

Object:

```yaml
rule:
  pattern:
    context: class A { $FIELD = $INIT }
    selector: field_definition
    strictness: relaxed
```

Pattern gives the strongest correspondence to “write code that looks like the desired shape.”

## 7.3 Atomic: `kind`

Exact kind:

```yaml
rule:
  kind: call_expression
```

ESQuery-style selector:

```yaml
rule:
  kind: call_expression > identifier
```

Use the playground or `--debug-query` to discover grammar kinds.

## 7.4 Atomic: `regex`

```yaml
rule:
  kind: identifier
  regex: '^[a-z]+$'
```

Properties:

* Rust regex engine;
* applied to node text;
* reference semantics are whole-node-text matching rather than arbitrary stream scanning;
* Rust regex intentionally lacks constructs such as lookaround/backreferences.

`regex` is best used to refine a syntactically grounded candidate. Avoid building a structural parser out of regex when `pattern`/`kind` can establish node meaning first.

## 7.5 Atomic: `nthChild`

Exact one-based named-child position:

```yaml
rule:
  kind: number
  nthChild: 3
```

An+B:

```yaml
rule:
  nthChild: 2n+1
```

Object form:

```yaml
rule:
  nthChild:
    position: 2n+1
    reverse: true
    ofRule:
      kind: function_declaration
```

Key rules:

* one-based, CSS-like indexing;
* counts **named** siblings;
* `ofRule` filters siblings before positional calculation;
* `reverse` counts from the opposite end.

0.44 fixed metavariable binding leakage across siblings evaluated in `nthChild.ofRule`; do not reproduce older workarounds against current behavior without retesting.

## 7.6 Atomic: `range`

```yaml
rule:
  range:
    start: { line: 0, column: 0 }
    end:   { line: 0, column: 3 }
```

Coordinates are zero-based; start is inclusive and end is exclusive.

This is especially valuable when integrating another analyzer:

```text
compiler/type checker reports source range
  -> ast-grep range rule selects syntax node at range
  -> ast-grep structural rewrite applies local edit
```

Prefer stable byte/range interoperability from structured APIs when available rather than manually converting displayed line numbers.

## 7.7 Relational rule mental model

Read a relational rule as:

```text
TARGET  relation  SURROUNDING-MATCH
```

Examples:

* `inside`: target is inside an ancestor;
* `has`: target has a matching child/descendant;
* `precedes`: target occurs before a matching sibling/neighbor sequence;
* `follows`: target occurs after one.

The relational constraint narrows the **target node**; it does not change which node is reported unless you change the root rule.

## 7.8 `stopBy`

Relational traversal accepts a stop policy:

```yaml
has:
  pattern: $MY_PATTERN
  stopBy: end
```

Modes:

| `stopBy` | Meaning |
|---|---|
| `neighbor` | default; only immediate relation step |
| `end` | continue to relation boundary/root/leaf/edge |
| rule object | continue until this stop rule is encountered |

Custom boundary:

```yaml
inside:
  stopBy:
    kind: function_declaration
  pattern: class $C { $$$BODY }
```

The stop check is inclusive in relational matching semantics: a node satisfying both the desired surrounding condition and stop boundary can still count.

### Performance/correctness rule

Prefer `neighbor` when direct relation is the invariant. Use `end` only when arbitrary depth is intentional. A broad `stopBy: end` can both increase work and admit surprising distant structure.

## 7.9 `field` for `inside` / `has`

Scope relation to a grammar field:

```yaml
rule:
  kind: pair
  has:
    field: key
    regex: '^prototype$'
```

This distinguishes object **keys** from identical text in values.

`field` is supported for `inside` and `has`, not the sibling-order relations `precedes`/`follows`.

## 7.10 `inside`

```yaml
rule:
  pattern: await $EXPR
  inside:
    any:
      - kind: for_statement
      - kind: while_statement
    stopBy: end
```

Use custom stop boundaries to avoid climbing through a nested function when the rule is intended to remain within one execution scope.

## 7.11 `has`

Direct-child search by default:

```yaml
rule:
  kind: call_expression
  has:
    field: function
    pattern: console.log
```

Descendant search:

```yaml
rule:
  kind: function_declaration
  has:
    pattern: dangerous($$$ARGS)
    stopBy: end
```

Be explicit about whether arbitrary-depth descendants are actually intended.

## 7.12 `precedes` / `follows`

```yaml
rule:
  pattern: console.log('hello')
  follows:
    pattern: console.log('world')
```

Extended sibling search:

```yaml
rule:
  kind: expression_statement
  precedes:
    kind: return_statement
    stopBy: end
```

These describe source-sibling relationships, not arbitrary control-flow precedence.

## 7.13 Composite: `all`

```yaml
rule:
  all:
    - pattern: foo($A)
    - inside:
        pattern: bar($A)
        stopBy: end
```

Advantages:

* conjunction;
* explicit ordering;
* safest form for capture dependency.

## 7.14 Composite: `any`

```yaml
rule:
  any:
    - pattern: var $A = $B
    - pattern: let $A = $B
    - pattern: const $A = $B
```

Use for alternatives that report the same conceptual target.

## 7.15 Composite: `not`

```yaml
rule:
  pattern: console.log($X)
  not:
    pattern: console.log('allowed')
```

0.44 fixed metavariable environment leakage from negated rules. Treat `not` as a predicate that must not export captures for later positive logic.

Also remember: top-level `constraints` are evaluated **after** the primary rule match, so they cannot be used to retroactively make a metavariable inside `not` mean something different during negation.

## 7.16 Composite: `matches`

Reference a utility rule:

```yaml
rule:
  matches: some-utility-id
```

Modern parameterized global utilities can also receive rule-valued arguments; section 9 covers the exact scoping/export model.

## 7.17 One-node-at-a-time composite semantics

This does **not** mean “has a number and a string somewhere”:

```yaml
has:
  all:
    - kind: number
    - kind: string
```

It asks for one descendant node that is simultaneously both kinds, so it cannot match.

Correct form:

```yaml
all:
  - has:
      kind: number
  - has:
      kind: string
```

Each `has` can succeed on a different descendant.

---

## 7.18 ESQuery-style `kind` selectors

Modern ast-grep supports a deliberately limited CSS/ESQuery-like syntax in `kind`, and since 0.43 the CLI `run --kind` accepts it directly.

### Structural combinators

| Syntax | Meaning |
|---|---|
| `A > B` | `B` direct child of `A` |
| `A B` | `B` descendant of `A` |
| `A + B` | `B` adjacent/next sibling after `A` |
| `A ~ B` | `B` later/following sibling after `A` |
| `A, B` | selector alternatives |

Examples:

```bash
ast-grep run -l js -k 'call_expression > identifier' src/
ast-grep run -l js -k 'function_declaration identifier' src/
```

### Pseudo-classes added in 0.42+

```text
:has(...)
:not(...)
:is(...)
:nth-child(An+B)
:nth-child(An+B of selector)
:nth-last-child(...)
```

Examples:

```yaml
rule:
  kind: 'call_expression:has(identifier)'
```

```yaml
rule:
  kind: 'identifier:not(property_identifier)'
```

```yaml
rule:
  kind: ':is(class_declaration, function_declaration)'
```

```yaml
rule:
  kind: 'identifier:nth-child(2n+1)'
```

### Selector limitations

Do not assume full browser CSS or full npm `esquery` compatibility. The supported syntax is intentionally constrained. In particular:

* grammar **node kinds**, not CSS classes, are the primitives;
* only documented combinators/pseudo-classes are safe;
* complex semantics still map to ast-grep relational rules rather than an unrestricted selector evaluator.

### When ESQuery is better than YAML relations

Prefer selector syntax when:

* the relation is simple and local;
* no metavariable capture is needed;
* a one-line `run -k` query is easier to audit;
* the selector accurately communicates the grammar relation.

Prefer explicit YAML when:

* relation depth needs a custom `stopBy`;
* `field` is important;
* metavariables/constraints/fixes are needed;
* a reusable lint rule requires diagnostics/tests.

## 7.19 Rule-object optimization advice

A scalable rule usually has an obvious positive anchor:

```yaml
rule:
  kind: call_expression
  has:
    ...
```

or:

```yaml
rule:
  pattern: foo($$$ARGS)
```

Broad rules composed mainly of negation/regex/relations are harder to prune and reason about. The config engine can use **potential kinds** to avoid running impossible rules on every node; give it syntactic information whenever you can.

### Agent checklist

```text
[ ] Is the reported target node the root rule I actually want?
[ ] Is there a positive kind/pattern anchor?
[ ] Does relation depth match the real invariant?
[ ] Can a grammar field make the rule more precise?
[ ] Are capture-dependent conditions explicitly ordered in all:?
[ ] Does all:/any: apply to one node, not an imagined node set?
[ ] Is not: capture-free from the caller's perspective?
[ ] Would an ESQuery selector be simpler and equally precise?
[ ] Have grammar kinds/fields been verified rather than guessed?
```


---

# 8) Lint-rule YAML — complete finding/reporting/patching/file-selection surface

## 8.0 Rule file vs rule object vs project config

Three YAML concepts must remain separate:

```text
Rule object
  = matcher expression under `rule:` or relational/composite sub-rules

Rule configuration / lint rule
  = one executable finding policy: id + language + rule + diagnostics/fix/etc.

sgconfig.yml
  = project routing: rule dirs, test dirs, utils, languages, injections
```

A common agent error is to put `sgconfig.yml` fields into a rule file or vice versa.

## 8.1 Full top-level capability map

A lint/codemod rule can use these broad field groups:

### Identity / interpretation

```text
id
language
```

### Finding

```text
rule
constraints
utils
```

### Patching

```text
transform
fix
rewriters
```

### Reporting / linting

```text
severity
message
note
labels
```

### File selection

```text
files
ignores
```

### Documentation / metadata

```text
url
metadata
```

Not every field is required. A search/codemod rule may not need a rich diagnostic message; a lint rule normally should.

## 8.2 Canonical production rule shape

```yaml
id: no-console-log
language: TypeScript

rule:
  pattern: console.log($$$ARGS)

constraints: {}

message: Avoid console.log in production code.
severity: warning
note: |
  Use the project logger so output receives the normal metadata,
  routing, and severity treatment.

labels:
  ARGS:
    style: secondary
    message: logged arguments

files:
  - 'src/**/*.ts'
  - 'src/**/*.tsx'
ignores:
  - 'src/**/__generated__/**'

fix: logger.info($$$ARGS)

metadata:
  category: observability
  autofix: review
```

The exact rule should of course reflect the repository's logger/import policy; this example illustrates field placement.

## 8.3 `id`

Use a stable, unique rule identifier:

```yaml
id: security.no-eval
```

The identifier is used by:

* filtering;
* severity overrides;
* suppressions;
* tests/snapshots;
* JSON/SARIF diagnostics;
* organizational documentation.

Modern ast-grep gained support for deriving a default rule id from a filename in some loading paths (0.41), but **explicit IDs remain the safer production convention** because renaming a file should not silently rename suppression/CI policy.

## 8.4 `language`

```yaml
language: TypeScript
```

It controls how pattern code is parsed and which source files are eligible after project/file routing.

Use a built-in language name/alias or a custom language registered in `sgconfig.yml`.

## 8.5 `rule`

Required matcher root for the finding:

```yaml
rule:
  pattern: await $_
  inside:
    any:
      - kind: for_statement
      - kind: while_statement
    stopBy: end
```

One successful rule match always has one **target node**. Auxiliary captures/relations add context but do not create multiple target nodes inside one match.

## 8.6 `constraints`

Apply additional rules to **captured single metavariables** after the root rule has matched:

```yaml
rule:
  pattern: console.log($GREET)
constraints:
  GREET:
    kind: identifier
```

Keys omit `$`.

### Constraints contract

* designed for single captures such as `$ARG`;
* not the mechanism for constraining an entire `$$$ARGS` sequence;
* evaluated after root matching;
* cannot repair the logical meaning of a `not` sub-rule during the earlier match phase.

### Negation footgun

This does not mean “console.log except string” in the way an inexperienced author may expect:

```yaml
rule:
  pattern: console.log($GREET)
  not:
    pattern: console.log($STR)
constraints:
  STR:
    kind: string
```

`not` is evaluated before the top-level constraint refines `$STR`, so the negative pattern can conflict with the positive pattern too broadly.

Put the actual kind/pattern logic **inside the negative rule** instead.

## 8.7 `utils`

Local reusable matcher components:

```yaml
utils:
  is-literal:
    any:
      - kind: string
      - kind: number
      - kind: 'true'
      - kind: 'false'

rule:
  matches: is-literal
```

Local utils:

* belong to the enclosing rule configuration;
* use the same language;
* are zero-argument;
* are rule objects, not complete lint rules;
* do not independently carry their own top-level constraints/reporting.

Global utilities are covered in section 9.

## 8.8 `message`

Concise finding text:

```yaml
message: Avoid using $FUNC here.
```

Captured/transformed metavariables can be interpolated in diagnostic-oriented template text where supported.

Good messages describe **what** is wrong; use `note` for detailed rationale/remediation.

## 8.9 `severity`

```yaml
severity: error
```

Values:

```text
error
warning
info
hint
off
```

Use severity deliberately:

* `error` — intended CI gate;
* `warning` — meaningful issue, non-gating by default policy;
* `info` — informational finding;
* `hint` — low-friction guidance / common default level;
* `off` — disabled rule, also skipped by test unless `--include-off`.

Runtime `scan --error/--warning/...` can override definition-time severity.

## 8.10 `note`

Longer explanation, preferably actionable; Markdown is supported in the reporting ecosystem:

```yaml
note: |
  This call bypasses the centralized logger.

  Prefer `logger.info(...)` when the message is informational.
```

Do not put an entire policy essay in `message` when `note` exists.

## 8.11 `labels`

Customize highlighted ranges by capture:

```yaml
labels:
  METHOD:
    style: primary
    message: method being diagnosed
  CLASS:
    style: secondary
    message: enclosing class
```

Keys omit `$`.

Styles:

```text
primary
secondary
```

### Critical range requirement

A label must correspond to a **real matched AST node**, because it needs a source range. Therefore:

* captures from `rule` / applicable constraints can be labeled;
* transformed strings created by `transform` cannot be labeled as source ranges.

The ast-grep LSP also respects labels and can expose label messages as related diagnostic information.

## 8.12 `files`

Rule-local include filter:

```yaml
files:
  - 'src/**/*.ts'
  - 'src/**/*.tsx'
```

Object syntax allows case-insensitive matching:

```yaml
files:
  - glob: 'README.md'
    caseInsensitive: true
```

Mixed list is allowed:

```yaml
files:
  - 'src/**/*.ts'
  - glob: 'readme.md'
    caseInsensitive: true
```

Paths are relative to the **project root (`sgconfig.yml` directory)**. Do not add leading `./`.

## 8.13 `ignores`

Rule-local exclusion filter:

```yaml
ignores:
  - 'src/**/__generated__/**'
  - glob: 'BUILD'
    caseInsensitive: true
```

Evaluation order:

```text
1. if `ignores` matches -> rule skipped for file
2. else, if `files` exists -> file must match at least one include
3. else -> rule is eligible
```

### `ignores` is not CLI ignore discovery

Rule `ignores` filters a rule against files that reached the rule engine. It is different from filesystem discovery controlled by `.gitignore`, `.ignore`, hidden-file policy, `--no-ignore`, and CLI `--globs`.

For global repository exclusions across many tools, `.ignore` / repository traversal policy can be more maintainable than repeating the same `ignores` stanza in every rule.

## 8.14 `url`

Associate a documentation/remediation URL with the rule when the schema/client supports it:

```yaml
url: https://internal.example/rules/security.no-eval
```

Use stable docs rather than ephemeral issue links when humans will see the diagnostic for years.

## 8.15 `metadata`

Arbitrary structured metadata that downstream JSON consumers can request with `scan --include-metadata`:

```yaml
metadata:
  category: security
  owner: platform-security
  autofix: false
  cwe: CWE-95
```

Do not place matching logic in metadata; it is descriptive/integration data.

## 8.16 Multiple rule documents in one YAML file

```yaml
id: first-rule
language: JavaScript
rule:
  pattern: alert($MSG)
message: Avoid alert.
severity: warning
---
id: second-rule
language: JavaScript
rule:
  pattern: confirm($MSG)
message: Avoid confirm.
severity: warning
```

This is useful for a tightly related migration family, but avoid giant files that make IDs/tests/ownership hard to navigate.

## 8.17 Reporting design guidance

A high-quality lint rule has four separable concerns:

```text
matcher       -> precise syntax condition
message/note  -> human explanation
severity      -> organizational enforcement
fix           -> safe mechanical remediation, if one exists
```

Do not weaken a matcher merely because the automated fix cannot handle every finding. It can be valid to report broadly and fix only a safer subcase via a separate rule.

### Agent checklist

```text
[ ] Is id explicit and stable?
[ ] Does language match how pattern strings are intended to parse?
[ ] Is the target node the best diagnostic/fix range?
[ ] Are constraints only filtering captures they can legally filter?
[ ] Is message concise and note actionable?
[ ] Do labels point to real captured AST nodes?
[ ] Are files/ignores relative to sgconfig root and free of leading ./ ?
[ ] Is caseInsensitive used only where required?
[ ] Is metadata separated from execution logic?
[ ] Is severity an intentional deployment choice?
```

---

# 9) Utility rules — local/global reuse and parameterized utilities

## 9.0 Why utilities matter

Without utilities, a serious rule catalog tends to duplicate the same syntax concepts:

```text
is literal
is public method
is test function
is logger call
is unsafe block
is React component
```

Utilities let rule authors give those concepts stable names and compose them with `matches`.

## 9.1 Local utilities

Inside one rule config:

```yaml
id: example
language: JavaScript

utils:
  is-literal:
    any:
      - kind: string
      - kind: number
      - kind: 'true'
      - kind: 'false'

rule:
  matches: is-literal
```

Local utilities are best for implementation details that should not become project-wide API.

## 9.2 Global utilities

Project layout:

```text
repo/
  sgconfig.yml
  rules/
    no-hardcoded-log.yml
  utils/
    is-literal.yml
```

Project config:

```yaml
ruleDirs:
  - rules
utilDirs:
  - utils
```

Global utility file:

```yaml
id: is-literal
language: TypeScript
rule:
  any:
    - kind: string
    - kind: number
    - kind: 'true'
    - kind: 'false'
```

Call from rule:

```yaml
rule:
  matches: is-literal
```

Global utility files intentionally support a more limited field surface than lint rules: identity/language + matching-related composition such as `rule`, `constraints`, and local `utils`, rather than severity/message/fix policy.

## 9.3 Utility naming policy

Treat global utility IDs like an internal rule API:

```text
is-expression-like
is-test-function
is-project-logger
has-public-visibility
```

Avoid names tied to one caller such as `helper1` or `thing-for-rule-27`.

## 9.4 Parameterized global utilities (0.42+, experimental)

Parameterized utilities make a global utility act like a **matcher function that accepts rule arguments**.

Definition:

```yaml
# utils/audit-log-call.yml
id: audit-log-call
arguments: [logger-rule]
language: TypeScript
rule:
  pattern: $OBJ.$METHOD($$$ARGS)
  all:
    - has:
        kind: member_expression
        has:
          field: object
          matches: logger-rule
    - has:
        field: arguments
        has:
          kind: string
```

Call site:

```yaml
rule:
  matches:
    audit-log-call:
      logger-rule:
        regex: '^console$'
```

The argument value is a **full ast-grep rule object**, not a string.

## 9.5 Parameterized utility rules

Current experimental rules:

1. only **global** utilities can declare `arguments`;
2. local `utils:` remain zero-argument;
3. all declared arguments are required — no optional parameters;
4. arguments are matcher rules;
5. parameter matches execute in an isolated metavariable environment;
6. captured exports are reconciled back only after successful parameterized matching;
7. conflicting exported captures can make the parameterized call fail.

## 9.6 Name resolution / shadowing

Inside a parameterized utility, bare:

```yaml
matches: some-name
```

resolves conceptually in lexical priority:

```text
current parameter binding
  -> local utility
  -> global zero-argument utility
```

A parameter can shadow a same-named utility.

Avoid ambiguous naming even though shadowing is defined; an LLM agent should not need to mentally execute a name-resolution puzzle to understand a lint rule.

## 9.7 Metavariable isolation and export

Parameterized utility arguments do not simply share the caller's capture map while matching. They work in temporary/isolated environments. After the whole utility succeeds, compatible captures can be exported.

Practical consequences:

* do not assume a parameter can see every caller binding implicitly;
* do not write late conditions that depend on undocumented capture leakage;
* if the caller already binds a name differently, export can fail;
* use explicit utility contracts and tests.

## 9.8 Performance implication

The project's 0.42 design notes call parameterized utilities experimental and note that their implementation can reduce static **potential-kind inference**. If the rule engine cannot infer which node kinds are possible, it may evaluate the rule more broadly.

Optimization tactic:

```yaml
rule:
  all:
    - kind: call_expression
    - matches:
        audit-log-call:
          logger-rule:
            regex: '^console$'
```

An explicit positive kind guard can restore easy pruning and also clarify intent.

## 9.9 Recursion/cycles

Do not use utilities to create recursive matcher abstractions unless explicitly documented/tested. The config system detects utility dependency cycles, and parameter names should not be mistaken for recursive utility links.

## 9.10 When to use parameterization

Good case:

```text
same structural matcher
+ one interchangeable syntactic predicate
+ multiple caller rules with different reporting policy
```

Poor case:

```text
utility accepts 6 parameters
+ every caller supplies a different rule tree
+ utility body becomes an unreadable mini-program
```

At that point, duplication or a host API may be clearer.

### Agent checklist

```text
[ ] Is reuse actually conceptual, not merely textual coincidence?
[ ] Should helper be local or a global catalog API?
[ ] Are global utility IDs stable and descriptive?
[ ] If parameterized, is the experimental status acceptable?
[ ] Are every argument and capture/export behavior covered by tests?
[ ] Is an explicit kind anchor needed for pruning?
[ ] Is name shadowing avoided even when legal?
[ ] Would a host-language function be clearer than deeply parameterized YAML?
```

---

# 10) Rewrite system — CLI rewrite, `fix`, range expansion, indentation

## 10.0 Rewrite mental model

ast-grep's core codemod model is:

```text
find target node
  -> capture metavariables
  -> optionally transform captured text/subtrees
  -> render replacement template
  -> optionally expand replacement range
  -> produce edit
  -> preview / interactively accept / apply all
```

The patch is textual at the replacement boundary even though the match is syntax-aware.

## 10.1 One-off CLI rewrite

```bash
ast-grep run -l js \
  -p 'oldApi($$$ARGS)' \
  -r 'newApi($$$ARGS)' \
  src/
```

Use `-i` for review and `-U` to apply all.

## 10.2 YAML string `fix`

```yaml
rule:
  pattern: console.log($$$ARGS)
fix: logger.log($$$ARGS)
```

The fix string is a **template string, not Tree-sitter-parsed code**. Captured metavariables are substituted into text.

This gives flexibility, but also means ast-grep does not automatically guarantee the generated replacement parses.

### Required validation

After nontrivial codemods:

```text
formatter
parser/compiler/type checker
tests
repository-specific lint
```

## 10.3 Delete a target

```yaml
fix: ''
```

replaces the target range with an empty string.

However, deleting a list element often also requires a comma/trivia range; use `FixConfig`.

## 10.4 `FixConfig`

```yaml
fix:
  template: ''
  expandEnd:
    regex: ','
```

Fields:

```text
template     required replacement template
expandStart  optional rule for extending beginning of replaced range
expandEnd    optional rule for extending end of replaced range
```

Example — remove an object pair plus a trailing comma:

```yaml
rule:
  kind: pair
  has:
    field: key
    regex: '^Remove$'
fix:
  template: ''
  expandEnd:
    regex: ','
```

## 10.5 Expansion semantics

Range expansion walks adjacent syntax/trivia according to the expansion rule and extends while the condition remains satisfied, subject to the rule's expansion boundary semantics.

Use it for:

* commas around deleted list entries;
* surrounding whitespace/trivia;
* delimiters that belong operationally to the patch but not the target AST node.

Do not broaden a target node merely to make a fix convenient; keep the diagnostic target semantically precise and use fix-range expansion for patch mechanics.

## 10.6 Multi-option interactive fixes

Modern ast-grep can attach **multiple remediation proposals to one diagnostic** and expose them in interactive/editor workflows. In `ast-grep scan --interactive`, the UI can show the alternatives, `Tab` cycles through the proposals, and `Enter` applies the selected one.

The project documents the rule shape as an array of titled fix templates:

```yaml
fix:
  - title: Use the compatibility API
    template: compatApi($$$ARGS)
  - title: Use the new API
    template: newApi($$$ARGS)
```

Use this when the detector is unambiguous but more than one repair is valid—for example, when policy permits either widening a type or making an input required.

### Documentation-drift warning

The ast-grep project blog and 0.38-era changelog document multi-fix support in the CLI/LSP, but the current standalone `fix` reference still presents the type as only `String | FixConfig`. That makes the **exact multi-fix YAML schema a version-sensitive surface** even though the capability itself is established.

For generated production rules:

```text
1. verify the target 0.45.x binary/editor accepts the array form;
2. keep each title unique and action-oriented;
3. test every proposal, not only the first;
4. do not assume --update-all has a meaningful policy for choosing among semantically distinct alternatives;
5. prefer interactive/editor selection when human judgment is part of the repair contract.
```

A single deterministic `fix` remains preferable for unattended codemods.

## 10.7 Indentation sensitivity

ast-grep preserves replacement-template indentation relative to the source insertion location.

Example:

```yaml
id: lambda-to-def
language: Python
rule:
  pattern: '$B = lambda: $R'
fix: |-
  def $B():
    return $R
```

The template's internal indentation is retained and shifted according to the replacement's source indentation context.

### Agent implication

Test at least:

```text
column 0 occurrence
nested one indentation level
nested multiple levels
multiline capture interpolation
```

for indentation-sensitive languages.

## 10.8 Missing metavariable in fix

A metavariable referenced in a fix but not matched can collapse to empty text under rewrite substitution behavior. Treat this as dangerous, not convenient.

A production rule should have tests proving every replacement capture is bound in all successful match alternatives.

## 10.9 Uppercase concatenation ambiguity

Metavariable names themselves use uppercase syntax. Writing something like:

```text
$VARName
```

can be interpreted as a longer metavariable token rather than `$VAR` + literal `Name`.

Use a `transform` to build the intended new string instead of relying on ambiguous template concatenation.

## 10.10 Rewrite safety classification

### Class A — structural substitution, low risk

```text
oldFn($$$ARGS) -> newFn($$$ARGS)
```

No evaluation-order/typing change expected beyond name substitution.

### Class B — syntax restructuring, medium risk

```text
promise.then($F) -> await ...
```

May change enclosing grammar/control structure.

### Class C — semantic migration, high risk

Changes ownership, types, imports, concurrency, exception handling, side-effect order, or cross-file declarations.

For Class C, ast-grep should usually be only the edit engine inside a broader semantic validation workflow.

## 10.11 Rewrite transaction discipline

Recommended repository automation:

```text
1. require clean/known working-tree policy
2. run rule tests
3. enumerate matches and store counts/files
4. preview or stage edits
5. apply
6. run formatter
7. re-run ast-grep query to confirm old shape removed
8. run syntax/type/compiler checks
9. run unit/integration tests
10. inspect diff size and unexpected files
11. commit only after all gates
```

### Agent anti-patterns

* `-U` on an untested new pattern;
* assuming syntax-aware match means semantically safe replacement;
* generating an import/reference without handling its scope;
* deleting list items without separators;
* using fix templates whose captures are optional across `any` branches;
* depending on malformed output being repaired by a formatter;
* applying overlapping independent codemods in one uncontrolled pass.

---

# 11) Transformations and rewriters — generated metavariables and recursive rewrites

## 11.0 Why transformations exist

A `fix` can substitute captures, but real codemods often need to derive new text:

* remove a prefix;
* extract a substring;
* convert identifier case;
* recursively rewrite syntax inside a captured subtree;
* join generated children.

`transform` creates new metavariable-like strings before rendering the fix.

## 11.1 Transform map

```yaml
transform:
  NEW_NAME:
    <transformation>:
      source: $OLD_NAME
      ...
fix: $NEW_NAME
```

Transform keys omit `$` in the map but become addressable as `$NEW_NAME` downstream.

Transforms can chain: a later transform can use output from an earlier one.

## 11.2 Supported transformation families

Current reference exposes:

```text
replace
substring
convert
rewrite
```

A concise function-like string syntax is also supported in modern versions (0.38.3+).

## 11.3 `replace`

Object form:

```yaml
transform:
  NEW_FN:
    replace:
      source: $OLD_FN
      replace: '^debug'
      by: 'release'
```

String form:

```yaml
transform:
  NEW_FN: replace($OLD_FN, replace='^debug', by='release')
```

The regular expression uses Rust regex semantics.

### Named regex captures

Inside a `replace` transformation, named regex groups can be referenced in that transformation's `by` field:

```yaml
transform:
  NEW_FN:
    replace:
      source: $OLD_FN
      replace: 'debug(?<REST>.*)'
      by: 'release$REST'
```

This capture namespace is local to the replace transformation; regular matcher `regex` does not become a general regex-capture exporter.

## 11.4 `substring`

```yaml
transform:
  INNER:
    substring:
      source: $RAW
      startChar: 1
      endChar: -1
```

String form:

```yaml
transform:
  INNER: substring($RAW, startChar=1, endChar=-1)
```

Semantics:

* `startChar` inclusive;
* `endChar` exclusive;
* negative indexes count from end;
* indexing is Unicode **character** count rather than byte count.

Do not mix substring character indexes with JSON byte offsets.

## 11.5 `convert`

```yaml
transform:
  KEBAB:
    convert:
      source: $OLD_NAME
      toCase: kebabCase
```

Supported output cases:

| `toCase` | Example |
|---|---|
| `lowerCase` | `astGrep` → `astgrep` |
| `upperCase` | `astGrep` → `ASTGREP` |
| `capitalize` | `astGrep` → `AstGrep` |
| `camelCase` | `ast_grep` → `astGrep` |
| `snakeCase` | `astGrep` → `ast_grep` |
| `kebabCase` | `astGrep` → `ast-grep` |
| `pascalCase` | `astGrep` → `AstGrep` |

Separator-sensitive conversions can customize `separatedBy` with separator classes including:

```text
Dash
Dot
Space
Slash
Underscore
CaseChange
```

Example:

```yaml
transform:
  NEW_VAR:
    convert:
      source: $VAR
      toCase: kebabCase
      separatedBy: [underscore]
```

The official transformation example uses the lowercase separator token `underscore`; follow the current transformation schema/examples when generating this field mechanically.

## 11.6 Sequential transformations

```yaml
transform:
  KEBABED:
    convert:
      source: $OLD_FN
      toCase: kebabCase
  RELEASED:
    replace:
      source: $KEBABED
      replace: '(?<ROOT>)-debug$'
      by: '$ROOT-release'
  FINAL:
    convert:
      source: $RELEASED
      toCase: camelCase
fix: $FINAL($$$ARGS)
```

Use sequencing to avoid ambiguous metavariable+uppercase literal concatenation.

## 11.7 Rewriter rules

Top-level `rewriters` define named AST rewrite operations usable from a `rewrite` transformation:

```yaml
rewriters:
  - id: remove-quotes
    rule:
      pattern: "'$A'"
    fix: $A
```

A rewriter is not a lint diagnostic. It is a patching helper used to transform a captured subtree.

## 11.8 `rewrite` transformation

**Stability:** the current transformation reference still labels `rewrite` **experimental**. Pin the ast-grep version and regression-test any production rule that depends on recursive rewriters.

```yaml
transform:
  NEW_BODY:
    rewrite:
      source: $BODY
      rewriters: [remove-quotes, another-rewriter]
```

Optional generated aggregation:

```yaml
transform:
  GENERATED:
    rewrite:
      source: $BODY
      rewriters: [to-statement]
      joinBy: "\n"
```

String form:

```yaml
transform:
  GENERATED: rewrite($BODY, rewriters=[to-statement], joinBy='\n')
```

## 11.9 Rewriter selection/overlap semantics

Important current rules:

* only rewriters listed for the transformation participate;
* within one candidate node, rewriters are tried in list order;
* **first matching rewriter wins** for that node;
* rewrite matches do not overlap;
* higher/closer-to-root nodes win before nested overlapping possibilities.

This prevents a recursive rewrite from applying incompatible edits to both a parent and its child simultaneously.

## 11.10 `joinBy`

Normally rewritten syntax is reassembled in its original structural placement. `joinBy` instead joins generated rewritten pieces with a specified string:

```yaml
joinBy: "\n"
```

Use for transformations that intentionally flatten/generate a sequence.

## 11.11 Transform vs host API

Use YAML transforms when the operation is:

* local;
* string/identifier-oriented;
* deterministic from captures;
* expressible as replace/slice/case/subtree rewrite.

Escalate to a host API when replacement requires:

* arbitrary computation;
* symbol tables/types;
* external data;
* sorting/deduplication across matches;
* cross-file coordination;
* nonlocal import management;
* complex conditional code generation.

### Agent checklist

```text
[ ] Is transformed value textual or does it need semantic knowledge?
[ ] Are Unicode character indexes distinguished from byte offsets?
[ ] Are regex captures used only inside replace transformation scope?
[ ] Are chained transform dependencies ordered clearly?
[ ] Are rewriter overlaps understood?
[ ] Is first-rewriter-wins ordering intentional?
[ ] Does joinBy preserve syntactically valid output?
[ ] Would procedural host code be easier to test than nested transformations?
```

---

# 12) Project configuration — `sgconfig.yml`, discovery, directories, globs

## 12.0 Role of `sgconfig.yml`

`sgconfig.yml` defines the ast-grep **project root and execution catalog**, not one lint rule.

It tells the CLI where to discover:

* rules;
* rule tests/snapshots;
* global utility rules;
* nonstandard language/file mappings;
* custom Tree-sitter parsers;
* custom-language outline extraction;
* experimental language injections.

## 12.1 Canonical project scaffold

```text
repo/
  sgconfig.yml
  rules/
  rule-tests/
  utils/
  outline/              # optional project/custom language extractors
  parsers/              # optional custom grammar libraries
```

Create interactively:

```bash
ast-grep new project
```

Create elsewhere with `-b/--base-dir`:

```bash
ast-grep new project --base-dir tooling/ast-grep
```

## 12.2 Baseline config

```yaml
ruleDirs:
  - rules

utilDirs:
  - utils

testConfigs:
  - testDir: rule-tests
    snapshotDir: __snapshots__
```

Modern ast-grep has relaxed some historical requirements around always having `ruleDirs`, but a normal scan project should configure its rule catalog explicitly.

## 12.3 `ruleDirs`

```yaml
ruleDirs:
  - rules
  - security-rules
```

Paths resolve relative to `sgconfig.yml`.

0.45 includes an ignore-related fix so ignore-file behavior around rule directories is more disciplined than older implementations. Still, verify monorepo discovery with `scan --inspect summary/entity` when the working directory can vary.

## 12.4 `testConfigs`

```yaml
testConfigs:
  - testDir: rule-tests
    snapshotDir: __snapshots__
  - testDir: migration-tests
```

Each entry requires `testDir`; `snapshotDir` is optional. The config reference's stable default is a `__snapshots__` folder beneath the test directory, although a generated detailed CLI page has shown a conflicting prose default; prefer config schema/aggregate CLI behavior or explicitly set it to remove ambiguity.

## 12.5 `utilDirs`

```yaml
utilDirs:
  - utils
  - shared/ast-grep-utils
```

Global utility files discovered here are addressable through `matches` according to language/scope rules.

## 12.6 `languageGlobs`

Map nonstandard files to a built-in/custom language:

```yaml
languageGlobs:
  html: ['*.vue', '*.svelte', '*.astro']
  json: ['.eslintrc', '.prettierrc']
  cpp:  ['*.c']
  tsx:  ['*.ts']
```

Key property: **languageGlobs takes precedence over default extension mapping**.

Use cases:

* extensionless config files;
* language variants sharing syntax;
* project conventions;
* overriding a built-in extension parser deliberately.

### Risk

A broad override can make rules appear “wrong” when the actual problem is that a file is being parsed under a different grammar than an agent assumed. Keep overrides narrow and documented.

## 12.7 Config discovery in monorepos

Recommended:

```bash
ast-grep scan --config path/to/sgconfig.yml
ast-grep outline --config path/to/sgconfig.yml <paths>
```

when:

* agent CWD is not deterministic;
* multiple configs exist;
* repository tooling runs from a nested workspace;
* editor/LSP root differs from repository root.

## 12.8 `new` scaffolding commands

```bash
ast-grep new project
ast-grep new rule no-alert -l JavaScript -y -c sgconfig.yml
ast-grep new test no-alert-basic -y -c sgconfig.yml
ast-grep new util is-literal -l TypeScript -y -c sgconfig.yml
```

If multiple configured destination dirs exist, interactive mode asks which to use; `--yes` chooses defaults/first configured destinations and requires necessary CLI arguments.

---

# 13) Languages — built-ins, aliases, language globs, custom parsers, injections

## 13.0 Language strategy

Language selection affects:

* source parsing;
* pattern parsing;
* available node kinds/fields;
* outline extractors;
* injected subdocuments;
* custom grammar compatibility.

Never port a rule between languages by changing only `language:` unless the grammar shape has been verified.

## 13.1 Built-in language coverage

The official language reference has historically listed the following broad built-ins/aliases:

| Language | Common CLI aliases / extensions (representative) |
|---|---|
| Bash | `bash`; sh/bash/zsh/ksh/bats and related shell extensions |
| C | `c`; `.c`, `.h` |
| C++ | `cpp`, `cc`, `c++`, `cxx`; C++/CUDA/Arduino-related extensions |
| C# | `cs`, `csharp` |
| CSS | `css` |
| Dart | `dart` |
| Elixir | `elixir`, `ex` |
| Go | `go`, `golang` |
| Haskell | `haskell`, `hs` |
| HCL | `hcl` |
| HTML | `html` |
| Java | `java` |
| JavaScript | `javascript`, `js`, `jsx` |
| JSON | `json` |
| Kotlin | `kotlin`, `kt` |
| Lua | `lua` |
| Markdown | `markdown`, `md` (verify exact alias/extension mapping against installed binary) |
| Nix | `nix` |
| PHP | `php` |
| Python | `python`, `py` |
| Ruby | `ruby`, `rb` |
| Rust | `rust`, `rs` |
| Scala | `scala` |
| Solidity | `solidity`, `sol` |
| Swift | `swift` |
| TypeScript | `typescript`, `ts` |
| TSX | `tsx` |
| YAML | `yaml`, `yml` |

### Current-table drift: Dart and Markdown

The table above includes both because the released 0.45.1 `ast-grep-language` package exposes `Dart` and `Markdown` in its built-in language enum. Some generated website tables have lagged those source/package additions.

The 0.45-era source/changelog confirms:

* **Dart support restored in 0.42.1**;
* **Markdown support added in 0.43.0**.

Some generated “built-in language list” pages have not consistently surfaced both additions. Therefore:

```bash
ast-grep --help
ast-grep run -l dart -p '<valid dart pattern>' ...
ast-grep run -l markdown -p '<valid markdown pattern>' ...
```

against your installed 0.45.x binary is the authoritative compatibility check.

Python mapping also gained Bazel-oriented file handling in the 0.44 line; treat exact default extension lists as version-sensitive and prefer `languageGlobs` when repository correctness depends on them.

## 13.2 Grammar variation is a first-class concern

The same-looking pattern can parse differently across languages.

Dart is a clear example: a standalone call-looking snippet can be interpreted differently because top-level expressions are not legal in the grammar. The robust pattern is a **valid enclosing context + selector**.

This principle generalizes to:

* class fields vs assignment expressions;
* JSON pairs;
* language attributes/decorators;
* type-only fragments;
* shell fragments;
* embedded code.

## 13.3 `customLanguages`

Register a compiled Tree-sitter dynamic library:

```yaml
customLanguages:
  mojo:
    libraryPath: parsers/mojo.so
    extensions: [mojo]
    outlineRules: outline/mojo.yml
    expandoChar: _
    languageSymbol: tree_sitter_mojo
```

Fields:

| Field | Required | Purpose |
|---|---:|---|
| `libraryPath` | yes | parser dynamic library path or platform map |
| `extensions` | yes | filename extensions |
| `outlineRules` | no | custom outline extractor file |
| `expandoChar` | no | parser-safe replacement char used internally for `$` pattern variables |
| `languageSymbol` | no | exported Tree-sitter language symbol, default derived from name |

Paths are relative to `sgconfig.yml`.

## 13.4 Cross-platform parser libraries

`libraryPath` can be platform-specific:

```yaml
customLanguages:
  mylang:
    libraryPath:
      aarch64-apple-darwin: parsers/mylang-macos-arm64.dylib
      x86_64-unknown-linux-gnu: parsers/mylang-linux-x64.so
    extensions: [mylang]
```

This is necessary when a rule project is shared across heterogeneous developer/CI systems.

### Deployment checklist

```text
[ ] parser ABI/Tree-sitter compatibility tested
[ ] target triple represented
[ ] library path relative to config is stable
[ ] untrusted repositories cannot redirect arbitrary dynamic library loading
[ ] CI bundles/builds the required parser artifact
```

## 13.5 Building a custom parser

One documented route uses `tree-sitter` CLI and `TREE_SITTER_LIBDIR` to compile/load a grammar dynamic library. Exact build output suffix differs by platform.

Example flow:

```bash
cd path/to/tree-sitter-mylang
export TREE_SITTER_LIBDIR=/absolute/path/to/output
tree-sitter test
```

Then register the produced shared library in `sgconfig.yml`.

For parser debugging, `tree-sitter parse <file>` can expose the grammar tree independently of ast-grep.

## 13.6 `expandoChar`

ast-grep must parse pattern text containing `$META`, even if `$` is not legal where the target grammar expects an identifier. Built-ins/custom languages can use an **expando character** strategy to replace `$` temporarily with parser-valid syntax, then reinterpret the corresponding node as a metavariable.

If patterns for a custom grammar fail unexpectedly around `$`, inspect this configuration and the pattern parse rather than treating it as a matcher bug first.

## 13.7 Language injections (experimental)

Multi-language source files need a host grammar plus embedded-language regions.

Config shape:

```yaml
languageInjections:
  - hostLanguage: js
    rule:
      pattern: styled.$TAG`$CONTENT`
    injected: css
```

Fields:

```text
hostLanguage  language of container document
rule          matcher locating the injected region (captures $CONTENT)
injected      target language or candidate list
```

Dynamic language example:

```yaml
languageInjections:
  - hostLanguage: js
    rule:
      pattern: styled.$LANG`$CONTENT`
    injected: [css, scss, less]
```

When `injected` is a list, `$LANG` selects among candidates according to injection semantics.

## 13.8 Built-in injection behavior

ast-grep has native handling for common HTML/JavaScript/CSS embedding. Project configuration extends this idea to patterns such as CSS-in-JS.

The operational pipeline is:

```text
file discovery
  -> host-language inference
  -> injection-region extraction
  -> parse region using injected language
  -> match rules/patterns against region syntax
```

0.42 improved the LSP so diagnostics also scan injected languages.

## 13.9 Injection correctness risks

* injection boundary omits/mis-captures delimiter text;
* nested templates contain escapes that alter the embedded source;
* dynamic language capture does not exactly match a candidate name;
* host grammar update changes the capture shape;
* a rewrite inside the injected region must still produce valid host text/escaping.

Treat injections as a boundary between two parsers, not as a transparent multi-language compiler.

### Agent checklist

```text
[ ] Is the source actually being parsed under the language I think?
[ ] Has the pattern parse been inspected for this grammar?
[ ] Are languageGlobs overriding the default mapping?
[ ] Are Dart/Markdown availability verified against the installed binary if critical?
[ ] For custom grammars, is the dynamic library trusted and platform-compatible?
[ ] Is expandoChar appropriate for metavariable parsing?
[ ] For injections, does the rule capture the exact $CONTENT region?
[ ] Is LSP/scan behavior on embedded regions tested separately from host syntax?
```

---

# 14) Outline extraction rules — custom items/members and JSON structure contracts

## 14.0 Extractor file model

An outline extractor file is a stream of YAML documents:

```yaml
<extractor 1>
---
<extractor 2>
---
<extractor 3>
```

Each extractor describes one syntax construct that should become an outline **item** or **member**.

## 14.1 Common required fields

```yaml
id: ts-class
language: TypeScript
role: item
symbolType: class
rule:
  kind: class_declaration
name: $NAME
```

Core common fields:

| Field | Required | Meaning |
|---|---:|---|
| `id` | yes | stable extractor id |
| `language` | yes | target language |
| `role` | yes | `item` or `member` |
| `symbolType` | yes | lower-camel outline/LSP-like type |
| `name` | yes | display-name template |
| `signature` | no | display-signature template; fallback is first non-empty source line |
| `rule` | yes | syntax matcher |

Outline extractors can also use normal matching support such as `constraints`, local `utils`, and `transform`.

## 14.2 `symbolType`

Common canonical types include:

```text
file
module
namespace
package
class
method
property
field
constructor
enum
interface
function
variable
constant
key
enumMember
struct
operator
typeParameter
```

Do not invent a special “import” type solely to encode import status; use `isImport`. Likewise, export state belongs in `isExported`.

## 14.3 `name`

Template from captures/transforms:

```yaml
name: $NAME
```

or:

```yaml
transform:
  CLEAN_NAME:
    replace:
      source: $RAW_NAME
      replace: '^r#'
      by: ''
name: $CLEAN_NAME
```

This is one reason outline extractors can reuse the normal rule/transform system rather than requiring a separate parser DSL.

## 14.4 `signature`

```yaml
signature: function $NAME($$$PARAMS)
```

If omitted, outline uses the first non-empty source line of the matched node. Explicit signatures are often better when declarations span multiple lines or contain bodies/modifiers you want to elide.

The 0.45.1 C# fix around multiline member signatures is a reminder that fallback formatting is language-sensitive.

## 14.5 Item-only: `isImport`

```yaml
isImport: true
```

or a rule:

```yaml
isImport:
  has:
    kind: import_clause
```

Type:

```text
Boolean | Rule
```

Default false.

## 14.6 Item-only: `isExported`

```yaml
isExported:
  has:
    regex: '^export\b'
```

Type:

```text
Boolean | Rule
```

Default true according to current extractor field reference.

Again, “exported” is local syntax classification, not transitive module-resolution truth.

## 14.7 Member-only: `parentRuleIds`

```yaml
parentRuleIds:
  - ts-class
  - ts-interface
```

Required for `role: member`.

It references **item extractor IDs**, not item names or symbol types. Attachment is determined by actual syntax containment under matching parent items.

Outline does not attach a member by receiver type, import identity, trait implementation, or semantic name resolution.

## 14.8 Member-only: `isPublic`

Boolean:

```yaml
isPublic: true
```

Rule:

```yaml
isPublic:
  has:
    regex: '^pub\b'
```

Default true when omitted.

This feeds `--pub-members` filtering.

## 14.9 Custom language example

`sgconfig.yml`:

```yaml
customLanguages:
  mojo:
    libraryPath: parsers/mojo.so
    extensions: [mojo]
    outlineRules: outline/mojo.yml
```

`outline/mojo.yml`:

```yaml
id: mojo-function
language: mojo
role: item
symbolType: function
rule:
  pattern: fn $NAME($$$ARGS) { $$$BODY }
name: $NAME
signature: fn $NAME($$$ARGS)
```

Then:

```bash
ast-grep outline src -l mojo
```

## 14.10 Parent/member example

```yaml
id: my-class
language: mylang
role: item
symbolType: class
rule:
  pattern: class $NAME { $$$BODY }
name: $NAME
signature: class $NAME
---
id: my-method
language: mylang
role: member
symbolType: method
parentRuleIds: [my-class]
rule:
  pattern: fn $NAME($$$ARGS) { $$$BODY }
name: $NAME
signature: fn $NAME($$$ARGS)
isPublic:
  follows:
    pattern: pub
```

The exact visibility grammar should be authored from actual node kinds/fields; this is illustrative structure, not a universal grammar.

## 14.11 Outline extractor design rules

### Prefer stable syntax anchors

```yaml
kind: function_definition
```

or a valid code pattern with selector.

Avoid an extractor whose identity depends on whitespace/source formatting regex alone.

### Keep item/member levels shallow

The CLI intentionally exposes **top-level item + direct member**, not arbitrary recursive symbol nesting. Model the output shape users/agents need, not every grammar node.

### Make signatures compact

The purpose is navigation. A signature should not reproduce a 30-line generic/where-clause/body unless that information is necessary.

### Test import/export/public classifiers

These flags alter default directory output and `--pub-members`, so an incorrect classifier can make symbols disappear from the agent's default map.

## 14.12 JSON contract consumption

Consumers should treat:

```text
path + range + astKind + extractor-driven name/type
```

as the reliable local syntax coordinates.

Do not manufacture a persistent global symbol ID from `name` alone. If you need cross-run identity, combine source-control identity/path/range/signature and reconcile with semantic/indexing systems according to your application contract.

### Agent checklist

```text
[ ] Are extractor IDs stable and unique?
[ ] Is role correct: item vs member?
[ ] Is symbolType from the shared vocabulary?
[ ] Does name always resolve to useful nonempty text?
[ ] Is explicit signature preferable to first-line fallback?
[ ] Are import/export/public flags syntax-accurate?
[ ] Do members list the correct parentRuleIds?
[ ] Is semantic attachment deliberately out of scope?
[ ] Are custom-language extractors registered relative to sgconfig?
```


---

# 15) Rule testing — valid/invalid cases, snapshots, filtering, test configuration

## 15.0 Testing mental model

A production structural rule is a classifier. Test it on both expected non-findings and expected findings.

| Code expectation | Rule does not report | Rule reports |
|---|---|---|
| valid | **Validated** | **Noisy** (false positive) |
| invalid | **Missing** (false negative) | **Reported** |

A rule that only has “should match” fixtures can still be unusably noisy. A rule that only has “should not match” fixtures can silently miss real cases.

## 15.1 Test project setup

`sgconfig.yml`:

```yaml
ruleDirs:
  - rules

testConfigs:
  - testDir: rule-tests
    snapshotDir: __snapshots__
```

Layout:

```text
repo/
  sgconfig.yml
  rules/
    no-await-in-loop.yml
  rule-tests/
    no-await-in-loop-test.yml
    __snapshots__/
```

## 15.2 Minimal test file

```yaml
id: no-await-in-loop
valid:
  - for (let a of b) { console.log(a) }
invalid:
  - async function foo() { for (var bar of baz) await bar; }
```

The `id` binds the test document to the rule ID.

## 15.3 Match-only development loop

```bash
ast-grep test --skip-snapshot-tests
```

Use while matcher behavior is still changing. It verifies valid/invalid classification without requiring diagnostic snapshots.

## 15.4 Snapshot tests

Snapshots lock down reporting details such as:

* diagnostic span;
* labels;
* message/note-related output;
* fix/report structure represented by snapshot schema.

Generate/update all:

```bash
ast-grep test -U
```

Interactively accept only intended changes:

```bash
ast-grep test -i
```

`--skip-snapshot-tests` and `--update-all` conflict by design.

## 15.5 Test filtering

```bash
ast-grep test -f 'security/**'
```

The filter is a glob over test cases according to CLI test discovery semantics.

Use it for local iteration; run the complete suite in CI.

## 15.6 `--include-off`

Rules with:

```yaml
severity: off
```

are normally skipped by test execution. Include them explicitly:

```bash
ast-grep test --include-off
```

This is useful for staged rules that are not deployed yet but whose matcher/fix contract should remain healthy.

## 15.7 Explicit test/snapshot directories

```bash
ast-grep test \
  --config sgconfig.yml \
  --test-dir rule-tests \
  --snapshot-dir __snapshots__
```

The config-level default is best made explicit in repositories where generated CLI prose/version drift could create ambiguity.

## 15.8 Recommended rule test matrix

Every nontrivial rule should consider:

```text
[ ] smallest true positive
[ ] smallest true negative
[ ] nested true positive
[ ] similar syntax that must not match
[ ] alternative whitespace/comments
[ ] multiline form
[ ] zero/one/many list elements where $$$ is used
[ ] repeated-metavariable equality cases
[ ] same text in wrong grammar field
[ ] nested function/scope boundary for relational traversal
[ ] ignored/generated path behavior if files/ignores matter
[ ] fix output at top level
[ ] fix output under indentation
[ ] fix with first/middle/last list item if separators are expanded
[ ] transformed identifier edge cases
[ ] Unicode text if substring/range behavior matters
```

## 15.9 Snapshot philosophy

A snapshot should protect behavior humans/tools depend on, not merely freeze every incidental formatting detail forever.

When a snapshot changes, classify the diff:

```text
matcher target changed      -> likely semantic rule change
label range changed         -> diagnostic UX/API change
message changed             -> policy/documentation change
fix changed                 -> codemod behavior change
parser-induced span changed -> dependency/version review needed
```

## 15.10 CI test sequence

```bash
ast-grep test --color=never
ast-grep scan --format github
```

For a repository that treats rule authoring as software engineering, also validate YAML/schema and any generated rule catalog docs.

## 15.11 Exit status of `ast-grep test`

`test` does **not** follow the grep-like `0`/`1` contract that `run` uses (section 3.14). It reports distinct nonzero codes per failure class. Measured against 0.45.1:

| Code | Meaning |
|---|---|
| `0` | every selected case passed |
| `3` | `--filter` selected no test case |
| `4` | a `valid`/`invalid` assertion failed, **or** a snapshot is missing or mismatched |
| `6` | a configured `testDir` does not exist |
| `8` | a rule or config file could not be parsed |

These numbers are not part of a published CLI stability contract, so treat **"zero versus nonzero" as the durable check** and the specific code as a diagnostic hint. In particular, do not write `if status == 1` — `test` never returns `1`.

Two of these are silent-success hazards rather than failures:

* **A test file naming a rule `id` that does not exist exits `0`.** The run reports `test result: ok. 0 passed; 0 failed;` — a typo in the `id` field does not fail, it deletes the test. Assert on the *number of cases executed*, not only on the exit code, whenever a suite is expected to be non-empty.
* **`--filter` that matches nothing exits `3`, not `0`.** This is the desirable behavior and it is worth relying on: a CI slice whose filter has drifted out of date fails loudly instead of silently testing nothing.

`--skip-snapshot-tests` suppresses the snapshot half of code `4`, which is why it is the right flag for a gate that runs before snapshots have been reviewed. `--update-all` rewrites the snapshots to match current output and then exits `0`, so it can never fail — that makes it a deliberate authoring step and never a step inside a gate (section 15.9).

### Agent checklist

```text
[ ] Are both noisy and missing-match failure modes tested?
[ ] Is snapshot skipping limited to matcher iteration?
[ ] Are snapshots reviewed rather than blindly regenerated?
[ ] Are off/staged rules tested intentionally?
[ ] Does CI run the full suite rather than only developer filters?
[ ] Are fix edge cases represented explicitly?
[ ] Are parser/version upgrades followed by snapshot review?
```

---

# 16) Severity, suppressions, runtime overrides, and CI gating

## 16.0 Severity is deployment policy

Rule logic answers:

```text
Does this syntax match the concern?
```

Severity answers:

```text
How should this environment treat the concern?
```

Separating those lets the same rule catalog serve:

* exploratory audits;
* editor hints;
* warnings during migration;
* hard CI gates after adoption.

## 16.1 Severity levels

```text
error
warning
info
hint
off
```

`error` is the meaningful hard-failure level for scan gating.

## 16.2 Runtime override examples

```bash
# promote one rule
ast-grep scan --error=security.no-eval

# promote several
ast-grep scan \
  --error=security.no-eval \
  --error=security.no-dynamic-code

# turn one off
ast-grep scan --off=legacy.noisy-rule

# set all active rules to warning for audit
ast-grep scan --warning
```

When a rule ID is supplied, use `=`.

## 16.3 Line suppressions

Generic suppression on preceding line:

```js
// ast-grep-ignore
console.log('suppressed')
```

Rule-specific:

```js
// ast-grep-ignore: no-console
console.log('suppressed')
```

Multiple IDs:

```js
// ast-grep-ignore: no-console, no-debug-log
console.log('suppressed')
```

Same-line suppression:

```js
console.log('suppressed') // ast-grep-ignore: no-console
```

## 16.4 Suppression attachment rule

A suppression comment can suppress the next-line diagnostic only when there is no preceding AST construct on the suppression comment's same line. This prevents a trailing comment after unrelated source from accidentally suppressing the following statement.

Modern releases include correctness fixes around trailing/multiline suppression handling, so avoid reproducing old workarounds without testing current behavior.

## 16.5 File-level suppression

File-wide suppression has a deliberately specific form:

```js
// ast-grep-ignore

// remaining file content...
```

Conditions:

1. suppression comment is the **first line**;
2. second line is **empty**.

Rule-specific file suppression:

```js
// ast-grep-ignore: no-debugger

// this rule is suppressed for the file; other rules remain active
```

## 16.6 `unused-suppression`

Built-in hygiene finding:

```text
unused-suppression
```

It behaves as a hint with an auto-fix by default when ast-grep can safely know the complete active rule set.

Automatic unused-suppression reporting is disabled when scan is intentionally incomplete, including when:

* any rule is disabled by CLI `--off`;
* `--rule` is used;
* `--inline-rules` is used;
* `--filter` is used.

Reason: if ast-grep did not run the full intended catalog, it cannot safely conclude a suppression is unnecessary.

Enforce in CI:

```bash
ast-grep scan --error=unused-suppression
```

## 16.7 `no-suppress-all`

Blanket:

```text
ast-grep-ignore
```

can hide every rule on a line and can even result from a typo that omitted the colon before a rule ID.

Enable the built-in policy rule:

```bash
ast-grep scan --warning=no-suppress-all
# or hard gate
ast-grep scan --error=no-suppress-all
```

Then developers must write rule-specific suppressions.

## 16.8 Inspect effective severity

```bash
ast-grep scan --inspect entity
```

Inspection output can show final/effective rule severity after definition + runtime overrides.

This is the first diagnostic when a rule unexpectedly does or does not fail CI.

## 16.9 Suppression governance pattern

For large rule catalogs:

```text
error rules require rule-specific suppression
+ no-suppress-all enabled
+ unused-suppression promoted in CI
+ suppression count tracked by rule ID
+ code review requires explanation for new suppressions
```

This turns suppressions into explicit technical debt rather than invisible lint bypasses.

### Agent checklist

```text
[ ] Is severity chosen separately from matcher correctness?
[ ] Are rule IDs stable enough for long-lived suppressions?
[ ] Are blanket suppressions disallowed where governance requires it?
[ ] Is unused-suppression only interpreted when full rule coverage is active?
[ ] Is finalSeverity checked when CI behavior surprises you?
[ ] Are suppression comments added only as deliberate exceptions, never as an automatic fix for a noisy rule?
```

---

# 17) Output contracts — terminal, JSON, SARIF, GitHub annotations, coordinates

## 17.0 Treat output as an API choice

ast-grep has outputs for different consumers:

| Consumer | Recommended output |
|---|---|
| human terminal | rich/default text |
| shell script, small result | pretty/compact JSON |
| shell/agent stream, large result | `--json=stream` |
| GitHub Actions | `scan --format github` |
| code scanning ingestion | `scan --format sarif` |
| editor | LSP diagnostics |

## 17.1 `run` JSON match schema

Conceptual current shape:

```ts
interface Match {
  text: string
  range: Range
  file: string
  lines: string
  replacement?: string
  replacementOffsets?: ByteOffset
  metaVariables?: MetaVariables
}

interface Range {
  byteOffset: ByteOffset
  start: Position
  end: Position
}

interface ByteOffset {
  start: number
  end: number
}

interface Position {
  line: number
  column: number
}

interface MetaVariables {
  single: Record<string, MetaVar>
  multi: Record<string, MetaVar[]>
  transformed: Record<string, string>
}

interface MetaVar {
  text: string
  range: Range
}
```

Current JSON also includes language in actual CLI match output examples; consumers should tolerate additive fields and should validate against their installed version rather than using a hand-written closed schema forever.

## 17.2 `scan` rule-match extension

```ts
interface RuleMatch extends Match {
  ruleId: string
  severity: 'error' | 'warning' | 'info' | 'hint'
  note?: string
  message: string
}
```

With:

```bash
--include-metadata
```

rule metadata is included for JSON consumers.

## 17.3 Coordinate contract

All of the following are **zero-based**:

```text
line
column
byteOffset
```

Byte offsets are UTF-8 byte positions; end is exclusive.

This distinction matters because:

* Unicode code points can occupy multiple UTF-8 bytes;
* a UI column can be a character position while a byte offset is different;
* transformations such as `substring` count Unicode characters, not bytes.

Never substitute one coordinate domain for another.

## 17.4 Pretty / compact / stream

Pretty:

```bash
ast-grep run -p 'Some($A)' --json .
```

Compact:

```bash
ast-grep run -p 'Some($A)' --json=compact .
```

Stream:

```bash
ast-grep run -p 'Some($A)' --json=stream .
```

For large repositories, stream avoids constructing/consuming one huge array before downstream processing can begin.

## 17.5 Streaming parser pattern

Python:

```python
import json
import subprocess

p = subprocess.Popen(
    ["ast-grep", "run", "-l", "rust", "-p", "unwrap()", "--json=stream", "."],
    stdout=subprocess.PIPE,
    text=True,
)

for line in p.stdout:
    match = json.loads(line)
    # process one match without holding the full result set
```

In production, inspect `returncode` and stderr before treating the stream as complete.

## 17.6 Capture data

Single capture:

```json
"single": {
  "A": {
    "text": "matched",
    "range": { "...": "..." }
  }
}
```

Multi capture:

```json
"multi": {
  "ARGS": [
    { "text": "a", "range": { "...": "..." } },
    { "text": "b", "range": { "...": "..." } }
  ]
}
```

Transformed strings are text-only outputs, not source nodes:

```json
"transformed": {
  "NEW_NAME": "newName"
}
```

Do not expect source ranges for transformed values.

## 17.7 Replacement fields

When a rewrite/fix is in play, JSON can contain:

```text
replacement
replacementOffsets
```

Use the structured offsets rather than locating replacement text by searching the source string.

## 17.8 `outline` JSON differs intentionally

Outline JSON is grouped **by file**, not one structural match object per syntax node. `--json=stream` emits one file object per line.

Do not feed outline JSON into a parser written for `run --json=stream` without a type discriminator or separate code path.

## 17.9 GitHub format

```bash
ast-grep scan --format github
```

Designed for direct GitHub workflow annotations. It is a presentation/integration contract, not the richest machine data model.

## 17.10 SARIF

```bash
ast-grep scan --format sarif > results.sarif
```

Use when the receiving system expects SARIF/code-scanning results.

Persist the exact ast-grep version alongside long-lived SARIF archives if reproducibility across rule/parser upgrades matters.

## 17.11 Output stability policy for agent tooling

Agent wrappers should:

```text
1. select JSON explicitly
2. parse by documented field names
3. tolerate additive unknown fields
4. validate required fields
5. preserve byte offsets + line/column
6. keep stderr separate
7. record ast-grep --version in debug traces
8. treat a nonzero process status as an execution signal, not merely empty JSON
```

---

# 18) LSP/editor integration — diagnostics, code actions, rule reload behavior

## 18.0 LSP contract

Start:

```bash
ast-grep lsp
```

Explicit config:

```bash
ast-grep lsp -c path/to/sgconfig.yml
```

Core server capabilities documented today:

* publish diagnostics;
* code actions for diagnostic fixes.

Client needs standard document lifecycle notifications:

```text
textDocument/didOpen
textDocument/didChange
textDocument/didClose
```

## 18.1 Required project setup

Diagnostics depend on a linting project with `sgconfig.yml` reachable from the workspace/server root.

If the editor launches in a different directory than your shell, a command that works in terminal can still produce no editor diagnostics.

Debug:

```text
1. verify editor can find `ast-grep` executable
2. verify workspace root
3. verify sgconfig path
4. run ast-grep scan manually from same root
5. use explicit lsp -c path if needed
```

## 18.2 VS Code extension

The official integration exposes structural search/replace UX and can use LSP diagnostics/code actions.

Two distinct capabilities should not be conflated:

```text
extension structural search UI
vs
LSP-backed project diagnostics
```

The latter requires a configured ast-grep project.

## 18.3 YAML schema validation

Rule YAML can opt into schema-aware editor validation with a schema header and a YAML language server capable of consuming the schema.

This is valuable because it catches:

* misspelled fields;
* wrong value shapes;
* unsupported enum values;
* some node/field schema issues.

Do not make schema validation the only rule test: a syntactically valid config can still be logically wrong.

## 18.4 Neovim

Representative `nvim-lspconfig` shape:

```lua
require('lspconfig').ast_grep.setup({
  cmd = { 'ast-grep', 'lsp' },
})
```

The editor guide provides a broader default filetype list. In a custom setup, include only filetypes your rule catalog/language support actually needs if that simplifies routing.

## 18.5 Injected-language diagnostics

0.42-era LSP work added scanning of injected languages for diagnostics. This matters for:

* JS/CSS/HTML mixed documents;
* project-defined injections such as styled-components;
* any rule catalog targeting embedded language regions.

Test editor behavior separately from batch `scan`; incremental document updates and injection extraction are additional moving parts.

## 18.6 Fix actions and `FixConfig`

Modern LSP quick-fix handling supports advanced fix-range expansion such as `expandStart`/`expandEnd` on current versions. That means a rule can offer an editor fix that correctly consumes separators/trivia rather than only replacing the exact target node.

## 18.7 LSP rule evolution/reload

The 0.39 line added/strengthened rule reloading behavior. Nevertheless, when debugging a stale editor state, restart/reload the workspace before concluding the CLI and LSP use different match semantics.

## 18.8 LSP is not a semantic language server

ast-grep's LSP serves **ast-grep rule diagnostics and fixes**. It does not replace rust-analyzer, Pyright/Pyrefly, tsserver, clangd, etc. for compiler/type-aware language intelligence.

An optimal editor often runs both:

```text
semantic language server
+ ast-grep lint LSP
```

### Agent checklist

```text
[ ] Is sgconfig in/resolved from the editor workspace root?
[ ] Does editor PATH include the expected ast-grep binary/version?
[ ] Does manual scan reproduce the diagnostic?
[ ] Are injected languages involved?
[ ] Is stale rule reload state possible?
[ ] Is the requested feature an ast-grep lint diagnostic or a semantic LSP feature?
```

---

# 19) Programmatic APIs — JavaScript/N-API, Python/PyO3, Rust core, WASM

## 19.0 When to embed instead of shelling out

Use the API when the task needs arbitrary procedural logic such as:

* count/order matched nodes;
* branch edits based on node contents;
* coordinate several edits in one buffer;
* compute replacement text with host-language libraries;
* traverse fields/ancestors programmatically;
* integrate directly into a service/editor/tool process.

Use the CLI when:

* process boundaries are acceptable;
* repository file discovery is desired;
* a YAML rule can express the logic;
* streaming JSON is a simpler stable integration boundary.

The official API guide explicitly notes that applying YAML-style `fix` through JS/Python APIs remains more experimental than basic parsing/traversal/edit primitives.

---

## 19.1 JavaScript / `@ast-grep/napi`

The JavaScript binding is powered by napi.rs and is described by the project as the most robust/reliable programmatic binding.

Install:

```bash
npm install @ast-grep/napi
```

### Main workflow

```ts
import { parse, Lang } from '@ast-grep/napi'

const root = parse(Lang.TypeScript, `
  const x = foo(1)
`).root()

const call = root.find('foo($A)')
if (call) {
  const arg = call.getMatch('A')
  console.log(arg?.text())
}
```

### Main function families

Current N-API surface includes functions conceptually for:

```text
parse
parseAsync
kind
pattern
findInFiles
registerDynamicLanguage
```

Exact TypeScript declarations in the package/source are the version-pinned authority.

### `SgRoot`

The root object owns parsed source/tree lifetime and exposes:

```ts
root(): SgNode
filename(): string // where applicable in API version
```

Keep nodes tied to their owning root; do not assume a node is an independent mutable AST object.

### `SgNode` inspection

Representative methods:

```ts
range()
isLeaf()
isNamed()
isNamedLeaf()
kind()
is(kind)
text()
getRoot()
```

### Refinement predicates

```ts
matches(...)
inside(...)
has(...)
precedes(...)
follows(...)
```

These mirror rule semantics on a selected node.

### Search

```ts
find(matcher)
findAll(matcher)
```

Matcher forms can include:

* pattern string;
* numeric kind ID;
* config object.

### Traversal

```ts
children()
field(name)
parent()
child(nth)
ancestors()
next()
nextAll()
prev()
prevAll()
```

Use grammar `field()` when the field semantics are stable; it is usually more intention-revealing than positional child indexes.

### Metavariables

Single capture:

```ts
getMatch('A'): SgNode | null
```

Multi capture:

```ts
getMultipleMatches('ARGS'): SgNode[]
```

Note that multi captures may expose syntax sequence nodes/tokens according to binding behavior; inspect results rather than assuming a list contains only semantic expressions.

### `NapiConfig`

Conceptual type:

```ts
interface NapiConfig {
  rule: object
  constraints?: object
  language?: FrontEndLanguage
  transform?: object
  utils?: object
}
```

This lets host code reuse significant parts of YAML rule semantics.

### Edit API

Nodes are immutable. `replace` creates an edit:

```ts
const edit = node.replace('new text')
const newSource = root.commitEdits([edit])
```

Conceptual edit:

```ts
interface Edit {
  startPos: number
  endPos: number
  insertedText: string
}
```

**Important:** direct API `replace(text)` is host-supplied replacement text; do not assume it interpolates YAML metavariables like CLI `fix` does.

### Deprecated language namespaces

Older convenience namespaces such as:

```ts
js.parse(...)
html.parse(...)
css.parse(...)
```

are deprecated. Prefer top-level functions with explicit `Lang`, e.g.:

```ts
parse(Lang.JavaScript, source)
```

### Language coverage

The N-API package ships JS-ecosystem languages directly by default and can register more parsers dynamically. Do not assume CLI's full built-in language catalog equals one N-API package's embedded parser set.

---

## 19.2 JavaScript performance rules

### Minimize FFI crossings

Bad shape:

```text
JS loops every node
  -> calls Rust one node at a time
  -> large N-API overhead
```

Better:

```ts
const nodes = root.findAll({ rule: { kind: 'member_expression' } })
```

Let Rust traverse/filter, then transfer only the relevant nodes/results.

### Prefer `parseAsync` for workloads where blocking JS matters

For many independent parse operations, async parsing can avoid monopolizing the JS event loop, though the optimal architecture often goes further and uses multi-file Rust-side search.

### Prefer `findInFiles` for large repository searches

`findInFiles` performs path discovery/parsing/search in Rust and can use multiple Rust threads, reducing JS↔Rust transfer overhead.

Conceptual config:

```ts
interface FindConfig {
  paths: string[]
  matcher: NapiConfig
}
```

When correctness depends on callback completion ordering/counts, test the exact binding version; historical docs warn that Promise/callback completion coordination can be subtle in `findInFiles`-style APIs.

---

## 19.3 Python / `ast-grep-py`

Install:

```bash
pip install ast-grep-py
```

### Parse/search

```python
from ast_grep_py import SgRoot

root = SgRoot("print('hello world')", "python").root()
call = root.find(pattern="print($A)")
if call:
    arg = call.get_match("A")
    print(arg.text())
```

### `SgRoot`

```python
class SgRoot:
    def __init__(self, src: str, language: str) -> None: ...
    def root(self) -> SgNode: ...
```

### Search

```python
node.find(pattern="foo($A)")
node.find(kind="call")
node.find_all(...)
```

Python type stubs expose rule keyword arguments and config objects.

### Inspection/refinement

Representative snake_case methods:

```text
range
is_leaf
is_named
is_named_leaf
kind
text
matches
inside
has
precedes
follows
```

### Traversal

```text
children
ancestors
next
next_all
prev
prev_all
parent
child
field
```

Check the installed stub for exact names/version.

### Captures

```python
match.get_match('A')
match.get_multiple_matches('ARGS')
```

Some Python conveniences permit capture access through mapping/index syntax depending on binding version; prefer explicit methods in shared tooling for readability.

### Edit API

```python
root = SgRoot("print('hello world')", "python").root()
node = root.find(pattern="print($A)")
edit = node.replace("logger.info('hello world')")
new_src = root.commit_edits([edit])
```

Nodes remain immutable; edits produce a new source string.

### Python integration choice

If Python code already performs semantic analysis/data processing, `ast-grep-py` is excellent for **syntax-local candidate parsing and precise range edits**. For whole-repo high-throughput scans, compare the overhead of Python-driven file iteration against invoking the Rust CLI with `--json=stream`.

---

## 19.4 Rust / `ast-grep-core`

The core crate is for applications embedding ast-grep's matcher/traversal directly.

Current docs describe it as providing:

```text
parsing
traversing
searching
replacing Tree-sitter nodes
```

Key re-exports include:

```rust
Language
Matcher
NodeMatch
Pattern
PatternError
Doc
```

Core types include:

```text
Node
Position
MatchStrictness
AstGrep (type alias)
```

### Matcher implementations

Notable core matchers:

```text
Pattern       structural Tree-sitter pattern
KindMatcher   node kind matcher
RegexMatcher  textual regex matcher
```

### Pattern construction

Representative API:

```rust
Pattern::try_new(src, lang)
Pattern::new(src, lang)
Pattern::contextual(context, selector, lang)
pattern.with_strictness(...)
```

`try_new` is preferable when untrusted/user-authored patterns can fail and you want explicit error handling.

### `MatcherExt`

Provides convenience methods such as:

```rust
match_node(node)
find_node(node)
```

returning `NodeMatch` with populated metavariable environment where applicable.

### Stability posture

The Rust crate is the internal core substrate and is a lower-level integration surface than the CLI. Pin exact minor/patch versions and compile against rustdoc rather than copying code from a different version's blog post.

---

## 19.5 WASM

0.41 introduced a WebAssembly ast-grep package. The WASM surface targets environments such as:

* browsers;
* Deno;
* edge runtimes;
* Node environments where native add-ons are undesirable.

The currently visible npm package/docs can lag the 0.45 CLI line, so **treat WASM package versioning independently**.

Documented setup concept:

```ts
import {
  initializeTreeSitter,
  setupParser,
  parse,
  kind,
} from '@ast-grep/wasm'

await initializeTreeSitter()
await setupParser('javascript', '/path/to/tree-sitter-javascript.wasm')

const sg = parse('javascript', 'console.log("hello")')
const node = sg.root().find('console.log($ARG)')
```

`web-tree-sitter` is a peer/runtime dependency in the published WASM packaging model.

### WASM caveats

* parser WASM binaries must be distributed and loaded;
* parser/runtime versions must be compatible;
* filesystem-oriented CLI behavior is not automatically reproduced in a browser;
* published WASM language set/package version may lag native CLI;
* benchmark native N-API vs WASM for server-side Node before choosing portability over speed.

---

## 19.6 API vs CLI decision table

| Requirement | Best first choice |
|---|---|
| repo scan from shell/agent | CLI JSON stream |
| lint project + suppressions/tests | CLI/project YAML |
| one in-memory editor buffer | JS/Python API |
| custom AST algorithm | host API |
| native Rust service | `ast-grep-core` |
| browser syntax tool | WASM |
| maximum Node multi-file throughput | N-API `findInFiles` candidate |
| easiest stable process boundary | CLI |

### Agent checklist

```text
[ ] Does host procedural logic add real value over YAML/CLI?
[ ] Is package version pinned independently from CLI version?
[ ] Are nodes treated as immutable views?
[ ] Are edits accumulated and committed deliberately?
[ ] Is metavariable interpolation implemented by the correct layer?
[ ] Are FFI crossings minimized?
[ ] Is whole-repo work pushed into Rust where possible?
[ ] Are dynamic parsers/WASM assets version-compatible and trusted?
```

---

# 20) Traversal, performance, pruning, concurrency, and scaling

## 20.0 Performance model

End-to-end cost is roughly:

```text
filesystem discovery
+ file reads
+ language inference
+ Tree-sitter parse
+ node traversal
+ rule evaluation
+ capture/transform/fix generation
+ output serialization/rendering
```

Optimization should target the largest term in the actual workload, not simply increase thread count.

## 20.1 First optimization: scan fewer files

Before matcher micro-optimization:

```bash
ast-grep run ... src/specific-subtree
ast-grep scan --globs 'crates/core/**' ...
ast-grep outline src -l rust
```

Use:

* path arguments;
* language selection;
* `.gitignore`/`.ignore`;
* rule `files`/`ignores`;
* CLI `--globs`.

Parsing a file you could have excluded is usually more expensive than a small matcher refinement.

## 20.2 Positive-kind pruning

The config engine can infer **potential node kinds** for many rules and avoid evaluating them on impossible syntax nodes.

Efficient:

```yaml
rule:
  kind: call_expression
  has:
    ...
```

or a concrete pattern with a known root kind.

Less optimizable:

```yaml
rule:
  all:
    - regex: '...'
    - not: ...
    - matches: highly-dynamic-parameterized-util
```

A positive structural anchor improves both speed and understandability.

## 20.3 Relational traversal depth

```yaml
has:
  stopBy: end
```

can scan arbitrary descendant depth for every candidate target.

Prefer:

```yaml
has:
  field: arguments
  kind: string
```

when the actual invariant is direct/field-specific.

Similarly, bound `inside` at semantic-looking syntax boundaries such as functions/classes when appropriate instead of climbing to the file root.

## 20.4 Regex placement

Prefer:

```yaml
kind: identifier
regex: '^foo_'
```

rather than a text regex that runs broadly across arbitrary node kinds.

Regex should usually be a **refinement** after syntax has narrowed the candidate universe.

## 20.5 Capture cost

Use non-capturing metavariables for values you do not need:

```text
$_
$_ARG
```

rather than capturing and storing equality state unnecessarily.

This is a small optimization compared with file/pruning choices, but it also documents intent.

## 20.6 Parameterized utility performance

Because experimental parameterized utilities are bound dynamically, potential-kind inference can become conservative. Restore an explicit kind/pattern guard at the caller when possible.

Benchmark a heavily reused parameterized utility before making it the root of thousands of rules over a monorepo.

## 20.7 Transform/rewrite costs

0.44 added caching for transform replace regexes. Still:

* avoid repeated complex transforms when a simpler captured replacement works;
* recursive rewriter traversal is more expensive than direct fix substitution;
* rewriter complexity should be justified by actual codemod needs.

## 20.8 Threads

```bash
-j 0   # heuristic/default
-j 8   # explicit approximate worker count
```

Reasons to lower threads:

* CI CPU quota;
* shared developer machine;
* disk/network filesystem bottleneck;
* memory pressure from many concurrent parses.

Reasons to benchmark higher:

* large source corpus on fast NVMe;
* compute-heavy rules;
* many independent files.

## 20.9 JSON output scaling

Large result set:

```bash
--json=stream
```

is preferable to a giant pretty JSON array because downstream work can start immediately and memory does not need to scale with complete result count.

For human navigation, `outline` text can be substantially smaller than dumping structured JSON.

## 20.10 Outline implementation scaling

Modern outline extraction includes:

* streaming extraction items;
* iterator-oriented implementation;
* single traversal optimization;
* bounded work queue improvements.

This makes it appropriate as a fast **pre-read map** in agent workflows. It should not be repurposed as a semantic index just because the output is compact.

## 20.11 N-API scaling

Prefer:

```text
one Rust-side findAll
```

over:

```text
many JS callbacks into Rust for each node
```

Prefer `findInFiles` for multi-file search when it can express the task; it avoids shipping every file string/root across FFI and uses Rust-side parallelism.

## 20.12 Caching in agent systems

ast-grep CLI itself is designed to scan quickly, but a long-lived coding-agent platform may cache:

* file hashes;
* previous outline JSON;
* query result fingerprints;
* parsed semantic indexes from other tools.

Do not cache an ast-grep result without a clear invalidation key. At minimum include:

```text
file content identity
ast-grep version
parser/language version where externally controlled
rule/pattern identity
project config identity
```

## 20.13 Performance benchmark protocol

```text
1. fix corpus revision
2. warm/cold filesystem behavior deliberately
3. record ast-grep version
4. record command/rule set
5. count files and bytes scanned
6. record thread count
7. separate terminal rendering from matcher time if possible
8. compare result counts for correctness
9. run multiple trials
10. test representative CI hardware, not only workstation
```

### Anti-patterns

* optimizing thread count while scanning 10x unnecessary files;
* removing a precise kind anchor to make YAML shorter;
* using `stopBy: end` as a default habit;
* transferring entire ASTs through FFI when only matches are needed;
* collecting millions of JSON matches into one in-memory array;
* caching syntax results without file/tool/rule identity.


---

# 21) Debugging and troubleshooting — a deterministic triage playbook

## 21.0 Classify the failure before changing the rule

Most ast-grep failures belong to one of seven layers:

```text
1. version / binary mismatch
2. language / parser selection
3. query parse shape
4. matcher semantics
5. file / rule discovery
6. rewrite / transform behavior
7. output / integration interpretation
```

Debug **one layer at a time**. Changing pattern syntax, globs, config, and strictness together destroys the evidence needed to understand the failure.

Recommended order:

```text
ast-grep --version
        ↓
confirm language
        ↓
--debug-query=cst
        ↓
minimal run / inline rule
        ↓
--inspect summary|entity
        ↓
dry-run rewrite / JSON
        ↓
full project scan / CI integration
```

## 21.1 Verify the binary and current help first

```bash
ast-grep --version
ast-grep --help
ast-grep run --help
ast-grep scan --help
ast-grep outline --help
```

This reference is anchored to 0.45.1, but the installed binary is always the execution oracle for command-line flags.

On Linux, use the canonical binary name:

```bash
ast-grep
```

Do not assume `sg` resolves to ast-grep. `sg` is deprecated by ast-grep and collides with the system `sg`/`setgroups` command on common Linux environments.

## 21.2 Problem: a pattern returns no matches

### Step 1 — force the language

```bash
ast-grep run -l ts -p 'foo($A)' src/example.ts
```

If this works while inference does not, the issue is file-language mapping rather than the structural matcher.

### Step 2 — inspect the query parse

```bash
ast-grep run -l ts -p 'foo($A)' --debug-query=cst
```

Use:

| format | best use |
|---|---|
| `ast` | named-node structure |
| `cst` | named + unnamed tokens; best general debugger |
| `sexp` | compact structural comparison |
| `pattern` | ast-grep pattern/metavariable interpretation |

### Step 3 — test the smallest real source file

```bash
printf '%s\n' 'foo(x)' \
  | ast-grep run --stdin -l ts -p 'foo($A)'
```

Now the variables are reduced to:

* parser;
* pattern;
* matcher.

### Step 4 — add context if the snippet is not a legal standalone parse

```yaml
rule:
  pattern:
    context: |
      class A {
        $FIELD = $INIT
      }
    selector: field_definition
```

The rule should express the node you want, while `context` only supplies enough surrounding syntax for Tree-sitter to parse it correctly.

## 21.3 Problem: Playground works, CLI does not

The official FAQ identifies two recurring causes:

1. **parser-version skew** — the web Playground parser may not match the parser embedded in the installed CLI;
2. **encoding differences** — CLI processing is UTF-8 while the Playground environment uses UTF-16, which can alter Tree-sitter error recovery for malformed/incomplete snippets.

Do not try to reconcile the two by intuition. Inspect the CLI parse:

```bash
ast-grep run -l <lang> -p '<pattern>' --debug-query=cst
```

Then compare that exact syntax tree with the Playground representation.

Durable fix:

```text
make the pattern unambiguous and valid,
not “tune it until both error-recovery heuristics happen to agree.”
```

## 21.4 Problem: too many matches

Escalation order:

```text
1. add a more specific pattern/kind
2. add a relational constraint
3. add a field constraint
4. tighten strictness if textual/unnamed-node differences matter
5. use constraints on captured metavariables
6. limit file scope
```

Example:

```yaml
rule:
  all:
    - kind: call_expression
    - pattern: eval($ARG)
    - inside:
        kind: function_declaration
        stopBy: end
```

Avoid starting with a giant regular expression over node text. That throws away the main reason to use ast-grep.

## 21.5 Problem: repeated metavariable does not behave as expected

Capturing metavariables enforce equality by name:

```text
$A ... $A
```

means both occurrences must bind equivalently.

Non-capturing metavariables do not:

```text
$_A ... $_A
```

If equality is desired, do not prefix the name with `_`.

If punctuation/operator capture is desired, inspect the CST and use unnamed-node capture when required:

```text
$$OP
```

## 21.6 Problem: a multi-metavariable captures too much or too little

```text
$$$ARGS
```

matches a sequence in list-like syntax. The following pattern node acts as an anchor for where the sequence can stop.

Debug with a minimal source corpus containing:

```text
zero items
one item
many items
nested item
spread/rest item where legal
```

Do not use a multi-metavariable as the **root** matcher in modern ast-grep. Root multi-metavariable matching was rejected in the 0.44 line because it produces an ill-defined match root.

## 21.7 Problem: comments or trivia unexpectedly change the result

First identify the active strictness:

```bash
ast-grep run -l js -p 'foo($A)' --strictness smart ...
```

Then compare deliberately:

```bash
for s in cst smart ast relaxed signature template; do
  echo "=== $s ==="
  ast-grep run -l js -p 'foo($A)' --strictness "$s" fixture.js
 done
```

Modern releases include correctness fixes around root metavariable matching when comments are present. If a comment-sensitive edge case behaves differently across versions, reproduce on 0.45.1 before compensating in rule logic.

## 21.8 Problem: `kind` / ESQuery does not match

Distinguish:

```text
plain Tree-sitter node kind
```

from:

```text
ESQuery-style selector expression
```

Use a simple `kind` first:

```bash
ast-grep run -l ts -k call_expression file.ts
```

Then add selector relationships:

```bash
ast-grep run -l ts -k 'call_expression > identifier' file.ts
```

For pseudo-class selectors such as `:has`, `:not`, `:is`, and positional variants, construct fixtures that test the selector alone before embedding it in a larger YAML rule.

## 21.9 Problem: the right pattern works in `run` but project scan does not

This is usually a **project routing** problem.

Check:

```bash
ast-grep scan --inspect summary
ast-grep scan --inspect entity
```

Ask:

```text
Which sgconfig.yml was discovered?
What is projectDir?
Was the target file included?
Which language was assigned?
Was the rule loaded?
Was the rule disabled by severity or filtering?
Did rule files/ignores reject the path?
```

Then isolate one rule:

```bash
ast-grep scan --rule rules/my-rule.yml path/to/fixture
```

Or remove disk config from the equation:

```bash
ast-grep scan --inline-rules "$(cat rules/my-rule.yml)" path/to/fixture
```

## 21.10 Problem: a file is silently skipped

Inspect file discovery:

```bash
ast-grep run -p 'foo($A)' --inspect entity .
ast-grep scan --inspect entity .
```

Potential causes:

* hidden-file default;
* `.gitignore` / `.ignore`;
* global Git ignore;
* parent ignore;
* repo exclude;
* rule `files`;
* rule `ignores`;
* `languageGlobs` remapping;
* unsupported extension;
* explicit CLI glob exclusion.

Temporarily neutralize ignore layers **surgically**:

```bash
--no-ignore hidden
--no-ignore vcs
```

Do not default to `--no-ignore` everywhere; ignored vendor/build/generated trees are often intentionally excluded and can explode scan cost.

## 21.11 Problem: extensionless or unusual file is parsed as the wrong language

One-off:

```bash
cat .eslintrc \
  | ast-grep run --stdin -l json -p '$A'
```

Project-level:

```yaml
languageGlobs:
  json:
    - '.eslintrc'
    - '.prettierrc'
```

Remember: `languageGlobs` can override the normal extension mapping. Treat it as parser-routing policy, not merely an include filter.

## 21.12 Problem: injected-language matches do not appear

Verify three things separately:

```text
1. host file parses under hostLanguage
2. injection rule captures the intended region
3. injected language value resolves to a registered language
```

Use a tiny host fixture and run the host injection rule in isolation if needed.

`languageInjections` remains experimental. Pin the ast-grep version if an agent workflow depends on it.

## 21.13 Problem: custom language fails to load

Check:

```text
libraryPath exists
library matches host target triple / ABI
extensions route the desired files
languageSymbol is correct if non-default
expandoChar is valid for the grammar
```

For multi-platform repos, prefer:

```yaml
libraryPath:
  aarch64-apple-darwin: parser-macos-arm64.dylib
  x86_64-unknown-linux-gnu: parser-linux-x64.so
```

Then inspect the grammar itself using Tree-sitter tooling if ast-grep pattern parsing is surprising:

```bash
tree-sitter parse path/to/file
```

## 21.14 Problem: rewrite output looks wrong

Separate **matching** from **generation**.

First, prove the match:

```bash
ast-grep run -l ts -p 'old($A)' source.ts
```

Then add the rewrite without applying:

```bash
ast-grep run -l ts -p 'old($A)' -r 'new($A)' source.ts
```

Then use interactive review:

```bash
ast-grep run -l ts -p 'old($A)' -r 'new($A)' -i source.ts
```

Only after fixture tests should an unattended workflow use:

```bash
-U
```

For YAML fixes, test:

```text
capture substitution
indentation
expanded range
transform output
rewriter output
parse validity of generated code
format/lint/typecheck afterward
```

## 21.15 Problem: `FixConfig` expansion edits too much

`FixConfig` can expand the edit range around the matched node. The matcher and the edit boundary are therefore separate concepts.

Build fixtures where the target occurs:

* first in a list;
* middle in a list;
* last in a list;
* only element;
* adjacent to comments;
* multiline.

The output should remain syntactically valid in every positional case.

## 21.16 Problem: transform output is empty

Check the transform pipeline in order:

```text
source metavariable captured?
transform key references correct name?
regex/string parameters match?
output metavariable used in fix?
```

Do not assume a transform creates a capture if its input never bound.

Use `constraints` or an `all:` sequence when prior capture establishment must be ordered.

## 21.17 Problem: local/global utility is not resolving

Resolution concerns:

```text
parameter binding
local utility
project global utility
```

For experimental parameterized utility rules, all declared arguments are required and are rule objects rather than scalar values.

Temporarily inline the utility logic into the calling rule. If the match returns, the issue is utility resolution/binding rather than the atomic matcher.

## 21.18 Problem: project rule order appears nondeterministic

A plain Rule object is logically an AND over fields, but its field order is not a reliable metavariable execution order.

If one condition must capture before another consumes that capture:

```yaml
rule:
  all:
    - pattern: foo($A)
    - inside:
        pattern: bar($A)
        stopBy: end
```

Use explicit `all:` sequencing.

## 21.19 Problem: scan severity / exit behavior surprises CI

Debug the **effective** severity, not only the YAML declaration.

Sources can include:

```text
rule YAML severity
CLI --error/--warning/--info/--hint/--off overrides
inline suppression comments
special suppression governance rules
```

Run a small CI fixture and inspect the shell status explicitly:

```bash
set +e
ast-grep scan --error=my.rule fixture/
status=$?
echo "exit=$status"
set -e
```

Do not write CI logic based on an old assumption that every diagnostic level gates identically.

## 21.20 Problem: suppression does not apply

Check:

1. comment syntax for the language;
2. exact rule ID;
3. whether the suppression is on the same or preceding attachment line supported by ast-grep;
4. whether file-level placement is valid;
5. whether `no-suppress-all` or `unused-suppression` governance is intentionally reporting the comment itself.

Keep suppressions narrow:

```text
specific rule ID > blanket suppression
```

## 21.21 Problem: JSON consumer breaks

Verify:

```bash
ast-grep run ... --json=stream | head -n 1 | jq .
```

Remember:

* line and column are zero-based;
* offsets are byte-oriented in ast-grep's range representation;
* `run`, `scan`, and `outline` have related but not identical JSON shapes;
* `stream` should be consumed record-by-record;
* metadata is scan-specific and must be requested where required.

Do not deserialize all ast-grep outputs into one universal schema.

## 21.22 Problem: LSP shows no diagnostics

Check:

```text
ast-grep executable visible to editor
workspace root
sgconfig.yml discovery
ruleDirs
rule severity
file language mapping
```

Run from the same logical root:

```bash
ast-grep lsp -c /absolute/or/workspace/sgconfig.yml
```

Also run:

```bash
ast-grep scan -c /same/sgconfig.yml path/to/file
```

If CLI scan reports the issue and LSP does not, the remaining problem is editor/server wiring rather than the rule.

## 21.23 Problem: `outline` omits a symbol

First ask whether the language's current **outline extractor** models that construct. `outline` does not generically infer symbols from every Tree-sitter node; it uses bundled or supplied extraction rules.

Try:

```bash
ast-grep outline file --items all --view expanded
```

Then:

```bash
ast-grep run -l <lang> -k '<node-kind>' file
```

If `run` sees the syntax but `outline` does not, write or augment an outline extractor rather than treating the parser as broken.

For custom languages:

```yaml
customLanguages:
  mylang:
    outlineRules: outline/mylang.yml
```

## 21.24 Problem: outline import/export result looks semantically wrong

`outline` reports **syntactic** import/export/publicness signals.

It does not prove:

* module resolution;
* re-export transitivity;
* visibility after macro/code generation;
* runtime import target;
* type-level accessibility.

Use compiler/LSP/module tools for those claims.

## 21.25 Minimal known-good project harness

Use this to separate installation problems from repo complexity.

```text
/tmp/sg-smoke/
├── sgconfig.yml
├── rules/
│   └── no-console.yml
├── rule-tests/
│   └── no-console-test.yml
└── src/
    └── demo.js
```

`sgconfig.yml`:

```yaml
ruleDirs:
  - rules

testConfigs:
  - testDir: rule-tests
```

`rules/no-console.yml`:

```yaml
id: no-console
language: JavaScript
severity: warning
message: Avoid console.log.
rule:
  pattern: console.log($$$ARGS)
```

`src/demo.js`:

```javascript
console.log('hello')
```

Then:

```bash
cd /tmp/sg-smoke
ast-grep scan
ast-grep run -l js -p 'console.log($$$ARGS)' src/demo.js
ast-grep test --skip-snapshot-tests
ast-grep outline src/demo.js --view expanded
```

This validates the four main execution paths independently.

### Debugging anti-patterns

* editing the full production ruleset before reproducing one rule;
* using Playground output as the authoritative parse of the installed binary;
* adding regex complexity to compensate for an incorrectly parsed pattern;
* disabling all ignore logic rather than identifying the skipped-file cause;
* applying `-U` while still debugging the matcher;
* assuming LSP failure proves scan failure;
* treating outline omissions as semantic evidence;
* changing multiple matching layers simultaneously.

### Agent checklist

```text
[ ] Record ast-grep --version.
[ ] Force language while debugging.
[ ] Inspect --debug-query=cst.
[ ] Reduce to one source fixture.
[ ] Reduce to one pattern/rule.
[ ] Inspect project discovery separately.
[ ] Prove match before rewrite.
[ ] Validate generated syntax after rewrite.
[ ] Treat outline and LSP as separate layers.
[ ] Re-run the production command only after the minimal case is understood.
```

---

# 22) Correctness, safety, trust boundaries, and destructive-operation policy

## 22.0 ast-grep is syntax-aware, not inherently safe-to-apply

Structural matching reduces many classes of text-rewrite error, but it does **not** establish semantic equivalence.

A successful rewrite can still alter:

* overload resolution;
* type inference;
* evaluation order;
* ownership/borrowing;
* macro expansion;
* exception behavior;
* reflection;
* serialization names;
* API visibility;
* generated artifacts;
* build-system assumptions.

Therefore:

```text
match correctness ≠ rewrite semantic correctness
```

## 22.1 Mutation risk tiers

| operation | risk | recommended policy |
|---|---:|---|
| `run` search | low | freely use on known source |
| `outline` | low | freely use on known source |
| `scan` diagnostics | low | freely use on known source/config |
| rewrite dry-run | moderate | inspect diff |
| `-i` interactive apply | moderate | review each edit class |
| `-U` apply-all | high | clean VCS state + tested rule |
| custom dynamic language load | high trust | only trusted parser binaries/config |

## 22.2 VCS guard before bulk rewrite

Recommended precondition:

```bash
git status --short
```

For unattended or agentic modification:

```text
1. know whether the worktree is already dirty
2. preserve unrelated user changes
3. run search/dry-run first
4. apply only intended paths
5. inspect git diff
6. run formatter
7. run targeted tests/typecheck/lint
8. run full relevant validation
```

Do not use a bulk rewrite as an implicit formatter or conflict-resolution mechanism.

## 22.3 Untrusted `sgconfig.yml` is not merely data when custom languages are enabled

`customLanguages.libraryPath` loads a native Tree-sitter dynamic library.

That creates a code-loading trust boundary:

```yaml
customLanguages:
  example:
    libraryPath: parsers/example.so
```

An automated scanner must **not** blindly trust repository-provided native parser binaries from an untrusted checkout.

Recommended service policy:

```text
trusted repository/config?
  yes -> project customLanguages may be allowed
  no  -> reject/strip custom native language registration, or isolate it
```

Potential mitigations:

* allowlist languages/parser hashes;
* supply centrally managed parsers;
* run scanning in a sandbox/container with restricted filesystem/network privileges;
* disallow arbitrary `libraryPath` outside an approved parser directory;
* separate configuration parsing from dynamic-library activation.

## 22.4 Symlink traversal

`--follow` expands the filesystem trust boundary.

Risks:

* scanning outside intended repo tree;
* unexpectedly huge trees;
* duplicate traversal paths;
* sensitive paths reachable via symlink.

Even though ast-grep detects symlink loops, an untrusted repository can still point at locations you did not intend to inspect.

For service scanning, leave `--follow` off unless the deployment has an explicit path-containment policy.

## 22.5 Ignore overrides can expose secrets and generated data

```bash
--no-ignore hidden --no-ignore vcs
```

may cause a scan to enter:

* `.env`-like files;
* cache directories;
* hidden tooling state;
* generated outputs;
* vendored dependencies;
* local secrets not intended for analysis.

Use ignore overrides as a debugging/scoped operation, not a blanket production default.

## 22.6 Output privacy

Human/JSON output can contain:

* matched source text;
* surrounding source lines;
* captures;
* replacements;
* rule metadata;
* file paths;
* outline signatures/imports.

If results leave the developer machine, treat ast-grep output as **source-code data**.

Do not send broad `--json` or context output to external telemetry just because it is structured.

## 22.7 Path handling

Agent systems should treat paths as opaque filesystem identities where possible.

Do not:

* construct shell commands by string concatenation with untrusted paths;
* assume paths contain no whitespace/newlines/shell metacharacters;
* normalize away meaningful symlink boundaries without policy.

Prefer process APIs with argument arrays when invoking ast-grep programmatically.

## 22.8 Shell interpolation hazards in inline YAML

This is convenient:

```bash
ast-grep scan --inline-rules "$(cat rule.yml)" src
```

but generated shell strings can be fragile.

For automation:

* use `--rule path/to/rule.yml` when possible;
* if inline rules are needed, pass arguments directly through a subprocess API;
* never interpolate untrusted code/rule text into a shell command without safe argument handling.

## 22.9 Regex denial-of-service posture

ast-grep uses Rust regex semantics for its `regex` fields/filters, which deliberately omit features such as backreferences/lookaround and are designed around predictable matching behavior.

Still, regex breadth can cause excessive **work volume** even when the regex engine itself avoids classic catastrophic backtracking:

```text
very broad node candidates × huge corpus × relational traversal
```

Use a structural positive anchor first.

## 22.10 Recursive rewriters need termination reasoning

Recursive or nested rewrite systems can generate new shapes that themselves qualify for additional rewriting depending on how the workflow is orchestrated.

For every recursive rewriter/codemod, define:

```text
termination measure
maximum expected recursion/nesting
idempotence expectation
fixture for already-transformed code
```

A useful postcondition:

```bash
# after applying the codemod
ast-grep scan --rule transformation-source-rule.yml .
```

should either find zero targets or only intentionally retained cases.

## 22.11 Generated text must be reparsed

A `fix` or transform emits text. Tree-sitter structural correctness of the **input match** does not guarantee syntactic correctness of the output.

Validation sequence:

```text
apply edits
→ parse/format
→ compiler/typechecker
→ targeted tests
→ broader tests
```

For language-agnostic agent systems, at minimum rerun the language parser and formatter if available.

## 22.12 Formatting ownership

ast-grep preserves/reconstructs source around edits but is not a universal language formatter.

Decide explicitly:

```text
ast-grep owns structural edit
language formatter owns canonical formatting
```

That separation usually produces simpler rewrite rules.

## 22.13 Comments and trivia are semantics in some ecosystems

Comments may drive:

* lint suppressions;
* code generation;
* doc tests;
* type checker directives;
* coverage;
* bundler annotations;
* framework metadata.

A strictness mode that ignores comments for **matching** does not mean comments are irrelevant to the rewrite's semantic effect.

Include comment-adjacent fixtures for destructive rules.

## 22.14 Macro/template/generated-language caveat

Tree-sitter matches surface syntax before downstream expansion.

Examples where syntax-level evidence may be insufficient:

* Rust macros;
* C/C++ preprocessor;
* JSX transforms;
* template languages;
* generated ORM/model code;
* conditional compilation.

Use ast-grep to narrow/transform the surface representation, then validate under the language's actual build semantics.

## 22.15 AST parser version is part of query identity

Structural matching depends on grammar node shapes.

A parser upgrade can change:

* node kind;
* named/unnamed boundaries;
* field names;
* error recovery;
* nesting.

For production rule packs:

```text
ast-grep version + parser bundle version
```

should be treated as part of the tested compatibility surface.

## 22.16 Coordinate conversion safety

JSON line/column coordinates are zero-based. Consumers often use one-based editor/display coordinates.

Never mix them silently.

Recommended internal type distinction:

```text
SgPoint0 { line0, column0 }
EditorPoint1 { line1, column1 }
ByteOffset { offset }
```

Convert at system boundaries.

## 22.17 Concurrent edits and stale ranges

Do not compute ast-grep ranges on one file version and apply them to another.

For agent/daemon architecture:

```text
query result → content/version identity → edit validation → write
```

If the file changed in between, rerun the structural query.

This is especially important when JSON output is used as an intermediate edit plan outside ast-grep itself.

## 22.18 Idempotence as a codemod quality test

Where appropriate:

```text
apply codemod once  -> changes
apply codemod twice -> no changes
```

Idempotence is not universally required, but it is an excellent default for migrations.

Add it to test fixtures when the intended final state is canonical.

## 22.19 Negative-space testing

A structural rule can be perfectly correct on matching examples and still be unsafe due to false positives.

Every destructive rule should have:

```text
positive examples
near-miss negative examples
already-migrated examples
comment/trivia examples
nested examples
boundary/list-position examples
syntax-error examples where relevant
```

## 22.20 Security posture for an ast-grep service

Recommended deployment modes:

| deployment | posture |
|---|---|
| trusted local CLI | project config and dynamic parsers acceptable |
| CI on first-party repo | pin version; review parser/config dependencies |
| bot on fork PRs | do not trust new native parser binaries/config automatically |
| multi-tenant scanner | sandbox, path scope, resource limits, parser allowlist |
| coding-agent daemon | version-stamped results, VCS-aware writes, no stale edit application |

### Agent checklist

```text
[ ] Is the repo/config trusted?
[ ] Does config load native custom languages?
[ ] Is --follow necessary?
[ ] Are ignore overrides scoped?
[ ] Is the worktree state known before writing?
[ ] Was the matcher proven before -U?
[ ] Are rewrite fixtures negative as well as positive?
[ ] Will generated text be reparsed/formatted/typechecked?
[ ] Are ranges tied to the exact file version?
[ ] Is source-containing output handled as sensitive code data?
```

---

# 23) Agentic coding workflows — using ast-grep as a structural tool layer

## 23.0 Core model for coding agents

For an LLM coding agent, ast-grep is most valuable when assigned a **specific stage** in code understanding:

```text
filesystem/text discovery
        ↓
syntax structure     ← ast-grep's strongest layer
        ↓
semantic/type/ref facts
        ↓
change planning
        ↓
structural rewrite   ← ast-grep's second strongest layer
        ↓
compiler/test verification
```

Do not ask ast-grep to replace every adjacent layer.

## 23.1 The modern navigation loop: search → outline → targeted read

The official 0.44+ guidance explicitly positions `outline` as a cheap first pass for coding agents.

Recommended loop:

```text
1. identify candidate files
2. outline directory exports or candidate files
3. narrow by symbol/type/import
4. read only relevant source ranges
5. use run/YAML rules for precise structural facts
6. escalate to semantic tools only when needed
```

Examples:

```bash
# public map of a package
ast-grep outline src --items exports

# inspect a candidate file
ast-grep outline src/parser.ts

# zoom into one class
ast-grep outline src/parser.ts \
  --match '^Parser$' \
  --type class \
  --view expanded

# inspect dependencies
ast-grep outline src/parser.ts --items imports --view signatures
```

This is often more token-efficient than reading whole files merely to discover where the relevant declarations are.

## 23.2 Suggested repository instruction for agents

A concise repository policy can be placed in `AGENTS.md`, `CLAUDE.md`, or an equivalent agent instruction file:

```markdown
## Structural code navigation

Use `ast-grep outline` before broad full-file reads when exploring source.
Use `ast-grep run` for syntax-aware search when text search would overmatch.
Use compiler/LSP tools for type resolution, references, dispatch, and call graphs.
Before ast-grep rewrites, preview matches/diffs; use `-U` only after validating the rule on fixtures.
```

The important design choice is not the exact prose. It is making the **tool boundary** explicit so agents do not either ignore ast-grep or over-attribute semantic meaning to it.

## 23.3 Workflow: unfamiliar repository orientation

### Phase A — establish surface

```bash
find . -maxdepth 2 -type f | head
ast-grep outline src --items exports --view names
```

### Phase B — inspect entry candidates

```bash
ast-grep outline src/main.rs --view digest
ast-grep outline src/lib.rs --items imports --view signatures
```

### Phase C — structural query

```bash
ast-grep run -l rust -k 'function_item' src/
```

### Phase D — semantic escalation

Use rust-analyzer/rustc/other compiler-backed data for:

* actual references;
* inferred types;
* trait dispatch;
* macro expansion;
* call graph.

### Phase E — targeted source read

Read only the lines/ranges identified by outline/query plus enough local context to reason about implementation.

## 23.4 Workflow: locating an API usage precisely

Text-first candidate discovery:

```bash
rg 'Client' src
```

Structural refinement:

```bash
ast-grep run -l rust -p 'Client::new($$$ARGS)' src
```

Contextual constraint when needed:

```yaml
id: client-new-in-async
language: Rust
rule:
  pattern: Client::new($$$ARGS)
  inside:
    kind: function_item
    stopBy: end
severity: hint
message: Client construction site.
```

Now the agent can distinguish syntactic constructor-like calls from comments/strings/import text.

## 23.5 Workflow: assess migration blast radius

Before writing a codemod:

```text
1. count structural shapes
2. sample each shape
3. identify exceptions
4. define canonical target state
5. only then write the rewrite
```

Example:

```bash
ast-grep run -l ts \
  -p 'legacyApi($$$ARGS)' \
  --json=stream src \
  | jq -r '.file' \
  | sort | uniq -c | sort -nr
```

Then examine representative matches across:

* argument arity;
* nesting;
* chaining;
* awaited/not awaited;
* expression/statement positions;
* test/generated code.

## 23.6 Workflow: codemod development lifecycle

```text
search-only pattern
→ fixture corpus
→ rewrite dry run
→ interactive apply on sample
→ rule tests/snapshots
→ apply scoped subtree
→ inspect diff
→ format/typecheck/test
→ apply broader scope
→ verify source pattern exhausted
```

Canonical command progression:

```bash
# 1 search
ast-grep run -l ts -p 'old($A)' src

# 2 dry rewrite
ast-grep run -l ts -p 'old($A)' -r 'new($A)' src

# 3 interactive
ast-grep run -l ts -p 'old($A)' -r 'new($A)' -i src/subtree

# 4 apply after validation
ast-grep run -l ts -p 'old($A)' -r 'new($A)' -U src

# 5 prove remaining cases
ast-grep run -l ts -p 'old($A)' src
```

## 23.7 Workflow: durable lint rule authoring

Start in CLI:

```bash
ast-grep run -l js -p 'eval($A)' src
```

Promote to YAML when you need:

* context/selector;
* relational logic;
* reusable utilities;
* constraints;
* severity/message/labels;
* path routing;
* fixes/transforms/rewriters;
* tests;
* LSP/CI deployment.

Then add a rule test immediately.

Do not wait until the rule is “finished” before adding negatives; false-positive discovery is part of rule design.

## 23.8 Workflow: rule generation by an LLM

LLMs are good at proposing candidate structural rules but should be constrained by executable evidence.

Recommended loop:

```text
agent proposes rule
→ parse/debug rule pattern
→ run on positive fixtures
→ run on negative fixtures
→ inspect real-repo samples
→ revise
→ snapshot/fix test
→ only then deploy
```

A generated rule should never be accepted merely because its YAML schema is valid.

## 23.9 Workflow: query composition for an agent

Choose the smallest expressive surface:

| need | first choice |
|---|---|
| exact syntax shape | `pattern` |
| node type | `kind` |
| parent/child/sibling shape | ESQuery `kind` or relational YAML |
| text property of known node | `regex` refinement |
| ancestor/descendant with depth control | YAML relational rule |
| multiple logical clauses | `all` / `any` / `not` |
| reusable predicate | utility + `matches` |
| captured-node qualification | `constraints` |
| output text derivation | `transform` |
| nested structural generation | rewriter |

Avoid prematurely jumping to the most expressive YAML surface.

## 23.10 Workflow: dependency direction with outline

Local dependency scan:

```bash
ast-grep outline src --items imports --view signatures
```

Filter import entries:

```bash
ast-grep outline src \
  --items imports \
  --match 'datafusion|arrow' \
  --view signatures
```

This is useful for deciding which files to inspect next.

It does **not** resolve the import to a concrete module/file. Follow with language tooling or repository conventions.

## 23.11 Workflow: public API inventory

```bash
ast-grep outline src --items exports --view names
```

Then focused signatures:

```bash
ast-grep outline src \
  --items exports \
  --type function,class,struct,enum \
  --view signatures
```

Use cases:

* API review;
* change-impact planning;
* documentation gaps;
* locating extension points;
* candidate surface for compatibility checks.

## 23.12 Workflow: changed-file structural review

After editing:

```bash
git diff --name-only --diff-filter=ACMR HEAD \
  | tr '\n' '\0' \
  | xargs -0 -r ast-grep outline --items exports --view signatures
```

This is safer than command substitution for filenames with spaces.

For a multi-language changed set, let extension inference work or partition by language when explicit language selection is necessary.

## 23.13 Workflow: “find related code” without pretending syntax equals semantics

Use ast-grep for **structural relations you can state explicitly**:

```text
same callee syntax
same constructor form
same decorator/attribute
same enclosing syntactic kind
same import form
same method declaration shape
```

Do not translate vague “related” into an arbitrary AST neighborhood.

If the relation is semantic:

```text
calls this function
implements this trait/interface
references this symbol after aliasing
inherits after type resolution
```

use compiler/LSP/index facts.

## 23.14 Workflow: combine ast-grep with ripgrep

A strong hybrid:

```text
rg for cheap lexical candidate narrowing
→ ast-grep for syntax precision
```

Example:

```bash
rg -l 'unsafe' crates/ \
  | xargs ast-grep run -l rust -k unsafe_block
```

Conversely:

```text
ast-grep outline to locate structural surface
→ rg inside the small candidate set for literals/docs/config names
```

Neither tool needs to replace the other.

## 23.15 Workflow: combine with compiler/LSP facts

Pattern:

```text
semantic tool returns source ranges/files
→ ast-grep targets the syntax at those locations
→ ast-grep emits/executes structural rewrite
→ semantic tool revalidates
```

The `range` atomic rule is particularly useful when an external analyzer has already identified a source interval.

Example conceptual YAML:

```yaml
rule:
  all:
    - kind: call_expression
    - range:
        start: { line: 10, column: 4 }
        end: { line: 10, column: 18 }
```

This lets ast-grep become the syntax-aware edit layer beneath a semantic detector.

## 23.16 Workflow: long-lived code-intelligence daemon

If ast-grep is integrated into a daemon rather than used as a transient CLI:

```text
watch file changes
→ hash/version file
→ selectively refresh syntax-derived facts
→ keep semantic/index layers separate
→ serve version-stamped results
```

Potential ast-grep-derived facts:

* structural query matches;
* symbol outline entries;
* import/export syntax;
* selected custom lint facts.

Do not infer a global call/reference graph from these alone.

## 23.17 Workflow: repository-specific outline extraction

When default outline coverage misses an important project construct, encode it.

Example conceptual extractor:

```yaml
id: project-handler
language: TypeScript
role: item
symbolType: function
rule:
  pattern: registerHandler($NAME, $HANDLER)
name: $NAME
signature: registerHandler($NAME, ...)
isExported: true
```

Then:

```bash
ast-grep outline src --outline-rules tooling/project-outline.yml
```

This can expose domain-level syntactic constructs to agents without modifying the main application source.

## 23.18 Workflow: custom language onboarding

```text
compile Tree-sitter parser
→ register customLanguages
→ verify parser with run/debug-query
→ add languageGlobs/extensions
→ write structural rules
→ add outlineRules
→ test on positive/negative corpus
→ distribute per-platform parser binaries securely
```

A language is not “agent-ready” merely because `run -l custom` parses it. Good navigation also needs outline extraction rules for important declarations/members.

## 23.19 Workflow: monorepo policy

For large monorepos, decide whether ast-grep is:

```text
one root project
or
multiple independently versioned/configured projects
```

Use explicit `-c` paths in automation when CWD-based upward discovery could select the wrong root.

Recommended CI style:

```bash
ast-grep scan -c tooling/ast-grep/sgconfig.yml packages/foo
```

rather than depending on whichever directory the CI runner happened to `cd` into.

## 23.20 Workflow: review-only / no-write agent

Use:

```text
outline
run
scan without fix application
JSON/SARIF output
```

Do not grant `-U` or file-write capability merely because the same binary supports it.

This separation is useful for security review agents and pre-merge audit bots.

## 23.21 Workflow: machine-readable agent protocol

Prefer streaming JSON:

```bash
ast-grep run ... --json=stream
ast-grep scan ... --json=stream
ast-grep outline ... --json=stream
```

Attach an envelope in your own system:

```json
{
  "tool": "ast-grep",
  "toolVersion": "0.45.1",
  "operation": "run",
  "queryId": "...",
  "repoRevision": "...",
  "fileVersionPolicy": "content-hash",
  "results": "streamed separately"
}
```

Do not make downstream consumers infer version/config identity from raw matches.

## 23.22 Workflow: result confidence classification

A useful agent annotation:

| fact | confidence category |
|---|---|
| node matched exact pattern | syntax-confirmed |
| symbol shown in outline | syntax-extracted |
| import statement present | syntax-confirmed |
| import resolves to file X | **not established by ast-grep** |
| call syntax `foo()` present | syntax-confirmed |
| runtime dispatch invokes function X | **not established by ast-grep** |
| rewrite transformed exact matched AST | edit-confirmed |
| rewrite preserves behavior | **requires semantic/test evidence** |

This prevents downstream reasoning from upgrading syntax facts into semantic facts implicitly.

## 23.23 Agent prompt pattern for structural queries

When delegating rule creation to an LLM, provide:

```text
language + version/context
positive source examples
negative near-miss examples
desired matched node
whether captures are needed
whether rewrite is required
allowed scope
validation command
```

Better:

```text
“Match the call_expression representing foo.bar(x) only when it is inside an async
function. Capture x as $ARG. Do not match foo.bar() or other methods. Return YAML
plus positive/negative fixtures.”
```

Worse:

```text
“write an ast-grep rule for foo”
```

## 23.24 Agent policy: when **not** to use ast-grep

Prefer another tool when the primary question is:

* exact arbitrary text / binary content → `rg`, grep, byte tools;
* symbol definition/reference after name resolution → LSP/compiler/index;
* inferred type → type checker/compiler;
* dynamic dispatch/runtime call graph → semantic/runtime analysis;
* whole-program dataflow → dedicated analysis;
* formatting → language formatter;
* Git history → Git/gix;
* file-change notification → filesystem watcher;
* persistent graph query → CPG/index substrate.

ast-grep remains valuable as a syntax filter inside several of these workflows.

## 23.25 Official AI companion surfaces

The ast-grep project now documents several AI-oriented companion workflows. These are **ecosystem/documentation surfaces**, not additional 0.45.1 CLI subcommands.

### Claude Code skill

The official AI guide points to an ast-grep skill that teaches an agent to author and iteratively debug structural rules. This is useful when the coding environment has a native skill mechanism.

The important operational principle is the skill's workflow rather than any one agent vendor:

```text
break the structural question into subrules
→ test subrules
→ compose relational/composite rule
→ inspect AST/pattern parse when it fails
→ test against source examples
→ iterate until positive and negative behavior is correct
```

### `AGENTS.md` / repository prompting

For lightweight adoption, the official guide recommends putting ast-grep usage guidance in repository-level agent instructions. In modern 0.44+ environments, combine that with the outline-first policy documented above.

### LLM-optimized documentation bundle

The project publishes:

```text
https://ast-grep.github.io/llms.txt
https://ast-grep.github.io/llms-full.txt
```

The full bundle is useful when an agent must generate complex rules without relying on stale memorized syntax. A production agent should still verify version-sensitive CLI/schema details against its installed binary.

### Experimental `ast-grep-mcp`

The official AI guide also points to an **experimental external MCP server** for developing/refining ast-grep rules through tool calls.

Treat it as a separate project/integration layer:

```text
ast-grep core/CLI capability
        ≠
ast-grep-mcp server lifecycle/API/stability
```

If deploying it, version and secure the MCP server independently and preserve the same rule-fixture validation loop described in this reference.

## 23.26 Playground and interactive rule prototyping

The online Playground is an official companion tool for trying structural patterns/rules against sample source and sharing a reproducer.

Best uses:

* rapid rule prototyping;
* demonstrating positive/negative examples;
* sharing a minimal structural-search issue;
* learning Tree-sitter node/pattern behavior.

Do not use Playground success as final evidence for a production rule because parser versions and text encoding can differ from the installed CLI. The durable workflow is:

```text
prototype in Playground
→ reproduce with installed CLI
→ --debug-query when parse differs
→ add repository rule tests/snapshots
```

### Agent checklist

```text
[ ] Use outline before broad reads when syntax mapping is enough.
[ ] Use run for one-off structural questions.
[ ] Promote to YAML only when richer logic/deployment is needed.
[ ] Distinguish syntax facts from semantic facts in downstream reasoning.
[ ] Use streaming JSON for machine pipelines.
[ ] Version-stamp cached results.
[ ] Keep a clean mutation/validation boundary.
[ ] Pair rewrites with compiler/test evidence.
[ ] Teach repository agents the ast-grep tool boundary explicitly.
```

---

# 24) Capability matrices and quick-reference appendices

## 24.0 Command selection matrix

| command | primary purpose | project config required? | mutation capable? | machine output |
|---|---|---:|---:|---|
| `run` | one structural query / rewrite | no | yes | JSON |
| `outline` | compact syntax structure/navigation | no for built-ins; config helps custom languages | no | JSON |
| `scan` | run one/many YAML rules | project mode yes; `--rule`/inline no | yes | JSON/GitHub/SARIF |
| `test` | rule fixture/snapshot tests | normally yes | snapshots only | test report |
| `new` | scaffold project/rule/test/util | depends on subcommand | creates files | terminal |
| `lsp` | editor diagnostics/actions | yes | via code actions | LSP protocol |
| `completions` | shell completion generation | no | no | script to stdout |
| `help` | help | no | no | terminal |

## 24.1 Current top-level CLI inventory in 0.45.x

```text
run
outline
scan
test
new
lsp
completions
help
```

Historical references that omit `outline` are pre-0.44-era inventories.

## 24.2 `run` option matrix

| option | purpose | notes |
|---|---|---|
| `-p, --pattern` | structural code pattern | one of pattern/kind query routes |
| `-k, --kind` | node kind or ESQuery selector | 0.43+ selector support in run |
| `--selector` | select subnode from parsed pattern | useful with context-heavy pattern |
| `-r, --rewrite` | replacement text | candidate edit until applied |
| `-l, --lang` | query language | required for stdin/debug-query cases |
| `--debug-query` | query parse introspection | `pattern/ast/cst/sexp` |
| `--strictness` | pattern matching strictness | six values incl. `template` |
| `--follow` | follow symlinks | expands traversal boundary |
| `--no-ignore` | disable ignore layer | repeatable |
| `--stdin` | parse stdin | language must be known |
| `--globs` | include/exclude path globs | later matching glob wins |
| `-j, --threads` | approximate thread count | `0`/heuristic default |
| `-i, --interactive` | interactive rewrite review | conflicts with JSON/stdin constraints where applicable |
| `-U, --update-all` | apply all rewrites | destructive |
| `--json` | structured match output | `pretty/stream/compact` |
| `--color` | color policy | `auto/always/ansi/never` |
| `--inspect` | discovery trace | stderr |
| `--heading` | filename heading style | `auto/always/never` |
| `-A/-B/-C` | context lines | human output |

## 24.3 `outline` option matrix

| option | purpose |
|---|---|
| `--items auto|structure|exports|imports|all` | choose top-level entry class |
| `--view auto|names|signatures|digest|expanded` | choose detail level |
| `--match <REGEX>` | filter top-level entries by useful text fields |
| `--type <CSV>` | filter top-level symbol types |
| `--pub-members` | hide private members in member-bearing views |
| `-l, --lang` | explicit language |
| `-c, --config` | project config / custom languages |
| `--stdin` | outline stdin |
| `--outline-rules` | add custom extractors |
| `--no-default-outline-rules` | replace built-in extractors |
| `--follow` | follow symlinks |
| `--no-ignore` | ignore-layer override |
| `--globs` | path selection |
| `-j, --threads` | concurrency |
| `--json` | `pretty/stream/compact` structured output |
| `--color` | text color policy |

## 24.4 `scan` control-plane matrix

| category | options |
|---|---|
| config | `-c/--config` |
| single rule | `-r/--rule` |
| inline rules | `--inline-rules` |
| rule subset | `--filter` |
| metadata | `--include-metadata` |
| rewrite | `-i`, `-U` |
| severity overrides | `--error`, `--warning`, `--info`, `--hint`, `--off` |
| terminal | `--report-style`, `--color` |
| CI | `--format github|sarif` |
| machine | `--json` |
| discovery | `--inspect` |
| paths | `--globs`, `--no-ignore`, `--follow`, PATHS |
| concurrency | `-j` |
| stdin | `--stdin` |

## 24.5 Query-surface choice matrix

| question | surface |
|---|---|
| “Does this exact code form exist?” | `pattern` |
| “Find every node of kind X.” | `kind` |
| “Find X under/next to Y concisely.” | ESQuery `kind` |
| “Find X under Y with capture/stop/field logic.” | YAML relational rule |
| “Find node text matching regex.” | `kind` + `regex` |
| “Require several conditions.” | `all` |
| “Allow alternatives.” | `any` |
| “Exclude a case.” | `not` |
| “Reuse predicate.” | `matches` utility |
| “Check captured node.” | `constraints` |

## 24.6 Pattern strictness matrix

| mode | rough intent | typical use |
|---|---|---|
| `cst` | exact concrete syntax nodes | punctuation/trivia-sensitive structural matching |
| `smart` | default practical structural match | general code search |
| `ast` | named AST-oriented match | ignore more concrete syntax details |
| `relaxed` | AST-like with comments relaxed | code shape where comments should not block match |
| `signature` | structure while ignoring source text details | signature/shape-oriented matching |
| `template` | text-oriented template matching while node kinds are ignored | cases where source-template shape matters more than parser node-kind identity |

Always verify edge behavior with the installed version when strictness choice is correctness-critical.

## 24.7 Metavariable matrix

| form | meaning | capture? | equality by repeated name? |
|---|---|---:|---:|
| `$A` | one named AST node | yes | yes |
| `$_` / `$_A` | one named AST node | no | no |
| `$$A` | one node including unnamed-node use case | yes | yes |
| `$$$A` | zero-or-more sequence | yes | yes/sequence semantics |
| `$$$` | anonymous sequence | no named capture | no |

Root multi-metavariable queries are not a supported modern match-root strategy.

## 24.8 Atomic Rule object matrix

| field | input | purpose |
|---|---|---|
| `pattern` | string or pattern object | structural code pattern |
| `kind` | kind / selector string | node-kind/ESQuery match |
| `regex` | Rust regex | match full node text |
| `nthChild` | number / An+B / object | sibling position |
| `range` | start/end points | source-range match |

A practical rule should have a positive structural anchor whenever possible.

## 24.9 Relational Rule object matrix

| relation | main-node relation | `field`? | `stopBy`? |
|---|---|---:|---:|
| `inside` | target has matching ancestor | yes | yes |
| `has` | target has matching descendant | yes | yes |
| `precedes` | matching node appears after target | no | yes |
| `follows` | matching node appears before target | no | yes |

`stopBy` forms:

```text
neighbor  (default)
end
Rule object
```

## 24.10 Composite matrix

| operator | meaning | special note |
|---|---|---|
| `all` | logical AND | guarantees listed evaluation order |
| `any` | logical OR | alternatives on same candidate node |
| `not` | negation | capture leakage bugs were fixed in modern releases |
| `matches` | utility-rule invocation | supports local/global/parameterized utility patterns |

## 24.11 ESQuery selector matrix

Core combinators supported by modern ast-grep include:

| syntax | relation |
|---|---|
| `A > B` | direct child |
| `A B` | descendant |
| `A + B` | adjacent/next sibling |
| `A ~ B` | following sibling |
| `A, B` | selector alternatives |
| `A:has(...)` | descendant/relational existence condition |
| `A:not(...)` | exclusion |
| `A:is(...)` | alternatives/grouping |
| `A:nth-child(...)` | forward positional selection |
| `A:nth-last-child(...)` | reverse positional selection |

These are ast-grep's **supported ESQuery-style subset**, not a promise of full browser CSS or every external ESQuery implementation feature.

## 24.12 Rule YAML layers

```text
Rule object
  = matching predicate

RuleConfig / rule YAML
  = identity + language + matching + diagnostics + fix + paths + metadata

sgconfig.yml
  = project discovery + rule/test/util dirs + language routing/customization

outline extractor YAML
  = syntax-to-outline-item/member mapping
```

Do not put project routing concerns into the Rule object or matching concerns into `sgconfig.yml` merely because all are YAML.

## 24.13 Executable rule top-level field catalog

The major surface includes:

```text
id
language
severity
message
note
labels
metadata
url
files
ignores
rule
constraints
utils
transform
fix
rewriters
```

Some fields are optional and some capabilities are context-dependent. Validate the current schema/CLI when generating a rule pack programmatically.

## 24.14 `sgconfig.yml` capability catalog

Major project-level surfaces:

```text
ruleDirs
utilDirs
testConfigs
languageGlobs
customLanguages
languageInjections  [experimental]
```

Custom language fields:

```text
libraryPath
extensions
outlineRules
expandoChar
languageSymbol
```

`libraryPath` may be a per-target-triple mapping for cross-platform projects.

## 24.15 Outline extractor field matrix

Common:

```text
id
language
role: item | member
symbolType
name
signature
rule
constraints
utils
transform
```

Item-specific:

```text
isImport
isExported
```

Member-specific:

```text
parentRuleIds
isPublic
```

An outline extractor is a **projection schema** from language syntax into ast-grep's intentionally small common symbol model.

## 24.16 Transform matrix

Major transform families:

| transform | use |
|---|---|
| `replace` | regex/text replacement of capture text |
| `substring` | select substring from capture |
| `convert` | case/separator conversion |
| `rewrite` | invoke structural rewriter and combine output |

Common convert styles:

```text
lowerCase
upperCase
capitalize
camelCase
snakeCase
kebabCase
pascalCase
```

Use the current schema for exact separator-token spelling when generating YAML mechanically.

## 24.17 Rewrite hierarchy

```text
simple string fix
        ↓
FixConfig with range expansion
        ↓
transformed metavariable in fix
        ↓
rewriter selection
        ↓
recursive/nested structural rewrite
```

Choose the lowest layer that can express the migration.

## 24.18 Output-format matrix

| mode | run | scan | outline | best use |
|---|---:|---:|---:|---|
| terminal text | yes | yes | yes | human/agent context |
| JSON pretty | yes | yes | yes | debugging/small machine output |
| JSON compact | yes | yes | yes | compact arrays |
| JSON stream | yes | yes | yes | large pipelines |
| GitHub annotations | no | yes | no | Actions diagnostics |
| SARIF | no | yes | no | code-scanning ingestion |

## 24.19 JSON coordinate contract

Treat ast-grep output as:

```text
range.start.line    zero-based
range.start.column  zero-based
range.end.line      zero-based
range.end.column    zero-based
offset              byte-oriented source offset
```

Convert once at UI/editor boundaries.

## 24.20 Human output decision

```text
Need nearby source?      run -C N
Need compact repo map?   outline --view names
Need signatures?         outline --view signatures
Need members?            outline --view digest|expanded
Need lint diagnostics?   scan --report-style rich
Need terse CI logs?      scan --report-style short
```

## 24.21 API surface matrix

| ecosystem | package/crate | core objects | typical use |
|---|---|---|---|
| Rust | `ast-grep-core` (+ language/config crates) | `AstGrep`, `Node`, matchers | native embedding / custom tooling |
| Node.js | `@ast-grep/napi` | `SgRoot`, `SgNode`, config/edit objects | JS/TS tools |
| Python | `ast-grep-py` | `SgRoot`, `SgNode`, Rule/Config/Edit | Python orchestration |
| browser/WASM | ast-grep WASM package | WASM binding surface | browser/playground-like embedding |
| shell | `ast-grep` CLI | commands/JSON | language-neutral processes/agents |

Binding release versions can lag the Rust workspace/CLI. Pin each package independently.

## 24.22 Syntax vs semantic capability matrix

| capability | ast-grep status |
|---|---|
| parse source to Tree-sitter syntax tree | yes |
| exact/structural syntax search | yes |
| relational AST search | yes |
| regex refinement of syntax nodes | yes |
| syntax-aware rewrite | yes |
| lint diagnostics/severity/suppression | yes |
| rule tests/snapshots | yes |
| compact local outline | yes, experimental/alpha-era surface |
| import/export syntax identification | yes, extractor-dependent |
| type inference | no |
| reference resolution | no |
| overload resolution | no |
| full call graph | no |
| dataflow/taint analysis | no built-in whole-program semantic engine |
| transitive re-export resolution | no |
| incremental editor syntax reuse as an exposed ast-grep query service | not the CLI's contract |

## 24.23 Built-in/custom/injected language distinction

```text
built-in language
  parser ships with the relevant distribution

custom language
  user registers Tree-sitter native parser + extensions

injected language
  ast-grep locates embedded regions in a host language and parses them as another language
```

These are distinct capabilities and have different deployment/trust implications.

## 24.24 Current documentation-drift ledger

This is intentionally included because coding agents often scrape a single reference page and over-generalize it.

### Release UI vs workspace version

At the time of this reference:

```text
workspace package version: 0.45.1
GitHub Releases “Latest” UI: 0.45.0
```

Use the released package/binary you actually install as the execution anchor.

### Strictness reference drift

Current CLI help/reference includes:

```text
template
```

among strictness values, while some rule-reference prose may lag that addition.

### Language table drift

Recent changelog/blog updates include language changes such as Dart restoration and Markdown support that may appear before every static language table is updated.

### Test snapshot default drift

Different current documentation surfaces have shown different textual defaults for the snapshot directory name. Production rule repositories should set `--snapshot-dir` or project test configuration explicitly if path stability matters.

### Changelog-only flags

A historical changelog can mention a flag that is no longer present in current generated CLI docs. Do not infer current support from changelog presence alone.

Rule:

```text
current binary --help > current generated CLI reference > release notes > old tutorials
```

for **CLI flag existence**.

## 24.25 Version feature timeline relevant to 0.45.1

| line | notable capability/change |
|---|---|
| 0.39 | broader ESQuery-style `kind` foundation |
| 0.40 | path config improvements; later case-insensitive file config support |
| 0.41 | WASM work; rule ID defaulting improvements |
| 0.42 | parameterized utility rules; ESQuery pseudo-selectors; injected-language LSP improvements |
| 0.42.x | Dart restoration; reverse positional selector support |
| 0.43 | Markdown support; ESQuery-style selector in `run`; test traversal improvements |
| 0.44 | `outline` alpha preview; extraction/performance work; matching correctness fixes |
| 0.44.1 | custom-language outline rules; broader bundled outline coverage; bounded queue improvements |
| 0.45 | outline refinements; alias deprecation and robustness fixes |
| 0.45.1 | correctness fixes including comment/root-metavariable behavior and outline refinements |

The table is an upgrade orientation aid, not a substitute for the complete changelog.

## 24.26 Rule-test quality matrix

For a serious rule, include where relevant:

```text
[ ] simplest positive
[ ] simplest negative
[ ] near-miss syntax
[ ] alternate whitespace
[ ] comments/trivia
[ ] nested occurrence
[ ] multiple occurrences in file
[ ] zero/one/many sequence capture
[ ] list first/middle/last position
[ ] already-fixed source
[ ] malformed/error-recovery source if relevant
[ ] path include/exclude behavior
[ ] suppression behavior
[ ] fix snapshot
[ ] idempotence
```

## 24.27 Codemod production-readiness matrix

```text
[ ] source query count understood
[ ] source shapes sampled
[ ] false-positive fixtures written
[ ] output syntax parses
[ ] formatter clean
[ ] typecheck/compiler clean
[ ] targeted tests clean
[ ] full relevant suite clean
[ ] second codemod pass behaves as intended
[ ] source pattern exhaustion checked
[ ] unrelated worktree edits preserved
```

---

# 25) Upgrade policy, source-of-truth hierarchy, and final LLM-agent execution playbook

## 25.0 Version pinning policy

For deterministic automation, pin ast-grep rather than using an unconstrained “latest” installer.

Record at least:

```text
CLI version
installation channel/package
platform/architecture
project sgconfig identity
rule-pack revision
custom parser identities
```

For binding-based applications, pin the binding version separately from the CLI.

## 25.1 Source-of-truth hierarchy

Use different source priorities for different questions.

### Exact CLI option exists?

```text
1. installed `ast-grep <command> --help`
2. current generated CLI reference
3. same-version release source
4. changelog/blog
5. older guides/examples
```

### YAML schema / rule field semantics?

```text
1. same-version schema/source/reference
2. current official rule/config reference
3. guide examples
4. blog/changelog
```

### Version introduction / behavioral change?

```text
1. changelog/release source
2. versioned source/tests
3. blog announcement
```

### Binding API?

```text
1. exact installed package type declarations/rustdoc/Python API
2. package-specific current docs
3. generic ast-grep guide
```

Never use one source hierarchy for every kind of fact.

## 25.2 Upgrade procedure

When moving beyond 0.45.1:

```text
1. read changelog from current pinned version to target
2. compare `ast-grep --help` and each used subcommand
3. run rule tests and snapshots without updating them
4. run representative `run` query golden tests
5. run outline golden/schema tests if consumed programmatically
6. validate custom language loading on each target platform
7. validate injected-language cases
8. compare JSON schemas/fields actually consumed
9. run dry scan over production corpus and compare finding counts
10. only then update snapshots/goldens intentionally
```

## 25.3 Golden testing for machine consumers

If your system parses ast-grep JSON, keep small golden fixtures for:

```text
run single capture
run multi capture
run replacement
scan rule metadata
scan suppression/severity case
outline item/member/import/export
Unicode coordinate case
empty result
multiple files in stream mode
```

Parse semantically rather than byte-comparing field order unless order is actually part of your own contract.

## 25.4 Rule-pack compatibility declaration

Recommended README block:

```text
ast-grep compatibility
- tested CLI: 0.45.1
- minimum supported CLI: <explicit if tested>
- custom parsers: <versions/hashes>
- experimental features used: parameterized utils / language injections / outline rules
- CI command: <exact>
- rule test command: <exact>
```

This is far more useful to an agent than “requires recent ast-grep.”

## 25.5 Final decision tree

```text
Need a compact syntax map?
  → outline

Need one structural query?
  → run

Need project linting / reusable complex rule?
  → scan + rule YAML

Need to test a rule?
  → test

Need editor diagnostics?
  → lsp

Need in-process AST traversal/query?
  → Rust / N-API / Python / WASM binding

Need type/ref/call semantics?
  → compiler/LSP/index/CPG, optionally narrowed by ast-grep
```

## 25.6 Final query-authoring decision tree

```text
Can a code pattern express it?
  yes → pattern
  no  ↓

Is it primarily a node-kind relationship?
  yes → kind / ESQuery
  no  ↓

Need ancestor/descendant/sibling depth/field logic?
  yes → relational YAML
  no  ↓

Need Boolean composition?
  → all / any / not

Need reuse?
  → utility + matches

Need captured-node qualification?
  → constraints

Need generated text?
  → fix / transform / rewriter
```

## 25.7 Final rewrite decision tree

```text
Simple replacement with captures?
  → string fix/rewrite

Need edit boundary expansion?
  → FixConfig

Need derived text?
  → transform

Need syntax-conditional nested output?
  → rewriter

Before -U:
  → fixtures + dry diff + clean VCS scope

After apply:
  → parse + format + typecheck/compiler + tests
```

## 25.8 LLM-agent invariant set

An agent using ast-grep should preserve these invariants:

```text
I1. Never claim semantic resolution from syntax-only evidence.
I2. Never apply a rewrite before proving the match set.
I3. Never treat a Playground parse as stronger than the installed CLI parse.
I4. Never consume project config from an untrusted repo without considering native custom-language loading.
I5. Never apply stale source ranges to a changed file.
I6. Never rely on CWD config discovery when deterministic automation requires a fixed config.
I7. Never let broad JSON/source output cross trust boundaries accidentally.
I8. Never upgrade ast-grep without rerunning rule/query golden tests.
I9. Never use a complex matcher when a simpler positive structural anchor suffices.
I10. Never hide experimental-feature dependence from the deployment contract.
```

## 25.9 Fast daily-use command card

```bash
# version/help
ast-grep --version
ast-grep --help

# structural search
ast-grep run -l <lang> -p '<pattern>' <paths>

# kind / ESQuery
ast-grep run -l <lang> -k '<kind-or-selector>' <paths>

# parse debugger
ast-grep run -l <lang> -p '<pattern>' --debug-query=cst

# compact code map
ast-grep outline <path>
ast-grep outline <dir> --items exports --view names

# machine outline
ast-grep outline <path> --json=stream

# project scan
ast-grep scan -c sgconfig.yml

# one rule
ast-grep scan --rule rules/example.yml <path>

# scan machine output
ast-grep scan --json=stream

# CI integrations
ast-grep scan --format github
ast-grep scan --format sarif

# rule tests
ast-grep test

# dry rewrite
ast-grep run -l <lang> -p '<old>' -r '<new>' <paths>

# interactive rewrite
ast-grep run -l <lang> -p '<old>' -r '<new>' -i <paths>

# apply only after validation
ast-grep run -l <lang> -p '<old>' -r '<new>' -U <paths>

# discovery debugger
ast-grep scan --inspect entity
```

## 25.10 Minimal project card

```yaml
# sgconfig.yml
ruleDirs:
  - rules

utilDirs:
  - utils

testConfigs:
  - testDir: rule-tests

languageGlobs:
  json:
    - '.eslintrc'
```

Rule:

```yaml
id: example-rule
language: TypeScript
severity: warning
message: Example diagnostic.
rule:
  pattern: oldApi($ARG)
fix: newApi($ARG)
```

## 25.11 Deployment recommendations

| deployment | recommended ast-grep stance |
|---|---|
| developer workstation | CLI + outline + interactive rewrites + editor LSP |
| repository CI | pinned CLI, explicit config, test rules first, SARIF/GitHub as needed |
| migration bot | version-pinned dry-run/apply pipeline, clean VCS checks, semantic validation |
| coding agent | outline-first navigation, JSON stream for precise facts, targeted structural queries |
| code-intelligence daemon | subprocess or embedded API, content-versioned cache, syntax facts kept separate from semantic graph |
| untrusted multi-tenant scanner | no untrusted native parser loading, no symlink escape, resource/path limits, read-only by default |

## 25.12 Anti-pattern inventory

* **Avoid:** treating ast-grep as grep with a fancier regex syntax.
* **Avoid:** treating syntax as resolved semantics.
* **Avoid:** reading whole repositories when `outline` can first map candidate structure.
* **Avoid:** running `outline` and then inventing a call graph from proximity.
* **Avoid:** using `regex` as the only broad root when a positive AST kind exists.
* **Avoid:** relying on implicit rule-field evaluation order when captures are dependent.
* **Avoid:** unbounded `stopBy: end` everywhere.
* **Avoid:** project-wide `-U` as the first test of a codemod.
* **Avoid:** accepting LLM-generated rule YAML without executable positive/negative fixtures.
* **Avoid:** loading native custom-language libraries from an untrusted checkout.
* **Avoid:** consuming huge pretty JSON arrays when stream mode exists.
* **Avoid:** parsing output text when JSON is available for machine integration.
* **Avoid:** assuming all bindings and CLI packages share one release number.
* **Avoid:** assuming a changelog-mentioned flag still exists in current help.
* **Avoid:** assuming built-in language lists update atomically across every docs page.
* **Avoid:** ignoring parser-version changes in structural query regression tests.
* **Avoid:** applying external range plans after a file has changed.

## 25.13 Final agent checklist

```text
Before querying
[ ] Verify installed version if behavior matters.
[ ] Identify syntax vs semantic question.
[ ] Use outline for cheap structural navigation where appropriate.
[ ] Select explicit language when inference is ambiguous.

While authoring
[ ] Start with the simplest positive structural anchor.
[ ] Use --debug-query=cst for parse uncertainty.
[ ] Add relational/composite logic only as required.
[ ] Use explicit all: when capture order matters.
[ ] Use non-capturing metavars for throwaway values.

Before deploying a rule
[ ] Add positive and near-miss negative fixtures.
[ ] Test path filters and suppressions if used.
[ ] Test fixes/snapshots.
[ ] Pin/declare experimental features.

Before writing code
[ ] Know VCS/worktree state.
[ ] Prove the match set.
[ ] Review dry diff or interactive sample.
[ ] Scope paths deliberately.

After writing code
[ ] Inspect diff.
[ ] Reparse/format.
[ ] Typecheck/compile.
[ ] Run targeted and broader tests.
[ ] Check intended source pattern exhaustion/idempotence.

For services/agents
[ ] Treat config/native parser loading as a trust boundary.
[ ] Tie results to file/content/tool/config identity.
[ ] Use JSON stream for machine pipelines.
[ ] Keep syntax facts distinct from semantic indexes.
```

---

# Primary official references used for this document

The URLs below are deliberately kept as durable source anchors for agents that need to re-verify a detail against a later ast-grep release.

## Release / source

* ast-grep repository: https://github.com/ast-grep/ast-grep
* workspace manifest: https://github.com/ast-grep/ast-grep/blob/main/Cargo.toml
* changelog: https://github.com/ast-grep/ast-grep/blob/main/CHANGELOG.md
* releases: https://github.com/ast-grep/ast-grep/releases

## CLI

* CLI reference: https://ast-grep.github.io/reference/cli.html
* run: https://ast-grep.github.io/reference/cli/run.html
* outline: https://ast-grep.github.io/reference/cli/outline.html
* scan: https://ast-grep.github.io/reference/cli/scan.html
* test: https://ast-grep.github.io/reference/cli/test.html
* new: https://ast-grep.github.io/reference/cli/new.html
* tooling overview: https://ast-grep.github.io/guide/tooling-overview.html

## Matching / rules

* pattern syntax: https://ast-grep.github.io/guide/pattern-syntax.html
* pattern deep dive: https://ast-grep.github.io/advanced/pattern-parse.html
* Rule object reference: https://ast-grep.github.io/reference/rule.html
* rule config reference: https://ast-grep.github.io/reference/yaml.html
* atomic rules: https://ast-grep.github.io/guide/rule-config/atomic-rule.html
* relational rules: https://ast-grep.github.io/guide/rule-config/relational-rule.html
* composite rules: https://ast-grep.github.io/guide/rule-config/composite-rule.html
* utility rules: https://ast-grep.github.io/guide/rule-config/utility-rule.html
* transforms: https://ast-grep.github.io/guide/rewrite-code/transform.html
* rewriters: https://ast-grep.github.io/guide/rewrite-code/rewriter.html
* multi-option interactive fixes: https://ast-grep.github.io/blog/interactive-demo

## Project / language

* sgconfig reference: https://ast-grep.github.io/reference/sgconfig.html
* built-in languages: https://ast-grep.github.io/reference/languages.html
* custom language guide: https://ast-grep.github.io/advanced/custom-language.html
* language injection guide: https://ast-grep.github.io/advanced/language-injection.html

## Outline / coding-agent navigation

* outline guide: https://ast-grep.github.io/guide/outline-code.html
* outline extractor reference: https://ast-grep.github.io/reference/outline-rules
* introducing outline: https://ast-grep.github.io/blog/introducing-outline.html

## Testing / diagnostics / integration

* rule testing: https://ast-grep.github.io/guide/test-rule.html
* severity/reporting: https://ast-grep.github.io/guide/project/severity.html
* suppression: https://ast-grep.github.io/guide/project/suppress-rule.html
* JSON output: https://ast-grep.github.io/guide/tools/json.html
* editor integration: https://ast-grep.github.io/guide/tools/editors.html
* FAQ: https://ast-grep.github.io/advanced/faq.html
* AI tools guide: https://ast-grep.github.io/advanced/prompting.html
* LLM full documentation bundle: https://ast-grep.github.io/llms-full.txt
* Playground: https://ast-grep.github.io/playground.html

## Programmatic APIs

* API overview: https://ast-grep.github.io/reference/api.html
* JavaScript/N-API guide: https://ast-grep.github.io/guide/api-usage/js-api.html
* Python API guide: https://ast-grep.github.io/guide/api-usage/py-api.html
* Rust crate docs: https://docs.rs/ast-grep-core/

---

# Closing operational summary

For modern ast-grep 0.45.1, the most important conceptual update over older references is that ast-grep now spans **three related but distinct roles**:

```text
1. structural search / lint / rewrite
2. low-token local code outlining for humans and coding agents
3. embeddable syntax-query APIs for Rust, JavaScript, Python, and WASM contexts
```

The strongest architecture uses those capabilities without overstating them:

```text
ast-grep owns syntax structure and syntax-aware edits;
semantic tools own resolved program meaning;
versioned tests own confidence that a rule still means what its author intended.
```

That boundary is what makes ast-grep especially useful inside an advanced coding-agent toolchain: it can cheaply and deterministically answer a large class of structural questions, expose exactly targeted source regions, and execute precise codemods—while leaving type resolution, whole-program graph reasoning, and behavioral validation to tools designed for those layers.
