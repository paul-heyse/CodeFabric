#!/usr/bin/env bash
# Prove optional caching, affected closure, resource isolation, and watch widening semantics.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"

cargo nextest run \
  --manifest-path "$repo_root/Cargo.toml" \
  --locked \
  --no-default-features \
  --features model-compiler \
  --bin codefabric-model \
  -E 'test(model_incremental) | test(model_cache) | test(model_every_family) | test(model_unknown_read) | test(model_scheduler) | test(model_watch) | test(model_property)'

diagnostics_root="$(mktemp -d "${TMPDIR:-/tmp}/codefabric-model-incremental.XXXXXX")"
cleanup() {
  find "$diagnostics_root" -type f -delete 2>/dev/null || true
  find "$diagnostics_root" -depth -type d -empty -delete 2>/dev/null || true
}
trap cleanup EXIT

for family in registry-cbef schemas adapter proto; do
  first="$diagnostics_root/$family-first.json"
  second="$diagnostics_root/$family-second.json"
  CODEFABRIC_MODEL_CACHE_MODE=read-write \
    "$repo_root/scripts/model_exec.sh" family-check "$family" "$repo_root" >"$first"
  CODEFABRIC_MODEL_CACHE_MODE=read-only \
    "$repo_root/scripts/model_exec.sh" family-check "$family" "$repo_root" >"$second"
  jq -e '.cache_lookup.status == "hit"' "$second" >/dev/null
  diff \
    <(jq -S 'del(.cache_lookup, .compiler_invocations, .stage_root)' "$first") \
    <(jq -S 'del(.cache_lookup, .compiler_invocations, .stage_root)' "$second")
done

printf 'model incremental check passed; cache timing remains diagnostic only\n'
