# Present-State CPG FastMCP Serving Specification

**Artifact ID:** `codefabric-present-state-cpg-fastmcp-serving`
**Artifact kind:** Normative document
**Compatible suite major:** 1
**Release date:** 2026-08-20
**Canonical digest:** External; recorded in `codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_artifact_index.json`

**Status:** Released normative implementation specification
**Synchronized suite version:** 1.3
**Specification version:** 1.3
**Primary MCP framework:** FastMCP Python `3.4.7`
**Python contract framework:** Pydantic `2.13.4`
**Settings framework:** `pydantic-settings 2.15.0`
**MCP transport:** One local STDIO server process per programming agent
**Backend:** One central native-Rust daemon per authorized repository/worktree group; one coordinator per workspace
**Semantic query contract:** `code_property_graph_semantic_query_specification_v1.3.md`, version `1.3`
**Data plane:** Native Rust, Apache Arrow, Apache DataFusion, Delta Lake / delta-rs
**Scope:** Read-only present-state CPG fact retrieval for LLM programming agents
**Excluded deployment profiles:** HTTP, ASGI, multi-user gateways, and shared Python adapter processes

---

## 0. Synchronized CodeFabric 1.3 governing contract

This document is a released member of the synchronized **CodeFabric present-state CPG specification suite, version 1.3**. The suite integrates the architecture-completion contracts `G-01` through `G-84`; the earlier standalone completion specification is retained only as a historical design record and is no longer required to interpret this release.

The cross-cutting source of authority is `codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md`. This document permanently owns the domain contracts assigned to it by that manifest. A less-specific statement elsewhere in this document SHALL be read through the 1.3 contract sections and SHALL NOT override them.

### 0.1 Artifact identity and version

```yaml
artifact_id: "codefabric-present-state-cpg-fastmcp-serving"
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

This document specifies a production-grade FastMCP coordination layer for serving the composable semantic CPG fact-query contract to LLM programming agents through one dedicated STDIO process per agent.

The deployment model is deliberately asymmetric:

```text
one central native-Rust CPG daemon
    owns source-state coordination and continuous updating
    owns CPG fact generation, reconciliation, and derived facts
    owns ServingSnapshot-pinned current-state catalogs
    owns semantic phrase resolution
    owns canonical request-schema and semantic validation
    owns query dependency-DAG compilation
    owns DataFusion planning and execution
    owns graph traversal and deterministic summaries
    owns snapshot consistency, resource governance, and result materialization

one FastMCP STDIO process per programming agent
    owns MCP protocol framing and component publication
    owns one immutable Pydantic Settings snapshot
    owns strict, versioned model-facing Pydantic contracts
    owns a long-lived authenticated daemon client
    forwards semantic request JSON without translating it into SQL or graph syntax
    maps daemon progress, errors, deadlines, and cancellation into MCP behavior
    filters daemon control data into explicit public output allowlists
    adapts completed responses for inline or immutable-resource delivery
    exposes instructions, schemas, recipes, prompts, and status
```

The central rule is:

> **The Python process is a thin, typed, observable contract and coordination adapter. It is not a second semantic engine, CPG service, query planner, graph engine, or cache of semantic truth.**

Pydantic has a precise but deliberately bounded role:

```text
Pydantic validates and serializes:
    process settings
    compact MCP input envelopes
    adapter-owned daemon DTOs
    public MCP outputs
    public ToolResult metadata
    safe validation diagnostics
    adapter JSON Schemas

Pydantic does not validate or interpret:
    the complete semantic CPG query language
    semantic phrases
    result-reference meaning
    graph topology
    canonical fact records one by one
    the complete canonical CPG response object graph
```

The result is a local MCP surface that is:

- easy for programming agents to discover and use;
- semantically precise rather than storage-aware;
- composable in one request;
- read-only and process-isolated;
- consistent across multiple simultaneous agents;
- inexpensive to launch per agent;
- resilient to daemon and publication changes;
- safe against accidental metadata and subclass-field leakage;
- reproducible through exact dependency pins and generated contract schemas;
- compatible with a continuously updated native-Rust CPG implementation.

---

## 2. Source basis and dependency graph

This specification is grounded in the synchronized 1.3 suite:

| Artifact | Governing responsibility |
|---|---|
| `code_property_graph_present_state_fact_ontology_specification_v1.3.md` | Canonical fact forms, certainty/resolution/directness, IDs, unknowns, property facts |
| `present_state_cpg_fact_generation_specification_python_rust_v1.3.md` | Provider observation contracts, contexts, byte-safe source DTOs, async job protocols |
| `present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md` | Reconciliation/derivation authority, schemas, durable publication, ServingSnapshot catalog |
| `codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md` | Workspace topology, source truth, hot overlay, freshness barrier, recovery, operational state |
| `code_property_graph_semantic_query_specification_v1.3.md` | Canonical semantic JSON, structured freshness, PublicSnapshotMetadata, statuses/errors |
| `fastmcp_python_advanced_reference_3.4.7.md` | FastMCP 3.4.7 STDIO behavior |
| `pydantic_python_advanced_reference_2.13.4.md` | Pydantic adapter contracts and serialization |

Dependency precedence is the suite-wide §0 contract. The adapter packages exact request/response schemas for semantic query 1.3 and SHALL fail on fingerprint mismatch.

Application architecture remains:

- gRPC/Protobuf over a local UDS as the primary Python-to-Rust boundary;
- canonical semantic request/response as daemon-owned JSON;
- a four-tool MCP catalog;
- Pydantic as a small public-contract firewall;
- large responses as immutable agent/workspace/snapshot-scoped resources;
- no FastMCP task mode, Tool Search, Code Mode, or semantic response cache on the canonical path.

## 3. Normative language

The key words **SHALL**, **SHALL NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.

- **SHALL / SHALL NOT**: conformance requirement.
- **SHOULD / SHOULD NOT**: strong recommendation; departure requires a documented reason.
- **MAY**: optional behavior.

---

# Part I — Governing Architecture

## 4. End-to-end topology

```text
Programming Agent A
    │ MCP over STDIO
    ▼
FastMCP adapter process A
    │ gRPC over UDS; agent capability + immutable workspace_id
    ▼
Central native-Rust CodeFabric daemon
    ├─ WorkspaceRegistry
    ├─ optional CommonRepoActor(s)
    ├─ WorkspaceCoordinator A  -> one source instance / active ServingSnapshot
    ├─ WorkspaceCoordinator B  -> another linked worktree or non-Git root
    ├─ ReconciliationEngine and DerivationRegistry
    ├─ overlay-aware DataFusion query service
    └─ result artifact/snapshot lease service
    ▼
Arrow + DataFusion + Delta + SQLite operational state
```

The topology repeats for agents B/C/D. Each adapter has one agent identity and one authorized immutable `workspace_id`; the central daemon may serve many workspaces.

### 4.1 Why one STDIO process per agent

A separate process provides MCP-stream isolation, process failure isolation, natural agent quotas/result ownership, immutable local settings, and cancellation on subprocess close.

### 4.2 Why a central multi-workspace daemon

The daemon shares expensive native infrastructure while preserving one coordinator and one active snapshot pointer per workspace. It prevents adapters from loading catalogs, generating facts, choosing worktree defaults, or interpreting freshness independently.

### 4.3 Four contract planes

```text
1. MCP declaration/public Pydantic contract
2. daemon RPC control contract
3. canonical semantic query JSON contract
4. native ServingSnapshot/data-fabric contract
```

The adapter owns plane 1, jointly versions plane 2, packages but does not reinterpret plane 3, and never implements plane 4.

## 5. Responsibility matrix

| Concern | FastMCP adapter | Rust daemon |
|---|---:|---:|
| MCP STDIO framing and component publication | **Owns** | Does not own |
| Model-facing descriptions and instructions | **Owns** | Supplies capability data |
| Immutable process settings | **Pydantic-owned adapter contract** | Does not own |
| Compact tool-envelope validation | **Pydantic/FastMCP** | Revalidates hard limits |
| Public output filtering and serialization | **Pydantic-owned allowlists** | Supplies terminal data |
| Public adapter JSON Schemas | **Generated from Pydantic** | Does not own |
| Full semantic request JSON Schema | Publishes exact packaged copy | **Authoritative validation** |
| Full canonical response JSON Schema | Publishes exact packaged copy | **Authoritative validation** |
| Semantic phrase/entity/reference resolution | Never implements | **Owns** |
| Query-block dependency graph | Cheap structural precheck only | **Authoritative compiler** |
| SQL / DataFusion `Expr` / `LogicalPlan` generation | Prohibited | **Owns** |
| Snapshot pinning and table-version selection | Never performs | **Owns** |
| Graph traversal and derived calculations | Never performs | **Owns** |
| Canonical response construction | Never reconstructs facts | **Owns** |
| Canonical response JSON decoding | Delivery-only | Produces validated bytes |
| Large result storage | Exposes MCP resource | **Owns bytes and lease** |
| Agent-specific request identity | **Owns and forwards** | Enforces |
| Progress notifications | Maps to MCP `Context` | Emits semantic phases |
| Cancellation | Detects MCP cancellation | Cancels execution and cleanup |
| Operational logging and tracing | Adapter spans | Daemon spans and plans |
| Fact completeness and unknown semantics | Never invents | **Owns** |
| Source text | Never caches broadly | Controlled daemon access |

### 5.1 Prohibited adapter behavior

The adapter SHALL NOT:

- construct SQL from semantic query blocks;
- query Delta tables directly;
- open an Arrow or DataFusion catalog;
- traverse graph edges locally;
- recreate the complete semantic request schema as a Pydantic model graph;
- instantiate one Pydantic model per entity, fact, path, or source-context record;
- reinterpret an unresolved fact as absence;
- merge exact and possible targets;
- repair or silently rewrite an invalid semantic request;
- answer from a previous result when the daemon is unavailable;
- infer source facts from local filesystem reads;
- expose arbitrary daemon methods as MCP tools;
- forward unrestricted daemon dictionaries into model-visible output or metadata;
- use broad duck-typed Pydantic serialization on public contracts;
- mutate source, graph, publication, or daemon configuration.

---

## 6. Hard system invariants

1. **One snapshot per request.** The daemon applies freshness and leases one immutable ServingSnapshot before semantic resolution.
2. **One canonical identity space.** Every canonical ID in one response belongs to the pinned snapshot.
3. **No data-plane duplication in Python.** Python never becomes an Arrow/DataFusion processing layer.
4. **Read-only MCP surface.** Every exposed tool has read-only semantics.
5. **No arbitrary SQL or physical graph syntax.** The agent sees only the semantic contract.
6. **No silent truncation.** Explicit limits, hard rejections, and unavailable facts remain distinct.
7. **Unknown remains data.** Unresolved entities and facts are returned explicitly.
8. **Exact and possible remain distinct.** The adapter never collapses resolution classes.
9. **Direct and transitive remain distinct.** The adapter never flattens them.
10. **Cancellation is end-to-end.** MCP cancellation reaches native execution and cleanup.
11. **STDOUT is protocol-only.** Every ordinary Python log goes to STDERR or telemetry.
12. **Contracts are independently versioned.** FastMCP, Pydantic wire schemas, RPC, semantic query, ontology, schema bundle, and publication are separate.
13. **Public output is schema-closed.** Every model-visible dictionary is produced from an explicit Pydantic public model or canonical daemon response.
14. **No raw metadata forwarding.** `ToolResult.meta` is built only from `PublicToolMeta`.
15. **Public unknown fields fail.** Public Pydantic models use `extra="forbid"`.
16. **Additive RPC fields do not leak.** Daemon DTOs may ignore negotiated additive fields, but public mapping is explicit.
17. **Validation and serialization schemas are separate artifacts.** The model-visible output schema uses Pydantic serialization mode.
18. **No per-request schema compilation.** Pydantic models and `TypeAdapter`s are built once and reused.
19. **No I/O in Pydantic validators or serializers.** Daemon access belongs in lifespan/services/tool handlers.
20. **Settings are immutable for process lifetime.** Configuration changes require a new adapter process.

---

## 7. Non-goals

This adapter is not:

- a code-writing or source-editing server;
- an autonomous refactoring service;
- a vulnerability or risk scoring service;
- a Git-history service;
- a generic SQL, graph, or filesystem interface;
- a public network service;
- a shared multi-user Python gateway;
- a task queue;
- a second semantic validator;
- a Pydantic representation of every CPG fact.

All deployment guidance in this specification assumes local STDIO with one adapter process per programming agent.

---
# Part II — Rust Daemon Boundary

## 8. Recommended transport: gRPC over Unix domain socket

The primary daemon boundary SHOULD be **gRPC with Protobuf over a Unix domain socket** on Linux and macOS.

Value:

- generated, typed Rust and Python clients;
- server-streaming progress and result events;
- built-in deadlines and cancellation;
- HTTP/2 flow control;
- explicit status codes;
- mature observability integration;
- binary framing without inventing a transport;
- independent version negotiation;
- efficient local IPC without opening a network port.

Recommended endpoint form:

```text
unix:///run/user/<uid>/codefabric/cpgd.sock
```

Development fallback:

```text
unix:///tmp/codefabric-<uid>/cpgd.sock
```

A loopback TCP endpoint MAY be supported when UDS is unavailable, but it SHALL require a random capability token and SHALL bind only to loopback by default.

### 8.1 Why not make the daemon itself an MCP server

The daemon's native interface is broader and lower-level than the model-facing contract. Making it the MCP server would couple:

- Rust daemon release cadence to MCP framework behavior;
- physical query operations to model-visible tools;
- internal service methods to an agent-facing security surface;
- central daemon lifecycle to each host's STDIO expectations.

The Python adapter intentionally keeps MCP-specific concerns outside the data-plane daemon.

### 8.2 Why not HTTP between every adapter and the daemon

Loopback HTTP is viable, but UDS provides a simpler local trust boundary and avoids port management. The daemon RPC is not intended to be public or multi-tenant over a network. Remote access should be a separate, explicitly authenticated deployment profile.

---

## 9. Protobuf service and accepted-handle protocol

The Protobuf contract strongly types transport control while carrying canonical semantic JSON bytes.

```proto
syntax = "proto3";
package codefabric.cpgd.v1;

service CpgQueryService {
  rpc Handshake(HandshakeRequest) returns (HandshakeResponse);
  rpc GetStatus(StatusRequest) returns (StatusResponse);
  rpc ValidateQuery(ValidateQueryRequest) returns (ValidateQueryResponse);
  rpc StartQuery(StartQueryRequest) returns (StartQueryResponse);
  rpc StreamQuery(StreamQueryRequest) returns (stream QueryEvent);
  rpc AttachQuery(AttachQueryRequest) returns (stream QueryEvent);
  rpc CancelQuery(CancelQueryRequest) returns (CancelQueryResponse);
  rpc ReadResult(ReadResultRequest) returns (stream ResultChunk);
  rpc ReleaseResult(ReleaseResultRequest) returns (ReleaseResultResponse);
}

enum FreshnessPolicy {
  FRESHNESS_POLICY_UNSPECIFIED = 0;
  BEST_AVAILABLE_SNAPSHOT = 1;
  AWAIT_LATEST = 2;
  REQUIRE_CURRENT_FOR_TARGETS = 3;
  REQUIRE_SOURCE_CURRENT = 4;
  REQUIRE_SEMANTIC_CURRENT = 5;
}

message StartQueryRequest {
  string agent_instance_id = 1;
  string workspace_id = 2;
  string mcp_call_id = 3;
  string rpc_attempt_id = 4;
  optional string semantic_request_id = 5;
  string semantic_query_spec_version = 6;
  bytes request_json = 7;
  string request_checksum = 8;
  FreshnessPolicy freshness_policy = 9;
  DeliveryPreference delivery_preference = 10;
  string host_capability_profile_digest = 11;
  int64 deadline_unix_ms = 12;
  string idempotency_key = 13;
  PayloadCompression payload_compression = 14;
}

message StartQueryResponse {
  string daemon_query_id = 1;
  bytes resume_token = 2;
  int64 accepted_at_unix_ms = 3;
  QueryExecutionState query_execution_state = 4;
  string queue_class = 5;
  optional uint32 queue_position = 6;
  string negotiated_request_version = 7;
  string negotiated_response_version = 8;
  string effective_semantic_request_id = 9;
}

