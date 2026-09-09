# Repository infrastructure reference

Updated 2026-09-08. Consult this implementation/tooling rationale when relevant.
Current workflow instructions live in [AGENTS.md](../AGENTS.md); current behavior and
known failures live in [STATUS.md](../STATUS.md). Historical measurements below are
not current product certification. Runtime features/pins describe the installed graph,
not completed implementation of the revised target.

## 1. Scope boundary — what was *not* decided

The infrastructure specification still does not decide semantic source decomposition:

- which production concepts deserve their own Rust source file;
- whether a concept is a file, an inline module, a nested module, or a folder;
- how domain functionality is grouped or named;
- whether the FastMCP adapter is one Python module or several;
- naming conventions for semantic production directories.

Do not infer a preferred domain decomposition from temporary compatibility modules.

Do not create another Cargo root or package merely to organize code. The two additional
Cargo roots accepted by the design have explicit build justifications: the extractor uses
a distinct dated-nightly compiler-private toolchain, and the sidecar isolates a pinned
unstable Pyrefly integration. “Another conceptual area” and “the file got long” remain
insufficient.

---

## 2. Repository shape

Four independent build domains, no root `[workspace]` and no `crates/` directory:

```
CodeFabric/
├── Cargo.toml  Cargo.lock  rust-toolchain.toml  stable daemon/data-plane package
├── src/                                        library + thin codefabric/codefabricd binaries
├── tests/                                      one stable-root integration-test target
├── rustc-extractor/                           dated-nightly Cargo root
├── pyrefly-sidecar/                           pinned-source Cargo root
├── codefabric-cpg-mcp/                        Python adapter + local uv.lock
├── contracts/                                 released wire/fixture inputs + bounded legacy products
├── fuzz/                                      JCS parser/canonicalizer harness
├── tooling/proto/                             hermetic Protobuf generator
├── justfile                                   repository operational API
├── scripts/                                   bootstrap, validators, doc outliners
├── sgconfig.yml  rules/                       structural governance harness
├── tooling/ast-grep/        doc-navigation extractors (§10.1)
├── docs/                    the governing spec, upfront_design/, library_ref/,
│                            spec_index/, plans/, reviews/
├── .claude/                 settings.json (permissions + SessionStart hook) · skills/
├── .codex/                  config.toml · hooks/ · skills -> ../.claude/skills
├── .agents/skills           -> ../.claude/skills  (the path Codex documents)
├── .cargo/config.toml       sccache wrapper + shared stable target
├── .config/nextest.toml     test profiles
├── .github/workflows/ci.yml four-domain + contracts/governance CI
├── bacon.toml  deny.toml  _typos.toml  clippy.toml  .envrc
└── target/                   generated and ignored, including agent context caches
```

**Absent on purpose** — each has a named trigger that would justify creating it:

| Not here | Add it when |
|---|---|
| `crates/`, root `[workspace]` | a new package clears repo-spec §0.3 independently |
| `benches/` | a stable performance workload exists worth defending (§1) |
| `supply-chain/` (cargo-vet) | dependency trust becomes an engineering goal (§32) |
| `tests/fixtures/` | a test needs reusable non-code data (§4.4) |
| `deep-assurance.yml` | the surfaces it covers exist; a permanently-red workflow is worse than none (§52) |
| `src/main.rs`, additional `src/bin/*` | another operational shell clears repo-spec §3 independently; reuse the existing lib target |
| Retired plan certification scripts | No trigger: ordinary Markdown and Git replace plan activation/state machinery |

### 2.1 Why the domains are separate

The stable root owns source state, Arrow/Delta/DataFusion processing, snapshots, and query
execution. `rustc-extractor/` owns compiler-private Rust facts; `pyrefly-sidecar/` owns the
pinned Pyrefly query integration; `codefabric-cpg-mcp/` owns presentation only. The three
process boundaries use generated Protobuf contracts. Python never becomes an Arrow or
DataFusion processing layer, and no native-extension build surface exists.

### 2.2 Rust test topology

