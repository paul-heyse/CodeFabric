#!/usr/bin/env bash
# Validate one closed model family without touching committed outputs or authorities.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
family="${1:-}"

case "$family" in
  registry-cbef)
    test_filter='test(model_cbef) | test(model_registry) | test(model_overlay)'
    ;;
  schemas)
    test_filter='test(model_tablespec) | test(model_row_encoder) | test(model_schema) | test(model_driver_cannot_generate_compatibility_acceptance)'
    ;;
  "")
    printf 'aggregate model-family-check becomes available after WP10\n' >&2
    exit 2
    ;;
  *)
    printf 'unknown model family: %s\n' "$family" >&2
    exit 2
    ;;
esac

before_status="$(git -C "$repo_root" status --porcelain=v1 --untracked-files=all | shasum -a 256 | awk '{print $1}')"

cargo nextest run \
  --manifest-path "$repo_root/Cargo.toml" \
  --locked \
  --no-default-features \
  --features model-compiler \
  --bin codefabric-model \
  -E "$test_filter"

report="$($repo_root/scripts/model_exec.sh family-check "$family" "$repo_root")"
stage_root="$(jq -r '.stage_root' <<<"$report")"
case "$family" in
  registry-cbef)
    jq -e '
      .family == "registry-cbef"
      and .domain_count == 17
      and .enum_domain_count > 0
      and .flag_domain_count > 0
      and (.rendered_outputs | length) == 6
    ' <<<"$report" >/dev/null
    rustc --edition=2024 --crate-type lib \
      "$stage_root/src/generated/model_identity_recipes.rs" \
      -o "$stage_root/model_identity_recipes.rlib"
    rustc --edition=2024 --crate-type lib \
      "$stage_root/src/generated/model_registries.rs" \
      -o "$stage_root/model_registries.rlib"
    rustc --edition=2024 --crate-type lib \
      "$stage_root/tooling/model-transition/consumer-overlays/registry-cbef-wp32.rs" \
      -o "$stage_root/registry-cbef-wp32-overlay.rlib"
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT \
      uv run --frozen --project "$repo_root/codefabric-cpg-mcp" \
      python -m py_compile \
      "$stage_root/codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_registries.py"
    jq -e '
      .entity_field_count == 5
      and .relation_fact_field_count == 6
      and (.syntax_detail_fields | index("occurrence_family_code")) != null
      and (.syntax_detail_fields | index("reconciliation_step_code")) != null
      and (.syntax_detail_fields | index("raw_kind_disposition_code")) != null
      and (.forbidden_legacy_shapes | index("ENTITY:12-fields")) != null
      and (.forbidden_legacy_shapes | index("RELATION_FACT:8-fields")) != null
    ' "$stage_root/contracts/generated/model/registry-cbef-transition-validation.json" >/dev/null
    ;;
  schemas)
    jq -e '
      .family == "schemas"
      and .table_count >= 21
      and .operational_table_count == 24
      and .public_schema_count == 8
      and (.rendered_outputs | length) == 12
      and (.syntax_detail_fields | index("occurrence_family_code")) != null
      and (.syntax_detail_fields | index("reconciliation_step_code")) != null
      and (.syntax_detail_fields | index("raw_kind_disposition_code")) != null
    ' <<<"$report" >/dev/null
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH="$repo_root" \
      uv run --frozen --project "$repo_root/codefabric-cpg-mcp" \
      python "$repo_root/tooling/model/validate_staged_schemas.py" "$stage_root"
    rustc --edition=2024 --crate-type lib \
      "$stage_root/src/generated/model_schema_tables.rs" \
      -o "$stage_root/model_schema_tables.rlib"
    cargo run \
      --manifest-path "$repo_root/Cargo.toml" \
      --locked \
      --no-default-features \
      --features data-fabric \
      --bin codefabric-model-schema-consumer \
      -- "$stage_root"
    ;;
esac

after_status="$(git -C "$repo_root" status --porcelain=v1 --untracked-files=all | shasum -a 256 | awk '{print $1}')"
[ "$before_status" = "$after_status" ] || {
  printf 'model family check changed the repository worktree\n' >&2
  exit 1
}

printf 'model family check passed: %s\n' "$family"
