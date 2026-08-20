# Rust MIR — advanced technical reference for CPG construction and continuous updating

Modeled after the supplied Tree-sitter-in-Rust advanced reference: this document begins with a version-pinned capability map and then expands it into self-contained implementation chapters. The emphasis is **using Rust MIR as a semantic/control-flow substrate for a Rust code property graph (CPG)** and operating that extraction continuously as a repository changes.

The target architecture is a **native Rust compiler-integrated extractor**. The preferred first surface is `rustc_public` (formerly Stable MIR), with narrowly scoped `rustc_private` integration when the public surface does not expose a required compiler fact. Textual MIR dumps are treated as diagnostics, not the durable machine interface.

## Version / source anchors

This reference is pinned to the Rust nightly documentation snapshot available on **2026-08-18**, where the nightly `rustc_public` docs identify the API as `rustc_public 1.100.0-nightly` and describe it as a public interface for third-party tools requiring type information, MIR bodies, monomorphized instances, and ABI details. `rustc_public` is still compiler-shipped rather than a normal crates.io dependency; its stated goal is a semver-guaranteed publication, but current users must pin a nightly compiler and install compiler-development components. ([rustc_public crate][rustc-public]) ([Initial integration][rustc-public-initial])

Recommended toolchain pin:

```toml
# rust-toolchain.toml
[toolchain]
channel = "nightly-2026-08-18"
components = ["rustc-dev", "llvm-tools", "rust-src"]
profile = "minimal"
```

Current integration requires the unstable compiler-library gate:

```rust
#![feature(rustc_private)]
extern crate rustc_public;

// Current run!/run_with_tcx! initialization also uses compiler crates.
extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
```

`rustc_public::run!` invokes the callback after compiler analyses and before code generation. All `rustc_public` objects are tied to compiler thread-local state and **must not escape the callback or be shared across threads**. A continuous CPG extractor therefore converts compiler objects into its own owned, versioned graph facts inside the callback and only persists those owned facts. ([Initial integration][rustc-public-initial])

MIR itself is Rust's mid-level IR, constructed from HIR. It is CFG-based, has no nested expressions, and makes types explicit. It is used for borrow checking, dataflow checks, optimization, constant evaluation, and code generation. ([MIR guide][mir-guide])

---

## Architectural recommendation in one page

```text
editor / filesystem events
        │
        ├─────────────── fast syntactic lane ────────────────┐
        │                                                    │
        │      Tree-sitter / source index                    │
        │      provisional symbols / spans / syntax edges    │
        │                                                    │
        └─ debounce / build-unit invalidation                │
                 │                                           │
                 v                                           │
          Cargo metadata + target graph                      │
                 │                                           │
                 v                                           │
   RUSTC_WORKSPACE_WRAPPER / compiler driver                 │
                 │                                           │
                 v                                           │
           rustc analyses / MIR                              │
                 │                                           │
      ┌──────────┴───────────┐                               │
      │                      │                               │
 rustc_public           rustc_private                        │
 default surface        narrow escape hatch                  │
      │                      │                               │
      └──────────┬───────────┘                               │
                 v                                           │
     normalized owned extraction facts                       │
     (definitions/types/CFG/uses/calls/instances)            │
                 │                                           │
                 v                                           │
      per-item semantic fingerprints                         │
                 │                                           │
                 v                                           │
          subgraph delta computation                         │
                 │                                           │
                 └──────── reconcile syntax + semantic ──────┘
                                  │
                                  v
                       transactional CPG update
```

**Recommended production stance:** let rustc's own incremental compiler decide how much compiler work can be reused, but make the **CPG updater independently incremental at item/subgraph granularity**. Re-running a compiler invocation does not imply rewriting an entire crate graph: extract normalized facts, fingerprint each owner/function/instance, and replace only changed semantic subgraphs plus edges whose endpoints changed.

---

## Feature inventory: what the reference covers

This document covers MIR's role in the compiler pipeline; `rustc_public`; direct `rustc_private` access; Cargo/workspace integration; compiler-wrapper design; MIR bodies, locals, basic blocks, statements, terminators, operands, places/projections, rvalues, types and generic arguments; source spans and macro provenance; MIR visitors; generic versus monomorphized MIR; `Instance` resolution; direct, indirect, closure, trait-object, vtable, drop-glue and hidden use edges; CFG and dataflow overlays; borrow/move/ownership modeling; def-use and control dependence; async/coroutine lowering; constants/statics; unsafe and FFI boundaries; interprocedural summaries; a concrete CPG schema; stable identity across compiler sessions; normalized fingerprints; serialization and transactional graph mutation; Cargo dependency invalidation; continuous compiler orchestration; debounce and scheduling; hybrid Tree-sitter+MIR operation; external dependency strategies; performance; observability; testing; nightly upgrade gates; security; production deployment; and an end-to-end reference extractor architecture.

---

# Proposed comprehensive documentation map

## 0) Scope, versioning, and MIR mental model
Define MIR as the compiler semantic/control-flow layer of the CPG; pin the dated nightly; separate source syntax, generic MIR, monomorphic instances, and derived analyses; establish which compiler facts are authoritative and which IDs are only session-local.

## 1) Toolchain installation and compiler-library setup
Install and pin nightly `rustc`, `rustc-dev`, `llvm-tools`, and `rust-src`; explain sysroot compiler crates, `#![feature(rustc_private)]`, reproducible CI/toolchain manifests, and the compiler-version metadata that must accompany every extraction event.

## 2) First executable `rustc_public` MIR extractor
Build the smallest in-process compiler driver using `rustc_public::run!`; enumerate local crate items, obtain MIR bodies, convert compiler-owned values to owned records inside the callback, and prove the callback/TLS lifetime constraint with safe and unsafe patterns.

## 3) Where MIR sits in the rustc pipeline
Map source → AST/macro expansion → HIR/type checking → MIR → borrow checking/dataflow → MIR transforms → monomorphization/codegen; distinguish generic owner MIR from optimized/codegen-oriented forms so agents do not assume there is a single timeless MIR.

## 4) Choosing an extraction surface: `rustc_public` vs `rustc_private` vs dumps
Compare public structured MIR, direct `TyCtxt`/rustc-private queries, `--emit=mir`, and `-Z dump-mir`; define a narrow private-adapter policy and explain why pretty-printed MIR is unsuitable as a durable machine protocol.

## 5) Cargo metadata, packages, targets, and compilation units
Use Cargo metadata and the actual compiler invocation as semantic configuration authority; model package/target/features/cfg/target-triple/build-script/proc-macro context and define build-context identity so incompatible configurations never overwrite one another.

## 6) Compiler-wrapper integration with Cargo
Use `RUSTC_WORKSPACE_WRAPPER` or, deliberately, `RUSTC_WRAPPER`; preserve rustc argument semantics, forward probe invocations, avoid stdout corruption, extract only intended crates, propagate compiler status, and emit facts over an idempotent side channel.

## 7) Crate and item discovery
Enumerate the local crate, external crates, entry point, traits/impls, and all local MIR-bearing items; define owner-centric extraction records and item facts that remain useful independently of MIR statement numbering.

## 8) MIR `Body` anatomy
Deep-dive `Body`, local declaration ordering, `var_debug_info`, basic blocks, source span, return place, arguments, user locals, and temporaries; define the function-owner partition that becomes the basic atomic replacement unit in the graph.

## 9) Locals, arguments, temporaries, and debug-variable information
Recover user-oriented names while retaining compiler locals; distinguish return/argument/inner locals, captures and temporaries; map local type/source facts into stable owner-scoped graph nodes without treating local numbers as global identity.

## 10) Basic blocks and CFG fundamentals
Build exact normal control-flow topology from basic blocks and terminators; define entry/exit/successor semantics, cleanup blocks, edge labels, block ordering, and how block-local sequence relates to CPG statement nodes.

## 11) Statements and state transitions
Cover assignment, storage markers, discriminant changes, fake reads, place mentions, user-type ascription, intrinsics, coverage and no-op statements; explain which variants produce runtime facts versus analysis/debug facts.

## 12) Terminators, normal edges, unwind edges, and calls
Model `Goto`, `SwitchInt`, `Return`, `Drop`, `Call`, `Assert`, `InlineAsm`, abort/resume/unreachable and unwind behavior; preserve edge kind so cleanup/panic paths are not collapsed into ordinary CFG flow.

## 13) Operands, constants, moves, and copies
Treat `Move`, `Copy`, constants and runtime-check operands as semantically distinct; identify function definitions carried as operands even when not immediately called, which is essential for a complete executable-use/monomorphization graph.

## 14) Places and projections as the memory-location vocabulary
Normalize local + projection paths (`Deref`, `Field`, `Index`, constant index, subslice, downcast, opaque cast) into access paths; define exact versus summarized places and owner-local canonicalization rules for fields/arrays/deref paths.

## 15) Rvalues and value-production semantics
Extract `Use`, references, reborrows, address-of, casts, aggregates, binary/unary operations, discriminants, lengths, repeats and thread-locals; turn rvalue structure into read/borrow/address and value-dependency facts.

## 16) Types, generics, traits, and type normalization
Represent `TyKind`, ADTs, function definitions/pointers, references, raw pointers, tuples, arrays, closures/coroutines, generic arguments, traits and impl relationships; canonicalize type keys without serializing rustc debug strings as identity.

## 17) Source spans, files, macro expansion, and source anchoring
Preserve source ranges and file identities while accepting that macro-expanded MIR may not map one-to-one to source syntax; distinguish display spans from semantic fingerprints and define reconciliation with the source/Tree-sitter layer.

## 18) MIR visitor APIs and extraction passes
Use `MirVisitor`/structured matches for exhaustive traversal; explain visit recursion, locations, public coarse place context versus richer rustc-private contexts, and why separate normalization passes are easier to version/test than one monolithic visitor.

## 19) Generic MIR versus monomorphized MIR
Store generic owner CFG once, add concrete instance overlays selectively, and understand the graph-size tradeoff of eagerly duplicating MIR for every monomorphization; define when instance bodies are required for precise interprocedural analysis.

## 20) `Instance` resolution and concrete callable identity
Use `Instance::resolve`, `resolve_for_fn_ptr`, `resolve_closure`, and `resolve_drop_in_place`; model instance kind, generic args, ABI, mangled name and optional body so a declared function and executable specialization remain distinct graph entities.

## 21) Direct call-edge extraction
Extract syntactic/declarative target, resolved monomorphic target, arguments, destination, normal successor and unwind successor; classify resolution confidence and retain unresolved/partially resolved edges rather than silently dropping them.

## 22) Function pointers, closures, callable operands, and indirect calls
Track function items that become values, fn-pointer coercions, closure instances and callable operands; separate `REFERENCES_FN`, `MAY_CALL`, and concrete `CALLS_INSTANCE` so address-taken functions are not misclassified as executed calls.

## 23) Trait dispatch, vtables, unsizing, and dynamic call over-approximation
Model static trait resolution separately from `dyn Trait`; use impl inventories and unsizing/vtable facts to build sound candidate sets, annotate precision/confidence, and avoid pretending MIR alone provides a whole-program points-to solution.

## 24) Drop glue, shims, intrinsics, and compiler-generated uses
Treat `Drop` terminators, recursive drop glue, vtable methods, callable shims, intrinsics and compiler-generated helper instances as executable uses. Mirror rustc monomorphization-collector thinking rather than collecting only explicit `Call` terminators.

## 25) Mapping MIR CFG into a CPG
Specify node/edge types for blocks, statements, terminators, CFG normal/unwind branches, sequencing and ownership; preserve raw MIR variant metadata while exposing stable semantic edge families for queries.

## 26) Read/write/move/copy dataflow edges
Derive access events structurally from statements/rvalues/terminators; encode read, write, move, copy, discriminant, drop and storage effects with place/local/location provenance; make event ownership deterministic for incremental replacement.

## 27) Borrows, references, raw pointers, and ownership edges
Represent shared/mutable borrow, reborrow, raw address-taking and dereference; distinguish syntactic MIR borrow events from exact borrow-checker loan liveness, and define the optional rustc-private enrichment boundary for deeper borrowck facts.

## 28) Place abstraction, alias domains, and memory access paths
Define exact projections, field-sensitive summaries, wildcard array indices, dereference/alias boundaries and heap/object abstractions; document precision knobs so scalable queries can choose exact, owner-local or conservative alias domains.

## 29) Move paths, initialization, and ownership-state overlays
Reconstruct moved/uninitialized/reinitialized states from MIR events and CFG, or optionally import richer compiler-private facts; retain uncertainty explicitly and avoid asserting borrow-checker-equivalent precision unless the same facts are actually consumed.

## 30) Reaching definitions, def-use, and SSA-like overlays
Compute owner-local reaching definitions over CFG, create `DEF`/`USE` events and `REACHES` edges, handle projections/kills conservatively, and version derived analyses separately from raw compiler facts for cheap recomputation.

## 31) Dominators, post-dominators, and control dependence
Build dominator/post-dominator trees and control-dependence edges from the normalized CFG; treat unwind/cleanup policy explicitly and use these overlays to support slicing, taint, condition-to-effect and blast-radius queries.

## 32) Closures, captures, async functions, and coroutines
Relate source closures/async functions to compiler-generated definitions/instances, capture/debug information and lowered state-machine control flow; preserve source-owner relationships so generated MIR is useful without obscuring source intent.

## 33) Constants, statics, CTFE, and allocation references
Extract static/constant references and, where needed, constant-evaluated allocations/function pointers; distinguish value facts from executable-use facts and include constants that can introduce hidden function/static reachability.

## 34) Unsafe operations, FFI, inline assembly, and trust boundaries
Mark raw dereference/address, foreign calls, ABI transitions and inline assembly as semantic trust boundaries; retain opaque effects/conservative summaries where Rust MIR cannot expose behavior implemented outside the analyzable program.

## 35) Interprocedural summaries and scalable graph queries
Compute compact read/write/call/alloc/panic/drop/taint summaries, cache them per owner/instance, and propagate changes only through summary dependencies; use summaries as graph-query acceleration rather than duplicating full transitive reachability.

## 36) Recommended Rust CPG schema
Define stable source, semantic-definition, MIR-flow, type, access, call/instance, dependency and derived-analysis node/edge families; specify ownership, provenance, confidence, generation and schema-version fields.

## 37) Stable identifiers across compiler sessions
Reject raw `DefId`/`CrateNum` as persistent keys; prefer `DefPathHash`/`StableCrateId`/stable source IDs behind a private adapter or construct explicit application keys from Cargo build context, qualified definition identity and owner-local structural anchors.

## 38) Canonical serialization and semantic fingerprints
Normalize ordered/unordered fields, stable keys, types, spans and enum variants into deterministic bytes; maintain semantic and source-sensitive fingerprints so whitespace/range movement can update presentation without forcing semantic subgraph rewrites.

## 39) Extraction protocol, batching, and transaction boundaries
Convert compiler TLS-bound objects to owned records, emit begin/owner/end manifests, include toolchain/build/schema generations, write atomically, reject partial/out-of-order events and make repeated deliveries idempotent.

## 40) Cargo dependency graph and invalidation domains
Track package/target dependencies plus Cargo.toml/lock/config, build scripts, generated source, proc macros, features, cfg and target changes; distinguish scheduling invalidation from actual semantic graph mutation determined after compiler/fingerprint comparison.

## 41) Continuous-update architecture
Implement a two-speed syntax/semantic pipeline: immediate Tree-sitter edits plus debounced Cargo/rustc refresh; expose freshness state, retain last-good semantics during invalid code, and replace only owners whose normalized compiler output changed.

## 42) Mapping file edits to crates, targets, and semantic owners
Use source syntax to map byte edits to likely owners and Cargo targets for prioritization, but let compiler/build facts remain correctness authority for macros/config/non-local effects; define tiered invalidation for body, signature, trait/impl, macro and build-context changes.

## 43) Leveraging rustc incremental compilation and query reuse
Keep incremental target directories/config stable and let rustc red/green query tracking suppress compiler work; do not mirror internal dep-nodes as the primary external protocol; independently fingerprint graph facts so compiler reuse and graph write suppression are decoupled.

## 44) Subgraph diffing and atomic graph replacement
Compare previous/new owner manifests, compute unchanged/changed/added/removed sets, replace owner-local partitions and affected cross-owner edges transactionally, use generations/tombstones against stale events, and advance snapshot visibility atomically.

## 45) Debounce, scheduling, cancellation, and backpressure
Coalesce editor bursts by target/configuration, supersede stale queued work, bound compiler concurrency, prioritize interactive requests, propagate cancellation at orchestration boundaries, and prevent slow graph analysis from blocking ingestion.

## 46) Failure isolation, stale-state policy, and recovery
Differentiate compile failure, extractor failure, protocol corruption and storage failure; retain last-known-good semantic snapshots with explicit staleness metadata; never publish partial compiler output as a successful graph generation.

## 47) Hybrid Tree-sitter + MIR code intelligence
Use Tree-sitter for immediate syntax, lexical scopes and edit-local ownership guesses while rustc/MIR supplies types, resolved definitions, CFG and ownership semantics; reconcile by source anchors/owner keys and expose freshness/provenance per fact.

## 48) External crates, dependency summaries, and source availability
Choose metadata-only, summary, source-indexed or full MIR strategies for dependencies; deduplicate versions by Cargo package identity; avoid rebuilding the transitive dependency universe on local edits while preserving external call/type endpoints.

## 49) Performance engineering
Measure compile latency, extraction CPU/allocations, event size, fingerprint cost, graph diff/write time and derived-analysis queues; reduce data volume with owner partitions, summaries, binary/streaming protocols and selective instance expansion rather than unsafe semantic shortcuts.

## 50) Testing and golden validation
Build MIR micro-fixtures for every statement/terminator/projection/call/trait/drop/async pattern; assert normalized facts and graph invariants, use textual MIR only as human diagnostics, and run incremental edit sequences to prove add/change/delete/stale/failure behavior.

## 51) Nightly/API upgrades and compatibility gates
Treat every nightly bump as compiler-provider migration: compile adapters, regenerate fixtures, compare enum exhaustiveness/schema and representative graph diffs, verify call/use completeness, then explicitly advance the supported toolchain tuple.

## 52) Observability and debugging
Emit structured timings/counters and per-invocation IDs, record compiler/toolchain/build-context metadata, surface extraction/graph generations and stale states, and use rustc tracing or MIR dumps only as targeted diagnostic aids.

## 53) Security and resource governance
Assume build scripts/proc macros/compiler plugins execute code; sandbox untrusted repositories, constrain CPU/memory/output/temp storage, redact sensitive paths/env, isolate tenants, and cap graph/query amplification from pathological code.

## 54) Production deployment patterns
Compare local IDE daemon, CI/batch indexer, shared repository service and multi-configuration analysis farm; define compiler-process isolation, cache/target-dir ownership, graph tenancy and freshness SLAs for each.

## 55) Reference extractor / daemon architecture
Provide a concrete multi-crate Rust workspace layout for protocol, identity, MIR normalization, private adapter, compiler driver, Cargo orchestrator, graph diff/store, derived analyses and daemon; define interfaces that keep rustc types confined to compiler callbacks.

## 56) CPG query recipes enabled by MIR
Show blast-radius, ownership-transfer, mutation-path, panic/cleanup, dynamic-dispatch, source-to-runtime, def-use and unsafe-boundary queries and the exact raw/derived edges each requires.