Cargo compiles **every** top-level `tests/*.rs` as its own integration-test crate. So
there is exactly one: `tests/integration.rs`, whose cases live in `tests/integration/`
(repo-spec §4.2, §61.3). A second top-level target needs a materially different feature
set, process environment, harness, external service, platform restriction, or resource
group (§4.3) — not just another subsystem.

The inline `mod integration { … }` wrapper inside `tests/integration.rs` is load-bearing:
a test crate root resolves submodules against `tests/`, so without it case modules resolve
at the wrong level and encourage top-level test-target proliferation.

Most tests are colocated `#[cfg(test)]` modules beside the implementation, so private
invariants are testable without widening visibility (§4.1).

---

## 3. The architectural invariant: isolated build domains

```
agent → attach-only Rust launcher → FastMCP adapter → private UDS gRPC
       WorkspaceSupervisor → one stable Rust daemon per workspace
                              ├─ rustc extractor subprocess
                              └─ Pyrefly sidecar subprocess
```

The root keeps one default production aggregate and exposes narrow build capabilities for
tools and assurance:

```toml
[features]
default = ["local-workstation"]
canonical-json = ["dep:base64", "dep:blake3", "dep:serde", "dep:serde_json", "..."]
contract-models = ["canonical-json", "dep:serde_yaml_ng"]
provider-contracts = ["contract-models", "dep:arrow-array", "dep:arrow-schema", "dep:thiserror"]
release-compiler = ["provider-contracts"]
fact-generation = ["provider-contracts", "dep:tree-sitter", "dep:ruff_python_parser", "..."]
data-fabric = ["provider-contracts", "dep:arrow", "...", "dep:datafusion", "dep:deltalake", "..."]
repository-input = ["contract-models", "dep:gix", "dep:rustix", "dep:url"]
operational-state = ["contract-models", "dep:rusqlite", "dep:rustix", "dep:url"]
rpc = ["dep:prost", "dep:tokio", "dep:tonic", "dep:tonic-prost"]
semantic-release = ["release-compiler", "fact-generation", "data-fabric"]
compatibility-probes = ["canonical-json", "data-fabric", "repository-input", "operational-state", "rpc"]
local-workstation = ["daemon", "compatibility-probes"]
s3-storage = ["data-fabric", "deltalake/s3"]
```

| Surface | Command | Dependency graph |
|---|---|---|
| local workstation | `cargo check --all-targets` | local provider authority; no `deltalake-aws` or AWS SDK |
| featureless substrate | `cargo check --all-targets --no-default-features` | dependency-free root substrate |
| canonical JSON | `cargo check --no-default-features --features canonical-json` | strict JSON/JCS only; no data fabric, repository, or RPC |
| contract models | `cargo check --no-default-features --features contract-models` | runtime wire models only; no compiler or generated-output closure |
| provider contracts | `just feature-architecture-check provider-contracts` | application-owned jobs/results and minimal Arrow types; no provider, fabric, state, RPC, or daemon closure |
| release compiler | `just feature-architecture-check release-compiler` | fallible behavior-bearing program compilation; no provider, fabric, state, RPC, or daemon closure |
| Protobuf tooling | `cargo check --no-default-features --features proto-tooling --bin codefabric-proto-gen` | generator-only graph |
| S3 deployment | `cargo check --all-targets --features s3-storage` | explicit delta-rs S3 graph |

`scripts/stable_graph_check.sh` verifies the actual resolved version and feature graph,
including a single Arrow/Parquet/DataFusion/object-store/kernel universe, exact delta-rs
and gix identities, the kernel-forced latent `object_store` features, and the
local-vs-S3 activation boundary. It also proves narrow feature graphs omit unrelated
heavy package families and that stable root/sidecar target sharing does not cross the
dated-nightly extractor boundary. Compiled capability is not provider authority.

---

## 4. Manifests, pins, and the decisions inside them

### Rust

