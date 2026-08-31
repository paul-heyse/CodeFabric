# Rust gRPC Daemon Stack — Reference Companion

This document is the deep-dive companion to `SKILL.md` in the same directory. SKILL.md carries the
core map (version anchors, the document's layout, reading strategy, the seventeen reading paths, the
Rust↔Python seam, and the key invariants); this file carries the full **chapter and sub-chapter
index with line numbers**, the **symbol → canonical location index** across fourteen crates and two
CLIs, the **task index**, the **ten decision trees**, and the **fourteen navigation rules**.

Cross-references back into the core map are written `SKILL §...`. Read SKILL.md first; reach here
once you know which chapter you need, when a design choice gets hard, or — most often — when you
have an API name and need the place that actually explains it.

**This is a pure library navigator.** It indexes what the reference says, nothing more. No project
doctrine, no design-spec anchoring, no policy about which capabilities are permitted here — that
belongs to whatever consumes this skill, not to the skill.

**Line-number policy: seek by line, cite by section.** Line numbers appear only in §1, because line
numbers move when a document is regenerated and section identifiers do not. Re-derive the whole of
§1 with:

```bash
just lib-outline docs/library_ref/rust_grpc_daemon_advanced_reference_tonic_0.14.6.md
just lib-outline docs/library_ref/rust_grpc_daemon_advanced_reference_tonic_0.14.6.md \
  --view expanded --match '^12\)'
```

If a `Read(offset, ...)` lands on the wrong heading, re-derive before trusting anything else here.

| Section | What it is | Reach for it when |
|---|---|---|
| **§1** | Chapter index with lines, sizes and subsection ranges; sub-indexes for the four chapters that break the numbering | you have a section number, or you need to know where to `Read` |
| **§2** | **Symbol → canonical location**, ~190 API names across 14 crates and 2 CLIs, plus 19 verified-absent names | you have a name and need the definition |
| **§3** | Task → location, phrased as goals | you have a goal and no name |
| **§4** | Ten decision trees | you are choosing between options rather than implementing one |
| **§5** | Fourteen navigation rules | before your first grep |

---

## §1 — Document map

`docs/library_ref/rust_grpc_daemon_advanced_reference_tonic_0.14.6.md` — **3,837 lines**, one
document, **48 chapters** (§0-§47) with no appendices.

Front matter: title (1) · `## Version / source anchors` (23) · `### Primary source families` (60) ·
`# Feature inventory` (73) · `# Proposed comprehensive documentation map` (120). Chapters start at
**173** with `# 0)`.

Line numbers are **start lines**, fence-aware, for use as `Read(offset, ...)`. `Lines` is the
chapter's own span including its heading. `Subs` is the `## N.M` range; where a chapter's
subsections are **unnumbered** the cell says so and §1.2 indexes them individually.

### §1.1 Chapter index (§0-§47)

| § | Line | Lines | Subs | Title |
|---|---:|---:|---|---|
| — | 1 | 72 | — | Front matter — version/source anchor table (29-56), source-precedence ladder (62-69) |
| — | 73 | 47 | — | Feature inventory — the layer diagram (79-96) and the coverage list |
| — | 120 | 53 | — | **Proposed comprehensive documentation map** — the doc's own TOC, §0-§47 in one line each |
| **§0** | 173 | 90 | 0.0-0.4 | Scope, versioning, and architectural mental model |
| **§1** | 263 | 55 | 1.1-1.5 | Dependency topology and package selection |
| **§2** | 318 | 66 | 2.1-2.4 | Recommended Cargo feature and pinning posture |
| **§3** | 384 | 58 | 3.0-3.5 | `protoc` 36.x — compiler role and compatibility |
| **§4** | 442 | 110 | 4.0-4.6 | Buf CLI — schema governance and repository workflow |
| **§5** | 552 | 53 | 5.0-5.3 | Canonical `.proto` → descriptor → Rust/Python generation pipeline |
| **§6** | 605 | 97 | 6.0-6.5 | `tonic-prost-build` — service + message integration |
| **§7** | 702 | 80 | 7.0-7.8 | `prost-build` — Rust message mapping controls |
| **§8** | 782 | 87 | 8.0-8.5 | Generated Rust code anatomy and module inclusion |
| **§9** | 869 | 76 | 9.0-9.6 | `prost` runtime — message contract and hot-path semantics |
| **§10** | 945 | 49 | 10.0-10.3 | `prost-types` and well-known/descriptor types |
| **§11** | 994 | 68 | 11.0-11.5 | `bytes` — payload ownership and selective `Bytes` mapping |
| **§12** | 1062 | 73 | 12.0-12.5 | `tonic` object model — Request / Response / Status / Streaming / Extensions |
| **§13** | 1135 | 47 | 13.0-13.5 | Generated Tonic client fundamentals |
| **§14** | 1182 | 55 | 14.0-14.3 | Generated Tonic server fundamentals |
| **§15** | 1237 | 49 | 15.0-15.3 | RPC cardinalities and method-shape selection |
| **§16** | 1286 | 82 | 16.0-16.5 | Metadata, extensions, interceptors, and boundary context |
| **§17** | 1368 | 72 | 17.0-17.4 | Status codes and `tonic-types` richer errors |
| **§18** | 1440 | 70 | 18.0-18.6 | Deadlines, `grpc-timeout`, timeout budgets, and cancellation |
| **§19** | 1510 | 86 | 19.0-19.5 | Streaming, flow control, and backpressure |
| **§20** | 1596 | 84 | 20.0-20.6 | Message sizing, chunking, compression, and payload policy |
| **§21** | 1680 | 54 | 21.0-21.5 | `Endpoint`, `Channel`, and connection lifecycle |
| **§22** | 1734 | 55 | 22.0-22.3 | Unix-domain-socket client integration |
| **§23** | 1789 | 77 | 23.0-23.4 | Unix-domain-socket server integration and peer credentials |
| **§24** | 1866 | 51 | 24.0-24.4 | Socket filesystem lifecycle, process identity, and local security |
| **§25** | 1917 | 54 | 25.0-25.5 | Tokio runtime design for the daemon |
| **§26** | 1971 | 48 | 26.0-26.4 | `tokio-stream` adapters |
| **§27** | 2019 | 62 | 27.0-27.5 | `tokio-util::CancellationToken` and structured cancellation |
| **§28** | 2081 | 86 | 28.0-28.8 | Tower middleware, admission, load shedding, buffers, timeout, retry |
| **§29** | 2167 | 57 | 29.0-29.6 | HTTP/2 flow windows, concurrent streams, keepalive, frames, headers |
| **§30** | 2224 | 47 | 30.0-30.3 | `tonic-health` |
| **§31** | 2271 | 56 | 31.0-31.4 | `tonic-reflection` and descriptor embedding |
| **§32** | 2327 | 58 | 32.0-32.5 | `prost-reflect` and dynamic schema tooling |
| **§33** | 2385 | 47 | 33.0-33.4 | `pbjson`, `pbjson-build`, and `pbjson-types` |
| **§34** | 2432 | 67 | 34.0-34.5 | Descriptor fingerprints, schema authority, and build reproducibility |
| **§35** | 2499 | 91 | 35.0-35.9 | Cross-language Python ↔ Rust interoperability |
| **§36** | 2590 | 96 | 36.0-36.9 | Mapping the stack to the FastMCP / daemon service lifecycle |
| **§37** | 2686 | 60 | 37.0-37.5 | Graceful shutdown, reconnect, resume, and accepted-handle semantics |
| **§38** | 2746 | 141 | 38.0-38.8 | Performance engineering and recommended baseline configuration |
| **§39** | 2887 | 144 | 39.0-39.10 | Testing, fuzzing, compatibility, and executable contract checks |
| **§40** | 3031 | 93 | 40.0-40.8 | Security hardening |
| **§41** | 3124 | 48 | 41.0-41.5 | Optional / alternative packages and what not to adopt by default |
| **§42** | 3172 | 45 | 42.0-42.2 | Official `grpc` Rust preview: watch, do not migrate production yet |
| **§43** | 3217 | 97 | 43.0-43.7 | Upgrade and compatibility discipline |
| **§44** | 3314 | 77 | **unnumbered ×7** | Anti-pattern inventory — see §1.2 |
| **§45** | 3391 | 142 | 45.1-45.10 | Dense API / dependency / decision matrices — see §1.3 |
| **§46** | 3533 | 155 | **46.1 only** | Agent implementation checklist — see §1.2 |
| **§47** | 3688 | 150 | **unnumbered ×8** | Source index — see §1.2 |

