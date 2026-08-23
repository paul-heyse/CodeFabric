# Comprehensive Present-State Code Property Graph Ontology Specification

**Artifact ID:** `codefabric-present-state-cpg-ontology`
**Artifact kind:** Normative document
**Compatible suite major:** 1
**Release date:** 2026-08-20
**Canonical digest:** External; recorded in `codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_artifact_index.json`

**Status:** Released normative specification
**Synchronized suite version:** 1.3
**Specification version:** 1.3
**Target languages:** Python and Rust
**Primary purpose:** Present-state code-intelligence fact substrate for LLM programming agents
**Artifact type:** Language-neutral core ontology with Python- and Rust-specific extensions
**Scope boundary:** Facts and mechanically derived facts only; no task-specific or evaluative analysis
**Audit integration (2026-08-20):** Plan-audit F-001; fixed initial CBEF, path-platform, registry-code, and family allocations.

---

## 0. Synchronized CodeFabric 1.3 governing contract

This document is a released member of the synchronized **CodeFabric present-state CPG specification suite, version 1.3**. The suite integrates the architecture-completion contracts `G-01` through `G-84`; the earlier standalone completion specification is retained only as a historical design record and is no longer required to interpret this release.

The cross-cutting source of authority is `codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md`. This document permanently owns the domain contracts assigned to it by that manifest. A less-specific statement elsewhere in this document SHALL be read through the 1.3 contract sections and SHALL NOT override them.

### 0.1 Artifact identity and version

```yaml
artifact_id: "codefabric-present-state-cpg-ontology"
artifact_kind: document
version: "1.3"
compatible_suite_major: 1
status: released
canonical_digest: external
```

The canonical digest and exact source digest are recorded in `codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_artifact_index.json`. Versions are integer pairs, never floating-point values; `1.10` is newer than `1.9`.

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

This document specifies a comprehensive ontology for a Code Property Graph (CPG) whose primary purpose is to provide LLM programming agents with a maximally rich, semantically precise representation of the **present state of analyzed code**.

The CPG defined here is a **fact substrate**, not an automated software-engineering decision system.

It SHALL represent:

- facts directly harvested from source text;
- facts obtained from parsers, semantic analyzers, type systems, compilers, and intermediate representations;
- facts mechanically derived from graph topology or semantic dataflow;
- deterministic summaries of larger fact sets;
- explicit unresolved or unknown semantic states.

It SHALL NOT attempt to make higher-level judgments such as:

- whether a refactor is safe;
- which tests are impacted by a change;
- whether code is risky or poorly designed;
- whether an architectural dependency should be removed;
- whether a vulnerability is exploitable;
- what code should be changed.

Those conclusions are deliberately left to downstream reasoning systems, including LLM programming agents.

The design objective is:

> **Provide the richest possible present-state semantic evidence from which an intelligent programming agent can perform its own reasoning.**

---

## 2. Normative scope

### 2.1 Included information

The ontology covers the current analyzed program, including:

- source text and lexical structure;
- complete syntax;
- declarations and semantic identities;
- lexical scopes and bindings;
- references and name resolution;
- modules, imports, exports, and code-declared dependencies;
- types and type relationships;
- members, inheritance, traits, protocols, and implementations;
- callables, call sites, arguments, and dispatch;
- control flow;
- values and computations;
- dataflow and def-use;
- state and abstract memory locations;
- aliasing and points-to relationships;
- initialization and program-point state;
- Rust ownership, borrowing, moves, copies, and drops;
- direct and transitive effects;
- exceptions, panic, unwind, and cleanup;
- resource lifetimes;
- async, generator, task, thread, channel, and lock semantics;
- closures and captures;
- generated and lowered code;
- generic specialization and Rust monomorphization;
- macros and macro expansion;
- Rust MIR;
- objective graph-derived facts;
- deterministic semantic summaries;
- explicit unresolved facts.

### 2.2 Excluded information

The ontology excludes the following as first-class CPG fact domains:

#### Historical state

- Git history;
- commit history;
- prior revisions;
- semantic diff across revisions;
- code churn;
- historical hotspots;
- blame information;
- longitudinal evolution.

#### Runtime observation

- runtime execution traces;
- code coverage;
- production profiling;
- sampled values;
- runtime-observed call edges;
- production telemetry.

#### External environment state

- active virtual environments;
- installed package inventories;
- host operating-system state;
- environment variables;
- deployment state;
- current machine-specific configuration;
- live infrastructure state.

Code that *declares or consumes* configuration remains part of the source ontology. For example:

- Rust `#[cfg(...)]` syntax is a source fact;
- Cargo feature declarations are code/project facts;
- Python import statements are code facts;
- environment-variable reads in source are code facts.

The ontology simply does not attempt to model which external environment is currently active.

#### Evaluative or task-oriented conclusions

The ontology excludes conclusions such as:

- `REFACTOR_SAFE`;
- `TEST_IMPACTED`;
- `HIGH_RISK`;
- `GOD_CLASS`;
- `VULNERABLE`;
- `ARCHITECTURALLY_BAD`;
- `SHOULD_REWRITE`;
- `RECOMMENDED_CHANGE`.

Tests themselves remain ordinary code and receive the same semantic representation as all other code.

---

## 3. Definition and canonical forms of a fact

A **fact** is an objective proposition about the present-state analyzed program that is either:

1. directly observable in source;
2. determined by language semantics;
3. exposed by a compiler or semantic engine;
4. mechanically derived from other program facts; or
5. a deterministic summary of another fact set.

The ontology recognizes five semantic classes:

| Fact class | Definition | Example |
|---|---|---|
| **Source fact** | Directly observable from source text | Identifier `foo` occupies bytes 120–123 |
| **Semantic fact** | Determined by language semantics or a semantic analyzer | This occurrence of `foo` resolves to function `F` |
| **Compiler/lowered fact** | Exposed by compiler or intermediate representation | MIR block `bb7` branches to `bb9` and `bb10` |
| **Derived graph fact** | Deterministically computed from other graph facts | Basic block `B3` dominates `B14` |
| **Summary fact** | Deterministic compression of another fact set | Function `F` may write fields `{x, y}` |

### 3.1 Canonical first-class fact forms

Every query-visible proposition SHALL use one of three storage-neutral forms:

```text
ENTITY_EXISTENCE_FACT
RELATION_FACT
PROPERTY_FACT
```

An entity ID identifies the program object. A fact ID identifies a proposition about that object or between objects.

- A **relation fact** has subject entity, relation kind, object entity, and optional role/ordinal.
- A **property fact** has subject entity, property kind, typed value, and optional program point.
- An **entity-existence fact** asserts that an entity is represented in the current snapshot; it is encoded as a property fact when requested independently.

Every first-class fact SHALL have:

```text
fact_id
workspace_id
analysis_context_id
fact_form
fact_kind
owner_id
producer and producer version
certainty
resolution
directness
source evidence where applicable
derivation metadata where applicable
completeness interpretation
```

Entity and extension-table columns MAY duplicate commonly requested properties for scan efficiency, but a duplicated column is not the canonical provenance-bearing proposition. If a property is sourced independently, has independent certainty, or may conflict across providers, the canonical proposition SHALL be a `PROPERTY_FACT` with its own evidence.

### 3.2 Facts and non-facts

The following are facts:

```text
CALLS_EXACT(F, G)
TRANSITIVELY_REACHES(F, G)
SAME_CALL_SCC(F, G)
DOMINATES(B1, B9)
MAY_ALIAS(X, Y)
PROPERTY(F, cyclomatic_complexity, 7)
PROPERTY(X, inferred_type, T)
```

The following are not facts within this specification:

```text
CHANGING_G_IS_RISKY_FOR_F
THIS_REFACTOR_IS_SAFE
THIS_TEST_IS_RELEVANT
THIS_MODULE_SHOULD_BE_SPLIT
```

## 4. Design principles

### 4.1 Present-state only

All facts describe one analyzed program state.

The graph MAY contain internal generation or freshness metadata required for consistency, but historical states are outside this ontology.

### 4.2 Raw and normalized representations SHALL coexist

The ontology SHALL preserve both:

- the provider-native or language-native representation; and
- a normalized semantic representation.

This is required for future completeness.

For syntax:

```text
SYNTAX_NODE
  raw_language_kind
  normalized_kind
  source_span
  frontend_fields
```

For Rust MIR:

```text
MIR_NODE
  raw_mir_variant
  normalized_semantic_kind
  owner
  source_correspondence
```

A normalized enumeration MUST NOT prevent representation of newly introduced Python grammar nodes or Rust compiler variants.

### 4.3 Syntax occurrence and semantic entity SHALL remain distinct

An identifier occurrence in source is not the same object as the semantic declaration it denotes.

Likewise:

- call syntax is not a callable;
- type syntax is not a semantic type;
- a member-access expression is not the member declaration;
- a generic declaration is not a monomorphized instance.

### 4.4 Call sites SHALL be first-class entities

The graph MUST NOT reduce call semantics solely to caller-to-callee edges.

A call site carries essential information:

- source location;
- receiver;
- arguments;
- binding of arguments to parameters;
- dispatch mechanism;
- declared target;
- exact target;
- candidate target set;
- unknown target state.

Derived caller-to-callee relations MAY be materialized for convenience.

### 4.5 Unknown is a first-class fact

Absence of a resolved relationship MUST NOT be used to imply impossibility.

For example:

```text
MAY_CALL -> UNKNOWN_CALL_TARGET
REFERS_TO -> UNKNOWN_SYMBOL
MAY_POINT_TO -> UNKNOWN_MEMORY
MAY_RESOLVE_MEMBER -> UNKNOWN_MEMBER
```

is superior to silently omitting unresolved facts.

### 4.6 Direct and transitive facts SHALL remain distinguishable

For example:

```text
DIRECTLY_WRITES
TRANSITIVELY_WRITES

DIRECTLY_CALLS
TRANSITIVELY_CALLS

DIRECT_EFFECT
TRANSITIVE_EFFECT
```

A downstream agent must be able to distinguish behavior authored in the subject itself from behavior inherited through callees.

### 4.7 Objective derivation is permitted; evaluative interpretation is not

The following are valid derived facts:

- SCC membership;
- dominance;
- post-dominance;
- control dependence;
- reaching definitions;
- liveness;
- points-to sets;
- alias sets;
- transitive reachability;
- loop structure;
- recursion;
- mechanically computed metrics.

The following are not:

- risky;
- fragile;
- safe;
- bad architecture;
- likely impacted;
- recommended.

---

# Part I — Language-Neutral Core Ontology

## 5. Source and lexical ontology

### 5.1 Core entities

#### `SOURCE_FILE`

Represents one analyzed source file.

Required conceptual properties:

- stable file identity within the analyzed codebase;
- repository-relative or workspace-relative path;
- language;
- source length;
- source digest or equivalent present-state identity.

#### `SOURCE_SPAN`

Represents a half-open byte interval in a specific source file.

Conceptual properties:

```text
file
start_byte
end_byte
```

Line and column positions MAY be derived presentation properties but byte offsets SHOULD remain canonical.

#### `TOKEN`

Represents a lexical token.

Properties:

- raw token kind;
- normalized token kind;
- span;
- lexical text or text reference;
- ordinal within file.

Specialized token categories MAY include:

```text
IDENTIFIER_TOKEN
KEYWORD_TOKEN
OPERATOR_TOKEN
PUNCTUATION_TOKEN
LITERAL_TOKEN
STRING_TOKEN
NUMBER_TOKEN
```

#### `COMMENT`

Represents a source comment, preserving exact source range and text.

#### `DOCUMENTATION`

Represents language-recognized documentation constructs such as:

- Python docstrings;
- Rust doc comments;
- documentation attributes.

#### `PRAGMA_OR_DIRECTIVE`

Represents source directives such as:

- Python type comments;
- Python `type: ignore`;
- Rust attributes;
- Rust `cfg` declarations;
- language pragmas.

#### `PARSE_ERROR`

Represents parser-recognized invalid syntax.

#### `MISSING_SYNTAX`

Represents parser-synthesized missing syntax used during error recovery.

### 5.2 Core lexical relationships

```text
CONTAINS_SPAN
TOKEN_OF
LEXICALLY_PRECEDES
DOCUMENTS
DIRECTIVE_APPLIES_TO
```

A provider MAY expose stronger lexical ordering relationships, but the canonical ontology SHOULD preserve source ordering deterministically.

---

## 6. Syntax ontology

The ontology SHALL be capable of representing every syntax construct supported by the language frontend.

### 6.1 Universal syntax entities

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
MEMBER_ACCESS
SUBSCRIPT_ACCESS
INDEX_ACCESS
CALL_EXPRESSION
ASSIGNMENT
BRANCH
LOOP
RETURN
YIELD
AWAIT
RAISE_OR_PANIC_SYNTAX
IMPORT_OR_USE_SYNTAX
```

The normalized kind hierarchy MAY be more detailed, but no provider-native node kind may become unrepresentable.

### 6.2 Required syntax-node properties

```text
language
raw_kind
normalized_kind
source_span
is_named
is_error
is_missing
```

Optional provider-specific properties MAY include:

```text
grammar_field_name
frontend_node_id
parse_state
extra/trivia flags
```

Provider-local IDs MUST NOT be treated as durable semantic identity.

### 6.3 Structural relationships

#### `AST_CHILD(parent, child, field_name, ordinal)`

Canonical ordered syntax containment.

`field_name` SHALL be retained where the language frontend provides it.

Examples:

```text
condition
body
target
value
receiver
callee
argument
decorator
return_type
pattern
guard
```

Additional structural convenience relations MAY include:

```text
PARENT_OF
ENCLOSES
LEXICAL_NEXT
```

---

## 7. Semantic identity ontology

Syntax nodes represent occurrences. Semantic nodes represent language-level entities.

### 7.1 Core semantic entity kinds

```text
MODULE
NAMESPACE
SCOPE

SYMBOL
DECLARATION
BINDING
REFERENCE

FUNCTION
METHOD
CLOSURE
LAMBDA
CONSTRUCTOR
PARAMETER