message QueryEvent {
  oneof event {
    SnapshotPinnedEvent snapshot_pinned = 1;
    ProgressEvent progress = 2;
    ResponseChunkEvent response_chunk = 3;
    ArtifactReadyEvent artifact_ready = 4;
    TerminalEvent terminal = 5;
  }
}
```

`StartQuery` SHALL return the durable `daemon_query_id` and opaque resume token before a potentially long freshness wait. This gives the adapter a cancellation and reconnection handle immediately. `StreamQuery` consumes that handle for the initial event stream; `AttachQuery` resumes after a caller-provided `uint64` sequence. The first semantic event is `SnapshotPinnedEvent`, emitted after the freshness barrier with the canonical `PublicSnapshotMetadata` record. Every event carries one monotonically increasing `uint64` sequence and exactly one of the five variants above.

`ArtifactReadyEvent` SHALL issue the opaque result `lease_token` alongside the artifact identity,
checksum, content type, encoding, and expiry. `ReadResult` and `ReleaseResult` accept exactly that
token. A resume token, artifact ID, checksum, capability token, or derivable value SHALL NOT be
substituted for the result lease token.

The RPC freshness enum SHALL match the canonical request's top-level freshness policy; a mismatch is `INVALID_REQUEST_SCHEMA`. The Protobuf contract does not duplicate all semantic query-form models.

## 10. Handshake, workspace binding, and compatibility negotiation

Every adapter performs one lifespan handshake before advertising transport readiness.

Request fields include adapter/FastMCP/Pydantic/Python versions, RPC version, supported semantic-query versions, packaged schema fingerprints, agent ID, immutable configured workspace ID, delivery/frame capabilities, trace capability, and capability token.

Response fields include:

```text
daemon/Rust build and RPC versions
authorized workspace_id
repository_id/worktree_id optional
workspace kind and lifecycle state
active semantic-query/request/response schema versions/fingerprints
ontology/schema/provider/derivation bundle versions
active PublicSnapshotMetadata when available
workspace bootstrapping/degraded reason when no active snapshot exists
supported languages/forms/capabilities
result resource capabilities and hard limits
```

### 10.1 Fail-fast rules

Startup fails for protocol-major mismatch, unsupported semantic-query major, same-version schema fingerprint mismatch, rejected capability, unauthorized/mismatched workspace ID, or invalid public contract bundle.

The adapter MAY be transport-ready while the authorized workspace is `WORKSPACE_BOOTSTRAPPING`; status/reference tools remain available and fact queries return the canonical bootstrapping error. Availability of a durable publication alone is not a readiness proxy.

## 11. Daemon operations

### 11.1 `Handshake`

Checks compatibility and establishes the adapter's authorized workspace and agent identity.

### 11.2 `GetStatus`

Returns operational status and present-state capability metadata. It does not run a CPG fact query.

### 11.3 `ValidateQuery`

Performs:

- JSON Schema validation;
- fact-only boundary validation;
- semantic phrase resolution;
- query-ID and result-reference validation;
- dependency-DAG construction;
- cycle detection;
- input/result role type checking;
- capability availability checks;
- negative-claim coverage checks;
- unbounded path and amplification checks;
- normalized request generation;
- deterministic cost-class estimation.

It SHALL NOT execute the fact retrieval plan.

### 11.4 `StartQuery`

Validates transport metadata and returns the accepted query handle, resume token, negotiated versions, and effective semantic request ID without waiting for freshness, planning, or execution.

### 11.5 `StreamQuery`

Starts delivery for an accepted query and emits snapshot, progress, response-chunk or artifact, and terminal events in sequence order.

### 11.6 `AttachQuery`

Reattaches to an accepted query from a caller-provided sequence/checksum cursor. Replay follows the negotiated orphan-retention and idempotency rules.

### 11.7 `CancelQuery`

Best-effort explicit cancellation for cases where transport cancellation is not sufficient or cleanup must be confirmed.

### 11.8 `ReadResult`

Streams bytes for an immutable, agent-scoped result artifact or one of its logical subresources.

### 11.9 `ReleaseResult`

Allows early deletion after the agent no longer needs the result. TTL cleanup remains authoritative.

---

## 12. Query lifecycle

```text
1. authenticate agent capability and exact workspace_id
2. validate adapter byte/JSON structural limits
3. establish semantic_request_id, mcp_call_id, rpc_attempt_id
4. normalize idempotency key inputs
5. return StartQueryResponse with daemon_query_id and resume_token
6. validate canonical request schema and RPC/request freshness agreement
7. resolve target scope needed by the freshness policy
8. apply lifecycle freshness barrier
9. atomically clone Arc<ServingSnapshot> and acquire lease
10. emit SnapshotPinnedEvent/PublicSnapshotMetadata on StreamQuery or AttachQuery
11. resolve semantic phrases and type-check query dependency DAG
12. reject evaluative, negative-without-coverage, cross-workspace, or unbounded requests
13. compile internal PlanSpec and qualified DataFusion plans against the leased catalog
14. execute independent DAG branches under resource governance
15. materialize canonical response and orthogonal statuses/coverage
16. validate deterministic order, response schema, and checksum
17. deliver inline or persist immutable snapshot-scoped artifact
18. emit exactly one TerminalEvent and release execution resources
19. retain only artifact/snapshot leases required for configured TTL
```

The adapter coordinates authentication, accepted-handle capture, progress, delivery adaptation, and cancellation; it never pins or interprets the snapshot itself.

## 13. Progress model

The daemon SHOULD emit a small semantic phase vocabulary:

```text
accepted
validating request
resolving semantic phrases
pinning current snapshot
planning query dependency graph
planning fact retrieval
executing query blocks
materializing canonical response
validating result
externalizing large result
complete
```

Progress events SHOULD include:

```text
phase
completed_units
total_units
current query_id when relevant
human-readable message
elapsed_ms
```

They SHALL NOT expose physical table names, SQL, internal edge labels, or source content.

The adapter maps these events to `Context.report_progress(...)`. Debug-only physical plans remain daemon operational artifacts.

---

## 14. Cancellation and deadlines

### 14.1 Accepted-handle cancellation

`StartQuery` returns the accepted handle first. The adapter records `daemon_query_id` and the opaque resume token before opening `StreamQuery` or awaiting freshness, planning, or terminal delivery.

If MCP cancellation or STDIO closure occurs:

1. cancel the active gRPC stream;
2. send best-effort `CancelQuery(daemon_query_id)`;
3. daemon cancels freshness wait, DataFusion/custom operators, and artifact creation;
4. daemon releases memory/spill/snapshot leases and deletes incomplete artifacts;
5. cancellation acknowledgement is recorded operationally.

Transport cancellation remains effective even if explicit cancellation acknowledgement cannot be delivered.

### 14.2 Deadline hierarchy

```text
MCP host deadline
  ≥ FastMCP tool timeout
    ≥ adapter gRPC deadline
      ≥ daemon freshness + execution deadline
```

A cleanup margin is reserved. Recommended status/reference/validate/query/resource-read defaults remain configurable; daemon hard budgets are authoritative.

## 15. Idempotency and reconnect behavior

Before validation and canonical hashing, the adapter establishes `effective_semantic_request_id`: preserve the supplied canonical JSON value, or generate a new opaque value and inject it into the normalized request. The RPC field and normalized JSON field SHALL match. The effective value is retained for every retry/resume of that MCP operation and echoed in every terminal output.

Before snapshot pinning, the normalized idempotency key is:

```text
workspace_id
+ agent_instance_id
+ effective_semantic_request_id
+ canonical normalized request JSON hash
+ normalized structured freshness policy
```

A generated semantic request ID does not create cross-invocation idempotency. A caller requesting that behavior SHALL provide a stable semantic request ID. After pinning, the accepted record additionally stores `snapshot_id` and SHALL never migrate to a newer snapshot.

Rules:

- same semantic request ID/hash/freshness may resume or return the existing terminal result;
- same semantic request ID with different content/freshness is rejected;
- `mcp_call_id` and `rpc_attempt_id` are correlation values, not semantic idempotency identity;
- reconnect may resume terminal delivery by accepted daemon query ID or semantic request ID under the same agent/workspace capability;
- externalized result ID remains stable on idempotent retry;
- broad middleware retries around arbitrary tool calls remain prohibited.

## 16. Daemon-side query sessions

A session contains:

```text
agent identity and immutable workspace_id
semantic_request_id / mcp_call_id / rpc_attempt IDs
normalized request and freshness hashes
daemon_query_id
freshness barrier state
pinned ServingSnapshot ID and lease
base publication/table-version map + overlay checksum
analysis-context set
query dependency DAG and plans
resource/cancellation/spill scope
result materialization and trace state
```

No Python object is authoritative session state. Repository/worktree IDs are diagnostic attributes of the bound workspace, not alternate routing keys.

## 17. Result artifacts and snapshot leases

A result artifact SHALL be immutable, unguessable, agent/workspace scoped, and tied to:

```text
snapshot_id
workspace_id
repository_id/worktree_id optional
base publication and table-version digest
overlay generation and checksum
source generation/inventory digest
analysis-context set
ontology/schema/provider/derivation/query versions
canonical response checksum
semantic request/idempotency record
```

It stores canonical UTF-8 JSON bytes plus optional logical subresources, has bounded TTL, and is deleted on release/expiry. It never keeps a live DataFusion session, but its metadata is sufficient to audit the exact input snapshot. Snapshot/version retirement waits for active query/artifact leases.

# Part III — FastMCP and Pydantic Server Contract

## 18. Framework and package posture

The production adapter SHALL exact-pin:

```text
fastmcp            == 3.4.7
pydantic           == 2.13.4
pydantic-settings  == 2.15.0
```

FastMCP 4 prerelease APIs and Pydantic prerelease APIs SHALL NOT be mixed into this implementation.

Recommended application dependencies:

```toml
[project]
name = "codefabric-cpg-mcp"
requires-python = ">=3.12"
dependencies = [
  "fastmcp==3.4.7",
  "pydantic==2.13.4",
  "pydantic-settings==2.15.0",
  "grpcio==1.83.0",
  "protobuf==7.36.0",
]
```

`grpcio-tools==1.83.0` is an exact build/development dependency, not a production
runtime import. It owns the suite's single Protobuf compiler invocation. `orjson` is not
an adapter dependency: sorted ordinary JSON is not RFC 8785 canonical JSON, ProtoJSON
is owned by Protobuf, and MCP structured values are owned by Pydantic/FastMCP.
Re-adoption requires a named non-canonical boundary, fixed options and limits, semantic
fixtures, and a benchmark.

The project SHALL NOT pin `pydantic-core` independently; Pydantic selects its matching core release. The adapter does not require Arrow, DataFusion, Delta Lake, Ruff, Pyrefly, Tree-sitter, rustc bindings, or an HTTP application framework.

Exact Pydantic pins are justified because generated JSON Schemas, union behavior, error details, and serialization shape are external contract artifacts.

---

## 19. Pydantic adapter-contract architecture

### 19.1 Contract families

The Python package SHALL define separate model families:

```text
settings contracts
    process-lifetime configuration and secrets

public MCP wire contracts
    exact model-visible structured_content and ToolResult.meta

adapter application DTOs
    terminal daemon values and handshake/status summaries

canonical semantic payloads
    opaque JSON objects/bytes owned and validated by the Rust daemon
```

These families SHALL reside in separate modules because their trust and compatibility obligations differ.

### 19.2 Strict public base model

All public wire contracts SHOULD inherit from:

```python
from pydantic import BaseModel, ConfigDict


class StrictWireModel(BaseModel):
    model_config = ConfigDict(
        extra="forbid",
        strict=True,
        frozen=True,
        validate_default=True,
        hide_input_in_errors=True,
        allow_inf_nan=False,
        validate_by_alias=True,
        validate_by_name=True,
        serialize_by_alias=True,
    )
```

Rationale:

| Setting | Contract value |
|---|---|
| `extra="forbid"` | catches accidental public-field drift |
| `strict=True` | prevents adapter-side coercion of already structured values |
| `frozen=True` | prevents mutation after validation |
| `validate_default=True` | treats defaults as contract data |
| `hide_input_in_errors=True` | reduces accidental input disclosure |
| `allow_inf_nan=False` | rejects values that are not valid interoperable JSON numbers |
| explicit alias policy | avoids changing defaults across major versions |

Settings models SHALL not use global strict mode because environment values arrive as strings and require typed conversion.

### 19.3 Public vs daemon extra-field policy

```text
public MCP models       extra="forbid"
daemon application DTO extra="ignore" only after RPC-minor compatibility negotiation
canonical response      daemon JSON Schema authority
```

An ignored additive daemon field SHALL never be copied to public output automatically.

### 19.4 Structural JSON adapter

The primary query input remains compact at the FastMCP declaration boundary, but the handler SHALL validate that it is a recursive JSON object:

```python
from pydantic import ConfigDict, JsonValue, TypeAdapter

JSON_OBJECT_ADAPTER = TypeAdapter(
    dict[str, JsonValue],
    config=ConfigDict(
        strict=True,
        allow_inf_nan=False,
        hide_input_in_errors=True,
    ),
)
```

This adapter SHALL be instantiated once at module import. It validates only JSON shape:

- string object keys;
- recursive JSON-compatible values;
- finite numbers;
- no bytes, datetime objects, tuples, arbitrary Python instances, or custom mappings.

It SHALL NOT interpret semantic query meaning.

### 19.5 Why the full semantic schema is not a Pydantic model graph

The semantic query specification contains open controlled-language values, eight deeply composable request forms, prior-result references, graph patterns, and daemon-owned compatibility rules. Reimplementing it in Python would:

- create a second source of truth;
- increase context size of the primary tool;
- duplicate cross-language schema evolution;
- impose model-build and traversal cost;
- tempt the adapter to perform semantic validation.

The complete Draft 2020-12 request and response schemas remain packaged resources and daemon-authoritative contracts.

---

## 20. Server construction

The server SHALL publish serialization-mode output schemas generated from the public Pydantic models. The contract module SHALL attach an explicit Draft 2020-12 dialect declaration and export stable constants:

```python
QUERY_TOOL_OUTPUT_SCHEMA = QUERY_TOOL_OUTPUT_SERIALIZATION_SCHEMA
VALIDATE_TOOL_OUTPUT_SCHEMA = VALIDATE_TOOL_OUTPUT_SERIALIZATION_SCHEMA
STATUS_TOOL_OUTPUT_SCHEMA = STATUS_TOOL_OUTPUT_SERIALIZATION_SCHEMA
REFERENCE_TOOL_OUTPUT_SCHEMA = REFERENCE_TOOL_OUTPUT_SERIALIZATION_SCHEMA
```

The underlying constants are generated with `model_json_schema(mode="serialization")`; they are not manually maintained schemas.

Recommended server construction:

```python
from fastmcp import FastMCP

mcp = FastMCP(
    name="CodeFabric Present-State CPG",
    version="1.3.0",
    instructions=SERVER_INSTRUCTIONS,
    lifespan=server_lifespan,
    on_duplicate="error",
    strict_input_validation=True,
    mask_error_details=True,
    list_page_size=50,
    tasks=False,
)
```

### 20.1 Configuration rationale

| Setting | Decision | Reason |
|---|---|---|
| explicit name/version | required | stable product identity independent of dependencies |
| instructions | required | primary model-facing use policy |
| lifespan | required | settings, handshake, and one reusable daemon client |
| duplicate error | required | component collisions are contract defects |
| strict FastMCP input validation | recommended | outer JSON fields should not be coerced casually |
| masked generic errors | required | internal traces never leak through generic exceptions |
| bounded list page | safe default | resource/recipe catalogs remain bounded |
| tasks disabled | required | daemon owns scheduling and query state |
| no token auth layer | intentional | local STDIO security is process/socket/capability based |

### 20.2 Startup schema self-check

Before serving, the adapter SHOULD:

1. build every public Pydantic schema;
2. compare packaged schema snapshots to generated schemas;
3. verify the public schema bundle fingerprint;
4. verify packaged semantic request/response schema fingerprints;
5. fail startup on drift under an unchanged contract version.

### 20.3 STDIO entrypoint

```python
if __name__ == "__main__":
    mcp.run()
```

No code may write arbitrary bytes or text to STDOUT.

---

## 21. Public MCP component catalog

The model-visible catalog SHOULD remain small.

### Tools

1. `query_code_graph`
2. `validate_code_graph_query`
3. `get_code_graph_status`
4. `get_code_graph_reference`

### Resources

- query specification;
- semantic request and response schemas;
- adapter public output schemas;
- concise agent guide;
- recipe index and recipe templates;
- current capabilities and snapshot metadata;
- immutable large-result artifacts and subresources.

### Prompts

- `author_code_graph_query`;
- `interpret_code_graph_facts`.

A four-tool catalog makes discovery transforms unnecessary.

---

## 22. Primary tool: `query_code_graph`

### 22.1 Purpose

Execute one complete composable semantic CPG fact request against one atomically consistent current snapshot.

### 22.2 Input contract

```python
from typing import Any, Literal
from pydantic import Field
from fastmcp.dependencies import CurrentContext
from fastmcp.server.context import Context

