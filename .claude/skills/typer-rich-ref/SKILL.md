---
name: typer-rich-ref
description: "INACTIVE in this repository — both target documents are missing. Routes `docs/library_ref/typer.md` and `docs/library_ref/rich.md`, neither of which exists here; Typer and Rich are not declared dependencies of CodeFabric and appear zero times in the design suite. Invoke only to read the notice explaining that, or if those two references are later written. CodeFabric's administrative CLI (`RM W2` work package 8) is a Rust daemon-side surface with no library chosen; the Python FastMCP adapter is STDIO-only and `SRV §6` invariant 11 keeps STDOUT protocol-only, which precludes a Rich Console there. For references that do exist, see `docs/spec_index/library-routing.md`."
allowed-tools: Read, Grep, Glob, Bash
---

# Typer + Rich Reference Navigator

> **Not usable in this repository — both target documents are absent.** This skill routes
> `docs/library_ref/typer.md` and `docs/library_ref/rich.md`; neither exists here, so every
> section reference below resolves to nothing. It was authored for a different project and has
> not been re-grounded on CodeFabric. See *Project context* at the end before relying on any of
> it.

## Version anchor

All guidance assumes:

* **Typer 0.25.0** (released 2026-04-26), Python ≥ 3.10. Transitive deps `click>=8.2.1`, `shellingham>=1.3.0`, `rich>=13.8.0`, `annotated-doc>=0.0.2`. Pin Typer **at the application boundary**, not just dev-time, because behavior can shift when Click/Rich/Shellingham move.
* **Rich 15.0.0** (released 2026-04-12, "So Long 3.8 Release"), Python ≥ 3.9. ReadTheDocs still labels itself "Rich 14.1.0" so feature exposition there can lag behind PyPI metadata. Treat **PyPI + GitHub release notes as authoritative for runtime compatibility**, ReadTheDocs as authoritative for API/feature exposition.
* Stack contract: `Typer.Typer → typer.main.get_command → click.Command/Group → Click runtime`, with Rich consumed for help-panel rendering, traceback formatting, prompts, and any project-defined renderables on the `Console` substrate.

If a doc snippet references a different version, verify against the stack above before adopting.

---

## How the two reference documents are organized

| Doc | Path | Lines | Top-level structure | Authoritative scope |
|-----|------|-------|---------------------|---------------------|
| **typer** | `docs/library_ref/typer.md` | ~32,600 | 28 numbered sections (§0, §2-§26, §28-§29; §1 and §27 omitted from deep-dive). No upfront catalog — file opens directly with §0. Each section has internal §N.M subsections, plus dense `## N.K` recipe blocks, anti-pattern inventories, decision matrices, and "agent checklist" closers. | Full Typer surface from a deferred-Click-generator stance: `Typer()` vs `typer.run()`, command/callback/context model, Annotated parameter metadata, type conversions, prompts/env/validation, subcommand trees, help/markup/completion, stdout/stderr/Rich integration, progress, exit/abort, file/path types, packaging, testing with `CliRunner`, large-app layout, advanced UX bundles |
| **rich** | `docs/library_ref/rich.md` | ~33,300 | Upfront feature-category **catalog** (lines 5-364) listing 28 sections, then **deep-dive expansions** for §0, §2-§17, §19-§28 (§1 and §18 deep-dives missing — see catalog rows for high-level guidance). Section 17 has an editorial duplicate at line 19454; ignore the duplicate. Each deep-dive carries §N.M subsections, recipes, anti-patterns, and an agent checklist. §28 is a syntax cookbook with §28.1-§28.12 reference tables. | Full Rich surface as **terminal rendering substrate**: Console construction/capability probes, style/theme system, markup, Text spans, highlighters, pretty/inspect, logging handler, traceback, prompts, tables, panels/columns/render-groups, Markdown, syntax, progress, status, tree, layout, console protocol, export/record, Jupyter, deterministic testing, accessibility/degradation, CLI integration, performance, security, ready-to-paste recipes |

**Reading strategy.** Find the right section in the indexes below, then read with `Read(offset=N, limit=M)`. Sections are 150-450 lines; subsections are 30-80 lines. For cross-cutting concerns, use the Unified Concern Cross-Reference. For agent recipes, both docs surface "agent checklist" / "agent implementation rules" subsections at the end of each section.