## 57) Best practices, anti-patterns, and agent invariants
Consolidate non-negotiable rules: MIR is not AST; Cargo configuration matters; compiler handles never persist; normal/unwind and move/copy differ; explicit calls are incomplete; instance and definition differ; last-good snapshots survive transient compile failure.

## 58) Capability gaps and future-facing design
Identify where rustc_public still requires private adapters or external analyses—stable persistent IDs, detailed borrowck/alias/dep-graph facts, richer monomorphization enumeration—and isolate those gaps behind versioned capabilities so the CPG schema remains durable.

---

# Suggested expansion order

1. **Sections 0–7:** mental model, toolchain, first extractor, compiler pipeline, API-surface choice, Cargo/wrapper integration, crate discovery.
2. **Sections 8–18:** core MIR vocabulary and traversal: bodies, locals, CFG, statements, terminators, operands, places, rvalues, types, spans, visitors.
3. **Sections 19–24:** monomorphization and interprocedural uses: instances, calls, indirect dispatch, vtables, drop glue, shims.
4. **Sections 25–35:** CPG semantics: CFG, dataflow, ownership, aliasing, move/init state, def-use, control dependence, async, constants, unsafe boundaries, summaries.
5. **Sections 36–46:** graph schema, identity, fingerprints, extraction protocol, Cargo invalidation, continuous updating, rustc incremental reuse, atomic deltas, scheduling, recovery.
6. **Sections 47–58:** hybrid syntax/semantic operation, dependency treatment, performance, tests, upgrades, observability, security, deployments, reference architecture, query recipes, best practices, future gaps.

---

# Rust MIR Advanced — 0) Scope, versioning, and MIR mental model

## 0.0 Identity: what MIR is

MIR is a **compiler IR, not a source AST**. Rustc constructs it from HIR after parsing, macro expansion, name resolution, and type checking have established much of the language meaning. The Rust compiler guide highlights three properties that are decisive for CPG construction:

```text
MIR is CFG-based.
MIR has no nested expressions.
MIR types are explicit.
```

Those properties make MIR unusually suitable for control/data-flow extraction: source expression nesting has already been lowered into assignments, temporaries, places, operands, statements, and terminators. ([MIR guide][mir-guide])

## 0.1 What MIR contributes to a CPG

| CPG concern | MIR value | What MIR does not replace |
|---|---|---|
| control flow | explicit basic blocks + terminator successors | high-level source syntax structure |
| calls | call terminators + typed callable operands | whole-program sound points-to analysis |
| data flow | assignments, operands, places, moves/copies | a ready-made reaching-definitions graph |
| ownership | move/copy distinction, references, drops | full borrowck facts in `rustc_public` |
| types | explicit local/place/operand types | stable cross-nightly serialization contract |
| monomorphization | concrete `Instance` APIs | automatic whole-program instance enumeration in all public use cases |
| source mapping | spans on items/statements/terminators | a pristine one-to-one source AST mapping after macro lowering |

**CPG invariant:** represent MIR as a *semantic/flow layer attached to source-level definitions*, not as a replacement for the source layer.

## 0.2 Recommended layered graph

```text
Layer A: source syntax
  File -> module/item/expression syntax nodes

Layer B: compiler semantic definitions
  Crate -> Def -> Type/Trait/Impl/Generic relationships

Layer C: MIR flow
  FunctionOwner -> BasicBlock -> Statement/Terminator
  Local/Place -> READ/WRITE/MOVE/COPY/BORROW
  BasicBlock -> CFG successor

Layer D: interprocedural instance graph
  FnDef -> Instance<GenericArgs>
  CALLS_DECLARED / CALLS_INSTANCE / MAY_CALL

Layer E: derived analysis
  reaching-def, control-dependence, summaries, taint, ownership state
```

Keeping these layers distinct allows syntax-only updates to exist temporarily while semantic compilation catches up; it also prevents compiler-generated MIR nodes from being mistaken for source-authored AST nodes.

## 0.3 Stability stance

`rustc_public` is the preferred extraction API because it is specifically intended to become the public, semver-aware compiler-analysis interface. It is not yet a normal stable dependency, so pin the **entire nightly toolchain**, not just a crate version. `rustc_private` is more complete but exposes rustc implementation details that can change on any nightly.

Agent rule:

```text
Default: rustc_public.
Escape hatch: run_with_tcx! + rustc_private for facts unavailable publicly.
Diagnostics only: --emit=mir / -Z dump-mir.
Never parse pretty-printed MIR as the primary CPG protocol.
```

---

# Rust MIR Advanced — 1) Toolchain installation and compiler-library setup

## 1.0 Required compiler components

The `rustc_public` integration guide requires a nightly toolchain and at least `rustc-dev` plus `llvm-tools`; it recommends pinning a dated nightly and commonly includes `rust-src`. ([Initial integration][rustc-public-initial])

```bash
rustup toolchain install nightly-2026-08-18 \
  --component rustc-dev \
  --component llvm-tools \
  --component rust-src

rustup override set nightly-2026-08-18
rustc --version --verbose
rustc --print sysroot
```

Recommended repository pin:

```toml
# rust-toolchain.toml
[toolchain]
channel = "nightly-2026-08-18"
components = ["rustc-dev", "llvm-tools", "rust-src"]
profile = "minimal"
```

## 1.1 Extractor package

A compiler-integrated binary uses sysroot compiler crates rather than ordinary crates.io dependencies for `rustc_public`/`rustc_driver`:

```rust
#![feature(rustc_private)]

extern crate rustc_public;
extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
```

Ordinary serialization/protocol crates remain regular dependencies:

```toml
[package]
name = "mir-cpg-driver"
version = "0.1.0"
edition = "2024"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
blake3 = "1"
anyhow = "1"
```

## 1.2 Build and smoke-test commands

```bash
cargo +nightly-2026-08-18 check -p mir-cpg-driver
cargo +nightly-2026-08-18 build -p mir-cpg-driver
./target/debug/mir-cpg-driver --version  # if your wrapper defines this
```

For a production extractor, store the following in its protocol header:

```text
rustc_version
rustc_commit_hash
rustc_public_surface_version/date
host_triple
target_triple
extractor_schema_version
feature/config hash
```

The compiler API and MIR details are tied to the pinned toolchain. Treat a nightly bump as a schema/provider upgrade requiring fixtures and graph-diff review.

## 1.3 CI gate

```bash
rustup show active-toolchain
cargo +nightly-2026-08-18 check --workspace
cargo +nightly-2026-08-18 test -p mir-cpg-driver
```

Do not let developer machines silently float to `nightly`; use the repository pin.

---

# Rust MIR Advanced — 2) First executable `rustc_public` MIR extractor

## 2.0 Minimal extraction shape

Current `rustc_public` initialization is through `run!` or `run_with_tcx!`. The callback runs after compiler analysis and before codegen; `rustc_public` objects are valid only inside the callback. ([Initial integration][rustc-public-initial])

```rust
#![feature(rustc_private)]
extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_public;

use std::ops::ControlFlow;
use rustc_public::CrateDef;

fn analyze() -> ControlFlow<(), ()> {
    for item in rustc_public::all_local_items() {
        eprintln!("item={}", item.name());
        if let Some(body) = item.body() {
            eprintln!("  blocks={}", body.blocks.len());
            eprintln!("  span={}", body.span.diagnostic());
        }
    }
    ControlFlow::Continue(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = rustc_public::run!(&args, analyze);
    if let Err(err) = result {
        eprintln!("compiler/extractor failed: {err:?}");
        std::process::exit(1);
    }
}
```

The exact wrapper argument shape should be tested against the pinned compiler; the important architectural contract is that the embedded driver receives the same rustc arguments Cargo intended to execute.

## 2.1 Do not return compiler objects

Bad:

```rust
// Conceptually wrong: the returned objects are tied to compiler TLS.
let items = rustc_public::run!(args, || {
    ControlFlow::Continue(rustc_public::all_local_items())
});
```

Good:

```rust
#[derive(serde::Serialize)]
struct OwnedItemFact {
    fq_name: String,
    block_count: usize,
}

// Build Vec<OwnedItemFact> *inside* the callback.
```

The official guide explicitly warns that using `rustc_public` objects after the callback can panic or return incorrect values, and that they should not be shared across threads. ([Initial integration][rustc-public-initial])

## 2.2 First CPG extraction target

For the first production milestone, extract only:

```text
crate metadata
item FQN / kind / span / type
MIR body owner
local declarations
basic blocks
statement kinds
terminator kinds + successors
call operands
place accesses
source spans
```

Then add monomorphic instances, borrow/ownership overlays, and derived analyses behind schema-versioned feature flags.

---

# Rust MIR Advanced — 3) Where MIR sits in the rustc pipeline

## 3.0 Compiler pipeline

```text
source
  -> tokenization / parsing
  -> macro expansion / AST
  -> name resolution
  -> HIR lowering / type checking
  -> MIR construction
  -> borrow checking + MIR analyses
  -> MIR transformations / optimizations
  -> monomorphization collection
  -> codegen (LLVM IR or other backend)
```

MIR is constructed from HIR and is used for flow-sensitive safety checks, optimization, CTFE, and code generation. Rustc's compiler overview notes that MIR remains generic for much of the pipeline, which lets analyses operate before potentially huge monomorphization expansion. ([MIR guide][mir-guide]) ([Compiler overview][compiler-overview])

## 3.1 There is not one timeless MIR

Internally, rustc runs MIR passes and represents different phases. A CPG extractor must define which semantic surface it wants:

| Surface | Strength | Risk |
|---|---|---|
| body exposed by `CrateItem::body()` | source-owner-centric generic MIR | generic calls may still need instance resolution |
| `Instance::body()` | eagerly monomorphized body; constants evaluated | potentially much larger graph |
| internal pre/post-borrowck queries | maximum fidelity | rustc_private instability |
| optimized MIR | closer to codegen | source constructs may be transformed away |

`Instance::body()` is documented as eagerly monomorphized with constants evaluated. ([Instance][instance])

## 3.2 Recommended dual representation

Store **generic owner MIR** as the canonical per-definition flow graph and store **instance-specific overlays** only where concrete dispatch/type specialization is needed.

```text
FnDef foo<T>
  owns generic CFG once

Instance foo::<u32>
  MONOMORPHIZES -> foo<T>
  concrete type args
  concrete call targets/summaries

Instance foo::<String>
  MONOMORPHIZES -> foo<T>
  concrete type args
  concrete call targets/summaries
```

This avoids duplicating every statement for every monomorphic instance while retaining concrete interprocedural precision.

---

# Rust MIR Advanced — 4) Choosing an extraction surface: `rustc_public` vs `rustc_private` vs dumps

## 4.0 Surface comparison

| Surface | Machine API | Stability | Best use |
|---|---:|---:|---|
| `rustc_public` | yes | intended public surface; currently nightly/compiler-shipped | default CPG extraction |
| `rustc_private` / `TyCtxt` | yes | fully unstable | facts unavailable publicly; compiler-query integration |
| `--emit=mir` | textual artifact | format not a stable protocol | debugging, fixtures, human inspection |
| `-Z dump-mir` | textual/debug files | nightly/debug-only | pass-by-pass diagnostics |

`rustc_public` is explicitly designed for external verification engines, linters and code generators needing MIR/type/instance/ABI information. ([rustc_public crate][rustc-public])

## 4.1 Why not parse MIR text

Text dumps are lossy as an API boundary:

```text
formatting changes create false graph deltas
IDs/names are printer-oriented
structured types require reparsing strings
spans/provenance are harder to preserve
new enum variants can silently become unrecognized text
no compiler-owned visitor/type API
```

Use `emit_mir`/pretty-printing in golden tests to explain structured extraction differences, not as the CPG wire format.

## 4.2 `rustc_private` escape-hatch policy

Add a private adapter only when all are true:

```text
[ ] exact missing semantic fact is identified
[ ] rustc_public cannot represent it on pinned nightly
[ ] fact materially improves correctness/precision
[ ] adapter is isolated behind a tiny internal trait
[ ] fixture covers the behavior
[ ] nightly upgrade CI compiles and semantically diffs it
```

Example missing-fidelity cases may include stable compiler-internal identifiers such as `DefPathHash`, detailed borrowck/dataflow state, or full internal `PlaceContext` categories. Do not expose rustc-private types beyond the adapter boundary.

---

# Rust MIR Advanced — 5) Cargo metadata, packages, targets, and compilation units

## 5.0 Cargo is part of semantic correctness

A Rust file cannot be analyzed correctly in isolation when `cfg`, features, target, build scripts, proc macros, dependency versions, and crate type affect compilation. Use Cargo's machine-readable metadata as the build-graph authority. The Cargo reference documents `cargo metadata` as a stable, versioned JSON representation of packages and dependencies. ([Cargo external tools][cargo-metadata])

```bash
cargo metadata --format-version 1 --all-features > target/cpg/cargo-metadata.json
```

For a real daemon, do not blindly force `--all-features`; match the project's configured feature/target profile or explicitly model multiple configurations.

## 5.1 CPG build-context identity

A semantic graph snapshot should include:

```text
Cargo package ID
Cargo target name + kind
crate name
feature set
--cfg set
rustc target triple
profile/check mode
build-script outputs
relevant environment/config hash
rustc commit
```

Two compilations of the same source file under different feature/target combinations can legitimately have different MIR and must not overwrite each other unless the product intentionally chooses a single canonical configuration.

## 5.2 Unit of semantic invalidation

Recommended hierarchy:

```text
file edit
  -> possible source owners
  -> Cargo target(s) containing file
  -> crate compilation unit(s)
  -> changed MIR owners after compile
  -> changed CPG subgraphs after fingerprint diff
```

Do not equate “file changed” with “rewrite every function in workspace.”

---

# Rust MIR Advanced — 6) Compiler-wrapper integration with Cargo

## 6.0 `RUSTC_WORKSPACE_WRAPPER`

Cargo supports `RUSTC_WORKSPACE_WRAPPER`: for workspace members Cargo invokes the specified wrapper and passes the real rustc path as the first wrapper argument. `RUSTC_WRAPPER` intercepts broader compiler invocations; if both are set, Cargo nests them. ([Cargo environment variables][cargo-env])

```bash
RUSTC_WORKSPACE_WRAPPER=/absolute/path/to/mir-cpg-driver \
  cargo +nightly-2026-08-18 check --workspace --all-targets
```

Use `RUSTC_WORKSPACE_WRAPPER` when the CPG primarily needs first-party workspace code. Use `RUSTC_WRAPPER` only when you deliberately want to inspect dependency compilations too.

## 6.1 Wrapper contract

Cargo effectively launches:

```text
mir-cpg-driver <real-rustc> <rustc-args...>
```

The wrapper must:

```text
1. preserve every rustc argument that affects semantics;
2. identify non-analysis probes (`--print`, version/config probes) and forward safely;
3. decide which crate targets deserve extraction;
4. run compiler analysis in-process for extraction;
5. emit owned facts to a side channel;
6. preserve rustc exit/failure behavior expected by Cargo;
7. avoid stdout pollution where Cargo expects machine output.
```

## 6.2 Side-channel output

Prefer one file/socket message per compiler invocation:

```text
target/cpg/events/<session>/<package>/<target>/<crate-hash>.jsonl
```

or a local Unix socket / named pipe to the daemon. Include invocation ID and schema version so partial/duplicate deliveries are idempotent.

## 6.3 Check mode

`cargo check` is normally the right semantic refresh command because the extractor callback runs after analysis, before codegen. Avoid forcing full LLVM codegen merely to obtain MIR unless a specific instance/codegen fact requires it.

---

# Rust MIR Advanced — 7) Crate and item discovery

## 7.0 Crate discovery APIs

The `rustc_public` crate exposes:

```text
local_crate()
external_crates()
find_crates(name)
entry_fn()
all_local_items()
```

`all_local_items()` covers local definitions with MIR bodies, including functions, closures, static initializers, and constants. ([rustc_public crate][rustc-public])

## 7.1 Item facts

For each `CrateItem`, collect at minimum:

```text
fully-qualified name
item kind
crate name / local-vs-external
span
has_body
requires_monomorphization
type
attributes needed by product policy
```

`CrateDef` provides name/trimmed name/crate/span; `CrateItem` provides body/kind/type/monomorphization-related accessors. ([CrateDef][crate-def]) ([CrateItem][crate-item])

## 7.2 Owner-centric extraction

Make the item owner the replacement boundary:

```text
OwnerRecord {
  owner_key,
  definition_facts,
  generic_mir_fingerprint,
  nodes[],
  edges[],
  outgoing_symbolic_calls[],
}
```

When a function changes, replace its owner partition and then repair cross-owner edges. This makes transactional updates tractable.

---

# Rust MIR Advanced — 8) MIR `Body` anatomy

## 8.0 `Body` is the per-function IR container

`rustc_public::mir::Body` is documented as the IR representation of a single function. It exposes `blocks`, debug-variable information and the body span, with methods for return/argument/inner locals and local declarations. The first local is the return place, then function arguments, then user locals and temporaries. ([Body][body])

```text
Body
  span
  locals:
    _0 = return place
    _1.._N = arguments
    remaining = user locals + temporaries
  var_debug_info[]
  blocks[]
```

## 8.1 CPG owner mapping

Recommended nodes:

```text
FUNCTION_OWNER
LOCAL
BASIC_BLOCK
MIR_STATEMENT
MIR_TERMINATOR
```

Recommended edges:

```text
FUNCTION_OWNER -[HAS_LOCAL]-> LOCAL
FUNCTION_OWNER -[ENTRY_BLOCK]-> BASIC_BLOCK(0)
FUNCTION_OWNER -[CONTAINS]-> BASIC_BLOCK
BASIC_BLOCK -[CONTAINS order=i]-> MIR_STATEMENT
BASIC_BLOCK -[TERMINATES_WITH]-> MIR_TERMINATOR
```

Do not make numeric local/basic-block indexes globally unique. Scope them under the stable owner key.

## 8.2 Fingerprint boundary

Normalize a body into your own deterministic representation before hashing. Exclude compiler-session-local handles; include semantic variants, types, spans according to chosen source sensitivity, and successor topology.

---

# Rust MIR Advanced — 9) Locals, arguments, temporaries, and debug-variable information

## 9.0 Local categories

Body local ordering provides a reliable coarse classification:

```text
_0             return place
_1..arg_count  arguments
remaining      user variables and compiler temporaries
```

Local declarations carry types and source information. `var_debug_info` can recover user-oriented variable naming/capture information that raw temporary numbering cannot. ([Body][body])

## 9.1 CPG representation

Store both compiler and user views:

```text
LocalNode {
  owner_key,
  local_index,
  role: RETURN | ARG | INNER,
  type_key,
  source_span?,
  debug_names[],
}
```

A single MIR local should not automatically be equated with a source variable: lowering can introduce temporaries, split patterns, and synthesize storage.

## 9.2 Continuous-update identity

Within an unchanged normalized body, `local_index` is convenient. Across body edits, local indexes may churn. Therefore:

```text
persistent graph ID = owner_key + normalized local identity
transient compiler coordinate = local_index
```

For user locals, source binding span/name can help match prior nodes. For pure temporaries, replacement of the entire owner partition is safer than trying to preserve identity across edits.

---

# Rust MIR Advanced — 10) Basic blocks and CFG fundamentals

