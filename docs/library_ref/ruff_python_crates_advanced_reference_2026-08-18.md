# Ruff Python Internal Crates in Rust — Advanced Technical Reference for LLM Coding Agents

> **Version anchor:** Ruff **0.16.1** / internal component crates **0.0.7** / Rust edition **2024** / Ruff workspace MSRV **1.95**.  
> **Snapshot date:** 2026-08-18.  
> **Audience:** LLM coding agents and engineers embedding Ruff's Rust-native Python syntax, source, semantic, code-generation, and formatting infrastructure in code-intelligence systems, refactoring tools, linters, indexers, and transformation services.

This document follows the same operating style as the supplied DataFusion advanced reference: pin the deployable version first; establish a mental model; inventory the capability surface; show canonical Rust patterns; make cross-crate contracts explicit; and end every major subsystem with deployment rules, failure modes, anti-patterns, and agent checklists.

---

## Source-of-truth hierarchy

Use sources in this order when implementing against these crates:

1. **The exact Ruff tag used by the workspace** (`0.16.1` for this document).
2. **The crate's generated docs.rs documentation for the same `0.0.x` component version.**
3. Ruff's official repository source and Ruff formatter/parser documentation.
4. This document as an architectural and implementation reference.

Do **not** treat an example copied from Ruff `main` as compile-compatible with `0.0.7`. Ruff explicitly publishes these crates as **internal components whose Rust APIs are unstable and will have frequent breaking changes**.

Canonical upstream anchors:

- Ruff workspace `0.16.1`: https://github.com/astral-sh/ruff/tree/0.16.1
- Ruff crate versioning policy: https://docs.astral.sh/ruff/versioning/
- `ruff_python_parser`: https://docs.rs/ruff_python_parser/0.0.7/ruff_python_parser/
- `ruff_python_ast`: https://docs.rs/ruff_python_ast/0.0.7/ruff_python_ast/
- `ruff_python_semantic`: https://docs.rs/ruff_python_semantic/0.0.7/ruff_python_semantic/
- `ruff_python_trivia`: https://docs.rs/ruff_python_trivia/0.0.7/ruff_python_trivia/
- `ruff_python_index`: https://docs.rs/ruff_python_index/0.0.7/ruff_python_index/
- `ruff_python_codegen`: https://docs.rs/ruff_python_codegen/0.0.7/ruff_python_codegen/
- `ruff_python_formatter`: https://docs.rs/ruff_python_formatter/0.0.7/ruff_python_formatter/
- `ruff_source_file`: https://docs.rs/ruff_source_file/0.0.7/ruff_source_file/
- Ruff formatter design/user documentation: https://github.com/astral-sh/ruff/blob/0.16.1/docs/formatter.md

---

# Feature inventory: what this reference covers

The eight-crate capability surface breaks naturally into the following responsibilities:

| Crate | Primary responsibility | Persistable output? | Owns semantics? | Preserves source trivia? |
|---|---|---:|---:|---:|
| `ruff_source_file` | Source text, lines, line endings, byte-offset ↔ line/column conversion | Yes, if you serialize your own representation | No | Source text itself |
| `ruff_python_parser` | Lexing + Python parsing into Ruff AST; tokens and parse diagnostics | AST/tokens can be consumed; API is unstable | Syntax only | Tokens retain lexical coverage; AST does not own trivia |
| `ruff_python_ast` | Typed Python AST node model, visitors, transformers, helpers, names | Yes with selected optional features, but not a stable wire contract | Syntactic structure | No CST-style whitespace ownership |
| `ruff_python_trivia` | Comments, lightweight tokenization, whitespace/indentation, pragmas, text wrapping | Usually recomputed | No | Yes, via source ranges/utilities |
| `ruff_python_index` | Fast per-file index of lexical facts omitted from AST | Usually recomputed | No | Indexes comments, multiline/interpolated strings, continuations |
| `ruff_python_semantic` | Ruff linter-style scopes, bindings, definitions, references, imports, branch/context state | Possible only through your own normalized layer | **Name/binding semantics**, not type checking | No |
| `ruff_python_codegen` | Unparse AST nodes/suites to valid Python source | Output is source text | No | **No exact round-trip guarantee**; code is regenerated |
| `ruff_python_formatter` | Ruff/Black-style pretty printing with comment attachment and formatting options | Output is formatted source | No | Reads trivia/comments but intentionally normalizes formatting |

The most important architectural fact is that these crates are **not a single LibCST-like lossless object model**. Ruff deliberately separates:

```text
source text
   │
   ├─ byte/line model -------------------------- ruff_source_file
   │
   ├─ lexer + parser --------------------------- ruff_python_parser
   │      ├─ tokens
   │      └─ typed AST ------------------------- ruff_python_ast
   │
   ├─ source-side lexical/trivia queries ------- ruff_python_trivia
   │
   ├─ token-derived omitted-fact index --------- ruff_python_index
   │
   ├─ scope/binding/reference semantics -------- ruff_python_semantic
   │
   ├─ AST -> Python source --------------------- ruff_python_codegen
   │
   └─ AST + source + trivia -> formatted source  ruff_python_formatter
```

That decomposition is highly suitable for a code-property-graph or indexing service, but it means an agent must choose the correct representation rather than expecting one tree to answer every question.

---

# Proposed comprehensive documentation map

0. Scope, versioning, and mental model  
1. Installation, crate selection, and workspace layout  
2. First executable stack: parse → inspect → source locations  
3. `ruff_source_file` deep dive  
4. `ruff_python_parser` deep dive  
5. `ruff_python_ast` deep dive  
6. `ruff_python_trivia` deep dive  
7. `ruff_python_index` deep dive  
8. `ruff_python_semantic` deep dive  
9. `ruff_python_codegen` deep dive  
10. `ruff_python_formatter` deep dive  
11. Cross-crate ownership and lifetime model  
12. Source-preserving edits vs AST regeneration  
13. Building a LibCST-replacement analysis stack  
14. Code-intelligence / CPG deployment pattern  
15. Incrementality and cache boundaries  
16. Error tolerance and partially invalid Python  
17. Performance and allocation strategy  
18. Concurrency and service deployment  
19. Testing and golden/snapshot strategy  
20. Upgrade/version migration discipline  
21. Security and untrusted-source considerations  
22. Capability decision tables  
23. Global anti-pattern inventory  
24. LLM-agent implementation checklist  
25. Reference links

---

# 0) Scope, versioning, and mental model

## 0.0 Version anchors

For Ruff `0.16.1`, the Ruff workspace pins the eight component crates to `0.0.7`:

```toml
[workspace.dependencies]
ruff_python_parser = { version = "0.0.7" }
ruff_python_ast = { version = "0.0.7" }
ruff_python_semantic = { version = "0.0.7" }
ruff_python_trivia = { version = "0.0.7" }
ruff_python_index = { version = "0.0.7" }
ruff_python_codegen = { version = "0.0.7" }
ruff_python_formatter = { version = "0.0.7" }
ruff_source_file = { version = "0.0.7" }
```

The workspace uses Rust edition 2024 and declares Rust `1.95` as its minimum toolchain for this release.

### Agent invariant — release-train pinning

```text
Treat 0.0.7 as one coherent release train.
Do not mix ruff_python_ast 0.0.6 with ruff_python_parser 0.0.7.
Do not copy signatures from main without checking tag 0.16.1.
Do not depend on caret-version upgrades for these crates in a production code-intel service.
```

Recommended exact pins:

```toml
[workspace.dependencies]
ruff_python_parser = "=0.0.7"
ruff_python_ast = "=0.0.7"
ruff_python_semantic = "=0.0.7"
ruff_python_trivia = "=0.0.7"
ruff_python_index = "=0.0.7"
ruff_python_codegen = "=0.0.7"
ruff_python_formatter = "=0.0.7"
ruff_source_file = "=0.0.7"
ruff_text_size = "=0.0.7"
```

`ruff_text_size` is not one of the eight requested crates, but it is part of nearly every cross-crate API because Ruff uses `TextSize` and `TextRange` as the canonical byte-offset coordinate system.

---

## 0.1 What the stack is / is not

### It is

- A Rust-native Python lexer/parser and typed AST.
- A source-coordinate and newline model.
- A strong set of lexical/trivia helpers.
- A Ruff-specific per-file semantic model for scopes, bindings, references, imports, and qualified-name resolution.
- An AST transformer infrastructure.
- An AST unparser/code generator.
- A production-grade Python pretty printer/formatter.

### It is not

- A stable public SDK promised by Astral.
- A single lossless CST comparable to LibCST's ownership model.
- An incremental parser in the Tree-sitter sense.
- A cross-project type checker.
- A substitute for Pyrefly/ty/Pyright semantic type inference.
- A persistent CPG or repository index.
- A source-preserving codemod system by default.

### Agent rule

Never write:

> “`ruff_python_semantic` gives us Python typing.”

Correct phrasing:

> “`ruff_python_semantic` gives us Ruff's linter-oriented lexical/semantic model: scopes, bindings, definitions, imports, references, execution contexts, and qualified-name resolution. Type inference remains a separate layer.”

---

## 0.2 Canonical analysis pipeline

```text
UTF-8 source string
  │
  ├─► ruff_source_file::LineIndex
  │      byte offsets ↔ line/column/LSP position
  │
  └─► ruff_python_parser
         │
         ├─► Parsed<ModModule>
         │      ├─ syntax() / suite()     -> ruff_python_ast
         │      ├─ tokens()               -> lexical token stream
         │      ├─ errors()               -> ParseError[]
         │      └─ unsupported_syntax_errors()
         │
         ├─► TriviaRanges / CommentRanges -> ruff_python_trivia
         │
         ├─► Indexer::from_tokens         -> ruff_python_index
         │
         └─► AST traversal
                ├─ your structural facts
                ├─ Ruff semantic-model population / queries
                └─ Pyrefly query daemon enrichment
```

Transformation choices branch later:

```text
AST transform
   │
   ├─ exact/minimal edit desired
   │      └─ compute TextRange edits against original source
   │
   ├─ regenerated Python fragment desired
   │      └─ ruff_python_codegen::Generator
   │
   └─ canonical formatted file desired
          └─ ruff_python_formatter::format_module_source / format_module_ast
```

---

## 0.3 Minimum vocabulary

| Term | Meaning | Agent use |
|---|---|---|
| `TextSize` | Byte offset/length in Ruff's text model | Stable in-memory coordinate currency |
| `TextRange` | Half-open byte range `[start, end)` | Persist ranges only with content/version identity |
| `LineIndex` | Precomputed line-start/index data | Convert offsets to user/LSP locations efficiently |
| `Parsed<T>` | Parser output containing syntax, tokens, errors | Prefer over immediately discarding token/error data |
| `ModModule` | Parsed module AST root | Normal `.py` / `.pyi` root |
| `Suite` | Sequence of `Stmt` | Module/function/class bodies |
| `Stmt` / `Expr` | Sum types over Python statements/expressions | Primary structural dispatch |
| `AnyNodeRef` | Erased/reference view across AST node kinds | Generic node tooling |
| `Visitor` | Evaluation-order recursive AST visitor | Read-only traversal |
| `Transformer` | Mutable/consuming AST transform support | Structural rewrites |
| `TriviaRanges` | Source ranges for trivia relevant to formatter/comment handling | Preserve/query source-side facts |
| `CommentRanges` | Sorted non-overlapping comment ranges | Cheap comment lookup |
| `Indexer` | Token-derived index for omitted lexical facts | One-time per parse/file version |
| `SemanticModel` | Ruff linter semantic state/query surface | Resolve bindings/import-qualified names/scopes |
| `Generator` | AST-to-source unparser | Generate code after structural mutation |
| `PyFormatOptions` | Per-file formatter configuration | Canonical formatting contract |

---

## 0.4 The coordinate invariant

Ruff AST nodes and lexical structures use byte ranges. The source string used to interpret a `TextRange` must be the **exact text version from which the AST/tokens/index were produced**.

```text
(source_revision, TextRange) = meaningful location
TextRange alone              = unsafe persistent identity
```

For a continuously updating graph, persist something like:

```rust
struct SourceSpan {
    file_id: FileId,
    revision: ContentHash,
    start: u32,
    end: u32,
}
```

Do not retain ranges across edits without translating or invalidating them.

---

## 0.5 Separation from Pyrefly

Recommended responsibility split:

```text
Ruff parser/AST/trivia/index
  -> what syntax exists, where it exists, lexical context

ruff_python_semantic
  -> local scope/binding/import/reference identity useful to lint-style analysis

Pyrefly query daemon
  -> inferred types, cross-module definition/type resolution, type-driven call targets

CPG / persistent graph
  -> normalized durable entities + edges + provenance + revision identity
```

Use Ruff semantics where they are cheaper or more precise for syntax-local facts, but do not duplicate Pyrefly's type system into the ingestion layer.

---

# 1) Installation, crate selection, and Rust project layout

## 1.0 Minimal coherent dependency set

For analysis-only use:

```toml
[dependencies]
ruff_python_parser = "=0.0.7"
ruff_python_ast = "=0.0.7"
ruff_python_trivia = "=0.0.7"
ruff_python_index = "=0.0.7"
ruff_source_file = "=0.0.7"
ruff_text_size = "=0.0.7"
```

Add Ruff semantic analysis:

```toml
ruff_python_semantic = "=0.0.7"
```

Add AST regeneration:

```toml
ruff_python_codegen = "=0.0.7"
```

Add canonical Ruff formatting:

```toml
ruff_python_formatter = "=0.0.7"
```

The formatter is a materially heavier dependency boundary than the parser/AST/trivia/source stack because it also depends on Ruff's generic formatter, database layer, macros, Salsa, and formatting support crates.

---

## 1.1 Workspace pinning

Recommended:

```toml
[workspace]
resolver = "2"
members = [
  "crates/python-frontend",
  "crates/code-intel-core",
  "crates/code-intel-daemon",
]

[workspace.dependencies]
ruff_python_parser = "=0.0.7"
ruff_python_ast = "=0.0.7"
ruff_python_semantic = "=0.0.7"
ruff_python_trivia = "=0.0.7"
ruff_python_index = "=0.0.7"
ruff_python_codegen = "=0.0.7"
ruff_python_formatter = "=0.0.7"
ruff_source_file = "=0.0.7"
ruff_text_size = "=0.0.7"
```

Downstream internal crates should use `workspace = true` rather than repeating version strings.

---

## 1.2 Optional features

### `ruff_python_ast`

`0.0.7` exposes optional features including:

```text
schemars
cache
serde
get-size
salsa
```

Use them deliberately:

- `serde`: serialization support for applicable AST structures and supporting types.
- `schemars`: schema generation; pulls JSON/schema dependencies.
- `get-size`: memory accounting support.
- `salsa`: integration hooks used by Ruff's incremental infrastructure.
- `cache`: Ruff cache-related derivations/integration.

Do not enable every feature preemptively in a latency-sensitive daemon.

### `ruff_source_file`

```text
serde
get-size
```

### `ruff_python_formatter`

```text
default = ["serde"]
serde
schemars
```

If you only call formatter APIs inside a controlled binary and do not need config serialization, consider whether disabling defaults is worthwhile—but only after compile/integration tests against the exact target version.

The parser, trivia, index, codegen, and semantic crates do not expose broad user-oriented feature surfaces in `0.0.7` comparable to the AST/formatter/source crates.

---

## 1.3 Recommended facade crate

Because these APIs are unstable, isolate them behind **your own stable adapter**:

```text
crates/python-frontend/
  src/
    lib.rs
    parse.rs
    ast.rs
    lexical.rs
    semantic.rs
    locations.rs
    edits.rs
    format.rs
    version.rs
```

`lib.rs` should expose your own domain types, not Ruff internals everywhere:

```rust
pub struct ParsedPythonFile {
    // private Ruff-native representation
}

pub struct PythonSpan {
    pub start: u32,
    pub end: u32,
}

pub enum PythonParseStatus {
    Valid,
    RecoveredWithErrors,
    UnsupportedForTargetVersion,
}
```

This reduces a Ruff component upgrade from a repository-wide migration to an adapter-crate migration.

---

## 1.4 Build and CI commands

```bash
cargo check --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo tree -d
```

Upgrade-specific checks:

```bash
cargo tree -i ruff_python_ast
cargo tree -i ruff_python_parser
cargo tree -i ruff_text_size
```

Agent rule:

```text
One Ruff internal release train per binary.
If cargo tree shows 0.0.6 and 0.0.7 simultaneously across shared AST/TextRange boundaries,
stop and reconcile versions before debugging type mismatches.
```

---

## 1.5 Project layout for a code-intelligence daemon

```text
code-intel/
  crates/
    python-frontend/
      parse.rs           # parser entry points + parse diagnostics
      ast_extract.rs     # AST -> normalized syntax facts
      lexical.rs         # trivia + Indexer
      semantic.rs        # optional Ruff semantic adapter
      source.rs          # LineIndex / line-column mappings
      edits.rs           # source-range edit planner
      regenerate.rs      # codegen for isolated fragments
      format.rs          # optional formatter boundary

    graph-core/
      schema.rs
      facts.rs
      deltas.rs
      provenance.rs

    pyrefly-client/
      query.rs
      types.rs
      symbols.rs

    daemon/
      watcher.rs
      scheduler.rs
      cache.rs
      reconcile.rs
```

Do not make the CPG schema depend directly on `ruff_python_ast::Expr` or `BindingId`; those IDs and enum shapes are implementation details of a particular Ruff release.

---

# 2) First executable stack: parse → inspect → source locations

## 2.0 Minimal parse program

```rust
use ruff_python_parser::parse_module;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
import os

def greet(name: str) -> str:
    return f"Hello, {name}"
"#;

    let parsed = parse_module(source)?;

    println!("statements={}", parsed.suite().len());
    println!("tokens={}", parsed.tokens().len());

    Ok(())
}
```

`parse_module` is the normal strict convenience entry point for a full module. It returns `Result<Parsed<ModModule>, ParseError>`.

---

## 2.1 Keep the complete `Parsed` object

Do not immediately call `into_syntax()` if downstream analysis needs comments, lexical facts, target-version diagnostics, or source-preserving edits.

```rust
let parsed = parse_module(source)?;

let module = parsed.syntax();
let tokens = parsed.tokens();
let parse_errors = parsed.errors();
let version_errors = parsed.unsupported_syntax_errors();
```

The parser deliberately exposes syntax and token data together.

---

## 2.2 Build trivia and lexical index once

```rust
use ruff_python_index::Indexer;
use ruff_python_trivia::TriviaRanges;

let parsed = ruff_python_parser::parse_module(source)?;
let trivia = TriviaRanges::from(parsed.tokens());
let indexer = Indexer::from_tokens(parsed.tokens(), source);

println!("comments={}", indexer.comment_ranges().len());
```

Treat these as **per-source-revision derived values**.

---

## 2.3 Convert AST byte ranges to user positions

```rust
use ruff_source_file::{LineIndex, SourceCode};
use ruff_text_size::Ranged;

let index = LineIndex::from_source_text(source);
let source_code = SourceCode::new(source, &index);

for stmt in parsed.suite() {
    let start = source_code.line_column(stmt.start());
    let end = source_code.line_column(stmt.end());
    println!("{start} .. {end}");
}
```

Persist byte ranges in graph provenance; compute line/column representations at API/render time unless you have a measured reason to duplicate both.

---

## 2.4 First-agent checklist

```text
[ ] Pin all Ruff component crates to the same 0.0.x.
[ ] Parse with ruff_python_parser.
[ ] Retain Parsed<T>, not only the AST, if lexical/source-aware work follows.
[ ] Treat TextRange as UTF-8 byte offsets tied to a file revision.
[ ] Build TriviaRanges / Indexer once per parsed revision.
[ ] Build LineIndex once per source revision and reuse it.
[ ] Normalize Ruff AST/semantic facts into your own durable graph schema.
```


---

# 3) `ruff_source_file` — source coordinates, lines, and newline semantics

## 3.0 Identity

`ruff_source_file` is the coordinate-conversion and source-line utility layer. It does not parse Python. Its job is to make byte-oriented syntax ranges usable for diagnostics, APIs, editor/LSP positions, and line-aware transformations.

Core public surface in `0.0.7`:

```text
SourceCode
SourceFile
SourceFileBuilder
LineIndex
LineColumn
SourceLocation
SourceRow
OneIndexed
LineEnding
PositionEncoding
Line
LineRanges
UniversalNewlines
UniversalNewlineIterator
NewlineWithTrailingNewline
find_newline
```

---

## 3.1 `LineIndex`

### Role

`LineIndex` precomputes information needed for efficient offset-to-line/column conversion.

Canonical creation:

```rust
use ruff_source_file::LineIndex;

let index = LineIndex::from_source_text(source);
```

Do this once per source revision, not for every diagnostic.

### Deployment pattern

```rust
struct FileAnalysis {
    source: Arc<str>,
    line_index: LineIndex,
    // parsed AST / tokens / lexical indexes ...
}
```

The `LineIndex` should have exactly the same lifetime/revision as the source string it indexes.

---

## 3.2 `SourceCode`

`SourceCode<'src, 'index>` pairs a source string with a `LineIndex` and exposes convenient source queries:

```rust
let source_code = SourceCode::new(source, &index);

let line_col = source_code.line_column(offset);
let location = source_code.source_location(offset, PositionEncoding::Utf16);
let line = source_code.line_index(offset);
let start = source_code.line_start(line);
let end = source_code.line_end(line);
let range = source_code.line_range(line);
let text = source_code.line_text(line);
let slice = source_code.slice(node);
```

### Agent distinction

```text
TextRange / TextSize       -> byte-offset source coordinates used by Ruff AST/tokens
LineColumn                 -> one-indexed human-oriented line/column
SourceLocation             -> line + character offset under a chosen position encoding
PositionEncoding           -> editor/API coordinate convention
```

Do not conflate Unicode scalar columns with UTF-16 LSP character positions.

---

## 3.3 `PositionEncoding`

Use `PositionEncoding` when converting byte offsets for an external protocol. In an editor/LSP deployment, negotiate or deliberately choose the encoding expected by the client; UTF-16 remains common in LSP ecosystems.

Pattern:

```rust
let location = source_code.source_location(
    node.start(),
    PositionEncoding::Utf16,
);
```

Do **not** calculate LSP positions by counting Rust `char`s manually.