**Where the weight is.** Median chapter is **66 lines**; only six exceed 100 — §46 (155), §47 (150),
§39 (144), §45 (142), §38 (141), §4 (110). Every other chapter fits in a single `Read`. That makes
whole-chapter reads the normal move here, unlike the larger references other skills route.

### §1.2 The four chapters that break the numbering

`## N.M` holds for §0-§43 and §45. It does **not** hold for §44, §46, or §47, so `--view expanded`
and any `^## 44\.` grep return nothing useful for them. These are their real contents.

**§44 — anti-pattern inventory (3314-3390), 52 items in 7 unnumbered buckets:**

| Bucket | Lines | Items |
|---|---|---:|
| `## Contract and code generation` | 3316-3328 | 10 |
| `## Protobuf / payload` | 3329-3339 | 8 |
| `## Channel / connection` | 3340-3348 | 6 |
| `## Concurrency / flow control` | 3349-3358 | 7 |
| `## Deadlines / cancellation` | 3359-3369 | 8 |
| `## UDS / security` | 3370-3380 | 8 |
| `## Runtime lifecycle` | 3381-3389 | 5 |

**§46 — agent implementation checklist (3533-3659).** The whole checklist is **one `text` code
fence (3535-3656)** containing ~104 `[ ]` items under 14 ALL-CAPS block headers. Those headers are
inside the fence, so no heading tool and no `^#` grep will ever surface them. This table is the only
index of them:

| Block | Line | Block | Line |
|---|---:|---|---:|
| `VERSION / TOOLCHAIN` | 3536 | `DEADLINES / CANCELLATION` | 3585 |
| `SCHEMA AUTHORITY` | 3545 | `STREAMING / BACKPRESSURE` | 3594 |
| `CODE GENERATION` | 3553 | `MESSAGE / PAYLOAD LIMITS` | 3603 |
| `UDS` | 3561 | `HTTP/2` | 3611 |
| `AUTHORIZATION` | 3571 | `PROTOBUF SEMANTICS` | 3617 |
| `CHANNEL / SERVER` | 3578 | `HEALTH / REFLECTION / DYNAMIC TOOLS` | 3624 |
| | | `SHUTDOWN` | 3632 |
| | | `TESTS` | 3641 |

`## 46.1 Source map by topic` (3660-3687) is the chapter's only real subsection: an 18-row table
mapping a topic to the `[KEY]` link families in §47, so a claim can be traced to the right upstream
source family rather than treating all links as equivalent.

**§47 — source index (3688-3837).** Markdown link-reference definitions (`[KEY]: url`) in six
groups, then two prose closers:

| Group | Line | Refs |
|---|---:|---:|
| `## Tonic / gRPC Rust` | 3690 | 15 |
| `## Prost / Protobuf Rust` | 3708 | 7 |
| `## Tokio / Tower / Bytes` | 3718 | 11 |
| `## Protocol Buffers compiler / compatibility` | 3732 | 7 |
| `## Buf` | 3742 | 7 |
| `## Optional ProtoJSON / compiler packaging` | 3752 | 4 |
| `## Companion project references` | 3759 | prose — names the FastMCP, `grpcio`, and Python-protobuf companion references |
| `## Final architecture compression` | 3771 | **the whole document as one fence (3775-3837)** |

**`## Final architecture compression` (3771-3837) is the single densest target in the file.** Sixty
lines of `text` fence reduce all 48 chapters to five stacked blocks: `RELEASED .proto AUTHORITY`
(3776), `UDS ADMISSION` (3806), `RPC LIFECYCLE` (3812), `DATA POLICY` (3821), `PERFORMANCE POLICY`
(3827). Read it first when orienting and last when reviewing.

### §1.3 §45 — the ten matrices (3391-3532)

**108 of the document's 180 table rows are in this one chapter.** Chapters §0-§44 are prose and code
fences with almost no tabular structure — only §15.0, §16.0, §17.0 and §46.1 carry tables at all. So
when you are *choosing* rather than implementing, come here first.

