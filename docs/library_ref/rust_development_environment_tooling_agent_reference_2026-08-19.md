# Rust Development Environment Tooling — Advanced Reference for LLM Programming Agents

**Tooling covered:** `rust-analyzer`, `rust-src`, `llvm-tools-preview`, `rustc-dev`, Miri, `cargo-binstall`, `cargo-update`, `sccache`, `just`, Bacon, Watchexec, `fd`, Typos, cargo-nextest, cargo-llvm-cov, cargo-insta, cargo-mutants, cargo-fuzz, cargo-hack, cargo-msrv, cargo-semver-checks, cargo-shear, cargo-machete, cargo-udeps, cargo-deny, cargo-audit, cargo-vet, cargo-geiger, cargo-auditable, cargo-expand, cargo-show-asm, cargo-binutils, cargo-bloat, Hyperfine, Samply, cargo-flamegraph, Cross, cargo-zigbuild, and Maturin  
**Reference date:** 2026-08-19  
**Primary audience:** LLM programming agents and engineers operating a high-assurance, high-performance Rust development environment  
**Operating systems:** Linux and macOS are the primary deployment targets; most Rust-side guidance is also applicable to Windows  
**Document structure:** source and version policy, layered mental model, tool-by-tool deep dives, integrated workflows, repository configuration patterns, execution-cost tiers, anti-patterns, and a final agent checklist

---

## Executive purpose

Rustup's normal development profile gives a strong language baseline—compiler, Cargo, standard library, formatter, linter, and documentation—but it does not by itself provide a complete engineering control system. A best-in-class environment must answer several different questions:

1. **What does the code mean while it is being edited?**
2. **Does it compile and behave correctly under ordinary execution?**
3. **Does the test suite actually constrain behavior, or merely execute lines?**
4. **Is unsafe code free of undefined behavior under the executions we explored?**
5. **Do all supported feature combinations, Rust versions, and targets remain valid?**
6. **Are dependencies necessary, policy-compliant, audited, and recoverable from a production binary?**
7. **Where do time, allocations, code size, symbols, and machine instructions actually come from?**
8. **Can the same repository be built, tested, packaged, and inspected reproducibly by humans, CI, and programming agents?**

No single tool answers all of those questions. This reference treats the installed additions as a **composed development-control plane**:

```text
developer or programming-agent intent
                │
                ▼
       repository command contract
                │
              `just`
                │
       ┌────────┴─────────┐
       │                  │
       ▼                  ▼
continuous feedback   explicit one-shot work
Bacon / Watchexec     CI / release / investigation
       │                  │
       └────────┬─────────┘
                ▼
semantic and build substrate
rust-analyzer + rust-src + sccache + rustc/Cargo
                │
    ┌───────────┼──────────────────────────┐
    │           │                          │
    ▼           ▼                          ▼
correctness   compatibility             dependency assurance
nextest       cargo-hack                cargo-shear/machete/udeps
llvm-cov      cargo-msrv                cargo-deny/audit/vet
Insta         semver-checks             geiger/auditable
mutants
fuzz
Miri
    │           │                          │
    └───────────┴─────────────┬────────────┘
                              ▼
                  compiler/artifact evidence
       expand → MIR/LLVM/asm → symbols/sections → size
                              │
                    show-asm / binutils / bloat
                              │
                              ▼
                      performance evidence
                  Hyperfine + Samply + flamegraph
                              │
                              ▼
                    target and package delivery
                   Cross + cargo-zigbuild + Maturin
```

The central design principle is:

> **Use each tool for the kind of evidence it can actually produce, and combine independent evidence rather than allowing one green command to stand in for correctness.**

Coverage is not mutation testing. A vulnerability database scan is not a dependency audit. Cross-compilation is not target execution. A flame graph is not a before/after benchmark. Macro expansion is not the original source program. An unsafe-code count is not a security verdict. The reference repeatedly makes these boundaries explicit.

---


## Documentation map

| Part | Sections | Purpose |
|---|---:|---|
| Foundations | 0–5 | scope, version policy, trust/mutability model, inventory, topology, and tool-selection rules |
| Part I | 6–10 | rust-analyzer, rust-src, LLVM tools, rustc-dev, and Miri |
| Part II | 11–18 | installation/update lifecycle, caching, command orchestration, watchers, discovery, and spelling hygiene |
| Part III | 19–23 | nextest, source coverage, snapshots, mutation testing, and fuzzing |
| Part IV | 24–30 | feature matrices, MSRV, semver, and three-layer unused-dependency analysis |
| Part V | 31–36 | dependency policy, RustSec scanning, audit attestations, unsafe inventory, and binary provenance |
| Part VI | 37–44 | macro expansion, MIR/LLVM/assembly, symbols, code size, benchmarking, and profiling |
| Part VII | 45–48 | containerized cross-target work, Zig-based linking, and Rust–Python packaging |
| Part VIII | 49–61 | end-to-end LLM-agent workflows by change/risk class |
| Part IX | 62–71 | reusable repository configuration templates |
| Part X | 72–75 | anti-patterns, failure taxonomy, final invariants, and implementation checklist |
| Source index | — | official documentation and primary project repositories |

The document is intentionally searchable by exact executable or component name. Each tool chapter uses the same operating pattern:

```text
identity
→ compelling capabilities
→ canonical commands/configuration
→ integrations
→ limitations and failure modes
→ LLM-agent decision rule
```


# 0) Scope, exclusions, and baseline assumptions

## 0.1 Included additions

This document covers every non-native addition from the comprehensive installation set, except for the explicitly excluded source-navigation tools:

### Rustup/toolchain components

- `rust-analyzer`
- `rust-src`
- `llvm-tools-preview`
- `rustc-dev`
- `miri`

### Cargo-installed or standalone Rust tools

- `cargo-binstall`
- `cargo-update`
- `sccache`
- `just`
- `bacon`
- `watchexec`
- `fd`
- `typos`
- `cargo-nextest`
- `cargo-llvm-cov`
- `cargo-insta`
- `cargo-mutants`
- `cargo-fuzz`
- `cargo-hack`
- `cargo-msrv`
- `cargo-semver-checks`
- `cargo-shear`
- `cargo-machete`
- `cargo-udeps`
- `cargo-deny`
- `cargo-audit`
- `cargo-vet`
- `cargo-geiger`
- `cargo-auditable`
- `cargo-expand`
- `cargo-show-asm`
- `cargo-binutils`
- `cargo-bloat`
- `hyperfine`
- `samply`
- `flamegraph` / `cargo flamegraph`
- `cross`
- `cargo-zigbuild`
- `maturin`

## 0.2 Explicit exclusions

The following are outside the document's scope:

- Ripgrep
- ast-grep
- Tree-sitter and Tree-sitter CLI
- Native compiler, linker, debugger, profiler, and build dependencies such as CMake, Ninja, Clang, GCC, LLDB, GDB, `perf`, Valgrind, Heaptrack, mold, LLD, OpenSSL development headers, `pkg-config`, and related operating-system packages

Some included tools rely on native facilities at runtime—for example, cargo-flamegraph requires an operating-system sampling backend, Cross requires a container runtime for its normal workflow, and cargo-zigbuild requires Zig. This reference identifies those external prerequisites but does not document or install them.

## 0.3 Standard Rust tools are assumed, not re-documented

The following baseline commands remain part of every workflow but are not standalone subjects here:

```bash
cargo check
cargo build
cargo test
cargo fmt
cargo clippy
cargo doc
cargo metadata
cargo tree
cargo vendor
```

An agent must not replace these fundamentals merely because more specialized tools are installed. Specialized tools refine or extend the evidence produced by the baseline.

## 0.4 Tools mentioned previously but not part of the installed set

This document intentionally does **not** add sections for tools that were discussed as possible future additions but were not included in the installation command, such as `cargo-dist`, `release-plz`, and `wasm-pack`. Their absence prevents an agent from falsely assuming that the local environment has those capabilities.

---

# 1) Versioning, reproducibility, and source-of-truth policy

## 1.1 Why a single static version table is the wrong operational anchor

These tools do not share one release train:

- rustup components are coupled to a particular Rust toolchain;
- stable and nightly can carry different component builds;
- Cargo-installed CLIs release independently;
- some tools support a broad range of Rust versions while others require a recent compiler;
- nightly-only compiler internals can change without stable compatibility guarantees;
- prebuilt binaries may differ by target and packaging channel.

Accordingly, this reference is **capability-anchored to released documentation available on 2026-08-19**, while the local machine's installed versions are the source of truth for execution. Before depending on a flag or output schema, an agent should capture the local version and consult that version's help.

## 1.2 Mandatory environment inventory

At the beginning of a substantial task—or once per reusable session—capture:

```bash
rustc -vV
cargo -V
rustup show active-toolchain
rustup toolchain list
rustup component list --installed
cargo install --list
```

For a focused tool, also capture:

```bash
<tool> --version
<tool> --help
```

For Cargo subcommands, either form may expose version information depending on the program:

```bash
cargo nextest --version
cargo llvm-cov --version
cargo deny --version
cargo semver-checks --version
```

Do not infer installed versions from this document, a lockfile, or a prior session.

## 1.3 Pinning policy

Use three different pinning policies:

| Domain | Recommended policy |
|---|---|
| Stable compiler for ordinary applications | Pin only when the repository requires deterministic compiler behavior; otherwise declare `rust-version` and test upgrades |
| Nightly compiler for Miri or `rustc-dev` | Pin an exact nightly date in `rust-toolchain.toml` or CI |
| Third-party Cargo CLIs in CI | Install an explicit version; do not silently consume “latest” on every run |
| Developer-global CLI environment | Periodic managed upgrades are acceptable, followed by a validation run |
| Public library compatibility | Declare and verify MSRV; do not confuse the developer's current compiler with the supported floor |

A compiler-internals repository should normally use a date-pinned nightly:

```toml
# rust-toolchain.toml
[toolchain]
channel = "nightly-YYYY-MM-DD"
profile = "minimal"
components = [
  "rustfmt",
  "clippy",
  "rust-analyzer",
  "rust-src",
  "rustc-dev",
  "llvm-tools-preview",
  "miri",
]
```

A normal stable application should generally avoid making all developers use nightly merely because one scheduled Miri job needs it. Prefer targeted commands:

```bash
cargo +nightly miri test
cargo +nightly udeps
```

## 1.4 Released documentation precedence

When sources disagree, use this order:

1. `--help` and behavior of the installed executable.
2. Documentation for the exact installed release.
3. Tagged release notes or source for that release.
4. Current official documentation.
5. Repository `main`, issue discussions, and third-party tutorials only as supplementary evidence.

This is especially important for:

- nextest configuration fields introduced in recent releases;
- nightly Miri flags;
- cargo-semver-checks lint behavior;
- rust-analyzer configuration keys;
- cargo-binutils wrappers around unstable LLVM tooling;
- compiler-private APIs exposed through `rustc-dev`.

## 1.5 Global installation versus project reproducibility

`cargo-binstall` and `cargo install` normally place executables in a user-global bin directory. That is convenient but does not make a repository self-describing.

A repository should therefore declare its expected tool interface through at least one of:

- a `justfile`;
- CI installation steps with explicit versions;
- a `tools.toml`, developer setup script, or equivalent manifest;
- generated environment inventory attached to CI artifacts;
- documentation identifying optional versus required checks.

The agent should not assume that “installed on this machine” means “available to every contributor or CI worker.”

---

# 2) Agent execution model: cost, mutability, and trust

## 2.1 Four execution-cost tiers

Not every installed tool belongs in the same loop.

| Tier | Typical cadence | Examples | Expected cost |
|---|---|---|---|
| **Tier 0 — discovery** | Every task | version inventory, `just --list`, `fd`, targeted `cargo expand` | seconds |
| **Tier 1 — local feedback** | Every edit or small batch | rust-analyzer, Bacon, `cargo check`, targeted nextest, Typos, Machete/Shear | seconds to a few minutes |
| **Tier 2 — pull-request assurance** | Before review/merge | full nextest, doctests, deny/audit, coverage, selected feature matrix, snapshots | minutes |
| **Tier 3 — scheduled/release/investigation** | Nightly, release, or risk-triggered | Miri seed sweeps, fuzzing, mutation testing, full feature powerset, MSRV discovery, Cross matrix, cargo-vet backlog, profiling | minutes to hours |

An agent must choose a tier based on risk and change scope. Running every Tier 3 tool after every edit wastes compute and can make agents less reliable by encouraging timeouts, partial runs, and ignored failures.

## 2.2 Mutability classification

Commands that look like “analysis tools” may still modify state.

### Usually read-only with respect to source

Examples include:

```bash
cargo nextest run
cargo llvm-cov
cargo deny check
cargo audit
cargo shear
cargo machete
cargo +nightly udeps
cargo semver-checks
cargo geiger
cargo bloat
cargo asm
hyperfine ...
samply record ...
cargo flamegraph ...
```

They still create or modify build artifacts under `target`, caches, reports, and local metadata.

### Explicitly source- or repository-mutating

Examples include:

```bash
cargo shear --fix
typos -w
cargo insta accept
cargo insta reject
cargo vet init
cargo vet certify
cargo update
```

### Environment-mutating

Examples include:

```bash
cargo binstall ...
cargo install-update -a
maturin develop
rustup component add ...
```

An LLM agent must inspect the diff after source mutation and must not auto-accept snapshots, auto-certify audits, or globally update tools merely to make a check green.

## 2.3 The hidden execution boundary: build scripts and procedural macros

Many Cargo commands can execute repository-controlled code:

- `build.rs` build scripts;
- procedural macros;
- test binaries;
- benchmark binaries;
- custom setup scripts;
- binaries invoked by `just`, Watchexec, or nextest.

Therefore:

> **Do not run Cargo tooling on an untrusted repository outside a sandbox merely because the command's name contains “check,” “audit,” or “metadata.”**

The tool itself may be trustworthy while the project evaluation path executes untrusted code.

## 2.4 Evidence record required from autonomous agents

For every nontrivial check, preserve:

```text
tool and version
rustc/cargo version
active toolchain and target
workspace/package scope
feature selection
profile
exact command
exit status
relevant output or report path
known exclusions
whether the command modified source, lockfiles, snapshots, manifests, or environment
```

A bare statement such as “tests pass” is insufficient. It should become:

```text
cargo-nextest <version>
rustc <version>, host x86_64-unknown-linux-gnu
command: cargo nextest run --workspace --all-features -P ci
result: 1,842 passed; 0 failed; 7 skipped
separate doctest command: cargo test --workspace --doc --all-features
artifacts: target/nextest/ci/junit.xml
```

## 2.5 Stable command surface for agents

Agents should prefer repository-owned commands over constructing ad hoc invocations:

```bash
just ci-fast
just ci-full
just coverage
just miri
just fuzz-parser
```

This gives maintainers control over features, packages, timeouts, environment variables, target selection, and report locations. `just` is the recommended command contract; the underlying tools remain directly available for focused investigation.

---

# 3) Capability inventory and default cadence

| Tool/component | Primary evidence or capability | Default cadence |
|---|---|---|
| rust-analyzer | edit-time semantic model, navigation, diagnostics, assists | always on |
| rust-src | standard-library source and compiler-tool support | installed per toolchain |
| llvm-tools-preview | LLVM coverage, symbol, section, and disassembly support | prerequisite |
| rustc-dev | compiler-private APIs and compiler-internals tooling | targeted/nightly |
| Miri | interpreted UB and concurrency-model checks | scheduled/risk-triggered |
| cargo-binstall | prebuilt Cargo CLI acquisition | setup/update |
| cargo-update | update globally installed Cargo executables | maintenance |
| sccache | reusable compilation artifacts | always on where effective |
| just | repository command contract | every agent task |
| Bacon | persistent Rust-aware background feedback | local development |
| Watchexec | generic filesystem-triggered command/restart orchestration | targeted/local |
| fd | fast, ignore-aware file enumeration | every agent task |
| Typos | low-cost source spelling/identifier hygiene | local/PR |
| cargo-nextest | reliable, parallel test execution and CI reporting | local/PR |
| cargo-llvm-cov | source-based line/region/branch coverage | PR/scheduled |
| cargo-insta | reviewed snapshot/approval tests | change-specific |
| cargo-mutants | behavioral strength of the test suite | scheduled/risk-triggered |
| cargo-fuzz | coverage-guided adversarial input generation | continuous/scheduled for input surfaces |
| cargo-hack | feature and compiler-version matrix execution | PR/scheduled |
| cargo-msrv | discover and verify minimum supported Rust | release/scheduled |
| cargo-semver-checks | public API compatibility against a baseline | PR/release for libraries |
| cargo-shear | unused/misplaced dependency and unlinked-file analysis | local/PR |
| cargo-machete | very fast heuristic dependency scan | local |
| cargo-udeps | compiler-oriented unused dependency validation | scheduled/adjudication |
| cargo-deny | dependency policy: licenses, sources, bans, advisories | PR |
| cargo-audit | RustSec vulnerability scan of resolved dependencies/binaries | PR/scheduled |
| cargo-vet | trusted human audit attestations and audit-gap management | dependency changes |
| cargo-geiger | unsafe-code surface inventory | scheduled/review |
| cargo-auditable | dependency metadata embedded in built binaries | release |
| cargo-expand | macro and derive expansion inspection | targeted |
| cargo-show-asm | MIR, LLVM IR, assembly, disassembly | targeted/performance |
| cargo-binutils | symbols, sections, sizes, disassembly via LLVM tools | targeted/release |
| cargo-bloat | code-size attribution by crate/function | targeted/release |
| Hyperfine | statistically structured command benchmarking | performance changes |
| Samply | interactive sampling profiles and timelines | performance investigation |
| cargo-flamegraph | static flamegraph generation | performance investigation/CI artifact |
| Cross | containerized cross-build and cross-test environments | target matrix |
| cargo-zigbuild | Zig-based linker/sysroot portability and glibc targeting | release/cross-build |
| Maturin | Rust-backed Python package development and wheel production | Python interop |

---

# 4) Recommended repository topology

A large Rust workspace benefits from separating configuration by concern:

```text
repository/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml             # only when a repository-level pin is warranted
├── justfile                        # stable human/agent command surface
├── bacon.toml                      # background jobs
├── deny.toml                       # dependency policy
├── _typos.toml                     # spelling policy
├── .config/
│   └── nextest.toml                # test profiles, retries, groups, timeouts, JUnit
├── .cargo/
│   └── config.toml                 # sccache and target-safe Cargo configuration
├── supply-chain/
│   ├── config.toml                 # cargo-vet policy
│   ├── audits.toml                 # review attestations
│   └── exemptions.toml             # explicit deferred coverage
├── fuzz/
│   ├── Cargo.toml
│   ├── fuzz_targets/
│   ├── corpus/
│   └── artifacts/
├── snapshots/ or tests/snapshots/  # Insta snapshots, depending on module layout
├── scripts/                        # commands too complex for inline just recipes
└── target/                         # non-source build/report output
```

The exact layout is flexible. The invariant is that policy and command semantics live in version control rather than in agent memory.

---

# 5) Tool selection rules at a glance

## 5.1 “Which test tool should I run?”

```text
Need fast ordinary tests?                  cargo-nextest
Need doctests?                            cargo test --doc
Need to know which code ran?              cargo-llvm-cov
Need to know whether assertions matter?   cargo-mutants
Need adversarial inputs?                  cargo-fuzz
Need large structured expected output?    cargo-insta
Need UB/aliasing/data-race interpretation? Miri
```

## 5.2 “Which dependency tool should I run?”

```text
Fast heuristic unused-dependency hint?    cargo-machete
Primary unused/misplaced dependency scan? cargo-shear
Nightly compiler-oriented adjudication?   cargo-udeps
License/source/ban/advisory policy?        cargo-deny
Known RustSec vulnerability scan?         cargo-audit
Human review attestations/trust graph?     cargo-vet
Unsafe usage inventory?                   cargo-geiger
Recover dependency tree from binary?      cargo-auditable
```

## 5.3 “Which performance or code-generation tool should I run?”

```text
Need reproducible before/after timing?     Hyperfine
Need interactive hotspot/timeline view?    Samply
Need a portable static flame artifact?     cargo-flamegraph
Need macro-generated Rust?                 cargo-expand
Need MIR, LLVM IR, or assembly?            cargo-show-asm
Need symbols, sections, disassembly?        cargo-binutils
Need code-size attribution?                cargo-bloat
```

## 5.4 “Which target-delivery tool should I run?”

```text
Need containerized cross build/test?        Cross
Need Zig linker or explicit glibc floor?    cargo-zigbuild
Need Python extension/wheel packaging?      Maturin
```

These are complementary choices, not interchangeable brands.

---

# Part I — Toolchain and semantic/compiler foundations

# 6) rust-analyzer

## 6.0 Identity

rust-analyzer is the Rust language server used to construct an edit-time semantic model of a workspace. It is not merely syntax highlighting. It resolves names and types, expands macros, evaluates Cargo metadata, invokes build scripts when configured, surfaces diagnostics, supplies completion and navigation, and performs source transformations through assists. It is the primary mechanism by which an LLM-capable editor can obtain compiler-like context before a full build.

Official reference: [rust-analyzer manual][RA].

## 6.1 Why it is compelling for programming agents

An agent working only from text search must repeatedly rediscover:

- which trait method is selected;
- which generic instantiation is involved;
- where a symbol is declared or implemented;
- what a macro expands to;
- which features and target configuration are active;
- whether a proposed edit introduces type or borrow errors.

rust-analyzer turns many of those questions into structured editor operations. Its highest-value capabilities are:

