---
artifact: interface-design-review
date: 2026-08-30
version: v2
status: complete
interface_path: docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.0.md
principles_path: docs/library_ref/full_data_fabric_design_principles_v2.md
plan_path: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md
baseline_commit: db67f7cbbd1ce96e7d7a98a790a0a5ef246fbc34
working_tree_digest: 878af321fae777b64c0563dff36f8dae6bee00c379cf6a9fc9b1f6b6810e062d
verdict: redesign-required
---

# Daemon gRPC to FastMCP boundary: independent design review and target architecture

## 0. Executive decision

The foundational topology is right and should be retained:

```text
one agent
  -> one presentation-only FastMCP STDIO process
  -> one long-lived grpc.aio channel over a private UDS
  -> one authoritative Rust daemon per workspace
  -> one immutable, authorized FabricEpoch per query
```

The steady-state interface, however, should be redesigned before the serving work in the active
implementation plan is completed. The present wire shape combines control-event replay, response
delivery, and result storage too tightly; the adapter still packages semantic registries that the
daemon is meant to own; several lifecycle and authorization fields repeat caller claims rather than
derive from an authenticated session; and the apparent streaming paths currently materialize or
retain whole results.

The recommended target is a **typed control plane plus immutable resource plane**:

- Protobuf owns only transport, lifecycle, identity, range, and closed control vocabulary.
- Canonical JSON remains the daemon-owned semantic request and public response contract.
- Arrow remains the daemon-owned relational result representation.
- Query events carry state changes only; they never carry result bytes.
- Every completed result is published once into one immutable resource registry.
- Small inline MCP output is a bounded read of the same canonical resource, not a second result
  route.
- Large output is exposed as manifests and hard-bounded resource pages/ranges because FastMCP
  resources materialize their return value and are not an unbounded byte-stream transport.
- The FastMCP tool schema remains stable and small. Released schemas and live capabilities are
  introspected from the daemon through digest-addressed reference resources.
- A supervisor-issued, single-use launch grant creates a short-lived, daemon-instance-bound session;
  ordinary calls derive agent/workspace identity from that granted principal instead of trusting
  repeated body fields.
- One application scheduler owns query admission, fairness, quotas, cancellation, and retention.
  Tower enforces only transport abuse ceilings and preserves a cheap control lane.

This is a structural performance design, not a benchmark claim. It removes known unbounded copies,
replay growth, and duplicated routes while deliberately leaving compression, HTTP/2 window,
keepalive, queue-size, and spill-threshold tuning to representative measurement.

### Decision summary

| Area | Decision |
|---|---|
| Topology | Preserve private UDS gRPC and the presentation-only FastMCP process. |
| Breaking wire evolution | Introduce `codefabric.cpgd.v2`; retain v1 files as immutable compatibility history. |
| RPC shape | Nine focused operations: handshake, status, reference, validate, start, watch, cancel, read resource, release resource. |
| Query delivery | Accepted handle first; resumable control-only watch; immutable resource after completion. |
| Inline delivery | Adapter performs one bounded read of the canonical public JSON resource. |
| Arrow delivery | Range/page resources from a streaming daemon sink; no Python assembly of whole relations. |
| Introspection | Stable released contract projections plus separate epoch-live capability/proof projections. |
| Schema ownership | `.proto` owns transport numbers; released JSON schemas own semantic grammar; Pydantic owns only the MCP envelope. |
| Authentication | Supervisor-issued one-time grant, UDS peer check, handshake-minted session metadata, and object/data authorization. |
| Admission | One WP19 application scheduler; transport middleware is not a second scheduler. |
| Performance posture | One warm channel, bounded messages and buffers, selective Rust `Bytes`; no Flight, TLS, gRPC compression, or custom window tuning without evidence. |
| Plan impact | If this redesign is accepted, the entire active plan becomes stale: pause execution, publish the serving-design successor, then version and independently re-audit the plan before any execution resumes. |

The existing v1 review at the adjacent path was treated as ideation, not as an authority or a
baseline to preserve. This document independently re-derived the boundary from the governing
serving specification, the v2 principles, the post-plan target, the current tree, and the pinned
library capabilities. Where the recommendations differ, this document states the reason.

---

## 1. Review basis and practical boundary

### 1.1 Scope

This review covers the complete interface path between the daemon application boundary and the MCP
consumer:

- `contracts/rpc/cpg_query_service.proto` and its descriptor-set generation;
- Tonic service construction, UDS transport, authentication, query lifecycle, event replay, result
  publication, range reads, cancellation, admission, and observability;
- the generated Python gRPC bindings and `grpc.aio` client lifecycle;
- the FastMCP tools, resources, prompts, progress projection, error mapping, and public Pydantic
  envelopes;
- schema, vocabulary, version, capability, status, guide, recipe, and proof introspection;
- compatibility migration from the current released v1 meanings to the proposed v2 service.

Provider extraction, semantic query compilation, DataFusion plan construction, Delta persistence,
and lifecycle commands are not redesigned here. They are constraints: this boundary must carry
their authoritative outcomes without acquiring a second semantic implementation.

### 1.2 Governing sources

The normative design is `SRV`,
`docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.0.md`, especially:

- `SRV §§0–3`: authority, topology, the four contract planes, and adapter package posture;
- `SRV §§4–6`: negotiation, accepted handles, and lifecycle;
- `SRV §7`: four tools plus resources and prompts;
- `SRV §§8–10`: progress, reconnect, one logical response, and immutable resources;
- `SRV §§11–14`: error/status separation, authorization, lifespan, fairness, and observability;
- `SRV §15`: executable acceptance obligations.

The doctrine lens is
`docs/library_ref/full_data_fabric_design_principles_v2.md` (`PRIN`), including the staticness
test and P1–P36. The post-plan target is
`docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v2_2026-08-29.md`
(`PLAN`), particularly WP04, WP15, WP19, WP20, DB03, DB04, and the replan policy in §9.

The library decisions use the exact repository references routed by the requested skills:

- `rust_grpc_daemon_advanced_reference_tonic_0.14.6.md` (`tonic-ref`);
- `grpcio_python_advanced_reference_1.83.0.md` (`grpcio-ref`);
- `protobuf_python_advanced_reference_7.36.0.md` (`protobuf-ref`);
- `orjson_python_advanced_reference_3.12.0.md` (`orjson-ref`);
- `fastmcp_python_advanced_reference_3.4.7.md` (`fastmcp-ref`);
- `pydantic_python_advanced_reference_2.13.4.md` (`pydantic-ref`);
- `datafusion55_arrow59_design_principle_alignment_manual_2026-08-24.md`
  (`datafusion-arrow-ref`).

The dependency pins are not repeated here as authority; `FAB §2.1` remains the only authoritative
pin ledger.

### 1.3 Current-versus-target status

This review is intentionally not a release assessment of the current tree. At the review baseline:

- HEAD was `db67f7cbbd1ce96e7d7a98a790a0a5ef246fbc34`;
- the worktree already contained extensive user-owned changes and untracked target modules;
- execution state reported WP01 in progress, while WP04, WP15, WP19, and WP20 were not started;
- new Arrow publication structures existed, but the production daemon still used substantial
  predecessor serving paths.

Consequently, each finding below is classified as one of:

- **preserve** — a sound current foundation;
- **close in the active plan** — already an explicit target obligation;
- **new design delta** — changes the accepted interface and therefore requires a design successor
  and plan revision;
- **measure** — an optimization decision that must not be frozen without a workload.

No state label or partially implemented module is treated as proof that an end-to-end behavior is
complete. This document establishes a design target and proof obligations; it does not certify
implementation or release readiness.

### 1.4 Evidence method

Positive current-state claims were checked against source, generated contracts, package metadata,
and the active plan. Negative claims were bounded to named paths and combined textual search with
candidate inspection. Library claims come from the version-pinned repository references rather
than general recollection.

The `working_tree_digest` records the reviewed dirty-tree snapshot before this v2 file was added.
Line anchors in findings are evidence aids for that snapshot; section citations to normative design
remain the durable authority.

---

## 2. Non-negotiable constraints

The target must preserve these invariants even when a locally faster-looking alternative exists.

1. **One semantic owner.** Rust owns validation, query meaning, authorization, epoch selection,
   execution, result construction, capability state, and resource leases. Python cannot replay or
   infer any of them (`SRV §§1–3`; `PRIN` P3, P6, P23, P34).
2. **One immutable query world.** Every accepted query binds exactly one authorized child catalog
   and one immutable FabricEpoch. Reconnect and resource reads cannot silently reselect current
   state (`SRV §§5–6, 10, 12`; `PRIN` P11, P17, P23).
3. **No judgment facts.** Presentation may explain status and facts, but cannot invent risk,
   impact, safety, or recommendation semantics.
4. **Unknown is explicit.** Absent provider capability, unavailable epoch-live projection, partial
   relation, expired cursor, and unsupported delivery remain typed states rather than empty success.
5. **Control and semantics stay separate.** Protobuf is the static control protocol. Canonical JSON
   is the released semantic request/response protocol. Arrow is the relational data plane.
6. **Accepted work is recoverable.** `StartQuery` returns a durable logical handle before expensive
   work. A transport disconnect does not erase or duplicate the accepted operation.
7. **No unbounded materialization.** No daemon event vector, Tonic queue, Python list, `bytes.join`,
   FastMCP resource, or Pydantic model may scale without a named hard bound.
8. **Introspection does not become authority.** Descriptor reflection describes the gRPC control
   service. Semantic reference projections explain the query system. Neither dynamically executes
   arbitrary schema nor becomes a second validation engine.
9. **Repeated data has a declared disposition.** Repetition is permitted only when it is generated
   from one owner or is a load-bearing routing/integrity echo derived atomically from the same
   outcome.
10. **Performance claims are measured.** Compression, Flight, HTTP/2 tuning, task offload, caching,
    spill thresholds, and page sizes are selected by the acceptance workload, not folklore.

---

## 3. Authority, representation, and version model

### 3.1 Information ownership matrix

