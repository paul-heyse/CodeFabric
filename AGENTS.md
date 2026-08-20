# AGENTS.md — repository infrastructure

What is in this repository, why each piece was chosen, and what it does and does not prove.

This file documents the **package, tooling, and assurance architecture**. It is the
answer to "what can this repo do, and what did we already decide?" It deliberately says
nothing about how production code should be organized — see *Scope boundary* below.

| File | Answers |
|---|---|
| **AGENTS.md** (this file) | infrastructure decisions, capabilities, evidence model |
| `README.md` | what CodeFabric is, how to bootstrap, the handful of commands you need |
| `CLAUDE.md` | the system being built: the design specs, cross-cutting doctrine, research tooling |
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

> **Status: pre-implementation.** The infrastructure described here is in place and
> verified end to end. What is in `src/` and `python/codefabric/` is a deliberately
> minimal seed whose only job was to prove the toolchain — a `version()` accessor and a
> `normalize_workspace_id()` that can fail. It is not a design. Replace it.

---

## 0. Design corpus map — what is being built

The system itself is specified in two sibling directories. They are **design inputs, not
infrastructure**: deliberately not an input to any package/repository/tooling decision here
(see *Scope boundary*), revised in place while in flux, and navigated with
`just spec-outline` rather than read whole. `CLAUDE.md` carries the composition story and
the cross-cutting doctrine; this section is only the finding aid.

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

---

## 1. Scope boundary — what was *not* decided

The governing specification governs package/repository/tooling architecture **only**. It
explicitly declines to decide (repo-spec §2, §45 scope note):

- which production concepts deserve their own Rust source file;
- whether a concept is a file, an inline module, a nested module, or a folder;
- how domain functionality is grouped or named;
- whether the Python façade is one file or several;
- naming conventions for semantic production directories.

**Do not infer a preferred decomposition from the current seed.** `src/lib.rs` and
`src/python.rs` are a free choice and say so in their own doc comments. The only
structural constraints are the ones in *Repository shape* and *The architectural
invariant* below.

Equally: do not create a new crate to organize production code. That needs a
package/build justification from repo-spec §0.3 — independent reuse, dependency
isolation, distinct platform requirements, an independent release lifecycle, a separately
built artifact, or *measured* compilation benefit. "Another conceptual area" and "the file
got long" are explicitly insufficient (repo-spec §61.2).

---

## 2. Repository shape

One Cargo package, one library crate. No `[workspace]` table, no `crates/` directory
(repo-spec §0.3, §1.1, §77).

```
CodeFabric/
├── Cargo.toml  Cargo.lock  rust-toolchain.toml     Rust package + pins
├── pyproject.toml  uv.lock  .python-version        Python package + pins
├── justfile                                        the operational API
├── src/                     Rust crate      lib.rs · python.rs   (layout: free choice)
├── python/codefabric/       public Python package
│   ├── __init__.py          the supported contract
│   ├── py.typed             PEP 561 marker
│   └── _native.pyi          hand-written stub for the private extension
├── tests/                   ONE integration target: integration.rs + integration/
├── python_tests/            interface tests: test_api.py · test_packaging.py
├── scripts/                 bootstrap · wheel_test · tooling_inventory · doc outliners
├── tooling/ast-grep/        doc-navigation extractors (see CLAUDE.md)
├── .cargo/config.toml       sccache wrapper
├── .config/nextest.toml     test profiles
├── .github/workflows/ci.yml Tier A only
├── bacon.toml  deny.toml  _typos.toml  clippy.toml  .envrc
└── target/  dist/           generated, ignored
```

**Absent on purpose** — each has a named trigger that would justify creating it:

| Not here | Add it when |
|---|---|
| `crates/`, `[workspace]` | a second package clears repo-spec §0.3, measured first (§79) |
| `fuzz/` | a parser, decoder, protocol, or untrusted-input surface exists (§23) |
| `benches/` | a stable performance workload exists worth defending (§1) |
| `supply-chain/` (cargo-vet) | dependency trust becomes an engineering goal (§32) |
| `tests/fixtures/` | a test needs reusable non-code data (§4.4) |
| `deep-assurance.yml`, `wheels.yml` | the surfaces they cover exist; a permanently-red deep workflow is worse than none (§52) |
| `scripts/coverage_python.sh` | cross-language Rust-through-Python coverage becomes worth the wiring (§21.2) |
| `src/main.rs`, `src/bin/` | a CLI is needed — same package, reusing the lib target (§3) |

### 2.1 Why Python has its own source root

`python/codefabric/` is a **packaging/language-boundary** choice required by Maturin's
mixed layout, not a semantic decomposition rule (repo-spec §1.2). It keeps Python sources
out of Cargo's conventional `src/`.

One consequence is easy to miss: because Python sources are not at the repository root,
Ruff's default `src = ["."]` cannot distinguish first-party from third-party imports, so it
sorts every import block wrongly. `pyproject.toml` therefore sets
`src = ["python", "python_tests"]`. The spec's example manifest assumes a default layout
and omits this.

### 2.2 Rust test topology

Cargo compiles **every** top-level `tests/*.rs` as its own integration-test crate. So
there is exactly one: `tests/integration.rs`, whose cases live in `tests/integration/`
(repo-spec §4.2, §61.3). A second top-level target needs a materially different feature
set, process environment, harness, external service, platform restriction, or resource
group (§4.3) — not just another subsystem.

The inline `mod integration { … }` wrapper inside `tests/integration.rs` is load-bearing:
a test crate root resolves submodules against `tests/`, so without it `mod errors;` would
look for `tests/errors.rs` and every case would need its own top-level file — precisely
the crate explosion §4.2 exists to prevent.

Most tests are colocated `#[cfg(test)]` modules beside the implementation, so private
invariants are testable without widening visibility (§4.1).

---

## 3. The architectural invariant: two compile surfaces

Rust is the implementation core; Python is the interface layer. The dependency direction
is one-way and is the one thing here that is genuinely load-bearing.

```
Python caller → python/codefabric/ → codefabric._native → src/  (Python-agnostic)
```

PyO3 lives behind an optional Cargo feature (repo-spec §6.1, §26.2):

```toml
[features]
default = []
python = ["dep:pyo3"]
```

which produces two surfaces that `just check` and `just clippy` both exercise:

| Surface | Command | Dependency graph |
|---|---|---|
| pure Rust core | `cargo check --all-targets` | **zero dependencies** |
| core + PyO3 adapter | `cargo check --all-targets --features python` | 13 packages, rooted at `pyo3` |

That zero is the invariant made measurable: the core builds and tests with no Python
runtime present, exactly as a non-Python Rust consumer would see it. **A Python-only
dependency must never leak into the featureless core.** Verify with `cargo tree
--no-default-features`.

Three related decisions:

- **No `pyo3/extension-module` feature** (repo-spec §6.2, anti-pattern §81.5). Maturin
  sets the extension-build environment for the build that needs it; enabling that feature
  globally interferes with ordinary Rust test linking. `Cargo.toml` carries
  `pyo3 = { version = "0.29.2", optional = true }` and nothing more.
- **`crate-type = ["rlib", "cdylib"]`** — one target serving both ordinary Rust consumers
  and the Maturin extension.
- **`#[pymodule(name = "_native")]`** must match the last component of Maturin's
  `module-name = "codefabric._native"` (repo-spec §6.3). Those two strings are coupled.

Error translation is a boundary responsibility (§6.5). `src/python.rs` centralizes
`crate::Error → PyErr`; centralizing is a choice, the *predictability* is the requirement,
and `python_tests/test_api.py` covers the mapping from the Python side.

