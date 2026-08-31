---
name: rust-grpc-daemon-ref
description: "Reference navigator for the Rust half of a local gRPC daemon boundary — the version-pinned deep-dive behind *how a Tonic daemon serves a Python adapter over a Unix socket*. SKILL.md maps `docs/library_ref/rust_grpc_daemon_advanced_reference_tonic_0.14.6.md` (48 chapters, §0-§47): dependency topology and Cargo posture, protoc/Buf schema governance, tonic-prost-build and prost-build codegen, generated-code anatomy, the prost and bytes runtimes, the Tonic object model, client and server fundamentals, method shapes, metadata/extensions/interceptors, status codes, deadlines and cancellation, streaming and backpressure, message sizing, Endpoint/Channel, UDS client and server with peer credentials, Tokio runtime design, tokio-stream, CancellationToken, Tower admission, HTTP/2 knobs, health/reflection/dynamic descriptor tooling, descriptor fingerprints, Rust↔Python interop, shutdown and resume, performance, testing, hardening, alternatives, and upgrade discipline. REFERENCE.md (same folder) is the mechanical layer: the chapter index with line numbers, sub-indexes for the four chapters that break the numbering, a symbol-to-subsection index over fourteen crates plus a verified-absent list, a task index, ten decision trees, and the navigation hazards. Use when Rust touches `use tonic::`/`tonic::Request`/`Status`/`Streaming`/`MetadataMap`/`Endpoint`/`Channel`/`Server::builder`/`serve_with_incoming`/`UdsConnectInfo`/`use prost::`/`prost_build::Config`/`tonic_prost_build`/`compile_fds`/`bytes::Bytes`/`UnixListener`/`UnixStream`/`UCred`/`UnixListenerStream`/`ReceiverStream`/`CancellationToken`/`ServiceBuilder`/`tonic-health`/`tonic-reflection`/`tonic-types`/`prost-reflect`/`pbjson`, when writing a `build.rs` or a `.proto` gate, or when editing `Cargo.toml` pins for those crates. The Python client, `_pb2` anatomy, and ProtoJSON → sibling `grpcio-orjson-protobuf-ref`; the MCP surface above it → `fastmcp-pydantic-ref`."
allowed-tools: Read, Grep, Glob, Bash
---

# Rust gRPC Daemon Reference Navigator

Routes the deep-dive reference for the **Rust side of a local gRPC boundary** — Tonic, Prost, Tokio,
Tower, `bytes`, the protocol extensions, and the `protoc`/Buf schema workflow that feeds them, all
aimed at one daemon serving one Python adapter over one Unix-domain socket. This SKILL.md is the
**core map**: version anchors, what the stack owns, the document's layout, seventeen reading paths
by problem context, the Rust↔Python seam, and the key invariants. The companion **`REFERENCE.md`**
(same folder) carries the chapter index with line numbers, sub-indexes for the four chapters whose
subsections are unnumbered or fenced, the **symbol → canonical location index** across fourteen
crates and two CLIs — with the nineteen names the document never writes — the task index, ten
decision trees, and the fourteen navigation rules. Reach for REFERENCE.md as soon as you have an API
name or a decision to make; cross-references back here are written `SKILL §...`.

**This is a pure library navigator.** It indexes what the reference says, nothing more. No project
doctrine, no design-spec anchoring, no policy about which capabilities are permitted here — that
belongs to whatever consumes this skill, not to the skill.

**Out of scope** (covered elsewhere): the Python client and server, `grpc.aio`, Python interceptors
and channel options, `_pb2`/`_pb2_grpc` anatomy, the `.proto` language itself, presence and wire
format, ProtoJSON, and schema evolution as language-neutral topics → sibling
**`grpcio-orjson-protobuf-ref`**. The MCP surface above the adapter → **`fastmcp-pydantic-ref`**.
Producing the payloads this boundary carries — Arrow, DataFusion, Delta →
**`datafusion-pyarrow-rust-ref`** and **`deltalake-rust-ref`**; canonical JSON bytes and digests →
**`canonicalization-lib-ref`**. And the document's own boundary, stated in its opening (§0.0, and
the thesis at the top of the file): the RPC stack is a **control, transport, lifecycle,
compatibility, and flow-control** boundary. It is **not** a second semantic implementation of the
daemon's query/data model, not a service mesh (§0.2), and not a process manager (§24.3).

---

## The stack, and what this document owns

These are not eleven topics. They are one column, and the document is organised down it:

```text
   .proto contract                  <- authority; everything below is derived (0.3)
        |
   protoc 36.x / Buf CLI            <- compile, lint, break-check, emit FileDescriptorSet
        |
   tonic-prost-build + prost-build  <- Rust mapping decisions, same wire schema
        |
   prost messages / tonic services  <- generated; never hand-edited
        |
   tonic transport (HTTP/2)         <- Request/Response/Status/Streaming/Extensions
        |
   Tower Service + Tokio runtime    <- admission, cancellation, task lifetime
        |
   UnixListener / UnixStream        <- the socket, and the kernel peer credentials on it
```

| Layer | Owns | Explicitly does **not** own |
|---|---|---|
| **Buf / protoc** | schema build, format, lint, breaking-change policy, the canonical `FileDescriptorSet` | application semantics, authorization, lifecycle compatibility (§43.5) |
| **`prost-build` / `tonic-prost-build`** | Rust representation choices — `Bytes` mapping, map type, attributes, extern paths, descriptor emission | the wire schema, which is unchanged by any of them (§7.0) |
| **`prost` / `prost-types`** | encode/decode, structural typing, well-known types | domain invariants (§9.6) · unknown-field passthrough (§9.5) |
| **`bytes`** | Rust-side ownership and cheap slicing/cloning | end-to-end zero copy (§11.4) · a reason to skip externalizing large results (§11.5) |
| **`tonic`** | the RPC object model, transport, HTTP/2 knobs, generated client/server | query admission (§28.2) · semantic error representation (§17.1) |
| **`tower`** | cross-cutting service behavior around routing | the daemon's own queue and fairness (§28.5) |
| **`tokio` / `tokio-stream` / `tokio-util`** | the runtime, the socket, stream adaptation, the cancellation tree | task ownership decisions, which are yours (§25.3) |
| **health / reflection / `tonic-types` / `prost-reflect` / `pbjson`** | standard protocol extensions and dynamic descriptor tooling | readiness authority (§30.1) · compatibility negotiation (§31.3) · canonical JSON (§33.2) |

---

## Version anchors

* **Tonic 0.14.6 family** — `tonic`, `tonic-prost`, `tonic-prost-build`, and the optional
  `tonic-health` / `tonic-reflection` / `tonic-types` move as one release family and constrain each
  other; keep their patch versions aligned (§43.1). Prost is its own family — `prost`,
  `prost-build`, `prost-types` on one 0.14.x patch (§43.2).
* **The 0.14 codegen topology changed.** `tonic-build` is now the *generic* service generator;
  Protobuf integration lives in `tonic-prost-build`. Pre-0.14 `tonic_build::compile_protos(...)`
  examples are wrong for this stack and copying one is a listed anti-pattern (§6.5, §45.2, §44).
* **Three independent version sequences.** `protoc` is on 36.x, the companion Python runtime on
  7.36.x, Rust Prost on 0.14.x. They are not one numbering scheme (§3.1).
* **MSRV.** Treat Tonic's **Rust 1.88** as the effective floor for the combined transport stack, and
  keep `rust-version` explicit so resolution fails predictably (§2.4).
* **`tokio-util` has no default features** — `CancellationToken` needs `rt` (§2.2). Tonic's
  compression and TLS gates stay off in a local UDS profile (§2.3).
* **The official `grpc` Rust crate is a 0.9.0 preview** and is explicitly not for production; Tonic
  0.14.6 remains the recommendation. §42.2 lists the eight conditions that would trigger
  re-evaluation (§42.0).

**Read the pins from `FAB §2.1` and the session context — never from this document.** §45.10 and the
§2.1 manifest record what the reference was *written against*, which is not automatically this
repository's resolution: the document anchors `tokio-stream` 0.1.19 where the repository pins
`=0.1.18`. Do not copy a number out of a code fence into a tracked file.

---

## The document and how to read it

`docs/library_ref/rust_grpc_daemon_advanced_reference_tonic_0.14.6.md` — **3,837 lines**, one
document, **48 chapters** (`# N)` — closing paren, not a dot), **no appendices**.

| Block | Lines | Contents |
|---|---|---|
| Front matter | 1-172 | version/source anchor table, the source-precedence ladder, the feature inventory, and the document's own §0-§47 TOC |
| Body | 173-3313 | §0-§43, uniform `## N.M` subsections |
| Reference apparatus | 3314-3837 | §44 anti-patterns · §45 the ten matrices · §46 the checklist · §47 sources and the closing compression |

**This document is unusually flat.** Median chapter is **66 lines**; only six exceed 100. Almost
every chapter fits in a single `Read`, and they are written to be read in order — the binding rule
is usually in the last subsection, not the first. So **read whole chapters here**; windowing costs
more than it saves.