| Information | Sole authority | Derived appearances allowed | Forbidden coequal copy |
|---|---|---|---|
| RPC methods, cardinalities, field numbers, transport enums | released `.proto` plus normalized service descriptor closure | Rust/Python generated stubs; startup/test descriptor assertions | model compiler or handwritten Python transport registry |
| supervisor grant/revoke/ack records | versioned supervisor-control record schema | supervisor/daemon codecs and audit-safe summaries | query `.proto`, FastMCP model, or ad hoc environment/body fields |
| semantic request grammar, stable form/vocabulary IDs and meanings | released QRY JSON Schema and daemon validator | digest-addressed reference resource; human guide; stable tool documentation | Pydantic recreation of query forms or Proto messages per form |
| semantic response/status dimensions | released public JSON schema plus Rust result construction | bounded MCP projection; resource manifest/index echoes | terminal strings independently assembled in Rust/Python |
| current capability/form/vocabulary membership, availability, aliases, provenance, and proof state | admitted daemon model/FabricEpoch | authorized live reference projections and cache index | wheel-packaged registry census or redefinition of released meanings |
| Arrow schemas and relation metadata | admitted model/FabricEpoch and FAB schema authority | IPC schema, resource manifest, presentation summary | Python Arrow-schema reconstruction |
| enforced limits and queue state | one runtime scheduler/quota configuration | handshake/status snapshots tagged with daemon instance and expiry | adapter constants presented as daemon capacity |
| principal/workspace/permission grant | workspace supervisor launch-grant authority | consumed grant record and daemon session claims | adapter instance ID or caller-supplied workspace/agent identity |
| session credential, expiry, and enforcement | daemon session authority constrained by supervisor grant/revocation generation | typed request extension and redacted trace fields | FastMCP/Pydantic session state machine or transport-only peer identity |
| result identity and bytes | immutable daemon resource registry | descriptor, digest, byte count, MIME type, resource links | legacy artifact map plus separate Arrow registry |
| MCP presentation envelope | strict Pydantic/FastMCP declarations | JSON Schema generated in serialization mode | semantic validation or daemon lifecycle state |

`PRIN` P3 does not mean that a value may appear only once. It means one owner decides it. A total
byte count in a resource descriptor and an HTTP-like range response, for example, is a useful
derived echo. It is retained only if both are constructed from one sealed resource record and are
checked for equality. Independently maintained copies are removed (`PRIN` P26, P27, P30, P31).

### 3.2 Staticness classes

The target uses the `PRIN` staticness test explicitly.

| Class | Examples | Publication behavior |
|---|---|---|
| Class 1: released and immutable | RPC descriptor closure, released request/response schemas, stable control enums, guide template, public error-detail types | available before an epoch; versioned, retained, digest-addressed |
| Class 2: live and reconstructible | effective limits, current query-form capability, current vocabularies, proof status, authorized catalog, resource inventory | served from daemon state with epoch/instance/proof identity and explicit unavailable/unknown states |
| Class 3: operation-local | accepted handle, event cursor, progress, lease, cancellation state | scoped, expiring, owner-bound, never packaged |

The public query language is Class 1; whether a given form is currently executable is Class 2.
Changing the FastMCP tool schema whenever an epoch changes would incorrectly merge those classes
and would destabilize MCP host schema caches.

### 3.3 Compatibility axes

The following versions are independent. Their values may coincide, but they must never be inferred
from each other.

| Axis | Owner | Compatibility question |
|---|---|---|
| design-suite identity | `SUITE` manifest | Which normative design release applies? |
| RPC package/version | released `.proto` | Can these peers decode and honor method/field meanings? |
| descriptor-closure digest | normalized service closure | Are the exact generated control contracts equivalent? |
| semantic QRY profile | QRY release authority | Which request meanings are accepted? |
| public JSON schema identity | released schema artifact | Which input/output structure is being validated? |
| MCP component/catalog version | FastMCP presentation package | Which stable four-tool envelope is exposed? |
| adapter distribution version | Python package metadata | Which client implementation is installed? |
| daemon build and instance | daemon release/session authority | Which process accepted this session/query? |
| interface snapshot identity | daemon reference publisher | Which static/live projection set was observed? |
| FabricEpoch | lifecycle/data-fabric authority | Which immutable semantic/data world answers the query? |

The handshake returns these axes explicitly where relevant. It never treats `codefabric.cpgd.v2`
as proof of a suite, QRY profile, schema, or epoch version.

---

## 4. Present-state findings

### 4.1 Foundations to preserve

- The single-workspace-daemon/private-UDS/presentation-only-adapter topology matches `SRV §§1–3`.
- The UDS path is protected by a private directory, restrictive mode, and same-UID peer checking in
  `src/rpc.rs`.
- One pinned Protobuf compiler invocation produces Python code and the FileDescriptorSet consumed
  by Rust `compile_fds` in `tooling/proto/generate.rs:25–32`.
- The service already uses the right broad lifecycle idea: validate, accepted handle, observe,
  cancel, read, release.
- The Python process reuses one `grpc.aio` channel and generated stub.
- Canonical request JSON stays opaque at the transport boundary; the adapter does not implement the
  semantic query engine.
- The emerging Arrow result registry separates content identity from opaque authorization/lease
  tokens and has the beginnings of expiry collection.
- Public Pydantic envelopes are strict/frozen/extra-forbid rather than permissive semantic mirrors.

These are architectural assets. The target refines them rather than replacing gRPC, moving query
execution into Python, or creating a new service process.

### 4.2 Findings and required dispositions

| ID | Severity | Current evidence | Why it matters | Disposition |
|---|---|---|---|---|
| IR-IF01 | high | `server.py:18–19,154–185,356–367` imports packaged registries, schema manifests, fingerprints, and synthesizes reference content. | The adapter owns a stale semantic census and cannot describe the admitted epoch honestly. | WP15/DB03 plus target §7. |
| IR-IF02 | high | Handshake verifies `credential_proof`, while ordinary methods authorize repeated body `workspace_id` (`query_service.rs:757–805,2020,2106,2145`). | Same-UID transport identity is not per-agent/session authority; caller claims are duplicated. | New design delta: session metadata and layered authorization. |
| IR-IF03 | high | Query state retains `Vec<QueryEvent>` (`query_service.rs:1083`); useful production events are appended after work; progress/result-chunk variants are unused. | Replay is unbounded and not a live, flow-controlled observation surface. | New control journal in target §6. |
| IR-IF04 | high | Both result routes implement server streaming with `stream::once` (`query_service.rs:2512,2556`). | Cardinality advertises streaming but the implementation is a one-chunk range response. | True bounded `ReadResource` streaming. |
| IR-IF05 | high | Python loops over one-chunk calls then joins all chunks (`daemon/client.py:518–692`; `arrow_resources.py:568–599`). | Peak adapter memory scales with result size; FastMCP then adds materialization/base64 overhead. | Bounded inline only; independently decodable pages or explicitly opaque ranges for large bytes. |
| IR-IF06 | high | Legacy artifact records/lease cache coexist with `PublishedArrowResultRegistry`; production construction does not call `with_published_results`. | There are two result authorities and the stronger registry is not the production composition root. | WP15/WP19: one registry and one wiring path. |
| IR-IF07 | high | `ResultRelationInput` retains batches before materializing Arrow IPC; the adapter can reassemble whole relations. | A 1 GiB hard limit can still imply multi-GiB transient memory and copies. | Stream batches into bounded memory/spill sink; never collect by default. |
| IR-IF08 | high | Idempotency is indexed only by a caller key (`query_service.rs:2175–2260`). | A reused key can return a handle for different semantic input and synthesized acceptance metadata. | Bind key to normalized operation identity and return typed conflict. |
| IR-IF09 | high | Event checksum is derived from `query_id:sequence` (`query_service.rs:1444`). | The cursor does not bind event content, principal, expiry, or daemon generation. | Authenticated/bound resume cursor and released event-content digest frame. |
| IR-IF10 | high | Reported queue class and several states are strings; the `.proto` duplicates descriptor fields and declares unused compression. | Closed control vocabularies admit invalid combinations and drift across Rust/Python. | New v2 typed control messages; reserve removed fields. |
| IR-IF11 | medium | The RPC surface has separate `StreamQuery` and `AttachQuery`; Python never attaches. | Two methods duplicate nearly identical streaming semantics without delivering recovery. | One resumable `WatchQuery` request. |
| IR-IF12 | high | FastMCP lifespan yields before the first handshake (`server.py:74`); socket existence is used as daemon readiness. | Tools may be published against an incompatible or unready daemon. | Eager bounded handshake before lifespan yield; health only for liveness. |
| IR-IF13 | high | Query deadline/admission fields exist, but active scheduling and remaining-budget propagation are incomplete; queue class is decorative. | Advertised limits do not prove enforced limits; transport waiting can consume the accepted-work budget. | One WP19 scheduler and monotonic execution budget. |
| IR-IF14 | medium | Rust and Python independently define frame, chunk, inline, and relation thresholds (`rpc.rs`, `settings.py:58–62`, `client.py:594–601`). | A policy preference is easily mistaken for daemon capacity. | One enforcer; effective value is a negotiated minimum with named sources. |
| IR-IF15 | high | The daemon can remove an existing socket path and treats path existence as readiness (`daemon.rs:1217–1267`). | It can unlink a live endpoint or publish a stale/unready one. | Generation lock, safe stale probe, inode-aware unlink, protocol readiness. |
| IR-IF16 | medium | Error translation parses gRPC detail strings for some resource states; semantic and transport status are also repeated in terminal strings/JSON. | Callers couple to prose and may confuse outer failure with semantic partial success. | Typed allowlisted rich details; canonical semantic failure remains data. |
| IR-IF17 | medium | `mcp_call_id` is overwritten with `daemon_query_id` (`server.py:256`). | Cross-layer traces cannot distinguish invocation, RPC attempt, accepted operation, and resource. | Typed correlation model in target §9. |
| IR-IF18 | medium | The query tool accepts `dict[str, Any]`, but the live schemas/capabilities learned at handshake are not projected; no required prompts are registered. | The adapter is thin but not introspectable, so an agent cannot reliably author or interpret requests. | Stable envelope plus daemon-derived references and two prompts. |
| IR-IF19 | medium | Python packaging says `>=3.12`, while source/tooling use a 3.14 baseline. | A wheel may claim support for an interpreter on which it cannot import or pass gates. | Make package metadata match the proved runtime floor. |
| IR-IF20 | medium | Completed query/session/idempotency/task maps have no complete owner-wide retention lifecycle; Arrow `collect_expired` is not production-wired. | A long-lived daemon accumulates state even if individual resources have TTLs. | One lifecycle supervisor and explicit retention table. |

### 4.3 Root cause

Most findings share one root cause: the current boundary treats a **query response** as both a
control-stream event and an artifact, while the adapter compensates for incomplete daemon
introspection with static package data. That produces three coupled systems:

```text
control replay owns some result delivery
artifact registry owns other result delivery
adapter package owns a copy of interface meaning
```

The target removes the coupling:

```text
control journal owns lifecycle observation only
resource registry owns every result/reference byte
daemon reference publisher owns interface meaning
FastMCP owns only presentation and discovery
```

---

## 5. Target topology and responsibility

```text
workspace supervisor / launcher authority
  |-- launches daemon with one unnamed private supervisor-control socket
  |-- registers grant digest + claims, waits for daemon acknowledgement
  `-- launches adapter with the one-time raw grant on an inherited descriptor
             |                                      |
             |                                      v
             |                            FastMCP adapter below
             v
Rust daemon supervisor-control boundary
  - fixed minimal grant/revoke/ack schema
  - supervisor generation + monotonic sequence
  - volatile daemon-generation-bound grant table
  - no public pathname and no query/semantic messages

MCP client / agent
  |
  | tools/list, tools/call, resources/read, prompts/get
  v
FastMCP 3.4.7 presentation process
  - four stable tools
  - fixed resource templates
  - two dynamic prompts
  - strict stable Pydantic envelopes
  - ResourceLink responses for non-inline bytes
  - no semantic registry, scheduler, artifact store, or query engine
  |
  | one process-lifetime grpc.aio channel + generated v2 stub
  | binary session metadata + trace metadata
  v
private workspace UDS
  - private parent directory, restrictive socket mode
  - kernel same-UID peer check
  - bounded control messages
  v
Tonic transport boundary
  - generated static service
  - auth/session middleware and typed request context
  - cheap control-lane protection
  - gRPC health; diagnostic reflection only when enabled
  |
  v
Rust application boundary
  +-----------------------+-------------------------+
  | query coordinator     | interface publisher     |
  | - accepted handles    | - released projections  |
  | - one scheduler       | - live epoch projections|
  | - cancellation tree   | - digest/index/cache    |
  | - bounded journals    |                         |
  +-----------------------+-------------------------+
  | one immutable resource registry                 |
  | - canonical public JSON                         |
  | - result manifest                               |
  | - Arrow relation/page resources                 |
  | - reference artifacts                           |
  | - owner/token/lease/TTL/range authorization     |
  +-------------------------------------------------+
  |
  v
authorized DataFusion child catalog pinned to one FabricEpoch
```

The supervisor-control and Tonic/application labels are two ingress boundaries of the **same Rust
daemon process**, not two daemons or a new package/build domain.

No internal Rust module boundary is prescribed here. The architecture names responsibilities and
contracts, not a file/folder decomposition, consistent with the repository scope boundary.

---

## 6. Exact gRPC v2 contract

### 6.1 Why a v2 package is required

The current `codefabric.cpgd.v1` meanings are released compatibility inputs under `SRV §0`.
Removing repeated body identity, deleting response bytes from query events, replacing overlapping
watch methods, and changing string state into closed typed control shapes are behaviorally breaking.
They must not be smuggled into reused field numbers or changed method meanings.

Create a new `codefabric.cpgd.v2` service. Preserve v1 `.proto`, descriptor fixtures, and interop
evidence as immutable history. A temporary v1 translator may coexist only during a measured,
explicitly fenced migration; it adapts into the same internal application services and must not
create dual query or result semantics.

### 6.2 RPC surface

| RPC | Cardinality | Responsibility and retry rule |
|---|---|---|
| `Handshake` | unary | Authenticate bootstrap capability, apply RPC-major/minor/required-feature compatibility, report service-closure identity, negotiate profiles/host bounds, and mint a session. Retry only before a session/query exists and within startup budget. |
| `GetStatus` | unary | Cheap, bounded liveness/readiness/queue/current-epoch summary. Safe to retry under a short deadline. |
| `GetReference` | unary | Resolve one authorized interface projection or resource descriptor by stable selector/digest. Safe to retry by request identity. |
| `ValidateQuery` | unary | Daemon-owned semantic/capability/cost validation without execution. Safe to retry if the requested epoch/freshness identity is fixed. |
| `StartQuery` | unary | Atomically register accepted/queued work and return its original acceptance record. Never blindly retry; replay only with an idempotency identity bound to the full operation. |
| `WatchQuery` | unary to stream | Start or resume control events from an optional bound cursor. Transport reconnect reopens this RPC; it does not restart the query. |
| `CancelQuery` | unary | Idempotently request cancellation of one accepted query. Safe to retry within a short cleanup deadline. |
| `ReadResource` | unary to stream | Stream one authorized bounded byte range/page as one or more chunks with real flow control. Retry only by immutable offset/digest. |
| `ReleaseResource` | unary | Idempotently release one lease; tombstone stabilizes duplicate/race outcomes. |

Standard `grpc.health.v1.Health` is additionally served. Health proves process/service liveness; it
does not replace semantic compatibility, session establishment, epoch readiness, or authorization.
Tonic reflection is compiled only in a development/diagnostic feature and remains disabled by
default in production (`tonic-ref §§30–31`).

### 6.3 Handshake

The request contains only bootstrap and negotiation facts:

- random adapter instance identity for diagnostics, not authority;
- binary launcher capability in `*-bin` metadata, not a logged JSON/body string;
- supported v2 RPC minor range and feature bits, plus the adapter's observed service-closure digest;
- supported semantic QRY profiles and released public-schema digests;
- launcher-known MCP host bounds and feature claims;
- adapter distribution/component identities.

The response contains:

- daemon build identity and random daemon-instance generation;
- selected RPC/profile/schema identities;
- compact released reference-index identity;
- effective enforced control/chunk/session limits with origin and expiry;
- readiness/degraded state with typed reason references;
- a short-lived opaque session credential bound to peer UID, daemon instance, adapter instance,
  workspace, permissions, negotiated profile, and expiry.

Runtime compatibility is decided by equal RPC major, an overlapping supported minor range, every
required feature being understood, and the selected semantic profile/schema policy. Exact
service-closure equality is a co-release/build oracle and a useful fast-path diagnostic, not the
runtime compatibility algorithm: a tested additive minor may legitimately have a different
descriptor digest. The compatibility window is fixed by immutable N/N-1 fixtures and explicit
feature rules, never by “decode succeeded” or digest equality alone.

The root of application identity is the **workspace supervisor launch grant**, not the adapter's
self-declared instance ID. For each authorized adapter launch, the supervisor creates a high-entropy,
single-use bootstrap capability bound to workspace, granted agent/principal identity, permissions,
daemon generation, issuance/expiry, and a nonce. Before launching the adapter, it deposits only the
token digest and bound claims in the daemon's private launch-grant table through the privileged
supervisor-to-daemon lifecycle channel. It passes the raw capability to the adapter out of band
through an inherited descriptor/pipe; if a supported launcher cannot do so, the only fallback is an
owner-verified `0600` capability file in the private runtime directory, opened without symlink
following and unlinked immediately after read. Capabilities never travel in argv, ordinary
environment variables, repository files, or logs. Handshake hashes and constant-time matches the
token, atomically consumes the registered grant, and mints the session; replaying the bootstrap
grant fails. This privileged grant-registration path is part of the supervisor launch contract, not
a public FastMCP or general query RPC.

The supervisor can revoke an adapter grant or principal by advancing a per-principal revocation
generation over the same private supervisor-control channel. Session validation checks that
generation on every call. Session renewal proves possession of the still-valid current session and
preserves the supervisor-granted principal; it cannot change workspace or permissions. Supervisor
restart, daemon restart, explicit revoke, expiry, or peer-UID mismatch invalidates the session.
Explicit principal revocation also cancels that principal's active queries and denies subsequent
resource reads; session rotation alone preserves the stable principal and does not orphan its
accepted queries or leases.

The session token is a bearer capability within the same-UID local trust boundary. This design does
not claim protection from a fully compromised process running as the same user. It does prevent a
caller from changing workspace or agent identity merely by editing request bodies, and it gives the
daemon one revocable, expiring application principal per adapter.

Session renewal is singleflight in the adapter. Re-handshake never silently restarts accepted work:
a lost daemon generation yields an explicit lost/incompatible outcome unless durable lifecycle
recovery proves the query/resource in a new generation.

The supervisor-control transport is an unnamed inherited Unix socketpair created before daemon
exec, not another filesystem socket and not part of `CpgQueryService`. Its only application-owned,
versioned, length-delimited records are `RegisterLaunchGrant`, `RevokePrincipal`,
`AdvanceSupervisorGeneration`, and `Acknowledgement`; their schema is sole authority for this
boundary and carries no semantic query/result fields. Possession of the explicitly inherited
descriptor plus the launch-time expected supervisor UID/PID/generation authenticates the channel.
Messages have a supervisor generation, monotonic sequence, operation ID, expiry, and content
digest. Duplicate operation ID with identical content is idempotent; changed content, gaps,
out-of-order generation, unknown record type, or replay after acknowledgement fails closed.

The grant/session tables are intentionally volatile and bound to the daemon generation. They are
not recovered from Delta or a repository file. Daemon restart empties them and requires the
supervisor to establish a new channel and register new grants before launching adapters. Loss of the
control channel forbids new grants, renewal, and revocation acknowledgement; the daemon reports the
security dependency degraded, lets already authorized work follow the separately accepted drain
policy only until its short session/query bounds, and accepts no new session authority. This is one
security/lifecycle mutation boundary, not a second semantic writer.

### 6.4 Start and idempotency

`StartQueryRequest` contains:

- canonical request bytes plus media type, QRY profile, public-schema digest, and request digest;
- requested freshness/delivery policy as released semantic/control values;
- a relative query execution budget, represented as `google.protobuf.Duration`;
- a random idempotency key;
- request-scoped MCP host capability bounds if they were unavailable at lifespan startup;
- correlation identifiers that are not principal claims.

It does **not** contain authoritative agent or workspace identity. The server derives those from the
session context.

The received digest is an integrity echo, never operation authority. Before idempotency lookup or
admission, the daemon strictly decodes the request, rejects duplicate/non-domain JSON, recomputes the
released canonical bytes and digest, requires the transmitted bytes to equal that canonical form,
and compares the transmitted digest in constant time. All subsequent identity and idempotency use
only the daemon-computed digest. A bytes/digest mismatch is `INVALID_ARGUMENT` and cannot select an
existing operation.

The idempotency record binds at least:

```text
principal/session compatibility class
+ workspace
+ canonical request digest and semantic profile
+ requested freshness and epoch-pinning policy
+ delivery policy and relevant host bounds
+ exact normalized requested execution duration
```

A replay with the same bound identity returns the original acceptance timestamp, queue decision,
query handle, and policy. Reuse with different input returns typed `IDEMPOTENCY_CONFLICT`. Entries
have a documented TTL and tombstone behavior; they do not grow for the daemon lifetime.

`StartQuery` performs only bounded authentication, validation necessary for safe registration,
idempotency lookup, quota admission/queue registration, and durable acceptance. It does not wait for
freshness, planning, DataFusion execution, result encoding, or publication.

### 6.5 Control event model

`WatchQuery` emits only lifecycle facts:

```text
SnapshotPinned
Progress
ResultReady
Terminal
```

- `SnapshotPinned` identifies the exact authorized FabricEpoch and proof/capability snapshot.
- `Progress` carries a closed phase enum, completed/total units when meaningful, and a safe message
  code; it is observational and may be coalesced.
- `ResultReady` carries one immutable result-package descriptor and lease reference. It carries no
  result bytes and no duplicate canonical descriptor JSON.
- `Terminal` carries the query lifecycle outcome (`SUCCEEDED`, `CANCELLED`, `FAILED`, `LOST`) and a
  result or typed outer-error reference. Semantic block-level status remains in canonical result
  data.

Each event has one typed header:

```text
daemon_instance_id
query_id
sequence
observed_at
event_kind
event_content_digest
trace correlation
```

`event_content_digest` is not deterministic Protobuf serialization. It is BLAKE3 over a released,
application-owned, length-framed `query-event-content-v1` projection containing daemon generation,
query ID, sequence, event kind, and the normalized logical event fields, excluding the digest
itself. Construction has cross-language known-answer tests. The digest establishes event identity
and corruption detection, not semantic correctness.

The query coordinator owns a compact state machine plus a replay journal bounded by event count,
encoded bytes, and TTL. Progress is coalesced **before** sequence allocation. Snapshot, ResultReady,
and Terminal are never discarded; capacity is reserved for terminal state. A slow watcher cannot
block DataFusion execution or atomic result publication. True backpressure applies to result writers
and resource reads, not to optional progress observation.

The resume cursor is opaque and binds query identity, next sequence, preceding event-content
digest, authorized principal, daemon generation, and expiry. A cursor before the retained window
returns typed `RESUME_WINDOW_EXPIRED`; a foreign and nonexistent query produce the same public
non-disclosing result.

Delivery is ordered **at least once**. An ambiguous disconnect may cause the last event to be
replayed. The adapter deduplicates by `(daemon_generation, query_id, sequence,
event_content_digest)` and rejects the same sequence with different content. Within the retained
window there are no required-event gaps, and the query state machine owns exactly one terminal
state; the protocol does not claim exactly-once terminal delivery.

Unifying initial observation and reattachment in `WatchQuery` eliminates duplicate Stream/Attach
semantics while retaining the accepted-handle recovery model. A bidirectional multiplexed session
stream is explicitly rejected in §11.

### 6.6 Protobuf shape rules

- Every closed transport vocabulary is an enum with zero `*_UNSPECIFIED`.
- Mutually exclusive variants use `oneof`; presence-sensitive scalars use explicit presence or a
  wrapper only where default and absent differ.
- Removed numbers and names are reserved permanently (`protobuf-ref §§7, 11–12, 26, 37–38`).
- Semantic QRY requests/responses remain canonical JSON bytes, never ProtoJSON or `Any`.
- Only bulk payload fields such as resource chunk data use selective Prost `bytes::Bytes` mapping
  through `compile_fds_with_config`. This reduces Rust cloning but is not called end-to-end
  zero-copy: Python receives `bytes`, FastMCP materializes bytes, and MCP JSON-RPC may base64 them
  (`tonic-ref §§7, 11`).
- A `ResourceChunk` carries offset, payload, and chunk digest. Total length and whole-resource
  digest belong to the sealed descriptor. End-of-range is determined from requested range and bytes
  received; redundant final/next-offset echoes are retained only if generated from the same range
  record and shown to improve diagnostics.
- Protobuf deterministic serialization is not used as a semantic canonicalization claim
  (`protobuf-ref §39`).

---

## 7. Immutable result and reference resource plane

### 7.1 One registry

One daemon registry owns every byte exposed across the boundary:

- canonical public query-response JSON;
- one result manifest;
- zero or more independently decodable Arrow IPC page resources and, where explicitly needed,
  opaque whole-object/range resources;
- reference-index and reference-projection artifacts when too large for control responses;
- optional diagnostic artifacts that pass disclosure policy.

The registry is not a `BTreeMap` of whole byte vectors. It stores sealed metadata plus a storage
handle to a bounded-memory/spill/object-store writer selected by the local data-fabric policy.
Ownership, authorization, quota, lease, TTL, checksum, and cleanup apply uniformly.

The content path is:

```text
DataFusion SendableRecordBatchStream
  -> relation-aware Arrow IPC encoder
  -> bounded writer / spill boundary
  -> validate schema, coverage, byte count, digest, trailer
  -> seal immutable resource records
  -> atomically publish one result package
  -> append ResultReady event
