# CodeFabric 1.3 Implementation Roadmap

**Artifact ID:** `codefabric-implementation-roadmap`  
**Artifact kind:** Implementation-planning document  
**Roadmap version:** 1.0  
**Target design release:** CodeFabric present-state CPG specification suite 1.3  
**Status:** Planning baseline  
**Primary deployment profile:** `local-workstation-v1`  
**Primary implementation:** Native Rust daemon and data plane, date-pinned nightly Rust extractor, Rust Pyrefly sidecar, and Python FastMCP adapter  
**Purpose:** Sequence realization of the finalized 1.3 target design into dependency-correct implementation waves that are each suitable for a separate detailed implementation plan
**Audit integration (2026-08-20):** Plan-audit F-002/F-003/F-005/F-014; clarified Wave-0 pinning, pre-snapshot readiness, provider-before-activation order, and integrated-foundation execution discipline.

---

## 0. Authority, purpose, and roadmap boundary

This roadmap translates the finalized CodeFabric 1.3 design into an implementation order. It does **not** alter the design, relax a conformance obligation, or create a competing architecture. If this roadmap conflicts with a 1.3 normative specification, the 1.3 specification and suite governance manifest prevail.

The roadmap has four goals:

1. begin with the build, contract, identity, security, and persistence dependencies on which all later behavior relies;
2. prove the architecture through a narrow end-to-end slice before implementing ontology breadth;
3. build facts in increasing semantic depth—source, syntax, local semantics, project/compiler semantics, flow, effects, and summaries;
4. defer complete semantic-query, RPC, and FastMCP output surfaces until their underlying fact, completeness, snapshot, and lifecycle contracts are stable.

The waves are **scope units, not calendar estimates**. Each wave is intended to become one detailed implementation plan with a coherent objective, a bounded set of major work packages, explicit entry dependencies, and a binary exit gate.

Exactly one implementation plan and one schema-current execution state may be mutable at
a time. A remediation overlay freezes its predecessor as read-only provenance: trusted
ancestor proving commits remain historical evidence, incomplete packets remain
incomplete, and no second executor advances the frozen state. A successor may become
active only after a cache-disabled full check, exact two-root reproduction, independent
consumer validation, required decommission zero states, and state/proving-commit
reconciliation pass at one candidate commit. Activation is a separate sealed handoff:
the active-plan pointer changes only after certification, and the successor's first
packet revalidates the inherited surface before product execution resumes.

---

## 1. Source design and governing implementation invariants

The roadmap is based on the synchronized 1.3 artifacts:

- `codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md`;
- `code_property_graph_present_state_fact_ontology_specification_v1.3.md`;
- `present_state_cpg_fact_generation_specification_python_rust_v1.3.md`;
- `present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md`;
- `codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md`;
- `code_property_graph_semantic_query_specification_v1.3.md`;
- `present_state_cpg_fastmcp_serving_specification_v1.3.md`.

Every wave SHALL preserve these suite-wide invariants:

1. `workspace_id` identifies exactly one authorized analyzed source instance.
2. One immutable leased `ServingSnapshot` is the only query pin.
3. Current stable filesystem bytes, not watcher events, Git objects, or prior provider output, are present-state authority.
4. Provider observations are not canonical facts until reconciled.
5. Context-sensitive facts never cross analysis-context boundaries.
6. Unknown remainder is explicit; missing data does not prove absence.
7. The Rust daemon owns semantic interpretation, planning, execution, snapshots, and canonical result bytes.
8. The Python FastMCP process remains a thin adapter and does not become a second query or graph engine.
9. Every compatibility-sensitive artifact is versioned and fingerprinted.
10. Incremental results must converge to the clean-rebuild result for identical inputs.

---

## 2. Wave-sizing and sequencing rules

### 2.1 What constitutes one wave

A wave is correctly sized when it has:

- one dominant architectural objective;
- normally four to eight major work packages;
- no more than two newly introduced cross-process or public compatibility boundaries;
- one integrated acceptance fixture or corpus extension;
- one explicit capability, profile, or readiness checkpoint;
- a clear list of intentionally deferred work.

A detailed plan may divide a wave into internal epics, but it should not change the wave's externally visible completion condition.

### 2.2 Sequencing principles

The implementation order follows these rules:

```text
contracts before generated code consumers
identity and source authority before facts
publication and snapshots before continuous mutation
source/syntax before semantic enrichment
local semantics before project/compiler semantics
provider observations before cross-provider reconciliation
intraprocedural facts before interprocedural fixed points
direct facts before transitive summaries
fact and completeness semantics before semantic query
semantic query before public RPC adaptation
Rust RPC before Python FastMCP presentation
correctness before acceleration
acceptance gates before a production claim
```

### 2.3 Early vertical slice without premature productization

The suite requires an end-to-end golden slice early. This roadmap therefore introduces a deliberately narrow query, streaming, and artifact path in Wave 5. That path is a conformance harness and architectural proof, **not** the final `SERVING_V1` product surface. Full query semantics, RPC behavior, and FastMCP outputs remain later waves.

### 2.4 Continuous obligations

Every wave, beginning with Wave 1, SHALL update as applicable:

- requirement traceability;
- registry and schema sources;
- generated Rust/Python types;
- golden fixtures and canonical outputs;
- fault points;
- security cases;
- observability and metrics;
- clean-rebuild comparison coverage;
- upgrade compatibility records.

Testing, security, and observability are not deferred wholesale to the final wave; the final wave closes the full acceptance gates.

Those updates are model-driven. Requirement declarations remain co-located with their
normative source; implementation and oracle declarations remain with the code or
evidence they describe. Aggregate requirements, traceability, bundle membership,
artifact/provenance indexes, output censuses, and assurance-profile membership are
compiled views and SHALL NOT be maintained as parallel manifests. Packet-named mutation
campaigns are not completion evidence. The generic file-scoped mutation tool remains an
optional human Tier-C diagnostic outside edit, packet, milestone, CI, and release
profiles.

---

## 3. Roadmap at a glance

| Stage | Wave | Name | Primary completion signal |
|---|---:|---|---|
| Foundation | 0 | Program, toolchain, and build foundation | Every process domain builds reproducibly from a clean checkout |
| Foundation | 1 | Machine contracts, registries, and code generation | **Readiness Gate A** passes |
| Foundation | 2 | Daemon kernel, workspace registry, path security, and source images | Authorized Git/non-Git workspaces can be registered and inventoried safely |
| Foundation | 3 | Canonical data fabric, publication, overlay, and snapshot kernel | Synthetic canonical facts survive overlay, publication, lease, and recovery |
| Core facts | 4 | Source and syntax fact generation | Python/Rust source and syntax facts are canonical and queryable internally |
| Core facts | 5 | End-to-end vertical golden slice | **Readiness Gate B** passes |
| Core facts | 6 | Continuous update, freshness, and core equivalence | `CORE_SOURCE_V1` is complete; incremental core-source scenarios compare equal to rebuild |
| Core facts | 7 | Git-aware lifecycle acceleration and topology | Git transitions converge correctly with generic fallback preserved |
| Semantic profiles | 8 | Python local semantic substrate | Python declarations, scopes, bindings, imports, CFG, and direct def-use are current |
| Semantic profiles | 9 | Pyrefly project semantics and Python profile closure | `PYTHON_SEMANTIC_V1` is complete |
| Semantic profiles | 10 | Rust compiler/MIR semantic core | Rust definitions, types, MIR, CFG, and calls are current under a pinned context |
| Semantic profiles | 11 | Rust ownership, lowering, and profile closure | `RUST_SEMANTIC_V1` is complete |
| Semantic profiles | 12 | Full reconciliation, completeness, contexts, and unknown remainder | Cross-provider canonical state and negative-proof semantics are complete |
| Advanced analysis | 13 | Intraprocedural flow and graph analyses | Advanced flow is complete within owners/components |
| Advanced analysis | 14 | Effects, resources, concurrency, and interprocedural summaries | `ADVANCED_FLOW_V1` is complete; formal **Readiness Gate C** passes |
| Query/output | 15 | Controlled language, resolver, and core `PlanSpec` compiler | Deterministic core semantic queries lower to executable plans |
| Query/output | 16 | Full composable query and canonical response | All eight query forms and canonical logical-response semantics pass conformance |
| Query/output | 17 | Daemon RPC, artifacts, credentials, and multi-agent serving | Complete accepted-handle query service operates over local IPC |
| Query/output | 18 | FastMCP agent-facing outputs | `SERVING_V1` is complete in real MCP hosts |
| Acceptance | 19 | Failure, security, performance, upgrade, and release acceptance | **Readiness Gates D–G** pass; production-conformant baseline is releasable |