**Load this first when orienting**: §0 (the four contract planes and the authority hierarchy), then
`## Final architecture compression` at the very end (3771-3837) — sixty fenced lines that reduce all
48 chapters to five stacked blocks. When you are *choosing* rather than implementing, §45's ten
matrices and §44's 52 anti-patterns are faster than any chapter.

**Three navigation hazards**, quantified in `REFERENCE §5`: a bare `rg '^# '` overcounts headings by
eight (TOML comments in one Cargo fence in §2.1); §44, §46 and §47 abandon `## N.M`, so
section-number greps against them return nothing; and §46's ~104-item checklist lives *inside* a
code fence where no heading tool will find it. `REFERENCE §1.2` indexes all three by line.

---

## Reading paths by problem context

Seventeen jobs. Each names the chapters in the order they actually help. `REFERENCE §3` is the same
material as a flat goal index.

### 1. You are designing or changing the `.proto` contract

Start at **§0.1** — the four planes — and decide what genuinely belongs on the control plane.
Protobuf should strongly type authentication, compatibility, handles, progress, cancellation,
leases, result descriptors and bounded envelopes; growing it into a duplicate semantic DTO graph is
the document's named "most common architecture error". Then **§0.3** for the authority hierarchy
(`.proto` → descriptor → generated caches), **§4.1**/**§4.4** to fix the Buf lint and breaking
policy, and **§43.5** for what a schema change must pass. **§3.4** before any Edition move.

### 2. You are generating and committing the bindings

**§6.1** for the minimal `build.rs`, **§6.2** for the production builder and its fifteen options,
**§7** when a mapping needs `prost_build::Config` directly. **§6.3** if Buf already produced the
descriptor set and you want to generate from it rather than re-running `protoc` — that is the path
that makes fingerprint, reflection, Rust API and Python fixtures identical by construction
(**§6.4**). Then **§8** for what lands where, and **§5.2** for the CI drift gate. **§6.5** is the
one to read twice: do not copy pre-0.14 `tonic-build` examples.

### 3. You are choosing a method shape for a new RPC

**§15.0**'s four-row table, then **§15.1**'s mapping for the nine daemon methods, then the matching
**§36.N**. Two rules dominate: return an accepted handle before long work (**§15.2**), and do not
reach for bidirectional streaming to make cancellation look symmetrical (**§15.1**). A resumable
stream needs application-level position — HTTP/2 supplies none (**§15.3**).

### 4. You are writing the server handler

**§14.0** for the trait shape, **§14.2** for the handler flow, and **§8.5** for the rule that keeps
generated types from spreading: validate and authorize at the boundary, convert to daemon-owned
types, run the domain operation, convert back. **§12** for the object model underneath. If the
method streams, **§14.3** — the stream outlives the handler future, so per-query leases and
cancellation guards must move into the stream state.

### 5. You are writing or embedding a client

**§13.0**-**§13.2** for the generated client and per-call requests, **§13.3** for the size ceilings,
**§13.1** and **§13.5** for the one rule that matters most — one long-lived channel per process,
cloned handles, never a connection per call. **§21** for `Endpoint` and `Channel`, **§22.1** for the
UDS connector.

### 6. You are putting the boundary on a Unix socket

**§22.0** for what UDS does and does not prove, **§22.1** for the client connector, **§23.0** for
the listener and `serve_with_incoming`, **§26.1** for the `UnixListenerStream` that joins them.
Then **§24** for the parts that are easy to get wrong: runtime-directory permissions established
*before* bind, safe stale-socket handling, and symlink exposure. **§22.3**: a socket file existing
is not readiness.

### 7. You are deciding who may call

**§23.1** to get `UdsConnectInfo` out of request extensions, **§23.2** for `UCred` and the
platform-dependent `pid()`, **§23.3** for the eight-step admission ladder, **§23.4** for why PID
alone is not a principal. Then **§40.1**-**§40.4** for the hardening view: verify kernel-provided
identity, never a caller-supplied one; bind the credential to agent, workspace, operation, expiry
and anti-replay identity; and reauthorize on every sensitive call rather than treating an opaque ID
as bearer authority. **§39.7** for the tests.

### 8. You are deciding what an error means

**§17.0**'s eleven-row code table, then **§17.1** — the distinction that shapes the whole response
contract: a semantic failure in one query block is a *record inside a successful response*, not a
transport failure. **§12.4** for what `Status` is genuinely for, **§17.2**-**§17.4** for richer
details and the rule that typed details are not automatically safe. **§45.8** is the one-screen
version. **§40.7** and **§16.5** for what must never leak, in bodies or in trailing metadata.

### 9. You are building a stream and keeping it bounded

**§19.0** first: HTTP/2 flow control is an asset, and draining a stream into an application queue
throws it away. **§19.1** and **§19.2** for the unbounded/bounded channel decision, **§26.2** and
**§26.3** for the adapters, **§19.3** for carrying backpressure out of the execution engine, and
**§19.4** for what each stream item must carry to be reassembled. **§19.5** is the acceptance test.

### 10. You are deciding how big a payload may be

**§20.0** — keep six separate limits, because a 1 GiB logical result needs no large gRPC message.
**§20.1** for the 4 MiB Tonic decode default, which is an implementation default and not your
policy, **§20.2**-**§20.3** for setting both ends. **§11.5** for the small-inline/large-externalized
split, **§11.3** for which fields deserve `Bytes`. **§20.6** last and loudest: raising the limit is
not the first fix.

### 11. You are wiring deadlines and cancellation

**§18.0** separates four things people conflate — deadline, local timeout, cancellation, business
TTL. **§18.2** for propagating a budget as `grpc-timeout` rather than a local future timeout,
**§18.3** for nesting, **§18.6** for reserving cleanup. **§18.4** for the two-layer model: transport
cancellation stops transport work, `CancelQuery` addresses the accepted logical query. **§27** for
the token tree that carries it inward, and **§18.5** — dropping a future does not reach spawned
tasks.

### 12. You are doing admission control

**§28.0**-**§28.1** for Tower's place and why layer order is semantic. **§28.2** is the core point:
a connection-wide concurrency limit is not query admission, and conflating them makes `CancelQuery`
unable to get service. Then **§28.3**-**§28.8** per layer, **§29** for the HTTP/2 knobs — which are
explicitly *not* initial tuning requirements for a local socket — and **§40.8** for the
starvation failure mode this exists to prevent.

### 13. You are shutting down, reconnecting, or resuming

**§37.0**'s twelve-step shutdown order, **§37.1** for shutdown-aware serving, **§27.5** for
cancelling the parent token and draining within the grace. **§37.2**: a long stream may outlive the
grace, which is exactly why resume exists. **§37.4** for reconnect — re-Handshake and `AttachQuery`,
never a fresh `StartQuery` because the stream broke. **§37.5** for credential invalidation across
restart.

### 14. You are proving Rust↔Python compatibility

**§35.1** — a Rust build passing proves nothing about the Python package. **§34** is the mechanism:
descriptors are the best derived contract artifact, hashed with documented normalization
(**§34.1**-**§34.2**), asserted at build time (**§34.3**), and compared as a *semantic graph*, never
as generated source text (**§34.4**). Then **§35.2** for the fixture matrix, **§35.4** for the
unknown-field asymmetry, **§35.3** for why ProtoJSON needs its own fixtures, and **§35.9** for why
the interop test must use a real socket.

### 15. You are making a performance claim

**§38.0**: for local UDS, structural choices dominate knob tuning, and the twelve-item baseline is
already the answer most of the time. **§38.2** to measure in layers before attributing anything,
**§38.3** for per-method metrics, **§38.5** for copy accounting. **§38.8** fixes the tuning order —
message shape first, exotic transport settings eighth. **§38.1** is explicit that configuring every
available knob to look tuned is not an improvement.

### 16. You are testing it

**§39.0**'s eight test layers, **§39.1** for the schema gates (the chapter's only executable
commands), **§39.2** for descriptor assertions, **§39.3** for wire round trips including old/new
pairs. **§39.4** for the twelve cases every RPC owes, **§39.5** for streaming and resume,
**§39.6** for size boundaries, **§39.7** for peer credentials, **§39.8** for fuzz targets. **§39.10**
is the standard to hold them to: a self-generated expected output is weak; independently generated
clients or released fixtures across the real wire are not.

### 17. You are adopting a crate, or upgrading one

**§41** for what not to adopt by default and why — vendored `protoc`, a second Protobuf runtime,
direct `hyper`/`h2`, generic serialization inside gRPC, gRPC-Web, TLS on a local socket. **§42** for
the official `grpc` Rust preview and the eight conditions that would justify revisiting it. **§43**
for upgrade discipline: eleven distinct upgrade units, two lockstep families, a sixteen-item gate,
and the warning that an unchanged descriptor fingerprint does **not** mean unchanged behavior
(**§43.4**).

---

## The seam: Rust ↔ Python

The document deliberately does not re-document `grpcio` or Python `protobuf` (**§35.0**). It covers
only the joint invariants, and these are the ones that fail in practice:

1. **Generation inputs must match, and a green Rust build does not prove it** (**§35.1**). Same
   `.proto` release, imports, package and service names, field numbers, cardinalities.
2. **Compare descriptors, not source** (**§34.4**). Both generators legitimately emit different
   text from the same schema; the fingerprint is over the normalized `FileDescriptorSet`
   (**§34.1**-**§34.2**), never over generated files (**§5.1**).
3. **Unknown fields are asymmetric** (**§9.5** ↔ **§35.4**). Python's runtime may preserve them
   across parse/reserialize; Prost must not be assumed to. Do not design a compatibility mechanism
   that needs Rust to be a transparent preserving proxy.
4. **Rust owns semantic authority; Python is presentation** (**§36.3**). Python may validate for
   presentation but must not become the schema/capability authority.
5. **Deadlines and cancellation must cross the boundary for real** (**§35.7**-**§35.8**). Python
   `timeout=` has to arrive as a server-visible deadline; `call.cancel()` has to reach the internal
   token and still produce exactly one terminal state.
6. **Size ceilings are set twice** (**§20.3**), and the interop test uses the actual socket, not
   loopback TCP (**§35.9**).

---

## Key invariants

The ten that prevent the most errors; the fourteen navigation rules are in `REFERENCE.md §5`.

1. **The RPC stack is a boundary, not a second model.** Control, transport, lifecycle,
   compatibility, flow control — and nothing semantic. Letting the Protobuf contract grow into a
   duplicate fact/DTO graph is the document's named most common architecture error. (§0.1, §44)
2. **The `.proto` source plus its compatibility policy is the authority; everything else is
   derived.** Descriptor set, Rust output, Python output, reflection descriptor. Generated source
   must never become the place a contract change is made. (§0.3, §44)
3. **Wrap generated types at the boundary.** Validate, authorize, convert to daemon-owned types, run
   the domain operation, convert back — do not let generated structs spread through subsystems
   because they are convenient. (§8.5, §14.2)
4. **Never depend on Prost preserving unknown fields.** Negotiate compatibility explicitly, or keep
   the original encoded bytes. Python may behave differently; that asymmetry is the trap. (§9.5,
   §35.4)
5. **A failed query block is not a failed RPC.** If the released response contract represents
   per-block semantic errors as records, returning a `Status` instead is wrong. (§12.4, §17.1,
   §45.8)
6. **Raising the message limit is never the first fix.** Chunk, externalize, range-read, fix the
   schema, or find the duplicated data first. The 4 MiB Tonic default is an implementation default,
   not application policy. (§20.1, §20.6)
7. **No unbounded queue between production and the wire.** An unbounded bridge converts remote
   backpressure into unbounded daemon memory; a large Tower `Buffer` in front of an
   already-bounded daemon queue does the same thing less visibly. (§19.1, §28.5, §45.5)
8. **Transport concurrency is not query admission.** Conflating them lets heavy streams occupy every
   slot and starves the control methods that would have stopped them. (§28.2, §40.8)
9. **The socket is not authorization.** Neither is the path, nor an opaque artifact ID, nor a
   caller-supplied PID, nor a PID at all — they are reused. Kernel credentials plus a short-lived
   bound capability credential, reauthorized per sensitive call. (§23.3, §23.4, §40.2, §40.4)
10. **A self-generated expected output is a weak oracle.** Independently generated clients and
    servers, or released fixtures, across the actual wire. (§39.10, §35.2)

---

## Navigation hazards

Two that will bite immediately; the full quantified set is `REFERENCE §5`.

* **`rg '^# '` reports 59 chapters; there are 51.** Eight TOML comments inside one `Cargo.toml`
  fence in §2.1 are the decoys. Use `just lib-outline`, and remember the chapter pattern is
  `^# N)` — a closing paren, not a dot.
* **§44, §46 and §47 do not use `## N.M`,** and §46's entire ~104-item checklist is inside a code
  fence. Section-number greps against those three chapters find nothing. `REFERENCE §1.2` is their
  only index.

**Rule of thumb:** the Rust daemon, its socket, and everything generated into it → this skill. The
Python adapter that calls it, and the `.proto` language itself → **`grpcio-orjson-protobuf-ref`**.
The MCP surface above that → **`fastmcp-pydantic-ref`**. What travels through the boundary as
payload → **`datafusion-pyarrow-rust-ref`**, **`deltalake-rust-ref`**, **`canonicalization-lib-ref`**.