---

## typer.md — section index

| § | Line | Title | Key subsections / agent value |
|---|------|-------|-------------------------------|
| **0** | 3 | Scope, versioning, Typer mental model | §0.1 version anchor, §0.3 deferred-Click-generator model, §0.4 Click object identity, §0.5 execution decomposition, §0.7 constructor knobs, §0.8 signature→CLI schema, §0.9 Annotated control plane, §0.11 Click interop, §0.12 agent-safe baseline, §0.13 LLM correctness checklist, §0.14 distilled invariants |
| **2** | 520 | Two front doors: `typer.run` vs `Typer()` | §2.0 selection invariant, §2.5 hidden cost of `run()`, §2.7 explicit `Typer()`, §2.9 production defaults, §2.13 migration recipe, §2.14 testing implications, §2.16 anti-patterns, §2.17 decision matrix, §2.18 agent rules, §2.19 deployment template |
| **3** | 1223 | Core application object: `typer.Typer(...)` | §3.1 high-signal constructor subset, §3.5 callback patterns, §3.6 `invoke_without_command`, §3.7 `no_args_is_help`, §3.8 `context_settings`, §3.10 `rich_markup_mode`, §3.11 `rich_help_panel`, §3.12 `suggest_commands`, §3.13 pretty exception settings, §3.14 Command vs Group generation, §3.16 UX policy bundle, §3.18 root callback with state, §3.19 deployment template, §3.20 failure modes |
| **4** | 2171 | Commands and command registration | §4.2 registration syntax, §4.4 default name derivation, §4.6 one-vs-multi command grammar, §4.7 grammar break recipe, §4.8 docstring help, §4.10 short_help, §4.12 hidden, §4.13 deprecated, §4.14 rename migration, §4.16 rich help panels, §4.17 command-level context_settings, §4.21 thin-wrapper pattern, §4.23 production template, §4.24 anti-patterns |
| **5** | 3159 | App callbacks and global options — `@app.callback()` | §5.1 minimal global option, §5.4 top-level-only scope, §5.8 `typer.Context` in callbacks, §5.9 conditional via `invoked_subcommand`, §5.10 `invoke_without_command=True`, §5.13 `ctx.obj` shared state, §5.15 logging config, §5.16 config loading, §5.17 auth init, §5.18 root version flag, §5.19 app vs command-level option placement, §5.22 nested apps, §5.24 testing, §5.26 anti-patterns |
| **6** | 4309 | Context, state, invocation metadata | §6.1 minimal injection, §6.2 injection locations, §6.3 field map, §6.4 `invoked_subcommand` discriminator, §6.5-§6.8 state design + accessor, §6.9 nested context chain, §6.10-§6.11 context_settings, §6.13 `resilient_parsing` for completion safety, §6.14 `CallbackParam`, §6.15 `ctx.params`, §6.18 custom help flags, §6.19 `ctx.args` vs declared `list[str]`, §6.21 app factory pattern |
| **7** | 5386 | CLI arguments — `typer.Argument(...)` | §7.2 Annotated syntax (preferred), §7.3 required vs optional, §7.4 requiredness ≠ Optional, §7.5 legacy default-value syntax, §7.6 reference subset, §7.10-§7.11 envvar fallback, §7.13-§7.14 callback validation, §7.15-§7.16 default_factory, §7.23 Path validations, §7.25 custom parser, §7.28 variadic positional, §7.29 args vs options decision table, §7.30 production template |
| **8** | 6764 | CLI options — `typer.Option(...)` | §8.4 Annotated syntax, §8.5 reference subset, §8.6 name derivation, §8.7-§8.11 long/short/alias rules, §8.12-§8.15 required/prompt forms, §8.18 metavar, §8.19 rich_help_panel, §8.20-§8.21 envvar, §8.22 callback validation, §8.23 eager option (version), §8.26-§8.27 numeric/path validation, §8.28 placement (callback vs command) |
| **9** | 8279 | Modern metadata style: `Annotated` vs legacy default metadata | When to use each, migration rules, hotfix exception (`# minimal-diff hotfix in legacy code`), house-style mixing rules |
| **10** | 9511 | Type system and built-in conversions | str/int/float/bool/Path/datetime/UUID/Enum/Choice handling, conversion failure messages, custom Click types |
| **11** | 10815 | Boolean flags and flag naming | `--admin/--no-admin` pair behavior, single-flag mode, custom flag-name overrides, count flags |
| **12** | 11957 | Multiple values, lists, and tuples | Repeated `--user`, fixed-arity tuples, variadic positional `list[str]`, error messages on arity mismatch |
| **13** | 13295 | Prompts, confirmation, passwords | `prompt=True`, `confirmation_prompt=True`, `hide_input=True`, default-prompt patterns |
| **14** | 14348 | Environment variables and configuration inputs | `envvar=`, multi-name fallback, `show_envvar`, env var precedence |
| **15** | 15674 | Validation, callbacks, parameter post-processing | Per-parameter callbacks, cross-parameter validation, callback signatures with `ctx`/`param` |
| **16** | 17032 | Subcommands and nested command groups — `app.add_typer(...)` | Nested mounting, multi-file layout, callback inheritance, `no_args_is_help` for groups, group context discipline |
| **17** | 18337 | Help system, Rich help panels, markup, command UX | `rich_markup_mode="rich"`/`"markdown"`, panel grouping, help formatting, suggest_commands |
| **18** | 19371 | Shell completion and value autocompletion | `--install-completion`, custom autocompleters, `shell_complete` callable, completion-safe side effects |
| **19** | 20503 | Output, colors, Rich integration, stdout/stderr | `typer.echo` vs `typer.secho` vs Rich Console, stream selection, JSON-to-stdout discipline (warnings to stderr), color control envs |
| **20** | 21534 | Progress bars, spinners, long-running tasks | `typer.progressbar`, integration with `rich.progress`, status spinners |
| **21** | 22528 | Termination, errors, aborts, exit codes | `typer.Exit(code=...)`, `typer.Abort`, exit code conventions, BadParameter, exception conversion |
| **22** | 23724 | File objects, paths, filesystem-heavy CLIs | `Path`, `typer.FileText`, file open modes, lazy file types, atomic writes |
| **23** | 25026 | Launching external applications — `typer.launch(...)` | Open URLs, files, browser; cross-platform behavior |
| **24** | 26068 | Packaging, deployment, executable entry points | `pyproject.toml` `[project.scripts]`, `__main__.py`, Hatchling/setuptools/uv-build, isolation via `uv tool install` |
| **25** | 27457 | `typer` command for scripts and dev workflows | `typer my_script.py run`, `typer ... utils docs`, dev-tool packaging |
| **26** | 28517 | Testing and QA — `typer.testing.CliRunner` | `CliRunner.invoke`, output capture, env injection, exit-code assertions, fixture patterns |
| **28** | 29955 | Large-app architecture and modular project layout | `commands/<group>/app.py` + `<group>/<cmd>.py` layout, `app_factory` pattern, context propagation across groups, root assembly, testing strategy |
| **29** | 31749 | Advanced UX and operational best practices | Verbosity levels, structured logging integration, JSON output mode, observability, deployment recipes |