CLASS
STRUCT
ENUM
UNION
TRAIT
PROTOCOL
INTERFACE
ENUM_VARIANT
FIELD
PROPERTY
MEMBER

VARIABLE
LOCAL
GLOBAL
STATIC
CONSTANT

TYPE_ALIAS
TYPE_PARAMETER
LIFETIME_PARAMETER
CONST_PARAMETER

EXTERNAL_SYMBOL
SYNTHESIZED_SYMBOL
GENERATED_SYMBOL
```

Not every language uses every kind.

### 7.2 Common semantic properties

```text
name
qualified_name
semantic_kind
visibility
mutability
source_span
name_span

is_external
is_generated
is_synthesized
```

Language-specific modifiers MAY include:

```text
async
unsafe
const
static
final
abstract
classmethod
extern
default
```

### 7.3 Core ownership relationships

```text
DECLARES
DEFINED_IN
OWNED_BY
CONTAINS
HAS_SCOPE
ENCLOSING_SCOPE
```

---

## 8. Scope, binding, and name-resolution ontology

### 8.1 Scope facts

A `SCOPE` represents a language-recognized lexical or semantic name-resolution domain.

Scope facts include:

- scope kind;
- parent scope;
- declared bindings;
- visible bindings;
- free variables;
- captured variables;
- shadowing relationships.

### 8.2 Binding relationships

```text
BINDS
REFERS_TO
MAY_REFER_TO
SHADOWS
CAPTURES
CAPTURED_FROM
ALIASES
REBINDS
```

### 8.3 Reference classification

Identifier/reference occurrences SHOULD be classifiable as:

```text
declaration
definition
read
write
read_write
import_binding
parameter_binding
capture
type_reference
call_reference
member_reference
```

### 8.4 Unresolved references

An unresolved occurrence SHALL remain explicit:

```text
REFERS_TO -> UNKNOWN_SYMBOL
```

or, when multiple candidates remain:

```text
MAY_REFER_TO -> candidate_1
MAY_REFER_TO -> candidate_2
```

---

## 9. Module, import, export, and dependency ontology

### 9.1 Entities

```text
MODULE
PACKAGE
CRATE
IMPORT_DECLARATION
IMPORT_BINDING
EXPORT
REEXPORT
EXTERNAL_DEPENDENCY_REFERENCE
```

### 9.2 Relationships

```text
IMPORTS_MODULE
IMPORTS_SYMBOL
EXPORTS
REEXPORTS
ALIASES
DEFINED_IN_MODULE
DEPENDS_ON_MODULE
```

### 9.3 Required distinction

The following SHALL remain semantically distinct:

```text
import/use syntax
local imported binding
resolved module
resolved imported symbol
re-exported symbol
```

A single import syntax occurrence may therefore produce several semantic facts.

---

## 10. Type ontology

Types SHALL be graph entities, not merely strings.

### 10.1 Common type families

```text
UNKNOWN_TYPE
ERROR_TYPE
ANY_OR_DYNAMIC
NEVER_OR_BOTTOM
NULL_OR_NONE

PRIMITIVE_TYPE
NOMINAL_TYPE
CLASS_OBJECT_TYPE
TYPE_OBJECT

LITERAL_TYPE
UNION_TYPE
INTERSECTION_TYPE

CALLABLE_TYPE
BOUND_METHOD_TYPE

TUPLE_TYPE
ARRAY_TYPE
LIST_TYPE
SEQUENCE_TYPE
MAPPING_TYPE
STRUCTURAL_TYPE

GENERIC_TYPE
TYPE_PARAMETER
TYPE_VARIABLE
ASSOCIATED_TYPE
TYPE_ALIAS

REFERENCE_TYPE
POINTER_TYPE
```

### 10.2 Type relationships

```text
DECLARED_TYPE
INFERRED_TYPE
COMPUTED_TYPE
EXPECTED_TYPE
TYPE_OF

PARAMETER_TYPE
RETURN_TYPE
FIELD_TYPE

TYPE_PARAMETER_OF
TYPE_ARGUMENT
INSTANTIATES

SUBTYPE_OF
SUPERTYPE_OF
BOUNDED_BY
CONSTRAINED_BY

COERCES_TO
CASTS_TO
NARROWS_TO
```

### 10.3 Distinct type concepts

The graph SHALL preserve separately, where available:

- declared type;
- inferred/computed type;
- expected/contextual type.

These facts SHALL NOT be collapsed into one `HAS_TYPE` fact unless the original distinctions remain recoverable.

---

## 11. Member and object-model ontology

### 11.1 Entities

```text
MEMBER
FIELD
METHOD
PROPERTY
DESCRIPTOR
ASSOCIATED_ITEM
```

### 11.2 Relationships

```text
DECLARES_MEMBER
HAS_MEMBER
INHERITS
IMPLEMENTS
IMPLEMENTS_TRAIT
IMPLEMENTS_METHOD
OVERRIDES
OVERRIDDEN_BY

RESOLVES_MEMBER
MAY_RESOLVE_MEMBER
```

### 11.3 Member properties

Potential objective properties include:

```text
visibility
static_or_instance_status
class_member_status
read_only
writeable
final
abstract
receiver_type
declaring_type
resolved_owner_type
```

Language-specific member resolution SHALL be permitted without forcing Python and Rust into identical mechanics.

---

## 12. Callable contract ontology

Every callable SHOULD expose a complete, objective invocation contract.

### 12.1 Callable properties

```text
name
qualified_name

parameter_count
parameter_ordering
parameter_kinds
default_values_or_default_expressions

receiver_semantics
variadic_status

generic_parameters
return_type

async_status
generator_status

ABI_or_calling_convention
unsafe_status
const_status
```

### 12.2 Relationships

```text
HAS_PARAMETER
RETURNS_TYPE
HAS_TYPE_PARAMETER
HAS_GENERIC_CONSTRAINT
CAPTURES
```

---

## 13. Call-site ontology

Call sites SHALL be first-class graph entities.

### 13.1 Entities

```text
CALL_SITE
CALLEE_EXPRESSION
RECEIVER
ARGUMENT
ARGUMENT_BINDING
```

### 13.2 Relationships

```text
CONTAINS_CALL
HAS_CALLEE_EXPRESSION
HAS_RECEIVER
HAS_ARGUMENT
ARGUMENT_BINDS_TO

CALLS_DECLARATION
CALLS_EXACT_TARGET
CALLS_INSTANCE
MAY_CALL
CALLS_UNKNOWN

REFERENCES_CALLABLE
TAKES_FUNCTION_ADDRESS
PASSES_CALLABLE
RETURNS_CALLABLE
```

### 13.3 Call-site properties

```text
source_span
call_syntax_kind
dispatch_kind
resolved_target_count
resolution_status
```

### 13.4 Derived caller/callee facts

The graph MAY materialize convenience relations:

```text
DIRECT_CALLER
DIRECT_CALLEE
TRANSITIVE_CALLER
TRANSITIVE_CALLEE
```

but these SHALL be derivable from call-site facts.

---

## 14. Dispatch ontology

Dispatch mechanism SHALL be explicit.

### 14.1 Common dispatch kinds

```text
DIRECT
STATIC_METHOD
BOUND_METHOD
CONSTRUCTOR
CLOSURE
FUNCTION_POINTER
CALLABLE_OBJECT
STATIC_TRAIT
DYNAMIC_TRAIT
VTABLE
VIRTUAL_OVERRIDE
INTRINSIC
FOREIGN
COMPILER_SHIM
DROP_GLUE
UNKNOWN_DYNAMIC
```

### 14.2 Dispatch facts

```text
dispatch_mechanism
declared_target
resolved_target
possible_target_set
receiver_type
target_instance
```

The graph SHALL distinguish:

- declared contract target;
- exact executable target;
- possible target set.

---

## 15. Control-flow ontology

### 15.1 Entities

```text
CONTROL_FLOW_GRAPH
ENTRY
EXIT
BASIC_BLOCK
INSTRUCTION
OPERATION
BRANCH
SWITCH
LOOP_HEADER
RETURN_POINT
EXCEPTIONAL_EXIT
```

### 15.2 CFG relationships

```text
CFG_NEXT
CFG_TRUE
CFG_FALSE
CFG_CASE
CFG_LOOP_BACK
CFG_BREAK
CFG_CONTINUE
CFG_RETURN
CFG_EXCEPTION
CFG_UNWIND
CFG_CALL_RETURN
```

Normal and exceptional control flow SHALL remain distinct.

---

## 16. Derived control-flow facts

The following mechanically derived facts are part of the ontology.

```text
PREDECESSOR
SUCCESSOR

REACHABLE_BLOCK
UNREACHABLE_BLOCK

DOMINATES
STRICTLY_DOMINATES
IMMEDIATE_DOMINATOR

POST_DOMINATES
IMMEDIATE_POST_DOMINATOR

CONTROL_DEPENDENT_ON

BACK_EDGE
LOOP_MEMBER
LOOP_HEADER
LOOP_NESTING_DEPTH

CFG_SCC_MEMBER
```

### 16.1 Reachability scope

`UNREACHABLE_BLOCK` is meaningful only relative to a defined CFG root or owner. The graph MUST preserve sufficient ownership context to avoid ambiguous global claims.

---

## 17. Value and computation ontology

### 17.1 Value entities

```text
VALUE
CONSTANT_VALUE
PARAMETER_VALUE
RETURN_VALUE
TEMPORARY_VALUE
MERGED_VALUE
UNKNOWN_VALUE
```

### 17.2 Computation entities

```text
UNARY_OPERATION
BINARY_OPERATION
COMPARISON
CAST_OPERATION
COERCION_OPERATION
AGGREGATE_OR_CONSTRUCTION
INDEX_OPERATION
FIELD_ACCESS_OPERATION
```

### 17.3 Relationships

```text
PRODUCES_VALUE
CONSUMES_VALUE
OPERAND
RESULT
```

This layer allows source expressions and lowered/compiler values to participate in one common value-flow model.

---

## 18. Definition/use and dataflow ontology

### 18.1 Entities

```text
DEFINITION_EVENT
USE_EVENT
```

### 18.2 Definition categories

```text
initialization
assignment
parameter_initialization
mutation
return_assignment
merged_definition
```

### 18.3 Use categories

```text
read
argument
condition
return
receiver
index
dereference
```

### 18.4 Relationships

```text
DEFINES
USES
REACHES
DEF_USE
DATA_DEP
VALUE_FLOWS_TO
```

### 18.5 Derived dataflow facts

```text
REACHING_DEFINITION
LIVE_AT
KILLS_DEFINITION
```

The graph MAY include SSA-like derived structures, but SHALL NOT require source languages themselves to be represented in SSA form.

---

## 19. Abstract memory and state-location ontology

### 19.1 Location entities

```text
LOCAL_LOCATION
PARAMETER_LOCATION
GLOBAL_LOCATION
STATIC_LOCATION

FIELD_LOCATION
INSTANCE_MEMBER_LOCATION
CLASS_MEMBER_LOCATION

INDEXED_LOCATION
CONTAINER_ELEMENT_LOCATION

DEREFERENCED_LOCATION
HEAP_OBJECT
UNKNOWN_MEMORY
```

### 19.2 Access paths

Memory paths SHALL be representable structurally.

Conceptual example:

```text
base
  .field
  [index]
  *
  downcast
  subslice
```

An expression such as:

```text
object.x.y[i]
```

SHOULD be representable as a structured access path rather than an opaque string.

### 19.3 Memory relationships

```text
READS
WRITES
MUTATES
INITIALIZES
DEINITIALIZES
TAKES_ADDRESS
DEREFERENCES
```

---

## 20. Alias and points-to ontology

### 20.1 Relationships

```text
MUST_ALIAS
MAY_ALIAS
DOES_NOT_ALIAS
POINTS_TO
MAY_POINT_TO
```

`DOES_NOT_ALIAS` SHALL only be asserted when proven under the analysis model.

### 20.2 Derived structures

```text
ALIAS_SET
POINTS_TO_SET
```

The ontology favors conservative uncertainty over false precision.

---

## 21. Program-point state ontology

Objective state facts MAY include:

```text
INITIALIZED_AT
UNINITIALIZED_AT
MAY_BE_UNINITIALIZED_AT

KNOWN_CONSTANT_AT
POSSIBLE_CONSTANT_SET

NULL_AT
NON_NULL_AT
MAY_BE_NULL_AT

VARIANT_AT
POSSIBLE_VARIANTS_AT
```

These facts are relative to a program point and SHALL retain the corresponding control-flow location.

---

## 22. Effect ontology

Effects describe observable program behavior without evaluating whether that behavior is desirable.

### 22.1 Direct effect kinds

```text
READS_STATE
WRITES_STATE
MUTATES_ARGUMENT

ALLOCATES
DEALLOCATES

MAY_RAISE
MAY_PANIC
MAY_UNWIND

PERFORMS_IO
MAY_BLOCK

SPAWNS_TASK
SPAWNS_THREAD
AWAITS

ACQUIRES_LOCK
RELEASES_LOCK

CALLS_FOREIGN_CODE
USES_UNSAFE_OPERATION
USES_INLINE_ASSEMBLY
```

### 22.2 Direct versus transitive effects

The ontology SHOULD distinguish:

```text
DIRECT_EFFECT
TRANSITIVE_EFFECT
```

For example:

```text
DIRECTLY_WRITES(function, location)
TRANSITIVELY_WRITES(function, location)
```

---

## 23. Exceptional-flow ontology

### 23.1 Entities

```text
RAISE_SITE
PANIC_SITE
ASSERT_SITE
HANDLER
CATCH_CLAUSE
EXCEPT_CLAUSE
FINALLY_REGION
CLEANUP_REGION
UNWIND_EDGE
```

### 23.2 Relationships

```text
RAISES
MAY_RAISE
HANDLED_BY
MAY_BE_HANDLED_BY
PROPAGATES_TO
UNWINDS_TO
EXECUTES_CLEANUP
```

This layer represents mechanism only. It does not assign risk.

---

## 24. Resource-lifetime ontology

### 24.1 Entities

```text
RESOURCE_CREATION
RESOURCE_ACQUISITION
RESOURCE_USE
RESOURCE_RELEASE
RESOURCE_DROP
```

### 24.2 Relationships

```text
CREATES_RESOURCE
ACQUIRES_RESOURCE
OWNS_RESOURCE
TRANSFERS_RESOURCE
USES_RESOURCE
RELEASES_RESOURCE
DROPS_RESOURCE
```

No `RESOURCE_LEAK` conclusion is required by this specification.

---

## 25. Async and concurrency ontology

### 25.1 Entities

```text
COROUTINE
FUTURE
GENERATOR
TASK
THREAD
CHANNEL
LOCK
```

### 25.2 Relationships

```text
CREATES_FUTURE
SPAWNS
AWAITS
YIELDS
RESUMES
JOINS

