# CodeFabric Continuous CPG Update and Lifecycle Management Specification

**Status:** Draft normative implementation specification  
**Specification date:** 2026-08-19  
**Primary implementation language:** Rust  
**Primary runtime topology:** One central CodeFabric daemon per workspace or workspace group; one FastMCP STDIO coordination process per programming agent  
**Primary change source:** `notify-debouncer-full` over `notify`  
**Primary concurrency stack:** Tokio, Rayon, Crossbeam, DashMap, optional `tokio-rayon`  
**Primary durable data plane:** Arrow, DataFusion, Delta Lake / delta-rs  
**Companion specifications:**

- `code_property_graph_present_state_fact_ontology_specification.md`
- `present_state_cpg_fact_generation_specification_python_rust.md`
- `present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake.md`
- `code_property_graph_semantic_query_specification.md`

---

## 1. Purpose

This document specifies how CodeFabric continuously maintains a coherent, current, queryable Code Property Graph while source files are being created, edited, renamed, deleted, temporarily broken, reformatted, regenerated, or changed concurrently by multiple programming agents.

The design SHALL:

- detect relevant workspace changes rapidly;
- convert watcher output into authoritative source-state reconciliation rather than blindly replaying filesystem events;
- update only the smallest sound invalidation domain;
- preserve query availability for unaffected facts;
- explicitly withdraw facts that are no longer justified by current source;
- support current syntax even while semantic providers fail;
- prevent partially generated or cross-generation facts from becoming visible;
- provide atomic query snapshots;
- prioritize interactive freshness;
- use bounded concurrency and backpressure;
- recover from watcher loss, provider failure, daemon crash, and partial durable commits;
- converge to the same CPG as a clean rebuild for the same source state.

The core objective is:

> **An agent querying immediately after an edit receives either current facts or an explicit capability gap—never silently stale facts presented as current.**

---

## 2. Source basis

This specification uses the attached references as its technical basis.

| Reference | Relevant design contribution |
|---|---|
| `notify_debouncer_full_rust_reference.md` | Debounce semantics, event normalization, rename stitching, rescan handling, watcher lifecycle, bounded handoff, authoritative reconciliation, shutdown, and CPG integration |
| `rust_parallel_concurrency_stack_reference_2026-08-19.md` | Tokio/Rayon/Crossbeam/DashMap role separation, process-wide thread budgeting, bounded admission, cancellation, backpressure, incremental-indexer architecture, testing, and deployment |
| `rust_mir_cpg_continuous_reference_2026-08-18.md` | Two-speed syntax/semantic pipeline, rustc incremental reuse, owner fingerprints, compile-failure handling, compiler extraction manifests, subgraph replacement, and recovery |
| `present_state_cpg_fact_generation_specification_python_rust.md` | Fact ownership, capability status, provider authority, dependency order, unknown materialization, and owner-scoped publication |
| `present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake.md` | Owner replacement, multi-table MVCC, publication manifests, DataFusion reconciliation, durable validation, query-snapshot pinning, and Delta recovery |
| `code_property_graph_semantic_query_specification.md` | One atomically consistent query snapshot, explicit unavailable fact families, current-state semantics, and non-ambiguous empty-result handling |

Where this document adds mechanisms not directly specified by those references—most importantly the in-memory hot overlay—it does so as an architectural inference required to meet the stated interactive-latency target.

---

## 3. Scope

### 3.1 Included

This specification covers:

- daemon bootstrap and warm restart;
- watcher registration and readiness;
- filesystem event ingestion;
- debounce and downstream coalescing;
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
- provider crashes and timeouts;
- queue overflow and resource saturation;
- storage conflicts and partial commits;
- graceful and forced shutdown;
- testing, observability, and performance policy.

### 3.2 Excluded

This specification does not introduce:

- Git-history analysis;
- prior-code-state querying;
- runtime observation;
- test-impact conclusions;
- refactor-safety conclusions;
- risk scoring;
- recommendations;
- unsaved editor-buffer overlays unless supplied through a future explicit overlay API.

Filesystem source bytes remain the present-state source of truth.

---

# Part I — Lifecycle and Scenario Inventory

## 4. Lifecycle phases

Every workspace passes through the following lifecycle phases.

