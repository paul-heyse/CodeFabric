# Rust gRPC / Protobuf Daemon Stack — advanced technical reference / feature-category catalog

This reference is designed for **LLM coding agents and engineers building a production Rust daemon behind a Python/FastMCP adapter**, with special emphasis on a local, authenticated gRPC boundary over Unix-domain sockets (UDS). It is a companion to the existing `grpcio` Python and `protobuf` Python advanced references: it does not re-document those Python libraries except where cross-language contract behavior materially affects the Rust implementation.

The target architecture is:

```text
programming agent
  <-> FastMCP 3.4.7 STDIO adapter (Python)
       <-> grpc.aio + generated Python bindings
            <-> authenticated private gRPC / HTTP/2 over Unix-domain socket
                 <-> Tonic / Tokio Rust daemon
                      -> released Protobuf control contract
                      -> canonical JSON semantic payloads where specified
                      -> Arrow IPC payloads where negotiated
                      -> DataFusion / Arrow / Delta / domain services
```

The central design rule throughout this document is that the RPC stack is a **control, transport, lifecycle, compatibility, and flow-control boundary**. It must not become a second semantic implementation of the daemon's canonical query/data model.

---

## Version / source anchors

**Research date: 2026-08-30.**

Primary deployable Rust targets used in this reference:

| Component | Target | Role |
|---|---:|---|
| `tonic` | **0.14.6** | production Rust gRPC / HTTP/2 client-server runtime |
| `tonic-prost` | **0.14.6** | Prost codec integration for Tonic |
| `tonic-prost-build` | **0.14.6** | Protobuf + Prost + Tonic code-generation integration |
| `tonic-build` | **0.14.6** | generic Tonic service-code generator; normally indirect when using Protobuf |
| `prost` | **0.14.4** | Rust Protocol Buffers message runtime |
| `prost-build` | **0.14.4** | Rust Protobuf code generation and mapping configuration |
| `prost-types` | **0.14.4** | Protobuf well-known / descriptor types |
| `bytes` | **1.12.1** | reference-counted and mutable byte buffers |
| `tokio` | **1.53.1** | async runtime and Unix socket I/O |
| `tokio-stream` | **0.1.19** | stream adapters including `UnixListenerStream` |
| `tokio-util` | **0.7.19** | cancellation and Tokio utilities |
| `tower` | **0.5.3** | middleware / `Service` / admission primitives used by Tonic |
| `tonic-health` | **0.14.6** | standard `grpc.health.v1` service |
| `tonic-reflection` | **0.14.6** | gRPC server reflection |
| `tonic-types` | **0.14.6** | Google RPC richer error/status types for Tonic |
| `prost-reflect` | **0.16.5** | optional descriptors + dynamic messages / reflection |
| `pbjson` | **0.9.0** | optional Protobuf JSON mapping support for Prost types |
| `pbjson-build` | **0.9.0** | optional serde-generation for ProtoJSON-style mapping |
| `pbjson-types` | **0.9.0** | optional WKT JSON helpers; evaluate type coverage before relying on it |
| `protoc` | **36.x** | active Protocol Buffers compiler release line |
| Buf CLI | **v1.x stable line** | schema build/lint/format/breaking/codegen governance |

Two version boundaries deserve explicit attention:

1. **Tonic 0.14 changed the code-generation topology.** `tonic-build` is now the generic service generator; Protobuf/Prost integration belongs in `tonic-prost` and `tonic-prost-build`. New code should not copy older `tonic_build::compile_protos(...)` examples without checking the 0.14 API.
2. **Protobuf language/runtime versions are not one numeric sequence.** `protoc` is currently on the 36.x release line; the companion Python runtime is on 7.36.x. Rust Prost has its own independent crate version 0.14.x.

Tonic 0.14.6 declares **MSRV Rust 1.88**. Prost 0.14.4 declares an older MSRV, but a daemon using current Tonic should treat **Rust 1.88 as the effective minimum** unless another project dependency requires newer.

### Primary source families

Use these source families in this precedence order when APIs or behavior disagree:

1. `docs.rs` pages for the exact crate release used in production.
2. Tagged crate source / changelogs for implementation details and feature gating.
3. gRPC protocol documentation for cross-language RPC semantics.
4. Protocol Buffers documentation for schema/compiler compatibility.
5. Buf documentation for repository-level schema workflow.
6. Current GitHub `main` only for roadmap/future work, never as silent production API guidance.

---

# Feature inventory: what this reference covers

The combined Rust daemon communication stack is best understood as several tightly integrated layers:

```text
.proto sources
  -> Buf governance + protoc / FileDescriptorSet
      -> prost-build message generation
      -> tonic-prost-build service generation
          -> prost generated messages
          -> tonic generated client/server surfaces
              -> tonic-prost codec
                  -> tonic transport / HTTP/2
                      -> Tokio runtime + UnixStream/UnixListener
                          -> Tower service/middleware layers
                              -> daemon application services
```

The reference covers:

- package selection, features, version pinning, MSRV, and dependency ownership;
- `protoc` 36.x, descriptor sets, Editions, and compiler/runtime separation;
- Buf build/lint/format/breaking/generate workflows;
- `tonic-prost-build`, `prost-build`, generated client/server code, and descriptor embedding;
- Prost message semantics relevant to RPC boundaries;
- `bytes::Bytes` and selective `bytes` field generation for high-volume payloads;
- Tonic requests/responses/metadata/extensions/interceptors/status;
- unary and streaming cardinalities;
- deadlines, `grpc-timeout`, cancellation, resume semantics, and cleanup;
- stream backpressure and bounded buffering;
- message-size limits, chunking, compression, HTTP/2 flow control, keepalive, and connection lifetime;
- Tonic `Endpoint`, `Channel`, `Server`, custom connectors/incoming streams, and UDS operation;
- Tokio `UnixStream`/`UnixListener`, `UCred`, peer-process verification, socket permissions and lifecycle;
- `tokio-stream` and `tokio-util::CancellationToken`;
- Tower concurrency limits, load shedding, timeout/retry/buffer risks, and method-aware admission;
- `tonic-health`, `tonic-reflection`, `tonic-types`, `prost-reflect`, and optional `pbjson` tooling;
- descriptor fingerprints and cross-language wire compatibility;
- Rust↔Python `grpc.aio` interoperability;
- performance baselines and tuning order for a local UDS deployment;
- graceful shutdown, reconnect/resume, leases, result reads, and daemon supervision;
- security and denial-of-service boundaries;
- tests and executable acceptance oracles;
- the emerging official `grpc` Rust crate and why it is not yet the production recommendation;
- dense decision matrices, anti-patterns, and agent implementation rules.

---

# Proposed comprehensive documentation map

0. Scope, versioning, and architectural mental model  
1. Dependency topology and package selection  
2. Recommended Cargo feature/pinning posture  
3. `protoc` 36.x — compiler role and compatibility  
4. Buf CLI — schema governance and repository workflow  
5. Canonical `.proto` → descriptor → Rust/Python generation pipeline  
6. `tonic-prost-build` — service + message integration  
7. `prost-build` — Rust message mapping controls  
8. Generated Rust code anatomy and module inclusion  
9. `prost` runtime — message contract and hot-path semantics  
10. `prost-types` and well-known/descriptor types  
11. `bytes` — payload ownership, `Bytes`, and selective zero-copy-oriented mapping  
12. `tonic` object model — Request/Response/Status/Streaming/Extensions  
13. Generated Tonic client fundamentals  
14. Generated Tonic server fundamentals  
15. RPC cardinalities and method-shape selection  
16. Metadata, extensions, interceptors, and boundary context  
17. Status codes and `tonic-types` richer errors  
18. Deadlines, `grpc-timeout`, timeout budgets, and cancellation  
19. Streaming, flow control, and backpressure  
20. Message sizing, chunking, compression, and payload policy  
21. `Endpoint`, `Channel`, and connection lifecycle  
22. Unix-domain-socket client integration  
23. Unix-domain-socket server integration and peer credentials  
24. Socket filesystem lifecycle, process identity, and local security  
25. Tokio runtime design for the daemon  
26. `tokio-stream` adapters  
27. `tokio-util::CancellationToken` and structured cancellation  
28. Tower middleware, admission, load shedding, buffers, timeout, retry  
29. HTTP/2 flow windows, concurrent streams, keepalive, frame/header settings  
30. `tonic-health`  
31. `tonic-reflection` and descriptor embedding  
32. `prost-reflect` and dynamic schema tooling  
33. `pbjson` / `pbjson-build` / `pbjson-types`  
34. Descriptor fingerprints, schema authority, and build reproducibility  
35. Cross-language Python↔Rust interoperability  
36. Mapping the stack to the FastMCP / daemon service lifecycle  
37. Graceful shutdown, reconnect, resume, and accepted-handle semantics  
38. Performance engineering and recommended baseline configuration  
39. Testing, fuzzing, compatibility, and executable contract checks  
40. Security hardening  
41. Optional/alternative libraries and what not to adopt by default  
42. Official `grpc` Rust preview: watch, do not migrate production yet  
43. Upgrade and compatibility discipline  
44. Anti-pattern inventory  
45. Dense API / dependency / decision matrices  
46. Agent implementation checklist  
47. Source index

---

# 0) Scope, versioning, and architectural mental model

## 0.0 Scope

This document addresses the **communication protocol and daemon-operating substrate**. It intentionally excludes generic engineering tools unless they directly shape the runtime semantics of the gRPC/UDS boundary.

Included examples:

- Protobuf compiler and schema governance;
- Rust message/RPC code generation;
- gRPC transport/runtime;
- Tokio UDS operation;
- peer-credential extraction;
- flow control / backpressure;
- timeout/cancellation semantics;
- Tower middleware that directly wraps gRPC services;
- health/reflection/richer-status protocol extensions;
- dynamic descriptor tooling needed for contract checks.

Excluded from core coverage:

- generic log formatting libraries;
- general benchmark runners;
- generic database clients;
- generic CLI frameworks;
- unrelated async utility crates;
- application/business-domain libraries.

## 0.1 The four contract planes

For the target FastMCP/daemon architecture, keep four representations conceptually separate:

```text
control plane
    released Protobuf RPC messages

semantic request/response plane
    released canonical JSON profile, plus typed result relations where negotiated

bulk/provider data plane
    Arrow IPC or other explicitly negotiated binary payloads under RPC framing

agent presentation plane
    FastMCP / Pydantic / MCP JSON-RPC
```

The most common architecture error is allowing the control-plane Protobuf contract to expand into a duplicate semantic DTO model. Protobuf should strongly type **authentication, compatibility, handles, progress, flow-control metadata, cancellation, leases, result descriptors, terminal state, and bounded transport envelopes** while semantic facts remain owned by the daemon's canonical representation.

## 0.2 One process-local RPC channel, not a service mesh

This profile is unusually favorable to simple gRPC configuration:

```text
one FastMCP process
    -> one long-lived grpc.aio channel
        -> one UDS endpoint
            -> one local Rust daemon
```

Consequences:

- no DNS/name resolver is needed in the steady-state path;
- no client-side load balancer is needed;
- TLS is usually unnecessary if the local threat model is satisfied by UDS ACL + kernel peer credentials + application credential;
- keepalive is usually unnecessary;
- connection pools are not useful;
- HTTP/2 bandwidth-delay-product tuning is much less important than on WAN links;
- message copies, serialization, scheduling, stream backpressure, cancellation, and admission control dominate the useful tuning surface.

## 0.3 Contract authority hierarchy

Recommended authority model:

```text
.proto source + compatibility policy       AUTHORITY
        |
        +-> FileDescriptorSet             derived compatibility artifact
        +-> Rust generated source         derived cache
        +-> Python generated source       derived cache
        +-> reflection descriptor         derived runtime artifact
```

Generated source must never become the place where contract changes are made manually.

## 0.4 Agent rule

When an implementation decision affects both languages, prefer the **wire contract or descriptor** as the source of truth. Do not infer cross-language compatibility from Rust type equality or Python generated class names alone.

---

# 1) Dependency topology and package selection

## 1.1 Production core

For this architecture the minimal production Rust-side set is:

```text
tonic
  + tonic-prost
  + prost
  + bytes
  + tokio
  + tokio-stream   (UDS listener -> Stream integration)
  + tokio-util     (structured cancellation)
```

`tower` becomes a direct dependency if application code intentionally uses Tower layers or `ServiceBuilder`; Tonic itself already uses Tower internally.

`prost-types` is needed when the schema uses Protobuf well-known/descriptor types or when the application decodes descriptor sets with Prost.

## 1.2 Build-only core

```text
tonic-prost-build
  + prost-build    (direct only when your build.rs config references it)
  + protoc 36.x    (system/build tool, unless descriptor set supplied externally)
```

`tonic-build` is normally an **indirect/internal build dependency** for Protobuf users. Add it directly only when you use its lower-level generic service-generation APIs.

## 1.3 Protocol-adjacent optional packages

```text
tonic-health       standard gRPC health protocol
tonic-reflection   standard gRPC reflection protocol
tonic-types        google.rpc richer status details
prost-reflect      dynamic messages / descriptor introspection
pbjson*            optional ProtoJSON-style serde mapping for Prost messages
```

## 1.4 Toolchain packages, not application libraries

```text
protoc             canonical Google Protocol Buffers compiler
Buf CLI            schema build/lint/format/breaking/generation workflow
```

These belong in development/CI tooling and should not be runtime daemon dependencies.

## 1.5 What not to mix casually

Do not mix both Rust Protobuf ecosystems (`prost` and `rust-protobuf`) in one RPC surface without a deliberate interoperability reason. Tonic 0.14's supported production Protobuf path is Prost-oriented; adding a second generated-message ecosystem generally increases conversion, build, and descriptor complexity without improving the boundary.

---

# 2) Recommended Cargo feature and pinning posture

## 2.1 Representative production manifest

For an application repository with a committed `Cargo.lock`, an explicit dependency surface can be:

```toml
[dependencies]
tonic = { version = "0.14.6", features = ["transport"] }
tonic-prost = "0.14.6"
prost = "0.14.4"
prost-types = "0.14.4"
bytes = "1.12.1"

tokio = { version = "1.53.1", features = [
  "macros",
  "rt-multi-thread",
  "net",
  "signal",
  "sync",
  "time",
] }
tokio-stream = { version = "0.1.19", features = ["net"] }
tokio-util = { version = "0.7.19", features = ["rt"] }

tower = { version = "0.5.3", features = [
  "limit",
  "load-shed",
  "timeout",
  "util",
] }

# Optional protocol/ops extensions
tonic-health = "0.14.6"
tonic-types = "0.14.6"
# tonic-reflection = "0.14.6"
# prost-reflect = "0.16.5"
# pbjson = "0.9.0"
# pbjson-types = "0.9.0"

[build-dependencies]
tonic-prost-build = "0.14.6"
# Add directly if using prost_build::Config in build.rs:
prost-build = "0.14.4"
# Optional ProtoJSON generation:
# pbjson-build = "0.9.0"
```

For an application, **the lockfile is the exact dependency resolution contract**. A `Cargo.toml` semver declaration plus committed `Cargo.lock` is usually preferable to hard `=x.y.z` syntax everywhere, because it preserves ordinary Cargo ecosystem behavior while still making production resolution reproducible.

## 2.2 Feature minimization

Do not use `tokio = { features = ["full"] }` or `tower = { features = ["full"] }` merely for convenience in a production daemon unless the project actually needs all of those facilities. The communication stack needs a fairly narrow subset.

For `tokio-util`, note that **no features are enabled by default**. `CancellationToken` support belongs behind the `rt` feature.

## 2.3 Tonic compression/TLS features

