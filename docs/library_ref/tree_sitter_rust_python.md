# Tree-sitter in Rust — advanced technical reference / feature-category catalog

**Updated Python-parsing edition:** includes a dedicated language-specific chapter for parsing Python source from the native Rust Tree-sitter host using `tree-sitter-python`.

Modeled after the supplied DataFusion-in-Rust advanced reference: this document starts with a version-pinned capability map and then expands the map into self-contained technical chapters. The target is **native Rust-hosted Tree-sitter** using the `tree-sitter` Rust crate and Rust grammar crates. It is **not** a guide to `py-tree-sitter`, not a guide to calling a Rust grammar from Python, and not a claim that Tree-sitter's parser runtime is implemented in pure Rust.

## Version / source anchors

This document is pinned to `tree-sitter` **0.26.12**, the current released Rust binding at the time of writing. The crate describes itself as the Rust bindings to the Tree-sitter parsing library. The Rust crate provides the ownership/lifetime-safe host API (`Parser`, `Tree`, `Node`, `Query`, `QueryCursor`, etc.); the core parser runtime that the binding drives is Tree-sitter's C implementation. Grammar crates such as `tree-sitter-json` or `tree-sitter-javascript` supply generated `LanguageFn` values; they are language definitions, not alternate runtimes. ([tree-sitter Rust API][rustdoc]) ([Tree-sitter guide][getting-started])

ABI anchors for `tree-sitter` 0.26.12:

```text
LANGUAGE_VERSION                = 15
MIN_COMPATIBLE_LANGUAGE_VERSION = 13
```

A runtime accepts generated grammars whose ABI lies in that supported window; Tree-sitter is generally backward-compatible with older supported grammar ABIs but not forward-compatible with grammar ABIs newer than the runtime. ([LANGUAGE_VERSION][language-version]) ([MIN_COMPATIBLE_LANGUAGE_VERSION][min-language-version])

Example grammar anchors used only to make snippets executable:

```toml
[dependencies]
tree-sitter = "=0.26.12"
tree-sitter-json = "0.24.8"
tree-sitter-python = "=0.25.0"
# Optional examples:
# tree-sitter-javascript = "0.25.0"
```

`tree-sitter-json` 0.24.8 exposes `LANGUAGE: tree_sitter_language::LanguageFn`, plus `HIGHLIGHTS_QUERY` and `NODE_TYPES`. The core examples convert that `LanguageFn` into `tree_sitter::Language` and execute all parsing/query operations through the native Rust `tree-sitter` API. ([tree-sitter-json][json-crate])

For Python-source parsing from Rust, this edition additionally pins upstream `tree-sitter-python` **0.25.0**. That grammar crate exposes `LANGUAGE`, `NODE_TYPES`, `HIGHLIGHTS_QUERY`, and `TAGS_QUERY`; its Rust package compiles the generated native parser/scanner through `bindings/rust/build.rs`. The host remains `tree-sitter` Rust—`tree-sitter-python` is the Python grammar package. ([tree-sitter-python][python-crate]) ([tree-sitter-python Cargo.toml][python-cargo])

---

## Feature inventory: what the reference covers

The native Rust surface naturally breaks into: grammar/runtime separation; Cargo and grammar-crate integration; parser lifecycle; contiguous and callback-based input; UTF-8/UTF-16/custom decoding; concrete syntax trees; node identity, fields, ranges, named/anonymous/extra/error/missing nodes; efficient cursor traversal; incremental edits and changed ranges; Tree-sitter queries; captures, predicates, directives, quantification, alternation, anchors, and range-limited query execution; included ranges and multi-language orchestration; parse-state lookahead; grammar metadata and static `node-types.json`; grammar creation and generated Rust bindings; grammar DSL, precedence, conflicts, lexing, and external scanners; highlighting/tags companion crates; WASM grammars; raw FFI and allocator hooks; concurrency; cancellation/progress callbacks; performance; observability/DOT logging; testing; version migration; production architecture; security/resource controls; and code-intelligence-specific recipes; plus a Python-specific native-Rust layer covering `tree-sitter-python`, Python indentation/scanner behavior, encodings, functions/classes/decorators, imports, assignments/bindings/scopes, comprehensions, pattern matching, f-strings, type syntax, query packs, incremental invalidation, and Python-version/semantic boundaries.

---

# Proposed comprehensive documentation map

## 0) Scope, versioning, and Tree-sitter mental model
Define precisely what the Rust crate is, the C-runtime boundary, `LanguageFn` vs `Language`, and the canonical parse/query pipeline.

## 1) Installation, crate selection, and Rust project layout
Cargo pinning, grammar dependencies, workspace patterns, feature flags, build tools, and native compilation expectations.

## 2) First executable Rust app
Minimal JSON parse, node inspection, tree printing, traversal, and a first query.

## 3) Architecture: Rust host, C runtime, generated grammar
FFI boundary, generated parser/scanner code, ownership, build/link path, and what is actually Rust vs C.

## 4) Languages, grammar crates, `LanguageFn`, ABI, and metadata
Loading grammar crates, `Language::new`, ABI checks, semantic metadata, node/field/supertype introspection, language registries.

## 5) `Parser` lifecycle and state
`Parser::new`, `set_language`, parser reuse, reset, logging, included ranges, WASM store, and parser-pool patterns.

## 6) Text input and encodings
`parse`, callback input, UTF-8, UTF-16 LE/BE, chunked text, ropes/piece tables, `Decode`, and source retention.

## 7) Syntax tree model: `Tree`, `Node`, and CST semantics
Concrete vs abstract syntax, root nodes, source-free trees, node lifetimes, named/anonymous/extra nodes, immutability and edits.

## 8) Node inspection API
Kinds, grammar names/IDs, fields, child APIs, siblings, parent navigation, byte/point ranges, text extraction, parse states.

## 9) Efficient tree traversal
`TreeCursor`, iterator-backed child traversal, DFS/BFS patterns, pruning, descendant lookup, hot-path guidance.

## 10) Incremental parsing
`InputEdit`, `Tree::edit`, reparse with old tree, editor change conversion, multiple edits, full replacement fallbacks.

## 11) Changed ranges, identity, and cache invalidation
`Tree::changed_ranges`, `Node::id`, reuse semantics, stale-node hazards, invalidation scopes, incremental indexes.

## 12) Query mental model and compilation
`Query::new`, language binding, pattern/capture metadata, query errors, compile-once/share-many architecture.

## 13) Query syntax
Node patterns, fields, negated fields, anonymous tokens, wildcards, `ERROR`, `MISSING`, supertypes/subtypes.

## 14) Query operators
Captures, `+`/`*`/`?`, groups, alternations, anchors, quantified captures, capture quantifiers.

## 15) Query matches and captures in Rust
`QueryCursor::matches`, `captures`, `QueryMatch`, `QueryCapture`, `StreamingIterator`, text providers, result materialization.

## 16) Query predicates and directives
`eq?`, `match?`, `any-of?`, property predicates/settings, general predicates, binding-vs-CLI handling responsibilities.

## 17) Query cursor controls and containment
Byte/point ranges, containing ranges, max start depth, match limits, overflow detection, cursor reuse.

## 18) Parse/query progress and cancellation
`ParseOptions`, `ParseState`, `QueryCursorOptions`, budget checks, cancellation, deadlines, cooperative admission control.

## 19) Included ranges and multi-language documents
`Range`, `set_included_ranges`, host/injected language orchestration, coordinate contracts, injection-query patterns.

## 20) Error recovery and incomplete code
`ERROR`, missing nodes, `has_error`, `is_error`, `is_missing`, editor tolerance, diagnostic extraction.

## 21) Parse states and lookahead
`parse_state`, `next_parse_state`, `Language::next_state`, `lookahead_iterator`, completion candidates and recovery diagnostics.

## 22) Static node types and typed wrappers
Generated `node-types.json`, grammar constants, type generation, field/cardinality metadata, schema drift checks.

## 23) Language introspection
Node kinds, fields, visibility/named flags, supertypes/subtypes, parse-state counts, ABI/semantic metadata.

## 24) Grammar crate packaging and build mechanics
`parser.c`, scanner C/C++, Rust `build.rs`, `bindings/rust/lib.rs`, `tree-sitter-language`, static linking.

## 25) Creating a grammar
`tree-sitter init`, repository layout, `tree-sitter.json`, generation, parsing, binding generation, publication.

## 26) Grammar DSL
`seq`, `choice`, `optional`, `repeat`, `repeat1`, strings/regexes, `token`, `token.immediate`, `alias`, `field`, `reserved`.

## 27) Precedence, associativity, conflicts, and GLR behavior
`prec`, `prec.left/right`, `prec.dynamic`, conflicts, ambiguity, LR(1)-friendly structure, conflict diagnostics.

## 28) Lexing and keyword extraction
Context-aware lexing, lexical precedence, longest match, string-vs-regex specificity, `word`, extras.

## 29) External scanners
Scanner lifecycle, `valid_symbols`, serialization, allocator integration, included ranges, C/C++ build/link constraints.

## 30) Grammar testing and correctness
Corpus tests, `tree-sitter test`, update mode, invalid-input tests, platform/language attributes, regression strategy.

## 31) Syntax highlighting from Rust
`tree-sitter-highlight`, `HighlightConfiguration`, highlighter-per-thread, injection callback, event streams.

## 32) Tags and code navigation from Rust
`tree-sitter-tags`, tag/local queries, definitions/references, symbol ranges, limitations vs semantic analysis.

## 33) WASM grammar execution in Rust
`wasm` feature, `WasmStore`, Wasmtime engine, loading grammar bytes, native-vs-WASM tradeoffs.

## 34) Raw FFI and ownership escape hatches
`ffi`, `from_raw`/`into_raw`, unsafe preconditions, cross-language integration, lifecycle rules.

## 35) Allocation and memory model
Tree sharing, parser/query/cursor allocation, `set_allocator`, external-scanner allocator reuse, source-text ownership.

## 36) Concurrency and sharing patterns
Clone trees per concurrent mutation domain, immutable query sharing, parser/cursor pools, per-thread highlighters, locking policy.

## 37) Performance tuning
Incremental edits, cursor traversal, query range restriction, query/cursor reuse, avoiding text copies, grammar complexity, measurement.

## 38) Observability and debugging
Parser logger, `LogType`, S-expressions, DOT graphs, query diagnostics, timing, counters, error-node instrumentation.

## 39) Error handling and diagnostics
`LanguageError`, `QueryError`, `IncludedRangesError`, WASM errors, `Option<Tree>` cancellation/no-language semantics, contextual errors.

## 40) API stability and upgrades
Runtime/grammar ABI, semantic grammar metadata, query compatibility, crate/API drift, upgrade tests.

## 41) Production deployment patterns
Editor service, batch indexer, language-server subsystem, repository scanner, incremental code-intelligence daemon, multi-tenant parsing service.

## 42) Security and resource governance
Untrusted source, adversarial grammar/query complexity, deadlines, query match limits, WASM trust boundary, grammar supply chain.

## 43) Code-intelligence recipes
Symbols, imports, calls, scopes, incremental file index, change-local query reruns, graph extraction, hybrid syntactic/semantic pipelines.

## 44) Best practices and anti-patterns
Consolidated agent rules, architecture invariants, failure modes, code-generation checklist.

## 45) Parsing Python source with native Rust Tree-sitter
`tree-sitter-python` setup and commands; Rust-hosted Python parsing; Python external-scanner/indentation behavior; source encoding; Python CST and field contracts; functions/classes/decorators/parameters; imports; bindings/scopes/comprehensions/pattern matching; f-strings/docstrings/type syntax; Python query packs; incremental indexing; CLI validation; semantic limitations; testing; and upgrade gates.

---

# Suggested expansion order

1. **Sections 0–6:** mental model, installation, first app, runtime/grammar split, language and parser/input APIs.
2. **Sections 7–11:** trees, nodes, traversal, incremental editing, invalidation.
3. **Sections 12–18:** query language, query execution, predicates, ranges, cancellation.
4. **Sections 19–23:** multi-language parsing, errors, lookahead, static/introspective grammar metadata.
5. **Sections 24–30:** grammar packaging, authoring, DSL, lexing/scanners, tests.
6. **Sections 31–40:** highlighting/tags, WASM, FFI, memory, concurrency, performance, diagnostics, upgrades.
7. **Sections 41–44:** production, governance, code-intelligence recipes, consolidated best practices.
8. **Section 45:** Python-specific native-Rust parsing, extraction, incremental indexing, commands, and version/semantic considerations.

---

# Tree-sitter Advanced — 0) Scope, versioning, and mental model

## 0.0 Version anchors and documentation stance

Use **docs.rs for exact released Rust signatures** and the official Tree-sitter book/source for semantic behavior. This reference is anchored to `tree-sitter = 0.26.12`. The official crate landing page identifies 0.26.12 and exposes the modern Rust surface including `ParseOptions`, `ParseState`, custom decoding, query cursor controls, and the optional WASM runtime. Main-branch guide material can evolve; pin the crate and grammar crates used in a production workspace. ([tree-sitter Rust API][rustdoc])

```toml
[workspace.dependencies]
tree-sitter = "=0.26.12"
tree-sitter-json = "=0.24.8"
```

For grammar compatibility, distinguish **crate semver** from **grammar ABI** and **grammar semantic version metadata**:

```text
Rust crate version:      tree-sitter 0.26.12
Runtime grammar ABI:     accepts ABI 13..=15
Grammar crate version:   e.g. tree-sitter-json 0.24.8
Grammar metadata:        LanguageMetadata { major_version, minor_version, patch_version }
```

A grammar crate version and runtime crate version need not numerically match. The Rust ecosystem deliberately uses `tree-sitter-language::LanguageFn` as the narrow handoff from a generated grammar crate into the runtime, and `Language::new(LanguageFn)` / `From<LanguageFn>` construct the runtime `Language`. ([LanguageFn][language-fn]) ([Language][language])

## 0.1 Identity: what native Rust Tree-sitter is / is not

**Tree-sitter is a parser generator plus an incremental parsing library.** In a Rust application, the `tree-sitter` crate is the idiomatic Rust binding/ownership layer around Tree-sitter's native parser runtime. Generated grammars are separate compiled artifacts. ([Getting Started][getting-started])

| Axis | Tree-sitter gives you | You still provide |
|---|---|---|
| Parsing | Error-tolerant concrete syntax tree construction | Language grammar dependency and source bytes |
| Incrementality | Edit old tree + reparse with structural reuse | Correct edit coordinates and source buffer mutation |
| Syntax navigation | Nodes, fields, ranges, cursors | Domain interpretation and semantic model |
| Pattern matching | Query language + Rust query execution | Query definitions, capture semantics, result model |
| Multi-language | Included ranges, injection-friendly APIs | Language detection and orchestration policy |
| Semantics | Syntactic structure and parse states | Name resolution, types, control/data flow, macro expansion, compiler semantics |
| Grammar authoring | CLI, DSL, GLR/LR parser generation | Grammar design, conflicts, external scanners, tests |

**Agent invariant:** do not describe the Rust crate as a pure-Rust implementation of the parsing engine. Correct phrasing is: **“The application executes Tree-sitter through its native Rust bindings; the binding drives Tree-sitter's native C parser runtime and generated grammar code.”**

## 0.2 Canonical architecture

```text
Rust application
  ├─ source buffer: String / Vec<u8> / rope / piece table
  ├─ grammar crate: tree_sitter_foo::LANGUAGE : LanguageFn
  │     └─ generated parser.c (+ optional scanner.c/scanner.cc)
  └─ tree_sitter crate
        ├─ Language
        ├─ Parser
        │    └─ native Tree-sitter runtime
        │         └─ Tree
        │              └─ Node / TreeCursor
        └─ Query
             └─ QueryCursor
                  └─ QueryMatch / QueryCapture
```

The four foundational parser objects in the official API are language, parser, syntax tree, and syntax node. The Rust binding then adds Rust-specific wrappers and richer query/input/lifetime APIs. ([Getting Started][getting-started])

## 0.3 Canonical parse pipeline

```text
LanguageFn
  → Language::new / .into()
  → Parser::set_language(&language)
  → Parser::parse(source, old_tree)
  → Tree
  → Tree::root_node()
  → Node navigation / TreeCursor traversal
  → Query + QueryCursor (optional extraction layer)
```

Minimal executable form:

```rust
use tree_sitter::{Language, Parser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let language: Language = tree_sitter_json::LANGUAGE.into();
    let mut parser = Parser::new();
    parser.set_language(&language)?;

    let source = r#"{"name":"tree-sitter","ok":true}"#;
    let tree = parser.parse(source, None).expect("parse was cancelled");

    let root = tree.root_node();
    println!("{}", root.to_sexp());
    assert!(!root.has_error());
    Ok(())
}
```

`Parser::parse` returns `Option<Tree>` rather than `Result<Tree, _>` because cancellation/progress control can terminate parsing; language assignment errors are handled by `set_language`. ([Parser][parser])

## 0.4 Minimum vocabulary

| Term | Meaning | Use |
|---|---|---|
| `LanguageFn` | Small function-pointer wrapper exported by a generated grammar crate | Grammar/runtime handoff |
| `Language` | Runtime handle describing one generated grammar | Assign to parser; compile queries; introspect grammar |
| `Parser` | Stateful parsing engine instance | Parse/reparse source; configure language/ranges/logger |
| `Tree` | Parsed CST for an entire input domain | Root node, edits, changed ranges, clone/share boundary |
| `Node<'tree>` | Lightweight view into a tree node | Inspect kind/fields/ranges/children/errors |
| `TreeCursor<'tree>` | Stateful efficient traversal cursor | Repeated structural navigation without repeated lookup |
| `Point` | Zero-based `{row, column}` position | Coordinate half of edits/ranges |
| `Range` | Byte + point start/end range | Included ranges, changed ranges, query coordination |
| `InputEdit` | Old/new edit geometry | Keep old tree synchronized before incremental reparse |
| `Query` | Immutable compiled S-expression patterns tied to a language | Extract structural features |
| `QueryCursor` | Stateful query executor | Match/capture iteration, ranges, limits |
| `ParseState` | Progress state exposed during parsing | Cooperative cancellation, telemetry |
| `LookaheadIterator` | Valid-symbol iterator for a parse state | Completion/recovery diagnostics |

## 0.5 CST, not AST

Tree-sitter constructs a **concrete syntax tree**. Named nodes generally correspond to named grammar rules; anonymous nodes generally correspond to literal tokens. Punctuation and keywords can therefore remain structurally present even when an application chooses to consume only named nodes. Extra nodes (commonly comments) are separately identifiable. ([Node][node])

```rust
let root = tree.root_node();
println!("all children: {}", root.child_count());
println!("named children: {}", root.named_child_count());
```

Do not build a semantic system that assumes `named_child_count == child_count`. Use named nodes when you want grammar-level constructs; use all nodes when exact syntax/token structure matters.

## 0.6 Tree/source separation

A `Tree` stores syntactic structure and positions, **not a durable copy of the original source text**. APIs such as `Node::utf8_text` require the caller to supply the corresponding source bytes. Consequently, a production document state normally couples source and tree explicitly:

```rust
struct ParsedDocument {
    source: Vec<u8>,
    tree: tree_sitter::Tree,
}
```

This separation is valuable for editor integrations because the source buffer can be a rope/piece table while Tree-sitter stores structural state; it also means nodes cannot magically recover lexeme text after the application discards or mutates the source without maintaining coordinate consistency.

## 0.7 Incremental mental model

```text
old source S0 + old tree T0
          |
          | edit E
          v
new source S1
old tree T0 --Tree::edit(E)--> coordinate-adjusted T0'
          |
          | Parser::parse(S1, Some(&T0'))
          v
new tree T1 with internal structure reused where possible
```

Tree-sitter's advanced parsing guide describes this as the two essential steps: edit the old tree so its coordinates agree with the new document, then parse the new source while providing that edited old tree. ([Advanced Parsing][advanced-parsing])

## 0.8 Query mental model

```text
query source (S-expression patterns)
  → Query::new(&language, source)
  → immutable Query
  → QueryCursor::new()
  → matches/captures(root_node, text_provider)
  → QueryMatch / QueryCapture
```

Queries are grammar-specific. A query that compiles against one grammar revision can fail against another if node kinds or fields change. `LanguageMetadata` exists in part so applications can detect grammar version changes that may invalidate queries. ([Query][query]) ([Language][language])

## 0.9 LLM-agent invariants

1. **Grammar is not runtime.** `tree-sitter-json::LANGUAGE` supplies a generated grammar; `tree_sitter::Parser` executes it.
2. **Tree is not source text.** Keep the authoritative bytes/rope separately.
3. **Positions have two coordinate systems.** Incremental edits require byte offsets and row/column points.
4. **Edit before reparse.** Mutate source and old tree consistently, then pass the edited tree as `old_tree`.
5. **Nodes borrow trees.** Do not design caches around long-lived `Node<'tree>` handles across tree replacement.
6. **Query is grammar-specific.** Compile/query-test against the exact grammar family you ship.
7. **Query cursor is mutable execution state.** Reuse one cursor serially; use separate cursors for concurrent query executions.
8. **Syntax is not semantics.** Tree-sitter alone does not resolve names/types/calls with compiler precision.
9. **Error-tolerant parse is still useful.** `ERROR` and missing nodes are first-class recovery artifacts; do not discard an entire tree because `has_error()` is true.
10. **Current Rust API matters.** Many internet examples target older 0.20–0.24 APIs; verify current docs.rs signatures.

## 0.10 Anti-pattern inventory

* Calling `tree-sitter-rust` “the Rust implementation of Tree-sitter.” It is a Rust-language grammar.
* Treating `tree_sitter::Tree` as a self-contained source file.
* Reusing an old tree after source mutation without `Tree::edit`.
* Keeping `Node` handles as durable IDs across reparses.
* Walking huge child lists with repeated `node.child(i)` when cursor/iterators are more appropriate.
* Compiling the same `Query` for every file/request.
* Running whole-tree queries after every keystroke when a changed-range-limited query is sufficient.
* Assuming `root.has_error()` means the tree is unusable.
* Using query captures as a substitute for semantic name resolution.
* Sharing one mutable parser/cursor across concurrent requests without synchronization/pooling.

---

# Tree-sitter Advanced — 1) Installation, crate selection, and Rust project layout

## 1.0 Source-of-truth stance

Pin the runtime independently from grammar crates. Grammar crates often publish on their own cadence; the compatibility contract is the Tree-sitter grammar ABI plus the `LanguageFn` bridge, not numerical semver equality. The current runtime ABI window is 13 through 15. ([Language version][language-version])

## 1.1 Minimal `Cargo.toml`

```toml
[package]
name = "ts-native-rust"
version = "0.1.0"
edition = "2024"

[dependencies]
tree-sitter = "=0.26.12"
tree-sitter-json = "=0.24.8"
```

`tree-sitter-json` is only a compact example grammar. Replace/add grammar crates for the languages your application analyzes.

## 1.2 Grammar/runtime version rule

Good:

```toml
tree-sitter = "=0.26.12"
tree-sitter-json = "0.24.8"
tree-sitter-javascript = "0.25.0"
```

Do **not** force grammar versions to equal the core runtime version. Instead, instantiate the `Language`, call `set_language`, and fail fast if the generated grammar ABI is incompatible.

```rust
use tree_sitter::{Language, Parser, LANGUAGE_VERSION, MIN_COMPATIBLE_LANGUAGE_VERSION};

let language: Language = tree_sitter_json::LANGUAGE.into();
println!("grammar ABI={}", language.abi_version());
println!("runtime ABI window={}..={}", MIN_COMPATIBLE_LANGUAGE_VERSION, LANGUAGE_VERSION);

let mut parser = Parser::new();
parser.set_language(&language)?;
```

## 1.3 Why `tree-sitter-language` exists

`tree_sitter_language::LanguageFn` wraps the generated C grammar entry function. Grammar crates export this tiny value rather than owning a particular `tree_sitter::Language` type from a specific runtime crate version. The application then converts it into the `Language` type of the runtime it actually uses. ([LanguageFn][language-fn])

```rust
let language = tree_sitter::Language::new(tree_sitter_json::LANGUAGE);
// equivalent ergonomic path:
let language2: tree_sitter::Language = tree_sitter_json::LANGUAGE.into();
```

This greatly reduces “two versions of `tree-sitter` mean two incompatible Rust types” problems that older grammar bindings could encounter.

## 1.4 Workspace pattern

```toml
# Cargo.toml
[workspace]
members = [
  "crates/syntax-core",
  "crates/code-indexer",
  "crates/cli",
]
resolver = "2"

[workspace.dependencies]
tree-sitter = "=0.26.12"
tree-sitter-json = "=0.24.8"
tree-sitter-javascript = "=0.25.0"
```

```toml
# crates/syntax-core/Cargo.toml
[dependencies]
tree-sitter = { workspace = true }
tree-sitter-json = { workspace = true }
tree-sitter-javascript = { workspace = true }
```

Recommended library boundary:

```text
syntax-core/
  src/
    lib.rs
    language.rs      # language registry + grammar metadata
    parser_pool.rs   # parser construction / pooling
    document.rs      # source + tree + edit application
    traversal.rs     # cursor walkers
    queries.rs       # compiled query registry
    extract.rs       # capture -> domain record mapping
    diagnostics.rs   # ERROR/MISSING handling
    incremental.rs   # changed-range invalidation
```

## 1.5 Feature flags

The core crate's important user-facing feature split is:

```text
std   default; enables normal standard-library integrations and errors/debug graph helpers
wasm  enables WasmStore / Wasmtime-backed grammar execution
```

WASM grammar execution is opt-in:

```toml
tree-sitter = { version = "=0.26.12", features = ["wasm"] }
```

For normal native grammar crates, do not enable `wasm` merely because Tree-sitter can use it; native generated parsers are simpler and usually the expected path.

## 1.6 Native build expectations

Although application code is Rust, the Tree-sitter runtime and generated grammars contain C (and occasionally an external C++ scanner). Cargo builds these native components through build scripts/`cc`. A machine/container compiling from source therefore needs a functioning native compiler toolchain.

Typical production CI checks:

```bash
cargo check --workspace
cargo test --workspace
cargo build --release
cargo tree -d
```

If a grammar has an external C++ scanner, ensure the build environment has the C++ toolchain/runtime expected by that grammar.

## 1.7 Feature facade for generated/agent code

A small facade reduces dependency/path drift:

```rust
// syntax-core/src/prelude.rs
pub use tree_sitter::{
    InputEdit, Language, Node, Parser, Point, Query, QueryCursor, Range,
    StreamingIterator, Tree, TreeCursor,
};

pub type TsResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
```

Agent rule: centralize grammar construction in one module instead of sprinkling `tree_sitter_foo::LANGUAGE.into()` across the repository.

## 1.8 Language registry pattern

```rust
use tree_sitter::Language;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum LanguageId {
    Json,
    JavaScript,
}

pub fn language(id: LanguageId) -> Language {
    match id {
        LanguageId::Json => tree_sitter_json::LANGUAGE.into(),
        LanguageId::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
    }
}
```

If query sets are also registered, bind them to the same logical language entry and test them together.

## 1.9 Production dependency checklist

```text
[ ] Pin core tree-sitter runtime.
[ ] Pin/lock grammar crates independently.
[ ] Test Parser::set_language for every shipped grammar.
[ ] Record grammar LanguageMetadata when present.
[ ] Compile all shipped Query strings in CI.
[ ] Ensure native C/C++ build tools are present.
[ ] Enable wasm only if the product actually loads Wasm grammars.
[ ] Centralize language/query registry in one crate.
[ ] Run cargo tree -d and inspect duplicate grammar/runtime families when APIs cross crate boundaries.
```

---

# Tree-sitter Advanced — 2) First executable Rust app

## 2.0 Objective

Prove the complete native Rust-hosted path:

```text
grammar LanguageFn
  → tree_sitter::Language
  → Parser
  → source bytes
  → Tree
  → Node inspection
  → TreeCursor traversal
  → Query / QueryCursor
```

## 2.1 Minimal parse

```rust
use tree_sitter::{Language, Parser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let language: Language = tree_sitter_json::LANGUAGE.into();
    let mut parser = Parser::new();
    parser.set_language(&language)?;

    let source = br#"{"answer": 42, "active": true}"#;
    let tree = parser.parse(source, None).expect("parse cancelled");
    let root = tree.root_node();

    println!("kind={}", root.kind());
    println!("range={:?}", root.range());
    println!("sexp={}", root.to_sexp());
    println!("has_error={}", root.has_error());

    Ok(())
}
```

## 2.2 Named vs anonymous children

For a JSON object, punctuation is represented by anonymous syntax nodes while pairs/strings/numbers are named grammar nodes.

```rust
let root = tree.root_node();
let object = root.named_child(0).expect("object");

println!("children={}", object.child_count());
println!("named_children={}", object.named_child_count());

for i in 0..object.child_count() {
    let child = object.child(i).unwrap();
    println!(
        "{} named={} bytes={:?}",
        child.kind(),
        child.is_named(),
        child.byte_range(),
    );
}
```

## 2.3 Extract node text

```rust
let bytes = source.as_slice();
let object = tree.root_node().named_child(0).unwrap();
for child in object.named_children(&mut object.walk()) {
    let text = child.utf8_text(bytes)?;
    println!("{} => {text}", child.kind());
}
```

The tree stores ranges; `utf8_text` slices caller-supplied source. Keep the source that corresponds to the tree.

## 2.4 TreeCursor DFS

```rust
fn print_tree(node: tree_sitter::Node<'_>, depth: usize) {
    println!("{}{} {:?}", "  ".repeat(depth), node.kind(), node.byte_range());

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            print_tree(cursor.node(), depth + 1);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}
```

For hot traversal, prefer one cursor-driven iterative DFS (shown later) rather than recursively creating a fresh cursor per node.

## 2.5 First query

```rust
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let language: Language = tree_sitter_json::LANGUAGE.into();
    let mut parser = Parser::new();
    parser.set_language(&language)?;

    let source = br#"{"name":"Ada","age":42,"ok":true}"#;
    let tree = parser.parse(source, None).unwrap();

    let query = Query::new(
        &language,
        r#"
        (pair
          key: (string (string_content) @key)
          value: (_) @value)
        "#,
    )?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_slice());

    while let Some(m) = matches.next() {
        println!("pattern={}", m.pattern_index);
        for capture in m.captures {
            let name = &query.capture_names()[capture.index as usize];
            let text = capture.node.utf8_text(source)?;
            println!("  @{name}: {text}");
        }
    }

    Ok(())
}
```

The Rust query iterators are streaming iterators because returned matches borrow cursor-owned execution storage. Import `tree_sitter::StreamingIterator` and consume each match before asking the cursor for the next one.

## 2.6 First incremental edit

```rust
use tree_sitter::{InputEdit, Point};

let old_source = br#"{"x":1}"#.to_vec();
let mut tree = parser.parse(&old_source, None).unwrap();

// Insert `0` after `1`: {"x":10}
let edit = InputEdit {
    start_byte: 6,
    old_end_byte: 6,
    new_end_byte: 7,
    start_position: Point::new(0, 6),
    old_end_position: Point::new(0, 6),
    new_end_position: Point::new(0, 7),
};

tree.edit(&edit);
let new_source = br#"{"x":10}"#;
let new_tree = parser.parse(new_source, Some(&tree)).unwrap();

for range in tree.changed_ranges(&new_tree) {
    println!("changed: {range:?}");
}
```

The exact edit geometry must describe the source mutation. In a real editor integration, derive it from the text-buffer edit rather than hand-coding positions.

## 2.7 First error-tolerant parse

```rust
let source = br#"{"x": [1, 2, }"#;
let tree = parser.parse(source, None).unwrap();
let root = tree.root_node();

assert!(root.has_error());
println!("{}", root.to_sexp());
```

A syntax error does not mean parsing failed. The resulting CST contains recoverable structure plus error/missing nodes that let editor/indexing features continue operating.

## 2.8 First-app anti-patterns

* Calling `unwrap()` on `set_language` in library code without context about the grammar/ABI.
* Parsing a `String`, discarding it, then expecting nodes to provide source text later.
* Recursively calling `node.child(i)` over enormous lists in a performance-sensitive indexer.
* Treating `has_error()` as equivalent to “no usable structure.”
* Building a new `Parser` and recompiling queries for every file in a batch job.
* Forgetting `StreamingIterator` when iterating query matches/captures.

---

# Tree-sitter Advanced — 3) Architecture: Rust host, C runtime, generated grammar

## 3.0 Layer model

```text
application Rust
    |
    | safe-ish ergonomic API + lifetimes
    v
crate tree-sitter (binding_rust)
    |
    | FFI
    v
Tree-sitter native core (C)
    |
    +---- generated grammar parser.c
    |
    +---- optional scanner.c / scanner.cc
```

The official Tree-sitter build guide describes the core library as a C library and identifies `lib/src/lib.c` plus its include directories as sufficient to embed the runtime. The Rust crate packages/builds that runtime and exposes idiomatic Rust wrappers. ([Getting Started][getting-started])

## 3.1 What the Rust wrapper owns

Representative wrappers:

```text
Parser      -> NonNull<TSParser>
Tree        -> NonNull<TSTree>
Language    -> *const TSLanguage
Node<'tree> -> TSNode + PhantomData tying it to Tree
Query       -> native query handle + Rust-side predicate metadata
QueryCursor -> native cursor + Rust execution state
```

The wrapper's value is not merely function naming: it encodes Drop behavior, borrowing relationships, safe slices/strings where possible, iterator adapters, typed errors, and feature-gated WASM/standard-library integrations.

## 3.2 Generated grammar path

A grammar repository's Rust binding normally exposes:

```rust
pub const LANGUAGE: tree_sitter_language::LanguageFn = /* generated entry point */;
```

The grammar build compiles generated `src/parser.c` and optional external scanner source. `LanguageFn` wraps the generated C function that returns a grammar pointer. ([LanguageFn][language-fn])

## 3.3 Why this architecture matters for deployment

