"""Strict presentation models for the target-only daemon v2 boundary."""

from enum import StrEnum
from functools import lru_cache
from typing import Annotated, Any, Literal, cast

from pydantic import (
    BaseModel,
    ConfigDict,
    Field,
    JsonValue,
    StringConstraints,
    TypeAdapter,
    model_validator,
)

from .json import JsonValue as CanonicalJsonValue
from .json import canonicalize_value, checksum

Checksum = Annotated[str, StringConstraints(pattern=r"^b3:[0-9a-f]{64}$")]
NonEmptyString = Annotated[str, StringConstraints(min_length=1, max_length=512)]
NonNegativeInt = Annotated[int, Field(ge=0)]
PositiveInt = Annotated[int, Field(gt=0)]
type JsonObject = dict[str, JsonValue]
type WireSchemaMode = Literal["validation", "serialization"]

JSON_SCHEMA_DIALECT = "https://json-schema.org/draft/2020-12/schema"
_PUBLIC_SCHEMA_BASE_URI = "https://codefabric.dev/schema/adapter/2.3"


class StrictWireModel(BaseModel):
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


class AuthorityProjection(StrictWireModel):
    """Public, non-secret authority generations for one daemon session."""

    session_id: str
    session_generation: PositiveInt
    daemon_generation: PositiveInt
    supervisor_generation: PositiveInt
    policy_generation: NonNegativeInt
    revocation_generation: NonNegativeInt


class SafeErrorProjection(StrictWireModel):
    """Allowlisted daemon failure metadata without server prose."""

    code: Literal[
        "INVALID_REQUEST",
        "VALIDATION_REJECTED",
        "INPUT_REQUIRED",
        "NOT_AUTHORIZED",
        "IDEMPOTENCY_CONFLICT",
        "CONTINUATION_EXPIRED",
        "CONTINUATION_REPLAYED",
        "GENERATION_MISMATCH",
        "QUERY_NOT_FOUND",
        "RESOURCE_NOT_FOUND",
        "RESOURCE_EXPIRED",
        "RANGE_NOT_SATISFIABLE",
        "CAPACITY_UNAVAILABLE",
        "CANCELLED",
        "RESUME_WINDOW_EXPIRED",
        "DAEMON_UNAVAILABLE",
        "INTERNAL",
    ]
    layer: Literal[
        "TRANSPORT",
        "AUTHORIZATION",
        "VALIDATION",
        "QUERY",
        "RESOURCE",
        "LIFECYCLE",
    ]
    retryable: bool
    retry_after_ms: NonNegativeInt | None = None
    diagnostic_reference: Literal[
        "",
        "lifecycle.failed_closed",
        "query.challenge_rejected",
        "query.terminal",
    ] = ""


class PublicStatusProjection(StrictWireModel):
    """Closed allowlist for the daemon's canonical public status document."""

    semantic_release: NonEmptyString
    lifecycle: Literal["BOOTSTRAPPING", "READY", "DRAINING", "FAILED_CLOSED"]
    lifecycle_sequence: NonNegativeInt
    active_epoch_id: str | None = None
    running_queries: NonNegativeInt
    queued_queries: NonNegativeInt
    accepted_queries: NonNegativeInt
    reserved_result_bytes: NonNegativeInt
    reserved_result_pages: NonNegativeInt


class ResourceReference(StrictWireModel):
    """One daemon-minted public handle expressed only through a bounded URI."""

    uri: str
    kind: Literal["result_manifest", "result_page", "reference"]
    media_type: str
    package_id: str | None = None
    page_ordinal: NonNegativeInt | None = None
    total_bytes: NonNegativeInt
    content_checksum: Checksum
    expires_at_unix_ms: int


class QueryToolInput(StrictWireModel):
    request: JsonObject
    delivery: Literal["automatic", "inline", "resource"] = "automatic"


class ValidateToolInput(StrictWireModel):
    request: JsonObject


class ValidationIssue(StrictWireModel):
    code: str
    semantic_field_id: str = ""
    presentation_key: str = ""
    retryable: bool = False


class InputRequirementProjection(StrictWireModel):
    semantic_field_id: str
    input_kind: Literal[
        "string",
        "integer",
        "boolean",
        "enum",
        "string_collection",
        "integer_collection",
        "boolean_collection",
        "enum_collection",
    ]
    presentation_key: str
    description_key: str | None = None
    required: bool
    constraints: JsonObject | None = None
    authorized_choices: tuple[JsonObject, ...] = ()


type ProcessingState = Literal[
    "pending",
    "running",
    "partial",
    "unknown",
    "unavailable",
    "excluded",
    "limited",
    "unsupported",
    "failed",
    "cancelled",
]