- go to definition, declaration, implementation, and type definition;
- find references and call hierarchy;
- hover types, inferred lifetimes, documentation, and layout information;
- semantic rename rather than textual replacement;
- workspace symbols and document symbols;
- completion informed by traits, imports, and expected types;
- inlay hints for inferred types, parameter names, closures, lifetimes, and chaining;
- code actions and assists;
- macro expansion views;
- diagnostics from its own analysis and from a configurable Cargo check command;
- awareness of Cargo features, build scripts, procedural macros, and target-specific `cfg`.

For an LLM agent, these operations reduce the amount of code that must be placed in context and reduce the risk of broad textual edits.

## 6.2 Configuration domains that materially change fidelity

The semantic model depends on project configuration. An agent must inspect editor or workspace settings before trusting results.

Important domains include:

- **Cargo features:** default features, all features, or a named feature set;
- **target triple:** host target versus a cross target;
- **build-script execution:** whether generated `cfg`, environment variables, and `OUT_DIR` outputs are available;
- **procedural macro support:** whether proc macros are expanded;
- **check command:** `check`, Clippy, or another configured command;
- **workspace discovery:** linked projects, excluded packages, and detached files;
- **target directory:** whether rust-analyzer uses a separate target tree to reduce lock/contention;
- **environment variables:** values that affect build scripts and conditional compilation;
- **sysroot and source:** standard library source availability through `rust-src`.

A diagnostic is only as representative as the active feature/target configuration.

## 6.3 Canonical agent workflow

1. Query workspace symbols or definitions before using text search for a semantic task.
2. Inspect references before renaming or changing a public type.
3. Check implementations and trait bounds before altering a trait.
4. Inspect macro expansion when behavior is generated.
5. Apply the smallest edit.
6. Wait for or request fresh diagnostics.
7. Confirm with the repository's actual `cargo check`, Clippy, and test commands.

rust-analyzer is a fast semantic oracle, not the final build authority.

## 6.4 Limitations and failure modes

- A detached file may not inherit the intended Cargo project.
- Proc-macro or build-script failures can leave parts of the model incomplete.
- Only one feature/target configuration is modeled at a time.
- Generated code may not be available until build scripts run.
- The language server may lag after large workspace mutations.
- Compiler-private/nightly projects can require matching sources and toolchain pinning.
- A green rust-analyzer state does not prove all targets or feature combinations compile.

## 6.5 Agent decision rule

> Prefer rust-analyzer for symbol identity, types, references, implementations, and edit-time diagnostics; prefer Cargo matrix tools for configuration coverage and rustc for authoritative compilation.

---

# 7) `rust-src`

## 7.0 Identity

`rust-src` installs the Rust standard-library source corresponding to a toolchain. It has no user-facing command of its own. It is a substrate consumed by rust-analyzer, Miri, compiler-development tools, standard-library builds, and source navigation.

Official reference: [rustup components][RUSTUP-COMP].

## 7.1 Why it is compelling

Without matching standard-library source, semantic tooling has less direct access to the implementation and source definitions of:

- `core`;
- `alloc`;
- `std`;
- platform abstractions;
- intrinsic-facing APIs;
- standard traits, collections, synchronization primitives, and I/O types.

The component enables:

- go-to-definition into the standard library;
- source-level inspection of standard APIs;
- rust-analyzer sysroot modeling;
- Miri sysroot construction;
- `-Z build-std` and custom-standard-library workflows;
- compiler and MIR tools that need the matching library tree.

For agents, it means “inspect the implementation” can include the toolchain's actual standard-library source rather than relying on remembered behavior.

## 7.2 Toolchain coupling

Install it separately for each toolchain that uses it:

```bash
rustup component add --toolchain stable rust-src
rustup component add --toolchain nightly rust-src
```

A nightly compiler-internals workflow must use the source shipped with the same nightly. Do not point a date-pinned compiler at an arbitrary checkout unless the tool explicitly requires a compiler source checkout and documents that setup.

## 7.3 Limitations

- Source availability is not a public stability guarantee for private implementation details.
- Reading an implementation does not change the API contract.
- Platform-specific code may be selected by `cfg` and differ from the host path an agent first sees.
- `rust-src` is not the same as `rustc-dev`: one provides library source; the other provides compiler libraries and metadata.

## 7.4 Agent decision rule

> Use `rust-src` to improve semantic/source fidelity; do not cite standard-library internals as a stable contract when public documentation says otherwise.

---

# 8) `llvm-tools-preview`

## 8.0 Identity

`llvm-tools-preview` installs selected LLVM utilities built for and distributed with a Rust toolchain. The component is the shared foundation behind several installed tools rather than a normal end-user interface.

Official references: [rustup components][RUSTUP-COMP], [rustc instrumentation coverage][RUST-COV].

## 8.1 Capabilities unlocked

The component supports or materially improves:

- cargo-llvm-cov;
- cargo-binutils wrappers such as `cargo size`, `cargo nm`, and `cargo objdump`;
- cargo-fuzz corpus coverage;
- inspection of LLVM-generated object and executable artifacts;
- merging and rendering source-based coverage data;
- compiler-internals projects that consume LLVM tools.

This creates an important multi-tool chain:

```text
rustc -Cinstrument-coverage
        │
        ▼
raw profile data
        │
   llvm-profdata
        │
        ▼
merged profile
        │
     llvm-cov
        │
        ▼
text / HTML / LCOV / JSON / Cobertura reports
```

and:

```text
Rust build artifact
        │
        ├─ llvm-size
        ├─ llvm-nm
        ├─ llvm-objdump
        └─ llvm-objcopy
```

## 8.2 Toolchain matching

Install the component on the toolchain that performs the instrumented build:

```bash
rustup component add --toolchain stable llvm-tools-preview
rustup component add --toolchain nightly llvm-tools-preview
```

Do not casually mix LLVM tools from one Rust toolchain with artifacts produced by a very different toolchain. Wrapper tools generally locate the matching sysroot utilities and are preferable to manually hard-coding paths.

## 8.3 Why “preview” matters

The component name signals that the exposed utility set and CLI behavior are not covered by Rust's normal stable API guarantees. cargo-binutils also warns that its interface can be influenced by the underlying LLVM tools. Agents should capture tool versions and avoid brittle parsing of human-readable output where a machine-readable format exists.

## 8.4 Agent decision rule

> Treat `llvm-tools-preview` as a version-coupled substrate. Invoke it through higher-level tools unless direct LLVM operation is necessary and documented.

---

# 9) `rustc-dev`

## 9.0 Identity

`rustc-dev` installs compiler libraries and metadata needed by tools that use Rust compiler internals, commonly through `rustc_private` or the evolving compiler-driver/public-internals interfaces. It is unusually relevant to MIR extraction, compiler plugins, custom linting, and code-property-graph generation.

Official references: [rustc development guide][RUSTC-DEV], [rustc public API guide][RUSTC-PUBLIC].

## 9.1 What it enables

Depending on the nightly and chosen API layer, `rustc-dev` enables tools to interact with:

- parsing and AST structures;
- HIR;
- type context and type checking;
- trait selection;
- MIR bodies;
- monomorphization and codegen metadata;
- spans, source maps, diagnostics, and session configuration;
- compiler callbacks and query execution.

For a CPG or semantic indexer, this is the route to facts that syntax-only parsing cannot faithfully infer:

- resolved definitions;
- actual call targets where the compiler can determine them;
- trait and impl relationships;
- type-normalized entities;
- desugared control flow;
- MIR places, operands, terminators, and borrow semantics;
- compilation-state diagnostics.

## 9.2 Nightly pinning is an architectural requirement

Compiler-private APIs can change between nightly releases. A production compiler-integrated tool should therefore treat this tuple as one compatibility unit:

```text
nightly date
rustc-dev component
rust-src component
llvm-tools-preview component
tool source revision
tool lockfile
```

A canonical project pin:

```toml
[toolchain]
channel = "nightly-YYYY-MM-DD"
profile = "minimal"
components = ["rustc-dev", "rust-src", "llvm-tools-preview", "rustfmt", "clippy"]
```

Upgrade intentionally:

1. create an upgrade branch;
2. change the nightly date;
3. refresh compile errors against the new compiler APIs;
4. rerun extraction golden tests;
5. compare graph/fact counts and semantic diffs;
6. benchmark performance;
7. document changed compiler semantics.

## 9.3 `rustc-dev` is not required for ordinary application development

Do not impose it on every Rust repository. It increases toolchain size and couples code to unstable internals. Use it when the task genuinely needs compiler queries or MIR-level fidelity.

## 9.4 Agent guardrails

- Never “fix” compiler-internals code by switching to the latest nightly without evaluating semantic output.
- Avoid storing raw compiler-internal IDs as durable cross-run identities; derive stable identities from source/package/definition semantics.
- Separate compiler diagnostics from extraction failures.
- Treat partially compiling workspaces as a first-class state rather than silently serving stale facts.
- Keep syntax-level fallback paths available when semantic compilation fails.

## 9.5 Agent decision rule

> Use `rustc-dev` only for compiler-grade facts unavailable from stable APIs; pin the entire compiler/tool tuple and test semantic output, not just compilation.

---

# 10) Miri

## 10.0 Identity and mental model

Miri interprets Rust's Mid-level Intermediate Representation and checks many classes of undefined behavior that ordinary execution may not expose. It is a dynamic interpreter, not a theorem prover and not a native-speed sanitizer.

Official reference: [Miri repository and user guide][MIRI].

## 10.1 High-value checks

Miri can detect, within supported execution paths:

- out-of-bounds and use-after-free memory access;
- invalid uninitialized-value use;
- violated reference and pointer aliasing rules;
- invalid values for Rust types;
- misaligned access;
- memory leaks, subject to configuration;
- data races and some weak-memory behaviors;
- unsupported or invalid pointer provenance operations;
- violations that arise only under selected thread schedules or allocation layouts.

This is particularly valuable for:

- custom unsafe data structures;
- FFI boundary wrappers;
- memory-mapped or pointer-heavy code;
- parsers with manual buffer manipulation;
- lock-free/concurrent structures;
- code that converts between integers and pointers;
- CPG/MIR extraction infrastructure that uses compiler-private or unsafe APIs.

## 10.2 Canonical commands

Prepare the nightly sysroot:

```bash
cargo +nightly miri setup
```

Run tests:

```bash
cargo +nightly miri test
```

Run a binary:

```bash
cargo +nightly miri run
```

Run a filtered test:

```bash
cargo +nightly miri test test_name_fragment
```

Explore several randomized executions:

```bash
MIRIFLAGS="-Zmiri-many-seeds=0..16" cargo +nightly miri test
```

Cross-interpret a target property:

```bash
cargo +nightly miri test --target s390x-unknown-linux-gnu
```

The last example is valuable for endian-sensitive logic. It does not replace native or emulated target testing.

## 10.3 Miri plus nextest

When both Miri and cargo-nextest are installed:

```bash
cargo +nightly miri nextest run -jN
```

This restores test-level parallelism by running tests in separate interpreter processes. It is often dramatically faster for large suites, but it changes semantics:

- one test per process can improve isolation and reporting;
- it can require repeated test-crate execution overhead;
- it cannot detect races between separate tests sharing an external resource;
- nextest does not execute doctests.

Therefore retain a smaller `cargo miri test` job when cross-test shared-resource races matter, and run doctests separately where Miri-compatible.

## 10.4 Multiple seeds are not optional for concurrent/unsafe risk

Miri randomizes aspects such as allocation addresses and thread interleavings. One successful seed covers one explored execution. For high-risk unsafe or concurrent code:

```bash
MIRIFLAGS="-Zmiri-many-seeds=0..64" cargo +nightly miri test -p critical-crate
```

Use a smaller range locally and a larger scheduled sweep. On failure, rerun the specific seed using `-Zmiri-seed=<n>` and preserve it as regression evidence.

## 10.5 Isolation and unsupported operations

Miri intentionally isolates interpreted code from much of the host. FFI, networking, operating-system calls, and platform APIs may be unsupported or shimmed. An “unsupported operation” is not proof of a program bug.

Do not reflexively set:

```text
-Zmiri-disable-isolation
```

Doing so grants host access and changes reproducibility and safety assumptions. Prefer:

- isolate the pure core from I/O wrappers;
- use `#[cfg(miri)]` or `#[cfg_attr(miri, ignore)]` sparingly and document why;
- pass deterministic environment values with the narrowest supported Miri option;
- preserve an ordinary native integration test for excluded code.

## 10.6 What Miri does not prove

Miri does not prove:

- every input was tested;
- every schedule was tested;
- native FFI code is safe;
- production optimization/codegen behaves identically;
- logic is functionally correct;
- resource usage or performance is acceptable;
- unsupported target APIs work.

Combine it with fuzzing, mutation testing, ordinary tests, and target execution.

## 10.7 Agent decision rule

> Run Miri when unsafe code, concurrency, pointer semantics, or FFI wrappers change; record target and seed; treat unsupported operations separately from UB findings; never claim exhaustive proof.

---

# Part II — Tool acquisition, orchestration, and fast iteration

# 11) `cargo-binstall`

## 11.0 Identity

cargo-binstall installs Cargo-distributed command-line programs by locating prebuilt release artifacts, with source compilation as a fallback. It is designed to avoid the time and build dependency burden of compiling every developer tool locally.

Official reference: [cargo-binstall][BINSTALL].

## 11.1 Why it is compelling

A rich Rust workstation may contain dozens of Cargo tools. Rebuilding all of them from source:

- consumes substantial CPU time;
- pollutes caches;
- requires every tool's native build dependencies;
- makes machine provisioning slow;
- makes ephemeral CI setup unnecessarily expensive.

cargo-binstall uses crate metadata and linked release infrastructure to locate compatible binaries. Where supported, it provides a near drop-in alternative to:

```bash
cargo install <crate>
```

with:

```bash
cargo binstall <crate>
```

## 11.2 Reproducible installation

For automated setup, use explicit versions and noninteractive mode:

```bash
cargo binstall cargo-nextest@<version> --no-confirm
cargo binstall cargo-deny@<version> --no-confirm
```

Avoid unversioned installs in reproducible CI. A global developer bootstrap may use current releases, but it should capture the resulting versions.

## 11.3 Artifact provenance and trust boundary

Prebuilt installation changes the supply-chain path:

```text
source build:
crate source → local rustc → local executable

binstall:
crate metadata → release artifact source → downloaded executable
```

cargo-binstall performs transport and package checks and has limited support for signed artifacts, including an option to require signatures where available. However:

- a checksum proves artifact integrity relative to the published checksum, not benign behavior;
- not every project publishes signed binaries;
- third-party quick-install infrastructure can participate in artifact resolution;
- falling back to `cargo install` changes both cost and execution behavior.

For a hardened environment, inspect available strategies and consider:

```bash
cargo binstall --only-signed <crate>
```

only where the selected packages actually publish compatible signatures. Do not turn a signature requirement into a false claim that all ecosystem binaries are signed.

When the QuickInstall artifact strategy is used, current cargo-binstall documentation notes that limited package/version/target/user-agent telemetry may be emitted by that strategy. Privacy-sensitive environments can disable it through the documented command-line/environment configuration, for example:

```bash
export BINSTALL_DISABLE_TELEMETRY=true
```

or can disable the QuickInstall strategy entirely. Record this as environment policy rather than relying on each agent to remember it.

## 11.4 Agent rules

- Use cargo-binstall for environment provisioning, not project dependency management.
- Pin versions in CI.
- Report when it fell back to source compilation.
- Do not silently install a missing tool during a code-review task unless environment mutation is authorized.
- Do not use `--skip-signatures` as a routine convenience.
- Treat a newly installed executable as code execution from the package's distribution channel.

## 11.5 Integration with cargo-update

cargo-binstall and cargo-update form a lifecycle pair:

```text
initial installation → cargo-binstall
periodic global update → cargo install-update
```

That combination is appropriate for a developer workstation. It is not a substitute for pinned CI tooling.

---

# 12) `cargo-update`

## 12.0 Identity

The `cargo-update` package supplies Cargo subcommands—most notably `cargo install-update`—for checking and updating executables previously installed through Cargo.

Official reference: [cargo-update][CARGO-UPDATE].

## 12.1 Compelling capabilities

Typical commands:

```bash
cargo install-update -a
cargo install-update cargo-nextest cargo-deny
```

It can:

- list installed Cargo executables and available updates;
- update all or selected tools;
- preserve per-tool installation configuration;
- configure toolchain, features, default-feature behavior, and version constraints for future updates;
- use cargo-binstall when available for faster upgrades.

This closes an operational gap: Cargo manages project dependencies well, but globally installed developer tools otherwise become a manually maintained fleet.

## 12.2 Correct usage model

Use it for **developer environment maintenance**, followed by validation:

```bash
cargo install-update -a
rustc -vV
cargo install --list
just ci-fast
```

For critical workstations, save the before/after inventory.

## 12.3 What it does not update

It does not replace:

```bash
rustup update
rustup component add ...
```

Rustup manages compilers and rustup components. cargo-update manages Cargo-installed executables.

It also does not update a project's `Cargo.lock`; `cargo update` is the separate Cargo command that does that.

## 12.4 Agent guardrails

- Never confuse `cargo install-update` with `cargo update`.
- Do not update global tools in the middle of debugging unless version drift is the suspected cause.
- Do not run `-a` in CI expecting reproducibility.
- After an upgrade, rerun representative workflows because output schemas and flags may change independently.
- Preserve an explicit version pin for tools parsed by automation.

---

# 13) `sccache`

## 13.0 Identity

sccache is a compiler wrapper that stores reusable compilation outputs in a local, remote, or multi-level cache. It supports Rust and several native compiler families.

Official reference: [sccache][SCCACHE].

## 13.1 Why it matters for agent throughput

Programming agents often create many nearby build states:

1. inspect;
2. make a small edit;
3. compile;
4. revise;
5. test a subset;
6. switch branches or worktrees;
7. run another tool that rebuilds a similar graph.

Without a compilation cache, repeated compilation becomes the limiting factor in agent iteration. sccache can reuse artifacts when the compiler inputs match, reducing latency for:

- branch switches;
- clean-ish builds;
- multiple worktrees;
- CI workers sharing a remote cache;
- repeated builds across `cargo check`, test, and specialized tools;
- repositories with large stable dependency graphs.

## 13.2 Canonical Cargo configuration

```toml
# .cargo/config.toml or ~/.cargo/config.toml
[build]
rustc-wrapper = "sccache"
```

or for a scoped invocation:

```bash
RUSTC_WRAPPER=sccache cargo build
```

Inspect effectiveness:

```bash
sccache --show-stats
sccache --zero-stats
```

A best-in-class environment tracks hit rate rather than merely assuming the wrapper helps.

## 13.3 Cache topology

sccache supports:

- local disk cache;
- remote object or key-value backends;
- multi-level caches with local-first behavior and backfill;
- optional distributed compilation modes.

The value and risk differ:

```text
local cache:
fast, simple, machine-scoped

shared remote cache:
higher fleet reuse, but requires credentials, isolation, retention,
latency, and trust policy

distributed compilation:
additional execution infrastructure and a larger security/operations surface
```

## 13.4 Cache correctness and reproducibility

A cache hit is valid only if all relevant inputs are represented in the cache key. Build scripts, environment variables, compiler versions, target settings, profiles, and flags can influence outputs.

Agent rules:

- do not manually copy cache entries;
- do not use cache success as evidence of a fresh semantic check;
- capture compiler and target when comparing builds;
- isolate caches where confidential artifacts or incompatible trust domains exist;
- use cache-busting or recache controls when investigating possible corruption;
- monitor hit/miss and error statistics;
- allow an intentional bypass when diagnosing compiler-wrapper behavior:

```bash
RUSTC_WRAPPER= cargo build
```

## 13.5 Interaction with coverage and profiling

Coverage-instrumented and profiling builds use different flags from normal builds, so they should naturally occupy distinct cache keys. A repository may also use separate target directories to avoid accidental artifact confusion:

```bash
CARGO_TARGET_DIR=target/coverage cargo llvm-cov ...
CARGO_TARGET_DIR=target/profiling cargo build --profile profiling
```

Do not “optimize” by forcing incompatible profiles to share artifacts.

## 13.6 Agent decision rule

> Keep sccache enabled for normal iteration, measure its hit rate, and bypass it only for focused diagnosis. It accelerates evidence production; it does not create evidence.

---

# 14) `just`

## 14.0 Identity

Just is a cross-platform command runner that stores named recipes in a `justfile`. It resembles a build-oriented command language but is intentionally suited to project tasks rather than dependency-based artifact construction.

Official reference: [Just manual][JUST].

## 14.1 Why it is foundational for LLM agents

A repository with 30 installed tools should not require every agent to reconstruct the correct command-line combinations. A `justfile` becomes a stable API:

```bash
just --list
just check
just test
just ci-fast
just ci-full
just coverage
just miri
```

This provides:

- discoverable named tasks;
- parameters and defaults;
- recipe dependencies;
- environment loading;
- working-directory handling;
- modular imported recipe files;
- grouped/annotated commands;
- shell selection;
- guard/confirmation attributes for dangerous tasks;
- consistent commands for humans, agents, and CI.

## 14.2 Command-contract design

Good recipe names express intent, not implementation:

```text
good: test-unit
good: ci-fast
good: release-verify
bad: run-nextest-with-these-13-flags
```

The implementation can change without retraining the agent.

A canonical skeleton:

