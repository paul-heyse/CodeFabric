---
artifact: design-dossier
design_id: codefabric-compiled-release-provider-fabric-runtime-boundary
version: v1
date: 2026-09-02
status: accepted
baseline_commit: de5db65c2834458eb57c7133183b8cef67a2491a
working_tree_digest: 0857debec2e29aec5ad593951c6b761f8ed14b36709c6eccb948897754196604
primary_scope:
  - Cargo.toml
  - src/lib.rs
  - src/fabric.rs
  - src/fabric/
  - src/ruff_adapter.rs
  - src/tree_sitter_adapter.rs
  - src/provider_native_syntax.rs
  - src/provider_raw_kinds.rs
  - src/production_provider_recipe.rs
  - src/pyrefly_service.rs
  - src/rustc_service.rs
  - src/query_service.rs
  - src/daemon.rs
  - src/supervisor.rs
  - src/bin/
  - rustc-extractor/
  - pyrefly-sidecar/
  - contracts/rpc/
  - codefabric-cpg-mcp/
  - rules/
  - scripts/
  - tooling/ci/
  - tests/
  - justfile
doctrine_path: docs/library_ref/full_data_fabric_design_principles_v2.md
design_inputs:
  - docs/designs/codefabric_execution_proved_relational_data_fabric_design_v3_2026-08-30.md
  - docs/reviews/interface_design_review_daemon_grpc_fastmcp_boundary_2026-09-01_v5.md
  - docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v5_2026-09-01.md
  - docs/authoritative_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.3.md
  - docs/authoritative_design/present_state_cpg_fact_generation_specification_python_rust_v2.3.md
  - docs/authoritative_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v2.3.md
  - docs/authoritative_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v2.3.md
  - docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.3.md
---

# CodeFabric compiled-release, provider, fabric, and runtime boundary design v1

## 1. Executive decision

The failing isolated feature build is an architectural signal, not a request for more conditional
compilation. The immediate failure is **not** a Protobuf, UDS, gRPC, or FastMCP mismatch. It occurs
one layer inward: fact-provider adapters and relational proof code import daemon-owned release
composition types. The current `CompiledSemanticRelease` then compounds the problem by carrying five
zero-sized authority markers while its actual provider, transformation, query, proof, policy, and
runtime behavior is distributed across daemon-gated modules and repeated global `current()` calls.

CodeFabric will retain the accepted product topology and make its internal dependency direction
real. The selected design is a layered single stable-root crate with:

1. application-owned provider jobs, observations, coverage, gaps, and provenance contracts;
2. provider adapters that consume only lane-specific jobs and emit owned Arrow batches or
   relation-scoped Arrow IPC;
3. an Arrow/DataFusion/Delta fabric engine that has no dependency on daemon, supervisor, gRPC,
   FastMCP, Git, SQLite, or concrete provider implementations;
4. one behavior-bearing `CompiledSemanticRelease`, compiled once at daemon startup and injected
   through application services, that privately owns the actual provider, transformation, query,
   proof, and policy programs required by SUITE §3 and FAB §1;
5. a daemon imperative shell that owns source I/O, process lifecycles, operational state, command
   actors, cancellation trees, activation, and task joining; and
6. thin generated-Protobuf/Tonic and Python `DaemonPort` adapters, with FastMCP remaining
   presentation-only.

The release capability remains non-forgeable by construction where Rust visibility can help, but
Rust privacy is not treated as correctness or authorization proof. Semantic authority is enforced
at provider-output admission, epoch proof, the sole `FabricCommand` mutation path, exact Delta
activation, and per-request authorization. Empty marker possession is removed as an acceptance
argument.

This dossier also closes four related defects exposed by the comprehensive library review:

- generated rustc Protobuf messages currently survive into admitted application state;
- query cancellation and spawned-task ownership are not one structured parent/child tree;
- `IdentityPreservingViewTable` implements the legacy DataFusion scan entry point and therefore
  does not explicitly preserve DataFusion 55 `ScanArgs` and `StatisticsRequest`; and
- the root daemon destroys the Pyrefly sidecar after every run even though the sidecar and GEN §7.3
  are designed around one long-lived incremental state per workspace/context.

No legacy-operability bridge is part of the target. The remaining v5 execution is paused because
the plan's own design-level replan trigger fired. Completed functional outcomes and beneficial
WP49 deletions are inputs to the successor implementation plan, not reasons to preserve the faulty
ownership structure. The successor plan must cut directly to this design and must not restore a
hash, generated registry, static schema bundle, comparator, or predecessor route merely to satisfy
stale packet wording.

### 1.1 What is preserved

The review found substantial target-quality implementation that should be retained:

- Arrow 59.2.0 as the canonical semantic data representation;
- DataFusion 55.0.0 sessions, catalogs, typed expressions, logical plans, streaming execution,
  least-authority child sessions, and plan-derived schema;
- exact-version delta-rs state, application-owned multi-table activation, zero library retries,
  exact commit readback, and unknown-outcome reconciliation;
- Tree-sitter and Ruff in-process adapters, the separately built Pyrefly and rustc process
  domains, raw-plus-normalized facts, explicit unknowns, and relation-scoped Arrow IPC;
- the released `codefabric.cpgd.v2` control contract, hermetic descriptor/code generation,
  private UDS, accepted query handles, resumable event streams, bounded resources, and explicit
  cancellation;
- one lifespan-owned Python daemon channel and a FastMCP 4 presentation surface with no Python
  semantic or data-plane authority; and
- one stable Cargo package, one stable library crate, the existing justified auxiliary Cargo
  roots, and thin operational binaries.

### 1.2 Current-tree evidence and trust boundary

The baseline is commit `de5db65c2834458eb57c7133183b8cef67a2491a` with a pre-existing dirty
WP49 tree. The recorded working-tree digest is SHA-256 over the task-relevant binary diff and
porcelain status, excluding the unrelated untracked `Untitled` file. It identifies the reviewed
state; it is not correctness evidence.

The decisive executable observation is:

```text
cargo check --locked --no-default-features --features fact-generation
  -> E0433: ruff_adapter/tree_sitter_adapter cannot import fabric::production_kernel
  -> E0432: they cannot import daemon-gated production_provider_recipe
```

The relevant live edges are:

```text
RuffAdapter / TreeSitterAdapter
  -> CompiledProviderAuthority
  -> CompiledProviderExecutionProfile / CompiledProviderLane
  -> daemon-gated production_provider_recipe and production_kernel
```

An interim `cfg(feature = "daemon")` patch lets isolated `data-fabric` compile by hiding
`proof` and `derived_producer_closure`, but it does not repair the dependency graph and it makes
the fabric capability less truthful. That patch is diagnostic evidence and must not become the
target architecture.

Further current-tree evidence:

- `COMPILED_SUITE_VERSION` and provider/query release literals remain `2.2.0` while the sole
  authoritative suite is v2.3; live reference projection consults that stale Rust value;
- `AcceptedRustcOwner` and `AcceptedRustcCompilation` retain generated `OwnerBegin`, `OwnerEnd`,
  `CompilationBegin`, and `CompilationEnd` messages after transport validation;
- `CompiledSemanticRelease::current()` is repeatedly reconstructed across daemon, query,
  provider, workspace, and test paths instead of being one injected compiled product;
- `data-fabric` depends on the broad `repository-state` feature, which unnecessarily brings both
  gix and SQLite into the fabric graph;
- `QueryCoordinator` stores `JoinHandle`s but terminal/expiry paths remove and abort them without
  a complete join discipline;
- `IdentityPreservingViewTable` delegates through `scan` while stronger wrappers already preserve
  DataFusion 55 structured scan arguments; and
- `DisposablePyreflySidecarProcess` is unconditionally terminated and joined after each analysis,
  forfeiting Pyrefly's native `Query::change_files` incremental state.

Current code is evidence of the change surface, not design authority. The v2.3 suite, accepted
product designs, pinned library references, and this dossier's selected boundaries govern the
successor implementation.

### 1.3 Scope and non-goals

This design changes ownership, construction, lifecycle, and proof boundaries. It does not change
the eight semantic query forms, public four-tool/two-resource FastMCP surface, released v2 wire
semantics, canonical fact ontology, or exact durable publication model.

