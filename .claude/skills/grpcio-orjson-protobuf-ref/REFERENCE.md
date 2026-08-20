# gRPC + Protobuf + orjson — Reference Companion

Companion to `SKILL.md` in this folder. SKILL.md is the narrative — what each library owns and
which chapters to read for the problem you have. **This file is the mechanical layer**: where
things are, by line and by symbol. Come here once you know which document you need.

All three live at `docs/library_ref/`:

* **`grpcio`** = `grpcio_python_advanced_reference_1.83.0.md` — 2,205 lines, §0-§40, 189 subsections
* **`protobuf`** = `protobuf_python_advanced_reference_7.36.0.md` — 1,707 lines, §0-§44, 143 subsections
* **`orjson`** = `orjson_python_advanced_reference_3.12.0.md` — 1,293 lines, §0-§33, 112 subsections

Citations are `grpcio §N.M` / `protobuf §N.M` / `orjson §N.M`, matching each document's own
numbering. **The alias is never optional** — §0-§33 exist in all three documents and mean different
things in each (Rule 1). Line numbers appear only in §1, because line numbers move when a document
is regenerated and section numbers do not: seek by line, cite by section, and if a line looks wrong
re-derive §1 with `just lib-outline`.

| Section | What it is | Reach for it when |
|---|---|---|
| **§1** | Chapter maps with line numbers, sizes, and subsection ranges | you have a section number, or you need to know where to `Read` |
| **§2** | **Symbol → canonical location**, ~170 public API names | you have a name and need the definition |
| **§3** | Task → location, phrased as goals | you have a goal and no name |
| **§4** | Nine decision trees | you are choosing between library options |
| **§5** | Eight navigation rules | before searching any of the three files |

---

## §1 — Document maps

Front matter is identical in shape across all three (SKILL §"The shared skeleton"): title at line 1,
`## Version / source anchors` at line 5, a feature inventory, then
`# Proposed comprehensive documentation map`. Deep-dive chapters begin after it. **Each document
ends with a `# Source index`** — the last numbered chapter runs up to it, not to EOF.

`Lines` is the chapter's own span including its heading. `Subs` is the `## N.M` range; **`—` means
the chapter is deliberately flat** — prose or a single table, with no subsections for
`lib-outline --view expanded` to show. That is not a tooling failure (Rule 3).

### §1.1 `grpcio` — 41 chapters (§0-§40)

Front matter: title (1) · `## Version / source anchors` (5) · `## Feature inventory` (34) ·
`# Proposed comprehensive documentation map` (40). Chapters start at 86; `# Source index` at 2190.

| § | Line | Lines | Subs | Title |
|---|---:|---:|---|---|
| **§0** | 86 | 79 | 0.0-0.4 | Scope, versioning, and mental model |
| **§1** | 165 | 92 | 1.1-1.4 | Installation, package selection, and project layout |
| **§2** | 257 | 82 | 2.1-2.5 | `.proto` contract and Python code generation |
| **§3** | 339 | 48 | 3.1-3.4 | Generated-code anatomy |
| **§4** | 387 | 49 | 4.1-4.6 | RPC cardinalities and call-shape selection |
| **§5** | 436 | 81 | 5.1-5.5 | Synchronous client fundamentals |
| **§6** | 517 | 73 | 6.1-6.5 | Synchronous server fundamentals |
| **§7** | 590 | 39 | 7.1-7.4 | Channel, callable, call, and future object model |
| **§8** | 629 | 39 | 8.1-8.5 | Metadata and request context |
| **§9** | 668 | 57 | 9.1-9.5 | Status codes, `RpcError`, and application error contracts |
| **§10** | 725 | 47 | 10.1-10.5 | Deadlines, timeouts, cancellation, and propagation |
| **§11** | 772 | 59 | 11.1-11.6 | TLS, credentials, authentication, and authorization boundaries |
| **§12** | 831 | 55 | 12.1-12.5 | Synchronous interceptors |
| **§13** | 886 | 47 | 13.1-13.5 | AsyncIO architecture and event-loop invariants |
| **§14** | 933 | 75 | 14.1-14.5 | AsyncIO client fundamentals |
| **§15** | 1008 | 51 | 15.1-15.4 | AsyncIO server fundamentals |
| **§16** | 1059 | 29 | 16.1-16.4 | AsyncIO interceptors |
| **§17** | 1088 | 89 | 17.1-17.7 | Streaming, read/write discipline, and flow control |
| **§18** | 1177 | 56 | 18.1-18.5 | Channel reuse, connectivity, keepalive, and connection lifecycle |
| **§19** | 1233 | 43 | 19.1-19.5 | Compression, message sizes, and channel/server options |
| **§20** | 1276 | 39 | 20.1-20.6 | Name resolution, service config, load balancing, and retries |
| **§21** | 1315 | 30 | 21.1-21.4 | Health checking |
| **§22** | 1345 | 20 | 22.1-22.4 | Server reflection |
| **§23** | 1365 | 35 | 23.1-23.4 | Rich status details (`grpcio-status`) |
| **§24** | 1400 | 36 | 24.1-24.4 | Channelz, admin services, CSDS, and runtime diagnostics |
| **§25** | 1436 | 46 | 25.1-25.5 | OpenTelemetry / Python observability |
| **§26** | 1482 | 64 | 26.1-26.6 | Testing generated and generic RPC surfaces |
| **§27** | 1546 | 47 | 27.1-27.5 | Server lifecycle, graceful shutdown, and drain behavior |
| **§28** | 1593 | 42 | 28.1-28.5 | Concurrency, thread pools, executors, and AsyncIO migration pools |
| **§29** | 1635 | 55 | 29.1-29.6 | Performance engineering |
| **§30** | 1690 | 35 | 30.1-30.4 | Multiprocessing, fork, subprocesses, and worker models |
| **§31** | 1725 | 48 | 31.1-31.3 | Generic handlers and custom request/response codecs |
| **§32** | 1773 | 41 | 32.1-32.3 | Dynamic invocation and method descriptors |
| **§33** | 1814 | 18 | 33.1-33.3 | xDS and advanced deployment surfaces |
| **§34** | 1832 | 48 | 34.1-34.4 | Security hardening |
| **§35** | 1880 | 74 | 35.1-35.7 | FastMCP / daemon integration architecture |
| **§36** | 1954 | 32 | 36.1-36.4 | Production topology patterns |
| **§37** | 1986 | 41 | 37.1-37.6 | Upgrade and compatibility guidance for the 1.8x line |
| **§38** | 2027 | 25 | — | Anti-pattern inventory |
| **§39** | 2052 | 84 | 39.1-39.7 | Dense API and decision matrices |
| **§40** | 2136 | 54 | — | Agent implementation checklist |