| § | Line | Header | Rows | Answers |
|---|---:|---|---:|---|
| 45.1 | 3393 | `Need \| Reach for \| Scope` | 18 | which crate owns a need, and whether it is runtime / build / optional / tooling |
| 45.2 | 3416 | `Task \| Correct default` | 4 | the Tonic 0.14 codegen topology — `tonic-prost-build`, not `tonic-build` |
| 45.3 | 3425 | `Concern \| Owner` | 8 | who owns each part of the UDS path, from runtime directory to readiness |
| 45.4 | 3438 | `Need \| Primitive` | 7 | which of the seven deadline/cancel primitives applies |
| 45.5 | 3450 | `Buffer/queue \| Default posture` | 6 | allowed / prohibited / avoid, per queue |
| 45.6 | 3461 | `Deployment/payload \| Baseline` | 5 | when compression is even a question |
| 45.7 | 3471 | `Environment \| Reflection` | 4 | reflection on/off per environment |
| 45.8 | 3480 | `Failure/info \| Place` | 6 | gRPC status vs canonical response record vs logs |
| 45.9 | 3491 | `Need \| Tool` | 10 | which descriptor tool for which job |
| 45.10 | 3506 | `Component \| Reference target` | 20 | the document's own version anchors, restated — see Rule 9 |

---

## §2 — Symbol → canonical location

**Use this instead of grep.** The document names its own APIs constantly in prose and in 150 code
fences, so a literal search returns hits with no signal about which one explains the symbol:
`UdsConnectInfo` appears 11 times, `UCred` 7. Every row points at the subsection that *defines or
explains* the symbol; **Also** lists the other places worth reading.

Each row was verified by locating the symbol **inside the cited subsection's own line range**, not
by whole-file grep — see Rule 4. Rows marked **absent** are collected in §2.11: their concept is
covered but the literal name never appears, so grep for them returns nothing.

### §2.1 `tonic` — request/response object model

| Symbol | Defined at | Also |
|---|---|---|
| `tonic::Request<T>` | **§12.1** | construction §13.2 · handler signature §14.0 |
| `metadata()` / `metadata_mut()` | **§12.1** | insertion example §13.2 · types §16.1 |
| `extensions()` / `extensions_mut()` | **§12.1** | the pattern §12.2 · UDS read §23.1 · principal §16.3 |
| `into_inner()` | **§12.1** | — |
| `Request::set_timeout(Duration)` | **§18.2** | listed among request surfaces as `set_timeout(Duration)` §12.1 · matrix §45.4 |
| `Request::new(...)` | **§13.2** | — |
| `Response<T>` | **§12.3** | fully qualified `tonic::Response<T>` in the §12.1 example · handler signature §14.0 |
| `tonic::Status` | **§12.4** | code table §17.0 · UDS construction §23.1 · placement matrix §45.8 |
| `Status::unauthenticated` | **§23.1** | code table §17.0 |
| `tonic::Streaming<T>` | **§12.5** | server-stream lifetime §14.3 |
| request extensions (type-keyed values) | **§12.2** | `RpcPrincipal` §16.3 · `UdsConnectInfo` §23.1 |
| `MetadataMap` | **§16.1** | the three planes §16.0 · header-size limit §29.6 |
| interceptors | **§16.2** | what not to run in one §16.2 · async policy → §28 |
| `tonic::async_trait` | **§14.0** | — |
| `tonic::include_proto!` | **§8.3** | — |
| `include_file_descriptor_set!` | **§8.4** | fully qualified `tonic::include_file_descriptor_set!` §31.1 · matrix §45.9 |

### §2.2 `tonic` — client, server, and transport

| Symbol | Defined at | Also |
|---|---|---|
| generated `Client<T>` (shown as `CpgQueryServiceClient<T>`) | **§13.0** | module anatomy §8.1 · cloning §13.5 |
| `max_decoding_message_size` | **§13.3** | client config §20.2 · the 4 MiB default §20.1 |
| `max_encoding_message_size` | **§13.3** | §20.2 · §20.1 |
| compression negotiation (send/accept) | **§13.4** | policy §20.4 · feature gates §2.3 |
| generated service trait | **§14.0** | module anatomy §8.2 · thin handlers §14.2 |
| generated `...Server<T>` wrapper | **§14.1** | §8.2 |
| `Server::builder()` | **§23.0** | health wiring §30.0 · baseline posture §38.1 |
| `add_service(...)` | **§23.0** | §26.1 · §30.0 |
| `serve_with_incoming(...)` | **§23.0** | §26.1 · §36.0 · matrix §45.3 |
| `concurrency_limit_per_connection` | **§28.3** | knob list §29.0 · why it is not admission §28.2 |
| `load_shed(bool)` | **§28.4** | §29.0 |
| `initial_stream_window_size` | **§29.0** | tuning caution §29.1 |
| `initial_connection_window_size` | **§29.0** | §29.1 |
| `max_concurrent_streams` | **§29.0** | §29.3 · control starvation §40.8 |
| `http2_adaptive_window` | **§29.0** | §29.2 |
| `http2_keepalive_interval` / `http2_keepalive_timeout` | **§29.0** | §29.4 |
| `max_frame_size` | **§29.0** | §29.5 — not a chunking substitute |
| `max_header_list_size` | **§29.0** | §29.6 |
| `max_connection_age` / grace | **§29.0** | — |
| `tonic::transport::Endpoint` | **§21.0** | UDS client §22.1 · connect deadline §21.3 |
| `Endpoint::try_from(...)` | **§21.2** | — |
| `Endpoint::from_static(...)` | **§22.1** | — |
| `connect_with_connector(...)` | **§21.2** | full UDS client §22.1 |
| `connect_timeout` | **§21.3** | distinct from per-RPC deadline §18.0 |
| `Channel` | **§21.1** | fully qualified `tonic::transport::Channel` §22.1 · clone rather than reconnect §13.5 · shutdown §21.5 |
| `Connected` trait | **§23.1** | — |
| `tonic::transport::server::UdsConnectInfo` | **§23.1** | extensions §12.2 · Handshake §36.1 · matrix §45.3 · test §39.7 |
| `peer_addr` / `peer_cred` fields | **§23.1** | §36.1 |
| shutdown-aware serving | **§37.1** | full shutdown order §37.0 |