SENDS
RECEIVES

ACQUIRES
RELEASES
```

### 25.3 Derived concurrency relationships

Where supported:

```text
MAY_RUN_CONCURRENTLY_WITH
HAPPENS_BEFORE
```

These SHALL remain mechanically justified relations, not inferred performance or correctness judgments.

---

## 26. Closure and capture ontology

### 26.1 Entities

```text
CLOSURE
CAPTURE
CAPTURED_SYMBOL
```

### 26.2 Relationships

```text
CAPTURES
CAPTURED_FROM
CAPTURES_BY_VALUE
CAPTURES_BY_REFERENCE
CAPTURES_MUTABLY
```

Language profiles define which capture modes are meaningful.

---

## 27. Generated and lowered-code ontology

### 27.1 Entities

```text
SOURCE_ENTITY
GENERATED_ENTITY
EXPANSION
LOWERED_ENTITY
COMPILER_INSTANCE
```

### 27.2 Relationships

```text
GENERATED_FROM
EXPANDED_FROM
LOWERS_TO
CORRESPONDS_TO
SPECIALIZES
MONOMORPHIZES
```

The ontology SHALL preserve the ability to map generated/lowered entities back to source-authored constructs where the provider exposes such provenance.

---

## 28. Generic and specialization ontology

### 28.1 Entities

```text
GENERIC_DECLARATION
GENERIC_PARAMETER
GENERIC_ARGUMENT
SPECIALIZATION
```

### 28.2 Relationships

```text
HAS_GENERIC_PARAMETER
TYPE_ARGUMENT
CONST_ARGUMENT
LIFETIME_ARGUMENT
INSTANTIATES
SPECIALIZES
SUBSTITUTES
```

A generic declaration and a concrete specialization SHALL remain distinct entities.

---

## 29. Objective graph-analysis facts

Mechanically computed graph structure is explicitly part of the ontology.

### 29.1 Generic graph facts

```text
IN_DEGREE
OUT_DEGREE

SCC_ID
SCC_SIZE
IS_RECURSIVE_SCC

CONNECTED_COMPONENT

TRANSITIVELY_REACHES
TRANSITIVELY_REACHED_BY

SHORTEST_GRAPH_DISTANCE
```

These SHOULD identify the graph projection on which they were computed.

### 29.2 Call-graph-specific facts

```text
DIRECT_CALLER
DIRECT_CALLEE
TRANSITIVE_CALLER
TRANSITIVE_CALLEE

CALL_SCC
RECURSIVE_FUNCTION
MUTUALLY_RECURSIVE_SET
```

### 29.3 Control-graph-specific facts

```text
DOMINATES
POST_DOMINATES
CONTROL_DEPENDENT_ON
BACK_EDGE
LOOP_MEMBER
CFG_SCC_MEMBER
```

---

## 30. Objective structural metrics

Mechanically derived scalar measurements MAY be included.

Recommended examples:

```text
statement_count
expression_count
basic_block_count
cfg_edge_count

cyclomatic_complexity
loop_count
loop_nesting_depth

direct_call_count
unique_direct_callee_count
direct_caller_count

parameter_count
generic_parameter_count

branch_count
return_count
raise_or_panic_count

read_count
write_count
```

The ontology explicitly excludes evaluative labels derived from these metrics.

For example, it MAY store:

```text
cyclomatic_complexity = 18
```

but SHALL NOT canonically infer:

```text
HIGH_COMPLEXITY = true
```

---

## 31. Interprocedural summary ontology

Interprocedural summaries are deterministic compressed facts intended to reduce repeated traversal costs.

### 31.1 Recommended callable summary contents

```text
direct_callees
may_callees

direct_reads
transitive_reads

direct_writes
transitive_writes

parameters_read
parameters_mutated

possible_return_types
possible_return_values_or_value_classes

may_allocate
may_deallocate

may_perform_io
may_block

may_raise
may_panic
may_unwind

may_spawn
may_await

may_use_unsafe
may_cross_ffi

unknown_effect
```

### 31.2 Summary rules

A summary SHALL remain attributable to the underlying fact families from which it was computed.

A summary MUST NOT replace the lower-level facts needed to explain or recompute it.

---

## 32. Explicit unknown ontology

### 32.1 Unknown entity classes

```text
UNKNOWN_SYMBOL
UNKNOWN_TYPE
UNKNOWN_CALL_TARGET
UNKNOWN_MEMBER
UNKNOWN_MODULE
UNKNOWN_MEMORY
UNKNOWN_EFFECT
UNKNOWN_EXTERNAL_IMPLEMENTATION
```

### 32.2 Unknown relationships

Examples:

```text
MAY_CALL -> UNKNOWN_CALL_TARGET
MAY_RESOLVE_MEMBER -> UNKNOWN_MEMBER
REFERS_TO -> UNKNOWN_SYMBOL
MAY_POINT_TO -> UNKNOWN_MEMORY
```

Unknown facts SHALL be preserved instead of represented as absent edges.

---

# Part II — Python Ontology Profile

## 33. Python scope ontology

Python-specific scope kinds include:

```text
MODULE_SCOPE
FUNCTION_SCOPE
CLASS_SCOPE
LAMBDA_SCOPE
COMPREHENSION_SCOPE
ANNOTATION_SCOPE
TYPE_PARAMETER_SCOPE
```

The ontology SHALL preserve Python's language-specific scoping semantics rather than approximating every scope as a generic block scope.

---

## 34. Python binding ontology

Python-specific binding kinds include:

```text
LOCAL_BINDING
PARAMETER_BINDING
GLOBAL_BINDING
NONLOCAL_BINDING

IMPORT_BINDING

CLASS_MEMBER_BINDING
INSTANCE_MEMBER_BINDING

COMPREHENSION_TARGET
LOOP_TARGET
WITH_TARGET
EXCEPTION_TARGET
MATCH_CAPTURE
WALRUS_BINDING

TYPE_PARAMETER_BINDING
TYPE_ALIAS_BINDING

FREE_VARIABLE
CELL_VARIABLE
BUILTIN_REFERENCE
```

The graph SHOULD preserve whether a binding is declaration-like, assignment-like, imported, captured, or synthesized.

---

## 35. Python type ontology extensions

Python-specific type kinds SHOULD include:

```text
ANY
UNKNOWN
NEVER
NONE_TYPE

CLASS_INSTANCE
CLASS_OBJECT
MODULE_TYPE

LITERAL_TYPE
UNION_TYPE
INTERSECTION_TYPE

CALLABLE
BOUND_METHOD
OVERLOAD

TYPE_VAR
PARAM_SPEC
TYPE_VAR_TUPLE
SELF

PROTOCOL
TYPED_DICT
TYPE_ALIAS

ANNOTATED
UNPACK
TYPE_GUARD
TYPE_IS
```

Where possible, the graph SHOULD preserve provenance distinguishing:

- explicit annotation;
- inferred type;
- contextual expected type;
- narrowing result.

---

## 36. Python object-model ontology

Python-specific relationships include:

```text
MRO_PRECEDES
METACLASS_OF

DESCRIPTOR_FOR
PROPERTY_FOR
GETTER_FOR
SETTER_FOR
DELETER_FOR

CLASS_METHOD_OF
STATIC_METHOD_OF

RESOLVES_ATTRIBUTE
MAY_RESOLVE_ATTRIBUTE
```

### 36.1 Attribute resolution

Attribute/member resolution SHOULD preserve:

- receiver type;
- declaring class;
- MRO resolution;
- descriptor/property semantics;
- instance versus class binding;
- dynamic/unknown fallback.

---

## 37. Python call ontology

Python-specific call kinds include:

```text
DIRECT_FUNCTION_CALL
BOUND_METHOD_CALL
CLASS_METHOD_CALL
STATIC_METHOD_CALL

CONSTRUCTOR_CALL
CALLABLE_OBJECT_CALL

SUPER_CALL
DECORATOR_APPLICATION

ASYNC_FUNCTION_CALL
GENERATOR_CREATION
```

Constructor call semantics MAY separately model:

```text
__new__
__init__
```

when statically resolvable.

Callable-object invocation MAY separately resolve:

```text
__call__
```

---

## 38. Python dynamic-semantics facts

The graph SHOULD explicitly represent syntax/semantics associated with dynamic behavior.

```text
USES_EVAL
USES_EXEC

USES_GETATTR
USES_SETATTR
USES_DELATTR
USES___DICT__

USES_GLOBALS
USES_LOCALS
USES_VARS

DYNAMIC_IMPORT
STAR_IMPORT

MONKEY_PATCH_WRITE
DYNAMIC_ATTRIBUTE_WRITE
```

These are factual observations.

The ontology SHALL NOT infer a generic negative quality or danger label from them.

Unknown-target relationships SHOULD be retained when these constructs prevent complete static resolution.

---

## 39. Python decorator ontology

Decorators SHALL be represented through at least two distinct semantic relationships:

```text
DECORATED_BY
DECORATOR_APPLICATION_CALL
```

The first captures the structural declaration relationship.

The second captures the executable semantics of decorator application.

Framework-generated behavior MAY be represented using synthesized semantic entities when a semantic provider can identify them objectively.

---

## 40. Python pattern-matching ontology

Entities:

```text
MATCH_STATEMENT
MATCH_CASE
PATTERN
PATTERN_BINDING
GUARD
```

Relationships SHOULD connect:

- match subject;
- case;
- pattern;
- bindings introduced by the pattern;
- guard;
- corresponding control-flow edges.

---

## 41. Python comprehension ontology

A comprehension SHOULD represent:

```text
COMPREHENSION
COMPREHENSION_SCOPE
GENERATOR_CLAUSE
COMPREHENSION_TARGET
COMPREHENSION_ITERABLE
COMPREHENSION_FILTER
COMPREHENSION_RESULT
```

Comprehension-local bindings SHALL remain distinct from surrounding-scope bindings.

---

## 42. Python context-manager ontology

Entities and relationships MAY include:

```text
CONTEXT_MANAGER
ENTER_CALL
EXIT_CALL
ASYNC_ENTER_CALL
ASYNC_EXIT_CALL
```

with statically resolved targets where available.

The graph SHOULD preserve exceptional-control relationships through context-manager exit logic where derivable.

---

## 43. Python async and generator ontology

Python SHOULD distinguish:

```text
ASYNC_FUNCTION
COROUTINE_OBJECT
AWAIT_SITE
ASYNC_ITERATOR
ASYNC_CONTEXT_MANAGER

GENERATOR_FUNCTION
GENERATOR_OBJECT
YIELD_SITE
YIELD_FROM_SITE
```

Calling an async or generator function and executing/resuming its body SHALL remain distinct facts.

---

# Part III — Rust Ontology Profile

## 44. Rust source-semantic entities

Rust-specific source entities include:

```text
CRATE
MODULE
USE_DECLARATION

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
ASSOCIATED_FUNCTION
ASSOCIATED_TYPE
ASSOCIATED_CONST

TYPE_ALIAS
OPAQUE_TYPE

CONST
STATIC

MACRO_DECLARATION
MACRO_INVOCATION
MACRO_EXPANSION

EXTERN_BLOCK
FOREIGN_FUNCTION
```

---

## 45. Rust declaration properties

Rust semantic declarations MAY expose:

```text
visibility
mutability
unsafe
async
const
extern
ABI
variadic
defaultness
repr
```

Attributes SHALL be represented as source facts and associated with the declaration or syntax construct to which they apply.

---

## 46. Rust generic ontology

Rust generic entities include:

```text
TYPE_PARAMETER
LIFETIME_PARAMETER
CONST_PARAMETER
WHERE_PREDICATE
TRAIT_BOUND
LIFETIME_BOUND
```

Relationships include:

```text
BOUNDED_BY
OUTLIVES
IMPLEMENTS
ASSOCIATED_WITH
```

Generic arguments SHALL remain structured:

```text
TYPE_ARGUMENT
LIFETIME_ARGUMENT
CONST_ARGUMENT
```

---

## 47. Rust type ontology extensions

Rust-specific normalized type kinds SHOULD include:

```text
BOOL
CHAR
INTEGER
FLOAT
STR
NEVER

ADT
TUPLE
ARRAY
SLICE

REFERENCE
RAW_POINTER

FN_DEF
FN_POINTER

CLOSURE_TYPE
COROUTINE_TYPE

DYN_TRAIT
OPAQUE_TYPE

GENERIC_PARAMETER
ASSOCIATED_TYPE
PROJECTION_TYPE
TYPE_ALIAS
```

Additional type facts MAY include:

```text
mutability
region_or_lifetime
generic_arguments
ABI
```

### 47.1 Type-adjustment relationships

```text
AUTO_DEREF_TO
AUTO_REF_TO
UNSIZES_TO
COERCES_TO
REIFIES_FN_POINTER
```

These SHOULD be preserved where rustc exposes them reliably.

---

## 48. Rust MIR ontology

MIR is a semantic/control-flow layer attached to Rust source-level definitions.

### 48.1 MIR entities

```text
MIR_BODY
MIR_LOCAL
MIR_BASIC_BLOCK
MIR_STATEMENT
MIR_TERMINATOR

PLACE
PLACE_PROJECTION

OPERAND
RVALUE

