# CodeFabric operational API (spec section 14).
#
# This file is the first command surface an agent should inspect (spec sections 59 and
# 92). Recipes express intent, not tool flags, so implementations can change without
# invalidating what callers know.
#
# Two rules govern everything below:
#
#   1. Mutating recipes are never dependencies of a validation recipe (spec section
#      14.1). Everything in the [mutating] group must be invoked deliberately, and its
#      diff inspected.
#   2. Availability is not a mandate to run (spec section 73.1). Pick the smallest tool
#      set that answers the risk question in the section 60 change-risk table, then
#      escalate. Running every deep tool after every edit is an anti-pattern.

set shell := ["bash", "-euo", "pipefail", "-c"]

# Variadic recipes forward their arguments with "$@" rather than {{ args }}.
# Without this, just re-expands the interpolated string and a quoted argument
# containing a space is silently re-split -- so `just spec-outline <path>
# --match '^5. Authority'` would search for `^5.` and treat `Authority` as a
# second path, returning a wrong outline instead of an error.
set positional-arguments

default:
    @just --list

# ------------------------------------------------------------------ environment

[doc("Environment report: toolchains, versions, required tools, direnv state")]
[group('environment')]
doctor:
    ./scripts/bootstrap.sh

[doc("Capture a non-secret tooling inventory to target/tooling-inventory.txt")]
[group('environment')]
inventory:
    ./scripts/tooling_inventory.sh
    @echo "wrote target/tooling-inventory.txt"

[doc("Resolved package metadata, no dependencies")]
[group('environment')]
metadata:
    cargo metadata --format-version 1 --no-deps

# sccache is a committed build prerequisite (.cargo/config.toml). Spec section 13.2:
# watch the hit rate rather than assuming the wrapper helps.

[doc("sccache hit rate and cache state")]
[group('environment')]
cache-stats:
    sccache --show-stats

[doc("Zero sccache statistics before a measurement run")]
[group('environment')]
cache-zero-stats:
    sccache --zero-stats

[doc("Navigate docs/upfront_design by section without reading whole specs")]
[group('environment')]
spec-outline *args:
    ./scripts/spec-outline.sh "$@"

[doc("Navigate docs/library_ref by chapter without reading whole references")]
[group('environment')]
lib-outline *args:
    ./scripts/lib-outline.sh "$@"

# --------------------------------------------------- formatting / static feedback

[doc("Check formatting of Rust and Python sources")]
[group('static')]
fmt:
    cargo fmt --all -- --check
    uv run ruff format --check python python_tests

# Both compile surfaces (spec section 26.2): the featureless core must build without a
# Python runtime, and the PyO3 adapter must build with it. A Python-only dependency must
# never leak into the featureless core.

[doc("Type-check both compile surfaces: featureless core and python feature")]
[group('static')]
check:
    cargo check --all-targets
    cargo check --all-targets --features python

[doc("Clippy on both compile surfaces, warnings denied")]
[group('static')]
clippy:
    cargo clippy --all-targets -- -D warnings
    cargo clippy --all-targets --features python -- -D warnings

[doc("Ruff lint over the Python facade and its tests")]
[group('static')]
python-lint:
    uv run ruff check python python_tests

[doc("Pyrefly type check")]
[group('static')]
python-type:
    uv run pyrefly check

[doc("Spelling and identifier hygiene")]
[group('static')]
typos:
    typos

# ------------------------------------------------------------------------ tests

[doc("Rust tests via nextest (does NOT include doctests)")]
[group('test')]
test-rust:
    cargo nextest run

# nextest does not run doctests. This is a separate, mandatory step -- never report "all
# Rust tests passed" from nextest alone (spec sections 18.2 and 62.2).

[doc("Rust doctests -- nextest does not cover these")]
[group('test')]
doctest:
    cargo test --doc

# Fast local iteration path only. A development install is never packaging evidence
# (spec sections 44 and 62.3); that is what wheel-test is for.

[doc("Build and install the native extension into the local environment")]
[group('test')]
python-develop:
    uv run maturin develop

[doc("Python interface tests against a development install")]
[group('test')]
test-python: python-develop
    uv run pytest

[doc("Everything: Rust tests, doctests, and Python interface tests")]
[group('test')]
test: test-rust doctest test-python

# ----------------------------------------------------------------- fast / PR gates

[doc("Fast unused-dependency hygiene")]
[group('gate')]
deps-fast:
    cargo machete
    cargo shear --deny-warnings

[doc("Dependency policy and known-advisory scan")]
[group('gate')]
policy:
    cargo deny check
    cargo audit

# The routine baseline. Run it before editing and record pre-existing failures separately
# from anything the edit causes (spec section 59.1).

[doc("The routine gate: format, check, lint, types, tests, typos, dep hygiene")]
[group('gate')]
ci-fast: fmt check clippy python-lint python-type test typos deps-fast

[doc("ci-fast plus policy, the ci nextest profile, and snapshot review state")]
[group('gate')]
ci-pr: ci-fast policy
    cargo nextest run --features python -P ci
    cargo test --doc --features python
    cargo insta pending-snapshots

# ------------------------------------------------------- coverage / test quality

# Coverage answers what executed, not whether assertions constrain behavior; pair it with
# mutation testing (spec sections 21 and 62.4). No percentage threshold is configured --
# section 21.1 warns against adopting one merely because the tool supports it.

[doc("Rust line coverage to target/coverage/lcov.info")]
[group('quality')]
coverage:
    mkdir -p target/coverage
    cargo llvm-cov nextest \
      --all-features \
      --lcov \
      --output-path target/coverage/lcov.info