### §2.3 Build-side — `tonic-prost-build`, `prost-build`

| Symbol | Defined at | Also |
|---|---|---|
| `tonic_prost_build::compile_protos(...)` | **§6.1** | prefer `configure()` in production §6.1 |
| `tonic_prost_build::configure()` | **§6.2** | with a custom Prost config §7.1 |
| `.build_client(bool)` / `.build_server(bool)` | **§6.2** | generated anatomy §8.1-§8.2 |
| `.build_transport(bool)` | **§6.2** | — |
| `.bytes(paths)` | **§6.2** | the mapping decision §7.2 · candidates §11.3 |
| `.btree_map(paths)` | **§6.2** | §7.3 — map order is not a durable contract |
| `.type_attribute` / `.message_attribute` / `.enum_attribute` / `.field_attribute` | **§6.2** | example and caution §7.4 |
| `.extern_path(...)` | **§6.2** | when to use it §7.5 |
| `.file_descriptor_set_path(...)` | **§6.2** | §7.6 · reflection §31.1 |
| `.skip_protoc_run()` | **§6.2** | §7.6 · Buf architecture B §4.5 |
| `.protoc_arg(...)` | **§6.2** | — |
| `.with_extended_rust_types(...)` | **§6.2** | — |
| `.codec_path(...)` | **§6.2** | — |
| `.generate_default_stubs(...)` | **§6.2** | — |
| `.use_arc_self(...)` | **§6.2** | — |
| `compile_fds(...)` / `compile_fds_with_config(...)` | **§6.3** | why it matters §6.4 |
| `.compile_with_config(...)` | **§7.1** | — |
| `prost_build::Config::new()` | **§7.1** | when a direct dependency is needed §1.2 |
| `Config::bytes(...)` | **§7.2** | candidates and non-candidates §11.3 |
| `include_file(...)` | **§7.7** | — |
| source-info exclusion | **§7.8** | `--exclude-source-info` §34.1 · fingerprint §5.1 |
| `tonic-build` — **not** the Protobuf entry point | **§6.5** | topology rule §45.2 · anti-pattern §44 |

### §2.4 `prost` and `prost-types`

| Symbol | Defined at | Also |
|---|---|---|
| `prost::Message` | **§9.0** | derive on generated structs §8.0 |
| `encode` / `encode_length_delimited` / `merge` / `clear` | **§9.0** | — |
| `encode_to_vec()` | **§9.0** | example §9.1 |
| `decode(...)` / `decode_length_delimited` | **§9.0** | example §9.2 · descriptor decode §10.1 |
| `encoded_len()` | **§9.0** | framing caveat §9.3 |
| `bytes::Buf` / `BufMut` integration | **§9.0** | — |
| unknown enum numeric values | **§9.4** | wire fixtures §39.3 |
| unknown-field non-preservation | **§9.5** | Python asymmetry §35.4 · anti-pattern §44 |
| `FileDescriptorSet` | **§10.0** | fully qualified `prost_types::FileDescriptorSet` §10.1 and §6.3 · decode §10.1 · fingerprint §34.0 |
| `FileDescriptorProto` / `DescriptorProto` / `FieldDescriptorProto` | **§10.0** | — |
| `Timestamp` | **§10.0** | not a monotonic budget §10.2 |
| `Duration` (well-known type) | **§10.0** | §10.2 |
| `Any` | **§10.0** | avoid on the control plane §10.3 · anti-pattern §44 |
| `Struct` / `Value` / `ListValue` / `FieldMask` | **§10.0** | `Struct` anti-pattern §44 |
| `FileDescriptorSet::decode(...)` | **§10.1** | pool construction §32.1 |

### §2.5 `bytes`

| Symbol | Defined at | Also |
|---|---|---|
| `bytes::Bytes` | **§11.1** | why the crate matters here §11.0 · selective mapping §11.3 |
| `.clone()` (shares storage) / `.slice(..)` | **§11.1** | — |
| `bytes::BytesMut` | **§11.2** | — |
| `BytesMut::with_capacity` / `extend_from_slice` / `freeze()` | **§11.2** | — |
| `split` / `split_to` / `reserve` | **§11.2** | — |
| selective Protobuf `bytes` mapping | **§11.3** | the builder call §7.2 · §6.2 |
| the "not true zero-copy" caveat | **§11.4** | Python side §35.5 · copy accounting §38.5 |
| small/medium inline vs large externalized | **§11.5** | payload limits §20.0 · buffering matrix §45.5 |

### §2.6 `tokio`, `tokio-stream`, `tokio-util`

| Symbol | Defined at | Also |
|---|---|---|
| `tokio::net::UnixListener` | **§23.0** | `bind` example §26.1 · matrix §45.3 |
| `tokio::net::UnixStream` | **§21.2** | client connector §22.1 · `Connected` impl §23.1 |
| `UnixStream::connect(path)` | **§21.2** | §22.1 |
| `UCred` | **§23.2** | read via `UdsConnectInfo` §23.1 · PID reuse §23.4 · verify §40.2 |
| `uid()` / `gid()` / `pid()` | **§23.2** | tests per OS §39.7 |
| `#[tokio::main(flavor = "multi_thread")]` | **§25.1** | — |
| `spawn_blocking` | **§25.2** | what not to block on §25.2 |
| `tokio::spawn` and task ownership | **§25.3** | shutdown consequence §37.0 · anti-pattern §44 |
| Tokio signal handling | **§25.4** | shutdown order §37.0 |
| Tokio runtime metrics | **§25.5** | measure in layers §38.2 |
| `tokio::sync::mpsc::unbounded_channel` | **§19.1** | prohibited for result data §45.5 · §26.3 |
| `tokio::sync::mpsc::channel(n)` | **§19.2** | wrapping it §26.2 · capacity is a real parameter §19.2 |
| `tokio::select!` | **§27.2** | cancellation-safety caveat §27.2 |
| `tokio_stream::wrappers::UnixListenerStream` | **§26.1** | server wiring §23.0 · `net` feature §26.1 · matrix §45.3 |
| `tokio_stream::wrappers::ReceiverStream` | **§26.2** | bounded channel §19.2 |
| `UnboundedReceiverStream` | **§26.3** | not for result streams §26.3 |
| `StreamExt` / stream combinators | **§26.4** | ordering risk §26.4 |
| `CancellationToken` | **§27.0** | written `tokio_util::CancellationToken` in §45.4 · `rt` feature §2.2 |
| `new` / `clone` / `cancel` / `is_cancelled` / `cancelled_owned` / `run_until_cancelled` | **§27.0** | — |
| `child_token()` | **§27.0** | the cancellation tree §27.1 |
| `cancelled()` | **§27.0** | select pattern §27.2 |

