---
artifact: interface-design-review
date: 2026-09-01
version: v1
status: complete
interface_path: docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.2.md
serving_specification: docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.2.md
principles_path: docs/library_ref/full_data_fabric_design_principles_v2.md
fastmcp_reference: docs/library_ref/fastmcp_python_advanced_reference_4.0.0.md
plan_path: docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v4_2026-09-01.md
composes_with: docs/reviews/interface_design_review_daemon_grpc_fastmcp_boundary_2026-09-01_v5.md
amends: docs/reviews/interface_design_review_daemon_grpc_fastmcp_boundary_2026-09-01_v4.md
reviewed_head: 6e74cfbbe23da73dd110a2adb232276e00f9a3ad
working_tree: dirty-pre-existing-and-in-progress
working_tree_digest: 7c6bf2b2adaa2031217aa4bec07835fc7abc43476d8d540d3d4f036a423c21c3
baseline: intentionally-not-taken
verdict: revision-required
target_status: accepted
---

# FastMCP 4 presentation boundary: clean-sheet target design

## 0. Decision

Adopt FastMCP 4 as a **modern-only, sessionless presentation cell** at the edge of the existing
Rust daemon architecture. Use its native request-state guards for daemon-authored structured input
requirements, native completion for a small authorized reference-selector surface, request-local
context, strict schema generation, cancellation observation, and OpenTelemetry spans. Do not adopt
FastMCP sessions, background tasks, authorization policy, response caching, gateways, proxying,
Code Mode, or a custom server extension in the local product profile.

The target topology is:

```text
modern MCP host (protocol 2026-07-28)
  <-> direct STDIO
  <-> attach-only Rust launcher
  <-> one per-agent FastMCP 4 presentation cell
      <-> one lifespan-owned authenticated grpc.aio UDS channel/session
      <-> one shared Rust workspace daemon
          -> semantic/query/authorization/scheduler/resource/Delta/DataFusion authority
```

The architectural rule is:

> FastMCP owns MCP protocol and presentation mechanics. The Rust daemon exclusively owns semantic
> truth, validation, authorization, query acceptance, execution lifecycle, durable state, resource
> leases, provenance, recovery, and every decision that can change a query's meaning.

This is not a compatibility migration. CodeFabric is still in the design phase, there is no
deployed predecessor to preserve, and old FastMCP 3 behavior is not a product requirement. The
adapter must reject pre-`2026-07-28` MCP protocol eras rather than carry an alternate functional
path. Historical specifications and fixtures remain provenance, not live operability.

FastMCP 4 does not displace the supervisor, launch grant, private UDS, generated Protobuf service,
or Rust execution path accepted by the v4 and v5 interface reviews. It materially revises the
FastMCP part of v4 and requires a successor serving specification and implementation plan before
the remaining serving work resumes.

## 1. Why the design must change

FastMCP 4 is not merely a new API surface. Its modern protocol model makes each interaction leg a
new request, introduces sealed request-state continuation for input requirements, moves background
tasks into an extension, adds extension negotiation and completion handlers, expands middleware
coverage, and makes session state an explicit opt-in application choice. Those changes let the
CodeFabric edge become smaller and more truthful if adopted selectively.

A version-only upgrade would retain several design defects already visible in the working tree:

| Current mechanism | Evidence | Why it is not the target |
|---|---|---|
| FastMCP 3.4.7 and MCP SDK 1.29.0 | `codefabric-cpg-mcp/pyproject.toml` | It cannot exercise the modern request model or FastMCP 4's current protocol types. |
| Import-time global `FastMCP` construction | `codefabric-cpg-mcp/src/codefabric_cpg_mcp/server.py` | It obscures the composition root and makes launch-derived dependencies harder to construct before publication. |
| Duplicate top-level freshness | `server.py`, `contracts/wire_models.py`, `daemon/client.py` | Freshness already belongs to the canonical semantic request; a second RPC/presentation value can disagree. |
| `ValidateQuery` before every `StartQuery` | `daemon/client.py` | Rust validates again during start, creating a race and two acceptance paths. |
| Python `_resource_leases` map and generated public handles | `daemon/client.py` | Python becomes a mutable resource-capability authority and restart hazard. |
| No production `CancelQuery` call | adapter source | Cancelling an MCP request can leave accepted daemon work running without an explicit policy decision. |
| Random `mcp_call_id` and `rpc_attempt_id` | `daemon/client.py` | These duplicate native request correlation and leak transport attempts into the public contract. |
| Camel-case Python SDK fields | `ToolAnnotations(readOnlyHint=...)` | FastMCP 4's Python API is snake case; the compatibility bridge would hide stale code. |
| Pydantic runtime schemas plus static adapter schemas | `contracts/wire_models.py`, `contracts/adapter/*.schema.json` | Two schema authorities can drift and already describe different surfaces. |
| Two specified prompts but zero registered prompts | `SRV v2.2`, `server.py` | A declaration without production consumption violates causal-declaration doctrine. |
| Gates naming deleted tests and unused packages | `justfile`, `SRV v2.2` | Passing or failing those declarations would not prove the target behavior. |

The focused current adapter suite being green is evidence about the current FastMCP 3 adapter, not
evidence that its ownership model or public contract should survive.

## 2. Design drivers and non-negotiable invariants

