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
import asyncio
import time
from importlib.resources import files
from importlib.util import find_spec
from pathlib import Path

from fastmcp import Client
from codefabric_cpg_mcp.contracts.wire_models import (
    StatusToolOutput,
    WireSchemaName,
    wire_schema,
    wire_schema_fingerprints,
)
from codefabric_cpg_mcp.daemon.generated import cpg_query_service_pb2 as query_pb
from codefabric_cpg_mcp.server import (
    REFERENCE_RESOURCE_TEMPLATE,
    RESULT_RESOURCE_TEMPLATE,
    create_server,
)
from codefabric_cpg_mcp.settings import Settings


class InventoryPort:
    """Lifespan-only port for installed-wheel discovery; no business double."""

    def __init__(self, settings: Settings) -> None:
        self._settings = settings
        self.connect_calls = 0
        self.close_calls = 0

    def current_settings(self) -> Settings:
        return self._settings

    async def connect(self, *, correlation_id: str = "adapter-connect") -> None:
        assert correlation_id == "adapter-connect"
        self.connect_calls += 1

    async def close(self) -> None:
        self.close_calls += 1


settings = Settings(
    format="codefabric.adapter-launch.v1",
    query_socket=Path("/tmp/codefabric-installed-wheel-inventory.sock"),
    launch_grant_hex="ab" * 32,
    adapter_program=Path("/usr/bin/python3"),
    adapter_arguments=("-m", "codefabric_cpg_mcp"),
    daemon_generation=7,
    supervisor_generation=11,
    session_expires_at_unix_ms=int(time.time() * 1000) + 120_000,
    maximum_request_state_ttl_seconds=1,
)

contracts_root = files("codefabric_cpg_mcp.contracts")
for module in (
    "fingerprints",
    "identity",
    "index",
    "model_registries",
    "query_forms",
    "schemas",
):
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
generated_root = files("codefabric_cpg_mcp.daemon.generated")
assert {entry.name for entry in generated_root.iterdir() if entry.is_file()} == {
    "__init__.py",
    "cpg_query_service_pb2.py",
    "cpg_query_service_pb2.pyi",
    "cpg_query_service_pb2_grpc.py",
}
fingerprints = dict(wire_schema_fingerprints("serialization"))
schema = wire_schema(WireSchemaName.STATUS_TOOL_OUTPUT, "serialization")
assert schema["title"] == "StatusToolOutput"
assert fingerprints[WireSchemaName.STATUS_TOOL_OUTPUT].startswith("b3:")
assert StatusToolOutput.model_fields
assert callable(create_server)
assert query_pb.DESCRIPTOR.name == "contracts/rpc/cpg_query_service.proto"
assert query_pb.DESCRIPTOR.package == "codefabric.cpgd.v2"

port = InventoryPort(settings)
server = create_server(settings, lambda _settings: port)
assert server._extensions == {}
assert server.transforms == []
assert server.providers == [server.local_provider]
assert server._support_tasks_by_default is False


async def inspect_installed_server() -> None:
    async with Client(server, mode="auto", cache=False) as client:
        tools = {tool.name: tool for tool in await client.list_tools()}
        assert set(tools) == {
            "query_code_graph",
            "validate_code_graph_query",
            "get_code_graph_status",
            "get_code_graph_reference",
        }
        templates = await client.list_resource_templates()
        assert {str(template.uri_template) for template in templates} == {
            RESULT_RESOURCE_TEMPLATE,
            REFERENCE_RESOURCE_TEMPLATE,
        }
        assert await client.list_resources() == []
        assert await client.list_prompts() == []
        discovery = client.session.discover_result
        assert discovery is not None
        assert discovery.capabilities.tasks is None
        assert discovery.capabilities.completions is not None
        assert discovery.capabilities.extensions == {"io.modelcontextprotocol/ui": {}}


asyncio.run(inspect_installed_server())
assert port.connect_calls == 1
assert port.close_calls == 1
print(fingerprints[WireSchemaName.STATUS_TOOL_OUTPUT])
PY
