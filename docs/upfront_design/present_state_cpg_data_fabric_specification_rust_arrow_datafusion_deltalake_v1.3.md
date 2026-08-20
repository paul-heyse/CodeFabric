# Present-State CPG Data Fabric Specification

**Artifact ID:** `codefabric-present-state-cpg-data-fabric`
**Artifact kind:** Normative document
**Compatible suite major:** 1
**Release date:** 2026-08-20
**Canonical digest:** External; recorded in `codefabric_v1.3_manifest.json`

**Status:** Released normative implementation specification
**Synchronized suite version:** 1.3
**Specification version:** 1.3
**Companion specification:** `present_state_cpg_fact_generation_specification_python_rust_v1.3.md`
**Primary implementation language:** Rust
**Core data-plane technologies:** Apache Arrow Rust, Apache DataFusion Rust, and `deltalake` / delta-rs
**Logical scope:** Present-state Python and Rust code-property-graph facts and mechanically derived facts
**Excluded semantic scope:** Git/history analytics, runtime observation, test-impact assessment, refactor assessment, risk scoring, recommendations, and other evaluative conclusions
**Audit integration (2026-08-20):** Plan-audit F-006/F-007; separated table policy axes and adopted pinned delta-rs application transactions.

---

## 0. Synchronized CodeFabric 1.3 governing contract

This document is a released member of the synchronized **CodeFabric present-state CPG specification suite, version 1.3**. The suite integrates the architecture-completion contracts `G-01` through `G-84`; the earlier standalone completion specification is retained only as a historical design record and is no longer required to interpret this release.

The cross-cutting source of authority is `codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md`. This document permanently owns the domain contracts assigned to it by that manifest. A less-specific statement elsewhere in this document SHALL be read through the 1.3 contract sections and SHALL NOT override them.

### 0.1 Artifact identity and version

```yaml
artifact_id: "codefabric-present-state-cpg-data-fabric"
artifact_kind: document
version: "1.3"
compatible_suite_major: 1
status: released
canonical_digest: external
```

The canonical digest is recorded in `codefabric_v1.3_manifest.json`. Versions are integer pairs, never floating-point values; `1.10` is newer than `1.9`.

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

This document specifies the complete data fabric that stores, transforms, derives, publishes, and serves the present-state CPG facts defined by the companion fact-generation specification.

The fabric SHALL provide:

- one canonical Arrow representation for every fact batch;
- one transactional Delta Lake persistence contract for every durable table;
- one DataFusion catalog and planning surface over the published current state;
- deterministic owner-scoped replacement of facts;
- cross-table publication consistency;
- typed relational schemas for all fact families;
- a universal graph projection for generic traversal;
- vectorized and streaming calculations for reconciliation and derived facts;
- high-performance storage layout, pruning, compaction, and query patterns;
- explicit schema, integrity, provenance, completeness, and unknown-state contracts.

The governing architecture is:

```text
Fact providers and derived analyzers
        ↓
Arrow RecordBatch streams
        ↓
Schema validation and normalization
        ↓
DataFusion reconciliation / derivation plans
        ↓
Delta Lake owner-scoped table updates
        ↓
Durable publication pins exact Delta versions
        ↓
ServingSnapshot combines the durable base with one hot overlay
        ↓
Overlay-aware DataFusion current-state catalog
        ↓
LLM-agent fact queries
```

The data fabric SHALL stop at factual storage and factual calculation. It SHALL NOT encode conclusions such as `SAFE_TO_REFACTOR`, `TEST_IMPACTED`, `HIGH_RISK`, or `SHOULD_CHANGE`.

---

## 2. Source basis and version anchors

This specification is grounded in the attached references and uses their terminology and version posture.

| Technology | Version anchor used by this specification | Primary role |
|---|---:|---|
| Arrow Rust | `58.4.0` family | Canonical in-memory schemas, arrays, buffers, builders, `RecordBatch`, vectorized kernels, Parquet interchange |
| DataFusion Rust | `54.1.0` | Catalog, SQL/DataFrame/Expr planning, streaming execution, joins, aggregations, custom functions, custom logical/physical operators |
| `deltalake` / delta-rs | `1.0.0` at git rev `9f9223197469897ef05ae4369eb4fd1390174e65` | Transactional Delta tables, table schemas, DataFusion providers, writes, DML, constraints, optimize, vacuum |
| Parquet Rust | `58.4.0` | Physical data-file format beneath Delta Lake |
| `object_store` | `0.13.2` | Local and object-store I/O used by DataFusion and delta-rs |
| Rust toolchain | `1.94.1` for the pinned delta-rs baseline | Workspace compatibility floor |
| Delta kernel | `buoyant_kernel` and `buoyant_kernel_engine` on the released `0.25.x` line | Selected **transitively** by the pinned delta-rs revision; not independently pinned by CodeFabric |

The delta-rs `1.0.0` target is a pinned pre-release revision rather than a tagged stable release: the upstream workspace declares crate version `1.0.0`, but no `rust-v1.0.0` tag exists and the upstream changelog still begins at `rust-v0.32.3`. All code generated from this specification SHALL be compile-tested against that exact revision before adoption.

The Rust floor of `1.94.1` is set by the pinned revision itself, which raised its MSRV from `1.91.1` after upstream AWS crates increased theirs. It is a build-tooling obligation, not a CodeFabric language-feature requirement.

The storage-substrate contracts in sections 2, 12.5–12.9, 67.3, 98.1–98.3, 100.1, 101.1, 103.4, 111.1 and 112.6 were integrated from `docs/codefabric_delta_rs_9f922319_design_change_recommendations_2026-08-20.md`, which assessed the move from delta-rs `35cfed45…` to `9f922319…`. That assessment found no required change to the ontology, semantic query model, hot-overlay model, multi-table publication model, or `ServingSnapshot` consistency semantics; the changes are confined to the implementation baseline, the provider lifecycle, and the conformance suite.

The pinned revision declares looser upstream requirements than CodeFabric's exact pins — `arrow = "58"`, `parquet = "58"`, `datafusion = "54.0.0"` — all of which are caret requirements satisfied by CodeFabric's `=58.4.0` and `=54.1.0`. CodeFabric pins exactly where delta-rs pins loosely; the exact pins remain authoritative for this specification.

### 2.1 Canonical workspace baseline

```toml
[workspace]
resolver = "3"

[workspace.package]
edition = "2024"
rust-version = "1.94.1"

[workspace.dependencies]
datafusion = "=54.1.0"

arrow = "=58.4.0"
arrow-array = "=58.4.0"
arrow-buffer = "=58.4.0"
arrow-schema = "=58.4.0"
arrow-cast = "=58.4.0"
arrow-select = "=58.4.0"
arrow-ord = "=58.4.0"
arrow-string = "=58.4.0"
arrow-row = "=58.4.0"

parquet = { version = "=58.4.0", features = ["arrow", "async", "object_store"] }
object_store = "=0.13.2"

deltalake = {
  git = "https://github.com/delta-io/delta-rs.git",
  rev = "9f9223197469897ef05ae4369eb4fd1390174e65",
  default-features = false,
  features = ["rustls", "datafusion"]
}

tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
futures = "0.3"
url = "2"
tracing = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
blake3 = "1"
```

Utility crates such as `blake3`, `serde`, `tokio`, and `futures` MAY be used inside the Rust implementation. The storage, batch, query, and relational-computation engines SHALL remain Arrow, DataFusion, and Delta Lake.

Resolver `3` is required: the pinned delta-rs workspace uses it, and it is the resolver that participates in Rust-version-aware dependency resolution — which matters once a `rust-version` floor is declared.

Object-store backends are **not operationally enabled** by the default CodeFabric
feature set. The mandatory `local-workstation-v1` profile writes to a local
filesystem Delta namespace and SHALL accept neither cloud URL schemes nor cloud
credentials, endpoints, or storage-option maps:

```toml
[features]
default = ["local-workstation"]
canonical-json = [
  "dep:base64", "dep:blake3", "dep:serde", "dep:serde_json",
  "dep:serde_json_canonicalizer", "dep:thiserror",
]
contracts-tooling = ["canonical-json", "dep:serde_yaml_ng", "dep:tempfile"]
data-fabric = [
  "dep:arrow", "dep:arrow-array", "dep:arrow-buffer", "dep:arrow-cast",
  "dep:arrow-ord", "dep:arrow-row", "dep:arrow-schema", "dep:arrow-select",
  "dep:arrow-string", "dep:datafusion", "dep:deltalake", "dep:futures",
  "dep:object_store", "dep:parquet", "dep:tracing",
]
rpc = ["dep:prost", "dep:tokio", "dep:tonic", "dep:tonic-prost"]
repository-state = ["dep:gix", "dep:rusqlite", "dep:rustix", "dep:url"]
compatibility-probes = ["canonical-json", "data-fabric", "repository-state", "rpc"]
local-workstation = ["contracts-tooling", "compatibility-probes"]
proto-tooling = ["dep:prost-build", "dep:protoc-bin-vendored", "dep:tonic-prost-build"]
s3-storage = ["data-fabric", "deltalake/s3"]
```

These are additive build-capability features inside the single stable root package, not
deployment alternatives and not semantic source-decomposition rules. Narrow contract,
Protobuf, and fuzz invocations SHALL disable default features and select only their owning
capability. The default `local-workstation` aggregate SHALL retain the complete accepted
local production graph. Source modules, required-feature binaries, the single integration
test target, local recipes, and CI SHALL use the same feature ownership. The accepted
correction and its proof obligations are specified in
`docs/designs/codefabric_build_cache_and_feature_isolation_design_v1_2026-08-20.md`.

A deployment profile that requires object-store durability SHALL enable the
corresponding storage feature explicitly. In particular, the default resolved
graph SHALL contain neither `deltalake-aws` nor the `aws-sdk-*` family, while
`s3-storage` SHALL resolve `deltalake-aws` through `deltalake/s3`.

The exact pinned graph has one important limitation that graph evidence SHALL
report rather than hide: `buoyant_kernel` 0.25.x's `arrow-58` feature
unconditionally requests `object_store` 0.13.2 with its `aws`, `azure`, `gcp`,
and `http` features. Those latent implementations therefore compile in the
default binary even though CodeFabric neither registers nor authorizes them
under `local-workstation-v1`. Cargo feature unification cannot subtract those
features, and a direct dependency declaration with fewer features does not
change the downstream request.

Accordingly, local-profile isolation is an application-owned provider and
configuration boundary, not a claim that all cloud-related code is absent from
the compiled graph. The resolved-feature report, the absence of
`deltalake-aws`/AWS SDK packages, and negative provider-factory tests are all
required evidence. Any advisory exception forced by this latent surface SHALL
be exact-ID and exact-version bound, carry an owner and review trigger in a
committed machine-readable registry, and be removed or explicitly re-approved
before an S3-enabled release. Removing `s3` from the default set remains
dependency and authority hygiene; it does not change any durable-state
contract in this specification.

### 2.2 Version-alignment invariant

```text
one Arrow major/minor family
one Parquet family matching Arrow
one DataFusion family
one object_store family
one pinned delta-rs revision
one transitively selected Delta kernel line (buoyant_kernel + buoyant_kernel_engine)
```

CI SHALL reject duplicate Arrow, Parquet, DataFusion, or `object_store` versions that cross public type boundaries.

The Delta kernel line is part of the alignment universe even though CodeFabric does not pin it directly. `buoyant_kernel` and `buoyant_kernel_engine` are compiled against a specific Arrow feature (`arrow-58`), so a kernel pair drawn from a different line can introduce a second Arrow type universe underneath `deltalake` without appearing anywhere in CodeFabric's own manifests. The duplicate-version gate SHALL therefore inspect the resolved graph, not only the declared dependencies, and SHALL fail when the kernel pair is split across minor lines or bound to a different Arrow feature than the workspace Arrow family.

CodeFabric does not pin the kernel. The pinned delta-rs revision consumes **released** `buoyant_kernel` and `buoyant_kernel_engine` crates on the `0.25.x` line — the engine is a separate package from the kernel — rather than a separately git-pinned kernel revision.

CodeFabric SHALL NOT declare those crates directly merely because delta-rs depends on them. The `deltalake` git pin plus the committed `Cargo.lock` selects the matching pair transitively, and doing so keeps the number of unpublished upstream revisions CodeFabric must coordinate at one.

Should a CodeFabric crate ever need kernel APIs directly, it SHALL:

- declare both `buoyant_kernel` and `buoyant_kernel_engine` where engine facilities are required;
- isolate every kernel type behind one application-owned adapter module;
- expose no kernel type across a CodeFabric public crate boundary;
- carry a compile test against the exact pinned delta-rs revision.

This is the same provider-isolation discipline the fact-generation specification applies to Tree-sitter, Ruff, Pyrefly and `rustc_public`.

---

# Part I — Architectural Model

## 3. Technology responsibility model

### 3.1 Arrow responsibility

Arrow SHALL be the canonical in-memory and inter-component fact representation.

Arrow owns:

- canonical `DataType`, `Field`, and `Schema` contracts;
- typed array builders;
- null bitmaps;
- immutable `RecordBatch` publication units;
- zero-copy slicing and projection where possible;
- vectorized scalar kernels;
- batch streams between extractors, reconciler, DataFusion, and Delta writers;
- Parquet schema hints and round-trip fixtures.

Arrow SHALL NOT be treated as a graph database or durable table catalog.

### 3.2 DataFusion responsibility

DataFusion SHALL be the relational planning and execution engine.

DataFusion owns:

- the current-state query catalog;
- `DataFrame`, `Expr`, and `LogicalPlanBuilder` pipelines;
- schema binding and type coercion;
- projection, predicate, and limit pushdown;
- joins, aggregations, windows, sorting, and union;
- custom UDFs, UDAFs, UDTFs, logical nodes, and `ExecutionPlan`s;
- graph and dataflow calculations implemented as custom Rust operators;
- validation queries and publication integrity checks;
- streaming query results as `SendableRecordBatchStream`.

DataFusion SHALL NOT be treated as the durable source of truth.

### 3.3 Delta Lake responsibility

Delta Lake SHALL be the durable, transactional table-state authority.

Delta Lake owns:

- durable table schemas;
- atomic per-table commits;
- optimistic concurrency control;
- active Parquet file selection;
- append, delete, update, merge, and bounded overwrite operations;
- table constraints and metadata;
- exact table versions used by a publication;
- compaction, Z-order maintenance, and vacuum;
- object-store portability.

### 3.4 Canonical invariant

```text
Delta is the durable table-state authority.
DataFusion is the query and calculation engine.
Arrow is the batch and memory contract.
Parquet is the physical fact-file representation.
```

---

## 4. Present-state semantics and operational versioning

The semantic product exposes one present-state graph. Delta transaction history exists as a storage mechanism but SHALL NOT be exposed as a code-history ontology.

The fabric distinguishes:

```text
semantic history       excluded
transaction versions   required for atomic storage and recovery
```

Old Delta versions MAY exist temporarily for:

- multi-table publication consistency;
- failed-publication recovery;
- optimistic retry resolution;
- maintenance safety.

They SHALL NOT be presented to agents as prior code states unless a separate future history product is explicitly introduced.

---

## 5. Hybrid relational graph design

The canonical physical model SHALL be a **hybrid relational graph**:

1. a universal `entity` Delta table stores every graph node;
2. a universal `relation` Delta table stores every graph edge;
3. strongly typed extension tables store payloads that do not fit the common graph envelope;
4. control tables publish exact versions of all tables;
5. serving views combine generic topology with typed detail.

This design is mandatory because it provides:

- generic graph traversal without unioning dozens of tables;
- typed schemas without an EAV property store;
- predicate pushdown by entity/relation family;
- fast source/target joins;
- extension-table scans only when payload detail is requested;
- deterministic materialization into Arrow streams.

### 5.1 Explicitly prohibited canonical designs

The following SHALL NOT be canonical persistence models:

```text
one JSON blob per entity
one Map<String,String> property bag per fact
one EAV table for all properties
one serialized petgraph object
one opaque provider-native payload as the only representation
one table per individual relation kind
```

Cold provider evidence MAY use a compact map or binary payload, but canonical queryable fields SHALL remain typed columns.

---

## 6. Deployment topology

### 6.1 Default topology: one fact namespace per `workspace_id`

The present-state storage unit is one authorized analyzed source instance.

```text
/cpg/<workspace-id>/control/...
/cpg/<workspace-id>/facts/...
/cpg/<workspace-id>/derived/...
```

The namespace is valid for both:

```text
Git-backed workspace  -> one main or linked worktree
non-Git workspace     -> one authorized filesystem root
```

Every row still carries `workspace_id` so exported/shared tables remain self-describing and cross-workspace mixing can be rejected. `repository_id` and `worktree_id` are nullable subordinate identities.

A common Git repository MAY have a separate shared immutable cache area:

```text
/cpg-common/<repository-id>/objects-and-read-caches/...
```

This area is not a present-state fact namespace and SHALL not contain one common current pointer for multiple worktrees.

### 6.2 Shared-corpus topology

A shared corpus uses the same schemas and keeps `workspace_id` as the leading identity/partition dimension. It MAY add `workspace_bucket` for bounded partitioning. `repository_id` is never a substitute partition key for present state.

### 6.3 Catalog namespaces

The DataFusion catalog exposes:

