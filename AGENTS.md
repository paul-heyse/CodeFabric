# AGENTS.md — repository infrastructure

What is in this repository, why each piece was chosen, and what it does and does not prove.

This file documents the **package, tooling, and assurance architecture**. It is the
answer to "what can this repo do, and what did we already decide?" It deliberately says
nothing about how production code should be organized — see *Scope boundary* below.

| File | Answers |
|---|---|
| **AGENTS.md** (this file) | **the canonical agent instructions.** Infrastructure decisions, the design corpus, doctrine, evidence model, tooling. Codex loads it directly; Claude Code loads it through a `@AGENTS.md` import in `CLAUDE.md`, so both agents read exactly this. |
| `README.md` | human onboarding: what CodeFabric is, how to install it, supported platforms |
| `CLAUDE.md` | a thin shim — the import above, plus Claude-specific harness notes |
| `docs/upfront_design/` | the design corpus: governance manifest, six domain specs, implementation roadmap — see *Design corpus map* below |
| `docs/spec_index/` | navigation and traceability over that corpus; derived, never normative |
| `docs/rust_core_python_interface_repository_specification_2026-08-20.md` | **the governing spec.** Authoritative for everything here |
| `docs/library_ref/rust_development_environment_tooling_agent_reference_2026-08-19.md` | per-tool capability reference |

**Every `§N` below is a section of one of the last two documents** — `repo-spec §N` for the
governing specification, `tooling-ref §N` for the tooling reference, and a bare `§N` means
repo-spec. Design-corpus citations carry their own artifact tag (`SUITE §N`, `RM §0`, …),
the convention `docs/spec_index/README.md` fixes. This file's own sections are referred to by name, never by number. Most config
files in this repository also carry their own rationale comments; this file is the
cross-cutting view.

> **Status: implementation in progress.** The four Wave 0 build domains, dependency
> boundaries, adapter shell, Protobuf/UDS compatibility harness, and aggregate gates are
> present. The versioned execution state under `docs/plans/state/` is authoritative for
> packet and milestone progress.

---

## 0. Design corpus map — what is being built

The system itself is specified in two sibling directories. They are **design inputs, not
infrastructure**: deliberately not an input to any package/repository/tooling decision here
(see *Scope boundary*), revised in place while in flux, and navigated with
`just spec-outline` rather than read whole.

`docs/upfront_design/` — the authoritative suite: a governance manifest, six domain
specifications, and the implementation roadmap.

| Tag | File | What it holds |
|---|---|---|
| `SUITE` | `codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md` | who owns what: artifact precedence, all 84 `AC-G` contract owners, the generated `contracts/` tree, Readiness Gates A–G |
| `ONT` | `code_property_graph_present_state_fact_ontology_specification_v1.3.md` | what facts exist: language-neutral core ontology + Python and Rust profiles |
| `GEN` | `present_state_cpg_fact_generation_specification_python_rust_v1.3.md` | how facts are produced: Tree-sitter / Ruff / Pyrefly / `rustc_public` provider stack, reconciliation, derived analyses |
| `FAB` | `present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md` | how facts are stored and served: Arrow schemas, Delta tables, DataFusion catalog; `FAB §2.1` pins the Cargo dependency baseline |
| `QRY` | `code_property_graph_semantic_query_specification_v1.3.md` | how agents ask: semantic-first JSON envelope, eight request forms |
| `LIFE` | `codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md` | how it stays current: watcher → update waves → snapshots → publication; runtime topology |
| `SRV` | `present_state_cpg_fastmcp_serving_specification_v1.3.md` | how agents connect: one FastMCP STDIO process per agent over the Rust daemon boundary |
| `RM` | `codefabric_1.3_implementation_roadmap_v1.0.md` | in what order: waves W0–W19; subordinate to the specs and manifest (`RM §0`) |

`docs/spec_index/` — a derived navigation and traceability layer over those eight. **Never
normative**: cite the section it points at, not the index.

