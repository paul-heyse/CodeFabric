---
artifact: authoritative-design
artifact_id: codefabric-continuous-cpg-lifecycle
suite_id: codefabric-relational-data-fabric
suite_version: 2.1.0
artifact_tag: LIFE
artifact_version: 2.1.0
authority_status: current
predecessor_path: docs/authoritative_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v2.0.md
---

# CodeFabric Continuous CPG Update and Lifecycle Specification v2.1

## 0. Authority, identity, and compatibility

The stable artifact ID is `codefabric-continuous-cpg-lifecycle` (`LIFE`). This document is the
current normative owner of workspace registration, source truth, watching and Git acceleration,
update waves, invalidation, provider/derived scheduling, freshness barriers, resource admission,
recovery, shutdown, and deployment cutover.

The v2.0 predecessor is immutable release history. V2.1 preserves its current-source semantics,
source-trust and event-health separation, safe path/root behavior, owner-scoped invalidation,
syntax-first availability, explicit provider gaps, supersession, bounded concurrency,
one-snapshot queries, crash recovery, fairness, and presentation topology. It replaces mutable
serving pointers, direct publisher routes, static capability/transition censuses, and generated
current-state packages with `FabricCommand`, immutable `FabricEpoch`, Delta activation events,
and executed relational proof.

Released lifecycle/freshness/status symbols retain their public meaning. They are external wire
declarations, not a hand-maintained inventory of current runtime capabilities. Current state and
allowed transitions are derived from actor state, durable command/activation events, exact epoch
relations, and executable transition queries.

## 1. Runtime topology and ownership

The mandatory local profile is:

```text
one authorized workspace source instance
  -> one Rust daemon and one WorkspaceCoordinator actor
       -> source/watch/Git adapters
       -> exact provider processes and derived analyses
       -> one FabricCommand actor and fenced writer generation
       -> immutable FabricEpoch construction and activation
       -> query/update/resource scheduler
  -> one FastMCP STDIO presentation process per programming agent
```

One workspace is one registered Git worktree or non-Git root. `workspace_id` is the routing and
authorization identity; optional repository/worktree IDs describe topology but do not replace it.
Linked worktrees have independent source generations, update actors, epochs, and writer fences.
Immutable repository object/cache resources may be shared only when they cannot carry worktree
HEAD/index/source state.

Only the coordinator actor mutates workspace lifecycle state. Only the `FabricCommand` actor may
admit durable domain mutations. Only an activation event may select semantic current. FastMCP,
provider adapters, query tasks, SQLite, watchers, and gix cannot activate an epoch.

## 2. Source truth, paths, and repository policy

Stable filesystem bytes opened under the registered root are source authority. Every source
image records workspace, byte-safe relative path identity, file/source generation, exact bytes or
digest, encoding/newline metadata where applicable, and capture fence. Display strings are never
identity or authorization keys.

Root confinement uses descriptor-relative/component-wise opening, symlink policy, owner/mode
checks where applicable, and post-open containment. String-prefix checks are insufficient.
Non-UTF-8 paths remain representable. A source path slot, content generation, and semantic owner
are different identities, so create, atomic replace, delete, rename, and move do not conflate
path continuity with content or declaration identity.

`notify-debouncer-full` provides low-latency invalidation hints. gix provides read-only topology,
inventory/status/index/tree-diff/attribute/ignore acceleration. Neither emits semantic facts or
authoritative bytes. The daemon MUST NOT mutate Git, check out, invoke Git credentials/network,
run hooks, or execute clean/smudge filters by default. A gix failure degrades acceleration and
falls back to bounded authoritative filesystem reconciliation when bytes remain readable.

Every Git-derived candidate carries the exact repository/worktree and `GitStateVector` fence.
HEAD/index/inclusion/attribute/operation-state change invalidates that candidate before commit.
Blob OIDs are cache/baseline hints, never the sole current-content identity.

## 3. Lifecycle phases and orthogonal state

### 3.1 Workspace phase

The released phase vocabulary remains:

```text
UNINITIALIZED -> GIT_DISCOVERING -> WATCH_REGISTERING
  -> BOOTSTRAPPING | WARM_RECOVERING -> GIT_STATE_VERIFYING -> READY
  -> COLLECTING_CHANGES -> SNAPSHOTTING_SOURCE
  -> UPDATING_FAST -> UPDATING_SEMANTIC -> RECONCILING -> READY
```

