"""Model, schema, fingerprint, and FastMCP equivalence tests."""

import asyncio
import json
import subprocess
from collections.abc import AsyncIterator
from dataclasses import dataclass
from pathlib import Path
from typing import Any, cast

import pytest
from fastmcp import Client, Context, FastMCP
from fastmcp.dependencies import CurrentContext, Depends
from fastmcp.server.lifespan import lifespan
from mcp.types import Tool as MCPTool
from mcp.types import ToolAnnotations
from pydantic import JsonValue, ValidationError

from codefabric_cpg_mcp.contracts.fingerprints import (
    FASTMCP_TOOL_KEYS,
    fastmcp_tool_fingerprint,
    fastmcp_tool_manifest,
    normalize_mcp_tool,
)
from codefabric_cpg_mcp.contracts.schemas import schema_fingerprints, schema_manifest
from codefabric_cpg_mcp.contracts.wire_models import (
    JSON_OBJECT_ADAPTER,
    InlineDelivery,
    QueryCounts,
    QueryToolInput,
    ResourceDelivery,
    StatusToolOutput,
    ValidateQueryOutput,
)

ROOT = Path(__file__).resolve().parents[2]
PROBE_SERVER = ROOT / "codefabric-cpg-mcp/tests/test_adapter_contracts.py:probe_mcp"


@dataclass(frozen=True, slots=True)
class ProbeRuntime:
    marker: str


@lifespan
async def probe_lifespan(_server: FastMCP[Any]) -> AsyncIterator[dict[str, Any]]:
    yield {"runtime": ProbeRuntime(marker="runtime-only")}


_CURRENT_CONTEXT = CurrentContext()


def runtime_from_context(ctx: Context = _CURRENT_CONTEXT) -> ProbeRuntime:
    state = ctx.lifespan_context
    runtime = state.get("runtime") if isinstance(state, dict) else None
    if not isinstance(runtime, ProbeRuntime):
        raise RuntimeError("probe runtime is unavailable")
    return runtime


_PROBE_RUNTIME = Depends(runtime_from_context)
READ_ONLY = ToolAnnotations(
    readOnlyHint=True,
    destructiveHint=False,
    idempotentHint=True,
    openWorldHint=False,
)
SCHEMAS = cast(dict[str, dict[str, Any]], schema_manifest()["serialization"])
probe_mcp = FastMCP(
    name="CodeFabric Contract Probe",
    version="1.0.0",
    lifespan=probe_lifespan,
    on_duplicate="error",
    strict_input_validation=True,
)


@probe_mcp.tool(
    name="contract_probe_validate",
    output_schema=SCHEMAS["ValidateQueryOutput"],
    annotations=READ_ONLY,
)
async def contract_probe_validate(
    request: dict[str, JsonValue],
    ctx: Context = _CURRENT_CONTEXT,
    runtime: ProbeRuntime = _PROBE_RUNTIME,
) -> ValidateQueryOutput:
    assert runtime.marker == "runtime-only"
    return ValidateQueryOutput(
        valid=True,
        request_id=str(ctx.request_id),
        normalized_request=request,
        dependency_graph={},
        resolved_semantics={},
        capability_requirements=(),
        resource_estimate={},
        errors=(),
        warnings=(),
    )


@probe_mcp.tool(
    name="contract_probe_status",
    output_schema=SCHEMAS["StatusToolOutput"],
    annotations=READ_ONLY,
)
async def contract_probe_status(
    runtime: ProbeRuntime = _PROBE_RUNTIME,
) -> StatusToolOutput:
    assert runtime.marker == "runtime-only"
    return StatusToolOutput(
        ready=True,
        workspace_id="workspace-probe",
        agent_instance_id="agent-probe",
        snapshot=None,
        versions={"adapter": "1.0"},
        supported_languages=("python", "rust"),
        supported_request_forms=("probe",),
        capability_statuses=(),
        freshness_state="CURRENT",
        service_limits={},
        notices=(),
    )