```text
cpg_control   workspace, repository/worktree, contexts, publications,
              active ServingSnapshot metadata, capabilities, diagnostics,
              and read-only operational-state views
cpg_base      canonical entities, relation facts, property facts, evidence
cpg_python    Python-specific extensions
cpg_rust      Rust/MIR-specific extensions
cpg_derived   registered materialized derived facts and summaries
cpg_serving   stable overlay-aware views and table functions
```

### 6.4 Daemon and coordinator topology

One daemon MAY host a repository/worktree group and multiple workspaces. It owns one `WorkspaceCoordinator`, operational-state partition, active snapshot pointer, and query admission domain per `workspace_id`. A non-Git workspace participates identically except that repository/worktree/Git fields are null or `NOT_APPLICABLE`.

# Part II — Canonical Types, Identity, and Schema Contracts

## 7. Canonical physical types and identity

The schema registry SHALL define the following reusable logical types.

| Logical type | Arrow type | Delta type | Invariant |
|---|---|---|---|
| `id16` | `Binary` | `BINARY` | exactly 16 bytes |
| `hash32` | `Binary` | `BINARY` | exactly 32 bytes |
| `code16` | `Int16` | `SHORT` | registered enum code |
| `code32` | `Int32` | `INTEGER` | registered enum code |
| `flags64` | `Int64` | `LONG` | registered bitset |
| `count64` | `Int64` | `LONG` | non-negative |
| `byte_offset` | `Int64` | `LONG` | non-negative byte position |
| `ordinal32` | `Int32` | `INTEGER` | ordered role position |
| `bucket16` | `Int16` | `SHORT` | bounded operational bucket |
| `text` | `Utf8` | `STRING` | losslessly decoded/display text only |
| `bytes` | `Binary` | `BINARY` | authoritative bytes |
| `timestamp_utc` | `Timestamp(Microsecond, UTC)` | `TIMESTAMP` | operational time only |
| `id_list` | `List<Binary>` | `ARRAY<BINARY>` | sorted/deduplicated where specified |
| `string_map` | `Map<Utf8,Utf8>` | `MAP<STRING,STRING>` | cold diagnostic metadata only |

### 7.1 ID derivation

All durable IDs are application-owned 16-byte values. The canonical preimage is domain-separated, versioned, and length-prefixed. Context-dependent entities and facts include both `workspace_id` and `analysis_context_id`; source-only records use `context:source`.

```text
entity_id = BLAKE3_128(domain/entity || workspace || context || semantic key)
fact_id   = BLAKE3_128(domain/fact   || workspace || context || fact form/kind/value)
owner_id  = BLAKE3_128(domain/owner  || workspace || context || owner key)
type_id   = BLAKE3_128(domain/type   || workspace || context || canonical type algebra)
```

The full 256-bit digest SHOULD be retained in collision-diagnostic storage. A detected unequal-preimage collision fails activation with `ID_COLLISION`.

### 7.2 Public ID encoding

Binary IDs round-trip through the ontology 1.3 lowercase public encoding, including kind-validated `entity:<kind>:<hex>` and `fact:<kind>:<hex>` forms. Display prefixes never participate in internal identity.

### 7.3 Buckets and accelerators

Buckets derive from the first digest byte and remain operational. Signed `*_hash64` columns MAY be added for statistics/Z-ordering and SHALL be marked hidden operational metadata.

---

## 8. Canonical enum and state registries

All categorical columns use the synchronized ontology 1.3 registries. Numeric meanings are append-only and SHALL not be assigned independently in this document.

Required domains include ordinary ontology kinds plus these orthogonal state domains:

```text
provider_run_state
owner_capability_state
completeness_state
query_execution_state
query_availability_state
freshness_state
durable_publication_state
serving_activation_state
source_trust_state
event_stream_health
```

All shared state, owner-capability, completeness, certainty, resolution, and directness codes are exactly those in ontology §§62.1–62.10. DataFusion registers immutable dimension batches for code-to-name lookup; an `enum_catalog` Delta table MAY mirror them for external introspection.

---

## 9. Common canonical fact metadata

Every canonical entity, relation fact, and property fact carries or inherits:

```text
workspace_id
analysis_context_id
source_generation
owner_id
owner_bucket
language
certainty_code
resolution_code
directness_code
producer_code
derivation_code optional
file_id/start_byte/end_byte optional
fact_hash64
```

The workspace namespace does not justify omitting `workspace_id` from rows. `analysis_context_id` is required and uses `context:source` for context-independent source/syntax facts.

One entity row MAY denormalize selected canonical properties for scan efficiency. Independently sourced properties remain first-class rows in `property_fact`; `fact_evidence` supports relation and property facts without ambiguity.

## 10. Schema metadata conventions

Every Arrow schema SHALL carry:

```text
com.codefabric.cpg.table_name
com.codefabric.cpg.table_family
com.codefabric.cpg.table_grain
com.codefabric.cpg.schema_version
com.codefabric.cpg.ontology_version
com.codefabric.cpg.primary_key
com.codefabric.cpg.partition_columns
com.codefabric.cpg.durable_mutation_class
com.codefabric.cpg.overlay_mutation_policy
com.codefabric.cpg.materialization_role
com.codefabric.cpg.compatibility_mode
```

Important fields SHALL carry metadata such as:

```text
com.codefabric.cpg.semantic_type = id16 | hash32 | byte_offset | enum:<domain>
com.codefabric.cpg.primary_key_part = true
com.codefabric.cpg.foreign_key = <table>.<field>
com.codefabric.cpg.hidden_operational = true
com.codefabric.cpg.id_width = 16
```

Metadata is advisory unless consumed by explicit validation code. It SHALL NOT replace nullability, table constraints, or application integrity checks.

---

## 11. Schema registry

All schemas SHALL be defined once in a Rust schema registry.

```rust
pub struct TableSpec {
    pub table_code: i16,
    pub name: &'static str,
    pub schema_version: &'static str,
    pub arrow_schema: arrow_schema::SchemaRef,
    pub primary_key: &'static [&'static str],
    pub partition_columns: &'static [&'static str],
    pub zorder_columns: &'static [&'static str],
    pub durable_mutation: DurableMutationClass,
    pub overlay_mutation: OverlayMutationPolicy,
    pub materialization_role: MaterializationRole,
    pub dependencies: &'static [i16],
    pub required_for_publication: bool,
}
```

These are orthogonal contract axes:

```text
DurableMutationClass:
  STATIC_DIMENSION | CURRENT_SINGLETON | OWNER_REPLACED_FACT |
  PUBLICATION_APPEND | DERIVED_OWNER_REPLACED | GLOBAL_DERIVED_REPLACEMENT

OverlayMutationPolicy:
  OWNER_REPLACE | PRIMARY_KEY_UPSERT | FULL_TABLE_REPLACE |
  BASE_IMMUTABLE | NOT_APPLICABLE

MaterializationRole:
  DURABLE_EFFECTIVE | BUNDLE_DIMENSION | QUERY_TIME_DERIVED |
  OPERATIONAL_PROJECTION
```

Durable mutation governs Delta writes, overlay mutation governs how an
existing effective table participates in a hot snapshot, and materialization
role governs whether the surface is durable fact state, bundle-backed,
computed from a leased snapshot, or an operational projection. No axis may be
derived implicitly from another.

The registry SHALL generate or validate:

- Arrow `SchemaRef`;
- Delta `StructType`;
- DataFusion `DFSchema` compatibility;
- primary-key metadata;
- Delta creation properties;
- builder capacity hints;
- schema fingerprints;
- table dependency order.

### 11.1 Schema round-trip gate

Every schema SHALL pass:

```text
Arrow Schema
  → Delta StructType
  → create empty Delta table
  → open Delta table
  → DataFusion TableProvider schema
  → Arrow Schema
  → exact contract comparison
```

---

# Part III — Multi-Table Publication and Snapshot Consistency

## 12. Durable publication and serving-snapshot model

Delta transactions are atomic per table. Durable base state therefore uses manifest-pinned multi-table MVCC, while current interactive state is a `ServingSnapshot`.

### 12.1 Durable publication

A durable publication maps:

```text
publication_id
  → workspace_id
  → exact Delta version for every required durable table
  → source generation and inventory digest
  → analysis-context set
  → ontology/schema/provider/derivation/toolchain/inclusion-policy fingerprints
```

A publication has only durable states. It never claims hot activation.

### 12.2 ServingSnapshot

The sole query-pinning object is:

```text
ServingSnapshot
  snapshot_id
  workspace_id
  repository_id/worktree_id optional
  source_generation and source_inventory_digest
  source_trust/event-stream/Git summaries
  durable_base_publication and exact table-version map
  consolidated overlay generation/checksum/table manifests
  analysis_context_set_id and exact contexts
  capability-index and diagnostics-index checksums
  ontology/schema/provider/derivation/query-language bundle versions
```

The overlay contains replacement rows, owner/table tombstones, capability withdrawals, and diagnostics. A snapshot is constructed off-path, validated, and activated by one atomic pointer swap.

### 12.3 Current pointers

Two pointers are distinct:

```text
current_publication[workspace_id]  durable base pointer in Delta
active_snapshot[workspace_id]     interactive ServingSnapshot pointer in the operational store/memory
```

A common repository may have many rows in both maps, one per workspace/worktree.

### 12.4 No semantic-history guarantee

Older publications and retired snapshots exist only for reader leases, recovery, retries, and safe vacuum. They are not exposed as code-history facts.

### 12.5 Delta engine snapshots are not CodeFabric serving snapshots

Both layers use the word *snapshot*, and they mean different things. The distinction is normative.

```text
ServingSnapshot                          delta-rs Snapshot / EagerSnapshot
  = durable publication                    = storage-engine view of ONE Delta
  + exact multi-table version map            table at ONE version
  + consolidated hot overlay               + log-replay, materialization and
  + source generation                        cache state
  + analysis contexts
  + capabilities / diagnostics
  + exact interpretation bundle digests

  the only query pin                       an implementation object for
                                           reading one durable table
```

A delta-rs `Snapshot`, `EagerSnapshot`, checkpoint selection, materialized-file cache, or DataFusion `TableProvider` SHALL NOT independently define CodeFabric current-state identity. Current-state identity is defined only by the leased `ServingSnapshot` together with its exact durable Delta table-version map and overlay identity. Delta engine objects are reconstructible accelerators and SHALL NOT independently define fact freshness, completeness, publication identity, or semantic snapshot identity.

This rule exists to foreclose an attractive but incorrect simplification. The storage engine's own snapshot model is single-table and version-scoped; CodeFabric's consistency object is multi-table, overlay-bearing, and carries interpretation metadata. The former cannot substitute for the latter, however similar the vocabulary.

### 12.6 Snapshot-scoped durable provider set

For each active `ServingSnapshot`, the daemon SHOULD build one immutable set of exact-version Delta `TableProvider`s for the durable base and reuse those providers for every lease on that snapshot:

```text
ServingSnapshot
  ├─ durable publication metadata
  ├─ DeltaBaseCatalog
  │    ├─ table_code → exact delta_version
  │    ├─ table_code → Arc<dyn TableProvider>
  │    └─ table_code → table-root / schema identity diagnostics
  ├─ consolidated hot overlay
  ├─ capability index
  └─ diagnostics index
```

Activation order:

```text
1. read the immutable durable publication manifest
2. resolve the exact Delta version for each required table from publication_table
3. construct an exact-version delta-rs table/snapshot/provider
4. register the provider in the candidate snapshot's private DataFusion catalog
5. wrap it with the CodeFabric overlay-aware provider (section 91)
6. run activation integrity checks
7. freeze the catalog and provider set
8. atomically activate the new ServingSnapshot
9. every lease reuses those exact provider objects
```

Providers SHALL be discarded when their owning `ServingSnapshot` becomes unreferenced. A provider built for one publication/version SHALL NOT be rebound to another publication by mutating its underlying table state.

The abstraction stores the DataFusion provider and CodeFabric identity metadata. It SHALL NOT expose or serialize delta-rs internal snapshot or cache types.

This makes provider lifetime exactly equal to snapshot lease lifetime, makes intra-query drift between the table-version map and the provider set impossible by construction, and avoids replaying the Delta log independently for every semantic query.

### 12.7 Delta materialization caches are ephemeral

The pinned delta-rs revision maintains an internal snapshot identity over table root, Delta version, checkpoint version, protocol, and metadata, and reuses materialized file state only when it matches that identity and the requested statistics capability.

CodeFabric SHALL NOT duplicate that state. Materialized-file and statistics caches internal to delta-rs SHALL remain process-local, non-authoritative, and rebuildable. They MAY be retained by provider objects for the lifetime of a leased `ServingSnapshot`, but SHALL be reconstructible solely from the table root, pinned Delta version, and storage configuration. They SHALL NOT be serialized into the operational store, Delta control tables, or `ServingSnapshot` wire metadata, and SHALL NOT participate in semantic digests, publication equality, or query completeness proofs.

Durable recovery requires only the publication manifest, the exact table-version map, table roots and storage configuration, the schema bundle, and normal operational recovery state.

Where CodeFabric maintains its own provider or cache map, the key SHALL be:

```text
workspace_id
publication_id
table_code
delta_table_root_identity
delta_version
schema_bundle_digest
```

Keying by table name alone, or by any notion of "latest", is prohibited.

### 12.8 Checkpoint arrival is identity-neutral

A Delta checkpoint may be created, replaced, or first discovered for a table version that a publication already pins. The pinned delta-rs revision can rebuild a snapshot at the **same** Delta version once a newer checkpoint becomes available:

```text
before            entity table version 421, replay = JSON commits after checkpoint 400
maintenance       checkpoint 421 is written
after refresh     entity table version 421, replay = checkpoint 421
semantic identity unchanged
```

The addition, replacement, or later discovery of a checkpoint for an already pinned table version does not change the logical content identity of that version. Checkpoint choice is a replay optimization and SHALL NOT by itself advance publication generation, source generation, fact generation, or freshness.

A new `ServingSnapshot` MAY still be constructed for operational reasons, but its logical durable-base content digest SHALL compare equal when no Delta table version and no overlay content changed. Without this rule, background checkpoint maintenance would spuriously advance freshness generations, invalidate query caches, and produce publication churn and false state-changed notifications.

### 12.9 Publication validation is not provider construction

The pinned revision can defer active-file and statistics materialization. That is desirable for query latency and memory, and it creates one architectural hazard worth naming:

```text
cheap provider construction != durable publication validation
```

Publication validation SHALL still establish, by doing the work:

```text
the exact requested Delta table version exists
the schema digest matches the schema registry
protocol and table features are compatible
table metadata and the partition contract are correct
publication table checksums and counts pass
owner and relation integrity queries pass
every required table is present in the publication manifest
cross-table publication invariants pass
```

Where an integrity obligation requires enumerating active files or reading fact rows, the validator SHALL perform that enumeration or read. Successful construction of a delta-rs provider object SHALL NOT be treated as evidence of any of the above.

Once durable publication validation has succeeded, the provider set MAY defer file and statistics replay until a query needs it. Correctness is established eagerly; materialization stays lazy.

## 13. Control-plane schemas and operational-state store

Durable fact/publication metadata is stored in Delta. High-churn lifecycle state is stored in one embedded **SQLite WAL operational database** per daemon repository/worktree group. The daemon exposes transactionally captured read-only Arrow/DataFusion views of that database under `cpg_control`. The operational database is not a semantic-history store; retention is bounded and current/recovery oriented.

### 13.1 `workspace`

**Primary key:** `workspace_id`.

| Column | Type | Null | Meaning |
|---|---|---:|---|
| `workspace_id` | `id16` | no | One authorized source instance |
| `repository_id` | `id16` | yes | Optional common Git repository |
| `worktree_id` | `id16` | yes | Optional Git worktree |
| `workspace_kind_code` | `code16` | no | Git worktree or non-Git root |
| `canonical_name` | `Utf8` | no | Display/catalog name |
| `root_path_bytes` | `Binary` | no | Authoritative authorized root identity |
| `root_path_display` | `Utf8` | no | Non-authoritative display |
| `root_path_encoding_code` | `code16` | no | Platform/path encoding |
| `authorization_fingerprint` | `hash32` | no | Root/source-disclosure policy |
| `language_mask` | `Int16` | no | Indexed profiles |
| `created_at` | `timestamp_utc` | no | Operational creation time |

### 13.2 `common_repository`

**Primary key:** `repository_id`; absent for non-Git workspaces.

```text
repository_id
common_dir_path_bytes
common_dir_path_display
object_format_code
trust_policy_fingerprint
created_at
```

### 13.3 `analysis_context`

**Primary key:** `(workspace_id, analysis_context_id)`.

```text
workspace_id
analysis_context_id
context_kind_code          source | python | rust
context_fingerprint
provider_bundle_version
compiler_or_language_version
configuration_manifest_uri optional
active
```

### 13.4 `analysis_context_set`

**Primary key:** `analysis_context_set_id`.

```text
analysis_context_set_id
workspace_id
ordered_context_ids
set_fingerprint
created_at
```

### 13.5 `publication`

**Primary key:** `publication_id`; grain is one durable attempt.

