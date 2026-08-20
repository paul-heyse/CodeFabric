# Pyrefly in Rust — Advanced Technical Reference for LLM Coding Agents and Python CPG Infrastructure

> **Version anchor:** Pyrefly **1.2.0** (stable release train used by this reference) / Rust edition **2024**.  
> **Snapshot date:** 2026-08-19.  
> **Primary use case:** embedding or operating Pyrefly as the semantic/type-resolution layer in a **fully Rust-implemented Python code-property-graph (CPG) pipeline**, alongside Ruff-derived syntax/trivia/indexing.  
> **Audience:** LLM coding agents and engineers building repository-scale code intelligence, call graphs, semantic indexes, refactoring systems, dependency analysis, and continuously updated CPGs.

This document follows the operating style of the companion Ruff/DataFusion references: pin the implementation first; distinguish stable from unstable boundaries; establish a mental model before APIs; inventory capabilities; show deployable Rust/process patterns; make cross-system contracts explicit; and end major areas with agent rules, failure modes, anti-patterns, and checklists.

The most important architectural recommendation in this document is:

```text
Do not make the main CPG process depend directly on Pyrefly's internal Rust types.

Instead:

  main Rust CPG service
        │
        │ stable, application-owned DTO protocol
        ▼
  Rust Pyrefly semantic sidecar
        │
        ├─ pinned Pyrefly source/tag
        ├─ pyrefly::query::Query for bulk semantic extraction
        ├─ optional TSP/LSP services
        ├─ optional Glean declaration/xref adapter
        └─ isolates Pyrefly/Ruff-internal version coupling
```

This gives the CPG a **stable semantic contract** while retaining an entirely Rust implementation.

---

# Source-of-truth hierarchy

When implementing against Pyrefly, use sources in this order:

1. **The exact Pyrefly release/tag used by the semantic service** (`1.2.0` for this document).
2. Pyrefly source at that exact tag, especially `ARCHITECTURE.md`, `pyrefly/lib/query.rs`, `pyrefly/lib/query/type_table.rs`, `pyrefly/lib/state/*`, and the TSP protocol types.
3. The Pyrefly `1.2.0` release notes for semantic/behavioral changes that may alter graph output even when APIs compile unchanged.
4. Official Pyrefly documentation at `pyrefly.org`.
5. The companion Ruff reference for the syntax/trivia/indexing side of the architecture.
6. This document as the integration and CPG architecture reference.

Pyrefly's source still explicitly labels the Rust library surface as unstable. The `pyrefly::query` module remains even more explicit:

```text
"Query interface for pyrefly. Just experimenting for the moment -
not intended for external use."
```

The nested library export surface states that it is **not stable and may change during minor version increments**.

Therefore, agents must distinguish two questions:

```text
Can we technically call this Rust API?
    YES, when pinned to exact source.

Should our product expose or compile-couple itself to this API?
    Usually NO.

Recommended:
    isolate the unstable Rust API behind an application-owned adapter/sidecar.
```

### What changed materially in 1.2.0 for CPG integration

The most important 1.2.0 integration changes are:

```text
Query bulk type output:
  old 1.1.x: per-occurrence TypeShape values
  1.2.0:    deduplicated type table + located type indices + structural hashes

Query filtering:
  1.2.0 adds TypeQueryStmtWalker / TypeQueryExprVisitor,
  allowing the sidecar to type only CPG-relevant expressions.

Ruff dependency boundary:
  old 1.1.1: Pyrefly pinned Ruff source crates to a Git revision
  1.2.0:    Pyrefly consumes Ruff component crates 0.0.6 from crates.io

CPG-facing indexing:
  1.2.0 adds pyrefly_glean_schema as a workspace crate and retains
  an internal Glean conversion/report path for declarations/xrefs/search facts.

Build integration:
  1.2.0 adds an experimental bazel-check command in addition to Buck/custom
  build-system infrastructure.

Semantic behavior:
  attrs, functools.partial, singledispatch, pattern matching, TypedDict,
  overload, Enum, lambda, property/descriptor and framework semantics all improve;
  these can change graph facts without changing your adapter signatures.
```

Canonical pinned upstream anchors:

- Pyrefly repository: https://github.com/facebook/pyrefly
- Pyrefly `1.2.0` release: https://github.com/facebook/pyrefly/releases/tag/1.2.0
- Pyrefly `1.2.0` tag: https://github.com/facebook/pyrefly/tree/1.2.0
- Architecture: https://github.com/facebook/pyrefly/blob/1.2.0/ARCHITECTURE.md
- Main crate manifest: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/Cargo.toml
- Main library surface: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/lib/lib.rs
- Experimental query API: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/lib/query.rs
- Query type table: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/lib/query/type_table.rs
- State/incrementality: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/lib/state/state.rs
- Retention levels: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/lib/state/require.rs
- Epoch model: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/lib/state/epoch.rs
- TSP command: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/lib/commands/tsp.rs
- TSP protocol Rust types: https://github.com/facebook/pyrefly/blob/1.2.0/crates/tsp_types/src/protocol.rs
- Glean schema crate: https://github.com/facebook/pyrefly/tree/1.2.0/crates/pyrefly_glean_schema
- Glean report conversion: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/lib/report/glean.rs
- Bazel checker: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/lib/commands/bazel_check.rs
- Pyrefly Python utilities: https://github.com/facebook/pyrefly/tree/1.2.0/crates/pyrefly_python
- Pyrefly type model: https://github.com/facebook/pyrefly/tree/1.2.0/crates/pyrefly_types
- Pyrefly graph utilities: https://github.com/facebook/pyrefly/tree/1.2.0/crates/pyrefly_graph

---

# Feature inventory: what Pyrefly contributes to a Python CPG

A high-quality Python CPG requires multiple representations. Pyrefly should own the **type-aware semantic enrichment layer**, not every layer.

| CPG concern | Primary source | Pyrefly role | Recommended persisted fact |
|---|---|---|---|
| source bytes / line coordinates | Ruff/source store | consumes same source | file digest + coordinate epoch |
| lexical tokens/comments/trivia | Ruff | none | syntax/trivia facts |
| AST structure | Ruff | internally reparses using Ruff parser | normalized AST/CPG nodes from your Ruff frontend |
| scopes / syntactic declarations | Ruff + CPG extractor | validation/enrichment | `DECLARES`, `CONTAINS`, scope IDs |
| inferred expression types | **Pyrefly Query type table** | authoritative static type oracle | `HAS_TYPE` / `TYPE_AT` |
| declared vs computed type | **Pyrefly TSP** | structured distinction | separate declared/computed type facts |
| contextual expected type | **Pyrefly TSP** | contextual typing oracle | optional `EXPECTED_TYPE` |
| symbol definitions | Pyrefly LSP + optional Glean facts | cross-file resolution / bulk index bootstrap | `REFERS_TO` / `DEFINITION_OF` |
| references | Pyrefly LSP index + optional Glean xrefs | validation/supplement | `REFERENCES` |
| call targets | **Pyrefly Query** | type-aware static callee resolution | `CALLS` with confidence/provenance |
| class fields / properties | **Pyrefly Query** | resolved member type/finality | `HAS_MEMBER`, member type |
| inheritance syntax | Ruff | Pyrefly MRO/type semantics | `EXTENDS` + resolved class identity |
| subtype relation | **Pyrefly Query/solver** | `is_subtype` oracle | usually query-time, not all-pairs persisted |
| import syntax | Ruff | module-resolution oracle | `IMPORTS_MODULE`, resolved module target |
| module exports | Pyrefly | export/type dependency model | exported-symbol metadata |
| def-use / dataflow | CPG extractor | Pyrefly internal bindings are useful semantics, but private | normalized def-use edges owned by CPG |
| CFG | CPG extractor | flow typing can validate semantics but no public bulk CFG export | CPG-owned control-flow edges |
| dynamic/reflection uncertainty | CPG policy | types/callees may be incomplete | explicit unresolved/conservative edges |
| incremental invalidation | filesystem + Pyrefly | semantic dependency oracle | snapshot/generation metadata |

### The central rule

```text
Ruff tells you what the source says.
Pyrefly tells you what the source means statically.
The CPG decides how those facts become durable graph identity and edges.
```

---

# Proposed comprehensive documentation map

0. Scope, versioning, and Pyrefly mental model  
1. Installation, build, and deployment surfaces  
2. Workspace/crate architecture  
3. Type-checker execution model: exports → bindings → answers  
4. Module identity, project configuration, search paths, and environments  
5. Ruff parser dependency and cross-version integration boundary  
6. State, transactions, `Require`, epochs, and incrementality  
7. Pyrefly type model (`pyrefly_types`)  
8. Flow-sensitive inference, narrowing, and `Any`/unknown semantics  
9. Experimental Rust `pyrefly::query::Query` API  
10. Whole-file deduplicated type-table extraction  
11. Type-aware call/callee extraction  
12. Class attributes, properties, and member semantics  
13. Qualified targets and subtype queries  
14. Type Server Protocol (TSP)  
15. Language Server Protocol (LSP) as a semantic query surface  
15A. Glean semantic/index export surface
16. Query vs TSP vs LSP/Glean decision matrix  
17. What “complete Python CPG” can and cannot mean  
18. Division of responsibility: Ruff + Pyrefly + CPG logic  
19. Canonical CPG node/edge schema for Pyrefly enrichment  
20. Recommended fully Rust CPG architecture  
21. Stable semantic-sidecar protocol design  
22. Full-repository bootstrap/indexing workflow  
23. Incremental file-change workflow  
24. Coordinate/range synchronization across Ruff and Pyrefly  
25. Semantic identity, provenance, confidence, and stale-fact prevention  
26. Imports, modules, exports, re-exports, and package identity  
27. Inheritance, MRO, protocols, generics, and dispatch  
28. Dynamic Python: reflection, monkey-patching, decorators, descriptors, and fallbacks  
29. Error tolerance and partially invalid repositories  
30. Performance, batching, memory, thread pools, and caching  
31. Persistence and transactional CPG updates  
32. Testing and correctness strategy  
33. Observability and diagnostics  
34. Production deployment and security  
35. Upgrade/version migration discipline  
36. Global anti-pattern inventory  
37. LLM-agent implementation checklist  
38. Reference links and source audit

---

# 0) Scope, versioning, and Pyrefly mental model

## 0.0 Version anchor

This document uses **Pyrefly 1.2.0** as the production anchor. The release was cut at the end of July 2026 and is the latest stable release used by this reference.

The main crate declares:

```toml
[package]
name = "pyrefly"
version = "1.2.0"
edition = "2024"
```

The root workspace includes:

```text
crates/pyrefly_build
crates/pyrefly_bundled
crates/pyrefly_config
crates/pyrefly_derive
crates/pyrefly_glean_schema
crates/pyrefly_graph
crates/pyrefly_lsp_test
crates/pyrefly_python
crates/pyrefly_types
crates/pyrefly_util
crates/tsp_types
pyrefly
pyrefly_wasm
```

`pyrefly_glean_schema` and `pyrefly_lsp_test` are new workspace-level surfaces compared with the 1.1.1 reference. The former is directly interesting to CPG/indexing work; the latter is primarily test infrastructure.

### Agent invariant — exact-source pinning

Do not treat Pyrefly's internal Rust API as a normal stable crate API.

Recommended product pin:

```text
Pyrefly semantic sidecar:
  upstream_repo = facebook/pyrefly
  ref           = 1.2.0
  Cargo.lock    = committed
  upstream SHA  = recorded in build metadata
  protocol      = your own stable DTO schema version
```

If a future Pyrefly version is adopted, upgrade the sidecar and its adapter in isolation, then run semantic parity tests before upgrading production.

### 1.1.1 → 1.2.0 migration summary for this reference

```text
REQUIRED adapter change:
  get_type_shapes_in_file(...) -> get_type_table_in_file(...)

REQUIRED DTO change:
  decode per-file type indices through response.type_table

RECOMMENDED optimization:
  use entry.hash as an upstream structural cache key,
  but verify shape equality before treating a hash collision as identity.

OPTIONAL optimization:
  pass TypeQueryStmtWalker to restrict extraction to expressions
  the CPG actually persists.

BUILD change:
  Pyrefly now consumes Ruff component crates 0.0.6 from crates.io.

SEMANTIC regression/golden change:
  expect improved/different results around Any narrowing, attrs,
  partial/singledispatch, TypedDict, pattern matching, overloads,
  lambdas, descriptors/properties and framework-generated members.
```

---

## 0.1 What Pyrefly is

Pyrefly is a Rust-native Python type checker and language server. For a CPG system, its highest-value capabilities are:

- project-aware module/import resolution;
- full-module binding construction;
- type inference for expressions and variables;
- flow-sensitive refinement;
- class/member/type metadata;
- type-aware call-target resolution;
- definition/reference/navigation infrastructure;
- cross-module dependency tracking;
- incremental rechecking;
- structured type-server protocol support.

It is **not** a CPG and should not be asked to become one.

Pyrefly's own architecture describes a module-centric checker with three conceptual stages:

```text
1. determine module exports
2. convert each module into bindings
3. solve bindings, consulting other modules when needed
```

That is exceptionally useful for a CPG because the semantic output can be layered over syntax nodes already created by the Ruff frontend.

---

## 0.2 Mental model for CPG integration

```text
                    ┌──────────────────────────────┐
                    │        source snapshot       │
                    └──────────────┬───────────────┘
                                   │
                 ┌─────────────────┴──────────────────┐
                 │                                    │
                 ▼                                    ▼
        Ruff syntax frontend                 Pyrefly semantic engine
        --------------------                 -----------------------
        parser + AST                         module resolver
        tokens/trivia                       exports
        source ranges                        bindings/scopes
        syntax nodes                         type solver
        lexical facts                        flow refinement
                 │                           call-target inference
                 │                                    │
                 ▼                                    ▼
        structural CPG facts                 semantic fact DTOs
                 │                                    │
                 └─────────────────┬──────────────────┘
                                   ▼
                            CPG reconciler
                    identity + provenance + confidence
                                   │
                                   ▼
                            atomic graph commit
```

The two parsers are not a correctness problem if both analyze the same immutable source snapshot. The CPG joins them through **source identity and ranges**, not by trying to share AST objects.

---

## 0.3 Pyrefly bindings are conceptually close to a CPG — but do not persist them directly

Pyrefly's architecture document gives an example such as:

```text
define int@0 = from builtins import int
define x@1   = 4: int@0
use x@2      = x@1
anon @2      = print(x@2)
export x     = x@2
```

This looks attractive as a ready-made def-use graph.

However:

```text
Pyrefly internal binding keys
    ≠
stable product-level graph schema
```

Reasons:

- internal `Key`/binding types change;
- byte-offset identity is source-snapshot-local;
- solver artifacts such as `Var` are implementation details;
- not every CPG relation maps one-to-one to a Pyrefly binding;
- direct persistence creates an upgrade migration problem.

**Recommended:** use Pyrefly binding/answer state as a semantic oracle, but map it to your own stable graph facts.

---

## 0.4 “Complete” has two meanings

For Python, no static analyzer can be both perfectly sound and perfectly complete in the presence of:

- `eval` / `exec`;
- runtime import construction;
- monkey-patching;
- dynamic `__getattr__`;
- metaclasses;
- arbitrary descriptors;
- framework-generated code;
- reflection;
- native extensions;
- environment-dependent module loading.

Therefore use the following product language:

```text
Complete CPG schema coverage:
  every syntactic construct and semantic fact category has a representation.

Best-effort static semantic resolution:
  Pyrefly resolves types, definitions, imports, members, and call targets
  where statically knowable.

Explicit uncertainty:
  unresolved/dynamic possibilities are retained rather than silently dropped.
```

A graph that drops unresolved dynamic edges is not “more correct”; it is merely overconfident.

---

## 0.5 Minimum vocabulary

| Term | Meaning in this reference |
|---|---|
| `ModuleName` | Pyrefly logical module identity (`foo.bar`) |
| `ModulePath` | filesystem/memory/bundled/namespace source backing a module |
| `Handle` | module + path + `SysInfo` analysis handle |
| `State` | long-lived analysis state/cache/invalidation coordinator |
| `Transaction` | consistent read/compute view over state |
| `Require` | retention/detail level: Exports, Errors, Indexing, Everything |
| `Bindings` | per-module binding graph and semantic work items |
| `Answers` | solved type/semantic results for bindings/expressions |
| `Type` | Pyrefly's internal rich type representation |
| Query type table | deduplicated normalized type structures plus located response-local indices and structural hashes |
| `PythonASTRange` | Pyrefly source location representation used by Query |
| `Query` | experimental in-process bulk semantic query facade |
| TSP | Type Server Protocol: structured type-service JSON-RPC surface |
| LSP | Language Server Protocol: IDE/navigation surface |
| semantic sidecar | application-owned Rust process wrapping pinned Pyrefly internals |
| semantic snapshot | CPG-visible generation marker for one consistent Pyrefly result set |

---

## 0.6 Agent invariants

```text
Invariant 1:
Pyrefly is the semantic/type layer, not the syntax source of truth for the CPG.

Invariant 2:
Never persist Pyrefly internal enum/discriminant identity as your graph schema.

Invariant 3:
Every semantic fact must carry source snapshot/generation provenance.

Invariant 4:
A type string is display data; structured type-table/TSP types are preferred.

Invariant 5:
A missing callee is not proof that a call has no target.

Invariant 6:
Use exact Pyrefly version pins for in-process Query integration.

Invariant 7:
Prefer a process/DTO boundary when the main service uses a different Ruff component version/source.

Invariant 8:
Do not assume Pyrefly exposes a public bulk CFG or durable def-use API.
Build CFG/structural def-use in the CPG and use Pyrefly to enrich/validate it.
```

---


## 0.7 Pyrefly 1.2.0 release capability delta

Pyrefly 1.2.0 was released on **2026-07-31**. The upstream release notes describe a very large release, and semantic CPG consumers should treat it as a meaningful behavior-version change.

### Type-system and inference improvements

Notable 1.2.0 capabilities include:

```text
attrs:
  recognizes attrs class/field APIs and synthesizes relevant field/method semantics

functools.partial:
  validates bound arguments and computes a residual callable signature
  across generic, overloaded, constructor, bound-method and TypedDict-unpack cases

functools.singledispatch:
  understands registered implementations and improves generic inference

returning Any:
  new opt-in diagnostics can warn when functions return Any

pattern matching:
  better __match_args__, sequence capture, narrowing and exhaustiveness

variadic tuples:
  more precise prefix/suffix unpack inference

TypedDict:
  synthesized required/optional key metadata and improved literal get/pop behavior

overloads:
  improved matching and inference

Enum:
  value typing can become a union of literal member values

lambdas:
  improved contextual typing, including *args/**kwargs

properties/descriptors:
  properties are treated more accurately as data descriptors

cyclic aliases/finality:
  improved cyclic type-alias detection and class/finality reasoning

type-object narrowing:
  isinstance(..., type) better preserves generic type arguments

Self:
  narrowing retains Self information more accurately
```

### Framework/library modeling

1.2.0 expands or improves semantics for:

```text
Pydantic
Django ForeignKey-related behavior
factory-boy
PEP 561 partial stub packages
custom typeshed-path handling for stdlib + third-party stubs
```

These are especially relevant to CPGs because framework-generated declarations and members often do not appear as ordinary source declarations.

### LSP and code-navigation improvements

The 1.2.0 release includes improvements around:

```text
hover detail/verbosity
auto-import behavior
rename across aliases and keyword arguments
go-to-definition into non-Python files where applicable
notebook support
document/workspace symbols
semantic tokens
region/baseline handling
match-capture symbol identity
```

The practical CPG consequence is improved confidence when LSP is used as a fallback or parity oracle for definitions/references.

### Coverage/configuration/build-system changes

Relevant changes include:

```text
coverage reporting moved beyond its earlier experimental status
coverage can stream modules to reduce peak memory
python-platform configuration can describe more than one/all platform values
file-level ignore-errors can be targeted by error code
experimental bazel-check support was added
```

### Performance changes

Upstream 1.2.0 work includes:

```text
working-directory/project-resolution caching
directory-entry and namespace-probe caching
split typeshed stdlib/third-party loading
stub load-map memory reductions
interpreter discovery caching
smaller calculation cells
more lock-free computed-answer reads
```

Do not copy upstream benchmark percentages into capacity planning without reproducing them on your repositories, but expect the 1.2 line to have a different performance envelope from 1.1.x.

### Intentional behavior changes that require new CPG goldens

