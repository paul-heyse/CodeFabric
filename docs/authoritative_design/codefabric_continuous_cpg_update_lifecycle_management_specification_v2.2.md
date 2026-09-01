---
artifact: authoritative-design
artifact_id: codefabric-continuous-cpg-lifecycle
suite_id: codefabric-relational-data-fabric
suite_version: 2.2.0
artifact_tag: LIFE
artifact_version: 2.2.0
authority_status: current
predecessor_path: docs/authoritative_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v2.1.md
---

# CodeFabric Continuous CPG Update and Lifecycle Specification v2.2

## 0. Authority, identity, and compatibility

The stable artifact ID is `codefabric-continuous-cpg-lifecycle` (`LIFE`). This document is the
current normative owner of workspace registration, source truth, watching and Git acceleration,
update waves, invalidation, provider/derived scheduling, freshness barriers, resource admission,
recovery, shutdown, and deployment cutover.

The v2.1 predecessor is immutable release history. V2.2 preserves its current-source semantics,
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
  -> one WorkspaceSupervisor and one Rust daemon
       -> one WorkspaceCoordinator actor
       -> source/watch/Git adapters
       -> exact provider processes and derived analyses
       -> one FabricCommand actor and fenced writer generation
       -> immutable FabricEpoch construction and activation
       -> query/update/resource scheduler
  -> one attach-only AgentStdioLauncher and FastMCP STDIO presentation
     process per programming agent