---

## 4. Dependency graph and permitted parallelism

### 4.1 Acceptance-order graph

```text
W0 → W1 → W2 → W3 → W4 → W5 → W6
                              │
                              ├────────→ W7 ────────┐
                              ├────────→ W8 → W9 ──┤
                              └────────→ W10 → W11 ┤
                                                   ↓
                                                  W12 → W13 → W14
                                                                  ↓
                                                  W15 → W16 → W17 → W18 → W19
```

Wave 7, the Python lane, and the Rust lane may proceed in parallel after the continuous core is stable. Wave 12 is the integration barrier requiring the completed Git correctness path and both language semantic profiles.

### 4.2 Parallel prework that does not change acceptance order

The following prework is useful but SHALL remain behind generated contracts and test doubles until its acceptance wave:

- query grammar/parser and `PlanSpec` prototypes may begin after Wave 1 against golden fixture tables;
- daemon RPC skeletons may begin after Wave 1 and use a fake query executor;
- the FastMCP adapter may begin after Wave 1 against a fake daemon generated from the Protobuf contract;
- graph-algorithm prototypes may begin after Wave 4 using canonical projection fixtures;
- Pyrefly and rustc sidecar process harnesses may begin after Wave 1, but their facts are not accepted until their profile waves.

Parallel prework SHALL NOT create alternate schemas, identifiers, status enums, or request interpretations.

---

# Part I — Foundation Waves

## 5. Wave 0 — Program, toolchain, and build foundation

### Objective

Create a reproducible implementation environment for the four distinct compatibility domains before any product logic is written.

### Entry dependencies

- Finalized CodeFabric 1.3 prose suite.
- No implementation artifacts are assumed.

### Major work packages

1. **Stable Rust daemon/data-plane domain**
   - pin the stable Rust toolchain and edition required by the 1.3 data-fabric baseline;
   - pin Arrow, Parquet, DataFusion, object-store, delta-rs, gix, Tokio, and supporting crates;
   - prohibit duplicate public Arrow/DataFusion/object-store families across crate boundaries.

2. **Nightly rustc extractor domain**
   - pin the exact dated nightly and compiler components;
   - isolate `rustc_public` and narrowly scoped `rustc_private` use in a separate executable/build domain;
   - ensure compiler-owned values cannot escape its adapter boundary.

3. **Pyrefly sidecar domain**
   - pin the exact Pyrefly source/tag and independent lockfile;
   - create a sidecar executable shell without exposing Pyrefly-internal Rust types.

4. **Python FastMCP adapter domain**
   - pin Python, FastMCP, Pydantic, and pydantic-settings;
   - create a locked environment and minimal STDIO-safe executable shell.

5. **Repository command and CI contract**
   - provide stable commands for formatting, linting, building, unit tests, integration tests, schema generation, and dependency checks;
   - establish deterministic test roots, temporary state roots, and fixture conventions;
   - configure caching without allowing cache contents to become correctness authority.

6. **Process-boundary build prerequisites**
   - pin and verify one exact Protobuf compiler/generator identity for Rust and Python rather than accepting an ambient system `protoc`;
   - establish canonical artifact output locations for generated contracts and fixtures.

### Exit evidence

- A clean checkout builds all four domains without manual edits.
- Each executable prints its exact version/toolchain identity to STDERR or a diagnostic command.
- CI rejects duplicate incompatible Arrow/DataFusion/object-store versions.
- No compiler-, Pyrefly-, gix-, delta-rs-, or FastMCP-internal type crosses an application-owned boundary.
- Skeleton processes start and terminate cleanly without writing non-protocol output to STDOUT.

### Explicitly deferred

- Registry contents and generated application contracts.
- Workspace registration and source access.
- Fact schemas, providers, storage, queries, and user-visible tools.

---

## 6. Wave 1 — Machine contracts, registries, and code generation

### Objective

Instantiate the complete machine-contract tree required by the 1.3 design and make it the sole source for generated compatibility types.

### Entry dependencies

- Wave 0 build and code-generation environment.

### Major work packages

1. **`contracts/` source tree**
   - manifests, registries, identity specifications, schemas, query grammar, Protobuf definitions, adapter schemas, bundles, deployment profile, fault registry, comparator rules, and security corpus manifest.

2. **Repository model compiler and verifier**
   - implement the repository-owned `codefabric-model` compiler, assurance graph, and sole transactional reconciler;
   - discover self-identifying native sources, evidence, and acceptance through closed
     family roots and typed family declarations, compile them through staged typed
     ingress and cross-record validation, and derive every source/output census,
     consumer edge, and provenance obligation;
   - emit `contracts/manifests/suite-manifest.json` as a compatibility/provenance view
     of the compiled repository model, never as compiler bootstrap or an authored
     membership list;
   - derive typed artifact, dependency, output, consumer, resource, and provenance nodes
     directly from family-native declarations and the owner-accepted release census;
   - permit only typed source/semantic views and exact model edges; reject authored command
     graphs, central member/path lists, duplicate ownership, and dependency cycles;
   - canonicalize semantic projections and separately fingerprint exact source bytes;
   - enforce catalog-selected byte, nesting, collection, token/alias, graph, and
     diagnostic budgets before and after parse as appropriate;
   - expose the stable read-only repository command `just model-release-check`.

3. **Ontology and categorical registries**
   - entity, relation, property, fact-kind, unknown, projection, summary, capability,
     error, provider, derivation, lifecycle-state-machine, phrase, enum, and flag
     registries;
   - append-only code validation and duplicate-authority detection.

4. **Identity and canonicalization specifications**
   - CBEF/ID preimage encoding and public ID round trip;
   - path canonicalization and ordering;
   - canonical type algebra;
   - canonical JSON and checksum vectors.

5. **Schema generation**
   - one closed schema Contract IR compiled by a typed derivation unit into Arrow/Delta
     schemas and table metadata, a generated Rust `TableSpec` registry, and operational
     SQLite DDL;
   - analysis context, `ServingSnapshot`, public snapshot metadata, source context, public status, semantic request/response, and `PlanSpec` schemas;
   - generated public schemas are cataloged authorities whose semantic source is their
     exact sole derivation output, never hand-maintained sibling definitions;
   - FastMCP public input/output schemas generated from the adapter Contract IR through
     Pydantic validation/serialization views.

6. **Protocol generation**
   - daemon query service, provider control, Pyrefly sidecar, and rustc extractor Protobuf packages;
   - negotiated feature registry;
   - one exact Python compiler invocation producing Python stubs and a committed
     `FileDescriptorSet`, followed by Rust generation from that same descriptor IR;
   - resolve both compiler stages from typed derivation invocations, map descriptor names
     to the exact governed root-input census, and treat the normalized descriptor census
     as a generated review projection rather than semantic authority;
   - descriptor-model assertions that distinguish semantic descriptors from generated
     language source and never treat deterministic Protobuf serialization as canonical bytes.

7. **Bundle and deployment manifests**
   - ontology, schema, provider, derivation, query-language, tool-contract, toolchain, and model-pack bundles;
   - effective `local-workstation-v1` profile.

8. **Requirement traceability**
   - stable requirement IDs and end-to-end trace records from ontology through storage, query, serving, and tests.

9. **Generated-data and conformance discipline**
   - emit language-neutral index and schema data once as canonical resources, while
     reserving generated Rust/Python source for statically useful types and behavior;
   - derive provider/version raw-kind JSON review catalogs and the Rust hot-path lookup
     binding together from one closed typed derivation; adapters validate that exact
     grammar/query identity at startup and never parse review JSON per source file;
   - emit peer artifact/derivation index records so every generated output has one owner
     and consumers can inspect its complete resolved lineage without creating a second
     digest authority;
   - keep owner-reviewed known-answer vectors independent from generator-derived,
     property, differential, and fuzz corpora; generation may stage candidates but
     SHALL NOT approve or overwrite normative expected values.

10. **Proof-preserving command feedback**
   - keep independent intent recipes and the complete milestone gate;
   - change aggregate command edges only after controlled warm and fresh-target
     Hyperfine measurements demonstrate material benefit;
   - compare the closure of the compiled assurance graph derived from co-located declarations and live Just,
     Nextest, Pytest, rule, fixture, and consumer inventories, preserving exact
     toolchains, targets, features, tests, fixtures, and negative cases;
   - unknown command reads conservatively widen to the full applicable profile; no
     hand-maintained proof manifest or packet-specific mutation recipe is authoritative;
   - never use `cargo clean` or shared-cache deletion as routine benchmark setup.