---

## 3.4 `SourceFile` and `SourceFileBuilder`

`SourceFile` packages a name, source text, and lazily initialized line index. It is cheap to clone because the backing representation is `Arc`-owned.

```rust
use ruff_source_file::SourceFileBuilder;

let file = SourceFileBuilder::new("example.py", source)
    .finish();

let code = file.source_text();
let index = file.index();
let source_code = file.to_source_code();
```

If you already computed a line index:

```rust
let file = SourceFileBuilder::new("example.py", source)
    .line_index(index)
    .finish();
```

### When to use

Use `SourceFile` when an in-process API benefits from carrying source name + contents + coordinate index together.

### When not to use

Do not use Ruff `SourceFile` as your durable repository-file identity. Your graph should own a stable file key/path identity and revision/hash independently of Ruff's internal representation.

---

## 3.5 Universal newline handling

Python source can contain LF, CRLF, or lone CR physical line endings. Ruff exposes:

```text
LineEnding
UniversalNewlines
UniversalNewlineIterator
NewlineWithTrailingNewline
find_newline
```

Use these rather than `str::lines()` when preserving Python-compatible physical-line semantics matters.

Example:

```rust
use ruff_source_file::UniversalNewlines;

for line in source.universal_newlines() {
    // inspect line content while handling LF / CRLF / CR correctly
}
```

### Agent invariant

Never assume `\n` is the only line terminator when calculating source edits or diagnostic line boundaries.

---

## 3.6 `LineRanges`

`LineRanges` extends source strings with byte-range-aware line helpers and is used throughout Ruff's lexical/indexing code.

Typical use cases:

- locate the start of the physical line containing an offset;
- determine line range/end;
- reason about continuation lines;
- slice line-local source without repeatedly scanning from byte zero.

This trait is especially useful in edit generation and lexical analysis because the AST itself does not contain physical-line objects.

---

## 3.7 Coordinate conversion policy for a CPG

Recommended persisted representation:

```text
file_id
content_revision / content_hash
start_byte
end_byte
```

Recommended derived API representation:

```text
line
column
LSP start/end
source snippet
```

Why:

1. Ruff's AST/tokens natively use byte offsets.
2. Byte offsets make slicing exact and cheap.
3. Line/column conventions can differ by encoding.
4. Recomputing user positions from a cached `LineIndex` is cheap.
5. Duplicating byte + line/column data introduces consistency risk after edits.

---

## 3.8 Error boundaries

`TextRange` slicing assumes ranges are valid for the source text used. A range from revision N against revision N+1 can:

- select the wrong source;
- land inside a UTF-8 code point;
- panic in direct indexing paths;
- silently corrupt diagnostic/edit semantics.

### Guardrail

Tie every per-file frontend bundle to an immutable source revision:

```rust
struct PythonFrontendSnapshot {
    revision: FileRevision,
    source: Arc<str>,
    line_index: LineIndex,
    // Parsed, Indexer, extracted facts ...
}
```

Replace the entire bundle on content change.

---

## 3.9 Feature flags

`ruff_source_file 0.0.7`:

```text
serde    -> serde + ruff_text_size/serde
get-size -> get-size2 + ruff_text_size/get-size
```

Use `serde` only if you truly serialize Ruff-native coordinate helper types. A more stable system boundary is usually your own schema containing primitive `u32` byte offsets.

---

## 3.10 Anti-pattern inventory

- Rebuilding `LineIndex` for every diagnostic.
- Persisting `LineColumn` but discarding exact byte spans.
- Persisting a `TextRange` without file revision identity.
- Treating column count as UTF-16 automatically.
- Using `str::lines()` where lone CR handling matters.
- Recomputing line starts with ad-hoc `rfind('\n')` everywhere.
- Making Ruff `SourceFile` the repository's global identity type.
- Serializing Ruff internal types across service boundaries without an explicit compatibility policy.

---

## 3.11 Agent checklist

```text
[ ] Keep byte ranges as canonical source coordinates.
[ ] Build one LineIndex per immutable source revision.
[ ] Use SourceCode for repeated line/column/slice operations.
[ ] Choose PositionEncoding explicitly for editor/LSP output.
[ ] Use UniversalNewlines for Python physical-line semantics.
[ ] Invalidate LineIndex whenever source text changes.
[ ] Persist your own file/revision identity around Ruff ranges.
```

---

# 4) `ruff_python_parser` — lexer, parser, tokens, and recoverable parse state

## 4.0 Identity and parser architecture

`ruff_python_parser` is Ruff's hand-written Python parser. It combines lexical analysis with recursive-descent parsing and Pratt parsing for expression precedence. The public output is a typed Ruff AST plus token stream and parse diagnostics.

Key public entry points in `0.0.7`:

```text
parse_module
parse_expression
parse_expression_range
parse_parenthesized_expression_range
parse_string_annotation
parse
parse_unchecked
parse_unchecked_source
parse_cells_unchecked
lexer::lex
Parsed<T>
ParseOptions
Mode
ParseError
UnsupportedSyntaxError
```

---

## 4.1 `parse_module`

Default strict full-module parse:

```rust
use ruff_python_parser::parse_module;

let parsed = parse_module(source)?;
let module = parsed.syntax();
let suite = parsed.suite();
```

Signature shape:

```rust
pub fn parse_module(source: &str)
    -> Result<Parsed<ModModule>, ParseError>
```

Use for valid/expected-valid `.py` or module text when first-error failure is appropriate.

---

## 4.2 `parse_expression`

For isolated Python expressions:

```rust
use ruff_python_parser::parse_expression;

let parsed = parse_expression("foo.bar(x + 1)")?;
let expr = parsed.expr();
```

Useful for:

- validated expression fields in configuration;
- annotation fragments;
- template/query sublanguages that embed Python expressions;
- unit tests for expression extractors.

---

## 4.3 Range-based expression parsing

`parse_expression_range(source, range)` parses a source subrange while retaining coordinates relative to the original source prefix.

```rust
use ruff_python_parser::parse_expression_range;
use ruff_text_size::{TextRange, TextSize};

let range = TextRange::new(TextSize::new(5), TextSize::new(7));
let parsed = parse_expression_range("11 + 22 + 33", range)?;
```

Use range-aware parsing rather than slicing to a new string when **absolute source offsets must remain meaningful**.

---

## 4.4 Parenthesized-expression mode

`parse_parenthesized_expression_range` allows syntax that would be valid inside parentheses even if the source fragment itself lacks literal parentheses.

This is useful for multiline annotations and similar contexts.

```rust
let parsed = parse_parenthesized_expression_range(source, annotation_range)?;
```

Agent rule:

```text
Do not "fix" a multiline annotation fragment by injecting synthetic parentheses into the source
if the parser already exposes ParenthesizedExpression mode.
```

---

## 4.5 String annotations

`parse_string_annotation(source, string_literal)` extracts and parses the contents of a `StringLiteral`, accounting for opener/closer length and triple-quoted behavior.

Use this for old-style / forward-reference annotations such as:

```python
x: "pkg.Type | None"
```

rather than manually stripping quotes and losing source coordinates.

---

## 4.6 General `parse` + `ParseOptions`

```rust
use ruff_python_parser::{parse, Mode, ParseOptions};

let parsed = parse(
    source,
    ParseOptions::from(Mode::Module),
)?;
```

`Mode` values:

```text
Module
Expression
ParenthesizedExpression
Ipython
```

IPython mode recognizes supported escape-command syntax relevant to notebooks/interactivity.

---

## 4.7 `parse_unchecked`: critical for code intelligence

`parse_unchecked` differs from strict `parse`: it returns `Parsed<Mod>` **without converting parser errors into an immediate `Err`**.

```rust
let parsed = ruff_python_parser::parse_unchecked(
    source,
    ParseOptions::from(Mode::Module),
);

for error in parsed.errors() {
    report(error);
}

// Depending on recovery, syntax/tokens remain available for analysis.
let syntax = parsed.syntax();
```

### Why this matters for an editor/daemon

Files are frequently invalid while being edited:

```python
def foo(
    x: list[
```

A code-intelligence system should not throw away the entire previous graph merely because the current snapshot has a transient parse error.

Recommended policy:

```text
valid parse
  -> publish high-confidence fresh syntax facts

recoverable parse with errors
  -> extract only facts whose ranges/nodes are trustworthy
  -> mark file syntax state degraded
  -> preserve unaffected prior semantic facts where appropriate

catastrophic/unusable parse
  -> retain previous stable graph facts with stale/dirty marker
  -> retry on next edit
```

---

## 4.8 `parse_unchecked_source`

```rust
use ruff_python_ast::PySourceType;
use ruff_python_parser::parse_unchecked_source;

let parsed = parse_unchecked_source(source, PySourceType::Python);
```

`PySourceType` maps `.py` / `.pyi` / notebook semantics to parser options and always yields a module-form parse.

Use this when your file-type layer already knows the Python source type.

---

## 4.9 Notebook/cell parsing

`parse_cells_unchecked` parses ordered, non-overlapping source ranges as independent modules and merges the result while preserving offsets into the combined source. Ruff uses this style for notebook cell semantics, where cells are independently syntactic but later cells can reference earlier definitions.

Conceptual use:

```rust
let parsed = parse_cells_unchecked(
    concatenated_source,
    cell_ranges,
    &options,
);
```

This is substantially safer than pretending an entire notebook is one ordinary physical Python module.

---

## 4.10 `Parsed<T>` contract

A `Parsed<T>` contains:

```text
syntax: T
tokens: Tokens
errors: Vec<ParseError>
unsupported_syntax_errors: Vec<UnsupportedSyntaxError>
```

Important methods:

```rust
parsed.syntax()
parsed.tokens()
parsed.errors()
parsed.unsupported_syntax_errors()
parsed.into_syntax()
parsed.into_errors()
parsed.has_valid_syntax()
parsed.has_invalid_syntax()
parsed.has_no_syntax_errors()
parsed.has_syntax_errors()
parsed.as_result()
```

For module output:

```rust
parsed.suite()
parsed.into_suite()
```

For expression output:

```rust
parsed.expr()
parsed.expr_mut()
parsed.into_expr()
```

### Important distinction

`has_valid_syntax()` concerns parser errors; `has_no_syntax_errors()` also accounts for target-version `UnsupportedSyntaxError`s.

An agent must not silently equate the two.

---

## 4.11 Tokens

The token stream is critical for capabilities the AST does not own:

- comments;
- exact lexical token boundaries;
- quote/string token details;
- non-logical newlines;
- continuation-line detection;
- source-side indexing;
- formatter comment attachment/trivia derivation.

Keep tokens as part of the per-revision frontend snapshot if any lexical or source-preserving operation follows parsing.

### Agent rule

```text
AST-only analysis is enough for structural facts.
AST + tokens + original source is the baseline for source-aware analysis.
```

---

## 4.12 Lexer-only use

The public `lexer` module allows tokenization when a full AST is unnecessary.

Potential uses:

- fast lexical scanning;
- token-based heuristics;
- source instrumentation;
- debugging parser discrepancies.

However, for ordinary file ingestion, prefer one parse and reuse `Parsed::tokens()` rather than lexing and parsing separately.

---

## 4.13 Parse errors vs unsupported syntax

Maintain separate states:

```text
ParseError
  -> source violates Python grammar / parser reports syntactic issue

UnsupportedSyntaxError
  -> syntax exists but is invalid for configured target Python version/context
```

This allows a system to report:

> “This file parses as modern Python but violates the project's configured target version.”

instead of flattening everything into “invalid Python.”

---

## 4.14 Parser is not incremental

The Ruff parser is fast, but its public API is whole-source parsing. It does not provide Tree-sitter-style edit operations that mutate an existing parse tree in response to a byte-range edit.

For a daemon:

```text
filesystem/editor edit
  -> determine changed file
  -> whole-file Ruff reparse (fast, authoritative AST)
  -> diff extracted facts/ranges against prior snapshot
```

If sub-file incremental parsing is a hard requirement, use Tree-sitter as an edit-local front layer and Ruff as a validation/deep-extraction layer.

---

## 4.15 Parser recursion and hostile input

Parsing arbitrary source is substantially safer than executing it, but a service accepting untrusted files should still:

- cap file size;
- enforce request/task deadlines;
- avoid recursive post-parse visitors without depth discipline;
- keep the daemon isolated from source execution/import side effects;
- fuzz custom extraction layers;
- handle parse errors as data, not panics.

Never use `.unwrap()` on user-controlled parse operations in a server path.

---

## 4.16 Canonical service wrapper

```rust
use ruff_python_ast::PySourceType;
use ruff_python_parser::{Parsed, parse_unchecked_source};

pub fn parse_for_indexing(
    source: &str,
    source_type: PySourceType,
) -> Parsed<ruff_python_ast::ModModule> {
    parse_unchecked_source(source, source_type)
}
```

Keep policy outside the parser wrapper:

```rust
let parsed = parse_for_indexing(source, source_type);

let quality = if parsed.errors().is_empty() {
    ParseQuality::Clean
} else {
    ParseQuality::Recovered
};
```

---

## 4.17 Anti-pattern inventory

- Calling `parse_module(...).unwrap()` on editor/user content.
- Discarding tokens immediately after parsing.
- Treating unsupported-target-version syntax as identical to grammar failure.
- Re-lexing source after a parse just to obtain comments.
- Parsing a substring with `parse_expression` when absolute offsets are required.
- Injecting synthetic parentheses instead of using parenthesized-expression mode.
- Assuming Ruff parsing is incrementally editable.
- Deleting all graph facts on a transient parse error.
- Parsing notebook concatenation as if cell boundaries do not matter.

---

## 4.18 Agent checklist

```text
[ ] Use parse_module for strict valid-module workflows.
[ ] Use parse_unchecked_source for editor/indexing workflows that must survive invalid syntax.
[ ] Preserve Parsed<T> until token/trivia/index extraction is complete.
[ ] Distinguish ParseError from UnsupportedSyntaxError.
[ ] Use range-aware expression parsing when absolute source offsets matter.
[ ] Use parse_string_annotation for stringified annotations.
[ ] Use parse_cells_unchecked for notebook-style independent cells when appropriate.
[ ] Treat whole-file reparse as the Ruff parser's normal incremental boundary.
```

---

# 5) `ruff_python_ast` — typed Python syntax model, traversal, and transformation

## 5.0 Identity

`ruff_python_ast` defines Ruff's strongly typed Python AST and a large body of utilities layered around it. It is the primary structural representation produced by `ruff_python_parser`.

Major public module families in `0.0.7` include:

```text
comparable
docstrings
find_node
helpers
identifier
name
operator_precedence
parenthesize
relocate
script
statement_visitor
stmt_if
str
str_prefix
token
traversal
types
visitor
whitespace
```

The crate also re-exports generated node types, expression helpers, integer helpers, node indexing, operator precedence, and Python-version types.

---

## 5.1 Root and node families

The normal structural hierarchy is:

```text
Mod
  ├─ ModModule
  │    └─ Suite = Vec<Stmt>-like statement sequence
  └─ ModExpression
       └─ Expr

Stmt
  ├─ FunctionDef
  ├─ ClassDef
  ├─ Return
  ├─ Delete
  ├─ TypeAlias
  ├─ Assign / AnnAssign / AugAssign
  ├─ For / While / If
  ├─ With
  ├─ Match
  ├─ Raise / Try / Assert
  ├─ Import / ImportFrom
  ├─ Global / Nonlocal
  └─ ...

Expr
  ├─ Name / Attribute
  ├─ Call
  ├─ Constant / string families
  ├─ BinOp / BoolOp / UnaryOp / Compare
  ├─ Lambda
  ├─ IfExp
  ├─ Dict / Set / List / Tuple
  ├─ comprehensions
  ├─ Await / Yield
  ├─ Subscript / Slice
  ├─ NamedExpr
  └─ ...

Pattern
  └─ Python structural-pattern-matching nodes

TypeParam
  └─ modern generic parameter syntax
```

Do not persist enum discriminants or assume node layouts are stable across component releases.

---

## 5.2 `PySourceType`

`PySourceType` distinguishes:

```text
Python  -> .py / .pyw
Stub    -> .pyi
Ipynb   -> notebook Python source
```

Helpers include:

```rust
PySourceType::from_extension(...)
PySourceType::try_from_extension(...)
PySourceType::try_from_path(...)
source_type.is_py_file()
source_type.is_stub()
source_type.is_py_file_or_stub()
source_type.is_ipynb()
```

Use this instead of scattered extension string checks throughout the frontend.

---

## 5.3 Python versions

The AST crate exposes Ruff's Python-version representation. Use the exact Ruff type for parser/formatter configuration inside the adapter crate, but normalize to your own stable enum at service/config boundaries if long-term API stability matters.

Version-sensitive syntax matters for:

- parser unsupported-syntax diagnostics;
- formatter target-version choices;
- builtins in Ruff semantic analysis;
- syntax generation decisions.

---

## 5.4 Ranges and `Ranged`

AST nodes implement/use Ruff's range conventions through `ruff_text_size::Ranged`.

```rust
use ruff_text_size::Ranged;

let range = stmt.range();
let start = stmt.start();
let end = stmt.end();
let raw = &source[range];
```

### Agent invariant

Use the AST node range to point at original source; do not reconstruct the source spelling from AST fields if the task is only to inspect or minimally edit the original text.

---

## 5.5 Read-only `Visitor`

`ruff_python_ast::visitor::Visitor` recursively visits nodes in **evaluation order**.

```rust
use ruff_python_ast::{Expr, Stmt};
use ruff_python_ast::visitor::{self, Visitor};

struct CallCollector<'a> {
    calls: Vec<&'a Expr>,
}

impl<'a> Visitor<'a> for CallCollector<'a> {
    fn visit_expr(&mut self, expr: &'a Expr) {
        if matches!(expr, Expr::Call(_)) {
            self.calls.push(expr);
        }
        visitor::walk_expr(self, expr);
    }
}
```

Important: call the relevant `walk_*` helper from overridden methods unless you intentionally want to prune traversal.

---

## 5.6 Evaluation order vs source order

The general `Visitor` uses evaluation-order semantics. If your extractor needs source-order traversal, use the source-order visitor utilities instead.

Decision rule:

```text
call/dataflow-oriented analysis -> evaluation-order Visitor can be useful
source outline / textual ordering -> source-order visitor
statements only                 -> StatementVisitor
custom exact semantics          -> manual traversal
```

Do not assume “visitor order” always means byte-position order.

---

## 5.7 `StatementVisitor`

For collectors that need only statements, use the specialized statement visitor rather than paying attention to every expression.

Examples:

- function/class outline;
- imports;
- assignments;
- control-flow statement inventory;
- return/raise collectors.

This communicates intent and can reduce accidental expression-level work.

---

## 5.8 Transformer

`ruff_python_ast::visitor::transformer::Transformer` is the AST-mutation route.

Conceptual pattern:

```rust
use ruff_python_ast::visitor::transformer::Transformer;

struct MyTransformer;

// Implement the appropriate transformation hooks,
// then run over the module/suite according to the 0.0.7 transformer API.
```

### Critical caveat

A transformed Ruff AST does **not** automatically preserve the exact original whitespace/comment placement. After a structural transform you must choose:

1. minimal source edits derived from node ranges, or
2. `ruff_python_codegen` regeneration, or
3. `ruff_python_formatter` canonical formatting.

The Transformer is an AST operation, not a LibCST codemod guarantee.

---

## 5.9 `AnyNodeRef`

Use `AnyNodeRef` when writing generic code that must operate across many AST node kinds without defining a giant application-specific enum.

Typical uses:

- comment association;
- generic parent maps;
- source-range indexing;
- node-kind instrumentation;
- formatter integration.

Do not persist `AnyNodeRef`; it is an in-process borrowed view.

---

## 5.10 Node indices

The AST exposes node-index infrastructure including `AtomicNodeIndex`. Treat these as **snapshot-local implementation IDs**, not durable semantic identities.

For a CPG, derive stable-ish identity from domain semantics plus provenance:

```text
file identity
node kind
qualified lexical path / enclosing scope
source range
content hash / structural digest
```

not from Ruff's node index alone.

---

## 5.11 `name` and qualified names

The `name` module contains name representations used heavily by semantic analysis, including qualified-name concepts.

Use Ruff name utilities when performing in-process matching:

```text
os.path.join
collections.abc.Iterable
typing.Optional
```

But normalize external graph data to your own string/segment representation rather than serializing Ruff internal name types.

---

## 5.12 Helpers

`ruff_python_ast::helpers` is a high-value utility module. Before writing custom traversal logic, search it for existing operations such as:

- callable/decorator normalization;
- return-statement collection;
- import/path utilities;
- expression mapping helpers;
- string/annotation helpers;
- statement/body inspection.

Agent rule:

```text
Before adding a bespoke 30-line AST helper, search:
  ruff_python_ast::helpers
  ruff_python_ast::traversal
  ruff_python_ast::visitor
  ruff_python_semantic::analyze
```

This reduces correctness drift from Python syntax corner cases.

---

## 5.13 `find_node`

Use node-finding utilities for source-position-based navigation when the exact API matches your need. This is preferable to recursively scanning the entire AST for every cursor query.

For repeated IDE-style queries, however, build your own per-file interval/range index once rather than doing a fresh AST walk for each cursor position.

---

## 5.14 `docstrings`

Docstring helpers centralize Python docstring recognition and extraction semantics. Use them rather than assuming:

```text
first statement == string literal == docstring
```

in every context.

Docstring analysis is useful for:

- symbol documentation;
- code search embeddings;
- API extraction;
- lint/refactor tools;
- formatter/docstring-code integration.

---

## 5.15 `operator_precedence` and parenthesization

AST regeneration must preserve Python semantics under precedence. Ruff exposes operator precedence and parenthesization helpers for this reason.

Do not implement ad-hoc rules such as:

```text
"add parentheses around every BinOp"
```

when generating transformed expressions. Let Ruff's generator or existing precedence helpers handle syntactic correctness.

---

## 5.16 `relocate`

Relocation utilities are relevant when moving/copying AST nodes and their ranges. Be careful: changing a range value does not magically make the source text at the new range correspond to the node.

Use relocation primarily for internally constructed/generated AST workflows where coordinate semantics are deliberately managed.

For minimal source edits, favor source range edits over mutating AST coordinates.

---

