# ripgrep 15.2.0 + PCRE2 10.47 — advanced technical reference

## Version / source anchors

This document is pinned to **ripgrep 15.2.0**, released **2026-07-15**. It treats the `rg` 15.2.0 source tree, release notes, user guide, FAQ, and flag definitions as the authoritative ripgrep sources. The upstream 15.2.0 release artifacts may bundle an older PCRE2, while package-manager and locally built copies can dynamically link the system PCRE2. This edition targets an environment in which `rg --pcre2-version` confirms **PCRE2 10.47**. Always verify the actual binary with `rg --version` and `rg --pcre2-version` before depending on version-gated PCRE2 behavior. [RG-RELEASE] [RG-README] [RG-FLAGS]

The PCRE2 deep dive is pinned to **PCRE2 10.47** (2025-10-21), the latest stable PCRE2 release at the time of this reference. It includes the complete 10.45 pattern feature expansion (Unicode 16 data, scan-substring assertions, Perl-style extended character classes, enhanced variable-length lookbehind behavior and related syntax), the 10.46 security fix for CVE-2025-58050, and the 10.47 additions: recursion/subroutine calls that return selected capture groups, AArch64 JIT SIMD improvements, plus several library/build APIs that are intentionally distinguished from what stock `rg -P` exposes. [PCRE2-NEWS] [PCRE2-PATTERN] [PCRE2-CHANGELOG]

> **Security boundary — PCRE2 10.47.** PCRE2 10.47 contains the 10.46 fix for CVE-2025-58050, so the 10.45 `(*ACCEPT)` + scan-substring read-past-end issue is no longer a reason to reject this version. PCRE2 remains a backtracking engine, however: attacker-controlled patterns can still cause excessive CPU/memory use or exploit future engine defects. Treat arbitrary PCRE2 patterns as executable resource-consuming input; prefer ripgrep's default engine for untrusted regexes and retain process/resource limits when exposing `rg -P` through a service. [PCRE2-1046] [PCRE2-NEWS]

---

## Feature inventory

ripgrep 15.2.0 is simultaneously:

- a recursive repository-aware file enumerator;
- a line-oriented text search engine;
- a multiline search engine when `-U/--multiline` is enabled;
- a front end to the Rust regex engine by default;
- an optional PCRE2 front end via `-P/--pcre2` / `--engine=pcre2`;
- a machine-readable search producer via `--json`;
- a file-listing/filtering tool via `--files`, globs, types and ignore rules;
- a text-encoding transcoder for search via `-E/--encoding`;
- a compressed-file/preprocessor orchestration layer;
- a non-mutating output-rewrite tool via `-r/--replace`;
- a highly scriptable CLI with deterministic exit codes, config files and detailed debug/trace output.

The 15.2 release itself is focused rather than disruptive: it improves directory traversal on very large corpora, adds `aarch64-unknown-linux-musl` release binaries, honors `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM`, and fixes several ignore-matching edge cases across multiple directories. [RG-CHANGELOG]

---

# Comprehensive documentation map

## 0) Scope, release stance, and the ripgrep mental model
## 1) Installation, release binaries, PCRE2 verification, and source builds
## 2) Execution pipeline and performance model
## 3) CLI topology, argument parsing, precedence, and configuration
## 4) Pattern sources: positional, `-e`, `-f`, stdin, and multi-pattern OR semantics
## 5) Regex engine selection: default vs PCRE2 vs auto
## 6) Literal search, case handling, Unicode, and fixed-string mode
## 7) Multiline search, dotall, anchors, CRLF, and NUL-delimited data
## 8) Whole-word / whole-line wrappers and match semantics
## 9) Standard output modes, context, headings, color, and only-matching
## 10) Counts, file-list modes, quiet mode, and exit status
## 11) Replacements and capture interpolation
## 12) Recursive traversal, roots, depth, symlinks, and filesystem boundaries
## 13) Automatic filtering and the ignore stack
## 14) ripgrep 15.2 ignore/traversal changes
## 15) Manual filtering: globs, file types, precedence, and type customization
## 16) Hidden, binary, text, max-filesize, and terminal-safety behavior
## 17) Encodings, transcoding, BOMs, raw-byte mode, and byte offsets
## 18) Compressed files and preprocessors
## 19) I/O strategy, mmap, threads, buffering, sorting, and performance cliffs
## 20) JSON output as an agent/programmatic query interface
## 21) Configuration files, environment, aliases, and override design
## 22) Debugging search space and execution with `--files`, `--debug`, `--trace`
## 23) Shell scripting, robust filename transport, and deterministic automation
## 24) PCRE2 integration architecture inside ripgrep 15.2
## 25) PCRE2 10.47 syntax and feature map
## 26) Lookahead, lookbehind, and assertion design
## 27) Variable-length lookbehind in PCRE2 10.47
## 28) Backreferences, named groups, duplicate names, and capture semantics
## 29) Atomic groups, possessive quantifiers, branch reset, and conditionals
## 30) Subroutines, recursion, recursion tests, and recursive patterns
## 31) Backtracking control verbs, `\K`, `\G`, `\R`, and advanced control
## 32) Unicode/UCP, properties, script runs, case folding, and ASCII restrictions
## 33) PCRE2 10.47 extended character classes and set algebra
## 34) PCRE2 10.47 scan-substring assertions
## 35) Pattern-level resource limits and safety hardening
## 36) JIT behavior, unsupported JIT features, and performance design
## 37) PCRE2 newline, BSR, multiline, dotall, CRLF, and ripgrep interactions
## 38) PCRE2 replacement API vs ripgrep `--replace`: what does *not* carry through
## 39) PCRE2 library capabilities not exposed by ripgrep
## 40) PCRE2 10.47 security posture and untrusted-pattern policy
## 41) High-value code-search recipes for programming agents
## 42) Evidence-oriented search patterns for LLM coding agents
## 43) Performance engineering and anti-pattern inventory
## 44) Upgrade guide: old ripgrep references → 15.2.0
## 45) CLI flag-family lookup matrix
## 46) PCRE2 10.47 pattern-feature lookup matrix
## 47) Engine-selection and compatibility matrix
## 48) Production / agent checklists
## 49) Source index

---

# ripgrep Advanced — 0) Scope, release stance, and mental model

## 0.0 Identity

`rg` is a **line-oriented recursive search program with repository-aware traversal**. By default it searches the current directory recursively, respects `.gitignore`/`.ignore`/`.rgignore`, skips hidden paths, avoids binary content, and uses a fast Rust regex engine with full Unicode support. PCRE2 is an optional alternative matcher for constructs that cannot be expressed by the default finite-automata engine. [RG-README]

The most useful decomposition is:

```text
CLI / config / environment
        ↓
pattern set + engine selection
        ↓
PATH / stdin source selection
        ↓
directory walker + ignore/glob/type/hidden/binary filters
        ↓
reader / transcoder / decompressor / preprocessor
        ↓
line-oriented or multiline matcher
        ↓
standard printer / summary mode / JSON printer
        ↓
stdout + stderr + exit code
```

Agent rule: **do not conflate “regex engine” with “file discovery.”** `-P` changes pattern compilation/matching, but it does not bypass ignore rules, hidden-file filtering, type filters, encoding conversion, output formatting or traversal policy.

## 0.1 Stable target and build-dependent features

Use these probes before generating engine-specific commands:

```bash
rg --version
rg --pcre2-version
```

`--pcre2-version` prints the PCRE2 version in use and exits; if PCRE2 support was not compiled in, it prints an error and exits non-zero. [RG-FLAGS]

Never infer the PCRE2 version from the ripgrep version alone. Official release binaries and distro packages can be compiled differently, and a source build may link a system PCRE2 rather than the bundled/static one.

## 0.2 The “fast default, fancy opt-in” principle

Default engine:

```bash
rg 'foo\w+bar' src/
```

PCRE2 only when the pattern needs it:

```bash
rg -P '(?<=class\s)Foo\b' src/
```

Auto selection when an interface wants to accept both ordinary and advanced regexes:

```bash
rg --engine=auto '(?<=class\s)Foo\b' src/
```

The default engine is preferable whenever practical because it is designed around finite automata and strong worst-case search guarantees. PCRE2 is a backtracking engine with a much richer language and correspondingly more ways to create pathological patterns. [RG-FAQ]

---

# 1) Installation, release binaries, PCRE2 verification, and source builds

## 1.0 Verify before relying on features

```bash
rg --version
rg --pcre2-version
```

Reference Linux 15.2 release output includes:

```text
ripgrep 15.2.0 (rev e89fff8)
features:+pcre2
...
PCRE2 10.47 is available (JIT is available)
```

Treat that as a **reference release-build profile**, not a universal guarantee for every package-manager binary.

## 1.1 Source build with PCRE2

The 15.2 source tree exposes the Cargo feature `pcre2`. A canonical build is:

```bash
git clone https://github.com/BurntSushi/ripgrep
cd ripgrep
git checkout 15.2.0
cargo build --release --features pcre2
./target/release/rg --version
./target/release/rg --pcre2-version
```

To force static PCRE2 linkage when using the `pcre2-sys` build path, the project documentation describes `PCRE2_SYS_STATIC=1`; system PCRE2 can also be discovered through `pkg-config`. [RG-README]

## 1.2 Deployment rule

For reproducible agent infrastructure, record at least:

```text
rg version
rg revision
features:+/-pcre2
PCRE2 version
JIT available/unavailable
OS / architecture
```

A command-generation agent should probe those once at environment initialization and cache the capability profile rather than discovering missing PCRE2 only after an advanced query fails.

---

# 2) Execution pipeline and performance model

## 2.0 Search cost has three independent dimensions

1. **Discovery cost** — directory walking, ignore evaluation, metadata, file type/glob tests.
2. **Read/decode cost** — file I/O, mmap/buffering, transcoding, decompression, preprocessors.
3. **Matching/printing cost** — regex engine, match density, line-number calculation, context, JSON/terminal output.

A “slow regex” is not always the problem. On huge repositories, traversal can dominate. On highly matching corpora, printing can dominate. On compressed/preprocessed input, subprocess overhead can dominate.

## 2.1 15.2 traversal improvement

15.2 includes a dedicated traversal performance improvement for very large corpora. If upgrading from an older ripgrep release specifically for codebase indexing/search workflows, this is one of the highest-value 15.2 changes even when regex behavior is unchanged. [RG-CHANGELOG]

## 2.2 Literal extraction matters

Both ripgrep’s default engine and PCRE2 can benefit dramatically when a pattern contains useful literals. Compare conceptually:

```regex
ERROR: .* timeout
```

with a pattern like:

```regex
[A-Za-z]{30}
```

The former exposes literal anchors that can let the engine skip large portions of input; the latter can force far more candidate examination. This is why apparently “more specific” regexes can be much faster than generic classes.

---

# 3) CLI topology, argument parsing, precedence, and configuration

## 3.0 Canonical invocation forms

```bash
rg [OPTIONS] PATTERN [PATH...]
rg [OPTIONS] -e PATTERN... [PATH...]
rg [OPTIONS] -f PATTERNFILE... [PATH...]
rg [OPTIONS] --files [PATH...]
rg [OPTIONS] --type-list
command | rg [OPTIONS] PATTERN
rg --help
rg --version
```

## 3.1 Last-flag-wins as a design principle

Many toggles have negated forms. Where flags conflict, ripgrep intentionally supports override-friendly command composition:

```bash
rg --ignore-case --case-sensitive 'Foo' .
rg --pcre2 --engine=default 'foo' .
rg --mmap --no-mmap 'foo' .
```

This is what makes a global config safe when the CLI can override it later.

## 3.2 `--` as an option parser barrier

```bash
rg -- '-leading-dash-pattern' .
```

Prefer `-e` when programmatically generating patterns, because it makes the pattern/path distinction explicit:

```bash
rg -e '-leading-dash-pattern' .
```

## 3.3 Explicit PATH avoids stdin auto-detection surprises

If stdin exists unexpectedly in an execution environment, force filesystem search by specifying the root:

```bash
rg 'needle' ./
```

For agent subprocesses, always pass an explicit root when the intent is repository search.

---

# 4) Pattern sources and multi-pattern semantics

## 4.0 Positional pattern

```bash
rg 'TODO|FIXME' src/
```

## 4.1 Repeatable `-e/--regexp`

```bash
rg -e 'TODO' -e 'FIXME' -e 'HACK' src/
```

All patterns are ORed: a line/match qualifies when any supplied pattern matches.

Once `-e` or `-f` is used, positional arguments are paths rather than a positional pattern.

## 4.2 Pattern files

```bash
rg -f patterns.txt src/
rg -f patterns-a.txt -f patterns-b.txt src/
```

One line is one pattern. **An empty line is an empty regex and therefore matches everywhere.** Treat blank lines as a validation error in programmatically generated pattern files unless “match all” is intentional.

## 4.3 Patterns from stdin

```bash
printf '%s\n' 'TODO' 'FIXME' | rg -f - src/
```

This consumes stdin as a **pattern source**. It cannot simultaneously use that same stdin stream as search data.

## 4.4 Engine globality

The chosen engine applies to the entire pattern set:

```bash
rg --engine=pcre2 -e '(?<=foo)bar' -e '(baz)\1' src/
```

You cannot compile one `-e` with the default engine and another with PCRE2 in the same invocation.

---

# 5) Regex engine selection: default vs PCRE2 vs auto

## 5.0 Exact choices

```bash
rg --engine=default PATTERN [PATH...]
rg --engine=pcre2   PATTERN [PATH...]
rg --engine=auto    PATTERN [PATH...]
rg -P PATTERN [PATH...]
```

`-P/--pcre2` is shorthand for choosing PCRE2. `--engine` and `-P` override one another according to argument order. [RG-FLAGS]

## 5.1 Default engine

Use for:

- ordinary literals and classes;
- alternation;
- captures used by ripgrep replacement;
- bounded/unbounded repetition;
- Unicode classes/properties;
- most source-code token searches;
- cases where predictable linear-style behavior is more important than fancy assertions.

Does **not** support arbitrary lookaround or backreferences.

## 5.2 PCRE2

Use when you need:

- positive/negative lookahead;
- positive/negative lookbehind;
- backreferences;
- recursion/subroutines;
- conditionals;
- atomic groups and richer backtracking control;
- PCRE2 verbs such as `(*SKIP)(*F)`;
- variable-length lookbehind supported in modern PCRE2;
- 10.45+ scan-substring assertions or extended classes, or 10.47 returned-capture recursion.

## 5.3 Auto

```bash
rg --engine=auto 'ordinary-regex' .
rg --engine=auto '(?<=foo)bar' .
```

Auto prefers the default engine and can fall back to PCRE2 when required. It is convenient for interactive use but less ideal when exact performance/semantics must be auditable, because the engine choice becomes implicit.

Agent rule: **generate an explicit engine when reproducibility matters.**

## 5.4 PCRE2 introspection limitation

The 15.2 flag documentation warns that PCRE2 has fewer introspection APIs available to ripgrep. One practical consequence: a PCRE2 regex containing `\n` without ripgrep multiline mode can silently fail to match instead of producing the helpful default-engine error. If newline participation is intended, always add `-U`. [RG-FLAGS]