MIR_CALL_SITE
DROP_SITE
ASSERT_SITE
```

### 48.2 MIR ownership

Every MIR entity SHALL be attributable to its MIR body and source-level semantic owner where correspondence exists.

### 48.3 Raw MIR variants

Each MIR statement, terminator, operand, rvalue, and projection SHOULD preserve its provider-native/raw variant identifier in addition to normalized meaning.

---

## 49. Rust place and projection ontology

A Rust MIR `Place` SHALL be represented as:

```text
base_local
projection_1
projection_2
...
```

Projection kinds include:

```text
DEREF
FIELD
INDEX
CONSTANT_INDEX
SUBSLICE
DOWNCAST
OPAQUE_CAST
```

Thus a place such as conceptually:

```text
x.foo[i].bar
```

is a structured memory/access path rather than a serialized string.

---

## 50. Rust MIR state-transition ontology

The following SHALL remain semantically distinct:

```text
READ
WRITE

COPY
MOVE

BORROW_SHARED
BORROW_MUT
REBORROW

RAW_ADDRESS_OF

STORAGE_LIVE
STORAGE_DEAD

INIT
DEINIT

DROP
```

In particular:

> `MOVE` and `COPY` MUST NOT be collapsed.

---

## 51. Rust ownership and borrow ontology

Where compiler-semantic facts support it, the graph SHOULD represent:

```text
OWNS
MOVED_TO
COPIED_TO

BORROWS_SHARED
BORROWS_MUTABLY
REBORROWS

LOAN
LOAN_CREATED_AT
LOAN_LIVE_AT

REGION
OUTLIVES
REGION_CONTAINS

MOVE_PATH
```

### 51.1 Program-point ownership state

Derived facts MAY include:

```text
OWNED_AT
MOVED_AT
BORROWED_SHARED_AT
BORROWED_MUT_AT
UNINITIALIZED_AT
```

Such facts SHALL retain program-point identity.

---

## 52. Rust call and executable-instance ontology

### 52.1 Rust dispatch/call kinds

```text
DIRECT_FN
STATIC_TRAIT_DISPATCH
DYNAMIC_TRAIT_DISPATCH

FN_POINTER
CLOSURE

INTRINSIC
FOREIGN_CALL

DROP_GLUE
COMPILER_SHIM
COROUTINE_RESUME
UNKNOWN_INDIRECT
```

### 52.2 Definition versus instance

The ontology SHALL distinguish:

```text
DECLARED_FUNCTION
MONO_INSTANCE
```

Relationships include:

```text
MONOMORPHIZES
TYPE_ARGUMENT
LIFETIME_ARGUMENT
CONST_ARGUMENT
CALLS_INSTANCE
```

A generic source body SHOULD remain represented once at the source-semantic level even when multiple executable specializations exist.

---

## 53. Rust trait and dynamic-dispatch ontology

### 53.1 Entities

```text
TRAIT
TRAIT_METHOD
IMPL
IMPL_METHOD
DYN_TRAIT_TYPE
VTABLE
VTABLE_ENTRY
```

### 53.2 Relationships

```text
IMPLEMENTS_TRAIT
IMPLEMENTS_METHOD

INVOKES_TRAIT_CONTRACT
STATICALLY_RESOLVES_TO

UNSIZES_TO_DYN
USES_VTABLE

MAY_DISPATCH_TO
```

The graph SHALL distinguish:

- static trait resolution;
- dynamic trait dispatch;
- conservative candidate targets.

---

## 54. Rust macro ontology

### 54.1 Entities

```text
MACRO_DEFINITION
MACRO_INVOCATION
EXPANSION
EXPANDED_ITEM
```

### 54.2 Relationships

```text
INVOKES_MACRO
EXPANDS_TO
GENERATED_FROM
SOURCE_CORRESPONDENCE
```

Where exposed by the compiler/frontend, hygiene and expansion-context information SHOULD be retained.

---

## 55. Rust drop and destruction ontology

### 55.1 Entities

```text
DROP_SITE
DROP_IMPL
DROP_GLUE
```

### 55.2 Relationships

```text
DROPS
INVOKES_DROP_IMPL
INVOKES_DROP_GLUE
DROPS_FIELD
```

Compiler-generated destruction is executable semantics and MUST NOT be omitted merely because no explicit source `drop()` call exists.

---

## 56. Rust async and coroutine-lowering ontology

### 56.1 Entities

```text
ASYNC_FUNCTION
FUTURE_TYPE
COROUTINE_BODY
COROUTINE_STATE
SUSPEND_POINT
RESUME_POINT
```

### 56.2 Relationships

```text
LOWERS_TO_COROUTINE
CREATES_FUTURE
HAS_STATE
SUSPENDS_AT
RESUMES_AT
```

The ontology SHALL preserve:

```text
calling async function
!=
executing async body
```

---

## 57. Rust unsafe and FFI ontology

### 57.1 Entities

```text
UNSAFE_BLOCK
UNSAFE_FUNCTION
RAW_POINTER_DEREF
RAW_ADDRESS
INLINE_ASM
FOREIGN_FUNCTION
EXTERN_BLOCK
```

### 57.2 Relationships

```text
CONTAINS_UNSAFE_OPERATION
CALLS_FOREIGN
CROSSES_FFI
```

These are objective facts only.

---

## 58. Rust constants, statics, and CTFE ontology

Rust MAY expose:

```text
CONST_ITEM
STATIC_ITEM
CONST_VALUE
CTFE_RESULT
CONST_ALLOCATION
```

Relationships:

```text
REFERENCES_CONST
REFERENCES_STATIC
EVALUATES_TO
REFERENCES_ALLOCATION
```

This information SHOULD be included where compiler APIs expose it reliably and structurally.

---

# Part IV — Derived Fact Families

## 59. Derived facts versus source facts

Derived facts are permitted when they are mechanically reproducible from lower-level facts.

Examples:

```text
DOMINATES
POST_DOMINATES
CONTROL_DEPENDENT_ON
REACHING_DEFINITION
TRANSITIVELY_REACHES
CALL_SCC
MAY_ALIAS
LOOP_NESTING_DEPTH
```

A derived fact SHOULD identify:

- the fact family from which it was computed;
- the graph projection or owner over which it was computed;
- the derivation method or analysis version where operationally useful.

This metadata is supporting provenance, not historical analysis.

---

## 60. Recommended graph projections

The same underlying CPG MAY produce multiple graph projections.

### 60.1 Syntax graph

Nodes:

- syntax nodes.

Edges:

- AST containment;
- lexical adjacency.

### 60.2 Symbol graph

Nodes:

- declarations;
- references;
- bindings;
- scopes.

Edges:

- declares;
- binds;
- refers-to;
- captures;
- shadows.

### 60.3 Type graph

Nodes:

- semantic types;
- declarations.

Edges:

- type-of;
- subtype;
- generic argument;
- implementation;
- coercion.

### 60.4 Call graph

Nodes:

- callable definitions and/or callable instances.

Edges:

- exact calls;
- possible calls.

Primary source:

- call-site facts.

### 60.5 Control-flow graph

Nodes:

- blocks/instructions.

Edges:

- normal and exceptional CFG edges.

### 60.6 Dataflow graph

Nodes:

- definition events;
- use events;
- values;
- locations.

Edges:

- reaches;
- def-use;
- value-flow;
- data dependency.

### 60.7 Memory/alias graph

Nodes:

- abstract locations;
- pointers/references;
- values.

Edges:

- points-to;
- may-alias;
- must-alias.

### 60.8 Ownership graph

Primarily Rust-specific.

Nodes:

- values;
- places;
- loans;
- regions;
- move paths.

Edges:

- owns;
- moves;
- copies;
- borrows;
- reborrows;
- outlives.

---

# Part V — Fact Metadata and Conformance

## 61. Universal fact metadata

Every first-class fact SHALL carry a common metadata envelope. Optionality is defined per fact kind, not left to each provider.

```text
fact_id                 required
workspace_id            required
analysis_context_id     required; `context:source` for context-independent facts
fact_form               relation | property | entity existence
fact_kind               required canonical registry code
subject_entity_id       required
object_entity_id        required only for relation facts
typed_value             required only for property facts
program_point_entity_id optional
owner_id                required
language                required
source_file_id          optional
source_span             optional
producer                required
producer_version        required
certainty               required
resolution              required
directness              required
derivation_kind         optional
supporting_fact_ids     optional
source_generation       required operational fence
```

Snapshot/publication IDs are infrastructural and SHALL not be interpreted as semantic history. They are nevertheless required in storage and response provenance so a fact can be audited against the exact present-state input.

### 61.1 Property-level provenance

A canonical entity row cannot stand in for multiple independently produced propositions. Names, qualified names, declared types, computed types, flags, source correspondences, and semantic classifications SHALL be represented as property facts whenever they have independent source, certainty, or conflict semantics.

Denormalized entity columns MAY exist only when they are generated from the selected canonical property fact and can be reconstructed deterministically.

---

## 62. Canonical evidence, resolution, directness, and completeness registries

The following registries are orthogonal. Implementations SHALL NOT collapse them into one confidence score.

### 62.1 Evidence certainty

| Code | Name | Meaning |
|---:|---|---|
| 10 | `SOURCE_EXACT` | Exact source or syntax observation from the pinned bytes |
| 20 | `COMPILER_EXACT` | Exact compiler/lowered fact for the pinned build context |
| 30 | `STATIC_SEMANTIC` | Deterministic static semantic result from an analyzer |
| 40 | `SOUND_MAY` | Conservative set member required for soundness |
| 50 | `MODELLED` | Deterministic model-pack fact, not directly observed |
| 60 | `HEURISTIC` | Reproducible heuristic that is not sound/exact |
| 70 | `UNRESOLVED` | Explicit unknown or unresolved proposition |

### 62.2 Resolution class

| Code | Name | Meaning |
|---:|---|---|
| 10 | `EXACT` | One exact endpoint/value is established |
| 20 | `STATICALLY_RESOLVED` | Statically selected under the indexed semantic context |
| 30 | `SOUND_POSSIBLE` | Candidate belongs to a conservative sound set |
| 40 | `POSSIBLE` | Candidate is possible but not proven sound/exhaustive |
| 50 | `MODELLED` | Endpoint/value comes from an external semantic model |
| 60 | `HEURISTIC` | Endpoint/value comes from a heuristic |
| 70 | `UNRESOLVED` | Unknown remainder or unresolved endpoint is retained |
| 80 | `UNAVAILABLE` | The fact family could not be produced for this snapshot |
| 90 | `NOT_APPLICABLE` | The resolution concept does not apply |

### 62.3 Directness

| Code | Name | Meaning |
|---:|---|---|
| 10 | `DIRECT` | Directly authored/extracted relationship or property |
| 20 | `TRANSITIVE` | Mechanically derived closure/path relationship |
| 30 | `SUMMARY` | Deterministic summary of another fact set |
| 40 | `NOT_APPLICABLE` | Directness is not meaningful for this fact |

### 62.4 Completeness

| Code | Name | Meaning |
|---:|---|---|
| 10 | `COMPLETE` | Complete for the declared owner/scope/context/profile |
| 20 | `PARTIAL` | Some supported facts are present and the remainder is characterized |
| 30 | `INDETERMINATE` | Missing information prevents a completeness claim |
| 40 | `UNAVAILABLE` | The fact family is unavailable for the scope |
| 50 | `NOT_APPLICABLE` | The fact family does not apply |

### 62.5 Canonical owner-capability state

| Code | Name |
|---:|---|
| 10 | `CURRENT` |
| 20 | `PENDING` |
| 30 | `INVALIDATED` |
| 40 | `PARTIAL` |
| 50 | `UNAVAILABLE_PARSE` |
| 60 | `UNAVAILABLE_COMPILE` |
| 70 | `UNAVAILABLE_PROVIDER` |
| 80 | `UNAVAILABLE_DERIVATION` |
| 90 | `EXCLUDED` |
| 100 | `UNSUPPORTED` |
| 110 | `REMOVED` |
| 120 | `NOT_APPLICABLE` |

### 62.6 Canonical provider-run state

| Code | Name | Meaning |
|---:|---|---|
| 10 | `QUEUED` | Accepted and waiting for admission/execution |
| 20 | `RUNNING` | Provider work is active |
| 30 | `SUCCEEDED` | Terminal complete provider output accepted |
| 40 | `PARTIAL` | Terminal provider output accepted with explicit partial capability |
| 50 | `FAILED` | Provider returned a terminal domain failure |
| 60 | `TIMED_OUT` | Provider deadline expired |
| 70 | `CANCELLED` | Cancellation was acknowledged |
| 80 | `SUPERSEDED` | A newer source/context generation made the run unnecessary |
| 90 | `CRASHED` | Provider process terminated unexpectedly |
| 100 | `PROTOCOL_ERROR` | Provider violated the accepted stream/manifest protocol |
| 110 | `STALE_RESULT` | Output completed for a stale source/context generation and was rejected |
| 120 | `STALE_GIT_BASELINE` | Output depended on a Git candidate baseline that became invalid |

### 62.7 Canonical query execution, availability, completeness, freshness, limit, and dependency states

```text
QueryExecutionState:
  ACCEPTED | RUNNING | COMPLETE | FAILED | CANCELLED |
  DEADLINE_EXCEEDED | NOT_EXECUTED_DEPENDENCY

QueryAvailabilityState:
  AVAILABLE | PARTIAL | UNAVAILABLE | NOT_APPLICABLE

CompletenessState:
  COMPLETE | PARTIAL | INDETERMINATE | UNAVAILABLE | NOT_APPLICABLE

FreshnessState:
  AWAITING_CURRENT | CURRENT | POTENTIALLY_STALE | UNAVAILABLE

LimitState:
  NOT_APPLIED | EXPLICIT_LIMIT_REACHED | HARD_LIMIT_REJECTED

DependencyState:
  READY | FAILED_DEPENDENCY | NOT_APPLICABLE
```

These dimensions are orthogonal. For example, execution may be `COMPLETE`, availability `PARTIAL`, completeness `INDETERMINATE`, and freshness `CURRENT` in the same result.

### 62.8 Canonical durable publication and serving activation states

```text
DurablePublicationState:
  STAGING | VALIDATING | VALIDATED | COMMITTING | COMPLETE | FAILED | ABANDONED

