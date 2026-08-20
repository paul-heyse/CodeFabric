---
name: grpcio-orjson-protobuf-ref
description: "Reference navigator for the three version-pinned Python deep-dives behind *the wire under a FastMCP adapter*: `grpcio_python_advanced_reference_1.83.0.md` (channels, servers, the four RPC cardinalities, metadata, deadlines, status/`RpcError`, TLS/credentials, interceptors, `grpc.aio`, streaming and flow control, keepalive, message sizes, health, reflection, rich status, Channelz, testing, drain, concurrency, fork; §0-§40), `protobuf_python_advanced_reference_7.36.0.md` (Editions vs proto3, generated `_pb2` anatomy, the `Message` API, field presence, repeated/map/oneof/enum semantics, unknown fields, the binary wire format, ProtoJSON, descriptors and `DescriptorPool`, `Any`, well-known types, schema evolution, upb, memory and threading rules; §0-§44), and `orjson_python_advanced_reference_3.12.0.md` (`dumps`/`loads`, the native type set, the 14 `OPT_*` flags, datetime and integer contracts, the `default` callback, `Fragment`, parser limits, JSON Lines, the GIL; §0-§33). SKILL.md is narrative: it explains what each library owns, then gives a reading path for each problem context — designing a `.proto`, generating bindings, writing a client or server, mapping errors, building a stream, deciding whether JSON belongs at all, and operating the boundary. REFERENCE.md (same folder) is the mechanical layer: chapter maps with line numbers, a symbol-to-subsection index, decision trees, and the navigation hazards. Use when Python touches `import grpc`/`grpc.aio`/`insecure_channel`/`secure_channel`/`grpc.server`/`ServicerContext`/`RpcError`/`AioRpcError`/`StatusCode`/`add_*Servicer_to_server`/`wait_for_ready`/`maximum_concurrent_rpcs`/`grpc_tools.protoc`, or `_pb2`/`_pb2_grpc`/`google.protobuf`/`SerializeToString`/`ParseFromString`/`CopyFrom`/`MergeFrom`/`HasField`/`WhichOneof`/`ByteSize`/`DESCRIPTOR`/`DescriptorPool`/`json_format`/`MessageToDict`/`FieldMask`/`Any`, or `import orjson`/`orjson.dumps`/`orjson.loads`/`OPT_*`/`Fragment`/`JSONEncodeError`/`JSONDecodeError`, or a `.proto` file, or when pinning those packages. Serving the MCP surface and modelling its data → sibling `fastmcp-pydantic-ref`. Rust-side parsing, storage, or query → siblings `code-facts-lib-ref`, `deltalake-rust-ref`, `datafusion-pyarrow-rust-ref`, `gix-notify-ref`."
allowed-tools: Read, Grep, Glob, Bash
---

# gRPC + Protobuf + orjson Reference Navigator

Routes the three deep-dive references behind **the wire beneath a Python adapter** — the schema
that defines what may be said, the transport that says it, and the JSON encoder that must be kept
out of the way of both.

These three are not three topics. They are **one stack with a strict layering**, and most of the
mistakes the documents warn about are layering mistakes: putting business data in gRPC metadata,
putting JSON where protobuf already was, calling `HasField` on a field that has no presence,
serializing a protobuf message with `orjson.dumps`. If you read only the chapter that matches the
symbol you are holding, you will miss the rule, because **the rule usually lives in the document
next door**. §"The seam" below is where those rules are collected.

This SKILL.md is the **narrative**: what each library owns, and — for the problem you actually
have — which chapters to read and in what order. The companion **`REFERENCE.md`** (same folder) is
the **mechanical layer**: chapter maps with line numbers, a symbol → defining-subsection index,
decision trees, and the file-level navigation hazards. Reach for REFERENCE.md once you know which
document you need. Cross-references back here are written `SKILL §...`.

**These are pure library navigators.** They index what the three references say, nothing more. No
project doctrine, no design-spec anchoring, no policy about which capabilities are permitted here.

---

## The stack, and who owns what