§1 (a sequel-to-§0 onboarding section) and §27 (a sequel-to-§26 advanced testing section) are not present as deep-dives — §0 and §26 absorb their material.

---

## rich.md — section index

The file opens with a feature-category catalog (lines **5-364**) — read it first when you need a one-page map of the whole Rich surface. Deep-dive expansions follow.

| § | Line | Title | Key subsections / agent value |
|---|------|-------|-------------------------------|
| **catalog** | 5-364 | Feature-category catalog (all 28 sections at high level) | Use as cheat-sheet when picking which deep-dive to load. §1 (Installation) and §18 (Live display) live **only** in the catalog — they have no deep-dive expansion. |
| **0** | 366 | Scope, versioning, mental model | §0.0 version anchors, §0.1 capability surface, §0.2 Console + renderables + styles + capabilities primitive model, §0.3 renderable-first architecture, §0.4 strings vs `Text`, §0.5 style strings/`Style`/themes, §0.6 terminal detection/degradation, §0.7 stdout/stderr/file/capture/export channels, §0.8 Console Protocol overview, §0.9 built-in renderable map, §0.10 LLM-deployment considerations, §0.13 immediate operational heuristics |
| **2** | 1235 | Core Console API | §2.1 `Console()` construction, §2.2 global console patterns, §2.3 capability probes, §2.4 `print()` primary render, §2.5 width/wrap/align/overflow/crop, §2.6 `log()`, §2.8 `rule()`, §2.9 `input()`, §2.10 `status()`, §2.11 JSON APIs, §2.12 stdout vs stderr, §2.13 file-backed deterministic width, §2.14 `capture()`, §2.15 paging, §2.16 alternate screen, §2.17 detection/interactivity, §2.18 env vars (`NO_COLOR`, `FORCE_COLOR`, `TERM`), §2.19 method selection matrix |
| **3** | 2465 | Styling system | §3.1 style grammar, §3.2 color decision matrix, §3.3 attributes, §3.4 hyperlinks, §3.5 markup vs `style=`, §3.6 `Style` API, §3.8 themes, §3.9 overriding defaults, §3.10 loading themes from config, §3.11 style taxonomy for agent apps, §3.12 console-wide vs theme vs per-renderable, §3.14 deterministic style behavior |
| **4** | 3418 | Console markup | `[bold red]...[/]` grammar, escaping, `Text.from_markup`, `MarkupError`, security around untrusted markup |
| **5** | 4311 | Text object model | `Text()` construction, span model, `append`/`stylize`/`highlight_words`, `Text.assemble`, span overlap rules, immutability vs mutation patterns |
| **6** | 5560 | Highlighting and highlighters | `ReprHighlighter`, `JSONHighlighter`, `RegexHighlighter`, custom highlighter authoring, catastrophic backtracking warnings, when to disable for parser-facing output |
| **7** | 6592 | Pretty printing and object introspection | `pprint`, `inspect`, REPL hook (`~/.pythonrc.py`), max_length/max_depth, dataclass/attrs handling |
| **8** | 7801 | Logging integration | `RichHandler`, install patterns, level/path/markup/show_time, JSON-to-stdout interaction, double-handler pitfalls, library vs app stance |
| **9** | 9080 | Tracebacks and exception UX | `rich.traceback.install`, `Traceback.from_exception`, sitecustomize integration, library safety, redaction |
| **10** | 10181 | Prompts and interactive input | `Prompt.ask`, `IntPrompt`, `Confirm`, default/choices/case-sensitive, prompt-during-Live discipline (stop Live before prompting) |
| **11** | 11228 | Tables and grids | `Table` construction, columns/rows, padding/borders, `Grid`, alignment, sort, footer, expand/min_width, justified columns, automation/CI anti-patterns |
| **12** | 12482 | Panels, padding, alignment, columns, render groups | `Panel`, `Padding`, `Align`, `Columns`, `Group`, composition rules, terminal-width sensitivity |
| **13** | 13630 | Markdown rendering | `Markdown(...)`, code-theme selection, terminal hyperlink toggling, headings/lists/blockquotes, dynamic-record anti-patterns |
| **14** | 14586 | Syntax highlighting | `Syntax(...)`, theme selection, line numbers, dedent, code highlighters, terminal-color caveats |
| **15** | 15930 | Progress bars and task tracking | `Progress` columns, total/completed semantics, transient progress, multiple tasks, integration with iteration helpers, parent/child progress (Queue pattern), nested Progress in Live |
| **16** | 17331 | Status and spinners | `console.status()`, spinner names, CI vs interactive guidance, when to prefer durable phase logs |
| **17** | 18265 | Tree rendering | `Tree`, `add()`, guide_style, expanded/hidden root, file/dependency/command/AST/config patterns, depth control, cycles/DAGs limits, Tree+Panel/Columns/Table integration. **Note**: lines 19454-20642 are a duplicate of §17; ignore them. |
| **18** | (catalog only, 209-232) | Live display | High-level guidance only. For Progress in Live see §15; for Layout in Live see §19. |
| **19** | 20643 | Layout and full-screen terminal compositions | `Layout`, splits, full-screen apps, alternate screen, Layout in Live, refresh discipline |
| **20** | 21867 | Console protocol and custom renderables | `__rich_console__`, `__rich_measure__`, `RenderResult`, `Measurement`, custom renderable authoring, untrusted-markup-injection guard |
| **21** | 23321 | Exporting, recording, documentation artifacts | `record=True`, `export_text`/`export_html`/`export_svg`, deterministic snapshots, theme embedding |
| **22** | 24523 | Jupyter, IPython, REPL, notebooks | Jupyter detection, ipython_config, repeated extension-load discipline, notebook-specific renderable behavior |
| **23** | 25570 | Testing Rich output | Deterministic Console (file-backed, fixed width, `force_terminal=False`/`True`), capture vs export, snapshot patterns, animation-frame anti-patterns |
| **24** | 27037 | Accessibility, portability, degradation | `NO_COLOR`, monochrome, narrow terminals, missing terminfo, fallback unicode, dumb terminals |
| **25** | 28327 | Integration patterns with CLI apps | Click/Typer integration, command vs library-mode Console, error reporting, structured output discipline |
| **26** | 29674 | Performance and large-output behavior | Repeated markup parsing, immutable-state rendering, expensive renders in Live (60Hz pitfalls), huge files, console reuse |
| **27** | 30900 | Security and robustness | Untrusted markup, hyperlink injection, output that may be parsed downstream, console-as-attack-surface |
| **28** | 32169 | Reference appendix and syntax cookbook | §28.1 Console constructor options, §28.2 method cheat sheet, §28.3 style grammar, §28.4 markup syntax, §28.5 renderables/import paths, §28.6 Table option matrix, §28.7 Progress column matrix, §28.8 Live/Layout option matrix, §28.9 Traceback/logging option matrix, §28.10 common recipes, §28.11 final checklist, §28.12 one-page import block |

