---
name: code-facts-lib-ref
description: "Reference navigator for the CPG fact-extraction provider stack — the four version-pinned deep-dives that turn source bytes into facts. SKILL.md maps them at `docs/library_ref/`: `tree_sitter_rust_python.md` (incremental CST, queries, error recovery; §0-§45, incl. the 1,868-line Python chapter §45), `ruff_python_crates_advanced_reference_2026-08-18.md` (Python coordinates, tokens, typed AST, trivia, scopes/bindings; §0-§25), `pyrefly_rust_cpg_advanced_reference_1.2.0_2026-08-19.md` (Python types, call targets, members, imports; §0-§38 + Appendices A-E), `rust_mir_cpg_continuous_reference_2026-08-18.md` (`rustc_public` MIR, CFG, dataflow, instances; §0-§58 + Appendices A-R); REFERENCE.md (same folder) holds per-doc section indexes, the cross-document authority matrix, decision trees, and operating rules. Use when Rust touches `use tree_sitter::`/`tree_sitter_python`/`use ruff_python_ast::`/`use ruff_python_parser::`/`use ruff_python_semantic::`/`use ruff_python_trivia::`/`pyrefly::query::Query`/`rustc_public`/`rustc_private`, authors `Parser`/`Tree`/`Node`/`InputEdit`/`QueryCursor`/`Parsed<ModModule>`/`Indexer`/`TriviaRanges`/`SemanticModel`/`get_type_table_in_file`/`get_callees_with_location`/`Body`/`Place`/`Instance::resolve`/`DefPathHash`, or edits a provider adapter, `Cargo.toml` pins for those crates, or `rust-toolchain.toml`. Derived graph analyses (petgraph) and fact storage/serving → siblings `datafusion-pyarrow-rust-ref` / `deltalake-rust-ref`."
allowed-tools: Read, Grep, Glob, Bash
---

# Code-Facts Provider Reference Navigator

Routes the four deep-dive references behind CodeFabric's **fact generation** layer — the providers that turn source bytes into facts, before anything is stored or served. This SKILL.md is the **core map**: version anchors, the four-document topic table, reading strategy, where-to-look routing, and the key invariants. The companion **`REFERENCE.md`** (same folder) carries the per-document section indexes (every § and appendix with line numbers), the cross-document **authority matrix** (which provider *wins* per fact family, not merely which doc mentions it), the eight decision trees (provider choice · Python syntax · type source · Rust surface · call resolution · broken source · stable identity · invalidation), the full 18 operating rules, and the CodeFabric project context. Reach for REFERENCE.md once you know *which* document you need and want its section map; cross-references back here are written `SKILL §...`.

**Out of scope** (covered elsewhere): derived graph analyses — traversal, SCCs, dominators, condensation — are petgraph's job (`docs/library_ref/petgraph.md`; fact-gen spec §7.5 governs its isolation). Filesystem watching/debounce, and Git worktree/index/ignore state → sibling **`gix-notify-ref`** (`docs/library_ref/notify_debouncer_full_rust_reference.md`, `gix_rust_advanced_reference.md`) — it decides *whether a file matters and what changed*; these four decide *what it means*. Storing or serving facts → siblings **`datafusion-pyarrow-rust-ref`** and **`deltalake-rust-ref`**. Searching *this* repository (as opposed to parsing a target repository) is `ast-grep`/`rg` work — see `.claude/skills/_shared/code-intelligence.md`, not these docs.

---

## Version anchors

