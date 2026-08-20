# Rust-Core / Python-Interface Repository — Comprehensive Development & Tooling Specification

**Reference date:** 2026-08-20  
**Primary audience:** LLM programming agents and engineers maintaining a small-to-medium Rust-first project with a Python interface  
**Architecture stance:** one Rust package/crate by default; Python is the interface layer over a private PyO3 native module; additional Rust crates are introduced only for demonstrable package/build reasons; **the semantic organization of production source files and folders is intentionally outside this specification**  
**Primary platforms:** Linux and macOS  
**Revision scope:** production Rust/Python semantic file and folder decomposition is explicitly unspecified; this document governs package/repository/tooling architecture only  

---

## Executive specification

This repository SHALL begin as **one Cargo package containing one primary Rust library crate**, not as a workspace of many internal crates. The package may additionally produce a `cdylib` from the same library target so Maturin can expose a private PyO3 extension to Python.

Python SHALL be treated as the **interface layer** and Rust as the **implementation core**. The architectural requirement is dependency direction and language-boundary ownership, not a prescribed internal source tree.

The intended dependency direction is one-way:

```text
Python caller
    │
    ▼
public Python package
python/<package>/...
    │
    ▼
private native extension
<package>._native
    │
    ▼
PyO3 binding/adaptation boundary
(location within the Rust crate is repository-defined)
    │
    ▼
Rust core implementation
(internal files/modules/folders intentionally unspecified)
```

The reverse direction is forbidden at the architectural level: the Rust core must not require the Python façade, Python object ownership, or Python-facing exception types to function as a normal Rust library. Exactly **how the Rust core or Python façade is divided into production files, modules, or semantic folders is not governed by this document**.

The repository SHALL expose a version-controlled operational contract through `just`. Humans, LLM agents, and CI SHOULD invoke stable recipes such as `just check`, `just test`, `just ci-fast`, `just ci-pr`, `just coverage`, and `just wheel-test` instead of reconstructing tool flags from memory.

The installed development stack SHALL be treated as a set of complementary evidence generators rather than a monolithic test suite. Cheap compile/lint/test feedback runs continuously; coverage, feature matrices, dependency policy, and wheel validation run at pull-request cadence; Miri, fuzzing, mutation testing, heavy compatibility matrices, profiling, and deep supply-chain work are risk-triggered or scheduled.

### Explicit scope boundary: production source organization

This specification intentionally does **not** decide:

- which production concepts deserve their own Rust source files;
- whether a Rust concept should be a file, inline module, nested module, or folder;
- how domain functionality should be grouped or named;
- whether Python façade functionality belongs in one file or several;
- whether a helper belongs beside a caller or in a shared internal module;
- naming conventions for semantic production-code directories.

Those are design decisions to be made from the concrete application domain and can evolve without changing the repository/tooling contract defined here.

# Part I — Repository architecture and package boundaries

## 0) Design goals and non-goals

### 0.1 Goals

The repository design SHALL optimize for:

1. **fast ordinary iteration** — minimize unnecessary compilation units and test executables;
2. **clear architecture** — Rust core separated from Python adaptation without package proliferation;
3. **strong agent legibility** — predictable locations, explicit commands, narrow responsibility boundaries;
4. **high assurance when needed** — the installed tooling can be escalated according to change risk;
5. **reproducibility** — compiler/tool configuration and commands live in the repository;
6. **small-project ergonomics** — avoid infrastructure whose maintenance cost exceeds its value;
7. **package correctness** — a local import is not considered proof that a distributable Python wheel is valid;
8. **honest performance evidence** — profiles identify candidates; controlled benchmarks verify outcomes;
9. **bounded build-artifact growth** — sensible profiles and target separation without reflexive `cargo clean`;
10. **easy graduation** — the layout can become a small workspace later without forcing one prematurely.

### 0.2 Non-goals

At inception the repository does **not** optimize for:

- independently versioned internal Rust libraries;
- dozens of separately publishable crates;
- every possible Python interpreter or OS/architecture;
- continuous execution of every expensive assurance tool;
- strict supply-chain audit certification if the project remains a personal hobby project;
- a microservice topology;
- a plugin framework before one is actually needed.

### 0.3 Core rule: crate boundaries are package/build boundaries

This specification distinguishes **crate topology** from **source organization**.

A Rust crate is a compilation/dependency unit. A new crate therefore requires an explicit package-level or build-level justification, such as:

1. independent reuse or publication;
2. hard dependency isolation that truly requires a separate package;
3. materially distinct platform/runtime/dependency requirements;
4. independent version/release lifecycle;
5. a separately built artifact that genuinely needs its own package; or
6. **measured** compilation benefit from a crate boundary.

Creating a new crate solely because the production code has another conceptual area, has grown in line count, or could be placed in another folder is not sufficient justification.

What replaces that crate internally—one file, many files, inline modules, nested directories, or another source organization—is **deliberately outside scope**. The only initial structural requirement is that the Rust implementation remains within the single package/crate unless a real crate-boundary justification appears.

## 1) Canonical repository topology

Use this as the starting **repository/tooling topology**. Production-source internals are represented by ellipses intentionally.

```text
myproject/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── pyproject.toml
├── uv.lock
├── .python-version
├── README.md
│
├── src/                                   # Rust library crate source
│   ├── lib.rs                             # conventional library crate root
│   └── ...                                # production Rust organization: unspecified
│
├── python/                                # Python package source root for Maturin mixed layout
│   └── myproject/
│       ├── __init__.py
│       ├── py.typed                       # PEP 561 marker
│       ├── _native.pyi                    # native-extension type stub if maintained manually
│       └── ...                            # Python interface organization: unspecified
│
├── tests/                                 # Rust external integration-test area
│   ├── integration.rs                     # preferred single top-level integration-test target
│   ├── integration/                       # internal modules for that test target
│   │   └── ...
│   └── fixtures/
│       └── ...
│
├── python_tests/                          # Python public-interface/package tests
│   └── ...
│
├── benches/                               # optional, when stable performance workloads exist
│   └── ...
├── fuzz/                                  # optional, when a fuzz-worthy input surface exists
│   ├── Cargo.toml
│   ├── fuzz_targets/
│   ├── corpus/
│   └── artifacts/
│
├── .cargo/
│   └── config.toml
├── .config/
│   └── nextest.toml
├── bacon.toml
├── deny.toml
├── _typos.toml
├── justfile
│
├── supply-chain/                          # cargo-vet state; optional until adopted
│   ├── config.toml
│   ├── audits.toml
│   └── exemptions.toml
│
├── scripts/                               # reproducible operational scripts
│   ├── wheel_test.sh
│   ├── tooling_inventory.sh
│   └── ...
│
├── .github/
│   └── workflows/
│       ├── ci.yml
│       ├── deep-assurance.yml
│       └── wheels.yml
│
├── target/                                # generated; ignored
├── dist/                                  # generated wheels/sdists; ignored or retained as release artifacts elsewhere
└── .gitignore
```

### 1.1 Why there is no top-level `crates/` initially

A `crates/` directory is a convention for a multi-package workspace. It carries no intrinsic benefit for a repository that currently needs only one Rust package and can encourage accidental proliferation of compilation units.

The default is therefore:

```text
one Cargo package
    -> one primary Rust library crate
    -> internal Rust source topology chosen by the application
    -> one Python extension produced from that package
```

If the project later develops a second genuine package, introducing a workspace and a `crates/` convention may become reasonable; see Part XIV.

### 1.2 Why Python gets its own source root

`python/myproject/` is a packaging/language-boundary choice, not a semantic decomposition rule. It works naturally with Maturin's mixed-project layout and keeps Python package sources separate from Cargo's conventional Rust `src/` root.

No further judgment about how production functionality is divided **inside** `src/` or `python/myproject/` is made here.

## 2) Internal production source organization is intentionally unspecified

The Rust crate may use any source organization that remains understandable and maintainable for the project. This specification does not prefer domain-oriented modules, type-oriented modules, flat files, nested module trees, or any other semantic organization.

Likewise, the Python package may use one façade file or many files and subpackages. That decision is not part of the repository/tooling standard.

The constraints that **are** in scope are only architectural/tooling constraints:

- remain one Rust package/crate initially unless §0.3 justifies another crate;
- preserve the Rust-core / Python-interface dependency direction;
- ensure the package can build as ordinary Rust without requiring a Python runtime when that is part of the intended contract;
- ensure the Maturin/PyO3 build surface is explicit and testable;
- keep public API and packaging behavior covered by appropriate tests;
- avoid source-layout choices that accidentally multiply Cargo packages or top-level integration-test crates without reason.

An LLM programming agent MUST NOT infer a preferred production file/folder decomposition from examples elsewhere in this document. Any path shown for a production source file is illustrative unless explicitly identified as a Cargo, Python-package, test, tooling, or generated-artifact convention.

## 3) Binary targets, if the project needs them

If the project also has a CLI or diagnostic executable, keep it in the same package unless it is independently distributed or dependency-isolated.

For one primary executable:

```text
src/main.rs
```

For optional utilities:

```text
src/bin/
├── inspect.rs
└── benchmark_fixture.rs
```

Where an executable exists, prefer reusing the library target rather than duplicating implementation:

```rust
fn main() -> anyhow::Result<()> {
    myproject::run_cli()
}
```

Avoid maintaining a second implementation of the same behavior solely because an executable target exists.

---

## 4) Rust test topology: avoid test-crate explosion

### 4.1 Colocate most tests

