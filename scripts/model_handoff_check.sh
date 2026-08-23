#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. \
  uv run --frozen --project codefabric-cpg-mcp \
  pytest -q tooling/ci/test_model_handoff.py

env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. \
  uv run --frozen --project codefabric-cpg-mcp \
  python tooling/ci/model_handoff.py --root "$repo_root"
