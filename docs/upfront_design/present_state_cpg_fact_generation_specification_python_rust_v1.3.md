# Present-State Code Property Graph Fact Generation Specification

**Artifact ID:** `codefabric-present-state-cpg-fact-generation`
**Artifact kind:** Normative document
**Compatible suite major:** 1
**Release date:** 2026-08-20
**Canonical digest:** External; recorded in `codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/artifact-index.json`

**Status:** Released normative implementation specification
**Synchronized suite version:** 1.3
**Specification version:** 1.3
**Companion ontology:** `code_property_graph_present_state_fact_ontology_specification_v1.3.md`
**Target languages:** Python and Rust
**Implementation language:** Rust
**Primary provider stack:** Tree-sitter, Ruff Python crates, Pyrefly, `rustc_public`/MIR, narrowly scoped `rustc_private`, and petgraph
**Scope:** Generation of present-state facts and mechanically derived facts only
**Out of scope:** History, runtime observation, environment inventory, test-impact analysis, refactor analysis, risk scoring, recommendations, and other evaluative conclusions
**Audit integration (2026-08-20):** Plan-audit F-001; fixed provider wire packages, event mappings, registry shape, and shared credit authority.

---

## 0. Synchronized CodeFabric 1.3 governing contract

This document is a released member of the synchronized **CodeFabric present-state CPG specification suite, version 1.3**. The suite integrates the architecture-completion contracts `G-01` through `G-84`; the earlier standalone completion specification is retained only as a historical design record and is no longer required to interpret this release.

The cross-cutting source of authority is `codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md`. This document permanently owns the domain contracts assigned to it by that manifest. A less-specific statement elsewhere in this document SHALL be read through the 1.3 contract sections and SHALL NOT override them.

### 0.1 Artifact identity and version

```yaml
artifact_id: "codefabric-present-state-cpg-fact-generation"
artifact_kind: document
version: "1.3"
compatible_suite_major: 1
status: released
canonical_digest: external
```

The canonical digest and exact source digest are recorded in `codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/artifact-index.json`. Versions are integer pairs, never floating-point values; `1.10` is newer than `1.9`.

### 0.2 Permanent ownership and precedence

| Concern | Normative owner in 1.3 |
|---|---|
| Fact meanings, kinds, properties, evidence semantics, identifiers, unknowns, projections, summaries, concurrency, effects, and conformance profiles | Present-State CPG Ontology Specification 1.3 |
| Immutable source images, analysis-context discovery, provider protocols, provider authority, capability evidence, model packs, precision profiles, generated/lowered capture, and normalized observations | Present-State CPG Fact Generation Specification 1.3 |
| Arrow/Delta schemas, canonical reconciliation, derivation materialization, durable publications, hot overlays, `ServingSnapshot`, snapshot leases, and overlay-aware DataFusion providers | Present-State CPG Data Fabric Specification 1.3 |
| Workspace registration, authorized roots, watching, Git interpretation, invalidation, update waves, operational state, freshness barriers, recovery, and daemon lifecycle | Continuous CPG Update and Lifecycle Specification 1.3 |
| Controlled semantic language, deterministic resolver, typed `PlanSpec`, result references, completeness proofs, cost limits, canonical JSON, source context, streaming, and response semantics | Semantic Query Specification 1.3 |
| Protobuf RPC, capability credentials, local IPC, cancellation, artifacts, MCP resources, public status, fairness, and serving-layer source-disclosure enforcement | FastMCP Serving Specification 1.3 |
| Cross-cutting artifact governance, compatibility, release profile, acceptance tests, upgrades, and release manifest | Suite Governance and Release Manifest 1.3 |

A downstream layer SHALL consume its upstream machine artifact or API and SHALL NOT recreate the same registry, parser, identity rule, status mapping, or semantic interpretation.

### 0.3 Canonical component topology and terminology

```text
workspace registry and authorization
        ↓
WorkspaceCoordinator actor (one per workspace_id)
        ├─ source inventory and immutable source-image store
        ├─ watcher/Git interpretation and update-wave scheduler
        ├─ provider job manager
        ├─ reconciliation and derivation engine
        ├─ durable publication manager
        └─ active ServingSnapshot pointer
                ↓
overlay-aware DataFusion catalog
                ↓
semantic resolver → typed PlanSpec → execution → canonical response/artifact
                ↓
per-agent FastMCP STDIO adapter
```

Canonical terms are:

| Term | Meaning |
|---|---|
| workspace | One registered and authorized source instance: one Git worktree or one non-Git root |
| repository | Optional common Git repository parent shared by one or more workspaces |
| context | One deterministic Python or Rust semantic/build configuration |
| context set | Ordered immutable set of contexts pinned by a snapshot |
| owner | Smallest deterministic current-state replacement unit for a fact family |
| provider observation | Provider-owned evidence before canonical reconciliation |
| canonical fact | Reconciled first-class entity-existence, relation, or property proposition |
| durable publication | Immutable Delta table-version map for a coherent durable base |
| hot overlay | Immutable in-memory effective-state delta over one durable publication |
| ServingSnapshot | Durable base plus consolidated overlay and all interpretation metadata |
| capability | Named fact-production ability for a declared scope, context, and profile |
| completeness | Whether a declared fact universe is closed for a declared proof scope |

### 0.4 Compatibility and fail-fast negotiation

Compatibility is negotiated by artifact family, not by an approximate global version match:

- ontology and public schema families require the same major and advertised minor/code support;
- direct Arrow/Delta table readers and writers require the exact schema-bundle digest;
- ID-preimage and type-algebra versions require an exact match and changes require reindexing;
- provider and RPC protocols require the same major, a negotiated minor, and compatible required feature bits;
- a `ServingSnapshot` pins exact ontology, schema, provider, derivation, phrase-registry, query-language, and deployment-profile digests;
- the rustc extractor requires the exact pinned nightly/toolchain and adapter digest for its Rust context;
- model packs require matching schema major, semantic compatibility, target package range, and trust policy.

Negotiation SHALL fail before query acceptance or provider activation with a stable error such as `INCOMPATIBLE_MAJOR`, `UNSUPPORTED_MINOR`, `BUNDLE_DIGEST_MISMATCH`, `REQUIRED_FEATURE_UNSUPPORTED`, `SCHEMA_DIGEST_MISMATCH`, `TOOLCHAIN_MISMATCH`, or `MODEL_PACK_INCOMPATIBLE`.

### 0.5 Requirement traceability and generated machine contracts

Normative requirements use stable IDs of the form `CF-<owner>-<four digits>` and participate in a generated trace graph from ontology kind through provider capability, storage mapping, query phrase, response field, RPC/MCP surface, implementation unit, and verification test. IDs are never reused.

The suite SHALL generate and fingerprint, at minimum:

```text
ontology and property registries
canonical enum/flag and error registries
analysis-context, type-algebra, graph-projection, summary, precision, and model-pack registries
Arrow/Delta schema bundle and overlay schema bundle
semantic request and response JSON Schemas
controlled phrase registry and grammar
PlanSpec schema
Protobuf RPC package
FastMCP/Pydantic public schemas
provider protocol schemas
bundle manifests and deployment profile
requirements trace graph and conformance reports
```

Prose is not a substitute for these machine contracts. Generated artifacts SHALL be reproducible from one declared source and compared by canonical digest in CI.

### 0.6 Default deployment profile

The mandatory baseline profile is local, single-user, read-only, and present-state only:

- Linux and macOS are the conforming 1.x platforms; Windows is explicitly unsupported by `local-workstation-v1`;
- one central daemon hosts multiple authorized workspaces, with one mutable coordinator and one active snapshot pointer per workspace;
- one FastMCP STDIO process is launched per programming agent;
- daemon communication uses authenticated local IPC; network listeners are disabled by default;
- the daemon never mutates repositories, runs Git credentials, executes hooks, performs checkout, or follows unauthorized roots;
- source bytes are authoritative, with Git and watcher data used only for interpretation and acceleration;
- HTTP/ASGI, multi-user gateways, distributed fabrics, history analytics, runtime observations, and write-capable agent tools are excluded from the 1.3 baseline.

### 0.7 Canonical source-instance and root identity

`workspace_id` identifies exactly one authorized analyzed source instance. For Git it maps one-to-one to one linked or main worktree; for non-Git it maps to one registered root. `repository_id` and `worktree_id` are nullable subordinate identities and never replace `workspace_id`.

Workspace registration is explicit, persisted, authorization-scoped, and stateful. Root confinement is enforced with byte/native paths, component-wise secure opening, symlink policy, and post-open containment checks rather than string-prefix tests.

### 0.8 Canonical current-state object and leases

A durable publication is not the current query state. The sole query pin is one immutable leased `ServingSnapshot`:

```text
ServingSnapshot
    = exact durable base publication and Delta table-version map
    + one consolidated immutable hot-overlay manifest
    + source generation and inventory digest
    + analysis-context set
    + capability and diagnostics indexes
    + source-trust, event-stream, and Git-acceleration summaries
    + exact ontology/schema/provider/derivation/query/deployment bundle digests
```

Every query applies its structured freshness policy, atomically leases one snapshot, and uses that snapshot for semantic resolution, planning, execution, response materialization, artifact retention, and source-context reads.

### 0.9 Freshness policies and barrier semantics

The public vocabulary is:

```text
BEST_AVAILABLE_SNAPSHOT      explicit opt-in; may be POTENTIALLY_STALE
AWAIT_LATEST                 wait through the admitted-event barrier
REQUIRE_CURRENT_FOR_TARGETS  default; requested capabilities current for resolved targets
REQUIRE_SOURCE_CURRENT       current source/syntax; semantic gaps remain explicit
REQUIRE_SEMANTIC_CURRENT     requested semantic/derived capabilities current or fail
```

A prior snapshot SHALL never satisfy a current requirement. Barrier admission, superseding generations, owner capability, and terminal query freshness are governed by the lifecycle state machine.

### 0.10 Analysis contexts, canonical types, dependencies, and FFI

Every semantic or compiler-dependent fact carries a required `analysis_context_id`; source and syntax facts use `context:source`. A snapshot pins an ordered `analysis_context_set_id`. Incompatible contexts never merge into one exact fact, path, or negative proof.

Python and Rust contexts are discovered deterministically, canonically serialized, fingerprinted, and selected according to the generation and query contracts. Type identity uses the canonical type algebra rather than provider debug strings. External dependencies follow the declaration/body policy, and cross-language links follow the explicit FFI profile with exact, possible, or unknown linkage evidence.

### 0.11 Byte-safe paths, file identity, and source content

Path identity is byte/native and workspace-relative. The common contract carries raw bytes, platform/encoding code, deterministic comparison key, display string, and lossy-display flag. Display text is never an identifier or authorization key.

Source bytes are authoritative. Decoded text is optional and tagged with encoding/newline metadata. File identity distinguishes a source path slot from a content generation and from semantic owners, so replacement, atomic save, rename, and move are represented without conflating path continuity with content or declaration identity.

### 0.12 Canonical IDs and first-class facts

Internal IDs are application-owned 16-byte BLAKE3-derived values over versioned, domain-separated, length-prefixed canonical preimages. Public IDs are lowercase, typed, and round-trippable. Context-sensitive propositions include `workspace_id` and `analysis_context_id` in their preimage.

Every query-visible proposition is a first-class fact with fact ID, owner, context, provenance, certainty, resolution, directness, precision profile, and completeness interpretation. Relations use the universal relation contract; independently sourced properties use the universal property-fact contract; denormalized columns are projections only.

### 0.13 Orthogonal state dimensions and completeness

The suite SHALL NOT overload one status. It maintains distinct provider-run, owner-capability, completeness, query-execution, query-availability, freshness, limit, dependency, publication, snapshot-activation, source-trust, event-stream-health, and Git-acceleration dimensions.

Unknown remainder is explicit. A negative claim is valid only under the completeness and negative-proof algebra or from an explicit negative fact. Empty, unavailable, unresolved, filtered-empty, and limit-reached outcomes remain distinguishable.

### 0.14 Reconciliation, derivation, and materialization ownership

Provider adapters emit observations; they never write canonical graph state. The data-fabric `ReconciliationEngine` is the sole canonicalization authority. The derivation registry assigns exactly one implementation and precision profile to every derived family and declares whether the family is materialized durably, maintained in the overlay, computed on demand, or unavailable.

Petgraph, DataFusion operators, and custom solvers are implementation mechanisms, not competing semantic authorities.

### 0.15 Query, RPC, and serving boundaries

A 1.3 semantic query targets exactly one authorized workspace. Separately indexed dependencies and submodules are endpoint-only unless their declarations are represented inside the same snapshot. Composite cross-workspace body traversal remains unsupported.

The semantic layer owns controlled-language resolution and typed `PlanSpec`; the adapter forwards canonical request bytes and never constructs SQL, graph syntax, or semantic interpretations. Semantic request ID, MCP call ID, RPC attempt ID, and daemon query ID are distinct. Stable errors preserve layer, retryability, safe message, diagnostic reference, field/phrase context, and dependency failure.

### 0.16 Authorization, source disclosure, and local security

Fact access, source-text disclosure, path disclosure, diagnostics, and artifact reads are separately authorized. Local transport authentication uses short-lived capability credentials bound to agent, workspace, adapter process, operations, and expiry. All source and artifact reads recheck authorization; display paths never widen scope.

Provider processes, build scripts, proc macros, model packs, malformed source, requests, and artifacts are treated as untrusted inputs under the sandbox and adversarial-corpus contracts.

### 0.17 Conformance, upgrades, and supersession

The suite is accepted only through the golden corpus, clean-rebuild comparator, machine-contract conformance harness, deterministic fault injection, performance profiles, security corpus, and upgrade/rollback choreography in the suite manifest.

Any older example that uses repository-only scoping, publication-only query pinning, UTF-8-only path identity, optional contexts, a single ambiguous status, provider-native identity, or adapter-side semantic interpretation is superseded by this section and the permanent 1.3 completion-contract sections in this document.

## 0.18 Release-integration status

This 1.3 document contains its permanent architecture-completion contracts and explicit cross-layer obligations. It no longer depends on `codefabric_architecture_completion_and_missing_design_specifications_v1.0.md` as a normative override. The historical gap IDs remain in headings and trace artifacts so every decision can be audited back to `G-01` through `G-84`.

## 1. Purpose

This document specifies **how to generate every fact family defined by the Comprehensive Present-State Code Property Graph Ontology** from the selected Rust-based analysis libraries and from additional deterministic processing implemented by the CPG system.

The governing separation is:

```text
Provider libraries
    harvest source, syntax, semantic, type, and compiler facts

CPG normalization
    converts provider-native objects into stable canonical facts

CPG reconciliation
    joins overlapping provider outputs into one coherent present-state graph

CPG derived-analysis layer
    computes objective graph, control-flow, dataflow, alias, ownership,
    and summary facts that the provider libraries do not emit directly

LLM programming agent
    reasons over those facts and draws task-specific conclusions
```

This specification SHALL stop at the fact substrate. It SHALL NOT encode judgments such as:

```text
SAFE_TO_REFACTOR
TEST_IMPACTED
HIGH_RISK
VULNERABLE
SHOULD_CHANGE
```

The output is a comprehensive factual representation from which downstream agents may independently reason.

---

## 2. Source basis and version anchors

This specification is grounded in the following attached references.

| Reference | Version anchor | Role in this specification |
|---|---:|---|
| `tree_sitter_rust_python.md` | `tree-sitter 0.26.12`; `tree-sitter-python 0.25.0`; `tree-sitter-rust 0.24.2`; grammar ABI 15 | Incremental concrete syntax, raw node kinds, fields, ranges, parse recovery, queries, changed ranges, and source/CST reconciliation |
| `ruff_python_crates_advanced_reference_2026-08-18.md` | Ruff `0.16.1`; component crates `0.0.7` | Python source coordinates, tokens, typed AST, comments/trivia, omitted lexical facts, local scopes/bindings/references/import semantics |
| `pyrefly_rust_cpg_advanced_reference_1.2.0_2026-08-19.md` | Pyrefly `1.2.0` | Python project-aware types, computed/declared/expected types, call targets, members, imports, declarations/xrefs, subtype and MRO-aware semantics |
| `rust_mir_cpg_continuous_reference_2026-08-18.md` | nightly `2026-08-18`; `rustc_public 1.100.0-nightly` | Rust semantic definitions, types, MIR bodies, CFG, places, rvalues, calls, instances, moves, borrows, drop, unwind, generated code, and compiler provenance |
| `petgraph.md` | petgraph `0.8.3` | In-memory graph projections and algorithms: traversal, SCCs, condensation, dominators, reachability, filtered/reversed views, and graph construction |
| `code_property_graph_present_state_fact_ontology_specification_v1.3.md` | current companion artifact | Canonical node, relationship, metric, summary, uncertainty, Python-profile, and Rust-profile requirements |

Provider APIs are version-sensitive. All provider-specific integration SHALL be isolated behind application-owned adapters and DTOs.

---

## 3. Normative scope

### 3.1 Included generation work

This specification includes generation of:

- current source files and byte spans;
- tokens, comments, documentation, directives, parse errors, and missing syntax;
- complete raw and normalized syntax;
- declarations, symbols, scopes, bindings, and references;
- imports, exports, re-exports, and source-declared dependency edges;
- semantic types and type relationships;
- class/member, trait/impl, MRO, descriptor, and override facts;
- callable contracts, call sites, arguments, argument-to-parameter binding, dispatch, and targets;
- normal and exceptional control flow;
- values, definition/use events, reaching definitions, liveness, and data dependencies;
- memory locations, access paths, reads, writes, aliases, and points-to facts;
- Rust moves, copies, borrows, reborrows, regions, loans where available, and drops;
- direct and transitive effects;
- exceptions, panic, unwind, cleanup, and resource-lifetime facts;
- async, generator, coroutine, task, thread, channel, and lock facts;
- closures and captures;
- macros, generated code, lowered code, MIR, shims, drop glue, and specializations;
- objective graph analyses;
- deterministic interprocedural summaries;
- explicit unknowns.

### 3.2 Excluded generation work

The system SHALL NOT collect or emit as ontology facts:

- Git history or prior snapshots;
- semantic changes across revisions;
- runtime traces or coverage;
- production values or profiles;
- host/interpreter environment inventories;
- live deployment state;
- test-impact conclusions;
- refactor-safety conclusions;
- vulnerability exploitability conclusions;
- architectural quality judgments;
- recommendations.

### 3.3 Required analysis-context model without an environment ontology

Providers require reproducible project/build inputs. Version 1.3 therefore treats analysis context as required operational provenance rather than an optional environment fact.

Each indexed context SHALL have an application-owned `analysis_context_id` and fingerprint. At minimum:

```text
PythonAnalysisContext
  Python language version and platform tags
  project/module roots and import-resolution policy
  selected config files
  stub roots and typed dependency identities
  lockfile/dependency-set fingerprint
  Pyrefly/provider bundle fingerprint

RustAnalysisContext
  package and target identity
  feature set
  cfg set and target triple
  compilation profile and relevant rustflags
  Cargo metadata/lock/config fingerprints
  build-script/proc-macro/generated-input policy
  pinned rustc toolchain and extractor bundle fingerprint
```

Source and syntax facts that do not depend on semantic configuration use the reserved `context:source`. Every semantic/compiler fact, owner capability, provider run, canonical ID, publication, overlay, and query result SHALL carry its exact context.

A workspace MAY index multiple contexts. Provider output from different contexts SHALL be emitted as separate owner/context batches and SHALL never be reconciled into one exact proposition. Configuration details remain operational inputs and are not exposed as a host-environment inventory ontology.

# Part I — Provider Architecture

## 4. Provider responsibility model

### 4.1 Provider roles