The default test lives next to the implementation:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_state() {
        // ...
    }
}
```

Benefits:

- private invariants are testable without widening visibility;
- agents see tests while editing implementation;
- refactors move implementation and tests together;
- fewer top-level test compilation units are created.

### 4.2 Group external integration tests into one top-level target

Cargo treats every top-level `tests/*.rs` file as a separate integration-test crate. Therefore use:

```text
tests/
├── integration.rs
└── integration/
    ├── happy_path.rs
    ├── errors.rs
    └── persistence.rs
```

`tests/integration.rs`:

```rust
mod integration {
    mod errors;
    mod happy_path;
    mod persistence;
}
```

This preserves organization while producing one external integration-test crate rather than one executable per file.

### 4.3 When multiple integration-test targets are justified

Use a separate top-level test target only when it needs a materially different:

- feature set;
- process environment;
- test harness;
- external service setup;
- platform restriction; or
- resource group.

Do not create one per subsystem.

### 4.4 Fixtures

Put reusable non-code test data in `tests/fixtures/`. Prefer small, deterministic, human-inspectable fixtures. Large generated corpora should be reproducibly generated or stored in a dedicated test-data strategy rather than casually committed.

---

# Part II — Rust ↔ Python boundary

## 5) Python package and native-module boundary

### 5.1 Public package versus private native module

The native module SHALL be private by convention:

```text
myproject             public Python package
myproject._native     implementation extension
```

Users should import from the public package rather than depend directly on `_native`. The public package is the supported Python contract; `_native` is an implementation detail whose symbol layout may change as bindings evolve.

Exactly how the public Python package is divided into `.py` files or subpackages is outside this specification.

### 5.2 `__init__.py` defines the package-root contract

The package root should expose only the names intentionally supported at that import path. This is an API/packaging rule, not a prescription for where those names are implemented.

The implementation may re-export from one or many internal files; the semantic layout is repository-defined.

### 5.3 Python remains the interface layer

The required architectural property is that Python does not become a second independent implementation of core behavior. Python may provide interface-oriented adaptation such as ergonomic argument handling, typing, compatibility/deprecation shims, and presentation of Rust results/errors.

This section intentionally does not prescribe which Python file owns any such behavior. The relevant invariant is **single-source implementation semantics in Rust, with Python adapting that implementation for Python callers**.

## 6) PyO3 binding layer

### 6.1 Keep PyO3 behind a feature

The normal Rust core must remain buildable without Python:

```toml
[features]
default = []
python = ["dep:pyo3"]
```

and:

```rust
#[cfg(feature = "python")]
mod python;
```

This gives the project two deliberate compile surfaces:

```text
pure Rust core:       cargo check
Python extension:     maturin / cargo check --features python
```

### 6.2 Do not use the legacy `pyo3/extension-module` feature

Modern PyO3 deprecates the historical Cargo feature for ordinary Maturin builds. Maturin sets the extension-build environment for the build that needs it, avoiding the old behavior where enabling `extension-module` globally could interfere with normal Rust test/link workflows.

The dependency should therefore resemble:

```toml
pyo3 = { version = "0.29.2", optional = true }
```

not:

```toml
pyo3 = { version = "0.29.2", optional = true, features = ["extension-module"] }
```

Pin or constrain versions according to repository policy rather than copying these exact numbers indefinitely.

### 6.3 Native-module declaration

Use PyO3's current declarative module form for the private `_native` extension. The exact Rust file/module that contains this declaration is repository-defined.

Illustrative shape:

```rust
#[pyo3::pymodule(name = "_native")]
mod python_module {
    // Export binding functions/classes here or re-export them from
    // whatever internal Rust source organization the project chooses.
}
```

The declared Python module name `_native` must match the final component of Maturin's `module-name = "myproject._native"`.

### 6.4 Core contract remains Python-agnostic

The Rust implementation consumed by the binding layer should expose ordinary Rust types and `Result`-style errors rather than requiring `PyAny`, `PyObject`, or Python exception types throughout the core implementation.

For example, the architectural boundary should resemble:

```text
ordinary Rust inputs/results
        │
        ▼
PyO3 conversion / call adapter
        │
        ▼
Python objects / exceptions
```

The **location** of the conversion/adapter code is deliberately unspecified. It may be one module, several modules, or colocated with other code as long as Python-specific dependencies do not become a prerequisite for the pure Rust core contract.

### 6.5 Error translation is a boundary responsibility

Rust errors presented through Python need a consistent Python-facing mapping. The mapping MAY be centralized or distributed according to the application's design; this specification does not require an `errors.rs`, `exceptions.py`, or any other semantic file.

The invariant is behavioral: equivalent Rust error classes should map predictably to the documented Python exception contract, and that contract should be covered by Python-facing tests.

## 7) GIL/free-threaded behavior and long Rust work

The binding layer should minimize time for which Python execution rights are held when the operation is pure Rust and does not touch Python objects.

Conceptually:

```rust
#[pyfunction]
fn run_query(py: Python<'_>, query: String) -> PyResult<String> {
    let result = py.detach(|| crate::engine::run_query(query));
    result.map_err(errors::to_python)
}
```

Use this pattern only when the closure truly does not access Python-bound values. Convert inputs before detaching and convert outputs after reattachment.

For heavy data movement, prefer coarse-grained calls:

```text
bad:
Python loop -> native call per row -> Python loop -> native call per row

better:
Python passes batch/table/query -> Rust processes batch -> Python receives result
```

This reduces FFI crossings, Python object churn, and serialization overhead.

Free-threaded Python support, if advertised, SHALL be treated as an explicit compatibility target rather than assumed from Rust thread safety. Test the actual supported interpreter mode and audit Python-object access rules.

---

## 8) Type information for Python users

Ship:

```text
python/myproject/py.typed
python/myproject/_native.pyi
```

The `.pyi` file documents the native module's callable/type surface for static type checkers. The public Python façade should carry ordinary inline annotations.

Do not rely on native runtime signatures alone as the public typing contract. A private native stub plus a typed façade allows the Python API to remain richer and more stable than the raw FFI surface.

---

# Part III — Manifest and environment specification

## 9) `Cargo.toml` baseline

A practical starting manifest:

```toml
[package]
name = "myproject"
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"

[lib]
name = "myproject"
crate-type = ["rlib", "cdylib"]

[features]
default = []
python = ["dep:pyo3"]

[dependencies]
pyo3 = { version = "0.29.2", optional = true }

[dev-dependencies]
# Keep this list intentionally small; add test-only crates only when justified.

[lints.rust]
unsafe_code = "deny"

[lints.clippy]
all = "warn"
pedantic = "warn"

[profile.dev]
debug = "line-tables-only"

[profile.dev.package."*"]
debug = false

[profile.test]
debug = "line-tables-only"

[profile.test.package."*"]
debug = false

[profile.debugging]
inherits = "dev"
debug = "full"

[profile.profiling]
inherits = "release"
debug = "full"
strip = "none"
```

### 9.1 Lint policy should be adapted, not cargo-culted

`clippy::pedantic` is useful but intentionally opinionated. If a warning is inappropriate, suppress the specific lint at the narrowest scope with a reason. Do not globally weaken Clippy simply to produce a green build.

If the project legitimately needs `unsafe`, replace crate-wide `unsafe_code = "deny"` with a policy such as `warn` plus explicitly reviewed modules. The key rule is that unsafe surface is intentional and visible.

### 9.2 Development debug-information policy

Full debug information for every dependency can dramatically inflate `target/`. `line-tables-only` for first-party dev/test code plus `debug = false` for dependency packages usually gives useful source locations while constraining artifact growth. Keep a separate `debugging` profile when full debugger-quality information is required.

### 9.3 Do not over-optimize release profiles by folklore

Do not automatically add:

```text
lto = "fat"
codegen-units = 1
panic = "abort"
opt-level = 3
strip = true
```

without a reason. These can change compile time, debuggability, binary size, runtime behavior, or profiling quality. Measure the actual workload with Hyperfine and inspect artifacts before committing tuning.

---

## 10) `rust-toolchain.toml`

For this project, use stable as the repository default:

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

Nightly remains a **targeted analysis toolchain** for Miri and cargo-udeps. If a later functionality uses `rustc-dev`/compiler-private APIs, create an exact date-pinned nightly contract for that subsystem or upgrade the repository intentionally. Do not make rolling nightly the default merely because the workstation has it installed.

Recommended targeted invocation pattern:

```bash
cargo +nightly miri test
cargo +nightly udeps --all-targets
```

For reproducible CI, replace symbolic `nightly` with an exact nightly date owned by the workflow or repository configuration.

---

## 11) `pyproject.toml` baseline

Use Maturin as the build backend and uv for environment/lock management:

```toml
[build-system]
requires = ["maturin>=1.14,<2"]
build-backend = "maturin"

[project]
name = "myproject"
version = "0.1.0"
requires-python = ">=3.12"
dependencies = []

[tool.maturin]
python-source = "python"
module-name = "myproject._native"
features = ["python"]

[dependency-groups]
dev = [
  "maturin>=1.14,<2",
  "pytest>=8",
  "ruff",
  "pyrefly",
]

[tool.pytest.ini_options]
testpaths = ["python_tests"]
addopts = ["-ra"]

[tool.ruff]
target-version = "py312"
line-length = 100

[tool.ruff.lint]
select = ["E", "F", "I", "UP", "B", "SIM"]
ignore = ["E501"]

[tool.ruff.format]
docstring-code-format = true

[tool.pyrefly]
project-includes = [
  "python/**/*.py",
  "python_tests/**/*.py",
]
python-version = "3.12"
```

### 11.1 Python-version policy

`>=3.12` is a reasonable *example baseline* for a new hobby project in 2026, not a universal requirement. If another floor is selected, update all of these together:

```text
project.requires-python
.python-version
Ruff target-version
Pyrefly python-version
CI Python matrix
Maturin wheel matrix
```

Avoid claiming support for Python versions that are never installed and tested.

### 11.2 Python dependencies

Published runtime dependencies belong in `project.dependencies`. Local-only developer tools belong in standardized `[dependency-groups]`, which uv resolves and locks. Do not make lint/test tooling runtime dependencies of the package.

---

## 12) `.python-version` and uv

Pin the preferred local interpreter:

```text
3.12
```

Typical bootstrap:

```bash
uv python install
uv sync
```

Normal Python command execution should go through uv:

```bash
uv run pytest
uv run ruff check .
uv run ruff format --check .
uv run pyrefly check
uv run maturin develop
```

Commit `uv.lock` for this application/hobby project so development and CI resolve the same Python tool/dependency graph.

---

# Part IV — Repository-owned command and feedback layer

## 13) `.cargo/config.toml` and sccache

The repository SHOULD make compiler caching explicit if `sccache` is considered part of the required development environment:

```toml
[build]
rustc-wrapper = "sccache"
```

This is preferable to relying on an undocumented shell environment because agents and CI can see the intended wrapper.

### 13.1 Caveat: repository config implies a tool prerequisite

A committed `rustc-wrapper = "sccache"` means contributors without `sccache` cannot build until it is installed. For a personal project this is usually acceptable if the bootstrap documentation installs the tool. If portability to arbitrary contributors is more important, keep `RUSTC_WRAPPER=sccache` in local/CI environment configuration instead.

### 13.2 Monitor cache effectiveness

Expose:

```bash
sccache --show-stats
sccache --zero-stats
```

through `just`. Do not assume caching is useful merely because it is enabled. Watch cache hit rate after normal edit/test cycles.

### 13.3 Never use `cargo clean` as routine hygiene

Incremental compilation and sccache exist to avoid repeated work. `cargo clean` should be reserved for:

- suspected stale/corrupt artifacts;
- controlled clean-build measurement;
- reclaiming disk intentionally;
- diagnosing profile/feature interactions that cannot be isolated another way.

A large `target/` tree is not proof that the final binary or wheel is bloated. Analyze final artifacts independently.

---

## 14) `justfile` as the operational API

The `justfile` SHALL be the first command surface an LLM agent inspects. Recipes should express **intent**, not expose a forest of tool-specific flags.

A recommended baseline follows. Adjust package names and platform-specific shell behavior as needed.

```just
set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

# -------------------- environment --------------------

doctor:
    rustc -vV
    cargo -V
    rustup show active-toolchain
    rustup component list --installed
    cargo install --list
    uv --version
    uv run python --version
    uv run maturin --version
    uv run ruff --version
    uv run pyrefly --version
    sccache --show-stats || true

metadata:
    cargo metadata --format-version 1 --no-deps

cache-stats:
    sccache --show-stats

cache-zero-stats:
    sccache --zero-stats

# -------------------- formatting / static feedback --------------------

fmt:
    cargo fmt --all -- --check
    uv run ruff format --check python python_tests

fmt-write:
    cargo fmt --all
    uv run ruff format python python_tests

check:
    cargo check --all-targets
    cargo check --all-targets --features python

clippy:
    cargo clippy --all-targets -- -D warnings
    cargo clippy --all-targets --features python -- -D warnings

python-lint:
    uv run ruff check python python_tests

python-type:
    uv run pyrefly check

typos:
    typos

# -------------------- tests --------------------

test-rust:
    cargo nextest run

doctest:
    cargo test --doc

python-develop:
    uv run maturin develop

test-python: python-develop
    uv run pytest

test: test-rust doctest test-python

# -------------------- fast / PR gates --------------------

deps-fast:
    cargo machete
    cargo shear --deny-warnings

policy:
    cargo deny check
    cargo audit

ci-fast: fmt check clippy python-lint python-type test typos deps-fast

ci-pr: ci-fast policy
    cargo nextest run --features python -P ci
    cargo test --doc --features python
    cargo insta pending-snapshots

# -------------------- coverage / test quality --------------------

coverage:
    mkdir -p target/coverage
    cargo llvm-cov nextest \
      --all-features \
      --lcov \
      --output-path target/coverage/lcov.info

snapshots-review:
    cargo insta review

# Mutating: never a dependency of CI.
snapshots-accept:
    cargo insta accept

mutants-file path:
    cargo mutants -f {{path}}

miri:
    cargo +nightly miri test

miri-seeds seeds="16":
    MIRIFLAGS="-Zmiri-many-seeds=0..{{seeds}}" cargo +nightly miri test

udeps:
    cargo +nightly udeps --all-targets --all-features

fuzz target seconds="60":
    cargo fuzz run {{target}} -- -max_total_time={{seconds}}

fuzz-coverage target:
    cargo fuzz coverage {{target}}

# -------------------- feature / compatibility --------------------

features-each:
    cargo hack check --each-feature

features-no-default:
    cargo hack check --no-default-features

features-python:
    cargo check --features python