Non-goals:

- no new Cargo root or package merely to enforce conceptual organization;
- no dynamic user-authored semantic release, plugin registry, YAML/JSON program loader, or hot
  suite switching;
- no DataFusion, Arrow, Delta, or semantic processing in Python;
- no Protobuf duplicate of semantic relations and no generated message in admitted domain state;
- no all-provider process migration without measured containment need;
- no replacement of exact Delta table/version authority with SQLite, filesystem listing, or a
  digest;
- no second production constructor, compatibility facade, dual run, or fallback to the current
  marker-token route; and
- no new hash used as proof. Existing hashes remain only where a released identity or integrity
  contract needs bytes; executable validation decides correctness.

## 2. Constraints and target invariants

### I-60 — Inward, acyclic dependency direction

Foundation contracts know no provider library, engine, storage, daemon, or transport. Provider
adapters depend on application provider contracts. The fabric engine depends on Arrow/DataFusion/
Delta and application contracts, not concrete providers or daemon modules. The compiled release
depends on provider and fabric capabilities. Daemon composition depends on all inward layers.
Tonic and FastMCP terminate at application ports. No inward layer imports an outward layer.

### I-61 — One behavior-bearing compiled release

`CompiledSemanticRelease` contains validated executable program values, not unit markers. It owns
one `SuiteIdentity` plus compiled provider, transformation, query, proof, and policy programs. A
single startup compile either returns a coherent immutable release or prevents readiness. The
same `Arc` is constructor-injected into startup, update, query, status, and reference services.

### I-62 — Provider execution is a job; authority is admission

A provider adapter receives an immutable application-owned lane job that binds source/context,
requested families/scopes, exact provider/protocol/schema identities, effective bounded resources,
deadline, cancellation, trust posture, and run identity. Parser construction receives no release
authority. Only release-owned admission can turn the resulting Arrow batches, coverage, gaps, and
diagnostics into candidate epoch inputs.

### I-63 — Arrow-native provider boundary

In-process providers emit Arrow batches with the same session-derived `SchemaContract` used by
out-of-process relation-scoped Arrow IPC. Provider-native objects, borrowed nodes, compiler values,
generated Protobuf messages, semantic JSON blobs, and debug text cannot cross the adapter boundary.
Every stream/batch set has terminal coverage, remainder, diagnostics, source/context/run pins, and
bounded row/byte accounting.

### I-64 — Provider-native lifecycle without provider authority leakage

Tree-sitter and Ruff may retain bounded revision-native state inside their adapters. One contained
Pyrefly sidecar per workspace/context retains one long-lived `Query`/state and advances through
change batches. Rustc objects remain callback/process-local. All adapters expose only application-
owned jobs, observations, gaps, and Arrow relations. Provider failure never mutates the fabric.

### I-65 — DataFusion remains the visible relational compiler and executor

Typed programs compile to native `Expr` and `LogicalPlan` values, schemas come from admitted Arrow
and built plans, and execution remains streamed. Provider/view wrappers preserve projection,
filters, limits, structured statistics requests, ordering, partitioning, equivalence properties,
metrics, and cancellation. A custom physical operator is allowed only at the lowest necessary
extension level and must expose the full optimizer/execution contract.

### I-66 — Delta and temporal state stay separate

Delta owns exact single-table durable state and versions. The application activation event owns
the exact multi-table vector. SQLite owns only reconstructible temporal control such as queues,
attempts, checkpoints, leases, and progress. gix owns repository observation only. The fabric
feature cannot acquire gix or SQLite merely because the daemon uses them.

### I-67 — Structured cancellation and task ownership

One daemon parent cancellation tree owns child tokens for every accepted query, provider run,
DataFusion execution, materialization, and result operation. CodeFabric durable cancellation IDs,
idempotent `CancelQuery`, terminal state, cleanup reserve, and restart behavior remain application
semantics. Every spawned task has one owner, cancellation path, terminal observation, and join.
Synchronous provider loops receive a bounded application cancellation probe.

### I-68 — Transport types terminate at transport

Generated Prost/Tonic types exist only in generated modules and transport adapters. Handlers
validate/authorize and convert them to application-owned control values. Application results are
converted back at the boundary. Opaque Arrow IPC bytes remain opaque; the rule does not introduce a
redundant semantic DTO conversion. gRPC cancellation stops a transport leg; only explicit durable
query cancellation stops accepted logical work.

### I-69 — One categorical identity per concept

Suite, application, adapter, provider, query-program, proof-program, wire, and storage versions are
distinct typed concepts. The suite identity is declared once in the release definition and
projected into epochs/status/reference data. Subprograms bind to it structurally rather than
embedding duplicated suite-version strings. The FastMCP package version and Protobuf wire version
are not silently conflated with the suite version.

### I-70 — Hard cutover and physical decommission

The new release compiler, provider jobs, feature boundaries, and injected application services
replace the current route atomically. Empty authority tokens, global `current()` lookups,
daemon-owned adapter profiles, leaked generated messages, disposable Pyrefly ownership, stale
v2.2 runtime literals, and the interim core-module `cfg` workaround are physically absent after
cutover. There is no dormant fallback.

### I-71 — Executable architecture and causal proof

Every feature is independently useful and independently compiled. Structural rules reject reverse
imports and provider/generated type leakage. Behavior tests prove that changing a release program
operand changes or rejects execution, that invalid provider output cannot be admitted, that exact
Delta pins survive restart, that cancellation joins all owned work, and that transport/presentation
cannot author semantics. Digests and source scans support those oracles but never substitute for
them.

## 3. Target architecture

### 3.1 Dependency and execution shape

Arrows below point from a consumer to what it depends on. The conceptual layers do not prescribe
source-file or directory names.

```text
FastMCP 4 presentation
  -> Python DaemonPort / one grpc.aio channel
    -> thin Tonic transport adapter / generated v2 control types
      -> daemon application services and structured task tree
        -> one injected Arc<CompiledSemanticRelease>
          -> CompiledProviderProgram -> application ProviderJob contracts
          -> CompiledTransformationProgram
          -> CompiledQueryProgram
          -> CompiledProofProgram
          -> CompiledPolicyProgram
        -> provider adapters -> owned Arrow batches / relation-scoped Arrow IPC
        -> Arrow/DataFusion fabric engine -> exact delta-rs snapshots and commits
        -> repository-input and operational-state adapters

foundation: typed identity, schema, command, policy, coverage, gap, and provenance values
```

The execution path remains:

```text
immutable source/context
  -> release-prepared provider jobs
  -> provider-native execution
  -> Arrow batches + coverage/gap/provenance
  -> release-owned admission and typed transformations
  -> programmatic DataFusion candidate
  -> independent relational proof
  -> exact Delta publication vector
  -> one activation event
  -> authorized child session
  -> daemon query package
  -> bounded gRPC delivery
  -> FastMCP presentation
```

### 3.2 Authority and representation map

| Concept | Authority | Runtime/derived form | Never authority |
|---|---|---|---|
| suite semantics | compiled release programs | injected immutable release | unit token, version string, digest |
| provider request | release-prepared `ProviderJob` | adapter-specific invocation | daemon global, parser constructor |
| provider observation | accepted Arrow batch + terminal coverage | candidate relation | provider object, generated message |
| relation schema | admitted Arrow schema / built logical-plan schema | `SchemaContract`, `DFSchema` | static generated registry |
| transformation/query/proof | compiled behavior-bearing program | `Expr`, `LogicalPlan`, expectation/fault relations | SQL text, marker type |
| current table state | exact Delta version | exact-version provider | object listing, SQLite row |
| current fabric state | exact activation-selected vector | immutable `Arc<FabricEpoch>` | latest lookup, process pointer |
| temporal coordination | application state machine | SQLite record/lease/checkpoint | semantic relation authority |
| repository observation | immutable source image + gix acceleration | source/job inputs | gix object identity as canonical ID |
| cancellation | durable query identity + in-memory token tree | child token/probe/process cancel | dropped stream alone |
| wire contract | released `.proto` + descriptor policy | generated Rust/Python types | generated source edits |
| presentation | daemon canonical response | Pydantic/FastMCP projection | Python cache/catalog/session |
| correctness | independent executable expectations and faults | proof relations and terminal result | hash, capture, self-golden |