| Provider | Authoritative responsibility | Non-responsibility |
|---|---|---|
| **Source store** | Current source bytes, paths, digests, line indexes, immutable per-run snapshot | Semantics |
| **Tree-sitter** | Error-tolerant CST, every grammar node/token-like node, fields, byte/point ranges, missing/error nodes, changed ranges, query-based local syntax extraction | Name resolution, types, compiler semantics, CFG/dataflow |
| **Ruff parser/AST** | Authoritative typed Python AST on the current source, parser tokens, parse diagnostics, unsupported-syntax diagnostics | Cross-project typing |
| **Ruff trivia/index** | Comments, explicit parentheses, multiline/interpolated strings, continuations, pragma/source-layout facts | Name/type resolution |
| **Ruff semantic adapter** | Python per-module scopes, bindings, definitions, references, imports, qualified names, builtins, shadowing, rebinding, branch/execution context | Full Python typing |
| **Pyrefly sidecar** | Python module resolution, inferred types, declared/computed/expected types, call targets, members, subtype/MRO-aware semantics, optional declarations/xrefs | Persistent graph topology, CFG, def-use |
| **Tree-sitter Rust grammar** | Rust source CST and source-level syntax, including incomplete code | Compiler-resolved Rust semantics |
| **rustc_public/MIR adapter** | Rust definitions, types, generics, traits/impls, MIR, calls, instances, places, state transitions, unwind, drop, constants, ABI | Durable IDs, some borrowck/vtable details |
| **rustc_private adapter** | Narrowly selected missing compiler facts: stable IDs, exact source mapping, borrowck/loan facts, vtable/mono detail where required | General-purpose graph schema |
| **petgraph** | Ephemeral in-memory graph projection and graph algorithms | Persistent storage, semantic extraction, query service |
| **CPG custom analysis** | Python CFG, all def-use/dataflow overlays, alias/points-to, effect propagation, control dependence, summaries, unknown materialization, provider reconciliation | Provider-native parsing/type checking |

### 4.2 Central responsibility rule

```text
Tree-sitter tells us what concrete syntax exists now.
Ruff tells us what Python source says structurally and lexically.
Ruff semantic tells us local Python binding meaning.
Pyrefly tells us what Python means statically across modules and types.
rustc/MIR tells us what Rust means after compiler analysis and lowering.
petgraph computes graph properties over normalized projections.
CodeFabric owns identity. Provider normalization is defined here; canonical reconciliation and derivation materialization are owned by the data-fabric `ReconciliationEngine` and `DerivationRegistry`; lifecycle owns scheduling and activation.
```

---

## 5. Authority and precedence

When multiple providers describe the same conceptual fact, the reconciler SHALL apply the following authority rules.

### 5.1 Python authority order

| Fact family | Primary authority | Secondary/fallback |
|---|---|---|
| Current source bytes | Source store | None |
| Concrete syntax during incomplete edits | Tree-sitter | Ruff recovered parse |
| Typed Python syntax on parsable source | Ruff AST | Tree-sitter CST |
| Tokens/comments/trivia | Ruff parser/trivia/index | Tree-sitter extras |
| Local scope/binding/reference | Ruff semantic adapter | Custom AST scope builder |
| Cross-module definition/reference | Pyrefly Glean/LSP/internal adapter | Ruff qualified-name resolution |
| Computed expression types | Pyrefly Query type table | TSP |
| Declared/expected type distinction | Pyrefly TSP | Ruff annotation syntax + Query computed type |
| Call targets | Pyrefly Query callees | Type-directed custom candidate generation |
| Members/properties/finality | Pyrefly Query/TSP | Ruff class/decorator syntax |
| Python CFG | CPG custom builder from Ruff AST | Tree-sitter only when Ruff parse is unusable |
| Def-use/dataflow | CPG custom analysis | Ruff/Pyrefly used for binding/type enrichment |
| Alias/points-to | CPG custom conservative analysis | Type facts constrain candidates |

### 5.2 Rust authority order

| Fact family | Primary authority | Secondary/fallback |
|---|---|---|
| Current source bytes | Source store | None |
| Source CST | Tree-sitter Rust grammar | rustc spans/HIR-adjacent facts |
| Semantic definitions/types | rustc_public | rustc_private |
| Stable compiler identity | rustc_private stable-key adapter | Application qualified-name key |
| CFG/state transitions | MIR | Source syntax only for correspondence |
| Calls/instances | MIR + `Instance` resolution | Trait/fn-pointer custom overapproximation |
| Moves/copies/borrows/drop | MIR | borrowck adapter for exact loan state |
| Exact borrowck loan/region facts | rustc_private | CPG conservative dataflow |
| Macro expansion/source mapping | rustc spans/private source-map adapter | Tree-sitter invocation syntax |
| Derived analyses | CPG + petgraph | None |

### 5.3 Conflict policy

The reconciler SHALL never silently overwrite conflicting provider facts.

It SHALL instead:

1. retain provider-specific evidence;
2. choose one canonical fact according to authority;
3. record the conflicting evidence in diagnostics or provenance;
4. emit an unknown or multiple-candidate fact if conflict prevents a sound canonical result.

---

## 6. End-to-end architecture

```text
                    CURRENT SOURCE SNAPSHOT
                              |
             +----------------+----------------+
             |                                 |
      Tree-sitter lane                   Authoritative semantic lane
      ----------------                   ---------------------------
      Python/Rust CST                    Python: Ruff + Pyrefly
      errors/missing                     Rust: rustc + MIR
      raw node kinds                     compiler definitions/types
      source fields                      calls/places/instances
             |                                 |
             +----------------+----------------+
                              |
                       NORMALIZATION
                              |
                     application-owned DTOs
                              |
                       RECONCILIATION
                              |
                    canonical base fact set
                              |
                  GRAPH PROJECTION BUILDERS
                              |
      +-----------+-----------+----------+-----------+
      |           |           |          |           |
    CFG       Call graph   Def-use    Alias graph  Type graph
      |           |           |          |           |
      +-----------+-----------+----------+-----------+
                              |
               petgraph + custom fixed-point analyses
                              |
                    derived objective facts
                              |
                  interprocedural summaries
                              |
                    explicit unknown facts
                              |
                  atomic present-state publication
```

---

## 7. Provider isolation requirements

### 7.1 Tree-sitter adapter

The adapter SHALL expose application-owned records and SHALL NOT expose long-lived `Node<'tree>` values.

Recommended output:

```rust
pub struct RawSyntaxFact {
    pub id: SyntaxOccurrenceId,
    pub raw_kind: String,
    pub normalized_kind: NormalizedSyntaxKind,
    pub span: SourceSpan,
    pub named: bool,
    pub extra: bool,
    pub error: bool,
    pub missing: bool,
    pub parent: Option<SyntaxOccurrenceId>,
    pub field_name: Option<String>,
    pub ordinal: u32,
}
```

### 7.2 Ruff adapter

The adapter SHALL contain all Ruff `0.0.x` types. Public output SHALL use CPG-owned records.

```rust
pub struct PythonFrontendBatch {
    pub source: SourceFileFact,
    pub tokens: Vec<TokenFact>,
    pub syntax: Vec<SyntaxFact>,
    pub comments: Vec<CommentFact>,
    pub directives: Vec<DirectiveFact>,
    pub scopes: Vec<ScopeFact>,
    pub bindings: Vec<BindingFact>,
    pub references: Vec<ReferenceFact>,
    pub imports: Vec<ImportFact>,
    pub parse_diagnostics: Vec<ParseFact>,
}
```

### 7.3 Pyrefly sidecar

Pyrefly SHALL be isolated behind a stable process/DTO boundary because:

- its Rust library API is explicitly unstable;
- its Ruff component version may differ from the main process;
- its allocator/threading/crash behavior should be isolated;
- response-local type indexes and internal IDs are not product identity.

Recommended request groups:

```text
load_workspace
analyze_files
get_type_table
get_callees
get_members
resolve_definition
get_declarations_xrefs
get_declared_computed_expected_type
```

### 7.4 rustc adapter

All `rustc_public` and `rustc_private` objects SHALL be converted to owned records inside the compiler callback.

No compiler-owned object may escape the callback or cross threads.

### 7.5 petgraph isolation

Petgraph node/edge indexes SHALL be treated as ephemeral projection handles.

Persistent fact IDs SHALL remain application-owned domain IDs.

---

# Part II — Canonical Extraction Contracts

## 8. Immutable source-image contract

Every provider job SHALL analyze one immutable source image from one workspace and source generation.

```rust
pub struct WorkspacePathDto {
    pub workspace_relative_bytes: std::sync::Arc<[u8]>,
    pub platform_code: PathPlatformCode,
    pub comparison_key: std::sync::Arc<[u8]>,
    pub display: String,
    pub display_is_lossy: bool,
}

pub struct SourceSnapshot {
    pub workspace_id: WorkspaceId,
    pub source_generation: u64,
    pub file_id: FileId,
    pub path: WorkspacePathDto,
    pub language: LanguageId,
    pub bytes: std::sync::Arc<[u8]>,
    pub digest: ContentDigest,
    pub line_index: LineIndexDto,
    pub encoding: SourceEncoding,
    pub newline_kind: NewlineKind,
}
```

`display` is never identity. Providers SHALL echo `workspace_id`, `source_generation`, `file_id`, and `digest` in terminal manifests so stale or cross-workspace output can be rejected.

---

## 9. Canonical source coordinates

The canonical range is:

```text
(workspace_id, file_id, source_digest, start_byte, end_byte)
```

with half-open bytes `[start_byte, end_byte)`. Line/column and UTF-16 positions are boundary representations only.

- Ruff `TextRange` and Tree-sitter byte ranges map directly.
- Pyrefly/TSP/LSP ranges are converted by the sidecar against the supplied digest.
- rustc spans are converted inside the compiler process.
- Approximate source correspondence SHALL set `source_anchor_precision = APPROXIMATE`.

Per-file interval indexes SHALL cover syntax nodes, expressions, declarations, identifier occurrences, call sites, type syntax, and semantic entities.

---

## 10. Provider-observation metadata

Provider adapters emit observations, not canonical facts.

```rust
pub struct ObservationMeta {
    pub observation_id: ObservationId,
    pub workspace_id: WorkspaceId,
    pub analysis_context_id: AnalysisContextId,
    pub source_generation: u64,
    pub owner_id: OwnerId,
    pub language: LanguageId,
    pub producer: ProducerId,
    pub producer_version: String,
    pub certainty: EvidenceCertainty,
    pub resolution: ResolutionClass,
    pub source_span: Option<SourceSpan>,
    pub input_digest: ContentDigest,
}
```

The certainty and resolution values SHALL be the ontology 1.3 registries. `COMPILER_EXACT` is an evidence-certainty value, not an alternate unregistered vocabulary.

Canonical `fact_id`, selected canonical producer, directness, property-fact creation, and conflict resolution are assigned only by the data-fabric `ReconciliationEngine`.

---

## 11. Two-stage batch and Arrow boundary

The provider boundary has exactly two stages:

1. **Adapter-owned observation chunks** for provider isolation and bounded local construction.
2. **Canonical normalized Arrow streams** at the data-fabric boundary.

```rust
pub struct ProviderObservationChunk {
    pub header: ObservationChunkHeader,
    pub entities: Vec<EntityObservation>,
    pub relations: Vec<RelationObservation>,
    pub properties: Vec<PropertyObservation>,
    pub diagnostics: Vec<ProviderDiagnostic>,
    pub capabilities: Vec<OwnerCapabilityObservation>,
}
```

A chunk is bounded and MAY use row DTOs inside one process. It SHALL be converted immediately to registered Arrow `RecordBatch` schemas before entering reconciliation or durable/hot publication. Cross-process providers SHOULD stream Arrow IPC observation batches plus typed manifest events rather than one unbounded `Vec` payload.

The conversion layer SHALL:

- avoid retaining duplicate row and Arrow representations after handoff;
- use bounded channels and backpressure;
- preserve owner/context/source-generation fences;
- validate schema and dictionary/enum codes;
- reject partial terminal manifests unless the provider explicitly reports `PARTIAL` capability.

---

## 12. Raw and normalized observation preservation

Every provider-native kind that may matter later SHALL remain representable through raw-kind registries and cold evidence payloads. Canonical observations additionally use normalized ontology kinds.

Tree-sitter raw node and field catalogs are generated from the exact grammar crate's
`NODE_TYPES` plus runtime `Language` introspection. The generated catalog records the
package version, runtime ABI, node-types digest, complete named/anonymous node-kind
inventory, complete field inventory, and grammar fingerprint. Only the mapping from
provider-native kinds to application-owned normalized kinds is owner-authored. A typed
normalization record may define exact raw-name overrides, non-overlapping raw-name prefix
families, and an optional canonical default, in that precedence order; ignores remain
exact raw keys. The contract compiler resolves those authored rules over the complete
generated inventory and rejects ambiguous prefixes or any raw kind without an explicit
normalize, ignore, or version-bound unsupported outcome. Ruff raw-kind mappings generate
exhaustive enum matches from that resolved table so a pinned-provider upgrade cannot
introduce an unmapped variant silently. Provider fixture discovery is validation, not
raw-kind authority.

```text
provider-native observation
    → normalized observation record
    → Arrow observation batch
    → data-fabric reconciliation
    → canonical entity/relation/property facts
```

Provider-native payloads SHALL not become the only queryable representation.

---

## 13. Canonical semantic-identity inputs

This specification defines identity inputs; the ontology/data fabric own final ID serialization.

### 13.1 Named entities

Named semantic keys SHALL include:

```text
workspace_id
analysis_context_id when context-dependent
language
entity kind
owning module/crate/package identity
canonical qualified name or compiler-stable key
specialization/generic arguments when executable identity differs
```

### 13.2 Source occurrences

Occurrence keys SHALL include workspace, exact file identity, source digest/range,
normalized occurrence family/kind, normalized role, structural parent/field/ordinal
anchor, and owner. Path display text, provider-local node handles, provider identity,
and reconciliation disposition SHALL not participate.

Providers construct occurrence and relation identities only through the generated
recipe-aware CBEF builders owned by AC-G-13. The builder encodes occurrence structure
inside the released `ENTITY.semantic_key` and occurrence-specific relation structure
inside the governed `RELATION_FACT.role`; providers SHALL NOT append CBEF fields or call
the generic field-vector encoder. Occurrence-family codes and persisted syntax/provider
flags come from generated enum/flag accessors. Module-local numeric codes, bit masks, and
overloaded flag bits for reconciliation or evidence disposition are prohibited.

### 13.3 Anonymous entities

Anonymous entities use owner-relative normalized structural anchors. When source movement invalidates the anchor, a new ID is correct unless a language/compiler stable key proves continuity.

### 13.4 Rust compiler identity

Session-local `DefId`, `CrateNum`, MIR block/local ordinals, and pointer addresses SHALL not escape. The rustc adapter emits stable compiler keys where available and explicit canonical key components otherwise.

### 13.5 Context separation

A context-dependent entity or proposition SHALL include `analysis_context_id` in its ID preimage. Context-independent source/syntax entities use `context:source`. Reconciliation SHALL reject an attempted exact merge across different contexts.

# Part III — Python Fact Generation

## 14. Python pipeline overview

```text
SourceSnapshot
  |
  +-- Tree-sitter Python
  |     raw CST, fields, errors, missing nodes, incremental changed ranges
  |
  +-- Ruff parser/source/trivia/index
  |     typed AST, tokens, comments, parentheses, strings, continuations
  |
  +-- Ruff semantic adapter
  |     local scopes, bindings, references, imports, shadow/rebind
  |
  +-- Pyrefly sidecar
        module semantics, types, callees, members, xrefs, definitions
  |
  +-- CPG custom Python analyses
        CFG, values, access paths, def-use, aliasing, effects,
        exception/resource/async models
  |
  +-- Reconciliation and derived graph analyses
```

---

## 15. Python source and lexical fact generation

| Ontology fact | Provider | Generation rule | Additional processing |
|---|---|---|---|
| `SOURCE_FILE` | Source store | One node per current `.py` or `.pyi`; notebook containers are unsupported in the 1.x core-source profile | Normalize path; compute digest and line index; inventory a notebook with explicit `UNSUPPORTED` capability evidence rather than parsing cells implicitly |
| `SOURCE_SPAN` | All providers | Convert every range to canonical bytes | Validate boundaries against current source |
| `TOKEN` | Ruff parser tokens | Emit every parser token with raw kind and span | Normalize token class; assign ordinal |
| `IDENTIFIER_TOKEN` | Ruff tokens/AST | Token kind or identifier AST range | Link to identifier occurrence |
| `KEYWORD_TOKEN` | Ruff tokens | Keyword token kind | Preserve spelling |
| `OPERATOR_TOKEN` | Ruff tokens | Operator/punctuation token | Link to operation syntax |
| `LITERAL_TOKEN` | Ruff tokens | Numeric/string/bytes literal token | Preserve raw spelling hash or slice |
| `COMMENT` | Ruff `CommentRanges`/tokens | Emit exact comment ranges | Classify own-line/end-of-line |
| `DOCUMENTATION` | Ruff docstring helpers | Detect module/class/function docstrings | Link documentation to semantic owner |
| `PRAGMA_OR_DIRECTIVE` | Ruff trivia/pragmas | Recognize `noqa`, `type: ignore`, formatter directives, type comments | Classify directive and target syntax |
| `PARSE_ERROR` | Ruff `ParseError` + Tree-sitter `ERROR` | Emit both; canonicalize overlapping errors | Ruff is typed-parser authority; TS preserves recovery region |
| `MISSING_SYNTAX` | Tree-sitter missing nodes | Emit zero-width missing nodes and expected kind | Do not synthesize from Ruff absence |
| Explicit parentheses | Ruff `ParenthesizedExpressions` | Set `syntax_detail.explicitly_parenthesized` on the reconciled occurrence | Attach to smallest matching expression; do not duplicate the typed extension as a relation |
| Multiline/interpolated string range | Ruff `Indexer` | Emit string-region facts | Link to string expression |
| Continuation line | Ruff `Indexer` | Emit physical continuation fact | Useful for exact source structure |

### 15.1 Tree-sitter use

Tree-sitter SHALL preserve the complete raw CST, including anonymous tokens and parser recovery. It is the completeness fallback when Ruff cannot produce a clean typed AST.

### 15.2 Ruff use

Ruff SHALL be the canonical Python typed syntax source when its parse is usable. Tree-sitter nodes and Ruff AST nodes SHALL be related through `CORRESPONDS_TO` when ranges/kinds match.

---

## 16. Python syntax fact generation

### 16.1 Ruff AST traversal

The Ruff adapter SHALL traverse every `Stmt`, `Expr`, `Pattern`, and `TypeParam`.

It SHALL emit:

```text
SYNTAX_NODE
STATEMENT
EXPRESSION
PATTERN
DECLARATION_SYNTAX
TYPE_SYNTAX
PARAMETER_SYNTAX
ARGUMENT_SYNTAX
BLOCK
LITERAL
OPERATION
ATTRIBUTE_ACCESS
SUBSCRIPT_ACCESS
CALL_EXPRESSION
ASSIGNMENT
BRANCH
LOOP
RETURN
YIELD
AWAIT
RAISE_SYNTAX
IMPORT_SYNTAX
```

### 16.2 Structural roles

For every child relationship, the adapter SHALL emit:

```text
AST_CHILD(parent, child, field_name, ordinal)
```

Field names SHALL be application-owned normalized names such as:

```text
name
parameters
decorator
returns
body
condition
target
value
receiver
callee
argument
keyword_argument
iterable
guard
pattern
handler
finally_body
```

### 16.3 Evaluation order

The syntax graph SHALL preserve source containment order separately from evaluation order.

The Python CFG/value extractor SHALL use Ruff's evaluation-order visitor semantics or a custom explicit traversal; source-order AST child ordinals SHALL not be assumed to equal runtime evaluation order.

### 16.4 Tree-sitter correspondence

For each Ruff AST node, find the smallest Tree-sitter named node satisfying:

```text
same or enclosing byte range
compatible normalized kind
compatible parent field
```

Emit:

```text
RUFF_AST_NODE --CORRESPONDS_TO--> TREE_SITTER_CST_NODE
```

Unmatched nodes remain valid; no one-to-one relationship is assumed.

### 16.5 Parse recovery representation

One provider recovery condition has one normalized application observation. Its
canonical projection produces all applicable surfaces together:

```text
PARSE_ERROR or MISSING_SYNTAX entity
syntax_detail.error or syntax_detail.missing flag
source_annotation diagnostic row
```

Overlapping Ruff and Tree-sitter parse errors reconcile to one canonical parse-error
entity while retaining both provider evidence records. `MISSING_SYNTAX` originates
only from a Tree-sitter missing node and is never inferred from Ruff absence. These
surfaces are projections of one observation, not independently authored facts.

---

## 17. Python semantic-entity generation

### 17.1 Declarations