## 10.0 CFG contract

MIR's core is a control-flow graph. A basic block contains a sequence of statements followed by exactly one terminator. Terminators determine successor blocks. ([MIR guide][mir-guide])

```text
bb0:
  stmt0
  stmt1
  ...
  terminator -> bb1 / bb2 / return / unwind ...
```

## 10.1 Edge classes

Use distinct CPG edge labels/properties:

```text
CFG_NORMAL
CFG_TRUE / CFG_FALSE        # where branch interpretation is available
CFG_SWITCH(value)
CFG_CALL_RETURN
CFG_UNWIND
CFG_DROP_RETURN
CFG_ASSERT_SUCCESS
CFG_ASSERT_UNWIND
```

Do not collapse unwind edges into ordinary control flow if your analysis cares about resource cleanup, panic behavior, or reachability.

## 10.2 Entry/exit modeling

Create synthetic graph nodes when useful:

```text
FUNCTION_ENTRY -> bb0
Return terminators -> FUNCTION_EXIT
Resume/Abort/Unreachable -> exceptional/terminal sink classes
```

This simplifies dominator, reachability, and interprocedural algorithms without mutating the underlying MIR facts.

---

# Rust MIR Advanced — 11) Statements and state transitions

## 11.0 Current public statement categories

The current `rustc_public::mir::StatementKind` surface includes assignments, fake reads, discriminant changes, storage live/dead markers, place mentions, user-type assertions, coverage/intrinsic markers, const-eval counters, and NOPs. ([StatementKind][statement-kind])

Important category for CPG construction:

```rust
StatementKind::Assign(place, rvalue)
```

This is the principal source of write and value-flow facts.

## 11.1 Assignment lowering

Translate:

```text
Assign(dst, rhs)
```

into:

```text
statement -[WRITES]-> canonical_place(dst)
rhs operands/places -[FLOWS_TO]-> statement
statement -[DEFINES]-> dst local/place version (derived layer)
```

Do not treat `StorageLive`/`StorageDead` as writes to the value. They delimit storage lifetime and are useful for lifetime/stack-style analyses.

## 11.2 Discriminants

`SetDiscriminant` and `Rvalue::Discriminant` are important for enum/variant flow. Preserve variant indices and map them to ADT variant definitions when type information permits.

---

# Rust MIR Advanced — 12) Terminators, normal edges, unwind edges, and calls

## 12.0 Terminator categories

Current public `TerminatorKind` includes `Goto`, `SwitchInt`, `Resume`, `Abort`, `Return`, `Unreachable`, `Drop`, `Call`, `Assert`, and `InlineAsm`. Terminators exposing possible unwinding carry `UnwindAction`; the public terminator APIs expose successors. ([TerminatorKind][terminator])

## 12.1 Call terminator

A call conceptually contains:

```text
callable operand
argument operands
destination place
normal target (if returning)
unwind behavior
```

CPG edges:

```text
CALLSITE -[CALLABLE_OPERAND]-> ...
CALLSITE -[ARGUMENT index=n]-> operand/place/value
CALLSITE -[WRITES_RETURN]-> destination
CALLSITE -[CFG_CALL_RETURN]-> target block
CALLSITE -[CFG_UNWIND]-> cleanup/propagation sink
```

## 12.2 `SwitchInt`

Do not model it as merely binary `if`: Rust `match`, enum discriminants, booleans and integer switches can produce multiple targets. Preserve switch values and otherwise target.

## 12.3 Cleanup semantics

Internal MIR documents explicit constraints on cleanup/unwind blocks; treating unwind as first-class avoids impossible control-flow paths in downstream analyses. ([Internal TerminatorKind][internal-terminator])

---

# Rust MIR Advanced — 13) Operands, constants, moves, and copies

## 13.0 `Operand`

The public MIR operand enum currently distinguishes:

```text
Copy(Place)
Move(Place)
Constant(ConstOperand)
RuntimeChecks(...)
```

and can report its type relative to body locals. ([Operand][operand])

## 13.1 Move vs copy is a semantic edge

Never normalize `Move` and `Copy` into a generic READ if ownership analyses matter.

```text
Copy(p) -> READS + COPIES_FROM
Move(p) -> READS + MOVES_FROM + potential invalidation of source ownership state
```

Whether a type is `Copy` is already reflected in lowering choices; preserve the operation as emitted.

## 13.2 Function items as operands

Rustc's own monomorphization collector emphasizes that **taking a reference to a function is a use even if no explicit call occurs**. It finds function/method items used in operand position, not only `Call` terminators. A CPG call/use extractor should mirror that principle. ([Mono collector][mono-collector])

Recommended edges:

```text
REFERENCES_FUNCTION
TAKES_FN_ADDRESS
PASSES_CALLABLE
CALLS
```

Keep them distinct so a graph query can ask “may become callable later” without pretending the function was invoked here.

---

# Rust MIR Advanced — 14) Places and projections as the memory-location vocabulary

## 14.0 `Place` is MIR's location vocabulary

A public `Place` is:

```text
base Local
+ Vec<ProjectionElem>
```

Projection elements include dereference, field, dynamic index, constant index, subslice, downcast, and opaque cast. `Place::ty` resolves the final projected type using the body's local declarations. ([Place][place]) ([ProjectionElem][projection])

Examples:

```text
_1                    local
(*_1)                 Deref
((*_1).2)             Deref -> Field(2)
_2[_3]                Index(_3)
((_4 as Variant).0)   Downcast -> Field(0)
```

## 14.1 Canonical access-path key

```text
PlaceKey {
  owner_key,
  base_local,
  projections: [Deref, Field(...), Index(...), ...]
}
```

Do not stringify MIR place printers as the durable key; serialize structured projection variants.

## 14.2 Field names

Public field projections use source-order field indexes and carry type information. If the CPG wants source field names, resolve the ADT/variant field definition through the type model rather than guessing from the index.

## 14.3 Alias caution

Two different `PlaceKey`s are not necessarily distinct memory locations after dereference, indexing, raw pointers, unions, or aliasing references. Access paths are a syntactic/typed location abstraction, not a complete alias analysis.

---

# Rust MIR Advanced — 15) Rvalues and value-production semantics

## 15.0 Public rvalue surface

Current `rustc_public::mir::Rvalue` covers address creation, aggregates, binary/checked operations, casts, dereference copies, discriminants, lengths, references, repeats, thread-local references, unary operations, ordinary use, and reborrows. ([Rvalue][rvalue])

Key CPG mappings:

| Rvalue | Suggested semantic edges |
|---|---|
| `Use(op)` | operand value flows to destination |
| `Ref(region, kind, place)` | BORROWS / BORROWS_MUT according to kind |
| `Reborrow(ty, mutability, place)` | REBORROWS |
| `AddressOf(kind, place)` | TAKES_RAW_ADDRESS |
| `BinaryOp` | operands -> COMPUTES -> destination |
| `Cast` | CASTS_TO type |
| `Aggregate` | element operands -> aggregate construction |
| `Discriminant(place)` | READS_DISCRIMINANT |
| `Len(place)` | READS_METADATA/LENGTH |
| `ThreadLocalRef(item)` | REFERENCES_STATIC/TLS |

## 15.1 Preserve lowering distinctions

An aggregate assignment is not equivalent to a set of independent field writes when destructors/initialization semantics matter. The Rvalue documentation explicitly notes why aggregate construction must remain distinguishable for dataflow. ([Rvalue][rvalue])

## 15.2 Typed expression layer

You can derive CPG expression nodes from rvalues, but keep the raw MIR rvalue variant as a property so future analysis can recover compiler semantics without reverse-engineering a normalized label.

---

# Rust MIR Advanced — 16) Types, generics, traits, and type normalization

## 16.0 Types are explicit in MIR

MIR's explicit type information is one of its strongest advantages over syntax-only parsing. Public type APIs expose `Ty`, `TyKind`, function definitions/pointers, ADTs, closures, dynamic trait objects, generic arguments, regions and constants. `TyKind::fn_def()` can deconstruct a function type into a definition plus generic arguments. ([TyKind][tykind])

## 16.1 Type-node normalization

Do not use `Debug` output as the only type identity. Normalize recursively:

```text
primitive kind
ADT definition key + generic args
reference(mutability, region-class, pointee)
raw pointer(mutability, pointee)
tuple(elements)
array(element, const length)
FnDef(def, args)
FnPtr(signature)
Dynamic(trait predicates, region)
Closure(def, args)
Coroutine(def, args)
```

## 16.2 Generic definition vs concrete type

Store two related identities:

```text
Definition: Vec<T>
Type instance: Vec<u8>
```

and connect with `INSTANTIATES_TYPE` / `TYPE_ARGUMENT` edges. This allows source-level and instance-level queries without duplicating definition metadata.

## 16.3 Regions

Most CPGs should not make raw compiler region IDs durable across sessions. Preserve region semantics only at the abstraction level required by the product unless you are deliberately exporting borrowck/NLL facts from rustc_private.

---

# Rust MIR Advanced — 17) Source spans, files, macro expansion, and source anchoring

## 17.0 Source anchoring

Items and MIR elements carry spans/source information. `rustc_public::Span` provides diagnostic/file/line helpers inside the compiler callback. Compiler-internal `SourceMap`/`SourceFile` give richer byte-position and stable-source-file information when a private adapter is justified. ([rustc_public Span source][public-span]) ([SourceFile][source-file])

## 17.1 Store normalized source coordinates

Recommended CPG span:

```text
SourceSpan {
  logical_file_key,
  byte_start?, byte_end?,
  line_start, col_start,
  line_end, col_end,
  macro_origin_class?,
}
```

For exact source joins with Tree-sitter, byte offsets are best. If `rustc_public` does not expose the required byte coordinates, use a narrow `SourceMap` adapter or correlate line/column plus source text.

## 17.2 Macro reality

MIR is produced after macro expansion. A span can refer to macro invocation/expanded provenance rather than a simple source-authored expression. Therefore the CPG should allow:

```text
MIR_NODE -[ORIGINATES_AT]-> SOURCE_SPAN
MIR_NODE -[EXPANDED_FROM]-> MACRO_INVOCATION   # when available
```

Do not assume every MIR statement maps 1:1 to one Tree-sitter source node.

## 17.3 Stable file identity

Rustc internally uses `StableSourceFileId`, a hash of filename and stable crate identity, to support stable hashing. For a rustc_private-enhanced extractor this is a strong file-key input; otherwise combine Cargo package/target identity with normalized repository-relative path. ([StableSourceFileId][stable-source-file])

---

# Rust MIR Advanced — 18) MIR visitor APIs and extraction passes

## 18.0 `MirVisitor`

`rustc_public` exposes `MirVisitor` and `MutMirVisitor`. The immutable visitor provides hooks for bodies, blocks, statements, terminators, places, projections, rvalues, operands, types, constants, regions, generic args, spans and debug-variable info. Default `visit_*` calls the corresponding `super_*`. ([MirVisitor][mir-visitor])

Conceptual extractor:

```rust
struct FactVisitor<'a> {
    owner: &'a str,
    facts: Vec<Fact>,
}

impl rustc_public::mir::MirVisitor for FactVisitor<'_> {
    fn visit_statement(&mut self, stmt: &rustc_public::mir::Statement,
                       loc: rustc_public::mir::visit::Location) {
        // emit structured statement facts
        self.super_statement(stmt, loc);
    }

    fn visit_terminator(&mut self, term: &rustc_public::mir::Terminator,
                        loc: rustc_public::mir::visit::Location) {
        // emit call/CFG facts
        self.super_terminator(term, loc);
    }
}
```

## 18.1 Visitor versus explicit walk

Use explicit per-block iteration when block/statement order and successor topology are the primary concern; use visitors for cross-cutting extraction of nested operands/places/types. Combining both is often clearest.

## 18.2 Public `PlaceContext` is intentionally coarse

Current `rustc_public` visitor `PlaceContext` exposes mutating/non-mutating information through a compact public abstraction, while internal rustc has richer categories such as borrow/drop/store/call. If your CPG requires exact access categories, derive many of them from statement/rvalue/terminator structure first; use rustc_private only for the remainder. ([Public visitor source][public-visitor-source]) ([Internal PlaceContext][internal-place-context])

---

# Rust MIR Advanced — 19) Generic MIR versus monomorphized MIR

## 19.0 Two interprocedural views

Generic MIR answers:

```text
What does this source definition do for arbitrary type parameters?
```

Monomorphized MIR answers:

```text
What does this concrete instance do after substituting actual generic arguments?
```

Rustc's own compiler overview emphasizes that MIR stays generic for efficient analysis before monomorphization. ([Compiler overview][compiler-overview])

## 19.1 CPG recommendation

Store generic MIR exactly once per source owner. Add concrete instance nodes:

```text
InstanceKey = DefinitionKey + CanonicalGenericArgs + InstanceKind
```

Edges:

```text
INSTANCE -[MONOMORPHIZES]-> DEFINITION
INSTANCE -[TYPE_ARGUMENT n]-> TYPE
CALLSITE -[CALLS_INSTANCE]-> INSTANCE
```

Materialize instance-specific full CFG only on demand if your analyses require it. Otherwise store summaries/resolved call edges to control graph explosion.

## 19.2 When concrete MIR matters

```text
trait-method resolution
generic dispatch precision
constant folding that affects control flow
intrinsic/shim specialization
concrete drop glue
ABI/layout-sensitive analysis
```

---

# Rust MIR Advanced — 20) `Instance` resolution and concrete callable identity

## 20.0 `Instance`

`rustc_public::mir::mono::Instance` exposes concrete generic arguments, specialized type, ABI, names/mangled names, body access, const evaluation, and resolution helpers such as `resolve`, `resolve_for_fn_ptr`, `resolve_closure`, and `resolve_drop_in_place`. `Instance::body()` returns eagerly monomorphized MIR with constants evaluated. ([Instance][instance])

## 20.1 Resolution pattern

```text
call operand type -> TyKind::FnDef(def, args)
                 -> Instance::resolve(def, args)
                 -> concrete Instance
                 -> CALLS_INSTANCE edge
```

Resolution can fail or remain indirect; encode uncertainty rather than inventing a target.

## 20.2 Instance key

Never persist raw `InstanceDef` handles. Convert inside callback to:

```text
InstanceRecord {
  definition_key,
  canonical_generic_args,
  instance_kind,
  specialized_type,
  human_name,
  optional_mangled_name,
}
```

The mangled name is useful for binary correlation but should not be the sole semantic identity.

---

# Rust MIR Advanced — 21) Direct call-edge extraction

## 21.0 Direct calls

For each `TerminatorKind::Call`:

```text
1. create CALLSITE node keyed by owner + MIR location;
2. extract callable operand and its type;
3. if function type is `FnDef`, capture Def + GenericArgs;
4. try `Instance::resolve` when concrete enough;
5. connect arguments and destination;
6. create normal-return and unwind CFG edges;
7. retain symbolic target even if instance resolution is unavailable.
```

## 21.1 Edge taxonomy

```text
CALLS_DECLARED   call -> source/generic FnDef
CALLS_INSTANCE   call -> concrete Instance
MAY_CALL         call -> candidate target
REFERENCES_FN    non-call operand -> FnDef/Instance
```

This prevents a common CPG error: conflating a source method definition with a codegen instance.

## 21.2 External functions

A direct call can target an external crate or foreign function. Create a stable external definition node even when no body is available; mark body availability separately.

## 21.3 Calls are not the full mono-use graph

Rustc's monomorphization collector treats function references, drop glue and vtable methods as uses in addition to explicit calls. If the product wants “what code can this item cause to be instantiated/referenced?”, model a broader `USES_MONO_ITEM` relation beside `CALLS`. ([Mono collector][mono-collector])

---

# Rust MIR Advanced — 22) Function pointers, closures, callable operands, and indirect calls

## 22.0 Indirect callable categories

A call operand may be:

```text
FnDef             usually directly resolvable
FnPtr             indirect function pointer
Closure           closure call / shim resolution
Coroutine closure async callable forms
Dynamic callable  trait-object/vtable dispatch
```

`FnDef` and `FnPtr` are distinct type categories: a function item has its own anonymous definition type and can coerce to a function pointer. ([TyKind][tykind])

## 22.1 Function-pointer analysis

At minimum:

```text
assignment of FnDef -> local       => local MAY_POINT_TO function
copy/move of fn pointer local      => propagate points-to set
phi-like join through CFG          => union candidate sets
call through local                 => MAY_CALL candidates
```

This lightweight intra-procedural points-to propagation gives much more precision than marking every same-signature function as a candidate.

## 22.2 Closures

Represent closure definition, capture environment and concrete callable instance separately:

```text
CLOSURE_DEF
CLOSURE_VALUE / capture construction
CAPTURES edges
CALLS_CLOSURE_INSTANCE
```

Use `Instance::resolve_closure` when the needed closure definition/args/kind are available. ([Instance][instance])

---

# Rust MIR Advanced — 23) Trait dispatch, vtables, unsizing, and dynamic call over-approximation

## 23.0 Dynamic dispatch requires vtable thinking

A trait-object unsizing conversion can create a vtable containing method pointers even when those methods are never explicitly called at the source site. Rustc's monomorphization collector explicitly creates mono items for dyn-compatible methods referenced by such vtables. ([Mono collector][mono-collector])

## 23.1 CPG relations

```text
TYPE -[IMPLEMENTS]-> TRAIT
UNSIZE_SITE -[CREATES_TRAIT_OBJECT]-> DYN_TYPE
DYN_TYPE -[MAY_DISPATCH_TO]-> METHOD_INSTANCE
CALLSITE -[MAY_CALL]-> METHOD_INSTANCE
```

## 23.2 Precision tiers

Tier 0:

```text
unknown dynamic target
```

Tier 1:

```text
all known impl methods compatible with trait method
```

Tier 2:

```text
flow-sensitive concrete receiver points-to / unsize-origin candidates
```

Tier 3:

```text
mirror rustc-private vtable/mono-item resolution for the compiled program
```

Make precision level explicit in edge properties; never present an over-approximation as a guaranteed dynamic target.

## 23.3 Default methods and shims

Trait calls can resolve to impl methods, default methods, or compiler-generated shims. Keep `InstanceKind` as part of the target identity.

---

# Rust MIR Advanced — 24) Drop glue, shims, intrinsics, and compiler-generated uses

## 24.0 Hidden executable dependencies

Important executable uses not reducible to ordinary source calls:

```text
drop glue
explicit Drop::drop
vtable methods
function addresses
closures and call shims
intrinsics / fallback bodies
thread-local references
compiler-generated shims
static/const allocation references
```

Rustc's collector documentation is an excellent completeness checklist: it discovers uses by inspecting MIR from graph roots, and specifically calls out function operands, drop glue and unsizing/vtable effects. ([Mono collector][mono-collector])

## 24.1 Drop edges

For `TerminatorKind::Drop(place, ...)`:

```text
DROP_SITE -[DROPS]-> PLACE/TYPE
DROP_SITE -[MAY_INVOKE_DROP_GLUE]-> drop-in-place Instance
DROP_GLUE -[MAY_CALL]-> explicit Drop::drop / nested drop glue
```

`Instance::resolve_drop_in_place(ty)` provides the public concrete drop-in-place resolution surface. ([Instance][instance])

## 24.2 Keep use graph separate from call graph

