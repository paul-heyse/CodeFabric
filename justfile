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

[doc("Run the routine gate once and cache its verdict under target/")]
[group('environment')]
baseline:
    ./scripts/bootstrap.sh --baseline

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

[doc("Check formatting of stable-domain Rust sources")]
[group('static')]
root-fmt:
    cargo fmt --all -- --check

# The default local profile and the featureless substrate are both load-bearing.

[doc("Type-check default local and featureless stable-domain surfaces")]
[group('static')]
root-check:
    cargo check --all-targets
    cargo check --all-targets --no-default-features

[doc("Clippy on default local and featureless stable-domain surfaces")]
[group('static')]
root-clippy:
    cargo clippy --all-targets -- -D warnings
    cargo clippy --all-targets --no-default-features -- -D warnings

[doc("Spelling and identifier hygiene")]
[group('static')]
typos:
    typos

# ------------------------------------------------------------------------ tests

[doc("Rust tests via nextest (does NOT include doctests)")]
[group('test')]
root-test-rust:
    cargo nextest run

# nextest does not run doctests. This is a separate, mandatory step -- never report "all
# Rust tests passed" from nextest alone (spec sections 18.2 and 62.2).

[doc("Rust doctests -- nextest does not cover these")]
[group('test')]
root-doctest:
    cargo test --doc

[doc("Everything in the stable root: Rust tests and doctests")]
[group('test')]
root-test: root-test-rust root-doctest

# ----------------------------------------------------------------- fast / PR gates

[doc("Fast unused-dependency hygiene")]
[group('gate')]
deps-fast:
    cargo machete
    cargo shear --deny-warnings

[doc("Dependency policy and known-advisory scan")]
[group('gate')]
policy:
    ./scripts/advisory_policy_check.sh --audit
    cargo deny check --hide-inclusion-graph advisories bans sources

[doc("Validate exact advisory exceptions against lockfile, deny, and RustSec")]
[group('gate')]
advisory-policy-check:
    ./scripts/advisory_policy_check.sh

# The routine baseline. Run it before editing and record pre-existing failures separately
# from anything the edit causes (spec section 59.1).

[doc("Validate the exact resolved stable dependency and feature graph")]
[group('gate')]
stable-graph-check:
    ./scripts/stable_graph_check.sh

[doc("Run repository structural governance rules")]
[group('gate')]
governance-scan:
    ast-grep test --skip-snapshot-tests
    ast-grep scan \
      --globs '!contracts/generated/**' \
      --globs '!src/generated/**' \
      --globs '!codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated/**' \
      --globs '!rustc-extractor/src/generated/**' \
      --globs '!pyrefly-sidecar/src/generated/**'

[doc("The routine stable-root gate")]
[group('gate')]
root-ci-fast: root-fmt root-check root-clippy root-test typos deps-fast stable-graph-check

# --------------------------------------------------------- independent build domains

[doc("Format-check the dated-nightly rustc extractor")]
[group('extractor')]
extractor-fmt:
    cd rustc-extractor && cargo fmt --all -- --check

[doc("Compile and lint the dated-nightly rustc extractor")]
[group('extractor')]
extractor-check:
    cd rustc-extractor && cargo check --all-targets --locked
    cd rustc-extractor && cargo clippy --all-targets --locked -- -D warnings

[doc("Test the dated-nightly rustc extractor")]
[group('extractor')]
extractor-test:
    cd rustc-extractor && cargo test --locked

[doc("Launch the built extractor directly and verify exact stderr-only identity")]
[group('extractor')]
extractor-identity:
    #!/usr/bin/env bash
    set -euo pipefail
    (cd rustc-extractor && cargo build --locked)
    temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/codefabric-extractor-identity.XXXXXX")"
    trap 'rm -rf "$temporary_root"' EXIT
    ./scripts/run_rustc_extractor.sh --identity >"$temporary_root/stdout" 2>"$temporary_root/stderr"
    test ! -s "$temporary_root/stdout"
    cmp rustc-extractor/toolchain-identity.json "$temporary_root/stderr"

[doc("Run the complete extractor gate")]
[group('extractor')]
extractor-ci-fast: extractor-fmt extractor-check extractor-test extractor-identity

[doc("Format-check the stable Pyrefly sidecar")]
[group('sidecar')]
sidecar-fmt:
    cd pyrefly-sidecar && cargo fmt --all -- --check

[doc("Compile and lint the stable Pyrefly sidecar")]
[group('sidecar')]
sidecar-check:
    cd pyrefly-sidecar && cargo check --all-targets --locked
    cd pyrefly-sidecar && cargo clippy --all-targets --locked -- -D warnings

[doc("Test the stable Pyrefly sidecar")]
[group('sidecar')]
sidecar-test:
    cd pyrefly-sidecar && cargo test --locked

[doc("Check sidecar advisories, bans, and sources (licenses excluded)")]
[group('sidecar')]
sidecar-policy:
    cd pyrefly-sidecar && cargo deny check --hide-inclusion-graph advisories bans sources
    cd pyrefly-sidecar && cargo audit