@mcp.tool(
    name="query_code_graph",
    version="1.3",
    description=(
        "Execute one composable present-state CPG fact query. Put every needed "
        "entity lookup, fact retrieval, traversal, path, pattern, set combination, "
        "summary, and source-context block in the single request. Semantic values "
        "are plain language inside the structured request."
    ),
    tags={"cpg", "facts", "read", "primary"},
    timeout=120.0,
    annotations=READ_ONLY_CLOSED_WORLD_ANNOTATIONS,
    meta={
        "semantic_query_specification": "1.3",
        "adapter_contract": "1.3",
        "canonical": True,
        "daemon_backed": True,
    },
    output_schema=QUERY_TOOL_OUTPUT_SCHEMA,
)
async def query_code_graph(
    request: dict[str, Any] = Field(
        description=(
            "A complete request conforming to the Composable Semantic CPG Fact "
            "Query Specification 1.3. Use get_code_graph_reference or the "
            "cpg://schema/request/1.3 resource for the full schema."
        )
    ),
    delivery: Literal["automatic", "inline", "resource"] = Field(
        default="automatic",
        description=(
            "MCP delivery preference only. It does not change query semantics."
        ),
    ),
    ctx: Context = CurrentContext(),
) -> ToolResult:
    ...
```

### 22.3 Why `request` remains a dictionary

Publishing the entire nested semantic union as the tool input schema would consume substantial model context on every initialization. The handler therefore applies the reusable JSON-object `TypeAdapter`, serializes deterministically, and delegates full schema and semantic validation to the daemon.

### 22.4 Discriminated delivery output

The outer Pydantic output is transport-owned and uses the canonical query status dimensions plus the exact `PublicSnapshotMetadata` record:

```yaml
semantic_request_id: string
mcp_call_id: string
execution_state: COMPLETE | FAILED | CANCELLED | DEADLINE_EXCEEDED
availability_state: AVAILABLE | PARTIAL | UNAVAILABLE
completeness_state: COMPLETE | PARTIAL | INDETERMINATE | UNAVAILABLE
freshness_state: CURRENT | POTENTIALLY_STALE | UNAVAILABLE
limit_state: NOT_APPLIED | EXPLICIT_LIMIT_REACHED | HARD_LIMIT_REJECTED
snapshot:
  snapshot_id: string
  workspace_id: string
  repository_id: string | null
  worktree_id: string | null
  source_generation: integer
  source_inventory_digest: string
  durable_base_publication: string
  base_table_version_digest: string
  overlay_generation: integer
  overlay_checksum: string
  analysis_context_set_id: string
  analysis_context_ids: [string]
  freshness_state: CURRENT | POTENTIALLY_STALE | UNAVAILABLE
  source_trust_state: string
  event_stream_health: string
  git_acceleration_status: string
  git_operation_summary: object | null
  pending_update_count: integer
  ontology_version: string
  schema_bundle_version: string
  provider_bundle_version: string
  derivation_bundle_version: string
  query_language_version: string
  capability_summaries: [object]
  diagnostic_references: [string]
delivery:
  mode: inline
  canonical_mime_type: application/json
  result_bytes: integer
  checksum: string
  response: object
# OR
delivery:
  mode: resource
  canonical_mime_type: application/json
  result_bytes: integer
  checksum: string
  result_resource:
    uri: string
    manifest_uri: string
    expires_at: timestamp
    subresource_uris: [string]
  preview: object | null
counts: object
query_statuses: [object]
notices: [string]
```

The semantic request contains no delivery policy. The outer `delivery` argument affects only MCP inline/resource presentation and is excluded from canonical semantic request hashing.

### 22.5 Canonical response boundary

The nested inline `response` is intentionally typed as `dict[str, JsonValue]`. The Rust daemon has already validated the canonical response schema and referential integrity. The adapter validates the outer delivery contract without rebuilding every fact as a Python object.

### 22.6 Tool result shaping

The adapter SHALL construct and serialize explicit models:

```python
public_output = QueryToolOutput(...)
public_meta = PublicToolMeta(...)

return ToolResult(
    content=[TextContent(type="text", text=build_human_summary(public_output))],
    structured_content=public_output.model_dump(
        mode="json",
        exclude_none=True,
    ),
    meta=public_meta.model_dump(
        mode="json",
        exclude_none=True,
    ),
)
```

The adapter SHALL NOT return `meta=result.operational_meta` or otherwise forward an open daemon mapping.

---

## 23. Tool: `validate_code_graph_query`

### 23.1 Purpose

Validate and resolve a semantic request without executing fact retrieval.

Use cases:

- diagnose JSON or semantic schema errors;
- see how semantic phrases resolve;
- detect ambiguity;
- type-check prior-result references;
- inspect the query dependency DAG;
- detect unbounded paths or patterns;
- determine unavailable fact families;
- obtain a normalized request.

### 23.2 Pydantic output

`ValidateQueryOutput` SHALL define:

```text
valid
request_id
normalized_request
dependency_graph
resolved_semantics
capability_requirements
resource_estimate
errors
warnings
```

The outer structure is Pydantic-owned. `normalized_request` and `resolved_semantics` remain JSON values produced by the daemon.

Validation is objective service behavior, not codebase judgment.

---

## 24. Tool: `get_code_graph_status`

This tool returns a `StatusToolOutput` containing:

```text
adapter readiness
workspace and agent identity
active snapshot summary
FastMCP/Pydantic/settings/daemon/RPC/query/ontology/schema versions
supported languages and request forms
fact-family capability statuses
freshness state
hard service limits
safe notices
```

The output SHALL be assembled from explicit fields. It SHALL NOT expose socket paths, capability tokens, raw daemon configuration, or unfiltered status mappings.

The tool SHALL NOT trigger CPG generation or wait for a new publication.

---

## 25. Tool: `get_code_graph_reference`

This tool provides tool-only clients access to the same guidance and schemas exposed as native MCP resources.

Allowed references SHOULD be a constrained enum:

```text
agent_guide
query_specification
request_schema
response_schema
query_tool_output_schema
validate_tool_output_schema
status_tool_output_schema
reference_tool_output_schema
recipe_index
recipe:<name>
capabilities
```

The response SHALL use a discriminated `ReferenceToolOutput` for inline content or a resource URI. This is not a generic filesystem reader.

---

## 26. Tool annotations

All tools SHALL use read-only annotations equivalent to:

```python
from mcp.types import ToolAnnotations

READ_ONLY_CLOSED_WORLD_ANNOTATIONS = ToolAnnotations(
    title="Query Code Graph",
    readOnlyHint=True,
    destructiveHint=False,
    idempotentHint=True,
    openWorldHint=False,
)
```

Annotations are advisory and are not authorization controls.

---

## 27. Resources

### 27.1 Static contract resources

```text
cpg://guide/agent
cpg://spec/query/1.3
cpg://schema/request/1.3
cpg://schema/response/1.3
cpg://schema/mcp/query-tool-output/1.3
cpg://schema/mcp/validate-tool-output/1.3
cpg://schema/mcp/status-tool-output/1.3
cpg://schema/mcp/reference-tool-output/1.3
cpg://recipes/index
cpg://recipes/<recipe-name>
```

Recommended MIME types:

```text
guide/specification/recipes   text/markdown
JSON schemas                  application/schema+json
```

### 27.2 Live informational resources

```text
cpg://capabilities/current
cpg://snapshot/current
```

Dynamic resource output SHALL be validated through the same public status models or a resource-specific strict model.

### 27.3 Result resources

```text
cpg-result://<result-id>
cpg-result://<result-id>/manifest
cpg-result://<result-id>/query/<query-id>
cpg-result://<result-id>/entities
cpg-result://<result-id>/facts
cpg-result://<result-id>/paths
cpg-result://<result-id>/groups
cpg-result://<result-id>/source-contexts
cpg-result://<result-id>/chunk/<chunk-index>
```

The root resource contains the complete canonical response. Subresources are deterministic projections, not new queries.

### 27.4 Resource metadata

Resource metadata SHALL use explicit Pydantic allowlists. Result IDs, checksums, byte counts, publication IDs, and expiry may be exposed. Internal filesystem paths, storage keys, retry state, and daemon error chains may not.

### 27.5 Resource security

Every read forwards agent/workspace identity and the daemon enforces ownership and expiry. The adapter never resolves result IDs to arbitrary filesystem paths.

---

## 28. Prompts

### 28.1 `author_code_graph_query`

The prompt SHOULD direct the model to:

- formulate objective facts rather than conclusions;
- combine all needed query blocks in one request;
- use prior-result references;
- keep exact, possible, heuristic, and unresolved facts separate;
- ask for direct facts before transitive expansion;
- bound path and pattern expansion;
- retrieve source text only after semantic filtering;
- use canonical IDs from prior responses where available.

### 28.2 `interpret_code_graph_facts`

The prompt SHOULD direct the model to:

- cite returned facts and source contexts;
- preserve uncertainty and directness;
- treat missing coverage as indeterminate;
- keep syntax, semantic entities, call sites, and executable instances separate;
- identify query-block failures and unavailable capabilities;
- label engineering conclusions as downstream reasoning.

---

## 29. Server instructions

```text
This server returns objective present-state code-property-graph facts for the
current indexed workspace. Use query_code_graph for substantive work.

Place every needed entity lookup, fact retrieval, relationship traversal, path,
pattern, set combination, deterministic summary, and source-context request into
one composable request. Give every block a query_id and reference earlier results.

Use plain semantic language inside the structured request. Do not send SQL,
physical table names, graph edge labels, regular expressions, compiler IDs, or
database syntax.

Keep exact, possible, heuristic, and unresolved facts distinct. Keep direct and
transitive facts distinct. Do not infer absence unless the response establishes
fact-family completeness or returns an explicit negative fact.

Prefer direct facts first, then bounded transitive traversal. Request source text
only for the final relevant subset.

This server does not decide whether a refactor is safe, which change should be
made, whether code is risky, or which tests are impacted. Ask for the factual
callers, callees, reads, writes, aliases, overrides, implementations, control
flow, types, source context, and unresolved facts needed for your own reasoning.

Use validate_code_graph_query when a request is ambiguous or rejected. Use
get_code_graph_reference for the complete schema and recipes. Large results may
be returned as immutable cpg-result resources rather than inline.
```

---

## 30. Why the server does not expose eight query-form tools

The eight request forms are blocks inside one dependency DAG. Separate tools would force multiple round trips, break fan-in/fan-out composition, complicate one-snapshot execution, duplicate scope/defaults, and increase selection burden.

---

## 31. Deliberate FastMCP exclusions

The canonical path SHALL not use:

- background tasks, because the daemon already owns query scheduling and state;
- Tool Search or Code Mode, because the catalog contains only four tools;
- response caching middleware, because publication and agent identity must be part of cache semantics;
- session state for semantic truth, because each request is explicit and the daemon owns snapshots;
- Apps or interactive UI components, because programming agents need a compact factual contract;
- HTTP transport, because the selected deployment is one local STDIO process per agent.

---

## 32. Query and static-resource caching policy

The adapter SHALL not cache query responses. The daemon may cache plans/results keyed by publication, canonical request hash, workspace, and authorization.

Static guide and schema resources MAY be loaded once from package resources. Generated Pydantic schemas MAY be cached as module constants because their defining model set is immutable for the process lifetime.

---
# Part IV — Lifespan, Settings, Dependency Injection, and Middleware

## 33. FastMCP lifespan and immutable settings

The lifespan owns exactly one validated settings snapshot, one reusable daemon client, one validated handshake summary, and their cleanup.

```python
from dataclasses import dataclass
from pydantic import ValidationError
from fastmcp.server.lifespan import lifespan


@dataclass(frozen=True, slots=True)
class RuntimeState:
    settings: Settings
    daemon: CpgDaemonClient
    handshake: HandshakeSummary


@lifespan
async def server_lifespan(server):
    try:
        settings = Settings()
    except ValidationError as exc:
        emit_safe_startup_validation_errors(exc)  # STDERR only
        raise RuntimeError("Invalid CodeFabric CPG adapter configuration") from exc

    client = await CpgDaemonClient.connect(settings)
    handshake_raw = await client.handshake(
        agent_instance_id=settings.agent_instance_id,
        workspace_id=settings.workspace_id,
        adapter_version=ADAPTER_VERSION,
        capability_token=settings.capability_token.get_secret_value(),
    )
    handshake = HandshakeSummary.model_validate(handshake_raw)
    verify_compatibility(handshake)

    try:
        yield RuntimeState(settings=settings, daemon=client, handshake=handshake)
    finally:
        await client.aclose()
```

### 33.1 Settings source policy

Production settings source order SHALL be explicit:

```text
constructor overrides used by tests/controlled launchers
    > environment variables
        > configured file-secret source
```

Dotenv and CLI parsing SHALL be disabled in the production `Settings` class. Developer tests MAY explicitly provide a separate test settings source.

### 33.2 Secret policy

`SecretStr` reduces accidental representation/log leakage but is not an encryption boundary. The raw token may be unwrapped only where constructing daemon authentication metadata. It SHALL not appear in:

- model dumps;
- `ToolResult.meta`;
- validation diagnostics;
- trace attributes;
- exception text;
- launch files committed to source control.

### 33.3 Connection model

The client SHOULD support multiple in-flight requests over one channel. gRPC multiplexes streams; no global Python query lock is required.

### 33.4 Readiness

If settings, schema self-check, handshake, or compatibility validation fails, the process SHALL fail fast rather than serve a guaranteed-broken contract.

---

## 34. Dependency injection boundary

Use `CurrentContext()` for request-scoped MCP behavior and a custom dependency for typed runtime state.

```python
from fastmcp.dependencies import CurrentContext, Depends


def runtime_from_context(ctx: Context = CurrentContext()) -> RuntimeState:
    state = ctx.lifespan_context
    if not isinstance(state, RuntimeState):
        raise RuntimeError("FastMCP lifespan state is unavailable")
    return state


def daemon_from_runtime(
    state: RuntimeState = Depends(runtime_from_context),
) -> CpgDaemonClient:
    return state.daemon
```

Injected infrastructure SHALL not appear in the model-visible tool schema.

Pydantic SHALL not be used to hold live gRPC channels or request contexts. Those are ordinary typed runtime objects owned by lifespan and DI.

---

## 35. Middleware stack

Recommended order:

```text
1. ErrorHandlingMiddleware
2. TraceAndCorrelationMiddleware
3. AdapterAdmissionMiddleware
4. DetailedTimingMiddleware
5. StructuredLoggingMiddleware
6. resolved FastMCP handler
```

### 35.1 Error handling

Generic exceptions are masked. Intentional safe errors use `ToolError`, `ResourceError`, or `PromptError`. Pydantic errors are translated through a dedicated safe-error function before client exposure.

### 35.2 Trace and correlation

Establish:

```text
trace_id
MCP request_id
agent_instance_id
workspace_id
component name/version
Pydantic public contract version
```

Forward trace context to the daemon.

### 35.3 Admission

A modest per-process cap MAY prevent runaway local loops, but daemon admission is authoritative. Recommended initial defaults:

```text
4 requests per second
burst capacity 8
maximum 4 concurrent query calls per adapter
```

### 35.4 Timing

Distinguish:

```text
FastMCP input validation
JSON structural validation
canonical request serialization
query validation/execution RPC
public output model validation
public output serialization
result resource reads
status/reference calls
```

### 35.5 Structured logging

Payload logging is disabled by default. Log hashes, counts, statuses, versions, and timings. Never log `ValidationError` objects wholesale because their details can include raw input.

### 35.6 Retry

Broad server retry middleware is prohibited. The daemon client may retry only idempotent, resumable transport failures under the RPC contract.

---

## 36. Internal vs client-visible logging

### Client-visible through `Context`

- semantic phase progress;
- concise notice that a result was externalized;
- actionable ambiguity or limit guidance;
- safe contract-validation paths and codes;
- reconnect notice only when it affects the call.

### STDERR / telemetry only

- stack traces;
- raw `ValidationError` details;
- input values;
- socket paths;
- capability tokens;
- physical plans;
- source/result payloads;
- daemon internal error chains;
- retry internals.

---
# Part V — Query Semantics, Validation, and Error Mapping

## 37. Validation layers

```text
Layer 0: MCP transport/host limits
  frame/message size and process ownership

Layer 1: FastMCP declaration validation
  request is an object and delivery preference is valid

Layer 2: adapter structural guard
  reusable TypeAdapter[dict[str, JsonValue]]
  finite numbers, string keys, recursive JSON values
  canonical JSON byte/depth/node limits

Layer 3: daemon JSON Schema
  complete semantic request-envelope and query-form contract

Layer 4: daemon semantic validator
  controlled-language resolution
  fact-only boundary
  result-reference typing
  dependency DAG
  capability/coverage rules
  boundedness and resource governance