§1 (Installation/runtime/deployment) and §18 (Live display) deep-dives are absent. Use the catalog rows at lines 30-43 (§1) and 209-232 (§18). For Live display orchestration in practice, the working material lives in §15 (Progress in Live), §19 (Layout in Live), and §28.8 (option matrix).

---

## Unified Concern Cross-Reference

| Concern | typer.md | rich.md |
|---------|----------|---------|
| **Mental model / scope / version** | §0 | §0 (+ catalog 5-364) |
| **App / Console construction** | §2, §3 | §2.1-§2.3 |
| **Help text and panels** | §3.10-§3.11, §4.16, §17 | §3.5 (markup vs style), §28.4 |
| **Argument / option declaration** | §7, §8 | -- |
| **Annotated metadata style** | §9 | -- |
| **Type conversions** | §10, §11, §12 | -- |
| **Prompts and confirmation** | §13 | §10 |
| **Environment variables** | §14, §3.8 (context_settings) | §2.18 (NO_COLOR/FORCE_COLOR/TERM) |
| **Validation and callbacks** | §15, §7.13-§7.14, §8.22 | -- |
| **Subcommands / groups** | §16, §28 | -- |
| **Stdout vs stderr discipline** | §19, §29 | §2.12, §2.13, §8 (logging) |
| **Console output / styling / markup** | §17, §19 | §0.4-§0.7, §3, §4, §5 |
| **Tables / panels / layouts** | -- | §11, §12, §19 |
| **Progress bars and spinners** | §20 | §15, §16 |
| **Live displays** | -- | §15, §19, §28.8 (catalog 209-232 for §18) |
| **Tracebacks / exception UX** | §3.13, §21 | §9 |
| **Logging integration** | §29 | §8 |
| **Exit codes / abort** | §21 | -- |
| **File / path types** | §22 | -- |
| **External launch** | §23 | -- |
| **Shell completion** | §0.10, §18 | -- |
| **Packaging / entry points** | §0.10, §2.11, §24 | -- |
| **Testing** | §2.14, §5.24, §6.22, §26 | §23 |
| **Capture / export / record** | §26 (CliRunner output) | §2.14, §21 |
| **Large-app architecture** | §28 | §0.10 (deployment), §25 (CLI integration patterns) |
| **Performance / startup cost** | §0.7 (constructor), §24 (cold-start) | §26 |
| **Security / untrusted input** | §15 (validation), §22 (path) | §4 (markup), §27 |
| **Accessibility / degradation** | §19 (color control envs) | §0.6, §24 |
| **Custom renderables** | -- | §20, §0.8 |
| **Theming** | -- | §3.8-§3.10 |
| **Reference cheat sheets** | §0.14, §3.21, §4.25, … (per-section "agent checklist") | §28 (entire syntax cookbook) |