The target must satisfy all of the following together:

1. One shared Rust daemon per workspace remains the only semantic and durable authority.
2. One directly connected STDIO presentation process remains isolated per attached agent.
3. Every MCP operation is reauthorized by the daemon through the authenticated gRPC session.
4. Python never parses Arrow, joins relations, filters semantic results, chooses epochs, or
   interprets provider facts.
5. A query is accepted at most once through one atomic daemon operation.
6. Missing structured input is explicit; absence never silently changes query meaning.
7. Interaction continuation survives only as a bounded, sealed, non-authoritative challenge.
8. Accepted query work is daemon-owned and is never silently restarted by an adapter retry.
9. Result resources are daemon-minted, bounded, immutable, reauthorized, and stream with natural
   gRPC/STDIO backpressure.
10. The production MCP catalog contains only proved, intentionally consumed capabilities.
11. STDOUT remains MCP protocol bytes only; diagnostics and telemetry cannot contaminate it.
12. Every positive capability and every rejected authority has an executable oracle.

## 3. Alternatives considered

### 3.1 Alternative A — dependency-only FastMCP 4 migration

Keep the current four tools, client flow, Python lease table, duplicate freshness, and global server
shape, changing only imports and pins.

This minimizes immediate code churn but preserves the wrong ownership and misses the one FastMCP 4
capability that materially improves the functional contract: a standard structured continuation
when a daemon can prove that a request is incomplete. It is rejected.

### 3.2 Alternative B — adopt FastMCP 4 sessions, tasks, auth, and cache wholesale

Use `UserSession` or `SessionProvider` for agent/query state, `TasksExtension` and Docket for long
queries, FastMCP roles/scopes for authorization, and response middleware for result reuse.

This appears library-rich but creates second owners for identities, task acceptance, cancellation,
progress, authorization, retention, and cache validity. The daemon would become a backend behind a
competing Python control plane. It violates the data-fabric ownership, state, mutation-path, and
inward-dependency principles. It is rejected.

### 3.3 Alternative C — isolated modern FastMCP 4 presentation cell

Keep one per-agent STDIO edge, use only FastMCP's protocol-native presentation capabilities, and
project every semantic or durable decision from the daemon. Make modern guarded input a first-class
daemon outcome, remove duplicated Python resource state, and make cancellation explicit.

This provides the strongest functionality without creating a second truth. It is the accepted
local-workstation target.

### 3.4 Alternative D — shared stateless FastMCP HTTP edge

A shared HTTP edge could amortize Python startup and memory at high agent density. It would also
replace structural inherited-fd and peer-credential identity with OAuth, TLS, Origin, reverse-proxy,
tenant-isolation, and shared-state responsibilities. STDIO hosts would still need a bridge.

This is a credible future remote-deployment profile, not the local target. It requires a separate
design and measured density need; it must not be combined with the local STDIO server.

### 3.5 Alternative E — Rust-native MCP edge

Removing Python could reduce startup and resident memory, but it would reimplement protocol-era
negotiation, guarded input, completion, schema publication, host quirks, and testing facilities
without evidence that the presentation process is the bottleneck. It is rejected unless measured
performance and a separately researched, pinned Rust MCP implementation overturn that conclusion.

## 4. Target ownership model

### 4.1 Rust daemon authority

The daemon is the sole owner of:

- canonical semantic requests, normalization, freshness, validation, and capability checks;
- query admission, idempotency, queueing, execution budgets, progress, cancellation, and terminal
  state;
- workspace, principal, policy, session, daemon-generation, and revocation enforcement;
- DataFusion plans and execution, Arrow schemas/pages, Delta snapshots, epochs, proof, and
  provenance;
- accepted query IDs, result package IDs, public resource handles, resource leases, expiry,
  release, and tombstones;
- clarification requirements, allowed answer shapes, continuation round limits, and challenge
  binding; and
- restart, recovery, retention, and cleanup decisions.

### 4.2 FastMCP presentation-cell authority

The Python process owns only:

- strict parsing and presentation validation of the MCP-facing DTOs;
- construction of one FastMCP application after launch settings are verified;
- one lifespan-owned `grpc.aio` channel and current authenticated daemon session;
- deterministic mapping between public Pydantic DTOs and generated Protobuf messages;
- one MCP request's local progress, cancellation, error redaction, and correlation context;
- mapping a typed daemon input requirement to FastMCP's input-request representation; and
- bounded conversion of daemon bytes into one MCP response or resource chunk without semantic
  interpretation.

Permitted mutable Python state is limited to the channel/session and in-flight request cleanup
records. It has process lifetime only and is reconstructible. There is no Python CPG, catalog,
workflow, task, result, resource-lease, authorization, cache, or session registry.

### 4.3 FastMCP library authority

FastMCP owns:

- MCP framing and direct STDIO service;
- modern protocol negotiation mechanics;
- tool/resource/completion dispatch and schema publication;
- request-local `Context`, guarded continuation plumbing, progress transport, and cancellation
  observation;
- extension negotiation mechanics even though the base profile registers no extension; and
- server request spans and framework-level inspection.

FastMCP metadata, annotations, visibility, cache hints, and fingerprints are observations or client
advice. They never authorize an operation or prove semantic correctness.

## 5. Public MCP contract

