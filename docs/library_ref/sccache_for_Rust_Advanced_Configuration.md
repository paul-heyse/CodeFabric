# sccache for Rust — Advanced Configuration & Best-in-Class Reference

**Primary scope:** Rust compilation through Cargo / `rustc` only  
**sccache version anchor:** **0.17.0** (released 2026-07-29)  
**Reference date:** **2026-08-29**  
**Primary audience:** engineers, platform teams, CI owners, and LLM programming agents operating high-performance Rust development environments  
**Primary operating systems:** Linux and macOS; Windows semantics are called out where materially different  
**Source policy:** released upstream sccache 0.17.0 documentation/source and the current Cargo Book are authoritative; recommendations in this guide are explicitly separated from upstream facts

---

## Executive purpose

`sccache` is best understood as a **content-addressed cache in front of individual cacheable `rustc` invocations**, not as a Cargo build cache and not as a general-purpose incremental build system. Its largest gains appear when Cargo must invoke `rustc` again, the invocation satisfies sccache's Rust cacheability rules, and the same compiler inputs have already been compiled into a warm local or remote cache.

For Rust, that definition has two consequences that materially change a best-in-class design:

1. **ordinary `cargo check` is not the canonical sccache workload.** sccache's Rust path requires `link` to be present in `--emit`; Cargo documents `cargo check` as skipping final code generation, and normal check-mode library invocations use metadata rather than `link`. Use `cargo build`, or the compile phase of `cargo test`, to validate Rust compiler-cache effectiveness.
2. **cross-checkout path normalization is not implemented for Rust in released sccache 0.17.0.** The generic `SCCACHE_BASEDIRS` option exists, but upstream issue #2652 and the still-open PR #2794 explicitly document that the released Rust cache key remains checkout-path-sensitive. Stable absolute checkout paths are therefore the current safe portability strategy.

A best-in-class Rust configuration has to answer seven independent questions:

1. **Which `rustc` invocations are actually cacheable?**
2. **Which Cargo commands generate those invocations?**
3. **Which inputs participate in cache identity, and where can hidden inputs escape that model?**
4. **Should a workflow optimize for rustc incremental compilation or whole-invocation sccache reuse?**
5. **Should storage be local-only or hierarchical across local and remote tiers?**
6. **Which trust domains may read from or write to shared compiler artifacts?**
7. **How will hits, misses, non-cacheable reasons, latency, and link-time floors be measured rather than assumed?**

The canonical starting policy of this reference is:

```text
Cargo integration
  [build] rustc-wrapper = "sccache"

shared-reuse build/test/CI commands
  CARGO_INCREMENTAL=0
  cargo build / cargo test --no-run / cargo test

interactive check loop
  preserve Cargo/rustc incremental behavior when useful
  do not expect ordinary cargo check units to become sccache hits

sccache 0.17 execution policy
  evaluate client-side mode on the actual workload
  prefer it after validation for simple local-cache workloads
  do not assume it is universally faster or behaviorally identical in every multi-level topology

storage policy
  L0 local SSD disk
  + optional low-latency shared Redis
  + optional durable object storage / CI-native cache

path policy for released 0.17.0
  keep absolute checkout paths stable wherever cross-run reuse matters
  DO NOT rely on SCCACHE_BASEDIRS for Rust cross-worktree/cross-checkout hits

security policy
  remote cache is trusted build infrastructure
  untrusted jobs are read-only or fully isolated

observability policy
  validate with cargo build, cold Cargo target state, and warm sccache
  capture standard/advanced stats and non-cacheable reasons
  retain JSON stats in CI when practical
```

This is **not** a universal statement that sccache should replace incremental compilation for every local edit loop. Rust's native incremental compilation is often superior when one developer repeatedly edits the same workspace crate inside one long-lived `target/` tree. The correct model is therefore command- and workload-specific:

```text
Shared-reuse BUILD mode
  sccache ON
  rustc incremental OFF
  cargo build / compile-producing test workflow
  optimized for cold target trees, repeated revisions, CI, stable-path runners, fleet reuse

Hot CHECK/edit mode
  rustc incremental ON when it helps
  ordinary cargo check is not a meaningful sccache-hit target
  optimized for repeated edits in one persistent target directory
```

Treating those as different optimization problems is the single most important Rust-specific design decision in a serious sccache deployment.

## Documentation map

| Part | Sections | Purpose |
|---|---:|---|
| Foundations | 0–6 | scope, version policy, mental model, Rust cacheability, cache-key correctness, Cargo integration |
| Compiler policy | 7–12 | incremental compilation, execution modes, paths/worktrees, profiles/features/targets, macros/build scripts, link boundaries |
| Storage | 13–20 | disk, multi-level topology, Redis, S3/R2, GitHub Actions, secondary backends, compression, sizing |
| Operations | 21–27 | security, CI trust policy, observability, debugging, distributed compilation, rollout, benchmarking |
| Canonical profiles | 28–33 | workstation, worktree/agent, team remote cache, CI, GitHub Actions, dual-mode development |
| Reference | 34–39 | configuration precedence, environment-variable catalog, anti-patterns, decision tables, checklist, source index |

The operating pattern used throughout is:

```text
upstream fact
→ Rust-specific consequence
→ recommended configuration
→ tradeoff / exception
→ validation rule
```

---

# 0) Scope, exclusions, and baseline assumptions

## 0.1 Included

This reference covers sccache only as it relates to Rust:

- Cargo integration with `rustc-wrapper` / `RUSTC_WRAPPER`;
- Rust cacheability boundaries;
- cache-key formation and dependency discovery;
- `rustc` incremental compilation interaction;
- Cargo workspaces, profiles, features, targets, cross-compilation, and `RUSTFLAGS`;
- build scripts and procedural macros as correctness and cacheability boundaries;
- local disk, multi-level cache storage, and remote backends;
- client-side versus server-side execution in sccache 0.17;
- remote cache trust, credentials, and CI isolation;
- statistics, diagnostics, cache recaching, and benchmarking;
- optional distributed compilation where it affects Rust deployment choices.

## 0.2 Explicitly excluded

The guide does **not** document C, C++, CUDA, HIP, MSVC, GCC, Clang, or their preprocessing semantics. Options such as `SCCACHE_DIRECT` and `[cache.disk.preprocessor_cache_mode]` exist in sccache but are **irrelevant to pure Rust compilation** and should not be copied into a Rust-only configuration merely because they appear in the global sccache configuration reference.

A Rust repository can still transitively invoke a native compiler through crates such as `cc`, `cmake`, or `bindgen`-driven build scripts. That is a separate compiler path. This document does not optimize it.

## 0.3 Baseline assumptions

A normal environment already has:

```bash
rustc -vV
cargo -V
rustup show active-toolchain
sccache --version
```

and uses Cargo to generate ordinary `rustc` invocations. sccache's Rust support is explicitly designed around Cargo-shaped invocations; arbitrary hand-authored `rustc` commands may fall outside the supported cacheability envelope.

---

# 1) Version and source-of-truth policy

## 1.1 Version anchor

This document is pinned to **sccache 0.17.0**. That matters because 0.17.0 introduced a major execution-architecture change: **client-side mode**, which moves the compilation/cache pipeline into each short-lived sccache CLI process while retaining the daemon as shared cache/storage/statistics infrastructure.

Upstream marks client-side mode as recommended and expects it eventually to become the only supported mode. It is opt-in in 0.17.0.

## 1.2 Why version pinning matters operationally

The following surfaces can change independently:

- supported compiler arguments;
- cache-key version and parsing rules;
- storage backend fields;
- default cache sizes and behavior;
- distributed protocol behavior;
- CLI flags and statistics schema;
- GitHub Actions integration behavior.

Before automation parses output or assumes a feature, capture:

```bash
sccache --version
sccache --help
rustc -vV
cargo -V
```

For CI, pin the sccache binary or action version instead of silently consuming an arbitrary future release.

## 1.3 Configuration restart rule

The daemon owns storage configuration and shared state. Upstream notes that some environment changes require a server restart to take effect. When changing storage, path normalization, distributed configuration, or execution-mode-affecting settings during diagnosis, use the explicit lifecycle:

```bash
sccache --stop-server || true
sccache --start-server
sccache --show-stats
```

Do not conclude that a new configuration is ineffective until the server state is known to have been refreshed.

---

# 2) Rust mental model: where sccache sits

## 2.1 The normal Cargo path

Without sccache:

```text
Cargo
  │
  ├─ resolves package graph / features / profiles / targets
  ├─ decides which units need compilation
  │
  └─ invokes rustc N times
         │
         └─ rustc reads source + dependencies + flags + environment
```

With sccache:

```text
Cargo
  │
  └─ invokes wrapper for each rustc unit
         │
         ▼
      sccache
         │
         ├─ identify real rustc/toolchain
         ├─ parse rustc invocation
         ├─ obtain dependency/source/env information
         ├─ compute cache identity
         ├─ query storage
         │     ├─ hit  → restore outputs
         │     └─ miss → invoke rustc → store outputs
         │
         └─ return rustc-compatible status/stdout/stderr
```

The wrapper does not replace Cargo's dependency graph or unit scheduling. Cargo still decides which invocations exist; sccache decides whether a cacheable invocation needs actual compilation.

## 2.2 sccache is not Cargo's `target/` directory

These two caches operate at different layers:

```text
Cargo target artifacts
  build-system state tied to Cargo unit graph and target directory

sccache artifacts
  reusable results of compatible individual compiler invocations
```

This is why sccache is valuable after:

- deleting or changing `target/`;
- rebuilding from another checkout **only when Rust path-sensitive inputs remain identical**; released 0.17.0 does not normalize distinct Rust worktree roots;
- moving between CI runners;
- using separate target directories for separate workflows;
- rebuilding a dependency graph that is identical at the compiler-input level.

## 2.3 Cache hit does not mean Cargo skipped the unit

Cargo may decide a unit needs compilation and launch sccache. sccache can then turn that would-be compilation into a cache hit. Build logs and timing should therefore be interpreted at both levels.

## 2.4 What the daemon means in 0.17

Even client-side mode is not daemonless. In 0.17:

```text
short-lived sccache CLI processes
  run compiler/cache pipeline
        │
        └─ IPC → long-lived local daemon
                    ├─ storage backend
                    ├─ shared state
                    └─ aggregate statistics
```

For a local disk backend, client-side mode can obtain a local path through the storage handshake and read the cached entry directly. For remote backends it receives raw bytes through the daemon/storage abstraction.

---

# 3) What is cacheable in Rust

## 3.1 Upstream Rust requirements

sccache's Rust path is deliberately narrower than “anything rustc can compile.” The released Rust integration expects Cargo-like invocations with:

- `--emit`;
- `--crate-name`;
- `--out-dir`;
- an actual source-file input rather than stdin;
- `link` present in `--emit`;
- only `link`, `metadata`, and `dep-info` among the supported emitted artifact kinds.

An explicit `-o file` output form is unsupported for the Rust cache path. The important practical consequence is that **the presence of a Rust compiler invocation is not sufficient; its output mode must also be cacheable.**

## 3.2 Crate-type boundary

The most important crate-type rule is:

> Rust crate compilations that invoke the system linker are not cacheable by sccache.

Upstream explicitly lists:

```text
not cacheable because they invoke the system linker:
  bin
  dylib
  cdylib
  proc-macro
```

The released 0.17 Rust implementation represents the direct cacheable crate types as:

```text
rlib
staticlib
```

### Practical crate-type matrix

| Rust unit | Expected sccache value | Reason |
|---|---:|---|
| registry dependency library → `rlib` | **high** | direct cacheable compilation result |
| unchanged workspace library → `rlib` with incremental disabled | **high** | direct cacheable compilation result |
| `staticlib` | cacheable | explicitly represented as a cacheable crate type |
| final `bin` target | not cacheable | system linker boundary |
| `cdylib` | not cacheable | system linker boundary |
| `dylib` | not cacheable | system linker boundary |
| `proc-macro` crate itself | not cacheable | system linker boundary |
| `build.rs` executable itself | generally not cacheable | executable / linker boundary |
| library dependencies of build scripts/proc macros | often cacheable | ordinarily `rlib` units |

This explains a common pattern: a build can have excellent dependency-library reuse while still showing unavoidable non-cacheable work around build scripts, proc macros, test executables, and final linking.

## 3.3 Cargo-command cacheability is not uniform

The Rust `--emit` restriction makes the Cargo command itself a first-order design variable.

| Cargo workflow | Rust sccache value | What to expect |
|---|---:|---|
| `cargo build` | **primary validation/workhorse** | normal library units use `--emit=...,link`; cacheable `rlib`/`staticlib` units can hit |
| `cargo build --release` | **high for reusable library units** | separate optimized keys; final binary/linking remains outside the cacheable Rust surface |
| `cargo test --no-run` | **partial to high** | dependencies/libraries can hit; generated test executables are linker-driving and not cacheable |
| `cargo test` | **same compile reuse + test execution** | sccache affects compilation only, never test runtime |
| `cargo run` | **same compile phase as build, then execution** | cacheable dependency/library work can hit; final binary remains non-cacheable |
| `cargo check` | **low / not a canonical cache workload** | normal check-mode crates skip code generation and omit `link`, so the principal Rust units are not cacheable |
| `cargo clippy` | **do not assume build-like hit behavior** | it is check-oriented compiler work; benchmark separately rather than using it as a cache acceptance test |

Cargo's own documentation describes `cargo check` as compiling without the final code-generation step. sccache's Rust documentation simultaneously requires `link` in `--emit`. These two upstream contracts are why a “cold target, warm sccache, `cargo check`” experiment is the wrong acceptance test for Rust caching.

## 3.4 Architectural consequence for binaries

A monolithic binary crate concentrates more application code behind a non-cacheable final crate compilation than a library-heavy design. When it also improves software architecture independently, a thin binary target over substantial library crates increases the fraction of work that can be reused:

```text
less cache-friendly
  large bin crate containing most application logic

more cache-friendly
  library crate(s) containing application logic
  + thin bin crate for CLI/runtime wiring
```

Do **not** refactor solely to satisfy a cache. When library extraction is already desirable for testing, modularity, or multiple front ends, cacheability is an additional benefit.

## 3.5 Tests, examples, and benchmarks

Cargo test/example/benchmark graphs mix cacheable library units with executable-like outputs. Expect **mixed** behavior rather than a blanket statement that “tests are cached.”

For cache validation, `cargo test --no-run` is useful when the test build graph is representative, but interpret statistics correctly:

```text
cacheable dependency/library compilations → may hit
proc-macro/build-script/test binaries      → expected non-cacheable work
actual test execution                       → unrelated to sccache
```

Use advanced statistics to see the breakdown instead of inferring cacheability from the Cargo command name.

---

# 4) Rust cache-key anatomy

## 4.1 Documented high-level inputs

For Rust, upstream documents a BLAKE3-based digest that incorporates, among other inputs:

- the path to the real `rustc` executable;
- the host triple for that `rustc`;
- the rustc sysroot path;
- digests of shared libraries under the rustc sysroot `lib` directory;
- rlib dependency information used by the distributed client path;
- parsed arguments from the `rustc` invocation;
- digests of discovered compiled files.

The implementation also derives full source files and tracked environment dependencies from rustc-generated dep-info.

## 4.2 Why the *real* rustc matters

Rustup typically exposes a proxy named `rustc`. sccache resolves compiler identity rather than treating the generic proxy path as the whole toolchain identity. Compiler/toolchain drift should therefore naturally separate cache identities.

Operational rule:

```text
Never expect stable hits across materially different Rust toolchains merely because
`rustc` appears at the same shell path.
```

Record `rustc -vV` alongside cache benchmarks.

## 4.3 Source discovery through dep-info

The Rust implementation runs rustc in a dep-info-oriented mode to discover source inputs, then hashes the referenced files. This is one reason Cargo-shaped invocations are the supported path: the wrapper can derive a compiler-informed input set rather than hashing only the crate root.

## 4.4 Environment dependencies

Modern rustc dep-info can contain `# env-dep:` records. sccache parses those records and includes the values in its dependency model. This covers normal tracked compile-time environment uses such as `env!` / `option_env!` on supported Rust versions.

The important distinction is:

```text
tracked compiler-visible environment dependency
  → can participate in cache identity

arbitrary hidden side effect / external read
  → may not participate unless rustc exposes it as a dependency
```

## 4.5 Compiler arguments are semantic inputs

Anything that changes the effective rustc command line should be presumed to split the cache unless sccache explicitly normalizes it. Examples include:

- target triple or target JSON;
- `--cfg` values;
- feature-derived `--cfg feature=...` flags;
- optimization level;
- debuginfo settings;
- codegen units;
- panic strategy;
- LTO-related flags;
- CPU feature / target-cpu flags;
- `--extern` dependency identities and locations;
- link search paths and static-library inputs;
- instrumentation flags;
- profile settings materialized as rustc arguments;
- `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, target-specific rustflags.

This is a correctness property, but it also means a fleet with unnecessary flag variation will fragment its cache.

## 4.6 Cache-key versioning

sccache internally versions the Rust cache-key scheme. Treat sccache upgrades as capable of causing a cold-cache event even when the source tree is unchanged. Do not build operational expectations around indefinite cache-key stability across versions.

---

# 5) Correctness boundaries and hidden inputs

## 5.1 Cache correctness principle

A reusable artifact is correct only if every input that can affect the output is represented in the cache identity or otherwise forces a miss.

For ordinary compiler-tracked Rust source and environment inputs, sccache works with rustc's dependency information. The dangerous cases are **code executed at compile time that performs external reads unknown to rustc's normal dependency model**.

## 5.2 Procedural macros

Upstream explicitly warns that procedural macros that read files from the filesystem may not be cached properly.

A proc macro can do arbitrary host I/O:

```rust
// conceptual example only
let text = std::fs::read_to_string("schema.json")?;
```

If that external file is not represented in rustc's tracked inputs, changing it can violate the cache's assumption that the compiler inputs are unchanged.

### Recommended policy

For proc macros under your control:

- prefer explicit token/input-driven behavior;
- use compiler-supported tracked path/env mechanisms when available and stable for your toolchain;
- avoid invisible reads from the repository, `$HOME`, `/tmp`, network services, clocks, or mutable global state;
- add a clean rebuild / cache-bypass test for macro outputs that depend on external resources.

For third-party proc macros with filesystem behavior, treat cache correctness as a validation requirement rather than an assumption.

## 5.3 Build scripts

`sccache` is not a cache of `build.rs` execution. Cargo decides when the build script runs, and build scripts can emit values that alter downstream compilation:

```text
cargo:rustc-cfg
cargo:rustc-env
cargo:rustc-link-search
cargo:rustc-link-lib
rerun-if-changed
rerun-if-env-changed
```

Downstream effects that become rustc arguments or tracked environment normally create distinct compiler inputs. But arbitrary build-script side effects remain a Cargo/build-system correctness concern, not something sccache can repair.

### Best practice

A reproducible build script should:

- declare `rerun-if-changed` / `rerun-if-env-changed` accurately;
- avoid non-hermetic network or host-state inputs;
- emit deterministic output for deterministic inputs;
- place generated outputs in Cargo's expected output directory;
- not rely on timestamps or random values unless that non-reproducibility is intentional.

The more hermetic the Cargo build, the safer and more valuable shared compilation caching becomes.

---

# 6) Cargo integration surfaces

## 6.1 Preferred wrapper mechanisms

### User-global Cargo configuration

```toml
# ~/.cargo/config.toml
[build]
rustc-wrapper = "sccache"
```

This is appropriate when sccache is guaranteed to exist on the workstation and the user wants it for most repositories.

### Environment-scoped wrapper

```bash
export RUSTC_WRAPPER=sccache
cargo build
```

This is the preferred pattern for CI, containers, task runners, and controlled scripts because the dependency on sccache is explicit in the environment.

### Repository-local Cargo configuration

```toml
# .cargo/config.toml
[build]
rustc-wrapper = "sccache"
```

Use this only when the repository intentionally standardizes sccache and its developer bootstrap guarantees the executable exists. Committing a mandatory wrapper into a general open-source repository can make ordinary Cargo builds fail for contributors who do not have sccache installed.

## 6.2 Absolute path versus PATH lookup

A user-global machine config can use an absolute path when executable identity is intentionally fixed:

```toml
[build]
rustc-wrapper = "/opt/homebrew/bin/sccache"
```

A portable shared configuration normally uses `sccache` and relies on a controlled `PATH`.

Do not hard-code one developer's filesystem path in committed repository configuration.

## 6.3 Bypassing sccache for diagnosis

To remove the wrapper from a single invocation:

```bash
RUSTC_WRAPPER= cargo build
```

When also comparing against rustc incremental compilation, set it explicitly:

```bash
RUSTC_WRAPPER= CARGO_INCREMENTAL=1 cargo build
```

A diagnosis is not valid if one run uses a different profile, feature set, target, or target directory by accident.

## 6.4 Wrapper chaining

Cargo also has `rustc-workspace-wrapper`, which has different semantics from `rustc-wrapper`. When custom wrappers, instrumentation, or build tooling are already present, verify the exact nesting Cargo constructs rather than blindly replacing one wrapper with another.

The operational invariant is:

```text
there must be one well-understood path from Cargo to the real rustc,
and every wrapper in that path must preserve arguments, environment, and exit semantics.
```

---

# 7) The critical Rust tradeoff: sccache versus rustc incremental compilation

## 7.1 Upstream constraint

sccache's Rust documentation states that rustc incremental compilation must be disabled **for a Rust invocation to be cacheable by sccache**.

Cargo's defaults make the interaction subtle:

- the `dev` profile defaults to incremental compilation;
- the `release` profile defaults to non-incremental compilation;
- Cargo documents incremental compilation as applying only to workspace members and path dependencies;
- registry dependencies therefore often remain excellent sccache candidates even when a developer has not globally disabled incremental compilation;
- `CARGO_INCREMENTAL=0` overrides profile/config incremental settings for the invocation.

The last point matters because a global repository setting of `incremental = false` also affects workflows such as `cargo check`, where sccache cannot cache the normal metadata-only library units. A best-in-class Rust setup should not disable useful incremental checking merely to satisfy a cache that cannot accelerate that check unit.

## 7.2 The two reuse scopes

### Rust incremental compilation

Optimizes:

```text
same crate
+ same/familiar target directory
+ nearby source revision
→ reuse internal compiler work products
```

Strengths:

- excellent for repeated small edits to one large local workspace crate;
- fine-grained reuse inside a changed crate;
- particularly relevant to `cargo check` / edit-feedback loops;
- no remote service required.

Weaknesses:

- reuse is tied to local incremental state;
- poor portability across clean target directories/runners/machines;
- incremental data can consume substantial disk;
- a Rust invocation using incremental mode is not cacheable by sccache.

### sccache

Optimizes:

```text
identical cacheable rustc invocation inputs
+ link-producing cacheable crate type
→ reuse complete compiler output
```

Strengths:

- strong for cold Cargo target trees with a warm sccache;
- excellent for stable dependency graphs;
- valuable across repeated revisions and CI workers when path-sensitive inputs match;
- enables shared remote compiler-result reuse.

Weaknesses:

- cannot partially reuse a changed crate the way rustc incremental can;
- ordinary `cargo check` units are generally outside its Rust cacheable surface;
- final linker-driving units remain non-cacheable;
- released 0.17.0 Rust keys remain checkout-path-sensitive;
- remote cache hits can be slower than local compilation for very small units;
- shared caches introduce trust and operations concerns.

## 7.3 Best-in-class command policy: do not force one mode onto every Cargo command

Recommended repository-level integration:

```toml
# .cargo/config.toml
[build]
rustc-wrapper = "sccache"
```

Then select incremental policy at the command/workflow boundary.

### Shared-reuse build mode

```bash
CARGO_INCREMENTAL=0 cargo build --workspace
```

Use for:

- CI build jobs;
- cold/isolated target trees;
- repeated clean-ish builds;
- build/test compile phases where cross-run reuse matters;
- benchmarking sccache itself.

### Interactive check mode

```bash
RUSTC_WRAPPER= cargo check --workspace
```

Allow the normal profile/incremental policy unless repository measurements justify
something else, and bypass the wrapper for this mode. The deployed 0.17.0 binary rejects
incremental Rust invocations rather than treating them as an ordinary non-cacheable
pass-through; normal check-mode library invocations also omit `link` in any event.

## 7.4 Why a global `incremental = false` is not the default recommendation here

This configuration is valid:

```toml
[build]
rustc-wrapper = "sccache"
incremental = false
```

but it applies the non-incremental policy broadly. It is appropriate when the repository consciously prioritizes build/CI parity over incremental edit feedback.

For a developer-focused repository, the more flexible default is:

```text
global wrapper     = sccache
shared build jobs  = CARGO_INCREMENTAL=0
interactive check  = profile default / measured policy
```

That preserves sccache eligibility where it matters without unnecessarily degrading a `cargo check` loop that sccache cannot meaningfully cache.

## 7.5 Required benchmark matrix

Benchmark at least:

```text
A. cargo build, cold target + empty sccache
B. cargo build, cold target + warm sccache
C. cargo build, one-line workspace edit, sccache + CARGO_INCREMENTAL=0
D. cargo build, same edit, no wrapper + CARGO_INCREMENTAL=1
E. cargo check, repeated small edit with normal incremental policy
F. cargo check, wrapper bypassed, same target state
```

The winner depends on the workload. Do not compare a warm incremental target against a cold Cargo target and attribute the difference to sccache alone.

## 7.6 Recommended repository command contract

A high-quality task runner should make the difference explicit:

```just
# Reusable compiler-output mode; suitable for CI-equivalent local validation.
build-shared:
    CARGO_INCREMENTAL=0 cargo build --workspace

# Interactive semantic feedback; sccache hits are not the objective.
check:
    RUSTC_WRAPPER= cargo check --workspace --all-targets

# Apples-to-apples build benchmark against rustc incremental.
build-incremental:
    RUSTC_WRAPPER= CARGO_INCREMENTAL=1 cargo build --workspace
```

Do not toggle modes silently inside one benchmark or diagnostic claim.

---

# 8) sccache 0.17 execution modes

## 8.1 Server-side mode

Server-side mode remains available in 0.17. Each short-lived wrapper process sends the compile request to the daemon, and the daemon performs cache lookup, compiler execution on a miss, output handling, and storage.

```text
Cargo → sccache CLI → IPC → daemon → cache / rustc
```

Under highly parallel local compilation, a single daemon can become an avoidable execution bottleneck.

## 8.2 Client-side mode

Enable with either:

```bash
export SCCACHE_CLIENT_SIDE=1
```

or:

```toml
client_side_mode = true
```

In client-side mode:

```text
Cargo
  └─ many sccache CLI processes
       ├─ parse/hash/compile locally
       └─ IPC cache operations / stats → daemon → storage
```

The 0.17 release introduced this architecture. Current upstream configuration documentation calls it the recommended mode and says it is expected to become the only supported configuration in the future.

## 8.3 Best-in-class interpretation: recommended architecture, but validate 0.17.0 on the real topology

Do **not** translate “upstream recommended” into “unconditionally set it everywhere.” Client-side mode is new in 0.17.0, and current upstream issue/PR evidence shows behavior and performance still being exercised:

- open issue **#2796** reports that a writable local L0 was not populated from a read-only WebDAV L1 in a `disk,webdav` multi-level configuration when client-side mode was enabled;
- open PR **#2794** includes a mixed ClickHouse benchmark where the author's client-side run was substantially slower than server-side, and explicitly calls the slowdown interesting enough for follow-up.

Those reports do **not** prove client-side mode is generally slow or broken for Rust. They do mean that an authoritative 0.17.0 deployment should validate it rather than cargo-cult it.

### Recommended policy

```text
single-machine local disk cache
  enable SCCACHE_CLIENT_SIDE=1
  A/B benchmark once
  keep it if equal/faster and stats/cache population are correct

multi-level remote cache
  test both modes during rollout
  specifically test slow-tier hit → L0 backfill
  keep the mode that satisfies correctness + latency for the chosen backend combination

distributed compilation
  client-side mode is incompatible; use server-side path
```

## 8.4 Client-side incompatibilities

In 0.17, client-side mode is ignored when:

- `SCCACHE_ERROR_LOG` file logging is used;
- distributed compilation is configured.

Those configurations retain server-side compile processing.

### Debugging consequence

Do not set `SCCACHE_ERROR_LOG` permanently in an environment that expects client-side mode. Prefer stderr logging for ordinary client-side diagnosis:

```bash
SCCACHE_LOG=debug SCCACHE_LOG_MILLIS=1 cargo build
```

If a persistent server log file is necessary, knowingly accept server-side processing for that diagnostic session.

## 8.5 Acceptance test for execution mode

After changing mode:

```bash
sccache --stop-server || true
sccache --start-server
sccache --zero-stats
rm -rf target
CARGO_INCREMENTAL=0 cargo build --workspace
sccache --show-adv-stats
```

For multi-level storage also verify:

```text
remote hit with local L0 empty
artifact returned correctly
local L0 populated/backfilled as intended
subsequent local hit occurs
```

The existence of the daemon does **not** imply client-side mode is off; the daemon remains as a storage/state/stats service.

---

# 9) Rust path identity in 0.17.0: the `SCCACHE_BASEDIRS` gap

## 9.1 The generic documentation and the Rust implementation diverge

sccache's generic configuration reference exposes:

```text
SCCACHE_BASEDIRS
basedirs = [...]
```

and describes it as stripping equivalent source-root prefixes for cross-checkout cache reuse. **That description is not sufficient for Rust in released 0.17.0.**

Upstream issue **#2652**, “Wire SCCACHE_BASEDIRS into Rust hash key,” explicitly states that the existing implementation did not cover the Rust compiler. As of 2026-08-29, PR **#2794**, “rust: support SCCACHE_BASEDIRS across checkout roots,” remains open. The PR states directly that current Rust compilation includes checkout-specific paths in its cache key and that existing `SCCACHE_BASEDIRS` support does not provide cross-checkout Rust hits.

Therefore:

> **Do not use `SCCACHE_BASEDIRS` as a claimed Rust optimization on sccache 0.17.0.**

## 9.2 Why Rust is path-sensitive

Rust/Cargo invocations can carry absolute paths through several channels, including:

- source paths;
- working directory identity;
- `--extern` paths;
- `-L` search paths;
- Cargo-provided path-valued environment variables;
- remap-related compiler arguments;
- toolchain/sysroot paths.

Historical sccache discussion also notes that location information can be embedded in Rust artifacts. Blindly stripping paths from a key without ensuring the compiler emits path-independent/equivalent artifacts can therefore be a correctness problem, not merely a missed optimization.

## 9.3 Current best-in-class path policy

For released 0.17.0, maximize Rust cache portability by standardizing the paths that actually reach Cargo/rustc:

```text
CI runners / containers
  use a canonical absolute checkout root across equivalent jobs
  keep CARGO_HOME / toolchain layout stable when practical
  keep target/profile/features/RUSTFLAGS semantically stable

local worktrees
  accept that distinct absolute worktree roots can split Rust cache keys
  do not promise workspace-crate cross-worktree hits
  keep independent Cargo target directories for concurrency correctness