msrv:
    cargo msrv verify

semver baseline:
    cargo semver-checks --baseline-rev {{baseline}}

# -------------------- dependency / supply chain --------------------

vet:
    cargo vet

unsafe-surface:
    cargo geiger

# Mutating. Review manifest diff afterwards.
deps-fix:
    cargo shear --fix

# -------------------- package validation --------------------

wheel:
    rm -rf dist
    uv run maturin build --release --out dist

wheel-test: wheel
    bash scripts/wheel_test.sh

# -------------------- artifact / performance investigation --------------------

bloat:
    cargo bloat --release --crates

symbols:
    cargo nm --release

sections:
    cargo size --release

asm:
    cargo asm

profile-build:
    cargo build --profile profiling

# -------------------- maintenance --------------------

tool-updates-check:
    cargo install-update --list || cargo install-update --help
```

### 14.1 Mutating recipes are never hidden dependencies

These MUST remain explicit:

```text
fmt-write
snapshots-accept
deps-fix
typos -w
cargo vet certify
cargo update
cargo install-update -a
maturin publish
```

An agent may run read-only checks autonomously when authorized to work in the repository, but source/environment/release mutation needs an explicit reason and resulting diff inspection.

### 14.2 Separate Python development install from wheel validation

`test-python` intentionally uses `maturin develop` for speed. `wheel-test` is a separate artifact gate. This prevents the common error of treating a development install as packaging evidence.

---

## 15) Bacon: one Rust-owned continuous check

Initialize from the installed version:

```bash
bacon --init
```

Then keep jobs cheap and explicit:

```toml
default_job = "check"

[jobs.check]
command = ["cargo", "check", "--all-targets"]

[jobs.check-python-feature]
command = ["cargo", "check", "--all-targets", "--features", "python"]

[jobs.nextest]
command = ["cargo", "nextest", "run"]
need_stdout = true
analyzer = "nextest"
```

### 15.1 Ownership rule

Only one background system should continuously run the expensive Rust check.

Recommended division:

```text
rust-analyzer -> semantic model, diagnostics, references, assists
Bacon         -> persistent Cargo/rustc check job
Watchexec     -> arbitrary non-Rust task or process restart only
```

Do not simultaneously configure editor check-on-save, Bacon, and Watchexec to run the same full `cargo check` or Clippy command.

### 15.2 Optional machine-readable Bacon locations

If the installed Bacon supports it, export `.bacon-locations` for editor/agent consumption and add it to `.gitignore`. An agent must verify the output corresponds to the current source generation before treating an empty list as success.

---

## 16) Watchexec: targeted orchestration, not a second Rust checker

Useful roles:

```bash
# Restart an executable on Rust/config changes.
watchexec -e rs,toml -r -- cargo run

# Rebuild/install the Python extension only when relevant files change.
watchexec -w src -w Cargo.toml -w pyproject.toml -- uv run maturin develop

# Trigger interface tests on Python edits.
watchexec -w python -w python_tests -- uv run pytest
```

Do not use all of these continuously at once by default. The main value of Watchexec is arbitrary filesystem-triggered orchestration and process lifecycle control; Bacon remains the normal Rust diagnostics loop.

---

## 17) `_typos.toml`

A sensible baseline:

```toml
[files]
extend-exclude = [
  "target/",
  "dist/",
  ".venv/",
  "fuzz/corpus/",
  "fuzz/artifacts/",
]

[default.extend-words]
# Add only genuine project/domain vocabulary here.
```

Do not broadly exclude source, snapshots, error messages, or stubs. Misspellings in a serialized field, exception name, or public Python symbol can become an API defect.

Never run `typos -w` over the repository without inspecting the diff; identifier and protocol-string corrections can be breaking changes.

---

# Part V — Test architecture and evidence model

## 18) Ordinary Rust tests: cargo-nextest + doctests

Use nextest as the default test runner:

```bash
cargo nextest run
```

A practical `.config/nextest.toml`:

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
```

### 18.1 Retries are diagnostic

If a test fails first and passes on retry, it is flaky evidence, not a clean pass. Preserve `flaky-result = "fail"` unless there is an explicit temporary policy.

### 18.2 Doctests are separate

Nextest does not replace:

```bash
cargo test --doc
```

An agent must not report “all Rust tests passed” from nextest alone when doctests exist.

### 18.3 Shared-resource tests

If a handful of integration tests share a database, fixed port, GPU, or heavyweight fixture, use nextest test groups/concurrency limits. Do not serialize the entire suite with one global test thread.

---

## 19) Python interface tests with pytest

Python tests SHOULD exercise the **public Python package**, not the private native module, except for narrowly scoped binding-contract tests.

Primary categories:

```text
python_tests/
├── test_api.py            public functions/classes and Pythonic behavior
├── test_errors.py         exception mapping and messages/contracts
├── test_conversions.py    Python <-> Rust boundary cases
├── test_threading.py      detach/thread behavior if relevant
└── test_packaging.py      import/package artifact smoke checks
```

### 19.1 No domain logic duplication

If the same semantic case is already exhaustively tested in Rust, Python generally needs only enough tests to prove:

1. the interface accepts expected Python values;
2. conversion to the Rust core is correct;
3. results convert back correctly;
4. errors map correctly;
5. packaging/import behavior works.

Do not replay thousands of core-algorithm cases through Python unless cross-language behavior itself is under test.

### 19.2 Public API versus native API tests

Default:

```python
from myproject import run_query
```

Only explicit adapter tests should do:

```python
from myproject import _native
```

This keeps the raw extension free to evolve internally.

---

## 20) Snapshot testing with Insta

Use snapshot tests only where the output is large, structured, and reviewable:

- normalized query/plan structures;
- parser or compiler output;
- diagnostics;
- schema or metadata structures;
- stable serialized representations.

Do not snapshot every scalar result.

Preferred lifecycle:

```text
run tests
  -> pending snapshot diff
  -> inspect semantic change
  -> accept only intended deltas
  -> rerun cleanly
```

Never make `cargo insta accept` part of CI or an automatic agent “fix.”

### 20.1 Normalize true nondeterminism

Normalize timestamps, temp paths, random IDs, and unordered collections only when those values are not part of the contract. Do not redact fields simply because they make a regression visible.

---

## 21) Source coverage with cargo-llvm-cov

Ordinary Rust coverage:

```bash
cargo llvm-cov nextest --all-features --html
```

CI artifact:

```bash
cargo llvm-cov nextest \
  --all-features \
  --lcov \
  --output-path target/coverage/lcov.info
```

Coverage answers **what executed**, not whether tests asserted meaningful behavior.

### 21.1 Avoid arbitrary percentage worship

A threshold may be useful once the project has a stable baseline, but do not adopt a number solely because the tool supports `--fail-under-lines`. Prefer changed-critical-module coverage and uncovered-branch investigation over synthetic score optimization.

### 21.2 Coverage across Python -> Rust

Python tests can exercise native Rust code that ordinary Rust tests do not. For complete cross-language coverage:

```text
cargo-llvm-cov exports instrumentation environment
        │
        ▼
Maturin builds the extension with those Rust flags
        │
        ▼
Python loads that exact instrumented extension
        │
        ▼
pytest exercises public Python API
        │
        ▼
LLVM raw profile data
        │
        ▼
merge + render Rust coverage report
```

Because exact `cargo-llvm-cov` external-test environment flags can evolve, the repository script SHOULD derive them from the installed tool (`cargo llvm-cov show-env --help`) rather than hard-code undocumented internal paths.

Keep this workflow in `scripts/coverage_python.sh` or an equivalent recipe once the extra coverage is valuable; it need not exist on day one.

---

## 22) Mutation testing with cargo-mutants

Mutation testing is expensive but particularly useful for core validation/state-transition logic.

Recommended local/risk-triggered use:

```bash
cargo mutants -f <changed-rust-source-file>
```

A survivor means “this plausible source mutation did not cause the selected tests to fail.” It can indicate:

- missing assertions;
- unreachable behavior;
- an equivalent mutant;
- the wrong test scope;
- weak error-path testing.

It is not automatically a bug.

### 22.1 Combine coverage and mutation evidence

```text
uncovered + surviving mutant -> first establish reachability
covered + surviving mutant   -> strengthen behavioral assertion
covered + killed mutant      -> stronger evidence of a meaningful test
```

Run focused mutation campaigns on changed high-risk modules; reserve full-project campaigns for scheduled work.

---

## 23) Fuzzing with cargo-fuzz

Add `fuzz/` only when the project has a suitable surface such as:

- parser;
- decoder/serializer;
- query syntax;
- file format;
- protocol;
- compact state-machine input;
- unsafe buffer manipulation.

Do not add an empty fuzz package merely because cargo-fuzz is installed.

### 23.1 Target design

A fuzz target should be:

- deterministic;
- fast;
- mostly pure;
- coarse enough to reach meaningful semantics;
- free of uncontrolled network/database effects;
- implemented by calling production code rather than duplicating it.

### 23.2 Findings become ordinary regressions

```text
fuzzer discovers crash/mismatch
    -> minimize input
    -> understand root cause
    -> add deterministic Rust regression test
    -> preserve useful corpus seed
    -> run Miri on minimized case if unsafe path involved
```

Use bounded interactive runs:

```bash
cargo fuzz run parser -- -max_total_time=60
```

Long-running campaigns belong in scheduled infrastructure.

---

## 24) Miri for unsafe/concurrent risk

If the code remains fully safe Rust, Miri can be occasional. It becomes mandatory risk-triggered evidence when changes touch:

- `unsafe`;
- raw pointers;
- FFI wrappers;
- atomics/custom concurrency;
- `MaybeUninit`;
- manual `Send`/`Sync`;
- aliasing/provenance-sensitive code.

Commands:

```bash
cargo +nightly miri test
MIRIFLAGS="-Zmiri-many-seeds=0..16" cargo +nightly miri test
```

### 24.1 Python FFI caveat

Miri may not execute the real CPython native boundary. Split the assurance model:

```text
pure Rust invariants/conversions -> Miri where supported
PyO3/CPython integration         -> native Python tests
wheel ABI/import behavior        -> clean wheel installation tests
```

Do not disable Miri isolation broadly to force unsupported integration paths through it.

### 24.2 Miri is exploration, not proof

Record:

- toolchain/nightly;
- test subset;
- target;
- seed range;
- exclusions or unsupported operations.

Never report one green Miri run as proof of soundness.

---

## 25) Test-quality triangulation

The project SHALL use the tools according to the question each answers:

| Question | Tool |
|---|---|
| Do normal tests pass reliably? | nextest + pytest + doctests |
| Which Rust regions executed? | cargo-llvm-cov |
| Do assertions detect plausible faults? | cargo-mutants |
| What unanticipated inputs find new behavior? | cargo-fuzz |
| Did complex structured output change? | Insta |
| Did selected unsafe/concurrent executions violate Rust validity rules? | Miri |
| Does the built wheel actually install/import? | Maturin clean-wheel test |

A best-in-class result comes from **orthogonal evidence**, not from running more copies of the same kind of check.

---

# Part VI — Feature, dependency, supply-chain, and compatibility controls

## 26) Keep Cargo features few and semantically meaningful

For the initial repository, the expected feature surface is intentionally small:

```toml
[features]
default = []
python = ["dep:pyo3"]
```

Do not introduce a feature for every internal module. Features should represent user/build capabilities that legitimately alter dependencies or compiled functionality.

Every feature increases the configuration state space. `cargo test --all-features` validates only the maximal additive union and can hide accidental coupling.

### 26.1 Feature validation with cargo-hack

At minimum:

```bash
cargo hack check --each-feature
cargo hack check --no-default-features
cargo check --features python
```

As the feature count grows, define valid groups/combinations rather than reflexively running the complete powerset. A powerset grows exponentially and can turn assurance into permanently ignored CI.

### 26.2 The Python feature is special

The core contract is:

```text
no features -> pure Rust library compiles/tests
python       -> Rust library + PyO3 adapter compiles
Maturin      -> builds the Python extension using python feature
```