Layer 5: daemon canonical response validation
  response JSON Schema
  dictionary references
  deterministic ordering
  identity and snapshot consistency

Layer 6: adapter public-envelope validation
  QueryToolOutput / ValidateQueryOutput / StatusToolOutput
  discriminated delivery invariants
  public metadata allowlist

Layer 7: Pydantic serialization contract
  model_dump(mode="json") matching serialization-mode schema
```

The daemon is authoritative for layers 3–5. Pydantic is authoritative for layers 2, 6, and 7 only.

### 37.1 Input complexity preflight

Before semantic execution, the adapter SHALL enforce:

```text
root object requirement
maximum canonical request bytes
maximum JSON depth
maximum JSON container/value count
maximum translated validation errors
```

Transport limits SHOULD bound input before Python materialization where the MCP host permits configuration. Pydantic field constraints do not replace message-size limits.

### 37.2 Output validation cost boundary

The adapter validates the small outer envelope and metadata. It SHALL not traverse and instantiate one model per canonical CPG fact. This preserves the thin-adapter performance invariant.

---

## 38. Snapshot and freshness behavior

The adapter displays daemon-returned `PublicSnapshotMetadata` and never substitutes locally cached status.

The daemon applies the request's structured freshness policy before pinning. Once `QuerySnapshotPinned` is emitted, execution and any artifact remain bound to that immutable `ServingSnapshot`, including its overlay and context set, even when a newer snapshot becomes active.

Only explicit `best_available_snapshot` may produce `POTENTIALLY_STALE`. Current-required requests return current facts or canonical unavailability/freshness failure.

## 39. Query-level vs tool-level errors

### 39.1 Query-level errors in the canonical response

Examples:

- one query block is semantically ambiguous;
- one fact family is unavailable;
- a dependent block is not executed;
- an explicit query limit is reached;
- coverage is partial.

This preserves successful independent branches.

### 39.2 Tool-level errors terminate the MCP call

Examples:

- settings or adapter contract is invalid;
- adapter cannot connect to the daemon;
- protocol/schema versions are incompatible;
- capability token is rejected;
- request exceeds a hard adapter limit;
- daemon cannot pin a queryable snapshot;
- terminal RPC data cannot be validated into application DTOs;
- public output construction fails;
- canonical inline JSON is corrupt.

Use a safe `ToolError`; retain full diagnostics internally.

---

## 40. Error registry and Pydantic translation

The adapter preserves the semantic-query 1.3 canonical error code/envelope. Adapter-local codes are limited to the adapter boundary:

| Code | Meaning | MCP behavior |
|---|---|---|
| `ADAPTER_INPUT_NOT_JSON` | non-JSON Python value in compact tool input | safe ToolError |
| `ADAPTER_INPUT_LIMIT` | byte/depth/node cap exceeded | safe ToolError |
| `ADAPTER_INPUT_VALIDATION` | compact adapter envelope invalid | safe structured issues |
| `ADAPTER_OUTPUT_CONTRACT` | terminal daemon data cannot form public model | masked ToolError + alert |
| `DAEMON_UNAVAILABLE` | local transport/service unavailable | ToolError |
| `CONTRACT_MISMATCH` | RPC/schema/bundle incompatibility | startup failure or ToolError |

Semantic errors retain codes such as `INVALID_REQUEST_SCHEMA`, `SEMANTIC_PHRASE_AMBIGUOUS`, `CONTEXT_NOT_INDEXED`, `COMPOSITE_SNAPSHOT_UNSUPPORTED`, `CURRENT_FACTS_UNAVAILABLE`, `FRESHNESS_DEADLINE_EXCEEDED`, `QUERY_HARD_LIMIT_EXCEEDED`, `CANCELLED`, and `INTERNAL_INVARIANT_VIOLATION`.

The public error envelope carries code, layer, retryability, safe message, and optional field/phrase/candidates/dependency/diagnostic ID. The adapter SHALL not rename semantic failures.

### 40.1 Safe Pydantic error translation

Consume `ValidationError.errors()` and omit input, context, URLs, causes, tracebacks, and object repr. Translate only a bounded number of safe code/path/message records.

### 40.2 Usage errors and snippet validity

`PydanticUserError`, invalid model definitions, or unsupported serialization arguments are adapter defects and fail startup/tests. Every normative Python snippet SHALL be import/compile tested against Pydantic 2.13.4 and FastMCP 3.4.7. `hide_input_in_errors` belongs in model/adapter configuration and SHALL not be passed to `model_dump`.

## 41. No silent fallback or unrestricted serialization

The adapter and daemon SHALL NOT silently substitute:

- syntax text for semantic type resolution;
- name equality for semantic identity;
- possible targets for exact targets;
- stale publication data for current requirements;
- local filesystem source for daemon source context;
- partial results as complete;
- a resource summary as the canonical response;
- raw daemon metadata for a public output model;
- subclass-only fields through broad duck-typed serialization.

Public output SHALL use annotation-driven Pydantic serialization. `SerializeAsAny`, `serialize_as_any=True`, broad polymorphic serialization, and generic fallback serializers are prohibited on public contracts.

---

## 42. Resource governance

The daemon SHALL bound:

```text
request bytes and query-block count
semantic phrase length
result-reference count
pattern bindings and relationships
path depth/count/frontier
source-context bytes
requested result count
execution time
memory and spill
per-agent/global concurrency
```

The adapter additionally bounds:

```text
recursive JSON depth
recursive JSON node count
Pydantic issue count
inline public-envelope size
reference/resource metadata size
```

An explicit semantic query limit is part of query meaning. A hard operational limit is not; it produces a rejection or clearly incomplete status.

---
# Part VI — Delivery Adaptation

## 43. Inline and resource delivery as a discriminated union

Recommended defaults:

```text
inline target threshold       256–512 KiB, host-benchmarked
maximum inline hard limit     configurable
resource chunk size           256 KiB
result TTL                    30 minutes or process/session lifetime
```

### 43.1 `automatic`

- inline below the configured threshold;
- immutable resource above it;
- never truncate merely to remain inline.

### 43.2 `inline`

- attempt inline;
- reject or explicitly externalize if the hard host limit would be exceeded;
- report final behavior.

### 43.3 `resource`

- always preserve the full canonical response as an immutable result artifact;
- return only a typed manifest and optional small preview.

### 43.4 Pydantic delivery variants

```python
class InlineDelivery(StrictWireModel):
    mode: Literal["inline"]
    canonical_mime_type: Literal["application/json"] = "application/json"
    result_bytes: NonNegativeInt
    checksum: Checksum
    response: dict[str, JsonValue]


class ResourceDelivery(StrictWireModel):
    mode: Literal["resource"]
    canonical_mime_type: Literal["application/json"] = "application/json"
    result_bytes: NonNegativeInt
    checksum: Checksum
    result_resource: ResultResource
    preview: dict[str, JsonValue] | None = None


Delivery = Annotated[
    InlineDelivery | ResourceDelivery,
    Field(discriminator="mode"),
]
```

---

## 44. One logical response under MCP v3

The semantic request produces one daemon response envelope containing every query result.

When host payload limits make a large inline result impractical, the tool response contains one immutable resource reference to that complete envelope. Reading the resource is transport-level content retrieval, not a second CPG query or recomputation.

The manifest SHALL state:

```text
complete canonical response exists
resource URI and manifest URI
snapshot/publication
byte size and checksum
query-result count
expiry
```

---

## 45. Result subresources

Subresources make large responses agent-efficient without changing semantics.

```text
query_code_graph returns cpg-result://R

agent may read:
  cpg-result://R/manifest
  cpg-result://R/query/direct_calls
  cpg-result://R/facts
  cpg-result://R/source-contexts
```

Every subresource retains canonical IDs, publication metadata, and checksums. The root response remains authoritative.

Result manifest and public resource metadata SHALL be Pydantic-validated. Raw result bytes remain daemon-owned canonical JSON.

---

## 46. Human summary

Every tool call SHOULD include a concise text block generated from `QueryToolOutput`, for example:

```text
Complete on snapshot snapshot:7b… (publication 421).
8 query blocks returned 14 entities, 63 facts, 2 paths, and 6 source contexts.
The 94 KiB canonical response is included inline.
```

or:

```text
Complete on snapshot snapshot:7b… (publication 421).
The 8.7 MiB canonical response was preserved at cpg-result://r_8f… and expires
at 2026-08-19T22:10:00-05:00. No facts were truncated.
```

The summary is navigation only. The Pydantic `structured_content` and canonical daemon response are authoritative.

---
# Part VII — Agent Guidance and Recipes

## 47. Preferred query-authoring sequence

1. Define the objective facts needed for the programming task.
2. Put all related fact requests into one request envelope.
3. Use one `find code entities` block for each initial anchor.
4. Reuse returned roles through `results_of` references.
5. Request direct call, state, type, CFG, dataflow, alias, or ownership facts.
6. Add bounded transitive traversal only where needed.
7. Combine result sets by canonical identity rather than repeated text matching.
8. Summarize objective facts only after the underlying fact sets are defined.
9. Retrieve source/syntax context for the final relevant facts.
10. Inspect coverage and unknowns before drawing conclusions.

---

## 48. Recipe: inspect one callable comprehensively

```yaml
specification: composable semantic CPG fact query
version: "1.3"
semantic_request_id: inspect-graph-store-commit
scope:
  workspace_id: workspace:0123456789abcdef0123456789abcdef
  codebase: the current authorized indexed workspace
  analysis_contexts:
    mode: default
freshness:
  policy: require_current_for_targets
  target_scope: infer from query inputs
defaults:
  uncertainty: include exact, possible, heuristic, and unresolved facts and keep them separate
  unknowns: include explicit unknown entities and relationships whenever relevant
  absence: assert absence only when the scoped fact family is complete or an explicit negative fact exists
  representation: do not collapse source occurrences, semantic entities, call sites, executable instances, or lowered entities
queries:
  - query_id: target
    request: find code entities
    looking_for: the callable `GraphStore::commit`
    return:
      include:
        - canonical semantic identity
        - callable signature
        - source location

  - query_id: contract
    request: retrieve facts about code
    about:
      - results_of: target
        select: the returned callable entities
    facts:
      - complete callable contract
      - declared, inferred, computed, expected, and narrowed types where available
      - generic parameters and concrete executable specializations

  - query_id: calls
    request: retrieve facts about code
    about:
      - results_of: target
        select: the returned callable entities
    facts:
      - every directly contained call site
      - receiver, arguments, and argument-to-parameter bindings
      - exact, possible, heuristic, and unknown targets kept separate

  - query_id: state
    request: retrieve facts about code
    about:
      - results_of: target
        select: the returned callable entities
    facts:
      - direct reads and writes to abstract memory locations
      - direct effects, exceptions, cleanup, and resource events
      - unresolved memory and effect facts

  - query_id: context
    request: retrieve source and syntax context
    for:
      - results_of: target
        select: the returned callable entities
    context:
      - exact source for the callable body
      - enclosing type and module outline
```

---

## 49. Recipe: callers that also write a selected location

```yaml
queries:
  - query_id: target
    request: find code entities
    looking_for: the callable `GraphStore::commit`

  - query_id: callers
    request: follow code relationships
    starting_from:
      - results_of: target
        select: the returned callable entities
    relationship: direct callers through first-class call-site facts
    direction: from callee to caller
    distance: one relationship step

  - query_id: writers
    request: find code entities
    looking_for: callables that directly write the abstract location `transaction_state`

  - query_id: both
    request: combine result sets
    inputs:
      - results_of: callers
        select: the direct caller entities
      - results_of: writers
        select: the writer callable entities
    combination: intersection by canonical semantic identity

  - query_id: context
    request: retrieve source and syntax context
    for:
      - results_of: both
        select: the combined callable entities
    context:
      - the containing statement and callable source
```

---

## 50. Recipe: Rust ownership and unwind facts

```yaml
queries:
  - query_id: rust_fn
    request: find code entities
    looking_for: the Rust function `apply_change`

  - query_id: ownership
    request: retrieve facts about code
    about:
      - results_of: rust_fn
        select: the source-authored callable and its Rust MIR body
    facts:
      - every move, copy, shared borrow, mutable borrow, reborrow, and raw address-taking event
      - structured places and projections for each event
      - initialization and move state at each relevant program point
      - active loans and regions when available
      - drop and drop-glue facts
      - normal and unwind control-flow successors kept separate
      - explicit unknown ownership or alias facts

  - query_id: paths
    request: find connecting fact paths
    from:
      - results_of: ownership
        select: definitions of values that are moved
    to:
      - results_of: ownership
        select: drops or uses of the same values
    relationships:
      - value flow
      - reaching definition
      - move and drop relationships
    path_policy: all shortest fact paths up to twelve relationship steps
```

---

## 51. Recipe: Python type and dispatch facts

```yaml
queries:
  - query_id: call
    request: find code entities
    looking_for: the Python call site to `handler.process` inside `dispatch_request`

  - query_id: semantics
    request: retrieve facts about code
    about:
      - results_of: call
        select: the returned call-site entities
    facts:
      - receiver expression and computed receiver type
      - member-resolution order and descriptor/property semantics
      - declared target, exact target, sound possible targets, heuristic targets, and unknown target
      - argument-to-parameter bindings
      - declared, computed, expected, and narrowed argument types
      - decorator or callable-object behavior that affects dispatch

  - query_id: source
    request: retrieve source and syntax context
    for:
      - results_of: call
        select: the returned call-site entities and target declarations
    context:
      - containing statement
      - enclosing callable signature
      - target declaration signatures
```

---

## 52. Recipe: bounded dependency path

```yaml
queries:
  - query_id: start
    request: find code entities
    looking_for: the module `api.handlers`

  - query_id: end
    request: find code entities
    looking_for: the module `storage.delta`

  - query_id: path
    request: find connecting fact paths
    from:
      - results_of: start
        select: the returned module entities
    to:
      - results_of: end
        select: the returned module entities
    relationships:
      - imports or re-exports
      - direct calls through call-site facts
    path_policy: all shortest paths with at most eight relationship steps
    return:
      include:
        - ordered entity and fact IDs
        - certainty summary
        - supporting source locations
```

---

## 53. Agent interpretation checklist

```text
[ ] Did every query result come from the same snapshot?
[ ] Are exact and possible targets separated?
[ ] Are direct and transitive facts separated?
[ ] Are generated/lowered entities distinct from source-authored entities?
[ ] Are call sites distinct from callables?
[ ] Are values distinct from memory locations?
[ ] Are moves distinct from copies and borrows?
[ ] Are normal and unwind paths distinct?
[ ] Are unknowns explicit?
[ ] Is the relevant fact family complete for any negative inference?
[ ] Did an explicit or hard limit make a result incomplete?
[ ] Did any dependency query fail?
[ ] Are conclusions clearly downstream reasoning rather than CPG facts?
```

---

# Part VIII — Python Implementation Specification

## 54. Recommended repository layout

```text
codefabric-cpg-mcp/
  pyproject.toml
  uv.lock
  README.md
  src/
    codefabric_cpg_mcp/
      __init__.py
      __main__.py
      server.py
      settings.py
      instructions.py
      telemetry.py
      middleware.py
      delivery.py
      schema_export.py
      contracts/
        __init__.py
        types.py            # reusable Annotated scalar contracts
        public.py           # model-visible MCP output models
        daemon.py           # application-owned daemon DTOs
        json.py             # reusable TypeAdapter registry
        errors.py           # safe ValidationError translation
      daemon/
        __init__.py
        client.py
        generated/
          cpg_query_pb2.py
          cpg_query_pb2_grpc.py
      resources/
        agent_guide.md
        query_specification.md
        request.schema.json
        response.schema.json
        mcp_schemas/
          query-tool-output-1.3.schema.json
          validate-tool-output-1.3.schema.json
          status-tool-output-1.3.schema.json
          reference-tool-output-1.3.schema.json
        recipes/
          index.md
          inspect_callable.yaml
          rust_ownership.yaml
          python_dispatch.yaml
      components/
        tools.py
        resources.py
        prompts.py
  proto/
    cpg_query_service.proto
  tests/
    unit/
    contracts/
    settings/
    in_memory/
    stdio/
    daemon_integration/
    conformance/
    load/
```

The package SHALL not contain one giant `models.py`. Settings, public wire contracts, daemon DTOs, and canonical semantic payloads have different trust boundaries.

---

## 55. Settings implementation

```python
from __future__ import annotations

import secrets
from typing import Annotated, Literal

from pydantic import AliasChoices, Field, SecretStr, model_validator
from pydantic_settings import (
    BaseSettings,
    PydanticBaseSettingsSource,
    SettingsConfigDict,
)


OpaqueId = Annotated[
    str,
    Field(
        min_length=1,
        max_length=256,
        pattern=r"^[A-Za-z0-9][A-Za-z0-9_.:-]*$",
    ),
]