| Column | Type | Null |
|---|---|---:|
| `publication_id` | `id16` | no |
| `workspace_id` | `id16` | no |
| `repository_id` | `id16` | yes |
| `worktree_id` | `id16` | yes |
| `durable_state_code` | `code16` | no |
| `source_generation` | `Int64` | no |
| `source_inventory_digest` | `hash32` | no |
| `analysis_context_set_id` | `id16` | no |
| `git_state_fingerprint` | `hash32` | yes |
| `inclusion_policy_fingerprint` | `hash32` | no |
| `base_fact_digest` | `hash32` | no |
| `derived_fact_digest` | `hash32` | yes |
| `ontology_version` | `Utf8` | no |
| `schema_bundle_version` | `Utf8` | no |
| `provider_bundle_version` | `Utf8` | no |
| `derivation_bundle_version` | `Utf8` | no |
| `toolchain_bundle_version` | `Utf8` | no |
| `started_at` | `timestamp_utc` | no |
| `completed_at` | `timestamp_utc` | yes |
| `required_table_count` | `Int32` | no |
| `published_table_count` | `Int32` | no |
| `diagnostic_count` | `Int64` | no |

Canonical `DurablePublicationState` values are `STAGING`, `VALIDATING`, `VALIDATED`, `COMMITTING`, `COMPLETE`, `FAILED`, and `ABANDONED`.

### 13.6 `publication_table`

**Primary key:** `(publication_id, table_code)`.

```text
publication_id
workspace_id
table_code
table_uri
delta_version
schema_fingerprint
row_count
owner_count
table_checksum
required
validated
```

### 13.7 `current_publication`

**Primary key:** `workspace_id`.

```text
workspace_id
publication_id
pointer_generation
updated_at
```

This pointer is updated last with compare-and-swap fencing. It is the durable base pointer, not the active query pointer.

### 13.8 `serving_snapshot_manifest` and `active_snapshot`

These are operational-store records exposed read-only through DataFusion.

```text
serving_snapshot_manifest:
  snapshot_id
  workspace_id
  base_publication_id
  source_generation
  source_inventory_digest
  analysis_context_set_id
  overlay_generation
  overlay_checksum
  overlay_table_manifest_bytes
  capability_index_checksum
  diagnostics_index_checksum
  source_trust_state_code
  event_stream_health_code
  git_acceleration_status_code
  bundle_versions
  serving_activation_state_code
  created_at

active_snapshot:
  workspace_id
  snapshot_id
  pointer_generation
  activated_at
```

`ServingActivationState` values are `BUILDING`, `VALIDATING`, `READY`, `ACTIVE`, `RETIRED`, and `FAILED`.

### 13.9 `owner`

**Primary key:** `(workspace_id, analysis_context_id, owner_id)`.

```text
workspace_id
analysis_context_id
source_generation
owner_id
parent_owner_id optional
owner_bucket
owner_kind_code
language
file_id optional
semantic_entity_id optional
start_byte/end_byte optional
source_fingerprint optional
semantic_fingerprint optional
capability_mask
```

### 13.10 `capability_status`

**Primary key:** `(workspace_id, analysis_context_id, owner_id, capability_code)`.

```text
workspace_id
analysis_context_id
source_generation
snapshot_id optional
owner_id
owner_bucket
capability_code
owner_capability_state_code
completeness_state_code
provider_run_id optional
producer_code optional
reason_code optional
diagnostic_id optional
fallback_source_available
coverage_scope_fingerprint
```

### 13.11 `diagnostic`

Diagnostics include workspace/context/source-generation and a stable error/diagnostic code. Messages are safe display strings; detailed provider payloads are cold and access-controlled.

### 13.12 Operational read-only views

The SQLite operational store SHALL expose at least:

```text
common_repository_state
worktree_state
workspace_update_state
source_trust_state
update_wave
update_wave_item
provider_run
git_operation_run
hot_overlay_manifest
snapshot_lease
result_artifact_lease
```

Lifecycle owns their state transitions and retention. The fabric owns their Arrow schemas and consistent read-only DataFusion projection. They SHALL not be written through ordinary query SQL.

# Part IV — Universal Graph Tables

## 14. `entity`

**Grain:** one canonical CPG entity.
**Primary key:** `(workspace_id, analysis_context_id, entity_id)`.

| Column | Type | Null | Meaning |
|---|---|---:|---|
| `workspace_id` | `id16` | no | Source instance |
| `analysis_context_id` | `id16` | no | `context:source` or semantic context |
| `source_generation` | `Int64` | no | Generation fence |
| `entity_id` | `id16` | no | Canonical entity identity |
| `owner_id` | `id16` | no | Replacement owner |
| `owner_bucket` | `bucket16` | no | Physical partition |
| `language` | `code16` | no | Common/Python/Rust |
| `entity_family_code` | `code16` | no | Canonical family |
| `entity_kind_code` | `code32` | no | Ontology kind |
| `raw_kind_code` | `code32` | yes | Provider-native registry kind |
| `file_id` | `id16` | yes | Source file |
| `start_byte` | `Int64` | yes | Source start |
| `end_byte` | `Int64` | yes | Source end |
| `name` | `Utf8` | yes | Denormalized selected canonical name |
| `qualified_name` | `Utf8` | yes | Denormalized selected canonical qname |
| `parent_entity_id` | `id16` | yes | Immediate canonical parent |
| `type_id` | `id16` | yes | Denormalized selected computed type |
| `flags` | `Int64` | no | Canonical flags |
| `fact_hash64` | `Int64` | no | Equality/clustering accelerator |

Name/type/flags columns SHALL be generated from canonical property facts and SHALL not carry independent provenance.

---

## 15. `relation`

**Grain:** one canonical relation-shaped fact.
**Primary key:** `(workspace_id, analysis_context_id, fact_id)`.

| Column | Type | Null |
|---|---|---:|
| `workspace_id` | `id16` | no |
| `analysis_context_id` | `id16` | no |
| `source_generation` | `Int64` | no |
| `fact_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `language` | `code16` | no |
| `relation_family_code` | `code16` | no |
| `relation_kind_code` | `code32` | no |
| `source_id` | `id16` | no |
| `target_id` | `id16` | no |
| `source_bucket` | `bucket16` | no |
| `target_bucket` | `bucket16` | no |
| `ordinal` | `Int32` | yes |
| `role_code` | `code16` | yes |
| `distance` | `Int32` | yes |
| `directness_code` | `code16` | no |
| `file_id/start_byte/end_byte` | source reference | yes |
| `certainty_code` | `code16` | no |
| `resolution_code` | `code16` | no |
| `producer_code` | `code16` | no |
| `derivation_code` | `code16` | yes |
| `flags` | `Int64` | no |
| `fact_hash64` | `Int64` | no |

Relation endpoints SHALL belong to the same workspace and analysis context, except explicit external/unknown endpoint entities that are context-tagged within the same snapshot. Cross-context and cross-workspace exact edges are rejected.

---

## 16. First-class property facts and evidence

### 16.1 `property_fact`

**Grain:** one canonical property-shaped proposition.
**Primary key:** `(workspace_id, analysis_context_id, fact_id)`.

| Column | Type | Null |
|---|---|---:|
| `workspace_id` | `id16` | no |
| `analysis_context_id` | `id16` | no |
| `source_generation` | `Int64` | no |
| `fact_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `subject_entity_id` | `id16` | no |
| `property_kind_code` | `code32` | no |
| `program_point_entity_id` | `id16` | yes |
| `value_kind_code` | `code16` | no |
| `value_entity_id` | `id16` | yes |
| `value_bool` | `Boolean` | yes |
| `value_int64` | `Int64` | yes |
| `value_float64` | `Float64` | yes |
| `value_text` | `Utf8` | yes |
| `value_bytes` | `Binary` | yes |
| `value_type_id` | `id16` | yes |
| `directness_code` | `code16` | no |
| `certainty_code` | `code16` | no |
| `resolution_code` | `code16` | no |
| `producer_code` | `code16` | no |
| `derivation_code` | `code16` | yes |
| `file_id/start_byte/end_byte` | source reference | yes |
| `fact_hash64` | `Int64` | no |

Exactly one value representation SHALL be populated according to `value_kind_code`. Complex structures use entity references or typed extension tables rather than opaque JSON.

### 16.2 `fact_evidence`

**Grain:** one provider observation supporting or conflicting with a canonical relation/property fact.

```text
evidence_id
workspace_id
analysis_context_id
source_generation
fact_id
fact_form_code
owner_id/owner_bucket
provider_code/provider_version
provider_run_id
observation_id
raw_kind_code optional
file_id/start_byte/end_byte optional
certainty_code
resolution_code
conflict_disposition_code
cold_payload optional
```

`fact_id` is unambiguous because both relation and property tables use the same canonical fact-ID domain and `fact_form_code` is explicit.

---

## 17. `source_file`

**Grain:** one current source file.
**Primary key:** `(workspace_id, file_id)`.

| Column | Type | Null |
|---|---|---:|
| `workspace_id` | `id16` | no |
| `source_generation` | `Int64` | no |
| `file_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `path_bytes` | `Binary` | no |
| `path_display` | `Utf8` | no |
| `path_encoding_code` | `code16` | no |
| `path_case_key` | `Binary` | yes |
| `path_display_is_lossy` | `Boolean` | no |
| `language` | `code16` | no |
| `source_digest` | `hash32` | no |
| `byte_len` | `Int64` | no |
| `line_count` | `Int32` | no |
| `encoding_name` | `Utf8` | yes |
| `newline_kind_code` | `code16` | no |
| `source_bytes` | `Binary` | no |
| `decoded_text` | `Utf8` | yes |
| `line_start_offsets` | `List<Int64>` | no |
| `module_entity_id` | `id16` | yes |
| `is_stub` | `Boolean` | no |
| `flags` | `Int64` | no |

`source_bytes` and `path_bytes` are authoritative. Text/display fields are conveniences only.

## 18. `source_token`

**Grain:** one lexical token.

| Column | Type | Null |
|---|---|---:|
| `token_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `file_id` | `id16` | no |
| `ordinal` | `Int32` | no |
| `token_kind_code` | `code32` | no |
| `start_byte` | `Int64` | no |
| `end_byte` | `Int64` | no |
| `normalized_value` | `Utf8` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

Token text SHALL normally be recovered from `source_file` to avoid duplication.

## 19. `source_annotation`

**Grain:** one comment, documentation item, directive, pragma, parse error, or missing-syntax record.

| Column | Type | Null |
|---|---|---:|
| `annotation_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `file_id` | `id16` | no |
| `annotation_kind_code` | `code32` | no |
| `start_byte` | `Int64` | no |
| `end_byte` | `Int64` | no |
| `target_entity_id` | `id16` | yes |
| `text` | `Utf8` | yes |
| `diagnostic_code` | `code32` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 20. `syntax_detail`

**Grain:** one syntax entity extension keyed by `entity_id`.

| Column | Type | Null |
|---|---|---:|
| `entity_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `raw_kind_code` | `code32` | no |
| `normalized_kind_code` | `code32` | no |
| `parent_syntax_id` | `id16` | yes |
| `field_role_code` | `code16` | yes |
| `ordinal` | `Int32` | yes |
| `named` | `Boolean` | no |
| `extra` | `Boolean` | no |
| `error` | `Boolean` | no |
| `missing` | `Boolean` | no |
| `explicitly_parenthesized` | `Boolean` | no |
| `provider_node_flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

`AST_CHILD` relations SHALL be generated into `relation` from these parent/role/ordinal columns.

## 21. `semantic_detail`

**Grain:** one semantic declaration/symbol/member entity extension.

| Column | Type | Null |
|---|---|---:|
| `entity_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `semantic_kind_code` | `code32` | no |
| `visibility_code` | `code16` | yes |
| `mutability_code` | `code16` | yes |
| `declaration_syntax_id` | `id16` | yes |
| `name_span_start` | `Int64` | yes |
| `name_span_end` | `Int64` | yes |
| `signature_hash` | `hash32` | yes |
| `external` | `Boolean` | no |
| `generated` | `Boolean` | no |
| `synthesized` | `Boolean` | no |
| `modifiers` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 22. `scope_detail`

**Grain:** one scope entity extension.

| Column | Type | Null |
|---|---|---:|
| `scope_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `scope_kind_code` | `code16` | no |
| `parent_scope_id` | `id16` | yes |
| `semantic_entity_id` | `id16` | yes |
| `file_id` | `id16` | yes |
| `start_byte` | `Int64` | yes |
| `end_byte` | `Int64` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 23. `binding_detail`

**Grain:** one binding entity extension.

| Column | Type | Null |
|---|---|---:|
| `binding_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `scope_id` | `id16` | no |
| `bound_entity_id` | `id16` | yes |
| `binding_kind_code` | `code16` | no |
| `name` | `Utf8` | no |
| `definition_event_id` | `id16` | yes |
| `target_scope_id` | `id16` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 24. `reference_detail`

**Grain:** one reference entity extension.

| Column | Type | Null |
|---|---|---:|
| `reference_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `scope_id` | `id16` | no |
| `reference_kind_code` | `code16` | no |
| `name` | `Utf8` | no |
| `resolved_entity_id` | `id16` | yes |
| `candidate_count` | `Int32` | no |
| `unknown_reason_code` | `code16` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 25. `module_import_detail`

**Grain:** one import/use occurrence.

| Column | Type | Null |
|---|---|---:|
| `import_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `source_module_id` | `id16` | no |
| `target_module_id` | `id16` | yes |
| `imported_entity_id` | `id16` | yes |
| `local_binding_id` | `id16` | yes |
| `import_kind_code` | `code16` | no |
| `relative_level` | `Int16` | yes |
| `source_name` | `Utf8` | no |
| `alias_name` | `Utf8` | yes |
| `star_import` | `Boolean` | no |
| `unknown_reason_code` | `code16` | yes |

**Partitioning:** `owner_bucket`.

---

# Part VI — Types, Members, Calls, and Control Flow

## 26. `type_detail`

**Grain:** one canonical semantic type entity.

| Column | Type | Null |
|---|---|---:|
| `type_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `type_kind_code` | `code32` | no |
| `canonical_key` | `Utf8` | no |
| `display_name` | `Utf8` | yes |
| `primitive_code` | `code16` | yes |
| `nominal_entity_id` | `id16` | yes |
| `callable_entity_id` | `id16` | yes |
| `raw_shape_hash` | `hash32` | yes |
| `nullable_semantics_code` | `code16` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `(type_kind_code, owner_bucket)`.

Type algebra components and relationships such as union members, generic arguments, parameters, bounds, coercions, and narrowing SHALL be rows in `relation` using the `TYPE` family.

## 27. `type_fact_detail`

**Grain:** one subject/type attribution relation extension.

| Column | Type | Null |
|---|---|---:|
| `relation_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `subject_id` | `id16` | no |
| `type_id` | `id16` | no |
| `type_role_code` | `code16` | no |
| `program_point_id` | `id16` | yes |
| `origin_code` | `code16` | no |
| `certainty_code` | `code16` | no |

**Partitioning:** `owner_bucket`.

## 28. `member_relation_detail`

**Grain:** one member/inheritance/implementation/override relation extension.

| Column | Type | Null |
|---|---|---:|
| `relation_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `declaring_type_id` | `id16` | yes |
| `member_entity_id` | `id16` | yes |
| `contract_member_id` | `id16` | yes |
| `receiver_type_id` | `id16` | yes |
| `resolution_kind_code` | `code16` | yes |
| `mro_position` | `Int32` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 29. `callable_detail`

**Grain:** one callable semantic entity extension.

| Column | Type | Null |
|---|---|---:|
| `callable_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `signature_id` | `id16` | yes |
| `return_type_id` | `id16` | yes |
| `parameter_count` | `Int32` | no |
| `generic_parameter_count` | `Int32` | no |
| `calling_convention_code` | `code16` | yes |
| `abi_name` | `Utf8` | yes |
| `callable_flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 30. `parameter_detail`

**Grain:** one callable parameter.

| Column | Type | Null |
|---|---|---:|
| `parameter_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `callable_id` | `id16` | no |
| `ordinal` | `Int32` | no |
| `name` | `Utf8` | yes |
| `parameter_kind_code` | `code16` | no |
| `type_id` | `id16` | yes |
| `default_syntax_id` | `id16` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 31. `call_site_detail`

**Grain:** one call-site entity extension.

| Column | Type | Null |
|---|---|---:|
| `call_site_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `caller_id` | `id16` | no |
| `syntax_id` | `id16` | yes |
| `callee_syntax_id` | `id16` | yes |
| `receiver_value_id` | `id16` | yes |
| `result_value_id` | `id16` | yes |
| `dispatch_kind_code` | `code16` | no |
| `declared_target_id` | `id16` | yes |
| `resolved_target_count` | `Int32` | no |
| `call_flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 32. `call_argument_detail`

**Grain:** one argument occurrence at one call site.

| Column | Type | Null |
|---|---|---:|
| `argument_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `call_site_id` | `id16` | no |
| `ordinal` | `Int32` | no |
| `keyword_name` | `Utf8` | yes |
| `argument_syntax_id` | `id16` | yes |
| `argument_value_id` | `id16` | yes |
| `parameter_id` | `id16` | yes |
| `binding_status_code` | `code16` | no |
| `spread_kind_code` | `code16` | yes |

**Partitioning:** `owner_bucket`.

## 33. `call_target_detail`

**Grain:** one target candidate for one call site.
**Primary key:** `(call_site_id, target_id, target_instance_id, target_kind_code)`.

| Column | Type | Null |
|---|---|---:|
| `call_site_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `target_id` | `id16` | no |
| `target_instance_id` | `id16` | yes |
| `target_kind_code` | `code16` | no |
| `exact` | `Boolean` | no |
| `certainty_code` | `code16` | no |
| `evidence_relation_id` | `id16` | no |

**Partitioning:** `owner_bucket`.

## 34. `cfg_graph`

**Grain:** one control-flow graph.