## 5.17 Tokens live in AST crate namespace

Ruff's token types are defined/exposed via `ruff_python_ast::token`, while the parser produces them.

That means the AST crate is not purely “tree nodes”; it also hosts the lexical token model used across parser, trivia, index, and formatter code.

This is one reason the whole Ruff internal release train must remain version-aligned.

---

## 5.18 String model

Modern Python strings are not one trivial constant node. Ruff models string flags/prefixes and interpolated-string structure needed for f-strings and newer syntax.

Agents must avoid assumptions such as:

```text
all string literals are Expr::Constant(str)
f-string contents are one opaque string
quote choice is semantically irrelevant to code generation
```

Use the AST's string types and helper modules (`str`, `str_prefix`) rather than raw regex parsing whenever the source is actual Python syntax.

---

## 5.19 Comparable AST

The `comparable` module provides an equivalent object hierarchy with stronger equality/hash-oriented behavior for structural comparison use cases.

Potential applications:

- detect structurally equivalent expressions;
- deduplicate normalized syntax patterns;
- compare before/after transforms ignoring non-semantic object identity.

Do not assume equality in a comparable AST equals semantic program equivalence; it is structural.

---

## 5.20 AST feature flags

`ruff_python_ast 0.0.7`:

```toml
[features]
schemars = ["dep:schemars", "dep:serde_json"]
cache = ["dep:ruff_cache", "dep:ruff_macros"]
serde = [
  "dep:serde",
  "ruff_text_size/serde",
  "dep:ruff_cache",
  "char_str/serde",
  "compact_str/serde",
  "thin-vec/serde",
]
get-size = ["dep:get-size2", "char_str/get-size", "ruff_text_size/get-size"]
salsa = ["dep:salsa"]
```

### Deployment rule

For a CPG, **normalize AST facts instead of serializing the entire Ruff AST as your canonical database**. Serialization support is useful for caches/debugging but the AST schema is explicitly unstable.

---

## 5.21 AST extraction pattern

```rust
use ruff_python_ast::{Expr, Stmt};
use ruff_python_ast::visitor::{self, Visitor};
use ruff_text_size::Ranged;

struct Extractor<'a> {
    source: &'a str,
    facts: Vec<SyntaxFact>,
}

impl<'a> Visitor<'a> for Extractor<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::FunctionDef(function) => {
                self.facts.push(SyntaxFact::Function {
                    name: function.name.to_string(),
                    range: to_span(function.range()),
                });
            }
            Stmt::ClassDef(class) => {
                self.facts.push(SyntaxFact::Class {
                    name: class.name.to_string(),
                    range: to_span(class.range()),
                });
            }
            _ => {}
        }

        visitor::walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Call(call) = expr {
            self.facts.push(SyntaxFact::CallSite {
                range: to_span(call.range()),
            });
        }
        visitor::walk_expr(self, expr);
    }
}
```

Keep your `SyntaxFact` stable and small; let Ruff AST remain an ephemeral frontend representation.

---

## 5.22 Parent maps

The AST does not require every node to own an explicit parent pointer. If your analysis needs parent relationships repeatedly, construct them once during traversal:

```text
child range/node index -> parent range/node kind
```

For a CPG, parent/contains edges naturally belong in the normalized graph and need not be queried from AST state after extraction.

---

## 5.23 Anti-pattern inventory

- Persisting Ruff AST enums directly as the long-term graph schema.
- Treating `AtomicNodeIndex` as a stable repository-wide ID.
- Using evaluation-order Visitor when source order is required.
- Overriding `visit_*` and forgetting to recurse via `walk_*`.
- Writing a custom Transformer and expecting comments/formatting to survive automatically.
- Regex-parsing Python strings/decorators when AST helpers already exist.
- Reimplementing Python operator precedence in a source generator.
- Assuming `.py`, `.pyi`, and notebook source behave identically.
- Assuming AST structure contains every lexical fact needed for source-preserving edits.

---

## 5.24 Agent checklist

```text
[ ] Use Stmt / Expr / Pattern as structural dispatch roots.
[ ] Use Ranged/TextRange for original-source provenance.
[ ] Choose evaluation-order, source-order, or statement-only visitor deliberately.
[ ] Call walk_* after custom visitor logic unless pruning is intentional.
[ ] Search AST helpers before implementing custom syntax logic.
[ ] Use Transformer only for AST mutation, not as a promise of source preservation.
[ ] Normalize AST facts into your own stable domain model.
[ ] Treat node indices and Ruff internal enums as snapshot/version-local.
[ ] Keep token/trivia/source layers available when exact source behavior matters.
```


---

# 6) `ruff_python_trivia` deep dive

## 6.0 Identity: source-side facts that the AST does not own

`ruff_python_trivia` is Ruff's lightweight source-analysis layer for information that is either absent from the AST or awkward to recover by walking AST nodes alone. It provides token-derived comment/parenthesis indexes, simple forward/backward tokenization, whitespace and indentation predicates, pragma/suppression helpers, source-content checks around ranges, and text wrapping/indentation utilities.

The crate should be understood as:

```text
original source + token ranges
    -> cheap lexical/source queries
    -> no full parse-tree ownership
    -> no name resolution
    -> no type information
```

Its role in a code-intelligence frontend is complementary to `ruff_python_ast`:

| Question | Preferred layer |
|---|---|
| “Is this node a call expression?” | `ruff_python_ast` |
| “Does this node's source region contain a comment?” | `ruff_python_trivia::CommentRanges` |
| “Is there non-whitespace before this statement on its line?” | `ruff_python_trivia::has_leading_content` |
| “What is the indentation before this offset?” | trivia/source helpers |
| “Is this comment a pragma/suppression?” | trivia pragma helpers |
| “What tokens exist around this exact source range?” | `SimpleTokenizer` / `BackwardsTokenizer` |
| “Which binding does this name refer to?” | `ruff_python_semantic` / Pyrefly |

---

## 6.1 Crate surface

The `0.0.7` crate exports:

```rust
pub use comment_ranges::{CommentRanges, ParenthesizedExpressions, TriviaRanges};
pub use comments::*;
pub use cursor::*;
pub use pragmas::*;
pub use tokenizer::*;
pub use whitespace::*;
pub mod textwrap;
```

The important conceptual groups are:

```text
comment_ranges
  TriviaRanges
  CommentRanges
  ParenthesizedExpressions

comments
  own-line vs end-of-line/comment positioning helpers

cursor
  low-level source cursor helpers

pragmas
  pragma/suppression recognition

tokenizer
  SimpleTokenizer
  BackwardsTokenizer
  SimpleToken / SimpleTokenKind

whitespace
  PythonWhitespace
  indentation/content predicates

textwrap
  dedent / dedent_to / indent / indent_first_line
```

---

## 6.2 `TriviaRanges`: combined token-derived source indexes

`TriviaRanges` groups the range indexes used by Ruff's formatter/comment-placement code:

```rust
pub struct TriviaRanges {
    comments: CommentRanges,
    parenthesized_expressions: ParenthesizedExpressions,
}
```

Public query surface:

```rust
let trivia = TriviaRanges::from(parsed.tokens());

let comments = trivia.comments();
let parens = trivia.parenthesized();
```

A canonical parse path:

```rust
use ruff_python_parser::parse_module;
use ruff_python_trivia::TriviaRanges;

fn build_trivia(source: &str) -> Result<TriviaRanges, ruff_python_parser::ParseError> {
    let parsed = parse_module(source)?;
    Ok(TriviaRanges::from(parsed.tokens()))
}
```

### Why keep `TriviaRanges` rather than only `CommentRanges`

The formatter needs more than comments: whether an expression was parenthesized can affect source-aware formatting decisions. If your downstream tooling may eventually use formatter APIs, retain/build `TriviaRanges`; if your only need is comment membership, `CommentRanges` alone is sufficient.

---

## 6.3 `CommentRanges`

`CommentRanges` stores comment `TextRange`s sorted by start offset, with no overlapping ranges. That ordering enables binary-search/partition-point operations instead of rescanning source for every AST node.

### Construction

You normally obtain it from token-derived infrastructure rather than manually constructing ranges:

```rust
let parsed = ruff_python_parser::parse_module(source)?;
let trivia = ruff_python_trivia::TriviaRanges::from(parsed.tokens());
let comments = trivia.comments();
```

`ruff_python_index::Indexer` also constructs and exposes a `CommentRanges` instance from the same token stream.

### Range intersection

```rust
use ruff_text_size::TextRange;

if comments.intersects(node.range()) {
    // At least one comment range intersects this byte range.
}
```

### Comments contained in a source range

```rust
for range in comments.comments_in_range(node.range()) {
    let text = &source[*range];
    println!("{text}");
}
```

### Node-level comment check

`has_comments` is source-aware: it expands consideration to the line start/end when the AST node itself is the only content on its line, so comments immediately associated with a statement are not missed simply because they lie just outside the AST node's direct range.

```rust
if comments.has_comments(stmt, source) {
    // Treat source-preserving rewrite conservatively.
}
```

### Own-line vs end-of-line comments

```rust
let own_line = CommentRanges::is_own_line(comment_range.start(), source);
```

This distinction matters for edit safety:

```python
# own-line comment
x = 1  # end-of-line comment
```

Deleting the `x = 1` source range without considering its end-of-line comment can leave an orphaned comment or alter intended meaning.

### Comment blocks

`block_comments(source)` identifies consecutive own-line comments whose `#` tokens align in the same column and which form a non-empty block. This is useful for formatter/comment-placement logic and for agents that need to preserve a semantically grouped comment block as one edit unit.

---

## 6.4 `ParenthesizedExpressions`

`ParenthesizedExpressions` is an index of source ranges enclosed by matching parentheses:

```rust
let parenthesized = trivia.parenthesized();

if parenthesized.contains(expr.range()) {
    // The token-derived index records this expression range as parenthesized.
}
```

This fills a classic AST information gap. Many ASTs normalize away syntactically optional parentheses:

```python
x = (a + b)
y = a + b
```

The semantic AST may represent the inner expression similarly while source-preserving or formatting-sensitive logic still cares that the first source explicitly used parentheses.

### Agent rule

Do not infer explicit parentheses solely from AST shape. Query token/source metadata.

---

## 6.5 Lightweight tokenization

The trivia crate exposes a deliberately lightweight tokenizer separate from the full Ruff parser token stream. Use it for local lexical questions where reparsing a module or allocating a full AST is unnecessary.

Conceptual pattern:

```rust
use ruff_python_trivia::SimpleTokenizer;

for token in SimpleTokenizer::starts_at(range.start(), source) {
    // inspect token.kind(), token.range(), stop when outside target
}
```

The exact constructor/method surface is internal-API-sensitive; pin `0.0.7` and verify the current rustdoc/source before copying a constructor from a later Ruff tag.

Good uses:

- Find punctuation adjacent to an AST node.
- Inspect commas/colons/parentheses in a narrow range.
- Determine whether only trivia exists between two AST nodes.
- Locate local syntax around a candidate edit.
- Walk backward from an offset without building a second parse tree.

Bad uses:

- Reimplement the Python parser.
- Resolve identifiers.
- Infer parent AST nodes.
- Treat simple tokenization as a syntax-validity oracle.

---

## 6.6 `BackwardsTokenizer`

Backward lexical scans are common in source-aware transforms:

```text
current AST node start
   <- skip whitespace/comments
   <- inspect previous delimiter/keyword/token
```

`BackwardsTokenizer` exists so Ruff rules do not repeatedly hand-roll reverse byte scanning that gets comments, strings, Unicode identifiers, or line continuations wrong.

Use it when a transformation depends on what immediately precedes a node but a complete parent/AST relationship does not encode the desired concrete syntax.

---

## 6.7 Whitespace and indentation helpers

The whitespace surface includes Python-aware tests such as:

```text
is_python_whitespace
leading_indentation
indentation_at_offset
has_leading_content
has_trailing_content
```

### `has_leading_content`

Use this to detect whether non-whitespace appears before a node on the same logical/source line:

```python
x = 1; y = 2
       ^ y has leading content on its line
```

### `has_trailing_content`

Likewise detects content after a node on its line:

```python
x = 1; y = 2
^^^^^ x has trailing content
```

These are important guardrails for statement-level edits. Replacing the entire physical line for `x = 1` would accidentally remove `y = 2`.

### Indentation

Source-aware rewrite rule:

```text
Never synthesize nested multiline code using hard-coded four spaces
when preserving existing source style matters.
```

Derive indentation from source/trivia or from `ruff_python_codegen::Stylist` where appropriate.

---

## 6.8 Newline-distance helpers

The crate exposes helpers such as:

```text
lines_before
lines_after
lines_after_ignoring_trivia
lines_after_ignoring_end_of_line_trivia
```

Use them when deciding whether an edit should consume or preserve blank lines, comments, and end-of-line trivia.

Typical use:

```text
function A end
<two blank lines>
function B start
```

A source edit that removes A must decide whether the spacing belongs to A, B, or the module-level separator policy. These helpers make such decisions explicit instead of using ad hoc `\n\n` searches.

---

## 6.9 Pragmas and suppression comments

Ruff needs to recognize source comments that affect tooling behavior rather than ordinary prose comments. The trivia crate therefore includes pragma/suppression helpers such as `is_pragma_comment` and related suppression classification.

Examples of source constructs that an agent should treat carefully include:

```python
# noqa
# noqa: F401
# type: ignore
# fmt: skip
```

Not every comment is semantically inert from a tooling perspective.

### Agent invariant

Before deleting, moving, or regenerating a source region containing comments:

```text
1. identify comment ranges;
2. identify whether comments are pragma/suppression-like;
3. preserve or deliberately migrate their attachment;
4. re-run syntax/lint/type validation after the edit.
```

---

## 6.10 `textwrap`

`ruff_python_trivia::textwrap` exposes source-text utilities including:

```text
dedent
 dedent_to
 indent
 indent_first_line
```

These are useful for generating/repositioning multiline replacement text while preserving a destination indentation contract.

Example transformation shape:

```text
original subtree source
  -> dedent relative to original context
  -> transform/generate
  -> indent to destination context
  -> insert as TextRange replacement
```

This is usually safer than applying `.replace("\n", "\n    ")` blindly because Python source can contain empty lines and multiline constructs that require consistent indentation handling.

---

## 6.11 Deployment pattern: one trivia snapshot per source revision

Recommended per-file snapshot:

```rust
struct ParsedFile {
    revision: FileRevision,
    source: std::sync::Arc<str>,
    parsed: ruff_python_parser::Parsed<ruff_python_ast::ModModule>,
    trivia: ruff_python_trivia::TriviaRanges,
    indexer: ruff_python_index::Indexer,
    // normalized facts live separately
}
```

Do not mutate the source while retaining trivia ranges from the prior source version. Every range is meaningful only against its originating text.

Cache key:

```text
(file identity, content hash/revision, Ruff frontend version, Python target/source type)
```

---

## 6.12 When trivia should gate an automated edit

Treat a candidate edit as higher-risk if any of the following are true:

```text
comment intersects edit
pragma comment near edit
multi-statement line
backslash continuation involved
multiline string adjacent
explicit parentheses affect presentation
replacement changes indentation level
edit consumes blank lines across declarations
```

This does not mean “refuse the edit.” It means choose a source-aware edit strategy and validate the result rather than assuming AST semantics are sufficient.

---

## 6.13 Anti-pattern inventory

- Rescanning the entire source with regex for `#` comments when token-derived comment ranges already exist.
- Treating `#` inside a string as a comment.
- Assuming the AST tells you whether optional parentheses existed.
- Deleting a statement range without checking end-of-line comments.
- Treating `# noqa`, `# type: ignore`, or `# fmt: skip` as ordinary disposable prose.
- Hard-coding four-space indentation in a source-preserving refactor.
- Performing byte slicing with line/column coordinates without converting through Ruff's offset model.
- Keeping trivia ranges after source text changes.
- Using `SimpleTokenizer` as a substitute for syntax validation.

---

## 6.14 Agent checklist

```text
[ ] Build TriviaRanges from the same Parsed token stream as the AST.
[ ] Use CommentRanges rather than regex comment detection.
[ ] Check comment intersection before structural source edits.
[ ] Distinguish own-line and end-of-line comments.
[ ] Preserve pragma/suppression comments deliberately.
[ ] Query explicit-parenthesis metadata when concrete syntax matters.
[ ] Use lightweight tokenizers for local lexical questions only.
[ ] Use indentation/textwrap helpers for multiline replacement text.
[ ] Invalidate trivia after every source revision.
[ ] Reparse and validate every generated source edit.
```

---

# 7) `ruff_python_index` deep dive

## 7.0 Identity: a token-derived omitted-fact index, not a project index

The name `ruff_python_index` is easy to misread in a code-intelligence architecture. It is **not** Ruff's repository symbol index, type index, search database, or incremental project graph.

The crate is intentionally small. Its public entry point is:

```rust
pub use indexer::Indexer;
```

Its source describes `Indexer` as a structure used to index source code for efficient lookup of tokens omitted from the AST, such as commented lines.

Think:

```text
Parsed::tokens() + source
   -> Indexer
       comment ranges
       multiline-string ranges
       interpolated-string ranges
       continuation line starts
       multi-statement-line helpers
```

not:

```text
repository -> symbols -> references -> database
```

---

## 7.1 Construction

Canonical construction is cheap and direct:

```rust
use ruff_python_index::Indexer;
use ruff_python_parser::parse_module;

fn index_source(source: &str) -> Result<(), ruff_python_parser::ParseError> {
    let parsed = parse_module(source)?;
    let indexer = Indexer::from_tokens(parsed.tokens(), source);

    println!("comments={}", indexer.comment_ranges().len());
    Ok(())
}
```

Input invariant:

```text
parsed.tokens() and source must describe the exact same source revision.
```

---

## 7.2 Internal indexed facts

At `0.0.7`, `Indexer` holds conceptually:

```rust
pub struct Indexer {
    continuation_lines: Vec<TextSize>,
    interpolated_string_ranges: InterpolatedStringRanges,
    multiline_ranges: MultilineRanges,
    comment_ranges: CommentRanges,
}
```

The indexer walks the token stream once and also inspects source gaps between tokens to detect continuation-related newlines that are not represented as ordinary logical-newline tokens.

---

## 7.3 Comment ranges

```rust
let comments = indexer.comment_ranges();
```

This returns Ruff's `CommentRanges` structure, giving the same efficient range-query model described in the trivia section.

Architectural choice:

```text
If you already build Indexer for every parsed file,
use indexer.comment_ranges() for lint/edit extraction paths.

If you need formatter comment-placement data,
build TriviaRanges as well because it also indexes parentheses.
```

Avoid keeping two independently produced comment-range structures unless they serve different downstream ownership/lifetime needs.

---

## 7.4 Interpolated-string ranges

```rust
let ranges = indexer.interpolated_string_ranges();
```

This tracks source ranges for interpolated strings, including nested interpolated strings.

Why this matters:

```python
f"outer {f'inner {x}'}"
```

A raw textual search for braces, quote delimiters, or names can easily cross string-expression boundaries. The index lets source-aware rules cheaply ask whether an offset lies in a special string region and distinguish nested ranges.

Ruff's current parser/indexer also accounts for newer interpolated-string token forms, including t-string token kinds in the indexed token scan.

---

## 7.5 Multiline-string ranges

```rust
let ranges = indexer.multiline_ranges();
```

Use this when a source operation is sensitive to physical newlines but should not treat newlines inside a multiline string as normal statement-layout newlines.

Example:

```python
QUERY = """
select *
from table
"""
```

An agent searching for “blank lines separating statements” must not classify the line structure inside the string as module-level blank-line structure.

---

## 7.6 Continuation line starts

```rust
let starts = indexer.continuation_line_starts();
```

The indexer tracks explicit backslash continuations, including consecutive continuation lines.

```python
value = a + \
        b + \
        c
```

Relevant public helpers include:

```text
is_continuation(offset, source)
preceded_by_continuations(offset, source)
```

These are particularly useful before statement-level source edits, where a statement's AST range may begin after one or more physical continuation lines.

---

## 7.7 Multi-statement-line helpers

The indexer combines continuation awareness with trivia content checks to expose:

```text
preceded_by_multi_statement_line(stmt, source)
followed_by_multi_statement_line(stmt, source)
in_multi_statement_line(stmt, source)
```

Example:

```python
x = 1; import sys
```

A fix that assumes an import owns its physical line may be invalid here.

Canonical guard:

```rust
if indexer.in_multi_statement_line(stmt, source) {
    // Narrow the edit to the exact statement range and surrounding separator,
    // or choose a more conservative transformation.
}
```

---

## 7.8 Why the indexer is valuable even if you have Tree-sitter

In a hybrid frontend, Tree-sitter may already give a concrete syntax tree and incremental changed ranges. `Indexer` can still be useful because it exposes Ruff-specific, Python-aware lexical classifications that Ruff's own rules and formatter ecosystem expect.

A practical hybrid split:

```text
Tree-sitter:
  changed-range discovery
  error-tolerant incremental tree
  structural query acceleration

Ruff parser + Indexer:
  authoritative Python AST on clean snapshots
  Ruff token semantics
  comment / multiline / interpolation / continuation facts
```

Do not keep the indexer only because “it is called index.” Keep it when these exact source facts simplify your downstream correctness logic.

---

## 7.9 Cache and lifecycle

Recommended lifecycle:

```text
source revision N
  parse once
  build Indexer once
  answer many edit/lint/extraction queries

source revision N+1
  discard N's Indexer
  parse/rebuild
```

Because the indexer is derived from source/token ranges, attempting to patch it manually after arbitrary edits is usually more fragile than rebuilding it.

For code-intelligence services, its rebuild cost should be measured against the parser cost; do not prematurely invent persistent serialization for it.

---

## 7.10 Anti-pattern inventory

- Calling `ruff_python_index` a symbol index.
- Persisting `Indexer` as the repository's code-intelligence database.
- Using it for cross-module name resolution.
- Expecting type information.
- Reusing an indexer after source edits.
- Scanning comments again when `Indexer::comment_ranges()` already exists.
- Assuming physical newlines are statement boundaries without checking multiline strings/continuations.
- Performing whole-line edits without `in_multi_statement_line` checks.

---

## 7.11 Agent checklist

