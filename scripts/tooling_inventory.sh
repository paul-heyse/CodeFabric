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
  echo "--- build domains ---"
  echo "stable-root: Cargo.toml"
  [ ! -f rustc-extractor/Cargo.toml ] || echo "rustc-extractor: rustc-extractor/Cargo.toml"
  [ ! -f rustc-extractor/Cargo.toml ] || (cd rustc-extractor && rustc -vV)
  [ ! -f pyrefly-sidecar/Cargo.toml ] || {
    echo "pyrefly-sidecar: pyrefly-sidecar/Cargo.toml"
    cat pyrefly-sidecar/toolchain-identity.json
  }
  [ ! -f codefabric-cpg-mcp/pyproject.toml ] || {
    echo "FastMCP adapter: codefabric-cpg-mcp/pyproject.toml"
    uv --version
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT \
      uv run --project codefabric-cpg-mcp --frozen \
      python -m codefabric_cpg_mcp --identity 2>&1 1>/dev/null
  }
  [ ! -f tooling/proto/toolchain-identity.json ] || {
    echo "Protobuf generators: tooling/proto/toolchain-identity.json"
    cat tooling/proto/toolchain-identity.json
  }
  echo
  echo "--- sccache ---"
  sccache --show-stats || true
} > target/tooling-inventory.txt