```

The query never collects `Vec<RecordBatch>` merely to serialize it. Temporary objects are invisible
until the entire package is validated and sealed. Failure or cancellation removes uncommitted
objects through the cancellation/cleanup owner.

### 7.2 Result package

One `ResultPackageDescriptor` identifies:

- query and pinned FabricEpoch/proof identities;
- canonical public-response resource;
- manifest resource;
- relation page resources with relation/schema/dictionary scope, sequence, coverage, row/byte
  counts, media type, digest, and availability;
- optional opaque whole-object resource and byte-range capability, distinctly typed;
- package expiry and lease policy;
- explicit partial/unknown/capability states from the canonical outcome.

The descriptor is the sole typed transport projection of one sealed Rust result outcome. A compact
MCP summary and resource links are derived from it. If a field is echoed in the canonical JSON for
agent interpretation, construction and acceptance tests prove exact equivalence.

### 7.3 Inline delivery is a policy over the same resource

There is no separate inline execution route.

After `ResultReady`, the adapter chooses presentation according to the daemon decision and the
effective host limit:

```text
effective_inline_limit = min(
    daemon enforced hard limit,
    negotiated MCP host limit,
    adapter presentation policy limit,
)
```

If the canonical public-response resource fits, the adapter performs one bounded
`ReadResource` request, verifies the range and whole-resource digest, parses it once, returns the
strict MCP envelope, and releases the lease when no links remain. This costs one additional local
UDS RPC compared with embedding bytes in an event, but it removes a second payload route, payload
replay memory, and control/data head-of-line coupling. The performance suite must measure this
tradeoff; the architecture reopens only if representative p99 evidence shows the extra local RPC is
material.

If it does not fit, the tool returns a concise structured summary plus `ResourceLink` blocks. Reading
a linked resource is repeatable until explicit release or expiry. A read never silently releases the
lease.

### 7.4 FastMCP resource reality

FastMCP resource functions return materialized `str`, `bytes`, or `ResourceResult`; they do not
provide an unbounded streaming response to the MCP host (`fastmcp-ref §§7.5–7.6, 33.17–33.18`).
Therefore:

- reference and result JSON pages are independently valid JSON documents (or a released bounded
  NDJSON page profile), never arbitrary fragments, and have strict decoded-byte/item bounds;
- canonical result JSON is available only when within a strict resource bound;
- large Arrow relations are exposed as independently valid Arrow IPC pages, each carrying its schema
  and every dictionary/message prefix needed to decode that page;
- an arbitrary range from a monolithic Arrow IPC object is labeled
  `application/octet-stream` and is offered only to a client explicitly reassembling/verifying the
  complete object; it is never advertised as a standalone Arrow document;
- no handler assembles a complete large relation in Python;
- resource kind, MIME type, page/byte range, total length, digest, expiry, and continuation are
  explicit;
- MCP base64 expansion and host limits are included in the page-size benchmark.

Recommended stable templates are conceptually:

```text
codefabric-reference://{kind}/{key}{?profile,page_token,if_digest}
codefabric-result://{package_id}/{resource_id}{?page_token,offset,length}
```

Exact URI escaping, opaque IDs, and query syntax require a FastMCP registration probe before they
are frozen. Arbitrary filesystem paths, object-store URLs, and lease tokens never appear in a URI.

### 7.5 Why not Arrow Flight now

The DataFusion/Arrow reference distinguishes internal `RecordBatch` streams, ephemeral Arrow IPC,
and Arrow Flight/Flight SQL for Arrow-native process/network consumers (`datafusion-arrow-ref` P22,
INT-01, INT-05, INT-08).

The FastMCP adapter is not an Arrow-native analytical consumer. It would still receive Python bytes,
materialize a FastMCP resource, and base64 the body over MCP. Adding Flight would introduce another
server/client/auth/schema/discovery surface without removing that bottleneck. The target therefore
uses Arrow IPC behind the generic immutable resource service.

Reopen Flight only when a supported direct Arrow-native client exists and measurements show that it
needs bulk columnar transport outside MCP. At that point prefer the standard pinned Arrow Flight
protocol over an application-specific live `RecordBatch` gRPC protocol. This review neither adds
PyArrow to the adapter nor treats Flight as a performance badge.

---

## 8. Introspection without semantic duplication

### 8.1 Two projection classes

The daemon publishes two explicitly different reference classes.

**Released projections** are available before any epoch:

- request and response JSON schemas;
- public status schema;
- control-contract/service-closure summary;
- stable query-form definitions and vocabulary definitions;
- authored guide and recipe templates;
- public typed outer-error catalog.

**Live projections** require current daemon/model state and authorization:

- current executable capability by query form/fact family;
- admitted form/vocabulary membership, availability, aliases, and provenance without redefining
  released IDs or meanings;
- active/reconstructible epoch summary;
- proof and freshness status;
- enforced limits and queue state;
- available result/reference resources;
- guide/recipe instantiations that cite current keys and gaps.

A live projection always carries daemon instance, interface snapshot, model/FabricEpoch/proof identity
as applicable, authorization scope, produced-at/expiry, media type, schema identity, digest, and one
of `CURRENT`, `DEGRADED`, `UNKNOWN`, or `UNAVAILABLE`. Empty lists are never used to imply a
provider or capability is absent.

### 8.2 `GetStatus` stays cheap

`GetStatus` reports a bounded operational summary only:

- daemon generation and readiness;
- selected/current epoch identity or explicit unavailability;
- accepted/queued/running counts and enforced limits;
- coarse capability/proof state with links/digests to detail;
- session expiry/renewal hint;
- health of mandatory serving dependencies without exposing secrets.

It does not inline schemas, registry rows, guides, or large capability catalogs. A cheap status path
must remain serviceable under query saturation and during partial bootstrap (`tonic-ref §36.2`).

### 8.3 `GetReference`

The selector is a small stable envelope:

```text
kind: request_schema | response_schema | public_status_schema
    | reference_index | query_form | vocabulary | capability
    | guide | recipe | proof_status | transport_contract