[doc("Run the complete sidecar routine gate")]
[group('sidecar')]
sidecar-ci-fast: sidecar-fmt sidecar-check sidecar-test

[doc("Check adapter Ruff formatting and lint")]
[group('adapter')]
adapter-lint:
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp ruff format --check codefabric-cpg-mcp
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp ruff check codefabric-cpg-mcp

[doc("Type-check the configured adapter source and test trees")]
[group('adapter')]
adapter-type:
    cd codefabric-cpg-mcp && env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen pyrefly check

[doc("Test the locked FastMCP adapter")]
[group('adapter')]
adapter-test:
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp pytest codefabric-cpg-mcp/tests

[doc("Test locked-command STDIO startup, shutdown, and protocol silence")]
[group('adapter')]
adapter-stdio-test:
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp pytest codefabric-cpg-mcp/tests/test_stdio.py

[doc("Build and import the adapter wheel with its canonical artifact-index resource")]
[group('adapter')]
adapter-wheel-test:
    ./scripts/adapter_wheel_test.sh

[doc("Run the complete adapter gate")]
[group('adapter')]
adapter-ci-fast: adapter-lint adapter-type adapter-test

# -------------------------------------------------------- contracts / governance

[doc("Check formatting and lint for shared contract tooling")]
[group('contracts')]
contracts-tooling-lint:
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp ruff format --check tooling/contracts tooling/ci
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp ruff check tooling/contracts tooling/ci

[doc("Validate every catalog JSON Schema against the hermetic Draft 2020-12 metaschema")]
[group('contracts')]
schema-check: contracts-tooling-lint
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp pytest tooling/contracts/test_json_schema_check.py
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/contracts/json_schema_check.py

[doc("Verify fixture oracle classification and immutable normative-KAT boundaries")]
[group('contracts')]
fixture-check: contracts-tooling-lint
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp pytest tooling/contracts/test_fixture_candidates.py
    ./scripts/fixture_governance_check.sh

[doc("Verify adapter Contract-IR generation and structural governance")]
[group('contracts')]
adapter-contracts-governance: contracts-tooling-lint
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/contracts/generate_adapter_models.py check
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp pytest tooling/contracts/test_generate_adapter_models.py
    ./scripts/adapter_contract_governance_check.sh

[doc("Verify generated adapter contracts including FastMCP runtime equivalence")]
[group('contracts')]
adapter-contracts-check: adapter-contracts-governance
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp pytest codefabric-cpg-mcp/tests/test_adapter_contracts.py

[doc("Generate adapter Contract-IR outputs twice and compare exact bytes")]
[group('contracts')]
adapter-contracts-repro-check:
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/contracts/generate_adapter_models.py repro-check

[doc("Benchmark adapter model import, schema build, validation, and serialization")]
[group('perf')]
adapter-contracts-bench:
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/contracts/benchmark_adapter_contracts.py

[doc("Validate active plan, review, and schema-2 execution-state contracts")]
[group('gate')]
artifacts-check: contracts-tooling-lint
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp pytest tooling/ci/test_artifact_contracts.py tooling/ci/test_wave0_reconciliation.py
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/artifact_contracts.py artifacts-check

[doc("Derive active-plan input freshness and proving-commit trust")]
[group('gate')]
plan-status:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/artifact_contracts.py plan-status

[doc("Reject Cargo target outputs in the index or reachable HEAD history")]
[group('gate')]
tracked-target-zero-state-check:
    @env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/artifact_contracts.py tracked-target-zero-state-check

[doc("Validate catalog-v2 derivation units, resolved invocations, and legacy zero-state")]
[group('contracts')]
compilation-units-check:
    ./scripts/compilation_units_check.sh
    cargo nextest run --locked --no-default-features --features contracts-tooling -E 'test(wp06a)' --no-tests=fail

[doc("Prove Tier-A command coverage and materialize the exact current graph")]
[group('gate')]
proof-coverage-check: contracts-tooling-lint
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp pytest tooling/ci/test_proof_coverage.py
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/proof_coverage.py

[doc("Verify committed Protobuf outputs and generator identity")]
[group('contracts')]
proto-check:
    ./scripts/proto_dependency_check.sh
    cargo check --locked --no-default-features --features proto-tooling --bin codefabric-proto-gen
    cargo clippy --locked --no-default-features --features proto-tooling --bin codefabric-proto-gen -- -D warnings
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp ruff format --check tooling/proto/generate.py tooling/proto/test_generate.py
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT uv run --frozen --project codefabric-cpg-mcp ruff check tooling/proto/generate.py tooling/proto/test_generate.py
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp pytest tooling/proto/test_generate.py codefabric-cpg-mcp/tests/test_proto.py
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/proto/generate.py check

[doc("Generate twice in isolated roots and compare byte digests")]
[group('contracts')]
proto-repro-check: proto-check
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/proto/generate.py repro-check

[doc("Verify the AC-G-05 tree, JCS corpus, generated bytes, and negative fixtures")]
[group('contracts')]
contracts-verify: schema-check fixture-check
    cargo run --locked --no-default-features --features contracts-tooling --bin codefabric-contracts -- verify --profile full
    ./scripts/contracts_negative_check.sh

