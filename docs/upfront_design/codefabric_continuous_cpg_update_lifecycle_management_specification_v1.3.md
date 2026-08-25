# CodeFabric Continuous CPG Update and Lifecycle Management Specification

**Artifact ID:** `codefabric-continuous-cpg-lifecycle`
**Artifact kind:** Normative document
**Compatible suite major:** 1
**Release date:** 2026-08-20
**Canonical digest:** External; recorded in `codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_artifact_index.json`

**Status:** Released normative implementation specification
**Synchronized suite version:** 1.3
**Specification version:** 1.3
**Specification date:** 2026-08-20
**Revision:** synchronized source-instance / ServingSnapshot architecture
**Supersedes:** The prior non-gix revision of this specification and the standalone gix lifecycle addendum
**Primary implementation language:** Rust
**Primary runtime topology:** One central CodeFabric daemon per authorized repository/worktree group; one independent worktree lifecycle coordinator and immutable CPG snapshot per analyzed worktree; one FastMCP STDIO coordination process per programming agent
**Primary change sources:** `notify-debouncer-full` over `notify` for low-latency invalidation, plus read-only `gix` for Git-aware repository/worktree state, inventory, status, index, and bulk-transition reconciliation
**Primary concurrency stack:** Tokio, Rayon, Crossbeam, DashMap, optional `tokio-rayon`
**Primary durable data plane:** Arrow, DataFusion, Delta Lake / delta-rs
**Git state baseline:** `gix = 0.86.0` minimum; read-only application policy; no repository mutation, checkout, network, credentials, hooks, or external filters in the lifecycle daemon
**Audit integration (2026-08-20):** Plan-audit F-008; fixed safe descriptor API and strict local trust-profile mechanics.
**Companion specifications:**

- `code_property_graph_present_state_fact_ontology_specification_v1.3.md`
- `present_state_cpg_fact_generation_specification_python_rust_v1.3.md`
- `present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md`
- `code_property_graph_semantic_query_specification_v1.3.md`
- `gix_rust_advanced_reference.md`

---

## 0. Synchronized CodeFabric 1.3 governing contract

This document is a released member of the synchronized **CodeFabric present-state CPG specification suite, version 1.3**. The suite integrates the architecture-completion contracts `G-01` through `G-84`; the earlier standalone completion specification is retained only as a historical design record and is no longer required to interpret this release.

The cross-cutting source of authority is `codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md`. This document permanently owns the domain contracts assigned to it by that manifest. A less-specific statement elsewhere in this document SHALL be read through the 1.3 contract sections and SHALL NOT override them.

### 0.1 Artifact identity and version

```yaml
artifact_id: "codefabric-continuous-cpg-lifecycle"
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

This document specifies how CodeFabric continuously maintains a coherent, current, queryable Code Property Graph while source files are being created, edited, renamed, deleted, temporarily broken, reformatted, regenerated, moved by Git operations, or changed concurrently by multiple programming agents.

The design SHALL:

- detect relevant worktree changes rapidly;
- distinguish common Git repository state from linked-worktree present-state source;
- convert watcher output into authoritative source-state reconciliation rather than blindly replaying filesystem events;
- use gix to interpret repository/worktree topology, paths, inclusion policy, HEAD, index, conflict stages, and bulk tracked-tree transitions;
- preserve stable current filesystem bytes and CodeFabric digests as the final source authority;
- update only the smallest sound invalidation domain;
- preserve query availability for unaffected facts;
- explicitly withdraw facts that are no longer justified by current source;
- support current syntax even while semantic providers fail;
- prevent partially generated, stale-Git-baseline, or cross-generation facts from becoming visible;
- provide atomic query snapshots;
- prioritize interactive freshness;
- use bounded concurrency and backpressure;
- recover from watcher loss, gix degradation, provider failure, daemon crash, and partial durable commits;
- converge to the same CPG as a clean Git-aware rebuild for the same worktree source state.

The core objective is:

> **An agent querying immediately after an edit receives either current facts or an explicit capability gap—never silently stale facts presented as current.**

The governing integration rule is:

> **notify discovers urgency; gix explains Git state and reduces candidate work; current filesystem bytes establish truth; CodeFabric controls semantic invalidation and atomic publication.**

## 2. Source basis

This specification uses the attached references as its technical basis.

| Reference | Relevant design contribution |
|---|---|
| `notify_debouncer_full_rust_reference.md` | Debounce semantics, event normalization, rename stitching, rescan handling, watcher lifecycle, bounded handoff, authoritative reconciliation, shutdown, and CPG integration |
| `gix_rust_advanced_reference.md` | Repository/worktree discovery, `git_dir`/`common_dir`/`work_dir`, byte-safe Git paths, index and conflict stages, status, pathspecs, attributes, ignores, Git-aware directory walking, HEAD/tree state, tree/blob diff, rename tracking, linked worktrees, submodules, locks/tempfiles, caches, interruption, thread-safety, security, and workflow-completeness boundaries |
| `rust_parallel_concurrency_stack_reference_2026-08-19.md` | Tokio/Rayon/Crossbeam/DashMap role separation, process-wide thread budgeting, bounded admission, cancellation, backpressure, incremental-indexer architecture, testing, and deployment |
| `rust_mir_cpg_continuous_reference_2026-08-18.md` | Two-speed syntax/semantic pipeline, rustc incremental reuse, owner fingerprints, compile-failure handling, compiler extraction manifests, subgraph replacement, and recovery |
| `present_state_cpg_fact_generation_specification_python_rust_v1.3.md` | Fact ownership, capability status, provider authority, dependency order, unknown materialization, and owner-scoped publication |
| `present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md` | Owner replacement, multi-table MVCC, publication manifests, DataFusion reconciliation, durable validation, query-snapshot pinning, and Delta recovery |
| `code_property_graph_semantic_query_specification_v1.3.md` | One atomically consistent query snapshot, explicit unavailable fact families, current-state semantics, and non-ambiguous empty-result handling |

### 2.1 Source-of-truth hierarchy

The lifecycle system SHALL use the following authority order:

```text
current stable filesystem bytes
    = authoritative present-state source content

CodeFabric BLAKE3 content digest
    = canonical current source-content identity

notify-debouncer-full
    = low-latency invalidation signal and rename assistance

gix
    = Git-aware repository/worktree topology, path/inclusion semantics,
      HEAD/index/operation-state interpretation, and candidate-delta accelerator

Tree-sitter / Ruff / Pyrefly / rustc / MIR
    = syntax and semantic fact providers

CodeFabric owner/fact-family reconciliation
    = present-state CPG mutation authority
```

A Git blob object ID SHALL NOT replace the CodeFabric current-byte digest. A gix status or tree-diff result SHALL generate candidate paths; it SHALL NOT directly mutate CPG facts or prove the current filesystem byte state.

### 2.2 Version and security stance

The default supported gix release is exactly:

```toml
gix = "=0.86.0"
notify-debouncer-full = "=0.7.0"
notify = "=8.2.0"
```

`gix <= 0.85.0` is below the required security floor for Windows-capable deployments because of the published incremental-checkout symlink/reparse-point issue. Although CodeFabric does not perform checkout, the dependency floor SHALL still be enforced.

All gix APIs SHALL be isolated behind application-owned DTOs and adapters. Exact method signatures and feature relationships SHALL be verified against version-matched `gix 0.86.0` rustdoc and the resolved Cargo feature graph.

Where this document adds mechanisms not directly specified by those references—most importantly the in-memory hot overlay—it does so as an architectural inference required to meet the stated interactive-latency target.

## 3. Scope

### 3.1 Included

This specification covers:

- daemon bootstrap and warm restart;
- exact Git repository and linked-worktree discovery;
- common-repository versus worktree identity;
- watcher registration and readiness for source roots and selected Git metadata;
- filesystem event ingestion;
- debounce and downstream coalescing;
- Git-native path, pathspec, ignore, exclude, and attribute interpretation;
- tracked, untracked, ignored, conflicted, submodule, and nested-repository classification;
- gix status and index candidate reduction;
- HEAD/tree state and tree-to-tree candidate diff for bulk Git transitions;
- byte-safe Git and platform path representations;
- file inventory and content snapshotting;
- change classification;
- owner discovery;
- invalidation planning;
- syntax-lane updates;
- Python semantic updates;
- Rust compiler/MIR updates;
- derived-fact recomputation;
- query-serving during updates;
- hot in-memory publication;
- asynchronous durable Delta publication;
- capability withdrawal and recovery;
- compile failure and parser failure;
- event loss and full reconciliation;
- Git metadata, index, HEAD, worktree, and submodule topology changes;
- provider crashes and timeouts;
- queue overflow and resource saturation;
- storage conflicts and partial commits;
- graceful and forced shutdown;
- testing, observability, performance, trust, and security policy.

### 3.2 Excluded

This specification does not introduce:

- Git-history analysis;
- prior-code-state querying;
- commit, blame, revision-walk, churn, or lineage facts;
- runtime observation;
- test-impact conclusions;
- refactor-safety conclusions;
- risk scoring;
- recommendations;
- unsaved editor-buffer overlays unless supplied through a future explicit overlay API;
- repository mutation by the CPG daemon;
- staging or index writes;
- ref or HEAD writes;
- checkout, reset, switch, restore, merge, rebase, cherry-pick, stash, or push orchestration;
- automatic fetch, clone, or submodule initialization;
- Git credential-helper execution;
- hook execution;
- repository-configured clean/smudge command execution by default.

Filesystem source bytes remain the present-state source of truth. gix improves Git-aware interpretation and work avoidance; it does not replace current source verification.

# Part I — Lifecycle and Scenario Inventory

## 4. Lifecycle phases

Every analyzed worktree passes through the following lifecycle phases.

```text
UNINITIALIZED
    ↓
GIT_DISCOVERING
    ↓
WATCH_REGISTERING
    ↓
BOOTSTRAPPING or WARM_RECOVERING
    ↓
GIT_STATE_VERIFYING
    ↓
READY
    ↓
COLLECTING_CHANGES
    ↓
GIT_METADATA_RECONCILING when required
    ↓
SNAPSHOTTING_SOURCE
    ↓
FAST_ANALYSIS
    ↓
FAST_PUBLICATION
    ↓
SEMANTIC_ANALYSIS
    ↓
DERIVED_RECOMPUTATION
    ↓
VALIDATION
    ↓
HOT_PUBLICATION
    ↓
DURABLE_FLUSH
    ↓
READY
```

Bulk Git transitions may enter:

```text
GIT_BULK_RECONCILING
    ↓
HEAD_TREE_DIFF
    ↓
STATUS_INDEX_RECONCILIATION
    ↓
AUTHORITATIVE_SOURCE_VERIFICATION
    ↓
ordinary fast/semantic/derived publication pipeline
```

Exceptional transitions may enter:

```text
RECONCILING
GIT_DEGRADED
DEGRADED
BLOCKED
STOPPING
FAILED
```

The daemon SHALL continue serving the newest valid immutable snapshot whenever doing so does not misrepresent freshness. A gix failure SHOULD degrade Git acceleration before it degrades source correctness; the system SHALL fall back to bounded generic filesystem reconciliation whenever the filesystem remains accessible.

## 5. Startup and recovery scenarios

### 5.1 Cold start without an index

**Trigger:** No valid durable publication exists.

**Required behavior:**

1. acquire the CodeFabric-owned daemon singleton lease;
2. register source-root and selected Git-metadata watchers before or concurrently with inventory;
3. start the event-generation counter;
4. open or discover the exact Git worktree through the gix adapter;
5. establish `GitRepositoryIdentity` and `GitWorktreeIdentity`;
6. capture initial `GitStateVector G0`;
7. enumerate included source files through Git-native path, exclude, attribute, and directory-walk semantics when the workspace is a Git worktree;
8. fall back to the bounded generic CodeFabric walker when Git is unavailable or the root is not a Git worktree;
9. reuse safe content-addressed cache entries for clean tracked files only under the blob-transform safeguards defined by this specification;
10. capture stable current source images and CodeFabric content digests for all remaining files;
11. perform complete fast and semantic generation;
12. derive required facts;
13. validate;
14. capture `GitStateVector G1`;
15. if materially relevant Git state changed between `G0` and `G1`, reconcile the candidate delta before publication;
16. create the first hot snapshot;
17. durably publish;
18. replay source and Git-metadata events received during bootstrap;
19. set `source_trust_state = CURRENT`, set `event_stream_health` to its independently determined value, and transition the workspace lifecycle to `READY`.

Queries before first valid snapshot SHALL return `WORKSPACE_BOOTSTRAPPING`.

### 5.2 Warm start with unchanged source and Git state

**Trigger:** A durable publication exists, current source inventory matches its digest, and the publication's compatible Git-state vector matches the active worktree.

**Required behavior:**

- open the durable base publication, reconstruct the overlay-aware catalog pinned by the recovered `ServingSnapshot`, and lease that snapshot for readiness validation;
- reopen the exact Git worktree;
- verify worktree identity, inclusion-policy fingerprint, HEAD/index compatibility, and source-inventory digest;
- construct the immutable serving snapshot;
- replay watcher events received during verification;
- skip regeneration;
- transition to `READY`.

A changed HEAD with byte-identical current source does not by itself require semantic regeneration. A changed inclusion-policy fingerprint does require inventory reconciliation.

### 5.3 Warm start with source changes made while daemon was stopped

**Trigger:** Durable source inventory differs from the current worktree.

**Required behavior:**

- capture the current Git-state vector;
- use bounded gix status/index candidate reduction where healthy;
- include current watcher-dirty paths and any Git metadata changes;
- use generic inventory when gix acceleration is unavailable;
- perform stable current-byte reads and CodeFabric digest comparison;
- classify added, removed, changed, moved, mode-changed, symlink-changed, and newly included/excluded files;
- run the ordinary incremental pipeline over that delta;
- do not assume watcher events exist for downtime changes.

### 5.4 Warm start after HEAD or worktree baseline transition

**Trigger:** The durable publication's HEAD tree differs from the current worktree's HEAD tree or the worktree operation state indicates a completed/interrupted Git transition.

**Required behavior:**

- compute a bounded gix tree-to-tree candidate diff when both trees are available;
- combine the diff with current gix status/index candidates;
- include untracked, conflicted, submodule, and watcher-dirty paths;
- verify every candidate against current filesystem bytes;
- reconcile inclusion inventory;
- publish one coherent bulk update wave.

The tree diff is an accelerator, not a source-content authority.

### 5.5 Warm start with orphan staging publications

**Trigger:** Delta contains `STAGING`, incomplete, or failed publications not referenced by `current_publication`.

**Required behavior:**

- preserve the current pointer;
- inspect operation IDs and checksums;
- resume only idempotent known-safe work;
- otherwise mark abandoned;
- reconcile current source and current Git state independently;
- schedule cleanup after pinned-version safety checks.

### 5.6 Warm start after crash with unflushed hot overlay

**Trigger:** Source files are newer than the durable publication and no durable overlay journal exists.

**Required behavior:**

- rebuild from current source;
- never assume lost in-memory overlay contents;
- use gix status/tree candidates to reduce recovery reads when possible;
- keep the old durable publication queryable only with `POTENTIALLY_STALE` status until reconciliation completes;
- strict-current queries SHALL wait or fail explicitly.

### 5.7 Bare repository

A bare repository has no current filesystem worktree and therefore cannot directly satisfy the present-state worktree CPG contract.

Default behavior:

```text
bare repository + no explicit materialized source root
    → recognize repository topology
    → mark source capability unavailable
    → do not start ordinary source watcher or CPG generation