```

Examples of canonical CI checkout roots:

```text
/workspace/repo
/src/repo
/build/project
```

The exact path does not matter; **consistency across equivalent jobs does**.

## 9.4 Why `--remap-path-prefix` alone is not a 0.17 workaround

`--remap-path-prefix` is useful for reproducibility/debug-path control, but in released 0.17.0 it should not be presented as a complete cache-key normalization solution. The open Rust `SCCACHE_BASEDIRS` work has to reason about both:

```text
compiler-emitted path identity
AND
sccache cache-key path identity
```

A compiler flag whose own source path differs can itself participate in the parsed compiler arguments. Do not invent ad hoc path-stripping wrappers around the cache key unless you are prepared to prove artifact equivalence.

## 9.5 Practical worktree consequence

For parallel Git worktrees:

```text
worktree A: /repo/main
worktree B: /repo/task-17
```

expect the following in 0.17.0:

```text
registry/toolchain-level reuse may still occur where inputs match
workspace/path-sensitive Rust units may miss because checkout roots differ
SCCACHE_BASEDIRS does not fix that released behavior
```

This reduces—but does not eliminate—the usefulness of a shared sccache for agent/worktree workflows. The highest-value stable use cases remain:

- cold target trees in the **same checkout path**;
- CI jobs using a canonical path;
- stable dependency compiles;
- repeated builds of previously seen revisions at the same path;
- fleet sharing where filesystem/toolchain layouts are deliberately standardized.

## 9.6 Track the upstream Rust implementation, do not pre-document it as released

Maintain an explicit upgrade watch on:

- issue #2652: Rust `SCCACHE_BASEDIRS` support;
- PR #2794: current implementation effort;
- Rust cache-key version changes accompanying that work.

When a future released version merges equivalent functionality, re-run cross-checkout correctness and hit-rate tests before enabling it. Do not copy the open-PR behavior into a 0.17.0 production standard.

---

# 10) Cargo profiles, features, targets, and flags

## 10.1 Profiles naturally partition compiler outputs

Cargo profiles change compiler arguments. A `dev` build and a `release` build should not be expected to share all cache entries because optimization, debuginfo, codegen, assertions, panic strategy, and other settings differ.

That is correct behavior.

Do not try to force profile convergence merely to improve cache hit rate.

## 10.2 Features

Cargo features typically become configuration and dependency differences. Different feature sets can produce different:

- `--cfg feature=...` arguments;
- dependency graphs;
- optional dependencies;
- generated code paths.

Expect cache fragmentation when a CI matrix tests many feature combinations. The right optimization is to avoid meaningless feature combinations, not to make semantically different builds share artifacts.

## 10.3 Target triples

Host and cross-target builds are distinct compiler contexts. Keep target identity explicit:

```bash
cargo build --target x86_64-unknown-linux-gnu
cargo build --target aarch64-unknown-linux-gnu
```

Do not interpret low sharing between different targets as poor cache effectiveness.

## 10.4 Custom target JSON

A custom target specification is a semantic compiler input. Changes to target JSON must invalidate incompatible results. Keep target files version-controlled and deterministic.

## 10.5 `RUSTFLAGS` and encoded rustflags

A fleet with divergent `RUSTFLAGS` can have a low shared hit rate even when source and toolchain match. Common sources of fragmentation include:

```text
-C target-cpu=native
-C target-feature=...
-C link-arg=...
-C debuginfo=...
-C codegen-units=...
--cfg ...
-Z ...
```

### Strong policy for shared CI caches

Use standardized flags per named build class:

```text
ci-check
ci-test
ci-release
coverage
profiling
sanitizer/nightly
```

Do not inject machine-specific `target-cpu=native` into a cache shared across heterogeneous machines unless the cache namespace and execution assumptions deliberately isolate compatible hardware.

## 10.6 Coverage and profiling

Instrumentation flags correctly create distinct outputs. Treat coverage/profiling as separate build classes, and often separate Cargo target directories as well:

```bash
CARGO_TARGET_DIR=target/coverage cargo llvm-cov ...
CARGO_TARGET_DIR=target/profiling cargo build --profile profiling
```

sccache can still accelerate compatible cacheable units, but do not expect ordinary development entries to match instrumented ones.

---

# 11) Workspaces and dependency-graph strategy

## 11.1 Where sccache usually earns its keep

Large workspaces often contain:

```text
many stable third-party rlibs
+ many stable internal leaf/core libraries
+ a smaller set of actively edited crates
+ final binaries/tests/macros/build scripts
```

That shape is favorable to sccache because the stable library layers can hit even while the actively edited crate misses.

## 11.2 High-fanout core crates

Changing a foundational crate can invalidate downstream crate compilation through changed dependency metadata. Cache reuse will fall because the compiler inputs genuinely changed.

This is not solved by increasing cache size.

Performance work should distinguish:

```text
miss due to eviction
miss due to changed compiler input
miss due to path fragmentation
miss due to toolchain/profile/feature split
non-cacheable crate type
```

Only the first is primarily a storage-capacity problem.

## 11.3 Parallel worktrees

Parallel worktrees still benefit from a shared compiler cache, but released 0.17.0 has a material Rust limitation: different absolute checkout roots are part of Rust cache identity, and `SCCACHE_BASEDIRS` does not normalize them.

Therefore the correct expectation is:

```text
same worktree, cold target, warm sccache
  strong reuse opportunity

different worktree root, same revision
  do not expect equivalent workspace-crate hit rates in 0.17.0

stable registry/toolchain inputs
  may still produce useful dependency reuse
```

Keep independent target directories for concurrent worktrees. Do not force a single mutable Cargo `target/` directory across autonomous agents merely to compensate for the current path-key limitation.

## 11.4 Target-directory isolation is not cache isolation

These can coexist:

```text
worktree A → target/a
worktree B → target/b
worktree C → target/c
                 │
                 └──── all use same sccache L0/L1/L2 storage
```

This is often a cleaner concurrency architecture than forcing all worktrees through one Cargo target tree.

---

# 12) Link boundaries, proc macros, and generated-code hotspots

## 12.1 Final link time remains outside sccache's main Rust value

Because link-producing crate types are non-cacheable, sccache does not eliminate final linker work. A build can show high cache hit rate and still be link-bound.

Therefore Rust build acceleration is a layered problem:

```text
sccache
  reduces cacheable rustc compilation work

fast linker / link configuration
  reduces non-cacheable final link work

Cargo graph / features / crate structure
  determines how much work exists

rustc incremental
  may accelerate changed local crates in hot-edit mode
```

Do not attribute linker time to an sccache failure.

## 12.2 Proc-macro-heavy builds

Proc-macro crate compilation is non-cacheable and proc-macro execution occurs inside downstream compilation. Large macro-heavy graphs can therefore have a lower ceiling on sccache's end-to-end speedup.

Measure:

- dependency library hits;
- proc-macro compile count/time;
- macro expansion time through compiler profiling if needed;
- final link time.

## 12.3 Generated source

Generated Rust source can still be cached when rustc sees it as a normal source dependency and its contents are stable. The system becomes fragile when generation depends on hidden, unstable, or undeclared inputs.

A strong generated-code pipeline is:

```text
explicit inputs
  → deterministic generator
  → stable generated bytes
  → rustc-visible source dependency
  → sccache-compatible compilation
```

A weak pipeline is:

```text
implicit host/network/time inputs
  → non-deterministic generated bytes
  → unexplained cache misses or correctness risk
```

---

# 13) Local disk cache: the universal L0

## 13.1 Default behavior

Without another backend, sccache uses local disk storage. Upstream defaults are:

| Platform | Default local cache path |
|---|---|
| Linux | `~/.cache/sccache` |
| macOS | `~/Library/Caches/Mozilla.sccache` |
| Windows | `%LOCALAPPDATA%\Mozilla\sccache` |

The default maximum local cache size is **10 GiB**.

## 13.2 Rust-only local configuration

A Rust workstation can keep disk configuration minimal:

```toml
[cache.disk]
dir = "/fast/local/ssd/sccache"
size = 53687091200  # 50 GiB
```

Equivalent environment form:

```bash
export SCCACHE_DIR="$HOME/.cache/sccache"
export SCCACHE_CACHE_SIZE="50G"
```

`client_side_mode = true` / `SCCACHE_CLIENT_SIDE=1` is a separate execution-architecture choice, not a property of the disk backend. Upstream recommends client-side mode in 0.17, but this reference requires an A/B benchmark and cache-population acceptance test before making it a workstation invariant.

For a simple disk-only workstation, client-side mode is the first mode to try:

```bash
export SCCACHE_CLIENT_SIDE=1
```

If it regresses the actual workload, use the server-side default and record the benchmark evidence.

## 13.3 Sizing guidance

The upstream 10 GiB default is a safe general default, not a best-in-class value for every large Rust workspace. Size L0 based on the **working set**, not total repository size.

Starting heuristics:

| Workload | Practical initial L0 |
|---|---:|
| small personal Rust projects | 10 GiB |
| medium workspace / several profiles | 20–30 GiB |
| large dependency graph / many worktrees | 30–50 GiB |
| large multi-target / multi-profile workstation with ample SSD | 50–100 GiB |

These are recommendations, not upstream defaults. Measure eviction behavior and hit rate before allocating more disk.

## 13.4 Storage device

Prefer:

```text
local NVMe / fast SSD
```

over:

```text
network filesystem
slow external disk
cloud-synced user folder
```

A cache hit is only valuable if reading/decompressing/restoring the artifact is cheaper than compiling it.

## 13.5 One local server per local cache

Upstream warns that local storage supports only one sccache server at a time; concurrent servers sharing one local cache directory can race and cause spurious failures.

Therefore:

```text
one user/session cache directory
↔ one logical sccache server
```

If isolation requires multiple servers, give them separate disk cache directories.

## 13.6 Read-only local mode

```bash
export SCCACHE_LOCAL_RW_MODE=READ_ONLY
```

This is useful for immutable pre-seeded images or forensic reproduction. It is not useful on an empty cache because it only adds lookup overhead without ever populating entries.

---

# 14) Multi-level cache architecture

## 14.1 Why multi-level is the best general topology

sccache 0.17 supports hierarchical storage. A canonical Rust team topology is:

```text
L0  local disk        fastest, private hot set
 │
 ▼ miss
L1  regional Redis    low-latency shared team set
 │
 ▼ miss
L2  S3 / object store larger durable shared set
```

or for GitHub-hosted CI:

```text
L0  ephemeral runner disk
 │
 ▼
L1  GitHub Actions cache backend
```

## 14.2 Read path

Backends are checked from fastest to slowest. On a hit in a slower level, sccache automatically backfills faster levels asynchronously.

Consequences:

- remote history can warm local disk automatically;
- subsequent reads avoid repeated network latency;
- the order of levels is performance-critical.

## 14.3 Write path

Writes go to configured read/write levels in parallel. Read-only levels are skipped.

## 14.4 Configuration

Environment:

```bash
export SCCACHE_MULTILEVEL_CHAIN="disk,redis,s3"
export SCCACHE_MULTILEVEL_WRITE_ERROR_POLICY="l0"
```

File:

```toml
[cache.multilevel]
chain = ["disk", "redis", "s3"]
write_error_policy = "l0"
```

Valid 0.17 level names are:

```text
disk
redis
memcached
s3
gcs
azure
gha
webdav
oss
cos
```

## 14.5 Ordering rule

Always order expected latency fastest → slowest:

```text
good:
  disk,redis,s3

good:
  disk,gha

usually bad:
  s3,disk
  gha,disk
```

Every miss at an early slow tier delays access to later tiers.

## 14.6 Write error policy

sccache 0.17 supports:

| Policy | Behavior | Recommended use |
|---|---|---|
| `ignore` | write errors never fail compilation | cache is strictly best-effort |
| `l0` | fail only when first read/write level fails | **default and general recommendation** |
| `all` | any read/write-level write failure can fail operation | unusual strict environments |

For most development and CI:

```toml
write_error_policy = "l0"
```

This preserves a healthy local cache while treating remote storage as an accelerator rather than a build-availability dependency.

## 14.7 0.17.0 validation caveat for read-only/multi-level combinations

Multi-level caching is relatively new and should receive backend-combination acceptance tests. On the reference date:

- issue #2796 is open for client-side `disk,webdav` behavior where L0 was not populated from the slower read-only level;
- issue #2773 remains open after reporting `disk,s3` behavior with read-only S3 where the local disk cache was not populated in the tested 0.16 setup.

These reports are backend/mode-specific and should not be generalized into “multi-level is broken.” They *do* justify a mandatory acceptance test before calling a topology production-ready:

```text
cold L0 + warm L1/L2 → remote hit
remote hit → correct artifact
backfill → L0 becomes populated
second request → L0 hit
read-only remote → local RW behavior remains correct
remote outage → build still succeeds by compilation
```

## 14.8 Best-in-class rule

> A remote compilation cache should normally be **performance infrastructure, not build correctness infrastructure**.

Builds should remain correct when remote caching is temporarily unavailable. `l0` or, in some CI systems, `ignore`, expresses that principle better than `all`.

---

# 15) Redis as a low-latency shared L1

## 15.1 When Redis is the strongest remote tier

Redis is compelling when:

- developers/CI are on a low-latency network to the service;
- many machines repeatedly build overlapping Rust graphs;
- sub-second remote access matters;
- the team can operate memory-bounded cache infrastructure;
- the cache is intentionally ephemeral rather than archival.

Recommended topology:

```text
workstation / runner disk
  → regional Redis
  → optional object storage
```

## 15.2 Endpoint forms

Single node:

```bash
export SCCACHE_REDIS_ENDPOINT="rediss://cache.internal.example:6379"
```

Cluster:

```bash
export SCCACHE_REDIS_CLUSTER_ENDPOINTS="rediss://r1:6379,rediss://r2:6379,rediss://r3:6379"
```

Use one or the other, not both.

## 15.3 Credentials

Prefer separate credential variables rather than deprecated credential-bearing `SCCACHE_REDIS` URLs:

```bash
export SCCACHE_REDIS_USERNAME="$CACHE_USER"
export SCCACHE_REDIS_PASSWORD="$CACHE_PASSWORD"
```

The old `SCCACHE_REDIS` variable is deprecated for security reasons.

## 15.4 TLS

Use `rediss://` across networks that are not already protected as a trusted private transport. Avoid the documented `#insecure` TLS escape hatch except in controlled diagnosis; it disables hostname verification and SNI.

## 15.5 Memory policy

Upstream specifically recommends configuring Redis with a bounded `maxmemory` and an eviction policy suited to cache data such as:

```text
allkeys-lru
```

Do not deploy a shared Redis sccache backend with unbounded memory growth.

## 15.6 Expiration

Environment:

```bash
export SCCACHE_REDIS_EXPIRATION="1209600"  # example: 14 days
```

The default Redis behavior in sccache is no expiration unless configured. `SCCACHE_REDIS_TTL` is a deprecated synonym.

A TTL is useful when:

- toolchains change frequently;
- storage is constrained;
- old branches have low reuse value;
- Redis eviction alone does not provide the desired lifecycle.

Do not choose a TTL shorter than the reuse horizon you are trying to capture.

## 15.7 Key prefix

```bash
export SCCACHE_REDIS_KEY_PREFIX="rust/team-a/project-x/"
```

Use prefixes for:

- sharing one Redis service between applications;
- trust-domain separation;
- operational deletion/rotation;
- project ownership clarity.

Do not assume a prefix itself is a security boundary; access controls still matter.

## 15.8 Read-only mode

```bash
export SCCACHE_REDIS_RW_MODE=READ_ONLY
```

This is valuable for untrusted or lower-trust CI contexts consuming a trusted cache without gaining poisoning capability.

## 15.9 Canonical Redis config

```toml
client_side_mode = true

[cache.multilevel]
chain = ["disk", "redis"]
write_error_policy = "l0"

[cache.disk]
dir = "/var/cache/sccache"
size = 21474836480  # 20 GiB

[cache.redis]
endpoint = "rediss://cache.internal.example:6379"
# Credentials should normally come from environment / secret injection.
db = 0
expiration = 1209600
key_prefix = "rust/project-x/"
```

Keep secrets out of a committed config file.

---

# 16) S3 and Cloudflare R2 as durable remote tiers

## 16.1 When object storage is the right backend

S3-compatible storage is strongest when:

- the cache should survive runner/workstation churn;
- a large shared artifact set matters more than minimum lookup latency;
- object storage is already governed and credentialed;
- global or multi-region teams need a durable origin tier;
- Redis memory cost would be excessive.

Object storage is often best as **L2 behind disk or Redis**, not as the first lookup tier on a developer workstation.

## 16.2 AWS S3 minimum configuration

```bash
export SCCACHE_BUCKET="company-rust-sccache"
export SCCACHE_REGION="us-east-1"
export SCCACHE_S3_USE_SSL="true"
export SCCACHE_S3_KEY_PREFIX="project-x/"
```

## 16.3 Credentials

sccache supports normal AWS-style sources, including:

- `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY`;
- AWS profile configuration;
- instance metadata credentials;
- AssumeRole;
- web-identity role credentials.

For CI, prefer short-lived identity federation / web identity over long-lived static access keys where the platform supports it.

## 16.4 Public/read-only access

