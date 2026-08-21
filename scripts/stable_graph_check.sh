#!/usr/bin/env bash
# Validate the resolved stable-domain graph, not merely Cargo.toml text.
set -euo pipefail

cd "$(dirname "$0")/.."
metadata="$(cargo metadata --locked --format-version 1)"

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

require_family_version '^arrow($|-)' '58.4.0' 'Arrow'
require_one_version parquet 58.4.0
require_family_version '^datafusion($|-)' '54.1.0' 'DataFusion'
require_one_version object_store 0.13.2
require_one_version gix 0.86.0
require_one_version rusqlite 0.40.2
require_one_version rustix 1.1.4
require_one_version hyper-util 0.1.20
require_one_version prost 0.14.4
require_one_version tokio-stream 0.1.18
require_one_version tonic 0.14.6
require_one_version tonic-prost 0.14.6
require_one_version tower 0.5.3
require_one_version base64 0.22.1
require_one_version serde_json 1.0.151
require_one_version serde_json_canonicalizer 0.3.2
require_one_version serde_path_to_error 0.1.20
require_one_version serde_yaml_ng 0.10.0
require_one_version tempfile 3.27.0
require_one_version thiserror 2.0.20

delta_sources="$(printf '%s' "$metadata" | jq -r \
  '.packages[] | select(.name | test("^deltalake($|-)")) | .source' | sort -u)"
case "$delta_sources" in
  *'https://github.com/delta-io/delta-rs.git?rev=9f9223197469897ef05ae4369eb4fd1390174e65#9f9223197469897ef05ae4369eb4fd1390174e65'*) ;;
  *) fail "delta-rs did not resolve from the approved immutable revision" ;;
esac
[ "$(printf '%s\n' "$delta_sources" | sed '/^$/d' | wc -l | tr -d ' ')" = 1 ] || \
  fail "delta-rs resolved from more than one source"

kernel_lines="$(printf '%s' "$metadata" | jq -r '
  .packages[]
  | select(.name == "buoyant_kernel" or .name == "buoyant_kernel_engine")
  | (.version | split(".")[0:2] | join("."))
' | sort -u)"
[ "$kernel_lines" = '0.25' ] || \
  fail "Delta kernel packages are not confined to the released 0.25 line: $kernel_lines"
for kernel in buoyant_kernel buoyant_kernel_engine; do
  [ "$(versions_for "$kernel" | wc -l | tr -d ' ')" = 1 ] || \
    fail "$kernel resolved more than once"
done

root="$(printf '%s' "$metadata" | jq -r '.packages[] | select(.name == "codefabric") | .id')"
[ -n "$root" ] || fail 'root package metadata is absent'
root_shape="$(printf '%s' "$metadata" | jq -c --arg root "$root" '
  .packages[] | select(.id == $root)
  | {
      edition,
      rust_version,
      crate_types: ([.targets[] | select(.name == "codefabric") | .crate_types[]] | unique),
      features
    }
')"
printf '%s' "$root_shape" | jq -e '
  .edition == "2024"
  and .rust_version == "1.94.1"
  and .crate_types == ["rlib"]
  and .features == {
    "canonical-json": [
      "dep:base64",
      "dep:blake3",
      "dep:serde",
      "dep:serde_json",
      "dep:serde_json_canonicalizer",
      "dep:thiserror"
    ],
    "compatibility-probes": [
      "canonical-json",
      "data-fabric",
      "repository-state",
      "rpc"
    ],
    "contracts-tooling": [
      "canonical-json",
      "dep:prost",
      "dep:prost-types",
      "dep:serde_path_to_error",
      "dep:serde_yaml_ng",
      "dep:tempfile"
    ],
    "data-fabric": [
      "dep:arrow",
      "dep:arrow-array",
      "dep:arrow-buffer",
      "dep:arrow-cast",
      "dep:arrow-ord",
      "dep:arrow-row",
      "dep:arrow-schema",
      "dep:arrow-select",
      "dep:arrow-string",
      "dep:datafusion",
      "dep:deltalake",
      "dep:futures",
      "dep:object_store",
      "dep:parquet",
      "dep:tracing"
    ],
    "default": ["local-workstation"],
    "local-workstation": ["contracts-tooling", "compatibility-probes"],
    "proto-tooling": ["dep:prost", "dep:prost-types", "dep:tonic-prost-build"],
    "repository-state": ["dep:gix", "dep:rusqlite", "dep:rustix", "dep:url"],
    "rpc": ["dep:prost", "dep:tokio", "dep:tonic", "dep:tonic-prost"],
    "s3-storage": ["data-fabric", "deltalake/s3"]
  }