Ruff AST SHALL generate source-backed declaration candidates for:

```text
MODULE
FUNCTION
ASYNC_FUNCTION
LAMBDA
CLASS
PARAMETER
TYPE_PARAMETER
TYPE_ALIAS
VARIABLE
IMPORT_BINDING
MATCH_CAPTURE
COMPREHENSION_TARGET
EXCEPTION_TARGET
WITH_TARGET
```

### 17.2 Ruff semantic enrichment

A version-pinned traversal adapter SHALL populate `SemanticModel` and normalize:

```text
SCOPE
BINDING
DEFINITION
REFERENCE
SHADOWS
REBINDS
BUILTIN_REFERENCE
RESOLVED_REFERENCE
UNRESOLVED_REFERENCE
IMPORT
QUALIFIED_NAME
BRANCH_CONTEXT
EXECUTION_CONTEXT
```

`SemanticModel::new` SHALL NOT be treated as a complete analysis; the adapter must reproduce or reuse Ruff's binding/traversal/cleanup order.

### 17.3 Canonical merge

```text
Ruff AST declaration candidate
    + Ruff semantic binding/definition
    + Pyrefly declaration/definition identity
    = canonical semantic entity
```

The canonical semantic node SHALL remain distinct from every source occurrence.

---

## 18. Python scope and binding generation

### 18.1 Scope construction

Create scopes for:

```text
module
function
class
lambda
comprehension
annotation
type-parameter domain
```

Primary source:

- Ruff semantic scopes when available;
- custom AST scope builder as fallback.

### 18.2 Binding events

Generate binding events for:

```text
function/class names
parameters
assignment targets
annotated assignments
augmented assignments
named expressions
imports and aliases
for/async-for targets
with/async-with targets
exception targets
match captures
comprehension targets
global/nonlocal declarations
type parameters
type aliases
```

### 18.3 Reference classification

Every `Name` occurrence SHALL be classified:

```text
READ
WRITE
READ_WRITE
DELETE
TYPE_REFERENCE
CALL_REFERENCE
IMPORT_REFERENCE
```

### 18.4 Resolution

- Ruff semantic produces local lexical binding edges.
- Pyrefly definition/xref data upgrades cross-module and type-dependent resolution.
- Unresolved names emit `REFERS_TO -> UNKNOWN_SYMBOL`.
- Star-import-dependent names emit `MAY_REFER_TO` candidates if exports are available; otherwise `UNKNOWN_SYMBOL`.

### 18.5 Shadow/rebind

Normalize Ruff shadow and rebinding state into:

```text
SHADOWS
REBINDS
GLOBAL_RESOLUTION
NONLOCAL_RESOLUTION
CAPTURES
CAPTURED_FROM
```

---

## 19. Python module/import/export generation

### 19.1 Import syntax

Ruff AST emits:

```text
IMPORT_DECLARATION
relative_level
imported_module_text
imported_name
alias
star_import
```

### 19.2 Semantic resolution

Pyrefly module resolver/Glean/TSP resolves:

```text
IMPORTS_MODULE
IMPORTS_SYMBOL
```

Ruff qualified-name resolution provides a local fallback.

### 19.3 Import binding

Each imported local name becomes a `BINDING` connected to:

- import syntax;
- resolved module;
- imported symbol where available.

### 19.4 Exports and re-exports

Primary sources:

- Pyrefly export/index data;
- explicit `__all__` custom evaluation for literal containers;
- top-level binding rules.

Custom logic SHALL:

1. statically evaluate literal `__all__` assignments/concatenations where safe;
2. classify imports exposed from module scope as re-export candidates;
3. emit `EXPORTS` and `REEXPORTS`;
4. emit `UNKNOWN_MODULE` or incomplete export status when dynamic export construction is encountered.

No runtime import execution is permitted.

---

## 20. Python type generation

### 20.1 Computed types from Query table

For each file:

1. call `get_type_table_in_file`;
2. decode the response-local type table;
3. intern every normalized type into the CPG type graph;
4. map every located occurrence to the best matching expression/reference node;
5. emit `COMPUTED_TYPE`.

The response-local `type_index` SHALL never be persisted as global identity.

### 20.2 Type interning

Canonical type identity is computed from structured normalized shape:

```text
kind
qualified name
ordered type arguments
callable parameter/return shapes
type-variable bounds
traits such as TypedDict/tuple
```

Pyrefly structural hashes MAY accelerate interning but SHALL be verified against shape equality.

### 20.3 Declared types

Sources:

- Ruff annotation syntax;
- Pyrefly TSP `getDeclaredType`;
- Pyrefly declaration type metadata.

Emit:

```text
DECLARED_TYPE
PARAMETER_TYPE
RETURN_TYPE
FIELD_TYPE
```

### 20.4 Expected types

Use TSP `getExpectedType` for high-value expression classes or a bulk sidecar extension.

Emit `EXPECTED_TYPE`.

If expected type is unavailable, do not infer it solely from assignment syntax unless the inference rule is exact and explicitly marked `STATIC_SEMANTIC`.

### 20.5 Type relationships

Generate:

```text
TYPE_ARGUMENT
TYPE_PARAMETER_OF
BOUNDED_BY
CONSTRAINED_BY
ALIAS_TARGET
UNION_MEMBER
INTERSECTION_MEMBER
```

Use Pyrefly `is_subtype` only for demanded relationships or bounded candidate sets. Do not materialize all-pairs subtype closure.

### 20.6 Flow narrowing

For each expression occurrence, compare:

```text
declared type of binding
computed type at occurrence
```

Emit `NARROWS_TO` when the occurrence type is a strict refinement.

The cause MAY be classified from surrounding Ruff AST/CFG:

```text
none check
isinstance
literal comparison
match pattern
TypeGuard
TypeIs
truthiness
```

If cause cannot be proven, emit the narrowing edge without a cause label.

### 20.7 Type uncertainty

Materialize distinct unknowns:

```text
UNKNOWN_TYPE
ANY explicit/implicit/error where exposed
UNBOUND
NEVER
```

Do not treat missing type output as `Any`.

---

## 21. Python object/member generation

### 21.1 Source-declared members

Ruff class-body traversal emits:

```text
DECLARES_MEMBER
METHOD
CLASS_VARIABLE
PROPERTY_CANDIDATE
NESTED_TYPE
```

Instance fields assigned through `self.x` are collected as member candidates with assignment locations.

### 21.2 Pyrefly member enrichment

`get_attributes` and structured type queries provide:

```text
member type
property/field kind
finality
synthesized members where exposed
```

### 21.3 MRO and inheritance

Ruff emits base syntax. Pyrefly resolves base types.

Custom logic:

1. emit `INHERITS`;
2. obtain MRO from a sidecar adapter if available;
3. otherwise compute C3 linearization from resolved direct bases;
4. emit ordered `MRO_PRECEDES`;
5. emit `UNKNOWN_TYPE`/unknown base when dynamic.

### 21.4 Descriptors and properties

Use:

- decorator syntax;
- Pyrefly property/descriptor semantics;
- resolved getter/setter/deleter functions.

Emit:

```text
PROPERTY_FOR
DESCRIPTOR_FOR
GETTER_FOR
SETTER_FOR
DELETER_FOR
CLASS_METHOD_OF
STATIC_METHOD_OF
```

### 21.5 Overrides

For each class method:

1. traverse resolved MRO parents;
2. locate same-name member;
3. verify callable/member compatibility where available;
4. emit `OVERRIDES` and `OVERRIDDEN_BY`.

This is deterministic semantic processing, not an evaluative judgment.

### 21.6 Member resolution at access sites

For every `Attribute` expression:

1. obtain receiver computed type;
2. ask Pyrefly definition/member resolution when available;
3. emit `RESOLVES_MEMBER`;
4. for union receivers, emit `MAY_RESOLVE_MEMBER` to each candidate;
5. if `__getattr__`, descriptor, metaclass, or dynamic writes prevent resolution, include `UNKNOWN_MEMBER`.

---

## 22. Python callable-contract generation

### 22.1 Syntax contract

Ruff emits:

```text
parameter order
positional-only
positional-or-keyword
varargs
keyword-only
kwargs
defaults
decorators
return annotation
async
generator syntax
type parameters
```

### 22.2 Semantic contract

Pyrefly callable types enrich:

```text
resolved parameter types
resolved return type
overloads
bound receiver
generic variables
residual signatures for partials where exposed
```

### 22.3 Argument binding

Custom Python argument binder SHALL map actual arguments to formal parameters:

- positional order;
- positional-only restrictions;
- keyword matching;
- duplicate argument detection state;
- `*args` expansion when statically known;
- `**kwargs` expansion when keys are statically known;
- defaulted parameters;
- bound receiver insertion.

Emit:

```text
HAS_ARGUMENT
ARGUMENT_BINDS_TO
```

Unexpanded dynamic splats SHALL bind to an `UNKNOWN_ARGUMENT_SET` sentinel rather than being ignored.

---

## 23. Python call-site and dispatch generation

### 23.1 Call-site creation

Every Ruff `Call` expression becomes `CALL_SITE`.

Emit:

```text
HAS_CALLEE_EXPRESSION
HAS_RECEIVER where attribute/bound form
HAS_ARGUMENT
CONTAINS_CALL
```

### 23.2 Call-target enrichment

Use Pyrefly `get_callees_with_location`.

Map returned kinds to:

```text
DIRECT_FUNCTION_CALL
BOUND_METHOD_CALL
CLASS_METHOD_CALL
STATIC_METHOD_CALL
CONSTRUCTOR_CALL
CALLABLE_OBJECT_CALL
DECORATOR_APPLICATION
```

### 23.3 Target reconciliation

Resolve Pyrefly target strings to canonical symbols:

1. exact qualified internal declaration;
2. internal stub declaration;
3. external symbol;
4. unknown external symbol.

Emit:

```text
CALLS_EXACT_TARGET
CALLS_DECLARATION
MAY_CALL
```

### 23.4 Constructor semantics

Where Pyrefly exposes `__new__`/`__init__` targets, represent them separately.

A class-call site may emit:

```text
CALLS_DECLARATION -> class constructor contract
MAY_CALL/CALLS_EXACT_TARGET -> __new__
MAY_CALL/CALLS_EXACT_TARGET -> __init__
```

### 23.5 Callable objects

If receiver type defines `__call__`, emit a callable-object dispatch edge to that member.

### 23.6 Decorator applications

For each decorator syntax:

- emit `DECORATED_BY`;
- create a decorator-application call site;
- resolve through Pyrefly;
- emit call edges.

### 23.7 Dynamic calls

When target resolution is missing because of:

```text
Any
getattr
registry lookup
dynamic import
monkey patching
unknown callable value
```

emit:

```text
CALLS_UNKNOWN -> UNKNOWN_CALL_TARGET
dispatch_kind = UNKNOWN_DYNAMIC
```

Known candidate targets may coexist with an unknown remainder.

---

## 24. Python CFG generation

Neither Ruff nor Pyrefly provides the required durable whole-file CFG. The CPG SHALL implement a Python CFG builder over Ruff AST.

### 24.1 CFG unit

Build one CFG per:

```text
module body
function
async function
lambda
comprehension/generator expression where separate evaluation graph is useful
```

### 24.2 Core nodes

```text
ENTRY
EXIT
BASIC_BLOCK
EXPRESSION_OPERATION
STATEMENT_OPERATION
RETURN_POINT
EXCEPTIONAL_EXIT
SUSPEND_POINT
RESUME_POINT
```

### 24.3 Evaluation-order rules

The builder SHALL model Python evaluation order for:

- callee before arguments;
- positional arguments in order;
- keyword value expressions in order;
- boolean short circuit;
- chained comparisons with single evaluation of intermediate operands;
- conditional expressions;
- assignment RHS before targets;
- augmented-assignment read before write;
- attribute/subscript receiver/index evaluation;
- iterable before loop target/body;
- context expressions before `with` body;
- decorator evaluation/application order;
- default expressions at function definition time;
- class bases, keywords, decorators, and class body execution;
- comprehension iterable/filter/result ordering.

### 24.4 Statement rules

| Construct | CFG generation |
|---|---|
| Sequential statements | `CFG_NEXT` |
| `if`/`elif`/`else` | condition block with `CFG_TRUE`/`CFG_FALSE`; merge block |
| `while` | condition/header, true body edge, false exit/else edge, loop back |
| `for`/`async for` | iterator setup, next-test header, target binding, body, loop back, exhaustion/else |
| `break` | edge to loop exit |
| `continue` | edge to loop header/next iteration |
| `return` | evaluate value then `CFG_RETURN` to exit |
| `raise` | evaluate exception/cause then exceptional edge |
| `try` | protected region, typed/general handlers, else, finally, propagation |
| `try*` | exception-group handler paths represented separately |
| `with` | evaluate managers, enter calls, body, reverse-order exit calls on normal/exceptional paths |
| `async with` | await enter/exit operations and suspension edges |
| `match` | subject once, ordered case tests, pattern binds, guards, body, next-case failure |
| `assert` | condition true continuation; false raises `AssertionError` |
| `yield` | edge to suspend, resume edge to continuation |
| `yield from` | delegation loop/suspend model |
| `await` | suspend and resume edges |
| function/class declaration | definition-time expression evaluation in enclosing CFG; body gets separate CFG |

### 24.5 Exceptional edges

Every operation that may raise SHOULD have an exceptional successor at the chosen precision.

Scalable policy:

- exact explicit `raise`;
- exact call/attribute/subscript/iteration/context-manager exceptional categories;
- summarized exceptional edge from basic block to nearest active handler/finally;
- preserve handler type syntax and Pyrefly-resolved exception type where available.

### 24.6 Finally semantics

The builder SHALL route every exit from a protected region through `finally`:

```text
normal fallthrough
return
break
continue
exception
```

and then resume the pending continuation unless `finally` overrides it.

### 24.7 CFG validation

For each CFG:

```text
one entry
one synthetic normal exit
explicit exceptional exit
every nonterminal block has successor
return does not fall through
break/continue target valid enclosing loop
finally routing complete
```

---

## 25. Python value and dataflow generation

### 25.1 Value nodes

Create value-producing nodes for:

```text
literals
name reads
attribute reads
subscript reads
call returns
unary/binary/comparison results
container construction
lambda/function/class objects
await/yield results
conditional merges
```

### 25.2 Definition events

Generate definitions for:

```text
parameters
assignments
annotated assignments with value
augmented assignments
loop targets
with targets
exception targets
match captures
named expressions
imports
function/class bindings
comprehension targets
```

### 25.3 Use events

Generate uses for:

```text
name reads
receiver reads
callee reads
argument reads
conditions
return/yield values
index/key expressions
decorators
annotations when evaluated
```

### 25.4 Access-path extraction

Normalize assignment/read targets:

```text
x                     LOCAL/GLOBAL/CELL location
obj.x                 FIELD/INSTANCE_MEMBER location
C.x                   CLASS_MEMBER location when resolved
obj[index]            INDEXED_LOCATION
module.x              MODULE/GLOBAL location when resolved
```

### 25.5 Reaching definitions

Run owner-local forward dataflow:

```text
IN[B]  = union OUT[pred]
OUT[B] = GEN[B] union (IN[B] - KILL[B])
```

Binding identity from Ruff determines local/global/cell variable domains.

Attribute/container locations use conservative kill rules.

Emit:

```text
REACHING_DEFINITION
REACHES
DEF_USE
DATA_DEP
VALUE_FLOWS_TO
KILLS_DEFINITION
```

### 25.6 Merge values

At CFG joins with multiple reaching definitions, create `MERGED_VALUE` or retain a multi-source relation set.

The ontology need not expose SSA syntax, but merge provenance SHALL remain recoverable.

### 25.7 Liveness

Run backward dataflow over local/cell/global binding domains:

```text
OUT[B] = union IN[succ]
IN[B]  = USE[B] union (OUT[B] - DEF[B])
```

Emit `LIVE_AT` when materialization is enabled.

---

## 26. Python memory, alias, and points-to generation

Python has no compiler-provided complete alias analysis in this stack. The CPG SHALL implement a conservative abstract-object analysis.

### 26.1 Abstract object allocation sites

Create abstract objects for:

```text
list/dict/set/tuple construction
class instance construction
lambda/function/class object creation
generator/coroutine creation
comprehension result
literal mutable containers
unknown external return
```

Immutable primitive literals MAY use canonical value nodes rather than allocation objects.

### 26.2 Points-to constraints

Generate constraints:

```text
x = allocation       => x points-to object
x = y                => points-to(x) includes points-to(y)
x = call(...)        => points-to(x) includes call-return abstraction
obj.f = y            => field points-to propagation
x = obj.f            => points-to(x) includes field points-to
container[k] = y     => element summary points-to
x = container[k]     => x receives element summary
```

### 26.3 Field sensitivity

Default precision:

```text
field-sensitive for statically known attribute names
key-insensitive or literal-key-sensitive for mappings
index-insensitive for dynamic sequence indexes
allocation-site-sensitive within callable
```

### 26.4 Alias facts

Derive:

```text
MUST_ALIAS
```

only for identical singleton points-to sets under exact assignment semantics.

Derive:

```text
MAY_ALIAS
```

when points-to sets intersect.

Unknown external/dynamic operations add `UNKNOWN_MEMORY` to affected sets.

### 26.5 Dynamic invalidation

The following facts widen alias/member state:

```text
setattr with dynamic name
__dict__ mutation
eval/exec
unknown external call receiving mutable object
monkey patching
star imports affecting globals
```

The analysis SHALL not pretend unaffected precision after these barriers.

---

## 27. Python effect generation

### 27.1 Direct effects from syntax/dataflow

Generate direct effects for:

```text
global/nonlocal writes
attribute/container writes
argument-reachable writes
calls
raise
await/yield
imports
resource acquisition/release model events
```

### 27.2 API model packs

Certain objective effects require library semantic models.

A model pack may specify:

```text
function/method qualified name
direct effects
argument mutation positions
resource creation/release
task/thread spawn
lock acquire/release
channel send/receive
blocking or I/O behavior
return points-to relation
```

Examples:

```text
open                     creates file resource; performs I/O
file.close               releases resource
asyncio.create_task      spawns task
threading.Thread.start   spawns thread
Lock.acquire/release     synchronization
queue.put/get            sends/receives
socket/file/database APIs I/O
```

Model facts SHALL be marked `MODELLED` and include model-pack version.

### 27.3 Unknown effects

Any unresolved call SHALL contribute `UNKNOWN_EFFECT` to the direct summary unless it is proven pure by a model.

### 27.4 Transitive effects

Interprocedural propagation is defined in Part V.

---

## 28. Python exceptional-flow generation

### 28.1 Raise facts

Ruff AST emits explicit raise sites.

Pyrefly types may resolve exception classes.

### 28.2 Call and operation exceptions

Use:

- model-pack declared exceptions where available;
- conservative `MAY_RAISE -> UNKNOWN_EXCEPTION` for unresolved calls;
- built-in operation categories for attribute/subscript/iteration/context-manager operations.

### 28.3 Handler matching

Custom logic maps possible raised types to ordered handlers:

```text
HANDLED_BY
MAY_BE_HANDLED_BY
PROPAGATES_TO
EXECUTES_CLEANUP
```

Dynamic/unknown exception types may match any compatible general handler and retain an unknown propagation edge.

---

## 29. Python resource-lifetime generation

Resource facts require protocol/model knowledge rather than generic syntax alone.

### 29.1 Resource creation

Create resource nodes for modeled constructors/factories.

### 29.2 Ownership and transfer

Use points-to and argument binding to emit:

```text
CREATES_RESOURCE
OWNS_RESOURCE
TRANSFERS_RESOURCE
USES_RESOURCE
```

### 29.3 Release/drop

Modeled close/release methods emit `RELEASES_RESOURCE`.

`with`/`async with` emits enter/exit lifecycle facts.

Python garbage collection SHALL NOT be modeled as deterministic `DROP` unless a specific semantic model justifies it.

---

## 30. Python async, generator, and concurrency generation

### 30.1 Async and generator creation

Ruff syntax identifies async/generator functions.

Call-site type/callee information determines:

```text
CREATES_FUTURE
creates generator object
```

The callee body execution remains separate.

### 30.2 Await/yield

CFG builder emits:

```text
AWAITS
YIELDS
SUSPENDS_AT
RESUMES_AT
```