### 3.3 Compiled release construction

The production composition root performs one fallible compile before workspace readiness:

```text
CurrentSemanticReleaseDefinition
  -> validate categorical identities and closed program graph
  -> compile provider/transformation/query/proof/policy programs
  -> cross-check relation, field, schema, dependency, coverage, and provenance closure
  -> CompiledSemanticRelease
  -> Arc injected into every application service
```

The release owns actual immutable program values:

```text
CompiledSemanticRelease
  suite: SuiteIdentity
  providers: CompiledProviderProgram
  transformations: CompiledTransformationProgram
  queries: CompiledQueryProgram
  proof: CompiledProofProgram
  policy: CompiledPolicyProgram
```

Each program has private fields and exposes narrow behavior such as preparing a provider job,
admitting output, adding transformations, compiling a semantic query, constructing independent
proof inputs/faults, or reducing a child-session policy. Callers cannot submit an alternative
program set. A program is causally read during execution; no `_authority` parameter is accepted
and ignored.

The definition is closed Rust code, not a generated file or runtime registry. Compilation derives
inventories and observation relations from program values and the installed DataFusion session.
It does not reintroduce a schema digest as semantic authority. If a released boundary requires an
identity byte sequence, that identity is derived once and used only for matching, provenance, and
reproduction; schema equality, execution, and proof remain the validators.

The release is not globally discoverable. `CompiledSemanticRelease::current()` is replaced by one
composition-root construction and constructor injection. Tests may compile explicit fixture
definitions; production has exactly one definition and no runtime selector. A future requirement
for co-resident or hot-swapped suites is a replan trigger, not a reason to add a registry now.

### 3.4 Provider job, execution-policy, and admission boundary

The current `CompiledProviderExecutionProfile` mixes three concerns. They separate as follows:

| Concern | Target owner |
|---|---|
| provider/API/build identity, supported relation families, schemas, field roles, authority, coverage and unknown semantics | `CompiledProviderProgram` |
| policy ceilings for bytes, rows, work, depth, workers, retention, wall time, cancellation acknowledgement and trust | `CompiledPolicyProgram` |
| effective source/context/run values and limits for one invocation | immutable `ProviderJob` |

The daemon supplies operational source images, requested scope, current time budget, deployment
resource envelope, and trust material. The release intersects those values with compiled policy
and emits a lane-specific validated job. The effective values are recorded in provenance. An
adapter cannot widen them and an operator cannot replace semantic provider/schema authority.

Tree-sitter and Ruff constructors receive only exact adapter configuration needed to validate the
loaded library/grammar and establish bounded native state. Their run methods consume jobs. Pyrefly
and rustc control messages carry the job's control fields and return relation-scoped Arrow IPC.
The in-process lanes construct equivalent Arrow batches directly. All four lanes terminate in one
application-owned result shape:

```text
ProviderRunResult
  run/source/context/provider/schema pins
  relation batches or streams
  requested/completed/remainder/unknown coverage
  diagnostics and trust/resource outcome
  terminal status
```

Admission validates every field, schema, pin, count, bound, sequence, terminal, and provenance
edge before a candidate builder can consume it. The release-to-admission boundary, not possession
of a parser-construction token, is the construction choke point required by P32.

Provider library types remain adapter-private. The public `provider_raw_kinds` helpers that accept
`tree_sitter::Language`, Ruff `NodeKind`, or Ruff `TokenKind` move behind the corresponding adapter;
only application-owned raw-kind entries and Arrow rows escape. Generated rustc begin/end messages
are converted immediately into application-owned compilation/owner headers and terminal values.

### 3.5 Provider-native incremental lifecycle

Tree-sitter retains only its bounded revision cache and reuses changed ranges. Ruff retains a
bounded parsed revision but reparses whole files according to the provider capability. Pyrefly
changes materially: the supervisor owns one contained sidecar per workspace and compatible
analysis context. Its lifecycle is:

```text
spawn -> validate exact build/protocol -> initialize context -> add files
      -> analyze generation N -> change_files/add_files -> analyze N+1 -> ... -> drain/join
```

Context/search-path/interpreter/typeshed/config changes create a new context and controlled
reconstruction. A normal run cancellation does not destroy healthy workspace state. Protocol
corruption, panic, unresponsive cancellation, trust loss, or context mismatch terminates the
process group, emits an explicit capability gap, and reconstructs from immutable source/context
inputs. Syntax lanes remain available while semantic capability is degraded. A shared multi-
workspace sidecar is excluded until memory evidence justifies its isolation cost.

Rustc remains a per-compilation contained process because compiler execution has a different trust
and lifecycle model. No compiler value crosses the callback/process boundary; admitted owner and
compilation records are application values.

### 3.6 Arrow and DataFusion fabric core

The fabric engine owns canonical schemas, `RecordBatch` validation, programmatic session assembly,
catalog/provider installation, expression/logical-plan compilation, exact-provider wrappers,
execution streaming, child-session reduction, and relational proof evaluation. It accepts
synthetic application-owned batches in isolated tests and therefore remains useful without any
concrete provider library or daemon.

Provider and view wrappers use the DataFusion 55 structured scan surface. In particular,
`IdentityPreservingViewTable` must preserve all incoming `ScanArgs`: projection, filters, limit,
and `StatisticsRequest`. Its schema-identity wrapper must preserve values, partitioning, ordering,
equivalence properties, metrics, and optimizer sequencing. Existing `SchemaIdentityExec` remains
justified only as a narrow metadata-identity operator; it cannot conceal a semantic calculation.

Generic proof and derived-closure mechanics remain inward:

- generic relation/closure compilation, execution, and result decoding belong to the fabric
  engine;
- release-specific relation IDs, expectations, fault programs, and query-family selection belong
  to `CompiledProofProgram` and `CompiledQueryProgram`; and
- daemon startup invokes those programs but does not define them.

This split replaces both extremes: blanket daemon-gating of proof/closure and ungating the current
monolithic `production_kernel`.

### 3.7 Delta, repository input, and operational state

The current Delta path is retained. A mutation uses the exact session-bound logical plan, one
application transaction identity, predecessor checks, zero delta-rs retries, exact commit
readback, and unknown-outcome reconciliation. Queries reopen the activation-selected exact
version vector. Partial table commits never become visible before the application activation
event. Provider refresh is explicit.

The direct feature graph separates three state concerns:

- `data-fabric`: Arrow/DataFusion/Delta/object-store state and exact table/version behavior;
- `repository-input`: gix plus descriptor-relative source observation needed to create immutable
  source inputs; and
- `operational-state`: SQLite/rustix storage for queues, attempts, leases, checkpoints, workspace
  records, and progress.

The daemon composes all three through application-owned ports. `data-fabric` cannot depend on
`repository-input` or `operational-state`. SQLite data required for restart is still durable
operational evidence, but it cannot select semantic table state or substitute for Delta
activation.

### 3.8 Structured cancellation, task ownership, and streaming

The daemon directly adopts exact `tokio-util` 0.7.19 with its `rt` feature for process-local
`CancellationToken` trees. It remains a daemon capability, not a fact-generation dependency.
CodeFabric retains an application wrapper for durable IDs, budgets, terminal semantics, and the
bounded synchronous probe used by Tree-sitter/Ruff/gix loops.

```text
daemon shutdown token
  -> workspace token
    -> accepted query token
      -> freshness/provider token
      -> DataFusion execution token
      -> materialization/package token
      -> resource-read token
```

Every `tokio::spawn` in the production path is registered with an owner that cancels and awaits it.
Terminal, expiry, shutdown, and failure paths cannot merely remove a `JoinHandle`. Draining stops
new admission, signals the parent token, reserves cleanup time, joins children, releases engine/
provider/result resources, persists terminal state where required, and only then closes services.

Tonic streams use direct composition or small bounded `mpsc` channels and preserve HTTP/2
backpressure. There is no unbounded bridge queue, generic Tower retry, or detached retention task.
A dropped `WatchQuery` ends observation only. Explicit `CancelQuery` addresses accepted work and
survives reconnect semantics.

### 3.9 gRPC and FastMCP boundary