Two particularly visible changes are:

```text
Any + isinstance evidence:
  can narrow to the tested class instead of retaining Any

unannotated class attributes:
  can union assignments across constructors instead of taking the first assignment
```

This means a 1.2 migration can legitimately change:

```text
HAS_COMPUTED_TYPE
HAS_MEMBER
CALLS / MAY_CALL
dynamic/unresolved flags
diagnostic-linked confidence
```

even when source bytes are unchanged.

Release anchor:
https://github.com/facebook/pyrefly/releases/tag/1.2.0

---

# 1) Installation, build, and deployment surfaces

## 1.0 The four relevant ways to run Pyrefly

For CPG infrastructure, separate these surfaces:

| Surface | Entry point | Best for | Stability |
|---|---|---|---|
| CLI batch checker | `pyrefly check` | CI/smoke validation | user-facing/stable |
| LSP server | `pyrefly lsp` | definitions, refs, hover, navigation | protocol-facing |
| TSP server | `pyrefly tsp` | structured type queries | protocol-facing, evolving |
| Rust Query API | `pyrefly::query::Query` | **bulk type tables/callees/attributes** | explicitly experimental |

The ordinary CLI command set in 1.2.0 includes:

```text
check
snippet
dump-config
buck-check
bazel-check
init
lsp
tsp
infer
coverage
suppress
stubgen
```

Pyrefly 1.2.0 also adds an experimental `bazel-check` command for build-system-driven checking. There is **no upstream `pyrefly query` CLI command** in this version. If your architecture refers to a “Pyrefly query daemon”, that should mean an application-owned Rust daemon wrapping `pyrefly::query::Query`, not a nonexistent upstream command.

---

## 1.1 User-facing installation for diagnostics

For development/verification, install and run the Pyrefly CLI using the official package distribution method supported by your environment, then verify:

```bash
pyrefly --version
pyrefly check
pyrefly dump-config
```

For a production semantic sidecar, prefer a **source-pinned Cargo build** so the binary and Query API are built from the exact audited source.

---

## 1.2 Source-pinned sidecar workspace

Recommended repository layout:

```text
smartref/
  Cargo.toml

  crates/
    python-cpg-core/
    python-ruff-frontend/
    python-semantic-protocol/
    python-cpg-store/
    python-cpg-service/

  vendor-services/
    pyrefly-semantic/
      Cargo.toml
      Cargo.lock
      src/
        main.rs
        backend.rs
        protocol.rs
      pyrefly/                 # git submodule / vendored source / pinned checkout
```

Two viable dependency approaches:

### A. Git dependency pin

```toml
[dependencies]
pyrefly = { git = "https://github.com/facebook/pyrefly", tag = "1.2.0" }
```

This may not be sufficient by itself if you also need internal workspace crates that are path dependencies in Pyrefly's own manifest. Test the actual Cargo graph.

### B. Pinned source workspace / vendoring — preferred

Check out the exact Pyrefly tag and build the adapter as part of a controlled workspace or patch the local manifest deliberately.

Advantages:

- exact source is auditable;
- its path dependencies resolve naturally;
- Cargo.lock can be committed;
- adapting to unstable internal exports is localized;
- no accidental upgrade from a broad version range.

---

## 1.3 Release build

Pyrefly's own root profile enables:

```toml
[profile.release]
debug = 1
lto = true
codegen-units = 1
strip = "debuginfo"
```

For the semantic sidecar, mirror upstream first, then benchmark changes.

```bash
cargo build --release -p smartref-pyrefly-semantic
```

Do not benchmark debug builds.

---

## 1.4 Allocators

Pyrefly's binary uses:

```text
Linux/macOS Cargo builds -> jemalloc
Windows                  -> mimalloc
```

That is an upstream implementation choice, not a requirement for a CPG adapter. If embedding Pyrefly into another process, allocator policy becomes a whole-process decision.

This is another argument for a sidecar: allocator and memory behavior remain isolated.

---

## 1.5 Threading

The CLI obtains a Pyrefly `ThreadCount` and passes it to commands and Query state. Pyrefly also uses Rayon/thread-pool infrastructure internally.

Do not wrap every Pyrefly request in arbitrary Tokio task fan-out. Treat Pyrefly as a parallel engine with its own scheduler.

Recommended:

```text
CPG orchestrator:
  many asynchronous repositories/files

Pyrefly sidecar:
  bounded request queue
  one long-lived State/Query per workspace
  Pyrefly-owned worker/thread pool
```

---

## 1.6 First architecture smoke test

Before attempting CPG extraction, verify all three stages:

```text
1. Config/module resolution succeeds.
2. add_files/type checking succeeds.
3. get_type_table_in_file returns a located, deduplicated type table.
```

Then add call resolution:

```text
4. get_callees_with_location returns expected targets for fixture calls.
```

Finally verify invalidation:

```text
5. edit dependency module
6. notify Pyrefly
7. type/callee output for importer changes
```

That is the minimum proof that the semantic backend is useful for incremental CPG enrichment.

---

# 2) Workspace and crate architecture

## 2.0 Crate map

Pyrefly's architecture and 1.2.0 workspace expose these major crates/surfaces:

| Crate | Role | CPG relevance |
|---|---|---|
| `pyrefly_util` | generic utilities, locking, IO, threading, ranges | operational support |
| `pyrefly_derive` | proc macros (`TypeEq`, `Visit`, etc.) | internal |
| `pyrefly_python` | Python modules/names/paths/AST helpers with no checker-specific typing rules | **module identity and Python utilities** |
| `pyrefly_graph` | indexing/caching of dependent computations | incrementality substrate |
| `pyrefly_bundled` | bundled typeshed / third-party stubs | semantic completeness |
| `pyrefly_config` | configuration + Mypy/Pyright compatibility | project environment |
| `pyrefly_build` | source databases, build-system mapping, module resolver | **monorepo/build identity** |
| `pyrefly_types` | rich type representation and type operations | **type semantics** |
| `pyrefly_glean_schema` | generated Rust Glean/Python index schemas | **optional bulk declaration/xref/search fact contract** |
| `tsp_types` | structured TSP Rust protocol types | **wire DTO source** |
| `pyrefly` | checker, bindings, solver, state, LSP/TSP/query/report logic | **main semantic engine** |
| `pyrefly_lsp_test` | reusable LSP test support | test-only integration aid |
| `pyrefly_wasm` | WASM sandbox | usually not CPG service path |

### 1.2 visibility change worth noticing

`pyrefly::binding` is publicly reachable in 1.2.0. **Do not interpret public module visibility as a stability guarantee.** The crate-level library warning and Query warning remain in force. Use public binding internals only inside a pinned adapter if there is no higher-level Query/TSP/Glean path.

---

## 2.1 `pyrefly_python`

Public modules include:

```text
ast
comment_section
display
docstring
dunder
folding
ignore
keywords
module
module_name
module_path
nesting_context
qname
short_identifier
symbol_kind
sys_info
```

File types include:

```text
.py
.pyi
.ipynb
```

Compiled-module suffix awareness includes:

```text
.pyc
.pyx
.pyd
```

### CPG use

Use Pyrefly's module name/path abstractions inside the sidecar, but normalize them before crossing the application protocol:

```rust
pub struct ModuleIdentityDto {
    pub module_name: String,
    pub source_uri: String,
    pub source_kind: SourceKindDto,
}
```

Do not serialize Pyrefly's `ModulePath` enum directly as your permanent wire contract.

---

## 2.2 `pyrefly_types`

This crate defines the checker type universe. The internal `Type` enum includes, among others:

```text
Literal
LiteralString
Callable
CallableResidual
Function
BoundMethod
Overload
Union
Intersect
ClassDef
ClassType
TypedDict
PartialTypedDict
ShapedArray
IntTuple
NNModule
DataFrame
Series
Int
Size
Dim
TypeLevelDslCall
Tuple
Module
Forall
Var
Quantified
QuantifiedValue
TypeGuard
TypeIs
Annotated
Unpack
TypeVar
ParamSpec
TypeVarTuple
SpecialForm
Concatenate
Args/Kwargs forms
Type
TypeForm
Ellipsis
Any
Never
TypeAlias
UntypedAlias
Sentinel
SuperInstance
```

This is richer than most CPGs should persist directly.

### CPG rule

Persist a normalized, product-owned type graph:

```text
Pyrefly Type
    │
    ▼
Query indexed type-table / TSP type
    │
    ▼
CanonicalType DTO
    │
    ├─ kind
    ├─ qualified_name
    ├─ args
    ├─ callable signature
    ├─ bounds / constraints
    ├─ traits
    ├─ declaration location
    └─ display
```

Never make graph readers understand every internal `pyrefly_types::Type` variant.

---

## 2.3 `pyrefly_graph`

The public top-level modules are:

```text
calculation
index
index_map
```

Pyrefly's architecture describes this crate as infrastructure for indexing values and caching computations that may depend on one another.

For CPG implementation, **do not reuse this crate merely because both systems contain graphs**. It is Pyrefly checker infrastructure, not a general persistent code graph.

---

## 2.4 `pyrefly_bundled`

Bundled typeshed is essential for semantic resolution of:

- builtins;
- standard library;
- many third-party packages;
- signatures unavailable in runtime Python source.

A CPG should retain provenance:

```text
definition provenance:
  source
  stub
  bundled_typeshed
  third_party_stub
  synthesized
```

A definition resolved to a `.pyi` stub is still a useful semantic target even if runtime implementation source is unavailable.

---

## 2.5 `pyrefly_config`

Configuration is part of semantic identity.

Changes to:

- Python version;
- interpreter;
- search path;
- typeshed;
- site packages;
- build-system/module mapping;
- strictness/preset;

can change the meaning of unchanged Python source.

Therefore include a **semantic environment fingerprint** in every workspace generation.

Example:

```text
SemanticEnvironmentFingerprint:
  pyrefly_version
  config_digest
  python_version
  interpreter_identity
  search_path_digest
  typeshed_digest_or_version
  build_system_mapping_digest
```

---


## 2.6 `pyrefly_build` and the module resolver

Pyrefly 1.2.0 substantially expands `pyrefly_build`, including a dedicated `module_resolver` module and source-database abstractions.

CPG implications:

```text
filesystem path != semantic module identity
build target mapping can determine import identity
generated/overlaid paths may differ from physical source paths
```

For ordinary source workspaces, the sidecar can continue using `ModuleName` + `ModulePath`. For Buck/custom/Bazel environments, treat build-system module mapping as part of the semantic environment fingerprint and do not recreate it independently in the CPG.

The ordinary `BuildSystemArgs` surface currently covers Buck and custom query backends; the experimental `bazel-check` command has its own explicit manifest/input path.

---

## 2.7 `pyrefly_glean_schema`

Pyrefly 1.2.0 adds `pyrefly_glean_schema` as a workspace crate and moves generated Glean schema/fact types into it.

The generated Python schema includes predicates for areas such as:

```text
modules
classes
functions/methods
variables/fields
types
qualified/search names
cross references by file/target
source spans
```

Pyrefly also contains an internal `report::glean` conversion path that can serialize per-module Glean entries to JSON.

### CPG use

Glean is an **optional bulk index surface**, not a replacement for Query type tables:

```text
Query type table:
  best for inferred type-at-expression facts

Query callees:
  best for type-aware call targets

Glean:
  attractive for bulk declarations, searchable symbol facts and xrefs

LSP:
  best fallback for interactive/navigation semantics
```

The schema crate is public as a crate, but the conversion/report path inside the main `pyrefly` crate is not a stable product contract. If you use it, expose only application-owned DTOs from the sidecar.

---

# 3) Type-checker execution model: exports → bindings → answers

## 3.0 Canonical pipeline

Pyrefly's architecture gives the essential model:

```text
module source
   │
   ▼
exports
   │
   ▼
bindings
   │
   ▼
solver / answers
   │
   ▼
diagnostics + IDE/query semantic facts
```

More explicitly:

```text
repository
  │
  ├─ determine exports for each module
  │     └─ transitively solve import *
  │
  ├─ per-module binding construction
  │     ├─ declarations
  │     ├─ uses
  │     ├─ scope
  │     ├─ static context
  │     └─ flow context
  │
  └─ solve bindings
        ├─ local type relationships
        ├─ imported exports
        ├─ class/MRO/member queries
        ├─ overload/call solving
        └─ flow refinements
```

---

## 3.1 Why the module-centric design matters

Pyrefly deliberately solves a module substantially as a unit instead of attempting to lazily solve one arbitrary binding like some IDE engines.

Implication for a CPG:

```text
Bad architecture:
  for each syntax node:
      ask Pyrefly to independently type it from scratch

Good architecture:
  load/recheck file/module once
  bulk-extract semantic facts from retained Answers/Bindings
```

This is why `Query::get_type_table_in_file` is attractive in 1.2.0: it turns retained module state into a single deduplicated whole-file semantic batch.

---

## 3.2 Exports

Exports form the cross-module contract.

A module importer often depends not on “the whole file” but on:

- existence of a name;
- type of a name;
- metadata for a name;
- wildcard export set;
- class information;
- type alias information.

Pyrefly 1.2.0's state model tracks these categories explicitly for fine-grained invalidation.

### CPG consequence

When file A changes:

```text
Do not automatically invalidate every semantic fact in every importer.

Instead:
  let Pyrefly recompute dependency impact,
  then refresh semantic facts only for affected modules/files.
```

The CPG can still maintain a conservative fallback invalidation path if the semantic sidecar cannot expose an affected-file set.

---

## 3.3 Bindings

Bindings model the semantic work required to understand a module.

Useful mental categories:

```text
definition binding
use binding
anonymous check/work item
export binding
class field binding
class metadata binding
MRO/base binding
type parameter binding
type alias binding
```

Do not infer public API stability from these concepts. They are implementation internals.

---

## 3.4 Answers

`Answers` stores solved semantic results and exposes internal mechanisms such as:

```text
get_type_at(...)
get_type_trace(range)
solver()
get_idx(...)
```

The Query facade uses these to extract whole-file types and callees.

### CPG rule

The CPG should never need to know whether a type came from:

```text
answers.get_type_at(binding_index)
```

versus:

```text
answers.get_type_trace(range)
```

The semantic sidecar normalizes both into:

```rust
LocatedTypeFact {
    range,
    type_,
    confidence,
    provenance,
}
```

---

## 3.5 Recursive solving and `Type::Var`

Pyrefly may introduce a temporary `Var` when recursively solving mutually dependent type relationships, then constrain/resolve it.

Do not persist intermediate solver variables as user-facing graph types unless explicitly debugging solver state.

Persist solved/final client-facing representations.

---

## 3.6 Flow types

Pyrefly distinguishes static annotation/type information from flow-refined knowledge.

Example:

```python
x: int | None = get_x()

if x is not None:
    use(x)
```

The `x` inside the branch can have a narrower computed type than its declaration.

For CPG queries, this distinction is valuable:

```text
DECLARED_TYPE(x) = int | None
COMPUTED_TYPE(x @ use) = int
```

TSP's declared/computed/expected type separation is especially valuable here.

---

# 4) Module identity, project configuration, search paths, and environments

## 4.0 Module identity is semantic, not just a filepath

The same file can mean different things depending on:

- import root;
- package layout;
- namespace packages;
- generated-source mapping;
- Python environment;
- site packages;
- typeshed;
- build system.

Therefore:

```text
FileId != ModuleId
```

Recommended model:

```rust
FileIdentity {
    repo_relative_path,
    content_digest,
}

ModuleIdentity {
    module_name,
    source_uri,
    environment_id,
}
```

One file may be mapped differently under different project environments.

---

## 4.1 `ModuleName` and `ModulePath`

Inside the Pyrefly sidecar, construct semantic handles using Pyrefly's own abstractions.

Conceptually:

```rust
Handle::new(
    module_name,
    module_path,
    sys_info,
)
```

The `Query` API creates handles after consulting its `ConfigFinder`.

---

## 4.2 Config finder

The pinned library exposes helper construction through the unstable library export surface, including:

```text
default_config_finder
default_config_finder_with_overrides
```

A Query backend is conceptually:

```rust
let finder = default_config_finder(None);
let query = Query::new(finder, thread_count);
```

Treat this as a pinned-source pattern, not a guaranteed future signature.

---

## 4.3 Interpreter and environment discovery

Project-aware typing may require querying the configured Python environment.

For an untrusted-repository service, this is a security boundary.

Deployment choices:

```text
trusted developer workstation:
  interpreter auto-discovery may be acceptable

multi-tenant service:
  do not execute arbitrary repository-provided Python
  use controlled environment manifests / prebuilt env mapping
  sandbox any interpreter query
```

---

## 4.4 Typeshed and site packages

To reproduce type results, the semantic sidecar must know:

- bundled typeshed version from its Pyrefly build;
- site-package paths;
- custom stubs;
- source packages;
- project search paths.

Store a hash of this environment in the semantic snapshot.

---

## 4.5 Build-system integrations

Pyrefly can integrate with build-system/module-mapping logic. In a CPG system, treat module mapping as a pluggable semantic input:

```text
Filesystem resolver
Buck/source-database resolver
Custom build query resolver
Generated-source resolver
Monorepo custom resolver
Bazel manifest-driven checker path
```

### Bazel in 1.2.0

The new experimental `bazel-check` command consumes a JSON manifest describing:

```text
target identity
check roots: sources + stubs
search path / explicit imports
workspace/repository roots
path overlays
Python version/platform
preset + error severity configuration
```

For a CPG built over a Bazel monorepo, do not independently guess Python module names from physical paths if this build-derived mapping is available. Treat the Bazel target/search-path manifest as part of `SemanticEnvironmentFingerprint`.

The `pyrefly_build` library also exposes a dedicated module resolver and source-database abstractions for build-integrated environments.

Do not assume filesystem path → dotted module name is always sufficient.

---

# 5) Ruff parser dependency and the cross-version integration boundary

## 5.0 Critical fact

Pyrefly 1.2.0 itself depends on Ruff component crates, including:

```text
ruff_python_ast       0.0.6
ruff_python_parser    0.0.6
ruff_source_file      0.0.6
ruff_text_size        0.0.6
ruff_notebook         0.0.6
ruff_annotate_snippets 0.0.6
```

The important 1.2.0 change is that these are now crates.io version dependencies rather than the specific Ruff Git revision used by Pyrefly 1.1.1.

The companion Ruff reference produced for this CPG architecture is pinned to Ruff 0.16.1 / component crates 0.0.7.

Those are still **not the same Rust crate universe**.

---

## 5.1 Duplicate-type failure mode

If the main CPG crate depends on:

```text
ruff_python_ast 0.0.7
```

while Pyrefly depends on:

```text
ruff_python_ast 0.0.6
```

Rust sees distinct types even when names look identical.

Example conceptual failure:

```text
expected ruff_python_ast(0.0.6)::Expr
found    ruff_python_ast(0.0.7)::Expr
```

### Rule

Never design the integration around passing your Ruff AST node objects directly into Pyrefly unless the entire workspace deliberately resolves exactly one identical Ruff component version.

---

## 5.2 Best boundary: source snapshot + ranges

The safest shared contract is:

```text
path / module identity
source digest
UTF-8 source bytes or verified shared file
byte/TextRange or LSP range
semantic generation
```

Both engines may parse the same source independently.

That is acceptable because parsing is not the expensive semantic work and it decouples release trains.

---

## 5.3 In-process single-workspace option

If you insist on one binary with both systems, there are now two realistic choices:

```text
A. Pin the CPG Ruff frontend to Pyrefly's 0.0.6 component family.
B. Patch Pyrefly and the CPG to one deliberately selected Ruff component family.
```

Then:

1. use exact `=` versions or workspace/path patches;
2. run `cargo tree -d`;
3. verify only one copy of every Ruff type-bearing crate crosses the boundary;
4. compile the full workspace;
5. run parser/type/call integration goldens;
6. treat every Pyrefly upgrade as a coordinated Ruff upgrade.

This is viable but higher maintenance.

---

## 5.4 Sidecar option — recommended

```text
main process:
  Ruff 0.16.1 / component crates 0.0.7
  stable CPG domain types

sidecar:
  Pyrefly 1.2.0
  Pyrefly's Ruff component crates 0.0.6
  unstable internal Query types

wire:
  JSON / MessagePack / Cap'n Proto / Protobuf
  application-owned versioned DTO
```

Benefits:

- no duplicate Rust type conflicts;
- independent upgrades;
- crash isolation;
- memory isolation;
- allocator isolation;
- stable graph-facing contract;
- easy rollback.

