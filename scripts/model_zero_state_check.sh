#!/usr/bin/env bash
# Permanent reintroduction guard for the superseded catalog/compiler/proof surfaces.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

fail() {
  printf 'model zero-state check failed: %s\n' "$1" >&2
  exit 1
}

for path in \
  src/bin/codefabric-contracts.rs \
  src/contracts/artifacts.rs \
  src/contracts/compiler.rs \
  src/contracts/schema_artifacts.rs \
  src/contracts/schema_models.rs \
  codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/artifact-index.json \
  tooling/contracts/generate_adapter_models.py \
  tooling/proto/generate.py \
  tooling/ci/proof-coverage.json \
  tooling/ci/proof_coverage.py \
  scripts/adapter_contract_governance_check.sh \
  scripts/compilation_units_check.sh \
  scripts/contracts_negative_check.sh \
  scripts/contracts_repro_check.sh \
  scripts/proto_dependency_check.sh; do
  [ ! -e "$path" ] || fail "superseded path remains: $path"
done

recipe_names="$(just --dump --dump-format json | jq -r '.recipes | keys[]')"
for recipe in \
  contracts-tooling-lint schema-check adapter-contracts-governance \
  adapter-contracts-check adapter-contracts-repro-check proof-coverage-check \
  compilation-units-check proto-check proto-repro-check contracts-verify \
  contracts-verify-released contracts-repro-check proto-gen contracts-gen \
  adapter-contracts-gen; do
  if printf '%s\n' "$recipe_names" | rg -qx "$recipe"; then
    fail "superseded recipe remains: $recipe"
  fi
done
if printf '%s\n' "$recipe_names" | rg -q '^mutants-wp'; then
  fail 'packet-specific mutation recipe remains'
fi

profile_json="$(just --dump --dump-format json)"
for profile in _model-profile-edit _model-profile-changed _model-profile-tier-a _model-profile-release; do
  if jq -e --arg profile "$profile" '
    .recipes[$profile].dependencies
    | any(.recipe == "mutants-file" or (.recipe | startswith("mutants-wp")))
  ' <<<"$profile_json" >/dev/null; then
    fail "$profile directly selects mutation checking"
  fi
done

scan_roots=(
  Cargo.toml justfile README.md AGENTS.md .github scripts src tooling/model tooling/proto tooling/ci
  codefabric-cpg-mcp/src codefabric-cpg-mcp/pyproject.toml docs/authoritative_design docs/spec_index
)
if rg -n \
  'contracts-tooling|target/debug/codefabric-contracts|tooling/(contracts/generate_adapter_models|proto/generate)\.py|artifact-index\.json|PUBLIC_SCHEMA_ARTIFACTS|sync_(toolchain_identity|requirements|traceability|bundle_members)|embed_semantic_digests' \
  "${scan_roots[@]}" \
  -g '!scripts/model_zero_state_check.sh' \
  -g '!scripts/stable_graph_check.sh' \
  -g '!tooling/model/test_*.py' \
  -g '!tooling/ci/test_*.py' \
  -g '!**/__pycache__/**' >/dev/null; then
  rg -n \
    'contracts-tooling|target/debug/codefabric-contracts|tooling/(contracts/generate_adapter_models|proto/generate)\.py|artifact-index\.json|PUBLIC_SCHEMA_ARTIFACTS|sync_(toolchain_identity|requirements|traceability|bundle_members)|embed_semantic_digests' \
    "${scan_roots[@]}" \
    -g '!scripts/model_zero_state_check.sh' \
    -g '!scripts/stable_graph_check.sh' \
    -g '!tooling/model/test_*.py' \
    -g '!tooling/ci/test_*.py' \
    -g '!**/__pycache__/**' >&2
  fail 'superseded control-plane text remains in a live surface'
fi

ast-grep test --skip-snapshot-tests >/dev/null
ast-grep scan \
  --globs '!contracts/generated/**' \
  --globs '!src/generated/**' \
  --globs '!codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated/**' \
  --globs '!rustc-extractor/src/generated/**' \
  --globs '!pyrefly-sidecar/src/generated/**' >/dev/null

printf 'model zero-state check passed\n'