### 30.3 Task/thread/channel/lock

Generate through model packs and argument/call resolution.

### 30.4 Happens-before

Only emit `HAPPENS_BEFORE` for explicit semantics such as:

```text
awaited task completion before continuation
thread join before continuation
lock release/acquire synchronization when modelled
channel send/receive ordering when guaranteed
```

Otherwise retain only `MAY_RUN_CONCURRENTLY_WITH`.

---

## 31. Python closure and capture generation

Use Ruff semantic free/cell variable information.

Emit:

```text
CAPTURES
CAPTURED_FROM
```

Python capture mode is reference-to-cell semantics for captured local bindings; do not incorrectly label ordinary closure captures as Rust-style by-value unless a specific construct copies a value.

Comprehensions receive separate scopes and captures.

---

## 32. Python generated/synthesized semantic generation

Pyrefly may expose synthesized declarations or framework-generated members.

Generate:

```text
SYNTHESIZED_SYMBOL
GENERATED_FROM
DECLARES_MEMBER
```

only when provider evidence identifies an owning source entity or framework rule.

Do not invent generated source ranges.

Source span may be absent or refer to the owning declaration.

---

## 33. Python explicit-unknown generation

Materialize unknown nodes for:

```text
unresolved name
unresolved import
unresolved member
unresolved call target
unknown type
unknown memory
unknown effect
unknown dynamic attribute name
```

Dynamic constructs SHALL emit both the observable syntax fact and the associated unknown semantic remainder.

Examples:

```text
getattr(obj, name)        -> USES_GETATTR + MAY_RESOLVE_MEMBER UNKNOWN_MEMBER
exec(code)                -> USES_EXEC + UNKNOWN_EFFECT + UNKNOWN_MEMORY
dynamic import            -> DYNAMIC_IMPORT + UNKNOWN_MODULE
unknown callable          -> CALLS_UNKNOWN
```

---

# Part IV — Rust Fact Generation

## 34. Rust pipeline overview

```text
SourceSnapshot
  |
  +-- Tree-sitter Rust
  |     current CST, source declarations, attributes, macros, errors
  |
  +-- Cargo/rustc invocation selected externally
  |     semantic compilation context
  |
  +-- rustc_public callback
  |     crate items, definitions, types, MIR bodies, instances
  |
  +-- rustc_private adapter
  |     stable IDs, source-map precision, borrowck/vtable details as needed
  |
  +-- CPG normalizer
  |     owned Rust DTOs
  |
  +-- custom analyses + petgraph
        def-use, liveness, alias, ownership state, control dependence,
        reachability, summaries
```

---

## 35. Rust source and lexical generation

Tree-sitter Rust SHALL generate:

```text
SOURCE_FILE
SOURCE_SPAN
SYNTAX_NODE
STATEMENT
EXPRESSION
PATTERN
TYPE_SYNTAX
ATTRIBUTE
MACRO_INVOCATION
COMMENT
DOCUMENTATION
PARSE_ERROR
MISSING_SYNTAX
AST_CHILD
LEXICALLY_PRECEDES
```

Tree-sitter remains the source-level completeness layer, including code that does not compile.

Rust tokens may be represented from all CST leaves or from an additional lexer if exact token classification beyond CST leaves is required.

---

## 36. Rust semantic-definition generation

### 36.1 Item discovery

Within `rustc_public::run!`:

- enumerate local items;
- enumerate external referenced definitions as encountered;
- obtain names, kinds, parent relationships, types, and spans;
- copy to owned records.

Generate:

```text
CRATE
MODULE
FUNCTION
METHOD
CLOSURE
STRUCT
ENUM
UNION
VARIANT
FIELD
TRAIT
IMPL
ASSOCIATED_ITEM
TYPE_ALIAS
OPAQUE_TYPE
CONST
STATIC
EXTERN_BLOCK
FOREIGN_FUNCTION
```

### 36.2 Identity

Use `DefPathHash`/stable crate identity through private adapter where available.

Fallback to application qualified keys.

### 36.3 Source correspondence

Map rustc definition spans to Tree-sitter source declarations.

Emit:

```text
SOURCE_SYNTAX --CORRESPONDS_TO--> SEMANTIC_DEFINITION
```

One macro invocation may correspond to multiple generated definitions.

---

## 37. Rust type and generic generation

### 37.1 Type normalization

Recursively normalize `Ty/TyKind` into:

```text
primitive
ADT(definition, args)
tuple
array
slice
reference(mutability, region abstraction, pointee)
raw pointer
FnDef
FnPtr
closure
coroutine
dynamic trait
opaque
generic parameter
associated/projection type
alias
never
```

### 37.2 Type graph edges

Emit:

```text
TYPE_OF
DECLARED_TYPE
PARAMETER_TYPE
RETURN_TYPE
FIELD_TYPE
TYPE_ARGUMENT
LIFETIME_ARGUMENT
CONST_ARGUMENT
TYPE_PARAMETER_OF
BOUNDED_BY
OUTLIVES
```

### 37.3 Trait/impl graph

Emit:

```text
IMPLEMENTS_TRAIT
IMPLEMENTS_METHOD
SUPERTRAIT
ASSOCIATED_WITH
```

### 37.4 Coercions and adjustments

Extract from compiler facts where exposed:

```text
AUTO_DEREF_TO
AUTO_REF_TO
UNSIZES_TO
COERCES_TO
REIFIES_FN_POINTER
```

If unavailable in `rustc_public`, use a narrow private adapter or omit with capability status; do not reconstruct from source text alone.

---

## 38. Rust MIR-body generation

For each MIR-bearing item emit:

```text
MIR_BODY
MIR_LOCAL
MIR_BASIC_BLOCK
MIR_STATEMENT
MIR_TERMINATOR
OPERAND
RVALUE
PLACE
PLACE_PROJECTION
```

### 38.1 Body ownership

```text
semantic callable/const/static
    --LOWERS_TO-->
MIR_BODY
```

### 38.2 Locals

Classify:

```text
return place
argument
inner/user local
compiler temporary
capture
```

Use debug-variable information for source naming, but retain compiler-local identity under owner scope.

### 38.3 Blocks/statements/terminators

Preserve raw variants, spans, ordinals, cleanup flags, and successors.

---

## 39. Rust CFG generation

MIR provides authoritative CFG topology.

### 39.1 Edge mapping

| MIR terminator | Canonical edges |
|---|---|
| `Goto` | `CFG_NEXT` |
| `SwitchInt` | `CFG_CASE`; `CFG_TRUE`/`CFG_FALSE` where boolean |
| `Return` | `CFG_RETURN` |
| `Call` | `CFG_CALL_RETURN` plus unwind |
| `Drop` | `CFG_NEXT`/drop-return plus unwind |
| `Assert` | success plus `CFG_UNWIND`/panic |
| `Resume` | exceptional propagation |
| `Abort` | exceptional terminal |
| `Unreachable` | terminal |
| `InlineAsm` | normal destinations and unwind |

### 39.2 Statement-level expansion

If instruction-level CFG is materialized:

```text
block entry
 -> statement 0
 -> statement 1
 -> terminator
 -> successor block entry
```

This expansion is derived and MAY remain query-time if storage volume is a concern.

---

## 40. Rust place, memory, and access-event generation

### 40.1 Place normalization

```rust
PlaceKey {
    owner,
    base_local,
    projections: Vec<Projection>
}
```

Projection kinds:

```text
DEREF
FIELD
INDEX
CONSTANT_INDEX
SUBSLICE
DOWNCAST
OPAQUE_CAST
```

### 40.2 Access-event intermediate representation

Every MIR construct SHALL first normalize to `AccessEvent`.

```rust
pub struct AccessEvent {
    pub owner: OwnerId,
    pub location: MirLocation,
    pub place: PlaceKey,
    pub kind: AccessKind,
    pub type_id: TypeId,
    pub span: Option<SourceSpan>,
}
```

### 40.3 Mapping

| MIR construct | Access events |
|---|---|
| `Operand::Copy(p)` | `READ`, `COPY` |
| `Operand::Move(p)` | `READ`, `MOVE` |
| `Assign(dst, rhs)` | `WRITE dst` plus RHS events |
| `Ref` | shared/mut borrow |
| `Reborrow` | reborrow |
| `AddressOf` | raw address |
| Call destination | write on normal return |
| `Drop(p)` | drop use |
| `SetDiscriminant` | variant mutation |
| `StorageLive/Dead` | storage state |
| `ThreadLocalRef` | static/TLS reference |

The AccessEvent stream is the canonical input for def-use, ownership-state, alias, and effect analyses.

---

## 41. Rust call and instance generation

### 41.1 Direct calls

For every call terminator:

1. create a call-site node;
2. capture callable operand;
3. capture arguments and destination;
4. extract declared `FnDef` where possible;
5. run `Instance::resolve`;
6. emit normal and unwind successors.

Emit:

```text
CALLS_DECLARATION
CALLS_EXACT_TARGET
CALLS_INSTANCE
MAY_CALL
CALLS_UNKNOWN
```

### 41.2 Function references

Function items in non-call operand positions emit:

```text
REFERENCES_CALLABLE
TAKES_FUNCTION_ADDRESS
PASSES_CALLABLE
RETURNS_CALLABLE
```

### 41.3 Function pointers

Custom intraprocedural points-to propagation:

```text
FnDef coercion -> function-pointer value
copy/move -> propagate set
CFG join -> union set
indirect call -> MAY_CALL each target
```

Unknown external pointer origins add `UNKNOWN_CALL_TARGET`.

### 41.4 Closures

Represent:

```text
closure definition
closure value/environment
captures
closure instance
call shim/target
```

Use `Instance::resolve_closure` where available.

### 41.5 Monomorphic instances

Create `MONO_INSTANCE` keyed by:

```text
definition
canonical generic arguments
instance kind
```

Emit `MONOMORPHIZES`, arguments, ABI/name metadata, and `CALLS_INSTANCE`.

Generic MIR remains one source-level body unless concrete body materialization is explicitly enabled.

---

## 42. Rust trait and dynamic-dispatch generation

### 42.1 Static dispatch

Compiler resolution emits exact implementation targets.

### 42.2 Dynamic trait dispatch

Generate candidate target sets from:

- trait method contract;
- impl inventory;
- unsizing/vtable creation sites;
- receiver points-to/type flow;
- private vtable adapter where available.

Emit:

```text
INVOKES_TRAIT_CONTRACT
USES_VTABLE
MAY_DISPATCH_TO
MAY_CALL
```

Candidate edges are `SOUND_MAY` or `POSSIBLE`, not exact.

Unknown external implementors yield `UNKNOWN_EXTERNAL_IMPLEMENTATION` where the selected compilation does not establish a closed set.

---

## 43. Rust macro and generated-code generation

Tree-sitter emits macro definition/invocation syntax.

rustc span/expansion data emits:

```text
EXPANSION
EXPANDED_ITEM
EXPANDS_TO
GENERATED_FROM
SOURCE_CORRESPONDENCE
```

Use a private span/hygiene adapter if `rustc_public` does not expose sufficient provenance.

No one-to-one source/MIR assumption is permitted.

---

## 44. Rust move, initialization, and ownership-state generation

### 44.1 Base facts

Access events directly emit:

```text
MOVED_TO
COPIED_TO
BORROWS_SHARED
BORROWS_MUTABLY
REBORROWS
DROPS
```

### 44.2 Ownership-state dataflow

Use a forward lattice per place abstraction:

```text
UNINITIALIZED
INITIALIZED
MOVED
MAYBE_INITIALIZED
MAYBE_MOVED
```

Transfer rules:

- assignment initializes destination;
- move invalidates moved path;
- storage live/dead changes availability;
- drop consumes/destroys path;
- call destination initializes on return;
- branch joins form `MAYBE_*`.

Emit program-point facts:

```text
OWNED_AT
MOVED_AT
UNINITIALIZED_AT
```

### 44.3 Exact borrowck facts

When required, rustc_private borrowck adapter emits:

```text
LOAN
LOAN_CREATED_AT
LOAN_LIVE_AT
REGION
REGION_CONTAINS
OUTLIVES
MOVE_PATH
```

If not available, capability status SHALL state that loan liveness is conservative/absent.

---

## 45. Rust def-use and liveness generation

### 45.1 Definitions

```text
assignment destination
call destination on normal return
parameter initialization
return place definition
discriminant mutation
```

### 45.2 Uses

```text
copy/move operands
places read by rvalues
call target and arguments
switch input
assert input
drop place
```

### 45.3 Dataflow

Use the same custom worklist engine as Python with Rust place-aware kill semantics.

Field-sensitive default:

- exact field kills itself and subpaths;
- whole-base write kills all projected subpaths;
- dereference/index writes use alias-aware conservative kills.

Emit:

```text
DEFINES
USES
REACHING_DEFINITION
DEF_USE
DATA_DEP
VALUE_FLOWS_TO
LIVE_AT
```

---

## 46. Rust alias and points-to generation

### 46.1 Safe-reference points-to

Generate from:

```text
Ref/Reborrow
assignments of references/pointers
moves/copies
function-pointer coercions
aggregate fields
call return summaries
```

### 46.2 Precision

Default:

```text
field-sensitive places
allocation/local-site sensitivity
constant-index sensitivity
dynamic-index wildcard
deref edges through points-to sets
```

### 46.3 Raw pointers/unsafe

Raw pointer operations and FFI widen points-to sets.

Emit `UNKNOWN_MEMORY` at opaque boundaries.

### 46.4 Alias facts

As in Python:

```text
MUST_ALIAS only for proven singleton equality
MAY_ALIAS for intersecting points-to sets
DOES_NOT_ALIAS only with compiler/proven separation
```

Rust borrow facts may establish stronger non-alias results for active mutable/shared loans, but such facts require precise borrowck integration.

---

## 47. Rust drop and resource generation

### 47.1 Drop sites

Each MIR `Drop` terminator creates `DROP_SITE`.

Emit:

```text
DROPS
DROPS_FIELD
INVOKES_DROP_GLUE
INVOKES_DROP_IMPL
```

Use `Instance::resolve_drop_in_place`.

### 47.2 Recursive drop glue

Represent nested type drop dependencies without requiring each compiler-generated instruction to be source-authored.

### 47.3 Resource semantics

Rust RAII types may be classified through model packs:

```text
file/socket/lock/guard/transaction resource types
```

MIR drop remains factual even without domain resource classification.

---

## 48. Rust async/coroutine generation

Create separate entities for:

```text
ASYNC_FUNCTION
FUTURE_TYPE
COROUTINE_BODY
COROUTINE_STATE
SUSPEND_POINT
RESUME_POINT
```

Emit:

```text
LOWERS_TO_COROUTINE
CREATES_FUTURE
HAS_STATE
SUSPENDS_AT
RESUMES_AT
```

Use source spans to correlate source `await` expressions to lowered regions best-effort.

Calling an async function SHALL not be represented as immediate execution of its body.

---

## 49. Rust constants/statics/CTFE generation

Generate:

```text
CONST_ITEM
STATIC_ITEM
THREAD_LOCAL_STATIC
CONST_VALUE
CTFE_RESULT
CONST_ALLOCATION
```

Use compiler const evaluation where available.

Normalize values into:

```text
scalar
structured literal
bytes digest
referenced static/function set
opaque
```

Internal compiler allocation handles SHALL not persist.

---

## 50. Rust unsafe/FFI/inline-assembly generation

Tree-sitter/source facts identify unsafe blocks/functions and extern syntax.

MIR/compiler facts identify:

```text
raw address
raw pointer dereference where detectable
foreign call
ABI transition
inline assembly
intrinsic
union access
```

Emit:

```text
CONTAINS_UNSAFE_OPERATION
CALLS_FOREIGN
CROSSES_FFI
USES_INLINE_ASSEMBLY
```

Opaque boundaries contribute `UNKNOWN_EFFECT` and `UNKNOWN_MEMORY` unless modeled.

---

## 51. Rust explicit-unknown generation

Create explicit unknowns for:

```text
indirect fn pointer target
dyn trait target remainder
external definition without body
opaque FFI effect
unsafe alias state
unmapped macro-generated source
unsupported rustc/MIR variant
unavailable borrowck fact
```

Unsupported compiler variants SHALL trigger explicit diagnostics and `UNKNOWN_*` facts rather than silent omission.

---

# Part V — Derived Analyses and petgraph

## 52. Petgraph role

Petgraph SHALL be used as an **ephemeral algorithm kernel**, not as canonical persistence.

### 52.1 Default projection type

For most CPG projections:

```rust
petgraph::graph::DiGraph<DomainNodeId, ProjectionEdge>
```

with:

```text
HashMap<DomainNodeId, NodeIndex>
```

Use because:

- CPG projections are sparse;
- parallel edges may matter;
- algorithm support is broad;
- projections can be rebuilt from canonical facts.

### 52.2 StableGraph use

Use `StableDiGraph` only for long-lived mutable in-memory projections whose handles survive deletions.

Persistent graph identity remains `DomainNodeId`.

### 52.3 GraphMap prohibition for general CPG

`GraphMap` SHALL NOT be the default because:

- it forbids parallel edges;
- CPG nodes often have non-`Copy` external IDs;
- the same endpoints may carry several semantically distinct relations.

It MAY be used for simple interned-ID set relations.

### 52.4 CSR use

`Csr` MAY be built for large, immutable, traversal-heavy projections after sorting and deduplicating edges.

---

## 53. Projection construction

Each derived analysis SHALL declare:

```text
projection name
included node kinds
included edge kinds
edge direction
parallel-edge reduction policy
unknown-node policy
root policy
```

Examples:

```text
call_exact
call_may
cfg_normal
cfg_full
type_inheritance
module_dependency
def_use
points_to
ownership
```

Unknown targets SHALL remain vertices where relevant.

---

## 54. Reachability generation

### 54.1 Direct facts

Direct adjacency derives from canonical edges.

### 54.2 Transitive reachability

Use:

- `Dfs`/`Bfs` for per-root traversal;
- `has_path_connecting` for one-off boolean checks;
- custom multi-source traversal for batches.

Emit:

```text
TRANSITIVELY_REACHES
TRANSITIVELY_REACHED_BY
```

### 54.3 Materialization policy

Do not automatically materialize all-pairs closure for very large graphs.

Supported modes:

```text
on-demand traversal
per-entrypoint cached reachability
SCC-condensed DAG closure
full closure only for bounded projections
```

Facts SHALL identify the projection and exact/may edge policy.

---

## 55. Strongly connected components and recursion

Use `kosaraju_scc` or `tarjan_scc`.

For call graphs:

```text
SCC size > 1 -> mutually recursive set
SCC size = 1 with self-edge -> recursive function
```

Emit:

```text
SCC_ID
SCC_SIZE
CALL_SCC
RECURSIVE_FUNCTION
MUTUALLY_RECURSIVE_SET
CFG_SCC_MEMBER
```

`condensation` MAY create a component DAG for summary propagation.

---

## 56. Dominance generation

Use `petgraph::algo::dominators::simple_fast`.

### 56.1 Forward dominators

- build CFG projection;
- define a synthetic/real entry;
- restrict to reachable nodes;
- compute immediate dominators;
- emit `IMMEDIATE_DOMINATOR`, `DOMINATES`, `STRICTLY_DOMINATES`.

### 56.2 Post-dominators

- create synthetic exit when multiple exits;
- connect all normal exits to synthetic exit;
- reverse the CFG using a graph adaptor or materialized reverse;
- run dominators from synthetic exit;
- map result back;
- emit post-dominator facts.

Separate normal-only and full exceptional CFG analyses if both are retained.

---

## 57. Control-dependence generation

Petgraph does not directly emit control dependence.

Custom algorithm:

1. compute post-dominator tree;
2. for each CFG edge `A -> B` where `B` does not post-dominate `A`;
3. walk from `B` up the immediate post-dominator chain until `ipdom(A)`;
4. mark visited nodes `CONTROL_DEPENDENT_ON A`;
5. retain edge predicate/case label from the originating CFG edge.

Exceptional-flow control dependence SHALL be computed separately or explicitly included by policy.

---

## 58. Loop generation

### 58.1 Natural loops

For every CFG edge `u -> v` where `v DOMINATES u`, classify a back edge.

Compute natural-loop members by reverse predecessor traversal from `u` to `v`.

Emit:

```text
BACK_EDGE
LOOP_HEADER
LOOP_MEMBER
LOOP_NESTING_DEPTH
```

### 58.2 Irreducible loops

Use CFG SCCs when a cyclic component has multiple entries or no single natural header.

Emit SCC-based loop region with `loop_kind = irreducible`.

### 58.3 Source loop correspondence

Map derived loop regions to source `for`/`while`/comprehension loops where spans overlap; compiler-lowered loops may have no direct source loop.

---

## 59. Reaching-definitions framework

Petgraph supplies traversal/topology; CPG code supplies lattice and transfer functions.

### 59.1 Generic worklist

```text
initialize IN/OUT
enqueue entry/all blocks
pop block
compute new state
if changed:
    enqueue successors
repeat to fixed point
```

### 59.2 Domain

Facts are sets of definition IDs indexed by abstract location.

### 59.3 Kill semantics

Language-specific:

- Python bindings and abstract fields/containers;
- Rust places and projection overlap;
- alias-aware widening.

### 59.4 Outputs

```text
REACHING_DEFINITION
REACHES
DEF_USE
DATA_DEP
KILLS_DEFINITION
```

---

## 60. Liveness generation

Backward fixed-point over use/def sets.

Emit `LIVE_AT` for selected variables/places/program points.

For scale, liveness MAY be kept as compressed bitsets per block rather than individual graph edges, while still exposed through the fact API.

---

## 61. Points-to and alias analysis

Petgraph may represent the constraint graph, but CPG code implements the solver.

### 61.1 Constraint kinds

```text
address/allocation
copy
load
store
field projection
call parameter
call return
unknown external
```

### 61.2 Solver

Use iterative set propagation, optionally SCC-condense copy constraints.

### 61.3 Language profiles

- Python: allocation-site abstract objects and dynamic unknown widening.
- Rust: local/place/reference/pointer targets; stronger safe-reference facts; raw-pointer/FFI widening.

### 61.4 Outputs

```text
POINTS_TO
MAY_POINT_TO
MUST_ALIAS
MAY_ALIAS
DOES_NOT_ALIAS
ALIAS_SET
POINTS_TO_SET
```

---

## 62. Shortest graph distance

For unweighted projections:

- BFS is preferred.
- `dijkstra` with unit edge cost MAY be used.

Emit `SHORTEST_GRAPH_DISTANCE` only for explicitly selected bounded/rooted projections or on demand.

Do not compute dense all-pairs distance by default.

---

## 63. Connected components

Use:

- `connected_components` for weak components;
- SCC algorithms for directed mutual reachability.

Emit `CONNECTED_COMPONENT` with projection identity.

---

## 64. Transitive reduction and closure

Petgraph `tred` routines are DAG-only.

Policy:

1. never run directly on cyclic graph;
2. condense SCCs first if needed;
3. treat reduction/closure as structural projection artifacts;
4. do not assume weights/provenance survive petgraph's intermediate representation;
5. map reduced edges back to canonical domain IDs.

The base ontology generally requires reachability, not necessarily transitive-reduction edges.

---

## 65. Structural metric generation

### 65.1 Counts

Directly count:

```text
statement_count
expression_count
basic_block_count
cfg_edge_count
branch_count
return_count
raise_or_panic_count
read_count
write_count
parameter_count
generic_parameter_count
direct_call_count
unique_direct_callee_count
direct_caller_count
```

### 65.2 Cyclomatic complexity

Per connected CFG owner:

```text
M = E - N + 2P
```

where `P` is the number of connected components, normally one after adding entry/exit.

Alternative branch-count formulation MAY be stored only if labelled with method.

### 65.3 Loop metrics

Derived from loop regions:

```text
loop_count
maximum_loop_nesting_depth
```

No qualitative label is produced.

---

## 66. Interprocedural summary generation

### 66.1 Local summary

For each callable compute from local facts:

```text
direct callees
may callees
direct reads/writes
parameter reads/mutations
returns
allocation/deallocation
I/O/blocking
raise/panic/unwind
spawn/await
unsafe/FFI
unknown effect
```

### 66.2 Call SCC processing

1. build call graph including exact or selected may edges;
2. compute SCCs;
3. topologically process condensed SCC DAG;
4. within recursive SCC, iterate summaries to fixed point;
5. union callee summaries according to edge certainty policy.

### 66.3 Direct/transitive distinction

Store:

```text
DIRECT_EFFECT
TRANSITIVE_EFFECT
```

Never replace local facts with transitive summary facts.

### 66.4 Unknown propagation

If a callable may call an unknown target:

```text
unknown_effect = true
```

and transitive summaries SHALL not claim a closed complete effect set.

---

# Part VI — Complete Fact-Generation Matrix

## 67. Structural relationship generation

| Relationship | Python generation | Rust generation | Additional logic |
|---|---|---|---|
| `CONTAINS` | Ruff AST/semantic owners | Tree-sitter + rustc owners + MIR body containment | Canonical owner merge |
| `AST_CHILD` | Ruff AST; Tree-sitter raw CST | Tree-sitter Rust | Normalize field/ordinal |
| `ENCLOSES` | Range containment index | Range containment index | Derive only immediate or queried closure |
| `LEXICALLY_PRECEDES` | Ruff token/AST ordering | Tree-sitter leaf/source ordering | Sort by byte range |
| `DEFINED_IN` | Ruff declarations + semantic binding | rustc definition parent | Identity reconciliation |
| `OWNED_BY` | Scope/class/module owner | crate/module/impl/body owner | Canonical owner IDs |
| `HAS_SCOPE` | Ruff semantic/custom scope | rustc function/module plus source scope approximation | Separate semantic vs lexical scopes |
| `ENCLOSING_SCOPE` | Ruff scope parent | Source syntax/rustc owner | Normalize |


## 67A. Source and lexical relationship and typed-extension generation

| Relationship/fact | Python generation | Rust generation | Additional logic |
|---|---|---|---|
| `CONTAINS_SPAN` | Source file to token/comment/syntax ranges | Source file to CST/compiler ranges | Range validation |
| `TOKEN_OF` | Ruff parser token to source file/syntax occurrence | Tree-sitter leaf token or lexer token to file | Link by exact range |
| `LEXICALLY_PRECEDES` | Token/source ordering | CST leaf/source ordering | Emit immediate relation; closure query-time |
| `DOCUMENTS` | Ruff docstring helpers | Rust doc comments/attributes | Attach to nearest language-recognized declaration |
| `DIRECTIVE_APPLIES_TO` | Ruff pragma/type-comment association | Rust attribute/cfg attachment | Syntax-role-specific association |
| `syntax_detail.explicitly_parenthesized` | Ruff `ParenthesizedExpressions` | Tree-sitter punctuation/CST | Python source-layout fact on the reconciled syntax occurrence; not a relation kind |
| `source_annotation(PARSE_ERROR)` → `target_entity_id` | Ruff parse diagnostic + Tree-sitter error node | Tree-sitter error + rustc diagnostic evidence | Retain provider-specific evidence; `syntax_detail.error` is the occurrence flag |
| `source_annotation(MISSING_SYNTAX)` → `target_entity_id` | Tree-sitter missing node | Tree-sitter missing node | Zero-width expected-kind fact; `syntax_detail.missing` is the occurrence flag |

The last three rows are modeled typed-extension/annotation surfaces rather than
canonical relation kinds. Implementations SHALL NOT allocate duplicate
`EXPLICITLY_PARENTHESIZED`, `PARSE_ERROR_AT`, or `MISSING_AT` relation codes: the
closed relation registry remains the authority for relation-shaped facts.

## 68. Symbol and binding relationship generation

| Relationship | Python generation | Rust generation | Additional logic |
|---|---|---|---|
| `DECLARES` | Ruff AST + semantic model | rustc definitions | Merge source and semantic entities |
| `BINDS` | Ruff binding events | Pattern/local/parameter bindings from source/compiler | Normalize binding kinds |
| `REFERS_TO` | Ruff local + Pyrefly definitions | rustc resolved defs/types | Unknown if unresolved |
| `MAY_REFER_TO` | union/star/dynamic candidates | indirect/external candidate defs | Candidate set construction |
| `SHADOWS` | Ruff semantic | Rust lexical source analysis where meaningful | Python authoritative |
| `CAPTURES` | Ruff free/cell vars | closure capture/compiler facts | Capture mode normalization |
| `CAPTURED_FROM` | Ruff semantic owner | Rust source owner/place | — |
| `ALIASES` | import/type alias declarations | `use` aliases/type aliases | Distinguish name alias from memory alias |
| `REBINDS` | Ruff global/nonlocal/reassignment | Rust shadow/new binding generally separate | Python-specific semantics |

## 69. Module/dependency relationship generation

| Relationship | Python generation | Rust generation | Additional logic |
|---|---|---|---|
| `IMPORTS_MODULE` | Ruff import syntax + Pyrefly module resolver | rustc crate/module resolution + `use` syntax | External symbol nodes |
| `IMPORTS_SYMBOL` | Pyrefly/Glean/LSP + Ruff aliases | rustc resolved `use` | — |
| `EXPORTS` | Pyrefly exports + static `__all__` | Rust visibility/re-export | Public-surface rules |
| `REEXPORTS` | import binding exposed by module | `pub use` | — |
| `DEFINED_IN_MODULE` | Ruff semantic module owner | rustc parent module | — |
| `DEPENDS_ON_MODULE` | derive from imports/references | derive from use/type/call references | Preserve relation causes |

## 70. Type relationship generation

| Relationship | Python generation | Rust generation | Additional logic |
|---|---|---|---|
| `DECLARED_TYPE` | TSP + annotation syntax | rustc declarations | Structured type interning |
| `INFERRED_TYPE` | Pyrefly Query | rustc computed/compiler type | Provider naming normalized |
| `COMPUTED_TYPE` | Pyrefly occurrence type | MIR local/operand/place type | — |
| `EXPECTED_TYPE` | TSP or sidecar extension | rustc expected/coercion facts if exposed | Optional capability |
| `TYPE_OF` | Pyrefly/rustc | rustc | — |
| `PARAMETER_TYPE` | callable signature | rustc function signature | — |
| `RETURN_TYPE` | callable signature | rustc function signature | — |
| `FIELD_TYPE` | Pyrefly members | rustc ADT fields | — |
| `TYPE_PARAMETER_OF` | Ruff/Pyrefly | rustc generics | Identity per declaration |
| `TYPE_ARGUMENT` | Pyrefly structured types | rustc generic args | Ordered |
| `LIFETIME_ARGUMENT` | N/A | rustc generic args | Region abstraction |
| `CONST_ARGUMENT` | N/A or literal typing construct | rustc generic args | Normalize const |
| `SUBTYPE_OF` | targeted Pyrefly solver result | trait/nominal relationships where compiler exposes | Avoid all-pairs |
| `SUPERTYPE_OF` | reverse index | reverse index | Derived |
| `BOUNDED_BY` | Pyrefly type vars | Rust bounds | — |
| `CONSTRAINED_BY` | Pyrefly type vars | Rust where predicates | — |
| `OUTLIVES` | N/A | Rust lifetime/borrowck facts | private adapter if exact |
| `INSTANTIATES` | class/generic use | ADT/function instance | — |
| `SPECIALIZES` | overload/generic specialization where provider exposes | monomorphic instance | — |
| `SUBSTITUTES` | Pyrefly generic solution if exposed | rustc generic substitutions | — |
| `COERCES_TO` | Pyrefly computed/expected relation | rustc coercion facts | Do not infer merely from cast syntax |
| `CASTS_TO` | Ruff cast-like syntax/builtins where explicit | MIR cast | — |
| `NARROWS_TO` | declared vs occurrence computed type | CFG/discriminant/dataflow type state | Cause classification custom |

## 71. Member relationship generation

| Relationship | Python generation | Rust generation | Additional logic |
|---|---|---|---|
| `DECLARES_MEMBER` | Ruff class body + Pyrefly attributes | rustc ADT/trait/impl | — |
| `HAS_MEMBER` | effective member inventory | resolved ADT/trait members | MRO/impl expansion |
| `INHERITS` | bases resolved by Pyrefly | supertraits/nominal relationships where applicable | C3 MRO for Python |
| `IMPLEMENTS` | protocol/subtype query | impl graph | — |
| `IMPLEMENTS_TRAIT` | protocol model if explicit | rustc impl | — |
| `IMPLEMENTS_METHOD` | override/contract match | rustc impl item mapping | — |
| `OVERRIDES` | MRO name/member resolution | trait impl/inherent override-like mapping | Signature compatibility |
| `OVERRIDDEN_BY` | reverse edge | reverse edge | Derived |
| `RESOLVES_MEMBER` | Pyrefly member definition | rustc method/field resolution | — |
| `MAY_RESOLVE_MEMBER` | union/dynamic candidate set | dyn trait/ambiguous candidate set | Include unknown remainder |


## 71A. Python-specific object-model relationship generation

| Relationship | Generation |
|---|---|
| `MRO_PRECEDES` | Pyrefly MRO adapter when available; otherwise C3 linearization over resolved direct bases |
| `METACLASS_OF` | Pyrefly class type metadata plus Ruff `metaclass=` syntax |
| `DESCRIPTOR_FOR` | Pyrefly descriptor classification; decorator/source fallback |
| `PROPERTY_FOR` | Ruff `@property` syntax reconciled with Pyrefly member semantics |
| `GETTER_FOR` | Property getter declaration to property member |
| `SETTER_FOR` | `@x.setter` declaration to property member |
| `DELETER_FOR` | `@x.deleter` declaration to property member |
| `CLASS_METHOD_OF` | `@classmethod` syntax and Pyrefly call/member kind |
| `STATIC_METHOD_OF` | `@staticmethod` syntax and Pyrefly call/member kind |
| `RESOLVES_ATTRIBUTE` | Pyrefly member definition at access site |
| `MAY_RESOLVE_ATTRIBUTE` | Union/dynamic candidate set plus explicit unknown remainder |

## 71B. Rust-specific object-model relationship generation

| Relationship | Generation |
|---|---|
| `SUPERTRAIT` | rustc trait predicates |
| `INHERENT_IMPL_FOR` | rustc impl self type without trait ref |
| `TRAIT_IMPL_FOR` | rustc impl trait ref and self type |
| `ASSOCIATED_WITH` | associated const/type/function to trait or impl |
| `STATICALLY_RESOLVES_TO` | compiler-resolved trait/inherent method call |
| `UNSIZES_TO_DYN` | compiler coercion/unsizing facts |
| `USES_VTABLE` | dynamic call/vtable creation evidence |
| `MAY_DISPATCH_TO` | impl inventory, unsize origin, and receiver-flow candidates |

## 72. Invocation relationship generation

| Relationship | Python generation | Rust generation | Additional logic |
|---|---|---|---|
| `CONTAINS_CALL` | Ruff call AST owner | MIR/source call owner | — |
| `HAS_CALLEE_EXPRESSION` | Ruff call.func | MIR callable operand/source syntax | — |
| `HAS_RECEIVER` | attribute/bound syntax | method receiver argument/source | Normalize |
| `HAS_ARGUMENT` | Ruff ordered args/keywords | MIR argument operands | — |
| `ARGUMENT_BINDS_TO` | custom Python binder | Rust ABI/signature positional mapping | Dynamic splat unknown |
| `CALLS_DECLARATION` | Pyrefly target | MIR FnDef/trait contract | — |
| `CALLS_EXACT_TARGET` | Pyrefly exact target | resolved Instance/direct function | Certainty |
| `CALLS_INSTANCE` | generally N/A | Rust mono instance | — |
| `MAY_CALL` | union/candidate targets | fn pointer/dyn candidates | Conservative set |
| `CALLS_UNKNOWN` | unresolved dynamic call | unresolved indirect/FFI | Unknown sentinel |
| `REFERENCES_CALLABLE` | name/attribute as value | FnDef operand | — |
| `TAKES_FUNCTION_ADDRESS` | callable stored/passed | FnDef-to-FnPtr coercion | — |
| `PASSES_CALLABLE` | argument binder/type | MIR argument flow | — |
| `RETURNS_CALLABLE` | return value type/flow | MIR return flow | — |

## 73. Control-flow relationship generation

| Relationship | Python generation | Rust generation | Additional logic |
|---|---|---|---|
| `CFG_NEXT` | custom AST CFG | MIR successor/statement sequence | — |
| `CFG_TRUE` | branch/boolean short circuit | boolean `SwitchInt` | Preserve condition |
| `CFG_FALSE` | branch/boolean short circuit | boolean `SwitchInt` | — |
| `CFG_CASE` | match/switch | `SwitchInt` cases | Case/pattern label |
| `CFG_LOOP_BACK` | loop builder | derived back edge/MIR topology | — |
| `CFG_BREAK` | loop context | source correspondence if applicable | Python direct |
| `CFG_CONTINUE` | loop context | source correspondence if applicable | Python direct |
| `CFG_RETURN` | return statement | Return terminator | — |
| `CFG_EXCEPTION` | Python exception model | panic/exception abstraction if desired | Separate from unwind |
| `CFG_UNWIND` | finally/handler propagation model | MIR unwind | — |
| `CFG_CALL_RETURN` | call continuation | MIR call target block | — |

## 74. Dataflow relationship generation

| Relationship | Python generation | Rust generation | Additional logic |
|---|---|---|---|
| `DEFINES` | assignment/binding events | assignment/call destination/parameters | — |
| `USES` | name/attribute/subscript/use events | operands/rvalues/terminators | — |
| `REACHES` | reaching-def solver | reaching-def solver | Alias-aware kills |
| `DEF_USE` | derived from reaches | derived from reaches | — |
| `DATA_DEP` | use-to-def/value dependency | use-to-def/value dependency | — |
| `VALUE_FLOWS_TO` | expression/assignment/call mapping | operand/rvalue/call mapping | Interprocedural extension |
| `PRODUCES_VALUE` | expression | rvalue/operation | — |
| `CONSUMES_VALUE` | operation/use | statement/terminator | — |
| `OPERAND` | AST operation children | MIR operands | Ordered |
| `RESULT` | expression result | assignment destination/temp | — |

## 75. Memory relationship generation

| Relationship | Python generation | Rust generation | Additional logic |
|---|---|---|---|
| `READS` | use/access-path extractor | AccessEvent | — |
| `WRITES` | target/access-path extractor | AccessEvent | — |
| `MUTATES` | attribute/container/argument writes | mutable borrow/write | Alias/points-to |
| `INITIALIZES` | first/assignment defs | assignment/storage state | Dataflow |
| `DEINITIALIZES` | delete/rebinding abstractions | move/storage dead/drop | Language-specific |
| `TAKES_ADDRESS` | limited Python reflection/object refs | MIR address/ref | — |
| `DEREFERENCES` | Python object/member access abstraction | MIR projection | — |
| `MUST_ALIAS` | singleton equal points-to | proven singleton/compiler facts | Conservative |
| `MAY_ALIAS` | intersecting points-to | intersecting points-to | — |
| `DOES_NOT_ALIAS` | rarely proven | borrowck/type facts | Only exact |
| `POINTS_TO` | exact singleton points-to | exact reference/function ptr | — |
| `MAY_POINT_TO` | conservative set | conservative set | Unknown memory |

## 76. Ownership/lifetime relationship generation

| Relationship | Python generation | Rust generation | Additional logic |
|---|---|---|---|
| `OWNS` | resource/object ownership model only | ownership-state/places | Python modelled |
| `MOVED_TO` | not language-level move | MIR Move | — |
| `COPIED_TO` | assignment/value copy abstraction only if useful | MIR Copy | Do not equate Python assignment with copy |
| `BORROWS_SHARED` | N/A | MIR Ref | — |
| `BORROWS_MUTABLY` | N/A | MIR mutable Ref | — |
| `REBORROWS` | N/A | MIR Reborrow | — |
| `LOAN_CREATED_AT` | N/A | borrowck adapter | Optional |
| `LOAN_LIVE_AT` | N/A | borrowck adapter | Optional |
| `REGION_CONTAINS` | N/A | borrowck/region adapter | Optional |
| `OUTLIVES` | N/A | generic/borrowck facts | — |
| `DROPS` | explicit/modelled close only; no GC assumption | MIR Drop/drop glue | — |
| `DROPS_FIELD` | N/A | recursive drop glue | — |
| `TRANSFERS_RESOURCE` | points-to/argument model | moves/returns/model packs | — |
| `RELEASES_RESOURCE` | modeled close/with exit | drop/release model | — |