The shift from a Git-pinned Ruff revision to crates.io 0.0.6 reduces dependency awkwardness, but it does **not** remove the version-boundary problem when the main frontend uses 0.0.7.

---

## 5.5 Why TSP alone is not always enough

TSP gives a cleaner upstream protocol for position-oriented type queries, but Pyrefly 1.2.0 still does not expose a single TSP request equivalent to:

```text
give me the deduplicated inferred type table
and all type-aware call targets for this file
```

For initial CPG indexing, thousands of cursor-position type requests are less efficient than Query's retained whole-file traversal.

Therefore the strongest deployment pattern remains:

```text
custom Rust semantic sidecar:
  Query bulk type table
  Query bulk callees
  Query member/subtype utilities
  optional Glean bulk symbol/xref facts
  optional TSP structured position queries
  optional LSP navigation queries
```

---

# 6) State, transactions, `Require`, epochs, and incrementality

## 6.0 State mental model

```text
State
  ├─ configuration
  ├─ module cache
  ├─ dependency metadata
  ├─ file/memory overlays
  ├─ epochs
  ├─ worker pool
  └─ committed semantic generations

Transaction
  ├─ consistent read/compute view
  ├─ may be committable
  ├─ runs requested modules
  └─ queries AST / bindings / answers / errors
```

`Query` owns a long-lived `State`.

---

## 6.1 `Require` levels

Pyrefly 1.2.0 defines:

```text
Exports
Errors
Indexing
Everything
```

Semantics:

| Level | Retains / computes |
|---|---|
| `Exports` | enough for module dependency/export information |
| `Errors` | diagnostics |
| `Indexing` | enough references/index state for IDE features |
| `Everything` | AST + bindings + answers + type trace required by Query bulk extraction |

`Query::add_files` runs target handles with `Require::Everything`, which is why later type/callee queries can inspect retained AST/bindings/answers.

### CPG rule

For files whose semantic facts will be bulk extracted:

```text
Require::Everything
```

For transitive dependencies that only need exports:

```text
Require::Exports
```

This asymmetry is important for memory control.

---

## 6.2 Epoch model

Pyrefly uses an epoch model with the invariant:

```text
checked >= computed >= changed
```

The implementation description is:

```text
checked: last point where correctness was validated
computed: last point where value was recomputed
changed: dependency-side change generation

If A depends on B and A.computed < B.changed:
    A is stale
```

Do not expose Pyrefly's internal epoch objects directly. Use them as conceptual support for your own semantic-generation barrier.

---

## 6.3 Fine-grained dependency tracking

Pyrefly state tracks cross-module dependencies by categories including:

```text
name existence
name type
name metadata
wildcard export set
class
type alias
specific exported computation key
```

This is far more precise than:

```text
module A imports module B -> invalidate A whenever any byte of B changes
```

### CPG opportunity

If the sidecar can expose the set of rechecked/changed modules after a transaction, the CPG can update only those semantic partitions.

If not exposed by the public Query facade, add an application-owned adapter inside the pinned sidecar. Do **not** reach into private CPG code from the main process.

---

## 6.4 `Query::change_files`

The Query facade:

1. clears its type-string cache;
2. creates a committable transaction;
3. invalidates categorized file events;
4. runs export-level recomputation;
5. commits;
6. re-adds all files previously loaded via `add_files`.

That is a convenience API, not necessarily the most optimal future bulk-update mechanism.

### Agent rule

Start with `change_files` for correctness.

Only optimize re-add/requery behavior after profiling.

---

## 6.5 File event model

Recommended sidecar request:

```json
{
  "op": "apply_changes",
  "base_generation": 41,
  "changes": [
    {
      "uri": "file:///repo/pkg/a.py",
      "kind": "modified",
      "content_digest": "..."
    }
  ]
}
```

Sidecar response:

```json
{
  "generation": 42,
  "rechecked_modules": ["pkg.a", "pkg.b"],
  "invalidated_modules": ["pkg.a", "pkg.b"],
  "diagnostic_count": 3
}
```

`rechecked_modules` is an **application-owned extension**; it is not guaranteed by upstream Query 1.2.0.

---

## 6.6 Snapshot barrier

Never combine:

```text
Ruff parse from file generation N+1
Pyrefly type facts from generation N
```

Atomic enrichment key:

```text
(repo_id, file_id, content_digest, semantic_generation)
```

Commit only when the sidecar confirms that its analyzed content digest matches the CPG syntax snapshot.

---

## 6.7 Incremental pipeline

```text
filesystem event
   │
   ▼
content store creates immutable snapshot
   │
   ├─► Ruff structural update
   │
   └─► Pyrefly change notification
            │
            ▼
       semantic recheck
            │
            ▼
     affected-file fact batches
            │
            ▼
       graph delta planner
            │
            ▼
       one atomic CPG commit
```

---

## 6.8 Agent checklist

```text
[ ] Keep one long-lived Pyrefly State/Query per workspace/environment.
[ ] Use Everything only for files requiring bulk semantic extraction.
[ ] Model semantic generations explicitly.
[ ] Join semantic facts to exact source digests.
[ ] Treat configuration/environment changes as global semantic invalidations.
[ ] Start with Query::change_files correctness before custom optimization.
[ ] Prefer Pyrefly fine-grained dependency impact over all-importer invalidation.
[ ] Do not persist internal Epoch or ModuleDeps structs as product schema.
```

---

# 7) Pyrefly type model (`pyrefly_types`)

## 7.0 Internal type universe vs CPG type universe

Pyrefly's internal type enum is optimized for solving Python typing semantics. It includes solver- and implementation-specific variants that a durable CPG should not mirror one-for-one.

Think in two layers:

```text
Pyrefly internal:
  highly expressive solver representation
  may contain implementation artifacts
  optimized for type operations

CPG canonical:
  durable semantic representation
  normalized for graph queries
  version-independent
```

---

## 7.1 Important internal categories

### Nominal/class types

```text
ClassDef
ClassType
Type(...)
SelfType
SuperInstance
```

CPG normalization:

```text
TypeNode(kind="class", qname="pkg.C", args=[...])
```

Keep distinction between:

```text
class object:
  type[pkg.C]

instance:
  pkg.C
```

because call resolution and member lookup differ.

---

### Function/callable types

```text
Function
BoundMethod
Callable
Overload
Forall<Function/Callable>
CallableResidual
```

CPG normalization should preserve:

```text
callable
  ├─ parameters
  ├─ return type
  ├─ overload branches
  ├─ generic parameters
  └─ bound receiver type, where relevant
```

Do not flatten every callable into one display string.

---

### Union/intersection types

```text
Union
Intersect
```

Persist ordered/canonicalized type children, not only:

```text
"int | str | None"
```

A structured graph supports:

```text
find calls where receiver may be None
find all union alternatives containing pkg.Protocol
```

---

### Type variables and generics

```text
Quantified
TypeVar
ParamSpec
TypeVarTuple
ElementOfTypeVarTuple
Forall
Concatenate
Unpack
Args/Kwargs forms
```

CPG should support generic-variable identity separately from its rendered name. Two `T` declarations in separate scopes are not the same type variable.

Recommended:

```rust
CanonicalTypeVar {
    semantic_id,
    display_name,
    declaration,
    variance,
    bound,
    constraints,
}
```

---

### TypedDict

Internal types distinguish:

```text
TypedDict
PartialTypedDict
```

The Query type-table representation exposes traits:

```text
typed_dict
partial_typed_dict
```

Persist these traits because structural map-like types are useful for:

- field-level dataflow;
- key access;
- schema-like analysis;
- API payload modeling.

---

### Literals

```text
Literal
LiteralString
Sentinel
```

Literal types are especially useful for:

- flow refinement;
- branch predicates;
- enum/sentinel analysis;
- framework dispatch.

Avoid stripping all literal information during ingestion.

---

### `Any`

Pyrefly internally distinguishes:

```text
AnyStyle::Explicit
AnyStyle::Implicit
AnyStyle::Error
```

This is extremely valuable for confidence modeling.

Conceptually:

```text
Explicit Any:
  user intentionally opted out of typing

Implicit Any:
  information absent / inference default

Error Any:
  analysis degraded because an error occurred
```

If your integration boundary exposes this distinction, preserve it.

If using a protocol surface that collapses these cases, record the loss:

```text
type_precision = "unknown_any_origin"
```

---

### `Never`

`Never` / `NoReturn` can signal:

- unreachable continuation;
- always-raising function;
- impossible narrowed branch.

Use it when constructing or validating control-flow reachability.

---

### Solver `Var`

`Type::Var` is a recursive-solving placeholder.

**Never treat it as a stable Python user type.**

If a `Var` escapes a final query unexpectedly:

```text
mark semantic fact as incomplete
record Pyrefly diagnostic/context
do not assign a fake user-visible type
```

---

## 7.2 Deduplicated type table: preferred Query output in 1.2.0

Pyrefly 1.2.0 replaces the older public per-occurrence `TypeShape` Query output with a deduplicated type-table representation.

The primary response types are conceptually:

```rust
pub struct TypeTableResponseData {
    pub type_table: Vec<SerializedTypeTableEntry>,
    pub types: Vec<LocatedTypeTableRef>,
}

pub struct LocatedTypeTableRef {
    pub location: PythonASTRange,
    pub type_index: usize,
}

pub struct SerializedTypeTableEntry {
    pub kind: IndexedTypeShapeKind,
    pub hash: u64,
}
```

The indexed shape categories are:

```text
Named {
  name,
  args: [type_index...],
  unspecified_type_arg_count,
  traits
}

Callable {
  params: [type_index...],
  return_type: type_index,
  is_staticmethod
}

TypeVariable {
  name,
  bounds: [type_index...]
}
```

### Why the type table is a better CPG transport

It moves repetition out of each occurrence:

```text
many occurrences of builtins.int
        │
        └── all point to one per-response type-table entry
```

Each table entry also carries a structural `xxh64` hash. Pyrefly computes the hash from the complete normalized shape fields used for structural equality. That lets a client maintain a cross-file/cross-request cache such as:

```text
upstream_structural_hash -> decoded normalized shape
```

**Do not treat a 64-bit hash alone as cryptographic identity.** On a cache hit, verify that the decoded shape matches if correctness depends on collision freedom.

### Important normalization behavior

The 1.2.0 type-table converter normalizes internal types into CPG-friendly shapes. Examples:

```text
ClassDef             -> typing.Type[qualified_class]
ClassType            -> qualified named type + args
Union + None         -> typing.Optional[...]
other Union          -> typing.Union[...]
Function/Callable    -> callable params + return + staticmethod bit
TypedDict            -> named shape + TypedDict/PartialTypedDict trait
anonymous TypedDict  -> builtins.dict[str, union-of-field-value-types]
Tuple                -> typing.Tuple[...] + Tuple trait
TypeVar              -> type variable + bounds
TypeAlias reference  -> module-qualified alias + args
Any                  -> typing.Any
Never/NoReturn       -> typing.Never / typing.NoReturn
DataFrame/Series     -> underlying normalized type
```

The converter also recognizes 1.2-era internal specialized types such as `IntTuple`, `DataFrame`, `Series`, `Int`, `TypeLevelDslCall`, shaped arrays and NN modules. Your permanent CPG schema should not copy those internal variants; retain the normalized table shape unless the application genuinely needs the extra distinction.

---

## 7.3 Suggested canonical type DTO

Decode the per-file table into an application-owned recursive or interned type representation.

```rust
#[derive(Serialize, Deserialize)]
pub struct CanonicalType {
    pub kind: CanonicalTypeKind,
    pub display: Option<String>,
    pub upstream_structural_hash: Option<u64>,
    pub provenance: TypeProvenance,
    pub precision: TypePrecision,
}

pub enum CanonicalTypeKind {
    Named {
        qname: String,
        args: Vec<TypeId>,
        traits: Vec<String>,
        unspecified_type_arg_count: Option<u32>,
    },
    Callable {
        params: Vec<TypeId>,
        returns: TypeId,
        is_staticmethod: bool,
    },
    TypeVariable {
        name: String,
        bounds: Vec<TypeId>,
    },
    Unknown,
}
```

The sidecar may materialize a recursive DTO, or it may preserve an indexed table and let the CPG store decode/intern it. The latter is often more efficient.

---

## 7.4 Type interning in the CPG

Do not create a duplicate type node for every expression.

Recommended two-level strategy:

```text
Pyrefly response:
  local type_index -> SerializedTypeTableEntry

sidecar/global cache:
  structural_hash + verified shape -> CanonicalTypeId

CPG:
  ExpressionOccurrence --HAS_COMPUTED_TYPE--> CanonicalTypeId
```

The Pyrefly `type_index` is **response-local**. Never persist it as global identity.

A reasonable canonical key is:

```text
(environment_id, canonical_structural_type_hash)
```

where your own canonical hash is computed from your normalized schema. The upstream Pyrefly hash can accelerate lookup, but your storage identity should remain application-owned.

---

## 7.5 Agent rules

```text
Use Query type-table entries/TSP structured types before display strings.
Never persist response-local type_index as global identity.
Use structural hash as a cache hint, not the sole correctness proof.
Preserve callable structure and is_staticmethod.
Preserve generic args and unspecified generic arity.
Preserve TypedDict/tuple traits.
Distinguish class object from class instance.
Do not persist Type::Var as a normal user type.
Treat Any as a precision signal, not just another nominal type.
Intern normalized types instead of duplicating by occurrence.
Use filtered walkers when the CPG deliberately stores only a subset of expressions.
```

---

# 8) Flow-sensitive inference, narrowing, and uncertainty

## 8.0 Why flow types matter to a CPG

A purely declaration-based graph misses Python's local semantics.

Example:

```python
def f(x: int | None):
    if x is None:
        return
    x.bit_length()
```

At the call:

```text
declared x: int | None
computed x: int
```

The second fact is more useful for call-target resolution and member lookup.

---

## 8.1 Declared, computed, expected

The three type concepts are distinct:

```text
declared type:
  what annotation/declaration says

computed type:
  what static analysis infers at this expression/location

expected type:
  what surrounding context expects here
```

Example:

```python
x: list[str] = []
```

Conceptually:

```text
declared type of x       = list[str]
computed type of []      = list[str] or a context-shaped list type
expected type of []      = list[str]
```

TSP is designed to expose these concepts as separate request types.

---

## 8.2 CPG edge model

Recommended:

```text
Symbol/Expression
  --DECLARED_TYPE--> Type

ExpressionOccurrence
  --COMPUTED_TYPE--> Type

ExpressionOccurrence
  --EXPECTED_TYPE--> Type
```

Do not overwrite declaration type with flow-refined type.

---

## 8.3 Narrowing provenance

Optional advanced model:

```text
NarrowingFact {
    range,
    before_type,
    after_type,
    cause:
      isinstance
      is_none
      truthiness
      match_pattern
      type_guard
      type_is
      equality_literal
      exception_path
      other
}
```

Pyrefly may not expose narrowing provenance directly through Query/TSP. If you need it, derive the cause from Ruff AST/control-flow and compare types before/inside branch.

---

## 8.4 Flow joins

At control-flow joins:

```python
if cond:
    x = 1
else:
    x = "s"
use(x)
```

type becomes roughly:

```text
Literal[1] | Literal["s"]
```

Your CPG CFG should own the branch/join topology. Pyrefly validates/enriches the value type.

---

## 8.5 Unknown vs unresolved

Never collapse all of these into one state:

```text
no type fact returned
Any
unknown
unbound
Never
analysis error
unsupported dynamic construct
```

Suggested `TypePrecision`:

```rust
enum TypePrecision {
    Precise,
    Union,
    ExplicitAny,
    ImplicitAny,
    ErrorAny,
    Unknown,
    Unbound,
    Unsupported,
}
```

Populate as much as the integration surface exposes.

---


## 8.6 Pyrefly 1.2.0 behavioral changes that affect CPG semantics

Semantic goldens from 1.1.x should not be expected to remain byte-for-byte identical.

Most important for graph consumers:

- **`Any` narrowing:** when `isinstance` supplies concrete runtime evidence, Pyrefly 1.2.0 narrows an `Any` value to the tested class rather than preserving `Any` as the computed flow type. This can turn previously unresolved member/call facts into resolvable ones.
- **Unannotated class attributes:** assignments across constructors are unioned rather than simply taking the first assignment. Member type and dispatch edges can therefore widen.
- **Pattern matching:** more precise class/sequence capture and exhaustiveness behavior can alter occurrence types in match arms.
- **Lambda contextual typing:** lambda parameters, `*args` and `**kwargs` receive better contextual types.
- **Properties/descriptors:** data-descriptor behavior is modeled more accurately, affecting attribute write/read semantics.
- **TypedDict/overload/Enum improvements:** these can change return/argument/member types without any CPG source-structure change.

Treat these as **semantic engine changes**, not graph corruption. Update expected semantic goldens deliberately.

---

# 9) Experimental Rust `pyrefly::query::Query` API

## 9.0 Status

The module itself says:

```text
Query interface for pyrefly.
Just experimenting for the moment.
Not intended for external use.
```

Treat every signature in this section as **pinned to 1.2.0**.

---

## 9.1 Why Query is nevertheless compelling for CPG generation

It exposes exactly the high-throughput operations a repository index wants:

```text
load/recheck files
bulk deduplicated type tables for a file
filtered type traversal
bulk call targets for a file
class member metadata
subtype test
qualified callable target resolution
```

Unlike cursor-oriented protocols, it traverses retained AST/answers once per bulk query.

---

## 9.2 Conceptual construction

Pinned-source shape:

```rust
use pyrefly::query::Query;

// ConfigFinder and ThreadCount imports are part of the pinned Pyrefly
// source graph; keep these imports inside the semantic adapter crate.

let query = Query::new(config_finder, thread_count);
```

Recommended application wrapper:

```rust
pub struct PyreflyBackend {
    query: Query,
    generation: AtomicU64,
    workspace: WorkspaceIdentity,
}
```

Do not expose `Query` outside this crate.

---

## 9.3 `add_files`

Shape:

```rust
pub fn add_files(
    &self,
    files: Vec<(ModuleName, ModulePath)>
) -> Vec<String>
```

Behavior:

- remembers files for future change handling;
- creates a committable transaction;
- runs handles at `Require::Everything`;
- collects diagnostics;
- commits state.

### CPG usage

Initial bootstrap:

```text
discover source files
resolve module identities
batch add_files
record diagnostics
bulk query semantic facts
```

Do not call `add_files` independently before every fact query.

---

## 9.4 `change_files`

Shape:

```rust
pub fn change_files(&self, events: &CategorizedEvents)
```

Use for filesystem/source changes.

The sidecar should translate your file watcher event model into Pyrefly's event model.

---

## 9.5 `get_type_table_in_file`

Primary 1.2.0 shape:

```rust
pub fn get_type_table_in_file(
    &self,
    name: ModuleName,
    path: ModulePath,
    walker: Option<&TypeQueryStmtWalker>,
) -> Option<TypeTableResponseData>
```

Response:

```text
type_table:
  deduplicated normalized type shapes

types:
  (PythonASTRange, type_index) references into that table
```

Unlike the old 1.1.x bulk path, the response intentionally carries **no per-location display string**. This avoids repeated stringification and repeated normalized type structures.

### CPG mapping

```text
response = query.get_type_table_in_file(...)

for local_entry in response.type_table:
    canonical_id = intern(local_entry.hash, local_entry.kind)

for located in response.types:
    syntax_node = range_index.best_expression_match(located.location)
    type_id = canonical_id_for(response.type_table[located.type_index])
    emit HAS_COMPUTED_TYPE(syntax_node, type_id)
```

Validate every `type_index` against the response table before use.

---

## 9.6 Filtered type extraction with `TypeQueryStmtWalker`

Pyrefly 1.2.0 adds public Query aliases for caller-supplied traversal:

```rust
pub type TypeQueryExprVisitor<'a> =
    dyn FnMut(&'a Expr, Option<&'a Expr>) -> bool + 'a;

pub type TypeQueryStmtWalker =
    dyn for<'a> Fn(&'a [Stmt], &mut TypeQueryExprVisitor<'a>);
```

Pass `None` for the default full expression traversal.

Pass a custom walker when you deliberately want only CPG-relevant expression classes, for example:

```text
call.func expressions
attribute receivers/expressions
names
decorators
class bases
returns
assignment RHS
annotations
```

This is a significant new 1.2 optimization lever. It lets the Pyrefly sidecar avoid materializing located type facts that the graph will immediately discard.

