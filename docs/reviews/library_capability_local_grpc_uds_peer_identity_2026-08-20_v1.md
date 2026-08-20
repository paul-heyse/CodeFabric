---
artifact: library-capability-research
topic: local-grpc-uds-peer-identity
version: v1
date: 2026-08-20
status: accepted-for-wp05
baseline_commit: a689f1ddf712c0f8fe5cf93d9a50a559f84e4b91
scope:
  - Cargo.toml
  - tooling/proto/
  - src/rpc.rs
  - tests/integration/rpc.rs
  - codefabric-cpg-mcp/
---

# Library Capability Research: Local gRPC UDS Peer Identity

## Decision Summary

Adopt and wrap tonic 0.14.6 with prost 0.14.4, tonic-prost 0.14.6,
tonic-prost-build 0.14.6, protoc-bin-vendored 3.2.0, Tokio 1.x's native Unix
peer-credential API, and hyper-util 0.1.20's Tokio I/O adapter. The stable
CodeFabric boundary is `AuthorizedUnixStream` plus `VerifiedPeerIdentity` and
`SameUserInterceptor`; no tonic transport or Tokio credential type crosses
that project-owned policy boundary.

Python retains grpcio/grpcio-tools 1.83.0 and protobuf 7.36.0 behind the
adapter's private daemon module. Both runtimes set send and receive control
message limits to 4 MiB.

## Capability Requirements

Must-haves from WP05, Serving §8, and AC-G-61:

- generated Protobuf/gRPC clients and servers in Rust and Python;
- private UDS transport on Linux and macOS;
- kernel-supplied peer UID, GID, and available PID captured at accept time;
- verified identity propagated into tonic request extensions;
- missing/mismatched identities rejected before handler dispatch;
- symmetric 4 MiB encode/decode limits;
- no ambient system `protoc`, no first-party unsafe, exact generator identity;
- byte-deterministic committed stubs and cross-language wire compatibility.

Preferences are a small project-owned policy wrapper, stable 1.94.1 compiler
compatibility, and reuse of the already-adopted Tokio runtime. License
evaluation is excluded by explicit user direction.

## Environment and Version Baseline

Observed in the resolved WP05 graph and adapter lock:

| Surface | Resolved identity | Relevant feature/API |
|---|---:|---|
| tonic | 0.14.6 | default transport/codegen; `Connected`; generated size setters |
| prost / prost-build | 0.14.4 | messages; explicit `Config::protoc_executable` |
| tonic-prost / tonic-prost-build | 0.14.6 | current prost codec and tonic codegen split |
| protoc-bin-vendored | 3.2.0 | bundled libprotoc 31.1 and include path |
| Tokio | lock-resolved 1.x | `net`; `UnixStream::peer_cred()` |
| hyper-util | 0.1.20 | `TokioIo` for tonic's Hyper I/O traits |
| grpcio / grpcio-tools | 1.83.0 | UDS client and bundled libprotoc 35.1 |
| protobuf Python | 7.36.0 | generated/runtime contract |

The root is one stable Cargo package on rustc 1.97.1 with declared MSRV
1.94.1. The adapter is an independent uv project on CPython 3.14.7 with a
`>=3.12` package floor.

## Current Code and Custom Infrastructure

Before WP05 there was no gRPC server/client code, Protobuf generator, UDS peer
credential wrapper, or adapter daemon module. Custom code is therefore limited
to policy and orchestration rather than preserved legacy machinery.

## Candidate Matrix

| Candidate | Capability fit | Version basis | Effort/risk | Custom code | Decision |
|---|---|---|---|---|---|
| tonic + Tokio + prost | Full: generated gRPC, UDS incoming streams, `Connected`, limits | Current exact releases and compiled probe | Low-medium; current codegen is split across tonic-build/tonic-prost-build | Thin accepted-stream and policy wrapper | **adopt + wrap** |
| grpcio-rust | Could provide gRPC, but duplicates the selected async/runtime ecosystem and did not displace peer-policy code | Not pinned or otherwise present | Higher integration and migration cost | Similar credential/policy wrapper | reject |
| Custom HTTP/2 + Protobuf framing | Could be built but would recreate cancellation, flow control, status, and codegen integration | No existing implementation | High correctness/security burden | Large transport implementation | reject |
| Custom peer-credential syscall wrapper | Behavior is available from Tokio on Linux/macOS | Tokio's compiled source and probe | Adds first-party unsafe/platform branches | Unnecessary syscall code | reject |

## Verified Capability Findings

1. tonic's `Connected` associated `ConnectInfo` is cloned into request
   extensions for accepted streams. A real UDS call observed the project-owned
   `VerifiedPeerIdentity` in a pre-dispatch interceptor.
2. Tokio's `UnixStream::peer_cred()` returns UID/GID and optional PID on the
   required Linux/macOS family. The wrapper rejects a non-allowed UID while
   constructing the accepted stream, with no first-party unsafe.
3. The generated tonic client and server expose both
   `max_encoding_message_size` and `max_decoding_message_size`. Four behavior
   probes distinguish server decode, client encode, server encode, and client
   decode rejection at 4 MiB.
4. tonic 0.14 uses `tonic-prost-build` for prost-integrated generation and
   `tonic-prost::ProstCodec` in generated code. Treating old tonic-build-only
   examples as the full current surface would be incomplete.