**§39 lookup matrices** (2052-2135) — the fastest answer layer in this document:
`39.1` runtime constructors (2054, every public channel/server signature in one table) ·
`39.2` cardinality (2066) · `39.3` credentials (2075) · `39.4` **status-code rule table** (2087) ·
`39.5` component packages and whether each belongs in production (2103) ·
`39.6` sync-vs-AsyncIO by workload (2115) · `39.7` streaming shape by requirement (2125).

### §1.2 `protobuf` — 45 chapters (§0-§44)

Front matter: title (1) · `## Version / source anchors` (5) ·
**`### The critical version-numbering distinction` (9)** — the only `###` in the document, and the
authority on 7.36.x-vs-36.x · `## Feature inventory` (50) ·
`# Proposed comprehensive documentation map` (56). Chapters start at 106; `# Source index` at 1694.

| § | Line | Lines | Subs | Title |
|---|---:|---:|---|---|
| **§0** | 106 | 53 | 0.0-0.4 | Scope, versioning, and mental model |
| **§1** | 159 | 51 | 1.1-1.4 | Installation, compiler/runtime topology, and project layout |
| **§2** | 210 | 35 | 2.1-2.3 | Syntax/edition selection: Editions, proto3, proto2 |
| **§3** | 245 | 39 | — | First `.proto` and generation pipeline |
| **§4** | 284 | 37 | 4.1-4.4 | Generated Python module anatomy and type stubs |
| **§5** | 321 | 64 | 5.1-5.4 | `Message` base API and object lifecycle |
| **§6** | 385 | 36 | 6.1-6.4 | Scalar fields, defaults, and type assignment |
| **§7** | 421 | 49 | 7.1-7.5 | **Field presence: explicit vs implicit** |
| **§8** | 470 | 41 | 8.1-8.3 | Singular message fields and ownership |
| **§9** | 511 | 35 | 9.1-9.4 | Repeated scalar/message fields |
| **§10** | 546 | 37 | 10.1-10.4 | Maps and map-entry semantics |
| **§11** | 583 | 40 | 11.1-11.3 | `oneof` and mutually exclusive state |
| **§12** | 623 | 30 | 12.1-12.4 | Enums and unknown enum values |
| **§13** | 653 | 20 | 13.1-13.4 | Strings, bytes, numeric ranges, and floats |
| **§14** | 673 | 28 | 14.1-14.3 | Unknown fields and forward compatibility |
| **§15** | 701 | 34 | 15.1-15.4 | Binary wire format mental model |
| **§16** | 735 | 40 | 16.1-16.4 | Serialization and deterministic output |
| **§17** | 775 | 48 | 17.1-17.6 | Parsing, merge, copy, equality, and clear operations |
| **§18** | 823 | 54 | 18.1-18.7 | ProtoJSON mapping and compatibility |
| **§19** | 877 | 25 | — | TextFormat and why it is not a stable interchange wire format |
| **§20** | 902 | 32 | 20.1-20.4 | Descriptors and generated reflection |
| **§21** | 934 | 37 | 21.1-21.4 | `DescriptorPool`, symbol lookup, and file descriptors |
| **§22** | 971 | 21 | — | `message_factory` and dynamic message classes |
| **§23** | 992 | 35 | 23.1-23.3 | `Any` and type URLs |
| **§24** | 1027 | 35 | 24.1-24.5 | Well-known types |
| **§25** | 1062 | 26 | 25.1-25.3 | Extensions and custom options |
| **§26** | 1088 | 41 | 26.1-26.5 | Schema evolution and binary compatibility |
| **§27** | 1129 | 17 | — | JSON-specific schema evolution hazards |
| **§28** | 1146 | 36 | 28.1-28.4 | Package/module/import rules |
| **§29** | 1182 | 21 | — | Python type stubs and static typing |
| **§30** | 1203 | 31 | 30.1-30.4 | Runtime/compiler/generated-code compatibility |
| **§31** | 1234 | 15 | — | Python runtime architecture and upb |
| **§32** | 1249 | 35 | 32.1-32.5 | Performance and serialization sizing |
| **§33** | 1284 | 27 | 33.1-33.3 | Memory behavior and object reuse |
| **§34** | 1311 | 18 | — | Threading/concurrency and mutation rules |
| **§35** | 1329 | 28 | — | Validation limitations and semantic invariants |
| **§36** | 1357 | 26 | 36.1-36.4 | Security and untrusted payloads |
| **§37** | 1383 | 35 | 37.1-37.6 | Testing contracts and compatibility |
| **§38** | 1418 | 24 | — | gRPC integration |
| **§39** | 1442 | 39 | 39.1-39.3 | Pydantic/orjson/FastMCP integration boundaries |
| **§40** | 1481 | 22 | 40.1-40.4 | Editions 2023/2024/2026 and feature lifecycle |
| **§41** | 1503 | 42 | 41.1-41.5 | Upgrade/migration guidance for 7.36 |
| **§42** | 1545 | 28 | — | Anti-pattern inventory |
| **§43** | 1573 | 71 | 43.1-43.6 | Dense API/reference matrices |
| **§44** | 1644 | 50 | — | Agent schema-authoring checklist |