### Important callback rule

The expression callback records one selected expression but does **not** recursively walk children for you. A custom walker owns recursion policy.

---

## 9.7 Range matching strategy

Do not require exact range equality for every expression.

Some representations may use:

- identifier-only range;
- attribute range;
- whole expression range;
- adjusted Python AST location.

Use a layered matcher:

```text
1. exact byte range + expected node kind
2. exact start/end converted from line/col
3. smallest enclosing expression with same start
4. semantic occurrence synthetic node
```

Never attach a type to an arbitrary overlapping node.

---

## 9.8 Type-table timing APIs

Pyrefly 1.2.0 exposes:

```rust
get_type_table_in_file_with_timing(...)
get_type_table_in_file_with_timing_filtered(...)
```

`TypeQueryTiming` now contains:

```text
located_count
setup
transform
total
```

The old 1.1.x timing breakdown for per-occurrence stringification/cache/shape profiles is no longer the public bulk path because the type table intentionally avoids per-location display string generation.

Benchmark:

```text
located expressions / second
setup cost
type-table transform cost
total extraction latency
response bytes
unique table entries / located occurrences
global structural-hash cache hit rate
```

---

# 10) Whole-file deduplicated type-table extraction

## 10.0 Recommended bulk DTO

A sidecar can either pass through the indexed structure nearly as-is or decode it immediately.

Efficient application-owned response:

```json
{
  "semanticGeneration": 42,
  "contentDigest": "...",
  "typeTable": [
    {
      "hash": 123456789,
      "kind": "named",
      "name": "builtins.int",
      "args": [],
      "traits": []
    }
  ],
  "types": [
    {
      "range": {"startByte": 120, "endByte": 125},
      "type": 0
    }
  ]
}
```

The `type` integer is scoped to this response only. The main CPG should resolve it before persisting occurrence facts.

Convert Pyrefly `PythonASTRange` to byte ranges inside the sidecar if possible so the main CPG uses one coordinate system.

---

## 10.1 Why whole-file extraction is superior

For a 5,000-expression file:

```text
TSP per-expression:
  thousands of JSON-RPC requests
  repeated range conversion
  protocol overhead
  repeated lookup work

Query type table:
  one retained module state
  one controlled AST traversal
  one deduplicated type structure
  one response batch
```

The 1.2.0 table is even better suited to large files because repeated types are serialized once per response.

---

## 10.2 Deduplication and structural hashes

Two separate kinds of deduplication matter:

```text
occurrence deduplication:
  NEVER remove distinct expression occurrences

type-structure deduplication:
  desirable; many occurrences can share one canonical type
```

Pyrefly already performs per-response structural deduplication and emits a structural hash on each table entry.

Recommended:

```text
response type_index
    -> verify local table entry
    -> lookup global cache by upstream hash
    -> compare canonical shape on hit
    -> intern to CPG TypeId
```

Never persist `type_index`; it is not stable across files or calls.

---

## 10.3 Names vs non-name expressions

Query deliberately uses a stronger path for name expressions:

```text
try BoundName key
else Definition key
else type trace
```

This can give better symbol typing than treating every node as a generic range trace.

In 1.2.0 callee extraction also has a fallback for name expressions that can recover a `ClassDef` when the generic type trace is absent, improving constructor-call resolution.

Do not reproduce these rules in the main CPG; let Pyrefly own them.

---

## 10.4 Full traversal vs filtered traversal

Default:

```text
walker = None
=> visit all expressions
```

Filtered:

```text
walker = Some(application policy)
=> record only selected expression positions
```

Use full traversal when:

- the CPG stores flow-sensitive types broadly;
- coding agents frequently inspect arbitrary expressions;
- storage is inexpensive relative to re-query latency.

Use filtered traversal when:

- the graph only needs high-value semantic anchors;
- very large repositories make full occurrence typing expensive;
- a second on-demand semantic query path exists.

A practical filtered set:

```text
Name
Attribute
Call.func and Call
Subscript
Decorator
Class base
Return value
Assignment RHS
Annotation expression
Comprehension target/iterable when dataflow matters
```

---

## 10.5 Type fact provenance

Every applied type fact should carry:

```text
engine = "pyrefly"
engine_version = "1.2.0"
backend = "query_type_table"
semantic_generation
content_digest
upstream_structural_hash
precision
```

If the type was additionally refined by a TSP declared/expected query, retain both provenance sources instead of overwriting the Query fact.

---

## 10.6 Type-table validation checklist

```text
[ ] type_index is in bounds
[ ] local table is decoded before occurrence persistence
[ ] recursive/index references are cycle-safe
[ ] upstream hash collision is not assumed impossible
[ ] canonical CPG type identity includes semantic environment
[ ] source digest matches the structural graph generation
[ ] filtered-walker coverage is recorded in backend capabilities
```

---

# 11) Type-aware call/callee extraction

## 11.0 Why this is one of Pyrefly's highest-value CPG features

A syntax-only call:

```python
x.run()
```

does not identify the target.

Pyrefly can combine:

```text
receiver type
bound-method type
class MRO
function metadata
overloads/generics
constructor behavior
```

to produce a much stronger static call edge.

---

## 11.1 `get_callees_with_location`

Shape:

```rust
pub fn get_callees_with_location(
    &self,
    name: ModuleName,
    path: ModulePath,
    location: Option<PythonASTRange>,
) -> Option<Vec<(PythonASTRange, Callee)>>
```

With `location = None`, the implementation traverses the file.

`Callee` contains:

```rust
pub struct Callee {
    pub kind: String,
    pub target: String,
    pub class_name: Option<String>,
}
```

Known kind strings in the implementation include:

```text
function
method
classmethod
staticmethod
```

---

## 11.2 Resolution behaviors visible in the implementation

The pinned Query code handles cases including:

- ordinary functions;
- bound methods;
- classmethods;
- staticmethods;
- overloaded functions;
- generic/forall callables;
- classes/constructors;
- callable instances through `__call__`;
- `__init__` / `__new__` selection;
- unions of possible callees;
- TypedDict-related method naming;
- decorator callables;
- some wrapped functions such as `lru_cache` special handling;
- callable parameters in selected cases.

It uses type traces and MRO-aware solver operations.

---


## 11.2A 1.2.0 call-resolution implications

The 1.2.0 Query implementation adds a useful fallback when resolving the type of a call target that is a bare `Name`: if the ordinary expression type trace is absent, it can consult the bound/definition key and recover a `ClassDef`. This improves constructor-target recovery for cases where a generic range trace alone is insufficient.

The broader 1.2 type-system work also improves the semantic inputs used by call resolution:

```text
functools.partial residual signatures
singledispatch registrations
generic/overload inference
lambda contextual typing
descriptor/property semantics
TypedDict call/argument behavior
framework-generated members
```

Do not hard-code these special cases in the CPG. Persist the target(s) Pyrefly supplies and keep engine version/provenance so graph diffs can be explained across upgrades.

---

## 11.3 CPG call edge schema

Do not store only:

```text
CALL -> "pkg.C.m"
```

Recommended:

```rust
CallEdge {
    call_node_id,
    target_symbol_id: Option<SymbolId>,
    target_qname: String,
    dispatch: DispatchKind,
    receiver_type: Option<TypeId>,
    confidence: ResolutionConfidence,
    provenance: SemanticProvenance,
}
```

Dispatch kinds:

```text
direct_function
instance_method
class_method
static_method
constructor_init
constructor_new
callable_object
decorator
dynamic_unknown
```

---

## 11.4 Multiple targets

For:

```python
x: A | B
x.run()
```

Pyrefly may return multiple callees.

Persist all:

```text
CALL --MAY_CALL--> A.run
CALL --MAY_CALL--> B.run
```

Do not arbitrarily choose one.

---

## 11.5 Missing targets

`get_callees_with_location` can return no target for cases Pyrefly cannot model.

Represent:

```text
call.resolution = unresolved
```

Optionally add a conservative syntactic edge:

```text
candidate_name = "run"
receiver_type = whatever Pyrefly knows
```

Never interpret empty result as “dead call”.

---

## 11.6 Query implementation caveats

The current code contains TODOs and panic branches for unsupported/unexpected type shapes.

Therefore:

```text
Query callee extraction = high-value best-effort semantic analysis
not a formal total function over all valid Python
```

The sidecar must catch process-level failures and degrade safely.

Best deployment isolation:

```text
one sidecar per workspace / worker
supervised restart
last-good semantic snapshot remains queryable
```

---

## 11.7 Target identity reconciliation

Pyrefly returns string `target` names.

Reconcile them to durable CPG symbol nodes:

```text
1. exact qualified symbol lookup
2. module + class + function lookup
3. stub/external symbol node
4. unresolved external target placeholder
```

Do not drop a resolved external target simply because source is not in the repository.

---

## 11.8 Decorators

Query explicitly considers class/function decorators.

This is valuable because decorators are executable call sites and may influence runtime semantics.

Model both:

```text
DECORATED_BY
```

structural relation and:

```text
CALLS
```

execution-like relation for decorator evaluation/application where your CPG semantics support it.

---

# 12) Class attributes, properties, and member semantics

## 12.0 `get_attributes`

Shape:

```rust
pub fn get_attributes(
    &self,
    name: ModuleName,
    path: ModulePath,
    class_name: &str,
) -> Option<Vec<Attribute>>
```

where:

```rust
pub struct Attribute {
    pub name: String,
    pub kind: Option<String>,
    pub annotation: String,
    pub is_final: bool,
}
```

Current implementation can identify property-like fields and finality.

---

## 12.1 CPG member model

For class `C`:

```text
ClassNode(C)
  --DECLARES_MEMBER--> MemberNode(x)
  --DECLARES_MEMBER--> MethodNode(f)
```

Enrich member:

```text
member.type
member.is_final
member.kind = property | field | ...
member.declaring_class
```

---

## 12.2 String annotation caveat

`get_attributes` returns the member annotation as a display string rather than a type-table entry.

If structured member types are important:

- query expression/name types in the file and join by declaration range; or
- extend the pinned sidecar to transform the internal `Type` through the same indexed type-table canonicalization machinery;
- avoid parsing the rendered type string if you can access structured internals.

---

## 12.3 Inherited members

`get_attributes` is oriented around fields associated with the named class definition.

Do not assume it returns the entire effective MRO member set.

For effective member lookup:

```text
source declared members
+
Pyrefly definition/member resolution at attribute-use locations
+
MRO-aware call resolution
```

---

## 12.4 Descriptor semantics

Properties/descriptors make a naive field graph incorrect:

```python
class C:
    @property
    def x(self) -> int: ...
```

The syntax node is a method, but use-site semantics are attribute-like.

Preserve both identities:

```text
function declaration
property/member semantic role
getter relationship
```

---


## 12.5 Pyrefly 1.2.0 member/framework improvements

1.2.0 expands the quality of class/member semantics that can reach the CPG:

```text
attrs:
  class fields, validators, converters and generated methods

Pydantic:
  improved field/validator/config modeling

Django:
  improved ForeignKey and related field semantics

factory-boy:
  improved factory-generated class behavior

unannotated class attributes:
  constructor assignments are unioned across assignments

properties:
  modeled as data descriptors more accurately
```

These improvements can create or change **synthesized members**, effective member types and property behavior.

One Query-specific caveat remains: when `get_attributes` falls back from a field defined in a method, the 1.2.0 implementation may inspect the first recorded value expression. Treat the method as a convenient member summary API, not a complete per-assignment history. Your CPG should still keep every source assignment structurally.

---

# 13) Qualified targets and subtype queries

## 13.0 `resolve_target_from_qualified_name`

Shape:

```rust
pub fn resolve_target_from_qualified_name(
    &self,
    name: ModuleName,
    path: PathBuf,
    qualified_name: &str,
) -> Option<Vec<Callee>>
```

The current implementation builds a synthetic snippet:

```python
x = qualified_name
```

type-checks it, then resolves callable target information.

Use cases:

- validate a known qualified callable;
- resolve configuration-specified plugin targets;
- map framework hook names to callables.

Do not call this for every ordinary source call; use the bulk callee extractor.

---

## 13.1 `is_subtype`

Shape:

```rust
pub fn is_subtype(
    &self,
    name: ModuleName,
    path: PathBuf,
    lt: &str,
    gt: &str,
) -> Result<bool, String>
```

The implementation caches parsed type strings and asks the solver's subtype relation.

### CPG use

Good query-time operations:

```text
Is receiver type a subtype of framework.BaseHandler?
Does this function accept objects compatible with ProtocolX?
Does this class satisfy TypedDictionary-style request?
```

Avoid materializing every pairwise subtype relation in the graph.

---

## 13.2 Inheritance vs subtyping

These are not identical.

Persist structural inheritance:

```text
C --EXTENDS--> B
```

from class base syntax + resolved base identity.

Use Pyrefly subtype queries for semantic questions involving:

- protocols;
- generics;
- unions;
- structural compatibility;
- transformed/synthetic types.

---

# 14) Type Server Protocol (TSP)

## 14.0 Why TSP matters

TSP is a protocol-level type service designed to let another language tool consume type-checker results.

For a CPG, its biggest advantages are:

- structured serialized types;
- declaration locations;
- protocol negotiation;
- snapshot semantics;
- process isolation;
- no direct Pyrefly internal Rust type coupling.

---

## 14.1 Starting the server

Pyrefly 1.2.0 includes:

```bash
pyrefly tsp
```

Relevant options include:

```text
--indexing-mode
--workspace-indexing-limit
--transport stdio
--transport ipc://<name>
```

The non-internal default workspace indexing limit in the pinned source is 2000 user files.

For a repository CPG service, set this deliberately.

---

## 14.2 TSP transport

Supported main transport in the command surface:

```text
stdio
ipc://<name>
```

Recommended service integration:

```text
same host:
  IPC/local named pipe

simple worker:
  stdio child process
```

Do not expose an unauthenticated TSP endpoint over a public network.

---

## 14.3 Protocol version

The generated protocol type has versions up through a current:

```text
0.4.1
```

Always negotiate rather than assuming.

Sidecar initialization should record:

```text
pyrefly version
TSP protocol version
server capabilities
```

---

## 14.4 Core request methods in 1.2.0

Generated TSP request method names include:

```text
typeServer/connection
typeServer/getComputedType
typeServer/getDeclaredType
typeServer/getExpectedType
typeServer/getPythonSearchPaths
typeServer/getSnapshot
typeServer/getSupportedProtocolVersion
typeServer/resolveImport
```

Notification:

```text
typeServer/snapshotChanged
```

---

## 14.5 TSP type representation

The protocol contains a structured `Type` union with categories such as:

```text
BuiltIn
Declared
Function
Class
Union
Module
Typevar
Overloaded
Synthesized
TypeReference
```

This is far richer than a hover string.

Common type metadata includes:

```text
id
kind
flags
typeAliasInfo
declaration
```

---

## 14.6 Type flags

Flags include:

```text
Instantiable
Instance
Callable
Literal
Interface
Generic
FromAlias
Unpacked
Optional
Unbound
```

These map naturally to CPG type traits.

---

## 14.7 Declarations

TSP distinguishes:

```text
regular declaration
synthesized declaration
```

Regular declarations can carry:

```text
category
name
source node/range
```

Categories include:

```text
intrinsic
variable
parameter
type parameter
type alias
function
class
import
```

This is excellent for definition graph enrichment.

---

## 14.8 Source nodes

TSP `Node` contains:

```text
URI
zero-based range using LSP-like character offsets
```

The main CPG should convert once into its canonical byte coordinate system.

Always honor position encoding/capability semantics; do not assume Unicode character count equals UTF-8 byte offset.

---

## 14.9 Synthesized declarations

Do not throw away synthesized declarations.

Examples conceptually include:

```text
dataclass-generated methods
builtins/intrinsics
decorator-generated members
```

Represent:

```text
SymbolNode {
  origin = synthesized
  source_uri = conceptual owning file if supplied
}
```

This is crucial for a complete semantic graph.

---

## 14.10 Type aliases

TSP includes rich type alias metadata such as:

```text
name
full name
module name
file URI
scope id
PEP 695-style indicator
type parameters
type arguments
computed variance
```

This is substantially better than flattening aliases to targets.

Persist alias identity separately:

```text
AliasTypeNode --ALIASES_TYPE--> TypeNode
```

---

## 14.11 Snapshot notifications

Use:

```text
typeServer/getSnapshot
typeServer/snapshotChanged
```

as a semantic cache epoch when operating through TSP.

Never cache a TSP response indefinitely without binding it to a snapshot.

---

## 14.12 TSP strengths and weaknesses for CPG

### Strengths

- structured type protocol;
- declaration metadata;
- expected/declared/computed types;
- import resolution;
- snapshot model;
- clean process boundary.

### Weaknesses

- cursor/location-oriented requests;
- no upstream bulk whole-file type dump in this pinned version;
- no Query-equivalent whole-file callee list;
- protocol evolves;
- large initial CPG bootstrap may require many requests.

---

# 15) LSP as a semantic query surface

## 15.0 Useful Pyrefly LSP features

Pyrefly language services include capabilities such as:

```text
hover
document symbols
workspace symbols
inlay hints
completion
definition
declaration
type definition
references
document highlight
rename
semantic tokens
signature help
implementation
call hierarchy
```

For a CPG, the most relevant are:

```text
definition
declaration
type definition
references
implementation
call hierarchy
workspace/document symbols
```

---

## 15.1 Definition resolution

Use LSP definition as a cross-check or fallback when:

- Query returns a callable target string but graph identity is ambiguous;
- import alias resolution is complex;
- a symbol references a stub/external source;
- multiple definitions are possible.

However, bulk definition requests per identifier are expensive.

---

## 15.2 References

Pyrefly's `Require::Indexing` exists specifically to retain enough information for features such as references.

For full initial CPG generation, a dedicated bulk internal adapter is preferable if available.

For incremental “find references” or user-driven queries, LSP is appropriate.

---

## 15.3 Call hierarchy

LSP call hierarchy is an IDE feature, not automatically a complete repository call graph.

Use it as:

```text
validation
interactive fallback
cross-check
```

not as the only source for initial graph construction.

---

## 15.4 Symbol indexes

Workspace/document symbols can help:

- enumerate externally visible entities;
- cross-check syntax symbol extraction;
- reconcile generated/synthetic semantics.

The Ruff AST remains the source of truth for physical syntax declarations.

---



## 15.5 Pyrefly 1.2.0 LSP changes relevant to CPG parity

When using LSP as a validation or fallback surface, 1.2.0 improves several areas that can affect comparison results:

- richer hover output and import detail;
- more capable auto-import behavior;
- rename support across import aliases and keyword-argument references;
- go-to-definition support for some non-Python targets;
- improved notebook behavior;
- document/workspace symbol and semantic-token behavior;
- better symbol identity for pattern-match captures.

Therefore differential tests such as:

```text
CPG REFERS_TO vs LSP definition
CPG REFERENCES vs LSP references
CPG symbol search vs workspace symbols
```

should be rebaselined when moving from 1.1.x to 1.2.0.

---

# 15A) Glean semantic/index export surface

## 15A.0 Why this exists in the 1.2.0 reference

Pyrefly 1.2.0 promotes the generated Glean schema into the `pyrefly_glean_schema` workspace crate. The main checker retains an internal conversion path that builds per-module Glean facts from a `Transaction`/`Handle` and serializes them to JSON.

This is especially relevant to a CPG because Glean's Python schema is already organized around index-like entities and relations.

---

## 15A.1 Useful fact families

The generated schema includes predicate families for concepts such as:

```text
VariableDeclaration
FunctionDeclaration / method declarations
Module
Type
qualified/scoped names
Search*ByName indexes
XRefsViaNameByFile
XRefsViaNameByTarget
source byte spans
```

The exact schema is large and generated; do not copy every predicate into the CPG.

---

## 15A.2 Best role in the architecture

Recommended:

```text
Ruff:
  structural source graph

Pyrefly Query type table:
  expression types

Pyrefly Query callees:
  call targets

Pyrefly Glean:
  optional bulk declaration/xref/search bootstrap

TSP:
  structured declared/computed/expected type detail

LSP:
  interactive navigation fallback
```

Glean can reduce the need to issue many LSP definition/reference requests during initial indexing.

---

## 15A.3 Stability boundary

`pyrefly_glean_schema` provides generated schema types, but Pyrefly's conversion/report pipeline is not presented as a stable public CPG SDK.

Therefore:

```text
do not persist raw Glean JSON as the product graph contract
do not make graph readers depend on Glean predicate version numbers
normalize selected facts inside the Pyrefly sidecar
```