```text
UNINITIALIZED
    ↓
WATCH_REGISTERING
    ↓
BOOTSTRAPPING or WARM_RECOVERING
    ↓
READY
    ↓
COLLECTING_CHANGES
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

Exceptional transitions may enter:

```text
RECONCILING
DEGRADED
BLOCKED
STOPPING
FAILED
```

The daemon SHALL continue serving the newest valid immutable snapshot whenever doing so does not misrepresent freshness.

---

## 5. Startup and recovery scenarios

### 5.1 Cold start without an index

**Trigger:** No valid durable publication exists.

**Required behavior:**

1. register watcher before or concurrently with inventory;
2. start an event-generation counter;
3. enumerate included source files;
4. capture content digests;
5. perform complete fast and semantic generation;
6. derive required facts;
7. validate;
8. create first hot snapshot;
9. durably publish;
10. replay events received during bootstrap;
11. mark workspace `READY`.

Queries before first valid snapshot SHALL return `WORKSPACE_BOOTSTRAPPING`.

### 5.2 Warm start with unchanged source

**Trigger:** Durable publication exists and current source inventory matches its digest.

**Required behavior:**

- open publication-pinned DataFusion catalog;
- construct immutable serving snapshot;
- replay watcher events received during verification;
- skip regeneration;
- transition to `READY`.

### 5.3 Warm start with source changes made while daemon was stopped

**Trigger:** Durable source inventory differs from filesystem inventory.

**Required behavior:**

- compute a digest-based corrective delta;
- classify added, removed, changed, and moved files;
- run the ordinary incremental pipeline over that delta;
- do not assume watcher events exist for downtime changes.

### 5.4 Warm start with orphan staging publications

**Trigger:** Delta contains `STAGING`, incomplete, or failed publications not referenced by `current_publication`.

**Required behavior:**

- preserve current pointer;
- inspect operation IDs and checksums;
- resume only idempotent known-safe work;
- otherwise mark abandoned;
- reconcile current source independently;
- schedule cleanup after pinned-version safety checks.

### 5.5 Warm start after crash with unflushed hot overlay

**Trigger:** Source files are newer than durable publication and no durable overlay journal exists.

**Required behavior:**

- rebuild from current source;
- never assume lost in-memory overlay contents;
- keep old durable publication queryable only with `potentially_stale` status until reconciliation completes;
- strict-current queries SHALL wait or fail explicitly.

---

## 6. Routine file-operation scenarios

### 6.1 Isolated source modification

- mark path dirty;
- read current bytes;
- compute digest;
- classify update;
- invalidate only affected owners and dependent fact families;
- supersede older work for the same path/owner.

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

- treat watcher event sequence as an invalidation hint;
- stat and read the final target path;
- compare content digest;
- preserve file identity only when rename/file-ID/content evidence supports it;
- never apply raw delete/create semantics directly to graph facts.

### 6.4 File creation

- add source-file facts;
- discover semantic owners;
- generate source, syntax, semantic, and derived facts;
- invalidate import/module topology where relevant;
- create cross-owner facts owned by affected callers/importers.

### 6.5 File deletion

- tombstone the file owner and all descendant owners;
- remove owner-scoped facts;
- invalidate incoming references, imports, and call-target facts as required;
- materialize explicit unknown targets/modules when unresolved references remain.

### 6.6 File rename within the same logical module

- verify final content;
- preserve file-instance identity when supported;
- update path facts and source spans;
- retain semantic identities only when module/qualified-name identity is unchanged;
- recompute path-sensitive imports and source correspondence.

### 6.7 File move changing module/package identity

- treat as more than a path rename;
- invalidate module identity, imports, exports, qualified names, semantic IDs, and dependents;
- preserve content lineage only as an internal optimization;
- publish new present-state semantic identity.

### 6.8 Directory rename

- apply prefix-aware source inventory update;
- verify descendants;
- recompute path/module identities;
- broaden invalidation when package/module resolution changes.

### 6.9 Directory deletion

A single parent removal may stand in for many child events.

Required behavior:

- enumerate indexed descendants under normalized prefix;
- delete all affected owners;
- recompute dependency and derived facts;
- validate no dangling relation endpoints.

### 6.10 Metadata-only event

- stat file;
- if content digest unchanged and policy does not make metadata semantic, emit no CPG update;
- retain operational timestamp only outside the semantic CPG.

### 6.11 Content digest unchanged

- classify as `NO_OP`;
- do not parse;
- do not publish a new semantic snapshot unless path or metadata facts changed.

### 6.12 Whitespace/comment-only change

When a semantic-token fingerprint proves semantics unchanged:

- update source, token, comment, syntax, and span facts;
- reanchor semantic facts through exact structural mapping;
- avoid compiler/type reanalysis when safe;
- invalidate documentation/directive semantics if changed.

Python indentation, type comments, and semantic directives SHALL prevent unsafe classification as trivia-only.

### 6.13 Formatter change across many files

- detect high path count and high semantic-token reuse;
- use parallel hashing and fast syntax remapping;
- avoid one Delta publication per file;
- create one or a few multi-owner hot snapshots;
- micro-batch durable flush.

### 6.14 Generated-source burst

- exclude generated outputs that are not part of the analyzed source model;
- for included generated source, use bulk-reconcile mode;
- prevent output→watch→generator feedback loops.

### 6.15 File extension or language change

- remove old language-profile owners;
- create new language-profile owners;
- recompute module/dependency topology;
- never reinterpret old provider facts under the new language.

### 6.16 Symlink creation, deletion, or retargeting

- apply configured symlink policy;
- authorize resolved target;
- treat target change as potential subtree replacement;
- avoid lexical-prefix-only trust decisions.

### 6.17 Permission or transient read failure

- keep file marked dirty;
- invalidate facts that cannot be justified;
- retry with bounded backoff;
- report `SOURCE_UNREADABLE`;
- never preserve old facts as current merely because read failed.

### 6.18 File changes while being read

- compare metadata and digest before/after read;
- retry until a stable source image is captured or deadline is reached;
- never analyze torn bytes.

### 6.19 Oversized or binary-like source file

- mark source capability `EXCLUDED_BY_LIMIT` or `UNSUPPORTED_CONTENT`;
- expose path, size, and diagnostic;
- avoid parsing;
- do not claim absence of semantic facts.

---

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

### 9.1 Branch switch, checkout, or large patch application

The watcher may emit thousands of events without a transaction boundary.

Required behavior:

- enter `BULK_RECONCILE` when thresholds are exceeded;
- cancel or supersede per-file semantic jobs;
- inventory the workspace authoritatively;
- compute digest delta in parallel;
- generate one update wave;
- prioritize active/query-target files first;
- converge the remainder in background.

### 9.2 Watcher event loss or overflow

On `need_rescan()` or equivalent completeness-threatening error:

- set source trust state to `UNVERIFIED`;
- enter reconciliation generation;
- coalesce duplicate rescan signals;
- inventory watched roots;
- compute minimal corrective delta;
- fence events arriving during reconciliation;
- publish only after the corrected source snapshot is coherent.

### 9.3 Watcher backend failure

- mark watcher health degraded;
- continue serving pinned snapshot with freshness warning;
- retry registration or switch to polling according to policy;
- run authoritative reconciliation after recovery.

### 9.4 Watched root deletion

- mark all contained source owners removed after verification;
- attempt rewatch of parent/root according to policy;
- do not assume watcher will continue receiving descendants.

### 9.5 Difficult/network filesystem

- validate native watcher behavior at deployment;
- use `PollWatcher` when required;
- separate poll interval from debounce interval;
- cap poll amplification and content comparison.

### 9.6 Event ingress queue saturation

The watcher callback SHALL NOT block indefinitely.

Recommended policy:

1. attempt bounded nonblocking enqueue;
2. if full, set `reconcile_required` atomically;
3. increment dropped-to-reconcile metric;
4. discard individual event details;
5. schedule authoritative reconciliation.

This converts overflow into bounded loss with deterministic recovery instead of unbounded memory growth.

---

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
- or return `CURRENT_FACTS_NOT_YET_AVAILABLE`.

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

Every dirty path or configuration input SHALL be classified into one update class after current-state verification.

| Code | Update class | Typical trigger | Minimum action |
|---|---|---|---|
| `U0` | No-op | Event but digest unchanged | No graph update |
| `U1` | Source-layout only | Line endings, comments, whitespace with semantic token equivalence | Source/token/syntax/span remap |
| `U2` | File-local syntax | Expression/statement edit with no declaration interface change | File syntax + owner-local analysis |
| `U3` | Owner-local semantics | Function body, local binding, local type flow | Owner semantic/CFG/dataflow replacement |
| `U4` | Module/type interface | Signature, class member, trait/impl, import/export | Module/type and direct dependents |
| `U5` | Interprocedural semantic | Dispatch set, callable contract, public type relation | Reverse dependency and summary propagation |
| `U6` | Compilation unit/project | Cargo/Python project config, macro/build context | Affected project or crate graph |
| `U7` | Workspace reconciliation | Event loss, bulk switch, inclusion-policy change | Authoritative inventory delta |
| `U8` | Fabric/provider migration | Schema, ontology, provider or derivation version | Controlled regeneration/migration |

Classification is an optimization. When uncertain, the planner SHALL broaden invalidation.

---

## 14. Invalidation dimensions

Invalidation is not one Boolean. It is a set over:

```text
owner
fact family
representation
provider capability
derived projection
```

Example:

```text
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