```text
[ ] Construct Indexer once from Parsed::tokens() + the exact source text.
[ ] Use comment_ranges() for fast comment queries.
[ ] Use interpolated_string_ranges() around f/t-string-sensitive edits.
[ ] Use multiline_ranges() for physical-line analysis.
[ ] Check continuation_line_starts / preceded_by_continuations where relevant.
[ ] Guard statement edits with in_multi_statement_line().
[ ] Rebuild the index after source changes.
[ ] Do not conflate this crate with semantic/project indexing.
```

---

# 8) `ruff_python_semantic` deep dive

## 8.0 Identity: Ruff's linter semantic state

`ruff_python_semantic` provides the semantic structures Ruff's AST checker uses while traversing a Python module. Its public surface models:

```text
module
  scopes
    bindings
      references
      shadowed/rebound relationships
    definitions
  branches
  globals/nonlocals
  imports
  resolved / unresolved names
  execution context
  handled exceptions
  recognized modules / qualified names
```

The key object is `SemanticModel<'a>`.

### Critical non-goal

`SemanticModel` is **not** Ruff's modern full Python type-checking engine and is not a substitute for Pyrefly. It is optimized for linter rules that need Python name/binding/import/scope knowledge while walking one module.

Use this responsibility split:

```text
ruff_python_semantic
  lexical name binding
  scope membership
  local import/qualified-name resolution
  shadowing
  definitions/references used by Ruff lint analysis
  branch/execution context

Pyrefly query daemon
  inferred types
  cross-module definition/type resolution
  subtype/call compatibility
  type-driven dispatch/call graph enrichment
```

---

## 8.1 Public module map

The crate exposes modules and re-exports centered around:

```text
analyze
binding
branches
cfg
context
definition
globals
imports
model
nodes
reference
scope
star_import
```

Representative public types include:

```text
SemanticModel
Binding / BindingId / BindingKind / BindingFlags / Bindings
Definition / DefinitionId / Definitions
ResolvedReference / ResolvedReferenceId
UnresolvedReference
Scope / ScopeId / ScopeKind / Scopes
BranchId / Branches
NodeId / Nodes
Module / Modules
Import / FromImport / SubmoduleImport / StarImport
Globals
ExecutionContext
Exceptions
```

Treat these IDs as **arena/snapshot-local identifiers**, not stable persistent IDs for your CPG.

---

## 8.2 `SemanticModel<'a>` state model

At the pinned release the model contains, among other state:

```text
target/source configuration
module identity
AST-node stack + current node
branch stack + current branch
scope arena + current scope
Definition arena + current definition
Binding arena
resolved-reference arena
unresolved-reference arena
globals arena
shadowed-binding map
delayed annotations
rebinding scopes
semantic flags
seen modules
handled exceptions
resolved-name map
```

This is a traversal-state object, not merely an immutable table loaded after parsing.

---

## 8.3 Construction is not analysis

A critical integration detail is that `SemanticModel::new(...)` initializes the model; it does **not** magically populate a complete module semantic graph.

Ruff's own AST checker is responsible for traversing the AST, building the `SemanticModel`, and running rules. Ruff documents its checker flow as one evaluation-order pass whose semantic phase consists broadly of:

```text
1. Binding: introduce names for the current node.
2. Traversal: visit children in Python evaluation order.
3. Cleanup: leave/update scope, branch, exception, deferred context.
4. Analysis: run lint rules using the accumulated model.
```

Therefore this is unsafe architecture:

```rust
let semantic = SemanticModel::new(/* ... */);
// Wrong assumption: semantic is now a complete module symbol table.
```

A standalone consumer has three realistic options:

```text
A. Depend on / reuse a Ruff traversal layer that populates SemanticModel.
B. Reproduce the required semantic traversal carefully in an adapter crate.
C. Use only selected semantic helper types/functions and let Pyrefly own symbol semantics.
```

For an external production code-intelligence system, option B should be isolated behind a version-pinned adapter, because this traversal behavior is an internal Ruff contract and can change.

---

## 8.4 Model construction shape

The constructor takes configuration and module identity, conceptually:

```rust
SemanticModel::new(
    typing_modules,
    custom_builtins,
    target_version,
    source_type,
    path,
    module,
)
```

The model borrows data associated with the module/analysis lifetime. This reinforces the recommended architecture:

```text
Ruff semantic model = ephemeral per-file analysis snapshot
Your normalized semantic facts = persistent CPG contract
```

Do not attempt to keep borrowed `SemanticModel<'a>` values indefinitely across source revisions.

---

## 8.5 Scopes

Python scope rules are not equivalent to simple lexical block nesting. Relevant scope contexts include module, function/lambda, class, comprehensions, and special behavior around `global` and `nonlocal`.

The semantic layer tracks scopes in `Scopes` and addresses them with `ScopeId`.

Useful normalized CPG facts:

```text
Scope {
  stable_id,
  kind,
  owner_node,
  parent_scope,
  source_range
}

CONTAINS_SCOPE(parent, child)
OWNS_SCOPE(definition, scope)
```

Avoid persisting raw `ScopeId`; derive your stable ID from file identity + syntactic owner + graph-generation revision/key.

---

## 8.6 Bindings

Bindings represent concrete name-binding events. Examples:

```python
x = 1                  # assignment binding
import os              # import binding
from x import y as z   # import binding for z
def f(): ...           # function-name binding
class C: ...           # class-name binding
for item in items: ... # target binding
except E as exc: ...   # exception-target binding
```

The model records each binding's source range, kind, flags, scope, references, source-node association, execution context, and handled-exception context.

Normalize binding events separately from definitions when your graph needs accurate Python rebinding behavior:

```text
symbol name "x"
  ├─ binding at module line 1
  ├─ rebinding at module line 20
  └─ local shadow binding inside function
```

A single “variable node per spelling” loses this behavior.

---

## 8.7 Shadowing and rebinding

The model tracks shadowed bindings across scopes and rebinding scopes for `global` / `nonlocal` behavior.

Example:

```python
x = 1

def f():
    x = 2
```

The local `x` shadows the outer binding.

Contrast:

```python
x = 1

def f():
    global x
    x = 2
```

Now the assignment participates in global rebinding semantics rather than introducing the same kind of local shadow.

For a CPG, model these relationships explicitly:

```text
SHADOWS(binding_local, binding_outer)
REBINDS(binding_event, scope_or_symbol)
```

Do not infer them later from identical identifier text.

---

## 8.8 Definitions vs bindings

A robust semantic graph should not collapse every binding into a definition. Python has binding events that do not correspond cleanly to named function/class definitions, and annotations can create delayed annotation relationships.

Recommended graph separation:

```text
Definition
  function / class / module / parameter / ... as applicable

Binding
  name-binding event with source and scope

Reference
  name-use event
```

Then relate them:

```text
DEFINES
BINDS
REFERS_TO
SHADOWS
ANNOTATES
```

This aligns more naturally with Ruff's own arenas and prevents overloading a single node type.

---

## 8.9 Resolved and unresolved references

Ruff maintains both resolved and unresolved references. This is important for code intelligence because “unresolved” is a first-class state, not an exception to hide.

Persist a confidence/state distinction:

```text
ReferenceResolution::Resolved(target_binding)
ReferenceResolution::Builtin(name)
ReferenceResolution::Unresolved(reason/flags)
ReferenceResolution::DeferredToTypeLayer
```

A source analyzer should not invent an edge merely to make the graph complete.

---

## 8.10 Symbol lookup

The model exposes a `Symbol` result conceptually distinguishing:

```text
Binding(BindingId)
Builtin
Unbound
```

This is superior to a nullable binding ID because builtins are a meaningful successful lookup state even when a concrete builtin binding has not yet been materialized in the arena.

Agent rule:

```text
Unbound != builtin.
Builtin != local binding.
Preserve the distinction in normalized facts.
```

---

## 8.11 Qualified-name resolution

One of the most valuable semantic capabilities is resolving an expression to a qualified import/name path where possible.

Example source:

```python
import pathlib as p

x = p.Path("a")
```

Syntactically, the call target is an attribute expression over `p`. Semantically, the qualified name can resolve to something like:

```text
pathlib.Path
```

This is extremely useful for rule matching and CPG normalization.

Representative uses:

```text
resolve_qualified_name(expr)
match_typing_expr(expr, target)
match_typing_qualified_name(name, target)
resolve_builtin_symbol(expr)
match_builtin_expr(expr, symbol)
```

Do not equate qualified-name resolution with dynamic/runtime call-target certainty. Python values can be reassigned, monkey-patched, imported conditionally, and exposed through dynamic attributes. Preserve a “lexically/semantically resolved import path” confidence class rather than claiming dynamic certainty.

---

## 8.12 Typing-module awareness

`SemanticModel` recognizes standard typing modules and optionally user-configured typing modules. This allows linter semantics such as:

```text
typing.List
typing_extensions.TypeAlias
custom_typing_module.Protocol
```

without relying only on literal spelling.

This is **typing-symbol recognition**, not full type inference.

---

## 8.13 Builtins

The semantic layer knows Python-version/source-type builtins and magic globals and can materialize/query builtin bindings.

This helps distinguish:

```python
open("x")             # builtin open if not shadowed

open = custom_open
open("x")             # local/global binding now shadows builtin

import builtins
builtins.open("x")    # explicit module lookup
```

A text-only or AST-only analysis that labels every `open(...)` call as builtin `builtins.open` is wrong.

---

## 8.14 Imports

The crate models multiple import forms:

```python
import pkg
import pkg.sub
import pkg as alias
from pkg import value
from pkg import value as alias
from .pkg import value
from pkg import *
```

For CPG ingestion, preserve both:

```text
syntactic import statement
semantic local binding(s)
```

and normalize the target module/name where resolution is possible.

Recommended graph facts:

```text
IMPORTS_MODULE(file, module_name)
IMPORTS_NAME(file, qualified_name)
BINDS_IMPORT(local_binding, imported_name)
STAR_IMPORT(file, module_name)
```

Star imports should remain low-confidence for subsequent unresolved names unless your project/module resolver can determine exported names.

---

## 8.15 Branch context

Ruff tracks branch identity/context to answer whether bindings/references occur in the same branch and to support linter reasoning around conditional control flow.

Example:

```python
if flag:
    x = 1
else:
    x = 2
print(x)
```

A flat list of binding events cannot fully express conditional reachability. Your normalized graph may choose a coarser or richer control-flow model, but do not assume branch context is irrelevant simply because both bindings spell `x`.

---

## 8.16 Execution context

Python annotations, typing-only blocks, runtime code, deferred function bodies, and class-body evaluation can have different execution semantics.

The semantic model tracks execution context as part of its analysis state. Preserve at least a coarse distinction when it changes meaning:

```text
runtime
annotation/type-expression context
typing-only guard if determinable
```

This becomes particularly important when a CPG is used to answer “could this import/call execute at runtime?” rather than merely “does the syntax exist?”

---

## 8.17 Handled exceptions

The model tracks exceptions handled by surrounding `try` blocks while traversing nodes.

Example:

```python
try:
    risky()
except AttributeError:
    recover()
```

Lint rules may need to know that an operation occurs under a handler. A CPG can normalize this into control-flow/exception edges if that level of analysis is valuable.

---

## 8.18 `analyze::*` helpers

The `analyze` namespace contains focused semantic helpers for areas such as:

```text
class
function_type
imports
logging
terminal
type_inference
typing
visibility
```

Important naming caveat: the presence of an `analyze::type_inference` helper module does **not** make `ruff_python_semantic` a full type checker. These helpers exist to answer targeted linter questions.

Agent policy:

```text
Use Ruff semantic helpers for the narrowly documented question they solve.
Use Pyrefly for authoritative project type queries.
```

---

## 8.19 CFG module

The crate publicly exposes `cfg`. Treat this as Ruff's semantic/control-flow infrastructure and verify exact types/functions against the pinned tag before direct external dependence. Because internal CFG APIs are especially likely to evolve with lint-rule needs, normalize any useful control-flow facts into your own graph rather than exposing Ruff CFG types across service boundaries.

---

## 8.20 Recommended integration boundary

For a production external system, create an adapter crate:

```text
crates/python-ruff-frontend/
  parser.rs
  ast_extract.rs
  trivia.rs
  semantic.rs
  edit.rs
  normalize.rs
```

Public API from your adapter:

```rust
pub struct PythonSemanticSnapshot {
    pub scopes: Vec<ScopeFact>,
    pub bindings: Vec<BindingFact>,
    pub references: Vec<ReferenceFact>,
    pub imports: Vec<ImportFact>,
}
```

Do **not** expose this from your platform crate:

```rust
pub fn semantic_model(...) -> ruff_python_semantic::SemanticModel<'_>;
```

Why:

- It leaks unstable Ruff types into the rest of the workspace.
- It leaks borrow/lifetime coupling.
- It makes 0.0.x migrations repository-wide.
- It encourages downstream code to depend on semantic traversal implementation details.

---

## 8.21 Pyrefly complementarity

Recommended semantic merge:

```text
Ruff frontend fact                       Pyrefly enrichment
──────────────────────────────────────   ───────────────────────────────
name reference range                  +  inferred type
local binding                         +  canonical definition target
import-qualified lexical name         +  cross-module symbol identity
call expression                       +  callable type / candidate target
attribute expression                  +  resolved member/type if available
function/class definition             +  full signature / type params
annotation syntax                     +  resolved annotation type
```

The merge key should be based on source identity/range + source revision, with explicit handling for query results that point to generated/stub/external locations.

---

## 8.22 Confidence classes for semantic edges

A CPG should not label every semantic edge “resolved” equally. Recommended confidence taxonomy:

```text
SYNTACTIC
  structure is directly in AST

LEXICAL_BINDING
  Ruff scope/binding resolution

IMPORT_QUALIFIED
  Ruff qualified-name/import resolution

TYPE_RESOLVED
  Pyrefly/type-checker resolution

HEURISTIC
  fallback inference / dynamic approximation

UNRESOLVED
  target intentionally unknown
```

This makes downstream LLM agents much less likely to overstate call-graph certainty.

---

## 8.23 Lifecycle and concurrency

Recommended pattern:

```text
per file revision:
  immutable source Arc<str>
  parsed AST/tokens
  build semantic state during one traversal
  emit normalized facts
  discard or keep only for short-lived query batch
```

Parallelize across files where possible. Avoid sharing one mutable `SemanticModel` between concurrent tasks; it represents traversal-local mutable state.

Cross-module enrichment should occur after local facts are normalized, through Pyrefly/project-resolution services.

---

## 8.24 Failure policy

When semantic construction/querying cannot resolve a name:

```text
Do not fail the whole file.
Do not invent a target.
Emit unresolved/local syntax facts.
Queue type/project resolver enrichment separately.
```

The persistent graph should be monotonic in information quality where practical:

```text
syntax fact exists immediately
  -> local binding edge added
  -> type-resolved target edge added later
```

rather than deleting useful syntax because a semantic layer failed.

---

## 8.25 Anti-pattern inventory

- Treating `SemanticModel::new` as a one-call semantic analyzer.
- Copying Ruff Checker behavior partially without respecting binding/traversal/cleanup order.
- Calling Ruff's linter semantic layer a type checker.
- Persisting `BindingId`, `ScopeId`, `NodeId`, or `DefinitionId` as stable graph IDs.
- Treating qualified import-name resolution as guaranteed runtime dispatch.
- Collapsing all same-spelled variables into one graph node.
- Ignoring `global`, `nonlocal`, shadowing, or rebinding.
- Dropping unresolved references from the graph.
- Treating star imports as fully resolved without project export analysis.
- Letting Ruff semantic types leak through your platform-wide API.
- Sharing a mutable per-traversal model across files/tasks.

---

## 8.26 Agent checklist

```text
[ ] Treat SemanticModel as linter-oriented per-module semantic state.
[ ] Remember constructor initialization is not full semantic analysis.
[ ] Reuse/replicate semantic traversal only behind a version-pinned adapter.
[ ] Preserve scopes, bindings, definitions, and references as separate concepts.
[ ] Normalize shadowing/rebinding explicitly.
[ ] Use qualified-name resolution where it improves import/call classification.
[ ] Distinguish Builtin / Binding / Unbound.
[ ] Preserve unresolved references.
[ ] Keep raw Ruff IDs snapshot-local.
[ ] Use Pyrefly for full type/cross-module semantic resolution.
[ ] Attach confidence/provenance to semantic edges.
[ ] Normalize into stable CPG facts before leaving the frontend boundary.
```

---

# 9) `ruff_python_codegen` deep dive

## 9.0 Identity: AST unparser, not a lossless source printer

`ruff_python_codegen` turns Ruff AST structures back into Python source. It understands Python precedence, syntax delimiters, string escaping, indentation, and line endings.

Its central concepts are:

```text
Stylist
  detects/holds indentation + line-ending conventions

Generator
  AST -> Python source

Mode
  Ruff default generation
  CPython ast.unparse-like generation

round_trip
  parse module -> infer style -> unparse suite
```

The crucial distinction:

```text
codegen preserves AST meaning/syntax validity targets
codegen does NOT own original comments/whitespace trivia
```

Therefore it is useful for generated replacement fragments and transformed ASTs, but it is not a drop-in LibCST “print exactly what I did not modify” facility.

---

## 9.1 Crate surface

At `0.0.7` the public top-level surface includes:

```rust
pub use generator::{Generator, Mode};
pub use stylist::{Indentation, Stylist};

pub fn round_trip(code: &str) -> Result<String, ParseError>;
```

The generator depends on Ruff's AST, parser, literal handling, source-file line endings, and text-size model.

---

## 9.2 `round_trip`

Canonical use:

```rust
use ruff_python_codegen::round_trip;

fn regenerate(source: &str) -> Result<String, ruff_python_parser::ParseError> {
    round_trip(source)
}
```

Internally this performs conceptually:

```text
parse_module(source)
  -> Stylist::from_tokens(parsed.tokens(), source)
  -> Generator from stylist
  -> unparse suite
  -> generated String
```

### Do not interpret the name as byte round-trip

`round_trip` means parse/unparse through Ruff's AST generator. It does **not** guarantee:

```text
output == input bytes
comments survive
blank lines survive
parenthesization survives exactly
quote selection survives exactly
all formatting survives
```

If byte preservation matters, keep the original source and apply range edits.

---

## 9.3 `Stylist`

`Stylist` derives style properties from the parsed token/source stream. The generator uses at least indentation and line-ending preferences.

Canonical source-derived construction used by Ruff:

```rust
let parsed = ruff_python_parser::parse_module(source)?;
let stylist = ruff_python_codegen::Stylist::from_tokens(parsed.tokens(), source);
```

Use a source-derived stylist when replacement text should blend with an existing file rather than assuming `\n` and four spaces.

---

## 9.4 `Generator`

Construction:

```rust
use ruff_python_codegen::Generator;

let generator: Generator = (&stylist).into();
```

Or explicitly from indentation + line ending:

```rust
let generator = Generator::new(stylist.indentation(), stylist.line_ending());
```

Public convenience methods include generation of individual statements and expressions:

```rust
let text = Generator::from(&stylist).stmt(stmt);
let text = Generator::from(&stylist).expr(expr);
```

These are excellent primitives for minimal source edits:

```text
1. locate one AST node in original source;
2. construct/transform replacement AST fragment;
3. Generator::expr / ::stmt -> replacement text;
4. replace only original node TextRange;
5. retain untouched source bytes everywhere else.
```

---

## 9.5 Generator modes

`Mode` has two important modes:

```text
Mode::Default
  Ruff's default unparsing behavior

Mode::AstUnparse
  aims to emit source aligned with CPython ast.unparse-style choices
```

Usage:

```rust
use ruff_python_codegen::{Generator, Mode};

let text = Generator::from(&stylist)
    .with_mode(Mode::AstUnparse)
    .expr(expr);
```

Choose mode deliberately and snapshot-test the generated text. Do not let generated output style become an accidental API contract.

---

## 9.6 Precedence-aware generation

A major reason to use `Generator` instead of concatenating strings is precedence.

Suppose an agent transforms:

```python
x = a * b
```

into an expression that semantically needs:

```python
x = (a + c) * b
```

A naive generator might emit:

```python
x = a + c * b
```

and silently change semantics. Ruff's generator implements a detailed precedence model for expressions/statements/operators and inserts required delimiters/parentheses.

Agent invariant:

```text
Never hand-build arbitrary Python expression strings when you already have an AST.
Use the code generator or a proven source-edit template.
```

---

## 9.7 String literal generation

The generator handles Python string/bytes prefixes, quote styles, raw strings, triple quotes, Unicode/ASCII escaping, f-string/t-string structures, and mode-specific quoting behavior.

This is another area where string concatenation is dangerous:

```text
raw prefix + quote style + embedded quote + backslash + newline
```

can produce syntactically different or invalid Python if manually escaped.

---

## 9.8 Statement and expression fragment generation

Preferred refactor architecture:

```rust
fn replacement_for_expr(
    expr: &ruff_python_ast::Expr,
    stylist: &ruff_python_codegen::Stylist,
) -> String {
    ruff_python_codegen::Generator::from(stylist).expr(expr)
}
```

Then preserve surrounding source:

```text
source[0..expr.start]
+ generated replacement
+ source[expr.end..]
```

For multiple edits, use a validated edit engine rather than concatenating incrementally; see §12.

---

## 9.9 Suite/module generation caveat

`Generator` exposes internal suite-unparsing operations, but some final buffer methods are not intended as a broad stable external API. For whole-module regeneration, `round_trip` is the simple public path for existing source; for transformed modules, verify the exact `0.0.7` public methods before designing a long-lived external API around internal generator internals.

This is exactly why a local adapter is valuable:

```rust
pub trait PythonEmitter {
    fn emit_expr(&self, expr: &Expr) -> String;
    fn emit_stmt(&self, stmt: &Stmt) -> String;
}
```

Only your adapter needs modification when Ruff changes generator visibility/signatures.

---

## 9.10 Comments and codegen

Comments are not normal AST child nodes. If you parse:

```python
x = 1  # important
```

and regenerate only the AST assignment, the comment is not guaranteed to be emitted by `Generator`.

Therefore:

```text
AST transform + Generator over whole file
  != source-preserving codemod
```

Correct choices:

```text
Need untouched comments/whitespace to remain byte-identical?
  -> range edit original source

Need replacement fragment from AST?
  -> Generator for fragment + range edit

Need canonical full-file formatting with comments retained/repositioned?
  -> ruff_python_formatter
```