---

## Decision Trees

### Tree 1: Choose between `typer.run()` and `typer.Typer()`

```
Single one-shot script, no roadmap for subcommands?
  -> typer.run(function) (typer §2.1-§2.3)
Need any of: subcommands, callback, packaging entry point, testing surface, completion?
  -> typer.Typer() with explicit production defaults (typer §2.7, §2.9, §2.19)
Migrating an existing typer.run() to multi-command?
  -> follow §2.13 migration recipe; do NOT bolt subcommands onto run()
```

### Tree 2: Place a parameter

```
Required positional, named in usage line?
  -> typer.Argument(...) (typer §7)
Named flag with -- or -? Default makes it optional?
  -> typer.Option(...) (typer §8)
Boolean toggle?
  -> bool default + Option (typer §11)
Multiple values?
  Variable count?
    -> list[str] / List[T] (typer §12)
  Fixed count?
    -> tuple[T1, T2, T3] (typer §12)
Path / file?
  -> typer.Argument/Option with Path + exists/file_okay/dir_okay/readable/writable (typer §7.23, §8.27, §22)
Want Annotated[...] metadata or default-value metadata?
  -> Annotated is canonical; default-value form only for §9 hotfix exception
```

### Tree 3: Declare global state and cross-command context

```
Need global option visible at root?
  -> @app.callback() with parameter (typer §5.1, §5.4)
Need shared state (config, auth, logging) across all subcommands?
  -> @app.callback(invoke_without_command=False) populates ctx.obj (typer §5.13, §6.5)
Need to run something only when no subcommand was given?
  -> @app.callback(invoke_without_command=True) + ctx.invoked_subcommand check (typer §5.10, §3.6, §6.4)
Need Click-level parser tweak (eg. allow_extra_args)?
  -> context_settings={...} on Typer or command (typer §3.8, §6.10-§6.11)
Need completion-safe side effects in the callback?
  -> guard on ctx.resilient_parsing (typer §6.13)
```

