"""Modern-only, presentation-only FastMCP server for the daemon v2 contract."""

from __future__ import annotations

import asyncio
import base64
import binascii
import logging
import time
from collections.abc import AsyncIterator, Callable, Mapping
from contextvars import ContextVar
from dataclasses import dataclass
from typing import Annotated, Any, Literal, cast
from urllib.parse import quote, unquote

import fastmcp
from fastmcp import Context, FastMCP
from fastmcp.dependencies import CurrentContext, Depends
from fastmcp.exceptions import FastMCPError, ResourceError, ToolError
from fastmcp.server.lifespan import lifespan
from fastmcp.server.middleware import CallNext, Middleware, MiddlewareContext
from fastmcp.tools import ToolResult
from mcp import MCPError
from mcp.server.request_state import RequestStateSecurity
from mcp.types import (
    Completion,
    CompletionArgument,
    CompletionContext,
    ElicitRequest,
    ElicitRequestFormParams,
    ElicitResult,
    InputRequiredResult,
    PromptReference,
    ResourceTemplateReference,
    TextContent,
    ToolAnnotations,
)
from opentelemetry import trace
from opentelemetry.trace import SpanKind, Tracer
from pydantic import BaseModel, ConfigDict, Field, ValidationError

from .contracts.wire_models import (
    AuthorityProjection,
    InputRequirementProjection,
    JsonObject,
    PublicStatusProjection,
    PublicToolMeta,
    QueryToolInput,
    QueryToolOutput,
    ReferenceToolOutput,
    ResourceReference,
    SafeErrorProjection,
    StatusToolOutput,
    StrictWireModel,
    ValidateQueryOutput,
    ValidateToolInput,
    ValidationIssue,
    WireSchemaName,
    wire_schema,
)
from .daemon import (
    AcceptedQuery,
    AuthorityGeneration,
    BooleanCollectionInputAnswer,
    BooleanInputAnswer,
    ChallengeAnswer,
    ChoiceCollectionInputAnswer,
    ChoiceInputAnswer,
    CpgDaemonClient,
    DaemonPort,
    DaemonProtocolError,
    DaemonRpcError,
    InputChallenge,
    InputRequirement,
    IntegerCollectionInputAnswer,
    IntegerInputAnswer,
    ManifestSelector,
    PageSelector,
    ReferenceSelector,
    ResourceHandle,
    ResourceReadLimits,
    SafeError,
    StringCollectionInputAnswer,
    StringInputAnswer,
    ValidationRejection,
)
from .settings import Settings

SERVER_NAME = "CodeFabric Relational Data Fabric"
SERVER_VERSION = "2.3.0"
MODERN_PROTOCOL_VERSION = "2026-07-28"
RESULT_RESOURCE_TEMPLATE = "cpg://result/{handle}/{selector}/{page_ordinal}"
REFERENCE_RESOURCE_TEMPLATE = "cpg://reference/{handle}/{kind}/{version}"
_MAX_GUARD_STATE_BYTES = 64 * 1024
_BUSINESS_METHODS = frozenset({"tools/call", "resources/read", "completion/complete"})
_OBSERVABLE_METHODS = frozenset(
    {
        "completion/complete",
        "ping",
        "prompts/get",
        "prompts/list",
        "resources/list",
        "resources/read",
        "resources/templates/list",
        "server/discover",
        "tools/call",
        "tools/list",
    }
)
_REQUEST_CORRELATION: ContextVar[str | None] = ContextVar(
    "codefabric_mcp_request_correlation", default=None
)
_CURRENT_CONTEXT = CurrentContext()
_LOGGER = logging.getLogger(__name__)
_TRACER: Tracer = trace.get_tracer("codefabric.fastmcp.presentation", SERVER_VERSION)


def _safe_correlation_id(value: object | None) -> str:
    if isinstance(value, bool) or not isinstance(value, str | int):
        return "unavailable"
    candidate = str(value)
    if (
        0 < len(candidate) <= 128
        and candidate.isascii()
        and all(character.isalnum() or character in "._-:" for character in candidate)
    ):
        return candidate
    return "unavailable"


SERVER_INSTRUCTIONS = """\
CodeFabric exposes bounded factual queries over one daemon-selected immutable workspace epoch.
Validate unfamiliar requests before execution. Query results and references are daemon-owned
resources; this FastMCP process performs presentation validation only.
"""

READ_ONLY = ToolAnnotations(
    title="CodeFabric data fabric",
    read_only_hint=True,
    destructive_hint=False,
    idempotent_hint=True,
    open_world_hint=False,
)

type DaemonFactory = Callable[[Settings], DaemonPort]
type PublicReferenceKind = Literal[
    "capability",
    "guide",
    "recipe",
    "request-schema",
    "response-schema",
    "snapshot",
]