The released v2 Protobuf remains the canonical control plane. It strongly types compatibility,
authentication context, authority generations, accepted handles, progress, bounded resource
descriptors, cancellation, and terminal state. Semantic request/response meaning remains in the
daemon's canonical contracts and Arrow relations.

`ProductionQueryService` separates conceptually into:

```text
thin Tonic handler
  -> validate transport shape, peer/session authority, message bounds and deadline
  -> convert to application control value
  -> call query application service with injected release/workspace ports
  -> map application outcome to generated response/status
```

Generated types terminate there. The Rust/Python code generators and descriptor set remain
derived from the one `.proto`; generated source is not edited. The one long-lived UDS channel,
accepted-handle/event-stream model, explicit cancel/resume, bounded resource reads, peer
credentials, launch grants, and supervisor topology remain unchanged.

The Python adapter continues to own only presentation, strict Pydantic validation/projection,
FastMCP context, and one lifespan-owned `DaemonPort`. It does not load the compiled release,
provider inventories, Arrow, DataFusion, Delta, SQLite, or a semantic catalog. Suite identity is
daemon-authored live data. The FastMCP package/server version remains a separate identity.

### 3.10 Target feature lattice

Cargo features remain additive under resolver 3 and describe compiled possibility, not runtime
semantic capability. The target lattice is:

```text
canonical-json
  -> contract-models

provider-contracts
  -> contract-models + minimal Arrow array/schema surface

fact-generation
  -> provider-contracts + Tree-sitter/Ruff/Rayon/provider-side petgraph

data-fabric
  -> contract-models + provider-contracts
  -> Arrow/Parquet/DataFusion/delta-rs/object_store/fabric-side petgraph/Tokio

repository-input
  -> contract-models + gix/rustix/url

operational-state
  -> contract-models + rusqlite/rustix/url

rpc
  -> Prost/Tonic/Tokio generated-control substrate

semantic-release
  -> fact-generation + data-fabric

daemon
  -> semantic-release + repository-input + operational-state + rpc
  -> ArcSwap/notify/process/supervisor/cancellation/health dependencies

compatibility-probes
  -> explicit pinned-library probe aggregate only

local-workstation
  -> daemon + compatibility-probes

s3-storage
  -> data-fabric + explicit delta-rs S3 activation
```

The exact final feature spelling is an implementation-plan concern, but these dependency edges are
not optional. `provider-contracts` may be implemented as an internal feature/module boundary in the
same package; it does not justify a new crate. Broad Tokio capabilities are activated only by the
features that use them.

An isolated feature must prove useful behavior, not merely compile after important modules are
hidden:

- `fact-generation` constructs exact in-process adapters, executes provider fixtures, and emits
  owned Arrow batches/coverage without DataFusion, Delta, gix, SQLite, Tonic, or daemon;
- `data-fabric` builds and proves a candidate from synthetic Arrow inputs and exact Delta fixtures
  without concrete provider libraries, gix, SQLite, Tonic, ArcSwap, or daemon;
- `semantic-release` compiles a complete release and executes a provider-to-proof fixture without
  repository, operational-state, RPC, or supervisor dependencies;
- `rpc` proves generated wire and UDS transport contracts without DataFusion, Delta, provider
  libraries, or semantic-release; and
- `daemon` is the only stable-root aggregate that may compose all of them.

Runtime capability is a relational observation over the resolved feature graph, compiled release,
installed providers/session/catalog/functions, deployment policy, current exact identities,
coverage/gaps, and proof. No static feature flag or capability registry can claim availability.

### 3.11 State and resource ownership

| State/resource | Owner | Lifetime | Durable authority |
|---|---|---|---|
| compiled release programs | daemon application composition | process/binary release | no; bound into epoch/release provenance |
| provider job | update wave/provider run | one bounded run | job identity/provenance only |
| Tree-sitter/Ruff native state | adapter worker | bounded revisions | no |
| Pyrefly `Query`/context | workspace sidecar supervisor | compatible workspace/context | no; outputs/gaps are published facts |
| rustc compiler values | extractor callback/process | one compilation | no |
| candidate Arrow batches/session | epoch builder | one candidate | no until exact publication |
| active `FabricEpoch` | workspace authority | immutable selected epoch | selection is Delta activation authority |
| Delta table/version | delta-rs transaction log | retained history | yes, per table |
| activation vector/event | application activation relation | retained history | yes, multi-table selection |
| SQLite queues/leases/checkpoints | operational-state adapters | restart-bounded/retained policy | operational only |
| query cancellation token/tree | daemon/query owner | accepted query/process generation | no; durable cancel/terminal records are application state |
| Tonic stream/channel | transport adapter | connection/request | no |
| FastMCP context/Pydantic model | Python presentation | request/response | no |

## 4. Library decisions

### LD-40 — Arrow 59.2.0 is the provider and fabric data boundary

**Decision:** retain-current

**Version basis:** Arrow/Parquet 59.2.0 from FAB §2.1 and the live root/extractor manifests.

**Displaces:** intermediate provider-object/DTO authority in `src/provider_native_syntax.rs` while
retaining application-owned job, coverage, gap, provenance, and batch contracts.

**Risk:** premature batch materialization could add copies or memory pressure; bounded builders,
streaming, row/byte ceilings, and workload measurement mitigate it.

**Validation:** `just provider-job-contract-check`; `just provider-ipc-contract-integrity-check`.

Retain and widen correct use. In-process provider output becomes Arrow-native at the
fact-generation boundary; out-of-process providers retain relation-scoped Arrow IPC. Arrow schema,
arrays, `RecordBatch`, null semantics, buffers, builders, stream readers/writers, and bounded batch
iteration carry semantic data. Application metadata and `SchemaContract` carry field meaning.

**Selected capabilities.** ARR-01–ARR-05 and ARR-08; schema metadata and nested/null validation;
bounded stream consumption; exact shared Arrow universe across roots.

**Rejected.** Provider objects, one-row Protobuf facts, JSON semantic payloads, raw Parquet as
logical state, or an intermediate DTO census as production authority.

**Placement and obligations.** Minimal Arrow array/schema crates are allowed in provider contracts/
fact generation; DataFusion and Delta remain absent there. Batch builders pre-size where measured,
validate lengths/types/nullability/metadata, and preserve raw-plus-normalized observations. Oracles:
`just provider-job-contract-check` and `just provider-ipc-contract-integrity-check`.

### LD-41 — DataFusion 55.0.0 remains the semantic compiler and executor

**Decision:** retain-current

**Version basis:** DataFusion 55.0.0 with Arrow/Parquet 59.2.0 and `object_store` 0.13.2 from FAB
§2.1 and the resolved root graph.

**Displaces:** daemon-owned generic proof/closure behavior and legacy `scan` delegation in
`src/fabric/programmatic_schema.rs`; retains the programmatic fabric engine and its native plans.

**Risk:** a wrapper or custom node can conceal optimizer inputs/properties; `ScanArgs` spies,
property/value tests, and the highest-viable-extension rule mitigate it.

**Validation:** `just datafusion-scan-contract-check`;
`just datafusion-plan-schema-cache-check`.

Retain DataFusion sessions, catalogs, providers, expressions, logical plans, native
optimizer visibility, structured scans, streaming execution, statistics, runtime resources, and
least-authority child sessions. Move release-specific programs above the generic engine instead of
moving engine behavior into daemon code.

**Selected capabilities.** MOD-02–MOD-05 and MOD-07; CAT-03–CAT-07 and CAT-10; LOG-01–LOG-07;
PHY streaming/resource contracts; RUN session/runtime/resource ownership; OBS plan/provider
observations; GOV and TST contract-derived checks. `ScanArgs` and `StatisticsRequest` are preserved
through every wrapper.

**Rejected.** SQL strings, serialized plans, hidden optimizer nodes, eager full-result collection,
physical-plan caches, or a custom operator where a native expression/provider is sufficient.

**Placement and obligations.** The fabric engine owns the DataFusion types. The compiled release
owns typed program construction. The daemon owns operational budgets, not semantic plan shape.
Oracles: `just datafusion-scan-contract-check`, `just datafusion-plan-schema-cache-check`, and
`just scheduled-streamed-semantic-query-check`.

### LD-42 — delta-rs at `43a0cf10…` owns exact single-table durable state

