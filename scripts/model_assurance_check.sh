#!/usr/bin/env bash
# Prove live evidence collection and conservative capability profile compilation.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"

cargo nextest run \
  --manifest-path "$repo_root/Cargo.toml" \
  --locked \
  --no-default-features \
  --features model-compiler \
  --bin codefabric-model \
  -E 'test(model_assurance) | test(model_profiles) | test(model_changed_profile) | test(model_removed_or_renamed) | test(model_live_collector) | test(model_every_selected) | test(model_missing_rule)'

report="$($repo_root/scripts/model_exec.sh assurance --root "$repo_root")"
jq -e '
  .schema_version == 1
  and ([.profiles[] | .selected_recipe_count > 0] | all)
  and ([.profiles[] | .rust_test_count > 0] | all)
  and ([.profiles[] | .python_test_count > 0] | all)
  and ([.profiles[] | .rule_pair_count > 0] | all)
  and ([.profiles[] | .fixture_count > 0] | all)
  and ([.profiles[] | .requirement_count > 0] | all)
  and ([.profiles[].selected_capabilities[] | contains("mutant") or contains("WP")] | any | not)
  and (.profiles.changed.selected_capabilities == .profiles."tier-a".selected_capabilities)
' <<<"$report" >/dev/null

printf 'model assurance check passed\n'