`SCCACHE_S3_NO_CREDENTIALS` can enable public read-only object access. Upstream explicitly notes this can be useful for pull requests that cannot safely receive credentials.

This pattern can be strong for public projects:

```text
trusted main/release jobs
  authenticated READ_WRITE

untrusted fork PRs
  unauthenticated READ_ONLY
```

provided the bucket policy truly prevents writes without credentials.

## 16.5 Server-side encryption

0.17 configuration supports:

```toml
[cache.s3]
server_side_encryption = true
# OR AWS-managed KMS:
server_side_encryption_aws_kms = true
# OR customer-managed KMS key:
server_side_encryption_kms_key_id = "arn:aws:kms:..."
```

Use encryption according to organizational policy. A compilation cache can contain proprietary compiled code and metadata; it should not be treated as non-sensitive merely because it is “only a cache.”

## 16.6 Cloudflare R2

R2 uses the S3-compatible backend. Upstream requires/recommends the equivalent of:

```bash
export SCCACHE_BUCKET="my-r2-bucket"
export SCCACHE_ENDPOINT="https://ACCOUNT_ID.r2.cloudflarestorage.com"
export SCCACHE_REGION="auto"
```

Use TLS.

## 16.7 Canonical disk + S3 config

Storage configuration:

```toml
[cache.multilevel]
chain = ["disk", "s3"]
write_error_policy = "l0"

[cache.disk]
dir = "/var/cache/sccache"
size = 32212254720  # 30 GiB

[cache.s3]
bucket = "company-rust-sccache"
use_ssl = true
key_prefix = "project-x/"
server_side_encryption = true
```

Region/credentials can be injected through the environment so the same file remains portable.

Choose client-side versus server-side mode **after** validating this exact multi-level topology. Do not bake `client_side_mode = true` into the shared storage configuration merely because upstream recommends the architecture in general; 0.17.0 is the first client-side release and current multi-level reports justify explicit backfill testing.

## 16.8 Latency rule

Before adopting S3 as a remote tier, benchmark:

```text
remote cache-hit latency
vs
local compile time of common units
```

If many small Rust crates compile faster than object retrieval, a remote hit can be a regression. A disk/Redis tier in front mitigates this.

---

# 17) GitHub Actions cache backend

## 17.1 Native sccache GHA backend

sccache can use GitHub Actions cache storage. The relevant runtime token/URL normally come from the Actions environment.

Core Rust variables:

```bash
RUSTC_WRAPPER=sccache
CARGO_INCREMENTAL=0
SCCACHE_GHA_ENABLED=true
```

The dedicated Mozilla sccache action documents the same Rust wrapper + GHA enablement pattern.

## 17.2 Action version

As of this reference date, `mozilla-actions/sccache-action` **v0.0.11** is the latest release, published 2026-07-29. For reproducible CI, pin the action version and, where desired, pin the sccache version it installs.

Example:

```yaml
- name: Install/configure sccache
  uses: mozilla-actions/sccache-action@v0.0.11
  with:
    version: "v0.17.0"
```

The action also provides a post-run statistics step by default.

## 17.3 Best-in-class GHA topology

Use local ephemeral disk as L0 and GHA as L1:

```bash
export SCCACHE_MULTILEVEL_CHAIN="disk,gha"
export SCCACHE_DIR="$RUNNER_TEMP/sccache"
export SCCACHE_CACHE_SIZE="10G"
```

Why keep disk on an ephemeral runner?

- repeated compiler units within the job can hit local storage;
- multi-level design is intended to backfill L0 from slower hits; verify that behavior with the pinned 0.17.0 mode/backend combination.
- remote traffic is reduced;
- disk disappears after the job, which is acceptable because GHA is the cross-job tier.

## 17.3.1 0.17.0 acceptance requirement

For `disk,gha`, validate:

```text
cold disk + warm GHA → GHA hit
hit returns correct artifact
disk becomes populated
second identical cold-target build → disk hit where applicable
GHA rate limit/outage → compilation still succeeds
```

Repeat with client-side mode enabled if that is the intended production setting.

## 17.4 Cache namespace / rotation

The GHA backend exposes cache-to-cache-from semantics and documentation supports a version control used to purge or rotate cache history. Treat the namespace/version as an operational lever for:

- deliberate global invalidation;
- trust-domain changes;
- major toolchain/platform changes if you want explicit segregation;
- recovery from suspected poisoning/corruption.

Do not rotate it on every commit; that destroys reuse.

## 17.5 Rate limits

Upstream notes that if the GitHub Actions cache service rate-limits sccache, the build continues but storage may not occur. That is the correct failure posture for performance infrastructure.

## 17.6 PR write policy

For untrusted fork pull requests, use a read-only backend or a namespace that is never consumed by trusted release jobs.

Conceptual policy:

```yaml
# trusted branch
SCCACHE_GHA_RW_MODE: READ_WRITE

# untrusted PR
SCCACHE_GHA_RW_MODE: READ_ONLY
```

Exact GitHub permission/token behavior must also be verified; environment-variable intent cannot grant permissions the platform does not provide.

---

# 18) Secondary storage backends

sccache 0.17 also supports GCS, Azure Blob Storage, Memcached, WebDAV, Alibaba OSS, and Tencent COS. These can participate in multi-level chains.

## 18.1 Decision matrix

| Backend | Best fit | Main caution |
|---|---|---|
| GCS | GCP-native organization | credential and read/write policy |
| Azure Blob | Azure-native organization | identity/connection-string policy |
| Memcached | simple low-latency ephemeral shared tier | weaker durability and cache-management semantics than Redis |
| WebDAV | existing generic HTTP/WebDAV cache service | latency/auth/TLS quality varies widely |
| Alibaba OSS | Alibaba Cloud-native deployment | credential and regional architecture |
| Tencent COS | Tencent Cloud-native deployment | credential and regional architecture |

For a greenfield Rust build platform, this guide generally prefers:

```text
Redis for low-latency shared L1
S3-compatible object storage for durable L2
GHA for GitHub-hosted CI convenience
```

unless cloud/platform alignment makes another backend materially simpler.

## 18.2 Environment-variable surface

### GCS

```text
SCCACHE_GCS_BUCKET
SCCACHE_GCS_CREDENTIALS_URL
SCCACHE_GCS_KEY_PATH
SCCACHE_GCS_RW_MODE
```

### Azure

```text
SCCACHE_AZURE_CONNECTION_STRING
SCCACHE_AZURE_BLOB_CONTAINER
SCCACHE_AZURE_KEY_PREFIX
SCCACHE_AZURE_RW_MODE
```

The released 0.17.0 Azure backend documents connection-string authentication. Do not assume identity mechanisms added on the development branch are available when the deployment is pinned to 0.17.0.

### Memcached

```text
SCCACHE_MEMCACHED_ENDPOINT
SCCACHE_MEMCACHED_USERNAME
SCCACHE_MEMCACHED_PASSWORD
SCCACHE_MEMCACHED_EXPIRATION
SCCACHE_MEMCACHED_KEY_PREFIX
SCCACHE_MEMCACHED_RW_MODE
```

`SCCACHE_MEMCACHED` is deprecated as an alias for the endpoint.

### WebDAV

```text
SCCACHE_WEBDAV_ENDPOINT
SCCACHE_WEBDAV_KEY_PREFIX
SCCACHE_WEBDAV_USERNAME
SCCACHE_WEBDAV_PASSWORD
SCCACHE_WEBDAV_TOKEN
SCCACHE_WEBDAV_RW_MODE
```

### Alibaba OSS

```text
SCCACHE_OSS_BUCKET
SCCACHE_OSS_ENDPOINT
SCCACHE_OSS_KEY_PREFIX
ALIBABA_CLOUD_ACCESS_KEY_ID
ALIBABA_CLOUD_ACCESS_KEY_SECRET
SCCACHE_OSS_NO_CREDENTIALS
SCCACHE_OSS_RW_MODE
```

### Tencent COS

```text
SCCACHE_COS_BUCKET
SCCACHE_COS_ENDPOINT
SCCACHE_COS_KEY_PREFIX
TENCENTCLOUD_SECRET_ID
TENCENTCLOUD_SECRET_KEY
SCCACHE_COS_RW_MODE
```

---

# 19) Compression strategy

## 19.1 zstd level

sccache exposes:

```bash
SCCACHE_CACHE_ZSTD_LEVEL=<1..22>
```

Default: **3**.

Upstream's own example notes that level 10 produced roughly 0.9× the size at roughly 1.6× the compression time versus level 3 in its test. The exact economics depend on artifact mix and hardware.

## 19.2 Best-in-class default

Keep level **3** unless measurement shows network/storage cost dominates CPU cost.

Why:

- compile caches are latency-sensitive;
- many entries are written once and may not be read enough to amortize expensive compression;
- local SSD capacity is usually cheaper than developer/CI CPU latency;
- multi-level storage already keeps a hot local tier.

## 19.3 When to increase compression

Consider a higher level when:

- remote egress is expensive;
- bandwidth is constrained;
- L2 object storage size is a meaningful cost;
- cache writes happen on compute-rich machines and reads dominate;
- benchmarked end-to-end wall time improves.

Do not tune based on compressed size alone.

## 19.4 Existing entries do not change

Upstream notes the compression setting affects newly compressed cache entries. Changing the level does not retroactively recompress existing entries. A full effect requires cache regeneration.

---

# 20) Cache sizing and retention as a working-set problem

## 20.1 Define the reuse horizon

Ask:

```text
How far back do builds commonly revisit identical compiler units?
```

Examples:

- minutes/hours: rapid agent or developer iteration;
- days: PR and branch switching;
- weeks: long-lived release branches;
- months: infrequent maintenance builds.

The right retention is the smallest one that captures economically useful reuse.

## 20.2 L0 sizing

L0 should fit the hot working set:

```text
current toolchain
× common target(s)
× common profiles
× active feature families
× active branches/worktrees
```

If L0 is constantly evicting entries that are reused minutes later, increase it.

## 20.3 Remote sizing

Remote storage should not be an unbounded graveyard. Use one or more of:

- Redis `maxmemory` + LRU;
- Redis expiration;
- object-store lifecycle policies;
- GHA cache lifecycle semantics;
- namespace retirement after toolchain/project migration.

## 20.4 Toolchain churn

A new Rust toolchain can naturally create a largely distinct working set. Frequent nightly movement can therefore multiply storage pressure. If nightly builds matter, treat them as a separate expected working-set class rather than assuming stable sharing with stable Rust.

## 20.5 Do not size from hit rate alone

A 95% hit rate might be excellent or meaningless depending on what the 5% misses cost. Track wall-time value:

```text
saved compiler seconds per cache byte / per remote request
```

Large, expensive library crates deserve retention more than tiny crates that compile in milliseconds, even though the cache does not make that policy distinction itself.

---

# 21) Remote-cache security and trust model

## 21.1 A Rust compilation cache is a supply-chain boundary

Cached Rust outputs are not inert metadata. They are object/library artifacts that can later be linked into executables or loaded into build processes. A maliciously substituted cache artifact can therefore become a software-supply-chain compromise.

Treat a remote cache with the same seriousness as other build infrastructure that can influence produced binaries.

## 21.2 Core trust invariant

```text
A job may write to a cache only if every downstream consumer is willing to trust
that job as a producer of compiler artifacts.
```

This is stronger than “the job can read the source repository.”

## 21.3 Trust-domain patterns

### Strongest shared-team pattern

```text
trusted developer machines / trusted CI
  READ_WRITE → trusted shared cache

untrusted forks / external PRs
  READ_ONLY  → trusted shared cache
```

### Isolation pattern

```text
trusted main/release namespace
  completely separate from
untrusted PR namespace
```

### Public project pattern

```text
public readable cache
  produced only by trusted CI

fork PRs
  anonymous/read-only
```

## 21.4 Do not rely on cache keys as an authorization system

Content-addressing detects identity according to the cache algorithm. It does not authenticate who produced an entry. Authentication, bucket/Redis permissions, CI token policy, and network controls provide the trust boundary.

## 21.5 Secrets

Do not commit:

- Redis passwords;
- cloud access keys;
- WebDAV tokens;
- shared-key connection strings;
- scheduler authentication tokens.

Prefer:

- CI secret stores;
- workload identity / OIDC;
- instance/workload roles;
- managed identity;
- short-lived credentials.

## 21.6 TLS

Use TLS for remote caches unless a tightly controlled private transport provides equivalent protection. This is particularly important because caches can contain proprietary compiled code and credentials may travel with requests.

## 21.7 Namespace separation

A key prefix or cache namespace is useful for operational separation:

```text
company/rust/project-a/
company/rust/project-b/
```

but it is not a substitute for authorization. If one credential can overwrite both prefixes, the trust domains are not isolated.

## 21.8 Release builds

For high-assurance release production, choose one of these explicitly:

```text
A. trusted read-only cache produced by equally trusted build infrastructure
B. isolated release cache
C. cache bypass / clean compile for the final reproducibility attestation
```

The strongest answer depends on the release threat model. Do not let an ordinary convenience cache silently become an unreviewed release input.

---

# 22) CI/CD design patterns

## 22.1 CI should optimize cache reuse without making cache availability a build dependency

Canonical Rust build environment:

```bash
export RUSTC_WRAPPER=sccache
export CARGO_INCREMENTAL=0
export SCCACHE_MULTILEVEL_WRITE_ERROR_POLICY=l0
```

For sccache 0.17.0, add `SCCACHE_CLIENT_SIDE=1` **only after** validating the selected local/remote topology. Then configure the platform's storage backend.

Do not add `SCCACHE_BASEDIRS` expecting Rust path normalization in 0.17.0.

## 22.2 Canonical absolute checkout paths are the current Rust portability lever

Equivalent CI jobs should use the same checkout root wherever feasible:

```text
job A → /workspace/repo
job B → /workspace/repo
job C → /workspace/repo
```

Avoid randomized build roots if shared Rust cache reuse is a design goal. Also stabilize:

- `CARGO_HOME` location;
- rustup/toolchain layout;
- target triple;
- profile/features;
- semantic `RUSTFLAGS`.

This is not aesthetic filesystem tidiness; it directly affects the released Rust cache identity.

## 22.3 Job-local disk plus shared remote

```text
job
 ├─ ephemeral local disk L0
 └─ shared remote L1/L2
```

This can outperform remote-only when the job reuses the same compiler result more than once. However, validate L0 backfill for the exact 0.17.0 execution mode/backend combination before standardizing it.

## 22.4 Do not cache Cargo `target/` blindly just because sccache exists

Cargo target caching and sccache solve overlapping but different problems. A CI system may use:

```text
sccache only
Cargo target cache only
both
```

Benchmark the combination. Large archived target directories can have restore/upload costs, stale-state complexity, and platform coupling that outweigh their benefit.

## 22.5 Matrix design

A CI matrix naturally creates distinct compiler keys for changes such as:

- Rust toolchain;
- target triple;
- profile;
- feature set;
- `RUSTFLAGS`;
- coverage/profiling instrumentation.

Do not try to force those semantically different jobs into one artifact identity. Shared storage can host them together because sccache's compiler key distinguishes the inputs.

## 22.6 Untrusted PRs

Never grant fork-controlled code privileged write credentials merely to improve cache hit rate. Prefer:

```text
trusted branches/internal PRs
  shared read + write

untrusted fork PRs
  shared read-only if the backend/platform safely supports it
  OR isolated cache
  OR no shared remote cache
```

A key prefix is an operational namespace; it is not an authorization boundary by itself.

## 22.7 Trusted mainline

Trusted mainline/release jobs are the natural writers that populate shared cache history for later readers. If a dedicated warmer exists, it should compile **representative build workloads** such as `cargo build`, not `cargo check` under the assumption that check results will populate the Rust compiler cache.

## 22.8 Failure posture

A cache outage should normally produce:

```text
cache miss / backend warning
→ real rustc compilation
→ valid build result
```

not:

```text
remote cache unavailable
→ build correctness failure
```

This is why `l0`/best-effort remote semantics are generally appropriate.

---

# 23) Observability and cache effectiveness

## 23.1 Core commands

```bash
sccache --show-stats
sccache --show-adv-stats
sccache --show-stats --stats-format=json
sccache --show-adv-stats --stats-format=json
sccache --zero-stats
```

0.17 supports `text` and `json` stats formats.