1. **Native compiler required at build time.** Even a pure Rust application source tree compiles C generated/runtime code.
2. **Grammar ABI is separate from Rust ABI.** The runtime checks grammar ABI compatibility at `set_language`.
3. **External scanners can introduce C++ runtime requirements.** Audit individual grammars.
4. **Raw handles are escape hatches, not the default.** Stay in safe wrapper types unless interoperating with another FFI layer.
5. **Allocator overrides are global native-core configuration.** Treat them as process architecture, not request-level tuning.

## 3.4 Ownership boundaries

```text
Parser owns parser state; Drop deletes native parser.
Tree owns a reference-counted native tree; Clone is cheap at the core level.
Node borrows the tree lifetime; it does not independently own the tree.
TreeCursor borrows the tree/node domain it walks.
Query owns compiled query state and is reusable/shareable.
QueryCursor owns mutable query-execution state.
```

Never transmute/extend `Node<'tree>` lifetimes to store nodes after the owning tree is replaced.

## 3.5 FFI audit rule

Any use of:

```rust
Language::from_raw
Parser::from_raw
Tree::from_raw
QueryCursor::from_raw
/* or into_raw counterparts */
```

should be isolated in an FFI adapter module with an explicit ownership comment answering:

```text
Who allocated the object?
Who owns it now?
Who will free it?
Can the source layer still use it after transfer?
What logger/DOT/WASM state points into Rust memory?
```

## 3.6 Architecture anti-patterns

* Assuming “Rust bindings” means no native build/link layer exists.
* Shipping a grammar with an external C++ scanner without CI coverage on every target.
* Dynamically loading arbitrary grammar libraries into a long-lived process without a trust/version policy.
* Converting raw pointers in business logic rather than a narrowly audited FFI module.
* Treating grammar ABI compatibility as proof that existing queries still semantically match the new grammar.

---

# Tree-sitter Advanced — 4) Languages, grammar crates, `LanguageFn`, ABI, and metadata

## 4.0 Language construction

```rust
use tree_sitter::Language;

let json_a = Language::new(tree_sitter_json::LANGUAGE);
let json_b: Language = tree_sitter_json::LANGUAGE.into();
assert_eq!(json_a, json_b);
```

`Language` is the opaque runtime grammar object generated by the Tree-sitter CLI and exposed through the Rust binding. ([Language][language])

## 4.1 ABI compatibility check

```rust
use tree_sitter::{LANGUAGE_VERSION, MIN_COMPATIBLE_LANGUAGE_VERSION};

let abi = json_a.abi_version();
if !(MIN_COMPATIBLE_LANGUAGE_VERSION..=LANGUAGE_VERSION).contains(&abi) {
    return Err(format!(
        "grammar ABI {abi} outside runtime window {}..={}",
        MIN_COMPATIBLE_LANGUAGE_VERSION,
        LANGUAGE_VERSION,
    ).into());
}
```

`Parser::set_language` performs the actual compatibility check and returns `LanguageError` on mismatch; an explicit check is useful for startup diagnostics and inventory telemetry.

## 4.2 Semantic grammar metadata

```rust
if let Some(meta) = json_a.metadata() {
    println!(
        "grammar semantic version={}.{}.{}",
        meta.major_version,
        meta.minor_version,
        meta.patch_version,
    );
}
```

Metadata is generated from grammar configuration when provided. The Rust source explicitly notes that this version information can be used to signal likely query incompatibility on grammar upgrades. ([Language][language])

Recommended persisted fingerprint:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
struct GrammarFingerprint {
    name: Option<String>,
    abi: usize,
    semantic_version: Option<(u8, u8, u8)>,
}
```

Use it in cache/index manifests so a daemon can invalidate query-derived data when the grammar changes.

## 4.3 Language names and older grammars

```rust
println!("name={:?}", json_a.name());
```

`Language::name()` can return `None` for older parsers. Do not use the runtime language name as the only registry key; keep an application-level `LanguageId`.

## 4.4 Node-kind inventory

```rust
for id in 0..json_a.node_kind_count() {
    let id = id as u16;
    if let Some(name) = json_a.node_kind_for_id(id) {
        println!(
            "id={id} name={name} named={} visible={} supertype={}",
            json_a.node_kind_is_named(id),
            json_a.node_kind_is_visible(id),
            json_a.node_kind_is_supertype(id),
        );
    }
}
```

This is useful for diagnostics, generated adapters, grammar diff tooling, and query validation. Do not assume numeric kind IDs are stable across grammar generations unless you explicitly pin and test them.

## 4.5 Field inventory

```rust
for id in 1..=json_a.field_count() {
    let id = id as u16;
    if let Some(name) = json_a.field_name_for_id(id) {
        println!("field {id} = {name}");
        assert_eq!(json_a.field_id_for_name(name), Some(id));
    }
}
```

Prefer field names in application logic for readability. Cache field IDs only inside a grammar-version-scoped object if profiling proves the lookup matters.

## 4.6 Supertypes and subtypes

```rust
for &super_id in json_a.supertypes() {
    if let Some(name) = json_a.node_kind_for_id(super_id) {
        println!("supertype {name}");
        for &sub_id in json_a.subtypes_for_supertype(super_id) {
            println!("  - {}", json_a.node_kind_for_id(sub_id).unwrap_or("<?>"));
        }
    }
}
```

Supertypes are grammar-level abstract categories that can shorten `node-types.json` and query patterns while not necessarily appearing as visible nodes themselves. ([Static Node Types][static-node-types])

## 4.7 Grammar registry with compiled queries

```rust
use std::sync::Arc;
use tree_sitter::{Language, Query};

struct GrammarBundle {
    language: Language,
    symbols: Arc<Query>,
    imports: Arc<Query>,
}

fn json_bundle() -> Result<GrammarBundle, tree_sitter::QueryError> {
    let language: Language = tree_sitter_json::LANGUAGE.into();
    let symbols = Arc::new(Query::new(&language, "(pair key: (string) @key)")?);
    let imports = Arc::new(Query::new(&language, "(string) @string")?);
    Ok(GrammarBundle { language, symbols, imports })
}
```

Compile queries once at bundle construction, not per document.

## 4.8 Upgrade gate

At startup/CI:

```text
[ ] Load every LanguageFn into Language.
[ ] Assert ABI in runtime window.
[ ] Record semantic grammar metadata.
[ ] Compile every query against the loaded Language.
[ ] Parse representative corpus samples.
[ ] Compare node-types fingerprint when upgrading grammar.
[ ] Run incremental-edit regression tests, not only full parses.
```

---

# Tree-sitter Advanced — 5) `Parser` lifecycle and state

## 5.0 Parser state model

```text
Parser
  ├─ current Language
  ├─ included ranges
  ├─ optional logger
  ├─ optional DOT graph destination
  ├─ optional WasmStore (feature=wasm)
  └─ native parse stacks/state reused across calls
```

A parser is stateful and intended to be reused. Assign a language, parse many documents serially, reset when abandoning a parse state, or switch language when the architecture deliberately shares parser instances across grammars. ([Parser][parser])

## 5.1 Construction and language assignment

```rust
use tree_sitter::{Language, Parser};

fn make_json_parser() -> Result<Parser, tree_sitter::LanguageError> {
    let language: Language = tree_sitter_json::LANGUAGE.into();
    let mut parser = Parser::new();
    parser.set_language(&language)?;
    Ok(parser)
}
```

Prefer a constructor that cannot return an unconfigured parser to downstream code.

## 5.2 Reuse vs recreate

**Reuse a parser** for a serial stream of documents of the same language. Parser creation is not usually the dominant parse cost, but avoiding churn simplifies configuration and retains the intended stateful execution model.

For concurrent parsing, use a pool or one parser per worker rather than putting one parser behind a heavily contended global mutex.

```rust
struct ParserWorker {
    parser: Parser,
}
```

## 5.3 Switching language

```rust
let js: Language = tree_sitter_javascript::LANGUAGE.into();
parser.set_language(&js)?;
let tree = parser.parse("let x = 1;", None).unwrap();
```

When switching languages, also switch the query bundle and any cached field/kind IDs. Never carry grammar-specific query/capture assumptions across a language switch.

## 5.4 Reset

`Parser::reset()` resets parser state after an interrupted/cancelled parse so a new parse does not resume from the old partial state. Use it when a cancellation policy abandons the current parse and the next call should start clean.

```rust
let maybe_tree = parser.parse_with_options(/* ... */);
if maybe_tree.is_none() {
    parser.reset();
}
```

## 5.5 Logger

```rust
use tree_sitter::{LogType, Parser};

parser.set_logger(Some(Box::new(|kind: LogType, message: &str| {
    eprintln!("[{kind:?}] {message}");
})));
```

Parser logging is diagnostic and can be extremely verbose. Keep it off by default in production and enable it for selected files/requests.

## 5.6 Included ranges as parser state

Included ranges persist as parser configuration until changed. A parser borrowed from a pool must not accidentally retain ranges from the previous request.

Safe pool-return rule:

```text
Before returning parser to general pool:
  set_included_ranges(&[])
  disable request-specific logger
  stop DOT graph printing if enabled
  reset if last parse was cancelled
  restore expected language if pool is language-specific
```

## 5.7 WASM store as parser state

With `wasm` enabled, the parser can own a `WasmStore`. Native parsers should not pay this complexity unless runtime-loaded WASM grammars are a product requirement.

## 5.8 Parser-pool architecture

```text
ParserPool<LanguageId>
  ├─ Rust worker 0: Parser(language X)
  ├─ Rust worker 1: Parser(language X)
  └─ ...

Request
  → acquire parser
  → configure request-scoped ranges/progress
  → parse
  → scrub request-scoped state
  → return parser
```

For a repository indexer, thread-local or worker-owned parsers are often simpler than a general async pool because parsing is CPU work and parser access is naturally serialized per worker.

## 5.9 Parser anti-patterns

* Global `Mutex<Parser>` for all languages and requests.
* Returning unconfigured `Parser::new()` from a library API that expects successful parsing.
* Forgetting to clear included ranges when reusing a parser.
* Leaving debug logger/DOT output enabled in a hot service.
* Reusing a cancelled parser without considering `reset()`.
* Switching language without switching its compiled queries and schema assumptions.


# Tree-sitter Advanced — 6) Text input and encodings

## 6.0 Input API decision tree

```text
Contiguous UTF-8 bytes / &str
  → Parser::parse(...)

Non-contiguous UTF-8 source (rope/piece table/chunk store)
  → Parser::parse_with(callback, old_tree)
  → or parse_with_options(...)

Contiguous/chunked UTF-16
  → parse_utf16_le / parse_utf16_be families

Other byte encoding
  → parse_custom_encoding::<D, ...>(callback, old_tree, options)
  → D: Decode
```

The native Rust binding accepts source directly or asks the application for slices as parsing advances. This is a major integration advantage for editors: the parser does not require flattening a rope into one giant `String` before every parse. ([Rust API][rustdoc])

## 6.1 Contiguous UTF-8

```rust
let source = "let x = 1;";
let tree = parser.parse(source, None).expect("parse cancelled");
```

or raw bytes:

```rust
let source: &[u8] = b"let x = 1;";
let tree = parser.parse(source, None).unwrap();
```

Choose bytes when downstream code is fundamentally byte-offset based. Tree-sitter byte coordinates should normally be treated as authoritative internal coordinates because all node/edit ranges include bytes even when point coordinates are also available.

## 6.2 Callback input

The crate's basic documentation demonstrates callback parsing where Tree-sitter provides both a byte offset and `Point`, and the callback returns bytes beginning at that location. ([Rust API][rustdoc])

```rust
use tree_sitter::Point;

let lines = [
    "{",
    "  \"x\": 1",
    "}",
];

let tree = parser.parse_with(
    &mut |_byte: usize, position: Point| -> &[u8] {
        let row = position.row;
        let column = position.column;
        if let Some(line) = lines.get(row) {
            let bytes = line.as_bytes();
            if column < bytes.len() {
                &bytes[column..]
            } else {
                b"\n"
            }
        } else {
            &[]
        }
    },
    None,
).unwrap();
```

The callback contract is **not** “return the entire remaining document.” It may return a slice of any useful length starting at the requested location. Empty means EOF.

## 6.3 Rope / piece-table adapter pattern

A production text buffer usually exposes a cheap “chunk at byte” operation:

```rust
trait SourceBuffer {
    fn chunk_at_byte(&self, byte: usize) -> &[u8];
}

fn parse_buffer(
    parser: &mut tree_sitter::Parser,
    source: &impl SourceBuffer,
    old_tree: Option<&tree_sitter::Tree>,
) -> Option<tree_sitter::Tree> {
    parser.parse_with(
        &mut |byte, _point| source.chunk_at_byte(byte),
        old_tree,
    )
}
```

Real rope APIs may return UTF-8 chunks indexed in chars rather than bytes. Build and test the conversion layer once; do not scatter byte/char translations across query/traversal logic.

## 6.4 UTF-16

The Rust binding exposes UTF-16 little-endian and big-endian parsing. This is useful when the authoritative editor/document representation is already UTF-16 and flattening/transcoding would dominate latency.

Agent rule:

```text
Do not confuse Tree-sitter Point.column with LSP UTF-16 character units.
Tree-sitter positions are parser/input coordinates; LSP positions must be converted according to the negotiated LSP position encoding.
```

If the input is UTF-8 but an API consumer is UTF-16-positioned, retain a line index capable of converting offsets rather than parsing a second representation solely for coordinates.

## 6.5 Custom encodings and `Decode`

The 0.26 Rust API supports `Parser::parse_custom_encoding` with a generic `D: Decode`. `Decode::decode(bytes)` returns `(code_point, bytes_consumed)`; decoding failure is signaled with a code point of `-1`. ([Decode source][decode-source])

```rust
use tree_sitter::Decode;

struct Latin1;

impl Decode for Latin1 {
    fn decode(bytes: &[u8]) -> (i32, u32) {
        match bytes.first() {
            Some(&b) => (i32::from(b), 1),
            None => (-1, 0),
        }
    }
}
```

Conceptual invocation:

```rust
let tree = parser.parse_custom_encoding::<Latin1, _, _>(
    &mut |byte, _point| &encoded_source[byte..],
    old_tree,
    None,
);
```

Use custom decoders only when the source's native encoding truly needs to be preserved. UTF-8 normalization upstream is simpler when it does not break product coordinate contracts.

## 6.6 Lossy UTF-8

The current crate includes `LossyUtf8`, a built-in decode-related type for tolerant input handling. Treat tolerant decoding as a data-quality policy: if exact source fidelity matters, preserve raw bytes and surface invalid encoding rather than silently normalizing without provenance.

## 6.7 Source retention contract

Recommended document object:

```rust
struct ParsedDocument<B> {
    buffer: B,
    tree: tree_sitter::Tree,
    revision: u64,
}
```

Invariants:

```text
buffer revision == tree revision
node byte ranges index exactly that buffer revision
query TextProvider reads exactly that buffer revision
InputEdit is applied to both buffer and old tree in the same logical transaction
```

## 6.8 Text extraction

```rust
fn node_text<'a>(
    node: tree_sitter::Node<'_>,
    source: &'a [u8],
) -> Result<&'a str, std::str::Utf8Error> {
    node.utf8_text(source)
}
```

For non-contiguous sources, avoid materializing text for every node. Query predicate evaluation may need a text provider, but downstream structural extraction should carry byte ranges and materialize text only where required.

## 6.9 Performance rules

* Parse directly from your authoritative buffer when feasible.
* Never rebuild a full `String` on every keystroke solely to satisfy Tree-sitter.
* Maintain byte/line indexes alongside rope edits.
* Store byte ranges in syntax-derived records; convert to UI coordinate systems at boundaries.
* Avoid UTF-8 validation/text allocation when a consumer only needs kind/range/field.
* Benchmark callback chunk sizes for your rope implementation; pathological tiny fragments can increase callback overhead.

## 6.10 Input anti-patterns

* Returning bytes that do not begin at the callback's requested byte/point.
* Mutating the source while a parse/query is reading it.
* Using a stale source revision with a new tree.
* Treating Unicode scalar/UTF-8 byte/UTF-16 code-unit columns as interchangeable.
* Re-encoding whole files on every incremental change when a native chunked representation is available.

---

# Tree-sitter Advanced — 7) Syntax tree model: `Tree`, `Node`, and CST semantics

## 7.0 Tree object

`Tree` represents the syntactic structure of an entire parsed input domain. It exposes root access, language, edit, changed ranges, included ranges, walking, raw ownership conversion, and optional DOT graph output. ([Tree][tree])

```rust
let root = tree.root_node();
let language = tree.language();
let ranges = tree.included_ranges();
```

## 7.1 Node is a lightweight tree view

```rust
pub struct Node<'tree>(/* native node + borrow marker */);
```

The lifetime associates the node with its tree. Node is cheap to copy, but the underlying tree must remain alive. ([Node][node])

```rust
fn root_kind(tree: &tree_sitter::Tree) -> &str {
    tree.root_node().kind()
}
```

Do not attempt to return a `Node` whose tree is a local temporary that will be dropped.

## 7.2 Named, anonymous, and extra nodes

```rust
for i in 0..root.child_count() {
    let node = root.child(i).unwrap();
    println!(
        "{} named={} extra={}",
        node.kind(),
        node.is_named(),
        node.is_extra(),
    );
}
```

Definitions:

```text
named node     grammar rule with a named symbol; typically a structural construct
anonymous node literal token from grammar; punctuation/keywords often live here
extra node     grammar extra such as comment/whitespace-like construct allowed broadly
```

These categories are orthogonal to whether a node has children.

## 7.3 Error and missing nodes

```rust
if root.has_error() {
    // Walk only relevant subtrees and inspect:
    // node.is_error()
    // node.is_missing()
}
```

`is_error()` identifies syntax error nodes representing source that could not be incorporated normally. `is_missing()` identifies zero-width nodes inserted by the parser during error recovery. `has_error()` is recursive: the node itself or some descendant contains an error. ([Node][node])

## 7.4 Positions

Every node exposes:

```rust
node.start_byte();
node.end_byte();
node.byte_range();
node.start_position();
node.end_position();
node.range();
```

`Point` rows and columns are zero-based. A `Range` contains both byte and point endpoints. ([Point/Range source][binding-source])

Recommended domain representation:

```rust
#[derive(Clone, Copy, Debug)]
struct SourceSpan {
    start_byte: usize,
    end_byte: usize,
}
```

Store points only when a consumer needs them or when converting edits; bytes are compact and sufficient to slice source and compare intervals.

## 7.5 Root with offset

`Tree::root_node_with_offset` lets the root be viewed as shifted by an offset. This can be useful when a separately parsed fragment must be reported in host-document coordinates, but it does not rewrite the underlying source. Use one consistent offset policy; double-applying host offsets is a common embedded-language bug.

## 7.6 Language of a node/tree

The current API exposes language on both tree/node surfaces. This becomes important in systems that may process trees from different grammar bundles. Avoid pattern logic that assumes every incoming node belongs to a single global grammar.

## 7.7 CST normalization layer

Many applications should not expose raw Tree-sitter nodes directly. Create domain records:

```rust
#[derive(Debug, Clone)]
struct SyntaxEntity {
    kind: String,
    start_byte: usize,
    end_byte: usize,
    field: Option<String>,
}
```

For code intelligence, richer records may include:

```text
language_id
grammar_semver
capture_name
syntactic_role
container span
source revision
```

This makes caches serializable and prevents Tree-sitter node lifetimes from leaking into unrelated layers.

## 7.8 Tree cloning and snapshots

Tree-sitter's native tree copy is cheap because internal tree structures are reference-counted. The official concurrency guide warns that individual native `TSTree` instances should be copied before independent concurrent use. In Rust, clone a `Tree` when handing independent snapshots to concurrent work that may edit/reparse. ([Advanced Parsing][advanced-parsing])

```rust
let snapshot = tree.clone();
```

Do not infer that `Clone` duplicates the entire syntax tree; it is designed to be inexpensive.

## 7.9 Tree anti-patterns

* Serializing `Node` pointers/IDs as durable index data.
* Storing only `Point` and losing byte offsets needed for slicing/edits.
* Filtering anonymous nodes everywhere and later discovering punctuation/operator identity was needed.
* Treating `extra` as synonymous with “ignore forever”; comments can power docs, directives, and injections.
* Mutating one tree instance while another thread assumes it is the same logical snapshot; clone first.

---

# Tree-sitter Advanced — 8) Node inspection API

## 8.0 Node capability map

The current Rust `Node` API includes: kind and grammar IDs/names, language, named/extra/error/missing/changed flags, parse states, byte/point ranges, indexed and named child access, field-based access, parent/sibling navigation, descendant lookup by byte/point, text extraction, cursor construction, S-expression display, and coordinate editing. ([Node][node])

## 8.1 Kind vs grammar name/id

```rust
println!("kind={}", node.kind());
println!("kind_id={}", node.kind_id());
println!("grammar_name={}", node.grammar_name());
println!("grammar_id={}", node.grammar_id());
```

`kind` is normally the application-facing node type. Grammar-oriented APIs matter when aliases/symbol identity or parse-table integration matters. Do not persist numeric IDs across grammar versions.

## 8.2 Child access complexity

The Rust docs explicitly note that `Node::child(i)` and `named_child(i)` are technically `log(i)` and recommend iterator-based `children` / `named_children` for walking long lists. ([Node][node])

Bad hot loop:

```rust
for i in 0..node.child_count() {
    consume(node.child(i as u32).unwrap());
}
```

Prefer:

```rust
let mut cursor = node.walk();
for child in node.children(&mut cursor) {
    consume(child);
}
```

or named-only:

```rust
let mut cursor = node.walk();
for child in node.named_children(&mut cursor) {
    consume(child);
}
```

## 8.3 Fields

```rust
let key = pair.child_by_field_name("key");
let value = pair.child_by_field_name("value");
```

Fields encode semantic roles assigned by the grammar and are often much more robust than hard-coded child indices.

If multiple children can share one field:

```rust
let mut cursor = node.walk();
for child in node.children_by_field_name("argument", &mut cursor) {
    // ...
}
```

Field IDs can be pre-resolved from `Language` for hot code, but scope the cached ID to the grammar fingerprint.

## 8.4 Parent and siblings

```rust
let parent = node.parent();
let next = node.next_sibling();
let prev = node.prev_sibling();
let next_named = node.next_named_sibling();
let prev_named = node.prev_named_sibling();
```

Use named siblings when structural adjacency matters but punctuation/comments are irrelevant; use raw siblings when token adjacency is meaningful.

## 8.5 Descendant lookup by byte range

```rust
let smallest = root.descendant_for_byte_range(start, end);
let smallest_named = root.named_descendant_for_byte_range(start, end);
```

This is ideal for editor cursor/selection mapping: map a byte span to the smallest enclosing syntax node without scanning the entire tree.

Point-based equivalents exist when the caller already has Tree-sitter `Point`s.

## 8.6 First child near an offset

Current node/cursor APIs include first-child-for-byte/point helpers. Use them to skip directly into large child lists around an edit location instead of linearly stepping from child zero.

## 8.7 Text extraction

```rust
let text = node.utf8_text(source_bytes)?;
```

Do not call this indiscriminately for millions of nodes if you only need kinds/spans. Text decoding/slicing should be demand-driven.

## 8.8 S-expression representation

```rust
println!("{}", node.to_sexp());
```

S-expressions are excellent golden-test/debug representations, but not a stable serialized domain model across grammar upgrades. Grammar node names/structure can change.

## 8.9 `has_changes`

After editing a tree, nodes can be marked as changed. This is useful for introspection, but changed-node flags are not the same abstraction as `Tree::changed_ranges` between old and new completed trees. For incremental downstream work, changed ranges are usually easier to turn into invalidation intervals.

## 8.10 Node identity

`Node::id()` provides an identity meaningful within a tree and can sometimes be preserved when subtrees are structurally reused in an incremental parse. The docs caution that unchanged does not guarantee reuse; changed nodes are not reused. Therefore:

```text
node.id == optimization hint / snapshot identity
node.id != durable source symbol ID
```

For durable index IDs, derive identity from semantic/syntactic keys plus stable file/revision context.

## 8.11 Parse-state APIs

```rust
let before = node.parse_state();
let after = node.next_parse_state();
```

These states can be fed into language lookahead to derive syntactically valid symbols near a node. This powers sophisticated completion/recovery tooling covered later.

## 8.12 Node inspection anti-patterns

* Child-index-based extraction when grammar fields exist.
* Persisting `kind_id`, `grammar_id`, or `id()` across grammar upgrades.
* Extracting UTF-8 text for every visited node by default.
* Assuming an unchanged node ID is guaranteed across every reparse.
* Using only named nodes for transformations that need punctuation/operator tokens.

---

# Tree-sitter Advanced — 9) Efficient tree traversal

## 9.0 Why `TreeCursor`

`TreeCursor` is explicitly the stateful efficient tree-walking object. It tracks the current node and supports parent/child/sibling movement plus field information, depth and descendant index. ([TreeCursor][tree-cursor])

## 9.1 Iterative depth-first traversal

```rust
fn walk_all(root: tree_sitter::Node<'_>, mut f: impl FnMut(tree_sitter::Node<'_>, u32)) {
    let mut cursor = root.walk();
    let mut entered = true;

    loop {
        if entered {
            f(cursor.node(), cursor.depth());
        }

        if entered && cursor.goto_first_child() {
            entered = true;
            continue;
        }

        if cursor.goto_next_sibling() {
            entered = true;
            continue;
        }

        loop {
            if !cursor.goto_parent() {
                return;
            }
            if cursor.goto_next_sibling() {
                entered = true;
                break;
            }
        }
    }
}
```

For very deep/untrusted source, iterative traversal avoids growing the Rust call stack.

## 9.2 Named-only traversal

There is no single “goto first named child” cursor primitive mirroring every `Node` convenience API, so two practical approaches are:

1. use `node.named_children(&mut cursor)` when traversing a local list;
2. walk all cursor nodes and predicate `is_named()` when you need one persistent cursor.

Benchmark if the tree is extremely large; clarity normally matters more than micro-optimizing this distinction.

## 9.3 Field-aware cursor

```rust
let field = cursor.field_name();
let field_id = cursor.field_id();
```

When extracting generic structural records, capturing the cursor's current field avoids re-deriving role from parent child indices.

## 9.4 Pruned traversal

```rust
fn should_descend(node: tree_sitter::Node<'_>, changed: std::ops::Range<usize>) -> bool {
    node.end_byte() > changed.start && node.start_byte() < changed.end
}
```

In an incremental indexer, skip subtrees whose byte ranges do not intersect invalidation ranges. Be careful when the extraction depends on ancestor context or cross-range relationships; expand invalidation to semantic boundaries when needed.

## 9.5 Direct descendant lookup instead of traversal

For editor position requests:

```rust
let node = root.named_descendant_for_byte_range(cursor_byte, cursor_byte)?;
```

This is generally preferable to scanning from root for each hover/completion request.

## 9.6 Traversal with a stack of containers

Code intelligence often needs container context:

```rust
#[derive(Clone, Debug)]
struct Container {
    kind: String,
    span: std::ops::Range<usize>,
}
```

Push when entering function/class/module-like nodes, pop when leaving. A cursor walker that emits enter/leave events is a useful primitive:

```text
Enter(node, field, depth)
Leave(node, depth)
```

This avoids repeated parent-chain scans for every leaf capture.

## 9.7 Query vs traversal decision

| Need | Prefer |
|---|---|
| All nodes in exact structural order | `TreeCursor` |
| A few grammar patterns | `Query` |
| Position → enclosing node | descendant lookup |
| Named child fields from known parent | `child_by_field_name` |
| Broad symbol extraction across language | query + capture mapping |
| Complex stateful context across nesting | cursor traversal or hybrid |

Do not force every extraction problem into the query language. Queries excel at declarative local structural patterns; explicit traversal can be clearer for stateful algorithms.

## 9.8 Traversal performance checklist

```text
[ ] Avoid child(i) loops on large child lists.
[ ] Reuse one TreeCursor for a traversal.
[ ] Use named_children when anonymous tokens are irrelevant.
[ ] Use descendant_for_* for point/range requests.
[ ] Prune by changed ranges for incremental workloads.
[ ] Avoid text materialization until a matched node actually needs text.
[ ] Prefer iterative traversal for adversarial/deep trees.
```

---

# Tree-sitter Advanced — 10) Incremental parsing

## 10.0 The critical invariant

The old tree passed into reparse must have coordinates that correspond to the **new source**. Therefore:

```text
1. derive edit from old source → new source
2. mutate/source-buffer transaction
3. Tree::edit(old_tree, edit)
4. Parser::parse(new_source, Some(&old_tree))
5. install returned new tree
```

The official advanced parsing guide identifies exactly these two Tree-sitter-specific steps: edit the old tree and then parse using it. ([Advanced Parsing][advanced-parsing])

## 10.1 `InputEdit`

```rust
pub struct InputEdit {
    pub start_byte: usize,
    pub old_end_byte: usize,
    pub new_end_byte: usize,
    pub start_position: Point,
    pub old_end_position: Point,
    pub new_end_position: Point,
}
```

This describes one replacement:

```text
old interval: [start_byte, old_end_byte)
new interval: [start_byte, new_end_byte)
```

with point equivalents for line/column geometry. ([InputEdit][input-edit])

## 10.2 Insert, delete, replace

Insert:

```text
old_end_byte == start_byte
new_end_byte > start_byte
```

Delete:

```text
old_end_byte > start_byte
new_end_byte == start_byte
```

Replace:

```text
old_end_byte > start_byte
new_end_byte > start_byte
```

The endpoints can differ by any length; “replacement” is not required to preserve line count.

## 10.3 Edit transaction object

```rust
struct TextEdit {
    input_edit: tree_sitter::InputEdit,
    replacement: Vec<u8>,
}

impl ParsedDocument {
    fn apply_edit(&mut self, edit: TextEdit) {
        let e = edit.input_edit;
        self.tree.edit(&e);
        self.source.splice(e.start_byte..e.old_end_byte, edit.replacement);
        self.revision += 1;
    }
}
```

In production, order can be source-then-tree or tree-then-source as long as no observer sees a mismatched pair; update both under one document mutation boundary.

## 10.4 Multiple edits

If the editor emits multiple edits, coordinate convention matters. Apply edits in the sequence and coordinate space defined by your source protocol. A safe approach is:

```text
for each edit in protocol order:
  compute InputEdit against current buffer state
  tree.edit(edit)
  mutate buffer
then reparse once with final edited tree
```

Do not batch arbitrary old-coordinate edits into one list and apply them naively after prior edits have shifted positions.

## 10.5 Reparse

```rust
let new_tree = parser
    .parse(&doc.source, Some(&doc.tree))
    .ok_or(ParseCancelled)?;

doc.tree = new_tree;
```

If no trustworthy old tree/edit mapping exists—e.g. source replaced wholesale from an external snapshot—perform a full parse with `None` rather than feeding a stale tree and hoping for correctness.

## 10.6 Editing retained points/ranges

`InputEdit` now exposes `edit_point` and `edit_range` helpers. They update standalone coordinates using the same edit geometry. ([InputEdit][input-edit])

```rust
edit.edit_point(&mut point, &mut byte);
edit.edit_range(&mut cached_range);
```

This is useful for auxiliary editor markers, but do not blindly “edit” a semantic result and assume it remains valid. Coordinates can be shifted while syntactic meaning changes.

## 10.7 Cached nodes after edit

The advanced guide warns that if your application stores `Node` instances, you need to edit those nodes as well or obtain fresh nodes from the edited/new tree. In Rust, the cleaner production design is usually to **not persist `Node` handles across document revisions**. Persist spans/domain records and re-resolve as needed.

## 10.8 Incremental parse validation

A valuable correctness test:

```text
incremental_tree = edit old tree + parse(new, old)
full_tree        = parse(new, None)
assert structural equivalence / same S-expression
```

Run this over randomized realistic edit sequences. Incremental bugs in editor integrations are often edit-coordinate bugs, not parser bugs.

## 10.9 Editor integration checklist

```text
[ ] Maintain byte + line/column coordinates for every edit.
[ ] Use one source revision for source/tree/query reads.
[ ] Apply edits in protocol-defined coordinate order.
[ ] Edit old tree before passing it to reparse.
[ ] Fall back to full parse when edit lineage is uncertain.
[ ] Do not retain Node handles across replacement of the Tree.
[ ] Compare incremental vs full parse in integration/property tests.
```

## 10.10 Incremental anti-patterns

* Reparse with `Some(old_tree)` without first editing it after source changes.
* Compute `Point` columns from Unicode scalar count when the source adapter expects byte columns.
* Apply a list of stale coordinates after each preceding edit shifts the text.
* Assume incremental reuse implies semantic identity of all unchanged-looking nodes.
* Optimize downstream extraction incrementally before proving the parse/edit layer itself is correct.

---

# Tree-sitter Advanced — 11) Changed ranges, identity, and cache invalidation

## 11.0 `Tree::changed_ranges`

After reparsing, compare the **edited old tree** to the new tree:

```rust
let old_edited = old_tree; // already Tree::edit-ed
let new_tree = parser.parse(new_source, Some(&old_edited)).unwrap();