```

A future virtual-tree mode is a separate product profile and is not implied by gix tree/blob access.

### 5.8 Linked worktree discovery

When gix discovers multiple linked worktrees:

- create or attach one `WorktreeCoordinator` per authorized analyzed worktree;
- give each worktree its own watcher, source inventory, update waves, source generation, hot overlay, semantic-provider state, and serving snapshot;
- share immutable object-database/cache resources only through a common-repository service;
- never merge current source facts across worktrees.

### 5.9 Git metadata corrupt or unavailable at startup

When source files remain accessible:

- classify Git acceleration as `DEGRADED` or `UNAVAILABLE`;
- use the generic bounded inventory walker;
- capture current filesystem bytes and CodeFabric digests;
- continue CPG generation;
- expose a Git-state diagnostic;
- retry gix health independently.

gix failure SHALL not cause false source-current claims.

## 6. Routine file-operation scenarios

### 6.1 Isolated source modification

- mark the path dirty;
- read stable current bytes;
- compute the CodeFabric digest;
- classify the update;
- invalidate only affected owners and dependent fact families;
- supersede older work for the same path/owner;
- do not run full gix status for an ordinary isolated save.

### 6.2 Repeated saves of one file

- coalesce into one latest dirty generation;
- never commit analysis produced from an older digest;
- allow older CPU work to finish only when cancellation cost exceeds expected savings;
- discard stale output by generation check.

### 6.3 Editor atomic save

Common shape:

```text
write temp
rename temp over target
remove/replace old inode
```

Required behavior:

- treat the watcher sequence as an invalidation hint;
- stat and read the final target path;
- compare the current digest;
- combine notify rename stitching, filesystem file-ID evidence, current content, and—only when needed—gix status/rename evidence;
- preserve file identity only when the evidence supports it;
- never apply raw delete/create semantics directly to graph facts.

### 6.4 File creation

- add source-file facts;
- classify tracked/untracked/ignored state through the Git-state adapter where available;
- exclude ignored untracked files unless explicitly overridden by CodeFabric policy;
- discover semantic owners;
- generate source, syntax, semantic, and derived facts;
- invalidate import/module topology where relevant;
- create cross-owner facts owned by affected callers/importers.

### 6.5 File deletion

- verify current absence;
- tombstone the file owner and all descendant owners;
- remove owner-scoped facts;
- invalidate incoming references, imports, and call-target facts as required;
- materialize explicit unknown targets/modules when unresolved references remain;
- update index/status operational state without treating it as source truth.

### 6.6 File rename within the same logical module

- verify final current content;
- use evidence in this order: current worktree path, stable filesystem file ID, notify tracker/file-ID evidence, bounded gix rename candidate when applicable, identical content digest, and source structural identity;
- preserve file-instance identity when justified;
- update path facts and source spans;
- retain semantic identities only when module/qualified-name identity is unchanged;
- recompute path-sensitive imports and source correspondence.

### 6.7 File move changing module/package identity

- treat the operation as more than a path rename;
- invalidate module identity, imports, exports, qualified names, semantic IDs, and dependents;
- preserve content lineage only as an operational optimization;
- publish new present-state semantic identity.

### 6.8 Directory rename

- apply prefix-aware source inventory update;
- verify descendants;
- use gix path semantics where the directory is within a Git worktree;
- recompute path/module identities;
- broaden invalidation when package/module resolution changes.

### 6.9 Directory deletion

A single parent removal may stand in for many child events.

Required behavior:

- enumerate indexed descendants under the normalized Git/worktree prefix;
- delete all affected owners;
- recompute dependency and derived facts;
- validate no dangling relation endpoints.

### 6.10 Metadata-only event

- stat the file;
- inspect Git mode/type changes when relevant;
- if content digest and semantic file kind are unchanged, emit no semantic CPG update;
- retain operational timestamp and Git-state change only outside the semantic CPG.

### 6.11 Content digest unchanged

- classify as `NO_OP` unless path, source-file kind, inclusion state, or semantic metadata changed;
- do not parse;
- do not publish a new semantic snapshot solely because HEAD or index changed.

### 6.12 Whitespace/comment-only change

When a semantic-token fingerprint proves semantics unchanged:

- update source, token, comment, syntax, and span facts;
- reanchor semantic facts through exact structural mapping;
- avoid compiler/type reanalysis when safe;
- invalidate documentation/directive semantics if changed.

Python indentation, type comments, semantic directives, Rust attributes, and macro token trees SHALL prevent unsafe classification as trivia-only.

### 6.13 Formatter change across many files

- detect high path count and high semantic-token reuse;
- use Git-aware candidate pruning plus parallel hashing and fast syntax remapping;
- avoid one Delta publication per file;
- create one or a few multi-owner hot snapshots;
- micro-batch durable flush.

### 6.14 Generated-source burst

- exclude generated outputs that are not part of the analyzed source model using Git and CodeFabric inclusion policy;
- for included generated source, use bulk-reconcile mode;
- prevent output→watch→generator feedback loops.

### 6.15 File extension or language change

- remove old language-profile owners;
- create new language-profile owners;
- recompute module/dependency topology;
- never reinterpret old provider facts under the new language.

### 6.16 Symlink creation, deletion, or retargeting

- apply configured symlink policy;
- authorize the resolved target;
- use gix mode/status information as supplemental evidence;
- treat target or file-kind change as potential subtree replacement;
- avoid lexical-prefix-only trust decisions;
- never follow symlinks outside authorized roots.

### 6.17 Permission or transient read failure

- keep the file marked dirty;
- invalidate facts that cannot be justified;
- retry with bounded backoff;
- report `SOURCE_UNREADABLE`;
- never preserve old facts as current merely because read failed.

### 6.18 File changes while being read

- compare metadata and digest before and after read;
- retry until a stable source image is captured or deadline is reached;
- include the current Git-state fence for Git-derived candidate waves;
- never analyze torn bytes.

### 6.19 Oversized or binary-like source file

- mark source capability `EXCLUDED_BY_LIMIT` or `UNSUPPORTED_CONTENT`;
- expose path, size, Git classification, and diagnostic;
- avoid parsing;
- do not claim absence of semantic facts.

### 6.20 Index-only staging change

When the index changes but current worktree bytes do not:

- refresh the operational `GitStateVector`;
- do not invalidate source or semantic CPG facts;
- update conflict-stage status if applicable;
- avoid semantic publication unless client-visible operational state is snapshot-scoped.

### 6.21 Executable-bit-only change

- update source-file mode metadata;
- do not regenerate Python/Rust semantics unless execution policy depends on the bit;
- preserve current content digest and semantic owners.

### 6.22 Git file-kind or symlink-mode change

- treat regular-file↔symlink, file↔directory, and Git mode changes as source-topology invalidations;
- verify current filesystem object type;
- apply root/symlink authorization;
- reconcile descendants where required.

### 6.23 `.gitignore`, `info/exclude`, or trusted exclude change

- recompute the inclusion-policy fingerprint;
- run Git-aware dirwalk over affected scope;
- include tracked files regardless of later ignore matching;
- tombstone newly excluded untracked files;
- generate newly included files;
- leave unchanged still-included owners untouched.

### 6.24 `.gitattributes` or trusted attribute-source change

- recompute the attribute fingerprint;
- identify paths whose blob/worktree transformation or classification semantics may have changed;
- invalidate blob-OID cache equivalence;
- preserve current-byte source facts unless current bytes or language interpretation changed;
- never execute external filters by default.

### 6.25 Git metadata event

Raw changes under selected Git metadata paths SHALL be normalized to semantic categories such as:

```text
HEAD_CHANGED
INDEX_CHANGED
REPOSITORY_OPERATION_STATE_CHANGED
INCLUSION_POLICY_CHANGED
ATTRIBUTES_CHANGED
SUBMODULE_TOPOLOGY_CHANGED
WORKTREE_TOPOLOGY_CHANGED
REPOSITORY_CONFIG_CHANGED
```

The daemon SHALL reopen/re-read current gix state rather than replay metadata file operations.

### 6.26 Conflict-stage change

- reload index state;
- mark affected paths `GIT_CONFLICTED`;
- analyze current filesystem conflict content normally;
- do not collapse stages to a fictitious stage-zero fact;
- expose conflict metadata in query operational status.

### 6.27 Linked-worktree addition or removal

- refresh common-repository worktree topology;
- create a separate coordinator for each newly authorized worktree;
- stop and retire a removed worktree coordinator after verification;
- keep common immutable ODB caches separate from worktree-specific HEAD/index/source state.

### 6.28 Submodule or nested-repository topology change

- update parent operational topology;
- register an initialized authorized submodule as a separate workspace when configured;
- never flatten child source into the parent worktree identity;
- never auto-fetch or auto-initialize;
- classify non-submodule nested repositories according to explicit workspace policy.

## 7. Python-specific lifecycle scenarios

### 7.1 Valid Python body-local edit

- Tree-sitter and Ruff parse current source;
- invalidate containing callable/module owners;
- rerun local CFG/dataflow;
- request Pyrefly refresh for the affected module;
- propagate summary changes only if hashes change.

### 7.2 Python parse error

- publish current source and Tree-sitter error-tolerant CST;
- publish Ruff diagnostics if available;
- withdraw typed-AST-dependent and semantic fact families for affected ranges/owners;
- return source context for queries needing unavailable semantics.

### 7.3 Python type error with valid parse

- retain current syntax and source semantics;
- publish Pyrefly facts that its response contract declares valid;
- mark unresolved or failed type/call capabilities explicitly;
- do not treat diagnostics as provider failure by default.

### 7.4 Pyrefly sidecar unavailable

- retain current Ruff/Tree-sitter/local-binding facts;
- withdraw or mark unavailable project types, cross-module definitions, members, and call targets for affected modules;
- schedule sidecar restart;
- avoid negative claims in unavailable families.

### 7.5 Python import-root or project-config change

Even though environment inventory is not a CPG domain, analysis configuration is an extractor input.

Required behavior:

- invalidate module resolution, cross-module references, types, and call targets across the affected project;
- preserve source/syntax;
- rerun Pyrefly project analysis;
- publish semantic capabilities only under one coherent selected context.

### 7.6 Stub or external dependency interface change

- invalidate dependent module type and member facts;
- preserve unrelated source facts;
- use content-addressed dependency summaries where available.

---

## 8. Rust-specific lifecycle scenarios

### 8.1 Valid Rust body-local edit

- update Tree-sitter source syntax immediately;
- identify likely containing item;
- schedule incremental rustc analysis for affected target;
- replace MIR owner facts only after a complete extraction manifest;
- recompute owner-local derived facts;
- propagate interprocedural summaries until stable.

### 8.2 Rust syntax error

- publish current source and Tree-sitter CST with error/missing nodes;
- withdraw compiler semantic, MIR, ownership, and type facts for invalidated owners;
- retain unaffected owners proven current;
- attach rustc diagnostics when available.

### 8.3 Rust type, borrow, or lifetime error

- source and syntax remain current;
- compiler diagnostics are current;
- semantic/MIR capabilities for invalidated owners or compilation unit become unavailable;
- unaffected owner facts may remain only when invalidation analysis proves their dependencies unchanged.

### 8.4 Rust crate compile failure

Default policy:

```text
current source/syntax           publishable
new compiler-semantic facts     not publishable without complete manifest
invalidated old semantic facts  hidden/tombstoned in current snapshot
unaffected current owners       retained if soundly outside invalidation frontier
compiler diagnostics            publishable
```

The current query snapshot SHALL NOT silently combine current syntax with old semantic facts for invalidated owners.

### 8.5 Build script or procedural macro failure

- mark affected compilation units unavailable;
- preserve source-level invocation/declaration syntax;
- withdraw generated/expanded/compiler facts whose generation cannot be proven current;
- isolate and time-limit the compiler process;
- treat untrusted build scripts/proc macros as process-sandbox concerns.

### 8.6 Macro definition change

- invalidate every invocation and generated owner reachable through macro dependency facts;
- rerun affected compilation units;
- prefer broader invalidation when expansion dependency is uncertain.

### 8.7 Trait, impl, public signature, or generic-bound change

- invalidate direct semantic owner;
- invalidate call-target resolution, trait candidate sets, monomorphized instances, and dependent summaries;
- propagate through reverse semantic dependency graph;
- recompute SCCs only in affected graph region when safe.

### 8.8 Cargo manifest, lockfile, target, feature, or toolchain input change

- classify as build-context invalidation;
- invalidate affected crate graph and compiler-semantic capabilities;
- preserve source/syntax;
- run Cargo metadata;
- rebuild compilation-unit mapping;
- rerun rustc under selected context.

### 8.9 Crate/module file topology change

- recompute Cargo target/module ownership;
- invalidate qualified identities, imports/uses, generated definitions, and cross-crate edges where affected.

### 8.10 rustc extractor protocol failure

- reject the entire incomplete compiler event;
- do not commit owner rows without valid begin/owner/end manifest semantics;
- retain prior durable data only outside the active current snapshot;
- expose provider failure status.

---

## 9. Bulk and exceptional filesystem scenarios

### 9.1 Branch switch, checkout, or large tracked-tree transition

The watcher may emit thousands of events without a transaction boundary.

Required behavior:

1. detect the source/Git metadata event storm and enter `GIT_BULK_RECONCILING`;
2. cancel or supersede per-file semantic jobs;
3. capture the old and new HEAD/tree identities when available;
4. compute a bounded gix tree-to-tree diff for tracked candidate paths;
5. optionally run bounded rewrite/rename detection when candidate count is below policy limits;
6. run current gix status/index reconciliation for surviving local modifications, untracked files, conflicts, mode changes, and submodules;
7. union the gix candidates with watcher-dirty paths;
8. reconcile inclusion policy;
9. read and hash current filesystem bytes for every candidate;
10. publish one fast syntax wave;
11. run semantic and derived convergence;
12. publish one coherent hot snapshot;
13. durable-flush only after the Git stabilization tuple is coherent.

The stabilization tuple is:

```text
HEAD target
HEAD tree
index fingerprint
repository operation state
watcher event watermark
dirty-path set
inclusion-policy fingerprint
attributes fingerprint
```

Tree diff and status are candidate accelerators; current bytes remain authoritative.

### 9.2 Large untracked/generated patch application

- enter `BULK_RECONCILE` when thresholds are exceeded;
- use Git-native inclusion/dirwalk to prune ignored output;
- inventory untracked included paths;
- compute current digests in parallel;
- prioritize active/query-target files first;
- converge the remainder in background.

### 9.3 Watcher event loss or overflow

On `need_rescan()` or equivalent completeness-threatening error:

- set source trust state to `UNVERIFIED`;
- enter reconciliation generation;
- coalesce duplicate rescan signals;
- use bounded gix status/index candidate reduction where healthy;
- broaden to Git-aware full inventory when candidate completeness is uncertain;
- fall back to generic full inventory on gix failure;
- fence events arriving during reconciliation;
- publish only after the corrected source snapshot and Git-state vector are coherent.

### 9.4 Watcher backend failure

- mark watcher health degraded;
- continue serving the pinned snapshot with freshness warning;
- retry registration or switch to polling according to policy;
- run authoritative Git-aware or generic reconciliation after recovery.

### 9.5 Watched root deletion

- verify whether the worktree was removed, moved, or temporarily unavailable;
- use common-repository worktree topology where available;
- mark all contained source owners removed only after verification;
- attempt rewatch according to policy.

### 9.6 Difficult or network filesystem

- validate native watcher and gix filesystem behavior at deployment;
- use `PollWatcher` when required;
- separate poll interval from debounce interval;
- cap poll amplification, status traversal, and content comparison;
- preserve generic-reconcile fallback.

### 9.7 Event ingress queue saturation

The watcher callback SHALL NOT block indefinitely.

Recommended policy:

1. attempt bounded nonblocking enqueue;
2. if full, set `reconcile_required` atomically;
3. increment dropped-to-reconcile metric;
4. discard individual event details;
5. schedule Git-aware authoritative reconciliation.

### 9.8 Git state changes during status or inventory scan

- capture `GitStateVector G0` before the operation;
- capture `G1` after the operation;
- reject the candidate delta when relevant HEAD, index, operation, or inclusion fingerprints changed;
- retry or escalate to a broader reconcile.

### 9.9 Corrupt or unavailable Git metadata

- classify Git acceleration `DEGRADED`;
- do not infer a clean worktree;
- use bounded generic inventory and current-byte digests;
- retain query service correctness;
- retry gix health separately.

### 9.10 Sparse or special index behavior

- rely on gix's released interpretation where explicitly supported;
- never reimplement index extensions casually;
- fall back to authoritative inventory when sparse/skip-worktree semantics are uncertain;
- require Git CLI parity fixtures before enabling sparse-checkout-specific optimizations.

## 10. Query and concurrency scenarios

### 10.1 Query during update

- pin the latest immutable serving snapshot;
- continue against that snapshot even if a newer one publishes;
- never mix base and overlay generations;
- return pending/unavailable capability metadata.

### 10.2 Strict-current query during pending update

Depending on request policy:

- wait for a freshness barrier up to deadline;
- trigger targeted priority refresh;
- or return `CURRENT_FACTS_UNAVAILABLE`.

### 10.3 Multiple agents editing concurrently

- filesystem source is authoritative;
- dirty generations coalesce by path and owner;
- newer edits supersede older jobs regardless of client;
- each query pins one snapshot;
- per-agent STDIO processes SHALL not own separate mutable graph state.

### 10.4 One agent queries the file another agent is editing

- return one pinned snapshot;
- identify pending source generation if newer events exist;
- strict mode waits or fails;
- never splice direct filesystem text into semantic results from a different snapshot without explicit source-only labeling.

### 10.5 Long query overlaps publication

- query keeps its `Arc<ServingSnapshot>`;
- publication constructs a new immutable snapshot off-path;
- atomic pointer swap does not invalidate running query.

### 10.6 Update superseded while running

- job checks generation before expensive stages and before publication;
- stale outputs are discarded;
- side-effect-free CPU work may finish but cannot commit;
- compiler child may be terminated according to cancellation-cost policy.

---

## 11. Storage and publication scenarios

### 11.1 Hot snapshot published, durable flush pending

- queries use hot snapshot;
- durable lag is reported operationally;
- source files remain recovery authority;
- new updates may supersede or merge into hot overlay.

### 11.2 Durable Delta commit conflict

- reload latest writable table state;
- inspect operation metadata;
- retry idempotently;
- never blindly append;
- current serving snapshot remains unchanged.

### 11.3 Crash during table updates

- active durable pointer remains unchanged;
- hot snapshot is lost unless journaled;
- incomplete Delta versions remain unreferenced;
- startup reconciliation restores current source.

### 11.4 Crash after publication complete but before pointer swap

- completed publication is not active;
- startup may validate and activate it only if source digest still matches;
- otherwise abandon and reconcile.

### 11.5 Crash after pointer swap but before catalog refresh

- `current_publication` is authoritative;
- startup/query catalog reopens exact pinned versions;
- no semantic inconsistency results.

### 11.6 Validation failure

- reject candidate snapshot/publication;
- retain old active snapshot;
- mark affected wave failed;
- schedule targeted retry or full reconcile.

### 11.7 Derived fixed point exceeds limits

- do not publish incomplete derived family as complete;
- publish base/current local facts only if capability statuses explicitly mark derived family unavailable;
- schedule broader or offline recomputation.

### 11.8 Disk full or spill directory exhausted

- stop new durable/large derived work;
- continue serving immutable snapshot;
- surface `STORAGE_BLOCKED`;
- avoid corrupting current pointer.

---

## 12. Shutdown and upgrade scenarios

### 12.1 Graceful shutdown with no pending work

- reject new update commands;
- stop watcher with joined shutdown;
- close ingress;
- finish active queries;
- flush optional durable work;
- close sidecars and runtimes.

### 12.2 Graceful shutdown with pending work

Policy options:

```text
DRAIN:
  finish current coherent wave and durable publication

CANCEL:
  discard uncommitted wave and retain current snapshot
```

A bounded deadline SHALL choose between them.

### 12.3 Forced shutdown during update

- no partially mutable serving snapshot exists;
- durable pointer remains at last complete publication;
- restart reconciles source.

### 12.4 Provider or toolchain upgrade

- version adapter and derivation bundle;
- invalidate facts whose producer semantics changed;
- run golden and clean-rebuild equivalence suites;
- publish only after schema/protocol compatibility validation.

### 12.5 Schema migration

- create new table root when required;
- transform current publication;
- validate Arrow/Delta/DataFusion contracts;
- activate via manifest pointer;
- preserve old version only through recovery window.

---

# Part II — Update Categories and Invalidation

## 13. Canonical update classes

Every dirty path, Git metadata transition, or analysis-input change SHALL be classified after current-state verification.

| Code | Update class | Typical trigger | Minimum action |
|---|---|---|---|
| `U0` | No-op | Event, HEAD, or index change but current source/inclusion unchanged | Operational state only |
| `U1` | Source-layout only | Line endings, comments, whitespace with semantic-token equivalence | Source/token/syntax/span remap |
| `U2` | File-local syntax | Expression/statement edit with no declaration interface change | File syntax + owner-local analysis |
| `U3` | Owner-local semantics | Function body, local binding, local type flow | Owner semantic/CFG/dataflow replacement |
| `U4` | Module/type interface | Signature, class member, trait/impl, import/export | Module/type and direct dependents |
| `U5` | Interprocedural semantic | Dispatch set, callable contract, public type relation | Reverse dependency and summary propagation |
| `U6` | Compilation unit/project/inclusion context | Cargo/Python project config, macro/build context, `.gitignore`, trusted excludes, attributes | Affected project, crate, or inventory scope |
| `U7` | Worktree reconciliation | Event loss, branch switch, worktree topology change, bulk patch, inclusion-policy change | Authoritative Git-aware/generic inventory delta |
| `U8` | Fabric/provider migration | Schema, ontology, gix/provider, or derivation version | Controlled regeneration/migration |

Classification is an optimization. When uncertain, the planner SHALL broaden invalidation.

### 13.1 Git-aware update subcodes

| Subcode | Parent | Meaning |
|---|---|---|
| `UG0` | `U0` | Git metadata changed but current source and inclusion did not |
| `UG1` | `U1/U2` | Path, mode, symlink, or source-layout change |
| `UG2` | `U6` | Ignore, attribute, or inclusion policy changed |
| `UG3` | `U6` | Index or conflict-stage state changed |
| `UG4` | `U7` | HEAD tree or bulk tracked baseline changed |
| `UG5` | `U7` | Linked-worktree, submodule, or nested-repository topology changed |
| `UG6` | `U7` | Git state could not be trusted; generic authoritative reconcile required |

Subcodes improve orchestration and observability. They SHALL NOT replace owner/fact-family invalidation semantics or become semantic CPG facts.

## 14. Invalidation dimensions

Invalidation is not one Boolean. It is a set over:

```text
worktree
owner
fact family
representation
provider capability
derived projection
```

Git operational state is evaluated separately:

```text
path inclusion
path/file kind
HEAD/index baseline
conflict stage
worktree/submodule topology
```

Example:

```text
worktree: linked worktree A
owner: Rust function `parse`
invalidate:
  source spans
  source syntax
  MIR body
  CFG
  def-use
  ownership
retain:
  unrelated module imports
  other function MIR
```

An index-only staging event may update the `GitStateVector` while invalidating no CPG fact family.

### 14.1 Fact-family groups

```text
SOURCE
RAW_SYNTAX
TYPED_SYNTAX
LOCAL_BINDINGS
PROJECT_DEFINITIONS
TYPES
MEMBERS
CALL_TARGETS
CFG
DATAFLOW
ALIAS
OWNERSHIP
BORROWCK
EFFECTS
MACRO_PROVENANCE
GENERATED_CODE
DERIVED_CONTROL
INTERPROCEDURAL_SUMMARY
```

### 14.2 Operational Git-state groups

```text
REPOSITORY_LAYOUT
WORKTREE_LAYOUT
HEAD_BASELINE
INDEX_BASELINE
CONFLICT_STAGES
REPOSITORY_OPERATION_STATE
INCLUSION_POLICY
ATTRIBUTE_POLICY
SUBMODULE_TOPOLOGY
WORKTREE_TOPOLOGY
GIT_ACCELERATION_HEALTH
```

### 14.3 Invalidation result

```rust
pub struct InvalidationPlan {
    pub worktree_id: WorktreeId,
    pub source_generation: u64,
    pub git_state_fence: Option<GitStateVector>,
    pub changed_files: Vec<FileId>,
    pub changed_owners: Vec<OwnerId>,
    pub capability_withdrawals: Vec<CapabilityWithdrawal>,
    pub owner_replacements: Vec<OwnerReplacementPlan>,
    pub dependent_owners: Vec<OwnerId>,
    pub derived_scopes: Vec<DerivedScope>,
    pub build_units: Vec<BuildUnitId>,
    pub inventory_reconcile: bool,
    pub bulk_reconcile: bool,
}
```

## 15. Ownership and relation invalidation

Facts SHALL have deterministic replacement owners within one `worktree_id`.

Recommended ownership:

```text
file source/syntax facts                 → file owner
Python local semantics                   → module/file owner
Python CFG/dataflow                      → callable/module owner
Rust MIR facts                           → MIR body owner
call-site target edges                   → caller/call-site owner
type/member declarations                 → declaring type/module owner
derived CFG facts                        → CFG owner
interprocedural summary                  → callable/instance owner
```

Incoming edges are not automatically owned by their target.

A callee body edit therefore does not require rewriting all incoming call-site edges unless:

- callable identity changed;
- signature/dispatch contract changed;
- target resolution may change;
- the callee was removed.

Git operational changes follow separate rules:

- HEAD or index identity does not own semantic facts;
- a path/inclusion change invalidates the file owner only after current state is verified;
- a linked worktree has independent owners even when Git blob objects are shared;
- submodule source owners belong to the child worktree namespace, not the parent.

## 16. Invalidation propagation graph

The daemon SHALL maintain an operational dependency graph between owners and fact families.

Edge categories include:

```text
imports/module dependency
references semantic declaration
calls callable
uses type
implements trait/protocol
inherits type
macro expansion dependency
generated-from dependency
summary depends on callee summary
derived projection depends on base owner
path/module identity depends on worktree-relative path
source inclusion depends on ignore/attribute policy
build unit depends on manifest/lock/toolchain input
```

Propagation algorithm:

1. seed changed worktree/owner/family pairs;
2. traverse only relationship categories affected by the update class;
3. add dependent owner/family pairs;
4. stop when a dependent fingerprint is unchanged;
5. escalate to component/project rebuild at threshold;
6. reject propagation results if the Git/source generation fence became stale.

Git tree-diff, status, and rename candidates seed this graph only after current filesystem verification.

## 17. Safe semantic-reuse fast path

Semantic facts MAY be reused without provider reanalysis only when a conservative proof exists.

Required conditions SHOULD include:

- normalized semantic-token stream unchanged;
- declarations and owner boundaries map exactly;
- semantic directives unchanged;
- no module path or configuration change;
- no macro token-tree change;
- source-anchor mapping is complete and unambiguous.

When these conditions hold:

- update source and syntax facts;
- rewrite spans/source correspondence;
- retain semantic payload;
- label derivation as `REANCHORED_UNCHANGED_SEMANTICS`.

If any condition fails, rerun semantic provider.

---

# Part III — State Model

## 18. Workspace lifecycle state

```text
UNINITIALIZED
GIT_DISCOVERING
WATCH_REGISTERING
BOOTSTRAPPING
WARM_RECOVERING
GIT_STATE_VERIFYING
READY
UPDATING_FAST
UPDATING_SEMANTIC
RECONCILING
GIT_BULK_RECONCILING
GIT_DEGRADED
DEGRADED
BLOCKED
STOPPING
FAILED
```

Only one coordinator task SHALL mutate one worktree's lifecycle state.

A common-repository actor MAY coordinate shared immutable Git resources and linked-worktree topology, but it SHALL NOT own worktree source generations or serving snapshots.

## 19. Source trust and event-stream health

Source truth and watcher health are orthogonal.

### 19.1 `SourceTrustState`

```text
UNVERIFIED          active snapshot may not represent current bytes
VERIFYING           authoritative inventory/digest verification is running
CURRENT             required current-byte checks and generation fences passed
POTENTIALLY_STALE   prior valid snapshot intentionally exposed only by BEST_AVAILABLE_SNAPSHOT
UNAVAILABLE         source root cannot currently be verified/read
```

`CURRENT` replaces the ambiguous prior terminal labels `TRUSTED` and `STABLE`.

### 19.2 `EventStreamHealth`

```text
HEALTHY
RESCAN_REQUIRED
DEGRADED
UNAVAILABLE
```

A watcher overflow sets `RESCAN_REQUIRED` and source trust to `UNVERIFIED` until reconciliation completes. A degraded watcher does not permanently prevent `CURRENT` after a generic authoritative inventory proves the source state; it does require continued reconciliation strategy and diagnostics.

### 19.3 Reconciliation phase

`RECONCILING` is a workspace lifecycle phase, not a source-trust value. `GIT_STATE_CHANGED_DURING_SCAN` is a reconciliation reason/diagnostic, not a stable trust state.

## 20. Git acceleration and lifecycle state

```text
NOT_A_GIT_WORKTREE
GIT_UNAVAILABLE
GIT_READY
GIT_METADATA_DIRTY
GIT_SCANNING
GIT_OPERATION_IN_PROGRESS
GIT_BULK_RECONCILING
GIT_DEGRADED
```

Semantics:

- `NOT_A_GIT_WORKTREE`: generic CodeFabric inventory is authoritative.
- `GIT_UNAVAILABLE`: the root is expected to be Git-aware but the adapter cannot currently open or interpret it.
- `GIT_READY`: topology and state vector are current.
- `GIT_METADATA_DIRTY`: selected metadata changed and current state must be reread.
- `GIT_SCANNING`: status, index, dirwalk, or tree-diff work is active.
- `GIT_OPERATION_IN_PROGRESS`: merge, rebase, cherry-pick, revert, bisect, apply, or another recognized operation is active.
- `GIT_BULK_RECONCILING`: a HEAD-tree or worktree-scale transition is being reconciled.
- `GIT_DEGRADED`: Git acceleration failed or is untrusted; generic current-byte reconciliation remains available.

These are operational states, not semantic CPG facts.

## 21. Update-wave state

```text
COLLECTING
GIT_CANDIDATE_BUILDING
GIT_BASELINE_VERIFYING
SNAPSHOTTING
CLASSIFYING
FAST_ANALYZING
FAST_VALIDATING
FAST_PUBLISHED
SEMANTIC_ANALYZING
DERIVING
VALIDATING
HOT_PUBLISHED
DURABLE_FLUSHING
DURABLE_PUBLISHED
SUPERSEDED
CANCELLED
FAILED
```

The registry migration from the pre-production coarse machine is append-only and
explicit. Exact-name states retain their existing codes: `COLLECTING = 10`,
`SNAPSHOTTING = 20`, `FAILED = 60`, and `SUPERSEDED = 70`. The coarse states
`RUNNING = 30`, `PUBLISHING = 40`, and `COMPLETE = 50` remain decode-only deprecated
values with no transition target and are never emitted by the new scheduler. New states
append as follows:

```text
80 GIT_CANDIDATE_BUILDING       90 GIT_BASELINE_VERIFYING
100 CLASSIFYING                 110 FAST_ANALYZING
120 FAST_VALIDATING             130 FAST_PUBLISHED
140 SEMANTIC_ANALYZING          150 DERIVING
160 VALIDATING                  170 HOT_PUBLISHED
180 DURABLE_FLUSHING            190 DURABLE_PUBLISHED
200 CANCELLED
```

Before activation, migration inventories persisted rows. Terminal historical coarse
`COMPLETE` rows may remain readable as historical evidence. Any persisted nonterminal
`RUNNING` or `PUBLISHING` row is rejected with a migration diagnostic and a new
generation starts from `COLLECTING`; no ambiguous mid-wave continuation is inferred.
Generated transition code, writers, fixtures, and SQL constraints prove zero new
emission of deprecated codes.

A wave is immutable after `HOT_PUBLISHED`.

A Git-accelerated wave cannot advance beyond `GIT_BASELINE_VERIFYING` if its relevant `GitStateVector` changed.

## 22. Provider-run state

```text
QUEUED
RUNNING
SUCCEEDED
PARTIAL
FAILED
TIMED_OUT
CANCELLED
SUPERSEDED
CRASHED
PROTOCOL_ERROR
STALE_RESULT
STALE_GIT_BASELINE
```

Every provider or gix-operation output SHALL include the source generation and input baseline it analyzed.

## 23. Owner capability and completeness state

The lifecycle uses the ontology 1.3 owner-capability registry:

```text
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
```

Completeness is a separate value: `COMPLETE`, `PARTIAL`, `INDETERMINATE`, `UNAVAILABLE`, or `NOT_APPLICABLE`.

Each record SHALL include:

```text
workspace_id
analysis_context_id
owner_id
capability_code
source_generation
snapshot_id when activated
provider_run_id when applicable
reason_code
diagnostic_id optional
fallback_source_available
coverage_scope_fingerprint
```

Provider-run state remains separate and maps to the shared provider registry. A capability may be `CURRENT` but conservative/partial; that distinction is carried by completeness and certainty/resolution, not one overloaded status.

## 24. Durable publication and serving activation states

Hot and durable transitions use separate state machines.

### 24.1 `DurablePublicationState`

```text
STAGING
VALIDATING
VALIDATED
COMMITTING
COMPLETE
FAILED
ABANDONED
```

### 24.2 `ServingActivationState`

```text
BUILDING
VALIDATING
READY
ACTIVE
RETIRED
FAILED
```

A publication in `COMPLETE` may be a durable base without being the currently active query snapshot. `HOT_ACTIVE` and `DURABLE_ACTIVE` SHALL not be stored in the durable publication state column.

## 25. Query freshness, availability, and execution state

Public query status is orthogonal:

```text
freshness_state:
  CURRENT
  AWAITING_CURRENT
  POTENTIALLY_STALE
  UNAVAILABLE

