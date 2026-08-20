# Code-Facts Providers — Detailed Reference

This document is the deep-dive companion to `SKILL.md` in the same directory. SKILL.md carries the core map (version anchors, the four-document topic table, reading strategy, where-to-look routing, and the seven key invariants); this file carries the full **per-document section indexes** (every § and appendix with its line number), the **cross-document authority matrix** (which provider wins per fact family), the **decision trees**, the **full 18 operating rules**, and the **CodeFabric project context**.

Cross-references back into the core map are written `SKILL §...`. Read SKILL.md first; reach here once you know which document you need and want its section map, when a routing choice gets hard, or when two providers both appear to answer a question and you need to know which one the project treats as authoritative.

These four docs are **unusually hostile to naive grepping** and there are three independent hazards — a shared `# N) ` prefix between `ruff` and `pyrefly`, internal tables-of-contents whose `## N)` headings are shape-identical to body subsections, and code-fence comments that match `^# `. Operating Rules 15-17 spell each out. Prefer `lib-outline` over a heading grep — it parses markdown, so none of the three hazards apply (Rule 18) — and use the line numbers in §1 below with `Read(offset, limit)`.

## Table of Contents

- **§1 — Per-document section indexes**
  - §1.1 `tree_sitter_rust_python.md` (§0-§45, plus the §45 Python sub-index)
  - §1.2 `ruff_python_crates_advanced_reference_2026-08-18.md` (§0-§25)
  - §1.3 `pyrefly_rust_cpg_advanced_reference_1.2.0_2026-08-19.md` (§0-§38 + §15A + Appendices A-E)
  - §1.4 `rust_mir_cpg_continuous_reference_2026-08-18.md` (§0-§58 + Appendices A-R)
- **§2 — Cross-document authority matrix** (which provider wins per fact family)
- **§3 — Decision trees** (provider choice · Python syntax · type source · Rust surface · call resolution · broken source · stable identity · invalidation)
- **§4 — Operating rules** (full set of 18)
- **§5 — Project context: CodeFabric**

---

## §1 — Per-document section indexes

Line numbers are **start lines**, verified against the source files, for use as `Read(offset, ...)`. Deep-dive rows are **bolded**; front-matter and appendix rows are plain.

### §1.1 `tree_sitter_rust_python.md` — section index (§0-§45)

9,439 lines. Front matter `1-197`; deep-dives start at line `198` with `# Tree-sitter Advanced — 0)` and use `## N.M` subsections. Nearly every chapter closes with an anti-pattern inventory; §19-§23 also close with an agent checklist. **§45 is a 1,868-line Python chapter — see the sub-index below.**

| § | Line | Title | Key subsections |
|---|------|-------|-----------------|
| — | 1 | Front matter | version/source anchors · feature inventory |
| — | 43 | **Proposed comprehensive documentation map** — the doc's own TOC, `## 0)`-`## 45)` | one abstract per chapter |
| — | 185 | Suggested expansion order | 8 reading phases |
| **0** | 198 | Scope, versioning, and mental model | 0.2 canonical architecture · 0.3 parse pipeline · 0.4 vocabulary · **0.5 CST, not AST** · 0.6 tree/source separation · 0.7 incremental model · 0.8 query model · **0.9 LLM-agent invariants** · 0.10 anti-patterns |
| **1** | 393 | Installation, crate selection, Rust project layout | 1.1 minimal `Cargo.toml` · **1.2 grammar/runtime version rule** · 1.3 why `tree-sitter-language` · 1.5 feature flags · 1.6 native build · 1.8 language registry · 1.9 production dependency checklist |
| **2** | 576 | First executable Rust app | 2.1 minimal parse · 2.2 named vs anonymous · 2.4 `TreeCursor` DFS · 2.5 first query · 2.6 first incremental edit · 2.7 first error-tolerant parse |
| **3** | 764 | Architecture: Rust host, C runtime, generated grammar | 3.0 layer model · 3.1 what the Rust wrapper owns · **3.4 ownership boundaries** · 3.5 FFI audit rule |
| **4** | 864 | Languages, grammar crates, `LanguageFn`, ABI, metadata | 4.0 language construction · **4.1 ABI compatibility check** · 4.4 node-kind inventory · 4.5 field inventory · 4.6 supertypes · 4.7 grammar registry · 4.8 upgrade gate |
| **5** | 1016 | `Parser` lifecycle and state | 5.0 state model · 5.2 reuse vs recreate · 5.4 reset · 5.6 included ranges as state · **5.8 parser-pool architecture** |
| **6** | 1139 | Text input and encodings | **6.0 input API decision tree** · 6.2 callback input · 6.3 rope/piece-table adapter · 6.4 UTF-16 · 6.5 `Decode` · **6.7 source retention contract** · 6.9 performance rules |
| **7** | 1335 | Syntax tree model: `Tree`, `Node`, CST semantics | 7.1 `Node` is a lightweight view · 7.2 named/anonymous/extra · **7.3 error and missing nodes** · 7.4 positions · 7.5 root with offset · 7.7 CST normalization layer · 7.8 cloning/snapshots |
| **8** | 1481 | Node inspection API | 8.1 kind vs grammar name/id · 8.2 child access complexity · 8.3 fields · **8.5 descendant lookup by byte range** · 8.9 `has_changes` · **8.10 node identity** · 8.11 parse-state APIs |
| **9** | 1625 | Efficient tree traversal | 9.0 why `TreeCursor` · 9.1 iterative DFS · 9.3 field-aware cursor · 9.4 pruned traversal · **9.7 query vs traversal decision** · 9.8 perf checklist |
| **10** | 1754 | Incremental parsing | **10.0 the critical invariant** · 10.1 `InputEdit` · 10.2 insert/delete/replace · 10.3 edit transaction · 10.5 reparse · 10.7 cached nodes after edit · 10.8 validation · 10.9 editor checklist |
| **11** | 1912 | Changed ranges, identity, cache invalidation | **11.0 `Tree::changed_ranges`** · 11.1 structural not textual · 11.2 ranges may be conservative · 11.3 node IDs and reuse · 11.4 invalidation strategy · 11.5 expand to semantic boundaries · **11.7 stable domain identities** |
| **12** | 2035 | Query mental model and compilation | 12.0 compiled structural matcher · **12.1 compile once, execute many** · 12.3 query metadata · 12.4 capture-name mapping · 12.6 disable captures/patterns · 12.8 grammar upgrade contract |
| **13** | 2195 | Query syntax | 13.0 basic node pattern · 13.1 fields · 13.2 negated fields · 13.4 wildcards · **13.6 `ERROR`** · **13.7 `MISSING`** · 13.8 supertypes · 13.10 test harness |
| **14** | 2331 | Query operators | 14.0 capture operator · 14.1 quantifiers · 14.2 capture quantifiers in Rust · 14.4 alternations · 14.6 anchors `.` · 14.8 operator design rules |
| **15** | 2513 | Query matches and captures in Rust | 15.0 two execution modes · 15.1 `QueryMatch` · 15.3 text provider · 15.4 capture-centric iteration · **15.5 materialize spans, not nodes** · 15.6 reusing `QueryCursor` |
| **16** | 2638 | Query predicates and directives | 16.1 equality family · 16.2 regex family · 16.3 `any-of?` · 16.5 `set!` directives · **16.7 directives are consumer conventions** · 16.8 predicate performance |
| **17** | 2769 | Query cursor controls and containment | 17.1 byte range · 17.2 point range · 17.3 containing ranges · 17.4 zero end is special · 17.5 max start depth · 17.6 match limit · 17.8 range-local extraction |
| **18** | 2891 | Parse/query progress and cancellation | 18.0 progress callbacks · 18.1 parse deadline · 18.4 query deadline · 18.5 cancellation policy by workload · **18.6 previous-tree behavior on cancelled parse** · 18.8 budget layering |
| **19** | 3032 | Included ranges and multi-language documents | 19.1 `Range` contract · 19.2 setting included ranges · **19.4 coordinates remain document-global** · 19.6 with incremental parsing · 19.8 injection ownership record · 19.9 recursive injections · 19.12 agent checklist |
| **20** | 3257 | Error recovery and incomplete code | **20.0 produce a tree through errors** · 20.1 error inspection API · **20.2 ERROR vs MISSING** · 20.3 querying recovery nodes · **20.5 partial semantic extraction policy** · 20.7 incomplete code and stable IDs · 20.9 syntax errors are not API failures · 20.10 ingestion best practices |
| **21** | 3452 | Parse states, lookahead, completion, diagnostics | 21.1 node parse states · 21.2 `next_state` · 21.3 lookahead iterator · 21.4 completion candidate pipeline · 21.5/21.6 ERROR/MISSING diagnostics · **21.9 parse state is ephemeral** |
| **22** | 3656 | Static node types and typed Rust wrappers | **22.0 `node-types.json` is the CST schema** · 22.3 `NODE_TYPES` · 22.5 typed wrapper pattern · 22.6 IDs for hot paths, names for contracts · **22.8 schema drift detection** · 22.10 agent-facing schema export |
| **23** | 3904 | Language introspection and grammar metadata | 23.2 ABI validation · 23.3 node-kind inventory · 23.5 field inventory · 23.7 parse-state inventory · **23.9 grammar fingerprinting** · 23.10 startup validation · 23.11 grammar-diff report |
| **24** | 4097 | Creating a grammar crate for native Rust consumption | 24.2 repository layout · 24.4 Rust binding facade · 24.5 build script · **24.7 generated artifacts as contracts** · 24.9 version policy |
| **25** | 4293 | Grammar DSL reference | `seq`/`choice`/`repeat`/`optional`/`token`/`token.immediate` · 25.8 parse precedence · 25.9 lexical precedence · `alias` · `field` · `reserved` · **25.15 CST-first grammar design** |
| **26** | 4493 | Precedence, ambiguity, and GLR conflicts | 26.1 static precedence · 26.2 associativity · 26.3 declared conflicts · 26.4 dynamic precedence · 26.6 refactor before adding conflicts · 26.8 downstream impact |
| **27** | 4624 | Lexing, extras, keywords, token boundaries | 27.0 context-aware lexing · 27.1 lexical precedence sequence · 27.2 keyword vs identifier · 27.3 `word` token · **27.4 extras** · 27.5 immediate tokens |
| **28** | 4739 | CST design: hidden rules, fields, supertypes, aliases, inline | **28.0 grammar shape is downstream API shape** · 28.1 hidden rules · 28.3 fields · 28.4 supertypes · 28.5 aliases · 28.7 field cardinality · 28.8 CST compatibility policy |
| **29** | 4885 | External scanners | 29.1 declare external tokens · 29.2 required functions · **29.3 `valid_symbols` is a hard contract** · **29.5 serialization is incremental-parsing state** · 29.9 test matrix · 29.10 fuzzing |
| **30** | 5026 | Grammar generation, corpus tests, Rust binding CI | 30.1 generated artifacts · 30.2 ABI generation · 30.3 corpus tests · 30.8 incremental correctness tests · 30.9 query golden tests · 30.10 error-recovery tests · 30.13 migration review checklist |
| **31** | 5217 | Syntax highlighting in native Rust | 31.2 `Highlighter` lifecycle · 31.3 highlight names are application policy · 31.4 `HighlightConfiguration` · 31.5 event stream · 31.6 injections · 31.7 locals query · 31.9 incremental highlighting |
| **32** | 5368 | Tags and code-navigation extraction | 32.0 what `tree-sitter-tags` provides · 32.1 `TagsContext` · 32.3 generated tag record · **32.5 tags vs custom query extraction** · 32.6 hybrid pattern · 32.7 incremental tag invalidation |
| **33** | 5494 | WASM grammar loading from Rust | 33.1 feature flag · 33.2 engine and store · 33.4 attach store to parser · 33.5 store ownership · **33.7 native vs WASM policy** · 33.8 cache WASM languages |
| **34** | 5606 | Raw FFI interop and ownership boundaries | **34.0 stay in safe Rust unless interop requires raw handles** · **34.3 ownership matrix** · 34.4 borrowed node semantics · 34.5 callback lifetime hazards · 34.6 FFI panic rule · 34.8 raw-pointer audit |
| **35** | 5702 | Memory allocation and native resource behavior | 35.0 allocation model · 35.1 global allocator override · **35.2 why changing allocators late is dangerous** · 35.4 tree lifetime and cache pressure · **35.5 avoid storing `Node` forests** · 35.6 query memory controls |
| **36** | 5797 | Concurrency, thread safety, and pooling | **36.0 two layers of truth** · 36.2 parser pool · **36.3 one parser per concurrent parse** · 36.4 query reuse · 36.5 tree revision ownership · 36.7 result commit race · 36.9 pool sizing |
| **37** | 5931 | Performance engineering and benchmarking | 37.1 core metrics · 37.2 incremental speedup benchmark · 37.3 query benchmark dimensions · **37.5 avoid source copying** · 37.6 precompile queries · 37.7 range-local extraction · 37.9 large-file policy |
| **38** | 6130 | Observability and debugging | **38.0 debug at four layers** · 38.1 parser logger · 38.3 S-expression snapshot · 38.5 DOT parser graphs · 38.7 edit audit record · 38.8 query diagnostics · 38.10 error-node instrumentation |
| **39** | 6351 | Error handling and diagnostics | 39.0 error taxonomy · `LanguageError` · `IncludedRangesError` · `QueryError` · **39.4 parse `Option<Tree>`** · 39.6 UTF-8 slicing error · 39.8 recoverable vs fatal |
| **40** | 6502 | API stability, ABI compatibility, and upgrades | **40.0 version axes are independent** · 40.1 runtime ABI window · **40.2 assignment is the authoritative ABI gate** · 40.4 grammar upgrade checklist · 40.6 CST schema migration · **40.7 numeric IDs are not cross-version contracts** |
| **41** | 6634 | Production deployment patterns | Patterns A-F: editor service · batch indexer · **continuously updating daemon** · language-server subsystem · multi-tenant API · plugin platform · 41.6 storage model · **41.7 atomic update boundary** · 41.9 graceful degradation |
| **42** | 6812 | Security and resource governance | 42.1 untrusted source · 42.2 untrusted query text · 42.3 match-limit semantics · 42.4 native grammar trust · 42.6 injection attacks · 42.7 coordinate validation |
| **43** | 6964 | Code-intelligence recipes | 43.1 symbol definitions · 43.2 imports/modules · 43.3 calls · 43.5 scope records · 43.6 references · **43.7 changed-range owner invalidation** · **43.8 owner identity** · 43.11 hybrid call graph · 43.13 fault-tolerant index transaction |
| **44** | 7234 | Consolidated best practices, invariants, anti-patterns | **44.0 core mental-model invariants** · 44.1-44.8 agent rules (setup/input/traversal/incremental/queries/errors/grammar/production) · **44.9 high-value anti-pattern list** · 44.10 reference architecture · 44.11 final checklist |
| **45** | 7506 | **Parsing Python source with native Rust Tree-sitter** — 1,868 lines, `45.0`-`45.39` | see §1.1a below |
| — | 9374 | Source reference anchors | link definitions |