| Column | Type | Null |
|---|---|---:|
| `cfg_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `callable_id` | `id16` | yes |
| `cfg_kind_code` | `code16` | no |
| `entry_node_id` | `id16` | no |
| `exit_node_id` | `id16` | no |
| `exceptional_exit_node_id` | `id16` | yes |
| `node_count` | `Int32` | no |
| `edge_count` | `Int32` | no |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 35. `cfg_node_detail`

**Grain:** one CFG node entity extension.

| Column | Type | Null |
|---|---|---:|
| `cfg_node_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `cfg_id` | `id16` | no |
| `node_kind_code` | `code16` | no |
| `syntax_id` | `id16` | yes |
| `mir_statement_id` | `id16` | yes |
| `ordinal` | `Int32` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 36. `cfg_edge_detail`

**Grain:** one CFG relation extension keyed by the corresponding `relation_id`.

| Column | Type | Null |
|---|---|---:|
| `relation_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `cfg_id` | `id16` | no |
| `condition_id` | `id16` | yes |
| `case_value_text` | `Utf8` | yes |
| `case_value_hash` | `Int64` | yes |
| `exception_type_id` | `id16` | yes |
| `edge_flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

---

# Part VII — Values, Dataflow, Memory, and State

## 37. `value_detail`

**Grain:** one value entity extension.

| Column | Type | Null |
|---|---|---:|
| `value_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `value_kind_code` | `code16` | no |
| `type_id` | `id16` | yes |
| `producer_operation_id` | `id16` | yes |
| `constant_value_id` | `id16` | yes |
| `syntax_id` | `id16` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 38. `operation_detail`

**Grain:** one normalized computation operation.

| Column | Type | Null |
|---|---|---:|
| `operation_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `cfg_node_id` | `id16` | yes |
| `operation_kind_code` | `code32` | no |
| `result_value_id` | `id16` | yes |
| `type_id` | `id16` | yes |
| `syntax_id` | `id16` | yes |
| `raw_kind_code` | `code32` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

Operands SHALL be `relation` rows of kind `OPERAND` with `ordinal` and `role_code`.

## 39. `dataflow_event_detail`

**Grain:** one definition, use, read, write, move, copy, borrow, or related event.

| Column | Type | Null |
|---|---|---:|
| `event_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `cfg_node_id` | `id16` | yes |
| `event_kind_code` | `code16` | no |
| `binding_id` | `id16` | yes |
| `value_id` | `id16` | yes |
| `location_id` | `id16` | yes |
| `syntax_id` | `id16` | yes |
| `ordinal` | `Int32` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

Reaching-definitions, def-use, data-dependency, and value-flow outputs SHALL be `relation` rows in the `DATAFLOW` family.

## 40. `memory_location_detail`

**Grain:** one canonical abstract memory/access-path location.

| Column | Type | Null |
|---|---|---:|
| `location_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `location_kind_code` | `code16` | no |
| `base_entity_id` | `id16` | yes |
| `base_local_id` | `id16` | yes |
| `type_id` | `id16` | yes |
| `parent_location_id` | `id16` | yes |
| `projection_depth` | `Int16` | no |
| `canonical_path_hash` | `hash32` | no |
| `display_path` | `Utf8` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 41. `access_path_component`

**Grain:** one ordered projection component of one memory location.

| Column | Type | Null |
|---|---|---:|
| `component_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `location_id` | `id16` | no |
| `ordinal` | `Int16` | no |
| `projection_kind_code` | `code16` | no |
| `field_entity_id` | `id16` | yes |
| `index_value_id` | `id16` | yes |
| `variant_entity_id` | `id16` | yes |
| `constant_index` | `Int64` | yes |
| `subslice_from` | `Int64` | yes |
| `subslice_to` | `Int64` | yes |

**Partitioning:** `owner_bucket`.

## 42. `memory_access_detail`

**Grain:** one access event over one location.

| Column | Type | Null |
|---|---|---:|
| `access_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `cfg_node_id` | `id16` | yes |
| `location_id` | `id16` | no |
| `value_id` | `id16` | yes |
| `access_kind_code` | `code16` | no |
| `program_point_id` | `id16` | yes |
| `certainty_code` | `code16` | no |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

Alias and points-to facts SHALL be `relation` rows in the `MEMORY_ALIAS` family. `alias_relation_detail` MAY store program-point and analysis-domain payloads when needed.

## 43. `program_state_detail`

**Grain:** one objective state fact for one subject at one program point.

| Column | Type | Null |
|---|---|---:|
| `state_fact_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `subject_id` | `id16` | no |
| `program_point_id` | `id16` | no |
| `state_kind_code` | `code16` | no |
| `state_value_code` | `code16` | no |
| `related_id` | `id16` | yes |
| `certainty_code` | `code16` | no |

**Partitioning:** `owner_bucket`.

---

# Part VIII — Effects, Exceptions, Resources, Async, and Generated Semantics

## 44. `effect_detail`

**Grain:** one direct or transitive callable effect.

| Column | Type | Null |
|---|---|---:|
| `effect_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `callable_id` | `id16` | no |
| `effect_kind_code` | `code16` | no |
| `direct` | `Boolean` | no |
| `target_id` | `id16` | yes |
| `source_call_site_id` | `id16` | yes |
| `certainty_code` | `code16` | no |
| `unknown` | `Boolean` | no |
| `model_pack_code` | `code16` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `(effect_kind_code, owner_bucket)`.

The corresponding semantic relation SHALL also exist in `relation` when the effect has a target entity/location.

## 45. `exception_detail`

**Grain:** one raise/panic/assert/handler/unwind semantic event or relation payload.

| Column | Type | Null |
|---|---|---:|
| `exception_fact_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `site_id` | `id16` | no |
| `cfg_node_id` | `id16` | yes |
| `exception_kind_code` | `code16` | no |
| `exception_type_id` | `id16` | yes |
| `handler_id` | `id16` | yes |
| `relation_kind_code` | `code16` | no |
| `certainty_code` | `code16` | no |

**Partitioning:** `owner_bucket`.

## 46. `resource_event_detail`

**Grain:** one resource lifecycle event.

| Column | Type | Null |
|---|---|---:|
| `resource_event_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `cfg_node_id` | `id16` | yes |
| `resource_kind_code` | `code16` | no |
| `resource_id` | `id16` | yes |
| `location_id` | `id16` | yes |
| `event_kind_code` | `code16` | no |
| `transfer_target_id` | `id16` | yes |
| `model_pack_code` | `code16` | yes |
| `certainty_code` | `code16` | no |

**Partitioning:** `owner_bucket`.

## 47. `async_event_detail`

**Grain:** one async/concurrency relation payload.

| Column | Type | Null |
|---|---|---:|
| `async_event_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `cfg_node_id` | `id16` | yes |
| `concurrency_kind_code` | `code16` | no |
| `subject_id` | `id16` | no |
| `target_id` | `id16` | yes |
| `relation_kind_code` | `code16` | no |
| `certainty_code` | `code16` | no |
| `model_pack_code` | `code16` | yes |

**Partitioning:** `owner_bucket`.

## 48. `capture_detail`

**Grain:** one closure-capture fact.

| Column | Type | Null |
|---|---|---:|
| `capture_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `closure_id` | `id16` | no |
| `captured_entity_id` | `id16` | yes |
| `captured_location_id` | `id16` | yes |
| `source_scope_id` | `id16` | yes |
| `capture_kind_code` | `code16` | no |
| `ordinal` | `Int32` | yes |

**Partitioning:** `owner_bucket`.

## 49. `generated_detail`

**Grain:** one generated/lowered entity or expansion record.

| Column | Type | Null |
|---|---|---:|
| `generated_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `generated_kind_code` | `code16` | no |
| `source_entity_id` | `id16` | yes |
| `source_syntax_id` | `id16` | yes |
| `expansion_id` | `id16` | yes |
| `generation_depth` | `Int16` | yes |
| `provenance_code` | `code16` | no |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

Generated/lowered relationships SHALL also be canonical rows in `relation`.

---

# Part IX — Python and Rust Extension Tables

## 50. `python_dynamic_detail`

**Grain:** one Python dynamic-semantics observation.

| Column | Type | Null |
|---|---|---:|
| `dynamic_fact_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `subject_id` | `id16` | no |
| `dynamic_kind_code` | `code16` | no |
| `target_name` | `Utf8` | yes |
| `target_value_id` | `id16` | yes |
| `unknown_entity_id` | `id16` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

This table covers objective facts such as use of `eval`, `exec`, dynamic imports, `getattr`, `setattr`, `__dict__`, star imports, monkey-patch writes, and dynamic attribute writes.

## 51. `rust_mir_body`

**Grain:** one MIR body.

| Column | Type | Null |
|---|---|---:|
| `body_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `definition_entity_id` | `id16` | no |
| `mir_phase_code` | `code16` | no |
| `return_local_id` | `id16` | no |
| `argument_count` | `Int32` | no |
| `local_count` | `Int32` | no |
| `basic_block_count` | `Int32` | no |
| `source_span_start` | `Int64` | yes |
| `source_span_end` | `Int64` | yes |
| `mir_fingerprint` | `hash32` | no |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 52. `rust_mir_local`

**Grain:** one MIR local.

| Column | Type | Null |
|---|---|---:|
| `local_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `body_id` | `id16` | no |
| `ordinal` | `Int32` | no |
| `local_kind_code` | `code16` | no |
| `debug_name` | `Utf8` | yes |
| `type_id` | `id16` | no |
| `mutability_code` | `code16` | no |
| `source_start` | `Int64` | yes |
| `source_end` | `Int64` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 53. `rust_instance`

**Grain:** one concrete executable Rust instance.

| Column | Type | Null |
|---|---|---:|
| `instance_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `definition_entity_id` | `id16` | no |
| `instance_kind_code` | `code16` | no |
| `body_id` | `id16` | yes |
| `abi_name` | `Utf8` | yes |
| `mangled_name` | `Utf8` | yes |
| `generic_argument_count` | `Int32` | no |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

Generic arguments are `relation` rows from instance to type/lifetime/const argument entities with ordinals.

## 54. `rust_loan`

**Grain:** one compiler-exposed or conservatively derived loan.

| Column | Type | Null |
|---|---|---:|
| `loan_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `body_id` | `id16` | no |
| `place_id` | `id16` | no |
| `loan_kind_code` | `code16` | no |
| `created_at_node_id` | `id16` | no |
| `region_id` | `id16` | yes |
| `borrowed_type_id` | `id16` | yes |
| `certainty_code` | `code16` | no |

**Partitioning:** `owner_bucket`.

## 55. `rust_region`

**Grain:** one Rust region/lifetime entity extension.

| Column | Type | Null |
|---|---|---:|
| `region_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `body_id` | `id16` | yes |
| `region_kind_code` | `code16` | no |
| `display_name` | `Utf8` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 56. `rust_move_path`

**Grain:** one move-path node.

| Column | Type | Null |
|---|---|---:|
| `move_path_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `body_id` | `id16` | no |
| `place_id` | `id16` | no |
| `parent_move_path_id` | `id16` | yes |
| `ordinal` | `Int32` | no |

**Partitioning:** `owner_bucket`.

## 57. `rust_vtable_entry`

**Grain:** one vtable entry candidate.

| Column | Type | Null |
|---|---|---:|
| `vtable_entry_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `vtable_id` | `id16` | no |
| `dyn_type_id` | `id16` | no |
| `concrete_type_id` | `id16` | yes |
| `ordinal` | `Int32` | no |
| `target_instance_id` | `id16` | yes |
| `entry_kind_code` | `code16` | no |
| `certainty_code` | `code16` | no |

**Partitioning:** `owner_bucket`.

## 58. `rust_macro_expansion`

**Grain:** one Rust macro expansion.

| Column | Type | Null |
|---|---|---:|
| `expansion_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `invocation_syntax_id` | `id16` | no |
| `macro_definition_id` | `id16` | yes |
| `expansion_depth` | `Int16` | no |
| `callsite_file_id` | `id16` | yes |
| `callsite_start` | `Int64` | yes |
| `callsite_end` | `Int64` | yes |
| `defsite_file_id` | `id16` | yes |
| `defsite_start` | `Int64` | yes |
| `defsite_end` | `Int64` | yes |
| `hygiene_context_hash` | `Int64` | yes |

**Partitioning:** `owner_bucket`.

---

# Part X — Unknowns, Derived Components, Metrics, and Summaries

## 59. `unknown_detail`

**Grain:** one explicit unknown entity.

| Column | Type | Null |
|---|---|---:|
| `unknown_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `unknown_kind_code` | `code16` | no |
| `subject_id` | `id16` | yes |
| `expected_relation_kind_code` | `code32` | yes |
| `reason_code` | `code16` | no |
| `provider_code` | `code16` | yes |
| `diagnostic_id` | `id16` | yes |
| `detail` | `Utf8` | yes |

**Partitioning:** `owner_bucket`.

Unknown nodes SHALL also exist in `entity`; edges to them SHALL exist in `relation`.

## 60. `derived_component`

**Grain:** one SCC, connected component, loop, recursive set, or other graph component.

| Column | Type | Null |
|---|---|---:|
| `component_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `projection_code` | `code16` | no |
| `component_kind_code` | `code16` | no |
| `size` | `Int64` | no |
| `header_or_root_id` | `id16` | yes |
| `recursive` | `Boolean` | no |
| `nesting_depth` | `Int32` | yes |
| `derivation_code` | `code16` | no |

**Partitioning:** `(projection_code, owner_bucket)`.

Membership SHALL be `relation` rows in the `DERIVED_GRAPH` family.

## 61. `metric`

**Grain:** one scalar objective metric for one subject.

| Column | Type | Null |
|---|---|---:|
| `metric_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `subject_id` | `id16` | no |
| `metric_code` | `code16` | no |
| `int_value` | `Int64` | yes |
| `float_value` | `Float64` | yes |
| `derivation_code` | `code16` | no |
| `flags` | `Int64` | no |

**Partitioning:** `(metric_code, owner_bucket)`.

Only objective measurements are permitted.

## 62. `callable_summary`

**Grain:** one scalar summary row per callable or Rust instance.

| Column | Type | Null |
|---|---|---:|
| `callable_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `instance_id` | `id16` | yes |
| `direct_callee_count` | `Int64` | no |
| `may_callee_count` | `Int64` | no |
| `direct_read_count` | `Int64` | no |
| `transitive_read_count` | `Int64` | no |
| `direct_write_count` | `Int64` | no |
| `transitive_write_count` | `Int64` | no |
| `return_type_count` | `Int64` | no |
| `summary_flags` | `Int64` | no |
| `unknown_effect` | `Boolean` | no |
| `derivation_code` | `code16` | no |
| `summary_fingerprint` | `hash32` | no |

**Partitioning:** `owner_bucket`.

Actual read/write/call/effect member sets SHALL remain typed relations/effect rows rather than opaque lists. Optional cached `id_list` columns MAY be added only as a serving accelerator.

---

# Part XI — Arrow Ingestion and Batch Construction

## 63. Provider-observation to Arrow contract

The canonical data-fabric boundary accepts only validated Arrow `RecordBatch` streams conforming to registered **observation schemas**. Provider adapters MAY use bounded row DTO chunks internally, but row DTOs are not retained alongside Arrow after handoff.

Required stream properties:

```text
workspace/context/source-generation manifest precedes batches
bounded batch row/byte sizes
schema fingerprint on every stream
backpressure through bounded async channels
terminal completed/partial/failed manifest
no provider-native object lifetimes crossing the boundary
```

The `ReconciliationEngine` consumes entity/relation/property observation streams and emits canonical owner-scoped Arrow batches for `entity`, `relation`, `property_fact`, evidence, capabilities, and typed extensions.

## 64. Batch-size policy

Starting values:

```text
small/wide extension tables      16,384 rows per RecordBatch
normal fact tables               65,536 rows per RecordBatch
narrow relation/event tables    131,072 rows per RecordBatch
source_file                       bounded by file count, not bytes
```

Batch size SHALL be benchmarked against:

- Arrow builder allocation;
- DataFusion batch overhead;
- Parquet row-group formation;
- owner-local replacement size;
- memory pool limits.

## 65. Builder policy

Hot-path encoding SHALL use typed Arrow builders and preallocation.

```text
PrimitiveBuilder::with_capacity
BinaryBuilder::with_capacity
StringBuilder::with_capacity
ListBuilder with child capacity
StructArray construction from typed child arrays
```

Serde row conversion SHALL NOT be the primary high-volume path.

### 65.1 Null policy

Null SHALL mean semantically unavailable or inapplicable, not merely “not populated by this producer.”

Missing provider evidence SHALL usually be represented through:

- certainty/resolution codes;
- capability status;
- explicit unknown entities;
- fact evidence.

### 65.2 String policy

Persisted schemas SHALL use `Utf8`, not `Utf8View`, because Delta/Parquet table contracts must remain stable. DataFusion MAY use view types internally during query execution.

### 65.3 Dictionary policy

Repeated strings MAY be dictionary-encoded in transient Arrow batches, but durable Delta schemas SHALL remain semantic `STRING` columns. Parquet writer dictionary encoding SHALL be preferred over making dictionary types part of the table contract.

### 65.4 Nested-type policy

`Struct`, `List`, and `Map` SHALL be used for bounded, cohesive payloads such as:

- line offsets;
- cold diagnostics metadata;
- optional cached summary sets.

Core graph adjacency, type components, arguments, access-path components, and provider evidence SHALL remain row-oriented relations for pushdown and joins.

---

## 66. Batch validation

Every batch SHALL pass before entering DataFusion or Delta:

```text
schema exact match
column count match
row count equal across arrays
non-null key enforcement
id length enforcement
bucket derivation check
source span bounds
start <= end
registered enum codes
owner_id present
no duplicate primary key within batch
```

Validation SHALL use Arrow kernels where possible and custom vectorized validators otherwise.

---

# Part XII — Delta Table Creation and Write Operations

## 67. Delta table creation

Each table SHALL be created from the schema registry using validated `StructType::try_new(...)` schemas.

Creation SHALL set:

- table comment/description;
- schema version metadata;
- ontology version metadata;
- partition columns;
- target-file-size property where supported;
- table constraints;
- log/checkpoint retention policy;
- CDF disabled by default.

### 67.1 Column mapping

Default:

```text
delta.columnMapping.mode = none
```

Column mapping SHALL not be enabled unless all reader, writer, DML, CDF, optimize, and schema-evolution paths are compatibility-certified.

### 67.2 Type widening

Delta type widening SHALL be disabled by default. Schema migrations SHALL be explicit and tested across Arrow, DataFusion, and Delta.

### 67.3 Table-feature compatibility and `V2Checkpoint`

The pinned delta-rs protocol checker recognizes `V2Checkpoint` as both a reader and a writer feature. The table-feature compatibility registry SHALL be updated accordingly, so that a table declaring `V2Checkpoint` is not rejected merely because the selected delta-rs build was assumed not to understand it.

Recognition is not adoption. The policy is:

```text
V2Checkpoint read compatibility                 ALLOWED
V2Checkpoint existing-table write compatibility ALLOWED_BY_LIBRARY
CodeFabric create/enable by default             NO
CodeFabric maintenance rollout                  BENCHMARK_AND_CONFORMANCE_REQUIRED
```

CodeFabric-owned tables SHALL continue to use the existing checkpoint policy. Protocol-checker recognition is not a designed checkpoint maintenance policy; the current baseline does not need V2 checkpoints for correctness; and durable publication identity is table-version based and independent of checkpoint format (section 12.8).

Features that remain unsupported — including identity columns and type widening — SHALL continue to fail closed.

---

## 68. Table mutation classes

Each physical durable table SHALL be assigned one `DurableMutationClass`.
This class governs durable writes only; it does not determine overlay or
catalog behavior.

| Class | Tables | Default operation |
|---|---|---|
| Static dimension | enum catalogs, provider registry | append-only / replace at bootstrap |
| Current singleton | repository, current_publication | merge/upsert one row |
| Owner-replaced fact | entity, relation, almost all extensions | delete owner rows then append replacement |
| Publication append | publication, publication_table | append then pointer update |
| Derived owner-replaced | metrics, summaries, owner-local derived facts | delete owner rows then append |
| Global derived replacement | global call SCC / closure if materialized | bounded table overwrite before publication |

Full-table overwrite SHALL be limited to initial bootstrap, controlled schema migration, or small global derived tables.

---

## 69. Owner replacement protocol

The default owner replacement for one physical table is:

```text
1. Open table at latest writable state.
2. Delete rows whose owner_id is in the replacement owner set.
3. Append validated replacement RecordBatch stream.
4. Reload table state.
5. Validate owner row counts/checksum.
6. Record final Delta version in publication_table.
```

The delete and append are separate Delta commits. This is safe because the publication pointer continues to reference the old table version until the complete new publication is validated.

### 69.1 MERGE optimization

`DeltaTable::merge` MAY replace delete+append when:

- the table has a stable primary key;
- source cardinality is large enough to justify merge planning;
- delete-not-matched semantics are verified for the pinned delta-rs API;
- execution plans and DML metrics are regression-tested.

Delete+append remains the normative baseline because its visibility is controlled by the publication manifest.

### 69.2 Removed owners

An owner removed from the current source SHALL be represented by deleting all rows for that `owner_id` from every owner-scoped table and omitting it from the replacement batches.

---

## 70. Idempotency and retry

Every write operation SHALL carry:

```text
publication_id
operation_id
table_code
owner-set fingerprint
input checksum
```

in Delta commit metadata where supported.

At the pinned delta-rs revision, CodeFabric SHALL attach that metadata with
`CommitProperties::with_metadata` and SHALL use a Delta application transaction
for every retryable table commit. The application transaction identity is:

```text
app_id = "codefabric/<workspace_id>/<table_code>/<mutation_phase>"
version = coordinator-persisted monotonically increasing signed 64-bit sequence
```

`mutation_phase` distinguishes independently retryable commits such as
`owner-delete`, `owner-append`, `singleton-upsert`, `publication-append`, and
maintenance. The operational write record binds `operation_id` to
`(app_id, version, input checksum, expected output checksum)` before the Delta
commit. The commit uses
`CommitProperties::with_application_transaction(Transaction::new(app_id,
version))`. This is the primary per-table duplicate/conflict marker; it does not
replace CodeFabric's multi-table publication and pointer protocols.

Retry logic SHALL:

1. reload latest table state;
2. read the latest Delta application-transaction version for the operation's
   `app_id`;
3. reconcile that version with the persisted operation record and commit
   metadata;
4. validate expected rows/checksum when the version is equal or has advanced;
5. retry only when the prior outcome is known not to have committed.

Blind append retry is prohibited.

---

## 71. Durable publication and active-snapshot algorithm

### 71.1 Durable base publication

1. create publication row in `STAGING` for one `workspace_id`;
2. pin source generation, inventory digest, context set, Git/inclusion fingerprints, and all bundle/toolchain versions;
3. write affected owner replacements to Delta tables;
4. record exact table versions/checksums;
5. run schema, referential, capability, and checksum validation;
6. transition through `VALIDATING` → `VALIDATED` → `COMMITTING` → `COMPLETE`;
7. compare-and-swap `current_publication[workspace_id]` last;
8. on failure mark `FAILED` or `ABANDONED`; never expose intermediate table versions through serving views.

### 71.2 Interactive ServingSnapshot activation

Hot activation is separate:

1. reconcile and derive owner/table changes into an immutable consolidated overlay;
2. construct a complete `ServingSnapshotManifest` over the current durable base;
3. validate overlay schemas, tombstones, capabilities, source/context fences, and checksums;
4. transition serving activation `BUILDING` → `VALIDATING` → `READY`;
5. atomically compare-and-swap `active_snapshot[workspace_id]` to `ACTIVE`;
6. retain the prior snapshot until leases release;
7. asynchronously flush/rebase the overlay into a later durable publication.

A durable publication MAY become the new base without being the current interactive snapshot until the overlay is rebased and a new snapshot pointer is activated.

## 72. Internal planning policy and single reconciliation authority

The daemon data-fabric `ReconciliationEngine` is the sole canonical reconciliation owner. It consumes normalized observation Arrow streams, applies ontology/provider precedence, emits canonical entity/relation/property/evidence batches, and records conflicts. Fact-generation adapters and lifecycle scheduling SHALL not independently canonicalize or deduplicate facts.

DataFusion `Expr`, `LogicalPlanBuilder`, joins, windows, and custom operators MAY implement reconciliation plan families, but the module/API boundary and output schemas are those of `ReconciliationEngine`.

## 73. Reconciliation plan families

### 73.1 Source-range reconciliation

DataFusion joins provider observations using:

```text
file_id
range overlap or exact range
normalized kind
parent role
owner
```

The highest-authority observation becomes canonical; all others become `fact_evidence`.

### 73.2 Declaration reconciliation

```text
syntax declaration candidate
  JOIN local semantic binding
  LEFT JOIN project/compiler semantic declaration
  GROUP BY canonical semantic key
  → semantic entity
```

### 73.3 Type reconciliation

```text
declared type
computed type
expected type
flow-narrowed type
```

remain separate type-fact relations. Canonical type nodes are deduplicated by `type_id`.

### 73.4 Call-target reconciliation

Exact targets, may-targets, declaration targets, and unknown targets SHALL remain distinct rows. No aggregate shall collapse them into one unqualified `CALLS` relationship.

### 73.5 Unknown materialization

A DataFusion anti-join SHALL identify required semantic relationships with no resolved target and generate explicit unknown entities and relations according to the companion specification.

---

## 74. Canonical deduplication

Deduplication SHALL use deterministic primary keys and `row_number()` over authority order where multiple canonical candidates remain.

Authority ordering SHALL be encoded as integer rank and SHALL be stable across releases unless the ontology version changes.

Canonical dedupe plans SHALL sort only when required; hash aggregation and partition-local dedupe are preferred.

---

## 75. Integrity validation queries

Before publication, DataFusion SHALL verify at minimum:

```text
primary-key uniqueness for every table
entity IDs are 16 bytes
relation source and target exist in entity
owner IDs exist in owner
file spans lie within source_file.byte_len
start_byte <= end_byte
syntax parent is a syntax entity
call target points to callable/instance/unknown target
CFG edge endpoints belong to same cfg_id
CFG entry/exit nodes exist
dataflow events refer to existing CFG/value/location entities
access-path ordinals are contiguous per location
type relations point to type entities where required
unknown relations point to matching unknown kinds
summary derivation version matches publication derivation bundle
publication row counts match table scans
```

Foreign-key-like checks are application-enforced; Delta does not substitute for these joins.

---

# Part XIV — Calculations and Derived-Fact Execution

## 76. Calculation-placement policy

Use the highest-level DataFusion surface that preserves optimizer visibility.

```text
built-in Expr / aggregate
  before custom UDF

UDF / UDAF / UDTF
  before custom logical/physical operator

custom physical operator
  only for graph/fixed-point algorithms not naturally relational
```

---

## 77. Arrow kernel catalog

The fabric SHALL implement vectorized Arrow kernels for:

```text
validate_id16(binary) -> boolean
id_bucket(binary) -> int16
id_hash64(binary) -> int64
id_to_hex(binary) -> utf8
span_length(start, end) -> int64
validate_span(start, end, file_len) -> boolean
flags_has(flags, mask) -> boolean
flags_or(array<int64>) -> int64
canonical_path_hash(base, projections...) -> binary32
fact_row_hash(selected columns...) -> int64
fact_checksum_update(batch) -> binary state
sorted_unique_id_list(list<binary>) -> list<binary>
```

Kernels SHALL operate on arrays and preserve null semantics explicitly.

---

## 78. DataFusion scalar UDFs

Recommended registered UDFs:

| Function | Signature | Purpose |
|---|---|---|
| `cpg_id_bucket` | `BINARY -> SMALLINT` | Add mandatory bucket filter |
| `cpg_id_hash64` | `BINARY -> BIGINT` | Z-order/statistics accelerator |
| `cpg_id_hex` | `BINARY -> STRING` | Human-readable output |
| `cpg_span_len` | `(BIGINT,BIGINT) -> BIGINT` | Span metric |
| `cpg_flags_has` | `(BIGINT,BIGINT) -> BOOLEAN` | Flag filtering |
| `cpg_source_slice` | `(BINARY,BIGINT,BIGINT) -> STRING/BINARY` | Exact source snippet |
| `cpg_relation_family_name` | `SMALLINT -> STRING` | Serving display |
| `cpg_entity_kind_name` | `INTEGER -> STRING` | Serving display |

These UDFs SHALL be immutable and deterministic.

---

## 79. DataFusion aggregate UDFs

Recommended UDAFs:

### 79.1 `cpg_id_set_union`

```text
input: BINARY or LIST<BINARY>
state: sorted/deduplicated LIST<BINARY>
output: LIST<BINARY>
```

Used for bounded summaries and fixed-point propagation.

### 79.2 `cpg_fact_checksum`

```text
input: deterministic row hash
state: order-independent multiset checksum + row count
output: BINARY(32)
```

Used for table and owner validation.

### 79.3 `cpg_flags_or`

Bitwise union of effect and summary flags.

UDAF states SHALL be mergeable, deterministic, serializable in Arrow, and memory-accounted.

---

## 79A. Derivation registry and single-authority matrix

Every materialized derived family SHALL have exactly one `DerivationRegistry` entry:

```text
derivation_id and version
input fact families and required completeness
analysis context and precision profile
implementation owner
materialization mode
output table/fact kinds
unknown propagation and convergence policy
```

Default 1.3 authority matrix:

| Derived family | Authoritative implementation | Materialization |
|---|---|---|
| owner-local CFG SCCs, dominators, post-dominators, loops | custom Rust/petgraph over `GraphProjectionDto` | hot + durable owner facts |
| owner-local reaching definitions and liveness | custom Rust dataflow solver | hot + durable owner facts |
| workspace call/dependency reachability and SCCs | DataFusion custom CSR operators | materialized/cached by registry profile |
| points-to and alias fixed point | custom Rust fixed-point engine with Arrow input/output | hot + durable selected profile |
| interprocedural summaries | DataFusion/custom CSR summary fixpoint | hot + durable summaries |
| ad hoc bounded paths not materialized above | DataFusion query-time graph operator | query result only |

Sections 80–90 describe algorithms and physical options under this matrix. They SHALL not be interpreted as authorizing a second implementation to publish the same derivation/profile. Petgraph and DataFusion positions are ephemeral; canonical facts retain application IDs and supporting fact IDs.

## 80. Relationally expressible derived facts

The following SHALL use ordinary DataFusion plans:

```text
direct caller/callee projections
direct in/out degree
unique callee/caller counts
entity/relation family counts
owner fact counts
branch/return/read/write counts
cyclomatic complexity from CFG nodes/edges
unknown counts
exact-vs-may target counts
summary scalar flags and counts
source span lengths
member/type/call lookup views
```

Example cyclomatic calculation:

```text
M = E - N + 2P
```

where `E` is CFG edge count, `N` is CFG node count, and `P` is connected-component count for the selected CFG policy.

---

## 81. Custom logical operators

The fabric SHALL define application-owned logical nodes for nontrivial graph computations.

```text
CpgGraphTraverse
CpgStrongComponents
CpgDominators
CpgPostDominators
CpgControlDependence
CpgNaturalLoops
CpgReachingDefinitions
CpgLiveness
CpgPointsTo
CpgSummaryFixpoint
```

Each logical node SHALL expose:

- input plans;
- input expressions;
- deterministic output schema;
- graph scope keys;
- relation-kind filters;
- certainty policy;
- maximum depth/iteration policy;
- display/EXPLAIN representation.

---

## 82. Custom physical graph representation

Inside graph execution operators, global `id16` values SHALL be mapped to dense local `u32` indexes per graph scope.

Recommended in-memory CSR representation:

```text
node_ids:          BinaryArray / FixedSizeBinary-compatible temporary buffer
row_offsets:       UInt64Buffer, length N + 1
neighbors:         UInt32Buffer, length E
edge_kind:         Int32Array, length E
edge_fact_ids:     BinaryArray, length E
```

This representation SHALL be built directly from sorted Arrow edge batches.

Petgraph is not required inside the data fabric. The operator may implement algorithms directly over Arrow-owned CSR buffers while still conforming to DataFusion `ExecutionPlan` contracts.

---

## 83. Reachability and graph traversal

### 83.1 Query-time traversal

`CpgGraphTraverseExec` SHALL support:

```text
seed IDs
relation-family/kind mask
direction: outgoing | incoming | both
maximum depth
maximum output rows
optional path predecessor output
certainty policy: exact-only | include-may
```

Output schema:

| Column | Type |
|---|---|
| `seed_id` | `id16` |
| `node_id` | `id16` |
| `depth` | `Int32` |
| `predecessor_id` | `id16` nullable |
| `via_relation_id` | `id16` nullable |
| `path_certainty_code` | `code16` |

### 83.2 Materialized reachability

Transitive closure SHALL only be materialized when bounded and repeatedly useful, such as:

- owner-local CFG reachability;
- call SCC condensation DAG reachability;
- small module dependency graphs.

Unbounded whole-graph closure is prohibited as a default table because of quadratic amplification.

---

## 84. SCC and recursion calculation

`CpgStrongComponentsExec` SHALL implement Tarjan or Kosaraju over CSR partitions.

Inputs:

```text
graph_scope_id
source_id
target_id
selected relation kinds
```

Outputs:

- `derived_component` rows;
- `relation` membership rows;
- recursive flags;
- component size metrics;
- condensed DAG edges when requested.

Global call-graph SCC computation SHALL use exact edges and may-edge variants as separate projection codes.

---

## 85. Dominator and post-dominator calculation

CFGs SHALL be grouped by `cfg_id` and computed owner-locally.

Outputs:

```text
IMMEDIATE_DOMINATOR
DOMINATES
STRICTLY_DOMINATES
IMMEDIATE_POST_DOMINATOR
POST_DOMINATES
```

Post-dominators SHALL use a synthetic exit when the CFG has multiple exits. Normal and unwind policies SHALL be selectable and encoded in `projection_code`.

Only immediate dominator edges are mandatory to materialize. Full dominance closure MAY be query-time or materialized for small CFGs.

---

## 86. Control dependence and loop calculation

Control dependence SHALL be derived from post-dominator frontiers.

Natural loops SHALL be derived from back edges whose targets dominate their sources. Irreducible loops SHALL use SCC fallback and explicit loop-kind codes.

Outputs:

```text
BACK_EDGE
LOOP_MEMBER
LOOP_HEADER
CONTROL_DEPENDENT_ON
loop nesting-depth metrics
```

---

## 87. Reaching definitions and liveness

`CpgReachingDefinitionsExec` and `CpgLivenessExec` SHALL use dense bitsets over owner-local definitions/variables.

### 87.1 Reaching definitions

