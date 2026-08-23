#!/usr/bin/env bash
# Prove read-only action planning, desired-tree parity, and source-fenced staging.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
before_status="$(git -C "$repo_root" status --porcelain=v1 --untracked-files=all | shasum -a 256 | awk '{print $1}')"

cargo nextest run \
  --manifest-path "$repo_root/Cargo.toml" \
  --locked \
  --no-default-features \
  --features model-compiler \
  --bin codefabric-model \
  -E 'test(model_action_key) | test(model_affected_closure) | test(model_desired_tree) | test(model_planned_output) | test(model_rejects_duplicate_output) | test(model_cache_entry) | test(model_plan_is_insertion) | test(model_explain_reports) | test(model_transition_consumer_patch) | test(model_cycles_project)'

plan_json="$("$repo_root/scripts/model_exec.sh" plan --root "$repo_root")"
jq -e '
  .output_count > 0
  and (.action_order | length) > 0
  and ([.changes[] | select(.kind != "unchanged")] | length) == 0
  and ([.action_keys[] | select(startswith("b3:") and length == 67)] | length)
      == (.action_keys | length)
' <<<"$plan_json" >/dev/null

changed_json="$(
  "$repo_root/scripts/model_exec.sh" plan \
    docs/upfront_design/code_property_graph_present_state_fact_ontology_specification_v1.3.md \
    --root "$repo_root"
)"
jq -e '
  ([.affected[] | select(startswith("action:"))] | length) > 0
  and ([.affected[] | select(startswith("output:"))] | length) > 0
' <<<"$changed_json" >/dev/null

check_json="$("$repo_root/scripts/model_exec.sh" check edit --root "$repo_root")"
jq -e '
  .profile == "edit"
  and .plan.output_count > 0
  and ([.plan.changes[] | select(.kind != "unchanged")] | length) == 0
' <<<"$check_json" >/dev/null

explain_json="$(
  "$repo_root/scripts/model_exec.sh" explain \
    codefabric-present-state-cpg-ontology "$repo_root"
)"
jq -e '
  (.model | length) > 0
  and (.plan | length) > 0
  and ([.plan[].consumers[]] | length) > 0
  and ([.plan[].oracles[]] | length) > 0
' <<<"$explain_json" >/dev/null

after_status="$(git -C "$repo_root" status --porcelain=v1 --untracked-files=all | shasum -a 256 | awk '{print $1}')"
[ "$before_status" = "$after_status" ] || {
  printf 'model plan/check changed the repository worktree\n' >&2
  exit 1
}

printf 'model plan check passed\n'