Server lifecycle:

```bash
sccache --start-server
sccache --stop-server
```

Distributed-only:

```bash
sccache --dist-status
```

## 23.2 What to measure

At minimum capture:

```text
compile requests
cache hits
cache misses
non-cacheable compilations/calls
non-cacheable reasons
cache read errors
cache write errors
cache timeouts
forced recaches
compilation failures
average cache read hit latency
average cache read miss latency
average cache write latency
compiler time
cache size / maximum size
cache location/type
```

Exact displayed fields can evolve; JSON consumers should be version-aware.

## 23.3 Hit-rate formulas

A useful basic rate is:

```text
cache hit rate among cache lookups
= hits / (hits + misses)
```

But also compute:

```text
cacheable fraction
= (hits + misses) / relevant compiler requests
```

A 99% hit rate over only 30% cacheable work can still leave most wall time untouched.

## 23.4 Time-weighted effectiveness

The most meaningful metric is approximate compiler time avoided:

```text
sum(estimated compile time of hit units)
- cache lookup/restore overhead
```

A tiny crate hit and a 60-second crate hit should not be valued equally merely because both increment the same counter.

## 23.5 Build-class baselines

Maintain separate expectations for:

```text
warm same-branch rebuild
branch/worktree revisit
clean target rebuild
fresh CI runner
release build
feature-matrix build
cross-target build
```

Combining them into one global hit-rate number hides the optimization target.

## 23.6 Zeroing statistics for experiments

Before a controlled experiment:

```bash
sccache --zero-stats
```

Run exactly one build class, then capture JSON:

```bash
sccache --show-adv-stats --stats-format=json > sccache-stats.json
```

Preserve:

```text
sccache version
rustc -vV
Cargo version
command
profile
features
target
RUSTFLAGS
CARGO_INCREMENTAL
cache topology
source revision
whether target/ was warm or cold
```

Without those, the hit rate is difficult to reproduce.

---

# 24) Failure taxonomy and debugging

## 24.1 First classify the symptom

```text
A. wrapper not invoked
B. invocation non-cacheable
C. cacheable but always misses
D. cache hits but build still slow
E. cache read/write errors
F. suspected stale/corrupt artifact
G. server/config startup failure
H. remote backend latency or outage
```

Each requires a different investigation.

## 24.2 Wrapper not invoked

Check:

```bash
cargo -V
which sccache
sccache --version
printf '%s\n' "$RUSTC_WRAPPER"
cargo --config 'build.rustc-wrapper="sccache"' check
```

Also inspect layered Cargo config; a repository, parent directory, environment variable, or command-line config may override another setting.

## 24.3 Non-cacheable Rust units

Use advanced stats and identify whether the units correspond to expected boundaries:

```text
bin / dylib / cdylib / proc-macro
incremental compilation
unsupported rustc invocation shape
special compiler argument
```

Do not attempt to “fix” expected final-link non-cacheability.

## 24.4 Cacheable but always misses

Investigate differences in:

```text
rustc/toolchain
source bytes
absolute checkout/toolchain roots
profile
features
target
RUSTFLAGS
build-script outputs
compile-time env dependencies
generated source
sccache version/cache-key version
```

The correct question is not “why does the repository look the same?” but “which compiler input differs?”

## 24.5 Hits but build is still slow

Break wall time into:

- Cargo graph/work scheduling;
- sccache lookup/restore;
- cache misses;
- non-cacheable proc macros/build scripts;
- final linking;
- tests or runtime work;
- build-script execution;
- remote cache latency.

High sccache hit rate is not a guarantee of low total build time.

## 24.6 Logging

Client-side-friendly diagnostic:

```bash
SCCACHE_LOG=debug SCCACHE_LOG_MILLIS=1 cargo build
```

Trace only when necessary because volume can be high:

```bash
SCCACHE_LOG=trace SCCACHE_LOG_MILLIS=1 cargo build
```

Server-file logging:

```bash
SCCACHE_ERROR_LOG=/tmp/sccache.log SCCACHE_LOG=debug cargo build
```

Remember: in 0.17, `SCCACHE_ERROR_LOG` is incompatible with client-side mode and forces server-side behavior.

## 24.7 Foreground server diagnosis

When startup/backend configuration is failing, run the server without daemonizing so errors are visible:

```bash
SCCACHE_START_SERVER=1 SCCACHE_NO_DAEMON=1 SCCACHE_LOG=debug sccache
```

Use a separate shell for the build if needed.

## 24.8 Force recache

When cache contents are suspected to be broken, sccache supports:

```bash
SCCACHE_RECACHE=1 cargo build
```

Use this to overwrite/repopulate results. It is a diagnostic/recovery mechanism, not a normal build setting.

## 24.9 Full bypass

For a correctness oracle:

```bash
RUSTC_WRAPPER= CARGO_INCREMENTAL=0 cargo build
```

Compare outputs/tests against the cached build under equivalent compiler inputs.

## 24.10 Corruption response

If a shared cache is suspected of returning bad artifacts:

1. stop writes from untrusted/unknown producers;
2. capture sccache/toolchain/config versions;
3. reproduce with cache bypass;
4. reproduce with `SCCACHE_RECACHE=1` into an isolated namespace if possible;
5. rotate or purge the suspect namespace;
6. audit credential/write access;
7. add a regression test for the hidden input or poisoning path.

Do not merely delete one local target directory and declare the issue resolved.

---

# 25) Distributed compilation for Rust

## 25.1 What it is

sccache can distribute cache misses to build servers through a scheduler. This is distinct from remote **storage**:

```text
remote cache
  avoids compilation when an artifact already exists

distributed compilation
  executes a cache miss on another machine
```

They solve different problems.

## 25.2 Infrastructure surface

The upstream distributed system includes:

```text
client sccache
scheduler
build servers
compiler/toolchain packaging
network authentication
sandboxed remote execution
```

The scheduler is currently Linux-hosted. Build servers are primarily Linux; upstream also documents FreeBSD. macOS and Windows clients are supported but less heavily tested in the quickstart, and non-Linux client/toolchain cases require more explicit toolchain packaging.

## 25.3 Client-side mode interaction

A configured distributed scheduler is mutually exclusive with 0.17 client-side mode. Distributed compilation therefore keeps the server-side architecture.

## 25.4 Best-in-class recommendation

For Rust, adopt distributed compilation **after** optimizing:

1. cacheability and `CARGO_INCREMENTAL=0` for the shared-reuse workflow;
2. local disk cache;
3. low-latency shared remote cache;
4. checkout/toolchain path stability;
5. linker performance;
6. Cargo graph/features and unnecessary rebuilds;
7. ordinary local/CI CPU parallelism.

Then consider distributed compilation if **cache misses remain the dominant wall-time cost** and the organization can justify operating remote execution infrastructure.

## 25.5 Why it is not the default

Distributed Rust compilation adds:

- scheduler/build-server availability;
- toolchain packaging and compatibility;
- sandbox/runtime administration;
- root/bubblewrap operational requirements in the documented Linux builder setup;
- network latency;
- authentication/authorization;
- a larger attack surface;
- troubleshooting complexity;
- loss of 0.17 client-side mode.

For many teams, `disk → Redis → S3` captures most of the recurring compile savings with much less operational complexity.

## 25.6 Where it becomes compelling

Distributed compilation can make sense for:

- very large Rust graphs with expensive frequent misses;
- centralized high-core build farms;
- large organizations already operating secure build infrastructure;
- fleets where developer machines are intentionally lightweight;
- CI where miss latency materially gates throughput.

## 25.7 Validation

Use:

```bash
sccache --dist-status
```

and track:

```text
remote compilations
failed distributed compilations
fallback behavior
scheduler saturation
server CPU utilization
toolchain packaging time
network transfer time
```

Do not infer distribution merely from the scheduler being reachable.

---

# 26) Rollout strategy

## 26.1 Phase 1 — establish a local Rust build baseline

1. Pin sccache 0.17.0.
2. Configure `rustc-wrapper = "sccache"`.
3. Use a **`cargo build`** workload for cache validation.
4. Run with `CARGO_INCREMENTAL=0` for the shared-reuse experiment.
5. Measure cold target + empty sccache, then cold target + warm sccache.
6. Inspect advanced stats and non-cacheable reasons.

Do not begin with remote infrastructure.

## 26.2 Phase 2 — establish path stability

Before blaming cache storage, make equivalent jobs use equivalent absolute layouts:

```text
checkout root
CARGO_HOME
rustup/toolchain path
relevant generated-source paths
```

For 0.17.0 Rust, **do not use `SCCACHE_BASEDIRS` as a substitute for this step.**

## 26.3 Phase 3 — evaluate client-side mode

A/B test server-side and `SCCACHE_CLIENT_SIDE=1` using the representative `cargo build` workload.

For multi-level storage, add explicit tests for:

- remote hit with empty local L0;
- L0 backfill;
- second-request L0 hit;
- read-only remote behavior.

## 26.4 Phase 4 — add shared remote storage

Choose the simplest backend that solves the measured reuse problem:

```text
low-latency team network → Redis
fleet/CI durability       → S3/R2/GCS/Azure
GitHub Actions            → GHA backend
```

Start with remote cache as an accelerator, not a build availability dependency.

## 26.5 Phase 5 — enforce trust policy

Separate trusted writers from untrusted consumers before broad rollout. Validate credential absence, read-only behavior, namespace policy, and cache outage fallback.

## 26.6 Phase 6 — tune economics

Only after correctness and reuse are proven should you tune:

- disk size;
- remote retention/TTL;
- compression;
- Redis memory policy;
- storage hierarchy;
- client-side mode selection;
- statistics/telemetry retention.

## 26.7 Phase 7 — evaluate distributed compilation only if justified

Distributed compilation is a separate infrastructure decision. Consider it only when **cache misses themselves** remain a dominant wall-time cost after ordinary caching and linker/build-graph optimization.

---

# 27) Benchmarking playbook

## 27.1 Benchmark objective

The objective is not “maximize hit rate.” It is:

> Minimize developer/CI wall time and infrastructure cost while preserving build correctness and trust.

## 27.2 Required build scenarios

For a serious Rust workspace, benchmark at least:

| Scenario | Why |
|---|---|
| `cargo build`, cold target + empty sccache | true compilation baseline |
| `cargo build`, cold target + warm local sccache | maximum local-cache value |
| `cargo build`, empty L0 + warm remote | remote-cache value |
| `cargo build`, warm target no-op | Cargo's own best case |
| one-line leaf-library edit | local edit behavior |
| one-line high-fanout core-library edit | invalidation cost |
| switch to a previously built revision **at same absolute checkout path** | historical reuse without path confounder |
| same revision under a different checkout root | quantify the known 0.17 Rust path-sensitivity penalty |
| changed feature set | semantic cache split |
| changed Rust toolchain | toolchain split |
| release build | optimized-profile behavior and link floor |
| `cargo test --no-run` | representative test compilation with mixed cacheable/non-cacheable units |

## 27.3 Benchmark `cargo check` separately—and not as an sccache hit test

`cargo check` answers a different question. Because normal check-mode Rust library invocations omit `link`, it should be benchmarked as an edit-feedback workflow:

```bash
# normal/check-oriented mode; sccache 0.17 rejects incremental Rust invocations
RUSTC_WRAPPER= cargo check --workspace
```

Do not interpret a lack of sccache hits during ordinary `cargo check` as a storage failure.

## 27.4 Compare incremental build mode separately

For **the same `cargo build` command**:

```bash
# sccache shared-reuse mode
RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 cargo build --workspace

# rustc incremental hot-build mode
RUSTC_WRAPPER= CARGO_INCREMENTAL=1 cargo build --workspace
```

That is the apples-to-apples comparison for changed workspace crates.

## 27.5 Replication and cache-state notation

Use multiple runs. Clear only the layer relevant to the experiment:

```text
clear Cargo target only
clear sccache L0 only
rotate/isolated remote namespace only
clear both only for true cold baseline
```

Every result should name both states:

```text
Cargo target: cold | warm | incremental-warm
sccache L0:    empty | warm
remote:        empty | warm | disabled
checkout path: exact absolute path
```

“Clean build” is otherwise ambiguous.

## 27.6 Metrics per run

Capture:

```text
wall time
user CPU
system CPU
peak memory if practical
cache hits/misses/non-cacheable reasons
cache read/write latency
network bytes/requests if available
link time if material
total compiler time
```

A count-weighted hit rate can be misleading; an expensive 20-second crate is worth more than many 100-ms hits.

## 27.7 Hyperfine-style methodology

A local warm-sccache / cold-Cargo-target benchmark:

```bash
hyperfine \
  --prepare 'rm -rf target && sccache --zero-stats' \
  'RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 cargo build --workspace'
```

This intentionally leaves sccache storage warm. Label it exactly that way.

For a true cold baseline, isolate or clear the sccache storage as a separate setup step; do not conflate the two experiments.

---

# 28) Best-in-class profile: standalone Rust workstation

## 28.0 Objective

Optimize one developer machine for:

- repeat `cargo build` after target invalidation;
- branch switching in the same checkout path;
- dependency-heavy workspaces;
- occasional alternate target directories;
- low operational complexity;
- fast `cargo check` feedback without forcing a cache-oriented non-incremental policy onto it.

## 28.1 Recommended Cargo integration

Use the wrapper globally, but keep incremental policy workflow-specific:

```toml
# ~/.cargo/config.toml or repository .cargo/config.toml
[build]
rustc-wrapper = "sccache"
```

Do **not** automatically add `incremental = false` globally unless the team has measured and consciously accepted the effect on interactive checking.

Shared-reuse build:

```bash
CARGO_INCREMENTAL=0 cargo build --workspace
```

Interactive check:

```bash
RUSTC_WRAPPER= cargo check --workspace
```

## 28.2 Recommended local sccache environment

Start with:

```bash
export SCCACHE_CACHE_SIZE=30G
export SCCACHE_CACHE_ZSTD_LEVEL=3
```

On Linux/macOS, leaving `SCCACHE_DIR` at the platform default is usually preferable unless the default home/cache location is not on fast local storage.

For client-side mode:

```bash
export SCCACHE_CLIENT_SIDE=1
```

is the **upstream-recommended 0.17 architecture** and the default candidate for a simple local disk cache. Keep it only after a one-time A/B benchmark and cache-population validation on the actual machine.

## 28.3 Why 30 GiB rather than the upstream 10 GiB default?

`10G` is a safe general default, not a Rust-specific optimum. Large Rust workspaces can produce mutually incompatible artifact sets across:

- toolchain versions;
- profiles;
- target triples;
- feature combinations;
- `RUSTFLAGS` variants;
- branches/revisions.

A **20–50 GiB** local cache is a reasonable initial engineering range when SSD space allows. This is a recommendation, not an upstream default. Size it from eviction/reuse evidence.

## 28.4 Acceptance test

```bash
sccache --version
sccache --stop-server || true
sccache --start-server

# First: cold Cargo target + initially cold/partially cold sccache.
sccache --zero-stats
rm -rf target
CARGO_INCREMENTAL=0 cargo build --workspace
sccache --show-adv-stats

# Second: cold Cargo target + warm sccache.
sccache --zero-stats
rm -rf target
CARGO_INCREMENTAL=0 cargo build --workspace
sccache --show-adv-stats
```

The second run is the basic proof that sccache—not Cargo's `target/` tree—is returning compiler artifacts.

Do **not** replace these build commands with `cargo check` for cache acceptance testing.

## 28.5 Standalone-workstation invariant

```text
fast local SSD cache
+ workflow-specific incremental policy
+ measured client-side/server-side choice
+ build-based acceptance tests
> elaborate remote infrastructure for one machine
```

Do not deploy Redis/object storage solely because sccache supports them.

---

# 29) Best-in-class profile: multi-worktree / programming-agent workstation

## 29.0 Why this profile is different

Parallel worktrees/agents create a workload that *looks* ideal for sccache:

```text
same repository history
same toolchain
same dependencies
same target/profile/features
multiple independent target directories
frequent revision transitions
```

But released 0.17.0 adds a crucial qualifier:

```text
multiple absolute checkout roots
→ Rust cache identity can differ
→ SCCACHE_BASEDIRS does not normalize Rust keys yet
```