query_availability_state:
  AVAILABLE
  PARTIAL
  UNAVAILABLE
  NOT_APPLICABLE

query_execution_state:
  ACCEPTED
  RUNNING
  COMPLETE
  FAILED
  CANCELLED
  DEADLINE_EXCEEDED
  NOT_EXECUTED_DEPENDENCY
```

Workspace conditions such as `WORKSPACE_BOOTSTRAPPING`, `WORKSPACE_BLOCKED`, source unverified, Git acceleration degraded, and pending updates are reason codes/diagnostics attached to these orthogonal fields. `GIT_ACCELERATION_DEGRADED` alone does not imply stale source after generic verification.

## 26. Core Rust state structures

```rust
pub struct WorkspaceCoordinatorState {
    pub workspace_id: WorkspaceId,
    pub repository_id: Option<RepositoryId>,
    pub worktree_id: Option<WorktreeId>,
    pub lifecycle: WorkspaceLifecycleState,
    pub source_trust: SourceTrustState,
    pub event_stream_health: EventStreamHealth,
    pub git_acceleration: GitAccelerationStatus,
    pub git_state: Option<GitStateVector>,
    pub next_event_seq: u64,
    pub newest_dirty_generation: u64,
    pub active_analysis_context_set: AnalysisContextSetId,
    pub active_snapshot: std::sync::Arc<ServingSnapshot>,
    pub active_wave: Option<UpdateWaveId>,
}

pub struct WorkspaceIdentity {
    pub workspace_id: WorkspaceId,
    pub root: PlatformPath,
    pub kind: WorkspaceKind,
    pub repository_id: Option<RepositoryId>,
    pub worktree_id: Option<WorktreeId>,
    pub authorization_fingerprint: [u8; 32],
}
```

Exactly one coordinator task mutates one workspace state. A common-repository actor shares only immutable/read caches and topology data; it never owns source generations or snapshot pointers.

## 27. Watcher and Git-state roles

`notify-debouncer-full` SHALL be treated as:

```text
live invalidation signal
+ event normalization
+ rename assistance
```

It SHALL NOT be treated as:

```text
durable journal
authoritative file state
filesystem transaction stream
Git status engine
graph mutation engine
```

gix SHALL be treated as:

```text
repository/worktree topology authority
Git-relative path and inclusion semantics
HEAD/index/operation-state interpreter
status/tree-diff candidate accelerator
```

It SHALL NOT be treated as:

```text
current source-byte authority
repository mutation engine
checkout/switch/reset orchestrator
network/fetch service
semantic fact provider
```

The watcher handler SHALL perform only:

- event classification;
- event sequencing;
- cheap path normalization;
- bounded enqueue or reconcile escalation.

It SHALL NOT parse, compile, run status, traverse trees, hash large files, or mutate the CPG.

Selected Git metadata paths SHALL be watched separately and normalized into Git-state categories. Current Git state SHALL always be reread through the gix adapter after such an event.

## 28. Debounce policy

Recommended starting profile for interactive local indexing:

```text
debounce timeout: 50–100 ms
tick rate:        10–25 ms, and never greater than timeout
gather window:    10–25 ms after handler delivery
```

The filesystem debounce and downstream gather window are independent.

Continuous writes may yield periodic eligible batches; the design SHALL therefore rely on generation supersession rather than assuming one trailing-edge event.

---

## 29. Application event facade

```rust
pub enum WatchChange {
    DirtyPath {
        path: PlatformPath,
        seq: u64,
    },
    RemovedPath {
        path: PlatformPath,
        seq: u64,
    },
    Renamed {
        from: PlatformPath,
        to: PlatformPath,
        seq: u64,
    },
    GitMetadata {
        change: GitStateChange,
        seq: u64,
    },
    ReconcileRequired {
        root: WorkspaceRootId,
        seq: u64,
        reason: ReconcileReason,
    },
    WatcherError {
        seq: u64,
        class: WatcherFailureClass,
    },
}

pub enum GitStateChange {
    HeadChanged,
    IndexChanged,
    RepositoryOperationStateChanged,
    InclusionPolicyChanged,
    AttributesChanged,
    SubmoduleTopologyChanged,
    WorktreeTopologyChanged,
    RepositoryConfigChanged,
    UnknownGitMetadataChanged,
}
```

Fine-grained `notify::EventKind` and raw Git metadata file events SHALL not leak into downstream graph logic.

## 30. Bounded ingress and overflow recovery

Recommended implementation:

```text
watch handler
  → try_send into bounded Tokio channel
      success → coordinator receives event
      full    → set reconcile_required flag; increment metric
```

The event callback must not block behind parsing, gix status/tree work, or storage.

When the queue is full:

- individual event details may be discarded;
- the worktree enters `RECONCILE_REQUESTED`;
- `RECONCILE_REQUESTED` is a coordinator-owned boolean/reason flag, not a
  `WorkspaceLifecycle`, `EventStreamHealth`, or `UpdateWaveState` value;
- the next reconcile uses gix status/index candidate reduction when healthy;
- candidate uncertainty or gix failure broadens to Git-aware or generic full inventory.

The bounded queue is a latency and memory contract.

## 31. Dirty registry

The worktree coordinator owns a map:

```text
normalized platform path → latest DirtyEntry
```

Where available, each entry also carries the byte-safe Git repository-relative path.

Repeated events update one entry rather than enqueueing unlimited work.

Git metadata changes are coalesced by semantic category rather than by raw metadata file.

The map SHOULD be actor-owned. DashMap is optional for read-heavy auxiliary access but not required for the authoritative mutation path.

## 32. Bulk-mode thresholds

Enter bulk reconcile when any threshold is exceeded:

```text
dirty path count
dirty path ratio
event rate
directory subtree removal
watcher rescan signal
manifest/config invalidation
queue overflow
workspace-wide formatter/codegen signature
HEAD tree transition
index conflict burst
worktree/submodule topology change
inclusion-policy fingerprint change
```

When a HEAD-tree transition is known, prefer tree-diff + status candidate generation. When Git state is unavailable or unstable, broaden to authoritative inventory.

Thresholds are workload configuration, not ontology facts.

## 33. Source image capture

Each file analysis SHALL consume an immutable `SourceImage`.

```rust
pub struct SourceImage {
    pub worktree_id: WorktreeId,
    pub file_id: FileId,
    pub path: PlatformPath,
    pub git_repo_path: Option<GitRepoPath>,
    pub bytes: std::sync::Arc<[u8]>,
    pub digest: [u8; 32],
    pub size: u64,
    pub file_kind: SourceFileKind,
    pub read_generation: u64,
}
```

Capture algorithm:

1. read metadata;
2. open and read current filesystem bytes from the authorized descriptor;
3. compute the CodeFabric BLAKE3 digest, rewind that descriptor, and read/hash the bytes again;
4. capture metadata after each read;
5. verify identical content digests, stable size, file identity, mtime/change token, and source-generation fence;
6. if changed, retry or defer;
7. publish the source image only when stable.

Git blob OIDs may accelerate cache lookup but SHALL not replace this source-image contract.

## 34. Source inventory

Maintain one current operational inventory per `worktree_id`.

Required fields include:

```text
normalized platform path
byte-safe Git repository-relative path when applicable
file identity if available
current CodeFabric content digest
size
source file kind/mode
language classification
tracked/untracked/ignored/conflicted classification
inclusion state
Git blob OID when safely available
current file owner
```

### 34.1 Git worktree inventory

For a healthy Git worktree:

- use gix pathspec, exclude, attribute, and directory-walk semantics;
- include tracked source files;
- include untracked non-ignored source files;
- exclude ignored untracked files unless CodeFabric policy explicitly overrides;
- never traverse `.git` as source;
- classify submodules and nested repositories as separate topology domains.

### 34.2 Non-Git or degraded fallback

Use the bounded generic CodeFabric inventory walker.

### 34.3 Inventory digest

A Merkle-style directory/root digest SHALL be maintained so one file update changes only the affected path and ancestor hashes. File leaves use BLAKE3 over the typed `codefabric.inventory.file.v1` frame containing the length-prefixed raw workspace path, content digest (or the all-zero unsupported marker), byte length, file-kind code, classification code, and inclusion-state code. Directory nodes use the typed `codefabric.inventory.directory.v1` frame over byte-name-sorted `(length-prefixed child name, child kind, child digest)` entries. The root node digest is `worktree_inventory_digest`.

The inventory digest is based on current worktree state, not merely HEAD or index state.

## 35. Rename and identity policy

A matched rename is an optimization, not proof.

Evidence hierarchy:

```text
1. current worktree path identity
2. stable filesystem file ID when available
3. notify tracker/file-ID evidence
4. bounded gix rename/rewrite candidate for bulk transitions
5. identical current CodeFabric digest
6. source structural and semantic identity
```

Processing:

1. resolve the final path state;
2. verify current content digest and file kind;
3. evaluate whether language/module identity changed;
4. preserve file-instance identity when justified;
5. invalidate semantic identities if qualified path changed;
6. otherwise treat as remove+add.

Filesystem file IDs, watcher tracker IDs, gix object IDs, and gix rename similarity SHALL NOT be canonical CPG IDs.

## 36. Rescan generation fence

Rescan algorithm:

```text
record watcher event watermark W0
capture GitStateVector G0 when available
mark source trust UNVERIFIED

generate candidates:
  gix status/index when healthy
  Git-aware full inventory when required
  generic full inventory on gix failure/non-Git root

record watcher watermark W1
capture GitStateVector G1

if material G0/G1 change invalidates the scan:
  retry or broaden reconcile

compute current-byte delta
process events with seq > W0
reverify paths touched during inventory
apply corrective wave
set source_trust_state CURRENT
```

Events during scan are not discarded; they become `dirty_after_reconcile`.

Strict-current queries SHALL not claim source completeness while trust is `UNVERIFIED`.

# Part V — Git-Aware Repository and Worktree State

This part defines the complete gix integration boundary. It is normative for Git worktrees and optional only for non-Git source roots.

The governing pipeline is:

```text
notify-debouncer-full
    ↓
dirty path / selected Git metadata hints
    ↓
gix read-only state adapter
    repository/worktree topology
    Git-native inclusion
    status/index candidate reduction
    HEAD-tree bulk diff
    operation state
    ↓
authoritative current filesystem reads
    ↓
CodeFabric BLAKE3 source digests
    ↓
ordinary owner/fact-family invalidation and publication
```

gix improves discovery, topology, inclusion fidelity, and work avoidance. It never supersedes current-byte verification.

## 37. `git_state` module boundary

The single stable CodeFabric package SHOULD add an application-owned module boundary:

```text
git_state
  repository discovery and trust
  repository/common/worktree layout
  worktree identity
  Git-relative path representation
  index snapshot
  HEAD/ref/tree snapshot
  repository operation state
  status and candidate-delta calculation
  Git-native inclusion and attributes
  submodule/worktree topology
  gix cache and interruption management
  application-owned DTOs
```

The rest of CodeFabric SHALL consume application-owned records, not gix types.

### 37.1 Responsibility boundary

```text
git_state
  says:
    which Git worktree this is
    what Git considers tracked/untracked/ignored/conflicted
    what HEAD and index currently identify
    which tracked paths are candidates after a baseline transition
    which Git operation is in progress
    how repository-relative paths should be interpreted

codefabric-source
  says:
    what bytes actually exist in the current filesystem
    whether the read was stable
    what current content digest those bytes have

codefabric-coordinator
  decides:
    which source owners and fact families must be invalidated
    what update wave and publication may become current
```

### 37.2 gix is not a new semantic provider

gix information SHALL remain operational lifecycle metadata except for source-declared Git files already parsed as ordinary source/project files.

The CPG fact ontology remains history-free.

---

## 38. Read-only repository policy

The lifecycle daemon SHALL open repositories read-only by application policy.

Prohibited operations include:

```text
write object
write ref
write index
checkout
reset
switch
fetch
clone
push
merge publication
clean/smudge external command execution
credential helper execution
hook execution
```

gix mutation APIs may exist in the dependency but SHALL not be exposed through CodeFabric lifecycle interfaces.

A private module split enforces this inside the single package:

```text
git_state::read
  contains gix dependency and read-only adapter

git_state::admin
  absent from default product
```

---

## 39. Recommended gix feature profile

A default read-oriented lifecycle profile SHOULD begin with:

```toml
[dependencies]
gix = {
  version = "=0.86.0",
  default-features = false,
  features = [
    "sha1",
    "index",
    "status",
    "attributes",
    "excludes",
    "dirwalk",
    "blob-diff",
    "interrupt",
    "parallel",
    "auto-chain-error",
    "tracing"
  ]
}
```

### 39.1 Feature notes

- `sha1` is required for ordinary repositories.
- `sha256` MAY be added for repository compatibility testing, but SHALL NOT be treated as proof of complete Git SHA-256/reftable support.
- `index` supports index and conflict-stage inspection.
- `status` supports worktree/index/tree state comparisons.
- `attributes`, `excludes`, and `dirwalk` support Git-native worktree inclusion.
- `blob-diff` supports tree/blob diff and rewrite detection.
- `interrupt` supports cancellation of long operations.
- `parallel` is optional and SHALL be coordinated with CodeFabric's process-wide thread budget.
- `tracing` integrates lifecycle observability.

The default broad gix feature bundle SHOULD be avoided because it enables capabilities unnecessary to a read-only daemon, including mutation and credential-oriented surfaces.

The exact transitive relationship between `status`, `dirwalk`, `attributes`, and `excludes` SHALL be confirmed against the resolved 0.86 feature graph before finalizing the manifest.

---

## 40. Source-instance and Git topology model

The lifecycle distinguishes the externally authorized source instance from optional Git parents.

```text
WorkspaceInstance
  workspace_id                      canonical present-state scope
  authorized root
  watcher and source inventory
  analysis-context set
  update waves and provider state
  hot overlay and active ServingSnapshot

CommonRepository optional
  repository_id
  immutable/shared object database and refs
  linked-worktree topology and safe read caches

GitWorktree optional
  worktree_id
  per-worktree work directory, git dir, HEAD, index, operation state
```

```rust
pub struct GitRepositoryIdentity {
    pub repository_id: RepositoryId,
    pub common_dir_key: PathIdentity,
    pub object_format: GitObjectFormat,
}

pub struct GitWorktreeIdentity {
    pub worktree_id: WorktreeId,
    pub repository_id: RepositoryId,
    pub work_dir: Option<PlatformPath>,
    pub git_dir: PlatformPath,
    pub common_dir: PlatformPath,
    pub is_main_worktree: bool,
    pub is_bare: bool,
}
```

### 40.1 Scoping rule

Every present-state CPG, source generation, overlay, capability set, durable publication pointer, and active snapshot is scoped to `workspace_id`. For a Git-backed workspace, the workspace maps one-to-one to one authorized `worktree_id`. For a non-Git root, both Git IDs are null.

Two linked worktrees therefore always have different workspace IDs and independent current pointers, even when they share `repository_id` and immutable object caches.

## 41. Startup discovery

The daemon SHOULD receive an explicit workspace root and use exact-path `gix::open` when the expected repository/worktree is known.

`gix::discover` MAY be used only when discovery semantics are intended and the start path is explicit.

Startup SHALL resolve through gix:

```text
repository kind
bare/non-bare
work directory
per-worktree Git directory
common directory
worktree list
HEAD state
repository operation state
object format
```

CodeFabric SHALL NOT construct:

```text
<workspace>/.git
.git/worktrees/<name>
```

by string concatenation.

---

## 42. Bare repositories

A bare repository has no current filesystem worktree and therefore cannot directly supply the present-state source model assumed by this lifecycle specification.

Policy:

```text
bare repository + no materialized source root
  → repository recognized
  → source capability unavailable
  → no normal watcher/CPG generation

bare repository + explicit virtual-tree mode
  → future separate product profile
  → not part of this present-state worktree lifecycle
```

gix's ability to read trees/blobs does not silently convert the current product into a revision-indexing service.

---

## 43. Canonical path representations

Git-relative and operating-system paths are distinct, and display strings are never identity.

```rust
pub struct GitRepoPath {
    pub raw: std::sync::Arc<[u8]>,
    pub display: String,
    pub display_is_lossy: bool,
}

pub struct PlatformPath {
    pub native: std::ffi::OsString,
    pub workspace_relative_bytes: std::sync::Arc<[u8]>,
    pub comparison_key: std::sync::Arc<[u8]>,
    pub encoding_code: PathEncodingCode,
    pub display: String,
    pub display_is_lossy: bool,
}
```

Invariants:

1. workspace-relative bytes establish stored path identity;
2. filesystem I/O uses native platform paths;
3. Git adapters preserve Git path bytes;
4. comparison keys follow repository/platform case semantics;
5. display strings are escaped/lossy presentation only;
6. every conversion remains within the authorized workspace root and symlink policy.

The data-fabric source schema uses `path_bytes`, `path_display`, `path_encoding_code`, `path_case_key`, and `path_display_is_lossy` plus non-null `workspace_id`.

---

## 44. Path normalization and authorization

Normalize in two explicit steps:

```text
platform-native path
  → authorized workspace-relative platform path
  → Git repository-relative byte path when Git-backed