class _GuardState(BaseModel):
    """Plaintext sealed by FastMCP; the daemon continuation remains opaque."""

    model_config = ConfigDict(
        extra="forbid",
        strict=True,
        frozen=True,
        validate_default=True,
        hide_input_in_errors=True,
    )

    format: Literal["codefabric.fastmcp-guard.v1"] = "codefabric.fastmcp-guard.v1"
    authority: AuthorityGeneration
    semantic_request_id: str
    challenge_id: str
    round: int = Field(gt=0)
    remaining_rounds: int = Field(ge=0)
    issued_at_unix_ms: int
    expires_at_unix_ms: int
    sealed_at_unix_ms: int
    seal_expires_at_unix_ms: int
    maximum_answer_bytes: int = Field(gt=0)
    explanation_code: Literal[
        "required_input_missing",
        "reference_ambiguous",
        "bounded_selection_required",
    ]
    requirements: tuple[InputRequirement, ...]
    daemon_continuation_b64: str = Field(repr=False, min_length=2)

    @classmethod
    def from_challenge(
        cls,
        challenge: InputChallenge,
        *,
        sealed_at_unix_ms: int,
        seal_expires_at_unix_ms: int,
    ) -> _GuardState:
        return cls(
            authority=challenge.authority,
            semantic_request_id=challenge.semantic_request_id,
            challenge_id=challenge.challenge_id,
            round=challenge.round,
            remaining_rounds=challenge.remaining_rounds,
            issued_at_unix_ms=challenge.issued_at_unix_ms,
            expires_at_unix_ms=challenge.expires_at_unix_ms,
            sealed_at_unix_ms=sealed_at_unix_ms,
            seal_expires_at_unix_ms=seal_expires_at_unix_ms,
            maximum_answer_bytes=challenge.maximum_answer_bytes,
            explanation_code=challenge.explanation_code,
            requirements=challenge.requirements,
            daemon_continuation_b64=base64.urlsafe_b64encode(challenge.daemon_continuation).decode(
                "ascii"
            ),
        )

    def to_challenge(self) -> InputChallenge:
        try:
            continuation = base64.b64decode(
                self.daemon_continuation_b64,
                altchars=b"-_",
                validate=True,
            )
        except (ValueError, binascii.Error) as error:
            raise ValueError("guard continuation encoding is invalid") from error
        if base64.urlsafe_b64encode(continuation).decode("ascii") != self.daemon_continuation_b64:
            raise ValueError("guard continuation encoding is non-canonical")
        return InputChallenge(
            authority=self.authority,
            semantic_request_id=self.semantic_request_id,
            challenge_id=self.challenge_id,
            round=self.round,
            remaining_rounds=self.remaining_rounds,
            issued_at_unix_ms=self.issued_at_unix_ms,
            expires_at_unix_ms=self.expires_at_unix_ms,
            maximum_answer_bytes=self.maximum_answer_bytes,
            explanation_code=self.explanation_code,
            requirements=self.requirements,
            daemon_continuation=continuation,
        )


@dataclass
class _DaemonSlot:
    """The sole lifespan-owned channel/session reference."""

    initial_settings: Settings
    port: DaemonPort | None = None

    def require(self) -> DaemonPort:
        if self.port is None:
            raise RuntimeError("daemon lifespan is not active")
        return self.port

    def current_settings(self) -> Settings:
        """Resolve limits from the current generation, never a factory-time closure."""

        if self.port is None:
            return self.initial_settings
        return self.port.current_settings()


def _protocol_version(context: MiddlewareContext[Any]) -> str | None:
    fastmcp_context = context.fastmcp_context
    if fastmcp_context is None:
        return None
    request_context = fastmcp_context.request_context
    if request_context is None:
        return None
    return cast(str | None, request_context.protocol_version)


def _unsupported_protocol() -> MCPError:
    return MCPError(
        code=-32600,
        message="Unsupported protocol era",
        data={"code": "unsupported_protocol_era"},
    )


class ModernProtocolPolicyMiddleware(Middleware):
    """Admit only the sole 2026-07-28 product protocol."""

    async def on_initialize(
        self,
        context: MiddlewareContext[Any],
        call_next: CallNext[Any, Any],
    ) -> Any:
        del context, call_next
        raise _unsupported_protocol()

    async def on_discover(
        self,
        context: MiddlewareContext[Any],
        call_next: CallNext[Any, Any],
    ) -> Any:
        # Discovery is the only request admitted before an era is selected.
        # A client that already selected an older era is still rejected.
        if _protocol_version(context) not in {None, MODERN_PROTOCOL_VERSION}:
            raise _unsupported_protocol()
        return await call_next(context)

    async def on_request(
        self,
        context: MiddlewareContext[Any],
        call_next: CallNext[Any, Any],
    ) -> Any:
        if context.method != "server/discover" and (
            _protocol_version(context) != MODERN_PROTOCOL_VERSION
        ):
            raise _unsupported_protocol()
        return await call_next(context)


class CorrelationMiddleware(Middleware):
    """Bind one allowlisted request identifier without retaining request payloads."""

    async def on_request(
        self,
        context: MiddlewareContext[Any],
        call_next: CallNext[Any, Any],
    ) -> Any:
        request_id = _safe_correlation_id(
            context.fastmcp_context.request_id if context.fastmcp_context is not None else None
        )
        token = _REQUEST_CORRELATION.set(request_id)
        try:
            return await call_next(context)
        finally:
            _REQUEST_CORRELATION.reset(token)