class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        case_sensitive=True,
        extra="ignore",
        frozen=True,
        validate_default=True,
        hide_input_in_errors=True,
        validate_by_alias=True,
        validate_by_name=True,
        env_file=None,
        cli_parse_args=False,
    )

    daemon_target: Annotated[str, Field(min_length=1, max_length=2048)] = Field(
        validation_alias=AliasChoices(
            "CODEFABRIC_CPG_DAEMON_TARGET",
            "CODEFABRIC_DAEMON_TARGET",
        )
    )
    workspace_id: OpaqueId = Field(
        validation_alias="CODEFABRIC_WORKSPACE_ID"
    )
    agent_instance_id: OpaqueId = Field(
        default_factory=lambda: f"stdio-{secrets.token_hex(8)}",
        validation_alias="CODEFABRIC_AGENT_INSTANCE_ID",
    )
    capability_token: SecretStr = Field(
        validation_alias="CODEFABRIC_CPG_CAPABILITY_TOKEN"
    )

    query_timeout_seconds: Annotated[float, Field(gt=0, le=600)] = Field(
        default=120.0,
        validation_alias="CODEFABRIC_CPG_QUERY_TIMEOUT_SECONDS",
    )
    inline_result_bytes: Annotated[
        int, Field(ge=16 * 1024, le=8 * 1024 * 1024)
    ] = Field(
        default=384 * 1024,
        validation_alias="CODEFABRIC_CPG_INLINE_RESULT_BYTES",
    )
    max_request_bytes: Annotated[
        int, Field(ge=64 * 1024, le=16 * 1024 * 1024)
    ] = Field(
        default=2 * 1024 * 1024,
        validation_alias="CODEFABRIC_CPG_MAX_REQUEST_BYTES",
    )
    max_json_depth: Annotated[int, Field(ge=4, le=128)] = Field(
        default=48,
        validation_alias="CODEFABRIC_CPG_MAX_JSON_DEPTH",
    )
    max_json_nodes: Annotated[int, Field(ge=100, le=2_000_000)] = Field(
        default=100_000,
        validation_alias="CODEFABRIC_CPG_MAX_JSON_NODES",
    )
    max_validation_errors: Annotated[int, Field(ge=1, le=100)] = Field(
        default=20,
        validation_alias="CODEFABRIC_CPG_MAX_VALIDATION_ERRORS",
    )
    result_ttl_seconds: Annotated[int, Field(ge=60, le=86_400)] = Field(
        default=1_800,
        validation_alias="CODEFABRIC_CPG_RESULT_TTL_SECONDS",
    )
    log_level: Literal["CRITICAL", "ERROR", "WARNING", "INFO", "DEBUG"] = Field(
        default="INFO",
        validation_alias="CODEFABRIC_CPG_LOG_LEVEL",
    )

    @model_validator(mode="after")
    def validate_daemon_target(self) -> "Settings":
        if not self.daemon_target.startswith(("unix://", "tcp://")):
            raise ValueError("daemon_target must use unix:// or tcp://")
        return self

    @classmethod
    def settings_customise_sources(
        cls,
        settings_cls: type[BaseSettings],
        init_settings: PydanticBaseSettingsSource,
        env_settings: PydanticBaseSettingsSource,
        dotenv_settings: PydanticBaseSettingsSource,
        file_secret_settings: PydanticBaseSettingsSource,
    ) -> tuple[PydanticBaseSettingsSource, ...]:
        return init_settings, env_settings, file_secret_settings
```

Rules:

- construct `Settings()` once during lifespan;
- do not reread environment variables on each tool call;
- fail startup on invalid required configuration;
- do not mutate settings in place;
- use explicit constructor settings in tests;
- unwrap `capability_token` only at the daemon-authentication boundary.

---

## 56. Public contracts, daemon DTOs, and reusable adapters

### 56.1 Public models

The public model family SHALL include at least:

```text
SnapshotSummary
QueryCounts
QueryStatus
ResultResource
InlineDelivery
ResourceDelivery
QueryToolOutput
PublicToolMeta
ValidationIssue
ValidateQueryOutput
StatusToolOutput
ReferenceToolOutput
```

The full reference implementation is supplied in `codefabric_cpg_mcp_pydantic_contracts.py`.

### 56.2 Daemon DTO policy

Generated Protobuf messages are the transport contract. The client wrapper converts accepted/terminal messages into application-owned Pydantic DTOs for invariants such as:

- exactly one of inline response or result manifest;
- non-negative counts/timings;
- known terminal status;
- valid snapshot and result identifiers;
- finite numeric telemetry.

High-frequency progress frames SHOULD remain lightweight generated Protobuf objects unless profiling demonstrates value in additional validation.

### 56.3 JSON adapter registry

```python
# contracts/json.py
from pydantic import ConfigDict, JsonValue, TypeAdapter

JSON_OBJECT_ADAPTER = TypeAdapter(
    dict[str, JsonValue],
    config=ConfigDict(
        strict=True,
        allow_inf_nan=False,
        hide_input_in_errors=True,
    ),
)
```

Never construct this adapter per call.

### 56.4 Safe error translation

```python
# contracts/errors.py
from pydantic import ValidationError


def safe_validation_issues(
    exc: ValidationError,
    *,
    limit: int,
) -> tuple[ValidationIssue, ...]:
    issues: list[ValidationIssue] = []
    for item in exc.errors()[:limit]:
        issues.append(
            ValidationIssue(
                code=str(item.get("type", "validation_error")),
                path=tuple(
                    p if isinstance(p, (str, int)) else str(p)
                    for p in item.get("loc", ())
                ),
                message=str(item.get("msg", "Invalid value."))[:1024],
            )
        )
    return tuple(issues)
```

Do not expose `input`, `ctx`, URLs, causes, or validator traces.

---

## 57. Daemon client interface

```python
from collections.abc import AsyncIterator, Awaitable, Callable
from dataclasses import dataclass
from typing import Literal

ProgressCallback = Callable[[float, float | None, str], Awaitable[None]]

@dataclass(frozen=True)
class AcceptedQuery:
    daemon_query_id: str
    workspace_id: str
    resume_token: bytes
    events: AsyncIterator[DaemonQueryEvent]

class CpgDaemonClient:
    @classmethod
    async def connect(cls, settings: Settings) -> "CpgDaemonClient": ...

    async def handshake(self, **identity: object) -> HandshakeSummary: ...
    async def get_status(self) -> DaemonStatusSummary: ...
    async def validate_query(self, request_json: bytes) -> DaemonValidationResult: ...

    async def start_query(
        self,
        *,
        request_json: bytes,
        delivery: Literal["automatic", "inline", "resource"],
        semantic_request_id: str,
        mcp_call_id: str,
        rpc_attempt_id: str,
        timeout_seconds: float,
    ) -> AcceptedQuery: ...

    async def await_terminal(
        self,
        accepted: AcceptedQuery,
        *,
        progress: ProgressCallback,
    ) -> DaemonQueryResult: ...

    async def read_result(self, *, result_id: str, subresource: str | None = None) -> bytes: ...
    async def cancel_query(self, daemon_query_id: str) -> None: ...
    async def aclose(self) -> None: ...
```

`start_query` returns as soon as the unary `StartQuery` response supplies the accepted handle and resume token, then binds `events` to `StreamQuery`. Reattachment uses the same opaque token with an event sequence cursor. The interface exposes no Arrow/DataFusion/Delta/graph types.

## 58. Primary tool implementation pattern

```python
import asyncio
import secrets
from pydantic import ValidationError
from fastmcp.exceptions import ToolError
from fastmcp.tools.tool import ToolResult
from mcp.types import TextContent

async def query_code_graph(
    request: dict[str, object],
    delivery: DeliveryPreference = "automatic",
    ctx: Context = CurrentContext(),
    runtime: RuntimeState = Depends(runtime_from_context),
) -> ToolResult:
    settings = runtime.settings

    try:
        json_request = JSON_OBJECT_ADAPTER.validate_python(request)
    except ValidationError as exc:
        raise ToolError(
            format_safe_input_error(
                safe_validation_issues(exc, limit=settings.max_validation_errors)
            )
        ) from exc

    guard_json_complexity(
        json_request,
        max_depth=settings.max_json_depth,
        max_nodes=settings.max_json_nodes,
    )

    json_request, semantic_request_id = ensure_effective_semantic_request_id(
        json_request
    )
    request_json = canonical_json_bytes(json_request)
    if len(request_json) > settings.max_request_bytes:
        raise ToolError("The semantic request exceeds the adapter byte limit.")

    mcp_call_id = str(ctx.request_id)
    rpc_attempt_id = f"rpc-{secrets.token_hex(8)}"

    accepted: AcceptedQuery | None = None
    try:
        accepted = await runtime.daemon.start_query(
            request_json=request_json,
            delivery=delivery,
            semantic_request_id=semantic_request_id,
            mcp_call_id=mcp_call_id,
            rpc_attempt_id=rpc_attempt_id,
            timeout_seconds=settings.query_timeout_seconds,
        )
        result = await runtime.daemon.await_terminal(
            accepted,
            progress=lambda done, total, message: ctx.report_progress(
                done, total, message
            ),
        )
    except asyncio.CancelledError:
        if accepted is not None:
            await runtime.daemon.cancel_query(accepted.daemon_query_id)
        raise
    except KnownDaemonError as exc:
        raise ToolError(exc.safe_message) from exc
    except Exception as exc:
        raise ToolError(
            "The CPG daemon could not complete the request. "
            "Use get_code_graph_status to check readiness."
        ) from exc

    try:
        public_output = build_public_query_output(result)
        public_meta = build_public_tool_meta(result)
    except ValidationError as exc:
        record_internal_contract_failure(exc, result)
        raise ToolError(
            "The CPG daemon returned a result that failed the adapter contract."
        ) from exc

    return ToolResult(
        content=[TextContent(type="text", text=build_human_summary(public_output))],
        structured_content=public_output.model_dump(mode="json", exclude_none=True),
        meta=public_meta.model_dump(mode="json", exclude_none=True),
    )
```

`canonical_json_bytes` is the shared `codefabric-jcs-v1` RFC 8785 encoder and applies
the AC-G-53 restrictions. `ensure_effective_semantic_request_id` validates a supplied
value or injects a newly generated opaque value into the normalized JSON object
**before** canonical serialization, request hashing, and RPC submission. It returns the
exact value copied into the RPC control field.

### 58.1 Inline response decoding

```python
canonical_response = JSON_OBJECT_ADAPTER.validate_json(result.response_json)
```

The daemon's canonical response validation remains authoritative.

### 58.2 Public mapping

All fields, including `PublicSnapshotMetadata`, are mapped explicitly. Additive daemon fields SHALL not cross through `**dto.model_dump()` or unrestricted polymorphic serialization.

## 59. Result resources, status, and references

Result resource handlers forward result identity to the daemon and return bytes. Manifest and `meta` fields are strict public models.

Static specifications and schema resources SHALL be loaded from package resources:

```python
from importlib.resources import files


def package_text(relative: str) -> str:
    return (
        files("codefabric_cpg_mcp.resources")
        .joinpath(relative)
        .read_text("utf-8")
    )
```

No resource handler accepts arbitrary filesystem paths.

Dynamic status/reference tool outputs SHALL be validated into their public models before serialization.

---

## 60. Schema generation and STDIO launch

### 60.1 Schema export

```python
import json
from pathlib import Path