**§43 lookup matrices** (1573-1643): `43.1` **message operations** (1575, every `Message` method in
one table) · `43.2` container operations by shape (1593) · `43.3` presence decision matrix (1602) ·
`43.4` serialization-format matrix — binary / ProtoJSON / TextFormat / ad-hoc (1611) ·
`43.5` **compatibility matrix, binary vs ProtoJSON per change** (1620) · `43.6` toolchain pins (1632).

### §1.3 `orjson` — 34 chapters (§0-§33)

Front matter: title (1) · `## Version / source anchors` (5) · `## Feature inventory` (21) ·
`# Proposed comprehensive documentation map` (27). Chapters start at 66; `# Source index` at 1288.

| § | Line | Lines | Subs | Title |
|---|---:|---:|---|---|
| **§0** | 66 | 43 | 0.0-0.3 | Scope, versioning, and mental model |
| **§1** | 109 | 40 | 1.1-1.5 | Installation, platform, and deployment constraints |
| **§2** | 149 | 53 | 2.1-2.5 | Core API: `dumps()` and `loads()` |
| **§3** | 202 | 39 | 3.1-3.3 | Supported native Python types |
| **§4** | 241 | 27 | 4.1-4.4 | Strings and strict UTF-8 |
| **§5** | 268 | 43 | 5.1-5.5 | Integers, floats, booleans, and null |
| **§6** | 311 | 34 | 6.1-6.6 | `datetime`, `date`, and `time` |
| **§7** | 345 | 15 | — | UUID |
| **§8** | 360 | 35 | 8.1-8.5 | Dataclasses and TypedDict |
| **§9** | 395 | 33 | 9.1-9.4 | NumPy serialization |
| **§10** | 428 | 49 | 10.1-10.4 | `default` callback contract |
| **§11** | 477 | 35 | — | **Option-bitmask model and full inventory** |
| **§12** | 512 | 31 | 12.1-12.3 | Non-string dictionary keys |
| **§13** | 543 | 39 | 13.1-13.5 | Formatting: indent, sorting, newline |
| **§14** | 582 | 20 | — | Date/time option combinations |
| **§15** | 602 | 38 | 15.1-15.3 | Passthrough options |
| **§16** | 640 | 19 | — | Strict integer policy |
| **§17** | 659 | 33 | 17.1-17.4 | `orjson.Fragment` |
| **§18** | 692 | 56 | 18.1-18.6 | `loads()` input types, parser limits, and errors |
| **§19** | 748 | 15 | — | `dumps()` errors and unsupported values |
| **§20** | 763 | 29 | 20.1-20.3 | JSON Lines / NDJSON patterns |
| **§21** | 792 | 31 | 21.1-21.4 | Bytes, files, sockets, and HTTP boundaries |
| **§22** | 823 | 26 | 22.1-22.3 | GIL, concurrency, and async applications |
| **§23** | 849 | 49 | 23.1-23.6 | Performance engineering and benchmarking |
| **§24** | 898 | 39 | 24.1-24.4 | Pydantic integration |
| **§25** | 937 | 17 | — | FastAPI integration |
| **§26** | 954 | 36 | 26.1-26.3 | FastMCP integration |
| **§27** | 990 | 40 | 27.1-27.3 | Protobuf/gRPC integration |
| **§28** | 1030 | 28 | 28.1-28.6 | Security and trust boundaries |
| **§29** | 1058 | 50 | 29.1-29.5 | Testing and compatibility fixtures |
| **§30** | 1108 | 44 | 30.1-30.5 | Upgrade guidance for 3.12 |
| **§31** | 1152 | 24 | — | Anti-pattern inventory |
| **§32** | 1176 | 62 | 32.1-32.4 | Dense API/option matrices |
| **§33** | 1238 | 50 | — | Agent implementation checklist |