Good DTO examples:

```text
BulkDeclarationFact
BulkReferenceFact
SearchSymbolFact
SourceSpan
```

---

## 15A.4 Reconciliation rule

If Ruff and Glean both describe the same source declaration:

```text
Ruff owns source node identity.
Glean/Pyrefly enrich semantic resolution and xrefs.
```

Do not create duplicate declaration nodes merely because the semantic index has its own fact identity.

---

# 16) Query vs TSP vs LSP/Glean decision matrix

| Need | Preferred surface | Why |
|---|---|---|
| full-file inferred expression types | **Query type table** | one traversal, deduped structure, structural hashes |
| selected high-value expression types | **Query type table + filtered walker** | avoid unnecessary occurrences |
| bulk call targets | **Query** | type/MRO-aware batch extraction |
| class field/property/finality summary | **Query** | direct member API |
| subtype checks | **Query** | direct solver-backed operation |
| declaration-rich type object at one position | **TSP** | structured protocol type |
| declared vs computed vs expected type | **TSP** | explicit request distinction |
| import resolution | **TSP / Pyrefly module resolver** | project-aware |
| semantic snapshot notifications | **TSP** | protocol concept |
| bulk declarations/xrefs/search facts | **Glean adapter** | index-oriented fact families |
| definition/reference navigation fallback | **LSP** | mature IDE operations |
| call hierarchy / implementation navigation | **LSP** | interactive IDE semantics |
| cold repository bootstrap | **Query + optional Glean** | fewer round trips |
| persistent CPG schema | **application-owned DTOs** | isolates all upstream instability |

### Recommended hybrid

```text
initial/reindex:
  Query type table
  Query callees
  Query members
  optional Glean declaration/xref facts

incremental:
  Pyrefly change_files / state invalidation
  requery affected type tables/callees/index facts

interactive deep semantic question:
  TSP or LSP

persistence:
  normalized CPG-owned facts only
```

The key 1.2.0 change is that Query's deduplicated table makes it an even stronger bulk ingestion path than in 1.1.x.

---

# 17) What “complete Python CPG” can and cannot mean

## 17.0 Completeness target

For this architecture, define completeness as:

```text
Every source construct:
  represented structurally.

Every statically resolvable semantic relationship:
  represented with Pyrefly-derived enrichment.

Every unresolved semantic relationship:
  represented explicitly as unresolved/conservative rather than omitted.

Every fact:
  versioned and traceable to source + semantic engine generation.
```

This is a useful, achievable engineering definition.

---

## 17.1 Structural completeness

Ruff/CPG frontend should ensure coverage for:

```text
modules
imports
classes
functions
lambdas
parameters
type parameters
assignments
annotations
attributes
calls
operators
comprehensions
generators
control statements
exceptions
with statements
match patterns
decorators
string interpolation
subscripts
literals
type aliases
```

Pyrefly is not responsible for producing this inventory.

---

## 17.2 Semantic completeness

Attempt to enrich:

```text
symbol identity
definition target
reference target
module/import target
declared type
computed type
expected type where valuable
class/member type
call target(s)
class base target
subtype compatibility
synthesized declarations
stub/external definitions
flow-refined occurrence type
```

---

## 17.3 Dynamic incompleteness is a fact

Example:

```python
name = input()
fn = getattr(module, name)
fn()
```

A robust graph records:

```text
CALL kind = dynamic
target = unresolved
receiver/module type = known if available
target-name source = dynamic expression
```

It does not discard the call because no target was statically identified.

---

## 17.4 Soundness policy

Choose one:

### Precision-first

Only emit Pyrefly-resolved targets.

```text
Pros: low false positives
Cons: missing edges under dynamic/Any code
```

### Conservative

Emit semantic targets plus syntax-derived possible targets.

```text
Pros: better impact-analysis recall
Cons: more false positives
```

### Hybrid — recommended

```text
CALLS_RESOLVED       exact/high-confidence Pyrefly target
MAY_CALL             type/union/conservative target
DYNAMIC_CALL         unresolved dynamic site
```

Queries choose edge classes by purpose.

---

# 18) Division of responsibility: Ruff + Pyrefly + CPG logic

## 18.0 Responsibility table

| Capability | Ruff frontend | Pyrefly | CPG logic |
|---|---:|---:|---:|
| source ranges | primary | secondary | normalize |
| AST | primary | internal copy | persist normalized structure |
| comments | primary | no | optional graph facts |
| parse errors | primary | also observes | reconcile |
| lexical scope skeleton | yes | deeper semantic | graph identity |
| binding resolution | partial/linter semantics | **strong** | persist target |
| expression types | no | **primary** | canonicalize/intern |
| calls | syntax site | **target resolution** | edge semantics |
| CFG | **derive** | flow typing internally | persist |
| def-use | derive structural/dataflow | semantic validation | persist |
| imports | syntax | **module resolution** | resolved edges |
| base classes | syntax | type/MRO resolution | resolved `EXTENDS` |
| comments/trivia edits | primary | no | patch system |
| persistence | no | no | **primary** |
| history | Git/gix | no | **primary** |
| incremental file events | watcher | consumes | coordinate |
| semantic invalidation | no | **primary** | apply graph delta |

---

## 18.1 Do not duplicate Pyrefly's type checker

Avoid implementing in the CPG:

```text
Python overload resolution
generic type solving
protocol compatibility
flow narrowing
MRO member typing
descriptor type semantics
TypedDict typing
ParamSpec solving
TypeGuard / TypeIs semantics
```

Those belong in Pyrefly.

---

## 18.2 Do not outsource graph topology to Pyrefly

The CPG should still build:

```text
AST parent/child
scope containment
statement order
control flow
basic def-use
assignment/value flow
exception flow
import syntax
decorator syntax
class base syntax
```

because these are stable structural facts and should remain available even when Pyrefly cannot type-check a file.

---

## 18.3 Redundant facts can be useful

Some overlap is desirable:

```text
Ruff says:
  identifier `x` occurs here

Pyrefly says:
  this occurrence resolves to declaration D and has type T

CPG says:
  occurrence node X -> REFERS_TO D
                    -> HAS_TYPE T
```

The overlap acts as a consistency check.

---

# 19) Canonical CPG node/edge schema for Pyrefly enrichment

## 19.0 Core node kinds

Recommended stable node families:

```text
Repository
File
Module
Scope
Symbol
Class
Function
Parameter
TypeParameter
Variable
Member
Import
Expression
Call
Literal
Type
TypeAlias
ExternalSymbol
SyntheticSymbol
Diagnostic
```

Do not create a new node kind for every upstream AST or Pyrefly enum variant.

---

## 19.1 Stable node identity

### Source-backed symbols

Possible key:

```text
(repo_id,
 file_id,
 semantic_symbol_kind,
 declaration_anchor)
```

where declaration anchor is resilient:

```text
qualified name
+
declaration start byte
+
syntax fingerprint
```

Do not use byte offset alone across edits.

---

## 19.2 Occurrence nodes

Expression/reference occurrences are source-snapshot-specific.

Key:

```text
(file_generation, start_byte, end_byte, syntax_kind)
```

Historical graph layers can keep old generations separately.

---

## 19.3 Type nodes

Key by canonical semantic form:

```text
(environment_id, canonical_type_hash)
```

If type alias identity matters:

```text
AliasTypeNode != expanded target TypeNode
```

---

## 19.4 Core semantic edges

```text
File         --DECLARES_MODULE--> Module
Module       --DECLARES--> Symbol
Scope        --CONTAINS--> Symbol/Expression
Symbol       --HAS_DECLARED_TYPE--> Type
Expression   --HAS_COMPUTED_TYPE--> Type
Expression   --HAS_EXPECTED_TYPE--> Type
Reference    --REFERS_TO--> Symbol
Call         --CALLS--> Function/Symbol
Call         --MAY_CALL--> Function/Symbol
Class        --EXTENDS--> Class
Class        --DECLARES_MEMBER--> Member
Member       --HAS_TYPE--> Type
Import       --IMPORTS_MODULE--> Module
Import       --IMPORTS_SYMBOL--> Symbol
TypeAlias    --ALIASES_TYPE--> Type
Type         --TYPE_ARGUMENT--> Type
Function     --RETURNS_TYPE--> Type
Parameter    --HAS_TYPE--> Type
Symbol       --SYNTHESIZES--> SyntheticSymbol
```

---

## 19.5 Provenance properties

Every semantic edge should be able to answer:

```text
Who asserted this?
On what source?
At what semantic generation?
With what confidence?
```

Suggested:

```rust
SemanticProvenance {
    engine: "pyrefly",
    engine_version: "1.2.0",
    backend: "query" | "tsp" | "lsp",
    semantic_generation: u64,
    content_digest: Digest,
    resolution: "exact" | "union" | "heuristic" | "unresolved",
}
```

---

## 19.6 Diagnostics as graph context

Type errors matter to confidence.

Example:

```text
File --HAS_DIAGNOSTIC--> Diagnostic
Diagnostic --AFFECTS_RANGE--> Expression
```

A semantic edge derived from an error region can carry:

```text
degraded = true
```

Do not globally discard all semantic facts from a file with one type error.

---

# 20) Recommended fully Rust Python CPG architecture

## 20.0 Production topology

```text
┌──────────────────────────────────────────────────────────────┐
│                     Rust CPG service                         │
│                                                              │
│  filesystem watcher / gix                                   │
│             │                                                │
│             ▼                                                │
│       immutable source store                                 │
│          │             │                                     │
│          │             └──────────────┐                      │
│          ▼                            ▼                      │
│  Ruff frontend worker          Pyrefly semantic client       │
│  --------------------          -----------------------       │
│  parse/AST                      stable DTO protocol           │
│  source/ranges                         │                      │
│  trivia/index                          │ IPC                  │
│  CFG/structural facts                  ▼                      │
│          │                     ┌───────────────────────┐      │
│          │                     │ Pyrefly Rust sidecar │      │
│          │                     │-----------------------│      │
│          │                     │ pinned 1.2.0         │      │
│          │                     │ Query bulk APIs      │      │
│          │                     │ optional TSP/LSP     │      │
│          │                     │ Ruff components 0.0.6│      │
│          │                     └──────────┬────────────┘      │
│          │                                │ semantic DTOs     │
│          └──────────────┬─────────────────┘                   │
│                         ▼                                     │
│                  fact reconciler                              │
│             identity / confidence / epoch                     │
│                         │                                     │
│                         ▼                                     │
│                  atomic CPG writer                            │
│                         │                                     │
│                         ▼                                     │
│                 persistent graph/index                        │
└──────────────────────────────────────────────────────────────┘
```

Every component is Rust.

---

## 20.1 Why a sidecar is still “entirely Rust”

“Entirely Rust” should mean:

```text
no Python parser process
no Python type-checker process
no LibCST runtime
no Python interpreter required for core parsing/semantic analysis
```

A separate Rust process is still a Rust implementation.

The process boundary improves engineering quality.

---

## 20.2 Suggested crates

```text
python-source
  immutable snapshots, digests, file identities

python-ruff-frontend
  parser, AST extraction, trivia, CFG, syntax facts

python-semantic-protocol
  stable DTOs and request/response schema

python-pyrefly-client
  IPC client, generation tracking, retries

smartref-pyrefly-semantic
  pinned Pyrefly adapter

python-cpg-model
  durable graph schema

python-cpg-reconcile
  range/symbol/type identity mapping

python-cpg-store
  atomic persistence

python-cpg-service
  orchestration / queries
```

---

## 20.3 One sidecar per workspace vs shared sidecar

### Per workspace

```text
Pros:
  clean config/environment isolation
  easy lifecycle
  no cross-repo cache contamination
  simple failure isolation

Cons:
  memory overhead
```

Recommended for correctness-first deployment.

### Shared multi-workspace sidecar

Requires explicit separation of:

```text
State
ConfigFinder
search paths
semantic generations
file IDs
caches
```

Only implement after memory benchmarks justify it.

---

## 20.4 Sidecar lifecycle

```text
spawn
  -> initialize workspace
  -> load config/environment
  -> load files
  -> emit generation 1
  -> serve semantic queries
  -> receive change batches
  -> atomically advance generation
  -> shutdown
```

---

## 20.5 Crash behavior

Main CPG service should retain:

```text
last_good_semantic_generation
last_good_facts
```

If sidecar crashes:

```text
syntax graph continues updating
semantic facts marked stale
sidecar restarted
semantic reconciliation catches up
```

Do not take the entire code-intelligence service down.

---

# 21) Stable semantic-sidecar protocol design

## 21.0 Design goal

The sidecar protocol should be **much more stable than Pyrefly**.

Version it independently:

```text
SmartRefPythonSemanticProtocol v1
```

---

## 21.1 Initialization

Request:

```json
{
  "method": "workspace/initialize",
  "params": {
    "workspaceId": "repo-abc",
    "rootUri": "file:///repo",
    "configPath": null,
    "expectedPyreflyVersion": "1.2.0",
    "protocolVersion": 1
  }
}
```

Response:

```json
{
  "protocolVersion": 1,
  "pyreflyVersion": "1.2.0",
  "workspaceGeneration": 0,
  "capabilities": {
    "queryTypeTable": true,
    "filteredTypeWalk": true,
    "bulkCallees": true,
    "classAttributes": true,
    "subtypeQuery": true,
    "gleanBulkIndex": true,
    "tspPassthrough": true
  }
}
```

---

## 21.2 Load files

```json
{
  "method": "workspace/load",
  "params": {
    "files": [
      {
        "module": "pkg.a",
        "uri": "file:///repo/pkg/a.py",
        "digest": "..."
      }
    ]
  }
}
```

Sidecar can read from disk, or for stronger snapshot consistency the main service can provide memory content through a controlled shared/source interface.

---

## 21.3 File fact batch

Best endpoint for CPG:

```json
{
  "method": "semantic/fileFacts",
  "params": {
    "module": "pkg.a",
    "uri": "file:///repo/pkg/a.py",
    "digest": "...",
    "include": [
      "types",
      "callees",
      "attributes",
      "diagnostics"
    ]
  }
}
```

Response:

```json
{
  "generation": 42,
  "digest": "...",
  "types": [],
  "callees": [],
  "classes": [],
  "diagnostics": []
}
```

One request should reuse one consistent Pyrefly transaction if your adapter can do so.

---

## 21.4 Located type DTO

```rust
pub struct LocatedTypeFact {
    pub range: ByteRange,
    pub type_: CanonicalType,
    pub role: TypeRole,
    pub precision: TypePrecision,
}
```

Role:

```text
computed
declared
expected
```

Query bulk extraction usually yields computed occurrence types.

TSP can fill declared/expected roles.

---

## 21.5 Callee DTO

```rust
pub struct CalleeFact {
    pub call_range: ByteRange,
    pub callee_range: ByteRange,
    pub targets: Vec<CalleeTarget>,
}

pub struct CalleeTarget {
    pub qualified_name: String,
    pub class_name: Option<String>,
    pub kind: CalleeKind,
    pub confidence: ResolutionConfidence,
}
```

---

## 21.6 Class DTO

```rust
pub struct ClassSemanticFacts {
    pub qname: String,
    pub declaration_range: ByteRange,
    pub members: Vec<MemberFact>,
}
```

---

## 21.7 Diagnostics DTO

Do not transport rendered text only.

Prefer:

```rust
DiagnosticFact {
    range,
    code,
    message,
    severity,
}
```

If upstream Query only gives rendered diagnostic text in the initial adapter, record that limitation and improve the sidecar later.

---

## 21.8 Change request

```json
{
  "method": "workspace/applyChanges",
  "params": {
    "baseGeneration": 41,
    "changes": [
      {
        "uri": "file:///repo/pkg/a.py",
        "module": "pkg.a",
        "kind": "modified",
        "oldDigest": "...",
        "newDigest": "..."
      }
    ]
  }
}
```

Reject stale base generations rather than silently racing.

---

## 21.9 Semantic generation

Response:

```json
{
  "generation": 42,
  "affectedModules": [
    "pkg.a",
    "pkg.consumer"
  ]
}
```

This affected-module list may require a small pinned-source extension because upstream Query does not promise it as a public return value.

---

## 21.10 Encoding choice

### JSON

Best for:

- development;
- debuggability;
- schema evolution;
- easy fixtures.

### MessagePack/CBOR

Good intermediate binary DTO.

### Cap'n Proto / Protobuf

Useful for:

- very large bulk fact batches;
- strict schema evolution;
- cross-language future use.

Pyrefly already uses Cap'n Proto elsewhere, but that does not require your protocol to do so.

Start with JSON unless profiling shows serialization is significant.

---

# 22) Full-repository bootstrap/indexing workflow

## 22.0 Phase overview

```text
discover
  -> identify modules
  -> parse syntax
  -> load Pyrefly
  -> bulk semantic extract
  -> reconcile
  -> persist
  -> validate coverage
```

---

## 22.1 Discover files

Use Rust filesystem/Git index:

```text
.py
.pyi
selected .ipynb strategy
```

Apply:

- ignore rules;
- generated-code policy;
- repository config;
- virtualenv/site-package boundaries.

---

## 22.2 Compute immutable source digests

For each file:

```text
BLAKE3/SHA-like digest
```

Every downstream fact batch references it.

---

## 22.3 Build syntax graph

Ruff frontend emits:

```text
file/module syntax
declarations
references/occurrences
calls
class bases
attributes
CFG
structural def-use
comments/trivia as needed
range index
```

This stage must succeed as far as possible even if Pyrefly later fails.

---

## 22.4 Resolve module identities

Do not guess all dotted names from filesystem layout.

Use project configuration plus Pyrefly module resolution where possible.

Persist:

```text
file -> module mapping
```

---

## 22.5 Bulk load Pyrefly

Batch module/path pairs through Query.

Suggested chunking:

```text
hundreds or low thousands per load batch
```

Benchmark repository shape; avoid one IPC request per file for initialization if your protocol can carry arrays.

---

## 22.6 Query semantic facts

For each source module retained at `Everything`:

```text
type table
callees
class attributes
diagnostics
```

Optionally add:

```text
Glean declaration/xref/search facts
```

Use `walker = None` for full occurrence typing or a recorded `TypeQueryStmtWalker` policy for filtered extraction.

The sidecar should decode local type indices, intern normalized types, and convert `PythonASTRange` to canonical byte ranges before the batch reaches persistent storage.

Optionally only keep `Everything` for first-party source modules and `Exports` for dependencies.

---

## 22.7 Reconcile facts

Use range indexes + qualified names.

Reconciliation sequence:

```text
types:
  located range -> expression occurrence

callees:
  call range -> call node
  target qname -> symbol/external symbol

members:
  class qname -> class node
  member name -> declaration/member node

diagnostics:
  range -> source node(s)
```

---

## 22.8 Persist atomically

Commit per file or coherent batch only when:

```text
source_digest matches
Ruff extraction succeeded/has known partial state
Pyrefly semantic generation is current
fact reconciliation completed
```

---

## 22.9 Coverage metrics

Track:

```text
files discovered
files parsed
files semantically loaded
expressions
expressions with type
calls
calls with >=1 resolved target
references
references with definition
classes
classes with member facts
imports
imports resolved
diagnostics
unresolved/dynamic calls
```

A CPG without coverage metrics cannot honestly claim robustness.

---

# 23) Incremental file-change workflow

## 23.0 Change pipeline

```text
watcher event
  │
  ▼
read new content
  │
  ▼
new immutable digest
  │
  ├─────────► Ruff changed-file extraction
  │
  └─────────► Pyrefly change_files / daemon change batch
                      │
                      ▼
              dependency invalidation
                      │
                      ▼
              affected semantic modules
                      │
           ┌──────────┴───────────┐
           ▼                      ▼
    changed file facts      importer facts
           │                      │
           └──────────┬───────────┘
                      ▼
                 graph delta
                      │
                      ▼
                  atomic commit
```

---

## 23.1 Separate structural and semantic invalidation

A file edit may:

### Structural only

```python
x = foo(
    1,
)
```

to reformatted equivalent.

Syntax ranges change; semantic dependency may not.

### Local semantic

```python
x = 1
```

to:

```python
x = "s"
```

local types/calls may change.

### Export semantic

```python
def f() -> int:
```

to:

```python
def f() -> str:
```

importers may change.

### Module topology

```python
from a import X
```

to:

```python
from b import X
```

module dependency graph changes.

Use different invalidation scopes.

---

## 23.2 Stable syntax IDs