**Decision:** retain-current

**Version basis:** delta-rs 1.0.0 at exact revision
`43a0cf10a313e5077c48637ad786a05359136bbb`, DataFusion 55.0.0, Arrow/Parquet 59.2.0, and
`object_store` 0.13.2 from FAB §2.1.

**Displaces:** no correct Delta path; it prevents `repository-state`, SQLite, raw listings, or an
implicit-latest provider from substituting for exact table/version and activation authority.

**Risk:** partial multi-table publication and unknown commit outcomes remain application concerns;
zero library retries, exact readback, reconciliation, and activation-vector tests mitigate them.

**Validation:** `just delta-publication-contract-check`;
`just delta-durability-protocol-integrity-check`.

Retain the current exact-version, zero-library-retry design. delta-rs owns transaction
logs, snapshots, protocol/features, table metadata, per-table optimistic commits, CDF, and exact-
version providers. CodeFabric owns multi-table publication, idempotency, retry/reconciliation,
schema contracts, provenance, retention policy, and activation.

**Selected capabilities.** exact snapshot/time travel (STA); protocol/feature validation (SCH/GOV);
exact read snapshot and application transaction identity (TXN); high-level writes/DML; exact-
version DataFusion provider binding (QRY); history/CDF/provenance (OBS); contract-derived conflict,
restart, and compatibility tests (TST).

**Rejected.** Implicit latest semantics, raw file listing, automatic provider refresh, blind
library retry, cross-table atomicity claims, checkpoint identity, or SQLite selection of current
tables.

**Placement and obligations.** Delta stays in `data-fabric`; operational adapters remain outside.
Every mutation records before/after exact versions and readback/reconciliation outcome. Oracles:
`just delta-publication-contract-check`, `just delta-durability-protocol-integrity-check`, and
`just delta-exact-reconstruction-v4-check`.

### LD-43 — Tonic/Prost own transport; Tokio owns process-local async structure

**Decision:** wrap

**Version basis:** Tonic/tonic-prost 0.14.6, Prost 0.14.4, Tokio from the resolved root graph, and
direct `tokio-util` 0.7.19 with `rt` for daemon cancellation.

**Displaces:** generated messages in admitted Rust state, combined Tonic/application service logic,
ad hoc atomic-only async cancellation, and abort/drop-without-join task ownership.

**Risk:** transport cancellation and durable query cancellation can be conflated, or cancellation
bridges can detach; distinct application semantics, token ownership, bounded channels, cleanup
reserve, and join assertions mitigate it.

**Validation:** `just proto-check`; `just generated-type-boundary-check`;
`just cancellation-tree-check`; `just grpc-slow-consumer-check`.

Retain Tonic 0.14.6, Prost 0.14.4, the released descriptors, private UDS, one channel,
accepted handles, unary control RPCs, unary-stream observations/resources, and bounded backpressure.
Tighten the generated-type boundary and directly adopt `tokio-util` 0.7.19 `CancellationToken` in
the daemon.

**Selected capabilities.** Tonic reference §§0.1, 8.5, 15.2–15.3, 18–19, 23–28, 34–40:
descriptor authority, thin handlers, UDS peer identity, explicit deadline budgets, bounded streams,
structured cancellation, owned tasks, cross-language fixtures, and graceful drain/resume.

**Rejected.** Protobuf semantic-model duplication, generated application DTOs, bidirectional
streaming without need, unbounded bridge queues, generic retries, service-mesh machinery, TLS for
the current private same-user UDS profile, or stream-drop as logical cancellation.

**Placement and obligations.** Generated messages end at transport/provider-process adapters.
Application services use owned values. Every task is cancelled and joined. Oracles:
`just proto-check`, `just generated-type-boundary-check`,
`just cancellation-tree-check`, and `just grpc-slow-consumer-check`.

### LD-44 — Provider libraries retain native strengths behind application adapters

**Decision:** wrap

**Version basis:** Tree-sitter 0.26.12 with Python grammar 0.25.0 and Rust grammar 0.24.2, Ruff
0.0.7, Pyrefly 1.2.0 at revision `1933169ad8ee9e4d4114112eb56ef0811fb0a094`, and the
dated-nightly rustc extractor declared by the v2.3 suite and live manifests.

**Displaces:** daemon release tokens at adapter construction, public provider-native type helpers,
generated compiler messages in application state, and disposable Pyrefly process ownership.

**Risk:** long-lived native state can become stale or exceed resources; exact context/source pins,
bounded caches, capability gaps, supervised restart, and clean-vs-incremental equivalence mitigate
it.

**Validation:** `just provider-type-boundary-check`; `just exact-provider-batch-check`;
`just pyrefly-incremental-lifecycle-check`.

Retain direct Tree-sitter 0.26.12 and Ruff 0.0.7 in the stable root, pinned Pyrefly in
its sidecar, and the dated-nightly rustc extractor. Tighten provider object isolation and use each
provider's native lifecycle rather than flattening them into one execution pattern.

**Selected capabilities.** Tree-sitter incremental edits/changed ranges/error recovery and queries;
Ruff typed AST/tokens/trivia/scopes with whole-file parse; one long-lived Pyrefly `Query` per
workspace/context using `change_files` and bulk queries; rustc public/private extraction confined
to the compiler callback; owned Arrow batches, application identity, explicit unknowns, and
provider-authority reconciliation.

**Rejected.** Provider-local IDs as canonical identity, borrowed/library types across boundaries,
disposable Pyrefly runs, text MIR, silence on compile/type failure, or making every provider a
process solely for symmetry.

**Placement and obligations.** Provider adapters consume `ProviderJob`; release admission consumes
only owned result relations. Oracles: `just provider-type-boundary-check`,
`just exact-provider-batch-check`, `just pyrefly-incremental-lifecycle-check`, and
`just provider-trust-coverage-remainder-check`.

### 4.6 No new package; features and structural governance enforce the layers

Keep the accepted repository/build-domain architecture. The dependency problem is
solvable inside the stable package with feature edges, module visibility, constructor injection,
and structural checks. A crate split adds a release/build boundary without an independent consumer,
toolchain, deployment, or distribution requirement.

**Rejected.** A root workspace, conceptual microcrates, or making `fact-generation` depend on
`daemon`. A future repeated failure of feature/structural enforcement is a named replan trigger.

**Placement and obligations.** The feature lattice in §3.10 is executable governance. Oracles:
`just features-each`, `just stable-graph-check`, and `just feature-architecture-check`.

## 5. Alternatives and clean-sheet challenge

### A — Add more `cfg(feature = "daemon")` annotations

This makes some isolated builds green by hiding proof, closure, and bridge methods. It leaves
fact-generation broken, preserves outward imports, and turns features into reachability tricks
rather than useful capabilities. Rejected.

### B — Make `fact-generation` and `data-fabric` depend on `daemon`

This compiles the current monolith but reverses the intended dependency direction, drags process,
RPC, repository, SQLite, and supervisor dependencies into leaf capabilities, and makes
`features-each` meaningless. Rejected.

### C — Move the current unit authority tokens into a common module

This removes the immediate imports but retains capability theater: execution semantics remain
distributed and marker parameters can remain causally inert. It also fails to address stale
identity, operational/semantic policy mixing, global lookup, or generic proof ownership. Rejected.

### D — Split every layer into a Cargo crate/workspace

This offers the strongest compiler-enforced dependency boundaries and could improve incremental
builds. It is not justified now: no layer needs an independent toolchain, artifact, deployment,
consumer, or release; the governing repository architecture explicitly rejects packages created
for conceptual organization. Reconsider only after repeated same-package enforcement failure or a
real independent consumer appears.

### E — Move Tree-sitter and Ruff out of process

This gives uniform process containment and Arrow IPC but adds latency, memory, packaging, startup,
recovery, and protocol surfaces while sacrificing efficient in-process revision state. Current
evidence does not show that their bounded in-process trust/resource model is inadequate. Rejected;
reopen on measured crash, memory, cancellation, or trust failure.

### F — Layered single crate with behavior-bearing release programs (selected)