---

## 9.11 Recommended transform pattern

```text
original source
   │
   ├─ parse -> AST + tokens
   ├─ detect exact target Expr/Stmt
   ├─ clone/build replacement AST
   ├─ Generator::expr/stmt(replacement)
   ├─ source-aware indentation adjustment if needed
   ├─ TextRange edit
   ├─ reparse edited source
   └─ optional formatter pass
```

This gives most of the correctness benefit of AST generation while preserving untouched source.

---

## 9.12 Validation policy

Every generated replacement should pass at least:

```text
syntactic validation:
  Ruff parse edited file

structural validation:
  expected AST pattern exists

semantic/type validation where relevant:
  Pyrefly diagnostics/query

diff validation:
  changed ranges are limited to intended region
```

For an LLM coding agent, “the generated text looks valid” is not sufficient.

---

## 9.13 Anti-pattern inventory

- Assuming `round_trip(source) == source`.
- Using codegen as a comment-preserving printer.
- Regenerating an entire file for a one-expression edit when exact source preservation matters.
- Hand-implementing Python precedence.
- Manually escaping complex string literals from AST values.
- Hard-coding line endings/indentation instead of deriving a `Stylist`.
- Treating generator formatting as Ruff formatter formatting; they are different responsibilities.
- Exposing unstable generator internals across the whole application.

---

## 9.14 Agent checklist

```text
[ ] Use Generator for AST -> Python fragments.
[ ] Derive Stylist from the same source/tokens when blending with existing code.
[ ] Choose Default vs AstUnparse mode deliberately.
[ ] Let Generator handle precedence and string escaping.
[ ] Preserve original source outside the target TextRange.
[ ] Do not expect comments/trivia to be regenerated by AST codegen.
[ ] Reparse every generated edit.
[ ] Run type/semantic validation when behavior could change.
[ ] Hide Generator behind your own adapter if used broadly.
```

---

# 10) `ruff_python_formatter` deep dive

## 10.0 Identity: canonical pretty printer with source/comment awareness

`ruff_python_formatter` is Ruff's Python formatting engine. It is fundamentally a **pretty printer**: it consumes parsed syntax plus source/trivia/comment information and produces normalized Python formatting according to Ruff formatter options.

It should be used for:

```text
whole-file canonical formatting
range formatting
formatter-equivalent output
comment-aware pretty printing
source maps between original/formatted positions
```

It should not be used when the requirement is:

```text
“leave every untouched byte exactly unchanged”
```

That requirement is a range-edit/source-preservation problem, not a formatter problem.

---

## 10.1 Dependency weight

Among the eight requested crates, the formatter is one of the heaviest integration points. Its `0.0.7` manifest depends on infrastructure including:

```text
ruff_cache
ruff_db
ruff_formatter
ruff_macros
ruff_python_ast
ruff_python_parser
ruff_python_trivia
ruff_source_file
ruff_text_size
salsa
tracing
...
```

This is deliberate: formatting is not merely “AST -> string.” Ruff associates comments with AST nodes, tracks source mapping, uses formatter IR/printer infrastructure, and supports database-backed file formatting.

Deployment implication:

```text
If your service only analyzes source, do not depend on ruff_python_formatter by default.
Put formatting in an optional adapter/feature or transformation service layer.
```

---

## 10.2 Main public entry point: `format_module_source`

Canonical whole-source formatting:

```rust
use ruff_python_formatter::{PyFormatOptions, format_module_source};

fn format_python(source: &str) -> Result<String, ruff_python_formatter::FormatModuleError> {
    let printed = format_module_source(source, PyFormatOptions::default())?;
    Ok(printed.as_code().to_string())
}
```

Internally, Ruff performs conceptually:

```text
parse(source, ParseOptions)
  -> Parsed AST + tokens
  -> TriviaRanges::from(tokens)
  -> associate comments with AST/source
  -> format AST into formatter document/IR
  -> print document
  -> Printed source
```

This comment-association step is the key reason formatter output can preserve comments semantically while still moving/reflowing them according to formatting rules.

---

## 10.3 `format_module_ast`

If you already have the parsed module, trivia ranges, and source, avoid reparsing by using the AST-oriented entry point:

```rust
use ruff_python_formatter::{PyFormatOptions, format_module_ast};
use ruff_python_parser::{ParseOptions, parse};
use ruff_python_trivia::TriviaRanges;

let options = PyFormatOptions::default();
let parsed = parse(source, ParseOptions::from(options.source_type()))?;
let trivia = TriviaRanges::from(parsed.tokens());

let formatted = format_module_ast(&parsed, &trivia, source, options)?;
let printed = formatted.print()?;
let output = printed.as_code();
```

Use this path in a frontend service that already parses source for AST extraction.

### Invariant

`parsed`, `trivia`, and `source` must all belong to the same source revision.

---

## 10.4 Comment attachment model

The formatter builds a `Comments` structure from:

```text
AST
original SourceCode
TriviaRanges
```

Formatting rules then treat comments as leading, dangling, or trailing relative to AST nodes. Ruff asserts that all comments are accounted for during formatting.

This is stronger than AST codegen for comments, but it is still not LibCST-style ownership of every whitespace token. The formatter is free to normalize layout.

---

## 10.5 `PyFormatOptions`

`PyFormatOptions` is the resolved per-file formatting contract.

Important fields/options include:

| Option | Purpose |
|---|---|
| source type | `.py` vs `.pyi` formatting behavior |
| target Python version | Syntax/version-dependent formatting |
| indent style | spaces or tabs |
| indent width | visual indentation width |
| line width | preferred wrap width; default 88 |
| line ending | LF/CRLF behavior |
| quote style | single/double/preserve |
| magic trailing comma | respect/ignore |
| source map generation | map original to formatted positions |
| docstring code formatting | opt-in formatting of code examples |
| docstring code line width | fixed or dynamic |
| preview mode | preview formatting behavior |
| nested string quote style | alternating/preferred for nested strings |

---

## 10.6 Options construction

Default:

```rust
let options = ruff_python_formatter::PyFormatOptions::default();
```

Infer source type from path:

```rust
use std::path::Path;
use ruff_python_formatter::PyFormatOptions;

let options = PyFormatOptions::from_extension(Path::new("module.pyi"));
```

Explicit source type:

```rust
use ruff_python_ast::PySourceType;

let options = PyFormatOptions::from_source_type(PySourceType::Stub);
```

Builder-style modification:

```text
with_target_version
with_indent_width
with_quote_style
with_magic_trailing_comma
with_indent_style
with_line_width
with_line_ending
with_docstring_code
with_docstring_code_line_width
with_preview
with_nested_string_quote_style
with_source_map_generation
```

Because formatter option types come partly from lower-level Ruff formatter crates, verify exact constructors (for line width/indent width/source-map settings) against `0.0.7` before copying examples from a later tag.

---

## 10.7 Quote style

`QuoteStyle` includes:

```text
Single
Double     # default
Preserve
```

`Preserve` should not be interpreted as “preserve every original string byte.” It is a quote preference within formatter behavior, not a general lossless-source mode.

---

## 10.8 Magic trailing comma

`MagicTrailingComma`:

```text
Respect   # default
Ignore
```

This controls whether a trailing comma acts as a signal to expand/wrap certain constructs.

Example:

```python
func(
    a,
    b,
)
```

Formatting decisions may differ depending on whether the trailing comma signal is respected.

---

## 10.9 Preview mode

`PreviewMode` controls formatting behavior gated behind Ruff preview semantics.

Agent deployment rule:

```text
Never silently enable preview in a refactoring service
unless the workspace's formatter policy explicitly enables it.
```

Formatting diffs can change across preview/stable modes even when source semantics do not.

---

## 10.10 Nested string quote style

For modern Python nested interpolated strings, `NestedStringQuoteStyle` lets Ruff choose between alternating quotes and preferred quote style.

This is a concrete example of why a formatter must be version-aware: Python 3.12+ string grammar and nested interpolation behavior differ from older assumptions.

---

## 10.11 Docstring code formatting

`DocstringCode` can enable formatting of code snippets embedded in docstrings. `DocstringCodeLineWidth` controls whether those snippets use a fixed width or inherit dynamically.

This can expand the edit surface substantially. If an LLM agent is applying a localized code fix, enabling docstring-code formatting at the same time can create unrelated diffs.

Policy:

```text
Respect repository formatter configuration.
Do not turn on additional formatter features opportunistically during a semantic edit.
```

---

## 10.12 `FormatModuleError`

Whole-module formatting can fail in three broad phases:

```text
ParseError
FormatError
PrintError
```

Handle them separately in telemetry when possible:

```text
parse failure  -> invalid/unsupported source or parser issue
format failure -> formatter rule/internal document construction issue
print failure  -> formatter printer/output issue
```

For automated code modification, formatter failure should not cause the original source to be discarded. Preserve the unformatted successful edit and report formatting as a separate stage failure if product policy allows.

---

## 10.13 `format_range`

The crate exports range formatting for formatting a selected source region while respecting Python logical-line/AST boundaries.

Use range formatting when:

- An edit touched a contained region and repository policy allows localized formatter changes.
- A whole-file formatter pass would create excessive unrelated diff churn.
- Your editor/service needs formatter-on-selection behavior.

Do not assume the exact requested byte range is the exact formatted range. Formatters often widen to syntactically/logically meaningful boundaries.

---

## 10.14 `formatted_file` and `Db`

The formatter supports Ruff's database-backed file model:

```rust
formatted_file(db, file) -> Result<Option<String>, FormatModuleError>
```

Semantics:

```text
None        -> formatted output is identical to source
Some(text)  -> file would change
```

This route uses `ruff_db` parsing/source queries and is useful when embedding inside a Ruff/ty-style Salsa database architecture.

For a standalone code-intelligence frontend that already owns file text/revisions, the direct `format_module_source` / `format_module_ast` APIs are usually simpler and reduce coupling to Ruff's database layer.

---

## 10.15 Source maps

The formatter can generate source-position information when enabled. This is useful when a tool must map diagnostics/selections from pre-format to post-format source.

Architectural rule:

```text
If formatting is part of an automated edit pipeline and downstream diagnostics
still refer to pre-format offsets, either:
  A. regenerate/requery diagnostics after formatting; or
  B. retain/use source maps for position translation.
```

Re-querying against the final text is generally safer for code-intelligence systems because semantic state may also change.

---

## 10.16 `pretty_comments`

The public `pretty_comments(module, trivia, source)` helper renders the formatter's comment attachment view for debugging.

Use it when diagnosing cases such as:

```text
comment unexpectedly moved
comment appears dangling
formatting range absorbs nearby comment
formatter differs from intuition
```

This is a debugging surface, not a persistent metadata format.

---

## 10.17 Formatter vs codegen

| Requirement | `ruff_python_codegen` | `ruff_python_formatter` |
|---|---:|---:|
| Generate source for an AST expression | **Yes** | Overkill |
| Generate source for one transformed statement | **Yes** | Possible indirectly but overkill |
| Preserve comments across whole-file pretty printing | No AST ownership of comments | **Yes, comment-aware** |
| Preserve original whitespace exactly | No | No |
| Canonical Ruff formatting | No | **Yes** |
| Apply line-width/quote/trailing-comma policy | Limited generator style | **Yes** |
| Minimal source edit | Use as replacement-fragment emitter | Use only if formatting desired |
| Format a selected range | No | **Yes** |

---

## 10.18 Formatter vs source-preserving edit engine

These are different layers:

```text
Source-preserving edit engine
  owns original text
  replaces exact TextRange(s)
  untouched bytes remain identical

Ruff formatter
  consumes full syntax/source/trivia
  computes canonical presentation
  may change many bytes outside semantic edit intent
```

A robust coding agent pipeline often uses both sequentially:

```text
semantic edit
  -> minimal range replacement
  -> reparse/validate
  -> formatter only if repository policy requires it
  -> reparse/validate final text
```

---

## 10.19 Production formatting policy

Recommended service configuration:

```text
FormatterConfigKey = {
  ruff_frontend_version,
  target_python_version,
  source_type,
  line_width,
  indent_style/width,
  line_ending,
  quote_style,
  magic_trailing_comma,
  preview,
  docstring_code,
  nested_string_quote_style
}
```

Formatting must be deterministic for a given source + configuration key.

If repository settings come from `pyproject.toml`, normalize those settings into your service's formatting request rather than letting an LLM choose them ad hoc.

---

## 10.20 Idempotency test

Formatter invariant:

```text
format(format(source)) == format(source)
```

Add this to integration/golden tests for representative corpus files and every Ruff upgrade.

Also test semantic preservation:

```text
parse(source) and parse(formatted)
  -> equivalent normalized syntax/semantic facts for intended formatter-safe constructs
```

---

## 10.21 Formatter deployment boundary

Recommended workspace:

```text
python-frontend-core
  ruff_source_file
  ruff_python_parser
  ruff_python_ast
  ruff_python_trivia
  ruff_python_index
  optional semantic adapter

python-transform
  ruff_python_codegen
  edit engine

python-format
  ruff_python_formatter
  Ruff config adapter
```

This keeps formatter compile/dependency weight out of analysis-only binaries and allows agents to choose a minimal dependency surface.

---

## 10.22 Anti-pattern inventory

- Calling the formatter a lossless CST printer.
- Running whole-file formatting after every tiny edit without repository policy.
- Assuming `QuoteStyle::Preserve` means preserve all source formatting.
- Enabling preview/docstring formatting opportunistically.
- Passing parsed AST/trivia from a different source revision.
- Formatting invalid source and silently replacing the original on failure.
- Treating formatter output positions as identical to input positions.
- Ignoring `.py` vs `.pyi` source type.
- Depending on `ruff_db` if a direct source API is sufficient.
- Using formatter solely to emit one expression when codegen is a lighter fit.

---

## 10.23 Agent checklist

```text
[ ] Use format_module_source for simple whole-file formatting.
[ ] Use format_module_ast when parse/trivia state already exists.
[ ] Build TriviaRanges from the exact same Parsed tokens/source revision.
[ ] Resolve PyFormatOptions from repository policy, not agent preference.
[ ] Set source type and target Python version correctly.
[ ] Treat formatting as canonicalization, not source preservation.
[ ] Use range formatting when localized formatter churn is required.
[ ] Re-query/rebuild source positions after formatting.
[ ] Test formatter idempotency.
[ ] Keep formatting optional/separate from analysis-only deployments.
```

---

# 11) Cross-crate ownership and lifetime model

## 11.0 Mental model: source revision is the root of truth

Every Ruff frontend object is ultimately meaningful relative to a particular source revision.

```text
FileRevision
  ├─ Arc<str> source
  ├─ LineIndex / SourceFile
  ├─ Parsed<ModModule>
  │    ├─ AST nodes with TextRange
  │    └─ Tokens with TextRange
  ├─ TriviaRanges
  ├─ Indexer
  ├─ ephemeral SemanticModel/traversal state
  └─ normalized persistent facts
```

The invariant is stronger than ordinary Rust borrowing:

```text
A TextRange from revision N is semantically invalid against revision N+1
unless you explicitly map/rebase it.
```

Even if the numeric range still fits inside the new string, it may now point to different text.

---

## 11.1 Recommended file snapshot abstraction

Build your own owning snapshot around Ruff:

```rust
use std::sync::Arc;

use ruff_python_ast::ModModule;
use ruff_python_index::Indexer;
use ruff_python_parser::Parsed;
use ruff_python_trivia::TriviaRanges;
use ruff_source_file::LineIndex;

pub struct PythonFileSnapshot {
    pub file_id: FileId,
    pub revision: FileRevision,
    pub source: Arc<str>,
    pub line_index: LineIndex,
    pub parsed: Parsed<ModModule>,
    pub trivia: TriviaRanges,
    pub indexer: Indexer,
}
```

Depending on exact trait implementations and the ownership model you choose, you may prefer to build derived pieces inside an analysis closure rather than storing every Ruff value in one struct. The architectural point is the same: **all derived data must share one revision identity.**

---

## 11.2 Stable vs ephemeral data

| Data | Recommended lifetime | Persist? |
|---|---:|---:|
| raw source bytes/text | file revision | Yes if repository snapshot store needs it |
| content hash | file revision | Yes |
| `LineIndex` | file revision | Usually cache/recompute |
| `Parsed<T>` | active analysis/query window | Usually no |
| Ruff AST nodes | active analysis/query window | No as public wire model |
| parser tokens | active source-aware query window | Usually no |
| `TriviaRanges` | file revision | Usually recompute |
| `Indexer` | file revision | Usually recompute |
| `SemanticModel` | semantic traversal/query window | No |
| normalized syntax/semantic facts | repository graph revision | **Yes** |
| Pyrefly result facts | type-project revision | **Yes/cache** with provenance |
| formatter output | final edit revision | Persist only as source result |

---

## 11.3 Why normalized facts should own strings/IDs

Ruff AST and semantic structures often borrow from the parsed/source lifetime. Persistent CPG facts should not.

Bad boundary:

```rust
pub struct PersistentDefinition<'a> {
    pub name: &'a str,
    pub ruff_binding: BindingId,
}
```

Better:

```rust
pub struct PersistentDefinition {
    pub id: SymbolId,
    pub name: Box<str>,
    pub file_id: FileId,
    pub revision: FileRevision,
    pub span: ByteSpan,
    pub kind: DefinitionKind,
}
```

This also decouples graph schema migrations from Ruff crate upgrades.

---

## 11.4 Range ownership

Define a range with explicit file/revision provenance:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SourceSpan {
    pub file_id: FileId,
    pub revision: FileRevision,
    pub start: u32,
    pub end: u32,
}
```

Do not pass around a bare `(u32, u32)` where it can outlive the parse snapshot.

For LSP-facing APIs, convert to line/character only at the transport boundary; keep byte ranges internally because that is Ruff's native coordinate system.

---

## 11.5 Dependency direction

Recommended dependency graph:

```text
platform graph/schema
        ▲
        │ normalized facts only
        │
python_frontend_adapter
  ├─ ruff_source_file
  ├─ ruff_python_parser
  ├─ ruff_python_ast
  ├─ ruff_python_trivia
  ├─ ruff_python_index
  └─ ruff_python_semantic (optional/internal adapter)

python_transform_adapter
  └─ ruff_python_codegen

python_format_adapter
  └─ ruff_python_formatter
```

Avoid:

```text
200 application modules -> direct imports of ruff_python_ast::*
```

because every internal API change then becomes a workspace-wide migration.

---

## 11.6 Agent checklist

```text
[ ] Make source revision identity explicit.
[ ] Keep Ruff ranges tied to exact source revision.
[ ] Normalize borrowed/internal Ruff data before persistence.
[ ] Do not persist arena/node IDs as stable identifiers.
[ ] Convert byte offsets to LSP positions only at API boundaries.
[ ] Centralize Ruff dependencies behind frontend/transform/format adapters.
```

---

# 12) Source-preserving edits vs AST regeneration

## 12.0 Why this layer is necessary

LibCST's defining convenience is not merely parsing Python; it owns enough concrete syntax that modifying one node can often preserve unrelated formatting/comments automatically.

The Ruff stack does not have one object with that behavior. To reproduce the **useful source-preserving property** without a monolithic CST, use an explicit edit engine:

```text
AST/semantic analysis determines WHAT should change.
Trivia/index/source analysis determines SAFE EDIT BOUNDARIES.
Codegen optionally determines replacement fragment text.
Edit engine applies TextRange replacements to ORIGINAL SOURCE.
Parser validates final Python.
Formatter optionally canonicalizes final source.
```

---

## 12.1 Canonical edit type

```rust
use ruff_text_size::TextRange;

#[derive(Debug, Clone)]
pub struct Edit {
    pub range: TextRange,
    pub replacement: String,
}
```

Production version should also include:

```text
file ID
source revision/content hash
edit reason/rule ID
confidence/applicability
expected old text hash or old text
```

Example:

```rust
pub struct CheckedEdit {
    pub file_id: FileId,
    pub revision: FileRevision,
    pub range: TextRange,
    pub expected_old_text: Box<str>,
    pub replacement: String,
    pub origin: EditOrigin,
}
```

---

## 12.2 Apply edits from right to left

For non-overlapping edits against one source revision, sort descending by start offset and replace from right to left so earlier byte offsets remain valid.

```rust
use ruff_text_size::TextRange;

#[derive(Debug)]
struct Edit {
    range: TextRange,
    replacement: String,
}

fn apply_edits(source: &str, mut edits: Vec<Edit>) -> Result<String, &'static str> {
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start()));

    let mut previous_start = source.len();
    for edit in &edits {
        let start = usize::from(edit.range.start());
        let end = usize::from(edit.range.end());

        if start > end || end > source.len() {
            return Err("edit range outside source");
        }
        if end > previous_start {
            return Err("overlapping edits");
        }
        if !source.is_char_boundary(start) || !source.is_char_boundary(end) {
            return Err("edit range is not a UTF-8 boundary");
        }
        previous_start = start;
    }

    let mut output = source.to_owned();
    for edit in edits {
        let start = usize::from(edit.range.start());
        let end = usize::from(edit.range.end());
        output.replace_range(start..end, &edit.replacement);
    }
    Ok(output)
}
```

Ruff's ranges originate from UTF-8 source tokenization and should lie on valid boundaries; still validate externally supplied or rebased edits.

---

## 12.3 Detect overlapping edits before mutation

Two individually valid fixes can conflict:

```text
Edit A: replace 100..130
Edit B: replace 120..125
```

Do not let ordering decide silently which fix wins.

Conflict policy options:

```text
REJECT_ALL_OVERLAPS
  safest for agent-generated edits

PRIORITY
  accept higher-confidence/rule-priority edit

MERGE_SEMANTICALLY
  re-run second transformation on first edit result
  safest only when transformation engine supports rebasing