Exceptional phases include `GIT_BULK_RECONCILING`, `GIT_DEGRADED`, `DEGRADED`, `BLOCKED`,
`STOPPING`, and `FAILED`. A phase is not source trust, provider status, query availability, or
proof result.

### 3.2 Source and event state

```text
SourceTrustState:
  UNVERIFIED | VERIFYING | CURRENT | POTENTIALLY_STALE | UNAVAILABLE

EventStreamHealth:
  HEALTHY | RESCAN_REQUIRED | DEGRADED | UNAVAILABLE

GitAccelerationStatus:
  NOT_A_GIT_WORKTREE | GIT_UNAVAILABLE | GIT_READY | GIT_METADATA_DIRTY |
  GIT_SCANNING | GIT_OPERATION_IN_PROGRESS | GIT_BULK_RECONCILING | GIT_DEGRADED
```

Watcher overflow sets `RESCAN_REQUIRED` and invalidates current-byte trust until reconciliation.
A degraded watcher or Git adapter may coexist with `CURRENT` only after a generic authoritative
inventory and byte verification.

### 3.3 Work and capability state

Update waves retain distinct collection, snapshot, fast analysis/publication, semantic analysis,
derivation, validation, hot publication, durable flush/publication, superseded, cancelled, and
failed stages. Provider runs retain `QUEUED`, `RUNNING`, `SUCCEEDED`, `PARTIAL`, `FAILED`,
`TIMED_OUT`, `CANCELLED`, `SUPERSEDED`, `CRASHED`, `PROTOCOL_ERROR`, `STALE_RESULT`, and
`STALE_GIT_BASELINE`.

Owner capability remains separate from completeness:

```text
capability: CURRENT | PENDING | INVALIDATED | PARTIAL | UNAVAILABLE_PARSE |
            UNAVAILABLE_COMPILE | UNAVAILABLE_PROVIDER | UNAVAILABLE_DERIVATION |
            EXCLUDED | UNSUPPORTED | REMOVED | NOT_APPLICABLE
completeness: COMPLETE | PARTIAL | INDETERMINATE | UNAVAILABLE | NOT_APPLICABLE
```

Capability begins unknown and becomes advertised only after the exact producer/coverage/proof
relations pass. A successful provider process alone is not capability proof.

## 4. Event ingestion, dirty registry, and authoritative reconciliation

Event handlers do minimal work: normalize the authorized path identity, assign monotonic event
sequence, classify source versus selected metadata, and enqueue/coalesce into bounded ingress.
Queues are bounded. Repeated events for one path collapse to the newest generation; overflow,
loss, root replacement, or ambiguous rename broadens to a rescan rather than dropping state.

The dirty registry records the newest required source generation and reason per path/scope. It is
temporal coordination, not graph authority. The update actor chooses:

```text
isolated save -> targeted current-byte verification
bounded ambiguity -> gix status/index candidates + byte verification
branch/bulk change -> HEAD tree diff + status/index + byte verification
gix unavailable/untrusted -> generic authorized inventory + byte verification
event loss/overflow -> generation-fenced full authorized reconcile
```

Rename detection may preserve a path-slot relationship when proved, but canonical facts are
rebuilt from current source. Ignore/attribute policy is inclusion behavior, never authorization.
Nested repositories, submodules, linked worktrees, conflict stages, sparse/index-only changes,
mode changes, and symlinks remain explicit; they are not silently flattened.

## 5. Source images, classification, and invalidation

Each accepted update wave captures immutable source images only after a stability check. Reads
that change during capture retry or broaden. Source image identity and source generation fence
every provider, derivation, publication, and proof job.

Change classification computes differences; no static change census is current authority. The
invalidation planner joins changed source facts with programmatic relation ownership, semantic
dependency edges, context/environment identities, provider coverage, and derived-analysis
dependencies. It determines the smallest sound set of owners/relations to replace or withdraw.

Rules:

- invalidated semantic facts are hidden before current syntax is activated;
- unaffected owners remain current only when dependency validity is proved;
- owner deletion emits owner/relation tombstones even when replacement is empty;
- environment, manifest, import, macro, build, policy, explicit-input, program, or application-release changes may
  broaden beyond textually changed files;