| File | Answers |
|---|---|
| `README.md` | citation convention, suite census, the `# Part`/`# Appendix` structure `spec-outline` cannot see, gap register |
| `fact-domain-map.md` | one fact domain traced across all six domain specs |
| `library-routing.md` | which `docs/library_ref/` chapter covers a given spec section; the version-pin ledger |
| `wave-traceability.md` | which spec sections and contracts each roadmap wave implements |
| `contract-census.md` | all 84 `AC-G` contracts with owner, consumers, and wave |
| `invariants-and-doctrine.md` | the invariants every wave must preserve, traced to their normative homes |

### 0.1 How the suite composes

Read the six domain specs as a stack: each layer consumes the one above, the ontology is the
root vocabulary, and the governance manifest sits across all of them. The end-to-end data
path (`GEN §6` + `LIFE §93` + `FAB §1`):

```text
source change → dirty registry → update wave → source images → invalidation plan
  → fast syntax lane (Tree-sitter)           → immutable syntax-current snapshot
  → semantic lane (Ruff+Pyrefly / rustc+MIR) → normalization → reconciliation
  → derived lane (petgraph, fixed-point)     → interprocedural summaries
  → validated immutable hot snapshot         → async Delta publication → DataFusion serving
```

Runtime topology: one central **Rust daemon per workspace** owns source state, snapshots,
provider orchestration, query execution and capability status; one **FastMCP STDIO process
per agent** is presentation only and must never hold independent mutable CPG state
(`LIFE §122`).

Consult `docs/spec_index/`'s gap register before concluding a search failed: several cited
authorities — requirement IDs, the flag registry, property names — are build outputs under
`contracts/`, not prose that exists anywhere yet.

### 0.2 Cross-cutting doctrine

Violating any of these contradicts every spec at once.

- **Fact substrate, not judgment.** The system emits facts and mechanically derived facts. It
  never encodes `SAFE_TO_REFACTOR`, `TEST_IMPACTED`, `HIGH_RISK`, `SHOULD_CHANGE`, complexity
  verdicts, or test-impact conclusions. The query service *rejects* evaluative requests; the
  fact-equivalent form is the allowed rewrite. Excluded domains: git history, runtime
  observation/coverage, environment inventory.
- **Absence is never proof of absence.** Missing provider output must materialize as an
  *explicit unknown* or *capability gap*, never an empty result implying "none". Compile
  failure yields capability gaps, not stale-current compiler facts.
- **Raw and normalized coexist.** Every syntax/MIR fact keeps both the provider-native kind
  and the normalized kind; normalization must not block representing a new grammar or
  compiler variant.
- **Syntax occurrence ≠ semantic entity.** Call syntax is not a callable; type syntax is not
  a type. Call sites are first-class entities, not just caller→callee edges.
- **Canonical identity is application-owned.** Raw `DefId`, MIR local/block indices,
  Tree-sitter node IDs, Ruff node indices and Pyrefly internal keys are never canonical
  identity (`GEN §13`). Rust prefers `StableCrateId + DefPathHash`.
- **Provider isolation.** Every provider sits behind an application-owned adapter emitting
  application-owned DTOs; no long-lived borrowed provider type (e.g. `Node<'tree>`) escapes
  an adapter.
- **Authority, never silent overwrite.** Conflicting provider facts resolve by the
  per-fact-family authority tables (`GEN §5`); conflicting evidence is retained in
  provenance/diagnostics, and unresolvable conflict emits an unknown or multi-candidate fact.
- **Atomic present state.** Owner-scoped replacement plus manifest-pinned multi-table MVCC: a
  publication maps to an exact Delta version per table, and intermediate versions are
  invisible through `cpg_serving` until the pointer advances. Every query pins exactly one
  immutable snapshot and never mixes source generations.

`LIFE §§157–159` lists the mandatory consistency, performance and failure invariants — check
any incremental-update design against them.

