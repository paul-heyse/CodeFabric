---
artifact: interface-design-review
date: 2026-08-30
version: v1
status: complete
interface_path: docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.0.md
verdict: revision-required
---

# Daemon ↔ FastMCP Interface — Design Review and Target Architecture

One boundary is under review: everything between the Rust workspace daemon and the agent-facing
MCP surface. Concretely, the released `.proto` contract, the Tonic service that serves it, the
`grpc.aio` client that consumes it, and the FastMCP tools, resources and prompts that project it.

Three questions drive the review, in the order the request posed them:

1. **Is this the highest-performance shape available** for a local, same-user, single-adapter
   boundary carrying bounded control messages and unbounded Arrow/JSON results?
2. **Is the same information structure repeated** across the wire schema, the Rust model, the
   Python model, and the packaged adapter data — and where can a repetition be replaced by a
   derivation?
3. **Can the adapter explain, to an agent, an interface it does not own** — the semantic request
   grammar, the live capability surface, the query forms, the vocabularies, the limits?

The answers are: the *skeleton* is right and should be kept; the *typing* is inverted; the
*repetition* is systemic and traceable to one cause; and the *introspection* is close to absent
because the adapter ships a frozen copy of what it should be asking for.

---

## 0. Scope, method, and governing lenses

### 0.1 In scope

| Layer | Artifacts |
|---|---|
| wire contract | `contracts/rpc/cpg_query_service.proto` (404 lines, 9 RPCs, 5-variant `QueryEvent` oneof) |
| generation | `tooling/proto/` — one `grpc_tools.protoc` invocation → one `FileDescriptorSet` → `tonic_prost_build::Builder::compile_fds` |
| Rust service | `src/rpc.rs` (transport primitives, UDS peer credentials), `src/query_service.rs` (4,085 lines) |
| Rust result plane | `src/fabric/published_arrow_result.rs`, `src/fabric/arrow_result_resource.rs` |
| Python transport | `codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/{channel,client,arrow_resources}.py` |
| Python presentation | `.../server.py`, `.../contracts/{wire_models,schemas,index,model_registries}.py` and the four package-data JSON files |

### 0.2 Out of scope

QRY semantic compilation, FAB execution and Delta state, LIFE update waves, GEN provider
extraction. They are cited only where the boundary's shape depends on them.

### 0.3 Governing lenses

- **`PRIN`** — `docs/library_ref/full_data_fabric_design_principles_v2.md`. The staticness test
  (§A) and P1–P36 are the primary design authority for this review; every target-architecture
  decision below names the principle it satisfies.
- **`tonic-ref`** — `docs/library_ref/rust_grpc_daemon_advanced_reference_tonic_0.14.6.md`.
- **`grpcio-ref` / `protobuf-ref` / `orjson-ref`** — the three Python wire references.
- **`fastmcp-ref` / `pydantic-ref`** — the two Python boundary references.
- **`SRV`** — the serving specification named in this document's frontmatter.

### 0.4 Evidence tiers

Every claim below is labelled by the strongest tier that confirmed it, per `PRIN` P29:

- **T1 (construction)** — a compiler or type system decides it.
- **T2 (structural)** — `ast-grep`/parse-level match.
- **T3 (textual)** — `rg` over source, with the coverage envelope stated.

Findings in this document are predominantly **T3 with a T2 read-through**: file-and-line anchors
plus a read of the surrounding function. Where a claim is a *zero-hit* — "this is never emitted",
"this value is never enforced" — it is stated as a bounded textual claim over the named tree, not
as proof of absence. Tool versions: `rg` 15.2.0, `ast-grep` 0.45.1.

---

## 1. Present-state map

```text
agent
 │  MCP STDIO (4 tools, 4 resource templates, 0 prompts registered)
 ▼
FastMCP 3.4.7 adapter ─── one process, one workspace, one agent
 │  strict/frozen Pydantic boundary (generated from Contract IR)
 │  grpc.aio insecure_channel, 4 MiB symmetric ceilings
 ▼
private UDS, mode 0600, peer-UID checked at accept
 │  codefabric.cpgd.v1.CpgQueryService — 9 methods
 ▼
Tonic 0.14.6 daemon
 │  ProductionQueryService<B: SemanticQueryBackend>
 │  5 × Arc<Mutex<BTreeMap<..>>> registries + tokio::spawn per query
 ▼
FAB epoch / DataFusion / Delta
```

The nine methods and their current cardinalities:

| Method | Shape | State |
|---|---|---|
| `Handshake` | unary | implemented; version/feature/host-profile negotiation |
| `GetStatus` | unary | implemented; canonical JSON `PublicStatusView` in a `bytes` field |
| `ValidateQuery` | unary | implemented; partial (see DR-023) |
| `StartQuery` | unary | implemented; returns accepted handle before execution |
| `StreamQuery` | server stream | implemented; replays an in-memory `Vec<QueryEvent>` |
| `AttachQuery` | server stream | implemented in Rust, **never called from Python** |
| `CancelQuery` | unary | implemented |
| `ReadResult` | server stream | implemented; **returns `stream::once` on both branches** |
| `ReleaseResult` | unary | implemented; two branches |

---

## 2. What this design already gets right

These are load-bearing and the target architecture keeps every one of them. Stating them
explicitly matters, because several of the changes proposed in §5 would otherwise read as a
rewrite rather than a correction.

**K-1 — Accepted handle before long work.** `StartQuery` validates, mints `daemon_query_id`,
`resume_token` and `cancel_token`, spawns execution, and returns
(`src/query_service.rs:2139`–`2295`). Event delivery is a separate resumable RPC. This is exactly
`tonic-ref` §15.2 and §36.4, and it is the decision that makes cancellation and reconnect
addressable at all. Do not touch it.

**K-2 — One descriptor authority, two languages.** A single pinned `grpc_tools.protoc`
(libprotoc 35.1) emits the Python bindings *and* the `FileDescriptorSet` that
`tonic_prost_build::Builder::compile_fds` consumes (`tooling/proto/generate.rs:30`–`33`,
`tooling/proto/toolchain-identity.json`). This is the arrangement `tonic-ref` §6.3–§6.4 and §34.4
recommend, and under `PRIN` §A.3 it is the correct third-preference shape: a materialized copy
whose authority is the regeneration check, not the copy.

**K-3 — Kernel peer credentials at accept time.** `AuthorizedUnixStream::authenticate` reads
`SO_PEERCRED` before the stream is handed to Tonic and refuses a foreign UID
(`src/rpc.rs:104`–`119`, `:180`–`196`); the socket is created and immediately chmod'd to `0600`
(`src/query_service.rs:1296`). `tonic-ref` §22–§24. The identity is also surfaced as
`Connected::ConnectInfo`, which is the right hook for the per-call authorization proposed in §5.7.

**K-4 — Symmetric, intentional message ceilings.** 4 MiB encode and decode on the Tonic server
(`src/query_service.rs:1313`–`1314`) and on the `grpc.aio` channel
(`daemon/channel.py:5`–`11`). `tonic-ref` §20.2–§20.3.

