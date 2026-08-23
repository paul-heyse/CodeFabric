"""Independent behavioral proofs for the external Pydantic/FastMCP driver."""

from __future__ import annotations

import asyncio
import importlib
import json
import shutil
import sys
from dataclasses import dataclass
from pathlib import Path

import pytest
from codefabric_cpg_mcp.contracts.json import checksum
from fastmcp import Client, FastMCP
from mcp.types import Tool as MCPTool
from pydantic import ValidationError

from tooling.model.adapter_contract_ir import _load_candidate
from tooling.model.adapter_driver import (
    PROTOCOL_VERSION,
    AdapterDriverIr,
    AdapterProjection,
    DriverRequest,
    RenderResponse,
    execute,
)

ROOT = Path(__file__).resolve().parents[2]
IR_PATH = ROOT / "contracts/adapter/adapter-model-ir.json"


def _request(source: str | None = None) -> DriverRequest:
    source = IR_PATH.read_text(encoding="utf-8") if source is None else source
    plan = execute(
        DriverRequest(
            protocol_version=PROTOCOL_VERSION,
            operation="plan",
            source=source,
            source_digest=checksum(source.encode()),
        )
    )
    return DriverRequest(
        protocol_version=PROTOCOL_VERSION,
        operation="render",
        source=source,
        source_digest=checksum(source.encode()),
        planned_outputs=plan.outputs,
    )


def _rendered() -> RenderResponse:
    response = execute(_request())
    assert isinstance(response, RenderResponse)
    return response


def _output(response: RenderResponse, suffix: str) -> str:
    return next(
        output.contents for output in response.outputs if output.path.endswith(suffix)
    )


def test_model_adapter_round_trips_all_contract_ir_shapes_strictly() -> None:
    response = _rendered()
    module = _load_candidate(_output(response, "/wire_models.py").encode())

    query = module.QueryToolInput(request={"kind": "search"}, delivery="inline")
    assert (
        module.MODEL_ADAPTERS["QueryToolInput"].validate_json(
            module.MODEL_ADAPTERS["QueryToolInput"].dump_json(query)
        )
        == query
    )
    delivery = module.TYPE_ADAPTERS["Delivery"].validate_python(
        {
            "mode": "inline",
            "result_bytes": 2,
            "checksum": "b3:" + "a" * 64,
            "response": {"ok": True},
        }
    )
    assert (
        module.TYPE_ADAPTERS["Delivery"].dump_python(delivery, mode="json")["mode"]
        == "inline"
    )
    assert (
        module.MODEL_ADAPTERS["QueryToolInput"]
        is module.MODEL_ADAPTERS["QueryToolInput"]
    )


def test_model_validation_and_serialization_schema_modes_are_distinct_and_stable() -> (
    None
):
    first = _rendered()
    second = _rendered()
    assert [(item.path, item.contents) for item in first.outputs] == [
        (item.path, item.contents) for item in second.outputs
    ]
    schemas = json.loads(_output(first, "/adapter-schemas.json"))
    fingerprints = json.loads(_output(first, "/adapter-fingerprints.json"))
    assert set(schemas["validation"]) == set(schemas["serialization"])
    assert set(schemas["validation"]) == set(fingerprints["validation"])
    assert (
        fingerprints["validation"]["StatusToolOutput"]
        != fingerprints["serialization"]["StatusToolOutput"]
    )


@dataclass(frozen=True, slots=True)
class _ToolView:
    value: MCPTool
    runtime_marker: str = "not-client-visible"

    def to_mcp_tool(self) -> MCPTool:
        return self.value


def _load_fingerprint_module(tmp_path: Path, source: str):
    package = tmp_path / "codefabric_cpg_mcp/contracts"
    package.mkdir(parents=True)
    (tmp_path / "codefabric_cpg_mcp/__init__.py").write_text("", encoding="utf-8")
    (package / "__init__.py").write_text("", encoding="utf-8")
    shutil.copy2(
        ROOT / "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/json.py",
        package / "json.py",
    )
    (package / "fingerprints.py").write_text(source, encoding="utf-8")
    sys.path.insert(0, str(tmp_path))
    try:
        return importlib.import_module("codefabric_cpg_mcp.contracts.fingerprints")
    finally:
        sys.path.remove(str(tmp_path))
        for name in tuple(sys.modules):
            if name == "codefabric_cpg_mcp" or name.startswith("codefabric_cpg_mcp."):
                sys.modules.pop(name, None)


def test_model_fastmcp_fingerprint_changes_only_for_client_visible_contract_changes(
    tmp_path: Path,
) -> None:
    response = _rendered()
    module = _load_fingerprint_module(tmp_path, _output(response, "/fingerprints.py"))
    base = _ToolView(
        MCPTool(name="probe", description="v1", inputSchema={"type": "object"})
    )
    same_wire = _ToolView(base.value, runtime_marker="different-runtime")
    changed = _ToolView(
        MCPTool(name="probe", description="v2", inputSchema={"type": "object"})
    )
    assert module.fastmcp_tool_fingerprint([base]) == module.fastmcp_tool_fingerprint(
        [same_wire]
    )
    assert module.fastmcp_tool_fingerprint([base]) != module.fastmcp_tool_fingerprint(
        [changed]
    )