## 77. Effect relationship generation

| Relationship | Python generation | Rust generation | Additional logic |
|---|---|---|---|
| `READS_STATE` | local access summary | AccessEvent summary | Direct/transitive |
| `WRITES_STATE` | write summary | AccessEvent summary | — |
| `MUTATES_ARGUMENT` | points-to + writes | alias + writes/mut borrow | — |
| `ALLOCATES` | allocation-site syntax/model | aggregate/box/allocation models | Some effects modelled |
| `DEALLOCATES` | modelled only | drop/deallocation model | — |
| `MAY_RAISE` | explicit + model + unknown calls | N/A or error abstraction | — |
| `MAY_PANIC` | N/A | assert/panic/model/unknown | — |
| `MAY_UNWIND` | exception propagation | MIR unwind | — |
| `PERFORMS_IO` | API model packs | FFI/std API model packs | Modelled |
| `MAY_BLOCK` | API model packs | API/FFI model packs | Modelled |
| `SPAWNS_TASK` | asyncio model | runtime API model | Modelled |
| `SPAWNS_THREAD` | threading model | thread API model | Modelled |
| `AWAITS` | syntax/CFG | async/coroutine source/MIR | — |
| `ACQUIRES_LOCK` | model pack | lock/guard model | Modelled |
| `RELEASES_LOCK` | model pack | drop/guard model | Modelled |
| `CALLS_FOREIGN_CODE` | native extension/external model | foreign call | — |
| `USES_UNSAFE_OPERATION` | N/A | source/MIR | — |
| `USES_INLINE_ASSEMBLY` | N/A | MIR | — |


## 77A. Exceptional-flow relationship generation

| Relationship | Python generation | Rust generation | Additional logic |
|---|---|---|---|
| `RAISES` | Explicit `raise` and exactly modelled operation | Explicit panic/abort/assert category where exact | Exception/panic type node |
| `MAY_RAISE` | Call/API models and unknown calls | Generally N/A; use `MAY_PANIC` | Conservative |
| `MAY_PANIC` | N/A | MIR assert, panic calls, bounds/operation models | Unknown panic where opaque |
| `HANDLED_BY` | Exact handler match | Cleanup/catch abstraction only when represented | Ordered handler matching |
| `MAY_BE_HANDLED_BY` | Unknown/union exception candidates | Unwind cleanup candidates | Conservative |
| `PROPAGATES_TO` | Unhandled exception to enclosing callable/exit | Resume/unwind propagation | Interprocedural summary |
| `UNWINDS_TO` | Finally/handler exceptional edge | MIR unwind successor | — |
| `EXECUTES_CLEANUP` | `finally`, context-manager exit | cleanup block/drop glue | — |

## 77B. Resource-lifetime relationship generation

| Relationship | Python generation | Rust generation | Additional logic |
|---|---|---|---|
| `CREATES_RESOURCE` | Versioned API model for constructors/factories | Type/API model or compiler-known constructor | `MODELLED` unless compiler-direct |
| `ACQUIRES_RESOURCE` | Context manager/acquire API model | guard/lock/resource constructor model | — |
| `OWNS_RESOURCE` | Points-to and local/field ownership convention | Rust ownership/place state | Language-specific |
| `TRANSFERS_RESOURCE` | Assignment/argument/return points-to flow | Move/return/argument flow | — |
| `USES_RESOURCE` | Modelled method/call receiver use | Place/call use | — |
| `RELEASES_RESOURCE` | close/release/`__exit__` model | drop/release method/drop glue | — |
| `DROPS_RESOURCE` | No generic GC assertion | MIR drop of modelled resource type | — |

## 77C. Async and concurrency relationship generation

| Relationship | Python generation | Rust generation | Additional logic |
|---|---|---|---|
| `CREATES_FUTURE` | Calling resolved async function | Calling async fn / coroutine construction | Keep separate from body execution |
| `SPAWNS` | asyncio/thread/process model packs | Tokio/std thread/task model packs | Target callable/task relation |
| `AWAITS` | Ruff `Await` and CFG | source/MIR coroutine semantics | — |
| `YIELDS` | `yield`/`yield from` | coroutine/generator facts where applicable | — |
| `RESUMES` | CFG resume node | coroutine resume edges | — |
| `JOINS` | task/thread join model | join/await model | — |
| `SENDS` | queue/channel/socket model | channel model | — |
| `RECEIVES` | queue/channel/socket model | channel model | — |
| `ACQUIRES` | lock model | lock/guard model | — |
| `RELEASES` | lock model | guard drop/release model | — |
| `MAY_RUN_CONCURRENTLY_WITH` | Spawned task/thread lifetime overlap | task/thread lifetime overlap | Conservative interval relation |
| `HAPPENS_BEFORE` | await/join and guaranteed synchronization | await/join/channel/lock guarantees | Emit only when semantic guarantee is explicit |

## 77D. Closure and capture relationship generation

| Relationship | Python generation | Rust generation | Additional logic |
|---|---|---|---|
| `CAPTURES` | Ruff free/cell-variable semantics | compiler closure capture facts | — |
| `CAPTURED_FROM` | Binding owner/scope | source place/owner | — |
| `CAPTURES_BY_VALUE` | Only explicit copied/default-bound semantics when proven | Rust capture mode | Python ordinary capture is not by-value |
| `CAPTURES_BY_REFERENCE` | Python closure cell | Rust shared/reference capture | — |
| `CAPTURES_MUTABLY` | Nonlocal/cell mutation fact | Rust mutable capture | — |

## 77E. Program-point state relationship generation

| Fact | Python generation | Rust generation | Additional logic |
|---|---|---|---|
| `INITIALIZED_AT` | Reaching-def and assignment state | Ownership/init dataflow | Program point required |
| `UNINITIALIZED_AT` | Definite absence/unbound state | Move/init dataflow | Exact only |
| `MAY_BE_UNINITIALIZED_AT` | Branch merge or unresolved binding | Lattice join | Conservative |
| `KNOWN_CONSTANT_AT` | Literal/constant propagation | MIR const/CTFE/dataflow | Optional bounded propagation |
| `POSSIBLE_CONSTANT_SET` | Branch-merged literal set | Switch/const propagation | Bounded set; widen otherwise |
| `NULL_AT` | Precise `None` state | Niche/option state only if modelled | Language-specific |
| `NON_NULL_AT` | Pyrefly narrowing/CFG | Reference/non-null type fact where exact | — |
| `MAY_BE_NULL_AT` | Union/flow state | Option/raw pointer state if modelled | — |
| `VARIANT_AT` | Match/type narrowing | MIR discriminant dataflow | — |
| `POSSIBLE_VARIANTS_AT` | Match/union flow | discriminant dataflow | — |

## 78. Generated/lowered relationship generation

| Relationship | Python generation | Rust generation | Additional logic |
|---|---|---|---|
| `GENERATED_FROM` | Pyrefly synthesized/framework member | macro/compiler-generated entity | Provider/model provenance |
| `EXPANDED_FROM` | decorator/framework model where explicit | macro expansion | — |
| `EXPANDS_TO` | framework model only | macro invocation to generated items | — |
| `LOWERS_TO` | source async/generator to semantic object where modelled | source definition to MIR/coroutine | Span reconciliation |
| `CORRESPONDS_TO` | TS CST ↔ Ruff AST ↔ semantic entities | TS syntax ↔ rustc/MIR | Range/kind matching |
| `MONOMORPHIZES` | N/A | Instance to generic definition | — |
| `SPECIALIZES` | overload/generic solution if exposed | Rust instance | — |

## 79. Derived graph relationship generation

| Relationship | Projection/algorithm |
|---|---|
| `TRANSITIVELY_REACHES` | DFS/BFS or SCC-condensed DAG closure |
| `TRANSITIVELY_REACHED_BY` | Reverse projection traversal |
| `DOMINATES` | petgraph `dominators::simple_fast` plus tree closure |
| `STRICTLY_DOMINATES` | `DOMINATES` excluding self |
| `IMMEDIATE_DOMINATOR` | petgraph dominator result |
| `POST_DOMINATES` | Dominators on reversed CFG with synthetic exit |
| `IMMEDIATE_POST_DOMINATOR` | Reversed-dominator result |
| `CONTROL_DEPENDENT_ON` | Custom post-dominator frontier algorithm |
| `BACK_EDGE` | CFG edge whose target dominates source |
| `LOOP_MEMBER` | Natural-loop reverse traversal; SCC fallback |
| `DIRECT_CALLER/CALLEE` | Projection of call-site target edges |
| `TRANSITIVE_CALLER/CALLEE` | Traversal over selected call projection |
| SCC/recursion facts | `kosaraju_scc`/`tarjan_scc` |
| connected component | `connected_components` or custom directed policy |
| shortest distance | BFS or unit-cost Dijkstra |
| structural metrics | Counts/formulas over canonical facts |

---

# Part VII — Reconciliation and Unknown Semantics

## 80. Range reconciliation algorithm

For a provider fact at source range `R`:

1. exact range + expected normalized kind;
2. exact name subrange inside declaration;
3. smallest enclosing compatible expression/declaration;
4. same start with compatible end/kind;
5. provider-only synthetic occurrence node.

The reconciler SHALL never attach a semantic fact to an arbitrary overlapping syntax node.

---

## 81. Declaration reconciliation

Merge declaration candidates by:

```text
file
semantic owner
declaration kind
name
name span
full declaration span
qualified name where available
```

If Ruff and Pyrefly disagree:

- retain Ruff source declaration;
- retain Pyrefly semantic target as evidence;
- emit conflict diagnostic;
- avoid creating duplicate canonical declarations unless they denote distinct source/stub entities.

---

## 82. Type reconciliation

Multiple types may legitimately coexist:

```text
declared
computed
expected
narrowed
```

Conflicts within the same category SHALL preserve producer evidence and choose according to authority.

---

## 83. Call-target reconciliation

For one call site, target edges are partitioned:

```text
exact targets
sound/conservative may targets
modelled targets
unknown remainder
```

An exact target does not automatically eliminate an unknown remainder when dynamic semantics may alter dispatch.

---

## 84. Explicit unknown-materialization rules

| Condition | Required unknown |
|---|---|
| unresolved identifier | `UNKNOWN_SYMBOL` |
| missing/failed type fact | `UNKNOWN_TYPE` with reason |
| unresolved call | `UNKNOWN_CALL_TARGET` |
| dynamic attribute | `UNKNOWN_MEMBER` |
| unresolved import | `UNKNOWN_MODULE` |
| opaque alias/pointer/object | `UNKNOWN_MEMORY` |
| unresolved/opaque call effects | `UNKNOWN_EFFECT` |
| dynamic trait open set | `UNKNOWN_EXTERNAL_IMPLEMENTATION` |

Missing provider output SHALL NOT by itself prove absence.

---

## 85. Capability reporting

Provider output SHALL report the canonical owner-capability state and completeness separately.

```text
owner_capability_state:
  CURRENT
  PENDING
  INVALIDATED
  PARTIAL
  UNAVAILABLE_PARSE
  UNAVAILABLE_COMPILE
  UNAVAILABLE_PROVIDER
  UNAVAILABLE_DERIVATION
  EXCLUDED
  UNSUPPORTED
  REMOVED
  NOT_APPLICABLE

completeness_state:
  COMPLETE
  PARTIAL
  INDETERMINATE
  UNAVAILABLE
  NOT_APPLICABLE
```

Each record SHALL include:

```text
workspace_id
analysis_context_id
owner_id
capability_code
source_generation
input snapshot/digest
provider_run_id
reason_code
diagnostic_id optional
fallback_source_available
```

Provider-run state is separate and uses the ontology 1.3 `ProviderRunState` registry: `QUEUED`, `RUNNING`, `SUCCEEDED`, `PARTIAL`, `FAILED`, `TIMED_OUT`, `CANCELLED`, `SUPERSEDED`, `CRASHED`, `PROTOCOL_ERROR`, `STALE_RESULT`, or `STALE_GIT_BASELINE`. The lifecycle and query layers aggregate these records; they SHALL not invent a second provider-run or capability vocabulary.

# Part VIII — Handoff to Reconciliation, Derivation, and Lifecycle

## 86. Generation output boundary

This specification ends at validated normalized provider-observation Arrow streams and provider capability manifests.

The data-fabric `ReconciliationEngine` is the only component that:

- selects canonical observations under provider authority rules;
- creates canonical entity, relation, and property facts;
- assigns canonical fact IDs and property-level provenance;
- collapses duplicates and retains conflicts;
- creates owner-scoped replacement batches.

The lifecycle coordinator schedules jobs and enforces source/context/generation fences, but it does not reconcile facts.

---

## 87. Derived-analysis boundary

Derived families are not independently selected in this document. The data-fabric `DerivationRegistry` assigns one implementation and precision profile per family.

Provider adapters MAY emit direct facts required by a derivation. Petgraph/custom solvers/DataFusion operators MAY be used only through the registered derivation implementation. A generation adapter SHALL not materialize a competing SCC, dominance, points-to, reaching-definition, or summary fact family.

---

## 88. Activation boundary

The lifecycle and fabric specifications own hot and durable activation:

```text
provider observations
  → canonical reconciliation
  → registered derivations
  → owner/table validation
  → immutable hot overlay
  → ServingSnapshot activation
  → asynchronous durable publication
```

Generation completion does not imply query visibility. A batch becomes queryable only through an activated `ServingSnapshot` that pins its workspace, source generation, analysis-context set, capability index, and contract bundles.

# Part IX — Rust Workspace Architecture

## 89. Recommended crates

```text
cpg-schema/
    canonical nodes, edges, metadata, enums, DTOs

source-snapshot/
    immutable bytes, paths, digests, line/position conversion

syntax-tree-sitter/
    language registry, parser pools, CST normalization, queries

python-ruff-frontend/
    Ruff parse, AST extraction, trivia/index, local semantic adapter

python-pyrefly-protocol/
    stable DTO definitions

python-pyrefly-sidecar/
    pinned Pyrefly adapter and process server

python-cfg/
    Python evaluation-order CFG builder

rust-mir-protocol/
    owned extraction records

rust-mir-driver/
    rustc wrapper and rustc_public callback

rust-private-adapter/
    stable IDs, spans, borrowck/vtable escape hatches

cpg-reconcile/
    data-fabric-owned ReconciliationEngine implementation; identity, range matching, provider authority/conflict handling

cpg-analysis/
    DerivationRegistry implementations selected by the data-fabric contract; projection builders and registered analyses

cpg-models/
    library/API effect and resource model packs

cpg-store/
    current-state transactional persistence

cpg-service/
    orchestration and agent-facing fact query API
```

---

## 90. Provider job interfaces

Provider execution is asynchronous and accepted-handle based.

```rust
#[async_trait::async_trait]
pub trait ProviderJobFactory: Send + Sync {
    fn capabilities(&self) -> CapabilitySet;
    async fn start(
        &self,
        request: ExtractRequest,
    ) -> Result<Box<dyn ProviderJob>, ProviderError>;
}

#[async_trait::async_trait]
pub trait ProviderJob: Send {
    fn accepted(&self) -> &ProviderAccepted;
    async fn next_event(&mut self) -> Result<Option<ProviderEvent>, ProviderError>;
    async fn cancel(&mut self) -> Result<CancelAcknowledgement, ProviderError>;
}

pub enum ProviderEvent {
    ObservationChunk(ProviderObservationChunk),
    ArrowIpcChunk(ArrowObservationChunk),
    Diagnostic(ProviderDiagnostic),
    Progress(ProviderProgress),
    Completed(ProviderCompletedManifest),
    Failed(ProviderFailure),
}
```

The 1.3 wire packages and services are fixed:

| Contract source | Protobuf package | Service |
|---|---|---|
| `provider_control.proto` | `codefabric.provider.v1` | `ProviderControl` |
| `pyrefly_sidecar.proto` | `codefabric.pyrefly.v1` | `PyreflySidecar` |
| `rustc_extractor.proto` | `codefabric.rustc.v1` | `RustcExtractor` |

Wire events map to the application-owned interface rather than creating a
second event taxonomy. `RunAccepted`/`CompilationAccepted` construct
`ProviderAccepted`; progress and begin/end markers map to `Progress` with a
typed phase; observation chunks map to `ObservationChunk` or `ArrowIpcChunk`
according to payload kind; diagnostics map to `Diagnostic`; successful terminal
records map to `Completed`; failed, cancelled, stale, or protocol-invalid
terminal records map to `Failed` with the canonical failure code. The complete
mapping table is a required part of `feature-registry.yaml` and is verifier
checked against these variants.

Requirements:

- `ProviderAccepted` is available before long-running work;
- every event carries workspace, context, source generation, provider run, and input fingerprint;
- local CPU providers run on bounded Rayon capacity and stream through bounded channels;
- Pyrefly/rustc providers run in isolated processes with deadlines, backpressure, cancellation, and terminal manifests;
- superseded output is discarded by generation fence even if cancellation is not immediate;
- provider-specific types never cross the interface.

The reconciler and derivation interfaces are defined by the data-fabric specification and SHALL not be redefined here.

---

## 91. Canonical graph-projection DTO

The application projection representation is library-neutral:

```rust
pub struct GraphProjectionDto {
    pub projection_id: ProjectionId,
    pub workspace_id: WorkspaceId,
    pub analysis_context_id: AnalysisContextId,
    pub node_ids: std::sync::Arc<[DomainNodeId]>,
    pub row_offsets: std::sync::Arc<[u64]>,
    pub target_indices: std::sync::Arc<[u32]>,
    pub edge_fact_ids: std::sync::Arc<[FactId]>,
    pub edge_kind_codes: std::sync::Arc<[u32]>,
    pub projection_fingerprint: [u8; 32],
}
```

Nodes are sorted by canonical domain ID; outgoing edges are sorted by target ID, kind, and fact ID. Converters MAY build:

- an ephemeral `petgraph::DiGraph` for owner-local algorithms;
- Arrow/CSR arrays for DataFusion/custom physical operators.

Petgraph indexes and CSR integer positions are ephemeral and never replace canonical IDs. Derived facts retain supporting canonical fact IDs.

## 92. Model-pack interface

```rust
pub trait SemanticModelPack {
    fn match_callable(&self, qname: &str, type_info: Option<TypeId>) -> Option<CallableModel>;
    fn match_type(&self, qname: &str) -> Option<TypeModel>;
}
```

Model outputs are objective semantic facts and SHALL be tagged `MODELLED`.

---

# Part X — Validation and Conformance

## 93. Provider fixture requirements

### 93.1 Python fixtures

Cover:

```text
all statement/expression/pattern/type-parameter forms
scope kinds
global/nonlocal/shadow/capture
imports/re-exports/star import
decorators
properties/descriptors/class/static methods
MRO and multiple inheritance
overloads/generics/protocols/TypedDict
union dispatch
dynamic getattr/setattr/eval/exec
try/except/else/finally/except*
with/async with
match patterns and guards
comprehensions
yield/yield from/await
calls with all argument forms
```

### 93.2 Rust fixtures

Cover:

```text
items/traits/impls/generics/lifetimes
macros
MIR statement/terminator/rvalue variants
places and every projection
moves/copies/borrows/reborrows
static and dynamic dispatch
function pointers and closures
monomorphization
drop glue
async/coroutine
panic/unwind/assert
const/static/TLS
unsafe/raw pointers/FFI/inline asm
```

---

## 94. Differential validation

Required cross-provider checks:

```text
Tree-sitter Python declarations vs Ruff AST declarations
Ruff identifier occurrences vs Ruff semantic references
Ruff/Pyrefly source range agreement
Ruff qualified names vs Pyrefly resolved targets
Pyrefly Query type occurrences vs TSP spot checks
Pyrefly call targets vs LSP call hierarchy spot checks
Tree-sitter Rust items vs rustc definitions
Tree-sitter source calls vs MIR call-site correspondence
MIR CFG vs emitted CFG projection
Incremental/current parse vs clean full parse equivalence where applicable
```

---

## 95. Algorithm validation

### 95.1 CFG

- all edges valid;
- entry/exit semantics;
- no unintended fallthrough;
- exception/finally routing;
- MIR successor parity.

### 95.2 Dominance

Cross-check small fixtures by brute-force path enumeration.