* **tree-sitter 0.26.12** + **tree-sitter-python 0.25.0** (exact-pinned `=`) — Rust bindings (`Parser`, `Tree`, `Node`, `Query`, `QueryCursor`) driving Tree-sitter's **C** runtime; grammar crates supply generated `LanguageFn` values. ABI gate: `LANGUAGE_VERSION = 15`, `MIN_COMPATIBLE_LANGUAGE_VERSION = 13` — assignment to a `Parser` is the authoritative compatibility check (`ts §40.2`). Produces a **CST, not an AST**, and always returns a tree — errors are `ERROR`/`MISSING` nodes, not failures.
* **Ruff 0.16.1** / internal component crates **0.0.7** / edition 2024 / MSRV 1.95 — eight crates (`ruff_source_file`, `_parser`, `_ast`, `_trivia`, `_index`, `_semantic`, `_codegen`, `_formatter`) plus `ruff_text_size` for the `TextSize`/`TextRange` coordinate currency. Astral publishes these as **internal components with frequent breaking changes**; pin the whole train together.
* **Pyrefly 1.2.0** / edition 2024 — Rust-native Python type checker. The `pyrefly::query::Query` facade is labelled *"not intended for external use"*; 1.2.0 replaced per-occurrence type shapes with a **deduplicated type table** (`get_type_table_in_file`). Pyrefly consumes Ruff component crates **0.0.6** — one minor behind the anchor above, which is the whole reason for a sidecar.
* **`rustc_public` 1.100.0-nightly** @ nightly **2026-08-18**, components `rustc-dev` + `llvm-tools` + `rust-src` — compiler-shipped (not crates.io) MIR surface, formerly Stable MIR, with `rustc_private` as a narrowly scoped escape hatch. **This repo pins stable and deliberately does not declare `rustc-dev`** — adopting MIR extraction is an architectural decision, not a toolchain edit (repo spec §76; CLAUDE.md *Environment*).
* **Stack contract — Python**: `source bytes → ts CST (fast lane, survives broken edits) → Ruff tokens + typed AST + trivia + scopes/bindings → Pyrefly Query type table + callees + members → CPG reconciler (authority + provenance + confidence) → fact batch`. **Rust**: `source bytes → ts CST (fast lane) → rustc_public Body/MIR → Instance resolution → owner-scoped fact batch`. Both lanes analyze **the same immutable source snapshot** and join on `(file, content_digest, byte range)` — never on shared parser objects.

---

## The four reference documents

All live at `docs/library_ref/`. Each opens with a version anchor and its own **"Proposed comprehensive documentation map"** (a usable TOC), then deep-dives. **The deep-dive H1 prefix differs per doc and two of them collide** — `ruff` and `pyrefly` both use a bare `# N) `, so a `^# \d+\)` grep returns hits from both. Always scope to one file (Operating Rule 15).

| Doc | Path (`docs/library_ref/`) | Lines | Deep-dive prefix | Scope (deep-dive range) |
|-----|------|------:|------------------|-------|
| **ts** | `tree_sitter_rust_python.md` | 9,439 | `# Tree-sitter Advanced — N) ` | **§0-§45** — native Rust-hosted Tree-sitter: crate/ABI selection, `Parser` lifecycle, text input/encodings, CST model, node inspection, cursor traversal, **incremental parsing + `changed_ranges`**, the whole query subsystem (syntax, operators, predicates, cursors), **error recovery**, parse states/lookahead, static node types, grammar authoring/DSL/scanners, highlighting, tags, WASM, FFI, allocation, concurrency, perf, deployment, security, code-intelligence recipes. **§45 is a 1,868-line Python chapter** (`45.0`-`45.39`) — a doc within the doc. |
| **ruff** | `ruff_python_crates_advanced_reference_2026-08-18.md` | 7,246 | `# N) ` *(collides with pyrefly)* | **§0-§25** — the eight Ruff component crates one per chapter (§3 source coords, §4 parser/tokens, §5 typed AST, §6 trivia, §7 index, §8 semantic, §9 codegen, §10 formatter), then cross-crate ownership, source-preserving edits, LibCST replacement, **CPG deployment pattern (§14)**, incrementality, error tolerance, perf, concurrency, testing, upgrades, security, **capability decision tables (§22)**, global anti-patterns, agent checklist. |
| **pyrefly** | `pyrefly_rust_cpg_advanced_reference_1.2.0_2026-08-19.md` | 8,231 | `# N) ` *(collides with ruff)* | **§0-§38 + §15A + Appendices A-E** — Pyrefly as the Python semantic/type oracle: crate map, exports→bindings→answers model, module identity, **the Ruff cross-version boundary (§5)**, state/`Require`/epochs, type model, flow narrowing, the experimental **`Query` API (§9-§13)** — type table, callees, attributes, subtype — plus **TSP (§14)**, LSP (§15), Glean (§15A), the **Query-vs-TSP-vs-LSP decision matrix (§16)**, CPG schema, sidecar protocol, bootstrap/incremental workflows, coordinate sync, identity/provenance, imports, MRO, dynamic Python, error tolerance, perf, testing, upgrades. |
| **mir** | `rust_mir_cpg_continuous_reference_2026-08-18.md` | 5,207 | `# Rust MIR Advanced — N) ` | **§0-§58 + Appendices A-R** — `rustc_public` as the Rust semantic substrate: toolchain setup, extraction-surface choice (§4), Cargo wrapper integration, `Body` anatomy, locals, **CFG (§10-§12)**, operands/places/rvalues, types, spans/macros, visitors, generic-vs-mono MIR, **`Instance` resolution + call extraction (§20-§24)**, CFG→CPG mapping, **dataflow/borrows/moves/def-use (§26-§31)**, closures/async, unsafe/FFI, summaries, schema, **stable identifiers (§37)**, fingerprints, continuous update (§39-§46), hybrid Tree-sitter+MIR (§47), perf, testing, upgrades. **Appendices A-R are 2,206 lines — 42% of the doc** and carry the normalized extraction model (J), exhaustive extraction rules (K), call-graph construction (L), the continuous-update algorithm (M), and the validation fixture matrix (Q). |

