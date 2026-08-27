#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

scope="${1:-all}"
case "$scope" in
  all|python|rust) ;;
  *) printf 'usage: %s [all|python|rust]\n' "$0" >&2; exit 2 ;;
esac

env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. \
  uv run --frozen --project codefabric-cpg-mcp \
  python tooling/ci/semantic_provider_legacy_zero_state.py "$scope"

for rule in \
  tooling/ast-grep/semantic-provider-legacy/direct-provider-invocation.yml \
  tooling/ast-grep/semantic-provider-legacy/observation-schema-include-bypass.yml \
  tooling/ast-grep/semantic-provider-legacy/opaque-semantic-json-ingest.yml; do
  ast_status=0
  ast-grep scan --rule "$rule" --json=compact \
    --globs '!contracts/generated/**' \
    --globs '!src/generated/**' \
    --globs '!rustc-extractor/src/generated/**' \
    --globs '!pyrefly-sidecar/src/generated/**' >/dev/null || ast_status=$?
  if [ "$ast_status" -gt 1 ]; then
    printf 'semantic provider legacy structural probe failed: %s\n' "$rule" >&2
    exit "$ast_status"
  fi
done

just model-plan >/dev/null
case "$scope" in
  python) just root-check sidecar-check ;;
  rust) just root-check extractor-check ;;
  all) just root-check sidecar-check extractor-check ;;
esac