---

# 6) Literal search, case handling, Unicode, and fixed strings

## 6.0 Fixed strings

```bash
rg -F 'literal.with[regex](punctuation)' .
```

`-F/--fixed-strings` applies to the whole pattern set. Use it whenever the request is semantically literal; it avoids regex parsing and is often faster/safer for agent-generated exact evidence searches.

## 6.1 Case modes

```bash
rg -s 'Foo' .          # case-sensitive
rg -i 'Foo' .          # case-insensitive
rg -S 'foo' .          # smart case
```

Smart case means lowercase-only patterns are insensitive while a pattern containing uppercase becomes sensitive.

## 6.2 Unicode

Unicode mode is on by default. For ASCII/code-token semantics:

```bash
rg --no-unicode '\bfoo\b' src/
```

With PCRE2, ripgrep’s `--unicode`/`--no-unicode` controls the UTF/UCP behavior wired into the matcher. Historical `--pcre2-unicode` flags are deprecated aliases; new automation should use the engine-independent Unicode flags. [RG-CHANGELOG]

## 6.3 Why `--no-unicode` can be useful in code search

For ASCII identifier searches, it makes `\w`, `\b` and related classes line up more closely with many programming-language identifier subsets and can reduce Unicode-property overhead. Do not apply it blindly when source identifiers or target text may legitimately contain non-ASCII characters.

---

# 7) Multiline search, dotall, anchors, CRLF, and NUL data

## 7.0 Three separate concepts

```text
-U / --multiline      ripgrep allows matches to span line terminators
(?m)                  regex ^ and $ become line-aware
(?s) / --multiline-dotall
                      dot may match line terminators
```

Do not conflate them.

## 7.1 Multiline

```bash
rg -U 'foo\nbar' file.txt
rg -U '(?s)BEGIN.*?END' .
```

When a multiline pattern can actually consume `\n`, ripgrep may need contiguous file data in memory. For stdin or other non-mmap sources this can require reading to EOF before matching begins. [RG-FLAGS]

## 7.2 Dotall

```bash
rg -U --multiline-dotall 'foo.*?bar' .
rg -U '(?s:foo.*?bar)' .
```

`--multiline-dotall` has no practical effect without multiline search.

## 7.3 Line anchors inside a multiline haystack

```bash
rg -U '(?m)^class\s+\w+' src/
rg -U '(?sm)^BEGIN\b.*?^END\b' .
```

## 7.4 CRLF

```bash
rg --crlf '^foo$' .
```

Use when line-anchor behavior must treat CRLF as one terminator.

## 7.5 NUL-delimited input

`--null-data` changes the line terminator to NUL and implies text-oriented searching. Treat it as a specialized binary/record-stream mode. Do not combine it casually with `--crlf`; conflict semantics are override/ordering based.

---

# 8) Whole-word / whole-line wrappers

## 8.0 Whole word

```bash
rg -w 'yield' src/
```

This is a matcher wrapper, not a file-selection feature. In PCRE2 mode ripgrep internally implements word wrapping with lookaround around `\w`, which means Unicode/UCP configuration affects what counts as a word character. [RG-PCRE2-MATCHER]

## 8.1 Whole line

```bash
rg -x 'pass' src/
```

`-x/--line-regexp` is equivalent in intent to requiring the entire line to match.

## 8.2 Avoid redundant anchors

Prefer:

```bash
rg -x '\s*pass\s*' src/
```

instead of layering `-x` around `^...$` unless the anchor semantics are intentionally part of the expression.

---

# 9) Standard output modes, context, headings, color, and only-matching

## 9.0 Line/column/file metadata

```bash
rg -n 'needle' .
rg -n --column 'needle' .
rg --with-filename 'needle' file.txt
```

## 9.1 Context

```bash
rg -B 3 'ERROR' app.log
rg -A 5 'ERROR' app.log
rg -C 3 'ERROR' app.log
```

## 9.2 Only matching text

```bash
rg -o '\b[A-F0-9]{40}\b' .
```

This is especially useful as a structured-but-non-JSON extraction primitive.

## 9.3 Pretty mode

```bash
rg -p 'needle' .
```

Use human formatting interactively; avoid it for parsers.

## 9.4 Color

For subprocess parsers, force deterministic non-color output:

```bash
rg --color=never 'needle' .
```

For terminal UI, `--color=always` may be desirable when piping into a pager that supports ANSI sequences.

## 9.5 Separators: context, field, and path

Four flags control the delimiters ripgrep prints. All accept escape sequences such as `\t` or `\x7F`. [RG-FLAGS]

| Flag | Delimits | Default |
|---|---|---|
| `--context-separator=SEP` | non-contiguous context blocks | `--` |
| `--no-context-separator` | removes the block separator entirely | — |
| `--field-context-separator=SEP` | path/line/column/text **within a context line** | `-` |
| `--field-match-separator=SEP` | path/line/column/text **within a matching line** | `:` |

`--context-separator` is only consulted when `-A`/`-B`/`-C` is in play:

```bash
rg -C1 -n --context-separator='###' 'needle' file
rg -C1 -n --no-context-separator      'needle' file
```

Two behaviors that surprise scripts:

* setting `--context-separator=''` still emits a **blank line**; only `--no-context-separator` removes the separator line completely;
* `--field-context-separator` and `--field-match-separator` are what make context lines print `path-12-text` while matching lines print `path:12:text`. A parser that splits on `:` and assumes every line is a match will misread every context line.

Reshaping separators is a **human-output** tool. For machine consumption use `--json` (section 20) or `-0/--null` plus `--path-separator` (section 45.9); do not build a parser on top of retuned delimiters, because the field set itself is not part of any stability contract.

---

# 10) Counts, file-list modes, quiet mode, and exit status

## 10.0 Counts

```bash
rg -c 'TODO' .             # matching lines per file
rg --count-matches 'TODO' .
```

With multiline matches, understand whether you need line counts or match counts; they are not equivalent.

## 10.1 File-list search modes

```bash
rg -l 'needle' .                    # files with matches
rg --files-without-match 'needle' .
rg --files .                        # candidate inventory, no content search
```

## 10.2 Quiet mode

```bash
if rg -q 'needle' path; then
  echo found
fi
```

Use `-q` when only existence matters. It avoids output work and can terminate earlier.

## 10.3 Exit statuses

Canonical scripting contract:

```text
0  at least one match (or successful special mode where applicable)
1  no matches
2  error
```

Do not treat exit 1 as a process failure in automation; it is a normal “not found” result.

## 10.4 Match caps: `-m/--max-count`

```bash
rg -m 2 -n 'needle' .
```

`-m NUM` limits the number of matching **lines per file searched**, not across the whole run. [RG-FLAGS]

Counting rules:

* under `-U/--multiline`, one match spanning several lines counts **once**;
* several matches on a single line count **once**, as in non-multiline mode;
* `0` is legal and causes ripgrep to search nothing — it then reports no matches and exits `1`.

Three interactions matter more than the flag itself:

* **`-c/--count` reports the capped number, not the true one.** `rg -c` on a file with three matching lines prints `3`; `rg -m 2 -c` on the same file prints `2`. A cap silently converts a census into a floor, so never combine `-m` with `-c` when the count is the evidence.
* **`-A`/`-C` can print more matches than the cap.** When a trailing context line itself contains a match, it is printed as a match line, so the visible match count can exceed `NUM`.
* **The cap does not change the exit status.** A run that hits the cap still exits `0`; `-l/--files-with-matches` is likewise unaffected, because file membership is decided by the first match.

Use `-m 1` as a cheap existence probe when `-l` or `-q` will not do — for example when you want the first matching line of each file — and keep it away from any counting or completeness claim (section 42.8).

---

# 11) Replacements and capture interpolation

## 11.0 Replacement is output-only

```bash
rg 'fast\s+(\w+)' README.md -r 'fast-$1'
```

Ripgrep **never modifies the searched file**. `--replace` rewrites the matching portion in printed output. [RG-GUIDE]

## 11.1 Named groups

```bash
rg '(?P<lhs>\w+)=(?P<rhs>\w+)' -r '$rhs=$lhs' .
```

## 11.2 PCRE2 pattern + ripgrep replacement

```bash
rg -P '(?<q>["\x27])(?<body>.*?)\k<q>' \
  -r '${body}' source.txt
```

The **pattern** is compiled by PCRE2, but the replacement string is interpreted by ripgrep’s printer. This distinction becomes crucial for PCRE2’s evolving `pcre2_substitute()` syntax—including 10.47 `$+`; see §38.

## 11.3 Whole-line output replacement

Match the whole line or combine with `-o` depending on desired output:

```bash
rg '^.*secret=.*$' -r 'REDACTED' .
rg -o 'secret=\S+' -r 'secret=REDACTED' .
```

---

# 12) Recursive traversal, roots, depth, symlinks, and filesystem boundaries

## 12.0 Roots

```bash
rg 'needle' .
rg 'needle' src tests docs
rg 'needle' path/to/file.py
```

## 12.1 Depth

```bash
rg --max-depth 1 'needle' src/
```

## 12.2 Follow symlinks

```bash
rg -L 'needle' .
```

Symlink loops and broken targets can produce errors. `--no-messages` suppresses selected read/open messages, but suppression does not turn an actual I/O error into success.

## 12.3 Stay on one filesystem

```bash
rg --one-file-system 'needle' /large/root
```

Useful for mounted monorepos, container host mounts and system scans.

## 12.4 Explicit paths are intentional overrides

If a known file is ignored, passing that exact file path is the strongest “search this file” request:

```bash
rg 'needle' ignored/generated/file.rs
```

For directory roots, ignore-rule relativity still matters; do not assume “explicit directory” equals “explicit every descendant file.”

---

# 13) Automatic filtering and the ignore stack

## 13.0 Default behavior

Recursive search normally skips:

- paths excluded by ignore rules;
- hidden files/directories;
- binary data via NUL-byte heuristics;
- symlink traversal.

## 13.1 Ignore sources and precedence

Conceptual precedence from lower to higher:

```text
--ignore-file
Git global excludes
.git/info/exclude
.gitignore
.ignore
.rgignore
manual -g/--glob overrides (separate, higher-priority path override layer)
```

Within directory-tree ignore sources, more specific nested ignore files can override ancestors according to gitignore semantics.

## 13.2 Unrestricted ladder

```bash
rg -u   'needle' .   # no ignore rules
rg -uu  'needle' .   # + hidden
rg -uuu 'needle' .   # + binary search allowance
```

This is the fastest diagnostic ladder when content appears to be missing.

## 13.3 Surgical ignore toggles

Use fine-grained `--no-ignore-*` switches when debugging VCS vs `.ignore` vs global ignore behavior instead of immediately discarding every filtering layer.

---

# 14) ripgrep 15.2 ignore/traversal changes

15.2.0 specifically changes the traversal/filtering layer in ways that matter to repository automation. [RG-CHANGELOG]

## 14.1 Git config environment variables

ripgrep now respects:

```text
GIT_CONFIG_GLOBAL
GIT_CONFIG_SYSTEM
```

This matters when the Git global/system ignore configuration used by a build agent is deliberately redirected.

## 14.2 Multi-directory ignore matching fixes

15.2 fixes several gitignore matching bugs when searching across multiple directories. If an older reference includes workarounds for surprising ignore behavior across multiple explicit roots, re-test them against 15.2 before preserving the workaround.

## 14.3 `.jj` optimization under `--no-ignore`

15.2 no longer checks for `.jj` existence when ignore processing is disabled. This is primarily an implementation/performance correctness detail, but it reinforces the rule that `--no-ignore` should cut unnecessary repository metadata work.

## 14.4 Huge-corpus traversal

The release includes an explicit traversal-time optimization for very large corpora. For agent systems that call `rg` repeatedly, nevertheless prefer **narrow search roots** and **specific globs/types**: a faster full-tree walk is still more expensive than avoiding the walk.

---

# 15) Manual filtering: globs, file types, precedence, customization

## 15.0 Glob include/exclude

```bash
rg -g '*.py' 'needle' .
rg -g '*.py' -g '!**/.venv/**' 'needle' .
```

Later matching globs win.

## 15.1 Correct directory glob

```bash
rg -g 'src/**' 'needle' .
```

Not:

```bash
rg -g 'src' 'needle' .
```

## 15.2 Case-insensitive path globs

```bash
rg --iglob '*.PY' 'needle' .
```

Content case flags (`-i`) and glob case flags are independent.

## 15.3 File types

```bash
rg -tpy 'needle' .
rg -Tjson 'needle' .
rg --type-list
```

## 15.4 Add/redefine a type

```bash
rg --type-add 'proto:*.proto' -tproto 'service' .
rg --type-clear py --type-add 'py:*.py' -tpy 'needle' .
```

Persist type additions through the ripgrep config rather than assuming `--type-add` mutates a global registry.

## 15.5 Agent search-space policy

A good programming-agent search normally constrains **both semantic target and search space**:

```bash
rg -n -tpy -g '!**/generated/**' 'class\s+Foo\b' src tests
```

This is usually faster and returns higher-signal evidence than searching the whole repository then post-filtering paths.

---

# 16) Hidden, binary, text, max-filesize, and terminal safety

## 16.0 Hidden paths

```bash
rg --hidden 'needle' .
```

Explicitly exclude `.git` when hidden content is wanted but repository object data is not:

```bash
rg --hidden -g '!.git/**' 'needle' .
```

## 16.1 Binary mode vs text mode

`--binary` permits searching binary files while preserving binary-aware output behavior. `-a/--text` treats binary input as ordinary text and can emit control bytes to the terminal.

For forensics/programmatic use, prefer `--json` or redirect stdout to a file rather than printing arbitrary bytes to an interactive terminal.

## 16.2 Max filesize

Use `--max-filesize` to bound accidental searches of giant generated/artifact files when repository layout is not tightly controlled.

---

# 17) Encodings, transcoding, BOMs, raw bytes, and offsets

## 17.0 Default `auto`

By default ripgrep uses best-effort BOM sniffing for UTF-8/UTF-16, not general encoding detection. UTF-16 detected by BOM is transcoded to UTF-8 before matching. [RG-GUIDE]

## 17.1 Explicit encoding

```bash
rg -E windows-1252 'café' docs/
rg -E shift_jis '検索' legacy/
```

## 17.2 Raw bytes

```bash
rg -E none -a '(?-u:\x00\xFF)' suspect.bin
```

`-E none` disables encoding conversion, including BOM handling. Pair with byte-oriented matching and `-a` when binary heuristics would otherwise interfere.

## 17.3 Offset implication

If input is transcoded, offsets reported by machine output refer to the **searched/transcoded data**, not necessarily the byte positions in the original on-disk encoding. Use raw-byte mode when exact file-byte offsets are required for patching or forensic correlation.

---

# 18) Compressed files and preprocessors

## 18.0 Compressed search

```bash
rg -z 'needle' logs/
```