**Reading strategy.** Find the section in REFERENCE.md's per-doc index (SKILL §1 there), then `Read(offset, limit)`. To rederive a map live — after a doc is revised, or to zoom into one chapter — use `lib-outline <file>` (`just lib-outline`), which is markdown-aware and so immune to the grep hazards in REFERENCE.md Rules 15-17; `spec-outline` is for `docs/upfront_design/` and refuses these files. Deep-dives run 30-2,000 lines with `## N.M` subsections; **nearly every chapter closes with an anti-pattern inventory and/or an agent checklist** — load the closing 100-200 lines before drafting code. Three sub-corpora deserve to be treated as documents in their own right: `ts §45` (Python, 40 subsections), `mir` Appendices A-R, and `pyrefly` Appendices A-E. Four chapters are pure decision aids and are worth reading before designing anything: `ruff §22`, `pyrefly §16`, `mir` Appendix C, `mir` Appendix H.

---

## Where do I look?

| Question | Doc |
|---|---|
| Parse Python/Rust source that may be **mid-edit or syntactically broken**; incremental reparse; changed ranges | **ts** (§10, §11, §20) |
| Structural pattern matching over syntax — captures, predicates, cursors | **ts** (§12-§17) |
| Typed Python AST, tokens, comments/trivia, byte↔line coordinates, docstrings, f-string structure | **ruff** (§3-§7) |
| Python scopes, bindings, references, shadowing, qualified names, import syntax semantics | **ruff** §8 (lexical) → **pyrefly** §26 (cross-module resolution) |
| Inferred Python types, declared-vs-computed-vs-expected, narrowing, `Any` semantics | **pyrefly** (§7, §8, §10, §14) |
| Python call targets, class members, properties, MRO, subtype checks | **pyrefly** (§11, §12, §13, §27) |
| Rust definitions, types, MIR bodies, CFG, places, moves/borrows, drop, unwind | **mir** (§8-§17, §26-§31) |
| Rust call edges — direct, indirect, trait dispatch, drop glue | **mir** (§20-§24 + Appendix L) |
| A canonical, edit-stable ID for a node/definition | **mir** §37 + Appendix O · **ts** §11.7 · **ruff** §14.3 · **pyrefly** §19.1 |
| What to emit when source won't parse or a crate won't compile | **ts** §20 · **ruff** §16 · **pyrefly** §29 · **mir** §46 |
| Continuous update — what invalidates what, and how a batch commits atomically | **ts** §11 · **ruff** §15 · **pyrefly** §23 · **mir** §41-§46 + Appendix M |

For deeper routing — the full cross-document **authority matrix** (which provider is authoritative per fact family, reconciled against fact-gen spec §5) and the eight decision trees — see **`REFERENCE.md`**.

---

## Key invariants

The seven that prevent the most errors; the full set of **18 operating rules** is in `REFERENCE.md`.