A Python-only dependency must not accidentally leak into the featureless core.

---

## 27) MSRV policy with cargo-msrv

For a private hobby application, a formal long-lived Minimum Supported Rust Version may have little value. The simplest defensible policy is often:

```text
repository toolchain / current stable is supported
older compilers are not promised
```

If the Rust library later becomes public/reusable, add a `rust-version` commitment to `Cargo.toml` and validate it:

```bash
cargo msrv verify
```

Use `cargo msrv find` to investigate the actual floor, not as an automatic reason to advertise the oldest compiler that happens to work.

A real MSRV promise must include representative feature states, especially `python` if users are expected to build that Rust feature directly.

---

## 28) Semver checks: apply to the API you actually promise

`cargo-semver-checks` is valuable if the Rust crate itself has downstream Rust consumers. If Python is the only supported external interface and the Rust crate is implementation-private, the Rust public API is primarily an internal architecture surface and strict semver gating may be unnecessary.

If a Rust API becomes supported externally:

```bash
cargo semver-checks --baseline-rev <last-release-tag>
```

Record:

- baseline revision/version;
- feature set;
- target;
- intended release classification.

### 28.1 Python API compatibility requires different evidence

`cargo-semver-checks` cannot prove Python API compatibility. Use:

- typed public façade tests;
- API/snapshot or signature contracts where useful;
- wheel-install tests;
- explicit deprecation policy;
- release notes.

Do not label the entire package “semver compatible” because the Rust API checker passed.

---

## 29) Unused-dependency analysis: Machete -> Shear -> Udeps

The three installed tools occupy different fidelity/cost points.

```text
cargo-machete -> very fast heuristic hint
cargo-shear   -> primary static workspace hygiene gate
cargo-udeps   -> heavier nightly/compiler-oriented adjudication
```

### 29.1 Routine gate

```bash
cargo machete
cargo shear --deny-warnings
```

### 29.2 Disputed/high-impact finding

```bash
cargo +nightly udeps --all-targets --all-features
```

Before removing a dependency, inspect:

- feature-gated use;
- build scripts;
- procedural/declarative macro expansion;
- examples/benches/tests;
- generated code;
- renamed dependencies;
- platform-specific `cfg`.

If macro-generated use is suspected, `cargo expand` is the natural next tool.

### 29.3 Dependency removal verification

After removal:

```text
cargo check --all-targets
cargo hack check supported feature states
cargo nextest run
cargo test --doc
cargo deny check
cargo audit
Python extension build + pytest if Python-facing graph changed
```

Do not let scanners “vote.” Reconcile what each analysis model actually sees.

---

## 30) `deny.toml`: executable dependency policy

Even a hobby project benefits from a basic policy for:

- licenses;
- Git/registry sources;
- denied crates or versions;
- duplicate-version visibility;
- advisories;
- yanked/unmaintained policy.

Initialize with the installed tool:

```bash
cargo deny init
```

Then edit the generated configuration deliberately.

### 30.1 Do not copy an organization's license policy blindly

The acceptable license set depends on how the project is distributed. A private hobby tool and a commercial redistributable package can have different needs. Define the actual policy and make any exception narrow and documented.

### 30.2 Duplicate dependencies deserve attention, not automatic failure

Multiple major versions can increase compile time and artifact size, but may be unavoidable because of transitive constraints. Pair:

```bash
cargo deny check bans
cargo tree -d
```

and fix only when the dependency paths admit a safe resolution.

---

## 31) cargo-audit: known-vulnerability evidence

Run on PR/dependency changes and periodically:

```bash
cargo audit
```

Use JSON output for machine integration where helpful:

```bash
cargo audit --json
```

A finding requires triage of:

```text
advisory
affected crate/version
dependency path
patched version
target/feature reachability
runtime exposure
remediation or temporary exception
```

A green scan means the resolved graph has no matching known RustSec advisory under the current database; it does not mean dependencies have been comprehensively security-audited.

---

## 32) cargo-vet: optional but high-value trust layer

For a personal hobby project, cargo-vet is **recommended but optional** because maintaining human audit attestations can become substantial work. Adopt it when:

- the project handles sensitive data;
- third-party dependency trust matters strongly;
- the project may become distributed or production-like;
- dependency review is itself an engineering goal.

Initialize:

```bash
cargo vet init
```

Then version-control `supply-chain/` state.

### 32.1 Agent boundary

An LLM agent may:

- identify audit gaps;
- summarize dependency diffs;
- enumerate unsafe/build/proc-macro behavior;
- prepare a human review checklist;
- run `cargo vet diff`/`inspect`.

It must not fabricate human review authority or execute `cargo vet certify` as though autonomous analysis were an accountable human attestation.

---

## 33) cargo-geiger: unsafe-surface inventory

Run when unsafe code or dependencies matter:

```bash
cargo geiger
```

Use it to answer:

- did first-party unsafe surface increase?;
- which dependency introduces most unsafe code?;
- where should Miri/fuzz/manual review focus?;

Do **not** interpret the count as a vulnerability score. One subtle invalid unsafe invariant can matter more than many generated bindings.

If the initial codebase contains no first-party `unsafe`, preserving that state is a useful default. If PyO3 or a performance optimization requires unsafe internals, isolate and document them.

---

## 34) cargo-auditable: provenance for standalone Rust binaries

If the package ships a standalone Rust executable, build it with dependency metadata where practical:

```bash
cargo auditable build --release
cargo audit bin target/release/<binary>
```

This is most valuable when operators may later possess the executable but not its original repository state.

### 34.1 Wheel caveat

A Python wheel containing a native extension has its own package metadata and binary provenance concerns. Do not assume wrapping the Rust build with cargo-auditable automatically creates a complete wheel SBOM or provenance record. For wheels preserve at least:

- source commit;
- `Cargo.lock` hash;
- `uv.lock` hash;
- rustc/Cargo/Maturin versions;
- Python ABI/platform tag;
- wheel filename/hash;
- dependency-policy/audit results;
- clean-install test result.

---

## 35) Dependency-change campaign

Any edit to `Cargo.toml`, `Cargo.lock`, features, patches, Git sources, or build/proc-macro dependencies triggers:

```text
1. Why is the dependency/change needed?
2. cargo metadata / cargo tree
3. Machete + Shear
4. Udeps if disputed or high impact
5. cargo-deny
6. cargo-audit
7. cargo-vet if adopted
8. Geiger delta if security-sensitive
9. cargo-hack feature checks
10. Rust tests/doctests
11. Python extension build + pytest if reachable from interface
12. build-time/binary-size measurement if material
```

### 35.1 Add-dependency decision record

Before adding a substantial dependency, answer:

```text
[ ] Does an existing dependency already solve it?
[ ] Is local code smaller/safer?
[ ] Runtime, build, proc-macro, or dev only?
[ ] Are default features needed?
[ ] Does it raise compiler/MSRV requirements?
[ ] Does it add native/system dependencies?
[ ] What license/source applies?
[ ] Known advisories?
[ ] New unsafe surface?
[ ] Duplicate major versions?
[ ] Target support?
[ ] Compile-time and binary-size effect material?
```

This is especially important in a small project: dependency overhead can exceed the amount of code it replaces.

---

# Part VII — Generated code, compiler evidence, artifacts, and performance

## 36) cargo-expand for macro/derive investigation

Use:

```bash
cargo expand
cargo expand module::path
```

when the question is:

- what did this derive generate?;
- why does a trait implementation exist?;
- is an apparently unused dependency referenced by a macro?;
- why does a diagnostic point into generated code?;
- what code is conditionally emitted under the Python feature?

Expanded source is a debugging representation, not the canonical source file. Never edit it directly.

---

## 37) cargo-show-asm: MIR -> LLVM -> assembly

`cargo asm` is the preferred targeted tool when investigating one Rust function's compiled representation.

Use it for questions such as:

- did this abstraction inline away?;
- are bounds checks still present?;
- was a generic specialized as expected?;
- did vectorization occur?;
- what MIR shape corresponds to this source?;
- why is a hot loop unexpectedly branchy?

Always match the relevant:

```text
package / target / feature set / profile / target triple / function instantiation
```

Inspecting host-debug assembly and generalizing to the release wheel is invalid evidence.

---

## 38) cargo-binutils: inspect the final linked artifact

Use Cargo-aware LLVM wrappers for linked facts:

```bash
cargo size --release
cargo nm --release
cargo objdump --release -- --disassemble
```

High-value uses for this mixed project:

- verify expected native symbols;
- inspect section sizes;
- investigate stripping/debug information;
- inspect final disassembly after LTO/linker behavior;
- diagnose missing FFI symbols;
- validate release artifact properties.

Record the artifact path and hash when the inspection is part of a release/performance claim.

---

## 39) cargo-bloat: explain native code size

Run:

```bash
cargo bloat --release --crates
cargo bloat --release -n 50
```

when a native artifact or wheel grows unexpectedly.

Investigate in this order:

```text
wheel/native artifact grew
    -> cargo-bloat --crates
    -> identify crate/function contributors
    -> cargo tree -d / cargo-deny duplicate view
    -> cargo-show-asm or cargo-binutils where mechanism unclear
    -> change one cause
    -> rebuild and compare final artifact
```

Do not infer binary size solely from `target/` disk usage.

---

## 40) Performance measurement hierarchy

### 40.1 Hyperfine verifies outcomes

Use a controlled before/after benchmark:

```bash
hyperfine \
  --warmup 3 \
  --export-json target/bench/compare.json \
  '<before-command>' \
  '<after-command>'
```

Record:

- exact input dataset/hash;
- build profile;
- machine/CPU;
- command and environment;
- warmup/run policy;
- sccache state if compilation is benchmarked;
- result distribution.

### 40.2 Samply locates interactive hotspots

Build with symbols:

```bash
cargo build --profile profiling
samply record target/profiling/<binary> <args>
```

For a Python-driven native workload, profile the Python process that loads the built extension or construct a representative Rust benchmark executable if the core path can be exercised directly. The chosen workload must reflect the question being investigated.

### 40.3 cargo-flamegraph creates portable static evidence

Use when a static SVG is valuable for review or comparison. Platform-native sampling prerequisites and symbolization still apply.

### 40.4 Required causal ladder

```text
controlled baseline
    -> profiler identifies hotspot
    -> expand/show-asm/binutils/bloat explains mechanism if necessary
    -> one controlled code change
    -> correctness suite
    -> exact benchmark repeated
```

A narrower flame frame or nicer assembly is not itself a performance improvement.

---

## 41) Build-time measurement and the crate-boundary decision

If compilation latency becomes a problem, **measure before restructuring into crates**.

Benchmark at least:

```text
clean build
incremental edit in hot module
incremental edit in Python binding module
featureless core build
Python-feature build
Rust tests
Maturin develop rebuild
```

Control whether sccache is warm or cold.

A possible future crate split is justified only if measurements show that isolating a stable heavy subsystem lets Cargo avoid material recompilation or if architectural dependency isolation independently warrants it.

Do not assume:

```text
more crates = faster builds
```

or:

```text
one crate = always faster
```

Crate boundaries trade fixed compilation/linking/metadata overhead against incremental invalidation isolation. The correct answer is workload-dependent.

---

# Part VIII — Python quality, packaging, and release artifact validation

## 42) Ruff: one Python formatter/linter

Use Ruff for both formatting and linting to keep the Python tool surface small:

```bash
uv run ruff format --check python python_tests
uv run ruff check python python_tests
```

Write mode is explicit:

```bash
uv run ruff format python python_tests
uv run ruff check --fix python python_tests
```

Any automatic fix should be followed by diff review and tests.

### 42.1 Why Python quality still matters in a Rust-core project

A thin layer can still introduce:

- wrong defaults;
- mistyped argument names;
- stale exports;
- broken exception mapping;
- import cycles;
- package metadata problems;
- inconsistent public API.

“Thin” is not “unimportant.” It is the user-facing contract.

---

## 43) Pyrefly: explicit Python type policy

Configure `[tool.pyrefly]` instead of relying on the unconfigured/basic preset. An unconfigured checker intentionally enables only a narrower high-confidence diagnostic set; a repository-level configuration makes the intended type surface explicit.