```

Git-facing normalization SHOULD use gix path/pathspec APIs. Path normalization SHALL not widen authorization. Symlink resolution, `..`, nested mounts, case-only renames, non-UTF-8 paths, separators, exclusions, and root-relative behavior are tested per platform.

Semantic query source-boundary phrases compile only to filters over the pre-authorized inventory produced by these rules.

## 45. File identity hierarchy

Recommended evidence hierarchy:

```text
1. current worktree path identity
2. stable filesystem file ID when available
3. notify rename tracker/file-ID evidence
4. Git rename candidate from tree/status diff
5. identical current content digest
6. prior semantic/source structural identity
```

No single signal proves semantic identity.

Git blob OID SHALL NOT replace CodeFabric's present-state source digest because worktree bytes may differ from raw object bytes due to:

- line-ending conversion;
- filters;
- encodings;
- symlink materialization;
- attribute-driven behavior.

---

## 46. Inclusion policy

The source inventory SHALL distinguish:

```text
tracked
untracked_not_ignored
untracked_ignored
tracked_but_ignored_pattern_matches
excluded_by_codefabric_policy
submodule_gitlink
nested_repository
special_file
```

Recommended default inclusion:

| File class | Default |
|---|---|
| Tracked source file | Include |
| Tracked file matching a later ignore pattern | Include; tracked state wins |
| Untracked source file not ignored | Include |
| Untracked ignored file | Exclude |
| Explicitly CodeFabric-included ignored source | Optional policy override |
| `.git` internal file | Exclude from source CPG; watch selected metadata separately |
| Build/cache output | Exclude through Git and CodeFabric policy |
| Submodule contents | Separate workspace policy |
| Symlink | Apply source/symlink policy; never blindly follow outside root |
| Device/socket/special file | Exclude |

Git ignore rules SHALL NOT be treated as authorization.

CodeFabric's explicit security and root-boundary policy always has higher authority.

---

## 47. Git-aware directory walking

Cold-start and authoritative-rescan inventory SHOULD use gix worktree dirwalk/exclude/attribute stacks when the workspace is a Git worktree.

Benefits:

- nested `.gitignore` precedence;
- repository excludes;
- worktree-specific excludes;
- Git-compatible path semantics;
- early pruning of ignored build/cache directories;
- consistent untracked-file classification;
- reduced hashing and parsing.

Fallback:

```text
not a Git worktree
  → existing generic CodeFabric inventory walker
```

### 47.1 Boundaries

The Git-aware walker SHALL remain bounded by:

```text
maximum file count
maximum directory depth
maximum total bytes considered
maximum time
cancellation signal
symlink policy
```

---

## 48. Attribute policy

gix attributes SHOULD be used for:

- detecting paths whose worktree bytes may differ from raw blobs;
- identifying binary/text classification hints;
- line-ending/filter awareness;
- optional generated or language-policy hints only when explicitly configured.

Attributes SHALL NOT automatically execute filters.

Changes to:

```text
.gitattributes
repository attribute sources
trusted attribute configuration
```

trigger an `INCLUSION_OR_CONTENT_POLICY` reconcile.

---

## 49. Ignore-policy fingerprint

The Git-state adapter SHOULD compute an operational fingerprint over the active inclusion policy inputs, including as applicable:

```text
.gitignore files
per-worktree excludes
common info/exclude
trusted global exclude configuration
CodeFabric include/exclude configuration
pathspec configuration
```

When the fingerprint changes:

1. inventory affected subtree or worktree;
2. compute added/removed inclusion candidates;
3. update watchers if watch-root pruning is used;
4. invalidate source owners newly included or excluded;
5. do not require semantic regeneration for still-included unchanged files.

---

## 50. Operational Git state vector

Each `ServingSnapshot` and update wave SHOULD carry an operational Git baseline.

```rust
pub struct GitStateVector {
    pub repository_id: RepositoryId,
    pub worktree_id: WorktreeId,

    pub head_kind: HeadKind,
    pub head_target: Option<GitObjectId>,
    pub head_tree: Option<GitObjectId>,

    pub index_fingerprint: Option<[u8; 32]>,
    pub index_entry_count: Option<u64>,
    pub has_conflict_stages: bool,

    pub repository_state: GitOperationState,

    pub inclusion_policy_fingerprint: [u8; 32],
    pub attributes_fingerprint: [u8; 32],

    pub worktree_inventory_digest: [u8; 32],
}
```

The `inclusion_policy_fingerprint`, `attributes_fingerprint`, and
`worktree_inventory_digest` are authoritative observations owned by the policy compiler and
the descriptor-relative inventory boundary. They SHALL be supplied to Git state capture as
one immutable `GitStateObservations` value. The Git adapter SHALL bind them into the returned
vector but SHALL NOT recreate them by independently walking source files or accepting ambient
configuration. This keeps gix responsible for Git-native classification while current stable
filesystem bytes remain source truth.

### 50.1 Object ID representation

```rust
pub struct GitObjectId {
    pub algorithm: GitHashAlgorithm,
    pub bytes: Vec<u8>,
}
```

The design SHALL not assume a fixed SHA-1 text width at internal boundaries.

---

## 51. Git operation state

The coordinator SHOULD expose a normalized state enum derived from `Repository::state` and other exact gix surfaces:

```text
CLEAN
MERGE
REBASE
CHERRY_PICK
REVERT
BISECT
APPLY
OTHER_OPERATION
UNKNOWN
```

Only states actually exposed and verified in gix 0.86 SHALL be mapped explicitly. Unknown/new states map to `OTHER_OPERATION` or `UNKNOWN`.

### 51.1 Lifecycle effect

During a Git operation:

- watcher processing continues;
- current source/syntax facts may publish;
- source completeness remains possible;
- update waves are grouped more aggressively;
- durable Delta flush MAY be delayed briefly for quiescence;
- strict-current target queries still receive current worktree facts;
- operation state is returned as operational metadata.

The daemon SHALL not wait indefinitely for a Git operation to end.

---

## 52. Metadata watch set

In addition to recursively watching the worktree source root, CodeFabric SHOULD watch selected Git metadata locations resolved through gix.

Potential metadata classes:

```text
per-worktree HEAD
per-worktree index
per-worktree operation-state paths
common config
worktree config
current symbolic-ref target where practical
packed-refs or ref storage indicators where practical
info/exclude
attribute-policy sources
.gitmodules
worktree registration topology
```

Exact paths SHALL be resolved through repository/worktree APIs or a version-pinned adapter.

The daemon SHALL NOT watch:

```text
the entire object database
the entire pack directory
all refs recursively without need
arbitrary credential/config locations
```

Metadata events are coalesced into semantic categories rather than exposed raw.

---

## 53. Git metadata event facade

Extend `WatchChange` or add a companion event:

```rust
pub enum GitStateChange {
    HeadChanged,
    IndexChanged,
    RepositoryOperationStateChanged,
    InclusionPolicyChanged,
    AttributesChanged,
    SubmoduleTopologyChanged,
    WorktreeTopologyChanged,
    RepositoryConfigChanged,
    UnknownGitMetadataChanged,
}
```

Each category causes current gix state to be reopened/re-read; raw metadata event order is not authoritative.

---

## 54. Status as an accelerator

gix status SHOULD be used to identify candidate worktree changes during:

- warm startup;
- authoritative rescan;
- queue overflow recovery;
- bulk event storms;
- metadata-triggered baseline changes;
- periodic audit.

It SHALL NOT be the final proof that CPG source bytes are current.

Final authority remains:

```text
stable filesystem read
+ CodeFabric content digest
```

### 54.1 Candidate classes

The adapter SHOULD normalize status into:

```text
TRACKED_UNCHANGED
TRACKED_MODIFIED
TRACKED_DELETED
TRACKED_TYPE_CHANGED
UNTRACKED_INCLUDED
UNTRACKED_IGNORED
INDEX_ADDED
INDEX_MODIFIED
INDEX_DELETED
CONFLICTED
SUBMODULE_CHANGED
UNKNOWN
```

The exact mapping depends on gix's released status representation.

---

## 55. Status-based reconcile algorithm

```text
1. capture GitStateVector G0
2. run bounded gix status with selected untracked policy
3. capture GitStateVector G1
4. if relevant HEAD/index/state changed during scan:
       retry or broaden reconcile
5. normalize status entries to candidate paths
6. add paths dirty from watcher watermark
7. for each candidate:
       capture stable SourceImage
       compare CodeFabric digest
8. reconcile inventory for deletions/untracked inclusion
9. publish source delta only after generation fencing
```

This avoids hashing every unchanged tracked file on every reconcile.

### 55.1 Periodic and post-bulk audit

Because status may rely on filesystem stat optimizations and because CodeFabric's CPG
currentness has stricter semantics, the coordinator SHALL run this audit after every
bulk transition and after a configurable idle/quiescence interval. It compares:

- source inventory count;
- selected file digests;
- Git-state vector;
- optional full Merkle root.

Any divergence triggers an authoritative inventory rescan, marks Git acceleration
`GIT_DEGRADED`, and prevents strict-current completion until generic current-byte
inventory and the Git-state vector reconverge. A passing audit restores `GIT_READY`.

---

## 56. Index integration

The index SHOULD be read for:

- tracked path inventory;
- staged versus unstaged classification;
- conflict stages;
- entry modes;
- current index fingerprint;
- detecting index-only changes that need not alter current worktree CPG facts;
- differentiating a Git operation from arbitrary filesystem deletion.

### 56.1 Index-only changes

A staging operation may change the index without changing current filesystem bytes.

Default CPG behavior:

```text
worktree bytes unchanged
  → no source or semantic CPG regeneration
  → update operational GitStateVector only
```

The present-state CPG represents code in the current worktree, not the staging area.

### 56.2 Conflicts

When the index contains non-zero conflict stages:

- mark the affected path `GIT_CONFLICTED`;
- current filesystem bytes remain the source fact authority;
- semantic analysis proceeds if source is parsable;
- status response exposes conflict metadata;
- no assumption is made that stage 0 fully represents the file.

### 56.3 External index writers

After Git CLI or another tool changes the index:

- discard cached `Reference`/index snapshots as required;
- reopen or reload;
- never retain stale attached gix objects across the mutation boundary.

---

## 57. Sparse or special index behavior

The attached gix reference establishes that the index contains extensions and is not a simple map.

CodeFabric SHALL therefore:

- avoid reimplementing index semantics;
- preserve gix's interpretation of entries and stages;
- verify exact sparse-index/skip-worktree support before relying on it;
- fall back to authoritative inventory when support is incomplete;
- add Git CLI parity fixtures for sparse checkout before enabling sparse-specific optimization.

---

## 58. HEAD and tracked baseline

The Git-state adapter SHOULD resolve at wave boundaries:

```text
HEAD target
HEAD commit where applicable
HEAD tree
```

Unborn HEAD and non-commit HEAD targets SHALL be supported as explicit states.

HEAD identity is operational baseline metadata, not the CPG source snapshot identity.

---

## 59. Branch switch / checkout acceleration

When the daemon observes a change from `old_head_tree` to `new_head_tree`, it SHOULD use gix tree-to-tree diff to produce a tracked candidate delta.

Pipeline:

```text
old HEAD tree
   │
   ├─ gix tree diff ── added / deleted / modified / mode-changed candidates
   │
   └─ optional bounded rewrite detection ── rename/copy candidates

current worktree status
   ├─ local tracked modifications
   ├─ untracked files
   └─ conflicts/submodule state

union of candidate paths
   ↓
authoritative current reads and CodeFabric digest comparison
   ↓
one bulk update wave
```

### 59.1 Correctness rule

Tree diff does not replace worktree status or current-byte verification because:

- local modifications may survive checkout;
- untracked files are absent from commit trees;
- conflict state may be present;
- filters and line endings affect worktree bytes;
- watcher events may describe additional changes.

---

## 60. Rename detection policy

gix rewrite/rename detection MAY improve semantic identity preservation during:

- branch switch;
- large patch application;
- repository-wide move;
- reconcile after watcher loss.

It SHOULD NOT run for every small save.

Policy:

```text
if candidate_count <= rename_detection_limit
and bulk_mode
and identity preservation benefit is material:
    run bounded rename detection
else:
    treat as add/delete candidates
```

Even after gix proposes a rename, CodeFabric SHALL verify:

- final current content digest;
- language classification;
- module/package identity;
- path-dependent semantics;
- source structural matching.

Rename similarity is evidence, not canonical semantic identity.

---

## 61. Mode-only and symlink changes

gix status/tree diff can identify changes not captured by content bytes alone, including:

- executable-bit changes;
- symlink/file type changes;
- Git mode changes.

Policy:

- executable-bit-only changes usually do not alter Python/Rust semantic facts but do alter source-file metadata;
- symlink target/type changes trigger full source-policy verification;
- file-to-directory and directory-to-file transitions trigger subtree reconcile;
- CodeFabric SHOULD expose source-file kind accurately.

---

## 62. Bulk operation stabilization tuple

For branch switches and Git operations, CodeFabric SHOULD evaluate a stabilization tuple:

```text
HEAD target
HEAD tree
index fingerprint
repository operation state
worktree event watermark
dirty-path set
```

A bulk wave is eligible for full semantic/durable publication when:

- the tuple remained stable over the configured gather window;
- source reads were generation-consistent;
- all required candidate paths were reconciled.

The tuple additionally includes the inclusion-policy and attributes fingerprints from
the captured `GitStateVector`; either changing invalidates the candidate baseline.

The fast syntax lane MAY publish earlier with pending semantic capability metadata.

---

## 63. Blob OID as auxiliary cache key

For a clean tracked regular file, the index/tree blob OID can be used as an auxiliary immutable identity.

Potential cache:

```text
(blob_oid, worktree_transform_fingerprint, provider_bundle)
    → source digest
    → parsed syntax batch
    → normalized semantic owner batch where path-independent
```

### 63.1 Required safeguards

A blob OID is reusable for current worktree source only when CodeFabric proves that:

- file is clean relative to the relevant index/tree;
- no external clean/smudge transformation changes analyzed bytes;
- line-ending/encoding policy is represented;
- symlink/file type matches;
- path-sensitive semantic identity is accounted for;
- provider context matches.

Otherwise, read and hash current bytes.

---

## 64. Content-addressed cache hierarchy

Recommended hierarchy:

```text
Level 1:
  CodeFabric BLAKE3 worktree-byte digest
  authoritative for parser and source facts

Level 2:
  Git blob OID + transform fingerprint
  accelerator for clean tracked files

Level 3:
  owner semantic fingerprint
  suppresses normalized fact rewrites

Level 4:
  derived projection fingerprint
  suppresses CFG/summary recomputation
```

This can substantially accelerate branch switching between commits that reuse many unchanged blobs.

---

## 65. gix cache configuration

Object and pack caches SHOULD be enabled only for operations that decode repository objects:

- HEAD-tree traversal;
- tree diff;
- blob retrieval;
- submodule metadata;
- optional virtual snapshot operations.

Cache limits SHALL be integrated into CodeFabric's global memory budget.

During tuning:

- use gix cache-efficiency diagnostics;
- measure cold and warm paths;
- remove diagnostic features after sizing;
- avoid duplicating large cache layers in gix and CodeFabric.

Commit-graph/history caches are not justified by the present-state lifecycle unless an ancestry operation is introduced for a separate non-CPG control function.

---

## 66. Submodule and external-workspace policy

A submodule is a distinct workspace and lifecycle domain. The parent stores only operational topology:

```text
submodule path bytes/display
gitlink object ID
trusted/redacted configured URL when operationally needed
initialized/present state
child workspace_id when separately authorized/opened
```

Child source bodies and facts belong only to the child `ServingSnapshot`.

Version 1.3 query behavior is intentionally single-workspace:

- a parent query MAY return a submodule/external declaration as an endpoint-only external entity;
- it SHALL NOT traverse the child's body or join child facts into the parent snapshot;
- a request for cross-workspace body traversal fails with `COMPOSITE_SNAPSHOT_UNSUPPORTED`;
- the caller may issue a separate authorized query to the child workspace.

Changes to `.gitmodules`, gitlinks, child presence, or child HEAD trigger topology reconciliation. CodeFabric SHALL not auto-fetch or auto-initialize submodules. Recursion depth and workspace count are bounded.

## 67. Nested repositories

A nested Git repository that is not a configured submodule SHALL be classified according to workspace policy:

```text
exclude
separate workspace
explicitly include as ordinary files without traversing nested .git
```

It SHALL never be followed recursively without an explicit policy.

---

## 68. Linked-worktree scheduling

One common repository may have many active workspaces, one per authorized worktree.

```text
CommonRepoActor
  shared gix common-repository handle strategy
  immutable ODB/object caches
  linked-worktree topology registry

WorkspaceCoordinator A
  workspace_id A / worktree_id A
  watcher, inventory, contexts, waves, snapshots

WorkspaceCoordinator B
  workspace_id B / worktree_id B
  watcher, inventory, contexts, waves, snapshots
```

A common-directory metadata change may enqueue reconciliation for multiple coordinators. Worktree HEAD/index/source changes remain local. Query admission, idempotency, durable pointers, and artifacts are always keyed by workspace ID.

## 69. Repository handle model

The daemon SHALL NOT use:

```rust
Arc<gix::Repository>
```

as a globally shared concurrent handle.

Recommended options:

1. hold a `ThreadSafeRepository` in the common repository service and create thread-local handles for jobs;
2. reopen exact repository/worktree paths per bounded blocking job;
3. use one repository actor and return detached application DTOs;
4. detach object data before crossing thread/lifetime boundaries.

The final choice SHALL be verified against gix 0.86 ownership semantics and benchmarked.

---

## 70. Execution placement

gix work is primarily blocking filesystem/CPU work.

It SHALL not run directly on latency-sensitive Tokio workers.

Recommended placement:

```text
Tokio coordinator
    ↓ bounded Git-work semaphore
blocking Git adapter job
    ↓
gix status/tree/index/ODB work
    ↓ detached GitCandidateDelta DTO
Tokio coordinator
```

Options include:

- dedicated bounded Git worker pool;
- Rayon pool if operations do not perform inappropriate blocking waits;
- `spawn_blocking` with an explicit semaphore for coarse blocking Git calls.

Because Rayon is optimized for CPU work and gix may perform filesystem I/O, a dedicated blocking pool is generally clearer for status/dirwalk/ODB operations. CPU-heavy post-processing can then move to Rayon.

---

## 71. Parallelism policy

gix internal `parallel` behavior and CodeFabric outer parallelism compete for the same cores.

Policy:

1. bound outer concurrent Git jobs first;
2. benchmark gix parallel on representative repositories;
3. avoid running many internally parallel status/diff jobs simultaneously;
4. reserve CPU for query serving, parsing, rustc, and DataFusion;
5. expose gix worker/caching metrics.

Potential modes:

```text
interactive:
  one bounded Git job
  internal parallel enabled if beneficial

multi-worktree bulk:
  few outer Git jobs
  reduced or disabled internal parallel

single huge monorepo reconcile:
  one outer job
  internal parallel enabled
```

---

## 72. Cancellation

Each long-running gix operation SHOULD receive an interruption signal connected to:

- update-wave supersession;
- workspace shutdown;
- strict query deadline;
- bulk-reconcile cancellation;
- global resource pressure.

A cancelled gix operation returns no authoritative candidate set. The coordinator either:

- retries against current state; or
- escalates to a later reconcile.

---

## 73. Staleness of attached values

References, index handles, and attached objects are snapshots and may become stale after external Git operations.

Rules:

- do not retain them across update waves;
- retain immutable OIDs and application DTOs instead;
- reopen/reload after metadata changes;
- include the captured `GitStateVector` with every candidate delta;
- reject candidate deltas whose baseline no longer matches.

---

## 74. gix lock/tempfile integration

gix lock/tempfile facilities SHOULD be considered for CodeFabric-owned local operational files, such as:

- daemon singleton lease;
- local endpoint descriptor;
- optional hot-overlay IPC journal;
- source inventory checkpoint;
- workspace registration file;
- shutdown/recovery marker.

### 74.1 Rules

- lock/temp destination must reside on a filesystem supporting the required atomic rename semantics;
- lock contention is explicit;
- crash tests cover every publication point;
- gix locks do not provide multi-table Delta atomicity;
- Git repository locks SHALL not be acquired merely for reading the worktree;
- CodeFabric SHALL not block Git CLI operations with unnecessary repository locks.

---

## 75. Singleton daemon lease

For one daemon state root (the AC-G-62 repository/worktree group), regardless
of how many authorized workspaces it contains:

```text
acquire CodeFabric-owned lock
write endpoint metadata to tempfile
fsync at configured durability level
atomic rename
serve
remove/retire on joined shutdown
```

The lock path SHOULD be outside mutable repository source when possible.

---

## 76. Repository trust policy

The gix adapter SHALL define:

```text
allowed config sources
whether environment overrides are accepted
whether global/system Git config is consulted
which attributes/excludes are trusted
whether external commands are permitted
whether network is permitted
```

Recommended default for local CodeFabric indexing:

```text
network                 disabled
credential helpers      disabled
external filters        disabled
hooks                    disabled
repository mutation     disabled
checkout                disabled
trusted path roots       explicit
resource limits          enabled
allowed config sources   CodeFabric-owned + repository-local only
environment overrides    rejected
global/system Git config not consulted
```

Repository-local attributes and excludes may influence inventory
classification but never authorization. Their external filter/command content
is not executed. These defaults are part of `local-workstation-v1`; weakening
one requires a separately fingerprinted trust profile.

---

## 77. Clean/smudge filters

CodeFabric analyzes current filesystem bytes. It therefore does not need to execute clean/smudge filters for ordinary worktree CPG generation.

gix attributes MAY reveal that filters exist, which is useful for determining that blob OID and worktree bytes are not interchangeable.

If a future virtual-tree mode materializes worktree-equivalent bytes from Git objects:

- filter execution is separately opt-in;
- external commands run in a sandbox;
- nondeterministic output disables semantic cache reuse;
- time/output limits apply;
- untrusted repository filter config is rejected.

---

## 78. Symlink and Windows safety

The daemon remains read-only and therefore should not invoke checkout. Nonetheless:

- pin `gix =0.86.0` exactly (the security floor is also the accepted release pin);
- test symlink and Windows reparse-point behavior;
- never follow source symlinks outside authorized roots;
- do not expose arbitrary checkout destinations;
- retain the gix security floor in upgrade gates.

---

## 79. Resource governance

gix operations SHALL be bounded by:

```text
file count
directory depth
blob size
object count
decoded object bytes
rename candidate count
status runtime
diff runtime
cache bytes
concurrent Git jobs
submodule recursion
```

Corrupt repositories and object databases are treated as untrusted inputs.

---

## 80. Generic fallback invariant

For any gix read-side failure that does not make the filesystem inaccessible:

```text
fall back to:
  CodeFabric root policy
  bounded generic filesystem inventory
  stable current-byte reads
  CodeFabric digests