ServingActivationState:
  BUILDING | VALIDATING | READY | ACTIVE | RETIRED | FAILED
```

A durable publication state SHALL never be used as the active query-snapshot state.

### 62.9 Canonical source trust, event-stream health, and Git acceleration states

```text
SourceTrustState:
  UNVERIFIED | VERIFYING | CURRENT | POTENTIALLY_STALE | UNAVAILABLE

EventStreamHealth:
  HEALTHY | RESCAN_REQUIRED | DEGRADED | UNAVAILABLE

GitAccelerationStatus:
  NOT_A_GIT_WORKTREE | GIT_UNAVAILABLE | GIT_READY | GIT_METADATA_DIRTY |
  GIT_SCANNING | GIT_OPERATION_IN_PROGRESS | GIT_BULK_RECONCILING | GIT_DEGRADED
```

`CURRENT` source trust and `HEALTHY` event-stream health are independent. Git acceleration may be `DEGRADED` while source trust is `CURRENT` after authoritative generic reconciliation.

### 62.10 Registry governance

The numeric registry is a generated machine-readable suite artifact. Codes are append-only; names are never reassigned. Every Arrow enum dimension, Delta code column, RPC/Pydantic enum, semantic JSON Schema, and lifecycle state mapper SHALL be generated from or conformance-tested against the same registry. Operational-only states remain infrastructure metadata and are not semantic program facts merely because their codes are defined here.

For the initial 1.3 allocation, an enum block that lists canonical names but no
explicit numeric values assigns codes in declaration order starting at `10`
and incrementing by ten within that registry domain, matching Suite Manifest
AC-G-06. The generated registry materializes those numbers
and freezes them append-only. Later prose reordering does not renumber an
accepted registry.

Public phrases such as “exact,” “statically resolved,” “sound possible,” and “possible but not proven” SHALL map to these exact registries.

---

## 63. Ownership of facts

Every fact SHALL have a deterministic replacement/recomputation owner. Typical owner kinds are:

```text
source_file
module
scope
callable
class_or_type
MIR_body
crate_or_build_unit
workspace_global_derivation
```

`workspace_global_derivation` is permitted only for facts whose scope cannot be replaced safely by a smaller owner. Ownership does not create history; it defines current-state replacement, capability aggregation, and derivation invalidation.

---

## 64. Required identity and public encoding rules

### 64.1 Internal ID representation

Canonical IDs are application-owned 128-bit values. The BLAKE3 preimage SHALL use:

```text
domain tag
version byte
length-prefixed workspace_id
length-prefixed analysis_context_id or `context:source`
length-prefixed canonical semantic key fields
```

Sets are sorted before serialization; integers use fixed-width big-endian encoding; absent optional values have an explicit tag. Display strings and provider-local IDs are never inserted as untyped concatenations.

### 64.2 Public encoding

The public encoding is lowercase ASCII and round-trippable:

```text
workspace:<32-lowercase-hex>
repository:<32-lowercase-hex>
worktree:<32-lowercase-hex>
context:<32-lowercase-hex>
snapshot:<32-lowercase-hex>
publication:<32-lowercase-hex>
entity:<kind-slug>:<32-lowercase-hex>
fact:<kind-slug>:<32-lowercase-hex>
```

The decoder SHALL validate the prefix, kind slug where applicable, exact 32-hex payload, and expected domain before converting to the internal 16-byte value. `context:source` is the sole reserved non-hex literal and maps to the suite-defined source/syntax context constant; no other symbolic payload is accepted.

### 64.3 Source identity

Source identity SHALL include `workspace_id` plus workspace-relative path bytes and the file-identity policy. Source positions alone are not file identity. Display paths are non-authoritative.

### 64.4 Semantic identity

Semantic entities SHALL include the owning workspace and the analysis context whenever language semantics can differ by context. Source position alone is insufficient.

### 64.5 Provider-local IDs

Transient parser IDs, Pyrefly internal IDs, rustc session-local `DefId`, MIR local/block ordinals, and petgraph indexes SHALL never be persisted as canonical global identity.

### 64.6 Anonymous entities

Anonymous entities use owner-relative structural identity plus normalized semantic role. Source-occurrence identity therefore includes the normalized occurrence kind in addition to the structural parent/role/ordinal anchor: incompatible provider observations at the same byte range remain distinct canonical occurrences, while provider-local node handles and provider identity remain excluded. Context-sensitive anonymous entities include the analysis context in their preimage.

### 64.7 Collision handling

The full 256-bit digest SHOULD be retained in collision-diagnostic storage. If two unequal canonical preimages produce the same 128-bit ID, activation SHALL fail with `ID_COLLISION`; the system SHALL not silently re-key one row.

## 65. Required separation of fact types

Implementations conforming to this ontology SHALL distinguish at least the following:

```text
syntax occurrence != semantic entity

declaration != reference

type syntax != semantic type

call expression != call site != callable

declared callable != executable specialization

value != memory location

read != write

copy != move

borrow != raw-address taking

normal CFG edge != exceptional/unwind edge

direct effect != transitive effect

resolved target set != unknown target

source entity != generated/lowered entity
```

---

## 66. Mandatory unknown semantics

A conforming implementation MUST NOT use missing edges as a universal representation of uncertainty.

At minimum, the following unknown concepts SHOULD exist:

```text
UNKNOWN_SYMBOL
UNKNOWN_TYPE
UNKNOWN_CALL_TARGET
UNKNOWN_MEMBER
UNKNOWN_MODULE
UNKNOWN_MEMORY
UNKNOWN_EFFECT
```

Where a dynamic or external construct creates an unresolved candidate space, the graph SHOULD retain an explicit unknown relation.

---

## 67. No evaluative ontology rule

The canonical ontology SHALL NOT include leaf facts whose meaning is primarily an engineering judgment rather than program semantics.

Excluded examples:

```text
SAFE_TO_REFACTOR
RISK_SCORE
TEST_IMPACT
LIKELY_BUG
POOR_DESIGN
VULNERABLE
RECOMMENDATION
HOTSPOT
GOD_OBJECT
SHOULD_INLINE
SHOULD_EXTRACT
```

A downstream analysis system MAY derive such conclusions, but they do not belong to this base CPG specification.

---

# Part VI — Canonical Layer Model

## 68. Ontology layers

The complete present-state CPG is organized into the following conceptual layers:

```text
L0  Source
    text, spans, tokens, comments, documentation

L1  Syntax
    complete raw + normalized syntax structure

L2  Semantic identity
    declarations, symbols, scopes, bindings, references

L3  Type semantics
    types, narrowing, generics, subtyping, coercions

L4  Object semantics
    members, inheritance, traits/protocols, implementation

L5  Invocation
    callable contracts, call sites, arguments, dispatch, targets

L6  Control flow
    blocks, branches, loops, normal and exceptional flow

L7  Values and dataflow
    values, definitions, uses, reaching definitions, dependencies

L8  State and memory
    locations, access paths, reads, writes, aliasing

L9  Ownership and lifetime
    moves, copies, borrows, drops, resource lifetime

L10 Effects
    state mutation, allocation, I/O, raise/panic, async, FFI, unsafe

L11 Generated and lowered semantics
    macros, expansions, MIR, coroutines, shims, specializations

L12 Graph-derived facts
    reachability, SCCs, dominance, post-dominance,
    control dependence, loops, recursion, graph metrics

L13 Semantic summaries
    per-callable reads, writes, calls, effects, returns, unknowns

L14 Explicit uncertainty
    unresolved symbols, types, calls, members, memory, effects
```

---

# Part VII — Recommended Canonical Relationship Inventory

## 69. Structural relationships

```text
CONTAINS
AST_CHILD
ENCLOSES
LEXICALLY_PRECEDES
DEFINED_IN
OWNED_BY
HAS_SCOPE
ENCLOSING_SCOPE
```

## 70. Symbol and binding relationships

```text
DECLARES
BINDS
REFERS_TO
MAY_REFER_TO
SHADOWS
CAPTURES
CAPTURED_FROM
ALIASES
REBINDS
```

## 71. Module and dependency relationships

```text
IMPORTS_MODULE
IMPORTS_SYMBOL
EXPORTS
REEXPORTS
DEFINED_IN_MODULE
DEPENDS_ON_MODULE
```

## 72. Type relationships

```text
DECLARED_TYPE
INFERRED_TYPE
COMPUTED_TYPE
EXPECTED_TYPE
TYPE_OF

PARAMETER_TYPE
RETURN_TYPE
FIELD_TYPE

TYPE_PARAMETER_OF
TYPE_ARGUMENT
LIFETIME_ARGUMENT
CONST_ARGUMENT

SUBTYPE_OF
SUPERTYPE_OF
BOUNDED_BY
CONSTRAINED_BY
OUTLIVES

INSTANTIATES
SPECIALIZES
SUBSTITUTES

COERCES_TO
CASTS_TO
NARROWS_TO
```

## 73. Member relationships

```text
DECLARES_MEMBER
HAS_MEMBER
INHERITS
IMPLEMENTS
IMPLEMENTS_TRAIT
IMPLEMENTS_METHOD
OVERRIDES
OVERRIDDEN_BY
RESOLVES_MEMBER
MAY_RESOLVE_MEMBER
```

## 74. Invocation relationships

```text
CONTAINS_CALL
HAS_CALLEE_EXPRESSION
HAS_RECEIVER
HAS_ARGUMENT
ARGUMENT_BINDS_TO

CALLS_DECLARATION
CALLS_EXACT_TARGET
CALLS_INSTANCE
MAY_CALL
CALLS_UNKNOWN

REFERENCES_CALLABLE
TAKES_FUNCTION_ADDRESS
PASSES_CALLABLE
RETURNS_CALLABLE
```

## 75. Control-flow relationships

```text
CFG_NEXT
CFG_TRUE
CFG_FALSE
CFG_CASE
CFG_LOOP_BACK
CFG_BREAK
CFG_CONTINUE
CFG_RETURN
CFG_EXCEPTION
CFG_UNWIND
CFG_CALL_RETURN
```

## 76. Dataflow relationships

```text
DEFINES
USES
REACHES
DEF_USE
DATA_DEP
VALUE_FLOWS_TO
PRODUCES_VALUE
CONSUMES_VALUE
OPERAND
RESULT
```

## 77. Memory relationships

```text
READS
WRITES
MUTATES
INITIALIZES
DEINITIALIZES
TAKES_ADDRESS
DEREFERENCES

MUST_ALIAS
MAY_ALIAS
DOES_NOT_ALIAS
POINTS_TO
MAY_POINT_TO
```

## 78. Ownership/lifetime relationships

```text
OWNS
MOVED_TO
COPIED_TO
BORROWS_SHARED
BORROWS_MUTABLY
REBORROWS
LOAN_CREATED_AT
LOAN_LIVE_AT
REGION_CONTAINS
OUTLIVES

DROPS
DROPS_FIELD
TRANSFERS_RESOURCE
RELEASES_RESOURCE
```

## 79. Effect relationships

```text
READS_STATE
WRITES_STATE
MUTATES_ARGUMENT
ALLOCATES
DEALLOCATES
MAY_RAISE
MAY_PANIC
MAY_UNWIND
PERFORMS_IO
MAY_BLOCK
SPAWNS_TASK
SPAWNS_THREAD
AWAITS
ACQUIRES_LOCK
RELEASES_LOCK
CALLS_FOREIGN_CODE
USES_UNSAFE_OPERATION
USES_INLINE_ASSEMBLY
```

## 80. Generated/lowered relationships

```text
GENERATED_FROM
EXPANDED_FROM
EXPANDS_TO
LOWERS_TO
CORRESPONDS_TO
MONOMORPHIZES
SPECIALIZES
```

## 81. Derived graph relationships

```text
TRANSITIVELY_REACHES
TRANSITIVELY_REACHED_BY

DOMINATES
STRICTLY_DOMINATES
IMMEDIATE_DOMINATOR

POST_DOMINATES
IMMEDIATE_POST_DOMINATOR

CONTROL_DEPENDENT_ON

BACK_EDGE
LOOP_MEMBER

DIRECT_CALLER
DIRECT_CALLEE
TRANSITIVE_CALLER
TRANSITIVE_CALLEE
```

---

# Part VIII — Conformance Requirements

## 82. Core conformance

A CPG implementation conforms to the **Core Present-State Ontology** when it can represent:

1. source spans and syntax;
2. semantic declarations and references;
3. scopes and bindings;
4. semantic types;
5. call sites and call targets;
6. control flow;
7. values and def-use;
8. state reads/writes;
9. unresolved semantic facts;
10. objective derived graph facts.

## 83. Python profile conformance

A Python-conformant implementation additionally SHOULD represent:

- Python-specific scopes;
- Python binding categories;
- Python inferred and contextual types;
- MRO;
- descriptors/properties;
- constructor and callable-object semantics;
- decorators as executable application;
- comprehensions;
- pattern matching;
- async/generator semantics;
- dynamic constructs and explicit unknowns.

## 84. Rust profile conformance

A Rust-conformant implementation additionally SHOULD represent:

- crates/modules/items;
- generics and lifetimes;
- traits/impls;
- Rust semantic types;
- macros and expansion;
- MIR bodies;
- MIR places and projections;
- reads/writes/moves/copies;
- borrows and loans where available;
- static and dynamic trait dispatch;
- function pointers/closures;
- monomorphic instances;
- drop glue;
- async/coroutine lowering;
- unsafe/FFI operations.

## 85. Advanced derived-fact conformance

An advanced implementation SHOULD compute:

```text
dominators
post-dominators
control dependence
reaching definitions
def-use
liveness
loops
SCCs
recursion
transitive reachability
alias/points-to sets
objective per-callable effect summaries
```

---

# Part IX — Non-Goals

## 86. No agent reasoning inside the ontology

The ontology is deliberately designed so that a downstream LLM can answer questions such as:

- “What code could execute from here?”
- “Where can this value come from?”
- “Who writes this field?”
- “Which implementations could this call dispatch to?”
- “What code mutates this parameter?”
- “What happens during Rust drop?”
- “What conditions govern execution of this write?”

The CPG provides the underlying facts.

It does not canonically answer:

- “Should I change this?”
- “Is this refactor safe?”
- “Which test should I run?”
- “Is this architecture good?”
- “Is this vulnerability exploitable?”

Those are downstream reasoning problems.

---

# Part X — Final Specification Principle

## 87. Governing design rule

Every canonical node, property, and relationship added to the CPG SHOULD satisfy this test:

> **Does this describe an objective fact about the present-state program, or a mechanically derivable property of those facts?**

If yes, it belongs in the ontology.

If instead the proposed fact primarily answers:

> **What should an engineer conclude or do?**

then it belongs in a downstream analysis or agent-reasoning layer, not in the base CPG.

The target architecture is therefore:

```text
Present-state source
        ↓