```text
      .proto contract              <- protobuf: what MAY be said, and how it may change
          |
   protoc / grpc_tools.protoc      <- generation: _pb2.py, _pb2.pyi, _pb2_grpc.py
          |
    generated Message classes      <- protobuf: presence, defaults, unknown fields, wire bytes
          |
   Stub / Servicer / Channel       <- grpcio: cardinality, deadlines, status, metadata, streams
          |
        HTTP/2 transport
          |
   ................................ orjson sits OUTSIDE this column ................
   it belongs on interfaces that were already JSON, never on the protobuf path
```

| Library | Owns | Explicitly does **not** own |
|---|---|---|
| **protobuf** | schema semantics, field identity, presence, wire/JSON mappings, evolution rules, descriptors | domain validation (`protobuf` §35), authorization, canonical hashing (§16.2), arbitrary JSON |
| **grpcio** | transport, framing, cardinality, deadlines, cancellation, metadata, status, credentials, connection lifecycle | your authorization model, idempotency/retry policy, schema evolution policy, service discovery (`grpcio` §0.0) |
| **orjson** | fast JSON encode/decode for data that is *genuinely* JSON | schema validation, streaming, file I/O, ProtoJSON (`orjson` §0.1) |

Two framing statements worth internalising before any of the detail. `grpcio` §0.4: do not choose
gRPC because it is "faster than HTTP" — its value is *contracted RPC semantics plus mature
connection machinery*. `orjson` §0.2: do not insert JSON into a protobuf/gRPC boundary that was
already binary solely to use orjson.

---

## Version anchors

* **`grpcio` 1.83.0**, paired with **`grpcio-tools` 1.83.0** — pin the pair together (§37.1). PyPI
  requires **Python ≥3.10**; the 1.81 line dropped 3.9 (§37.3). Release-line facts an agent will
  otherwise get wrong live in the **front matter (lines 9-14), not in a numbered chapter**:
  1.83.0 added `abort_with_status` to the AsyncIO `ServicerContext` ABC, **1.82.0 was yanked**
  (the lesson is drawn out in §37.2), and **1.80.x made EventEngine the default Python path** — a
  runtime transition with no API change, which is why §37.5 insists upgrades run concurrency,
  shutdown, streaming and observability tests rather than import tests.
* **`protobuf` 7.36.0** (Python runtime, PyPI, **Python ≥3.10**) — and the single most common
  version error with this library: **the Python package line is 7.36.x while the compiler/monorepo
  release family is 36.x**. `protobuf==7.36.0` and `protoc 36.x` are the *same* release, not a
  mismatch. That distinction is in the front matter under `### The critical version-numbering
  distinction` (line 9) — the one `###` heading in the entire document, and invisible to
  chapter-level navigation. Python's generated code has an unusually long compatibility window
  (thin descriptors since 3.20, §30.2), which is *not* a licence to mix arbitrary protoc and
  runtime versions (§30.1, §30.3).
* **`orjson` 3.12.0** — **CPython only**, wheels for 3.10-3.15; no PyPy, no PEP 554
  subinterpreters, no embedded Android/iOS (§1.3). 3.12.0 **substantially rewrote the serializer**
  (§30.2), so an upgrade wants correctness *and* benchmark runs. The **GIL is held for the whole of
  `dumps()` and `loads()`** (§22.1) — native code does not mean parallel threads.

Each document's own pin outranks any figure quoted elsewhere, including here. All three say the
same thing about coordination: `grpcio` §1.4 lists **five separately-versioned things that must move
together** — the grpcio runtime, the grpcio-tools plugin, the protobuf runtime, the protoc
toolchain, and your `.proto` schema version — and notes that a lockfile alone does not record which
compiler built the committed generated files.

---

## The three documents and how to read them

All three live at `docs/library_ref/`. They were written to a shared template, and knowing that
template is most of the navigation.

| Doc | Path (`docs/library_ref/`) | Lines | Chapters | Subs | Deep-dive prefix |
|-----|------|------:|---|---:|---|
| **grpcio** | `grpcio_python_advanced_reference_1.83.0.md` | 2,205 | **§0-§40** | 189 | `# gRPC Python Advanced — 0)`, then `# N) ` |
| **protobuf** | `protobuf_python_advanced_reference_7.36.0.md` | 1,707 | **§0-§44** | 143 | `# Protocol Buffers Advanced — 0)`, then `# N) ` |
| **orjson** | `orjson_python_advanced_reference_3.12.0.md` | 1,293 | **§0-§33** | 112 | `# orjson Advanced — 0)`, then `# N) ` |

