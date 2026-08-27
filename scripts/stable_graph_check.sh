#!/usr/bin/env bash
# Validate stable package boundaries from Cargo's resolved model. Feature tables are
# intentionally checked by capability, not duplicated here as a second manifest.
set -euo pipefail

cd "$(dirname "$0")/.."
# Pin and direct-feature census spans optional domains, so resolve the union here;
# capability boundaries below are checked again with per-feature Cargo trees.
metadata="$(cargo metadata --locked --format-version 1 --all-features)"
default_metadata="$(cargo metadata --locked --format-version 1)"

fail() {
  printf 'stable graph check failed: %s\n' "$1" >&2
  exit 1
}

versions_for() {
  printf '%s' "$metadata" | jq -r --arg name "$1" \
    '.packages[] | select(.name == $name) | .version' | sort -u
}

require_one_version() {
  local name="$1" expected="$2" actual
  actual="$(versions_for "$name")"
  [ "$actual" = "$expected" ] || \
    fail "$name resolved to '${actual:-nothing}', expected exactly $expected"
}

require_family_version() {
  local expression="$1" expected="$2" label="$3" actual
  actual="$(printf '%s' "$metadata" | jq -r --arg expression "$expression" \
    '.packages[] | select(.name | test($expression)) | .version' | sort -u)"
  [ "$actual" = "$expected" ] || \
    fail "$label resolved to '${actual:-nothing}', expected one $expected family"
}

require_family_version '^arrow($|-)' '59.2.0' 'Arrow'
require_one_version parquet 59.2.0
require_family_version '^datafusion($|-)' '55.0.0' 'DataFusion'
require_one_version object_store 0.13.2
require_one_version buoyant_kernel 0.25.1
require_one_version buoyant_kernel_engine 0.25.0
sqlparser_versions="$(versions_for sqlparser)"
[ "$sqlparser_versions" = $'0.61.0\n0.62.0' ] || \
  fail "sqlparser resolved to '${sqlparser_versions:-nothing}', expected the documented delta-core/DataFusion 0.61.0/0.62.0 split"
base64_versions="$(versions_for base64)"
[ "$base64_versions" = $'0.22.1\n0.23.1' ] || \
  fail "base64 resolved to '${base64_versions:-nothing}', expected the documented application/transitive 0.22.1/0.23.1 split"
for pin in \
  'gix 0.86.0' 'notify-debouncer-full 0.7.0' 'petgraph 0.8.3' \
  'rusqlite 0.40.2' 'rustix 1.1.4' 'prost 0.14.4' \
  'tonic 0.14.6' 'tonic-prost 0.14.6' \
  'serde_json 1.0.151' 'serde_json_canonicalizer 0.3.2' \
  'serde_yaml_ng 0.10.0' 'toml 1.1.4+spec-1.1.0' \
  'tempfile 3.27.0' 'thiserror 2.0.20' 'unicode-casefold 0.2.0' \
  'unicode-normalization 0.1.25' 'rayon 1.12.0' \
  'tree-sitter 0.26.12' 'tree-sitter-python 0.25.0' \
  'tree-sitter-rust 0.24.2' 'ruff_python_ast 0.0.7' \
  'ruff_python_index 0.0.7' 'ruff_python_parser 0.0.7' \
  'ruff_python_semantic 0.0.7' 'ruff_python_trivia 0.0.7' 'ruff_source_file 0.0.7' \
  'ruff_text_size 0.0.7'; do
  require_one_version ${pin}
done

delta_source='git+https://github.com/delta-io/delta-rs.git?rev=43a0cf10a313e5077c48637ad786a05359136bbb#43a0cf10a313e5077c48637ad786a05359136bbb'
delta_packages="$(printf '%s' "$metadata" | jq -r \
  '.packages[] | select(.name | test("^deltalake($|-)")) | [.name, .version, .source] | @tsv' | sort)"
printf '%s' "$metadata" | jq -e --arg source "$delta_source" '
  [.packages[] | select(.name | test("^deltalake($|-)"))] as $packages
  | ($packages | length) == 4
    and ([$packages[].name] | sort) ==
      ["deltalake", "deltalake-aws", "deltalake-core", "deltalake-derive"]
    and all($packages[]; .version == "1.0.0" and .source == $source)