Raw syntax facts
        ↓
Semantic facts
        ↓
Compiler / IR facts
        ↓
Derived graph facts
        ↓
Deterministic summaries
        ↓
Explicit unknowns
        ↓
Comprehensive CPG fact substrate
        ↓
LLM programming-agent reasoning
```

This specification intentionally stops at the **Comprehensive CPG fact substrate** boundary.

---

# CodeFabric 1.3 architecture-completion contracts

The standalone architecture-completion specification has been propagated into its permanent owners. This part contains the full normative contracts owned by this document: `G-12`, `G-13`, `G-15`, `G-16`, `G-17`, `G-18`, `G-70`, `G-71`, `G-72`, `G-73`, `G-74`, `G-75`, `G-76`, `G-77`. References to a gap ID elsewhere in the synchronized suite resolve to these sections.

## AC-G-12 — File identity across replacement, rename, and move
### Decision

Canonical file identity is present-state and path-based. Rename continuity is operational evidence used to reuse caches, not a hidden historical identity. Therefore a workspace-relative path change changes `file_id` deterministically.

### Contract

```text
file_id = BLAKE3_128(CBEF-v1(
  domain = SOURCE_FILE,
  workspace_id,
  context:source,
  WorkspacePath.comparison_key_bytes
))
```

`comparison_key_bytes` is byte-exact on case-sensitive workspaces and uses the registered case-insensitive volume rule where applicable. The raw/reversible path bytes remain a mutable property of the `SOURCE_FILE` entity and are collision-checked before activation.

Behavior:

| Change | Canonical identity |
|---|---|
| Same path, content modified | Preserve `file_id`; content digest changes |
| Same path, editor atomic replacement/inode replacement | Preserve `file_id` |
| Path renamed or moved | New `file_id` |
| File deleted and later recreated at same path | Same path-derived `file_id`; `source_generation` and content digest prevent stale-row acceptance |
| Case-only rename on a case-insensitive workspace | Preserve the comparison-key identity but update raw/display path; a comparison-key collision is rejected |
| Two paths collapse to one comparison key | Workspace enters `BLOCKED_PATH_COLLISION` until resolved |

The lifecycle engine MAY create an operational `FileContinuityEvidence` record:

```text
old_file_id
new_file_id
wave_id
evidence: same filesystem object | exact digest | Git rename | content similarity
score
unique_mapping
```

This record permits parse/tree/cache transfer but is not a semantic fact and does not alter canonical IDs.

Owner and semantic identity follow their own canonical keys:

- file- and syntax-owner IDs change when their file ID changes;
- module/type/callable identities may remain stable only when their canonical qualified semantic key remains unchanged under the selected context;
- anonymous/source-anchored entities change when their structural owner/path key changes;
- no entity is preserved merely because a heuristic rename score is high.

The move-matching algorithm used for cache reuse is deterministic: same stable filesystem identity first; then exact unique content digest; then a unique Git rename mapping; then a unique content-similarity mapping at or above `0.85`. Ties produce no continuity mapping.
## AC-G-13 — Canonical ID preimage serialization
### Decision

All application-owned IDs use one binary encoding named **CodeFabric Binary Encoding Format version 1 (`CBEF-v1`)**. String concatenation, delimiter-based encoding, debug formatting, and provider-native serialization are prohibited.

### Contract

A CBEF record is:

```text
magic            4 bytes  ASCII "CFID"
format_version   1 byte   0x01
record_domain    2 bytes  unsigned big-endian registry code
field_count      2 bytes  unsigned big-endian
fields           repeated in ascending field-tag order
```

Each field is:

```text
field_tag        2 bytes unsigned big-endian
type_code        1 byte
payload_length   4 bytes unsigned big-endian
payload          exact bytes
```

Core type codes:

| Code | Type | Encoding |
|---:|---|---|
| 0 | absent optional | zero-length payload |
| 1 | bytes | exact bytes |
| 2 | UTF-8 semantic string | UTF-8 after the field-schema-declared normalization rule |
| 3 | raw path | platform code byte followed by exact canonical path bytes |
| 4 | unsigned integer | fixed-width big-endian declared by field schema |
| 5 | signed integer | fixed-width two's-complement big-endian |
| 6 | boolean | one byte `00` or `01` |
| 7 | ID | exact 16 bytes |
| 8 | digest | exact 32 bytes |
| 9 | ordered list | element count plus length-prefixed encoded elements |
| 10 | set | encoded elements sorted bytewise and deduplicated |
| 11 | map | key/value pairs sorted by encoded key |
| 12 | tagged union | variant code followed by variant payload |

Container encodings use unsigned 32-bit big-endian element counts and unsigned
32-bit big-endian per-element, key, value, or variant-payload lengths. The
outer field `payload_length` is computed after semantic-string normalization
and covers the exact emitted payload bytes. Decoders SHALL reject duplicate or
non-ascending field tags, non-minimal/truncated containers, and trailing bytes.

The initial `record_domain` allocation follows the canonical-domain list below
in declaration order starting at `0x0001`. Within each domain recipe, field
tags follow the owner-approved recipe order starting at `0x0001`. A recipe
change after acceptance is a versioned contract change; tags are never reused
or inferred from Rust/Python field layout.

Every UTF-8 field recipe SHALL declare one normalization rule: `NONE`, `NFC`, `NFKC`, `ASCII_LOWER`, or an explicitly named language/ecosystem canonicalizer. The default is `NONE`. Raw source text and path bytes are never Unicode-normalized. Python semantic identifier keys use Python's NFKC identifier semantics while preserving raw spelling separately; Rust semantic identifiers use the exact compiler/application canonical form selected by the Rust identity recipe; registry slugs use `ASCII_LOWER`. Package, module, crate, and qualified-name fields use their ecosystem-specific declared canonicalizer rather than a universal Unicode rule.

ID derivation uses unkeyed BLAKE3-256 over the complete CBEF record; the internal ID is the first 16 bytes. The full 32-byte digest and a compact canonical-preimage diagnostic record SHALL be retained in collision-diagnostic storage.

Public encoding remains the synchronized 1.3 lowercase, round-trippable form:

```text
workspace:<32-lowercase-hex>
repository:<32-lowercase-hex>
worktree:<32-lowercase-hex>
context:<32-lowercase-hex>
snapshot:<32-lowercase-hex>
publication:<32-lowercase-hex>
entity:<kind-slug>:<32-lowercase-hex>
fact:<kind-slug>:<32-lowercase-hex>
```

The decoder validates prefix, kind slug, exact payload width, and expected domain before returning 16-byte identity. `context:source` is the sole symbolic non-hex ID. File, owner, type, artifact, and other domain encodings SHALL receive registry-defined prefixes using the same 32-hex payload rule; a prefix is never inferred from the caller's expected type.

Canonical domain recipes SHALL exist for at least:

```text
WORKSPACE
REPOSITORY
WORKTREE
ANALYSIS_CONTEXT
CONTEXT_SET
SOURCE_FILE
OWNER
ENTITY
RELATION_FACT
PROPERTY_FACT
TYPE
PUBLICATION
SERVING_SNAPSHOT
RESULT_ARTIFACT
SOURCE_CONTEXT
UNKNOWN_REMAINDER
```

The CBEF authority is executable schema, not documentation for callers to reproduce.
The model compiler SHALL generate one recipe-aware builder, validator, and typed field
view per domain. A builder fixes the domain code, field tags, type codes, widths,
normalization, optionality, and declared order; it rejects a missing, extra, duplicate,
mis-typed, or non-normalized field before encoding. Production callers SHALL NOT submit
an arbitrary vector of tagged fields to the generic codec. The generic framing codec may
remain private implementation machinery and SHALL validate the selected domain recipe
when decoding as well as generic frame legality.

The released `ENTITY` recipe remains exactly five fields. Source-occurrence structure—
including normalized occurrence family/kind, file identity, source digest/range,
parent/role/ordinal anchor, and any context-sensitive discriminator—is encoded as the
typed, versioned `semantic_key` payload rather than appended as undocumented CBEF
fields. The released `RELATION_FACT` recipe remains exactly six fields; occurrence or
program-point specificity belongs in the governed `role` tagged union or in the owning
fact recipe, never in ad hoc trailing fields. A genuine recipe change requires an
owner-accepted contract version and regenerated builders before any producer may emit it.

All categorical components inside a semantic key, role, or fact payload—including
occurrence-family codes and persisted provider-node flag bits—come from the governed
enum/flag registries. Generated accessors are the only production spelling of those
allocations; first-party numeric literals and module-local bit assignments are
non-conforming.

A detected unequal-preimage 128-bit collision blocks activation with `ID_COLLISION`. There is no re-salting or silent fallback.
## AC-G-15 — Canonical type algebra
### Decision

Types are represented by a language-neutral tagged algebra with Python- and Rust-specific variants. Provider debug strings are display evidence only and never canonical identity.

### Contract

The canonical algebra includes:

```text
Unknown(reason_code)
Error(diagnostic_class)
AnyDynamic(language_semantics)
NeverBottom
NullNone
Primitive(language, canonical_name)
Nominal(declaration_entity_id, ordered_type_args)
Alias(alias_entity_id, target_type_id, transparency)
Literal(canonical_scalar_value)
Union(sorted_unique_member_type_ids)
Intersection(sorted_unique_member_type_ids)
Tuple(ordered_elements, variadic_tail optional)
Callable(parameters, return_type, receiver, variadic, effects_profile optional)
TypeObject(instance_type)
ClassObject(class_entity_id)
Generic(parameter_binders, body)
TypeVariable(parameter_entity_id, variance, bounds, constraints, default)
AssociatedType(owner_entity_id, item_entity_id, substitutions)
Projection(base_type, trait_or_protocol, member)
Reference(mutability, region, pointee)
RawPointer(mutability, pointee)
Array(element, const_length)
Slice(element)
Mapping(key, value)
Sequence(element)
Structural(required_members)
FunctionDefinition(callable_entity_id, substitutions)
FunctionPointer(signature)
Closure(closure_entity_id, substitutions)
Coroutine(coroutine_entity_id, yield_type, return_type, resume_type)
DynTrait(ordered_principal_and_auto_traits, region)
ImplTrait(opaque_entity_id, bounds)
ConstArgument(canonical_const_algebra)
RecursiveBinder(binder_arity, body)
BoundVariable(de_bruijn_index, variable_index, kind)
```

Rules:

- union/intersection members are flattened, deduplicated, and sorted by encoded type ID;
- Python `Optional[T]` canonicalizes as `Union(T, NullNone)` while its source syntax remains separate;
- aliases remain first-class and expose a separate normalized target; alias transparency never silently erases alias identity;
- bound variables use de Bruijn indexing so alpha-renaming does not change identity;
- Rust free regions and named lifetimes reference canonical parameter entities; inference/session-local region IDs are not persisted;
- const generic values use a typed integer/boolean/char/bytes/unevaluated-expression algebra, not debug text;
- recursive types use explicit binders and back-references, never cyclic JSON object identity;
- unknown and error types are distinct and owner/context scoped;
- type IDs include `workspace_id` and `analysis_context_id` whenever nominal or semantic meaning is context-dependent.

The type-algebra version is compatibility-sensitive. A major change requires reindexing every type-dependent fact.
## AC-G-16 — External dependency identity and body policy
### Decision

Third-party dependencies are endpoint-only by default. Bodies are indexed only when exact dependency source is version-locked, locally available, provider-supported, and explicitly authorized.

### Contract

An external dependency identity contains:

```text
ecosystem: python | cargo | system | builtin
source_kind: registry | git | local-path | bundled | system | unresolved
source_namespace_id: canonical registry/index ID, Git repository+commit digest, authorized local-source digest, or bundled-model ID
package_or_crate_name
resolved_version, immutable revision, or immutable source digest
lockfile/source provenance
module/crate path
qualified symbol path
analysis context
```

Policy classes:

| Class | Behavior |
|---|---|
| Workspace source | Full source and body indexing subject to ACL |
| Workspace member package/crate | Full indexing in the same workspace/context |
| Locked local dependency source explicitly authorized | May index bodies as an external-body partition in the same context |
| Third-party dependency without authorized body | Endpoint declarations/model facts only |
| Standard library/builtins | Signed model-pack declarations and effects; bodies endpoint-only unless explicitly bundled |
| Unresolved external | Explicit unknown external entity/remainder |

No version-1.x query traverses into a separately indexed workspace. When an external body is absent, call/dependency completeness records carry `external_unknown_remainder=true` unless a signed closed model establishes complete behavior for the requested fact family.

External source disclosure is independently authorized. A dependency declaration may be queryable while its source path/text is redacted.

Dependency upgrades produce new external identities and context IDs. Display names alone never merge versions.
## AC-G-17 — Cross-language and FFI linking profile
### Decision

Version 1.x includes a declarative **Static FFI Linking Profile v1** for Python-extension and C-ABI boundaries. It does not perform native binary disassembly or runtime linking analysis.

### Contract

The profile recognizes:

- Rust `extern` declarations and `extern "C"` definitions;
- `#[no_mangle]`, `#[export_name]`, and ABI/calling-convention facts;
- PyO3 `#[pymodule]`, `#[pyfunction]`, `#[pyclass]`, and `#[pymethods]` expansions when compiler/macro evidence is available;
- generated binding manifests produced by Maturin/PyO3 build integration;
- Python imports and member calls resolving to registered extension-module exports;
- C header/binding declarations when represented by an authorized generated-source manifest.