#### §1.1a `ts §45` — Python chapter sub-index (7506-9373)

| §45.M | Line | Topic |
|---|------|-------|
| 45.0 | 7508 | Scope: Python *source* parsed by a Rust host |
| 45.1 | 7544 | Version/dependency anchors (`tree-sitter = "=0.26.12"`, `tree-sitter-python = "=0.25.0"`) |
| 45.2 | 7589 | First executable Python parser in Rust |
| 45.3 | 7633 | Minimal reusable Python parser wrapper |
| 45.4 | 7670 | CLI commands for validating grammar and queries |
| 45.5 | 7763 | **Indentation and strings require an external scanner** |
| 45.6 | 7811 | Python source encoding policy |
| 45.7 | 7857 | Root, statements, logical lines, semicolons |
| 45.8 | 7893 | **Indentation, blocks, and incomplete edits** |
| 45.9 | 7935 | Identifiers, keywords, soft keywords |
| 45.10 | 7971 | Functions: names, parameters, return types, async, type params |
| 45.11 | 8027 | **Decorators: the definition span is not the decorated span** |
| 45.12 | 8068 | Classes and generic class syntax |
| 45.13 | 8095 | Parameters are a family of node shapes |
| 45.14 | 8154 | Imports: module, name, alias, relative depth kept separate |
| 45.15 | 8219 | **Calls, attributes, and syntactic callees** |
| 45.16 | 8279 | Assignments and binding targets |
| 45.17 | 8319 | Scope construction for Python |
| 45.18 | 8371 | Comprehensions create special binding contexts |
| 45.19 | 8415 | `match`/`case`: pattern identifiers can be bindings, not reads |
| 45.20 | 8465 | Strings, f-strings, interpolation, format specifiers |
| 45.21 | 8504 | Docstrings are a convention, not a grammar node |
| 45.22 | 8536 | Annotations, PEP-695 type parameters, type aliases |
| 45.23 | 8577 | **Grammar acceptance is not a CPython-version validator** |
| 45.24 | 8615 | Error recovery for Python editors |
| 45.25 | 8660 | Completion-oriented parse-state use |
| 45.26 | 8693 | **Incremental parsing: Python-specific edit hazards** |
| 45.27 | 8736 | Python query pack: definitions, imports, calls, owners |
| 45.28 | 8804 | Rust query execution helper |
| 45.29 | 8856 | Upstream `TAGS_QUERY`: useful baseline, intentionally small |
| 45.30 | 8895 | Upstream highlighting query |
| 45.31 | 8920 | `.py`, `.pyi`, generated Python, notebooks |
| 45.32 | 8962 | **Python-specific semantic limitations** |
| 45.33 | 9011 | Python code-intelligence extraction architecture |
| 45.34 | 9065 | Performance and worker design |
| 45.35 | 9110 | Python test corpus |
| 45.36 | 9158 | Python-specific development commands |
| 45.37 | 9210 | Upgrade contract for `tree-sitter-python` |
| 45.38 | 9251 | **Python-specific anti-pattern inventory** |
| 45.39 | 9281 | **Python implementation checklist** |

### §1.2 `ruff_python_crates_advanced_reference_2026-08-18.md` — section index (§0-§25)

7,246 lines. Front matter `1-109`; deep-dives start at line `110` with `# 0)` — **the bare `# N) ` prefix collides with `pyrefly`** (Rule 15). The most systematically templated of the four: nearly every chapter ends `## N.x Anti-pattern inventory` then `## N.y Agent checklist`. No appendices.

| § | Line | Title | Key subsections |
|---|------|-------|-----------------|
| — | 1 | Front matter | `## Source-of-truth hierarchy` (line 11) — the "internal components, frequent breaking changes" warning |
| — | 38 | **Feature inventory** | 8-crate × 5-attribute responsibility table + ASCII pipeline diagram |
| — | 79 | Proposed comprehensive documentation map | the doc's own TOC |
| **0** | 110 | Scope, versioning, and mental model | 0.0 version anchors · 0.1 what the stack is / is not · 0.2 canonical analysis pipeline · 0.3 vocabulary · **0.4 the coordinate invariant** · **0.5 separation from Pyrefly** |
| **1** | 304 | Installation, crate selection, Rust project layout | 1.0 minimal dependency set · **1.1 workspace pinning (`=0.0.7`)** · 1.2 optional features · **1.3 recommended facade crate** · 1.4 build/CI commands · 1.5 daemon layout |
| **2** | 521 | First executable stack: parse → inspect → source locations | 2.0 minimal parse · **2.1 keep the complete `Parsed` object** · 2.2 build trivia + index once · 2.3 byte ranges → user positions · 2.4 first-agent checklist |
| **3** | 618 | `ruff_source_file` — coordinates, lines, newline semantics | 3.1 `LineIndex` · 3.2 `SourceCode` · **3.3 `PositionEncoding`** · 3.4 `SourceFile`/`SourceFileBuilder` · 3.5 universal newlines · 3.6 `LineRanges` · **3.7 coordinate policy for a CPG** · 3.8 error boundaries · 3.10 anti-patterns · 3.11 checklist |
| **4** | 898 | `ruff_python_parser` — lexer, parser, tokens, recoverable parse | 4.1 `parse_module` · 4.3 range-based expression parsing · 4.4 parenthesized mode · 4.5 string annotations · **4.7 `parse_unchecked`: critical for code intelligence** · 4.8 `parse_unchecked_source` · 4.9 notebook/cell parsing · 4.10 `Parsed<T>` contract · 4.11 tokens · **4.13 parse errors vs unsupported syntax** · **4.14 parser is not incremental** · 4.17 anti-patterns · 4.18 checklist |
| **5** | 1319 | `ruff_python_ast` — typed syntax model, traversal, transformation | 5.1 node families · 5.2 `PySourceType` · 5.4 `Ranged` · 5.5 `Visitor` · **5.6 evaluation order vs source order** · 5.7 `StatementVisitor` · 5.8 `Transformer` · 5.9 `AnyNodeRef` · **5.10 node indices** · 5.11 qualified names · 5.12 helpers · 5.14 docstrings · 5.15 operator precedence · **5.18 string model** · 5.19 comparable AST · 5.22 parent maps · 5.23 anti-patterns · 5.24 checklist |
| **6** | 1832 | `ruff_python_trivia` deep dive | **6.0 source-side facts the AST does not own** · 6.2 `TriviaRanges` · 6.3 `CommentRanges` · 6.4 `ParenthesizedExpressions` · 6.5 `SimpleTokenizer` · 6.6 `BackwardsTokenizer` · 6.7 whitespace/indentation · 6.9 pragmas and suppression · 6.10 `textwrap` · **6.12 when trivia should gate an edit** · 6.13 anti-patterns · 6.14 checklist |
| **7** | 2288 | `ruff_python_index` deep dive | **7.0 token-derived omitted-fact index, not a project index** · 7.2 internal indexed facts · 7.3 comment ranges · 7.4 interpolated-string ranges · 7.5 multiline-string ranges · 7.6 continuation line starts · **7.8 why it is valuable even with Tree-sitter** · 7.9 cache/lifecycle · 7.10 anti-patterns |
| **8** | 2552 | `ruff_python_semantic` deep dive | **8.0 Ruff's linter semantic state (not type checking)** · 8.2 `SemanticModel<'a>` · **8.3 construction is not analysis** · 8.5 scopes · 8.6 bindings · 8.7 shadowing/rebinding · 8.8 definitions vs bindings · 8.9 resolved/unresolved references · **8.11 qualified-name resolution** · 8.13 builtins · 8.14 imports · 8.15 branch context · 8.16 execution context · **8.19 CFG module** · 8.20 integration boundary · **8.21 Pyrefly complementarity** · 8.22 confidence classes · 8.25 anti-patterns · 8.26 checklist |
| **9** | 3244 | `ruff_python_codegen` deep dive | **9.0 unparser, not a lossless source printer** · 9.2 `round_trip` · 9.3 `Stylist` · 9.4 `Generator` · 9.6 precedence-aware generation · 9.7 string literal generation · **9.10 comments and codegen** · 9.11 recommended transform pattern · 9.13 anti-patterns |
| **10** | 3599 | `ruff_python_formatter` deep dive | 10.1 dependency weight · 10.2 `format_module_source` · 10.3 `format_module_ast` · **10.4 comment attachment model** · 10.5 `PyFormatOptions` · 10.8 magic trailing comma · 10.13 `format_range` · 10.15 source maps · **10.17 formatter vs codegen** · 10.18 formatter vs source-preserving edits · 10.22 anti-patterns |
| **11** | 4109 | Cross-crate ownership and lifetime model | **11.0 source revision is the root of truth** · 11.1 file snapshot abstraction · 11.2 stable vs ephemeral data · 11.3 why normalized facts own strings/IDs · 11.4 range ownership · 11.5 dependency direction |
| **12** | 4283 | Source-preserving edits vs AST regeneration | 12.1 canonical edit type · **12.2 apply edits right to left** · 12.3 detect overlaps · 12.4 expected-old-text guard · 12.6 narrowest-correct-range · 12.8 comment-safe deletion · 12.10 indentation-sensitive insertion · **12.11 edit validation ladder** · 12.13 anti-patterns |
| **13** | 4678 | Building a LibCST-replacement analysis stack | 13.0 capability mapping · 13.2 what you do not get automatically · 13.4 parent provider · 13.5 position provider · 13.6 qualified-name provider · 13.7 scope provider · 13.8 codemod replacement · 13.11 differential validation |
| **14** | 4952 | **Code-intelligence / CPG deployment pattern** | 14.0 frontend architecture · **14.1 why normalize before the CPG transaction** · 14.2 normalized fact families · **14.3 stable node IDs** · 14.4 file-level replacement transaction · 14.5 async enrichment must stay versioned · 14.6 call graph · 14.7 import graph · 14.8 comment policy · 14.10 checklist |
| **15** | 5226 | Incrementality and cache boundaries | **15.0 Ruff parser is not incremental** · 15.1 cache hierarchy · 15.2 content-addressed parse cache · 15.3 derived-cache invalidation · 15.4 file-fact diff · **15.5 optional Tree-sitter changed-range oracle** · 15.6 debouncing · 15.8 semantic cache boundary · 15.9 Pyrefly cache boundary |
| **16** | 5430 | Error tolerance and partially invalid Python | 16.0 two parser modes · **16.1 syntax error vs unsupported syntax** · 16.2 live-editor policy · 16.3 CI/committed-source policy · **16.4 stale-good snapshot strategy** · 16.5 notebook cells · 16.6 error propagation |
| **17** | 5586 | Performance and allocation strategy | 17.1 parse source by reference · **17.2 parse once, derive many** · 17.4 `LineIndex` caching · 17.5 range-first representation · 17.7 formatter isolation · 17.8 batch graph writes · 17.9 string interning · 17.11 telemetry |
| **18** | 5806 | Concurrency and service deployment | **18.0 preferred concurrency axis: files** · 18.1 avoid shared mutable semantic state · 18.3 backpressure · 18.4 cancellation · 18.5 duplicate work suppression · 18.6 service process boundary |
| **19** | 5955 | Testing and golden/snapshot strategy | **19.0 test the normalized contract, not only Ruff calls** · 19.1 parser fixture matrix · 19.2 source-range tests · 19.4 semantic tests · 19.5 Pyrefly merge tests · 19.8 differential tests vs CPython · 19.10 repository corpus · 19.11 upgrade snapshots |
| **20** | 6205 | Upgrade and version-migration discipline | **20.1 release-train invariant** · 20.2 upgrade workflow · 20.4 semantic-drift checklist · **20.5 exhaustive matches are upgrade sentinels** · 20.6 new Python version adoption · 20.8 adapter-version provenance · 20.9 anti-patterns |
| **21** | 6410 | Security and untrusted-source considerations | **21.0 parsing source does not execute Python** · 21.1 resource-exhaustion threat model · 21.2 input limits · 21.3 paths are data, not authority · 21.7 diagnostics and source privacy |
| **22** | 6568 | **Capability decision tables** | **22.0 "Which crate do I reach for?"** · 22.1 analysis-level decision tree · 22.2 source transformation · 22.3 persistence · 22.4 reliability · **22.5 agent-safe "do not confuse" table** |
| **23** | 6692 | Global anti-pattern inventory | 23.0 architecture · 23.1 parsing/source · 23.2 AST · 23.3 trivia/index · 23.4 semantics · 23.5 edits/codegen · 23.6 formatter · 23.7 deployment |
| **24** | 6767 | LLM-agent implementation checklist | 24.0 before writing code · 24.1-24.8 per-task checklists · **24.9 agent operating rules — concise promptable version** |
| **25** | 6943 | Reference links and source audit | 25.0 pinned anchors · 25.1-25.9 per-crate links · **25.11 source-audit commands** · 25.12 final architecture recommendation |

### §1.3 `pyrefly_rust_cpg_advanced_reference_1.2.0_2026-08-19.md` — section index (§0-§38 + §15A + Appendices A-E)

8,231 lines. Front matter `1-202`; deep-dives start at line `203` with `# 0)` — **the bare `# N) ` prefix collides with `ruff`** (Rule 15). **Numbering is not strictly integer**: `§15A` sits between §15 and §16, and subsections `11.2A` and `D.3A` were inserted for 1.2.0 material. Appendices A-E (`7289-8231`) carry the sidecar skeleton, DTO schema, and reconciliation recipes.