**Read whole chapters.** This is the single most important difference from the larger references in
this corpus. **No chapter in any of the three documents exceeds 92 lines** (`grpcio` §1 is the
longest); the medians are 48, 35 and 35.5 lines. Locating a subsection costs more than reading the
chapter that contains it. Use `lib-outline` to find the chapter number, then `Read` the whole span
from REFERENCE.md §1.

**Subsection depth is uniform `## N.M` in all three**, so `lib-outline --view expanded` is complete
and trustworthy — unlike the FastMCP reference next door. A chapter that returns no subsections is
not a tooling failure: 21 of the 120 numbered chapters are deliberately flat prose or a single
table (`orjson` §7, §11, §14, §16, §19, §25, §31, §33 · `protobuf` §3, §19, §22, §27, §29, §31,
§34, §35, §38, §42, §44 · `grpcio` §38, §40).

### The shared skeleton

Every one of the three opens and closes the same way. Learn the shape once and you can land in any
of them:

| Role | grpcio | protobuf | orjson |
|---|---|---|---|
| `## Version / source anchors` | line 5 | line 5 | line 5 |
| `## Feature inventory` | line 34 | line 50 | line 21 |
| `# Proposed comprehensive documentation map` | line 40 | line 56 | line 27 |
| **§0** scope, versioning, mental model | 86 | 106 | 66 |
| **§1** installation, packaging, project layout | 165 | 159 | 109 |
| … the body … | §2-§37 | §2-§41 | §2-§30 |
| **anti-pattern inventory** | **§38** | **§42** | **§31** |
| **dense lookup matrices** | **§39** | **§43** | **§32** |
| **agent checklist** | **§40** | **§44** | **§33** |
| `# Source index` | 2190 | 1694 | 1288 |

That closing triple is the intended fast-lookup layer. When you want *the answer* rather than the
reasoning — which status code, which `OPT_` flag, which `Message` method, which credential
primitive — **go to the matrices chapter first** (`grpcio` §39, `protobuf` §43, `orjson` §32) and
fall back to the prose chapter only for *why*. When you want to know whether a design is already
known to be wrong, the anti-pattern chapter is 19-23 one-line entries and is cheap to scan whole.

---

## Reading paths by problem context

This is the part to use. Find the situation you are actually in; the path is ordered, and the order
matters because each document assumes the layer above it is already settled.

### 1. You are designing or changing the `.proto` contract

Start in **protobuf**, not in grpcio. The schema decisions are the ones you cannot take back:
`protobuf` §0.3-§0.4 establish that a field's durable identity is its **number**, not its name, and
that a shipped number is part of the wire history forever.

Read **§2** to choose Editions vs proto3 — the choice is really about *presence defaults*, and §2.3
warns against migrating a mature schema as a formatting exercise. Then **§7 (field presence)**,
which the document itself calls "one of the most important protobuf concepts for agent-authored
schema design": the difference between *unset* and *set to the default* determines whether patch
semantics can work at all, and §7.5 is the presence matrix that answers it per field shape. Work
through the field-kind chapters as your schema needs them — **§6** scalars and defaults, **§8**
singular message fields and the `CopyFrom` ownership rule, **§9** repeated, **§10** maps (§10.2 is
the message-valued-map read-creates-an-entry surprise), **§11** `oneof`, **§12** enums and the
zero-value rule, **§13** `string` vs `bytes`.

Before committing, read **§26** (binary evolution: never renumber, always `reserved`) and **§27**
(the JSON hazards — short, flat, and the chapter most often skipped). §26.5 is the one to take
seriously: wire-compatible is not semantically compatible.

Only then go to **grpcio §4** to pick the RPC cardinality, and **grpcio §2.5**, which explicitly
hands schema-evolution policy back to protobuf rather than encoding it in method names.

### 2. You are generating and committing the bindings