```

The system may be slower, but it remains correct.

---

## 81. No history ontology

Do not add:

```text
commit nodes
blame facts
historical code states
churn
lineage
revision walks
```

to the present-state CPG as part of this integration.

HEAD/tree/OIDs are operational update baselines only.

---

## 82. No Git mutation orchestration

Do not use gix to implement:

```text
switch
restore
reset
merge workflow
rebase
cherry-pick
stash
push
```

inside the CPG daemon.

The attached reference explicitly distinguishes available plumbing from incomplete porcelain orchestration.

---

## 83. No checkout in the indexing daemon

The daemon analyzes the worktree supplied by external tools.

It does not materialize source trees.

This avoids:

- worktree mutation races;
- index/HEAD coupling;
- untracked-file loss risk;
- Windows checkout security exposure;
- feedback loops with the watcher.

---

## 84. No external filters by default

Do not execute repository-defined filters during indexing.

Analyze current bytes.

Use attributes only to understand cache equivalence and source policy.

---

## 85. No status on every event

gix status is valuable for reconciliation and bulk transitions, but too expensive and unnecessary for each isolated save.

Ordinary hot path:

```text
event → stable read → digest → local invalidation
```

---

## 86. No blob OID as sole source identity

Git blob OID identifies object content, not necessarily current analyzed worktree bytes.

BLAKE3 current-byte digest remains canonical.

---

## 87. No shared `Arc<Repository>`

Follow the gix handle model. Use thread-local handles, `ThreadSafeRepository`, an actor, or reopened repositories.

---

## 88. Phase 1 — Repository correctness

Implement first:

- gix 0.86 exact pin;
- repository/worktree discovery;
- common/worktree identity;
- byte-safe path DTO;
- HEAD/index/state vector;
- linked-worktree tests;
- no mutation/external-command policy.

This phase improves design correctness even before performance acceleration.

---

## 89. Phase 2 — Git-native inventory

Implement:

- excludes/attributes/dirwalk;
- inclusion-policy fingerprint;
- tracked/untracked/ignored classification;
- selected metadata watches;
- `.gitignore` and `.gitattributes` reconcile.

---

## 90. Phase 3 — Status/index acceleration

Implement:

- bounded status adapter;
- candidate delta DTO;
- index conflict stages;
- warm-start and rescan candidate pruning;
- generic fallback;
- Git CLI parity.

---

## 91. Phase 4 — Bulk HEAD-tree acceleration

Implement:

- HEAD-tree baseline;
- tree-to-tree candidate diff;
- bounded rename detection;
- branch-switch stabilization tuple;
- blob-OID cache mapping.

---

## 92. Phase 5 — Shared caches and advanced topology

Implement:

- common repository actor;
- linked-worktree shared immutable ODB cache;
- submodule workspace topology;
- cache-efficiency tuning;
- operational lock/tempfile integration.

---

# Part VI — Update Pipeline

## 93. Pipeline overview

```text
source and selected Git metadata events
    ↓
dirty registry / Git metadata registry
    ↓
update wave
    ↓
Git candidate strategy
    ├─ isolated path: no status scan
    ├─ reconcile: bounded gix status/index
    ├─ branch switch: HEAD-tree diff + status
    └─ gix unavailable: generic inventory
    ↓
stable current source images
    ↓
CodeFabric BLAKE3 digest comparison
    ↓
change classification
    ↓
invalidation plan
    ↓
fast syntax lane
    ↓
fast immutable snapshot
    ↓
semantic provider lane
    ↓
owner-local derived lane
    ↓
interprocedural propagation
    ↓
validated immutable hot snapshot
    ↓
asynchronous Delta publication
```

Every Git-derived candidate set SHALL carry the `GitStateVector` against which it was constructed. A stale vector invalidates the candidate result before source or graph publication.

## 94. Fast syntax lane

### 94.1 Purpose

Provide current source navigation and explicit syntax gaps as quickly as possible.

### 94.2 Work

- current source facts;
- Tree-sitter incremental parse;
- parse errors and missing syntax;
- token/comment/trivia extraction;
- likely owner boundaries;
- source-to-owner mapping;
- capability withdrawals for invalidated semantic facts.

### 94.3 Publication

The daemon MAY publish a syntax-current snapshot before semantic providers finish.

This snapshot SHALL:

- include current source/syntax;
- remove invalidated semantic facts from visibility;
- retain unaffected semantic owners;
- mark pending/unavailable capabilities;
- never present stale invalidated facts as current.

---

## 95. Python semantic lane

Pipeline:

```text
Ruff typed parse
  → local scopes/bindings/references
  → Pyrefly module refresh
  → type/member/call reconciliation
  → Python CFG/dataflow
  → owner-local derived facts
  → summary propagation
```

Pyrefly requests SHOULD be batched by module dependency neighborhood.

A sidecar response is eligible only if its source digests match the wave.

---

## 96. Rust semantic lane

Pipeline:

```text
Cargo metadata / target mapping
  → rustc incremental invocation
  → owned extractor records
  → complete invocation manifest
  → owner fingerprint comparison
  → changed-owner replacement
  → MIR-derived facts
  → interprocedural propagation
```

### 96.1 Compiler manifest rule

A rustc extraction run SHALL emit:

```text
CompilationBegin
OwnerBegin / owner observation chunks / OwnerEnd ...
CompilationEnd with source/build digest and owner manifest
```

No owner facts are publishable without a valid manifest policy.

### 96.2 Partial compilation policy

Default:

- compile failure does not produce a fresh compiler generation;
- invalidated owners remain semantically unavailable;
- unchanged/unaffected owners remain current only when dependency validity is established;
- last-known-good compiler rows may remain in hidden operational cache but SHALL not be visible as present-state facts for invalidated owners.

### 96.3 Query fallback on compile failure

When a query targets unavailable Rust semantics, response SHALL include:

```text
availability_state: UNAVAILABLE
completeness_state: UNAVAILABLE
freshness_state: CURRENT
owner_capability_state: UNAVAILABLE_COMPILE | UNAVAILABLE_PROVIDER | PENDING
reason_code: compile_error | extractor_failure | pending
current source location
current source context
compiler diagnostics
affected owner/build unit
```

The MCP layer SHOULD make the current source directly available in the same logical response.

---

## 97. Registered owner-local derived lane

The lifecycle scheduler invokes owner-local entries from the data-fabric `DerivationRegistry`; it does not choose algorithms independently.

Typical default entries are CFG SCCs, dominators/post-dominators, loops, reaching definitions, and liveness using custom Rust/petgraph over `GraphProjectionDto`. The job output is fenced by workspace, context, source generation, derivation version, and input fact fingerprint.

A superseded or incomplete derivation is discarded and its capability remains `PENDING`, `INVALIDATED`, or `UNAVAILABLE_DERIVATION` as appropriate.

---

## 98. Registered interprocedural derived lane

The scheduler invokes the single registered implementation for call/dependency reachability, points-to/alias, effect/resource propagation, and summaries. The registry may select DataFusion CSR operators or a custom Rust fixed-point engine, but only one implementation/profile can publish one fact family.

Interprocedural invalidation uses dependency/fingerprint stop conditions. Lifecycle owns admission, supersession, priority, and activation; the data fabric owns derivation semantics and output schemas.

## 99. Validation stages

### 99.1 Fast validation

Before syntax-current publication:

- source digest matches the immutable source image;
- worktree identity matches the active coordinator;
- Git-derived candidate state matches the wave's `GitStateVector` fence;
- Arrow schemas validate;
- source spans are in bounds;
- owner IDs are deterministic;
- capability withdrawals are complete;
- no stale generation result is included.

### 99.2 Owner semantic validation

- provider manifest is complete;
- fact primary keys are unique within owner;
- relation endpoints exist or point to explicit unknown;
- CFG entry/exit and edges are valid;
- def-use endpoints are valid;
- source correspondence uses the current digest;
- path/module identity uses the current worktree-relative path.

### 99.3 Affected-component validation

- cross-owner reference and call endpoints;
- SCC assignment consistency;
- summary dependency generation;
- derived fixed-point convergence;
- submodule and linked-worktree boundaries are not crossed without explicit external endpoints.

### 99.4 Git-state validation

For Git-accelerated waves:

- repository/common/worktree identity is unchanged;
- relevant HEAD/tree/index fingerprints still match;
- inclusion and attribute fingerprints still match;
- conflict-stage classification is current;
- status/tree-diff candidates were verified through current-byte reads;
- no gix-attached object or session-local handle was persisted.

### 99.5 Durable publication validation

- Delta row counts/checksums;
- cross-table endpoint checks;
- publication table completeness;
- schema fingerprints;
- pinned version integrity;
- worktree and Git-state control rows match the active serving snapshot.

Whole-worktree validation SHOULD run periodically and after bulk reconcile, not on every one-line edit.

# Part VII — Atomicity and Serving Snapshot Design

## 100. Atomicity model

The system SHALL define six distinct atomicity boundaries.

### 100.1 Source-image atomicity

One file is analyzed from one stable byte image.

### 100.2 Owner-batch atomicity

All facts owned by one owner/fact-family replacement become visible together.

### 100.3 Hot-snapshot atomicity

All table overlays, capability withdrawals, and derived facts in a hot generation become visible through one pointer swap.

### 100.4 Durable table atomicity

Each Delta table commit is individually atomic.

### 100.5 Durable multi-table atomicity

Publication manifest pins exact table versions; `current_publication` changes last.

### 100.6 Query atomicity

A query pins one immutable serving snapshot for its entire execution.

The filesystem itself does not provide multi-file transaction atomicity. CodeFabric approximates logical edit batches with debounce, gather windows, explicit barriers, and source-generation watermarks.

---

## 101. Why a hot overlay is required

Writing every tiny edit immediately into many Delta tables would:

- add durable-commit latency to interactive freshness;
- create small files;
- require excessive multi-table commits;
- conflict with the data-fabric micro-batching policy.

Therefore the interactive serving layer SHALL support:

```text
durable base publication
+ one immutable consolidated hot overlay
= current serving snapshot
```

Delta remains durable authority. The hot overlay is the current write-through serving state.

---

## 102. ServingSnapshot

```rust
pub struct ServingSnapshot {
    pub snapshot_id: SnapshotId,
    pub workspace_id: WorkspaceId,
    pub repository_id: Option<RepositoryId>,
    pub worktree_id: Option<WorktreeId>,
    pub source_generation: u64,
    pub source_inventory_digest: [u8; 32],
    pub source_trust: SourceTrustState,
    pub event_stream_health: EventStreamHealth,
    pub durable_base_publication: PublicationId,
    pub base_table_versions: SnapshotTableVersionMap,
    pub overlay_generation: u64,
    pub overlay_checksum: [u8; 32],
    pub overlay: std::sync::Arc<ConsolidatedOverlay>,
    pub analysis_context_set_id: AnalysisContextSetId,
    pub analysis_contexts: std::sync::Arc<[AnalysisContextSummary]>,
    pub capability_index: CapabilityIndex,
    pub diagnostics: DiagnosticIndex,
    pub ontology_version: ContractVersion,
    pub schema_bundle_version: ContractVersion,
    pub provider_bundle_version: ContractVersion,
    pub derivation_bundle_version: ContractVersion,
    pub query_language_version: ContractVersion,
    pub git_state: Option<GitStateVector>,
    pub git_acceleration_status: GitAccelerationStatus,
    pub activation_state: ServingActivationState,
}
```

The object is immutable and shared by `Arc`. `snapshot_id` is derived from the complete manifest, including overlay checksum and context set. A linked worktree or non-Git root has its own snapshot. The durable publication alone SHALL never be described as the active snapshot when an overlay exists.

## 103. Hot overlay contents

The overlay contains:

```text
replacement rows by table and owner
table-specific owner tombstones
fact-family capability withdrawals
new diagnostics
new source inventory rows
owner-local and affected derived facts
```

A tombstone may hide base rows even when no replacement facts exist.

This is how a compile failure withdraws invalid semantic facts without deleting unrelated facts.

---

## 104. Consolidated overlay rule

The serving path SHALL use:

```text
base + one consolidated overlay
```

not an unbounded chain of overlays.

When a new wave publishes:

- merge its replacements into the existing overlay;
- replace prior overlay rows for the same owner/table;
- preserve later generations;
- create a new immutable consolidated overlay;
- atomically swap snapshot pointer.

---

## 105. Overlay-aware DataFusion providers

For each owner-scoped table:

```text
current rows =
    overlay replacement rows
    UNION ALL
    base rows
      ANTI JOIN overlay tombstoned owner/table pairs
      ANTI JOIN overlay replaced owner/table pairs
```

A custom `TableProvider` SHOULD push:

- owner ID;
- owner bucket;
- entity/source/target ID;
- file ID;
- capability filters.

The provider SHALL bind one `ServingSnapshot`, not consult mutable global state during execution.

---

## 106. Atomic pointer swap

The coordinator constructs the complete new `ServingSnapshot` off-path.

Activation:

1. validate snapshot;
2. publish snapshot through `tokio::sync::watch<Arc<ServingSnapshot>>` or a tiny `RwLock<Arc<_>>`;
3. update workspace status;
4. notify waiters.

Query handlers clone the `Arc`.

No long-running query holds a global graph lock.

---

## 107. Durable overlay flush

Flush policy is threshold-based:

```text
maximum overlay age
maximum overlay rows
maximum overlay bytes
maximum changed owners
idle/quiescence opportunity
shutdown drain
```

Flush steps:

1. capture overlay watermark;
2. stage Delta publication;
3. replace affected owners;
4. reconcile and validate;
5. activate durable publication;
6. rebase any newer overlay changes on new base;
7. atomically publish rebased hot snapshot;
8. retire old base after readers release and retention permits.

---

## 108. Crash recovery policy

### 108.1 Default local interactive mode

The hot overlay need not be fsynced per edit because source files are authoritative.

After crash, rebuild missing current changes from source.

### 108.2 Optional durable overlay journal

A service requiring faster crash recovery MAY append compact Arrow IPC change manifests before hot activation.

The journal stores operational update inputs, not semantic history.

### 108.3 Invariant

A lost hot overlay may cause temporary durable lag after restart, but SHALL never cause a false current semantic claim.

---

# Part VIII — Scheduling, Parallelism, and Backpressure

## 109. Runtime responsibility split

### 109.1 Tokio

Use for:

- watcher channel and coordinator;
- timers/gather windows;
- filesystem and object-store I/O;
- Pyrefly and compiler-process orchestration;
- query RPC/daemon protocol;
- freshness waiters;
- publication orchestration;
- shutdown.

### 109.2 Rayon

Use for:

- hashing large batches;
- Tree-sitter/Ruff parsing across files;
- normalization;
- owner-local CFG/dataflow;
- graph projection construction;
- per-owner derivation;
- parallel encoding of Arrow batches.

### 109.3 Crossbeam

Use only where a synchronous CPU pipeline benefits from:

- bounded MPMC channels;
- work-stealing deques;
- specialized low-level queues.

Do not build a custom scheduler when Rayon suffices.

### 109.4 DashMap

Use selectively for:

- content-addressed immutable caches;
- provider-result caches;
- workspace/common-repository lookup registry;
- query-plan cache.

Do not use DashMap as the central update transaction manager.

Never hold DashMap guards across `.await` or nested map operations.

### 109.5 `tokio-rayon`

May bridge Tokio orchestration to Rayon jobs.

It is not admission control; pair it with semaphores and generation checks.

### 109.6 Bounded blocking gix execution class

gix repository, status, index, tree, and ODB work is primarily blocking filesystem/CPU work.

It SHALL NOT run directly on latency-sensitive Tokio workers.

Recommended placement:

```text
Tokio worktree coordinator
    ↓ bounded Git-work semaphore
dedicated blocking Git worker or bounded spawn_blocking task
    ↓
gix open/status/index/tree/ODB operation
    ↓ detached application DTO
Tokio coordinator
```

A dedicated blocking Git pool is generally clearer than Rayon for filesystem-heavy status/dirwalk work. CPU-heavy normalization of the returned candidate set MAY move to Rayon.

gix internal `parallel` behavior and CodeFabric outer parallelism SHALL be coordinated through the same process-wide thread budget.

## 110. Actor-owned coordinator

Each workspace SHOULD have one Tokio coordinator task owning:

- dirty registry;
- lifecycle state;
- active wave;
- generation counters;
- snapshot pointer;
- provider job registry;
- reconcile flag.

Commands arrive through bounded channels.

Heavy work is delegated; immutable results return to the coordinator.

This provides deterministic state transitions without coarse locking.

---

## 111. Work priorities

Recommended priority classes:

```text
P0  strict-current query target / active edited file
P1  fast source and syntax update
P2  semantic provider update for recently edited owners
P3  owner-local derived facts
P4  interprocedural convergence
P5  durable Delta flush
P6  compaction, vacuum, audits, cache warming
```

Bulk rebuild SHALL not starve P0–P2 work.

---

## 112. Admission control

Maintain process-wide limits for:

```text
concurrent source reads
Rayon CPU work
concurrent blocking gix jobs
gix status/tree-diff operations
concurrent Python semantic jobs
concurrent rustc invocations
DataFusion derivations
Delta writers
query executions
```

Rust compiler concurrency SHOULD normally be low because rustc consumes substantial CPU and memory and uses its own internal parallelism.

Git-job concurrency SHOULD also remain low: a small number of bounded status/tree/ODB jobs is generally preferable to many simultaneously internally parallel gix jobs.

## 113. Thread-budget policy

All pools share one hardware budget.

Recommended starting approach:

```text
Tokio workers:
  small I/O/orchestration pool, not one worker per logical CPU by default

Interactive Rayon pool:
  reserved for small latency-sensitive tasks

Bulk Rayon pool:
  bounded so interactive + bulk threads do not exceed CPU budget

gix blocking workers:
  small bounded pool; coordinate gix internal parallelism with outer job count

rustc processes:
  one or a small number per workspace/process

DataFusion target partitions:
  workload-adjusted and coordinated with the same budget
```

Potential gix modes:

```text
interactive:
  one bounded Git job
  internal parallel enabled only when benchmarked beneficial

multi-worktree bulk:
  few outer Git jobs
  reduced or disabled internal parallel

single huge monorepo reconcile:
  one outer job
  internal parallel enabled
```

Exact values SHALL be benchmarked on target hardware.

## 114. Supersession and cancellation

### 114.1 Generation rule

Every work item carries:

```text
worktree generation
source digest
owner generation
provider context
GitStateVector fence when Git-derived
```

Before accepting output:

```text
if result generation != latest required generation:
    mark STALE_RESULT
    discard

if Git-derived baseline no longer matches:
    discard and retry/broaden reconcile