class AllowlistedTelemetryMiddleware(Middleware):
    """Create one safe span without FastMCP component keys, URIs, or exception prose."""

    def __init__(self, settings: Callable[[], Settings]) -> None:
        self._settings = settings

    async def on_request(
        self,
        context: MiddlewareContext[Any],
        call_next: CallNext[Any, Any],
    ) -> Any:
        request_id = _safe_correlation_id(
            context.fastmcp_context.request_id if context.fastmcp_context is not None else None
        )
        current = self._settings()
        method = context.method
        operation = (
            method if isinstance(method, str) and method in _OBSERVABLE_METHODS else "unsupported"
        )
        with _TRACER.start_as_current_span(
            "codefabric.mcp.request",
            kind=SpanKind.SERVER,
            attributes={
                "codefabric.mcp.operation": operation,
                "codefabric.mcp.protocol_era": _protocol_version(context) or "unnegotiated",
                "codefabric.mcp.request_id": request_id,
                "codefabric.daemon.generation": current.daemon_generation,
            },
            record_exception=False,
            set_status_on_exception=False,
        ):
            return await call_next(context)


class DeadlineCancellationMiddleware(Middleware):
    """Apply one bounded presentation deadline only to business operations."""

    def __init__(self, timeout_seconds: Callable[[], float]) -> None:
        self._timeout_seconds = timeout_seconds

    async def on_request(
        self,
        context: MiddlewareContext[Any],
        call_next: CallNext[Any, Any],
    ) -> Any:
        if context.method not in _BUSINESS_METHODS:
            return await call_next(context)
        try:
            async with asyncio.timeout(self._timeout_seconds()):
                return await call_next(context)
        except TimeoutError as error:
            raise MCPError(
                code=-32001,
                message="Operation deadline exceeded",
                data={"code": "operation_deadline_exceeded"},
            ) from error


class SafeErrorMiddleware(Middleware):
    """Ensure unexpected Python failures never publish exception prose."""

    async def on_request(
        self,
        context: MiddlewareContext[Any],
        call_next: CallNext[Any, Any],
    ) -> Any:
        try:
            return await call_next(context)
        except MCPError, FastMCPError:
            raise
        except Exception as error:
            _LOGGER.error("redacted internal MCP operation failure")
            raise MCPError(
                code=-32603,
                message="Internal error",
                data={"code": "internal"},
            ) from error


def _output_schema(name: WireSchemaName) -> dict[str, Any]:
    return wire_schema(name, "serialization")


def _correlation_id(ctx: Context | None = None) -> str:
    if ctx is not None:
        return _safe_correlation_id(ctx.request_id)
    return _REQUEST_CORRELATION.get() or "mcp-operation"


def _enrich_current_span(**attributes: str | int) -> None:
    """Attach only explicitly allowlisted, non-payload correlation values."""

    span = trace.get_current_span()
    if span.is_recording():
        for key, value in attributes.items():
            span.set_attribute(
                key, _safe_correlation_id(value) if isinstance(value, str) else value
            )


def _authority(value: AuthorityGeneration) -> AuthorityProjection:
    return AuthorityProjection(**value.model_dump())


def _safe_error(value: SafeError | None) -> SafeErrorProjection | None:
    if value is None:
        return None
    return SafeErrorProjection(
        code=value.code,
        layer=value.layer,
        retryable=value.retryable,
        retry_after_ms=value.retry_after_ms,
        diagnostic_reference=value.diagnostic_reference,
    )


def _validation_issue(value: Any) -> ValidationIssue:
    return ValidationIssue(
        code=value.code,
        semantic_field_id=value.semantic_field_id,
        presentation_key=value.presentation_key,
        retryable=value.retryable,
    )


def _input_requirement(value: InputRequirement) -> InputRequirementProjection:
    constraints = None
    if value.constraints is not None:
        constraints = cast(JsonObject, value.constraints.model_dump(mode="json"))
    choices = tuple(
        cast(JsonObject, choice.model_dump(mode="json")) for choice in value.authorized_choices
    )
    return InputRequirementProjection(
        semantic_field_id=value.semantic_field_id,
        input_kind=value.input_kind,
        presentation_key=value.presentation_key,
        description_key=value.description_key,
        required=value.required,
        constraints=constraints,
        authorized_choices=choices,
    )


def _public_reference_kind(value: str) -> PublicReferenceKind:
    normalized = value.replace("_", "-")
    if normalized not in {
        "capability",
        "guide",
        "recipe",
        "request-schema",
        "response-schema",
        "snapshot",
    }:
        raise ValueError("daemon returned an unknown reference kind")
    return cast(PublicReferenceKind, normalized)


def _daemon_reference_kind(value: PublicReferenceKind) -> Any:
    return value.replace("-", "_")


def _bounded_bytes(value: ResourceHandle, limits: ResourceReadLimits) -> int:
    if value.byte_length <= 0 or value.byte_length > limits.maximum_resource_bytes:
        raise ValueError("daemon resource exceeds the one-call presentation bound")
    return value.byte_length