let ranges: Vec<_> = old_edited.changed_ranges(&new_tree).collect();
```

The Rust docs define these as ranges whose syntactic structure has changed and require the old tree's coordinates to have been edited to match the new document. ([Tree][tree])

## 11.1 Changed ranges are structural, not merely textual

A text edit can have syntactic effects beyond the directly replaced bytes, for example:

```text
insert quote        → string may extend across later tokens
remove delimiter    → parent parse structure may change substantially
change keyword      → statement/expression node kind may change
indent-sensitive external scanner → structural change can extend across lines
```

Use `changed_ranges` as the parser's syntactic invalidation signal rather than using only the raw text edit interval.

## 11.2 Ranges may be conservative

The native API notes that changed ranges may be somewhat larger than the mathematically exact changed area; Tree-sitter attempts to make them small. Build invalidation logic that tolerates conservative ranges. ([Tree FFI docs][tree-ffi])

## 11.3 Node IDs and reuse

`Node::id()` can remain the same for structurally reused nodes, but current docs explicitly caution that a node not marked changed is not guaranteed to be reused. A node marked changed will not be reused. ([Node][node])

Therefore:

```text
same id across old/new → evidence of reuse
no same id             → not evidence of semantic change by itself
```

Never use node IDs as cross-process or durable database keys.

## 11.4 Invalidation strategy for extracted records

Suppose a file index stores records with source spans:

```rust
struct IndexedRecord {
    start_byte: usize,
    end_byte: usize,
    kind: RecordKind,
}
```

A simple invalidator:

```rust
fn intersects(a: std::ops::Range<usize>, b: std::ops::Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}
```

Delete/recompute records intersecting changed syntax ranges, then run relevant queries with cursor byte-range restrictions around those ranges.

## 11.5 Expand to semantic boundaries

For code intelligence, raw syntax changed ranges may be too narrow for derived facts:

```text
function signature changed → invalidate whole function symbol + callers/import exports maybe
scope declaration changed  → invalidate containing scope name-resolution cache
import changed             → invalidate file-level dependency edges
class base list changed    → invalidate class semantic envelope
```

Pattern:

```text
changed Range
  → find smallest named descendant/container
  → ascend to invalidation boundary kind
  → rerun extractors on boundary
```

## 11.6 Query-range rerun

```rust
for r in changed_ranges {
    cursor.set_byte_range(r.start_byte..r.end_byte);
    // execute query on root or suitable container
}
```

Be aware that ordinary query range restriction returns matches that **intersect** the range; containing-range variants require captures to be fully contained. Choose according to invalidation semantics.

## 11.7 Stable domain identities

Better symbol key candidates:

```text
file identity
language
symbol kind
normalized name
container identity
signature discriminator
source-relative structural path (optional)
```

Then use current spans as mutable metadata. This survives shifts from edits better than using byte offset or Tree-sitter node ID alone.

## 11.8 Invalidation anti-patterns

* Rerunning every query over a million-line file after a one-character edit.
* Rerunning only the literal text edit interval and missing larger parse changes.
* Treating `Node::id()` as a persistent symbol key.
* Shifting cached semantic results with `InputEdit::edit_range` without revalidating meaning.
* Ignoring ancestor/container dependencies in extracted relationships.


# Tree-sitter Advanced — 12) Query mental model and compilation

## 12.0 Query = compiled structural matcher

A `Query` is a set of S-expression patterns compiled against one `Language`. It is not SQL, XPath, regex-over-source, or an AST visitor generator. A query matches **tree structure**, optionally constrained by fields, sibling relationships, text predicates, and metadata predicates. ([Query][query])

```rust
use tree_sitter::{Language, Query};

let language: Language = tree_sitter_json::LANGUAGE.into();
let query = Query::new(
    &language,
    r#"
      (pair
        key: (string (string_content) @key)
        value: (_) @value)
    "#,
)?;
```

## 12.1 Compile once, execute many

`Query` is immutable after construction except for explicit pattern/capture disabling operations and implements `Send`/`Sync` in the Rust binding. Compile static query sets once per grammar bundle and share them. ([Query][query])

```rust
use std::sync::Arc;

struct Queries {
    symbols: Arc<Query>,
    imports: Arc<Query>,
    calls: Arc<Query>,
}
```

Do not invoke `Query::new` inside a per-node loop or per-file hot path unless the query source is truly dynamic.

## 12.2 Query errors are build/configuration errors for static queries

```rust
let query = Query::new(&language, query_source)
    .map_err(|e| format!("invalid symbol query: {e}"))?;
```

For embedded static query files, treat compile failure as startup/CI failure. A shipped application should not discover a misspelled node kind after processing 10,000 files.

Recommended CI:

```rust
#[test]
fn all_queries_compile() {
    let language: Language = tree_sitter_json::LANGUAGE.into();
    for (name, source) in ALL_QUERY_SOURCES {
        Query::new(&language, source)
            .unwrap_or_else(|e| panic!("query {name} failed: {e}"));
    }
}
```

## 12.3 Query metadata inventory

Current `Query` exposes:

```text
pattern_count()
capture_names()
capture_quantifiers(pattern_index)
capture_index_for_name(name)
start_byte_for_pattern(pattern_index)
end_byte_for_pattern(pattern_index)
is_pattern_rooted(pattern_index)
is_pattern_non_local(pattern_index)
is_pattern_guaranteed_at_step(byte_offset)
property_predicates(pattern_index)
property_settings(pattern_index)
general_predicates(pattern_index)
disable_capture(name)
disable_pattern(index)
```

This surface makes query compilation inspectable enough to build query validators, debugging UIs, and performance policy checks. ([Query][query])

## 12.4 Capture-name mapping

```rust
for (i, name) in query.capture_names().iter().enumerate() {
    println!("capture {i} = @{name}");
}

let value_capture = query.capture_index_for_name("value")
    .expect("query must define @value");
```

Map capture indices to enum/domain roles once:

```rust
#[derive(Clone, Copy, Debug)]
enum CaptureRole {
    Name,
    Definition,
    Reference,
    Container,
}
```

Then query execution can avoid repeated string comparisons for every capture.

## 12.5 Pattern-level routing

A multi-pattern query can encode different record types:

```scheme
(function_declaration
  name: (identifier) @name) @definition.function

(class_declaration
  name: (identifier) @name) @definition.class
```

`QueryMatch.pattern_index` identifies the matched pattern. Precompute a `Vec<PatternRole>` indexed by that integer.

## 12.6 Disable captures/patterns

```rust
let mut query = Query::new(&language, source)?;
query.disable_capture("debug-only");
query.disable_pattern(3);
```

Use this for product profiles when one compiled query source is shared but a subset of captures/patterns is unnecessary. Prefer separate clear query files when profiles are substantially different; mutation of shared query configuration should happen before `Arc<Query>` publication.

## 12.7 Rooted and non-local patterns

Query introspection distinguishes rooted/non-local patterns. Non-local patterns can match within a repeating sequence in ways that make range-local execution less straightforward. If building a highly incremental indexer, audit query locality rather than assuming every query can be safely restricted to only the literal changed bytes.

## 12.8 Grammar upgrade contract

On grammar update:

```text
load new Language
  → check ABI
  → inspect semantic metadata
  → compile all queries
  → run query golden tests
  → run corpus extraction diffs
```

Successful query compilation proves names/fields are legal, **not** that capture semantics are unchanged. Golden fixtures are mandatory for critical extraction.

## 12.9 Query anti-patterns

* Compile a query per file.
* Treat query source as language-independent.
* Use capture strings as unchecked magic values throughout business logic.
* Rely on numeric pattern/capture indexes without binding them to the compiled query instance.
* Assume successful compilation means identical extraction after grammar upgrade.
* Make a giant universal query with hundreds of unrelated patterns when independent queries have different invalidation/performance needs.

---

# Tree-sitter Advanced — 13) Query syntax

## 13.0 Basic node pattern

A query consists of one or more S-expression patterns. ([Basic Query Syntax][query-syntax])

```scheme
(identifier)
```

Nested pattern:

```scheme
(call_expression
  function: (identifier))
```

## 13.1 Fields

Fields constrain child roles:

```scheme
(pair
  key: (string)
  value: (number))
```

Field-based queries are generally more robust than positional child assumptions because grammar authors use fields precisely to label roles such as `name`, `body`, `left`, `right`, `arguments`, and `value`.

## 13.2 Negated fields

Query syntax can require the absence of a field:

```scheme
(class_declaration
  name: (identifier)
  !type_parameters)
```

Use negative-field constraints when absence itself is semantically relevant. Avoid over-constraining queries with negative fields that merely happen to be absent in today's grammar fixtures.

## 13.3 Anonymous nodes

Literal tokens are matched with quotes:

```scheme
(binary_expression
  operator: "+")
```

Anonymous tokens are useful when operator/keyword identity matters but the grammar does not represent each token as a named rule.

## 13.4 Wildcards

Tree-sitter distinguishes wildcard forms:

```scheme
(_)     ; any named node
_       ; any node, including anonymous nodes
```

Use the narrowest wildcard that preserves intended semantics. `(_)` is usually safer for structural captures where punctuation should not match.

## 13.5 Capturing parent plus child

```scheme
(pair
  key: (string) @key
  value: (_) @value) @pair
```

Capture the parent when downstream invalidation/context needs the whole structural unit, even if text extraction only uses the child.

## 13.6 `ERROR`

Tree-sitter exposes error nodes to queries:

```scheme
(ERROR) @syntax.error
```

This makes generic error-region extraction grammar-independent at the query level.

## 13.7 `MISSING`

Missing nodes can be queried by expected symbol, e.g. conceptually:

```scheme
(MISSING ";") @missing.semicolon
```

or a named missing symbol depending on grammar/query syntax. Missing nodes have zero width and represent parser-inserted recovery artifacts, so source slicing their range often returns empty text; the **expected kind** is the useful diagnostic.

## 13.8 Supertypes

If a grammar declares a supertype, query patterns can match it and optionally constrain a subtype using supertype/subtype syntax:

```scheme
(expression) @expr
(expression/binary_expression) @binary
```

This is valuable for grammar evolution because one supertype query can cover many concrete expression variants. ([Static Node Types][static-node-types])

## 13.9 Anonymous-token exactness

Do not parse source text with regex merely to determine an operator already represented as an anonymous child. Query the token or inspect child kind; it preserves grammar interpretation and handles comments/extras correctly.

## 13.10 Query syntax test harness

```rust
fn assert_query_matches(
    language: &tree_sitter::Language,
    source: &[u8],
    query_source: &str,
) {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(language).unwrap();
    let tree = parser.parse(source, None).unwrap();
    let query = tree_sitter::Query::new(language, query_source).unwrap();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);
    assert!(tree_sitter::StreamingIterator::next(&mut matches).is_some());
}
```

## 13.11 Syntax anti-patterns

* Querying only by node name when a field constraint can disambiguate roles.
* Using `_` when `(_)` is intended and accidentally matching punctuation.
* Encoding operator meaning with source regex instead of anonymous token patterns.
* Matching a long exact ancestor chain that makes the query brittle to harmless grammar changes.
* Ignoring supertypes when they provide a stable conceptual abstraction.

---

# Tree-sitter Advanced — 14) Query operators

## 14.0 Capture operator

Captures associate a name with a matched node:

```scheme
(identifier) @name
```

Captures can apply to nested nodes, alternatives, repeated nodes, or entire patterns. ([Query Operators][query-operators])

## 14.1 Quantifiers

Postfix query quantifiers:

```scheme
(comment)+   ; one or more
(comment)*   ; zero or more
(string)?    ; optional
```

Quantifiers are **query pattern repetition**, not grammar rule declarations.

Example:

```scheme
(class_declaration
  (decorator)* @decorator
  name: (identifier) @name)
```

A quantified capture may yield multiple captures within one match. Do not model every capture name as a scalar.

## 14.2 Capture quantifiers in Rust

`Query::capture_quantifiers(pattern_index)` exposes one of:

```text
Zero
ZeroOrOne
ZeroOrMore
One
OneOrMore
```

for capture slots. ([CaptureQuantifier][capture-quantifier])

Use this metadata in generated extractors:

```text
One / ZeroOrOne      → Option/single-value domain field is plausible
OneOrMore/ZeroOrMore → Vec domain field is plausible
```

Still validate the actual query semantics before auto-generating Rust structs.

## 14.3 Groups

Parentheses can group sibling patterns:

```scheme
(
  (comment)
  (function_declaration)
)
```

Quantifiers can apply to groups:

```scheme
(
  (number)
  ("," (number))*
)
```

Grouping is useful when adjacency/sequence is the semantic unit rather than one parent rule.

## 14.4 Alternations

Square brackets define alternatives:

```scheme
(call_expression
  function: [
    (identifier) @function
    (member_expression
      property: (property_identifier) @method)
  ])
```

A capture can apply to an entire alternation:

```scheme
[
  "if"
  "for"
  "while"
  "return"
] @keyword
```

## 14.5 Quantified alternation semantics

These are not equivalent:

```scheme
[
  (preproc_include)
  (comment)
]+ @capture
```

vs:

```scheme
[
  (preproc_include)+
  (comment)
] @capture
```

The official operators guide illustrates that quantifying the alternation can create one match with many captures, while quantifying one alternative can create separate matches. ([Query Operators][query-operators])

Agent rule: golden-test quantified query shapes. Small punctuation placement can change **match cardinality**, not merely capture cardinality.

## 14.6 Anchors: `.`

Anchor before first child:

```scheme
(array . (identifier) @first-identifier)
```

Only first named child can satisfy it.

Anchor after last child:

```scheme
(block (_) @last-expression .)
```

Only last named child can satisfy it.

Anchor between children:

```scheme
(dotted_name
  (identifier) @left
  .
  (identifier) @right)
```

Requires immediate **named** sibling adjacency; anonymous nodes are ignored by anchor restrictions. ([Query Operators][query-operators])

## 14.7 Anchors with optional/repeated nodes

With a zero-length quantifier match, an anchor on that side cannot enforce adjacency to a nonexistent node. This matters in patterns such as doc-comments before declarations:

```scheme
(translation_unit
  (comment)* @doc
  .
  (function_definition) @function)
```

If no comment exists, the function can still match; if comments exist they must be immediately adjacent under the named-node definition. Test empty and multiple cases.

## 14.8 Operator design rules

```text
Use captures to identify output nodes, not every structural constraint node.
Use +/*/? intentionally; inspect capture quantifiers.
Use alternation to consolidate truly equivalent roles.
Use anchors for sibling position/adjacency rather than post-filtering on byte gaps.
Remember anchors ignore anonymous nodes.
Golden-test cardinality whenever a quantifier is near a capture or alternation.
```

---

# Tree-sitter Advanced — 15) Query matches and captures in Rust

## 15.0 Two execution modes

```text
QueryCursor::matches(...)  → stream QueryMatch values
QueryCursor::captures(...) → stream captures in source/query order semantics
```

Use matches when pattern grouping matters. Use captures when the pipeline is capture-centric, such as highlighting or simple token classification.

## 15.1 `QueryMatch`

Current Rust shape:

```text
QueryMatch {
  pattern_index: usize,
  captures: &[QueryCapture],
}
```

`QueryCapture` contains:

```text
node: Node
index: u32   // index into query.capture_names()
```

([QueryMatch][query-match]) ([QueryCapture][query-capture])

## 15.2 Iterating matches

```rust
use tree_sitter::StreamingIterator;

let mut cursor = tree_sitter::QueryCursor::new();
let mut matches = cursor.matches(&query, root, source);

while let Some(m) = matches.next() {
    let role = pattern_roles[m.pattern_index];
    for capture in m.captures {
        let capture_name = query.capture_names()[capture.index as usize];
        consume(role, capture_name, capture.node);
    }
}
```

The `StreamingIterator` contract means `m` cannot be stashed indefinitely while advancing the iterator because it borrows cursor-managed storage. Materialize only the data you need:

```rust
#[derive(Debug, Clone)]
struct OwnedCapture {
    pattern: usize,
    name: String,
    range: std::ops::Range<usize>,
}
```

## 15.3 Text provider

`QueryCursor` does not require one contiguous `&[u8]`; it accepts a `TextProvider`. The Rust trait is implemented for `&[u8]` and also for closures returning iterators of byte-like chunks. ([TextProvider][text-provider])

Contiguous:

```rust
cursor.matches(&query, root, source.as_slice())
```

Chunked provider concept:

```rust
let provider = |node: tree_sitter::Node<'_>| {
    rope_chunks_for(node.byte_range())
};
```

This matters because built-in text predicates such as `#eq?` and `#match?` must inspect capture text.

## 15.4 Capture-centric iteration

The `captures` stream yields capture occurrences associated with their match context. Use it when your output is naturally a flat stream of classifications and you do not need to correlate multiple captures from the same match.

For symbol extraction, matches are usually clearer because name/container/signature captures must be assembled into one record.

## 15.5 Materialize spans, not nodes

```rust
struct CaptureRecord {
    pattern_index: usize,
    capture_index: u32,
    start_byte: usize,
    end_byte: usize,
}
```

Persisting spans keeps caches independent of Tree lifetimes and reduces memory retention.

## 15.6 Reusing `QueryCursor`

```rust
let mut cursor = QueryCursor::new();
for file in files {
    // reset/configure range as needed
    let mut matches = cursor.matches(&query, file.root(), file.bytes());
    while let Some(m) = matches.next() { /* ... */ }
}
```

Cursor reuse avoids repeated native allocation. Ensure per-run range/depth/match-limit settings are intentionally reset or configured.

## 15.7 Query result ordering

Do not design correctness around undocumented incidental ordering beyond the cursor/query API's intended capture/match iteration behavior. If downstream output requires deterministic lexical order across multiple independent queries, sort materialized domain records by `(start_byte, end_byte, kind)` explicitly.

## 15.8 Match/capture anti-patterns

* Collecting borrowed `QueryMatch` values across iterator advances.
* Storing `Node` in a long-lived index instead of materialized range/kind/text data.
* Repeatedly calling `capture_names()` string lookups inside huge loops instead of prebinding indexes.
* Using `captures` when multiple captures must be correlated into one entity.
* Assuming every capture name occurs once per match despite quantifiers.

---

# Tree-sitter Advanced — 16) Query predicates and directives

## 16.0 Predicate model

Predicate S-expressions begin with `#`, and predicates conventionally end in `?`. They attach conditions/metadata to a query pattern. The official query guide documents text equality, regex matching, set membership, property predicates, and directives. ([Predicates][query-predicates])

## 16.1 Equality family

```scheme
((identifier) @variable.builtin
  (#eq? @variable.builtin "self"))
```

Families:

```text
#eq?
#not-eq?
#any-eq?
#any-not-eq?
```

For quantified captures, the `any-` variants change the default “all captured nodes must satisfy” behavior to “at least one captured node satisfies.”

## 16.2 Regex family

```scheme
((identifier) @constant
  (#match? @constant "^[A-Z][A-Z_]+"))
```

Complement/quantified variants follow the same pattern (`not-`, `any-`).

The Rust `Query` constructor parses common text predicates and stores the associated predicate machinery used by query execution. Regex-heavy queries can materially affect query cost; anchor patterns structurally before relying on broad regex filters.

## 16.3 `any-of?`

```scheme
((identifier) @builtin
  (#any-of? @builtin
    "arguments"
    "module"
    "console"
    "window"
    "document"))
```

Prefer `any-of?` to a giant regex alternation when the intended set is literal and finite.

## 16.4 Property predicates: `is?` / `is-not?`

```scheme
((identifier) @variable.builtin
  (#is-not? local))
```

In Rust, inspect via:

```rust
for (property, positive) in query.property_predicates(pattern_index) {
    println!("property={property:?} positive={positive}");
}
```

Properties require a consumer that gives them meaning. The query syntax can carry metadata about `local`, but your application/highlighting pipeline decides how that property is populated/interpreted.

## 16.5 `set!` directives

A pattern can attach settings:

```scheme
((comment) @injection.content
  (#set! injection.language "doxygen"))
```

Rust:

```rust
for property in query.property_settings(pattern_index) {
    println!("setting={property:?}");
}
```

This mechanism is heavily used in highlighting/injection query conventions.

## 16.6 General predicates

`Query::general_predicates(pattern_index)` returns user-defined predicates that are not among the Rust binding's recognized `match?`, `eq?/not-eq?`, `is?/is-not?`, and `set!` families. ([Query source][query-source])

This is critical: **arbitrary predicates are metadata until your host interprets them.** Do not assume the Tree-sitter C core magically executes application-defined predicate names.

Conceptual dispatcher:

```rust
for pred in query.general_predicates(pattern) {
    match pred.operator.as_ref() {
        "my-domain?" => evaluate_domain_predicate(pred, captures),
        other => return Err(format!("unsupported predicate #{other}")),
    }
}
```

Use the actual current struct fields exposed by your pinned crate when implementing this; the design point is to make unsupported predicates explicit.

## 16.7 Directives are consumer conventions

Official examples include directives used by highlighting tooling such as `#set!`; higher-level tools can define additional directives such as source-adjustment behavior. The core query grammar can carry them, but the host library/tool is responsible for semantics.

## 16.8 Predicate performance

Order of attack:

```text
1. constrain by grammar node kind
2. constrain by field / parent shape
3. constrain by anchors/adjacency
4. only then evaluate capture text predicate
```

A query that captures every identifier and regex-filters afterward may be dramatically more expensive than a query structurally narrowed to the relevant declaration context.

## 16.9 Predicate anti-patterns

* Assuming custom predicates run without host implementation.
* Encoding structural conditions as expensive source-text regexes.
* Using a regex alternation for a finite keyword set when `any-of?` expresses intent.
* Ignoring quantified-capture `any-` semantics.
* Treating `#set!` as a parser mutation; it is pattern metadata for consumers.

---

# Tree-sitter Advanced — 17) Query cursor controls and containment

## 17.0 Cursor state

`QueryCursor` is the mutable execution object. The current Rust API exposes match/capture execution, range controls, match limit, overflow detection, and max start depth. ([QueryCursor][query-cursor])

## 17.1 Byte range

```rust
cursor.set_byte_range(start..end);
```

Ordinary range restriction returns matches that **intersect** the range. A capture/match may start outside the requested range if part of it intersects. The official query API documents this behavior. ([Query API][query-api])

This is ideal for incremental invalidation when a changed byte range might be contained inside a larger syntax entity that must be recomputed.

## 17.2 Point range

```rust
cursor.set_point_range(start_point..end_point);
```

Use when the caller naturally owns Tree-sitter `Point` coordinates. Byte ranges are usually simpler and faster to integrate with source slices and cache intervals.

## 17.3 Containing ranges

```rust
cursor.set_containing_byte_range(start..end);
// or
cursor.set_containing_point_range(start_point..end_point);
```

Containing variants only return matches whose captured nodes are fully within the range. ([Query API][query-api])

Decision:

```text
Need any affected entity around edit → ordinary intersecting range
Need output strictly inside a selected region → containing range
```

## 17.4 Zero end is special at the C API level

The official API notes that an end value of zero is treated as unbounded, so `(0,0)` means the full tree rather than empty range. Rust range methods wrap the same native semantics. Avoid clever zero-length range encodings; if you want “nothing,” do not execute the query.

## 17.5 Maximum start depth

```rust
cursor.set_max_start_depth(Some(depth));
```

Use when patterns should start only within a bounded depth from the execution node. This can reduce work in deeply nested trees when the product only needs top-level declarations.

For example, a top-level symbol index can query the file root with a small start-depth policy rather than matching nested definitions accidentally.

## 17.6 Match limit

```rust
cursor.set_match_limit(8192);
let limit = cursor.match_limit();
```

Current Rust docs constrain the limit to `> 0` and `<= 65536`. After execution:

```rust
if cursor.did_exceed_match_limit() {
    // Results may be incomplete; surface degradation/diagnostic.
}
```

([QueryCursor][query-cursor])

A match limit is a **correctness signal**, not merely a performance stat. If exceeded, do not silently treat the partial results as complete index data.

## 17.7 Cursor configuration wrapper

```rust
#[derive(Clone, Debug)]
struct QueryBudget {
    match_limit: u32,
    max_start_depth: Option<u32>,
}

fn configure(cursor: &mut tree_sitter::QueryCursor, b: &QueryBudget) {
    cursor.set_match_limit(b.match_limit);
    cursor.set_max_start_depth(b.max_start_depth);
}
```

Centralize limits so different extractors do not accidentally run with wildly different resource behavior.

## 17.8 Range-local extraction pattern

```rust
for changed in changed_ranges {
    let start = changed.start_byte;
    let end = changed.end_byte;

    cursor.set_byte_range(start..end);
    let mut matches = cursor.matches(&query, new_tree.root_node(), source);
    while let Some(m) = tree_sitter::StreamingIterator::next(&mut matches) {
        // replace affected records
    }

    if cursor.did_exceed_match_limit() {
        return Err("query match limit exceeded".into());
    }
}
```

Consider merging overlapping/nearby changed ranges before executing the same query repeatedly.

## 17.9 Cursor anti-patterns

* Range-limiting a non-local query without tests proving complete extraction.
* Ignoring `did_exceed_match_limit()`.
* Treating intersecting range semantics as fully-contained semantics.
* Forgetting cursor configuration persists into subsequent executions.
* Sharing one cursor among simultaneously running query iterators.

---

# Tree-sitter Advanced — 18) Parse/query progress and cancellation

## 18.0 Modern progress callbacks

The 0.26 Rust API provides callback-based cooperative cancellation for both parsing and query execution:

```text
ParseOptions::progress_callback(&ParseState) -> ControlFlow<()>
QueryCursorOptions::progress_callback(&QueryCursorState) -> ControlFlow<()>
```

`ParseState` exposes current byte offset and whether an error has been encountered; `QueryCursorState` exposes current byte offset. ([ParseOptions][parse-options]) ([QueryCursorOptions][query-cursor-options])

## 18.1 Parse deadline

```rust
use std::{ops::ControlFlow, time::{Duration, Instant}};
use tree_sitter::ParseOptions;

let deadline = Instant::now() + Duration::from_millis(50);
let mut progress = |_state: &tree_sitter::ParseState| {
    if Instant::now() >= deadline {
        ControlFlow::Break(())
    } else {
        ControlFlow::Continue(())
    }
};

let options = ParseOptions::new().progress_callback(&mut progress);
let tree = parser.parse_with_options(
    &mut |byte, _point| &source[byte..],
    old_tree,
    Some(options),
);

if tree.is_none() {
    parser.reset();
}
```

Use the exact `parse_with_options` overload appropriate to your source adapter. The important contract is that a break cancels parsing and returns no completed tree.

## 18.2 Parse telemetry

```rust
let mut furthest = 0usize;
let mut saw_error = false;
let mut progress = |s: &tree_sitter::ParseState| {
    furthest = furthest.max(s.current_byte_offset());
    saw_error |= s.has_error();
    ControlFlow::Continue(())
};
```

Do not perform expensive logging on every callback invocation. Accumulate cheap counters/flags and emit one record after the parse.

## 18.3 Reusing `ParseOptions`

`ParseOptions::reborrow()` exists so one options object can be reused across calls with a shortened borrow lifetime. ([ParseOptions][parse-options])

Use this when a worker owns one callback state and parses multiple documents serially.

## 18.4 Query deadline

Conceptual current-Rust pattern:

```rust
use std::ops::ControlFlow;
use tree_sitter::{QueryCursorOptions, StreamingIterator};

let deadline = std::time::Instant::now() + std::time::Duration::from_millis(20);
let mut progress = |_state: &tree_sitter::QueryCursorState| {
    if std::time::Instant::now() >= deadline {
        ControlFlow::Break(())
    } else {
        ControlFlow::Continue(())
    }
};

let options = QueryCursorOptions::new().progress_callback(&mut progress);
let mut matches = cursor.matches_with_options(
    &query,
    root,
    source,
    options,
);

while let Some(m) = matches.next() {
    // consume completed matches before cancellation
}
```

Pin/check the exact `matches_with_options` signature when copying into code; the current API's semantic point is that query progress is separately cancellable from parse progress.

## 18.5 Cancellation policy by workload

| Workload | Recommended budget behavior |
|---|---|
| interactive editor parse | short deadline; retain previous tree if new parse cancelled |
| background reparse | larger deadline; retry full/incremental according to queue policy |
| repository batch index | per-file wall/CPU budget; record failure and continue |
| user query endpoint | strict parse + query budgets; match limits; response truncation policy |
| grammar test/fuzz | often no tight deadline, but external harness timeout |

## 18.6 Previous-tree behavior on cancelled parse

Never replace the document's last known-good `Tree` with `None`. Keep the prior tree and schedule retry/fallback if product semantics permit.

```text
new parse succeeds → commit new source/tree revision pair
new parse cancelled → do not commit half-created tree; preserve old stable tree or mark syntax stale
```

If source text itself has already committed to a new revision, retaining an old tree requires marking it stale and never slicing new source with old node ranges.

## 18.7 Query cancellation completeness

A cancelled query can yield a prefix/partial set of matches. Unless your product explicitly supports partial output, mark the extraction incomplete and do not publish it as a full symbol/call/reference index.

## 18.8 Budget layering

```text
request deadline
  ├─ parse progress callback
  ├─ query progress callback
  ├─ query match_limit
  ├─ query range/depth restriction
  └─ service-level output record/byte cap
```

No single mechanism replaces the others. Match limit bounds in-progress query state; a progress callback bounds time/work; range/depth reduce search domain.

## 18.9 Cancellation anti-patterns

* Treating `Option<Tree>::None` as a syntax error rather than cancellation/no completed parse.
* Cancelling and then reusing parser partial state on an unrelated document without `reset()`.
* Publishing partial query captures as complete results.
* Performing expensive locks/logging inside progress callbacks.
* Setting only a query match limit and calling that a time budget.


# Tree-sitter Advanced — 19) Included ranges and multi-language documents

## 19.0 Mental model: one parser, one language, selected source ranges

Tree-sitter's included-range facility does **not** turn one parser into a polyglot parser. A `Parser` still has exactly one active `Language`; included ranges tell that parser which disjoint spans of the original document belong to that language.

```text
full source document
  ├─ host syntax
  ├─ embedded language A range
  ├─ host syntax
  ├─ embedded language A range
  └─ host syntax

Parser(language = A)
  + set_included_ranges([range1, range2])
  → one A-language Tree whose coordinates still refer to the full document
```

This is the correct primitive for templates, fenced code blocks, script/style sections, tagged strings, notebooks, and other mixed-language files. The application remains responsible for discovering injection regions and orchestrating one parser/tree per language layer. See [advanced-parsing].

## 19.1 `Range` contract

```rust
use tree_sitter::{Point, Range};

let range = Range {
    start_byte: 12,
    end_byte: 42,
    start_point: Point::new(1, 0),
    end_point: Point::new(3, 5),
};
```

A `Range` carries **both** byte offsets and row/column `Point`s. They must describe the same positions in the source revision. Never derive only the byte coordinates and leave stale points: parser coordinates, diagnostics, and included-range validation assume the pair is coherent.

## 19.2 Setting included ranges

Current Rust API:

```rust
use tree_sitter::{Parser, Point, Range};

fn configure_embedded_ranges(parser: &mut Parser) -> Result<(), tree_sitter::IncludedRangesError> {
    let ranges = [
        Range {
            start_byte: 20,
            end_byte: 80,
            start_point: Point::new(1, 6),
            end_point: Point::new(4, 2),
        },
        Range {
            start_byte: 120,
            end_byte: 180,
            start_point: Point::new(7, 4),
            end_point: Point::new(10, 0),
        },
    ];

    parser.set_included_ranges(&ranges)
}
```

`Parser::set_included_ranges` accepts multiple disjoint ranges. They must be ordered from earliest to latest and may not overlap. The Rust method returns `IncludedRangesError` identifying the first incorrect range when that invariant is violated. Passing an empty slice restores whole-document parsing. See [parser].

## 19.3 Read back parser configuration

```rust
let configured: Vec<tree_sitter::Range> = parser.included_ranges();
for range in configured {
    println!("{}..{}", range.start_byte, range.end_byte);
}
```

Treat `included_ranges()` as parser configuration introspection, not as a derived semantic language map. Your document/injection layer should remain the source of truth for why each range exists.

## 19.4 Coordinates remain document-global

Included-range trees retain coordinates relative to the **original entire document**. That is the principal reason to use the API instead of concatenating embedded snippets into a temporary buffer.

```text
Bad adapter:
  extract snippets → concatenate → parse → invent source-map back to host

Preferred adapter:
  discover host ranges → set_included_ranges → parse original source callback
  → nodes already have full-document byte/point coordinates
```

This dramatically simplifies diagnostics, capture ranges, edit propagation, and source slicing.

## 19.5 Two-parser HTML/template pattern

Conceptual orchestration:

```rust
struct LayerTrees {
    host_tree: tree_sitter::Tree,
    embedded_tree: tree_sitter::Tree,
}

fn parse_layers(
    source: &[u8],
    host_parser: &mut tree_sitter::Parser,
    embedded_parser: &mut tree_sitter::Parser,
    embedded_ranges: &[tree_sitter::Range],
) -> Option<LayerTrees> {
    let host_tree = host_parser.parse(source, None)?;

    embedded_parser.set_included_ranges(embedded_ranges).ok()?;
    let embedded_tree = embedded_parser.parse(source, None)?;

    Some(LayerTrees { host_tree, embedded_tree })
}
```

In a real injection pipeline the host tree is normally queried first to *derive* the embedded ranges, then the embedded parser executes over the same source revision.

## 19.6 Included ranges and incremental parsing

When the source changes, both the old tree and the included-range coordinates may need adjustment.

```text
edit source revision N → N+1
  1. compute InputEdit
  2. edit host tree
  3. incrementally reparse host
  4. rerun injection query in affected host region
  5. recompute / edit embedded ranges
  6. edit old embedded tree using same source edit when still valid
  7. set new included ranges
  8. incrementally reparse embedded layer
```

Do **not** assume an old injection boundary remains correct merely because the text edit was outside the embedded payload. A host edit can change delimiters, nesting, or parse recovery and thereby move or invalidate language ranges.

## 19.7 Range normalization

A robust injection layer should normalize before calling Tree-sitter:

```text
sort by start_byte
reject start > end
reject overlapping ranges
coalesce only when language semantics permit it
validate byte/point correspondence against the same source snapshot
exclude zero-length ranges unless they carry explicit product meaning
```

Do not silently coalesce ranges merely to satisfy the non-overlap invariant: adjacency and separation may carry parser-state semantics.

## 19.8 Injection ownership record

For code-intelligence systems, store more than raw `Range`:

```rust
#[derive(Clone)]
struct InjectionRegion {
    language_id: String,
    range: tree_sitter::Range,
    host_node_kind: String,
    host_node_id: usize,
    revision: u64,
}
```