- Python semantic-environment changes refresh affected modules and conservative reverse importers;
- Rust compile/configuration changes refresh affected crates/owners through the policy launcher;
- a stale generation, source digest, context, provider version, or Git fence cannot commit; and
- absence of new rows never substitutes for a withdrawal or explicit unknown.

## 6. Update pipeline and analysis lanes

The pipeline is:

```text
events -> dirty registry -> source images -> classified change/invalidation
  -> fast syntax provider relations
  -> optional syntax-current candidate
  -> exact Python/Rust semantic provider relations
  -> owner-local derived relations
  -> affected interprocedural/common graph relations
  -> schema/integrity/provenance/capability proof
  -> FabricCommand publication
  -> sealed FabricEpoch activation
```

### 6.1 Fast syntax lane

Tree-sitter and Ruff syntax/token/trivia/current source relations may produce a syntax-current
epoch before semantic work completes. That epoch removes invalidated semantic rows, retains only
proved unaffected owners, and exposes pending/unavailable capabilities. It never serves stale
invalidated semantics as current.

### 6.2 Python semantic lane

Exact Ruff/Pyrefly observations are source/context/environment pinned and exchanged through
relation-scoped Arrow IPC. Provider-native output remains `raw`. Application-owned CFG,
evaluation order, flow, alias, effect, resource, async, and summary calculations are separate
`derived` phases. A sidecar result is eligible only when source digests, environment, affected
module closure, provider release, schema, coverage, and trailer agree.

### 6.3 Rust semantic lane and trust launcher

Rust source and compiler work is separated from lifecycle policy. For untrusted workspaces the
launcher supplies immutable/read-only source/dependency views, private output/target paths, no
inherited credentials or network, bounded environment/process/CPU/memory/time/output, process-
group cancellation, and descriptor-relative ingress/egress validation. Build scripts and proc
macros execute only inside that profile. Failure to establish containment fails closed or emits
an explicitly authorized degraded capability; host execution is never silent fallback.

Exact `rustc_public` observations, narrow explicitly identified private enrichment, and
application-owned MIR analyses remain different authorities. Compile failure produces current
source/syntax plus `UNAVAILABLE_COMPILE`/explicit diagnostics and unknowns; last-known-good
invalidated compiler facts may remain in a hidden cache but are not present-state output.

### 6.4 Derived lanes

The typed transformation selects each algorithm and its native
DataFusion/recursive/function/extension rung.
Outputs identify algorithm release, input epoch/projection, precision, invalidation scope,
completeness, and provenance. Nonconvergence, cancellation, resource exhaustion, or missing input
produces an explicit gap. Clean recomputation is the correctness oracle.

## 7. Validation and candidate construction

Before an epoch may activate, validation proves:

- source image/generation and Git fences;
- provider protocol, schema, coverage, and exact version;
- `FAB` `SchemaContract` at ingress, logical/physical plan, stream, every batch, and sink;
- owner keys, endpoints, required unknowns, and authoritative replacement/tombstones;
- normalization/authority/conflict and derived producer closure;
- cross-owner/interprocedural affected closure;
- exact Delta root/version and immutable segment identity;
- policy/authorization and bound-plan closure;
- independent expectation, provenance, capability, resource, and causal proof; and
- complete activation pins under one exact application/provider release vector.

Validation produces violation/coverage/proof relations. It does not persist a free-standing green
flag or approve its own expected rows. An uncovered or unavailable required input is `unknown`
and blocks activation.

## 8. One mutation path and durable publication

Every update becomes a typed `FabricCommand` carrying operation ID, expected predecessor,
workspace, authorization, writer generation, input/program/application/source/provider pins, resource
envelope, and intended relation changes. One actor owns staging, deduplication, cancellation
boundaries, zero-retry Delta writes, unknown-outcome reconciliation, proof, and activation.

An OS-backed workspace lease and strictly monotonic durable writer generation are acquired before
domain writes. Every durable boundary rejects stale generation. SQLite records queues, retry
schedules, leases, cancellation acknowledgements, and command stage only; deleting it and
reconstructing from Delta/source must not alter semantic current.