```just
set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

check:
    cargo check --workspace --all-targets

test:
    cargo nextest run --workspace

doctest:
    cargo test --workspace --doc

ci-fast: check test doctest
    cargo deny check
    cargo audit
    typos
```

On repositories that must support Windows without Bash, use platform-specific recipes or a portable shell strategy rather than copying this skeleton blindly.

## 14.3 Parameters and scoped operations

Agents should be able to target a package or test without editing recipes:

```just
test-package package:
    cargo nextest run -p {{package}}

test-filter filter:
    cargo nextest run -E 'test({{filter}})'
```

Validate quoting carefully. Recipe parameters are not automatically safe shell tokens; avoid passing untrusted strings into shell interpolation.

## 14.4 Dangerous operations

Mark mutating or release actions visibly and require explicit confirmation where supported:

```text
publish
accept-snapshots
fix-dependencies
update-toolchain
```

Do not hide destructive work behind an innocuous recipe such as `fix`.

## 14.5 Machine-readable discovery

An agent should first run:

```bash
just --list
```

and, where useful, inspect Just's available metadata/listing output for the installed version. It should prefer declared recipes over ad hoc equivalents unless investigating the recipe itself.

## 14.6 Agent decision rule

> Treat the `justfile` as the repository's operational API. Call direct tools when no suitable recipe exists, when narrowing scope, or when debugging the recipe.

---

# 15) Bacon

## 15.0 Identity

Bacon is a persistent, Rust-aware background code checker. It watches the workspace, runs configured jobs, and presents compact diagnostics designed for rapid edit-feedback cycles.

Official reference: [Bacon][BACON].

## 15.1 Compelling capabilities

Bacon can orchestrate jobs around:

- `cargo check`;
- Clippy;
- ordinary tests;
- cargo-nextest;
- documentation checks;
- Miri;
- custom commands.

Its key advantage is not merely file watching. It understands Rust diagnostic output and maintains a focused view of current failures, reducing terminal noise during repeated edits.

It can also emit location information that external editors or agents can consume, including a `.bacon-locations` workflow in supported configurations.

## 15.2 Bacon versus rust-analyzer

They overlap but are not redundant:

| rust-analyzer | Bacon |
|---|---|
| maintains semantic editor model | executes repository commands continuously |
| fast incremental diagnostics | authoritative Cargo/rustc job output |
| navigation, rename, completion | terminal-centric failure queue |
| one active project configuration | configurable job selection |
| editor protocol | standalone development loop |

Use both. rust-analyzer catches issues as code is formed; Bacon confirms the configured build/check behavior.

## 15.3 Bacon versus Watchexec

Bacon is preferred when:

- the command is Rust/Cargo-centric;
- diagnostics should be parsed and navigable;
- the desired workflow is an interactive code-check dashboard.

Watchexec is preferred when:

- arbitrary files or languages trigger work;
- a server or daemon must restart;
- event kinds and changed paths matter;
- JSON filesystem events feed another process.

Do not run Bacon and Watchexec on the same expensive command unless duplication is intentional.

## 15.4 Suggested jobs

Keep the default job cheap:

```text
default edit loop → cargo check or Clippy
manual switch     → targeted nextest
manual switch     → docs
scheduled/manual  → Miri
```

Miri, mutation testing, full feature powersets, and fuzzing do not belong in an every-save Bacon job.

## 15.5 Agent workflow

An editor-integrated agent can use Bacon in two modes:

1. **human-supervised persistent mode:** a developer sees diagnostics while the agent edits;
2. **machine-readable location mode:** the agent reads the current failure locations, fixes one cluster, then requests a fresh run.

The agent must ensure output is fresh after its edit and must not quote a stale Bacon screen as the current build state.

---

# 16) Watchexec

## 16.0 Identity

Watchexec is a general-purpose cross-platform filesystem watcher that runs, restarts, or signals commands when matching events occur.

Official reference: [Watchexec CLI][WATCHEXEC].

## 16.1 High-value capabilities

Watchexec supports:

- recursive watching;
- ignore-file integration, including `.gitignore` and `.ignore`;
- extension and glob filters;
- filtering by filesystem event type;
- event coalescing for editor save patterns;
- command restarts;
- process-group management;
- configurable signals;
- desktop notifications;
- timing information;
- changed-path environment variables;
- JSON event emission.

That makes it useful for tasks outside Bacon's Rust-specific model:

```bash
watchexec -e rs,toml -- just ci-fast
watchexec -w config -r -- cargo run --bin daemon
watchexec --emit-events-to=json-stdio --only-emit-events
```

## 16.2 Relevance to daemon and CPG development

For a code-intelligence daemon, Watchexec is valuable as:

- a development harness around the daemon;
- a way to restart a local server on binary/config changes;
- a source of reproducible filesystem-event streams;
- a test tool for ignore/debounce/coalescing behavior;
- a means of triggering validation scripts after schema or query changes.

It should not be confused with the daemon's embedded production watcher. A command-line watcher is a development-control tool, not necessarily the library architecture used in the shipped service.

## 16.3 Process lifecycle matters

When restarting servers, verify:

- whether Watchexec runs through a shell;
- which process group receives termination;
- which signal is sent;
- whether child processes are cleaned up;
- whether the replacement starts before the old process releases ports/files;
- whether a debounce window is adequate.

Agents should not stack multiple shell layers casually; signal delivery and quoting can change.

## 16.4 Agent decision rule

> Use Watchexec when changed-file events or process restarts are the primary abstraction; use Bacon when Rust diagnostic feedback is the primary abstraction.

---

# 17) `fd`

## 17.0 Identity

`fd` is a fast, parallel file-discovery utility with ergonomic regex/glob filtering and ignore-aware defaults.

Official reference: [fd][FD].

## 17.1 Why it remains useful in a semantic environment

Semantic tools answer symbol questions; file discovery answers repository-shape questions:

- locate manifests, snapshots, schemas, migrations, generated files, and fixtures;
- enumerate all files with a particular extension;
- find files outside the active Cargo workspace;
- execute a command over a selected set;
- inspect directory topology before choosing a search strategy.

Examples:

```bash
fd 'Cargo\.toml$'
fd -e rs
fd 'snapshot' tests
fd -e toml -x typos {}
```

## 17.2 Ignore behavior is both a feature and a trap

By default, `fd` usually respects ignore rules and omits hidden paths. This is ideal for normal source discovery, but an agent performing a completeness or repository-forensics task must consciously include excluded paths:

```bash
fd -H ...     # include hidden
fd -I ...     # do not respect ignore files
fd -u ...     # unrestricted shortcut where supported
```

Do not claim that a file does not exist without accounting for ignore and hidden-file behavior.

## 17.3 Parallel execution guardrail

`fd -x`/`--exec` can run commands over many files, often concurrently. That is useful for read-only formatting or inspection, but dangerous for:

- commands that mutate shared files;
- tools using one shared output path;
- package managers;
- scripts not safe for parallel execution.

Use batch execution or explicit serialization when required.

## 17.4 Agent decision rule

> Use `fd` for fast file-set construction; state whether ignore and hidden rules were active whenever completeness matters.

---

# 18) Typos (`typos-cli`)

## 18.0 Identity

Typos is a fast source-code spell checker designed to minimize false positives in identifiers, file names, comments, and structured repositories.

Official reference: [Typos][TYPOS].

## 18.1 Why it is more than cosmetic

Spelling errors can create:

- confusing public APIs;
- inconsistent telemetry names;
- silently mismatched configuration keys;
- poor diagnostics;
- bad comments and documentation;
- duplicate concepts caused by two spellings;
- brittle schema/query contracts.

For LLM-generated code, low-cost spelling review is valuable because model output can introduce plausible but inconsistent identifiers across many files.

## 18.2 Canonical modes

Check:

```bash
typos
```

Show a patch/diff where supported:

```bash
typos --diff
```

Write changes only after review:

```bash
typos -w
```

Use machine-readable output when integrating an agent or CI:

```bash
typos --format json
```

Consult the installed version's help because formatting and correction flags can evolve.

## 18.3 Configuration policy

Store project decisions in `_typos.toml` or another supported project configuration location:

- intentional domain words;
- identifier-specific corrections;
- excluded generated/vendor paths;
- file-type overrides;
- case-sensitive exceptions;
- locale/language expectations.

Do not globally exclude a whole directory when a narrow word exception will preserve more coverage.

## 18.4 Auto-fix guardrail

Spelling fixes can alter:

- public identifiers;
- serialized field names;
- database columns;
- CLI flags;
- protocol strings;
- snapshots;
- test fixtures.

Therefore an agent must inspect the semantic effect of every source-code correction. Prefer automatic writes for comments and docs only when the diff is unambiguous.

## 18.5 Agent decision rule

> Run Typos as a cheap PR gate; treat identifier corrections as API changes until proven otherwise.

---

# Part III — Testing, coverage, behavioral strength, and adversarial execution

# 19) cargo-nextest

## 19.0 Identity

cargo-nextest is an infrastructure-oriented Rust test runner. It builds Rust test binaries through Cargo, enumerates individual tests, and executes them with richer scheduling, isolation, configuration, reporting, retries, partitioning, and archival behavior than the default harness invocation.

Official reference: [cargo-nextest][NEXTEST].

## 19.1 Why it is compelling

For nontrivial workspaces, nextest provides:

- fast parallel test execution;
- one-test-per-process isolation;
- reliable termination and signal handling;
- per-test settings;
- retries with fixed or exponential backoff;
- explicit treatment of flaky tests;
- slow-test detection and termination;
- test groups and concurrency limits;
- filter expressions;
- default filters;
- setup and wrapper scripts;
- JUnit output;
- test partitioning across CI workers;
- archive-and-reuse workflows;
- record/replay/rerun capabilities in current releases;
- integrations with coverage, mutation testing, Miri, and debuggers.

The value is not simply “more threads.” It is predictable test infrastructure.

## 19.2 Canonical commands

Run workspace tests:

```bash
cargo nextest run --workspace
```

Run all features:

```bash
cargo nextest run --workspace --all-features
```

Run a package:

```bash
cargo nextest run -p package-name
```

Use a filterset:

```bash
cargo nextest run -E 'test(parser) & package(core-crate)'
```

Select a repository profile:

```bash
cargo nextest run -P ci
```

Agents should prefer filter expressions or package scope over repeatedly running an entire large workspace while debugging one failure.

## 19.3 Repository configuration

nextest reads repository configuration from:

```text
.config/nextest.toml
```

A defensible starting point:

```toml
[profile.default]
fail-fast = true
slow-timeout = { period = "60s", terminate-after = 2 }

[profile.ci]
fail-fast = false
retries = { backoff = "exponential", count = 2, delay = "1s", max-delay = "10s", jitter = true }
flaky-result = "fail"
slow-timeout = { period = "60s", terminate-after = 3 }
global-timeout = "2h"

[profile.ci.junit]
path = "junit.xml"
```

This is a policy example, not a universal recommendation:

- retries may hide nondeterminism if flaky results are still treated as passing;
- short timeouts may be inappropriate for instrumented, Miri, or constrained CI runs;
- network tests may need specific backoff;
- integration tests may need test groups or setup scripts;
- the repository's installed nextest version must support the selected fields.

Current nextest releases expose the repository schema locally through their help system; an agent should validate configuration rather than guessing fields.

## 19.4 Flakiness policy

Retries should answer “is this failure nondeterministic?” rather than make a red test disappear.

Recommended CI policy:

```toml
[profile.ci]
retries = 2
flaky-result = "fail"
```

This permits diagnostic retries while still failing the build when a test only passes on rerun. Marking known flaky tests as allowed should be narrow, temporary, owned, and measurable.

## 19.5 Test grouping and scarce resources

Some tests cannot safely run at unconstrained parallelism because they share:

- databases;
- ports;
- GPUs;
- filesystem fixtures;
- rate-limited services;
- large memory budgets.

Use nextest test groups and per-test overrides rather than globally setting one test thread. This retains parallelism for safe tests while enforcing the real resource constraint.

## 19.6 Partitioning and archives

Large CI suites can be split across workers. Hash-based partitions are stable for a test set; count/slice-based partitions distribute contiguous slices. Archive workflows can separate compilation from execution and reuse built tests across machines or stages.

Agents must preserve:

- exact target;
- feature set;
- profile;
- nextest version;
- archive format expectations;
- test list/filter;
- target execution compatibility.

A compiled archive does not make a binary runnable on an incompatible worker.

## 19.7 Critical limitation: doctests

nextest does not currently run doctests. Every comprehensive suite needs a separate step:

```bash
cargo test --workspace --doc --all-features
```

An agent must never report “all Rust tests passed” from nextest alone when the repository uses doctests.

## 19.8 Integration with other tools

```text
nextest + llvm-cov  → fast source-based coverage
nextest + mutants   → mutation execution backend
nextest + Miri      → parallel interpreter processes
nextest + CI        → partitions, archives, JUnit
nextest + Insta     → snapshot tests run as ordinary tests
```

Each integration can change process isolation and build behavior; record the exact command.

## 19.9 Agent decision rule

> Use nextest as the default ordinary test runner in substantial workspaces, but always add doctests separately and treat retries as flakiness evidence rather than success laundering.

---

# 20) cargo-llvm-cov

## 20.0 Identity

cargo-llvm-cov wraps Rust's LLVM source-based coverage instrumentation and reporting pipeline. It can run Cargo tests, nextest, binaries, examples, and other targets while producing precise line, region, function, and—where supported—branch coverage.

Official reference: [cargo-llvm-cov][LLVM-COV].

## 20.1 Why it is compelling

Coverage answers:

- which code regions executed;
- which branches or functions were not exercised;
- whether a new error path has a test;
- whether a fuzz corpus reaches intended logic;
- whether a package or workspace regressed relative to a baseline.

It supports common outputs such as:

- terminal summaries;
- HTML;
- LCOV;
- JSON;
- Cobertura XML;
- Codecov-compatible reports.

## 20.2 Canonical commands

Workspace coverage with ordinary test harness:

```bash
cargo llvm-cov --workspace --all-features
```

Coverage using nextest:

```bash
cargo llvm-cov nextest --workspace --all-features
```

HTML report:

```bash
cargo llvm-cov nextest --workspace --all-features --html
```

LCOV artifact:

```bash
cargo llvm-cov nextest \
  --workspace \
  --all-features \
  --lcov \
  --output-path target/coverage/lcov.info
```

Fail below a chosen policy threshold only if the repository has consciously defined one:

```bash
cargo llvm-cov --fail-under-lines <percentage>
```

Do not invent an arbitrary threshold merely because the tool supports it.

## 20.3 Merging multiple execution sources

A mature repository may need coverage from:

- unit tests;
- integration tests;
- external black-box tests;
- examples;
- a Python process loading a Rust extension;
- fuzz corpus replay;
- generated protocol tests.

cargo-llvm-cov supports environment-driven and deferred reporting workflows. The general model is:

```text
clean/start coverage environment
        │
        ├─ execute source A
        ├─ execute source B
        └─ execute source C
        │
        ▼
merge profile data
        │
        ▼
render one report
```

Use the installed help for exact `--no-report`, `--no-clean`, and environment-export options. The agent must ensure every execution used compatible instrumented artifacts and profile paths.

## 20.4 Coverage plus nextest

nextest often speeds up the execution portion:

```bash
cargo llvm-cov nextest
```

Retain a separate doctest coverage step if doctests matter and the current toolchain/tool release supports the desired mode. Doctest instrumentation has historically required nightly or additional constraints; check the installed documentation rather than assuming parity.

## 20.5 Coverage exclusions

Generated code, macros, test utilities, or unreachable platform code can distort percentages. Exclude narrowly and transparently. Do not remove hard-to-test production logic from the report simply to improve a metric.

A coverage report should identify:

- workspace/package scope;
- feature set;
- target;
- excluded paths;
- whether tests, doctests, external tests, and fuzz corpus replay were included.

## 20.6 What coverage does not prove

A covered line may have no meaningful assertion. A test may execute an error branch without validating its output. This is why cargo-mutants complements cargo-llvm-cov.

The useful triangulation is:

```text
uncovered code
    → add reachability test or justify exclusion

covered + surviving mutant
    → assertion/behavior gap

covered + mutant killed
    → stronger evidence that the test constrains behavior
```

## 20.7 Agent decision rule

> Use coverage to identify unexecuted regions and regression trends; never equate a high percentage with behavioral correctness.

---

# 21) cargo-insta

## 21.0 Identity

cargo-insta is the command-line companion to the Insta snapshot-testing library. Snapshot tests compare a current structured output against a reviewed reference artifact and provide workflows for inspecting, accepting, rejecting, and cleaning up changes.

Official references: [Insta][INSTA], [cargo-insta command guide][INSTA-CLI].

## 21.1 Why snapshot testing is compelling

Snapshot tests are especially effective for outputs that are:

- large;
- structured;
- human-reviewable;
- expected to evolve;
- awkward to encode as many individual assertions.

High-value Rust uses include:

- AST, HIR, MIR, and CPG fragments;
- serialized schemas;
- query plans;
- diagnostics and error messages;
- command output;
- normalized API responses;
- generated code;
- configuration expansion;
- parser recovery behavior.

For an LLM agent, a snapshot diff exposes broad unintended output changes that may be missed by narrow assertions.

## 21.2 Canonical workflow

Run snapshot tests:

```bash
cargo insta test
```

Review pending changes interactively:

```bash
cargo insta review
```

Inspect pending snapshots:

```bash
cargo insta pending-snapshots
```

Accept or reject only after semantic review:

```bash
cargo insta accept
cargo insta reject
```

The exact command names/options should be checked against the installed release.

## 21.3 Inline versus external snapshots

Insta supports:

- external `.snap` files;
- inline snapshots embedded in source;
- debug snapshots;
- serialized formats such as JSON/YAML/RON where enabled;
- redactions and normalization.

Use external snapshots for large outputs and inline snapshots for small, highly local expectations. Keep snapshot location predictable for agents.

## 21.4 Redactions and normalization

Nondeterministic fields such as timestamps, random IDs, temporary paths, pointer addresses, or unordered map iteration can make snapshots noisy. Insta redactions or application-level normalization can stabilize them.

Rules:

- normalize only fields that are semantically irrelevant;
- preserve values that reveal real regressions;
- sort unordered collections before snapshotting when order is not part of the contract;
- record the normalization in test code, not in an agent's mental model.

## 21.5 Snapshot review is a code review

An agent must not auto-accept snapshots because tests failed after an intentional code change. The diff may reveal:

- lost graph nodes;
- changed identifiers;
- new nondeterminism;
- altered error classifications;
- accidental ordering;
- leaked paths or secrets;
- broad schema drift.

A safe workflow:

1. run tests without automatic acceptance;
2. inspect every pending diff;
3. correlate each change to intended source edits;
4. accept only the intended subset;
5. rerun tests from a clean state;
6. review the committed `.snap` diff.

## 21.6 Integration with nextest and mutation testing

Insta tests are ordinary Rust tests and can be run under nextest. cargo-mutants can determine whether changing snapshot-producing logic causes failures. This combination is strong for compiler and graph tooling:

```text
input corpus
   │
   ▼
semantic transformation
   │
   ▼
normalized snapshot
   │
   ├─ nextest checks regression
   └─ mutants checks whether the snapshot constrains behavior
```

Avoid enormous snapshots that reviewers no longer read; split by semantic concern.

## 21.7 Agent decision rule

> Use snapshots for complex reviewable outputs; never accept them without explaining the semantic delta.

---

# 22) cargo-mutants

## 22.0 Identity

cargo-mutants performs mutation testing: it makes small source changes—mutants—and runs tests to determine whether the suite detects them. A mutant that survives identifies behavior the tests may not constrain.

Official reference: [cargo-mutants][MUTANTS].

## 22.1 Why it adds evidence that coverage cannot

Coverage asks:

```text
Did a test execute this code?
```

Mutation testing asks:

```text
Would the tests fail if this code were wrong in a plausible way?
```

A line can be fully covered while its result is ignored. Typical mutations include:

- replacing a returned value;
- changing arithmetic;
- negating a condition;
- deleting or replacing a function body;
- changing a comparison;
- altering boolean behavior.

If tests still pass, the agent should investigate missing assertions, equivalent mutants, unreachable states, or insufficient test scope.

## 22.2 Canonical commands

Run against the workspace/package selection:

```bash
cargo mutants
```

Target a file:

```bash
cargo mutants -f src/module.rs
```

Use a narrower package or test filter according to installed help and repository configuration. cargo-mutants can use ordinary `cargo test` or nextest-backed execution.

## 22.3 Baseline requirements

Mutation testing assumes:

- the unmodified suite passes;
- tests are sufficiently deterministic;
- timeouts are meaningful;
- the test command exercises the mutated package;
- generated/source exclusions are intentional.

A flaky baseline produces meaningless survivor and timeout classifications.

## 22.4 Result classes

An agent should distinguish:

- **caught/killed:** tests failed under the mutation;
- **survived/missed:** tests passed;
- **timeout:** tests did not finish within policy;
- **unviable:** mutation did not compile;
- **equivalent or irrelevant:** mutation changes syntax but not observable semantics in the supported domain.

Do not label every survivor a bug. Triage it.

## 22.5 Incremental and scheduled use

Full mutation campaigns can be expensive. Recommended cadence:

- changed file/module on a risky PR;
- critical crate nightly;
- full workspace periodically;
- focused rerun after adding tests.

Mutation testing is especially valuable for:

- validation logic;
- safety checks;
- parsers and graph transformations;
- financial/engineering calculations;
- permission decisions;
- state-machine transitions;
- code with high coverage but high defect impact.

## 22.6 Combining coverage and mutants