```

For autonomous coding agents, prefer explicit conflict resolution over byte-level merge tricks.

---

## 12.4 Expected-old-text guard

Before applying an edit, verify the source slice still matches what the analysis saw:

```rust
fn verify_edit(source: &str, edit: &CheckedEdit) -> bool {
    let actual = &source[edit.range];
    actual == &*edit.expected_old_text
}
```

This catches stale edits when:

- another agent modified the file;
- editor text changed after analysis;
- a prior transformation altered an overlapping region;
- ranges came from a different source revision.

---

## 12.5 Source replacement strategies

### Strategy A — literal range replacement

Best when replacement syntax is simple and fully known:

```python
old_api(x)
```

→

```python
new_api(x)
```

Replace only the callee name range if possible, not the whole call.

### Strategy B — AST-generated fragment

Best when new syntax is structurally complex:

```text
build replacement Expr
  -> Generator::expr
  -> replace original Expr TextRange
```

### Strategy C — statement-level source template

Best when preserving comments/inner expressions but rearranging outer syntax:

```text
slice child source fragments from original
compose known safe statement template
replace one statement range
```

### Strategy D — whole-file formatter

Use only when canonical formatting is an intended postcondition.

---

## 12.6 Narrowest-correct-range principle

Prefer the smallest edit that expresses the semantic change without creating invalid syntax.

Example:

```python
requests.get(url, timeout=5)
```

If only changing module alias:

```text
replace `requests` range
not entire call range
not entire statement line
not entire file
```

Benefits:

- comments preserved automatically;
- user formatting preserved;
- merge conflicts reduced;
- diff review easier;
- less risk of string/indentation regressions.

---

## 12.7 When to widen the edit

Widen only when required by grammar/source ownership:

```text
renaming identifier                 -> identifier range
changing operator                   -> operator/token range if safely located
replacing expression                -> Expr range
changing statement form             -> Stmt range
moving a decorated definition       -> include decorators
removing statement on shared line   -> include only exact separator-aware segment
removing declaration + comments     -> deliberately include attached comment block
```

Trivia/index helpers should inform the widening decision.

---

## 12.8 Comment-safe deletion

Deleting:

```python
x = unused()  # why this exists
```

requires a policy:

```text
Delete statement only?
  leaves comment, possibly nonsensical

Delete EOL comment too?
  may lose valuable rationale

Migrate comment to next statement?
  changes attachment

Require human review?
  reasonable for ambiguous comment semantics
```

Agents should not automatically treat comment presence as “delete it too.”

---

## 12.9 Multi-statement line deletion

Example:

```python
x = 1; y = 2; z = 3
```

Deleting `y = 2` is not equivalent to deleting its physical line. The safe edit must account for one adjacent semicolon and surrounding whitespace.

Algorithm shape:

```text
1. indexer.in_multi_statement_line(stmt)
2. inspect tokens before/after exact Stmt range
3. choose leading or trailing semicolon segment
4. preserve neighbors
5. reparse
```

Do not use `source.line_range(stmt.start())` as the deletion range.

---

## 12.10 Indentation-sensitive insertion

When inserting a statement into a function/class/conditional body:

```text
find target body indentation from source
or derive style using Stylist
emit generated statement
indent multiline replacement consistently
insert at statement boundary
reparse
```

Avoid inferring indentation solely from AST depth because tabs/spaces and continuation indentation are source-level choices.

---

## 12.11 Edit validation ladder

Every automated edit should move through increasingly expensive gates:

```text
Gate 1 — range validity
  source revision matches
  expected old text matches
  no invalid overlaps

Gate 2 — lexical/source safety
  comments/pragmas considered
  multiline/continuation/multi-statement context considered

Gate 3 — Ruff parse
  final file has expected syntax validity

Gate 4 — structural assertion
  intended AST change occurred
  unrelated key AST facts remain

Gate 5 — Pyrefly/type checks
  no new relevant type diagnostics
  target resolution matches intent

Gate 6 — project tests/linters
  when available
```

An LLM should not be asked to “visually inspect” its own edit as the primary correctness check when deterministic validation exists.

---

## 12.12 Formatter placement

Recommended order:

```text
analyze original
  -> compute minimal edits
  -> apply edits
  -> parse edited text
  -> optionally format edited text
  -> parse formatted text
  -> semantic/type validation
  -> publish
```

Formatting before syntax validation can obscure whether a failure came from the edit or formatter.

---

## 12.13 Anti-pattern inventory

- Rebuilding a whole file from AST for a one-token change.
- Applying overlapping edits in arbitrary order.
- Applying an edit without checking source revision/expected old text.
- Deleting physical lines instead of syntax ranges.
- Ignoring comments and pragmas inside/wrapped around edits.
- Modifying source then reusing old AST/trivia/index ranges.
- Assuming formatter will repair semantically invalid generated code.
- Trusting an LLM's self-assessment instead of reparsing/type-checking.

---

## 12.14 Agent checklist

```text
[ ] Compute edits against immutable original source revision.
[ ] Prefer the narrowest syntactically correct TextRange.
[ ] Record expected old text or source hash.
[ ] Reject/resolve overlapping edits explicitly.
[ ] Use Indexer/TriviaRanges before line-sensitive edits.
[ ] Use Generator for complex replacement fragments, not whole-file preservation.
[ ] Apply edits right-to-left.
[ ] Reparse immediately after mutation.
[ ] Rebuild all range-derived state after mutation.
[ ] Format only as an explicit policy step.
[ ] Run Pyrefly/tests after semantic edits.
```

---

# 13) Building a LibCST-replacement analysis stack

## 13.0 Capability mapping

The correct question is not “which Ruff crate replaces LibCST?” but “which combination replaces each LibCST responsibility?”

| LibCST responsibility | Ruff/Rust replacement | Coverage |
|---|---|---|
| Parse Python | `ruff_python_parser` | Strong |
| Typed syntax tree | `ruff_python_ast` | Strong AST, not CST |
| Node byte ranges | AST + `ruff_text_size` | Strong |
| Line/column conversion | `ruff_source_file` | Strong |
| Comments | `ruff_python_trivia` / `ruff_python_index` | Strong range-level support |
| Explicit parentheses/concrete syntax hints | `TriviaRanges` + token/source queries | Partial/targeted rather than full CST |
| Whitespace/indentation | source + trivia helpers + `Stylist` | Strong query/generation support |
| Parent relationships | build during AST traversal / CPG | Straightforward |
| Scope metadata | `ruff_python_semantic` | Strong linter semantics |
| Local binding/reference resolution | `ruff_python_semantic` | Strong for Ruff lint model |
| Qualified import-name resolution | `ruff_python_semantic` | Strong targeted support |
| Full project type resolution | **Pyrefly daemon** | External semantic layer |
| Visitor API | `ruff_python_ast::visitor` | Strong |
| Transformer API | `ruff_python_ast::visitor::transformer` | Strong structural transforms |
| Exact unchanged-text preservation | original source + range edit engine | Strong when edits are narrow |
| AST -> source | `ruff_python_codegen` | Strong unparser |
| Canonical formatting | `ruff_python_formatter` | Strong |
| Incremental error-tolerant parse | Tree-sitter optional | Ruff parser is whole-file |

---

## 13.1 What you gain

Compared with a Python-process LibCST analysis path:

```text
single Rust process
no Python interpreter boundary for syntax extraction
shared TextRange coordinate model
fast parser/AST stack used by Ruff itself
source/trivia primitives optimized for lint fixes
native formatter/codegen integration
clean composition with Rust CPG storage
```

---

## 13.2 What you do not get automatically

LibCST users may be accustomed to:

```text
every whitespace/comment represented in concrete tree
metadata providers available as a unified framework
modifying tree then codegen preserving untouched concrete syntax
```

In the Ruff decomposition, you must explicitly keep:

```text
original source
AST
trivia/index
your own metadata graph
```

and implement edits as ranges.

This is more plumbing but often a better architecture for code intelligence because the persistent graph is no longer coupled to a source-reconstruction tree.

---

## 13.3 Recommended metadata normalization

Instead of recreating LibCST metadata providers one-for-one, normalize only facts your platform needs:

```text
ParentMap
ScopeGraph
BindingMap
ImportMap
QualifiedNameMap
CommentAttachmentHints
NodeSourceSpanMap
TypeMap          <- Pyrefly
DefinitionMap    <- Pyrefly + Ruff
```

Persist these in your CPG/data substrate, not in a parser-specific metadata wrapper.

---

## 13.4 Parent provider replacement

During traversal:

```rust
struct ParentMap {
    // stable normalized node ID -> stable normalized parent node ID
}
```

When entering a node:

```text
create/lookup normalized node ID
if parent stack nonempty -> emit CONTAINS(parent, child)
push node
walk children
pop node
```

This is generally cheaper and more useful than keeping a parent pointer inside every AST node.

---

## 13.5 Position provider replacement

AST `TextRange` + `LineIndex` replaces position metadata:

```text
node.range() -> byte span
LineIndex     -> user/editor line + character offset
```

Persist byte ranges as canonical; derive line/column views on demand.

---

## 13.6 Qualified-name provider replacement

Combine:

```text
Ruff local/import qualified-name resolution
+ Pyrefly project definition/type resolution
```

Store both provenance layers rather than choosing one and discarding the other.

Example:

```text
Call target syntax: np.array
Ruff import-qualified: numpy.array
Pyrefly resolved definition: <site-package/stub symbol ID>
```

This is richer than a single “qualified name” string.

---

## 13.7 Scope provider replacement

Use Ruff semantic scopes when your semantics adapter is enabled. Otherwise, Pyrefly definition/reference queries can provide a project-level alternative, but keeping Ruff's local binding events is still useful for immediate per-file extraction and linter-style reasoning.

---

## 13.8 Codemod replacement

LibCST codemod pattern:

```text
match CST node
return modified CST node
module.code
```

Recommended Ruff pattern:

```text
match AST + semantic facts
compute replacement AST/text
inspect trivia/index context
emit CheckedEdit<TextRange>
apply to original source
reparse
format optionally
```

This should be exposed as your own transformation SDK so agents do not need to understand every low-level Ruff crate for each fix.

---

## 13.9 Agent-facing transformation API

Example platform API:

```rust
pub trait PythonTransform {
    fn analyze(&self, file: &PythonAnalysis<'_>, out: &mut Vec<ProposedEdit>);
}

pub struct PythonAnalysis<'a> {
    pub source: &'a str,
    pub module: &'a ruff_python_ast::ModModule,
    pub trivia: &'a ruff_python_trivia::TriviaRanges,
    pub indexer: &'a ruff_python_index::Indexer,
    pub facts: &'a NormalizedPythonFacts,
}
```

The LLM selects or implements transformation logic; the platform owns edit validation/application.

---

## 13.10 LibCST migration strategy

Migrate capability by capability rather than replacing the entire system in one step:

```text
Phase 1
  Ruff parser/AST for read-only extraction
  compare facts against LibCST

Phase 2
  Ruff source/trivia/index for positions/comments
  retire LibCST metadata providers used only for analysis

Phase 3
  Pyrefly for type/definition semantic enrichment
  normalize graph schema

Phase 4
  range-edit engine + Ruff codegen for transformations
  differential-test against existing codemods

Phase 5
  Ruff formatter integration
  retire Python-process LibCST path
```

---

## 13.11 Differential validation

During migration, run both frontends on a representative corpus:

```text
LibCST facts
Ruff-stack facts
    -> normalized comparison
```

Compare:

```text
function/class/import counts
identifier/reference spans
scope membership
qualified names
comments near transformed nodes
edit outputs
parse success
Pyrefly diagnostics
```

Expected differences should be explicitly documented rather than patched with ad hoc exceptions.

---

## 13.12 Agent checklist

```text
[ ] Map LibCST capabilities to separate Ruff/source/type layers.
[ ] Do not search for a mythical single Ruff CST replacement crate.
[ ] Keep original source as preservation authority.
[ ] Normalize metadata into your graph rather than parser-provider objects.
[ ] Use Ruff AST for structure, trivia/index for concrete source facts.
[ ] Use Pyrefly for type/project semantics.
[ ] Replace codemods with validated range edits.
[ ] Differential-test before retiring the LibCST path.
```

---

# 14) Code-intelligence / CPG deployment pattern

## 14.0 Recommended frontend architecture

```text
                    FILE WATCHER / GIT SNAPSHOT
                              │
                              ▼
                        source revision
                              │
               ┌──────────────┴──────────────┐
               │                             │
               ▼                             ▼
       optional Tree-sitter          Ruff source/parser
       changed-range oracle           │
       error-tolerant tree            ├─ LineIndex
               │                      ├─ Parsed AST/tokens
               │                      ├─ TriviaRanges
               │                      └─ Indexer
               │                             │
               └──────────────┬──────────────┘
                              ▼
                       syntax extractor
                              │
                    normalized local facts
                              │
               ┌──────────────┴──────────────┐
               ▼                             ▼
       Ruff semantic adapter          Pyrefly daemon
       local bindings/imports         types/definitions
               │                             │
               └──────────────┬──────────────┘
                              ▼
                         fact merger
                              │
                              ▼
                       CPG transaction
```

---

## 14.1 Why normalize before the CPG transaction

Do not let graph-writing code traverse Ruff AST directly while mutating the graph. Instead:

```text
parse/traverse
  -> immutable fact batch
  -> validate/dedupe
  -> one graph transaction
```

Benefits:

- parse failure never partially corrupts graph state;
- easy diff between old/new fact batches;
- easy unit tests without database;
- parser upgrades isolated from persistence;
- atomic file-level replacement.

---

## 14.2 Normalized fact families

Suggested local fact set:

```text
FileFact
NodeFact
ContainsEdge
DefinitionFact
ScopeFact
BindingFact
ReferenceFact
ImportFact
CallSiteFact
InheritanceSyntaxFact
DecoratorFact
AnnotationFact
LiteralFact
CommentFact (only if product needs it)
DiagnosticFact
```

Type-enrichment set:

```text
ResolvedDefinitionEdge
InferredTypeFact
CallableSignatureFact
ResolvedAttributeEdge
ResolvedCallTargetEdge
Subtype/InheritanceSemanticEdge
```

---

## 14.3 Stable node IDs

Do not use pointer addresses or Ruff `NodeId` as persistent IDs.

A practical source-derived ID:

```text
hash(
  language,
  file stable ID,
  node kind,
  start byte,
  end byte,
  optional local discriminator/content hash
)
```

For identity that should survive line insertions, consider symbol identity separately from syntax-node identity:

```text
DefinitionStableId
  module identity + qualified lexical path + kind + signature discriminator
```

Then link each revision's syntax node to the stable symbol.

---

## 14.4 File-level replacement transaction

Recommended graph update:

```text
BEGIN
  mark file revision N+1 staging
  insert new facts/edges
  validate referential invariants
  swap active file revision N -> N+1
  delete/GC stale N facts asynchronously or in transaction
COMMIT
```

If the data store supports immutable snapshots, append the revision and atomically update the active revision pointer.

Never delete old file facts before the new parse/extraction succeeds.

---

## 14.5 Semantic enrichment can be asynchronous in architecture, but not logically unversioned

A type daemon may return after syntax extraction. Tag every result with the file/project revision it was computed against.

Conceptually:

```text
syntax revision = abc123
Pyrefly project revision = abc123
-> merge allowed

syntax revision = def456
Pyrefly result = abc123
-> stale; reject/requery/rebase by explicit policy
```

The implementation may run stages concurrently, but a published graph must never combine untracked stale semantic results with new source.

---

## 14.6 Call graph construction

Use a tiered call target model:

```text
Tier 0 syntactic
  Call node -> callee Expr

Tier 1 lexical/import-qualified
  Ruff semantic qualified name

Tier 2 type-resolved
  Pyrefly callable/definition result

Tier 3 dynamic heuristic
  framework/plugin reflection heuristics
```

Store candidate targets and confidence rather than forcing one target.

Example:

```python
obj.process(x)
```

AST tells you the call shape; Ruff local semantics may resolve `obj` only partially; Pyrefly may determine `obj: Processor`; dynamic subclassing may still make multiple runtime targets possible.

---

## 14.7 Import graph

Build import edges directly from AST/local semantics; do not wait for full typing.

```text
module A
  IMPORTS module B
  IMPORTS B.symbol
```

Then project resolver/Pyrefly can connect those names to physical files/stubs/packages.

---

## 14.8 Comment policy in the CPG

Do not store every comment by default unless your product needs comment search, doc-context retrieval, suppression reasoning, or source-preserving transforms.

Three tiers:

```text
Tier 1
  keep comments only in source; query via ranges

Tier 2
  persist comments associated with declarations/doc context

Tier 3
  persist all comment nodes/ranges for semantic search
```

A code-intelligence product for LLMs may benefit from Tier 2 because rationale/TODO comments near definitions are useful retrieval context without bloating the graph with every formatter comment.

---

## 14.9 Frontend service API

Example request:

```rust
pub struct AnalyzePythonRequest {
    pub file_id: FileId,
    pub revision: FileRevision,
    pub source: Arc<str>,
    pub source_type: PySourceType,
    pub target_version: PythonVersion,
}
```

Response:

```rust
pub struct AnalyzePythonResponse {
    pub facts: PythonFactBatch,
    pub parse_diagnostics: Vec<FrontendDiagnostic>,
    pub semantic_status: SemanticStatus,
    pub source_hash: ContentHash,
}
```

Do not return raw Ruff AST over IPC unless you intentionally make its unstable schema part of your service contract.

---

## 14.10 Agent checklist

```text
[ ] Normalize Ruff output before graph writes.
[ ] Use atomic file-revision transactions.
[ ] Never persist raw Ruff node/arena IDs.
[ ] Version every Pyrefly/type result against source/project revision.
[ ] Build call targets in confidence tiers.
[ ] Preserve unresolved edges rather than guessing.
[ ] Keep parser/semantic adapter behind one service/crate boundary.
[ ] Publish graph revision only after validation succeeds.
```

---

# 15) Incrementality and cache boundaries

## 15.0 Ruff parser is not an incremental parser

`ruff_python_parser` parses source text into a new `Parsed` result. It does not expose Tree-sitter-style edit application to mutate an old parse tree into a new one.

Therefore “incremental Ruff frontend” means **incremental at the file/fact/system level**, not necessarily incremental AST mutation.

---

## 15.1 Recommended cache hierarchy

```text
Repository revision
  └─ File content hash
       ├─ Source/LineIndex cache
       ├─ Parsed AST/tokens cache
       ├─ Trivia/Indexer cache
       ├─ normalized syntax fact cache
       ├─ semantic fact cache
       └─ Pyrefly enrichment cache
```

A file with unchanged content does not need reparsing just because another unrelated file changed, unless project semantic results for it depend on the changed file.

---

## 15.2 Content-addressed parse cache

Cache key:

```text
(language=python,
 content_hash,
 source_type,
 target_python_version,
 ruff_frontend_version)
```

Why include target version:

- unsupported-syntax diagnostics are version-dependent;
- AST/parser behavior can have version gates.

Why include Ruff version:

- internal AST shape/API and parsing behavior can change across releases.

---

## 15.3 Derived-cache invalidation

```text
source changes
  -> invalidate LineIndex
  -> invalidate Parsed
  -> invalidate TriviaRanges
  -> invalidate Indexer
  -> invalidate local semantic facts
  -> invalidate local normalized facts
  -> notify Pyrefly/project semantic layer
```

Do not attempt to keep old range-based derived structures without explicit edit/rebase logic.

---

## 15.4 File-fact diff

After reparsing a changed file:

```text
old normalized fact batch
new normalized fact batch
  -> hash/sort by stable fact key
  -> added / removed / changed
  -> graph delta
```

This is where most of the incremental win should live.

Even if parsing the whole changed file is cheap, replacing the entire repository graph is not.

---

## 15.5 Optional Tree-sitter changed-range oracle

If files are large or editor updates extremely frequent:

```text
edit event
  -> Tree-sitter incremental reparse
  -> changed ranges
  -> decide whether Ruff full parse is needed now
  -> debounce/coalesce changes
  -> Ruff authoritative snapshot on stable/clean interval
```

Tree-sitter and Ruff can coexist:

```text
Tree-sitter = responsiveness/error tolerance/change localization
Ruff = canonical Python AST/token/source semantics for committed extraction snapshot
```

---

## 15.6 Debouncing editor events

Do not run heavy semantic enrichment for every keystroke if the service is consuming live editor changes.

Suggested stages:

```text
0-50 ms  local incremental syntax UI path
100-300 ms debounced Ruff parse/extraction
300+ ms project type/semantic refresh as daemon supports
```

Exact timings are workload-dependent; benchmark rather than hard-code these example ranges as universal.

For Git/CI/indexing workloads, no debounce is needed: analyze immutable file snapshots directly.

---

## 15.7 Parse-once rule

Within one analysis pass:

Bad:

```text
parser for AST extractor
parser again for comments
parser again for formatter
parser again for import extractor
```

Better:

```text
one Parsed result
  -> AST extractor
  -> tokens -> TriviaRanges
  -> tokens -> Indexer
  -> formatter AST path if needed
```

---

## 15.8 Semantic cache boundary

Because Ruff semantic state is built through traversal and can borrow AST/source, cache **normalized semantic facts**, not the `SemanticModel` as a long-lived cross-revision artifact.

```text
file hash + semantic-adapter version + config
  -> normalized scopes/bindings/references/import-qualified facts
```

---

## 15.9 Pyrefly cache boundary

Treat Pyrefly as a project-semantic service with its own incremental state. Do not duplicate its internal type cache in your Ruff frontend.

Cache only query results that are expensive/reused and always tag them with:

```text
project identity
source revision or daemon snapshot ID
query kind
source location/symbol key
Pyrefly version/config
```

---

## 15.10 Anti-pattern inventory

- Calling whole-file Ruff parsing “non-incremental system architecture.”
- Persisting stale AST/trivia objects to avoid a cheap reparse.
- Reparsing the same source separately for every extractor.
- Recomputing unchanged file syntax facts after unrelated repository changes.
- Applying old semantic/type results to new source without version checks.
- Attempting to patch `Indexer` manually after arbitrary source edits without measurement/need.
- Building a second type-cache layer that fights the Pyrefly daemon's own incrementality.

---

## 15.11 Agent checklist

```text
[ ] Key frontend caches by content + config + Ruff version.
[ ] Parse each source revision once per analysis pipeline.
[ ] Rebuild range-derived state after edits.
[ ] Diff normalized facts for graph incrementality.
[ ] Reuse unchanged-file fact batches.
[ ] Version semantic/type results.
[ ] Use Tree-sitter only where incremental/error-tolerant behavior provides value.
[ ] Let Pyrefly own project type incrementality.
```

---

# 16) Error tolerance and partially invalid Python

## 16.0 Two parser modes of operation

The parser exposes both strict convenience functions and unchecked/recovering output.

Strict:

```text
parse_module(source) -> Result<Parsed<ModModule>, ParseError>
parse(source, options) -> Result<Parsed<Mod>, ParseError>
```

Recovering:

```text
parse_unchecked(source, options) -> Parsed<Mod>
parse_unchecked_source(...)
parse_cells_unchecked(...)
```

`Parsed<T>` retains:

```text
syntax
tokens
errors
unsupported_syntax_errors
```

---

## 16.1 Syntax error vs unsupported syntax

`Parsed` distinguishes ordinary parse errors from version-related unsupported syntax errors.

Important methods:

```text
has_valid_syntax()
has_invalid_syntax()
has_no_syntax_errors()
has_syntax_errors()
errors()
unsupported_syntax_errors()
```

Do not use only `has_valid_syntax()` if your requirement is “valid for configured target Python version.”

---

## 16.2 Live-editor policy

For live/incomplete source:

```text
parse_unchecked
  -> retain recoverable AST/tokens
  -> emit parse diagnostics
  -> mark affected facts LOW_CONFIDENCE/INCOMPLETE
  -> preserve prior semantic facts only by explicit stale-data policy