Tonic 0.14 has optional feature gates for compression (gzip, deflate, zstd) and TLS backends. Do not enable them by default in a local UDS profile. Each adds code and, for compression/TLS, runtime cost and policy surface.

## 2.4 MSRV

Treat Tonic's Rust 1.88 requirement as the effective floor for the combined daemon transport stack. Keep the workspace's `rust-version` explicit so CI and developer environments fail predictably rather than discovering the requirement indirectly during dependency resolution.

---

# 3) `protoc` 36.x — compiler role and compatibility

## 3.0 What `protoc` owns

`protoc` parses `.proto` source, resolves imports and language features, constructs descriptor graphs, validates the schema, and invokes language/service plugins or emits descriptor sets. It is a **build-time compiler**, not a Rust runtime library.

The active compiler release line as of this reference is **36.x**. Protobuf Edition 2026 requires at least `protoc 36.0`.

## 3.1 Compiler vs language runtimes

Do not conflate:

```text
protoc 36.x                   compiler / repository release line
protobuf Python 7.36.x        Python runtime line
prost 0.14.x                  Rust runtime / generator ecosystem
```

Each has an independent package/versioning model.

## 3.2 Compiler discovery

`prost-build` can invoke `protoc` automatically. Production build reproducibility improves if the compiler path/version is explicit rather than inherited from whichever `protoc` appears first on a workstation `PATH`.

Typical strategies:

1. CI image contains an exact `protoc 36.x` binary.
2. Build environment exports a known `PROTOC` path.
3. Build uses a pre-generated `FileDescriptorSet` and tells Prost to skip running `protoc`.
4. A vendored-protoc crate is used only when distributing a compiler with the Rust build is an intentional tradeoff.

## 3.3 Descriptor-set generation

A descriptor set is a high-value contract artifact:

```bash
protoc \
  -I proto \
  --include_imports \
  --descriptor_set_out=target/proto/codefabric.binpb \
  proto/codefabric/cpgd/v1/cpg_query.proto
```

The exact command/flags should be owned by build tooling rather than copied ad hoc into developer instructions.

## 3.4 Edition policy

Do not enable Edition 2026 merely because `protoc 36.x` supports it. An Edition change alters the schema-language feature model and can affect code generators. Adopt it only after Rust Prost, Python protobuf, gRPC codegen, Buf checks, and old/new cross-language fixtures are proven for the intended feature set.

## 3.5 Agent rules

- Record `protoc --version` in build diagnostics.
- Never generate production Rust and Python bindings with accidentally different schema inputs.
- Never infer schema compatibility from successful compiler output alone.
- Treat `FileDescriptorSet` as a useful derived fingerprintable artifact, not as the editable authority.

---

# 4) Buf CLI — schema governance and repository workflow

## 4.0 Why Buf is worth making first-class

Buf does not replace the Protobuf wire format or generated code runtime. It replaces the fragile **repository workflow around `protoc`** with one module-aware toolchain for:

```text
build
format
lint
breaking-change detection
dependency resolution
code generation
descriptor/image production
schema distribution (optional BSR)
```

For a cross-language Rust/Python daemon contract, this is especially valuable because the schema must remain identical and generated artifacts must not drift.

## 4.1 `buf.yaml`

Representative workspace:

```yaml
version: v2
modules:
  - path: proto
lint:
  use:
    - STANDARD
breaking:
  use:
    - FILE
```

For a released RPC surface, evaluate a stricter breaking-change policy than the default if package/service-level compatibility is central. The exact Buf rule category should be frozen as part of your contract policy.

## 4.2 Build

```bash
buf build
```

A successful build proves the module compiles under Buf's Protobuf compiler/model. For a plain standard descriptor set:

```bash
buf build --as-file-descriptor-set -o target/proto/codefabric.binpb
```

This creates an artifact that can be consumed by Prost/Tonic build steps, reflection, fingerprinting, or compatibility tests.

## 4.3 Format and lint

```bash
buf format -w
buf lint
```

Use formatting as a deterministic source-style step and linting as a schema-quality gate. Do not let an LLM "clean up" field numbers or package names just to satisfy cosmetic intuitions; compatibility rules take precedence.

## 4.4 Breaking-change detection

Typical CI form:

```bash
buf breaking --against '.git#branch=main'
```

Other comparison inputs can include Buf images, Git repositories, and BSR modules. Keep the baseline explicit and reproducible.

## 4.5 `buf.gen.yaml`

Buf can drive code-generation plugins through one declarative file. This is attractive when the same schema generates Rust and Python outputs and you want plugin/version configuration in one place.

However, Rust's `tonic-prost-build` also has valuable Rust-specific mapping APIs such as selective `bytes::Bytes`, attributes, and `extern_path`. Therefore there are two viable generation architectures:

### Architecture A — Buf governs schema; Rust `build.rs` generates Rust

```text
buf build/lint/breaking
   -> .proto authority
      -> Rust build.rs / tonic-prost-build
      -> Python grpc_tools / protoc generation
```

Simple and familiar, but two generation commands must be proven to consume identical sources/toolchain versions.

### Architecture B — Buf emits canonical descriptor; generators consume descriptor

```text
buf build --as-file-descriptor-set
   -> canonical FileDescriptorSet
      -> Rust tonic-prost-build / prost-build skip_protoc_run
      -> Python descriptor/codegen pathway as supported by build system
```

This can make the descriptor graph itself the common compiler output, reducing divergent include/import resolution.

### Architecture C — Buf drives all generation plugins

Best when every required plugin and Rust mapping option can be represented cleanly in `buf.gen.yaml`.

For this daemon, **A or B** is typically easiest while preserving the full Prost mapping surface.

## 4.6 BSR

The Buf Schema Registry is optional. A local monorepo does not need BSR to benefit from Buf build/lint/breaking/generate. Introduce BSR only if schema publication, remote module dependencies, generated SDK distribution, or organization-level governance creates material value.

---

# 5) Canonical `.proto` → descriptor → Rust/Python generation pipeline

## 5.0 Recommended authority flow

```text
proto/codefabric/cpgd/v1/*.proto
        |
        +-- buf format
        +-- buf lint
        +-- buf breaking
        +-- buf build -> FileDescriptorSet
        |
        +-- Rust codegen
        |     tonic-prost-build + prost-build
        |
        +-- Python codegen
              grpcio-tools/protoc
```

The build should prove that all branches derive from the same released schema revision.

## 5.1 Descriptor fingerprint

A stable schema fingerprint should be computed over an explicitly normalized contract artifact. Recommended approach:

1. Build the exact descriptor set from the released source graph.
2. Decide whether source-code info belongs in the fingerprint; usually **exclude it** so comments/locations do not alter wire-contract identity.
3. Preserve a deterministic file ordering policy.
4. Hash the serialized descriptor set with a documented hash algorithm.
5. Store the fingerprint in release metadata/Handshake compatibility data.

Do not hash generated Rust/Python source as the canonical schema identity; codegen formatting can change without a wire-contract change.

## 5.2 Generated-code drift gate

CI should regenerate and fail on diff:

```text
schema changes
  -> regenerate Rust
  -> regenerate Python
  -> compare committed generated cache if one is intentionally retained
  -> run cross-language interop
```

If the project does **not** commit generated Rust output because Cargo generates into `OUT_DIR`, compare descriptor fingerprints and compiled API tests instead. Generated-cache policy should be explicit rather than accidental.

## 5.3 Source distribution caveat

If source distributions/wheels or constrained foreign build environments cannot run compilers, committed/generated Python cache files may be necessary. Treat them as derived release artifacts and validate their descriptors against the authoritative schema during packaging.

---

# 6) `tonic-prost-build` — service + message integration

## 6.0 Role

`tonic-prost-build` is the normal Tonic 0.14 entry point when `.proto` services should become:

```text
Prost message structs/enums
+ Tonic client modules
+ Tonic server traits/routers
+ tonic-prost codec bindings
```

It integrates `prost-build` message generation with Tonic service generation.

## 6.1 Minimal build script

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::compile_protos("proto/codefabric/cpgd/v1/cpg_query.proto")?;
    Ok(())
}
```

For production, prefer `configure()` so mapping, descriptors, rerun behavior, and output paths are deliberate.

## 6.2 Builder

Representative:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_client(true)
        .build_server(true)
        .file_descriptor_set_path("target/proto/codefabric.binpb")
        .compile_protos(
            &["proto/codefabric/cpgd/v1/cpg_query.proto"],
            &["proto"],
        )?;
    Ok(())
}
```

High-value builder options include:

- `build_client(bool)` — generate client code;
- `build_server(bool)` — generate server code;
- `build_transport(bool)` — include transport convenience APIs where applicable;
- `bytes(paths)` — pass selective bytes-field mapping through to Prost;
- `btree_map(paths)` — map selected maps to `BTreeMap`;
- `type_attribute`, `message_attribute`, `enum_attribute`, `field_attribute` — add Rust attributes;
- `extern_path(...)` — map Protobuf types/packages to externally generated Rust paths;
- `file_descriptor_set_path(...)` — emit or consume descriptor set;
- `skip_protoc_run()` — use an already-built descriptor set;
- `protoc_arg(...)` — pass compiler flags;
- `with_extended_rust_types(...)` — extended WKT mapping support;
- `codec_path(...)` — override codec path for specialized integration;
- `generate_default_stubs(...)` — generate default server stubs when desired;
- `use_arc_self(...)` — alter generated server trait ownership shape;
- `compile_fds(...)` / `compile_fds_with_config(...)` — generate directly from a `FileDescriptorSet`.

## 6.3 `compile_fds`

When Buf or another build step has already produced a canonical `FileDescriptorSet`, Tonic can generate from that descriptor graph rather than invoking `protoc` again.

Conceptually:

```rust
use prost_types::FileDescriptorSet;

let fds: FileDescriptorSet = /* decode build artifact */;
tonic_prost_build::compile_fds(fds)?;
```

For custom mapping, use the configurable builder or `compile_fds_with_config` path.

## 6.4 Why this matters for the daemon

A descriptor-driven build can make these identical by construction:

```text
Handshake schema fingerprint
reflection descriptor
Rust generated API
Python interop fixture schema
breaking-change baseline
```

That is stronger than separately trusting multiple generator invocations over a directory tree.

## 6.5 Do not use `tonic-build` as the old all-in-one API

Tonic 0.14 documentation explicitly directs Protobuf users toward `tonic-prost-build`. `tonic-build` remains useful for generic service code generation and is part of the implementation stack, but should not be the default user-facing compiler entry point for this design.

---

# 7) `prost-build` — Rust message mapping controls

## 7.0 Role

`prost-build` turns Protobuf message/enum definitions into idiomatic Rust types carrying `prost` derive metadata. It is the layer that decides many important **Rust representation choices** while preserving the same wire schema.

## 7.1 Custom `Config`

When using mappings beyond Tonic's convenience builder:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut prost = prost_build::Config::new();

    prost.bytes(&[
        ".codefabric.cpgd.v1.ResponseChunkEvent.payload",
        ".codefabric.cpgd.v1.ReadResultResponse.data",
    ]);

    tonic_prost_build::configure()
        .compile_with_config(
            prost,
            &["proto/codefabric/cpgd/v1/cpg_query.proto"],
            &["proto"],
        )?;

    Ok(())
}
```

## 7.2 `bytes(...)`

By default, protobuf `bytes` fields generate as `Vec<u8>`. `Config::bytes(...)` changes selected fields/packages/messages to `bytes::Bytes`.

This is one of the highest-value representation knobs for your specific design because the RPC carries:

- Arrow IPC fragments;
- response chunks;
- immutable artifact/range-read bytes;
- checksums/opaque binary tokens where applicable.

Use it selectively on **large or frequently forwarded byte payloads**. Do not map every tiny token/hash to `Bytes` simply because zero-copy sounds desirable.

## 7.3 `btree_map(...)`

Maps generate as `HashMap` by default under ordinary std settings. `btree_map` can select `BTreeMap` output. Use only when deterministic sorted iteration is genuinely useful to application code; protobuf map serialization order is not a durable semantic contract.

## 7.4 Attribute injection

`type_attribute`, `message_attribute`, `enum_attribute`, and `field_attribute` can add derives or policy attributes:

```rust
prost.type_attribute(
    ".codefabric.cpgd.v1.PublicStatus",
    "#[derive(serde::Serialize)]",
);
```

Avoid globally adding broad derives (`Serialize`, `Eq`, hashing, arbitrary validation) to every generated type without considering whether the semantics actually hold.

## 7.5 `extern_path`

Use `extern_path` when a shared Protobuf package/type is generated once in another Rust crate and should be referenced rather than duplicated. This is valuable in larger schemas with shared common types.

## 7.6 Descriptor path and `skip_protoc_run`

`file_descriptor_set_path(...)` can either capture the descriptor produced during normal codegen or, together with `skip_protoc_run()`, tell Prost to use a descriptor file produced externally.

This is the key integration point for a Buf-produced canonical descriptor set.

## 7.7 Include file

`include_file(...)` can generate a Rust include file that re-exports generated modules. This is useful for multi-file schemas and reduces error-prone handwritten module trees.

## 7.8 Source-info policy

Descriptor source info contains comments and source locations and can materially increase descriptor size. If descriptors are used only for runtime reflection/fingerprinting and documentation comments are not required, consider excluding source info in the canonical production descriptor artifact.

---

# 8) Generated Rust code anatomy and module inclusion

## 8.0 Message code

A message such as:

```proto
message ProgressEvent {
  uint64 sequence = 1;
  string phase = 2;
  bytes detail = 3;
}
```

will generate a Rust struct conceptually similar to:

```rust
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ProgressEvent {
    #[prost(uint64, tag = "1")]
    pub sequence: u64,
    #[prost(string, tag = "2")]
    pub phase: ::prost::alloc::string::String,
    #[prost(bytes = "bytes", tag = "3")]
    pub detail: ::prost::bytes::Bytes,
}
```

Exact generated spelling is generator-version dependent; do not depend on private formatting.

## 8.1 Service client module

A service produces a module such as:

```text
cpg_query_service_client
  -> CpgQueryServiceClient<T>
```

which wraps a generic gRPC service/transport and exposes typed async methods.

## 8.2 Service server module

Likewise:

```text
cpg_query_service_server
  -> CpgQueryService trait
  -> CpgQueryServiceServer<T>
```

Your application implements the generated trait; Tonic routes RPC method paths to it.

## 8.3 `tonic::include_proto!`

A common module layout:

```rust
pub mod cpgd {
    pub mod v1 {
        tonic::include_proto!("codefabric.cpgd.v1");
    }
}
```

The macro loads generated files from Cargo's build output (`OUT_DIR`).

## 8.4 `include_file_descriptor_set!`

If the build emits a named descriptor set into `OUT_DIR`, Tonic provides a macro to include those bytes at compile time. This is useful for reflection or runtime fingerprint/self-check logic without shipping separate `.proto` source files.

## 8.5 Boundary wrapper rule

Do not let generated types spread through every daemon subsystem merely because they are convenient. Prefer:

```text
Tonic handler
  -> validate/authorize transport contract
  -> convert to daemon-owned request/control type where useful
  -> run domain operation
  -> convert result/event envelope to generated Protobuf
```

For intentionally opaque canonical JSON/Arrow payload fields, no second semantic conversion layer is needed; validate the released payload contract and carry the bytes/object according to the design.

---

# 9) `prost` runtime — message contract and hot-path semantics

## 9.0 `prost::Message`

Generated message structs implement `prost::Message`. High-value methods include:

```text
encode
encode_to_vec
encode_length_delimited
encoded_len
merge
decode
decode_length_delimited
clear
```

The trait is designed around `bytes::Buf` / `BufMut`, allowing codecs to work with buffer abstractions rather than requiring every caller to hand-build a `Vec<u8>`.

## 9.1 Encoding

```rust
use prost::Message;