Run:

```bash
uv run pyrefly check
```

The type checker should see:

- public Python source;
- Python tests where useful;
- `_native.pyi`;
- the active development environment after `maturin develop` when import resolution requires it.

### 43.1 Suppressions are reviewed artifacts

Use narrow suppressions with a reason. Do not run a mass suppression command merely to make a newly introduced type checker green.

### 43.2 Public typing is part of the interface contract

If a Python public function changes from:

```python
def run_query(query: str) -> str: ...
```

to another signature, treat it as an API change even if Rust compilation and `cargo-semver-checks` are green.

---

## 44) Maturin development workflow

Normal cycle:

```bash
uv sync
uv run maturin develop
uv run pytest
```

`maturin develop` is allowed to mutate the local project environment; it is the **fast iteration path**, not the release evidence path.

### 44.1 Private native module configuration

Keep:

```toml
[tool.maturin]
python-source = "python"
module-name = "myproject._native"
features = ["python"]
```

and keep the PyO3 module name synchronized with `_native`.

### 44.2 Avoid import-path ambiguity

Tests should make it clear whether they are importing:

- source Python files plus a development-installed extension; or
- a built wheel in a fresh environment.

A stale editable/development extension can produce misleading success. `wheel-test` therefore creates a clean environment.

---

## 45) Clean wheel validation

`scripts/wheel_test.sh` should implement the following semantics:

```text
1. require exactly the intended freshly built wheel
2. create a temporary isolated virtual environment
3. install that wheel, not the repository source tree
4. run smoke/API tests against installed package
5. confirm import origin resolves inside temporary environment/site-packages
6. record Python version and wheel filename
7. delete environment after completion unless retained for debugging
```

A portable shell example:

```bash
#!/usr/bin/env bash
set -euo pipefail

wheel="$(find dist -maxdepth 1 -name '*.whl' -print | head -n 1)"
test -n "$wheel"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

uv venv --python "$(uv run python -c 'import sys; print(sys.executable)')" "$tmp/venv"
uv pip install --python "$tmp/venv/bin/python" "$wheel" pytest

WHEEL_TEST_ROOT="$tmp/venv" PYTHONPATH= "$tmp/venv/bin/python" - <<'PY'
import os
import pathlib
import myproject

path = pathlib.Path(myproject.__file__).resolve()
root = pathlib.Path(os.environ["WHEEL_TEST_ROOT"]).resolve()
print(f"installed package: {path}")
assert path.is_relative_to(root), (path, root)
PY

PYTHONPATH= "$tmp/venv/bin/python" -m pytest python_tests -q
```

The exact isolation assertion should be adapted to the operating system and temp-path scheme; its intent is to detect accidentally importing the repository's source tree or pre-existing editable install.

### 45.1 Stronger CI wheel test

CI should:

1. build the wheel in one job/stage;
2. persist it as an artifact;
3. create a fresh environment/job;
4. install only the artifact plus test dependencies;
5. run Python interface tests;
6. hash the tested wheel.

This proves the artifact that passed packaging tests is the one retained.

---

## 46) ABI strategy: start simple

For a hobby project, prefer normal CPython-specific wheels initially. Do not enable `abi3` simply to reduce wheel count unless distribution breadth justifies the compatibility tradeoffs.

If `abi3` is adopted later:

- choose an explicit minimum Python ABI;
- verify every supported Python version;
- ensure used PyO3/Python APIs are available under that ABI;
- distinguish CPython stable ABI support from free-threaded/other interpreter modes;
- test actual wheels rather than relying on tags.

Fewer wheel files are valuable only if they preserve the functionality you need.

---

## 47) Cross-platform build strategy

Do not create an enormous release matrix before there are users for it. A reasonable progression:

```text
Stage 1: development host only
Stage 2: Linux x86_64 + macOS current architecture
Stage 3: add macOS arm64/x86_64 or Linux arm64 when needed
Stage 4: explicit old-runtime / manylinux compatibility if distributing broadly
```

### 47.1 Cross

Use `cross` when a containerized cross-build/test environment provides real value. Report whether a target was only compiled or actually executed under a runner/emulator.

### 47.2 cargo-zigbuild

Use `cargo-zigbuild` when linker/sysroot portability or a controlled Linux glibc floor matters. A successful link does not prove the wheel/runtime works in the oldest supported environment.

### 47.3 Maturin remains package authority

Cross/zig build layers solve Rust/native target problems; Maturin still owns Python package construction and tags. The release gate must combine target build evidence with wheel-install evidence.

---

## 48) Release artifact record

For each retained wheel or Rust executable, store or emit:

```text
source commit SHA
Cargo.lock SHA-256
uv.lock SHA-256
rustc -vV
cargo -V
Maturin version
Python version/ABI
target triple
Cargo feature set
build profile
wheel/executable filename
artifact SHA-256
cargo-deny result
cargo-audit result
wheel clean-install test result
native target execution level (if applicable)
```

This is enough provenance for a hobby project to be reproducible and diagnosable without building a full enterprise attestation platform.

---

# Part IX — CI, scheduled assurance, and automation topology

## 49) CI philosophy: three assurance tiers

Do not put every installed tool on the critical path of every commit. A small project benefits from fast feedback more than from a CI suite whose cost causes it to be skipped.

### Tier A — required on every pull request / meaningful change

Target: fast enough that it remains routine.

```text
Rust format
Rust check
Clippy
nextest
Rust doctests
Python Ruff format/lint
Pyrefly
Maturin development/native build as applicable
pytest
Typos
cargo-shear
cargo-machete (optional if Shear already low-cost)
cargo-deny
cargo-audit
```

For a solo hobby project, `cargo-deny` and `cargo-audit` may run once per PR/push rather than on every local edit.

### Tier B — conditional or broader PR evidence

Trigger according to change type:

```text
cargo-llvm-cov
Insta pending snapshot validation
cargo-hack feature combinations
clean wheel build/install tests
cargo-semver-checks when supported Rust API changes
cargo-msrv verify when an MSRV promise exists
Cross/target checks when target-specific code changes
binary-size comparison when dependencies/release flags change
```

### Tier C — scheduled or risk-triggered deep assurance

```text
Miri multiple seeds
cargo-mutants
cargo-fuzz campaigns / corpus replay
cargo-udeps
cargo-vet audit-gap review
cargo-geiger trend
full cargo-hack powerset (only if tractable)
wide target matrix
Hyperfine regression baselines
Samply/flamegraph investigations
bloat/binutils release inspection
```

A Tier C failure must still have an owner/response. A “deep assurance” workflow that is permanently red is worse than a smaller meaningful suite.

---

## 50) Recommended `ci.yml`

A compact GitHub Actions design can use separate Rust and mixed-package jobs. Exact action versions should be pinned to trusted release tags or commit SHAs according to repository policy.

Conceptual workflow:

```yaml
name: CI

on:
  pull_request:
  push:
    branches: [main]

jobs:
  rust:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@<pinned>
      - uses: dtolnay/rust-toolchain@<pinned>
        with:
          toolchain: stable
          components: rustfmt,clippy,llvm-tools-preview
      - uses: mozilla-actions/sccache-action@<pinned>
      - uses: cargo-bins/cargo-binstall@<pinned>
        with:
          version: "<pinned-binstall-version>"
      - name: Install Rust CLIs
        run: |
          # Install explicit versions or use a pinned installer action.
          cargo binstall --no-confirm cargo-nextest@<version>
          cargo binstall --no-confirm cargo-deny@<version>
          cargo binstall --no-confirm cargo-audit@<version>
          cargo binstall --no-confirm cargo-shear@<version>
      - run: cargo fmt --all -- --check
      - run: cargo check --all-targets
      - run: cargo check --all-targets --features python
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo nextest run -P ci
      - run: cargo test --doc
      - run: cargo shear --deny-warnings
      - run: cargo deny check
      - run: cargo audit

  python:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@<pinned>
      - uses: astral-sh/setup-uv@<pinned>
      - uses: dtolnay/rust-toolchain@<pinned>
        with:
          toolchain: stable
      - run: uv sync --frozen
      - run: uv run ruff format --check python python_tests
      - run: uv run ruff check python python_tests
      - run: uv run pyrefly check
      - run: uv run maturin develop
      - run: uv run pytest
```

### 50.1 Why not rely on globally preinstalled tools in CI

The developer workstation may use current global tools managed by cargo-update. CI is an evidence environment and should install explicit versions so a tool release cannot silently change a merge gate.

### 50.2 Frozen lock behavior

CI SHOULD fail rather than silently rewrite lock state. For Python use uv's frozen/locked mode appropriate to the installed version. For Rust commit `Cargo.lock` and avoid incidental `cargo update` during verification.

---

## 51) Wheel workflow

A wheel workflow should test the **built artifact** across the interpreter/platform matrix actually advertised.

Conceptual stages:

```text
matrix build
    OS x Python x architecture policy
        │
        ▼
Maturin produces wheel
        │
        ▼
wheel artifact retained
        │
        ▼
fresh environment installs artifact
        │
        ▼
pytest public API suite
        │
        ▼
artifact hash + metadata retained
```

### 51.1 Start with a narrow support matrix

For a personal project, two actively used environments are better than six nominally supported environments no one executes.

Example initial policy:

```text
Linux x86_64, Python 3.12/current chosen floor
macOS current development architecture, same Python floor
```

Expand only when distribution needs arise.

### 51.2 Do not build a wheel per feature combination

The Python wheel should correspond to one deliberate product configuration. Cargo feature matrices test Rust compilation compatibility; they are not all distinct Python products unless explicitly designed that way.

---

## 52) `deep-assurance.yml`

Run on schedule and manually. Example conceptual jobs:

```text
miri:
  pinned nightly
  cargo miri test
  multiple seeds for critical tests

mutation:
  cargo mutants on critical modules or full crate within a time budget

fuzz:
  replay corpus always
  bounded fuzz campaigns for parser/input targets
  retain crash artifacts

udeps:
  pinned nightly
  cargo udeps --all-targets --all-features

supply-chain:
  cargo vet
  cargo geiger

performance:
  only if stable benchmark environment exists
  compare to stored or explicitly selected baseline
```

Do not run uncontrolled indefinite fuzzing or benchmark performance on highly variable hosted runners and treat small differences as regressions.

---

## 53) CI artifacts

Prefer machine-readable outputs:

```text
nextest     -> JUnit
llvm-cov    -> LCOV / JSON / Cobertura
Ruff        -> machine-friendly diagnostics where needed
Pyrefly     -> structured output if supported by pinned release
cargo-audit -> JSON
Typos       -> JSON
Shear       -> JSON
Hyperfine   -> JSON
flamegraph  -> SVG
Samply      -> profile artifact
fuzz        -> corpus/crash artifact
Maturin     -> wheel files
```

The agent/reporting layer should summarize results rather than paste enormous logs.

---

# Part X — Repository hygiene and generated-state policy

## 54) `.gitignore`

A baseline:

```gitignore
# Rust
/target/

# Python
/.venv/
__pycache__/
*.py[cod]
.pytest_cache/
.ruff_cache/

# Maturin/package output
/dist/
# `maturin develop` may place the native extension in the mixed Python source tree.
/python/**/*.so
/python/**/*.pyd
/python/**/*.dylib

# Local diagnostics
.bacon-locations

# Profilers / generated reports that are not intentionally versioned
*.profraw
*.profdata
flamegraph.svg

# OS/editor noise
.DS_Store
```

Do **not** ignore:

```text
Cargo.lock
uv.lock
.config/nextest.toml
bacon.toml
deny.toml
_typos.toml
justfile
py.typed
_native.pyi
reviewed Insta snapshots
cargo-vet policy/audit state if adopted
fuzz regression corpus seeds intentionally retained
```

### 54.1 Lockfiles

This project is an application/distributable package, not merely a generic library, so commit both:

```text
Cargo.lock
uv.lock
```

They are part of reproducible development/package evidence.

---

## 55) Generated files policy

Every generated file belongs to one of three categories:

### A. Ephemeral build output — never versioned

```text
target/
dist/
coverage raw profiles
profiling temp files
local caches
```

### B. Reviewed regression artifact — versioned

```text
Insta snapshots
small fuzz regression corpus entries
schema golden files
stable generated stubs if repository policy chooses to generate rather than hand-maintain them
```

### C. Release evidence — stored as CI/release artifacts, not normal source

```text
wheel
coverage HTML
JUnit
Hyperfine JSON
flamegraph SVG
Samply profile
artifact hashes
full tooling inventory
```

Agents must know which category applies before deleting, accepting, or committing generated content.

---

## 56) `scripts/` policy

Use `scripts/` only when a task is too complex for a readable `just` recipe or needs substantial procedural logic.

Good examples:

```text
wheel_test.sh
coverage_python.sh
tooling_inventory.sh
benchmark_compare.py
```

Bad examples:

```text
run_tests.sh       # if it simply says cargo nextest run
format.sh          # if it simply says cargo fmt
misc.py            # unclear ownership
helpers.sh         # arbitrary dumping ground
```

`just` should remain the discoverable front door:

```text
agent -> just wheel-test -> scripts/wheel_test.sh
```

not:

```text
agent -> memorizes scripts/wheel_test.sh path
```

---

## 57) Tooling inventory script

`scripts/tooling_inventory.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

mkdir -p target

{
  date -u
  uname -a || true
  rustc -vV
  cargo -V
  rustup show active-toolchain
  rustup component list --installed
  cargo install --list
  uv --version
  uv run python --version
  uv run maturin --version
  uv run ruff --version
  uv run pyrefly --version
  sccache --show-stats || true
} > target/tooling-inventory.txt
```

Do not dump the full environment because it may contain credentials. Capture only execution-relevant, non-secret facts.

---

## 58) Repository documentation minimum

`README.md` should include:

```text
what the project does
architecture: Rust core / Python façade
supported OS/Python policy
bootstrap: uv sync + Rust tool requirements
common commands: just --list / just ci-fast / just test
how to build a wheel
where generated artifacts go
whether nightly is required for normal development (normally no)
```

A separate `CONTRIBUTING.md` is optional for a solo project. Prefer one accurate README over ceremonial documentation no one maintains.

---

# Part XI — LLM programming-agent operating specification

## 59) Mandatory session bootstrap

Before a substantial code change, an agent SHOULD run or obtain the equivalent of:

```bash
pwd
just --list
rustc -vV
cargo -V
rustup show active-toolchain
cargo metadata --format-version 1 --no-deps
uv --version
uv run python --version
```

Then inspect:

```text
Cargo.toml
pyproject.toml
rust-toolchain.toml
justfile
relevant Rust production source
relevant Python interface source
nearby tests
```

If the task depends on an optional deep tool, capture its version/help before relying on nontrivial flags.

### 59.1 Baseline before edit

For ordinary work:

```bash
just ci-fast
```

or a narrower repository-owned baseline if the full command is expensive. Record pre-existing failures; do not attribute them to the edit.

---

## 60) Change-risk classification

Every change should be classified before selecting validation.

| Change | Minimum additional evidence |
|---|---|
| comment/docs only | Ruff/Typos as relevant |
| local safe Rust logic | check/Clippy + targeted nextest |
| public Python façade | pytest + Pyrefly + Ruff + wheel test if packaging-significant |
| PyO3 conversion/binding | Rust tests + pytest + Maturin build; Miri where pure unsafe conversion logic permits |
| error mapping | Rust error tests + Python exception tests |
| Cargo feature | cargo-hack + featureless and `python` builds |
| dependency | hygiene + deny/audit + tests + feature matrix |
| unsafe/pointer/concurrency | Geiger + Miri + fuzz where input-driven + native tests |
| parser/protocol | coverage + fuzz + snapshots + mutation testing on critical logic |
| macro/derive issue | cargo-expand |
| performance claim | Hyperfine baseline/after + profiler; codegen tools if mechanism matters |
| binary/wheel size | cargo-bloat + binutils + final wheel size |
| cross-target code | Cross/native target evidence |
| Python packaging | fresh wheel install + pytest |
| public Rust API | semver-checks + feature surface |

The agent must not run Tier C tools merely to appear thorough. It should run them when they produce evidence relevant to the risk.

---

## 61) Editing invariants

### 61.1 Preserve dependency direction

Before introducing PyO3/Python object dependencies into code used by the ordinary Rust core, stop and confirm that the dependency is genuinely part of the language-binding boundary. This specification does not dictate which Rust file or folder owns that boundary.

### 61.2 Do not create a crate merely to organize production code

A new conceptual area in the application does **not** by itself justify `crates/<name>/Cargo.toml`. Apply the crate-boundary criteria from §0.3. If none apply, keep the implementation inside the existing crate using whatever internal file/module organization best fits the project.

### 61.3 Do not create many top-level integration-test files

Add test cases beneath an existing integration-test target unless a distinct integration-test crate is intentional. This rule exists because Cargo compiles top-level `tests/*.rs` files as separate integration-test crates; it does not govern production source layout.

### 61.4 Keep Python as the interface layer

If an agent begins implementing a second independent copy of core behavior in Python, it must reassess the boundary. The Python package should adapt and expose the Rust implementation rather than diverge from it.

### 61.5 Keep the native extension private

Do not document `_native` as public API and do not make ordinary application callers depend on it by default.

## 62) Validation invariants

### 62.1 rust-analyzer is not final authority

Use semantic tooling for navigation, type context, references, and edit-time diagnostics. Confirm with repository Cargo commands.

### 62.2 Nextest is not all Rust tests

Run doctests separately where present.

### 62.3 Development import is not wheel proof

`maturin develop` + pytest validates the development integration. Only a clean installation of the built wheel validates package construction.

### 62.4 Coverage is not test strength

Use mutation testing for critical assertion sensitivity.

### 62.5 Miri is not exhaustive soundness proof

State test/seed/target scope.

### 62.6 `--all-features` is not a feature matrix

Use cargo-hack when features matter.

### 62.7 A scanner result is not permission to mutate dependencies

Reconcile static/compiler/macro/target usage before removal.

### 62.8 Profiling is not outcome measurement

Hyperfine or a project benchmark verifies the change.

### 62.9 Cross build is not runtime support

State compiled / linked / emulated / native-tested separately.

---

## 63) Source/environment mutation controls

The following commands are explicitly mutating and SHALL NOT be hidden inside validation recipes:

```text
cargo shear --fix
typos -w
cargo insta accept
cargo vet certify
cargo update
cargo install-update -a
rustup update
uv lock --upgrade / dependency upgrades
Ruff --fix
maturin publish
```

After any source/manifest mutation:

1. inspect the diff;
2. identify semantic impact;
3. rerun relevant validation;
4. disclose what the tool changed.

---

## 64) Failure taxonomy

Agents SHALL classify a failure before attempting remediation:

| Class | Example |
|---|---|
| environment/tool | executable absent |
| tool version | CLI flag/config field unsupported |
| Rust source compilation | rustc error |
| build script/proc macro | generated/build-time failure |
| Python environment | wrong interpreter or unresolved environment |
| binding compilation | PyO3/Maturin native build failure |
| test assertion | deterministic nextest/pytest failure |
| flaky/nondeterministic | retry-dependent result |
| UB/interpreter | Miri finding |
| fuzz | crash/panic/timeout/mismatch |
| coverage | required path unexecuted |
| mutation | surviving behavioral fault |
| feature/MSRV/semver | compatibility failure |
| dependency hygiene | unused/misplaced dependency |
| policy/advisory | deny/audit result |
| audit trust | cargo-vet gap |
| artifact | symbol/size/provenance issue |
| packaging | wheel build/tag/import/install failure |
| target | linker/runner/runtime incompatibility |
| performance | measured regression |
| infrastructure | cache/container/filesystem/credential issue |

Do not change production code to compensate for an environment failure, or relax policy to hide a functional failure.

---

## 65) Evidence record for nontrivial checks

A programming agent reporting a result SHOULD preserve:

```text
tool + version
rustc/Cargo version
Python + Maturin version when relevant
active toolchain
target
features
profile
exact command
package/test scope
exit status
important counts/findings
report/artifact path
known exclusions
source/environment mutations
```

Prefer a statement such as:

```text
cargo-nextest <version>, rustc <version>
command: cargo nextest run -P ci
result: N passed, 0 failed; doctests passed separately
Python: pytest N passed after maturin development install
```

rather than “tests are good.”

---

# Part XII — Workflow recipes by development task

## 66) Ordinary Rust implementation change

```text
1. inspect definition/references with semantic tooling
2. edit smallest coherent module
3. Bacon/rust-analyzer feedback
4. cargo fmt
5. cargo check + Clippy
6. targeted nextest
7. full `just ci-fast` before completion
```

No Miri/fuzz/mutants unless the changed semantics warrant them.

---

## 67) Python façade change

```text
1. keep native/core API unchanged if possible
2. update annotations/stubs as needed
3. Ruff format/lint
4. Pyrefly
5. maturin develop if native import needed
6. targeted pytest
7. full Python tests
8. wheel-test if exports/package layout/stubs/metadata changed
```

If the Python change adds substantial computation, reconsider the language boundary.

---

## 68) PyO3 binding change

```text
1. identify pure Rust contract being exposed
2. keep Python-specific conversion at the Rust↔Python binding boundary; internal file placement is repository-defined
3. cargo check --features python
4. Clippy with python feature
5. Rust conversion/error tests
6. maturin develop
7. pytest boundary tests
8. wheel build + clean install for packaging-significant changes
9. Miri on pure unsafe/conversion internals where supported
10. Geiger if unsafe surface changed
```

For long pure Rust calls, reassess whether `Python::detach` is appropriate.

---

## 69) Parser or untrusted-input change

```text
nextest regression suite
    -> coverage
    -> snapshot/golden semantic output where useful
    -> bounded fuzz run / corpus replay
    -> minimize any finding
    -> deterministic regression
    -> Miri on unsafe path if applicable
    -> mutation test changed validation logic
```

Fuzzer output is discovery evidence; ordinary tests become permanent regression evidence.

---

## 70) Dependency change

Use the campaign from §35 and additionally rebuild the Python wheel if the dependency participates in the native extension. A dependency that only builds under the pure Rust test profile may still fail in a Maturin release build because features/linking differ.

---

## 71) Performance optimization

```text
1. define user-relevant metric and workload
2. record Hyperfine/application baseline
3. profile representative workload with Samply/flamegraph
4. inspect bloat/expand/assembly only if needed to explain mechanism
5. change one thing
6. run correctness suite
7. repeat exact benchmark
8. report runtime + binary-size/build-time tradeoffs
```

For Python-facing latency, benchmark both:

```text
pure Rust core path
public Python call path
```

when boundary overhead may matter.

---

## 72) Release/wheel candidate

```text
1. clean source status and pinned locks
2. `just ci-pr`
3. required conditional/deep checks
4. `just wheel`
5. fresh-environment wheel installation
6. Python public API tests from wheel
7. target/platform smoke tests
8. final wheel hash and tooling inventory
9. retain release artifact/evidence
10. publish only as a separately authorized action
```

`maturin publish` is never implied by “build,” “test,” or “prepare release candidate.”

---

# Part XIII — Complete installed-tool disposition for this repository

## 73) Tool-by-tool policy matrix

The earlier tooling reference remains the detailed capability reference. This section translates every installed addition into a **repo-specific disposition** so an agent knows whether the tool is always-on, routine, conditional, or environment-maintenance-only.