| Decision | Value | Why |
|---|---|---|
| root toolchain | exact `1.98.0` | reproducible stable daemon/data-plane boundary; upgrade deliberately |
| components | `rustfmt`, `clippy`, `rust-analyzer`, `rust-src`, `llvm-tools-preview` | `llvm-tools-preview` is the substrate for coverage, binutils and fuzz-coverage (tooling-ref §8); `rust-src` gives semantic tools stdlib source (§7) |
| extractor toolchain | `nightly-2026-08-18` in its own root | owns `rustc-dev`; it never contaminates the stable root |
| `rust-version` | `1.95.0` | floor imposed by the Ruff 0.0.7 provider train (above delta-rs's 1.94.1 floor) and verified with `cargo msrv verify` |
| `license` | **not declared** | no license chosen yet; `publish = false` stops Cargo warning. licensing is excluded from routine dependency-policy checks |
| lints | `unsafe_code = "deny"`, clippy `all` + `pedantic` = warn | there is no first-party `unsafe`; preserving that is a useful default (§33) |
| `clippy.toml` | five `doc-valid-idents` words + the `..` defaults marker | `pedantic` enables `doc_markdown`, which wants backticks around proper nouns in prose. The list names known-good words — it does **not** disable the lint (§9.1) |
| dev profiles | `debug = "line-tables-only"`, deps `debug = false` | useful source locations without inflating `target/` (§9.2) |
| extra profiles | `debugging` (full debug), `profiling` (release + symbols, `strip = "none"`) | profiling needs release codegen with symbols preserved (§40.2) |
| release tuning | **none** | `lto`, `codegen-units = 1`, `panic = "abort"`, `strip` are not added by folklore; measure first (§9.3) |

### Dependency baseline

| Decision | Value | Why |
|---|---|---|
| Arrow/Parquet | `59.2.0` | one public type universe |
| DataFusion | `55.0.0` | query, catalog, execution |
| object_store | `0.13.2` | canonical storage abstraction |
| delta-rs | git rev `43a0cf10…` | pinned pre-release; compile-probed |
| gix | `0.86.0`, narrow read profile + SHA-256 probe | present-state accelerator, never byte authority |
| SQLite | `rusqlite 0.40.2`, `bundled` + `backup` | operational state and online backup |
| safe filesystem | `rustix 1.1.4`, `fs` | descriptor-relative authoritative reads |
| Rust gRPC/Protobuf | tonic/tonic-prost `0.14.6`, prost `0.14.4` | UDS transport, generated service boundary |
| Descriptor/Rust generator | grpcio-tools `1.83.0` + tonic-prost-build `0.14.6` `compile_fds` | one pinned compiler emits Python and the descriptor IR; Rust consumes that exact IR |

The root and both auxiliary Cargo locks are committed. Python has one domain-local lock,
`codefabric-cpg-mcp/uv.lock`; there is no root uv project. The single grpcio-tools
libprotoc 35.1 identity, committed descriptor digest, and Rust `compile_fds` API are
recorded under `tooling/proto/`; descriptor equivalence and shared-wire interoperability
are both proved.

---

## 5. The command contract

`just --list` is the first thing to read (repo-spec §14, §59, §92). Recipes express
**intent**, not tool flags, so implementations can change without invalidating what
callers know. Current groups:

`environment` · `static` · `test` · `extractor` · `sidecar` · `adapter` · `contracts` ·
`gate` · `quality` · `compat` · `supply-chain` · `perf` · `mutating`

Two rules govern all of it:

1. **Mutating recipes are never dependencies of a gate** (§14.1). The five that
   change state — `root-fmt-write`, `proto-gen`, `typos-write`, `snapshots-accept`,
   `deps-fix` — must be invoked deliberately and their diff inspected; four carry
   `[confirm(...)]` prompts.
   (`tool-updates-check` sits in the group for visibility but only lists available
   updates.)
2. **Availability is not a mandate to run** (§73.1). Pick the smallest tool set that
   answers the risk question in *Choose evidence by change risk* below, then escalate.

The justfile sets `positional-arguments` so variadic recipes forward `"$@"` rather than
`{{args}}`. Without it, `just` re-expands the interpolated string and a quoted argument
containing a space is silently re-split — `just spec-outline <path> --match '^5. Authority'`
would search for `^5.` and treat `Authority` as a second path, returning a wrong outline
instead of an error.

---

## 6. Environment and feedback layer

### sccache is a hard prerequisite

`.cargo/config.toml` commits `scripts/sccache-wrapper.sh` as `rustc-wrapper`. The wrapper
requires the repository's supervised per-user sccache service; an unavailable service
fails immediately with `just setup-sccache` rather than silently compiling uncached.
`just doctor` verifies the current version, generated configuration, UDS endpoint, and
storage contract while reporting cumulative error/timeout telemetry. `just
sccache-canary` proves only transport/storage liveness with a repeated cacheable `rlib`
compile. `just sccache-effectiveness` is the opt-in Cargo-shaped cold-target/warm-cache
measurement; `just cache-stats` reports advanced statistics. A cumulative hit percentage
alone is not performance evidence. `tooling/rust-tool-versions.env` is the single exact
workstation/CI CLI manifest; `just tools-doctor` checks it and `just setup-tools`
idempotently reconciles only missing or mismatched tools.

Nothing host-specific belongs in that file — no `-C target-cpu=native`, no absolute paths,
no one machine's linker.

`cargo clean` is not routine hygiene (§13.3). Reserve it for suspected stale artifacts,
controlled clean-build measurement, reclaiming disk, or isolating a profile/feature
interaction. A large `target/` is not evidence that a release artifact is large.

The cache and artifact topology is deliberate:

- local sccache uses a launchd/systemd-user supervised UDS service with a dedicated 40 GiB
  cache on local SSD. The stable socket directory is owned by the current uid with mode
  0700 so it satisfies the exact Codex UDS permission without cross-user access. Setup is
  serialized, removes only the exact stale socket before service start, compares generated
  files, and does not restart a healthy unchanged daemon. The systemd user unit deliberately
  has no `After=default.target` cycle or `RuntimeDirectory`, applies `UMask=0077`, and passes
  the same fixed socket identity through both `ExecStart` and `SCCACHE_SERVER_UDS`. The
  entrypoint rejects disagreement, `just doctor` rejects an installed-unit/config drift,
  and every setup migration proves a real repeated compile before succeeding;
- client-side mode performs cache reads in the compiler process, so the Codex sandbox only
  needs read access to the cache plus access to the single service socket;
- sccache 0.17.0 does **not** apply `SCCACHE_BASEDIRS` to Rust keys. Distinct absolute
  worktree roots may therefore miss for workspace crates; keep independent target trees
  and do not claim cross-worktree Rust path normalization;
- the wrapper sends Cargo's non-compiling `rustc -vV`, version, and `--print` discovery
  queries directly to rustc before consulting sccache. Cargo persists both successful and
  failed discovery output in `target/.rustc_info.json`, so allowing transient service state
  to fail those queries can replay a repaired error. The canary rejects a cached sccache
  failure with a targeted instruction to remove only that generated file, never the whole
  Cargo target;
- stable root and stable Pyrefly-sidecar builds share the repository `target/`;
- the dated-nightly extractor uses `target/extractor/`;
- Miri/udeps use the exact `nightly-2026-08-18` toolchain and
  `target/nightly-assurance/`;
- cargo-fuzz uses `target/fuzz/<nightly-host>/` and explicitly selects the native host.

Incremental policy is workflow-specific. Just establishes `CARGO_INCREMENTAL=0` for
compile-producing build/test/gate paths, as does CI. Local `root-check`, `root-clippy`,
`root-test-incremental`, `extractor-check`, and `sidecar-check` explicitly restore rustc
incremental compilation;
they also bypass sccache because 0.17.0 rejects incremental Rust invocations instead of
passing them through, while ordinary check units omit `link` anyway. The committed wrapper
recognizes Cargo's incremental compiler shape and routes only that incompatible invocation
directly to the real rustc, so raw Cargo retains safe profile defaults; named recipes remain
the supported reproducible command surface. `just build-shared` and `just
build-incremental` expose the apples-to-apples build comparison; `just
sccache-effectiveness` uses repeated Hyperfine samples, and `just linker-benchmark`
compares the pinned default linker with mold without changing repository defaults. The
wrapper remains
mandatory in compile-producing routine workflows; no-wrapper paths are explicit check,
controlled measurement, or diagnosis commands.

Cargo 1.98.0 still rejects `build.build-dir` as an unstable Cargo surface, so it is not
committed or benchmarked through a rolling nightly. Revisit it only after the pinned stable
Cargo exposes the setting; keep CI registry/target caching similarly measurement-gated.

### One continuous checker

Ownership is divided (repo-spec §15.1, tooling-ref §72.22):

```
agent LSP      →  semantic model, references, types, assists
Bacon          →  the persistent cargo check job          (bacon.toml)
just root-check-fast → on-demand library type-check for a hot edit loop
Watchexec      →  non-Rust tasks and process restarts only
```

Do not configure editor check-on-save, Bacon, and Watchexec to run the same
`cargo check`.

**The Cursor rust-analyzer extension is deliberately disabled.** It violated the rule
above: with `checkOnSave`, `cargo.allTargets` and `check.workspace` all at their `true`
defaults and three entries in `linkedProjects`, every trigger started `cargo check
--workspace --all-targets` *once per linked project* — three concurrent cargo processes,
one on the dated nightly — into the shared repository `target/`, contending for its Cargo
build lock with the CLI and with agent shells. Confirmed 2026-08-30 by PPID chain, not
inference. `.vscode/settings.json` retains `cargo.targetDir` and `check.extraEnv` so the
configuration is correct if the extension is ever re-enabled.

Agents keep a semantic model through their own rust-analyzer (Claude Code ships one as a
plugin), so nothing in the ladder of §10.1 is lost. What is deferred to an explicit check
is what only rustc produces: borrow-check and lifetime errors, the `unused_imports` /
`dead_code` / `unused_variables` lints, and Clippy.

Measured cost of a check on the stable root, 2026-08-30, hyperfine 1.20.0, n=5, warm
target, after a one-line edit to a private fn: `cargo check --lib` 6.22s ±0.31,
`cargo check --all-targets` 10.23s ±0.96. Against the repository `target/`, `just root-check-fast` is ~7.5s and `just root-check` ~15s, the latter because it checks two surfaces. A no-op check is ~1s for either recipe. Populating a cold check
cache is a one-time ~2min — check units (`--emit=metadata`) and test-build units
(`--emit=link --test`) are separate unit families, so a stretch of test-only work leaves
the check cache cold. Two things measured as **non**-levers and must not be re-litigated
without new evidence: the sccache wrapper (sccache records *zero* compile requests on
check workloads — every unit carries `-C incremental=` and the wrapper execs straight to
rustc), and `RUSTC_WRAPPER` toggling (it does not enter Cargo's fingerprint; alternating
`just` and raw `cargo` rebuilds nothing).

`bacon.toml` defines stable-root `check`, `clippy`, and `nextest` jobs through the same
incremental/no-wrapper helper, uses `--locked`, and exports
`.bacon-locations` for editor/agent consumption — an agent must confirm that file matches
the current source generation before reading an empty list as success (§15.2).

### Shell environment — recipes are the boundary

Every supported Just recipe must work from a fresh non-interactive shell without `direnv`,
sourcing a bootstrap script, activating Python, or manually choosing a Rust toolchain.
`scripts/repo-shell.sh` removes inherited Python/Conda/direnv/Rust/Cargo/linker/sccache
overrides, puts the
repository Cargo router and Rustup first, and places uv's cache under `target/uv-cache`.
Stable Cargo resolves through `rustup run 1.98.0`; extractor and assurance Cargo resolve
through `rustup run nightly-2026-08-18`.

Workstation shell startup also reorders `~/.cargo/bin` ahead of Homebrew in login and
non-login zsh/Bash. This is a convenience, not gate authority: the repository Cargo router
still pins both Cargo and rustc/rustdoc to the selected Rustup toolchain so inherited PATH
ordering cannot mix installations.

Root `.envrc` only exports `CF_ROOT`, the repository uv-cache location, and convenient
tool paths; it does not run `uv sync` or activate the adapter. Use the idempotent
`just setup` (or a focused setup recipe) explicitly. `scripts/bootstrap.sh` is a verifier and
context reporter: execution re-enters the canonical repository shell, sourcing it
intentionally changes nothing, `--quiet` is silent when healthy, and `--context` emits the
agent report. Python commands remain domain-explicit;
there is no root uv project.

---