### 0.3 Library references (`docs/library_ref/`)

Deep, **version-pinned** references for the exact dependency baseline the specs assume:
Arrow/Parquet, DataFusion, delta-rs, `object_store`, tree-sitter, Ruff, Pyrefly, petgraph,
`notify-debouncer-full`, gix, FastMCP, Pydantic, FastAPI, `rustc_public`.

**Consult the matching file before writing code against any of them** — several APIs are
pre-release or nightly-sensitive. Do not quote a version from here or from a skill: the
session context prints the pins extracted live from `FAB §2.1`, which is the only
authoritative source. `docs/spec_index/library-routing.md` maps a spec section to the
reference chapter that covers it.

---

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
├── Cargo.toml  Cargo.lock  rust-toolchain.toml  stable daemon/data-plane rlib
├── src/  tests/                               stable-root code and one test target
├── rustc-extractor/                           dated-nightly Cargo root
├── pyrefly-sidecar/                           pinned-source Cargo root
├── codefabric-cpg-mcp/                        Python adapter + local uv.lock
├── contracts/                                 AC-G-05 authority + shared fixtures
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
| `src/main.rs`, `src/bin/` | a CLI is needed — same package, reusing the lib target (§3) |
| `scripts/artifact_check.sh`, `scripts/plan_status.sh` + recipes | phase 2 of the process-policy redesign lands them (`.claude/skills/_shared/artifact-schemas.md` §8) |

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
agent → FastMCP adapter → private UDS gRPC → stable Rust daemon
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
model-compiler = ["dep:gix", "dep:petgraph", "dep:rustix", "..."]
data-fabric = ["dep:arrow", "...", "dep:datafusion", "dep:deltalake", "..."]
rpc = ["dep:prost", "dep:tokio", "dep:tonic", "dep:tonic-prost"]
repository-state = ["dep:gix", "dep:rusqlite", "dep:rustix", "dep:url"]
compatibility-probes = ["canonical-json", "data-fabric", "repository-state", "rpc"]
local-workstation = ["daemon", "compatibility-probes"]
s3-storage = ["data-fabric", "deltalake/s3"]
```

| Surface | Command | Dependency graph |
|---|---|---|
| local workstation | `cargo check --all-targets` | local provider authority; no `deltalake-aws` or AWS SDK |
| featureless substrate | `cargo check --all-targets --no-default-features` | dependency-free root substrate |
| canonical JSON | `cargo check --no-default-features --features canonical-json` | strict JSON/JCS only; no data fabric, repository, or RPC |
| contract models | `cargo check --no-default-features --features contract-models` | runtime wire models only; no compiler or generated-output closure |
| model compiler | `cargo check --no-default-features --features model-compiler --bin codefabric-model` | handwritten repository model, drivers, assurance, and reconciler |
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
| root toolchain | `stable` | stable daemon/data-plane boundary |
| components | `rustfmt`, `clippy`, `rust-analyzer`, `rust-src`, `llvm-tools-preview` | `llvm-tools-preview` is the substrate for coverage, binutils and fuzz-coverage (tooling-ref §8); `rust-src` gives semantic tools stdlib source (§7) |
| extractor toolchain | `nightly-2026-08-18` in its own root | owns `rustc-dev`; it never contaminates the stable root |
| `rust-version` | `1.95.0` | floor imposed by the Ruff 0.0.7 provider train (above delta-rs's 1.94.1 floor) and verified with `cargo msrv verify` |
| `license` | **not declared** | no license chosen yet; `publish = false` stops Cargo warning. `deny.toml` sets `licenses.private.ignore = true` so our own crate is not reported "Unlicensed" |
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

`.cargo/config.toml` commits `rustc-wrapper = "sccache"`. This is the deliberate side of
repo-spec §13.1's caveat: making the wrapper visible in version control beats an
undocumented shell environment, at the cost that **cargo fails outright without sccache
installed**. `just doctor` checks for it; `just cache-stats` reports the hit rate, because
§13.2 is explicit that you watch the rate rather than assume caching helps.

Nothing host-specific belongs in that file — no `-C target-cpu=native`, no absolute paths,
no one machine's linker.

`cargo clean` is not routine hygiene (§13.3). Reserve it for suspected stale artifacts,
controlled clean-build measurement, reclaiming disk, or isolating a profile/feature
interaction. A large `target/` is not evidence that a release artifact is large.

The cache and artifact topology is deliberate:

- sccache remains host-global; the repository does not set `SCCACHE_DIR`;
- stable root and stable Pyrefly-sidecar builds share the repository `target/`;
- the dated-nightly extractor uses `target/extractor/`;
- Miri/udeps use `target/nightly-assurance/`;
- cargo-fuzz uses `target/fuzz/<nightly-host>/` and explicitly selects the native host.

CI sets `CARGO_INCREMENTAL=0` so its sccache backend receives cacheable compiler outputs.
Local builds retain Cargo incremental compilation.

### One continuous checker

Ownership is divided (repo-spec §15.1, tooling-ref §72.22):

```
rust-analyzer  →  semantic model, references, types, assists
Bacon          →  the persistent cargo check job          (bacon.toml)
Watchexec      →  non-Rust tasks and process restarts only
```

Do not configure editor check-on-save, Bacon, and Watchexec to run the same
`cargo check`. `bacon.toml` defines stable-root `check`, `clippy`, and `nextest` jobs,
and exports `.bacon-locations` for editor/agent consumption — an agent
must confirm that file matches the current source generation before reading an empty list
as success (§15.2).

### Shell environment — the trap that matters for agents

`direnv` applies `.envrc` in **interactive shells only**. Agent harnesses typically run
each command in a fresh non-interactive shell that inherits nothing. Invoke tools without
assuming an activated environment:

```bash
direnv exec . <cmd>              # full .envrc environment, non-interactively
. scripts/bootstrap.sh && <cmd>  # within one compound command
```

`scripts/bootstrap.sh` is the shared mechanism: sourcing it applies the adapter's
domain-local virtual environment and repository paths; running it checks all four domains.
`--quiet` is silent when healthy; `--context` emits a compact block for agent context
injection. Python commands are always domain-explicit; there is no root uv environment.

---

## 7. Test architecture and what each layer proves

Evidence is **orthogonal**, not redundant (repo-spec §25). WP01 starts with executable
compatibility tests; each later packet adds behavioral proof at the boundary it owns.

| Question | Instrument | Recipe |
|---|---|---|
| Do ordinary stable-root Rust tests pass? | cargo-nextest | `just root-test-rust` |
| Do documented examples still work? | `cargo test --doc` | `just root-doctest` |
| Does the exact stable graph match the design? | resolved metadata/tree validator | `just stable-graph-check` |
| Do structural boundaries hold? | tested ast-grep rules | `just governance-scan` |
| Is the complete DesiredTree reproducible? | dual isolated model generation | `just model-repro-check` |
| Do all four domains pass their routine gates? | aggregate command | `just ci-fast` |
| Which Rust regions executed? | cargo-llvm-cov | `just coverage` |
| Do assertions detect plausible faults? | cargo-mutants | `just mutants-file <path>` |
| What inputs find new behavior? | cargo-fuzz | `just fuzz jcs_decode_canonicalize` |
| Did structured output change? | cargo-insta | `just snapshots-review` |
| Do unsafe/concurrent executions violate Rust's rules? | Miri | `just miri`, `just miri-seeds` |
| Does the adapter protocol behave? | adapter-local pytest/FastMCP client | added in WP04 |

### Three traps the tooling will not catch for you

1. **`cargo nextest` does not run doctests.** Never report "all Rust tests passed" from
   nextest alone. `just root-test` covers both; `just root-test-rust` does not (§18.2, §62.2).
2. **`--all-features` is not a feature matrix.** It validates only the maximal additive
   union and hides accidental coupling. Use `just features-each` (§26.1, §62.6).
3. **Coverage is not test strength.** A covered line may assert nothing. Triangulate:
   uncovered + surviving mutant → establish reachability first; covered + surviving mutant
   → strengthen the assertion (§21, §22.1, §62.4).

### nextest profiles

`.config/nextest.toml` defines `default` (fail-fast, 60s slow timeout) and `ci`
(no fail-fast, exponential retries, `flaky-result = "fail"`, 2h global timeout, JUnit
output). **Retries are diagnostic**: a test that only passes on rerun still fails the
build (§18.1). When tests come to share a database, port, or other scarce resource,
constrain them with a nextest test group rather than serializing the whole suite (§18.3) —
the file carries a commented example.

### Adapter test posture

From WP04 onward, Python tests exercise the FastMCP adapter through its public protocol
surface and real gRPC stubs; they do not replay Rust domain logic or import a native
extension. Rust owns domain validation, Arrow processing, and query execution.

---

## 8. Assurance tiers — what runs where

Not every installed tool belongs on the critical path of every commit (repo-spec §49).

**Tier A — every meaningful change.** Wired into `just ci-fast` and CI:

```
root fmt/check/clippy/nextest/doctests · adapter lint/type/test/STDIO
· extractor + sidecar domain gates at milestones · typos · machete · shear
· stable graph · tested ast-grep rules · duplicate-family negative fixture · proto drift
          [+ advisory registry · deny advisories/bans/sources · audit in CI/ci-pr]