**grpcio §2.2** has the canonical `python -m grpc_tools.protoc` invocation with `--python_out`,
`--pyi_out` and `--grpc_python_out`; **§2.3** turns it into a CI check that fails on diff, and
**§26.6** repeats that as a test obligation. **protobuf §1.2** and **§1.4** explain why the
generator belongs in the build rather than the runtime image, **§4** is the anatomy of what comes
out (`_pb2.py` descriptors, `_pb2.pyi` stubs, `_pb2_grpc.py` service bindings), and **§29** covers
what the `.pyi` files do and do not buy you. **grpcio §3** covers the same generated files from the
gRPC side — §3.2 shows the `Stub`/`Servicer`/`add_..._to_server` shape, §3.4 warns against
depending on generated internals.

Two import traps: **grpcio §2.4** and **protobuf §28.2** both say the protobuf `package` is a wire
namespace and the Python module path is a filesystem artefact — they are not the same thing, and
generated imports must resolve from an installed wheel, not only from the repo root.

If you are pairing versions, **protobuf §30** is the compatibility chapter and §30.4 spells out the
four-component stack (protoc, protobuf runtime, grpcio-tools, grpcio).

### 3. You are writing the client that calls the daemon

Decide the execution model first — **grpcio §0.2** and **§39.6** — and then stay in it; §0.2 is
explicit that you choose one model per service boundary.

For an async client: **§13** first, because its invariants are the ones that bite. §13.2 —
`grpc.aio` objects are event-loop-bound; §13.3 — blocking work inside a coroutine still blocks
everything; §13.5 — async is not automatically faster. Then **§14** for the constructors, the
awaitable call object (§14.3, when you need metadata *and* the response), and `AioRpcError`
handling (§14.4). For a sync client, **§5** is the equivalent, with `.future()` in §5.4 and
`channel_ready_future` in §5.5.

Then the three chapters that are really *client design* rather than API surface: **§18.1** (reuse
the channel and the stub — creating one per call is the headline anti-pattern), **§10** (every call
gets a deadline, and §10.4 explains propagating a *fraction* of the remaining budget rather than a
fresh fixed timeout), and **§9.2** (branch on `code()`, never on `details()` text).

**§35.2** and **§35.3** show the shape this should end up in: a typed client facade that owns the
stub, created once at process start and closed at shutdown, with error mapping in one place. Add
**§8** if you need request metadata, **§11** for credentials, **§16.2** for client interceptors,
**§18.3** for `wait_for_ready`, **§19.3** for message-size options.

### 4. You are implementing the server side in Python

**§6** for the synchronous server (`grpc.server(...)` with a thread pool, §6.1) or **§15** for the
AsyncIO server (§15.1; §15.2 warns against treating `migration_thread_pool` as the default
execution home for new code). **§6.2** carries the boundary rule both server chapters depend on:
convert at the edge, and do not thread `ServicerContext` through your domain code.

Then the operational chapters, which are what separate a working server from a deployable one:
**§28** (thread pools sized for blocking concurrency, `maximum_concurrent_rpcs` as an admission
boundary above it), **§27** (graceful shutdown — §27.2 is the drain sequence, §27.4 the ordering
rule that dependencies close *after* the server), **§21** (health as a real protocol, not a
hand-rolled `Ping`), **§19** (message limits), **§12** or **§16.1** (server interceptors, with
§12.4's ordering policy and §12.5's "no business logic here").

**§30** if processes are involved at all — §30.1's rule is fork/spawn *first*, create gRPC objects
in the child, never the reverse.

### 5. You are deciding what an error means

**grpcio §9** is the whole model: §9.1 maps situations to `StatusCode` values, §9.3 is
`context.abort`, §9.5 is the discipline that keeps `INTERNAL` meaningful. **§39.4** is the same
mapping as a lookup table and is usually the faster read.

When code-plus-text is not enough, the answer spans two documents: **grpcio §23** introduces
`grpcio-status` and `google.rpc.Status`, and the structured details it carries are protobuf `Any`
payloads — so **protobuf §23** is where the packing/unpacking semantics and the §23.3 allowlist
warning actually live. §23.3 in *grpcio* then reminds you those detail messages are versioned
schema like any other, and §23.4 that structured details make leaking internals easier, not safer.

Finally **grpcio §35.4**: define one mapping layer from gRPC status to domain errors, rather than
letting every call site interpret status codes independently.

### 6. You are building a stream

Choose the shape in **grpcio §4.3-§4.6**, and read §4.6 before committing: once established, a
stream stays pinned to one backend and is not rebalanced per message, which makes it a
*deployment* decision as much as an API one.