let bytes = message.encode_to_vec();
```

For the normal Tonic path, application code does not call this manually for every request; `tonic-prost` performs the codec operation.

## 9.2 Decoding

```rust
let event = ProgressEvent::decode(payload)?;
```

Again, Tonic normally performs this automatically at the RPC boundary.

## 9.3 `encoded_len()`

`encoded_len()` is useful for admission/testing metrics, but remember that the gRPC message has its own framing/HTTP2 overhead. A 4 MiB application payload can exceed a 4 MiB transport message limit after the Protobuf envelope is encoded.

## 9.4 Enums

Prost-generated enums expose integer representation and helpers. Unknown enum numeric values can remain representable at the field's integer layer depending on the generated shape; application logic must not assume every received number maps to a currently-known semantic variant.

## 9.5 Unknown fields: important cross-language caveat

Do **not** assume that Rust Prost decode→re-encode preserves arbitrary unknown fields the same way a full Python/C++ Protobuf runtime may. General unknown-field preservation has historically been a missing/ongoing Prost capability and remains an explicit project roadmap/work item around the 0.14 line.

For this daemon contract, the safe policy is:

```text
released Protobuf control messages
  -> endpoints understand declared fields
  -> compatibility negotiated explicitly
  -> never depend on a Rust intermediary preserving unknown additive fields by blind decode/re-encode
```

If transparent proxying/preservation of future fields becomes a requirement, test that exact behavior against the pinned Prost version or preserve the original encoded message bytes rather than assuming semantic round-trip preservation.

## 9.6 Semantic validation

Prost enforces Protobuf structural typing, not domain invariants. The daemon must still validate:

- field combinations;
- bounds beyond scalar ranges;
- query/lease state transitions;
- checksum/sequence invariants;
- authorization bindings;
- compatibility fingerprints;
- limits and resource class.

Do not add a second generic validation framework merely because Protobuf itself is not semantic validation; implement contract-owned checks at the boundary or domain layer that owns the invariant.

---

# 10) `prost-types` and well-known/descriptor types

## 10.0 Role

`prost-types` provides Rust representations for Protocol Buffers well-known and descriptor messages. High-value types include:

```text
FileDescriptorSet
FileDescriptorProto
DescriptorProto
FieldDescriptorProto
Timestamp
Duration
Any
Struct / Value / ListValue
FieldMask
```

## 10.1 Descriptor decoding

```rust
use prost::Message;
use prost_types::FileDescriptorSet;

let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/codefabric.binpb"));
let fds = FileDescriptorSet::decode(bytes.as_slice())?;
```

Use this for startup self-checks, descriptor fingerprints, reflection registration, or contract tooling—not for ordinary per-RPC execution.

## 10.2 Time types

For transport deadlines/expiration timestamps, prefer semantically explicit timestamp/duration messages if they are part of the released Protobuf contract. Be precise about whether a value is:

```text
wall-clock instant
monotonic local duration
relative TTL
absolute lease expiration
```

A `Timestamp` should not be used to represent a monotonic timeout budget.

## 10.3 `Any`

Avoid `Any` for the daemon's normal released control plane. `Any` moves type selection to runtime and makes compatibility/authorization harder. Use ordinary messages/`oneof` when variants are known. Dynamic type slots should be a deliberate extension mechanism, not a shortcut for schema design.

---

# 11) `bytes` — payload ownership and selective `Bytes` mapping

## 11.0 Why this crate is directly relevant

`bytes` is not merely a generic utility in this stack. Tonic/Hyper/Prost integrate around byte buffers, and large opaque payload fields are one of the few places where ownership/copy behavior can materially dominate a local UDS call.

## 11.1 `Bytes`

`Bytes` is an immutable, reference-counted byte container designed for cheap slicing/cloning and network-oriented sharing.

```rust
use bytes::Bytes;

let chunk: Bytes = ...;
let clone = chunk.clone();      // shares backing storage
let view = chunk.slice(0..1024);
```

Cloning a `Bytes` does not copy the full buffer; it increments shared ownership metadata.

## 11.2 `BytesMut`

`BytesMut` is mutable and supports capacity management, split/split_to, freeze, reserve, and reclaim-style operations.

```rust
use bytes::BytesMut;

let mut buf = BytesMut::with_capacity(1 << 20);
buf.extend_from_slice(source);
let immutable = buf.freeze();
```

Use it when a transport payload is being assembled incrementally. Do not maintain one globally shared mutable buffer across concurrent RPCs.

## 11.3 Selective Protobuf mapping

Recommended candidates for `prost_build::Config::bytes(...)`:

```text
ResponseChunkEvent.payload
ReadResultResponse.data
ArrowIpcChunk.payload
opaque result descriptor bytes where large
```

Less compelling candidates:

```text
32-byte checksum
small capability nonce
small opaque ID
```

## 11.4 `Bytes` is not “true zero-copy end-to-end” by magic

The network stack, Protobuf codec, Python runtime, Arrow decoder, or application conversion may still copy. The value is **reducing avoidable Rust-side ownership copies** and making slices/clones cheap. Benchmark the actual end-to-end path rather than claiming zero-copy solely because a field is `Bytes`.

## 11.5 Canonical result policy

For very large immutable result objects, the best optimization is usually **not** a larger `Bytes` field. It is your existing architecture:

```text
small/medium -> bounded inline/chunked transfer
large        -> immutable result resource + range reads
```

---

# 12) `tonic` object model — Request / Response / Status / Streaming / Extensions

## 12.0 Tonic's role

Tonic is the production Rust gRPC implementation in this stack. Its transport feature provides the batteries-included client/server runtime built on Tokio, Hyper/HTTP2, and Tower.

## 12.1 `Request<T>`

A Tonic request wraps the decoded Protobuf message plus transport/request context:

```rust
pub async fn handshake(
    &self,
    request: tonic::Request<HandshakeRequest>,
) -> Result<tonic::Response<HandshakeResponse>, tonic::Status> {
    let metadata = request.metadata();
    let extensions = request.extensions();
    let body = request.into_inner();
    ...
}
```

Important surfaces:

- `metadata()` / `metadata_mut()`;
- `extensions()` / `extensions_mut()`;
- local/remote address helpers where applicable;
- peer certificate access for TLS transports;
- `set_timeout(Duration)` when constructing outbound requests.

## 12.2 Extensions

Extensions are type-keyed request-local values. Tonic's transport inserts connection information such as `UdsConnectInfo`; interceptors/layers can insert validated principal/correlation objects for handlers.

Good pattern:

```text
UDS peer credentials + metadata credential
  -> interceptor / boundary policy
      -> AuthenticatedRpcContext in Request.extensions
          -> method handler
```

Do not repeatedly parse raw metadata in every service method.

## 12.3 `Response<T>`

A response carries the Protobuf result plus response metadata/extensions. Keep stable business/compatibility fields in Protobuf; use metadata only for transport/request context that is not part of the durable semantic contract.

## 12.4 `Status`

`tonic::Status` represents an RPC-level failure. It carries a gRPC code, message, and optional binary details/metadata depending on construction path.

Use `Status` for:

- invalid outer transport/RPC input;
- authentication/authorization failure;
- incompatible RPC contract/handshake;
- unavailable daemon/service;
- deadline/cancel conditions;
- transport/resource exhaustion;
- internal RPC boundary invariant failures.

Do not convert independent query-block semantic errors into transport failure if the canonical response contract says they are records inside a successful logical response.

## 12.5 `Streaming<T>`

Inbound server/client streaming is represented by `tonic::Streaming<T>`. Outbound server streaming is normally an application stream whose item type is `Result<Message, Status>`.

Streams are **message streams**, not raw byte streams. Chunk logical payloads deliberately so one stream item remains a bounded Protobuf message.

---

# 13) Generated Tonic client fundamentals

## 13.0 Generic client type

Generated clients are parameterized over a transport/service type:

```text
CpgQueryServiceClient<T>
```

The convenience `connect(...)` path exists when transport generation is enabled, but a UDS client normally uses a custom Tonic `Endpoint` connector and then constructs the generated client from the resulting `Channel`.

## 13.1 Long-lived channel/client

For a single daemon process, create one channel/client for the process lifetime and clone the generated client as needed. Tonic clients are designed around multiplexed HTTP/2 channels; creating a new connection per RPC defeats that design.

## 13.2 Per-call request

```rust
let mut req = tonic::Request::new(GetStatusRequest {});
req.metadata_mut().insert("x-rpc-attempt-id", attempt_id.parse()?);
req.set_timeout(Duration::from_millis(500));

let response = client.get_status(req).await?;
```

## 13.3 Max decode/encode size

Generated clients expose methods such as:

```text
max_decoding_message_size(...)
max_encoding_message_size(...)
```

Current Tonic defaults include a **4 MiB decoding limit**. Do not inherit this as an accidental business rule; configure explicit ceilings that comfortably contain the largest valid encoded envelope while keeping logical chunk sizes lower.

## 13.4 Compression negotiation

Generated clients expose send/accept compression configuration when the relevant crate features are enabled. Leave compression disabled in the UDS baseline and enable only after measurement.

## 13.5 Cloning clients

Clone the generated client handle when independent tasks need it rather than creating another physical connection. The underlying `Channel` is cheaply cloneable and multiplexed.

---

# 14) Generated Tonic server fundamentals

## 14.0 Service trait

Generated server code defines an async service trait. A conceptual implementation:

```rust
#[tonic::async_trait]
impl CpgQueryService for CpgService {
    async fn get_status(
        &self,
        request: Request<GetStatusRequest>,
    ) -> Result<Response<GetStatusResponse>, Status> {
        ...
    }

    type StreamQueryStream = Pin<
        Box<dyn Stream<Item = Result<QueryEvent, Status>> + Send + 'static>
    >;

    async fn stream_query(
        &self,
        request: Request<StreamQueryRequest>,
    ) -> Result<Response<Self::StreamQueryStream>, Status> {
        ...
    }
}
```

Exact generated associated types/signatures depend on RPC cardinality and codegen version.

## 14.1 Service wrapper

The generated `...Server<T>` wraps your implementation and exposes server configuration methods such as message-size and compression policy.

## 14.2 Keep handler bodies thin

Recommended handler flow:

```text
extract authenticated request context
-> validate outer Protobuf fields
-> call daemon application service
-> map domain/control result to Protobuf
-> return
```

Keep DataFusion planning, semantic query interpretation, resource lifecycle, and persistent mutation in the daemon layers that already own them rather than in Tonic glue.

## 14.3 Server streaming lifetime

A server-streaming method returns a stream whose lifetime can outlive the initial handler future. Ensure any per-query lease/subscription/cancellation guard is moved into the stream state or held by a task whose cleanup executes when the stream is dropped/cancelled.

---

# 15) RPC cardinalities and method-shape selection

## 15.0 Four gRPC method shapes

| Shape | Client | Server | Best fit |
|---|---|---|---|
| unary-unary | one message | one message | handshake/status/validate/start/cancel/release |
| unary-stream | one message | stream | query event feed, range/chunk feed |
| stream-unary | stream | one result | bulk ingest where sender chunks data |
| stream-stream | stream | stream | truly interactive full-duplex protocol |

## 15.1 Recommended mapping for the current daemon service

```text
Handshake       unary-unary
GetStatus       unary-unary
ValidateQuery   unary-unary
StartQuery      unary-unary
StreamQuery     unary-stream
AttachQuery     unary-stream
CancelQuery     unary-unary
ReadResult      unary-unary OR unary-stream depending released range contract
ReleaseResult   unary-unary
```

Do not introduce bidirectional streaming merely to make cancellation/progress look symmetrical. The accepted-handle + event stream + explicit Cancel/Attach model is simpler to recover and test.

## 15.2 Accepted handle before long work

`StartQuery` should return quickly after authorization/validation sufficient to establish the accepted handle. Long freshness/planning/execution work belongs after an addressable query identity exists.

This gives cancellation/reconnect a stable target even when the stream is broken.

## 15.3 Resumable stream invariant

A resumable stream needs application-level position:

```text
daemon_query_id
resume token
monotonic event sequence
optional checksum/cursor integrity
one terminal event
```

HTTP/2 itself does not supply semantic resume.

---

# 16) Metadata, extensions, interceptors, and boundary context

## 16.0 Three context planes

Do not mix these three mechanisms:

```text
Protobuf fields
  -> durable released RPC contract

gRPC metadata
  -> per-call transport/request context

Tonic Request extensions
  -> Rust-process-local typed context after admission/interceptors
```

Examples:

| Value | Preferred location |
|---|---|
| semantic request ID required by released RPC contract | Protobuf field |
| result lease token if part of released call contract | Protobuf field unless intentionally transport credential |
| `traceparent` | metadata |
| short-lived capability credential | metadata, if designed as transport auth |
| verified Unix peer UID/PID object | request extension |
| decoded authenticated agent/workspace principal | request extension |
| query JSON | Protobuf message payload field, not metadata |

## 16.1 Metadata types

Tonic exposes an HTTP-header-like `MetadataMap`. Text metadata keys/values and binary `-bin` metadata have different types/encoding rules. Keep metadata small; it rides HTTP/2 headers/trailers and is subject to header-size limits.

## 16.2 Interceptors

Tonic interceptors are useful for small synchronous boundary checks/rewrites around generated services/clients, such as:

- injecting a capability credential;
- extracting request IDs;
- validating mandatory metadata syntactically;
- inserting a request-local principal object;
- adding trace context where a tracing layer is not already doing so.

Do not run expensive async authorization/database work in a synchronous interceptor. For asynchronous policy, use an appropriate Tower layer/service wrapper or perform the authorization in the async method boundary before domain work starts.

## 16.3 Extension pattern

Conceptual server admission:

```rust
#[derive(Clone)]
struct RpcPrincipal {
    agent_id: AgentId,
    workspace_id: WorkspaceId,
    peer_uid: u32,
    peer_pid: Option<u32>,
}
```

After validating the UDS connection and credential, insert this into request extensions so handlers receive a typed verified identity instead of reparsing raw metadata.

## 16.4 Trace identity

Preserve distinct IDs rather than collapsing them:

```text
semantic_request_id
mcp_call_id
rpc_attempt_id
daemon_query_id
artifact/read lease IDs
trace/span IDs
```

They solve different correlation/idempotency/lifecycle problems. Use trace propagation for distributed tracing, but keep durable business/request identifiers in their released contract fields where required.

## 16.5 Error metadata

Do not leak raw paths, provider errors, SQL/DataFusion plans, credentials, or internal state through trailing metadata simply because metadata is less visible than response bodies. Apply the same public-redaction policy as to Protobuf error details.

---

# 17) Status codes and `tonic-types` richer errors

## 17.0 Basic gRPC status

Tonic status codes map to standard gRPC codes. Recommended outer-layer mapping:

| Situation | gRPC code |
|---|---|
| malformed / invalid outer RPC message | `InvalidArgument` |
| missing/expired authentication | `Unauthenticated` |
| authenticated principal lacks operation/workspace access | `PermissionDenied` |
| requested daemon/result object absent | `NotFound` where absence is the actual outer failure |
| operation incompatible with current state | `FailedPrecondition` |
| accepted ID reused inconsistently | often `AlreadyExists`, `Aborted`, or a released application-specific control error according to contract |
| daemon admission/quotas exhausted | `ResourceExhausted` |
| daemon not ready/temporarily unavailable | `Unavailable` |
| caller budget expired | `DeadlineExceeded` |
| caller cancelled | `Cancelled` |
| internal transport-layer invariant failure | `Internal` |

Do not string-match status messages in clients. Stable behavior belongs in code and/or typed detail messages.

## 17.1 Query semantic errors are not necessarily RPC errors

For the current architecture, query-level semantic errors can coexist with independently successful query blocks. They belong in the canonical logical response when the outer RPC succeeded.

```text
RPC transport succeeds
   -> canonical response
        query block A: success
        query block B: semantic error record