Canonical relations include:

```text
FFI_EXPORTS
FFI_IMPORTS
BINDS_FOREIGN_SYMBOL
WRAPS_FOREIGN_CALLABLE
CALLS_FOREIGN_EXACT
MAY_CALL_FOREIGN
FOREIGN_SIGNATURE_OF
```

An exact cross-language link requires all of:

1. identical extension module logical name;
2. identical exported symbol or generated Python name;
3. compatible ABI/calling convention;
4. matching build/context manifest;
5. compatible normalized parameter/return contract;
6. no conflicting candidate.

Otherwise candidate edges use `SOUND_POSSIBLE`, `POSSIBLE`, or an unknown FFI target. A packaging/build manifest may supply exact symbol mappings but may not override contradictory compiler evidence.

FFI facts use the originating language contexts and an explicit bridge identity; graph traversal SHALL not pretend the Python and Rust entities share one analysis context.
## AC-G-18 — Path canonicalization, display, URI, and ordering
### Decision

Path identity preserves platform-native bytes while exposing a deterministic internal component encoding and a separate display representation.

### Contract

`WorkspacePath` contains:

```text
workspace_id
platform_code
raw_relative_path_bytes
canonical_component_bytes
comparison_key_bytes
case_sensitivity_mode
display_string
display_is_lossy
```

Canonical component encoding uses `/` as an internal separator and percent-escapes literal `/`, `%`, and non-display bytes within components. It is reversible to the original component bytes. It does not resolve symlinks.

Platform rules:

- **Linux/Unix (`platform_code = 0x01`):** raw `OsStr` bytes are authoritative. Default comparison is byte-exact.
- **macOS (`platform_code = 0x02`):** raw filesystem bytes are authoritative. Registration probes the volume for case sensitivity. The comparison key uses Unicode NFD plus full case folding only when the volume is case-insensitive; non-UTF-8 components fall back to byte-exact comparison and trigger a diagnostic.
- **Windows/WTF-8 (`platform_code = 0x03`):** no conforming runtime in 1.x; the registry reserves a WTF-8 path encoding for future use.

All other platform-code values are reserved and rejected by released-profile
decoders.

Two distinct raw paths producing the same comparison key are a blocking collision on case-insensitive workspaces.

Display encoding:

- valid UTF-8 components are shown as text;
- invalid bytes are rendered `%XX` using uppercase hex;
- display strings are never accepted back as authorization or ID input without reversible decoding.

Canonical URI form is:

```text
codefabric://workspace/<workspace-hex>/path/<base64url-without-padding-of-raw-relative-bytes>
```

Deterministic ordering is by `(comparison_key_bytes, raw_relative_path_bytes)`. Source-response ordering, inventory checksums, and clean-rebuild comparisons use this order.

## AC-G-70 — Machine ontology registry
### Decision

The prose ontology is instantiated by versioned machine registries for entity kinds, relation kinds, property kinds, fact kinds, unknowns, and representation/projection roles. The machine registries are the code-generation authority; prose supplies definitions and rationale.

Initial registry allocations are design-contract artifacts, not implementation
defaults. They SHALL be accepted by the ontology owner before generated code
consumes them. When a record has a primary ontology layer, `family_code` is the
layer number plus one (`L0 = 1` through `L14 = 15`); a kind spanning layers
names one primary layer and records other projection memberships separately.

### Contract

Every entity-kind record contains:

```yaml
kind_code: positive append-only integer
kind_slug: lowercase-kebab-case
canonical_name: UPPER_SNAKE_CASE
family_code
parent_kind_code: optional
language_profile: core | python | rust | generated
abstract: boolean
source_or_semantic_or_lowered: source | syntax | semantic | compiler | derived | unknown
allowed_owner_kinds: []
required_property_codes: []
optional_property_codes: []
default_capability_code
storage_extension_table: optional
query_phrase_ids: []
public_display_template_id
introduced/deprecated/replacement versions
```

Every relation-kind record additionally contains:

```text
relation family
allowed subject kind families
allowed object kind families
role/ordinal requirements
self-edge policy
multiplicity/cardinality
symmetry/transitivity properties
certainty/resolution/directness applicability
unknown-remainder relation kind optional
inverse relation kind optional
projection memberships
owner-selection rule
storage mapping
```

Every property-kind record contains the full schema in `G-71`. A fact-kind registry unifies relation-shaped, property-shaped, and entity-existence facts and maps each to statement templates, response roles, and evidence/completeness semantics.

Registry invariants:

- each numeric code and slug is globally unique within its registry domain;
- family/parent graphs are acyclic;
- abstract kinds cannot appear in canonical entity rows;
- subject/object/property constraints are generated into ingestion/query validators;
- every concrete ontology kind maps to at least one capability and storage table;
- every query-visible kind has at least one canonical phrase or is intentionally ID-only;
- deprecated kinds remain readable and map to migration rules; they are not emitted by new snapshots unless compatibility encoding requests them;
- provider-native raw kinds remain in separate provider registries and do not consume canonical ontology codes;
- Tree-sitter raw-kind catalogs are generated from each pinned grammar package's
  `NODE_TYPES` constant and `Language` ABI/inventory, never maintained as copied YAML;
- Ruff raw-kind catalogs are generated through exhaustive matches over the pinned AST,
  token, trivia, and semantic enum surfaces, so adding an upstream variant fails the
  generator build until it is classified;
- authored provider normalization records map generated raw-provider keys to canonical
  ontology kinds through exact overrides, non-overlapping prefix families, then an
  optional canonical default; exact ignores and version-bound unsupported outcomes are
  also explicit. The compiler resolves this precedence over the complete inventory;
  ambiguous prefixes or an unmapped generated key are release failures.

Each generated provider raw-kind catalog records at least:

```text
provider_id and exact provider/package version
language and grammar ABI where applicable
raw_kind_key and upstream spelling
named/visible/supertype flags and field-role inventory where applicable
provider source fingerprint
generation-unit identity and input semantic/source digests
```

Generated raw catalogs live under `contracts/generated/provider-raw-kinds/`; authored
normalization mappings live under `contracts/registry/provider-normalization/`. The
catalog compiler owns the join and generates exhaustive Rust lookups. Fixtures exercise
the generated inventory but are never an authority for its contents.

The ontology bundle contains canonical JSON forms of all registries and generated Rust/Python/Protobuf/Arrow lookup artifacts.
## AC-G-71 — Property schema, value types, cardinality, null, and storage mapping
### Decision

Every canonical property is a first-class property-fact definition with explicit value algebra and cardinality. A null database cell is never used to mean an unknown semantic value.

### Contract

Property registry record:

```yaml
property_code
property_slug
canonical_name
subject_kind_constraints
value_type:
  scalar: boolean | signed_integer | unsigned_integer | decimal | utf8 | bytes | id | enum | digest
  or: entity_ref | type_ref | source_span | canonical_scalar | structured_value
  list_element: optional
cardinality: EXACTLY_ONE | ZERO_OR_ONE | ZERO_OR_MORE | ONE_OR_MORE
required_profiles: []
owner_rule
context_rule: source | semantic | inherited-from-subject
source_span_allowed: boolean
certainty_required: boolean
resolution_applicability
directness_applicability
null_semantics: prohibited
unknown_value_policy
canonicalization_rule
storage:
  canonical_table: property_fact
  denormalized_entity_column: optional
  extension_table_column: optional
query_phrase_ids
statement_template_id
```

Rules:

1. `property_fact.typed_value` is a tagged union generated from the value type; exactly one variant is present.
2. A missing `EXACTLY_ONE` property is a capability/validation gap, not a row with null value.
3. An unknown semantic value is represented by an explicit registered unknown value/entity or an unavailable/indeterminate capability record according to the property definition.
4. Multi-valued properties are one fact row per canonical value unless the property explicitly defines an ordered structured list as its value.
5. `ZERO_OR_ONE` prohibits two active canonical values for the same `(subject, property, context, program point)`; conflicts are retained as evidence/diagnostics and canonical resolution becomes unresolved.
6. Source spans and program points participate in the fact preimage only when the registry says the proposition is occurrence/program-point specific.
7. Denormalized columns are projections of one selected canonical property fact and have no independent provenance.
8. Extension-table columns must declare round-trip mappings to/from property facts or be marked payload-only/non-query-visible.

The schema generator SHALL produce Arrow fields, Delta constraints where expressible, Pydantic/JSON value schemas, ingestion validators, and cardinality integrity queries from this registry.
## AC-G-72 — Mandatory conformance profiles
### Decision

Conformance is advertised through exact named profiles, not general statements that the implementation “supports Python,” “supports Rust,” or “supports advanced analysis.”

### Contract

The initial profiles are:

### `CORE_SOURCE_V1`

Mandatory for every enabled workspace:

```text
workspace/source inventory and byte-safe paths
stable source bytes/digests and source spans
tokens/comments/documentation/directives where language-supported
complete provider-native and normalized CST for supported files
parse-error and missing-syntax entities
source/syntax ownership and capability records
canonical IDs, property facts, provenance, unknowns, source-context retrieval
ServingSnapshot, freshness, canonical query/response, basic entity/fact/source queries
```

### `PYTHON_SEMANTIC_V1`

Requires `CORE_SOURCE_V1` plus, for every applicable Python module in the selected context:

```text
Ruff typed AST and lexical index
scopes, bindings, declarations, references, imports/exports
module resolution
computed and declared types where Pyrefly supports them
member resolution and call-target candidate sets
Python CFG and direct def-use
explicit dynamic/external unknown remainder
```

### `RUST_SEMANTIC_V1`

Requires `CORE_SOURCE_V1` plus, for every applicable Rust build unit/context:

```text
rustc semantic definitions/types/traits/impls/generics
MIR bodies, locals, statements, terminators, normal/unwind CFG
call instances/candidates, moves/copies/borrows/drop facts
macro/generated/lowered correspondence where available
explicit compiler/provider gaps on invalid source
```

### `ADVANCED_FLOW_V1`

Requires one language semantic profile plus:

```text
dominance/post-dominance, control dependence, loops
reaching definitions, liveness, direct def-use/value flow
points-to/alias under BALANCED_V1
direct effect/resource facts
call SCC/recursion and callable direct/transitive summaries
completeness/negative-proof evidence for each family
```

### `SERVING_V1`

Requires:

```text
controlled language and typed PlanSpec
all eight query forms and typed result references
context partitioning, source ACL, cost/limit enforcement
accepted-handle RPC, streaming/artifacts, cancellation
canonical schemas/registries/bundles and conformance fixtures
```

Profile status is `COMPLETE`, `PARTIAL`, `UNAVAILABLE`, or `NOT_APPLICABLE` with missing mandatory capability codes. A product SHALL not advertise a profile as supported merely because some owners implement it. `PARTIAL` profiles are usable only when every query exposes precise owner/capability coverage.

Optional features outside a profile do not change the profile's meaning; a profile minor version is required to add a mandatory capability.
## AC-G-73 — Unknown entities, unknown remainder, and explicit negative facts
### Decision

Unknowns are scoped propositions with deterministic identity. There is no single global “unknown node” that collapses unrelated uncertainty.

### Contract

Mandatory unknown entity/value kinds:

```text
UNKNOWN_SYMBOL
UNKNOWN_TYPE
UNKNOWN_MODULE
UNKNOWN_MEMBER
UNKNOWN_CALL_TARGET
UNKNOWN_EXTERNAL_IMPLEMENTATION
UNKNOWN_VALUE
UNKNOWN_MEMORY_LOCATION
UNKNOWN_EFFECT
UNKNOWN_RESOURCE
UNKNOWN_FFI_TARGET
UNKNOWN_CONCURRENCY_TARGET
```

Unknown identity includes:

```text
workspace/context
owner or query proof scope
unknown kind
originating fact/relation role
reason code
candidate-set digest where present
program point/source occurrence where relevant
```

Example:

```text
unknown_id = BLAKE3_128(CBEF-v1(
  domain = UNKNOWN_REMAINDER,
  workspace, context, owner, relation kind, role, reason, candidate digest
))
```

One candidate set may contain exact/possible known entities plus one unknown-remainder entity. The unknown edge means additional targets may exist; it is not another ordinary candidate.

Required unknown reason classes include:

```text
DYNAMIC_LANGUAGE_OPEN_WORLD
EXTERNAL_BODY_NOT_INDEXED
PROVIDER_UNAVAILABLE
ANALYSIS_WIDENED
REFLECTION_OR_CODE_GENERATION
FFI_UNRESOLVED
UNSUPPORTED_CONSTRUCT
CONFLICTING_EXACT_EVIDENCE
SOURCE_INVALID
```

`AUTHORIZATION_EXCLUDED` is a query-coverage gap reason, not a canonical ontology unknown. Authorization must not create or persist a semantic unknown fact that reveals the existence of denied source.

Explicit negative facts are allowed only through the negative registry. Initial negative fact families are:

```text
PROVEN_DOES_NOT_ALIAS_UNDER_PROFILE
PROVEN_NO_PATH_WITHIN_PROJECTION_AND_BOUNDARY
PROVEN_NOT_SUBTYPE_IN_CLOSED_TYPE_UNIVERSE
PROVEN_NO_RESOLVED_MEMBER_IN_CLOSED_MEMBER_SET
```

Each negative fact includes the positive fact kind it negates, proof-universe fingerprint, derivation/profile, complete coverage proof, and supporting facts. Negative facts are not materialized exhaustively; they are created when a registered analysis or query proof requests them.

No negative fact is valid across a different projection, context, authorization scope, source generation, or precision profile.
## AC-G-74 — Graph projection registry
### Decision

Every traversal or graph derivation names a versioned projection that completely defines node/edge membership and uncertainty semantics.

### Contract

Projection record:

```yaml
projection_id
version
node_kind_families
edge_kind_codes
context_policy
representation_policy
certainty_filter
resolution_filter
directness_filter
include_unknown_edges
include_external_endpoints
normal_exception_unwind_policy
edge_directionality
parallel_edge_policy
weight_semantics
owner/boundary applicability
materialization_policy
query_phrase_ids
```