The host-node provenance lets the indexer invalidate an embedded tree when the owning structural node changes, even if a byte-range heuristic appears unchanged.

## 19.9 Recursive injections

Tree-sitter does not impose a one-level limit. An application can recursively run injection discovery:

```text
HTML tree
  → <script> JavaScript ranges
      → tagged template / GraphQL ranges
  → <style> CSS ranges
```

Production policy should cap recursion depth and repeated-language cycles. A malformed query or grammar should not be able to generate an unbounded injection graph.

## 19.10 Parser pool design

A parser has mutable language/range/logger state. Prefer a parser pool keyed by grammar/language identity rather than repeatedly mutating a single global parser under contention.

```text
ParserPool
  rust        → [Parser, Parser, ...]
  javascript  → [Parser, Parser, ...]
  html        → [Parser, Parser, ...]
```

Before returning a parser to the pool:

```text
clear/replace included ranges according to pool contract
clear request-local logger
call reset() when prior parse was cancelled and state must not resume
ensure the expected language is installed
```

## 19.11 Anti-pattern inventory

* Concatenating embedded snippets and then reconstructing host coordinates manually.
* Passing unsorted or overlapping included ranges.
* Reusing stale `Point` values after adjusting only byte offsets.
* Treating one parser with many ranges as a multi-language parser.
* Failing to recompute injections after a host-tree structural change.
* Publishing embedded captures whose source revision does not match the host tree.
* Allowing recursive injection graphs without depth/cycle limits.

## 19.12 Agent checklist

```text
[ ] One Parser has one active Language.
[ ] Discover embedded ranges from a host-language tree/query.
[ ] Keep byte and Point coordinates coherent.
[ ] Sort ranges and ensure they do not overlap.
[ ] Pass [] to return to whole-document parsing.
[ ] Parse the original full source so coordinates stay document-global.
[ ] Recompute injection ranges after relevant host edits.
[ ] Version every host tree, embedded tree, and injection record together.
[ ] Bound recursive injections and parser-resource use.
```

# Tree-sitter Advanced — 20) Error recovery and incomplete code

## 20.0 Tree-sitter's contract: produce a tree through errors

Tree-sitter is designed for editing and analysis of incomplete source. A parse does not normally fail merely because code is syntactically invalid. Instead, the resulting concrete syntax tree records recovery using `ERROR` nodes and zero-width `MISSING` nodes.

```text
valid source       → ordinary grammar nodes
unrecognized span  → ERROR node
expected token absent but recoverable → MISSING node
```

This distinction is central for IDEs and continuously updating code graphs: **`has_error()` is metadata about tree quality, not a reason to discard the tree.** See [query-syntax] and [node].

## 20.1 Error inspection API

```rust
let root = tree.root_node();

if root.has_error() {
    eprintln!("tree contains at least one syntax error");
}

fn classify(node: tree_sitter::Node<'_>) {
    if node.is_error() {
        println!("ERROR at {:?}", node.range());
    }
    if node.is_missing() {
        println!("MISSING {} at {:?}", node.kind(), node.range());
    }
}
```

Useful node flags:

```text
is_error()    → this node itself is an ERROR node
is_missing()  → parser inserted this zero-width missing node
has_error()   → this node or a descendant contains syntax error/recovery
is_extra()    → node corresponds to an extra such as a comment, depending on grammar
```

## 20.2 ERROR vs MISSING

`ERROR` and `MISSING` convey different recovery evidence.

```text
ERROR:
  parser consumed source text it could not incorporate into the grammar
  typically has a non-empty source range

MISSING:
  parser inferred an expected construct without consuming source text
  typically has a zero-width range at the insertion position
  `kind()` describes what was missing
```

For diagnostics, an `ERROR` often means “unexpected material here”; a `MISSING` node often provides a stronger completion hint: “this specific symbol/kind was expected here.”

## 20.3 Querying recovery nodes

Query syntax can match these recovery forms directly:

```scheme
(ERROR) @syntax.error
(MISSING) @syntax.missing
(MISSING ";") @missing.semicolon
```

This lets a single query pipeline emit syntax-recovery diagnostics alongside ordinary semantic captures. See [query-syntax].

## 20.4 Error-aware traversal

```rust
fn collect_problem_nodes<'tree>(root: tree_sitter::Node<'tree>) -> Vec<tree_sitter::Node<'tree>> {
    let mut out = Vec::new();
    let mut stack = vec![root];

    while let Some(node) = stack.pop() {
        if !node.has_error() {
            continue; // entire subtree is clean
        }
        if node.is_error() || node.is_missing() {
            out.push(node);
        }
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                stack.push(cursor.node());
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    out
}
```

The `has_error()` guard is a cheap structural pruning opportunity: skip clean subtrees rather than exhaustively inspecting every node for diagnostics.

## 20.5 Partial semantic extraction policy

A parser recovery tree can still contain trustworthy declarations outside the damaged region. Do not make “root has error” an all-or-nothing gate.

Recommended record-level policy:

```text
capture function_definition
  if capture node itself has no error descendants → high confidence
  if error exists only in function body but signature is clean → signature can remain usable, lower confidence if needed
  if ERROR/MISSING intersects identifier/type/binding syntax → mark symbol incomplete or suppress
```

Quality should be attached to the **smallest semantic unit**, not only to the file.

## 20.6 Error containment helper

```rust
fn intersects_error(node: tree_sitter::Node<'_>) -> bool {
    node.has_error()
}

fn is_recovery_artifact(node: tree_sitter::Node<'_>) -> bool {
    node.is_error() || node.is_missing()
}
```

For higher precision, query the actual error/missing ranges and test overlap with semantic capture ranges rather than relying only on ancestor `has_error()`.

## 20.7 Incomplete code and stable IDs

Never assume a recovery node's shape or ID is stable across subsequent keystrokes. Error-recovery structure can reorganize substantially when a delimiter is added. Cache semantic entities by a stronger product identity strategy, and use changed ranges / owning stable nodes to invalidate them.

## 20.8 Diagnostic severity

Suggested distinction:

| Evidence | Suggested interpretation |
|---|---|
| `MISSING` punctuation | precise parse-recovery hint; often low-to-medium severity |
| `MISSING` identifier/type | stronger incomplete construct signal |
| small `ERROR` leaf span | unexpected token/fragment |
| large `ERROR` subtree | parser lost structure; lower semantic confidence over region |
| root `has_error`, capture clean | do not automatically reject capture |

The grammar—not Tree-sitter core—determines the exact recovery shape, so diagnostics should be tested per grammar/version.

## 20.9 Syntax errors are not parser API failures

Separate these states explicitly:

```text
Some(Tree) + root.has_error() = parse completed, syntax invalid/incomplete
Some(Tree) + !root.has_error() = parse completed, structurally valid per grammar
None                        = no completed tree (no language / cancellation or callback-related termination)
Err(set_language/...)       = configuration/API problem
```

This prevents a common service bug where invalid source is incorrectly surfaced as infrastructure failure.

## 20.10 Best practices for code-intelligence ingestion

```text
Always retain the completed tree.
Extract unaffected semantics even when errors exist elsewhere.
Attach source revision + grammar version to all error diagnostics.
Prefer range-local confidence to file-global rejection.
Index ERROR/MISSING diagnostics separately from semantic graph facts.
Invalidate aggressively when an edit touches a recovery region.
```

## 20.11 Anti-pattern inventory

* Rejecting every tree whose root `has_error()`.
* Treating `ERROR` and `MISSING` as identical.
* Assuming `MISSING` nodes consume source bytes.
* Emitting source text for zero-width missing nodes as if it were a real token.
* Caching recovery-node IDs as durable semantic identities.
* Publishing semantic facts from an error-intersecting capture without quality policy.
* Reporting `None` from `parse` as “syntax error.”

## 20.12 Agent checklist

```text
[ ] Distinguish completed-with-errors from parser failure/cancellation.
[ ] Use is_error / is_missing / has_error intentionally.
[ ] Query ERROR and MISSING directly when useful.
[ ] Prune clean subtrees during diagnostic collection.
[ ] Assess error overlap per semantic record.
[ ] Keep unaffected symbols even when another region is broken.
[ ] Invalidate recovery-heavy regions aggressively on edits.
```

# Tree-sitter Advanced — 21) Parse states, lookahead, completion, and diagnostics

## 21.0 Why parser state matters

Tree-sitter exposes LR parse-state information on nodes and a language-level lookahead iterator over valid symbols in a parse state. This gives an IDE or diagnostic engine a grammar-derived answer to:

```text
"What symbols could legally come next here?"
```

rather than relying purely on hand-coded token lists.

Core API relationship:

```text
Node::parse_state() / Node::next_parse_state()
  + Node::grammar_id()
  + Language::next_state(...)
  + Language::lookahead_iterator(state)
    → valid grammar symbols for that state
```

See [language] and [node].

## 21.1 Node parse states

```rust
let node = tree.root_node();
println!("before={}", node.parse_state());
println!("after={}", node.next_parse_state());
```

`parse_state()` is the parser state at the node's position; `next_parse_state()` exposes the state after the node. These are grammar/parser implementation details: pin grammar/runtime versions if persisted or interpreted across process boundaries.

## 21.2 `Language::next_state`

```rust
let state_after = language.next_state(node.parse_state(), node.grammar_id());
```

The method combines a parse state and grammar symbol to compute the next parse state. This can be paired with lookahead iteration to reason about valid continuations.

Use `grammar_id()` rather than assuming `kind_id()` is always the semantically correct symbol input for parser-state logic; aliases can distinguish node presentation from underlying grammar symbol identity.

## 21.3 Lookahead iterator

```rust
if let Some(mut iter) = language.lookahead_iterator(node.next_parse_state()) {
    for symbol_id in &mut iter {
        println!("valid symbol id: {symbol_id}");
    }
}
```

A newly created `LookaheadIterator` represents the symbols that are valid in the specified parse state. Invalid states yield `None`. See [lookahead].

For product use, convert symbol IDs to language names and filter internal/error symbols according to UI policy.

## 21.4 Completion candidate pipeline

```text
cursor position
  → locate structural node / leaf near cursor
  → determine appropriate parse state
  → iterate grammar-valid symbols
  → map symbol IDs to node/token names
  → filter anonymous punctuation / internals if desired
  → enrich with scope/type/name-resolution candidates
  → rank product completions
```

Tree-sitter lookahead answers **syntactic possibility**, not semantic validity. It does not know which variable names are in scope, which methods exist on a type, or whether a symbol is accessible.

## 21.5 ERROR-node diagnostic guidance

Official language API guidance recommends using a lookahead iterator on the first leaf node's parse state for symbols valid inside an `ERROR` node. That state can expose what the parser expected when recovery began. See [language].

Conceptual helper:

```rust
fn first_leaf(mut node: tree_sitter::Node<'_>) -> tree_sitter::Node<'_> {
    loop {
        match node.child(0) {
            Some(child) => node = child,
            None => return node,
        }
    }
}
```

Then:

```rust
if error_node.is_error() {
    let leaf = first_leaf(error_node);
    if let Some(iter) = language.lookahead_iterator(leaf.parse_state()) {
        // inspect syntactically valid symbols
    }
}
```

## 21.6 MISSING-node diagnostic guidance

For a `MISSING` node, the preceding non-extra leaf's state is often the useful state for lookahead, because the missing node represents what should have followed the already-parsed prefix. Official Rust docs explicitly call out this strategy. See [language].

## 21.7 Symbol-name conversion

`Language` exposes node-kind inventory and symbol-name lookup. A completion layer can produce stable internal records:

```rust
#[derive(Debug)]
struct SyntaxCandidate {
    symbol_id: u16,
    name: String,
    named: bool,
    visible: bool,
}
```

Population is grammar-version dependent; use `node_kind_for_id`, `node_kind_is_named`, and `node_kind_is_visible` or corresponding current language metadata APIs.

## 21.8 Filter policy

Raw lookahead may contain symbols unsuitable for end-user completion:

```text
internal hidden grammar symbols
ERROR
anonymous punctuation
closing delimiters
keywords
named grammar categories
```

Maintain two layers:

```text
raw grammar-valid symbols = debugging / correctness evidence
user completion candidates = product-filtered + semantic enrichment
```

Do not destroy the raw list before diagnostics; it can be invaluable when explaining why a completion disappeared after a grammar upgrade.

## 21.9 Parse state as ephemeral implementation detail

Do not store a bare `u16` state in a durable database without its grammar identity/version.

```rust
struct VersionedParseState {
    grammar: String,
    grammar_abi: u32,
    grammar_version: String,
    state: u16,
}
```

Even that should usually be diagnostic/cache metadata, not a public durable interface. Grammar regeneration can change state numbering without changing high-level language syntax.

## 21.10 Completion ranking architecture

Recommended split:

```text
Tree-sitter layer:
  syntactically valid terminal/nonterminal candidates

semantic graph / compiler layer:
  names in scope
  inferred types
  member candidates
  imports/modules
  visibility

ranking layer:
  prefix match
  usage frequency
  locality
  expected syntactic category
  semantic type compatibility
```

Tree-sitter should constrain the search space, not replace a semantic analyzer.

## 21.11 Anti-pattern inventory

* Treating lookahead candidates as complete semantic completions.
* Persisting raw parse-state numbers as a stable external API.
* Feeding presentation aliases into state transitions without considering `grammar_id()`.
* Showing every hidden/internal symbol to users.
* Guessing expected tokens from an `ERROR` node's text alone when parser state is available.
* Expecting lookahead to resolve identifiers or types.

## 21.12 Agent checklist

```text
[ ] Use node parse-state APIs only under a pinned grammar/runtime context.
[ ] Use Language::lookahead_iterator for syntax-valid candidates.
[ ] Use grammar_id for parser-state transitions where appropriate.
[ ] For ERROR, inspect the first leaf's state.
[ ] For MISSING, consider the previous non-extra leaf's state.
[ ] Keep raw grammar candidates separate from product-ranked completions.
[ ] Add semantic/scope resolution above the Tree-sitter layer.
```

# Tree-sitter Advanced — 22) Static node types and typed Rust wrappers

## 22.0 `node-types.json` is the grammar's static CST schema

Generated grammars include `src/node-types.json`, a structured description of every possible syntax-node type. It is the correct source for generating typed wrappers, schema metadata, query validation helpers, and agent-facing node catalogs. See [static-node-types].

At minimum, each top-level node type contains:

```json
{
  "type": "function_definition",
  "named": true
}
```

The pair `(type, named)` uniquely identifies a top-level node-type entry in the generated schema.

## 22.1 Internal-node schema

Node types that can contain children may include:

```json
{
  "type": "example",
  "named": true,
  "fields": {
    "name": {
      "required": true,
      "multiple": false,
      "types": [
        { "type": "identifier", "named": true }
      ]
    }
  },
  "children": {
    "required": false,
    "multiple": true,
    "types": [
      { "type": "comment", "named": true }
    ]
  }
}
```

Key contracts:

```text
fields       → named child slots addressable by field name
children     → possible named children without fields
required     → at least one is expected for structurally valid trees
multiple     → cardinality may exceed one
types        → allowed child node-type alternatives
```

Error recovery can still violate ideal cardinality at runtime, so generated wrappers must return `Option`/iterators rather than using unchecked assumptions.

## 22.2 Supertypes

A grammar can declare abstract categories as `supertypes`; generated `node-types.json` records the subtype relationship without forcing an extra visible CST layer for every abstract grammar category. See [grammar-writing] and [static-node-types].

This is useful for typed modeling:

```rust
#[derive(Debug, Clone, Copy)]
enum Expression<'tree> {
    Identifier(Identifier<'tree>),
    Call(CallExpression<'tree>),
    Binary(BinaryExpression<'tree>),
    // ... grammar-derived variants
}
```

The wrapper generator can build these enums from the static supertype/subtype catalog instead of maintaining a hand-written list.

## 22.3 Grammar crates commonly expose `NODE_TYPES`

Many official/community Rust grammar crates embed the generated JSON as a constant:

```rust
let raw: &str = tree_sitter_json::NODE_TYPES;
```

The exact exported constants are grammar-crate specific; inspect that grammar's Rust docs. `tree-sitter-json` currently exposes both its `LANGUAGE` function wrapper and `NODE_TYPES`, making it suitable for compile-time schema generation or test-time validation. See [json-crate].

## 22.4 Deserialize schema at build/test time

Illustrative model:

```rust
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
struct NodeType {
    #[serde(rename = "type")]
    kind: String,
    named: bool,
    #[serde(default)]
    fields: BTreeMap<String, ChildSpec>,
    children: Option<ChildSpec>,
    #[serde(default)]
    subtypes: Vec<KindRef>,
}

#[derive(Debug, Deserialize)]
struct ChildSpec {
    required: bool,
    multiple: bool,
    types: Vec<KindRef>,
}

#[derive(Debug, Deserialize)]
struct KindRef {
    #[serde(rename = "type")]
    kind: String,
    named: bool,
}
```

Do not ship `serde_json` into the hot parse path merely to inspect node types at runtime if generated code can materialize the schema as Rust constants/types during build.

## 22.5 Typed wrapper pattern

```rust
#[derive(Clone, Copy)]
struct FunctionDefinition<'tree> {
    node: tree_sitter::Node<'tree>,
}

impl<'tree> FunctionDefinition<'tree> {
    fn cast(node: tree_sitter::Node<'tree>) -> Option<Self> {
        (node.kind() == "function_definition").then_some(Self { node })
    }

    fn name(self) -> Option<tree_sitter::Node<'tree>> {
        self.node.child_by_field_name("name")
    }

    fn node(self) -> tree_sitter::Node<'tree> {
        self.node
    }
}
```

Generation can remove repetitive string literals and make the application's accepted grammar surface explicit.

## 22.6 Use IDs for hot paths, names for contracts

Generated wrappers can resolve field/node IDs once:

```rust
let field_id = language.field_id_for_name("name");
let kind_id = language.id_for_node_kind("function_definition", true);
```

Then hot traversal can use IDs while logs/query files and durable configuration retain readable names. This reduces repeated string lookup without turning grammar numeric IDs into public contracts.

## 22.7 Typed-wrapper safety rule

Static node schema describes the grammar's **possible valid shapes**, not a guarantee that every live node is complete. Recovery may yield missing/error children.

Bad generated API:

```rust
fn name(self) -> Node<'tree> {
    self.node.child_by_field_name("name").unwrap()
}
```

Preferred:

```rust
fn name(self) -> Option<Node<'tree>> {
    self.node.child_by_field_name("name")
}
```

A separate validated/strict layer may upgrade optional fields after checking `!node.has_error()` and required-child presence.

## 22.8 Schema drift detection

At CI time:

```text
regenerate grammar
  → node-types.json changes?
  → generated Rust wrapper diff
  → query compile tests
  → semantic extractor golden tests
  → intentional review
```

This turns grammar upgrades into explicit schema migrations rather than silent extractor breakage.

## 22.9 Typed query/capture catalogs

Use node-types metadata to validate query literals:

```text
query references kind "function_declaration"
  ↓
static node catalog says kind exists?
  ↓ yes
field "name" valid on that kind?
  ↓ yes
compile Query as final authority
```

Static prevalidation produces better generator/agent diagnostics; `Query::new` remains the source of truth for runtime compatibility.

## 22.10 Agent-facing schema export

For an LLM/code agent, a compact grammar schema is more useful than dumping raw `node-types.json`:

```text
function_definition
  fields:
    name: identifier [required, single]
    parameters: parameters [required, single]
    body: block [required, single]

expression (supertype)
  subtypes: identifier | call_expression | binary_expression | ...
```

Generate this artifact mechanically from the pinned grammar package so prompt context cannot drift from the actual parser.

## 22.11 Anti-pattern inventory

* Hand-maintaining a node-kind enum while ignoring generated `node-types.json`.
* Assuming `required: true` means a runtime node can never be incomplete under recovery.
* Unwrapping every field child in an editor/indexer.
* Persisting numeric kind/field IDs without grammar version identity.
* Allowing grammar upgrades without node-schema diffs.
* Sending raw megabytes of node-types JSON to agents when a compact generated schema suffices.

## 22.12 Agent checklist

```text
[ ] Treat node-types.json as the static CST schema.
[ ] Generate typed wrappers or catalogs from the pinned grammar.
[ ] Preserve Option/iterator behavior for recovery cases.
[ ] Use supertypes to generate abstract Rust enums/categories.
[ ] Resolve numeric IDs for hot paths only.
[ ] Diff node schema on every grammar upgrade.
[ ] Compile all queries against the upgraded language in CI.
```

# Tree-sitter Advanced — 23) Language introspection and grammar metadata

## 23.0 `Language` is an introspectable grammar object

A Tree-sitter `Language` is not only a parser function pointer. The Rust API exposes the grammar's ABI, node-kind inventory, field inventory, parse-state count, supertypes/subtypes, symbol relationships, optional semantic metadata, name, and WASM status. See [language].

This is enough to build a runtime grammar registry without separately scraping generated C files.

## 23.1 Core language identity

```rust
let language: tree_sitter::Language = tree_sitter_json::LANGUAGE.into();

println!("abi={}", language.abi_version());
println!("name={:?}", language.name());
println!("metadata={:?}", language.metadata());
println!("wasm={}", language.is_wasm());
```

`name()` / `metadata()` depend on what the grammar was generated with and may be absent for older parsers. Do not make optional language metadata your only registry key.

## 23.2 ABI validation

```rust
let abi = language.abi_version();
let supported = (tree_sitter::MIN_COMPATIBLE_LANGUAGE_VERSION
    ..=tree_sitter::LANGUAGE_VERSION)
    .contains(&abi);

assert!(supported);
```

The parser itself enforces compatibility in `Parser::set_language`; explicit checks are still useful for registry diagnostics and startup health reports.

## 23.3 Node-kind inventory

Current `Language` APIs support enumerating/looking up node kinds:

```rust
for id in 0..language.node_kind_count() {
    let id = id as u16;
    if let Some(name) = language.node_kind_for_id(id) {
        println!(
            "{id}: {name} named={} visible={}",
            language.node_kind_is_named(id),
            language.node_kind_is_visible(id),
        );
    }
}
```

Use this for diagnostics, query generators, schema catalogs, completion-symbol filtering, and upgrade diffs.

## 23.4 Name → kind ID

```rust
let id = language.id_for_node_kind("string", true);
```

The `named` boolean is part of the lookup contract. A named grammar rule and an anonymous literal can share textual spelling but are semantically distinct node kinds.

## 23.5 Field inventory

Conceptual catalog:

```rust
for id in 1..=language.field_count() {
    let id = id as u16;
    if let Some(name) = language.field_name_for_id(id) {
        println!("field {id}: {name}");
    }
}
```

And reverse lookup:

```rust
if let Some(id) = language.field_id_for_name("name") {
    println!("name field id={id}");
}
```

Field IDs are excellent process-local performance keys but should be treated as grammar-version scoped.

## 23.6 Supertypes and subtypes

The language object exposes grammar supertype/subtype relationships. This enables runtime questions such as:

```text
"Which concrete grammar kinds are expressions?"
"Is this kind a subtype of the grammar's declaration category?"
```

These relationships are especially useful when generating extractors that should operate over abstract syntax categories without hard-coding every concrete node kind.

## 23.7 Parse-state inventory

```rust
let states = language.parse_state_count();
println!("parse states={states}");
```

Use parse-state count for diagnostics and bounds checks, not as a measure of grammar “quality.” State count changes naturally as grammar generation changes and is not a stable performance predictor by itself.

## 23.8 Registry record

```rust
#[derive(Debug)]
struct GrammarDescriptor {
    registry_name: String,
    package_version: String,
    abi_version: u32,
    language_name: Option<String>,
    is_wasm: bool,
    node_kind_count: usize,
    field_count: usize,
    parse_state_count: usize,
}
```

Populate at startup and expose in diagnostics. This makes production incidents answerable: “Which exact grammar did worker 17 parse this file with?”

## 23.9 Grammar fingerprinting

For stronger identity, combine external package/build metadata with generated artifacts:

```text
grammar registry name
crate/package version
Tree-sitter runtime version
grammar ABI
hash(node-types.json)
hash(query bundle)
optional git revision / build provenance
```

Do not derive a stable grammar identity only from `Language::name()`: two incompatible builds can legitimately report the same language name.

## 23.10 Startup validation

At process startup, validate all grammar/query bundles:

```text
for grammar in registry:
  construct Language
  verify ABI in supported range
  set on a parser
  compile highlight/index/injection/locals queries
  check required node kinds/fields
  record fingerprint
  fail fast or quarantine grammar on mismatch
```

This shifts failures from random user documents to deterministic deployment startup.

## 23.11 Grammar-diff report

When upgrading a grammar, generate:

```text
node kinds added / removed
named/visible flag changes
field names added / removed
supertype/subtype changes
ABI change
node-types hash change
query compile result changes
representative parse-tree golden diffs
```

This is a much stronger migration signal than “Cargo updated successfully.”

## 23.12 Anti-pattern inventory

* Using only language name as grammar identity.
* Treating numeric kind/field IDs as stable across grammar regeneration.
* Interpreting parse-state count as a direct quality/performance score.
* Loading plugins without validating grammar ABI.
* Waiting for a production query to discover a removed node kind.
* Mixing query bundles built against a different grammar fingerprint.

## 23.13 Agent checklist

```text
[ ] Record runtime + grammar package/build versions.
[ ] Validate ABI before serving traffic.
[ ] Build node-kind and field catalogs from Language/node-types.json.
[ ] Scope numeric IDs to the exact grammar build.
[ ] Compile every query bundle at startup/CI.
[ ] Generate grammar-diff reports during upgrades.
[ ] Include grammar fingerprint in parse/index diagnostics.
```

# Tree-sitter Advanced — 24) Creating a grammar crate for native Rust consumption

## 24.0 Scope distinction

A Tree-sitter grammar is typically *authored* in `grammar.js`, but a Rust application does not interpret that JavaScript at runtime. The Tree-sitter CLI generates parser artifacts (principally C), and the grammar's Rust package compiles those artifacts and exposes a Rust-facing `LanguageFn`/`LANGUAGE` constant.

```text
grammar.js
  → tree-sitter generate
      → src/parser.c
      → src/node-types.json
      → src/grammar.json
      → optional scanner.c / scanner.cc
  → Cargo build script compiles generated parser
  → Rust grammar crate exports LANGUAGE
  → application: let language: tree_sitter::Language = crate::LANGUAGE.into()
```

That is the native-Rust deployment model covered by this document. See [grammar-init], [grammar-generate], and [language-fn].

## 24.1 Initialize a grammar project

```bash
tree-sitter init
```

Current `tree-sitter init` can generate the Rust package surface when Rust bindings are enabled in `tree-sitter.json`:

```text
Cargo.toml
bindings/rust/build.rs
bindings/rust/lib.rs
```

It also creates language/package metadata and other language bindings according to configuration. See [grammar-init].

## 24.2 Recommended repository layout

```text
tree-sitter-my-language/
  Cargo.toml
  grammar.js
  tree-sitter.json
  bindings/
    rust/
      build.rs
      lib.rs
  queries/
    highlights.scm
    injections.scm
    locals.scm
    tags.scm
  src/
    parser.c                 # generated
    grammar.json             # generated
    node-types.json          # generated
    scanner.c                # optional handwritten external scanner
    tree_sitter/
      parser.h               # generated/support header
      alloc.h
      array.h
  test/
    corpus/
      declarations.txt
      expressions.txt
      errors.txt
```

Keep generated parser artifacts committed when that is the ecosystem convention for the grammar, so downstream Rust consumers do not need a JavaScript runtime or the Tree-sitter CLI merely to compile the crate.

## 24.3 `Cargo.toml` conceptual shape

A grammar crate commonly has a build script and `tree-sitter-language` as its small runtime-facing type bridge.

```toml
[package]
name = "tree-sitter-my-language"
version = "0.1.0"
edition = "2021"
build = "bindings/rust/build.rs"

[dependencies]
tree-sitter-language = "0.1"

[build-dependencies]
cc = "1"

[dev-dependencies]
tree-sitter = "0.26"
```

Use the versions produced/recommended by the current CLI template for a real grammar project. The application consuming the grammar normally depends on `tree-sitter` itself; the grammar crate can remain lightweight around `LanguageFn`.

## 24.4 Rust binding facade

Representative modern grammar binding:

```rust
use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_my_language() -> *const ();
}

pub const LANGUAGE: LanguageFn = unsafe {
    LanguageFn::from_raw(tree_sitter_my_language)
};

pub const NODE_TYPES: &str = include_str!("../../src/node-types.json");
```

The exact generated binding is CLI-version specific; do not hand-copy an old template without comparing it to the current generated output.

## 24.5 Build script responsibilities

Conceptually:

```text
Cargo build.rs
  → rerun if parser.c / scanner source changes
  → compile parser.c as C
  → compile scanner.c as C or scanner.cc as C++ when present
  → include src/ headers
  → link generated object(s) into grammar crate
```

`cc::Build` is commonly used. Keep compiler flags portable and minimal; the generated parser should not depend on application-specific global C flags.

## 24.6 Consumer smoke test

```rust
#[test]
fn grammar_loads_in_rust_runtime() {
    let language: tree_sitter::Language = tree_sitter_my_language::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).unwrap();

    let tree = parser.parse("example", None).unwrap();
    assert!(!tree.root_node().kind().is_empty());
}
```

Every grammar crate should have a Rust binding smoke test. This catches broken symbols, ABI mismatch, build-script errors, and packaging omissions before publication.

## 24.7 Generated artifacts as contracts

| Artifact | Runtime / tooling role |
|---|---|
| `parser.c` | generated parsing tables/lexer/parser code compiled into native grammar |
| `grammar.json` | structured grammar representation; can regenerate parser without reevaluating `grammar.js` |
| `node-types.json` | static CST schema for typed consumers |
| `scanner.c/.cc` | optional custom lexing state/logic |
| Rust `lib.rs` | exposes grammar to Rust as `LanguageFn` |
| query `.scm` files | higher-level extraction/highlighting/tagging contracts |

`tree-sitter generate` documents these generated outputs directly. See [grammar-generate].

## 24.8 CLI generation runtime

Grammar generation still requires a JavaScript-compatible grammar evaluation environment unless using generated `grammar.json` or the CLI's supported generation runtime options. Starting with CLI 0.26, `tree-sitter generate` also supports the experimental native QuickJS runtime option. This affects **grammar build tooling**, not the Rust parser runtime. See [grammar-generate].

## 24.9 Version policy

Version at least these independently:

```text
Tree-sitter Rust runtime version
Tree-sitter CLI version used for generation
grammar crate/package version
generated parser ABI
query bundle revision
```

A grammar crate update can alter CST shape without changing the runtime crate. Conversely, a runtime upgrade can change Rust API behavior without changing grammar source.

## 24.10 Packaging checklist

```text
[ ] Cargo package includes bindings/rust/*.
[ ] Package includes src/parser.c and required headers.
[ ] Include scanner source when grammar has externals.
[ ] Include node-types.json if NODE_TYPES is exported.
[ ] Include query files intentionally exposed as constants/assets.
[ ] Rust smoke test calls Parser::set_language and parses fixture.
[ ] CI tests packaged crate, not only repository working tree.
```

## 24.11 Anti-pattern inventory

* Calling a grammar crate “the Tree-sitter runtime.”
* Requiring downstream Rust users to run `tree-sitter generate` during ordinary application build.
* Hand-editing `parser.c` instead of changing grammar/scanner source.
* Publishing a grammar crate whose Rust binding symbol is untested.
* Updating generated files with a different CLI and not recording the generator version.
* Omitting query/schema assets expected by downstream extractors.

# Tree-sitter Advanced — 25) Grammar DSL reference

## 25.0 Mental model

Tree-sitter's grammar DSL specifies both **parse structure** and **lexical tokens**. The CLI transforms this declarative grammar into generated C tables/code; none of the JavaScript DSL calls occur in the Rust application's parse hot path. See [grammar-dsl].

## 25.1 Fundamental rule references

```javascript
export default grammar({
  name: 'mini',

  rules: {
    source_file: $ => repeat($.statement),
    statement: $ => seq($.identifier, ';'),
    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,
  },
});
```

`$` is the grammar-symbol object; `$.identifier` references another grammar rule.

## 25.2 `seq`

```javascript
function_definition: $ => seq(
  'fn',
  $.identifier,
  $.parameter_list,
  $.block,
),
```

`seq(a, b, c)` requires its arguments in order.

## 25.3 `choice`

```javascript
expression: $ => choice(
  $.identifier,
  $.number,
  $.call_expression,
  $.binary_expression,
),
```

`choice` describes alternatives; argument order is not a general parsing-precedence mechanism.

## 25.4 `repeat` / `repeat1`

```javascript
source_file: $ => repeat($.statement),
arguments: $ => repeat1($.expression),
```

```text
repeat(x)  → zero or more
repeat1(x) → one or more
```

## 25.5 `optional`

```javascript
return_statement: $ => seq(
  'return',
  optional($.expression),
  ';',
),
```

`optional(x)` matches zero or one instance.

## 25.6 `token`

```javascript
identifier: $ => token(seq(
  /[A-Za-z_]/,
  repeat(/[A-Za-z0-9_]/),
)),
```

`token(rule)` collapses a terminal-only DSL expression into one lexical token. It cannot wrap an arbitrary nonterminal reference like `token($.foo)`.

## 25.7 `token.immediate`

```javascript
member_access: $ => seq(
  $.expression,
  token.immediate('.'),
  $.identifier,
),
```

An immediate token disallows extras/whitespace before that token. Use only when adjacency is truly part of language syntax.

## 25.8 Parse precedence

```javascript
binary_expression: $ => choice(
  prec.left(2, seq($.expression, '*', $.expression)),
  prec.left(1, seq($.expression, '+', $.expression)),
),
```

Core forms:

```text
prec(N, rule)
prec.left([N], rule)
prec.right([N], rule)
prec.dynamic(N, rule)
```

Static `prec` resolves grammar-generation conflicts; dynamic precedence participates at runtime when GLR conflict paths remain intentionally ambiguous.