```

`just ci-pr` adds root and sidecar policy, two-root Protobuf reproduction, the `ci`
nextest profile, and pending-snapshot review state.

**Tier B — conditional.** Available as recipes, not wired into CI: `coverage`,
`features-each`, `features-no-default`, `semver`, `msrv`.

**Tier C — risk-triggered or scheduled.** Available as recipes: `miri`, `miri-seeds`,
`mutants-file`, `fuzz`, `udeps`, `unsafe-surface`, plus the whole `perf` group.

**CI implements Tier A plus the path/pin-triggered extractor and sidecar gates.** Those
two run on relevant changes, pushes, scheduled runs, and manual milestone dispatch; root,
adapter, contracts, and governance run on every PR. Every CI step mirrors a justfile
recipe. Actions are pinned to commit SHAs and CLIs to explicit versions (§50.1).

### Choose evidence by change risk

Classify before validating (repo-spec §60). The rows most likely to apply here:

| Change | Minimum additional evidence |
|---|---|
| comment/docs only | formatting / Typos as relevant |
| local safe Rust logic | check + Clippy + targeted nextest |
| FastMCP adapter | adapter pytest + Pyrefly + Ruff + protocol tests |
| gRPC boundary | Rust and Python interop + size/identity/error tests |
| Cargo feature | `features-each` + default and featureless builds |
| dependency | `deps-fast` + `policy` + tests + feature matrix |
| unsafe/pointer/concurrency | Geiger + Miri + native tests |
| parser/protocol | coverage + fuzz + snapshots + mutation testing |
| performance claim | Hyperfine before/after + profiler |
| Python packaging | locked adapter sync + subprocess/import proof |

**Do not run Tier C tools to appear thorough.** Run them when they produce evidence
relevant to the risk (§73.1).

---

## 9. Dependency and supply-chain policy

Three unused-dependency tools sit at different fidelity/cost points (repo-spec §29):

```
cargo-machete → fast heuristic hint          just deps-fast
cargo-shear   → primary static hygiene gate  just deps-fast
cargo-udeps   → nightly compiler adjudication  just udeps   (disputed findings only)
```

**A scanner result is not permission to mutate dependencies** (§62.7). Before removing
one, reconcile feature-gated use, build scripts, macro expansion, examples/benches/tests,
generated code, renamed dependencies, and platform `cfg`. `cargo expand` is the natural
next tool when macro-generated use is suspected.

`deny.toml` was generated with `cargo deny init` and then edited deliberately. Its
non-obvious decisions:

- **`all-features = true`.** Policy checks include the optional S3 deployment graph;
  default-profile isolation is proven separately by `stable-graph-check`.
- **License checks are currently inactive by user direction.** The dormant allow-list is
  retained only as a future decision aid; neither `just policy` nor CI evaluates it, and
  it is not assurance evidence.
- **`licenses.private.ignore = true`** because this package declares no `license` field
  (see *Manifests, pins, and the decisions inside them*). Revisit when a license is chosen.
- **`multiple-versions = "deny"` with exact transitive skips.** Type-bearing
  Arrow/Parquet/DataFusion/object_store/buoyant-kernel families are never skipped. A
  committed second-Arrow graph must fail on every governance run; the script also checks
  the deny-config shape before accepting that expected failure.
- **crates.io plus one approved Git source**, `unknown-git = "deny"`. The only exception
  is delta-rs; the resolved-graph validator enforces its immutable revision.
- **Exact advisory exceptions only.** `tooling/security/advisory-exceptions.json` records
  package/version, rationale, owner, and the mandatory WP19 review trigger. The checker
  requires exact equality with `deny.toml`, `Cargo.lock`, and the current RustSec result.

The policy audit is green only after applying the exact registered exceptions; it means
there are no *unregistered* current findings, **not** that dependencies have been
security-audited or that registered findings are safe (§31).

**cargo-vet is deliberately not adopted** (§32). Maintaining human audit attestations is
real work and `supply-chain/` should not exist merely to have one. An agent may identify
audit gaps, summarize dependency diffs and prepare a review checklist — it must never run
`cargo vet certify`, which is an accountable *human* attestation (§32.1).

`cargo geiger` (`just unsafe-surface`) is an inventory, not a vulnerability score (§33).
Use it to decide where Miri, fuzzing and manual review should focus.

### Spelling is an API concern

`_typos.toml` deliberately does **not** exclude source, snapshots, error messages or
stubs: a misspelling in a serialized field, an exception name, or a public Python symbol
becomes an API defect (§17). It excludes only generated output and `docs/library_ref/`
(~9 MB of third-party prose that would bury real findings). The word list carries one
entry, `writeable`, with its reason recorded — adding every finding to the dictionary to
get a green run is an anti-pattern (tooling-ref §72.5). `typos -w` can rewrite identifiers,
so `just typos-write` is confirm-gated.

---

## 10. Tooling capability inventory

All 34 tools from the reference are installed and resolvable; `just doctor` verifies the
subset the repository contract actually requires, and `just inventory` captures a
non-secret record to `target/tooling-inventory.txt` (repo-spec §57).

| Capability | Tool | Status here | Entry point |
|---|---|---|---|
| Command contract | `just` | **required** | `just --list` |
| Compilation cache | `sccache` | **required** — committed wrapper | `just cache-stats` |
| Environment report | `scripts/bootstrap.sh` | **required** | `just doctor` |
| Test runner | cargo-nextest | Tier A | `just root-test-rust` |
| Stable graph validation | Cargo metadata/tree + jq | Tier A | `just stable-graph-check` |
| Structural boundaries | ast-grep | Tier A | `just governance-scan` |
| Adapter lint/types/tests | Ruff, Pyrefly, pytest | Tier A | `just adapter-ci-fast` |
| Protobuf generation | prost/tonic + grpcio-tools | Tier A | `just proto-check` |
| Duplicate-family policy | cargo-deny + negative fixture | Tier A | `just duplicate-family-check` |
| Spelling | Typos | Tier A | `just typos` |
| Unused deps (fast) | cargo-machete, cargo-shear | Tier A | `just deps-fast` |
| Dependency policy | cargo-deny, cargo-audit | Tier A (CI / `ci-pr`) | `just policy` |
| Continuous check | Bacon | local loop | `bacon` |
| Process/file triggers | Watchexec | targeted local | — |
| File discovery | `fd` | agent discovery | — |
| Coverage | cargo-llvm-cov | Tier B | `just coverage` |
| Feature matrix | cargo-hack | Tier B | `just features-each` |
| Rust API compatibility | cargo-semver-checks | Tier B — only if the Rust API becomes externally supported (§28) | `just semver <rev>` |
| MSRV | cargo-msrv | WP01 packet gate / Tier B later | `just msrv` |
| Snapshots | cargo-insta | when snapshot-worthy output exists (§20) | `just snapshots-review` |
| UB / aliasing / races | Miri | Tier C, risk-triggered | `just miri`, `just miri-seeds` |
| Assertion strength | cargo-mutants | Tier C | `just mutants-file <path>` |
| Adversarial inputs | cargo-fuzz | Tier C — JCS target present | `just fuzz jcs_decode_canonicalize` |
| Unused deps (compiler) | cargo-udeps | Tier C, disputed findings | `just udeps` |
| Unsafe inventory | cargo-geiger | Tier C | `just unsafe-surface` |
| Audit attestations | cargo-vet | **not adopted** (§32) | — |
| Binary provenance | cargo-auditable | only for standalone executables (§34) | — |
| Macro expansion | cargo-expand | targeted investigation | `cargo expand` |
| MIR / LLVM / asm | cargo-show-asm | targeted | `just asm` |
| Symbols / sections | cargo-binutils | targeted | `just symbols`, `just sections` |
| Code-size attribution | cargo-bloat | targeted | `just bloat` |
| Benchmarking | Hyperfine | performance claims | — |
| Profiling | Samply, cargo-flamegraph | performance investigation | `just profile-build` |
| Cross-target | Cross, cargo-zigbuild | only supported foreign targets | — |
| Tool lifecycle | cargo-binstall, cargo-update | workstation maintenance | `just tool-updates-check` |

### Performance evidence has a required order

Profiling locates a hotspot; only a controlled benchmark verifies an improvement
(repo-spec §40.4):

```
controlled baseline → profiler finds hotspot → expand/asm/binutils/bloat explains
mechanism → ONE controlled change → correctness suite → the exact benchmark repeated
```

A narrower flame frame or nicer assembly is not itself an improvement. And `target/` disk
usage is not artifact size — measure the final executable or package (§39, §82.2).

---

### 10.1 Navigating the documentation corpus

The corpora are too large to read. `.claude/skills/_shared/code-intelligence.md` is the full
reference — a three-tier instrument ladder for *proof*, and `outline` first for *navigation*.

| Tier | Instrument | Proves | Reach |
|---|---|---|---|
| 1 | `cargo check`, `uv run ruff` | caller coverage **by construction** — rename the symbol, rebuild clean | Rust strong, Python partial |
| 2 | `ast-grep` | structure: call sites, impls, signatures, member access | any tree-sitter language |
| 3 | `rg` | literal residue: strings, config, comments, cross-language, generated | everything, including what has no AST |

Escalate down for breadth, up for proof. A tier-3 zero-hit is not caller completeness; a
tier-1 clean rebuild is. Label every claim with the highest tier that confirmed it, and with
the tool version — the session context prints both.

Three navigators, none interchangeable:

- `just spec-outline` — `docs/upfront_design/` by section. h2-rooted (`## N.` items,
  `### N.N` members).