### 5.1 One modern product profile

The only supported MCP protocol era is `2026-07-28`. FastMCP may contain older negotiation code, but
CodeFabric does not expose an older functional profile. A small `ModernProtocolPolicyMiddleware`
rejects an older negotiated era at the earliest stable initialization hook and rechecks it before
operation dispatch. The error is typed and public-safe.

There is no legacy equivalence suite, `ctx.elicit()` fallback, initialize-dependent domain state,
or alternate result shape. The sole legacy oracle proves rejection. If FastMCP cannot reliably
enforce the minimum era before business dispatch, that is a replan trigger, not permission to add
compatibility branches.

### 5.2 Four product tools

The stable product catalog remains exactly:

1. `query_code_graph` — validate and, when complete, atomically accept one query;
2. `validate_code_graph_query` — pure dry-run validation and preparation information;
3. `get_code_graph_status` — cheap live daemon status projection; and
4. `get_code_graph_reference` — authorized live reference projection.

The catalog is deliberately small. Relations, entity kinds, query forms, and providers do not
become dynamically generated tools. Tool Search, Code Mode, apps, proxy providers, and transforms
are not part of this server.

`query_code_graph` retains only presentation-level delivery preference if the product still needs
it: `automatic`, `inline`, or `resource`. Delivery does not enter semantic identity. Freshness is
removed from the tool wrapper and exists exactly once inside the canonical semantic request.

Tool annotations use canonical FastMCP 4 snake-case Python fields and describe behavior only. Rust
authorization remains decisive.

### 5.3 Resources

Keep two bounded resource families:

- immutable result manifest/page resources; and
- authorized reference resources.

The daemon mints a high-entropy **public resource handle** after it creates the authoritative
resource/lease record. The handle is not a secret daemon lease token and is insufficient without a
currently authenticated, authorized gRPC session. It binds resource/package identity, workspace,
principal or sharing class, policy/revocation generation, daemon generation, and expiry. Every read
and release is checked by the daemon.

The adapter embeds only the public handle and bounded selector/page index in an MCP URI and forwards
it back to the daemon. It keeps no handle-to-secret map. A daemon restart invalidates old-generation
handles; an authorized client may obtain a new handle from the retained result package if recovery
policy permits.

The adapter materializes at most one bounded, independently decodable page or projection per
resource call. It never exposes filesystem/object-store paths, secret tokens, or complete relations.

### 5.4 No prompts in the base catalog

Remove the two phantom prompt declarations from the successor serving specification. Query authoring
guidance and reference material are already available through the live reference tool/resource, and
the guarded query operation handles proven missing inputs. Adding prompts only to demonstrate a
FastMCP feature would create another stale prose surface.

A future prompt must have a distinct user outcome, derive dynamic semantic content from the daemon,
and clear its own visibility, injection, versioning, and executable-proof review.

### 5.5 Narrow completion

Register FastMCP 4 completion only for safe variables of the reference resource template, such as an
authorized reference kind or released version selector. Completion does not apply to tool arguments
and must not be simulated with another tool.

Each completion request:

1. re-enters the modern protocol and authorization middleware;
2. queries the daemon's live authorized reference projection;
3. caps output at 100 candidates or the daemon's smaller enforced maximum;
4. reports `total` and `has_more` when the protocol shape supports them; and
5. returns advisory values that the eventual resource operation validates again.

Never complete result handles, repository paths, source/entity inventories, hidden capabilities,
principal data, or any selector whose existence is itself unauthorized information.

## 6. Daemon-authored guarded input

### 6.1 Atomic query preparation and acceptance

Replace the ordinary `ValidateQuery` then `StartQuery` sequence with one atomic `StartQuery`
operation whose response is a closed oneof:

```text
StartQueryOutcome =
  Accepted(accepted_query)
  | InputRequired(input_challenge)
  | Rejected(validation_failure)
```

`StartQuery` validates, authorizes, normalizes, checks capacity, and either accepts exactly once or
returns without creating accepted work. `ValidateQuery` remains a separate pure dry-run tool and
returns the same preparation facts as data, including input requirements, without creating a
FastMCP guard.

Because the current `codefabric.cpgd.v2` contract is not deployed, update the v2 source, descriptor,
generated Rust/Python bindings, and interoperability fixtures as one forward-only transaction. Do
not create a v3 service merely to preserve an unshipped v2 response. Historical descriptor evidence
is retained outside the live package only where release governance requires it.

### 6.2 Challenge ownership

An `InputRequired` response is authored by the daemon from typed semantic validation. It contains:

- a bounded set of stable semantic field identifiers;
- an allowlisted input kind and strict constraints for each value;
- safe labels/descriptions or stable presentation keys;
- optional daemon-authorized enum choices;
- a daemon-issued opaque continuation token;
- expiry and maximum remaining rounds; and
- a public-safe explanation code.

The adapter may translate labels and structural input types, but it does not choose which question to
ask, merge semantic defaults, infer an answer, or decide whether the completed request is valid.

### 6.3 FastMCP request-state binding

The adapter maps the challenge to `InputRequiredResult`. FastMCP seals the outgoing `request_state`
and unseals it on the next request. The plaintext value exposed to the tool is still an opaque
daemon token. It must bind at least:

- agent/principal and workspace;
- original semantic request identity and normalized immutable fields;
- daemon session class and generation;
- challenge and round number;
- issue/expiry and policy/revocation generation; and
- the exact allowed response shape.

FastMCP's seal protects transport continuation integrity and binds method/arguments. It is not a
substitute for daemon authority, particularly because local STDIO does not provide a FastMCP
authenticated principal. The daemon always verifies its own token and reauthorizes the next leg.

Use a process-ephemeral FastMCP request-state key with a bounded TTL no longer than the daemon
challenge and launch/session limit. Adapter restart intentionally invalidates an unfinished guard;
it never loses accepted work because acceptance has not occurred yet.

### 6.4 Re-entry and bounded behavior

Every interaction leg is a new request and traverses protocol admission, redaction, tracing,
deadline, authorization, and daemon validation again. The tool reads `ctx.input_responses` and the
unsealed `ctx.request_state`, passes both to the daemon, and returns the daemon's next closed outcome.

The daemon enforces a configured maximum round count and total challenge size. The adapter does not
hard-code a semantic round limit. Tampered, expired, replayed, cross-request, cross-agent,
cross-workspace, wrong-generation, or wrong-argument state is rejected. Abandoning a guard creates no
task, lease, query, or cleanup burden beyond expiring the small daemon challenge record if one is
retained.

Only structured input elicitation is permitted. The adapter does not invoke model sampling, list
roots, or ask an LLM to resolve semantic ambiguity.

## 7. Server construction, injection, and middleware

### 7.1 Application factory

Replace import-time publication with a pure application factory called after fd3 launch settings
are parsed and validated:

```text
verified launch settings
  -> construct DaemonPort/channel factory
  -> construct FastMCP 4 server with explicit policy
  -> register four tools, two resource families, one completion handler, middleware
  -> run direct STDIO
```

The invariant is one published server surface, not exactly one textual `FastMCP(...)` call. No
provider, transform, mounted server, or generated component is added unless it realizes an accepted
public capability.

### 7.2 Dependency injection

Inject a hidden, typed `DaemonPort` backed by the lifespan-owned `CpgDaemonClient`. Use
`CurrentContext` only for request-local MCP identity, progress, cancellation, and guard inputs.
Public tool arguments remain explicit Pydantic models. Do not use `CallArgument` or arbitrary
context state unless a concrete hidden dependency needs a public argument and cannot receive it
directly.

Tool and resource functions depend inward on the narrow application-owned port, not generated stub
types throughout the presentation layer. Generated Protobuf types terminate inside the daemon
adapter module.

### 7.3 Minimal middleware stack

Use a small, ordered, operation-aware middleware stack:

1. modern protocol admission;
2. request/correlation and built-in trace enrichment;
3. bounded deadline and cancellation projection;
4. public-safe error mapping and redaction.

Generic middleware now sees more inbound traffic in FastMCP 4. Prefer operation-specific hooks so
initialize, cancellation, completion, and notification traffic cannot accidentally enter tool
counters or policy logic. There is no generic retry, response cache, payload logger, or policy
engine in Python.

### 7.4 Correlation and telemetry

Use the FastMCP request ID as the MCP-leg correlation ID. Remove generated Python `mcp_call_id` and
`rpc_attempt_id` from public results and semantic identity. Keep these identities distinct:

- MCP request/leg ID — presentation observation;
- daemon query ID — accepted-work authority;
- semantic request ID — canonical request identity;
- epoch/package/resource IDs — daemon data-fabric identities; and
- challenge ID — bounded pre-acceptance continuation.

Attach only allowlisted protocol era, operation, request, challenge, query, and daemon-generation
attributes to telemetry. Never record canonical request payloads, input answers, source content,
session tokens, request state, resource handles, or gRPC metadata. Exporters use configured sinks or
STDERR; STDOUT remains protocol-only.

## 8. Cancellation, deadlines, reconnect, and retry

FastMCP cancellation is observation of host intent. The adapter translates cancellation only after
the daemon has returned an accepted query ID:

1. catch the actual request cancellation signal;
2. invoke daemon `CancelQuery` under a separate bounded cleanup budget;
3. await cancellation acknowledgement or observed terminal state as policy requires;
4. record a redacted cleanup outcome; and
5. re-raise cancellation to FastMCP.

Before acceptance, cancellation simply ends the attempt and leaves no query. Dropping `WatchQuery`
cancels observation only; it must not implicitly cancel accepted work. The explicit cancellation
bridge performs that policy decision.

Transport loss never resubmits `StartQuery`. A replacement channel performs a new handshake and
resumes `WatchQuery` by daemon query identity/cursor if the accepted work is still authorized.
Retries are limited to operations the daemon contract declares idempotent and must use the original
daemon idempotency identity. FastMCP tasks are not used as a retry or recovery wrapper.

MCP host deadlines, daemon queue/admission budgets, execution budgets, cancellation cleanup budgets,
and resource-stream timeouts remain typed and distinct. Python does not synthesize a wall-clock
deadline into semantic freshness.

## 9. FastMCP 4 capability disposition