```

Do not abort the gRPC call merely because one semantic query block failed if the released response contract explicitly represents that result.

## 17.2 `tonic-types`

`tonic-types` 0.14.6 supplies Google RPC utility messages and `StatusExt` helpers for the **gRPC Richer Error Model**. It can attach structured details such as:

```text
BadRequest
ErrorInfo
RetryInfo
PreconditionFailure
ResourceInfo
QuotaFailure
RequestInfo
DebugInfo (usually restrict in production)
```

This is the Rust counterpart to Python `grpcio-status` / `google.rpc.Status` handling.

## 17.3 Where richer status is useful here

Good candidates:

- structured handshake compatibility failure;
- outer field validation failures;
- retryable daemon-unavailable details;
- quota/admission diagnostics;
- transport/result-resource precondition details.

Poor candidate:

- duplicating the entire canonical semantic query error model into `google.rpc.Status` details.

## 17.4 Safe details rule

Typed details do not make information safe automatically. Every detail message must obey the released public error allowlist and redaction policy.

---

# 18) Deadlines, `grpc-timeout`, timeout budgets, and cancellation

## 18.0 Distinguish four concepts

```text
deadline      caller's absolute completion budget
local timeout local future/task timing control
cancellation  caller/operation says stop
business TTL  lifetime of an artifact/token/lease
```

They are not interchangeable.

## 18.1 Python caller budget

The FastMCP Python adapter should pass an explicit timeout on each `grpc.aio` call derived from the remaining MCP/tool budget. The daemon should never depend on Python's default behavior being finite.

## 18.2 Tonic outbound `Request::set_timeout`

When a Rust gRPC client calls downstream gRPC services, use `Request::set_timeout(Duration)` when the timeout must propagate as gRPC `grpc-timeout` metadata.

A transport `Endpoint` timeout and an RPC `grpc-timeout` are different layers. Do not assume a local client future timeout communicates the remaining budget to the server.

## 18.3 Budget nesting

Preserve cleanup/serialization time:

```text
MCP host deadline
    > FastMCP tool timeout
        > Python gRPC deadline
            > daemon executable/freshness budget
                > downstream provider/data operation budget
                    + cleanup reserve
```

A useful implementation pattern is to compute a `DeadlineBudget` once at the RPC boundary and pass remaining durations to internal operations rather than hardcoding independent fixed timeouts everywhere.

## 18.4 Transport cancellation vs semantic CancelQuery

Use both:

```text
client cancels StreamQuery call
    -> gRPC stream is cancelled/dropped
    -> best-effort CancelQuery(query_id, resume/authority proof)
        -> daemon-wide CancellationToken
```

The gRPC cancellation stops transport work. The explicit `CancelQuery` addresses the accepted logical query and allows cancellation to remain meaningful across reconnects or when transport acknowledgement cannot be delivered.

## 18.5 Do not depend on dropped response future alone

Dropping a future or stream may cause transport cancellation, but daemon work can exist in independently spawned tasks. Application cancellation must explicitly reach those tasks.

## 18.6 Cleanup reserve

When a deadline fires, cleanup may still need to:

- release DataFusion reservations;
- cancel provider jobs;
- close cursors/streams;
- remove incomplete artifacts;
- drop leases;
- emit or persist a terminal state.

Do not give the inner computation the entire outer timeout and leave zero budget for cleanup.

---

# 19) Streaming, flow control, and backpressure

## 19.0 Native flow control is an asset

HTTP/2 and gRPC provide flow-control mechanisms so a fast sender cannot indefinitely overrun a slow receiver. Preserve that behavior instead of eagerly draining a stream into an unbounded application queue.

Recommended Python consumption:

```text
await next grpc.aio event
 -> validate/project
 -> deliver/consume
 -> await next event
```

Recommended Rust production:

```text
query/event producer
 -> small bounded channel or direct stream
 -> Tonic response stream
 -> HTTP/2 flow control
 -> Python consumer
```

## 19.1 Do not create an unbounded bridge queue

Anti-pattern:

```rust
let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
```

for a result stream whose consumer may be arbitrarily slow.

A slow agent could then translate remote backpressure into unbounded daemon memory growth.

## 19.2 Bounded channel when decoupling is necessary

```rust
let (tx, rx) = tokio::sync::mpsc::channel::<Result<QueryEvent, Status>>(8);
```

Choose capacity from measured scheduling needs. Capacity is an application memory/concurrency parameter, not a generic "higher is faster" knob.

## 19.3 Backpressure across DataFusion/Arrow

The daemon should not materialize an entire result simply because the gRPC stream API is asynchronous. Preserve bounded batch/chunk iteration from the execution engine through materialization and delivery where feasible.

```text
DataFusion batch
  -> validate/order/checksum stage
      -> bounded response chunk
          -> gRPC send
```

Large canonical results should still externalize to immutable result resources according to policy.

## 19.4 Stream item semantics

Every `ResponseChunkEvent` should remain independently bounded and have enough identity to validate ordering/reassembly:

```text
query ID
monotonic event sequence
chunk index / offset where applicable
content type/format version
length
checksum semantics
```

Do not assume gRPC message boundaries alone are sufficient for application-level resume/integrity.

## 19.5 Slow consumer test

Acceptance must include a deliberately slow Python consumer while the daemon produces quickly. Assert:

- bounded RSS growth;
- bounded queue depth;
- no lost sequence;
- cancellation still responds promptly;
- terminal event semantics remain correct;
- no artifact/epoch lease is prematurely released.

---

# 20) Message sizing, chunking, compression, and payload policy

## 20.0 Transport maximum vs logical maximum

Maintain separate limits:

```text
semantic request JSON max
single protobuf encoded message max
response chunk payload max
inline MCP response max
artifact range-read chunk max
artifact total max
```

A logical result can be 1 GiB while no individual gRPC message is remotely that large.

## 20.1 Tonic default receive limit

Tonic's generated client/server surfaces default to a **4 MiB decoding limit**. Treat this as an implementation default, not your application policy.

The current application profile has an inline hard maximum near 4 MiB, so configure the gRPC encoded-message ceiling **above the maximum valid encoded envelope** rather than exactly equal to the logical payload size.

Example policy shape:

```text
result chunk payload max       1 MiB
encoded chunk message cap      2 MiB (illustrative; calculate exact envelope headroom)
control message cap            lower where practical
MCP inline hard max            4 MiB logical canonical response
large result                   externalized
```

The exact numbers should be contract/test-derived, not copied from this example.

## 20.2 Client configuration

Generated client:

```rust
let client = CpgQueryServiceClient::new(channel)
    .max_decoding_message_size(MAX_RPC_MESSAGE)
    .max_encoding_message_size(MAX_RPC_MESSAGE);
```

Apply the corresponding generated server limits as well so both ends agree on an intentional envelope.

## 20.3 Python symmetry

Python `grpc.aio` channel options should set corresponding send/receive message ceilings. Contract tests must prove the largest valid Rust→Python and Python→Rust envelopes pass, and the first invalid size fails with the expected stable outer error.

## 20.4 Compression

For local UDS, default to **no gRPC compression**.

Why:

```text
no WAN bandwidth charge
very low RTT
large results already chunk/externalize
Arrow IPC is binary and may already have its own compression/storage properties
compression consumes CPU and can increase latency
```

Benchmark gzip/deflate/zstd only if profiling identifies byte-copy/socket bandwidth as material.

## 20.5 Mixed-secret compression

If attacker-controlled content and secrets can appear in the same compressed response/request, evaluate compression side-channel implications. This is less likely in the private local profile but should remain part of the security review if compression is enabled.

## 20.6 Never raise limits as the first fix

A giant request failing a size limit is usually a signal to evaluate:

- chunking;
- resource externalization;
- range reads;
- schema design;
- accidental duplicated data;
- debug metadata leakage.

---

# 21) `Endpoint`, `Channel`, and connection lifecycle

## 21.0 `Endpoint`

`tonic::transport::Endpoint` describes client-side connection configuration: target URI, connection timeout, HTTP/2 settings, keepalive, concurrency/buffer behavior, TLS for network transports, and connectors.

For UDS the URI is primarily an authority/config placeholder because the physical connection is created by a custom connector.

## 21.1 `Channel`

A `Channel` is the multiplexed client transport. Clone it cheaply; do not reconnect per RPC.

```text
one daemon target
 -> one Channel
 -> multiple generated clients / cloned client handles
 -> many concurrent HTTP/2 RPC streams
```

## 21.2 `connect_with_connector`

Tonic explicitly supports custom transport connectors, including Unix sockets.

Conceptual:

```rust
use tokio::net::UnixStream;
use tonic::transport::Endpoint;
use tower::service_fn;

let path = socket_path.clone();
let channel = Endpoint::try_from("http://[::]:50051")?
    .connect_with_connector(service_fn(move |_| {
        UnixStream::connect(path.clone())
    }))
    .await?;
```

The dummy HTTP URI does not turn the UDS into TCP; the connector determines the underlying IO stream.

## 21.3 Connection establishment deadline

Use `connect_timeout`/startup timeout deliberately so a missing daemon/socket causes readiness failure promptly. This is distinct from per-RPC deadlines after the channel is established.

## 21.4 Reconnect behavior

A long-lived Tonic channel can recover transport connections. Application-level query streams still need explicit `AttachQuery`/resume semantics because connection recovery does not replay partially consumed application streams safely.

## 21.5 Shutdown

When daemon client ownership ends, drop/close the channel after in-flight operations are cancelled/drained according to application policy. Avoid retaining channel clones in detached tasks that outlive the owner.

---

# 22) Unix-domain-socket client integration

## 22.0 Why UDS is preferred locally

UDS provides:

- no externally routable TCP listener;
- filesystem-mediated endpoint access;
- kernel peer credential support;
- lower local-stack overhead in many cases;
- simple one-user daemon topology.

It does **not** by itself prove the caller is the intended agent process. Combine it with explicit authentication/authorization.

## 22.1 Client connector

Recommended client construction:

```rust
use std::path::PathBuf;
use tokio::net::UnixStream;
use tonic::transport::Endpoint;
use tower::service_fn;

async fn connect_uds(path: PathBuf) -> Result<tonic::transport::Channel, tonic::transport::Error> {
    Endpoint::from_static("http://localhost")
        .connect_with_connector(service_fn(move |_| {
            UnixStream::connect(path.clone())
        }))
        .await
}
```

For the Python adapter, use the gRPC Core Unix-target form supported by `grpcio` (for example `unix:///...`) and test it against the real generated server.

## 22.2 Socket path naming

The socket path should be derived from an authorized runtime directory, not arbitrary user input. Prefer a per-user or per-workspace runtime path whose parent permissions are controlled.

## 22.3 Startup race

The adapter should not assume "socket file exists" means the daemon is ready. Startup should be:

```text
socket path resolved
 -> channel connects
 -> Handshake succeeds
 -> compatibility/auth/workspace admission succeeds
 -> adapter publishes readiness
```

Stale socket files are possible after abnormal termination; readiness must be protocol-based.

---

# 23) Unix-domain-socket server integration and peer credentials

## 23.0 Listener

Tokio's `UnixListener` provides async accept for Unix stream sockets. `tokio-stream` provides `UnixListenerStream`, which adapts it into a `Stream` acceptable to Tonic's `serve_with_incoming` path.

Conceptual server:

```rust
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;

let listener = UnixListener::bind(&socket_path)?;
let incoming = UnixListenerStream::new(listener);

Server::builder()
    .add_service(CpgQueryServiceServer::new(service))
    .serve_with_incoming(incoming)
    .await?;
```

## 23.1 `Connected` and `UdsConnectInfo`

Tonic implements its `Connected` trait for Tokio `UnixStream`. Each request can access `UdsConnectInfo` through request extensions:

```rust
use tonic::transport::server::UdsConnectInfo;

let info = request
    .extensions()
    .get::<UdsConnectInfo>()
    .ok_or_else(|| Status::unauthenticated("missing UDS peer info"))?;
```

`UdsConnectInfo` contains:

```text
peer_addr
peer_cred: Option<UCred>
```

## 23.2 `UCred`

Tokio's Unix credential type exposes operating-system-derived identity such as:

```text
uid()
gid()
pid() -> Option<_> on supported Unix platforms
```

The exact availability of PID information is OS-specific. Never make a cross-platform invariant depend on `pid()` being `Some` unless the deployment platform is explicitly constrained and tested.

## 23.3 Authentication stack

Recommended local admission:

```text
1. socket parent directory owned by expected OS user
2. socket mode/ownership verified
3. peer UID/GID extracted from kernel UDS credentials
4. peer PID checked where supported/required
5. short-lived capability credential validated
6. credential binds agent + workspace + operation + expiry + anti-replay ID
7. request gets typed RpcPrincipal extension
8. each sensitive RPC reauthorizes current operation/object/lease
```

Do not treat the socket pathname or possession of an opaque artifact ID as authorization.

## 23.4 PID limitations

PID identity can be reused by the OS after process exit. If the credential binds a specific process instance, combine PID with additional anti-replay/process-start identity or a credential minted/held by that process rather than treating PID alone as a cryptographic principal.

---

# 24) Socket filesystem lifecycle, process identity, and local security

## 24.0 Runtime directory

Prefer a private runtime directory whose permissions are established **before** binding the socket.

Illustrative local profile:

```text
runtime directory 0700
socket accessible only to owner
artifact dirs 0700
artifact files 0600
```

Exact socket mode behavior is platform/umask-dependent; set/check ownership/permissions explicitly rather than relying on developer-shell umask.

## 24.1 Stale socket cleanup

Startup should handle:

```text
path absent -> bind
path exists and live expected daemon -> do not clobber
path exists but stale -> verify safely, then unlink/rebind
path wrong owner/type -> fail closed
```

Blindly deleting any supplied pathname before bind can become a filesystem attack.

## 24.2 Symlink/path attacks

Keep the socket under a trusted private parent. Avoid following attacker-controlled symlinks or using a world-writable parent without secure path handling.

## 24.3 Daemon ownership

The process supervisor should own daemon lifetime; gRPC is not a process manager. Define:

- how the socket path is communicated;
- who creates runtime dirs;
- startup readiness;
- abnormal exit cleanup;
- restart policy;
- how adapters distinguish old/stale capability credentials after restart.

## 24.4 Local does not mean harmless

A compromised same-user process may be able to read files, connect to sockets, or inspect process data. UDS permissions are one layer, not a replacement for short-lived operation-scoped credentials and server-side authorization.

---

# 25) Tokio runtime design for the daemon

## 25.0 What Tokio owns

Tokio provides the async I/O driver, scheduler, timers, task system, and Unix socket primitives that Tonic's transport uses.

## 25.1 Multi-thread runtime baseline

For a daemon that also coordinates DataFusion/providers/Arrow and performs substantial async orchestration, the multi-thread runtime is the normal baseline:

```rust
#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    run_daemon().await
}
```

Tune worker count only if profiling shows scheduler/threading effects. DataFusion and other engines may own their own thread pools; increasing Tokio worker threads blindly can oversubscribe CPU.

## 25.2 Blocking work

Do not block Tokio workers with:

- synchronous filesystem/network calls that can stall;
- expensive compression;
- CPU-heavy canonicalization/checksum loops if large enough to monopolize a worker;
- blocking process waits;
- long locks.

Use native async APIs, engine-owned pools, `spawn_blocking` for appropriate short/controlled blocking work, or dedicated execution resources according to ownership.