Prioritize survivors in covered regions:

```text
not covered + mutant survives
    first solve reachability

covered + mutant survives
    add or strengthen assertions

mutant times out
    inspect infinite-loop or termination sensitivity

mutant unviable
    not a behavioral gap
```

## 22.7 Maintenance caveat

Mutation tooling evolves and can be computationally demanding. Pin the tool version in CI, retain reports, and avoid parsing unstable human output when a structured format is available.

## 22.8 Agent decision rule

> Use mutation testing to evaluate test sensitivity, not as a binary project score. Every survivor requires semantic triage.

---

# 23) cargo-fuzz

## 23.0 Identity

cargo-fuzz is the recommended Cargo integration for libFuzzer-based coverage-guided fuzzing of Rust code. It creates and manages fuzz targets, corpora, artifacts, builds, and execution.

It normally relies on a nightly Rust toolchain and LLVM sanitizer/libFuzzer support. Pin that nightly in CI rather than consuming an unbounded rolling compiler, and install `llvm-tools-preview` on the same nightly when using corpus coverage.

Official reference: [Rust Fuzz Book][FUZZ].

## 23.1 Why it is compelling

Fuzzing continuously mutates inputs and uses coverage feedback to discover executions that normal test authors did not anticipate. It is highly valuable for:

- parsers;
- decoders and serializers;
- query languages;
- file formats;
- network protocols;
- path handling;
- unsafe buffer manipulation;
- graph import/export;
- compression or binary formats;
- state machines with compact input encodings.

## 23.2 Project initialization and target structure

Initialize:

```bash
cargo fuzz init
```

Add a target:

```bash
cargo fuzz add target_name
```

Run:

```bash
cargo fuzz run target_name
```

A fuzz target should be:

- deterministic where possible;
- fast;
- free of uncontrolled network and disk effects;
- able to reject invalid input cheaply;
- narrow enough for the fuzzer to learn useful coverage;
- built around a reusable production function rather than a duplicate parser.

## 23.3 Corpus and artifact semantics

Typical structure:

```text
fuzz/
├── fuzz_targets/
├── corpus/<target>/
└── artifacts/<target>/
```

- **Corpus inputs** are useful seeds and discovered coverage-expanding cases.
- **Artifacts** are crashing, timeout, or otherwise noteworthy inputs.
- A minimized crashing input should become a permanent regression test where practical.
- Corpus growth should be reviewed; do not commit unlimited redundant data.

## 23.4 Reproducing and minimizing failures

On failure, preserve:

- fuzz target;
- exact artifact;
- toolchain;
- sanitizer/libFuzzer options;
- feature/target configuration;
- stack trace;
- minimized reproducer.

Use cargo-fuzz's reproduce/minimize workflows as exposed by the installed version. Do not “fix” a crash and delete the input before adding a deterministic regression test.

## 23.5 Structure-aware fuzzing

Raw bytes are ideal for some parsers, but complex APIs may need structured input generation. A structured fuzzer can derive typed inputs and still serialize them into the fuzzer's byte stream.

Use structure-aware fuzzing when random bytes rarely pass initial validation. Preserve some malformed-input targets as well, because parser rejection behavior is itself important.

## 23.6 Coverage of the fuzz corpus

With `llvm-tools-preview`, cargo-fuzz can replay the current corpus under source-based coverage:

```bash
cargo fuzz coverage target_name
```

This reveals paths the corpus does not reach. The integrated workflow is:

```text
seed corpus
    │
cargo fuzz run
    │
new corpus/crash artifacts
    │
cargo fuzz coverage
    │
identify unreachable semantic states
    │
improve seeds, dictionary, or target decomposition
```

Coverage of a corpus does not prove the fuzzer explored all values within a covered region.

## 23.7 Fuzzing plus Miri and ordinary tests

A strong unsafe/parser pipeline:

1. fuzz at native speed to discover small failing inputs;
2. minimize the input;
3. convert it to a deterministic test;
4. run that test under Miri if the path contains unsafe code;
5. retain it in nextest and coverage;
6. use mutation testing to ensure the regression assertion is meaningful.

Miri is too slow to serve as the main fuzzer, but it is excellent for deep analysis of selected cases.

## 23.8 Resource governance

Fuzzing is intentionally open-ended. Agents must set or obey:

- time budget;
- memory limit;
- worker count;
- corpus storage policy;
- timeout;
- artifact retention;
- target scope.

Do not start an indefinite fuzz job in an interactive request without an explicit bounded duration or existing repository policy.

## 23.9 Agent decision rule

> Fuzz untrusted-input surfaces continuously or on a schedule, preserve minimized failures, and promote every real finding into an ordinary regression test.

---

# Part IV — Feature compatibility, MSRV, semver, and dependency hygiene

# 24) cargo-hack

## 24.0 Identity

cargo-hack repeatedly invokes Cargo across systematically varied feature sets, package selections, compiler versions, and related configuration dimensions. It turns Cargo's additive feature model into an explicit validation matrix.

Official reference: [cargo-hack][HACK].

## 24.1 Why it is compelling

A normal command such as:

```bash
cargo test --all-features
```

proves only the maximal additive feature union compiles and tests. It does not prove:

- no-default-features works;
- each feature works alone;
- expected feature pairs work;
- optional dependencies are declared correctly;
- feature-gated modules do not rely on another accidental feature;
- a workspace package compiles without development-only dependencies;
- the declared MSRV supports relevant features.

cargo-hack explores these states.

## 24.2 Canonical command patterns

Check feature combinations:

```bash
cargo hack check --workspace --feature-powerset
```

Check each feature individually:

```bash
cargo hack check --workspace --each-feature
```

Check without default features:

```bash
cargo hack check --workspace --no-default-features
```

Exclude known incompatible or redundant combinations using the installed tool's exclusion/grouping options and repository policy.

## 24.3 Feature powersets can explode

For `n` independent features, a full powerset can approach `2^n` combinations. Many combinations are:

- semantically invalid;
- redundant under additive features;
- too expensive;
- equivalent because features imply one another.

Use:

- feature groups;
- mutually exclusive sets;
- depth caps;
- include/exclude lists;
- package scoping;
- changed-crate targeting;
- scheduled full matrices.

Do not run an unbounded powerset reflexively.

## 24.4 `--no-dev-deps` and publish-like validation

Libraries can accidentally rely on dev dependencies or workspace-only conditions. cargo-hack's support for excluding dev-dependency influence can expose publish-time failures that ordinary workspace tests hide.

Pair it with:

```bash
cargo package --no-verify   # inspect package contents; do not use as the only verification
cargo publish --dry-run     # when appropriate and authorized
```

The exact release workflow should remain repository-owned.

## 24.5 Compiler-version matrices

cargo-hack can cooperate with Rust version ranges and the manifest's `rust-version`, allowing the same operation to run across multiple compilers. This complements cargo-msrv:

```text
cargo-msrv:
discover/verify the floor

cargo-hack:
exercise selected feature/package states at that floor and other versions
```

## 24.6 Agent decision rule

> Use cargo-hack whenever features affect code, dependencies, or public API. Optimize the matrix consciously; “all features” is not a substitute for feature compatibility.

---

# 25) cargo-msrv

## 25.0 Identity

cargo-msrv finds and verifies the Minimum Supported Rust Version (MSRV) for a project by testing candidate toolchains through rustup.

Official reference: [cargo-msrv][MSRV].

## 25.1 Why MSRV requires tooling

A manifest can declare:

```toml
[package]
rust-version = "1.xx"
```

but the declaration can become stale when:

- code adopts a newer language/library feature;
- a dependency raises its MSRV;
- build scripts use newer APIs;
- a feature combination has a higher requirement;
- the lockfile resolves versions incompatible with the floor.

cargo-msrv turns the policy into an executable check.

## 25.2 Key modes

Find the minimum version:

```bash
cargo msrv find
```

Verify the declared or selected version:

```bash
cargo msrv verify
```

Inspect the current policy/result:

```bash
cargo msrv show
```

List candidate versions or obtain structured output according to installed help.

Use a custom check command when ordinary `cargo check` is not representative—for example, a library may require a package-specific feature set or test build.

## 25.3 Discovery versus verification

MSRV discovery can be relatively expensive because it evaluates several compiler versions. Run it:

- when establishing a new policy;
- after dependency changes likely to raise MSRV;
- before a major release;
- periodically.

Run verification more frequently:

```bash
cargo msrv verify
```

A failure should lead either to:

- code/dependency changes that restore the declared floor; or
- an intentional policy update and release communication.

Never simply bump `rust-version` to silence CI without understanding what raised it.

## 25.4 Lockfile and dependency resolution complications

For libraries, the oldest compiler may not be able to resolve or build the newest dependency release. A valid MSRV policy may require:

- dependency upper bounds;
- resolver configuration;
- minimal-versions testing where appropriate;
- a committed lockfile for applications;
- lockfile regeneration under a compatible toolchain;
- separate library and workspace policy.

An agent must report whether the failure came from project source, dependency MSRV, Cargo resolution, or toolchain availability.

## 25.5 Integration with cargo-hack

A strong compatibility gate is:

```text
1. cargo msrv verify
2. cargo hack check at MSRV for supported feature states
3. cargo hack check on stable for broad feature states
4. ordinary tests on current stable
```

Do not assume that finding an MSRV with default features proves every advertised feature supports it.

## 25.6 Agent decision rule

> Treat MSRV as a product/API commitment. Verify it under the same feature and package conditions users are promised.

---

# 26) cargo-semver-checks

## 26.0 Identity

cargo-semver-checks compares Rust public API information—derived from rustdoc JSON—between a current crate and a baseline, applying a large set of semver-oriented lints to detect likely breaking changes.

Official reference: [cargo-semver-checks][SEMVER].

## 26.1 Why it is compelling

Rust's type system encodes a large public compatibility surface. Breaking changes can be subtle:

- removing an item;
- narrowing visibility;
- changing generics or trait bounds;
- adding required trait items;
- changing enum variant fields;
- making a type non-exhaustive or changing exhaustiveness behavior;
- modifying implementations that downstream code relies on;
- changing auto-trait behavior;
- altering constness, safety, or ABI-relevant signatures.

Manual diff review is necessary but incomplete. cargo-semver-checks systematically analyzes the API graph.

## 26.2 Baseline selection is the most important decision

The tool can compare against:

- the latest published release;
- a specific version;
- a Git revision;
- another local path/root;
- a chosen baseline branch.

A generic command:

```bash
cargo semver-checks
```

may use a crates.io baseline appropriate for published crates. For an unpublished internal crate, an agent must specify a meaningful baseline instead of accepting a default that cannot represent the release contract.

Record:

```text
current source revision
baseline source/version/revision
feature selection
target/toolchain
lint policy
```

## 26.3 Feature-aware API analysis

Features can add or remove public API. A semver gate must match the crate's compatibility promise:

- default feature API;
- all-features API;
- selected public feature profiles;
- target-specific APIs.

Do not report “semver compatible” without naming the analyzed feature set.

## 26.4 Witnesses and diagnostics

Current versions can produce example downstream code—witnesses—for many violations. These are valuable because they explain how a user could fail to compile. Preserve witnesses in review discussion where they clarify impact.

The tool's exit statuses distinguish detected semver violations from tool/check failures. Agents must not collapse every nonzero exit into “breaking change.”

## 26.5 What it does not cover

cargo-semver-checks focuses on public API compatibility. It does not comprehensively prove:

- runtime behavior is compatible;
- serialization formats are stable;
- error messages remain stable;
- performance remains acceptable;
- feature defaults did not change user behavior;
- database/protocol schemas remain compatible;
- unsafe invariants are preserved.

Combine it with snapshots, integration tests, schema checks, and release notes.

## 26.6 Recommended library workflow

```text
PR changes public crate
    │
    ├─ cargo semver-checks against release baseline
    ├─ snapshot/schema compatibility tests
    ├─ cargo hack across public feature profiles
    └─ changelog/release classification
```

If a breaking change is intentional, do not suppress the finding without documenting the major-version or explicit policy decision.

## 26.7 Agent decision rule

> Run semver checks against an explicit, relevant baseline and report the exact feature surface. A passing API check is not a behavioral compatibility guarantee.

---

# 27) cargo-shear

## 27.0 Identity

cargo-shear performs fast static analysis to identify unused dependencies, dependencies placed in the wrong manifest section, and source files that are not linked into the crate graph.

Official reference: [cargo-shear][SHEAR].

## 27.1 Why it is the recommended primary dependency-hygiene tool

Unused dependencies impose costs:

- compile time;
- binary size in some cases;
- vulnerability and license surface;
- feature unification side effects;
- maintenance burden;
- conceptual noise for agents.

Misplaced dependencies—such as a test-only crate in normal dependencies—can leak into production builds. Unlinked files can represent abandoned code or a missing `mod` declaration.

cargo-shear combines these checks with low execution cost and machine-readable output.

## 27.2 Canonical commands

Analyze:

```bash
cargo shear
```

Fail on warnings in CI:

```bash
cargo shear --deny-warnings
```

Emit structured output where supported:

```bash
cargo shear --json
```

Apply fixes only after review:

```bash
cargo shear --fix
```

## 27.3 Macro and generated-code limitations

Static dependency discovery can miss references hidden behind:

- declarative/procedural macro expansion;
- generated code;
- build scripts;
- platform-specific `cfg`;
- stringly invoked tools;
- examples/benches not included in the selected analysis;
- re-export patterns.

cargo-shear supports a more expensive expansion-assisted mode in current releases. That may require nightly/cargo-expand and should be targeted when a likely false positive involves macros.

## 27.4 Test-target policy

Whether test targets are considered can change findings. A dependency used only in integration tests may belong in `dev-dependencies`, not be deleted. Use the relevant test-target option and inspect all target categories before changing the manifest.

## 27.5 Fix workflow

Safe agent sequence:

1. run `cargo shear --deny-warnings`;
2. classify each finding;
3. inspect `cfg`, macros, build scripts, examples, benches, and tests;
4. remove or move one dependency;
5. run `cargo check --all-targets` and cargo-hack matrix;
6. run tests;
7. inspect `Cargo.lock` change;
8. rerun Shear.

Avoid one large automatic fix followed by broad debugging.

## 27.6 Agent decision rule

> Use cargo-shear as the default dependency-hygiene gate; treat findings as hypotheses until macro, target, and generated-code usage is checked.

---

# 28) cargo-machete

## 28.0 Identity

cargo-machete is a deliberately fast, heuristic unused-dependency detector. It scans repository source and manifests rather than performing a full compiler analysis.

Official reference: [cargo-machete][MACHETE].

## 28.1 Why it remains useful alongside cargo-shear

Machete's value is latency. It can cheaply identify obvious candidates during local work or across very large workspaces. It supports:

- workspace scans;
- CI-friendly exit behavior;
- ignored dependency metadata;
- renamed dependencies;
- structured output in current releases;
- optional Cargo metadata enrichment.

Recommended positioning:

```text
fastest hint             → cargo-machete
primary static policy    → cargo-shear
nightly/compiler check   → cargo-udeps
```

## 28.2 Precision tradeoff

Because Machete is heuristic, false positives can arise from:

- macro use;
- build scripts;
- generated code;
- target-specific source;
- indirect naming patterns;
- non-Rust assets or external tools.

Do not let three unused-dependency tools “vote” without understanding their analysis models.

## 28.3 Metadata mode

An optional metadata-assisted mode can improve accuracy, but invoking Cargo may:

- increase runtime;
- execute more project evaluation;
- create/update lock state depending on the command and repository;
- fail on partial workspaces.

An agent should choose the mode based on trust and fidelity requirements.

## 28.4 Agent decision rule

> Use Machete for low-cost discovery, not as sole authorization to edit `Cargo.toml`.

---

# 29) cargo-udeps

## 29.0 Identity

cargo-udeps uses compiler-produced information to identify unused dependencies. It normally runs on nightly and offers a complementary, more compiler-oriented perspective.

Official reference: [cargo-udeps][UDEPS].

## 29.1 Canonical invocation

```bash
cargo +nightly udeps --workspace --all-targets
```

Add feature flags appropriate to the repository.

## 29.2 Why it is useful as an adjudicator

When Shear or Machete reports a dependency that may be hidden by macros or complex compilation behavior, cargo-udeps can supply an independent compiler-based result.

It is particularly useful:

- before large dependency removals;
- for confusing renamed dependencies;
- after static tools disagree;
- in scheduled hygiene checks.

## 29.3 Limitations

- Nightly is required and behavior can be toolchain-sensitive.
- Doctest usage has limitations.
- Dependencies with multiple versions or identical names can complicate interpretation.
- Some generated/build-script patterns remain difficult.
- Running a compiler-oriented check is slower than source scanning.
- Partial or currently noncompiling workspaces cannot produce complete results.

## 29.4 Agent decision rule

> Use cargo-udeps as a heavier secondary check, not the fastest local gate. A clean result still requires all advertised targets/features to have been represented.

---

# 30) Integrated dependency-hygiene adjudication

## 30.0 Why three tools are useful

The installed stack intentionally contains overlapping tools because they trade latency for fidelity:

| Tool | Analysis style | Strength | Weakness |
|---|---|---|---|
| cargo-machete | heuristic source scan | extremely fast | more false positives/negatives |
| cargo-shear | targeted static workspace analysis | broad, fast, actionable | macro/generated/target caveats |
| cargo-udeps | compiler-oriented/nightly | independent compiler evidence | slower, nightly, target limitations |

## 30.1 Recommended pipeline

```text
local/PR:
cargo machete
cargo shear --deny-warnings

if a finding is disputed or high-impact:
cargo +nightly udeps --workspace --all-targets

before removal:
inspect macros/build.rs/cfg/examples/benches/tests

after removal:
cargo check --workspace --all-targets
cargo hack check <supported feature matrix>
cargo nextest run
cargo test --doc
cargo deny check
cargo audit
```

## 30.2 Ignore policy

When a dependency must remain despite a warning:

- add the narrowest tool-specific ignore;
- state why it is used;
- link to the build script/macro/external invocation;
- add a test that would fail if the dependency integration breaks, where possible;
- revisit ignores periodically.

Do not create a broad global allowlist that defeats the tools.

## 30.3 Agent invariant

> A dependency is removable only after usage analysis, configuration coverage, compilation, tests, and policy scans agree—not merely because one scanner emitted its name.

---

# Part V — Dependency policy, vulnerability intelligence, audit trust, unsafe surface, and binary provenance

# 31) cargo-deny

## 31.0 Identity

cargo-deny is a policy linter for a resolved Rust dependency graph. It evaluates licenses, advisories, banned/duplicated crates, and allowed dependency sources according to repository-owned configuration.

Official reference: [cargo-deny][DENY].

## 31.1 Why it is compelling

A lockfile answers “what was resolved.” It does not answer:

- whether every license is acceptable;
- whether a dependency comes from an approved registry or Git source;
- whether a particular crate/version is banned;
- whether duplicate major versions are tolerated;
- whether a RustSec advisory is allowed temporarily;
- whether unmaintained or yanked dependencies violate policy.

cargo-deny makes those decisions explicit in `deny.toml`.

## 31.2 Check domains

Typical commands:

```bash
cargo deny check
cargo deny check licenses
cargo deny check bans
cargo deny check advisories
cargo deny check sources
```

Use the all-domain command in CI, and focused domains while editing policy.

## 31.3 License policy

cargo-deny parses license expressions and analyzes crate license files. A serious policy should distinguish:

- clearly allowed SPDX licenses;
- denied licenses;
- confidence thresholds or exceptions;
- private/internal crates;
- packages with incomplete metadata;
- exact crate/version exceptions with rationale.

Do not generate a license allowlist from whichever dependencies happen to be present today. Define organizational policy first, then resolve exceptions.

## 31.4 Bans and duplicates

Duplicate versions can:

- increase compile time and binary size;
- duplicate global state or types;
- create interoperability problems;
- retain old vulnerable versions;
- signal incompatible transitive constraints.

Not every duplicate is actionable. The bans configuration can:

- deny a crate entirely;
- deny selected versions;
- skip specific known cases;
- highlight duplicate versions;
- enforce wrapper-crate policy.

Agents should use `cargo tree -d` alongside deny output to trace which packages hold conflicting constraints.

## 31.5 Source policy

Source checking can enforce:

- only crates.io registry dependencies;
- approved private registries;
- approved Git organizations/repositories;
- denied unknown Git sources;
- exact Git revisions instead of moving branches.

A Git dependency pinned to a branch is less reproducible than one pinned to an immutable revision. Policy should reflect that distinction.

## 31.6 Advisories overlap with cargo-audit

cargo-deny can check RustSec advisories, but it is not redundant with cargo-audit:

- cargo-deny integrates advisories into a broader repository policy;
- cargo-audit is a focused vulnerability-scanning workflow with its own reporting and binary integration.

Running both gives policy and focused scanning, but duplicate findings should be de-duplicated in reporting.

## 31.7 Configuration initialization

Start from the tool's generated/default configuration and then edit intentionally:

```bash
cargo deny init
```

Do not copy a random `deny.toml` from another project without understanding its licensing and source assumptions.

## 31.8 Agent decision rule

> Treat cargo-deny as executable dependency governance. Every allow, skip, and exception must be narrow, version-aware where possible, and justified.

---

# 32) cargo-audit

## 32.0 Identity

cargo-audit scans resolved Rust dependencies against the RustSec advisory database. It normally analyzes `Cargo.lock`, and it can also inspect dependency metadata embedded in binaries by cargo-auditable.

Official references: [cargo-audit][AUDIT], [RustSec advisory database][RUSTSEC].