ripgrep shells out to format-specific decompression tools for supported formats. It does not recursively treat arbitrary archive containers as searchable directory trees.

## 18.1 Arbitrary preprocessor

```bash
rg --pre ./pre-pdftotext --pre-glob '*.pdf' 'needle' docs/
```

`--pre-glob` limits subprocess overhead by sending only matching files through the preprocessor. [RG-FLAGS]

## 18.2 Security rule

A preprocessor is executable code. Never accept an arbitrary `--pre` command from an untrusted search request. For agent systems, preprocessor selection should come from a fixed allowlist.

---

# 19) I/O strategy, mmap, threads, buffering, sorting, and performance cliffs

## 19.0 mmap

```bash
rg --mmap 'needle' huge-static-file
rg --no-mmap 'needle' /var/log
```

Disable mmap for files that may be truncated/replaced during the search, such as rotating logs. ripgrep normally chooses I/O strategy heuristically.

## 19.1 Threads

Directory search is parallelized. Do not reflexively maximize thread count for every workload; compressed/preprocessed searches, network filesystems and heavily contended disks can become I/O- or subprocess-bound.

```bash
rg -j 1 'needle' .          # single-threaded: deterministic output order
rg --threads 8 'needle' .   # explicit worker count
```

`-j NUM` / `--threads NUM` sets the **approximate** worker count. `0` is the default and lets ripgrep choose heuristically. [RG-FLAGS]

Reach for an explicit value when:

* a CI runner enforces a CPU quota and the default over-subscribes it;
* another job is competing for the same cores;
* you are benchmarking and need the variable pinned;
* output order must be reproducible — `-j 1` is the cheapest way to get it, and any non-`none` value of `--sort` (section 19.2) forces single-threaded traversal anyway.

Searching a single file, or stdin, is single-threaded regardless of this flag.

## 19.2 Sorting

Sorting output can force coordination/buffering and reduce the natural benefit of parallel traversal. Request deterministic sorting only when the consumer requires it.

```bash
rg --sort path  'needle' .   # ascending
rg --sortr path 'needle' .   # descending
```

Both flags take the same `SORTBY` vocabulary: [RG-FLAGS]

| Value | Meaning | Threading |
|---|---|---|
| `none` | default; emit results as they are found | **multi-threaded** |
| `path` | file path | single-threaded |
| `modified` | last modified time | single-threaded |
| `accessed` | last accessed time | single-threaded |
| `created` | creation time | single-threaded |

Operational notes:

* **Every value except `none` forces single-threaded traversal.** Sorting is a throughput decision, not a formatting one.
* `path` order is produced by sorting directory entries **during traversal**, not by sorting the final path strings. Given `a/b` and `a+`, `a+` sorts after `a/b` under `--sort path` even though `+` precedes `/` in a plain lexicographic sort.
* If the requested criterion is unavailable on the filesystem — `created` on ext4, for instance — ripgrep detects it, prints an error and **exits without searching**. It is available on APFS, so the same command is not portable between a developer's macOS checkout and a Linux runner.
* **Precedence is last-flag-wins, like every other ripgrep flag (section 3.1).** `--help` says of `--sort` that "this flag overrides `--sortr`", which holds only when `--sort` appears later on the command line; `--sort path --sortr path` sorts descending. Do not rely on the stronger reading, especially when a config file (section 21) supplies one of the two.

For deterministic automation you usually want `--sort path` **and** an explicit engine and color policy — see the query determinism template in section 42.5.

## 19.3 Line buffering

For live pipelines:

```bash
tail -f app.log | rg --line-buffered 'ERROR'
```

Do **not** combine a never-ending stream with a multiline pattern that must read to EOF before searching.

## 19.4 Match density

Very high match density often makes printing the bottleneck. Use `-q`, `-l`, counts, `-o`, or a more selective pattern when the consumer does not need every matching line.

---

# 20) JSON output as an agent/programmatic query interface

## 20.0 Enable JSON

```bash
rg --json 'needle' src/
```

JSON mode emits typed messages such as begin/match/context/end/summary and implicitly enables stats. Several ordinary display flags are ignored or incompatible because JSON owns the output schema. [RG-FLAGS]

## 20.1 Arbitrary bytes

Text-like payload fields use a discriminated representation:

```json
{"text":"valid UTF-8"}
```

or:

```json
{"bytes":"BASE64..."}
```

A robust parser must support both.

## 20.2 Match offsets

Machine output supplies byte offsets for the match/submatches relative to the searched line/data. This is a much stronger evidence interface than parsing colored human output.

## 20.3 Recommended agent contract

Prefer:

```bash
rg --json --color=never ...
```

and parse events structurally. Do not scrape default terminal formatting to recover path/line/submatch data.

## 20.4 JSON vs `-l`

If you only need file names, `-l` is cheaper and simpler than JSON. Use JSON when you need exact match locations/content/context and stats.

---

# 21) Configuration files, environment, aliases, and override design

## 21.0 Config path

ripgrep configuration is opt-in via its config environment path. A config is essentially a sequence of CLI arguments, one per line.

## 21.1 Good global defaults

Reasonable examples:

```text
--smart-case
--hidden
--glob=!.git/**
```

Avoid global defaults that silently change the regex language, such as forcing PCRE2 everywhere, unless every consumer understands the change.

## 21.2 Override-friendly design

If config enables a feature, preserve the ability to turn it off per invocation:

```bash
rg --no-hidden 'needle' .
rg --engine=default 'needle' .
rg --no-ignore 'needle' .
```

## 21.3 Agent deployment

For deterministic automation, either:

- explicitly supply all behavior-affecting flags and use a controlled config, or
- launch with an environment that does not point at an unknown user config.

Hidden user config is a classic cause of “works in terminal, differs in agent subprocess” behavior.

---

# 22) Debugging search space and execution

## 22.0 Candidate set

```bash
rg --files .
```

## 22.1 Debug why a path was skipped

```bash
rg --debug 'needle' .
```

## 22.2 Deep trace

```bash
rg --trace 'needle' .
```

Trace output can be enormous. Use it only after `--files` and `--debug` fail to explain behavior.

## 22.3 Type debugging

```bash
rg --type-list | rg '^py:'
```

## 22.4 Engine capability debugging

```bash
rg --version
rg --pcre2-version
```

This four-stage diagnostic sequence resolves most agent failures:

```text
1. Is the file in `rg --files`?
2. If not, why does `--debug` skip it?
3. Is the expected type/glob definition active?
4. Is the required regex engine actually compiled/selected?
```

---

# 23) Shell scripting, filename transport, and deterministic automation

## 23.0 Quote patterns

Use single quotes in POSIX shells so the shell does not interpret `$`, `*`, backslashes or parentheses:

```bash
rg -P '(?<q>["\x27]).*?\k<q>' .
```

## 23.1 NUL-delimit filenames

When passing file paths to downstream tools, use NUL delimiters where available rather than line-delimited parsing. Filenames may contain whitespace and newlines.

## 23.2 Distinguish no-match from error

```bash
rg -q 'needle' .
case $? in
  0) echo found ;;
  1) echo not-found ;;
  2) echo search-error >&2 ;;
esac
```

## 23.3 Timeouts belong to the parent process

ripgrep does not turn arbitrary PCRE2 into a safe service just because it is a CLI. For untrusted/expensive searches, the caller should enforce wall-clock timeout, CPU/memory limits and search-root restrictions in addition to regex-level limits.


---

# 24) PCRE2 integration architecture inside ripgrep 15.2

This section is the most important bridge between “PCRE2 documentation” and “what `rg -P` actually does.” PCRE2 exposes a large C API, but ripgrep uses a deliberately narrower matcher wrapper. [RG-PCRE2-MATCHER] [RG-HIARGS]

## 24.0 The exact 15.2 builder path

On a PCRE2 search, ripgrep 15.2 effectively constructs a `grep::pcre2::RegexMatcherBuilder` and configures it from CLI state. At source level, the important behavior is:

```text
builder.multi_line(true)
builder.fixed_strings(...)
case mode -> caseless / smart case
-w -> builder.word(true)
-x -> builder.whole_line(true)
64-bit -> jit_if_available(true)
          max_jit_stack_size(10 MiB)
Unicode on -> utf(true) + ucp(true)
-U -> dotall(multiline_dotall) when requested
--crlf -> crlf(true)
build_many(patterns)
```

[RG-HIARGS]

This yields several high-value invariants.

### Invariant 1 — PCRE2 JIT is not merely “available”; ripgrep attempts to use it on 64-bit

On 64-bit targets, ripgrep calls `jit_if_available(true)`. If JIT compilation is unavailable or unsupported for a particular pattern, the wrapper is designed to fall back rather than requiring JIT. Ripgrep also sets a **10 MiB maximum custom JIT stack**, much larger than PCRE2’s ordinary default 32 KiB stack. On 32-bit targets ripgrep avoids this JIT path because of observed compile-time memory failures. [RG-HIARGS] [RG-PCRE2-MATCHER]

### Invariant 2 — `--unicode` means both PCRE2 UTF and UCP

Unless `--no-unicode` is supplied, ripgrep enables:

```text
PCRE2 UTF semantics  -> codepoints rather than raw bytes
PCRE2 UCP semantics  -> \w, \d, \s, \b etc use Unicode properties
```

UCP implies UTF in ripgrep’s wrapper. [RG-PCRE2-MATCHER]

### Invariant 3 — ripgrep always enables PCRE2 multiline-anchor mode

Ripgrep sets PCRE2 multiline mode at matcher construction independently of `-U`. Therefore `^` and `$` are line-oriented by default in normal `rg -P` use, including searches where `-U` allows one match to cross line boundaries.

Use PCRE2’s absolute anchors when you mean the entire searched haystack:

```regex
\A   absolute start
\z   absolute end
\Z   end, or before final newline under PCRE2 semantics
```

This corrects a common conceptual mistake: **ripgrep `-U` controls whether the searcher permits matches across line terminators; it is not the switch that turns PCRE2 `^`/`$` into multiline anchors.** [RG-HIARGS]

### Invariant 4 — `--multiline-dotall` only changes dot behavior when `-U` is active

Ripgrep only wires dotall from the CLI when multiline searching is enabled. A pattern can still use its own inline `(?s:...)` syntax, but ripgrep’s global dotall convenience flag is tied to `-U`.

### Invariant 5 — default-engine size knobs are not PCRE2 resource knobs

`--regex-size-limit` and `--dfa-size-limit` are passed into the Rust-regex matcher builder. The 15.2 PCRE2 builder path does not consume them. Do **not** rely on those flags to bound PCRE2 backtracking or PCRE2 compiled-pattern memory. Use PCRE2 pattern-level resource verbs where applicable and, for hostile inputs, external process limits. [RG-HIARGS]

## 24.1 Multiple patterns become one PCRE2 alternation

The PCRE2 wrapper compiles the pattern set as one regex by wrapping each pattern in a non-capturing group and joining with `|`:

```text
-e P1 -e P2 -e P3
       ↓
(?:P1)|(?:P2)|(?:P3)
```

This is why one engine/configuration applies globally and why very large pattern sets can affect compile time and capture numbering. [RG-PCRE2-MATCHER]

Agent rule: if downstream replacement logic depends on numeric capture indices, avoid combining unrelated capture-heavy patterns into one invocation. Named captures are easier to reason about but duplicate-name policy must still be valid PCRE2 syntax.

## 24.2 `-F` under PCRE2

When fixed-string mode is active, ripgrep escapes each supplied pattern before compiling the combined PCRE2 expression. Therefore:

```bash
rg -P -F '(?<=foo)bar' .
```

searches for those literal characters; it does not activate lookbehind.

## 24.3 `-w` implementation under PCRE2

Ripgrep’s PCRE2 wrapper implements whole-word mode by surrounding the expression with lookaround based on `\w`. Consequently:

- Unicode mode changes the meaning of `\w` and therefore `-w`;
- `--no-unicode -w` is often preferable for ASCII programming-language token searches;
- a user pattern whose boundary expectations differ from `\w` should encode its own boundaries instead of using `-w`.

## 24.4 UTF validation behavior in current ripgrep PCRE2 wrapper

Historical PCRE2 integrations exposed ways to disable UTF checking. The current ripgrep PCRE2 wrapper documents that this is deprecated/no-op because modern PCRE2 supports `PCRE2_MATCH_INVALID_UTF`, which the wrapper always sets. Do not generate the old `--no-pcre2-unicode` performance folklore as if it were an independent “skip UTF validation” control. Use the current `--no-unicode` semantics and benchmark the actual 15.2 binary. [RG-PCRE2-MATCHER]

---

# 25) PCRE2 10.47 syntax and feature map

PCRE2 is much larger than the subset typically used for grep-like searching. The following syntax families are directly useful through `rg -P` because they are expressed in the **pattern itself**.

## 25.0 Core atoms and quantifiers

```regex
.                 any character except configured newline unless dotall
\d \D             digit / non-digit
\s \S             whitespace / non-whitespace
\w \W             word / non-word
\p{L} \P{L}       Unicode property / negation
[abc]              class
[^abc]             negated class
x* x+ x?           greedy quantifiers
x*? x+? x??        lazy quantifiers
x*+ x++ x?+        possessive quantifiers
x{m,n}             bounded repeat
x{m,n}?            lazy bounded repeat
x{m,n}+            possessive bounded repeat
```

With ripgrep Unicode enabled, UTF+UCP make shorthand classes and boundaries Unicode-aware.

## 25.1 Groups

```regex
(...)              numbered capture
(?:...)            non-capturing group
(?<name>...)       named capture
(?P<name>...)      Python-style named capture
(?>...)            atomic group
(?|...)            branch-reset group
```

## 25.2 Inline options

Common inline modifiers:

```regex
(?i)               caseless
(?m)               multiline anchors (already enabled by ripgrep)
(?s)               dotall
(?x)               extended/free-spacing
(?xx)              extended class spacing rules
(?i:...)           scoped option
(?-i:...)          scoped disable
```

## 25.3 Absolute/boundary assertions

```regex
^ $                 line anchors under ripgrep's PCRE2 configuration
\A                  absolute start of subject
\z                  absolute end of subject
\Z                  end or before final newline
\b \B               word boundary / not-boundary
\G                  first matching position / continuation-sensitive anchor
\K                  reset the reported start of the current match
```

## 25.4 Advanced families available through the pattern

- lookahead/lookbehind;
- backreferences;
- conditionals;
- subroutine calls and recursion;
- verbs: `(*SKIP)`, `(*PRUNE)`, `(*COMMIT)`, `(*THEN)`, `(*ACCEPT)`, `(*FAIL)`;
- newline and BSR start verbs;
- resource-limit start verbs;
- `(*NO_JIT)` and optimization-control start verbs;
- script-run assertions;
- non-atomic lookarounds;
- scan-substring assertions (introduced in 10.45; security-fixed in 10.46);
- Perl-style extended character classes `(?[...])` (introduced in 10.45);
- 10.47 recursion/subroutine calls with selected returned capture groups.

## 25.5 PCRE2 10.47 delta relative to 10.45/10.46