def _resource_reference(
    value: ResourceHandle,
    limits: ResourceReadLimits,
    *,
    reference_kind: PublicReferenceKind | None = None,
    reference_version: str | None = None,
) -> ResourceReference:
    handle = quote(value.public_handle, safe="")
    _bounded_bytes(value, limits)
    if value.kind in {"result_manifest", "result_page"}:
        selector = "manifest" if value.kind == "result_manifest" else "page"
        page_ordinal = value.page_ordinal or 0
        uri = f"cpg://result/{handle}/{selector}/{page_ordinal}"
    elif value.kind == "reference":
        if reference_kind is None:
            raise ValueError("reference handle omitted its public selector")
        version = quote(reference_version or "current", safe="")
        uri = f"cpg://reference/{handle}/{reference_kind}/{version}"
    else:
        raise ValueError("projection resources are not a public base-catalog family")
    return ResourceReference(
        uri=uri,
        kind=value.kind,
        media_type=value.media_type,
        package_id=value.package_id,
        page_ordinal=value.page_ordinal,
        total_bytes=value.byte_length,
        content_checksum=value.content_checksum,
        expires_at_unix_ms=value.expires_at_unix_ms,
    )


def _tool_result(output: StrictWireModel, meta: PublicToolMeta, summary: str) -> ToolResult:
    return ToolResult(
        content=[TextContent(type="text", text=summary)],
        structured_content=output.model_dump(mode="json", exclude_none=True),
        meta=meta.model_dump(mode="json", exclude_none=True),
    )


def _selection_presentation(key: str) -> str:
    """Format the daemon's closed selection labels; choice IDs remain the submitted values."""
    if not key.startswith("selection."):
        return key
    label = key.removeprefix("selection.").replace("-", " ")
    return label[:1].upper() + label[1:]


def _request_schema(requirement: InputRequirement) -> dict[str, Any]:
    constraints = (
        requirement.constraints.model_dump(mode="python") if requirement.constraints else {}
    )
    kind = requirement.input_kind
    property_schema: dict[str, Any]
    if kind in {"string", "enum"}:
        property_schema = {"type": "string"}
    elif kind == "integer":
        property_schema = {"type": "integer"}
    elif kind == "boolean":
        property_schema = {"type": "boolean"}
    else:
        item_kind = {
            "string_collection": "string",
            "integer_collection": "integer",
            "boolean_collection": "boolean",
            "enum_collection": "string",
        }[kind]
        property_schema = {"type": "array", "items": {"type": item_kind}}

    if constraints.get("kind") == "string":
        if constraints.get("minimum_length") is not None:
            property_schema["minLength"] = constraints["minimum_length"]
        if constraints.get("maximum_length") is not None:
            property_schema["maxLength"] = constraints["maximum_length"]
    elif constraints.get("kind") == "integer":
        if constraints.get("minimum") is not None:
            property_schema["minimum"] = constraints["minimum"]
        if constraints.get("maximum") is not None:
            property_schema["maximum"] = constraints["maximum"]
    elif constraints.get("kind") == "collection":
        property_schema["minItems"] = constraints["minimum_items"]
        property_schema["maxItems"] = constraints["maximum_items"]
        property_schema["uniqueItems"] = constraints["unique_items"]

    if kind in {"enum", "enum_collection"}:
        choice_ids = [choice.choice_id for choice in requirement.authorized_choices]
        presentations = {
            choice.choice_id: _selection_presentation(choice.presentation_key)
            for choice in requirement.authorized_choices
        }
        if kind == "enum":
            property_schema["enum"] = choice_ids
        else:
            property_schema["items"]["enum"] = choice_ids
        property_schema["x-codefabric-choice-presentations"] = presentations

    schema: dict[str, Any] = {
        "type": "object",
        "properties": {
            "value": {
                **property_schema,
                "title": requirement.presentation_key,
            }
        },
        "additionalProperties": False,
    }
    if requirement.description_key:
        schema["properties"]["value"]["description"] = requirement.description_key
    if requirement.required:
        schema["required"] = ["value"]
    return schema


def _input_request(requirement: InputRequirement) -> ElicitRequest:
    return ElicitRequest(
        params=ElicitRequestFormParams(
            message=requirement.description_key or requirement.presentation_key,
            requested_schema=_request_schema(requirement),
        )
    )


def _guard_state(challenge: InputChallenge, settings: Settings) -> str:
    now_ms = int(time.time() * 1000)
    seal_expiry_ms = min(
        now_ms + settings.maximum_request_state_ttl_seconds * 1000,
        challenge.expires_at_unix_ms,
        settings.session_expires_at_unix_ms,
    )
    if seal_expiry_ms <= now_ms:
        raise ToolError("INPUT_WINDOW_TOO_SHORT")
    state = _GuardState.from_challenge(
        challenge,
        sealed_at_unix_ms=now_ms,
        seal_expires_at_unix_ms=seal_expiry_ms,
    ).model_dump_json()
    if len(state.encode("utf-8")) > _MAX_GUARD_STATE_BYTES:
        raise ToolError("INPUT_CHALLENGE_TOO_LARGE")
    return state


def _restore_guard(value: str, settings: Settings) -> InputChallenge:
    try:
        if len(value.encode("utf-8")) > _MAX_GUARD_STATE_BYTES:
            raise ValueError("guard state is oversized")
        state = _GuardState.model_validate_json(value, strict=True)
        now_ms = int(time.time() * 1000)
        if (
            state.sealed_at_unix_ms > now_ms
            or state.seal_expires_at_unix_ms <= now_ms
            or state.expires_at_unix_ms <= now_ms
            or settings.session_expires_at_unix_ms <= now_ms
            or state.seal_expires_at_unix_ms > state.expires_at_unix_ms
            or state.seal_expires_at_unix_ms > settings.session_expires_at_unix_ms
            or state.seal_expires_at_unix_ms - state.sealed_at_unix_ms
            > settings.maximum_request_state_ttl_seconds * 1000
            or state.authority.daemon_generation != settings.daemon_generation
            or state.authority.supervisor_generation != settings.supervisor_generation
        ):
            raise ValueError("guard state is outside current launch authority")
        return state.to_challenge()
    except (ValidationError, ValueError) as error:
        raise ToolError("INVALID_REQUEST_STATE") from error