```

### 114.2 Cooperative cancellation

Rayon/custom loops SHOULD check a generation or cancellation atomic at bounded intervals.

### 114.3 rustc process cancellation

Policy:

- terminate early superseded invocations when substantial cost remains;
- allow near-complete invocation to finish and discard if stale;
- never let a stale invocation publish.

### 114.4 DataFusion cancellation

Custom operators SHALL support cancellation and memory limits.

### 114.5 gix interruption

Long-running status, dirwalk, tree-diff, rename detection, and object traversal SHOULD receive a gix interruption signal connected to:

- wave supersession;
- workspace shutdown;
- strict query deadline;
- bulk reconcile cancellation;
- global resource pressure.

A cancelled gix operation produces no authoritative candidate delta.

## 115. Backpressure policy

Every unbounded queue is prohibited unless bounded accumulation is proven elsewhere.

When overloaded:

1. coalesce same-path work;
2. discard superseded generations;
3. escalate to bulk reconcile;
4. defer lower-priority derived/durable work;
5. preserve query service headroom;
6. expose degraded status.

Debounce timeout SHALL NOT be used as the primary overload-control mechanism.

---

## 116. Cache policy

Recommended caches:

```text
CodeFabric current-byte digest → parsed source/syntax facts
Git blob OID + worktree-transform fingerprint → current source digest
Git blob OID + transform/provider/path context → reusable parse/owner facts
HEAD tree pair + diff options → candidate tracked-tree delta
Git inclusion-policy fingerprint → compiled ignore/attribute stack
common repository identity → shared immutable ODB/cache resources
semantic owner fingerprint → normalized owner batch
dependency tuple → external summary
CFG fingerprint → derived control facts
call graph component fingerprint → SCC result
summary input hash → callable summary
DataFusion PlanSpec + snapshot schema hash → logical plan
```

Cache hierarchy:

```text
Level 1  CodeFabric BLAKE3 digest of actual worktree bytes
Level 2  Git blob OID + transform fingerprint
Level 3  owner semantic fingerprint
Level 4  derived projection fingerprint
```

A Git blob cache hit is reusable only when the file is clean and transform, line-ending, encoding, mode, path-sensitive semantic context, and provider context safeguards pass.

Cache entries SHALL be immutable and versioned by provider/schema/derivation bundle.

gix object and pack caches SHALL be bounded by CodeFabric's global memory budget. Commit-graph/history caches are not enabled merely because gix provides them.

# Part IX — Failure Taxonomy and Recovery

## 117. Failure classes

### 117.1 Watcher failures

```text
construction failure
registration failure
backend runtime error
watch exhaustion
root removal
event loss / rescan
channel saturation
handler panic
```

### 117.2 Git-state failures

```text
GIT_REPOSITORY_NOT_FOUND
GIT_REPOSITORY_CORRUPT
GIT_LAYOUT_CHANGED
GIT_WORKTREE_REMOVED
GIT_COMMON_DIR_UNAVAILABLE
GIT_HEAD_UNBORN
GIT_HEAD_NON_COMMIT
GIT_HEAD_CHANGED_DURING_SCAN
GIT_INDEX_CHANGED_DURING_SCAN
GIT_INDEX_CORRUPT
GIT_INDEX_CONFLICT_STAGES
GIT_STATUS_CANCELLED
GIT_STATUS_FAILED
GIT_TREE_DIFF_FAILED
GIT_OBJECT_MISSING
GIT_OBJECT_CORRUPT
GIT_INCLUSION_POLICY_CHANGED
GIT_ATTRIBUTES_UNTRUSTED
GIT_FILTER_EXECUTION_FORBIDDEN
GIT_SUBMODULE_LIMIT
GIT_PATH_ENCODING_UNREPRESENTABLE
GIT_HANDLE_STALE
GIT_LOCK_CONTENTION
```

### 117.3 Source failures

```text
file disappears during read
permission denied
unstable/torn read
invalid encoding
oversized input
symlink escape
path normalization failure
```

### 117.4 Syntax/provider failures

```text
Tree-sitter grammar mismatch
parser cancellation
Ruff parse failure
Ruff panic
Pyrefly sidecar crash
Pyrefly timeout
Pyrefly protocol mismatch
rustc compile error
rustc crash
build script/proc macro failure
extractor panic
compiler protocol corruption
toolchain unavailable
```

### 117.5 Reconciliation failures

```text
span mismatch
semantic identity collision
duplicate primary key
provider conflict
missing endpoint
schema mismatch
unknown enum code
incomplete owner manifest
stale GitStateVector
ambiguous worktree identity
```

### 117.6 Derived-analysis failures

```text
fixed-point nonconvergence
iteration limit
memory limit
spill failure
cancellation
graph invariant failure
algorithm panic
```

### 117.7 Storage/publication failures

```text
Delta optimistic conflict
partial table commits
object-store outage
disk full
schema conflict
checksum mismatch
publication validation failure
pointer update failure
catalog reopen failure
local operational lock/tempfile failure
```

### 117.8 Query failures

```text
snapshot unavailable
strict freshness timeout
fact family unavailable
workspace source unverified
Git state unverified
query cancellation
query memory limit
```

## 118. Failure-policy matrix

| Failure | Current source/syntax | Semantic facts | Query behavior | Recovery |
|---|---|---|---|---|
| Python parse error | Current via Tree-sitter | Affected families unavailable | Source + diagnostics + gap | Next edit/provider recovery |
| Pyrefly failure | Current | Local facts only; project types/calls unavailable | Partial facts, no negative claims | Restart/backoff |
| Rust compile error | Current via Tree-sitter | Invalidated compiler facts unavailable | Source + compiler diagnostics | Next successful compile |
| rustc extractor crash | Current | Affected compilation unit unavailable | Provider failure | Quarantine/restart |
| Event loss | Active snapshot potentially stale | Unchanged snapshot only | Strict queries wait/fail | Authoritative reconcile |
| Queue overflow | Active snapshot potentially stale | No blind incremental claim | Mark reconcile pending | Git-aware/generic bulk reconcile |
| Git repository not found | Current filesystem available | Normal source semantics possible | Git acceleration unavailable | Generic inventory/discovery retry |
| Git metadata corrupt | Current filesystem available | Normal source semantics possible | Git diagnostic; no clean-state inference | Generic full reconcile |
| Git index corrupt | Current filesystem available | Source CPG remains possible | Index/status acceleration unavailable | Generic inventory |
| HEAD/index changes mid-scan | Active snapshot retained | Candidate result rejected | Update pending | Retry/broaden reconcile |
| gix status/tree diff fails | Current filesystem available | No candidate inference accepted | Git acceleration degraded | Generic hash/inventory reconcile |
| Missing/corrupt blob | Current worktree read | Blob cache unavailable | No semantic gap if file readable | Read current bytes |
| Conflict stages | Current conflict content | Provider-dependent | Return conflict metadata + current facts/gaps | Continue as files change |
| Derived timeout | Base facts current | Derived family unavailable | Return base facts and gap | Retry/broader resources |
| Delta conflict | Hot snapshot current | Current hot facts | Durable lag only | Idempotent retry |
| Hot snapshot validation fail | Old snapshot current | Old snapshot current | No new snapshot | Retry/reconcile |
| Disk full | Active snapshot current | Hot overlay may continue within memory budget | Durable lag/block status | Operator action/cleanup |
| Daemon crash | Durable publication only | Rebuilt from source | Bootstrap/warm recovery | Reconcile |

gix failure SHALL degrade acceleration before it degrades source correctness.

## 119. Retry and backoff

Retryable:

```text
transient file lock/read
sidecar unavailable
object-store timeout
Delta conflict
polling/backend re-registration
gix repository temporarily unavailable
gix status/index/tree scan interrupted by concurrent Git mutation
Git metadata lock contention
```

Non-retryable without input, configuration, or version change:

```text
schema/protocol mismatch
unsupported provider version
deterministic parser adapter bug
invalid configuration
identity collision
forbidden external filter/command policy
unsupported repository format
```

Backoff SHALL be bounded, cancellation-aware, and isolated by provider/worktree.

## 120. Circuit breakers

Repeated failure of one provider or accelerator SHOULD open a per-provider/worktree circuit:

- stop rapid retry;
- keep other capabilities available;
- report degraded state;
- periodically probe;
- close after successful health check.

A gix circuit breaker disables Git acceleration and activates generic authoritative inventory; it does not disable current source analysis.

## 121. Fail-closed rules

The system SHALL fail closed for:

- incomplete rustc extraction manifest;
- ambiguous owner replacement;
- invalid relation endpoints;
- cross-generation source spans;
- stale Git-state candidate deltas;
- ambiguous repository/worktree identity;
- untrusted or unrepresentable path conversion;
- unknown publication table versions;
- schema incompatibility;
- capability status missing for invalidated families.

Fail closed means withholding affected facts or acceleration, not shutting down all query service.

When gix cannot provide trustworthy state but current filesystem bytes are readable, fail closed on the Git-derived candidate result and fall back to generic reconciliation.

# Part X — Agent and MCP Delivery Contract

## 122. Central daemon and workspace-registry authority

One central Rust daemon MAY manage one authorized repository/worktree group and multiple registered workspaces. It owns:

- the workspace registry and authorization bindings;
- optional common Git repository actors;
- one coordinator and active snapshot pointer per workspace;
- source/Git lifecycle, provider orchestration, reconciliation/derivation scheduling;
- query execution, capability state, and result artifacts.

FastMCP STDIO instances are per-agent coordination/presentation processes. They SHALL not maintain independent mutable CPG, Git, workspace-default, or snapshot state.

## 123. Daemon connection and request identity

Recommended local transport remains UDS on Linux/macOS and an explicitly secured local equivalent on other platforms.

Every request carries:

```text
agent_instance_id
workspace_id
semantic_request_id optional
mcp_call_id optional
rpc_attempt_id
freshness_policy
analysis_context selectors or defaults
request deadline
query payload
optional target paths/entities
```

`repository_id` and `worktree_id` may be echoed for diagnostics but are never routing substitutes. A request for an unregistered or unauthorized workspace is rejected; the daemon SHALL not resolve a mutable repository-level default.

## 124. Structured freshness policies

### 124.1 `BEST_AVAILABLE_SNAPSHOT`

Return immediately from the newest active immutable snapshot. It may be `POTENTIALLY_STALE`; this policy is explicit opt-in and never the default.

### 124.2 `AWAIT_LATEST`

Wait until all source events admitted before the barrier are represented in an active hot snapshot, up to deadline.

### 124.3 `REQUIRE_CURRENT_FOR_TARGETS` — default

Resolve target paths/owners and requested fact families, prioritize their update, and return only when those targets are current or explicitly unavailable for the current source.

### 124.4 `REQUIRE_SOURCE_CURRENT`

Require current source and syntax for the requested scope. Semantic/derived capabilities may be unavailable but prior semantic facts are not substituted.

### 124.5 `REQUIRE_SEMANTIC_CURRENT`

Require every requested semantic/derived capability for the resolved target scope or fail with explicit unavailable coverage.

The former `latest_published` label is deprecated and maps only to `BEST_AVAILABLE_SNAPSHOT` for internal compatibility.

## 125. Freshness barrier and pinning order

A barrier captures:

```text
workspace_id
event sequence at admission
newest dirty/source generation
target file digests or owner IDs when specified
requested analysis contexts and capabilities
GitStateVector fence when a Git candidate set was used
deadline and cancellation token
```

Completion requires:

- no required target has an unprocessed generation at or below the barrier;
- source trust is `CURRENT` for the required scope;
- a Git-derived candidate set, if used, remained valid or generic reconciliation replaced it;
- required capabilities are `CURRENT` or explicitly unavailable for the current bytes/context;
- a complete immutable `ServingSnapshot` covering the barrier is `READY`/`ACTIVE`.

The query sequence is normative:

```text
resolve authorized workspace
→ accept daemon query handle
→ apply freshness barrier
→ atomically clone Arc<ServingSnapshot>
→ acquire snapshot lease
→ resolve/plan/execute
```

A degraded gix accelerator does not block completion after generic current-byte reconciliation.

## 126. PublicSnapshotMetadata and query status

Every status/query/artifact surface SHALL reuse one versioned `PublicSnapshotMetadata` shape:

```text
snapshot_id
workspace_id
repository_id optional
worktree_id optional
source_generation
source_inventory_digest
durable_base_publication
base_table_version_digest
overlay_generation
overlay_checksum
analysis_context_set_id
analysis_context_ids
freshness_state
source_trust_state
event_stream_health
git_acceleration_status
git_operation_summary optional
pending_update_count
ontology_version
schema_bundle_version
provider_bundle_version
derivation_bundle_version
query_language_version
capability_summaries
diagnostic_references
```

Query execution, availability, completeness, freshness, limit, and dependency states are separate fields. Git object IDs are operational and omitted unless required to explain currentness/conflict.

Every governed execution allocates an execution ID before planning and records
the versioned semantic query identity, plan identity, input snapshot and Delta
versions, configuration fingerprint, software/library domain, output checksum
contract, and modeled reproducibility status. Unsupported volatility or
environment capture is explicit. Plan text and collected metrics are
diagnostics associated with that execution; neither is semantic identity, and
diagnostic collection SHALL NOT re-execute the query.

## 127. Empty-result semantics

The service SHALL distinguish:

```text
PROVEN_EMPTY
FILTERED_EMPTY
UNRESOLVED
FACT_FAMILY_UNAVAILABLE
PROVIDER_FAILED
UPDATE_PENDING
SOURCE_UNVERIFIED
LIMIT_REACHED
```

A compile failure SHALL never be represented as a normal empty call graph or empty type result.

---

## 128. Source fallback delivery

When semantic facts are unavailable for a current source owner, the query layer SHOULD include:

- current source file and range;
- worktree identity;
- enclosing syntax owner;
- parse/compiler diagnostics;
- Git conflict state when applicable;
- unavailable capability list;
- source retrieval handle or inline bounded context.

This is not an engineering recommendation; it is a transparent delivery fallback.

A compile or parse failure in a conflicted worktree SHALL not be represented as a normal empty semantic result.

## 129. Multiple-agent fairness

The daemon SHOULD enforce:

- per-client query concurrency limits;
- global query admission;
- update priority independent of client identity;
- optional priority boost for targets referenced by an active query;
- no permanent starvation from one agent's bulk operations.

---

# Part XI — Durable Operational State

## 130. Operational-state schemas and backing store

The lifecycle tables are stored in the data-fabric-defined embedded SQLite WAL operational database and exposed as read-only Arrow/DataFusion tables. They are not Delta fact history.

Required tables/views:

### 130.1 `common_repository_state`

```text
repository_id
common_dir_path_bytes/display
object_format_code
gix_version
trust_policy_fingerprint
worktree_count
git_health_code
updated_at
last_diagnostic_id optional
```

### 130.2 `worktree_state`

```text
workspace_id primary key
worktree_id optional unique
repository_id optional
work_dir_path_bytes/display
git_dir_path_bytes/display optional
lifecycle_state_code
source_trust_state_code
event_stream_health_code
git_acceleration_status_code
active_snapshot_id
analysis_context_set_id
source_generation
event_watermark
newest_dirty_generation
durable_generation
reconcile_required
updated_at
last_diagnostic_id optional
```

### 130.3 `git_state_vector`

Keyed by workspace and active source generation; fields remain the current HEAD/tree/index/inclusion fingerprints needed for verification and recovery.

### 130.4 `update_wave` and `update_wave_item`

Carry workspace ID, source generation, event watermark, state, candidate strategy, input fingerprints, counts, timing, and diagnostics. Paths use bytes/display/encoding fields.

### 130.5 `provider_run`

Carries workspace, analysis context, wave, provider, owner/build unit, source generation, input/output fingerprints, state, accepted/terminal times, and diagnostic.

### 130.6 `git_operation_run`

Carries workspace, baseline/result Git fingerprints, candidate/verified counts, state, timing, and diagnostic.

### 130.7 `table_mutation_operation`

Carries the generated Data Fabric §70 coordinator journal for every retryable
Delta mutation phase: operation/publication/table identity, application
identity and monotonic version, owner-set and input/expected-output checksums,
expected predecessor and committed Delta version, lifecycle state, and timing.
The operational database is the sole application-version allocator; Delta
application transactions remain the per-table commit authority.

### 130.8 `hot_overlay_manifest`, `snapshot_lease`, and `result_artifact_lease`

`snapshot_lease` carries the complete workspace, snapshot, base-publication,
required-version, overlay, holder/process, heartbeat, expiry, lifecycle, orphan,
and optional source-blob-holder coupling required for recovery and safe
retirement. It is the sole snapshot-lease authority. `result_artifact_lease` is
a normalized extension keyed by `lease_id` and adds only the artifact URI,
checksum, and artifact expiry; it does not duplicate snapshot identity or lease
lifecycle. `serving_snapshot_manifest` stores the exact AC-G-19 CBEF body, the
closed typed JSON view, and its digest, while activation observations remain in
`active_snapshot` and mutable activation columns. The v5-to-v6 pre-production
migration replaces the earlier unimplemented scaffold tables with this model;
no compatibility authority is retained.

### 130.9 Compatibility view

`workspace_update_state` MAY remain as a compatibility projection of `worktree_state`. New code uses the explicit workspace identity.

---

## 131. Operational-state retention and atomicity

SQLite transactions SHALL atomically update wave/provider state and the `active_snapshot` pointer record. In-memory pointer swap remains the query fast path; the operational record supports startup verification and diagnostics.

Retention is bounded. Cleanup preserves:

- registered workspace/common-repository topology;
- active and recovery-required durable publications;
- active snapshot manifest and overlay journal when enabled;
- current source/Git/context state required for recovery;
- in-flight waves/provider jobs/cancellation records;
- active snapshot and result-artifact leases.

Cleanup additionally preserves the complete provenance closure reachable from
every retained result or publication: execution and semantic request records,
plan artifacts, schema/specification/input identities, Delta commit metadata,
and source snapshot/blob references. A retention policy may expire a complete
closure only as one governed unit; it may not leave a retained result whose
lineage silently terminates.

Retained operational Git IDs or prior wave rows do not authorize historical source querying.

# Part XII — Performance Objectives and Tuning

## 132. Latency decomposition

Measure:

```text
event occurrence
→ debounced delivery
→ dirty/Git metadata registration
→ candidate strategy selected
→ gix candidate set ready when used
→ source image captured
→ fast syntax ready
→ hot snapshot active
→ semantic snapshot active
→ durable publication active
```

Metrics:

```text
watch_to_dirty
dirty_to_git_candidate
git_candidate_to_source
dirty_to_source
source_to_fast
fast_to_semantic
semantic_to_hot
hot_to_durable
query_freshness_wait
```

For ordinary isolated saves, `dirty_to_git_candidate` SHOULD be absent because no full Git scan is required.

## 133. Initial performance objectives

These are benchmark targets, not guarantees.

For a small isolated edit on a local filesystem:

```text
fast source/syntax visibility:
  target sub-second, preferably a few hundred milliseconds

ordinary-save Git overhead:
  near zero beyond path mapping and cached topology lookup

Python semantic convergence:
  target sub-second to low-single-digit seconds depending project

Rust semantic convergence:
  bounded primarily by rustc incremental check and affected target

query snapshot acquisition:
  near-constant-time Arc clone

durable Delta convergence:
  asynchronous and micro-batched
```

For Git-aware bulk transitions:

```text
candidate pruning:
  hash/read only paths produced by tree diff, status, inclusion reconciliation,
  watcher dirtiness, and verification policy

worktree topology refresh:
  bounded and independent from source-semantic generation

fallback:
  generic authoritative reconcile remains correctness-preserving
```

The daemon SHALL report actual p50/p95/p99 performance and gix candidate-pruning ratios.

## 134. Work granularity

Prefer:

- one cheap event registration per watcher/Git metadata category;
- one bounded gix job per reconcile or bulk transition, not per path;
- file batches for source parsing;
- owner batches for local derivation;
- module/crate batches for semantic providers;
- component batches for graph fixed points;
- multi-owner Arrow batches for storage.

Avoid one task, one status scan, one gix repository open, or one Parquet file per individual fact.

## 135. Memory policy

- immutable source bytes use `Arc`;
- worker-local builders avoid shared mutation;
- bounded queues;
- limited DataFusion memory pool;
- bounded spill directory;
- cache byte limits;
- gix object/pack cache limits;
- bounded Git candidate and rename sets;
- overlay size thresholds;
- early bulk-reconcile escalation under storms;
- common-repository immutable caches may be shared, but worktree mutable state may not.

## 136. Query headroom

Reserve capacity so update and Git reconciliation work does not consume every CPU, blocking thread, or memory resource.

Under overload:

- lower-priority durable and global derived work pauses first;
- expensive rename detection and cache warming are disabled;
- gix candidate scans remain bounded;
- source/syntax and query-target refresh remain prioritized;
- query service continues on the immutable snapshot;
- generic reconcile may replace repeated failed Git acceleration attempts.

# Part XIII — Validation and Testing

## 137. Correctness oracle

For any fixed worktree source snapshot and provider/derivation bundle:

> **The incrementally maintained CPG SHALL equal a clean rebuild under the same source bytes, worktree identity, inclusion policy, provider versions, schema versions, and derivation versions.**

Equality includes:

- canonical IDs;
- source inventory;
- worktree-relative paths;
- owner fact sets;
- capability statuses;
- unknown facts;
- derived facts;
- summaries;
- source spans.

Additionally:

```text
incremental source inventory
=
fresh Git-aware inventory + authoritative current-byte reads
```

where gix is healthy, and SHALL equal the generic authoritative inventory when gix acceleration is unavailable.

## 138. Lifecycle scenario tests

Minimum automated scenarios:

```text
cold bootstrap in Git and non-Git roots
warm unchanged start
offline source changes
single edit
repeated save supersession
atomic save
create/delete/rename/move
directory removal
whitespace-only edit
Python parse error
Python type error
Pyrefly crash
Rust syntax error
Rust type/borrow error
build script failure
Cargo manifest change
macro change
formatter burst
branch switch
checkout with surviving local changes
event loss/rescan
queue saturation
query during update
strict freshness timeout
provider timeout
derived nonconvergence
Delta conflict
crash at every publication boundary
graceful shutdown drain
forced shutdown cancel
schema/provider/gix upgrade
linked-worktree addition/removal
submodule topology change
.gitignore and attribute changes
index-only staging
conflict stages
corrupt Git metadata with generic fallback
```

## 139. Watcher and Git-metadata tests

Tests SHALL assert final authoritative state, not exact platform-specific event sequences.

Include:

- atomic replace;
- matched and unmatched rename;
- move across watch boundary;
- directory delete;
- rescan injection;
- queue overflow escalation;
- polling backend;
- symlink policy;
- root deletion;
- selected HEAD/index/operation-state metadata events;
- metadata event coalescing;
- no source-semantic regeneration for index-only staging;
- Git metadata watch recovery after linked-worktree changes.

## 140. Generation and supersession tests

- stale parse result cannot commit;
- stale rustc result cannot commit;
- stale Pyrefly result cannot commit;
- stale gix status/tree candidate delta cannot commit;
- HEAD/index change during reconcile forces retry or broadening;
- newer edit during reconcile is replayed;
- wave supersession preserves latest digest;
- query snapshot remains stable during pointer swap;
- gix interruption produces no authoritative candidate set.

## 141. Capability-gap and Git-degradation tests

Verify distinct responses for:

```text
syntax current / semantic pending
syntax current / compile failed
Python local facts current / Pyrefly unavailable
derived facts unavailable
source unverified after event loss
Git acceleration unavailable but source current through generic reconcile
Git state changed during scan
conflicted path with current source facts
proven empty with complete fact family
```

## 142. Crash-injection tests

Inject failure:

```text
before hot pointer swap
after hot pointer swap
during each Delta table commit
after publication COMPLETE
before current_publication update
after current_publication update
during catalog refresh
during overlay rebase
while holding CodeFabric singleton lease
during local tempfile/manifest rename
during gix status/tree scan
after Git candidate generation but before source verification
```

After restart, active query state SHALL be coherent and the current worktree SHALL be reconciled authoritatively.

## 143. Parallelism and handle-safety tests

- bounded queue memory;
- no DashMap guard across await;
- no deadlock under concurrent edits/queries;
- thread budget respected;
- bulk work does not starve query-target updates;
- cancellation stops fixed-point work;
- rustc concurrency remains bounded;
- gix blocking concurrency remains bounded;
- no shared long-lived `Arc<gix::Repository>`;
- attached gix values do not cross update waves;
- common-repository caches do not leak worktree-specific HEAD/index state.

## 144. Performance and Git-acceleration tests

Workloads:

```text
single save
save + formatter
10-file refactor
1,000-file generated update
10,000-file branch switch
100,000-file warm reconcile
compile-failing Rust edit
rapid 100-edit supersession
concurrent queries from multiple agents
multiple linked worktrees
large ignored build tree
large rename set
```

Measure:

```text
events received
events escalated to reconcile
unique dirty paths
gix open/status/index/tree-diff duration
gix candidate path count
gix candidate pruning ratio
generic fallback reconciles
source reads
hashes
parses
semantic jobs
stale jobs discarded
owner replacements
hot publication latency
durable publication latency
CPU
RSS
gix cache bytes/hit rate
spill
queue wait
query wait
```

Benchmark cold and warm gix cache paths separately.


## 145. gix integration conformance suite

### 145.1 Repository topology

```text
normal worktree
bare repository
unborn HEAD
detached HEAD
non-commit HEAD target where constructible
linked worktrees
worktree removal
common-dir sharing
```

### 145.2 Paths and inclusion

```text
nested .gitignore
info/exclude
trusted global excludes
tracked ignored file
ignored untracked file
explicit CodeFabric override
pathspec include/exclude
case-only rename
non-UTF8 Git path where supported
directory/file transition
symlink
```

### 145.3 Index and status

```text
staged-only change
unstaged change
both staged and unstaged
untracked source
deleted tracked file
executable-bit-only change
symlink mode change
conflict stages
external Git index writer
corrupt/truncated index
```

### 145.4 Bulk operations

```text
branch switch with no local changes
branch switch preserving local modification
branch switch with untracked files
large rename set
checkout event storm
HEAD moves but worktree remains byte-identical
repository operation state changes
```

### 145.5 Submodules and worktrees

```text
initialized submodule
uninitialized submodule
gitlink change
nested repository not a submodule
multiple linked worktrees edited concurrently
```

### 145.6 Failure and security

```text
corrupt object
missing object
object cache limit
status interruption
diff interruption
external filter configured but forbidden
untrusted repository config
Windows symlink/reparse-point fixture
gix lock contention
```

## 146. Git CLI parity oracle

Selected gix behavior SHOULD be cross-checked against a pinned Git executable in tests.

Appropriate oracle commands MAY include:

```text
git rev-parse
git status --porcelain=v2 -z
git ls-files -z
git check-ignore -z
git diff-tree --raw -z
git worktree list --porcelain
git submodule status
```

The Git CLI is a test oracle and debugging fallback, not a production dependency on the hot path.

Outputs SHALL be parsed in byte-safe/NUL-delimited forms where available.

## 147. gix upgrade gate

On every gix upgrade:

1. exact-pin the candidate release;
2. inspect Cargo feature changes;
3. inspect Gitoxide crate-status changes;
4. review security advisories;
5. compile the adapter;
6. run the Git parity suite;
7. run linked-worktree and conflict-stage suites;
8. run cross-platform tests;
9. measure cold/warm/status/tree-diff performance;
10. compare clean-rebuild equivalence;
11. update the capability matrix;
12. advance the supported version only after explicit review.

# Part XIV — Observability and Operations

## 148. Required metrics

### Watcher

```text
watch_events_total
watch_batches_total
watch_queue_full_total
watch_reconcile_requested_total
watch_backend_errors_total
watch_health_state
```

### Git state and reconciliation

```text
gix_open_duration
gix_status_duration
gix_status_entries
gix_dirwalk_duration
gix_dirwalk_entries
gix_tree_diff_duration
gix_tree_diff_entries
gix_rename_candidates
gix_index_load_duration
gix_index_entries
gix_conflicted_paths
gix_object_cache_bytes
gix_object_cache_hits
gix_object_cache_misses
gix_candidate_paths
gix_candidate_pruning_ratio
gix_fallback_reconciles
gix_interruptions
gix_errors_total by class
git_head_changes
git_index_changes
git_operation_state_changes
git_inclusion_policy_changes
linked_worktree_count
submodule_count
```

### Update pipeline

```text
dirty_paths
active_wave
waves_superseded_total
source_snapshot_retries_total
fast_update_duration
semantic_update_duration
derived_update_duration
hot_publication_duration
durable_lag_generations
```

### Providers

```text
provider_runs_total
provider_failures_total
provider_timeouts_total
provider_stale_results_total
rustc_compile_duration
pyrefly_duration
parse_duration
```

### Snapshot/query

```text
active_snapshot_generation
snapshot_pointer_swaps_total
query_snapshot_age
query_freshness_wait
queries_with_unavailable_capabilities
queries_with_git_acceleration_degraded
```

### Storage

```text
delta_commits_total
delta_conflicts_total
overlay_rows
overlay_bytes
small_file_count
publication_validation_failures
```

### Key Git performance metric

```text
candidate_pruning_ratio =
  1 - candidate_paths_hashed / total_included_paths