Therefore the best-in-class design is **shared cache + independent Cargo state + realistic expectations about path-sensitive misses**, not a promise of full cross-worktree reuse.

## 29.1 Canonical policy

Global wrapper:

```bash
export RUSTC_WRAPPER=sccache
export SCCACHE_CACHE_SIZE=50G
```

Shared-reuse build task:

```bash
CARGO_INCREMENTAL=0 cargo build --workspace
```

Interactive check task:

```bash
RUSTC_WRAPPER= cargo check --workspace --all-targets
```

Evaluate `SCCACHE_CLIENT_SIDE=1` on the actual agent concurrency pattern rather than assuming it is faster in every 0.17.0 workload.

## 29.2 Path policy

Do **not** generate `SCCACHE_BASEDIRS` from `git worktree list` on 0.17.0. It does not solve Rust cross-checkout reuse in the released implementation.

Instead choose among three deliberate strategies:

### Strategy A — canonical single checkout path, serialized ownership

For ephemeral agents that do not need simultaneous worktrees, reuse a canonical absolute path:

```text
/work/active-repo
```

Reset/switch revisions between tasks rather than creating path-divergent worktrees. This gives the strongest current Rust path stability but sacrifices parallel checkout isolation.

### Strategy B — parallel worktrees, accept path-sensitive misses

For true concurrent agents:

```text
/worktrees/agent-a
/worktrees/agent-b
/worktrees/agent-c
```

keep separate target directories and accept that workspace/path-sensitive Rust units may miss across roots. sccache can still be valuable inside each worktree and for matching dependency/toolchain inputs.

### Strategy C — containerized canonical internal path per agent

Where operationally reasonable, mount each agent's checkout at the **same path inside an isolated container/VM**, for example:

```text
host A path → container /workspace/repo
host B path → container /workspace/repo
```

This can standardize rustc-visible paths while preserving host-side isolation. It is a deployment architecture, not an sccache setting; validate generated paths, `CARGO_HOME`, toolchains, and mounts consistently.

## 29.3 Shared target directory versus shared sccache

Do not compensate for sccache's path limitation by casually forcing autonomous worktrees into one mutable Cargo target tree:

```text
shared CARGO_TARGET_DIR
  direct Cargo artifact sharing
  tighter locking/state coupling
  potential contention and stale-state interactions

independent target dirs + shared sccache
  safer isolation
  whole-compiler-result sharing where keys match
  some cross-worktree Rust misses remain in 0.17.0
```

For autonomous parallel work, independent target directories remain the safer default.

## 29.4 Agent command contract

```just
# Compile-producing shared-reuse path.
build-shared:
    CARGO_INCREMENTAL=0 cargo build --workspace

# Fast semantic feedback; not an sccache-hit acceptance target.
check:
    RUSTC_WRAPPER= cargo check --workspace --all-targets

# Apples-to-apples local hot-build alternative.
build-incremental:
    RUSTC_WRAPPER= CARGO_INCREMENTAL=1 cargo build --workspace

sccache-stats:
    sccache --show-adv-stats
```

## 29.5 Agent-workstation invariant

> Independent worktrees should not share mutable Cargo state merely to chase cache hits. In sccache 0.17.0, preserve isolation, standardize rustc-visible paths where architecture permits, and explicitly accept the remaining Rust cross-checkout limitation.

---

# 30) Best-in-class profile: team remote cache

## 30.0 Objective

A remote team cache is justified when multiple machines repeatedly compile substantially overlapping Rust graphs.

The best general topology is hierarchical:

```text
rustc invocation
      │
      ▼
local SSD L0
      │ miss
      ▼
low-latency shared L1        optional
      │ miss
      ▼
durable object L2           optional
```

In sccache 0.17, multi-level caching provides ordered reads, parallel writes, and automatic backfill from a slower hit into faster preceding levels.

## 30.1 Recommended topology A — local disk + Redis

Use when:

- team members are geographically/network close;
- low cache-hit latency matters;
- Redis is already operated reliably;
- retaining every historical artifact is unnecessary.

```bash
export SCCACHE_MULTILEVEL_CHAIN="disk,redis"
export SCCACHE_DIR="$HOME/.cache/sccache"
export SCCACHE_CACHE_SIZE=30G
export SCCACHE_REDIS_ENDPOINT="rediss://cache.internal.example:6379"
export SCCACHE_REDIS_EXPIRATION=1209600   # example: 14 days
export SCCACHE_REDIS_KEY_PREFIX="rust/project-a/"
export SCCACHE_MULTILEVEL_WRITE_ERROR_POLICY=l0
```

Operate Redis with bounded memory and an eviction policy appropriate for cache semantics, such as `allkeys-lru` as upstream recommends for a fixed-size cache.

## 30.2 Recommended topology B — local disk + S3/R2

Use when:

- durable fleet reuse matters more than single-digit-millisecond remote latency;
- ephemeral CI workers are important cache consumers;
- object-storage economics and lifecycle policies are preferable;
- cross-site access is required.

```bash
export SCCACHE_MULTILEVEL_CHAIN="disk,s3"
export SCCACHE_DIR="$HOME/.cache/sccache"
export SCCACHE_CACHE_SIZE=30G
export SCCACHE_BUCKET="org-rust-sccache"
export SCCACHE_REGION="us-east-1"
export SCCACHE_S3_KEY_PREFIX="project-a/"
export SCCACHE_S3_USE_SSL=true
export SCCACHE_MULTILEVEL_WRITE_ERROR_POLICY=l0
```

For Cloudflare R2 or another S3-compatible service, configure the service endpoint and its required region semantics; for R2, upstream documents `https://ACCOUNT_ID.r2.cloudflarestorage.com` and region `auto`.

## 30.3 Recommended topology C — disk + Redis + object store

Use only when the team is large enough to benefit from both a fast shared tier and durable backing:

```bash
export SCCACHE_MULTILEVEL_CHAIN="disk,redis,s3"
export SCCACHE_MULTILEVEL_WRITE_ERROR_POLICY=l0
```

Behavior:

```text
read:
  disk → redis → s3

s3 hit:
  return artifact
  asynchronously backfill redis and disk

write:
  write each read-write level in parallel
```

This is the highest-capability general topology, but it is not automatically the lowest-cost one.

## 30.4 Why `write_error_policy=l0` is the default recommendation

The cache should accelerate compilation, not make a valid Rust build unavailable because a remote cache tier is temporarily unhealthy.

`l0` means:

- failure to write the primary local level is considered material;
- failures in slower remote write tiers do not fail the build;
- read-only levels do not trigger write failures.

Use `all` only if remote cache persistence is itself a required build invariant, which is unusual.

## 30.5 Namespace strategy

A remote prefix should reflect **trust and retention boundaries**, not every ordinary cache-key dimension. sccache already keys on compiler/argument/source identity.

Useful examples:

```text
rust/project-a/trusted/
rust/project-a/untrusted-pr/<ephemeral-id>/
rust/project-b/trusted/
```

Usually unnecessary:

```text
rust/project-a/rust-1.91/x86/release/feature-abc/
```

unless operational lifecycle or access control requires that partition.

## 30.6 Credential policy

Prefer:

1. workload identity / web identity / short-lived role credentials;
2. platform-native ephemeral credentials;
3. narrowly scoped static credentials only when unavoidable.

For object storage, grant only required object operations in the intended bucket/prefix. Cache readers do not need administrative bucket privileges.

## 30.7 0.17.0 multi-level validation requirement

Before enabling a remote topology fleet-wide, prove the exact combination under both server-side and client-side mode where relevant:

```text
cold L0 + warm remote → hit
remote hit → L0 backfill
subsequent request → L0 hit
remote read-only → local RW still behaves correctly
remote outage → build compiles successfully
```

This is especially important on 0.17.0 because current open issues report edge cases in read-only/multi-level backfill behavior.

## 30.8 Remote-team invariant

> The fastest cache tier should be local; the broadest reuse tier should be shared; and remote unavailability should normally degrade to compilation rather than build failure.

---

# 31) Best-in-class profile: generic CI

## 31.0 CI objective

CI is one of sccache's strongest Rust use cases because runners commonly start with an empty Cargo target tree while rebuilding compiler units seen by prior jobs.

The ideal design separates:

```text
reproducible dependency/toolchain inputs
cacheable compiler outputs
job-local Cargo target state
stable rustc-visible checkout paths
trust-dependent remote write permission
```

## 31.1 Canonical CI environment

```bash
export RUSTC_WRAPPER=sccache
export CARGO_INCREMENTAL=0
export SCCACHE_MULTILEVEL_WRITE_ERROR_POLICY=l0
```

For sccache 0.17.0, treat:

```bash
export SCCACHE_CLIENT_SIDE=1
```

as a rollout choice to benchmark/validate, not an unconditional invariant. Then configure the remote backend.

## 31.2 Stable checkout-root policy

Released 0.17.0 does **not** provide Rust cross-root normalization through `SCCACHE_BASEDIRS`. Therefore equivalent CI jobs should build at a canonical absolute path when feasible:

```text
/workspace/repo
```

Keep that path stable across jobs/runners that are intended to share Rust compiler artifacts. Also keep `CARGO_HOME` and toolchain layout stable where practical.

If the CI platform injects a fixed per-repository workspace path, verify it rather than assuming. If it includes random/job-specific components, quantify the resulting cache split.

## 31.3 Trusted branch policy

For protected branches / trusted internal PRs:

```text
remote read  = yes
remote write = yes
```

For untrusted fork PRs, prefer:

```text
A. no shared remote credentials
B. shared remote READ_ONLY
C. isolated untrusted namespace with no trusted consumers
```

B is often the best performance/security balance when the backend safely supports it, but validate the exact read-only/multi-level behavior of the pinned sccache version.

## 31.4 Never namespace normal cache history by commit SHA

A per-commit namespace destroys cross-commit reuse. sccache already hashes compiler/source inputs. Namespace instead for:

- repository identity;
- trust domain;
- deliberate generation/purge boundaries.

Add a commit SHA only when deliberate isolation is the objective.

## 31.5 Matrix jobs

A matrix naturally splits compiler keys by:

- Rust toolchain;
- target triple;
- profile;
- feature set;
- `RUSTFLAGS`;
- instrumentation.

Do not fight those semantic splits. The shared backend can host them under one operational namespace unless access/retention policy says otherwise.

## 31.6 Preserve statistics

At the end of representative **build-producing** jobs:

```bash
sccache --show-stats
sccache --show-adv-stats
sccache --show-stats --stats-format=json > sccache-stats.json
```

Retain JSON as telemetry/artifact when cache economics matter. Parse it only against a pinned sccache version.

## 31.7 CI acceptance expectations

Do not set one universal hit-rate SLO. Establish baselines per job class:

```text
warm dependency-heavy cargo build
  high cacheable hit rate expected

changed core workspace library
  lower hit rate expected

new Rust toolchain / changed target
  near-cold expected

release binary
  dependencies may hit; final link remains non-cacheable

cargo check job
  low Rust sccache hit expectation is normal; it is not the primary cache benchmark
```

Alert on regressions from the appropriate baseline.

---

# 32) Best-in-class profile: GitHub Actions

## 32.0 Current integration

As of the reference date, Mozilla's official GitHub Action is:

```yaml
uses: mozilla-actions/sccache-action@v0.0.11
```

The action installs/configures sccache and emits a post-run statistics step. The native GitHub Actions storage backend is enabled with `SCCACHE_GHA_ENABLED`.

## 32.1 Canonical trusted build-job example

```yaml
name: rust-build

on:
  push:
  pull_request:

jobs:
  build:
    runs-on: ubuntu-latest
    env:
      RUSTC_WRAPPER: sccache
      CARGO_INCREMENTAL: "0"
      SCCACHE_GHA_ENABLED: "true"
      SCCACHE_MULTILEVEL_CHAIN: "disk,gha"
      SCCACHE_CACHE_SIZE: "10G"
      SCCACHE_MULTILEVEL_WRITE_ERROR_POLICY: "l0"

    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable

      - name: Set up sccache
        uses: mozilla-actions/sccache-action@v0.0.11
        with:
          version: "v0.17.0"

      - name: Build workspace
        run: cargo build --workspace --all-targets

      - name: Record advanced sccache stats
        if: always()
        run: sccache --show-adv-stats
```

### Path note

Do **not** add `SCCACHE_BASEDIRS=$GITHUB_WORKSPACE` expecting Rust path normalization in 0.17.0. Instead verify that the runner path for the same repository is stable across the jobs intended to share cache. If the absolute path changes, that is a real Rust key split in the released version.

### Client-side note

Upstream recommends the new client-side architecture, but because 0.17.0 is the first release with it and current multi-level reports exist, add:

```yaml
SCCACHE_CLIENT_SIDE: "1"
```

only after validating the specific `disk,gha` behavior and benchmarking wall time for the repository. Keep the version pin so that behavior does not change silently.

### Why pin both action and binary?

They are separate release surfaces:

```text
mozilla-actions/sccache-action@v0.0.11
  GitHub Action implementation

version: v0.17.0
  compiler-cache binary
```

Pinning both makes CI behavior auditable.

## 32.2 `disk,gha` versus GHA alone

Not every workflow needs an L0:

```text
GHA only
  simpler
  one storage layer

disk → GHA
  potentially faster repeated accesses in one job
  supports L0 backfill semantics
  requires validation of the pinned multi-level implementation
```

Measure both for short jobs.

## 32.3 Untrusted fork PRs

Do not expose writable trusted-cache capability to fork-controlled code. Where the GHA backend/event-token model gives the desired separation, use:

```bash
SCCACHE_GHA_RW_MODE=READ_ONLY
```

Otherwise disable the shared backend for that job or use an isolated namespace with no trusted consumers.

## 32.4 Cache generation rotation

The GHA backend exposes:

```bash
SCCACHE_GHA_VERSION
```

Changing it creates an operational cache-generation boundary. Use that for:

- suspected shared-cache corruption;
- deliberate deployment migration;
- major cache-policy changes.

Do not rotate routinely; routine rotation destroys useful reuse.

## 32.5 Rate-limit behavior

Upstream documents that GHA cache rate limiting can allow the build to continue while storage fails. Therefore successful compilation is not proof that an artifact was written. Inspect stats/logs when reuse unexpectedly collapses.

## 32.6 Recommended GitHub Actions rule

> Pin the action and binary, use `cargo build` for cache-producing/validation jobs, disable rustc incremental for those jobs, keep rustc-visible checkout paths stable, capture stats, and make cache-write permission depend on trust.

---

# 33) Dual-mode development: build reuse versus hot edit/check

## 33.0 Why one global policy is suboptimal

Rust has three distinct layers here:

```text
Cargo target freshness
rustc incremental state
sccache whole-invocation cache
```

and ordinary `cargo check` does not generate the same cacheable Rust output shape as `cargo build`. A best-in-class environment therefore exposes explicit workflows rather than forcing one switch globally.

## 33.1 Mode A — shared-reuse build

```bash
RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 cargo build --workspace
```

Best when:

- cold/isolated Cargo target trees are common;
- dependency compilation dominates;
- CI symmetry matters;
- multiple machines share work;
- previously built revisions are revisited at stable paths.

## 33.2 Mode B — interactive check

```bash
RUSTC_WRAPPER= cargo check --workspace
```

Best when:

- the objective is semantic feedback rather than codegen artifacts;
- one developer/agent repeatedly edits the same workspace crates;
- the target directory persists;
- Cargo's normal incremental behavior improves feedback.

Do not expect ordinary check units to be sccache hits. If wrapper overhead matters, benchmark:

```bash
RUSTC_WRAPPER= cargo check --workspace
```

## 33.3 Mode C — incremental build comparison

When the actual local bottleneck is repeated `cargo build` after edits, compare the same command:

```bash
RUSTC_WRAPPER= CARGO_INCREMENTAL=1 cargo build --workspace
```

against Mode A. This is the correct test of fine-grained incremental reuse versus whole-invocation cache reuse.

## 33.4 Repository-level interface

```just
build-shared:
    RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 cargo build --workspace

check:
    RUSTC_WRAPPER= cargo check --workspace --all-targets

build-incremental:
    RUSTC_WRAPPER= CARGO_INCREMENTAL=1 cargo build --workspace
```

## 33.5 Benchmark state must be explicit