```

A function body half-typed by the user should not necessarily wipe the entire file from the graph.

---

## 16.3 CI/committed-source policy

For repository indexing of committed source, a stricter default is reasonable:

```text
strict parse succeeds
  -> publish authoritative facts

strict parse fails
  -> store parse diagnostic
  -> optionally publish partial syntax facts in a quarantined/incomplete state
  -> do not replace prior authoritative semantic snapshot unless policy says so
```

---

## 16.4 Stale-good snapshot strategy

For editor code intelligence:

```text
revision N: valid -> authoritative semantic facts
revision N+1: temporarily invalid -> current syntax partial + semantic facts N marked stale
revision N+2: valid -> replace with authoritative N+2
```

Never present stale facts as if they belong to N+1 without a stale marker.

---

## 16.5 Notebook cells

`parse_cells_unchecked` exists specifically to parse independent source ranges, such as notebook cells, while preserving absolute offsets into a concatenated source representation.

This avoids a common notebook bug: treating all cells as one normal Python module where syntax boundaries and error attribution can become wrong.

If notebooks are in scope, model:

```text
notebook ID
cell ID/index
cell source range in synthesized source
cell-local line
absolute byte offset
```

`ruff_source_file::SourceRow` also distinguishes source-file rows from notebook cell/line coordinates.

---

## 16.6 Error propagation policy

A robust frontend should return diagnostics, not panic:

```rust
pub struct FrontendResult {
    pub facts: PythonFactBatch,
    pub diagnostics: Vec<FrontendDiagnostic>,
    pub completeness: Completeness,
}
```

`Completeness` might be:

```text
Complete
SyntaxRecovered
UnsupportedTargetSyntax
SemanticPartial
TypePending
Failed
```

---

## 16.7 Agent checklist

```text
[ ] Choose strict vs unchecked parse according to workload.
[ ] Distinguish parse errors from target-version unsupported syntax.
[ ] Preserve diagnostics as first-class output.
[ ] Mark partial/stale facts explicitly.
[ ] Do not erase a good graph snapshot because of one transient editor parse error.
[ ] Use cell-aware parsing for notebooks.
[ ] Never panic on user source syntax errors.
```

---

# 17) Performance and allocation strategy

## 17.0 Performance objective

For code intelligence, optimize **end-to-end freshness and query utility**, not parser microbenchmarks in isolation.

Pipeline latency:

```text
read source
+ parse
+ trivia/index
+ AST extraction
+ semantic traversal
+ Pyrefly query/enrichment
+ fact diff
+ graph transaction
```

A 2× faster parser matters little if graph writes or type queries dominate.

---

## 17.1 Parse source by reference

Keep source in an owning `Arc<str>` or equivalent and pass `&str` through Ruff layers. Avoid creating substring `String`s during traversal.

Bad:

```rust
let node_text = source[node.range()].to_string();
```

unless the text must outlive the snapshot.

Better inside the pass:

```rust
let node_text: &str = &source[node.range()];
```

Normalize/copy only values that enter the persistent fact batch.

---

## 17.2 Parse once, derive many

As in §15:

```text
Parsed
  ├─ AST traversal
  ├─ tokens -> trivia
  ├─ tokens -> Indexer
  └─ tokens/source -> Stylist if transforming
```

This is one of the simplest and highest-confidence performance wins.

---

## 17.3 Avoid premature persistence of frontend internals

Serializing AST/tokens/trivia can cost more CPU/storage than reparsing source. Benchmark before adding an AST cache on disk.

Prefer persistent normalized facts whose extraction is semantically valuable regardless of parser implementation.

---

## 17.4 `LineIndex` caching

Create `LineIndex` lazily or once per source revision. Do not rescan from file start for every diagnostic position.

`SourceFile`'s own design lazily caches its line index via `OnceLock`; if using standalone `LineIndex`, reproduce the one-per-revision discipline.

---

## 17.5 Range-first representation

Store ranges and IDs rather than source copies for local transient analysis:

```text
CallSite {
  range,
  callee_range,
  arg_ranges,
}
```

Copy identifier strings only when they become persistent indexed fields or cross a process boundary.

---

## 17.6 Semantic traversal cost

Do not build the full Ruff linter semantic model if your product only needs syntax + Pyrefly. The semantic adapter should be a measured capability, not mandatory ceremony.

Capability tiers:

```text
Tier 1 syntax
  parser + AST

Tier 2 source-aware syntax
  + trivia + Indexer

Tier 3 local semantic
  + Ruff semantic adapter

Tier 4 project semantic
  + Pyrefly
```

Run only the tiers required for the result being requested.

---

## 17.7 Formatter isolation

Formatting has materially more dependencies and work than syntax extraction. Keep it off the indexing hot path.

```text
indexing request -> never format
edit request     -> format only after accepted edit when policy requires
```

---

## 17.8 Batch graph writes

Emit a per-file fact batch and write it transactionally rather than one node/edge at a time during AST traversal.

Good:

```text
10k extracted facts in memory
  -> one bulk transaction
```

Bad:

```text
AST node
  -> synchronous DB RPC
  -> next AST node
  -> synchronous DB RPC
```

---

## 17.9 String interning

High-frequency values such as:

```text
identifier names
module names
qualified names
node kinds
file paths
```

may benefit from interning/deduplicated storage in the normalized graph layer. Do not mutate Ruff's AST representation to implement your persistence interning strategy.

---

## 17.10 Release builds and CPU tuning

Benchmark in optimized builds. Ruff itself is performance-oriented Rust; debug build behavior is not representative.

Typical benchmark commands:

```bash
cargo build --release
cargo test --release -p python-ruff-frontend
cargo bench -p python-ruff-frontend
```

Only enable aggressive CPU-specific build flags after verifying deployment CPU compatibility.

---

## 17.11 Performance telemetry

Measure at least:

```text
source bytes
parse ms
AST node count
trivia/index ms
semantic ms
Pyrefly query/enrichment ms
fact count
fact-diff ms
graph-write ms
peak memory per file/batch
cache hit/miss
```

Without stage telemetry, performance tuning becomes guesswork.

---

## 17.12 Agent checklist

```text
[ ] Keep one source allocation per revision where practical.
[ ] Slice source by range instead of cloning strings in hot traversal.
[ ] Parse once and share Parsed/tokens across derived work.
[ ] Cache LineIndex once per revision.
[ ] Persist normalized facts, not huge parser internals, unless benchmarks justify it.
[ ] Run only required semantic tiers.
[ ] Keep formatter out of indexing hot path.
[ ] Batch graph writes.
[ ] Benchmark release builds with stage-level telemetry.
```

---

# 18) Concurrency and service deployment

## 18.0 Preferred concurrency axis: files

The cleanest parallelism is file-level:

```text
worker 1 -> file A snapshot -> fact batch
worker 2 -> file B snapshot -> fact batch
worker 3 -> file C snapshot -> fact batch
...
```

Each worker owns/borrows a self-consistent source revision and ephemeral Ruff frontend state.

---

## 18.1 Avoid shared mutable semantic state

`SemanticModel` is traversal state. Do not place one mutable semantic model behind a global mutex and feed multiple modules through it.

Correct:

```text
one semantic traversal state per file analysis
```

Shared project data should live in your immutable/resolver/cache layer or Pyrefly daemon, not in one Ruff linter semantic model.

---

## 18.2 Work scheduling

Suggested queues:

```text
parse/extract CPU pool
semantic/type query pool/daemon client
DB write pool
format/transform pool
```

This prevents expensive format or project-semantic operations from blocking basic syntax indexing.

---

## 18.3 Backpressure

Repository-wide reindex can generate fact batches faster than the database can commit them.

Use bounded queues:

```text
file scanner
  -> bounded analysis queue
  -> bounded fact batch queue
  -> graph writers
```

Do not retain every parsed AST in memory while waiting for DB writes; normalize facts and drop frontend state as soon as possible.

---

## 18.4 Cancellation

Analysis request cancellation should drop ephemeral state cleanly:

```text
editor revision N request starts
revision N+1 arrives
  -> cancel/ignore N result
```

The graph layer must verify revision at commit time even if cancellation is best-effort.

---

## 18.5 Duplicate work suppression

Content-addressed scheduler rule:

```text
if same file content hash + config is already in-flight/cached
  -> share result or skip duplicate parse
```

This is valuable when multiple consumers request the same file during repository load.

---

## 18.6 Service process boundary

Two viable deployments:

### Embedded library

```text
Rust code-intel process
  -> Ruff crates in-process
```

Best for lowest latency and simplest ownership.

### Dedicated frontend service

```text
main platform
  -> RPC AnalyzePython(file, revision, text/config)
  -> Rust Ruff frontend service
  -> normalized facts only
```

Best when you want language frontend isolation, independent rollout, or polyglot architecture.

If using RPC, do not serialize Ruff AST as the protocol. Serialize your stable normalized fact schema.

---

## 18.7 Resource isolation

For untrusted/massive repositories, bound:

```text
maximum source file size
maximum concurrent analyses
maximum fact count per file
semantic query timeouts
formatter/edit output size
```

Python permits pathologically large literals/nesting/source files even if parser implementation is robust. Product-level resource policy is still required.

---

## 18.8 Agent checklist

```text
[ ] Parallelize across immutable file snapshots.
[ ] Use one mutable semantic traversal state per file analysis.
[ ] Bound analysis/fact-write queues.
[ ] Drop AST/trivia state after normalization when no longer needed.
[ ] Reject stale results at graph commit time.
[ ] Deduplicate identical in-flight content analyses.
[ ] Serialize normalized facts, not Ruff internals, across RPC.
[ ] Add file-size/concurrency/time/resource limits.
```

---

# 19) Testing and golden/snapshot strategy

## 19.0 Test the normalized contract, not only Ruff calls

Your tests should prove:

```text
source -> expected normalized facts
source + transform -> expected minimal diff
source + format -> expected canonical output
upgrade -> no unintended graph semantic drift
```

A unit test that merely asserts `parse_module(...).is_ok()` does not protect the code-intelligence contract.

---

## 19.1 Parser fixture matrix

Include:

```text
simple modules
async functions
classes/decorators
comprehensions
match/case
generators/yield
walrus expressions
PEP 695 type parameters/type aliases
f-strings including nested/interpolated strings
t-strings if target syntax supports them in pinned parser
.py stubs
IPython syntax when supported
notebook cells
invalid syntax
unsupported target-version syntax
Unicode identifiers
mixed line endings
```

---

## 19.2 Source-range tests

For every important AST fact:

```text
assert range slice == exact expected source
assert line/column conversion == expected
```

Example:

```rust
let name_text = &source[name_range];
assert_eq!(name_text, "target_name");
```

This catches off-by-one, decorator-range, Unicode, and multiline bugs early.

---

## 19.3 Trivia/index tests

Fixture categories:

```text
own-line comment
end-of-line comment
pragma comments
comment block
hash inside strings
multiline strings
nested f-strings
explicit backslash continuations
multiple consecutive continuations
multi-statement semicolon lines
parenthesized vs unparenthesized expressions
CRLF source
```

---

## 19.4 Semantic tests

Validate normalized semantics, not raw arena IDs:

```text
scope kind/owner
binding name/kind/range
reference -> binding relation
shadowing
rebindings/global/nonlocal
imports and aliases
qualified-name resolution
builtin shadowing
unresolved references
branch-sensitive examples
```

---

## 19.5 Pyrefly merge tests

Cases:

```text
import alias -> qualified name -> type definition
instance method call
class method/static method
protocol/interface-like dispatch
overload
union receiver
unresolved Any/dynamic value
external package/stub symbol
```

Assert confidence/provenance, not just target count.

---

## 19.6 Edit golden tests

Each transform fixture should assert:

```text
input source
proposed edits
exact output source
Ruff parse success
expected normalized semantic change
unrelated text remains identical where promised
```

Also test conflict/overlap rejection.

---

## 19.7 Formatter tests

At minimum:

```text
expected canonical output
idempotency
comments retained
.py vs .pyi differences where relevant
line endings
quote style
line width
magic trailing comma
preview off/on if supported
range formatting
```

---

## 19.8 Differential tests against CPython

For parser/codegen-sensitive behavior, consider corpus tests against CPython's `ast.parse`/`compile` for semantic syntax validity where practical.

Do not require identical AST representation; compare a normalized structural projection.

---

## 19.9 Differential tests against old LibCST path

During migration:

```text
same source
  -> LibCST normalized facts
  -> Ruff-stack normalized facts
  -> compare
```

Classify differences:

```text
expected architectural difference
Ruff improvement
LibCST improvement/regression blocker
normalization bug
semantic resolver difference
```

---

## 19.10 Repository corpus tests

Synthetic fixtures are insufficient. Maintain a representative Python corpus including:

```text
large real modules
typed modern code
legacy code
framework-heavy code
scientific/data code
metaprogramming-heavy code
stubs
notebooks if in scope
files with formatting oddities
```

Record:

```text
parse success rate
fact counts
semantic resolution rate
unresolved rate
latency
memory
```

---

## 19.11 Upgrade snapshots

Before upgrading Ruff component crates:

```text
run current frontend on corpus -> baseline facts
upgrade exact release train
run new frontend -> candidate facts
diff normalized outputs
```

Inspect intentional AST/semantic changes rather than blindly updating snapshots.

---

## 19.12 Agent checklist

```text
[ ] Test normalized facts, not raw Ruff debug output only.
[ ] Assert exact source ranges for important nodes.
[ ] Cover comments/continuations/multiline strings/multi-statement lines.
[ ] Test shadowing/global/nonlocal/import aliases.
[ ] Test Pyrefly merge provenance/confidence.
[ ] Golden-test minimal source edits.
[ ] Assert edited source reparses.
[ ] Assert formatter idempotency.
[ ] Run real-repository corpus tests.
[ ] Diff normalized facts before Ruff upgrades.
```

---

# 20) Upgrade and version-migration discipline

## 20.0 Internal `0.0.x` APIs require an explicit upgrade process

All eight requested crates are published as Ruff internal component crates. Their `0.0.7` package descriptions identify them as internal components, and Ruff's docs warn that these APIs are unstable and may experience frequent breaking changes.

Treat an upgrade as a frontend migration, not a routine patch bump.

---

## 20.1 Release-train invariant

For Ruff `0.16.1`:

```text
ruff_python_parser      0.0.7
ruff_python_ast         0.0.7
ruff_python_semantic    0.0.7
ruff_python_trivia      0.0.7
ruff_python_index       0.0.7
ruff_python_codegen     0.0.7
ruff_python_formatter   0.0.7
ruff_source_file        0.0.7
ruff_text_size          0.0.7
```

Do not upgrade one arbitrary crate independently unless Cargo/source inspection proves the dependency graph is designed for that combination.

Recommended workspace policy:

```toml
ruff_python_parser = "=0.0.7"
# ... all other Ruff component dependencies exact-pinned
```

---

## 20.2 Upgrade workflow

```text
1. Choose target Ruff release/tag.
2. Read target root Cargo.toml.
3. Record component crate versions and MSRV/edition.
4. Update all direct Ruff internal dependencies together.
5. cargo update with controlled lockfile diff.
6. cargo check adapter crates first.
7. fix compilation at the Ruff boundary.
8. run unit/golden/corpus tests.
9. diff normalized fact outputs against old frontend.
10. benchmark parse/extraction/semantic stages.
11. inspect formatter diff corpus if formatter upgraded.
12. publish only after graph/output drift is understood.
```

---

## 20.3 Compilation checklist

```text
[ ] Parser entry points still exist/signatures match.
[ ] Parsed<T> accessors still match.
[ ] AST node variants/fields updated.
[ ] Visitor methods/walk functions updated.
[ ] Transformer behavior updated.
[ ] TriviaRange construction/API updated.
[ ] Indexer methods updated.
[ ] SemanticModel construction/traversal adapter updated.
[ ] Binding/Scope/Reference kinds updated.
[ ] Generator public methods updated.
[ ] Formatter entry points/options updated.
[ ] Source/LineIndex position APIs updated.
```

---

## 20.4 Semantic-drift checklist

Compilation success is not enough. Check:

```text
AST ranges for definitions/calls/imports
new Python syntax node variants
parser recovery behavior
unsupported-version diagnostics
comment/parenthesis indexing
continuation/multiline-string indexing
scope creation
binding kinds
qualified-name resolution
builtin recognition
formatter output
codegen output
```

---

## 20.5 Exhaustive matches are deliberate upgrade sentinels

In your adapter, exhaustive matches over important AST enums can be valuable because a new Ruff/Python syntax node should force conscious handling.

For core graph extraction:

```rust
match stmt {
    Stmt::FunctionDef(node) => { /* ... */ }
    Stmt::ClassDef(node) => { /* ... */ }
    // ... explicitly cover semantically relevant variants
    _ => { /* generic fallback only if omission is intentionally safe */ }
}
```

Balance:

```text
exhaustive match
  -> upgrade breaks compilation when syntax surface changes
  -> good for correctness-critical extractors

wildcard fallback
  -> easier upgrades
  -> risk silently ignoring new syntax
```

For an LLM reference system, correctness-critical definition/reference extraction should bias toward explicit handling/tests.

---

## 20.6 New Python version adoption

When Ruff gains support for new Python grammar:

```text
parser may add AST variants/fields
formatter may add new formatting rules
semantic adapter may need new binding rules
Pyrefly version may need aligned grammar/type support
```

Do not raise your advertised target Python version until **all required layers** support it adequately.

---

## 20.7 Lockfile and supply-chain policy

Commit `Cargo.lock` for binaries/services. For controlled production builds:

```text
exact component pins
reviewed lockfile diff
cargo deny/audit as appropriate
reproducible build environment
known Rust toolchain >= pinned Ruff MSRV
```

Ruff `0.16.1` itself declares Rust `1.95` and edition 2024 in the workspace metadata; use that as the baseline compatibility anchor for this reference.

---

## 20.8 Adapter-version provenance

Persist frontend provenance with graph revisions:

```text
python_frontend = {
  ruff_release: "0.16.1",
  component_version: "0.0.7",
  adapter_schema_version: 12,
  target_python: "3.13",
  semantic_adapter_version: 5,
  pyrefly_version: "..."
}
```

This makes it possible to explain why two graph snapshots differ after tooling upgrades.

---

## 20.9 Anti-pattern inventory

- `ruff_python_parser = "0"` or loose floating internal dependencies.
- Upgrading parser but not AST/text-size crates deliberately.
- Treating `cargo check` as the entire upgrade validation.
- Blindly accepting changed snapshots after a Ruff upgrade.
- Keeping wildcard AST matches where new syntax would materially affect facts.
- Advertising a new Python target before semantic/type/formatter layers support it.
- Copying `main` branch code into an older pinned release without checking signatures.

---

## 20.10 Agent checklist

```text
[ ] Resolve the target Ruff tag first.
[ ] Read target root Cargo.toml for component versions/MSRV.
[ ] Upgrade the whole internal release train together.
[ ] Compile only through the adapter boundary first.
[ ] Run normalized-fact corpus diffs.
[ ] Inspect new AST variants/syntax.
[ ] Run formatter/codegen golden diffs.
[ ] Benchmark stage latency/memory.
[ ] Record frontend version provenance in graph metadata.
```

---

# 21) Security and untrusted-source considerations

## 21.0 Parsing source does not execute Python

The Ruff parser/AST/trivia/index stack analyzes text; it does not need to import or execute the analyzed Python program.

This is a major security advantage over reflection/import-based code analysis.

Maintain that boundary:

```text
Do not import target repository modules merely to improve analysis.
Do not run setup.py/pyproject hooks to discover symbols unless sandboxed and explicitly required.
Do not eval annotations/default expressions.
```

---

## 21.1 Resource-exhaustion threat model

Untrusted source can still consume resources through:

```text
very large files
extreme nesting
huge literals
massive generated modules
pathological comment/token counts
large notebooks
repositories with millions of small files
```

Add product-level limits even when parser code is memory-safe Rust.

---

## 21.2 Suggested input limits

Set policy for:

```text
max source bytes per file
max notebook cells / synthesized source bytes
max files per indexing job
max concurrent parse jobs
max normalized facts per file
max edit replacement size
max formatter output size
analysis timeout/deadline
```

Do not hard-code the example values from another system; benchmark representative repositories and expose administrative overrides.

---

## 21.3 Paths are data, not authority

A parser receives source and a path partly for source type/module/semantic context. Do not let analyzed code-controlled strings escape into filesystem operations without validation.

Example:

```python
open("../../secret")
```

is merely an AST call expression. The analyzer should not follow it as a file read.

---

## 21.4 Import resolution

If your project resolver follows imports to disk:

```text
scope allowed repository roots
scope virtualenv/site-package roots
resolve symlinks according to security policy
prevent traversal outside configured roots
separate workspace dependencies from arbitrary local filesystem
```

Pyrefly/project resolver configuration should be treated as part of the security boundary.

---

## 21.5 Code generation and edits

Generated source is not safe merely because it parses.

An LLM transformation could insert:

```python
import os
os.system(...)
```

if its task constraints are wrong. Therefore code-modification authorization belongs above the parser/generator.

Enforce:

```text
allowed file scope
allowed edit operation/rule scope
diff limits
review/approval policy
unit/type/lint validation
Git branch/PR isolation where applicable
```

---

## 21.6 Formatter safety boundary

Formatting should operate only on intended files/source buffers. Do not automatically scan arbitrary filesystem roots from an externally supplied path if the service API already has source text.

Prefer:

```text
format(source_text, resolved_options)
```

rather than:

```text
formatter_service(format_path_from_user)
```

unless filesystem access is explicitly governed.

---

## 21.7 Diagnostics and source privacy

Source excerpts can contain credentials/secrets. Logging policy:

```text
log file ID/path + range + error kind
avoid logging entire source by default
redact generated diagnostics if they include source excerpts in sensitive contexts
protect code-intelligence telemetry/storage as source code data
```

---

## 21.8 Agent checklist

```text
[ ] Never execute/import analyzed Python merely for syntax analysis.
[ ] Bound file/repository size and concurrency.
[ ] Keep import resolution inside configured roots.
[ ] Treat generated edits as privileged actions above the Ruff layer.
[ ] Validate diff scope and final syntax/types/tests.
[ ] Prefer source-buffer formatter APIs over arbitrary filesystem paths in services.
[ ] Do not leak source code into logs unnecessarily.
```

---

# 22) Capability decision tables

## 22.0 “Which crate do I reach for?”

| Need | Primary crate/API | Supporting layer |
|---|---|---|
| Parse a `.py` module | `ruff_python_parser::parse_module` | `ruff_python_ast` |
| Parse one expression | `parse_expression` | AST |
| Parse potentially invalid live source | `parse_unchecked` | diagnostics policy |
| Parse notebook cell ranges | `parse_cells_unchecked` | `ruff_source_file` |
| Inspect functions/classes/calls/imports | `ruff_python_ast` | Visitor |
| Walk all AST nodes | `visitor::Visitor` | `walk_*` |
| Walk in source order | `visitor::source_order` | — |
| Modify AST structure | `visitor::transformer` | codegen/edit engine |
| Get exact AST node byte span | `Ranged::range` | `ruff_text_size` |
| Convert offset to line/column | `ruff_source_file::LineIndex` / `SourceCode` | position encoding |
| Get original source for node | `&source[node.range()]` | exact source revision |
| Find comments | `TriviaRanges`/`CommentRanges` or `Indexer` | source |
| Detect explicit parenthesization | `TriviaRanges::parenthesized` | parser tokens |
| Detect line continuations | `Indexer` | source |
| Detect multiline/interpolated string regions | `Indexer` | tokens |
| Detect multi-statement line | `Indexer::in_multi_statement_line` | trivia |
| Local scope/binding/reference semantics | `ruff_python_semantic` | semantic traversal adapter |
| Import-qualified name | `SemanticModel` | import state |
| Full inferred type | **Pyrefly** | not Ruff semantic crate |
| Cross-module definition | **Pyrefly/project resolver** | local Ruff facts |
| Emit source for expression | `Generator::expr` | `Stylist` |
| Emit source for statement | `Generator::stmt` | `Stylist` |
| Re-unparse existing module | `round_trip` | not lossless |
| Canonically format source | `format_module_source` | `PyFormatOptions` |
| Format already parsed source | `format_module_ast` | `TriviaRanges` |
| Range-format | `format_range` | formatter |
| Preserve untouched bytes exactly | **your edit engine** | original source + Ruff ranges |

---

## 22.1 Analysis-level decision tree

```text
Do you need Python structure?
  no  -> source_file / trivia only if needed
  yes -> parser + AST

