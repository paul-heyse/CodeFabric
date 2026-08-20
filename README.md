# CodeFabric

A present-state code property graph: a fact substrate over a codebase that answers
semantic questions about what the code *is* right now.

CodeFabric emits facts and mechanically derived facts. It does not emit judgments — no
risk scores, no complexity verdicts, no "safe to refactor". Where a provider cannot
answer, the result is an explicit unknown or capability gap, never an empty result
implying "none".

**Status: pre-implementation.** The repository and tooling architecture described below
is in place and verified end to end. The system itself is not built yet; the design specs
in `docs/upfront_design/` are still in flux.

## Architecture

Rust is the implementation core; Python is the interface layer. The dependency direction
is one-way and enforced architecturally:

```
Python caller
  → python/codefabric/          public Python package, the supported contract
    → codefabric._native        private PyO3 extension
      → src/                    Rust core, Python-agnostic
```

The Rust core builds and tests as an ordinary Rust library with no Python runtime
present. All PyO3 conversion sits behind the optional `python` Cargo feature, which gives
two deliberate compile surfaces:

```
cargo check                      pure Rust core
cargo check --features python    core plus the PyO3 adapter
```

`codefabric._native` is private. Import from `codefabric`; the extension's symbol layout
may change as bindings evolve.

This is **one Cargo package with one library crate**. There is no `crates/` directory and
no workspace. A second crate requires a package or build justification — independent
reuse, dependency isolation, distinct platform requirements, an independent release
lifecycle, or *measured* compilation benefit. "The code grew another area" is not one.

How the Rust core and the Python façade are divided into files internally is deliberately
not fixed by the repository contract.

## Supported platforms

| | |
|---|---|
| Operating systems | Linux and macOS |
| Python | 3.14 and later (`requires-python = ">=3.14"`) |
| Rust | current stable, pinned by `rust-toolchain.toml` |
| Wheels | CPython-specific; no `abi3` |

The Python floor is declared in five places that must move together: `pyproject.toml`,
`.python-version`, Ruff's `target-version`, Pyrefly's `python-version`, and CI.

**Nightly Rust is not required for normal development.** It is a targeted analysis
toolchain, used only by `just miri` and `just udeps`. The repository deliberately does not
declare `rustc-dev`.

## Bootstrap

```bash
uv python install     # if 3.14 is not present
uv sync               # Python environment; also builds the native extension
just doctor           # verify toolchains, required tools, and direnv state
```

Required beyond a Rust toolchain and uv: `just`, `sccache` (committed as a
`rustc-wrapper` in `.cargo/config.toml`, so builds fail without it), `cargo-nextest`,
`cargo-deny`, `cargo-audit`, `cargo-shear`, `cargo-machete`, and `typos`. Install them
with `cargo binstall`; `just doctor` reports what is missing.

`direnv` is optional and applies only to interactive shells. Non-interactive callers
should use `uv run <cmd>`, `direnv exec . <cmd>`, or source `scripts/bootstrap.sh`.

## Common commands

`just --list` is the full contract. The everyday ones:

```bash
just ci-fast      # the routine gate: format, check, clippy, lint, types, tests, typos, deps
just test         # Rust tests, doctests, and Python interface tests
just check        # both compile surfaces
just wheel-test   # build a wheel and prove it installs in a clean environment
just doctor       # environment report
```

Recipes in the `[mutating]` group change source, manifests, or the environment. They are
never dependencies of a gate and must be invoked deliberately.

Note that `just test-rust` does **not** cover doctests — `cargo nextest` cannot run them.
`just test` includes both.

## Building a wheel

```bash
just wheel        # release wheel into dist/
just wheel-test   # build, then install into a throwaway venv and run python_tests
```

`uv run maturin develop` is the fast local iteration path and is **not** packaging
evidence. Only a clean-environment install of the built artifact validates the wheel;
`scripts/wheel_test.sh` asserts the import resolves inside the temporary environment so a
stale editable install cannot produce a false pass.

Publishing is a separate, explicit action. It is never implied by building or testing.

## Where things go

| Path | Contents |
|---|---|
| `src/` | Rust crate source |
| `python/codefabric/` | public Python package |
| `tests/` | Rust integration tests — one target, cases in `tests/integration/` |
| `python_tests/` | Python interface tests |
| `scripts/` | operational scripts too complex for a `just` recipe |
| `docs/upfront_design/` | system design specs (in flux) |
| `docs/library_ref/` | version-pinned dependency references |
| `target/` | all build output and reports — generated, ignored |
| `dist/` | built wheels — generated, ignored |

`target/` disk usage says nothing about wheel or binary size; measure the artifact
directly with `just bloat` or `just sections`.

## Governing specification

`docs/rust_core_python_interface_repository_specification_2026-08-20.md` defines this
repository's package and tooling architecture, the assurance tiers, and the agent
operating rules. Section 60's change-risk table is the guide for which tools a given
change actually warrants.