class ProcessingRemainder(StrictWireModel):
    language: Literal["python", "rust"]
    scope_kind: NonEmptyString
    path_bytes: tuple[Annotated[int, Field(ge=0, le=255)], ...]
    path: str | None = None
    target: str | None = None
    target_kind: str | None = None
    analysis_context_id: str | None = None
    state: ProcessingState
    reason_code: NonEmptyString

    @model_validator(mode="after")
    def exact_path(self) -> ProcessingRemainder:
        try:
            text = bytes(self.path_bytes).decode("utf-8")
        except UnicodeDecodeError:
            text = None
        if self.path != text:
            raise ValueError("processing path text differs from its raw bytes")
        return self


class QueryProcessingSummary(StrictWireModel):
    query_id: NonEmptyString
    source_generation: PositiveInt
    scope: NonEmptyString
    family: NonEmptyString
    languages: tuple[Literal["python", "rust"], ...]
    requested_partitions: NonNegativeInt
    completed_partitions: NonNegativeInt
    remaining_partitions: NonNegativeInt
    remainder: tuple[ProcessingRemainder, ...]
    next_offset: NonNegativeInt | None = None
    maximum_rows: PositiveInt | None = None
    additional_rows: bool | None = None

    @model_validator(mode="after")
    def exact_counts(self) -> QueryProcessingSummary:
        if self.completed_partitions + self.remaining_partitions != self.requested_partitions:
            raise ValueError("processing partition counts disagree")
        count = len(self.remainder)
        if count > min(self.remaining_partitions, 64):
            raise ValueError("processing remainder page exceeds its scope")
        expected_next = count if count < self.remaining_partitions else None
        if self.next_offset != expected_next:
            raise ValueError("processing remainder pagination disagrees")
        if any(row.language not in self.languages for row in self.remainder):
            raise ValueError("processing remainder is outside its language scope")
        return self


class QueryToolOutput(StrictWireModel):
    """One strict object with branch invariants for both terminal start outcomes."""

    outcome: Literal["accepted", "validation_rejection"]
    daemon_query_id: str | None = None
    semantic_request_id: str | None = None
    execution_state: Literal["SUCCEEDED", "FAILED", "CANCELLED", "LOST"] | None = None
    epoch_id: str | None = None
    source_generation: PositiveInt | None = None
    processing: tuple[QueryProcessingSummary, ...] = ()
    package_id: str | None = None
    manifest: ResourceReference | None = None
    pages: tuple[ResourceReference, ...] = ()
    total_rows: NonNegativeInt = 0
    total_pages: NonNegativeInt = 0
    total_bytes: NonNegativeInt = 0
    issues: tuple[ValidationIssue, ...] = ()
    error: SafeErrorProjection | None = None
    notices: tuple[str, ...] = ()

    @model_validator(mode="after")
    def closed_outcome(self) -> QueryToolOutput:
        if self.outcome == "accepted":
            if self.daemon_query_id is None or self.semantic_request_id is None:
                raise ValueError("accepted query output requires both query identities")
            if self.execution_state is None or self.issues:
                raise ValueError("accepted query output has an invalid terminal projection")
            if len(self.pages) != self.total_pages:
                raise ValueError("accepted query output has incomplete page descriptors")
            if tuple(page.page_ordinal for page in self.pages) != tuple(range(self.total_pages)):
                raise ValueError("accepted query output page descriptors are not ordered")
            if any(
                page.kind != "result_page" or page.package_id != self.package_id
                for page in self.pages
            ):
                raise ValueError("accepted query output page descriptors differ from the package")
        elif (
            self.error is None
            or self.daemon_query_id is not None
            or self.execution_state is not None
            or self.manifest is not None
            or self.pages
        ):
            raise ValueError("validation rejection output has an invalid terminal projection")
        return self


QUERY_TOOL_OUTPUT_ADAPTER = TypeAdapter(QueryToolOutput)


class PublicToolMeta(StrictWireModel):
    contract_version: Literal["2.3"] = "2.3"
    semantic_request_id: str | None = None
    daemon_query_id: str | None = None
    challenge_id: str | None = None
    epoch_id: str | None = None
    package_id: str | None = None


class ValidateQueryOutput(StrictWireModel):
    valid: bool
    semantic_request_id: str | None = None
    normalized_request: JsonObject | None = None
    input_requirements: tuple[InputRequirementProjection, ...] = ()
    errors: tuple[ValidationIssue, ...] = ()
    warnings: tuple[ValidationIssue, ...] = ()
    cost_class: str
    estimated_result_bytes: NonNegativeInt
    estimated_result_pages: NonNegativeInt