| FastMCP 4 capability | Decision | Boundary condition |
|---|---|---|
| Modern `2026-07-28` protocol | Adopt | Sole product era; older eras are rejected. |
| Sessionless requests | Adopt | Every leg re-enters policy; durable identity remains in the daemon. |
| `InputRequiredResult` | Wrap narrowly | Daemon-authored, pre-acceptance, structured input only. |
| `RequestStateSecurity` | Adopt | Ephemeral process seal plus independent daemon token and shorter effective TTL. |
| `Context` / hidden dependencies | Adopt | Request-local presentation and one lifespan daemon port only. |
| Completion handler | Wrap narrowly | Authorized reference selectors only, capped and revalidated. |
| Built-in OpenTelemetry spans | Adopt | Redacted correlation attributes; no payloads or STDOUT exporter. |
| `UserSession` | Reject | Requires FastMCP auth and duplicates daemon identity/state. |
| `SessionId` / `SessionProvider` | Reject | Adds bearer handles and another workflow store. |
| `TasksExtension` / `fastmcp-tasks` | Reject | Duplicates daemon acceptance, progress, cancellation, retry, and result state. |
| `ServerExtension` | Reject in base profile | Adds a second negotiated contract with no distinct functional outcome. |
| FastMCP auth, roles, scopes | Reject in local profile | fd3 launch authority and daemon reauthorization remain decisive. |
| SEP-990 identity assertion | Reject | Beta delegation is not a local tenant boundary. |
| `ClientGroup` | Reject in production adapter | CodeFabric is not an MCP gateway or upstream aggregator. |
| Response caching middleware | Reject | Keys and invalidation cannot safely encode daemon ACL/lease/live-state rules. |
| SEP cache hints | Reject in base profile | Mixed live and sensitive resources make uniform hints misleading. |
| Proxy/gateway/remote provider | Reject | There is one private daemon authority, not multiple MCP upstreams. |
| Tool Search / Code Mode / Apps | Reject | Four bounded product tools are intentional; no dynamic execution surface is needed. |
| Providers and transforms | Reject in base profile | They add composition machinery without an owned current outcome. |
| Prompts | Reject in base profile | Live references and guarded input cover the intended outcomes. |
| `ctx.elicit()` legacy path | Reject | No pre-modern functional profile. |
| Sampling and root listing | Reject | No daemon-approved CodeFabric outcome requires them. |
| Client cache | Reject in acceptance clients | Tests must observe current server behavior and authorization. |

Set `tasks=False` explicitly as fail-closed application policy and also prove that no
`TasksExtension`, task tool, Docket worker, or `fastmcp-tasks` dependency is present. The executable
absence oracle, not the keyword alone, establishes the result.

## 10. Package and API target

The adapter domain target is:

```text
fastmcp == 4.0.0
mcp     == 2.1.1
pydantic == 2.13.4
```

The exact isolated resolution was import-probed on Python 3.14.7. `mcp` remains a direct dependency
because CodeFabric imports protocol types. Do not add direct `fastmcp-slim`, `fastmcp-tasks`,
`Docket`, Redis, or `httpx2` dependencies unless production code directly owns a separately accepted
use. Remove the `pydantic-settings` requirement if fd3 settings continue to use strict Pydantic
models and no direct settings integration exists.

Use one canonical protocol-type import style consistently. `mcp_types` is the permanent model
package and `mcp.types` is a supported re-export; the implementation transaction must choose one and
govern it rather than mix both.

All Python SDK attributes use snake case. Run every adapter gate with
`FASTMCP_MCP_CAMELCASE_COMPAT=false`; wire JSON remains protocol-defined camel case. Any code that
passes only with the bridge is stale.

Strict/frozen/extra-forbid Pydantic models remain the MCP presentation schema authority. Remove or
archive `contracts/adapter/*.schema.json` as predecessor artifacts rather than regenerate a second
coequal registry. `fastmcp inspect --format mcp` and live JSON Schema are computed observations. A
fingerprint identifies a surface; it does not prove that the surface behaves correctly.

## 11. Security model

The local authorization chain remains:

```text
operator launch policy
  -> supervisor-issued single-use fd3 grant
  -> peer-credential-bound daemon handshake
  -> expiring daemon-generation gRPC session
  -> per-operation and per-object daemon reauthorization
```

FastMCP authentication is HTTP-scoped and does not replace that chain on STDIO. Tool annotations,
visibility, tags, completion omission, resource URI opacity, and request-state sealing are defense in
depth or presentation behavior, never authorization.

Required negative cases include:

- wrong UID, principal, workspace, operation, semantic profile, or daemon generation;
- revoked or expired launch/session/challenge/resource authority;
- replayed launch grant, request state, challenge answer, query start, or release;
- cross-agent and cross-workspace challenge/resource handle use;
- altered tool arguments paired with a valid-looking request-state seal;
- completion enumeration of denied references;
- malformed/oversized input requirements and resource ranges; and
- accidental secret or payload emission to errors, spans, logs, or STDOUT.

Public error results contain stable allowlisted codes and safe correlation IDs. Python never branches
on gRPC prose and never relays daemon internals, stack traces, paths, source bytes, tokens, or
unallowlisted metadata.

## 12. Performance and robustness posture

The design keeps expensive work in Rust and uses async Python only for I/O. One channel per adapter
is reused for its process lifetime; one daemon serves all agent adapters. No Python threadpool work,
Arrow decode, relation assembly, middleware cache, proxy fan-out, or background worker is introduced.

