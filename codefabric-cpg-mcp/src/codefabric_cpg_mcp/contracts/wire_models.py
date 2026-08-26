# @generated from codefabric.adapter.model-ir source b3:e8572d05a57be81326b49cf6515d455a96c6df1fe9e0d9f65c3efdd27ce7c6a4; codefabric-model-adapter-driver-v1; do not edit.
"""Statically typed public adapter contracts compiled from Contract IR."""

from typing import Annotated, Literal

from pydantic import BaseModel, ConfigDict, Field, JsonValue, StringConstraints, TypeAdapter

Checksum = Annotated[str, StringConstraints(pattern=r"^b3:[0-9a-f]{64}$")]
NonNegativeInt = Annotated[int, Field(ge=0)]
type JsonObject = dict[str, JsonValue]


class StrictWireModel(BaseModel):
    """Closed immutable model-visible MCP contract."""

    model_config = ConfigDict(
        extra="forbid",
        strict=True,
        frozen=True,
        validate_default=True,
        hide_input_in_errors=True,
        allow_inf_nan=False,
        validate_by_alias=True,
        validate_by_name=True,
        serialize_by_alias=True,
    )


JSON_OBJECT_ADAPTER = TypeAdapter(
    JsonObject,
    config=ConfigDict(strict=True, allow_inf_nan=False, hide_input_in_errors=True),
)


class SnapshotSummary(StrictWireModel):
    """Generated SnapshotSummary wire contract."""

    snapshot_id: str = Field(description="Immutable snapshot identity.")
    workspace_id: str = Field(description="Authorized workspace identity.")
    repository_id: str | None = Field(
        default=None, description="Repository identity when available."
    )
    worktree_id: str | None = Field(default=None, description="Worktree identity when available.")
    source_generation: NonNegativeInt = Field(description="Monotonic source generation.")
    source_inventory_digest: Checksum = Field(description="Canonical source inventory identity.")
    durable_base_publication: str = Field(description="Durable publication identity.")
    base_table_version_digest: Checksum = Field(description="Base table version identity.")
    overlay_generation: NonNegativeInt = Field(description="Overlay generation.")
    overlay_checksum: Checksum = Field(description="Overlay identity.")
    analysis_context_set_id: str = Field(description="Analysis-context set identity.")
    analysis_context_ids: tuple[str, ...] = Field(
        description="Ordered analysis-context identities."
    )
    freshness_state: Literal["CURRENT", "POTENTIALLY_STALE", "UNAVAILABLE"] = Field(
        description="Snapshot freshness state."
    )
    source_trust_state: str = Field(description="Source trust state.")
    event_stream_health: str = Field(description="Lifecycle event-stream health.")
    git_acceleration_status: str = Field(description="Git acceleration status.")
    git_operation_summary: JsonObject | None = Field(
        default=None, description="Safe Git operation summary."
    )
    pending_update_count: NonNegativeInt = Field(description="Pending update count.")
    ontology_version: str = Field(description="Ontology contract version.")
    schema_bundle_version: str = Field(description="Schema bundle version.")
    provider_bundle_version: str = Field(description="Provider bundle version.")
    derivation_bundle_version: str = Field(description="Derivation bundle version.")
    query_language_version: str = Field(description="Query-language contract version.")
    capability_summaries: tuple[JsonObject, ...] = Field(
        description="Explicit public capability summaries."
    )
    diagnostic_references: tuple[str, ...] = Field(
        description="Safe diagnostic reference identities."
    )


class QueryCounts(StrictWireModel):
    """Generated QueryCounts wire contract."""

    fact_count: NonNegativeInt = Field(description="Facts returned.")
    result_count: NonNegativeInt = Field(description="Logical query results returned.")
    truncated: bool = Field(description="Whether an explicit limit affected delivery.")


class QueryStatus(StrictWireModel):
    """Generated QueryStatus wire contract."""

    query_id: str = Field(description="Logical query identity.")
    state: Literal[
        "COMPLETE", "FAILED", "CANCELLED", "DEADLINE_EXCEEDED", "NOT_EXECUTED_DEPENDENCY"
    ] = Field(description="Terminal query state.")
    message: str | None = Field(default=None, description="Safe status explanation.")


class ResultResource(StrictWireModel):
    """Generated ResultResource wire contract."""

    uri: str = Field(description="Immutable result URI.")
    manifest_uri: str = Field(description="Result manifest URI.")
    expires_at: str = Field(description="RFC 3339 expiry timestamp.")
    subresource_uris: tuple[str, ...] = Field(description="Bounded result subresources.")