```text
CALL_GRAPH = runtime invocation relationships
MONO_USE_GRAPH = codegen reachability/reference relationships
```

They overlap but are not identical.

---

# Rust MIR Advanced — 25) Mapping MIR CFG into a CPG

## 25.0 Raw CFG facts

Materialize compiler facts first:

```text
BASIC_BLOCK
STATEMENT sequence
TERMINATOR
successor edges
unwind edges
```

Then derive normalized flow nodes if your CPG query language expects statement-level CFG.

## 25.1 Statement-level expansion

```text
bb0 -> stmt0 -> stmt1 -> term0
term0 -> bb1.entry
term0 -> bb2.entry
```

This is derivable from block structure and does not need to be duplicated in storage unless query performance justifies it.

## 25.2 Edge provenance

Every derived edge should retain:

```text
origin = MIR
owner_key
compiler snapshot/build_context
raw basic-block/statement coordinate
analysis version
```

This makes it possible to invalidate only MIR-derived analyses while keeping syntax/definition layers intact.

---

# Rust MIR Advanced — 26) Read/write/move/copy dataflow edges

## 26.0 Access classification

Derive access events from structured MIR:

```text
Operand::Copy(place)        READ + COPY
Operand::Move(place)        READ + MOVE
Assign(dst, rhs)            WRITE dst + rhs accesses
Ref(..., place)             BORROW
Reborrow(..., place)        REBORROW
AddressOf(..., place)       RAW_ADDRESS
Drop(place, ...)            DROP_USE
Call destination            WRITE_ON_RETURN
```

## 26.1 Event model

```text
AccessEvent {
  owner,
  mir_location,
  place_key,
  kind: READ | WRITE | MOVE | COPY | BORROW | ...,
  type_key,
  span,
}
```

This event stream is an ideal intermediate representation: use it to build CPG edges, reaching-def analysis, ownership-state analysis, and summaries without each consumer re-walking MIR.

## 26.2 Mutability is not enough

The current public visitor context tells whether a place access is mutating, but not every semantic subclass. Prefer exact structural extraction from operands/rvalues/terminators to avoid collapsing `Drop`, `Borrow`, `CallDestination`, and ordinary stores.

---

# Rust MIR Advanced — 27) Borrows, references, raw pointers, and ownership edges

## 27.0 Reference creation

Public rvalues distinguish `Ref`, `Reborrow`, and raw `AddressOf`. Preserve:

```text
borrow kind
mutability
source place
destination place/local
region representation at chosen abstraction
```

Suggested edges:

```text
BORROWS_SHARED
BORROWS_MUT
REBORROWS_SHARED
REBORROWS_MUT
TAKES_RAW_CONST_ADDRESS
TAKES_RAW_MUT_ADDRESS
```

## 27.1 Lifetime/borrowck depth

MIR powers the compiler's borrow checker, which runs dataflow to compute moves and regions over CFG points. The rustc-dev-guide describes initialization/move tracking, region inference, and “borrows in scope” as distinct analysis phases. ([Borrow checker][borrow-check])

`rustc_public` MIR alone does not hand you the full borrowck result graph. If exact loan/region facts are required, use a version-pinned rustc_private adapter or run your own conservative analysis over extracted access events.

## 27.2 CPG policy

Separate:

```text
compiler-observed reference construction facts
from
derived alias/loan/lifetime inferences
```

Mark the latter with an analysis algorithm/version.

---

# Rust MIR Advanced — 28) Place abstraction, alias domains, and memory access paths

## 28.0 Access paths are not aliases

Canonical places are useful names for memory access paths, but dereference/indexing can make multiple syntactic places overlap.

Recommended abstraction levels:

```text
Level 0: base local only
Level 1: field-sensitive, index-insensitive
Level 2: field + constant-index sensitive
Level 3: full MIR projection sequence
Level 4: points-to/alias sets across dereferences
```

## 28.1 Default CPG choice

Persist the exact projection path and precompute a coarser “alias bucket” for fast queries:

```text
exact = _3.Deref.Field(1).Index(_7)
coarse = owner::_3.*.field1[*]
```

## 28.2 Union/downcast caution

Enum downcasts and union-like storage semantics mean field disjointness must be based on Rust layout/type rules, not simple field-name inequality.

## 28.3 Incremental benefit

Place keys are owner-local. Replacing one owner partition automatically invalidates all derived alias facts for its locals without touching unrelated functions.

---

# Rust MIR Advanced — 29) Move paths, initialization, and ownership-state overlays

## 29.0 Move/initialization analysis

Rust borrow checking explicitly performs dataflow to determine what data is moved and when; this is central to initialization safety. ([Borrow checker][borrow-check])

A CPG can model a simplified ownership-state lattice:

```text
UNINITIALIZED
INITIALIZED
MOVED
MAYBE_INITIALIZED
MAYBE_MOVED
```

at CFG points for local/place abstractions.

## 29.1 Transfer examples

```text
Assign(p, ...)     -> p initialized
Move(p)            -> p/subpath moved
StorageDead(local) -> storage unavailable
Drop(p)            -> consumed/dropped according to path semantics
SetDiscriminant    -> variant state changes
```

## 29.2 Exactness boundary

Do not claim compiler-equivalent borrow safety from a simplified CPG lattice. Rustc's borrow checker also tracks move paths, regions, type constraints and borrows in scope. Use compiler-private borrowck facts if exact equivalence is a requirement.

## 29.3 Why still useful

Even a conservative overlay supports high-value questions:

```text
where values are consumed
which branch owns a moved resource
whether an API argument receives ownership vs copy/borrow
where destruction occurs
```

---

# Rust MIR Advanced — 30) Reaching definitions, def-use, and SSA-like overlays

## 30.0 MIR is not SSA, but it is SSA-friendly

MIR locals can be assigned multiple times. Build a derived def-use layer with standard CFG dataflow rather than pretending local IDs are single-assignment values.

## 30.1 Definitions and uses

Definitions:

```text
Assign destination
Call destination on successful return
SetDiscriminant mutation
possibly storage/init events depending on analysis
```

Uses:

```text
Copy/Move operands
places read by rvalues
callable + arguments
assert conditions
switch discriminants
drop places
```

## 30.2 Reaching definitions

For each place abstraction:

```text
IN[bb]  = union OUT[pred]
OUT[bb] = GEN[bb] union (IN[bb] - KILL[bb])
```

Field sensitivity determines KILL semantics. Use a versioned analysis key so changing alias precision does not mutate base MIR facts.

## 30.3 CPG edges

```text
DEF -[REACHES]-> USE
USE -[DATA_DEP]-> DEF
PARAM -[REACHES]-> USE
CALL_RETURN_DEF -[REACHES]-> USE
```

These edges enable slicing and taint analysis without re-running compiler integration.

---

# Rust MIR Advanced — 31) Dominators, post-dominators, and control dependence

## 31.0 Control dependence is derived

MIR provides CFG, not a precomputed program-dependence graph. Compute:

```text
dominators
post-dominators
control-dependence frontier
loop/SCC structure
```

## 31.1 Typical use

A statement `S` is control-dependent on branch `B` when execution of `S` depends on the outcome of `B`. Materialize:

```text
BRANCH -[CONTROLS condition/edge]-> STATEMENT/BLOCK
```

This plus reaching-def edges yields a program-dependence overlay suitable for slicing.

## 31.2 Exceptional flow policy

Decide whether unwind edges participate in post-dominators/control dependence. Provide separate normal-only and full-CFG analyses if both are valuable.

## 31.3 Incremental scope

Dominators/control dependence are owner-local. Recompute only when the owner's normalized CFG fingerprint changes.

---

# Rust MIR Advanced — 32) Closures, captures, async functions, and coroutines

## 32.0 Lowered callable forms

Rust closures, async functions and generators/coroutines lower into compiler-generated definitions/types and state machines. `rustc_public` exposes closure/coroutine type categories and local items can include closures with MIR bodies.

## 32.1 Closure graph

```text
SOURCE_CLOSURE
  -> CLOSURE_DEF
  -> capture environment construction
  -> CAPTURES place/type edges
  -> closure Instance(s)
```

Var debug information can help associate captured/user variables; type APIs identify closure definitions and args.

## 32.2 Async/coroutine graph

Represent source async function separately from generated coroutine body/state:

```text
ASYNC_FN -[LOWERS_TO]-> COROUTINE_DEF
COROUTINE_DEF -[HAS_MIR]-> state-machine CFG
AWAIT source syntax -[CORRESPONDS_TO]-> relevant suspension/control region (best-effort)
```

Do not expect a source `await` node to survive as one MIR operation after lowering.

## 32.3 Call graph

Async call construction and later poll/resume execution are semantically distinct. A precise runtime call graph may need edges to generated Future/poll/coroutine instances, not only the source async function.

---

# Rust MIR Advanced — 33) Constants, statics, CTFE, and allocation references

## 33.0 Constants and statics

`all_local_items()` includes constant/static initializers with bodies where applicable. MIR can contain constant operands and thread-local references; instance APIs support const evaluation. ([rustc_public crate][rustc-public])

## 33.1 Graph nodes

```text
CONST_DEF
STATIC_DEF
THREAD_LOCAL_STATIC
CONST_VALUE (optional normalized literal/allocation summary)
```

Edges:

```text
READS_CONST
REFERENCES_STATIC
INITIALIZES_STATIC
THREAD_LOCAL_REF
```

## 33.2 CTFE and allocations

Do not serialize rustc internal allocation handles. Export a product-specific summary: scalar value, bytes hash, referenced statics/functions, or “opaque const” marker.

## 33.3 Update rule

A const's value can change because its own source changes or because a referenced const changes. Let Cargo/rustc invalidate compilation; then semantic-fingerprint the resulting constant fact and propagate graph changes only if the normalized value/reference set changed.

---

# Rust MIR Advanced — 34) Unsafe operations, FFI, inline assembly, and trust boundaries

## 34.0 Unsafe does not disappear in MIR

Important boundaries to represent:

```text
raw pointer construction/deref
unsafe function calls
foreign functions / extern blocks
inline assembly
unions
intrinsics
transmute-like operations
```

## 34.1 Inline assembly

`TerminatorKind::InlineAsm` is an explicit terminator. Treat it as an opaque side-effect boundary unless you have architecture-specific analysis:

```text
INLINE_ASM -[MAY_READ]-> input places
INLINE_ASM -[MAY_WRITE]-> output places
INLINE_ASM -[CFG]-> destination(s)
INLINE_ASM -[CFG_UNWIND]-> unwind path where applicable
```

## 34.2 FFI

Foreign definitions may have no MIR body. Create external callable nodes with ABI/type facts and mark side effects conservatively.

## 34.3 Trust-boundary summaries

For taint/security queries, precompute summary flags:

```text
contains_unsafe_operation
calls_ffi
contains_inline_asm
uses_raw_pointer
```

Store the exact supporting locations so summary queries can drill down.

---

# Rust MIR Advanced — 35) Interprocedural summaries and scalable graph queries

## 35.0 Why summaries matter

A whole-workspace CPG can have millions of MIR facts. Many queries do not need to traverse every statement repeatedly. Compute per-owner or per-instance summaries:

```text
may_read globals/types
may_write globals/types
may_allocate / may_panic
calls direct targets
may-call dynamic targets
moves arguments
borrows arguments shared/mutably
returns argument-derived value
unsafe/FFI flags
```

## 35.1 Summary dependency graph

```text
summary(A) depends on local facts(A)
summary(A) depends on summaries(callees) for interprocedural properties
```

Use SCC iteration for recursive call components.

## 35.2 Incremental propagation

When owner A changes:

```text
recompute local summary(A)
if unchanged -> stop propagation
if changed   -> enqueue callers depending on A's summary
```

This is the same projection/firewall philosophy that makes compiler query systems incremental: downstream work only propagates when the observed result actually changes.

---

# Rust MIR Advanced — 36) Recommended Rust CPG schema

## 36.0 Recommended node labels

```text
WORKSPACE
PACKAGE
TARGET
CRATE
SOURCE_FILE
MODULE
DEFINITION
FUNCTION / METHOD / CLOSURE / CONST / STATIC
TRAIT / IMPL / ADT / FIELD / VARIANT
TYPE / TYPE_INSTANCE / GENERIC_PARAM
MIR_BODY
LOCAL
PLACE   (optional materialized node; can also be edge payload)
BASIC_BLOCK
MIR_STATEMENT
MIR_TERMINATOR
CALLSITE
MONO_INSTANCE
CONST_VALUE
ANALYSIS_SUMMARY
```

## 36.1 Core edges

```text
CONTAINS
DECLARES
TYPE_OF
IMPLEMENTS
MONOMORPHIZES
TYPE_ARGUMENT
SOURCE_SPAN
HAS_LOCAL
ENTRY_BLOCK
CFG_*
READS / WRITES / MOVES / COPIES
BORROWS_* / REBORROWS_* / TAKES_RAW_ADDRESS
CALLS_DECLARED / CALLS_INSTANCE / MAY_CALL
REFERENCES_FN / USES_MONO_ITEM
DROPS / MAY_INVOKE_DROP_GLUE
REACHES / DATA_DEP / CONTROL_DEP
CAPTURES
```

## 36.2 Provenance properties

Every fact should include enough provenance to answer:

```text
which compiler snapshot produced this?
which build configuration?
which source owner?
raw compiler coordinate?
derived or compiler-direct?
which extractor/analysis schema version?
```

## 36.3 Avoid graph-schema overfitting

Do not create a unique graph edge label for every rustc enum variant. Store raw variant tags/properties and expose stable semantic edge families; this reduces schema churn when MIR evolves.

---

# Rust MIR Advanced — 37) Stable identifiers across compiler sessions

## 37.0 Raw `DefId` is not persistent identity

Rustc's incremental-compilation documentation explicitly states that numeric IDs such as `DefId` can shift after source changes. Rustc bridges sessions using stable forms such as `DefPath`/`DefPathHash`. ([Incremental compilation][incremental])

## 37.1 Preferred identity hierarchy

With rustc_private adapter:

```text
crate = StableCrateId / Cargo package-target configuration
item  = DefPathHash
file  = StableSourceFileId
```

Without private identifiers, `rustc_public` exposes fully qualified names, crate info, parents/spans. Build an application key:

```text
package_id + target_key + crate_name + fully_qualified_definition_name + item_kind
```

and augment difficult anonymous/generated definitions with owner/source structural anchors.

## 37.2 Anonymous definitions

Closures, async-generated items and compiler-synthesized entities can have names/paths with unstable indices. Use:

```text
stable owner key
+ source span anchor
+ semantic kind
+ local structural ordinal/fingerprint
```

Then perform best-effort matching across snapshots. If ambiguous, replace the owner-local anonymous subgraph rather than forcing false identity continuity.

## 37.3 Instance identity

```text
InstanceKey = StableDefinitionKey + CanonicalGenericArgs + InstanceKind
```

Never use compiler-session numeric indexes as database primary keys.

---

# Rust MIR Advanced — 38) Canonical serialization and semantic fingerprints

## 38.0 Why fingerprint normalized facts

Rustc's own incremental engine compares stable fingerprints of query results so downstream work does not propagate merely because an input was re-evaluated. The compiler guide describes this red/green strategy and stable hashing across sessions. ([Incremental compilation][incremental])

Apply the same principle to the CPG.

## 38.1 Per-owner fingerprints

Suggested fingerprints:

```text
definition_fingerprint
body_structure_fingerprint
cfg_fingerprint
access_event_fingerprint
callsite_fingerprint
summary_fingerprint
instance_resolution_fingerprint
```

## 38.2 Canonicalization

Before hashing:

```text
sort unordered sets
normalize paths
serialize enum variants structurally
use stable item/type keys, not DefId/local addresses
normalize span policy
exclude volatile diagnostics/debug formatting
version the canonicalizer
```

## 38.3 Source-sensitive vs semantic-sensitive hashes

Keep both:

```text
semantic hash: ignores harmless span/whitespace movement
source hash: includes source anchors
```

Then a line insertion can update display spans without forcing expensive recomputation of semantically unchanged derived summaries.

---

# Rust MIR Advanced — 39) Extraction protocol, batching, and transaction boundaries

## 39.0 Never stream compiler handles to storage

Inside `run!` callback:

```text
rustc_public objects
  -> normalized owned records
  -> deterministic serialization
  -> bounded channel/file/socket
```

Outside callback:

```text
owned records only
```

## 39.1 Protocol envelope

```json
{
  "schema_version": 4,
  "rustc_commit": "...",
  "build_context": "...",
  "crate_key": "...",
  "invocation_id": "...",
  "status": "complete",
  "owners": []
}
```

## 39.2 Completion marker

Do not apply a partially written crate event as authoritative. Use:

```text
BEGIN invocation
OWNER batch...
END invocation + manifest/fingerprint
```

or write to a temporary file then atomic rename.

## 39.3 Idempotence

`invocation_id + crate_key + snapshot_generation` should let the daemon discard duplicate deliveries and reject out-of-order stale results.

---

# Rust MIR Advanced — 40) Cargo dependency graph and invalidation domains

## 40.0 Dependency domains

Cargo metadata yields package/target/dependency structure. Build reverse edges:

```text
changed package/target -> downstream workspace targets that may need semantic refresh
```

But a source edit usually first invalidates only the directly containing target; Cargo/rustc then determine which dependent units actually need rebuilding.

## 40.1 Inputs beyond `.rs`

Watch/invalidate on:

```text
Cargo.toml / Cargo.lock
.cargo/config.toml
build.rs and build-script inputs
generated source in OUT_DIR
proc-macro crates
feature/config changes
target-triple changes
environment variables declared through build-script rerun directives
```

A file watcher that only tracks `src/**/*.rs` is not semantically complete.

## 40.2 Dependency source strategy

For external crates, prefer stable definition/type summaries from crate metadata or compiler APIs. Avoid rebuilding/indexing every transitive dependency on every local keystroke.

---

# Rust MIR Advanced — 41) Continuous-update architecture

## 41.0 Recommended two-speed updater

```text
T0 edit arrives
  -> Tree-sitter updates syntax index immediately
  -> mark semantic snapshot stale for affected owner/file

T0 + debounce
  -> schedule cargo check for affected target/config
  -> rustc incremental compilation reuses green work
  -> wrapper extracts local crate MIR facts
  -> semantic diff computes changed owners
  -> graph transaction reconciles semantic layer
```

## 41.1 Snapshot states

Expose freshness explicitly:

```text
FRESH
SYNTAX_FRESH_SEMANTIC_STALE
COMPILING
SEMANTIC_FAILED_LAST_GOOD_RETAINED
```

This is better than deleting semantic edges during every incomplete edit.

## 41.2 Per-owner replacement

If compiler output says crate compiled successfully:

```text
for owner in extracted:
  if semantic_fingerprint unchanged:
      update source-span metadata only if needed
  else:
      replace owner's MIR partition
      repair outgoing/incoming cross-owner edges
```

Delete owners that existed in previous successful snapshot but are absent in the new complete crate manifest.

## 41.3 Cross-crate propagation

Only rerun downstream crate semantic extraction when Cargo/rustc schedules it or when your graph summary policy requires refreshed dependency-derived facts.

---

# Rust MIR Advanced — 42) Mapping file edits to crates, targets, and semantic owners

## 42.0 Edit-to-owner mapping

Fast path using source syntax index:

```text
byte edit range
 -> containing functions/items/modules
 -> affected Cargo target(s)
 -> semantic dirty owners
```

But macro/build/config edits can have nonlocal effects, so source containment is an optimization, not the sole correctness mechanism.

## 42.1 Invalidation tiers

```text
Tier A: function-body edit
  syntax owner dirty; target check

Tier B: signature/type/trait/impl edit
  owner + callers/implementors potentially dirty; rely on compiler dependencies

Tier C: macro definition/proc macro/build config
  broad crate/target invalidation

Tier D: Cargo features/target/dependency resolution
  build-context generation changes; potentially full semantic snapshot
```

## 42.2 Do not pre-guess too aggressively

The safest pattern is:

```text
use syntactic mapping to prioritize/schedule work;
use rustc/Cargo compile result as semantic authority;
use normalized output fingerprints to decide graph mutation.
```

---

# Rust MIR Advanced — 43) Leveraging rustc incremental compilation and query reuse

## 43.0 Rustc already has an incremental query engine

Rustc's queries are demand-driven and memoized. Incremental compilation persists a dependency graph and fingerprints; the red/green algorithm avoids recomputing downstream queries when a recomputed input yields the same result. ([Incremental compilation][incremental])

Do not recreate rustc's semantic invalidation engine outside the compiler.

## 43.1 What the CPG daemon should do

```text
keep Cargo target/incremental directories stable
invoke cargo check in a consistent target dir
avoid flags that disable incremental reuse
let rustc decide which queries/units are reusable
extract normalized result facts
perform independent CPG-level diffing
```

## 43.2 What not to assume

Rustc reusing a compiler query does not automatically notify your external graph store of “green owner X.” Your extractor protocol must still define what a complete compiler invocation contributes. A practical approach is to emit the local owners encountered in the successful invocation and use your own fingerprints to suppress graph writes.

## 43.3 Advanced private integration

A rustc_private plugin could theoretically inspect deeper dependency/query information, but coupling the graph updater to internal dep-nodes is high-maintenance. Prefer compiler incremental reuse as a black-box performance optimization unless exact query-level invalidation data is indispensable.

---

# Rust MIR Advanced — 44) Subgraph diffing and atomic graph replacement

## 44.0 Atomic replacement algorithm

Given previous owner manifest `P` and new `N`:

```text
unchanged = keys(P) ∩ keys(N) where fingerprint equal
changed   = keys(P) ∩ keys(N) where fingerprint differs
added     = keys(N) - keys(P)
removed   = keys(P) - keys(N)
```

Transaction:

```text
1. insert/update definition/span metadata for added/changed
2. replace owner-local MIR nodes for changed
3. insert added owner partitions
4. delete removed owner partitions
5. rebuild cross-owner edges touching changed/added/removed endpoints
6. update derived summaries affected by changed cross-owner facts
7. atomically advance crate snapshot generation
```

## 44.1 Tombstones

Use generation/tombstone semantics to prevent a late stale compiler event from resurrecting a deleted owner.

## 44.2 Edge ownership

Assign every edge a deterministic owner:

```text
local flow edge -> source owner
outgoing call edge -> caller owner
implements edge -> impl definition owner
incoming reverse index -> derived projection, not independently authored fact
```

This makes deletion deterministic.

---

# Rust MIR Advanced — 45) Debounce, scheduling, cancellation, and backpressure

## 45.0 Scheduling model

Separate queues:

```text
syntax updates: immediate, high priority, cheap
semantic target checks: debounced, cancel/supersede stale requests
background dependency refresh: lower priority
heavy derived analyses: incremental/post-commit
```

## 45.1 Debounce

Use a short debounce window for bursts of keystrokes, but do not delay explicit “save/build/test” events unnecessarily. Collapse multiple edits targeting the same Cargo unit into the newest requested source generation.

## 45.2 Cancellation

You cannot safely reuse arbitrary compiler objects after aborting their callback. Cancel at process/invocation boundaries; discard output unless a complete event/manifest is committed.

## 45.3 Backpressure

Bound:

```text
number of concurrent Cargo/rustc jobs
serialized fact queue bytes
pending graph transactions
derived-analysis tasks
```

Respect Cargo's jobserver rather than spawning uncontrolled nested compiler work.

## 45.4 Priority

Prioritize the target containing the active editor file, then reverse-dependency impact, then background complete-workspace convergence.

---

# Rust MIR Advanced — 46) Failure isolation, stale-state policy, and recovery

## 46.0 Last-known-good semantics

Incomplete Rust code is common during editing. A failed `cargo check` should not normally destroy the last successful MIR CPG.

Recommended state:

```text
syntax snapshot = newest source
semantic snapshot = last successful compiler generation
semantic freshness = stale/failed
compiler diagnostics = newest failed attempt
```

## 46.1 Recovery rules

```text
compiler crash -> quarantine invocation, retain prior semantic graph
extractor panic -> fail crate event, do not partially commit
protocol parse failure -> reject event by schema/version
owner extraction failure -> either fail entire crate snapshot or mark explicit partial mode; default fail closed
```

## 46.2 Partial compilation

Do not silently mix old and new owners under one “fresh” generation unless the protocol explicitly supports per-owner freshness and dependencies. Whole-crate successful-manifest semantics are simpler and safer.

---

# Rust MIR Advanced — 47) Hybrid Tree-sitter + MIR code intelligence

## 47.0 Why combine Tree-sitter and MIR

Tree-sitter excels at:

```text
sub-100ms incremental source structure
incomplete/broken code
direct byte-range mapping
comments/doc/source syntax
editor-local change ranges
```

MIR excels at:

```text
compiler-resolved types
lowered control flow
moves/copies/borrows/drops
callable definitions/instances
macro-expanded semantics
```

## 47.1 Reconciliation keys

Bridge using:

```text
source file key
owner fully-qualified name / stable definition key
source spans
function/item syntax kind
signature/type facts
```

## 47.2 Dual-edge provenance

A source call expression can have:

```text
AST/CST CALL node
  -[LOWERS_TO]-> MIR CALLSITE(s)
  -[RESOLVES_TO]-> definition/instance
```

One source expression may lower to multiple MIR operations; one MIR operation may originate from macro expansion without a direct source node. Make both cardinalities legal.

## 47.3 Continuous-update win

Use Tree-sitter to immediately identify likely dirty owners and maintain navigation during compile failure; use MIR to reconcile semantic truth after successful analysis.

---

# Rust MIR Advanced — 48) External crates, dependency summaries, and source availability

## 48.0 Three dependency modes

### Mode A — first-party only

Use `RUSTC_WORKSPACE_WRAPPER`. Create external dependency stubs from compiler type/definition references.

### Mode B — dependency summaries

Index external crate definitions/types/public call targets once per exact package version/feature/target tuple; reuse across workspaces.

### Mode C — full dependency MIR

Use broad wrapper integration and cache facts content-addressably. Expensive; reserve for whole-program security/analysis products.

## 48.1 Cache key

```text
Cargo package ID
source checksum/revision
feature/config set
target triple
rustc commit
extractor schema
```

## 48.2 Source availability

External crate MIR/type metadata can exist even when original source is not locally available in the same form. Separate semantic identity from source-file navigation availability.

---

# Rust MIR Advanced — 49) Performance engineering

## 49.0 Performance hierarchy

Optimize in this order:

```text
1. avoid compiler invocations that are superseded/irrelevant
2. preserve Cargo/rustc incremental cache reuse
3. scope to workspace/affected targets
4. normalize/fingerprint before graph writes
5. avoid full monomorphic CFG duplication
6. batch graph mutations
7. compute derived analyses only for changed owners/SCCs
8. optimize serialization allocations last
```

## 49.1 Measure

Per invocation record:

```text
Cargo scheduling latency
rustc wall/CPU time
extractor callback time
owners/bodies/blocks/statements scanned
instances resolved
serialized bytes
graph diff time
graph commit time
owners changed vs emitted
cache hit / no-op update ratio
```

## 49.2 Monomorphization explosion

Do not enumerate every possible generic instantiation. Prefer actually resolved/reachable instances under the chosen build configuration and store generic MIR once.

## 49.3 Serialization

JSONL is excellent for debugging; a binary schema (e.g. postcard/MessagePack/Protobuf/Arrow-like columnar batch) may become worthwhile for very large repositories. Keep a human-readable debug mode regardless.

---

# Rust MIR Advanced — 50) Testing and golden validation

## 50.0 Fixture matrix

Every extractor release should test:

```text
simple functions + branches + loops
match / enums / downcasts
moves vs copies
shared/mutable borrows
struct/tuple/array/index projections
generics with multiple concrete instances
trait static dispatch
trait-object dynamic dispatch
function pointers
closures + captures
async/await
Drop + nested drop glue
const/static/TLS
panic/unwind
unsafe/raw pointers
FFI
inline asm where target permits
macros/proc macros/build-script generated code
feature/cfg variants
```

## 50.1 Golden artifacts

For each fixture store:

```text
source
normalized CPG facts
semantic fingerprint manifest
optional pretty MIR dump for debugging
key graph query expected answers
```

Do not golden-test raw debug formatting alone.

## 50.2 Incremental tests

Apply edit sequences and assert:

```text
unrelated owner IDs remain stable
unchanged semantic fingerprints remain equal
changed owner partitions update
removed owners disappear
call edges repair correctly
failed compile retains last-good semantic snapshot
successful recovery advances generation
```

---

# Rust MIR Advanced — 51) Nightly/API upgrades and compatibility gates

## 51.0 Nightly is an API dependency

A dated nightly bump can change:

```text
rustc_public enum variants / methods
rustc_private module paths/query names
MIR lowering/optimization behavior
span mapping
trait/instance resolution
compiler wrapper argument expectations
```

## 51.1 Upgrade procedure

```text
1. change rust-toolchain.toml date in dedicated PR
2. compile extractor with exhaustive matches
3. run full fixture matrix
4. diff normalized CPG snapshots
5. inspect changed call/CFG/dataflow semantics
6. regenerate protocol schema only if required
7. benchmark representative repositories
8. deploy canary before broad rollout
```

## 51.2 Exhaustive matching policy

For internal MIR visitors rustc intentionally favors exhaustive pattern handling so new variants force code review. Apply the same policy in your adapter layer: new compiler variants should fail compilation or trigger explicit “unsupported” handling, not silently disappear. ([Internal visitor][internal-visitor])

## 51.3 Public/private separation

A rustc_public-only change should generally be cheaper to migrate than rustc_private internals. Track maintenance cost by adapter so private dependencies do not spread invisibly.

---

# Rust MIR Advanced — 52) Observability and debugging

## 52.0 Observability dimensions

Log per compiler invocation:

```text
invocation_id
Cargo package/target/crate
rustc commit
build config hash
source generation requested
callback start/end
owner count
body count
instance resolutions success/failure
normal/unwind CFG edge counts
call-edge classes
serialization bytes
warnings/unsupported variants
```

## 52.1 MIR debug tools

Useful diagnostics include pretty MIR emission/dumps, compiler tracing, and graph rendering from your own normalized facts. Rustc's tracing infrastructure can filter compiler modules such as borrow checking when debugging compiler behavior. ([rustc tracing][rustc-tracing])

## 52.2 Explainability

For every CPG edge derived from MIR, support an `explain` view:

```text
edge type
owner
MIR location
source span
raw enum/variant evidence
analysis rule/version
resolved target evidence
```

This is crucial for debugging false call edges in automated code-intelligence systems.

---

# Rust MIR Advanced — 53) Security and resource governance

## 53.0 Treat repositories as untrusted compiler input

Controls:

```text
sandbox compiler/extractor processes
CPU/memory/time limits
bounded output files/channels
path allowlists for daemon artifact writes
no execution of produced binaries
careful build-script/proc-macro trust policy
separate credentials from build environment
```

## 53.1 Build scripts and proc macros execute code

Running `cargo check` can execute build scripts and procedural macros. This is a substantially different trust boundary from Tree-sitter parsing raw source. For untrusted repositories, run semantic extraction in a hardened container/VM or disable/virtualize such execution where feasible, understanding that doing so can make compilation semantically incomplete.

## 53.2 Graph ingestion

Validate:

```text
schema version
size limits
string/path lengths
owner counts
edge endpoint ownership
invocation generation
compiler identity
```

Never allow compiler-originated names/paths to become arbitrary filesystem destinations.

---

# Rust MIR Advanced — 54) Production deployment patterns

## 54.0 Deployment patterns

### Local developer daemon

```text
watcher + syntax index + Cargo scheduler + MIR driver + embedded graph store
```

Best latency; reuse local target/incremental cache.

### CI indexer

```text
clean checkout -> cargo metadata -> full workspace check -> MIR facts -> graph snapshot
```

Best reproducibility; useful as authoritative baseline.

### Central multi-repo service

```text
isolated worker per repo/config
content-addressed dependency cache
artifact/event upload
central transactional graph
```

Requires strong sandboxing for build scripts/proc macros.

### Hybrid local/central

Local daemon produces deltas; central service periodically verifies with clean authoritative build.

## 54.1 Recommended consistency model

```text
syntax: eventually immediate / local latest
semantic MIR: last successful compiler snapshot
central graph: transactional per crate build generation
whole workspace: eventually convergent across crate generations
```

---

# Rust MIR Advanced — 55) Reference extractor / daemon architecture

## 55.0 Component architecture

```text
mir-cpg/
  crates/
    protocol/             # owned versioned extraction records
    identity/             # stable keys + canonicalization
    mir-normalize/        # rustc_public -> owned facts
    mir-private-adapter/  # tiny rustc_private escape hatch
    driver/               # rustc-compatible wrapper
    cargo-orchestrator/   # metadata + check scheduling
    graph-diff/           # fingerprints + owner deltas
    graph-store/          # transactional mutations
    analyses/             # reaching-def, control dep, summaries
    daemon/               # watcher, queues, API
```

## 55.1 Driver pseudocode

```rust
fn main() {
    let invocation = parse_wrapper_invocation(std::env::args());
    if invocation.is_probe() {
        forward_to_real_rustc(invocation);
        return;
    }

    let compiler_args = invocation.rustc_argv();
    let result = rustc_public::run!(&compiler_args, || {
        let event = extract_owned_crate_event();
        write_complete_event_atomically(event)?;
        std::ops::ControlFlow::Continue(())
    });

    propagate_compiler_status(result);
}
```

## 55.2 Normalizer API

```rust
trait MirFactSink {
    fn begin_owner(&mut self, owner: OwnerHeader);
    fn local(&mut self, fact: LocalFact);
    fn block(&mut self, fact: BlockFact);
    fn statement(&mut self, fact: StatementFact);
    fn terminator(&mut self, fact: TerminatorFact);
    fn access(&mut self, fact: AccessEvent);
    fn call(&mut self, fact: CallFact);
    fn end_owner(&mut self, fingerprint: Fingerprint);
}
```

Keep rustc types out of this trait so normalizer tests can run without embedding a compiler session.

## 55.3 Daemon loop

```text
watch -> dirty target -> cargo check
                      -> crate events
                      -> validate
                      -> diff owner manifests
                      -> graph transaction
                      -> enqueue derived analyses
                      -> publish fresh generation
```

---

# Rust MIR Advanced — 56) CPG query recipes enabled by MIR

## 56.0 Blast radius

Question:

```text
"If function X changes, what can be affected?"
```

Use:

```text
incoming CALLS_DECLARED/CALLS_INSTANCE/MAY_CALL
REFERENCES_FN
USES_MONO_ITEM
type/trait/impl dependencies
interprocedural summary dependencies
```

## 56.1 Ownership transfer

```text
"Where does value v stop being usable because ownership moves?"
```

Use `MOVES` access events + CFG + reaching definitions/ownership-state overlay.

## 56.2 Mutation pathways

```text
"What functions can mutate field F?"
```

Use canonical places, field projection, WRITES/BORROWS_MUT, call summaries, and alias abstraction.

## 56.3 Error/panic path

```text
"What cleanup/drop behavior is reachable if this call/assert unwinds?"
```

Traverse `CFG_UNWIND` plus `Drop`/drop-glue edges.

## 56.4 Dynamic dispatch candidates

```text
"Which concrete implementations can this dyn Trait call reach under this build?"
```

Use trait impl graph + unsize/vtable candidates + flow-sensitive receiver narrowing when available.

## 56.5 Source-to-runtime semantics

```text
"What compiler-level operations does this source expression lower into?"
```

Join Tree-sitter/source span to MIR statements/terminators and optionally monomorphic instances.

---

# Rust MIR Advanced — 57) Best practices, anti-patterns, and agent invariants

## 57.0 Agent invariants

1. **MIR is a flow/semantic layer, not the source AST.**
2. **`rustc_public` objects never cross the compiler callback boundary.** Convert to owned records inside the callback.
3. **Raw `DefId`/`CrateNum`/local indexes are session coordinates, not persistent graph IDs.**
4. **Use Cargo to establish the real compilation configuration.** Parsing a `.rs` file alone is not semantically sufficient.
5. **Treat moves and copies differently.**
6. **Treat normal and unwind control flow differently.**
7. **A `Call` terminator is not the full executable-use graph.** Include function operands, drop glue, vtables and shims where required.
8. **Generic definition and monomorphic instance are different graph entities.**
9. **Fingerprint normalized output before mutating the graph.**
10. **Retain last-known-good semantic state through temporary compile errors.**
11. **Let rustc's incremental engine optimize compiler work; independently incrementalize CPG writes.**
12. **Use rustc_private only behind a narrow, version-pinned adapter.**

## 57.1 Anti-pattern inventory

* Parsing `-Z dump-mir` text as the primary graph source.
* Persisting `DefId` numeric values across sessions.
* Returning `rustc_public` `Body`/`DefId`/`Ty` objects from `run!` callback.
* Running full `cargo build` with codegen on every keystroke merely to refresh MIR.
* Treating each changed file as a full-workspace graph replacement.
* Duplicating generic MIR CFG for every concrete type instance by default.
* Treating `FnDef` reference as an actual runtime call.
* Treating only explicit calls as monomorphization uses.
* Ignoring `Drop` and unwind edges.
* Treating projection paths as proof of alias disjointness.
* Assuming one source expression maps to one MIR statement.
* Deleting last-good semantic graph when incomplete editor code fails compilation.
* Indexing untrusted build scripts/proc macros outside a sandbox.
* Floating nightly without fixture/golden upgrade gates.

## 57.2 Pre-commit checklist

```text
[ ] toolchain date pinned
[ ] Cargo build context captured
[ ] compiler objects converted to owned facts inside callback
[ ] stable owner/type/instance keys used
[ ] generic and instance layers separated
[ ] normal/unwind CFG represented
[ ] move/copy/borrow/write classifications tested
[ ] direct + indirect + hidden executable uses addressed
[ ] per-owner semantic fingerprints computed
[ ] graph mutation transactional
[ ] last-known-good behavior tested
[ ] nightly upgrade fixture suite passes
```

---

# Rust MIR Advanced — 58) Capability gaps and future-facing design

## 58.0 Current public-surface gaps are a design input

`rustc_public` is explicitly still evolving toward a published semver interface. Design your CPG protocol so it is **more stable than the compiler adapter feeding it**. ([rustc_public crate][rustc-public])

Examples of facts that may still justify private adapters or derived analyses:

```text
stable compiler-internal IDs such as DefPathHash/StableSourceFileId
detailed borrowck/NLL loan and region facts
rich internal place-use categories
some monomorphization reachability details
macro expansion/source-map internals
compiler dep-graph/query-level invalidation metadata
```

## 58.1 Future-proofing rule

The adapter layer may change nightly; these should not:

```text
CPG semantic node/edge meanings
owned extraction protocol
stable application identity model
graph transaction semantics
analysis provenance/freshness model
```

## 58.2 Migration path

When `rustc_public` grows a capability currently implemented privately:

```text
1. add public provider behind same internal trait
2. run dual extraction on fixtures
3. compare normalized facts
4. switch provider
5. remove rustc_private dependency if no longer needed
```

The goal is monotonically shrinking the compiler-private surface over time.

---

# Appendix A — MIR-to-CPG mapping matrix

| MIR fact | Raw meaning | CPG node/edge | Incremental owner |
|---|---|---|---|
| `Body` | one function/initializer IR | `MIR_BODY` | definition owner |
| local 0 | return place | `LOCAL(role=RETURN)` | definition owner |
| argument local | formal parameter storage | `LOCAL(role=ARG)` | definition owner |
| inner local | user/temp storage | `LOCAL(role=INNER)` | definition owner |
| `BasicBlock` | CFG block | `BASIC_BLOCK` | definition owner |
| `Assign` | compute/store | `WRITES`, value-flow | definition owner |
| `Copy(place)` | non-consuming operand | `COPIES` + `READS` | definition owner |
| `Move(place)` | consuming operand | `MOVES` + `READS` | definition owner |
| `Ref` | reference creation | `BORROWS_*` | definition owner |
| `Reborrow` | borrow derived from reference/place | `REBORROWS_*` | definition owner |
| `AddressOf` | raw address | `TAKES_RAW_*_ADDRESS` | definition owner |
| `Call` | runtime invocation | `CALLSITE`, `CALLS_*` | caller owner |
| `Drop` | destruction point | `DROPS`, drop-glue use | owner/call summary |
| `SwitchInt` | branch | `CFG_SWITCH` | definition owner |
| `Assert` | branch + potential unwind | normal/unwind CFG | definition owner |
| `InlineAsm` | opaque target-level operation | unsafe/side-effect node | definition owner |
| `FnDef` operand | function item reference | `REFERENCES_FN` | referencing owner |
| `Instance` | concrete monomorphic callable | `MONO_INSTANCE` | definition + build config |
| unsizing to dyn | vtable creation pressure | vtable/may-dispatch edges | owner/instance |
| `SourceInfo`/span | source provenance | `SOURCE_SPAN` | source metadata partition |

