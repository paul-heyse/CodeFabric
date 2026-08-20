#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
binary="${repository_root}/target/debug/codefabric-contracts"
temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/codefabric-contracts-repro.XXXXXX")"
trap 'rm -rf "$temporary_root"' EXIT

cargo build --locked --manifest-path "${repository_root}/Cargo.toml" \
  --no-default-features --features contracts-tooling --bin codefabric-contracts

for isolated in "$temporary_root/first" "$temporary_root/second"; do
  mkdir -p "$isolated"
  cp -R "${repository_root}/contracts" "$isolated/contracts"
  rm -rf "$isolated/contracts/generated"
  "$binary" generate --root "$isolated"
done

diff -ru "$temporary_root/first/contracts/generated" "$temporary_root/second/contracts/generated"
cmp "$temporary_root/first/src/generated/contracts.rs" \
  "$temporary_root/second/src/generated/contracts.rs"
cmp \
  "$temporary_root/first/codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/generated/_contract_index.py" \
  "$temporary_root/second/codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/generated/_contract_index.py"
cmp \
  "$temporary_root/first/codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/generated/_contract_index.pyi" \
  "$temporary_root/second/codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/generated/_contract_index.pyi"

echo "two isolated contract generations are byte-identical"