### §2.7 `tower`

The document covers all six layers but writes them as **capability names, not type names** — see
§2.11. Route by concept:

| Concept (grep target) | Defined at | Also |
|---|---|---|
| `ServiceBuilder` | **§28.1** | primitive list §28.0 · direct dependency rule §1.1 |
| `tower::service_fn` | **§21.2** | UDS client §22.1 |
| `ConcurrencyLimit` | **§28.0** (primitive list) | explained §28.2 · Tonic's own knob §28.3 · starvation §40.8 |
| `LoadShed` | **§28.0** (primitive list) | explained §28.4 · Tonic's own `load_shed` §28.4 |
| `Buffer` | **§28.5** | primitive list §28.0 · prohibition §45.5 · anti-pattern §44 |
| `Timeout` | **§28.6** | primitive list §28.0 · four timeout concepts §18.0 |
| `Retry` | **§28.7** | primitive list §28.0 · per-method policy table §28.7 · anti-pattern §44 |
| rate limiting | **§28.8** | keep separate from query admission §28.8 |
| `limit` / `load-shed` / `timeout` / `util` feature flags | **§2.1** | feature minimization §2.2 |

### §2.8 Protocol extensions

| Symbol | Defined at | Also |
|---|---|---|
| `grpc.health.v1.Health` | **§30.0** | optional-package list §1.3 |
| `tonic_health::server::health_reporter()` | **§30.0** | verify against the pinned API §30.0 |
| `set_serving::<T>()` | **§30.0** | not-serving on drain §30.2 · shutdown order §37.0 |
| health vs `Handshake` as readiness | **§30.1** | local-profile value §30.3 |
| `tonic_reflection::server::Builder::configure()` | **§31.1** | — |
| `register_encoded_file_descriptor_set(...)` | **§31.1** | descriptor embedding §8.4 |
| `build_v1()` | **§31.1** | verify names against 0.14.6 §31.1 |
| reflection production policy | **§31.2** | matrix §45.7 · admin surface §40.6 |
| reflection ≠ compatibility negotiation | **§31.3** | descriptor equality test §31.4 |
| `tonic-types` `StatusExt` | **§17.2** | where it helps §17.3 |
| `BadRequest` / `ErrorInfo` / `RetryInfo` / `PreconditionFailure` / `ResourceInfo` / `QuotaFailure` / `RequestInfo` / `DebugInfo` | **§17.2** | safe-details rule §17.4 · Python side §35.6 |
| `prost_reflect::DescriptorPool` | **§32.1** | role §32.0 · cache one pool §32.3 · matrix §45.9 |
| `DescriptorPool::decode(...)` / `get_message_by_name(...)` | **§32.1** | — |
| `DynamicMessage` | **§32.2** | untrusted type dispatch §32.4 |
| `MessageDescriptor` / `FieldDescriptor` | **§32.0** | §32.2 |
| `ReflectMessage` | **§32.0** | `prost-reflect-build` §32.5 |
| `pbjson-build` | **§33.1** | the ProtoJSON problem §33.0 |
| never auto-derive canonical JSON from `pbjson` | **§33.2** | separate JSON fixtures §35.3 |
| `pbjson-types` | **§33.3** | conversion cost §33.4 |
| `protoc_bin_vendored::protoc_bin_path()` / `include_path()` | **§41.0** | why it is not the default §41.0 |

### §2.9 CLI surfaces — `protoc` and Buf

| Symbol | Defined at | Also |
|---|---|---|
| what `protoc` owns | **§3.0** | compiler vs runtime lines §3.1 |
| `PROTOC` environment path | **§3.2** | build reproducibility §3.2 |
| `-I` / `--include_imports` / `--descriptor_set_out` | **§3.3** | Buf equivalent §4.2 |
| `protoc --version` in build diagnostics | **§3.5** | — |
| Edition 2026 policy | **§3.4** | upgrade treatment §43.6 |
| `buf.yaml` | **§4.1** | — |
| `buf build` | **§4.2** | CI form §34.1 · gate §39.1 |
| `buf build --as-file-descriptor-set -o` | **§4.2** | `--exclude-source-info` §34.1 · architecture B §4.5 |
| `buf format -w` | **§4.3** | `--diff --exit-code` in CI §34.1, §39.1 |
| `buf lint` | **§4.3** | §34.1 · §39.1 |
| `buf breaking --against` | **§4.4** | §34.1 · §39.1 · schema upgrade §43.5 |
| `buf.gen.yaml` / `buf generate` | **§4.5** | the three generation architectures §4.5 · matrix §45.9 |
| BSR | **§4.6** | — |

### §2.10 Daemon RPC method vocabulary

The document names nine methods and treats them as fixed vocabulary. §15.1 fixes each one's
cardinality; §36.N explains it; §28.7 gives its retry posture.

| Method | Explained at | Cardinality | Retry posture |
|---|---|---|---|
| `Handshake` | **§36.1** | §15.1 unary-unary | §28.7 bounded startup retry possible |
| `GetStatus` | **§36.2** | §15.1 unary-unary | §28.7 safe bounded retry |
| `ValidateQuery` | **§36.3** | §15.1 unary-unary | §28.7 only if proven idempotent |
| `StartQuery` | **§36.4** | §15.1 unary-unary | §28.7 **never blind retry** |
| `StreamQuery` | **§36.5** | §15.1 unary-stream | §28.7 resume, do not replay |
| `AttachQuery` | **§36.6** | §15.1 unary-stream | §28.7 the resume path itself |
| `CancelQuery` | **§36.7** | §15.1 unary-unary | §28.7 only under released idempotency |
| `ReadResult` | **§36.8** | §15.1 unary-unary **or** unary-stream | §28.7 exact range/lease retry |
| `ReleaseResult` | **§36.9** | §15.1 unary-unary | §28.7 idempotent retry |