1. **Each provider owns a different question. Never ask one for a fact it does not have.** Ruff tells you what the source *says*; Pyrefly what it *means* statically; tree-sitter what survives a *broken* edit; MIR what Rust actually *compiles to*. A confident answer from the wrong provider is the failure mode this skill exists to prevent. (`ruff §0.5`; `pyrefly §18`; `mir §47`)
2. **Every provider sits behind an application-owned adapter emitting application-owned DTOs.** No long-lived `Node<'tree>` escapes a tree-sitter adapter; no `rustc_public`/`rustc_private` object escapes the compiler callback or crosses a thread; Ruff `0.0.x` types stay inside the Ruff adapter. This is normative, not stylistic (fact-gen spec §7). (`ts §7.1`, `§34`; `ruff §1.3`, `§11`; `mir §2.1`, `§39.0`)
3. **Absence is never proof of absence.** A missing callee is not proof the call has no target; a failed compile yields **capability gaps**, not an empty result implying "none". Materialize an explicit unknown or a multi-candidate fact instead. (`pyrefly §0.6`, `§11.5`, `§17.3`; `mir §23.2`, §46; `ts §20.5`)
4. **Canonical identity is application-owned.** Tree-sitter node IDs, Ruff `AtomicNodeIndex`, Pyrefly response-local type indices and binding keys, and raw `DefId` are all **snapshot-local implementation details**. Rust prefers `StableCrateId` + `DefPathHash`; everything else derives identity from source semantics plus provenance. (`mir §37`; `ts §8.10`, `§11.3`; `ruff §5.10`; `pyrefly §0.4`, `§19.1`)
5. **A byte range is meaningless without the content digest it was computed against.** `(file_id, content_digest, start, end)` is the unit of provenance; `TextRange`/`Range` alone is unsafe persistent identity, and a range from revision N applied to N+1 can slice mid-code-point. Replace the whole per-file snapshot bundle on change. (`ruff §0.4`, `§3.8`; `ts §6.7`; `pyrefly §6.6`, `§24.5`)
6. **Ruff 0.0.7 and Pyrefly's bundled 0.0.6 are distinct Rust type universes.** `ruff_python_ast(0.0.6)::Expr` ≠ `ruff_python_ast(0.0.7)::Expr` even though the names match. Never pass AST nodes across that boundary — share a source snapshot and ranges instead, which is the argument for the Pyrefly **sidecar** (also isolating its allocator, threading, and crash behavior). (`pyrefly §5`, `§20.1`)
7. **MIR extraction needs a date-pinned nightly plus `rustc-dev`, which this repo deliberately does not declare.** `rust-toolchain.toml` pins stable; nightly exists only for `just miri` / `just udeps`. Treat `mir` as design input until that decision is made explicitly — and note MIR is *not* SSA, `Instance` resolution is what makes call edges concrete, and dynamic dispatch is an over-approximation with declared precision tiers. (repo spec §76; `mir §1`, `§4`, `§23.2`, `§30.0`)

---

## Project context: CodeFabric

**Pre-implementation.** There is no adapter code yet — one Cargo package, one library crate, and a seed that only proves the toolchain. So these four docs are read *against the specs*, not against existing code, and the mapping that matters is fact-domain → doc, not crate → doc.

| Fact domain (ontology) | Primary doc | Supporting |
|---|---|---|
| Source bytes, coordinates, ranges | **ruff** §3 (Python) | **ts** §6 · **mir** §17 (Rust spans/macros) |
| Concrete syntax, incomplete edits, changed ranges | **ts** §7-§11, §20 | — |
| Python tokens, comments, trivia, omitted lexical facts | **ruff** §4, §6, §7 | **ts** §45.20-§45.21 |
| Python typed syntax, scopes, bindings, references | **ruff** §5, §8 | **ts** §45.15-§45.19 |
| Python types, callees, members, imports, MRO | **pyrefly** §7-§14, §26, §27 | **ruff** §8.11 (qualified names) |
| Rust semantic defs, types, MIR, CFG, dataflow, ownership | **mir** §8-§34 | **ts** (Rust CST) |
| Identity, provenance, confidence, unknowns | **mir** §37-§38 · **pyrefly** §25 | **ts** §11.7 · **ruff** §14.3 |
| Incremental invalidation and atomic publication | **mir** §41-§46 + App. M | **ts** §11 · **pyrefly** §23 · **ruff** §15 |

**Spec anchors.** `present_state_cpg_fact_generation_specification_python_rust_v1.3.md` §2 names five library references as its source basis — these four plus `petgraph.md`, which is routed by the `petgraph-ref` sibling. **§5.1/§5.2 give the authority order per fact family** (mirrored in REFERENCE.md §2) and **§7.1-§7.4 give the per-provider isolation requirements**, one subsection per provider here (§7.5 covers petgraph). Cite specs by **section number, not line number** — the spec filenames carry a version suffix that moves between releases and line numbers shift with every revision, while section numbers have held. `docs/spec_index/library-routing.md` maps spec sections to the chapters of these four references.

**Rule of thumb:** building a provider adapter, or reasoning about what a provider can and cannot tell you → this skill. Normalizing, storing, or serving the resulting facts → the fabric and serving siblings. Fuller context: `REFERENCE.md` §5.