Guard continuation avoids accepting and cancelling a query that could not yet be specified.
Completion is capped and daemon-indexed. Result pages remain bounded and streamed. FastMCP catalog
cost stays fixed at four tools and two resource families.

Do not freeze folklore budgets. The implementation plan must establish measured budgets for:

- cold installed-adapter startup to protocol readiness;
- steady resident memory per idle and active adapter;
- no-op/status and query-acceptance presentation overhead;
- one and maximum-round guard overhead;
- completion latency and result cardinality;
- bounded resource first-byte and sustained throughput;
- cancellation acknowledgement and cleanup;
- daemon restart/reconnect/resume; and
- N-agent concurrency, fairness, and aggregate Python memory.

If per-agent FastMCP 4 startup or memory dominates the measured deployment budget, reopen the shared
HTTP edge alternative as a separate profile. Do not preemptively weaken isolation.

## 13. Data-fabric principle alignment

| Principle | Target alignment |
|---|---|
| P1 — model-driven behavior | Canonical semantic models remain daemon inputs that causally drive validation and execution. |
| P2 — compile models into behavior | Rust compiles semantic requests and installed relations; Python does not re-encode the model. |
| P3 — one owner per concept | Sessions, queries, tasks, resources, policy, and caches have one daemon owner; FastMCP owns presentation only. |
| P4 — responsibility levels | Host, launcher, presentation cell, gRPC port, daemon, and data fabric have explicit non-overlapping roles. |
| P5 — boundary adapters | FastMCP and generated gRPC are narrow edge adapters around application-owned ports. |
| P6 — transport-independent meaning | Protocol era, delivery, and transport correlation cannot change semantic request meaning or epoch selection. |
| P7 — shared representations | Canonical JSON, Protobuf control messages, Arrow IPC, and Delta remain the deliberate boundary representations. |
| P8 — compute where data lives | Python performs no joins, filters, casts, planning, or relation materialization. |
| P9 — preserve provenance | Semantic provenance comes from the daemon; presentation adds only protocol/correlation observations. |
| P10 — closure | Query, epoch, package, resource, and challenge identities resolve through daemon-owned closure. |
| P11 — distinguish state classes | Request state, channel state, challenge state, query state, and durable truth have separate owners and lifetimes. |
| P12 — orthogonal proof | Schema, modern protocol, guard, authorization, cancellation, restart, vertical, and performance oracles are independent. |
| P13 — enforce at authority | The daemon reauthorizes every operation, completion, challenge continuation, resource read, and release. |
| P14 — leverage fit-for-purpose libraries | Native FastMCP guards/completion/context are used; ill-fitting Docket/session/auth abstractions are rejected. |
| P15 — expose structure | Closed daemon outcomes and strict DTOs preserve validation and lifecycle structure rather than hiding it in wrappers. |
| P16 — stage boundaries | Ingress, preparation, guarded input, acceptance, execution, publication, and resource delivery remain distinct phases. |
| P17 — reconstructibility | Adapter state is disposable; accepted work and durable artifacts remain daemon-owned and recoverable by policy. |
| P18 — identity is not correctness | Descriptors, schemas, manifests, seals, and fingerprints prove only their named identity/integrity property. |
| P19 — prove by re-execution | Modern interaction, restart, cancellation, and resource behavior are rerun against the installed vertical. |
| P20 — advertise proved capabilities | Only four tools, two resource families, and proved reference completion are published; no phantom prompt/task/extension. |
| P21 — metadata is not enforcement | Annotations, completion visibility, and cache metadata never substitute for daemon policy. |
| P22 — standards at real boundaries | MCP, Protobuf, canonical JSON, Arrow IPC, and Delta are used at their intended boundary and not duplicated. |
| P23 — local explicit state | Every mutable object has an owner, scope, lifetime, cleanup rule, and authority relation; Python lease state is removed. |
| P24 — observability without authority | Traces carry redacted correlations and protocol era but cannot determine semantic or lifecycle state. |
| P25 — clauses name oracles | Each target capability and forbidden duplicate authority has an executable acceptance check below. |
| P26 — declare only static truth | Exact pins and public contract names are declared; live catalogs, status, candidates, and manifests are derived. |
| P27 — causal declarations | Every registered component is production-consumed; no dormant prompt, provider, transform, or extension registry remains. |
| P28 — compute change | Live inspection/schema observations are recomputed; no hand-maintained compatibility fingerprint decides drift. |
| P29 — relational validation | Semantic/capability validation remains typed and relational in Rust; Python validates only its presentation DTO. |
| P30 — independent expectations | Expected catalogs, denied cases, and challenge/security fixtures are authored independently and fault-injected. |
| P31 — remove forgetting hazards | Duplicate freshness, schema registries, validation paths, IDs, and resource lease maps are eliminated. |
| P32 — correctness by construction | One launch grant constructs one principal/workspace presentation cell; invalid cross-agent local state is unrepresentable. |
| P33 — functional core, imperative shell | Deterministic DTO mapping surrounds a minimal async FastMCP/gRPC I/O shell. |
| P34 — one mutation path | Query acceptance/cancel/release use one daemon command path; no FastMCP task/session route bypasses it. |
| P35 — inward acyclic dependencies | FastMCP depends on an application port and generated transport; Rust semantic/data-fabric code never depends on FastMCP. |
| P36 — executable governance | Pins, catalog, protocol, absent authorities, state zero-state, denied cases, and vertical behavior are machine-checked. |

