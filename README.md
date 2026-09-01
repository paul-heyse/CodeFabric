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
`proto-tooling`, `rpc`, `repository-state`, and `data-fabric`
features keep focused tools from compiling unrelated production subsystems.

The additional Cargo roots are not a Cargo workspace. Their separate toolchains and
dependency isolation are build-domain requirements, not semantic source organization.

## Supported baseline

| Surface | Baseline |
|---|---|
| Stable daemon/data plane | Linux and macOS; exact development toolchain 1.98.0, verified MSRV 1.95.0 |
| rustc extractor | `nightly-2026-08-18`; exact compiler identity recorded |
| Pyrefly sidecar | Pyrefly 1.2.0 at an immutable source revision |
| FastMCP adapter | Python 3.14.7 development pin; package floor 3.12 |

## Bootstrap

```bash
just doctor      # toolchains, domain presence, required tools, and direnv state
just setup       # exact tools + locked adapter environment + supervised sccache service
just --list      # the operational API
just ci-fast     # current routine gate
```

The exact CLI identities live in `tooling/rust-tool-versions.env`; `just tools-doctor`
checks them and `just setup-tools` reconciles mismatches with cargo-binstall. The
repository requires `just`, `sccache`, `cargo-nextest`, `typos`, `rg`, `ast-grep`,
`jq`, and `uv`. Dependency gates additionally use `cargo-deny`, `cargo-audit`,
`cargo-shear`, and `cargo-machete`. `sccache` is a committed wrapper backed by a
supervised per-user service, so Cargo fails with a setup instruction rather than silently
running uncached. `just sccache-canary` proves a repeated Rust compile is a real cache hit.
It is a transport/storage liveness probe, not a repository performance claim; use the
opt-in `just sccache-effectiveness` for repeated Hyperfine cold-target/warm-cache samples.
`just linker-benchmark` compares the pinned default linker with mold without changing the
committed linker configuration. The service binds the exact fixed socket permitted by the
Codex sandbox; setup and doctor reject a symlink, a runtime-directory substitution, or any
disagreement between the supervisor and entrypoint socket identities.

Stable root and Pyrefly-sidecar builds share the repository `target/`. Dated-nightly
extractor, exact dated-nightly assurance, and sanitizer/fuzz artifacts use separate target
subdirectories. Compile-producing Just recipes and CI set `CARGO_INCREMENTAL=0` so sccache
can reuse complete compiler outputs. Local check and Clippy recipes retain rustc
incremental feedback and explicitly bypass sccache because ordinary `cargo check` units
are not a cache-hit workload and sccache 0.17.0 rejects incremental Rust invocations.
The committed wrapper likewise routes Cargo's incremental compiler shape directly to the
real rustc so an ordinary local Cargo profile does not fail; named Just recipes remain the
reproducible command contract.
Released sccache 0.17.0 remains sensitive to absolute Rust checkout paths, so parallel
worktrees keep independent target trees and do not claim `SCCACHE_BASEDIRS` normalization.

Every Just recipe establishes its own clean environment: repository-local uv cache,
domain-explicit Python project, Rustup-owned Cargo, and no inherited virtualenv, Conda,
direnv, toolchain, compiler-wrapper, flags, linker, sccache backend, or build-directory
overrides. Use `just <recipe>` directly
from interactive and non-interactive shells. Root `direnv` is optional convenience only;
it does not sync or activate Python. `scripts/bootstrap.sh` verifies and reports state but
does not mutate the caller's environment and executes its checks through that same clean
recipe shell.

The workstation cache is a supervised, disk-only 40 GiB cache. CodeFabric does not add a
remote cache or distributed compilation layer for the single-developer workstation.

## Common commands

`just --list` is authoritative. The everyday commands are:

```bash
just root-check           # default local and featureless stable-root builds
just root-clippy          # warnings denied on both root surfaces
just root-test            # nextest plus doctests
just adapter-ci-fast      # Ruff, Pyrefly, pytest, and STDIO discipline
just extractor-ci-fast    # dated-nightly extractor gate
just sidecar-ci-fast      # stable pinned-source sidecar gate
just stable-graph-check   # exact pins/families and local-vs-S3 activation boundary
just governance           # structure, provenance, and zero-state checks
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
| `contracts/` | released wire/fixture inputs and bounded predecessor products; not current semantic authority |
| `fuzz/` | native-target JCS parser/canonicalizer fuzz harness |
| `tooling/proto/` | hermetic Protobuf generation and version identity |
| `scripts/` | operational scripts too substantial for a `just` recipe |
| `rules/`, `sgconfig.yml` | structural governance rules and scan configuration |
| `docs/authoritative_design/` | authoritative system design suite |
| `docs/library_ref/` | version-pinned dependency references |
| `docs/spec_index/` | derived navigation and traceability; never normative |
| `docs/plans/`, `docs/reviews/` | implementation plans, execution state, and audits |
| `tooling/ast-grep/` | document-navigation extractors for `spec-outline`/`lib-outline` |
| `.claude/`, `.codex/`, `.agents/` | shared agent configuration and skills |
| `target/` | generated build output and reports; ignored |

## Governing documents

The authoritative system design is the discovered
`codefabric-relational-data-fabric` v2.0 suite under `docs/authoritative_design/`; its roadmap
orders capability stages and the active versioned implementation plan owns packet execution.
The adjacent v1.3 masters remain historical transition evidence, not a coequal target.
`AGENTS.md` documents the repository tooling, assurance model, design map, and operating rules.
The older `docs/rust_core_python_interface_repository_specification_2026-08-20.md` remains the
infrastructure source for compatible decisions unless the current suite explicitly supersedes
one of its semantic realization premises.