**K-5 — Protobuf did not become a second semantic DTO graph.** The canonical semantic request and
response travel as opaque `bytes` under typed control framing
(`canonical_request_json`, `canonical_public_status_json`, `canonical_result_descriptor_json`).
`tonic-ref` §0.1 names growing the control schema into a duplicate fact graph as the *most common
architecture error* in this stack; this design avoided it. `PRIN` P3, P22.

**K-6 — A genuinely strict presentation boundary.** `StrictWireModel` is
`extra="forbid", strict=True, frozen=True, validate_default=True, hide_input_in_errors=True,
allow_inf_nan=False` (`contracts/wire_models.py:16`–`27`), and every public model is compiled from
one Contract IR rather than hand-written. `pydantic-ref` invariants 1–2; `PRIN` P32.

**K-7 — Authorization handle separated from content identity.** In the Arrow result route,
`authorization_resource_id` and `content_resource_id` are required to differ, and the model
enforces it (`daemon/arrow_resources.py:96`–`99`, `:109`–`112`). This is the single best piece of
`PRIN` P32 modelling in the boundary: the illegal state — using a content digest as a capability —
is unrepresentable.

---

## 3. Findings

Stable IDs `DR-001`…`DR-025`. Severity: **critical** (correctness or security), **major**
(architectural defect that will compound), **minor** (hygiene). Each finding names its evidence
and the principle or reference it violates.

### 3.A Repeated information structure

#### DR-001 — The terminal query outcome is modelled four times · major

The six orthogonal result states exist as:

1. `TerminalEvent.availability_state / freshness_state / limit_state / dependency_state /
   semantic_execution_state / completeness_state` — six `string` fields
   (`cpg_query_service.proto:302`–`315`);
2. the same states *again* inside the canonical response JSON body, which the client
   cross-checks field by field (`daemon/client.py:465`–`475`, `expected_states`);
3. closed `Literal[...]` unions on `QueryToolOutput` (`contracts/wire_models.py:250`–`267`);
4. the Rust `PublicStatusView` / `PublicServiceLimits` serde structs.

The existence of `expected_states` is the honest admission that representations 1 and 2 can
disagree. `PRIN` §A.3 (friction rule — a control whose only failure mode is "somebody forgot"),
`PRIN` P3 (one authoritative owner), Y3 *frozen enumeration*.

#### DR-002 — Twelve closed vocabularies travel as open `string` · major

`availability_state`, `freshness_state`, `limit_state`, `dependency_state`, `cleanup_state`,
`semantic_execution_state`, `completeness_state`, `QueryStatusSummary.execution_state`,
`StartQueryResponse.queue_class` (`:227`), `ValidateQueryResponse.cost_class` (`:202`),
`ReleaseResultResponse.release_state` (`:403`), and `ProgressEvent.phase` (`:266`) are all
`string`, while the *same* vocabularies are closed `Literal` unions in Python and closed matches
in Rust. Six other vocabularies in the same file are already proper `enum`s, so this is
inconsistency rather than a considered choice.

Consequence: a Rust-side typo produces a Pydantic `ValidationError` at the MCP boundary — the
furthest possible point from the defect — instead of a compile error. `PRIN` P32 (illegal states
unrepresentable, closed variant sets, exhaustive matches), P12 (schemas are executable contracts),
`protobuf-ref` §12.

#### DR-003 — The adapter ships a frozen ontology census · critical

`contracts/model_registries.py` is 61 KB of package data holding 73 entity kinds, 56 relation
kinds, 33 property kinds, 45 phrases, 22 capabilities, 81 public errors, 6 providers and 13
projections. `get_code_graph_reference(reference="capabilities")` returns it verbatim
(`server.py:151`–`155`). `contracts/model_artifact_index.json` adds 200 KB of *repository build
governance* to a runtime presentation wheel.

