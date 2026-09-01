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
from importlib.resources import files
from importlib.util import find_spec

from codefabric_cpg_mcp.contracts.wire_models import (
    StatusToolOutput,
    WireSchemaName,
    wire_schema,
    wire_schema_fingerprints,
)
from codefabric_cpg_mcp.daemon.generated import cpg_query_service_pb2 as query_pb

contracts_root = files("codefabric_cpg_mcp.contracts")
for module in ("fingerprints", "index", "model_registries", "query_forms", "schemas"):
    assert find_spec(f"codefabric_cpg_mcp.contracts.{module}") is None
for artifact in (
    "adapter-fingerprints.json",
    "adapter-package-data.json",
    "adapter-schemas.json",
    "fingerprints.py",
    "index.py",
    "model_artifact_index.json",
    "model_registries.py",
    "query-form-contract.json",
    "query_forms.py",
    "schemas.py",
):
    assert not contracts_root.joinpath(artifact).is_file()
fingerprints = dict(wire_schema_fingerprints("serialization"))
schema = wire_schema(WireSchemaName.STATUS_TOOL_OUTPUT, "serialization")
assert schema["title"] == "StatusToolOutput"
assert fingerprints[WireSchemaName.STATUS_TOOL_OUTPUT].startswith("b3:")
assert StatusToolOutput.model_fields
assert query_pb.DESCRIPTOR.name == "contracts/rpc/cpg_query_service.proto"
assert query_pb.DESCRIPTOR.package == "codefabric.cpgd.v1"
print(fingerprints[WireSchemaName.STATUS_TOOL_OUTPUT])
PY