' >/dev/null || fail "root package shape or feature table drifted: $root_shape"
rg -q '^resolver = "3"$' Cargo.toml || fail 'Cargo resolver 3 is not declared'

declared_features() {
  printf '%s' "$metadata" | jq -c --arg root "$root" --arg name "$1" '
    .packages[] | select(.id == $root)
    | .dependencies[] | select(.name == $name)
    | .features | sort
  '
}

[ "$(declared_features deltalake)" = '["datafusion","rustls"]' ] || \
  fail 'deltalake direct features drifted'
[ "$(declared_features gix)" = '["attributes","auto-chain-error","blob-diff","dirwalk","excludes","index","interrupt","parallel","sha1","sha256","status","tracing"]' ] || \
  fail 'gix direct features drifted'
[ "$(declared_features rusqlite)" = '["backup","bundled"]' ] || \
  fail 'rusqlite direct features drifted'
[ "$(declared_features rustix)" = '["fs"]' ] || fail 'rustix direct features drifted'
[ "$(declared_features serde_json)" = '["arbitrary_precision"]' ] || \
  fail 'serde_json arbitrary_precision is required for pre-canonicalization range checks'

resolved_features() {
  local package_id
  package_id="$(printf '%s' "$metadata" | jq -r --arg name "$1" \
    '.packages[] | select(.name == $name) | .id')"
  printf '%s' "$metadata" | jq -r --arg package_id "$package_id" \
    '.resolve.nodes[] | select(.id == $package_id) | .features[]' | sort -u
}

gix_features="$(resolved_features gix)"
for required in sha1 sha256 status attributes excludes dirwalk blob-diff interrupt parallel auto-chain-error tracing; do
  printf '%s\n' "$gix_features" | rg -qx "$required" || fail "gix feature $required is absent"
done
if printf '%s\n' "$gix_features" | rg -q \
  '^(credentials|worktree-mutation|async-network-client.*|blocking-network-client.*|merge|tree-editor|worktree-stream|worktree-archive)$'; then
  fail 'gix activated a forbidden credential, network, or mutation feature'
fi

rusqlite_features="$(resolved_features rusqlite)"
for required in backup bundled; do
  printf '%s\n' "$rusqlite_features" | rg -qx "$required" || \
    fail "rusqlite feature $required is absent"
done
if printf '%s\n' "$rusqlite_features" | rg -q '^(cache|ffi-sqlite-wasm-rs)$'; then
  fail 'rusqlite default-only features leaked into the selected profile'
fi
printf '%s\n' "$(resolved_features rustix)" | rg -qx fs || fail 'rustix fs feature is absent'

# The pinned buoyant kernel's arrow-58 feature intentionally forces these latent
# object_store capabilities. Local authority is enforced by provider configuration and
# by excluding the Delta cloud implementation packages below; never misreport this
# compiled surface as absent.
object_store_features="$(resolved_features object_store)"
for required in aws azure gcp http cloud quick-xml; do
  printf '%s\n' "$object_store_features" | rg -qx "$required" || \
    fail "expected kernel-forced object_store feature $required is absent"
done

default_tree="$(cargo tree --locked --edges normal --prefix none)"
for required in datafusion deltalake gix tonic serde_json_canonicalizer; do
  printf '%s\n' "$default_tree" | rg -q "^${required} " || \
    fail "default local-workstation graph omits required package $required"