### Tree 4: Render output in a CLI command

```
Plain narration to user?
  -> typer.echo / typer.secho for trivial text (typer §19)
  -> rich.console.Console().print(...) when styling, alignment, or width handling matters
Structured data on stdout, narration must not contaminate?
  -> JSON/CSV to stdout via plain print or Console(file=stdout)
  -> warnings/progress to stderr via Console(stderr=True) (rich §2.12, typer §19, §29)
Status while waiting (no progress quantum)?
  -> console.status(...) (rich §2.10, §16)
Quantifiable progress?
  -> rich.progress.Progress with appropriate column set (rich §15, §28.7)
Multi-region full-screen UI?
  -> rich.layout.Layout in rich.live.Live (rich §19, §28.8)
Tabular result?
  -> rich.table.Table (rich §11)
Hierarchical structure (file tree, plan, dependency)?
  -> rich.tree.Tree (rich §17)
Multi-renderable container?
  -> rich.panel.Panel / Columns / Group (rich §12)
Code listing?
  -> rich.syntax.Syntax (rich §14)
Markdown body / docstring rendering?
  -> rich.markdown.Markdown (rich §13)
```

### Tree 5: Choose a `Console` instance

```
One module-level singleton for the whole CLI?
  -> Console() in app/console.py, imported everywhere (rich §2.2, §0.10)
Need to write to stderr (logs, warnings, narration alongside JSON-to-stdout)?
  -> Console(stderr=True) instance (rich §2.12, §0.7)
CI / non-TTY / deterministic test output?
  -> Console(file=..., force_terminal=False, width=N, color_system=None) (rich §2.13, §23)
Want HTML/SVG export for docs?
  -> Console(record=True), then export_html/export_svg (rich §21)
Inside Jupyter?
  -> default Console; Rich detects and adapts (rich §22)
Library code (not the app entrypoint)?
  -> accept a Console parameter; do NOT install global handlers (rich §8 logging, §9 traceback, §0.10)
```