**§17** is the discipline chapter — §17.3 on not mixing iterator and explicit `read()`/`write()`
styles on one call, §17.4-§17.5 on producing incrementally instead of materialising the whole
result first, **§17.6** on putting a resume position in the *schema* (this is a `.proto` decision,
so it belongs in path 1), and §17.7 on `try/finally` cleanup when the client disappears.

Then the consequences: **§18** (connection lifecycle and keepalive — §18.4 warns against copying
aggressive keepalive values from examples), **§27.3** (a long-lived stream cannot "finish naturally"
inside a five-second drain, so reconnect/resume is a deployment requirement), **§29.3** (streaming
is not a general speed optimisation), and **protobuf §33.2** (process chunks; do not accumulate
every request message).

### 7. You are deciding whether JSON belongs here at all

This is the context where reading only one document reliably produces the wrong answer, because
**each of the three states the rule from its own side and none states it completely**.

The default is: **no**. **orjson §27.1** and **protobuf §39.2** are the same prohibition seen from
both ends — do not serialize a protobuf message with `orjson.dumps`, because ProtoJSON has defined
field-name, bytes, enum, 64-bit-integer, well-known-type and presence mappings that a generic JSON
encoder knows nothing about. **protobuf §18** is the correct mechanism (`json_format.MessageToJson`
/ `MessageToDict`), and §18.5's 64-bit-integers-become-strings rule is the one most often mistaken
for a bug. If you genuinely need bytes out of a ProtoJSON dict, **protobuf §39.2** shows the
two-step and demands golden fixtures over the result; **orjson §27.2** agrees.

If a payload is *legitimately* opaque JSON riding inside a protobuf `bytes`/`string` field, three
sections define the obligations that creates: **orjson §27.3** (content meaning, schema version,
UTF-8, max size, validation owner), **protobuf §39.3** (otherwise schema evolution has silently
moved into an undocumented second protocol), and **grpcio §35.7** (name and version it explicitly
rather than mixing arbitrary JSON into every RPC). **orjson §21.4** adds that gRPC already frames
messages, so JSON inside a field is an *application sub-format*, not a transport concern.

Where orjson genuinely does belong: **orjson §26.1** — internal caches, genuinely-JSON auxiliary
data, redacted log/event payloads, JSON Lines export, side routes whose response body is bytes.
Where it does not: **§26.2**, returning `orjson.dumps(...)` from an MCP tool, which discards the
structured content the MCP layer would otherwise produce.

**grpcio §31.2** closes the loop from the transport side: a custom orjson gRPC codec is possible
(`grpcio` §31.1) and almost never worth it, because it forfeits the schema and compatibility guarantees that
were the reason to use gRPC.

One boundary this document does not cover: orjson is **not** a canonical-JSON serializer. Sorted
keys plus compact output is not RFC 8785, so `orjson.dumps(..., OPT_SORT_KEYS)` must never produce
fingerprint or digest bytes — for canonicalization, digests and the `codefabric-jcs-v1` profile,
use the sibling skill **`canonicalization-lib-ref`**.

### 8. You are getting orjson's own semantics right

Once JSON is legitimately in scope, orjson is small enough to learn properly. **§0.3** is a seven-line
invariant list worth reading verbatim. **§2** is the entire API — `dumps` returns **bytes** (§2.2,
and §23.3 on not round-tripping through `str`), `loads` takes bytes/bytearray/memoryview/str
(§2.3), there is **no file API** (§2.4), and options are an **OR'ed bitmask, never a list** (§2.5).

**§11** is the flat inventory of all 14 `OPT_*` constants; **§32.2** is the same list with a caveat
column and is the better first stop. The ones with semantics rather than formatting behind them:
**§10** the `default` callback — §10.2 is the trap, a callback that returns instead of raising
`TypeError` silently turns unsupported objects into `null`; **§12** non-string keys and the
duplicate-key collision hazard; **§16** `OPT_STRICT_INTEGER` and the 53-bit question; **§6** plus
**§14** for the datetime contract (pick one and test it — §14 warns against endpoints that disagree
about what a naive datetime means); **§17** `Fragment`, which **bypasses validation entirely**
(§17.2) and is the library's highest-risk surface (§28.1).