Byte offsets shift after edits.

Prefer node identity matching based on:

```text
syntax kind
qualified/container path
local structural fingerprint
relative ordinal
old/new range proximity
```

Then update ranges without deleting/recreating every graph node.

---

## 23.3 Semantic edge replacement

For a changed source occurrence:

```text
old HAS_TYPE edge -> remove
new HAS_TYPE edge -> insert
```

Type nodes themselves may remain interned.

For calls:

```text
replace all semantic target edges for affected call site atomically
```

Do not incrementally “add new targets” without removing stale old targets.

---

## 23.4 Importer refresh

Let Pyrefly's semantic invalidation guide importer refresh.

Fallback when no affected-module API exists:

```text
query known reverse importers from CPG
refresh those conservatively
```

This is safe but less efficient.

---

## 23.5 Semantic snapshots and UI readers

Readers should never observe:

```text
new syntax + half-old semantic edges
```

Use MVCC/transaction/generation tagging.

Query policy:

```text
read latest complete graph generation
```

or:

```text
read syntax latest, semantic_status=updating
```

but never pretend mixed data is complete.

---

# 24) Coordinate/range synchronization across Ruff and Pyrefly

## 24.0 Canonical coordinate choice

Use **UTF-8 byte offsets** inside the CPG.

Reasons:

- Ruff uses `TextSize`/`TextRange` byte-oriented coordinates;
- source slicing is trivial;
- compact;
- deterministic;
- no repeated Unicode conversion for internal processing.

At protocol boundaries, support LSP line/character positions.

---

## 24.1 Canonical range DTO

```rust
pub struct ByteRange {
    pub start: u32,
    pub end: u32,
}
```

Invariant:

```text
0 <= start <= end <= source.len_bytes
```

Files >4 GiB should be rejected or handled by an explicit wider coordinate scheme because many Ruff/Pyrefly source types assume 32-bit text sizes.

---

## 24.2 TSP/LSP conversion

TSP/LSP positions use:

```text
line
character offset
position encoding semantics
```

Create one `LineIndex` per source snapshot.

Conversion must use the negotiated position encoding.

Never:

```rust
byte = line_start + character_count
```

for arbitrary Unicode.

---

## 24.3 Query `PythonASTRange`

Convert Query's `PythonASTRange` inside the sidecar using the same analyzed source.

The Query implementation itself has helpers for mapping expression ranges through the module's lined buffer.

Prefer sidecar output:

```text
ByteRange
```

so the main CPG does not need Pyrefly range types.

---

## 24.4 Range type/kind matching

When matching a semantic range to Ruff nodes, include expected semantic role.

Example:

```text
callee range:
  match Call.func / Name / Attribute

type range:
  match expression occurrence

definition range:
  match symbol declaration/name
```

Generic “smallest containing AST node” is insufficient.

---

## 24.5 Source digest assertion

Every range batch must include the digest.

Before applying:

```rust
assert_eq!(batch.content_digest, syntax_snapshot.content_digest);
```

If mismatch:

```text
discard semantic batch
request current generation
```

Never attempt fuzzy range matching across different file contents.

---

# 25) Semantic identity, provenance, confidence, and stale-fact prevention

## 25.0 Semantic facts are assertions, not timeless truths

A type edge is true relative to:

```text
source snapshot
project configuration
Python environment
typeshed/stubs
Pyrefly version
```

Store enough provenance to reproduce it.

---

## 25.1 Workspace semantic environment ID

Hash:

```text
pyrefly version
Pyrefly source SHA
config
Python version
search paths
stub/type environment
build-system module map
feature/preset settings
```

Call this:

```text
semantic_environment_id
```

---

## 25.2 Confidence levels

Suggested:

```text
Exact
UnionResolved
TypeInferred
StubResolved
Synthesized
Conservative
DynamicUnresolved
DegradedByError
```

A fact can have both type precision and resolution confidence.

---

## 25.3 Provenance ranking

When multiple sources provide the same category:

```text
Pyrefly direct semantic resolution
  >
Pyrefly union/static candidate
  >
Ruff semantic-qualified-name heuristic
  >
syntax-only name candidate
```

Do not silently overwrite; preserve provenance or resolve according to policy.

---

## 25.4 Conflict example

Ruff lexical inference:

```text
foo.bar likely attribute `bar`
```

Pyrefly:

```text
foo is Any -> target unresolved
```

Correct graph:

```text
syntax attribute exists
receiver type = Any
resolved symbol = none
heuristic member name = "bar"
```

Not:

```text
bar resolves to whichever repository method is named bar
```

---

## 25.5 Stale semantic state

File semantic status:

```text
current
stale
pending
failed
not_applicable
```

When source changes before Pyrefly finishes:

```text
mark old semantic generation stale
retain for fallback if desired
never label as current
```

---

## 25.6 Agent checklist

```text
[ ] Canonical coordinates are UTF-8 bytes.
[ ] Every semantic batch carries content digest.
[ ] Every semantic fact carries engine/version/generation provenance.
[ ] TSP/LSP positions are converted with explicit encoding.
[ ] Types attach only to expression-role-compatible ranges.
[ ] Callees attach only to call/callee ranges.
[ ] Old semantic edges are replaced atomically.
[ ] Unresolved facts remain explicit.
[ ] Environment/config changes invalidate semantic generations.
```

---

# 26) Imports, modules, exports, re-exports, and package identity

## 26.0 Import syntax and import semantics are different facts

Ruff syntax:

```python
from pkg.sub import C as Alias
```

tells you:

```text
source module text = pkg.sub
imported name = C
local binding = Alias
```

Pyrefly resolves:

```text
which pkg.sub?
which C export?
source or stub?
namespace package?
re-export?
implicit submodule?
```

The CPG needs both.

---

## 26.1 Import graph schema

```text
ImportNode
  --IMPORTS_MODULE--> ModuleNode
  --IMPORTS_SYMBOL--> SymbolNode
  --BINDS_LOCAL_NAME--> Symbol/Binding
```

Properties:

```text
relative_level
original_module_text
original_name
local_alias
is_star
resolution_status
```

---

## 26.2 Star imports

Pyrefly's export stage explicitly accounts for transitive `import *`.

Avoid syntax-only expansion.

Graph:

```text
ImportStarNode --IMPORTS_WILDCARD_FROM--> Module
```

Optionally materialize resolved symbol edges when Pyrefly has a stable export set.

---

## 26.3 Re-exports

Example:

```python
# pkg/__init__.py
from .model import User
```

A consumer imports:

```python
from pkg import User
```

Preserve:

```text
pkg.User is exported/re-exported identity
origin may be pkg.model.User
```

Useful edges:

```text
ExportedSymbol --REEXPORTS--> OriginSymbol
```

Avoid canonicalizing all user-facing qnames immediately to origin; both API surface and source origin matter.

---

## 26.4 Module resolution through TSP

TSP provides a `resolveImport` request surface.

Use it when:

- building import graph from a protocol-only backend;
- validating environment path resolution;
- resolving dynamic module path metadata supplied outside Query.

Cache by semantic snapshot/environment.

---

## 26.5 Namespace packages

Do not require every module to have one physical `__init__.py`.

Module node can represent:

```text
namespace package
regular package
source module
stub module
extension module
bundled module
```

---

## 26.6 `.py` vs `.pyi`

When both exist:

```text
runtime implementation source
typing interface source
```

may differ.

Recommended graph:

```text
RuntimeSymbol
  --TYPED_BY_STUB--> StubSymbol
```

or at least preserve source-kind provenance on definition/type facts.

---

## 26.7 Third-party modules

Create external nodes instead of dropping:

```text
ExternalModule("pandas")
ExternalSymbol("pandas.DataFrame")
```

This supports:

- call graph boundary analysis;
- dependency inventory;
- usage analytics;
- agent context.

---

# 27) Inheritance, MRO, protocols, generics, and dispatch

## 27.0 Base syntax

Ruff emits:

```python
class C(B, Mixin):
```

as base expressions.

Resolve each base through Pyrefly/type/module semantics.

Graph:

```text
C --EXTENDS--> B
C --EXTENDS--> Mixin
```

Store source order.

---

## 27.1 MRO-aware dispatch

Pyrefly's Query callee resolution uses solver/MRO operations in several cases.

This is superior to:

```text
receiver class has method name -> choose it
```

because inherited methods, synthesized fields, and `__call__` matter.

---

## 27.2 Persist MRO?

Two options.

### Materialize

```text
C --MRO_NEXT--> B
B --MRO_NEXT--> A
```

Useful for fast graph queries.

### Compute on demand

Persist direct bases, use Pyrefly subtype/member resolution for semantic questions.

Recommendation:

```text
persist direct resolved bases
optionally cache MRO as derived fact
```

MRO is environment/semantic-engine dependent and should carry provenance.

---

## 27.3 Protocols

Structural protocol satisfaction does not imply direct inheritance.

Therefore:

```text
EXTENDS
```

must not be overloaded to mean:

```text
is subtype of
```

Use query-time `is_subtype` or a separate derived:

```text
SATISFIES_PROTOCOL
```

only when needed.

---

## 27.4 Generic classes

Preserve specialization:

```python
C[int]
C[str]
```

as two types referencing the same class declaration.

Graph:

```text
Type(C[int]) --INSTANCE_OF_GENERIC--> Class(C)
Type(C[int]) --TYPE_ARGUMENT[0]--> int
```

---

## 27.5 Generic callables

Do not treat generic function target identity as a different function per specialization unless your graph explicitly models instantiations.

Recommended:

```text
Call --CALLS--> generic FunctionNode
Call --INFERRED_SIGNATURE--> specialized CallableType
```

---

## 27.6 Overloads

One source symbol may have multiple overload signatures.

Graph options:

```text
FunctionSymbol
  --HAS_OVERLOAD--> Signature1
  --HAS_OVERLOAD--> Signature2
```

At a call site:

```text
Call --SELECTED/COMPATIBLE_SIGNATURE--> Signature
```

The current Query `Callee` API focuses on callable target identity rather than exposing a selected overload object. Use TSP structured function types if overload signature detail is required.

---

## 27.7 Constructors

A class call:

```python
C(...)
```

can semantically involve:

```text
C.__new__
C.__init__
```

Query has logic to find `__init__` or `__new__`.

Recommended CPG:

```text
Call --CONSTRUCTS--> Class C
Call --CALLS_CONSTRUCTOR--> C.__init__ / C.__new__
```

Keep class construction distinct from ordinary function call when possible.

---

## 27.8 Callable objects

```python
obj()
```

may dispatch to:

```text
type(obj).__call__
```

Query resolves callable class types via MRO where possible.

Store dispatch kind:

```text
callable_object
```

---

# 28) Dynamic Python and uncertainty modeling

## 28.0 `Any`

If receiver is `Any`:

```python
x: Any
x.foo()
```

do not create a fabricated `foo` target **unless flow evidence has narrowed the receiver away from `Any`**.

Represent:

```text
receiver_type = Any
call_resolution = dynamic_unresolved
member_name = foo
```

In 1.2.0, `isinstance(x, C)` can narrow a value whose prior type was `Any` to `C`. Inside that narrowed branch, treat Pyrefly's concrete computed type as semantic evidence and allow normal member/callee resolution; preserve the narrowing provenance because the original declaration may still be `Any`.

---

## 28.1 `getattr`

Static-name case:

```python
getattr(x, "foo")()
```

may be conservatively connected to a member named `foo` if receiver type is known.

Dynamic-name case:

```python
getattr(x, name)()
```

must remain dynamic unless a literal/flow type narrows `name`.

---

## 28.2 `setattr` / monkey patching

Example:

```python
C.foo = other
```

Syntax graph should represent member assignment.

A static type checker may not model all runtime mutation semantics.

Mark classes/modules with dynamic-mutation facts when appropriate:

```text
HAS_DYNAMIC_MEMBER_MUTATION
```

Impact analysis can widen candidate sets.

---

## 28.3 `__getattr__` / `__getattribute__`

Attribute lookup can be synthesized dynamically.

If Pyrefly returns a type but no concrete declaration:

```text
HAS_TYPE = useful
REFERS_TO = unresolved/synthetic
```

Do not require every typed attribute to have a physical declaration.

---

## 28.4 Descriptors

Descriptor access changes the effective type of:

```python
obj.field
```

relative to the raw class attribute object.

Prefer Pyrefly use-site computed type.

Structural declaration graph still records the descriptor object/method.

---

## 28.5 Decorators

Decorators can replace declarations:

```python
@decorator
def f(...): ...
```

Store:

```text
source function declaration
decorator expressions
post-decoration semantic callable type
```

Do not assume source function signature equals runtime callable signature.

---

## 28.6 Dataclasses and framework synthesis

Generated:

```text
__init__
__eq__
fields
properties
validators
```

may appear as synthesized semantic declarations.

Represent synthesized nodes with:

```text
origin = synthesized
generator/decorator = dataclass/framework if knowable
```

---

## 28.7 Metaclasses

Class construction and members can be altered by metaclasses.

Persist metaclass syntax/resolution where available.

Use uncertainty for member/call edges not statically resolved.

---

## 28.8 `eval` / `exec`

Mark explicit dynamic-code execution nodes.

```text
DynamicExecutionNode
```

If string literal code is available, an optional nested parser can parse it, but do not merge it into ordinary file semantics without separate provenance.

---

## 28.9 Dynamic imports

Examples:

```python
importlib.import_module(name)
__import__(name)
```

Literal module name:

```text
possible resolved import
```

non-literal:

```text
dynamic import
```

---

## 28.10 Native extensions

`.pyd`, extension modules, compiled packages may have stubs but no Python implementation AST.

Graph external/stub symbol edges and mark implementation unavailable.

---

# 29) Error tolerance and partially invalid repositories

## 29.0 Continuous code intelligence sees broken code

During editing:

```python
def f(
```

is temporarily invalid.

The CPG cannot require every file to type-check before updating.

---

## 29.1 Two-layer availability

```text
syntax layer:
  tolerant Ruff/Tree-sitter policy
  may retain previous facts for damaged region

semantic layer:
  Pyrefly last-good generation
  current generation may fail/partial
```

---

## 29.2 Parse errors

If Ruff or Pyrefly parser rejects a file:

```text
do not delete all historical semantic facts immediately
```

Recommended:

```text
current syntax facts where recoverable
last-good semantic facts marked stale
diagnostic attached
```

When file becomes valid, replace stale facts.

---

## 29.3 Type errors

Pyrefly is designed to continue type checking code with type errors.

Keep semantic facts that are still produced.

Add:

```text
degraded_by_diagnostics = true
```

for facts in/near affected regions if necessary.

---

## 29.4 Sidecar panic

Because the experimental Query code contains assumptions/panics:

```text
supervisor detects exit
marks semantic service unhealthy
preserves last-good graph
restarts sidecar
reloads workspace
```

Do not let one exotic type crash the CPG service.

---

## 29.5 Timeouts

Large pathological modules or environments can be slow.

Use:

```text
request timeout
workspace bootstrap timeout
memory limit
sidecar restart policy
```

If one file repeatedly fails, quarantine semantic extraction for that file while retaining syntax graph.

---

## 29.6 Unsupported feature status

Per file:

```text
semantic_status:
  current
  stale
  parse_failed
  typecheck_failed
  backend_failed
  unsupported
```

Expose this to LLM coding agents so they can calibrate trust.

---

# 30) Performance, batching, memory, thread pools, and caching

## 30.0 Performance model

Costs:

```text
project/config resolution
module parsing
export computation
binding construction
type solving
cross-module invalidation
bulk type traversal
callee traversal
serialization
CPG reconciliation
persistence
```

Measure each separately.

---

## 30.1 One long-lived Query

Bad:

```text
new Query
add one file
query
drop Query
repeat
```

Good:

```text
one Query/State per workspace
load repository
reuse solved state
apply change events
bulk query affected files
```

---

## 30.2 Batch IPC

Prefer:

```text
semantic/fileFacts for 50 files
```

or:

```text
semantic/fileFactsBatch
```

over 50 process round trips when responses remain manageable.

---

## 30.3 Query caches and 1.2.0 type-table caching

The Query object still maintains a type-string → internal `Type` cache for operations such as `is_subtype` that parse/resolve textual type expressions. It is cleared on file changes.

The 1.2.0 whole-file type path is different:

```text
per response:
  TypeTableBuilder deduplicates normalized shapes

per table entry:
  structural xxh64 hash

client opportunity:
  cross-file/cross-request decoded-shape cache
```

Recommended cache hierarchy:

```text
Pyrefly internal cache:
  checker-owned

sidecar decoded type cache:
  keyed by structural hash + verified shape

CPG type interning:
  application-owned durable canonical identity
```

Do not conflate these layers.

---

## 30.4 Memory retention levels

`Require::Everything` retains:

```text
AST
bindings
answers
answer trace
```

for target files.

On very large repositories, this can be memory expensive.

Strategies:

```text
first-party active workspace:
  Everything

cold/dependency modules:
  Exports / Indexing as needed

sharded repository:
  one sidecar per project/package partition
```

But sharding must preserve import semantics.

---

## 30.5 Workspace indexing limit

TSP/LSP has a workspace indexing limit.

For CPG use, configure explicitly based on repository size.

Do not accept default 2000 and then assume reference/navigation completeness in a 50k-file monorepo.

---

## 30.6 Thread pools

Pyrefly uses parallelism internally.

Tune:

```text
Pyrefly thread count
number of concurrent sidecars
CPG parser threads
database writer concurrency
```

as one system.

Oversubscription pattern to avoid:

```text
32 Ruff workers
x 32 Pyrefly threads
x 8 repositories
```

on a 32-core machine.

---

## 30.7 CPU affinity / workload classes

Optional production design:

```text
interactive workspace sidecar:
  fewer threads
  low latency

bulk indexing sidecar:
  more threads
  throughput optimized
```

---

## 30.8 Source serialization

If sidecar reads files from shared disk:

```text
zero source payload IPC
```

but source race is possible.

If main process sends content:

```text
perfect snapshot consistency
more IPC bandwidth
```

Hybrid:

```text
send digest + path
sidecar reads file
sidecar verifies digest
if mismatch -> reject/retry
```

---


## 30.9 Pyrefly 1.2.0 performance changes relevant to CPG services

The 1.2.0 release includes several upstream optimizations worth reflecting in capacity plans:

- working-directory/project resolution caching;
- unconditional directory-entry caching and cached namespace-package probes;
- split typeshed archives so stdlib loading can avoid eagerly constructing third-party indexes;
- load-map indices for stub paths to reduce retained heap;
- interpreter discovery caching;
- smaller calculation cells;
- more lock-free computed-answer reads.

The release notes report meaningful improvements on upstream benchmarks, but **benchmark your own repository shapes**. CPG workloads differ from CLI checking because they retain `Everything`, run bulk Query extraction and serialize semantic fact batches.

The new type-table path itself is also a serialization/per-occurrence optimization: it avoids repeated display strings and repeated structural type trees.

---

## 30.10 Incremental benchmarks


Measure:

```text
cold load
warm no-op check
local non-export edit
export type edit
base-class edit
widely imported module edit
typeshed/config edit
1 file / 10 files / 100 files change burst
```

For each:

```text
Pyrefly invalidation time
recheck time
semantic extraction time
serialized bytes
CPG reconciliation time
database commit time
```

---

# 31) Persistence and transactional CPG updates

## 31.0 Separate syntax and semantic tables/partitions

Recommended logical schema:

```text
syntax_nodes
syntax_edges
semantic_type_facts
semantic_call_edges
semantic_reference_edges
semantic_member_facts
diagnostics
type_nodes
generations
```

This makes semantic refresh independent of syntax topology.

---

## 31.1 Generation table

```text
WorkspaceGeneration:
  workspace_id
  source_generation
  semantic_generation
  pyrefly_version
  semantic_environment_id
  status
  committed_at
```

---

## 31.2 File fact replacement

For each affected file in transaction:

```text
delete/expire old semantic facts for file
insert new type facts
insert new call facts
insert new reference/member facts
update file semantic generation
commit
```

Do not mutate edges piecemeal without a completion marker.

---

## 31.3 Shared target nodes

Call edges may target functions in unaffected files.

Do not delete target symbol nodes when replacing caller semantic facts.

Ownership:

```text
symbol node owned by declaration file
call edge owned by call-site file
```

---

## 31.4 External symbol lifecycle

Key external symbols by:

```text
semantic_environment_id
qualified_name
declaration URI if available
```

A dependency version change may change symbol identity/definition.