Delta commits exact component versions first. Activation appends one predecessor-linked event
naming the complete input/program/application/source/provider/table/policy/proof set. Current is the unique
valid activation-chain head, never a mutable SQLite/Delta row or highest timestamp. Component
versions not selected by a valid event remain unreachable candidates until retention-safe
collection.

## 9. Admission, freshness, and epoch pinning

Request admission proceeds:

1. authenticate workspace/operation/source scope;
2. register the request against the current event watermark;
3. apply the selected freshness barrier and required capability scope;
4. reject or wait under the deadline without substituting a prior epoch;
5. derive authorization and clone one `Arc<FabricEpoch>`;
6. acquire query/result/table/segment/compiler/expectation leases;
7. execute and deliver entirely under that epoch; and
8. release leases only after terminal response/resource policy.

Public freshness policies retain the `QRY` meanings. Empty results include the pinned epoch,
coverage, capability, freshness, and limit state; an empty list alone is never sufficient.

Activation closes new admission before durable selection, revalidates predecessor/fence, appends
and reads back selection, swaps the epoch, reconciles temporal cache, and only then reopens.
Queries admitted earlier continue on their predecessor lease. A query cannot mix generations,
contexts, providers, functions, policies, table versions, overlays, proof, or source bytes.

## 10. Resource governance, fairness, and cancellation

Each epoch has one governed DataFusion runtime with bounded memory pool, private spill, object
stores, batch sizing, and target partitions. The coordinator admits update, provider, query,
graph, result, and maintenance work under process-wide CPU, memory, spill, process, row, byte,
time, and queue budgets. External provider/compiler processes count against admission.

Priority preserves security/recovery and source reconciliation, targeted strict-current updates,
ordinary source updates, interactive queries, semantic/derived work, durable flush/artifact work,
and maintenance in that order, with bounded aging and reserved update headroom. Scheduling is
fair by agent; one agent or query cannot monopolize workers, memory, spill, or result storage.

Cancellation and supersession are cooperative but reach actual work: debounce/wait, source
capture, gix jobs, provider process groups, DataFusion tasks/streams, graph loops, Delta staging,
artifact writes, and leases. A command is not interrupted inside an atomic durable critical
section; its outcome is reconciled before acknowledgement. Cancelled/superseded output cannot
commit.

Backpressure propagates from result consumer to Arrow stream/provider and never becomes an
unbounded Python/Rust queue. Resource exhaustion yields a typed terminal outcome and releases
reservations/spill/incomplete artifacts.

## 11. Failure and recovery semantics

### 11.1 Required behavior

- watcher loss/overflow: mark source unverified and run authoritative reconcile;
- gix corruption/failure: degrade acceleration, preserve current-byte correctness;
- provider crash/timeout/protocol error: emit explicit gap and withdraw invalidated capability;
- compile failure: retain current source/syntax, hide invalidated compiler facts;
- derivation nonconvergence/resource failure: explicit unavailable/partial derived family;
- Delta conflict/ambiguous response: read operation marker/version before any legal retry;
- proof/schema/policy failure: discard candidate and keep predecessor active;
- activation fork/missing predecessor/multiple heads: fail serving closed;
- crash before activation selection: predecessor remains current;
- crash after selection before swap: recover/install selected epoch before admission;
- spill/disk/resource exhaustion: terminate affected work without corrupting current epoch; and
- event or source change during scan: invalidate the candidate and replay newest generation.

Recovery runs with admission closed and no candidate epoch installed. It reconciles durable command
markers and activation evidence, derives the unique selected relation-root/version vector, rebuilds
one sealed session from exact provider batches, explicit typed inputs, and typed transformations,
then installs that selected epoch. Only afterward may it reconcile the receipt/ack cache and reopen
admission. It may discard reconstructible SQLite/in-memory caches. It never guesses, silently
rebases, or labels incomplete work healthy.

### 11.2 Clean reconstruction

A clean rebuild runs with predecessor generated registries/bundles/bootstrap/replay inputs
physically unavailable. It reads exact explicit typed inputs, inventories and captures current
source, reruns the real admitted providers/analyses, reconstructs typed transformations and exact
durable state/activation, and compares complete canonical/public rows, unknowns, diagnostics,
capability, and provenance with incremental state. Digest equality or replaying an accepted wave
alone is not the oracle.