**§32 lookup matrices** (1176-1237): `32.1` core API and both exception types (1178) ·
`32.2` **all 14 options with a caveat column** (1188 — better than §11, which is the bare list) ·
`32.3` native-type behaviour, dumps vs what loads returns (1207) ·
`32.4` **which serializer for which boundary** (1225).

---

## §2 — Symbol → canonical location

Where a name is actually *defined or explained*, as opposed to merely mentioned. Every row below
was checked by locating the symbol inside the cited subsection's own line range, not by whole-file
grep — see Rule 2 for why that distinction matters here.

`+` marks a second location worth reading. Symbols that appear in a matrix chapter are noted
because the matrix is often the faster answer.

### §2.1 `grpcio`

| Symbol | Defined at | Also |
|---|---|---|
| `grpc.insecure_channel` | **§5.1** | signature §39.1 · reuse §18.1 · aio §14.1 |
| `grpc.secure_channel` | **§5.1** | §11.1 (with TLS creds) · §39.1 |
| `grpc.intercept_channel` | **§12.1** | §39.1 |
| `grpc.server(...)` | **§6.1** | full signature §39.1 · executor sizing §28.1 |
| `grpc.aio.insecure_channel` / `secure_channel` | **§14.1** | lifecycle §14.2 · §39.1 |
| `grpc.aio.server(...)` | **§15.1** | §39.1 |
| `channel_ready_future` | **§5.5** | — |
| `add_insecure_port` | **§6.3** | §6.1 · §26.2 (`:0` in tests) |
| `add_secure_port` | **§6.3** | §11.2 |
| `start()` / `stop(grace)` / `wait_for_termination()` | **§6.4** | drain §27.1-§27.2 · async §15.3 |
| `maximum_concurrent_rpcs` | **§6.5** | as admission boundary §28.2 · §39.1 |
| `migration_thread_pool` | **§15.2** | §15.1 · §39.1 |
| `xds` (server option) | **§33.1** | signature §6.1 · §39.1 |
| `channel.unary_unary` / `unary_stream` / `stream_unary` / `stream_stream` | **§7.1** | dynamic use §32.1 · §39.2 |
| `.future(...)` | **§5.4** | — |
| `with_call(...)` | **§7.3** | — |
| call object: `initial_metadata()` / `trailing_metadata()` / `code()` / `details()` | **§14.3** (aio) | sync §7.3 |
| `wait_for_ready` | **§18.3** | §5.3 |
| `ServicerContext` | **§0.1** (vocabulary) | server use §6.2 · async §15.4 |
| `context.abort(...)` | **§9.3** | §6.2 |
| `abort_with_status(...)` | **§9.3** | async ABC §15.4 · with `grpcio-status` §23.2 |
| `invocation_metadata()` | **§8.2** | — |
| `time_remaining()` | **§10.2** | propagation §10.4 |
| `is_active()` | **§10.3** | streaming loop §17.1 |
| `RpcError` | **§9.2** | vocabulary §0.1 |
| `AioRpcError` | **§14.4** | facade mapping §35.2 |
| `debug_error_string()` | **§14.4** | — |
| `grpc.StatusCode` | **§9.1** | rule table §39.4 |
| `grpc.ssl_channel_credentials` | **§11.1** | mTLS §11.1 · §39.3 |
| `grpc.ssl_server_credentials` | **§11.2** | §39.3 |
| `access_token_call_credentials` | **§11.3** | §39.3 |
| `metadata_call_credentials` | **§39.3** | described §11.3 |
| `composite_channel_credentials` / `composite_call_credentials` | **§11.3** | §39.3 |
| `grpc.ServerInterceptor` / `intercept_service` / `handler_call_details` | **§12.2** | ordering §12.4 · async §16.1 |
| `read()` / `write()` / `done_writing()` / `grpc.aio.EOF` | **§17.3** | — |
| `grpc.max_send_message_length` / `max_receive_message_length` | **§19.3** | example §5.1 |
| `unary_unary_rpc_method_handler` (and the other three) | **§31.1** | §39.2 |
| `method_handlers_generic_handler` / `add_generic_rpc_handlers` | **§31.1** | — |
| `grpc.health.v1.Health` | **§21.1** | packaging §1.1 |
| `grpcio-status` | **§23.2** | §9.4 · §39.5 |
| `grpcio-health-checking` | **§21.1** | §1.1 · §39.5 |
| `grpcio-reflection` | **§22.2** | §1.1 · §39.5 |
| `grpcio-channelz` / Channelz | **§24.1** | §24.4 · §34.2 |
| CSDS | **§24.3** | xDS context §33.3 |
| `OpenTelemetryPlugin` / `grpc_observability` | **§25.2** | global-registration caveat §25.3 |
| `grpc_tools.protoc` | **§2.2** | CI drift check §2.3 |
| `_pb2_grpc.py` anatomy | **§3.2** | protobuf side `protobuf` §4.3 |
| method path `/<protobuf.package.ServiceName>/<MethodName>` | **§32.3** | §7.1 |

