#!/usr/bin/env bash
#
# Capture the execution-relevant tool inventory (spec section 57).
#
# Deliberately not a full environment dump -- that can contain credentials. Only
# non-secret facts that explain how a build or test result was produced.
#
# Output: target/tooling-inventory.txt (a CI artifact, category C in spec section 55).

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
mkdir -p target

{
  date -u
  uname -a || true
  echo
  rustc -vV
  cargo -V
  rustup show active-toolchain
  echo
  echo "--- installed rustup components ---"
  rustup component list --installed
  echo
  echo "--- installed cargo executables ---"
  cargo install --list
  echo
  echo "--- python toolchain ---"
  uv --version
  uv run python --version
  uv run maturin --version
  uv run ruff --version
  uv run pyrefly --version
  echo
  echo "--- sccache ---"
  sccache --show-stats || true
} > target/tooling-inventory.txt