Mandatory projections:

| Projection | Core membership and policy |
|---|---|
| `SYNTAX_TREE_V1` | syntax occurrences + ordered AST-child edges; source context only |
| `SYMBOL_BINDING_V1` | scopes/declarations/bindings/references/imports with exact and explicit possible/unknown resolution |
| `TYPE_GRAPH_V1` | semantic types, aliases, subtype/trait/protocol/argument relations; context-local |
| `CALL_EXACT_V1` | callable/instance nodes with exact or statically resolved call edges only |
| `CALL_SOUND_V1` | exact + sound-possible call edges + unknown remainder; default for transitive effects/summaries |
| `CFG_NORMAL_V1` | owner-local normal control-flow edges only |
| `CFG_FULL_V1` | normal + exceptional/unwind/cleanup edges with edge labels |
| `DATAFLOW_V1` | definitions/uses/values/locations and direct reaching/value-dependency edges under selected precision profile |
| `ALIAS_V1` | values/locations/points-to/alias relations with exact/sound/unknown separation |
| `OWNERSHIP_V1` | Rust moves/copies/borrows/reborrows/loans/drops and program-point state |
| `EFFECT_V1` | callables/effects/resources plus direct and registered summary propagation edges |
| `DEPENDENCY_V1` | semantic code-declared module/package/crate dependencies; excludes operational invalidation edges |
| `CONCURRENCY_V1` | task/thread/channel/lock/atomic events and static ordering edges |

Projection IDs and versions participate in path/derived fact IDs and completeness universes. A projection update changing membership or uncertainty policy is compatibility-sensitive and invalidates materialized results.

Unknown edges are never silently removed from a projection used for soundness or negative proof. Query requests may choose an exact-only projection, but its coverage statement makes the narrower semantics explicit.
## AC-G-75 — Interprocedural summary semantics registry
### Decision

Callable summaries are deterministic fact compressions with separate direct and transitive fields, explicit call-edge profile, unknown propagation, and supporting witnesses.

### Contract

A summary profile record declares:

```text
summary_profile_id/version
input projection and precision profile
included direct fact families
call projection for propagation
set/count/value aggregation operators
unknown/external propagation rules
SCC/fixpoint algorithm and widening
support-witness policy
completeness rule
```

`CALLABLE_SUMMARY_BALANCED_V1` contains:

```text
direct_called_targets by resolution class
direct_reads/writes/moves/copies/borrows/drops
direct_effect/resource/raise/panic/unwind facts
direct_allocates/spawns/locks/sends/receives
transitive versions of each supported set/count
recursive SCC ID and recursion flag
unknown call/effect/memory/resource remainder flags
analysis_widened flag
per-family completeness
supporting direct fact IDs
bounded witness call paths for propagated facts
summary content digest
```

Propagation uses `CALL_SOUND_V1`. Exact-only summaries are a separate profile and SHALL not be labeled sound over dynamic dispatch.

Algorithm:

1. compute direct summaries per callable/instance;
2. build call SCCs under the selected projection;
3. process condensation DAG bottom-up;
4. within an SCC, run a sorted deterministic worklist to fixpoint;
5. union/set/count operations use registry-defined idempotent semantics;
6. external/unknown call edges propagate the appropriate unknown effect/memory/resource remainder;
7. apply profile widening only at configured thresholds and mark it explicitly.

Direct and transitive fields never share one property code. A propagated fact records at least one deterministic minimal witness path when requested; absence of a witness makes the summary non-conforming for proof-sensitive use.

Large exact sets remain relation facts. Summary rows may store counts/digests and bounded inline IDs, but the query service SHALL be able to retrieve the supporting canonical facts rather than treating a truncated inline set as complete.
## AC-G-76 — Static concurrency and happens-before semantics
### Decision

The ontology models statically established ordering and possible concurrency. It does not predict a runtime schedule or infer ordering from “async” alone.

### Contract

Core entities:

```text
TASK
THREAD
COROUTINE_INSTANCE
SPAWN_EVENT
JOIN_EVENT
AWAIT_EVENT
SEND_EVENT
RECEIVE_EVENT
LOCK_ACQUIRE_EVENT
LOCK_RELEASE_EVENT
ATOMIC_EVENT
BARRIER_EVENT
CHANNEL
LOCK
ATOMIC_LOCATION
```

Core relations:

```text
PROGRAM_ORDER_BEFORE
SPAWNS
JOINS
AWAITS
SENDS
RECEIVES
ACQUIRES
RELEASES
SYNCHRONIZES_WITH_EXACT
MAY_SYNCHRONIZE_WITH
HAPPENS_BEFORE_EXACT
HAPPENS_BEFORE_SOUND_MAY
MAY_RUN_CONCURRENTLY
LOCK_ORDER_BEFORE
```

Rules:

1. Program order establishes exact order only within one modeled sequential execution domain and along feasible CFG edges.
2. A spawn event happens before child start; child completion happens before a successful modeled join continuation.
3. Releasing an exact mutex/lock happens before a later exact successful acquisition of the same lock only when the language/runtime model establishes that synchronization rule. Otherwise use possible synchronization.
4. Channel send/receive pairing is exact only when static identity and one-to-one pairing are proven; otherwise candidate receive/send edges plus unknown remainder are emitted.
5. Rust atomic orderings are preserved as properties. Release/acquire or sequentially consistent synchronization is exact only when the reads-from relationship is exact; otherwise it is possible. Relaxed operations do not create a synchronization edge by themselves.
6. `await`/yield is a suspension point and potential interleaving boundary, not a happens-before edge to arbitrary tasks.
7. Python's event-loop execution does not imply atomicity across awaits; code without an await may be sequential within one task but external calls/modelled callbacks can introduce unknown barriers.
8. Unknown external schedulers, reflection, FFI, callbacks, or runtime primitives create explicit concurrency unknown remainder.
9. `MAY_RUN_CONCURRENTLY` is derived only when neither exact happens-before direction is proven within the declared closed task/thread scope; incomplete scope makes the result possible/indeterminate, not exact.

Concurrency facts are context- and projection-scoped. Negative race/safety judgments are outside the ontology.
## AC-G-77 — Effect and resource model semantics
### Decision

Effects and resources use closed registries with explicit direct events, resource identities/states, model evidence, and transitive propagation. “Pure” or “no effect” is asserted only under complete coverage.

### Contract

Initial effect registry:

```text
READ_MEMORY
WRITE_MEMORY
ALLOCATE_MEMORY
DEALLOCATE_MEMORY
READ_FILE
WRITE_FILE
READ_NETWORK
WRITE_NETWORK
READ_DATABASE
WRITE_DATABASE
BEGIN_TRANSACTION
COMMIT_TRANSACTION
ROLLBACK_TRANSACTION
READ_STANDARD_INPUT
WRITE_STANDARD_OUTPUT
LOG_OR_TELEMETRY
READ_ENVIRONMENT
WRITE_ENVIRONMENT
SPAWN_PROCESS
SPAWN_THREAD_OR_TASK
BLOCK_THREAD
SLEEP_OR_WAIT
LOAD_DYNAMIC_LIBRARY
ACQUIRE_LOCK
RELEASE_LOCK
SEND_CHANNEL
RECEIVE_CHANNEL
READ_TIME
READ_RANDOMNESS
READ_GLOBAL_STATE
WRITE_GLOBAL_STATE
RAISE_EXCEPTION
PANIC_OR_ABORT
UNSAFE_OPERATION
FFI_CALL
DYNAMIC_CODE_EXECUTION
UNKNOWN_EXTERNAL_EFFECT
```

Resource kinds:

```text
FILE_HANDLE
SOCKET_OR_CONNECTION
DATABASE_CONNECTION_OR_TRANSACTION
LOCK_GUARD
CHANNEL_ENDPOINT
PROCESS_HANDLE
THREAD_OR_TASK_HANDLE
MEMORY_ALLOCATION
USER_DEFINED_MODELLED_RESOURCE
UNKNOWN_RESOURCE
```

Resource identity is either an exact semantic/memory entity or a modeled resource key:

```text
(resource kind, allocation/acquisition site, owner/context,
 normalized external key formula, instance/points-to abstraction)
```

Resource states/events:

```text
UNACQUIRED → ACQUIRED/OPEN → USED/TRANSFERRED/ESCAPED → RELEASED/CLOSED
```

Canonical relations include `ACQUIRES_RESOURCE`, `USES_RESOURCE`, `TRANSFERS_RESOURCE`, `ESCAPES_RESOURCE`, `RELEASES_RESOURCE`, and program-point state facts. An event may have multiple possible resources plus unknown remainder.

Evidence sources:

- Rust MIR/drop/RAII and compiler facts;
- Python syntax/CFG for context managers, generators, async managers, and explicit close calls;
- exact/modelled standard-library or dependency model-pack semantics;
- conservative custom dataflow/points-to propagation.

Direct effects are owned by the source callable/instance. Transitive effects use the registered summary profile. An unknown external call propagates `UNKNOWN_EXTERNAL_EFFECT` and any model-declared minimum effects.

Purity is represented only as a proven negative/summary property when all relevant direct/transitive effect capabilities are complete and no unknown remainder exists. The ontology does not infer resource leaks, correctness, or severity from state sequences.


## Cross-layer integration obligations

The following architecture-completion contracts are owned by another 1.3 artifact but are binding inputs to this specification. This document SHALL consume the named contract and SHALL NOT restate it with different semantics.

| Gap | Contract | Permanent owner | Integration obligation in this document |
|---|---|---|---|
| `G-09` | Generalized source-instance identity | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) | Preserve the concept, evidence, identity, or completeness meaning in ontology kinds and registries. |
| `G-14` | Analysis-context discovery, identity, and selection | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) | Preserve the concept, evidence, identity, or completeness meaning in ontology kinds and registries. |
| `G-36` | Provider capability granularity and aggregation | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) | Preserve the concept, evidence, identity, or completeness meaning in ontology kinds and registries. |
| `G-38` | Declarative model-pack format, matching, and trust | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) | Preserve the concept, evidence, identity, or completeness meaning in ontology kinds and registries. |
| `G-39` | Derived-analysis precision profiles | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) | Preserve the concept, evidence, identity, or completeness meaning in ontology kinds and registries. |
| `G-40` | Generated, expanded, stub, shim, and lowered source capture | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) | Preserve the concept, evidence, identity, or completeness meaning in ontology kinds and registries. |
| `G-48` | Completeness and negative-proof algebra | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Preserve the concept, evidence, identity, or completeness meaning in ontology kinds and registries. |

## Release conformance obligations

This specification inherits `G-78` through `G-84` from the suite governance and release manifest. Release acceptance SHALL include the portions of the golden corpus, clean-rebuild comparator, conformance harness, deterministic fault matrix, performance profiles, upgrade choreography, and adversarial security corpus that exercise ontology registries, profiles, property schemas, unknowns, projections, summaries, static concurrency, effects, and source-disclosure classifications.

A passing prose review is insufficient. The corresponding generated registries, schemas, protocol descriptors, fixtures, canonical outputs, and fault oracles SHALL pass the master release gates before an implementation may claim CodeFabric 1.3 conformance.

# Appendix A — Compact Ontology Checklist

```text
SOURCE
[ ] files
[ ] spans
[ ] tokens
[ ] comments
[ ] documentation
[ ] directives
[ ] parse errors

SYNTAX
[ ] every raw syntax node
[ ] normalized syntax kind
[ ] AST field names
[ ] lexical ordering

SEMANTICS
[ ] declarations
[ ] scopes
[ ] bindings
[ ] references
[ ] identity
[ ] imports/exports

TYPES
[ ] declared type
[ ] inferred/computed type
[ ] expected type
[ ] subtype relationships
[ ] generics
[ ] coercions
[ ] narrowing

OBJECT MODEL
[ ] members
[ ] inheritance
[ ] trait/protocol implementation
[ ] overrides
[ ] member resolution

CALLS
[ ] call sites
[ ] receiver
[ ] arguments
[ ] argument binding
[ ] dispatch kind
[ ] exact targets
[ ] may targets
[ ] unknown targets

CONTROL FLOW
[ ] entry/exit
[ ] basic blocks
[ ] normal CFG
[ ] exceptional CFG
[ ] dominators
[ ] post-dominators
[ ] control dependence
[ ] loops

VALUES / DATAFLOW
[ ] values
[ ] definitions
[ ] uses
[ ] reaching definitions
[ ] def-use
[ ] value flow
[ ] liveness

MEMORY
[ ] access paths
[ ] reads
[ ] writes
[ ] initialization
[ ] aliasing
[ ] points-to

OWNERSHIP / LIFETIME
[ ] moves
[ ] copies
[ ] borrows
[ ] reborrows
[ ] loans
[ ] regions
[ ] drops
[ ] resource lifecycle

EFFECTS
[ ] state mutation
[ ] allocation
[ ] I/O
[ ] blocking
[ ] exceptions
[ ] panic/unwind
[ ] spawn/await
[ ] locks
[ ] unsafe
[ ] FFI

GENERATED / LOWERED
[ ] Python synthesized semantics where objectively modelled
[ ] Rust macros
[ ] Rust MIR
[ ] Rust monomorphization
[ ] Rust shims/drop glue
[ ] async/coroutine lowering

DERIVED FACTS
[ ] reachability
[ ] SCCs
[ ] recursion
[ ] dominance
[ ] post-dominance
[ ] control dependence
[ ] loops
[ ] alias sets
[ ] effect summaries
[ ] structural metrics

UNCERTAINTY
[ ] unknown symbols
[ ] unknown types
[ ] unknown call targets
[ ] unknown members
[ ] unknown memory
[ ] unknown effects
```

---

# Appendix B — Explicitly Excluded Analytical Outputs

```text
historical change analysis
semantic diff across revisions
test-impact analysis
coverage analysis
runtime profiling
refactor-safety judgment
bug-likelihood judgment
risk scoring
architecture-quality scoring
vulnerability exploitability
recommendations
remediation plans
change prioritization
```

These may be produced by downstream systems using the facts specified here, but are not part of this ontology.