- `just lib-outline` — `docs/library_ref/` by chapter. h1-rooted, because those files are
  rooted a level higher; `spec-outline`'s mapping would find no chapters and silently
  flatten the rest. Each script refuses the other's tree rather than emit a misleading
  outline. Extractors live in `tooling/ast-grep/outline/`, with shape tests beside them.
- `ast-grep outline <dir>` — code. `--items exports` maps a subtree's public surface;
  `--view expanded --match '^Name$'` inspects one type's members. Syntax only: no reference
  resolution, no types, no call graph.

`--match` filters items, not members — zoom to the chapter, then `--view expanded`. Prefer
either outline over `rg` for headings in those files: both parse markdown, so a `# Cargo.toml`
inside a fenced block cannot masquerade as a heading.

The search traps that silently shrink results are printed in the session context rather than
restated here.

### 10.2 Project skills (`.claude/skills/`)

Twenty-one skills plus `_shared/`, discoverable by both agents: `.codex/skills` and
`.agents/skills` symlink to `.claude/skills`, so a skill is edited once and both read it.

- **Ten workflow skills**, intended as a flow: `design-development` →
  `library-capability-research` → `impl-plan` → `plan-audit` → `integrate-plan-audit` →
  `impl-plan-exec` → `impl-status` → `implementation-review`, with `lib-leverage` and
  `skill-eval` standalone. Each opens by reading shared policy from `_shared/`.