This option preserves one product/runtime topology, moves construction authority to the boundary
where it actually controls admission and program execution, keeps Arrow/DataFusion/Delta strengths
visible, exploits Pyrefly incrementality, and makes Cargo features plus structural rules executable
evidence of P35. It requires a direct cutover but no compatibility period or storage redesign.

The independent clean-sheet challenge agreed with this selection and specifically rejected the
interim `cfg` repair, a daemon dependency expansion, a new workspace, and all-provider processes.
Its strongest correction was that parser construction is not semantic authority: provider output
admission is.

## 6. Transition, cutover, and legacy disposition

### 6.1 Transition law

The repository is in design phase and no deployed predecessor has been established. The transition
therefore optimizes for one correct target, not operability of the route being replaced:

1. do not resume v5 WP49 certification against the current architecture;
2. preserve the dirty-tree purge and completed outcomes as current-tree inputs while attributing
   every overlapping edit before implementation;
3. land inward contracts and executable boundary checks before migrating consumers;
4. construct the behavior-bearing release and lane jobs before deleting old tokens;
5. move all production consumers to one injected release in one atomic cutover;
6. remove the marker/profile/global-lookup route and the interim `cfg` workaround in the same
   dependency-closed implementation sequence;
7. retain no compatibility facade, alternate constructor, feature alias, dormant branch, or dual
   provider execution; and
8. certify from one trusted target HEAD only after physical zero-state and real vertical behavior.

No stale plan criterion may require restoration of a deleted hash, static schema, generated
registry, evidence wrapper, comparator, or old runtime route. A requirement that still expresses a
valid functional outcome is translated to the new boundary and proved causally.

### 6.2 Dependency-ordered cutover

This is transition architecture, not a substitute for a successor implementation plan. The plan
must close dependencies in this order:

1. **Boundary guardrail.** Add the target feature metadata and structural/type-leak oracles with
   negative fixtures. At this stage failing production imports are expected and classified.
2. **Application contracts.** Establish provider job/result/coverage/gap/provenance values and
   application-owned rustc control projections. Allow minimal Arrow types without DataFusion.
3. **Release compiler.** Build the five behavior-bearing program products and a single fallible
   release constructor. Bind suite v2.3 once and derive subprogram observations from the compiled
   graph.
4. **Provider migration.** Move Tree-sitter/Ruff to job-driven Arrow output, convert rustc generated
   types at ingress, and put Pyrefly behind a long-lived workspace/context supervisor.
5. **Fabric split.** Separate generic proof/closure engines from release-specific programs, adopt
   DataFusion 55 structured scan forwarding, and remove core dependence on daemon tokens.
6. **State split.** Separate repository input and operational SQLite capabilities from the fabric
   feature while preserving exact Delta publication and restart behavior.
7. **Runtime ownership.** Add the daemon cancellation tree and complete join/drain ownership; inject
   one release into workspace startup, provider admission, query, status, and reference services.
8. **Transport confirmation.** Keep the released wire unless an application-owned conversion
   exposes a genuine missing control field. Thin Tonic handlers and the Python `DaemonPort` must
   pass existing v2/FastMCP 4 verticals without semantic duplication.
9. **Hard deletion.** Delete the old tokens, profile route, global lookups, stale literals,
   generated-type fields, disposable sidecar type, interim gates, and obsolete tests. Remove any
   WP49 residue already classified as predecessor evidence.
10. **Certification.** Run isolated capabilities, causal faults, restart/publication, slow-consumer,
    installed-process, and final zero-state gates at one candidate HEAD.

### 6.3 Material surface disposition

The inventory was built from structural outlines and textual import/caller searches over the Rust
root, auxiliary domains, Python adapter, contracts, tests, rules, scripts, and CI tooling. Generated
and library-reference content was treated according to its role rather than as production source.

| Current surface | Disposition | Target consumer/cutover | Required oracle |
|---|---|---|---|
| `CompiledSemanticRelease` concept and `SuiteIdentity` | **reshape/retain** | one fallibly compiled, behavior-bearing application release injected by daemon composition | `just release-program-contract-check` |
| `CompiledProviderAuthority`, `CompiledTransformationAuthority`, `CompiledQueryAuthority`, `CompiledProofAuthority`, `CompiledPolicyAuthority` unit structs | **replace/delete** | actual compiled program values with causal execution | release mutation/fault cases in `release-program-contract-check` |
| repeated `CompiledSemanticRelease::current()` calls | **delete** | one `Arc` constructor-injected into application services | structural zero-state in `feature-architecture-check` |
| `COMPILED_SUITE_VERSION = "2.2.0"` and duplicated v2.2 provider/query strings | **replace/delete** | one v2.3 categorical suite binding plus distinct typed subprogram versions | `just compiled-suite-identity-check` |
| semantic relation schemas, field roles, authority, coverage, and atomic provider admission in `production_provider_recipe` | **retain/reshape** | `CompiledProviderProgram` and release-owned admission | `just provider-job-contract-check` |
| `CompiledProviderLane` and mixed `CompiledProviderExecutionProfile` | **replace** | typed lane jobs; release semantics and policy ceilings separated from per-run effective values | job bound/tamper faults |
| Tree-sitter/Ruff adapters and bounded native revision state | **retain/reshape** | exact adapter config + lane job -> owned Arrow batches | isolated `fact-generation`; provider fixtures |
| adapter constructors requiring `CompiledProviderAuthority` | **delete** | parser/library validation without semantic authority token | forbidden-import structural fixture |
| public helpers accepting Tree-sitter/Ruff native types in `provider_raw_kinds` | **move/narrow** | adapter-private observation; owned entries escape | `just provider-type-boundary-check` |
| `provider_native_syntax` DTO-to-Arrow composition | **reshape** | fact-generation Arrow output plus release admission; no daemon import | isolated provider-to-Arrow behavior |
| Pyrefly sidecar implementation and `Query::change_files` support | **retain** | one contained state per workspace/context | `just pyrefly-incremental-lifecycle-check` |
| `DisposablePyreflySidecarProcess` and terminate-after-every-run path | **delete** | supervisor-owned sidecar lifecycle with cooperative cancel and bounded restart | incremental/cancel/crash causal cases |
| rustc extractor/service control protocol and Arrow relation validation | **retain** | thin generated-message adapter to owned compiler-run values | provider protocol/interoperability checks |
| generated `OwnerBegin`/`OwnerEnd`/`CompilationBegin`/`CompilationEnd` fields in admitted structs | **replace/delete** | application-owned headers/terminals immediately after wire validation | `just generated-type-boundary-check` |
| generic programmatic session, schema, epoch, child catalog, provider wrappers, exact Delta modules | **retain** | isolated fabric engine | `just data-fabric-core-check` **new** |
| release-specific constants/expectations embedded in generic proof/closure modules | **reshape/move** | `CompiledProofProgram`/`CompiledQueryProgram`; generic evaluator remains inward | proof-program causality cases |
| interim daemon gates on `proof` and `derived_producer_closure` | **delete** | explicit generic-engine/release-program split | isolated data-fabric and semantic-release builds |
| `production_workspace_startup` and `switchable_activation_authority` | **retain daemon-only** | imperative startup/activation shell with injected release | `just programmatic-runtime-lifecycle-check` |
| streamed-result-package method inside core child resource coordinator | **reshape** | generic reservation input in fabric; daemon bridge converts sealed package | data-fabric import rule + resource tests |
| `IdentityPreservingViewTable::scan`-only implementation | **replace** | lossless `scan_with_args` delegation and schema-identity wrapping | `just datafusion-scan-contract-check` |
| `SchemaIdentityExec` | **retain narrowly** | metadata identity only, with full execution-property/value checks | plan/property spy tests |
| `data-fabric -> repository-state` edge | **delete** | separate fabric, repository-input, and operational-state features | `just stable-graph-check`; dependency deny matrix |
| `git_state` and gix read-only observation | **retain** | repository-input adapter | repository-input isolated tests/governance |
| `operational_store`, workspace registry, command/checkpoint/lease SQLite stores | **retain/reclassify** | operational-state adapters outside semantic fabric authority | state-authority isolation tests |
| exact Delta writes, providers, CDF, maintenance, reconciliation, activation vector | **retain** | data-fabric plus application activation overlay | Delta publication/restart oracles |
| custom `Cancellation` atomic probe | **retain/reshape** | synchronous leaf probe bridged from daemon token tree | cancellation probe and responsiveness tests |
| unstructured production `tokio::spawn`, abort/drop-without-join paths | **replace** | owned token/task scopes with drain/join | `just cancellation-tree-check` |
| `.proto`, descriptor set, Rust/Python generated v2 bindings, reproducible generator | **retain** | transport/process control adapters only | proto contract/repro/type-boundary checks |
| combined Tonic handler/application logic in `ProductionQueryService` | **reshape** | thin Tonic adapter around application query service | fake-service and real UDS verticals |
| accepted-handle, watch/resume, explicit cancel, resource handle, UDS peer/grant model | **retain** | released v2 daemon interface | existing FastMCP 4 wire/security/recovery gates |
| Python `DaemonPort`, one lifespan channel, Pydantic projections, FastMCP 4 server | **retain** | presentation only | adapter authority-zero-state and STDIO vertical |
| `src/bin/codefabricd.rs` and `src/bin/codefabric.rs` | **retain thinly** | CLI parsing and composition root only; no schemas/provider profiles/semantic settings | binary source boundary + installed-process test |
| `tooling/proto/generate.rs` | **retain tooling-only** | hermetic descriptor-derived codegen | `just proto-repro-check` |
| current WP49 deletions of predecessor evidence/tooling | **retain when target-neutral** | successor plan re-audits collisions; do not restore stale proof machinery | final legacy zero state plus target package build |
| v5 WP49–WP52 completion wording/state | **supersede in remaining scope** | new plan version based on this design; prior proving commits remain evidence, not automatic certification | plan audit/status plus final rerun |