## 12. Shutdown and startup readiness

Shutdown order is:

```text
mark STOPPING -> stop new waves/admissions as policy requires
-> stop/join watchers and close ingress
-> drain newest safe work or cancel/supersede
-> stop gix/provider/compiler/graph/query tasks
-> reconcile or abandon incomplete commands
-> flush or discard unselected segments under deadline
-> close stores/endpoints
-> release writer and daemon leases last
```

Startup is ready only after authorized root/topology discovery or explicit non-Git fallback,
watch registration, authoritative inventory, current source/Git fence, event replay, command and
activation recovery, exact epoch construction, source trust `CURRENT`, and required capability
policy or explicit degraded mode. A durable Delta version alone is not readiness.

## 13. Durable deployment cutover and legacy fencing

The target deployment transition is predecessor-checked and crash-reconcilable:

```text
LEGACY_AUTHORITATIVE
-> LEGACY_QUIESCED
-> NEW_BINARY_FENCED_READ_ONLY
-> NEW_EPOCH_SELECTED
-> NEW_SERVING_NO_MUTATION
-> NEW_MUTATING
-> LEGACY_RETIRED
```

One external Rust cutover controller owns an immutable, fsync/readback-verified deployment
journal and compare-and-swap head in a private per-workspace directory. The journal records
process/lease transition evidence; it cannot write fabric facts or select semantic current.

Before `NEW_MUTATING`, exactly one enforcement profile must revoke the frozen predecessor across
process restart, controller restart, target crash, and host reboot: a bridge release checking a
monotonic retirement generation at every old serving/write ingress, or an external platform
boundary revoking the old entrypoint and storage/write authority. The exact frozen binary is
restarted and must be unable to bind, serve, or write. After target mutation, legacy rollback is
forbidden; recovery uses a compatible retained target epoch or a corrective forward command.
Runtime fallback, dual write, and simultaneous serving authority are prohibited.

## 14. Observability and security

Metrics/traces cover watcher health, dirty paths, source/Git reconciliation, wave/provider/
derivation stages, queue/fairness, epoch build/activation, Delta conflicts, memory/spill,
cancellation, result leases, and query freshness. Every event identifies workspace, generation,
epoch, command/query/provider where relevant. Absolute paths, raw path bytes, source, credentials,
remote URLs, environment, provider stderr, and physical plans are redacted by default.

Same-user UDS authentication, short-lived workspace/operation credentials, descriptor-relative
root/source access, provider containment, result-resource authorization, and child-catalog ACLs
fail closed. Git ignore and display paths cannot widen authorization. The daemon is read-only to
source repositories; administrative fabric mutation does not imply repository mutation.

## 15. Executable acceptance obligations

| Contract | Required executable oracle |
|---|---|
| event loss, rename, bulk, and reconcile | `just lifecycle-invalidation-conformance-check` |
| source/gix authority boundary | `just source-authority-boundary-check` |
| exact provider update and withdrawal | `just exact-provider-fabric-check`; `just stale-provider-current-zero-state-check` |
| one mutation path and temporal isolation | `just fabric-single-mutation-path-check`; `just temporal-store-boundary-check` |
| writer lease/generation | `just single-writer-fence-check` |
| activation order, pinning, and faults | `just fabric-activation-recovery-check`; `just activation-fault-matrix-check`; `just fabric-epoch-pinning-check` |
| resource, cancellation, and fairness | `just resource-governance-check` |
| cold/incremental equivalence | `just durable-epoch-reconstruction-check`; `just clean-rebuild-legacy-input-zero-state-check` |
| deployment state and old-binary revocation | `just fabric-cutover-state-machine-check`; `just legacy-writer-fence-check` |
| public lifecycle behavior | `just semantic-delivery-vertical-check`; `just provider-protocol-check` |

Checks assert final authoritative state, not platform-specific watcher event sequences or fragile
physical plan text. Crash injection covers every durable write, barrier, selection, readback,
swap, cache, reopen, acknowledgement, lease, journal, and process side-effect boundary. A v2.1
lifecycle is nonconforming while a bypass writer, mutable semantic pointer, runtime fallback,
stale-current provider row, or unreconciled unknown outcome exists.