class InlineDelivery(StrictWireModel):
    """Generated InlineDelivery wire contract."""

    mode: Literal["inline"] = Field(default="inline", description="Inline delivery discriminator.")
    canonical_mime_type: Literal["application/json"] = Field(
        default="application/json", description="Canonical response media type."
    )
    result_bytes: NonNegativeInt = Field(description="Exact canonical result size.")
    checksum: Checksum = Field(description="Canonical result checksum.")
    response: JsonObject = Field(description="Daemon-authoritative canonical response object.")


class ResourceDelivery(StrictWireModel):
    """Generated ResourceDelivery wire contract."""

    mode: Literal["resource"] = Field(
        default="resource", description="Resource delivery discriminator."
    )
    canonical_mime_type: Literal["application/json"] = Field(
        default="application/json", description="Canonical response media type."
    )
    result_bytes: NonNegativeInt = Field(description="Exact canonical result size.")
    checksum: Checksum = Field(description="Canonical result checksum.")
    result_resource: ResultResource = Field(description="Immutable result resource.")
    preview: JsonObject | None = Field(default=None, description="Optional bounded preview.")


class PublicToolMeta(StrictWireModel):
    """Generated PublicToolMeta wire contract."""

    contract_version: str = Field(description="Adapter public-contract version.")
    semantic_request_id: str = Field(description="Semantic request identity.")
    snapshot_id: str = Field(description="Pinned snapshot identity.")
    canonical_response_digest: Checksum = Field(description="Canonical daemon response identity.")
    daemon_rpc_version: str = Field(description="Negotiated daemon RPC version.")


class ValidationIssue(StrictWireModel):
    """Generated ValidationIssue wire contract."""

    code: str = Field(description="Stable validation code.")
    path: tuple[str, ...] = Field(description="Safe logical path.")
    message: str = Field(description="Safe issue explanation.")


class ValidateQueryOutput(StrictWireModel):
    """Generated ValidateQueryOutput wire contract."""

    valid: bool = Field(description="Whether the semantic request is valid.")
    request_id: str = Field(description="Validation request identity.")
    normalized_request: JsonObject | None = Field(
        default=None, description="Daemon-normalized semantic request."
    )
    dependency_graph: JsonObject = Field(description="Resolved dependency graph.")
    resolved_semantics: JsonObject = Field(description="Resolved semantic phrases.")
    capability_requirements: tuple[JsonObject, ...] = Field(description="Required capabilities.")
    resource_estimate: JsonObject = Field(description="Bounded resource estimate.")
    errors: tuple[ValidationIssue, ...] = Field(description="Validation failures.")
    warnings: tuple[ValidationIssue, ...] = Field(description="Validation warnings.")


class StatusToolOutput(StrictWireModel):
    """Generated StatusToolOutput wire contract."""

    ready: bool = Field(description="Adapter readiness.")
    workspace_id: str = Field(description="Authorized workspace identity.")
    agent_instance_id: str = Field(description="Agent instance identity.")
    snapshot: SnapshotSummary | None = Field(
        default=None, description="Active snapshot when available."
    )
    versions: JsonObject = Field(description="Explicit public component versions.")
    supported_languages: tuple[str, ...] = Field(description="Supported source languages.")
    supported_request_forms: tuple[str, ...] = Field(
        description="Supported semantic request forms."
    )
    capability_statuses: tuple[JsonObject, ...] = Field(
        description="Explicit public capability statuses."
    )
    freshness_state: Literal["CURRENT", "POTENTIALLY_STALE", "UNAVAILABLE"] = Field(
        description="Active freshness state."
    )
    service_limits: JsonObject = Field(description="Safe hard service limits.")
    notices: tuple[str, ...] = Field(description="Safe public notices.")


class InlineReference(StrictWireModel):
    """Generated InlineReference wire contract."""

    mode: Literal["inline"] = Field(default="inline", description="Inline reference discriminator.")
    media_type: str = Field(description="Reference media type.")
    text: str = Field(description="Packaged reference content.")


class ResourceReference(StrictWireModel):
    """Generated ResourceReference wire contract."""

    mode: Literal["resource"] = Field(
        default="resource", description="Resource reference discriminator."
    )
    uri: str = Field(description="Constrained MCP resource URI.")
    media_type: str = Field(description="Reference media type.")


class QueryToolInput(StrictWireModel):
    """Generated QueryToolInput wire contract."""

    request: JsonObject = Field(description="Complete daemon-owned semantic request object.")
    delivery: Literal["automatic", "inline", "resource"] = Field(
        default="automatic", description="MCP delivery preference only."
    )


class ValidateToolInput(StrictWireModel):
    """Generated ValidateToolInput wire contract."""

    request: JsonObject = Field(description="Complete daemon-owned semantic request object.")


class StatusToolInput(StrictWireModel):
    """Generated StatusToolInput wire contract."""

    pass