| § | Line | Title | Key subsections |
|---|------|-------|-----------------|
| — | 1 | Front matter | the "do not compile-couple to Pyrefly internals" recommendation |
| — | 34 | **Source-of-truth hierarchy** | the "not intended for external use" warning on `pyrefly::query` |
| — | 122 | **Feature inventory** | CPG-concern × provider table: which layer Pyrefly should and should not own |
| — | 158 | Proposed comprehensive documentation map | the doc's own TOC |
| **0** | 203 | Scope, versioning, and Pyrefly mental model | 0.1 what Pyrefly is · 0.2 mental model for CPG integration · **0.3 bindings look like a CPG — do not persist them** · 0.4 "complete" has two meanings · 0.5 vocabulary · **0.6 agent invariants** · **0.7 1.2.0 release capability delta** |
| **1** | 611 | Installation, build, and deployment surfaces | **1.0 the four ways to run Pyrefly** (CLI · LSP · TSP · Rust Query) · 1.2 source-pinned sidecar workspace · 1.3 release build · 1.4 allocators · 1.5 threading · 1.6 first architecture smoke test |
| **2** | 795 | Workspace and crate architecture | 2.0 crate map · 2.1 `pyrefly_python` · 2.2 `pyrefly_types` · 2.3 `pyrefly_graph` · 2.4 `pyrefly_bundled` (typeshed) · **2.5 `pyrefly_config` — config is part of semantic identity** · 2.6 `pyrefly_build` + module resolver · 2.7 `pyrefly_glean_schema` |
| **3** | 1094 | Type-checker execution model: exports → bindings → answers | 3.0 canonical pipeline · **3.1 why module-centric design matters** · 3.2 exports · 3.3 bindings · 3.4 answers · 3.5 `Type::Var` · **3.6 flow types** |
| **4** | 1290 | Module identity, project config, search paths, environments | **4.0 module identity is semantic, not a filepath** (`FileId != ModuleId`) · 4.1 `ModuleName`/`ModulePath` · 4.2 config finder · 4.3 interpreter discovery (a security boundary) · 4.4 typeshed/site packages · 4.5 build-system integrations |
| **5** | 1437 | **Ruff parser dependency and the cross-version boundary** | **5.0 Pyrefly pins Ruff component crates 0.0.6** · **5.1 duplicate-type failure mode** · 5.2 best boundary: source snapshot + ranges · 5.3 in-process single-workspace option · **5.4 sidecar option — recommended** · 5.5 why TSP alone is not enough |
| **6** | 1585 | State, transactions, `Require`, epochs, incrementality | 6.0 state model · **6.1 `Require` levels** (Exports/Errors/Indexing/Everything) · 6.2 epoch model (`checked >= computed >= changed`) · 6.3 fine-grained dependency tracking · 6.4 `Query::change_files` · 6.5 file event model · **6.6 snapshot barrier** · 6.8 checklist |
| **7** | 1816 | Pyrefly type model (`pyrefly_types`) | 7.0 internal vs CPG type universe · 7.1 internal categories · **7.2 deduplicated type table — preferred 1.2.0 output** · 7.3 canonical type DTO · 7.4 type interning · 7.5 agent rules |
| **8** | 2228 | Flow-sensitive inference, narrowing, uncertainty | 8.0 why flow types matter · **8.1 declared, computed, expected** · 8.2 CPG edge model · 8.3 narrowing provenance · 8.4 flow joins · **8.5 unknown vs unresolved** · 8.6 1.2.0 behavioral changes |
| **9** | 2405 | Experimental Rust `pyrefly::query::Query` API | **9.0 status (experimental)** · 9.2 conceptual construction · 9.3 `add_files` · 9.4 `change_files` · **9.5 `get_type_table_in_file`** · 9.6 filtered extraction with `TypeQueryStmtWalker` · 9.7 range matching strategy |
| **10** | 2651 | Whole-file deduplicated type-table extraction | 10.0 bulk DTO · **10.1 why whole-file extraction is superior** · 10.2 dedup and structural hashes · 10.3 names vs non-name expressions · 10.5 type fact provenance · 10.6 validation checklist |
| **11** | 2832 | **Type-aware call/callee extraction** | 11.0 highest-value CPG feature · **11.1 `get_callees_with_location`** · 11.2 resolution behaviors · 11.2A 1.2.0 call-resolution implications · 11.3 CPG call edge schema · 11.4 multiple targets · **11.5 missing targets** · 11.7 target identity reconciliation · 11.8 decorators |
| **12** | 3082 | Class attributes, properties, member semantics | **12.0 `get_attributes`** · 12.1 CPG member model · 12.2 string annotation caveat · 12.3 inherited members · 12.4 descriptor semantics · 12.5 1.2.0 member/framework improvements |
| **13** | 3216 | Qualified targets and subtype queries | 13.0 `resolve_target_from_qualified_name` · **13.1 `is_subtype`** · 13.2 inheritance vs subtyping |
| **14** | 3301 | Type Server Protocol (TSP) | 14.0 why TSP matters · 14.2 transport · 14.4 core request methods · **14.5 TSP type representation** · 14.6 type flags · 14.7 declarations · 14.9 synthesized declarations · 14.11 snapshot notifications · **14.12 TSP strengths/weaknesses for CPG** |
| **15** | 3595 | LSP as a semantic query surface | 15.1 definition resolution · 15.2 references · 15.3 call hierarchy · 15.4 symbol indexes · 15.5 1.2.0 LSP changes relevant to CPG parity |
| **15A** | 3710 | Glean semantic/index export surface | 15A.1 useful fact families · **15A.2 best role in the architecture** · 15A.3 stability boundary · 15A.4 reconciliation rule |
| **16** | 3804 | **Query vs TSP vs LSP/Glean decision matrix** | a single table (3804-3846) + `### Recommended hybrid` — read before choosing a semantic surface |
| **17** | 3847 | What "complete Python CPG" can and cannot mean | 17.1 structural completeness · 17.2 semantic completeness · **17.3 dynamic incompleteness is a fact** · 17.4 soundness policy |
| **18** | 3985 | Division of responsibility: Ruff + Pyrefly + CPG | **18.0 responsibility table** · 18.1 do not duplicate the type checker · 18.2 do not outsource graph topology · 18.3 redundant facts can be useful |
| **19** | 4072 | Canonical CPG node/edge schema for Pyrefly enrichment | 19.0 core node kinds · **19.1 stable node identity** · 19.2 occurrence nodes · 19.3 type nodes · 19.4 core semantic edges · 19.5 provenance properties · 19.6 diagnostics as graph context |
| **20** | 4235 | Recommended fully Rust Python CPG architecture | 20.0 production topology · **20.1 why a sidecar is still "entirely Rust"** · 20.2 suggested crates · 20.3 one sidecar per workspace · 20.5 crash behavior |
| **21** | 4404 | Stable semantic-sidecar protocol design | 21.1 initialization · 21.2 load files · 21.3 file fact batch · 21.4 located type DTO · 21.5 callee DTO · 21.6 class DTO · 21.8 change request · **21.9 semantic generation** · 21.10 encoding choice |
| **22** | 4661 | Full-repository bootstrap/indexing workflow | 22.1 discover files · **22.2 immutable source digests** · 22.3 build syntax graph · 22.4 resolve module identities · 22.5 bulk load · 22.6 query semantic facts · 22.7 reconcile · **22.8 persist atomically** · 22.9 coverage metrics |
| **23** | 4846 | Incremental file-change workflow | 23.0 change pipeline · **23.1 separate structural and semantic invalidation** · 23.2 stable syntax IDs · 23.3 semantic edge replacement · 23.4 importer refresh · 23.5 semantic snapshots |
| **24** | 5025 | Coordinate/range synchronization across Ruff and Pyrefly | **24.0 canonical coordinate choice** · 24.1 canonical range DTO · 24.2 TSP/LSP conversion · 24.3 `PythonASTRange` · 24.4 range type/kind matching · **24.5 source digest assertion** |
| **25** | 5144 | Semantic identity, provenance, confidence, stale-fact prevention | **25.0 semantic facts are assertions, not timeless truths** · 25.1 workspace semantic environment ID · 25.2 confidence levels · 25.3 provenance ranking · 25.4 conflict example · 25.5 stale semantic state · 25.6 checklist |
| **26** | 5291 | Imports, modules, exports, re-exports, package identity | **26.0 import syntax and import semantics are different facts** · 26.1 import graph schema · 26.2 star imports · 26.3 re-exports · 26.4 module resolution through TSP · 26.5 namespace packages · 26.6 `.py` vs `.pyi` |
| **27** | 5465 | Inheritance, MRO, protocols, generics, dispatch | 27.1 MRO-aware dispatch · **27.2 persist MRO?** · 27.3 protocols · 27.4 generic classes · 27.6 overloads · 27.7 constructors · 27.8 callable objects |
| **28** | 5663 | Dynamic Python and uncertainty modeling | 28.0 `Any` · 28.1 `getattr` · 28.2 monkey patching · 28.3 `__getattr__` · 28.4 descriptors · 28.5 decorators · **28.6 dataclasses and framework synthesis** · 28.7 metaclasses · 28.8 `eval`/`exec` · 28.10 native extensions |
| **29** | 5858 | Error tolerance and partially invalid repositories | **29.1 two-layer availability** · 29.2 parse errors · 29.3 type errors · 29.4 sidecar panic · 29.5 timeouts · **29.6 unsupported feature status** |
| **30** | 5975 | Performance, batching, memory, thread pools, caching | **30.1 one long-lived `Query`** · 30.2 batch IPC · 30.3 1.2.0 type-table caching · **30.4 memory retention levels** · 30.6 thread pools · 30.9 1.2.0 performance changes |
| **31** | 6236 | Persistence and transactional CPG updates | **31.0 separate syntax and semantic partitions** · 31.1 generation table · 31.2 file fact replacement · 31.3 shared target nodes · 31.4 external symbol lifecycle |
| **32** | 6346 | Testing and correctness strategy | 32.0 test layers · 32.1-32.13 fixtures (type · call · union dispatch · decorator · property · generic · stub · incremental · config · unicode · invalid-source · type-table) · 32.14 golden output · 32.15 differential tests |
| **33** | 6614 | Observability and diagnostics | 33.0 workspace metrics · **33.1 fact coverage** · 33.2 latency · 33.3 query timing API · 33.5 health states |
| **34** | 6731 | Production deployment and security | 34.0 trust model · **34.1 do not execute repository Python for semantic analysis** · 34.2 filesystem sandbox · 34.3 sidecar resource limits · 34.5 message limits |
| **35** | 6848 | Upgrade/version migration discipline | 35.1 generic upgrade checklist · **35.2 required 1.1.1 → 1.2.0 adapter migration** · 35.3 protocol compatibility · 35.5 semantic drift |
| **36** | 7031 | Global anti-pattern inventory | single list (7031-7067) |
| **37** | 7068 | LLM-agent implementation checklist | 37.0 architecture · 37.1 bootstrap · 37.2 types · 37.3 calls · 37.4 symbols/members · 37.5 incrementality · 37.6 protocol/ranges · 37.7 robustness · 37.8 upgrades |
| **38** | 7185 | Reference links and source audit | 38.0-38.4 pinned upstream anchors |
| — | 7236 | Final implementation guidance for coding agents | condensed closing rules |
| A | 7289 | **Appendix A — Pinned-source Rust sidecar skeleton** | A.0 manifest strategy (7293) · A.2 module/path construction (7355) · A.4 bulk type-table adapter (7415) · A.5 bulk call adapter (7468) · A.6 class attributes (7500) · A.8 applying file changes (7549) |
| B | 7601 | **Appendix B — Application-owned Rust DTO schema** | B.1 source identity (7634) · B.2 byte ranges (7657) · B.3 canonical type (7688) · B.5 located type (7742) · B.6 callee (7756) · B.9 provenance (7819) · B.10 file facts (7834) |
| C | 7852 | **Appendix C — CPG reconciliation recipes** | C.0 per-file range index (7854) · C.1 attach a type fact (7871) · C.2 attach a call fact (7892) · C.4 class member reconciliation (7930) · C.6 CFG and flow types (7967) · C.7 def-use (7992) · C.9 import edges (8048) |
| D | 8084 | **Appendix D — Semantic fact coverage tiers** | D.0 Tier 0 syntax-only fallback (8086) · D.1 Tier 1 baseline Pyrefly (8105) · D.2 Tier 2 class/type ops (8117) · D.3 Tier 3 TSP detail (8129) · D.3A Tier 3A Glean (8145) · D.4 Tier 4 navigation (8158) |
| E | 8189 | Appendix E — Practical deployment decision | closing recommendation |

