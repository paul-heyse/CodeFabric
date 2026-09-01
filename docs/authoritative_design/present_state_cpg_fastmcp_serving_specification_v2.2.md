---
artifact: authoritative-design
artifact_id: codefabric-present-state-cpg-fastmcp-serving
suite_id: codefabric-relational-data-fabric
suite_version: 2.2.0
artifact_tag: SRV
artifact_version: 2.2.0
authority_status: current
predecessor_path: docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.1.md
---

# Present-State CPG FastMCP Serving Specification v2.2

## 0. Authority, identity, and compatibility

The stable artifact ID is `codefabric-present-state-cpg-fastmcp-serving` (`SRV`). This document
is the current normative owner of the private daemon RPC, accepted query handles, FastMCP tools/
resources/prompts, strict public adapter models, cancellation and progress translation, public
status/error projection, immutable result resources, and one-logical-response delivery.

The v2.1 predecessor is immutable release history. V2.2 preserves the four-tool MCP catalog, one
STDIO adapter per agent, strict Pydantic boundary,
effective semantic request identity, progress/cancellation/deadline behavior, inline/resource
discriminated delivery, stable public errors, result-resource authorization/leases, and
orthogonal result status fields. The sole production package is
`codefabric.cpgd.v2`; historical v1 source, descriptors, allocations, and
fixtures are immutable evidence only and are not generated, packaged,
translated, negotiated, or served by production.

The adapter is presentation only. It does not import, replay, or own a semantic model, schema/
function/provider registry, query bundle, catalog fingerprint, capability census, Arrow
transformation, DataFusion plan, Delta table, proof gate, activation pointer, `FabricEpoch`, or
mutable CPG state. Current reference/status/query content is a filtered projection of the Rust
daemon's admitted epoch.

## 1. Topology and responsibility

```text
operator-owned AgentLaunchPolicy
  -> one WorkspaceSupervisor and one shared Rust workspace daemon
programming agent
  <-> attach-only AgentStdioLauncher
       <-> one FastMCP 3.4.7 STDIO process
       <-> authenticated private gRPC over Unix-domain socket
            <-> one Rust workspace daemon
                 -> LIFE source/update/recovery authority
                 -> FAB FabricEpoch/DataFusion/Arrow/Delta authority
                 -> QRY semantic compiler and canonical response authority
```

The Rust daemon owns workspace registration, source state, providers, derived analyses, epochs,
authorization, semantic resolution, DataFusion execution, Arrow schemas/streams, result bytes,
leases, capabilities, errors, and all durable mutation. The adapter owns MCP framing, one
immutable settings snapshot, a long-lived daemon client, strict public validation/serialization,
host delivery adaptation, progress, cancellation, and safe human summaries.

One adapter binds exactly one agent instance and one authorized workspace. Multiple adapters may
connect concurrently; none shares mutable adapter state or becomes a workspace coordinator.
HTTP, ASGI, multi-user gateways, network listeners, and shared Python adapter processes are not
part of the local v2.2 profile.

## 2. Four contract planes

The serving boundary separates:

| Plane | Representation | Owner |
|---|---|---|
| control | released Protobuf | daemon/RPC contract |
| semantic request/response | released canonical JSON profile plus typed Arrow result relations where negotiated | QRY/FAB |
| provider data | relation-scoped Arrow IPC under outer Protobuf framing | FAB/GEN; never FastMCP semantics |
| presentation | strict Pydantic/FastMCP tool, resource, prompt, and metadata models | SRV |

Protobuf carries authentication, compatibility, accepted handles, progress, flow control,
cancellation, leases, errors, result descriptors, and terminal state. It does not grow a second
semantic fact DTO graph. Python may incrementally decode validated result streams or present
canonical JSON, but it does not join, filter, normalize, cast, deduplicate, aggregate, traverse,
or otherwise reproduce Rust semantics.

## 3. Exact adapter baseline and package posture

The production adapter exact-pins:

```text
fastmcp            == 3.4.7
pydantic           == 2.13.4
pydantic-settings  == 2.15.0
```