class ReferenceToolInput(StrictWireModel):
    """Generated ReferenceToolInput wire contract."""

    reference: Literal[
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
    ] = Field(description="Constrained packaged reference identity.")


type Delivery = Annotated[
    InlineDelivery | ResourceDelivery,
    Field(discriminator="mode"),
]

type ReferenceToolOutput = Annotated[
    InlineReference | ResourceReference,
    Field(discriminator="mode"),
]


class QueryToolOutput(StrictWireModel):
    """Generated QueryToolOutput wire contract."""

    semantic_request_id: str = Field(description="Semantic idempotency identity.")
    mcp_call_id: str = Field(description="MCP invocation correlation identity.")
    execution_state: Literal[
        "COMPLETE", "FAILED", "CANCELLED", "DEADLINE_EXCEEDED", "NOT_EXECUTED_DEPENDENCY"
    ] = Field(description="Execution state.")
    availability_state: Literal["AVAILABLE", "PARTIAL", "UNAVAILABLE", "NOT_APPLICABLE"] = Field(
        description="Availability state."
    )
    completeness_state: Literal[
        "COMPLETE", "PARTIAL", "INDETERMINATE", "UNAVAILABLE", "NOT_APPLICABLE"
    ] = Field(description="Completeness state.")
    freshness_state: Literal["CURRENT", "POTENTIALLY_STALE", "UNAVAILABLE"] = Field(
        description="Freshness state."
    )
    limit_state: Literal["NOT_APPLIED", "EXPLICIT_LIMIT_REACHED", "HARD_LIMIT_REJECTED"] = Field(
        description="Limit state."
    )
    snapshot: SnapshotSummary = Field(description="Pinned public snapshot.")
    delivery: Delivery = Field(description="Discriminated delivery result.")
    counts: QueryCounts = Field(description="Query counts.")
    query_statuses: tuple[QueryStatus, ...] = Field(description="Per-query terminal statuses.")
    notices: tuple[str, ...] = Field(description="Safe public notices.")


MODEL_TYPES = (
    SnapshotSummary,
    QueryCounts,
    QueryStatus,
    ResultResource,
    InlineDelivery,
    ResourceDelivery,
    PublicToolMeta,
    ValidationIssue,
    ValidateQueryOutput,
    StatusToolOutput,
    InlineReference,
    ResourceReference,
    QueryToolInput,
    ValidateToolInput,
    StatusToolInput,
    ReferenceToolInput,
    QueryToolOutput,
)
MODEL_BY_NAME = {model.__name__: model for model in MODEL_TYPES}
TYPE_ADAPTERS = {
    "JsonObject": JSON_OBJECT_ADAPTER,
    "Delivery": TypeAdapter(Delivery),
    "ReferenceToolOutput": TypeAdapter(ReferenceToolOutput),
}
MODEL_ADAPTERS = {
    "SnapshotSummary": TypeAdapter(SnapshotSummary),
    "QueryCounts": TypeAdapter(QueryCounts),
    "QueryStatus": TypeAdapter(QueryStatus),
    "ResultResource": TypeAdapter(ResultResource),
    "InlineDelivery": TypeAdapter(InlineDelivery),
    "ResourceDelivery": TypeAdapter(ResourceDelivery),
    "QueryToolOutput": TypeAdapter(QueryToolOutput),
    "PublicToolMeta": TypeAdapter(PublicToolMeta),
    "ValidationIssue": TypeAdapter(ValidationIssue),
    "ValidateQueryOutput": TypeAdapter(ValidateQueryOutput),
    "StatusToolOutput": TypeAdapter(StatusToolOutput),
    "InlineReference": TypeAdapter(InlineReference),
    "ResourceReference": TypeAdapter(ResourceReference),
    "QueryToolInput": TypeAdapter(QueryToolInput),
    "ValidateToolInput": TypeAdapter(ValidateToolInput),
    "StatusToolInput": TypeAdapter(StatusToolInput),
    "ReferenceToolInput": TypeAdapter(ReferenceToolInput),
}
__all__ = [
    "SnapshotSummary",
    "QueryCounts",
    "QueryStatus",
    "ResultResource",
    "InlineDelivery",
    "ResourceDelivery",
    "QueryToolOutput",
    "PublicToolMeta",
    "ValidationIssue",
    "ValidateQueryOutput",
    "StatusToolOutput",
    "InlineReference",
    "ResourceReference",
    "QueryToolInput",
    "ValidateToolInput",
    "StatusToolInput",
    "ReferenceToolInput",
    "Delivery",
    "ReferenceToolOutput",
    "JSON_OBJECT_ADAPTER",
    "MODEL_ADAPTERS",
    "TYPE_ADAPTERS",
]