### §2.2 `protobuf`

| Symbol | Defined at | Also |
|---|---|---|
| `SerializeToString()` | **§16.1** | `deterministic=` §16.2 · matrix §43.1 |
| `SerializePartialToString()` | **§16.1** | §43.1 |
| `ParseFromString()` | **§17.1** | §43.1 |
| `MergeFromString()` | **§17.2** | §43.1 |
| `FromString` (classmethod) | **§5.2** | as deserializer §38 |
| `CopyFrom()` | **§17.3** | submessage assignment §8.1 · ownership §5.4 · unknown-field preservation §14.2 |
| `MergeFrom()` | **§17.2** | §43.1 |
| `ByteSize()` | **§16.3** | admission cap §32.2 |
| `Clear()` / `ClearField()` | **§17.6** | presence §7.1 · oneof group §11.2 · retained capacity §33.1 |
| `HasField()` | **§7.1** | not for repeated §9.3 · submessage §8.2 · oneof §11.2 |
| `WhichOneof()` | **§11.1** | §11.2 · §43.1 |
| `ListFields()` | **§17.5** | §43.1 |
| `IsInitialized()` / `FindInitializationErrors()` | **§5** (method list) | — |
| `DiscardUnknownFields()` | **§14.3** | §43.1 |
| `SetInParent()` | **§8.2** | — |
| `get_or_create()` (message maps) | **§10.2** | §43.2 |
| `DESCRIPTOR` (message and file) | **§20.1** / **§20.2** | contract tests §37.4 |
| `FieldDescriptor` | **§20.3** | `is_repeated`/`is_required` §20.4 · `ListFields` §17.5 |
| `GetOptions()` | **§25.3** | mutation is deprecated |
| `DescriptorPool` / `Default()` / `FindMessageTypeByName` | **§21.1** | custom pools §21.2 · conflicts §21.4 |
| `FileDescriptorSet` | **§21.3** | dynamic loading §22 |
| `message_factory` / `GetMessageClass` | **§22** | — |
| `json_format` / `MessageToJson` / `MessageToDict` / `Parse` / `ParseDict` | **§18.1** | orjson two-step §39.2 |
| `text_format` | **§19** | format matrix §43.4 |
| `DecodeError` | **§36.4** | — |
| `google.protobuf.Any` / `Pack` / `Unpack` / `Is` / `TypeName` | **§23** | security allowlist §23.3 |
| `Timestamp` | **§24.1** | — |
| `Duration` | **§24.2** | — |
| `FieldMask` | **§24.3** | why patch APIs need it §7.3-§7.4 · §43.3 |
| `Struct` / `Value` / `ListValue` | **§24.4** | overuse warning §42 |
| wrapper types (`Int32Value`, …) | **§24.5** | prefer native presence |
| `optional` (proto3 explicit presence) | **§7.1** | matrix §7.5 |
| `oneof` | **§11** | design rule §11.3 |
| `reserved` | **§26.2** | invariant §0.4 |
| `edition = "2023"` | **§2.1** | lifecycle §40 |
| `upb` | **§31** | — |
| `_pb2.py` / `_pb2.pyi` / `_pb2_grpc.py` | **§4.1** / **§4.2** / **§4.3** | grpc side `grpcio` §3 |
| packed repeated encoding | **§32.5** | wire types §15.2 |