def export_schema(path: Path, schema: dict[str, object]) -> None:
    path.write_text(
        json.dumps(schema, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )

export_schema(
    Path("query-tool-output-1.3.schema.json"),
    QUERY_TOOL_OUTPUT_SERIALIZATION_SCHEMA,
)
export_schema(
    Path("query-tool-output-1.3.validation.schema.json"),
    QUERY_TOOL_OUTPUT_VALIDATION_SCHEMA,
)
```

The exported constants SHALL contain `$schema: https://json-schema.org/draft/2020-12/schema` and a stable `$id`. CI SHALL compare both validation and serialization schemas, while FastMCP publishes the serialization-mode schema for outputs. Pretty source bytes are review artifacts; schema fingerprints are BLAKE3-256 over RFC 8785 canonical schema JSON and never over the pretty representation.

### 60.2 Host launch configuration

```json
{
  "mcpServers": {
    "codefabric-cpg": {
      "command": "uv",
      "args": [
        "run",
        "--frozen",
        "--project",
        "/absolute/path/codefabric-cpg-mcp",
        "python",
        "-m",
        "codefabric_cpg_mcp"
      ],
      "env": {
        "CODEFABRIC_CPG_DAEMON_TARGET": "unix:///run/user/1000/codefabric/cpgd.sock",
        "CODEFABRIC_WORKSPACE_ID": "workspace-main",
        "CODEFABRIC_AGENT_INSTANCE_ID": "cursor-primary",
        "CODEFABRIC_CPG_CAPABILITY_TOKEN": "injected-secret"
      }
    }
  }
}
```

The capability token SHOULD be injected by a launcher or credential store rather than committed to a project file.

---
# Part IX — Security and Isolation

## 61. Security model

The STDIO security boundary is:

```text
MCP host process ownership
+ adapter subprocess ownership
+ protected daemon socket
+ capability-token handshake
+ workspace authorization
+ agent-scoped result ownership
+ daemon read-only RPC allowlist
+ Pydantic schema-closed public serialization
```

### 61.1 Socket permissions

Recommended:

```text
parent directory mode 0700
socket mode           0600
owner                 current user or daemon service user
```

Loopback TCP fallback requires capability authentication and SHOULD use stronger channel protection if crossing a user/container boundary.

### 61.2 Read-only capability

The adapter capability authorizes only:

```text
handshake
status
validate query
execute fact query
cancel own query
read/release own result
```

It does not authorize publication, extraction, source mutation, maintenance, SQL, or generic table access.

### 61.3 Workspace and result isolation

The daemon binds the adapter to an authorized workspace. Every result read checks workspace and agent ownership; unguessable IDs are not sufficient alone.

### 61.4 Pydantic output firewall

Every model-visible adapter object SHALL be produced from an explicit public model. Security-critical rules:

- `extra="forbid"` on public models;
- default annotation-driven serialization;
- no `SerializeAsAny` or global duck typing;
- no generic serialization fallback such as `repr`;
- no open `dict[str, Any]` for public metadata;
- no `model_construct()` for untrusted or daemon-derived public values;
- no experimental partial validation before returning a terminal result;
- no validators or serializers that perform I/O;
- no dynamic models generated from agent input.

### 61.5 Source confidentiality

- never log source text or canonical responses;
- cap and audit source-context retrieval;
- prefer repository-relative paths;
- retain artifacts only for configured TTL;
- keep spill/result directories private;
- omit raw Pydantic input values from client errors.

---

## 62. Threat and misuse controls

| Threat | Control |
|---|---|
| enormous query DAG | byte/block/node/depth limits |
| cyclic path amplification | daemon boundedness checks |
| JSON nesting or container explosion | adapter complexity guard before semantic execution |
| non-finite JSON numbers | strict `JsonValue` TypeAdapter + `allow_inf_nan=False` |
| cross-agent result access | ownership checks + high-entropy IDs |
| arbitrary filesystem access | package resources and daemon result IDs only |
| arbitrary SQL | no SQL input/tool/RPC |
| stale cached truth | no adapter query cache |
| source leakage through errors | safe Pydantic issue translation; omit input/context |
| subclass-field leakage | schema-closed public serialization |
| daemon metadata leakage | explicit `PublicToolMeta` allowlist |
| dynamic-schema CPU/memory attack | no agent-driven model generation |
| protocol corruption | STDOUT reserved for MCP |
| dependency drift | exact pins + schema fingerprints + golden tests |
| result retained forever | TTL + release + quota cleanup |

Pydantic is a parser/validator, not a sandbox. Process and daemon resource limits remain mandatory.

---
# Part X — Observability and Operations

## 63. Trace topology

```text
MCP tool span
  ├─ FastMCP declaration validation
  ├─ adapter JSON structural validation
  ├─ canonical request serialization
  ├─ daemon RPC
  │    ├─ admission / queue
  │    ├─ snapshot pin
  │    ├─ semantic resolution
  │    ├─ logical / physical planning
  │    ├─ execution
  │    └─ response materialization
  ├─ public Pydantic model construction
  └─ MCP serialization/delivery
       ├─ inline JSON decode
       └─ resource manifest creation
```

W3C trace context SHOULD be forwarded to the daemon.

---

## 64. Adapter metrics

```text
adapter_startup_ms
settings_validation_ms
pydantic_schema_build_ms
schema_fingerprint_check_ms
handshake_ms
component_list_ms
tool_calls_total by tool/status
request_json_validation_ms
request_json_validation_failures_total by code
request_json_depth
request_json_nodes
request_bytes
daemon_rpc_ms
public_output_validation_ms
public_output_validation_failures_total by model
public_output_serialization_ms
result_bytes
inline_results_total
externalized_results_total
resource_reads_total
cancellations_total
timeouts_total
contract_mismatch_total
safe_tool_errors_total
unexpected_errors_total
```

Do not label metrics with semantic request text, source paths, or validation input values.

---

## 65. Daemon metrics surfaced through traces/status

The adapter may expose safe summarized metadata:

```text
queue_ms
semantic_resolution_ms
planning_ms
execution_ms
materialization_ms
spill_bytes
peak_memory_bytes
query-block and result counts
unknown counts
publication ID
result checksum
```

These are execution metadata, not CPG facts.

---

## 66. Logging rules

### Log

```text
trace ID
approved agent/workspace identifiers
component and contract versions
request hash and byte/node/block counts
publication ID
status/error code
timing and byte counts
externalization decision
cancellation/timeout
Pydantic error type and safe path only
```

### Do not log by default

```text
capability token
source text
raw request or response JSON
ValidationError input/context/cause
absolute local paths
provider evidence payloads
physical plan text
unfiltered daemon mappings
arbitrary exception repr returned to client
```

---

## 67. Readiness and liveness

### Adapter readiness

Ready only after:

- settings validation;
- Pydantic public schema build and fingerprint check;
- daemon channel connection;
- handshake validation;
- semantic contract fingerprint verification;
- an active, internally consistent `ServingSnapshot` summary for the bound `workspace_id` (a durable base publication alone is insufficient).

### Adapter liveness

The process and event loop are responsive. Liveness does not require a full CPG query.

### Status tool

`get_code_graph_status` is the model-visible readiness/capability check and may report degraded fact families without declaring the whole service unavailable.

---
# Part XI — Testing and Verification

## 68. Test layers

### 68.1 Pydantic settings tests

Required cases:

```text
required variable missing
valid environment conversion
invalid numeric range
invalid daemon scheme
canonical and migration aliases
constructor override precedence
production dotenv omission
file-secret source where configured
SecretStr repr/dump redaction
frozen mutation rejection
safe startup error rendering
```

### 68.2 Public contract tests

```text
unknown public field rejected
strict scalar coercion rejected
non-finite numbers rejected
invalid identifier/checksum/resource URI rejected
inline variant requires response
resource variant requires manifest
wrong discriminator rejected
negative counts/timings rejected
public ToolResult.meta allowlist only
subclass-only field leakage regression
model_dump(mode="json") exact shape
validation and serialization JSON Schema snapshots
round-trip revalidation where intended
```

### 68.3 JSON adapter tests

```text
ordinary JSON object accepted
bytes/datetime/custom object rejected
non-string mapping key rejected
NaN/infinity rejected
max depth rejected
max node count rejected
adapter instance reused rather than rebuilt
```

### 68.4 In-memory FastMCP tests

Use `Client(mcp)` to verify:

- four-tool catalog;
- tool input and generated output schemas;
- resources/templates/prompts;
- annotations and versions;
- structured content;
- safe intentional errors;
- middleware order;
- lifespan settings/cleanup.

### 68.5 Fake-daemon contract tests

Cover:

- handshake success/failure;
- additive unknown daemon field ignored but not exposed;
- inline and resource terminal variants;
- impossible terminal variant rejected;
- progress stream;
- partial query-block failure;
- cancellation/deadline;
- connection loss before/after acceptance;
- idempotent resume;
- result expiry/forbidden;
- raw daemon metadata leakage regression.

### 68.6 Real STDIO integration tests

Spawn the locked command and verify:

- no stray STDOUT bytes;
- environment injection;
- startup/teardown;
- concurrent calls;
- host cancellation;
- stderr logging;
- exact runtime dependency versions.

### 68.7 Daemon integration tests

Use fixture data fabric and test all eight query forms, composition, snapshot changes, and large-result delivery.

---

## 69. Semantic query conformance suite

Required fixtures remain:

```text
minimal entity query
all eight request forms
independent/dependent branches
fan-in/fan-out
cycle rejection
entity and phrase ambiguity
exact/possible/heuristic/unknown targets
direct vs transitive facts
negative claim with complete/incomplete coverage
unavailable fact family
unbounded cyclic path
explicit result limit and hard rejection
query-block failure isolation
source-context retrieval
Rust MIR/ownership facts
Python type/dispatch facts
one-snapshot consistency while publication advances
```

The Pydantic adapter must preserve, not reinterpret, every canonical distinction.

---

## 70. Contract fingerprinting

CI SHALL compare four independent schema families:

```text
FastMCP component manifest
Pydantic adapter validation schemas
Pydantic adapter serialization schemas
canonical semantic request/response schemas
```

Recommended commands:

```bash
fastmcp inspect src/codefabric_cpg_mcp/server.py --format mcp -o current-mcp.json
python -m codefabric_cpg_mcp.schema_export --check
```

Store fingerprints beside:

```text
FastMCP version
Pydantic version
pydantic-settings version
Python version
daemon RPC version
semantic query specification version
semantic schema hashes
adapter public schema hashes
server instructions hash
```

The protocol-facing FastMCP fingerprint profile is
`codefabric-fastmcp-tool-manifest-v1`. For each explicit tool it includes exactly these
serialized MCP keys when present:

```text
name
title
description
inputSchema
outputSchema
icons
annotations
_meta
execution
```

The value is obtained from
`tool.to_mcp_tool().model_dump(mode="json", by_alias=True, exclude_none=True)`. An
unexpected top-level key fails generation rather than being silently excluded.
Process-local IDs, callable identity, middleware, runtime state, and framework
bookkeeping never enter the MCP value. Tools are sorted by public name, the resulting
JSON value is encoded with `codefabric-jcs-v1`, and BLAKE3-256 produces the fingerprint.
Changing the inclusion list or profile ID is a public contract-version event.

Pydantic validation-mode and serialization-mode schemas are separately generated and
fingerprinted. FastMCP output publication SHALL equal the generated serialization-mode
view for the same public contract; equality is structural after strict JSON parsing, not
an incidental comparison of pretty bytes.

An unchanged public contract version with changed serialization schema SHALL fail CI.

---

## 71. Performance tests

Measure separately:

```text
Python import/startup
Pydantic model/schema build
settings validation
FastMCP construction
handshake
steady-state JSON TypeAdapter validation
canonical request serialization
small-query adapter overhead
gRPC stream overhead
public envelope validation/serialization
inline JSON decode
resource externalization/read throughput
concurrent calls and multiple adapters
cancellation latency
large-result memory use
invalid-input/error-heavy paths
```

Success criterion:

> Adapter overhead remains a small fraction of daemon execution time and does not scale with graph size except for final canonical JSON decode/delivery.

The adapter SHALL not instantiate Pydantic models per fact record.

---

## 72. Failure injection

Test:

- invalid settings at startup;
- Pydantic schema snapshot mismatch;
- daemon unavailable or restarting;
- corrupt inline JSON;
- canonical response schema mismatch;
- impossible daemon delivery combination;
- public output validation failure;
- incorrect checksum;
- result artifact disappearance;
- publication change during query;
- disk full during externalization;
- cancellation during traversal/materialization;
- agent killed without release;
- capability token revoked;
- contract major mismatch;
- accidental new daemon metadata field;
- accidental public subclass-only field.

---
# Part XII — Deployment and Lifecycle

## 73. Process ownership

```text
user/system service manager
    owns central native cpgd daemon

MCP host
    owns one codefabric-cpg-mcp subprocess per agent/session
```

The adapter SHALL not launch a second daemon automatically in production.

---

## 74. Startup sequence

```text
1. daemon starts and opens current publication
2. daemon creates protected UDS
3. MCP host starts adapter in locked environment
4. adapter imports and builds Pydantic contract schemas
5. adapter validates schema fingerprints
6. lifespan validates immutable Settings
7. adapter connects and handshakes
8. adapter validates handshake application DTO
9. FastMCP begins STDIO protocol
10. agent lists components and receives instructions
```

Failures before step 9 exit non-zero with safe diagnostics on STDERR.

---

## 75. Shutdown sequence

```text
1. host closes/cancels STDIO
2. in-flight tool coroutines receive cancellation
3. daemon query cancellation/release is sent best-effort
4. FastMCP lifespan shuts down
5. gRPC channel closes
6. adapter exits
7. central daemon remains for other agents
```

No settings reload or dynamic model rebuild occurs during shutdown.

---

## 76. Multiple agents

The daemon SHOULD implement global and per-agent admission, workspace memory/spill limits, fair scheduling, artifact quotas, and cancellation isolation.

Every adapter has its own immutable settings object, daemon channel, public schema constants, and agent identity. Adapters do not share Python mutable state.

---

## 77. Dependency upgrade policy

### 77.1 FastMCP

For each FastMCP upgrade:

1. read stable release/security notes;
2. update exact pin and lockfile in isolation;
3. inspect generated MCP manifest;
4. run in-memory and STDIO suites;
5. verify progress/cancellation/resources/error masking;
6. advance product/tool version if visible contract changes.

### 77.2 Pydantic

For each Pydantic upgrade:

1. update exact pin without independently forcing `pydantic-core`;
2. rebuild validation and serialization schemas;
3. compare schema fingerprints and semantic assertions;
4. run strictness, union, alias, secret, subclass-leakage, and error tests;
5. compare model dumps and safe validation diagnostics;
6. benchmark startup, validation, serialization, and invalid-input paths;
7. advance adapter/tool contract version for visible changes.

### 77.3 `pydantic-settings`

Upgrade separately from core Pydantic. Re-test source order, environment aliases, secret files, defaults, and startup failures. A settings-source change is an operational contract change even when MCP schemas are unchanged.

Prerelease dependency lines SHALL be evaluated only in isolated branches.

---

## 78. Semantic query and daemon upgrade policy

Version independently:

```text
MCP server product             1.3.0
query_code_graph tool          1.3
adapter public contract        1.3
semantic query specification   1.3
Pydantic                       2.13.4
pydantic-settings              2.15.0
daemon RPC                     1.x
ontology                       explicit version
schema bundle                  explicit version
derivation bundle              explicit version
active ServingSnapshot          snapshot ID plus durable-base and overlay identity
```

Pydantic integration does not alter the semantic query specification. Breaking semantic-query changes require a new semantic major version and normally a new tool version.

---
# Part XIII — Implementation Phases

## 79. Phase 1 — Contract-minimal adapter

Deliver:

- exact dependency pins and lockfile;
- strict public Pydantic model family;
- immutable Pydantic Settings;
- reusable JSON TypeAdapter;
- generated validation/serialization schema snapshots;
- STDIO server and daemon handshake;
- four public tools;
- static schema/spec resources;
- inline response path;
- safe Pydantic error translation and stderr logs;
- in-memory/fake-daemon tests;
- MCP and Pydantic contract fingerprints.

---

## 80. Phase 2 — Progress, cancellation, and observability

Deliver:

- server-streamed daemon events;
- `Context.report_progress` mapping;
- deadline/cancellation propagation;
- trace context;
- Pydantic validation/serialization metrics;
- structured logging;
- failure injection.

---

## 81. Phase 3 — Large result resources

Deliver:

- daemon artifact store;
- automatic delivery threshold;
- discriminated resource delivery model;
- root/manifest/query/dictionary/chunk resources;
- typed public resource metadata;
- TTL/release/authorization;
- checksum and schema validation.

---

## 82. Phase 4 — Agent guidance

Deliver:

- concise instructions;
- native resources;
- tool-only reference tool;
- query-authoring and interpretation prompts;
- recipes for Python, Rust, calls, state, CFG, dataflow, alias, ownership, effects, and source context.

---

## 83. Phase 5 — Production hardening

Deliver:

- service-manager integration;
- socket/token rotation;
- per-agent quotas and fair scheduling;
- real-host compatibility matrix;
- load tests with multiple adapter processes;
- dependency upgrade/rollback runbooks;
- public metadata leakage review;
- operational dashboards and alerts;
- independent security and contract review.

---

# CodeFabric 1.3 architecture-completion contracts

The standalone architecture-completion specification has been propagated into its permanent owners. This part contains the full normative contracts owned by this document: `G-58`, `G-59`, `G-60`, `G-61`, `G-63`, `G-64`, `G-65`, `G-66`, `G-67`, `G-68`, `G-69`. References to a gap ID elsewhere in the synchronized suite resolve to these sections.

## AC-G-58 — Complete Protobuf service and query state machine
### Decision

Query acceptance is a unary RPC that returns the daemon handle before freshness waiting or execution. Event streaming is a separate resumable RPC. The full semantic request and canonical response remain canonical JSON bytes carried by typed Protobuf control messages.

### Contract

The required service surface is:

```proto
service CpgQueryService {
  rpc Handshake(HandshakeRequest) returns (HandshakeResponse);
  rpc GetStatus(StatusRequest) returns (StatusResponse);
  rpc ValidateQuery(ValidateQueryRequest) returns (ValidateQueryResponse);
  rpc StartQuery(StartQueryRequest) returns (StartQueryResponse);
  rpc StreamQuery(StreamQueryRequest) returns (stream QueryEvent);
  rpc AttachQuery(AttachQueryRequest) returns (stream QueryEvent);
  rpc CancelQuery(CancelQueryRequest) returns (CancelQueryResponse);
  rpc ReadResult(ReadResultRequest) returns (stream ResultChunk);
  rpc ReleaseResult(ReleaseResultRequest) returns (ReleaseResultResponse);
}
```

`HandshakeRequest` contains adapter instance/version, supported RPC/query/schema ranges, feature bits, compression algorithms, desired workspace IDs, host result/resource capabilities, and capability credential proof. Response contains daemon instance/version, negotiated versions/features, installed bundle IDs/digests, effective limits profile, authorized workspaces/claims, server time, and readiness summary.

`StartQueryRequest` contains:

```text
agent_instance_id
workspace_id
mcp_call_id
rpc_attempt_id
semantic_request_id optional
semantic_query_version
canonical_request_json bytes
request_checksum
requested freshness policy
adapter delivery preference
host capability profile digest
deadline timestamp
idempotency key
```

`StartQueryResponse` contains:

```text
daemon_query_id
resume_token
accepted_at
query_execution_state = ACCEPTED
queue_class and optional queue position
negotiated request/response versions
effective semantic_request_id
```

`StreamQuery` and `AttachQuery` receive the query ID, resume token, and `after_sequence`. `QueryEvent` is a closed `oneof` of:

```text
SnapshotPinnedEvent
ProgressEvent
ResponseChunkEvent
ArtifactReadyEvent
TerminalEvent
```

All events carry query ID, sequence, snapshot ID when known, event timestamp, and event checksum. `TerminalEvent` occurs exactly once and includes execution/availability/freshness/limit/dependency states, canonical response checksum or error record, artifact reference where present, counts, and cleanup state.

`ResultChunk` includes artifact ID, offset, uncompressed length, payload, payload checksum, artifact checksum, content type, encoding, final-chunk flag, and lease expiry.

Protocol rules:

- Protobuf package major is `codefabric.cpgd.v1`;
- unknown additive fields are ignored according to Protobuf semantics, but unknown required feature bits fail handshake;
- `oneof` messages with no value or multiple wire values fail validation;
- sequence numbers are unsigned 64-bit and never reused within a query;
- default maximum uncompressed control message is 4 MiB; result/event payload chunks default to 1 MiB;
- negotiated payload compression is `identity` or `zstd`; checksums cover uncompressed canonical payload bytes;
- gRPC transport status reports transport/service failure; domain query failure is normally a terminal event with the canonical error record;
- malformed request checksums, IDs, versions, or credentials fail before query creation;
- the server persists enough accepted-query metadata to recover idempotency and orphan handling across adapter reconnects, but not across daemon restart unless an artifact already exists.

Before canonical hashing, the adapter establishes `effective_semantic_request_id`: preserve a supplied valid canonical-JSON value, otherwise generate an opaque ID and inject it into the normalized request. The RPC field and normalized JSON field SHALL match. The normalized pre-pin idempotency key is:

```text
workspace_id
+ agent_instance_id
+ effective_semantic_request_id
+ canonical normalized request checksum
+ structured freshness policy
```

The accepted record additionally pins `snapshot_id` once selected and never migrates to another snapshot. Reuse rules are: same key and same content returns the existing active handle or terminal result/artifact; same semantic request ID with different request/freshness returns `IDEMPOTENCY_CONFLICT`; `mcp_call_id` and `rpc_attempt_id` are correlation only. Active-record reconnect retention is the query orphan/replay window; terminal inline records are retained for one hour, and terminal artifact mappings for the artifact TTL. A generated semantic request ID provides idempotency only within retries of that MCP operation; cross-invocation idempotency requires a caller-supplied stable ID.

`ValidateQuery` performs schema/language/type/cost/authorization preflight against a selected current metadata view but does not promise the later execution snapshot. Its response clearly marks snapshot-dependent checks as provisional.
## AC-G-59 — Cancellation, acknowledgement, reconnect, and orphan handling
### Decision

Transport cancellation and semantic cancellation are distinct. The adapter SHALL issue explicit cancellation whenever its MCP call is cancelled, while stream disconnect alone starts a bounded reconnect grace.

### Contract

Cancellation states:

```text
NOT_FOUND
CANCELLATION_REQUESTED
CANCELLED
ALREADY_TERMINAL
FORCE_TERMINATED
```

`CancelQuery` is idempotent and authorized by query-bound resume/cancel token plus agent/workspace claims. Response includes the accepted state, acknowledgement time, terminal state if known, providers/operators still cleaning up, and whether forced termination was required.

Propagation:

1. daemon query cancellation token fires immediately;
2. freshness waits, resolver, DataFusion operators, artifact writers, and provider jobs observe the token;
3. queued provider work is removed;
4. in-process work acknowledges within 2 seconds or is marked non-cooperative;
5. sidecar/compiler process groups receive graceful termination, then forced kill after at most 10 seconds;
6. staged unactivated output is discarded;
7. a terminal `CANCELLED` event is recorded if the stream remains attached or is replayed later.

The adapter cancellation handler has the `daemon_query_id` from `StartQueryResponse`; it never waits for terminal execution to learn the handle.

Reconnect behavior:

- disconnecting `StreamQuery` does not immediately cancel the query;
- the daemon holds the replay buffer and query for 30 seconds;
- `AttachQuery` must present the resume token and last sequence checksum/number;
- successful attach resets the orphan grace;
- invalid token, agent mismatch, workspace mismatch, or replay request before an inconsistent sequence is rejected;
- after grace expiry, nonterminal orphaned queries are cancelled unless a completed artifact or explicit detached-execution flag exists;
- detached execution is disabled in the default MCP profile.

If the daemon restarts before terminal materialization, the query cannot resume and returns `QUERY_LOST_DAEMON_RESTART`; the adapter may retry idempotently under the same request key against a new snapshot according to its freshness policy.
## AC-G-60 — Capability credential issuance, binding, rotation, and revocation
### Decision

Adapter credentials are opaque random capabilities minted by the daemon launcher/admin service. They are not JWTs, user passwords, API keys stored in config, or command-line arguments.

### Contract

A credential record contains:

```text
credential_id
BLAKE3-256 token hash
agent_instance_id
allowed workspace IDs
permission claims
source ACL profile IDs
issued_at
not_before
expires_at
rotation_parent optional
first-bound process identity optional
revoked_at/reason optional
last_used_at
```

Token material is 32 cryptographically random bytes encoded base64url without padding. Only the hash is persisted.

Initial delivery SHALL use an inherited anonymous pipe/file descriptor opened by the trusted launcher. The token SHALL NOT appear in process arguments, ordinary environment variables, logs, status, crash reports, or files. The adapter reads it once, keeps it in locked/zeroizable memory where supported, and closes the descriptor.

Default lifetime is 8 hours and never exceeds 24 hours. The daemon issues a connection/session capability at handshake with one-hour lifetime; it rotates at half-life and is bound to:

```text
credential ID
agent instance
workspace claims
OS peer user
adapter process start token
negotiated daemon instance
connection nonce
```

The initial capability may reconnect during its lifetime but cannot be used concurrently by multiple bound process identities. Replay from a different peer/process returns `CREDENTIAL_REPLAY_DETECTED` and revokes the token.

Permissions are explicit:

```text
READ_FACTS
READ_SOURCE
READ_GENERATED_SOURCE
READ_EXTERNAL_SOURCE
READ_STATUS_BASIC
READ_STATUS_DIAGNOSTIC
CREATE_RESULT_ARTIFACT
```

There is no query-service mutation permission.

Revocation is immediate in SQLite and checked on every new RPC and long-running heartbeat. Active queries are cancelled when their credential is revoked. Rotation creates a new token and grace-revokes the predecessor after 60 seconds.

Audit records store credential ID, agent/workspace, action, result, and correlation IDs but never token material or source content.
## AC-G-61 — Local IPC platform and security profile
### Decision

Conforming 1.x deployment uses private Unix-domain sockets on Linux/macOS with OS peer-credential verification. Loopback TCP is an explicit fallback requiring mTLS plus capability credentials. Windows named pipes are not supported in 1.x.

### Contract

Socket paths:

```text
Linux: $XDG_RUNTIME_DIR/codefabric/<daemon-instance-short-id>/cpgd.sock
macOS: $TMPDIR/cf-<uid>/<daemon-instance-short-id>.sock
```

Parent directories are mode `0700`; socket mode is `0600`; owner is the daemon user. The daemon verifies directory and socket ownership, link count, and non-symlink status at creation and connection.

Peer authentication:

- Linux uses `SO_PEERCRED` and requires matching UID unless the deployment profile names an allowed service UID;
- macOS uses `getpeereid` with the same rule;
- peer credentials supplement but do not replace capability tokens;
- inherited descriptors are close-on-exec except those explicitly passed.

Singleton/stale handling:

1. daemon holds an exclusive lock file containing instance ID, PID, process start token, and socket inode;
2. startup probes an existing socket and validates the lock owner;
3. stale socket removal is allowed only when the lock is not held and the recorded process identity is dead;
4. symlinked or foreign-owned socket/lock paths cause a security failure, not cleanup.

Container use requires explicitly mounting only the socket and launcher token pipe; mounting the entire daemon state/source root into an adapter container is prohibited.

Loopback TCP fallback:

- disabled by default;
- binds only `127.0.0.1` and/or `::1`;
- requires TLS 1.3 with per-daemon ephemeral CA and mutual certificates plus the capability token;
- publishes no service discovery outside the private runtime file;
- rejects plaintext even on loopback.

Windows returns `PLATFORM_UNSUPPORTED` in 1.x; no implementation may silently downgrade to unauthenticated TCP.
## AC-G-63 — Immutable result artifact store
### Decision

Large results are stored in a daemon-owned local content-addressed artifact store with SQLite metadata, strict agent/workspace ownership, and lease-aware garbage collection. It is not Delta and not a generic filesystem server.

### Contract

Artifact URI:

```text
codefabric-result://<workspace-hex>/<artifact-hex>
```

Optional read-only subresource selectors are URI fragments or structured resource parameters, never path traversal:

```text
#query=<query-id>
#entity=<public-entity-id>
#fact=<public-fact-id>
#source-context=<context-id>
```

Artifact ID:

```text
BLAKE3_128(CBEF-v1(
  domain = RESULT_ARTIFACT,
  workspace_id,
  owning_agent_instance_id,
  snapshot_id,
  canonical logical response checksum,
  artifact format/version
))
```

Storage layout is daemon-private and content-addressed; filenames never contain workspace paths, query labels, or source identifiers. Files are immutable, mode `0600`, under mode-`0700` directories. The default local profile relies on OS user-private filesystem protection and does not add application-level encryption. A deployment requiring encryption SHALL declare an alternate artifact-storage profile; it may not claim the default profile provides encryption at rest.

Metadata includes owner agent/workspace, snapshot/base publication, byte length, content type/encoding, canonical checksum, created/expiry time, source-containing flag, lease IDs, read/release state, and subresource index digest.

Creation is idempotent by artifact ID. If an object already exists, its full checksum, owner, snapshot, format, and length SHALL match before the existing bytes are reused; mismatch is `ARTIFACT_ID_COLLISION` and blocks completion. Reuse may extend TTL only within the credential and source-containing policy. After expiry/GC, the same deterministic ID may be recreated from identical canonical bytes with a new metadata generation.

Defaults:

```text
ordinary TTL:             1 hour
artifact containing source bytes: 30 minutes
per-agent quota:          2 GiB
workspace quota:          8 GiB
global quota:             10 GiB
single artifact soft max: 256 MiB
single artifact hard max: 1 GiB
```

Quota exhaustion rejects artifact creation before releasing a query as complete, unless inline delivery succeeds. GC removes only expired/released artifacts with no active read/snapshot lease.

Artifacts survive daemon restart until TTL. Cross-agent reads are prohibited even if IDs are known. A new credential for the same logical agent does not inherit access unless the launcher explicitly preserves the same `agent_instance_id` claim.
## AC-G-64 — Delivery precedence, host limits, and automatic externalization
### Decision

Canonical semantic JSON contains no transport-delivery directive. The outer FastMCP tool envelope owns delivery preference and host capability adaptation.

### Contract

Outer preference:

```text
AUTO
INLINE
RESOURCE
```

Negotiated host profile contains:

```text
supports_mcp_resources
maximum_tool_result_bytes
supports_range_resource_reads
accepted_content_types
```

Decision algorithm:

1. daemon produces one canonical logical response/checksum independent of delivery;
2. effective inline hard maximum is the minimum of the deployment-profile hard maximum (4 MiB by default) and the negotiated host maximum;
3. effective `AUTO` threshold is the minimum of 512 KiB, the configured automatic threshold, and the effective inline hard maximum;
4. `AUTO`: inline when response bytes are within the effective automatic threshold; otherwise resource;
5. `RESOURCE`: always materialize an artifact;
6. `INLINE`: inline if within the effective inline hard maximum; if too large and resources are supported, override to resource and set `delivery_override_reason`; if resources are unsupported, fail `RESULT_TOO_LARGE_FOR_HOST`;
7. any response containing more source bytes than the configured inline source threshold, default 128 KiB, is externalized in `AUTO` mode;
8. semantic content, ordering, checksum, and completeness are identical in both modes.

The adapter may stream daemon chunks directly into an artifact without materializing the whole response in Python. It SHALL not rebuild or reinterpret canonical fact dictionaries.

Public tool output always includes delivery mode, canonical checksum, byte length, snapshot metadata, and either inline response or immutable resource reference—not both full payloads.
## AC-G-65 — Stable error registry and layer mappings
### Decision

All public/domain errors are generated from one append-only registry. Provider-native exceptions, SQL/DataFusion messages, filesystem errors, and validation traces are diagnostic evidence, not public error codes.

### Contract

Registry domains and numeric ranges:

```text
1000–1999  request/schema/controlled-language/entity/reference errors
2000–2999  authorization/workspace/path/source-disclosure errors
3000–3999  freshness/capability/completeness/context errors
4000–4999  provider/configuration/sandbox/generation errors
5000–5999  planning/execution/limit/cancellation errors
6000–6999  publication/snapshot/storage/operational-state errors
7000–7999  RPC/transport/credential/compatibility errors
8000–8999  artifact/resource/delivery errors
9000–9999  internal invariant/security failures
```

Each record contains:

```text
numeric code
stable name
owning layer
severity: INFO | WARNING | ERROR | FATAL
retryability: NEVER | SAME_SNAPSHOT | NEW_SNAPSHOT | AFTER_RECONFIGURATION | TRANSIENT
scope: REQUEST | QUERY_BLOCK | PROVIDER_RUN | WORKSPACE | DAEMON
safe public message template
allowed public detail fields
required diagnostic linkage
gRPC status mapping
MCP/tool error mapping
introduced/deprecated/replacement versions
```

Required named errors include all codes referenced in the synchronized 1.3 suite, including:

```text
INCOMPATIBLE_MAJOR, UNSUPPORTED_MINOR, BUNDLE_DIGEST_MISMATCH,
WORKSPACE_NOT_AUTHORIZED, PATH_OUTSIDE_AUTHORIZED_ROOT, SOURCE_ACCESS_DENIED,
BLOCKED_PATH_COLLISION,
FRESHNESS_DEADLINE_EXCEEDED, CAPABILITY_UNAVAILABLE, NEGATIVE_PROOF_INDETERMINATE,
SOURCE_SNAPSHOT_MISMATCH, PROVIDER_PROTOCOL_ERROR, SANDBOX_UNAVAILABLE,
QUERY_HARD_LIMIT_EXCEEDED, ENTITY_AMBIGUOUS, SEMANTIC_PHRASE_AMBIGUOUS,
CURRENT_POINTER_CONFLICT, ID_COLLISION, OVERLAY_GENERATION_CONFLICT,
CREDENTIAL_REPLAY_DETECTED, IDEMPOTENCY_CONFLICT, RESUME_WINDOW_EXPIRED,
RESULT_TOO_LARGE_FOR_HOST, ARTIFACT_ID_COLLISION, RESOURCE_EXPIRED, STATE_TRANSITION_VIOLATION,
INTERNAL_INVARIANT_VIOLATION
```

Public error instances use the synchronized envelope and may expose only registry-approved fields. Raw paths, source text, tokens, credentials, provider stderr, SQL plans, and host environment values are redacted unless an authorized diagnostic resource explicitly provides them.
## AC-G-66 — Public status contract and redaction levels
### Decision

Status is a stable operational contract with `BASIC` and `DIAGNOSTIC` authorization levels. It exposes enough freshness/capability information for agents without becoming a host, Git, or provider-internals inventory.

### Contract

`BASIC` status fields:

```text
daemon instance and public version range
workspace ID and authorized display name
workspace lifecycle/readiness state
active PublicSnapshotMetadata
source trust, event-stream health, Git acceleration summary
selected/default context IDs and public labels
capability aggregate by family/state
current barrier/reconciled event sequences
query/update queue counts and effective limits profile ID
last stable public diagnostic codes/timestamps
```

`DIAGNOSTIC` may additionally expose:

```text
provider run summaries and durations
update-wave state/counts
publication/overlay sizes and generations
sandbox/profile identifiers
bundle full digests
non-sensitive storage health/lease counts
```

Status SHALL NOT expose by default:

- raw workspace/common Git paths;
- Git remote URLs, branch names, commit messages, credentials, user config, or reflogs;
- source bytes or identifiers from unauthorized paths;
- environment variables, command lines containing secrets, home paths, socket tokens;
- raw provider stderr or compiler command lines;
- arbitrary SQLite/Delta rows.

Path displays are exposed only when the credential's source/path policy permits them. Diagnostics use IDs and safe messages otherwise.

Query responses include only public snapshot metadata, per-query coverage/status, and relevant diagnostic IDs; they do not embed daemon queue/provider internals.

The public status JSON Schema is closed and independently versioned.
## AC-G-67 — MCP resource read, range, expiry, and release semantics
### Decision

Result resources support complete and ranged immutable reads. A read acquires a short lease so an artifact cannot expire mid-stream.

### Contract

Resource read parameters:

```text
resource_uri
offset: non-negative u64, default 0
length: optional positive u64
expected_artifact_checksum: optional
expected_chunk_size: optional, max 1 MiB
```

Response metadata:

```text
content_type
encoding
artifact_byte_length
requested/delivered offset and length
artifact checksum / ETag
chunk checksum
final range flag
artifact expiry
resource-read lease ID
```

Rules:

- full reads are range reads from offset zero to EOF;
- maximum chunk payload is 1 MiB;
- offset beyond EOF returns a valid empty final range only when exactly equal to length; greater values are invalid;
- expected checksum mismatch fails before bytes are returned;
- every read reauthorizes agent/workspace ownership, current credential state, artifact permission, and current source-disclosure policy for source-bearing bytes/subresources; revocation or ACL narrowing denies new bytes even when the opaque artifact ID is known;
- beginning a read acquires/extends a 5-minute resource-read lease;
- expiry before read start yields `RESOURCE_EXPIRED`; expiry during an active leased read does not interrupt it;
- release is idempotent and prevents new reads after active read leases finish;
- active reads may finish after release but cannot extend TTL beyond their fixed grace;
- maximum concurrent resource reads per agent is four;
- content encoding/compression is explicit and checksums refer to uncompressed canonical bytes unless a separate compressed-byte checksum field is present;
- URI parsing is strict and never maps arbitrary URI path segments to filesystem paths.
## AC-G-68 — Multi-agent fairness, reservations, and starvation guarantees
### Decision

Scheduling uses weighted fair queues with reserved headroom for source updates and interactive freshness. No agent may monopolize query or artifact resources.

### Contract

Priority order:

```text
P0 security/recovery and source-authority reconciliation
P1 targeted fast updates required by waiting strict-current queries
P2 ordinary fast source/syntax updates
P3 interactive queries
P4 semantic/derived update work
P5 durable flush and artifact creation
P6 maintenance/compaction/vacuum
```

P0/P1 work may preempt queued lower priorities but does not interrupt already executing safe critical sections.

Default concurrency on `N` logical CPUs:

```text
Tokio worker threads:                 min(8, max(2, N / 4))
Rayon CPU workers across daemon:      max(2, N - 2)
reserved update permits:              max(1, ceil(0.30 × Rayon workers))
maximum query CPU permits:            max(1, floor(0.50 × Rayon workers))
background semantic/derivation:       remaining shared permits, capped at 0.30 × workers
per-agent active queries:             2
per-agent queued queries:             4
global active queries:                min(8, max(2, floor(N / 2)))
per-workspace simultaneous rustc jobs: 1
per-workspace Pyrefly runs:           1 active, module-level internal concurrency bounded
```

The shared permit ceiling prevents the category maxima from oversubscribing the pool. External compiler processes count against CPU/memory admission even though they use separate OS threads.

Query queue scheduling is deficit round robin by agent with weight 1. Administrative recovery has weight 4; no ordinary agent receives a higher permanent weight. A queued query waiting more than 5 seconds receives bounded aging, but cannot outrank P0/P1 freshness work.

Starvation rules:

- source reconciliation always retains at least one permit;
- when background work remains pending for 30 seconds and no strict-current deadline is at risk, it receives at least 10% of CPU permit time;
- one large query cannot consume more than half of global query memory/spill budget;
- artifact quotas are per agent and workspace as in `G-63`;
- repeated cancelled/failed expensive requests may trigger temporary admission backoff by credential ID.

Effective budgets are published in the deployment profile and status.
## AC-G-69 — Fine-grained source disclosure and fact ACL policy
### Decision

Fact authorization and source-byte authorization are separate. The default local launcher grants full fact and source access for its selected workspace, but the contract supports path/fact/source-context restrictions and records their effect on completeness.

### Contract

ACL decision levels per source partition/path/entity:

```text
ALLOW_FACTS_AND_SOURCE
ALLOW_FACTS_METADATA_ONLY
ALLOW_FACTS_REDACT_LOCATION
DENY_FACTS_AND_SOURCE
```

Credential claims select one or more ordered ACL profiles. Evaluation occurs at:

1. source inventory/catalog scan;
2. entity/fact query filtering;
3. path/location display;
4. source-context retrieval;
5. generated/external source retrieval;
6. artifact construction;
7. errors/status/logging.

Semantics:

- `ALLOW_FACTS_AND_SOURCE`: normal facts, locations, and bytes;
- `ALLOW_FACTS_METADATA_ONLY`: facts and canonical IDs may be returned; exact path/text are withheld and location uses a stable redacted source ID;
- `ALLOW_FACTS_REDACT_LOCATION`: non-location facts may be returned; source owner/path/span fields are omitted or redacted according to schema;
- `DENY_FACTS_AND_SOURCE`: entities/facts owned solely by the denied partition are excluded from the authorized query universe.

A stable redacted path label is derived from workspace, ACL profile, and file ID; it is not reversible by the client.

Authorization is applied before entity matching and cost/completeness computation. The response includes `authorization_scope_fingerprint`, counts of excluded partitions, and whether exclusions prevent a broader completeness/negative proof. A proof may still be complete **within the explicitly authorized universe** but SHALL not be worded as a statement about the entire workspace unless no relevant exclusion exists.

No source bytes may leak through fact statements, diagnostic messages, context lines, generated-source resources, artifact subresources, status, traces, or provider stderr. All such surfaces reuse the same ACL decision service.

The query source-boundary compiler can narrow ACL scope but never widen it.


## Cross-layer integration obligations

The following architecture-completion contracts are owned by another 1.3 artifact but are binding inputs to this specification. This document SHALL consume the named contract and SHALL NOT restate it with different semantics.

| Gap | Contract | Permanent owner | Integration obligation in this document |
|---|---|---|---|
| `G-09` | Generalized source-instance identity | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) | Transport, authenticate, stream, externalize, and report the contract without changing semantic meaning. |
| `G-19` | Complete `ServingSnapshot` manifest schema | [Data-fabric specification 1.3](./present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md) | Transport, authenticate, stream, externalize, and report the contract without changing semantic meaning. |
| `G-24` | Formal freshness state machine and query barrier | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) | Transport, authenticate, stream, externalize, and report the contract without changing semantic meaning. |
| `G-44` | Controlled semantic language grammar and phrase registry | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Transport, authenticate, stream, externalize, and report the contract without changing semantic meaning. |
| `G-45` | Deterministic semantic resolver architecture | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Transport, authenticate, stream, externalize, and report the contract without changing semantic meaning. |
| `G-46` | Typed internal `PlanSpec` | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Transport, authenticate, stream, externalize, and report the contract without changing semantic meaning. |
| `G-47` | Result-reference role type system and selector grammar | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Transport, authenticate, stream, externalize, and report the contract without changing semantic meaning. |
| `G-48` | Completeness and negative-proof algebra | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Transport, authenticate, stream, externalize, and report the contract without changing semantic meaning. |
| `G-49` | Entity matching, qualified-name parsing, grouping, and ranking | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Transport, authenticate, stream, externalize, and report the contract without changing semantic meaning. |
| `G-50` | Semantic source-boundary compiler | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Transport, authenticate, stream, externalize, and report the contract without changing semantic meaning. |
| `G-51` | Multi-context query semantics | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Transport, authenticate, stream, externalize, and report the contract without changing semantic meaning. |
| `G-52` | Query cost model, defaults, and hard limits | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Transport, authenticate, stream, externalize, and report the contract without changing semantic meaning. |
| `G-53` | Canonical JSON and checksum contract | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Transport, authenticate, stream, externalize, and report the contract without changing semantic meaning. |
| `G-54` | Canonical human-readable fact statements | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Transport, authenticate, stream, externalize, and report the contract without changing semantic meaning. |
| `G-55` | Source-context wire encoding | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Transport, authenticate, stream, externalize, and report the contract without changing semantic meaning. |
| `G-56` | Streaming, chunk interning, terminal completeness, and resumability | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Transport, authenticate, stream, externalize, and report the contract without changing semantic meaning. |
| `G-57` | Query plan cache contract | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Transport, authenticate, stream, externalize, and report the contract without changing semantic meaning. |
| `G-62` | Daemon service, configuration, discovery, singleton, and upgrade behavior | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) | Transport, authenticate, stream, externalize, and report the contract without changing semantic meaning. |