- **Eleven library-reference navigators** routing `docs/library_ref/`: `ast-grep-ripgrep-ref`,
  `canonicalization-lib-ref`, `code-facts-lib-ref`, `datafusion-pyarrow-rust-ref`,
  `deltalake-rust-ref`, `fastmcp-pydantic-ref`, `grpcio-orjson-protobuf-ref`, `gix-notify-ref`,
  `petgraph-ref`, and the two inert ones — `attrs-cattrs-ref` and `typer-rich-ref`, whose target
  documents have not been written. Both self-declare. `ast-grep-ripgrep-ref` is the one that is
  also project-anchored: it maps search use cases onto ast-grep, ripgrep and PCRE2 capabilities,
  and covers this repository's own `rules/` and outline extractors.

`_shared/` holds the policy every workflow skill loads: `code-intelligence.md` (research),
`evidence-policy.md` (the governing principle — executable beats derived beats recorded —
plus claim → required evidence), `validation-policy.md` (gates), `doctrine-policy.md`,
`subagent-orchestration.md`, `artifact-schemas.md` (artifact paths, frontmatter, the ID
minting rule, status vocabularies, and the §8 validation/derivation contract).

---

## 11. Invariants for agents working here

Items 1, 2, 5 and 6 are also asserted in the session context that `scripts/bootstrap.sh
--context` injects at session start, so an agent should already hold them. They are kept
here in full because the context block carries the rule and this section carries the reason.