### §1.4 `rust_mir_cpg_continuous_reference_2026-08-18.md` — section index (§0-§58 + Appendices A-R)

5,207 lines. Front matter `1-283` (incl. the doc's own TOC at `92`, rendered as `## N)` headings — **these collide in shape with body subsections**, Rule 16); deep-dives start at line `284` with `# Rust MIR Advanced — 0)` and use `## N.M` subsections. Chapters are short (30-70 lines each); **the appendices are where the density is — `3002-5207` is 2,206 lines, 42% of the document.**

| § | Line | Title | Key subsections |
|---|------|-------|-----------------|
| — | 1 | Front matter | `## Version / source anchors` (7) · `## Architectural recommendation in one page` (39, two-lane pipeline diagram) · `## Feature inventory` (86) |
| — | 92 | **Proposed comprehensive documentation map** | `## 0)`-`## 58)`, one abstract each — **shape-identical to body subsections** |
| — | 273 | Suggested expansion order | 6 reading phases |
| **0** | 284 | Scope, versioning, and MIR mental model | 0.0 what MIR is · 0.1 what MIR contributes to a CPG · **0.2 recommended layered graph** · 0.3 stability stance |
| **1** | 351 | Toolchain installation and compiler-library setup | **1.0 required compiler components** (`rustc-dev`, `llvm-tools`, `rust-src`) · 1.1 extractor package · 1.2 build/smoke-test · 1.3 CI gate |
| **2** | 440 | First executable `rustc_public` MIR extractor | 2.0 minimal extraction shape · **2.1 do not return compiler objects** · 2.2 first CPG extraction target |
| **3** | 525 | Where MIR sits in the rustc pipeline | 3.0 compiler pipeline · **3.1 there is not one timeless MIR** · 3.2 recommended dual representation |
| **4** | 580 | **Choosing an extraction surface** | 4.0 `rustc_public` vs `rustc_private` vs dumps · **4.1 why not parse MIR text** · 4.2 `rustc_private` escape-hatch policy |
| **5** | 625 | Cargo metadata, packages, targets, compilation units | **5.0 Cargo is part of semantic correctness** · 5.1 CPG build-context identity · 5.2 unit of semantic invalidation |
| **6** | 673 | Compiler-wrapper integration with Cargo | **6.0 `RUSTC_WORKSPACE_WRAPPER`** · 6.1 wrapper contract · 6.2 side-channel output · 6.3 check mode |
| **7** | 722 | Crate and item discovery | 7.0 crate discovery APIs · 7.1 item facts · **7.2 owner-centric extraction** |
| **8** | 774 | MIR `Body` anatomy | 8.0 per-function IR container · 8.1 CPG owner mapping · **8.2 fingerprint boundary** |
| **9** | 821 | Locals, arguments, temporaries, debug-variable info | 9.0 local categories · 9.1 CPG representation · 9.2 continuous-update identity |
| **10** | 865 | Basic blocks and CFG fundamentals | **10.0 CFG contract** · 10.1 edge classes · 10.2 entry/exit modeling |
| **11** | 910 | Statements and state transitions | 11.0 public statement categories · 11.1 assignment lowering · 11.2 discriminants |
| **12** | 948 | Terminators, normal edges, unwind edges, calls | 12.0 terminator categories · **12.1 call terminator** · 12.2 `SwitchInt` · 12.3 cleanup semantics |
| **13** | 986 | Operands, constants, moves, copies | 13.0 `Operand` · **13.1 move vs copy is a semantic edge** · 13.2 function items as operands |
| **14** | 1029 | Places and projections | **14.0 `Place` is MIR's location vocabulary** · 14.1 canonical access-path key · 14.2 field names · **14.3 alias caution** |
| **15** | 1074 | Rvalues and value-production semantics | 15.0 public rvalue surface · **15.1 preserve lowering distinctions** · 15.2 typed expression layer |
| **16** | 1105 | Types, generics, traits, type normalization | 16.0 types are explicit in MIR · 16.1 type-node normalization · **16.2 generic definition vs concrete type** · 16.3 regions |
| **17** | 1146 | Source spans, files, macro expansion, source anchoring | 17.1 normalized source coordinates · **17.2 macro reality** · 17.3 stable file identity |
| **18** | 1185 | MIR visitor APIs and extraction passes | 18.0 `MirVisitor` · 18.1 visitor vs explicit walk · **18.2 public `PlaceContext` is intentionally coarse** |
| **19** | 1224 | Generic MIR versus monomorphized MIR | **19.0 two interprocedural views** · 19.1 CPG recommendation · 19.2 when concrete MIR matters |
| **20** | 1273 | **`Instance` resolution and concrete callable identity** | 20.0 `Instance` · 20.1 resolution pattern · **20.2 instance key** |
| **21** | 1309 | Direct call-edge extraction | 21.0 direct calls · 21.1 edge taxonomy · 21.2 external functions · **21.3 calls are not the full mono-use graph** |
| **22** | 1346 | Function pointers, closures, callable operands, indirect calls | 22.0 indirect callable categories · 22.1 function-pointer analysis · 22.2 closures |
| **23** | 1390 | Trait dispatch, vtables, unsizing, dynamic over-approximation | 23.0 vtable thinking · 23.1 CPG relations · **23.2 precision tiers** · 23.3 default methods and shims |
| **24** | 1439 | Drop glue, shims, intrinsics, compiler-generated uses | **24.0 hidden executable dependencies** · 24.1 drop edges · **24.2 keep use graph separate from call graph** |
| **25** | 1482 | Mapping MIR CFG into a CPG | 25.0 raw CFG facts · 25.1 statement-level expansion · 25.2 edge provenance |
| **26** | 1524 | Read/write/move/copy dataflow edges | 26.0 access classification · 26.1 event model · **26.2 mutability is not enough** |
| **27** | 1562 | Borrows, references, raw pointers, ownership edges | 27.0 reference creation · **27.1 lifetime/borrowck depth** · 27.2 CPG policy |
| **28** | 1607 | Place abstraction, alias domains, memory access paths | **28.0 access paths are not aliases** · 28.1 default CPG choice · 28.2 union/downcast caution |
| **29** | 1642 | Move paths, initialization, ownership-state overlays | 29.0 move/initialization analysis · 29.1 transfer examples · **29.2 exactness boundary** |
| **30** | 1687 | Reaching definitions, def-use, SSA-like overlays | **30.0 MIR is not SSA, but it is SSA-friendly** · 30.1 definitions and uses · 30.2 reaching definitions · 30.3 CPG edges |
| **31** | 1739 | Dominators, post-dominators, control dependence | **31.0 control dependence is derived** · 31.2 exceptional flow policy · 31.3 incremental scope |
| **32** | 1772 | Closures, captures, async functions, coroutines | 32.0 lowered callable forms · 32.1 closure graph · 32.2 async/coroutine graph · 32.3 call graph |
| **33** | 1808 | Constants, statics, CTFE, allocation references | 33.0 constants and statics · 33.1 graph nodes · 33.2 CTFE and allocations |
| **34** | 1842 | Unsafe operations, FFI, inline assembly, trust boundaries | **34.0 unsafe does not disappear in MIR** · 34.1 inline assembly · 34.2 FFI · 34.3 trust-boundary summaries |
| **35** | 1888 | Interprocedural summaries and scalable graph queries | 35.0 why summaries matter · 35.1 summary dependency graph · 35.2 incremental propagation |
| **36** | 1929 | Recommended Rust CPG schema | 36.0 node labels · 36.1 core edges · 36.2 provenance properties · **36.3 avoid graph-schema overfitting** |
| **37** | 1997 | **Stable identifiers across compiler sessions** | **37.0 raw `DefId` is not persistent identity** · **37.1 preferred identity hierarchy** (`StableCrateId` + `DefPathHash`) · 37.2 anonymous definitions · 37.3 instance identity |
| **38** | 2044 | Canonical serialization and semantic fingerprints | 38.0 why fingerprint normalized facts · 38.1 per-owner fingerprints · 38.2 canonicalization · **38.3 source-sensitive vs semantic-sensitive hashes** |
| **39** | 2093 | Extraction protocol, batching, transaction boundaries | **39.0 never stream compiler handles to storage** · 39.1 protocol envelope · 39.2 completion marker · 39.3 idempotence |
| **40** | 2144 | Cargo dependency graph and invalidation domains | 40.0 dependency domains · **40.1 inputs beyond `.rs`** · 40.2 dependency source strategy |
| **41** | 2179 | Continuous-update architecture | **41.0 recommended two-speed updater** · 41.1 snapshot states · 41.2 per-owner replacement · 41.3 cross-crate propagation |
| **42** | 2230 | Mapping file edits to crates, targets, semantic owners | 42.0 edit-to-owner mapping · 42.1 invalidation tiers · 42.2 do not pre-guess too aggressively |
| **43** | 2273 | Leveraging rustc incremental compilation and query reuse | **43.0 rustc already has an incremental query engine** · 43.1 what the daemon should do · 43.2 what not to assume |
| **44** | 2302 | Subgraph diffing and atomic graph replacement | **44.0 atomic replacement algorithm** · 44.1 tombstones · 44.2 edge ownership |
| **45** | 2346 | Debounce, scheduling, cancellation, backpressure | 45.0 scheduling model · 45.1 debounce · 45.2 cancellation · 45.3 backpressure · 45.4 priority |
| **46** | 2386 | Failure isolation, stale-state policy, recovery | **46.0 last-known-good semantics** · 46.1 recovery rules · **46.2 partial compilation** |
| **47** | 2416 | **Hybrid Tree-sitter + MIR code intelligence** | 47.0 why combine · **47.1 reconciliation keys** · 47.2 dual-edge provenance · 47.3 continuous-update win |
| **48** | 2470 | External crates, dependency summaries, source availability | 48.0 three dependency modes · 48.1 cache key · 48.2 source availability |
| **49** | 2503 | Performance engineering | 49.0 performance hierarchy · **49.2 monomorphization explosion** · 49.3 serialization |
| **50** | 2547 | Testing and golden validation | 50.0 fixture matrix · 50.1 golden artifacts · 50.2 incremental tests |
| **51** | 2605 | Nightly/API upgrades and compatibility gates | **51.0 nightly is an API dependency** · 51.1 upgrade procedure · 51.2 exhaustive matching policy · 51.3 public/private separation |
| **52** | 2643 | Observability and debugging | 52.1 MIR debug tools · 52.2 explainability |
| **53** | 2687 | Security and resource governance | **53.0 treat repositories as untrusted compiler input** · **53.1 build scripts and proc macros execute code** · 53.2 graph ingestion |
| **54** | 2725 | Production deployment patterns | 54.0 deployment patterns · 54.1 recommended consistency model |
| **55** | 2771 | Reference extractor / daemon architecture | 55.0 component architecture · 55.1 driver pseudocode · 55.2 normalizer API · 55.3 daemon loop |
| **56** | 2842 | CPG query recipes enabled by MIR | 56.0 blast radius · 56.1 ownership transfer · 56.2 mutation pathways · 56.3 error/panic path · 56.4 dynamic dispatch candidates |
| **57** | 2904 | Best practices, anti-patterns, agent invariants | **57.0 agent invariants** · **57.1 anti-pattern inventory** · 57.2 pre-commit checklist |
| **58** | 2957 | Capability gaps and future-facing design | **58.0 public-surface gaps are a design input** · 58.1 future-proofing rule · 58.2 migration path |
| A | 3002 | Appendix A — MIR-to-CPG mapping matrix | single table |
| B | 3027 | Appendix B — Suggested owned extraction schema | — |
| C | 3083 | **Appendix C — Call-resolution decision tree** | read before designing call extraction |
| D | 3122 | Appendix D — Continuous-update state machine | — |
| E | 3154 | Appendix E — Cargo orchestration commands | debugging only; flags vary by nightly |
| F | 3193 | Appendix F — Continuous invalidation cookbook | — |
| G | 3208 | Appendix G — Completeness checklist for Rust call/use graphs | — |
| H | 3228 | **Appendix H — Source authority matrix** | which surface owns which Rust fact |
| I | 3242 | Appendix I — Source anchors | upstream links |
| J | 3276 | **Appendix J — Concrete normalized MIR extraction model** | J.1 envelope/version tuple (3280) · J.2 build-context identity (3323) · **J.3 stable owner keys (3355)** · J.4 source anchors (3377) · J.5 MIR location key (3403) · J.6 owner fact (3427) · J.7 block/terminator facts (3453) · **J.8 place normalization (3492)** · J.9 access events (3529) · J.10 call facts (3564) |
| K | 3606 | **Appendix K — Exhaustive structured extraction rules** | K.1 pass ordering (3610) · K.2 assignment (3627) · K.5 `FakeRead`/`PlaceMention` (3704) · K.7 references/reborrows (3727) · K.9 aggregates (3762) · K.15 `SwitchInt` · K.17 `Drop` · K.18 `Call` · K.20 `InlineAsm` · K.21 `Abort`/`Resume`/`Unreachable` |
| L | 3908 | **Appendix L — Rust executable-use and call-graph construction** | L.1 edge taxonomy · L.3 fn item → fn pointer · L.5 closures · L.6 static trait dispatch · L.7 dynamic trait dispatch · L.9 drop glue · L.12 cross-crate generic instantiation · **L.13 completeness modes** · **L.14 resolution confidence vocabulary** |
| M | 4140 | **Appendix M — Continuous update algorithm, end-to-end** | M.1 daemon state · M.2 edit ingestion · M.3 debounce key · M.5 compile request · M.8 staleness test · **M.9 owner diff** · **M.10 graph transaction** · M.12 derived analysis invalidation · **M.13 compile failure** · M.16 workspace config change · M.17 full resync |
| N | 4386 | Appendix N — Graph-store schema and ownership conventions | N.1 definition table · N.2 MIR block table · N.4 CFG edges · N.5 access table · N.6 callsite table · N.7 call candidate table · N.8 instance table · **N.11 generation visibility** |
| O | 4580 | **Appendix O — Stable identity and matching under edits** | O.1 identity classes · O.2 named definitions · O.3 impl blocks · O.4 closures/generated owners · O.5 MIR local identity · **O.7 why owner-level replacement is a sweet spot** |
| P | 4688 | Appendix P — `rustc_private` enrichment adapter | P.1 adapter contract · P.2 stable IDs · P.4 borrow checker facts · P.5 monomorphization collector parity · **P.7 upgrade containment** |
| Q | 4791 | **Appendix Q — Validation fixture matrix** | Q.1-Q.22: direct call · fn ref without call · fn pointer merge · move vs copy · borrow/reborrow · enum switch · static/dynamic dispatch · drop glue · panic/unwind · closure capture · async · FFI · inline asm · macro expansion · incremental edit sequences · signature change · delete owner · broken edit recovery · **Q.22 nightly migration golden gate** |
| R | 5085 | **Appendix R — Implementation checklist for an LLM coding agent** | R.1-R.8 milestones (compiler integration → normalized owners → raw semantic layer → call precision → incremental updater → fast editor lane → derived analyses → hardening) · **R.9 non-negotiable stop conditions** |

---

## §2 — Cross-document authority matrix

Legend: ✅ authoritative · 🔁 cross-cut/summary · — not covered.
Aliases: `ts` = tree_sitter_rust_python · `ruff` = ruff_python_crates · `pyrefly` = pyrefly_rust_cpg · `mir` = rust_mir_cpg.

**This matrix is not a "who mentions it" table.** Where CodeFabric has already ruled, the `Authoritative` column states the project's decision and cites `fact-gen §5.1` (Python) or `§5.2` (Rust). Where the project has not ruled, it states which doc is the better *reference* and why. Two providers answering the same question with different answers is the normal case, not an error — the reconciler retains both and records the conflict (fact-gen §5.3).

### Python parsing and syntax representation

| Topic | ts | ruff | pyrefly | mir | Authoritative |
|-------|---:|------|---------|-----|----------------|
| Concrete syntax on **incomplete/broken** source | **§20, §45.8, §45.24** ✅ | §16 🔁 | §29 🔁 | — | **ts** — fact-gen §5.1 makes Tree-sitter primary here, Ruff's recovered parse the fallback |
| Typed Python AST on **parsable** source | §45 🔁 | **§5** ✅ | §3 🔁 (reparses internally) | — | **ruff §5** — fact-gen §5.1 makes Ruff AST primary, `ts` CST the fallback |
| Lexer/token stream, parse diagnostics | §21 🔁 | **§4** ✅ | — | — | **ruff §4** — incl. `parse_unchecked` (§4.7) and `ParseError` vs `UnsupportedSyntaxError` (§4.13) |
| Python-specific grammar shapes (decorators, comprehensions, `match`, f-strings) | **§45.11-§45.20** ✅ | §5.18 🔁 | — | — | **ts §45** for CST node shapes; **ruff §5** for the typed model. They disagree on span boundaries — `ts §45.11` is explicit that a decorator's definition span ≠ the decorated span |
| `.py` vs `.pyi` vs notebooks | §45.31 ✅ | §4.9, §5.2 ✅ | §26.6 ✅ | — | **ruff §5.2** (`PySourceType`) for dispatch; **pyrefly §26.6** for stub-vs-source semantics |
| AST → source regeneration / formatting | — | **§9, §10** ✅ | — | — | **ruff §9** (codegen) / **§10** (formatter) — out of scope for fact extraction but adjacent |

### Source coordinates, ranges, and encodings

| Topic | ts | ruff | pyrefly | mir | Authoritative |
|-------|---:|------|---------|-----|----------------|
| Byte-offset coordinate model | §7.4, §19.4 ✅ | **§3** ✅ | §24 ✅ | §17 ✅ | **ruff §3** is the canonical Python coordinate layer (`TextSize`/`TextRange`, `LineIndex`, `PositionEncoding`); each other doc defines its own and §24 of `pyrefly` is the reconciliation recipe |
| Range ↔ line/column ↔ LSP position | — | **§3.2, §3.3** ✅ | §24.2 ✅ | §17.1 🔁 | **ruff §3.3** — choose `PositionEncoding` explicitly; never count `char`s |
| Ranges across two independent parsers | §19.4 🔁 | §0.4 🔁 | **§24** ✅ | §47.1 ✅ | **pyrefly §24** for Ruff↔Pyrefly; **mir §47.1** for Tree-sitter↔MIR reconciliation keys |
| Encoding/input strategy (UTF-8, UTF-16, ropes, callbacks) | **§6** ✅ | §3.5 🔁 | — | — | **ts §6** — `§6.0` is an input-API decision tree |
| Range validity across revisions | §11 ✅ | **§0.4, §3.8** ✅ | §6.6, §24.5 ✅ | §38 🔁 | **ruff §0.4** states the invariant; **pyrefly §24.5** adds the source-digest assertion. See Operating Rule 5 |

### Comments, trivia, and omitted lexical facts

| Topic | ts | ruff | pyrefly | mir | Authoritative |
|-------|---:|------|---------|-----|----------------|
| Comments and their placement | §27.4 (extras) 🔁 | **§6.3, §7.3** ✅ | — | — | **ruff §6** — fact-gen §5.1: Ruff trivia/index primary, Tree-sitter extras fallback |
| Pragmas / suppression comments (`# noqa`, `# type: ignore`) | — | **§6.9** ✅ | — | — | **ruff §6.9** |
| Continuation lines, multiline/interpolated string ranges | — | **§7.4-§7.6** ✅ | — | — | **ruff §7** — precisely the facts the AST omits (`§7.0`) |
| Docstrings | §45.21 🔁 | **§5.14** ✅ | — | — | **ruff §5.14** — `ts §45.21` is explicit that Python has no docstring grammar node |
| Whether trivia should block an automated edit | — | **§6.12, §12.8** ✅ | — | — | **ruff §6.12** |

### Python names, scopes, bindings, references

| Topic | ts | ruff | pyrefly | mir | Authoritative |
|-------|---:|------|---------|-----|----------------|
| Lexical scopes and binding events | §45.17-§45.19 🔁 | **§8.5-§8.7** ✅ | §3.3 🔁 | — | **ruff §8** — fact-gen §5.1: Ruff semantic adapter primary, custom AST scope builder fallback |
| Reference classification and local resolution | §43.6 🔁 | **§8.9, §8.10** ✅ | §15.2 🔁 | — | **ruff §8.9** for syntax-local; escalate to `pyrefly` when cross-module |
| **Cross-module** definition/reference resolution | — | §8.11 🔁 | **§15, §15A, §26** ✅ | — | **pyrefly** — fact-gen §5.1: Glean/LSP/internal adapter primary, Ruff qualified-name resolution fallback |
| Qualified-name matching (`os.path.join`) | — | **§8.11** ✅ | §13.0 ✅ | — | **ruff §8.11** for in-process matching; **pyrefly §13.0** to resolve a qname to a *target* |
| Import syntax vs import semantics | §45.14 ✅ | **§8.14** ✅ | **§26** ✅ | — | Three-way split by design: `ts`/`ruff` give the syntax, **pyrefly §26.0** insists they are *different facts* and owns resolution, star imports, re-exports |
| Builtins and `typing`-module awareness | — | **§8.12, §8.13** ✅ | §2.4 (typeshed) ✅ | — | **ruff §8.13** for the linter model; **pyrefly §2.4** for bundled typeshed provenance |

### Python types

| Topic | ts | ruff | pyrefly | mir | Authoritative |
|-------|---:|------|---------|-----|----------------|
| Inferred/computed expression types | — | — | **§7, §10** ✅ | — | **pyrefly §10** (`get_type_table_in_file`) — fact-gen §5.1: Query type table primary, TSP fallback. **Ruff does not do type inference** (`ruff §0.1`) |
| Declared vs computed vs expected | — | §5.3 (annotation syntax) 🔁 | **§8.1, §14** ✅ | — | **pyrefly §14** (TSP) — fact-gen §5.1: TSP primary, Ruff annotation syntax + Query computed type fallback |
| Flow narrowing and `Any` semantics | — | — | **§8** ✅ | — | **pyrefly §8** — incl. `§8.5 unknown vs unresolved` |
| Type normalization into a durable schema | — | — | **§7.3, App. B.3** ✅ | §16.1 (Rust) ✅ | **pyrefly §7.3** — never persist `pyrefly_types::Type` variants directly |
| Subtype / assignability checks | — | — | **§13.1** ✅ | §16 🔁 | **pyrefly §13.1** (`is_subtype`) — a query-time oracle, not an all-pairs persisted relation |

### Python call targets and members

| Topic | ts | ruff | pyrefly | mir | Authoritative |
|-------|---:|------|---------|-----|----------------|
| Call **sites** (syntactic) | **§45.15** ✅ | §5.21 ✅ | §11.3 🔁 | — | **ruff §5** / **ts §45.15** — a call site is a first-class entity, distinct from its target |
| Call **targets** (resolved) | §43.11 (hybrid) 🔁 | — | **§11** ✅ | — | **pyrefly §11** — fact-gen §5.1: Query callees primary. `§11.5` governs *missing* targets |
| Class members, properties, descriptors, finality | §45.12 🔁 | §5 (syntax) 🔁 | **§12** ✅ | — | **pyrefly §12** — fact-gen §5.1: Query/TSP primary, Ruff class/decorator syntax fallback |
| Inheritance syntax vs MRO semantics | §45.12 ✅ | §5.1 ✅ | **§27** ✅ | — | Syntax from `ruff`/`ts`; **pyrefly §27** owns MRO, protocols, generics, overload dispatch |
| Framework-synthesized members (attrs, Pydantic, dataclasses) | — | — | **§28.6, §12.5** ✅ | — | **pyrefly §28.6** — these have no ordinary source declaration |

### Rust syntax and semantics

| Topic | ts | ruff | pyrefly | mir | Authoritative |
|-------|---:|------|---------|-----|----------------|
| Rust concrete syntax | **✅** (Rust grammar) | — | — | §47 🔁 | **ts** — fact-gen §5.2: Tree-sitter Rust grammar primary, rustc spans fallback |
| Semantic definitions, types, generics | — | — | — | **§7, §16** ✅ | **mir** — fact-gen §5.2: `rustc_public` primary, `rustc_private` secondary |
| Extraction-surface choice | — | — | — | **§4 + App. H** ✅ | **mir §4** — and never parse textual MIR dumps (`§4.1`) |
| Stable compiler identity | — | — | — | **§37 + App. O** ✅ | **mir §37.1** — `StableCrateId` + `DefPathHash`; fact-gen §5.2 puts the exact stable-key adapter in `rustc_private` |
| Macro expansion and source mapping | §45-style invocation syntax 🔁 | — | — | **§17.2** ✅ | **mir §17.2** — fact-gen §5.2: rustc source-map adapter primary, Tree-sitter invocation syntax fallback |
| Cargo/build-context identity | — | — | §4.5 (Python analogue) 🔁 | **§5, §6, §40** ✅ | **mir §5.1** — Cargo metadata is part of semantic correctness, not deployment trivia |

### MIR CFG, dataflow, and ownership

| Topic | ts | mir | Authoritative |
|-------|---:|-----|----------------|
| Basic blocks, edges, unwind/cleanup | — | **§10, §12** ✅ | **mir §10** — fact-gen §5.2: MIR is primary for CFG/state transitions; source syntax only for correspondence |
| Places, projections, access paths | — | **§14, §28 + App. J.8** ✅ | **mir §14** — and `§28.0`: access paths are *not* aliases |
| Read/write/move/copy events | — | **§13.1, §26 + App. J.9** ✅ | **mir §26** |
| Borrows, references, ownership state | — | **§27, §29** ✅ | **mir §27** — fact-gen §5.2 routes *exact* borrowck loan/region facts to `rustc_private` (`mir` App. P.4), conservative dataflow to the CPG |
| Def-use, reaching definitions | — | **§30** ✅ | **mir §30** — note `§30.0`: MIR is not SSA |
| Dominators, control dependence | — | **§31** ✅ | **mir §31.0** — derived, not a MIR fact; fact-gen §5.2 assigns derived analyses to CPG + petgraph |
| Call/instance resolution | — | **§20-§24 + App. L** ✅ | **mir §20** — `Instance` resolution is what makes an edge concrete; App. L.14 gives the confidence vocabulary |
| Drop glue, shims, intrinsics | — | **§24** ✅ | **mir §24.2** — keep the *use* graph separate from the *call* graph |

### Node identity and stable keys

| Topic | ts | ruff | pyrefly | mir | Authoritative |
|-------|---:|------|---------|-----|----------------|
| Why the provider's own ID is not identity | **§8.10, §11.3** ✅ | **§5.10** ✅ | **§0.3** ✅ | **§37.0** ✅ | All four say the same thing independently — see Operating Rule 4. Fact-gen §13 makes it normative |
| Deriving an application-owned key | §11.7, §43.8 ✅ | §14.3 ✅ | §19.1 ✅ | **§37.1 + App. O** ✅ | **mir §37.1 + App. O** is the most developed treatment (identity classes, impl blocks, closures, owner-level replacement) — read it even for the Python side |
| Occurrence vs entity identity | §43.9 🔁 | §14.2 🔁 | **§19.2** ✅ | §36.0 ✅ | **pyrefly §19.2** — occurrence nodes are first-class, not collapsed into their referent |
| Fingerprints for change detection | §23.9 (grammar) ✅ | §15.2 ✅ | §25.1 ✅ | **§38** ✅ | **mir §38.3** — distinguish source-sensitive from semantic-sensitive hashes |

### Incremental update and invalidation

| Topic | ts | ruff | pyrefly | mir | Authoritative |
|-------|---:|------|---------|-----|----------------|
| Sub-file incremental **parsing** | **§10** ✅ | §15.0 (states it does *not*) ✅ | — | — | **ts §10** — Ruff has no incremental parser; whole-file reparse is its boundary (`ruff §4.14`, `§15.0`) |
| Changed-range computation | **§11.0** ✅ | §15.5 🔁 | — | — | **ts §11** — `ruff §15.5` explicitly names Tree-sitter as the optional changed-range oracle |
| Semantic recheck and dependency impact | — | §15.8 🔁 | **§6.3, §23** ✅ | **§40-§43** ✅ | **pyrefly §6.3** (Python, fine-grained export categories) · **mir §42** (Rust, edit → owner mapping) |
| Cache hierarchy and derived invalidation | §11.4 ✅ | **§15.1-§15.3** ✅ | §30.3 ✅ | §43 ✅ | **ruff §15.1** for the per-file cache ladder |
| Atomic replacement / transaction boundary | §41.7 ✅ | §14.4 ✅ | §22.8, §31 ✅ | **§44 + App. M.10** ✅ | **mir §44 + App. M** is the most complete algorithm; `pyrefly §31.0` adds the syntax/semantic partition split |
| Debounce, scheduling, backpressure, cancellation | §18 ✅ | §15.6, §18.3 ✅ | — | **§45** ✅ | **mir §45** · **ts §18** for parse/query deadlines |

### Broken source, error recovery, and capability gaps

| Topic | ts | ruff | pyrefly | mir | Authoritative |
|-------|---:|------|---------|-----|----------------|
| Producing *something* from unparsable source | **§20** ✅ | **§4.7, §16** ✅ | §29.2 🔁 | — | **ts §20** (always returns a tree) · **ruff §4.7** (`parse_unchecked`) — two different mechanisms for the same need |
| Error node taxonomy | **§20.2** (ERROR vs MISSING) ✅ | **§4.13** (ParseError vs UnsupportedSyntaxError) ✅ | §29.6 ✅ | §46.2 ✅ | Each doc owns its own taxonomy; do not flatten them into one "invalid" state |
| Partial-extraction policy | **§20.5, §20.10** ✅ | **§16.2-§16.4** ✅ | **§29.1** ✅ | **§46.0** ✅ | Four independent statements of "keep the last known good snapshot and mark it degraded" — `ruff §16.4` and `mir §46.0` are the crispest |
| Compile failure → capability gap | — | — | §29.6 ✅ | **§46.2 + App. M.13** ✅ | **mir App. M.13** — a failed compile must not publish stale-current compiler facts |
| Uncertainty as a first-class fact | §20.8 🔁 | §8.22 (confidence classes) ✅ | **§17, §28** ✅ | **§23.2, §58** ✅ | **pyrefly §17.3** (dynamic incompleteness is a fact) · **mir §23.2** (precision tiers) · **mir §58** (capability gaps are a design input) |

### Structural queries and pattern matching

| Topic | ts | others | Authoritative |
|-------|---:|--------|----------------|
| Compiled structural matchers over syntax | **§12-§17** ✅ | — | **ts** — this subsystem has no counterpart in the other three. `§12.1` compile-once/execute-many, `§16` predicates, `§17` cursor containment |
| Query-vs-traversal choice | **§9.7** ✅ | — | **ts §9.7** |
| Ready-made extraction queries for Python | **§45.27-§45.30** ✅ | — | **ts §45.27** (definitions/imports/calls/owners) — `§45.29` warns the upstream `TAGS_QUERY` is intentionally small |

### Provider isolation, versioning, and deployment

| Topic | ts | ruff | pyrefly | mir | Authoritative |
|-------|---:|------|---------|-----|----------------|
| Adapter/DTO boundary requirement | §3.4, §34.3 ✅ | **§1.3, §11** ✅ | **§20, §21 + App. B** ✅ | **§2.1, §39.0** ✅ | Normative in fact-gen §7.1-§7.4. **pyrefly App. B** is the most complete worked DTO schema |
| Version pinning discipline | **§40** (ABI axes) ✅ | **§20** (release train) ✅ | **§35** (semantic drift) ✅ | **§51** (nightly as API dep) ✅ | Each doc owns its own axis — they are genuinely different problems (ABI window · crate release train · behavior drift · nightly pin) |
| Cross-version type-universe collisions | §40.7 🔁 | §1.4 (`cargo tree -d`) ✅ | **§5.1** ✅ | §51.3 🔁 | **pyrefly §5.1** — the duplicate-type failure mode, and the reason for the sidecar |
| Process/crash/allocator isolation | §35, §36 ✅ | §18.6 🔁 | **§20.5, §1.4** ✅ | §46, §53 ✅ | **pyrefly §20** for the sidecar rationale; **mir §53.1** for the sharper Rust hazard — build scripts and proc macros *execute code* |
| Untrusted-input threat model | **§42** ✅ | **§21** ✅ | **§34** ✅ | **§53** ✅ | Read the one matching the provider you are deploying. `ruff §21.0` (parsing ≠ executing) and `mir §53.1` (compiling *does* execute) are the two poles |
| Concurrency model | **§36** ✅ | **§18** ✅ | **§1.5, §30.6** ✅ | §45, §49 ✅ | **ts §36.3** (one parser per concurrent parse) · **ruff §18.0** (concurrency axis is files) · **pyrefly §1.5** (do not fan out over Pyrefly's own scheduler) |

---

## §3 — Decision trees

Four of these adapt a decision aid that already exists upstream — `ruff §22`, `pyrefly §16`, `mir` App. C, `mir` App. H. Read the upstream section when the tree here is not enough.

### Which provider answers this question?

```
What does the source literally say, in Python, on parsable source?
  -> ruff §5 (typed AST) + §4 (tokens)                       (fact-gen §5.1)
What does the source say when it is mid-edit or broken?
  -> ts §20 (CST with ERROR/MISSING nodes)                   (fact-gen §5.1)
What does a Python name refer to, within one file?
  -> ruff §8 (scopes, bindings, references)
What does a Python name refer to, across modules?
  -> pyrefly §26 (imports/exports) + §15/§15A (defs/xrefs)
What is the type of this Python expression?
  -> pyrefly §10 (Query type table); §14 (TSP) for declared/expected
What does this Python call actually target?
  -> pyrefly §11 (get_callees_with_location)
What does Rust source say syntactically?
  -> ts (Rust grammar)                                        (fact-gen §5.2)
What does Rust mean semantically — types, CFG, dataflow, calls?
  -> mir §8-§34                                               (fact-gen §5.2)
How do I find all occurrences of a syntactic shape?
  -> ts §12-§17 (queries) — no counterpart in the other three
How do I keep any of this current as files change?
  -> ts §11 (changed ranges) · pyrefly §23 · mir §41-§46 + App. M
Anything about traversal, SCCs, dominators over the finished graph?
  -> NOT here. petgraph (docs/library_ref/petgraph.md; fact-gen §7.5)
```

### Python syntax: tree-sitter or Ruff?

Adapts `ruff §22.0` ("Which crate do I reach for?") and `§22.5` (the agent-safe "do not confuse" table).

```
Is the source guaranteed parsable (committed, CI, post-validation)?
  -> Ruff. Typed AST is richer and is the project's primary  (fact-gen §5.1)
Is the source live-edited / possibly broken?
  -> Tree-sitter for structure that survives                 (ts §20)
  -> Ruff parse_unchecked in parallel for typed facts        (ruff §4.7)
Do you need sub-file incremental reparse or changed ranges?
  -> Tree-sitter. Ruff has no incremental parser             (ruff §4.14, §15.0; ts §10, §11)
Do you need comments / trivia / continuation lines / f-string internals?
  -> Ruff §6 (trivia) + §7 (index) — the facts the AST omits (ruff §7.0)
Do you need to match a structural pattern across a file?
  -> Tree-sitter queries                                     (ts §12-§17)
Do you need byte<->line/column or LSP positions?
  -> Ruff §3 (LineIndex, SourceCode, PositionEncoding)
Do you need both engines on one file?
  -> Fine, and expected. Join on (file, content_digest, byte range).
     Never share parser objects across the boundary.         (pyrefly §5.2, §24)
```

### Where does a Python type fact come from?

Adapts `pyrefly §16`, which is a full matrix — read it before committing.

```
Bulk inferred types for every expression in a file?
  -> Query::get_type_table_in_file                            (pyrefly §9.5, §10)
  -> Filter with TypeQueryStmtWalker if you only persist some (pyrefly §9.6)
Declared vs computed vs expected, as separate facts?
  -> TSP                                                      (pyrefly §14, §8.1)
Is A a subtype of B?
  -> Query::is_subtype — query-time oracle, do not persist all pairs (pyrefly §13.1)
Resolve a qualified name to a target?
  -> Query::resolve_target_from_qualified_name                (pyrefly §13.0)
Class members / properties / descriptors?
  -> Query::get_attributes                                    (pyrefly §12.0)
Bulk declarations and cross-references for a whole repo?
  -> Glean adapter (optional bulk index)                      (pyrefly §15A)
Go-to-definition / find-references / call hierarchy?
  -> LSP                                                      (pyrefly §15)
Any of the above, persisted?
  -> Convert to application-owned DTOs first. Response-local
     type indices are not product identity.                   (pyrefly §7.3, App. B)
```

### Rust facts: which extraction surface?

Adapts `mir §4` and Appendix H (source authority matrix).

```
Syntax only — spans, node kinds, structural shape?
  -> Tree-sitter Rust grammar                                 (fact-gen §5.2)
Semantic definitions, types, MIR bodies, CFG, calls?
  -> rustc_public                                             (mir §4.0, §7-§24)
A fact rustc_public does not expose (exact borrowck loans,
detailed PlaceContext, monomorphization collector parity)?
  -> narrowly scoped rustc_private adapter                    (mir §4.2, App. P)
  -> contain it: P.7 upgrade containment
Textual MIR dumps?
  -> NEVER as a machine interface. Diagnostics only.          (mir §4.1)
Which nightly?
  -> exact date-pinned nightly + rustc-dev + rust-src + llvm-tools (mir §1.0)
  -> NOT this repo's current pin. Adopting it is an
     architectural decision.                                  (repo spec §76)
```

### Which call-resolution path?

`mir` Appendix C is already a call-resolution decision tree; Appendix L.14 gives the confidence vocabulary.

```
PYTHON
  Syntactic call site (always emit — it is a first-class entity)
    -> ruff §5.21 / ts §45.15
  Resolved target
    -> pyrefly §11 get_callees_with_location                  (fact-gen §5.1)
  Zero targets returned?
    -> NOT proof of no target. Emit an explicit unknown.      (pyrefly §11.5)
  Multiple targets (union receiver, overload, decorator)?
    -> multi-candidate fact + confidence                      (pyrefly §11.4, §11.8)

RUST
  Direct call terminator
    -> mir §21, resolve via Instance                           (mir §20)
  Function pointer / closure / callable operand
    -> mir §22 — indirect, over-approximate
  Trait object / dynamic dispatch
    -> mir §23 — declare a precision tier, do not pretend exactness
  Drop glue / shims / intrinsics
    -> mir §24 — these are *uses*, keep them out of the call graph (mir §24.2)
  Cross-crate generic instantiation
    -> mir App. L.12; completeness modes at L.13
Confidence is part of the fact, not a comment.                 (mir App. L.14)
```

### Source doesn't parse, or the crate doesn't compile

```
Python file has a syntax error
  -> ts still returns a tree (ERROR/MISSING nodes)             (ts §20.0-§20.2)
  -> ruff parse_unchecked still returns recovered syntax+tokens (ruff §4.7)
  -> extract only facts whose ranges are trustworthy           (ts §20.5; ruff §16.2)
  -> mark the file's syntax state degraded; keep prior good facts (ruff §16.4)
Python file parses but violates the target Python version
  -> UnsupportedSyntaxError, NOT a grammar failure. Separate state. (ruff §4.13, §16.1)
Pyrefly cannot type a file / sidecar panics / times out
  -> two-layer availability: syntax stays, semantics degrade    (pyrefly §29.1, §29.4, §29.5)
Rust crate fails to compile
  -> capability gap. Do NOT publish stale-current compiler facts. (mir §46.2, App. M.13)
  -> last-known-good semantics with an explicit staleness marker (mir §46.0)
In every case
  -> the absence of a fact is itself a fact. Materialize an
     explicit unknown or capability gap; never an empty result
     that reads as "none".                                      (SKILL invariant 3)
```

### How do I get a stable ID for X?

```
Rust definition / owner
  -> StableCrateId + DefPathHash                                (mir §37.1)
  -> raw DefId is NOT identity                                  (mir §37.0)
  -> anonymous owners (closures, impls, generated): mir App. O.3-O.4
Rust MIR local / basic block
  -> positional, snapshot-local. Do not persist as identity.    (mir App. O.5-O.6)
  -> owner-level replacement is the sweet spot                  (mir App. O.7)
Python definition
  -> module identity + qualified lexical path + kind            (pyrefly §19.1; ruff §14.3)
  -> NOT Ruff AtomicNodeIndex                                   (ruff §5.10)
  -> NOT a Pyrefly binding key or response-local type index     (pyrefly §0.3)
Syntax occurrence (any language)
  -> (file_id, content_digest, byte range, kind) + ordinal      (ts §11.7, §43.8)
  -> NOT a tree-sitter node id — reuse across reparse is not identity (ts §11.3)
Anything crossing a snapshot boundary
  -> must carry (file_id, content_digest, generation)           (pyrefly §24.5, §25)
```

### Incremental update: what invalidates what?

```
A file's bytes changed
  -> new immutable snapshot; replace the whole per-file bundle  (ruff §11.1; ts §6.7)
  -> ts: apply InputEdit, reparse, take changed_ranges          (ts §10, §11.0)
  -> changed_ranges are structural and may be conservative      (ts §11.1, §11.2)
  -> expand to semantic/owner boundaries before invalidating    (ts §11.5, §43.7)
  -> ruff: whole-file reparse (no incremental parser)           (ruff §15.0)
A Python module's exports changed
  -> let Pyrefly compute dependency impact; refresh only
     affected modules. Do not invalidate every importer.        (pyrefly §3.2, §6.3)
A Rust file changed
  -> map edit -> crate -> target -> semantic owner              (mir §42.0)
  -> invalidation tiers; do not pre-guess too aggressively      (mir §42.1-§42.2)
  -> rustc already has an incremental query engine; use it      (mir §43.0)
Config / interpreter / search path / typeshed / feature flags changed
  -> global semantic invalidation. This changes the meaning of
     unchanged source.                                          (pyrefly §2.5, §4; mir §40.1, App. M.16)
Committing the result
  -> owner-scoped replacement, one atomic transaction           (mir §44, App. M.10)
  -> never mix a syntax snapshot from generation N+1 with
     semantic facts from generation N                           (pyrefly §6.6)
```

---

## §4 — Operating rules

Rules 1-14 are semantic. **Rules 15-18 are meta-rules about navigating these four documents** — read them before your first grep.

1. **Each provider owns a different question.** Ruff = what the source *says*; Pyrefly = what it *means* statically; tree-sitter = what survives a *broken* edit and what matches a *pattern*; MIR = what Rust *compiles to*. Asking the wrong provider yields a confident wrong answer, which is the failure this skill exists to prevent. (`ruff §0.5`; `pyrefly §18`; `mir §47`)

2. **Every provider sits behind an application-owned adapter emitting application-owned DTOs.** No long-lived `Node<'tree>` leaves a tree-sitter adapter (`ts §7.1`, `§34.4`); no `rustc_public`/`rustc_private` object escapes the compiler callback or crosses a thread (`mir §2.1`, `§39.0`); Ruff `0.0.x` types stay inside the Ruff adapter (`ruff §1.3`, `§11`); Pyrefly sits behind a process boundary (`pyrefly §20`). This is normative — fact-gen §7.1-§7.4.

3. **Absence is never proof of absence.** Zero callees is not "no target" (`pyrefly §11.5`); a failed compile is a **capability gap**, not an empty fact set (`mir §46.2`, App. M.13); an unparsable region yields explicit unknowns, not silence (`ts §20.5`). Materialize the uncertainty.

4. **Canonical identity is application-owned.** Tree-sitter node IDs (`ts §8.10`, `§11.3`), Ruff `AtomicNodeIndex` (`ruff §5.10`), Pyrefly binding keys and response-local type indices (`pyrefly §0.3`), and raw `DefId` (`mir §37.0`) are all snapshot-local. Rust prefers `StableCrateId` + `DefPathHash` (`mir §37.1`); everything else derives from source semantics plus provenance. All four docs state this independently — that agreement is the signal.

5. **A byte range is meaningless without the content digest it was computed against.** `(file_id, content_digest, start, end)` is the unit of provenance. A range from revision N applied to N+1 can select the wrong text or slice mid-code-point. Replace the whole per-file bundle on change rather than translating ranges. (`ruff §0.4`, `§3.8`; `pyrefly §24.5`; `ts §6.7`)

6. **Raw and normalized kinds must coexist.** Keep the provider-native kind (`ts` node kind string, Ruff node type, MIR statement/terminator variant) *alongside* your normalized kind. Normalization must never block representing a grammar or compiler variant you have not seen yet. (`ts §7.7`; `mir §15.1`; fact-gen §12)

7. **A syntax occurrence is not a semantic entity.** Call syntax is not a callable; type syntax is not a type; import syntax is not a resolved module. Call sites are first-class entities, not merely caller→callee edges. (`pyrefly §26.0`, `§19.2`; `ts §45.15`)

8. **Conflicting provider facts are resolved by authority, never by silent overwrite.** Choose the canonical fact per fact-gen §5.1/§5.2 (mirrored in §2 above), retain the conflicting evidence in provenance/diagnostics, and emit an unknown or multi-candidate fact when the conflict is unresolvable. (fact-gen §5.3; `pyrefly §25.4`)

9. **Ruff 0.0.7 and Pyrefly's bundled 0.0.6 are distinct Rust type universes.** `ruff_python_ast(0.0.6)::Expr` ≠ `ruff_python_ast(0.0.7)::Expr` despite identical names. Never pass AST nodes across the boundary — share a source snapshot and ranges. Run `cargo tree -d` before debugging a type mismatch. (`pyrefly §5.0`, `§5.1`; `ruff §1.4`)

10. **`Require::Everything` only for files you will bulk-extract.** Transitive dependencies that only need exports get `Require::Exports`. This asymmetry is the main memory control in a Pyrefly sidecar; one long-lived `Query`/`State` per workspace, not one per request. (`pyrefly §6.1`, `§30.1`, `§30.4`)

11. **Tree-sitter's ABI window is the real compatibility gate, and assignment to a `Parser` is where it is checked.** `LANGUAGE_VERSION = 15` / `MIN_COMPATIBLE_LANGUAGE_VERSION = 13` for 0.26.12. Runtime version, grammar crate version, and ABI version are independent axes; numeric node/field IDs are not cross-version contracts. (`ts §40.0`, `§40.2`, `§40.7`, `§4.1`)

12. **Neither Ruff nor rustc gives you sub-file incremental parsing; tree-sitter does.** Ruff's boundary is whole-file reparse (`ruff §4.14`, `§15.0`); rustc's is the crate/owner (`mir §42`). If you need edit-local structure, tree-sitter is the fast lane and the other two are the deep lane — which is exactly the two-speed architecture in `mir §41.0` and `§47`.

13. **MIR extraction requires a date-pinned nightly plus `rustc-dev`, which this repo deliberately does not declare.** `rust-toolchain.toml` pins stable; nightly exists only for `just miri` / `just udeps`. Treat `mir` as design input until that is decided explicitly. Also: MIR is not SSA (`mir §30.0`), `Instance` resolution is what makes call edges concrete (`mir §20`), and dynamic dispatch is an over-approximation with declared precision tiers (`mir §23.2`). (repo spec §76; CLAUDE.md *Environment*)

14. **Parsing source does not execute it; compiling source does.** `ruff §21.0` and `ts §42.1` can treat untrusted input as a resource-exhaustion problem. `mir §53.1` cannot — build scripts and proc macros execute arbitrary code, and `pyrefly §34.1` warns against executing repository Python for interpreter discovery. The threat models are not interchangeable.

15. **`ruff` and `pyrefly` share the bare `# N) ` deep-dive prefix.** A `rg '^# [0-9]+\)'` returns 26 hits from `ruff` and 40 from `pyrefly` — 66 across two files that look identical in the output. `ts` (`# Tree-sitter Advanced — N) `) and `mir` (`# Rust MIR Advanced — N) `) are unique. **Always scope a heading grep to one file path.**

16. **`ts` and `mir` render their internal tables-of-contents as `## N) Title` headings, shape-identical to body subsections.** `rg '^## 25\)'` on `mir` returns the TOC entry at line 169, not the §25 body at 1482 (the body heading is `# Rust MIR Advanced — 25)`). Match `^## [0-9]+\.` for body subsections and `^# .* — [0-9]+\)` for chapters. `mir` has 59 such TOC entries and `ts` has 46, so this inflates every naive `##` count.

17. **Code-fence comments shadow `^# ` headings.** `# Cargo.toml` (`ts` 452, 468), `# rust-toolchain.toml` (`mir` 14, 371), `# noqa` / `# fmt: skip` (`ruff` 2000, 2165-2168), `# pkg/__init__.py` (`pyrefly` 5367) all match a naive heading grep. Verify a hit is a real heading — or filter fence state — before citing a line number.

18. **Use `lib-outline`, not `spec-outline`, on these files — and prefer it over `rg` for headings.** `lib-outline` (`scripts/lib-outline.sh`, `just lib-outline`, extractor `tooling/ast-grep/outline/library-ref.yml`) maps `item: '# $TITLE'` / `member: '## $TITLE'`, which fits these h1-rooted references; `spec-outline` maps h2/h3 for the specs in `docs/upfront_design/` and now refuses a `library_ref` path outright rather than emitting a flat, appendix-less wall. Two reasons to reach for it over a heading grep: it parses markdown, so fenced `# Cargo.toml` lines cannot masquerade as chapters (Rule 17), and TOC entries attach as members of the doc-map item rather than colliding with body subsections (Rule 16). Typical use: `lib-outline <file>` for the chapter map, then `--match '^Appendix M' --view expanded` to zoom. `--match` filters *items*, not members, so zoom to the chapter and expand. Note the rendered `NNNN:` prefix is 1-based (feed it to `Read`) while `range.start.line` in `--json` is 0-based. Whole-directory output is ~1,670 lines against 11.9 MB of source. Still remember CLAUDE.md's warning that unscoped `rg` over `docs/library_ref/` swamps results (`-g '!docs/library_ref/**'`).

---

## §5 — Project context: CodeFabric

**CodeFabric is pre-implementation.** One Cargo package, one library crate, and a seed whose only job was to prove the toolchain. There is no provider adapter code yet — so unlike the sibling library-ref skills, there is no crate→document map to give. These four docs are read *against the specs*, and the useful mapping is fact-domain → doc.

### Fact domain → document

| Fact domain | Primary | Supporting |
|---|---|---|
| Source bytes, coordinates, ranges | **ruff** §3 (Python) | **ts** §6 (input/encodings) · **mir** §17 (Rust spans, macros) |
| Concrete syntax, incomplete edits, changed ranges | **ts** §7-§11, §20 | — |
| Python tokens, comments, trivia, omitted lexical facts | **ruff** §4, §6, §7 | **ts** §45.20-§45.21 |
| Python typed syntax, scopes, bindings, references | **ruff** §5, §8 | **ts** §45.15-§45.19 |
| Python types (computed / declared / expected / narrowed) | **pyrefly** §7, §8, §10, §14 | — |
| Python call targets, members, MRO, imports | **pyrefly** §11, §12, §26, §27 | **ruff** §8.11, §8.14 (syntax + qnames) |
| Rust semantic defs, types, MIR, CFG, dataflow, ownership | **mir** §8-§34 | **ts** (Rust CST) |
| Rust call/use graph | **mir** §20-§24 + App. L | — |
| Identity, provenance, confidence, unknowns | **mir** §37-§38 + App. O · **pyrefly** §25 | **ts** §11.7 · **ruff** §14.3 |
| Incremental invalidation and atomic publication | **mir** §41-§46 + App. M | **ts** §11 · **pyrefly** §23 · **ruff** §15 |
| Derived analyses (traversal, SCC, dominators) | **not here** → `petgraph.md` | **mir** §31 for what MIR does *not* derive |

### Spec anchors

`present_state_cpg_fact_generation_specification_python_rust_v1.3.md` is the governing spec. Its §2 source basis names five library references — these four plus `petgraph.md`, routed by the `petgraph-ref` sibling — alongside the ontology specification as a companion artifact:

| Spec section | What it fixes | Mirrored here |
|---|---|---|
| §5.1 | Python authority order, per fact family | §2 tables, `Authoritative` column |
| §5.2 | Rust authority order, per fact family | §2 tables, `Authoritative` column |
| §5.3 | Conflict policy — retain evidence, never silently overwrite | Operating Rule 8 |
| §7.1 | Tree-sitter adapter: no long-lived `Node<'tree>` | Operating Rule 2 |
| §7.2 | Ruff adapter: all `0.0.x` types stay inside | Operating Rule 2, 9 |
| §7.3 | Pyrefly sidecar: process/DTO boundary, and why | Operating Rule 2, 9 |
| §7.4 | rustc adapter: nothing escapes the callback | Operating Rule 2 |
| §12 | Raw and normalized facts coexist | Operating Rule 6 |
| §13 | Canonical semantic identity is application-owned | Operating Rule 4 |

**Cite specs by section number, not line number.** The spec filenames carry a version suffix that moves between releases and their line numbers shift with every revision; section numbers have held. Confirm a citation with `spec-outline <spec>.md --match '^5\.'` before trusting it. For the four references, the matching tool is `lib-outline` (Rule 18). `docs/spec_index/` carries a verified cross-reference layer over the whole design suite, including a spec-section-to-library-chapter map.

### Boundary discipline

The doctrine in CLAUDE.md that these four docs exist to serve: **fact substrate, not judgment.** These providers emit facts and mechanically derived facts. None of them should be asked to produce `SAFE_TO_REFACTOR`, `HIGH_RISK`, complexity verdicts, or test-impact conclusions — and where a provider offers something evaluative (a lint severity, a diagnostic level), it enters the graph as a *fact about what the provider said*, with provenance, not as a judgment.

**Rule of thumb:** building a provider adapter, or reasoning about what a provider can and cannot tell you → this skill. Normalizing, storing, or serving the resulting facts → **`deltalake-rust-ref`** and **`datafusion-pyarrow-rust-ref`**. Running graph algorithms over the finished result → `docs/library_ref/petgraph.md`, which no skill currently routes.
