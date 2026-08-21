#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
binary="${repository_root}/target/debug/codefabric-contracts"
temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/codefabric-contracts-repro.XXXXXX")"
trap 'rm -rf "$temporary_root"' EXIT

cargo build --locked --manifest-path "${repository_root}/Cargo.toml" \
  --no-default-features --features contracts-tooling --bin codefabric-contracts

for isolated in "$temporary_root/first" "$temporary_root/second" "$temporary_root/reordered"; do
  mkdir -p "$isolated"
  while IFS= read -r authority_path; do
    mkdir -p "$(dirname "$isolated/$authority_path")"
    cp "$repository_root/$authority_path" "$isolated/$authority_path"
  done < <(
    jq -r '.artifacts[].authority_path' \
      "$repository_root/contracts/manifests/suite-manifest.json"
  )
  while IFS= read -r compiler_input; do
    mkdir -p "$(dirname "$isolated/$compiler_input")"
    cp "$repository_root/$compiler_input" "$isolated/$compiler_input"
  done < <(
    jq -r '[.artifacts[].semantic_projection_source |
      select(.source_kind == "derivation-output") | .output.path] | unique[]' \
      "$repository_root/contracts/manifests/suite-manifest.json"
  )
  while IFS= read -r fixture_path; do
    mkdir -p "$(dirname "$isolated/$fixture_path")"
    cp "$repository_root/$fixture_path" "$isolated/$fixture_path"
  done < <(
    jq -r '.records[].path' \
      "$repository_root/contracts/manifests/fixture-oracles.json"
  )
  # The closed toolchain identity binds these repository/package lock and isolated-domain
  # identity files. Keep the reproduction root self-contained without copying build output.
  for toolchain_input in \
    Cargo.lock \
    codefabric-cpg-mcp/uv.lock \
    rustc-extractor/toolchain-identity.json \
    pyrefly-sidecar/toolchain-identity.json; do
    mkdir -p "$(dirname "$isolated/$toolchain_input")"
    cp "$repository_root/$toolchain_input" "$isolated/$toolchain_input"
  done
  if [[ "$isolated" == "$temporary_root/reordered" ]]; then
    reordered_catalog="$(mktemp "$temporary_root/catalog-reordered.XXXXXX")"
    jq '.artifacts |= reverse | .derivations |= reverse | .resource_budget_profiles |= reverse' \
      "$isolated/contracts/manifests/suite-manifest.json" > "$reordered_catalog"
    mv "$reordered_catalog" "$isolated/contracts/manifests/suite-manifest.json"
  fi
  "$binary" generate --root "$isolated"
done

while IFS= read -r output_path; do
  cmp "$temporary_root/first/$output_path" "$temporary_root/second/$output_path"
done < <(
  jq -r '.derivations[] |
    select(.derivation_kind == "artifact-index" or .derivation_kind == "canonical-registry-set" or .derivation_kind == "schema-contract-compilation") |
    .outputs[].path' \
    "$repository_root/contracts/manifests/suite-manifest.json"
)

while IFS= read -r output_path; do
  cmp "$temporary_root/first/$output_path" "$temporary_root/reordered/$output_path"
done < <(
  jq -r '.derivations[] |
    select(.derivation_kind == "canonical-registry-set" or .derivation_kind == "schema-contract-compilation") |
    .outputs[].path' \
    "$repository_root/contracts/manifests/suite-manifest.json"
)

index_path="$(jq -r '.derivations[].outputs[] |
    select(.output_kind == "artifact-index") | .path' \
    "$repository_root/contracts/manifests/suite-manifest.json")"
for isolated in first reordered; do
  jq -cS '(.artifacts[] |
    select(.artifact_id == "codefabric.manifests.suite-manifest") |
    .source_digest) = "source-only-change"' \
    "$temporary_root/$isolated/$index_path" > "$temporary_root/$isolated-index-normalized.json"
done
cmp "$temporary_root/first-index-normalized.json" \
  "$temporary_root/reordered-index-normalized.json"

echo "two isolated generations are byte-identical; catalog reorder changes only its source identity"