`codefabric._native` is **private** (§5.1, §61.5). Import from `codefabric`. Only
narrowly-scoped binding-contract tests may touch `_native` (§19.2) — in this repo, exactly
one test in `test_packaging.py`.

---

## 4. Manifests, pins, and the decisions inside them

### Rust

| Decision | Value | Why |
|---|---|---|
| toolchain channel | `stable` | nightly is a *targeted analysis toolchain* for `just miri`/`just udeps` only, never the default (repo-spec §10) |
| components | `rustfmt`, `clippy`, `rust-analyzer`, `rust-src`, `llvm-tools-preview` | `llvm-tools-preview` is the substrate for coverage, binutils and fuzz-coverage (tooling-ref §8); `rust-src` gives semantic tools stdlib source (§7) |
| `rustc-dev` | **deliberately absent** | declaring it couples the repo to compiler-private APIs and forces an exact date-pinned nightly (repo-spec §76). If MIR extraction is ever built, that is a separate architecture decision, not an edit to this file |
| `rust-version` | **not declared** | do not advertise an MSRV that is not verified (§27). `just msrv` exists and is inert until this appears |
| `license` | **not declared** | no license chosen yet; `publish = false` stops Cargo warning. `deny.toml` sets `licenses.private.ignore = true` so our own crate is not reported "Unlicensed" |
| lints | `unsafe_code = "deny"`, clippy `all` + `pedantic` = warn | there is no first-party `unsafe`; preserving that is a useful default (§33) |
| `clippy.toml` | five `doc-valid-idents` words + the `..` defaults marker | `pedantic` enables `doc_markdown`, which wants backticks around proper nouns in prose. The list names known-good words — it does **not** disable the lint (§9.1) |
| dev profiles | `debug = "line-tables-only"`, deps `debug = false` | useful source locations without inflating `target/` (§9.2) |
| extra profiles | `debugging` (full debug), `profiling` (release + symbols, `strip = "none"`) | profiling needs release codegen with symbols preserved (§40.2) |
| release tuning | **none** | `lto`, `codegen-units = 1`, `panic = "abort"`, `strip` are not added by folklore; measure first (§9.3) |

### Python

| Decision | Value | Why |
|---|---|---|
| build backend | Maturin `>=1.14,<2` | mixed Rust/Python project layout (§11) |
| floor | `>=3.14` | declared in **five places that must move together**: `pyproject.toml`, `.python-version`, Ruff `target-version`, Pyrefly `python-version`, and CI (§11.1). CI currently runs a single interpreter, not a matrix |
| dev tooling | `[dependency-groups] dev` | lint/test tools are not runtime dependencies of the package (§11.2) |
| Ruff | formatter *and* linter; `select = ["E","F","I","UP","B","SIM"]` | one Python tool surface, not three (§42) |
| Pyrefly | explicitly configured `project-includes` | an unconfigured checker enables only a narrow high-confidence set; configuring it makes the intended type surface a decision (§43) |
| typing | `py.typed` + hand-written `_native.pyi` | native runtime signatures are not the public typing contract; a private stub plus a typed façade lets the Python API stay richer and more stable than the raw FFI surface (§8) |

Both lockfiles are committed. `Cargo.lock`, `uv.lock`, `.config/nextest.toml`,
`bacon.toml`, `deny.toml`, `_typos.toml`, `clippy.toml`, `justfile`, `py.typed` and
`_native.pyi` are the repository contract and are deliberately **not** gitignored
(repo-spec §54).

---

## 5. The command contract

`just --list` is the first thing to read (repo-spec §14, §59, §92). Recipes express
**intent**, not tool flags, so implementations can change without invalidating what
callers know. Ten groups:

`environment` · `static` · `test` · `gate` · `quality` · `compat` · `supply-chain` ·
`package` · `perf` · `mutating`

Two rules govern all of it:

1. **Mutating recipes are never dependencies of a gate** (§14.1). The four that
   change state — `fmt-write`, `typos-write`, `snapshots-accept`, `deps-fix` — must be
   invoked deliberately and their diff inspected; three carry `[confirm(...)]` prompts.
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
interaction. A large `target/` is not evidence that the wheel is large.

### One continuous checker

Ownership is divided (repo-spec §15.1, tooling-ref §72.22):

```
rust-analyzer  →  semantic model, references, types, assists
Bacon          →  the persistent cargo check job          (bacon.toml)
Watchexec      →  non-Rust tasks and process restarts only
```

Do not configure editor check-on-save, Bacon, and Watchexec to run the same
`cargo check`. `bacon.toml` defines `check`, `check-python-feature`, `clippy` and
`nextest` jobs, and exports `.bacon-locations` for editor/agent consumption — an agent
must confirm that file matches the current source generation before reading an empty list
as success (§15.2).

### Shell environment — the trap that matters for agents

`direnv` applies `.envrc` in **interactive shells only**. Agent harnesses typically run
each command in a fresh non-interactive shell that inherits nothing. So invoke tools one
of three ways rather than assuming an activated environment:

```bash
uv run <cmd>                     # Python
direnv exec . <cmd>              # full .envrc environment, non-interactively
. scripts/bootstrap.sh && <cmd>  # within one compound command
```

`scripts/bootstrap.sh` is the shared mechanism: sourcing it applies the environment,
running it checks one. `--quiet` is silent when healthy; `--context` emits a compact block
for agent context injection.

**One coupling worth knowing:** the build backend is Maturin, so `uv sync` compiles the
Rust extension — including the `uv sync` inside `.envrc`. A broken Rust build therefore
degrades the Python environment, not just the Rust one. `.envrc` treats sync failure as
non-fatal, so the symptom is a stale `.venv` rather than an unenterable directory.

---

## 7. Test architecture and what each layer proves

Evidence is **orthogonal**, not redundant (repo-spec §25). Current state: 9 nextest tests,
2 doctests, 12 pytest tests — all green.

| Question | Instrument | Recipe |
|---|---|---|
| Do ordinary Rust tests pass? | cargo-nextest | `just test-rust` |
| Do documented examples still work? | `cargo test --doc` | `just doctest` |
| Does the Python interface behave? | pytest against a dev install | `just test-python` |
| Which Rust regions executed? | cargo-llvm-cov | `just coverage` |
| Do assertions detect plausible faults? | cargo-mutants | `just mutants-file <path>` |
| What inputs find new behavior? | cargo-fuzz | `just fuzz <target>` *(no targets yet)* |
| Did structured output change? | cargo-insta | `just snapshots-review` |
| Do unsafe/concurrent executions violate Rust's rules? | Miri | `just miri`, `just miri-seeds` |
| Does the built wheel install and import? | clean-env wheel test | `just wheel-test` |

### Four traps the tooling will not catch for you

1. **`cargo nextest` does not run doctests.** Never report "all Rust tests passed" from
   nextest alone. `just test` covers both; `just test-rust` does not (§18.2, §62.2).
2. **`maturin develop` is not packaging evidence.** Only `just wheel-test` — a clean-env
   install of the built artifact — validates the wheel (§44.2, §62.3). A stale editable
   install produces convincing false passes, which is why `scripts/wheel_test.sh` asserts
   the *import origin* resolves inside the temporary venv. It also refuses to run when
   `dist/` holds more than one wheel, so a stale artifact cannot become the thing under
   test, and it prints the wheel name, sha256, and interpreter version as the record
   (§45 item 6).
3. **`--all-features` is not a feature matrix.** It validates only the maximal additive
   union and hides accidental coupling. Use `just features-each` (§26.1, §62.6).
4. **Coverage is not test strength.** A covered line may assert nothing. Triangulate:
   uncovered + surviving mutant → establish reachability first; covered + surviving mutant
   → strengthen the assertion (§21, §22.1, §62.4).