Supporting: accepted-handle rule §15.2 · resumable-stream invariant §15.3 · cancellation registry
§27.3 · reconnect §37.4 · streaming tests §39.5.

### §2.11 Verified absent — names the document never writes

These nineteen literals occur **zero** times in the file. Their concepts *are* covered; the name is
not. Grep will not find them, so route by the covering section and verify the exact API against the
pinned crate source, never against this document.

| Symbol you searched for | Concept covered at | Note |
|---|---|---|
| `ConcurrencyLimitLayer` | **§28.2** | written as "concurrency limits" |
| `LoadShedLayer` | **§28.4** | written as "load shedding"; Tonic's own `load_shed` *is* named |
| `BufferLayer` | **§28.5** | written as Tower `Buffer` |
| `TimeoutLayer` | **§28.6** | written as "timeout layer" |
| `RetryLayer` | **§28.7** | written as "retry layer" |
| `RateLimitLayer` | **§28.8** | written as "rate limits" |
| `ServingStatus` | **§30.0**, **§30.2** | health states appear as prose and as `NOT_SERVING` in §37.0 |
| `ErrorDetails` | **§17.2** | the detail *messages* are named; the builder type is not |
| `IntoRequest` | **§12.1**, **§13.2** | requests are constructed explicitly |
| `CompressionEncoding` | **§13.4**, **§20.4** | encodings named as gzip/deflate/zstd |
| `accept_compressed` / `send_compressed` | **§13.4** | described as "send/accept compression configuration" |
| `serve_with_shutdown` | **§37.1** | described as "Tonic's shutdown-aware serving API" |
| `prost::Name` | — | not covered; §10.3 discusses `Any` without the type-URL trait |
| `tonic::Code` / `Code::*` | **§17.0** | codes appear as bare names (`InvalidArgument`, `Unavailable`, …) in the table |
| `JoinSet` | **§25.3** | task ownership stated as a requirement, no API named |
| `tokio::signal` | **§25.4** | "Tokio signal handling" |
| `tokio::time::timeout` | **§45.4** | matrix row says "Tokio `timeout` / budget helper" |

---

## §3 — Task → location

Phrased the way you would phrase it. `SKILL §"Reading paths by problem context"` gives the ordered
narrative; this is the flat index.

### §3.1 Shaping the contract

| Goal | Go to |
|---|---|
| decide what belongs in Protobuf at all | §0.1 (the four planes) · §0.3 |
| pick a method shape | §15.0 table · §15.1 · §36 |
| decide where a value lives — field, metadata, or extension | §16.0 table |
| model a resumable stream | §15.3 · §36.6 · §39.5 |
| return an accepted handle before long work | §15.2 · §36.4 |
| choose a Buf lint/breaking policy | §4.1 · §4.4 · §43.5 |
| decide whether to adopt an Edition | §3.4 · §43.6 |

### §3.2 Generating and wiring

| Goal | Go to |
|---|---|
| write the `build.rs` | §6.1 (minimal) · §6.2 (production builder) |
| generate from an existing descriptor set instead of running `protoc` | §6.3 · §7.6 · §4.5 architecture B |
| map a `bytes` field to `bytes::Bytes` | §7.2 · §11.3 · §6.2 |
| add a derive to one generated type | §7.4 |
| reuse types generated in another crate | §7.5 |
| find where the generated modules land | §8.0-§8.2 · §8.3 |
| embed the descriptor set in the binary | §8.4 · §31.1 · §34.3 |
| stop generated types leaking through the daemon | §8.5 |
| choose between the three Buf generation architectures | §4.5 |
| gate generated-code drift in CI | §5.2 · §39.1 |

### §3.3 Serving and calling

| Goal | Go to |
|---|---|
| implement the server trait | §14.0 · §14.2 (keep it thin) |
| build the server and start serving | §23.0 · §26.1 |
| keep a server stream alive past its handler | §14.3 |
| construct a client over UDS | §22.1 · §21.2 |
| set a per-call timeout that the server can see | §18.2 · §12.1 |
| reuse one channel for the process lifetime | §13.1 · §21.1 · §13.5 |
| get the verified caller identity into a handler | §12.2 · §16.3 · §23.1 |
| add a standard health service | §30.0 · §30.1 |
| turn reflection on for development only | §31.1 · §31.2 · §45.7 |

### §3.4 UDS and security

| Goal | Go to |
|---|---|
| bind and serve on a Unix socket | §23.0 · §26.1 |
| read peer UID/GID/PID | §23.1 · §23.2 · §40.2 |
| design the admission ladder | §23.3 (8 steps) · §40.3 · §40.4 |
| handle a stale socket file safely | §24.1 · §24.2 |
| set runtime-directory and socket permissions | §24.0 · §40.1 |
| decide what PID can and cannot prove | §23.4 · §40.2 |
| write peer-credential tests | §39.7 |
| redact what an error may reveal | §40.7 · §16.5 · §17.4 |
| bound hostile input | §40.5 · §20.0 |

### §3.5 Flow, limits, and lifetime

| Goal | Go to |
|---|---|
| stop a slow consumer growing daemon memory | §19.0 · §19.1 · §19.2 · §19.5 |
| carry backpressure from the execution engine | §19.3 |
| pick message-size ceilings | §20.0 · §20.1 · §20.2 · §20.3 |
| decide what to do when a payload is too big | §20.6 · §11.5 |
| decide whether to compress | §20.4 · §45.6 |
| wire a cancellation tree | §27.0 · §27.1 · §27.3 |
| make cancellation survive a broken stream | §18.4 · §27.3 · §36.7 |
| reserve time for cleanup | §18.3 · §18.6 |
| shut down in the right order | §37.0 · §37.1 · §27.5 |
| reconnect and resume | §37.4 · §36.6 · §21.4 |

### §3.6 Proving and operating it