```text
IN[b]  = union OUT[p] for predecessors p
OUT[b] = GEN[b] union (IN[b] - KILL[b])
```

Outputs:

```text
REACHES
DEF_USE
DATA_DEP
```

Alias-aware kill rules SHALL be selected by analysis precision profile.

### 87.2 Liveness

```text
OUT[b] = union IN[s] for successors s
IN[b]  = USE[b] union (OUT[b] - DEF[b])
```

Outputs SHALL be program-point state rows or relations, not opaque bitsets.

Bitsets are internal execution state only.

---

## 88. Points-to and alias fixed point

`CpgPointsToExec` SHALL consume normalized constraints such as:

```text
address/reference creation
assignment/copy/move flow
field projection
load/store
argument-to-parameter flow
return flow
call target constraints
```

It SHALL iterate to fixed point per configured analysis domain.

Outputs:

```text
POINTS_TO
MAY_POINT_TO
MUST_ALIAS
MAY_ALIAS
DOES_NOT_ALIAS only when proven
```

Unknown memory SHALL be propagated explicitly rather than discarded.

---

## 89. Interprocedural summary fixed point

`CpgSummaryFixpointExec` SHALL:

1. build a selected call projection;
2. compute call SCCs;
3. condense to a DAG;
4. process SCCs in reverse topological order;
5. iterate recursive SCC members until summary stabilization;
6. union direct reads, writes, calls, effects, returns, and unknown flags;
7. emit transitive `effect_detail`, summary relations, and `callable_summary` rows.

Exact-only and exact-plus-may summaries SHALL be separate derivation profiles.

Unknown call targets SHALL set `unknown_effect = true` and prevent claims of a closed effect set.

---

## 90. Custom-operator execution requirements

Every custom `ExecutionPlan` SHALL:

- report a correct Arrow schema;
- expose correct `PlanProperties`;
- preserve partitioning/order claims conservatively;
- use DataFusion memory reservations;
- support cancellation;
- stream `RecordBatch` output;
- avoid unbounded output without explicit caps;
- expose metrics;
- spill or reject when memory limits are exceeded;
- include deterministic EXPLAIN formatting;
- have plan-property and execution golden tests.

---

# Part XV — Serving Catalog and Query Surface

## 91. `ServingSnapshot`-pinned overlay-aware catalog provider

Every query catalog SHALL be created from one leased immutable `ServingSnapshot`, never from mutable global pointers and never from a publication alone.

The durable half of that catalog is the snapshot-scoped `DeltaBaseCatalog` specified in section 12.6: one immutable set of exact-version Delta `TableProvider`s, built during candidate-snapshot construction and reused by every lease. This section specifies how the overlay is layered over those providers; section 12.6 specifies their construction and lifetime, and section 12.7 why their internal caches are never durable state.

For each table, the provider binds:

```text
exact durable Delta version from the base publication
one immutable overlay replacement batch set
owner/table replacement keys
tombstones
workspace/context/source-generation filters
schema and enum bundle fingerprints
```

For owner-scoped tables:

```text
current rows =
    overlay replacements
    UNION ALL
    base rows
      ANTI JOIN overlay replaced owner/table/context keys
      ANTI JOIN overlay tombstones
```

Every physical table SHALL have an explicit `overlay_mutation` in `TableSpec`.
Query-time-derived surfaces are expressed by
`materialization_role = QUERY_TIME_DERIVED` and
`overlay_mutation = NOT_APPLICABLE`; they consume the already-composed leased
snapshot and are not themselves overlay tables. Unspecified axis values or an
invalid cross-axis combination are schema errors.

The catalog exposes the same `snapshot_id`, workspace, context set, base publication, overlay generation/checksum, capability index, and source-trust metadata used by the query response. A long-running query never observes an active-pointer change.

## 92. Stable serving views

The catalog SHALL expose at least:

```text
cpg_serving.entities
cpg_serving.relations
cpg_serving.files
cpg_serving.syntax
cpg_serving.symbols
cpg_serving.types
cpg_serving.members
cpg_serving.calls
cpg_serving.call_graph
cpg_serving.cfg_nodes
cpg_serving.cfg_edges
cpg_serving.def_use
cpg_serving.value_flow
cpg_serving.memory_accesses
cpg_serving.aliases
cpg_serving.effects
cpg_serving.exceptions
cpg_serving.resources
cpg_serving.async_relations
cpg_serving.generated
cpg_serving.unknowns
cpg_serving.metrics
cpg_serving.callable_summaries
```

Views SHALL:

- hide operational hash/bucket columns by default;
- preserve fact IDs;
- expose enum names alongside codes where useful;
- retain certainty and resolution;
- avoid collapsing exact and may relationships.

---

## 93. Table functions

Recommended UDTFs:

### 93.1 `cpg_neighbors`

```text
cpg_neighbors(node_id, relation_family, direction)
```

Returns direct relation rows and endpoint metadata.

### 93.2 `cpg_reachable`

```text
cpg_reachable(seed_id, relation_set, direction, max_depth, include_may)
```

Backed by `CpgGraphTraverseExec`.

### 93.3 `cpg_source_context`

```text
cpg_source_context(entity_id, before_lines, after_lines)
```

Returns source bytes/text and enclosing syntax/semantic owners.

### 93.4 `cpg_owner_facts`

```text
cpg_owner_facts(owner_id, fact_family_mask)
```

Returns all fact IDs owned by the selected owner without scanning unrelated buckets.

These functions provide factual retrieval only.

---

## 94. Query-planning policy

Internal agent-query compilation SHOULD build `Expr` and `LogicalPlan` directly rather than emit arbitrary SQL.

The query compiler SHALL:

- bind enum names to codes;
- inject ID buckets;
- qualify all columns;
- alias computed output fields;
- push owner/file/entity filters to base tables;
- choose typed extension tables only when requested fields require them;
- cap recursive/traversal output;
- preserve exact/may distinctions;
- include certainty in returned facts.

---

# Part XVI — Physical Layout and Performance

## 95. Partitioning policy

### 95.1 Small control and dimension tables

No partitioning:

```text
repository
publication
publication_table
current_publication
enum catalog
```

### 95.2 Owner-local fact tables

Partition by:

```text
owner_bucket
```

### 95.3 Universal `entity`

Partition by:

```text
entity_family_code, owner_bucket
```

### 95.4 Universal `relation`

Partition by:

```text
relation_family_code, owner_bucket
```

### 95.5 High-volume effect/derived tables

Partition by their low-cardinality semantic family plus owner bucket, for example:

```text
effect_kind_code, owner_bucket
projection_code, owner_bucket
metric_code, owner_bucket
```

High-cardinality IDs SHALL NOT be partition columns.

---

## 96. Z-order and clustering policy

Z-order is a maintenance optimization, not a semantic requirement.

Recommended candidates:

| Table | Z-order candidates |
|---|---|
| `entity` | `entity_id_hash64`, `parent_entity_id_hash64`, `file_id_hash64` |
| `relation` | `source_hash64`, `target_hash64`, `relation_kind_code` |
| `reference_detail` | resolved-entity hash, name hash if materialized |
| `call_target_detail` | call-site hash, target hash |
| `memory_access_detail` | location hash, cfg-node hash |
| `effect_detail` | callable hash, target hash |

Z-order SHALL only be scheduled after representative query benchmarks show file-skipping benefit.

---

## 97. Parquet writer policy

Starting writer targets:

```text
target Delta file size          128–256 MiB
Parquet row-group size           32–128 MiB
compression                      ZSTD unless interoperability dictates otherwise
dictionary encoding              enabled for low/medium-cardinality strings and codes
statistics                       enabled for IDs, buckets, codes, file IDs, offsets
Bloom filters                    benchmark for point-looked-up IDs
Arrow schema metadata            retained
```

Very small owner updates SHALL be micro-batched across owners before publication to avoid tiny files.

One owner SHALL NOT imply one Parquet file.

---

## 98. DataFusion runtime policy

The query/derivation runtime SHALL configure:

```text
target_partitions               based on CPU and workload
batch_size                      benchmarked; start 65,536
limited memory pool             mandatory for services
spill directory                 mandatory for large/global calculations
max spill size                  bounded
metadata/file/statistics cache  enabled
Parquet pruning                 enabled
repartition joins/aggregates    enabled where beneficial
```

Custom graph operators SHALL use the same `RuntimeEnv`, memory pool, disk manager, and object-store registry as normal DataFusion execution.

### 98.1 Delta provider access profiles

Delta handles are opened for several purposes with different materialization needs. Every Delta handle SHALL be opened under exactly one declared access profile:

| Access profile | Materialization posture | `skip_stats` | Purpose |
|---|---|---:|---|
| `QUERY_SERVING` | exact-version provider; lazy replay permitted | **false** | normal semantic and DataFusion queries |
| `PUBLICATION_METADATA` | metadata-first / lazy | may be `true` only when no pruning or data scan is performed | schema, protocol and table-version validation |
| `APPEND_ONLY_WRITER` | metadata-first where safe | may be `true` | writes that do not inspect existing files |
| `VACUUM_FILESYSTEM_CHECK` | operation-specific | `true` may be appropriate | maintenance without query pruning |
| `OPTIMIZE_DML` | active files and statistics as the operation requires | `false`/default unless the upstream operation owns stronger replay | rewrite maintenance |

### 98.2 Query-serving statistics profile

A query-visible Delta provider SHALL NOT be created with a statistics-skipping configuration unless the query planner can prove that no data-skipping predicate will be required and the resulting performance regression is explicitly accepted.

The pinned delta-rs revision can replay active adds with stronger statistics capabilities when an internal operation demands them, even where the resident materialized cache was built without statistics. That is useful hardening. It is **not** a reason to disable statistics on the primary query path: the public `skip_stats` contract still permits a predicated query on a stats-less instance to scan every file, and partition pruning is a separate mechanism that does not compensate. This restates, for Delta handles specifically, the `metadata/file/statistics cache enabled` and `Parquet pruning enabled` requirements above.

### 98.3 Provider warm-up is a performance policy

Because replay is lazy, work can move from snapshot activation to the first operation that needs active files or statistics. For tables proven hot by measurement, the daemon MAY warm selected exact-version providers during `ServingSnapshot` activation with a bounded query that forces the required state through the provider's normal execution path.

Warm-up SHALL NOT be globally required, and SHALL NOT be part of snapshot correctness. Recommended default posture:

```text
cold or seldom-queried extension tables      leave lazy
entity / relation / high-frequency detail    benchmark optional warm-up
control tables                               eager cost is usually negligible
```

---

## 99. Update locality

Owner replacement and derived invalidation SHALL minimize rewritten files by:

- grouping changed owners by `owner_bucket`;
- sorting outgoing rows by owner and primary key;
- writing multi-owner batches;
- avoiding full-table overwrite;
- only recomputing derived owners reachable in the dependency graph;
- materializing global derived tables only when their cost is justified.

---

## 100. Compaction thresholds

An optimize job SHOULD be triggered when any partition exceeds configured thresholds such as:

```text
active file count
median file size below threshold
small-file ratio
query planning latency
post-DML rewrite fragmentation
```

Default maintenance:

- compact closed owner buckets or relation-family partitions;
- target 128–256 MiB files initially;
- cap concurrent optimize tasks;
- use the service DataFusion session state;
- require session fallback policy rather than silently using internal defaults.

### 100.1 Nested-schema optimize obligations

Optimize runs generically across the Delta namespace, and the schema registry already admits bounded `List`, `Map` and `Struct` payloads. Two nested cases are certified once for the maintenance subsystem rather than encoded as per-table exceptions.

**Physically nullable nested fields under a stricter logical schema.** Parquet written by other engines may mark a nested field optional where the Delta logical schema declares it `NOT NULL`:

```text
logical    meta: STRUCT<int_id: STRING NOT NULL> NULLABLE
physical   meta.int_id optional
```

The pinned delta-rs revision relaxes nested nullability for physical read and adaptation, then restores the strict logical Delta schema after the scan. Data that actually violates the logical contract still fails validation. CodeFabric SHALL rely on that adaptation layer and SHALL NOT weaken canonical schema nullability to accommodate physical encodings (see section 103.4).

**Nested field name equal to a top-level partition column.** Where a nested field shares its name with a partition column:

```text
date                             top-level partition column
properties STRUCT<date: STRING>  ordinary nested field
```

only the top-level column is a partition-field candidate. The nested field SHALL retain its ordinary representation through scan and optimize, with no partition or dictionary coercion.

Both cases are mandatory regression fixtures in section 112.3.

---

## 101. Vacuum policy

Vacuum SHALL preserve:

- every version pinned by `current_publication`;
- any publication in staging/validation that may still complete;
- the configured recovery publication, if one exists;
- any explicit operational hold.

The core fabric SHALL not retain old versions for semantic history.

Vacuum workflow:

```text
1. enumerate pinned table versions
2. dry run
3. verify candidates do not serve pinned publications
4. execute retention-governed vacuum
5. reopen current publication
6. run table and cross-table smoke queries
```

Vacuum reachability is computed over **table versions**, never over checkpoint files. Checkpoints do not define publication identity (section 12.8), so their creation or removal SHALL NOT by itself make a version reachable or unreachable. A checkpoint written at a version that a lease still pins is a replay artifact of a version that must be preserved for other reasons; it neither extends nor shortens that version's retention.

### 101.1 Delta action paths are opaque URI identities

Where maintenance code inspects Delta file actions directly, `Add.path`, `Remove.path` and change-data-feed action paths SHALL be created, encoded, decoded, and compared through delta-rs and `object_store` path facilities.

The pinned revision corrects percent-encoding of spaces in action paths — `part 0.parquet` serializes as `part%200.parquet` — while preserving `/` as the path hierarchy separator and `=` as the Hive partition delimiter, and while remaining compatible with already-encoded and legacy unencoded paths. Hand-rolled encoding is therefore both unnecessary and unsafe.

Specifically, maintenance code SHALL NOT:

```text
construct transaction-log action paths by string concatenation
decode an action path for display and reuse the display string as identity
compare CodeFabric source-path byte identity to Delta Parquet action paths
```

CodeFabric's byte-safe source-path contract governs **source files inside analyzed workspaces**. Delta's own Parquet and log object paths are a separate storage-identity domain. The two SHALL NOT be conflated, and no ontology change follows from this rule.

---

# Part XVII — Constraints, Integrity, and Schema Evolution

## 102. Delta constraints

Delta constraints SHOULD enforce row-local invariants where supported.

Examples:

```text
start_byte >= 0
end_byte >= start_byte
owner_bucket BETWEEN 0 AND 255
source_bucket BETWEEN 0 AND 255
target_bucket BETWEEN 0 AND 255
counts >= 0
required IDs NOT NULL
```

ID byte-length checks SHOULD be enforced in Arrow validation and MAY also be expressed as Delta checks when the pinned expression support is compile-tested.

### 102.1 Uniqueness and foreign keys

Delta does not replace application-level uniqueness and foreign-key validation.

The schema registry SHALL declare:

- primary keys;
- unique constraints;
- foreign-key-like references;
- required relation endpoint families.

DataFusion validation plans SHALL enforce these before publication.

### 102.2 DataFusion constraints

Published `TableProvider`s MAY expose primary/unique constraints to DataFusion only after the publication has passed uniqueness validation.

Incorrect constraint metadata is prohibited because optimizer behavior may rely on it.

---

## 103. Schema compatibility policy

Default compatible changes:

```text
add nullable column
add advisory field metadata
add new enum code
add new optional extension table
```

Default incompatible changes:

```text
rename/drop persisted column
change primary key
change partition columns
narrow type
change nullability from nullable to required
reuse enum code
change ID encoding
change table grain
```

### 103.1 Required-field additions

A new non-nullable field requires:

1. new schema version;
2. deterministic backfill;
3. validation;
4. publication using the migrated table;
5. compatibility review.

### 103.2 Partition evolution

Partition changes SHALL create a new Delta table root, backfill through DataFusion, validate, and update the publication manifest. In-place routine partition changes are prohibited.

### 103.3 Schema merge

`SchemaMode::Merge` SHALL not be the default ingestion mode. Schema evolution is an explicit migration operation.

### 103.4 Physical nested-schema adaptation

The Delta logical schema remains the authoritative durable table contract even where the Parquet physical encoding is looser — most commonly, a nested field written as optional beneath a logical `NOT NULL` declaration.

```text
Delta logical schema        authoritative CodeFabric durable table contract
Arrow batches               SHALL conform to the logical contract
Parquet physical nullability storage encoding detail, handled by the delta-rs adapter
```

CodeFabric SHALL rely on the pinned delta-rs and DataFusion schema-adaptation layer for that reconciliation. It SHALL NOT weaken canonical schema nullability to improve interoperability, and SHALL NOT read Delta-owned Parquet files through an independent raw-Parquet provider that bypasses the adaptation layer. The upstream fix resolves the mismatch at the correct layer and removes the need for an application workaround; the corresponding regression fixtures are in section 112.3.

---

# Part XVIII — Operational Workflows

## 104. Bootstrap workflow

```text
1. initialize schema registry
2. create control tables
3. create all required Delta fact tables
4. register immutable enum dimensions
5. ingest complete source/fact snapshot as Arrow streams
6. reconcile and derive
7. validate all tables
8. publish first manifest
9. open current-state DataFusion catalog
10. run conformance queries
```

## 105. Incremental owner refresh

```text
1. receive owner-scoped FactBatch outputs
2. validate Arrow schemas and IDs
3. encode base extension tables
4. replace affected owners in base tables
5. reconcile canonical entity/relation rows
6. rebuild owner-local CFG/dataflow/memory facts
7. recompute owner-local derived facts
8. propagate affected call/summary computations
9. validate
10. publish new manifest
```