The table below is the fastest way to distinguish **new pattern power that `rg -P` can actually use** from **PCRE2 library/runtime improvements that are inherited but not directly configurable through ripgrep**.

| PCRE2 10.47 change | Usable directly in an `rg -P` pattern? | Effect in ripgrep |
| --- | --- | --- |
| recursion/subroutine calls with returned capture groups | **yes** | new pattern-language capability; see §30.2 |
| AArch64 JIT SIMD code generation | implicit, not syntax | may accelerate JIT-eligible PCRE2 searches on AArch64/Apple Silicon; no new `rg` flag |
| CVE-2025-58050 fix inherited from 10.46 | n/a | removes the specific 10.45 `(*ACCEPT)` + scan-substring memory-safety defect |
| `pcre2_next_match()` | **no** | C API helper for safe global-match iteration; ripgrep owns its own matching loop |
| `$+` in `pcre2_substitute()` | **no** | PCRE2 substitution-API feature; `rg --replace` uses ripgrep's printer/replacement syntax |
| `PCRE2_CONFIG_EFFECTIVE_LINKSIZE` | **no** | library configuration introspection, not exposed as an `rg` option |
| faster named-capture lookup during compilation / improved error offsets | implicit | compile-time/runtime-quality improvement inherited by the linked engine |
| `pcre2_callout_enumerate()` crash fix | not normally relevant | library/API robustness fix; ripgrep does not expose callout-enumeration controls |
| ELF symbol-versioning support / modernized CMake exports / expanded platform CI | **no** | build/distribution improvements rather than regex syntax |

**Conservative version gate:** upstream 10.47 NEWS/ChangeLog present returned-capture recursion and `pcre2_next_match()` as 10.47 additions, while the generated 10.47 manpages label them “since 10.46.” Because the published 10.46 release was documented as security-only, this reference uses **10.47** as the minimum deployment gate for those capabilities. [PCRE2-NEWS] [PCRE2-CHANGELOG] [PCRE2-PATTERN] [PCRE2-API]

---

# 26) Lookahead, lookbehind, and assertion design

## 26.0 Positive lookahead

```bash
rg -P '\b\w+(?=\()' src/
```

Find identifier-like words followed by `(` without including the parenthesis in the match.

## 26.1 Negative lookahead

```bash
rg -P '\bfoo\b(?!\s*\()' src/
```

Find `foo` not immediately functioning like a call.

## 26.2 Positive lookbehind

```bash
rg -P '(?<=\bclass\s)\w+' -tpy .
```

## 26.3 Negative lookbehind

```bash
rg -P '(?<![A-Za-z0-9_])unsafe(?![A-Za-z0-9_])' -trust .
```

## 26.4 Prefer assertions when they simplify output, not merely because they exist

Default-engine alternative:

```bash
rg -o 'class\s+\KNAME'   # WRONG: \K is PCRE2-only
```

Often a simple capture under the default engine plus structured parsing is better:

```bash
rg --json 'class\s+(\w+)' src/
```

Use PCRE2 when the assertion itself materially reduces ambiguity or lets you express a condition without consuming context.

## 26.5 Atomic vs non-atomic assertions

Traditional lookarounds are atomic: once successful, ordinary backtracking does not revisit their internal alternatives. Modern PCRE2 also supports non-atomic positive assertions for specialized patterns where captures inside the assertion must change during backtracking. These are powerful but usually harder to audit and can defeat JIT for some combinations. Reserve them for patterns that demonstrably need that behavior. [PCRE2-PATTERN]

---

# 27) Variable-length lookbehind in PCRE2 10.47

PCRE2 10.43 added limited variable-length lookbehind, and that model remains available in PCRE2 10.47. The key constraint is that every top-level branch must have a **known maximum length**. The library’s default maximum for variable-length branches is **255 characters**. [PCRE2-PATTERN]

## 27.0 Works: bounded variable length

```bash
rg -P '(?<=foo.{0,20})bar' .
```

The lookbehind has a known maximum.

## 27.1 Does not work: unbounded lookbehind

```bash
rg -P '(?<=foo.*)bar' .
```

`.*` has no finite maximum, so PCRE2 cannot establish the lookbehind bound.

## 27.2 255-character default is effectively an rg constraint

PCRE2 exposes an API call to raise/lower maximum variable lookbehind. ripgrep 15.2 does **not** expose that compile-context API as a CLI option. Therefore a pattern that needs a variable-length lookbehind beyond the library default should be **rewritten**, not “fixed” by inventing an rg flag.

Alternatives:

```regex
# consume context and use \K
foo.{0,1000}\Kbar

# use capture/output parsing
foo.{0,1000}(bar)

# restructure with lookahead from an earlier anchor
```

## 27.3 `\K` as a practical lookbehind substitute

```bash
rg -P -o 'foo.{0,1000}\Kbar' .
```

`\K` resets the start of the reported match without requiring the preceding text to live inside lookbehind. This is frequently more flexible and can avoid lookbehind length constraints.

---

# 28) Backreferences, named groups, duplicate names, and capture semantics

## 28.0 Numeric backreference

```bash
rg -P '\b(\w+)\s+\1\b' .
```

Find duplicated words.

## 28.1 Matching paired quote delimiters

```bash
rg -P '(?<q>["\x27])(?<body>.*?)\k<q>' .
```

## 28.2 Named backreference forms

PCRE2 supports several named forms, including:

```regex
\k<name>
\k'name'
(?P=name)
\g{name}
```

Prefer `\k<name>` in ripgrep examples because it is explicit and does not collide visually with ripgrep replacement `$name` syntax.

## 28.3 Pattern capture vs replacement capture

Pattern:

```regex
(?<name>...)
```

Backreference **inside the PCRE2 pattern**:

```regex
\k<name>
```

Reference **inside `rg -r` output replacement**:

```text
$name
${name}
```

These are different parsers at different stages.

## 28.4 Backreferences create backtracking risk

A backreference can make matching depend on previously consumed text and removes the regular-language guarantees of ripgrep’s default engine. If a pattern does not truly require text equality, replace the backreference with an ordinary class/literal/alternation and return to the default engine.

---

# 29) Atomic groups, possessive quantifiers, branch reset, and conditionals

## 29.0 Atomic groups

```regex
(?>...)
```

Once the group succeeds, PCRE2 cannot backtrack into it. Useful both for semantic intent and to eliminate pointless backtracking.

Example:

```bash
rg -P '(?>[A-Za-z_][A-Za-z0-9_]*)\s*\(' src/
```

## 29.1 Possessive quantifiers

```regex
.*+
\w++
[0-9]++
```

Possessive quantifiers never give characters back. They are a concise atomicity tool.

Use them where backtracking can never help:

```regex
\b\w++:
```

But do not make a quantifier possessive if a later part may need to reclaim characters.

## 29.2 Branch-reset groups

```regex
(?|(foo)(bar)|(baz)(quux))
```

Capture numbers are reused across alternatives. Useful when several syntactic forms should feed a common replacement/extraction schema.

Agent rule: branch-reset is clearer when downstream logic expects “capture 1 means the same semantic field regardless of alternative.”

## 29.3 Conditionals

PCRE2 supports conditional subpatterns based on whether a group participated, assertions, recursion state and related tests.

Illustrative shape:

```regex
(?<q>["\x27])?value(?(q)\k<q>|\b)
```

Use conditionals sparingly in code-search automation; two simpler `-e` patterns are often easier to audit.

---

# 30) Subroutines, recursion, recursion tests, and recursive patterns

PCRE2 can recurse into the whole pattern or call a named/numbered subpattern.

Common forms include:

```regex
(?R)                recurse whole pattern
(?1)                call group 1
(?&name)            call named group
(?P>name)           named subroutine form
```

## 30.0 Balanced constructs

Recursive regexes can recognize some nested structures that ordinary regular expressions cannot conveniently express. For example, balanced parentheses can be approximated with a recursive group:

```regex
(?<par>\((?:[^()]++|(?&par))*\))
```

```bash
rg -P -U '(?<par>\((?:[^()]++|(?&par))*\))' source.txt
```

## 30.1 Why this is not a parser replacement

Programming languages have lexical states, strings/comments, escaping, macros and grammar context. Recursive PCRE2 is useful for narrow structural evidence, not as a robust substitute for tree-sitter/AST parsing.

Agent rule: use recursive regex for **bounded evidence retrieval**, then validate structure with a parser when correctness depends on syntax.

## 30.2 Returned capture groups from recursion/subroutine calls — 10.47

PCRE2 10.47 adds a powerful extension: a recursive or subroutine call can name capture groups whose values should be **retained when control returns to the caller**. Ordinary subroutine captures normally revert to their prior values after the call; returned-capture syntax selectively keeps specified captures, making a reusable regex subroutine behave more like a function that returns values. [PCRE2-NEWS] [PCRE2-PATTERN]

Supported forms are:

```regex
(?R(grouplist))       recurse whole pattern, returning captures
(?n(grouplist))       call absolute numbered group n, returning captures
(?+n(grouplist))      call relative forward group, returning captures
(?-n(grouplist))      call relative backward group, returning captures
(?&name(grouplist))   call named group, returning captures
(?P>name(grouplist))  Python-style named call, returning captures
```

`grouplist` is comma-separated and may contain absolute/relative group numbers plus group names in single quotes or angle brackets. Only listed groups are retained. The Oniguruma-style `\g<name>` / `\g'name'` subroutine forms do **not** support returned captures. [PCRE2-PATTERN]

Canonical example adapted from the PCRE2 reference:

```bash
rg -P '(?(DEFINE)(?<weekendday>(?|(?<short>Sat)urday|(?<short>Sun)day)))(?&weekendday(<short>)),\k<short>' file.txt
```

This matches values such as:

```text
Saturday,Sat
Sunday,Sun
```

The `short` capture is produced inside the subroutine, explicitly returned with `(<short>)`, and then referenced in the caller with `\k<short>`. Because ripgrep receives final PCRE2 capture locations, retained captures can also participate in normal ripgrep output/capture workflows; test generated capture numbering/naming explicitly when using `--replace` or machine-readable output.

### Version-gating note

The **10.47 NEWS and ChangeLog call this a 10.47 feature**, while the 10.47 `pcre2pattern` page says “Since PCRE2 10.46.” Since upstream describes 10.46 as a security-only release, this reference uses **`min_pcre2: 10.47`** as the conservative deployment gate for returned-capture recursion. [PCRE2-NEWS] [PCRE2-CHANGELOG] [PCRE2-PATTERN] [PCRE2-1046]

## 30.3 Design value for code-search patterns

Returned captures are useful when a large PCRE2 pattern defines reusable lexical fragments once and needs the caller to retain semantic fields extracted inside them. This can reduce duplication in complex evidence queries, especially with `(?(DEFINE)...)` libraries. It does **not** change the recommendation to use AST/CPG tooling when correctness depends on programming-language grammar.

## 30.4 Resource risk

Recursive patterns can consume significant match resources. Combine them with:

- narrow file roots/types;
- explicit match limits when feasible;
- parent-process timeout;
- representative adversarial tests.

---

# 31) Backtracking control verbs, `\K`, `\G`, `\R`, and advanced control

## 31.0 `\K` — reset match start

High leverage for extraction:

```bash
rg -P -o '\bclass\s+\K[A-Za-z_][A-Za-z0-9_]*' -tpy .
```

It keeps context in the pattern but removes it from the reported match.

## 31.1 `(*SKIP)(*F)` — exclude regions by making them fail-and-skip

Classic pattern family:

```regex
UNWANTED(*SKIP)(*F)|WANTED
```

Example: find TODO outside simple quoted strings (illustrative, not a full language lexer):

```bash
rg -P '"(?:\\.|[^"\\])*"(*SKIP)(*F)|\bTODO\b' src/
```

This pattern says: when a quoted string is recognized, skip it as a candidate region; otherwise search for TODO.

## 31.2 `(*PRUNE)`, `(*COMMIT)`, `(*THEN)`

These alter the backtracking search tree:

- `(*PRUNE)` discards selected backtracking paths;
- `(*COMMIT)` prevents the engine from bumping the starting position after failure;
- `(*THEN)` prunes to the next alternative at an appropriate level.

They can both improve performance and radically change semantics. Use them only with test cases showing intended failures as well as intended matches.

## 31.3 `(*ACCEPT)` / `(*FAIL)`

`(*ACCEPT)` forces immediate success at its point; `(*FAIL)`/`(*F)` forces failure.

Historical security note: `(*ACCEPT)` combined with scan-substring was part of CVE-2025-58050 in PCRE2 10.45. PCRE2 10.47 includes the 10.46 fix; see §40 for the current untrusted-pattern posture.

## 31.4 `\G`

`\G` matches at the first matching position in the subject or at continuation-sensitive positions in repeated matching APIs. In grep-style global searching it is more subtle than in a hand-managed iterative parser. Test its exact behavior under ripgrep before using it as a tokenizer state primitive.

## 31.5 `\R`

`\R` matches newline sequences according to PCRE2’s BSR policy, not simply the configured `.` newline rule. Pattern-start BSR controls are covered in §37.

---

# 32) Unicode/UCP, properties, script runs, case folding, ASCII restrictions

## 32.0 ripgrep default PCRE2 configuration

With ordinary `rg -P`:

```text
UTF  = on
UCP  = on
```

unless `--no-unicode` is used. [RG-HIARGS]

Consequences:

```regex
\w      Unicode-aware word character
\d      Unicode-aware decimal digit class under UCP semantics
\s      Unicode-aware whitespace
\b      boundary based on PCRE2 word-character semantics
\p{...} Unicode properties
```

## 32.1 PCRE2 10.47 uses Unicode 16 data (inherited from 10.45)

PCRE2 10.45 updated the Unicode Character Database to UCD 16, and PCRE2 10.47 retains that Unicode data baseline. Results involving newly assigned characters/properties can therefore differ from older PCRE2 builds. [PCRE2-1045-NEWS] [PCRE2-PATTERN]

## 32.2 10.45+ case-insensitive property behavior retained in 10.47

In 10.45, caseless matching of `\p{Ll}`, `\p{Lt}` and `\p{Lu}` changed to align with Perl: `/\p{Ll}/i` is no longer restricted to lower-case characters. PCRE2 10.47 retains this behavior. This also affects related POSIX lower/upper classes. [PCRE2-1045-NEWS] [PCRE2-PATTERN]

Version-sensitive agent rule: if a query depends on Unicode case category under `-i`, record PCRE2 version with the evidence.

## 32.3 PCRE2 10.43+ `\w` behavior retained in 10.47

In UCP mode, modern PCRE2 broadens `\w` to Perl-aligned categories including letters/numbers plus selected marks/connector punctuation. PCRE2 10.47 retains that model, so `\b` behavior can differ from older PCRE2 builds. [PCRE2-PATTERN]

For programming-language ASCII identifiers, explicit classes may be more stable:

```regex
[A-Za-z_][A-Za-z0-9_]*
```

or disable Unicode for the invocation when appropriate.

## 32.4 Script runs

PCRE2 pattern syntax supports script-run groups:

```regex
(*script_run:...)
(*sr:...)
```