## Release conformance obligations

This specification inherits `G-78` through `G-84` from the suite governance and release manifest. Release acceptance SHALL include the portions of the golden corpus, clean-rebuild comparator, conformance harness, deterministic fault matrix, performance profiles, upgrade choreography, and adversarial security corpus that exercise Protobuf RPC, credentials, local IPC, cancellation, artifacts, MCP resources, public status, fairness, source ACLs, adapter schemas, and upgrade behavior.

A passing prose review is insufficient. The corresponding generated registries, schemas, protocol descriptors, fixtures, canonical outputs, and fault oracles SHALL pass the master release gates before an implementation may claim CodeFabric 1.3 conformance.

# Appendix A — Recommended Environment Variables

```text
CODEFABRIC_CPG_DAEMON_TARGET
CODEFABRIC_WORKSPACE_ID
CODEFABRIC_AGENT_INSTANCE_ID
CODEFABRIC_CPG_CAPABILITY_TOKEN
CODEFABRIC_CPG_QUERY_TIMEOUT_SECONDS
CODEFABRIC_CPG_INLINE_RESULT_BYTES
CODEFABRIC_CPG_MAX_REQUEST_BYTES
CODEFABRIC_CPG_MAX_JSON_DEPTH
CODEFABRIC_CPG_MAX_JSON_NODES
CODEFABRIC_CPG_MAX_VALIDATION_ERRORS
CODEFABRIC_CPG_LOG_LEVEL
CODEFABRIC_CPG_OTEL_ENDPOINT
CODEFABRIC_CPG_RESULT_TTL_SECONDS
```