Public models are frozen, strict, schema-closed, and extra-forbid. Settings are immutable;
production dotenv loading is disabled; capability credentials use secret types and never
serialize/log. A module-scoped reusable JSON `TypeAdapter` validates the semantic request as a
JSON object, while the daemon performs the full released schema and semantic validation.

The v2 `.proto` sources and released field/method allocations are authority. Rust/Python generated
bindings and descriptors are derivable caches and MUST regenerate/compare and interoperate. A
committed generated cache is retained only when source-distribution/wheel installation in a
proved constrained foreign environment requires it. No package data may contain current program,
query, phrase, provider, function, capability, proof, or epoch state.

## 4. Handshake, launch grant, and v2 negotiation

Before adapter spawn, the supervisor resolves one immutable operator-owned
`AgentLaunchPolicy`, registers its claims with the daemon over an inherited
control socketpair, and delivers one single-use high-entropy capability to the
adapter through allowlisted inherited fd 3. Capability bytes never appear in
argv, ordinary environment, repository files, logs, process listings, or MCP
traffic. The adapter consumes that capability in one lifespan handshake before
advertising readiness.

The launcher supplies only an opaque policy ID. The supervisor safely resolves
that ID relative to its configured operator-owned policy root, never a workspace
or adapter-package path, and verifies no-follow opening, expected owner,
restricted mode, regular-file type, device, and inode before strict parsing.
The policy binds principal, workspace set, operations, profiles/features,
resource/deadline bounds, validity and revocation generation, adapter identity
where observable, and launch capacity. Configuration drift creates a new policy
revision rather than changing an accepted grant.

The supervisor authenticates the launcher through its owned rendezvous,
same-UID kernel peer credentials, supported PID/start identity, selected policy,
and bounded anti-replay request identity. Same UID alone is insufficient. It
reserves launch capacity, mints a capability, sends only its digest and
immutable claims to the daemon, and waits for control acknowledgement before
releasing capability bytes. Failed registration starts no adapter and consumes
no reusable credential.

The handshake request identifies only correlation/runtime capabilities and the
single-use grant. Principal, workspace set, operations, profiles, bounds,
expiry, revocation generation, and daemon/supervisor generation derive from the
registered grant and verified UDS peer, never from body claims.

The response contains safe daemon/Rust/RPC versions, authorized workspace identity and lifecycle,
active compatible semantic profiles, public schema fingerprints, admitted public snapshot/epoch
projection when available, supported languages/forms/capabilities derived from the epoch, result
resource capabilities, and hard limits.

Handshake fails before tool readiness on:

- RPC/protocol major mismatch or a non-v2 package;
- unsupported semantic-query major or required feature;
- same-version released-schema fingerprint mismatch;
- invalid/replayed/expired credential;
- wrong UID or supported PID/start observation, policy, workspace, operation, generation, expiry,
  revocation, replay, or launch capacity; or
- invalid public contract source/binding equivalence.

The adapter may be transport-ready while the workspace is bootstrapping. Status/reference remain
available; fact queries return the stable bootstrapping error. A durable Delta table alone does
not imply query readiness.

## 5. Released daemon service and accepted handles

The sole `codefabric.cpgd.v2.CpgQueryService` has exactly these operations:

```text
Handshake
GetStatus
GetReference
ValidateQuery
StartQuery
WatchQuery
CancelQuery
ReadResource
ReleaseResource
```

`StartQuery` normalizes the complete operation, reserves coordinator, journal,
idempotency, task, result, and retention capacity, and returns the original
acceptance record. Same idempotency key plus the same normalized operation
returns that record; any changed bound field is a typed conflict. Acceptance
never precedes enforceable capacity.

`WatchQuery` starts or resumes control observation from an optional
content-bound opaque cursor. A cursor binds query, session/principal class,
daemon generation, selected profile, next sequence, preceding event content,
and expiry. Reconnect watches the accepted query identity and never restarts it.
Events are one of:

```text
SnapshotPinnedEvent
ProgressEvent
ResultReadyEvent
TerminalEvent
```

Events contain no semantic result bytes. Replaceable progress is coalesced
before sequence allocation; snapshot, result-ready, and terminal events are not
dropped. `ResultReadyEvent` names one sealed immutable result package.
`ReadResource` streams one authorized bounded package, manifest, page, reference,
or projection descriptor with transport backpressure. `ReleaseResource`
idempotently releases a lease and stabilizes races with a bounded tombstone.
Neither method exposes a filesystem or object-store path.

`ValidateQuery` validates JSON schema, fact-only doctrine, request relations, phrase resolution,
query IDs and typed result references, DAG/cycles, capability/producer closure, negative-proof
coverage, bounds, authorization, and cost class without executing retrieval.

## 6. Query lifecycle

The daemon path is:

```text
authenticate exact agent/workspace/operation
-> enforce adapter byte/JSON complexity limits
-> establish semantic_request_id, mcp_call_id, rpc_attempt_id
-> normalize idempotency inputs and reserve all accepted-work capacity
-> validate released request and RPC freshness agreement
-> register/apply LIFE freshness barrier
-> clone and lease one FAB Arc<FabricEpoch>
-> emit SnapshotPinnedEvent and public snapshot projection
-> derive AccessScopeId and reduced child catalog
-> compile QRY request relations to bounded DataFusion plans
-> execute independent branches under epoch resources
-> validate Arrow/result schemas, order, coverage, errors, and checksum
-> encode bounded independently decodable Arrow IPC pages and seal manifest last
-> emit one TerminalEvent and release execution resources
```

The adapter captures handles, maps progress, applies host delivery policy, and propagates
cancellation. It does not select or inspect a `FabricEpoch`, compile a plan, or decide semantic
errors.

The effective semantic request ID is preserved if supplied or generated once and injected before
canonical hashing. Idempotency identity includes workspace, agent, effective semantic ID,
canonical normalized request hash, and structured freshness; after pinning it also retains the
epoch/snapshot. Same ID with different content/freshness is `IDEMPOTENCY_CONFLICT`. Retry/resume
never migrates an accepted query to a newer epoch.

## 7. Public FastMCP catalog

### 7.1 Tools

The stable catalog has exactly four semantic-facing tools:

1. `query_code_graph` — execute one complete eight-form compositional request;
2. `validate_code_graph_query` — validate/resolve/estimate without retrieval;
3. `get_code_graph_status` — return safe readiness/freshness/capability/limit status; and
4. `get_code_graph_reference` — retrieve constrained public schemas/guidance/reference views.

The catalog does not expose one tool per query form, arbitrary SQL, physical table access,
provider administration, epoch activation, `FabricCommand`, cutover, or generic filesystem
resources. Tool annotations remain read-only, non-destructive, and closed-world only within the
explicit authorized/coverage scope.

### 7.2 `query_code_graph`

Inputs are:

```text
request: JSON object conforming to negotiated QRY profile
delivery: automatic | inline | resource       default automatic
```

`delivery` is presentation-only and excluded from canonical semantic request identity. The outer
strict result exposes effective semantic/MCP IDs; execution, availability, completeness,
freshness, and limit states; public snapshot projection; delivery discriminant; byte length and
checksum; query statuses/counts/notices; and either an inline canonical response or immutable
resource reference. It never contains both full payloads.

The nested inline canonical response remains validated daemon-owned JSON. The adapter constructs
explicit allowlisted public models and `ToolResult.meta`; it MUST NOT forward an unrestricted
daemon mapping, operational metadata, internal plan, physical name, provider error, or secret.

### 7.3 Other tools

`validate_code_graph_query` returns `valid`, request ID, normalized request, dependency graph,
resolved semantics, capability requirements, resource estimate, errors, and warnings.

`get_code_graph_status` returns adapter readiness, authorized workspace/agent, active public
snapshot/epoch summary, compatible component versions, derived languages/forms/capabilities,
freshness, hard limits, and safe notices. It does not trigger generation or wait for publication.