```

Measure it separately for warm startup, branch switch, watcher-loss reconcile, formatter burst, and ordinary save.

## 149. Structured tracing

Every update trace SHOULD carry:

```text
repository_id
worktree_id
wave_id
source_generation
event_watermark
Git state fingerprint
HEAD/tree object ID when relevant
index fingerprint when relevant
Git operation state
Git candidate source
gix status mode
gix parallel mode
file_id
owner_id
provider_run_id
snapshot_id
publication_id
client_id when query-triggered
```

Absolute paths, raw Git path bytes, source bytes, remote URLs, credentials, and repository configuration values SHOULD be redacted from remote telemetry.

## 150. Health endpoint

The daemon SHOULD expose:

```text
common repository identity
worktree identity
workspace lifecycle
source trust
watcher backend
Git acceleration status
gix version
HEAD kind and compact target summary
index fingerprint/status
repository operation state
conflicted path count
inclusion/attribute policy fingerprint
active snapshot ID
source generation
dirty path count
active wave state
provider health
durable lag
last successful reconcile
last failure
```

The endpoint SHALL avoid exposing credentials, unredacted repository configuration, or unnecessary absolute paths.

# Part XV — Shutdown and Recovery

## 151. Shutdown ordering

```text
1. mark STOPPING
2. stop accepting new update waves
3. continue or reject new strict-current queries by policy
4. stop source and Git-metadata debouncers with joined stop
5. close ingress
6. choose drain or cancel
7. interrupt and stop active gix jobs
8. stop Pyrefly sidecars and compiler children
9. await worker completion
10. flush or discard hot overlay according to deadline
11. close durable stores
12. retire endpoint metadata
13. release the CodeFabric-owned singleton lease
14. release worktree and common-repository state
```

The watcher sources SHALL stop before consumer state is destroyed.

CodeFabric SHALL not leave Git repository locks behind and SHALL not hold locks that block ordinary Git CLI operations during shutdown.

## 152. Drain policy

Use when:

- daemon is expected to leave durable index current;
- shutdown deadline permits;
- active wave is near completion.

Drain only the newest non-superseded wave.

---

## 153. Cancel policy

Use when:

- process exit is urgent;
- source can be reconciled on restart;
- current durable publication remains valid.

Discard incomplete hot candidate and leave active pointer unchanged.

---

## 154. Startup readiness barrier

A worktree is `READY` only after:

- exact repository/worktree discovery completed or non-Git fallback was selected;
- source and selected Git metadata watchers registered;
- inventory verified;
- current `GitStateVector` captured when applicable;
- events during verification replayed;
- active snapshot constructed;
- `source_trust_state = CURRENT`;
- Git acceleration is `CURRENT`, explicitly `DEGRADED`, or not applicable;
- required provider capability policy is satisfied or explicit degraded mode entered.

A common-repository service is ready when its topology registry and shared immutable resources are initialized; this does not imply every linked worktree is ready.

# Part XVI — Rust Package Architecture

## 155. Recommended private module boundaries

These are application-owned module responsibilities inside CodeFabric's one stable
Cargo package. They do not imply packages, workspace members, or public crate boundaries.

```text
protocol
  daemon RPC, MCP-facing request/response DTOs, freshness contract

watch
  notify-debouncer wrapper, source/Git metadata event facade, watcher health

git_state
  read-only gix adapter, repository/worktree discovery, paths, inclusion,
  HEAD/index/status/tree diff, operation state, submodules, DTOs

git_worker
  bounded blocking gix execution, interruption, object/diff caches

git_testkit
  adversarial repositories, linked worktrees, conflict stages, Git CLI parity

source
  inventory, source images, BLAKE3 hashing, path/symlink policy, Merkle digest

coordinator
  common-repository registry, worktree actors, dirty registry, waves, lifecycle state

invalidation
  update classification, owner discovery, dependency propagation

python_update
  Tree-sitter/Ruff/Pyrefly orchestration

rust_update
  Cargo/rustc/MIR orchestration and compiler manifest handling

derived_update
  owner-local and interprocedural incremental analyses

hot_snapshot
  immutable overlay, tombstones, snapshot providers, pointer swap

durable_publisher
  Delta owner replacement, publication manifest, idempotent recovery

query
  snapshot-pinned DataFusion query service

daemon
  Tokio runtime, IPC server, repository/worktree registry, health, shutdown

fastmcp_boundary
  Python coordination layer; one STDIO process per agent
```

No public module outside `git_state` SHOULD expose gix types.

## 156. Core interfaces

```rust
#[async_trait::async_trait]
pub trait WorkspaceUpdater {
    async fn ensure_fresh(
        &self,
        request: FreshnessRequest,
    ) -> Result<FreshnessResult, UpdateError>;
}

pub trait GitStateAdapter {
    fn open_worktree(
        &self,
        root: &PlatformPath,
        registered: &RegisteredGitIdentity,
        policy: &GitTrustPolicy,
    ) -> Result<GitStateSnapshot, GitStateError>;

    fn capture_state(
        &self,
        identity: &GitWorktreeIdentity,
        observations: &GitStateObservations,
    ) -> Result<GitStateVector, GitStateError>;

    fn inventory(
        &self,
        identity: &GitWorktreeIdentity,
        observations: &GitStateObservations,
        cancel: &CancellationToken,
    ) -> Result<GitInventoryResult, GitStateError>;

    fn status_candidates(
        &self,
        identity: &GitWorktreeIdentity,
        baseline: &GitStateVector,
        cancel: &CancellationToken,
    ) -> Result<GitCandidateDelta, GitStateError>;

    fn tree_diff_candidates(
        &self,
        identity: &GitWorktreeIdentity,
        old_tree: &GitObjectId,
        new_tree: &GitObjectId,
        options: &GitTreeDiffOptions,
        cancel: &CancellationToken,
    ) -> Result<GitCandidateDelta, GitStateError>;
}

pub trait ChangeClassifier {
    fn classify(
        &self,
        old: Option<&IndexedSourceState>,
        new: Option<&SourceImage>,
        git: Option<&GitPathCandidate>,
        context: &ClassificationContext,
    ) -> UpdateClass;
}

pub trait InvalidationPlanner {
    fn plan(
        &self,
        changes: &[ClassifiedChange],
        snapshot: &ServingSnapshot,
    ) -> Result<InvalidationPlan, InvalidationError>;
}

pub trait HotSnapshotBuilder {
    fn build(
        &self,
        base: &ServingSnapshot,
        delta: ValidatedWaveDelta,
    ) -> Result<std::sync::Arc<ServingSnapshot>, SnapshotError>;
}

#[async_trait::async_trait]
pub trait DurableFlusher {
    async fn flush(
        &self,
        snapshot: std::sync::Arc<ServingSnapshot>,
        watermark: u64,
    ) -> Result<DurablePublicationResult, PublishError>;
}
```

The `GitStateAdapter` SHALL return detached CodeFabric DTOs. It SHALL not expose `gix::Repository`, attached objects, references, index handles, or thread-local provider state.

# Part XVII — Mandatory Invariants

## 157. Consistency invariants

```text
1. Every query pins exactly one immutable ServingSnapshot.
2. Every snapshot belongs to exactly one worktree_id.
3. A snapshot never mixes source generations.
4. Invalidated semantic facts are hidden before current syntax is published.
5. Missing provider output never proves absence.
6. Compile failure produces capability gaps, not stale-current compiler facts.
7. Unaffected owners remain queryable when their validity is proven.
8. Stale provider or gix results cannot commit.
9. Owner replacement is atomic at snapshot visibility.
10. Multi-table durable publication activates only through one complete manifest.
11. Watcher events are invalidation hints, not graph mutations.
12. gix status and tree diff generate candidates, not source facts.
13. Current stable filesystem bytes remain source truth.
14. CodeFabric BLAKE3 digest remains canonical current-content identity.
15. Git blob OID is auxiliary cache/baseline identity only.
16. need_rescan or queue overflow triggers authoritative reconciliation.
17. Every asynchronous job carries generation and source digest.
18. Every Git-derived job carries a GitStateVector fence.
19. Every derived fact references the exact base generation.
20. The active snapshot pointer changes only after validation.
21. Query serving never waits on a global graph mutation lock.
22. One linked worktree has one independent source lifecycle and CPG snapshot.
23. Common ODB/cache resources may be shared; worktree HEAD/index/source state may not.
24. Conflict stages are explicit and never collapsed to stage zero.
25. Git ignore rules are inclusion policy, never authorization.
26. External Git commands, filters, hooks, credentials, network, and mutation are disabled by default.
27. gix failure falls back to bounded authoritative filesystem reconciliation.
28. Repository-attached gix values do not cross long-lived concurrent boundaries.
29. Branch-switch optimization uses tree diff + status + current-byte verification.
30. Non-UTF8 Git paths remain representable.
31. gix 0.86.0 is the minimum supported release.
32. The incremental graph equals a clean Git-aware rebuild for the same worktree source snapshot.
```

## 158. Performance invariants

```text
1. Event handlers remain lightweight.
2. Queues are bounded or coalesced.
3. Repeated changes to one path collapse to one latest generation.
4. CPU-heavy work runs outside Tokio workers.
5. Blocking gix work runs in a bounded execution class.
6. Process-wide thread budgets include gix internal and outer parallelism.
7. Bulk work cannot starve interactive source/query work.
8. Full gix status does not run for every isolated save.
9. Very small updates do not force immediate Delta micro-files.
10. Hot overlays are consolidated rather than chained indefinitely.
11. Global derived tables are not recomputed when a bounded affected component suffices.
12. Git tree/status candidate pruning is used for bulk transitions where safe.
13. Blob-OID reuse never bypasses current-byte/transform safeguards.
14. Caches are immutable/versioned and have memory limits.
15. Linked worktrees share only immutable repository resources.
```

## 159. Failure invariants

```text
1. Provider or gix crash cannot partially mutate the active snapshot.
2. Publication failure cannot advance the current durable pointer.
3. Daemon crash cannot leave a partially visible in-memory snapshot.
4. Event loss is surfaced as degraded source trust.
5. Git-state mutation during scan invalidates the candidate result.
6. Reconciliation failure keeps the prior snapshot active.
7. Corrupt Git metadata degrades acceleration, not current-byte correctness.
8. Storage failure degrades durability without corrupting current query state.
9. Shutdown cancellation never labels incomplete work healthy.
10. Fact-family unavailability is explicit in every relevant query response.
11. Git conflict state is explicit and never presented as an ordinary clean file.
12. Forbidden Git command/filter/network behavior fails closed.
13. Missing Git objects do not prevent current source analysis when files are readable.
```

# CodeFabric 1.3 architecture-completion contracts

The standalone architecture-completion specification has been propagated into its permanent owners. This part contains the full normative contracts owned by this document: `G-09`, `G-10`, `G-11`, `G-24`, `G-25`, `G-27`, `G-28`, `G-29`, `G-41`, `G-62`. References to a gap ID elsewhere in the synchronized suite resolve to these sections.

## AC-G-09 — Generalized source-instance identity
### Decision

`workspace_id` is a persisted registration identity, not a hash of the current root path. Moving a registered root can preserve the workspace identity; copying or independently registering the same bytes creates a new workspace identity.

### Contract

At first registration the daemon generates a cryptographically random 128-bit `workspace_registration_nonce` and stores it in the operational registry. The canonical workspace ID is:

```text
workspace_id = BLAKE3_128(
  CBEF-v1(
    domain = WORKSPACE,
    registration_nonce,
    workspace_kind
  )
)
```

The nonce is not secret but is never user-selectable. It prevents accidental identity collision between independent copies of the same path or repository.

For Git-backed workspaces, repository registration generates a separate random 128-bit `repository_registration_nonce` and uses:

```text
repository_id = BLAKE3_128(CBEF-v1(
  domain = REPOSITORY,
  repository_registration_nonce
))

worktree_id = BLAKE3_128(CBEF-v1(
  domain = WORKTREE,
  repository_id,
  worktree_registration_nonce,
  worktree_kind
))

workspace_id = independent registered source-instance identity linked to worktree_id
```

Each worktree registration receives its own random 128-bit `worktree_registration_nonce`; reuse of a removed linked-worktree administrative name therefore cannot reuse identity. `worktree_administrative_key` is stored as verification metadata: `MAIN` for the main worktree and otherwise the exact byte-safe Git worktree administrative name/identity reported beneath the common repository's worktree administration area. It is not the mutable filesystem checkout path. Duplicate active administrative keys inside one repository are rejected.

For non-Git workspaces, `repository_id` and `worktree_id` are null.

Identity behavior:

| Event | Identity outcome |
|---|---|
| Root path moved and explicitly relinked to the same registered Git worktree or non-Git registration | Preserve `workspace_id`; update authorization fingerprint and root path record |
| Git linked worktree path moved while the same worktree administrative identity remains | Preserve `worktree_id` and `workspace_id` after reauthorization |
| Repository cloned or copied | New `repository_id`, `worktree_id`, and `workspace_id` |
| Non-Git directory copied | New `workspace_id` |
| Workspace removed then re-added without importing its registry record | New `workspace_id` |
| Root changes from non-Git to Git-backed | New workspace registration; identity is not silently converted |

A workspace record SHALL include a monotonically increasing `registration_revision`. Reconfiguration changes the revision and authorization/context fingerprints, not the workspace ID.
## AC-G-10 — Daemon workspace registry and administrative lifecycle
### Decision

Workspace mutation is performed only through a local administrative CLI/API distinct from the read-only query MCP and query RPC.

### Contract

The daemon exposes an admin service only to the same OS user and only over the private local IPC boundary. The supported commands are:

```text
codefabric workspace add <root>
codefabric workspace list
codefabric workspace show <workspace-id>
codefabric workspace relink <workspace-id> <new-root>
codefabric workspace configure <workspace-id> --profile <manifest>
codefabric workspace enable <workspace-id>
codefabric workspace disable <workspace-id>
codefabric workspace reconcile <workspace-id>
codefabric workspace remove <workspace-id> [--retain-data | --purge-data]
```

The workspace registry state machine is:

```text
REGISTERING → DISABLED
DISABLED → OPENING → BOOTSTRAPPING → READY
READY ↔ DEGRADED
OPENING | BOOTSTRAPPING | READY | DEGRADED → DISABLING → DISABLED
DISABLED → REMOVING → REMOVED
OPENING | BOOTSTRAPPING → FAILED
FAILED → DISABLED | REMOVING
```

Rules:

1. Registration is explicit; the daemon SHALL NOT crawl the host or automatically open an unknown repository.
2. `add` validates root authorization and creates the registration record before provider work starts.
3. `relink` requires proof that the new root is the intended source instance. For Git this means matching the stored repository/worktree administrative identity; for non-Git it requires an explicit operator acknowledgement and a source-inventory comparison.
4. `disable` stops watchers and providers, rejects strict-current queries, and retains durable data according to policy.
5. `remove --retain-data` retires the workspace and preserves durable tables/artifacts until normal retention expires. `--purge-data` requires a second explicit confirmation and no active leases.
6. Context/profile changes create a new `registration_revision`, invalidate affected contexts, and trigger a controlled reindex. They never mutate already published snapshots.
7. The query MCP SHALL expose no workspace-add, remove, relink, or configuration tool.

Nested registered roots are allowed only when explicitly registered as separate workspaces. Registration of a child root creates a mandatory exact subtree-boundary exclusion in the parent workspace inventory so the same source bytes are not indexed twice under different workspace identities. Each workspace authorizes and indexes its own root; parent queries do not traverse the child workspace. Removing the child registration does not automatically remove the parent exclusion without an explicit parent reconfiguration and reconciliation.
## AC-G-11 — Root authorization, symlink boundaries, and secure path opening
### Decision

The default profile indexes directory entries beneath the authorized root without following symlinks. A symlink is represented as a filesystem/source-inventory entry but its target is not read as source unless a future profile explicitly enables an internal-target policy.

### Contract

Root authorization stores:

```text
workspace_id
root_path_bytes
root_directory_file_identity
platform_code
case_sensitivity_mode
authorization_revision
authorization_fingerprint
allowed_source_disclosure_rules
```

Every workspace-relative path accepted from a watcher, Git adapter, query boundary, or RPC SHALL pass these checks:

1. not absolute;
2. contains no NUL;
3. contains no empty, `.` or `..` component after lexical splitting;
4. does not contain a platform drive/device prefix;
5. resolves by component-wise directory-handle traversal beneath the authorized root;
6. does not pass through a symlink, magic link, mount escape, or reparse point under the default profile;
7. final file type is a permitted regular file or explicitly represented symlink entry;
8. root directory identity still matches the authorized record.

Linux SHALL use `openat2` with `RESOLVE_BENEATH`, `RESOLVE_NO_MAGICLINKS`, and `RESOLVE_NO_SYMLINKS` where available. The fallback is a component-by-component `openat`/`fstatat` walk using `O_NOFOLLOW`. macOS SHALL use directory-relative opens and `fstat` checks with equivalent no-follow behavior.

The conforming Rust implementation uses the safe descriptor APIs in
`rustix = "=1.1.4"` with feature `fs` (or a separately approved equivalent)
so first-party code retains `unsafe_code = "deny"`. Linux additionally uses
`ResolveFlags::NO_XDEV` for the default no-nested-mount profile. The fallback
and macOS paths compare device and file identity after each descriptor-relative
open. Authoritative source bytes are read only from the resulting owned file
descriptor; gix path reads remain advisory acceleration evidence and are
revalidated before influencing source state.

Directory symlinks are never followed in 1.x. File symlink targets are not parsed in the default profile. A symlink entry may expose its link text as source metadata only if source disclosure authorization allows it.

Mount points beneath the root are denied by default when their device ID differs from the root. Explicitly authorized nested mounts require a separate authorization record and fingerprint.

A root identity or authorization change immediately sets source trust to `VERIFYING`; no strict-current query may complete until revalidation finishes.
## AC-G-24 — Formal freshness state machine and query barrier
### Decision

Freshness is determined by admitted event sequence, source reconciliation sequence, snapshot source generation, and owner-capability generation—not by elapsed time since publication.

### Contract

Each workspace maintains:

```text
admitted_event_sequence          monotonically increasing
reconciled_event_sequence        highest sequence fully reconciled into an active snapshot
current_source_generation        highest verified source image generation
active_snapshot_generation
owner_capability_generation[owner, context, capability]
```

On admission, a query records `barrier_sequence = admitted_event_sequence`. The policy algorithm is:

| Policy | Required condition before pinning |
|---|---|
| `BEST_AVAILABLE_SNAPSHOT` | None beyond one valid snapshot. If `reconciled_event_sequence < barrier_sequence`, freshness is `POTENTIALLY_STALE`. |
| `AWAIT_LATEST` | Active snapshot has `reconciled_event_sequence >= barrier_sequence`; capability gaps may remain explicit. |
| `REQUIRE_SOURCE_CURRENT` | `AWAIT_LATEST` plus source/syntax capabilities current for the request boundary. |
| `REQUIRE_CURRENT_FOR_TARGETS` | Source current; semantic resolution current enough to bind targets; every requested target/capability has generation equal to the active snapshot source generation or an explicit terminal unavailable state. |
| `REQUIRE_SEMANTIC_CURRENT` | Every requested semantic/derived capability is `CURRENT` and complete enough for the query's declared proof needs; otherwise fail. |

State transitions:

```text
clean/current
  -- relevant event admitted --> potentially stale / verifying
  -- authoritative read accepted --> source current, semantic pending
  -- fast lane activated --> source/syntax current
  -- provider/derivation success --> requested capabilities current
  -- provider terminal failure --> source current + capability unavailable
  -- rescan completed --> event stream healthy/current