def _accepted_input_value(
    requirement: InputRequirement,
    responses: Mapping[str, Any],
) -> Any:
    response = responses.get(requirement.semantic_field_id)
    if not isinstance(response, ElicitResult) or response.action != "accept":
        raise ToolError("INPUT_DECLINED")
    content = response.content
    if not isinstance(content, dict) or set(content) != {"value"}:
        raise ToolError("INVALID_INPUT_RESPONSE")
    return content["value"]


def _challenge_answers(
    challenge: InputChallenge,
    responses: Mapping[str, Any] | None,
) -> tuple[ChallengeAnswer, ...]:
    if responses is None:
        raise ToolError("INPUT_RESPONSE_REQUIRED")
    requirement_ids = {item.semantic_field_id for item in challenge.requirements}
    if not set(responses).issubset(requirement_ids):
        raise ToolError("INVALID_INPUT_RESPONSE")
    answers: list[ChallengeAnswer] = []
    for requirement in challenge.requirements:
        if requirement.semantic_field_id not in responses:
            if requirement.required:
                raise ToolError("INPUT_RESPONSE_REQUIRED")
            continue
        value = _accepted_input_value(requirement, responses)
        field_id = requirement.semantic_field_id
        kind = requirement.input_kind
        choices = {choice.choice_id for choice in requirement.authorized_choices}
        if kind == "string" and type(value) is str:
            answer: ChallengeAnswer = StringInputAnswer(semantic_field_id=field_id, value=value)
        elif kind == "integer" and type(value) is int:
            answer = IntegerInputAnswer(semantic_field_id=field_id, value=value)
        elif kind == "boolean" and type(value) is bool:
            answer = BooleanInputAnswer(semantic_field_id=field_id, value=value)
        elif kind == "enum" and type(value) is str and value in choices:
            answer = ChoiceInputAnswer(semantic_field_id=field_id, choice_id=value)
        elif kind.endswith("_collection") and isinstance(value, list):
            if kind == "string_collection" and all(type(item) is str for item in value):
                answer = StringCollectionInputAnswer(
                    semantic_field_id=field_id, values=tuple(value)
                )
            elif kind == "integer_collection" and all(type(item) is int for item in value):
                answer = IntegerCollectionInputAnswer(
                    semantic_field_id=field_id, values=tuple(value)
                )
            elif kind == "boolean_collection" and all(type(item) is bool for item in value):
                answer = BooleanCollectionInputAnswer(
                    semantic_field_id=field_id, values=tuple(value)
                )
            elif kind == "enum_collection" and all(
                type(item) is str and item in choices for item in value
            ):
                answer = ChoiceCollectionInputAnswer(
                    semantic_field_id=field_id, choice_ids=tuple(value)
                )
            else:
                raise ToolError("INVALID_INPUT_RESPONSE")
        else:
            raise ToolError("INVALID_INPUT_RESPONSE")
        answers.append(answer)
    return tuple(answers)


async def _cancel_accepted_query(
    port: DaemonPort,
    accepted: AcceptedQuery,
    settings: Settings,
    correlation_id: str,
) -> None:
    cleanup = asyncio.create_task(
        port.cancel_query(
            accepted.daemon_query_id,
            cancellation_id=f"cancel:{accepted.daemon_query_id}",
            correlation_id=correlation_id,
            timeout_seconds=settings.cancellation_cleanup_timeout_seconds,
        )
    )
    try:
        result = await asyncio.wait_for(
            asyncio.shield(cleanup),
            timeout=settings.cancellation_cleanup_timeout_seconds,
        )
        _LOGGER.info("daemon cancellation cleanup outcome=%s", result.acknowledgement)
    except TimeoutError, DaemonRpcError, DaemonProtocolError:
        cleanup.cancel()
        _LOGGER.warning("daemon cancellation cleanup did not acknowledge")


async def _read_and_release_resource(
    port: DaemonPort,
    public_handle: str,
    selector: ManifestSelector | PageSelector | ReferenceSelector,
    correlation_id: str,
) -> bytes:
    """Release one resource-scoped handle only after its full bounded read succeeds."""

    limits = port.current_resource_limits()
    settings = port.current_settings()
    content = await port.read_resource(
        public_handle,
        selector,
        offset=0,
        maximum_bytes=limits.maximum_chunk_bytes,
        correlation_id=correlation_id,
    )
    await port.release_resource(
        public_handle,
        release_id=f"release:{public_handle}",
        correlation_id=correlation_id,
        timeout_seconds=settings.cancellation_cleanup_timeout_seconds,
    )
    return content