## 32.1 Why it is compelling

cargo-audit provides a focused answer to:

```text
Does the resolved dependency set contain a crate/version associated
with a known RustSec advisory?
```

Canonical repository scan:

```bash
cargo audit
```

Machine-readable output:

```bash
cargo audit --json
```

Binary scan with auditable metadata:

```bash
cargo audit bin path/to/executable
```

## 32.2 Triage model

A finding should produce:

```text
advisory identifier
affected crate/version
dependency path
patched versions
unaffected versions if documented
actual feature/target reachability
runtime exposure
remediation owner
temporary exception expiry
```

Do not assume that an unused code path makes an advisory irrelevant, but do distinguish build-only, target-specific, and runtime exposure when prioritizing.

## 32.3 Exceptions

Temporary ignore mechanisms may be necessary when:

- no patched release exists;
- the affected code is provably not compiled or reachable;
- a replacement is underway;
- the advisory is informational or unmaintained rather than a direct vulnerability.

Every ignore should have:

- advisory ID;
- written rationale;
- expiry/review date;
- tracking issue;
- evidence supporting non-exposure.

## 32.4 Lockfile state

cargo-audit's result reflects the resolved dependency set. If there is no representative lockfile, the scan may not describe the production build. Applications should generally commit `Cargo.lock`; libraries should define how audit resolution is produced in CI.

## 32.5 Untrusted repository caveat

A command that causes Cargo to generate or refresh dependency state can evaluate project-controlled behavior. Run audits of untrusted source in a sandbox. “Security scanner” does not mean “safe to run arbitrary project code on the host.”

## 32.6 Agent decision rule

> Report cargo-audit findings as known-advisory evidence tied to an exact resolved graph. Do not claim it discovers unknown vulnerabilities or replaces dependency review.

---

# 33) cargo-vet

## 33.0 Identity

cargo-vet manages human audit attestations for third-party Rust dependencies. It verifies that dependencies satisfy project-defined criteria through audits performed by the project or imported from trusted entities.

Official reference: [Cargo Vet book][VET].

## 33.1 Why it adds a distinct assurance layer

cargo-audit asks whether a known advisory exists. cargo-vet asks whether trusted reviewers have inspected the dependency against criteria such as:

- safe to run;
- safe to deploy;
- custom organization-specific requirements.

It addresses the “no known advisory” gap: a crate can have no RustSec entry and still be malicious, unsound, or inappropriate.

## 33.2 Initialization and repository state

Initialize:

```bash
cargo vet init
```

This creates a `supply-chain/` policy/audit state. Existing dependencies can be represented through exemptions so adoption can begin without auditing the entire historical graph immediately.

That is a deliberate ratchet:

```text
existing graph → explicit exemptions
new/changed graph → audits required
exemptions → reduced over time
```

Exemptions are deferred work, not proof of review.

## 33.3 Normal update workflow

After a dependency update:

```bash
cargo vet
```

The tool identifies audit gaps and recommends efficient review paths. For a version update:

```bash
cargo vet diff crate old-version new-version
```

For a new crate:

```bash
cargo vet inspect crate version
```

After a human performs the review:

```bash
cargo vet certify
```

The certification command records an attestation. An LLM agent may prepare the diff, summarize risk, and identify unsafe/build/network behavior, but it must not certify that a human audit occurred unless an authorized reviewer actually made that judgment.

## 33.4 Relative audits

A key capability is auditing the delta between two versions instead of re-reading the whole crate. This makes continuous dependency upgrades manageable:

```text
version A fully audited
      +
human review of A → B diff
      =
version B accepted under recorded criteria
```

The validity of the chain depends on trustworthy base audits and complete diffs.

## 33.5 Imported audits and trust

Projects can import audit data from other organizations. Trust should be:

- explicit;
- scoped;
- reviewed;
- criteria-compatible;
- not assumed transitive merely because another project trusts a source.

Direct trust imports and custom criteria should reflect real organizational decisions.

## 33.6 Audit assistance role for LLM agents

An agent can:

- enumerate new dependencies;
- summarize crate purpose and reachable use;
- identify build scripts, proc macros, unsafe code, network/file/process access;
- compare versions;
- highlight obfuscated/generated code;
- map transitive dependency changes;
- draft an audit checklist;
- preserve evidence.

An agent must not:

- fabricate reviewer identity;
- execute `cargo vet certify` as if inspection itself were authorization;
- mark a crate safe solely from popularity or lack of advisories;
- hide exemptions to make CI pass.

## 33.7 Agent decision rule

> cargo-vet records trust-backed review, not automated scanning. Agents may assist the audit; certification remains an explicit accountable act.

---

# 34) cargo-geiger

## 34.0 Identity

cargo-geiger inventories use of Rust's `unsafe` features across a crate and its dependencies. It reports unsafe functions, expressions, trait implementations, and related statistics.

Official reference: [cargo-geiger][GEIGER].

## 34.1 Why it is compelling

Unsafe code is not automatically defective, but it is where Rust's compiler-enforced safety guarantees rely on additional invariants. An unsafe-surface inventory helps:

- prioritize manual review;
- detect new unsafe introduction;
- compare alternative dependencies;
- select Miri/fuzz targets;
- document critical trust boundaries;
- monitor whether a “safe wrapper” adds more unsafe internals over time.

Typical invocation:

```bash
cargo geiger
```

Use workspace/features/target options supported by the installed release to match production.

## 34.2 Interpretation

A count is not a risk score.

```text
one complex unsafe block with an invalid invariant
    may be riskier than
hundreds of carefully audited generated bindings
```

Assess:

- why unsafe is needed;
- whether it is isolated;
- invariant documentation;
- test/fuzz/Miri coverage;
- FFI involvement;
- maintenance quality;
- target-specific paths;
- whether the unsafe code is reachable in the production feature set.

## 34.3 Trend gating

A useful policy is not “zero unsafe everywhere,” but:

```text
no unreviewed increase in first-party unsafe surface
```

When counts change, the agent should show the source locations and review evidence rather than merely update a baseline.

## 34.4 Integration

```text
cargo-geiger identifies unsafe surface
        │
        ├─ Miri explores UB on executable paths
        ├─ cargo-fuzz generates adversarial inputs
        ├─ cargo-vet records third-party review
        └─ cargo-audit/deny assess dependency policy/advisories
```

## 34.5 Agent decision rule

> Use Geiger to locate and trend unsafe code, never to declare a crate safe or unsafe from counts alone.

---

# 35) cargo-auditable

## 35.0 Identity

cargo-auditable embeds a compact dependency tree into compiled Rust binaries so that deployed artifacts can later be associated with the crates and versions that produced them.

Official reference: [cargo-auditable][AUDITABLE].

## 35.1 Why it is compelling

Source repositories and lockfiles are often separated from a production binary. During incident response, operators may have only:

```text
/path/to/deployed/executable
```

cargo-auditable enables:

```bash
cargo audit bin /path/to/deployed/executable
```

to recover dependency metadata and scan it against current advisories.

This is a major operational advantage when:

- binaries are copied outside the build environment;
- containers outlive source branches;
- an advisory appears after release;
- an inventory system must map artifacts to crates;
- multiple build pipelines produce similar executable names.

## 35.2 Canonical build

```bash
cargo auditable build --release
```

It can generally wrap other Cargo build operations. Use the installed help to apply it to workspace packages, examples, or custom profiles.

## 35.3 Metadata is provenance evidence, not complete attestation

Embedded dependency information does not by itself prove:

- source integrity;
- reproducible build identity;
- compiler version;
- build flags;
- environment;
- signing;
- deployed file authenticity;
- runtime-loaded libraries;
- vulnerabilities in non-Rust components.

Combine it with:

- artifact hashes;
- release manifest;
- compiler/toolchain inventory;
- signatures or attestations;
- container SBOM;
- cargo-deny/audit results;
- deployment records.

## 35.4 SBOM-related evolution

Recent Rust/Cargo nightlies have been developing richer build SBOM data. cargo-auditable can participate in workflows that combine its stable embedded metadata model with Cargo's evolving SBOM output. Because nightly flags and schemas can change, pin the nightly and validate artifacts before making them release requirements.

## 35.5 Agent decision rule

> Build release binaries with auditable metadata where supported, then verify that the final shipped artifact—not merely an intermediate—contains the expected dependency inventory.

---

# 36) Integrated supply-chain assurance model

## 36.0 Why all five tools matter

| Question | Tool |
|---|---|
| Are licenses, sources, bans, duplicates, and advisory exceptions compliant? | cargo-deny |
| Does the resolved graph match known RustSec advisories? | cargo-audit |
| Has trusted human review covered third-party code? | cargo-vet |
| Where is unsafe code concentrated? | cargo-geiger |
| Can a deployed binary reveal its Rust dependency tree later? | cargo-auditable |

No tool subsumes the others.

## 36.1 Dependency-change gate

```text
Cargo.toml/Cargo.lock changes
        │
        ├─ cargo shear / machete / udeps
        │      unnecessary dependency?
        │
        ├─ cargo deny check
        │      license/source/ban/advisory policy?
        │
        ├─ cargo audit
        │      known vulnerability?
        │
        ├─ cargo vet
        │      trusted audit coverage?
        │
        ├─ cargo geiger diff
        │      unsafe surface change?
        │
        └─ tests/features/MSRV
               integration compatibility?
```

## 36.2 Release gate

```text
pinned source + lockfile + toolchain
        │
cargo auditable build --release
        │
final artifact hash
        │
cargo audit bin <final artifact>
        │
symbol/size inspection as needed
        │
sign/attest/package/deploy
```

## 36.3 Incident response

When a new advisory is published:

1. search source lockfiles;
2. scan deployed auditable binaries;
3. identify target/feature reachability;
4. map affected services and versions;
5. patch or mitigate;
6. rebuild with updated dependency;
7. rerun deny/audit/vet;
8. verify new artifact metadata;
9. retire affected binaries.

## 36.4 Agent invariant

> “Supply-chain clean” is not a valid unqualified statement. Report policy, advisory, audit, unsafe-surface, and binary-provenance evidence separately.

---

# Part VI — Macro expansion, compiler artifacts, code size, and performance evidence

# 37) cargo-expand

## 37.0 Identity

cargo-expand renders Rust source after macro expansion and derive expansion, generally by wrapping compiler pretty-printing facilities and formatting the result for inspection.

Official reference: [cargo-expand][EXPAND].

## 37.1 Why it is compelling

Macros can generate substantial behavior invisible in the authored source:

- trait implementations;
- serialization/deserialization code;
- builder methods;
- async transformations;
- tracing instrumentation;
- error conversions;
- framework glue;
- repeated schema code.

When an error points into generated code or a dependency appears unused despite a derive macro, expanded output can reveal the actual tokens seen after expansion.

Canonical use:

```bash
cargo expand
cargo expand module::path
cargo expand --package package-name module::path
```

Use the installed help for target, feature, and item filtering.

## 37.2 High-value agent questions

Use cargo-expand to answer:

- What did this derive generate?
- Which trait impl exists after macro expansion?
- Did a macro reference this dependency?
- Why does a method or bound appear in diagnostics?
- Which code is compiled under the selected feature set?
- Did an attribute macro rewrite the function signature/body?

Save targeted output rather than dumping an entire large workspace into context.

## 37.3 Expansion is a debugging representation

Expanded output can be:

- reformatted;
- lossy around hygiene and spans;
- filled with implementation detail;
- different under other features/targets;
- not intended to compile as a standalone source file;
- dependent on unstable compiler pretty-printing.

Do not edit expanded output. Edit the original source or macro definition.

## 37.4 Integration with dependency hygiene

When cargo-shear or cargo-machete flags a dependency used through a macro:

```text
dependency warning
      │
cargo expand targeted module
      │
search expanded tokens/imports/paths
      │
confirm actual macro-generated use
      │
add narrow ignore or improve tool configuration
```

This is a key multi-tool capability enabled by having cargo-expand installed.

## 37.5 Agent decision rule

> Use expanded source as an explanatory lens, not as the canonical program or stable compiler interface.

---

# 38) cargo-show-asm (`cargo asm`)

## 38.0 Identity

cargo-show-asm exposes multiple compiler and binary representations for selected Rust functions, including MIR, LLVM IR, LLVM input, assembly, disassembly, WebAssembly, and—in supported environments—llvm-mca analysis.

Official reference: [cargo-show-asm][SHOW-ASM].

## 38.1 Why it is unusually valuable

A performance or compiler agent often needs to trace:

```text
authored Rust
    │
macro-expanded Rust
    │
MIR
    │
LLVM IR
    │
machine assembly
    │
final linked disassembly
```

cargo-show-asm provides much of that ladder through one interface, usually invoked as:

```bash
cargo asm
```

It can help answer:

- Was a function inlined?
- Did bounds checks remain?
- Was an iterator vectorized?
- Did an abstraction compile away?
- Which branch or allocation remains?
- How did monomorphization specialize a generic?
- Does debug versus release codegen differ?
- What MIR did a compiler-integrated extraction path receive?

## 38.2 Canonical use pattern

List/select functions using the installed help, then request a representation. The exact flags may evolve, so an agent should first run:

```bash
cargo asm --help
```

and identify the fully qualified function. Always specify:

- package;
- binary/library target;
- profile;
- feature set;
- target triple;
- function instantiation where relevant.

Without these, the assembly may not represent production.

## 38.3 Generics, inlining, and missing functions

A requested function may be absent because:

- it was never monomorphized;
- it was inlined into callers;
- dead-code elimination removed it;
- the selected target did not use it;
- multiple generic instantiations exist;
- symbol names differ from source paths.

Use caller inspection, an appropriate benchmark/binary, or temporary non-inlining only as a diagnostic measure. Do not leave codegen-distorting attributes in production solely to make inspection easier.

## 38.4 MIR for compiler/CPG work

The MIR view is particularly valuable for validating assumptions made by a MIR extractor:

- block and terminator shape;
- desugaring;
- drop paths;
- borrow/move representation;
- call operands;
- assertions;
- cleanup blocks;
- optimization-stage differences.

It is a spot-checking tool, not a complete compiler query interface. For systematic extraction use `rustc-dev`/compiler APIs.

## 38.5 Agent decision rule

> Inspect codegen only after selecting the exact production-relevant build context. Never infer whole-program performance from one attractive function body.

---

# 39) cargo-binutils

## 39.0 Identity

cargo-binutils supplies Cargo-aware wrappers around the LLVM tools distributed with `llvm-tools-preview`, including size, symbol, disassembly, and object-copy operations. Commands commonly include `cargo size`, `cargo nm`, and `cargo objdump`.

Official reference: [cargo-binutils][BINUTILS].

## 39.1 Capabilities

Depending on target and installed release:

```bash
cargo size --release
cargo nm --release
cargo objdump --release -- --disassemble
```

The wrapper:

- builds or locates a Cargo artifact;
- selects the matching LLVM utility;
- demangles Rust symbols where supported;
- passes remaining options to the underlying LLVM tool.

High-value uses:

- inspect executable and section sizes;
- list defined/undefined symbols;
- confirm symbol export or stripping;
- disassemble final linked code;
- inspect object-file headers and sections;
- extract or transform sections with objcopy;
- investigate why a native symbol is missing.

## 39.2 Final artifact versus compiler output

cargo-show-asm is convenient for a selected Rust function. cargo-binutils is stronger when the question concerns the **linked artifact**:

```text
source-level function       → cargo-show-asm
linked symbols/sections     → cargo-binutils
whole executable disassembly→ cargo-binutils
```

Use both when inlining, LTO, or linker elimination changes what survives.

## 39.3 CLI stability

Underlying LLVM command-line interfaces are extensive and not Rust-stable APIs. Prefer:

- standard output formats;
- explicit tool versions;
- scripts that validate expected columns/JSON;
- direct report artifacts rather than fragile regex over decorative output.

## 39.4 Security-sensitive use

Symbol inspection can verify:

- debug symbols were stripped when policy requires;
- unintended exports are absent;
- expected auditable/SBOM sections exist;
- cryptographic or FFI symbols resolve from intended objects.

It does not prove those symbols are implemented correctly.

## 39.5 Agent decision rule

> Use cargo-binutils for final artifact facts. Record the artifact path and hash so the inspected binary is demonstrably the one discussed.

---

# 40) cargo-bloat

## 40.0 Identity

cargo-bloat attributes executable code size to crates and functions by analyzing symbols and linked artifacts. It supports common native executable formats and is intended as a practical estimate of what occupies binary code space.

Official reference: [cargo-bloat][BLOAT].

## 40.1 Why it is compelling

Large binaries can indicate:

- duplicate dependency versions;
- generic monomorphization explosion;
- unexpectedly retained features;
- heavy formatting/error machinery;
- duplicated serializers;
- large static tables;
- lack of stripping/LTO;
- broad runtime inclusion.

Canonical use:

```bash
cargo bloat --release
cargo bloat --release --crates
cargo bloat --release -n 50
```

Use package/bin/features/target flags that match the release artifact.

## 40.2 Function and crate views

- **Crate view** is best for broad ownership and dependency choices.
- **Function view** identifies concrete symbols and monomorphizations.
- **Filtering** narrows investigation to a module or crate.

A useful progression:

```text
whole binary grew
    │
cargo bloat --crates
    │
identify contributing crate
    │
cargo bloat function view/filter
    │
cargo nm/objdump/show-asm
    │
confirm mechanism
```

## 40.3 Estimates, not byte-perfect accounting

Symbol-based attribution can be approximate because of:

- inlining;
- shared code;
- linker-generated content;
- stripped symbols;
- generic folding;
- data sections versus text;
- platform format differences.

Use cargo-binutils section sizes and artifact size as complementary evidence.

## 40.4 Not a WebAssembly tool

cargo-bloat's support is oriented toward native object formats and should not be assumed for every target, especially WebAssembly. Select target-specific tooling where required.

## 40.5 Agent decision rule

> Use cargo-bloat to rank code-size contributors, then verify the cause with dependency, symbol, and codegen evidence before changing architecture.

---

# 41) Hyperfine

## 41.0 Identity

Hyperfine is a statistical command-line benchmarking tool. It repeatedly runs commands, measures timing, summarizes distributions, detects outliers, supports warmups and setup commands, and exports results.

Official reference: [Hyperfine][HYPERFINE].

## 41.1 Why it is foundational for performance claims

A one-off `time` invocation is easily distorted by:

- cold caches;
- background load;
- JIT or startup effects in dependencies;
- filesystem state;
- incremental compilation;
- random variance;
- outliers;
- different command setup.

Hyperfine supplies a controlled harness:

```bash
hyperfine \
  --warmup 3 \
  'command-before' \
  'command-after'
```

It can export JSON, CSV, Markdown, or other formats supported by the installed release.

## 41.2 Benchmark design

A valid benchmark records:

- exact command;
- working directory;
- environment variables;
- input dataset and hash;
- build profile;
- target/CPU;
- warmup count;
- run count or duration;
- preparation command;
- cache state policy;
- tool version;
- machine load;
- result distribution, not only mean.

Use `--prepare` for per-run setup only when that setup is intentionally outside the timed region. Use a cleanup command where supported to restore state.

## 41.3 Parameter scans

Hyperfine can sweep a parameter—for example thread count, batch size, or input scale. This is valuable for Rust runtime tuning:

```text
workers = 1, 2, 4, 8, 16
batch size = 1k, 10k, 100k
```

Export results and graph them separately. Do not select the fastest point without considering memory and tail latency.

## 41.4 Compilation benchmarking

When benchmarking compiler/build performance:

- decide whether sccache is enabled;
- decide whether `target` is warm or clean;
- do not accidentally include `cargo build` no-op checks;
- use distinct target dirs if comparing flags;
- record linker and compiler;
- benchmark representative incremental and clean cases separately.

## 41.5 Agent decision rule

> No performance improvement is accepted without an exact before/after benchmark under controlled conditions. Profilers identify where; Hyperfine verifies whether the change helped.

---

# 42) Samply

## 42.0 Identity

Samply is a sampling profiler that records application execution and opens the profile in the Firefox Profiler interface. It supports major desktop platforms and provides call trees, flame charts, timelines, source mapping, and interactive filtering.

Official reference: [Samply][SAMPLY].

## 42.1 Why it is compelling

Compared with a static flamegraph, Samply offers interactive investigation:

- zooming through time;
- call-tree inversion;
- searching functions;
- separating threads;
- inspecting samples by stack;
- source-level mapping;
- correlating phases;
- viewing on-CPU and, on supported platforms, off-CPU behavior;
- comparing startup, steady state, and shutdown.

Canonical pattern:

```bash
cargo build --profile profiling
samply record target/profiling/my-binary <args>
```

A profiling profile should usually inherit release optimization while retaining debug information:

```toml
[profile.profiling]
inherits = "release"
debug = true
```

## 42.2 Linux versus macOS semantics

Sampling backends differ by operating system. The kinds of on/off-CPU information available are not identical. An agent must state the platform and backend and avoid comparing profiles as if they were produced by one uniform kernel interface.

## 42.3 Local data and sharing

Profiles can contain:

- source paths;
- symbol names;
- command-line arguments;
- thread names;
- timing behavior;
- potentially sensitive application metadata.

Samply's normal workflow keeps profile data local unless explicitly shared/uploaded. Treat profile files as potentially sensitive artifacts.

## 42.4 Interpretation

Wide stacks indicate sampled time, not necessarily avoidable waste. A function may be wide because it performs the application's intended work.

Ask:

- Is it CPU work or waiting?
- Is it inclusive or self time?
- Does it scale with input?
- Is it a one-time startup cost?
- Does optimizing it improve end-to-end latency?
- Is the profile representative of production load?

## 42.5 Agent decision rule

> Use Samply for interactive hotspot and phase discovery; preserve the workload and verify changes with an independent benchmark.

---

# 43) cargo-flamegraph / `flamegraph`

## 43.0 Identity

cargo-flamegraph automates sampling and renders folded stack samples as a static flamegraph, typically an SVG. It uses platform-native profiling backends and symbolization.

Official reference: [cargo-flamegraph][FLAMEGRAPH].

## 43.1 Why it remains valuable alongside Samply

Flamegraphs are:

- compact;
- easy to attach to CI or an issue;
- easy to compare visually;
- independent of an interactive UI after generation;
- familiar to performance reviewers.

Canonical use:

```bash
cargo flamegraph --release --bin my-binary -- <args>
```

or profile an existing executable according to installed help.

## 43.2 Build configuration

Useful symbols generally require debug information:

```toml
[profile.release]
debug = true
```

or a separate profiling profile. LTO, inlining, frame pointers, stripping, and platform unwinding can affect stack quality. Do not change production flags permanently without evaluating their cost.

On current Linux Rust toolchains, the cargo-flamegraph project documents an important linker-layout caveat: when using LLD or mold, accurate `perf` stack traces may require passing the linker's `--no-rosegment` option. Apply that setting only to the profiling target/profile and verify it against the installed tool's current guidance; do not copy it into macOS or every release build.

## 43.3 Reading a flamegraph

- horizontal width represents sample proportion;
- vertical position represents call-stack depth;
- x-axis ordering is generally not chronological;
- a wide parent can be wide because of its children;
- absent frames may reflect inlining or symbolization failure;
- short runs can be statistically noisy.

A flamegraph identifies candidates, not proven optimizations.

## 43.4 Samply versus flamegraph

| Need | Preferred tool |
|---|---|
| interactive timeline and filtering | Samply |
| static artifact for review/CI | cargo-flamegraph |
| cross-thread phase exploration | Samply |
| quick conventional hotspot image | cargo-flamegraph |

Use both for critical investigations if their sampling models provide complementary visibility.

## 43.5 Native prerequisite boundary

The Rust command is installed, but the operating-system sampling backend must also exist and be permitted. This reference does not document those native dependencies. An agent should report “profiler backend unavailable” rather than claiming the Rust tool is broken.

## 43.6 Agent decision rule

> Use a flamegraph to select where to investigate; use Hyperfine or application benchmarks to establish whether a change improves the target metric.

---

# 44) Integrated compiler-to-performance investigation ladder

## 44.0 The full ladder

```text
1. Reproduce performance issue
        │
2. Hyperfine/application benchmark establishes baseline
        │
3. Samply or flamegraph identifies hot phase/function
        │
4. cargo-bloat checks code-size/monomorphization context
        │
5. cargo-expand reveals generated code if macros involved
        │
6. cargo-show-asm inspects MIR/LLVM/assembly
        │
7. cargo-binutils confirms final linked symbols/sections/disassembly
        │
8. Make one controlled change
        │
9. Run correctness suite
        │
10. Repeat exact benchmark
        │
11. Re-profile only if mechanism remains unclear
```

## 44.1 Example: unexpected binary and startup growth

```text
artifact size regression
    │
cargo bloat --crates
    │
new serialization crate dominates
    │
cargo tree -d + cargo deny bans
    │
duplicate major versions found
    │
resolve dependency constraints
    │
cargo bloat + cargo size
    │
Hyperfine startup benchmark
```

## 44.2 Example: iterator optimization question

```text
Hyperfine shows hot operation
    │
Samply points to transform()
    │
cargo asm shows bounds checks and missed vectorization
    │
change data layout or loop shape
    │
nextest + coverage + Miri as applicable
    │
Hyperfine verifies change
```

## 44.3 Evidence hierarchy

Do not skip directly from assembly inspection to a performance claim:

```text
assembly difference        = mechanism evidence
sampling profile           = time-attribution evidence
controlled benchmark       = outcome evidence
production telemetry       = real-world evidence
```

All are useful; they answer different questions.

---

# Part VII — Cross-target development and Rust–Python delivery

# 45) Cross (`cross`)

## 45.0 Identity

Cross is a Cargo-compatible command-line tool that performs cross-compilation and, for many targets, cross-execution/testing inside maintained container images. It normally relies on Docker or Podman and target-specific emulation or runners.

Official reference: [Cross][CROSS].

## 45.1 Why it is compelling

Rust can cross-compile many targets, but the complete environment may require:

- target C toolchains;
- system headers and libraries;
- linkers;
- target runners;
- emulation;
- reproducible image setup;
- environment forwarding;
- mounted assets.

Cross packages those concerns behind commands shaped like Cargo:

```bash
cross build --target aarch64-unknown-linux-gnu
cross test --target aarch64-unknown-linux-gnu
cross rustc --target <target> -- <rustc-args>
```

This reduces host-specific setup and gives CI a more consistent target environment.

## 45.2 Cross-compilation versus cross-testing

A successful build proves:

```text
the selected source and dependency graph produced a target artifact
```

A successful `cross test` can additionally prove:

```text
the test binary executed under the configured target runner/emulator
```

The second is stronger, but still not identical to real hardware or a production operating system. QEMU and container environments can differ in:

- CPU features;
- timing;
- kernel interfaces;
- filesystem behavior;
- available devices;
- thread scheduling;
- performance;
- libc/runtime details.

Use native hardware for target-specific critical behavior.

## 45.3 Configuration

Cross supports repository configuration through `Cross.toml` and workspace metadata. Typical concerns include:

- custom images;
- pre-build commands;
- environment passthrough;
- volumes;
- target runners;
- image toolchain additions;
- target-specific package installation.

Keep configuration version-controlled and minimal. A custom image becomes another supply-chain and maintenance artifact.

## 45.4 Integration with nextest

Cross and nextest can be composed, but the exact execution model depends on target support, runners, and archive strategy. A robust target matrix separates:

```text
build target
test execution target
test runner/emulator
feature profile
artifact archive
```

Do not assume an ordinary `cargo nextest run --target ...` can execute a foreign binary without a configured runner.

## 45.5 Container trust and credentials

Cross executes repository builds inside containers and may mount source, target directories, Cargo caches, SSH agents, registries, or credentials.

Agent rules:

- use least-privilege mounts;
- avoid forwarding secrets that are not required;
- pin custom image digests for release workflows;
- treat container images as executable dependencies;
- avoid mounting a privileged container runtime socket into untrusted builds;
- separate caches across trust domains.

## 45.6 Agent decision rule

> Use Cross when reproducible target build/test environments matter. Report whether the result was compile-only, emulated execution, or native execution.

---

# 46) cargo-zigbuild

## 46.0 Identity

cargo-zigbuild integrates Zig as a linker and cross-toolchain layer for Cargo. It is especially useful for producing Linux binaries that target an explicit glibc baseline and for selected cross-platform builds without installing a complete traditional target toolchain.

Official reference: [cargo-zigbuild][ZIGBUILD].

## 46.1 Why it is compelling

A binary compiled on a recent Linux host can accidentally require a newer glibc than the deployment environment. cargo-zigbuild supports target syntax that can express a glibc floor:

```bash
cargo zigbuild --release --target x86_64-unknown-linux-gnu.<glibc-version>
```

It can also cross-link selected Linux and macOS targets and produce macOS universal binaries where supported.

## 46.2 cargo-zigbuild versus Cross

| Cross | cargo-zigbuild |
|---|---|
| containerized build environment | Zig-based linker/toolchain |
| can run tests for many targets | primarily builds/links |
| supplies target image packages | does not automatically supply every target library/header |
| good for CI matrix isolation | good for portable release artifacts |
| emulation/runner support | no general target execution layer |

They can be used together in a larger release strategy, but neither automatically replaces the other.

## 46.3 glibc baseline verification

Specifying a version is not enough. Verify the final artifact using an appropriate compatibility check and, ideally, execute it in the oldest supported runtime environment.

The release record should include:

```text
target triple
requested glibc floor
cargo-zigbuild version
Zig version
rustc version
artifact hash
dynamic dependencies
oldest-environment execution result
```

## 46.4 Native and FFI dependencies

cargo-zigbuild cannot magically cross-compile every native dependency. Crates may require:

- target headers;
- static/dynamic libraries;
- build-script environment variables;
- `pkg-config` equivalents;
- target-specific source builds.

A pure-Rust graph is easiest. For native dependencies, define a reproducible sysroot/library strategy.

## 46.5 Static linking caveats

Static glibc linking and some cross-target combinations have important limitations. Musl, glibc, and macOS targets have different runtime and licensing/compatibility considerations. The agent must follow the target's documented support rather than infer that “Zig” makes every combination valid.

## 46.6 Agent decision rule

> Use cargo-zigbuild to control the linker/sysroot portability problem; verify the resulting artifact in a representative runtime because cargo-zigbuild does not itself test execution compatibility.

---

# 47) Maturin

## 47.0 Identity

Maturin builds, develops, and publishes Python packages backed by Rust through PyO3, CFFI, UniFFI, or standalone Rust binaries. It handles Python wheel metadata, platform tags, extension naming, mixed project layouts, and common manylinux workflows.

Official reference: [Maturin user guide][MATURIN].

## 47.1 Why it is compelling

Rust–Python projects otherwise require coordinated handling of:

- Cargo compilation;
- Python build-system metadata;
- extension module naming;
- Python interpreter/ABI selection;
- wheel tags;
- editable/development installation;
- source distributions;
- manylinux policy;
- mixed Rust/Python package contents;
- publishing.

Maturin makes these one coherent workflow.

## 47.2 Core commands

Create a project:

```bash
maturin new project-name
```

Install a development build into the active Python environment:

```bash
maturin develop
```

Build distributable wheels:

```bash
maturin build --release
```

Build and publish according to explicit authorization and repository policy:

```bash
maturin publish
```

Publishing is consequential and must never be inferred from a request to build or validate.

## 47.3 `develop` versus `build`

```text
maturin develop
    optimized for local iteration
    installs extension into active environment
    mutates that Python environment
    does not serve as final wheel validation

maturin build
    creates wheel artifacts
    supports release profiles/platform policy
    enables installation testing in clean environments
```

A green `maturin develop` does not prove the wheel is correctly packaged.

## 47.4 Binding models

Maturin supports several binding types:

- **PyO3:** idiomatic Rust bindings to CPython, with rich class/function conversion;
- **CFFI:** C ABI surface consumed through Python CFFI;
- **UniFFI:** generated bindings model for supported languages, including Python;
- **binary packages:** package a Rust executable for Python distribution workflows.

Select one intentionally. The packaging tool does not remove the need to design ownership, error conversion, threading, GIL, ABI, and type-stub behavior.

## 47.5 ABI and wheel policy

Important choices include:

- CPython-version-specific extension versus stable ABI/`abi3`;
- supported Python versions;
- platform tags;
- manylinux/musllinux policy;
- architecture matrix;
- universal2 on macOS where relevant;
- stripping and debug symbols;
- external shared libraries;
- reproducible source distribution.

An `abi3` build can reduce the wheel matrix but may restrict available Python APIs. Test every supported Python version even if one wheel serves several versions.

## 47.6 Maturin plus cargo-zigbuild

Maturin can integrate with Zig-based linking for portable Linux wheels in supported configurations. The combined concern is:

```text
Rust target + Python ABI + Linux policy tag + glibc floor + external libraries
```

Do not assume a successful link automatically satisfies auditwheel/manylinux policy. Inspect the built wheel and install it in clean target environments.

## 47.7 Testing architecture

A best-in-class mixed project has separate layers:

```text
Rust unit/property tests
    cargo-nextest

Rust UB checks
    Miri

Rust coverage
    cargo-llvm-cov

Python API tests against development install
    maturin develop + pytest

wheel integrity
    maturin build + clean-environment install + pytest

cross-version matrix
    supported Python versions × target platforms

release dependency provenance
    cargo-auditable where applicable + wheel metadata/native dependency checks
```

Maturin does not run Python tests by itself.

## 47.8 Agent workflow for a Python-facing Rust change

1. run Rust checks and targeted nextest;
2. run Miri/fuzzing for changed unsafe/parser code;
3. create or reuse an isolated Python environment;
4. run `maturin develop`;
5. run Python tests and typing/stub checks;
6. build release wheel;
7. inspect wheel contents/tags;
8. install wheel into a fresh environment;
9. rerun Python tests;
10. retain wheel hash and build inventory.

## 47.9 Agent decision rule

> Use `maturin develop` for iteration and `maturin build` plus clean-environment installation for package evidence. Never equate an import from the development environment with a validated wheel.

---

# 48) Integrated target and packaging workflow

## 48.0 Target taxonomy

An agent must distinguish:

```text
host compiler target
requested Rust target
linker/sysroot environment
test execution environment
production runtime floor
package ABI/tag
physical hardware
```

A single command may cover only two or three.

## 48.1 Recommended release matrix

For each supported target:

```text
1. cargo hack/check relevant feature profile
2. Cross build/test where supported
3. cargo-zigbuild release artifact where portability requires it
4. native or oldest-runtime smoke test
5. cargo-auditable build/scan for Rust executables where applicable
6. cargo bloat/binutils size and symbol policy
7. Maturin wheel build/install tests for Python packages
8. artifact hash, target, toolchain, and dependency inventory
```

## 48.2 Cross-target failure classification

Report one of:

- source compilation failure;
- target std/toolchain missing;
- linker/sysroot failure;
- native dependency cross-build failure;
- target runner/emulator failure;
- test failure under target execution;
- runtime compatibility failure;
- packaging/tagging failure;
- clean-environment installation failure.

“Cross build failed” is too vague for remediation.

## 48.3 Agent invariant

> Compilation, linking, packaging, and execution are separate gates. Report exactly which gates were completed.

---

# Part VIII — Integrated best-in-class workflows for programming agents

# 49) Agent session bootstrap

Before editing a repository, an autonomous or semi-autonomous programming agent should execute a bounded discovery sequence.

## 49.1 Repository command discovery

```bash
pwd
fd -H -d 3 '^(Cargo\.toml|rust-toolchain(\.toml)?|justfile|Justfile|bacon\.toml|deny\.toml|_typos\.toml)$'
just --list
```

If `just` is absent from the repository but installed globally, do not invent recipes. Fall back to documented Cargo commands.

## 49.2 Toolchain and workspace identity

```bash
rustc -vV
cargo -V
rustup show active-toolchain
rustup component list --installed
cargo metadata --format-version 1 --no-deps
```

Determine:

- workspace root;
- packages and default members;
- stable versus nightly pin;
- supported targets;
- declared `rust-version`;
- feature topology;
- generated code/build scripts;
- whether a Python package is present;
- whether the repository owns fuzz, snapshot, vet, deny, or nextest configuration.

## 49.3 Clean baseline

Run the cheapest repository-owned baseline first:

```bash
just ci-fast
```

or, absent a recipe:

```bash
cargo check --workspace --all-targets
cargo nextest run --workspace
cargo test --workspace --doc
```

Do not attribute pre-existing failures to the proposed edit. Capture them.

## 49.4 Risk classification

Classify the requested change:

| Change type | Additional tools triggered |
|---|---|
| documentation/comment only | Typos |
| local safe implementation | nextest, targeted coverage |
| macro/derive behavior | cargo-expand |
| public API | semver-checks, cargo-hack |
| feature/dependency change | hack, Shear/Machete/Udeps, deny/audit/vet |
| unsafe/pointer/concurrency | Miri, fuzz, Geiger |
| parser/protocol/input | fuzz, coverage, snapshots, mutants |
| performance | Hyperfine, Samply/flamegraph, show-asm/bloat |
| cross-target code | Cross, cargo-zigbuild, target-specific tests |
| Python binding | Maturin plus Rust and Python test layers |
| compiler/MIR integration | pinned nightly, rustc-dev, rust-src, snapshots |

The risk class determines the validation tier.

---

# 50) Fast local edit loop

## 50.0 Objective

Provide sub-minute feedback for ordinary edits without running every expensive assurance tool.

## 50.1 Composed stack

```text
rust-analyzer
    semantic completion, references, assists, diagnostics
        │
Bacon
    persistent cargo check/clippy/targeted tests
        │
sccache
    reduces repeated compilation
        │
just
    stable repository command semantics
        │
nextest
    focused test execution
        │
Insta
    reviewed structured-output deltas where applicable
```

## 50.2 Recommended sequence

1. use rust-analyzer to inspect symbol identity and references;
2. make the smallest coherent edit;
3. allow Bacon to complete the default check;
4. run a targeted nextest filter/package;
5. inspect pending snapshot diffs;
6. run `just ci-fast` before moving to another task.

## 50.3 Avoid duplicate background work

A common anti-pattern is:

```text
rust-analyzer check-on-save
+ Bacon cargo check
+ Watchexec cargo check
+ editor task cargo check
```

This can create four overlapping Cargo processes, lock contention, cache churn, and misleading stale outputs.

Recommended ownership:

```text
rust-analyzer → semantic diagnostics
Bacon         → one persistent Cargo job
Watchexec     → server restart or non-Rust pipeline only
```

## 50.4 Freshness requirement

After each edit, the agent must know which diagnostic generation corresponds to the current file state. If a background job was killed/restarted, wait for the completed current generation before reporting success.

---

# 51) Pull-request gate

## 51.0 Objective

Produce fast, deterministic evidence that the proposed change is reviewable and compatible with the repository's normal supported configuration.

## 51.1 Suggested gate

```text
format/check/lint baseline
    │
cargo-nextest workspace/default supported features
    │
cargo test --doc
    │
Typos
    │
cargo-shear
    │
cargo-deny
    │
cargo-audit
    │
targeted cargo-hack matrix
    │
coverage report for changed critical code
    │
snapshot pending-change check
```

## 51.2 Why the order matters

Run cheap structural failures first:

1. compile and lint;
2. ordinary tests;
3. dependency/source policy;
4. broader feature/coverage analysis.

There is little value spending 30 minutes on coverage if the workspace does not compile.

## 51.3 PR evidence packet

Attach or summarize:

- exact commands;
- current and baseline test counts;
- snapshot diffs;
- coverage delta for changed modules;
- dependency graph changes;
- new policy exceptions;
- feature combinations exercised;
- any skipped Tier 3 work and why.

Do not flood review with full logs when a structured summary and artifact links are available.

---

# 52) Full CI and scheduled assurance

## 52.0 CI should be layered

### Fast required PR jobs

- stable compilation/check/lint;
- nextest;
- doctests;
- Typos;
- cargo-shear;
- cargo-deny;
- cargo-audit;
- selected targets/features.

### Slower required or conditional jobs

- cargo-llvm-cov;
- cargo-semver-checks for public crates;
- cargo-msrv verify;
- Cross on primary foreign targets;
- clean-wheel tests for Maturin projects.

### Scheduled or risk-triggered jobs

- full cargo-hack feature powerset;
- Miri seed sweep;
- fuzz campaigns/corpus replay;
- cargo-mutants;
- cargo-udeps;
- cargo-vet backlog;
- Geiger trend;
- binary size and performance baselines;
- broad Cross matrix.

## 52.1 Why scheduled jobs still need ownership

A scheduled job that fails for months is not assurance. Every job should have:

- owner;
- failure routing;
- expected runtime;
- retention policy;
- suppression policy;
- documented response;
- periodic review of whether it still tests a meaningful contract.

## 52.2 Machine-readable artifacts

Prefer:

- JUnit from nextest;
- LCOV/JSON/Cobertura from cargo-llvm-cov;
- structured Typos/Shear/Machete/Audit output;
- mutation reports;
- fuzz artifacts/corpus hashes;
- benchmark JSON from Hyperfine;
- profile files and flamegraph SVG;
- cargo-vet state in version control;
- release artifact hashes and auditable metadata.

An agent should parse structured output where available and retain human-readable summaries.

---

# 53) Feature, MSRV, and API compatibility workflow

## 53.0 The three independent dimensions

```text
feature compatibility  → cargo-hack
compiler floor         → cargo-msrv
public API stability   → cargo-semver-checks
```

A crate can pass any two and fail the third.

## 53.1 Recommended public-library gate

```bash
cargo msrv verify
cargo hack check --workspace --each-feature
cargo hack check --workspace --no-default-features
cargo semver-checks --baseline-rev <release-tag-or-branch>
cargo nextest run --workspace --all-features
cargo test --workspace --doc --all-features
```

Adapt feature combinations to the advertised contract rather than blindly using powerset mode.

## 53.2 Baseline matrix

| Dimension | Baseline |
|---|---|
| compiler | declared `rust-version` |
| API | last compatible published/tagged release |
| features | default, each public optional feature, supported combinations |
| target | each target exposing distinct public `cfg` API |
| behavior | snapshots/protocol tests against release fixtures |

## 53.3 Failure interpretation

- MSRV fails because dependency raised floor → dependency policy/remediation.
- Feature-only build fails → accidental feature coupling.
- Semver check fails → public API change; classify intended/unintended.
- All-features passes but no-default fails → maximal union masked missing declaration.
- API passes but snapshots fail → behavioral/serialization compatibility change.

## 53.4 Agent invariant

> Compatibility is a matrix, not a single compiler run.

---

# 54) Unsafe, FFI, and concurrency assurance campaign

## 54.0 Trigger

Run this workflow when a change touches:

- `unsafe` blocks/functions/traits;
- raw pointers;
- custom allocation;
- atomics or lock-free structures;
- FFI;
- memory-mapped buffers;
- `MaybeUninit`;
- transmute/casts;
- manual `Send`/`Sync`;
- concurrency scheduling or shared state.

## 54.1 Composed workflow

```text
cargo-geiger
    identify changed unsafe surface
        │
ordinary nextest
    functional baseline
        │
cargo-llvm-cov
    executable path coverage
        │
cargo-fuzz
    generate adversarial inputs
        │
Miri
    interpret minimized/high-value paths
        │
cargo-mutants
    check safety assertions and guards matter
        │
Cross/native target tests
    platform behavior
```

## 54.2 Recommended sequence

1. inspect every changed unsafe block and invariant comment;
2. run Geiger before/after to reveal new unsafe surface;
3. add deterministic boundary tests;
4. run nextest and coverage;
5. fuzz public/safe wrapper inputs;
6. minimize failures;
7. run critical tests under Miri with multiple seeds;
8. test relevant targets;
9. use mutants to challenge checks and error branches;
10. preserve regression inputs.

## 54.3 FFI-specific boundary

Miri may not execute native FFI code. Split tests:

```text
pure Rust precondition/postcondition logic → Miri
FFI integration                           → native target test
ABI/symbol/export                         → cargo-binutils
Python FFI package                        → Maturin clean-wheel test
```

## 54.4 Concurrency-specific boundary

Use:

- nextest for ordinary isolated tests;
- `cargo miri test` where races between tests sharing resources matter;
- `cargo miri nextest run` for scalable per-test Miri execution;
- multiple Miri seeds for schedule diversity;
- native stress/load tests for real timing and system behavior.

## 54.5 Agent invariant

> Unsafe review requires invariant reasoning plus dynamic evidence. No tool alone certifies soundness.

---

# 55) Parser, protocol, and semantic transformation campaign

## 55.0 Trigger

Use for:

- parsers;
- decoders;
- code-property-graph extraction;
- query languages;
- schema transformations;
- compiler diagnostics;
- file import/export;
- network messages.

## 55.1 Multi-tool capability

```text
curated corpus
   │
nextest regression tests
   │
Insta snapshots of structured output
   │
llvm-cov identifies unvisited grammar/semantic paths
   │
cargo-fuzz expands adversarial input space
   │
minimized findings become regression tests
   │
cargo-mutants challenges transformation assertions
   │
Miri checks unsafe buffer/path logic
```

## 55.2 Golden/snapshot design

Snapshot semantic units rather than giant whole-program dumps:

- node kinds and stable IDs;
- edges grouped by semantic category;
- normalized diagnostics;
- plan fragments;
- canonical ordering;
- explicit “unknown/incomplete” states.

Redact ephemeral paths/timestamps, not semantic evidence.

## 55.3 Differential testing

Where two implementations exist—such as syntax fallback versus compiler-grade extraction—use the same corpus and compare normalized outputs. Snapshot only the intended difference classes.

Fuzzing can generate inputs for differential comparison:

```text
input
 ├─ implementation A
 └─ implementation B
        │
 normalized semantic diff
        │
 crash/mismatch corpus
```

## 55.4 Agent invariant

> Every fuzz-discovered semantic mismatch that reflects a real bug becomes a deterministic corpus test; the fuzzer is discovery, not long-term regression storage.

---

# 56) Dependency-change workflow

## 56.0 Trigger

Any change to:

- `Cargo.toml`;
- `Cargo.lock`;
- feature definitions;
- patches/replacements;
- Git/registry sources;
- build dependencies;
- proc macros.

## 56.1 Full composed gate

```text
intent and necessity
    │
cargo tree / metadata
    │
Machete + Shear (+ Udeps if needed)
    │
cargo-deny
    │
cargo-audit
    │
cargo-vet
    │
Geiger unsafe-surface comparison
    │
cargo-hack feature matrix
    │
MSRV verify
    │
nextest + doctests
    │
size/build-time check where material
```

## 56.2 Add-dependency checklist

```text
[ ] Why is a new crate preferable to local code or an existing dependency?
[ ] Is it runtime, build, or development-only?
[ ] Are default features necessary?
[ ] Does it change MSRV?
[ ] Does it introduce a proc macro or build script?
[ ] Which licenses and sources apply?
[ ] Any known advisories?
[ ] Is cargo-vet coverage available?
[ ] How much unsafe code is introduced?
[ ] Does it duplicate an existing major version?
[ ] What does it add to compile time or binary size?
[ ] Is target support compatible?
```

## 56.3 Update-dependency checklist

Use cargo-vet relative audit/diff where policy requires. Inspect:

- changelog;
- feature changes;
- MSRV;
- transitive graph;
- build scripts;
- unsafe changes;
- semver breakage despite version number;
- size/performance changes for critical dependencies.

## 56.4 Remove-dependency checklist

Do not remove until:

- Machete/Shear/Udeps evidence is reconciled;
- macros/generated/build scripts are inspected;
- all supported targets/features compile;
- tests/doctests pass;
- lockfile/policy changes are understood.

## 56.5 Agent invariant

> Dependency edits are code changes plus supply-chain changes plus compatibility changes.

---

# 57) Performance regression workflow

## 57.0 Trigger

Use when:

- latency/throughput/memory/code size changes;
- a profiler suggests a hotspot;
- a dependency adds noticeable weight;
- compiler flags or concurrency change;
- agent claims an optimization.

## 57.1 Mandatory baseline

Before editing:

```bash
hyperfine --warmup 3 --export-json target/bench/before.json '<command>'
```

Use a repository-owned benchmark where possible.

## 57.2 Profile selection

- CPU hotspot or phase ambiguity → Samply;
- static review artifact → cargo-flamegraph;
- code size → cargo-bloat and cargo-binutils;
- macro-generated overhead → cargo-expand;
- codegen mechanism → cargo-show-asm;
- compilation latency → Hyperfine with explicit sccache/target policy.

## 57.3 Correctness after optimization

Run:

- nextest;
- doctests;
- coverage for changed paths;
- Miri if unsafe/codegen-sensitive logic changed;
- snapshots;
- feature matrix if generic/feature logic changed.

Optimized wrong code is not an improvement.

## 57.4 Outcome verification

Repeat the exact benchmark:

```bash
hyperfine \
  --warmup 3 \
  --export-json target/bench/compare.json \
  '<before-command-or-binary>' \
  '<after-command-or-binary>'
```

When comparing commits, use separate worktrees/build outputs or reproducible artifact paths. Ensure sccache and filesystem cache policies are equivalent.

## 57.5 Report

```text
metric and workload
before distribution
after distribution
relative change
confidence/variance
CPU and environment
correctness checks
code-size/memory tradeoff
profile evidence explaining mechanism
```

## 57.6 Agent invariant

> A smaller assembly listing, narrower flame frame, or theoretical complexity improvement is not a measured performance result.

---

# 58) Cross-target release workflow

## 58.0 Release stages

```text
source/lock/toolchain pin
    │
host tests and compatibility
    │
Cross build/test matrix
    │
cargo-zigbuild portability artifacts where needed
    │
native/oldest-runtime smoke tests
    │
cargo-auditable release build
    │
cargo audit bin
    │
binutils/bloat symbol and size policy
    │
artifact hash and package/sign/attest
```

## 58.1 Target status language

Use precise labels:

- `compiled`;
- `linked`;
- `emulator-tested`;
- `native-tested`;
- `oldest-runtime-tested`;
- `packaged`;
- `installed from package`;
- `auditable metadata verified`.

Do not use a single green “supported” label when only compilation occurred.

## 58.2 Release reproducibility inventory

Record:

```text
source commit
Cargo.lock hash
rustc -vV
active toolchain
Cargo tool versions
target triple
features/profile
Cross image digest or cargo-zigbuild/Zig versions
artifact hash
auditable dependency inventory
test execution environment
```

---

# 59) Rust–Python package workflow

## 59.0 Development loop

```text
Rust edit
  → rust-analyzer/Bacon/sccache
  → cargo nextest
  → maturin develop
  → Python tests
```

## 59.1 Release loop

```text
Rust checks + Miri/fuzz where needed
  → supported Python version matrix
  → maturin build --release
  → inspect wheel names/tags/contents
  → install into fresh environment
  → run Python tests
  → cross-platform wheel matrix
  → hash and retain artifacts
```

## 59.2 Coverage across the language boundary

To measure Rust coverage from Python-driven calls:

1. build an instrumented extension using the environment emitted by cargo-llvm-cov;
2. install/load that exact extension;
3. run Python tests;
4. merge profile data;
5. render the Rust report.

Because exact environment commands can change, use:

```bash
cargo llvm-cov show-env --help
```

and the installed tool's external-test instructions. Do not assume `maturin develop` automatically inherits the correct coverage environment unless explicitly configured.

## 59.3 ABI and concurrency checks

For PyO3 or other bindings, test:

- supported Python versions;
- exception conversion;
- ownership/lifetime edges;
- GIL release/acquisition behavior;
- thread callbacks;
- buffer protocol/zero-copy contracts;
- interpreter shutdown;
- subinterpreters where promised;
- stable ABI claims;
- wheel external libraries.

## 59.4 Agent invariant

> The Rust library, Python API, and wheel artifact are three different test subjects.

---

# 60) Compiler-internals and CPG workflow

## 60.0 Environment contract

```text
date-pinned nightly
+ rustc-dev
+ rust-src
+ llvm-tools-preview
+ lockfile
+ semantic golden corpus
```

## 60.1 Extraction development loop

```text
rust-analyzer on pinned toolchain
    │
Bacon cargo check
    │
targeted compiler-fixture nextest
    │
Insta snapshots of HIR/MIR/graph facts
    │
cargo-show-asm MIR spot checks
    │
coverage of extractor branches
```

## 60.2 Upgrade validation

When changing nightly:

```text
compile tool
    │
run old fixture corpus
    │
snapshot semantic diff
    │
classify compiler API versus semantic changes
    │
run mutants on normalization/identity logic
    │
profile extraction throughput
    │
update reference/toolchain pin
```

## 60.3 Partial-compilation behavior

A compiler-integrated indexer must not silently serve stale “complete” semantic facts after a source edit that no longer compiles.

Tests should snapshot and assert:

- which files/items were extracted;
- which compiler phase failed;
- diagnostics;
- freshness/version of facts;
- syntax fallback availability;
- query response completeness markers;
- invalidation of facts derived from changed definitions.

Miri and fuzzing can target unsafe/custom parser layers, but compiler incompleteness is a lifecycle semantics problem verified primarily through fixtures and snapshots.

## 60.4 Agent invariant

> For compiler-integrated systems, semantic output is the product. Upgrade tests must compare facts and completeness states, not merely whether the extractor compiles.

---

# 61) Test-quality triangulation

## 61.0 Four independent axes

```text
nextest      → do ordinary tests pass reliably?
llvm-cov     → which regions executed?
mutants      → would plausible semantic faults be caught?
fuzz         → what unexpected inputs reach new behavior?
Insta        → did complex structured output change?
Miri         → did selected executions violate Rust validity/aliasing/concurrency rules?
```

## 61.1 Diagnostic matrix

| Observation | Likely action |
|---|---|
| low coverage, no mutants evaluated | add reachability tests |
| high coverage, many survivors | strengthen assertions |
| fuzz crash, no deterministic test | minimize and promote to regression |
| snapshot churn every run | normalize true nondeterminism |
| Miri failure only under one seed | preserve seed and isolate schedule-sensitive bug |
| nextest flaky retry passes | fail-on-flaky and investigate shared state |
| tests pass but semver fails | public API compatibility issue |
| tests pass but cargo-deny fails | governance issue, not functional correctness |
| coverage falls after dependency removal | inspect feature/target test scope |

## 61.2 Quality claim language

Prefer:

```text
“Default and all-feature suites passed under nextest; doctests passed separately.
Line coverage for crate X was Y under the documented exclusions.
No surviving mutants remained in changed module Z after equivalent-mutant triage.
Miri passed 16 seeds for the unsafe test subset.”
```

Avoid:

```text
“Fully tested.”
“100% safe.”
“No bugs.”
```

---

# Part IX — Repository configuration templates

The following templates are starting points. They must be adapted to the repository, supported operating systems, installed tool versions, and risk policy. An agent should validate each file with the owning tool before committing it.

# 62) `rust-toolchain.toml` patterns

## 62.1 Stable-first application

A normal application that only uses nightly for optional checks may omit a repository pin entirely or pin a stable release intentionally:

```toml
[toolchain]
channel = "stable"
profile = "default"
components = [
  "rustfmt",
  "clippy",
  "rust-analyzer",
  "rust-src",
  "llvm-tools-preview",
]
```

Using the symbolic `stable` channel allows upgrades over time. For bit-for-bit compiler reproducibility, replace it with an exact stable version and establish an upgrade cadence.

## 62.2 Compiler-internals repository

```toml
[toolchain]
channel = "nightly-YYYY-MM-DD"
profile = "minimal"
components = [
  "rustfmt",
  "clippy",
  "rust-analyzer",
  "rust-src",
  "rustc-dev",
  "llvm-tools-preview",
  "miri",
]
```

Use one exact date across development and CI.

## 62.3 Stable application with targeted nightly

Often the best policy is:

- no nightly in repository default;
- CI installs an exact nightly only for Miri/Udeps;
- recipes use `cargo +nightly-YYYY-MM-DD ...`.

This prevents compiler-private checks from silently changing the normal build compiler.

---

# 63) `.cargo/config.toml`

A minimal cache-aware configuration:

```toml
[build]
rustc-wrapper = "sccache"
```

A profiling profile belongs in `Cargo.toml`, not `.cargo/config.toml`:

```toml
[profile.profiling]
inherits = "release"
debug = true
```

## 63.1 What not to hard-code globally

Avoid placing host-specific linker, CPU, or target flags into a globally shared Cargo config unless every supported environment is compatible:

```text
-C target-cpu=native
host-specific linker path
one operating system's runner
one user's absolute sccache path
release-only experimental flags
```

Use target-specific tables, environment variables, CI configuration, or repository scripts.

## 63.2 sccache observability recipe

```just
cache-stats:
    sccache --show-stats

cache-reset-stats:
    sccache --zero-stats
```

A reset mutates statistics, not compilation artifacts, but should still be intentional during measurement.

---

# 64) `justfile` reference implementation

This example defines a broad agent-facing contract. It intentionally separates fast, PR, scheduled, mutating, and target-specific operations.

```just
set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

# ---------- discovery ----------

versions:
    rustc -vV
    cargo -V
    rustup show active-toolchain
    rustup component list --installed
    cargo install --list

metadata:
    cargo metadata --format-version 1 --no-deps

# ---------- fast local loop ----------

check:
    cargo check --workspace --all-targets

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo nextest run --workspace

test-package package:
    cargo nextest run -p {{package}}

doctest:
    cargo test --workspace --doc

typos:
    typos

deps-fast:
    cargo machete
    cargo shear --deny-warnings

ci-fast: check clippy test doctest typos deps-fast

# ---------- pull-request assurance ----------

test-all-features:
    cargo nextest run --workspace --all-features -P ci

doctest-all-features:
    cargo test --workspace --doc --all-features

coverage:
    mkdir -p target/coverage
    cargo llvm-cov nextest \
      --workspace \
      --all-features \
      --lcov \
      --output-path target/coverage/lcov.info

policy:
    cargo deny check
    cargo audit

snapshots-pending:
    cargo insta pending-snapshots

ci-pr: ci-fast test-all-features doctest-all-features policy snapshots-pending

# ---------- compatibility ----------

features-each:
    cargo hack check --workspace --each-feature

features-no-default:
    cargo hack check --workspace --no-default-features

features-powerset:
    cargo hack check --workspace --feature-powerset

msrv:
    cargo msrv verify

semver baseline:
    cargo semver-checks --baseline-rev {{baseline}}

# ---------- deep correctness ----------

miri package:
    cargo +nightly miri test -p {{package}}

miri-seeds package seeds="16":
    MIRIFLAGS="-Zmiri-many-seeds=0..{{seeds}}" cargo +nightly miri test -p {{package}}

udeps:
    cargo +nightly udeps --workspace --all-targets

mutants-file path:
    cargo mutants -f {{path}}

fuzz target seconds="60":
    cargo fuzz run {{target}} -- -max_total_time={{seconds}}

fuzz-coverage target:
    cargo fuzz coverage {{target}}

# ---------- supply chain ----------

vet:
    cargo vet

unsafe-surface:
    cargo geiger

auditable-build package:
    cargo auditable build --release -p {{package}}

audit-binary path:
    cargo audit bin {{path}}

# ---------- artifact inspection ----------

bloat bin:
    cargo bloat --release --bin {{bin}} --crates

symbols bin:
    cargo nm --release --bin {{bin}}

sections bin:
    cargo size --release --bin {{bin}}

asm:
    cargo asm

# ---------- mutating operations: invoke deliberately ----------

snapshots-review:
    cargo insta review

# Do not make acceptance a dependency of any CI recipe.
snapshots-accept:
    cargo insta accept

deps-fix:
    cargo shear --fix

typos-write:
    typos -w

# ---------- environment maintenance ----------

tool-updates-check:
    cargo install-update --list
```

## 64.1 Template caveats

- `cargo install-update --list` behavior/options should be validated against the installed cargo-update version; use its help if that mode differs.
- `--baseline-rev` requires a meaningful baseline revision.
- `cargo geiger` may need workspace/feature options.
- nextest profile `ci` must exist.
- Bash shell selection is not portable to all Windows environments.
- publication commands are deliberately absent.
- source-mutating recipes are never dependencies of validation recipes.

## 64.2 Recommended recipe annotations

Use Just's supported documentation/group/confirmation attributes in the installed release to make:

- cost tier visible;
- destructive recipes confirm;
- release tasks grouped;
- parameters discoverable.

The `justfile` is an agent interface, so naming clarity is more important than minimizing lines.

---

# 65) `.config/nextest.toml`

A practical baseline:

```toml
[profile.default]
fail-fast = true
slow-timeout = { period = "60s", terminate-after = 2 }

[profile.ci]
fail-fast = false
retries = { backoff = "exponential", count = 2, delay = "1s", max-delay = "10s", jitter = true }
flaky-result = "fail"
slow-timeout = { period = "60s", terminate-after = 3 }
global-timeout = "2h"
success-output = "never"
failure-output = "immediate-final"

[profile.ci.junit]
path = "junit.xml"

# Example: serialize tests using a scarce shared resource.
[test-groups.database]
max-threads = 1

[[profile.default.overrides]]
filter = 'test(database_)'
test-group = 'database'

# Example: allow a longer timeout for end-to-end tests.
[[profile.default.overrides]]
filter = 'test(e2e_)'
slow-timeout = { period = "120s", terminate-after = 5 }
```

## 65.1 Policy cautions

- `retries` plus `flaky-result = "fail"` is recommended when retries are diagnostic.
- Do not put genuinely flaky tests into a permanent pass-on-flaky bucket.
- Test-name filters are conventions; package/binary/tag expressions may be more robust.
- Shared-resource tests should use explicit groups, not sleep-based serialization.
- Validate locally:

```bash
cargo nextest help repo-config
cargo nextest run -P ci
```

## 65.2 Doctest companion

Every nextest-based CI definition needs:

```bash
cargo test --workspace --doc
```

or the relevant features/packages.

---

# 66) `bacon.toml`

Initialize from the installed default:

```bash
bacon --init
```

Then add only repository-specific jobs:

```toml
default_job = "check-all"

[jobs.check-all]
command = ["cargo", "check", "--workspace", "--all-targets"]

[jobs.nextest-workspace]
command = ["cargo", "nextest", "run", "--workspace"]
need_stdout = true
analyzer = "nextest"

[jobs.miri-critical]
command = ["cargo", "+nightly", "miri", "test", "-p", "critical-crate"]
need_stdout = true
```

Bacon's standard analyzer understands normal Cargo and Miri output; nextest uses the dedicated `nextest` analyzer.

## 66.1 Machine-readable locations

A global or project preference can export current diagnostics:

```toml
[exports.locations]
auto = true
path = ".bacon-locations"
line_format = "{kind} {path}:{line}:{column} {message}"
```

Add `.bacon-locations` to ignore rules unless the repository intentionally versions a diagnostic artifact.

## 66.2 Agent use

An agent consuming `.bacon-locations` must:

- verify the file timestamp/generation corresponds to current source;
- process errors before warnings;
- rerun after edits;
- not treat an empty file during a running job as success.

---

# 67) `deny.toml`

Generate a starting file:

```bash
cargo deny init
```

Then define organizational policy. A generic document cannot safely choose licenses or registries for the project.

Required policy decisions include:

```text
licenses:
  approved SPDX expressions
  denied licenses
  confidence handling
  exceptions

bans:
  duplicate-version policy
  denied crates/versions
  wrapper policies
  exact skips with rationale

advisories:
  vulnerability handling
  unmaintained/yanked policy
  temporary ignores and expiry

sources:
  approved registries
  approved Git repositories/organizations
  revision requirements
  unknown-source policy
```

## 67.1 Exception metadata

Where the file format permits notes or context, include:

```text
reason
owner
tracking issue
date added
expiry/review date
replacement plan
```

If the format does not hold all metadata, link to a repository issue or policy document.

---

# 68) `_typos.toml`

A minimal structure:

```toml
[files]
extend-exclude = [
  "target/",
  "vendor/",
  "fuzz/corpus/",
]

[default.extend-words]
# Domain terms go here, narrowly.
```