## 25.9 Lexical precedence

```javascript
keyword_if: $ => token(prec(2, 'if')),
identifier: $ => token(prec(1, /[a-z]+/)),
```

The key distinction is placement:

```text
prec(N, rule)             → parse precedence
 token(prec(N, rule))     → lexical precedence
```

Mixing the two is one of the most common grammar-debugging mistakes. See [grammar-writing].

## 25.10 `alias`

```javascript
property_name: $ => alias($.identifier, $.property_identifier),
```

If the alias target is a grammar symbol, the resulting node is named; if it is a string literal, it is anonymous. Aliasing changes the visible CST identity and therefore can affect queries/extractors.

## 25.11 `field`

```javascript
function_definition: $ => seq(
  'fn',
  field('name', $.identifier),
  field('parameters', $.parameter_list),
  field('body', $.block),
),
```

Fields are among the highest-leverage grammar features for Rust consumers: they replace positional child assumptions with named accessors.

## 25.12 `reserved`

Current grammar DSL supports reserved-word sets and contextual override via `reserved(wordset, rule)`. Use this for languages where keywords are reserved in some contexts but permitted as identifiers/property names in others. See [grammar-dsl].

## 25.13 Grammar-level configuration fields

Important top-level grammar functions:

```text
extras       → globally permitted tokens such as whitespace/comments
inline       → substitute a rule definition at usage sites; no runtime node
conflicts    → intentional LR conflicts explored using GLR
externals    → tokens supplied by handwritten scanner
precedences  → named parse-precedence groups
word         → token used for keyword extraction
supertypes   → abstract node categories hidden from tree but queryable
reserved     → global/contextual reserved word sets
```

## 25.14 Rust regex semantics

Grammar regexes are not evaluated by JavaScript's runtime regex engine during parsing. Tree-sitter generates lexer logic based on its supported Rust-regex-style syntax subset. Lookaround-style constructs are not generally available in grammar regexes. See [grammar-dsl].

## 25.15 CST-first grammar design

For a Rust code-intelligence consumer, ask of every rule:

```text
Will this create a useful node?
Should it be hidden?
Should a child have a field?
Should this be a supertype?
Will queries need to name this construct?
Will recovery around this rule preserve useful structure?
```

Grammar design is API design: every visible node/field becomes part of downstream extractor compatibility.

## 25.16 Anti-pattern inventory

* Translating a language specification's grammar literally without considering CST usability.
* Using rule order as a substitute for precedence.
* Confusing parse precedence with lexical precedence.
* Adding visible wrapper nodes that convey no useful structure.
* Using positional children when stable fields are available.
* Hiding a rule that downstream queries must distinguish without a better supertype/alias plan.
* Using complicated inline `extras` regexes that inflate generated lexer/parser code.

# Tree-sitter Advanced — 26) Precedence, ambiguity, and GLR conflicts

## 26.0 Why conflicts appear

Tree-sitter targets grammars close to LR(1), while using GLR machinery to support declared ambiguity. A highly natural CST grammar can be ambiguous even when the source language's interpretation is well-defined by precedence/context. See [grammar-writing].

Typical conflict:

```text
-a * b
  interpretation A: -(a * b)
  interpretation B: (-a) * b
```

## 26.1 Static precedence

```javascript
unary_expression: $ => prec(3, seq('-', $.expression)),

binary_expression: $ => choice(
  prec.left(2, seq($.expression, '*', $.expression)),
  prec.left(1, seq($.expression, '+', $.expression)),
),
```

Higher precedence binds more tightly when resolving eligible generation-time conflicts.

## 26.2 Associativity

```javascript
binary_expression: $ => choice(
  prec.left(2, seq($.expression, '*', $.expression)),
  prec.left(1, seq($.expression, '+', $.expression)),
  prec.right(0, seq($.expression, '=', $.expression)),
),
```

Use associativity when the ambiguity is *where the same precedence nests*:

```text
a + b + c
left  → (a + b) + c
right → a + (b + c)
```

## 26.3 Declared conflicts

When both interpretations are legitimately possible until more context is available:

```javascript
conflicts: $ => [
  [$.array, $.array_pattern],
],
```

This tells Tree-sitter to preserve GLR alternatives rather than demanding the generator pick one immediately.

## 26.4 Dynamic precedence

```javascript
pattern: $ => prec.dynamic(1, $.array_pattern),
```

Dynamic precedence only matters when runtime conflict branches are being compared. Do not use it as a first-line replacement for a structurally clearer grammar.

## 26.5 Read generator conflicts literally

`tree-sitter generate` reports an unresolved sequence with a marker at the ambiguity point and suggests possible fixes. Treat that output as a grammar proof obligation, not as noise to suppress.

Decision sequence:

```text
Is one parse actually wrong?
  → encode static precedence/associativity or refactor grammar

Are both parses semantically legitimate at that local point?
  → declare conflict
  → use context/dynamic precedence if one full parse should win
```

## 26.6 Refactor before adding conflicts

Excessive conflicts can increase runtime work and make recovery less predictable. Before adding one:

```text
remove pointless wrapper rule
factor common prefix
separate lexical ambiguity from parse ambiguity
use hidden helper rule
use explicit precedence
use field/alias without duplicating grammar branch
```

## 26.7 Conflict tests

Every declared conflict deserves corpus cases for:

```text
both intended interpretations
minimal ambiguous prefix
long/nested case
incomplete/error-recovery case
incremental edit crossing interpretation boundary
```

The final test category is especially important for code editors: a grammar that parses static examples correctly may still churn structure excessively under incremental edits.

## 26.8 Downstream impact

Conflict/precedence changes can alter:

```text
node nesting
which alternative node kind wins
ERROR placement
changed ranges after edits
query match counts
stable-node reuse
```

Therefore grammar conflict changes require extractor/query regression tests, not only `tree-sitter test` corpus updates.

## 26.9 Anti-pattern inventory

* Adding `conflicts` whenever generation fails without understanding ambiguity.
* Using huge arbitrary precedence numbers without a documented precedence table.
* Mixing lexical and parse conflicts.
* Updating precedence without snapshotting downstream query results.
* Assuming GLR ambiguity has zero runtime cost.
* Testing only complete valid source.

# Tree-sitter Advanced — 27) Lexing, extras, keywords, and token boundaries

## 27.0 Context-aware lexing

Tree-sitter lexes on demand in parser context: at a source position, it considers tokens valid in the current parse state. When several valid tokens match, the official grammar guide specifies an ordering involving lexical precedence, match length, string-vs-regex specificity, and finally grammar order. See [grammar-writing].

## 27.1 Lexical precedence sequence

Practical reasoning order:

```text
1. Which tokens are valid in the current parse context?
2. Highest explicit lexical precedence wins.
3. If tied, longest match wins.
4. If tied, string literal is preferred over regex.
5. If still tied, earlier grammar rule/token ordering breaks the tie.
```

Do not apply this ranking to parse rules; this is token recognition.

## 27.2 Keyword vs identifier

Naive grammar:

```javascript
identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,

if_statement: $ => seq(
  'if',
  $.expression,
  $.statement,
),
```

Without keyword extraction semantics, context-aware lexing can behave differently from conventional language lexers around prefixes such as `ifSomething`.

## 27.3 `word` token

```javascript
word: $ => $.identifier,
```

Tree-sitter can extract keyword tokens relative to this word token, improving both correctness around keyword boundaries and generated lexer performance. The word token must be suitable and unique as documented by the grammar guide. See [grammar-writing].

## 27.4 Extras

```javascript
extras: $ => [
  /[ \t\r\n]/,
  $.comment,
],
```

Extras can appear between ordinary tokens without being mentioned in every rule. Typical use: whitespace/comments.

Prefer named rules for complex extras:

```javascript
extras: $ => [/[ \t\r\n]/, $.comment],

rules: {
  comment: $ => token(choice(
    seq('//', /.*/),
    seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/'),
  )),
}
```

The official guide warns that inlining complex extra patterns broadly can inflate generated parser size. See [grammar-writing].

## 27.5 Immediate tokens

```javascript
attribute: $ => seq(
  '@',
  token.immediate(/[a-zA-Z_][a-zA-Z0-9_]*/),
),
```

Use immediate tokens when whitespace/extras are forbidden at that exact boundary. Do not use them merely to “fix” a precedence problem.

## 27.6 Token vs node visibility

String literals commonly become anonymous nodes; named token rules become named nodes unless hidden/aliased. This matters for Rust traversal:

```text
node.children(...)       → includes named and anonymous children
node.named_children(...) → excludes anonymous punctuation/keyword literals
```

A grammar's lexical design therefore directly changes how consumers should traverse.

## 27.7 Lexical regression corpus

Include tests for:

```text
keyword + identifier prefix collisions
longest-match operators: > vs >=, . vs .. vs ...
adjacent tokens without whitespace
comments between tokens
Unicode identifiers if language supports them
unterminated string/comment tokens
incremental edits at token boundaries
```

## 27.8 Anti-pattern inventory

* Using parse precedence to solve a token collision.
* Using token precedence to solve grammar nesting.
* Forgetting longest-match interactions between operators.
* Treating grammar order as the primary disambiguation mechanism.
* Defining contextual keywords as globally reserved without language justification.
* Marking whitespace-sensitive syntax with generic extras and then compensating in queries.

# Tree-sitter Advanced — 28) CST design: hidden rules, fields, supertypes, aliases, and inline rules

## 28.0 Grammar shape is downstream API shape

A good Tree-sitter grammar optimizes for an intuitive concrete syntax tree, not a literal transcription of a language standard's production hierarchy. The official guide explicitly recommends avoiding layers of indirection that add no useful syntax-tree information. See [grammar-writing].

## 28.1 Hidden rules

Rules whose names begin with `_` are hidden from the visible syntax tree.

```javascript
_expression: $ => choice(
  $.identifier,
  $.call_expression,
  $.binary_expression,
),
```

Use hidden helper categories to flatten the CST while retaining grammar composition.

## 28.2 `inline`

```javascript
inline: $ => [
  $._simple_type,
],
```

`inline` substitutes a rule definition at use sites, avoiding a runtime symbol/node for that helper. This is stronger than merely hiding a node and can affect parser generation. Use sparingly and benchmark grammar generation/parser size if heavily applied.

## 28.3 Fields

Fields are the strongest stable child API:

```javascript
call_expression: $ => seq(
  field('function', $.expression),
  '(',
  field('arguments', optional($.argument_list)),
  ')',
),
```

Rust:

```rust
let function = call.child_by_field_name("function");
let arguments = call.child_by_field_name("arguments");
```

Prefer fields to `child(0)`, `child(1)` whenever a semantic role exists.

## 28.4 Supertypes

```javascript
supertypes: $ => [
  $.expression,
  $.declaration,
],
```

Supertypes model abstract categories without forcing an extra visible node in every tree. They appear in static node-type metadata and can be targeted by queries.

This aligns well with Rust extractor abstractions:

```text
Expression
  ├─ Identifier
  ├─ CallExpression
  ├─ BinaryExpression
  └─ ...
```

## 28.5 Aliases

```javascript
shorthand_property: $ => alias($.identifier, $.property_identifier),
```

Aliases allow the same underlying rule to appear under a semantically more specific node kind. This can make downstream queries dramatically clearer, but alias changes are query-breaking CST API changes.

## 28.6 Named vs anonymous design

Use named nodes when downstream consumers need to:

```text
query the construct by semantic role
attach diagnostics/semantic facts
navigate it while ignoring punctuation
expose it in node-types schema
```

Use anonymous literals for syntax punctuation/keywords when a separate semantic node adds no value.

## 28.7 Field cardinality and node-types schema

Fields propagate to `node-types.json`, including allowed types, `required`, and `multiple` metadata. Therefore fields are simultaneously:

```text
grammar readability feature
runtime Node traversal API
static schema/code-generation API
query constraint API
```

## 28.8 CST compatibility policy

Treat these as semver-relevant for a published grammar package:

```text
renaming visible node kinds
named ↔ anonymous changes
renaming/removing fields
changing supertype membership
changing alias targets
major child nesting changes
```

Even when source-language coverage improves, downstream Rust queries can break.

## 28.9 Consumer-driven grammar review

For every important construct, inspect:

```bash
tree-sitter parse example.lang
```

Then ask:

```text
Can I locate declaration name by field?
Can I find body without child-index assumptions?
Can queries target the abstract category?
Are punctuation wrappers creating noise?
Does incomplete code still expose useful structure?
```

## 28.10 Anti-pattern inventory

* Exposing language-spec precedence layers as dozens of useless visible nodes.
* Hiding all abstract categories without supertypes when queries need them.
* Omitting fields for core semantic roles.
* Over-aliasing until grammar node identity becomes opaque.
* Treating CST shape as internal when publishing a grammar crate used by extractors.

# Tree-sitter Advanced — 29) External scanners

## 29.0 Purpose

External scanners handle tokens whose recognition requires state or logic inconvenient/impossible to express with ordinary grammar regex/token rules, such as indentation/dedentation, heredocs, and language-specific complex string delimiters. They are handwritten native code called by Tree-sitter's lexer. See [external-scanners].

## 29.1 Declare external tokens

```javascript
export default grammar({
  name: 'mini_indent',

  externals: $ => [
    $.indent,
    $.dedent,
    $.newline,
  ],

  rules: {
    // use $.indent / $.dedent / $.newline in grammar rules
  },
});
```

## 29.2 Required scanner functions

For a C scanner, Tree-sitter expects grammar-specific functions conceptually equivalent to:

```c
void *tree_sitter_my_language_external_scanner_create(void);
void tree_sitter_my_language_external_scanner_destroy(void *payload);
unsigned tree_sitter_my_language_external_scanner_serialize(
    void *payload,
    char *buffer
);
void tree_sitter_my_language_external_scanner_deserialize(
    void *payload,
    const char *buffer,
    unsigned length
);
bool tree_sitter_my_language_external_scanner_scan(
    void *payload,
    TSLexer *lexer,
    const bool *valid_symbols
);
```

Use the exact generated/current header contract when implementing a real scanner.

## 29.3 `valid_symbols` is a hard contract

At each scanner invocation, `valid_symbols[i]` tells the scanner whether the corresponding external token is valid in the current parse state.

```c
if (valid_symbols[INDENT]) {
    // attempt indentation recognition
}
```

Do not emit a token the parser has not marked valid. This can corrupt parse behavior and dramatically increase false branches.

## 29.4 Scanner cannot freely backtrack

External scanner logic must respect Tree-sitter's lexer contract. The official guide warns that scanning decisions and advancement are constrained; design state machines that only commit after valid recognition according to lexer APIs. See [external-scanners].

## 29.5 Serialization is incremental-parsing state

If the scanner holds state that affects future tokenization, it must serialize enough state for Tree-sitter to reuse old subtrees safely.

Examples:

```text
indent stack
active heredoc delimiter
nested lexical mode
raw-string delimiter length
```

Serialization buffer size is bounded by `TREE_SITTER_SERIALIZATION_BUFFER_SIZE`; keep state compact and deterministic. See [external-scanners].

## 29.6 State design

Good scanner state:

```text
small
plain-data-like
fully serializable
deterministic from consumed source
no process-global mutable dependencies
```

Bad scanner state:

```text
open file handles
heap graph impossible to serialize
source pointers into transient buffers
thread-local hidden mode
large arbitrary history
```

## 29.7 Allocation

Tree-sitter exposes allocation macros/helpers for external scanners. The official scanner docs discuss using Tree-sitter allocator macros and a `TREE_SITTER_REUSE_ALLOCATOR` mechanism, including caveats for dynamic linking. See [external-scanners].

Prefer fixed-size/simple scanner state when practical; allocation complexity becomes incremental-state complexity.

## 29.8 C vs C++ scanner

A grammar can ship `scanner.c` or `scanner.cc` according to need. The Rust grammar `build.rs` must compile it with compatible flags and C/C++ mode. Avoid introducing C++ runtime complexity merely for convenience in a tiny scanner.

## 29.9 Scanner test matrix

```text
initial state
all external token branches
nested scanner state
serialization/deserialization round-trip
incremental edit before token
incremental edit inside token
incremental edit after token
unterminated construct
EOF behavior
very long delimiter/indent sequence
```

## 29.10 Fuzzing scanners

External scanner code is the most obvious handwritten native-memory-safety surface in a grammar package. Fuzz malformed/untrusted source aggressively, and compile scanner code with sanitizers in dedicated CI when feasible.

## 29.11 Anti-pattern inventory

* Using an external scanner for a token ordinary regex/token rules can express cleanly.
* Ignoring `valid_symbols`.
* Keeping state but implementing empty/incorrect serialize/deserialize.
* Storing pointers into parser input across calls.
* Assuming scanner calls always occur in one obvious order.
* Allocating unbounded memory from source-controlled delimiters.
* Shipping handwritten scanner code without malformed-input fuzzing.

# Tree-sitter Advanced — 30) Grammar generation, corpus tests, and Rust binding CI

## 30.0 Generation workflow

```bash
tree-sitter generate
```

The CLI reads the grammar, resolves/generates parser tables, and writes native parser artifacts including `src/parser.c`, `src/grammar.json`, and `src/node-types.json`. Unresolved grammar conflicts fail generation with actionable diagnostics. See [grammar-generate].

## 30.1 Generated artifacts

Current documented generation outputs include:

```text
src/parser.c
src/tree_sitter/parser.h
src/tree_sitter/alloc.h
src/tree_sitter/array.h
src/grammar.json
src/node-types.json
```

External scanner source is handwritten and lives alongside these artifacts.

## 30.2 ABI generation

The CLI's `generate` command supports selecting a parser ABI. Normally use the current/default ABI unless you intentionally need compatibility with older Tree-sitter runtimes. Then verify that the generated grammar ABI falls within the Rust runtime's `MIN_COMPATIBLE_LANGUAGE_VERSION..=LANGUAGE_VERSION` range.

## 30.3 Corpus tests

Grammar tests live under `test/corpus/` and pair source text with expected S-expression trees:

```text
==================
Return statements
==================

func x() int {
  return 1;
}

---

(source_file
  (function_definition
    ...))
```

Run:

```bash
tree-sitter test
```

See [grammar-tests].

## 30.4 Target one corpus test

```bash
tree-sitter test -i 'Return statements'
```

Use focused iteration while authoring a rule, but always run the full corpus before commit.

## 30.5 Update expected trees intentionally

```bash
tree-sitter test -u
```

This is useful after an intentional CST migration, but it is dangerous as a blind “make tests pass” command. Review the resulting tree diffs line-by-line because downstream query compatibility may have changed.

## 30.6 Test attributes

The current corpus system supports attributes for cases such as CST behavior, expected errors/fail-fast, language selection, platform, and skipped tests. Use these intentionally for multi-language or platform-specific grammar behavior. See [grammar-tests].

## 30.7 Rust binding tests

Add application-facing tests in addition to CLI corpus tests:

```rust
#[test]
fn rust_binding_parses() {
    let language: tree_sitter::Language = tree_sitter_my_language::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).unwrap();

    let tree = parser.parse("valid fixture", None).unwrap();
    assert!(!tree.root_node().has_error());
}
```

Also test query constants:

```rust
#[test]
fn shipped_queries_compile() {
    let language: tree_sitter::Language = tree_sitter_my_language::LANGUAGE.into();
    tree_sitter::Query::new(&language, tree_sitter_my_language::HIGHLIGHTS_QUERY).unwrap();
}
```

Use only constants actually exposed by the grammar crate.

## 30.8 Incremental correctness tests

Static corpus parsing does not prove incremental correctness. Add a Rust harness:

```text
parse source A → old_tree
apply exact InputEdit to old_tree
parse source B with old_tree
parse source B from scratch
compare resulting S-expression / structural invariants
```

The incremental and fresh parses should describe the same final source structure.

## 30.9 Query golden tests

For each shipped/extractor query, snapshot:

```text
capture name
captured kind
byte range
selected text
pattern index when meaningful
```

This catches CST changes that still satisfy corpus tree snapshots but subtly alter query semantics.

## 30.10 Error-recovery tests

For every important construct, test incomplete forms:

```text
missing closing delimiter
missing identifier
half-written operator
unterminated string/comment
extra token
newline inserted at sensitive boundary
```

Editor-quality grammars are judged heavily by useful trees during invalid intermediate states.

## 30.11 CI pipeline

```text
tree-sitter generate --check/controlled regeneration policy
  → tree-sitter test
  → cargo test for Rust binding
  → query compilation tests
  → incremental fresh-vs-reparse equivalence tests
  → node-types/schema diff gate
  → sanitizer/fuzz suite for external scanner where applicable
  → packaged-crate smoke test
```

The exact CLI flags should be pinned to the CLI version used by the repository.

## 30.12 Generation diagnostics

Use generation logging when investigating parser states, conflict resolution, keyword extraction, and error recovery. The `generate` command documents a logging option that exposes parser-generation details. See [grammar-generate].

## 30.13 Migration review checklist

```text
[ ] Did parser.c change because grammar changed or generator changed?
[ ] Did node-types.json change?
[ ] Did any visible kind/field/supertype change?
[ ] Did shipped query files still compile?
[ ] Did query capture goldens change?
[ ] Did ERROR/MISSING recovery shapes change?
[ ] Did incremental changed ranges grow unexpectedly?
[ ] Did parser ABI change?
[ ] Did Rust grammar binding/package metadata change?
```

## 30.14 Anti-pattern inventory

* Running only `cargo test` and never `tree-sitter test` for a grammar project.
* Running only corpus tests and never testing the Rust binding.
* Blindly accepting `tree-sitter test -u` output.
* Never comparing incremental parse with fresh parse.
* Updating grammar and query bundle independently.
* Ignoring node-types schema diffs.
* Publishing a crate from repository state that differs from the packaged files.

# Tree-sitter Advanced — 31) Syntax highlighting in native Rust

## 31.0 Crate boundary

`tree-sitter-highlight` is a higher-level Rust crate built on the core parser/query machinery. Current docs.rs is aligned at 0.26.12. It standardizes highlight queries, locals behavior, language injections, and event streaming. See [highlight].

Use raw `Query` when you need arbitrary semantic extraction; use `tree-sitter-highlight` when you want the established Tree-sitter highlighting conventions.

## 31.1 Dependencies

```toml
[dependencies]
tree-sitter = "0.26.12"
tree-sitter-highlight = "0.26.12"
tree-sitter-javascript = "0.25"
```

Pin grammar versions independently. The grammar's query constants are version-coupled to that grammar's CST.

## 31.2 `Highlighter` lifecycle

```rust
use tree_sitter_highlight::Highlighter;

let mut highlighter = Highlighter::new();
```

Official crate guidance requires one `Highlighter` per thread used for highlighting. Prefer thread-local/pool ownership rather than locking one mutable instance globally. See [highlight].

## 31.3 Highlight names are application policy

```rust
let highlight_names = [
    "attribute",
    "comment",
    "constant",
    "function",
    "keyword",
    "number",
    "operator",
    "property",
    "string",
    "type",
    "variable",
    "variable.parameter",
];
```

The query produces logical highlight names; your renderer/theme maps them to visual styles. Keep theme semantics outside grammar queries.

## 31.4 `HighlightConfiguration`

```rust
use tree_sitter_highlight::HighlightConfiguration;

let language: tree_sitter::Language = tree_sitter_javascript::LANGUAGE.into();

let mut config = HighlightConfiguration::new(
    language,
    "javascript",
    tree_sitter_javascript::HIGHLIGHT_QUERY,
    tree_sitter_javascript::INJECTIONS_QUERY,
    tree_sitter_javascript::LOCALS_QUERY,
)?;

config.configure(&highlight_names);
```

Use the exact query constants exported by the grammar package. Some grammars expose different names or omit a query category.

## 31.5 Event stream

```rust
use tree_sitter_highlight::HighlightEvent;

let events = highlighter.highlight(
    &config,
    b"const x = new Y();",
    None,
    |_| None,
)?;

for event in events {
    match event? {
        HighlightEvent::Source { start, end } => {
            // emit source[start..end]
        }
        HighlightEvent::HighlightStart(highlight) => {
            // push style
        }
        HighlightEvent::HighlightEnd => {
            // pop style
        }
    }
}
```

This event model avoids constructing a second fully copied highlighted string unless the renderer requires one.

## 31.6 Injections

The final callback supplied to `highlight` resolves embedded language names to highlight configurations. This is the high-level counterpart to manually managing included ranges.

```text
highlight query identifies injection
  → injection callback resolves language/config
  → highlighter parses/highlights embedded region
  → events merged into host stream
```

Use a registry keyed by canonical language names and grammar fingerprints; avoid network/disk loading inside the callback hot path.

## 31.7 Locals query

Locals queries let the highlighter reason about local definitions/references so a capture such as an identifier can be highlighted according to local binding context. This is still syntax/query-driven—not a full type checker.

Keep locals queries versioned with the grammar and compile them at startup.

## 31.8 Byte ranges and rendering

Highlight events use source byte ranges. If your UI uses UTF-16 code units or grapheme indices, maintain a source-coordinate conversion layer. Do not reinterpret Tree-sitter byte offsets as character indices.

## 31.9 Incremental highlighting

The high-level crate can be used repeatedly, but a custom editor pipeline may get better control by maintaining parse trees and restricting raw highlight queries to changed structural ranges. Decide based on workload:

```text
small files / simple UI → re-highlight whole file
large files / high keystroke rate → incremental parse + changed-range re-query + span merge
```

Do not claim an incremental renderer is correct unless it handles highlight scopes that can extend beyond the edit (e.g., multiline strings, changed declaration/local context).

## 31.10 Highlight configuration cache

```text
key = grammar fingerprint + highlight query hash + locals hash + injection hash + highlight-name registry version
value = HighlightConfiguration
```

The configuration is grammar/query derived; invalidate it when any component changes.

## 31.11 Anti-pattern inventory

* One mutexed `Highlighter` for an entire high-concurrency service.
* Treating highlight names as hard-coded colors in grammar queries.
* Using stale queries after a grammar upgrade.
* Slicing a Rust `str` directly with arbitrary byte offsets without validating UTF-8 boundaries.
* Performing grammar loading in injection callbacks.
* Assuming syntax highlighting provides compiler-grade symbol resolution.

# Tree-sitter Advanced — 32) Tags and code-navigation extraction

## 32.0 What `tree-sitter-tags` provides

`tree-sitter-tags` is a higher-level Rust library for computing code-navigation tags from Tree-sitter queries. Current docs.rs is 0.26.12. It is useful for definition/reference-like navigation surfaces, but it is intentionally narrower than a full language server/compiler index. See [tags].

## 32.1 `TagsContext` lifecycle

```rust
use tree_sitter_tags::TagsContext;

let mut context = TagsContext::new();
```

Official guidance uses one tags context per thread performing tag computation. Pool/thread-local it similarly to parsers/query cursors.

## 32.2 `TagsConfiguration`

Conceptual modern usage:

```rust
use tree_sitter_tags::TagsConfiguration;

let language: tree_sitter::Language = tree_sitter_javascript::LANGUAGE.into();

let config = TagsConfiguration::new(
    language,
    tree_sitter_javascript::TAGS_QUERY,
    tree_sitter_javascript::LOCALS_QUERY,
)?;
```

Check the exact current constructor/signature for the grammar/crate versions you pin. Grammar constants vary.

## 32.3 Generated tag record

The current public `Tag` record exposes navigation-oriented information directly rather than a durable string `kind` field:

```text
range: Range<usize>
name_range: Range<usize>
line_range: Range<usize>
span: Range<Point>
utf16_column_range: Range<usize>
docs: Option<String>
is_definition: bool
syntax_type_id: u32
```

`range` covers the tagged construct, `name_range` isolates the symbol name, `line_range` / `span` / `utf16_column_range` provide presentation-oriented coordinates, `is_definition` distinguishes definition-like from reference-like captures, and `syntax_type_id` indexes the syntax type configured for the tagging query. Documentation may be attached when the query configuration supports it. Treat `syntax_type_id` as configuration-local metadata rather than a stable cross-version semantic enum. See [tags].

## 32.4 Definition-oriented query model

A tagging query usually distinguishes definitions from references and categorizes syntax types. Convert that crate-level representation into an application-owned semantic schema at the boundary:

```rust
#[derive(Debug, Clone)]
struct SymbolTag {
    semantic_kind: String,
    is_definition: bool,
    full_range: std::ops::Range<usize>,
    name_range: std::ops::Range<usize>,
    name: String,
    documentation: Option<String>,
    revision: u64,
}
```

Resolve `syntax_type_id` through the pinned `TagsConfiguration` / application mapping when producing `semantic_kind`. Do not expose configuration-local numeric syntax IDs directly as durable API fields.

## 32.5 Tags vs custom query extraction

Use `tree-sitter-tags` when:

```text
grammar ships a maintained tags query
generic editor navigation is enough
you want standardized Tree-sitter tag conventions
```

Use custom query/index logic when:

```text
you need call edges / import resolution / type relationships
language-specific scope rules matter
references need semantic disambiguation
you need richer metadata than tag kind/name/docs
```

## 32.6 Hybrid pattern

```text
Tree-sitter tags
  → fast candidate definitions
custom scope/reference queries
  → syntactic candidate references
compiler/LSP/semantic analyzer
  → resolution / disambiguation
code property graph
  → durable edges + cross-file relationships
```

This lets Tree-sitter maximize coverage and responsiveness without overstating syntax-only certainty.

## 32.7 Incremental tag invalidation

Treat tag records as owned by structural regions:

```text
changed_ranges(old,new)
  → find overlapping declaration/scope roots
  → delete tags owned by those roots
  → rerun tags/custom queries only on invalidated roots
  → preserve unaffected tags
```

If a language's tagging query depends on enclosing locals/scope, widen invalidation to the nearest scope root.

## 32.8 Anti-pattern inventory

* Calling Tree-sitter tags a complete semantic index.
* Persisting tag numeric IDs without version schema.
* Recomputing repository-wide tags after every edit.
* Failing to widen invalidation for changed scope/local context.
* Assuming every grammar ships a usable tags query.

# Tree-sitter Advanced — 33) WASM grammar loading from Rust

## 33.0 When WASM grammars are useful

Native grammar crates are simplest and fastest for a fixed language set. WASM grammar support is useful when grammar binaries need to be loaded dynamically, sandboxed/portable across supported host environments, or distributed without native shared-library loading.

The core `tree-sitter` Rust crate exposes WASM support behind the `wasm` feature. See [wasm-store] and [rustdoc].

## 33.1 Feature flag

```toml
[dependencies]
tree-sitter = { version = "0.26.12", features = ["wasm"] }
```

The WASM feature adds the runtime integration; it is not required for ordinary statically linked grammar crates.

## 33.2 Create engine and store

The crate re-exports the required Wasmtime engine surface for its WASM language integration.

```rust
use tree_sitter::{wasmtime::Engine, WasmStore};

let engine = Engine::default();
let mut store = WasmStore::new(&engine)?;
```

## 33.3 Load language bytes

```rust
let wasm_bytes = std::fs::read("tree-sitter-my-language.wasm")?;
let language = store.load_language("my_language", &wasm_bytes)?;
```

`WasmStore::load_language(name, bytes)` returns a normal `Language` handle. The resulting language is identified as WASM by `Language::is_wasm()`. See [wasm-store].

## 33.4 Attach WASM store to parser

```rust
let mut parser = tree_sitter::Parser::new();
parser.set_wasm_store(store)?;
parser.set_language(&language)?;

let tree = parser.parse(source, None).unwrap();
```

A parser using a WASM language must have a WASM store assigned. The language itself can be used with a compatible store; it does not have to be the exact instance that originally loaded the bytes according to the low-level API contract. See [wasm-ffi].

## 33.5 Store ownership

Parser methods include:

```text
set_wasm_store(store)
take_wasm_store()
```

Model store ownership explicitly in worker pools so a parser is never checked back into a native-only pool while retaining WASM-specific state unexpectedly.

## 33.6 Language-count diagnostics

```rust
println!("loaded wasm languages={}", store.language_count());
```

Use this as resource telemetry; do not dynamically load unbounded distinct grammars without quotas.

## 33.7 Native vs WASM policy

| Requirement | Prefer native grammar crate | Prefer WASM grammar |
|---|---:|---:|
| fixed trusted language set | yes | usually no |
| simplest Rust build | yes | no |
| dynamic third-party grammar upload | risky native | potentially useful with broader sandbox controls |
| maximum parser throughput | benchmark; generally native baseline | benchmark overhead |
| cross-platform grammar artifact | build per target | WASM can simplify distribution |
| no dynamic native code loading | yes if statically compiled | yes |

WASM parsing is not automatically a complete security sandbox: validate resources, deadlines, query limits, host/runtime configuration, and grammar provenance.

## 33.8 Cache WASM languages

```text
key = hash(wasm bytes) + Tree-sitter runtime version + WASM engine config
value = validated Language / store registry entry
```

Avoid compiling/loading the same WASM grammar for each file parse.

## 33.9 Upgrade tests

For every WASM grammar:

```text
load bytes
verify ABI
set on parser with store
parse smoke corpus
compile query bundle
run incremental parse test
measure load latency + parse latency
```

## 33.10 Anti-pattern inventory

* Enabling `wasm` when the language set is fixed and no dynamic loading is needed.
* Loading a WASM grammar on every request.
* Forgetting `Parser::set_wasm_store` before setting/using a WASM language.
* Assuming WASM removes the need for parse/query time and memory budgets.
* Accepting arbitrary grammar bytes without provenance/size/resource policy.

# Tree-sitter Advanced — 34) Raw FFI interop and ownership boundaries

## 34.0 Stay in safe Rust unless interop requires raw handles

The public Rust API wraps the C Tree-sitter runtime and exposes a `ffi` module plus `from_raw` / `into_raw` escape hatches for interoperability. Use these only at a clearly documented boundary. See [parser] and [ffi-source].

## 34.1 Parser raw handle

```rust
let parser = tree_sitter::Parser::new();
let raw = parser.into_raw();

// ownership is now outside the Rust wrapper

let parser = unsafe { tree_sitter::Parser::from_raw(raw) };
```