| Goal | Go to |
|---|---|
| prove Rust and Python were generated from the same schema | §35.1 · §34.4 · §5.2 |
| fingerprint the schema | §5.1 · §34.1 · §34.2 |
| assert the descriptor at build time | §34.3 · §31.4 · §39.2 |
| build the wire fixture matrix | §35.2 · §39.3 |
| test one RPC properly | §39.4 |
| test streaming and resume | §39.5 |
| test message-size boundaries | §39.6 |
| choose fuzz targets | §39.8 |
| judge whether an acceptance test is worth anything | §39.10 |
| measure latency and find the real cost | §38.2 · §38.3 · §38.5 |
| decide what to tune, in what order | §38.0 · §38.8 |
| plan a dependency upgrade | §43.0 · §43.1 · §43.2 · §43.3 |
| decide whether to adopt an optional crate | §41 · §42.2 |

---

## §4 — Decision trees

### Which chapter answers this?

```
I am deciding what the contract should say
  -> 0.1 four planes, 0.3 authority, 15 shapes, 16.0 placement
I am running the compiler or the schema workflow
  -> 3 protoc, 4 Buf, 5 pipeline
I am writing build.rs or reading generated code
  -> 6 tonic-prost-build, 7 prost-build, 8 anatomy
I am holding a message and asking what it does at runtime
  -> 9 prost, 10 prost-types, 11 bytes
I am writing a handler, a client, or a stream
  -> 12 object model, 13 client, 14 server, 19 streaming
I am putting it on a socket
  -> 21 Endpoint/Channel, 22 client UDS, 23 server UDS, 24 socket lifecycle
I am deciding who may call
  -> 23.2-23.4 peer credentials, 40 hardening
I am bounding something
  -> 18 deadlines, 19 backpressure, 20 sizes, 28 admission, 29 HTTP/2
I am ending something
  -> 27 cancellation, 37 shutdown/resume
I am proving it
  -> 34 descriptors, 35 interop, 39 tests
I am choosing rather than implementing
  -> 45 the ten matrices; 44 the 52 anti-patterns
```

### Which method shape?

```
One request, one bounded answer                 -> unary-unary        (15.0, 15.1)
One request, many events over time              -> unary-stream       (15.0, 36.5)
Client sends the bulk, server answers once      -> stream-unary       (15.0)
Genuinely interactive full duplex               -> stream-stream      (15.0)

Tempted by bidi to make cancel/progress symmetrical?
  -> NO. accepted handle + event stream + explicit Cancel/Attach     (15.1)
Long work behind a unary call?
  -> return the accepted handle first, then work                     (15.2, 36.4)
```

### Transport failure, or a record inside a successful response?

```
Outer RPC envelope malformed / auth / incompatible / unavailable
  -> tonic::Status, code from the table                             (17.0, 12.4)
One semantic query block failed, others succeeded, and the released
response contract represents that
  -> NOT a Status. A record in the canonical response.              (17.1, 45.8)
Need machine-readable structure on a real Status
  -> tonic-types richer details, allowlisted                        (17.2, 17.3, 17.4)
Tempted to duplicate the whole semantic error model into details
  -> NO                                                             (17.3)
Client wants to branch on the failure
  -> code and/or typed detail. NEVER string-match the message.      (17.0)
```

### Field, metadata, or extension?

```
Part of the durable released contract, visible to codegen
  -> Protobuf field                                                 (16.0)
Per-call transport context: trace parent, capability credential
  -> gRPC metadata, kept small (rides HTTP/2 headers)               (16.0, 16.1, 29.6)
Verified, typed, process-local: principal, peer UID/PID
  -> Request extension, inserted once at admission                  (16.0, 12.2, 16.3)
Large, or the query payload itself
  -> Protobuf field. Never metadata.                                (16.0)
Re-parsing raw metadata in every method?
  -> NO. Insert a typed extension once.                             (12.2)
```

### The payload is too big — what do I change?

```
FIRST, not last: is the limit the problem, or the payload?          (20.6)
  chunk it                     -> 20.0 separate limits, 19.4 item identity
  externalize it               -> 11.5 immutable resource + range reads
  range-read it                -> 36.8
  fix the schema               -> 20.6
  find duplicated data         -> 20.6
  find leaked debug metadata   -> 20.6
Only then set explicit ceilings, above the largest valid envelope,
never equal to the logical payload size                             (20.1, 20.2)
And set the matching Python ceilings                                (20.3)

Note: the 4 MiB Tonic decode default is an implementation default,
not your application policy.                                        (20.1, 13.3)
```

### Which cancellation or deadline primitive?

```
Caller's absolute completion budget       -> gRPC deadline           (18.0, 18.1)
Propagating it from a Rust client         -> Request::set_timeout    (18.2)
A local stage bound                       -> Tokio timeout / budget  (45.4)
Stopping the accepted logical operation   -> CancelQuery             (18.4, 36.7)
Fanning that out inside the daemon        -> CancellationToken tree  (27.0, 27.1)
Transport-level stop                      -> stream cancel/drop      (18.4)
Coming back after a broken stream         -> AttachQuery + cursor    (37.4, 36.6)

Dropping the response future is enough?
  -> NO. Spawned work needs an explicit signal.                      (18.5)
Give the inner work the whole outer deadline?
  -> NO. Reserve cleanup budget.                                     (18.3, 18.6)
```

### Tower layer, Tonic knob, or neither?

```
Bounding concurrent heavy work
  -> NEITHER. Daemon-owned query admission.                          (28.2)
Ceiling on transport abuse per connection
  -> Tonic concurrency_limit_per_connection                          (28.3)
Rejecting instead of queueing at a protection boundary
  -> Tower load shed / Tonic load_shed, only if deliberate           (28.4)
Adding a queue in front of the service
  -> AVOID. The daemon queue is already bounded.                     (28.5, 45.5)
A hard local safety timeout
  -> Tower timeout, coordinated with the gRPC deadline               (28.6, 18.0)
Retrying
  -> NOT generically. Per-method table.                              (28.7)
High-rate tiny control calls from an abusive caller
  -> rate limit, kept separate from query admission                  (28.8)

Layer order is semantic, not cosmetic -- document the stack.         (28.1)
```

### Is this UDS caller allowed?