[doc("Require every contract artifact to be released with zero warnings")]
[group('contracts')]
contracts-verify-released:
    cargo run --locked --no-default-features --features contracts-tooling --bin codefabric-contracts -- verify --profile released

[doc("Generate contracts twice in isolated roots and compare exact bytes")]
[group('contracts')]
contracts-repro-check:
    ./scripts/contracts_repro_check.sh

[doc("Prove family duplicate policy and its expected-failure fixture")]
[group('gate')]
duplicate-family-check:
    ./scripts/duplicate_family_check.sh

[doc("Prove retired native-extension seed and root packaging surfaces stay absent")]
[group('gate')]
seed-zero-state-check:
    ./scripts/seed_zero_state_check.sh

[doc("Run structural, artifact, graph-policy, and generated-output governance")]
[group('gate')]
governance: governance-scan artifacts-check plan-status tracked-target-zero-state-check duplicate-family-check seed-zero-state-check proto-check contracts-verify contracts-repro-check adapter-contracts-governance adapter-contracts-repro-check proof-coverage-check

[doc("Run the routine gate across all four build domains")]
[group('gate')]
ci-fast: root-ci-fast extractor-ci-fast sidecar-ci-fast adapter-ci-fast governance

[doc("ci-fast plus policy, the ci nextest profile, and snapshot review state")]
[group('gate')]
ci-pr: ci-fast policy sidecar-policy proto-repro-check
    cargo nextest run -P ci
    cargo test --doc
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
      --features local-workstation \
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

# Nightly is the extractor's production toolchain and remains isolated from this root.
# Miri explores executions; it never proves soundness (spec section 24.2). Record
# toolchain, seed range, and exclusions with any finding.

[doc("Miri UB check on the default toolchain's nightly")]
[group('quality')]
miri:
    CARGO_TARGET_DIR=target/nightly-assurance cargo +nightly miri test

[doc("Miri across a range of randomized seeds")]
[group('quality')]
miri-seeds seeds="16":
    CARGO_TARGET_DIR=target/nightly-assurance MIRIFLAGS="-Zmiri-many-seeds=0..{{seeds}}" cargo +nightly miri test

[doc("Compiler-oriented unused-dependency adjudication")]
[group('quality')]
udeps:
    CARGO_TARGET_DIR=target/nightly-assurance cargo +nightly udeps --all-targets --all-features

# Bounded runs only; long campaigns belong in scheduled infrastructure (spec section 23).
# WP06's canonical JSON decoder is the first production-path untrusted-input surface;
# its fuzz harness exercises the same parser and serializer used by the verifier.

[doc("Bounded fuzz run against one target")]
[group('quality')]
fuzz target seconds="60":
    rust_host="$(rustc +nightly -vV | sed -n 's/^host: //p')"; \
      runtime_corpus="target/fuzz-corpus/$rust_host/{{target}}"; \
      mkdir -p "$runtime_corpus"; \
      cp -R "fuzz/corpus/{{target}}/." "$runtime_corpus/"; \
      cargo +nightly fuzz run --target "$rust_host" --target-dir "target/fuzz/$rust_host" \
      {{target}} "$runtime_corpus" -- -max_total_time={{seconds}}

[doc("Coverage of a fuzz corpus")]
[group('quality')]
fuzz-coverage target:
    rust_host="$(rustc +nightly -vV | sed -n 's/^host: //p')"; \
      runtime_corpus="target/fuzz-corpus/$rust_host/{{target}}"; \
      mkdir -p "$runtime_corpus"; \
      cp -R "fuzz/corpus/{{target}}/." "$runtime_corpus/"; \
      cargo +nightly fuzz coverage --target "$rust_host" \
      --target-dir "target/fuzz/$rust_host" {{target}} "$runtime_corpus"

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

[doc("MUTATES: rewrite Rust formatting in place")]
[group('mutating')]
root-fmt-write:
    cargo fmt --all

[confirm("Regenerate committed Rust and Python Protobuf outputs. Continue?")]
[doc("MUTATES: regenerate committed Protobuf stubs and identity")]
[group('mutating')]
proto-gen:
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/proto/generate.py write

[confirm("Regenerate committed contract derivatives from authority sources. Continue?")]
[doc("MUTATES: regenerate contract indexes, canonical registries, and typed identities")]
[group('mutating')]
contracts-gen:
    cargo run --locked --no-default-features --features contracts-tooling --bin codefabric-contracts -- generate

[confirm("Regenerate committed Pydantic models and schema resources from Contract IR. Continue?")]
[doc("MUTATES: regenerate adapter models, schemas, and fingerprints")]
[group('mutating')]
adapter-contracts-gen:
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/contracts/generate_adapter_models.py write

[doc("MUTATES: emit fixture candidates to an isolated review directory")]
[group('mutating')]
fixture-candidates output_dir="target/fixture-candidates":
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/contracts/fixture_candidates.py --output-dir "{{output_dir}}"

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