## 106. Owner deletion

```text
1. identify removed owner and dependent generated owners
2. delete owner rows from every owner-scoped table
3. remove cross-owner relations owned by affected callers/sources
4. recompute affected global/SCC/summary tables
5. validate no dangling current relations
6. publish
```

## 107. Failed publication recovery

```text
active pointer remains unchanged
abandoned Delta versions remain unreferenced
retry uses same publication/operation IDs where safe
or start a replacement publication
cleanup occurs after retention and pinned-version checks
```

## 108. Schema migration workflow

```text
1. register new TableSpec version
2. create new table root when required
3. read current publication through pinned providers
4. transform with DataFusion
5. write migrated Delta table
6. validate Arrow/Delta/DataFusion schemas
7. publish manifest referencing new table/version
8. retain old pinned version until recovery window expires
9. vacuum according to policy
```

## 109. Maintenance workflow

```text
compact fragmented closed partitions
benchmark Z-order candidates
vacuum unreferenced versions after dry run
refresh statistics/manifest counts
reopen current catalog
run integrity and representative query suite
```

---

# Part XIX — Query, Validation, and Observability Artifacts

## 110. Plan artifact bundle

Every important derivation and serving query SHOULD be able to emit:

```text
input PlanSpec or query identifier
DataFusion version
Arrow version
schema registry version
publication ID
source table versions
logical plan
optimized logical plan
physical plan
output schema
partition count
EXPLAIN text/graphviz
execution metrics
row count
result checksum
```

Plan artifacts are operational diagnostics, not CPG facts.

---

## 111. Metrics

The fabric SHALL emit operational metrics for:

```text
provider rows received
Arrow rows encoded
validation failures
DataFusion reconciliation rows
Delta commits by table
owner replacement latency
publication latency
rows/files per table
Parquet file sizes
small-file counts
query planning/execution time
spill bytes
custom graph operator iterations
custom graph operator peak memory
unknown fact counts
integrity query failures
```

These metrics SHALL not be inserted into the semantic CPG metric table unless they describe objective code structure rather than fabric operation.

### 111.1 Delta activation and replay metrics

Because replay is lazy, work moves between opening a table and the first operation that needs active files or statistics. A single end-to-end query-latency metric hides that shift, so the durable-provider lifecycle SHALL be instrumented separately from query execution:

```text
delta_snapshot_open_ms
delta_provider_build_ms
delta_provider_activation_count
delta_first_scan_ms
delta_first_predicated_scan_ms
delta_table_version
delta_checkpoint_version_if_available    diagnostic only; never semantic identity
delta_active_file_count_when_materialized
delta_materialization_reason             QUERY | VALIDATION | DML | OPTIMIZE | CONFLICT_CHECK
delta_stats_policy_class                 CodeFabric-owned abstraction
```

Telemetry contracts SHALL NOT depend on private upstream enum or type names. Upstream states SHALL be mapped into CodeFabric-owned diagnostic categories, so that an upstream refactor cannot break a published metric contract. `delta_checkpoint_version_if_available` is explicitly diagnostic: it never contributes to publication or snapshot identity (section 12.8).

---

## 112. Testing strategy

### 112.1 Schema tests

- exact Arrow schema snapshots;
- Arrow-to-Delta-to-DataFusion round trip;
- field metadata preservation;
- unsupported type rejection;
- partition contract tests.

### 112.2 Batch tests

- empty batch;
- one row;
- all nullable fields null;
- maximum-length names/paths;
- invalid ID length;
- duplicate primary keys;
- malformed source spans.

### 112.3 Delta tests

- owner replacement visibility through old/new manifests;
- retry idempotency;
- concurrent publication conflict;
- delete+append recovery;
- optimize and vacuum safety;
- local and object-store backends;
- optimize Spark-style physically nullable nested fields under a stricter Delta logical schema;
- optimize a table whose nested field name equals a top-level partition column;
- assert pre/post-optimize logical schema digest equality;
- assert pre/post-optimize row and content digest equality;
- assert no nested dictionary or partition coercion leakage;
- data-file path containing a space serializes and reopens correctly;
- Hive partition delimiters survive action-path round trip;
- an already percent-encoded path round-trips without double encoding.

### 112.4 DataFusion tests

- catalog opens exact pinned versions;
- projection/filter/bucket pushdown;
- logical and physical plan snapshots;
- custom UDF/UDAF tests;
- custom graph operator golden results;
- memory/spill limits;
- cancellation.

### 112.5 Integrity tests

- dangling edge detection;
- owner completeness;
- CFG consistency;
- dataflow endpoint consistency;
- type endpoint consistency;
- unknown materialization;
- table row counts/checksums;
- cross-publication isolation.

### 112.6 delta-rs upgrade gate

Every change to the pinned delta-rs revision SHALL pass this gate before the pin is accepted. It exists because the storage engine's snapshot, cache and replay behavior can change without any change to CodeFabric's own contracts.

**Snapshot and cache behavior**

```text
[ ] an exact Delta version yields identical logical content with and without a
    checkpoint at that same version
[ ] a provider rebuilt after same-version checkpoint creation returns identical
    rows and checksums
[ ] a provider or snapshot cache from version N is never reused as version N+1 state
[ ] a table-root mismatch cannot reuse a cached provider or snapshot
[ ] daemon restart reconstructs providers solely from the publication manifest and
    Delta versions, with no persisted engine cache
```

**Lazy and eager equivalence**

```text
[ ] a lazy exact-version provider and a fully materialized read return identical rows
[ ] metadata-first load and normal load return identical protocol, schema and
    table metadata
[ ] the first predicated scan after lazy activation matches the steady-state scan
```

**Statistics policy**

```text
[ ] a QUERY_SERVING provider retains pruning capability
[ ] a deliberately stats-skipped test demonstrates the expected loss of file pruning
    and is never used as a production default
[ ] partition pruning remains correct independently of file statistics
```

**Optimize**

```text
[ ] Spark-style nested nullability fixture passes
[ ] nested field name matching a top-level partition column passes
[ ] logical schema digest is identical before and after optimize
[ ] a publication-pinned old version remains queryable until normal retention and
    vacuum policy permits removal
```

**Protocol features**

```text
[ ] a V2Checkpoint-declaring fixture passes generic feature compatibility
[ ] unsupported features such as identity columns and type widening still fail closed
[ ] CodeFabric-owned table creation does not enable V2Checkpoint implicitly
```

**Equivalence and performance**

```text
[ ] clean-build CPG logical digests match the golden corpus
[ ] canonical query responses are unchanged for unchanged source
[ ] publication, overlay and freshness semantics are unchanged
[ ] durable-base activation latency and peak RSS show no material regression
[ ] first filtered query latency is bounded
[ ] steady-state filtered query p50/p95 show no material regression
[ ] unfiltered full scan shows no material regression
[ ] owner-replacement conflict-check latency is bounded
[ ] optimize on a nested table is correct and within timing budget
[ ] reopening a table after checkpoint creation yields the same logical version
    and result
```

Acceptance SHALL be judged on representative CodeFabric table shapes across small, medium and large tables, not on upstream synthetic benchmarks.

---

# Part XX — Rust Workspace Architecture

## 113. Recommended crates

```text
cpg-schema
  Arrow/Delta schemas, enum registries, TableSpec, metadata keys

cpg-arrow
  typed builders, batch encoders, validators, Arrow kernels

cpg-delta
  table creation, owner replacement, DML, publication manifest, maintenance

cpg-catalog
  ServingSnapshot-pinned overlay-aware CatalogProvider / SchemaProvider / TableProvider wrappers

cpg-plans
  reconciliation plans, serving PlanSpec compiler, integrity plans

cpg-functions
  DataFusion UDFs, UDAFs, and UDTFs

cpg-graph-exec
  custom logical nodes and ExecutionPlans for graph/dataflow algorithms

cpg-publisher
  dependency scheduling, multi-table publication, recovery

cpg-query
  stable serving views and agent-facing fact query compiler

cpg-conformance
  fixtures, golden schemas, SQLLogicTests, property tests, benchmarks
```

Provider/extractor crates from the companion generation specification remain upstream of this fabric.

---

## 114. Core Rust interfaces

```rust
pub trait TableEncoder {
    fn table_spec(&self) -> &'static TableSpec;
    fn encode(&self, batch: CanonicalFactBatch) -> Result<Vec<RecordBatch>, EncodeError>;
}

#[async_trait::async_trait]
pub trait OwnerTableWriter {
    async fn replace_owners(
        &self,
        table: &TableSpec,
        owners: &[Id128],
        batches: Vec<RecordBatch>,
        operation: OperationContext,
    ) -> Result<CommittedTableVersion, WriteError>;
}

pub trait ReconciliationPlanner {
    fn build_plan(&self, inputs: ReconcileInputs) -> Result<LogicalPlan, PlanError>;
}

pub trait DerivationPlanner {
    fn dependencies(&self) -> &'static [TableCode];
    fn build_plan(&self, publication: &PublicationView) -> Result<LogicalPlan, PlanError>;
}

#[async_trait::async_trait]
pub trait DurablePublicationStore {
    async fn stage(&self, request: PublicationRequest) -> Result<PublicationId, PublishError>;
    async fn record_table(&self, version: CommittedTableVersion) -> Result<(), PublishError>;
    async fn validate(&self, publication: PublicationId) -> Result<ValidationReport, PublishError>;
    async fn advance_current_durable_base(&self, workspace_id: WorkspaceId, publication: PublicationId)
        -> Result<(), PublishError>;
}

#[async_trait::async_trait]
pub trait ServingSnapshotStore {
    async fn build(&self, request: ServingSnapshotBuildRequest)
        -> Result<ServingSnapshot, SnapshotError>;
    async fn activate(&self, workspace_id: WorkspaceId, snapshot: ServingSnapshot)
        -> Result<SnapshotId, SnapshotError>;
    async fn lease(&self, workspace_id: WorkspaceId, snapshot_id: SnapshotId)
        -> Result<ServingSnapshotLease, SnapshotError>;
}
```

Provider-specific and delta-rs-internal types SHALL not leak through stable application interfaces.

---

# Part XXI — Implementation Sequence

## 115. Phase 1 — Schema and publication foundation

Deliver:

- version-pinned workspace;
- `TableSpec` registry;
- `workspace`, optional `common_repository`, analysis-context, durable-publication, active-snapshot, owner, capability, and diagnostic tables/views;
- `entity`, `relation`, `property_fact`, and `fact_evidence`;
- durable-base publication provider plus `ServingSnapshot`-pinned overlay-aware DataFusion catalog;
- Arrow/Delta/DataFusion schema round-trip tests.

## 116. Phase 2 — Source and semantic base tables

Deliver:

- source, token, annotation, syntax, semantic, scope, binding, reference, import tables;
- typed Arrow encoders;
- owner replacement;
- canonical reconciliation plans.

## 117. Phase 3 — Types, calls, and CFG

Deliver:

- type, member, callable, parameter, call site, argument, target tables;
- CFG graph/node/edge tables;
- core serving views;
- point-query pushdown.

## 118. Phase 4 — Dataflow, memory, effects, and language extensions

Deliver:

- value, operation, dataflow event, memory/access-path, program-state tables;
- effect, exception, resource, async, capture, generated tables;
- Python dynamic and Rust MIR extension tables.

## 119. Phase 5 — Derived calculations

Deliver:

- direct relational metrics;
- reachability operator;
- SCC operator;
- dominator/post-dominator/control-dependence operators;
- loop derivation;
- reaching definitions/liveness;
- points-to/alias fixed point;
- interprocedural summary propagation.

## 120. Phase 6 — Performance and production hardening

Deliver:

- optimized partition specs;
- compaction and vacuum runbooks;
- Bloom/Z-order benchmarks;
- memory/spill policies;
- idempotent retries and recovery;
- object-store tests;
- full conformance and performance suite.

---

# CodeFabric 1.3 architecture-completion contracts

The standalone architecture-completion specification has been propagated into its permanent owners. This part contains the full normative contracts owned by this document: `G-19`, `G-20`, `G-21`, `G-22`, `G-23`, `G-26`, `G-37`, `G-42`. References to a gap ID elsewhere in the synchronized suite resolve to these sections.

## AC-G-19 — Complete `ServingSnapshot` manifest schema
### Decision

A `ServingSnapshot` is a content-addressed immutable manifest plus immutable runtime objects. Its ID covers the complete query-visible and freshness-relevant manifest. Two activations with different source, capability, context, or freshness generations therefore remain distinct even when their canonical fact rows happen to match; reactivating the exact same manifest yields the same ID.

### Contract

The canonical manifest is:

```yaml
manifest_version: "1.0"
snapshot_id: derived
workspace_id: workspace:...
repository_id: optional
worktree_id: optional
registration_revision: integer
source:
  source_generation: integer
  admitted_event_sequence: integer
  reconciled_event_sequence: integer
  inventory_digest: b3:...
  authorization_fingerprint: b3:...
  inclusion_policy_fingerprint: b3:...
  path_profile_version: "1.0"
  source_trust_state: CURRENT | POTENTIALLY_STALE | UNAVAILABLE
  event_stream_health: HEALTHY | RESCAN_REQUIRED | DEGRADED | UNAVAILABLE
  git_acceleration_status: canonical enum
  git_state_fingerprint: optional b3:...
contexts:
  context_set_id: context-set:...
  default_python_context_id: optional
  default_rust_context_id: optional
  records:
    - analysis_context_id
      context_manifest_digest
      capability_partition_digest
base_publication:
  publication_id: publication:...
  tables:
    - table_code
      table_uri
      delta_version
      schema_digest
      row_count
      primary_key_digest
      effective_content_digest
overlay:
  overlay_generation: integer
  overlay_digest: b3:...
  total_memory_bytes: integer
  tables:
    - table_code
      mutation_policy
      replacement_row_count
      owner_tombstone_count
      key_tombstone_count
      table_replacement: boolean
      row_digest
      tombstone_digest
indexes:
  capability_index_digest: b3:...
  diagnostic_index_digest: b3:...
  dependency_graph_digest: b3:...
bundles:
  ontology_bundle_id
  schema_bundle_id
  provider_bundle_id
  derivation_bundle_id
  query_language_bundle_id
  model_pack_bundle_id
  toolchain_bundle_id
limits_profile_digest: b3:...
manifest_digest: b3:...
```

The canonical manifest body is serialized with CBEF-v1 in the field order defined by the generated `ServingSnapshotManifest` schema, excluding only `snapshot_id` and `manifest_digest`. The identifiers are:

```text
manifest_digest = BLAKE3_256(
  "codefabric-serving-snapshot-manifest-v1" || canonical_manifest_body
)

snapshot_id = BLAKE3_128(CBEF-v1(
  domain = SERVING_SNAPSHOT,
  manifest_digest
))
```

Creation time, activation time, observed durable-pointer generation, active-pointer generation, memory address, and lease count belong to a separate mutable `SnapshotActivationRecord`; they do not participate in the immutable manifest or `snapshot_id`.

A manifest is valid only if every ordered table/schema/bundle reference exists and its digest verifies. Snapshot construction fails closed on a missing optional-looking field whose presence is required by the selected deployment/profile manifest.

The source-trust, event-stream-health, Git, and admitted/reconciled sequence values in the manifest are observations at snapshot construction. They are immutable. A later admitted filesystem event may make the still-active snapshot operationally potentially stale without rewriting it. Workspace status and query freshness compare current operational counters/state to the manifest and report the effective freshness separately; they never mutate an existing `ServingSnapshot`.
## AC-G-20 — Hot-overlay physical schemas and mutation representation
### Decision

Overlay rows use the exact canonical table schema. Overlay control metadata is held in immutable per-table manifests and typed tombstone indexes rather than appended to canonical query columns.

### Contract

Each overlay table object contains:

```rust
struct OverlayTable {
    table_code: i16,
    mutation_policy: OverlayMutationPolicy,
    replacement_batches: Vec<Arc<RecordBatch>>, // exact base table schema
    owner_tombstones: Arc<OwnerTombstoneIndex>,
    key_tombstones: Arc<PrimaryKeyTombstoneIndex>,
    full_table_replacement: bool,
    min_source_generation: i64,
    max_source_generation: i64,
    primary_key_ordering: PrimaryKeyOrdering,
    content_digest: [u8; 32],
}
```

Tombstone Arrow schemas are:

```text
owner_tombstone:
  workspace_id         id16
  analysis_context_id  id16
  table_code            code16
  owner_id              id16
  tombstone_generation  int64
  reason_code           code16

primary_key_tombstone:
  workspace_id         id16
  analysis_context_id  id16
  table_code            code16
  encoded_primary_key   binary  // CBEF encoding of table primary-key fields
  tombstone_generation  int64
  reason_code           code16
```

Replacement batches SHALL be:

- sorted by the table registry's primary-key ordering;
- duplicate-free within a consolidated overlay;
- validated against the exact schema digest;
- immutable after publication;
- generation-fenced to the snapshot source/context generation;
- stored as `Arc<RecordBatch>` and shared zero-copy among snapshot providers.

Per-table indexes MAY use Rust hash/sorted structures internally, but their logical content and digest are defined by the Arrow tombstone rows above.

Overlay construction has a hard memory reservation. It SHALL fail before activation rather than allocate beyond the workspace/query headroom budget.
## AC-G-21 — Overlay semantics for owner-scoped, cross-owner, and global tables
### Decision