### 14.2 Invalidation result

```rust
pub struct InvalidationPlan {
    pub source_generation: u64,
    pub changed_files: Vec<FileId>,
    pub changed_owners: Vec<OwnerId>,
    pub capability_withdrawals: Vec<CapabilityWithdrawal>,
    pub owner_replacements: Vec<OwnerReplacementPlan>,
    pub dependent_owners: Vec<OwnerId>,
    pub derived_scopes: Vec<DerivedScope>,
    pub build_units: Vec<BuildUnitId>,
    pub bulk_reconcile: bool,
}
```

---

## 15. Ownership and relation invalidation

Facts SHALL have deterministic replacement owners.

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

---

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
```

Propagation algorithm:

1. seed changed owners/families;
2. traverse only relationship categories affected by update class;
3. add dependent owner/family pairs;
4. stop when a dependent fingerprint is unchanged;
5. escalate to component/project rebuild at threshold.

---

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
WATCH_REGISTERING
BOOTSTRAPPING
WARM_RECOVERING
READY
UPDATING_FAST
UPDATING_SEMANTIC
RECONCILING
DEGRADED
BLOCKED
STOPPING
FAILED
```

Only one coordinator task SHALL mutate workspace lifecycle state.

---

## 19. Source trust state

```text
TRUSTED
EVENT_LOSS_SUSPECTED
RECONCILE_REQUESTED
RECONCILING
UNVERIFIED
STABLE
```

Semantics:

- `TRUSTED`: watcher plus inventory watermark support currentness.
- `EVENT_LOSS_SUSPECTED`: ordinary event sequence is not sufficient.
- `RECONCILING`: authoritative inventory is in progress.
- `UNVERIFIED`: current source may differ from active snapshot.
- `STABLE`: reconcile scan and post-scan event replay completed.

---

## 20. Update-wave state

```text
COLLECTING
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

A wave is immutable after `HOT_PUBLISHED`.

---

## 21. Provider-run state

```text
QUEUED
RUNNING
SUCCEEDED
SUCCEEDED_PARTIAL
FAILED
TIMED_OUT
CANCELLED
CRASHED
PROTOCOL_ERROR
STALE_RESULT
```

Every provider output SHALL include the source digest and generation it analyzed.

---

## 22. Owner capability state

Recommended current-state values:

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

Each status SHALL include:

```text
owner_id
capability_code
source_generation
reason_code
diagnostic_id
fallback_source_available
```

---

## 23. Publication state

```text
STAGING
BASE_VALIDATED
DERIVED_VALIDATED
HOT_ACTIVE
DURABLE_COMPLETE
DURABLE_ACTIVE
FAILED
ABANDONED
```

Hot and durable activation are distinct.

---

## 24. Query freshness state

```text
CURRENT
CURRENT_WITH_UNAVAILABLE_CAPABILITIES
UPDATE_PENDING
SOURCE_UNVERIFIED
POTENTIALLY_STALE
WORKSPACE_BOOTSTRAPPING
WORKSPACE_BLOCKED
```

This status is delivery metadata, not a CPG semantic fact.

---

## 25. Core Rust state structures

```rust
pub struct WorkspaceCoordinatorState {
    pub lifecycle: WorkspaceLifecycleState,
    pub source_trust: SourceTrustState,
    pub next_event_seq: u64,
    pub newest_dirty_generation: u64,
    pub active_snapshot: std::sync::Arc<ServingSnapshot>,
    pub active_wave: Option<UpdateWaveId>,
    pub reconcile_required: bool,
    pub durable_lag_generations: u64,
}

pub struct DirtyEntry {
    pub path: std::path::PathBuf,
    pub first_event_seq: u64,
    pub latest_event_seq: u64,
    pub observed_kinds: DirtyKindMask,
    pub latest_known_digest: Option<[u8; 32]>,
    pub priority: WorkPriority,
    pub rescan_required: bool,
}

pub struct UpdateWave {
    pub wave_id: UpdateWaveId,
    pub source_generation: u64,
    pub watermark_event_seq: u64,
    pub source_inventory_digest: [u8; 32],
    pub dirty_paths: Vec<std::path::PathBuf>,
    pub invalidation: InvalidationPlan,
    pub state: UpdateWaveState,
}
```

---

# Part IV — Watcher, Event Ingestion, and Source Snapshotting

## 26. Watcher role

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
graph mutation engine
```

The handler SHALL perform only:

- event classification;
- event sequencing;
- cheap path normalization;
- bounded enqueue or reconcile escalation.

It SHALL NOT parse, compile, hash large files, or mutate the CPG.

---

## 27. Debounce policy

Recommended starting profile for interactive local indexing:

```text
debounce timeout: 50–100 ms
tick rate:        10–25 ms, and never greater than timeout
gather window:    10–25 ms after handler delivery
```

The filesystem debounce and downstream gather window are independent.

Continuous writes may yield periodic eligible batches; the design SHALL therefore rely on generation supersession rather than assuming one trailing-edge event.

---

## 28. Application event facade