### 6.4 Generated and historical material

Released `.proto` source is retained authority; generated Rust/Python source and the descriptor set
are retained derived products. Historical designs, reviews, plans, and proving commits remain
immutable evidence and are not searched as live authority by runtime code. Removed WP49 evidence
issuance scripts, predecessor disposition ledgers, and static/generated schema products stay
removed unless the new plan proves a target consumer independent of their old role.

The current Python generated-provider files being removed by WP49 are not restored merely to make a
package census familiar. Only generated families still referenced by the released v2 query control
or justified provider-process contracts survive, and `proto-repro-check` derives that set from the
canonical descriptor pipeline.

### 6.5 Rollback, recovery, and forward repair

Before the hard cutover, rollback is ordinary commit-level code rollback to the last dependency-
closed target packet. It does not authorize reviving the v2.2 marker route in parallel. After
cutover, repair proceeds forward on the sole constructor.

Runtime recovery remains candidate-free:

- process loss invalidates process-local release objects, tokens, provider caches, Pyrefly process
  state, channels, sessions, and cursors;
- restart recompiles the one release, validates its categorical/version compatibility, reconciles
  operational records, reopens exact Delta activation pins, rebuilds the immutable epoch, and only
  then reopens admission;
- a release/vector mismatch fails closed and reports a typed capability/lifecycle gap;
- a Pyrefly crash preserves syntax state, marks semantic capability degraded, reconstructs the
  sidecar from immutable inputs, and publishes only a new proved epoch; and
- a discovered real deployed predecessor stops the transition and triggers a separate one-shot
  `AuthorityHandoff` design. It does not activate generic dormant cutover machinery.

## 7. Proof strategy

### 7.1 Contract-to-oracle matrix

Commands marked **new** are required target recipe names for the successor plan. Existing recipes
may be extended only when their stated intent remains exact.

| Contract | Named executable oracle | Discriminating truth |
|---|---|---|
| I-60 inward feature DAG | `just feature-architecture-check` **new**; `just features-each`; `just stable-graph-check` | each capability performs useful behavior; reverse imports and forbidden dependency families fail |
| I-61 behavior-bearing release | `just release-program-contract-check` **new** | missing/duplicate/inert program operands fail; causal operand mutations change or reject output |
| I-62 provider job/admission | `just provider-job-contract-check` **new** | wrong lane/build/schema/source/context/bounds/terminal/coverage cannot enter a candidate |
| I-63 Arrow-native boundary | `just exact-provider-batch-check`; `just provider-ipc-contract-integrity-check`; `just relation-ipc-provider-operations-check` | in-process batches and IPC obey the same schema, bounds, terminal and corruption semantics |
| I-64 provider isolation/lifecycle | `just provider-type-boundary-check` **new**; `just pyrefly-incremental-lifecycle-check` **new** | no native/generated type escapes; incremental state is reused and crash/cancel gaps are explicit |
| I-65 DataFusion visibility | `just data-fabric-core-check` **new**; `just datafusion-scan-contract-check` **new**; `just datafusion-contract-matrix-integrity-check`; `just datafusion-plan-schema-cache-check` | isolated fabric behavior passes; `ScanArgs`/statistics/properties/values survive wrappers and schema remains plan-derived |
| I-66 Delta/state separation | `just delta-publication-contract-check` **new**; `just delta-durability-protocol-integrity-check`; `just delta-exact-reconstruction-v4-check` | partial commits stay inactive; exact vectors reopen; gix/SQLite cannot select semantic state |
| I-67 cancellation/task ownership | `just cancellation-tree-check` **new**; `just query-retention-cancellation-restart-check`; `just grpc-slow-consumer-check` **new** | parent/child propagation, sibling isolation, cleanup reserve, bounded queues and zero unjoined tasks |
| I-68 transport termination | `just generated-type-boundary-check` **new**; `just proto-check`; `just proto-repro-check`; `just provider-ipc-contract-integrity-check` | generated types terminate at adapters and Rust/Python consume one descriptor contract |
| I-69 identity consistency | `just compiled-suite-identity-check` **new**; `just authoritative-design-conformance-check` | live suite/reference/epoch data is v2.3 and distinct identity concepts are not conflated |
| I-70 hard cutover | `just compiled-release-legacy-zero-state-check` **new**; `just fastmcp4-post-purge-surface-check` | no tokens/global lookups/v2.2 literals/disposable sidecar/compatibility route survives |
| I-71 end-to-end causality | `just semantic-release-vertical-check` **new**; `just fastmcp4-stdio-vertical-check`; `just semantic-request-program-check` | real source change reaches provider, fabric, Delta activation, daemon and presentation exactly once |

### 7.2 Feature and structural evidence

`feature-architecture-check` combines Cargo metadata/tree assertions with structural rules and
negative fixtures. It must prove at least:

```text
fact-generation forbids: datafusion, deltalake, gix, rusqlite, tonic, arc-swap, daemon imports
data-fabric forbids: Ruff/Tree-sitter/Pyrefly/rustc implementations, gix, rusqlite, tonic,
                     arc-swap, daemon/supervisor imports
semantic-release forbids: gix, rusqlite, tonic, supervisor, FastMCP concerns
rpc forbids: DataFusion, Delta, provider libraries, semantic-release behavior
repository-input forbids: DataFusion, Delta, provider libraries, Tonic
operational-state forbids: DataFusion semantic authority, provider libraries, Tonic
daemon is the sole union of release, state, repository, process and transport capabilities
```

Structural rules reject provider/native and generated Protobuf types in application DTO fields,
public signatures, and disallowed imports. Negative control files seed one violation per rule and
must be detected. A source grep with no seeded fault is not sufficient.

### 7.3 Causal behavioral evidence

The release oracle uses independently authored fixtures and faults:

- remove one provider relation, transformation, query form, proof expectation, policy operand, or
  provenance edge and prove compile/admission fails;
- mutate an input fact and prove the dependent DataFusion result changes while an unrelated result
  does not;
- provide valid-looking output under the wrong job/source/context/schema/provider identity and
  prove admission fails;
- return empty-complete, intentional remainder, unknown, timeout, cancellation, malformed Arrow,
  corrupted sequence, and oversize output and prove the distinct terminal/coverage states;
- change only a version string or digest while behavior is wrong and prove identity agreement does
  not produce acceptance; and
- attempt to construct semantic state from transport, SQLite, gix, FastMCP, or an inactive Delta
  commit and prove no production path exists.

Provider lifecycle evidence includes two Pyrefly generations in the same compatible workspace
context, provider-reported affected scope, healthy-context survival after one cancelled run, forced
crash followed by explicit gap and clean reconstruction, and bounded memory over repeated changes.