| Tool / component | Repository role | Default status |
|---|---|---|
| `rust-analyzer` | edit-time semantic model, references, types, assists | always on |
| `rust-src` | matching stdlib source for semantic/compiler tooling | installed prerequisite |
| `llvm-tools-preview` | coverage/binutils/fuzz-coverage substrate | installed prerequisite |
| `rustc-dev` | compiler-private/HIR/MIR integration | installed globally; **not a repo requirement now** |
| Miri | UB/aliasing/concurrency exploration | risk-triggered / scheduled |
| `cargo-binstall` | fast acquisition of developer CLIs | workstation/CI setup |
| `cargo-update` | developer-global Cargo CLI maintenance | periodic workstation maintenance |
| `sccache` | repeated compilation cache | always on if repo chooses committed wrapper |
| `just` | human/agent command contract | always used |
| Bacon | one persistent Rust check loop | local development |
| Watchexec | non-Rust triggers/process restarts | targeted local use |
| `fd` | fast ignore-aware repository file discovery | agent/developer discovery |
| Typos | spelling/identifier hygiene | local/PR |
| cargo-nextest | ordinary Rust test runner | local/PR |
| cargo-llvm-cov | Rust source coverage | conditional PR/scheduled |
| cargo-insta | reviewed structured-output snapshots | only where snapshot-worthy output exists |
| cargo-mutants | test assertion sensitivity | risk-triggered/scheduled |
| cargo-fuzz | adversarial input discovery | input-surface dependent/scheduled |
| cargo-hack | feature-state compatibility | conditional PR/scheduled |
| cargo-msrv | compiler-floor discovery/verification | only if MSRV becomes a commitment |
| cargo-semver-checks | Rust public API compatibility | only if Rust API is externally supported |
| cargo-shear | primary unused/misplaced dependency scan | local/PR |
| cargo-machete | extremely fast unused-dependency hint | local |
| cargo-udeps | compiler-oriented dependency adjudication | scheduled/disputed findings |
| cargo-deny | licenses/sources/bans/advisories policy | PR/dependency change |
| cargo-audit | RustSec known-advisory scan | PR/scheduled |
| cargo-vet | trusted human audit attestations | optional adoption / dependency changes |
| cargo-geiger | unsafe-code inventory/trend | risk-triggered/scheduled |
| cargo-auditable | embedded Rust dependency metadata | standalone executable release where useful |
| cargo-expand | macro/derive expansion | targeted investigation |
| cargo-show-asm | MIR/LLVM/assembly for selected functions | targeted performance/compiler investigation |
| cargo-binutils | linked symbols/sections/disassembly | targeted/release |
| cargo-bloat | native code-size attribution | targeted/release |
| Hyperfine | controlled command benchmarking | performance/build-time changes |
| Samply | interactive sampling profile | performance investigation |
| cargo-flamegraph | static sampling artifact | performance investigation/review |
| Cross | containerized cross-build/test | only supported foreign targets |
| cargo-zigbuild | linker/sysroot/glibc portability | release portability when needed |
| Maturin | Rust-backed Python development/wheel build | core interface tool |

### 73.1 Tool availability is not a mandate to run it

The project gains value from **having** the full diagnostic arsenal while keeping the normal loop small. An agent should select the smallest set of tools that answers the risk question, then escalate if evidence is inconclusive.

---

## 74) `fd` as repository-discovery primitive

Although semantic symbol work belongs to rust-analyzer, agents still need fast file-set discovery:

```bash
fd -H -d 3 '^(Cargo\.toml|pyproject\.toml|rust-toolchain\.toml|justfile|bacon\.toml|deny\.toml|_typos\.toml)$'
fd -e rs src tests
fd -e py python python_tests
fd -e pyi python
```

`fd` respects ignore rules by default. If completeness matters, explicitly account for hidden/ignored files rather than concluding a path does not exist.

Do not use parallel `fd -x` execution for source-mutating commands unless those commands are safe to run concurrently.

---

## 75) cargo-binstall and cargo-update lifecycle

### 75.1 Developer bootstrap

Use cargo-binstall to provision Rust CLIs quickly, preferably with explicit versions in a bootstrap script or documented setup manifest. Prebuilt binaries avoid compiling a large tooling fleet from source.

### 75.2 CI

Install **explicit tool versions**, even if the developer workstation tracks current releases. CI needs reproducible command behavior.

### 75.3 Workstation updates

Periodically:

```bash
cargo install-update -a
```

then capture versions and run:

```bash
just ci-fast
```

Do not update the entire global toolchain while diagnosing a source regression; that destroys the controlled before/after state.

### 75.4 Do not confuse update domains

```text
rustup update              -> Rust toolchains/components
cargo install-update       -> globally installed Cargo executables
cargo update               -> project dependency lock resolution
uv lock --upgrade / uv add -> Python project dependency resolution
```

These are four distinct mutations.

---

## 76) rustc-dev disposition

`rustc-dev` may remain installed on the developer machine because it is useful for compiler/MIR tooling, but this repository should **not** declare it in its normal `rust-toolchain.toml` unless the project actually begins consuming compiler-private APIs.

If that happens, the architecture changes materially:

```text
exact date-pinned nightly
+ rustc-dev
+ matching rust-src
+ matching llvm-tools-preview
+ semantic golden/snapshot corpus
+ deliberate nightly upgrade process
```

Do not introduce compiler-private coupling merely to inspect MIR once; `cargo-show-asm` or stable compiler outputs are the lower-cost investigation tools.

---

# Part XIV — When and how to graduate to a workspace

## 77) Do not pre-allocate a workspace

The initial root `Cargo.toml` should describe the package directly, not contain an empty `[workspace]` simply because another crate might exist someday.

A workspace is introduced when the repository has a **real second Cargo package**.

---

## 78) Legitimate future two-crate architecture

The most plausible split for a Rust-core/Python-interface project is:

```text
repository/
├── Cargo.toml                       # workspace
├── crates/
│   ├── core/
│   │   ├── Cargo.toml
│   │   └── src/
│   └── python-extension/
│       ├── Cargo.toml
│       └── src/
├── python/myproject/
└── pyproject.toml
```

Dependency direction:

```text
python-extension -> core
core             -X-> python-extension / PyO3
```

This split is justified when at least one is true:

- the core is independently reusable as a Rust library;
- PyO3/native packaging dependencies materially contaminate or slow core workflows;
- the core and extension need different target/platform constraints;
- separate build caching measurably improves real iteration;
- the core gains an independent consumer or release contract.

### 78.1 What should not happen after graduation

Do not turn:

```text
core
```

into:

```text
model-crate
errors-crate
parser-crate
storage-crate
query-crate
validation-crate
utils-crate
```

unless each new boundary separately satisfies the crate-creation test. A workspace with two meaningful crates can remain healthy; dozens of package-shaped modules recreate the original problem.

---

## 79) Benchmark before a compilation-motivated split

Before splitting:

1. capture current source revision;
2. zero/record sccache statistics;
3. measure clean build;
4. measure representative incremental edits;
5. measure test and Maturin rebuild latency;
6. perform candidate split on a branch;
7. repeat exactly;
8. compare target disk footprint and binary/wheel size separately;
9. keep the split only if benefits justify added architecture.

Use Hyperfine where command-level timing is appropriate. Do not let subjective “feels faster” decide a permanent package boundary.

---

## 80) Migration from an over-factored workspace to this structure

If converting an existing many-crate project:

### Phase 1 — inventory

Record:

```text
workspace member graph
dependency graph per crate
crate compile/test timings
features
integration-test targets
public inter-crate APIs
Python binding dependency path
```

### Phase 2 — classify crate boundaries

For each crate, mark:

```text
KEEP       real independent boundary
MERGE      organizational-only boundary
INVESTIGATE compilation/performance justification unclear
```

### Phase 3 — merge leaf crates first

Move source into modules while preserving APIs as much as practical. Replace cross-crate `pub` with `pub(crate)`/private visibility where possible.

### Phase 4 — collapse tests

Convert per-crate unit tests to colocated module tests and group external integration cases into a small number of top-level test targets.

### Phase 5 — simplify features/dependencies

Re-run Machete/Shear/Udeps and cargo-hack. Workspace decomposition often leaves duplicate or misplaced dependencies and unnecessary public feature wiring.

### Phase 6 — benchmark

Compare:

- clean build;
- representative incremental edit;
- nextest build/run;
- target disk usage;
- Maturin development build;
- final wheel/native artifact size.

Do not assume every metric improves simultaneously.

---

# Part XV — Anti-pattern compendium for this project

## 81) Structural anti-patterns

### 81.1 One crate per subsystem

**Problem:** turns namespaces into compiler boundaries, multiplies manifests/dependency edges/test targets, and weakens local code navigation.

**Correction:** modules first.

### 81.2 Dozens of `tests/*.rs` files

**Problem:** each top-level file is another integration-test executable crate.

**Correction:** one/few top-level integration targets with internal test modules.

### 81.3 PyO3 types throughout the Rust domain

**Problem:** Python becomes an architectural dependency of core behavior and pure Rust testing/reuse gets harder.

**Correction:** feature-gated adapter layer.

### 81.4 Public callers import `_native`

**Problem:** low-level FFI structure becomes the supported API.

**Correction:** curated Python façade.

### 81.5 Legacy `pyo3/extension-module` feature copied from old examples

**Problem:** current PyO3/Maturin no longer needs the historical global feature in the normal workflow and it can interfere with native test linking.

**Correction:** let modern Maturin configure extension builds.

### 81.6 Python reimplements Rust validation

**Problem:** two semantic sources of truth drift.

**Correction:** Rust owns domain validation; Python performs only interface normalization.

---

## 82) Build/test anti-patterns

### 82.1 Routine `cargo clean`

**Problem:** destroys incremental state and makes iteration slower.

**Correction:** clean only for diagnosis, controlled benchmarking, or disk reclamation.

### 82.2 Interpreting `target/` as shipped size

**Problem:** target contains many profiles, incremental caches, test executables, metadata, and intermediates.

**Correction:** measure final wheel/executable and use cargo-bloat/binutils.

### 82.3 Every expensive tool on every edit

**Problem:** slow feedback causes assurance to be skipped or ignored.

**Correction:** tiered risk-triggered execution.

### 82.4 Nextest alone equals complete tests

**Problem:** doctests are separate; Python interface is also a separate subject.

**Correction:** nextest + doctest + pytest as relevant.

### 82.5 High coverage equals strong tests

**Problem:** execution is not assertion quality.

**Correction:** mutation testing on critical logic.

### 82.6 Snapshot auto-acceptance

**Problem:** silently blesses regressions.

**Correction:** review semantic diffs.

### 82.7 Fuzz crash left only in `fuzz/artifacts`

**Problem:** discovery is not stable regression coverage.

**Correction:** minimize and promote to deterministic tests.

---

## 83) Packaging/API anti-patterns

### 83.1 `maturin develop` means release wheel works

**Correction:** fresh wheel installation test.

### 83.2 Stale development extension imported accidentally

**Correction:** clean-environment wheel tests and import-origin checks.

### 83.3 ABI3 adopted prematurely

**Correction:** start with ordinary CPython wheels; add stable ABI only when wheel-matrix benefit is real.

### 83.4 Python typing treated as optional polish

**Correction:** public Python façade is the product interface; stub/type-check it.

### 83.5 One FFI call per element

**Correction:** batch/coarsen boundaries and keep transformations in Rust.

---

## 84) Tooling anti-patterns

### 84.1 Cargo dependency removed because one scanner says unused

**Correction:** reconcile macros/build scripts/features/targets and validate after removal.

### 84.2 cargo-vet self-certification by an agent

**Correction:** agents assist; accountable reviewer certifies.

### 84.3 Geiger unsafe count treated as vulnerability score

**Correction:** use it to focus review/Miri/fuzzing.

### 84.4 cargo-audit green means supply chain is safe

**Correction:** it is known-advisory evidence only; deny/vet/review answer different questions.

### 84.5 Flamegraph means an optimization is proven

**Correction:** profiler locates; benchmark verifies.

### 84.6 Debug host assembly used to explain release wheel

**Correction:** match feature/profile/target/artifact.

### 84.7 Global tools upgraded during debugging

**Correction:** freeze environment until the source issue is reproduced; upgrade separately.

---

# Part XVI — Final implementation checklist

## 85) Repository shape