Input side: **§18** for accepted types, the 1024-deep nesting limit (§18.3), and `JSONDecodeError`
(§18.5) — with §18.6's reminder that a nesting limit is not a size limit. Output failures are
**§19**. Types that serialize natively without an option are **§3**; NumPy needs one (**§9**).

Two things `OPT_SORT_KEYS` is *not*: cheap (§13.3, §23.4) and canonical (§13.5, §28.5).

### 9. You are operating it

**Performance.** `grpcio` §29.1 is an ordered list of the highest-leverage rules and §29.2 explains
why channel creation is not request setup. Serialization cost is *inside* RPC latency
(`protobuf` §38), so measure it separately (`grpcio` §25.5, §29.5) before blaming transport.
`protobuf` §32 covers binary-vs-JSON and `ByteSize()`; §33 the retained-capacity behaviour of
`Clear()`. `orjson` §22-§23: the GIL is held throughout, so threads do not scale encoding, and
§23.6's warning applies — a 0.2 ms serializer is not the problem in a 100 ms request.

**Testing.** All three have a chapter and they are complementary, not redundant. `grpcio` §26 gives
the four layers, a loopback server recipe (§26.2), and a **per-RPC failure matrix** (§26.5) that is
the most directly actionable list in the three documents. `protobuf` §37 adds the schema-contract
tests: golden binary fixtures, a forward-unknown fixture, **JSON fixtures kept separate** (§37.3,
because §27 means binary compatibility does not imply JSON compatibility), descriptor assertions on
field numbers and full names (§37.4), and a cross-version client/server matrix (§37.6). `orjson`
§29 covers round-trip caveats (JSON preserves JSON types, not Python ones), golden bytes, and an
option-matrix obligation.

**Security.** `grpcio` §34 enumerates the threat boundaries and §34.3 makes the point that loopback
is not an identity boundary; §11.4 separates authentication from authorization. `protobuf` §36 —
typed does not mean safe; §23.3 — allowlist any dynamic `Any` dispatch. `orjson` §28 — `Fragment`
trust, `default` leaking reprs, and bounding total input size.

**Upgrading a pin.** Each document ends with a migration chapter and an explicit checklist:
`grpcio` §37 (checklist §37.6), `protobuf` §41 (§41.2 for the 6.x→7.x event, §41.5 on whether to
regenerate), `orjson` §30 (checklist §30.5). Read them alongside the *other* two documents'
compatibility chapters, because these pins move together (§"Version anchors" above).

---

## The seam: rules that live between the documents

Six rules that no single document states completely. If you are working near one of these, read
both sides.

1. **Never `orjson.dumps` a protobuf message.** `orjson` §27.1 · `protobuf` §39.2 · correct
   mechanism `protobuf` §18.1.
2. **Rich gRPC errors are protobuf messages.** `grpcio` §23 gives the envelope; `protobuf` §23
   gives `Any` packing and the §23.3 allowlist rule; `grpcio` §9.4 says when to reach for it.
3. **The generated stub *is* the serializer boundary.** `grpcio` §7.1 and `protobuf` §38 show the
   same `unary_unary(path, request_serializer=..., response_deserializer=...)` construction from
   opposite sides — which is why protobuf encode/decode is inside your RPC latency budget.
4. **JSON inside a protobuf field is a second protocol.** `orjson` §27.3 · `protobuf` §39.3 ·
   `grpcio` §35.7 · framing note `orjson` §21.4.
5. **Message size is governed twice.** `grpcio` §19.3 sets transport limits and §19.4 says raising
   them is not a fix; `protobuf` §32.2 sets an application cap via `ByteSize()` beneath it; the
   payload-shape answer is `grpcio` §29.4 (chunk envelopes) and `protobuf` §33.2.
6. **Neither library validates your domain.** `protobuf` §35 (protobuf enforces types and ranges,
   not `end >= start`) · `grpcio` §9.1 (`INVALID_ARGUMENT` is for input that is structurally valid
   protobuf but semantically wrong).

---

## Key invariants

Drawn from the documents' own invariant sections — `grpcio` §0.3 and §29.1, `protobuf` §0.2-§0.4,
`orjson` §0.3.