1. **Read `just --list` first.** Prefer a recipe over reconstructing tool flags
   (repo-spec §59, §92).
2. **Run `just ci-fast` before editing** and record pre-existing failures separately from
   anything your change causes (§59.1).
3. **Classify the change, then choose evidence** from the §8 table. Do not escalate to
   Tier C for appearance.
4. **rust-analyzer is not the final authority** — confirm with the repository's Cargo
   commands (§62.1).
5. **Never report "all Rust tests passed" from nextest alone.** Doctests are separate.
6. **Preserve domain isolation.** No compiler-private or Pyrefly dependency enters the
   stable root, and Python never becomes an Arrow/DataFusion processing layer.
7. **Keep Python as presentation only.** If adapter code begins implementing domain or
   query behavior, move that behavior behind the daemon contract.
8. **Do not create a crate to organize code, or a second top-level test file** (§61.2,
   §61.3).
9. **Mutating recipes need a reason and a diff review.** `root-fmt-write`, `proto-gen`,
    `typos-write`, `snapshots-accept`, `deps-fix`, and `cargo update` — none is ever an
    automatic fix (§14.1, §63).
10. **Miri never proves soundness.** Record toolchain, seed range, target and exclusions
    with any finding (§24.2, §62.5).
11. **Record the evidence, not the verdict** (§65): tool and version, toolchain and
    target, feature selection, profile, exact command, exit status, and whether the
    command mutated source, lockfiles, manifests or the environment.