```rust
pub enum WatchChange {
    DirtyPath {
        path: std::path::PathBuf,
        seq: u64,
    },
    RemovedPath {
        path: std::path::PathBuf,
        seq: u64,
    },
    Renamed {
        from: std::path::PathBuf,
        to: std::path::PathBuf,
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
```

Fine-grained `notify::EventKind` SHALL not leak into downstream graph logic.

---

## 29. Bounded ingress and overflow recovery

Recommended implementation:

```text
watch handler
  → try_send into bounded Tokio channel
      success → coordinator receives event
      full    → set reconcile_required flag; increment metric
```

The event callback must not block behind parsing or storage.

The bounded queue is a latency and memory contract.

---

## 30. Dirty registry

The coordinator owns a map:

```text
normalized path → latest DirtyEntry
```

Repeated events update one entry rather than enqueueing unlimited work.

The map SHOULD be actor-owned. DashMap is optional for read-heavy auxiliary access but not required for the authoritative mutation path.

---

## 31. Bulk-mode thresholds

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
```

Thresholds are workload configuration, not ontology facts.

---

## 32. Source image capture

Each file analysis SHALL consume an immutable `SourceImage`.

```rust
pub struct SourceImage {
    pub file_id: FileId,
    pub path: std::path::PathBuf,
    pub bytes: std::sync::Arc<[u8]>,
    pub digest: [u8; 32],
    pub size: u64,
    pub read_generation: u64,
}
```

Capture algorithm:

1. read metadata;
2. open and read bytes;
3. compute digest;
4. reread metadata;
5. verify stable size/identity;
6. if changed, retry or defer;
7. publish source image only when stable.

---

## 33. Source inventory

Maintain a current operational inventory:

```text
normalized path
file identity if available
content digest
size
language classification
inclusion state
current file owner
```

A Merkle-style directory/root digest SHOULD be maintained so one file update changes only the affected path and ancestor hashes.

This makes warm-start and reconciliation comparison efficient.

---

## 34. Rename policy

A matched rename is an optimization, not proof.

Processing:

1. resolve new path state;
2. verify content digest;
3. evaluate whether language/module identity changed;
4. preserve file-instance identity when justified;
5. invalidate semantic identities if qualified path changed;
6. otherwise treat as remove+add.

Filesystem file IDs and watcher tracker IDs SHALL NOT be canonical CPG IDs.

---

## 35. Rescan generation fence

Rescan algorithm:

```text
record event watermark W0
mark source trust UNVERIFIED
inventory all included files
record watermark W1
compute delta against indexed inventory
process events with seq > W0
reverify paths touched during inventory
apply corrective wave
mark source trust TRUSTED
```

Events during scan are not discarded; they become `dirty_after_reconcile`.

Strict-current queries SHALL not claim source completeness while trust is `UNVERIFIED`.

---

# Part V — Update Pipeline

## 36. Pipeline overview

```text
watch events
    ↓
dirty registry
    ↓
update wave
    ↓
source images
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

---

## 37. Fast syntax lane

### 37.1 Purpose

Provide current source navigation and explicit syntax gaps as quickly as possible.

### 37.2 Work

- current source facts;
- Tree-sitter incremental parse;
- parse errors and missing syntax;
- token/comment/trivia extraction;
- likely owner boundaries;
- source-to-owner mapping;
- capability withdrawals for invalidated semantic facts.

### 37.3 Publication

The daemon MAY publish a syntax-current snapshot before semantic providers finish.

This snapshot SHALL:

- include current source/syntax;
- remove invalidated semantic facts from visibility;
- retain unaffected semantic owners;
- mark pending/unavailable capabilities;
- never present stale invalidated facts as current.

---

## 38. Python semantic lane

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

## 39. Rust semantic lane

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

### 39.1 Compiler manifest rule

A rustc extraction run SHALL emit:

```text
BEGIN invocation
OWNER_COMPLETE owner...
END invocation with source/build digest and owner manifest
```

No owner facts are publishable without a valid manifest policy.

### 39.2 Partial compilation policy

Default:

- compile failure does not produce a fresh compiler generation;
- invalidated owners remain semantically unavailable;
- unchanged/unaffected owners remain current only when dependency validity is established;
- last-known-good compiler rows may remain in hidden operational cache but SHALL not be visible as present-state facts for invalidated owners.

### 39.3 Query fallback on compile failure

When a query targets unavailable Rust semantics, response SHALL include:

```text
fact_family_status: unavailable
reason: compile_error | extractor_failure | pending
current source location
current source context
compiler diagnostics
affected owner/build unit
```

The MCP layer SHOULD make the current source directly available in the same logical response.

---

## 40. Owner-local derived lane

Recompute immediately for changed owners:

- CFG normalization;
- dominators/post-dominators;
- control dependence;
- loops;
- value/access events;
- reaching definitions;
- liveness;
- owner-local alias/points-to;
- structural metrics;
- direct effects;
- direct callable summary.

These tasks are embarrassingly parallel across independent owners.

---

## 41. Interprocedural derived lane

### 41.1 Call graph

Update direct edges for changed caller owners.

### 41.2 Summary propagation

Use reverse call dependencies:

1. enqueue changed callable summaries;
2. propagate to callers;
3. recompute caller summary;
4. continue only when summary hash changes;
5. process recursive SCCs to fixed point.

### 41.3 SCC updates

Preferred policy:

- recompute SCCs for affected weakly connected or condensation region;
- fall back to whole graph when affected region exceeds threshold;
- never publish partially updated SCC assignments.

### 41.4 Reachability

Prefer query-time traversal.

Materialize only bounded frequently used closure.

### 41.5 Alias/points-to

Recompute at the narrowest sound scope.

If constraints cross owner/module boundary, propagate through the relevant component.

### 41.6 Fixed-point limits

If iteration, memory, or time limits are reached:

- mark derived capability unavailable;
- retain base facts;
- do not publish incomplete fixed-point results as complete.

---

## 42. Validation stages

### 42.1 Fast validation

Before syntax-current publication:

- source digest matches image;
- Arrow schemas validate;
- source spans are in bounds;
- owner IDs are deterministic;
- capability withdrawals are complete;
- no stale generation result included.

### 42.2 Owner semantic validation