## 25.3 Task ownership

Every spawned task should have a lifecycle owner:

```text
request-owned
query-owned
server-lifespan-owned
supervisor-owned
```

Detached `tokio::spawn` calls with no cancellation/join/error path are dangerous in a daemon where shutdown must leave no detached work.

## 25.4 Signals

Use Tokio signal handling to trigger structured daemon shutdown. Stop accepting new work before tearing down resources needed by existing handlers.

## 25.5 Runtime metrics

Tokio exposes runtime metrics, including newer scheduling-latency metrics under relevant APIs/feature surfaces. Use these only when scheduler contention is suspected; ordinary RPC latency and engine metrics remain the primary operational signals.

---

# 26) `tokio-stream` adapters

## 26.0 Role in this stack

`tokio-stream` converts Tokio async sources into `Stream` implementations used naturally by Tonic.

Most important here:

```text
UnixListenerStream
ReceiverStream
```

## 26.1 `UnixListenerStream`

Requires the crate's `net` feature.

```rust
let listener = tokio::net::UnixListener::bind(path)?;
let incoming = tokio_stream::wrappers::UnixListenerStream::new(listener);

Server::builder()
    .add_service(service)
    .serve_with_incoming(incoming)
    .await?;
```

## 26.2 `ReceiverStream`

Wrap a bounded Tokio `mpsc::Receiver` to expose an event stream:

```rust
let (tx, rx) = tokio::sync::mpsc::channel(8);
let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
```

This is a clean way to connect a query-owned producer task to a Tonic response stream if direct stream composition is inconvenient.

## 26.3 Avoid `UnboundedReceiverStream` for result data

It can be appropriate for intrinsically bounded control signals, but it should not be the default for potentially large query/result streams because it converts consumer slowness into memory growth.

## 26.4 Stream combinators

Use StreamExt/combinators carefully. A long chain of buffering/concurrency combinators can accidentally change ordering, concurrency, or memory use. The query contract requires deterministic sequence/order; preserve that explicitly.

---

# 27) `tokio-util::CancellationToken` and structured cancellation

## 27.0 Why it belongs in the core stack

`CancellationToken` provides a cloneable cancellation tree that is better suited to daemon-wide operation cancellation than ad hoc booleans/oneshot channels scattered through subsystems.

Key methods:

```text
new
clone
cancel
cancelled
cancelled_owned
child_token
is_cancelled
run_until_cancelled
```

## 27.1 Query cancellation tree

Recommended pattern:

```text
server shutdown token
   -> query token
       -> freshness child token
       -> provider child token(s)
       -> execution child token
       -> materialization child token
       -> artifact externalization child token
```

Cancelling a parent cancels children. Cancelling a child should not automatically cancel its parent/siblings unless the application explicitly propagates that failure.

## 27.2 Select pattern

```rust
tokio::select! {
    _ = cancel.cancelled() => {
        Err(QueryCancelled)
    }
    out = run_query_stage() => out,
}
```

`tokio::select!` cancellation safety depends on the futures involved. Verify each provider/stream/resource operation before assuming dropping a future is harmless.

## 27.3 Accepted-query registry

Store the query's cancellation token in the daemon's accepted-query lifecycle record so `CancelQuery` can cancel the same logical operation even if the original transport stream is gone.

## 27.4 Terminal event

Cancellation should still converge on **exactly one terminal query state/event**. Avoid multiple independent tasks racing to publish terminal events after the token is cancelled.

## 27.5 Server shutdown

Server shutdown should cancel a parent token, stop new admissions, and await/query cleanup within the shutdown grace. Do not simply abort all tasks immediately unless the hard shutdown deadline has elapsed.

---

# 28) Tower middleware, admission, load shedding, buffers, timeout, retry

## 28.0 Why Tower is protocol-adjacent here

Tonic's transport is built on the Tower `Service` abstraction. Tower layers can apply cross-cutting service behavior around RPC routing without putting policy inside every generated method.

Useful primitives include:

```text
ServiceBuilder
ConcurrencyLimit
LoadShed
Timeout
Buffer
Retry
rate limiting / utility layers where justified
```

## 28.1 `ServiceBuilder` order matters

Layers are composed in the order defined and transform each other's semantics. Document the stack rather than adding layers opportunistically.

## 28.2 Concurrency limits

A Tower/Tonic connection-wide concurrency limit is **not** the same thing as daemon query admission.

Bad architecture:

```text
connection max concurrency = 2
both slots occupied by long StreamQuery calls
CancelQuery cannot get service promptly
```

Better:

```text
RPC transport accepts a reasonable bounded number of concurrent streams
 -> method-level authorization/control
 -> daemon query admission allows 2 active / 4 queued
 -> lightweight Cancel/GetStatus/Read/Release remain serviceable
```

## 28.3 Tonic `concurrency_limit_per_connection`

Tonic Server exposes a per-connection limit. Use only as a transport-abuse ceiling, not as the main work scheduler.

## 28.4 Load shedding

Tonic exposes `load_shed(bool)`. With load shedding enabled, requests arriving when the service is not immediately ready can be rejected (for example as resource exhaustion) rather than buffered.

This can be useful at a protection boundary, but your daemon already owns queue/admission semantics. Avoid contradicting those semantics with an opaque outer shedder unless the policy is explicitly designed.

## 28.5 Buffers

Tower `Buffer` adds a queue in front of an inner service. This can increase the number of in-flight requests beyond downstream concurrency limits.

For this daemon, **do not add a large Tower buffer** in front of a daemon that already guarantees a bounded per-agent queue. Hidden queues harm cancellation/fairness/latency observability.

## 28.6 Timeout layer

A Tower timeout is a local service timing boundary. It can be useful as a hard safety limit, but must be coordinated with gRPC deadlines and application cleanup. Do not stack unrelated timeouts whose winner is unpredictable.

## 28.7 Retry layer

Do not attach a generic Tower retry layer to all gRPC methods.

Per-method policy:

```text
Handshake     possibly bounded connection/startup retry
GetStatus     safe bounded retry when transport transient
ValidateQuery maybe if proven idempotent
StartQuery    never blind retry; use effective semantic/idempotency identity
StreamQuery   resume/AttachQuery rather than generic replay
CancelQuery   retry only if released idempotency semantics permit
ReadResult    exact range/lease retry where contract permits
ReleaseResult idempotent retry if release is explicitly idempotent
```

## 28.8 Rate limits

If high-rate tiny control calls are an abuse concern, a rate limiter can supplement authentication. Keep it separate from CPU/memory query admission so lightweight status/cancel calls are not unfairly coupled to expensive query slots.

---

# 29) HTTP/2 flow windows, concurrent streams, keepalive, frames, headers

## 29.0 Tonic server knobs worth knowing

Current Tonic server transport exposes controls including:

```text
initial_stream_window_size
initial_connection_window_size
max_concurrent_streams
http2_adaptive_window
http2_keepalive_interval
http2_keepalive_timeout
max_frame_size
max_header_list_size
max_connection_age / grace (where available)
connection-level concurrency/load shedding/timeout
```

These are powerful but are **not initial tuning requirements** for a local UDS deployment.

## 29.1 Initial flow-control window

HTTP/2's conventional initial stream/connection window default is around 65,535 bytes. On a WAN with a large bandwidth-delay product, a small window can limit throughput. On a same-host UDS, RTT is tiny, so the benefit of multi-megabyte windows is much less obvious.

Start with defaults. Tune only if profiling shows the sender repeatedly stalls specifically on flow-control window updates while CPU/application work is otherwise ready.

## 29.2 Adaptive window

Tonic can enable adaptive HTTP/2 flow-control window management. If enabled, it supersedes some static initial-window assumptions. Benchmark it against static defaults rather than enabling it because "adaptive" sounds universally better.

## 29.3 `max_concurrent_streams`

HTTP/2 streams include long-lived server-streaming calls. Set a reasonable abuse ceiling but keep it high enough that active query streams do not prevent cancellation/status/resource control operations.

## 29.4 Keepalive

For a private local UDS:

```text
process crash -> kernel closes socket/FD -> peer observes failure
```

so keepalive pings usually add little. Leave disabled/default unless testing reveals a failure mode where half-open connections remain undetected for an unacceptable period.

For a future remote/TCP profile, reevaluate keepalive jointly with proxies/load balancers; do not reuse UDS assumptions.

## 29.5 Frame size

Do not tune HTTP/2 `max_frame_size` as a substitute for gRPC message chunking. One gRPC message can span multiple HTTP/2 frames; application chunk boundaries should remain semantically intentional.

## 29.6 Header-list limit

This protects metadata size. Keep authentication/trace/correlation metadata compact so normal calls remain far below the configured limit.

---

# 30) `tonic-health`

## 30.0 Standard health protocol

`tonic-health` implements the standard `grpc.health.v1.Health` service. It supplies generated protocol types and server utilities such as a health reporter/server pair.

Conceptual setup:

```rust
let (mut health_reporter, health_service) = tonic_health::server::health_reporter();

health_reporter
    .set_serving::<CpgQueryServiceServer<CpgService>>()
    .await;

Server::builder()
    .add_service(health_service)
    .add_service(CpgQueryServiceServer::new(service))
    .serve_with_incoming(incoming)
    .await?;
```

Exact generic service-health helpers should be verified against the pinned 0.14.6 API when implemented.

## 30.1 Liveness vs readiness vs compatibility

Do not let generic health replace the daemon `Handshake`.

```text
gRPC health
 -> process/service is serving

Handshake
 -> exact RPC major/minor/schema/profile/credential/workspace/agent compatibility
```

For the FastMCP adapter, **Handshake is the authoritative readiness gate**.

## 30.2 Shutdown

Set health to not-serving before or while draining if any external supervisor/client uses health to decide whether to route new calls.

## 30.3 Local profile value

Because one local adapter connects to one known daemon, health is optional operational convenience rather than a load-balancer requirement. It can still simplify diagnostics and standard tooling.


# 31) `tonic-reflection` and descriptor embedding

## 31.0 Purpose

`tonic-reflection` implements standard gRPC server reflection. Reflection lets generic clients/tooling ask a live server which services/messages/descriptors it exposes.

Useful for:

- `grpcurl`/interactive development;
- generated-contract debugging;
- descriptor/service-name verification;
- compatibility/diagnostic tooling;
- generic RPC clients in controlled environments.

## 31.1 Descriptor registration

Reflection is most robust when driven from the exact descriptor set generated from the released `.proto` authority.

Conceptual build:

```rust
const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("codefabric");
```

Conceptual registration:

```rust
let reflection = tonic_reflection::server::Builder::configure()
    .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
    .build_v1()?;
```

Current reflection APIs support registering descriptor sets and building v1 / v1alpha reflection services. Verify exact builder method names against 0.14.6 at implementation time.

## 31.2 Production policy

For this locked private control interface, the default recommendation is:

```text
development/test       reflection ON
production local UDS   reflection OFF unless diagnostics need it
```

Reflection reveals service and schema structure. Even though the socket is private, expose only what creates operational value.

## 31.3 Reflection is not compatibility negotiation

A client can discover the current descriptor and still be incompatible with it. The released Handshake/fingerprint/profile rules remain the compatibility authority.

## 31.4 Descriptor equality test

When reflection is enabled in test builds, assert that the reflected descriptor graph matches the build's canonical descriptor fingerprint. This catches a surprisingly damaging class of errors where the server binary and separately packaged schema artifact drift.

---

# 32) `prost-reflect` and dynamic schema tooling

## 32.0 Role

Prost intentionally keeps the generated-message runtime small and does not supply general runtime reflection through `prost::Message` itself. `prost-reflect` adds:

```text
DescriptorPool
DynamicMessage
MessageDescriptor / FieldDescriptor APIs
ReflectMessage
```

The current target is **0.16.5**.

## 32.1 `DescriptorPool`

A descriptor pool decodes/holds a `FileDescriptorSet` and resolves fully-qualified symbols.

Conceptually:

```rust
use prost_reflect::DescriptorPool;

let pool = DescriptorPool::decode(FILE_DESCRIPTOR_SET)?;
let message = pool
    .get_message_by_name("codefabric.cpgd.v1.HandshakeRequest")
    .expect("descriptor must exist");
```

## 32.2 `DynamicMessage`

A dynamic message can decode a payload using a runtime `MessageDescriptor` rather than a compile-time generated Rust type.

Use cases:

- descriptor compatibility tools;
- migration/inspection utilities;
- generic test harnesses;
- controlled reflection clients;
- decoding extension/test messages whose concrete type is not compiled into the tool.

Do **not** replace normal generated Tonic handlers with dynamic messages for this daemon. Generated types are safer, faster to reason about, easier to review, and much easier to statically check.

## 32.3 Cache pools

Building descriptor pools repeatedly is unnecessary. If runtime reflection is enabled, construct one immutable pool at process startup and share it.

## 32.4 Untrusted dynamic type dispatch

Treat a caller-selected type name as untrusted input. A generic `type_name -> DynamicMessage -> dispatch` mechanism can accidentally become a new capability surface. Allowlist what can be inspected/decoded, and never let dynamic reflection bypass the released RPC service catalog.

## 32.5 Build-time companion

`prost-reflect-build` exists to help generate `ReflectMessage` implementations. Add it only if generated typed messages genuinely need descriptor access in production code; ordinary daemon handlers do not.

---

# 33) `pbjson`, `pbjson-build`, and `pbjson-types`

## 33.0 What problem they solve

Protobuf has a defined JSON mapping (ProtoJSON). Ordinary `serde_json` on generated Rust structs is **not automatically equivalent** to ProtoJSON. `pbjson`/`pbjson-build` generate serde implementations intended to follow Protobuf JSON mapping semantics for Prost-generated types.

Targets in this reference:

```text
pbjson        0.9.0
pbjson-build  0.9.0
pbjson-types  0.9.0
```

## 33.1 `pbjson-build`

The build crate can generate `Serialize`/`Deserialize` implementations for descriptors/types so JSON conversion follows Protobuf conventions rather than Rust struct field conventions.

Use when:

- a released **control Protobuf** message must also be represented as ProtoJSON;
- fixtures need cross-language ProtoJSON equivalence;
- admin/debug tooling wants standards-aligned JSON for generated messages.

## 33.2 Do not use it for the canonical QRY JSON automatically

The current architecture has a **separately released canonical semantic JSON profile**. That JSON is not made authoritative by the Protobuf control schema.

Therefore:

```text
Protobuf control message -> pbjson ProtoJSON       optional control representation
canonical QRY JSON       -> its own released spec authoritative
```

Do not silently derive/replace QRY JSON with a `pbjson` serialization of a Rust control DTO graph.

## 33.3 `pbjson-types`

This crate provides JSON-aware implementations around Protobuf well-known types. Evaluate exact type support/compliance before making it a required production dependency; it is not necessary simply because `prost-types` is used.

## 33.4 Performance

A Protobuf -> serde/JSON -> Protobuf conversion is extra work. Keep the normal gRPC control hot path binary. JSON conversion should happen only where the contract/presentation actually requires JSON.

---

# 34) Descriptor fingerprints, schema authority, and build reproducibility

## 34.0 Why descriptors are the best derived contract artifact

A `FileDescriptorSet` captures the compiled schema graph independently of Rust/Python source formatting. It can support:

- schema fingerprinting;
- breaking-change analysis;
- reflection;
- dynamic inspection;
- cross-language build equivalence;
- package/service/method/field-number assertions.

## 34.1 Canonical descriptor build

Recommended CI:

```bash
buf format --diff --exit-code
buf lint
buf breaking --against '.git#branch=main'
buf build --as-file-descriptor-set --exclude-source-info \
  -o target/proto/codefabric.binpb
```