They constrain matched codepoints to a compatible Unicode script run, useful for mixed-script/spoofing analysis.

```bash
rg -P '\s+(*sr:\S+)' .
```

This is a lexical Unicode-security tool, not a full identifier policy implementation.

## 32.5 ASCII restriction pattern constructs

PCRE2 versions leading into 10.45 added controls for restricting selected shorthand/caseless interactions to ASCII, and those controls remain part of the 10.47 feature surface. Some controls exist as **extra compile options** and are not exposed by ripgrep; only pattern-level controls documented by PCRE2 can be assumed usable in `rg -P`. Keep that API/pattern distinction explicit.

---

# 33) PCRE2 10.47 extended character classes and set algebra

PCRE2 10.45 added two different extended-class systems; both remain in 10.47. Only one is reliably available through ripgrep without an application compile-option change. [PCRE2-1045-NEWS] [PCRE2-PATTERN]

## 33.0 Perl-style extended classes — directly usable through `rg -P` (introduced in 10.45, retained in 10.47)

Syntax:

```regex
(?[ ... ])
```

Set operations include union, intersection, subtraction, symmetric difference and complement according to PCRE2 extended-class grammar.

Example from the 10.45 feature family — letters in Thai or Greek scripts:

```bash
rg -P '(?[\p{L} & (\p{Thai} + \p{Greek})])+' .
```

This syntax is self-contained in the pattern and therefore is the **recommended PCRE2 10.45+ set-algebra syntax with ripgrep**, including in a verified 10.47 environment.

## 33.1 UTS#18 ordinary `[...]` extended syntax — compile option required

PCRE2 10.45 also added UTS#18-compatible nested classes and operators such as `&&`, `--` and `~~` inside ordinary brackets, and 10.47 retains them, but that behavior requires the application to compile with `PCRE2_ALT_EXTENDED_CLASS`. [PCRE2-1045-NEWS] [PCRE2-PATTERN]

Ripgrep 15.2’s PCRE2 builder does not expose a CLI switch for that extra compile option. Therefore **do not assume** this form works through stock `rg -P`:

```regex
[\p{L}&&[\p{Thai}||\p{Greek}]]
```

Use the Perl-style `(?[...])` form instead.

## 33.2 Performance

PCRE2 10.45 rewrote the character-class match engine so large/complex classes are more compact and can use binary search internally; 10.47 retains that implementation family. This can make set-heavy PCRE2 patterns more practical, but benchmark realistic corpus/pattern combinations rather than inferring speed from syntax alone. [PCRE2-1045-NEWS]

---

# 34) PCRE2 10.47 scan-substring assertions

Scan substring is an assertion family introduced in **PCRE2 10.45** and retained in 10.47. It applies a subpattern to the contents of a previously captured substring. Syntax:

```regex
(*scan_substring:(GROUP-LIST)SUBPATTERN)
(*scs:(GROUP-LIST)SUBPATTERN)
```

Group references may be absolute, relative or named. [PCRE2-PATTERN] [PCRE2-1045-NEWS]

## 34.0 Basic example

```bash
rg -P '\b(\w++)(*scs:(1).+rh)' .
```

This captures a word, then scans that capture from its beginning for a subpattern containing `rh` after at least one character.

## 34.1 Semantics

The substring scan:

- is anchored at the **start of the captured substring**;
- temporarily treats the substring end as subject end for `$`, `\Z`, `\z`;
- does not reset the main subject start, so lookbehind can still inspect before the captured substring;
- can contain captures that survive if the assertion succeeds.

[PCRE2-PATTERN]

## 34.2 When it helps

Use it when the main match must first identify a logical substring and then validate complex internal structure without re-consuming the entire outer pattern.

Potential use cases:

- captured identifier must satisfy a second constraint;
- captured token needs an internal palindrome-like check;
- a delimited substring must be validated independently of outer context.

## 34.3 When it is overkill

If a direct expression is clearer, prefer it:

```regex
\b\w+?rh\w*\b
```

is easier to audit than capture + scan-substring for the simple `rh` example.

## 34.4 JIT interaction

Scan-substring support is not equivalent to “all patterns JIT cleanly.” PCRE2 10.45 introduced improved JIT error reporting with `PCRE2_ERROR_JIT_UNSUPPORTED`, retained in 10.47 for unsupported constructs. Ripgrep uses `jit_if_available`, so a pattern that cannot be JIT compiled can fall back to interpreted PCRE2 rather than necessarily failing the entire `rg` command. [PCRE2-1045-NEWS] [RG-HIARGS]

## 34.5 Security warning

PCRE2 10.47 retains scan-substring syntax but includes the 10.46 fix for CVE-2025-58050. The historical 10.45 memory-safety issue therefore does **not** apply to a verified 10.47 runtime. Scan-substring patterns can still be computationally expensive, so untrusted-pattern deployments should retain the general safeguards in §40. [PCRE2-1046] [PCRE2-NEWS]

---

# 35) Pattern-level resource limits and safety hardening

PCRE2 lets a **pattern itself** lower selected matching limits at its beginning:

```regex
(*LIMIT_MATCH=100000)
(*LIMIT_DEPTH=1000)
(*LIMIT_HEAP=8192)
```

[PCRE2-PATTERN]

## 35.0 Example

```bash
rg -P '(*LIMIT_MATCH=100000)(a+)+b' suspect.txt
```

The pattern writer can only lower caller/default limits, not raise them.

## 35.1 What each limit means

- `LIMIT_MATCH` bounds the matcher’s work counter; it applies in different ways to interpreter/JIT.
- `LIMIT_DEPTH` bounds nested backtracking depth for the interpreter; JIT ignores this depth limit.
- `LIMIT_HEAP` limits interpreter heap in KiB; it does **not** constrain JIT memory in the same way.

[PCRE2-PATTERN]

## 35.2 Why these are not a complete sandbox

Ripgrep attempts JIT on 64-bit, so `LIMIT_DEPTH` and `LIMIT_HEAP` do not provide the same protections when JIT is active. `LIMIT_MATCH` is the most broadly relevant pattern-level work limiter, but a hostile search can still consume resources through:

- directory traversal;
- huge files;
- decompression/preprocessors;
- enormous output;
- regex compilation;
- JIT compilation;
- memory mapping/reading;
- many concurrent files.

Therefore service hardening must combine regex limits with process/container limits and search-space policy.

## 35.3 `(*NO_JIT)` as a diagnostic/safety lever

PCRE2 supports:

```regex
(*NO_JIT)...
```

When present at pattern start, an application JIT request is ignored. [PCRE2-PATTERN]

This is usable through `rg -P` and is valuable for:

- comparing JIT vs interpreter behavior;
- diagnosing a JIT-specific issue;
- making interpreter-specific resource limits relevant.

It is **not** generally a performance recommendation.

## 35.4 Optimization-control verbs

PCRE2 also supports pattern-start verbs such as:

```regex
(*NO_AUTO_POSSESS)
(*NO_START_OPT)
(*NO_DOTSTAR_ANCHOR)
```

These disable optimizations. They are primarily diagnostic or semantics-edge-case tools. Turning off start optimizations can make a search dramatically slower; never put these into a generic agent template.

---

# 36) JIT behavior, unsupported JIT features, and performance design

## 36.0 ripgrep 15.2 JIT policy

On 64-bit targets:

```text
jit_if_available(true)
max JIT stack = 10 MiB
```

On 32-bit targets, ripgrep does not attempt this JIT path because its source documents PCRE2 JIT compile failures due to memory constraints. [RG-HIARGS]

## 36.1 Verify build capability

```bash
rg --version
rg --pcre2-version
```

A release build can report:

```text
PCRE2 10.47 is available (JIT is available)
```

Capability does not prove every pattern is JIT-supported.

### The reported version can be lower than the linked library

`--pcre2-version` prints the version ripgrep was **built against**, taken from the PCRE2 headers at compile time. A binary that links PCRE2 dynamically can be running against a newer shared library than that string names, so the report is a **floor claimed by the build, not a measurement of the runtime**.

This is observable. On a ripgrep 15.2.0 build reporting

```text
PCRE2 10.45 is available (JIT is available)
```

the 10.47-only returned-capture recursion of section 30.2 nevertheless matches, while the same pattern without the returned-capture list correctly fails — so 10.47 semantics were live despite the 10.45 string.

The consequence for automation is asymmetric and worth stating plainly:

* a version report **at or above** the required version is good evidence the feature is present;
* a version report **below** it is **not** evidence the feature is absent.

Never gate a capability off a low version string alone. Feature-probe instead — section 42.9.

## 36.2 JIT-unsupported error behavior (introduced in 10.45, retained in 10.47)

PCRE2 10.45 introduced `PCRE2_ERROR_JIT_UNSUPPORTED` for patterns using features unsupported by JIT; that behavior remains in 10.47. [PCRE2-1045-NEWS]

Ripgrep’s “JIT if available” wrapper documents fallback behavior when JIT compilation returns an error, so a complex pattern may still run through the interpreter. [RG-PCRE2-MATCHER]

Performance implication: two PCRE2 patterns on the same rg binary may use different execution paths.

## 36.3 PCRE2 10.47 AArch64 JIT SIMD

PCRE2 10.47 adds **new SIMD code generation in the JIT for AArch64**. This is directly relevant to `rg -P` on 64-bit ARM systems (for example Apple Silicon and AArch64 Linux) when:

- the ripgrep binary is actually using PCRE2 10.47;
- `rg --pcre2-version` reports JIT availability; and
- the particular pattern can be JIT-compiled.

There is no new ripgrep flag for this feature; it is an implementation-level performance improvement inherited automatically from PCRE2. It should be treated as a potential acceleration, not a guarantee that every PCRE2 query is faster than 10.45/10.46 or faster than ripgrep's default engine. Benchmark representative patterns. [PCRE2-CHANGELOG]

## 36.4 Benchmark compile + search separately conceptually

JIT can make repeated searching fast but adds compile cost. Ripgrep compiles the matcher once per invocation and can then apply it across many files, so JIT amortizes better on:

- large file sets;
- large files;
- selective patterns with nontrivial matching work.

For one tiny input, PCRE2 compile/JIT overhead can dominate.

## 36.5 Literal-prefilter advantage remains important

Even with JIT, a pattern exposing useful fixed text can be much faster than a generic backtracking expression.

Prefer:

```regex
ERROR.*?timeout
```

when semantically correct over:

```regex
\w+.*?\w+
```

that provides no strong literal anchor.

## 36.6 Avoid catastrophic families

Classic danger shape:

```regex
(a+)+b
```

against a long run of `a` without `b`.

Improved shape where semantically valid:

```regex
(?>a+)+b
```

or simply:

```regex
a+b
```

Best optimization is usually **simplifying the language**, not sprinkling atomic constructs onto an unnecessarily complex pattern.

---

# 37) PCRE2 newline, BSR, multiline, dotall, CRLF, and ripgrep interactions

## 37.0 Ripgrep line orientation vs PCRE2 newline semantics

Ripgrep’s searcher normally provides line-oriented slices unless `-U` is enabled. Separately, PCRE2 itself has newline conventions affecting `^`, `$`, `.`, and `\N`.

Pattern-start newline verbs include:

```regex
(*CR)
(*LF)
(*CRLF)
(*ANYCRLF)
(*ANY)
(*NUL)
```

[PCRE2-PATTERN]

## 37.1 Ripgrep `--crlf`

Prefer the ripgrep flag when the desired policy is globally “treat CRLF correctly as the line terminator”:

```bash
rg -P --crlf '^foo$' windows-tree/
```

This keeps searcher and matcher line-terminator behavior aligned.

## 37.2 Pattern-specific newline override

For specialized PCRE2 semantics:

```bash
rg -P -U '(*ANYCRLF)^foo$' mixed.txt
```

Do not use a pattern-level newline override casually when ripgrep’s searcher is still splitting input according to a different line terminator; `-U` is often necessary when the PCRE2 expression must reason about multiple newline styles inside one subject buffer.

## 37.3 `\R` and BSR

By default `\R` follows Unicode newline semantics. PCRE2 lets the pattern change that:

```regex
(*BSR_ANYCRLF)\R
(*BSR_UNICODE)\R
```

Use `(*BSR_ANYCRLF)` when `\R` should mean CR, LF or CRLF only.

## 37.4 Dotall

```bash
rg -P -U '(?s)BEGIN.*?END' .
```

or:

```bash
rg -P -U --multiline-dotall 'BEGIN.*?END' .
```

## 37.5 Anchors under ripgrep

Because ripgrep sets PCRE2 multiline compile mode:

```regex
^  start of line
$  end of line
\A absolute start of searched subject
\z absolute end of searched subject
```

Use:

```bash
rg -P -U '(?s)\A.*required-marker.*\z' file.txt
```

when the condition truly concerns the whole file/haystack.

## 37.6 Multiline memory implication

A PCRE2 pattern that can cross `\n` requires ripgrep multiline search (`-U`) and can force contiguous buffering/mmap. This changes the I/O problem as much as the regex problem. Narrow paths and file sizes accordingly.

---

# 38) PCRE2 replacement API vs ripgrep `--replace`

This is a critical API boundary.

## 38.0 PCRE2 10.45–10.47 `pcre2_substitute()` evolution

PCRE2 10.45 substantially expanded replacement-language features in its **C API substitution function**, including additional octal escapes, title-casing/case transforms, more backreference forms and special variables. PCRE2 10.47 adds `$+` substitution support and improves validation when `PCRE2_SUBSTITUTE_MATCHED` is used. These remain **library API** features, not ripgrep printer syntax. [PCRE2-1045-NEWS] [PCRE2-NEWS] [PCRE2-CHANGELOG]

## 38.1 ripgrep does not use `pcre2_substitute()` for `-r`

Ripgrep’s matcher returns capture locations to its own printer, and ripgrep’s printer applies the `--replace` template. Therefore PCRE2 `pcre2_substitute()` enhancements—including 10.47 `$+`—**do not automatically become `rg -r` syntax**. [RG-HIARGS] [RG-GUIDE]

Correct ripgrep replacement style:

```bash
rg -P '(?<first>\w+)\s+(?<last>\w+)' \
  -r '$last, $first' names.txt
```

Do not assume this PCRE2 library replacement syntax is supported by `rg -r` merely because PCRE2 supports it in `pcre2_substitute()`:

```text
\u\L...
$&
$`
$'
$_
```

Consult ripgrep replacement docs, not PCRE2 substitute docs, for the output template.

## 38.2 Why this design is useful

It makes replacement behavior broadly consistent across ripgrep’s default and PCRE2 engines. Engine selection changes how the **match** is found; the printer remains the output-rewrite layer.

---

# 39) PCRE2 library capabilities not exposed by ripgrep

A comprehensive reference must distinguish “PCRE2 has this API” from “`rg -P` can configure it.”

## 39.0 Extra compile options with no rg CLI mapping

PCRE2 10.45 introduced extra options that remain available in 10.47, including:

```text
PCRE2_EXTRA_NO_BS0
PCRE2_EXTRA_PYTHON_OCTAL
PCRE2_EXTRA_NEVER_CALLOUT
PCRE2_EXTRA_TURKISH_CASING
```

and a `pcre2_set_optimize()` API. [PCRE2-NEWS]

Ripgrep 15.2’s PCRE2 builder path does not expose CLI switches for these options. Do not document them as `rg` flags.

## 39.1 UTS#18 ordinary extended class option

`PCRE2_ALT_EXTENDED_CLASS` is an application compile option, not a pattern-start switch exposed by ripgrep. Use `(?[...])` extended classes instead (§33).

## 39.2 Variable-lookbehind max setter

PCRE2 exposes a compile-context API to adjust the default 255-character variable-lookbehind maximum. ripgrep does not expose it. Rewrite the regex when the default is insufficient.

## 39.3 Match-context setters

PCRE2 has API-level setters for match limits, heap/depth limits, callouts and other match context. ripgrep does not give arbitrary access to the PCRE2 match context. Pattern-level limits can lower defaults; otherwise use an embedding built directly on PCRE2 if you need fine-grained context control.

## 39.4 JIT stack setter

The underlying wrapper supports a JIT-stack limit and ripgrep sets it internally to 10 MiB on 64-bit. There is no ordinary user-facing `rg --pcre2-jit-stack=...` flag.

## 39.5 Callouts

PCRE2 pattern callouts require an application-provided callback to be useful. Ripgrep is not a general PCRE2 callout host. Do not design `rg -P` workflows around application-specific callout processing.

## 39.6 Substitution callback/API

As §38 explains, `pcre2_substitute()` and its callback/configuration APIs are outside ripgrep’s replacement implementation.

## 39.7 PCRE2 10.47 library/API/build delta not exposed as ripgrep flags

The 10.47 release contains several additions that matter to PCRE2 embedders or packagers but are **not new `rg` CLI capabilities**. [PCRE2-NEWS] [PCRE2-CHANGELOG]

| PCRE2 10.47 change | Stock `rg -P` impact |
|---|---|
| returned captures from recursion/subroutines | **direct pattern feature**; see §30.2 |
| `pcre2_next_match()` | no direct CLI/API knob; ripgrep owns its own match iteration |
| `PCRE2_CONFIG_EFFECTIVE_LINKSIZE` | no CLI surface; library configuration introspection only |
| `$+` in `pcre2_substitute()` | no `rg --replace` carry-through; see §38 |
| improved `PCRE2_SUBSTITUTE_MATCHED` validation | direct-PCRE2 API only |
| `pcre2_callout_enumerate()` Unicode-class crash fix | not a normal `rg -P` callout-enumeration path |
| AArch64 JIT SIMD generation | implicit performance improvement when eligible; see §36.3 |
| faster named-capture lookup during compilation | possible implicit compile-time benefit; no flag |
| improved compile-error offsets/diagnostics | may improve surfaced PCRE2 compilation errors; no new syntax |
| dynamic-library symbol versioning | packaging/linking concern on supported Unix linkers |
| modern CMake exports | PCRE2 embedding/build-system concern |
| z/OS/native EBCDIC support | PCRE2 platform support, generally outside mainstream ripgrep deployments |
| expanded CI/build tooling | maintenance/portability, not regex syntax |

### `pcre2_next_match()` boundary

PCRE2 10.47 ships `pcre2_next_match(match_data, &start_offset, &options)` to make repeated/global matching safer, including correct progress after empty matches. Ripgrep does not expose this function; its searcher/matcher layers manage iteration themselves. Therefore this API is relevant if you embed PCRE2 directly, not when constructing an `rg -P` command. [PCRE2-API] [PCRE2-NEWS]

### 10.47 symbol versioning

PCRE2 10.47 adds linker scripts for dynamic-library symbol versioning on tested Linux/Solaris/FreeBSD toolchains. This can matter when distributing a dynamically linked custom ripgrep build, but it does not alter pattern semantics. The authoritative runtime check remains `rg --pcre2-version`, not merely `pcre2-config --version` or the presence of a package in the shell environment. [PCRE2-NEWS] [RG-README]

### Documentation chronology note

The 10.47 API manpage describes `pcre2_next_match()` as available since 10.46, while the 10.47 NEWS/ChangeLog call it a new 10.47 API. For ripgrep users this discrepancy has no practical CLI consequence because the function is not exposed; for direct PCRE2 embedding, gate conservatively on 10.47 unless the exact 10.46 artifact has been tested.

### Decision rule

If a requirement says “I need to call `pcre2_set_*`,” `pcre2_next_match()`, or another PCRE2 C API function, you probably need a direct PCRE2 binding/application, not ripgrep CLI.

---

# 40) PCRE2 10.47 security posture and untrusted-pattern policy

## 40.0 10.46 security fix is included

PCRE2 10.46 was a security-only release fixing CVE-2025-58050, a read-past-end bug introduced in 10.45 when an attacker-controlled pattern combined `(*ACCEPT)` with scan-substring (`(*scs:)` / `(*scan_substring:)`). PCRE2 10.47 includes that fix. A verified 10.47 runtime therefore does **not** carry the specific 10.45 vulnerability. [PCRE2-1046] [PCRE2-NEWS]

## 40.1 10.47 additional correctness/security-relevant fixes

PCRE2 10.47 also fixes a crash in `pcre2_callout_enumerate()` reachable with patterns containing Unicode character classes. Stock ripgrep does not expose `pcre2_callout_enumerate()` as a CLI facility, so this is primarily relevant to direct PCRE2 embeddings and tooling that introspects callouts rather than ordinary `rg -P` searching. [PCRE2-NEWS] [PCRE2-CHANGELOG]

## 40.2 Risk matrix

| Pattern source | Subject source | 10.47 posture |
|---|---|---|
| trusted developer | trusted repo | normal advanced-CLI risk |
| trusted developer | untrusted text | memory-safety issue above fixed; still benchmark pathological near-misses |
| untrusted user | trusted repo | **high resource-risk boundary** because PCRE2 permits backtracking/recursion/control verbs |
| untrusted user | untrusted corpus | isolate and constrain; prefer default engine when possible |

## 40.3 Preferred service posture

For any service accepting user-controlled regular expressions:

1. verify **PCRE2 10.47** (or a deliberately chosen later supported release) with `rg --pcre2-version` at startup;
2. prefer ripgrep's default engine unless PCRE2-only syntax is explicitly required;
3. constrain roots/globs/types and file sizes;
4. enforce subprocess wall-clock/CPU/memory limits;
5. cap output bytes/results;
6. avoid arbitrary preprocessors;
7. optionally reject recursion, scan-substring, backtracking verbs, or nested unbounded repetition in untrusted patterns;
8. log exact rg/PCRE2 versions and selected engine with the request.

## 40.4 Pattern-level limits are defense in depth only

`(*LIMIT_MATCH=...)`, `(*LIMIT_DEPTH=...)`, and `(*LIMIT_HEAP=...)` can reduce interpreter resource use, but they are not a complete sandbox. JIT ignores depth and heap limits, and the surrounding ripgrep process still consumes I/O, traversal, decompression, buffering and output resources.

## 40.5 Default engine as the safer request language

If users do not need backreferences/lookaround/recursion, expose ripgrep's **default engine** instead of PCRE2. This reduces regex-language attack surface and restores the default engine's finite-automata-oriented performance guarantees.

## 40.6 Allow advanced PCRE2 only as an explicit capability

```text
ordinary search API -> --engine=default
advanced trusted search API -> --engine=pcre2
untrusted arbitrary PCRE2 -> isolated + explicit limits + version gate
```

---

# 41) High-value code-search recipes for programming agents

These recipes are intended as patterns to adapt, not as claims that regex replaces language-aware parsing.

## 41.0 Find a symbol definition with an exact token boundary

Default engine first:

```bash
rg -n -tpy '\bclass\s+Target\b' src tests
```

Use PCRE2 only if contextual exclusion is necessary.

## 41.1 Same line must contain A and B in either order

PCRE2 lookaheads:

```bash
rg -P -n '^(?=.*\bfoo\b)(?=.*\bbar\b).*$' src/
```

Default-engine alternative if order can be enumerated:

```bash
rg -n '\bfoo\b.*\bbar\b|\bbar\b.*\bfoo\b' src/
```

Choose based on clarity and performance, not on a reflexive preference for PCRE2.

## 41.2 Find a token not preceded/followed by identifier characters

```bash
rg -P '(?<![A-Za-z0-9_])Target(?![A-Za-z0-9_])' src/
```

If ordinary word semantics are adequate, `-w Target` under the default engine is simpler and faster.

## 41.3 Extract Python class names without consuming `class `

```bash
rg -P -o -tpy '\bclass\s+\K[A-Za-z_][A-Za-z0-9_]*' .
```

## 41.4 Extract import target after contextual prefix

```bash
rg -P -o -tpy '(?<=\bfrom\s)[A-Za-z_][A-Za-z0-9_.]*' .
```

If the prefix length could grow beyond practical lookbehind limits, use `\K`:

```bash
rg -P -o -tpy '\bfrom\s+\K[A-Za-z_][A-Za-z0-9_.]*' .
```

## 41.5 Duplicate word/token detection

```bash
rg -P -n '\b(?<w>[A-Za-z_][A-Za-z0-9_]*)\s+\k<w>\b' .
```

## 41.6 Same quote delimiter

```bash
rg -P '(?<q>["\x27])(?:\\.|(?!\k<q>).)*\k<q>' file
```

This is still not a complete string-literal grammar for every language.

## 41.7 Exclude simple quoted strings then find token

```bash
rg -P '"(?:\\.|[^"\\])*"(*SKIP)(*F)|\bTODO\b' src/
```

This illustrates `(*SKIP)(*F)`; do not use it as a language lexer if comments/raw strings/interpolation matter.

## 41.8 Find call-like occurrences but not member-name prefixes

```bash
rg -P -n '(?<![A-Za-z0-9_])target(?=\s*\()' src/
```

## 41.9 Find a block spanning lines

```bash
rg -P -U '(?s)^BEGIN\b.*?^END\b' path
```

Because ripgrep enables PCRE2 multiline anchors, `^` is line-aware. `(?s)` makes dot cross newline; `-U` permits the searcher to return cross-line matches.

## 41.10 Extract only the body between delimiters

```bash
rg -P -U -o '(?s)BEGIN\K.*?(?=END)' path
```

Be careful on huge files: unbounded lazy dot still may inspect a large region.

## 41.11 Find Python definitions with decorators nearby

```bash
rg -P -U -tpy '(?s)(?:^\s*@[^\n]+\n)+^\s*(?:async\s+)?def\s+target\b' .
```

For correctness about decorator syntax/nesting, switch to AST tooling after locating candidates.

## 41.12 Search for a configuration key outside comments (simple format)

```bash
rg -P -n '^\s*(?!#)(?<key>[A-Za-z_][A-Za-z0-9_.-]*)\s*=' config/
```

## 41.13 Match Unicode identifiers from a specific script family

With PCRE2 10.47 extended classes (feature introduced in 10.45):

```bash
rg -P '(?[\p{L} & (\p{Greek} + \p{Latin})])+' .
```

## 41.14 Inspect a previously captured token with scan substring

```bash
rg -P '\b([A-Za-z_][A-Za-z0-9_]*+)(*scs:(1).*_[A-Z])' src/
```

This demonstrates the scan-substring feature introduced in 10.45 and available in 10.47. Use it only when the “capture then independently validate its contents” shape is genuinely clearer than one direct regex.

## 41.15 Find repeated delimiters with branch reset

```bash
rg -P '(?|"([^"]*)"|\x27([^\x27]*)\x27)' .
```

Both alternatives place the content in capture group 1. For complex downstream processing, named fields or JSON output are still easier to maintain.

## 41.16 Search only likely source files, not dependencies/artifacts

```bash
rg -n \
  -tpy -trust -truby -tjs \
  -g '!**/vendor/**' \
  -g '!**/generated/**' \
  -g '!**/dist/**' \
  'needle' src tests
```

The exact type set should be derived from the repository, not copied mechanically.

---

# 42) Evidence-oriented search patterns for LLM coding agents

## 42.0 Use ripgrep as a direct-evidence primitive

For code intelligence, a good ripgrep query should answer a narrow evidence question:

```text
“Where is this symbol mentioned?”
“Where is this config key defined?”
“Which files contain both tokens?”
“Where does this exact error string originate?”
“Which callsites use this literal argument?”
```

It should not be asked to infer semantic call graphs or type relationships that require parsing/type resolution.

## 42.1 Prefer staged narrowing

Stage 1 — file discovery:

```bash
rg --files -tpy src/
```

Stage 2 — high-recall literal/regular search:

```bash
rg -n -F 'target_symbol' -tpy src/
```

Stage 3 — contextual PCRE2 if needed:

```bash
rg -P -n '(?<![A-Za-z0-9_])target_symbol(?=\s*\()' -tpy src/
```

Stage 4 — parser/type-system confirmation using AST/CPG/LSP tooling.

## 42.2 Literal-first policy

If the information need names an exact symbol/string, start with:

```bash
rg -F -n 'ExactThing' relevant/root
```

This is faster, has fewer escaping mistakes and gives a direct baseline before a more selective regex is introduced.

## 42.3 Structured machine output

For an agent executor, use JSON when precise locations matter:

```bash
rg --json -F 'ExactThing' src/
```

Persist:

```text
path
line number
absolute byte offset
line bytes/text
submatch start/end
```

Then the LLM can cite direct evidence without reparsing human terminal formatting.

## 42.4 Use `-o` when the desired evidence is one field

```bash
rg -P -o '\bclass\s+\K[A-Za-z_][A-Za-z0-9_]*' -tpy src/
```

For a shell pipeline, this can be more efficient than sending full lines to a later parser.

## 42.5 Query determinism template

For reproducible repository search:

```bash
rg \
  --engine=default \
  --color=never \
  --no-messages \
  -n \
  -tpy \
  'PATTERN' \
  ./src
```

Adapt ignore/message behavior based on whether errors need to be surfaced; do not suppress errors in validation workflows where incomplete evidence would be dangerous.

## 42.6 PCRE2 capability gate

Programmatic initialization:

```bash
if rg --pcre2-version >/dev/null 2>&1; then
    : # advanced regex available
else
    : # use default-engine rewrite or reject advanced query