## 14. Legacy disposition

| Existing surface | Disposition |
|---|---|
| FastMCP 3.4.7, MCP 1.29.0, old lock entries | Replace with the exact FastMCP 4 target transaction. |
| Camel-case Python model fields and compatibility bridge | Replace with snake case; disable the bridge in every gate. |
| Import-time global server as constitutional shape | Replace with a verified-settings application factory. |
| Four semantic tools | Preserve as the product catalog, with guarded `query_code_graph`. |
| Two bounded resource families | Preserve, but use daemon-minted public handles and no Python lease map. |
| Normal-path `ValidateQuery` preflight | Delete; `StartQuery` validates and accepts atomically. |
| Explicit validation tool | Preserve as pure dry-run data with no guard side effect. |
| Top-level presentation/RPC freshness | Delete; canonical semantic freshness is sole authority. |
| Python `_resource_leases` and random handles | Delete after daemon public-handle contract lands. |
| Python-generated MCP/RPC attempt IDs | Delete from public/semantic DTOs; use request-local trace correlation. |
| Missing production `CancelQuery` bridge | Implement and prove. |
| Static `contracts/adapter/*.schema.json` | Archive as historical or remove from live authority/package/gates. |
| Phantom prompt requirements | Remove from successor target. |
| `pydantic-settings` requirement without direct use | Remove. |
| Dead `test_arrow_resources.py` gate references | Replace with current target oracles. |
| FastMCP 3 constructor/decorator structural rules | Replace with outcome-level catalog/authority checks. |
| Python identity/canonicalization semantics with no presentation consumer | Decommission; retain only strict boundary DTO needs. |
| Generated gRPC v2 port, fd3/UDS topology, supervisor/launcher | Preserve and revise coherently for the new `StartQuery` outcome. |
| Historical v1/v2 specifications and released evidence | Preserve as immutable history, never runtime fallback. |

Decommission is part of the implementation transaction. A new path does not count as complete while
the replaced registry, dependency, test, gate, package payload, or fallback remains live.

## 15. Authoritative design and plan consequences

The current serving specification v2.2 still pins FastMCP 3.4.7 and encodes stale package, prompt,
query, resource, and acceptance assumptions. Do not rewrite that released design in place. Issue a
suite successor, expected to be v2.3.0, and update at least:

- `SUITE` for the new suite identity and cross-artifact contract;
- `SRV` for the FastMCP 4 modern profile, guarded input, public resources, cancellation, and
  zero-state rules;
- `QRY` for typed preparation/input requirements and atomic accepted/input-required/rejected
  outcomes;
- `RM` for dependency-closed order and decommission;
- `docs/spec_index/library-routing.md` for the FastMCP 4 reference; and
- other derived indexes only after their authoritative sources change.

Update `LIFE` or `FAB` only if the successor adds a genuinely new lifecycle or data-fabric
invariant; do not restate serving mechanics across domain specifications.

Implementation plan v4 work packets WP37, WP38, WP39, WP40, and WP42 are stale where they encode
FastMCP 3 or the superseded serving boundary. Create a successor plan version rather than silently
editing their pass conditions:

- serving implementation must include FastMCP 4, modern admission, guard mapping, completion,
  daemon public handles, and cancellation;
- acceptance must include independently authored modern/guard/security expectations;
- decommission must remove old schemas, leases, prompts, IDs, dependencies, and dead gates;
- performance must run only after purge; and
- terminal proof must use the real supervisor, daemon, launcher, installed adapter, and modern host.

Do not resume the affected v4 serving packets unchanged. Unaffected proven Rust/data-fabric work is
not undone.

## 16. Executable acceptance obligations

Each proposed oracle names the fault that must make it fail.