### nextest profiles

`.config/nextest.toml` defines `default` (fail-fast, 60s slow timeout) and `ci`
(no fail-fast, exponential retries, `flaky-result = "fail"`, 2h global timeout, JUnit
output). **Retries are diagnostic**: a test that only passes on rerun still fails the
build (§18.1). When tests come to share a database, port, or other scarce resource,
constrain them with a nextest test group rather than serializing the whole suite (§18.3) —
the file carries a commented example.

### Python test posture

Python tests exercise the **public package**, not the private extension (§19.2), and
deliberately do not replay the Rust suite. Rust owns domain validation; what Python must
prove is that the interface accepts expected values, converts correctly in both
directions, maps errors as documented, and that packaging/import works (§19.1).

---

## 8. Assurance tiers — what runs where

Not every installed tool belongs on the critical path of every commit (repo-spec §49).

**Tier A — every meaningful change.** Wired into both `just ci-fast` and
`.github/workflows/ci.yml`:

```
fmt · check (both surfaces) · clippy (both surfaces) · ruff lint · pyrefly
· nextest · doctests · pytest · typos · machete · shear        [+ deny · audit in CI]
```

`just ci-pr` adds `policy` (deny + audit), the `ci` nextest profile with the `python`
feature, feature-flagged doctests, and `cargo insta pending-snapshots`.

**Tier B — conditional.** Available as recipes, not wired into CI: `coverage`,
`features-each`, `features-no-default`, `wheel-test`, `semver`, `msrv`.

**Tier C — risk-triggered or scheduled.** Available as recipes: `miri`, `miri-seeds`,
`mutants-file`, `fuzz`, `udeps`, `unsafe-surface`, plus the whole `perf` group.

**CI deliberately implements Tier A only.** There is no unsafe code, no fuzz surface and
no benchmark baseline yet, and §52 is explicit that a permanently-red deep-assurance
workflow is worse than a smaller meaningful suite. Every CI step mirrors a justfile
recipe — keep them in sync; the justfile is the API and CI exists for per-step
granularity. Actions are pinned to commit SHAs and Rust CLIs to explicit versions, so a
tool release cannot silently change a merge gate (§50.1). CI runs `uv sync --frozen` so a
lock mismatch fails rather than being silently rewritten (§50.2).

### Choose evidence by change risk

Classify before validating (repo-spec §60). The rows most likely to apply here:

| Change | Minimum additional evidence |
|---|---|
| comment/docs only | Ruff / Typos as relevant |
| local safe Rust logic | check + Clippy + targeted nextest |
| public Python façade | pytest + Pyrefly + Ruff; wheel test if packaging-significant |
| PyO3 conversion/binding | Rust tests + pytest + Maturin build |
| error mapping | Rust error tests **and** Python exception tests |
| Cargo feature | `features-each` + featureless and `python` builds |
| dependency | `deps-fast` + `policy` + tests + feature matrix |
| unsafe/pointer/concurrency | Geiger + Miri + native tests |
| parser/protocol | coverage + fuzz + snapshots + mutation testing |
| performance claim | Hyperfine before/after + profiler |
| Python packaging | fresh wheel install + pytest |

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

- **`all-features = true`.** The wheel ships with the `python` feature enabled, so the
  featureless graph is not the graph that gets distributed. Auditing the wrong dependency
  graph is a named failure mode (tooling-ref §72.16); without this, `pyo3` and its whole
  subtree are invisible to every check.
- **Permissive licenses only.** CodeFabric distributes a wheel containing statically
  linked Rust dependencies, so a copyleft or source-disclosure obligation would attach to
  the distributed artifact. The allow-list is *policy*, not a mirror of today's graph —
  `unused-allowed-license = "allow"` keeps unused entries from being findings (§30.1).