### 95.3 Reaching definitions/liveness

Cross-check against hand-authored expected sets and randomized tiny CFG solvers.

### 95.4 Alias/points-to

Test monotonicity, conservative widening, and unknown barriers.

### 95.5 Summaries

Verify:

```text
direct facts remain direct
transitive summaries reach fixed point
recursive SCCs converge
unknown effects propagate
```

---

## 96. Canonical invariants

A conforming implementation SHALL enforce:

```text
all edge endpoints exist
all source spans are valid current-byte ranges
syntax occurrence != semantic entity
call site != callable
type syntax != type entity
value != memory location
read != write
move != copy
borrow != raw address
normal edge != unwind edge
direct effect != transitive effect
unknown != absent
raw provider kind remains recoverable
provider-local ID is never canonical identity
derived fact identifies derivation and source projection
```

---

## 97. Capability gaps and required treatment

### 97.1 Python gaps

| Gap | Treatment |
|---|---|
| Exact whole-program alias analysis | Conservative allocation-site analysis |
| Complete dynamic call resolution | Candidate edges plus unknown remainder |
| Bulk expected types in Query | TSP or custom sidecar extension |
| Complete inherited member inventory | Pyrefly adapter or C3 + member resolution |
| Exact exception sets | API models plus unknown exception |
| Exact resource/concurrency semantics | Versioned model packs |
| Complete framework-generated code | Provider/modelled synthesized facts plus unknowns |
| Runtime monkey patching | Observable write fact plus unknown member/call effects |

### 97.2 Rust gaps

| Gap | Treatment |
|---|---|
| Stable IDs not public enough | Narrow rustc_private adapter or application key |
| Exact byte/macro provenance | SourceMap/hygiene private adapter |
| Exact borrowck loans/regions | Private adapter; otherwise conservative overlay |
| Complete raw-pointer alias analysis | Conservative points-to plus unknown memory |
| Exact vtable candidate inventory | Private adapter or impl/unsize overapproximation |
| Whole-program external bodies | External symbol nodes and unknown effects |
| Compiler API/nightly drift | Isolated adapter and exhaustive variant tests |

### 97.3 Petgraph gaps

| Gap | Treatment |
|---|---|
| No reaching-def/liveness engine | Implement custom worklist framework |
| No points-to solver | Implement custom monotone constraint solver |
| No control-dependence API | Derive from post-dominators |
| `simple_fast` dominators scalability limits | Replace behind analysis trait if needed |
| No persistent graph database | Keep canonical storage separate |
| DAG-only transitive reduction | Condense SCCs or use traversal |

No gap SHALL be hidden by silently omitting facts.

---

# Part XI — Implementation Sequence

## 98. Phase 1 — Source and syntax completeness

Implement:

```text
source snapshots
Tree-sitter Python/Rust raw CST
Ruff Python AST/tokens/trivia/index
canonical source spans
syntax nodes and AST_CHILD
parse errors/missing nodes
declarations and call-site syntax
```

## 99. Phase 2 — Semantic identity and types

Implement:

```text
Ruff semantic adapter
Pyrefly sidecar and type table
Pyrefly callees/members/imports
rustc definitions/types/generics/traits
canonical identity/type interning
unknown symbol/type/call facts
```

## 100. Phase 3 — CFG and access events

Implement:

```text
Python CFG builder
MIR CFG extraction
Python read/write/access paths
Rust AccessEvent stream
normal/exceptional/unwind edges
```

## 101. Phase 4 — Dataflow and ownership

Implement:

```text
definition/use events
reaching definitions
def-use
liveness
Python points-to/alias
Rust move/init state
Rust borrow facts available from MIR/private adapter
```

## 102. Phase 5 — Derived graph facts

Implement:

```text
reachability
SCCs and recursion
dominance/post-dominance
control dependence
loops
components
structural metrics
```

## 103. Phase 6 — Effects and summaries

Implement:

```text
direct effect extraction
model packs
resource/concurrency facts
local summaries
interprocedural SCC fixed point
unknown effect propagation
```

## 104. Phase 7 — Full conformance

Complete:

```text
all ontology relationships
provider capability reporting
gap/unknown semantics
differential fixtures
current-state atomic publication
agent-facing fact retrieval
```

---

# CodeFabric 1.3 architecture-completion contracts

The standalone architecture-completion specification has been propagated into its permanent owners. This part contains the full normative contracts owned by this document: `G-14`, `G-30`, `G-31`, `G-32`, `G-33`, `G-34`, `G-35`, `G-36`, `G-38`, `G-39`, `G-40`, `G-43`. References to a gap ID elsewhere in the synchronized suite resolve to these sections.

## AC-G-14 — Analysis-context discovery, identity, and selection
### Decision

Analysis contexts are immutable, explicit manifests. They describe the semantic inputs used by providers but are operational metadata rather than environment facts in the CPG ontology.

### Contract

#### Python context

A Python context manifest contains:

```yaml
context_kind: python
python_language_version: "3.<minor>"
implementation_profile: cpython-semantics
platform_tag: normalized target platform
module_roots: ordered workspace-relative path IDs
source_roots: ordered workspace-relative path IDs
stub_roots: ordered authorized source/blob references
dependency_roots: ordered authorized endpoint/body references
namespace_package_policy: pep420
import_precedence: exact ordered rule set
typeshed_bundle_digest: b3:...
lockfile_artifacts: ordered file IDs and digests
project_config_artifacts: ordered file IDs and digests
pyrefly_bundle_digest: b3:...
ruff_bundle_digest: b3:...
enabled_model_packs: ordered pack IDs/digests
```

The daemon SHALL NOT infer semantics from whichever interpreter or virtual environment happens to launch it. Registration must resolve a deterministic language version. Precedence is:

1. explicit workspace profile;
2. supported exact version in project type-checker configuration;
3. a unique version derivable from `requires-python`;
4. deployment-profile default, accompanied by `CONTEXT_DEFAULTED` diagnostic.

#### Rust context

A Rust context manifest contains:

```yaml
context_kind: rust
cargo_workspace_root: authorized path ID
package_id: canonical Cargo package identity
target_name: exact Cargo target name
target_kind: lib | bin | proc-macro | test | example | bench
feature_set: sorted exact feature names
no_default_features: boolean
cfg_set: sorted normalized cfg expressions
target_triple: exact triple
analysis_profile: codefabric-analysis
rust_toolchain: exact rustc version and commit hash
rustc_extractor_digest: b3:...
cargo_metadata_digest: b3:...
cargo_lock_digest: optional b3:...
cargo_config_digests: ordered
build_script_output_digest: optional
proc_macro_bundle_digest: optional
enabled_model_packs: ordered pack IDs/digests
```

The default local profile indexes Rust library, binary, and proc-macro targets under default features. Test/example/bench targets receive source/syntax facts but compiler-semantic contexts are opt-in.

#### Context identity

```text
analysis_context_id = BLAKE3_128(CBEF-v1(
  domain = ANALYSIS_CONTEXT,
  workspace_id,
  context_kind,
  canonical manifest excluding display labels
))
```

A context is immutable. Any compatibility-sensitive field change creates a new ID and invalidates dependent facts. A `context_set_id` hashes the ordered context IDs and their manifest digests. The snapshot designates one default Python and one default Rust context where present.
## AC-G-30 — Pyrefly sidecar wire protocol
### Decision

Pyrefly runs as a daemon-managed Rust sidecar isolated from the main process. Control messages use length-delimited Protobuf over a private Unix-domain socket; high-volume normalized observations use Arrow IPC stream payloads referenced by protocol events.

### Contract

The sidecar protocol service is logically:

```text
Handshake
OpenContext
AnalyzeModules
CancelRun
CloseContext
Shutdown
```

The corresponding Protobuf package is `codefabric.pyrefly.v1` and the service
name is `PyreflySidecar`; aliases or adapter-local package names are prohibited.

The protocol starts with `Hello`/`HelloAck` containing:

```text
protocol major/minor and feature bits
sidecar build and Pyrefly source digest
supported Python versions
supported observation schema digests
maximum frame and Arrow chunk sizes
sandbox profile digest
```

`OpenContext` receives the complete immutable Python context manifest and a `SourceSnapshotLease`. It returns a sidecar-local context handle that is never persisted as canonical identity.

`AnalyzeModules` includes:

```text
provider_run_id
workspace_id
analysis_context_id
context_manifest_digest
source_generation
source_snapshot_lease_id
ordered module requests
requested capability codes
deadline
output schema bundle digest
```

Each module request identifies module name, file ID, source digest, source blob reference, and dependency/module-resolution generation.

The analysis channel is bidirectional. Observation chunks are credit-controlled: the daemon initially grants four chunk credits and no more than 16 MiB of unacknowledged payload; after schema/digest validation it returns `ChunkAccepted(sequence, next_credit_bytes)` or `ChunkRejected(sequence, error_code)`. The sidecar SHALL not emit beyond granted credits.

The event stream is strictly:

```text
RunAccepted
RunProgress*
ModuleBegin
ObservationBatchChunk*
ModuleEnd
... repeated modules ...
RunTerminal
```

Every event carries `provider_run_id`, workspace/context IDs, source generation, monotonically increasing sequence number, and context/source manifest digests. `ObservationBatchChunk` carries:

```text
module_id
observation_family_code
Arrow IPC bytes or immutable payload reference
schema_digest
row_count
chunk_digest
```

`ModuleEnd` supplies per-family counts and a module digest. `RunTerminal` supplies the ordered module digest list, capability outcomes, overall digest, and terminal state.

Rules:

- stdout is unused for protocol and MAY be closed; stderr is diagnostic-only and size-limited;
- source bytes are read only through the immutable snapshot lease, never from mutable workspace paths;
- response-local Pyrefly type-table indexes are resolved into CPG-owned DTOs before emission;
- partial module chunks are not accepted unless a valid `ModuleEnd` and terminal `RunTerminal` cover them;
- output is staged until terminal verification;
- cancellation is idempotent and must produce `CancelAcknowledged` followed by terminal `CANCELLED` or `ALREADY_TERMINAL`;
- a sidecar crash rejects the entire active run, restarts the process with exponential backoff, and requires context reopen;
- a newer generation supersedes the run; completed stale output is discarded;
- protocol/schema/bundle mismatch fails before analysis.

One sidecar process MAY host multiple read-only contexts if its negotiated memory profile permits, but it SHALL serialize context mutation and use bounded per-context analysis concurrency.
## AC-G-31 — rustc extractor protocol
### Decision

The Rust extractor communicates over a dedicated inherited file descriptor or private Unix socket. It never writes extraction data to rustc stdout or stderr. The protocol is compilation-unit and owner-manifest based. The daemon is the long-lived transport server; the compiler wrapper is a short-lived client for one Cargo-selected compilation unit.

The corresponding Protobuf package is `codefabric.rustc.v1` and the service
name is `RustcExtractor`; aliases or wrapper-local package names are prohibited. The
service name identifies the protocol domain, not the hosting process. Its bidirectional
`Observe` method receives extractor events and returns daemon control/credit messages.
The pre-production client-streaming `Extract` shape is replaced rather than retained as
a second authority.

### Contract

The daemon constructs the immutable `ProviderWorkspaceView`, then launches the pinned
Cargo command with `RUSTC_WORKSPACE_WRAPPER` set to the CodeFabric extractor wrapper.
Cargo supplies the real rustc executable as wrapper argument zero and preserves the
complete compiler argument vector after it. `RUSTC_WRAPPER` remains available for the
repository's ordinary build cache; provider analysis explicitly disables that outer
wrapper and uses a sandbox-private target directory so a compiler-cache hit cannot skip
the required extraction callback. Provider observation reuse is owned by CodeFabric's
content-addressed provider cache, not by sccache.

Cargo launches the compiler wrapper with:

```text
CODEFABRIC_EXTRACTOR_ENDPOINT
CODEFABRIC_PROVIDER_RUN_ID
CODEFABRIC_WORKSPACE_ID
CODEFABRIC_ANALYSIS_CONTEXT_ID
CODEFABRIC_SOURCE_GENERATION
CODEFABRIC_CONTEXT_MANIFEST_DIGEST
CODEFABRIC_PROVIDER_RESOURCE_PROFILE_ID
```

The wrapper passes probe/version invocations through unchanged. For an analysis
invocation it calls the real compiler through the pinned `rustc_public` callback,
preserves the compiler's current directory, arguments, stdout, stderr, and exit status,
and sends only owned DTOs to the daemon-hosted endpoint. The daemon sends
`CompilationAccepted`, chunk acknowledgements/rejections, cancellation, and terminal
protocol decisions on the reverse stream. The wrapper sends `CompilationBegin`, owner
events, observation chunks, diagnostics, and `CompilationEnd` on the event stream.

The event sequence is:

```text
CompilationAccepted
CompilationBegin
OwnerBegin
OwnerObservationChunk*
OwnerEnd
... repeated owners ...
CompilationEnd
```

`CompilationBegin` contains:

```text
provider_run_id
compilation_unit_id
workspace/context/source generation
package and target identity
crate name/type/disambiguator
exact rustc version/commit
normalized rustc invocation digest
Cargo metadata/lock/config digests
build-script/proc-macro output digests
source snapshot manifest digest
requested capability set
```

`OwnerBegin` identifies the canonical compiler owner key, source correspondence, owner kind, and expected observation families. Chunks are Arrow IPC normalized observations with sequence, schema, row count, and digest. `OwnerEnd` closes the owner with family counts and an owner content digest.

`CompilationEnd` contains compiler exit status, closed-owner set digest, capability outcomes, diagnostic summary, and overall stream digest.

Acceptance rules:

1. No compiler-owned object leaves the rustc callback; all records are owned DTOs before framing.
2. The default 1.x profile accepts rustc semantic output only when `CompilationEnd` reports compiler success and every referenced owner is closed.
3. Compilation failure rejects all new compiler-semantic chunks for activation. Current source/syntax may still publish with `UNAVAILABLE_COMPILE` capability.
4. A future partial-compilation profile requires a new protocol feature and is disabled by default.
5. An unexpected owner, duplicate sequence, missing end record, count mismatch, stale source/context digest, or protocol EOF is `PROTOCOL_ERROR` and rejects the run.
6. Wrapper probe/version invocations pass through without extraction and without altering stdout.
7. Cargo/rustc stdout and stderr semantics remain unchanged; extraction diagnostics go to the dedicated channel and daemon trace.
8. Backpressure is provided by bounded socket buffers and explicit `ChunkAccepted`/`ChunkRejected` acknowledgements. The daemon grants at most four outstanding chunks and 16 MiB of unacknowledged payload per compilation. The wrapper SHALL not send beyond available credit; a blocked channel cooperatively yields, and exceeding the provider deadline terminates the compiler process group.
9. Cargo/rustc current directory, manifest paths, module files, and registered configuration paths SHALL resolve inside the immutable `ProviderWorkspaceView`; `CARGO_HOME`, target/output directories, and temporary directories are sandbox-private, offline, and quota-bounded.
10. The wrapper refuses extraction unless the daemon supplies an accepted resource
    profile and the effective trust profile authorizes compiler execution. A transport
    connection alone is never authorization.

`compilation_unit_id` is canonical over workspace, Rust context, package, target, crate type, and compiler invocation profile. Session-local `CrateNum` and `DefId` never become persistent IDs.
## AC-G-32 — Common asynchronous provider execution interface
### Decision

All providers implement one accepted-handle asynchronous job model. In-process parser calls, sidecars, and compiler processes differ in placement but not lifecycle semantics.

### Contract

Conceptual Rust interface:

```rust
#[async_trait]
trait ProviderExecutor {
    async fn submit(&self, spec: ProviderJobSpec) -> Result<AcceptedProviderJob>;
    async fn cancel(&self, run_id: ProviderRunId, reason: CancelReason)
        -> Result<CancelAcknowledgement>;
}

struct AcceptedProviderJob {
    run_id: ProviderRunId,
    accepted_generation: i64,
    events: Pin<Box<dyn Stream<Item = Result<ProviderEvent>> + Send>>,
}
```

`ProviderJobSpec` includes workspace/context/source IDs, source snapshot lease, requested capabilities, owner/module/build-unit scope, priority class, resource estimate, deadline, supersession key, required bundle/schema digests, and a required `resource_profile_id` resolved through the provider registry.

Placement policy:

| Provider/work | Placement |
|---|---|
| source hashing, I/O, protocol, scheduling | Tokio async runtime |
| Tree-sitter/Ruff parsing and owner-local deterministic CPU analysis | bounded dedicated Rayon pools |
| Pyrefly | sandboxed sidecar process |
| rustc/MIR | sandboxed compiler process group |
| DataFusion derivation | DataFusion execution pools with daemon cancellation token |

Each provider resource profile is a closed registry record containing:

```text
resource_profile_id
provider_id and supported capability set
maximum source bytes
maximum parse/query work and wall time
maximum visited nodes and traversal depth
maximum emitted rows and bytes
maximum diagnostics
maximum parser workers
maximum simultaneously retained incremental tree revisions
cancellation check interval and hard-stop policy
retry policy and bounded retry budget
```

The daemon resolves the profile before admission, validates the request estimate against
it, and passes the exact profile ID to the provider. Tree-sitter parses use progress
callbacks/timeouts and parser reset; queries use match limits and bounded captures;
Ruff traversals check the shared cancellation token at the registered interval. Rayon
pools have an explicit fixed width and are not the global pool. A limit outcome emits a
typed terminal capability state and bounded diagnostic rather than partial unframed rows.

Admission occurs before `RUNNING`. Each provider has global, workspace, and context permits. Submission returns an accepted handle before waiting for permits or freshness-dependent execution.

Supersession key is `(workspace, context, provider, scope, capability family)`. A newer source/context generation marks older queued work `SUPERSEDED`; running work is cooperatively cancelled when economical, otherwise allowed to finish and then rejected as stale.

Cancellation acknowledgement has a deadline of 2 seconds for in-process/sidecar jobs and 10 seconds before forced process-group termination for compiler jobs. Output after acknowledgement is ignored.

No provider may directly mutate canonical tables, the active overlay, or capability state. It emits staged observations and terminal capability evidence only.
## AC-G-33 — Immutable source snapshot transport
### Decision

The daemon owns one content-addressed immutable source-image store per active generation. In-process consumers share `Arc` bytes; out-of-process providers receive read-only blob references under a snapshot lease.

### Contract

`SourceImage` contains:

```text
workspace_id
source_generation
file_id
WorkspacePath
language code
byte_length
BLAKE3-256 content digest
blob reference
encoding classification
newline profile
line-index digest and format version
filesystem metadata needed for stable-read audit
```

Stable read algorithm:

1. open securely beneath the authorized root;
2. capture metadata `M0`;
3. read exact bytes with size limit and compute digest `D0`;
4. capture metadata `M1`, rewind the same descriptor, reread exact bytes, compute digest `D1`, and capture metadata `M2`;
5. reject/retry unless `M0 == M1 == M2` for identity, size, mtime, and relevant change token and `D0 == D1` byte-for-byte;
6. adopt `D0` as the source digest;
7. store immutable blob using temp-write, `fsync`, mode `0400`, atomic rename;
8. build line-index artifact and manifest;
9. issue a snapshot lease.

Blob path names are content hashes, not source paths. The provider sandbox receives read-only access only to the leased blob directory and context artifacts.

Providers that require ordinary project-relative filesystem layout receive a daemon-built immutable `ProviderWorkspaceView`:

```text
view_id
workspace_id/source_generation/context_id
root manifest mapping WorkspacePath → blob digest/mode/entry kind
configuration and lock artifacts
read-only materialized root
separate writable output/scratch root
view digest
```

The view is assembled off-path from leased blobs using read-only bind mounts, reflinks, hard links to immutable daemon-owned blobs, or verified copies according to platform capability. It is atomically published only after every entry digest and mode verifies, contains no writable link back to the live workspace or `.git`, and is mounted/read-opened read-only inside the provider sandbox. Cargo/rustc, Pyrefly, build scripts, and proc macros SHALL resolve source/configuration inputs from this exact view, never the mutable authorized root. Generated/build outputs are written only to the separate sandbox output tree and are captured under `G-40`.