### Exit evidence

- **Readiness Gate A passes.**
- Generated Rust and Python code compiles.
- Every released fixture validates against its public schema.
- Identity, path, type, enum, flag, and canonical-JSON known-answer vectors pass in both Rust and Python where applicable.
- Protobuf packages compile and round-trip in both languages.
- No mandatory requirement, ontology kind, storage field, query phrase, or public field is orphaned in traceability.
- Re-running generation from unchanged sources produces byte-identical canonical artifacts.
- Catalog reordering does not change compiled output bytes, and every accepted input
  remains within its declared resource budget.

### Explicitly deferred

- Runtime implementation behind generated interfaces.
- Delta tables, daemon state, providers, and queries.

---

## 7. Wave 2 — Daemon kernel, workspace registry, path security, and source images

### Objective

Implement the secure present-state source-instance control plane on which all fact generation depends.

### Entry dependencies

- Wave 1 generated identity, path, analysis-context, status, deployment, and error contracts.

### Major work packages

1. **Daemon lifecycle kernel**
   - singleton daemon lease;
   - configuration/profile loading;
   - multi-workspace process topology;
   - one `WorkspaceCoordinator` actor and active-snapshot slot per `workspace_id`.

2. **Operational-state persistence**
   - SQLite WAL store for registration, workspace lifecycle, source inventory, generations, jobs, barriers, credentials metadata, and recovery records;
   - transaction and migration foundation.

3. **Workspace administrative lifecycle**
   - explicit register, enable, disable, reconcile, remove, and inspect operations;
   - distinct Git worktree and non-Git root identities;
   - registration revisions and authorization fingerprints.

4. **Root authorization and secure opening**
   - user-owned root validation;
   - byte/native path handling;
   - component-wise secure open and post-open containment checks;
   - symlink, nested root, and unauthorized path rejection.

5. **File/path identity**
   - workspace-relative path slots, content generations, file instances, display paths, and URI forms;
   - rename/replacement identity primitives without relying on watcher or Git IDs.

6. **Source-image capture**
   - immutable current-byte snapshots with BLAKE3 digest and generation fence;
   - stable-read retry/defer behavior;
   - optional decoded text and line index as non-authoritative projections.

7. **Initial inventory**
   - bounded generic source walker;
   - gix repository/worktree discovery and read-only topology correctness;
   - language/file classification and unsupported-file policy.

8. **Internal administration and diagnostics**
   - non-MCP administrative CLI or test client;
   - safe status and diagnostic surfaces for bootstrap testing.

### Exit evidence

- Multiple linked worktrees of one repository register as distinct workspaces.
- A non-Git root registers without a synthetic repository identity.
- Authorized files can be captured byte-for-byte; escaped symlinks and path-prefix attacks fail.
- Concurrent file mutation during capture never yields a falsely stable source image.
- Restart restores registration and inventory state without claiming an active fact snapshot.
- Adversarial path and permission fixtures pass for Linux and macOS profiles.
- Wave-2 source-control-plane health is reported independently; the mandatory
  `WorkspaceLifecycle` remains `BOOTSTRAPPING` until Wave 3 activates the first
  valid `ServingSnapshot`, which is the sole transition to `READY`.

### Explicitly deferred

- Watcher-driven updates.
- Fact tables and canonical facts.
- Git status/index/HEAD acceleration beyond discovery correctness.

---

## 8. Wave 3 — Canonical data fabric, publication, overlay, and snapshot kernel

### Objective

Create the canonical fact-state substrate before introducing real providers.

### Entry dependencies

- Wave 1 generated Arrow/Delta, identity, state, and snapshot contracts.
- Wave 2 workspace and operational-state services.

### Major work packages

1. **Schema and table registry runtime**
   - generated `TableSpec` loading;
   - schema-digest validation;
   - Arrow/Delta/DataFusion round-trip checks.

2. **Control-plane tables and views**
   - workspace, common repository, analysis context/set, publication, publication table, current publication, snapshot manifest/active snapshot, owner, capability, and diagnostic representations.

3. **Universal canonical fact core**
   - `entity`, `relation`, `property_fact`, and `fact_evidence`;
   - canonical ID/public-ID handling;
   - provenance and categorical metadata.

4. **Delta durable namespace**
   - local-filesystem Delta namespace per workspace;
   - table creation, mutation classes, owner replacement, idempotency, and commit conflict handling.

5. **Hot overlay**
   - replacement rows, owner tombstones, key tombstones, and table replacement semantics;
   - deterministic consolidation and merge;
   - overlay memory accounting.

6. **Publication and active pointer protocols**
   - staged durable publication, table-version recording, validation, completion, and current pointer;
   - candidate `ServingSnapshot` construction, including exact-version Delta providers, private catalog, overlay wrappers, integrity checks, and frozen provider set, before atomic activation.

7. **Snapshot leases and retention**
   - immutable snapshot lease API;
   - base publication and overlay lifetime tracking;
   - interaction with result retention and future vacuum.

8. **Overlay-aware DataFusion catalog**
   - serving views and query-time-derived surfaces over the snapshot-owned durable-base providers and pinned overlay;
   - synthetic views sufficient for integration tests without rebuilding or rebinding providers after activation.

### Exit evidence

- Synthetic owner facts can be inserted, replaced, removed, overlaid, durably flushed, rebased, leased, and queried.
- A query against a leased snapshot is unaffected by a later active-snapshot swap.
- Crash/restart tests at publication and pointer boundaries recover to one coherent current state.
- Overlay merge equals the corresponding durable effective state under canonical comparison.
- Schema round-trip and integrity queries pass for the foundational tables.
- Every lease on one snapshot reuses that snapshot's pointer-identical provider set, and no active snapshot exists before that set is frozen.

### Explicitly deferred

- Real source/syntax/semantic providers.
- Complete reconciliation and derivation logic.
- Public semantic query language.

---

# Part II — Core Fact and Continuous-Correctness Waves

## 9. Wave 4 — Source and syntax fact generation

### Objective

Produce canonical present-state source and syntax facts for valid and invalid Python/Rust files.

### Entry dependencies

- Wave 2 immutable source images.
- Wave 3 observation ingestion and canonical fact storage.

### Major work packages

1. **Common provider-observation boundary**
   - generated async job and source-snapshot DTOs;
   - observation metadata, generation fences, batch manifests, and Arrow handoff.

2. **Tree-sitter Python and Rust adapters**
   - raw CST, normalized syntax kinds, fields, ranges, named/anonymous nodes, errors, missing syntax, and incremental tree support.

3. **Ruff syntax/lexical adapter**
   - typed Python AST, tokens, comments, documentation, directives, trivia/index facts, diagnostics, and source-coordinate reconciliation.

4. **Canonical source entities and relationships**
   - files, spans, tokens, annotations, syntax nodes, ordered `AST_CHILD`, declarations/call-site syntax detectable without project semantics, and source ownership.

5. **Source-context representation**
   - text-or-bytes payload, encoding/newline metadata, path redaction-ready references, and exact range extraction.

6. **Capability and unknown handling**
   - unsupported/oversized/binary/generated/vendored classification;
   - parse-error and missing-syntax entities;
   - explicit absence of unavailable semantics rather than retention of stale rows.

7. **Source/syntax table encoders and minimal reconciliation**
   - owner-scoped batches;
   - source-range reconciliation between Tree-sitter and Ruff evidence;
   - deterministic identity under unchanged bytes.

### Exit evidence

- Valid, partially invalid, and malformed Python/Rust fixtures produce exact source and syntax rows.
- Every raw provider syntax kind remains representable.
- Repeated full extraction of identical bytes produces identical canonical IDs and effective rows.
- Source-context round trips exact bytes and ranges.
- Unsupported files yield explicit capability/diagnostic outcomes rather than silent omission.

### Explicitly deferred

- Project-aware Python semantics.
- rustc semantic and MIR breadth.
- Watcher-driven incrementality.
- Full controlled semantic query language.

---

## 10. Wave 5 — End-to-end vertical golden slice

### Objective

Prove the complete architectural path with deliberately narrow functionality before adding breadth.

### Entry dependencies

- Waves 1–4.

### Major work packages