def test_model_adapter_generated_and_public_handlers_are_equivalent() -> None:
    response = _rendered()
    module = _load_candidate(_output(response, "/wire_models.py").encode())
    server = FastMCP(name="model-adapter-equivalence", strict_input_validation=True)
    expected = module.StatusToolOutput(
        ready=True,
        workspace_id="workspace-model",
        agent_instance_id="agent-model",
        snapshot=None,
        versions={"adapter": "1.0"},
        supported_languages=("python", "rust"),
        supported_request_forms=("status",),
        capability_statuses=(),
        freshness_state="CURRENT",
        service_limits={},
        notices=(),
    )

    async def status():
        return expected

    status.__annotations__["return"] = module.StatusToolOutput
    server.tool(
        name="status",
        output_schema=module.StatusToolOutput.model_json_schema(mode="serialization"),
    )(status)

    async def exercise() -> None:
        tool = await server.get_tool("status")
        assert tool is not None
        assert (
            tool.to_mcp_tool().outputSchema
            == module.StatusToolOutput.model_json_schema(mode="serialization")
        )
        async with Client(server) as client:
            result = await client.call_tool("status", {})
        assert result.structured_content == expected.model_dump(mode="json")

    asyncio.run(exercise())


def test_model_adapter_rejects_unknown_missing_mistyped_and_lax_values() -> None:
    response = _rendered()
    module = _load_candidate(_output(response, "/wire_models.py").encode())
    with pytest.raises(ValidationError, match="extra_forbidden"):
        module.QueryCounts(fact_count=1, result_count=1, truncated=False, leaked=True)
    with pytest.raises(ValidationError, match="int_type"):
        module.QueryCounts(fact_count="1", result_count=1, truncated=False)
    with pytest.raises(ValidationError, match="missing"):
        module.QueryCounts(fact_count=1, result_count=1)
    raw = json.loads(IR_PATH.read_text(encoding="utf-8"))
    raw["unknown"] = True
    with pytest.raises(ValidationError, match="extra_forbidden"):
        AdapterDriverIr.model_validate_json(json.dumps(raw), strict=True)

    alias_ir = json.loads(IR_PATH.read_text(encoding="utf-8"))
    query_counts = next(
        model for model in alias_ir["models"] if model["name"] == "QueryCounts"
    )
    query_counts["fields"][0]["alias"] = "factCount"
    alias_response = execute(_request(json.dumps(alias_ir)))
    assert isinstance(alias_response, RenderResponse)
    alias_module = _load_candidate(_output(alias_response, "/wire_models.py").encode())
    aliased = alias_module.QueryCounts(factCount=1, result_count=1, truncated=False)
    assert aliased.model_dump(mode="json") == {
        "factCount": 1,
        "result_count": 1,
        "truncated": False,
    }


def test_model_adapter_semantic_field_mutation_changes_both_schema_modes() -> None:
    original = json.loads(_output(_rendered(), "/adapter-fingerprints.json"))
    mutated_ir = json.loads(IR_PATH.read_text(encoding="utf-8"))
    query_counts = next(
        model for model in mutated_ir["models"] if model["name"] == "QueryCounts"
    )
    query_counts["fields"][0]["description"] = "Changed client-visible description."
    mutated_response = execute(_request(json.dumps(mutated_ir)))
    assert isinstance(mutated_response, RenderResponse)
    mutated = json.loads(_output(mutated_response, "/adapter-fingerprints.json"))
    for mode in ("validation", "serialization"):
        assert original[mode]["QueryCounts"] != mutated[mode]["QueryCounts"]


def test_model_adapter_driver_cannot_write_kats_or_acceptance() -> None:
    raw = json.loads(IR_PATH.read_text(encoding="utf-8"))
    raw["projections"][0]["path"] = "contracts/acceptance/generated-answer.json"
    with pytest.raises(ValidationError, match="cannot own acceptance"):
        AdapterDriverIr.model_validate_json(json.dumps(raw), strict=True)
    with pytest.raises(ValidationError, match="complete metadata"):
        AdapterProjection(
            output_id="output:incomplete-schema",
            path="contracts/adapter/incomplete.schema.json",
            role="public-json-schema",
            projection_kind="public-json-schema",
        )


def test_model_adapter_has_no_independent_schema_authority_or_hot_loop_construction() -> (
    None
):
    driver_source = (ROOT / "tooling/model/adapter_driver.py").read_text(
        encoding="utf-8"
    )
    response = _rendered()
    model_source = _output(response, "/wire_models.py")
    assert "PUBLIC_SCHEMA_ARTIFACTS" not in driver_source
    assert 'endswith("/wire_models.py")' not in driver_source
    assert "MODEL_ADAPTERS = {" in model_source
    assert "def parse(" not in model_source
    ir = AdapterDriverIr.model_validate_json(IR_PATH.read_bytes(), strict=True)
    public = [
        projection
        for projection in ir.projections
        if projection.role == "public-json-schema"
    ]
    assert public
    assert {projection.mode for projection in public} == {"validation", "serialization"}
    assert all(projection.artifact_id and projection.title for projection in public)
    assert all(projection.model_roots for projection in public)