```

One workspace is one registered Git worktree or non-Git root. `workspace_id` is the routing and
authorization identity; optional repository/worktree IDs describe topology but do not replace it.
Linked worktrees have independent source generations, update actors, epochs, and writer fences.
Immutable repository object/cache resources may be shared only when they cannot carry worktree
HEAD/index/source state.

Only the coordinator actor mutates workspace lifecycle state. Only the `FabricCommand` actor may
admit durable domain mutations. Only an activation event may select semantic current. FastMCP,
provider adapters, query tasks, SQLite, watchers, and gix cannot activate an epoch.

`WorkspaceSupervisor` is the sole daemon-launch authority. It owns the private
runtime directory, singleton lease and live probe, one private supervisor
rendezvous, the daemon-control socketpair, process group, restart generation,
grant registration, drain, join, reap, and owned-endpoint cleanup. An agent
invocation is attach-only and cannot start a daemon. `AgentLaunchPolicy` is an
operator-owned immutable input; the agent supplies only its opaque policy ID and
cannot declare principal, workspaces, operations, semantic profiles, resource
bounds, expiry, or revocation generation.

The configured policy root is outside workspace and adapter-package authority.
The supervisor resolves the opaque policy ID relative to that already
authorized root, walks and opens it without following symlinks, and rejects a
wrong owner, permissive mode, non-regular type, cross-device object, or replaced
inode. It parses the document strictly and binds one immutable policy revision
to principal, allowed workspace set, operations, semantic profiles/features,
MCP host/resource/deadline bounds, issue/not-before/expiry, revocation
generation, expected adapter distribution/executable identity where supported,
and maximum concurrent launches. Agent input is limited to the opaque policy ID
and genuinely variable presentation/runtime details. Policy drift creates a new
revision and never mutates an accepted grant.

### 1.1 Supervisor singleton and daemon control

Before starting `codefabricd`, the supervisor safely creates or opens its
private per-user runtime directory, acquires the per-workspace singleton lease,
and performs a live generation-aware probe. A lease records sufficient process
and generation identity that a PID file alone is never trusted. Stale recovery
requires lease ownership, a failed live probe, and exact owned socket type,
owner, device, and inode evidence. A losing racer never unlinks, signals, or
replaces the live winner.

The supervisor constructs daemon services before exposing a query socket,
creates one unnamed Unix socketpair, leaves every unrelated descriptor
close-on-exec, and maps the child endpoint to daemon stdin through safe
`OwnedFd`/`Stdio` ownership. It uses no pre-exec hook or ambient descriptor. It
starts the daemon in an owned process group, closes the parent copy of the child
endpoint, and requires generation/control acknowledgement before accepting an
adapter launch.

The retained endpoint is the only producer of length-bounded
`RegisterLaunchGrant`, `RevokePrincipal`, `AdvanceSupervisorGeneration`, and
`Acknowledgement` records. Each binds workspace, daemon and supervisor
generation, monotonic sequence, operation identity, expiry, and content
integrity. A gap, replay, changed duplicate, unknown record, wrong workspace,
operation, generation, or control-channel replacement fails closed. The daemon
derives launch authority only from an acknowledged registered grant and the
verified query-UDS peer; request-body identities are correlation assertions.

The supervisor authenticates an attaching launcher through the owned
rendezvous, same-UID kernel peer credentials, supported PID/start identity, the
selected policy, and bounded anti-replay request identity. Same UID alone never
selects claims. Linux and macOS may expose different PID detail, but both
require same UID, supervisor generation, and policy authorization; an absent or
mismatched observation fails closed whenever the selected platform/policy
requires it.

Control-channel loss closes new handshake and renewal authority and enters a
typed degraded/draining state. Already accepted queries retain their pinned
workspace and bounded cleanup contract rather than being silently restarted or
cancelled. The supervisor owns daemon drain, joined shutdown, signal
propagation, timeout escalation, child reap, singleton release, and exact
owned-socket cleanup; orphaned daemon operation is not a supported steady state.

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

Construction is phase typed. `PreEpochWorkspace` owns lawful empty-head
genesis and command recovery; `CandidateFabric` owns unselected computation;
`SealedEpoch` owns proved immutable state; `SelectedEpochRecord` binds the exact
read-back activation event, Delta/Arrow vector, writer fence, and control
horizon; only then may one complete `ActiveWorkspace` be atomically installed.
There is no separately mutable readiness Boolean, catalog/proof/vector bundle,
or query-visible half-installed workspace.

## 13. Fresh activation, singleton ownership, and forward repair

The selected deployment profile is `FreshActivation`: no deployed predecessor
owns the workspace UDS, daemon lease, writer lease, serving package, or
activation head. The target therefore does not construct predecessor comparison,
revocation, rollback, reboot simulation, bridge release, or compatibility state.
Historical implementations are never started merely to prove that they cannot
start.

An empty activation chain admits exactly one genesis operation through the sole
`FabricCommand` actor with `ExpectedHead::Empty`. The actor appends the activation
event, reads back that exact event and its reversible relation-root/version
vector, verifies its writer fence and selected control horizon, then atomically
installs the complete `ActiveWorkspace`. Crash before selection leaves the head
empty; crash after selection recovers and installs the selected epoch before
admission. Uncertain outcome or incoherent horizon is an explicit closed state
resolved from durable evidence and corrected forward, never guessed or rolled
back to a predecessor.

One private runtime directory admits one live `WorkspaceSupervisor` and daemon
per workspace. A losing singleton racer never unlinks, signals, or replaces the
winner. Supervisor restart advances generation and invalidates volatile grants,
sessions, and cursors. Loss of the inherited daemon-control channel closes new
handshake/renewal authority and enters a typed degraded/draining state while
already accepted work follows its bounded retention contract. Adapter or
launcher exit revokes launch-scoped authority without silently cancelling or
resubmitting accepted work.

If read-only deployment evidence later proves a real predecessor exists, stop
and version a separate one-shot `AuthorityHandoff` design. No dormant handoff
path, dual write, runtime fallback, or simultaneous serving authority is
permitted in this suite.

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

Operational security proof rejects symlink, wrong owner, wrong mode, wrong
type, cross-device replacement, replacement-inode cleanup, and unsafe existing
socket cases for policy roots, runtime directories, leases, rendezvous, and UDS
paths. It also rejects wrong UID, supported PID/start mismatch, wrong policy,
workspace, operation, daemon/supervisor generation, expiry, revocation, replay,
and launch-capacity requests before session authority. Partial spawn, early
exit, rendezvous/control loss, restart, signal, timeout, and stale-artifact
faults leave no live credential, inherited descriptor, unowned child, or
attacker-selected socket. The local profile does not claim defense against a
fully compromised same-UID process; cross-user or network deployment requires a
new identity and transport design.

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
| fresh genesis, singleton ownership, and forward repair | `just fresh-successor-activation-check`; `just supervisor-launch-contract-check` |
| platform descriptor/socket lifecycle | `just supervisor-launch-platform-check`; `just session-uds-presentation-boundary-rejection-check` |
| public lifecycle behavior | `just semantic-delivery-vertical-check`; `just provider-protocol-check` |

Checks assert final authoritative state, not platform-specific watcher event sequences or fragile
physical plan text. Crash injection covers every durable write, barrier, selection, readback,
swap, cache, reopen, acknowledgement, lease, journal, and process side-effect boundary. A v2.2
lifecycle is nonconforming while a bypass writer, mutable semantic pointer, runtime fallback,
stale-current provider row, or unreconciled unknown outcome exists.