1. **A field's durable identity is its number, not its name.** Renaming is binary-safe and
   JSON-breaking; a shipped number is never reused, and deleted ones are `reserved`
   (`protobuf` §0.3, §0.4, §26.1-§26.2).
2. **Reason about three compatibility layers separately**: binary wire, ProtoJSON, and
   generated-code/runtime/toolchain. A change can be safe in one and unsafe in another
   (`protobuf` §0.2, §27, §30).
3. **Reuse channels and stubs; a channel is long-lived and thread-safe.** Channel-per-call is the
   defining gRPC Python performance bug (`grpcio` §0.3, §18.1, §29.1-§29.2).
4. **Deadlines and cancellation are part of the RPC contract**, not timeout wrappers bolted on —
   set them at the caller, observe them in the handler, propagate a bounded remainder downstream
   (`grpcio` §0.3, §10).
5. **`grpc.aio` objects are event-loop-bound, and blocking work inside a coroutine blocks
   everything** sharing that loop (`grpcio` §13.2-§13.3).
6. **Metadata is transport context, not payload.** It is not governed by protobuf compatibility and
   it travels in HTTP/2 headers subject to proxy limits (`grpcio` §0.3, §8.4-§8.5).
7. **Presence and value are different questions.** Reading an unset scalar returns its default;
   that is not the same as the field being set (`protobuf` §6.2, §7).
8. **`dumps()` returns bytes, the GIL is held throughout, and `Fragment` bypasses validation**
   (`orjson` §0.3, §2.2, §22.1, §17.2).
9. **Deterministic is not canonical.** Neither `SerializeToString(deterministic=True)`
   (`protobuf` §16.2) nor `OPT_SORT_KEYS` (`orjson` §13.5, §28.5) is a cross-version signing format.
10. **Pin exactly and move the pins together** — grpcio, grpcio-tools, protobuf runtime, protoc,
    and the schema (`grpcio` §1.4, §37.1 · `protobuf` §30.4).

---

## Navigation hazards

The full list is REFERENCE §5. The two that will cost you time immediately:

* **Chapter numbers collide across all three documents.** §0-§33 exist in every one of them and
  mean different things: **§23** is rich status in grpcio, `Any` in protobuf, performance in orjson;
  **§26** is testing / schema evolution / **FastMCP integration**; **§27** is graceful shutdown /
  JSON evolution hazards / **protobuf-gRPC integration**. A bare `§N` is ambiguous — **always
  carry the document alias**, and when you read a citation from elsewhere, check which document it
  was written against.
* **Some load-bearing facts are in the front matter, above §0**, where chapter navigation cannot
  reach them: protobuf's 7.36.x-vs-36.x version-numbering distinction (line 9), grpcio's release-line
  notes including the 1.82.0 yank and the EventEngine default (lines 9-14), and orjson's
  CPython-only/GIL/platform statement (lines 9-11). If a version question is not answered by the
  migration chapter, read lines 1-60 of the document.

---

## FastMCP touchpoints

Each document carries its own bounded FastMCP-integration chapter, written from that library's
side. They are the right entry points for adapter-shaped questions, and they are deliberately the
*only* FastMCP content here:

* **`grpcio` §35** — the fullest treatment: process separation (§35.1), the client facade (§35.2),
  channel lifetime tied to the server lifespan rather than per tool call (§35.3), error mapping
  (§35.4), deadlines that leave budget for result conversion (§35.5), resumable change feeds
  (§35.6). Add **§30.3** (build the channel *inside* the child process; never inherit one) and
  **§0.4** for when gRPC is the right boundary at all.
* **`orjson` §26** — where orjson helps in an adapter (§26.1), why tool results should stay
  structured (§26.2), and keeping the daemon boundary protobuf (§26.3).
* **`protobuf` §39** — the boundary chapter: Pydantic for the Python-facing edge, protobuf for the
  wire, and §39.1's rule that generated messages should not become the public tool schema.

Anything about the MCP surface itself — tools, resources, `Context`, transports, auth, the client,
or Pydantic modelling — is out of scope here and belongs to the sibling **`fastmcp-pydantic-ref`**.
Rust-side work belongs to **`code-facts-lib-ref`**, **`gix-notify-ref`**, **`deltalake-rust-ref`**
and **`datafusion-pyarrow-rust-ref`**.