Flag spellings should be pinned/verified with the chosen Buf CLI version; the conceptual invariant is **one source-info-normalized descriptor artifact**.

## 34.2 Hashing

```text
schema_fingerprint = HASH(canonical FileDescriptorSet bytes)
```

Record:

```text
hash algorithm
Buf/protoc release
whether imports are included
whether source info is excluded
exact module/input
```

A digest with undocumented normalization is not a durable compatibility mechanism.

## 34.3 Build self-check

At Rust build/test time:

1. decode embedded descriptor;
2. assert expected service full name;
3. assert released method names/cardinalities;
4. assert critical message/field numbers;
5. compute/compare expected descriptor fingerprint.

At Python package test time, perform equivalent descriptor assertions against generated `_pb2` descriptors.

## 34.4 Cross-language descriptor comparison

Compare the **semantic descriptor graph/fingerprint**, not generated source text. Rust and Python code generators legitimately emit different source artifacts from the same schema.

## 34.5 Generated-code cache policy

If generated Python files are committed but Rust source is build-generated, that asymmetry is fine. What matters is that both prove derivation from the same authority and CI fails stale artifacts.

---

# 35) Cross-language Python ↔ Rust interoperability

## 35.0 Companion stack

The Python side already has dedicated advanced references for:

```text
grpcio / grpc.aio
protobuf Python runtime
```

This section therefore focuses only on **joint invariants**.

## 35.1 Generation inputs must match

```text
same .proto release
same imports
same package/service names
same field numbers/types/cardinalities
compatible compiler/runtime families
```

A Rust build passing by itself does not prove the Python package was regenerated.

## 35.2 Wire fixture matrix

For every released control message family, test at minimum:

```text
Rust serialize -> Python parse
Python serialize -> Rust parse
Rust request -> Python fixture expectations
Python request -> live Rust server
```

For important evolutionary changes also test old/new pairs:

```text
old Python client -> new Rust daemon
new Python client -> old released Rust daemon fixture/server
```

according to the promised compatibility policy.

## 35.3 ProtoJSON tests are separate

If any control Protobuf is also exposed as ProtoJSON, run explicit JSON fixtures in both languages. Binary wire compatibility does not prove JSON field-name/enum/64-bit-number compatibility.

## 35.4 Unknown field asymmetry

Python's full Protobuf runtime may preserve unknown binary fields across parse/reserialize, while Prost should not be assumed to do so. Therefore do not design a compatibility mechanism that requires a Rust daemon to act as a transparent unknown-field-preserving Protobuf proxy unless the pinned stack is explicitly tested for that requirement.

## 35.5 Bytes

Rust `bytes::Bytes` still arrives as Python `bytes` after grpcio/protobuf decoding. The Rust-side representation reduces Rust ownership/copy pressure but does not create a shared cross-process memory region.

For large Arrow data, treat Arrow IPC payloads as opaque validated bytes through the control boundary until/unless Python intentionally decodes them.

## 35.6 Status details

If using richer Google RPC status details:

```text
Rust tonic-types message
 -> gRPC status-details-bin
 -> Python grpcio-status / google.rpc.Status
```

Keep typed detail `.proto` definitions in the same schema governance pipeline and add bilateral decode fixtures.

## 35.7 Deadline semantics

Python `timeout=` must become a server-visible gRPC deadline. The Rust handler should read/obey the resulting deadline semantics through Tonic behavior and propagate a smaller remaining budget into inner operations.

## 35.8 Cancellation

Test Python `call.cancel()` / task cancellation against:

- Rust stream drop/cancellation;
- explicit `CancelQuery` handling;
- internal `CancellationToken` propagation;
- exactly-one terminal state;
- no leaked query/artifact/epoch resources.

## 35.9 UDS target

Python uses gRPC Core's UDS target syntax; Rust Tonic accepts `UnixListenerStream`. A real interop test must use the actual UDS transport—not only localhost TCP—because peer credentials, path permissions, target parsing, and failure modes differ.

---

# 36) Mapping the stack to the FastMCP / daemon service lifecycle

## 36.0 Process-level topology

```text
FastMCP adapter lifespan
  -> immutable settings
  -> resolve UDS endpoint
  -> create one grpc.aio channel
  -> create generated CpgQueryService stub/facade
  -> attach credential/trace interceptors
  -> Handshake
  -> publish FastMCP readiness
```

Rust daemon:

```text
process startup
  -> private runtime/artifact dirs
  -> remove/validate stale UDS state safely
  -> bind UnixListener
  -> initialize daemon authorities/services
  -> construct Tonic service
  -> optionally health/reflection admin services
  -> serve_with_incoming(UnixListenerStream)
```

## 36.1 `Handshake`

Transport goal:

```text
prove exact agent/workspace/process authority
+ negotiate RPC/profiles/schema fingerprints/limits/capabilities
```

Implementation stack:

```text
Tonic UdsConnectInfo.peer_cred
 + capability metadata/field
 + Protobuf HandshakeRequest
 -> daemon auth/compatibility service
 -> HandshakeResponse
```

Handshake failure occurs before semantic query readiness is advertised.

## 36.2 `GetStatus`

Keep this unary, cheap, and available while the workspace is bootstrapping. Do not make it queue behind long expensive query slots.

## 36.3 `ValidateQuery`

The outer Protobuf request carries released JSON/control data; Rust performs the actual schema/semantic/capability authorization validation. Python may do presentation-level validation but must not become the semantic authority.

## 36.4 `StartQuery`

Return accepted identity quickly:

```text
daemon_query_id
resume token
accepted time
queue state
negotiated profile versions
effective semantic request ID
```

Do not hold the unary response until DataFusion execution completes.

## 36.5 `StreamQuery`

Use server streaming. Every event has one monotonic sequence and one of the released event variants. Preserve backpressure instead of draining events into an unbounded queue.

## 36.6 `AttachQuery`

This is application-level resume. Validate query ID + resume token + cursor/checksum + current authority before replaying/continuing events.

## 36.7 `CancelQuery`

Resolve the accepted-query record, reauthorize, cancel its `CancellationToken`, and return an idempotent/defined acknowledgement. Transport cancellation remains independent.

## 36.8 `ReadResult`

Use bounded exact range/chunk reads. Reauthorize on every read. Verify offset/length/checksum/lease before returning bytes.

If the response is one bounded chunk, unary is simplest. If the released contract eventually supports multi-chunk streaming from one call, use unary-stream intentionally and preserve range semantics.

## 36.9 `ReleaseResult`

Keep unary and idempotent. It should release the caller's lease/retention claim without relying on the Python process completing a graceful shutdown.

---

# 37) Graceful shutdown, reconnect, resume, and accepted-handle semantics

## 37.0 Shutdown order

Recommended daemon sequence:

```text
termination requested
 -> stop accepting new semantic query admissions
 -> mark health NOT_SERVING where used
 -> retain control path long enough for cancellation/drain
 -> cancel or allow in-flight work according to grace policy
 -> emit/persist terminal states where possible
 -> release execution/provider resources
 -> finish active result reads or cancel according to lease contract
 -> stop Tonic server
 -> close durable/engine resources
 -> remove UDS file safely
 -> exit
```

## 37.1 Tonic serve shutdown

Use Tonic's shutdown-aware serving API or combine the incoming server future with a shutdown signal rather than killing the runtime abruptly.

## 37.2 Long streams

A server-streaming query may intentionally live longer than the process's shutdown grace. Therefore clients need `AttachQuery`/resume; graceful shutdown cannot mean "wait forever for every stream to naturally end."

## 37.3 Adapter shutdown

FastMCP adapter:

```text
stop admitting new MCP calls
 -> cancel active gRPC calls
 -> best-effort CancelQuery for accepted queries owned by dying adapter when policy requires
 -> release active result reads/leases where possible
 -> close grpc.aio channel
 -> exit with STDOUT still protocol-clean
```

## 37.4 Reconnect

On broken transport:

```text
re-establish channel
 -> reauthenticate / Handshake as required
 -> AttachQuery(existing accepted identity, resume cursor)
```

Never create a fresh `StartQuery` solely because the stream disconnected unless the contract explicitly determines the old accepted query no longer exists and the semantic idempotency policy permits restart.

## 37.5 Restart invalidation

Short-lived capability credentials should encode enough daemon/process/epoch authority that stale credentials from a prior daemon instance cannot silently authenticate after restart unless that continuity is explicitly intended.

---

# 38) Performance engineering and recommended baseline configuration

## 38.0 First principle

For local UDS, **structural choices dominate knob tuning**.

Highest-leverage baseline:

1. one long-lived Python channel/stub;
2. one Tokio/Tonic server, no per-call server startup;
3. bounded Protobuf messages;
4. `Bytes` for selected large byte fields;
5. bounded stream buffers;
6. native HTTP/2 flow control preserved;
7. daemon-owned query admission;
8. explicit deadlines/cancellation;
9. no generic retry;
10. no compression unless measured;
11. no keepalive unless measured;
12. default HTTP/2 windows until evidence says otherwise.

## 38.1 Recommended initial Tonic server posture

Conceptually:

```rust
let server = tonic::transport::Server::builder()
    // explicit limits added where contract-derived
    // no compression by default
    // no aggressive keepalive
    // no large hidden Tower buffer
    // transport concurrency high enough for control calls
    ;
```

Do not configure every available knob just to make the deployment look "tuned."

## 38.2 Measure latency in layers

Record at least:

```text
Python FastMCP adapter entry -> gRPC call start
gRPC client encode/send
Rust Tonic handler entry
transport auth/peer credential admission
outer request validation
query queue wait
freshness wait
query planning
DataFusion execution
materialization/checksum
stream send / slow-consumer stall
Python receive/decode
Pydantic/public projection
MCP delivery
```

## 38.3 RPC metrics

Per method:

```text
call count
p50/p95/p99 latency
status code
request encoded bytes
response encoded bytes / streaming message count
active streams
cancel count + cancel latency
deadline exceeded count
resource exhausted/admission count
```

Keep agent/workspace IDs out of low-cardinality metric labels unless the deployment scale and privacy policy explicitly allow them.

## 38.4 Serialization microbenchmarks

Benchmark separately:

```text
Prost encode/decode control message
Vec<u8> vs Bytes generated payload field
ResponseChunkEvent construction
Arrow IPC chunk wrapping
Python protobuf decode
Python public-model projection
```

If end-to-end latency is dominated by DataFusion, spending days removing a microsecond from Prost encoding is not valuable.

## 38.5 Copy accounting

For representative 1 MiB chunks, track:

```text
Arrow/output buffer creation
-> Protobuf field ownership
-> Prost encode
-> Tonic/HTTP2 buffers
-> kernel UDS
-> gRPC Core receive
-> Python protobuf bytes object
-> optional Arrow decode
```

Use profilers/alloc counters to identify actual copies before adding unsafe buffer tricks or custom codecs.

## 38.6 Concurrency benchmark

Test combinations such as:

```text
0/1/2 active heavy queries
+ GetStatus traffic
+ CancelQuery
+ ReadResult
+ slow StreamQuery consumer
```

Acceptance criterion: control operations remain low-latency even when query admission is saturated.

## 38.7 UDS vs loopback TCP benchmark

It is useful to retain one comparative benchmark, but UDS remains the architecture choice for security/topology reasons even if raw microbenchmark differences are small. Do not switch to TCP solely for a tiny benchmark win without reevaluating peer identity and listener exposure.

## 38.8 Tuning order

Only after baseline measurement consider, in order:

1. message/chunk shape;
2. avoidable buffer copies / `Bytes` mapping;
3. bounded channel capacity;
4. Tokio task/worker contention;
5. Tower/service admission topology;
6. HTTP/2 window/adaptive flow control;
7. compression;
8. exotic transport settings.

---

# 39) Testing, fuzzing, compatibility, and executable contract checks

## 39.0 Test layers

Use all of these:

```text
1. pure daemon-domain tests
2. generated Rust Protobuf message tests
3. descriptor/schema compatibility tests
4. in-process Tonic tests where useful
5. real UDS Rust server + Python grpc.aio client
6. FastMCP STDIO -> Python -> UDS -> Rust end-to-end
7. old/new released compatibility fixtures
8. hostile/slow/cancelled transport tests
```

## 39.1 Schema gates

```bash
buf format --diff --exit-code
buf lint
buf breaking --against <released-baseline>
buf build --as-file-descriptor-set -o target/proto/schema.binpb
```

Then compare expected fingerprint and generated bindings.

## 39.2 Descriptor assertions

Assert at least:

- package full name;
- service full name;
- released method names;
- RPC cardinality;
- critical field numbers;
- reserved/deleted field constraints through Buf breaking checks;
- schema fingerprint.

## 39.3 Wire round trips

Golden fixtures:

```text
Rust -> Python
Python -> Rust
old Python -> new Rust
new Python -> old Rust fixture/server
```

Include:

- defaults/presence-sensitive fields;
- max numeric values;
- unknown enum numeric value handling;
- `oneof` variants;
- bytes payloads;
- status details;
- timestamps/durations where used.

## 39.4 Service method tests

Every RPC should test:

```text
success
auth missing
wrong agent
wrong workspace
expired/replayed credential
wrong RPC/schema major
same-version fingerprint mismatch
malformed outer message
resource exhausted
client deadline
client cancellation
server shutdown/drain
```

plus method-specific lifecycle cases.

## 39.5 Streaming tests

`StreamQuery`/`AttachQuery`:

```text
monotonic sequence
exactly one terminal event
replay from cursor
invalid cursor/checksum
slow consumer
mid-stream transport loss
reattach after connection loss
cancellation at every phase
terminal retention expiry
```

## 39.6 Message-size tests

Test boundary values just below/at/above:

```text
semantic request limit
RPC encoded envelope max
response chunk max
artifact read chunk max
MCP inline threshold/hard max
```

## 39.7 Peer credential tests

On every supported OS target:

- expected UID accepted;
- different UID rejected where test environment permits;
- PID presence/absence matches platform expectations;
- missing `UdsConnectInfo` fails closed on endpoints that require UDS;
- stale/cross-process credential denied.

## 39.8 Fuzzing

Useful fuzz targets include:

- decode arbitrary Protobuf control bytes under input cap;
- query event sequence state machine;
- resume cursor/checksum parser;
- lease/range arithmetic;
- rich status detail decoder;
- descriptor fingerprint/parser;
- canonical JSON envelope validation.

Do not use fuzzing as proof of schema compatibility; it supplements deterministic contract fixtures.

## 39.9 Concurrency-model tests

For small custom synchronization components around accepted-query state, cancellation, terminal emission, or lease release, model-testing tools can be valuable. Avoid trying to model the entire Tonic/Tokio/DataFusion system in one concurrency test.

## 39.10 Acceptance oracle principle

A self-generated expected output is weak. The strongest acceptance tests compare independently generated clients/servers or released fixtures across the actual wire.

---

# 40) Security hardening

## 40.0 Threat layers

Treat separately:

```text
endpoint discovery/path attack
UDS filesystem access
kernel peer identity
application capability credential
authorization by agent/workspace/operation
Protobuf input abuse
stream/resource exhaustion
error/diagnostic disclosure
result artifact authorization
process restart/replay
```

## 40.1 UDS permissions

Keep the socket under a private directory, verify ownership/type, and fail closed on unexpected path state. Do not expose the daemon via a wildcard TCP listener in the local profile "for debugging" unless that listener has its own explicit authentication and is disabled by default.

## 40.2 Peer credentials

Verify the kernel-provided UID/GID and PID where available. Do not accept a caller-provided PID as proof of process identity.

## 40.3 Capability credential

Credential should bind at least the dimensions required by the authority model:

```text
agent instance
adapter process/session identity as designed
workspace
allowed operation set / ACL profile
expiration
anti-replay identity
possibly daemon/session binding
```

Never log the raw credential.

## 40.4 Reauthorization

Do not authenticate only at Handshake and then treat every future artifact ID/resume token as bearer authority. Reauthorize sensitive operations including:

```text
query start/attach/cancel
status/reference projection where sensitive
source-bearing result creation/read
artifact range read
lease release
```

## 40.5 Input resource limits

Protect against:

- oversized messages;
- huge repeated fields;
- pathological nested JSON inside an allowed bytes/string field;
- too many concurrent streams;
- slowloris-like long idle streams;
- compression bombs if compression is ever enabled;
- repeated reconnect/attach storms;
- artifact range abuse;
- metadata/header abuse.

## 40.6 Reflection/admin surface

Reflection, health, and any future Channelz/admin endpoint are separate capabilities. Enable only on the private listener and only when required; do not assume "standard gRPC service" means harmless metadata.

## 40.7 Error masking

Normalize raw internal failures into released public status/error forms. Never leak:

```text
filesystem paths
Git remote/history
DataFusion physical plans
SQL/internal table names
provider stderr
command lines/environment
credentials/tokens
raw source content unless explicitly authorized
```

## 40.8 Denial of service via control starvation

A security/resource-exhaustion policy that lets expensive streams consume every HTTP/2/concurrency slot can prevent cancellation and make overload worse. Reserve practical serviceability for control methods.


# 41) Optional / alternative packages and what not to adopt by default

## 41.0 `protoc-bin-vendored`

`protoc-bin-vendored` 3.2.0 packages Google-built `protoc` binaries for multiple platforms and exposes functions such as:

```rust
protoc_bin_vendored::protoc_bin_path()
protoc_bin_vendored::include_path()
```

Value case:

- a Cargo build must work without a separately installed compiler;
- cross-platform developer setup simplicity is more important than minimizing build dependencies;
- offline/self-contained source builds need a compiler artifact already in the dependency graph.

Why it is **not the recommended authority for this project by default**:

- compiler release ownership becomes indirect through a third-party crate version;
- the vendored crate can lag the active upstream `protoc` line;
- it pulls platform-specific binary packages into the dependency graph;
- the project already benefits from explicit Buf/protoc toolchain governance.

Prefer an explicitly managed `protoc 36.x` build tool or a canonical Buf-built descriptor set. Use vendoring only when a constrained build/distribution environment proves the need.

## 41.1 `rust-protobuf`

The Rust `protobuf` / rust-protobuf ecosystem is a legitimate alternative Protocol Buffers implementation, but this Tonic 0.14 stack is natively centered on Prost. Do not introduce a second message-runtime/codegen ecosystem unless a dependency or compatibility requirement specifically requires it.

## 41.2 direct `hyper`, `h2`, `http`, `http-body`

These are important **underlying Tonic dependencies and diagnostic concepts**, but application code should not depend on them directly simply to "tune gRPC." Reach below Tonic only when implementing a custom transport/middleware behavior unavailable through Tonic/Tower and after documenting why the abstraction escape is necessary.

## 41.3 generic serialization frameworks

Do not replace Prost with `serde`/bincode/MessagePack inside gRPC merely to chase serialization benchmarks. The schema/codegen/interoperability properties of Protobuf are part of the architecture. Opaque payload fields may intentionally carry canonical JSON or Arrow IPC, but the outer RPC remains Protobuf.

## 41.4 gRPC-Web

`tonic-web` is irrelevant to the current FastMCP -> local daemon profile. Do not add browser/gRPC-Web translation to the private UDS daemon boundary.

## 41.5 TLS crates

Tonic supports TLS feature sets, commonly rustls-based in current releases. The local UDS profile should not enable TLS unless the threat model changes. If a future remote profile is introduced, treat TLS/mTLS as a separate deployment profile rather than quietly changing the local one.

---

# 42) Official `grpc` Rust preview: watch, do not migrate production yet

## 42.0 Current state

In 2026 the gRPC project began publishing an official Rust `grpc` crate line (current preview **0.9.0**) alongside the Tonic codebase/repository.

The crate documentation is explicit:

```text
preview
not recommended for production
APIs unstable
```

Therefore **Tonic 0.14.6 remains the production recommendation** for this daemon reference.

## 42.1 Why this matters strategically

This is not a reason to freeze the architecture. It is a reason to keep the boundary clean:

```text
.proto contract
  -> generated RPC adapter layer
      -> daemon services
```

If the official Rust gRPC implementation matures, a future migration should primarily affect the generated/transport adapter rather than semantic/query/data-fabric layers.

## 42.2 Evaluation trigger

Re-evaluate the official `grpc` stack when all of the following are true:

- documentation removes the preview/non-production warning;
- server support covers the required production topology;
- Unix-domain sockets and peer credential access have a robust path;
- streaming/backpressure/cancellation/deadlines are production-tested;
- generated Protobuf integration is stable;
- health/reflection/richer status or equivalents meet requirements;
- Rust↔Python gRPC interop passes the existing acceptance suite;
- performance is at least comparable on representative workload.

Do not schedule migration merely because the crate name is more official.

---

# 43) Upgrade and compatibility discipline

## 43.0 Upgrade units

Treat these as coordinated but distinct upgrade surfaces:

```text
Rust toolchain / MSRV
Tonic family
Prost family
Tokio family
Tower
protoc release line
Buf CLI/config
.proto contract
Python grpcio
a Python protobuf runtime/compiler
optional reflection/status/json crates
```

Do not upgrade all simultaneously unless there is a compelling reason; smaller upgrade sets make regressions attributable.

## 43.1 Tonic family lockstep

Prefer matching patch versions across:

```text
tonic
tonic-prost
tonic-prost-build
tonic-health
tonic-reflection
tonic-types
```

when those crates are used. They are released as a coordinated family and often constrain one another.

## 43.2 Prost family lockstep

Likewise align:

```text
prost
prost-build
prost-types
```

on the same 0.14.x patch where practical.

## 43.3 Upgrade gate

For every RPC stack upgrade:

```text
[ ] resolve exact dependency graph / Cargo.lock
[ ] verify MSRV
[ ] regenerate Rust code
[ ] regenerate Python code if compiler/plugin changed
[ ] rebuild canonical descriptor
[ ] buf lint/breaking passes
[ ] descriptor fingerprint change is understood
[ ] Rust<->Python unary tests pass
[ ] streaming/resume tests pass
[ ] deadline/cancellation tests pass
[ ] UDS peer-credential tests pass
[ ] max-message boundary tests pass
[ ] slow-consumer/backpressure test passes
[ ] shutdown/drain tests pass
[ ] performance baseline compared
[ ] security/redaction tests pass
```

## 43.4 Runtime-only upgrade with unchanged descriptor

A Tonic/Tokio/Prost runtime upgrade can leave the `.proto` descriptor fingerprint unchanged. That does **not** mean behavior is unchanged. Re-run transport concurrency, cancellation, stream, shutdown, and performance tests.

## 43.5 Schema upgrade

A schema change must pass both:

```text
Buf breaking policy
+ released semantic/compatibility policy
```

Buf catches structural compatibility classes; it cannot prove application semantics, authorization, idempotency, or lifecycle compatibility.

## 43.6 Edition upgrade

Treat a change from proto3/Edition 2023/2024 to Edition 2026 as a **compiler+generator+schema-language migration**, even if the binary schema is intended to remain compatible.

## 43.7 Tonic master warning

The gRPC Rust repository explicitly notes that Tonic's `master` branch may contain breaking work and directs production users to the latest released 0.14.x line. Coding agents should not source examples from `main` and assume they compile against the pinned release.

---

# 44) Anti-pattern inventory

## Contract and code generation

- using generated Rust/Python code as the editable contract authority;
- compiling Rust and Python from different `.proto` trees/import roots;
- no committed/configured compiler version;
- relying on a workstation's arbitrary `protoc` from `PATH`;
- bypassing `buf breaking` for released contract changes;
- treating generated source hashes as the canonical schema fingerprint;
- adopting Edition 2026 features without testing every generator/runtime;
- using old Tonic pre-0.14 `tonic-build` examples blindly;
- hand-editing generated Tonic/Prost output;
- adding a second Rust Protobuf runtime without a concrete requirement.

## Protobuf / payload

- growing Protobuf into a duplicate semantic fact DTO graph;
- assuming Prost preserves unknown fields through decode/re-encode;
- encoding large Arrow/canonical payloads as nested structured Protobuf when the payload is intentionally opaque;
- using `Any` or `Struct` as a substitute for defining the known control contract;
- converting Protobuf to JSON and back on the binary hot path;
- applying `bytes::Bytes` to every bytes field without measurement;
- claiming end-to-end zero-copy just because Rust uses `Bytes`;
- increasing max message sizes instead of chunking/externalizing giant results.

## Channel / connection

- creating a gRPC channel per FastMCP tool call;
- creating multiple UDS channel pools to one daemon without a measured reason;
- adding DNS/LB/xDS to a single local socket topology;
- enabling aggressive keepalive on a UDS by default;
- enabling TLS on UDS without a threat-model reason;
- using TCP wildcard listeners for debug convenience in production.

## Concurrency / flow control

- global RPC concurrency = heavy-query concurrency;
- allowing query streams to occupy every control slot;
- adding an unbounded `mpsc` queue between query execution and gRPC;
- adding a large Tower `Buffer` in front of an already bounded daemon queue;
- materializing complete large query responses before sending the first chunk;
- increasing HTTP/2 windows before proving flow-control stalls are a bottleneck;
- parallelizing event processing in a way that violates released sequence order.

## Deadlines / cancellation

- no deadline because "the daemon is local";
- giving inner execution the entire outer deadline with no cleanup reserve;
- treating a local future timeout as propagated `grpc-timeout` automatically;
- assuming dropping the gRPC stream stops independently spawned daemon work;
- blind retry of `StartQuery` after transport failure;
- generic retry middleware around every method;
- creating a new logical query instead of `AttachQuery` after stream loss;
- multiple tasks racing to emit terminal events.

## UDS / security

- using socket existence as readiness;
- binding under an uncontrolled world-writable path;
- blindly unlinking an existing socket path;
- trusting caller-supplied PID/UID metadata;
- authenticating once at Handshake then treating opaque IDs as permanent authority;
- logging capability credentials;
- exposing reflection/admin diagnostics without deliberate policy;
- leaking internal paths/plans/provider errors through rich status details.

## Runtime lifecycle

- detached Tokio tasks with no owner/cancellation/join path;
- shutting down the Tokio runtime before query/artifact cleanup;
- closing engine/provider resources before Tonic handlers have drained;
- assuming graceful shutdown can wait forever for long-lived streams;
- leaving stale socket/result resources after abnormal shutdown without recovery policy.

---

# 45) Dense API / dependency / decision matrices

## 45.1 Core package matrix

| Need | Reach for | Scope |
|---|---|---|
| Rust gRPC runtime | `tonic` | runtime |
| Tonic↔Prost codec | `tonic-prost` | runtime |
| Rust Protobuf messages | `prost` | runtime |
| WKT/descriptors | `prost-types` | runtime if used |
| Protobuf/Tonic codegen | `tonic-prost-build` | build |
| custom Rust message mapping | `prost-build` | build |
| shared byte buffers | `bytes` | runtime |
| async runtime / UDS | `tokio` | runtime |
| UnixListener -> Stream | `tokio-stream` | runtime |
| cancellation tree | `tokio-util` | runtime |
| service middleware/admission | `tower` | runtime if directly used |
| standard health | `tonic-health` | optional runtime |
| server reflection | `tonic-reflection` | dev/admin/optional runtime |
| richer gRPC errors | `tonic-types` | optional runtime |
| dynamic descriptors/messages | `prost-reflect` | tooling/optional runtime |
| ProtoJSON serde mappings | `pbjson*` | optional build/runtime |
| compiler | `protoc` | build tool |
| schema governance | Buf CLI | dev/CI tool |

## 45.2 Tonic 0.14 codegen rule

| Task | Correct default |
|---|---|
| Protobuf + Tonic generation | `tonic-prost-build` |
| custom Prost representation | `prost-build::Config` passed through `tonic-prost-build` |
| generic/non-Protobuf Tonic service generator | `tonic-build` |
| runtime Protobuf codec | `tonic-prost` |

## 45.3 UDS ownership matrix

| Concern | Owner |
|---|---|
| runtime directory | daemon/controller |
| socket bind | Tokio `UnixListener` |
| incoming adapter | `tokio-stream::UnixListenerStream` |
| serve | Tonic `serve_with_incoming` |
| peer credentials | Tonic `UdsConnectInfo` / Tokio `UCred` |
| application principal | daemon auth layer |
| credential issuance | controller/security authority |
| readiness | Handshake, optionally health as supporting signal |

## 45.4 Deadline/cancel matrix

| Need | Primitive |
|---|---|
| caller RPC budget | Python `grpc.aio` timeout |
| Rust outbound propagated deadline | `Request::set_timeout` |
| local stage timeout | Tokio `timeout` / budget helper |
| accepted logical query cancel | `CancelQuery` |
| internal fan-out cancellation | `tokio_util::CancellationToken` |
| transport stream cancel | gRPC call/stream cancellation/drop |
| reconnect | `AttachQuery` with resume cursor/token |

## 45.5 Buffering matrix

| Buffer/queue | Default posture |
|---|---|
| gRPC/HTTP2 internal buffers | accept defaults, measure |
| bounded Tokio channel between producer/stream | allowed when useful; small explicit capacity |
| unbounded Tokio result queue | prohibit |
| Tower `Buffer` before query service | avoid unless specifically justified |
| full result in memory | only under bounded inline policy |
| large result | immutable externalized resource |

## 45.6 Compression matrix

| Deployment/payload | Baseline |
|---|---|
| local UDS control messages | none |
| local UDS Arrow chunks | none; benchmark only |
| future WAN repetitive JSON-like payload | evaluate compression |
| already-compressed payload | usually do not recompress |
| mixed untrusted + secret content | security review before compression |

## 45.7 Reflection matrix

| Environment | Reflection |
|---|---|
| developer local | on is useful |
| CI contract tests | useful |
| production private UDS | off by default |
| future public/remote API | deliberate admin/security decision |

## 45.8 Status placement matrix

| Failure/info | Place |
|---|---|
| authentication failure | gRPC status |
| compatibility handshake failure | gRPC status + safe typed detail / released handshake failure as designed |
| invalid outer RPC envelope | gRPC status |
| query-block semantic failure | canonical logical response record |
| retry hint for transport unavailability | gRPC richer detail if used |
| internal DataFusion plan error text | internal logs only / safe mapped public error |

## 45.9 Descriptor tool matrix

| Need | Tool |
|---|---|
| compile `.proto` | `protoc` or `buf build` |
| canonical repository build | `buf build` |
| style | `buf format` |
| lint | `buf lint` |
| breaking-change check | `buf breaking` |
| generate via plugins | `buf generate` |
| Rust generated code | `tonic-prost-build` / `prost-build` |
| runtime static descriptor | `include_file_descriptor_set!` / `prost-types` |
| runtime dynamic descriptor | `prost-reflect::DescriptorPool` |
| server reflection | `tonic-reflection` |

## 45.10 Current version matrix