### Tree 6: Style and theme policy

```
One-off color on a single string?
  -> markup: `[bold red]X[/]` (rich §4)
Reusable styled span needing precise length / programmatic spans?
  -> Text("X", style="bold red") + .append/.stylize (rich §5)
Application-wide named styles ("error", "warn", "ok")?
  -> Theme(...) attached to Console (rich §3.8, §3.11)
Override Rich defaults (eg. logging.level.error)?
  -> Theme with same key names overriding defaults (rich §3.9)
User-overridable theme (config file)?
  -> load Theme from .ini / .toml at startup (rich §3.10)
Hyperlinks in styles?
  -> "link https://..." style; check terminal support (rich §3.4)
```

### Tree 7: Test a Typer + Rich CLI

```
Behavior test (exit code, output strings, ctx state)?
  -> typer.testing.CliRunner.invoke(app, [...]) (typer §26)
Need stable Rich output (no ANSI / fixed width)?
  -> CliRunner(mix_stderr=False, env={"NO_COLOR": "1", "TERM": "dumb"}) and/or
  -> Console(file=StringIO(), force_terminal=False, width=80) (rich §23, typer §26)
Need to assert structured logs separately from narrative output?
  -> dedicated Console(stderr=True) for logs; capture stdout independently (rich §2.12, §8)
Need to test snapshots of styled output?
  -> Console(record=True) + export_text("plain") or export_html (rich §21, §23)
Need to inject env vars / config?
  -> CliRunner.invoke(... env={...}) (typer §26)
Need to test a callback or context-state factory in isolation?
  -> import the function directly; do NOT round-trip through CliRunner (typer §5.24, §6.22)
```

### Tree 8: Exit / abort / error

```
Normal failure with a specific exit code?
  -> raise typer.Exit(code=N) (typer §21)
User-cancellation semantics?
  -> raise typer.Abort() (prints "Aborted!", exits non-zero) (typer §21)
Bad CLI argument value?
  -> raise typer.BadParameter(...) (typer §15, §21)
Exception you want to render with Rich's traceback in dev, plain in prod?
  -> Typer's pretty_exceptions_enable / pretty_exceptions_show_locals (typer §3.13)
Library code raising an unhandled exception?
  -> let it propagate; install rich.traceback only in app entrypoint, never in library (rich §9, §0.10)
```

---

## Operating Rules

1. **Typer is a deferred Click generator. The decorated function is unchanged.** `@app.command()` registers metadata; calling `app()` builds a Click `Command`/`Group` and runs it. Do not assume the function has been wrapped or replaced. (typer §0.3, §0.4, §0.14)

2. **Default to `typer.Typer()`, not `typer.run()`.** `typer.run()` is convenient for one-shot scripts but has no stable app object — it precludes subcommands, packaging entry points, and ergonomic testing. Reach for `typer.run()` only when there will never be a second command. (typer §2.0, §2.5, §2.16)

3. **Use Annotated-style parameter metadata.** `name: Annotated[str, typer.Argument(help="...")]` is the modern, canonical control plane. Default-value metadata (`name: str = typer.Argument(...)`) is allowed only as a §9 hotfix exception in legacy code. Mixing styles within a project is the highest-risk authoring error. (typer §0.9, §7.2, §8.4, §9)

4. **`Optional[T]` does not control requiredness; the default does.** A parameter is required iff it has no default. Type `Optional[T]` only signals that `None` is a valid value, not that the CLI parameter is optional. (typer §7.3, §7.4)

5. **Stdout is for the program's primary output; everything else goes to stderr.** Status, progress, narration, warnings, and logs go to a `Console(stderr=True)` (or `typer.echo(..., err=True)`). JSON/data on stdout must not be contaminated by Rich animations. Pin this discipline in callback initialization and use separate Console instances. (typer §19, §29, rich §2.12, §2.13, §8)

6. **One `Console` per role: a module-level singleton for narrative output, plus a stderr Console.** Do not construct ad-hoc Consoles inside business logic. Centralize Console construction in `app/console.py` (or equivalent) and import from there. Library code accepts a Console parameter; it does not construct its own. (rich §0.10, §2.2)

