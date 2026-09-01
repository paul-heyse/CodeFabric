"""Released model, derived-schema, handshake, and FastMCP equivalence tests."""

import asyncio
import json
import os
import subprocess
from pathlib import Path

import pytest
from fastmcp import Client
from mcp.types import Tool as MCPTool
from pydantic import ValidationError

from codefabric_cpg_mcp.contracts.wire_models import (
    JSON_OBJECT_ADAPTER,
    JSON_SCHEMA_DIALECT,
    InlineDelivery,
    QueryCounts,
    QueryToolInput,
    ResourceDelivery,
    StatusToolOutput,
    WireSchemaName,
    wire_schema,
    wire_schema_fingerprints,
)
from codefabric_cpg_mcp.server import mcp
from codefabric_cpg_mcp.settings import process_settings

ROOT = Path(__file__).resolve().parents[2]
PRODUCTION_SERVER = ROOT / "codefabric-cpg-mcp/tests/production_server_entry.py:mcp"


def test_released_models_are_strict_closed_frozen_and_discriminated() -> None:
    with pytest.raises(ValidationError, match="extra_forbidden"):
        QueryCounts(fact_count=1, result_count=1, truncated=False, leaked=True)  # type: ignore[call-arg]
    with pytest.raises(ValidationError, match="int_type"):
        QueryCounts(fact_count="1", result_count=1, truncated=False)  # type: ignore[arg-type]
    with pytest.raises(ValidationError, match="extra_forbidden"):
        QueryCounts.model_validate({"factCount": 1, "result_count": 1, "truncated": False})
    counts = QueryCounts(fact_count=1, result_count=1, truncated=False)
    with pytest.raises(ValidationError, match="frozen"):
        counts.fact_count = 2  # type: ignore[misc]
    with pytest.raises(ValidationError):
        QueryToolInput(request={"not_json": b"bytes"})  # type: ignore[dict-item]
    with pytest.raises(ValidationError):
        ResourceDelivery.model_validate(
            {
                "mode": "inline",
                "result_bytes": 0,
                "checksum": "b3:" + "a" * 64,
                "result_resource": {
                    "uri": "cpg://result",
                    "manifest_uri": "cpg://manifest",
                    "expires_at": "now",
                    "subresource_uris": [],
                },
            }
        )
    assert (
        InlineDelivery(
            result_bytes=2,
            checksum="b3:" + "a" * 64,
            response={"ok": True},
        ).mode
        == "inline"
    )


def test_annotation_driven_serialization_does_not_leak_subclass_fields() -> None:
    class ExtendedStatus(StatusToolOutput):
        internal_secret: str

    class Envelope(QueryCounts):
        status: StatusToolOutput

    extended = ExtendedStatus(
        ready=True,
        workspace_id="workspace-probe",
        agent_instance_id="agent-probe",
        snapshot=None,
        versions={},
        supported_languages=(),
        supported_request_forms=(),
        capability_statuses=(),
        freshness_state="CURRENT",
        service_limits={},
        notices=(),
        internal_secret="must-not-cross-the-wire",
    )
    dumped = Envelope(
        fact_count=1,
        result_count=1,
        truncated=False,
        status=extended,
    ).model_dump(mode="json")

    assert "internal_secret" not in dumped["status"]


def test_module_scoped_json_adapter_rejects_non_json_values() -> None:
    assert JSON_OBJECT_ADAPTER.validate_python({"nested": [1, True, None]}) == {
        "nested": [1, True, None]
    }
    with pytest.raises(ValidationError):
        JSON_OBJECT_ADAPTER.validate_python({"bad": object()})


def test_schema_modes_are_canonical_named_and_fingerprinted() -> None:
    for mode in ("validation", "serialization"):
        fingerprints = dict(wire_schema_fingerprints(mode))
        assert set(fingerprints) == set(WireSchemaName)
        for name in WireSchemaName:
            schema = wire_schema(name, mode)
            assert schema["$schema"] == JSON_SCHEMA_DIALECT
            assert str(schema["$id"]).endswith(f".{mode}.schema.json")
    assert (
        dict(wire_schema_fingerprints("validation"))[WireSchemaName.STATUS_TOOL_OUTPUT]
        != dict(wire_schema_fingerprints("serialization"))[WireSchemaName.STATUS_TOOL_OUTPUT]
    )


def test_cli_inspect_matches_production_protocol_manifest(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    output = tmp_path / "mcp.json"
    environment = {
        **os.environ,
        "CODEFABRIC_CPG_DAEMON_TARGET": "unix:///tmp/codefabric-inspect.sock",
        "CODEFABRIC_WORKSPACE_ID": "workspace-inspect",
        "CODEFABRIC_AGENT_INSTANCE_ID": "agent-inspect",
        "CODEFABRIC_CPG_CAPABILITY_TOKEN": "inspect-secret",
    }
    subprocess.run(
        [
            "uv",
            "run",
            "--frozen",
            "--project",
            str(ROOT / "codefabric-cpg-mcp"),
            "fastmcp",
            "inspect",
            str(PRODUCTION_SERVER),
            "--format",
            "mcp",
            "--output",
            str(output),
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        env=environment,
        text=True,
    )
    cli = json.loads(output.read_text(encoding="utf-8"))
    for name, value in environment.items():
        if name.startswith("CODEFABRIC_"):
            monkeypatch.setenv(name, value)
    process_settings.cache_clear()

    async def in_process() -> list[dict[str, object]]:
        async with Client(mcp) as client:
            return [
                tool.model_dump(mode="json", by_alias=True, exclude_none=True)
                for tool in await client.list_tools()
            ]

    expected = asyncio.run(in_process())
    cli_tools = cli["tools"] if isinstance(cli, dict) else []
    actual = [
        MCPTool.model_validate(tool).model_dump(mode="json", by_alias=True, exclude_none=True)
        for tool in cli_tools
    ]
    expected.sort(key=lambda tool: str(tool["name"]))
    actual.sort(key=lambda tool: str(tool["name"]))
    assert actual == expected