1. **Golden repository v1**
   - one Python owner;
   - one small Rust build unit and MIR-bearing owner;
   - explicit unknown, property, relation, and derived-projection fixtures.

2. **Thin rustc extractor slice**
   - real pinned compiler invocation;
   - `CompilationBegin`/`OwnerBegin`/`OwnerEnd`/`CompilationEnd` manifest;
   - one body, basic blocks, and a minimal call/CFG fact set.

3. **Minimal canonical reconciliation**
   - enough declaration/range/call evidence resolution to create the golden canonical facts;
   - conflicts and unknowns preserved.

4. **Minimal derivation**
   - one registered projection or mechanically derived result over canonical rows.

5. **Minimal query compiler slice**
   - a bounded phrase subset for basic entity lookup, fact retrieval, and source context;
   - a narrow generated `PlanSpec` subset lowered to DataFusion.

6. **Minimal accepted-handle service path**
   - internal test client;
   - start, stream terminal result, cancel, read artifact, release artifact;
   - canonical response bytes.

7. **Hot and durable update path**
   - mutate one owner;
   - activate a hot overlay;
   - durably publish and rebase;
   - preserve snapshot pinning.

### Exit evidence

- **Readiness Gate B passes exactly as defined by the suite manifest.**
- The golden Python and Rust facts are visible through one semantic request.
- One result is streamed and the complete response can also be read as an immutable artifact.
- A hot update and later durable publication produce the expected canonical state.
- All IDs, rows, response bytes, and checksums match released golden outputs.

### Explicitly deferred

- Advertising any full conformance profile other than a test-only slice.
- General watcher lifecycle.
- Full provider breadth, all query forms, production RPC security, and FastMCP.

---

## 11. Wave 6 — Continuous update, freshness, and core equivalence

### Objective

Turn the bootstrap source/syntax pipeline into a continuously maintained present-state service while preserving clean-rebuild equivalence.

### Entry dependencies

- Wave 5 validated vertical path.

### Major work packages

1. **Watcher/event facade**
   - `notify-debouncer-full` integration;
   - normalized application events;
   - bounded ingress, overflow signals, and backend failure handling.

2. **Dirty registry and update-wave scheduler**
   - debounce, coalescing, generation counters, update classes, multi-file logical batches, superseding work, cancellation, and backpressure.

3. **Invalidation and operational dependency graph**
   - source, owner, fact-family, context, cross-owner, and global invalidation;
   - semantic capability withdrawal at admission.

4. **Fast syntax lane**
   - stable source recapture;
   - incremental Tree-sitter where safe;
   - current syntax publication before slower semantics;
   - explicit pending/unavailable status.

5. **Freshness and barrier state machine**
   - admitted-event barriers;
   - all public freshness policies;
   - source trust, event-stream health, owner capability, completeness, and query availability as orthogonal dimensions.

6. **Overlay activation and durable rebase**
   - deterministic consolidation;
   - owner/cross-owner/global mutation semantics;
   - asynchronous durable publication and active-pointer swap.

7. **Startup and crash recovery**
   - cold and warm start;
   - orphan staging publications;
   - lost unjournaled overlay behavior under the local profile;
   - full reconciliation after event loss.

8. **Canonical clean-rebuild comparator**
   - canonical table/state checksums;
   - difference artifacts;
   - core edit corpus.

### Exit evidence

- Isolated save, repeated save, atomic save, add, delete, rename, move, parse break/fix, formatter burst, generated burst, watcher overflow, and restart scenarios converge correctly.
- Strict-current queries never expose invalidated stale facts.
- Core-source incremental states compare equal to clean rebuilds.
- `CORE_SOURCE_V1` is advertised `COMPLETE` for supported files and exact coverage is returned per owner.
- The formal all-capability Gate C remains open until semantic and advanced-flow scenarios are added.

### Explicitly deferred

- Git status/index/HEAD acceleration.
- Python and Rust complete semantic profiles.
- Interprocedural derived facts.

---

## 12. Wave 7 — Git-aware lifecycle acceleration and topology

### Objective

Add Git fidelity and work avoidance without changing filesystem-byte authority or generic fallback correctness.

### Entry dependencies

- Wave 6 continuous source correctness.

### Major work packages

1. **Git-native inventory**
   - pathspec, excludes, attributes, directory walk, tracked/untracked/ignored/conflicted classification, inclusion-policy fingerprint, and metadata watch set.

2. **Status and index acceleration**
   - bounded candidate-delta DTO;
   - index stages/conflicts;
   - warm-start and rescan pruning;
   - periodic generic audit and CLI parity fixtures.

3. **HEAD/tree transition acceleration**
   - baseline fingerprint;
   - tree-to-tree candidate diff;
   - branch-switch stabilization tuple;
   - bounded rename candidates and current-byte verification.

4. **File-kind, mode, symlink, and attribute changes**
   - mode-only transitions;
   - symlink-mode safety;
   - ignore/attribute invalidation.

5. **Linked-worktree and common-repository topology**
   - shared immutable repository information with independent workspace coordinators;
   - no shared mutable `Arc<Repository>` misuse.

6. **Submodules and nested repositories**
   - separate workspace/topology domains;
   - endpoint-only external treatment unless separately authorized.

7. **Cache hierarchy and degradation**
   - blob OID as auxiliary key only;
   - source/owner/provider caches with required safeguards;
   - gix failure degrades acceleration, not correctness.

### Exit evidence

- Branch switches, large tracked-tree transitions, index conflicts, `.gitignore`/attribute changes, linked-worktree additions, and submodule topology changes converge to clean rebuild.
- Ordinary isolated saves do not trigger full status scans.
- Every Git candidate set is fenced by its `GitStateVector` and current bytes.
- Disabling or faulting gix falls back to bounded authoritative inventory with identical semantic state.

### Explicitly deferred

- History ontology, checkout, fetch, credentials, hooks, or any Git mutation.
- Semantic provider breadth not needed for Git lifecycle tests.

---

# Part III — Language Semantic Profile Waves

## 13. Wave 8 — Python local semantic substrate

### Objective

Build Python semantics that can be derived reliably from Ruff and application-owned analysis without Pyrefly project-wide enrichment.

### Entry dependencies

- Wave 6 continuous lifecycle.
- Wave 4 Ruff syntax/lexical adapter.

### Major work packages

1. **Python analysis-context discovery foundation**
   - project roots, module paths, Python version/config inputs, stubs, dependency declarations, and context fingerprinting.

2. **Ruff semantic adapter**
   - scopes, declarations, bindings, references, imports, qualified names, rebinding, shadowing, and execution context.

3. **Module/import/export facts**
   - source-declared modules, package identity, imports, aliases, exports, re-exports, and unresolved modules.

4. **Callable and member syntax contracts**
   - parameters, defaults, decorators, annotations, call sites, receivers, arguments, and argument-to-parameter binding where syntactically/local-semantically determined.

5. **Python CFG**
   - evaluation order, normal and exceptional edges, loops, branches, `try`/`finally`, context managers, comprehensions, generators, and async syntax.

6. **Direct definitions, uses, and access events**
   - binding-aware definition/use events;
   - reads, writes, and access paths;
   - direct def-use within supported owner scope.

7. **Unknown and capability handling**
   - dynamic name/member/call remainder;
   - unavailable project semantics kept explicit.

### Exit evidence

- Python local-semantic fixtures produce exact scopes, bindings, references, imports, CFG, and direct def-use.
- Body-local edits invalidate and replace only sound owners plus declared dependencies.
- Parse and semantic errors yield current syntax plus precise capability gaps.
- `PYTHON_SEMANTIC_V1` is advertised `PARTIAL`, with Pyrefly-owned mandatory capabilities explicitly missing.

### Explicitly deferred

- Project-wide type/member/call authority.
- Advanced points-to, effects, and interprocedural summaries.

---

## 14. Wave 9 — Pyrefly project semantics and Python profile closure

### Objective

Complete project-aware Python semantics behind a stable application-owned sidecar protocol.

### Entry dependencies

- Wave 8 local Python facts.
- Wave 1 `pyrefly_sidecar.proto` and provider contracts.

### Major work packages

1. **Pyrefly sidecar service**
   - exact pinned source/tag;
   - handshake and bundle negotiation;
   - accepted asynchronous jobs, progress, cancellation, deadlines, restart, and generation fences.

2. **Immutable source/context transfer**
   - source-image manifest and content transport;
   - module dependency neighborhood batching;
   - no direct provider filesystem authority beyond authorized inputs.