`get_code_graph_reference` accepts a constrained reference selector. It projects current
authorized program/capability/reference relations and released request/response schemas. It does
not read package-owned query bundles or arbitrary paths/URIs.

### 7.4 Resources and prompts

Resources include released request/response/public schemas, concise guide and recipe templates,
authorized live capability/snapshot/reference views, and immutable result artifacts/
subresources. Prompts remain `author_code_graph_query` and `interpret_code_graph_facts`. Prompt
and recipe content teaches objective fact queries and never embeds a current registry or executes
semantics.

## 8. Progress, deadlines, cancellation, and reconnect

The public semantic phase vocabulary remains small:

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

Progress may include completed/total units, current query ID, safe message, and elapsed time. It
never exposes SQL, table/function names, physical plans, internal edges, source, or credentials.

One relative execution budget is divided monotonically across admission,
freshness, planning, execution, materialization, delivery, and cleanup. Absolute
wall timestamps are observations only and never cancellation authority. Deadline
nesting reserves cleanup time:

```text
MCP host >= FastMCP tool >= adapter gRPC >= daemon freshness + execution
```

On MCP cancellation, the adapter sends idempotent `CancelQuery`. Dropping a
`WatchQuery` cancels observation only. Adapter/launcher exit revokes its session
and launch-scoped resources, while already accepted daemon work follows its
bounded query/retention contract. The daemon cancels freshness wait,
DataFusion/providers/graph/materialization only in response to the accepted
query's cancellation/deadline policy, releases reservations/spill/leases, and
deletes incomplete private pages.

Reconnect creates a fresh grant, channel, and session, then uses the accepted
daemon query identity and content-bound cursor. It never resubmits `StartQuery`.
Replay/resume windows and terminal retention are explicit. Broad automatic
retries around arbitrary MCP tool calls are prohibited.

## 9. One logical response and delivery policy

The daemon produces one sealed result package independent of transport delivery.
Its response envelope, manifest, bounded independently decodable Arrow IPC page
objects, leases, tombstones, and cleanup share one authority. Page and manifest
identities prove integrity, never semantic correctness. Bounded JSON projection,
Arrow transfer, inline presentation, and resources preserve the same epoch,
records, deterministic order, coverage, errors, and completeness.
Large results externalize; they are never silently truncated to fit MCP.

Default local thresholds retained from the released profile are:

```text
automatic inline threshold:       512 KiB maximum, further bounded by host profile
inline hard maximum:              4 MiB maximum, further bounded by host profile
automatic source-byte threshold:  128 KiB
result read chunk maximum:        1 MiB
single artifact soft maximum:     256 MiB
single artifact hard maximum:     1 GiB
```

`automatic` selects inline within the effective threshold, otherwise resource. `resource` always
externalizes. `inline` above the effective hard maximum overrides to resource when the host
supports it and records the reason; otherwise it fails `RESULT_TOO_LARGE_FOR_HOST`. Content,
order, checksum, and completeness remain identical.

## 10. Immutable result resources

Result packages are Rust-daemon-owned immutable canonical response objects, not Delta tables or
generic filesystem resources. The stable URI form is:

```text
codefabric-result://<workspace-hex>/<artifact-hex>
```

Selectors such as query/entity/fact/source-context are typed fragments or resource parameters,
never filesystem paths. Artifact identity binds workspace, owning agent, pinned epoch/snapshot,
canonical response checksum, and format/version. Creation is idempotent; an existing identity
with different owner/snapshot/format/length/checksum is `ARTIFACT_ID_COLLISION`.

Metadata includes agent/workspace, epoch/snapshot, exact durable/segment pins, semantic request,
content type/encoding/length/checksum, source-bearing flag, timestamps, lease IDs, and subresource
index identity. Files are private, immutable, and stored under mode-`0700` directories with
mode-`0600` files in the local profile.

