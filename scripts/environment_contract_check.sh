#!/usr/bin/env bash
# Prove that the repository shell boundary rejects a deliberately contaminated caller.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
# shellcheck source=../tooling/rust-tool-versions.env
source "$repo_root/tooling/rust-tool-versions.env"
probe='\
test "${CF_ROOT}" = "'"$repo_root"'" && \
test "${CODEFABRIC_REPO_SHELL}" = "1" && \
test "${UV_CACHE_DIR}" = "'"$repo_root"'/target/uv-cache" && \
test -z "${VIRTUAL_ENV+x}" && \
test -z "${UV_PROJECT_ENVIRONMENT+x}" && \
test -z "${PYTHONPATH+x}" && \
test -z "${CONDA_PREFIX+x}" && \
test -z "${CONDA_FAKE+x}" && \
test -z "${DIRENV_DIR+x}" && \
test -z "${RUSTUP_TOOLCHAIN+x}" && \
test -z "${RUSTC+x}" && \
test -z "${RUSTDOC+x}" && \
test -z "${RUSTC_WRAPPER+x}" && \
test -z "${RUSTC_WORKSPACE_WRAPPER+x}" && \
test -z "${RUSTFLAGS+x}" && \
test -z "${CARGO_ENCODED_RUSTFLAGS+x}" && \
test -z "${RUSTDOCFLAGS+x}" && \
test -z "${CARGO_ENCODED_RUSTDOCFLAGS+x}" && \
test -z "${CARGO_TARGET_DIR+x}" && \
test -z "${CARGO_BUILD_TARGET_DIR+x}" && \
test -z "${CARGO_BUILD_BUILD_DIR+x}" && \
test -z "${CARGO_BUILD_RUSTC_WRAPPER+x}" && \
test -z "${CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER+x}" && \
test -z "${CARGO_BUILD_TARGET+x}" && \
test -z "${CARGO_BUILD_JOBS+x}" && \
test -z "${CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER+x}" && \
test -z "${CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS+x}" && \
test -z "${CODEFABRIC_SCCACHE_CONF+x}" && \
test -z "${SCCACHE_DIR+x}" && \
test "${CARGO_INCREMENTAL}" = "0" && \
test "${CARGO_HOME}" = "${HOME}/.cargo" && \
test "$(command -v cargo)" = "'"$repo_root"'/scripts/cargo" && \
test "$(cargo --version)" = "$(rustup run '"$CODEFABRIC_STABLE_TOOLCHAIN"' cargo --version)" && \
test "$(cargo +nightly-2026-08-18 --version)" = "$(rustup run nightly-2026-08-18 cargo --version)" && \
test "$(cd '"$repo_root"'/rustc-extractor && cargo --version)" = "$(rustup run nightly-2026-08-18 cargo --version)"\
'

env \
  VIRTUAL_ENV=/tmp/codefabric-wrong-venv \
  UV_PROJECT_ENVIRONMENT=/tmp/codefabric-wrong-uv-project \
  PYTHONPATH=/tmp/codefabric-wrong-pythonpath \
  CONDA_PREFIX=/tmp/codefabric-wrong-conda \
  CONDA_FAKE=present \
  DIRENV_DIR=/tmp/codefabric-wrong-direnv \
  RUSTUP_TOOLCHAIN=beta \
  RUSTC=/usr/bin/false \
  RUSTDOC=/usr/bin/false \
  RUSTC_WRAPPER=/usr/bin/false \
  RUSTC_WORKSPACE_WRAPPER=/usr/bin/false \
  RUSTFLAGS=-Copt-level=0 \
  CARGO_ENCODED_RUSTFLAGS=-Copt-level=0 \
  RUSTDOCFLAGS=-Dwarnings \
  CARGO_ENCODED_RUSTDOCFLAGS=-Dwarnings \
  CARGO_HOME=/tmp/codefabric-wrong-cargo-home \
  CARGO_TARGET_DIR=/tmp/codefabric-wrong-target \
  CARGO_BUILD_TARGET_DIR=/tmp/codefabric-wrong-build-target \
  CARGO_BUILD_BUILD_DIR=/tmp/codefabric-wrong-build-dir \
  CARGO_BUILD_RUSTC_WRAPPER=/usr/bin/false \
  CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER=/usr/bin/false \
  CARGO_BUILD_TARGET=wrong-target \
  CARGO_BUILD_JOBS=999 \
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=/usr/bin/false \
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS=-Copt-level=0 \
  CODEFABRIC_SCCACHE_CONF=/tmp/codefabric-wrong-sccache-config \
  SCCACHE_DIR=/tmp/codefabric-wrong-sccache-cache \
  CARGO_INCREMENTAL=1 \
  PATH="$repo_root/codefabric-cpg-mcp/.venv/bin:/opt/homebrew/bin:$HOME/.cargo/bin:/usr/bin:/bin" \
  "$repo_root/scripts/repo-shell.sh" -cu "$probe" environment-contract-check

printf 'environment contract passed: contaminated Python, direnv, Rust, Cargo, sccache, PATH, target, and incremental overrides were isolated\n'