3. **Module and symbol resolution**
   - project module graph, imports, declarations/xrefs, and unresolved/external endpoints.

4. **Canonical type enrichment**
   - computed, declared, expected, and narrowed types;
   - canonical type-algebra conversion and interning;
   - uncertainty and unsupported-shape handling.

5. **Object model and member resolution**
   - inheritance/MRO, attributes, descriptors, properties, overrides, protocols, and resolved member candidates.

6. **Call-target enrichment**
   - exact/resolved/possible candidates;
   - constructors, bound methods, callable objects, decorators, and dynamic unknown remainder.

7. **Ruff/Pyrefly reconciliation and lifecycle**
   - provider precedence and conflicts;
   - context-sensitive IDs;
   - sidecar failure and restart behavior;
   - dependency-driven invalidation.

### Exit evidence

- `PYTHON_SEMANTIC_V1` is `COMPLETE` for the selected corpus/context.
- Cross-module definitions, types, members, and call targets match canonical fixtures.
- Sidecar failure never leaves invalidated prior semantics visible; unaffected owners remain current only when dependency validity is established.
- Incremental Python scenarios compare equal to clean rebuild.
- Multiple Python contexts remain partitioned and never merge exact facts.

### Explicitly deferred

- Advanced alias/points-to, effect models, concurrency, and transitive summaries.

---

## 15. Wave 10 — Rust compiler/MIR semantic core

### Objective

Establish the stable daemon-to-nightly-extractor boundary and generate core compiler-semantic facts.

### Entry dependencies

- Wave 6 continuous lifecycle.
- Wave 1 rustc extractor and provider contracts.

### Major work packages

1. **Rust context/build discovery**
   - Cargo metadata, packages, targets, features, cfg, target triple, build inputs, and context fingerprinting.

2. **rustc extractor process**
   - Cargo wrapper/driver integration;
   - exact nightly/toolchain negotiation;
   - sandbox, resource limits, interruption, and safe diagnostics.

3. **Invocation manifest protocol**
   - `BEGIN`, owner-complete records, and terminal invocation manifest;
   - source/build/toolchain digests;
   - no partial owner publication without a valid manifest.

4. **Definitions and type semantics**
   - crates/modules/items, types, generics, lifetimes, traits, impls, members, coercions, and adjustments.

5. **MIR core**
   - bodies, locals, arguments, blocks, statements, terminators, operands, places, rvalues, source correspondence, and raw normalized variants.

6. **CFG and calls**
   - normal/unwind edges;
   - direct calls, function references, closures, function pointers, and instance candidates.

7. **Compile-failure semantics**
   - invalid owners become unavailable;
   - hidden last-known-good cache is not present-state truth;
   - unaffected owners retained only under proven dependency validity.

### Exit evidence

- Valid crates yield exact semantic definitions, type facts, MIR, CFG, and call facts.
- Syntax/type/borrow/compiler failures produce current source/syntax plus explicit compiler capability gaps.
- Extractor crash, timeout, and stale generation outputs are rejected.
- `RUST_SEMANTIC_V1` is `PARTIAL`, with ownership/lowering mandatory capabilities identified.

### Explicitly deferred

- Full move/borrow/drop, exact loans/regions, vtables, macro/lowered correspondence, and advanced FFI.

---

## 16. Wave 11 — Rust ownership, lowering, and profile closure

### Objective

Complete the required Rust semantic profile, including ownership-state and generated/lowered correspondence.

### Entry dependencies

- Wave 10 compiler/MIR core.

### Major work packages

1. **Places, projections, and access events**
   - canonical memory/access paths;
   - reads, writes, moves, copies, borrows, reborrows, address taking, and drops.

2. **Ownership and initialization state**
   - move paths, initialized/uninitialized state, ownership transitions, and program-point facts.

3. **Exact compiler enrichments**
   - narrowly scoped `rustc_private` adapter for loans, regions, vtable/dispatch, stable compiler keys, or source maps where required and available.

4. **Executable instances and dynamic dispatch**
   - monomorphized instances, trait dispatch, vtables, closures, drop glue, shims, and unknown candidate remainder.

5. **Macros and generated/lowered correspondence**
   - invocations, expansions, source mapping, MIR lowering, async/coroutine lowering, and representation separation.

6. **Drop, unsafe, constants, and FFI facts**
   - destructor sites, resource implications, unsafe operations, inline assembly, extern declarations, and cross-language link evidence.

7. **Lifecycle and incremental owner fingerprints**
   - changed-owner replacement;
   - context/toolchain invalidation;
   - dependency-aware retention and cache rules.

### Exit evidence

- `RUST_SEMANTIC_V1` is `COMPLETE` for the selected corpus/context.
- Move/copy/borrow/drop, macro/lowered, instance, and unwind fixtures match expected facts.
- Compiler-version and adapter-digest mismatches fail before activation.
- Incremental Rust scenarios, including compile break/fix and signature/trait changes, compare equal to clean rebuild.

### Explicitly deferred

- Cross-language advanced flow, effect propagation, and full interprocedural summaries.

---

## 17. Wave 12 — Full reconciliation, completeness, contexts, and unknown remainder

### Objective

Integrate all provider lanes into one sound canonical state and formalize when negative statements are permitted.

### Entry dependencies

- Waves 7, 9, and 11.

### Major work packages

1. **Complete canonical reconciliation engine**
   - declaration, range, type, member, and call-target reconciliation;
   - provider precedence;
   - evidence retention and conflict diagnostics;
   - no provider writes canonical rows directly.

2. **Property cardinality and storage mapping**
   - typed property values;
   - exactly-one/zero-or-one/multi-valued integrity;
   - denormalized projection round trips.

3. **Capability aggregation**
   - provider run, owner capability, completeness, dependency, and query availability kept orthogonal;
   - owner/scope/context/profile aggregation.

4. **Unknown remainder and explicit negatives**
   - unknown entities/targets/types/memory/effects;
   - explicit negative facts;
   - proof-scope and unknown-remainder propagation.

5. **Completeness and negative-proof algebra**
   - owner, file, module, build unit, context, dependency, limit, and provider-failure composition;
   - proven-empty versus indeterminate distinctions.

6. **Multi-context and external dependency policy**
   - context sets and default contexts;
   - declaration-only external dependencies;
   - endpoint-only cross-workspace relations;
   - no cross-context exact path/traversal.

7. **Cross-language/FFI linking**
   - exact, possible, and unknown linkage evidence under the FFI profile.

8. **Derivation materialization registry completion**
   - one owner and one precision profile per derived family;
   - query-time versus materialized policy.

### Exit evidence

- No canonical fact is produced twice by competing authorities.
- Conflicting provider evidence remains inspectable while canonical resolution follows the registry.
- Negative queries return `PROVEN_EMPTY` only when the algebra permits it.
- Multi-context fixtures remain partitioned; external workspaces remain opaque endpoints.
- `CORE_SOURCE_V1`, `PYTHON_SEMANTIC_V1`, and `RUST_SEMANTIC_V1` are revalidated against exact profile requirements.

### Explicitly deferred

- Full advanced-flow algorithms and interprocedural effect/summary propagation.
- Full semantic query compiler.

---

# Part IV — Advanced Analysis Waves

## 18. Wave 13 — Intraprocedural flow and graph analyses

### Objective

Implement deterministic owner/component-local analyses over the reconciled canonical facts.

### Entry dependencies

- Wave 12 canonical facts, completeness semantics, projection registry, and materialization matrix.

### Major work packages

1. **Canonical graph projection runtime**
   - projection DTOs and registry;
   - Arrow/CSR/petgraph conversion;
   - deterministic node/edge ordering and evidence mapping.

2. **Reachability and components**
   - direct and bounded query-time reachability;
   - connected components and SCCs where owner/component local;
   - materialization thresholds.

3. **Dominance family**
   - dominators, post-dominators, control dependence, and validation against reference implementations.

4. **Loop analysis**
   - natural and irreducible loops;
   - source-loop correspondence and loop metrics.

5. **Reaching definitions and liveness**
   - generic worklist engine;
   - definition/use domains and kill semantics;
   - exact owner-local outputs.

6. **Points-to and alias under `BALANCED_V1`**
   - Python and Rust constraint profiles;
   - field sensitivity, unknown memory, alias sets, and precision metadata.

7. **Program-state and structural metrics**
   - initialization states, direct value flow, cyclomatic complexity, branch/loop counts, and registered metrics.

