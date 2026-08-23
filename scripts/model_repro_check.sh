#!/usr/bin/env bash
# Rebuild the complete model DesiredTree twice and compare its exact path/byte identity.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
before_status="$(git -C "$repo_root" status --porcelain=v1 --untracked-files=all | shasum -a 256 | awk '{print $1}')"

first="$($repo_root/scripts/model_exec.sh family-check aggregate "$repo_root")"
first_digest="$(jq -er '.tree_digest' <<<"$first")"
first_paths="$(jq -cS '.rendered_outputs' <<<"$first")"

second="$($repo_root/scripts/model_exec.sh family-check aggregate "$repo_root")"
second_digest="$(jq -er '.tree_digest' <<<"$second")"
second_paths="$(jq -cS '.rendered_outputs' <<<"$second")"

[ "$first_digest" = "$second_digest" ] || {
  printf 'aggregate model tree bytes are not reproducible: %s != %s\n' \
    "$first_digest" "$second_digest" >&2
  exit 1
}
[ "$first_paths" = "$second_paths" ] || {
  printf 'aggregate model tree path census is not reproducible\n' >&2
  exit 1
}

stage_root="$(jq -er '.stage_root' <<<"$second")"
env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH="$repo_root" \
  uv run --frozen --project "$repo_root/codefabric-cpg-mcp" \
  python "$repo_root/tooling/model/validate_aggregate.py" "$stage_root"

after_status="$(git -C "$repo_root" status --porcelain=v1 --untracked-files=all | shasum -a 256 | awk '{print $1}')"
[ "$before_status" = "$after_status" ] || {
  printf 'model reproduction changed the repository worktree\n' >&2
  exit 1
}

printf 'model reproduction check passed: %s\n' "$second_digest"