- provider manifest complete;
- fact primary keys unique within owner;
- relation endpoints exist or point to explicit unknown;
- CFG entry/exit and edges are valid;
- def-use endpoints are valid;
- source correspondence uses current digest.

### 42.3 Affected-component validation

- cross-owner reference and call endpoints;
- SCC assignment consistency;
- summary dependency generation;
- derived fixed-point convergence.

### 42.4 Durable publication validation

- Delta row counts/checksums;
- cross-table endpoint checks;
- publication table completeness;
- schema fingerprints;
- pinned version integrity.

Whole-repository validation SHOULD run periodically and after bulk reconcile, not on every one-line edit.

---

# Part VI — Atomicity and Serving Snapshot Design

## 43. Atomicity model

The system SHALL define six distinct atomicity boundaries.

### 43.1 Source-image atomicity

One file is analyzed from one stable byte image.

### 43.2 Owner-batch atomicity

All facts owned by one owner/fact-family replacement become visible together.

### 43.3 Hot-snapshot atomicity

All table overlays, capability withdrawals, and derived facts in a hot generation become visible through one pointer swap.

### 43.4 Durable table atomicity

Each Delta table commit is individually atomic.

### 43.5 Durable multi-table atomicity

Publication manifest pins exact table versions; `current_publication` changes last.

### 43.6 Query atomicity

A query pins one immutable serving snapshot for its entire execution.

The filesystem itself does not provide multi-file transaction atomicity. CodeFabric approximates logical edit batches with debounce, gather windows, explicit barriers, and source-generation watermarks.

---

## 44. Why a hot overlay is required

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

## 45. Serving snapshot

```rust
pub struct ServingSnapshot {
    pub snapshot_id: SnapshotId,
    pub source_generation: u64,
    pub source_inventory_digest: [u8; 32],
    pub durable_base_publication: PublicationId,
    pub overlay_generation: u64,
    pub tables: SnapshotTableSet,
    pub capability_index: CapabilityIndex,
    pub diagnostics: DiagnosticIndex,
    pub freshness: SnapshotFreshness,
}
```

The object is immutable and shared by `Arc`.

---

## 46. Hot overlay contents

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

## 47. Consolidated overlay rule

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

## 48. Overlay-aware DataFusion providers

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

## 49. Atomic pointer swap

The coordinator constructs the complete new `ServingSnapshot` off-path.

Activation:

1. validate snapshot;
2. publish snapshot through `tokio::sync::watch<Arc<ServingSnapshot>>` or a tiny `RwLock<Arc<_>>`;
3. update workspace status;
4. notify waiters.

Query handlers clone the `Arc`.

No long-running query holds a global graph lock.

---

## 50. Durable overlay flush

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

## 51. Crash recovery policy

### 51.1 Default local interactive mode

The hot overlay need not be fsynced per edit because source files are authoritative.

After crash, rebuild missing current changes from source.

### 51.2 Optional durable overlay journal

A service requiring faster crash recovery MAY append compact Arrow IPC change manifests before hot activation.

The journal stores operational update inputs, not semantic history.

### 51.3 Invariant

A lost hot overlay may cause temporary durable lag after restart, but SHALL never cause a false current semantic claim.

---

# Part VII — Scheduling, Parallelism, and Backpressure

## 52. Runtime responsibility split

### 52.1 Tokio

Use for:

- watcher channel and coordinator;
- timers/gather windows;
- filesystem and object-store I/O;
- sidecar and compiler-process orchestration;
- query RPC/daemon protocol;
- freshness waiters;
- publication orchestration;
- shutdown.

### 52.2 Rayon

Use for:

- hashing large batches;
- Tree-sitter/Ruff parsing across files;
- normalization;
- owner-local CFG/dataflow;
- graph projection construction;
- per-owner derivation;
- parallel encoding of Arrow batches.

### 52.3 Crossbeam

Use only where a synchronous CPU pipeline benefits from:

- bounded MPMC channels;
- work-stealing deques;
- specialized low-level queues.

Do not build a custom scheduler when Rayon suffices.

### 52.4 DashMap

Use selectively for:

- content-addressed immutable caches;
- provider-result caches;
- workspace lookup registry;
- query-plan cache.

Do not use DashMap as the central update transaction manager.

Never hold DashMap guards across `.await` or nested map operations.

### 52.5 `tokio-rayon`

May bridge Tokio orchestration to Rayon jobs.

It is not admission control; pair it with semaphores and generation checks.

---

## 53. Actor-owned coordinator

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

## 54. Work priorities

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

## 55. Admission control

Maintain process-wide limits for:

```text
concurrent source reads
Rayon CPU work
concurrent Python semantic jobs
concurrent rustc invocations
DataFusion derivations
Delta writers
query executions
```

Rust compiler concurrency SHOULD normally be low because rustc consumes substantial CPU and memory and uses its own internal parallelism.

---

## 56. Thread-budget policy

All pools share one hardware budget.

Recommended starting approach:

```text
Tokio workers:
  small I/O/orchestration pool, not one worker per logical CPU by default

Interactive Rayon pool:
  reserved for small latency-sensitive tasks

Bulk Rayon pool:
  bounded so interactive + bulk threads do not exceed CPU budget

rustc processes:
  one or a small number per workspace/process

DataFusion target partitions:
  workload-adjusted and coordinated with the same budget
```

Exact values SHALL be benchmarked on target hardware.

---

## 57. Supersession and cancellation

### 57.1 Generation rule

Every work item carries:

```text
workspace generation
file digest
owner generation
provider context
```

Before accepting output:

```text
if result generation != latest required generation:
    mark STALE_RESULT
    discard
```

### 57.2 Cooperative cancellation

Rayon/custom loops SHOULD check a generation or cancellation atomic at bounded intervals.

### 57.3 rustc process cancellation

Policy:

- terminate early superseded invocations when substantial cost remains;
- allow near-complete invocation to finish and discard if stale;
- never let a stale invocation publish.

### 57.4 DataFusion cancellation

Custom operators SHALL support cancellation and memory limits.

---

## 58. Backpressure policy

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

## 59. Cache policy

Recommended caches:

```text
content digest → parsed syntax facts
semantic owner fingerprint → normalized owner batch
dependency tuple → external summary
CFG fingerprint → derived control facts
call graph component fingerprint → SCC result
summary input hash → callable summary
DataFusion PlanSpec + snapshot schema hash → logical plan
```

Cache entries SHALL be immutable and versioned by provider/schema/derivation bundle.

---

# Part VIII — Failure Taxonomy and Recovery

## 60. Failure classes

### 60.1 Watcher failures

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

### 60.2 Source failures

```text
file disappears during read
permission denied
unstable/torn read
invalid encoding
oversized input
symlink escape
path normalization failure
```

### 60.3 Syntax/provider failures

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

### 60.4 Reconciliation failures

```text
span mismatch
semantic identity collision
duplicate primary key
provider conflict
missing endpoint
schema mismatch
unknown enum code
incomplete owner manifest
```

### 60.5 Derived-analysis failures

```text
fixed-point nonconvergence
iteration limit
memory limit
spill failure
cancellation
graph invariant failure
algorithm panic
```

### 60.6 Storage/publication failures

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
```

### 60.7 Query failures

```text
snapshot unavailable
strict freshness timeout
fact family unavailable
workspace source unverified
query cancellation
query memory limit
```

---

## 61. Failure-policy matrix

| Failure | Current source/syntax | Semantic facts | Query behavior | Recovery |
|---|---|---|---|---|
| Python parse error | current via Tree-sitter | affected families unavailable | source + diagnostics + gap | next edit/provider recovery |
| Pyrefly failure | current | local facts only; project types/calls unavailable | partial facts, no negative claims | restart/backoff |
| Rust compile error | current via Tree-sitter | invalidated compiler facts unavailable | source + compiler diagnostics | next successful compile |
| rustc extractor crash | current | affected compilation unit unavailable | provider failure | quarantine/restart |
| Event loss | active snapshot potentially stale | unchanged snapshot only | strict queries wait/fail | authoritative reconcile |
| Queue overflow | active snapshot potentially stale | no blind incremental claim | mark reconcile pending | bulk reconcile |
| Derived timeout | base facts current | derived family unavailable | return base facts and gap | retry/broader resources |
| Delta conflict | hot snapshot current | current hot facts | durable lag only | idempotent retry |
| Hot snapshot validation fail | old snapshot current | old snapshot current | no new snapshot | retry/reconcile |
| Disk full | active snapshot current | hot overlay may continue within memory budget | durable lag/block status | operator action/cleanup |
| Daemon crash | durable publication only | rebuilt from source | bootstrap/warm recovery | reconcile |

---

## 62. Retry and backoff

Retryable:

```text
transient file lock/read
sidecar unavailable
object-store timeout
Delta conflict
polling/backend re-registration
```

Non-retryable without input change:

```text
schema/protocol mismatch
unsupported provider version
deterministic parser adapter bug
invalid configuration
identity collision
```

Backoff SHALL be bounded and cancellation-aware.

---

## 63. Circuit breakers

Repeated failure of one provider SHOULD open a per-provider/workspace circuit:

- stop rapid retry;
- keep other capabilities available;
- report degraded state;
- periodically probe;
- close after successful health check.

---

## 64. Fail-closed rules

The system SHALL fail closed for:

- incomplete rustc extraction manifest;
- ambiguous owner replacement;
- invalid relation endpoints;
- cross-generation source spans;
- unknown publication table versions;
- schema incompatibility;
- capability status missing for invalidated families.

Fail closed means withholding affected facts, not shutting down all query service.

---

# Part IX — Agent and MCP Delivery Contract

## 65. Central daemon authority

The central Rust daemon owns:

- workspace source state;
- update lifecycle;
- snapshots;
- provider orchestration;
- CPG query execution;
- capability status.

FastMCP STDIO instances SHALL act as coordination and presentation layers.

They SHALL NOT maintain independent mutable CPG state.

---

## 66. Daemon connection

Recommended local transport:

```text
Unix domain socket on Linux/macOS
named pipe or loopback transport on Windows
```

Each request includes:

```text
client_id
workspace_id
request_id
freshness_policy
deadline
query payload
optional target paths/entities
```

---

## 67. Freshness policies

### 67.1 `latest_published`

Return immediately from active immutable snapshot.

Include pending/degraded status.

### 67.2 `await_latest`

Wait until all events observed before request admission are reflected in a hot snapshot, up to deadline.

### 67.3 `require_current_for_targets`

Re-stat/re-hash specified paths or resolve specified owners, prioritize their update, and wait for required fact families.

### 67.4 `require_source_current`

Require current source/syntax only; semantic facts may be unavailable.

### 67.5 `require_semantic_current`

Require requested semantic/derived capabilities or fail explicitly.

---

## 68. Freshness barrier

A barrier captures:

```text
event sequence at admission
current dirty generation
target file digests where specified
```

Completion requires:

- no required target has an unprocessed generation at or below barrier;
- active snapshot source generation covers barrier;
- requested capabilities are current or explicitly unavailable due to source/provider failure.

---

## 69. Query response status

Every response SHALL include:

```text
snapshot_id
source_generation
durable_base_publication
overlay_generation
freshness_state
source_trust_state
pending_update_count
capability_statuses relevant to query
diagnostics relevant to unavailable facts
```

---

## 70. Empty-result semantics

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

## 71. Source fallback delivery

When semantic facts are unavailable for a current source owner, the query layer SHOULD include:

- current source file and range;
- enclosing syntax owner;
- parse/compiler diagnostics;
- unavailable capability list;
- source retrieval handle or inline bounded context.

This is not an engineering recommendation; it is a transparent delivery fallback.

---

## 72. Multiple-agent fairness

The daemon SHOULD enforce:

- per-client query concurrency limits;
- global query admission;
- update priority independent of client identity;
- optional priority boost for targets referenced by an active query;
- no permanent starvation from one agent's bulk operations.

---

# Part X — Durable Operational State

## 73. New control-plane tables

The prior data fabric SHALL be extended with operational lifecycle tables.

These tables are not semantic CPG facts.

### 73.1 `workspace_update_state`

**Primary key:** `repository_id`.

Columns:

```text
repository_id             id16
lifecycle_state_code      code16
source_trust_state_code   code16
active_snapshot_id        id16
source_generation         int64
event_watermark           int64
newest_dirty_generation   int64
durable_generation        int64
reconcile_required        bool
updated_at                timestamp
last_diagnostic_id        id16 nullable
```

### 73.2 `update_wave`

**Primary key:** `wave_id`.

```text
wave_id                   id16
repository_id             id16
source_generation         int64
watermark_event_seq       int64
state_code                code16
source_inventory_digest   hash32
dirty_path_count          int64
changed_owner_count       int64
started_at                timestamp
fast_published_at         timestamp nullable
hot_published_at          timestamp nullable
durable_published_at      timestamp nullable
failure_diagnostic_id     id16 nullable
```

### 73.3 `update_wave_item`

**Primary key:** `(wave_id, item_id)`.

```text
wave_id
item_id
path
file_id nullable
owner_id nullable
update_class_code
priority_code
source_digest nullable
state_code
capability_mask
diagnostic_id nullable
```

### 73.4 `provider_run`

**Primary key:** `provider_run_id`.

```text
provider_run_id
wave_id
producer_code
scope_owner_id nullable
build_unit_id nullable
source_generation
state_code
input_digest
output_digest nullable
started_at
completed_at nullable
diagnostic_id nullable
```

### 73.5 `hot_overlay_manifest`

Durable only when optional overlay journaling is enabled.

```text
snapshot_id
base_publication_id
overlay_generation
source_generation
source_inventory_digest
journal_uri
journal_checksum
created_at
```

---

## 74. Operational state retention

Operational rows MAY be retained for short bounded troubleshooting windows.

They are not exposed as code history.

Vacuum/cleanup SHALL preserve:

- active durable publication;
- recovery-required publication;
- active overlay journal;
- in-flight operation IDs.

---

# Part XI — Performance Objectives and Tuning

## 75. Latency decomposition

Measure:

```text
event occurrence
→ debounced delivery
→ dirty registration
→ source image captured
→ fast syntax ready
→ hot snapshot active
→ semantic snapshot active
→ durable publication active
```

Metrics:

```text
watch_to_dirty
dirty_to_source
source_to_fast
fast_to_semantic
semantic_to_hot
hot_to_durable
query_freshness_wait
```

---

## 76. Initial performance objectives

These are benchmark targets, not guarantees.

For a small isolated edit on a local filesystem:

```text
fast source/syntax visibility:
  target sub-second, preferably a few hundred milliseconds