' >/dev/null || \
  fail "unexpected delta-rs package family: $delta_packages"

root="$(printf '%s' "$metadata" | jq -r '.packages[] | select(.name == "codefabric") | .id')"
[ -n "$root" ] || fail 'root package metadata is absent'
root_shape="$(printf '%s' "$metadata" | jq -c --arg root "$root" '
  .packages[] | select(.id == $root)
  | {edition, rust_version,
     crate_types: ([.targets[] | select(.crate_types | index("rlib")) | .crate_types[]] | unique),
     features}
')"
printf '%s' "$root_shape" | jq -e '
  .edition == "2024"
  and .rust_version == "1.95.0"
  and .crate_types == ["rlib"]
  and (.features | has("contracts-tooling") | not)
  and .features["contract-models"] == ["canonical-json", "dep:serde_yaml_ng"]
  and (.features["model-compiler"] | sort) == ([
    "contract-models",
    "dep:blake3", "dep:gix", "dep:notify-debouncer-full", "dep:petgraph",
    "dep:rustix", "dep:serde", "dep:serde_json",
    "dep:serde_json_canonicalizer", "dep:serde_yaml_ng", "dep:tempfile",
    "dep:thiserror", "dep:toml"
  ] | sort)
  and (.features["fact-generation"] | sort) == ([
    "contract-models", "dep:blake3", "dep:rayon", "dep:ruff_python_ast",
    "dep:ruff_python_index", "dep:ruff_python_parser", "dep:ruff_python_semantic", "dep:ruff_python_trivia",
    "dep:ruff_source_file", "dep:ruff_text_size", "dep:tree-sitter",
    "dep:tree-sitter-python", "dep:tree-sitter-rust", "dep:thiserror"
  ] | sort)
  and (.features["repository-state"] | sort) == ([
    "contract-models", "dep:blake3", "dep:gix", "dep:rusqlite", "dep:rustix",
    "dep:thiserror", "dep:url"
  ] | sort)
  and .features.default == ["local-workstation"]
' >/dev/null || fail "root package boundary drifted: $root_shape"
rg -q '^resolver = "3"$' Cargo.toml || fail 'Cargo resolver 3 is not declared'

declared_features() {
  printf '%s' "$metadata" | jq -c --arg root "$root" --arg name "$1" '
    .packages[] | select(.id == $root)
    | .dependencies[] | select(.name == $name and .kind == null)
    | .features | sort
  '
}

[ "$(declared_features deltalake)" = '["datafusion","rustls"]' ] || \
  fail 'deltalake direct features drifted'
[ "$(declared_features gix)" = '["attributes","auto-chain-error","blob-diff","dirwalk","excludes","index","interrupt","parallel","revision","sha1","sha256","status","tracing"]' ] || \
  fail 'gix direct features drifted'
[ "$(declared_features petgraph)" = '["std"]' ] || fail 'petgraph direct features drifted'
[ "$(declared_features rusqlite)" = '["backup","bundled"]' ] || \
  fail 'rusqlite direct features drifted'
[ "$(declared_features rustix)" = '["fs","process"]' ] || fail 'rustix direct features drifted'
[ "$(declared_features serde_json)" = '["arbitrary_precision"]' ] || \
  fail 'serde_json arbitrary_precision is required before canonicalization'

resolved_features() {
  local package_id
  package_id="$(printf '%s' "$metadata" | jq -r --arg name "$1" \
    '.packages[] | select(.name == $name) | .id')"
  printf '%s' "$metadata" | jq -r --arg package_id "$package_id" \
    '.resolve.nodes[] | select(.id == $package_id) | .features[]' | sort -u
}

resolved_default_features() {
  local package_id
  package_id="$(printf '%s' "$default_metadata" | jq -r --arg name "$1" \
    '.packages[] | select(.name == $name) | .id')"
  printf '%s' "$default_metadata" | jq -r --arg package_id "$package_id" \
    '.resolve.nodes[] | select(.id == $package_id) | .features[]' | sort -u
}