### §2.3 `orjson`

| Symbol | Defined at | Also |
|---|---|---|
| `orjson.dumps(obj, default=None, option=None)` | **§2.1-§2.2** | signature §32.1 |
| `orjson.loads(obj)` | **§2.1**, **§2.3** | limits/errors §18 · §32.1 |
| `orjson.Fragment` | **§17.1** | **trust boundary §17.2** · risk §28.1 · §32.1 |
| `orjson.JSONEncodeError` | **§19** | first appearance §4.1 · §32.1 |
| `orjson.JSONDecodeError` | **§18.5** | §4.3 · §32.1 |
| `default=` callback | **§10.1** | **must raise `TypeError` §10.2** · 254-level recursion cap §10.3 · never `default=str` §10.4 |
| `OPT_APPEND_NEWLINE` | **§13.4** | JSONL §20.1 · §11 · §32.2 |
| `OPT_INDENT_2` | **§13.2** | §11 · §32.2 |
| `OPT_NAIVE_UTC` | **§6.2** | combinations §14 · §32.2 |
| `OPT_NON_STR_KEYS` | **§12** | collision hazard §12.1 · §28.3 · §32.2 |
| `OPT_OMIT_MICROSECONDS` | **§6.4** | §14 · §32.2 |
| `OPT_PASSTHROUGH_DATACLASS` | **§15.2** | §8.3 · §32.2 |
| `OPT_PASSTHROUGH_DATETIME` | **§15.1** | §14 · §32.2 |
| `OPT_PASSTHROUGH_SUBCLASS` | **§15.3** | redaction example §15.3 · §32.2 |
| `OPT_SERIALIZE_DATACLASS` | **§8.4** | **deprecated no-op** · remove §30.4 |
| `OPT_SERIALIZE_NUMPY` | **§9.1** | dtype/layout limits §9.2 · §32.2 |
| `OPT_SERIALIZE_UUID` | **§7** | **deprecated no-op** · remove §30.4 |
| `OPT_SORT_KEYS` | **§13.3** | cost §23.4 · not canonical §13.5, §28.5 · §32.2 |
| `OPT_STRICT_INTEGER` | **§16** | 53-bit rationale §5.1 · §32.2 |
| `OPT_UTC_Z` | **§6.3** | §14 · §32.2 |
| the full option list | **§11** | **with caveats §32.2** |
| nesting limit (1024) | **§18.3** | not a size limit §18.6 |
| key cache | **§18.4** | not semantic |
| GIL behaviour | **§22.1** | invariants §0.3 |

---

## §3 — Task → location

Phrased the way you would phrase it. SKILL §"Reading paths by problem context" gives the ordered
narrative; this is the flat index.

### §3.1 Shaping the contract

| Goal | Go to |
|---|---|
| choose Editions vs proto3 | `protobuf` §2, §40 |
| distinguish "unset" from "set to default" | `protobuf` §7, matrix §7.5 |
| build a PATCH/partial-update request | `protobuf` §7.3-§7.4, `FieldMask` §24.3, §43.3 |
| express "exactly one of these" | `protobuf` §11 |
| add a field without breaking anyone | `protobuf` §26.3, matrix §43.5 |
| delete a field safely | `protobuf` §26.2 (`reserved`) |
| rename a field or enum value | `protobuf` §18.2, §18.6, §27, §43.5 — binary-safe, JSON-breaking |
| pick the RPC shape | `grpcio` §4, matrix §39.7 |
| put a resume token in a stream | `grpcio` §17.6 |
| decide `string` vs `bytes` vs int for an ID | `protobuf` §13.1, §13.3 |
| version the package namespace | `protobuf` §28.4 |

### §3.2 Generating and wiring

| Goal | Go to |
|---|---|
| the exact codegen command | `grpcio` §2.2 · `protobuf` §3 |
| fail CI when generated code drifts | `grpcio` §2.3, §26.6 · `protobuf` §37.5 |
| get type stubs for generated messages | `protobuf` §29, §4.2 |
| fix "imports work in the repo but not in the wheel" | `grpcio` §2.4 · `protobuf` §28.2-§28.3 |
| pair protoc / runtime / grpcio versions | `protobuf` §30, §30.4, §43.6 · `grpcio` §1.4 |
| know what is safe to depend on in generated files | `grpcio` §3.4 · `protobuf` §4.4, §31 |