Retained defaults are one-hour ordinary TTL, thirty-minute source-bearing TTL, 2 GiB per-agent,
8 GiB per-workspace, and 10 GiB global quota. Active query/result/read leases protect exact epoch,
Delta versions, segments, application/provider releases, expectations, and bytes from collection. Release is
idempotent. Expiry before read is `RESOURCE_EXPIRED`; a fixed active read lease may finish.

Every full/range read reauthorizes credential, agent/workspace ownership, artifact lease, and
current source-disclosure policy. Offset/checksum/range are validated before bytes. Cross-agent
access is denied even when the opaque ID is known; a narrowed/revoked ACL denies new bytes.

## 11. Public status, errors, and redaction

### 11.1 Orthogonal status

Public outputs retain QRY's execution, availability, completeness, freshness, limit, and
dependency dimensions. The v2 public snapshot record carries the epoch,
programmatic-assembly/release/proof fields selected by its current profile. Status is a projection of the
current admitted epoch and LIFE state, never an adapter cache or packaged census.

Basic status may expose safe workspace display identity, lifecycle/readiness, public epoch,
source/event/Git summaries, public context labels, capability aggregates, queue counts/effective
limits, and safe diagnostic codes. Diagnostic authorization may expose provider/update/durable
size and non-sensitive lease/storage summaries. Neither exposes raw roots, Git remotes/history,
credentials, environment, command lines, provider stderr, arbitrary SQLite/Delta rows, or
unfiltered catalogs.

### 11.2 Error layering

Query-level semantic errors remain records in the canonical logical response and permit
independent successful blocks. Tool-level authentication, handshake, invalid outer input,
transport, or unavailable delivery errors terminate the MCP call. The adapter maps layers without
renaming semantic errors or leaking raw causes.

Released public error names/numeric mappings are append-only compatibility declarations owned by
the released wire source, not a mutable current-state registry. Required names include the QRY
semantic set and transport/resource names such as:

```text
INCOMPATIBLE_MAJOR
UNSUPPORTED_MINOR
WORKSPACE_NOT_AUTHORIZED
PATH_OUTSIDE_AUTHORIZED_ROOT
SOURCE_ACCESS_DENIED
CAPABILITY_UNAVAILABLE
SOURCE_SNAPSHOT_MISMATCH
PROVIDER_PROTOCOL_ERROR
SANDBOX_UNAVAILABLE
IDEMPOTENCY_CONFLICT
CREDENTIAL_REPLAY_DETECTED
RESUME_WINDOW_EXPIRED
RESULT_TOO_LARGE_FOR_HOST
ARTIFACT_ID_COLLISION
RESOURCE_EXPIRED
STATE_TRANSITION_VIOLATION
INTERNAL_INVARIANT_VIOLATION
```

Each error exposes only its approved safe fields, layer, retryability, scope, message, and
diagnostic reference. Raw paths/source/tokens/credentials/provider errors/DataFusion text/host
state remain internal unless a separately authorized diagnostic resource permits them.

## 12. Authorization and source disclosure

Single-use launch grants bind principal, adapter launch, workspace set,
operations, ACL/semantic profile, host/resource bounds, daemon/supervisor
generation, issue/expiry, revocation generation, and anti-replay identity. The
daemon accepts a grant only after `WorkspaceSupervisor` registration and verifies
it against same-UID UDS peer credentials plus supported PID/start observation.
Body identifiers are correlation assertions only. Every query,
status/reference projection, source context, resource creation/read, and
cancellation/reconnect operation reauthorizes.

Fact and source authorization are separate. ACL outcomes preserve the released meanings:

```text
ALLOW_FACTS_AND_SOURCE
ALLOW_FACTS_METADATA_ONLY
ALLOW_FACTS_REDACT_LOCATION
DENY_FACTS_AND_SOURCE
```

Policy is compiled into FAB's reduced child catalog before semantic matching, cost/statistics,
planning, and negative-proof computation. Source/location redaction is applied before public
record/statement/error/trace/artifact construction. A query may be complete within the authorized
universe but cannot imply workspace-wide completeness when relevant data is denied.