Locked third-party sources, compiler metadata, typeshed/stubs, proc-macro binaries, and prebuilt dependency artifacts needed to execute a context are supplied through a separate immutable `DependencyInputBundle` whose manifest and digest are pinned by the analysis context. Mounting a dependency input permits semantic compilation/resolution; it does not by itself authorize indexing or source disclosure of dependency bodies. Network dependency resolution is prohibited in the default profile.

Default thresholds:

```text
inline provider control payload:     <= 256 KiB
mmap/read-only blob transport:       > 256 KiB
maximum ordinary source image:       16 MiB
maximum explicitly permitted image: 64 MiB
line-index offset width:             u64
```

Line index format is a little-endian array of `u64` byte offsets beginning with zero, plus newline-profile metadata. The digest covers the serialized index.

Encoding classification:

- Rust semantic parsing requires valid UTF-8;
- Python applies BOM and PEP-263 coding-cookie detection, then converts to a provider-compatible text representation while retaining original bytes and a byte-offset mapping;
- arbitrary undecodable files remain source inventory entries and receive an explicit unsupported-encoding capability status;
- source byte ranges always refer to original authoritative bytes.

Every provider request includes the expected digest; every terminal output echoes it. Digest mismatch is `SOURCE_SNAPSHOT_MISMATCH` and rejects output.

Runtime source blobs are lease-scoped and removed after all provider-run,
serving-snapshot, and source-artifact holders release. One typed snapshot-lease header
may own multiple blob members. Acquisition, renewal, release, restart orphaning, and
garbage collection are serialized through the operational-store writer. Restarted
active leases become orphaned and remain protective through the deployment-profile
grace period. Garbage collection rechecks holder protection inside the same write
transaction as member/metadata removal, deletes a bounded batch, and is idempotent.
Runtime source blobs are not semantic history.
## AC-G-34 — Build and project-configuration discovery
### Decision

Build/configuration discovery is deterministic, versioned, and separate from semantic analysis. It produces immutable context manifests and an invalidation dependency set.

### Contract

#### Python discovery

Recognized configuration sources, in precedence order:

1. explicit CodeFabric workspace profile;
2. `pyrefly.toml` or `[tool.pyrefly]` in `pyproject.toml`;
3. standard `pyproject.toml` project metadata, package discovery, and `requires-python`;
4. explicit source/stub roots from registration;
5. conservative defaults with diagnostics.

Recognized lock/dependency artifacts include `uv.lock`, `poetry.lock`, `pdm.lock`, `Pipfile.lock`, and explicitly registered frozen requirements files. When multiple lock systems are present, the workspace profile SHALL select one; automatic merging is prohibited.

The discovery product defines module roots, package/namespace rules, stub roots, dependency endpoint identities, typeshed bundle, Python version, and configuration digests. It never activates or inspects an arbitrary live virtual environment unless that environment path is explicitly registered and authorized.

#### Rust discovery

The daemon runs a pinned Cargo metadata command with the selected manifest, target triple, feature profile, and lock policy. It captures:

```text
Cargo.toml files
Cargo.lock
.cargo/config and config.toml files
rust-toolchain files
workspace/package/target graph
feature graph
build.rs identities
proc-macro crate identities
generated OUT_DIR roots
source/build dependency edges
```

`cargo metadata` output is normalized and hashed. Build scripts and proc macros are not executed by discovery; execution occurs only in the sandboxed compiler phase.

Any change to manifest/config/lock/toolchain/build script/proc-macro inputs invalidates the affected context manifest. Target-specific source changes use the operational dependency graph rather than forcing every context rebuild.

Discovery SHALL emit a complete `ConfigurationDependencySet` with file IDs/digests and reasons. Unknown or conflicting configuration yields a terminal context diagnostic; the daemon does not guess among incompatible lockfiles, target triples, or Python versions.
## AC-G-35 — Provider sandbox and trust model
### Decision

Repositories are treated as untrusted by default for any operation that can execute build scripts, proc macros, native loaders, external commands, or dependency hooks. Pure parsing may run in-process under resource limits.

### Contract

Trust profiles:

```text
UNTRUSTED_SANDBOXED   default
TRUSTED_LOCAL         explicit operator grant
PARSING_ONLY          semantic execution disabled
```

The `UNTRUSTED_SANDBOXED` provider environment SHALL provide:

- no network namespace/access;
- read-only immutable source blobs and declared dependency/context artifacts;
- no writable access to the workspace, Git metadata, state store, socket directory, home directory, SSH agents, cloud credentials, package-manager credentials, or host temporary directories;
- one private writable temporary/output directory with byte/inode quota;
- an explicit minimal environment allowlist;
- process, thread, CPU, wall-clock, file-size, open-file, address-space, and output limits;
- child-process containment and process-group termination;
- disabled core dumps;
- no inherited unrelated file descriptors;
- sanitized locale and deterministic timezone;
- provider binary/toolchain paths mounted read-only.

Linux conforming implementation uses user/mount/PID/network namespaces, read-only bind mounts, `no_new_privs`, seccomp filtering, and Landlock or equivalent path restrictions. If these protections cannot be established, untrusted compiler/Pyrefly semantic execution is unavailable rather than silently unsandboxed.

macOS conforming execution requires an approved Seatbelt/sandbox profile or an explicit `TRUSTED_LOCAL` grant. Pure Tree-sitter/Ruff parsing may remain available in-process.

Build scripts and proc macros may execute only in the sandbox. External Git filters, hooks, credential helpers, and arbitrary shell commands are never executed by CodeFabric.

`TRUSTED_LOCAL` relaxes sandbox availability requirements but still removes credentials and network by default; enabling network requires a separate future profile.

Sandbox profile digest and trust profile are pinned in provider runs and snapshots. A trust downgrade invalidates affected contexts.

Activation is deliberately staged. The Wave-5 compiler extractor may execute only the
committed golden Rust fixture, only under an explicit `TRUSTED_LOCAL` operator grant, and
only with network and credential access removed and the Wave-5 resource profile pinned.
Arbitrary registered repositories and `UNTRUSTED_SANDBOXED` compiler execution remain
fail-closed until the Wave-10 sandbox packet proves the platform containment contract
above. The Wave-5 exception is test-corpus scoped and cannot be selected by an ordinary
workspace registration.
## AC-G-36 — Provider capability granularity and aggregation
### Decision

Capabilities are registry-defined and reported at their smallest meaningful scope. Snapshot/query coverage is derived by a formal aggregation, not provider-specific status strings.

### Contract

Each capability definition contains:

```text
capability_code and slug
fact families produced
allowed scope kinds
required provider or derivation
prerequisite capabilities
applicability predicate
completeness proof rule
supported precision profiles
```

The provider registry record is fixed as:

```text
provider_id
provider_slug
placement: IN_PROCESS | SIDECAR | COMPILER_GROUP
protocol_package: optional
protocol_service: optional
toolchain_or_bundle_digest_fields: ordered
capability_codes: ordered unique
resource_profile_id
event_mapping_version
introduced/deprecated/replacement versions
```

`protocol_package` and `protocol_service` are required for sidecar/compiler
placements and absent for in-process providers. The registry references the
single provider-event mapping in `feature-registry.yaml`; provider-specific
event enums are prohibited at the application boundary. Credit-control values
are defined once in `provider_control.proto` as four outstanding chunks and
16 MiB of unacknowledged payload and are imported or conformance-tested by the
two provider protocols.

Mandatory initial scope kinds:

```text
workspace
analysis_context
build_unit
module_or_crate
source_file
semantic_owner
callable_or_MIR_body
workspace_global_derivation
```

Representative capability families:

```text
SOURCE_BYTES, SOURCE_INVENTORY, TOKENS, CST, TYPED_AST,
SCOPES_BINDINGS, IMPORT_RESOLUTION, DECLARED_TYPES, COMPUTED_TYPES,
MEMBER_RESOLUTION, CALL_TARGETS, RUST_MIR, BORROW_LOANS,
CFG, DOMINANCE, CONTROL_DEPENDENCE, DEF_USE, LIVENESS,
POINTS_TO_ALIAS, EFFECTS, CONCURRENCY, CALLABLE_SUMMARIES
```

A `CapabilityEvidence` record includes scope ID, context, source generation, owner-capability state, completeness state, provider run, reason, diagnostic, coverage fingerprint, external/unknown remainder flags, and supporting owner set digest.

Aggregation rules:

- `COMPLETE` only if every applicable child scope is complete, no uncharacterized child is missing, all required contexts are covered, and external/open-world policy permits closure;
- `PARTIAL` when available facts exist and the missing remainder is characterized;
- `INDETERMINATE` when a missing/unresolved remainder cannot be bounded;
- `UNAVAILABLE` when the capability applies but no usable facts exist;
- `NOT_APPLICABLE` only when the registry applicability predicate is false for every child.

One child `INDETERMINATE` prevents a parent `COMPLETE`. Excluded/unauthorized source is reported as an explicit scope exclusion and is not silently counted complete over the wider workspace.
## AC-G-38 — Declarative model-pack format, matching, and trust
### Decision

Model packs are non-executable, signed declarative semantic records for external/builtin behavior. They cannot run code or load plugins.

### Contract

A pack contains:

```yaml
pack_id: reverse-DNS identifier
pack_version: "1.0"
schema_version: "1.0"
targets:
  - ecosystem
    package_name
    version_range
    module_or_crate_pattern
    platform_constraints
records:
  - record_id
    record_kind: callable | member | type | effect | resource | ffi_bridge
    match:
      qualified_name
      receiver_type_pattern optional
      signature_pattern optional
      generic_constraints optional
      overload_selector optional
    semantics:
      return_type_formula optional
      purity: pure | impure | unknown
      direct_effects: []
      raises_or_panics: []
      resource_actions: []
      callback_argument_roles: []
      argument_to_result_relations: []
      dispatch_targets: []
    certainty: MODELLED
    closed_world_fields: []
    provenance
fixtures: []
pack_digest: b3:...
signature: Ed25519
```

Pattern grammar is closed and deterministic: package/module/crate names are normalized by the ecosystem context; qualified-name patterns contain exact segments plus at most one terminal `*` segment; receiver/signature patterns are typed algebra patterns with named parameter-role wildcards and no arbitrary regular expressions. Python version ranges use the pinned PEP 440 evaluator; Cargo ranges use the pinned Cargo-semver evaluator. A pack records the evaluator versions in its digest.

Matching order:

1. exact package version, exact qualified name, exact compatible signature/receiver;
2. exact version, qualified name, compatible overload;
3. signed bounded version range with compatible signature;
4. no match.

Two equally specific conflicting records make the modeled capability unavailable for that target. Model packs never override exact source/compiler facts. Their facts use `MODELLED` certainty and resolution.

Supported type/effect formulas are a closed declarative expression language over argument types/values and registered effect/resource codes. No arbitrary Python, Rust, regex backtracking engine, shell command, or native library is permitted.

Built-in packs are trusted by shipped digest. External packs require an Ed25519 signature from the configured trust store. Pack selection is determined by the analysis-context dependency lock and is pinned in the context/snapshot.

Each record requires positive, negative, and version-boundary fixtures. Pack activation fails if fixtures do not validate against the installed ontology/type/model schema.
## AC-G-39 — Derived-analysis precision profiles
### Decision

The derivation registry exposes three named precision profiles. `BALANCED_V1` is the default. Profiles are immutable compatibility artifacts; an implementation may optimize algorithms but not alter their semantics.

### Contract

| Dimension | `FAST_V1` | `BALANCED_V1` | `PRECISE_V1` |
|---|---|---|---|
| Intraprocedural flow | flow-sensitive | flow-sensitive | flow-sensitive |
| Branch/path sensitivity | path-insensitive | predicate-aware joins, no full path splitting | bounded path sensitivity for registered predicates |
| Interprocedural context | context-insensitive | context-insensitive summaries, concrete Rust instances separate | 1-call-site for Python and bounded receiver sensitivity for methods |
| Fields | first-level field sensitive | field sensitive for named fields/projections | field sensitive with bounded nested access paths |
| Heap abstraction | type/site merged | allocation-site + owner/context | allocation-site + bounded calling/receiver context |
| Arrays/maps | single summary element | constant index/key exact, otherwise wildcard | bounded small constant sets, otherwise wildcard |
| Strong updates | exact locals only | singleton non-aliased locations | singleton exact points-to locations |
| Exceptions/unwind | included conservatively | explicit normal/exception/unwind edges | explicit plus bounded handler-state refinement |
| Dynamic dispatch | open-world unknown remainder | exact + sound candidates + unknown remainder | same, with narrower context filters |
| Max points-to set before widening | 64 | 256 | 1024 |
| Fixed-point iteration cap | 32 | 64 | 128 |
| Summary recursion handling | SCC union | SCC worklist with widening | SCC worklist with delayed widening |

Python CFG is built from Ruff AST; Rust CFG and state transitions use MIR. Rust monomorphized instances are separate semantic contexts within the Rust analysis context's instance space, not merged by context-insensitive call summaries.

Kill semantics:

- exact local assignment kills prior exact local definitions;
- exact field assignment kills only when base points to one exact location under the profile;
- wildcard index/deref/unknown alias uses weak update;
- function calls apply direct/modelled/summary writes and preserve unknown remainder;
- exceptional edges participate in reaching definitions and liveness under the selected projection.

Convergence is deterministic by sorted worklists. Widening produces an explicit `analysis_widened` property and may change completeness to `PARTIAL` or `INDETERMINATE`; it never pretends the widened result is exact.

Profile ID and derivation-bundle digest are stored on every materialized derived fact.
## AC-G-40 — Generated, expanded, stub, shim, and lowered source capture
### Decision

Generated/lowered representations are separate identity domains linked to source-authored entities. Their bytes are retained only under snapshot/source-disclosure leases.

### Contract

Representation classes:

```text
SOURCE_AUTHORED
GENERATED_SOURCE
BUILD_SCRIPT_OUTPUT
PROC_MACRO_OUTPUT
MACRO_EXPANSION
EXTERNAL_STUB
COMPILER_SHIM
DROP_GLUE
MONOMORPHIZED_INSTANCE
MIR_LOWERED
```

Generated-source identity includes workspace, context, generator entity/build unit, generation role, logical output name, and content digest. It never masquerades as a workspace path. URI form is:

```text
codefabric-generated://<workspace>/<context>/<generated-entity-id>
```

Capture rules:

- build-script and proc-macro outputs are read only from the sandbox output tree and tied to the exact run/context digest;
- macro-expansion/source-map records come from compiler evidence and preserve invocation and expansion relationships;
- external stubs carry dependency/version/model provenance and source ACL classification;
- compiler shims/drop glue/MIR may have no textual source; they still expose structured lowered facts and optional diagnostic rendering;
- generated bytes, when available, use the same immutable source-image and line-index contract;
- source-authored and generated spans are never compared as if they share one file coordinate space.

Invalidation edges connect every generated artifact to its generator inputs, build unit, context, toolchain, and model/provider bundle. Any changed dependency invalidates the generated representation.

Default retention is runtime/snapshot-lease only. Durable facts retain content digests and provenance, not necessarily full generated bytes. Source disclosure rules independently govern generated text.
## AC-G-43 — Unsupported, oversized, binary, generated, and vendored files
### Decision

The default profile inventories every authorized regular entry but semantically analyzes only supported source files within explicit resource and inclusion limits.

### Contract

Defaults:

```text
ordinary semantic-analysis size limit: 16 MiB
explicit per-file hard limit:           64 MiB
first-pass binary sample:               8 KiB
maximum single line before warning:     1 MiB
maximum path components:                256
maximum path byte length:               4096
```

Binary classification is true when the initial sample contains NUL bytes, the file matches a registered binary signature, or required language decoding fails without a recognized source encoding. Binary files remain inventory entities with digest/size and `UNSUPPORTED_BINARY`; their bytes are not returned unless source ACL and an explicit byte-context request allow it.

Language mapping precedence:

1. explicit workspace override by path ID;
2. exact registered extension;
3. recognized shebang for text files;
4. no language mapping.

Conflicting extension/shebang evidence produces a diagnostic and uses the explicit/extension mapping; it never runs two language parsers silently.

Default exclusions from semantic body indexing:

```text
.git and Git administrative directories
target, build, dist, out, coverage, cache directories
node_modules
.venv, virtualenv and package-manager environments
vendored/third_party directories identified by profile
large generated dependency trees
```

Tests under ordinary workspace test directories are included for source/syntax; language semantic context coverage follows the context profile. Generated source under authorized build outputs is included only through the generated-source capture contract, not generic recursive walking.

Vendored or third-party source is endpoint-only by default. Explicitly authorizing it creates a context/body partition and counts against workspace quotas.

Oversized/unsupported files produce stable diagnostics and capability records; they never abort unrelated workspace analysis. A required configuration file exceeding the hard limit blocks the affected context with `CONFIG_ARTIFACT_TOO_LARGE`.


## Cross-layer integration obligations

The following architecture-completion contracts are owned by another 1.3 artifact but are binding inputs to this specification. This document SHALL consume the named contract and SHALL NOT restate it with different semantics.

| Gap | Contract | Permanent owner | Integration obligation in this document |
|---|---|---|---|
| `G-10` | Daemon workspace registry and administrative lifecycle | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-11` | Root authorization, symlink boundaries, and secure path opening | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-12` | File identity across replacement, rename, and move | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-13` | Canonical ID preimage serialization | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-15` | Canonical type algebra | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-16` | External dependency identity and body policy | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-17` | Cross-language and FFI linking profile | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-18` | Path canonicalization, display, URI, and ordering | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-24` | Formal freshness state machine and query barrier | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-25` | Machine-testable lifecycle transition tables | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-29` | Logical multi-file edit batches and publication barriers | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-37` | Canonical reconciliation algorithm | [Data-fabric specification 1.3](./present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-41` | Operational dependency graph schema and update algorithm | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-42` | Derivation materialization matrix | [Data-fabric specification 1.3](./present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-48` | Completeness and negative-proof algebra | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-70` | Machine ontology registry | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-71` | Property schema, value types, cardinality, null, and storage mapping | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-72` | Mandatory conformance profiles | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-73` | Unknown entities, unknown remainder, and explicit negative facts | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-74` | Graph projection registry | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-75` | Interprocedural summary semantics registry | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-76` | Static concurrency and happens-before semantics | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |
| `G-77` | Effect and resource model semantics | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Emit observations, capability evidence, provenance, and manifests conforming to the owner contract. |

## Release conformance obligations

This specification inherits `G-78` through `G-84` from the suite governance and release manifest. Release acceptance SHALL include the portions of the golden corpus, clean-rebuild comparator, conformance harness, deterministic fault matrix, performance profiles, upgrade choreography, and adversarial security corpus that exercise provider protocols, source-image transport, context discovery, model packs, precision profiles, observation batches, manifests, and provider failure behavior.

A passing prose review is insufficient. The corresponding generated registries, schemas, protocol descriptors, fixtures, canonical outputs, and fault oracles SHALL pass the master release gates before an implementation may claim CodeFabric 1.3 conformance.

# Appendix A — Provider Capability Legend

```text
DIRECT
    Provider emits the fact in structured form.

NORMALIZED
    Provider emits a lower-level construct mapped directly into canonical fact.

RECONCILED
    Fact requires joining two or more providers.

DERIVED
    Fact is mechanically computed from normalized facts.

MODELLED
    Fact is supplied by a versioned library/framework semantic model.

CONSERVATIVE
    Fact is a sound or intentionally widened approximation.

UNAVAILABLE
    Current provider stack cannot generate the fact.

UNKNOWN
    The analyzed code contains an unresolved semantic remainder.
```

---

# Appendix B — Required Model-Pack Categories

```text
Python:
    builtins
    context managers
    asyncio
    threading
    multiprocessing
    queue/channel APIs
    file/socket/database resources
    descriptors/framework synthesis
    common registries/callback systems

Rust:
    standard synchronization guards
    Tokio task/channel/lock APIs
    Rayon task boundaries where relevant
    std::thread
    filesystem/network/database resources
    allocator/resource types
    external FFI shims
```

Model packs SHALL add facts, never replace direct compiler/provider facts without explicit reconciliation.

---

# Appendix C — Explicit Non-Outputs

The fact-generation system SHALL NOT emit:

```text
change history
semantic diff
test impact
coverage
runtime profile
refactor safety
risk score
bug likelihood
architecture quality
vulnerability exploitability
recommendation
remediation
prioritization
```

These remain downstream agent reasoning tasks.