### §3.3 Calling and serving

| Goal | Go to |
|---|---|
| create a client that is not slow by construction | `grpcio` §18.1, §29.2, facade §35.2 |
| set and propagate deadlines | `grpcio` §10, §10.4 |
| stop the event loop from stalling | `grpcio` §13.3, §28.3 |
| send auth/trace context with a call | `grpcio` §8.1, §8.4, creds §11.3 |
| observe status/metadata on a *successful* call | `grpcio` §7.3 (sync), §14.3 (async) |
| bound concurrent work on the server | `grpcio` §6.5, §28.2, §28.3 |
| shut down without dropping in-flight work | `grpcio` §27.2, §27.4, async §27.5 |
| expose health to an orchestrator | `grpcio` §21 |
| raise or cap message size | `grpcio` §19.3, and read §19.4 first · `protobuf` §32.2 |
| run a server in a forked/child process | `grpcio` §30.1-§30.3 |
| call a method without generated stubs | `grpcio` §32.1-§32.2, §31.1 |

### §3.4 Errors

| Goal | Go to |
|---|---|
| pick a status code | `grpcio` §9.1, table §39.4 |
| return structured error details | `grpcio` §23 · `protobuf` §23 (`Any`) |
| stop clients string-matching error text | `grpcio` §9.2 |
| map RPC failures to domain errors once | `grpcio` §35.4 |
| catch a bad binary payload | `protobuf` §36.4 (`DecodeError`) |
| catch a bad JSON payload | `orjson` §18.5 |

### §3.5 Serialization and JSON

| Goal | Go to |
|---|---|
| turn a protobuf message into JSON | `protobuf` §18.1 — **not** orjson |
| understand why an `int64` came out as a string | `protobuf` §18.5, §13.4 |
| put JSON inside a protobuf field | `orjson` §27.3 · `protobuf` §39.3 · `grpcio` §35.7 |
| choose a serializer for a boundary | `orjson` §32.4 · `protobuf` §43.4 |
| serialize datetimes consistently | `orjson` §6, §14 |
| handle a type orjson does not know | `orjson` §10 |
| embed pre-serialized JSON | `orjson` §17 — and §17.2 before you do |
| write JSON Lines | `orjson` §20 |
| get reproducible bytes for a cache key | `orjson` §13.3, then §13.5 for why it is not canonical |
| get reproducible protobuf bytes | `protobuf` §16.2 — and why it is not canonical either |

### §3.6 When something is slow, leaking, or flaky

| Goal | Go to |
|---|---|
| find the real cost in an RPC | `grpcio` §29.1, §25.5, §29.5 · `protobuf` §38 |
| decide if streaming will help | `grpcio` §29.3, §4.6 |
| shrink a huge message | `grpcio` §29.4, §19.4 · `protobuf` §33.2 |
| explain flat throughput across threads | `orjson` §22.1 · `grpcio` §28.1 (GIL) |
| explain memory that never comes back | `protobuf` §33.1 |
| debug transport rather than application | `grpcio` §24.1 (Channelz), §18.2 |
| stop a test suite hanging on async fixtures | `grpcio` §26.3 |
| test every failure mode of an RPC | `grpcio` §26.5 |

---

## §4 — Decision trees

**1. Which document answers this?**
Is the question about *what may be said* (fields, types, presence, compatibility)? → `protobuf`.
About *how it travels* (calls, deadlines, status, connections, streams)? → `grpcio`.
About *JSON text* on an interface that is already JSON? → `orjson`.
About a protobuf message becoming JSON? → `protobuf` §18, and the answer is never orjson.

**2. Sync or `grpc.aio`?**
Host application already async (an MCP/ASGI process)? → `aio`. Handlers dominated by blocking
libraries? → sync server with a sized pool. CPU-heavy Python? → neither helps; move the work out of
process (`grpcio` §39.6, §13.5, §28.4). **Pick one per boundary** (§0.2).

**3. Which RPC cardinality?**
Bounded request → bounded response: **unary-unary** (§4.2). One request → many independently
meaningful results: **unary-stream** (§4.3). Chunked upload → one ack: **stream-unary** (§4.4).
Genuinely interleaved session: **stream-stream** (§4.5) — and read §4.6, because streams pin to one
backend and complicate deploys.

**4. Does this field need explicit presence?**
Does the code ever need "not supplied" ≠ "supplied as the default"? No → implicit is fine.
Yes → Editions default / proto3 `optional` (`protobuf` §7.1), or a `FieldMask` if the caller
selects paths (§24.3), or a `oneof` if the variants are exclusive (§11). Repeated and map fields
have no singular presence at all (§9.3, §7.5).