key: optional public/opaque key
profile: optional negotiated profile
page_size/page_token: optional bounded pagination
if_digest: optional immutable cache validator
```

The response either contains a small inline projection or an immutable resource descriptor. It
does not accept arbitrary file names, descriptor type URLs, SQL, or dynamic Protobuf messages.
Authorization and disclosure policy are checked before indicating whether a key exists.

The reference index is the compact discovery root. It maps safe keys to description, class,
schema/media type, digest, availability, and link; it does not copy the entire projection.

### 8.4 Stable FastMCP catalog

Keep exactly four tools required by `SRV §7`:

```python
query_code_graph(
    request: dict[str, JsonValue],
    delivery: Literal["automatic", "inline", "resource"] = "automatic",
    ctx: Context = CurrentContext(),
) -> ToolResult

validate_code_graph_query(
    request: dict[str, JsonValue],
    ctx: Context = CurrentContext(),
) -> ValidationToolOutput

get_code_graph_status(
    ctx: Context = CurrentContext(),
) -> StatusToolOutput

get_code_graph_reference(
    selector: ReferenceSelector,
    ctx: Context = CurrentContext(),
) -> ToolResult
```

Because FastMCP 3.4.7 does not infer an output schema when a handler returns `ToolResult`, the two
`ToolResult` tools declare it explicitly on their decorators, for example:

```python
QUERY_OUTPUT_SCHEMA = QueryToolOutput.model_json_schema(mode="serialization")

@mcp.tool(output_schema=QUERY_OUTPUT_SCHEMA)
async def query_code_graph(...) -> ToolResult:
    output = QueryToolOutput(...)
    return ToolResult(structured_content=output.model_dump(mode="json"), ...)
```

`get_code_graph_reference` does the same with its closed reference-output model. The model is
validated before wrapping so advertised `outputSchema` and actual `structuredContent` cannot drift.
The status and validation tools return typed models directly or also provide an explicit
serialization schema. Construct the server with `strict_input_validation=True` and `tasks=False`;
the catalog oracle asserts every tool's advertised input/output schema rather than inferring it from
annotations alone.

The query argument intentionally remains “JSON object” at the MCP envelope. Dynamically replacing
the tool schema with the daemon's complete live QRY schema would create a second validation surface,
make tool discovery epoch-dependent, and increase catalog/token size. Instead, tool descriptions
link to the request-schema and guide resources. Rust remains the semantic validator (`SRV §3`).

Register fixed resource templates rather than one dynamic resource per model row. Register the two
`SRV §7.4` prompts:

- **author a code-graph query** — fetches the current request schema, guide, form/capability
  projection, and examples at invocation time;
- **interpret code-graph facts** — fetches current response/status vocabulary and provenance guide
  at invocation time and explicitly forbids evaluative conclusions.

Prompt text in the wheel is only a stable presentation template. Current keys, capabilities,
vocabularies, and examples come from the daemon, so the prompts do not become semantic authority.

While `query_code_graph` consumes `WatchQuery`, it deduplicates at-least-once events and projects
each newly observed progress sequence through
`await ctx.report_progress(completed, total, safe_message)`. It rate-limits only presentation;
daemon progress coalescing remains authoritative. If the FastMCP call is cancelled, Python cancels
the watch/read, sends one short shielded best-effort `CancelQuery`, and re-raises cancellation. A
FastMCP client integration test must prove progress order, duplicate suppression, cancellation, and
terminal/result presentation.

Do not add a custom FastMCP Provider, ToolSearch/Code Mode, or FastMCP background task system:

- four fixed tools do not justify another dynamic catalog abstraction;
- the daemon already owns accepted-query lifecycle, so FastMCP tasks would duplicate state and
  cancellation authority;
- dynamic providers/transforms are not a license to install daemon-provided executable schema or
  code.

### 8.5 Pydantic boundary

Pydantic owns only the stable public presentation envelope:

- strict, frozen models with `extra="forbid"`, `hide_input_in_errors=True`, and
  `allow_inf_nan=False` where numeric fields exist;
- one module-scoped `TypeAdapter` for recursive `JsonValue`/query-object shape;
- discriminated inline/resource result variants;
- annotation-driven serialization that cannot leak subclass-only fields;
- serialization-mode JSON Schema for MCP outputs;
- small typed DTOs for status, reference selector, progress summary, errors, links, and result
  envelope.

Generated `_pb2` objects terminate inside the daemon-client package. That package converts them
once into frozen adapter DTOs. There is no `dict -> canonical bytes -> dict -> Pydantic -> dict`
round-trip unless the public boundary truly requires each transition.

Pydantic must not reproduce:

- the semantic QRY grammar;
- ontology/query-form/current-capability registries;
- Protobuf control messages;
- Arrow schemas or manifests;
- daemon admission or lifecycle state machines.

No new `orjson` dependency is warranted. gRPC uses generated Protobuf; semantic canonical JSON uses
the repository's strict JCS contract; Pydantic/FastMCP already owns presentation serialization.
`orjson` also holds the GIL during its work and is not a replacement for Protobuf or ProtoJSON
(`orjson-ref §§22–27`). Profile intentional JSON work before changing it.

---

## 9. Lifecycle, security, deadlines, and errors

### 9.1 Adapter lifespan

One `grpc.aio.Channel`, one generated stub, and one session manager are created in FastMCP lifespan
and closed unconditionally. Before lifespan yields and the catalog is usable, the adapter must:

1. validate local configuration without logging credentials;
2. open the UDS channel and wait within a bounded startup budget;
3. perform `Handshake` and verify negotiated descriptor/profile/schema identities;
4. fetch or validate the compact released reference index needed by catalog descriptions;
5. fail startup for transport, authentication, or compatibility failure.

A compatible `BOOTSTRAPPING` or `DEGRADED` workspace is not an adapter-startup failure. Lifespan
yields with GetStatus and released reference projections available; query/epoch-live operations
return the explicit unavailable/degraded state until readiness changes. This preserves bootstrap
introspection without pretending the query service is ready.

Socket existence and channel connectivity are not readiness. On transport reconnect, a singleflight
re-handshake refreshes the session. Accepted queries are resumed through `WatchQuery`; `StartQuery`
is never wrapped in a generic reconnect retry.

`grpc.aio` objects remain on their creating event loop. CPU-heavy canonicalization, validation, or
hashing is measured for loop lag at negotiated maxima; offload is introduced only where that
measurement requires it (`grpcio-ref §§13–18`).

### 9.2 Layered authentication and authorization

```text
private 0700 parent / 0600 UDS
  -> kernel peer credential and same-UID policy
  -> bootstrap binary capability on Handshake
  -> daemon-instance-bound expiring session on every call
  -> coarse method permission in transport/application middleware
  -> query/resource/source authorization in the application service
  -> authorized child catalog and row/column/source policy in FAB
```

A Tonic interceptor may parse metadata and attach a preliminary context, but revocation-aware or
async authentication belongs in service/Tower middleware. Object-specific authorization remains in
the application method because transport middleware cannot decide which epoch, source, result, or
lease may be disclosed (`tonic-ref §§16, 22–24`). Python uses a `grpc.aio` interceptor or equivalent
central call wrapper to attach current binary session and trace metadata for every cardinality.

Token comparison is constant-time where applicable. Tokens are redacted from logs, error details,
MCP `_meta`, resource URIs, and traces. PID is diagnostic, not principal. Foreign and nonexistent
handles are deliberately indistinguishable at the public boundary.

TLS is not added over the same-user private UDS. It would add certificate lifecycle and CPU cost
without strengthening the chosen local trust boundary. A future cross-host transport is a separate
design reopening, not a configuration toggle.

### 9.3 Deadlines and cancellation

Two time budgets are distinct:

- every individual gRPC call has a relative `timeout=`/`grpc-timeout`;
- an accepted query has a relative execution budget sent in `StartQuery` and converted once to a
  daemon monotonic deadline.

Do not send an absolute Unix deadline. It duplicates authority and introduces wall-clock skew. The
adapter subtracts a cleanup reserve before requesting an execution budget; cancellation/release use
their own short shielded cleanup deadlines.

The daemon owns a `CancellationToken` tree:

```text
daemon shutdown
  +-> accepted query execution
  |    -> freshness/epoch pin
  |    -> planning and providers
  |    -> DataFusion execution
  |    -> unsealed result writer
  +-> resource/lease lifecycle
       -> each live resource read
