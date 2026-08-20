#!/usr/bin/env bash
# Keep the retired seed/PyO3/Maturin packaging authority at zero (DB01).
set -euo pipefail

cd "$(dirname "$0")/.."
temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/codefabric-db01.XXXXXX")"
trap 'rm -rf "$temporary_root"' EXIT

fail() {
  printf 'seed zero-state check failed: %s\n' "$1" >&2
  exit 1
}

for manifest in Cargo.toml rustc-extractor/Cargo.toml pyrefly-sidecar/Cargo.toml; do
  if cargo tree --locked --manifest-path "$manifest" -i pyo3 \
    >"$temporary_root/tree.out" 2>"$temporary_root/tree.err"; then
    fail "pyo3 remains in the graph for $manifest"
  fi
  rg -q 'did not match any packages' "$temporary_root/tree.err" || \
    fail "pyo3 absence probe failed unexpectedly for $manifest"
done

# docs/ contains the retired infrastructure specification and historical plans. The
# datafusion skill REFERENCE is a generic library-capability index, not live project
# authority; all executable/configuration surfaces remain in scope.
legacy_backend='ma''turin'
legacy_native_package='codefabric.''_native'
legacy_stub='_native.''pyi'
legacy_python_tree='python/''codefabric'
legacy_test_tree='python_''tests'
legacy_packaging_hits="$({
  rg -n --hidden \
    -g '!.git/**' \
    -g '!docs/**' \
    -g '!.claude/skills/**/REFERENCE.md' \
    -e "${legacy_backend}|${legacy_native_package}|${legacy_stub}|${legacy_python_tree}|${legacy_test_tree}" . || true
})"
test -z "$legacy_packaging_hits" || {
  printf '%s\n' "$legacy_packaging_hits" >&2
  fail 'retired Python-extension packaging references remain in a live surface'
}

for retired_path in pyproject.toml uv.lock .python-version python "$legacy_test_tree" scripts/wheel_test.sh; do
  test ! -e "$retired_path" || fail "retired root path still exists: $retired_path"
done

rg -q 'codefabric-cpg-mcp/(pyproject\.toml|uv\.lock|\.python-version)' .envrc || \
  fail '.envrc does not watch the adapter project'
rg -q 'CF_ADAPTER_ROOT=' scripts/bootstrap.sh || \
  fail 'bootstrap does not own the adapter environment'

if rg -n '^python\s*=|cdylib|pyo3' Cargo.toml; then
  fail 'root Cargo package still exposes the retired Python binding surface'
fi
if rg -n 'features python|_native' CLAUDE.md AGENTS.md README.md; then
  fail 'active repository docs still describe the retired binding surface'
fi

env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT \
  uv run --frozen --project codefabric-cpg-mcp python - <<'PY'
from importlib.util import find_spec

import codefabric_cpg_mcp

assert codefabric_cpg_mcp.__name__ == "codefabric_cpg_mcp"
assert find_spec("codefabric") is None
PY

printf 'seed and packaging zero-state passed\n'