Rules:

- required values are validated at startup;
- agent/workspace identifiers may be logged only under approved policy;
- capability token never appears in logs or model dumps;
- environment values are launch configuration, not tool arguments;
- production dotenv loading is disabled;
- settings remain frozen for process lifetime.

---

# Appendix B — Version and Timeout Matrix

| Component | Version | Timeout | Notes |
|---|---:|---:|---|
| FastMCP | `3.4.7` | n/a | exact stable pin |
| Pydantic | `2.13.4` | n/a | adapter contract and schema generator |
| pydantic-settings | `2.15.0` | n/a | process configuration |
| MCP server product | `1.3.0` | n/a | adapter release |
| `query_code_graph` | `1.3` | 120 s | daemon hard budget authoritative |
| `validate_code_graph_query` | `1.3` | 20 s | no fact execution |
| `get_code_graph_status` | `1.3` | 5 s | live status |
| `get_code_graph_reference` | `1.3` | 10 s | package/small reference payload |

---

# Appendix C — Generated Adapter Schema Policy

The MCP delivery/output schemas SHALL NOT be hand-maintained JSON documents.

Normative source and derivation:

```text
typed Contract IR
    → generated statically typed Pydantic public model source
    → imported Pydantic models and compiled CoreSchema
    → validation-mode schema
    → serialization-mode schema
    → RFC 8785 canonical JSON
    → BLAKE3-256 content fingerprint
    → package resource + CI snapshot
```

The Contract IR owns adapter field identity, aliases, constraints, unions, and public
documentation. Pydantic owns validation, CoreSchema compilation, and both JSON Schema
views. Independently hand-maintained adapter JSON Schemas are prohibited. Models and
`TypeAdapter` instances are created at module import or lifespan construction and reused;
handlers SHALL NOT compile them per request.

FastMCP tool `output_schema` SHALL use serialization mode:

```python
QueryToolOutput.model_json_schema(mode="serialization")
```

CI SHALL also snapshot validation mode to detect unexpected acceptance changes:

```python
QueryToolOutput.model_json_schema(mode="validation")
```

The complete semantic request and canonical response schemas remain separately packaged daemon-owned artifacts and SHALL not be generated from these adapter models.

---

# Appendix D — Pydantic Feature Decision Matrix

| Feature | Decision | Reason |
|---|---|---|
| `BaseSettings` | use | typed immutable startup config |
| `SecretStr` | use | reduce accidental representation leakage |
| strict public `BaseModel` | use | schema-closed output contracts |
| discriminated unions | use | impossible delivery combinations become unrepresentable |
| `TypeAdapter[dict[str, JsonValue]]` | use | compact JSON structural boundary |
| serialization-mode JSON Schema | use | matches actual model-visible output |
| validation-mode JSON Schema | snapshot | acceptance regression testing |
| `model_dump(mode="json")` | use | public serialization contract |
| `ValidationError.errors()` | use | structured safe translation |
| `extra="forbid"` public models | use | catch contract drift |
| `extra="ignore"` daemon DTOs | selective | negotiated additive RPC compatibility only |
| full semantic query Pydantic graph | prohibit | duplicates daemon authority |
| per-fact Pydantic models | prohibit | graph-size Python overhead |
| `model_construct()` for external data | prohibit | bypasses validation |
| experimental partial validation | prohibit | terminal MCP requests/results must be complete |
| `SerializeAsAny` / broad duck typing | prohibit | accidental field leakage |
| broad polymorphic serialization | prohibit | public schema must remain closed |
| generic serialization fallback | prohibit | silently changes contract |
| dynamic model generation per request | prohibit | CPU/memory/cache attack surface |
| I/O in validators/serializers | prohibit | lifecycle/service boundary violation |
| `@validate_call` on FastMCP tools | avoid | duplicates FastMCP call validation |
| independent `pydantic-core` pin | prohibit | version incompatibility risk |

---

# Appendix E — Anti-pattern Inventory

- One MCP tool per semantic query form.
- One Python data-fabric instance per agent.
- Python-generated SQL from semantic phrases.
- Arbitrary SQL or edge-label tool inputs.
- Dynamic exposure of every daemon method.
- Full semantic request reimplementation as Pydantic models.
- Pydantic model construction for every fact record.
- FastMCP task state layered over daemon query state.
- Response cache without publication and agent identity.
- Printing logs to STDOUT.
- Reconnecting to the daemon for every call.
- Loading Arrow/Delta/DataFusion Python packages into the adapter.
- Truncating facts to satisfy payload limits.
- Treating a result preview as the canonical response.
- Retrying partial execution without daemon idempotency.
- Answering from stale data when the daemon is unavailable.
- Logging full Pydantic errors, input, requests, or responses.
- Forwarding `result.operational_meta` or any unrestricted daemon mapping.
- Using `SerializeAsAny`, broad polymorphic serialization, or generic fallback on public outputs.
- Using `model_construct()` for daemon or user data.
- Enabling experimental partial validation for a terminal request/result.
- Building a `TypeAdapter` inside each tool call.
- Generating Pydantic models from agent-supplied schemas.
- Reading dotenv files implicitly in production.
- Treating `SecretStr` as encryption.
- Treating FastMCP annotations or Pydantic validation as authorization.
- Mixing prerelease framework semantics into the stable implementation.

---

# Appendix F — Production Readiness Checklist

## Contract

```text
[ ] Server, tool, adapter, semantic-query, RPC, ontology, schema, and publication versions are distinct.
[ ] Four-tool catalog is stable and fingerprinted.
[ ] Semantic request/response schemas are packaged and hash-verified.
[ ] Pydantic validation/serialization schemas are generated and reviewed.
[ ] Tool descriptions and instructions are tested with target agents.
```

## Pydantic

```text
[ ] pydantic==2.13.4 exact-pinned.
[ ] pydantic-settings==2.15.0 exact-pinned.
[ ] pydantic-core is not independently forced.
[ ] Public models are strict, frozen, schema-closed, and extra-forbid.
[ ] Delivery uses a discriminated union.
[ ] JSON TypeAdapter is module-scoped and reused.
[ ] Settings are immutable and production dotenv is disabled.
[ ] Capability token uses SecretStr and never appears in dumps/logs.
[ ] ValidationError translation omits input/context/causes.
[ ] Public ToolResult.meta is an explicit allowlist.
[ ] Subclass-field leakage and broad serialization are regression-tested.
```

## Daemon boundary

```text
[ ] UDS/loopback endpoint is protected.
[ ] Capability token and workspace authorization are enforced.
[ ] Handshake fails on major/schema mismatch.
[ ] Multiple in-flight RPCs are supported.
[ ] Deadlines and cancellation propagate end-to-end.
[ ] Additive daemon fields cannot leak into public output.
```

## Semantic correctness

```text
[ ] Snapshot pins before semantic resolution.
[ ] All eight query forms pass conformance tests.
[ ] Dependency cycles and type mismatches are rejected.
[ ] Unknowns, uncertainty, directness, and coverage are preserved.
[ ] No negative claim is inferred from missing data.
[ ] No syntax/name/stale fallback exists.
```

## Delivery

```text
[ ] Inline threshold is benchmarked with real hosts.
[ ] Large results externalize without truncation.
[ ] Result resources are immutable, checksummed, scoped, and expiring.
[ ] Resource manifests validate through public Pydantic contracts.
[ ] Expiry and cross-agent denial are tested.
```

## FastMCP and operations

```text
[ ] fastmcp==3.4.7 exact-pinned.
[ ] Lifespan owns Settings and daemon client.
[ ] Strict input and masked generic errors are enabled.
[ ] No arbitrary STDOUT logging exists.
[ ] Middleware order is tested.
[ ] Background tasks and query response caching are disabled.
[ ] In-memory and real STDIO tests pass.
[ ] Adapter and daemon traces correlate.
[ ] Multiple adapter processes are load-tested.
[ ] Upgrade and rollback runbooks exist.
```

---

# Appendix G — Final Design Rules

```text
Rule 1:  Publish one canonical composable fact-query tool, not physical query tools.
Rule 2:  Keep semantic request meaning entirely inside the Rust daemon.
Rule 3:  Use FastMCP lifespan for one immutable Settings snapshot and daemon client.
Rule 4:  Use Pydantic only for adapter settings, DTOs, public contracts, and schemas.
Rule 5:  Keep the full semantic request/response schemas daemon-authoritative.
Rule 6:  Apply freshness and pin the ServingSnapshot before resolving names.
Rule 7:  Return every query block in one canonical response envelope.
Rule 8:  Preserve uncertainty, unknowns, directness, and representation boundaries.
Rule 9:  Model inline/resource delivery as a discriminated union.
Rule 10: Externalize large complete responses; never silently truncate.
Rule 11: Build public output and ToolResult.meta from explicit schema-closed models.
Rule 12: Never forward unrestricted daemon dictionaries to the agent.
Rule 13: Reuse TypeAdapters and models; never build schemas on hot paths.
Rule 14: Treat STDIO security as process/socket/capability security.
Rule 15: Propagate progress, deadlines, cancellation, identity, and traces end-to-end.
Rule 16: Keep Python free of fact generation, graph traversal, and DataFusion planning.
Rule 17: Generate and fingerprint validation and serialization schemas separately.
Rule 18: Version and test every contract independently.
```