```

Dropping `WatchQuery` cancels observation, not the resumable query. An actual MCP/client cancellation
signals `CancelQuery`; the adapter then stops its watch/current read. Query cancellation removes an
unsealed writer but does not revoke a resource that was already atomically sealed and published.
Sealed-resource reads are governed by their sibling resource/lease token: Release/expiry rejects new
reads, while an already-authorized fixed-range read may finish under the released race policy. Every
stage checks its owning token at a bounded interval, cleans uncommitted state, and reaches one fixed
outcome. Orphaned queries and leases expire under explicit policy (`SRV §10`).

### 9.4 Admission, fairness, and backpressure

WP19's application scheduler is the single authority for:

- active and queued queries per principal/workspace/global;
- CPU, memory, spill, provider, and result-byte budgets;
- result-writer and result-read concurrency;
- artifact quota, TTL, lease, and cleanup;
- cancellation and shutdown drain;
- admission class and fair scheduling.

`StartQuery` registers accepted/queued work promptly. A Tower semaphore must not wait for the entire
query and thereby become a second scheduler. Transport middleware may set a generous connection/RPC
abuse ceiling and message-size limits, with reserved capacity for Handshake, GetStatus, CancelQuery,
and ReleaseResource. No unbounded Tower buffer is permitted (`tonic-ref §28`).

Backpressure paths are independent:

- DataFusion to result writer is bounded by encoder and storage pressure;
- `ReadResource` is bounded by requested range, chunk size, transport flow control, and read quota;
- progress is coalesced into a bounded journal and cannot block result execution;
- Python consumes server streams with `async for` and holds at most the negotiated page/inline
  bound.

Advertised limits are snapshots of the same scheduler/quota objects that enforce them. This makes
configuration load-bearing rather than descriptive (`PRIN` P27).

### 9.5 Error layering

Use gRPC status only for outer protocol/transport failures:

| Condition | Code |
|---|---|
| malformed outer RPC or invalid range | `INVALID_ARGUMENT` or `OUT_OF_RANGE` |
| missing/expired session | `UNAUTHENTICATED` |
| denied operation/object/workspace | `PERMISSION_DENIED` with non-disclosing public detail |
| quota/admission/read pressure | `RESOURCE_EXHAUSTED` |
| incompatible descriptor/profile/state or expired resume window | `FAILED_PRECONDITION` |
| unknown/expired query or resource under disclosure policy | `NOT_FOUND` or the single policy-selected nondisclosing equivalent |
| daemon/bootstrap unavailable | `UNAVAILABLE` |
| RPC deadline/cancellation | standard deadline/cancel codes |
| unexpected implementation fault | redacted `INTERNAL` plus trace identity |

Attach allowlisted typed `google.rpc.Status` details for stable machine-readable outer failures only
after exact Rust/Python locked-version interop is compile- and runtime-probed. Unknown detail types
must remain forward compatible. The Python client has one mapper and never branches on
`RpcError.details()` prose (`grpcio-ref §23`; `tonic-ref §17`).

If that rich-status probe fails, the fallback is the standard gRPC status plus one released,
allowlisted ASCII trailing-metadata value such as `codefabric-error-code`; absence or an unknown
value leaves only the standard code. A non-OK RPC cannot rely on an ordinary response message, so
no response field is proposed as the fallback. Metadata is size-bounded, non-secret, and decoded by
the same central mapper.

Semantic validation failures, unsupported facts, capability gaps, partial blocks, and explicit
unknowns remain successful canonical response data when the outer request was processed correctly.
FastMCP errors are safe, concise presentation projections and do not expose UDS paths, tokens,
source text, object locations, or stack traces (`SRV §11`).

### 9.6 UDS lifecycle

The supervisor/daemon must:

- use a private, owner-verified parent directory and reject symlinks/non-sockets;
- hold a generation/instance lock before binding;
- probe an existing endpoint for a live compatible service before treating it as stale;
- verify owner, type, device, and inode before unlinking;
- publish protocol readiness only after service construction and self-checks;
- compare device/inode again before shutdown unlink so a replacement socket is never removed;
- tolerate kernel-level connection closure in foreign-UID tests rather than requiring a stable gRPC
  status that may never reach application code.

Path existence is neither liveness nor readiness.

### 9.7 Correlation and semantic observability

Keep these identities distinct and structured:

```text
MCP session id
MCP call id
adapter instance id
RPC attempt id
daemon instance id
accepted query id
FabricEpoch / proof identity
result package id
resource id
lease id
```

Every log/metric span states its layer and redaction class. Required operational metrics include:

- handshake compatibility/latency/session renewal;
- accepted/queued/running/terminal query counts and wait/runtime distributions;
- watch reconnect, cursor expiry, progress coalescing, journal bytes/age;
- result writer bytes, spill, seal latency, read bytes/chunks/backpressure, active leases, expiry;
- cancellation latency and cleanup residue;
- event-loop lag and Python peak materialization by route;
- transport status by allowlisted code, never raw request/source payload.

Metric labels are restricted to a reviewed bounded allowlist: RPC method/cardinality, standard
status, released public error code, closed phase, delivery/resource kind, queue class, and a few
static build/platform dimensions. Agent, workspace, MCP/query/resource/lease/epoch IDs, digests,
schema IDs, arbitrary vocabulary members, and source paths are forbidden as metric labels; they may
appear only in sampled access-controlled traces or redacted logs under retention policy. A label
census and high-cardinality negative fixture enforce this boundary.

Metrics report facts, not `HIGH_RISK`, “safe,” impact judgments, or inferred recommendations
(`PRIN` P24 and cross-cutting doctrine).

---

## 10. Library decisions

### LD-IF01 — Keep generated static Tonic/grpcio, not dynamic dispatch

**Decision.** The released `.proto` and normalized service descriptor closure remain the control
authority. The pinned generator emits Python bindings and one FileDescriptorSet; Rust compiles that
exact IR. Embed the relevant closure in the daemon and compare it semantically at startup/tests.

**Use.** Generated messages/stubs on hot paths; selective `Bytes`; descriptor assertions;
cross-language interop.

**Do not use.** Reflection, `prost-reflect`, Python descriptor pools, `Any`, or runtime factories for
production dispatch. Reflection is diagnostic only; semantic introspection is a separate service.

**Why.** Static dispatch preserves exhaustiveness and speed while one FDS pipeline eliminates
handwritten cross-language transport duplication (`tonic-ref §§5–11, 31–35`; `protobuf-ref §§22–23`).

### LD-IF02 — Fingerprint the service closure, not unrelated descriptors

**Decision.** Normalize and fingerprint `CpgQueryService` plus transitive imported message/enum
descriptors and declared compatibility features. Do not make extractor/provider service changes
break a query adapter handshake merely because all services share a generation invocation.

**Proof.** Assert methods, cardinalities, full names, field numbers/types/presence, enum values,
reserved tombstones, and closure digest in Rust and Python. Descriptor semantic equivalence, not
generated source text, is the co-release/build oracle. Runtime compatibility separately requires
equal major, overlapping minor policy, supported required features, and an immutable N/N-1 interop
matrix; it does not require every compatible additive descriptor closure to have the same digest.

### LD-IF03 — One `grpc.aio` channel and explicit application recovery

**Decision.** Reuse one loop-bound channel/stub per adapter lifespan. Set a timeout on every call.
Use explicit `WatchQuery` resume and immutable offset reads; never generic-retry `StartQuery` or
streaming methods.

**Why.** Channels are expensive and reconnect is transport recovery, not logical query recovery
(`grpcio-ref §§10, 13–18`; `tonic-ref §37`).

### LD-IF04 — Resource-first result delivery

**Decision.** Remove response bytes from query events. One immutable registry supplies bounded
inline reads and large resource pages.

**Why.** It makes control replay bounded, eliminates dual result construction, and matches
FastMCP's materializing resource model. The extra local read is measurable and preferable to a
permanent second authority unless evidence disproves it.

### LD-IF05 — Arrow IPC now; Flight only for a future direct Arrow client

**Decision.** Use internal `RecordBatch` streams and Arrow IPC resource encoding. Do not add Flight
to the FastMCP path.

**Why.** Flight is appropriate for Arrow-native clients; MCP remains the limiting materialization
boundary. This is an explicit, reversible library decision rather than custom protocol lock-in.

### LD-IF06 — Strict minimal Pydantic

**Decision.** Pydantic models only stable MCP envelopes and selector DTOs. Reuse module-level
adapters, discriminated unions, serialization schemas, and secret-safe serialization.

**Why.** This supplies clear MCP schema and validation without cloning daemon semantics
(`pydantic-ref §§19, 21, 26, 49`).

### LD-IF07 — No `orjson` on the boundary

**Decision.** Do not add or use `orjson` for Protobuf, ProtoJSON, semantic canonicalization, or
FastMCP response bytes.

**Why.** Each layer already has its authoritative serializer; an extra library would create another
encoding path without removing the large-payload bottleneck.

### LD-IF08 — Typed rich errors only after an interop probe

**Decision.** Adopt `tonic-types` and matching Python status/common-proto support only if the exact
locked versions round-trip all allowlisted details and preserve unknown details.

**Fallback.** Use standard gRPC codes plus one fixed, versioned, allowlisted trailing-metadata error
code. Never expect an ordinary response on a non-OK RPC and never fall back to parsing prose.

### LD-IF09 — Health on; reflection gated; compression/tuning off by default

**Decision.** Add standard health. Gate reflection to development/diagnostics. Keep gRPC compression,
keepalive, window tuning, aggressive channel options, and TLS off until a named workload proves a
need.

**Why.** Same-host UDS changes the latency/failure model. Compression can spend more CPU than it
saves, and premature HTTP/2 tuning can hide backpressure defects (`tonic-ref §§20, 29–31, 38`).

### LD-IF10 — Keep supervisor grant control separate and minimal

**Decision.** Use one inherited unnamed Unix socketpair and one fixed application-owned record
schema for supervisor grant registration, revocation, generation advance, and acknowledgement. Do
not expose this authority through FastMCP, the public query UDS, environment variables, argv, or a
second durable state store.

**Why.** The root of application identity needs an operationally authenticated mutation path, but
it has different callers, privileges, lifetime, and replay semantics from query RPCs. The volatile,
generation-bound channel prevents control-plane privilege from leaking into the semantic interface
while giving grant/session enforcement one explicit owner.

---

## 11. Alternatives considered and rejected

| Alternative | Attractive property | Reason rejected for this boundary |
|---|---|---|
| One typed RPC/message family per query form | Maximum IDE/static discoverability | Copies evolving semantic grammar into Protobuf and Pydantic; every ontology/query evolution becomes a three-language release. Violates one semantic owner. |
| One bidirectional multiplexed session stream for start/progress/results/cancel | Few method names and apparent symmetry | Couples accepted work to one connection, complicates partial failure/idempotency/resume, and introduces head-of-line interaction between unrelated actions. |
| Keep response chunks in `QueryEvent` | Saves one local read for small output | Forces result bytes into replay and control flow, makes progress/result backpressure inseparable, and maintains two delivery authorities. Reopen only on measured regression. |
| Separate `StreamQuery` and `AttachQuery` forever | Distinct names for first watch and resume | The request/cursor already expresses the distinction; duplicate methods add generated/public surface without different authority. v1 retains old meanings during migration. |
| Return all schemas/capabilities in `GetStatus` | One discovery call | Makes readiness expensive and unstable, mixes Class 1 and Class 2, and risks control-message limits. |
| Dynamically generate the FastMCP tool schema from live daemon JSON Schema | Rich schema in `tools/list` | Makes the tool catalog epoch-dependent, duplicates validation, increases tokens, and can hide unavailable language features. |
| Dynamic Protobuf/reflection/`Any` production service | Small handwritten service | Converts descriptors/type URLs into a runtime capability surface and loses compile-time exhaustiveness. Reflection also cannot explain opaque QRY semantics. |
| Python/PyArrow result processing | Convenient Arrow consumption | Violates presentation-only authority, adds a native package surface, and still materializes MCP output. |
| Arrow Flight in the FastMCP path | Standard high-throughput Arrow transport | Does not bypass MCP/Python materialization for current consumers and adds a second protocol surface. Preserve as a future direct-client trigger. |
| Shared-memory paths or local file URLs | Potentially fewer copies | Exposes filesystem/object lifetime, TOCTOU, permissions, cleanup, and sandbox problems as a public contract. |
| FastMCP background tasks for queries | Ready-made task UX | Duplicates daemon accepted-query, cancellation, retention, and recovery authority. |
| Whole-service Tower concurrency limit | Simple overload control | Can block `StartQuery` before acceptance and starve cancel/status/release; becomes a second scheduler. |
| gRPC compression by default | Smaller wire bytes | Same-host UDS and already compact control traffic make CPU cost likely dominant; Arrow may already use format-level compression. Measure first. |
| TLS on the private UDS | Familiar transport security | Adds certificate lifecycle without improving the selected same-user local boundary. Cross-host is a new design. |

---

## 12. Design-principle conformance

| Principle | Disposition | Target evidence |
|---|---|---|
| P1 model semantics | Maintains | Semantic grammar and live capability remain model/daemon-owned, not transport-owned. |
| P2 executable model | Advances | Reference/capability projections are emitted from admitted executable state and carry proof identity. |
| P3 one owner | Advances | One scheduler, one resource registry, one schema owner per plane, one session principal. |
| P4 hierarchy | Maintains | Suite/profile/schema/epoch/query/resource identities remain distinct and composable. |
| P5 variability behind contracts | Advances | Stable control and MCP envelopes hide evolving semantic/physical implementation. |
| P6 semantics separate from execution | Advances | Canonical semantic JSON remains separate from control lifecycle and Arrow execution bytes. |
| P7 shared fabric | Maintains | The boundary consumes the daemon's shared authorized data fabric rather than adding Python state. |
| P8 common representation | Advances | All results publish through one canonical package/resource representation. |
| P9 provenance | Advances | Events, projections, packages, and resources bind epoch/proof/schema/digest identities. |
| P10 closure | Advances | Service-closure descriptors and reference indexes make dependency closure explicit. |
| P11 immutable snapshots | Maintains | Accepted query, replay, and reads remain pinned to one FabricEpoch. |
| P12 executable schemas | Advances | Released schemas remain executable in Rust and addressable as verified projections. |
| P13 governance at authority | Advances | Auth, disclosure, limits, schema mapping, and publication are enforced at daemon owners. |
| P14 highest extension | Maintains | Uses standard Tonic/grpcio/health/Arrow IPC before custom mechanisms. |
| P15 optimizer | Maintains | Streaming resource publication does not move execution out of DataFusion. |
| P16 lifecycle | Advances | Sessions, queries, journals, idempotency, resources, leases, and tasks gain explicit owners/TTL. |
| P17 reconstruction | Advances | Cursor/package/epoch identity and durable acceptance distinguish reconnect from restart/recovery. |
| P18 fingerprint is identity, not correctness | Advances | Digests identify descriptor/events/resources; semantic/proof oracles establish correctness separately. |
| P19 reproducibility | Advances | One FDS input and immutable resource/projection digests make cross-language output reproducible. |
| P20 prove capabilities | Advances | Live capability projections carry proof/current/unknown state instead of package claims. |
| P21 enforced vs advisory | Advances | Effective limits come from the enforcer; adapter preferences remain named policy bounds. |
| P22 protocols | Advances | Static typed control, canonical semantic JSON, and Arrow resource planes have explicit contracts. |
| P23 state ownership | Advances | Accepted work, sessions, journals, resources, and leases stay in the daemon. |
| P24 semantic observability | Advances | Metrics expose lifecycle/resource facts without evaluative judgments. |
| P25 oracle | Advances | Cross-language descriptors, resume, authorization, resource, and performance behaviors gain independent oracles. |
| P26 immutable declarations | Advances | Only released contract projections are static; live capabilities are not declared in the wheel. |
| P27 load-bearing configuration | Advances | The same scheduler/quota values enforce and report limits. |
| P28 compute change | Maintains | Stable catalog plus digest/conditional reference reads makes changes explicit and cacheable. |
| P29 strongest relational validation | Advances | Structural descriptor, package, and schema comparisons replace prose/string matching. |
| P30 independent expectations | Advances | Failure fixtures and cross-language expected descriptors are frozen independently of runtime output. |
| P31 eliminate forget-to-sync | Advances | Generated projections replace hand-maintained Python registries and duplicated control strings. |
| P32 construction | Advances | One Rust outcome constructs descriptor, resource package, terminal projection, and MCP-safe summary inputs. |
| P33 functional core | Advances | Pure normalization/digest/range/projection construction is separated from I/O and lifecycle shells. |
| P34 one mutation | Advances | Query/resource lifecycle mutations pass through one coordinator/registry owner. |
| P35 inward dependencies | Advances | FastMCP/Pydantic depend on stable DTOs and daemon references, not provider/DataFusion internals. |
| P36 executable governance | Advances | Named gates below turn wire, package, authority, backpressure, and legacy claims into checks. |

The most important risk mitigations are P3, P16, P20–P23, P26–P32, P34, and P36. The design does not
claim that a schema digest or passing descriptor check proves semantic correctness (P18/P25).

---

## 13. Transition and plan integration

### 13.1 What the active plan already owns

The following outcomes already belong to the current target and should not be represented as new
parallel work:

- WP04: Arrow schema/IPC framing and cross-process relational boundary;
- WP15: daemon-derived queries, schemas, vocabularies, capabilities, proof/status, and result
  resources through a presentation-only adapter;
- WP19: one runtime's admission, quota, backpressure, cancellation, leases, retention, and clean
  reconstruction;
- WP20: independent public/security/performance release evidence;
- DB03: removal of static serving/query bundles, adapter fingerprints/registries/schema aggregates,
  and predecessor result paths after positive replacement proof;
- DB04: removal of generated semantic authorities/model-compiler products while retaining released
  public contracts and only narrowly proved derived build cache.

### 13.2 New design deltas

These decisions materially change the accepted serving design and cannot be recorded as mere helper
or batch-size adaptations:

1. a new `codefabric.cpgd.v2` service package and migration contract;
2. `WatchQuery` replacing the steady-state Stream/Attach pair;
3. removal of response bytes from query events;
4. one resource-first delivery path for both inline and linked output;
5. handshake-minted per-call session metadata and removal of body principal claims;
6. released-versus-live introspection classes with a dedicated `GetReference` surface;
7. bound replay cursor and bounded/coalesced control journal semantics;
8. typed rich outer-error details, if the compatibility probe succeeds;
9. service-closure descriptor identity rather than an unrelated aggregate descriptor identity;
10. the precise UDS generation/readiness lifecycle;
11. the supervisor-issued principal model and its private grant registration/revocation protocol.

`PLAN §2` explicitly says any accepted design or v2-principles change makes the plan stale. `PLAN
§9.2` classifies target invariant/authority changes as design reopening and materially different
proof/boundary changes as plan revision. Therefore this proposal must be accepted through a new
versioned SRV artifact. **At the moment it is accepted as a design change, all execution governed by
the current plan must pause**; the whole plan is stale, not merely the serving dependency cone. A new
immutable plan version must incorporate the redesign and receive independent audit before any plan
execution resumes. The decisions must not be silently absorbed into WP04, WP15, or WP19 state.

This is favorable timing: those three packets were not started at the review baseline. The change
can be governed before their implementations and proofs harden the predecessor interface.
The revised plan must assign the supervisor-control boundary to a dependency-closed predecessor of
session-enabled Handshake and WP15 serving, integrate its shutdown/restart/revocation lifecycle with
WP16/WP19 rather than creating an untracked writer, and make WP20 re-execute its independent
security evidence.

### 13.3 Compatibility cutover

Use a bounded replacement sequence:

1. Freeze v1 descriptor, behavior fixtures, consumers, and proving evidence.
2. Accept the successor serving design and revised plan.
3. Add v2 generated contracts and one internal application interface.
4. Adapt any temporary v1 facade and the new v2 facade to that same internal interface; prohibit
   dual execution, dual result stores, or silent fallback.
5. Build a v2 adapter with eager handshake and stable four-tool catalog.
6. Prove descriptor closure, Rust/Python interop, v1 compatibility fixtures, and v2 negative cases.
7. Shadow only safe read-only semantic comparisons against the same accepted inputs; no dual
   mutation or independent expected-result generation.
8. Atomically select the compatible daemon/adapter pair and fail closed on mismatch.
9. Prove no live v1 client, server registration, package import, routing, or fallback remains.
10. Remove the v1 runtime facade under DB03/DB05 as applicable; retain v1 `.proto`, descriptor,
    fixtures, and release history as immutable compatibility evidence.

If repository/external census proves v1 has never been released to a consumer, governance may
choose clean pre-release replacement. That fact must be proved; it is not assumed here.

### 13.4 Legacy disposition

| Legacy/product | End state |
|---|---|
| v1 `.proto`, normalized descriptor, compatibility fixtures | retain as immutable released/history evidence |
| v1 live service registration and adapter calls | remove after compatibility cutover proof |
| `ResponseChunkEvent` production meaning | absent in v2; reserve old fields/names in their owning package |
| separate `StreamQuery`/`AttachQuery` v2 methods | absent; one `WatchQuery` |
| legacy `artifact_records` and lease cache | remove after one registry is production-wired and proved |
| adapter `model_registries.py`, model artifact index, live schema fingerprints/censuses | remove or split; retain only truly released static schema artifacts with provenance |
| generated CPG query `_pb2` closure | retain as generated transport projection where foreign-build proof requires it |
| unrelated provider/extractor bindings in adapter wheel | remove unless a named package/descriptor proof demonstrates a consumer |
| Python Arrow manifest/schema reconstruction | remove after daemon descriptor/reference resources are authoritative |
| advertised but unenforced compression/modes/limits | remove or implement from one enforcer; no decorative capability |

---

## 14. Executable proof obligations

### 14.1 Contract and generation

| Named proof | Required discrimination |
|---|---|
| `just grpc-v2-contract-check` | Rebuild exact FDS; compare normalized CPG service closure in Rust/Python; assert methods, cardinalities, names, field numbers/types/presence, enums, reservations, and digest. Mutating a critical field must fail. |
| `just grpc-v2-compatibility-window-check` | Run immutable exact/N/N-1 client-server fixtures; equal major plus permitted additive minor/features succeeds, missing required feature or breaking meaning fails, and descriptor digest alone neither accepts nor rejects compatibility. |
| `just grpc-v2-interop-check` | Exercise every unary/server-streaming method across real UDS with generated clients/servers, maximum valid messages, malformed frames, and unknown fields. |
| `just grpc-v1-compatibility-check` | Prove frozen v1 fixtures retain released meanings during migration and no v2 field reuse mutates them. |
| `just adapter-protobuf-closure-check` | Inspect wheel/sdist and prove only the needed generated service closure is packaged, or document the exact foreign-build reason for more. |

### 14.2 Readiness, identity, and security

| Named proof | Required discrimination |
|---|---|
| `just adapter-eager-handshake-check` | Transport/auth/incompatibility failure prevents a usable catalog; compatible `BOOTSTRAPPING` yields status/released references while query execution is explicitly unavailable; ready handshake publishes the same stable catalog. |
| `just supervisor-launch-grant-check` | Real supervisor launches daemon with an unnamed inherited channel, registers and receives acknowledgement before adapter launch, passes the raw grant only by the approved descriptor/file path, and proves single consumption, changed-content replay rejection, monotonic revoke/generation handling, channel-loss fail-closed behavior, restart zero-state, and no public-query/semantic fields. |
| `just query-session-auth-check` | Supervisor grant is scoped, single-use, out-of-band, and consumed atomically; missing, replayed, expired, wrong-generation, wrong-peer, revoked, and cross-workspace sessions fail; body identity cannot elevate authority; renewal is singleflight and preserves the granted principal. |
| `just resource-disclosure-check` | Same owner reads; another principal with known IDs gets the nondisclosing outcome; error/log/meta/URI contain no token/path/source secret. |
| `just uds-generation-check` | Live socket is never unlinked; stale owned socket is safely replaced; symlink/wrong owner/wrong inode fails; shutdown cannot unlink a replacement. |
| `just grpc-error-detail-interop-check` | Every allowlisted rich detail round-trips Rust to Python without prose matching; unknown details remain safe and forward compatible. |

### 14.3 Query lifecycle and recovery

| Named proof | Required discrimination |
|---|---|
| `just accepted-query-idempotency-check` | Daemon strict-decodes/recanonicalizes/recomputes before lookup; mismatched bytes/digest fails; same bound input returns the original acceptance; changed request/freshness/principal/exact normalized duration conflicts; expiry/tombstone outcomes are fixed. |
| `just query-watch-resume-check` | Disconnect before/after every event variant; ordered at-least-once replay has no retained-window required-event gap; duplicate last delivery deduplicates by generation/query/sequence/content digest; same sequence/different content and forged/expired/foreign cursors fail; exactly one terminal state exists. |
| `just query-journal-bound-check` | High-frequency progress with no consumer keeps journal count/bytes/RSS bounded, coalesces before sequence, and retains snapshot/result/terminal. |
| `just query-cancellation-tree-check` | Cancel during freshness, provider, planning, DataFusion stream, writer, and seal; unsealed state cleans within bound. A sealed resource survives query cancel; release/expiry governs new reads and the fixed active-read race. |
| `just query-deadline-separation-check` | Acceptance RPC may end while query continues; monotonic execution budget still governs; cleanup reserve remains available. |
| `just query-admission-serviceability-check` | Saturate active/queued/read quotas; reported limits match enforcement; status/cancel/release remain bounded and serviceable. |

### 14.4 Resource and presentation

| Named proof | Required discrimination |
|---|---|
| `just result-resource-streaming-check` | 64–256 MiB synthetic relation streams through daemon with peak memory bounded independently of total bytes; no `Vec<RecordBatch>`/whole-byte collection on production path. |
| `just result-range-equivalence-check` | Every Arrow page independently decodes with schema/dictionaries; arbitrary raw ranges are octet-stream only; full, partial, repeated, overlapping, final-short, invalid, cancellation, release, and expiry reads reconstruct exact digest or return typed failure. JSON pages parse independently and raw JSON fragments are never mislabeled. |
| `just fastmcp-resource-bound-check` | Every resource read respects decoded and encoded host limits; Python RSS is bounded by one page; no whole large relation join/base64. |
| `just inline-resource-equivalence-check` | Inline JSON is byte/semantic equivalent to the canonical resource and is derived from the same sealed package; adapter threshold cannot exceed daemon/host bound. |
| `just dynamic-interface-reference-check` | Change admitted live reference rows and prove outputs/digests change without rebuilding the adapter; released tool schema remains unchanged. |
| `just adapter-package-authority-zero-state-check` | Wheel/sdist/import graph has no live ontology/query/capability/schema census or semantic executor. |
| `just fastmcp-catalog-check` | Exactly four tools, fixed templates, and two prompts; `strict_input_validation=True`; every tool has the expected non-null serialization `outputSchema`, including explicit schemas for `ToolResult`; structured content validates; no dynamic provider/task lifecycle; prompts fetch daemon references at invocation. |
| `just fastmcp-progress-cancellation-check` | A real FastMCP client observes ordered sequence-deduplicated `ctx.report_progress` updates; cancellation stops watch/read, issues one bounded shielded CancelQuery, and exposes no duplicate terminal/result. |
| `just observability-cardinality-check` | Metric label names and values are drawn only from the bounded allowlist; injecting query/resource/workspace/agent/epoch/digest/schema/source values fails; sampled traces retain redacted correlation. |

### 14.5 Performance suite

Measure on a warm channel and record hardware, build profile, payload, concurrency, and confidence
intervals. At minimum:

| Workload | Measures |
|---|---|
| handshake and status | p50/p95/p99 latency, allocations, descriptor-cache behavior |
| validate and start-to-accepted | latency under idle and saturated scheduler; control-lane serviceability |
| accepted-to-first-event and resume | journal latency, reconnect cost, event-loop lag |
| bounded inline JSON near threshold | end-to-end latency, Rust/Python CPU, copies/allocations, Python RSS |
| 64 MiB and 256 MiB Arrow resources | throughput, time-to-first-chunk, daemon/Python RSS, spill, CPU, range retry |
| slow/abandoned reader | bounded queues/RSS, query writer progress, cleanup latency |
| concurrent queries plus reads | fairness, queue wait, cancellation/status latency, FD/task/resource counts |
| compression off/on candidate | wall time, CPU, bytes, and p99; retain compression only with a representative win |

Acceptance should define budgets from measured supported workstations rather than copy thresholds
from this review. The current `SRV §9` size limits remain governing until a successor explicitly
changes them.

### 14.6 Required negative fixtures

At least these failures must be first-class fixtures:

- duplicate/reused Protobuf number, changed cardinality, missing reservation;
- descriptor matches but selected semantic profile/schema does not;
- query form released but currently unavailable;
- missing provider output represented as unknown/capability gap, not empty rows;
- canonical request bytes/digest mismatch and noncanonical/duplicate-key JSON before idempotency;
- idempotency collision with different normalized request;
- cursor with changed event bytes or wrong principal/generation;
- ambiguous watch disconnect with one duplicate event and deterministic adapter deduplication;
- terminal capacity under progress flood;
- Arrow page without required schema/dictionary, raw range mislabeled as Arrow, JSON fragment
  mislabeled as JSON, range overflow, digest mismatch, and expiry/release race;
- client disconnect during each lifecycle stage;
- slow MCP resource host and base64 expansion beyond host bound;
- foreign principal probing a known query/resource ID;
- stale/live/symlink UDS collision;
- forged supervisor channel, grant registration gap/out-of-order/replay, grant launched before
  acknowledgement, lost control channel, and daemon restart with stale grants/sessions;
- daemon restart between acceptance, result seal, and read;
- adapter package containing a forbidden live registry or semantic schema aggregate;
- high-cardinality identity or digest introduced as a metric label;
- v1 route still registered after the decommission fence.

---

## 15. Open measurements and bounded decisions

These questions do not block acceptance of the architecture, but they block freezing operational
constants or optional libraries:

1. What are the real MCP host limits for structured content, text, resources, and base64 bytes, and
   which capabilities are available before versus only during a request-scoped `Context`?
2. Does the post-WP04/WP19 execution path yield a true incremental `SendableRecordBatchStream` all
   the way to the resource writer? If not, that upstream design must be corrected; gRPC cannot hide
   eager collection.
3. What count/byte/TTL bounds satisfy resume needs for representative query durations and progress
   rates?
4. Which local spill/storage strategy supplies atomic seal, range reads, cleanup, and quota without
   exposing paths?
5. Does the extra local resource read for bounded inline JSON produce a material p99 regression?
6. Which exact versions of rich-status support interoperate under the locked Rust/Python graphs?
7. Which supported launchers can pass the supervisor's one-time grant through an inherited
   descriptor/pipe, and which require the specified owner-verified `0600` one-read fallback? The
   issuer, grant binding, consumption, renewal, and revocation semantics are already fixed in §6.3.
8. Are there any consumers outside this repository of the v1 service? The migration window depends
   on a proved census.
9. What independently decodable page size and, separately, opaque range chunk size minimize MCP
   overhead without causing event-loop lag or host rejection?
10. Does any future supported client consume Arrow natively outside MCP? Only that demand triggers a
    Flight design.

No queue size, cache size, compression mode, spill threshold, page size, keepalive interval, or
HTTP/2 window becomes normative until the corresponding probe answers the workload question.

---

## 16. Acceptance conditions for this target

This target is ready to become governing design when all of the following are true:

1. The design owner accepts the control/resource/introspection separation and the v2 compatibility
   posture.
2. A versioned SRV successor integrates the decisions, message meanings, authority matrix, error
   layering, resource behavior, and legacy disposition.
3. The active implementation plan is revised, dependency-closed, and independently re-audited in
   accordance with its §2 and §9 policies.
4. Exact FastMCP lifecycle/template/ResourceLink behavior and rich-error versions are compile- or
   runtime-probed where this review marks them as preflight questions.
5. The revised plan assigns every positive replacement, migration fence, deletion, and named proof
   to one packet before implementation.

Until then, the safe conclusion is:

> **The UDS gRPC/FastMCP topology is a sound development baseline, but the existing interface is not
> the best-in-class target. If this redesign is accepted, pause the current plan, publish the
> versioned design successor, and revise/re-audit the whole plan before execution resumes; do not
> interpret current partial gates or a static adapter catalog as release certification.**
