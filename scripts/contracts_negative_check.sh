#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
binary="${repository_root}/target/debug/codefabric-contracts"

cargo build --locked --manifest-path "${repository_root}/Cargo.toml" \
  --no-default-features --features contracts-tooling --bin codefabric-contracts

output="$(mktemp "${TMPDIR:-/tmp}/codefabric-contracts-negative.XXXXXX")"
trap 'rm -f "$output"' EXIT

for fixture in \
  "${repository_root}/contracts/fixtures/negative/perturbed-artifact.json" \
  "${repository_root}/contracts/fixtures/negative/drifted-digest.json"; do
  if "$binary" verify-checksum-fixture "$fixture" >"$output" 2>&1; then
    echo "negative contract fixture unexpectedly passed: $fixture" >&2
    exit 1
  fi
  if ! grep -q "checksum mismatch" "$output"; then
    echo "negative contract fixture failed for the wrong reason: $fixture" >&2
    cat "$output" >&2
    exit 1
  fi
done

echo "committed contract negative fixtures failed as expected"
