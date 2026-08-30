#!/usr/bin/env bash
# Prove that the repository shell boundary rejects a deliberately contaminated caller.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
probe='\
test "${CF_ROOT}" = "'"$repo_root"'" && \
test "${UV_CACHE_DIR}" = "'"$repo_root"'/target/uv-cache" && \
test -z "${VIRTUAL_ENV+x}" && \
test -z "${UV_PROJECT_ENVIRONMENT+x}" && \
test -z "${PYTHONPATH+x}" && \
test -z "${CONDA_PREFIX+x}" && \
test -z "${CONDA_FAKE+x}" && \
test -z "${DIRENV_DIR+x}" && \
test -z "${RUSTUP_TOOLCHAIN+x}" && \
test -z "${RUSTC_WRAPPER+x}" && \
test -z "${CARGO_TARGET_DIR+x}" && \
test "${CARGO_INCREMENTAL}" = "0" && \
test "$(command -v cargo)" = "'"$repo_root"'/scripts/cargo" && \
test "$(cargo --version)" = "$(rustup run stable cargo --version)" && \
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
  RUSTC_WRAPPER=/usr/bin/false \
  CARGO_TARGET_DIR=/tmp/codefabric-wrong-target \
  CARGO_INCREMENTAL=1 \
  PATH="$repo_root/codefabric-cpg-mcp/.venv/bin:/opt/homebrew/bin:$HOME/.cargo/bin:/usr/bin:/bin" \
  "$repo_root/scripts/repo-shell.sh" -cu "$probe" environment-contract-check

printf 'environment contract passed: contaminated Python, direnv, Rust, PATH, target, and incremental overrides were isolated\n'
