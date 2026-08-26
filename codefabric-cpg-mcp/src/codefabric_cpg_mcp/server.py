"""FastMCP STDIO adapter backed by the private CodeFabric daemon."""

from __future__ import annotations

from collections.abc import AsyncIterator
from datetime import UTC, datetime
from typing import Annotated, Any, Literal, cast

from fastmcp import Context, FastMCP
from fastmcp.dependencies import CurrentContext
from fastmcp.exceptions import ResourceError, ToolError
from fastmcp.server.lifespan import lifespan
from fastmcp.tools import ToolResult
from mcp.types import TextContent, ToolAnnotations
from pydantic import Field

from .contracts.json import canonicalize_value
from .contracts.model_registries import REGISTRY_IDS
from .contracts.schemas import schema_fingerprints, schema_manifest
from .contracts.wire_models import (
    JSON_OBJECT_ADAPTER,
    TYPE_ADAPTERS,
    InlineDelivery,
    InlineReference,
    PublicToolMeta,
    QueryCounts,
    QueryStatus,
    QueryToolInput,
    QueryToolOutput,
    ReferenceToolOutput,
    ResourceDelivery,
    ResourceReference,
    ResultResource,
    SnapshotSummary,
    StatusToolOutput,
    ValidateQueryOutput,
    ValidateToolInput,
    ValidationIssue,
)
from .daemon import CpgDaemonClient, DaemonQueryError
from .settings import process_settings

SERVER_INSTRUCTIONS = """\
Use CodeFabric to inspect a registered workspace's immutable present-state code graph.
Put related entity, fact, and relationship work into one composable semantic request.
Use validate_code_graph_query to diagnose a request without executing it.
"""

READ_ONLY_CLOSED_WORLD_ANNOTATIONS = ToolAnnotations(
    title="Query Code Graph",
    readOnlyHint=True,
    destructiveHint=False,
    idempotentHint=True,
    openWorldHint=False,
)
_CURRENT_CONTEXT = CurrentContext()


def _output_schema(name: str) -> dict[str, Any]:
    schemas = schema_manifest()["serialization"]
    if not isinstance(schemas, dict) or not isinstance(schemas.get(name), dict):
        raise RuntimeError(f"missing generated serialization schema: {name}")
    schema = dict(cast(dict[str, Any], schemas[name]))
    # MCP requires every tool output schema to advertise an object at its root. A
    # discriminated union of closed object variants is still an object contract,
    # but Pydantic emits only ``oneOf`` for the root TypeAdapter view.
    if "oneOf" in schema and "type" not in schema:
        schema["type"] = "object"
    return schema


@lifespan
async def server_lifespan(_server: FastMCP[Any]) -> AsyncIterator[dict[str, Any]]:
    """Load immutable settings and own one shared, lazily negotiated daemon channel."""

    settings = process_settings()
    client = CpgDaemonClient(settings)
    try:
        yield {"settings": settings, "daemon": client}
    finally:
        await client.close()


mcp = FastMCP(
    name="CodeFabric Present-State CPG",
    version="1.3.0",
    instructions=SERVER_INSTRUCTIONS,
    lifespan=server_lifespan,
    on_duplicate="error",
    strict_input_validation=True,
    mask_error_details=True,
    list_page_size=50,
    tasks=False,
)


async def _daemon(ctx: Context) -> CpgDaemonClient:
    client = cast(CpgDaemonClient, ctx.lifespan_context["daemon"])
    await client.connect()
    return client


def _snapshot(value: dict[str, Any]) -> SnapshotSummary:
    # JSON arrays are the wire representation of immutable tuple fields. Validate at
    # Pydantic's JSON boundary so strict mode preserves that documented JSON/Python seam.
    return SnapshotSummary.model_validate_json(canonicalize_value(value), strict=True)


def _tool_result(output: Any, meta: PublicToolMeta, summary: str) -> ToolResult:
    return ToolResult(
        content=[TextContent(type="text", text=summary)],
        structured_content=output.model_dump(mode="json", exclude_none=True),
        meta=meta.model_dump(mode="json", exclude_none=True),
    )


def _validation_issue(value: bytes) -> ValidationIssue:
    canonical = canonicalize_value(JSON_OBJECT_ADAPTER.validate_json(value, strict=True))
    if canonical != value:
        raise RuntimeError("daemon validation error record is not canonical JSON")
    record = cast(dict[str, Any], JSON_OBJECT_ADAPTER.validate_json(value, strict=True))
    code_value = record.get("name", record.get("code"))
    if not isinstance(code_value, (str, int)):
        raise RuntimeError("daemon validation error record omitted its code")
    raw_path = record.get("path")
    if isinstance(raw_path, list) and all(isinstance(item, str) for item in raw_path):
        path = tuple(raw_path)
    else:
        field = record.get("field")
        phase = record.get("phase")
        path = (field,) if isinstance(field, str) else ((phase,) if isinstance(phase, str) else ())
    message = next(
        (
            candidate
            for candidate in (record.get("safe_message"), record.get("detail"))
            if isinstance(candidate, str)
        ),
        value.decode("utf-8"),
    )
    return ValidationIssue(code=str(code_value), path=path, message=message)