# Appendix B — Suggested owned extraction schema

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CrateEvent {
    pub schema_version: u32,
    pub invocation_id: String,
    pub rustc_commit: String,
    pub build_context: BuildContext,
    pub crate_key: String,
    pub owners: Vec<OwnerFacts>,
    pub complete: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct OwnerFacts {
    pub owner_key: String,
    pub fq_name: String,
    pub kind: String,
    pub source: Option<SourceSpan>,
    pub type_key: String,
    pub body_fingerprint: String,
    pub locals: Vec<LocalFact>,
    pub blocks: Vec<BlockFact>,
    pub accesses: Vec<AccessFact>,
    pub calls: Vec<CallFact>,
    pub references: Vec<ReferenceFact>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MirLocationKey {
    pub block: u32,
    pub statement_index: u32,
    pub is_terminator: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PlaceKey {
    pub base_local: u32,
    pub projections: Vec<ProjectionFact>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ProjectionFact {
    Deref,
    Field { index: u32, field_type: String },
    Index { local: u32 },
    ConstantIndex { offset: u64, min_length: u64, from_end: bool },
    Subslice { from: u64, to: u64, from_end: bool },
    Downcast { variant_index: u32 },
    OpaqueCast { target_type: String },
}
```

**Schema rule:** the wire structs above deliberately contain only owned strings/integers/enums. No `rustc_public`/`rustc_middle` handle may appear in the protocol.

# Appendix C — Call-resolution decision tree

```text
Call terminator
  |
  +-- inspect callable Operand
       |
       +-- operand type is FnDef(def,args)
       |      |
       |      +-- add CALLS_DECLARED(def)
       |      +-- Instance::resolve(def,args)
       |             |
       |             +-- success -> CALLS_INSTANCE(instance)
       |             +-- failure -> symbolic/unresolved fact
       |
       +-- operand type is Closure
       |      +-- identify closure def/args/kind
       |      +-- resolve_closure when possible
       |
       +-- operand type is FnPtr
       |      +-- points-to propagation from assignments/casts
       |      +-- MAY_CALL candidate set
       |
       +-- dynamic trait/callable
              +-- trait method identity
              +-- known impl/vtable candidate set
              +-- receiver-flow narrowing
              +-- MAY_CALL with precision label
```

Additional non-call function uses:

```text
constant/function operand -> REFERENCES_FN
unsize to dyn            -> vtable method uses
Drop                     -> drop-in-place instance / Drop::drop
closure construction     -> closure body/shim uses
```

# Appendix D — Continuous-update state machine

```text
                edit
 FRESH ----------------------> SEMANTIC_STALE
   ^                                |
   |                                | debounce/schedule
   |                                v
   |                           COMPILING(gen N)
   |                              /       \
   |                       success         failure
   |                         /               \
   |                        v                 v
   |                 DIFFING/COMMIT       STALE_FAILED
   |                        |                 |
   +------------------------+                 |
                                              |
                                later successful compile
                                              |
                                              +------> FRESH
```

Rules:

```text
- Syntax generation may advance while semantic generation remains last-good.
- Only a complete successful compiler event can advance authoritative MIR generation.
- Out-of-order events are discarded by source/build generation.
- Graph update is owner-partition transactional.
- Derived analyses publish after base semantic commit, with their own generation/version.
```

# Appendix E — Cargo orchestration commands

Workspace discovery:

```bash
cargo +nightly-2026-08-18 metadata --format-version 1 > target/cpg/metadata.json
```

Full first-party semantic snapshot:

```bash
RUSTC_WORKSPACE_WRAPPER="$PWD/target/debug/mir-cpg-driver" \
  cargo +nightly-2026-08-18 check --workspace --all-targets
```

Single package:

```bash
RUSTC_WORKSPACE_WRAPPER="$PWD/target/debug/mir-cpg-driver" \
  cargo +nightly-2026-08-18 check -p my_package
```

Specific target examples:

```bash
cargo +nightly-2026-08-18 check -p my_package --lib
cargo +nightly-2026-08-18 check -p my_package --bin my_bin
cargo +nightly-2026-08-18 check -p my_package --tests
```

Compiler debug baseline:

```bash
# Human/debugging only; exact flags/output vary by nightly.
cargo +nightly-2026-08-18 rustc -p my_package --lib -- --emit=mir
```

**Do not** use the debug textual output as the persistent extractor protocol.

# Appendix F — Continuous invalidation cookbook

| Change | Immediate syntax action | Semantic action | Graph replacement scope after compile |
|---|---|---|---|
| whitespace/comment | Tree-sitter span update | often debounce; may skip if product permits | source spans only if semantics unchanged |
| function body | mark owner dirty | check containing target | changed owner(s) |
| function signature | mark owner + dependents suspect | check target; Cargo/rustc propagates | owner + call/type edges whose fingerprints changed |
| trait method/impl | mark trait/impl region | check affected target(s) | impl/dispatch summaries and changed callers |
| macro invocation | syntax owner dirty | compile; expansion may be nonlocal in owner | compiler-reported changed owners |
| macro definition | broad crate suspect | compile crate/downstream as Cargo decides | all changed owners by fingerprints |
| Cargo feature | new build-context generation | compile config | separate/new semantic snapshot |
| dependency version | dependency/build graph dirty | resolve + check | dependency summaries + affected owners |
| build.rs | build-input dirty | rerun Cargo | generated/config-affected owners |
| proc macro crate | macro consumers suspect | Cargo rebuild/check | compiler-reported changed owners |

# Appendix G — Completeness checklist for Rust call/use graphs

```text
[ ] Call terminators
[ ] FnDef operands not immediately called
[ ] Fn pointer assignments/propagation
[ ] closure/coroutine callable instances
[ ] trait static dispatch
[ ] trait-object dynamic dispatch candidates
[ ] unsizing/vtable-created method references
[ ] Drop terminators
[ ] nested drop glue
[ ] explicit Drop::drop
[ ] shims / InstanceKind
[ ] intrinsics / fallback bodies
[ ] thread-local/static/function references in constants/allocations as required
[ ] external/foreign calls
[ ] inline asm represented as opaque boundary
```

# Appendix H — Source authority matrix

| Question | Best authority |
|---|---|
| “What bytes/syntax changed right now?” | Tree-sitter/source buffer |
| “What crate/feature/target configuration applies?” | Cargo metadata + rustc invocation |
| “What function/type does this compile to?” | rustc semantic APIs |
| “What is the lowered CFG?” | MIR |
| “Is this operand a move or copy?” | MIR |
| “What exact loans are live at point P?” | rustc borrowck/private facts or equivalent derived analysis |
| “What concrete generic target is called?” | `Instance` resolution/monomorphic analysis |
| “What source text created this expanded MIR?” | compiler spans + syntax index; macro-aware best effort |
| “What should remain stable across recompiles?” | application stable keys / DefPathHash adapter, never raw DefId |

# Appendix I — Source anchors

[rustc-public]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_public/index.html "rustc_public — nightly rustc documentation"
[rustc-public-initial]: https://rust-lang.github.io/rustc_public/initial.html "Rustc Public Project — Initial Integration"
[mir-guide]: https://rustc-dev-guide.rust-lang.org/mir/index.html "Rust Compiler Development Guide — MIR"
[compiler-overview]: https://rustc-dev-guide.rust-lang.org/overview.html "Rust Compiler Development Guide — compiler overview"
[body]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_public/mir/struct.Body.html "rustc_public::mir::Body"
[statement-kind]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_public/mir/enum.StatementKind.html "rustc_public::mir::StatementKind"
[terminator]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_public/mir/enum.TerminatorKind.html "rustc_public::mir::TerminatorKind"
[internal-terminator]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_middle/mir/enum.TerminatorKind.html "rustc_middle::mir::TerminatorKind"
[operand]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_public/mir/enum.Operand.html "rustc_public::mir::Operand"
[place]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_public/mir/struct.Place.html "rustc_public::mir::Place"
[projection]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_public/mir/enum.ProjectionElem.html "rustc_public::mir::ProjectionElem"
[rvalue]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_public/mir/enum.Rvalue.html "rustc_public::mir::Rvalue"
[tykind]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_public/ty/enum.TyKind.html "rustc_public::ty::TyKind"
[instance]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_public/mir/mono/struct.Instance.html "rustc_public::mir::mono::Instance"
[mir-visitor]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_public/mir/visit/trait.MirVisitor.html "rustc_public::mir::visit::MirVisitor"
[public-visitor-source]: https://doc.rust-lang.org/nightly/nightly-rustc/src/rustc_public/mir/visit.rs.html "rustc_public MIR visitor source"
[internal-place-context]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_middle/mir/visit/enum.PlaceContext.html "rustc_middle::mir::visit::PlaceContext"
[internal-visitor]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_middle/mir/visit/index.html "rustc_middle MIR visitor"
[mono-collector]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_monomorphize/collector/index.html "rustc monomorphization collector"
[borrow-check]: https://rustc-dev-guide.rust-lang.org/borrow-check.html "Rust Compiler Development Guide — MIR borrow check"
[dataflow]: https://rustc-dev-guide.rust-lang.org/mir/dataflow.html "Rust Compiler Development Guide — MIR dataflow"
[incremental]: https://rustc-dev-guide.rust-lang.org/queries/incremental-compilation-in-detail.html "Rust Compiler Development Guide — incremental compilation in detail"
[cargo-env]: https://doc.rust-lang.org/cargo/reference/environment-variables.html "Cargo environment variables"
[cargo-metadata]: https://doc.rust-lang.org/cargo/reference/external-tools.html "Cargo external tools / cargo metadata"
[crate-def]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_public/crate_def/trait.CrateDef.html "rustc_public::crate_def::CrateDef"
[crate-item]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_public/struct.CrateItem.html "rustc_public::CrateItem"
[public-span]: https://doc.rust-lang.org/nightly/nightly-rustc/src/rustc_public/ty.rs.html "rustc_public Span implementation"
[source-file]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_span/struct.SourceFile.html "rustc_span::SourceFile"
[stable-source-file]: https://doc.rust-lang.org/nightly/nightly-rustc/rustc_span/struct.StableSourceFileId.html "rustc_span::StableSourceFileId"
[rustc-tracing]: https://rustc-dev-guide.rust-lang.org/tracing.html "Rust Compiler Development Guide — tracing"


# Appendix J — Concrete normalized MIR extraction model

This appendix turns the conceptual schema into a deliberately **rustc-independent owned model**. The exact Rust types are illustrative, but the separation is architectural: everything below is safe to retain, hash, serialize, diff, move across threads, and store after the compiler callback has ended.

## J.1 Envelope and version tuple

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtractionEnvelope {
    pub protocol_version: u32,
    pub normalizer_version: u32,
    pub analysis_version: u32,

    pub rustc_release: String,
    pub rustc_commit_hash: String,
    pub rustc_host: String,
    pub target_triple: String,

    pub build_context: BuildContextKey,
    pub invocation_id: String,
    pub snapshot_generation: u64,
    pub status: ExtractionStatus,

    pub crate_fact: CrateFact,
    pub owner_manifest: Vec<OwnerManifestEntry>,
    pub owners: Vec<OwnerFact>,
}
```

Recommended compatibility rule:

```text
protocol_version
  changes when the wire format is incompatible

normalizer_version
  changes when raw MIR -> normalized semantic facts changes

analysis_version
  changes when derived graph algorithms/summaries change

rustc/toolchain tuple
  identifies the compiler provider that produced the facts
```

Do **not** conflate these four version dimensions. A dominator algorithm improvement should not force a compiler-driver protocol bump; a nightly MIR enum change should not automatically invalidate every stored source node if normalization produces the same canonical facts.

## J.2 Build-context identity

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct BuildContextKey {
    pub package_id: String,
    pub target_name: String,
    pub target_kind: String,
    pub crate_name: String,
    pub features_hash: [u8; 32],
    pub cfg_hash: [u8; 32],
    pub target_triple: String,
    pub profile_semantics: String,
    pub build_script_hash: [u8; 32],
    pub toolchain_hash: [u8; 32],
}
```

The context key is intentionally broader than a Cargo package. The same package can generate different MIR under:

```text
feature A on / off
unix vs windows cfg
x86_64 vs wasm target
library vs test target
build.rs outputs
proc-macro output/configuration
compiler version
```

A CPG may choose to expose one configuration as “active,” but storage should make that a **selection policy**, not accidental key collision.

## J.3 Stable owner keys

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum OwnerKey {
    CompilerStable {
        stable_crate: u64,
        def_path_hash_hi: u64,
        def_path_hash_lo: u64,
    },
    PublicFallback {
        build_context_hash: [u8; 32],
        crate_name: String,
        qualified_name: String,
        item_kind: String,
        source_anchor: Option<SourceAnchor>,
    },
}
```

Use the compiler-stable form when a pinned `rustc_private` adapter exposes the stable identity required by the product. The fallback is not claimed to have rustc's exact stability guarantees; it is an **application identity scheme** and must be treated as such.

## J.4 Source anchors

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SourceAnchor {
    pub file_key: String,
    pub byte_start: u32,
    pub byte_end: u32,
    pub line_start: u32,
    pub column_start: u32,
    pub line_end: u32,
    pub column_end: u32,
    pub expansion_kind: ExpansionKind,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ExpansionKind {
    SourceAuthored,
    MacroExpansion,
    CompilerGenerated,
    Unknown,
}
```

Keep source anchors out of a *semantic-only* fingerprint if the product wants whitespace-only edits to avoid semantic replacement. Maintain a second source-layout fingerprint when exact navigation ranges matter.

## J.5 MIR location key

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct MirLocationKey {
    pub block: u32,
    pub slot: MirSlot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MirSlot {
    Statement(u32),
    Terminator,
}
```

`block` and statement indices are **owner-local coordinates**. Persistent graph identity is therefore:

```text
OwnerKey + MirLocationKey + normalized node role
```

not `bb3[7]` by itself.

## J.6 Owner fact

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OwnerFact {
    pub key: OwnerKey,
    pub qualified_name: String,
    pub kind: String,
    pub span: Option<SourceAnchor>,
    pub type_key: Option<TypeKey>,
    pub generic_params: Vec<GenericParamFact>,

    pub locals: Vec<LocalFact>,
    pub blocks: Vec<BlockFact>,
    pub accesses: Vec<AccessFact>,
    pub calls: Vec<CallFact>,
    pub references: Vec<ExecutableReferenceFact>,
    pub drops: Vec<DropFact>,

    pub semantic_fingerprint: [u8; 32],
    pub source_fingerprint: [u8; 32],
}
```

This record is a good **graph replacement boundary** because nearly all MIR-local nodes and edges can be deleted/reinserted under the owner without scanning the rest of the graph.

## J.7 Block and terminator facts

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BlockFact {
    pub index: u32,
    pub is_cleanup: bool,
    pub statements: Vec<StatementFact>,
    pub terminator: TerminatorFact,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TerminatorFact {
    pub location: MirLocationKey,
    pub kind: TerminatorTag,
    pub normal_successors: Vec<u32>,
    pub unwind_successors: Vec<u32>,
    pub span: Option<SourceAnchor>,
    pub detail: TerminatorDetail,
}
```

The storage schema should preserve both:

```text
raw-ish normalized variant tag
stable semantic edge family
```

Example:

```text
TerminatorTag::Assert
  -> CFG_TRUE / CFG_FALSE or normal/unwind semantics as normalized
  -> MAY_PANIC semantic property
```

Do not require every graph query to understand the exact rustc enum layout.

## J.8 Place normalization

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PlaceKey {
    pub local: u32,
    pub projections: Vec<ProjectionKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ProjectionKey {
    Deref,
    Field { index: u32, field_type: TypeKey },
    Index { index_local: u32 },
    ConstantIndex { offset: u64, from_end: bool, min_length: u64 },
    Subslice { from: u64, to: u64, from_end: bool },
    Downcast { variant: u32 },
    OpaqueCast { target: TypeKey },
}
```

Maintain two additional derived keys:

```text
exact_place_key
summary_place_key
```

For example:

```text
exact:   _3.field[2].*.x
summary: _3.field[*].*.x
```

This permits precise local queries and conservative interprocedural summaries without mutating the raw extracted representation.

## J.9 Access events

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AccessFact {
    pub location: MirLocationKey,
    pub place: PlaceKey,
    pub kind: AccessKind,
    pub type_key: TypeKey,
    pub span: Option<SourceAnchor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AccessKind {
    Read,
    Copy,
    Move,
    Write,
    BorrowShared,
    BorrowMut,
    ReborrowShared,
    ReborrowMut,
    AddressOfConst,
    AddressOfMut,
    Drop,
    DiscriminantRead,
    DiscriminantWrite,
    StorageLive,
    StorageDead,
    UnknownUse,
}
```

A public-MIR normalizer should derive these events from structured statement/rvalue/terminator shape. A rustc-private `PlaceContext` adapter can optionally refine/validate them, but **the stable schema should not depend on the private enum names**.

## J.10 Call facts

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CallFact {
    pub location: MirLocationKey,
    pub callable: OperandFact,
    pub args: Vec<OperandFact>,
    pub destination: Option<PlaceKey>,
    pub normal_target: Option<u32>,
    pub unwind_target: Option<u32>,

    pub declared_target: Option<DefinitionKey>,
    pub resolved_instance: Option<InstanceKey>,
    pub dispatch: DispatchKind,
    pub confidence: ResolutionConfidence,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum DispatchKind {
    Direct,
    StaticTrait,
    FunctionPointer,
    Closure,
    DynamicTrait,
    Intrinsic,
    Foreign,
    Shim,
    Unknown,
}
```

Do not force a single target into `resolved_instance` for genuinely indirect calls. Use candidate edges separately:

```text
CALLSITE -[CALLS_INSTANCE]-> concrete instance     // exact
CALLSITE -[MAY_CALL confidence=...]-> candidate    // conservative
CALLSITE -[CALLS_UNKNOWN]-> opaque boundary        // unresolved
```

---

# Appendix K — Exhaustive structured extraction rules

This appendix gives a **normalizer recipe** for MIR facts. The exact enum surface can change with the pinned nightly; matches against rustc APIs should be exhaustive so a compiler upgrade fails loudly rather than silently omitting a new semantic form.

## K.1 Extraction pass ordering

Recommended deterministic order:

```text
Pass 1  definition/type header
Pass 2  local declarations + debug variables
Pass 3  blocks/statements/terminators + CFG
Pass 4  place/access events
Pass 5  executable references + calls + drops
Pass 6  optional concrete instance resolution
Pass 7  canonicalization + per-owner fingerprint
Pass 8  emit owner record
```

Do not mix graph-store writes into these passes. Normalization should be a pure-ish transformation from compiler facts to owned records so it can be fixture tested.

## K.2 Assignment

Conceptual MIR:

```text
_3 = move _1;
```

Normalized facts:

```text
READ/MOVE _1 at location L
WRITE _3 at location L
VALUE_DEPENDS_ON source=_1 destination=_3
```

For a copy:

```text
_3 = copy _1;
```

emit `COPY`, not `MOVE`.

For a complex rvalue:

```text
_3 = Add(copy _1, const 1);
```

emit:

```text
COPY _1
CONSTANT 1
WRITE _3
VALUE_DEPENDS_ON(_1 -> _3)
```

The raw rvalue remains available for analyses that need exact operation semantics.

## K.3 Storage markers

`StorageLive(local)` / `StorageDead(local)` are not ordinary user assignments. Store them as:

```text
LOCAL_LIFETIME event
```

They may be useful for:

```text
liveness diagnostics
stack-slot reasoning
visual MIR timelines
filtering obviously dead temporary ranges
```

Do not use storage markers as a substitute for Rust source lexical scope.

## K.4 SetDiscriminant

For enum-state mutation:

```text
SetDiscriminant(place, variant)
```

emit:

```text
DISCRIMINANT_WRITE(place, variant)
WRITE-like effect on summarized enum place
```

This matters for state-machine/coroutine and enum dataflow even when no ordinary field assignment appears.

## K.5 FakeRead / PlaceMention / AscribeUserType

Classify explicitly as analysis/metadata facts unless product semantics require them. Do not silently map every place mention to a runtime read.

Suggested properties:

```text
runtime_effect = false/unknown
compiler_analysis_artifact = true
```

The CPG can still retain them for borrowck/debugging views without polluting ordinary read/write queries.

## K.6 `Rvalue::Use`

```text
Use(Copy(place)) -> COPY(place)
Use(Move(place)) -> MOVE(place)
Use(Constant(c)) -> CONSTANT_REFERENCE(c)
```

The destination of the surrounding assignment is a write.

## K.7 references and reborrows

For shared reference creation:

```text
_4 = & _2;
```

emit:

```text
BORROW_SHARED(place=_2)
WRITE(destination=_4)
POINTS_TO / BORROWS relation _4 -> _2
```

For mutable reference:

```text
BORROW_MUT
```

For reborrow, preserve distinction where exposed. Do **not** infer exact non-lexical loan lifetime solely from the existence of the reference rvalue.

## K.8 raw address-of

Raw pointer creation is not a Rust borrow in the same semantic sense:

```text
AddressOf(Const, place) -> ADDRESS_OF_CONST
AddressOf(Mut, place)   -> ADDRESS_OF_MUT
```

Mark the result as entering a less-constrained alias domain. Subsequent dereferences should cross an `UNSAFE_OR_RAW_ALIAS_BOUNDARY` in derived analyses.

## K.9 aggregates

For tuple/struct/enum/closure/coroutine aggregate construction:

```text
read/move/copy each operand
write destination aggregate
CONSTRUCTS kind/type
FIELD_VALUE_DEPENDS_ON where field mapping is available
```

Closure/coroutine aggregates should also link to the generated definition/instance and capture operands.

## K.10 casts

Retain:

```text
cast kind
source operand/type
target type
source span
```

Some casts have interprocedural consequences. In particular, coercion/unsizing to trait objects can introduce vtable dependencies and therefore potential executable references beyond the immediately visible operand. Rustc's monomorphization collector explicitly accounts for vtable method instantiations. ([Monomorphization collector][mono-collector])

## K.11 binary and checked operations

Emit reads of operands plus a value-producing operation node/property. Checked arithmetic may produce tuple/discriminant-like results; preserve the exact normalized rvalue tag rather than flattening everything into an opaque `BINARY_OP` when overflow behavior matters.

## K.12 discriminant and length reads

```text
Discriminant(place) -> DISCRIMINANT_READ
Len(place)          -> READ / LENGTH_READ
```

The second can be useful for bounds/control reasoning and array/slice analyses.

## K.13 thread-local references

Represent the thread-local/static definition endpoint explicitly:

```text
REFERENCES_STATIC
THREAD_LOCAL = true
```

Do not collapse it into an anonymous constant.

## K.14 `Goto`

```text
CFG_NORMAL block -> target
```

## K.15 `SwitchInt`

Emit one edge per target value/range according to what the API exposes plus an otherwise edge:

```text
CFG_CASE value=...
CFG_DEFAULT
```

Also read/copy/move the discriminant operand according to its form.

## K.16 `Return`

Emit:

```text
CFG_EXIT
READ/MOVE/COPY of return local only if that is explicit in normalized body semantics
```

The return place `_0` should remain a distinct local node.

## K.17 `Drop`

A `Drop` terminator is both control flow and executable semantic work:

```text
DROP(place)
USES_DROP_GLUE(type)
CFG_NORMAL -> target
CFG_UNWIND -> cleanup/unwind target if present
```

Resolve `drop_in_place` instance when concrete drop-glue identity is required.

## K.18 `Call`

At minimum:

```text
callable operand use
argument operand uses
destination write
normal CFG successor
unwind CFG successor
callsite source span
```

Then apply the call-resolution decision tree in Appendix C/L.

## K.19 `Assert`

Emit condition operand read plus:

```text
MAY_PANIC
CFG_NORMAL
CFG_UNWIND (if represented)
assert message/category metadata
```

This makes panic-path and cleanup slicing possible without parsing diagnostics strings.

## K.20 `InlineAsm`

Inline assembly is an opaque/unsafe effect boundary. Preserve explicit inputs/outputs where exposed, then conservatively summarize:

```text
MAY_READ_MEMORY
MAY_WRITE_MEMORY
MAY_CALL_UNKNOWN (if applicable to policy)
OPAQUE_EFFECT
```

Avoid asserting purity merely because the MIR wrapper shows no normal Rust call.

## K.21 `Abort`, `Resume`, `Unreachable`

Represent distinct exit semantics:

```text
ABORT_EXIT
UNWIND_RESUME
UNREACHABLE_EXIT
```

Do not merge them into a generic `RETURN` edge.

---

# Appendix L — Complete Rust executable-use and call-graph construction

A Rust CPG that only scans `TerminatorKind::Call` is materially incomplete. The compiler's monomorphization collector is a useful correctness model because it discovers codegen-relevant items by scanning MIR for **function uses, closures, drop glue, constants/allocations and vtable methods**, not merely explicit calls. ([Monomorphization collector][mono-collector])

## L.1 Edge taxonomy

Use separate edge families:

```text
REFERENCES_FN
  function definition/instance becomes a value or otherwise must exist

CALLS_DECLARED
  source/generic callable definition at a callsite

CALLS_INSTANCE
  exact concrete executable instance

MAY_CALL
  conservative indirect/dynamic target candidate

USES_DROP_GLUE
  implicit/explicit destructor instance dependency

USES_VTABLE
  trait-object/vtable dependency

USES_SHIM
  compiler-generated callable adaptation

USES_MONO_ITEM
  broad codegen/executable-use dependency
```

This prevents a common category error:

```text
&foo or fn_ptr = foo  !=  foo()
```

## L.2 Direct function-definition operand

Decision:

```text
operand type is TyKind::FnDef(def, args)
  -> REFERENCES_FN(def,args)
  -> if at Call terminator:
       Instance::resolve(def,args)
       -> exact CALLS_INSTANCE when resolved
```

Store both definition and instance edges; the definition-level graph remains compact and useful when instance enumeration is disabled.

## L.3 Function item coerced to function pointer

Conceptual source:

```rust
let f: fn(i32) -> i32 = foo;
```

Graph:

```text
assignment/cast -> REFERENCES_FN(foo)
local f         -> HOLDS_CALLABLE candidate foo
```

If `f` later appears as the call operand:

```text
CALLSITE -> MAY_CALL foo
```

Upgrade to exact only if flow/points-to analysis proves the candidate set singleton under the selected configuration.

## L.4 Function pointer merge

```rust
let f = if cond { foo } else { bar };
f(x)
```

Basic CPG result:

```text
f reaching callable definitions = {foo, bar}
CALLSITE MAY_CALL foo
CALLSITE MAY_CALL bar
```

A reaching-definitions overlay can provide this precision without requiring a full general-purpose pointer analysis.

## L.5 Closures

Model three identities:

```text
source closure syntax
compiler closure definition
callable closure instance/shim
```

Captures become data dependencies into the closure aggregate. Calls through closure traits may resolve through generated instances/shims. Preserve links back to the source closure for navigation.

## L.6 Static trait dispatch

For a monomorphized/generic call whose callable resolves to a specific impl method:

```text
CALLSITE -> CALLS_DECLARED trait/impl method as represented
CALLSITE -> CALLS_INSTANCE exact resolved instance
INSTANCE -> IMPLEMENTS/SPECIALIZES declaration
```

Use `Instance::resolve` under concrete generic arguments rather than guessing from syntax names. ([Instance][instance])

## L.7 Dynamic trait dispatch

For `dyn Trait`:

```text
receiver type indicates trait object
callsite method slot / trait method
```

Candidate construction can use:

```text
all known impls satisfying trait/type constraints
reachable unsizing coercions
vtable construction facts if private compiler access is enabled
flow-sensitive receiver candidate narrowing
```

Store:

```text
MAY_CALL candidate
confidence = TRAIT_IMPL_OVERAPPROX | VTABLE_OBSERVED | FLOW_NARROWED
```

A candidate set is not a proof that every edge executes.

## L.8 Unsizing/vtable-induced uses

When a value is coerced to a trait object, methods referenced by the produced vtable can become codegen-relevant even if there is no source/MIR `Call` at the coercion site. Rustc's collector has explicit vtable-method creation logic. ([Monomorphization collector][mono-collector])

CPG options:

```text
minimal semantic mode:
  UNSIZES_TO_DYN + IMPLEMENTS edges, derive candidates at query time

executable-use mode:
  materialize USES_VTABLE / USES_MONO_ITEM edges for vtable methods
```

## L.9 Drop glue

For every drop of `T`:

```text
DROP site -> USES_DROP_GLUE(T)
```

Depending on the requested completeness level, resolve nested destructor dependencies recursively. This matters for:

```text
resource cleanup graphs
panic/unwind reachability
codegen reachability
side-effect summaries
```

## L.10 Constants and allocations containing function pointers

CTFE allocations/constants can embed references to functions/statics. If the product wants **codegen/use completeness**, scan those values where the public/private API exposes them, mirroring the compiler collector's allocation scanning strategy. ([Monomorphization collector][mono-collector])

## L.11 Intrinsics and foreign items

Classify explicitly:

```text
intrinsic -> CALLS_INTRINSIC / opaque semantic summary
foreign   -> CALLS_FOREIGN + ABI + symbol endpoint
```

Do not expect an ordinary Rust MIR body.

## L.12 Cross-crate generic instantiation

A generic definition may live in dependency crate X while its monomorphic instance is code-generated in local crate Y. Therefore:

```text
DefinitionKey crate=X
InstanceKey build_context=Y + def=X + args=...
```

Keep definition provenance and codegen/analysis context separate.

## L.13 Completeness modes

Recommended configurable modes:

| Mode | Includes | Cost | Use |
|---|---|---:|---|
| `calls-basic` | explicit call terminators + direct defs | low | navigation |
| `calls-flow` | fn-pointer/closure reaching candidates | medium | blast radius |
| `uses-codegen` | fn refs, drop glue, vtables, constants/allocs | higher | executable-use completeness |
| `instances-full` | concrete instance expansion | potentially very high | deep whole-program analysis |

The graph protocol should record the enabled capability set so consumers know whether a missing edge means “not present” or “not extracted in this mode.”

## L.14 Resolution confidence vocabulary

```text
EXACT_INSTANCE
EXACT_DECLARATION_ONLY
FLOW_SINGLETON
FLOW_CANDIDATE
TRAIT_IMPL_OVERAPPROX
VTABLE_OBSERVED
EXTERNAL_OPAQUE
UNRESOLVED
```

Confidence belongs on the edge/fact, not only in logs.

---

# Appendix M — Continuous update algorithm: end-to-end reference

## M.1 State held by the daemon

```rust
struct WorkspaceState {
    cargo_graph: CargoGraph,
    active_build_contexts: Vec<BuildContextKey>,
    file_to_targets: FileTargetIndex,
    syntax_snapshots: SyntaxSnapshotStore,
    semantic_snapshots: SemanticSnapshotStore,
    in_flight: InFlightCompilations,
    generations: GenerationTable,
    analysis_queue: DerivedAnalysisQueue,
}
```

## M.2 Edit ingestion

On filesystem/editor edit `E`:

```text
1. assign edit sequence number
2. update source buffer
3. increment syntax generation
4. incrementally update Tree-sitter tree/index
5. map edit to likely source owners
6. mark semantic facts for those owners as suspect/stale
7. map file to affected Cargo targets/configurations
8. enqueue semantic refresh key
```

Do not delete semantic facts at step 6.

## M.3 Debounce key

Use:

```text
(package target, build-context hash)
```

not merely workspace path. Multiple edits to files belonging to the same target can coalesce into one compiler invocation.

## M.4 Scheduling priority

Example:

```text
P0 explicit user query needs semantic freshness
P1 currently open file
P2 direct dependency target of open file
P3 background workspace refresh
P4 external dependency enrichment
```

A large repository needs priority/preemption; FIFO alone allows background crates to starve interactive edits.

## M.5 Compile request

```rust
struct CompileRequest {
    build_context: BuildContextKey,
    requested_after_edit_seq: u64,
    target_generation: u64,
    reason: RefreshReason,
}
```

Before starting, check whether a newer request supersedes it.

## M.6 Compiler invocation

Canonical pattern:

```bash
RUSTC_WORKSPACE_WRAPPER=/abs/path/mir-cpg-driver \
  cargo +nightly-2026-08-18 check -p PACKAGE --lib
```

The precise Cargo selector is derived from the target graph; do not string-guess target membership from file paths.

## M.7 Event assembly

The wrapper emits to a staging area:

```text
BEGIN crate invocation
OWNER A
OWNER B
OWNER C
...
END crate invocation { owner_manifest_hash, success }
```

The daemon ignores a stream lacking a valid END marker.

## M.8 Staleness test

When event finishes:

```text
if event.request_generation < current_target_generation:
    event may be internally valid but is stale relative to newer edit
```

Policy choices:

```text
strict interactive:
  discard stale event entirely

progressive background:
  store as historical generation but do not publish as current
```

Never let a late process overwrite a newer published generation.

## M.9 Owner diff

For each stable owner key:

```text
old absent, new present -> ADDED
old present, new absent -> REMOVED
same semantic hash      -> SEMANTIC_UNCHANGED
semantic differs        -> CHANGED
source differs only     -> SOURCE_ONLY_CHANGED
```

## M.10 Graph transaction

```text
BEGIN GRAPH TX
  upsert build-context/crate snapshot
  apply added owner partitions
  replace changed owner partitions
  update source anchors for source-only changes
  remove deleted owner partitions
  recompute/relink cross-owner edges touching changed endpoints
  enqueue/version derived summaries
  set snapshot generation = G
COMMIT
```

Readers see generation `G-1` or `G`, never a mix.

## M.11 Cross-owner edge ownership

To make deletion cheap:

```text
outgoing call/reference edge owned by source owner
incoming index is secondary/query acceleration
```

Then replacing caller owner X automatically removes stale outgoing call edges without finding every target first.

Edges whose semantics belong to an aggregate object (for example global trait-candidate summaries) should be owned by a separately fingerprinted summary partition.

## M.12 Derived analysis invalidation

Raw owner change emits dependency keys:

```text
CFG_CHANGED(owner)
ACCESS_CHANGED(owner)
CALLSET_CHANGED(owner)
TYPE_SUMMARY_CHANGED(owner)
```

Derived analyses declare dependencies:

```text
reaching-def  <- CFG + ACCESS
control-dep   <- CFG
owner-summary <- ACCESS + CALLSET
interproc     <- owner-summary + callees
```

This recreates the **projection/firewall principle** of rustc incremental compilation at the graph layer without coupling to rustc dep-node internals. Rustc's own guide explains how projection queries can stop a broad changed collection from invalidating unaffected fine-grained consumers. ([Incremental compilation][incremental])

## M.13 Compile failure

If source is temporarily invalid:

```text
syntax generation advances
semantic published generation stays at last good G
status = SEMANTIC_FAILED_LAST_GOOD_RETAINED
compiler diagnostics associated with newer source generation
```

Queries can return:

```text
value + freshness metadata
```

rather than false emptiness.

## M.14 Extractor failure

Compiler succeeds, extractor panics/protocol corrupt:

```text
status = EXTRACTOR_FAILED
retain last good semantic graph
quarantine staging event
record rustc/extractor versions + invocation ID
```

Treat this differently from user code failing to compile.

## M.15 Storage failure

Do not acknowledge/publish generation until graph transaction commits. The compiler event can remain in durable staging for retry.

## M.16 Workspace-wide configuration change

Changes to feature set, target triple, toolchain or dependency resolution should generally create or invalidate a **build-context generation** rather than pretending to be a local function edit.

Possible policy:

```text
old config graph retained as historical/inactive
new config compiled into new BuildContextKey
active pointer flips after successful baseline
```

## M.17 Full resynchronization

Schedule periodic or explicit full validation:

```text
cargo metadata refresh
successful cargo check across desired targets
complete owner manifests
compare store for orphan/missing owners
repair inconsistencies
```

Incremental systems need a reconciliation path; correctness should not depend forever on every watcher event being observed.

---

# Appendix N — Graph-store schema and ownership conventions

The following is a logical schema; implement it in a property graph, relational store, columnar tables, or hybrid index as appropriate.

## N.1 Core definition table

```text
Definition(
  definition_key PK,
  build_context_key,
  crate_key,
  qualified_name,
  kind,
  type_key,
  source_anchor,
  owner_generation,
  semantic_fingerprint,
  source_fingerprint
)
```

## N.2 MIR block table

```text
MirBlock(
  owner_key,
  block_index,
  is_cleanup,
  PRIMARY KEY(owner_key, block_index)
)
```

## N.3 MIR operation table

```text
MirOp(
  owner_key,
  block_index,
  slot_kind,
  slot_index,
  normalized_kind,
  raw_variant_tag,
  source_anchor,
  properties_blob,
  PRIMARY KEY(owner_key, block_index, slot_kind, slot_index)
)
```

## N.4 CFG edges

```text
CfgEdge(
  owner_key,
  source_block,
  target_block,
  edge_kind,
  branch_value?,
  PRIMARY KEY(owner_key, source_block, target_block, edge_kind, branch_value)
)
```

Suggested `edge_kind`:

```text
NORMAL
CASE
DEFAULT
CALL_RETURN
DROP_RETURN
UNWIND
CLEANUP
FALSE_EDGE_OR_ANALYSIS_ONLY if relevant to selected MIR surface
```

## N.5 Access table

```text
Access(
  owner_key,
  location_key,
  access_ordinal,
  local,
  projection_key,
  summary_place_key,
  access_kind,
  type_key,
  source_anchor
)
```

Indexes:

```text
(owner_key, location_key)
(summary_place_key, access_kind)
(type_key, access_kind)
```

## N.6 Callsite table

```text
Callsite(
  callsite_key PK,
  owner_key,
  location_key,
  declared_target?,
  exact_instance?,
  dispatch_kind,
  resolution_confidence,
  source_anchor
)
```

## N.7 Call candidate table

```text
CallCandidate(
  callsite_key,
  target_definition_or_instance,
  confidence,
  evidence_kind,
  PRIMARY KEY(callsite_key, target_definition_or_instance, evidence_kind)
)
```

Evidence examples:

```text
DIRECT_FNDEF
REACHING_FN_PTR_DEF
STATIC_TRAIT_RESOLUTION
KNOWN_IMPL
OBSERVED_UNSIZE_VTABLE
FLOW_TYPE_NARROWING
```

## N.8 Instance table

```text
Instance(
  instance_key PK,
  definition_key,
  generic_args_key,
  instance_kind,
  build_context_key,
  type_key,
  abi_key,
  mangled_name?,
  has_body,
  instance_summary_fingerprint
)
```

## N.9 Type table

Use structural interning:

```text
Type(
  type_key PK,
  normalized_kind,
  structural_payload,
  display_name
)
```

`display_name` is presentation, not identity.

## N.10 Derived edges

Store derived analysis in separate versioned partitions:

```text
ReachingDef(analysis_version, owner_key, def_event, use_event)
ControlDependence(analysis_version, owner_key, predicate_block, dependent_block)
SummaryEffect(analysis_version, summary_key, effect_kind, endpoint)
```

This permits recomputing an algorithm without rewriting the raw MIR layer.

## N.11 Generation visibility

Every owner partition belongs to a semantic snapshot generation. The query layer should join against the active generation pointer instead of exposing rows from in-progress staging.

Conceptually:

```text
ActiveSnapshot(build_context_key -> generation)
```

A replacement transaction writes new rows under `generation=G+1`, validates, then flips the pointer.

---

# Appendix O — Stable identity and matching under edits

Rustc's incremental guide explicitly warns that `DefId` can shift across compilation sessions and explains stable `DefPath`/`DefPathHash` forms used to bridge sessions. ([Incremental compilation][incremental])

## O.1 Identity classes

Classify every graph key as one of:

```text
COMPILER_STABLE_CROSS_SESSION
APPLICATION_STABLE_BEST_EFFORT
OWNER_LOCAL_EPHEMERAL
SNAPSHOT_EPHEMERAL
```

Examples:

```text
DefPathHash                 COMPILER_STABLE_CROSS_SESSION (within rustc semantics)
qualified source item key   APPLICATION_STABLE_BEST_EFFORT
MIR Local(7)                OWNER_LOCAL_EPHEMERAL
BasicBlock(3)               OWNER_LOCAL_EPHEMERAL
CrateNum / DefId number     SNAPSHOT_EPHEMERAL
```

## O.2 Named definitions

Preferred key with private stable-id adapter:

```text
StableCrateId + DefPathHash
```

Fallback:

```text
BuildContext-independent crate/package identity
+ canonical module/impl/trait path
+ item kind
+ name
+ signature discriminator if necessary
```

Do not include absolute checkout path unless the product intentionally treats checkout location as identity.

## O.3 Impl blocks

Anonymous impl identity is harder. Candidate fallback key:

```text
crate identity
+ canonical self type
+ optional trait identity
+ source file logical key
+ normalized header fingerprint
```

Source byte offset can be an anchor/tiebreaker, but using it as the entire identity makes every insertion above the impl look like delete+add.

## O.4 Closures and generated owners

For closures/async-generated definitions:

```text
parent stable owner
+ source-anchor neighborhood
+ structural fingerprint
+ local ordinal only as final tiebreaker
```

Matching algorithm between old/new owner snapshots:

```text
1. exact stable key if compiler adapter provides it
2. exact parent + source overlap + structural fingerprint
3. best bipartite match on source distance + type/shape fingerprint
4. if ambiguous: replace parent-generated subgraph; do not fabricate continuity
```

False continuity is worse than a localized delete/add.

## O.5 MIR local identity

Local indices often change when lowering changes. Do not expose a guarantee such as “local 8 remains local 8 across edits.” For user-facing variable continuity, use debug/source association and source-layer variable identity; treat MIR locals as per-owner-version nodes.

## O.6 Basic block identity

Same rule. Block numbers are convenient **coordinates within one body snapshot**, not stable cross-edit IDs. Graph diff should normally replace the MIR partition when the owner semantic fingerprint changes rather than attempt fragile block-by-block identity preservation.

## O.7 Why owner-level replacement is a sweet spot

```text
whole crate replacement:
  simple but high write amplification

statement/block structural diff:
  low theoretical writes but identity matching is fragile/complex

owner-level replacement:
  stable owner identity is tractable
  MIR body sizes are bounded enough for cheap local replacement
  cross-owner edges have deterministic source ownership
```

Therefore owner-level replacement is the default recommendation.

---

# Appendix P — rustc-private enrichment adapter

`rustc_private` can expose facts absent or coarser in `rustc_public`, but it is an implementation API and should be isolated. `run_with_tcx!` exists specifically to provide `TyCtxt` alongside the public surface. ([Initial integration][rustc-public-initial])

## P.1 Adapter contract

```rust
pub trait CompilerEnrichment {
    fn stable_definition_key(&self, public_def: &PublicDefRef) -> Option<StableDefinitionKey>;
    fn stable_source_file_key(&self, span: &PublicSpanRef) -> Option<StableSourceFileKey>;
    fn detailed_place_contexts(&self, owner: &PublicOwnerRef) -> Vec<DetailedPlaceUse>;
    fn borrow_facts(&self, owner: &PublicOwnerRef) -> Option<OwnedBorrowFacts>;
    fn mono_use_facts(&self, owner: &PublicOwnerRef) -> Option<Vec<OwnedMonoUse>>;
    fn vtable_uses(&self, owner: &PublicOwnerRef) -> Option<Vec<OwnedVtableUse>>;
}
```

The rest of the system only sees owned stable structs.

## P.2 Stable IDs

Private/compiler internals can supply rustc stable identity forms such as `DefPathHash`, `StableCrateId`, and stable source file IDs. Convert them immediately to fixed-width owned keys.

Do not store:

```text
TyCtxt
DefId numeric index
CrateNum
interned rustc references
Span internals
```

## P.3 Detailed place contexts

Internal MIR visitors distinguish richer `MutatingUse`, `NonMutatingUse`, and `NonUse` contexts; public `rustc_public` place context is intentionally coarser on current APIs. ([Internal MIR visitor][internal-visitor]) ([Internal place context][internal-place-context])

Recommended pattern:

```text
stable raw schema: AccessKind
private adapter: map internal context -> AccessKind + evidence
public fallback: derive AccessKind structurally
fixture: assert both paths agree on covered cases
```

## P.4 Borrow checker facts

The borrow checker consumes MIR and performs move/initialization analysis and region/loan reasoning. ([Borrow checker][borrow-check])

If exact loan liveness is a product requirement, define an explicit capability:

```text
BORROWCK_LOANS_EXACT = true/false
```

and materialize owned facts such as:

```text
loan id local to owner snapshot
borrowed place
borrow kind
reserve/activation point if applicable
region/point membership or compressed range representation
conflicting access evidence
```

Do not label a derived simple reference-lifetime approximation “borrowck exact.”

## P.5 Monomorphization collector parity

A private adapter can potentially consume or emulate compiler “mentioned/used item” facts more directly. Keep this optional because collector internals can change. The public fallback should implement the executable-use checklist in Appendix L/G.

## P.6 Internal dep-graph caution

It is tempting to subscribe the CPG directly to rustc dep-nodes. Avoid making this a correctness dependency unless absolutely required:

```text
rustc dep-node taxonomy is internal
query keys/providers change
external graph wants different replacement boundaries
compiler green/red concerns computation reuse, not graph-store transaction semantics
```

Use compiler incremental reuse as performance infrastructure and the CPG's own canonical fingerprints as graph-mutation authority.

## P.7 Upgrade containment

Keep all private imports in one or a few crates:

```text
mir-private-adapter/
  stable_ids.rs
  place_context.rs
  borrowck.rs
  mono_uses.rs
  vtable.rs
```

Every nightly bump should fail compilation here first rather than throughout the daemon.

---

# Appendix Q — Validation fixture matrix for a Rust MIR CPG

A robust extractor needs **semantic micro-programs**, not only unit tests of serializer code.

## Q.1 Direct call

```rust
fn callee(x: i32) -> i32 { x + 1 }
fn caller() -> i32 { callee(41) }
```

Assert:

```text
caller has CALLSITE
CALLS_DECLARED callee
resolved CALLS_INSTANCE when instance resolution enabled
argument value dependency
normal return CFG
```

## Q.2 Function reference without call

```rust
fn foo(x: i32) -> i32 { x }
fn make_ptr() -> fn(i32) -> i32 { foo }
```

Assert:

```text
REFERENCES_FN foo
no ordinary runtime CALL edge from make_ptr to foo
```

## Q.3 Function pointer merge

```rust
fn a(x: i32) -> i32 { x }
fn b(x: i32) -> i32 { -x }
fn f(cond: bool, x: i32) -> i32 {
    let g: fn(i32) -> i32 = if cond { a } else { b };
    g(x)
}
```

Assert flow mode:

```text
callsite MAY_CALL a
callsite MAY_CALL b
```

## Q.4 Move vs copy

```rust
fn f(s: String, x: i32) {
    let a = s;
    let b = x;
    drop(a);
    let _ = b;
}
```

Assert:

```text
String transfer uses MOVE
integer transfer uses COPY where MIR represents copy
Drop/use edge for moved String target
```

## Q.5 Borrow/reborrow

```rust
fn f(x: &mut i32) {
    let y = &mut *x;
    *y += 1;
}
```

Assert:

```text
mutable/reborrow fact
write through dereference
place path retains Deref projection
```

## Q.6 Enum switch

```rust
enum E { A(i32), B }
fn f(e: E) -> i32 {
    match e { E::A(x) => x, E::B => 0 }
}
```

Assert:

```text
discriminant read
SwitchInt-like branching after lowering where applicable
case/default CFG representation
variant/downcast projections as emitted
```

Do not overfit the fixture to one pretty-printed MIR formatting; assert normalized semantic facts.

## Q.7 Trait static dispatch

```rust
trait T { fn m(&self) -> i32; }
struct S;
impl T for S { fn m(&self) -> i32 { 1 } }
fn f(s: &S) -> i32 { s.m() }
```

Assert exact resolved impl/instance under current build.

## Q.8 Dynamic dispatch

```rust
fn f(x: &dyn T) -> i32 { x.m() }
```

Assert:

```text
dispatch_kind = DynamicTrait
no fabricated exact target absent narrowing
candidate edges only under enabled candidate analysis
```

## Q.9 Drop glue

```rust
struct D;
impl Drop for D { fn drop(&mut self) {} }
fn f() { let _x = D; }
```

Assert drop executable-use facts even though source has no explicit call to `D::drop`.

## Q.10 Panic/unwind

```rust
fn f(v: &[i32], i: usize) -> i32 { v[i] }
```

Depending on lowering/optimization configuration, assert normalized potential panic/assert/cleanup semantics from the structured MIR actually exposed. Do not require a fixed block number.

## Q.11 Closure capture

```rust
fn f(x: i32) -> impl Fn(i32) -> i32 {
    move |y| x + y
}
```

Assert:

```text
source closure -> generated closure definition
capture of x
closure instance/callable relationship
```

## Q.12 Async function

```rust
async fn f(x: i32) -> i32 { x + 1 }
```

Assert source async owner ↔ generated coroutine/state-machine semantic nodes without demanding stable generated block/local numbering across nightlies.

## Q.13 FFI

```rust
unsafe extern "C" { fn puts(s: *const i8) -> i32; }
```

Assert foreign endpoint/ABI and opaque external effect classification.

## Q.14 Inline assembly

On supported target, include a tiny `asm!` fixture and assert opaque/unsafe effect boundaries plus explicit operands.

## Q.15 Macro expansion

```rust
macro_rules! make { () => { fn generated() -> i32 { 1 } } }
make!();
```

Assert generated semantic definition plus macro/source provenance best effort; never require one-to-one source node mapping.

## Q.16 Incremental body edit sequence

Snapshot 1:

```rust
fn f(x: i32) -> i32 { x + 1 }
fn g() -> i32 { 3 }
```

Snapshot 2:

```rust
fn f(x: i32) -> i32 { x + 2 }
fn g() -> i32 { 3 }
```

Assert:

```text
f semantic fingerprint changes
g semantic fingerprint stays identical
g partition receives zero semantic rewrites
crate generation advances atomically
```

## Q.17 Source-only edit sequence

Insert a comment above `f` with no semantic change. Depending on span policy:

```text
semantic fingerprint unchanged
source fingerprint/ranges may change
source metadata updates without re-running derived semantic analyses
```

## Q.18 Signature change

Change:

```rust
fn f(x: i32) -> i32
```

to another signature and verify:

```text
Definition/type fingerprint changes
callers may recompile/re-resolve as compiler dictates
cross-owner call/type edges update
```

## Q.19 Delete owner

Remove function `f`; successful complete crate manifest must cause a tombstoned/deleted owner partition and removal of outgoing edges. Incoming edges should either disappear/re-resolve in the same successful compile or the compile should fail, in which case last-good graph remains current.

## Q.20 Broken edit / recovery

Temporarily make source syntactically/type invalid:

```text
Tree-sitter/source layer updates
cargo check fails
semantic graph retains prior good generation
freshness says semantic stale/failed
```

Fix source and assert the new successful generation supersedes the last good state.

## Q.21 Feature configuration

```rust
#[cfg(feature = "a")]
fn selected() -> i32 { 1 }
#[cfg(not(feature = "a"))]
fn selected() -> i32 { 2 }
```

Compile both build contexts and assert they do not overwrite each other.

## Q.22 Nightly migration golden gate

For every pinned compiler upgrade:

```text
[ ] extractor compiles
[ ] all normalized enum matches exhaustive
[ ] fixture owner keys stable where promised
[ ] semantic hashes unchanged for semantically unchanged fixtures or migration explained
[ ] call/use completeness matrix passes
[ ] dynamic dispatch confidence unchanged or reviewed
[ ] drop/unwind facts reviewed
[ ] graph schema migration unnecessary or explicitly versioned
[ ] performance baseline within budget
```

---

# Appendix R — Implementation checklist for an LLM coding agent

Use this as the shortest path from an empty crate to a production-capable MIR CPG extractor.

## R.1 Milestone 1 — compiler integration

```text
[ ] pin nightly in rust-toolchain.toml
[ ] install rustc-dev, llvm-tools, rust-src
[ ] compile a binary with rustc_public + required compiler crates
[ ] run callback on a fixture rustc invocation
[ ] enumerate all_local_items()
[ ] prove no compiler object escapes callback
```

## R.2 Milestone 2 — normalized owner records

```text
[ ] definition header
[ ] stable/fallback owner key
[ ] body locals
[ ] blocks
[ ] statements
[ ] terminators
[ ] source anchors
[ ] deterministic serialization
[ ] per-owner semantic fingerprint
```

## R.3 Milestone 3 — CPG raw semantic layer

```text
[ ] CFG normal edges
[ ] CFG unwind edges
[ ] place normalization
[ ] READ/WRITE
[ ] MOVE/COPY distinction
[ ] BORROW/address events
[ ] Drop events
[ ] direct calls
[ ] function references
```

## R.4 Milestone 4 — concrete call precision

```text
[ ] type normalization
[ ] Instance::resolve
[ ] fn-pointer candidate propagation
[ ] closure resolution
[ ] trait static dispatch
[ ] dyn trait candidate policy
[ ] drop glue
[ ] vtable/unsize executable-use mode
```

## R.5 Milestone 5 — incremental graph updater

```text
[ ] Cargo metadata target map
[ ] RUSTC_WORKSPACE_WRAPPER
[ ] invocation side-channel protocol
[ ] complete owner manifest
[ ] semantic/source fingerprints
[ ] owner add/change/remove diff
[ ] atomic graph generation
[ ] stale event rejection
[ ] last-good state on compile failure
```

## R.6 Milestone 6 — fast editor lane

```text
[ ] file watcher/editor event feed
[ ] Tree-sitter incremental source index
[ ] edit -> likely owner/target mapping
[ ] freshness state exposed to query clients
[ ] debounce/coalescing
[ ] interactive compile priority
```

## R.7 Milestone 7 — derived analyses

```text
[ ] reaching definitions
[ ] control dependence
[ ] callable reaching sets
[ ] owner effect summaries
[ ] interprocedural summary propagation
[ ] capability/version flags for each analysis
```

## R.8 Milestone 8 — hardening

```text
[ ] micro-fixture matrix
[ ] edit-sequence integration tests
[ ] compiler-upgrade semantic diff CI
[ ] wrapper probe handling
[ ] build.rs/proc-macro sandbox policy
[ ] bounded compiler concurrency
[ ] durable staging + transaction retry
[ ] metrics/tracing
[ ] full reconciliation job
```

## R.9 Non-negotiable stop conditions

An agent should stop and fix architecture before proceeding if any are true:

```text
rustc_public types stored in long-lived structs outside callback
raw DefId used as persistent database ID
MIR text parser used as primary ingestion
one file edit triggers unconditional full graph delete/rebuild
compiler failure clears semantic graph
explicit Call terminators are the only function-use source
normal and unwind successors are merged without policy
feature/target configurations share one unqualified semantic snapshot
private rustc types leak into graph-store/protocol crates
```

These are structural defects; adding more extraction features on top of them increases migration cost.