---

## 31.5 History

If your CPG includes Git history:

```text
current semantic graph
```

and:

```text
historical source graph
```

need not both be fully type-checked continuously.

Recommended:

```text
semantic enrichment for HEAD/current workspace
on-demand semantic enrichment for historical revisions
```

unless historical semantic queries are a core product.

---

# 32) Testing and correctness strategy

## 32.0 Test layers

```text
unit
adapter fixture
protocol
semantic golden
incremental
cross-engine reconciliation
performance
upgrade
fault injection
```

---

## 32.1 Type fixture

```python
def f(x: int | None) -> int:
    if x is None:
        return 0
    return x + 1
```

Assert:

```text
parameter declaration type
branch-refined x type
return expression type
operator result type
```

---

## 32.2 Call fixture

Cover:

```python
def f(): ...

class A:
    def m(self): ...

class B(A):
    pass

f()
B().m()
```

Assert:

```text
direct function
constructor
inherited method
```

---

## 32.3 Union dispatch fixture

```python
x: A | B
x.run()
```

Assert both targets when appropriate.

---

## 32.4 Decorator fixture

```python
@decorator
def f(): ...
```

Assert:

```text
decorator call fact
decorated function type
source declaration identity
```

---

## 32.5 Property fixture

```python
class C:
    @property
    def x(self) -> int:
        return 1
```

Assert member/property semantics and use-site type.

---

## 32.6 Generic fixture

Cover:

```python
class Box[T]:
    value: T

b: Box[int]
```

Assert canonical type args.

---

## 32.7 Stub fixture

Create `.py` + `.pyi` / external stub combination.

Assert:

```text
runtime symbol
typing definition
type provenance
```

---

## 32.8 Incremental local change

Change:

```python
x = 1
```

to:

```python
x = "s"
```

Assert only expected semantic facts change.

---

## 32.9 Incremental exported change

Module A:

```python
def f() -> int: ...
```

to:

```python
def f() -> str: ...
```

Module B imports/calls A.f.

Assert importer semantics update without full-repo wipe.

---

## 32.10 Configuration change

Change Python version/search path/stub package.

Assert semantic environment ID changes and affected semantic facts are regenerated.

---

## 32.11 Unicode range fixture

Use:

```python
π = "😀"
```

Assert byte ↔ LSP/TSP position mapping.

---

## 32.12 Invalid-source fixture

Introduce syntax error, then fix.

Assert:

```text
no destructive loss of last-good semantic graph
stale status
recovery
```

---


## 32.13 Type-table fixture

Assert:

```text
located type indices are in bounds
structurally identical repeated types deduplicate
hash is stable for identical normalized shapes in multiple files
different normalized shapes never merge merely because hash matches
callable staticmethod flag is preserved
anonymous TypedDict normalizes to dict[str, value-union]
filtered walker emits only intended expression facts
```

Also measure:

```text
located_count / unique_type_table_entries
```

as a serialization-efficiency signal.

---

## 32.14 Golden output


Golden-test your own DTO, not Pyrefly debug strings.

Example:

```json
{
  "call": [120, 125],
  "targets": [
    {
      "qname": "pkg.C.m",
      "kind": "method"
    }
  ]
}
```

---

## 32.15 Differential tests

Useful comparisons:

```text
Query type-table type vs TSP computed type
Query callee target vs LSP definition at call target
Ruff declaration extraction vs Pyrefly document symbols
CPG reverse references vs LSP references
```

Differences should be classified, not automatically treated as failures.

---

# 33) Observability and diagnostics

## 33.0 Workspace metrics

```text
loaded modules
retained Everything modules
exports-only modules
semantic generation
recheck queue depth
last successful commit
sidecar RSS
thread count
```

---

## 33.1 Fact coverage

```text
typed_expression_ratio
resolved_call_ratio
resolved_import_ratio
resolved_reference_ratio
external_target_ratio
dynamic_call_ratio
degraded_fact_ratio
```

Break down by repository/package.

---

## 33.2 Latency

```text
initial load
incremental invalidation
Pyrefly solve
bulk type extract
bulk callee extract
IPC
reconcile
database commit
end-to-end watcher-to-queryable
```

Use percentiles.

---

## 33.3 Query timing API

Use `get_type_table_in_file_with_timing` or the filtered variant in diagnostic builds.

The 1.2.0 timing payload focuses on:

```text
setup
transform
total
located_count
```

Add application-side metrics for:

```text
unique type-table entries
located occurrences
dedup ratio
serialized bytes
structural-hash cache hits/misses
canonical type interning hits/misses
filtered-walker selected occurrence count
```

Do not expect the old 1.1.x `stringify/cache/shape profile` fields on this API.

---

## 33.4 Structured logs

Include:

```text
workspace
module
file
digest prefix
semantic generation
operation
duration
fact counts
Pyrefly version
error category
```

Do not log full source by default.

---

## 33.5 Health states

```text
healthy
loading
degraded
restarting
out_of_memory
config_error
protocol_mismatch
```

CPG API can expose semantic health to agents.

---

# 34) Production deployment and security

## 34.0 Trust model

A repository can contain:

- malicious file paths;
- huge files;
- pathological type graphs;
- configuration referencing external paths;
- build-system hooks;
- Python environment settings.

Static analysis is safer than executing code, but configuration discovery can still cross system boundaries.

---

## 34.1 Do not execute repository Python for semantic analysis

Core Pyrefly analysis does not require running application code.

Keep it that way.

If interpreter introspection is required:

```text
controlled interpreter
sandbox
timeout
restricted environment
no arbitrary project startup hooks
```

---

## 34.2 Filesystem sandbox

Restrict source access to:

```text
workspace root
approved site-package roots
bundled stubs
approved generated-source roots
```

Normalize symlinks/paths before access policy checks.

---

## 34.3 Sidecar resource limits

Use OS/container limits:

```text
memory
CPU
open files
process count
wall-clock timeout
```

A static analyzer can still be resource-exhausted.

---

## 34.4 IPC security

For local IPC:

```text
workspace-specific socket
filesystem permissions
randomized/private endpoint
no public bind
```

Validate:

```text
workspace ID
protocol version
message size
range bounds
path roots
```

---

## 34.5 Message limits

Reject:

```text
gigabyte JSON request
millions of requested paths
negative/overflow ranges
unknown protocol version
```

---

## 34.6 Dependency supply chain

Pin:

```text
Pyrefly tag + SHA
Cargo.lock
Ruff component versions/source used by Pyrefly
sidecar DTO protocol version
```

Audit updates.

---

# 35) Upgrade/version migration discipline

## 35.0 Why upgrades are special

Pyrefly's user-facing product can remain compatible while the Rust Query/library internals and semantic answers change substantially.

Therefore:

```text
Pyrefly minor upgrade
  = semantic-sidecar API migration
  + semantic graph regression event
```

1.2.0 demonstrates both classes at once: the bulk Query type API changed, and many inference/narrowing/framework behaviors improved.

---

## 35.1 Generic upgrade checklist

```text
[ ] Pin new Pyrefly tag/SHA.
[ ] Read release notes for behavior changes, not only API changes.
[ ] Diff pyrefly/lib/query.rs.
[ ] Diff pyrefly/lib/query/type_table.rs if present.
[ ] Diff pyrefly/lib/state/require.rs and state dependency tracking.
[ ] Diff generated TSP protocol/version enum.
[ ] Diff pyrefly_types Type variants.
[ ] Inspect Pyrefly Ruff dependency versions/source.
[ ] Inspect workspace crate additions/removals.
[ ] Compile semantic sidecar.
[ ] Run DTO/type-table golden fixtures.
[ ] Run incremental invalidation fixtures.
[ ] Compare type coverage.
[ ] Compare call resolution coverage.
[ ] Compare declaration/xref coverage if using Glean/LSP.
[ ] Compare memory/cold-load/recheck benchmarks.
[ ] Verify config/environment/build-system behavior.
[ ] Run large-repository shadow index.
[ ] Only then switch production.
```

---

## 35.2 Required 1.1.1 → 1.2.0 adapter migration

### Bulk type API

Remove assumptions around:

```text
get_types_in_file
get_type_shapes_in_file
TypeShape
TypeQueryTiming.stringify/cache/shape_profiles
```

Adopt:

```text
get_type_table_in_file(name, path, walker)
get_type_table_in_file_with_timing
get_type_table_in_file_with_timing_filtered
TypeTableResponseData
SerializedTypeTableEntry
LocatedTypeTableRef
IndexedTypeShapeKind
```

### Type identity

```text
old:
  occurrence carried recursive TypeShape

new:
  occurrence carries response-local type_index
  table entry carries normalized shape + structural hash
```

Your DTO decoder must resolve indices before durable persistence.

### Ruff coupling

```text
old 1.1.1:
  Pyrefly Ruff dependencies were pinned by Git revision

1.2.0:
  Pyrefly uses Ruff component crates 0.0.6 from crates.io
```

If the companion Ruff frontend remains on component crates 0.0.7, the sidecar boundary is still recommended.

### Semantic goldens

Explicitly review changes around:

```text
Any/isinstance narrowing
unannotated class attributes
attrs
functools.partial
singledispatch
TypedDict
pattern matching
overloads
Enum.value
lambda contextual typing
properties/descriptors
Pydantic/Django/factory-boy
PEP 561 partial stub packages
```

Do not “fix” these changes back to 1.1 behavior merely to preserve old CPG snapshots.

---

## 35.3 Protocol compatibility

Your own sidecar protocol should follow:

```text
additive minor fields
explicit protocol version
unknown-field tolerance where safe
capability negotiation
hard failure for incompatible major
```

Recommended 1.2 capability flags:

```text
query_type_table
query_filtered_type_walk
query_callees
query_attributes
query_subtype
query_qualified_target
tsp_0_4_1
glean_bulk_index
bazel_check_support
```

Do not force main CPG upgrades every time Pyrefly changes an internal enum.

---

## 35.4 Type normalization migrations

If Pyrefly changes normalized table shape or display:

```text
do not use display string as identity
do not use response-local index as identity
```

Canonical identity should be application structural identity.

The upstream structural hash is valuable as a cache key, but CPG migrations should be driven by your canonical schema version.

---

## 35.5 Semantic drift

An upgrade can improve type accuracy without API changes.

Track:

```text
resolved call count
type precision distribution
typed occurrence count
unique normalized type count
diagnostic changes
unresolved reference count
synthesized member count
```

A “better” checker can legitimately change graph topology substantially.

---

# 36) Global anti-pattern inventory

- Importing `pyrefly::query::Query` throughout the main CPG workspace.
- Treating the Query API or Glean conversion path as SemVer-stable.
- Calling a custom wrapper “the upstream Pyrefly query daemon” without clarifying it is application-owned.
- Passing Ruff AST objects from one Ruff component version into Pyrefly compiled against another.
- Persisting `pyrefly_types::Type` directly as graph storage schema.
- Persisting `Type::Var` solver placeholders as user types.
- Using type display strings or Query response-local `type_index` values as canonical type identity.
- Treating `Any` as high-confidence resolution.
- Dropping calls that have no resolved Pyrefly callee.
- Choosing one arbitrary target from a union dispatch.
- Assuming an LSP call hierarchy is the complete call graph.
- Issuing one TSP request per expression for cold repository indexing when the Query type-table bulk path is available.
- Recreating `Query`/`State` for every file.
- Loading every dependency module at `Require::Everything` without memory measurement.
- Accepting TSP's default workspace indexing limit without comparing it to repository size.
- Treating a file path as the complete Python module identity.
- Ignoring `.pyi`/typeshed targets because they lack runtime implementation source.
- Collapsing re-export identity into origin identity.
- Conflating direct inheritance with semantic subtyping/protocol compatibility.
- Assuming a property is an ordinary stored field.
- Treating synthesized declarations as nonexistent.
- Attaching semantic facts to syntax nodes from a different source digest.
- Converting LSP character positions to bytes by simple addition.
- Applying semantic edges piecemeal without a generation barrier.
- Deleting all semantic facts because one line has a type error.
- Deleting last-good semantics during temporary invalid syntax.
- Treating an empty callee result as proof no runtime target exists.
- Executing arbitrary repository Python to “help” type resolution.
- Sharing one global Pyrefly state across incompatible environments without strict isolation.
- Upgrading Pyrefly without checking its exact Ruff component versions/source.
- Upgrading sidecar and CPG protocol in one inseparable deployment.
- Storing no coverage metrics, then claiming semantic completeness.

---

# 37) LLM-agent implementation checklist

## 37.0 Architecture

```text
[ ] Keep Ruff syntax and Pyrefly semantics as separate layers.
[ ] Use a Rust semantic sidecar for Pyrefly unstable APIs.
[ ] Define an application-owned semantic DTO protocol.
[ ] Pin Pyrefly to exact tag/SHA.
[ ] Record Pyrefly's exact Ruff component dependency versions/source.
[ ] Use source digest + byte ranges as cross-engine join key.
```

## 37.1 Bootstrap

```text
[ ] Discover .py/.pyi files under governed roots.
[ ] Compute immutable content digests.
[ ] Parse/build structural CPG with Ruff.
[ ] Resolve file -> ModuleName/ModulePath mapping.
[ ] Create one long-lived Query/State per workspace.
[ ] Batch add files.
[ ] Bulk query type-table facts.
[ ] Bulk query callees.
[ ] Query class attributes as needed.
[ ] Reconcile targets to source/external symbols.
[ ] Commit graph atomically.
[ ] Record coverage metrics.
```

## 37.2 Types

```text
[ ] Prefer structured type-table/TSP types over strings.
[ ] Resolve response-local type indices, then intern canonical type nodes.
[ ] Preserve generic args.
[ ] Preserve callable params/return and `is_staticmethod`.
[ ] Preserve TypedDict/tuple traits.
[ ] Preserve type variable bounds/constraints where exposed.
[ ] Keep declared/computed/expected roles separate.
[ ] Represent Any/unknown precision.
[ ] Do not persist solver Var as a normal type.
```

## 37.3 Calls

```text
[ ] Create call nodes from Ruff syntax first.
[ ] Attach Query callees by exact source generation/range.
[ ] Preserve multiple targets.
[ ] Distinguish function/method/classmethod/staticmethod/constructor/callable-object.
[ ] Create external target nodes when source is absent.
[ ] Mark unresolved dynamic calls explicitly.
[ ] Do not fabricate targets by method-name search alone.
```

## 37.4 Symbols and members

```text
[ ] Keep source declaration identity CPG-owned.
[ ] Use Pyrefly to enrich member type/finality/property role.
[ ] Preserve synthesized semantic declarations separately.
[ ] Preserve re-export vs origin identity.
[ ] Preserve stub-vs-runtime provenance.
```

## 37.5 Incrementality

```text
[ ] Notify Pyrefly of changed files.
[ ] Advance semantic generation only after successful transaction.
[ ] Requery changed and semantically affected modules.
[ ] Replace stale semantic edges atomically.
[ ] Keep last-good semantics during temporary failure.
[ ] Mark stale/pending/failed state explicitly.
[ ] Treat config/environment change as semantic invalidation.
```

## 37.6 Protocol/ranges

```text
[ ] Canonical CPG range = UTF-8 byte range.
[ ] Convert Query PythonASTRange in the sidecar.
[ ] Convert TSP/LSP ranges with negotiated encoding.
[ ] Assert content digest before graph apply.
[ ] Version the semantic DTO protocol.
[ ] Reject incompatible protocol versions.
```

## 37.7 Robustness

```text
[ ] Supervise/restart sidecar.
[ ] Apply memory/CPU/time limits.
[ ] Quarantine pathological files without losing syntax graph.
[ ] Capture Pyrefly diagnostics.
[ ] Track semantic coverage ratios.
[ ] Run Unicode fixtures.
[ ] Run dynamic/Any fixtures.
[ ] Run large SCC/import-cycle fixtures.
```

## 37.8 Upgrades

```text
[ ] Diff Query internals.
[ ] Diff TSP schema.
[ ] Diff Type enum.
[ ] Diff Pyrefly Ruff component versions/source.
[ ] Run full semantic golden suite.
[ ] Run incremental suite.
[ ] Compare performance/coverage.
[ ] Shadow-index a real repository before rollout.
```

---

# 38) Reference links and source audit

## 38.0 Pyrefly core

- Repository: https://github.com/facebook/pyrefly
- `1.2.0` release: https://github.com/facebook/pyrefly/releases/tag/1.2.0
- `1.2.0` source: https://github.com/facebook/pyrefly/tree/1.2.0
- Architecture: https://github.com/facebook/pyrefly/blob/1.2.0/ARCHITECTURE.md
- Root workspace: https://github.com/facebook/pyrefly/blob/1.2.0/Cargo.toml
- Main crate manifest: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/Cargo.toml
- Library surface: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/lib/lib.rs
- CLI command enum: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/lib/commands/all.rs
- Bazel checker: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/lib/commands/bazel_check.rs

## 38.1 Query and semantic state

- Query API: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/lib/query.rs
- Query type table: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/lib/query/type_table.rs
- State implementation: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/lib/state/state.rs
- Require levels: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/lib/state/require.rs
- Epoch model: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/lib/state/epoch.rs

## 38.2 Type/model/build crates

- `pyrefly_python`: https://github.com/facebook/pyrefly/tree/1.2.0/crates/pyrefly_python
- `pyrefly_types`: https://github.com/facebook/pyrefly/tree/1.2.0/crates/pyrefly_types
- Internal Type enum: https://github.com/facebook/pyrefly/blob/1.2.0/crates/pyrefly_types/src/types.rs
- `pyrefly_graph`: https://github.com/facebook/pyrefly/tree/1.2.0/crates/pyrefly_graph
- `pyrefly_config`: https://github.com/facebook/pyrefly/tree/1.2.0/crates/pyrefly_config
- `pyrefly_bundled`: https://github.com/facebook/pyrefly/tree/1.2.0/crates/pyrefly_bundled
- `pyrefly_build`: https://github.com/facebook/pyrefly/tree/1.2.0/crates/pyrefly_build
- module resolver: https://github.com/facebook/pyrefly/blob/1.2.0/crates/pyrefly_build/src/module_resolver.rs

## 38.3 TSP/LSP

- TSP command implementation: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/lib/commands/tsp.rs
- `tsp_types`: https://github.com/facebook/pyrefly/tree/1.2.0/crates/tsp_types
- Generated TSP protocol: https://github.com/facebook/pyrefly/blob/1.2.0/crates/tsp_types/src/protocol.rs
- Pyrefly IDE docs: https://pyrefly.org/en/docs/IDE/
- Pyrefly configuration docs: https://pyrefly.org/en/docs/configuration/

## 38.4 Glean/index schemas

- `pyrefly_glean_schema`: https://github.com/facebook/pyrefly/tree/1.2.0/crates/pyrefly_glean_schema
- Glean fact envelope: https://github.com/facebook/pyrefly/blob/1.2.0/crates/pyrefly_glean_schema/src/report/glean/facts.rs
- Python schema: https://github.com/facebook/pyrefly/blob/1.2.0/crates/pyrefly_glean_schema/src/report/glean/schema/python.rs
- Python xref schema: https://github.com/facebook/pyrefly/blob/1.2.0/crates/pyrefly_glean_schema/src/report/glean/schema/python_xrefs.rs
- Internal Glean conversion/report entry: https://github.com/facebook/pyrefly/blob/1.2.0/pyrefly/lib/report/glean.rs

---

# Final implementation guidance for coding agents

When asked to implement Pyrefly-backed Python CPG semantics against Pyrefly 1.2.0, use this default architecture unless the repository explicitly specifies otherwise:

```text
1. Parse Python with the pinned Ruff frontend.
2. Build syntax nodes, ranges, CFG, structural def-use, imports, classes,
   functions, calls, attributes, and references from Ruff.
3. Do not pass Ruff AST objects across a 0.0.7 ↔ 0.0.6 crate-version boundary.
4. Send file/module identity + source digest to a Rust Pyrefly semantic sidecar.
5. The sidecar vendors exact Pyrefly 1.2.0 and owns pyrefly::query::Query.
6. Load first-party modules at Require::Everything.
7. Retrieve get_type_table_in_file for bulk expression types.
8. Decode response-local type indices and intern normalized shapes using
   application-owned type identity; use upstream structural hashes as cache hints.
9. Use a TypeQueryStmtWalker when the CPG only stores selected expression types.
10. Retrieve get_callees_with_location for bulk static call targets.
11. Retrieve get_attributes for class/member enrichment where needed.
12. Optionally adapt Pyrefly Glean facts for bulk declarations/xrefs/search facts.
13. Use TSP for structured declared/computed/expected per-position types,
    import resolution, declaration-rich type payloads, and snapshot semantics.
14. Use LSP for definition/reference/implementation/navigation fallbacks.
15. Convert all Pyrefly/TSP/LSP ranges to CPG UTF-8 byte ranges in the sidecar/client boundary.
16. Normalize every Pyrefly-derived fact into application-owned DTOs.
17. Attach semantic facts only when source digest and semantic generation match.
18. Persist multiple call targets; preserve unresolved dynamic calls.
19. Treat type errors as degraded information, not total graph failure.
20. Use Pyrefly's incremental invalidation to identify semantic refresh scope.
21. Replace affected semantic fact partitions atomically.
22. Keep last-good semantics marked stale while the sidecar recovers.
23. Treat 1.2 semantic improvements as intentional graph-semantic drift and update goldens deliberately.
24. Never expose Pyrefly internal Rust types, response-local indices, or version assumptions in the persistent graph schema.
```

The architectural objective is not to force one library to own all Python semantics. It is to make each layer authoritative for the facts it is best at:

```text
Ruff:
  source structure and lexical truth

Pyrefly Query:
  bulk static type/callee/member truth

Pyrefly Glean/LSP/TSP:
  index/navigation/structured-position semantic truth

CPG:
  durable identity, graph relationships, uncertainty, incrementality,
  history, provenance, and queryability
```

That decomposition gives a fully Rust Python code-intelligence pipeline without sacrificing robustness to the instability of any one library's internal API.

# Appendix A — Pinned-source Rust sidecar skeleton

> **Important:** this appendix is an implementation pattern for the exact `1.2.0` source tree. Pyrefly deliberately does not promise Rust API stability. Keep all imports and adaptations inside the sidecar crate and compile-test them against the pinned source.

## A.0 Manifest strategy

A vendored workspace can make internal companion crates explicit:

```toml
[package]
name = "smartref-pyrefly-semantic"
version = "0.1.0"
edition = "2024"

[dependencies]
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Prefer path dependencies into an exact checkout of facebook/pyrefly@1.2.0.
pyrefly = { path = "../pyrefly/pyrefly" }
pyrefly_python = { path = "../pyrefly/crates/pyrefly_python" }
pyrefly_util = { path = "../pyrefly/crates/pyrefly_util" }
```

Keep the checkout's `Cargo.lock`/dependency patch behavior intact. Pyrefly's root workspace has patched dependencies and internal path relationships; blindly extracting one package can change the resolved graph.

---

## A.1 Exact concepts used by the adapter

Pinned 1.2.0 source provides:

```rust
pyrefly::query::Query
pyrefly_python::module_name::ModuleName
pyrefly_python::module_path::ModulePath
pyrefly_util::thread_pool::ThreadCount
```

and the nested unstable library export surface includes `default_config_finder`.

Conceptual adapter initialization:

```rust
use pyrefly::query::Query;
use pyrefly::library::library::library::library::default_config_finder;
use pyrefly_util::thread_pool::ThreadCount;

pub struct SemanticEngine {
    query: Query,
}

impl SemanticEngine {
    pub fn new() -> Self {
        let config_finder = default_config_finder(None);
        let query = Query::new(config_finder, ThreadCount::AllThreads);
        Self { query }
    }
}
```

Because the nested `library::library::library::library` path exists specifically to discourage casual use and is documented as unstable, **wrap it once**. No other application crate should import it.

---

## A.2 Module/path construction

Pinned `ModulePath` constructors include:

```rust
ModulePath::filesystem(path: PathBuf)
ModulePath::memory(path: PathBuf)
ModulePath::namespace(path: PathBuf)
```

Example adapter domain:

```rust
#[derive(Clone)]
pub struct SourceModule {
    pub module_name: String,
    pub path: PathBuf,
    pub digest: String,
}
```

Convert at the boundary:

```rust
fn to_pyrefly_path(m: &SourceModule) -> ModulePath {
    ModulePath::filesystem(m.path.clone())
}
```

Use Pyrefly's module-name constructor supported by the pinned version inside one helper; do not scatter that construction through the codebase.

---

## A.3 Loading files

Conceptual:

```rust
pub fn load_modules(
    engine: &SemanticEngine,
    modules: &[SourceModule],
) -> Vec<String> {
    let inputs = modules
        .iter()
        .map(|m| {
            (
                to_module_name(&m.module_name),
                ModulePath::filesystem(m.path.clone()),
            )
        })
        .collect();

    engine.query.add_files(inputs)
}
```

The returned `Vec<String>` is rendered diagnostic output in Query 1.2.0. It is useful as an initial compatibility path, but the adapter should eventually expose structured diagnostics from lower-level state or a protocol surface.

---

## A.4 Bulk type-table adapter

The 1.2.0 adapter should consume the indexed table rather than an obsolete per-occurrence `TypeShape` vector.

Conceptual pattern:

```rust
pub fn file_type_table(
    engine: &SemanticEngine,
    module: &SourceModule,
) -> anyhow::Result<FileTypeTableDto> {
    let name = to_module_name(&module.module_name);
    let path = ModulePath::filesystem(module.path.clone());

    let response = engine
        .query
        .get_type_table_in_file(name, path, None)
        .ok_or_else(|| anyhow::anyhow!("Pyrefly has no retained semantic state"))?;

    decode_and_normalize_type_table(response, module)
}
```

Recommended decoder responsibilities:

```text
1. validate every LocatedTypeTableRef.type_index
2. convert PythonASTRange -> canonical UTF-8 byte range
3. decode IndexedTypeShapeKind references recursively/cycle-safely
4. preserve entry.hash as upstream_structural_hash
5. map/verify shape into application CanonicalType
6. intern by application-owned structural identity
7. emit located occurrence -> canonical TypeId facts
```

Do not return Pyrefly's response-local `type_index`, `PythonASTRange`, or `IndexedTypeShapeKind` directly through the long-lived CPG storage contract.

### Filtered variant

When a workspace policy stores only selected expression types:

```rust
let response = engine.query.get_type_table_in_file(
    name,
    path,
    Some(&walker),
);
```

Keep the walker implementation inside the pinned sidecar because its callback types are Ruff/Pyrefly-version-bound.

---

## A.5 Bulk call adapter

```rust
pub fn file_callees(
    engine: &SemanticEngine,
    module: &SourceModule,
) -> anyhow::Result<Vec<CallFactDto>> {
    let facts = engine
        .query
        .get_callees_with_location(
            to_module_name(&module.module_name),
            ModulePath::filesystem(module.path.clone()),
            None,
        )
        .ok_or_else(|| anyhow::anyhow!("No callee data"))?;

    group_callees_by_location(facts, module)
}
```

Group all targets for the same call/callee range before serialization.

Never return:

```text
one response row = one call node
```

unless multiple possible targets are aggregated.

---

## A.6 Class attributes

The upstream API requires a class name.

For each Ruff-declared top-level class you want to enrich:

```rust
let attrs = engine.query.get_attributes(
    module_name,
    module_path,
    class_name,
);
```

If nested classes are important, verify the pinned Query behavior rather than assuming a simple class-name string uniquely identifies them.

A stronger sidecar extension can identify classes by qualified name and declaration range.

---

## A.7 Subtype service

Do not send internal type objects across IPC.

Application endpoint:

```json
{
  "method": "semantic/isSubtype",
  "params": {
    "module": "pkg.context",
    "left": "pkg.Child[int]",
    "right": "pkg.Protocol"
  }
}
```

Adapter calls pinned `Query::is_subtype`.

Cache at:

```text
semantic generation + left canonical string + right canonical string
```

Do not keep subtype cache across semantic generation changes unless the sidecar can prove environment/type identities unchanged.

---

## A.8 Applying file changes

`Query::change_files` accepts Pyrefly `CategorizedEvents`.

Keep conversion isolated:

```rust
pub fn apply_change_batch(
    engine: &SemanticEngine,
    changes: &[FileChangeDto],
) -> anyhow::Result<()> {
    let events = to_pyrefly_events(changes)?;
    engine.query.change_files(&events);
    Ok(())
}
```

Then increment your application generation only after the call succeeds.

---

## A.9 Thread count

Pinned `ThreadCount` is:

```rust
pub enum ThreadCount {
    AllThreads,
    NumThreads(NonZeroUsize),
}
```

`AllThreads` caps Pyrefly's created thread pool at the host's available parallelism, with a maximum of 64 in the pinned implementation.

For a multi-repository service, prefer an explicit `NumThreads` per sidecar so total CPU use is bounded.

---

## A.10 Stack size

Pyrefly's worker pool uses a relatively large default thread stack and supports the environment variable:

```text
PYREFLY_STACK_SIZE
```

Do not lower it casually. Type solving can be recursive.

Track sidecar virtual memory/RSS effects before increasing thread count substantially.

---

# Appendix B — Application-owned Rust DTO schema

## B.0 Protocol root

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum SemanticRequest {
    Initialize(InitializeRequest),
    LoadWorkspace(LoadWorkspaceRequest),
    ApplyChanges(ApplyChangesRequest),
    FileFacts(FileFactsRequest),
    IsSubtype(IsSubtypeRequest),
    ResolveQualifiedTarget(ResolveQualifiedTargetRequest),
    Health,
    Shutdown,
}
```

Response envelope:

```rust
#[derive(Serialize, Deserialize)]
pub struct ResponseEnvelope<T> {
    pub protocol_version: u32,
    pub workspace_id: String,
    pub semantic_generation: u64,
    pub result: Result<T, SemanticErrorDto>,
}
```

---

## B.1 Source identity

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct SourceIdentityDto {
    pub uri: String,
    pub module_name: String,
    pub content_digest: String,
    pub source_kind: SourceKindDto,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum SourceKindDto {
    Python,
    Stub,
    Notebook,
    Namespace,
    ExternalStub,
}
```

---

## B.2 Byte ranges

```rust
#[derive(
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Hash,
)]
pub struct ByteRange {
    pub start: u32,
    pub end: u32,
}
```

Validate on deserialize/apply:

```rust
fn validate_range(range: ByteRange, source_len: usize) -> Result<()> {
    anyhow::ensure!(range.start <= range.end);
    anyhow::ensure!(usize::try_from(range.end)? <= source_len);
    Ok(())
}
```

---

## B.3 Canonical type

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct CanonicalTypeDto {
    pub display: String,
    pub kind: CanonicalTypeKindDto,
    pub precision: TypePrecisionDto,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CanonicalTypeKindDto {
    Named {
        qualified_name: String,
        args: Vec<CanonicalTypeDto>,
        traits: Vec<TypeTraitDto>,
        unspecified_type_arg_count: Option<u32>,
    },
    Callable {
        params: Vec<CanonicalTypeDto>,
        return_type: Box<CanonicalTypeDto>,
    },
    TypeVariable {
        name: String,
        bounds: Vec<CanonicalTypeDto>,
    },
    Union {
        members: Vec<CanonicalTypeDto>,
    },
    Unknown,
}
```

Query's indexed type table intentionally normalizes many internal distinctions. The canonicalizer may preserve that normalized shape and enrich it later with TSP detail. Store the upstream structural hash as provenance/cache metadata, but compute durable identity from the application-owned canonical structure.

---

## B.4 Type roles

```rust
pub enum TypeRoleDto {
    Declared,
    Computed,
    Expected,
    Member,
    Return,
    Parameter,
    Base,
}
```

---

## B.5 Located type

```rust
pub struct LocatedTypeDto {
    pub range: ByteRange,
    pub role: TypeRoleDto,
    pub type_id: CanonicalTypeId,
    pub upstream_structural_hash: Option<u64>,
    pub provenance: ProvenanceDto,
}
```

---

## B.6 Callee

```rust
pub struct CallFactDto {
    pub callee_range: ByteRange,
    pub targets: Vec<CalleeTargetDto>,
    pub provenance: ProvenanceDto,
}

pub struct CalleeTargetDto {
    pub qualified_name: String,
    pub defining_class: Option<String>,
    pub kind: CalleeKindDto,
    pub confidence: ResolutionConfidenceDto,
}
```

---

## B.7 Members

```rust
pub struct ClassFactsDto {
    pub class_name: String,
    pub class_qname: Option<String>,
    pub declaration_range: Option<ByteRange>,
    pub members: Vec<MemberFactDto>,
}

pub struct MemberFactDto {
    pub name: String,
    pub kind: MemberKindDto,
    pub type_: Option<CanonicalTypeDto>,
    pub is_final: bool,
}
```

---

## B.8 Diagnostics

```rust
pub struct DiagnosticDto {
    pub range: Option<ByteRange>,
    pub code: Option<String>,
    pub severity: DiagnosticSeverityDto,
    pub message: String,
}
```

If only rendered Query diagnostics are available:

```text
range = None
code = None
severity = Error/Unknown
message = rendered string
```

Mark backend capability so consumers know diagnostic structure is limited.

---

## B.9 Provenance

```rust
pub struct ProvenanceDto {
    pub engine: String,             // "pyrefly"
    pub engine_version: String,     // "1.2.0"
    pub backend: BackendKindDto,    // query/tsp/lsp
    pub source_digest: String,
    pub semantic_generation: u64,
    pub confidence: ResolutionConfidenceDto,
}
```

---

## B.10 File facts

```rust
pub struct FileSemanticFactsDto {
    pub source: SourceIdentityDto,
    pub semantic_generation: u64,
    pub types: Vec<LocatedTypeDto>,
    pub calls: Vec<CallFactDto>,
    pub classes: Vec<ClassFactsDto>,
    pub diagnostics: Vec<DiagnosticDto>,
    pub status: SemanticStatusDto,
}
```

This is the ideal atomic payload applied to one CPG semantic partition.

---

# Appendix C — CPG reconciliation recipes

## C.0 Build a per-file range index

From Ruff structural nodes:

```rust
struct RangeIndex {
    expressions_by_exact_range: HashMap<ByteRange, SmallVec<[NodeId; 2]>>,
    calls_by_callee_range: HashMap<ByteRange, SmallVec<[NodeId; 2]>>,
    declarations_by_name_range: HashMap<ByteRange, NodeId>,
    interval_tree: IntervalTree<NodeId>,
}
```

Maintain role-specific indexes rather than one generic interval map.

---

## C.1 Attach a type fact

Algorithm:

```text
input: LocatedTypeDto

1. require source_digest == file digest
2. exact expression range lookup
3. filter by expression-compatible node kind
4. if exactly one -> attach
5. if multiple -> choose smallest semantic-compatible node
6. if none -> interval lookup for exact semantic anchor rules
7. if still none -> create SemanticOccurrence node or record reconciliation miss
8. emit HAS_COMPUTED_TYPE
```

Never silently discard unmatched facts; count them.

---

## C.2 Attach a call fact

```text
input: CallFactDto

1. exact lookup in calls_by_callee_range
2. verify Call.func range / attribute/name target
3. map each target qname to SymbolId
4. if source symbol absent -> ExternalSymbol
5. emit CALLS/MAY_CALL edge per target
6. if no target -> retain DynamicCall fact
```

---

## C.3 Resolve target qname

Lookup order:

```text
exact source symbol qname
exact exported/re-export qname
stub symbol qname
external symbol cache
create external placeholder
```

Store both:

```text
reported_target_qname
canonical_target_symbol_id
```

This protects against future qname normalization changes.

---

## C.4 Class member reconciliation

```text
Ruff:
  class declaration + source members

Pyrefly:
  effective class field type/property/finality facts

Reconciler:
  join by class identity + member name
```

If Pyrefly reports a synthesized member not in Ruff syntax:

```text
create SyntheticMember node
origin_class = class
```

---

## C.5 Definition/reference fallback

If a Ruff reference cannot be resolved by local lexical logic:

```text
1. use Pyrefly LSP definition
2. map URI/range to source declaration
3. if external/stub -> external symbol
4. attach REFERS_TO
```

For bulk initial indexing, consider extending the sidecar to expose a batch definition map from Pyrefly's retained index instead of issuing LSP requests one by one.

---

## C.6 CFG and flow types

CPG owns CFG:

```text
BasicBlock
CONTROL_FLOW
BRANCH_TRUE
BRANCH_FALSE
EXCEPTION_FLOW
```

Pyrefly occurrence types enrich nodes.

Useful validation:

```text
if branch narrows x:
  type of x occurrences in successor differs from predecessor
```

Do not try to reconstruct Pyrefly's exact internal flow graph from output types.

---

## C.7 Def-use

CPG structural def-use:

```text
assignment -> local binding -> references
```

Pyrefly semantic definition resolution can correct:

- nonlocal/global;
- imports;
- shadowing;
- class/member ambiguity;
- cross-file targets.

If the two disagree:

```text
prefer Pyrefly for semantic target
retain structural edge provenance for diagnostics
```

---

## C.8 Inheritance

Ruff:

```text
class base expression range
```

Pyrefly:

```text
base expression computed type / definition
```

Reconcile to:

```text
Class --EXTENDS--> Class
```

If base is dynamic/unresolved:

```text
UnresolvedBaseEdge {
  source_expression,
  type_if_known
}
```

---

## C.9 Import edges

Ruff captures exact syntax.

TSP `resolveImport` or Pyrefly module resolver confirms target.

Do not rewrite the source import text in the graph to a canonical qname; store both.

---

## C.10 Graph confidence query patterns

### High-confidence blast radius

```text
CALLS edges confidence Exact/UnionResolved
REFERS_TO resolved
IMPORTS_MODULE resolved
```

### Conservative blast radius

Include:

```text
MAY_CALL
dynamic member-name candidates
unresolved imports by text
```

### Agent context mode

Return confidence/provenance alongside each edge so the coding LLM can decide whether to inspect source before changing a target.

---

# Appendix D — Recommended semantic fact coverage tiers

## D.0 Tier 0 — syntax-only fallback

Always available:

```text
AST
declarations
calls
attributes
imports
CFG
comments/trivia
structural def-use
```

No Pyrefly dependency.

---

## D.1 Tier 1 — baseline Pyrefly

```text
get_type_table_in_file
get_callees_with_location
diagnostics
```

This already dramatically improves CPG quality.

---

## D.2 Tier 2 — class and type operations

```text
get_attributes
is_subtype
resolve_target_from_qualified_name
```

Useful for framework-aware queries.

---

## D.3 Tier 3 — TSP structured detail

```text
computed type
declared type
expected type
declaration-rich type objects
type aliases
import resolution
semantic snapshots
```

Use selectively or batch through your adapter.

---

## D.3A Tier 3A — Glean bulk index enrichment

```text
bulk declarations
bulk xrefs
searchable symbol/name facts
source spans
```

Use only through a normalized sidecar adapter; do not persist raw Glean predicate identity as the CPG schema.

---

## D.4 Tier 4 — navigation index

```text
definitions
references
implementations
call hierarchy
workspace symbols
```

Use LSP or add bulk pinned-source adapter methods.

---

## D.5 Tier 5 — application-specific internal extensions

Only if needed:

```text
affected-module list after invalidation
bulk definition/reference map
resolved class MRO dump
structured member types
export/re-export fact batch
fine-grained dependency-change summary
```

These are **not claimed as upstream Query 1.2.0 methods**. They are reasonable extensions inside the isolated pinned sidecar because the product protocol remains stable even when the implementation changes.

---

# Appendix E — Practical deployment decision

For the complete Rust Python CPG described in this reference, use:

```text
Main process:
  Ruff-based structural frontend
  gix/repository watcher/history
  CPG graph/model/storage
  semantic client

Pyrefly sidecar:
  facebook/pyrefly@1.2.0
  Query::add_files
  Query::change_files
  Query::get_type_table_in_file
  Query::get_callees_with_location
  Query::get_attributes
  Query::is_subtype
  Query::resolve_target_from_qualified_name
  optional Glean/TSP/LSP exposure

Contract:
  application-owned versioned semantic DTOs

Persistence:
  graph nodes/edges never depend on Pyrefly internal Rust types
```

This is the highest-leverage arrangement because it simultaneously provides:

```text
Rust-only implementation
high bulk indexing throughput
strong Python type semantics
type-aware call graph edges
incremental semantic invalidation
process crash/memory isolation
independent Ruff/Pyrefly release trains
stable CPG persistence schema
```

The one thing it deliberately does **not** promise is perfect static resolution of Python's runtime dynamism. Instead, it makes unresolved semantics explicit and queryable, which is the correct foundation for a robust coding-agent CPG.