8. **Incremental derived invalidation**
   - owner/component fingerprints;
   - materialized replacement and query-time cache rules.

### Exit evidence

- Algorithm fixtures pass differential and metamorphic tests.
- Derived facts retain supporting fact IDs, projection/profile identity, and completeness state.
- Incremental owner/component changes compare equal to clean recomputation.
- `ADVANCED_FLOW_V1` is `PARTIAL`, pending direct effect/resource and interprocedural summary requirements.

### Explicitly deferred

- Full effect/resource model packs, concurrency/happens-before, and interprocedural SCC fixed points.

---

## 19. Wave 14 — Effects, resources, concurrency, and interprocedural summaries

### Objective

Complete advanced-flow semantics and fixed-point propagation across callable components.

### Entry dependencies

- Wave 13 intraprocedural facts.
- Wave 12 complete calls, unknowns, and completeness.

### Major work packages

1. **Direct effect extraction**
   - state mutation, allocation, I/O, blocking, exceptions/panic, spawn/await, locks, unsafe, and FFI;
   - language-neutral and language-specific codes.

2. **Model-pack runtime**
   - declarative format, version compatibility, trust/signature policy, match rules, evidence, and diagnostics;
   - no executable external extensions.

3. **Resource lifecycle**
   - acquisition, ownership/transfer, escape, release/drop, cleanup, and unknown external resource behavior.

4. **Exceptional and unwind semantics**
   - raised/panic/unwind facts, handlers, cleanup, drop paths, and direct/transitive separation.

5. **Static concurrency and happens-before**
   - tasks, threads, channels, locks, spawn/join/await, ordering evidence, and explicit uncertainty.

6. **Call graph SCC and recursion**
   - exact/possible edge treatment;
   - recursive component identities and convergence ordering.

7. **Callable summaries**
   - local summaries;
   - SCC fixed point for transitive calls, reads/writes, effects, resources, exceptions, and unknown remainder;
   - deterministic caps and non-convergence status.

8. **Advanced-flow lifecycle/equivalence**
   - interprocedural invalidation;
   - summary replacement;
   - full golden edit and provider-failure corpus.

### Exit evidence

- `ADVANCED_FLOW_V1` is `COMPLETE` for each supported semantic profile and precision configuration.
- Direct and transitive effects/summaries are never conflated.
- Unknown call/effect/resource remainder propagates conservatively.
- Fixed points converge deterministically or terminate with explicit bounded status.
- **Formal Readiness Gate C passes** across core, Python, Rust, Git, and advanced-flow golden scenarios.

### Explicitly deferred

- Complete agent-facing semantic language and transport.

---

# Part V — Query and Output Waves

## 20. Wave 15 — Controlled language, resolver, and core `PlanSpec` compiler

### Objective

Implement the deterministic semantic compiler foundation against stable facts and completeness semantics.

### Entry dependencies

- Wave 12 canonical semantics; Wave 14 is required for final acceptance of advanced phrases.
- Wave 1 query grammar, phrase registry, request schema, and `PlanSpec` schema.

### Major work packages

1. **Controlled-language parser**
   - `english-controlled-v1` grammar;
   - phrase-registry loading;
   - identifier, quoted name, ordinary word, path, and modifier parsing.

2. **Deterministic semantic resolver**
   - ontology mapping, synonyms, precedence, ambiguity, canonical interpretation, and no LLM/embedding authority on the canonical path.

3. **Entity matching**
   - qualified-name parsing, exact and ranked matching, grouping, context/language/representation filters, and candidate explanations.

4. **Typed internal `PlanSpec`**
   - source, filter, projection, join, traversal, grouping, ordering, limit, completeness, and result-role nodes;
   - schema/type validation before lowering.

5. **Semantic source-boundary compiler**
   - authorized inventory filters only;
   - no root widening or implicit external workspace opening.

6. **Freshness and snapshot binding**
   - authorized workspace resolution;
   - accepted query ID;
   - freshness barrier;
   - one immutable snapshot lease before semantic resolution/planning.

7. **Core DataFusion lowering**
   - entity lookup, fact retrieval, direct relationship following, source-context retrieval, and bounded point queries;
   - use stable serving views/table functions rather than storage-aware public syntax.

8. **Cost estimation foundation**
   - row, traversal, path, pattern, source-byte, and intermediate-state estimates;
   - pre-execution rejection for hard limits.

### Exit evidence

- Equivalent core phrases resolve to the same canonical semantics and `PlanSpec`.
- Ambiguous phrases are rejected or return candidates according to policy; none are silently guessed.
- Core query forms produce deterministic results and `resolved_semantics` records.
- Plans never cross workspace or context boundaries implicitly.
- Plan snapshots and physical plans are captured as conformance artifacts.

### Explicitly deferred

- All eight forms, full result references, resumable streaming, and public daemon service hardening.

---

## 21. Wave 16 — Full composable query and canonical response

### Objective

Complete the semantic query specification independently of MCP presentation.

### Entry dependencies

- Wave 15 compiler foundation.
- Wave 14 advanced fact families for complete phrase/query coverage.

### Major work packages

1. **All eight request forms**
   - find entities;
   - retrieve facts;
   - follow relationships;
   - connecting paths;
   - fact patterns;
   - combine result sets;
   - objective summaries;
   - source/syntax context.

2. **Query dependency DAG**
   - typed result-reference roles and selector grammar;
   - fan-in/fan-out;
   - cycle detection;
   - deterministic execution ordering.

3. **Path and pattern execution**
   - bounded traversal and path policies;
   - typed bindings, alternatives, negation under completeness rules, and resource limits.

4. **Completeness and negative-proof output**
   - proven-empty, unavailable, incomplete, filtered-empty, and limit-reached distinctions;
   - supporting scope/capability evidence.

5. **Canonical response materialization**
   - public snapshot metadata;
   - interned entity/fact/path/group/source-context dictionaries;
   - query results, coverage, errors, and deterministic ordering.

6. **Canonical JSON and fact statements**
   - exact checksum contract;
   - canonical human-readable fact statements as navigation, never authority.

7. **Streaming and resumability**
   - chunk interning, terminal completeness, checksums, resume tokens, partial-stream failure semantics, and no silent truncation.

8. **Plan cache**
   - key includes request semantics, query-language/ontology/schema bundles, limits, authorization class, context set, and snapshot-independent dimensions;
   - snapshot-sensitive execution state is never reused incorrectly.

### Exit evidence

- All request and response fixtures validate against canonical schemas.
- All eight query forms pass conformance across core, Python, Rust, advanced flow, unknown, and source-context cases.
- Composition references are role-safe and dependency cycles fail before execution.
- A partial stream is never reported as logical success without its terminal completeness record.
- Equivalent requests against one snapshot produce semantically and byte-canonically equivalent responses.

### Explicitly deferred

- Production local IPC credentials, multi-agent fairness, and FastMCP host behavior.

---

## 22. Wave 17 — Daemon RPC, artifacts, credentials, and multi-agent serving

### Objective

Expose the complete daemon query capability through the specified accepted-handle local service without adding adapter-side semantics.

### Entry dependencies

- Wave 16 canonical query engine.
- Wave 6 daemon lifecycle and snapshot service.

### Major work packages

1. **Complete Protobuf service**
   - handshake, status, validate, start, stream/attach, cancel, read result, and release result;
   - negotiated versions, feature bits, compression, bundles, and limits.

   The generated Python client substrate from Wave 1 becomes a production
   event-loop-owned `grpc.aio` `DaemonClient` here. One channel and stub live for the
   adapter lifespan, deadlines are mandatory, metadata/status translation is
   centralized, and shutdown explicitly closes the channel. Wave 1 does not claim this
   runtime behavior.

2. **Accepted-handle query state machine**
   - daemon query ID returned before freshness waiting/execution;
   - progress phases, terminal states, acknowledgement, idempotency, and reconnect.

3. **Cancellation, deadlines, and orphan handling**
   - cooperative cancellation through DataFusion/providers/graph operators;
   - attachment loss, grace periods, cleanup, and immutable completed results.

4. **Capability credentials and local IPC**
   - agent/workspace/process/operation/expiry binding;
   - issuance, rotation, revocation, socket permission and peer checks;
   - no network listener in the baseline profile.

5. **Immutable result artifact store**
   - canonical bytes, manifest, subresources/chunks, checksums, TTL, leases, release, and crash cleanup.