def test_generated_models_are_strict_closed_frozen_and_discriminated() -> None:
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
    schemas = schema_manifest()
    fingerprints = schema_fingerprints()

    for mode in ("validation", "serialization"):
        assert set(schemas[mode]) == set(fingerprints[mode])  # type: ignore[arg-type]
        for schema in schemas[mode].values():  # type: ignore[union-attr]
            assert schema["$schema"] == "https://json-schema.org/draft/2020-12/schema"  # type: ignore[index]
            assert str(schema["$id"]).endswith(f".{mode}.schema.json")  # type: ignore[index]
    assert (
        fingerprints["validation"]["StatusToolOutput"]
        != fingerprints["serialization"]["StatusToolOutput"]
    )  # type: ignore[index]


def test_fingerprint_policy_fails_unknown_protocol_fields() -> None:
    assert FASTMCP_TOOL_KEYS == (
        "name",
        "title",
        "description",
        "inputSchema",
        "outputSchema",
        "icons",
        "annotations",
        "_meta",
        "execution",
    )
    with pytest.raises(ValueError, match="unexpected MCP Tool fields"):
        normalize_mcp_tool({"name": "probe", "future": True})


def test_in_process_tools_and_client_use_generated_contracts() -> None:
    async def exercise() -> None:
        tools = [
            await probe_mcp.get_tool("contract_probe_validate"),
            await probe_mcp.get_tool("contract_probe_status"),
        ]
        assert all(tool is not None for tool in tools)
        concrete_tools = [tool for tool in tools if tool is not None]
        manifest = fastmcp_tool_manifest(concrete_tools)
        assert [tool["name"] for tool in manifest["tools"]] == [
            "contract_probe_status",
            "contract_probe_validate",
        ]
        assert "runtime" not in json.dumps(manifest)
        assert "ctx" not in json.dumps(manifest)
        assert fastmcp_tool_fingerprint(concrete_tools).startswith("b3:")

        async with Client(probe_mcp) as client:
            listed = await client.list_tools()
            assert {tool.name for tool in listed} == {
                "contract_probe_status",
                "contract_probe_validate",
            }
            status = await client.call_tool("contract_probe_status", {})
            assert status.structured_content == StatusToolOutput(
                ready=True,
                workspace_id="workspace-probe",
                agent_instance_id="agent-probe",
                snapshot=None,
                versions={"adapter": "1.0"},
                supported_languages=("python", "rust"),
                supported_request_forms=("probe",),
                capability_statuses=(),
                freshness_state="CURRENT",
                service_limits={},
                notices=(),
            ).model_dump(mode="json")
            validated = await client.call_tool(
                "contract_probe_validate", {"request": {"kind": "probe"}}
            )
            assert validated.structured_content is not None
            assert validated.structured_content["normalized_request"] == {"kind": "probe"}

    asyncio.run(exercise())


def test_cli_inspect_matches_in_process_protocol_manifest(tmp_path: Path) -> None:
    output = tmp_path / "mcp.json"
    subprocess.run(
        [
            "uv",
            "run",
            "--frozen",
            "--project",
            str(ROOT / "codefabric-cpg-mcp"),
            "fastmcp",
            "inspect",
            str(PROBE_SERVER),
            "--format",
            "mcp",
            "--output",
            str(output),
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    cli = json.loads(output.read_text(encoding="utf-8"))

    async def in_process() -> dict[str, object]:
        tools = await probe_mcp.list_tools()
        return fastmcp_tool_manifest(tools)

    expected = asyncio.run(in_process())
    cli_tools = cli["tools"] if isinstance(cli, dict) else []
    normalized_cli_tools = [
        normalize_mcp_tool(
            MCPTool.model_validate(tool).model_dump(mode="json", by_alias=True, exclude_none=True)
        )
        for tool in cli_tools
    ]
    normalized_cli_tools.sort(key=lambda tool: str(tool["name"]))
    actual = {
        "profile": expected["profile"],
        "tools": normalized_cli_tools,
    }
    assert actual == expected