fi
```

Also inspect the version string if a feature requires 10.43+, 10.45+, or 10.47 specifically.

## 42.7 Why PCRE2 version-gated features need explicit capability metadata

A query using:

```regex
(?[...])
(*scs:...)
```

is not merely “PCRE2.” It requires a sufficiently new PCRE2. Returned-capture recursion additionally requires 10.47. If an agent persists reusable query templates, annotate them with:

```text
engine: pcre2
min_pcre2: 10.45  # for 10.45-era extended classes / scan-substring
# use min_pcre2: 10.47 when the pattern uses returned-capture recursion
multiline_required: yes/no
unicode_required: yes/no
trusted_pattern_only: yes/no
```

## 42.8 Evidence completeness vs grep completeness

No grep command can prove “this is the complete call graph” in a language with aliases, imports, macros, dynamic dispatch, generated code or reflection. The correct contract is:

```text
ripgrep = direct lexical evidence + candidate retrieval
parser/CPG/type system = semantic confirmation/completeness
```

## 42.9 Feature-probe, do not version-probe

Sections 42.6 and 42.7 gate advanced patterns on the version string. That is the right shape but the wrong instrument on its own, because `--pcre2-version` can under-report the library actually linked (section 36.1). A version below the requirement does not establish that the construct is unavailable.

Probe the construct itself. A usable probe needs two halves — the feature, and a control that must fail — because a pattern that matches for the wrong reason proves nothing:

```bash
probe_returned_captures() {
    printf 'Saturday,Sat\n' > "$1/probe.txt"
    # the feature: a subroutine call that returns a named capture (10.47)
    rg -qP '(?(DEFINE)(?<d>(?|(?<s>Sat)urday|(?<s>Sun)day)))(?&d(<s>)),\k<s>' "$1/probe.txt" || return 1
    # the control: same pattern without the returned-capture list must NOT match
    rg -qP '(?(DEFINE)(?<d>(?|(?<s>Sat)urday|(?<s>Sun)day)))(?&d),\k<s>'      "$1/probe.txt" && return 1
    return 0
}
```

Record the **probe result** as the capability fact, alongside `rg --version` and the reported `--pcre2-version`, and treat a disagreement between the string and the probe as information rather than as an error. A probe is a few milliseconds and it is the only claim that survives a rebuild, a repackage, or a shared-library upgrade underneath a pinned binary.

The same discipline applies to any construct this document version-gates: scan-substring assertions (section 34), extended character classes (section 33), and returned-capture recursion (section 30.2). It is the ripgrep-side instance of a general rule — an absent signal is not a negative result, it is an unknown.

---

# 43) Performance engineering and anti-pattern inventory

## 43.0 Anti-pattern: `-P` for every search

Bad default:

```bash
rg -P 'literal_or_simple_regex' .
```

Better:

```bash
rg -F 'literal' .
rg 'simple_regex' .
```

PCRE2 is a capability escalation, not the baseline engine.

## 43.1 Anti-pattern: repository-wide multiline dotall

```bash
rg -P -U '(?s).*foo.*bar.*' .
```

Problems:

- can force full-file/contiguous reads;
- extremely weak literal prefix;
- can produce giant matches/output;
- PCRE2 backtracking surface grows;
- every file in the tree is considered.

Better: narrow types/paths and introduce bounded structure/literals.

## 43.2 Anti-pattern: unbounded nested repeats

```regex
(.+)+
(.*a)*
(a|aa)+b
```

These are classic backtracking hazards on near-miss data.

Prefer a single quantifier, atomic/possessive structure where semantically valid, or default-engine reformulation.

## 43.3 Anti-pattern: `-uuu -L` in normal code search

```bash
rg -uuu -L 'needle' /
```

This destroys the very filtering that makes ripgrep useful and can enter dependency trees, binary blobs, hidden caches, mounts and symlink-expanded trees.

Use unrestricted mode for diagnostics/audits, not as an agent default.

## 43.4 Anti-pattern: parse terminal output

Do not split lines on `:` and hope to reconstruct Windows paths, columns and content. Use `--json` for structured data or NUL-delimited file-list modes for filename transport.

## 43.5 Anti-pattern: global sort when order is irrelevant

Sorting disables some parallel execution benefits and creates extra coordination. Let the consumer sort only if deterministic display is actually required.

## 43.6 Anti-pattern: enormous output

If the consumer needs “is there any match?” use `-q`; if it needs files, use `-l`; if it needs counts, use count modes. Printing millions of matching lines wastes more time than regex matching itself.

## 43.7 Anti-pattern: giant catch-all pattern files

Thousands of unrelated alternatives can make compile behavior opaque, create capture-number interactions and prevent query-specific literal optimization. Partition pattern libraries by information need.

## 43.8 Anti-pattern: assuming `--regex-size-limit` protects PCRE2

It is only wired into ripgrep’s Rust matcher in 15.2. Use PCRE2/resource controls appropriate to PCRE2 and external limits.

## 43.9 Anti-pattern: API-only PCRE2 features in `rg` recipes

If documentation says “call `pcre2_set_*`,” stop: stock ripgrep CLI cannot make arbitrary PCRE2 API calls.

## 43.10 Anti-pattern: trust old PCRE2 performance folklore

Old ripgrep FAQ discussions of `--no-pcre2-unicode` reflect prior PCRE2 integration constraints. Current ripgrep uses modern invalid-UTF support and deprecated UTF-check toggles are no-op in its PCRE2 wrapper. Benchmark 15.2 behavior directly. [RG-PCRE2-MATCHER]

## 43.11 Anti-pattern: treat `--no-messages` as “no errors happened”

It suppresses output, not error state. Always inspect the exit code.

## 43.12 Anti-pattern: treating PCRE2 10.47 as a sandbox

PCRE2 10.47 contains the fix for the specific 10.45 scan-substring/`(*ACCEPT)` CVE, but version currency does not make PCRE2 a sandbox. General backtracking/recursion resource risk remains, so isolate and limit any service boundary that accepts untrusted patterns.

---

# 44) Upgrade guide: outdated ripgrep references → 15.2.0

## 44.0 Version anchor

Replace “latest docs/man page” language with a concrete target:

```text
ripgrep 15.2.0
release: 2026-07-15
commit: e89fff8
```

## 44.1 PCRE2 capability anchor

Do not just say “PCRE2 is supported.” Record:

```bash
rg --version
rg --pcre2-version
```

Upstream 15.2 release binaries may report PCRE2 10.45, while distro/local builds can link a newer system PCRE2 such as 10.47. Record the actual `rg --pcre2-version` result for the environment being automated.

## 44.2 Replace deprecated engine language

Preferred:

```bash
--engine=default
--engine=pcre2
--engine=auto
```

`--auto-hybrid-regex` remains compatibility language but is deprecated; use `--engine=auto` in new material. [RG-CHANGELOG]

## 44.3 Replace historical PCRE2-Unicode flags

Prefer:

```bash
--unicode
--no-unicode
```

rather than old `--pcre2-unicode` spellings.

## 44.4 Correct multiline-anchor mental model

In ripgrep 15.2 source, both matchers are compiled with multiline anchors enabled. Therefore do **not** teach:

> “When `-U` is enabled, you must add `(?m)` for `^`/`$` to mean line anchors.”

Instead teach:

```text
-U controls whether a match may cross lines.
^/$ are line-oriented under rg matcher configuration.
\A/\z are absolute subject anchors.
```

[RG-HIARGS]

## 44.5 Document actual JIT policy

A generic PCRE2 document may say JIT is optional. ripgrep 15.2 is more specific:

- 64-bit: `jit_if_available(true)`;
- custom max JIT stack: 10 MiB;
- 32-bit: ripgrep avoids JIT due to observed memory problems.

[RG-HIARGS]

## 44.6 Add the 10.45+ pattern surface and the 10.47 delta

A current reference should include the PCRE2 10.45 feature expansion that remains available in 10.47:

- UCD 16;
- case-insensitive Unicode property change;
- stricter `\x` parsing;
- scan-substring assertions;
- Perl-style extended character classes `(?[...])`;
- UTS#18 classes as compile-option-only for stock rg;
- improved class engine;
- JIT unsupported error semantics;
- inherited variable-length lookbehind from 10.43.

It should then add the 10.47-specific delta:

- recursion/subroutine calls that can return selected capture groups to the caller;
- AArch64 JIT SIMD code generation as an implicit performance improvement when the linked PCRE2/JIT path is actually used;
- the 10.46 security fix for CVE-2025-58050, inherited by 10.47;
- library/API/build additions such as `pcre2_next_match()`, `$+` support in `pcre2_substitute()`, effective-link-size introspection, symbol versioning and modernized CMake exports, while explicitly noting which are **not** ripgrep CLI features.

[PCRE2-1045-NEWS] [PCRE2-1046] [PCRE2-NEWS] [PCRE2-CHANGELOG]

## 44.7 Add 15.2 traversal/ignore changes

- huge-corpus traversal performance improvement;
- `GIT_CONFIG_GLOBAL` / `GIT_CONFIG_SYSTEM` support;
- multiple-directory ignore fixes;
- `.jj` no-ignore optimization;
- new `aarch64-unknown-linux-musl` release binary.

[RG-CHANGELOG]

## 44.8 Record the actual PCRE2 pairing

Do not infer the PCRE2 version from the ripgrep version or from the mere presence of a system `pcre2` package. A 15.2 binary can be bundled/static or dynamically linked. Gate version-specific patterns on `rg --pcre2-version`; this edition assumes that command reports 10.47.

---

# 45) CLI flag-family lookup matrix

This is a lookup map, not a replacement for `rg --help` on the deployed binary.

## 45.1 Engine and pattern language

| Goal | Flag / syntax | Notes |
|---|---|---|
| default Rust regex | `--engine=default` | preferred baseline |
| PCRE2 | `-P`, `--pcre2`, `--engine=pcre2` | optional build feature |
| automatic fallback | `--engine=auto` | default first, PCRE2 fallback |
| literal patterns | `-F`, `--fixed-strings` | applies to all patterns |
| disable Unicode | `--no-unicode` | affects both engines |
| force sensitive | `-s`, `--case-sensitive` | overrides case alternatives |
| ignore case | `-i`, `--ignore-case` | Unicode-aware unless disabled |
| smart case | `-S`, `--smart-case` | lowercase => insensitive |
| whole word | `-w`, `--word-regexp` | boundary wrapper |
| whole line | `-x`, `--line-regexp` | whole-line wrapper |
| multiline search | `-U`, `--multiline` | match may span lines |
| dotall convenience | `--multiline-dotall` | only relevant with `-U` |
| CRLF | `--crlf` | align line terminators |
| invert | `-v`, `--invert-match` | select nonmatching lines |

## 45.2 Pattern sources

| Goal | Flag |
|---|---|
| positional pattern | `rg PATTERN PATH...` |
| explicit pattern | `-e PATTERN`, repeatable |
| pattern file | `-f FILE`, repeatable |
| patterns from stdin | `-f -` |
| end option parsing | `--` |

All `-e`/`-f` patterns are ORed and use one engine.

## 45.3 Traversal and search space

| Goal | Flag |
|---|---|
| include hidden | `--hidden` |
| no ignores | `-u`, `--no-ignore` |
| no ignores + hidden | `-uu` |
| broad unrestricted | `-uuu` |
| follow symlinks | `-L`, `--follow` |
| recursion depth | `-d NUM`, `--max-depth NUM` |
| one filesystem | `--one-file-system` |
| max file size | `--max-filesize SIZE` |
| list candidate files | `--files` |
| worker threads | `-j NUM`, `--threads NUM` (`0` = heuristic) |
| deterministic order | `--sort path`, `--sortr path` (forces single-threaded) |
| sort by time | `--sort modified\|accessed\|created` |

## 45.4 Ignore controls

| Mechanism | Purpose |
|---|---|
| `--ignore-file FILE` | add gitignore-format rules |
| `--no-ignore-vcs` | disable VCS ignores |
| `--no-ignore-dot` | disable `.ignore`/`.rgignore` |
| `--no-ignore-parent` | skip parent ignore discovery |
| `--no-ignore-global` | disable global Git ignores |
| `--no-ignore-exclude` | disable repo excludes |
| `--no-ignore-files` | ignore explicit `--ignore-file` args |
| `--no-require-git` | apply VCS rules without repo marker requirement |
| `--no-ignore-messages` | suppress ignore parse messages |

## 45.5 Globs and types

| Goal | Flag |
|---|---|
| include/exclude glob | `-g GLOB`, `--glob GLOB` |
| insensitive glob | `--iglob GLOB` |
| all globs insensitive | `--glob-case-insensitive` |
| include type | `-t TYPE` |
| exclude type | `-T TYPE` |
| list types | `--type-list` |
| add type glob | `--type-add 'name:glob'` |
| clear type | `--type-clear TYPE` |

## 45.6 Binary/encoding/input

| Goal | Flag |
|---|---|
| treat binary as text | `-a`, `--text` |
| search binary mode | `--binary` |
| explicit encoding | `-E ENC`, `--encoding ENC` |
| raw bytes/no transcode | `-E none` |
| NUL record terminator | `--null-data` |
| compressed search | `-z`, `--search-zip` |
| preprocessor | `--pre CMD` |
| preprocessor filter | `--pre-glob GLOB` |
| disable mmap | `--no-mmap` |
| request mmap | `--mmap` |

## 45.7 Standard output

| Goal | Flag |
|---|---|
| line numbers | `-n`, `--line-number` |
| column | `--column` |
| byte offset | `-b`, `--byte-offset` |
| only matching | `-o`, `--only-matching` |
| before context | `-B NUM` |
| after context | `-A NUM` |
| both context | `-C NUM` |
| heading | `--heading` |
| no heading | `--no-heading` |
| path | `-H`, `--with-filename` |
| no path | `-I`, `--no-filename` |
| pretty | `-p`, `--pretty` |
| vimgrep | `--vimgrep` |
| replace output | `-r TEXT`, `--replace TEXT` |
| color policy | `--color=auto|always|never|ansi` |
| trim | `--trim` |
| line buffering | `--line-buffered` |
| block buffering | `--block-buffered` |
| cap matches per file | `-m NUM`, `--max-count NUM` |
| context block separator | `--context-separator SEP`, `--no-context-separator` |
| context field separator | `--field-context-separator SEP` |
| match field separator | `--field-match-separator SEP` |

## 45.8 Summary / existence modes

| Goal | Flag |
|---|---|
| count matching lines | `-c`, `--count` |
| count matches | `--count-matches` |
| files with matches | `-l`, `--files-with-matches` |
| files without match | `--files-without-match` |
| quiet/existence | `-q`, `--quiet` |
| stats | `--stats` |

## 45.9 Machine output / filename safety

| Goal | Flag |
|---|---|
| JSON Lines | `--json` |
| NUL after path | `-0`, `--null` |
| custom path separator | `--path-separator` |
| suppress selected IO errors | `--no-messages` |

## 45.10 Diagnostics

| Goal | Flag |
|---|---|
| version/build features | `--version` |
| PCRE2 version | `--pcre2-version` |
| debug search decisions | `--debug` |
| verbose trace | `--trace` |
| help | `--help` |

## 45.11 Default-engine-only tuning caveat

`--regex-size-limit` and `--dfa-size-limit` are consumed by the Rust matcher path in 15.2. They are not a substitute for PCRE2 match limits. [RG-HIARGS]

---

# 46) PCRE2 10.47 pattern-feature lookup matrix

| Feature | Syntax example | Works via `rg -P`? | Notes |
|---|---|---:|---|
| positive lookahead | `(?=...)` | yes | zero-width |
| negative lookahead | `(?!...)` | yes | zero-width |
| positive lookbehind | `(?<=...)` | yes | bounded rules apply |
| negative lookbehind | `(?<!...)` | yes | bounded rules apply |
| variable lookbehind | `(?<=.{0,20})` | yes | 10.43+, default max 255 |
| numeric backref | `\1` | yes | backtracking semantics |
| named backref | `\k<name>` | yes | prefer explicit form |
| named capture | `(?<name>...)` | yes | replacement uses `$name` separately |
| atomic group | `(?>...)` | yes | no backtrack inside |
| possessive quantifier | `++`, `*+`, `?+` | yes | no give-back |
| branch reset | `(?|...)` | yes | reused capture numbers |
| conditional | `(?(condition)yes|no)` | yes | many condition types |
| whole recursion | `(?R)` | yes | high resource risk |
| subroutine | `(?&name)` | yes | recursive/DRY patterns |
| returned subroutine captures | `(?&name(<cap>))` | **yes, 10.47** | selected captures survive return to caller |
| reset match start | `\K` | yes | useful for extraction |
| skip/fail | `(*SKIP)(*F)` | yes | exclusion idiom |
| commit/prune/then | `(*COMMIT)` etc | yes | backtracking control |
| accept/fail | `(*ACCEPT)`, `(*FAIL)` | yes | 10.45 CVE fixed in 10.46; 10.47 current posture in §40 |
| start no-JIT | `(*NO_JIT)` | yes | disables application JIT request |
| match limit | `(*LIMIT_MATCH=n)` | yes | can only lower caller limit |
| depth limit | `(*LIMIT_DEPTH=n)` | yes | ignored by JIT |
| heap limit | `(*LIMIT_HEAP=n)` | yes | interpreter heap, KiB |
| newline convention | `(*CRLF)` etc | yes | coordinate with `-U/--crlf` |
| BSR rule | `(*BSR_ANYCRLF)` | yes | controls `\R` |
| script run | `(*sr:...)` | yes | Unicode script consistency |
| extended class (Perl) | `(?[...])` | **yes, 10.45+** | preferred set algebra in rg |
| UTS#18 ordinary class | `[A&&[B]]` | **not by default** | requires API compile option not exposed by rg |
| scan substring | `(*scs:(1)...)` | **yes, 10.45+** | 10.47 includes the 10.46 CVE fix |
| callout API behavior | `(?C...)` | not useful as generic rg callback | rg does not expose custom callout handler |
| pcre2_substitute features | replacement syntax | no as rg feature | rg has its own replacement engine |
| `pcre2_next_match()` | C API | no as rg feature | ripgrep owns match iteration |
| AArch64 JIT SIMD | implementation/JIT | implicit | eligible `rg -P` patterns may benefit on arm64 |
| variable-lookbehind max setter | API | no CLI | rewrite pattern |
| extra compile options | API | no generic CLI | e.g. Turkish casing/Python octal |

## 46.1 Escapes worth knowing

```regex
\Q...\E       quote literal text inside regex
\A \z \Z      absolute/end anchors
\b \B         word boundary
\R            newline sequence
\N            non-newline/codepoint form depending context
\K            reset reported match start
\x{HHHH}      hex codepoint syntax in UTF patterns
\o{NNN}       explicit octal form
\g{name}      one of several group reference forms
```

PCRE2 10.45 made malformed/bare `\x` parsing stricter, and 10.47 retains that behavior: do not use a lone `\x` as a NUL shorthand; write `\x00`. [PCRE2-1045-NEWS]

## 46.2 Extended mode

Use scoped free-spacing for complex PCRE2:

```bash
rg -P '(?x)
  (?<name> [A-Za-z_] [A-Za-z0-9_]* )
  \s* \(
' src/
```

Shell newlines inside quoted strings are literal pattern newlines; if the pattern itself includes newline matching, remember `-U` requirements. For maintainable large patterns, a pattern file is often preferable.

---

# 47) Engine-selection and compatibility matrix

| Requirement | Default | PCRE2 10.47 | Recommendation |
|---|---:|---:|---|
| literal search | excellent | possible | `-F`, default |
| simple regular regex | excellent | yes | default |
| linear/predictable worst-case orientation | yes | no | default |
| lookahead | no | yes | PCRE2 |
| lookbehind | no | yes | PCRE2 |
| bounded variable lookbehind | no | yes | PCRE2 10.43+ |
| backreferences | no | yes | PCRE2 |
| recursion/subroutines | no | yes | PCRE2 |
| returned captures from subroutine/recursion | no | yes | PCRE2 10.47 |
| atomic/possessive | limited/different model | yes | PCRE2 if semantically needed |
| `(*SKIP)(*F)` | no | yes | PCRE2 |
| `\K` | no | yes | PCRE2 |
| Unicode classes | yes | yes | default unless PCRE2-only context needed |
| Unicode 16 PCRE2 properties | n/a/version of Rust Unicode tables | yes | record engine/version |
| Perl-style extended class `(?[...])` | no | yes | PCRE2 10.45+ |
| scan substring | no | yes | PCRE2 10.45+; 10.47 includes CVE fix |
| multiline cross-line | yes with `-U` | yes with `-U` | choose engine by syntax needs |
| JSON output | yes | yes | engine-independent printer |
| ignore/glob/type filtering | yes | yes | independent of engine |
| `rg --replace` | yes | yes | same printer layer |
| default `--regex-size-limit` | yes | not wired | not a PCRE2 guard |
| `--dfa-size-limit` | yes | not wired | not a PCRE2 guard |

## 47.1 Engine escalation algorithm for agents

```text
1. Is the request literal?
   -> -F / default
2. Can Rust regex express it cleanly?
   -> --engine=default
3. Does it specifically need lookaround/backrefs/PCRE2 verbs/etc.?
   -> verify PCRE2 version; --engine=pcre2
4. Is pattern untrusted?
   -> prefer default; otherwise require updated PCRE2 + isolation/limits
5. Is exact engine choice unimportant and input is trusted/interactive?
   -> --engine=auto is acceptable
```

---

# 48) Production / agent checklists

## 48.0 Installation/version checklist

```text
[ ] `rg --version` reports 15.2.0 (or deliberately chosen newer version).
[ ] PCRE2-dependent environment runs `rg --pcre2-version` successfully.
[ ] Exact PCRE2 version is recorded; do not infer from rg alone.
[ ] JIT availability is recorded where performance expectations depend on it.
[ ] Distro/local builds are tested separately from official release binaries.
```

## 48.1 Query-construction checklist

```text
[ ] Start with literal search when the target is literal.
[ ] Prefer default engine when it can express the query.
[ ] Set explicit PATH/root for filesystem searches.
[ ] Narrow file types/globs before expensive regexes.
[ ] Quote shell patterns safely.
[ ] Use -e for dash-leading/generated patterns.
[ ] Validate pattern files for accidental blank lines.
[ ] Use -U only when cross-line matching is needed.
[ ] Use \A/\z for absolute haystack anchors; ^/$ are line oriented in rg.
```

## 48.2 PCRE2 checklist

```text
[ ] Confirm required feature exists in the deployed PCRE2 version.
[ ] Gate returned-capture recursion on PCRE2 10.47.
[ ] For extended classes in rg, use Perl-style (?[...]); available since 10.45 and present in 10.47.
[ ] Do not assume PCRE2_ALT_EXTENDED_CLASS is enabled by rg.
[ ] Variable lookbehind is bounded; stock rg exposes no max-length setter.
[ ] Use backreferences only when equality to prior text is essential.
[ ] Add atomic/possessive constructs only with semantic justification/tests.
[ ] Consider (*LIMIT_MATCH=...) for expensive trusted patterns.
[ ] Remember ripgrep attempts JIT on 64-bit with a 10 MiB JIT stack.
[ ] A JIT-unsupported pattern may fall back to interpreted PCRE2.
[ ] `--regex-size-limit`/`--dfa-size-limit` are not PCRE2 safeguards.
[ ] `rg -r` uses ripgrep replacement syntax, not pcre2_substitute().
```

## 48.3 Untrusted-input checklist

```text
[ ] Prefer default regex engine for user-controlled patterns.
[ ] Do not treat PCRE2 10.47 as a resource sandbox for hostile patterns.
[ ] Verify the deployed PCRE2 version and keep it on a supported/patched release.
[ ] Apply wall-clock timeout to the rg subprocess.
[ ] Apply CPU/memory/process limits where applicable.
[ ] Constrain allowed roots.
[ ] Constrain file types/globs/max filesize.
[ ] Cap output bytes/results.
[ ] Disable arbitrary preprocessors.
[ ] Treat decompression as resource-expanding input.
[ ] Log exact rg + PCRE2 versions.
```

## 48.4 Evidence-quality checklist for coding agents

```text
[ ] Use `--json` when exact path/line/submatch offsets matter.
[ ] Preserve stderr/exit status when completeness matters.
[ ] Treat exit 1 as no-match, not search failure.
[ ] Do not claim semantic completeness from lexical grep alone.
[ ] Escalate candidate matches to parser/CPG/LSP/type evidence as needed.
[ ] Store the executed command/engine with evidence for reproducibility.
[ ] Record search-space filters (root, types, globs, ignores).
```

## 48.5 Performance checklist

```text
[ ] Avoid -P when default engine is enough.
[ ] Avoid -U when match cannot cross lines.
[ ] Preserve useful literals in patterns.
[ ] Avoid nested unbounded repeats.
[ ] Use -q/-l/counts when full lines are unnecessary.
[ ] Avoid sorting unless deterministic ordering is required.
[ ] Prefer search_path/file roots over preprocessors/readers when possible.
[ ] Benchmark PCRE2 patterns on representative near-miss inputs.
[ ] Narrow giant repositories before running expensive multiline PCRE2.
```

---

# 49) Source index

The reference deliberately favors **tag-pinned upstream source** for exact ripgrep behavior and PCRE2’s own 10.47 documentation for current pattern semantics, while retaining 10.45/10.46 release notes where feature-introduction or security chronology matters.

## Ripgrep 15.2.0

[RG-RELEASE]: https://github.com/BurntSushi/ripgrep/releases/tag/15.2.0 "ripgrep 15.2.0 release"
[RG-CHANGELOG]: https://github.com/BurntSushi/ripgrep/blob/15.2.0/CHANGELOG.md "ripgrep 15.2.0 changelog"
[RG-README]: https://github.com/BurntSushi/ripgrep/blob/15.2.0/README.md "ripgrep 15.2.0 README"
[RG-GUIDE]: https://github.com/BurntSushi/ripgrep/blob/15.2.0/GUIDE.md "ripgrep 15.2.0 Guide"
[RG-FAQ]: https://github.com/BurntSushi/ripgrep/blob/15.2.0/FAQ.md "ripgrep 15.2.0 FAQ"
[RG-FLAGS]: https://github.com/BurntSushi/ripgrep/blob/15.2.0/crates/core/flags/defs.rs "ripgrep 15.2.0 flag definitions"
[RG-HIARGS]: https://github.com/BurntSushi/ripgrep/blob/15.2.0/crates/core/flags/hiargs.rs "ripgrep 15.2.0 high-level CLI configuration"
[RG-PCRE2-MATCHER]: https://github.com/BurntSushi/ripgrep/blob/15.2.0/crates/pcre2/src/matcher.rs "ripgrep 15.2.0 PCRE2 matcher wrapper"

## PCRE2 10.47

[PCRE2-NEWS]: https://github.com/PCRE2Project/pcre2/blob/pcre2-10.47/NEWS "PCRE2 10.47 NEWS"
[PCRE2-CHANGELOG]: https://github.com/PCRE2Project/pcre2/blob/pcre2-10.47/ChangeLog "PCRE2 10.47 ChangeLog"
[PCRE2-PATTERN]: https://github.com/PCRE2Project/pcre2/blob/pcre2-10.47/doc/pcre2pattern.3 "PCRE2 10.47 pattern reference"
[PCRE2-SYNTAX]: https://github.com/PCRE2Project/pcre2/blob/pcre2-10.47/doc/pcre2syntax.3 "PCRE2 10.47 syntax quick reference"
[PCRE2-API]: https://github.com/PCRE2Project/pcre2/blob/pcre2-10.47/doc/pcre2api.3 "PCRE2 10.47 API reference"
[PCRE2-JIT]: https://github.com/PCRE2Project/pcre2/blob/pcre2-10.47/doc/pcre2jit.3 "PCRE2 10.47 JIT reference"

## PCRE2 historical release anchors

[PCRE2-1045-NEWS]: https://github.com/PCRE2Project/pcre2/blob/pcre2-10.45/NEWS "PCRE2 10.45 NEWS — feature introduction chronology"
[PCRE2-1046]: https://github.com/PCRE2Project/pcre2/releases/tag/pcre2-10.46 "PCRE2 10.46 security release / CVE-2025-58050"

---

# Final agent invariants

**Invariant 1 — search-space policy and regex policy are independent.** Choosing PCRE2 does not bypass ignores/globs/types/hidden/binary behavior.

**Invariant 2 — use the weakest regex language that cleanly expresses the information need.** Literal → default regex → PCRE2 is the preferred escalation order.

**Invariant 3 — `-U` is a searcher/haystack permission, not the PCRE2 multiline-anchor switch.** In ripgrep 15.2, PCRE2 multiline anchors are enabled by the matcher builder; use `\A`/`\z` for absolute subject boundaries.

**Invariant 4 — on 64-bit ripgrep 15.2, PCRE2 JIT is attempted when available and the custom max JIT stack is 10 MiB.** “JIT available” does not mean every pattern runs JIT.

**Invariant 5 — ripgrep’s default regex size/DFA limits are not PCRE2 match limits.** The actual source wires them into the Rust matcher only.

**Invariant 6 — PCRE2 10.47 retains the 10.45 extended set algebra forms, but stock ripgrep can rely on the self-contained Perl-style `(?[...])` form, not the API-gated UTS#18 ordinary-class mode.**

**Invariant 7 — variable-length lookbehind is available in 10.47 but bounded.** The default maximum is 255 and ripgrep exposes no setter; use `\K`, captures or pattern restructuring when needed.

**Invariant 8 — scan-substring is real and usable through `rg -P`; PCRE2 10.47 includes the 10.46 fix for the 10.45 `(*ACCEPT)` + scan-substring memory-safety issue.** It can still be computationally expensive, so hostile patterns require general resource isolation.

**Invariant 9 — PCRE2 matching and ripgrep replacement are separate layers.** PCRE2 10.45–10.47 `pcre2_substitute()` features, including 10.47 `$+`, are not automatically `rg --replace` features.

**Invariant 10 — PCRE2 10.47 returned-capture recursion is a real `rg -P` pattern capability.** Use `(?&name(grouplist))` / related forms when reusable regex subroutines must retain selected captured values; conservatively gate this on PCRE2 10.47.

**Invariant 11 — PCRE2 10.47 adds AArch64 JIT SIMD and several useful C APIs, but most of those are not new ripgrep CLI surfaces.** Separate pattern syntax from library API/build features.

**Invariant 12 — ripgrep is a lexical evidence engine, not a semantic code graph.** Use it aggressively for direct evidence, candidate retrieval and exact text relationships; pair it with syntax/type/CPG tools for semantic claims.