```text
[ ] One Rust package/crate at inception.
[ ] No `crates/` directory without a second genuine package.
[ ] Rust production source lives under the Cargo package source root, with semantic file/folder organization intentionally unspecified.
[ ] Python interface source lives under `python/<package>/`, with internal semantic file/folder organization intentionally unspecified.
[ ] Rust↔Python binding boundary is explicit without requiring a prescribed Rust source path.
[ ] Native extension is private (`<package>._native`).
[ ] Public Python package exports are curated.
[ ] External Rust integration tests are grouped into one/few top-level targets.
[ ] Python tests exercise the public package by default.
```

## 86) Manifests and locking

```text
[ ] Cargo.lock committed.
[ ] uv.lock committed.
[ ] `rust-toolchain.toml` stable-first.
[ ] Nightly used only by targeted commands unless architecture requires it.
[ ] PyO3 optional behind `python` feature.
[ ] Legacy `pyo3/extension-module` feature absent from normal Maturin setup.
[ ] Maturin uses `python-source` + private `module-name`.
[ ] Python version policy synchronized across pyproject/Ruff/Pyrefly/CI.
[ ] `py.typed` present.
[ ] Native `.pyi` stub present or generated by an intentional mechanism.
```

## 87) Fast development loop

```text
[ ] rust-analyzer enabled.
[ ] rust-src installed for active toolchain.
[ ] sccache enabled intentionally and hit rate observed.
[ ] `just --list` exposes normal repository operations.
[ ] Bacon owns one Rust background check.
[ ] Watchexec does not duplicate that check.
[ ] Ruff owns Python lint/format.
[ ] Pyrefly has explicit repository configuration.
```

## 88) Testing and test quality

```text
[ ] nextest is normal Rust test runner.
[ ] Rust doctests run separately.
[ ] pytest validates public Python API.
[ ] development install and wheel install are separate tests.
[ ] nextest retries expose flakes rather than launder them.
[ ] coverage report scope/features are explicit.
[ ] snapshots require semantic review.
[ ] mutation testing targets critical logic periodically.
[ ] fuzz targets exist only for suitable surfaces.
[ ] fuzz findings become deterministic regression tests.
[ ] Miri runs on unsafe/concurrent critical paths as applicable.
```

## 89) Compatibility and dependencies

```text
[ ] Featureless Rust core compiles.
[ ] `python` feature compiles.
[ ] cargo-hack exercises meaningful feature states.
[ ] MSRV is only claimed if intentionally supported and tested.
[ ] semver-checks is used only against a meaningful Rust API baseline.
[ ] Machete/Shear provide routine dependency hygiene.
[ ] Udeps adjudicates disputed/high-impact findings.
[ ] deny.toml encodes actual license/source/bans policy.
[ ] cargo-audit runs on representative lock graph.
[ ] cargo-vet adoption level is explicit.
[ ] Geiger changes trigger review rather than blind score gating.
```

## 90) Performance and artifacts

```text
[ ] Dedicated profiling profile preserves symbols.
[ ] Performance claims begin/end with controlled benchmarks.
[ ] Samply/flamegraph workload is representative.
[ ] cargo-show-asm uses relevant profile/features/target.
[ ] cargo-bloat/binutils inspect final native artifact when size matters.
[ ] `target/` disk size is not conflated with wheel/binary size.
[ ] Build-time crate splits require measured before/after evidence.
```

## 91) Python packaging/release

```text
[ ] `maturin develop` used only for local iteration evidence.
[ ] Release wheel built with `maturin build --release`.
[ ] Fresh environment installs the wheel artifact.
[ ] pytest passes against the installed wheel.
[ ] Import origin proves tests did not fall back to source/editable state.
[ ] Supported Python/OS/architecture matrix is honest and intentionally narrow.
[ ] ABI3 is adopted only if justified and tested.
[ ] Wheel hash and build inventory retained.
[ ] Publishing is an explicit separate action.
```

## 92) Agent controls

```text
[ ] Agent inspects `just --list` and repository configuration first.
[ ] Agent captures active toolchain/target/features for nontrivial evidence.
[ ] Agent classifies change risk before selecting expensive tools.
[ ] Agent does not create crates merely for organization.
[ ] Agent does not widen Rust visibility merely for tests.
[ ] Agent preserves Rust -> Python dependency direction without inventing a mandatory production source layout.
[ ] Agent treats source/environment-mutating tool operations explicitly.
[ ] Agent classifies failures before attempting fixes.
[ ] Agent separates pre-existing failures from regressions.
[ ] Agent reports exact evidence class and known exclusions.
```

---

# Part XVII — Recommended initial implementation sequence

## 93) Day-one setup order

For a new repository, implement in this order:

### Step 1 — Core package

Create the minimum Cargo package surface:

```text
Cargo.toml
Cargo.lock
rust-toolchain.toml
src/lib.rs
```

Add whatever additional Rust production source files/modules the application design requires; **their semantic organization is intentionally not specified here**.

Get:

```bash
cargo check
cargo test
cargo clippy
```

green before adding the Python packaging boundary where practical.

### Step 2 — Python package boundary

Create the minimum mixed-project packaging surface:

```text
pyproject.toml
.python-version
python/myproject/__init__.py
python/myproject/py.typed
python/myproject/_native.pyi       # if manually maintained
```

Add the PyO3 module declaration and any Python/Rust interface source using whatever internal production-code organization the project chooses. Add the optional `python` feature and Maturin private-module configuration. Then:

```bash
uv sync
uv run maturin develop
uv run pytest
```

The specification intentionally does not require `api.py`, `exceptions.py`, `src/python.rs`, or any other semantic production-code file.

### Step 3 — Operational contract

Add:

```text
justfile
.cargo/config.toml
.config/nextest.toml
bacon.toml
_typos.toml
```

Make `just ci-fast` green.

### Step 4 — Dependency governance

Add:

```text
deny.toml
cargo-audit gate
cargo-shear / cargo-machete gate
```

Do not adopt cargo-vet merely to populate another directory; adopt it when you intend to maintain the audit workflow.

### Step 5 — Packaging evidence

Add:

```text
scripts/wheel_test.sh
just wheel
just wheel-test
```

Validate a fresh wheel install.

### Step 6 — Deep tools only when surfaces appear

Add:

```text
fuzz/          when parser/untrusted input exists
snapshots      when structured golden output exists
mutants job    when critical logic/test maturity warrants it
Miri job       when unsafe/concurrency risk exists
performance    when a stable workload exists
Cross/Zig      when a second target is genuinely supported
```

This avoids ceremony while preserving a clear path to high assurance.

---

# Part XVIII — Source and version policy

## 94) Source-of-truth precedence

For every installed tool, use this order:

1. local executable `--help` and observed behavior;
2. documentation for the exact installed release;
3. tagged release notes/source;
4. current official documentation;
5. repository `main`, issues, and third-party material only as supplementary context.

Do not infer current CLI flags from this specification after tooling upgrades.

### 94.1 Current Rust/Python packaging anchors at reference date

At the 2026-08-19 reference date, the specification was written against the contemporary PyO3 0.29.x and Maturin 1.14.x generation. The manifest examples are therefore concrete enough to bootstrap the project, but the lockfiles and installed tool help remain execution authority.

---

## 95) Primary references

### Rust and Cargo

- Rust Cargo project layout: <https://doc.rust-lang.org/cargo/guide/project-layout.html>
- Cargo targets and integration tests: <https://doc.rust-lang.org/cargo/reference/cargo-targets.html>
- Cargo profiles: <https://doc.rust-lang.org/cargo/reference/profiles.html>
- Rust build performance guidance: <https://nnethercote.github.io/perf-book/build-configuration.html>
- Rustup components: <https://rust-lang.github.io/rustup/concepts/components.html>
- rust-analyzer manual: <https://rust-analyzer.github.io/book/>
- Miri: <https://github.com/rust-lang/miri>

### Rust development tooling

- cargo-nextest: <https://nexte.st/>
- cargo-llvm-cov: <https://github.com/taiki-e/cargo-llvm-cov>
- Insta: <https://insta.rs/>
- cargo-mutants: <https://mutants.rs/>
- Rust Fuzz Book: <https://rust-fuzz.github.io/book/>
- cargo-hack: <https://github.com/taiki-e/cargo-hack>
- cargo-msrv: <https://github.com/foresterre/cargo-msrv>
- cargo-semver-checks: <https://github.com/obi1kenobi/cargo-semver-checks>
- cargo-shear: <https://github.com/Boshen/cargo-shear>
- cargo-machete: <https://github.com/bnjbvr/cargo-machete>
- cargo-udeps: <https://github.com/est31/cargo-udeps>
- cargo-deny: <https://embarkstudios.github.io/cargo-deny/>
- cargo-audit / RustSec: <https://github.com/rustsec/rustsec/tree/main/cargo-audit>
- cargo-vet: <https://mozilla.github.io/cargo-vet/>
- cargo-geiger: <https://github.com/geiger-rs/cargo-geiger>
- cargo-auditable: <https://github.com/rust-secure-code/cargo-auditable>
- cargo-expand: <https://github.com/dtolnay/cargo-expand>
- cargo-show-asm: <https://github.com/pacak/cargo-show-asm>
- cargo-binutils: <https://github.com/rust-embedded/cargo-binutils>
- cargo-bloat: <https://github.com/RazrFalcon/cargo-bloat>
- Hyperfine: <https://github.com/sharkdp/hyperfine>
- Samply: <https://github.com/mstange/samply>
- cargo-flamegraph: <https://github.com/flamegraph-rs/flamegraph>
- Cross: <https://github.com/cross-rs/cross>
- cargo-zigbuild: <https://github.com/rust-cross/cargo-zigbuild>
- sccache: <https://github.com/mozilla/sccache>
- Just: <https://just.systems/man/en/>
- Bacon: <https://dystroy.org/bacon/>
- Watchexec: <https://github.com/watchexec/watchexec>
- cargo-binstall: <https://github.com/cargo-bins/cargo-binstall>
- cargo-update: <https://github.com/nabijaczleweli/cargo-update>

### Python interface and package toolchain

- PyO3 guide: <https://pyo3.rs/>
- Maturin guide: <https://www.maturin.rs/>
- uv projects/dependencies: <https://docs.astral.sh/uv/concepts/projects/dependencies/>
- Ruff configuration: <https://docs.astral.sh/ruff/configuration/>
- Pyrefly configuration: <https://pyrefly.org/en/docs/configuration/>
- pytest: <https://docs.pytest.org/>

---

# Closing architecture

The repository should remain structurally simple even though the development environment is sophisticated:

```text
                       PUBLIC PYTHON PACKAGE
                     python/myproject/...
                             │
                             ▼
                      myproject._native
                             │
                             ▼
                  Rust↔Python binding boundary
                  (source location unspecified)
                             │
                             ▼
                  ┌──────────────────────┐
                  │   ONE RUST CRATE     │
                  │                      │
                  │ production source   │
                  │ organization is     │
                  │ application-defined │
                  └──────────────────────┘
                             │
                  ┌──────────┴───────────┐
                  ▼                      ▼
             Rust test suite       Python interface/
                                  wheel test suite
```

Around that deliberately unconstrained product-source organization sits a strongly specified evidence system:

```text
edit correctly
  rust-analyzer + Bacon + sccache

operate consistently
  just + pinned configuration + uv

test behavior
  nextest + doctests + pytest + wheel tests

measure test reach/strength
  llvm-cov + Insta + mutants + fuzz

interrogate unsafe behavior
  Miri + Geiger + fuzz + reasoning

preserve configuration compatibility
  cargo-hack + optional MSRV/semver gates

control dependency risk
  Machete/Shear/Udeps + deny + audit + optional vet

understand compiled artifacts
  expand + show-asm + binutils + bloat

make performance claims
  Hyperfine + Samply/flamegraph

deliver beyond the host
  Maturin + optional Cross/cargo-zigbuild
```

The governing rule is:

> **Specify the repository and evidence architecture rigorously; leave semantic production-code decomposition to the application design.**

For this hobby-scale project, the specification therefore constrains package boundaries, language boundaries, testing topology, tooling, reproducibility, CI, and release evidence—while intentionally making no judgment about whether production functionality belongs in one file, many files, nested modules, or semantic folders.