Every performance claim should state:

```text
Cargo target warmth
incremental-state warmth
sccache L0/remote warmth
absolute checkout path
Rust toolchain
features/profile/target
client-side or server-side mode
```

## 33.6 Default decision rule

> Use sccache as the canonical **build-result reuse** layer. Preserve Rust incremental compilation for interactive checking or hot builds where measurements justify it. Do not disable incremental checking merely to optimize a cache path that `cargo check` normally cannot use.

---

# 34) Configuration hierarchy, precedence, and platform paths

## 34.0 Configuration surfaces

sccache configuration comes from:

1. sccache configuration file;
2. environment variables;
3. relevant CLI commands/flags;
4. Cargo wrapper and compilation environment.

Upstream explicitly states that environment configuration overrides values set in the sccache config file.

## 34.1 Config-file discovery

Override location:

```bash
SCCACHE_CONF=/absolute/path/to/config
```

Default locations:

| OS | sccache config |
|---|---|
| Linux | `~/.config/sccache/config` |
| macOS | `~/Library/Application Support/Mozilla.sccache/config` |
| Windows | `%APPDATA%\Mozilla\sccache\config\config` |

Local disk cache defaults:

| OS | local cache directory |
|---|---|
| Linux | `~/.cache/sccache` |
| macOS | `~/Library/Caches/Mozilla.sccache` |
| Windows | `%LOCALAPPDATA%\Mozilla\sccache` |

Default local cache size: **10 GiB**.

## 34.2 Environment precedence is useful for deployment overlays

A good division of responsibility is:

```text
config file
  stable workstation/backend defaults

environment
  credentials
  CI-specific paths
  trust-mode read/write setting
  temporary diagnosis/tuning
```

Do not commit secrets into `.cargo/config.toml` or the sccache config file in the repository.

## 34.3 Cargo wrapper precedence

Cargo provides both a config key and environment variable:

```toml
[build]
rustc-wrapper = "sccache"
```

```bash
RUSTC_WRAPPER=sccache
```

Cargo also has `rustc-workspace-wrapper`, but it is not a drop-in synonym. If both wrapper forms are set, Cargo nests them. A global sccache setup should therefore verify that editor/tooling configuration has not independently set `RUSTC_WORKSPACE_WRAPPER` in a way that changes invocation behavior.

## 34.4 Incremental precedence

Cargo's global build-level setting can be expressed as:

```toml
[build]
incremental = false
```

or:

```bash
CARGO_INCREMENTAL=0
```

Cargo documents that `CARGO_INCREMENTAL` overrides profile-level incremental settings. This makes it an especially useful guard in CI and wrapper scripts.

## 34.5 Server restart boundary

When a setting is consumed by the long-lived daemon, an already-running server may retain prior state. A safe troubleshooting sequence is:

```bash
sccache --stop-server || true
SCCACHE_LOG=debug sccache --start-server
```

With 0.17 client-side mode, compilation logic runs in client processes but the daemon still owns shared storage/stats. Do not assume “client-side” means “no server state.”

---

# 35) Rust-relevant configuration and environment-variable catalog

This catalog intentionally omits variables that only tune non-Rust compiler preprocessing behavior.

## 35.1 Core execution and config

| Variable | Purpose | Default / upstream behavior | Rust recommendation |
|---|---|---|---|
| `SCCACHE_CONF` | alternate config-file path | platform default if unset | use for managed deployments |
| `SCCACHE_CLIENT_SIDE` | run compile/cache pipeline in client, daemon as storage gateway | opt-in in 0.17; upstream calls it recommended | enable after A/B validation; especially test multi-level/backfill behavior |
| `SCCACHE_IDLE_TIMEOUT` | server idle timeout | `0` means permanent when explicitly set | leave default unless startup churn matters |
| `SCCACHE_NO_DAEMON` | do not background server | daemonizes normally | CI/debug only when useful |
| `SCCACHE_STARTUP_NOTIFY` | server-start completion notification socket | unset | orchestration-specific |
| `SCCACHE_MAX_FRAME_LENGTH` | max client/server frame size | implementation default | change only for diagnosed large-message issue |
| `SCCACHE_CACHED_CONF` | cached configuration behavior surface | version-specific | avoid relying on undocumented semantics |
| `SCCACHE_ALLOW_CORE_DUMPS` | allow server core dumps | off | diagnosis only; review secret exposure |

### Client-side compatibility caveat

`SCCACHE_CLIENT_SIDE=1` is ignored when:

- `SCCACHE_ERROR_LOG` is in use;
- distributed compilation is in use.

Additionally, open issue #2796 reports a 0.17-era multi-level L0-population problem with client-side `disk,webdav`. Treat backfill as an acceptance-test requirement for any multi-level topology.

## 35.2 Generic `SCCACHE_BASEDIRS` exists, but is not a Rust 0.17 feature

| Variable | Generic purpose | Rust 0.17.0 status | Recommendation |
|---|---|---|---|
| `SCCACHE_BASEDIRS` | strip configured path prefixes during supported cache-key computation | **not wired into the released Rust cache key** | do not rely on it for Rust cross-worktree/cross-checkout hits |

The generic configuration supports multiple absolute paths (`:` separator on Unix, `;` on Windows) and longest-prefix matching. Those mechanics are relevant to compiler paths that implement basedirs, but upstream issue #2652 and open PR #2794 establish that Rust support is still pending after 0.17.0.

For Rust 0.17.0, use stable absolute checkout/toolchain paths instead.

## 35.3 Local disk

| Variable | Purpose | Default | Recommendation |
|---|---|---|---|
| `SCCACHE_DIR` | local cache location | platform-specific | fast local SSD |
| `SCCACHE_CACHE_SIZE` | max disk cache size | `10G` | start around 20–50G for large workspaces if disk allows; measure |
| `SCCACHE_LOCAL_RW_MODE` | `READ_ONLY` / `READ_WRITE` | read-write | RW on owned local cache |

The local backend is designed around one sccache server owning a cache directory. Do not point multiple independent concurrently running sccache servers at the same local directory.

## 35.4 Compression

| Variable | Purpose | Default | Recommendation |
|---|---|---|---|
| `SCCACHE_CACHE_ZSTD_LEVEL` | zstd compression level | `3`; range `1–22` | leave at 3 initially |

Upstream's own example reports level 10 at roughly 0.9× size and 1.6× compression time versus level 3 for its test. Compression changes apply only to newly written entries.

## 35.5 Multi-level cache

| Variable | Purpose | Default | Recommendation |
|---|---|---|---|
| `SCCACHE_MULTILEVEL_CHAIN` | ordered fast→slow backend list | single backend | `disk,redis`, `disk,s3`, `disk,gha`, or `disk,redis,s3` |
| `SCCACHE_MULTILEVEL_WRITE_ERROR_POLICY` | write failure policy: `ignore`, `l0`, `all` | `l0` | keep `l0` for normal dev/CI |

Valid backend names in 0.17 configuration include:

```text
disk
redis
memcached
s3
gcs
azure
gha
webdav
oss
cos
```

## 35.6 Redis

| Variable | Purpose | Recommendation |
|---|---|---|
| `SCCACHE_REDIS_ENDPOINT` | single-node Redis URL | use `rediss://` across untrusted networks |
| `SCCACHE_REDIS_CLUSTER_ENDPOINTS` | comma-separated cluster URLs | use for managed Redis cluster |
| `SCCACHE_REDIS_USERNAME` | ACL username | secret/injected |
| `SCCACHE_REDIS_PASSWORD` | password | secret/injected |
| `SCCACHE_REDIS_DB` | Redis DB | default 0 unless isolation convention requires another |
| `SCCACHE_REDIS_EXPIRATION` | entry expiration seconds | choose from reuse horizon/memory budget |
| `SCCACHE_REDIS_KEY_PREFIX` | operational key prefix | repository/trust namespace |
| `SCCACHE_REDIS_RW_MODE` | read-only/read-write | RO for untrusted consumers |

The legacy `SCCACHE_REDIS` connection variable is deprecated; use the explicit endpoint configuration.

## 35.7 S3 / S3-compatible

| Variable | Purpose | Recommendation |
|---|---|---|
| `SCCACHE_BUCKET` | bucket | dedicated cache bucket/prefix |
| `SCCACHE_REGION` | AWS region; custom endpoints may use `auto` | explicit |
| `SCCACHE_ENDPOINT` | custom S3-compatible endpoint | TLS endpoint |
| `SCCACHE_S3_USE_SSL` | require TLS for endpoint | `true` |
| `SCCACHE_S3_ENABLE_VIRTUAL_HOST_STYLE` | virtual-host addressing | enable where service/acceleration requires |
| `SCCACHE_S3_KEY_PREFIX` | object prefix | project + trust domain |
| `SCCACHE_S3_RW_MODE` | read-only/read-write | RO for untrusted consumers |
| `SCCACHE_S3_NO_CREDENTIALS` | anonymous/no-credential access | public read-only use cases only |
| `SCCACHE_S3_SERVER_SIDE_ENCRYPTION` | SSE-S3 | org-policy dependent |
| `SCCACHE_S3_SERVER_SIDE_ENCRYPTION_AWS_KMS` | AWS-managed KMS | org-policy dependent |
| `SCCACHE_S3_SERVER_SIDE_ENCRYPTION_KMS_KEY_ID` | customer KMS key | org-policy dependent; highest precedence |

Credential resolution can use AWS credential mechanisms including static environment credentials, profiles, instance metadata, role assumption, and web identity depending on deployment.

## 35.8 GitHub Actions

| Variable | Purpose | Recommendation |
|---|---|---|
| `SCCACHE_GHA_ENABLED` | enable native GHA backend | `true` / upstream docs also show `on` |
| `SCCACHE_GHA_VERSION` | cache generation | stable value; change for deliberate purge |
| `SCCACHE_GHA_RW_MODE` | read-only/read-write | RO for untrusted contexts |
| `ACTIONS_RESULTS_URL` | service URL supplied by Actions runtime | platform-provided |
| `ACTIONS_RUNTIME_TOKEN` | runtime auth token | platform-provided, never persist |

Do not persist GitHub runtime tokens outside the job.

## 35.9 Logging and diagnostics

| Variable | Purpose | Recommendation |
|---|---|---|
| `SCCACHE_LOG` | `env_logger`-style log level/filter | `debug`/`trace` only during diagnosis |
| `SCCACHE_LOG_MILLIS` | millisecond timestamps | enable for latency/concurrency analysis |
| `SCCACHE_ERROR_LOG` | write errors to file | useful diagnosis, but disables client-side mode in 0.17 |
| `SCCACHE_RECACHE` | force cache overwrite/recache behavior | diagnosis/recovery only |

Never leave trace logging enabled in normal CI unless log volume and potential sensitive path/metadata exposure have been assessed.

## 35.10 Cargo-side variables that materially interact with sccache

| Variable | Purpose | Best-in-class sccache mode |
|---|---|---|
| `RUSTC_WRAPPER` | compiler wrapper | `sccache` |
| `CARGO_INCREMENTAL` | global incremental override | `0` |
| `CARGO_TARGET_DIR` | Cargo artifact tree | independent per worktree/job is usually fine |
| `RUSTFLAGS` | compiler flags | treat changes as intentional cache splits |
| `CARGO_BUILD_RUSTFLAGS` | Cargo config env equivalent | same |
| target-specific rustflags env/config | compiler target settings | same |

Do not attempt to “stabilize” cache hits by hiding semantically meaningful flags.

---

# 36) Backend and topology decision matrix

## 36.1 Backend characteristics

| Backend | Hit latency | Durability | Sharing scope | Operational burden | Best Rust use |
|---|---:|---:|---:|---:|---|
| local disk | excellent | machine-local | one machine | very low | default L0 |
| Redis | excellent–good | cache-oriented | LAN/team | medium | low-latency shared L1 |
| S3 / R2 | good–moderate | high | org/global | low–medium | durable shared L1/L2, CI fleet |
| GHA | platform-dependent | CI-managed | GitHub workflow ecosystem | low | GitHub Actions |
| GCS / Azure | good–moderate | high | org/global | low–medium | cloud-aligned alternative to S3 |
| Memcached | excellent | ephemeral | LAN/team | medium | simple volatile shared cache |
| WebDAV | variable | backend-dependent | variable | medium | existing WebDAV infrastructure |
| OSS / COS | good–moderate | high | org/global | low–medium | cloud-provider-native object storage |

## 36.2 Decision tree

```text
Only one persistent developer machine?
  yes → disk only
  no  ↓

GitHub-hosted CI only, no cross-platform team cache requirement?
  yes → disk + GHA (or GHA alone if benchmarked better/simpler)
  no  ↓

Low-latency shared network + managed Redis available?
  yes → disk + Redis
  no  ↓

Durable object store available?
  yes → disk + S3/R2/GCS/Azure-equivalent
  no  ↓

Need both low latency and durable global reuse at meaningful scale?
  yes → disk + Redis + object store
```

## 36.3 Selection rule by dominant cost

```text
CPU compile cost dominates, network fast:
  remote cache attractive

network RTT/bandwidth dominates, compiles cheap:
  remote cache may lose

storage cost dominates:
  shorter retention / lower tier / compression tuning

cache misses dominate and compiles are highly parallelizable:
  evaluate distributed compilation after cache tuning

final link dominates:
  sccache topology will not solve the bottleneck
```

## 36.4 Remote-cache break-even model

A remote hit is worthwhile when approximately:

```text
remote lookup
+ artifact download
+ decompression
+ local materialization
<
local rustc compilation avoided
```

A remote write is worthwhile when expected future avoided compile cost exceeds:

```text
compression
+ upload
+ storage
+ request overhead
```

The correct topology is therefore workload- and network-dependent even if cache correctness is identical.

---

# 37) Anti-pattern catalog

| Anti-pattern | Why it fails | Correct pattern |
|---|---|---|
| enable sccache but leave rustc incremental on | Rust invocation becomes non-cacheable / defeats intended mode | `CARGO_INCREMENTAL=0` with sccache |
| assume `target/` reuse and sccache reuse are the same | different caching layers | benchmark them independently |
| judge success from one warm Cargo build | Cargo may have done no compilation | cold `target/`, warm sccache `cargo build` test |
| use `cargo check` as the Rust cache acceptance workload | ordinary check units omit `link`, which sccache requires | validate with `cargo build` or representative compile-producing test workflow |
| rely on `SCCACHE_BASEDIRS` for Rust worktrees in 0.17.0 | generic docs overstate released Rust support; Rust key remains checkout-sensitive | standardize rustc-visible absolute paths; track #2652/#2794 |
| add ad hoc path-stripping around Rust cache keys | may make non-equivalent embedded-path artifacts share identity | use released behavior; require compiler + cache-key correctness before normalization |
| give fork PRs write credentials to trusted remote cache | cache poisoning / supply-chain boundary violation | read-only or isolated cache |
| use commit SHA as normal remote namespace | destroys cross-commit reuse | namespace by repo/trust/generation |
| clear sccache on every CI job | converts cache into expensive no-op | preserve remote cache across jobs |
| increase zstd level without measuring | trades CPU for size blindly | default 3, benchmark transfer-bound cases |
| make remote cache write failure fail every build | cache outage becomes build outage | multi-level `l0` policy normally |
| put local cache on slow NFS because it is “shared” | latency/locking/concurrency may erase benefit | local SSD L0 + proper remote backend |
| point multiple independent servers at one local disk cache | upstream warns of races/spurious failures | one server per local cache directory |
| enable client-side mode and assume `SCCACHE_ERROR_LOG` still uses it | 0.17 ignores client-side in that configuration | know compatibility boundary |
| enable client-side mode without testing multi-level backfill | open 0.17-era issue reports L0 population failure for one multi-level topology | test remote hit → L0 backfill → local hit |
| enable client-side mode and distributed compilation together | dist path disables client-side mode | choose architecture intentionally |
| assume a high hit rate guarantees fast builds | hits can be slow and linking/non-cacheable work can dominate | measure wall time + hit latency + link floor |
| assume a low hit rate means sccache is useless | expensive dependency hits may still save most time | weight by time avoided, not count only |
| treat `proc-macro` crates as ordinary cacheable libraries | system-linker crate type is not cached; macro behavior can read hidden files | expect non-cacheable macro crate + audit macro inputs |
| assume proc-macro filesystem reads are always tracked | upstream explicitly warns they may not cache properly | make inputs explicit/stable; bypass when correctness uncertain |
| force final `bin`/`cdylib` into cache expectations | system-linker crate types are not cached | optimize dependency compiles; tune linker separately |
| modify compiler flags to force cache reuse | risks semantic mismatch or simply changes key | preserve correct flags; let key split |
| use `SCCACHE_RECACHE` permanently | defeats normal hit semantics and adds write load | diagnosis/purge recovery only |
| parse human stats output as a stable API | formatting can change | JSON output + version pin when automating |
| share cache across security domains because keys are content-addressed | cache is still executable build output | enforce access/trust policy |
| deploy distributed compilation before measuring misses | large ops/security surface may solve the wrong bottleneck | tune cache, linker, graph, then evaluate dist |