This is a Class 2 fact — "what currently exists" — materialized as a Class 1 artifact, which
`PRIN` §A.1 classifies as **falsely static** and §A.2 as "a cache with no invalidation". It is
also the exact prohibition in `SRV` §3 ("No package data may contain current model, query, phrase,
provider, function, capability, proof, or epoch state"), `SRV` §7.3, and `SRV` §11.1 ("never an
adapter cache or packaged census"). `PRIN` P26, P3, Y3 *hand-maintained ledger*.

#### DR-004 — Contract version literals are declared in six places · major

`RPC_VERSION = "1.0"` / `SEMANTIC_QUERY_VERSION = "1.3"` (`client.py:44`–`45`); the same literals
hard-compared in `query_service.rs:1947`–`1955`; `FastMCP(version="1.3.0")` (`server.py:87`);
`@mcp.tool(version="1.3")` at `server.py:190, 372, 401, 424`; `PublicToolMeta(contract_version=
"1.3", daemon_rpc_version="1.0")` constructed by hand at `server.py:272`–`277`;
`SchemaFingerprint(version="1.3")` for every fingerprint (`client.py:153`); and the URL-shaped
stub `https://codefabric.dev/schema/1.3/...` (`server.py:176`).

`PRIN` P28 explicitly lists "a version constant bumped by hand" as the wrong form, and P31 names
this exact shape — *n* files that must be edited together where the only control is memory.

Compounding it: every one of these says **1.3** while the suite and the governing specification
are at **2.0.0**. The adapter is advertising a contract generation the design corpus has moved past.

#### DR-005 — The Arrow result is described three times, with two hand-written reconcilers · major

- `ArtifactReadyEvent` carries `artifact_checksum`, `content_type`, `encoding`,
  `lease_expires_at_unix_ms`, `result_contract_version`, `arrow_release` (`:282`–`296`) — every
  one of which is *also* inside `canonical_result_descriptor_json`. `client.py:369`–`390` exists
  solely to prove the two agree.
- `ArrowResultManifest.subresources` restates `ArrowResultPackageDescriptor.relations` field for
  field — `relation_id`, `schema_checksum`, `schema_byte_length`, `content_checksum`, `row_count`,
  `batch_count`, `byte_length`, coverage. `_validate_manifest_matches_descriptor` exists solely to
  prove *those* agree.

Three representations of one fact, two reconciliation functions, and two failure modes that only
exist because the representations exist. `PRIN` P3, §A.3.

#### DR-006 — `ResultChunk` carries four redundant fields and two mutually exclusive shapes · minor

`uncompressed_length` duplicates `len(payload)`; `final_chunk` duplicates
`next_offset == total_length` (the Python model asserts both relations at
`arrow_resources.py:288`–`296`); `artifact_checksum` and `content_checksum` are each empty on one
of the two routes; `authorization_resource_id` is empty on the legacy route. One message shape
with route-dependent emptiness, where a `oneof` — or one route — would make the illegal
combinations unrepresentable. `PRIN` P32.

#### DR-007 — Two parallel result routes with no plan to converge · major

`read_result` branches on `authorization_resource_id.is_empty()`
(`src/query_service.rs:2476`) into either the `PublishedArrowResultRegistry` or a legacy in-memory
`BTreeMap<String, ResultArtifact>`; `release_result` branches on `owner.is_some()`
(`:2566`). Python mirrors the split with `_lease_cache` and `_arrow_leases`
(`client.py:122`–`123`). Two write paths for the same concept violates `PRIN` P34 ("no second
route, including test-only ones") and P3.

#### DR-008 — Three different inline thresholds, none derived from the others · major

| Value | Source | Authority claimed |
|---|---|---|
| 512 KiB automatic / 4 MiB hard | `SRV` §9 | specification |
| 1 MiB (`maximum_inline_response_bytes`) | `query_service.rs:823` | daemon, reported to client |
| 384 KiB default | `settings.py:58`–`61` | adapter, **used for the decision** |

The `automatic` inline-vs-resource decision is made in Python from the Python default
(`client.py:414`–`419`) and ignores the daemon-reported limit entirely. The daemon owns limits;
the adapter decides using its own number. `PRIN` P3, P27 (a declaration execution does not read).

### 3.B Introspection — the adapter cannot describe what it does not own

#### DR-009 — The handshake response is negotiated, then discarded · major

`Handshake` returns `ReadinessSummary.supported_query_forms`, `supported_language_codes`,
`capability_codes`, `active_schema_fingerprints`, `installed_bundles` and a full
`EffectiveLimitsProfile` (`query_service.rs:1988`–`2011`). The adapter stores the response
(`client.py:186`) and thereafter reads exactly one field from it —
`effective_limits.maximum_payload_chunk_bytes` (`client.py:590`–`596`). None of the capability,
form, language, bundle or fingerprint content reaches an agent.

`PRIN` P27 states the test directly: change the declaration, and observable behavior must change.
Changing `supported_query_forms` in the daemon changes nothing an agent can see. Y3 *declaration
theater*.

#### DR-010 — There is no live schema; the released schemas are a dangling pointer · critical

`get_code_graph_reference("request_schema")` returns a synthetic `$ref` to
`https://codefabric.dev/schema/1.3/cpg-semantic-query-request.schema.json`
(`server.py:168`–`180`) — an unresolvable URL. `"query_specification"` returns the seven-word
literal `"# CodeFabric semantic query specification\n\nContract version: 1.3\n"`.

Meanwhile the `query_code_graph` tool's `request` parameter is typed `dict[str, Any]` with a
one-line description (`server.py:198`–`199`), so MCP-level schema introspection also yields
nothing. An agent asking "what may I put in `request`?" gets, from every available channel, no
answer. This is the single largest gap against the stated goal.

#### DR-011 — No progress reaches the agent · major

`CpgQueryService` never constructs a `ProgressEvent`. The variant is in the `oneof`
(`:329`–`336`), the header-extraction match arm exists (`query_service.rs:1451`), and the only
`ProgressEvent` constructions in the tree belong to `provider_runtime.rs`, a different service.
Symmetrically, `server.py` never calls `ctx.report_progress` — `ctx` is used only to reach
`lifespan_context` (`server.py:99`). `SRV` §8 specifies an eleven-phase public vocabulary; neither
side implements it. `fastmcp-ref` §9.5. *(T3 claim, envelope: `src/**` and
`codefabric-cpg-mcp/src/**`.)*

#### DR-012 — Resume is implemented and unreachable · major

`AttachQuery` validates query ID, resume token, `after_sequence` and `after_event_checksum`
(`query_service.rs:2320`–`2360`) — and no Python code references it. `StreamQuery` is called once
with `after_sequence=0` inside a single `try` (`client.py:322`–`330`); a broken stream cancels the
query rather than reattaching. `SRV` §8, `tonic-ref` §15.3, §36.6, §37.4.

#### DR-013 — Result resources are single-use and destructive on read · minor

`cpg://result/{artifact_id}` calls `_read_and_release`, which releases the lease after assembling
the payload (`client.py:685`–`695`); a second read of the same URI fails with "result resource is
absent or already released". The Arrow route is subtler: the lease is released once *every*
subresource has been consumed (`client.py:604`–`613`), so reading the manifest and all relations
silently invalidates all of them. A read-only MCP resource with at-most-once semantics is
surprising (`PRIN` Y4, least astonishment) and makes any agent retry a hard failure.

Related: `ResourceDelivery` returns URI strings inside `structured_content` rather than MCP
resource-link content blocks, so hosts that render links cannot.

### 3.C Transport, flow control, and performance

#### DR-014 — Nothing streams · critical

Despite `ReadResult` being declared `unary-stream`, both branches return
`Box::pin(stream::once(...))` (`query_service.rs:2513`–`2515`, `:2557`–`2559`). The client then
loops those single-chunk calls and reassembles the whole artifact in memory with
`b"".join(chunks)` (`client.py:680`). `ResponseChunkEvent` — the variant designed to carry inline
result bytes incrementally — is never emitted by this service.

The net effect is an architecture that avoids large messages by *externalizing everything*, then
reads each externalized artifact back into full memory at both ends. `tonic-ref` §19.0, §19.3,
§11.5, §20.0.

#### DR-015 — The event log is an unbounded in-memory `Vec` · critical

`QueryHandleState.events: Vec<QueryEvent>` (`query_service.rs:1082`–`1083`) is appended to by the
producer regardless of consumer progress, and `stream_after` clones each event out under a mutex
(`:1459`–`1481`). There is no channel, no capacity, and therefore no path for HTTP/2 flow control
to reach DataFusion. `tonic-ref` §19.1 names this precisely: an unbounded bridge converts remote
backpressure into unbounded daemon memory. It is the same defect as an
`unbounded_channel`, wearing a different costume.

#### DR-016 — Result bytes are held in daemon memory with no quota and no sweeper · critical

`artifact_records: Arc<Mutex<BTreeMap<String, ResultArtifact>>>` (`query_service.rs:1104`) holds
full artifact bytes, inserted at `:1811`–`1814` and removed only on explicit release. The
`ResultArtifactStore` does have `prune_expired_query_artifacts` (`:530`), but it operates on the
durable artifact bundles, not on this map. `SRV` §10 specifies a one-hour TTL, 2 GiB per-agent,
8 GiB per-workspace and 10 GiB global quotas; none is enforced on this path. An adapter that
crashes without releasing leaks the full result set until daemon restart.

#### DR-017 — Advertised concurrency limits are never enforced · major

`EffectiveLimitsProfile.maximum_concurrent_queries` is the constant `4`
(`query_service.rs:824`, `:2082`) and is reported in both `Handshake` and `GetStatus`. Outside
tests, no code reads it. `Server::builder()` adds exactly one layer — the peer-UID interceptor
(`:1319`–`1322`) — so there is no admission control of any kind: no per-agent semaphore, no
queueing, no load shedding. `SRV` §14's "two active and four queued queries per agent" is
unimplemented.

`tonic-ref` §28.2 and §40.8; `PRIN` P20 (advertise only what a prover confirms), Y3 *metadata
theater* and *capability overclaiming*.

#### DR-018 — Deadlines are flat, not nested, and the nesting inversion defeats error layering · major

One value, `settings.query_timeout_seconds` (default 120 s), is used simultaneously as:

- the FastMCP tool timeout — `@mcp.tool(timeout=120.0)` (`server.py:193`);
- every gRPC per-call `timeout=` (`client.py`, nine call sites);
- the daemon deadline — `deadline_unix_ms = int((time.time() + query_timeout_seconds) * 1000)`
  (`client.py:313`).

`SRV` §8 requires `MCP host ≥ FastMCP tool ≥ adapter gRPC ≥ daemon`, with cleanup reserve.
Here the daemon's deadline and the client's RPC deadline expire at the same instant, so the
adapter reliably observes a transport `DEADLINE_EXCEEDED` instead of the daemon's `TerminalEvent`
carrying its canonical error record. The careful error layering of `SRV` §11.2 — semantic failures
are records, transport failures are statuses — is bypassed at exactly the moment it matters most.
`tonic-ref` §18.0, §18.6.

#### DR-019 — Zstd is declared on the wire and rejected at runtime · minor

`PAYLOAD_COMPRESSION_ZSTD` (`:41`) and `CpgdFeature.ZSTD_PAYLOADS = 4`
(`contracts/model_registries.py`) are part of the released contract and the advertised feature
mask; `start_query` and `read_result` reject anything but `Identity`
(`query_service.rs:2172`–`2180`, `:2455`–`2461`). `PRIN` P20, Y3 *capability overclaiming*.

Note the correct resolution is **removal, not implementation**: `tonic-ref` §20.4 says the default
for a local UDS boundary is no gRPC compression.

#### DR-020 — Per-call authorization does not exist · critical

`CredentialProof` appears in exactly one message — `HandshakeRequest` (`:120`). Every subsequent
method authorizes by consulting a process-global claim map keyed only by workspace string
(`QueryAuthorization::authorize_workspace`, `query_service.rs:789`–`804`), with no reference to
the calling connection, credential, or identity. `agent_instance_id` is a **caller-supplied
string** used as a map key for host-profile binding (`:2093`–`2103`, `:2153`–`2163`).

The one genuine binding is the handle tokens: `resume_token` and `cancel_token` are MACs over
query, agent and workspace (`:2226`–`2238`), so possession proves the holder saw a
`StartQueryResponse` issued for that agent. That is real, and it is why `StreamQuery` can
authorize on the token alone (`:2298`–`2317`). But it is bearer authority over one query, minted
once, with no expiry and no anti-replay identity — not a reauthorization of the caller.

The effective security model is therefore: same UID, plus one shared bearer token presented once.
`SRV` §12 requires a credential bound to agent instance, adapter process, workspace, operations,
ACL profile, expiry and anti-replay identity, reauthorized on **every** query, status projection,
source context, artifact operation and cancellation.

`tonic-ref` §23.3, §23.4, §40.2, §40.4 are unusually blunt here: the socket is not authorization,
the path is not authorization, an opaque artifact ID is not bearer authority, and a caller-supplied
PID — or ID — is not a principal.

#### DR-021 — No health, no reflection, no runtime descriptor self-check · minor

The descriptor fingerprint exists at build time (`tooling/proto/toolchain-identity.json:
descriptor_sha256`), but the running daemon never asserts its embedded descriptor matches, serves
no `grpc.health.v1.Health`, and offers no reflection even in development builds. `tonic-ref` §30,
§31, §34.3. This is the diagnostic surface that makes "did the binary and the packaged schema
drift?" answerable at all.

#### DR-022 — Channel construction races daemon startup and permits an invalid target · minor

`create_local_channel` is `grpc.aio.insecure_channel(target, options=...)` with no
`wait_for_ready`, no keepalive, and no channel-ready gate (`channel.py:14`–`17`), so the first
`Handshake` competes with daemon socket bind. Separately, `Settings.validate_daemon_target`
accepts `tcp://` (`settings.py:93`–`98`), which `SRV` §1 excludes from the local profile and which
`insecure_channel` does not accept as a scheme — it expects `unix:…` or `host:port`. A misconfigured
target fails late and confusingly. `grpcio-ref` §18.3, §35.3.

### 3.D Contract hygiene

#### DR-023 — `ValidateQuery` returns hard-coded emptiness where unknown belongs · major

`server.py:392`–`398` returns `resolved_semantics={}`, `capability_requirements=()`,
`warnings=()`, and `dependency_graph={"checks": [...]}` built from
`provisional_snapshot_checks`, which the daemon populates with the single literal
`vec!["workspace-authorized"]` (`query_service.rs:2126`). `cost_class` is the literal
`"bounded-wave5"` (`:2129`) — a build-phase label on a released wire field.

An empty collection presented where "not computed" is the truth reads to an agent as "no
capabilities required, no warnings". `AGENTS.md` §0.2 ("absence is never proof of absence") and
`PRIN` P20 ("unknown is a value that must be representable in the contract").

#### DR-024 — Six of the specification's ten acceptance oracles do not resolve · major

`SRV` §15 names ten `just` recipes as the boundary's executable acceptance obligations. Resolving
them against `just --dump --dump-format json`:

| Named in `SRV` §15 | Resolves |
|---|---|
| `proto-contract-check` | **no** |
| `provider-protocol-check` | yes |
| `adapter-domain-boundary-check` | **no** |
| `adapter-package-authority-zero-state-check` | **no** |
| `semantic-delivery-vertical-check` | **no** |
| `semantic-query-conformance-check` | yes |
| `dynamic-reference-delivery-check` | **no** |
| `adapter-test` / `adapter-stdio-test` / `adapter-ci-fast` | yes |
| `package-interop-check` | **no** |
| `access-catalog-isolation-check` / `public-leakage-negative-check` / `resource-governance-check` / `daemon-static-bundle-target-zero-state-check` | **no** |

A contract clause whose oracle does not exist is not a contract (`PRIN` P25); the repository
already owns the gate that would catch this class — `just oracle-substance-check` — and the
specification's own table is not currently inside its envelope.

#### DR-025 — Declared Python floor is two minor versions below the code's actual floor · minor

`client.py:195` uses PEP 758 unparenthesized multi-exception syntax
(`except grpc.RpcError, DaemonProtocolError:`), which parses only on Python ≥ 3.14. Ruff
(`target-version = "py314"`) and Pyrefly (`python-version = "3.14"`) agree with the code;
`pyproject.toml` declares `requires-python = ">=3.12"`. A 3.12 or 3.13 install resolves, then
fails at import with a `SyntaxError`.

---

## 4. Root cause: three misplacements, not twenty-five defects

The findings above collapse into three structural causes. Fixing the causes fixes most of the
findings; fixing the findings individually would not fix the causes.

### 4.1 The typing is inverted across the boundary

Protobuf — the layer with real enums, field numbers, wire-compatible evolution rules, and
cross-language code generation — carries **open strings** for twelve closed vocabularies (DR-002).
Python — the presentation layer, which by `SRV` §2 must not be a semantic authority — carries the
**closed `Literal` unions** that define what those strings may be (DR-001).

The consequence is that Python became the de facto vocabulary authority by being the only layer
that closes the set. Every reconciliation function in `client.py` and `arrow_resources.py` exists
to re-establish, at runtime, a constraint that should have been established by construction.

### 4.2 Class 2 facts were materialized as Class 1 artifacts inside the Python package

The adapter *ships* what it should *ask for*: the ontology census, the capability list, the phrase
registry, the error registry, the contract version, the schema URLs, the inline threshold
(DR-003, DR-004, DR-008, DR-010). Every one of these is "what currently exists", which `PRIN` §A.1
classifies as derived-on-demand.

This is also, exactly, why introspection is absent. The adapter has nothing to *ask*, because it
was designed to already know — and what it knows is a snapshot of a system that has since moved
to suite 2.0.0 while the package still says 1.3.

### 4.3 The stream is a queue

Nothing between DataFusion and the socket carries backpressure (DR-014, DR-015). Because there is
no backpressure, every size problem must be solved by *externalizing*; because externalization is
the only tool, every large result is materialized in full — once in the daemon's memory
(DR-016), once in the adapter's (DR-014) — and then hand-verified with checksums, offsets and
round-trip counters that only exist to catch errors the design created.

---

## 5. Target architecture

Nine changes, `T1`–`T9`. Each names the principle it satisfies and the executable oracle that
decides it. §5.11 records what is deliberately *not* adopted, so the target does not drift into
over-engineering.

### 5.0 The Y2 design questions, answered for this boundary

`PRIN` Y2 requires these answers before implementing or materially revising a subsystem.

| Question | Answer for the daemon ↔ FastMCP boundary |
|---|---|
| Semantic concept represented | An authorized agent's bounded, resumable, epoch-pinned interaction with one immutable present-state fact graph |
| Authoritative representation | The released `.proto` (control) plus the released canonical QRY JSON profile (semantics). Both owned by the daemon |
| Derived representations | Rust prost types, Python `_pb2`, Pydantic public models, MCP tool/resource schemas — all generated, none authoritative |
| What is invariant | One terminal event per query; monotonic sequence; epoch never migrates within a query; checksum identity of the canonical response independent of delivery; no semantic computation in Python |
| Legal variation | Delivery mode (inline / resource / Arrow-native); freshness policy; host capability profile; negotiated feature bits |
| Hierarchy | `Handshake` → session; `StartQuery` → accepted query; `StreamQuery`/`AttachQuery` → event stream; `ArtifactReady` → leased result; subresource → bytes |
| Lifecycle phases | negotiate → validate → accept → pin → plan → execute → materialize → deliver → release |
| Logical vs physical | Logical: request relations, result relations, coverage, states. Physical: chunking, Arrow IPC framing, HTTP/2 windows, socket. The wire carries the first and never lets the second leak into semantics |
| Canonical boundary type | Arrow IPC for relation payloads (`PRIN` P22); RFC 8785 canonical JSON for control projections; Protobuf for control framing |
| Provenance | Emitted by the daemon operation into the result descriptor and terminal event, in the same transaction as the artifact (`PRIN` P9) |
| Identity and versioning | Content-derived `b3:` digests throughout; epoch ID; descriptor fingerprint; released wire major in the package name |
| Policy enforcement point | One Tonic interceptor resolving peer credential + capability credential → typed `AuthorizedCaller` in request extensions. Nothing downstream re-decides |
| Capability advertisement | Only from the admitted epoch, only where a prover confirms; `UNKNOWN` is representable (`PRIN` P20) |
| Drift detection | Descriptor fingerprint comparison at startup and at handshake; no recorded staleness markers (`PRIN` P28) |
| Explainability | Every rejection carries its layer, code, retryability and scope; every result carries its coverage accounting |
| Higher-level abstraction available | Yes — Protobuf enums, `oneof`, MCP resource templates, FastMCP `output_schema`. Use them before hand-rolling (`PRIN` P14) |
| Contract proof | §5.12's oracle table, one row per clause, each with a named falsifying change |
| Class 1 declarations | Enumerated in §5.13 |
| What must a human remember | Nothing. Every synchronization point identified in §3.A is closed by derivation in §5.2–§5.3 |

### 5.1 The four planes, sharpened

`SRV` §2's four-plane split is correct and is retained. One rule is added, because it is the rule
whose absence produced DR-001 and DR-002:

> **The control plane carries closed vocabularies and opaque payloads. It never carries an open
> string where a vocabulary exists, and never carries a JSON object whose shape the control plane
> also describes.**

Corollary: if a fact appears both as a typed Protobuf field and inside a canonical JSON payload in
the same message, one of the two is deleted — not cross-checked.

### 5.2 T1 — One vocabulary authority, three projections

`codefabric-model` already owns the registries (`ENUM_TRIPLES`, `REGISTRY_IDS`). Extend its
projection set so the *same* source emits:

1. a generated `contracts/rpc/cpg_vocabularies.proto` containing one `enum` per closed vocabulary,
   imported by `cpg_query_service.proto`;
2. Rust enums with `TryFrom<i32>` — already free from prost;
3. Python `Literal`/`StrEnum` members in `wire_models.py` — already generated today.

Then **delete** the twelve `string` fields (DR-002), the `expected_states` cross-check
(`client.py:465`–`475`), and every hand-written `Literal` list that is not a projection.

- Satisfies: `PRIN` P3, P12, P26, P32; `protobuf-ref` §12; `tonic-ref` §0.3.
- Wire compatibility: new enum-typed fields take new numbers; the string fields are `reserved`
  (`protobuf-ref` §26.1–§26.2). Not a rename.
- **Executable oracle:** extend `just query-form-contract-check` to assert
  `set(proto enum values) == set(Rust enum variants) == set(Pydantic Literal args)` by *computing*
  all three sets from the descriptor and the loaded modules — never by enumerating them
  (`PRIN` §A.3 rule 2). **Falsifying change:** add a value to one projection only.

### 5.3 T2 — Replace the packaged census with a live contract projection

Fold introspection into the existing `GetStatus` rather than adding a tenth method, using a typed
selector so the method count does not grow:

```proto
message StatusRequest {
  // ... existing fields ...
  repeated ContractProjection projections = 4;   // new
}

enum ContractProjection {
  CONTRACT_PROJECTION_UNSPECIFIED     = 0;
  CONTRACT_PROJECTION_REQUEST_SCHEMA  = 1;   // released QRY request JSON Schema
  CONTRACT_PROJECTION_RESPONSE_SCHEMA = 2;
  CONTRACT_PROJECTION_QUERY_FORMS     = 3;   // the eight forms + per-form parameter schema
  CONTRACT_PROJECTION_VOCABULARIES    = 4;   // phrases, kinds, capabilities, public errors
  CONTRACT_PROJECTION_CAPABILITIES    = 5;   // per-capability state incl. UNKNOWN / DEGRADED
  CONTRACT_PROJECTION_LIMITS          = 6;   // the enforced EffectiveLimitsProfile
  CONTRACT_PROJECTION_IDENTITY        = 7;   // epoch, suite version, descriptor fingerprint
}
```

Each projection returns canonical JSON bytes plus its checksum, drawn from the **admitted epoch**,
never from a package resource.

Then delete from the wheel: `model_registries.py`, `model_artifact_index.json`, and the
`_reference_content` literals (`server.py:150`–`180`).

`adapter-schemas.json` (74 KB) and `adapter-fingerprints.json` are a different case and should be
resolved differently: they *are* a legitimate Class 2 cache with a re-derivation oracle
(`schemas.py:26`–`48` re-verifies every fingerprint at import). But the derivation is cheap and
local — `TypeAdapter.json_schema()` over `MODEL_ADAPTERS` — so `PRIN` §A.3 rule 1 applies:
**derive on read** and delete both files. This removes 78 KB of package data and one whole class
of regenerate-and-commit friction (Y3 *regenerate-and-commit treadmill*).

- Satisfies: `PRIN` §A.1–§A.3, P3, P20, P26, P27, P28; `SRV` §3, §7.3, §11.1.
- **Executable oracle:** a new `just adapter-package-authority-zero-state-check` asserting the
  built wheel contains no model, phrase, capability, error, or epoch state — implemented as a
  relational query over the built wheel's resource manifest, not a filename regex (`PRIN` P29).
  **Falsifying change:** add one registry entry back to package data.
- **Second oracle:** `just dynamic-reference-delivery-check` — mutate a capability in the daemon's
  epoch and assert `get_code_graph_reference` output changes. This is the `PRIN` P27 causality
  test, executable.

### 5.4 T3 — Introspection as a first-class MCP surface

This is the change that answers the third question in the brief. Five parts:

**(a) The tool's input schema becomes the daemon's released request schema.** Today
`request: dict[str, Any]` teaches an agent nothing. Bind the released QRY request JSON Schema as
the `query_code_graph` input schema after handshake, via a FastMCP `Provider` or `ToolTransform`
so the component is published with the live schema rather than a static annotation
(`fastmcp-ref` §14.14, §15). Keep `strict_input_validation=True`; the daemon remains the semantic
authority (`SRV` §2, `protobuf-ref` §39.1 — the *released JSON Schema* is the right thing to
publish, never the generated Protobuf message).

This single change moves the request grammar from "unavailable" to "in the tool listing".

**(b) Resource templates for everything the daemon can project.** RFC 6570 templates
(`fastmcp-ref` §7.12–§7.14):

```text
codefabric://contract/schema/{artifact}
codefabric://contract/form/{form_id}
codefabric://contract/vocabulary/{registry}{?prefix,limit}
codefabric://status/capability/{capability_code}
codefabric://epoch/{epoch_id}/summary
```

Each resolves through the `GetStatus` projection of T2 and is therefore live by construction.

**(c) `get_code_graph_reference` becomes a projection, and its selector is daemon-enumerated.**
The `Literal[...]` of ten hard-coded reference names (`server.py:428`–`440`) is replaced by the
selector set the daemon reports, so a new reference kind needs no adapter release.

**(d) Prompts render from live vocabulary.** `author_code_graph_query` and
`interpret_code_graph_facts` — named in `SRV` §7.4 and currently unregistered — are added and
render the *current* forms and phrases, not packaged text.

**(e) Capability state is three-valued.** Every advertised capability carries
`CURRENT | DEGRADED | UNKNOWN`, and `UNKNOWN` is the default until a prover confirms
(`PRIN` P20). The same applies to `ValidateQuery`: DR-023's hard-coded empty collections become
explicit `not_computed` markers.

- **Executable oracle:** `just semantic-delivery-vertical-check` extended to drive an in-memory
  FastMCP client (`fastmcp-ref` §29.7) through: list tools → read the published input schema →
  construct a request that validates against it → execute → assert success. **Falsifying change:**
  publish a schema that does not describe the accepted grammar.

### 5.5 T4 — Make the stream a stream

Five coordinated changes, in dependency order:

1. **Emit `ProgressEvent`** from the daemon's execution phases; the phase vocabulary becomes an
   enum under T1. The eleven phases are already specified in `SRV` §8.
2. **Replace `QueryHandleState.events: Vec<QueryEvent>`** with a bounded
   `tokio::sync::mpsc::channel(N)` per subscriber, plus a bounded replay ring sized by the resume
   window. The producer `await`s `send()`, so a slow agent applies backpressure through HTTP/2 to
   the execution engine (`tonic-ref` §19.2, §19.3). Capacity is a measured parameter, not a
   "higher is faster" knob.
3. **Emit `ResponseChunkEvent`** for inline results, so the inline path is
   `batch → validate → chunk → send` with no full-artifact materialization in the daemon
   (`tonic-ref` §19.3).
4. **`ReadResult` genuinely streams ranges**, and `ResponseChunkEvent.payload` /
   `ResultChunk.payload` get `prost_build::Config::bytes([...])` so Rust-side ownership copies drop
   (`tonic-ref` §11.3). Do not claim end-to-end zero copy — §11.4.
5. **The adapter consumes incrementally**: `async for` over events, `ctx.report_progress` per
   `ProgressEvent` (`fastmcp-ref` §9.5), and at most one chunk plus a running BLAKE3 state in
   memory — never the assembled artifact. Arrow relations are handed to the caller as resource
   links and streamed on read rather than buffered at 64 MiB (`arrow_resources.py:597`).

- Satisfies: `PRIN` P33 (functional core / thin shell), P22; `tonic-ref` §19 entire.
- **Executable oracle:** the `tonic-ref` §19.5 slow-consumer test — a deliberately slow Python
  consumer against a fast producer, asserting bounded daemon RSS, bounded queue depth, no lost
  sequence, prompt cancellation, correct terminal semantics, no premature lease release.
  **Falsifying change:** replace the bounded channel with an unbounded one; RSS assertion fails.

### 5.6 T5 — Admission, quotas, and leases that exist

**Layer order** (`tonic-ref` §28.3–§28.8), stated because layer order is semantic:

```text
peer-UID interceptor          reject foreign UID at accept
credential interceptor        resolve AuthorizedCaller into extensions   (T6)
trace/correlation layer       semantic_request_id · mcp_call_id · rpc_attempt_id · daemon_query_id
deadline layer                observe grpc-timeout, reserve cleanup      (T7)
timing/metrics layer
query-admission semaphore     per agent+workspace: 2 active / 4 queued
```

The admission semaphore is **keyed on the query, not the connection**. `tonic-ref` §28.2 is
explicit that a connection-wide concurrency limit is not query admission: conflating them lets
heavy streams occupy every slot and starves the `CancelQuery` and `GetStatus` calls that would
have stopped them. `GetStatus` must stay cheap and available while the workspace is bootstrapping
(`tonic-ref` §36.2).

**Result registry**: byte quotas per agent / workspace / global, TTL sweeper, and — critically —
`EffectiveLimitsProfile` populated *from the enforcer's own state* rather than from a parallel
constant. `PRIN` P27: change the limit, and observable behavior must change.

**One result route**: delete the legacy in-memory `artifact_records` path entirely. `ResultOwner`
and `authorization_resource_id` become required, and the branches at `query_service.rs:2476`,
`:2516` and `:2566` collapse. `PRIN` P34.

- **Executable oracle:** `just resource-governance-check` — drive N+1 concurrent queries, assert
  the N+1st is queued not executed, assert `GetStatus` still answers under load, assert quota
  rejection carries `RESULT_TOO_LARGE_FOR_HOST` or the quota error, assert TTL expiry frees bytes.
  **Falsifying change:** raise `maximum_concurrent_queries` and observe the queued count change —
  if it does not, the value is still decorative.

### 5.7 T6 — Per-call authorization bound to a verified principal

```text
Handshake                       peer UID + capability token  ->  mints a short-lived,
                                                                  expiring, anti-replay
                                                                  session credential

every subsequent call           credential in gRPC metadata
                                  + VerifiedPeerIdentity from ConnectInfo
                                  -> interceptor resolves AuthorizedCaller
                                     { agent_instance, workspace, operations, acl_profile }
                                  -> inserted into request extensions
```

The structural consequence matters more than the mechanism: **`agent_instance_id` and
`workspace_id` are removed from request message bodies.** They are derived from the credential, so
a caller cannot name someone else — the illegal state becomes unrepresentable rather than
rejected. `PRIN` P32, P13; `tonic-ref` §16.1–§16.2, §23.3, §40.2, §40.4.

Credential material travels as metadata, which is transport context and not payload
(`grpcio-ref` invariant 6) — that is exactly what it is.

- **Executable oracle:** `just access-catalog-isolation-check` extended with the **denied case**
  (`PRIN` P13 requires it): agent A creates an artifact, agent B presents a valid credential and
  the known artifact ID, and the read fails. Plus: expired credential fails; replayed credential
  fails; credential for workspace X cannot read workspace Y.
  **Falsifying change:** drop the interceptor; the denied cases start passing.

### 5.8 T7 — A deadline ladder that cannot be inverted by configuration

Derive all four budgets from one setting plus fixed reserves, so the ordering is established by
construction rather than by three independently-set values that happen to agree:

```text
host_budget            H                       (from the MCP request, when supplied)
tool_timeout       =   H - host_reserve
grpc_deadline      =   tool_timeout - adapter_reserve
daemon_deadline    =   grpc_deadline - daemon_cleanup_reserve
```

Propagate as `grpc-timeout` on the call rather than wrapping in a local `asyncio.wait_for`
(`tonic-ref` §18.2), so the server sees a real deadline. Reserve cleanup time explicitly (§18.6),
which is what makes the daemon able to emit its `TerminalEvent` with a canonical error record
before the client's RPC dies — restoring the error layering DR-018 defeats.

- **Executable oracle:** a test asserting `tool_timeout > grpc_deadline > daemon_deadline` for the
  full configured range, plus a slow-query test asserting the adapter receives a
  `TerminalEvent` with `DEADLINE_EXCEEDED` and **not** a transport `DEADLINE_EXCEEDED`.
  **Falsifying change:** set the reserves to zero.

### 5.9 T8 — Runtime contract self-proof

1. **Embed the FDS** and assert at startup that its fingerprint equals
   `tooling/proto/toolchain-identity.json:descriptor_sha256`, and that service full name, method
   names, cardinalities and critical field numbers match (`tonic-ref` §34.3).
2. **Serve `grpc.health.v1.Health`** (`tonic-ref` §30) — while keeping `Handshake` as the
   readiness authority (§30.1). Health answers "is the process serving"; only `Handshake` answers
   "are we compatible".
3. **Reflection in development builds only** (`tonic-ref` §31.2), with the §31.4 test asserting the
   reflected descriptor graph equals the canonical fingerprint.
4. **Expose the descriptor fingerprint** through the `CONTRACT_PROJECTION_IDENTITY` projection so
   the adapter can prove its `_pb2` descriptors match the live daemon — comparing the **semantic
   descriptor graph**, never generated source text (`tonic-ref` §34.4, §35.1).

- **Executable oracle:** `just proto-contract-check` — regenerate the descriptor, compare against
  the committed one, assert the running binary's embedded descriptor matches, assert the Python
  `_pb2` descriptor pool agrees on package, service, method set and every field number.
  **Falsifying change:** renumber one field in the `.proto`.

### 5.10 T9 — Message shape hygiene

- `QueryEvent` stays a closed `oneof` — it is correct.
- `ResultChunk` loses `uncompressed_length` and `final_chunk` (both derivable), and the legacy
  route's empty-field variants disappear with T5's single-route collapse. `PRIN` P3.
- `ArtifactReadyEvent` loses the six fields that duplicate `canonical_result_descriptor_json`,
  and `client.py:369`–`390` is deleted with them (DR-005).
- `ArrowResultManifest.subresources` is either deleted in favour of the package descriptor, or
  the descriptor becomes the manifest — one of the two, not both plus a reconciler.
- `PAYLOAD_COMPRESSION_ZSTD` and `CpgdFeature.ZSTD_PAYLOADS` are removed until a prover exists
  (`PRIN` P20); the correct steady state for local UDS is no compression (`tonic-ref` §20.4).
- The canonical semantic response stays `bytes`. Do **not** adopt `pbjson` for it: the released
  QRY JSON profile is its own authority and must not be silently redefined as a ProtoJSON
  serialization of a Rust control DTO (`tonic-ref` §33.2).

### 5.11 Deliberately not adopted

Recorded so the target is bounded, and so a later reader does not re-litigate settled questions.
`PRIN` Y4 (KISS, YAGNI).

| Rejected | Why |
|---|---|
| Bidirectional streaming for cancellation symmetry | `tonic-ref` §15.1 — the accepted-handle + stream + explicit Cancel/Attach model is simpler to recover and test |
| gRPC compression on the UDS | `tonic-ref` §20.4 — no bandwidth charge, very low RTT, large results already externalize; costs CPU and latency |
| TLS on a same-user private socket | `tonic-ref` §41 |
| `orjson` anywhere on the Protobuf path; ProtoJSON for the QRY payload | `orjson-ref` §27.1, `protobuf-ref` §39.2, `tonic-ref` §33.2 |
| `prost-reflect` dynamic dispatch in production handlers | `tonic-ref` §32.2, §32.4 — generated types are safer and statically checkable; dynamic type dispatch becomes a capability surface |
| FastMCP background tasks (`tasks=False` stays) | The daemon already owns the accepted-handle lifecycle. Adding Docket creates a second lifecycle authority (`PRIN` P3, P34) |
| `ToolSearch` / Code Mode discovery transforms | `fastmcp-ref` §16.0 — four tools do not need catalog compression |
| Raising the 4 MiB message ceiling | `tonic-ref` §20.6 — never the first fix; chunk, externalize, range-read, or fix the schema |
| A second Cargo root, or Arrow processing in Python | `AGENTS.md` §3, `SRV` §2 |

### 5.12 Contract clauses and their oracles

`PRIN` P25: every clause names the executable that decides it, and every oracle names the change
that would falsify it. Rows marked *(new)* require a recipe that does not exist today (DR-024).

| Clause | Executable oracle | Falsifying change |
|---|---|---|
| Wire schema is the sole contract authority; Rust and Python are derived | `just proto-contract-check` *(new)* | renumber a field in one generator's output only |
| Every closed vocabulary is one enum, projected three ways | `just query-form-contract-check` (extend) | add a value to one projection only |
| No model/phrase/capability/epoch state in the wheel | `just adapter-package-authority-zero-state-check` *(new)* | re-add one registry entry to package data |
| Reference and status output derives from the admitted epoch | `just dynamic-reference-delivery-check` *(new)* | mutate a capability in the epoch; output must change |
| Published tool input schema describes the accepted grammar | `just semantic-delivery-vertical-check` *(new)* | publish a schema the daemon rejects |
| Inline and resource delivery are semantically identical | `just gate-b-delivery-equivalence-check` | truncate the resource route |
| One terminal event, monotonic sequence, resumable from any cursor | `just adapter-test` (extend with an Attach path) | drop an event; resume must fail |
| Backpressure reaches the execution engine | slow-consumer test, `tonic-ref` §19.5 *(new)* | swap the bounded channel for an unbounded one |
| Admission limits are enforced, not advertised | `just resource-governance-check` *(new)* | change the limit; queued count must change |
| Every call reauthorizes a bound credential | `just access-catalog-isolation-check` *(new)* | remove the interceptor; denied cases start passing |
| Deadline ladder is strictly nested with cleanup reserve | adapter deadline test *(new)* | set reserves to zero |
| Public output leaks no path, source, token, or provider text | `just public-leakage-negative-check` *(new)* | inject a raw path into an error record |
| STDOUT carries MCP protocol only | `just adapter-stdio-test` | write to stdout from a tool |
| Every clause above has a non-vacuous oracle | `just oracle-substance-check` | replace any oracle with an existence assertion |

### 5.13 Staticness classification of every declared artifact at this boundary

`PRIN` Y2's mandatory question, answered per artifact. Class 1 is legitimately frozen; Class 2
carries a re-derivation oracle or is deleted; Class 3 is a defect.

| Artifact | Class | Justification / action |
|---|---|---|
| `contracts/rpc/cpg_query_service.proto` | **1** | A released wire contract; its stability across time *is* its semantics (`PRIN` P22) |
| `tooling/proto/production-descriptor.pb` | **2** | Derived from the `.proto` by a pinned foreign toolchain the checking environment cannot universally reproduce — the legitimate materialization case. Oracle: `proto-contract-check` |
| `tooling/proto/toolchain-identity.json` | **1** | A record of a completed generation, with pinned compiler identity |
| `src/generated/*.rs`, `daemon/generated/*_pb2*` | **2** | Generated caches. Oracle: regeneration comparison |
| `contracts/wire_models.py` | **2** | Projection of the Contract IR. Oracle: model-family regeneration |
| `contracts/adapter-schemas.json` + `adapter-fingerprints.json` | **2** | **Delete** — derive on read from `MODEL_ADAPTERS` (`PRIN` §A.3 rule 1) |
| `contracts/model_registries.py` | **3** | **Delete** — a census of what currently exists (DR-003) |
| `contracts/model_artifact_index.json` | **3** | **Delete from the wheel** — repository build governance, not runtime presentation |
| `RPC_VERSION` / `SEMANTIC_QUERY_VERSION` / `contract_version` literals | **3** | **Delete** — replace with negotiated values from the handshake (DR-004) |
| `settings.inline_result_bytes` as the delivery threshold | **3** | **Delete** — the daemon's enforced limit is the authority (DR-008) |
| `_reference_content` literals and the schema `$ref` stub | **3** | **Delete** — replaced by the T2 projection (DR-010) |
| `EffectiveLimitsProfile` constants | **3 → 1-by-derivation** | Populate from the enforcer's state, so the value cannot disagree with behavior (DR-017) |
| Advisory `deny.toml` / dependency pins for this boundary | **1** | Pinned revisions; immutable by construction |

---

## 6. Migration sequence

Ordered so each step is independently shippable and each has a gate before the next depends on it.
Steps 1–4 are prerequisites for the rest; 5–9 are largely parallel.

| # | Step | Primary files | Gate |
|---|---|---|---|
| 1 | Register the missing acceptance recipes as failing stubs, so the specification's oracle table stops being vacuous | `justfile`, `tooling/ci/` | `just oracle-substance-check` |
| 2 | Add the runtime descriptor self-check and `proto-contract-check` | `src/query_service.rs`, `tooling/proto/` | `just proto-contract-check` |
| 3 | Generate the vocabulary enums; add enum-typed fields; `reserved` the string fields | `contracts/rpc/`, model driver | `just query-form-contract-check` |
| 4 | Switch Rust, Python and Pydantic to the enum projections; delete `expected_states` and the hand-written `Literal` lists | `query_service.rs`, `client.py`, `wire_models.py` | `just adapter-test`, `just root-test` |
| 5 | Add the `GetStatus` contract projections; delete the packaged census and the schema stubs | `contracts/rpc/`, `query_service.rs`, `server.py`, package data | `just adapter-package-authority-zero-state-check`, `just dynamic-reference-delivery-check` |
| 6 | Publish the live request schema as the tool input schema; add resource templates and prompts | `server.py` | `just semantic-delivery-vertical-check` |
| 7 | Bounded event channel; emit `ProgressEvent` and `ResponseChunkEvent`; incremental client consumption; `ctx.report_progress`; wire `AttachQuery` | `query_service.rs`, `client.py` | slow-consumer test, resume test |
| 8 | Credential interceptor; drop identity fields from request bodies; per-call reauthorization | `rpc.rs`, `query_service.rs`, `client.py` | `just access-catalog-isolation-check` |
| 9 | Admission semaphore, quotas, TTL sweeper; collapse to one result route; remove Zstd; deadline ladder; fix `requires-python` | `query_service.rs`, `settings.py`, `pyproject.toml` | `just resource-governance-check`, `just adapter-ci-fast` |

Two sequencing constraints worth stating: step 3 must land before step 4 in the *same* release
train, because the enum and string fields coexist only during the transition; and step 8 changes
message shapes that step 7's client rework touches, so doing 7 first avoids a second edit of the
same call sites.

---

## 7. Residual risks and open questions

**R-1 — Wire compatibility during the vocabulary migration.** Adding enum-typed fields beside the
string fields is additive and safe (`protobuf-ref` §26). Removing the string fields is not, and
must wait for one full release of both artifacts. If any consumer outside this repository exists,
this becomes a two-release migration; the review assumes none does, and that assumption should be
confirmed rather than inherited.

**R-2 — Backpressure changes the freshness/deadline interaction.** Once the producer blocks on a
slow consumer, a query can exceed its deadline for a reason that is not the daemon's fault. The
terminal event must distinguish "execution exceeded the deadline" from "delivery exceeded the
deadline" — a vocabulary addition that belongs in T1, not a later patch.

**R-3 — Publishing the live request schema makes the tool schema epoch-dependent.** Two adapters
against different epochs will advertise different schemas. That is correct, but it means tool
fingerprinting (`fastmcp-ref` §30.5–§30.9) must fingerprint the *envelope* — tool names,
annotations, output schemas — and treat the input schema as epoch-bound. Otherwise a legitimate
epoch change reads as contract drift.

**R-4 — The adapter still declares suite 1.3.** Nothing in this review resolves whether the 1.3
public contract should be renumbered to 2.0 or preserved as a compatibility surface. `SRV` §0
says v2.0 preserves v1.3's method names, tool catalog and error names and negotiates compatibility
before acceptance — which reads as "preserve, and negotiate", but the version literals in the code
say 1.3 unconditionally rather than as a negotiated floor. This is a decision for the contract
owner, not an inference this review should make.

**R-5 — `SemanticQueryBackend` is generic over `B`, and this review did not exercise a second
implementation.** The admission, quota and backpressure changes in T5 and T4 assume the backend
can be driven incrementally. If the current backend materializes results eagerly regardless, T4
step 3 becomes a `FAB`-side change rather than a serving-side one.