Every physical table declares exactly one overlay mutation policy in the
schema registry. Operational projections and query-time-derived surfaces use
`NOT_APPLICABLE`; their distinct meaning is carried by `MaterializationRole`.

### Contract

```text
OWNER_REPLACE
PRIMARY_KEY_UPSERT
FULL_TABLE_REPLACE
BASE_IMMUTABLE
NOT_APPLICABLE
```

Policy semantics:

| Policy | Effective table rule | Typical tables |
|---|---|---|
| `OWNER_REPLACE` | Remove all base/older-overlay rows for touched owners, then union replacement rows | entity, relation, property facts, owner-local CFG/dataflow, callable summaries |
| `PRIMARY_KEY_UPSERT` | Remove exact touched keys, then union replacements | cross-owner index rows whose owner cannot safely capture mutation scope |
| `FULL_TABLE_REPLACE` | A table tombstone hides the entire base table; overlay rows are the complete effective table | workspace-global SCC/component maps or global registries produced per snapshot |
| `BASE_IMMUTABLE` | No overlay writes permitted | enum dimensions, bundle registries, schema registry |
| `NOT_APPLICABLE` | No overlay object exists for this surface; materialization role supplies the reason | provider runs, progress, queue metrics, query-time-derived views |

Every canonical relation still has an `owner_id`. For direct inter-owner relations, the owner is the source/emitting semantic owner unless the derivation registry explicitly assigns a global derivation owner. This keeps most relation tables under `OWNER_REPLACE`.

Property facts inherit the subject entity's replacement owner unless the property registry names another deterministic owner.

Workspace-global derivations SHALL use `FULL_TABLE_REPLACE`; partial replacement of a global fixed-point result is prohibited unless a later derivation profile formally proves a smaller stable replacement partition.

Base-immutable tables are loaded from the pinned bundles and are not duplicated in each overlay.

The valid role combinations are generated and verifier-enforced. In
particular, `OPERATIONAL_PROJECTION` is never a `ServingSnapshot` effective
fact table; `QUERY_TIME_DERIVED` reads only one leased snapshot;
`BUNDLE_DIMENSION` is `BASE_IMMUTABLE`; and a `DURABLE_EFFECTIVE` table must
declare a durable mutation class plus either a real overlay policy or
`BASE_IMMUTABLE`.
## AC-G-22 — Deterministic overlay consolidation, merge, and durable rebase
### Decision

A new update wave is applied to the effective current snapshot and produces a new consolidated overlay. No query observes a chain of mutable overlays.

### Contract

For every table/key or table/owner, the highest accepted `source_generation` wins. Equal generations are legal only when payload digests are identical; otherwise activation fails with `OVERLAY_GENERATION_CONFLICT`.

Consolidation rules:

1. A higher-generation owner tombstone removes all earlier replacement rows for that owner.
2. Higher-generation replacement rows clear an earlier owner tombstone for the same owner.
3. A higher-generation primary-key tombstone removes an earlier key replacement.
4. Reintroduction at a higher generation clears the key tombstone.
5. `FULL_TABLE_REPLACE` at generation `g` discards every earlier table replacement, row, and tombstone for that table.
6. A superseded or stale wave is rejected before consolidation.
7. Consolidated replacement rows are sorted and deduplicated; all digests are recomputed from logical content, not insertion order.

Durable flush uses a three-snapshot protocol:

```text
S_old = base P_n + consolidated overlay O_n
capture O_flush = O_n
build and validate durable publication P_(n+1) equivalent to S_old
meanwhile accept newer waves into O_delta over S_old
CAS current_publication from P_n to P_(n+1)
rebase O_delta onto P_(n+1)
validate effective digest is unchanged
atomically activate S_new = P_(n+1) + rebased O_delta
```

Rows included in `O_flush` are removed during rebase only if their logical content digest is present in the new durable base. A failed CAS or digest mismatch aborts activation and restarts from the newly current base.

No durable-flush race may cause a generation to disappear or appear twice in effective state.
## AC-G-23 — Snapshot leases, overlay lifetime, result retention, and Delta vacuum
### Decision

Every query and result artifact obtains an explicit lease. In-process `Arc` ownership protects memory; SQLite lease records protect durable versions and crash recovery.

### Contract

Lease record:

```text
lease_id
lease_kind: QUERY | RESULT_ARTIFACT | RESOURCE_READ | MAINTENANCE
workspace_id
snapshot_id
base_publication_id
required_delta_versions
requires_overlay
agent_instance_id optional
created_at
last_heartbeat_at
expires_at
state: ACTIVE | RELEASING | RELEASED | EXPIRED | ORPHANED
process_instance_id
```

Rules:

- a query lease is created before execution and released after terminal response materialization;
- an artifact lease is created before the query lease is released if the artifact references snapshot-backed source subresources;
- `Arc<ServingSnapshot>` keeps overlay memory alive while any in-process lease exists;
- SQLite leases are heartbeated every 15 seconds for work lasting more than 30 seconds;
- query leases expire 5 minutes after the last heartbeat; artifact leases expire with artifact TTL; resource-read leases receive a 5-minute completion grace;
- after daemon crash, active leases become `ORPHANED`; vacuum treats them as active until the larger of their expiry or a 24-hour crash grace passes;
- a released artifact that contains all requested bytes independently may drop its snapshot lease once no subresource needs source/catalog access.

Delta vacuum SHALL retain every data file/version reachable from:

1. `current_publication`;
2. the active snapshot;
3. any non-expired lease;
4. any validated publication eligible for crash recovery;
5. the configured minimum retention window.

The default minimum Delta retention is seven days. A shorter retention requires an explicit unsafe maintenance profile and is never used by automated background maintenance.

Retired overlays are freed only when no `Arc` or lease references them. Vacuum, artifact GC, and overlay retirement are idempotent and separately auditable.
## AC-G-26 — Durable and active current-pointer transaction protocols
### Decision

The durable base pointer is updated in Delta under a workspace publication lease; the active snapshot pointer is updated transactionally in SQLite and then mirrored by an in-memory atomic `Arc` swap.

### Contract

#### Durable `current_publication`

1. The workspace coordinator acquires its exclusive publication lease.
2. It reads `(publication_id, pointer_generation)` from the pointer table at a pinned Delta version.
3. It verifies the new immutable publication is `COMPLETE` and references all required validated tables.
4. It commits a one-row replace conditioned on the expected pointer generation, writing generation `g + 1` and operation metadata containing the expected predecessor.
5. Delta optimistic concurrency conflict or predecessor mismatch fails with `CURRENT_POINTER_CONFLICT`.
6. The daemon reopens the committed pointer version and verifies exactly one row for the workspace.
7. Only then may the new base participate in snapshot activation.

The single-writer coordinator makes conflicts exceptional, but the CAS remains mandatory for crash/multi-process fencing.

#### Active snapshot

One SQLite `BEGIN IMMEDIATE` transaction SHALL:

1. insert the immutable snapshot manifest in `READY` state;
2. verify the expected active pointer generation and predecessor snapshot ID;
3. mark the predecessor `RETIRED` where present;
4. mark the new manifest `ACTIVE`;
5. replace `active_snapshot(workspace_id)` with generation `g + 1`;
6. commit.

After commit, the coordinator performs an atomic in-memory `ArcSwap` to the exact snapshot ID and pointer generation. If the process crashes between SQLite commit and memory swap, restart reconstructs memory from SQLite. It is prohibited to swap memory before the durable operational transaction commits.

Pointer recovery chooses only a manifest whose checksum, base publication, overlay, context set, and bundles all validate.
## AC-G-37 — Canonical reconciliation algorithm
### Decision

Reconciliation is deterministic, authority-driven, and conflict-preserving. Fuzzy matching is confined to explicitly defined source-correspondence rules and never silently selects semantic identity.

### Contract

Pipeline:

1. validate observation schemas, versions, source/context generation, and provider bundle;
2. normalize observations into application-owned DTOs;
3. sort by canonical reconciliation key;
4. match observations to source/syntax/semantic anchors;
5. group propositions by canonical fact preimage;
6. apply fact-family authority order;
7. emit canonical fact, evidence rows, conflicts, unknowns, and capability outcomes;
8. compute owner content fingerprints.

Canonical sort key:

```text
workspace_id, context_id, owner_id, fact_form, fact_kind,
subject key, role/ordinal, object/value key, provider precedence,
source start/end, observation digest
```

Source-correspondence rules:

| Kind | Match rule |
|---|---|
| identifier/reference | exact original-byte span required |
| declaration | exact name span plus compatible declaration kind and enclosing owner |
| call site | exact callee span preferred; otherwise same enclosing expression and overlap ratio >= 0.80 with unique candidate |
| type annotation | exact annotation span or exact bound semantic owner/role |
| generated/lowered item | explicit provider correspondence key; no fuzzy source-only match |

Authority behavior:

- higher-authority evidence selects the canonical proposition and lower authority remains in `fact_evidence`;
- two compatible exact observations coalesce;
- conflicting exact observations at the same authority produce `CONFLICTING_EXACT_EVIDENCE`, no arbitrary exact winner, and an unresolved/multiple-candidate canonical representation where possible;
- lower-authority conflicting evidence is retained and diagnosed but does not replace higher authority;
- provider-version skew outside the installed compatible bundle rejects the batch;
- display names and provider-local IDs never break ties.

Duplicate canonical preimages with unequal payloads are conflicts. Duplicate equal payloads coalesce with all evidence IDs.
## AC-G-42 — Derivation materialization matrix
### Decision

Every derived family has one required placement and authority. Query-time execution may compute additional views but never creates a competing canonical materialized fact family.

### Contract

| Derived family | Default precision | Placement | Canonical authority |
|---|---|---|---|
| owner-local CFG | language-specific exact/custom | publication + hot overlay | CPG CFG builder / MIR normalizer |
| normal/exception/unwind edge classification | exact/modelled | publication + overlay | CFG registry implementation |
| dominators/post-dominators | `BALANCED_V1` | publication + overlay | registered owner-local solver |
| control dependence | `BALANCED_V1` | publication + overlay | registered owner-local solver |
| loop headers/nesting | `BALANCED_V1` | publication + overlay | registered CFG solver |
| reaching definitions | `BALANCED_V1` | publication + overlay | registered dataflow solver |
| liveness | `BALANCED_V1` | publication + overlay | registered dataflow solver |
| direct def-use/data dependency | `BALANCED_V1` | publication + overlay | registered dataflow solver |
| points-to/alias sets | `BALANCED_V1` | publication + overlay | registered points-to solver |
| Rust move/borrow/ownership state | compiler exact plus profile fallback | publication + overlay | rustc/MIR/borrow adapter then reconciler |
| direct effects/resources | exact/modelled | publication + overlay | effect extractor/model reconciler |
| callable direct summaries | `BALANCED_V1` | publication + overlay | summary engine |
| transitive summaries | `BALANCED_V1` | publication + overlay | SCC/fixpoint summary engine |
| call SCC/recursion membership | `CALL_SOUND_V1` | full-table replacement per snapshot | graph derivation engine |
| type hierarchy SCCs where applicable | registered type projection | full-table replacement | graph derivation engine |
| arbitrary bounded reachability | request projection | query time | graph execution operator |
| connecting paths/path enumeration | request projection | query time | graph execution operator |
| ad hoc filtered transitive closure | request projection | query time, optionally plan-cache only | graph execution operator |
| deterministic count/group summaries | request-specific | query time | DataFusion plan |
| source context/snippets | n/a | query time from pinned source images | source-context service |

Query-time reachability/path results are response facts/paths, not durable canonical relation facts unless the derivation registry separately names a materialized closure family.

A derivation implementation is selected by `(derivation_family, profile_id, bundle_digest)`. Two implementations may be tested for equivalence, but only one is active authority in a snapshot.

## Cross-layer integration obligations

The following architecture-completion contracts are owned by another 1.3 artifact but are binding inputs to this specification. This document SHALL consume the named contract and SHALL NOT restate it with different semantics.

| Gap | Contract | Permanent owner | Integration obligation in this document |
|---|---|---|---|
| `G-09` | Generalized source-instance identity | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-12` | File identity across replacement, rename, and move | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-13` | Canonical ID preimage serialization | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-14` | Analysis-context discovery, identity, and selection | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-15` | Canonical type algebra | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-16` | External dependency identity and body policy | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-17` | Cross-language and FFI linking profile | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-18` | Path canonicalization, display, URI, and ordering | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-27` | Operational-state persistence | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-29` | Logical multi-file edit batches and publication barriers | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-39` | Derived-analysis precision profiles | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-41` | Operational dependency graph schema and update algorithm | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-43` | Unsupported, oversized, binary, generated, and vendored files | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-47` | Result-reference role type system and selector grammar | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-48` | Completeness and negative-proof algebra | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-49` | Entity matching, qualified-name parsing, grouping, and ranking | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-50` | Semantic source-boundary compiler | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-51` | Multi-context query semantics | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-52` | Query cost model, defaults, and hard limits | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-53` | Canonical JSON and checksum contract | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-54` | Canonical human-readable fact statements | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-55` | Source-context wire encoding | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-56` | Streaming, chunk interning, terminal completeness, and resumability | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-57` | Query plan cache contract | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-70` | Machine ontology registry | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-71` | Property schema, value types, cardinality, null, and storage mapping | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-72` | Mandatory conformance profiles | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-73` | Unknown entities, unknown remainder, and explicit negative facts | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-74` | Graph projection registry | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-75` | Interprocedural summary semantics registry | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-76` | Static concurrency and happens-before semantics | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |
| `G-77` | Effect and resource model semantics | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Persist, validate, merge, plan, materialize, and expose the contract without semantic reinterpretation. |

## Release conformance obligations

This specification inherits `G-78` through `G-84` from the suite governance and release manifest. Release acceptance SHALL include the portions of the golden corpus, clean-rebuild comparator, conformance harness, deterministic fault matrix, performance profiles, upgrade choreography, and adversarial security corpus that exercise Arrow/Delta schemas, overlay merge, publications, snapshots, reconciliation, materialization, canonical JSON production, and clean-rebuild equality.

A passing prose review is insufficient. The corresponding generated registries, schemas, protocol descriptors, fixtures, canonical outputs, and fault oracles SHALL pass the master release gates before an implementation may claim CodeFabric 1.3 conformance.

# Appendix A — Table Dependency Order

```text
repository
  ↓
owner, source_file
  ↓
source_token, source_annotation, syntax_detail
  ↓
semantic_detail, scope_detail, binding_detail, reference_detail, module_import_detail
  ↓
type_detail, type_fact_detail, member_relation_detail
  ↓
callable_detail, parameter_detail, call_site_detail, call_argument_detail, call_target_detail
  ↓
cfg_graph, cfg_node_detail, cfg_edge_detail
  ↓
value_detail, operation_detail, dataflow_event_detail
  ↓
memory_location_detail, access_path_component, memory_access_detail, program_state_detail
  ↓
effect_detail, exception_detail, resource_event_detail, async_event_detail, capture_detail
  ↓
Python/Rust/generated extension tables
  ↓
entity, relation canonical reconciliation
  ↓
derived_component, metric, callable_summary, derived relations
  ↓
publication_table
  ↓
publication COMPLETE
  ↓
current_publication
```

Implementations MAY write canonical `entity` and `relation` earlier, but publication dependencies SHALL reflect all extension and derived tables required by the active schema bundle.

---

# Appendix B — Default Table Properties

Starting defaults to benchmark:

```text
Delta CDF                         disabled
Delta column mapping              none
Delta type widening               disabled
Parquet compression               ZSTD
Target file size                  256 MiB for large fact tables
Target file size                  128 MiB for medium tables
Parquet row group                 64 MiB
Arrow schema metadata             enabled
Owner bucket count                256
DataFusion batch size             65,536
DataFusion target partitions      available parallelism, workload-adjusted
Memory pool                       limited
Spill directory                   configured and bounded
Optimize concurrency              4–8 tasks initially
Vacuum                            dry-run first; preserve manifest-pinned versions
```

---

# Appendix C — Mandatory Invariants

```text
1. Every published table version is pinned by one complete publication manifest.
2. Query sessions never mix table versions from different publications.
3. Every graph entity and relation uses a deterministic application-owned ID.
4. Every entity/relation belongs to one deterministic owner.
5. Every hot fact table has a typed Arrow/Delta schema; no EAV canonical store exists.
6. Every relation endpoint exists or points to an explicit unknown entity.
7. Exact, may, and unknown relationships remain distinguishable.
8. Direct and transitive effects remain distinguishable.
9. Source spans are byte-based and validated against current source bytes.
10. Delta schemas are table contracts; Arrow schemas are batch contracts; Parquet schemas are physical-file contracts.
11. Schema evolution is explicit and versioned.
12. Owner replacement is invisible until publication activation.
13. Custom DataFusion graph operators obey memory, spill, cancellation, streaming, and PlanProperties contracts.
14. Global transitive closure is not materialized without a bounded, demonstrated need.
15. Old Delta versions are operational state, not exposed semantic history.
16. No canonical table contains engineering recommendations or evaluative conclusions.
```

---

# Appendix D — Explicit Non-Outputs

The data fabric SHALL NOT create canonical tables or fields for:

```text
refactor safety
test impact
coverage
runtime profiling
historical change analysis
risk scores
bug likelihood
architecture quality
vulnerability exploitability
recommendations
remediation plans
change prioritization
```

Such products may be built later as downstream analyses over this factual fabric, but they are outside this specification.