## 37.1 The most dangerous correctness anti-pattern

The most serious mistake is treating sccache as proof that all compiler inputs are hermetic.

A cache is only as correct as the inputs represented in its identity. Rust build systems can contain hidden compile-time I/O through procedural macros or generated inputs. If a build path violates the assumptions of sccache's dependency discovery, **a fast hit can be worse than a slow miss** because it may return stale output.

When correctness is suspect:

```bash
RUSTC_WRAPPER= CARGO_INCREMENTAL=0 cargo clean
cargo build
```

and compare against the sccache path before restoring normal operation.

---

# 38) Operator and LLM-agent checklist

## 38.0 Session inventory

Before diagnosing/changing cache behavior:

```bash
rustc -vV
cargo -V
sccache --version
sccache --show-stats
```

Record:

```text
host/target triple
active toolchain
profile/features
RUSTFLAGS
RUSTC_WRAPPER
CARGO_INCREMENTAL
CARGO_TARGET_DIR
absolute checkout root
SCCACHE_CLIENT_SIDE
SCCACHE_MULTILEVEL_CHAIN
backend identity / namespace, without secrets
```

Do not record `SCCACHE_BASEDIRS` as a Rust normalization control on 0.17.0; if it is present globally, note that it does not establish Rust cross-checkout equivalence.

## 38.1 Installation acceptance test

```bash
command -v sccache
sccache --version
sccache --zero-stats
rm -rf target
RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 cargo build
sccache --show-adv-stats
```

Success means more than exit code 0: verify Rust compile requests and cacheable/non-cacheable accounting.

## 38.2 Cache-hit acceptance test

```bash
# Populate cache.
sccache --zero-stats
rm -rf target
RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 cargo build --workspace
sccache --show-adv-stats

# Cold Cargo target, warm sccache.
sccache --zero-stats
rm -rf target
RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 cargo build --workspace
sccache --show-adv-stats
```

The second build should demonstrate meaningful hits for unchanged cacheable library units.

If you substitute `cargo check`, a low Rust hit count is expected and does **not** validate the cache.

## 38.3 Checkout-path sensitivity test

Use this to quantify the released 0.17.0 limitation:

```text
1. build revision R at absolute root A with cold Cargo target
2. preserve sccache
3. build the same revision/toolchain/features at absolute root A again with cold target
4. record hits — this is same-path reuse
5. build the same revision at different absolute root B with cold target
6. record the hit-rate delta
```

Expected interpretation:

```text
same-path warm-cache hits
  prove ordinary Rust compiler-cache reuse

cross-root hit collapse for workspace/path-sensitive units
  is consistent with the known 0.17.0 Rust BASEDIRS gap
```

Do not “fix” the test by setting `SCCACHE_BASEDIRS`; that is exactly the unreleased Rust feature being tracked upstream.

## 38.4 Remote-cache acceptance test

Verify independently:

```text
L0 hit
remote hit with L0 empty
remote miss + successful compile
remote write
remote hit → L0 backfill
second request → L0 hit
remote outage fallback
read-only consumer behavior
untrusted job credential absence
```

Run the backfill test in the chosen client-side/server-side mode.

## 38.5 Incident procedure: suspected stale/corrupt cache

1. Capture versions/config/stats and absolute paths.
2. Reproduce with sccache enabled.
3. Reproduce with `RUSTC_WRAPPER=` and a clean/isolated target tree.
4. If only the cached path is wrong, isolate the affected namespace/backend.
5. Use deliberate recache or cache-generation rotation rather than semantic flag changes.
6. Investigate hidden compile-time inputs, especially procedural-macro filesystem reads/generated inputs.
7. Restore remote write access only after the correctness boundary is understood.

## 38.6 Incident procedure: hit-rate collapse

Check in this order:

```text
Cargo command changed from build to check?
Rust toolchain/compiler/sysroot changed?
absolute checkout root changed?
CARGO_HOME/toolchain layout changed?
feature/profile/target changed?
RUSTFLAGS changed?
Cargo lock/dependency graph changed?
source/build-script outputs changed?
remote backend unavailable/rate-limited?
cache generation/prefix changed?
entries evicted/expired?
work became non-cacheable (linker crate type / incremental / unsupported emit)?
```

This ordering is usually more productive than immediately increasing cache size.

## 38.7 Incident procedure: sccache appears slower

Measure:

```text
cache lookup latency
artifact transfer size/time
compression/decompression CPU
rustc compile time avoided
link time
Cargo scheduling idle time
client-side versus server-side mode
```

Then compare against the bypass path.

Possible outcomes:

- keep local cache, disable remote tier;
- move remote tier closer;
- add Redis L1 before object storage;
- revert to server-side mode if client-side regresses this workload;
- lower compression;
- increase local L0 hit probability;
- use incremental hot-build/check mode for that workflow;
- optimize the linker if link time dominates.

## 38.8 Quick command reference

```bash
# Identity
sccache --version
sccache --help

# Lifecycle
sccache --start-server
sccache --stop-server

# Statistics
sccache --show-stats
sccache --show-adv-stats
sccache --show-stats --stats-format=json
sccache --zero-stats

# Canonical Rust shared-reuse build
RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 cargo build --workspace

# Bypass sccache, retain non-incremental build semantics
RUSTC_WRAPPER= CARGO_INCREMENTAL=0 cargo build --workspace

# Hot-build incremental comparison
RUSTC_WRAPPER= CARGO_INCREMENTAL=1 cargo build --workspace

# Interactive check; not a cache-hit benchmark
RUSTC_WRAPPER= cargo check --workspace

# Diagnose logging
SCCACHE_LOG=debug SCCACHE_LOG_MILLIS=1 \
  RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 cargo build --workspace

# Distributed status, only if dist is deployed
sccache --dist-status
```

## 38.9 Final LLM-agent decision rules

1. **Inspect before changing.** Capture sccache/Cargo/rustc versions, command, target/profile/features, absolute paths, and relevant env.
2. **Use `cargo build`—not ordinary `cargo check`—to test Rust sccache hit behavior.**
3. **Disable rustc incremental compilation on the shared-reuse build path.**
4. **Do not globally disable incremental checking unless measurements justify it.**
5. **Treat distinct Rust checkout roots as distinct cache identity in released 0.17.0.**
6. **Do not rely on `SCCACHE_BASEDIRS` for Rust 0.17.0.** Track #2652/#2794 for a future release.
7. **Never infer cache effectiveness from Cargo wall time alone.** Read sccache stats/non-cacheable reasons.
8. **Do not clear caches reflexively.** Cache destruction removes evidence and performance value.
9. **Treat remote cache write access as privileged build-infrastructure access.**
10. **Prefer local SSD L0 before adding remote complexity.**
11. **Evaluate client-side mode rather than assuming it is universally faster in the first release.**
12. **Do not expect final `bin`/`dylib`/`cdylib`/`proc-macro` units to become cacheable.**
13. **Investigate procedural-macro hidden I/O when cache correctness is uncertain.**
14. **Deploy distributed compilation only after ordinary caching, build graph, path stability, and linker costs are understood.**
15. **Report evidence precisely:** cache state, hit/miss/non-cacheable counts, absolute checkout path, client/server mode, and benchmark command.

---

# 39) Authoritative source index and version-sensitivity map

## 39.0 Primary upstream sources

The reference should be maintained against released upstream documentation/source, not blog-post folklore.

### sccache repository and release

- sccache repository: <https://github.com/mozilla/sccache>
- releases: <https://github.com/mozilla/sccache/releases>
- v0.17.0 tag: <https://github.com/mozilla/sccache/tree/v0.17.0>
- v0.17.0 release: <https://github.com/mozilla/sccache/releases/tag/v0.17.0>

### Core documentation

- README / Rust Cargo wrapper usage: <https://github.com/mozilla/sccache/blob/v0.17.0/README.md>
- Rust-specific caveats: <https://github.com/mozilla/sccache/blob/v0.17.0/docs/Rust.md>
- Configuration: <https://github.com/mozilla/sccache/blob/v0.17.0/docs/Configuration.md>
- Architecture: <https://github.com/mozilla/sccache/blob/v0.17.0/docs/Architecture.md>
- Cache-key design: <https://github.com/mozilla/sccache/blob/v0.17.0/docs/Caching.md>
- Local storage: <https://github.com/mozilla/sccache/blob/v0.17.0/docs/Local.md>
- Multi-level caching: <https://github.com/mozilla/sccache/blob/v0.17.0/docs/MultiLevel.md>

### Remote storage

- Redis: <https://github.com/mozilla/sccache/blob/v0.17.0/docs/Redis.md>
- S3: <https://github.com/mozilla/sccache/blob/v0.17.0/docs/S3.md>
- GitHub Actions: <https://github.com/mozilla/sccache/blob/v0.17.0/docs/GHA.md>
- Google Cloud Storage: <https://github.com/mozilla/sccache/blob/v0.17.0/docs/Gcs.md>
- Azure: <https://github.com/mozilla/sccache/blob/v0.17.0/docs/Azure.md>
- WebDAV: <https://github.com/mozilla/sccache/blob/v0.17.0/docs/Webdav.md>

### Distributed compilation

- distributed quickstart: <https://github.com/mozilla/sccache/blob/v0.17.0/docs/DistributedQuickstart.md>

### Implementation-level Rust evidence

- Rust compiler integration: <https://github.com/mozilla/sccache/blob/v0.17.0/src/compiler/rust.rs>
- Rust BASEDIRS gap: <https://github.com/mozilla/sccache/issues/2652>
- Open Rust BASEDIRS implementation (not released): <https://github.com/mozilla/sccache/pull/2794>
- Open client-side multi-level L0 population report: <https://github.com/mozilla/sccache/issues/2796>
- Open read-only S3 + local multi-level report: <https://github.com/mozilla/sccache/issues/2773>
- configuration types/defaults: <https://github.com/mozilla/sccache/blob/v0.17.0/src/config.rs>
- CLI surface: <https://github.com/mozilla/sccache/blob/v0.17.0/src/main.rs>

## 39.1 Cargo sources

- Cargo configuration: <https://doc.rust-lang.org/cargo/reference/config.html>
- Cargo profiles: <https://doc.rust-lang.org/cargo/reference/profiles.html>
- Cargo environment variables: <https://doc.rust-lang.org/cargo/reference/environment-variables.html>
- Cargo build cache / rebuild behavior: <https://doc.rust-lang.org/cargo/guide/build-cache.html>

The Cargo Book is authoritative for `rustc-wrapper`, `build.incremental`, `CARGO_INCREMENTAL`, target directories, profiles, and flag precedence.

## 39.2 GitHub Actions integration

- Mozilla sccache action repository: <https://github.com/mozilla-actions/sccache-action>
- action releases: <https://github.com/mozilla-actions/sccache-action/releases>
- GitHub Marketplace listing: <https://github.com/marketplace/actions/sccache-action>

Reference-date action version: **v0.0.11**.

## 39.3 Facts that are especially version-sensitive

Revalidate these on any sccache upgrade:

```text
client-side mode opt-in/default/compatibility
Rust cacheability restrictions
cache-key version and inputs
configuration variable names
multi-level backend list
multi-level write semantics
backend auth and read-only controls
statistics fields / JSON schema
compression defaults
GitHub Actions backend protocol behavior
distributed compilation support matrix
```

## 39.4 Facts that are relatively stable but still verify

```text
Cargo rustc-wrapper semantics
CARGO_INCREMENTAL override semantics
system-linker crate-type boundary
local-versus-remote latency principle
trust requirement for shared executable artifacts
need to measure cache effectiveness
```

## 39.5 Upgrade procedure

When moving from sccache 0.17.0 to a later release:

1. Read every intervening release note.
2. Diff `docs/Rust.md`.
3. Diff `docs/Configuration.md` and `src/config.rs`.
4. Diff `docs/Architecture.md` for client/server changes.
5. Diff `docs/Caching.md` and Rust compiler implementation for cache-key changes.
6. Diff documentation for the deployed storage backend(s).
7. Verify CLI/statistics output expected by automation.
8. Run the cold-target/warm-cache benchmark suite.
9. Run same-path and cross-checkout Rust cache tests; re-evaluate `SCCACHE_BASEDIRS` only if the target release actually includes Rust support.
10. Test read-only and trusted-write CI contexts.
11. Only then update the pinned CI/workstation version.

---

# Final best-in-class configuration summary

For sccache **0.17.0** and Rust, the canonical starting point is intentionally smaller and more precise than “enable every cache knob”:

```toml
# ~/.cargo/config.toml or .cargo/config.toml
[build]
rustc-wrapper = "sccache"
```

Use a shared-reuse **build** command:

```bash
CARGO_INCREMENTAL=0 cargo build --workspace
```

Keep interactive checking independent:

```bash
RUSTC_WRAPPER= cargo check --workspace
```

and benchmark a hot incremental **build** alternative when relevant:

```bash
RUSTC_WRAPPER= CARGO_INCREMENTAL=1 cargo build --workspace
```

Local cache starting point:

```bash
export SCCACHE_CACHE_SIZE=30G          # 20–50G is a reasonable large-workspace starting range
export SCCACHE_CACHE_ZSTD_LEVEL=3      # upstream default
```

Client-side architecture:

```bash
export SCCACHE_CLIENT_SIDE=1
```

is the upstream-recommended 0.17 direction, but **treat it as validated configuration, not dogma**. A/B benchmark it and verify cache population/backfill for the exact backend topology before making it a fleet invariant.

### Rust 0.17 path rule

Do **not** configure `SCCACHE_BASEDIRS` expecting cross-worktree Rust hits. Released 0.17.0 does not wire that generic option into the Rust cache key. Instead:

```text
CI / containers
  use a canonical absolute checkout path
  stabilize CARGO_HOME/toolchain layout where practical

parallel local worktrees
  keep independent target trees
  accept path-sensitive workspace-crate misses
  do not sacrifice concurrency correctness to force one shared target tree
```

Track upstream issue #2652 and open PR #2794 for future released Rust path-normalization support.

Then choose only as much shared storage as the workload warrants:

```text
one workstation
  disk

team / low-latency LAN
  disk → Redis

team / durable cross-machine or CI reuse
  disk → S3/R2/GCS/Azure

large team needing fast + durable hierarchy
  disk → Redis → object store

GitHub Actions
  GHA alone or disk → GHA after measurement/acceptance testing
```

For 0.17.0 multi-level deployments, acceptance-test **remote hit → local backfill → subsequent local hit**, particularly when combining read-only remote tiers with a writable local tier and/or client-side mode.

Security default:

```text
trusted branch / trusted CI
  shared read + write

untrusted fork / external code
  shared read-only
  OR isolated cache
  OR no remote cache
```

Rust cacheability default:

```text
cargo build is the primary cache-producing/validation workload
ordinary cargo check is not

rlib/staticlib-style link-producing units
  principal cacheable surface

bin/dylib/cdylib/proc-macro linker-driving units
  non-cacheable

rustc incremental on a Rust invocation
  incompatible with that invocation being cached by sccache

procedural-macro filesystem I/O
  explicit correctness caveat

final linking
  separate build-time floor
```

And the governing optimization rule is:

> **Use sccache to maximize safe reuse across build contexts; use rustc incremental compilation to maximize reuse inside one persistent edit context. Make the former the shared engineering contract when worktrees, agents, clean builds, or CI matter, and retain the latter as a measured local specialization—not an accidental mixed configuration.**