The adapter cannot call mutation, schema/transformation change, activation, maintenance, cutover, or
administrative `FabricCommand` operations. Those live on separately authenticated daemon/
controller boundaries and are absent from tools, resources, prompts, query fields, and generic
snapshot methods.

## 13. Lifespan, middleware, and STDIO purity

`codefabric supervisor serve` owns one long-lived `WorkspaceSupervisor` per
workspace. Its private `supervisor.sock` accepts bounded launch/revoke/status
operations only, and its unnamed daemon-control socketpair accepts bounded
`RegisterLaunchGrant`, `RevokePrincipal`,
`AdvanceSupervisorGeneration`, and `Acknowledgement` records only. Neither
control channel carries semantic queries, Arrow, result, catalog, or Delta data.

The daemon endpoint of that unnamed socketpair is mapped safely to daemon stdin
through `OwnedFd`/`Stdio`; no pre-exec hook or ambient descriptor is permitted.
All unrelated descriptors remain close-on-exec. Control records bind workspace,
daemon/supervisor generation, monotonic sequence, operation, expiry, and content
integrity; gaps, replay, changed duplicates, unknown records, wrong bindings, or
channel replacement fail closed. The supervisor waits for daemon generation and
control acknowledgement before serving attach requests.

`codefabric mcp serve` is attach-only. `AgentStdioLauncher` asks the existing
supervisor for one policy-bound launch, starts exactly one installed adapter,
passes capability bytes only through allowlisted inherited fd 3, and directly
inherits host stdin/stdout so byte identity and OS backpressure are structural.
It never reads or rewrites MCP protocol bytes. Adapter stderr alone is bounded,
accounted, and forwarded diagnostically. Partial spawn, EOF, signal, timeout,
early exit, or control loss revokes unused authority, joins tasks, reaps owned
children, and leaves no unrelated inherited descriptor.

Before spawn the launcher creates a dedicated unidirectional capability
channel. The adapter reads one bounded generation-labelled grant frame at a
time and erases consumed capability buffers as far as safe-language ownership
allows. The channel is only launcher-to-adapter grant delivery: it cannot carry
adapter-minted claims or daemon control records, and EOF forbids further
handshake authority. While fd 3 remains valid, the supervisor may deliver a new
single-use grant after daemon generation changes.

The first fixed-descriptor implementation candidate is exact `command-fds`
0.3.3 with Tokio support. It must compile-probe and process-probe fd 3 as the
only extra child mapping on Linux and macOS while preserving first-party
`unsafe_code = "deny"`. Temporarily clearing `CLOEXEC` around spawn in the
multithreaded supervisor is forbidden. If no supported launcher can satisfy the
contract, the only permitted fallback is a one-shot file beneath the private
runtime root, opened without following symlinks, verified for expected owner
and mode `0600`, and unlinked immediately after the child opens it. That
platform choice must carry equivalent replay, path-substitution, logging,
cleanup, and distinguishing-fault proof; environment or argv delivery is never
allowed.

Adapter stderr uses a bounded forwarding policy with truncation and accounting;
secrets and grants are excluded at their source. The adapter directly observes
host EOF. Signals and deadlines propagate through the owned child process
group, and launcher exit joins stderr/lifecycle tasks and reaps the child. Any
partial spawn or launcher failure revokes the unused grant and closes every
created descriptor.

Adapter/launcher exit revokes session authority and releases launch-scoped
resources, but already accepted work follows its durable query and retention
contract. A surviving adapter reconnects with a new grant, gRPC channel, and
daemon session, then resumes `WatchQuery` by accepted query identity and bound
cursor; it never resubmits `StartQuery`. Adapter replacement creates a new
launch. Daemon restart advances generation and invalidates all volatile grants,
sessions, and cursors from the old generation; it reconstructs only durably
proved terminal/package state and requires fresh grants.

FastMCP lifespan constructs exactly one immutable settings object, validates public schemas,
opens/authenticates the daemon client, performs handshake, and exposes readiness. Dependency
injection supplies only settings, client, request context, and safe adapters; no global mutable
CPG/session/catalog object exists in Python.