| Component | Reference target |
|---|---:|
| Tonic | 0.14.6 |
| Tonic Prost | 0.14.6 |
| Tonic Prost Build | 0.14.6 |
| Tonic Build | 0.14.6 |
| Tonic Health | 0.14.6 |
| Tonic Reflection | 0.14.6 |
| Tonic Types | 0.14.6 |
| Prost | 0.14.4 |
| Prost Build | 0.14.4 |
| Prost Types | 0.14.4 |
| Bytes | 1.12.1 |
| Tokio | 1.53.1 |
| Tokio Stream | 0.1.19 |
| Tokio Util | 0.7.19 |
| Tower | 0.5.3 |
| Prost Reflect | 0.16.5 |
| pbjson family | 0.9.0 |
| protoc | 36.x active support |
| Rust MSRV dictated by Tonic | 1.88 |
| official new `grpc` Rust crate | 0.9.0 preview — not production |

---

# 46) Agent implementation checklist

```text
VERSION / TOOLCHAIN
[ ] Pin/review Tonic family 0.14.6.
[ ] Pin/review Prost family 0.14.4.
[ ] Record Tokio 1.53.1 / Tower 0.5.3 resolution.
[ ] Require Rust >=1.88 for the Tonic stack.
[ ] Record exact protoc 36.x compiler.
[ ] Pin Buf CLI policy/config in CI.
[ ] Do not use grpc Rust 0.9 preview in production.

SCHEMA AUTHORITY
[ ] .proto + compatibility policy are authority.
[ ] Run buf format/lint/breaking/build.
[ ] Emit canonical FileDescriptorSet.
[ ] Document descriptor fingerprint normalization/hash.
[ ] Compare Rust/Python generated descriptors.
[ ] Never hand-edit generated source.

CODE GENERATION
[ ] Use tonic-prost-build for Protobuf/Tonic 0.14.
[ ] Use prost-build::Config only for deliberate Rust mappings.
[ ] Use bytes::Bytes selectively for high-volume bytes fields.
[ ] Generate client and server surfaces intentionally.
[ ] Embed descriptor only if runtime self-check/reflection needs it.
[ ] Keep generated code at RPC boundary.

UDS
[ ] Bind under private trusted runtime directory.
[ ] Handle stale socket path safely.
[ ] Use Tokio UnixListener + UnixListenerStream.
[ ] Serve through Tonic serve_with_incoming.
[ ] Read UdsConnectInfo from Request extensions.
[ ] Validate kernel peer UID/GID and PID where supported.
[ ] Never trust caller-supplied process identity.
[ ] Pair UDS peer identity with short-lived capability credential.

AUTHORIZATION
[ ] Credential binds agent/workspace/operation/expiry/anti-replay identity.
[ ] Handshake authenticates and negotiates compatibility.
[ ] Reauthorize query attach/cancel and artifact read/release.
[ ] Opaque artifact/query IDs are not bearer authority by themselves.
[ ] Public errors/details are allowlisted/redacted.

CHANNEL / SERVER
[ ] One long-lived grpc.aio channel per FastMCP adapter.
[ ] Rust daemon has one stable UDS listener per profile.
[ ] Do not add DNS/LB/pools to single-UDS topology.
[ ] Keep transport concurrency distinct from heavy-query admission.
[ ] Ensure Cancel/GetStatus/Read/Release remain serviceable under query saturation.

DEADLINES / CANCELLATION
[ ] Every Python daemon RPC has bounded timeout.
[ ] Inner daemon budget reserves cleanup time.
[ ] Use explicit grpc-timeout propagation for Rust downstream gRPC calls.
[ ] Transport cancellation propagates into query CancellationToken.
[ ] Explicit CancelQuery addresses accepted logical query.
[ ] Exactly one terminal event/state is emitted.
[ ] Reconnect uses AttachQuery, not blind StartQuery replay.

STREAMING / BACKPRESSURE
[ ] StreamQuery events have monotonic sequence.
[ ] Producer does not materialize all results before sending.
[ ] Any bridge mpsc queue is bounded.
[ ] No unbounded result queue.
[ ] No large hidden Tower buffer.
[ ] Slow-consumer RSS remains bounded.
[ ] Control operations remain responsive during slow streams.

MESSAGE / PAYLOAD LIMITS
[ ] Explicit Rust max encode/decode message sizes.
[ ] Matching Python grpcio send/receive limits.
[ ] Logical payload max is lower than encoded envelope max.
[ ] Result read chunks remain bounded.
[ ] Large results externalize rather than raising transport limits.
[ ] Compression disabled initially on UDS.

HTTP/2
[ ] Keep default flow windows initially.
[ ] Keep keepalive disabled/default initially.
[ ] Set max concurrent streams as abuse ceiling, not query scheduler.
[ ] Tune windows/frame/header settings only from measured evidence.

PROTOBUF SEMANTICS
[ ] Do not assume Prost preserves unknown fields through re-encoding.
[ ] Test enum unknown-value behavior used by contract.
[ ] Avoid Any/Struct unless dynamic semantics are explicit.
[ ] Do not duplicate canonical semantic JSON as a Protobuf DTO graph.
[ ] ProtoJSON behavior tested separately if used.

HEALTH / REFLECTION / DYNAMIC TOOLS
[ ] Handshake remains readiness authority.
[ ] tonic-health optional supporting signal only.
[ ] Reflection enabled only where deliberate.
[ ] Reflection descriptor matches canonical fingerprint in tests.
[ ] prost-reflect only for dynamic tooling, not normal RPC dispatch.
[ ] pbjson only for control ProtoJSON use cases, never as implicit QRY JSON authority.

SHUTDOWN
[ ] Stop new semantic admissions first.
[ ] Drain/cancel in-flight operations under grace budget.
[ ] Query tokens/provider jobs/artifact writes cleaned up.
[ ] Long streams can resume after restart where contract allows.
[ ] Close service before engine resources it still needs.
[ ] Remove socket safely after server stops.
[ ] No detached Tokio tasks survive ownership shutdown.

TESTS
[ ] Rust serialize -> Python parse.
[ ] Python serialize -> Rust parse.
[ ] Real UDS generated-client integration.
[ ] FastMCP STDIO end-to-end integration.
[ ] Old/new compatibility fixtures.
[ ] Handshake fingerprint mismatch test.
[ ] Peer credential and credential replay tests.
[ ] Deadline/cancel at every lifecycle stage.
[ ] Stream disconnect/AttachQuery resume test.
[ ] Slow-consumer backpressure test.
[ ] Message-size boundary test.
[ ] Large artifact/range-read test.
[ ] Shutdown/restart test.
[ ] Performance baseline compared on dependency upgrades.
```

---

## 46.1 Source map by topic

The dense source index below is complemented by this topic map so coding agents can jump to the authoritative family most relevant to a claim instead of treating all links as equivalent.

| Topic / sections | Primary sources |
|---|---|
| Tonic 0.14 architecture, request/response, transport | [Tonic][TONIC], [Tonic latest API][TONIC-LATEST] |
| UDS peer credentials | [Tonic UDS][TONIC-UDS], [Tokio UnixStream][TOKIO-UNIX-STREAM], [Tokio UCred][TOKIO-UCRED] |
| Tonic server / HTTP2 knobs | [Tonic Server][TONIC-SERVER] |
| Tonic client endpoint/custom connector | [Tonic Endpoint][TONIC-ENDPOINT] |
| Tonic/Prost code generation | [tonic-prost-build][TONIC-PROST-BUILD], [tonic-build][TONIC-BUILD], [prost-build][PROST-BUILD] |
| Prost runtime/message behavior | [Prost][PROST], [Message trait][PROST-MESSAGE] |
| Prost unknown-field roadmap caveat | [Prost roadmap][PROST-ROADMAP] |
| Descriptor sets / reflection | [prost-build config][PROST-BUILD-CONFIG], [Buf descriptors][BUF-DESCRIPTORS], [tonic-reflection][TONIC-REFLECTION], [prost-reflect][PROST-REFLECT] |
| Protocol compiler / Editions | [Protobuf version support][PB-VERSION-SUPPORT], [Editions][PB-EDITIONS] |
| Buf workflow | [Buf CLI][BUF-CLI], [Buf breaking][BUF-BREAKING], [Buf generate][BUF-GENERATE] |
| Tokio runtime/UDS | [Tokio][TOKIO], [UnixListener][TOKIO-UNIX-LISTENER], [UnixStream][TOKIO-UNIX-STREAM] |
| Cancellation | [CancellationToken][CANCELLATION-TOKEN] |
| Tower middleware | [Tower][TOWER], [Tower features][TOWER-FEATURES] |
| Bytes ownership/buffers | [bytes][BYTES], [BytesMut][BYTES-MUT] |
| Health | [tonic-health][TONIC-HEALTH] |
| Rich errors | [tonic-types][TONIC-TYPES] |
| ProtoJSON helpers | [pbjson][PBJSON], [pbjson-build][PBJSON-BUILD], [pbjson-types][PBJSON-TYPES] |
| official Rust gRPC preview | [grpc preview][GRPC-RUST-PREVIEW], [gRPC Rust repo][GRPC-RUST-REPO] |
| optional vendored compiler | [protoc-bin-vendored][PROTOC-BIN-VENDORED] |

---

# 47) Source index

## Tonic / gRPC Rust

[TONIC]: https://docs.rs/tonic/0.14.6/  
[TONIC-LATEST]: https://docs.rs/tonic/latest/tonic/  
[TONIC-UDS]: https://docs.rs/tonic/latest/tonic/transport/server/struct.UdsConnectInfo.html  
[TONIC-SERVER]: https://docs.rs/tonic/latest/tonic/transport/struct.Server.html  
[TONIC-ENDPOINT]: https://docs.rs/tonic/latest/tonic/transport/struct.Endpoint.html  
[TONIC-REQUEST]: https://docs.rs/tonic/latest/tonic/struct.Request.html  
[TONIC-BUILD]: https://docs.rs/tonic-build/0.14.6/  
[TONIC-PROST]: https://docs.rs/tonic-prost/0.14.6/  
[TONIC-PROST-BUILD]: https://docs.rs/tonic-prost-build/0.14.6/  
[TONIC-PROST-BUILDER]: https://docs.rs/tonic-prost-build/latest/tonic_prost_build/struct.Builder.html  
[TONIC-HEALTH]: https://docs.rs/tonic-health/0.14.6/  
[TONIC-REFLECTION]: https://docs.rs/tonic-reflection/0.14.6/  
[TONIC-TYPES]: https://docs.rs/tonic-types/0.14.6/  
[GRPC-RUST-REPO]: https://github.com/grpc/grpc-rust  
[GRPC-RUST-PREVIEW]: https://docs.rs/grpc/0.9.0/grpc/  

## Prost / Protobuf Rust

[PROST]: https://docs.rs/prost/0.14.4/  
[PROST-MESSAGE]: https://docs.rs/prost/latest/prost/trait.Message.html  
[PROST-BUILD]: https://docs.rs/prost-build/0.14.4/  
[PROST-BUILD-CONFIG]: https://docs.rs/prost-build/latest/prost_build/struct.Config.html  
[PROST-TYPES]: https://docs.rs/prost-types/0.14.4/  
[PROST-ROADMAP]: https://github.com/tokio-rs/prost/issues/624  
[PROST-REFLECT]: https://docs.rs/prost-reflect/0.16.5/  

## Tokio / Tower / Bytes

[TOKIO]: https://docs.rs/tokio/1.53.1/  
[TOKIO-UNIX-LISTENER]: https://docs.rs/tokio/latest/tokio/net/struct.UnixListener.html  
[TOKIO-UNIX-STREAM]: https://docs.rs/tokio/latest/tokio/net/struct.UnixStream.html  
[TOKIO-UCRED]: https://docs.rs/tokio/latest/tokio/net/unix/struct.UCred.html  
[TOKIO-STREAM]: https://docs.rs/tokio-stream/0.1.19/  
[TOKIO-UTIL]: https://docs.rs/tokio-util/0.7.19/  
[CANCELLATION-TOKEN]: https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html  
[TOWER]: https://docs.rs/tower/0.5.3/  
[TOWER-FEATURES]: https://docs.rs/crate/tower/0.5.3/features  
[BYTES]: https://docs.rs/bytes/1.12.1/  
[BYTES-MUT]: https://docs.rs/bytes/latest/bytes/struct.BytesMut.html  

## Protocol Buffers compiler / compatibility

[PB-VERSION-SUPPORT]: https://protobuf.dev/support/version-support/  
[PB-CROSS-VERSION]: https://protobuf.dev/support/cross-version-runtime-guarantee/  
[PB-EDITIONS]: https://protobuf.dev/editions/overview/  
[PB-PROTO3]: https://protobuf.dev/programming-guides/proto3/  
[PB-ENCODING]: https://protobuf.dev/programming-guides/encoding/  
[PB-JSON]: https://protobuf.dev/programming-guides/json/  
[PROTOC-RELEASES]: https://github.com/protocolbuffers/protobuf/releases  

## Buf

[BUF-CLI]: https://buf.build/docs/cli/  
[BUF-BUILD]: https://buf.build/docs/reference/cli/buf/build/  
[BUF-LINT]: https://buf.build/docs/lint/  
[BUF-BREAKING]: https://buf.build/docs/breaking/  
[BUF-GENERATE]: https://buf.build/docs/generate/  
[BUF-DESCRIPTORS]: https://buf.build/docs/reference/descriptors/  
[BUF-CONFIG]: https://buf.build/docs/configuration/v2/buf-yaml/  

## Optional ProtoJSON / compiler packaging

[PBJSON]: https://docs.rs/pbjson/0.9.0/  
[PBJSON-BUILD]: https://docs.rs/pbjson-build/0.9.0/  
[PBJSON-TYPES]: https://docs.rs/pbjson-types/0.9.0/  
[PROTOC-BIN-VENDORED]: https://docs.rs/protoc-bin-vendored/3.2.0/  

## Companion project references

This reference is intended to be used alongside the project-specific advanced references for:

- FastMCP 3.4.7;
- gRPC Python / `grpcio` 1.83.x;
- Protocol Buffers Python 7.36.x;
- the authoritative Present-State CPG FastMCP Serving Specification v2.0;
- the DataFusion / Arrow / relational data-fabric design references governing daemon-owned semantics.

---

## Final architecture compression

For agent implementation, the entire document can be reduced to this invariant stack:

```text
RELEASED .proto AUTHORITY
  |
  +-- Buf format/lint/breaking/build
  |     -> canonical FileDescriptorSet + fingerprint
  |
  +-- Python generation
  |     -> grpc.aio long-lived client
  |
  +-- tonic-prost-build + prost-build
        -> generated Prost messages / Tonic service
             |
             +-- Tonic 0.14.6
             |     -> explicit message limits
             |     -> no default compression/keepalive tuning
             |     -> control concurrency != query admission
             |
             +-- Tokio 1.53.1
             |     -> UnixListener / UnixStream
             |     -> kernel UCred
             |     -> structured task lifetime
             |
             +-- tokio-stream
             |     -> UnixListenerStream / bounded ReceiverStream
             |
             +-- tokio-util CancellationToken
             |     -> query/provider/execution cancellation tree
             |
             +-- Tower only where policy is explicit
                   -> no hidden queue / generic retry

UDS ADMISSION
  filesystem ACL
  -> peer UID/GID/PID where available
  -> short-lived capability credential
  -> agent/workspace/operation authorization

RPC LIFECYCLE
  Handshake
  -> GetStatus / ValidateQuery
  -> StartQuery accepted handle
  -> StreamQuery / AttachQuery monotonic events
  -> CancelQuery
  -> ReadResult bounded chunks
  -> ReleaseResult

DATA POLICY
  Protobuf = control/transport contract
  canonical JSON = semantic contract where released
  Arrow IPC = typed bulk data where negotiated
  large results = immutable externalized resources

PERFORMANCE POLICY
  one channel
  async end-to-end
  bounded messages
  selected Bytes mappings
  native backpressure
  bounded queues
  daemon-owned admission
  precise deadlines/cancellation
  measurement before HTTP/2/compression tuning
```