Do not exclude snapshots by default; spelling changes in diagnostics or generated schema snapshots may be meaningful. Exclude binary corpus inputs and generated/vendor trees where human spelling review is not useful.

---

# 69) cargo-vet repository state

Initialize:

```bash
cargo vet init
```

Expected version-controlled state:

```text
supply-chain/
├── config.toml
├── audits.toml
├── exemptions.toml
└── imports.lock or related generated state
```

Exact files can evolve with cargo-vet. Commit the policy and audit data required by the installed version.

## 69.1 Review workflow

```bash
cargo vet
cargo vet diff <crate> <from> <to>
cargo vet inspect <crate> <version>
# human review
cargo vet certify
```

The final command is an accountable mutation. Agents should stop before certification unless explicitly authorized and the reviewer identity/criteria are valid.

---

# 70) Profiling build profile

Use a dedicated profile:

```toml
[profile.profiling]
inherits = "release"
debug = true
strip = false
```

Depending on platform and profiler, frame pointers or other flags may improve stack quality. Add them narrowly and benchmark their effect. Do not silently alter the shipped release profile.

Build and profile:

```bash
cargo build --profile profiling --bin my-binary
samply record target/profiling/my-binary <args>
```

or configure cargo-flamegraph to use the intended profile according to its installed help.

---

# 71) Tool inventory artifact

A simple environment capture script can become a CI artifact:

```bash
#!/usr/bin/env bash
set -euo pipefail

{
  date -u
  uname -a || true
  rustc -vV
  cargo -V
  rustup show active-toolchain
  rustup component list --installed
  cargo install --list
  sccache --show-stats || true
} > target/tooling-inventory.txt
```

On macOS, `uname` still works; additional OS-specific hardware/compiler details may be added. Do not include secrets or full environment dumps.

---

# Part X — Failure modes, anti-patterns, and final agent controls

# 72) Cross-tool anti-pattern compendium

## 72.1 Installing tools without creating a command policy

**Anti-pattern:** dozens of globally installed executables, but no `justfile`, CI pins, cadence, or ownership.

**Consequence:** agents choose inconsistent flags and produce incomparable results.

**Correction:** expose repository-owned intent recipes and document required versus optional tools.

## 72.2 Updating every tool during active debugging

**Anti-pattern:**

```bash
rustup update
cargo install-update -a
```

immediately before or during diagnosis.

**Consequence:** compiler and CLI behavior change at the same time as source, destroying the controlled experiment.

**Correction:** reproduce on the existing inventory; upgrade in a separate change.

## 72.3 Making nightly the default everywhere

**Anti-pattern:** use rolling nightly for a normal application solely because Miri or Udeps needs nightly.

**Consequence:** unrelated compiler drift, larger environment, unstable behavior.

**Correction:** stable-first build plus date-pinned targeted nightly commands.

## 72.4 Parsing decorative terminal output when structured output exists

**Anti-pattern:** regex over colored, human-oriented tables.

**Consequence:** minor tool upgrades break automation.

**Correction:** use JSON, LCOV, JUnit, machine-readable reports, or stable artifact formats.

## 72.5 Suppressing failures to obtain a green dashboard

Examples:

- adding broad cargo-deny skips;
- allowing all flaky nextest retries to pass;
- excluding low-coverage production modules;
- ignoring every surviving mutant;
- adding every Typos finding to the dictionary;
- exempting all cargo-vet dependencies indefinitely;
- disabling Miri isolation/checks;
- ignoring unsupported feature combinations rather than defining policy.

**Correction:** every suppression is narrow, justified, owned, and reviewable.

## 72.6 Auto-mutating source as “cleanup”

**Anti-pattern:**

```bash
cargo shear --fix
typos -w
cargo insta accept
```

followed by no semantic review.

**Correction:** analyze first, mutate one class at a time, inspect diff, rerun relevant gates.

## 72.7 Treating nextest as complete Rust testing

**Anti-pattern:** “all tests passed” after only `cargo nextest run`.

**Correction:** run doctests separately and account for build scripts/examples/benches as promised.

## 72.8 Retry laundering

**Anti-pattern:** retries turn nondeterministic failures into a green build.

**Correction:** configure `flaky-result = "fail"` or equivalent policy and route flakes to owners.

## 72.9 Coverage worship

**Anti-pattern:** optimize for a percentage, add assertion-free tests, exclude difficult code.

**Correction:** pair coverage with mutation testing, snapshots, and risk-based assertions.

## 72.10 Mutation-score gaming

**Anti-pattern:** mark survivors ignored merely to raise a score.

**Correction:** classify equivalent mutants, missing tests, unreachable code, and tool limitations with evidence.

## 72.11 Unbounded fuzzing in an interactive task

**Anti-pattern:** start an indefinite `cargo fuzz run` and promise later results.

**Correction:** use an explicit time/resource budget, return discovered artifacts now, and rely on scheduled infrastructure for continuous runs.

## 72.12 Claiming Miri proves soundness

**Anti-pattern:** one Miri run becomes “unsafe code is safe.”

**Correction:** state target, tests, seeds, unsupported operations, and unexecuted paths; combine with reasoning and fuzzing.

## 72.13 Letting an agent self-certify cargo-vet

**Anti-pattern:** the same agent scans a diff and records a human audit attestation without authorization.

**Correction:** agents assist; accountable reviewers certify.

## 72.14 Interpreting Geiger as a vulnerability scanner

**Anti-pattern:** fewer unsafe blocks means safer; more means vulnerable.

**Correction:** use counts for review targeting and trend, then inspect invariants and evidence.

## 72.15 Removing dependencies based on one scanner

**Anti-pattern:** Machete/Shear/Udeps emits a name, so the manifest is edited.

**Correction:** reconcile analysis models and verify macros, targets, features, tests, and build scripts.

## 72.16 Auditing the wrong dependency graph

**Anti-pattern:** scan an absent/stale lockfile or different feature/target resolution and generalize to production.

**Correction:** identify the exact resolved graph and scan final auditable artifacts.

## 72.17 Cross-compilation masquerading as target support

**Anti-pattern:** a foreign target linked, therefore it is “tested.”

**Correction:** distinguish compile, link, emulated test, native test, and oldest-runtime test.

## 72.18 cargo-zigbuild as universal sysroot magic

**Anti-pattern:** assume every native dependency and target library is supplied automatically.

**Correction:** inventory headers/libraries/build scripts and test final runtime compatibility.

## 72.19 `maturin develop` as release validation

**Anti-pattern:** extension imports locally, therefore the wheel is valid.

**Correction:** build a wheel, inspect it, install it in a clean environment, and rerun Python tests.

## 72.20 Profiling without a controlled benchmark

**Anti-pattern:** optimize the widest flame frame and claim success.

**Correction:** establish and repeat an end-to-end benchmark.

## 72.21 Inspecting the wrong codegen

**Anti-pattern:** use debug/default-feature host assembly to explain release/all-feature target performance.

**Correction:** record package, target, profile, features, function instantiation, and artifact.

## 72.22 Double- or triple-watching the same command

**Anti-pattern:** rust-analyzer, Bacon, and Watchexec all launch full workspace checks.

**Correction:** assign one owner to each continuous job.

## 72.23 Treating sccache as transparent without measurement

**Anti-pattern:** enable wrapper, ignore zero hit rate or remote errors.

**Correction:** inspect statistics, target/profile fragmentation, and cache policy.

## 72.24 Running “safe” Cargo tools on untrusted source

**Anti-pattern:** execute checks/audits directly on a hostile repository.

**Correction:** sandbox builds, restrict credentials/network/mounts, and remember build scripts/proc macros are executable code.

---

# 73) Failure classification vocabulary

Agents should map failures to one of these categories:

| Category | Examples |
|---|---|
| environment/tool missing | executable absent, rustup component unavailable |
| version incompatibility | CLI flag absent, nightly mismatch, compiler-private API drift |
| configuration error | invalid nextest/bacon/deny/Typos configuration |
| source compilation | rustc error in project |
| build-script/proc-macro | generated code or build dependency failure |
| test assertion | deterministic functional failure |
| flaky/nondeterministic | passes on retry or seed-dependent |
| timeout/resource | exceeded test/fuzz/mutation/global budget |
| UB/interpreter | Miri validity/aliasing/race finding |
| unsupported analysis | Miri FFI operation, profiler backend unavailable |
| coverage gap | intended region not executed |
| mutation survivor | behavior insufficiently constrained |
| fuzz finding | crash, panic, timeout, semantic mismatch |
| compatibility | feature, MSRV, semver, target |
| dependency hygiene | unused/misplaced/unlinked source |
| policy | license/source/ban/advisory violation |
| audit trust | cargo-vet coverage gap |
| unsafe-surface change | Geiger delta requiring review |
| artifact | symbol, section, size, auditable metadata |
| packaging | wheel tag/content/install failure |
| performance | statistically significant regression |
| infrastructure | container, runner, cache, credential, filesystem |

This classification prevents an agent from changing production code to compensate for an environment failure or suppressing a policy issue as if it were a unit-test problem.

---

# 74) Final LLM-agent invariants

## 74.0 Invariant 1 — discover repository intent first

Run `just --list`, inspect configuration, and use repository-owned commands before inventing a workflow.

## 74.1 Invariant 2 — identify the active configuration

Every result is scoped by toolchain, target, features, package set, profile, and environment.

## 74.2 Invariant 3 — stable and nightly have different jobs

Use stable for normal engineering unless the project itself requires nightly. Pin nightly for Miri, Udeps, and compiler internals.

## 74.3 Invariant 4 — semantic feedback is not authoritative compilation

rust-analyzer guides editing; Cargo/rustc validates the actual build.

## 74.4 Invariant 5 — doctests remain separate from nextest

Never report complete test success without the doctest step when applicable.

## 74.5 Invariant 6 — retries reveal flakes

A test that passes only on retry is not a clean pass.

## 74.6 Invariant 7 — coverage measures execution

It does not measure assertion quality.

## 74.7 Invariant 8 — mutation testing measures sensitivity

Survivors require triage; they are not automatically defects or ignorable noise.

## 74.8 Invariant 9 — fuzz findings become deterministic tests

Preserve minimized inputs and regression coverage.

## 74.9 Invariant 10 — Miri explores, it does not prove

Record targets, seeds, and unsupported operations.

## 74.10 Invariant 11 — feature support is a matrix

`--all-features` does not prove each feature or no-default configuration works.

## 74.11 Invariant 12 — MSRV is a public commitment

Verify the declared floor with representative features and dependencies.

## 74.12 Invariant 13 — semver requires an explicit baseline

Name the release/revision and feature surface.

## 74.13 Invariant 14 — dependency scanners produce hypotheses

Inspect macro, build-script, generated, test, and target usage before removal.

## 74.14 Invariant 15 — dependency assurance has several layers

Policy, known advisories, trusted audits, unsafe surface, and binary provenance remain distinct.

## 74.15 Invariant 16 — agents do not fabricate audit authority

cargo-vet certification requires accountable authorization.

## 74.16 Invariant 17 — a binary is the final artifact of record

Inspect and hash the actual shipped artifact, not an intermediate with the same name.

## 74.17 Invariant 18 — codegen evidence must match production context

Profile, features, target, linker, LTO, and monomorphization matter.

## 74.18 Invariant 19 — profiling locates; benchmarking verifies

No performance claim without controlled before/after outcome evidence.

## 74.19 Invariant 20 — cross-build is not cross-runtime validation

State exactly which target gates completed.

## 74.20 Invariant 21 — development install is not package validation

Maturin wheels require clean-environment installation tests.

## 74.21 Invariant 22 — source mutation is explicit

Do not run `--fix`, `-w`, snapshot acceptance, lockfile update, certification, publishing, or global upgrades as hidden substeps.

## 74.22 Invariant 23 — expensive tools are risk-triggered

Use cost tiers; do not create a permanently ignored assurance suite through overuse.

## 74.23 Invariant 24 — output freshness matters

Background results must correspond to the current source generation.

## 74.24 Invariant 25 — untrusted builds are executable

Sandbox repositories and restrict credentials, network, mounts, and container privileges.

## 74.25 Invariant 26 — failures are evidence

Classify the failure before editing code or policy.

## 74.26 Invariant 27 — preserve reproducibility

Record exact commands, versions, environment, output artifacts, and exclusions.

## 74.27 Invariant 28 — semantic output needs semantic regression tests

For compilers, parsers, and CPGs, compare normalized facts and completeness states, not only process exit codes.

---

# 75) Final implementation checklist

## 75.1 Environment

```text
[ ] Stable toolchain available.
[ ] Exact nightly pinned wherever compiler-private behavior matters.
[ ] rust-analyzer, rust-src, llvm-tools-preview installed on required toolchains.
[ ] rustc-dev and Miri installed on the pinned nightly where needed.
[ ] Cargo tool inventory captured.
[ ] sccache configured and hit rate observed.
[ ] Native/container/Zig prerequisites documented separately.
```

## 75.2 Repository command contract

```text
[ ] justfile exists or equivalent commands are documented.
[ ] Fast, PR, scheduled, release, and mutating tasks are separated.
[ ] Dangerous recipes require explicit invocation/confirmation.
[ ] CI pins third-party tool versions.
[ ] Tool output paths are deterministic.
```

## 75.3 Continuous feedback

```text
[ ] rust-analyzer models intended features/target.
[ ] Build scripts and proc macros are intentionally enabled/disabled.
[ ] Bacon owns the Rust background check.
[ ] Watchexec does not duplicate the same expensive job.
[ ] Background diagnostic freshness can be established.
```

## 75.4 Testing

```text
[ ] nextest is configured with repository profiles.
[ ] Doctests run separately.
[ ] Shared-resource tests use groups or explicit limits.
[ ] Retries are diagnostic and flaky results are visible.
[ ] JUnit/report artifacts are retained in CI.
[ ] Snapshot updates require review.
```

## 75.5 Test quality

```text
[ ] Coverage scope/features/targets are documented.
[ ] Critical covered code is mutation-tested periodically.
[ ] Fuzz targets exist for untrusted-input surfaces.
[ ] Fuzz failures are minimized and promoted to regression tests.
[ ] Unsafe/concurrent critical tests run under Miri.
[ ] Seed ranges and unsupported Miri paths are recorded.
```

## 75.6 Compatibility

```text
[ ] Feature states validated with cargo-hack.
[ ] no-default-features behavior tested where promised.
[ ] MSRV declared and verified.
[ ] Public crates use semver checks against an explicit baseline.
[ ] Target-specific public APIs are included in compatibility policy.
```

## 75.7 Dependency hygiene and supply chain

```text
[ ] Machete/Shear fast checks run.
[ ] Udeps is available for adjudication.
[ ] deny.toml expresses organizational policy.
[ ] RustSec scan runs on the representative lockfile.
[ ] cargo-vet policy/audit state is version-controlled where adopted.
[ ] Exemptions and ignores have owners/expiry.
[ ] Unsafe-surface changes are reviewed.
[ ] Release binaries embed auditable dependency metadata where feasible.
[ ] Final artifact can be scanned with cargo audit bin.
```

## 75.8 Performance and artifacts

```text
[ ] Profiling build retains useful symbols.
[ ] Baselines use Hyperfine or application benchmarks.
[ ] Samply/flamegraph workloads are representative.
[ ] Code-size regressions use Bloat plus section/symbol evidence.
[ ] Macro/codegen inspection uses the exact relevant features/profile/target.
[ ] Performance claims include before/after distributions and correctness checks.
```

## 75.9 Cross-target and Python delivery

```text
[ ] Compile, link, emulated test, native test, and runtime-floor states are separate.
[ ] Cross images/configuration are pinned and least-privilege.
[ ] cargo-zigbuild artifacts are tested in representative runtimes.
[ ] Maturin development and wheel validation are separate.
[ ] Wheels are installed in clean environments across supported Python versions.
[ ] Artifact hashes and build inventory are retained.
```

## 75.10 Agent reporting

```text
[ ] Exact command included.
[ ] Tool and compiler versions included.
[ ] Target/features/profile included.
[ ] Exit status and report location included.
[ ] Pre-existing failures separated.
[ ] Mutations to source/environment disclosed.
[ ] Skipped checks and rationale disclosed.
[ ] Claims match the actual evidence class.
```

---

# Source reference index

The links below are official project documentation, Rust project documentation, or primary project repositories. The operational source of truth remains the installed tool's `--help` and exact release documentation.

## Rust toolchain and compiler

[RA]: https://rust-analyzer.github.io/book/ "rust-analyzer manual"  
[RUSTUP-COMP]: https://rust-lang.github.io/rustup/concepts/components.html "rustup components"  
[RUST-COV]: https://doc.rust-lang.org/rustc/instrument-coverage.html "rustc instrumentation-based code coverage"  
[RUSTC-DEV]: https://rustc-dev-guide.rust-lang.org/ "rustc development guide"  
[RUSTC-PUBLIC]: https://rust-lang.github.io/rustc_public/getting-started.html "rustc_public project guidance"  
[MIRI]: https://github.com/rust-lang/miri "Miri official repository and user guide"

## Acquisition, orchestration, and local workflow

[BINSTALL]: https://github.com/cargo-bins/cargo-binstall "cargo-binstall official repository"  
[CARGO-UPDATE]: https://github.com/nabijaczleweli/cargo-update "cargo-update official repository"  
[SCCACHE]: https://github.com/mozilla/sccache "sccache official repository"  
[JUST]: https://just.systems/man/en/ "Just manual"  
[BACON]: https://dystroy.org/bacon/ "Bacon documentation"  
[WATCHEXEC]: https://github.com/watchexec/watchexec/tree/main/crates/cli "Watchexec CLI documentation"  
[FD]: https://github.com/sharkdp/fd "fd official repository"  
[TYPOS]: https://github.com/crate-ci/typos "Typos official repository"

## Testing and correctness

[NEXTEST]: https://nexte.st/ "cargo-nextest documentation"  
[LLVM-COV]: https://github.com/taiki-e/cargo-llvm-cov "cargo-llvm-cov official repository"  
[INSTA]: https://insta.rs/ "Insta documentation"  
[INSTA-CLI]: https://insta.rs/docs/cli/ "cargo-insta CLI documentation"  
[MUTANTS]: https://mutants.rs/ "cargo-mutants user guide"  
[FUZZ]: https://rust-fuzz.github.io/book/ "Rust Fuzz Book"

## Compatibility and dependency hygiene

[HACK]: https://github.com/taiki-e/cargo-hack "cargo-hack official repository"  
[MSRV]: https://github.com/foresterre/cargo-msrv "cargo-msrv official repository"  
[SEMVER]: https://github.com/obi1kenobi/cargo-semver-checks "cargo-semver-checks official repository"  
[SHEAR]: https://github.com/Boshen/cargo-shear "cargo-shear official repository"  
[MACHETE]: https://github.com/bnjbvr/cargo-machete "cargo-machete official repository"  
[UDEPS]: https://github.com/est31/cargo-udeps "cargo-udeps official repository"

## Supply chain and security

[DENY]: https://embarkstudios.github.io/cargo-deny/ "cargo-deny documentation"  
[AUDIT]: https://github.com/rustsec/rustsec/tree/main/cargo-audit "cargo-audit official source"  
[RUSTSEC]: https://rustsec.org/ "RustSec"  
[VET]: https://mozilla.github.io/cargo-vet/ "Cargo Vet book"  
[GEIGER]: https://github.com/geiger-rs/cargo-geiger "cargo-geiger official repository"  
[AUDITABLE]: https://github.com/rust-secure-code/cargo-auditable "cargo-auditable official repository"

## Compiler output, artifacts, and performance

[EXPAND]: https://github.com/dtolnay/cargo-expand "cargo-expand official repository"  
[SHOW-ASM]: https://github.com/pacak/cargo-show-asm "cargo-show-asm official repository"  
[BINUTILS]: https://github.com/rust-embedded/cargo-binutils "cargo-binutils official repository"  
[BLOAT]: https://github.com/RazrFalcon/cargo-bloat "cargo-bloat official repository"  
[HYPERFINE]: https://github.com/sharkdp/hyperfine "Hyperfine official repository"  
[SAMPLY]: https://github.com/mstange/samply "Samply official repository"  
[FLAMEGRAPH]: https://github.com/flamegraph-rs/flamegraph "cargo-flamegraph official repository"

## Cross-target and Python delivery

[CROSS]: https://github.com/cross-rs/cross "Cross official repository"  
[ZIGBUILD]: https://github.com/rust-cross/cargo-zigbuild "cargo-zigbuild official repository"  
[MATURIN]: https://www.maturin.rs/ "Maturin user guide"

---

# Closing operational model

The installed environment should be understood as a set of **orthogonal evidence generators coordinated through a repository-owned command layer**:

```text
edit correctly
    rust-analyzer + Bacon + sccache

test reliably
    nextest + doctests

measure test reach and strength
    llvm-cov + mutants + fuzz + snapshots

interrogate unsafe behavior
    Miri + fuzz + Geiger + reasoning

preserve compatibility
    cargo-hack + cargo-msrv + semver-checks

control dependency risk
    Shear/Machete/Udeps + deny + audit + vet + auditable

understand generated and compiled behavior
    expand + show-asm + binutils + bloat

make performance claims honestly
    Hyperfine + Samply/flamegraph + exact codegen evidence

deliver beyond the host
    Cross + cargo-zigbuild + Maturin

make it usable by agents
    just + version pins + structured artifacts + explicit mutability
```

The best-in-class outcome is not “all tools installed.” It is a repository in which an agent can select the correct evidence, run it under an explicit configuration, interpret its limits, preserve the result, and avoid making claims stronger than the tools support.