Middleware order is explicit and tested: safe error boundary, trace/correlation, credential and
workspace admission, deadline/cancellation, timing/metrics, and structured logging. Retry is
limited to proven idempotent transport operations. Query response caching and uncontrolled
background tasks are disabled.

STDOUT is MCP protocol only. Human/debug/telemetry logging goes to STDERR or configured sinks and
is redacted. Client-visible progress/messages use FastMCP context and safe allowlists. Public
serialization is always through explicit Pydantic models; subclass fields or new daemon fields
cannot leak additively.

Startup order is settings -> public schema self-check -> daemon client/handshake -> component
publication -> readiness. Shutdown stops new calls, cancels/awaits in-flight operations, releases
resource reads where possible, closes the daemon channel, and completes FastMCP lifespan without
leaving detached work.

## 14. Fairness and observability

The adapter preserves daemon scheduling and does not create an unbounded local queue. Default
limits retain two active and four queued queries per agent, subject to the negotiated runtime
profile. Progress and result reads respect backpressure. One adapter cannot monopolize daemon
CPU, memory, spill, provider jobs, artifact bytes, or range reads.

Trace identity keeps semantic request ID, MCP call ID, RPC attempt ID, daemon query ID,
agent/workspace, epoch/snapshot, and artifact/read lease distinct and correlated. Metrics cover
tool counts/latency, validation, handshake/reconnect, cancellations, stream bytes/backpressure,
inline/resource selection, artifact reads/releases/expiry, and safe error codes. They never log
request source content or unrestricted semantic payloads by default.

## 15. Executable acceptance obligations

| Contract | Required executable oracle |
|---|---|
| released Protobuf and cross-language wire | `just proto-contract-check`; `just provider-protocol-check` |
| supervisor, policy, singleton, and descriptor lifecycle | `just supervisor-launch-contract-check`; `just supervisor-launch-platform-check` |
| strict adapter boundary and package contents | `just adapter-domain-boundary-check`; `just adapter-package-authority-zero-state-check` |
| all four tools and eight semantic forms | `just semantic-delivery-vertical-check`; `just semantic-query-conformance-check` |
| dynamic status/reference from epoch | `just dynamic-reference-delivery-check` |
| public schema/error/result compatibility | `just adapter-test`; `just package-interop-check` |
| inline/resource semantic equivalence | `just semantic-delivery-vertical-check`; result-resource contract tests |
| accepted handle, watch resume, cancellation, and relative budget | `just adapter-test`; `just resource-governance-check` |
| ACL, bound catalog, and noninterference | `just access-catalog-isolation-check`; `just public-leakage-negative-check` |
| no Python semantic/data-plane authority | `just adapter-domain-boundary-check`; `just daemon-static-bundle-target-zero-state-check` |
| real STDIO protocol purity | `just adapter-stdio-test`; `just adapter-ci-fast` |

Tests include in-memory FastMCP, fake-daemon contract, real generated stubs, real STDIO,
daemon integration, cancellation at every lifecycle stage, reconnect/resume, slow consumer,
large resource/range reads, expiry/release, cross-agent/workspace/credential denial, package
installation, additive daemon fields, hostile error payloads, and public four-layer decoded
semantic equivalence. A static package fingerprint, tool count, self-generated expected output,
or execution capture alone is not acceptance.

Supervisor/launcher tests additionally reject wrong policy-file owner, mode,
type, device, inode, symlink or substituted path; unsafe pre-existing runtime or
socket objects; wrong peer UID, supported PID/start identity, policy, workspace,
operation, generation, expiry, revocation, replay, or capacity; descriptor
leakage; control/rendezvous loss; and partial spawn. They prove one live
supervisor/daemon with multiple authorized adapters, bounded stderr and direct
STDIO backpressure, joined/reaped children, accepted-work survival, and, if
selected, a `0600` fallback that cannot be read twice or reached through a
substituted path.