| Oracle | Required proof | Falsifying fault |
|---|---|---|
| `fastmcp4-dependency-contract-check` | Exact FastMCP/MCP/Pydantic/Python identities, frozen lock, import, and installed entry point. | Restore an old pin, add `fastmcp-tasks`, or import through the camel-case bridge. |
| `fastmcp4-modern-protocol-check` | `2026-07-28` initializes and operates; every older era is rejected before business behavior; no initialize/session domain state. | Run a legacy client or add a legacy result path. |
| `fastmcp4-public-surface-check` | Exactly four tools, two resource families, intended reference completion, no prompts/tasks/extensions/sessions/providers/transforms. | Register any forbidden component or remove a required one. |
| `fastmcp4-guard-roundtrip-check` | No query accepted before complete input; every leg re-enters; valid answers accept once; tamper, expiry, replay, wrong args/principal/workspace/generation and excess rounds fail. | Forge/reuse request state or accept during the first incomplete leg. |
| `fastmcp4-atomic-start-check` | Ordinary query invokes one atomic start path; explicit validate is pure; concurrent state change cannot make preflight and acceptance disagree. | Reintroduce normal-path `ValidateQuery` or mutate during validate. |
| `fastmcp4-adapter-authority-zero-state-check` | No Python semantic/session/task/cache/resource lease registries, Arrow/DataFusion/Delta processing, or canonical request rewriting. | Add `_resource_leases`, `UserSession`, task storage, or Arrow decode. |
| `fastmcp4-resource-authority-check` | Daemon-minted handle, per-read authorization, bounded streaming, expiry/revoke/release/restart behavior, no secret/path exposure. | Cross-agent handle read, stale-generation read, or Python handle map. |
| `fastmcp4-completion-authorization-check` | Only authorized capped safe candidates; unsupported/denied selectors reveal nothing; operation revalidates. | Return a denied reference, path, entity inventory, or result handle. |
| `fastmcp4-cancellation-recovery-check` | Host cancellation invokes daemon cancel once under cleanup budget; transport loss does not resubmit; reconnect resumes observation. | Merely drop the watch, resubmit start, or leave accepted work unintentionally running. |
| `fastmcp4-contract-observation-check` | Live MCP inspection/schema matches independently authored expected clauses with compatibility bridge off. | Change a tool/resource/schema while updating only the generated observation. |
| `fastmcp4-security-negative-check` | Denial matrix for grant/session/challenge/resource/revocation/generation and redaction. | Wrong principal/workspace succeeds or secrets enter logs/errors/spans. |
| `fastmcp4-stdio-vertical-check` | Real supervisor -> daemon -> launcher -> installed FastMCP 4 adapter -> modern client; decoded behavior, progress, guard, resource, cancel, stdout purity. | Replace any production process with an in-memory fake or emit diagnostic bytes to stdout. |
| `fastmcp4-performance-check` | Startup, RSS, call/guard/completion/resource overhead, cancellation, reconnect, and N-agent budgets measured separately. | Add synchronous blocking, unbounded candidate/page materialization, or per-call channel construction. |
| `fastmcp4-decommission-zero-state-check` | Old pins, camel-case use, prompts, task/session/cache authority, lease map, duplicate freshness/IDs/schemas, dead tests, and fallback paths absent from live scope. | Restore any predecessor production path or package payload. |

Framework in-memory tests remain useful for component behavior, but they do not replace the installed
STDIO vertical. `fastmcp inspect` is a compatibility observation, not a correctness oracle. Positive
fixtures and expected catalogs must be independently authored rather than generated from the server
under test.

## 17. Replan triggers

Reopen this design only if evidence establishes one of these conditions:

1. A required target MCP host cannot support `2026-07-28` or guarded input. That reopens the product
   host requirement; it does not automatically authorize a legacy compatibility path.
2. FastMCP cannot enforce the modern profile before business operation through a stable supported
   hook.
3. Guarded continuation cannot preserve strict output schemas, request cancellation, or STDIO
   behavior under the exact FastMCP 4 stack.
4. A daemon-minted public resource handle cannot eliminate Python lease state without exposing a
   secret or weakening per-read authorization.
5. Safe reference completion cannot be reauthorized without leaking denied existence.
6. Measured per-agent startup/RSS dominates the deployment budget enough to justify the separately
   secured shared HTTP profile.
7. Target hosts require standardized detached MCP task handles as a functional outcome. That would
   require an external-daemon-backed task projection design, not a Docket wrapper around daemon work.
8. A confirmed FastMCP 4 protocol or security defect requires a bounded upgrade hold. It does not
   justify a permanent parallel FastMCP 3 authority.

Normal implementation difficulty, stale predecessor tests, or a desire to preserve already-written
adapter code are not replan triggers.

## 18. Conclusion

The best FastMCP 4 design is deliberately selective. It gains a modern sessionless protocol,
daemon-authored structured continuation, safe live completion, request-local context, and mature
presentation instrumentation. It rejects every convenient-looking feature that would turn Python
into a second state, task, authorization, cache, gateway, or semantic authority.

That combination improves functionality and robustness while making the edge smaller: one modern
protocol profile, four tools, two bounded resource families, one narrow completion surface, no
prompts, no FastMCP sessions or tasks, no Python resource registry, and one atomic route into the
Rust data fabric.

## Appendix A — evidence reviewed

- `docs/library_ref/fastmcp_python_advanced_reference_4.0.0.md`, especially §§3–4, 9, 11–12,
  17–18, 29, 33, and 35–44.
- `docs/library_ref/full_data_fabric_design_principles_v2.md`, P1–P36.
- `docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.2.md`.
- `docs/reviews/interface_design_review_daemon_grpc_fastmcp_boundary_2026-09-01_v4.md`.
- `docs/reviews/interface_design_review_daemon_grpc_fastmcp_boundary_2026-09-01_v5.md`.
- `docs/plans/codefabric_execution_proved_relational_data_fabric_implementation_plan_v4_2026-09-01.md`.
- Current adapter, generated RPC, Rust query service, tests, contract schemas, package manifest,
  lockfile, and repository gates at reviewed HEAD plus the recorded dirty-tree snapshot.
- Isolated runtime probes against FastMCP 4.0.0, MCP SDK 2.1.1, Pydantic 2.13.4, and Python 3.14.7.
- Official FastMCP 4.0.0 release and upgrade guidance, checked 2026-09-01:
  <https://github.com/PrefectHQ/fastmcp/releases/tag/v4.0.0>,
  <https://pypi.org/project/fastmcp/4.0.0/>, and
  <https://gofastmcp.com/getting-started/upgrading/from-fastmcp-3>.

No implementation file was modified by this review.