DataFusion evidence uses a spy provider and plan/property checks to prove every wrapper receives
the same projection, filters, limit, and `StatisticsRequest`; schema-identity wrapping preserves
row values, ordering, partitioning, equivalence properties, and metrics; and no inner/outer
optimizer ordering regression returns.

Delta evidence commits a multi-table candidate partially, proves it remains invisible, completes
and activates the exact vector, advances a newer head, and proves the older epoch still reopens
unchanged. Injected unknown commit outcomes reconcile without duplicate writes.

Cancellation evidence exercises every lifecycle stage. It proves durable replay/idempotence,
parent/child propagation, sibling isolation, process escalation only after cooperative timeout,
cleanup reserve, reservation/lease release, one terminal, and zero live owned tasks after drain.

### 7.4 Transport and presentation evidence

The existing v2/FastMCP 4 suite remains load-bearing:

```text
just fastmcp4-daemon-wire-contract-check
just fastmcp4-atomic-start-check
just fastmcp4-resource-authority-check
just fastmcp4-daemon-security-recovery-check
just fastmcp4-guard-roundtrip-check
just fastmcp4-completion-authorization-check
just fastmcp4-cancellation-recovery-check
just fastmcp4-public-surface-check
just fastmcp4-adapter-authority-zero-state-check
just fastmcp4-stdio-vertical-check
```

A real UDS slow-consumer test measures bounded RSS and queue depth, ordered event/chunk sequence,
prompt reserved control RPCs, cancellation response, one terminal, and correct epoch/resource lease
retention. Python package tests prove one channel/server instance, strict Pydantic projection,
modern-only catalog, STDOUT purity, and absence of semantic/data-plane dependencies.

### 7.5 Performance and resource proof

No legacy implementation baseline is required: it was never functionally certified and is not the
target. Measure the new design against explicit bounds and workloads:

- cold daemon/release compilation and workspace reconstruction;
- Tree-sitter/Ruff cold and incremental batches;
- Pyrefly initial load versus same-context `change_files` generations;
- provider rows/bytes per second and peak bounded memory;
- DataFusion planning, first-batch, full-stream, spill, and cancellation latency;
- exact Delta write/readback/reopen and activation latency;
- gRPC first event, slow-consumer queue/RSS, resource read, and cancel acknowledgement; and
- one versus maximum admitted queries under the governed resource envelope.

The implementation may optimize only after profiles locate a bottleneck. It must not introduce
unsafe code, unbounded buffers, release-profile folklore, parallel type universes, provider object
leakage, lower-level Delta actions, or custom DataFusion operators without a design replan and
contract evidence.

### 7.6 Final certification boundary

The successor plan's terminal gate runs from one frozen candidate HEAD and includes:

```text
just root-fmt
just root-check
just root-clippy
just root-test
just features-no-default
just features-each
just stable-graph-check
just feature-architecture-check
just data-fabric-core-check
just release-program-contract-check
just provider-job-contract-check
just provider-type-boundary-check
just generated-type-boundary-check
just datafusion-scan-contract-check
just delta-publication-contract-check
just cancellation-tree-check
just grpc-slow-consumer-check
just pyrefly-incremental-lifecycle-check
just compiled-suite-identity-check
just compiled-release-legacy-zero-state-check
just semantic-release-vertical-check
just proto-check
just proto-repro-check
just adapter-ci-fast
just ci-fast
```

Equivalent existing recipe names may be reused only if the implementation plan records the exact
mapping and the recipe proves the same claim. A proving commit, captured output, plan state label,
or green subset cannot certify completion without the terminal rerun.

## 8. Data-fabric doctrine disposition

Only principles materially implicated by this review are classified.

| Principle | Status | Design effect |
|---|---|---|
| P1–P2 model and executable model | **Advances** | provider/release/policy meaning becomes typed behavior-bearing programs before orchestration |
| P3 one authority | **Advances** | one injected compiled release and one admission path replace distributed globals and unit tokens |
| P5 ports and adapters | **Advances** | provider, repository, operational state, gRPC and FastMCP terminate at application-owned contracts |
| P7–P8 canonical fabric/engine | **Maintains** | Arrow remains canonical and DataFusion remains the visible compiler/executor |
| P9–P10 provenance and closure | **Advances** | jobs/results/programs carry source/context/coverage/dependency/provenance closure causally |
| P11 and P23 state/resource ownership | **Advances** | Delta, SQLite, provider caches, tokens, tasks, channels and presentation state have distinct owners |
| P12 contract-driven | **Advances** | categorical identities and generated transport contracts stop leaking across concepts/layers |
| P13 authority governance | **Advances** | admission/proof/activation enforce authority; feature flags and marker possession do not |
| P14–P15 extension and optimizer visibility | **Maintains/advances** | native DataFusion remains preferred and `ScanArgs`/statistics/properties are preserved |
| P16 lifecycle phases/failures | **Advances** | long-lived Pyrefly and structured cancellation distinguish cancel, crash, gap, rebuild and drain |
| P18 fingerprints are identity only | **Risk-mitigated** | no new hash proves correctness; stale/digest agreement cannot bypass execution |
| P20 prover-backed capability | **Advances** | runtime capability derives from installed release/session/provider coverage and proof |
| P22 canonical protocols | **Maintains** | Protobuf remains control, Arrow remains semantic bulk data, FastMCP remains presentation |
| P25 and P31 independent oracles | **Advances** | each clause has a named causal/negative oracle; legacy agreement is unnecessary |
| P26–P30 execution-proved change | **Advances** | program operands are read by execution and mutations prove scoped downstream effects |
| P32 construction | **Advances** | release preparation plus output admission is the semantic choke point, not parser construction |
| P33 functional core/imperative shell | **Advances** | release/fabric programs are inward; daemon/process/I/O/activation remain the shell |
| P34 one mutation path | **Maintains** | sole `FabricCommand` and exact activation remain unchanged |
| P35 inward acyclic dependencies | **Advances** | the feature lattice and structural faults make dependency direction executable |
| P36 executable governance | **Advances** | Cargo metadata, type-boundary rules, causal tests and zero-state gates govern the architecture |

## 9. Risks and reopen triggers

| Risk or new requirement | Required response |
|---|---|
| Multiple suites must coexist or hot-swap in one daemon | Reopen release construction; do not add a registry or global selector opportunistically. |
| Same-package feature/structural rules repeatedly fail to prevent reverse dependencies | Evaluate a justified multi-crate split with measured build/runtime effects. |
| Tree-sitter/Ruff demonstrate process-fatal, memory, cancellation or trust failures under governed workloads | Reopen their process placement with measured evidence. |
| Long-lived Pyrefly exceeds the workspace memory envelope | Measure eviction/context partitioning first; consider bounded restart or a separately designed shared sidecar only with isolation proof. |
| Provider Arrow construction becomes a measured bottleneck | Optimize builders/buffers or IPC batching while preserving the Arrow contract; never leak provider objects. |
| A DataFusion wrapper cannot preserve `ScanArgs` or execution properties | Redesign at a higher supported provider/planner seam or replan; do not silently drop information. |
| Delta requires a low-level action/kernel path for a new outcome | Re-run the delta-rs operation-selection ladder and add protocol/concurrency tests before descending. |
| Cross-user or remote daemon deployment appears | Reopen UDS/security/credential/transport design; the current same-user local profile is not generalized. |
| Released wire must carry new semantic rows | Reject by default and reassess the four-plane boundary; Protobuf must not become a duplicate semantic model. |
| A real deployed predecessor is discovered | Stop and design a one-shot `AuthorityHandoff`; do not revive generic v5 cutover machinery. |
| Existing v2.3 suite text cannot express a load-bearing target contract | Issue a versioned suite refinement before implementation; do not alter v2.3 in place. |

## 10. Acceptance

This design is complete enough for a new versioned implementation plan. The plan must supersede
the remaining v5 scope, preserve independently valid WP43–WP48 outcomes and target-neutral WP49
deletions, implement the dependency-ordered cutover in §6.2, and make every new oracle in §7 an
explicit dependency-closed obligation. Implementation must not resume against the current unit-
token/global-lookup architecture.

accepted