`from_raw` requires a non-null pointer and assumes the caller is transferring a valid Tree-sitter parser. `into_raw` consumes the Rust wrapper.

## 34.2 Language raw handle

`Language` similarly exposes raw conversion methods. Grammar `LanguageFn` conversion should remain the preferred normal path; raw language pointers are for FFI/plugin bridges that already speak the C ABI.

## 34.3 Ownership matrix

```text
Rust wrapper owns native object
  → Drop releases it

into_raw(self)
  → Rust wrapper consumed
  → caller owns native pointer lifecycle

from_raw(ptr)
  → caller transfers ownership/validity assumptions into Rust wrapper
```

Never keep two independently owning wrappers around the same raw parser/query/cursor pointer.

## 34.4 Borrowed node semantics

`Node<'tree>` is not an owning heap allocation in the same sense as `Tree`; its lifetime is tied to the tree in safe Rust. This prevents a class of C-API bugs where a node handle outlives its tree.

Do not erase that lifetime via raw FFI and then store node handles after freeing/replacing the tree.

## 34.5 Callback lifetime hazards

Parser loggers, parse callbacks, progress callbacks, query text providers, and similar closures may bridge into C calls. Keep closure state alive for the duration of the call/configuration according to Rust API lifetime requirements; do not smuggle stack pointers through custom FFI callbacks.

## 34.6 FFI panic rule

Never let a Rust panic unwind across a C ABI boundary. If you provide custom extern callbacks:

```text
validate inputs
avoid panics
catch_unwind at appropriate outer Rust boundary if necessary
return an explicit failure/sentinel according to C contract
```

## 34.7 ABI plugin loading

If dynamically loading native grammar libraries:

```text
load symbol tree_sitter_<language>
obtain TSLanguage pointer
wrap/convert with correct ownership semantics
validate ABI before parser use
keep shared library loaded as long as language/function code may be invoked
```

Unloading a dynamic library while a `Language`, `Parser`, tree operation, or external scanner can call into its generated code is unsafe.

## 34.8 Raw-pointer audit checklist

```text
Who allocates?
Who owns?
Who frees?
Can object be cloned/copy-refcounted instead?
What is the C ABI version?
Does a callback pointer outlive its environment?
Can a shared library unload first?
Can a panic cross the boundary?
```

## 34.9 Anti-pattern inventory

* Using `ffi::*` to avoid learning the safe Rust API.
* Calling `from_raw` on borrowed/non-owned pointers.
* Calling `into_raw` and then assuming Rust `Drop` will still clean up.
* Holding raw `TSNode` values after freeing their tree.
* Unloading a native grammar shared library while its language is still live.
* Letting panics unwind across `extern "C"` boundaries.

# Tree-sitter Advanced — 35) Memory allocation and native resource behavior

## 35.0 Core allocation model

The Rust crate wraps a native C core; trees share structure aggressively, and incremental reparsing can reuse old subtrees. This makes tree copies and old-tree reuse efficient but does not mean the parser is allocation-free.

For most applications, use the default allocator and focus optimization on document/tree lifetime, parse/query scope, and cache size.

## 35.1 Global allocator override

The Rust crate exposes:

```rust
pub unsafe fn set_allocator(
    new_malloc: Option<unsafe extern "C" fn(usize) -> *mut std::ffi::c_void>,
    new_calloc: Option<unsafe extern "C" fn(usize, usize) -> *mut std::ffi::c_void>,
    new_realloc: Option<unsafe extern "C" fn(*mut std::ffi::c_void, usize) -> *mut std::ffi::c_void>,
    new_free: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>,
)
```

This is unsafe because it crosses FFI and mutates global allocator state. See [allocator].

## 35.2 Why changing allocators late is dangerous

The underlying C API documents a crucial invariant: if the allocator changes after Tree-sitter has already allocated objects, either all existing objects must have been freed or the new allocator must be able to free memory allocated by the old one. See [allocator-ffi].

Therefore:

```text
best time to set custom allocator = process initialization before any Tree-sitter object exists
```

## 35.3 Default allocation failure behavior

The low-level API documentation states that the default libc allocation path aborts the process on allocation failure. A service that needs stronger memory isolation should therefore combine process/container memory limits, file-size limits, parse budgets, and possibly worker isolation rather than assuming ordinary Rust `Result` propagation for every OOM.

## 35.4 Tree lifetime and cache pressure

A tree retains native syntax structure. A repository service can accidentally retain many revisions:

```text
document current tree
+ undo-history trees
+ query cache nodes/trees
+ background job snapshots
+ per-request clones
```

Cheap structural sharing is not zero memory. Bound retained revisions and release old trees promptly.

## 35.5 Avoid storing `Node` forests

Prefer stable product records:

```rust
struct NodeKey {
    start_byte: usize,
    end_byte: usize,
    kind_id: u16,
    revision: u64,
}
```

or your own semantic IDs over retaining millions of `Node` values tied to trees. Re-fetch nodes from current trees for transient operations.

## 35.6 Query memory controls

Query execution has its own state growth controls such as `QueryCursor::set_match_limit`; use range/depth constraints and cancellation callbacks in addition. A file that parses cheaply can still produce pathological query work from highly repetitive patterns.

## 35.7 Worker memory telemetry

Track at least:

```text
source bytes retained
number of live current trees
number of retained historical trees
query configuration count
parser/cursor pool size
WASM language/store count
index record count
process RSS / allocator metrics
```

Tree-sitter does not expose all native allocation detail through one high-level Rust counter, so process-level telemetry remains important.

## 35.8 Anti-pattern inventory

* Calling `set_allocator` after parsers/trees exist without cross-allocator compatibility.
* Retaining every tree revision indefinitely because copies are “cheap.”
* Keeping `Node` values as durable index records.
* Assuming Rust's global allocator automatically controls the C library's allocations.
* Relying on query match limits as a complete memory sandbox.

# Tree-sitter Advanced — 36) Concurrency, thread safety, and pooling

## 36.0 Two layers of truth

Rust wrappers expose `Send`/`Sync` implementations for several types, but the core Tree-sitter concurrency guidance still matters: individual underlying `TSTree` instances should be copied when used independently across threads. Tree copies are cheap because they increment an internal atomic reference count. See [advanced-parsing].

## 36.1 Operational default

```text
Parser      → worker-owned mutable object
QueryCursor → worker-owned mutable object
Highlighter → one per worker/thread
TagsContext → one per worker/thread
Language    → shared immutable registry object
Query       → shared immutable compiled query
Tree        → share/clone according to safe Rust + mutation policy; isolate edits by revision
```

Even when a type is technically `Sync`, exclusive worker ownership often produces simpler semantics and less contention.

## 36.2 Parser pool

```rust
struct ParserWorker {
    parser: tree_sitter::Parser,
    cursor: tree_sitter::QueryCursor,
}
```

Pool by language or set language on checkout according to a strict reset contract.

Worker checkout:

```text
verify language
set included ranges
install request-local budget/progress state
parse
run queries with cursor
clear transient state
return worker
```

## 36.3 One parser per concurrent parse

A `Parser` is stateful and mutable. Do not wrap one global parser in a mutex unless parse concurrency is intentionally serialized. Parser construction is cheap enough in many workloads that per-worker instances are cleaner; benchmark pool size rather than assuming one global object is optimal.

## 36.4 Query reuse

`Query` is compiled/immutable after construction and is designed to be reusable/shareable. `QueryCursor` carries execution state and should be worker-local/reused sequentially.

```rust
use std::sync::Arc;

struct GrammarQueries {
    symbols: Arc<tree_sitter::Query>,
    references: Arc<tree_sitter::Query>,
}
```

## 36.5 Tree revision ownership

```rust
struct DocumentSnapshot {
    revision: u64,
    source: std::sync::Arc<[u8]>,
    tree: tree_sitter::Tree,
}
```

Never mutate/edit the tree associated with an immutable published snapshot. Clone/create the next revision's tree and apply edits there according to your concurrency design.

## 36.6 Concurrent readers vs editor

Recommended actor/snapshot model:

```text
Document actor
  owns current mutable revision pipeline
  edit → incremental parse → publish immutable snapshot

Readers
  Arc<snapshot>
  run read-only queries/index work
  discard when finished
```

This avoids “source revision N+1 + tree revision N” races.

## 36.7 Result commit race

Every background result must carry source revision:

```rust
struct IndexedResult<T> {
    revision: u64,
    payload: T,
}
```

Before committing:

```text
if result.revision != document.current_revision:
    discard or merge only under explicit stale-result policy
```

## 36.8 Grammar registry synchronization

Build an immutable generation of registry/configuration:

```text
RegistryGeneration 42
  language handles
  compiled query bundles
  node/field metadata
  grammar fingerprints
```

Hot-upgrade by atomically publishing a new registry generation; let in-flight requests finish on the old generation instead of mutating queries/languages underneath them.

## 36.9 Pool sizing

Bound worker count by actual CPU/concurrency goals. Tree-sitter parsing is CPU work; an unbounded task fan-out on many files can reduce throughput through cache pressure/context switching even when memory permits it.

## 36.10 Anti-pattern inventory

* One global mutexed parser for all files/languages.
* Sharing mutable document source and tree without revision snapshots.
* Editing a tree while another task treats it as immutable current state.
* Recompiling identical queries in every worker.
* Committing background results without revision check.
* Unbounded parser workers based solely on queue length.

# Tree-sitter Advanced — 37) Performance engineering and benchmarking

## 37.0 Measure the entire pipeline

Separate performance stages:

```text
source acquisition
edit-coordinate calculation
incremental parse
changed-range computation
query execution
capture materialization/source slicing
semantic transform
index write
```

A fast parser can be hidden by an expensive broad query or index merge.

## 37.1 Core metrics

Per document/revision:

```text
source_bytes
parse_mode = fresh | incremental
parse_us
changed_range_count
changed_bytes_total
reused/unchanged-region proxy metrics
query_us per query bundle
match_count / capture_count
match_limit_exceeded
cancelled flag
ERROR/MISSING count
index records added/removed/retained
```

## 37.2 Incremental speedup benchmark

Benchmark both:

```text
T_fresh(new_source)
T_edit = edit(old_tree) + incremental_parse(new_source, old_tree)
```

Across edit classes:

```text
single identifier character
newline insertion
opening delimiter
closing delimiter
large paste
large deletion
edit near top of file
edit near EOF
edit inside recovery/error region
```

Incremental performance is distribution-dependent; average “one random byte edit” is not enough.

## 37.3 Query benchmark dimensions

```text
whole root vs changed scope
byte range restriction
point range restriction
max start depth
capture iteration vs match iteration
text predicates vs structural-only query
capture cardinality
match_limit pressure
```

Use query microbenchmarks only after validating semantic equivalence of the restricted query scope.

## 37.4 Tree traversal benchmark

Compare:

```text
recursive child(i) with repeated child lookup
TreeCursor DFS
named-only traversal
query-based structural selection
```

For large scans, `TreeCursor` generally avoids repeated node-child lookup overhead and gives field context cheaply.

## 37.5 Avoid source copying

Prefer source slices/Arc-backed buffers and pass references/callback chunks into Tree-sitter. Do not allocate a new `String` for every captured node.

Bad:

```rust
let text = source[node.byte_range()].to_vec();
```

Preferred when lifetime permits:

```rust
let text = &source[node.byte_range()];
```

Materialize owned strings only at a persistent API/index boundary.

## 37.6 Precompile queries

```text
startup:
  Query::new(language, source) once

hot path:
  reuse Query
  reset/reuse QueryCursor
```

Query compilation includes pattern parsing/validation and should not be repeated per file.

## 37.7 Range-local extraction

For continuously updating indexes:

```text
changed_ranges
  → expand to semantic owner scopes
  → query only those roots/ranges
  → replace owner-scoped records
```

This typically scales better and is easier to reason about than rerunning every query over the full root after every keystroke.

## 37.8 Grammar performance

Grammar changes can affect runtime through:

```text
parser state count
generated parser size
number/shape of GLR conflicts
external scanner cost
error recovery breadth
incremental subtree reuse
```

Never evaluate a grammar change only by corpus correctness.

## 37.9 Large-file policy

Use explicit tiers:

```text
normal files     → full parse/query feature set
large files      → parse with stricter deadline, fewer expensive queries
very large files → optional partial/disabled semantic indexing, still basic syntax if affordable
```

Size alone is imperfect; generated/minified/repetitive structure can be more pathological than moderate source size. Combine byte, line, and observed-work thresholds.

## 37.10 Warm vs cold benchmarks

Report:

```text
cold grammar/query initialization
warm fresh parse
warm incremental parse
warm query
end-to-end document update
```

Do not hide startup cost if your deployment is serverless/short-lived or dynamically loads grammars.

## 37.11 Benchmark correctness gate

Any optimization must preserve:

```text
same final parse tree semantics
same required captures
same index facts
same revision consistency
same error-recovery policy
```

Speeding up by narrowing invalidation incorrectly is a correctness regression.

## 37.12 Anti-pattern inventory

* Benchmarking only fresh parses for an editor workload.
* Reporting parser time while ignoring query/index cost.
* Compiling queries per file.
* Copying source text for every capture.
* Parallelizing to CPU×10 and calling the throughput drop a Tree-sitter issue.
* Optimizing query scope without capture-equivalence regression tests.

# Tree-sitter Advanced — 38) Observability and debugging

## 38.0 Debug at four layers

```text
source/edit layer
  → exact bytes, revision, InputEdit, encoding
parser layer
  → parse/lex logger, parse budget, ERROR/MISSING nodes
syntax-tree layer
  → S-expression, node ranges/IDs/changed ranges, DOT graph
query layer
  → pattern index, captures, predicates, match-limit/cancellation metrics
```

When a result is wrong, identify the first layer at which observed state diverges from expectation. Do not start by rewriting the query when the source/tree revision is already inconsistent.

## 38.1 Parser logger

`Parser::set_logger` accepts a callback receiving `LogType` and a message.

```rust
use tree_sitter::{LogType, Parser};

let mut parser = Parser::new();
parser.set_logger(Some(Box::new(|kind, message| {
    match kind {
        LogType::Parse => eprintln!("PARSE {message}"),
        LogType::Lex => eprintln!("LEX {message}"),
    }
})));
```

Use only for targeted diagnostics. Parser logs can be extremely verbose and source-sensitive. Remove the logger in normal hot paths. See [parser].

## 38.2 Structured logger adapter

```rust
parser.set_logger(Some(Box::new(|kind, message| {
    tracing::trace!(
        target: "tree_sitter",
        log_type = ?kind,
        message = message,
    );
})));
```

Production policy:

```text
logger disabled by default
sampled/feature-flagged enablement
redact or avoid source-derived payloads in shared telemetry
per-request correlation id outside callback where possible
```

## 38.3 S-expression snapshot

```rust
let sexp = tree.root_node().to_sexp();
println!("{sexp}");
```

`Node::to_sexp()` is the fastest human-readable structural diagnostic for most parser problems. Snapshot it in tests for small fixtures, not as a long-term stable format for arbitrary huge files.

## 38.4 Range-rich node printer

```rust
fn print_node(node: tree_sitter::Node<'_>, depth: usize) {
    println!(
        "{:indent$}{} named={} error={} missing={} bytes={:?} points={:?}..{:?}",
        "",
        node.kind(),
        node.is_named(),
        node.is_error(),
        node.is_missing(),
        node.byte_range(),
        node.start_position(),
        node.end_position(),
        indent = depth * 2,
    );
}
```

Include both bytes and points when diagnosing edit-coordinate bugs.

## 38.5 DOT parser graphs

With `std`, the parser exposes DOT graph printing to a file descriptor:

```rust
use std::fs::File;

let file = File::create("parse.dot")?;
parser.print_dot_graphs(&file);

let _tree = parser.parse(source, None);
parser.stop_printing_dot_graphs();
```

DOT output is intended for deep parser-state/stack debugging. It can be large; never leave it enabled accidentally in a service. See [parser].

## 38.6 Tree DOT graph

`Tree::print_dot_graph` can emit a graph representation of an already built syntax tree:

```rust
let file = std::fs::File::create("tree.dot")?;
tree.print_dot_graph(&file);
```

Use parser DOT graphs for parsing-process diagnostics; use tree DOT graph/S-expression for final structure.

## 38.7 Edit audit record

```rust
#[derive(Debug)]
struct ParseAudit {
    revision: u64,
    previous_revision: Option<u64>,
    edit: Option<tree_sitter::InputEdit>,
    source_bytes: usize,
    incremental: bool,
    parse_micros: u64,
    root_has_error: bool,
    changed_ranges: Vec<tree_sitter::Range>,
}
```

This record answers the most common incremental bug question: “Did we parse the source revision we think we parsed with the edit we think we applied?”

## 38.8 Query diagnostics

Log/measure:

```text
query id/hash
query grammar fingerprint
root kind/range
byte/point/containing range restrictions
max_start_depth
match_limit
match count
capture count
did_exceed_match_limit
query duration
progress cancellation
```

If a query silently returns fewer results, check cursor ranges and disabled patterns/captures before blaming grammar parsing.

## 38.9 Capture trace helper

```rust
use tree_sitter::StreamingIterator;

fn trace_query(
    query: &tree_sitter::Query,
    cursor: &mut tree_sitter::QueryCursor,
    root: tree_sitter::Node<'_>,
    source: &[u8],
) {
    let names = query.capture_names();
    let mut matches = cursor.matches(query, root, source);

    while let Some(m) = matches.next() {
        eprintln!("pattern={}", m.pattern_index);
        for capture in m.captures {
            eprintln!(
                "  @{} {} {:?}",
                names[capture.index as usize],
                capture.node.kind(),
                capture.node.byte_range(),
            );
        }
    }
}
```

Use only with bounded fixtures/ranges; debugging a pathological full-file query by dumping every match can itself become the failure.

## 38.10 Error-node instrumentation

Track separate counts:

```text
ERROR nodes
MISSING nodes
root.has_error
largest ERROR byte span
semantic captures intersecting error regions
```

A single `has_error=true` metric loses too much information for grammar/index health monitoring.

## 38.11 Upgrade diagnostics artifact

For a representative corpus, store a machine-readable report:

```text
file
source hash
grammar fingerprint
parse time
root S-expression hash
error/missing counts
query capture hashes
changed-range metrics for edit scenarios
```

Compare before/after runtime or grammar upgrades.

## 38.12 Anti-pattern inventory

* Leaving parser logger enabled globally.
* Logging raw source snippets into multi-tenant telemetry by default.
* Debugging incremental bugs without recording exact `InputEdit`.
* Treating S-expression formatting as a forever-stable wire protocol.
* Dumping unbounded query matches during an incident.
* Recording only `root.has_error` and no recovery detail.

# Tree-sitter Advanced — 39) Error handling and diagnostics

## 39.0 Error taxonomy

Tree-sitter Rust deliberately uses different return shapes for different failure classes:

```text
Parser::set_language        → Result<(), LanguageError>
Parser::set_included_ranges → Result<(), IncludedRangesError>
Query::new                  → Result<Query, QueryError>
WasmStore operations        → Result<..., WasmError>
Parser::parse*              → Option<Tree>
Node::utf8_text             → Result<&str, Utf8Error>
```

Do not collapse all of these into “parse error.”

## 39.1 `LanguageError`

Current source models at least version incompatibility, plus a WASM-related variant when that feature is enabled:

```rust
match parser.set_language(&language) {
    Ok(()) => {}
    Err(tree_sitter::LanguageError::Version(version)) => {
        eprintln!("incompatible grammar ABI: {version}");
    }
    #[cfg(feature = "wasm")]
    Err(tree_sitter::LanguageError::Wasm) => {
        eprintln!("WASM language requires/configuration failed for store");
    }
}
```

Pin to the current enum before exhaustive matching in shared libraries; feature flags alter the enum surface. See [binding-source].

## 39.2 `IncludedRangesError`

Current Rust binding defines it as a tuple struct containing the offending range index:

```rust
if let Err(tree_sitter::IncludedRangesError(index)) = parser.set_included_ranges(&ranges) {
    eprintln!("invalid included range at index {index}");
}
```

Validate range order/overlap earlier if you can provide richer product diagnostics. See [binding-source].

## 39.3 `QueryError`

`Query::new` provides structured compile diagnostics:

```text
row
column
offset
message
kind: QueryErrorKind
```

See [query-error].

```rust
match tree_sitter::Query::new(&language, query_source) {
    Ok(query) => query,
    Err(e) => {
        eprintln!(
            "query compile error {:?} at {}:{} (byte {}): {}",
            e.kind, e.row, e.column, e.offset, e.message
        );
        return;
    }
};
```

This is ideal for generated-query agents: return exact grammar/query diagnostics instead of a generic failure.

## 39.4 Parse `Option<Tree>`

```rust
let tree = parser.parse(source, old_tree);
```

`None` is not equivalent to syntactically invalid source. Invalid/incomplete source usually produces `Some(Tree)` containing error recovery. `None` can arise when no language is assigned or when parsing does not complete due to callback/progress cancellation behavior. See [parser].

Application error enum:

```rust
enum ParseOutcome {
    Complete(tree_sitter::Tree),
    CancelledOrIncomplete,
}
```

Better still, track your own cancellation flag inside the progress callback so `None` can be classified precisely.

## 39.5 WASM error

`WasmError` exposes a `kind` and message. Preserve both plus grammar byte hash/name in diagnostics. Never expose arbitrary untrusted engine internals/raw paths directly to public users without redaction. See [wasm-error].

## 39.6 UTF-8 source slicing error

`Node::utf8_text(source)` validates UTF-8 for the node's byte range:

```rust
match node.utf8_text(source) {
    Ok(text) => println!("{text}"),
    Err(err) => eprintln!("invalid UTF-8 in capture: {err}"),
}
```

If your system supports arbitrary bytes/custom encodings, use byte slices or the appropriate encoding-specific path rather than forcing UTF-8 semantics.

## 39.7 Context wrapping

A production error should include:

```text
operation: parse | set_language | query_compile | query_execute | included_ranges | wasm_load
document id (non-sensitive)
source revision
grammar registry id/version/fingerprint
query id/hash
range if applicable
cancelled / match-limit flags
underlying Tree-sitter error
```

Use `thiserror`/`anyhow` or your service's standard error model around Tree-sitter types; preserve the structured cause.

## 39.8 Recoverable vs fatal

| Failure | Typical service action |
|---|---|
| source syntax ERROR/MISSING | continue with partial-confidence tree/index |
| query match limit exceeded | mark extraction incomplete; narrow/adjust query or degrade |
| parse deadline exceeded | retain stale prior tree/result; reschedule/degrade |
| query compile error at startup | fail deployment or quarantine grammar/query bundle |
| grammar ABI mismatch | reject grammar at registry load |
| invalid included ranges | programming/data-consistency error; recompute ranges |
| WASM grammar load failure | quarantine grammar; do not retry per document blindly |

## 39.9 Anti-pattern inventory

* Reporting a source syntax error as a Rust `Err` from parse.
* Unwrapping grammar/query setup in a plugin system with untrusted packages.
* Swallowing `QueryError` location/message.
* Treating query match-limit overflow as a successful complete result.
* Retrying ABI-incompatible grammar assignment on every request.
* Losing source revision/grammar fingerprint in error telemetry.

# Tree-sitter Advanced — 40) API stability, ABI compatibility, and upgrades

## 40.0 Version axes are independent

```text
Rust runtime crate        tree-sitter 0.26.12
grammar crate             e.g. tree-sitter-json 0.24.8
generator CLI             tree-sitter CLI version
generated parser ABI      language.abi_version()
query bundle              grammar-specific .scm revision
application extraction schema
```

Do not use one “Tree-sitter version” field to represent all five.

## 40.1 Runtime ABI window

The Rust runtime publishes:

```text
LANGUAGE_VERSION = latest supported generated-language ABI
MIN_COMPATIBLE_LANGUAGE_VERSION = earliest supported ABI
```

For the runtime pinned by this document, those are 15 and 13 respectively. Tree-sitter documents backward compatibility across supported older generated languages but not forward compatibility with a newer unsupported ABI. See [language-version] and [min-language-version].

## 40.2 Assignment is the authoritative ABI gate

```rust
parser.set_language(&language)?;
```

`Parser::set_language` rejects an incompatible grammar. Perform it at registry initialization, not first user request.

## 40.3 Runtime upgrade checklist

```text
[ ] cargo update exact tree-sitter version in branch
[ ] compile all grammar crates
[ ] load every language into Parser
[ ] compile every Query
[ ] run corpus parse goldens
[ ] run incremental fresh-vs-old-tree equivalence tests
[ ] run error-recovery fixtures
[ ] run query capture goldens
[ ] run performance baseline
[ ] inspect release/API changes to ParseOptions/QueryCursor/etc.
[ ] rebuild WASM integration tests if feature used
```

## 40.4 Grammar upgrade checklist

```text
[ ] record old/new grammar package version and generation provenance
[ ] inspect node-types.json diff
[ ] visible kinds renamed/added/removed?
[ ] fields changed?
[ ] supertype membership changed?
[ ] queries compile?
[ ] capture goldens changed?
[ ] ERROR/MISSING shapes changed?
[ ] changed_ranges behavior widened?
[ ] parse performance changed?
```

A grammar upgrade is often more semantically disruptive to an extractor than a core runtime patch.

## 40.5 Query compatibility

A query is compiled against a specific `Language` and references grammar node/field names. Never cache a compiled `Query` under only the query-source hash.

Correct cache key:

```text
query source hash
+ grammar fingerprint
+ runtime major/minor as required by API behavior
```

## 40.6 CST schema migration

If the application persists syntactic records, version its extraction schema independently:

```rust
struct IndexHeader {
    extractor_schema_version: u32,
    grammar_fingerprint: String,
    runtime_version: String,
}
```

On mismatch, either migrate records or re-index. Do not silently mix facts produced from materially different CST contracts.

## 40.7 Numeric IDs are not cross-version contracts

These are version-scoped implementation values:

```text
Node::kind_id
Node::grammar_id
field IDs
parse states
query capture index
query pattern index
```

Persist readable semantic identifiers or your own stable enums when data survives process/grammar upgrades.

## 40.8 Upgrade canary

Before fleet rollout:

```text
run new registry against representative repositories
compare parse errors
compare symbol/reference/call counts
sample detailed graph diffs
compare p50/p95/p99 parse + query time
compare memory/RSS
```

Treat large unexplained drops in captures as correctness incidents even when no Rust errors occur.

## 40.9 Anti-pattern inventory

* Upgrading all grammar crates and runtime in one unreviewed bulk change.
* Treating successful compilation as query compatibility proof.
* Persisting kind IDs across grammar upgrades.
* Caching queries by source text alone.
* Skipping incomplete/error-recovery fixtures during migration.
* Ignoring performance regressions because output goldens pass.

# Tree-sitter Advanced — 41) Production deployment patterns

## 41.0 Pattern A — editor/document service

```text
DocumentState
  revision
  Arc<[u8]> source
  Tree current_tree
  semantic owner index
  diagnostics

Edit event
  → InputEdit
  → old_tree.edit
  → incremental parse with deadline
  → changed_ranges
  → owner-scope re-query
  → publish immutable new snapshot
```

Core requirements: low latency, stale-result suppression, useful incomplete-code recovery, precise coordinate conversion.

## 41.1 Pattern B — repository batch indexer

```text
file discovery
  → language classification
  → worker pool by grammar
  → fresh parse
  → symbol/import/reference/call queries
  → semantic transform
  → durable index write
```

Batch priorities:

```text
throughput
bounded per-file work
fault isolation
language coverage
reproducible grammar/query fingerprints
```

Use fresh parsing for first ingestion; incremental state becomes valuable when the repository is watched continuously.

## 41.2 Pattern C — continuously updating repository daemon

```text
filesystem watcher
  → debounce/coalesce file events
  → obtain exact new source
  → compute edit(s) if old source available
  → incremental Tree-sitter update
  → changed owner scopes
  → transactional index delta
  → publish new graph revision
```

Do not derive `InputEdit` directly from lossy filesystem notifications. The parser needs the exact textual edit relation or a deliberate fresh-parse fallback.

## 41.3 Pattern D — language-server subsystem

Tree-sitter responsibilities:

```text
fast syntax tree
syntax diagnostics/recovery
folding/selection ranges
structural symbols
query-driven semantic token candidates
injection detection
syntax-aware completion categories
```

Compiler/LSP semantic engine responsibilities:

```text
type checking
name resolution
cross-file module resolution
trait/interface dispatch
macro-expanded semantics where required
```

Use Tree-sitter as a fast structural substrate, not as a substitute for semantic compilation.

## 41.4 Pattern E — multi-tenant parse/query API

Tenant boundary should include:

```text
allowed grammar registry
allowed query IDs (prefer precompiled server-owned queries)
source byte limit
parse deadline
query deadline
query match limit
result byte/record limit
WASM grammar policy
telemetry namespace
```

Never expose arbitrary native grammar loading from an untrusted tenant into the main server process.

## 41.5 Pattern F — plugin grammar platform

```text
PluginRegistry
  signed/provenance-checked grammar package
  grammar fingerprint
  ABI validation
  node-schema catalog
  approved query bundle
  optional WASM bytes

Worker
  loads immutable registry generation
  enforces per-parse/query budgets
```

Prefer WASM or process isolation when third-party grammar code is not fully trusted; native external scanners are handwritten native code.

## 41.6 Storage model for continuously updated index

```rust
struct FileSyntaxState {
    file_id: u64,
    revision: u64,
    content_hash: [u8; 32],
    grammar_fingerprint: String,
    tree: tree_sitter::Tree,
    owners: Vec<OwnerRecord>,
}
```

Persisting `Tree` across process restarts is not a normal Tree-sitter wire format. Persist source/index facts and rebuild trees on process recovery unless you own a separate compatible serialization layer.

## 41.7 Atomic update boundary

```text
source revision + tree + extracted records + graph delta
```

should advance coherently. A failure after parse but before index commit must not leave “new tree / old graph” exposed as one current revision.

## 41.8 Backpressure

When edits arrive faster than indexing:

```text
collapse superseded revisions
keep newest source revision
cancel stale parse/query work
prefer one parse of latest source over finishing every intermediate keystroke
```

Tree-sitter's fast incremental design does not justify an unbounded revision queue.

## 41.9 Graceful degradation

```text
parse timeout        → keep old tree marked stale; schedule latest revision
query timeout        → publish syntax tree but hold semantic index update
match-limit overflow → flag query incomplete, rerun narrower scope or skip expensive feature
very large file      → syntax-only / reduced query set according to policy
```

## 41.10 Anti-pattern inventory

* Reusing old tree with a new source without exact edit synchronization.
* Persisting raw Node handles in a database.
* Treating filesystem event boundaries as text edit boundaries.
* Allowing stale background index results to overwrite a newer revision.
* One global parser/cursor for a whole service.
* No transactional boundary between file revision and graph facts.

# Tree-sitter Advanced — 42) Security and resource governance

## 42.0 Threat model

Inputs can be untrusted at multiple levels:

```text
source code bytes
query text
native grammar package / external scanner
WASM grammar bytes
injection language names/ranges
file paths used to load grammar/query assets
```

The trust policy for each must be explicit.

## 42.1 Untrusted source code

Even a memory-safe parser binding can consume CPU/memory on pathological input. Enforce:

```text
max source bytes
parse progress deadline/cancellation
worker concurrency cap
process/container memory limits
large-file fallback
error/cancellation telemetry
```

Source code may also contain secrets; avoid logging snippets by default.

## 42.2 Untrusted query text

If users can submit arbitrary query source:

```text
compile deadline at outer service level
query length limit
allowed grammar only
QueryCursor match limit
query progress deadline
byte/point search range policy
result row/byte caps
```

Prefer server-owned query IDs for production APIs when arbitrary structural query power is unnecessary.

## 42.3 Match-limit semantics

`QueryCursor::set_match_limit` limits the number of **in-progress** matches and is capped by the Rust API. After execution, inspect `did_exceed_match_limit()`. See [query-cursor].

```rust
cursor.set_match_limit(4096);
// execute
if cursor.did_exceed_match_limit() {
    // result is not guaranteed complete
}
```

Do not silently publish overflowed results as complete.

## 42.4 Native grammar trust

A native grammar package includes generated C and may include handwritten C/C++ external scanner code. Loading a third-party native grammar into your process is equivalent to trusting native code.

Controls:

```text
approved dependency/source registry
checksums/signatures/SBOM as applicable
code review for scanner.c/.cc
pinned versions
reproducible CI build
sandbox/process isolation for less-trusted plugins
```

## 42.5 WASM grammar trust

WASM can reduce some native-code loading risks, but still requires:

```text
byte-size limits
validated/provenance-checked module
engine/store resource limits where available
parse deadline
loaded-language count/quota
query limits
```

Treat it as a different execution boundary, not “safe by definition.”

## 42.6 Injection attacks

An injection query can cause recursive language parsing. Bound:

```text
maximum injection depth
maximum regions per document
maximum total injected bytes processed
allowed language transitions
cycle detection
```

Example cycle to reject:

```text
A injects B over full range
B injects A over same full range
```

## 42.7 Coordinate validation

Reject impossible edit/range data before FFI:

```text
start <= old_end
start <= new_end
points correspond to bytes/source revision
included ranges sorted/non-overlapping
node/capture byte ranges within source length before slicing
```

Safe Rust wrappers remove many pointer hazards, but application-created inconsistent source coordinates can still produce wrong results/panics in your own slicing code.

## 42.8 Custom allocator governance

`set_allocator` mutates global native allocator state and is unsafe. Treat it as process-initialization infrastructure, not a tenant/request feature. Never allow plugins/users to change it.

## 42.9 Result amplification

Small source can create many captures. Bound output separately from parse/query work:

```text
max captures
max serialized result bytes
max graph records per file
max diagnostics per file
```

Truncation must be explicit in response/index status.

## 42.10 Anti-pattern inventory

