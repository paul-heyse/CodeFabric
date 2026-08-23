# CodeFabric

CodeFabric is a present-state code property graph: a fact substrate over a codebase that
answers semantic questions about what the code is right now.

It emits facts and mechanically derived facts, never judgments such as risk scores or
“safe to refactor.” When a provider cannot answer, the result is an explicit unknown or
capability gap rather than an empty result implying “none.”

## Implementation status

Production implementation is in progress under the versioned plan in `docs/plans/`.
Wave 0 has established all four isolated build domains, their exact dependency/toolchain
identities, the locked Protobuf generator, and cross-domain gates. Contract and runtime
behavior land in Waves 1–3.

## Architecture

Four independently built domains meet across process or generated-contract boundaries:

```text
agent
  → codefabric-cpg-mcp/       Python FastMCP adapter; presentation only
    → private Protobuf/gRPC over a Unix-domain socket
      → root Cargo package   stable Rust daemon and Arrow/Delta/DataFusion data plane
           ├─ rustc-extractor/   dated-nightly rustc/MIR subprocess
           └─ pyrefly-sidecar/   pinned Pyrefly semantic subprocess
```

The root package is one rlib crate, edition 2024 with Rust 1.95.0 as its verified
compatibility floor. It has no native-extension build surface or root Python package. Its default
`local-workstation` accepts only local filesystem storage and excludes the Delta S3
implementation and AWS SDK; `s3-storage` enables them explicitly. The pinned Delta
kernel still compiles latent `object_store` cloud features, which the graph and advisory
policy checks report rather than concealing. Narrow `canonical-json`, `contract-models`,
`model-compiler`, `proto-tooling`, `rpc`, `repository-state`, and `data-fabric`
features keep focused tools from compiling unrelated production subsystems.

The additional Cargo roots are not a Cargo workspace. Their separate toolchains and
dependency isolation are build-domain requirements, not semantic source organization.

## Supported baseline

| Surface | Baseline |
|---|---|
| Stable daemon/data plane | Linux and macOS; Rust 1.95.0 or newer |
| rustc extractor | `nightly-2026-08-18`; exact compiler identity recorded |
| Pyrefly sidecar | Pyrefly 1.2.0 at an immutable source revision |
| FastMCP adapter | Python 3.14.7 development pin; package floor 3.12 |

## Bootstrap

```bash
just doctor      # toolchains, domain presence, required tools, and direnv state
just --list      # the operational API
just ci-fast     # current routine gate
```

The repository requires `just`, `sccache`, `cargo-nextest`, `typos`, `rg`, `ast-grep`,
`jq`, and `uv`. Dependency gates additionally use `cargo-deny`, `cargo-audit`,
`cargo-shear`, and `cargo-machete`. `sccache` is a committed `rustc-wrapper`, so Cargo
fails rather than silently running without it.

Stable root and Pyrefly-sidecar builds share the repository `target/` and the host-global
sccache. Dated-nightly extractor, nightly assurance, and sanitizer/fuzz artifacts use
separate target subdirectories. CI disables Cargo incremental compilation for better
sccache reuse; local incremental compilation remains enabled.

`direnv` is optional and only affects interactive shells. It syncs the adapter's locked
environment and never creates a root Python project. Non-interactive callers should use
`direnv exec . <cmd>` or source `scripts/bootstrap.sh` within the command.

## Common commands

`just --list` is authoritative. The everyday commands are:

```bash
just root-check           # default local and featureless stable-root builds
just root-clippy          # warnings denied on both root surfaces
just root-test            # nextest plus doctests
just adapter-ci-fast      # Ruff, Pyrefly, pytest, and STDIO discipline
just extractor-ci-fast    # dated-nightly extractor gate
just sidecar-ci-fast      # stable pinned-source sidecar gate
just model-family-check   # typed render plus independent staged consumers
just model-repro-check    # exact two-root DesiredTree reproduction
just model-assurance-check # live Just/test/rule evidence and profile soundness
just stable-graph-check   # exact pins/families and local-vs-S3 activation boundary
just governance           # model-derived structure, provenance, and zero-state checks
just ci-fast              # routine four-domain aggregate gate
```

Recipes in the `mutating` group change source, manifests, or the environment and are
never dependencies of a gate. Note that `cargo nextest` does not execute doctests;
`just root-test` covers both layers.

## Repository map

| Path | Contents |
|---|---|
| `src/` | stable daemon/data-plane library source |
| `tests/` | one Rust integration target; cases under `tests/integration/` |
| `rustc-extractor/` | dated-nightly extractor domain |
| `pyrefly-sidecar/` | pinned Pyrefly sidecar domain |
| `codefabric-cpg-mcp/` | Python FastMCP adapter project and its own `uv.lock` |
| `contracts/` | AC-G-05 authority, generated registries, and cross-language fixtures |
| `fuzz/` | native-target JCS parser/canonicalizer fuzz harness |
| `tooling/proto/` | hermetic Protobuf generation and version identity |
| `scripts/` | operational scripts too substantial for a `just` recipe |
| `rules/`, `sgconfig.yml` | structural governance rules and scan configuration |
| `docs/upfront_design/` | authoritative system design suite |
| `docs/library_ref/` | version-pinned dependency references |
| `docs/spec_index/` | derived navigation and traceability; never normative |
| `docs/plans/`, `docs/reviews/` | implementation plans, execution state, and audits |
| `tooling/ast-grep/` | document-navigation extractors for `spec-outline`/`lib-outline` |
| `.claude/`, `.codex/`, `.agents/` | shared agent configuration and skills |
| `target/` | generated build output and reports; ignored |

## Governing documents

The authoritative system design is the v1.3 suite under `docs/upfront_design/`; the
roadmap composes its implementation waves. `AGENTS.md` documents the repository’s
tooling, assurance model, design map, and operating rules. The older
`docs/rust_core_python_interface_repository_specification_2026-08-20.md` remains the
infrastructure source for compatible decisions, but accepted v4 plan decisions and the
v1.3 design corrections govern where they deliberately replace the seed-era extension shape.