_REFERENCE_MEDIA_TYPES = {
    "agent_guide": "text/markdown",
    "query_specification": "text/markdown",
    "request_schema": "application/schema+json",
    "response_schema": "application/schema+json",
    "recipe_index": "application/json",
    "capabilities": "application/json",
}


def _reference_content(reference: str) -> str:
    if reference == "capabilities":
        return canonicalize_value(
            {"registry_ids": {name: list(values) for name, values in REGISTRY_IDS.items()}}
        ).decode("utf-8")
    if reference == "recipe_index":
        return canonicalize_value(
            {
                "query": "query_code_graph",
                "reference": "get_code_graph_reference",
                "status": "get_code_graph_status",
                "validate": "validate_code_graph_query",
            }
        ).decode("utf-8")
    if reference in {"request_schema", "response_schema"}:
        schema_name = (
            "cpg-semantic-query-request"
            if reference == "request_schema"
            else "cpg-semantic-query-response"
        )
        return canonicalize_value(
            {
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "$ref": f"https://codefabric.dev/schema/1.3/{schema_name}.schema.json",
                "x-codefabric-contract-version": "1.3",
            }
        ).decode("utf-8")
    if reference == "agent_guide":
        return SERVER_INSTRUCTIONS
    if reference == "query_specification":
        return "# CodeFabric semantic query specification\n\nContract version: 1.3\n"
    raise ResourceError(f"unknown CodeFabric reference: {reference}")


@mcp.tool(
    name="query_code_graph",
    version="1.3",
    description="Execute one bounded composable present-state CPG fact query.",
    tags={"cpg", "facts", "read", "primary"},
    timeout=120.0,
    annotations=READ_ONLY_CLOSED_WORLD_ANNOTATIONS,
    meta={"semantic_query_specification": "1.3", "canonical": True, "daemon_backed": True},
    output_schema=_output_schema("QueryToolOutput"),
)
async def query_code_graph(
    request: Annotated[dict[str, Any], Field(description="Complete semantic query request.")],
    delivery: Annotated[
        Literal["automatic", "inline", "resource"],
        Field(description="MCP presentation preference only."),
    ] = "automatic",
    ctx: Context = _CURRENT_CONTEXT,
) -> ToolResult:
    """Execute one daemon-owned semantic query."""

    client = await _daemon(ctx)
    tool_input = QueryToolInput(request=request, delivery=delivery)
    try:
        result = await client.execute(tool_input)
    except DaemonQueryError as error:
        raise ToolError(error.canonical_bytes.decode("utf-8")) from None
    statuses = tuple(
        QueryStatus.model_validate(status, strict=True) for status in result.query_statuses
    )
    snapshot = _snapshot(cast(dict[str, Any], result.snapshot))
    if result.response is None:
        expires_at = (
            datetime.fromtimestamp(result.lease_expires_at_unix_ms / 1000, tz=UTC)
            .isoformat()
            .replace("+00:00", "Z")
        )
        delivery_result: InlineDelivery | ResourceDelivery = ResourceDelivery(
            result_bytes=result.result_byte_count,
            checksum=result.checksum,
            result_resource=ResultResource(
                uri=f"cpg://result/{result.artifact_id}",
                manifest_uri=f"cpg://result/{result.artifact_id}/manifest",
                expires_at=expires_at,
                subresource_uris=(),
            ),
            preview=None,
        )
    else:
        delivery_result = InlineDelivery(
            result_bytes=result.result_byte_count,
            checksum=result.checksum,
            response=result.response,
        )
    output = QueryToolOutput(
        semantic_request_id=result.semantic_request_id,
        mcp_call_id=result.daemon_query_id,
        execution_state=cast(Any, result.execution_state),
        availability_state=cast(Any, result.availability_state),
        completeness_state=cast(Any, result.completeness_state),
        freshness_state=cast(Any, result.freshness_state),
        limit_state=cast(Any, result.limit_state),
        snapshot=snapshot,
        delivery=delivery_result,
        counts=QueryCounts(
            fact_count=result.result_row_count,
            result_count=len(statuses),
            truncated=result.truncated,
        ),
        query_statuses=statuses,
        notices=result.notices,
    )
    meta = PublicToolMeta(
        contract_version="1.3",
        semantic_request_id=result.semantic_request_id,
        snapshot_id=output.snapshot.snapshot_id,
        canonical_response_digest=result.checksum,
        daemon_rpc_version="1.0",
    )
    return _tool_result(output, meta, f"Completed {len(statuses)} semantic query clauses.")


