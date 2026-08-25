#!/usr/bin/env bash
# Validate one closed model family without touching committed outputs or authorities.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
family="${1:-}"

[ -n "$family" ] || family=aggregate

case "$family" in
  aggregate)
    test_filter='test(model_detached_identity) | test(model_routine_tree) | test(model_rejects_missing_duplicate) | test(model_bundle_projection) | test(model_generated_aggregates) | test(model_promoted_consumers) | test(model_released_traceability) | test(model_driver_failure)'
    ;;
  adapter)
    test_filter='test(model_adapter)'
    ;;
  registry-cbef)
    test_filter='test(model_cbef) | test(model_registry) | test(model_provider)'
    ;;
  proto)
    test_filter='test(model_proto)'
    ;;
  schemas)
    test_filter='test(model_tablespec) | test(model_row_encoder) | test(model_schema) | test(model_driver_cannot_generate_compatibility_acceptance)'
    ;;
  *)
    printf 'unknown model family: %s\n' "$family" >&2
    exit 2
    ;;
esac

if [ "$family" = aggregate ]; then
  for child_family in registry-cbef schemas adapter proto; do
    "$repo_root/scripts/model_family_check.sh" "$child_family"
  done
fi

if [ "$family" = adapter ] || [ "$family" = proto ]; then
  env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT \
    uv sync --frozen --project "$repo_root/codefabric-cpg-mcp"
fi

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
  adapter)
    jq -e '
      .family == "adapter"
      and .validation.model_count > 0
      and .validation.union_count > 0
      and .validation.projection_count == (.rendered_outputs | length)
      and .validation.validation_schema_count == (.validation.model_count + .validation.union_count + 1)
      and .validation.serialization_schema_count == .validation.validation_schema_count
      and .tool_identity.python_version == "3.14.7"
      and .tool_identity.pydantic_version == "2.13.4"
      and .tool_identity.fastmcp_version == "3.4.7"
      and (.tool_identity.python_digest | startswith("b3:"))
      and (.tool_identity.ruff_digest | startswith("b3:"))
    ' <<<"$report" >/dev/null
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH="$repo_root:$repo_root/codefabric-cpg-mcp/src" \
      uv run --frozen --project "$repo_root/codefabric-cpg-mcp" \
      pytest "$repo_root/tooling/model/test_adapter_driver.py"
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH="$repo_root:$repo_root/codefabric-cpg-mcp/src" \
      uv run --frozen --project "$repo_root/codefabric-cpg-mcp" \
      python "$repo_root/tooling/model/validate_staged_adapter.py" "$stage_root"
    ;;
  aggregate)
    jq -e '
      .family == "aggregate"
      and .artifact_count >= .released_artifact_count
      and .released_artifact_count > 0
      and .family_output_count > 0
      and .governance_output_count > 0
      and .output_count == (.rendered_outputs | length)
      and .requirement_count == 84
      and .bundle_count == 8
      and .fixture_count > 0
      and (.tree_digest | startswith("b3:"))
    ' <<<"$report" >/dev/null
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH="$repo_root" \
      uv run --frozen --project "$repo_root/codefabric-cpg-mcp" \
      python "$repo_root/tooling/model/validate_aggregate.py" "$stage_root"
    ;;
  proto)
    jq -e '
      .family == "proto"
      and .source_count > 0
      and .descriptor_file_count == .source_count
      and .package_count == .source_count
      and .compiler_invocations == (if .cache_lookup.status == "hit" then 0 else 1 end)
      and (.rendered_outputs | length) == (3 + (4 * .source_count))
      and .tool_identity.schema == 4
      and .tool_identity.python."grpcio-tools" == "1.83.0"
      and .tool_identity.python.protobuf == "7.36.0"
      and .tool_identity.rust.descriptor_api == "tonic_prost_build::Builder::compile_fds"
      and (.tool_identity.rust.action_key | startswith("b3:"))
      and (.tool_identity.rust.binary_digest | startswith("b3:"))
    ' <<<"$report" >/dev/null
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH="$repo_root" \
      uv run --frozen --project "$repo_root/codefabric-cpg-mcp" \
      pytest "$repo_root/tooling/model/test_proto_driver.py"
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH="$repo_root" \
      uv run --frozen --project "$repo_root/codefabric-cpg-mcp" \
      python "$repo_root/tooling/model/validate_staged_proto.py" "$stage_root"
    ;;
  registry-cbef)
    jq -e '
      .family == "registry-cbef"
      and .domain_count == 17
      and .enum_domain_count > 0
      and .flag_domain_count > 0
      and (.rendered_outputs | length) == 10
      and (.tool_identity.action_key | startswith("b3:"))
      and (.tool_identity.executable_digest | startswith("b3:"))
      and .tool_identity.features == ["provider-inventory-tooling"]
    ' <<<"$report" >/dev/null
    for catalog in "$stage_root"/contracts/generated/provider-raw-kinds/*.json; do
      jq -e '
        (.catalog_id | length) > 0
        and (.provider_id | length) > 0
        and (.provider_version | length) > 0
        and (.runtime_inventory_fingerprint | startswith("b3:"))
        and .generation_unit_id == "driver:registry-cbef-v1/provider-raw-v1"
        and (.input_identities | length) == 3
      ' "$catalog" >/dev/null
    done
    jq -e '
      (.runtime_inventory.raw_kinds | length) > 0
      and (.runtime_inventory.fields | length) > 0
      and (.node_types | type) == "array"
    ' "$stage_root/contracts/generated/provider-raw-kinds/tree-sitter-python-0-25-0.json" >/dev/null
    jq -e '
      (.runtime_inventory.node_kinds | length) > 0
      and (.runtime_inventory.token_kinds | length) > 0
    ' "$stage_root/contracts/generated/provider-raw-kinds/ruff-python-0-0-7.json" >/dev/null
    grep -Fq 'ruff_python_ast::NodeKind::' "$stage_root/src/generated/provider_raw_kinds.rs"
    grep -Fq 'ruff_python_ast::token::TokenKind::' "$stage_root/src/generated/provider_raw_kinds.rs"
    rustc --edition=2024 --crate-type lib \
      "$stage_root/src/generated/model_identity_recipes.rs" \
      -o "$stage_root/model_identity_recipes.rlib"
    rustc --edition=2024 --crate-type lib \
      "$stage_root/src/generated/registries.rs" \
      -o "$stage_root/runtime_registries.rlib"
    env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT \
      uv run --frozen --project "$repo_root/codefabric-cpg-mcp" \
      python -m py_compile \
      "$stage_root/codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_registries.py"
    ;;
  schemas)
    jq -e '
      .family == "schemas"
      and .table_count >= 21
      and .operational_table_count == 26
      and .public_schema_count == 8
      and (.rendered_outputs | length) > .public_schema_count
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

printf 'model family check passed: %s\n' "$family"