def create_server(
    settings: Settings,
    daemon_factory: DaemonFactory = CpgDaemonClient,
) -> FastMCP[Any]:
    """Build one modern server after strict launch authority has been verified."""

    if settings.session_expires_at_unix_ms <= int(time.time() * 1000):
        raise RuntimeError("adapter launch session authority is expired")

    # Native FastMCP spans attach resource URIs, which contain daemon-minted
    # public handles. Preserve trace propagation but let this adapter's
    # allowlisted middleware own the observable request fields.
    fastmcp.settings.telemetry_mode = "propagation_only"
    fastmcp.settings.mcp_camelcase_compat = False
    # FastMCP's component logger includes resource URIs and exception tracebacks.
    # The dedicated adapter process suppresses that non-redactable channel;
    # CodeFabric emits only its fixed safe diagnostics and OTel attributes.
    framework_log_floor = logging.CRITICAL + 1
    logging.getLogger("fastmcp").setLevel(framework_log_floor)
    logging.getLogger("mcp").setLevel(framework_log_floor)

    slot = _DaemonSlot(initial_settings=settings)

    @lifespan
    async def server_lifespan(_server: FastMCP[Any]) -> AsyncIterator[dict[str, Any]]:
        port = daemon_factory(settings)
        slot.port = port
        try:
            await port.connect(correlation_id="adapter-connect")
            yield {"daemon_port": port}
        finally:
            slot.port = None
            await port.close()

    server = FastMCP(
        name=SERVER_NAME,
        version=SERVER_VERSION,
        instructions=SERVER_INSTRUCTIONS,
        lifespan=server_lifespan,
        middleware=[
            ModernProtocolPolicyMiddleware(),
            CorrelationMiddleware(),
            AllowlistedTelemetryMiddleware(slot.current_settings),
            DeadlineCancellationMiddleware(lambda: slot.current_settings().query_timeout_seconds),
            SafeErrorMiddleware(),
        ],
        on_duplicate="error",
        strict_input_validation=True,
        mask_error_details=True,
        list_page_size=50,
        request_state_security=RequestStateSecurity.ephemeral(
            ttl=60.0,
            audience=SERVER_NAME,
        ),
        tasks=False,
    )

    def daemon_port(ctx: Context = _CURRENT_CONTEXT) -> DaemonPort:
        port = cast(DaemonPort | None, ctx.lifespan_context.get("daemon_port"))
        if port is None or port is not slot.port:
            raise RuntimeError("daemon lifespan dependency is unavailable")
        return port

    port_dependency = Depends(daemon_port)

    @server.tool(
        name="query_code_graph",
        version="2.3",
        description="Atomically accept one bounded factual query or request typed input.",
        tags={"cpg", "facts", "read", "primary"},
        annotations=READ_ONLY,
        output_schema=_output_schema(WireSchemaName.QUERY_TOOL_OUTPUT),
    )
    async def query_code_graph(
        request: Annotated[dict[str, Any], Field(description="Complete semantic request.")],
        delivery: Literal["automatic", "inline", "resource"] = "automatic",
        ctx: Context = _CURRENT_CONTEXT,
        port: DaemonPort = port_dependency,
    ) -> ToolResult | InputRequiredResult:
        correlation_id = _correlation_id(ctx)

        async def progress(completed: int, total: int | None, stage: str) -> None:
            await ctx.report_progress(progress=completed, total=total, message=stage)

        try:
            if ctx.request_state is None:
                if ctx.input_responses:
                    raise ToolError("INVALID_INPUT_RESPONSE")
                outcome = await port.start_query(
                    QueryToolInput(request=request, delivery=delivery),
                    correlation_id=correlation_id,
                )
            else:
                challenge = _restore_guard(ctx.request_state, port.current_settings())
                outcome = await port.continue_query(
                    challenge,
                    _challenge_answers(challenge, ctx.input_responses),
                    correlation_id=correlation_id,
                )

            if isinstance(outcome, InputChallenge):
                _enrich_current_span(
                    **{
                        "codefabric.challenge.id": outcome.challenge_id,
                        "codefabric.semantic_request.id": outcome.semantic_request_id,
                    }
                )
                return InputRequiredResult(
                    input_requests={
                        requirement.semantic_field_id: _input_request(requirement)
                        for requirement in outcome.requirements
                    },
                    request_state=_guard_state(outcome, port.current_settings()),
                )

            if isinstance(outcome, ValidationRejection):
                error = _safe_error(outcome.error)
                assert error is not None
                output = QueryToolOutput(
                    outcome="validation_rejection",
                    semantic_request_id=outcome.semantic_request_id,
                    issues=tuple(_validation_issue(issue) for issue in outcome.issues),
                    error=error,
                )
                return _tool_result(
                    output,
                    PublicToolMeta(semantic_request_id=outcome.semantic_request_id),
                    "The daemon rejected the query before acceptance.",
                )

            accepted = outcome
            _enrich_current_span(
                **{
                    "codefabric.query.id": accepted.daemon_query_id,
                    "codefabric.semantic_request.id": accepted.semantic_request_id,
                }
            )
            try:
                active_settings = port.current_settings()
                result = await port.watch_query(
                    accepted,
                    correlation_id=correlation_id,
                    progress=progress,
                    timeout_seconds=active_settings.query_timeout_seconds,
                )
            except asyncio.CancelledError:
                await _cancel_accepted_query(
                    port,
                    accepted,
                    port.current_settings(),
                    correlation_id,
                )
                raise

            manifest = None
            if result.manifest is not None:
                manifest = _resource_reference(
                    result.manifest,
                    port.current_resource_limits(),
                )
            pages = tuple(
                _resource_reference(page, port.current_resource_limits()) for page in result.pages
            )
            output = QueryToolOutput(
                outcome="accepted",
                daemon_query_id=result.daemon_query_id,
                semantic_request_id=result.semantic_request_id,
                execution_state=cast(Any, result.execution_state),
                epoch_id=result.epoch_id,
                source_generation=result.source_generation,
                freshness=result.freshness,
                analysis_context_set_id=result.analysis_context_set_id,
                processing=result.processing,
                package_id=result.package_id,
                manifest=manifest,
                pages=pages,
                total_rows=result.total_rows,
                total_pages=result.total_pages,
                total_bytes=result.total_bytes,
                error=_safe_error(result.error),
                notices=result.notices,
            )
            return _tool_result(
                output,
                PublicToolMeta(
                    semantic_request_id=result.semantic_request_id,
                    daemon_query_id=result.daemon_query_id,
                    epoch_id=result.epoch_id,
                    package_id=result.package_id,
                ),
                f"Query reached terminal state {result.execution_state}.",
            )
        except DaemonRpcError as error:
            raise ToolError(f"{error.status.name}:{error.error.code}") from None
        except DaemonProtocolError:
            raise ToolError("DAEMON_PROTOCOL_ERROR") from None
        except ToolError:
            raise
        except Exception:
            _LOGGER.error("redacted internal query presentation failure")
            raise ToolError("INTERNAL") from None

    @server.tool(
        name="validate_code_graph_query",
        version="2.3",
        description="Purely validate and prepare a factual query without accepting work.",
        tags={"cpg", "validate", "read"},
        annotations=READ_ONLY,
        output_schema=_output_schema(WireSchemaName.VALIDATE_QUERY_OUTPUT),
    )
    async def validate_code_graph_query(
        request: Annotated[dict[str, Any], Field(description="Complete semantic request.")],
        ctx: Context = _CURRENT_CONTEXT,
        port: DaemonPort = port_dependency,
    ) -> ValidateQueryOutput:
        try:
            result = await port.validate(
                ValidateToolInput(request=request), correlation_id=_correlation_id(ctx)
            )
            return ValidateQueryOutput(
                valid=result.valid,
                semantic_request_id=result.semantic_request_id,
                normalized_request=result.normalized_request,
                input_requirements=tuple(
                    _input_requirement(item) for item in result.input_requirements
                ),
                errors=tuple(_validation_issue(item) for item in result.errors),
                warnings=tuple(_validation_issue(item) for item in result.warnings),
                cost_class=result.cost_class,
                estimated_result_bytes=result.estimated_result_bytes,
                estimated_result_pages=result.estimated_result_pages,
            )
        except DaemonRpcError as error:
            raise ToolError(f"{error.status.name}:{error.error.code}") from None
        except DaemonProtocolError:
            raise ToolError("DAEMON_PROTOCOL_ERROR") from None
        except ToolError:
            raise
        except Exception:
            _LOGGER.error("redacted internal validation presentation failure")
            raise ToolError("INTERNAL") from None

    @server.tool(
        name="get_code_graph_status",
        version="2.3",
        description="Read the daemon lifecycle and active exact epoch projection.",
        tags={"cpg", "status", "read"},
        annotations=READ_ONLY,
        output_schema=_output_schema(WireSchemaName.STATUS_TOOL_OUTPUT),
    )
    async def get_code_graph_status(
        ctx: Context = _CURRENT_CONTEXT,
        port: DaemonPort = port_dependency,
    ) -> StatusToolOutput:
        try:
            result = await port.status(correlation_id=_correlation_id(ctx))
            return StatusToolOutput(
                authority=_authority(result.authority),
                lifecycle=result.lifecycle,
                lifecycle_sequence=result.lifecycle_sequence,
                active_epoch_id=result.active_epoch_id,
                running_queries=result.running_queries,
                queued_queries=result.queued_queries,
                failure=_safe_error(result.failure),
                public_status=PublicStatusProjection(**result.public_status.model_dump()),
                source_observations=result.source_observations,
            )
        except DaemonRpcError as error:
            raise ToolError(f"{error.status.name}:{error.error.code}") from None
        except DaemonProtocolError:
            raise ToolError("DAEMON_PROTOCOL_ERROR") from None
        except ToolError:
            raise
        except Exception:
            _LOGGER.error("redacted internal status presentation failure")
            raise ToolError("INTERNAL") from None

    @server.tool(
        name="get_code_graph_reference",
        version="2.3",
        description="Mint one authorized daemon reference resource.",
        tags={"cpg", "reference", "read"},
        annotations=READ_ONLY,
        output_schema=_output_schema(WireSchemaName.REFERENCE_TOOL_OUTPUT),
    )
    async def get_code_graph_reference(
        kind: PublicReferenceKind,
        version: Annotated[
            str | None,
            Field(pattern=r"^[0-9]+[.][0-9]+[.][0-9]+$"),
        ] = None,
        ctx: Context = _CURRENT_CONTEXT,
        port: DaemonPort = port_dependency,
    ) -> ReferenceToolOutput:
        try:
            result = await port.reference(
                _daemon_reference_kind(kind),
                version,
                correlation_id=_correlation_id(ctx),
            )
            resource = _resource_reference(
                result.resource,
                port.current_resource_limits(),
                reference_kind=kind,
                reference_version=version,
            )
            return ReferenceToolOutput(reference_id=result.reference_id, resource=resource)
        except DaemonRpcError as error:
            raise ToolError(f"{error.status.name}:{error.error.code}") from None
        except DaemonProtocolError, ValueError:
            raise ToolError("DAEMON_PROTOCOL_ERROR") from None
        except ToolError:
            raise
        except Exception:
            _LOGGER.error("redacted internal reference presentation failure")
            raise ToolError("INTERNAL") from None

    @server.resource(
        RESULT_RESOURCE_TEMPLATE,
        name="codefabric-result",
        description="Read one bounded daemon result manifest or page.",
        mime_type="application/octet-stream",
    )
    async def get_result_resource(
        handle: str,
        selector: Literal["manifest", "page"],
        page_ordinal: int,
        ctx: Context = _CURRENT_CONTEXT,
        port: DaemonPort = port_dependency,
    ) -> bytes:
        if page_ordinal < 0 or (selector == "manifest" and page_ordinal != 0):
            raise ResourceError("INVALID_RESOURCE_SELECTOR")
        public_handle = unquote(handle)
        resource_selector = (
            ManifestSelector()
            if selector == "manifest"
            else PageSelector(page_ordinal=page_ordinal)
        )
        correlation_id = _correlation_id(ctx)
        try:
            return await _read_and_release_resource(
                port,
                public_handle,
                resource_selector,
                correlation_id,
            )
        except DaemonRpcError as error:
            raise ResourceError(f"{error.status.name}:{error.error.code}") from None
        except DaemonProtocolError, ValueError:
            raise ResourceError("DAEMON_PROTOCOL_ERROR") from None
        except ResourceError:
            raise
        except Exception:
            _LOGGER.error("redacted internal result resource failure")
            raise ResourceError("INTERNAL") from None

    @server.resource(
        REFERENCE_RESOURCE_TEMPLATE,
        name="codefabric-reference",
        description="Read one bounded authorized daemon reference projection.",
        mime_type="application/octet-stream",
    )
    async def get_reference_resource(
        handle: str,
        kind: PublicReferenceKind,
        version: str,
        ctx: Context = _CURRENT_CONTEXT,
        port: DaemonPort = port_dependency,
    ) -> bytes:
        public_handle = unquote(handle)
        version_value = None if version == "current" else unquote(version)
        correlation_id = _correlation_id(ctx)
        try:
            return await _read_and_release_resource(
                port,
                public_handle,
                ReferenceSelector(
                    reference_kind=_daemon_reference_kind(kind),
                    version=version_value,
                ),
                correlation_id,
            )
        except DaemonRpcError as error:
            raise ResourceError(f"{error.status.name}:{error.error.code}") from None
        except DaemonProtocolError, ValueError:
            raise ResourceError("DAEMON_PROTOCOL_ERROR") from None
        except ResourceError:
            raise
        except Exception:
            _LOGGER.error("redacted internal reference resource failure")
            raise ResourceError("INTERNAL") from None

    @server.completion
    async def authorized_reference_selector(
        reference: PromptReference | ResourceTemplateReference,
        argument: CompletionArgument,
        context: CompletionContext | None,
    ) -> Completion | None:
        if not isinstance(reference, ResourceTemplateReference):
            return None
        if reference.uri != REFERENCE_RESOURCE_TEMPLATE or argument.name not in {"kind", "version"}:
            return None
        arguments = context.arguments if context is not None and context.arguments else {}
        public_kind = arguments.get("kind")
        daemon_kind = None
        if isinstance(public_kind, str):
            try:
                daemon_kind = _daemon_reference_kind(cast(PublicReferenceKind, public_kind))
            except ValueError:
                return Completion(values=[], total=0, has_more=False)
        try:
            result = await slot.require().complete_reference(
                variable="kind" if argument.name == "kind" else "released_version",
                prefix=argument.value,
                kind=daemon_kind,
                selector=None,
                maximum_candidates=100,
                correlation_id=_correlation_id(),
            )
            values = [candidate.value for candidate in result.candidates[:100]]
            if argument.name == "kind":
                values = [_public_reference_kind(value) for value in values]
            return Completion(values=values, total=result.total, has_more=result.has_more)
        except DaemonRpcError as error:
            raise MCPError(
                code=-32003,
                message="Completion unavailable",
                data={"code": error.error.code},
            ) from None
        except DaemonProtocolError:
            raise MCPError(
                code=-32603,
                message="Internal error",
                data={"code": "daemon_protocol_error"},
            ) from None
        except MCPError:
            raise
        except Exception:
            _LOGGER.error("redacted internal completion presentation failure")
            raise MCPError(
                code=-32603,
                message="Internal error",
                data={"code": "internal"},
            ) from None

    return server


__all__ = [
    "MODERN_PROTOCOL_VERSION",
    "REFERENCE_RESOURCE_TEMPLATE",
    "RESULT_RESOURCE_TEMPLATE",
    "SERVER_NAME",
    "create_server",
]