5. prost-build 0.14.4 accepts an explicit protoc path, so the Rust generator
   uses the vendored binary without mutating process-global environment state.
6. tonic 0.14's connector expects Hyper runtime I/O traits; hyper-util's
   `TokioIo` adapts a Tokio `UnixStream` without a custom I/O shim.
7. grpcio-tools and protoc-bin-vendored embed different protoc versions. The
   generator records both identities and the shared-source digest; Rust and
   Python produce the same canonical probe wire bytes.

Primary API documentation: [tonic 0.14.6](https://docs.rs/tonic/0.14.6/tonic/),
[tonic `Connected`](https://docs.rs/tonic/0.14.6/tonic/transport/server/trait.Connected.html),
[prost-build 0.14.4](https://docs.rs/prost-build/0.14.4/prost_build/struct.Config.html),
and [protoc-bin-vendored 3.2.0](https://docs.rs/protoc-bin-vendored/3.2.0/protoc_bin_vendored/).

## Library Decisions

### LD-01 — Adopt tonic/prost's current split

- **Decision:** adopt exact tonic/tonic-prost/tonic-prost-build 0.14.6 and
  prost/prost-build 0.14.4.
- **Boundary:** generated modules remain private transport data; application
  contracts consume project DTOs in later waves.
- **Displaces:** manual gRPC framing, status handling, and service code.
- **Risk:** generated surface changes on upgrades.
- **Mitigation:** exact locks, generated-source digest headers, identity
  manifest, byte-regeneration gate, and behavior probes.
- **Confidence:** high.

### LD-02 — Wrap Tokio peer credentials in a tonic `Connected` stream

- **Decision:** wrap, using Tokio's API rather than direct syscalls.
- **Boundary:** only `VerifiedPeerIdentity` enters request extensions;
  `SameUserInterceptor` owns pre-dispatch policy.
- **Displaces:** platform `SO_PEERCRED`/`getpeereid` unsafe code.
- **Risk:** an incorrectly wired incoming stream could omit metadata.
- **Mitigation:** a real unidentified-stream negative test proves omission is
  rejected and handler instrumentation remains zero.
- **Confidence:** high on Linux/macOS; other platforms are outside profile.

### LD-03 — Adopt vendored generators with explicit dual identities

- **Decision:** adopt protoc-bin-vendored for Rust and grpcio-tools for Python.
- **Boundary:** `tooling/proto/generate.py` is the sole generator orchestrator.
- **Displaces:** ambient `protoc` and hand-maintained stubs.
- **Risk:** the embedded protoc versions differ.
- **Mitigation:** record both, hash the common source, require two isolated
  byte-identical generations, and run a shared wire fixture in both languages.
- **Confidence:** high.

## Custom Code Displaced or Retained

Retained custom code is policy-bearing only: stream admission, same-user
authorization, generator orchestration, identity recording, and drift checks.
Library code owns system calls, HTTP/2, gRPC framing, Protobuf parsing, channel
lifecycle, and generated method dispatch.

## Upgrade and Migration Implications

An upgrade must move tonic, tonic-prost, and tonic-prost-build together, rerun
the UDS/limit suite, regenerate twice, review output drift, and update the
identity manifest. Protobuf Python and grpcio-tools move through the adapter
lock. A generator identity change without output drift is still recorded; an
output change requires contract review.

## Risks and Open Validation

- Linux CI must repeat the macOS-proven UDS path because the credential syscall
  implementation is platform-specific.
- Creating a real process under a different UID is privilege/environment
  dependent. The current deterministic probe varies the authorized UID against
  a real kernel identity and proves pre-handler rejection; a privileged
  cross-user fixture may be added later as a typed platform capability.
- Full Python-to-Rust live gRPC service interop arrives with canonical service
  contracts; Wave 0 proves Python options and shared Protobuf wire bytes.

## Recommended Design Integration

No design correction is required. LD-10 intentionally left this selection to
WP05. Persist these exact versions and the `Connected`/interceptor boundary in
execution state; keep generated product contracts under the D-04 authority and
mirror rules when WP06 replaces the Wave 0 probe.

## Evidence Ledger

| ID | Claim | Status | Evidence | Coverage/limits | Used by |
|---|---|---|---|---|---|
| E-01 | Exact Rust stack resolves on MSRV-compatible releases | observed | Cargo.lock; `cargo metadata --features proto-tooling` | Current macOS resolution | LD-01 |
| E-02 | Same UID reaches handler with request extension | observed | `authenticated_uds_round_trip_propagates_peer_identity` | Real macOS UDS; Linux pending CI | LD-02 |
| E-03 | Missing/mismatched peer does not reach handler | observed | handler counter remains zero in negative UDS tests | Configured UID mismatch, not privileged second user | LD-02 |
| E-04 | All four Rust message-limit directions reject oversize | observed | `rust_client_and_server_apply_symmetric_four_mib_limits` | Exact tonic 0.14.6 | LD-01 |
| E-05 | Generators are versioned and cross-language bytes agree | observed | toolchain-identity.json; shared fixture tests | Wave 0 probe schema | LD-03 |
| E-06 | Generated APIs and limit setters exist | observed | exact generated source and compile probe; tonic docs | Exact pinned version | LD-01 |