Do you need concrete source facts omitted from AST?
  yes -> trivia + Indexer

Do you need local scope/import binding semantics?
  yes -> Ruff semantic adapter

Do you need inferred types or project-level target resolution?
  yes -> Pyrefly daemon

Do you need to mutate source?
  yes -> range edit engine
        + codegen for complex replacement fragments
        + formatter only if canonical formatting is policy
```

---

## 22.2 Source transformation decision table

| Transformation | Recommended strategy |
|---|---|
| rename one identifier with already-resolved exact occurrences | replace identifier ranges |
| rename symbol project-wide | Pyrefly/reference resolver → checked range edits |
| change literal value | replace literal/Expr range; codegen if escaping complex |
| replace function call | build new Expr → `Generator::expr` → range edit |
| insert import | source-aware import insertion logic + generated/import text + reparse |
| remove statement | `Indexer`/comments/separator-aware statement edit |
| reorder statements | source slices or AST generation with explicit comment policy |
| large structural rewrite | AST transform + codegen for target region; optional formatter |
| style-only change | formatter |
| whole-file canonicalization | formatter |

---

## 22.3 Persistence decision table

| Ruff-derived object | Persist directly? | Better persistence form |
|---|---:|---|
| `Parsed<ModModule>` | No | normalized facts + source hash |
| `Stmt`/`Expr` AST | No | node kind/span/facts |
| parser tokens | Usually no | source + optional selected token facts |
| `LineIndex` | No | recompute/cache |
| `TriviaRanges` | No | selected comment/pragma facts if needed |
| `Indexer` | No | recompute from source/tokens |
| `SemanticModel` | No | normalized scopes/bindings/references |
| Ruff arena IDs | **Never as stable IDs** | your stable IDs |
| formatter `Printed` | source output only | final source text |
| codegen output | source fragment | final edit/source |

---

## 22.4 Reliability decision table

| Fact | Typical confidence |
|---|---|
| AST node kind/range | high syntactic confidence on valid parse |
| comment/multiline/continuation range | high lexical confidence |
| local Ruff binding target | high local semantic confidence under model |
| import-qualified name | high lexical/import confidence, not runtime dispatch guarantee |
| Pyrefly definition/type | high static-analysis confidence, subject to dynamic Python/Any |
| dynamic call target | often multi-candidate/heuristic |
| string-based reflection target | heuristic unless framework-specific proof exists |

---

## 22.5 Agent-safe “do not confuse” table

| Do not confuse | With |
|---|---|
| `ruff_python_index::Indexer` | repository symbol index |
| `ruff_python_semantic` | full type checker |
| `ruff_python_codegen` | lossless CST printer |
| `ruff_python_formatter` | minimal-diff editor |
| `TextRange` | stable across source revisions |
| `BindingId` / `ScopeId` | persistent symbol ID |
| AST Visitor evaluation order | source order |
| parser recovery output | fully valid/complete syntax |
| import-qualified name | guaranteed runtime object |

---

# 23) Global anti-pattern inventory

## 23.0 Architecture

- Exposing Ruff internal types throughout the application instead of behind an adapter.
- Treating the eight crates as independent stable libraries with unrelated versioning.
- Persisting Ruff AST/semantic IDs as CPG identity.
- Building the database during AST traversal instead of producing a normalized fact batch.
- Treating type semantics and syntax semantics as one undifferentiated confidence level.

## 23.1 Parsing and source

- Parsing the same source independently for each analysis feature.
- Dropping parser tokens immediately then rescanning source with regex.
- Converting all source slices to owned `String`s during traversal.
- Using line/column as canonical internal coordinates instead of byte ranges.
- Reusing ranges after source mutation.
- Assuming `.py`, `.pyi`, IPython, and notebook cells are identical parse contexts.

## 23.2 AST

- Assuming AST is a CST.
- Inferring comments/optional parentheses from AST alone.
- Forgetting visitor recursion after overriding `visit_*`.
- Using evaluation order when source order is required.
- Ignoring new syntax through broad wildcards in correctness-critical extraction.
- Hand-building expression strings instead of using AST/codegen.

## 23.3 Trivia/index

- Regex-detecting comments.
- Treating `#` in a string as a comment.
- Deleting statements without comment/pragma checks.
- Deleting whole physical lines containing multiple statements.
- Ignoring explicit continuations and multiline strings in line analysis.
- Treating `ruff_python_index` as project indexing infrastructure.

## 23.4 Semantics

- Believing `SemanticModel::new` populates complete semantics by itself.
- Implementing an incomplete copy of Ruff's semantic traversal without tests.
- Calling Ruff semantic helper “type inference” a project type checker.
- Dropping unresolved references.
- Treating star imports as certain.
- Treating qualified lexical names as certain dynamic dispatch targets.
- Ignoring shadowing/global/nonlocal/rebinding.

## 23.5 Edits/codegen

- Reprinting whole files for tiny changes.
- Applying stale or overlapping edits.
- Assuming codegen preserves comments.
- Manually escaping Python strings when AST codegen exists.
- Skipping reparse after edit.
- Using formatter as a syntax repair engine.

## 23.6 Formatter

- Treating formatter as lossless.
- Formatting entire files unexpectedly in a targeted agent patch.
- Using agent-selected style rather than repository configuration.
- Enabling preview/docstring formatting without policy.
- Assuming input/output positions are unchanged.

## 23.7 Deployment

- No file-size/concurrency limits.
- Unbounded AST retention while graph writes lag.
- Sharing mutable semantic state across files.
- Publishing stale type results against a new revision.
- RPC-serializing Ruff AST as a long-lived service contract.
- Upgrading internal crates without normalized corpus diffs.

---

# 24) LLM-agent implementation checklist

## 24.0 Before writing code

```text
[ ] Read this document's version anchor.
[ ] Confirm workspace actually pins Ruff 0.16.1 / component 0.0.7 or update the reference.
[ ] Inspect root Cargo.lock/Cargo.toml before adding dependencies.
[ ] Identify the narrow capability needed: source / parser / AST / trivia / index / semantic / codegen / formatter.
[ ] Prefer existing project adapter abstractions over importing Ruff crates directly.
[ ] Determine target Python version and source type.
```

---

## 24.1 Parser task checklist

```text
[ ] Parse source only once per revision.
[ ] Keep Parsed<T>, not only into_syntax(), if tokens/diagnostics will be needed.
[ ] Choose strict parse_module/parse vs parse_unchecked deliberately.
[ ] Check unsupported_syntax_errors when target-version validity matters.
[ ] Use parse_expression for expression fragments.
[ ] Use cell-aware parsing for notebook cells.
[ ] Never panic on invalid user source.
```

---

## 24.2 AST task checklist

```text
[ ] Use AST enums and visitor helpers rather than regex syntax extraction.
[ ] Use Ranged/TextRange for source provenance.
[ ] Pick evaluation-order vs source-order visitor deliberately.
[ ] Call walk_* unless intentionally pruning traversal.
[ ] Search ruff_python_ast helpers before implementing custom syntax logic.
[ ] Build parent/containment edges into normalized graph during traversal.
[ ] Do not persist Ruff node indices as stable IDs.
```

---

## 24.3 Source/trivia/index checklist

```text
[ ] Build LineIndex once per revision.
[ ] Build TriviaRanges from Parsed::tokens().
[ ] Build Indexer from the same tokens + source.
[ ] Check comments before deleting/replacing declarations.
[ ] Preserve/handle pragma comments explicitly.
[ ] Check multi-statement lines before statement edits.
[ ] Check continuation/multiline/interpolated ranges for physical-line logic.
[ ] Invalidate every range-derived structure after edits.
```

---

## 24.4 Semantic checklist

```text
[ ] Do not assume SemanticModel::new performs the traversal.
[ ] Use the project's semantic adapter if available.
[ ] Preserve local scopes/bindings/references/imports in normalized facts.
[ ] Treat Ruff IDs as ephemeral.
[ ] Preserve unresolved references.
[ ] Attach provenance to qualified-name resolution.
[ ] Query Pyrefly for inferred type/project definitions.
[ ] Never upgrade a heuristic target to “certain” without evidence.
```

---

## 24.5 Edit checklist

```text
[ ] Verify file revision/content hash before applying.
[ ] Use narrowest safe TextRange.
[ ] Store expected old source for stale-edit detection.
[ ] Reject or resolve overlapping edits.
[ ] Generate complex Expr/Stmt replacement with Generator when useful.
[ ] Preserve untouched original source bytes.
[ ] Apply non-overlapping edits right-to-left.
[ ] Reparse final text.
[ ] Rebuild syntax/trivia/index/semantic state.
[ ] Run Pyrefly/tests/linters as required.
```

---

## 24.6 Formatter checklist

```text
[ ] Format only when policy calls for canonical formatting.
[ ] Use repository-resolved PyFormatOptions.
[ ] Correctly set source type and target version.
[ ] Prefer format_module_ast if the same parsed/trivia snapshot already exists.
[ ] Do not expect minimal diffs.
[ ] Re-run diagnostics/positions against final formatted text.
[ ] Test idempotency.
```

---

## 24.7 CPG checklist

```text
[ ] Extract into an immutable fact batch before DB mutation.
[ ] Use stable platform IDs, not Ruff IDs.
[ ] Include file/revision on source spans.
[ ] Commit file graph changes atomically.
[ ] Tag semantic edges with confidence/provenance.
[ ] Merge Pyrefly results only when revision matches.
[ ] Keep unresolved/partial states explicit.
[ ] Diff fact batches for incremental graph updates.
```

---

## 24.8 Upgrade checklist

```text
[ ] Pin target Ruff tag.
[ ] Read target workspace Cargo.toml.
[ ] Upgrade all component crates coherently.
[ ] Update adapter code only; prevent internal types leaking outward.
[ ] Run parser/AST/trivia/semantic/edit/formatter fixtures.
[ ] Run real-repository corpus.
[ ] Diff normalized facts.
[ ] Diff formatter outputs deliberately.
[ ] Benchmark stage latency/memory.
[ ] Record new frontend provenance.
```

---

## 24.9 Agent operating rules — concise promptable version

The following block can be embedded directly into a coding-agent instruction file:

```text
RUFF PYTHON FRONTEND RULES

1. The workspace pins Ruff internal crates as one exact release train. Do not change one Ruff
   component version independently and do not copy APIs from Ruff main without checking the pinned tag.

2. Parse a source revision once. Retain Parsed<T> long enough to reuse its AST, tokens, errors,
   TriviaRanges, and Indexer.

3. AST = semantic syntax structure, not a CST. Use original source + TextRange as the authority for
   exact text. Use trivia/index for comments, parentheses, multiline strings, and continuations.

4. ruff_python_index::Indexer is a per-file lexical omitted-fact index, not a repository/symbol index.

5. ruff_python_semantic is Ruff linter semantic state, not the project type checker. Use Pyrefly for
   inferred types and project-level semantic resolution. Never persist Ruff BindingId/ScopeId/NodeId.

6. For source changes, compute validated range edits against immutable original source. Prefer the
   narrowest correct edit. Use ruff_python_codegen::Generator for complex replacement fragments.
   Do not regenerate an entire file merely to preserve source.

7. ruff_python_formatter is a canonical pretty printer, not a minimal-diff editor. Run it only when
   repository policy requires formatting, with repository-resolved options.

8. Every edit must be reparsed. Semantic edits must run appropriate Pyrefly/type/test validation.
   Every derived range/index/semantic object must be rebuilt for the new source revision.

9. Normalize frontend facts into platform-owned stable types before persistence or RPC. Ruff 0.0.x
   structs/enums/IDs are internal and may break between releases.

10. Attach file revision and provenance/confidence to syntax/semantic/type edges. Never invent a
    semantic target merely to avoid an unresolved graph edge.
```

---

# 25) Reference links and source audit

## 25.0 Pinned workspace anchors

This document is anchored to:

```text
Ruff release/tag: 0.16.1
Ruff internal component version: 0.0.7
Rust edition: 2024
Ruff workspace rust-version/MSRV: 1.95
Snapshot date: 2026-08-18
```

Pinned repository root:

- https://github.com/astral-sh/ruff/tree/0.16.1
- https://github.com/astral-sh/ruff/blob/0.16.1/Cargo.toml

---

## 25.1 `ruff_python_parser`

Docs:

- https://docs.rs/ruff_python_parser/0.0.7/ruff_python_parser/

Pinned source:

- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_parser/Cargo.toml
- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_parser/src/lib.rs
- https://github.com/astral-sh/ruff/tree/0.16.1/crates/ruff_python_parser/src

Important source modules:

```text
lexer
parser
error
semantic_errors
string
token_set
token_source
typing
```

---

## 25.2 `ruff_python_ast`

Docs:

- https://docs.rs/ruff_python_ast/0.0.7/ruff_python_ast/

Pinned source:

- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_ast/Cargo.toml
- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_ast/src/lib.rs
- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_ast/src/visitor.rs

High-value modules to inspect before reinventing logic:

```text
helpers
visitor
visitor/source_order
visitor/transformer
statement_visitor
traversal
find_node
name
identifier
docstrings
operator_precedence
parenthesize
relocate
str / str_prefix
whitespace
token
types
```

---

## 25.3 `ruff_python_semantic`

Docs:

- https://docs.rs/ruff_python_semantic/0.0.7/ruff_python_semantic/

Pinned source:

- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_semantic/Cargo.toml
- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_semantic/src/lib.rs
- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_semantic/src/model.rs

To understand **how the model is populated**, inspect Ruff's pinned AST checker:

- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_linter/src/checkers/ast/mod.rs

This is an important source anchor because the checker explicitly owns the AST traversal that builds the `SemanticModel`.

---

## 25.4 `ruff_python_trivia`

Docs:

- https://docs.rs/ruff_python_trivia/0.0.7/ruff_python_trivia/

Pinned source:

- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_trivia/Cargo.toml
- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_trivia/src/lib.rs
- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_trivia/src/comment_ranges.rs

Inspect sibling modules for exact tokenizer/whitespace/pragma helper signatures before coding against them.

---

## 25.5 `ruff_python_index`

Docs:

- https://docs.rs/ruff_python_index/0.0.7/ruff_python_index/

Pinned source:

- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_index/Cargo.toml
- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_index/src/lib.rs
- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_index/src/indexer.rs

Agent reminder:

```text
Read indexer.rs before describing this crate.
Its purpose is lexical omitted-fact lookup, not symbol/project indexing.
```

---

## 25.6 `ruff_python_codegen`

Docs:

- https://docs.rs/ruff_python_codegen/0.0.7/ruff_python_codegen/

Pinned source:

- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_codegen/Cargo.toml
- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_codegen/src/lib.rs
- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_codegen/src/generator.rs
- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_codegen/src/stylist.rs

---

## 25.7 `ruff_python_formatter`

Docs:

- https://docs.rs/ruff_python_formatter/0.0.7/ruff_python_formatter/

Pinned source:

- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_formatter/Cargo.toml
- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_formatter/src/lib.rs
- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_python_formatter/src/options.rs

Official formatter design/user guide:

- https://github.com/astral-sh/ruff/blob/0.16.1/docs/formatter.md

Ruff describes the formatter as a high-performance, Black-compatible formatting engine whose primary goals include performance and unified integration with Ruff. That philosophy is consistent with treating it as canonical formatting rather than a lossless source tree.

---

## 25.8 `ruff_source_file`

Docs:

- https://docs.rs/ruff_source_file/0.0.7/ruff_source_file/

Pinned source:

- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_source_file/Cargo.toml
- https://github.com/astral-sh/ruff/blob/0.16.1/crates/ruff_source_file/src/lib.rs
- https://github.com/astral-sh/ruff/tree/0.16.1/crates/ruff_source_file/src

Core modules:

```text
line_index
line_ranges
newlines
```

---

## 25.9 Supporting crate: `ruff_text_size`

Although not one of the requested eight, `ruff_text_size` is a foundational API dependency because `TextSize`, `TextRange`, and `Ranged` are the coordinate vocabulary across parser, AST, source, trivia, index, semantic, codegen, and formatter layers.

Pin it with the same release train when directly imported:

```toml
ruff_text_size = "=0.0.7"
```

Docs:

- https://docs.rs/ruff_text_size/0.0.7/ruff_text_size/

---

## 25.10 Ruff versioning / product docs

- https://docs.astral.sh/ruff/versioning/
- https://docs.astral.sh/ruff/
- https://github.com/astral-sh/ruff/releases/tag/0.16.1

When package docs and `main` branch disagree with the pinned tag, the pinned tag wins for deployable code.

---

## 25.11 Source-audit commands for coding agents

When implementing or debugging against the installed workspace:

```bash
# Resolve the exact versions actually selected.
cargo tree -i ruff_python_parser
cargo tree -i ruff_python_ast
cargo tree -i ruff_python_semantic
cargo tree -i ruff_python_formatter
cargo tree -i ruff_text_size

# Look for duplicate Ruff component versions.
cargo tree -d | grep -E 'ruff_|ruff-python|ruff_python'

# Inspect source from the Cargo registry if needed.
cargo metadata --format-version 1 > /tmp/cargo-metadata.json

# Compile the isolated frontend first.
cargo check -p python-frontend

# Run focused tests.
cargo nextest run -p python-frontend
```

Repository-source workflow:

```bash
git clone https://github.com/astral-sh/ruff.git
cd ruff
git checkout 0.16.1

rg 'pub fn parse_module' crates/ruff_python_parser
rg 'pub trait Visitor' crates/ruff_python_ast
rg 'pub struct SemanticModel' crates/ruff_python_semantic
rg 'pub struct Indexer' crates/ruff_python_index
rg 'pub fn format_module_source' crates/ruff_python_formatter
```

---

## 25.12 Final architecture recommendation

For a Rust-native Python code-intelligence frontend replacing LibCST in an LLM-agent-oriented CPG system:

```text
CORE ANALYSIS
  ruff_source_file
  ruff_python_parser
  ruff_python_ast
  ruff_python_trivia
  ruff_python_index

LOCAL SEMANTICS (optional but valuable)
  version-pinned ruff_python_semantic adapter

PROJECT SEMANTICS / TYPES
  Pyrefly query daemon

SOURCE MODIFICATION
  original source + checked TextRange edit engine
  ruff_python_codegen for structurally complex replacement fragments

CANONICAL FORMATTING
  ruff_python_formatter, only when repository policy requests it

PERSISTENCE
  your normalized, versioned CPG fact schema
  never raw Ruff internal AST/semantic IDs as the platform contract
```

The most robust design is therefore not “replace LibCST with Ruff's AST.” It is:

```text
replace the monolithic CST/metadata/codemod abstraction
with explicit specialized layers whose outputs are normalized
into a stable code-intelligence model.
```

That separation is what allows the parser, type checker, source-preserving edit engine, formatter, and persistent graph to evolve independently without sacrificing correctness.