Python semantic convergence:
  target sub-second to low-single-digit seconds depending project

Rust semantic convergence:
  bounded primarily by rustc incremental check and affected target

query snapshot acquisition:
  near-constant-time Arc clone

durable Delta convergence:
  asynchronous and micro-batched
```

The daemon SHALL report actual p50/p95/p99 performance.

---

## 77. Work granularity

Prefer:

- file batches for source parsing;
- owner batches for local derivation;
- module/crate batches for semantic providers;
- component batches for graph fixed points;
- multi-owner Arrow batches for storage.

Avoid one task or one Parquet file per individual fact.

---

## 78. Memory policy

- immutable source bytes use `Arc`;
- worker-local builders avoid shared mutation;
- bounded queues;
- limited DataFusion memory pool;
- bounded spill directory;
- cache byte limits;
- overlay size thresholds;
- early bulk-reconcile escalation under storm.

---

## 79. Query headroom

Reserve capacity so update work does not consume every CPU and memory resource.

Under overload:

- lower-priority durable and global derived work pauses first;
- source/syntax and query target refresh remain prioritized;
- query service continues on immutable snapshot.

---

# Part XII — Validation and Testing

## 80. Correctness oracle

For any fixed source snapshot:

> **The incrementally maintained CPG SHALL equal a clean rebuild under the same provider, schema, and derivation versions.**

Equality includes:

- canonical IDs;
- owner fact sets;
- capability statuses;
- unknown facts;
- derived facts;
- summaries;
- source spans.

---

## 81. Lifecycle scenario tests

Minimum automated scenarios:

```text
cold bootstrap
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
schema/provider upgrade
```

---

## 82. Watcher tests

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
- root deletion.

---

## 83. Generation and supersession tests

- stale parse result cannot commit;
- stale rustc result cannot commit;
- newer edit during reconcile is replayed;
- wave supersession preserves latest digest;
- query snapshot remains stable during pointer swap.

---

## 84. Capability-gap tests

Verify distinct responses for:

```text
syntax current / semantic pending
syntax current / compile failed
Python local facts current / Pyrefly unavailable
derived facts unavailable
source unverified after event loss
proven empty with complete fact family
```

---

## 85. Crash-injection tests

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
```

After restart, active query state SHALL be coherent.

---

## 86. Parallelism tests

- bounded queue memory;
- no DashMap guard across await;
- no deadlock under concurrent edits/queries;
- thread budget respected;
- bulk work does not starve query-target updates;
- cancellation stops fixed-point work;
- rustc concurrency remains bounded.

---

## 87. Performance tests

Workloads:

```text
single save
save + formatter
10-file refactor
1,000-file generated update
10,000-file branch switch
compile-failing Rust edit
rapid 100-edit supersession
concurrent queries from multiple agents
```

Measure:

```text
events received
events escalated to reconcile
unique dirty paths
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
spill
queue wait
query wait
```

---

# Part XIII — Observability and Operations

## 88. Required metrics

### Watcher

