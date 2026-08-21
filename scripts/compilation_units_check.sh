#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
catalog="$repository_root/contracts/manifests/suite-manifest.json"

fail() {
  printf 'compilation-unit check: %s\n' "$1" >&2
  exit 1
}

jq -e '
  .catalog_schema_version == 2
  and ([.artifacts[] | has("semantic_projection_source")] | all)
  and ([.artifacts[] | has("generated_outputs") or has("depends_on")] | any | not)
  and ([.derivations[].derivation_id] | length == (unique | length))
  and ([.derivations[].outputs[].path] | length == (unique | length))
  and ([.derivations[].outputs[].primary_artifact_ids | length > 0] | all)
  and ([.derivations[] | select(.derivation_kind == "artifact-index")] | length == 1)
  and ([.derivations[] | select(.derivation_kind == "canonical-registry-set")] | length == 1)
  and ([.derivations[] | select(.derivation_kind == "protobuf-descriptor-and-python")] | length == 1)
  and ([.derivations[] | select(.derivation_kind == "protobuf-rust-from-descriptor")] | length == 1)
  and ([.derivations[] | select(.derivation_kind == "adapter-model-compilation")] | length == 1)
  and ([.derivations[] | select(.derivation_kind == "schema-contract-compilation")] | length == 1)
' "$catalog" >/dev/null || fail 'catalog v2 structural oracle failed'

if rg -n 'generated_outputs|depends_on|output_of_kind|output_record_of_kind|GeneratedOutputKind|GeneratedOutputProducer|SOURCE_RELATIVE' \
  "$repository_root/src" \
  "$repository_root/tests" \
  "$repository_root/tooling" \
  "$repository_root/scripts" \
  "$repository_root/codefabric-cpg-mcp/src" \
  -g '*.rs' -g '*.py' -g '*.sh' -g '!compilation_units_check.sh'; then
  fail 'legacy source-owned output or global-lookup mechanics remain'
fi

if rg -n 'wave0[_-]probe|codefabric\.wave0\.v1|wave0-proto' \
  "$repository_root/src" \
  "$repository_root/tests" \
  "$repository_root/tooling/proto" \
  "$repository_root/codefabric-cpg-mcp/src" \
  "$repository_root/codefabric-cpg-mcp/tests" \
  "$repository_root/contracts/manifests/suite-manifest.json"; then
  fail 'Wave-0 probe source, binding, test, or compilation-unit residue remains'
fi

for derivation_id in \
  codefabric.derivation.adapter-models \
  codefabric.derivation.artifact-index \
  codefabric.derivation.canonical-registries \
  codefabric.derivation.schema-contracts \
  codefabric.derivation.production-proto-descriptor-python \
  codefabric.derivation.production-proto-rust; do
  cargo run --quiet --locked --manifest-path "$repository_root/Cargo.toml" \
    --no-default-features --features contracts-tooling --bin codefabric-contracts -- \
    resolve-derivation "$derivation_id" --root "$repository_root" |
    jq -e --arg id "$derivation_id" \
      '.derivation.derivation_id == $id and (.generator.toolchain | length > 0)' >/dev/null ||
    fail "resolved invocation failed for $derivation_id"
done

printf 'catalog v2 derivation units and legacy zero-state verified\n'