```

Git degradation does not make source stale after a generic authoritative reconciliation. Watcher overflow immediately requires a rescan and prevents strict-current completion until reconciliation reaches the barrier.

A deadline expiry returns `FRESHNESS_DEADLINE_EXCEEDED` with current barrier/progress metadata. Cancellation aborts waiting and SHALL not pin a new snapshot afterward.

The query pinning order is strictly:

```text
authorize → record barrier → satisfy policy → atomically lease snapshot → resolve/bind/plan/execute
```

`REQUIRE_CURRENT_FOR_TARGETS` uses a candidate-lease loop: atomically lease one candidate snapshot after the source barrier, resolve targets only inside that lease, and inspect owner/capability generations. If any target is still pending, release the candidate without exposing semantic results, wait for the relevant update, and retry until the deadline. The first candidate satisfying all requirements becomes the single final leased snapshot used for binding, planning, execution, response materialization, and artifacts; target resolution is rerun within that same final lease.
## AC-G-25 — Machine-testable lifecycle transition tables
### Decision

Lifecycle state transitions are generated from registry YAML and validated at runtime. Scenario prose is explanatory; transition tables are executable policy.

### Contract

The following independent state machines are mandatory:

```text
WorkspaceLifecycle
SourceTrustState
EventStreamHealth
GitAccelerationStatus
UpdateWaveState
ProviderRunState
OwnerCapabilityState
DurablePublicationState
ServingActivationState
QueryExecutionState
ArtifactState
```

Representative mandatory transitions:

| Machine | From | Event/guard | To | Required action |
|---|---|---|---|---|
| Workspace | `BOOTSTRAPPING` | first valid snapshot activated | `READY` | publish readiness |
| Workspace | `READY` | source inaccessible | `DEGRADED` | preserve last snapshot; strict current unavailable |
| Source trust | `CURRENT` | relevant event admitted | `VERIFYING` | increment barrier sequence |
| Source trust | `VERIFYING` | stable reads and inventory reconciled | `CURRENT` | advance source generation |
| Event stream | `HEALTHY` | overflow/rescan flag | `RESCAN_REQUIRED` | schedule authoritative rescan |
| Wave | `COLLECTING` | gather barrier closes | `SNAPSHOTTING` | freeze path set |
| Wave | any nonterminal | newer wave supersedes and no committed dependents | `SUPERSEDED` | cancel/discard outputs |
| Provider | `QUEUED` | permit granted | `RUNNING` | start deadline |
| Provider | `RUNNING` | terminal manifest valid | `SUCCEEDED` or `PARTIAL` | stage output |
| Publication | `VALIDATED` | pointer lease held | `COMMITTING` | write tables/pointer |
| Snapshot | `READY` | active-pointer CAS succeeds | `ACTIVE` | retire prior snapshot |
| Query | `ACCEPTED` | stream attaches | `RUNNING` | begin barrier/planning |
| Query | `RUNNING` | terminal canonical response committed | `COMPLETE` | release execution resources |

Every transition definition contains `from`, `event`, `guard`, `to`, `actions`, `idempotency_key`, and `error_on_illegal`. Illegal transitions raise `STATE_TRANSITION_VIOLATION`, emit a diagnostic, and fail the affected operation; they are never coerced.

State-machine artifacts SHALL be model-checked for terminal reachability and absence of unintended cycles. Runtime transition records include the machine version and prior/new state codes.
## AC-G-27 — Operational-state persistence
### Decision

The operational-state authority is one SQLite database in WAL mode per daemon repository/worktree group. Delta remains the fact/publication authority. No RocksDB, redb, or independent append journal is used in the mandatory profile.

### Contract

SQLite settings:

```text
journal_mode = WAL
synchronous = FULL
foreign_keys = ON
trusted_schema = OFF
secure_delete = FAST
busy_timeout = 5000 ms
wal_autocheckpoint = 1000 pages
```

The coordinator actor is the sole logical writer for a workspace. Dedicated read connections expose transactionally consistent status/DataFusion snapshots.

Persisted domains include:

- workspace/repository/worktree registration and state;
- generation and admitted/reconciled sequence counters;
- update waves and items at state boundaries;
- provider jobs and terminal manifests;
- dependency graph and fingerprints;
- hot-overlay manifests and active snapshot pointers;
- query/artifact/snapshot leases;
- capability and diagnostic operational indexes;
- credential hashes/revocations and audit records.

High-volume progress events, parser nodes, source bytes, Arrow rows, and query result bytes do not belong in SQLite.

Snapshot activation atomicity is the SQLite transaction in `G-26`. Durable Delta publication may complete before active activation; recovery can retry activation idempotently. Active overlay state is not durably reconstructed unless the optional journal profile is enabled; after crash the daemon rebuilds it from current source.

Schema migrations are numbered, forward-only within a daemon binary, transactional, and preceded by an online backup. A binary SHALL refuse to open an operational schema newer than it supports.
## AC-G-28 — Startup readiness, durable usability, and recovery generations
### Decision

Daemon liveness, workspace readiness, and snapshot freshness are separate statuses. A valid prior durable snapshot may be usable as best-available while startup reconciliation runs, but is never labeled current before verification.

### Contract

Workspace startup states exposed publicly are:

```text
NO_SNAPSHOT
VERIFYING_DURABLE_SNAPSHOT
BEST_AVAILABLE_STALE
REPLAYING_EVENTS
RECONCILING_SOURCE
BUILDING_FIRST_SNAPSHOT
READY_CURRENT
READY_WITH_CAPABILITY_GAPS
BLOCKED
DISABLED
```

A durable snapshot is **usable** only when:

- all required Delta versions and schemas open;
- publication and table checksums verify;
- context and bundle digests are installed and compatible;
- its manifest is internally consistent;
- authorization still permits the workspace.

It is **current** only after:

- root identity and authorization revalidate;
- watcher registration is active or an explicit event-stream-unavailable status is handled;
- downtime source inventory and content digests reconcile;
- admitted startup events through the readiness barrier are incorporated;
- the active snapshot source generation equals the verified current source generation.

Generation rules:

- `source_generation` is persisted and increments once per accepted coherent source image/wave;
- restart never resets it;
- a rebuilt hot overlay uses a new generation even if bytes return to a prior digest;
- provider outputs echo the exact generation and are rejected if stale.

During `BEST_AVAILABLE_STALE`, only `BEST_AVAILABLE_SNAPSHOT` queries may complete. Other freshness policies wait or fail with a precise readiness/freshness error.

Daemon-level readiness is true when the registry and IPC are ready; workspace readiness is per workspace. Health remains true during recoverable reconciliation but false on operational-database corruption, identity collision, or security-boundary failure.
## AC-G-29 — Logical multi-file edit batches and publication barriers
### Decision

The lifecycle groups related mutations into update waves using bounded gather windows. It also supports an optional explicit source-batch coordination API outside MCP for tools that know an atomic edit set.

### Contract

Automatic wave defaults:

```text
ordinary gather quiet period:       75 ms
ordinary maximum gather duration:   500 ms
bulk Git-operation quiet period:    250 ms
bulk maximum gather duration:       2 s after Git metadata stabilizes
maximum paths per ordinary wave:    10,000 before escalation to bulk reconcile
```

A wave freezes its candidate path set at `SNAPSHOTTING`. Events arriving afterward belong to the next generation, even if they concern the same path.

Branch switches, checkout-like bulk filesystem transitions, watcher rescan signals, and context/configuration changes always use a bulk wave and publish no intermediate partial snapshot.

Optional explicit coordination RPC for trusted editor/agent harnesses:

```text
BeginSourceBatch(workspace_id, batch_id, expected_path_ids, deadline)
EndSourceBatch(workspace_id, batch_id)
AbortSourceBatch(workspace_id, batch_id)
```

This API is not exposed through the read-only FastMCP tools. While a batch is open:

- events are admitted and source trust becomes verifying;
- no wave containing expected paths activates before `EndSourceBatch`, abort, or deadline;
- unexpected relevant paths are included in the same batch when observed;
- deadline expiry closes the batch and reconciles all admitted events;
- nested batches are prohibited per workspace;
- an abandoned client cannot hold freshness indefinitely beyond the configured maximum, default 5 seconds.

The active prior snapshot remains queryable under best-available semantics. Strict-current queries wait at the batch barrier.

## AC-G-41 — Operational dependency graph schema and update algorithm
### Decision

The dependency graph is operational control data persisted in SQLite and checkpointed with each durable publication. It is not an ontology claim about software architecture.

### Contract

Canonical edge schema:

```text
workspace_id
analysis_context_id
upstream_kind_code
upstream_id
upstream_fingerprint
downstream_kind_code
downstream_id
downstream_capability_code
edge_kind_code
strength_code: EXACT | CONSERVATIVE | CONFIGURATION
source_generation
active
```

Initial edge kinds:

```text
SOURCE_TO_OWNER
OWNER_TO_PROVIDER_SCOPE
MODULE_IMPORT_DEPENDENCY
BUILD_UNIT_DEPENDENCY
CONFIGURATION_DEPENDENCY
MODEL_PACK_DEPENDENCY
GENERATED_OUTPUT_DEPENDENCY
OWNER_TO_DERIVATION_OWNER
SUMMARY_CALL_DEPENDENCY
PROJECTION_MEMBERSHIP_DEPENDENCY
CONTEXT_SET_DEPENDENCY
```

Update algorithm:

1. dirty source/config nodes seed a sorted worklist;
2. affected source/owner/provider scopes are scheduled;
3. accepted provider output computes source and semantic fingerprints;
4. propagation across a dependency edge stops when the fingerprint relevant to that edge is unchanged;
5. changed fingerprints enqueue downstream owners/capabilities;
6. cycles are processed as SCCs until stable or the precision profile widens;
7. inactive edges are removed by owner replacement, not left as stale dependencies.

Escalation defaults:

```text
>10,000 dirty owners or >10% of context owners  → rebuild affected build unit/context partition
>40% of context owners or context/config ID change → full context rebuild
watcher rescan plus unknown inventory delta        → full workspace inventory reconcile
```

A durable publication stores a canonical checkpoint digest and relational export of active dependency edges. Warm restart loads the checkpoint and applies any current-source delta; it does not reconstruct dependency policy heuristically from fact tables.
## AC-G-62 — Daemon service, configuration, discovery, singleton, and upgrade behavior
### Decision

The daemon is an explicit user service with a stable CLI, TOML configuration, and private runtime discovery file. It does not auto-install, auto-upgrade, or discover arbitrary repositories.

### Contract

Required commands:

```text
codefabricd serve --config <path>
codefabricd check-config --config <path>
codefabric daemon status
codefabric daemon stop
codefabric daemon drain
codefabric contracts verify
codefabric workspace ...
codefabric credentials ...
```

Configuration ownership:

```text
static restart-required:
  storage roots, socket profile, operational DB, bundle/toolchain locations,
  sandbox policy, hard limits, supported platform profile
reloadable:
  log level, telemetry sampling, soft quotas, maintenance schedule
workspace-admin only:
  roots, contexts, source ACL, model packs, trust profile
```

The daemon creates a private discovery file `daemon.json` containing only:

```text
daemon_instance_id
PID and process start token
socket endpoint
RPC major/minor range
basic readiness
startup time
public bundle-version summary
```

No credential, token, workspace root path, source path, or secret appears there. The adapter launcher selects a daemon explicitly or reads this file from the private runtime directory.

Linux default service integration is a systemd user unit; macOS MAY use launchd. Direct foreground execution remains supported. Only one daemon holds the group singleton lock. A second compatible process may run only during an explicit offline migration with a different state/runtime root.

Upgrade procedure is drain-and-restart, not rolling concurrent writers. `drain` rejects new queries/provider waves, lets current work reach terminal states within a deadline, flushes or records the overlay policy, checkpoints SQLite, and exits. Version mismatch during handshake yields actionable compatibility codes.

Liveness means process/IPC/operational DB are functioning. Readiness is per workspace and reported separately.

## Cross-layer integration obligations

The following architecture-completion contracts are owned by another 1.3 artifact but are binding inputs to this specification. This document SHALL consume the named contract and SHALL NOT restate it with different semantics.

| Gap | Contract | Permanent owner | Integration obligation in this document |
|---|---|---|---|
| `G-12` | File identity across replacement, rename, and move | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-18` | Path canonicalization, display, URI, and ordering | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-19` | Complete `ServingSnapshot` manifest schema | [Data-fabric specification 1.3](./present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-20` | Hot-overlay physical schemas and mutation representation | [Data-fabric specification 1.3](./present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-21` | Overlay semantics for owner-scoped, cross-owner, and global tables | [Data-fabric specification 1.3](./present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-22` | Deterministic overlay consolidation, merge, and durable rebase | [Data-fabric specification 1.3](./present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-23` | Snapshot leases, overlay lifetime, result retention, and Delta vacuum | [Data-fabric specification 1.3](./present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-26` | Durable and active current-pointer transaction protocols | [Data-fabric specification 1.3](./present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-30` | Pyrefly sidecar wire protocol | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-31` | rustc extractor protocol | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-32` | Common asynchronous provider execution interface | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-33` | Immutable source snapshot transport | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-34` | Build and project-configuration discovery | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-35` | Provider sandbox and trust model | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-36` | Provider capability granularity and aggregation | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-43` | Unsupported, oversized, binary, generated, and vendored files | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-51` | Multi-context query semantics | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-57` | Query plan cache contract | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-58` | Complete Protobuf service and query state machine | [FastMCP serving specification 1.3](./present_state_cpg_fastmcp_serving_specification_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-59` | Cancellation, acknowledgement, reconnect, and orphan handling | [FastMCP serving specification 1.3](./present_state_cpg_fastmcp_serving_specification_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-60` | Capability credential issuance, binding, rotation, and revocation | [FastMCP serving specification 1.3](./present_state_cpg_fastmcp_serving_specification_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-61` | Local IPC platform and security profile | [FastMCP serving specification 1.3](./present_state_cpg_fastmcp_serving_specification_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |
| `G-68` | Multi-agent fairness, reservations, and starvation guarantees | [FastMCP serving specification 1.3](./present_state_cpg_fastmcp_serving_specification_v1.3.md) | Schedule, fence, activate, recover, and invalidate state according to the owner contract. |

## Release conformance obligations

This specification inherits `G-78` through `G-84` from the suite governance and release manifest. Release acceptance SHALL include the portions of the golden corpus, clean-rebuild comparator, conformance harness, deterministic fault matrix, performance profiles, upgrade choreography, and adversarial security corpus that exercise workspace registration, secure paths, watcher/Git reconciliation, update waves, barriers, operational persistence, crash recovery, fairness, and daemon upgrades.

A passing prose review is insufficient. The corresponding generated registries, schemas, protocol descriptors, fixtures, canonical outputs, and fault oracles SHALL pass the master release gates before an implementation may claim CodeFabric 1.3 conformance.

# Appendix A — Update-Class Decision Guide

```text
event or selected Git metadata change received
  ↓
identify worktree and normalize platform/Git path
  ↓
Git metadata only?
  yes → reread GitStateVector and classify UG0/UG2/UG3/UG4/UG5
  no
  ↓
read authoritative current path state
  ↓
path absent?
  yes → removal
  no
  ↓
digest unchanged?
  yes → path/mode/inclusion/metadata-only or no-op
  no
  ↓
semantic-token fingerprint unchanged?
  yes → U1 source-layout update
  no
  ↓
owner boundary/interface changed?
  no → U2/U3 owner-local
  yes
  ↓
module/type/call contract changed?
  yes → U4/U5
  ↓
manifest/config/build/inclusion context changed?
  yes → U6
  ↓
HEAD tree transition, event loss, mass change, or topology change?
  yes → U7 using:
          tree diff + status + watcher dirty set + current-byte verification
  ↓
provider/schema/derivation/gix adapter version changed?
  yes → U8
```

Rules:

```text
ordinary isolated save:
  do not run full gix status

branch switch:
  prefer HEAD-tree diff + status candidate union

gix unavailable:
  generic authoritative inventory

any uncertainty:
  broaden invalidation or reconcile
```

# Appendix B — Recommended Starting Configuration

These values are starting points to benchmark.

```text
notify-debouncer-full timeout       75 ms
notify tick rate                    20 ms
downstream gather window            20 ms
watch ingress capacity              4,096 events
dirty path bulk threshold           1,000 paths or 10% of worktree
interactive query freshness wait    2 s default
source snapshot retry count         3

gix version                         =0.86.0
gix outer concurrent jobs           1–2 process-wide starting point
gix status/tree timeout             workload-specific bounded deadline
gix rename detection                bulk mode only; bounded candidate count
gix object/pack cache               explicit byte budget
gix network/credentials/hooks       disabled
gix checkout/ref/index writes       disabled

Pyrefly concurrent workspace jobs   1–2
rustc concurrent jobs               1 per worktree, globally bounded
overlay max age                     1–2 s
overlay max rows                    workload benchmark
Delta durable flush                 micro-batched
DataFusion batch size               65,536 starting point
limited memory pool                 mandatory
spill directory                     configured and bounded
```

A large monorepo MAY use one outer gix job with internal `parallel` enabled. A multi-worktree deployment MAY use several outer jobs with reduced internal parallelism. Benchmark both.

# Appendix C — Query Result Example for Non-Compiling Rust

```yaml
snapshot:
  snapshot_id: current-hot-snapshot
  repository_id: common-repository
  worktree_id: linked-worktree-a
  source_generation: 418
  freshness: current_with_unavailable_capabilities
  git_acceleration: current
  git_operation_state: merge
  has_conflict_stages: true

entity:
  representation: source syntax
  kind: Rust function declaration
  qualified_name: crate::parser::parse
  source:
    file: crates/parser/src/lib.rs
    start_line: 84
    end_line: 112

capabilities:
  source: current
  syntax: current
  semantic_definition: unavailable_compile
  types: unavailable_compile
  MIR: unavailable_compile
  ownership: unavailable_compile
  call_targets: unavailable_compile

diagnostics:
  - producer: rustc
    code: E0308
    message: mismatched types
    source_range:
      start_line: 97
      end_line: 97
  - producer: git_state
    code: GIT_CONFLICTED
    message: path has non-zero index conflict stages

source_fallback:
  available: true
  reason: requested semantic facts could not be generated from current source
```

No old MIR or type facts for the invalidated owner appear in this response.

# Appendix D — Clean-Rebuild Equivalence Procedure

```text
1. freeze current worktree source bytes, inclusion policy, and provider versions
2. capture GitRepositoryIdentity, GitWorktreeIdentity, and GitStateVector
3. export the incremental active snapshot
4. create an empty isolated fabric
5. run exact gix-aware worktree discovery and inventory
6. perform authoritative current-byte reads and CodeFabric digesting
7. run complete bootstrap generation
8. canonical-sort all fact tables
9. compare:
     worktree paths and file kinds
     inclusion state
     source digests
     IDs and owner facts
     capabilities and unknowns
     summaries and derived facts
     checksums
10. report every difference by worktree/owner/fact family
11. repeat with gix acceleration disabled and generic inventory
12. require both clean builds to produce the same semantic CPG
```

This procedure is the final correctness oracle for continuous updating.

# Appendix E — Recommended Read-Only gix Dependency Profile

```toml
[dependencies]
gix = {
  version = "=0.86.0",
  default-features = false,
  features = [
    "sha1",
    "index",
    "status",
    "attributes",
    "excludes",
    "dirwalk",
    "blob-diff",
    "interrupt",
    "parallel",
    "auto-chain-error",
    "tracing"
  ]
}
```

Implementation notes:

- at least one hash feature is required;
- `sha256` MAY be enabled for compatibility testing but does not imply complete SHA-256/reftable parity;
- exact transitive feature relationships SHALL be checked against the released manifest;
- the application policy forbids command, credential, network, checkout, ref-write, and index-write behavior even if a transitive crate exposes the underlying capability;
- the broad default gix bundle SHOULD be avoided for the lifecycle daemon.

The dependency shall be isolated inside the application-owned `git_state` module.

# Appendix F — Core Git-State DTOs

```rust
pub struct GitRepoPath {
    pub raw: std::sync::Arc<[u8]>,
    pub display: String,
    pub display_is_lossy: bool,
}

pub struct PlatformPath {
    pub native: std::ffi::OsString,
    pub workspace_relative_bytes: std::sync::Arc<[u8]>,
    pub comparison_key: std::sync::Arc<[u8]>,
    pub encoding_code: PathEncodingCode,
    pub display: String,
    pub display_is_lossy: bool,
}

pub struct GitObjectId {
    pub algorithm: GitHashAlgorithm,
    pub bytes: Vec<u8>,
}

pub struct GitStateSnapshot {
    pub identity: GitWorktreeIdentity,
    pub vector: GitStateVector,
    pub acceleration: GitAccelerationStatus,
}

pub struct GitCandidateDelta {
    pub baseline: GitStateVector,
    pub candidate_paths: Vec<GitPathCandidate>,
    pub conflicted_paths: Vec<GitRepoPath>,
    pub submodule_changes: Vec<SubmoduleCandidate>,
    pub rename_candidates: Vec<GitRenameCandidate>,
    pub requires_full_inventory: bool,
}

pub struct GitPathCandidate {
    pub repo_path: GitRepoPath,
    pub platform_path: PlatformPath,
    pub class: GitPathChangeClass,
    pub tracked: bool,
    pub ignored: bool,
    pub staged: bool,
    pub worktree_modified: bool,
    pub object_id: Option<GitObjectId>,
    pub mode: Option<u32>,
}

pub struct GitRenameCandidate {
    pub from: GitRepoPath,
    pub to: GitRepoPath,
    pub similarity: Option<f32>,
    pub source: GitRenameEvidence,
}
```

All DTOs are application-owned. No attached gix handle, reference, object, index, or repository value is persisted or shared through these types.