done
if printf '%s\n' "$default_tree" | rg -q \
  '^(aws-config|aws-credential-types|aws-runtime|aws-sdk-|aws-sigv4|aws-smithy-|aws-types|deltalake-aws)'; then
  fail 'default local-workstation graph activates the Delta S3 implementation or AWS SDK'
fi
legacy_python_bridge='py''o3'
printf '%s\n' "$default_tree" | rg -q "^${legacy_python_bridge} " && fail 'legacy Python bridge remains in the stable graph'

s3_tree="$(cargo tree --locked --edges normal --no-default-features --features s3-storage --prefix none)"
printf '%s\n' "$s3_tree" | rg -q '^deltalake-aws ' || \
  fail 's3-storage does not activate the delta-rs S3 implementation'
printf '%s\n' "$s3_tree" | rg -q '^(aws-config|aws-sdk-s3) ' || \
  fail 's3-storage does not activate the AWS SDK path'

assert_graph_omits() {
  local label="$1" tree="$2" forbidden
  forbidden='^(arrow($|-)|parquet |datafusion($|-)|deltalake($|-)|object_store |gix |rusqlite |tonic |tonic-prost )'
  if printf '%s\n' "$tree" | rg -q "$forbidden"; then
    fail "$label graph contains an unrelated heavyweight production family"
  fi
}

featureless_tree="$(cargo tree --locked --edges normal --no-default-features --prefix none)"
canonical_tree="$(cargo tree --locked --edges normal --no-default-features --features canonical-json --prefix none)"
contracts_tree="$(cargo tree --locked --edges normal --no-default-features --features contracts-tooling --prefix none)"
proto_tree="$(cargo tree --locked --edges normal --no-default-features --features proto-tooling --prefix none)"
assert_graph_omits featureless "$featureless_tree"
assert_graph_omits canonical-json "$canonical_tree"
assert_graph_omits contracts-tooling "$contracts_tree"
assert_graph_omits proto-tooling "$proto_tree"
printf '%s\n' "$canonical_tree" | rg -q '^serde_json_canonicalizer ' || \
  fail 'canonical-json does not activate the approved JCS serializer'
printf '%s\n' "$contracts_tree" | rg -q '^serde_yaml_ng ' || \
  fail 'contracts-tooling does not activate YAML contract parsing'
printf '%s\n' "$contracts_tree" | rg -q '^serde_path_to_error ' || \
  fail 'contracts-tooling does not activate path-aware typed diagnostics'
printf '%s\n' "$contracts_tree" | rg -q '^prost-types ' || \
  fail 'contracts-tooling does not activate typed descriptor-set projection'
printf '%s\n' "$proto_tree" | rg -q '^tonic-prost-build ' || \
  fail 'proto-tooling does not activate the Rust Protobuf generator'

root_target="$(printf '%s' "$metadata" | jq -r '.target_directory')"
sidecar_target="$(cargo metadata --locked --format-version 1 \
  --manifest-path pyrefly-sidecar/Cargo.toml | jq -r '.target_directory')"
extractor_target="$(cd rustc-extractor && cargo metadata --locked --format-version 1 | jq -r '.target_directory')"
[ "$root_target" = "$sidecar_target" ] || \
  fail "stable root and sidecar target directories differ: $root_target != $sidecar_target"
[ "$extractor_target" != "$root_target" ] || \
  fail 'dated-nightly extractor shares the stable target directory'

legacy_backend='ma''turin'
bootstrap_surface="$(./scripts/bootstrap.sh --context 2>&1)"
if printf '%s\n' "$bootstrap_surface" | rg -qi "${legacy_backend}"; then
  fail 'the operational command surface still advertises a removed root packaging command'
fi
just_dump="$(just --dump --dump-format json)"
if printf '%s' "$just_dump" | jq -e '
  .recipes | has("wheel") or has("python-develop") or has("test-python")
' >/dev/null; then
  fail 'the operational command surface still advertises a removed root packaging command'
fi

printf 'stable graph check passed\n'