@mcp.resource(
    "cpg://result/{artifact_id}",
    name="CodeFabric query result",
    description="Read one immutable daemon result artifact and release its lease.",
    mime_type="application/json",
)
async def get_query_result_resource(
    artifact_id: str,
    ctx: Context = _CURRENT_CONTEXT,
) -> str:
    """Resolve a previously returned resource delivery exactly once."""

    try:
        return (await (await _daemon(ctx)).read_resource(artifact_id)).decode("utf-8")
    except RuntimeError as error:
        raise ResourceError(str(error)) from None


@mcp.resource(
    "cpg://reference/{reference}/{version}",
    name="CodeFabric packaged reference",
    description="Read one constrained versioned CodeFabric reference.",
    mime_type="text/plain",
)
async def get_reference_resource(reference: str, version: str) -> str:
    """Resolve only accepted versioned reference identities."""

    if version != "1.3" or reference not in _REFERENCE_MEDIA_TYPES:
        raise ResourceError(f"unknown CodeFabric reference: {reference}/{version}")
    return _reference_content(reference)


@mcp.tool(
    name="validate_code_graph_query",
    version="1.3",
    description="Validate and resolve a semantic request without executing fact retrieval.",
    tags={"cpg", "validate", "read"},
    annotations=READ_ONLY_CLOSED_WORLD_ANNOTATIONS,
    output_schema=_output_schema("ValidateQueryOutput"),
)
async def validate_code_graph_query(
    request: Annotated[dict[str, Any], Field(description="Complete semantic query request.")],
    ctx: Context = _CURRENT_CONTEXT,
) -> ValidateQueryOutput:
    """Return the daemon's bounded validation view."""

    response, normalized = await (await _daemon(ctx)).validate(ValidateToolInput(request=request))
    errors = tuple(_validation_issue(value) for value in response.canonical_error_records_json)
    return ValidateQueryOutput(
        valid=response.valid,
        request_id=response.effective_semantic_request_id,
        normalized_request=normalized,
        dependency_graph={"checks": list(response.provisional_snapshot_checks)},
        resolved_semantics={},
        capability_requirements=(),
        resource_estimate={"cost_class": response.cost_class},
        errors=errors,
        warnings=(),
    )


@mcp.tool(
    name="get_code_graph_status",
    version="1.3",
    description="Return the safe public readiness, freshness, capability, and version view.",
    tags={"cpg", "status", "read"},
    annotations=READ_ONLY_CLOSED_WORLD_ANNOTATIONS,
    output_schema=_output_schema("StatusToolOutput"),
)
async def get_code_graph_status(ctx: Context = _CURRENT_CONTEXT) -> StatusToolOutput:
    """Read public daemon status without triggering generation."""

    client = await _daemon(ctx)
    response, status = await client.status()
    if response.workspace_id != status.get("workspace_id"):
        raise RuntimeError("daemon status workspace identity differs")
    public_status = dict(status)
    versions = public_status.get("versions")
    if not isinstance(versions, dict):
        raise RuntimeError("daemon status versions are unavailable")
    public_status["versions"] = {"adapter": "1.3", **versions}
    return StatusToolOutput.model_validate_json(canonicalize_value(public_status), strict=True)


@mcp.tool(
    name="get_code_graph_reference",
    version="1.3",
    description="Return a constrained packaged schema/reference view.",
    tags={"cpg", "reference", "read"},
    annotations=READ_ONLY_CLOSED_WORLD_ANNOTATIONS,
    output_schema=_output_schema("ReferenceToolOutput"),
)
async def get_code_graph_reference(
    reference: Annotated[
        Literal[
            "agent_guide",
            "query_specification",
            "request_schema",
            "response_schema",
            "query_tool_output_schema",
            "validate_tool_output_schema",
            "status_tool_output_schema",
            "reference_tool_output_schema",
            "recipe_index",
            "capabilities",
        ],
        Field(description="Constrained packaged reference identity."),
    ],
) -> ReferenceToolOutput:
    """Return generated schemas inline and stable resource links for larger references."""

    schema_names = {
        "query_tool_output_schema": "QueryToolOutput",
        "validate_tool_output_schema": "ValidateQueryOutput",
        "status_tool_output_schema": "StatusToolOutput",
        "reference_tool_output_schema": "ReferenceToolOutput",
    }
    if reference in schema_names:
        text = canonicalize_value(_output_schema(schema_names[reference])).decode()
        return TYPE_ADAPTERS["ReferenceToolOutput"].validate_python(
            InlineReference(media_type="application/schema+json", text=text), strict=True
        )
    return TYPE_ADAPTERS["ReferenceToolOutput"].validate_python(
        ResourceReference(
            uri=f"cpg://reference/{reference}/1.3",
            media_type=_REFERENCE_MEDIA_TYPES[reference],
        ),
        strict=True,
    )


# Eager validation prevents exposing tool definitions with drifted generated schemas.
schema_fingerprints()

__all__ = ["mcp"]