```
1. socket parent directory owned by the expected OS user             (23.3, 24.0)
2. socket mode/ownership verified                                    (23.3, 40.1)
3. peer UID/GID from kernel credentials                              (23.3, 23.2)
4. peer PID where supported/required                                 (23.3, 23.4)
5. short-lived capability credential validated                       (23.3, 40.3)
6. credential binds agent+workspace+operation+expiry+anti-replay     (23.3, 40.3)
7. typed principal inserted into request extensions                  (23.3, 16.3)
8. each sensitive RPC reauthorizes                                   (23.3, 40.4)

Socket pathname as authorization?          -> NO                     (23.3)
Opaque artifact ID as bearer authority?    -> NO                     (23.3, 40.4)
Caller-supplied PID?                       -> NO                     (40.2)
PID alone as a principal?                  -> NO, PIDs are reused    (23.4)
Same-user process is harmless?             -> NO                     (24.4)
```

### This document, or the Python sibling?

```
Rust daemon: tonic/prost/tokio/tower APIs, UDS server, build.rs,
Rust generated code, Rust-side limits and cancellation
  -> this document

Python adapter: grpc.aio channels and stubs, Python interceptors,
_pb2 anatomy, Python-side deadlines and options, orjson
  -> sibling grpcio-orjson-protobuf-ref

.proto language semantics, presence, wire format, ProtoJSON,
descriptor pools, schema evolution rules as language-neutral topics
  -> sibling grpcio-orjson-protobuf-ref (protobuf reference)

The joint invariants between the two -- matching generation inputs,
descriptor identity, unknown-field asymmetry, bilateral fixtures
  -> HERE, 34 and 35. Do not reinvent them on the Python side.
```

### Adopt this optional crate?

```
tonic-health        -> optional. Handshake stays the readiness authority.  (30.1, 30.3)
tonic-reflection    -> dev/CI yes; production private UDS off by default.  (31.2, 45.7)
tonic-types         -> yes where a real Status needs machine-readable
                       structure; not to mirror the semantic error model.  (17.3)
prost-reflect       -> tooling and contract checks only, never normal
                       RPC dispatch.                                       (32.2, 32.4)
pbjson*             -> only for control ProtoJSON; never as an implicit
                       canonical-JSON authority.                           (33.2)
protoc-bin-vendored -> only when a constrained build proves the need.      (41.0)
rust-protobuf       -> no, without a specific interop requirement.         (41.1, 1.5)
hyper / h2 / http   -> no direct dependency to "tune gRPC".                (41.2)
serde/bincode inside gRPC -> no. Opaque fields may carry other formats;
                       the outer RPC stays Protobuf.                       (41.3)
tonic-web           -> irrelevant to a private UDS boundary.               (41.4)
TLS crates          -> not on local UDS; a remote profile is a separate
                       deployment profile.                                 (41.5)
official grpc Rust  -> watch, do not migrate. 8 named triggers.            (42.0, 42.2)
```

---

## §5 — Navigation rules

Rules 1-6 are about finding things; 7-10 about reading them; 11-14 about trusting what you find.

1. **Chapters are `# N)` with a closing paren, not a dot.** The chapter regex is `^# (\d+)\) `;
   subsections are `^## (\d+)\.(\d+) `. A `^# N\.` pattern matches nothing in this file.

2. **A bare `rg '^# '` is wrong by eight.** It reports 59 top-level headings; the fence-aware count
   is **51**. All eight decoys are TOML comments inside one `Cargo.toml` fence in §2.1 — lines 350,
   353, 354, 355, 356, 360, 362, 363. The `##` and `###` counts are unaffected (298 and 4, raw and
   fence-aware agree). Use `just lib-outline`, which parses markdown properly.

3. **`just lib-outline`, never `just spec-outline`.** The two navigators refuse each other's trees:
   `lib-outline` is h1-rooted for `docs/library_ref/`, `spec-outline` is h2-rooted for
   `docs/authoritative_design/`. Zoom with
   `--view expanded --match '^12\)'` — note the match is against the chapter title, so the paren
   belongs in the pattern.

4. **Symbol citations in §2 were checked by subsection range, not by whole-file grep.** A name can
   appear in a dozen fences; only one subsection explains it. If you extend §2, check membership
   inside the cited range or the row is worthless.

5. **§44, §46 and §47 do not use `## N.M`.** `--view expanded` shows unnumbered `##` headings for
   §44 (7 buckets) and §47 (8 groups), and shows only `## 46.1` for §46. Section-number greps
   against those three chapters return nothing. §1.2 is their index.

6. **§46's checklist is inside a code fence.** ~104 `[ ]` items under 14 ALL-CAPS block headers, all
   invisible to every heading tool and to `^#` greps. The same is true of the terminal
   `## Final architecture compression` fence (3775-3837). §1.2 gives the line numbers.

7. **Read whole chapters.** Median chapter is 66 lines and only six exceed 100. Windowing into a
   66-line chapter costs more than reading it, and these chapters are written to be read in order —
   the rule usually lands in the last subsection.

8. **Seek by line, cite by section.** Line numbers live only in §1. Cite `§N.M` everywhere else,
   because regeneration moves lines and not section numbers.

9. **Do not copy a version out of this document into a tracked file.** §45.10 and the §2.1 manifest
   are the *document's* anchors — what it was written against. This repository's pins come from
   `FAB §2.1` and the session context. They are not always the same: the document anchors
   `tokio-stream` 0.1.19 while the repository pins `=0.1.18`.

10. **The document ranks itself below the pinned crate.** §30.0 says to verify the health helper
    against the pinned 0.14.6 API; §31.1 says the same for the reflection builder methods; §34.1
    says to pin and verify Buf flag spellings. Treat every code fence as a shape, not a signature.

11. **Zero grep hits is not zero coverage.** Nineteen names the document never writes are listed in
    §2.11 — all six Tower `*Layer` types among them. Route by concept before concluding a topic is
    missing.

12. **Only §45 is tabular.** 108 of the document's 180 table rows are in that one chapter; §15.0,
    §16.0, §17.0 and §46.1 hold the rest. Everywhere else the answer is prose or a `text` fence, so
    a table-shaped search strategy will miss most of the content.

13. **Python-side questions route to `grpcio-orjson-protobuf-ref`.** This document deliberately does
    not re-document `grpcio` or Python `protobuf` (§35.0); it covers only the joint invariants in
    §34 and §35. The FastMCP surface itself is `fastmcp-pydantic-ref`.

14. **Evidence over restatement.** If this file and a cited section disagree, open the section; if
    the disagreement survives, the document wins and this file needs a fix — flag it.