class StatusToolOutput(StrictWireModel):
    authority: AuthorityProjection
    lifecycle: Literal["BOOTSTRAPPING", "READY", "DRAINING", "FAILED_CLOSED"]
    lifecycle_sequence: NonNegativeInt
    active_epoch_id: str | None = None
    running_queries: NonNegativeInt
    queued_queries: NonNegativeInt
    failure: SafeErrorProjection | None = None
    public_status: PublicStatusProjection


class ReferenceToolOutput(StrictWireModel):
    reference_id: str
    resource: ResourceReference


REFERENCE_TOOL_OUTPUT_ADAPTER = TypeAdapter(ReferenceToolOutput)


class WireSchemaName(StrEnum):
    AUTHORITY_PROJECTION = "AuthorityProjection"
    INPUT_REQUIREMENT_PROJECTION = "InputRequirementProjection"
    PUBLIC_TOOL_META = "PublicToolMeta"
    QUERY_TOOL_INPUT = "QueryToolInput"
    QUERY_TOOL_OUTPUT = "QueryToolOutput"
    REFERENCE_TOOL_OUTPUT = "ReferenceToolOutput"
    RESOURCE_REFERENCE = "ResourceReference"
    SAFE_ERROR_PROJECTION = "SafeErrorProjection"
    STATUS_TOOL_OUTPUT = "StatusToolOutput"
    VALIDATE_QUERY_OUTPUT = "ValidateQueryOutput"
    VALIDATE_TOOL_INPUT = "ValidateToolInput"
    VALIDATION_ISSUE = "ValidationIssue"


_WIRE_SCHEMA_ADAPTERS: dict[WireSchemaName, TypeAdapter[Any]] = {
    WireSchemaName.AUTHORITY_PROJECTION: TypeAdapter(AuthorityProjection),
    WireSchemaName.INPUT_REQUIREMENT_PROJECTION: TypeAdapter(InputRequirementProjection),
    WireSchemaName.PUBLIC_TOOL_META: TypeAdapter(PublicToolMeta),
    WireSchemaName.QUERY_TOOL_INPUT: TypeAdapter(QueryToolInput),
    WireSchemaName.QUERY_TOOL_OUTPUT: QUERY_TOOL_OUTPUT_ADAPTER,
    WireSchemaName.REFERENCE_TOOL_OUTPUT: REFERENCE_TOOL_OUTPUT_ADAPTER,
    WireSchemaName.RESOURCE_REFERENCE: TypeAdapter(ResourceReference),
    WireSchemaName.SAFE_ERROR_PROJECTION: TypeAdapter(SafeErrorProjection),
    WireSchemaName.STATUS_TOOL_OUTPUT: TypeAdapter(StatusToolOutput),
    WireSchemaName.VALIDATE_QUERY_OUTPUT: TypeAdapter(ValidateQueryOutput),
    WireSchemaName.VALIDATE_TOOL_INPUT: TypeAdapter(ValidateToolInput),
    WireSchemaName.VALIDATION_ISSUE: TypeAdapter(ValidationIssue),
}


def _schema_slug(name: WireSchemaName) -> str:
    return "".join(
        f"-{character.lower()}" if index and character.isupper() else character.lower()
        for index, character in enumerate(name.value)
    )


def wire_schema(name: WireSchemaName, mode: WireSchemaMode) -> dict[str, Any]:
    schema = _WIRE_SCHEMA_ADAPTERS[name].json_schema(mode=mode)
    schema["$id"] = f"{_PUBLIC_SCHEMA_BASE_URI}/{_schema_slug(name)}.{mode}.schema.json"
    schema["$schema"] = JSON_SCHEMA_DIALECT
    return schema


@lru_cache(maxsize=2)
def wire_schema_fingerprints(
    mode: WireSchemaMode,
) -> tuple[tuple[WireSchemaName, str], ...]:
    return tuple(
        (
            name,
            checksum(canonicalize_value(cast(CanonicalJsonValue, wire_schema(name, mode)))),
        )
        for name in sorted(WireSchemaName, key=lambda value: value.value)
    )


__all__ = [
    "AuthorityProjection",
    "InputRequirementProjection",
    "JSON_OBJECT_ADAPTER",
    "JSON_SCHEMA_DIALECT",
    "JsonObject",
    "PublicToolMeta",
    "PublicStatusProjection",
    "QUERY_TOOL_OUTPUT_ADAPTER",
    "QueryToolInput",
    "QueryToolOutput",
    "REFERENCE_TOOL_OUTPUT_ADAPTER",
    "ReferenceToolOutput",
    "ResourceReference",
    "SafeErrorProjection",
    "StatusToolOutput",
    "StrictWireModel",
    "ValidateQueryOutput",
    "ValidateToolInput",
    "ValidationIssue",
    "WireSchemaMode",
    "WireSchemaName",
    "wire_schema",
    "wire_schema_fingerprints",
]