* Accepting arbitrary native grammar shared libraries in-process.
* Calling WASM grammars fully sandboxed without limits.
* Allowing arbitrary recursive injection graph.
* Using `did_exceed_match_limit` only for metrics and still committing partial index.
* Exposing `set_allocator` to plugin code.
* Logging raw multi-tenant source/capture text by default.
* Relying on source byte limit as the only work limiter.

# Tree-sitter Advanced — 43) Code-intelligence recipes

## 43.0 Recipe architecture

A robust syntax-derived code index should separate:

```text
Tree-sitter query capture
  → grammar-specific normalized syntax fact
  → semantic resolution/enrichment
  → graph/index record
```

Never make raw capture names your durable cross-language ontology.

## 43.1 Symbol definitions

Query example (grammar-specific placeholder kinds):

```scheme
(function_definition
  name: (identifier) @symbol.name) @symbol.definition

(class_definition
  name: (identifier) @symbol.name) @symbol.definition
```

Normalize:

```rust
struct SymbolDef {
    kind: SymbolKind,
    name: String,
    full_range: std::ops::Range<usize>,
    name_range: std::ops::Range<usize>,
    owner_scope: Option<SemanticId>,
    syntax_confidence: Confidence,
}
```

Use field constraints whenever grammar exposes them.

## 43.2 Imports/modules

```scheme
(import_statement
  source: (string) @import.source) @import
```

Tree-sitter can reliably capture the syntactic import string and imported names. Resolving that string to a file/module is a separate resolver based on project configuration, package manager, paths, conditional compilation, etc.

```text
syntax edge: file → import_specifier "foo/bar"
semantic edge: file → resolved module/file ID
```

Store both when resolution can fail or change.

## 43.3 Calls

```scheme
(call_expression
  function: (_) @call.callee
  arguments: (arguments) @call.arguments) @call
```

Normalize syntactic callee shape:

```text
identifier call        foo()
member call            obj.foo()
qualified/path call    mod::foo()
computed call          expr() / (get_fn())()
```

Do not create a definitive cross-function call edge solely from text equality. Emit a syntactic call site, then resolve with language/compiler context.

## 43.4 Function signatures

Use fields to avoid traversing body:

```scheme
(function_definition
  name: (_) @fn.name
  parameters: (_) @fn.parameters
  return_type: (_) @fn.return_type?) @fn.def
```

Optional-field query syntax is grammar/query-version specific; often separate patterns are clearer than overloading one pattern.

Extract signature facts even when body has syntax errors if signature range is clean.

## 43.5 Scope records

Capture syntactic scope roots:

```text
file/module
namespace/package
class/impl
function/method/closure
block where language has lexical block scope
```

Then create owner containment:

```text
smallest enclosing scope range contains definition/reference
```

For languages with complicated scope rules, enrich/override this with semantic analysis.

## 43.6 References

Tree-sitter can capture identifier occurrences, but not every identifier is a reference:

```text
definition name
field/property key
type name
label
import alias
attribute
macro token
```

Build grammar-specific exclusion/context patterns or locals queries before classifying a generic identifier as a candidate reference.

## 43.7 Changed-range owner invalidation

Algorithm:

```text
InputEdit
  → edit old tree
  → parse new tree with old
  → structural changed_ranges(old,new)
  → for each changed range:
      find smallest enclosing semantic owner in old and new trees
      union owner regions
  → delete index records owned by invalidated old owners
  → rerun queries over invalidated new owners
  → upsert new records
```

This avoids relying on raw text edit span, which can understate syntactic impact.

## 43.8 Owner identity

Use a composite rather than raw `Node::id` as durable semantic identity:

```text
language-specific owner kind
qualified/local name when available
parent semantic identity
stable signature features
source range as fallback
node id only as same-revision/reuse hint
```

A reused Tree-sitter node ID is valuable evidence but not sufficient for long-term repository identity.

## 43.9 Capture-to-record provenance

Every syntax fact should retain:

```text
file id
source revision
grammar fingerprint
query bundle version
query pattern/capture semantic id
source range
owner id
confidence / intersects_error
```

This makes it possible to explain and selectively rebuild records after grammar/query upgrades.

## 43.10 Multi-language document indexing

```text
host parse
  → injection regions
  → embedded parse trees with document-global coordinates
  → run language-specific extractors
  → merge records under one file revision
```

Prefix semantic IDs with language layer/region ownership so identical byte ranges from host/embedded parses do not collide accidentally.

## 43.11 Hybrid call graph

Recommended confidence tiers:

```text
Tier 1: Tree-sitter syntactic call site only
Tier 2: local syntactic resolution (unique local definition/import alias)
Tier 3: LSP/compiler/type-checker resolution
Tier 4: runtime/dynamic evidence if available
```

Store provenance/confidence instead of forcing uncertain syntax edges into the same bucket as compiler-resolved calls.

## 43.12 Structural search API

Expose precompiled query families rather than arbitrary raw query text when possible:

```text
find functions
find classes/types
find calls
find imports
find error regions
find nodes of kind X
find pattern within changed range
```

Internally use `QueryCursor` ranges/max depth to make these operations predictable.

## 43.13 Fault-tolerant index transaction

```text
parse complete?
  no → preserve old index revision / mark stale
  yes → run required extractors

required extractor complete?
  no → do not publish partial required graph revision
  yes → atomically commit file tree metadata + facts

optional extractor timeout?
  publish base revision with explicit optional-feature stale flag
```

## 43.14 Rust skeleton

```rust
struct FileEngine {
    parser: tree_sitter::Parser,
    symbol_query: std::sync::Arc<tree_sitter::Query>,
    call_query: std::sync::Arc<tree_sitter::Query>,
    cursor: tree_sitter::QueryCursor,
}

struct FileSnapshot {
    revision: u64,
    source: std::sync::Arc<[u8]>,
    tree: tree_sitter::Tree,
}

struct FileDelta {
    invalidated_owners: Vec<SemanticId>,
    new_symbols: Vec<SymbolDef>,
    new_calls: Vec<CallSite>,
}
```

Keep parser/cursor mutable worker state separate from immutable published document snapshots.

## 43.15 Anti-pattern inventory

* “Every identifier capture is a reference.”
* “Same callee text means same function.”
* Re-running repository-wide queries for one local edit.
* Invalidating only the raw edit span.
* Using Node IDs as durable graph IDs.
* Mixing syntax candidate edges with compiler-resolved edges without provenance.
* Publishing partial query output as a complete graph revision.

# Tree-sitter Advanced — 44) Consolidated best practices, invariants, and anti-patterns

## 44.0 Core mental-model invariants

**Invariant 1 — The Rust crate is a binding over Tree-sitter's native parser core.**

```text
Rust Parser/Tree/Node/Query API
  → Tree-sitter C runtime
  → generated grammar code/tables
```

A language grammar crate supplies `Language`; it is not the runtime itself.

**Invariant 2 — A `Tree` is tied to one exact source revision.**

Node byte/point ranges may only be interpreted against the matching source snapshot.

**Invariant 3 — Incremental parsing requires editing the old tree first.**

```text
old source/tree
  + exact InputEdit
  → old_tree.edit(edit)
  → parser.parse(new_source, Some(&old_tree))
```

Passing a stale unedited old tree is a correctness bug.

**Invariant 4 — Raw edit span is not structural change span.**

Use `Tree::changed_ranges` after reparsing to identify syntactic impact, then widen to semantic owners as required.

**Invariant 5 — Syntax errors normally produce trees.**

`ERROR`/`MISSING` nodes are first-class recovery information. Do not discard the whole file because `root.has_error()`.

**Invariant 6 — Query and grammar are version-coupled.**

Compile all queries against the exact `Language` at startup/CI and cache by grammar fingerprint.

**Invariant 7 — Query cursors/parsers are mutable execution state.**

Pool/own them per worker; share immutable `Language`/`Query` registry data.

**Invariant 8 — Tree-sitter is syntactic.**

Use compiler/LSP/domain analysis for semantic name/type/call resolution when correctness requires it.

## 44.1 Agent rules for parser setup

```text
[ ] Pin tree-sitter runtime version.
[ ] Pin every grammar crate/version.
[ ] Convert grammar LANGUAGE/LanguageFn into tree_sitter::Language.
[ ] Call parser.set_language at registry startup and handle ABI error.
[ ] Compile queries once, not per file.
[ ] Record grammar/query fingerprint.
[ ] Use one mutable parser/cursor per worker.
```

## 44.2 Agent rules for source input

```text
[ ] Know whether source is UTF-8, UTF-16LE/BE, or custom encoding.
[ ] Treat byte offsets as bytes, never generic character indices.
[ ] Keep Point row/column conventions consistent with Tree-sitter/source encoding.
[ ] For callback input, return data starting at requested offset/point and empty slice at EOF.
[ ] Call parser.reset before reusing a parser on unrelated input after a cancelled/interrupted parse if resume is not intended.
```

## 44.3 Agent rules for traversal

```text
[ ] Use fields for semantic child roles.
[ ] Use TreeCursor for long/recursive traversal.
[ ] Use named_children when punctuation is irrelevant.
[ ] Use descendant_for_* for positional lookup.
[ ] Avoid repeated child(i) loops over very large child lists when cursor iteration is better.
[ ] Re-fetch nodes after tree edit unless deliberately editing retained Node values.
```

## 44.4 Agent rules for incremental updates

```text
[ ] Compute exact InputEdit from old and new source.
[ ] Edit ranges/points cached outside the tree with InputEdit helpers.
[ ] old_tree.edit(edit) before reparse.
[ ] Reparse with old tree.
[ ] Compare old edited tree vs new tree with changed_ranges.
[ ] Map structural changes to semantic owner scopes.
[ ] Commit new source/tree/index under one revision.
[ ] Discard stale async results.
```

## 44.5 Agent rules for queries

```text
[ ] Build Query against exact Language.
[ ] Fail startup/CI on QueryError.
[ ] Use capture names as query-layer identifiers, then normalize to application schema.
[ ] Use matches when pattern grouping matters; captures for ordered capture stream.
[ ] Import StreamingIterator traits.
[ ] Restrict byte/point range when safe.
[ ] Use containing range when every match must fit fully inside a region.
[ ] Use max_start_depth for subtree destructuring/search control.
[ ] Set match limit and inspect overflow.
[ ] Add progress cancellation for untrusted/interactive workloads.
```

## 44.6 Agent rules for errors

```text
[ ] Some(Tree) with has_error = completed recovery tree.
[ ] ERROR = consumed unrecognized/unincorporable source region.
[ ] MISSING = inserted expected zero-width construct.
[ ] Preserve clean semantic facts outside error region.
[ ] Never report parse None as syntax error without classification.
[ ] Preserve QueryError row/column/offset/kind/message.
```

## 44.7 Agent rules for grammar packages

```text
[ ] Treat grammar CST as a downstream API.
[ ] Use fields generously for stable semantic roles.
[ ] Use hidden helpers/supertypes to keep CST readable.
[ ] Distinguish parse precedence from lexical precedence.
[ ] Declare only genuine GLR conflicts.
[ ] External scanner state must serialize correctly for incremental reuse.
[ ] Run tree-sitter corpus + Rust binding + incremental tests.
[ ] Diff node-types.json on upgrades.
```

## 44.8 Agent rules for production

```text
[ ] Bound source bytes.
[ ] Bound parse time via progress callback/deadline.
[ ] Bound query work via range/depth/match limit/deadline.
[ ] Bound result records/bytes.
[ ] Bound parser worker concurrency.
[ ] Bound injection depth/region count.
[ ] Treat native grammar/scanner code as trusted native code.
[ ] Treat WASM as a distinct boundary, not an unlimited sandbox.
[ ] Keep source logging off/redacted by default.
```

## 44.9 High-value anti-pattern list

1. **Using Python-oriented Tree-sitter examples in a Rust service.** Use the official Rust `tree-sitter` crate and Rust grammar packages.
2. **Calling the Rust language grammar `tree-sitter-rust` the Rust implementation of Tree-sitter.** It is only the grammar for parsing Rust source.
3. **Assuming the core is pure Rust.** The official Rust package binds to Tree-sitter's native C parsing implementation.
4. **Passing an old tree without applying `InputEdit`.** This corrupts the incremental contract.
5. **Applying only byte deltas and ignoring `Point`s.** Multiline edits require both coordinate systems.
6. **Treating `changed_ranges` as identical to edit ranges.** Structural changes can extend beyond edited bytes.
7. **Keeping stale nodes after editing without refetch/edit.** Cached node positions can be wrong.
8. **Using `child(i)` for every long traversal.** Prefer `TreeCursor`/child iterators.
9. **Rejecting entire files on syntax errors.** Recovery trees are designed for incomplete code.
10. **Compiling queries on every parse.** Precompile and cache by grammar fingerprint.
11. **Ignoring `did_exceed_match_limit()`.** Partial query results are not complete.
12. **Using arbitrary query text without deadlines/result limits in a service.** Query work can amplify.
13. **Treating syntax capture as semantic resolution.** Add compiler/LSP/domain analysis.
14. **Persisting Tree-sitter numeric IDs as stable cross-version schema.** Keep them process/grammar scoped.
15. **Recomputing whole-repository index after every edit.** Use changed owner scopes.
16. **Loading untrusted native grammars in-process.** Generated/handwritten native code is executable code.
17. **Changing the global native allocator at request time.** `set_allocator` is unsafe global initialization.
18. **Blindly accepting grammar regenerated output.** Review node/query/incremental diffs.

## 44.10 Reference architecture: continuously updating Rust code index

```text
                        ┌───────────────────────────┐
file/editor event ─────▶│ Source Revision Manager   │
                        │ Arc<[u8]>, revision, hash  │
                        └──────────────┬────────────┘
                                       │ exact edit / fresh fallback
                                       ▼
                        ┌───────────────────────────┐
                        │ Parser Worker Pool        │
                        │ Parser + QueryCursor      │
                        └──────────────┬────────────┘
                                       │
                    old tree.edit ─────┤
                    incremental parse  │
                                       ▼
                        ┌───────────────────────────┐
                        │ Syntax Snapshot           │
                        │ Tree + source revision    │
                        └──────────────┬────────────┘
                                       │ changed_ranges
                                       ▼
                        ┌───────────────────────────┐
                        │ Owner Invalidation        │
                        │ scope/function/type roots │
                        └──────────────┬────────────┘
                                       │ bounded queries
                                       ▼
                        ┌───────────────────────────┐
                        │ Syntax Fact Extractors    │
                        │ symbols/imports/calls/... │
                        └──────────────┬────────────┘
                                       │ candidates + provenance
                                       ▼
                        ┌───────────────────────────┐
                        │ Semantic Enrichment       │
                        │ LSP/compiler/resolvers    │
                        └──────────────┬────────────┘
                                       │
                                       ▼
                        ┌───────────────────────────┐
                        │ Atomic Graph/Index Commit │
                        │ file revision N           │
                        └───────────────────────────┘
```

## 44.11 Final implementation checklist

```text
Runtime and grammars
[ ] tree-sitter runtime exact/policy pin
[ ] grammar exact/policy pins
[ ] ABI startup validation
[ ] grammar fingerprints
[ ] query compile startup gate

Parsing
[ ] UTF-8/UTF-16/custom input policy
[ ] exact source revision association
[ ] progress deadline
[ ] ERROR/MISSING policy
[ ] large-file policy

Incremental updates
[ ] exact InputEdit
[ ] old tree edit
[ ] incremental reparse
[ ] changed_ranges
[ ] semantic owner widening
[ ] stale-result suppression

Queries
[ ] precompiled
[ ] capture schema normalized
[ ] range/depth restrictions
[ ] match limit + overflow check
[ ] progress deadline
[ ] result caps

Concurrency
[ ] parser/cursor worker pool
[ ] immutable registry generations
[ ] immutable published source/tree snapshots
[ ] bounded CPU concurrency

Security
[ ] native grammar trust policy
[ ] external scanner review/fuzzing
[ ] WASM grammar policy
[ ] recursive injection bounds
[ ] no raw-source telemetry by default

Testing
[ ] corpus tests
[ ] Rust binding smoke tests
[ ] query goldens
[ ] fresh-vs-incremental equivalence
[ ] error-recovery cases
[ ] grammar/runtime upgrade canary
[ ] performance baselines
```

# Tree-sitter Advanced — 45) Parsing Python source with native Rust Tree-sitter

## 45.0 Scope: Python *source* parsed by a Rust host

This chapter adds the language-specific layer for **parsing Python source code from Rust**. The architecture remains the same as the rest of this reference:

```text
Rust application
  └─ tree-sitter = 0.26.12
       └─ Parser / Tree / Node / Query / QueryCursor
            └─ tree-sitter-python = 0.25.0
                 ├─ LANGUAGE: tree_sitter_language::LanguageFn
                 ├─ generated src/parser.c
                 ├─ src/scanner.c
                 ├─ NODE_TYPES
                 ├─ HIGHLIGHTS_QUERY
                 └─ TAGS_QUERY
                      └─ parses *.py / *.pyi / other Python source bytes
```

This is **not**:

```text
Python process -> py-tree-sitter -> grammar
```

and it is **not**:

```text
Rust source parsing -> tree-sitter-rust grammar
```

The host language is Rust; the parsed language is Python. The current upstream Python grammar crate is `tree-sitter-python` **0.25.0**. Its Rust package uses `bindings/rust/build.rs`, depends on `tree-sitter-language`, and compiles generated native parser/scanner code with the `cc` crate. It exposes the standard grammar constants `LANGUAGE`, `NODE_TYPES`, `HIGHLIGHTS_QUERY`, and `TAGS_QUERY`. ([tree-sitter-python Rust crate][python-crate]) ([tree-sitter-python Cargo.toml][python-cargo])

**Agent invariant:** never substitute Python-binding syntax such as `Language(tspython.language())`, `Parser(PY_LANGUAGE)`, or Python `QueryCursor` examples into a native Rust implementation. In Rust, use `tree_sitter_python::LANGUAGE` and convert that `LanguageFn` into `tree_sitter::Language`.

---

## 45.1 Version and dependency anchors

Recommended pinned application manifest:

```toml
[package]
name = "python-syntax"
version = "0.1.0"
edition = "2024"

[dependencies]
tree-sitter = "=0.26.12"
tree-sitter-python = "=0.25.0"
```

The grammar package's own development manifest currently uses `tree-sitter = 0.25.8` only as a **dev dependency**; application code does not need to mirror that version. The runtime/grammar handoff is through `tree-sitter-language::LanguageFn`, and `Parser::set_language` is the authoritative ABI-compatibility gate. ([tree-sitter-python Cargo.toml][python-cargo])

Convenience setup commands:

```bash
cargo new python-syntax
cd python-syntax

cargo add tree-sitter@0.26.12
cargo add tree-sitter-python@0.25.0

cargo check
cargo run --release
```

For a workspace that wants reproducible exact pins, prefer editing the manifest to the exact `=` requirements shown above rather than relying only on the version requirement written by `cargo add`.

Useful dependency audit commands:

```bash
cargo tree -i tree-sitter
cargo tree -i tree-sitter-language
cargo tree -i tree-sitter-python
cargo tree -d
```

Deployment prerequisite: because the Python grammar crate builds generated native code, the build environment needs a working C compiler/toolchain. The grammar crate's `build.rs` is part of the normal Cargo build; end users do **not** need Node.js or the Tree-sitter CLI merely to consume the published crate. Node.js/QuickJS and the CLI are grammar-development tools, not normal runtime dependencies. ([tree-sitter-python Cargo.toml][python-cargo]) ([Tree-sitter grammar getting started][grammar-getting-started])

---

## 45.2 First executable Python parser in Rust

```rust
use tree_sitter::{Language, Parser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = br#"
from pathlib import Path

@decorator
async def load_text(path: Path) -> str:
    return await read_text(path)
"#;

    let language: Language = tree_sitter_python::LANGUAGE.into();

    let mut parser = Parser::new();
    parser.set_language(&language)?;

    let tree = parser
        .parse(source, None)
        .ok_or("parse cancelled or parser had no usable language")?;

    let root = tree.root_node();
    println!("root kind: {}", root.kind());
    println!("has error: {}", root.has_error());
    println!("{}", root.to_sexp());

    Ok(())
}
```

Expected root kind:

```text
module
```

The Python grammar's top-level rule is `module: repeat(_statement)`. Compound statements include functions, classes, decorated definitions, `if`, `for`, `while`, `try`, `with`, and `match`; simple statements include imports, assignments, returns, `global`, `nonlocal`, type aliases, and others. ([Python grammar source][python-grammar])

**Do not reject a tree merely because `has_error()` is true.** For editor/indexer use, return the tree, extract trusted subtrees outside recovery regions, and record error diagnostics separately.

---

## 45.3 Minimal reusable Python parser wrapper

```rust
use tree_sitter::{Language, Parser, Tree};

pub struct PythonParser {
    parser: Parser,
}

impl PythonParser {
    pub fn new() -> Result<Self, tree_sitter::LanguageError> {
        let language: Language = tree_sitter_python::LANGUAGE.into();
        let mut parser = Parser::new();
        parser.set_language(&language)?;
        Ok(Self { parser })
    }

    pub fn parse(&mut self, source: &[u8], old_tree: Option<&Tree>) -> Option<Tree> {
        self.parser.parse(source, old_tree)
    }
}
```

Recommended worker ownership:

```text
one mutable Parser per active worker
one immutable Language in the registry
one immutable compiled Query per query pack/generation
one Tree per file revision
source bytes retained alongside each published Tree
```

Do not place one `Parser` behind a global mutex and serialize all Python parsing unless concurrency is intentionally capped to one worker.

---

## 45.4 CLI commands for validating the Python grammar and queries

The production service does not need the CLI, but it is extremely useful when authoring or debugging Python-specific queries.

Install the CLI:

```bash
cargo install tree-sitter-cli --locked
```

Clone the upstream grammar for exact grammar-local validation:

```bash
git clone https://github.com/tree-sitter/tree-sitter-python.git
cd tree-sitter-python
```

Run the grammar corpus:

```bash
tree-sitter test
```

Parse one Python file using that grammar:

```bash
tree-sitter parse /absolute/path/to/example.py
```

Explicit grammar-path form from outside the grammar directory:

```bash
tree-sitter parse \
  --grammar-path /path/to/tree-sitter-python \
  /path/to/example.py
```

Inspect parse timing:

```bash
tree-sitter parse --time /path/to/example.py
```

Run the upstream tag query:

```bash
tree-sitter query \
  queries/tags.scm \
  /path/to/example.py
```

Run a custom query:

```bash
cat > /tmp/python-calls.scm <<'QUERY'
(call
  function: (identifier) @call.function) @call

(call
  function: (attribute
    object: (_) @call.receiver
    attribute: (identifier) @call.method)) @call
QUERY

tree-sitter query \
  /tmp/python-calls.scm \
  /path/to/example.py
```

Capture-ordered query output:

```bash
tree-sitter query \
  --captures \
  /tmp/python-calls.scm \
  /path/to/example.py
```

Range-local query debugging:

```bash
tree-sitter query \
  --byte-range 1000:5000 \
  /tmp/python-calls.scm \
  /path/to/example.py
```

The CLI's `parse` command accepts a grammar path and exits nonzero when parse errors occur; `query` accepts grammar paths plus byte/row range controls and capture-order output. These commands are ideal for validating a query before embedding it as a Rust `Query`. ([Tree-sitter parse CLI][cli-parse]) ([Tree-sitter query CLI][cli-query])

**Agent rule:** use the CLI for grammar/query exploration; use the Rust API in production. Do not shell out to `tree-sitter parse` once the grammar is already statically linked into the Rust process.

---

## 45.5 Python grammar architecture: indentation and strings require an external scanner

Python is unusually important for understanding Tree-sitter's external-scanner model. The upstream grammar declares external tokens for:

```text
_newline
_indent
_dedent
string_start
_string_content
escape_interpolation
string_end
comment
]
)
}
except
```

The grammar comments explain that comments are intentionally exposed to the external scanner so that it can preserve indentation/dedent structure during error recovery, and that closing delimiters are exposed so dedents are not emitted incorrectly while inside brackets. ([Python grammar source][python-grammar])

The native scanner also tracks string delimiter state, including quote kind and flags corresponding to raw, formatted, triple-quoted, and byte strings. ([Python scanner source][python-scanner])

Architecture consequence:

```text
Python source
  ├─ ordinary lexical rules in generated parser.c
  └─ indentation/string state in scanner.c
        ├─ NEWLINE
        ├─ INDENT / DEDENT stack
        └─ f/raw/triple/bytes string delimiter state
```

For consumers this means:

```text
Do not try to reconstruct Python blocks from leading spaces yourself.
Do not pre-strip indentation before parsing.
Do not normalize newlines in a way that invalidates byte/Point mappings.
Do not tokenize f-strings independently unless you deliberately need a second lexical layer.
Treat scanner.c changes as grammar changes that can affect incremental parsing.
```

The scanner's serialized state participates in incremental reuse. If you vendor/fork the Python grammar, scanner state and serialization are correctness-critical, not implementation trivia.

---

## 45.6 Python source encoding policy

Python itself determines a source file's encoding before parsing. Current Python defaults to UTF-8, supports a first/second-line encoding declaration, and ignores an initial UTF-8 BOM when the source encoding is UTF-8. Python's lexical analysis then operates on Unicode source characters. ([Python lexical reference][python-lexical])

Tree-sitter's Rust parser does **not** implement Python's encoding-cookie policy for your application. The host must define how repository bytes become parser input.

Recommended policy for a code-intelligence service:

```text
1. Read raw file bytes.
2. Detect UTF-8 BOM / Python coding cookie if non-UTF-8 Python files are in scope.
3. Decode according to product policy.
4. Parse UTF-8 bytes representing the decoded source.
5. Maintain a mapping if diagnostics must refer back to original encoded byte offsets.
```

Simpler UTF-8-only policy:

```rust
let bytes = std::fs::read(path)?;
let source = std::str::from_utf8(&bytes)?;
let tree = parser.parse(source.as_bytes(), old_tree);
```

If the repository contract guarantees UTF-8, this is preferable because byte offsets in Tree-sitter exactly correspond to the stored UTF-8 source buffer.

If legacy encodings are supported, beware:

```text
original-file byte offset != decoded UTF-8 Tree-sitter byte offset
```

That affects:

```text
InputEdit coordinates
external diagnostics
source-control patch offsets
LSP UTF-16 positions
persistent indexes keyed by byte ranges
```

**Agent invariant:** choose one canonical coordinate space. Do not silently decode Latin-1/Shift-JIS/etc. into UTF-8 and then report Tree-sitter byte offsets as offsets into the original file bytes.

---

## 45.7 Root, statements, logical lines, and semicolons

The Python grammar root is `module`. A module repeats statements. Simple statements are grouped into logical lines and may be separated with semicolons before the scanner-provided newline token; compound statements own suites/blocks. ([Python grammar source][python-grammar])

Conceptually:

```text
module
  ├─ import_statement
  ├─ expression_statement
  ├─ function_definition
  │    └─ block
  ├─ class_definition
  │    └─ block
  └─ ...
```

Python-specific consequence: physical line count is not the same as statement count.

Examples:

```python
x = 1; y = 2

value = (
    a
    + b
)
```

For symbol/statement indexing, use the CST's statement nodes, not `source.lines()`.

For line-oriented diagnostics, use `Node::start_position()` / `end_position()` only after the source-to-point coordinate policy is fixed.

---

## 45.8 Indentation, blocks, and incomplete edits

The grammar's `_suite` can represent:

```text
same-line simple statements
indented block
newline-only recovery form
```

and the `block` rule is closed by the external `_dedent` token. ([Python grammar source][python-grammar])

This is important while the user is typing:

```python
def f():
    if ready:
        work()
    # cursor here
```

A deletion or insertion of spaces near a line start may structurally affect many later lines because it changes the indentation stack. `Tree::changed_ranges` can therefore extend beyond the raw text edit.

Recommended invalidation rule for Python:

```text
raw edit
  -> Tree::edit(old_tree)
  -> incremental parse
  -> changed_ranges(old, new)
  -> widen each range to enclosing Python semantic owner
       function_definition
       class_definition
       module
       comprehension/lambda when indexed separately
  -> rerun owner-scoped extraction
```

Do not assume a two-space edit only invalidates two bytes of semantic output.

---

## 45.9 Identifiers, keywords, and soft keywords

A raw query such as:

```scheme
(identifier) @identifier
```

is almost never a sufficient Python reference extractor.

The same `identifier` syntax can participate in:

```text
function/class definitions
parameters
assignment targets
import bindings
for/comprehension targets
with-as bindings
except-as bindings
global/nonlocal declarations
pattern matching captures
attribute names
ordinary reads
call targets
type expressions
```

Python also has context-sensitive **soft keywords**. In current Python, `match`/`case` are soft keywords, and `type` is a soft keyword for the type-alias statement. ([Python compound statements][python-compound]) ([Python simple statements][python-simple])

The Tree-sitter Python grammar deliberately contains conflicts between normal expressions and `match_statement` / `type_alias_statement`, reflecting that contextual syntax. ([Python grammar source][python-grammar])

**Agent rule:** determine a name's role from its parent/field/context, not from token text alone.

---

## 45.10 Functions: names, parameters, return types, async, and type parameters

The current grammar defines `function_definition` with stable fields:

```text
name            -> identifier              required
parameters      -> parameters              required
return_type     -> type                    optional
type_parameters -> type_parameter          optional
body            -> block                   required
```

The rule also accepts an anonymous optional `async` token before `def`. ([Python node types][python-node-types]) ([Python grammar source][python-grammar])

Field-oriented extraction:

```rust
fn function_parts<'tree>(node: tree_sitter::Node<'tree>) {
    assert_eq!(node.kind(), "function_definition");

    let name = node.child_by_field_name("name").unwrap();
    let params = node.child_by_field_name("parameters").unwrap();
    let return_type = node.child_by_field_name("return_type");
    let type_params = node.child_by_field_name("type_parameters");
    let body = node.child_by_field_name("body").unwrap();

    println!("name node: {name:?}");
    println!("params: {params:?}");
    println!("return type: {return_type:?}");
    println!("type params: {type_params:?}");
    println!("body: {body:?}");
}
```

Definition query:

```scheme
(function_definition
  name: (identifier) @definition.function.name
  parameters: (parameters) @definition.function.parameters
  return_type: (type)? @definition.function.return_type
  body: (block) @definition.function.body) @definition.function
```

Async detection query:

```scheme
(function_definition
  "async" @definition.function.async
  name: (identifier) @definition.function.name) @definition.function
```

Do not infer `async` from the presence of `await`; an async function may contain no `await`, and `await` can appear in nested syntax owned by another function.

---

## 45.11 Decorators: the definition span is not the decorated span

Decorators are modeled by a separate wrapper:

```text
decorated_definition
  ├─ decorator+
  └─ definition: class_definition | function_definition
```

The `function_definition` or `class_definition` node itself begins at `def` / `class`; the parent `decorated_definition` includes the leading decorators. ([Python grammar source][python-grammar]) ([Python node types][python-node-types])

Example:

```python
@route("/users")
@cache(ttl=30)
async def list_users() -> list[User]:
    ...
```

If your symbol record's `range` should cover the whole decorated declaration, use the `decorated_definition` as the owner range while keeping the nested function/class node for name/signature fields.

Query:

```scheme
(decorated_definition
  (decorator)+ @definition.decorator
  definition: (function_definition
    name: (identifier) @definition.function.name) @definition.function) @definition.decorated

(decorated_definition
  (decorator)+ @definition.decorator
  definition: (class_definition
    name: (identifier) @definition.class.name) @definition.class) @definition.decorated
```

Do not treat every decorator as a plain direct call. The decorator child is an arbitrary Python expression; it can be an identifier, attribute, call, or more complex expression.

---

## 45.12 Classes and generic class syntax

The current `class_definition` fields are:

```text
name            -> identifier       required
superclasses    -> argument_list    optional
type_parameters -> type_parameter   optional
body            -> block            required
```

([Python node types][python-node-types])

Query:

```scheme
(class_definition
  name: (identifier) @definition.class.name
  type_parameters: (type_parameter)? @definition.class.type_parameters
  superclasses: (argument_list)? @definition.class.superclasses
  body: (block) @definition.class.body) @definition.class
```

Do not interpret every superclass expression as a statically resolvable class name. Python permits arbitrary expressions in the bases/keyword arguments used during class construction; syntax extraction can emit candidates, but semantic resolution is a later stage.

---

## 45.13 Parameters are a family of node shapes

`parameters` contains `parameter` supertypes rather than only identifiers. Current parameter subtypes include:

```text
identifier
default_parameter
typed_parameter
typed_default_parameter
list_splat_pattern
dictionary_splat_pattern
positional_separator
keyword_separator
tuple_pattern
```

([Python node types][python-node-types])

This covers shapes such as:

```python
def f(a, b=1, /, c: int = 2, *args, d, **kwargs):
    ...
```

Do not derive parameter names by collecting every descendant `identifier`: type annotations and default values also contain identifiers.

Recommended algorithm:

```text
for each direct named child of parameters:
  identifier                  -> binding name = child
  default_parameter           -> field name
  typed_default_parameter     -> field name
  typed_parameter             -> direct identifier/splat child, excluding field type
  list_splat_pattern          -> contained target identifier
  dictionary_splat_pattern    -> contained target identifier
  positional_separator       -> marker, no binding
  keyword_separator          -> marker, no binding
  tuple_pattern              -> recurse if supporting legacy destructuring grammar
```

Store parameter syntax separately from semantic position:

```rust
#[derive(Debug, Clone)]
pub enum PythonParameterKind {
    PositionalOnly,
    PositionalOrKeyword,
    VarArgs,
    KeywordOnly,
    VarKwargs,
}
```

The CST provides punctuation/marker structure; your normalization layer determines parameter-kind transitions around `/` and `*` markers.

---

## 45.14 Imports: preserve module, imported name, alias, and relative depth separately

The grammar distinguishes:

```text
import_statement
import_from_statement
future_import_statement
aliased_import
relative_import
import_prefix
wildcard_import
dotted_name
```