for kernel in buoyant_kernel buoyant_kernel_engine; do
  kernel_features="$(resolved_default_features "$kernel")"
  printf '%s\n' "$kernel_features" | rg -qx 'arrow-59' || \
    fail "$kernel does not activate arrow-59"
  if printf '%s\n' "$kernel_features" | rg -qx 'arrow-58'; then
    fail "$kernel retains forbidden arrow-58 activation"
  fi
done

object_store_features="$(resolved_default_features object_store)"
for latent in aws azure gcp http; do
  printf '%s\n' "$object_store_features" | rg -qx "$latent" || \
    fail "default object_store graph no longer reports kernel-forced latent $latent support"
done

gix_features="$(resolved_features gix)"
for required in sha1 sha256 revision status attributes excludes dirwalk blob-diff interrupt parallel auto-chain-error tracing; do
  printf '%s\n' "$gix_features" | rg -qx "$required" || fail "gix feature $required is absent"
done
if printf '%s\n' "$gix_features" | rg -q \
  '^(credentials|worktree-mutation|async-network-client.*|blocking-network-client.*|merge|tree-editor|worktree-stream|worktree-archive)$'; then
  fail 'gix activated a forbidden credential, network, or mutation feature'
fi

cargo_tree() {
  cargo tree --locked --edges normal --prefix none "$@"
}

require_in_tree() {
  local tree="$1" package="$2" label="$3"
  printf '%s\n' "$tree" | rg -q "^${package} " || fail "$label omits $package"
}

forbid_in_tree() {
  local tree="$1" expression="$2" label="$3"
  if printf '%s\n' "$tree" | rg -q "^(${expression}) "; then
    fail "$label contains forbidden package family $expression"
  fi
}

default_tree="$(cargo_tree)"
for package in datafusion deltalake gix tonic rayon tree-sitter ruff_python_parser ruff_python_semantic; do
  require_in_tree "$default_tree" "$package" 'default local-workstation graph'
done
forbid_in_tree "$default_tree" 'deltalake-aws|aws-config|aws-sdk-.*|pyo3' \
  'default local-workstation graph'

featureless_tree="$(cargo_tree --no-default-features)"
forbid_in_tree "$featureless_tree" 'datafusion.*|deltalake.*|arrow.*|pyo3|tonic|rusqlite|gix' \
  'featureless graph'

canonical_tree="$(cargo_tree --no-default-features --features canonical-json)"
forbid_in_tree "$canonical_tree" 'datafusion.*|deltalake.*|arrow.*|pyo3|tonic|rusqlite|gix|prost.*' \
  'canonical-json graph'

contract_tree="$(cargo_tree --no-default-features --features contract-models)"
require_in_tree "$contract_tree" serde_yaml_ng 'contract-models graph'
forbid_in_tree "$contract_tree" 'datafusion.*|deltalake.*|arrow.*|pyo3|tonic|rusqlite|gix|prost.*|tempfile' \
  'contract-models graph'

model_tree="$(cargo_tree --no-default-features --features model-compiler)"
for package in gix notify-debouncer-full petgraph rustix serde_yaml_ng tempfile; do
  require_in_tree "$model_tree" "$package" 'model-compiler graph'
done
forbid_in_tree "$model_tree" 'datafusion.*|deltalake.*|arrow.*|pyo3|tonic|rusqlite|ruff_python_.*|tree-sitter.*|prost.*' \
  'model-compiler graph'

s3_tree="$(cargo_tree --no-default-features --features s3-storage)"
require_in_tree "$s3_tree" deltalake-aws 's3-storage graph'
require_in_tree "$s3_tree" aws-config 's3-storage graph'

target_dir="$(printf '%s' "$metadata" | jq -r '.target_directory')"
sidecar_target="$(cd pyrefly-sidecar && cargo metadata --format-version 1 --no-deps | jq -r '.target_directory')"
extractor_target="$(cd rustc-extractor && cargo +nightly metadata --format-version 1 --no-deps | jq -r '.target_directory')"
[ "$target_dir" = "$sidecar_target" ] || fail 'root and sidecar must share the repository target cache'
[ "$target_dir" != "$extractor_target" ] || fail 'nightly extractor must use its isolated target cache'

printf 'stable graph check passed: model compiler and contract models are narrow, production and S3 boundaries are explicit\n'