6. **Delivery and status contracts**
   - host-limit-aware inline/resource decision;
   - stable error registry and layer mapping;
   - public status/redaction levels.

7. **Authorization and source disclosure**
   - independent fact, path, source-text, diagnostic, and artifact permissions;
   - reauthorization on every source/artifact read.

8. **Multi-agent governance**
   - global/per-agent/workspace admission, reservations, fairness, starvation guarantees, memory/spill/artifact quotas, and cancellation isolation.

### Exit evidence

- Rust and generated Python test clients pass handshake and compatibility negotiation.
- Start returns a handle before freshness waiting; stream, attach, cancel, reconnect, and read/release work under fault injection.
- Unauthorized workspace/source/artifact access fails without metadata leakage.
- Multiple agents receive fair service and cannot read or cancel one another's results.
- Completed artifacts retain the originally leased snapshot and survive allowed adapter reconnects.

### Explicitly deferred

- FastMCP tool/resource/prompt presentation and real MCP host compatibility.

---

## 23. Wave 18 — FastMCP agent-facing outputs

### Objective

Deliver the final model-facing local STDIO surface while preserving the daemon as the sole semantic authority.

### Entry dependencies

- Wave 17 production RPC service.
- Wave 1 generated adapter schemas and Protobuf client.

### Major work packages

1. **Strict adapter contracts and settings**
   - immutable pydantic-settings snapshot;
   - strict public Pydantic models;
   - reusable JSON `TypeAdapter` for canonical JSON bytes/metadata;
   - serialization-mode schema generation and fingerprint self-check.

2. **FastMCP server lifecycle**
   - one process per agent;
   - long-lived authenticated daemon client;
   - STDIO-safe startup/shutdown;
   - readiness and safe STDERR diagnostics.

   The four production handlers, their real published FastMCP tool manifests, and the
   equality check between published manifest schemas and generated Pydantic
   serialization schemas are owned by this wave. Wave 1 supplies only the model,
   schema, and fingerprint compiler substrate.

3. **Four public tools**
   - `query_code_graph`;
   - `validate_code_graph_query`;
   - `get_code_graph_status`;
   - `get_code_graph_reference`.

4. **Inline and resource delivery**
   - discriminated outputs;
   - complete canonical response preserved inline or by immutable resource;
   - previews and human summaries are explicitly non-authoritative.

5. **MCP resources**
   - schemas/specification/guide/recipes/status;
   - result root, manifest, query, dictionary, source-context, and chunk resources;
   - range, expiry, release, and authorization semantics.

6. **Progress, cancellation, errors, and telemetry**
   - daemon events mapped to MCP context;
   - cancellation propagation;
   - safe error translation;
   - trace/correlation and adapter metrics.

7. **Agent guidance**
   - concise instructions;
   - query-authoring and fact-interpretation prompts;
   - recipes covering Python, Rust, calls, CFG, dataflow, alias, ownership, effects, summaries, and source context.

8. **Real-host compatibility**
   - in-memory FastMCP tests;
   - real STDIO tests;
   - supported MCP host matrix;
   - multiple concurrent adapter processes.

### Exit evidence

- `SERVING_V1` is `COMPLETE`.
- A real programming agent can discover the four tools, author one composable request, receive progress, cancel, and retrieve complete inline or resource output.
- The adapter never constructs SQL, `PlanSpec`, graph traversals, or semantic interpretations.
- Public model schemas and packaged schema fingerprints are exact.
- No source, secret, internal exception, or unrestricted daemon metadata leaks through public serialization.

### Explicitly deferred

- HTTP/ASGI, multi-user gateways, shared Python adapters, distributed fabrics, write tools, history, and runtime observation.

---

# Part VI — Production Acceptance Wave

## 24. Wave 19 — Failure, security, performance, upgrade, and release acceptance

### Objective

Close the remaining master readiness gates and produce the first implementation that may claim conformance with the 1.3 design under `local-workstation-v1`.

### Entry dependencies

- Waves 0–18 complete.

### Major work packages

1. **Full conformance harness**
   - regenerate and verify all machine artifacts;
   - compile normative snippets/protocols;
   - run registry/schema/round-trip/traceability checks;
   - execute every profile corpus.

2. **Golden corpus and clean-rebuild comparator**
   - complete Python/Rust/edit/query/output corpus;
   - exact effective-state equality;
   - durable difference artifacts;
   - ID collision detection.

3. **Deterministic fault injection — Gate D**
   - watcher loss, event queue saturation, Git changes during scan, sidecar/extractor crashes, cancellation, Delta conflicts, disk/spill exhaustion, and process death at each persistence/pointer/artifact boundary.

4. **Security and authorization — Gate E**
   - malicious paths/symlinks, source and artifact ACLs, credential binding/revocation, cross-agent isolation, sandbox escapes, malformed RPC/JSON, adversarial source, model packs, and public-output leakage.

5. **Performance acceptance — Gate F**
   - selected hardware/workload profile;
   - cold bootstrap, interactive syntax, semantic refresh, query latency, memory, spill, artifact, compaction, and multi-agent fairness SLOs;
   - degradation behavior and reproducible reports.

6. **Data-layout and runtime hardening**
   - partitioning/clustering, Parquet writer policy, compaction/vacuum, memory/spill, cache limits, and maintenance runbooks;
   - local filesystem baseline first; optional object-store compatibility remains separately identified.

7. **Upgrade and rollback — Gate G**
   - additive and breaking synthetic ontology/schema/query/RPC/provider/toolchain upgrades;
   - reindex decisions, overlay handling, side-by-side validation, rollback window, and preserved artifacts.

8. **Operational release package**
   - service-manager integration;
   - startup/shutdown/recovery runbooks;
   - dashboards/alerts;
   - dependency and security review;
   - signed/fingerprinted release manifest and reproducibility evidence.

### Exit evidence

- **Readiness Gates D, E, F, and G pass.**
- Gates A–C remain green against the final implementation and release bundles.
- Every advertised conformance profile is exact and coverage-aware.
- The release can be rebuilt from a clean checkout and reproduces the declared machine-contract and binary provenance records.
- Rollback to the preserved prior compatible state is demonstrated, not merely documented.

### Explicitly deferred beyond the baseline

- Windows runtime support;
- remote network listeners and multi-user tenancy;
- HTTP/ASGI deployment;
- distributed or composite-workspace snapshots;
- source mutation, refactoring, or Git mutation tools;
- code history, semantic diffs, runtime observations, test-impact conclusions, risk scoring, and recommendations;
- unsaved editor-buffer overlays unless a future explicit overlay contract is designed.

---

## 25. Capability checkpoint map

| Checkpoint | Completed after | Meaning |
|---|---:|---|
| Contract-ready | Wave 1 | Machine contracts exist and generated consumers may be implemented |
| Vertical architecture proven | Wave 5 | One real Python/Rust path crosses providers, reconciliation, storage, snapshot, query, stream, and artifact |
| `CORE_SOURCE_V1` | Wave 6 | Current source/syntax facts and basic queries are continuously maintained |
| `PYTHON_SEMANTIC_V1` | Wave 9 | Required Python semantic facts are complete for selected contexts |
| `RUST_SEMANTIC_V1` | Wave 11 | Required Rust compiler/MIR/ownership facts are complete for selected contexts |
| Integrated semantic substrate | Wave 12 | Reconciliation, context, capability, unknown, and negative-proof semantics are complete |
| `ADVANCED_FLOW_V1` | Wave 14 | Flow, alias, effects, resources, recursion, and summaries are complete |
| Canonical query complete | Wave 16 | All eight request forms and canonical logical-response semantics are complete |
| Daemon serving complete | Wave 17 | Secure accepted-handle RPC, streaming, artifacts, and fairness are complete |
| `SERVING_V1` | Wave 18 | Programming agents can consume the complete service through FastMCP |
| Production-conformant baseline | Wave 19 | All implementation-readiness gates pass |

---

## 26. Master readiness-gate map

| Suite gate | Roadmap closure point | Important prerequisite work |
|---|---:|---|
| Gate A — Contract generation | Wave 1 | Wave 0 build/codegen foundation |
| Gate B — Vertical golden slice | Wave 5 | Waves 2–4 daemon/source/fabric foundation |
| Gate C — Continuous-update equivalence | Wave 14 | Core lifecycle plus both semantic profiles and advanced derived updates |
| Gate D — Failure and recovery | Wave 19 | Fault points added incrementally from Waves 2–18 |
| Gate E — Security and authorization | Wave 19 | Root security in Wave 2; provider sandboxing in Waves 9–11; IPC/ACL in Wave 17; adapter firewall in Wave 18 |
| Gate F — Performance | Wave 19 | Metrics and performance fixtures added continuously; optimization follows correctness |
| Gate G — Upgrade and rollback | Wave 19 | Artifact/version boundaries established in Waves 0–1 and preserved throughout |

