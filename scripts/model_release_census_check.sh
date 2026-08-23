#!/usr/bin/env bash
# Verify the immutable owner-accepted release census and its write-set boundary.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"

cargo nextest run \
  --manifest-path "$repo_root/Cargo.toml" \
  --locked \
  --no-default-features \
  --features model-compiler \
  --bin codefabric-model \
  -E 'test(model_release_census) | test(model_acceptance_paths) | test(model_sync_cannot_write) | test(model_generated_index_deletion)'

"$repo_root/scripts/model_exec.sh" release-census-check "$repo_root"

printf 'model release census check passed\n'
