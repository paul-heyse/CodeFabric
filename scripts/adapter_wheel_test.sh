#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/codefabric-adapter-wheel.XXXXXX")"
trap 'rm -rf "$temporary_root"' EXIT

uv build --project "$repository_root/codefabric-cpg-mcp" \
  --wheel --out-dir "$temporary_root/dist"
wheel_path="$(find "$temporary_root/dist" -maxdepth 1 -type f -name '*.whl' -print)"
if [[ -z "$wheel_path" || "$(printf '%s\n' "$wheel_path" | wc -l | tr -d ' ')" != "1" ]]; then
  echo "expected exactly one adapter wheel" >&2
  exit 1
fi

uv venv --python 3.14 "$temporary_root/venv"
uv pip install --python "$temporary_root/venv/bin/python" "$wheel_path"
"$temporary_root/venv/bin/python" - <<'PY'
from codefabric_cpg_mcp.contracts import (
    artifact_index,
    artifact_index_bytes,
    artifact_index_digest,
    validate_checksum,
)
from codefabric_cpg_mcp.contracts.schemas import schema_fingerprints, schema_manifest
from codefabric_cpg_mcp.contracts.wire_models import StatusToolOutput

resource = artifact_index_bytes()
index = artifact_index()
assert resource.startswith(b'{"_generated":')
assert index.generated.artifact_count == len(index.artifacts)
assert index.artifacts
validate_checksum(artifact_index_digest())
assert "StatusToolOutput" in schema_manifest()["serialization"]
assert schema_fingerprints()["serialization"]["StatusToolOutput"].startswith("b3:")
assert StatusToolOutput.model_fields
print(artifact_index_digest())
PY
