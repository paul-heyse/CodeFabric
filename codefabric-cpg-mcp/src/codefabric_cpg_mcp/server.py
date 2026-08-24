"""FastMCP STDIO adapter backed by the private CodeFabric daemon."""

from __future__ import annotations

from collections.abc import AsyncIterator
from datetime import UTC, datetime
from typing import Annotated, Any, Literal, cast

from fastmcp import Context, FastMCP
from fastmcp.dependencies import CurrentContext
from fastmcp.server.lifespan import lifespan
from fastmcp.tools import ToolResult
from mcp.types import TextContent, ToolAnnotations
from pydantic import Field

from .contracts.json import canonicalize_value
from .contracts.schemas import schema_fingerprints, schema_manifest
from .contracts.wire_models import (
    JSON_OBJECT_ADAPTER,
    TYPE_ADAPTERS,
    InlineDelivery,
    InlineReference,
    PublicToolMeta,
    QueryCounts,
    QueryStatus,
    QueryToolOutput,
    ReferenceToolOutput,
    ResourceDelivery,
    ResourceReference,
    ResultResource,
    SnapshotSummary,
    StatusToolOutput,
    ValidateQueryOutput,
    ValidationIssue,
)
from .daemon import CpgDaemonClient
from .daemon.generated import cpg_query_service_pb2 as query_pb
from .settings import Settings

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

    settings = Settings()
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


def _settings(ctx: Context) -> Settings:
    return cast(Settings, ctx.lifespan_context["settings"])


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
    result = await client.execute(request, delivery)
    raw_query_results = result.response.get("query_results", []) if result.response else []
    query_results = raw_query_results if isinstance(raw_query_results, list) else []
    statuses = tuple(
        QueryStatus(query_id=str(item.get("query_id", "unknown")), state="COMPLETE", message=None)
        for item in query_results
        if isinstance(item, dict)
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
        execution_state="COMPLETE",
        availability_state=("AVAILABLE" if result.availability_state == "AVAILABLE" else "PARTIAL"),
        completeness_state="COMPLETE",
        freshness_state=(
            "CURRENT" if result.freshness_state in {"CURRENT", "PINNED"} else "POTENTIALLY_STALE"
        ),
        limit_state=(
            "NOT_APPLIED" if result.limit_state == "NOT_APPLIED" else "EXPLICIT_LIMIT_REACHED"
        ),
        snapshot=snapshot,
        delivery=delivery_result,
        counts=QueryCounts(
            fact_count=result.result_row_count,
            result_count=len(statuses),
            truncated=result.limit_state != "NOT_APPLIED",
        ),
        query_statuses=statuses,
        notices=(),
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

    return (await (await _daemon(ctx)).read_resource(artifact_id)).decode("utf-8")


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

    response, normalized = await (await _daemon(ctx)).validate(request)
    errors = tuple(
        ValidationIssue(
            code="SEMANTIC_QUERY_INVALID",
            path=(),
            message=str(JSON_OBJECT_ADAPTER.validate_json(value, strict=True)),
        )
        for value in response.canonical_error_records_json
    )
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
    handshake = client.handshake_response
    if handshake is None:
        raise RuntimeError("daemon handshake is unavailable")
    return StatusToolOutput(
        ready=response.readiness == query_pb.WORKSPACE_READINESS_READY,
        workspace_id=_settings(ctx).workspace_id,
        agent_instance_id=_settings(ctx).agent_instance_id,
        snapshot=(
            _snapshot(cast(dict[str, Any], status["snapshot"]))
            if isinstance(status.get("snapshot"), dict)
            else None
        ),
        versions={
            "adapter": "1.3",
            "daemon": handshake.daemon_version,
            "rpc": handshake.negotiated_rpc_version,
            "semantic_query": handshake.negotiated_semantic_query_version,
        },
        supported_languages=("python", "rust"),
        supported_request_forms=tuple(handshake.readiness.supported_query_forms),
        capability_statuses=(),
        freshness_state=cast(Any, status.get("freshness_state", "UNAVAILABLE")),
        service_limits={
            "maximum_control_message_bytes": handshake.effective_limits.maximum_control_message_bytes,
            "maximum_payload_chunk_bytes": handshake.effective_limits.maximum_payload_chunk_bytes,
        },
        notices=(),
    )


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
        ResourceReference(uri=f"cpg://reference/{reference}/1.3", media_type="text/markdown"),
        strict=True,
    )


# Eager validation prevents exposing tool definitions with drifted generated schemas.
schema_fingerprints()

__all__ = ["mcp"]
