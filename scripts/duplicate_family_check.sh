#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
deny_config="${repository_root}/deny.toml"
fixture_manifest="${repository_root}/tooling/ci/duplicate-family-fixture/Cargo.toml"

bans_section="$(sed -n '/^\[bans\]/,/^\[sources\]/p' "$deny_config")"
if ! grep -q '^multiple-versions = "deny"$' <<<"$bans_section"; then
  echo "deny.toml must hard-deny duplicate versions" >&2
  exit 1
fi

if grep -E 'skip(-tree)? =|crate = ' <<<"$bans_section" |
  grep -Eiq 'arrow|parquet|datafusion|object_store|buoyant_kernel'; then
  echo "type-bearing dependency families must never appear in duplicate skips" >&2
  exit 1
fi

cargo deny --manifest-path "${repository_root}/Cargo.toml" \
  --config "$deny_config" check --hide-inclusion-graph bans

output="$(mktemp "${TMPDIR:-/tmp}/codefabric-duplicate-family.XXXXXX")"
trap 'rm -f "$output"' EXIT
if cargo deny --manifest-path "$fixture_manifest" --config "$deny_config" \
  check --hide-inclusion-graph bans >"$output" 2>&1; then
  echo "duplicate-family negative fixture unexpectedly passed" >&2
  exit 1
fi
if ! grep -q "duplicate entries for crate 'arrow-array'" "$output"; then
  echo "negative fixture failed for the wrong reason" >&2
  cat "$output" >&2
  exit 1
fi

echo "duplicate-family policy and negative fixture passed"