7. **Install `rich.traceback` and `RichHandler` only at the app entrypoint, never in library code.** Globals installed by libraries leak across processes and tests. Typer's `pretty_exceptions_enable` is the supported app-level hook. (typer §3.13, rich §8, §9, §0.10)

8. **Stop `Live` / `Progress` / `status` before prompting or before raw `input`.** Prompting through an active Live region produces interleaved output. Use `progress.stop()` / `live.stop()` / context exit before interactive input. (rich §10, §15, §19)

9. **Markup is unsafe with untrusted input.** `console.print(f"[bold]{user_input}[/]")` lets an attacker close/reopen markup tags. Use `console.print(user_text, style="bold")` or `Text(user_text, style="bold")`. Same caution for hyperlink markup. (rich §4, §27, §20)

10. **Make Rich tests deterministic by pinning the Console.** `Console(file=StringIO(), force_terminal=False, color_system=None, width=80)` plus `CliRunner(env={"NO_COLOR": "1", "TERM": "dumb"})` produces snapshot-stable output. Do not assert against ANSI bytes from a real terminal. (rich §23, typer §26)

11. **Place global options on the `@app.callback()`, not on every command.** Global state (`--verbose`, `--config`, `--profile`) belongs on the root callback, populated into `ctx.obj`, and consumed via `ctx.obj` in subcommands. Do not duplicate these options on each command. (typer §5.4, §5.13, §5.19, §6.5)

12. **Preserve Click interop.** Typer commands compile to Click commands via `typer.main.get_command(app)`. Drop into Click for parser-level escape hatches (`context_settings={"allow_extra_args": True, ...}`) rather than reimplementing parsing. (typer §0.11, §3.8, §6.10)

13. **Pin Typer at the application boundary, not just dev-time.** Click, Rich, and Shellingham are transitive runtime dependencies whose updates can change CLI behavior. `typer>=0.25.0,<0.26` in `[project].dependencies`, not just `[dev-dependencies]`. (typer §0.1)

14. **Each `Typer()` builds a Click `Command` for one command and a Click `Group` for >1 / callback / sub-Typer.** Account for this when packaging, generating help, or doing structural diffs of CLI surfaces. Force group generation when needed (`typer §3.15`). (typer §0.4, §3.14, §3.15, §4.6)

15. **Treat the catalog at rich.md lines 5-364 as authoritative for §1 and §18 navigation.** The deep-dives skip §1 (Installation) and §18 (Live display); the catalog rows there are the only first-party agent guidance for those topics, supplemented by §15 (Progress in Live), §19 (Layout in Live), and §28.8 (Live/Layout option matrix).

---

## Project context: CodeFabric — this skill does not apply

**CodeFabric has no Typer or Rich CLI, and neither library appears anywhere in the design
suite.** Three independent facts:

* **The reference documents do not exist.** `docs/library_ref/typer.md` and
  `docs/library_ref/rich.md` are absent. Every `typer §N` / `rich §N` citation in this file
  points at nothing.
* **Neither library is a declared dependency.** They appear in `uv.lock` only as transitive
  resolutions (`rich` 15.0.0, `typer` 0.27.1 — note the lockfile's Typer differs from the
  0.25.0 version anchor above), and `pyproject.toml` declares neither.
* **The design suite never mentions them.** `typer` and `rich` occur zero times across the
  seven artifacts in `docs/upfront_design/`.

The nearest real requirement is `RM W2` work package 8, *"Internal administration and
diagnostics"* — a non-MCP administrative CLI or test client for bootstrap testing. That is a
**Rust** daemon-side surface, not a Python one, and the specification does not choose a CLI
library for it. `SRV §7 Non-goals` and `SRV §31` further constrain the Python side: the FastMCP
adapter is STDIO-only, and `SRV §6` invariant 11 requires STDOUT to carry protocol frames only —
which rules out a Rich `Console` writing to STDOUT anywhere in that process.

**If a Python CLI is ever introduced**, this skill needs its two reference documents written
before it becomes usable, and this section rewritten against the actual module layout. Until
then, prefer the specification: `docs/spec_index/library-routing.md` maps spec sections to the
references that *do* exist.