- **`licenses.private.ignore = true`** because this package declares no `license` field
  (see *Manifests, pins, and the decisions inside them*). Revisit when a license is chosen.
- **`multiple-versions = "warn"`, not deny.** Duplicates deserve attention, not automatic
  failure — they are often forced by transitive constraints. Pair with `cargo tree -d`
  before attempting a resolution (§30.2).
- **crates.io only**, `unknown-git = "deny"`. A Git dependency pinned to a branch is
  materially less reproducible than one pinned to an immutable revision.

`cargo audit` green means the resolved graph matches no known RustSec advisory under the
current database — **not** that dependencies have been security-audited (§31).

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
| Test runner | cargo-nextest | Tier A | `just test-rust` |
| Python build/wheel | Maturin | Tier A | `just python-develop`, `just wheel` |
| Python lint + format | Ruff | Tier A | `just python-lint`, `just fmt` |
| Python types | Pyrefly | Tier A | `just python-type` |
| Spelling | Typos | Tier A | `just typos` |
| Unused deps (fast) | cargo-machete, cargo-shear | Tier A | `just deps-fast` |
| Dependency policy | cargo-deny, cargo-audit | Tier A (CI / `ci-pr`) | `just policy` |
| Continuous check | Bacon | local loop | `bacon` |
| Process/file triggers | Watchexec | targeted local | — |
| File discovery | `fd` | agent discovery | — |
| Coverage | cargo-llvm-cov | Tier B | `just coverage` |
| Feature matrix | cargo-hack | Tier B | `just features-each` |
| Wheel validation | `scripts/wheel_test.sh` | Tier B | `just wheel-test` |
| Rust API compatibility | cargo-semver-checks | Tier B — only if the Rust API becomes externally supported (§28) | `just semver <rev>` |
| MSRV | cargo-msrv | Tier B — inert until `rust-version` is declared | `just msrv` |
| Snapshots | cargo-insta | when snapshot-worthy output exists (§20) | `just snapshots-review` |
| UB / aliasing / races | Miri | Tier C, risk-triggered | `just miri`, `just miri-seeds` |
| Assertion strength | cargo-mutants | Tier C | `just mutants-file <path>` |
| Adversarial inputs | cargo-fuzz | Tier C — no `fuzz/` yet | `just fuzz <target>` |
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
usage is not artifact size — measure the wheel or binary (§39, §82.2).

---

## 11. Invariants for agents working here

1. **Read `just --list` first.** Prefer a recipe over reconstructing tool flags
   (repo-spec §59, §92).
2. **Run `just ci-fast` before editing** and record pre-existing failures separately from
   anything your change causes (§59.1).
3. **Classify the change, then choose evidence** from the §8 table. Do not escalate to
   Tier C for appearance.
4. **rust-analyzer is not the final authority** — confirm with the repository's Cargo
   commands (§62.1).
5. **Never report "all Rust tests passed" from nextest alone.** Doctests are separate.
6. **Never treat `maturin develop` as packaging evidence.**
7. **Preserve the dependency direction.** Before adding a PyO3 or Python-object dependency
   to code the core uses, confirm it genuinely belongs to the binding boundary (§61.1).
8. **Keep Python as an interface layer.** If you find yourself writing a second
   implementation of core behavior in Python, reassess the boundary (§61.4).
9. **Do not create a crate to organize code, or a second top-level test file** (§61.2,
   §61.3).
10. **Mutating recipes need a reason and a diff review.** `fmt-write`, `typos-write`,
    `snapshots-accept`, `deps-fix`, `cargo update`, `maturin publish` — none is ever an
    automatic fix (§14.1, §63).
11. **Miri never proves soundness.** Record toolchain, seed range, target and exclusions
    with any finding (§24.2, §62.5).
12. **Record the evidence, not the verdict** (§65): tool and version, toolchain and
    target, feature selection, profile, exact command, exit status, and whether the
    command mutated source, lockfiles, manifests or the environment.