```text
watch_events_total
watch_batches_total
watch_queue_full_total
watch_reconcile_requested_total
watch_backend_errors_total
watch_health_state
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

---

## 89. Structured tracing

Every update trace SHOULD carry:

```text
workspace_id
wave_id
source_generation
event_watermark
file_id
owner_id
provider_run_id
snapshot_id
publication_id
client_id when query-triggered
```

Absolute paths SHOULD be redacted in remote telemetry.

---

## 90. Health endpoint

The daemon SHOULD expose:

```text
workspace lifecycle
source trust
watcher backend
active snapshot ID
source generation
dirty path count
active wave state
provider health
durable lag
last successful reconcile
last failure
```

---

# Part XIV — Shutdown and Recovery

## 91. Shutdown ordering

```text
1. mark STOPPING
2. stop accepting new update waves
3. continue or reject new strict-current queries by policy
4. stop debouncer with joined stop
5. close ingress
6. choose drain or cancel
7. stop sidecars/compiler children
8. await worker completion
9. flush or discard hot overlay according to deadline
10. close durable stores
11. release workspace state
```

The watcher source SHALL stop before consumer state is destroyed.

---

## 92. Drain policy

Use when:

- daemon is expected to leave durable index current;
- shutdown deadline permits;
- active wave is near completion.

Drain only the newest non-superseded wave.

---

## 93. Cancel policy

Use when:

- process exit is urgent;
- source can be reconciled on restart;
- current durable publication remains valid.

Discard incomplete hot candidate and leave active pointer unchanged.

---

## 94. Startup readiness barrier

A workspace is `READY` only after:

- watcher registered;
- inventory verified;
- events during verification replayed;
- active snapshot constructed;
- source trust `TRUSTED`;
- required provider capability policy satisfied or explicit degraded mode entered.

---

# Part XV — Rust Workspace Architecture

## 95. Recommended crates

```text
codefabric-protocol
  daemon RPC, MCP-facing request/response DTOs, freshness contract

codefabric-watch
  notify-debouncer wrapper, event facade, watcher health

codefabric-source
  inventory, source images, hashing, path/symlink policy, Merkle digest

codefabric-coordinator
  workspace actor, dirty registry, waves, lifecycle state

codefabric-invalidation
  update classification, owner discovery, dependency propagation

codefabric-python-update
  Tree-sitter/Ruff/Pyrefly orchestration

codefabric-rust-update
  Cargo/rustc/MIR orchestration and compiler manifest handling

codefabric-derived-update
  owner-local and interprocedural incremental analyses

codefabric-hot-snapshot
  immutable overlay, tombstones, snapshot providers, pointer swap

codefabric-durable-publisher
  Delta owner replacement, publication manifest, idempotent recovery

codefabric-query
  snapshot-pinned DataFusion query service

codefabric-daemon
  Tokio runtime, IPC server, workspace registry, health, shutdown

codefabric-fastmcp
  Python coordination layer; one STDIO process per agent
```

---

## 96. Core interfaces

```rust
#[async_trait::async_trait]
pub trait WorkspaceUpdater {
    async fn ensure_fresh(
        &self,
        request: FreshnessRequest,
    ) -> Result<FreshnessResult, UpdateError>;
}

pub trait ChangeClassifier {
    fn classify(
        &self,
        old: Option<&IndexedSourceState>,
        new: Option<&SourceImage>,
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

---

# Part XVI — Mandatory Invariants

## 97. Consistency invariants

```text
1. Every query pins exactly one immutable ServingSnapshot.
2. A snapshot never mixes source generations.
3. Invalidated semantic facts are hidden before current syntax is published.
4. Missing provider output never proves absence.
5. Compile failure produces capability gaps, not stale-current compiler facts.
6. Unaffected owners remain queryable when their validity is proven.
7. Stale provider results cannot commit.
8. Owner replacement is atomic at snapshot visibility.
9. Multi-table durable publication activates only through one complete manifest.
10. Watcher events are invalidation hints, not graph mutations.
11. need_rescan or queue overflow triggers authoritative reconciliation.
12. Every asynchronous job carries generation and source digest.
13. Every derived fact references the exact base generation.
14. The active snapshot pointer changes only after validation.
15. Query serving never waits on a global graph mutation lock.
16. The incremental graph equals a clean rebuild for the same source snapshot.
```

## 98. Performance invariants

```text
1. Event handlers remain lightweight.
2. Queues are bounded or coalesced.
3. Repeated changes to one path collapse to one latest generation.
4. CPU-heavy work runs outside Tokio workers.
5. Process-wide thread budgets prevent oversubscription.
6. Bulk work cannot starve interactive source/query work.
7. Very small updates do not force immediate Delta micro-files.
8. Hot overlays are consolidated rather than chained indefinitely.
9. Global derived tables are not recomputed when a bounded affected component suffices.
10. Caches are immutable/versioned and have memory limits.
```

## 99. Failure invariants

```text
1. Provider crash cannot partially mutate active snapshot.
2. Publication failure cannot advance current durable pointer.
3. Daemon crash cannot leave a partially visible in-memory snapshot.
4. Event loss is surfaced as degraded source trust.
5. Reconciliation failure keeps prior snapshot active.
6. Storage failure degrades durability without corrupting current query state.
7. Shutdown cancellation never labels incomplete work healthy.
8. Fact-family unavailability is explicit in every relevant query response.
```

---

# Appendix A — Update-Class Decision Guide

```text
event received
  ↓
read authoritative current path state
  ↓
path absent?
  yes → removal
  no
  ↓
digest unchanged?
  yes → path/metadata-only or no-op
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
manifest/config/build context changed?
  yes → U6
  ↓
event loss or mass change?
  yes → U7
```

---

# Appendix B — Recommended Starting Configuration

These values are starting points to benchmark.

```text
notify-debouncer-full timeout       75 ms
notify tick rate                    20 ms
downstream gather window            20 ms
watch ingress capacity              4,096 events
dirty path bulk threshold           1,000 paths or 10% of workspace
interactive query freshness wait    2 s default
source snapshot retry count         3
Pyrefly concurrent workspace jobs   1–2
rustc concurrent jobs               1 per workspace, globally bounded
overlay max age                     1–2 s
overlay max rows                    workload benchmark
Delta durable flush                 micro-batched
DataFusion batch size               65,536 starting point
limited memory pool                 mandatory
spill directory                     configured and bounded
```

---

# Appendix C — Query Result Example for Non-Compiling Rust

```yaml
snapshot:
  snapshot_id: current-hot-snapshot
  source_generation: 418
  freshness: current_with_unavailable_capabilities

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

source_fallback:
  available: true
  reason: requested semantic facts could not be generated from current source
```

No old MIR or type facts for the invalidated owner appear in this response.

---

# Appendix D — Clean-Rebuild Equivalence Procedure

```text
1. freeze source inventory and provider versions
2. export incremental active snapshot
3. create an empty isolated fabric
4. run complete bootstrap generation
5. canonical-sort all fact tables
6. compare IDs, facts, capabilities, unknowns, summaries, and checksums
7. report any difference by owner/fact family
```

This procedure is the final correctness oracle for continuous updating.