`import_from_statement` exposes `module_name`; both import forms expose repeated `name` fields, and an aliased import has `name` and `alias` fields. ([Python grammar source][python-grammar]) ([Python node types][python-node-types])

Examples:

```python
import os
import numpy as np
import package.submodule
from pathlib import Path
from . import local
from ..core import api as core_api
from package import *
from __future__ import annotations
```

Recommended normalized record:

```rust
#[derive(Debug, Clone)]
pub struct PythonImport {
    pub kind: PythonImportKind,
    pub module: Option<String>,
    pub relative_level: usize,
    pub imported_name: Option<String>,
    pub alias: Option<String>,
    pub wildcard: bool,
    pub byte_range: std::ops::Range<usize>,
}

#[derive(Debug, Clone, Copy)]
pub enum PythonImportKind {
    Import,
    FromImport,
    FutureImport,
}
```

Do **not** collapse these:

```python
import a.b
from a import b
```

They have different binding and module-loading semantics.

Do **not** resolve relative imports from syntax alone. You also need file/package context: repository path, package roots, namespace-package policy, and potentially environment/import-system configuration.

---

## 45.15 Calls, attributes, and syntactic callees

The Python grammar's `call` node has two required fields:

```text
function  -> primary_expression
arguments -> argument_list | generator_expression
```

`attribute` has:

```text
object    -> primary_expression
attribute -> identifier
```

([Python node types][python-node-types]) ([Python grammar source][python-grammar])

Direct-call query:

```scheme
(call
  function: (identifier) @call.direct.name
  arguments: (_) @call.arguments) @call.direct
```

Attribute-call query:

```scheme
(call
  function: (attribute
    object: (_) @call.receiver
    attribute: (identifier) @call.method)
  arguments: (_) @call.arguments) @call.attribute
```

Generic call query:

```scheme
(call
  function: (_) @call.function
  arguments: (_) @call.arguments) @call
```

The generic form is often the best indexing primitive because legal call targets include nested calls, subscriptions, parenthesized expressions, attributes, and other primary expressions.

Example shapes that must remain distinct:

```python
f()
obj.f()
factory()()
registry[key]()
(get_handler())()
```

Syntax can tell you the callee expression. It cannot prove which runtime object is called. Treat syntax-derived call edges as **candidates** until name/import/type resolution enriches them.

---

## 45.16 Assignments and binding targets

The grammar models normal and annotated assignment through `assignment`:

```text
left  -> pattern | pattern_list       required
type  -> type                         optional
right -> expression/...               optional
```

and augmented assignment through `augmented_assignment`. It separately models the walrus operator as `named_expression` with `name` and `value` fields. ([Python grammar source][python-grammar]) ([Python node types][python-node-types])

Examples:

```python
x = value
x: int
x: int = value
a, b = pair
obj.attr = value
items[i] = value
x += 1
if (n := read()):
    ...
```

Binding extraction must distinguish:

```text
identifier target     -> local/global/nonlocal binding candidate
attribute target      -> attribute mutation, not local-name binding
subscript target      -> item mutation, not local-name binding
pattern list          -> recurse through destructuring targets
named expression      -> binding candidate in expression context
```

Do not label every `identifier` under the `left` field as the same semantic operation without walking the target pattern.

---

## 45.17 Scope construction for Python

Tree-sitter gives structural containment, not Python's full symbol table. Build a first-pass scope tree from syntax, then apply Python binding rules.

Useful syntax owners:

```text
module
function_definition
class_definition
lambda
list_comprehension
set_comprehension
dictionary_comprehension
generator_expression
```

Important directives:

```text
global_statement   -> identifier+
nonlocal_statement -> identifier+
```

The grammar exposes both as dedicated nodes containing identifiers. ([Python node types][python-node-types])

Recommended two-phase scope build:

```text
Phase 1: structural scopes
  module/function/class/lambda/comprehension owners

Phase 2: bindings and directives
  parameters
  assignments
  for targets
  comprehension targets
  import bindings
  with-as targets
  except-as aliases
  match captures
  named expressions
  global declarations
  nonlocal declarations
```

Why two phases: a `global`/`nonlocal` declaration changes how another syntactic assignment target is classified, and class/comprehension scopes do not behave identically to ordinary nested functions.

For exact CPython-compatible symbol-table behavior, use a semantic Python analyzer/compiler layer rather than claiming Tree-sitter alone is authoritative.

---

## 45.18 Comprehensions and generator expressions create special binding contexts

The grammar has dedicated nodes:

```text
list_comprehension
dictionary_comprehension
set_comprehension
generator_expression
for_in_clause
if_clause
```

The first four contain a `body` field and one or more comprehension clauses. `for_in_clause` exposes `left` and `right` fields and can include anonymous `async`. ([Python grammar source][python-grammar]) ([Python node types][python-node-types])

Example:

```python
[x.name for x in users if x.active]
{k: transform(v) for k, v in items}
(value async for value in stream())
```

Queries:

```scheme
(for_in_clause
  left: (_) @comprehension.target
  right: (_) @comprehension.iterable) @comprehension.for
```

```scheme
[
  (list_comprehension)
  (set_comprehension)
  (dictionary_comprehension)
  (generator_expression)
] @comprehension
```

Do not flatten comprehension targets into the enclosing function's ordinary local-binding model without applying Python's comprehension-scope rules.

---

## 45.19 `match` / `case`: pattern identifiers can be bindings, not reads

The grammar contains `match_statement`, `case_clause`, and an extensive pattern hierarchy including:

```text
case_pattern
as_pattern
class_pattern
dict_pattern
keyword_pattern
list_pattern
tuple_pattern
splat_pattern
union_pattern
```

A `match_statement` exposes `subject` and `body`; each `case_clause` contains patterns, optional guard, and consequence. ([Python grammar source][python-grammar]) ([Python node types][python-node-types])

Python's language reference states that successful patterns can bind names, and that `match` and `case` are soft keywords. ([Python compound statements][python-compound])

Therefore this is wrong:

```scheme
(identifier) @reference
```

for a global reference index.

In:

```python
match event:
    case UserCreated(user_id=user_id):
        handle(user_id)
```

some identifiers in the pattern denote value/class references while others denote capture bindings. The Tree-sitter CST provides the pattern shape; your Python-specific binder must classify roles.

Recommended rule:

```text
never emit references from match-pattern identifiers using a generic identifier query;
walk/query each pattern kind and explicitly emit:
  value/class reference candidates
  capture bindings
  wildcard/non-binding forms
```

---

## 45.20 Strings, f-strings, interpolation, and format specifiers

The current grammar models strings through scanner-backed nodes including:

```text
string
string_start
string_content
string_end
escape_sequence
escape_interpolation
interpolation
format_specifier
format_expression
type_conversion
concatenated_string
```

An `interpolation` node exposes an `expression` field plus optional `format_specifier` and `type_conversion` fields. ([Python node types][python-node-types]) ([Python grammar source][python-grammar])

Query Python expressions embedded in f-strings:

```scheme
(interpolation
  expression: (_) @fstring.expression) @fstring.interpolation
```

Example:

```python
f"user={user.name!r:>20}"
```

Do not treat the entire `string` source range as opaque if call/reference extraction should include expressions inside f-strings.

Conversely, do not scan raw braces in string text yourself; the grammar's external scanner already handles formatted-string delimiter/interpolation state.

---

## 45.21 Docstrings are a semantic convention, not a dedicated grammar node

The grammar exposes strings and expression statements; it does not expose a `docstring` node. Therefore a docstring extractor must apply Python-specific placement rules on top of the CST.

Structural heuristic:

```text
owner = module | function_definition.body | class_definition.body
first effective statement in owner
  if expression_statement containing a constant string/concatenated string:
      docstring candidate
  else:
      no docstring
```

Example:

```python
class Service:
    """Public service API."""

    def run(self):
        """Execute one cycle."""
        ...
```

Do not query every `(string)` as a docstring.

Do not classify formatted strings with runtime interpolation as docstrings merely because they are the first string-shaped syntax node; if exact Python `__doc__` behavior matters, confirm with a Python semantic parser/compiler model.

---

## 45.22 Type annotations, PEP-695-style type parameters, and type aliases

The grammar currently has a dedicated `type` superstructure, typed parameters/default parameters, optional function return types, function/class `type_parameters`, and a `type_alias_statement`. The type-alias grammar shape is:

```text
"type" left:type "=" right:type
```

and `type` can represent expression-like, generic, union, constrained, member, and splat type forms. ([Python grammar source][python-grammar])

Python's current reference documents `type` as a soft keyword and the `type` statement as the type-alias syntax introduced in Python 3.12. ([Python simple statements][python-simple])

Practical extraction rule:

```text
keep annotation/type syntax in a separate edge namespace from runtime value references
```

For example:

```python
def load[T](x: T) -> list[T]: ...
type UserId = int
```

A type-aware code graph may want:

```text
symbol: load
binds type parameter: T
annotation reference: T
annotation reference: list
annotation reference: T
symbol: UserId (type alias)
type reference: int
```

Tree-sitter identifies syntax; whether annotations are evaluated, deferred, imported only under `TYPE_CHECKING`, or resolved to actual symbols requires semantic context.

---

## 45.23 Python-version policy: grammar acceptance is not a strict CPython-version validator

The upstream grammar explicitly references both Python 2 and Python 3 grammars, and its current rule set still contains compatibility constructs such as `print_statement` and `exec_statement` alongside modern constructs such as `match_statement`, type aliases/type parameters, and formatted-string interpolation. ([Python grammar source][python-grammar])

Therefore:

```text
Tree-sitter parse success != accepted by CPython 3.14
Tree-sitter parse error   != necessarily the same diagnostic CPython emits
```

Recommended product model:

```rust
#[derive(Debug, Clone, Copy)]
pub enum PythonSyntaxPolicy {
    TreeSitterGrammar,
    TargetVersion { major: u8, minor: u8 },
}
```

For a target-version-aware product:

```text
Tree-sitter layer:
  fast tolerant CST + incremental updates

version policy layer:
  reject/flag syntax unavailable in configured Python version

semantic/compiler layer when required:
  exact symbol/type/import/runtime semantics
```

Do not advertise Tree-sitter as a drop-in replacement for CPython's PEG parser if exact acceptance/error messages are a requirement.

---

## 45.24 Error recovery for Python editors

High-value incomplete states include:

```python
def f(

class C:

from package import (

value = f"{user.

match x:
    case {
```

Tree-sitter is designed to return useful trees around syntax errors. Python-specific consumers should preserve:

```text
ERROR nodes
MISSING nodes
nearest enclosing function/class/module
source byte range
parser-state information when completion is needed
```

Recommended extraction confidence:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxConfidence {
    Clean,
    RecoveredAncestor,
    ErrorNode,
    MissingNode,
}
```

A call/definition outside an error subtree can still be indexed even if another part of the same file is incomplete.

Do not make `root.has_error()` a file-wide indexing kill switch.

---

## 45.25 Completion-oriented parse-state use for Python

For syntactic completion, combine Python-specific context with Tree-sitter parse-state lookahead:

```text
cursor byte offset
  -> smallest containing node
  -> node.parse_state() / next_parse_state()
  -> language.lookahead_iterator(state)
  -> grammar-valid symbol candidates
  -> Python semantic filter
       scope bindings
       imports
       member/type info
       keywords allowed by product/version
```

Tree-sitter lookahead answers **what grammar symbols are valid**, not which Python name/member is semantically meaningful.

Examples:

```python
from pathlib import |
obj.|
def f(a: |
match value:
    case |
```

The semantic completion engine must still provide module exports, object members, types, pattern names, etc.

---

## 45.26 Incremental parsing: Python-specific edit hazards

Python edits with potentially nonlocal structural consequences include:

```text
leading indentation change
insertion/deletion of opening bracket
insertion/deletion of closing bracket
backslash line-continuation edit
triple-quote delimiter edit
f-string prefix/delimiter edit
f-string brace edit
decorator insertion/removal
colon insertion/removal after compound statement
```

These may alter scanner state or statement/block structure beyond the exact edited bytes.

Correct pipeline remains:

```rust
old_tree.edit(&edit);
let new_tree = parser.parse(new_source, Some(&old_tree)).unwrap();
let changed: Vec<_> = old_tree.changed_ranges(&new_tree).collect();
```

Then widen to semantic owners before updating a code index.

For Python, a good owner hierarchy is:

```text
function_definition
class_definition
decorated_definition
lambda
comprehension/generator owner
module
```

If a changed range crosses multiple owners or touches indentation at a shared block boundary, conservatively widen to the common enclosing class/function/module.

---

## 45.27 Python query pack: definitions, imports, calls, and structural owners

A practical starter query can combine several patterns:

```scheme
; classes
(class_definition
  name: (identifier) @definition.class.name) @definition.class

; functions, including nested functions/methods
(function_definition
  name: (identifier) @definition.function.name
  parameters: (parameters) @definition.function.parameters) @definition.function

; imports
(import_statement
  name: (_) @import.name) @import

(import_from_statement
  module_name: (_) @import.module
  name: (_) @import.name) @import.from

(import_from_statement
  module_name: (_) @import.module
  (wildcard_import) @import.wildcard) @import.from

(future_import_statement
  name: (_) @import.future.name) @import.future

; direct calls
(call
  function: (identifier) @call.direct.name) @call.direct

; attribute calls
(call
  function: (attribute
    object: (_) @call.receiver
    attribute: (identifier) @call.method)) @call.attribute

; assignment target owner
(assignment
  left: (_) @binding.target) @binding.assignment

; walrus target
(named_expression
  name: (_) @binding.named_expression) @binding.named_expression.owner

; comprehension / for target
(for_in_clause
  left: (_) @binding.for_target
  right: (_) @reference.iterable) @for_in
```

This query intentionally captures **structural candidates**. It does not attempt to label all `identifier`s as reads/references because Python binding contexts require a separate classifier.

Compile once:

```rust
use tree_sitter::{Language, Query};

let language: Language = tree_sitter_python::LANGUAGE.into();
let query = Query::new(&language, include_str!("python_index.scm"))?;
```

Compile the query at service startup and fail startup/test gates if the pinned Python grammar no longer accepts it.

---

## 45.28 Rust query execution helper for Python source

```rust
use tree_sitter::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

pub fn captures(source: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let language: Language = tree_sitter_python::LANGUAGE.into();

    let mut parser = Parser::new();
    parser.set_language(&language)?;
    let tree = parser.parse(source, None).ok_or("parse cancelled")?;

    let query = Query::new(
        &language,
        r#"
        (class_definition name: (identifier) @class.name) @class
        (function_definition name: (identifier) @function.name) @function
        (call function: (_) @call.function) @call
        "#,
    )?;

    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);

    while let Some(m) = matches.next() {
        for capture in m.captures {
            let name = &capture_names[capture.index as usize];
            let range = capture.node.byte_range();
            let text = std::str::from_utf8(&source[range.clone()])?;
            println!("{name}: {range:?}: {text}");
        }
    }

    Ok(())
}
```

Production changes:

```text
precompile Query
reuse QueryCursor per worker
range-limit reruns after incremental edits
materialize normalized records, not Node handles
attach file revision and capture provenance
cap result count and time
```

---

## 45.29 Upstream Python `TAGS_QUERY`: useful baseline, intentionally small

`tree-sitter-python` exports `TAGS_QUERY`. The upstream query currently tags:

```text
module-level assignment identifier -> definition.constant
class_definition name             -> definition.class
function_definition name          -> definition.function
call identifier/attribute name    -> reference.call
```

([Python tags query][python-tags])

Use it when a lightweight, interoperable tag stream is sufficient:

```rust
let tag_query_source = tree_sitter_python::TAGS_QUERY;
```

But for a code-property graph or richer Python index, it is incomplete by design. It does not by itself model:

```text
imports and aliases
method/class ownership
nested scope semantics
parameter bindings
global/nonlocal
comprehension bindings
match captures
attribute writes
annotation/type references
relative-import resolution
call target resolution
```

A robust system should treat `TAGS_QUERY` as a smoke-test/reference artifact, not as a complete Python semantic index.

---

## 45.30 Upstream Python highlighting query

The grammar also exports `HIGHLIGHTS_QUERY`. The current upstream query distinguishes broad categories such as variables, constructors/constants by naming convention, function/method calls, decorators, function definitions, properties, types, literals, comments, strings, interpolations, and operators. ([Python highlights query][python-highlights])

Example native Rust configuration:

```rust
use tree_sitter::Language;
use tree_sitter_highlight::HighlightConfiguration;

let language: Language = tree_sitter_python::LANGUAGE.into();

let mut config = HighlightConfiguration::new(
    language,
    "python",
    tree_sitter_python::HIGHLIGHTS_QUERY,
    "", // injections query if your app adds one
    "", // locals query if your app adds one
)?;
```

Do not repurpose highlight captures as semantic classification. For example, the highlight query may infer constructors/constants from naming convention; that is presentation policy, not proof of symbol kind.

---

## 45.31 `.py`, `.pyi`, generated Python, and notebooks

### `.py`

Normal Python module source. Full definition/import/call/scope extraction applies.

### `.pyi`

Python stub files are syntactically Python-like and can be parsed with the same grammar, but their **semantic role is different**. In an index, retain a file-kind flag:

```rust
#[derive(Debug, Clone, Copy)]
pub enum PythonFileKind {
    Source,
    Stub,
    Generated,
}
```

Typical stub handling may prioritize signatures/types and suppress assumptions based on executable bodies.

### Generated Python

Generated code can be extremely large or repetitive. Apply the large-file and result-amplification controls from §§37/42 rather than special-casing the parser.

### Jupyter notebooks

A `.ipynb` file is JSON, not Python source. Do not pass the whole notebook to `tree-sitter-python`. Parse the notebook container separately, extract code-cell source, and parse each code cell with Python while maintaining an outer coordinate/provenance mapping.

```text
.ipynb JSON
  -> cell extraction
  -> code cell source
  -> Python parser
  -> cell-local CST
  -> notebook/cell provenance mapping
```

Do not pretend byte offsets inside a reconstructed code-cell string are byte offsets in the raw notebook JSON.

---

## 45.32 Python-specific semantic limitations

Tree-sitter can robustly extract syntax such as:

```text
module/class/function structure
parameter syntax
decorators
imports
assignment targets
calls/attributes/subscripts
control-flow constructs
comprehensions
patterns
annotations
strings/f-string expressions
ERROR/MISSING recovery structure
```

It does not by itself determine:

```text
which import path resolves in the active environment
whether star import introduces a particular name
which object an identifier refers to
which implementation an attribute call dispatches to
MRO / descriptors / metaclass effects
monkey-patched attributes
runtime-generated modules/classes/functions
exact inferred type
whether code is reachable
whether a syntactically accepted construct is valid for target Python version
full CPython symbol-table behavior
```

Recommended layering:

```text
Tree-sitter Python CST
  -> structural symbol/binding/reference candidates
  -> Python scope/import resolver
  -> type checker / language server / compiler facts
  -> enriched code graph
```

Do not force semantic responsibilities into increasingly complex Tree-sitter queries. Queries are structural pattern matchers, not an interpreter or type checker.

---

## 45.33 Python code-intelligence extraction architecture

Recommended normalized pipeline:

```text
source revision N
  │
  ├─ parse with tree-sitter-python
  │    └─ Tree(module)
  │
  ├─ structural query pack
  │    ├─ definitions
  │    ├─ imports
  │    ├─ calls
  │    ├─ binding targets
  │    ├─ global/nonlocal
  │    ├─ decorators
  │    ├─ annotations
  │    └─ match/comprehension contexts
  │
  ├─ CST-context classifier
  │    ├─ binding vs read
  │    ├─ owner scope
  │    ├─ method vs nested function
  │    ├─ decorated owner range
  │    └─ syntactic callee shape
  │
  ├─ module/package resolver
  │    └─ import edge candidates
  │
  ├─ semantic enrichment
  │    └─ type checker / language server / compiler index
  │
  └─ atomic file-revision commit
```

Suggested record provenance:

```rust
#[derive(Debug, Clone)]
pub struct PythonFactProvenance {
    pub grammar: &'static str,
    pub grammar_version: &'static str,
    pub query_pack_version: u32,
    pub file_revision: u64,
    pub capture_name: String,
    pub byte_range: std::ops::Range<usize>,
}
```

This makes grammar/query upgrades auditable and allows stale file-revision results to be discarded safely.

---

## 45.34 Performance and worker design for Python repositories

Python-specific performance defaults:

```text
Parser:
  one per worker
  set Python language once
  reuse across files

Query:
  compile Python query packs once
  share immutable Query values where permitted

QueryCursor:
  reuse per worker
  set byte ranges for incremental reruns

Trees:
  retain only active/recent revisions needed for incremental parse

Source:
  keep immutable bytes paired with each Tree revision
```

High-cardinality query hazards in Python include:

```text
(identifier) @x                    across entire repository
(_) @node                           wildcard captures
nested attribute/call queries      on generated code
all string/interpolation captures  on data-heavy source files
all assignments                    without owner/range restriction
```

For a continuously updating index:

```text
edit -> incremental parse -> changed_ranges -> owner widening -> local queries
```

is preferable to rerunning the full Python query pack over the entire file after each keystroke.

---

## 45.35 Python test corpus

A production Python parser/indexer should include syntax fixtures covering at least:

```text
ordinary def / async def
function annotations and return types
generic function/class/type alias syntax
decorators and decorator calls
methods, nested functions, nested classes
positional-only and keyword-only parameters
*args / **kwargs
default and typed-default parameters
import / import as / from import / relative / wildcard / __future__
assignments, annotated assignments, augmented assignment, walrus
for / async for
with / async with
try / except / except* / finally
lambda
list/set/dict comprehensions and generators
match/case patterns and guards
f-strings, conversion flags, format specifiers, nested expressions
triple-quoted/raw/bytes strings
comments and line continuations
multiline bracketed expressions
incomplete indentation
incomplete function/class/import/f-string/pattern syntax
ERROR and MISSING-node cases
very large generated Python file
UTF-8 identifiers
source with encoding declaration if supported by product policy
.py and .pyi files
```

Fresh-vs-incremental equivalence test:

```text
for each edit sequence:
  old_tree.edit(edit)
  incremental = parse(new_source, Some(old_tree))
  fresh       = parse(new_source, None)
  assert normalized extraction(incremental) == normalized extraction(fresh)
```

Do not require internal node IDs or exact recovery-node placement to be identical if semantic normalized output is the actual contract.

---

## 45.36 Useful Python-specific development commands

Application build/test:

```bash
cargo check --workspace
cargo test --workspace
cargo build --release
```

Dependency inspection:

```bash
cargo tree -i tree-sitter
cargo tree -i tree-sitter-python
cargo tree -d
```

Upstream grammar smoke tests:

```bash
git clone https://github.com/tree-sitter/tree-sitter-python.git
cd tree-sitter-python

tree-sitter test
tree-sitter parse /path/to/fixture.py
tree-sitter query queries/tags.scm /path/to/fixture.py
tree-sitter query --captures queries/highlights.scm /path/to/fixture.py
```

Custom query test:

```bash
tree-sitter query \
  --grammar-path /path/to/tree-sitter-python \
  --captures \
  queries/python_index.scm \
  tests/fixtures/python/**/*.py
```

Parser regeneration is needed only when developing/forking the grammar itself:

```bash
cd /path/to/tree-sitter-python
tree-sitter generate
tree-sitter test
```

`tree-sitter generate` emits the parser artifacts from `grammar.js`; it is **not** a normal application build step for a project depending on the published `tree-sitter-python` crate. ([Tree-sitter generate CLI][cli-generate])

---

## 45.37 Upgrade contract for `tree-sitter-python`

A Python grammar upgrade can change:

```text
node kinds
field names
supertype/subtype relationships
recovery structure
scanner state behavior
string/f-string shape
new-language-feature representation
query compatibility
highlight/tag captures
changed-range behavior
performance
```

Upgrade gate:

```text
[ ] bump tree-sitter-python in isolated branch
[ ] cargo tree / lockfile review
[ ] Parser::set_language ABI smoke test
[ ] compile every static Python Query
[ ] parse golden Python fixture corpus
[ ] compare normalized symbol/import/call/scope output
[ ] run fresh-vs-incremental equivalence tests
[ ] inspect ERROR/MISSING fixtures
[ ] inspect modern-syntax fixtures
[ ] inspect .pyi fixtures
[ ] benchmark representative large Python files
[ ] review upstream NODE_TYPES diff
[ ] review upstream tags/highlights query diff
[ ] canary on real repository sample before full rollout
```

For a code-property graph, the most important compatibility artifact is usually the **normalized extractor output**, not the raw S-expression.

---

## 45.38 Python-specific anti-pattern inventory

* Using the Python `py-tree-sitter` API examples in Rust code.
* Depending on `tree-sitter-rust` when the goal is to parse Python source.
* Treating `tree-sitter-python` as the runtime instead of a grammar package.
* Running `tree-sitter parse` as a subprocess for every production file instead of linking the grammar crate.
* Reconstructing blocks from leading spaces rather than trusting the Python grammar/scanner.
* Stripping or reindenting source before parsing without remapping positions.
* Ignoring Python encoding cookies while claiming original-byte offsets are preserved.
* Treating every `identifier` as a reference.
* Treating every assignment-target identifier as a local binding without `global`/`nonlocal` handling.
* Treating attribute/subscript assignment as a local-name binding.
* Treating match-pattern identifiers as ordinary reads.
* Flattening comprehension targets into the enclosing scope without Python scope rules.
* Treating function node range as including decorators.
* Treating every string as a docstring.
* Treating f-string content as opaque when embedded expressions matter.
* Deriving parameter names by collecting all descendant identifiers.
* Collapsing `import a.b` and `from a import b` into the same record.
* Resolving relative imports without package/file-system context.
* Treating syntactic calls as resolved runtime call graph edges.
* Treating successful Tree-sitter parsing as proof that the file is valid for one exact CPython version.
* Dropping the entire file from the index because one subtree contains an error.
* Rerunning all Python queries over the whole file after every edit.
* Ignoring scanner-sensitive changes such as indentation, brackets, and string delimiters.
* Using highlight naming heuristics as semantic symbol facts.
* Assuming `.ipynb` is Python source instead of a JSON container with Python code cells.

---

## 45.39 Python implementation checklist

```text
Dependencies
[ ] tree-sitter runtime pinned
[ ] tree-sitter-python grammar pinned
[ ] C build toolchain available
[ ] grammar ABI validated at startup/test

Input
[ ] .py / .pyi file classification
[ ] explicit source-encoding policy
[ ] original-vs-decoded coordinate mapping if non-UTF-8 supported
[ ] source bytes retained with Tree revision

Parsing
[ ] language set once per parser worker
[ ] Parser reused
[ ] ERROR/MISSING tolerated and recorded
[ ] indentation/scanner behavior not reimplemented externally

Definitions
[ ] function_definition fields used
[ ] async token handled explicitly if needed
[ ] type parameters / return annotations retained
[ ] class fields used
[ ] decorated_definition range widening handled
[ ] docstring heuristic separated from generic string extraction

Bindings/scopes
[ ] parameters normalized by parameter-node kind
[ ] assignments recursively classify target patterns
[ ] named_expression handled
[ ] import bindings handled
[ ] for/comprehension targets handled
[ ] global/nonlocal applied
[ ] match/case capture bindings handled
[ ] class/function/lambda/comprehension scope rules separated

Imports
[ ] import / from-import / future-import separated
[ ] aliased imports preserved
[ ] relative level preserved
[ ] wildcard imports represented explicitly
[ ] semantic module resolution performed outside Tree-sitter

Calls/references
[ ] direct and attribute calls captured
[ ] arbitrary callee expression retained
[ ] f-string embedded expressions queried
[ ] syntactic call candidates not mislabeled as resolved edges
[ ] generic identifier query not used as final reference index

Incremental updates
[ ] InputEdit exact
[ ] old Tree edited
[ ] incremental reparse uses old Tree
[ ] changed_ranges used
[ ] ranges widened to Python semantic owners
[ ] scanner-sensitive edits included in tests
[ ] stale revision results suppressed

Queries
[ ] static Python queries compiled at startup/test
[ ] QueryCursor reused
[ ] byte ranges used for incremental reruns
[ ] match/result/time limits applied
[ ] upstream TAGS_QUERY/HIGHLIGHTS_QUERY treated as optional baselines

Version policy
[ ] target Python version recorded if product cares
[ ] Tree-sitter acceptance not treated as exact CPython validation
[ ] modern syntax fixtures present
[ ] grammar NODE_TYPES/query diffs reviewed on upgrade

Testing
[ ] .py fixtures
[ ] .pyi fixtures
[ ] decorators
[ ] async syntax
[ ] imports
[ ] parameters
[ ] match/case
[ ] comprehensions
[ ] type aliases/type parameters
[ ] f-strings
[ ] indentation errors
[ ] incomplete editing states
[ ] fresh-vs-incremental equivalence
[ ] performance regression corpus
```


# Source reference anchors

The links below are the primary source set used for the version-sensitive and API-specific claims in this reference. Prefer the version-pinned docs.rs page matching the deployed crate when signatures differ from `latest`.

[rustdoc]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/
[getting-started]: https://tree-sitter.github.io/tree-sitter/using-parsers/1-getting-started.html
[advanced-parsing]: https://tree-sitter.github.io/tree-sitter/using-parsers/3-advanced-parsing.html
[language-version]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/constant.LANGUAGE_VERSION.html
[min-language-version]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/constant.MIN_COMPATIBLE_LANGUAGE_VERSION.html
[parser]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/struct.Parser.html
[parse-options]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/struct.ParseOptions.html
[language]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/struct.Language.html
[language-fn]: https://docs.rs/tree-sitter-language/0.1.7/tree_sitter_language/struct.LanguageFn.html
[tree]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/struct.Tree.html
[node]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/struct.Node.html
[tree-cursor]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/struct.TreeCursor.html
[input-edit]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/struct.InputEdit.html
[lookahead]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/struct.LookaheadIterator.html
[query]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/struct.Query.html
[query-cursor]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/struct.QueryCursor.html
[query-cursor-options]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/struct.QueryCursorOptions.html
[query-match]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/struct.QueryMatch.html
[query-capture]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/struct.QueryCapture.html
[capture-quantifier]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/enum.CaptureQuantifier.html
[text-provider]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/trait.TextProvider.html
[query-error]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/struct.QueryError.html
[query-syntax]: https://tree-sitter.github.io/tree-sitter/using-parsers/queries/1-syntax.html
[query-operators]: https://tree-sitter.github.io/tree-sitter/using-parsers/queries/2-operators.html
[query-predicates]: https://tree-sitter.github.io/tree-sitter/using-parsers/queries/3-predicates-and-directives.html
[query-api]: https://tree-sitter.github.io/tree-sitter/using-parsers/queries/4-api.html
[static-node-types]: https://tree-sitter.github.io/tree-sitter/using-parsers/6-static-node-types.html
[json-crate]: https://docs.rs/tree-sitter-json/0.24.8/tree_sitter_json/
[grammar-init]: https://tree-sitter.github.io/tree-sitter/cli/init.html
[grammar-generate]: https://tree-sitter.github.io/tree-sitter/cli/generate.html
[grammar-dsl]: https://tree-sitter.github.io/tree-sitter/creating-parsers/2-the-grammar-dsl.html
[grammar-writing]: https://tree-sitter.github.io/tree-sitter/creating-parsers/3-writing-the-grammar.html
[external-scanners]: https://tree-sitter.github.io/tree-sitter/creating-parsers/4-external-scanners.html
[grammar-tests]: https://tree-sitter.github.io/tree-sitter/creating-parsers/5-writing-tests.html
[highlight]: https://docs.rs/tree-sitter-highlight/0.26.12/tree_sitter_highlight/
[tags]: https://docs.rs/tree-sitter-tags/0.26.12/tree_sitter_tags/
[wasm-store]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/struct.WasmStore.html
[wasm-ffi]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/ffi/fn.ts_wasm_store_load_language.html
[wasm-error]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/struct.WasmError.html
[allocator]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/fn.set_allocator.html
[allocator-ffi]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/ffi/fn.ts_set_allocator.html
[binding-source]: https://docs.rs/tree-sitter/0.26.12/src/tree_sitter/lib.rs.html
[ffi-source]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/ffi/
[tree-ffi]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/ffi/
[decode-source]: https://docs.rs/tree-sitter/0.26.12/tree_sitter/trait.Decode.html
[query-source]: https://docs.rs/tree-sitter/0.26.12/src/tree_sitter/lib.rs.html

[python-crate]: https://docs.rs/tree-sitter-python/0.25.0/tree_sitter_python/
[python-cargo]: https://raw.githubusercontent.com/tree-sitter/tree-sitter-python/master/Cargo.toml
[python-grammar-repo]: https://github.com/tree-sitter/tree-sitter-python
[python-grammar]: https://raw.githubusercontent.com/tree-sitter/tree-sitter-python/master/grammar.js
[python-scanner]: https://raw.githubusercontent.com/tree-sitter/tree-sitter-python/master/src/scanner.c
[python-node-types]: https://raw.githubusercontent.com/tree-sitter/tree-sitter-python/master/src/node-types.json
[python-tags]: https://raw.githubusercontent.com/tree-sitter/tree-sitter-python/master/queries/tags.scm
[python-highlights]: https://raw.githubusercontent.com/tree-sitter/tree-sitter-python/master/queries/highlights.scm
[python-lexical]: https://docs.python.org/3/reference/lexical_analysis.html
[python-compound]: https://docs.python.org/3/reference/compound_stmts.html
[python-simple]: https://docs.python.org/3/reference/simple_stmts.html
[grammar-getting-started]: https://tree-sitter.github.io/tree-sitter/creating-parsers/1-getting-started.html
[cli-parse]: https://tree-sitter.github.io/tree-sitter/cli/parse.html
[cli-query]: https://tree-sitter.github.io/tree-sitter/cli/query.html
[cli-generate]: https://tree-sitter.github.io/tree-sitter/cli/generate.html