---

## 27. Cross-cutting workstreams that must not become late cleanup

### 27.1 Contracts and traceability

Every schema, enum, phrase, error, capability, provider, derivation, projection, summary, property, and public field introduced by a wave SHALL originate in the appropriate machine source and be traceable to implementation and tests.

### 27.2 Golden fixtures and deterministic comparison

Every provider or derived family SHALL arrive with:

- positive, negative, invalid, unknown, and boundary fixtures;
- exact expected provider observations where useful;
- exact canonical rows;
- canonical query/response fixtures when exposed;
- incremental versus clean-rebuild scenarios;
- minimized difference artifacts on failure.

### 27.3 Failure injection

Every state transition, process boundary, write boundary, pointer swap, artifact operation, and long-running algorithm SHALL expose deterministic fault points as it is implemented. Wave 19 executes the full matrix; it should not require retrofitting faultability.

### 27.4 Security

Security responsibilities progress with the architecture:

```text
Wave 2   roots, paths, file opens, configuration/state permissions
Wave 5   committed golden Rust fixture only under explicit TRUSTED_LOCAL grant;
         network and credentials removed; arbitrary repositories fail closed
Wave 7   Git trust, path semantics, symlinks, no mutation/external commands
Waves 9–11 provider sandboxing and untrusted compiler/type-checker inputs
Wave 14  model-pack trust and external semantic evidence
Wave 17  credentials, UDS, ACLs, artifacts, quotas, cross-agent isolation
Wave 18  Pydantic output firewall, STDIO discipline, public metadata review
Wave 19  complete adversarial corpus and independent acceptance
```

### 27.5 Observability and resource governance

Each wave SHALL define phase-level traces, counters, queue depths, memory, spill, cache, cancellation, and failure metrics for the work it introduces. Limits SHALL fail explicitly and SHALL not silently truncate or retain stale facts.

### 27.6 Upgrade discipline

Every unstable or independently versioned boundary—rustc nightly, Pyrefly, Ruff internals, Tree-sitter grammars, gix, delta-rs, DataFusion/Arrow, FastMCP, Pydantic, Protobuf, registries, and model packs—SHALL have fixtures and bundle fingerprints from its first wave.

---

## 28. Recommended structure for each later detailed wave plan

The next planning pass for any wave should produce a separate document with this minimum structure:

1. **Objective and exit condition** — quote the roadmap wave objective and make the pass/fail outcome explicit.
2. **Normative requirement inventory** — list the exact suite sections and `CF-*`/`G-*` requirements implemented.
3. **Entry assumptions** — identify prior-wave artifacts that are treated as stable.
4. **Work-package decomposition** — normally four to eight major packages, each with owned inputs and outputs.
5. **Interfaces and generated contracts** — enumerate schemas, Protobuf messages, registries, application DTOs, and version checks.
6. **State and data changes** — tables, operational records, state transitions, caches, and retention rules.
7. **Execution and concurrency model** — Tokio/Rayon/process placement, admission, cancellation, and backpressure.
8. **Failure and degradation behavior** — explicit capability gaps, retry boundaries, fault points, and recovery.
9. **Security controls** — trust boundary, authorization, path/source disclosure, sandboxing, and redaction.
10. **Verification plan** — unit, property, differential, golden, integration,
    incremental-equivalence, recovery, and host tests selected from the compiled
    assurance graph; mutation testing is optional diagnostic work, never a required
    packet campaign.
11. **Performance evidence** — measurements required even when the wave is not the final performance gate.
12. **Migration and rollback** — how pre-wave state/artifacts are accepted, upgraded, rebuilt, or rolled back.
13. **Deferred scope** — capabilities deliberately excluded so the plan cannot expand into later waves.
14. **Completion checklist** — machine-verifiable evidence required to close the wave.

A detailed plan SHALL not introduce a new high-level design decision. A discovered ambiguity is returned to the owning 1.3 specification as a design issue rather than resolved ad hoc inside implementation code.

An integrated program plan MAY cover consecutive waves when explicitly authorized by
the design owner/user and when one cross-wave graph materially improves dependency and
cutover safety. Execution remains wave-segmented: only the current wave and its accepted
predecessor interfaces are loaded as active context, each milestone restamps design and
plan digests, and parallel packets have disjoint write sets. Shared generator,
bootstrap, CI, catalog, or generated-output files require an explicit serialized
integration edge. The integrated form does not relax the normal four-to-eight-packet
sizing target for the executable slice of any one wave and does not permit a later wave
to certify an incomplete predecessor gate.

---

## 29. Primary specification traceability by wave

| Wave | Primary normative owners and sections |
|---:|---|
| 0 | Suite process/toolchain topology; Data Fabric §2; Fact Generation provider isolation; FastMCP §18 and §77–78 |
| 1 | Suite Manifest `AC-G-01`–`AC-G-08`, Part IV, Gate A; Ontology `AC-G-70`–`AC-G-72`; Query `AC-G-44`, `AC-G-46`, `AC-G-53`; Serving `AC-G-58`, `AC-G-65` |
| 2 | Lifecycle `AC-G-09`–`AC-G-11`, `AC-G-27`, `AC-G-28`, `AC-G-62`; Ontology `AC-G-12`, `AC-G-13`, `AC-G-18` |
| 3 | Data Fabric `AC-G-19`–`AC-G-23`, `AC-G-26`; Data Fabric Parts II–IV, XI–XIII, XV |
| 4 | Fact Generation Parts II–III/IV source-syntax sections; `AC-G-32`, `AC-G-33`, `AC-G-36`, `AC-G-43`; Ontology `CORE_SOURCE_V1` source/syntax subset |
| 5 | Suite Gate B; minimal slices of Fact Generation, Data Fabric, Query, and Serving contracts |
| 6 | Lifecycle Parts I–IV, VI–VIII; `AC-G-24`, `AC-G-25`, `AC-G-29`, `AC-G-41`; Suite `AC-G-79`; Ontology `CORE_SOURCE_V1` |
| 7 | Lifecycle Part V and §§88–92; gix correctness/acceleration contracts |
| 8 | Fact Generation Python §§14–19, 22–25; Ontology Python scopes/bindings/calls/CFG |
| 9 | Fact Generation `AC-G-14`, `AC-G-30`, `AC-G-34`–`AC-G-36`; Python §§20–23; Ontology `PYTHON_SEMANTIC_V1` |
| 10 | Fact Generation `AC-G-31`–`AC-G-35`; Rust §§34–42; lifecycle Rust semantic lane |
| 11 | Fact Generation Rust §§43–51 and `AC-G-40`; Ontology `RUST_SEMANTIC_V1`, Rust profiles |
| 12 | Data Fabric `AC-G-37`, `AC-G-42`; Ontology `AC-G-15`–`AC-G-17`, `AC-G-71`, `AC-G-73`; Query `AC-G-48`, `AC-G-51` |
| 13 | Ontology `AC-G-74`; Fact Generation derived §§52–65 and `AC-G-39`; Data Fabric calculations §§79A–90 |
| 14 | Ontology `AC-G-75`–`AC-G-77`; Fact Generation `AC-G-38`, §§27–30, 47–50, 66; Suite Gate C |
| 15 | Query `AC-G-44`–`AC-G-46`, `AC-G-49`, `AC-G-50`, `AC-G-52`; Query Parts I–II core execution |
| 16 | Query `AC-G-47`, `AC-G-48`, `AC-G-51`, `AC-G-53`–`AC-G-57`; all request/response conformance |
| 17 | Serving `AC-G-58`–`AC-G-69`; Lifecycle `AC-G-62`; Data Fabric snapshot/artifact lease integration |
| 18 | FastMCP Serving Parts III–XIII and `SERVING_V1` |
| 19 | Suite Manifest `AC-G-78`–`AC-G-84`, Gates D–G; all domain release-conformance obligations |

---

## 30. Roadmap completion criterion

This roadmap is complete when it can be used to create detailed implementation plans without changing wave boundaries or inventing cross-wave architecture. The implementation program is complete only when Wave 19 closes all master gates and the released implementation advertises only the exact conformance profiles it demonstrably satisfies.