**5. Where does this value belong — protobuf field or gRPC metadata?**
Part of the versioned business contract, should be visible to codegen → **field**.
Transport/request context: auth token, trace id, routing hint → **metadata** (`grpcio` §8.4).
Large → field, never metadata (§8.5).

**6. Status code or structured details?**
Caller only needs to branch → status code alone (`grpcio` §9.1). Caller needs typed, iterable
failure data (per-field validation, retry info) → `google.rpc.Status` with `Any` details
(`grpcio` §23 · `protobuf` §23) — and treat those detail messages as versioned schema (§23.3).

**7. Should this use orjson?**
Is the data already protobuf? → no (`orjson` §27.1). Is it an MCP tool result? → no, keep it
structured (§26.2). Is it a Pydantic model? → let Pydantic serialize, or `model_dump(mode="json")`
first (§24.2). Is it genuinely JSON on a bytes-oriented boundary — cache, log, JSONL, side route?
→ yes (§26.1).

**8. Which `OPT_` flags?**
Start from none. Add only for a stated requirement: JSONL → `OPT_APPEND_NEWLINE`; a fixed datetime
contract → `OPT_NAIVE_UTC` / `OPT_UTC_Z` / `OPT_OMIT_MICROSECONDS` (§14); JavaScript consumers of
64-bit ids → `OPT_STRICT_INTEGER` (§16); NumPy → `OPT_SERIALIZE_NUMPY` (§9.1). `OPT_SORT_KEYS` only
for a real determinism requirement (§13.3, §13.5). Never `OPT_SERIALIZE_DATACLASS` or
`OPT_SERIALIZE_UUID` — deprecated no-ops (§30.4). Combine with `|` (§2.5).

**9. Is this schema change safe?**
Check binary and ProtoJSON **separately** (`protobuf` §0.2, §43.5). Renames: binary-safe,
JSON-breaking. Field-number reuse: never. Singular↔repeated: needs analysis (§26.1). Adding a
`reserved` entry on delete: always. If JSON is an external API, it needs its own compatibility
tests (§18.7, §27, §37.3).

---

## §5 — Navigation rules

1. **Never write a bare `§N`.** §0-§33 exist in all three documents. §23 = rich status (grpcio) /
   `Any` (protobuf) / performance (orjson). §26 = testing / schema evolution / FastMCP. §27 =
   graceful shutdown / JSON evolution hazards / protobuf-gRPC integration. §31 = generic handlers /
   upb / anti-patterns. Always carry the alias, and when reading someone else's citation, work out
   which document it was written against before trusting it.
2. **Grep gives you mentions, not definitions.** These documents are small enough that grep
   *returns*, which makes it more tempting and no more correct: `orjson.dumps` appears in 31 of
   orjson's 112 subsections, `CopyFrom` in 6 protobuf subsections, `insecure_channel` in 12 grpcio
   subsections. The tail chapters (anti-patterns, matrices, checklist) mention nearly every symbol
   once, so a grep's last hits are almost always the checklist rather than the definition. Use §2.
3. **A chapter with no subsections is not a broken outline.** 21 of the 120 numbered chapters are
   deliberately flat. `orjson` §11 (the whole option inventory), `protobuf` §22 (dynamic messages),
   §38 (gRPC integration) and `grpcio` §38 (anti-patterns) are all real content with no `##`
   headings. `lib-outline --view expanded` returning nothing for them is correct.
4. **Read the whole chapter.** No chapter in any of the three exceeds 92 lines. Locating a
   subsection costs more than reading the chapter around it.
5. **Try the matrices chapter before the prose chapter.** `grpcio` §39, `protobuf` §43,
   `orjson` §32 are the deliberate fast-lookup layer and usually answer "which one do I use?"
   in one table. Use the prose chapter for *why*.
6. **Some facts live above §0.** Front matter carries the release-line and platform statements that
   no chapter repeats: protobuf's 7.36.x-vs-36.x distinction (line 9, the document's only `###`),
   grpcio's 1.82.0 yank and EventEngine-by-default notes (lines 9-14), orjson's CPython-only /
   no-PyPy / no-subinterpreters / GIL statement (lines 9-11). Read lines 1-60 for any version
   question the migration chapter does not settle.
7. **Each document ends with a `# Source index`**, not with its last chapter. `grpcio` 2190 ·
   `protobuf` 1694 · `orjson` 1288. A "read to EOF" will pull in the URL list.
8. **Parse headings fence-aware if you script anything.** All three embed `#`-prefixed lines inside
   code fences — `# gRPC Python Advanced`-style comments, `#` in shell blocks, and protobuf's
   `# 43.x` tables. A bare `rg '^# '` over-reports. `just lib-outline` already handles this;
   hand-rolled `grep` does not.