[doc("Interactively review pending snapshot diffs")]
[group('quality')]
snapshots-review:
    cargo insta review

# A surviving mutant is not automatically a bug (spec section 22). Triage it against
# coverage: uncovered plus surviving means establish reachability first; covered plus
# surviving means strengthen the assertion.

[doc("Mutation-test one changed file")]
[group('quality')]
mutants-file path:
    cargo mutants -f {{path}}

# Nightly is a targeted analysis toolchain, not the repository default (spec section 10).
# Miri explores executions; it never proves soundness (spec section 24.2). Record
# toolchain, seed range, and exclusions with any finding.

[doc("Miri UB check on the default toolchain's nightly")]
[group('quality')]
miri:
    cargo +nightly miri test

[doc("Miri across a range of randomized seeds")]
[group('quality')]
miri-seeds seeds="16":
    MIRIFLAGS="-Zmiri-many-seeds=0..{{seeds}}" cargo +nightly miri test

[doc("Compiler-oriented unused-dependency adjudication")]
[group('quality')]
udeps:
    cargo +nightly udeps --all-targets --all-features

# Bounded runs only; long campaigns belong in scheduled infrastructure (spec section 23).
# There is no fuzz/ directory yet -- add one when a parser or untrusted-input surface
# actually exists, not because cargo-fuzz is installed.

[doc("Bounded fuzz run against one target")]
[group('quality')]
fuzz target seconds="60":
    cargo fuzz run {{target}} -- -max_total_time={{seconds}}

[doc("Coverage of a fuzz corpus")]
[group('quality')]
fuzz-coverage target:
    cargo fuzz coverage {{target}}

# --------------------------------------------------------- feature / compatibility

# `--all-features` validates only the maximal additive union and can hide accidental
# coupling between features (spec sections 26 and 62.6).

[doc("Check every feature in isolation")]
[group('compat')]
features-each:
    cargo hack check --each-feature

[doc("Check with no default features")]
[group('compat')]
features-no-default:
    cargo hack check --no-default-features

[doc("Verify the declared MSRV (inert until Cargo.toml declares rust-version)")]
[group('compat')]
msrv:
    cargo msrv verify

[doc("Rust API compatibility against a baseline revision")]
[group('compat')]
semver baseline:
    cargo semver-checks --baseline-rev {{baseline}}

# ------------------------------------------------------- dependency / supply chain

# cargo-vet is deliberately NOT adopted (spec sections 32 and 93.4): maintaining human
# audit attestations is real work, and a supply-chain/ directory should not be created
# just to have one. Add a `vet` recipe when that workflow is genuinely intended.
#
# Geiger's count is an inventory, not a vulnerability score (spec section 33). Use it to
# decide where Miri, fuzzing, and manual review should focus.

[doc("Unsafe-code surface inventory")]
[group('supply-chain')]
unsafe-surface:
    cargo geiger

# ---------------------------------------------------------------- package validation

[doc("Build a release wheel into dist/")]
[group('package')]
wheel:
    rm -rf dist
    uv run maturin build --release --out dist

# Separate from test-python by design: a development install proves nothing about
# packaging (spec sections 44.2, 45, and 62.3).

[doc("Build a wheel and prove it installs and imports in a clean environment")]
[group('package')]
wheel-test: wheel
    ./scripts/wheel_test.sh

# ------------------------------------------------ artifact / performance investigation

# `target/` disk usage is not artifact size (spec sections 39 and 82.2). Measure the
# final wheel or binary. Profiling locates a hotspot; only a controlled benchmark
# verifies an improvement (spec section 40.4).

[doc("Attribute release code size by crate")]
[group('perf')]
bloat:
    cargo bloat --release --crates

[doc("Symbols in the release artifact")]
[group('perf')]
symbols:
    cargo nm --release

[doc("Section sizes of the release artifact")]
[group('perf')]
sections:
    cargo size --release

[doc("MIR, LLVM IR, or assembly for one function")]
[group('perf')]
asm *args:
    cargo asm "$@"

[doc("Release codegen with symbols preserved, for samply or flamegraph")]
[group('perf')]
profile-build:
    cargo build --profile profiling

# ------------------------------------------------------------- mutating operations
#
# Everything below changes source, manifests, or the environment. None of it is a
# dependency of any gate. After running one: inspect the diff, identify the semantic
# impact, rerun the relevant validation, and disclose what the tool changed
# (spec section 63).

[doc("MUTATES: rewrite Rust and Python formatting in place")]
[group('mutating')]
fmt-write:
    cargo fmt --all
    uv run ruff format python python_tests

[confirm("typos -w rewrites source in place; identifier fixes can be API changes. Continue?")]
[doc("MUTATES: apply spelling corrections to source")]
[group('mutating')]
typos-write:
    typos -w

[confirm("Accept all pending snapshots without semantic review. Continue?")]
[doc("MUTATES: accept pending snapshots -- never a CI step or automatic fix")]
[group('mutating')]
snapshots-accept:
    cargo insta accept

[confirm("cargo shear --fix edits Cargo.toml; scanners produce hypotheses, not permission. Continue?")]
[doc("MUTATES: remove dependencies cargo-shear reports as unused")]
[group('mutating')]
deps-fix:
    cargo shear --fix

[doc("List available updates for globally installed Cargo executables")]
[group('mutating')]
tool-updates-check:
    cargo install-update --list
