---
artifact: interface-design-review
date: 2026-09-01
version: v4
status: complete
supersedes: docs/reviews/interface_design_review_daemon_grpc_fastmcp_boundary_2026-08-31_v3.md
interface_path: docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.1.md
serving_specification: docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.1.md
lifecycle_specification: docs/authoritative_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v2.1.md
fabric_specification: docs/authoritative_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v2.1.md
principles_path: docs/library_ref/full_data_fabric_design_principles_v2.md
plan_path: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v4_2026-09-01.md
reviewed_head: f12329f05e3678698ff9a43ec4f69f95f42db12f
working_tree: dirty-pre-existing-and-in-progress
baseline: intentionally-not-taken
verdict: revision-required
target_status: accepted
---

# Production daemon composition through gRPC and FastMCP: forward-only target amendment

## 0. Decision

The target in
`docs/reviews/interface_design_review_daemon_grpc_fastmcp_boundary_2026-08-31_v3.md` remains
accepted except for its compatibility-preservation decision. The owner has explicitly removed old
client/server operability as a requirement because CodeFabric is still in the design phase.

The sole target is now the best forward design for the intended functional capabilities. The
implementation must not retain old method overlap, repeated identity claims, legacy strings,
legacy cursors, wall-deadline compatibility, a v1 negotiation profile, a translator, or generated
v1 runtime code merely to preserve operability. Released v1 artifacts remain immutable historical
evidence and allocation provenance only; they are not a production interface, implementation
dependency, package payload, acceptance oracle, or fallback.

All v3 decisions concerning compiled semantic authority, phase-typed startup, lawful genesis,
atomic active workspaces, DataFusion/Arrow/Delta execution, one query coordinator, bounded Arrow
page packages, lifecycle truth, UDS ownership, presentation-only FastMCP, fresh activation,
decommission, proof, and performance remain unchanged.

## 1. Forward-only RPC authority

Create one new production package, `codefabric.cpgd.v2`, with one statically generated service and
the following surface:

| RPC | Cardinality | Sole responsibility |
|---|---|---|
| `Handshake` | unary | Consume an authenticated single-use launch grant, negotiate the current v2 minor/features/semantic profile, report lifecycle state and enforced limits, and mint an expiring daemon-generation session. |
| `GetStatus` | unary | Return a cheap typed lifecycle/readiness/queue/current-epoch summary from the one lifecycle/coordinator authority. |
| `GetReference` | unary | Resolve one authorized live program, capability, schema, guide, or reference projection by a typed selector. |
| `ValidateQuery` | unary | Strictly validate one canonical semantic request and its capability/cost/freshness constraints without starting execution. |
| `StartQuery` | unary | Normalize the full operation, reserve bounded capacity, apply idempotency, and return the original acceptance record. |
| `WatchQuery` | unary to stream | Start or resume bounded control-event observation from an optional content-bound cursor; reconnect never restarts the query. |
| `CancelQuery` | unary | Idempotently cancel one accepted query under a bounded cleanup budget. |
| `ReadResource` | unary to stream | Read one authorized immutable bounded page/range from a sealed result/reference resource with transport backpressure. |
| `ReleaseResource` | unary | Idempotently release one resource lease and stabilize races with a bounded tombstone. |

Also serve `grpc.health.v1.Health` for process/service liveness only. Production reflection remains
disabled. A future incompatible contract creates `codefabric.cpgd.v3`; it does not reinterpret v2
field numbers or method meanings.

### 1.1 Clean message semantics

- Requests after `Handshake` carry one opaque binary session in fixed `*-bin` metadata. They do
  not repeat authoritative principal, agent, workspace, permission, daemon generation, or session
  IDs in message bodies.
- Closed state, event, queue, terminal, cleanup, release, error, and capability vocabularies are
  enums/oneofs with explicit presence. There are no compatibility prose fields whose old meaning
  internal code must continue to consume.
- `StartQuery` carries one relative `google.protobuf.Duration` execution budget. Absolute wall
  timestamps are observations only and never cancellation authority.
- `WatchQuery` replaces overlapping initial-stream/attach methods. Its opaque cursor binds query,
  principal/session class, daemon generation, selected profile, next sequence, preceding event
  content, and expiry.
- Query control events are `SnapshotPinned`, coalescible `Progress`, `ResultReady`, and `Terminal`.
  They contain no result bytes. `ResultReady` names the one sealed result package.
- `ReadResource` addresses immutable package, manifest, page, reference, or bounded projection
  resources through typed descriptors. It never exposes filesystem/object-store locations.
- Semantic QRY request/response values remain application-owned canonical bytes and typed result
  relations. Protobuf is control/lifecycle/resource transport, not a second semantic DTO graph.
- A digest, MAC, descriptor identity, or deterministic encoding proves identity, integrity,
  authentication, or compatibility only. It never proves semantic correctness.

### 1.2 Session root and supervisor control

The authority root is an operationally authenticated launch grant, not a self-declared adapter ID
or a reusable environment token:

```text
private runtime directory and same-UID peer credential
  -> inherited supervisor control socketpair
  -> RegisterLaunchGrant / RevokePrincipal / AdvanceGeneration / Acknowledge
  -> single-use bootstrap capability delivered through inherited descriptor
  -> Handshake consumes grant and mints session
  -> every RPC reauthorizes operation/workspace/query/resource
```

The supervisor-control schema is minimal, versioned, length-bounded, and contains no semantic
query/result data. Its volatile grant/session state is bound to daemon generation and is never
recovered from Delta, SQLite, repository files, argv, or ordinary environment variables. If a
supported launcher cannot pass an inherited descriptor, the only fallback is a no-follow,
owner-verified `0600` file in the private runtime directory, unlinked immediately after read.

Every session binds peer UID and supported PID observation, daemon/supervisor generation,
principal, workspace set, operations, selected profiles/features, host bounds, issue/expiry,
revocation generation, and anti-replay identity. Restart or revocation invalidates it. The adapter
reconnects with one replacement channel and a new handshake; accepted work is watched by query
identity and is never resubmitted implicitly.

## 2. Query, result, and error behavior

`StartQuery` reserves coordinator, journal, idempotency, task, result, and retention capacity before
it returns an accepted handle. The normalized idempotency identity includes every field that can
change operation meaning. Same key plus same normalized operation returns the original acceptance;
same key plus any changed bound field returns typed conflict.

`WatchQuery` is ordered at least once within a bounded replay window. Progress is coalesced before
sequence allocation; snapshot, result-ready, and terminal events are not discarded. Dropping the
watch cancels observation only. `CancelQuery` cancels work. Restart marks unsealed in-flight work
`LOST` unless durable recovery proves the exact retained terminal/package; it never reruns under a
different epoch.

Outer transport failures use standard gRPC status plus one stable allowlisted typed detail or
trailing-metadata code selected after an exact Rust/Python probe. Python branches on status and
typed code, never prose. Semantic gaps, unknowns, partial blocks, and per-block errors remain
successful semantic response data when the outer request was processed correctly.

DataFusion execution streams to bounded, independently decodable Arrow IPC pages. Partial pages
stay private; the manifest is published last, and one result package owns the response envelope,
manifest, page objects, leases, release, tombstones, and cleanup. FastMCP materializes at most one
negotiated bounded projection/page per resource call and never assembles a complete relation.

## 3. FastMCP target

FastMCP lifespan creates one `grpc.aio` channel, waits within a bounded connection budget,
handshakes, validates the selected v2 profile/reference index, and only then yields. A valid v2
`BOOTSTRAPPING` response is allowed; transport, authentication, or profile failure aborts startup.

The public catalog remains exactly four tools:

1. `query_code_graph`;
2. `validate_code_graph_query`;
3. `get_code_graph_status`;
4. `get_code_graph_reference`.

Manifest/page/reference access uses bounded resources. `Context.report_progress` projects
coalesced daemon progress. Strict/frozen/extra-forbid Pydantic models and module-level adapters own
presentation validation/schema only. Python owns no semantic registry, no mutable CPG, no Arrow
interpretation, and no `orjson` serialization policy.

## 4. Decommission consequence

After the v2 production service and installed adapter vertical pass, remove from live source,
builds, packages, recipes, services, generated includes, and tests:

- `codefabric.cpgd.v1` server/client selection and generated runtime bindings;
- `StreamQuery`, `AttachQuery`, `ReadResult`, and `ReleaseResult` production routes;
- repeated body principal/workspace/session authority;
- legacy string state/error/result fields, wall-deadline authority, sequence/checksum cursors, and
  old-client feature profiles;
- any translator, shim, dual-service registration, compatibility fallback, or fixture required
  only to keep v1 operational.

Preserve only versioned v1 `.proto`/descriptor/fixture history under an explicitly non-live
historical class. No target correctness gate invokes a v1 client or server. Unknown-field and
allocation lessons inform v2 schema tests, but v1 agreement is not an acceptance oracle.

## 5. Plan and proof consequences

Implementation plan v4 must:

1. version the authoritative suite for the forward-only `codefabric.cpgd.v2` target before wire
   implementation;
2. treat v2 source, descriptor, Rust/Python generation, service, client, adapter, package, and tests
   as one dependency-closed contract transaction;
3. use the real v2 binary/client/package in the source-to-FastMCP terminal vertical;
4. prove session/peer/revocation/generation/workspace/operation/handle/resource denial, bounded
   cursors/journals/pages, reconnect/watch behavior, and owned-socket lifecycle;
5. delete v1 live operability and dormant forward-cutover machinery rather than construct
   compatibility or handoff paths; and
6. certify decoded semantic behavior, exact durable readback, causal faults, resource bounds, and
   target-only zero state rather than legacy agreement.

Reopen design only if the clean v2 method/resource/session model cannot express a required
functional capability, the supervisor grant root cannot be implemented on a supported launcher,
or a genuinely deployed predecessor is discovered. None authorizes a compatibility shim by
default.
