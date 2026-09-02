"""One eager grpc.aio v2 session over the supervisor-selected Unix socket."""

from __future__ import annotations

import asyncio
from collections.abc import Awaitable, Callable, Iterable
from importlib.metadata import version
from typing import Annotated, Literal, Protocol, assert_never, cast

import grpc
from google.protobuf.duration_pb2 import Duration
from pydantic import (
    BaseModel,
    ConfigDict,
    Field,
    StringConstraints,
    TypeAdapter,
    ValidationError,
    model_validator,
)

from ..contracts.json import canonicalize_json, canonicalize_value, checksum
from ..contracts.wire_models import (
    JSON_OBJECT_ADAPTER,
    JsonObject,
    QueryToolInput,
    ValidateToolInput,
)
from ..settings import Settings, next_settings
from .channel import create_local_channel
from .generated import cpg_query_service_pb2 as query_pb
from .generated import cpg_query_service_pb2_grpc as query_grpc

SESSION_METADATA_KEY = "codefabric-session-bin"
SEMANTIC_PROFILE = "codefabric.semantic-query.v2"
RPC_MINOR = 0

type ProgressStage = Literal["executing"]
ProgressCallback = Callable[[int, int | None, ProgressStage], Awaitable[None]]
NonEmptyString = Annotated[str, StringConstraints(min_length=1, max_length=512)]
NonNegativeInt = Annotated[int, Field(ge=0)]
PositiveInt = Annotated[int, Field(gt=0)]


def _safe_contract_key(value: str, maximum_length: int = 128) -> bool:
    return (
        0 < len(value) <= maximum_length
        and value.isascii()
        and all(character.isalnum() or character in "._-:" for character in value)
    )


class _PortModel(BaseModel):
    """Strict immutable DTO exported by the application-owned daemon port."""

    model_config = ConfigDict(
        extra="forbid",
        strict=True,
        frozen=True,
        validate_default=True,
        hide_input_in_errors=True,
        allow_inf_nan=False,
    )


type SafeErrorCode = Literal[
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
type SafeErrorLayer = Literal[
    "TRANSPORT",
    "AUTHORIZATION",
    "VALIDATION",
    "QUERY",
    "RESOURCE",
    "LIFECYCLE",
]
type QueryState = Literal[
    "ACCEPTED",
    "QUEUED",
    "RUNNING",
    "SUCCEEDED",
    "FAILED",
    "CANCELLED",
    "LOST",
]
type ReferenceKind = Literal[
    "capability",
    "guide",
    "recipe",
    "request_schema",
    "response_schema",
    "snapshot",
]
type DiagnosticReference = Literal[
    "",
    "lifecycle.failed_closed",
    "query.challenge_rejected",
    "query.terminal",
]

_SAFE_ERROR_CODE_ADAPTER = TypeAdapter(SafeErrorCode)
_SAFE_ERROR_LAYER_ADAPTER = TypeAdapter(SafeErrorLayer)
_QUERY_STATE_ADAPTER = TypeAdapter(QueryState)
_REFERENCE_KIND_ADAPTER = TypeAdapter(ReferenceKind)
_PROGRESS_STAGE_ADAPTER = TypeAdapter(ProgressStage)
_DIAGNOSTIC_REFERENCE_ADAPTER = TypeAdapter(DiagnosticReference)


class AuthorityGeneration(_PortModel):
    session_id: NonEmptyString
    session_generation: PositiveInt
    daemon_generation: PositiveInt
    supervisor_generation: PositiveInt
    policy_generation: NonNegativeInt
    revocation_generation: NonNegativeInt


class SafeError(_PortModel):
    code: SafeErrorCode
    layer: SafeErrorLayer
    retryable: bool
    retry_after_ms: NonNegativeInt | None = None
    diagnostic_reference: DiagnosticReference = ""
    correlation_id: str = ""


class DaemonProtocolError(RuntimeError):
    """The daemon violated a typed v2 transport or presentation invariant."""


class DaemonRpcError(RuntimeError):
    """Stable typed daemon failure without server prose or secret material."""

    def __init__(self, status: grpc.StatusCode, error: SafeError) -> None:
        self.status = status
        self.error = error
        super().__init__(f"{status.name}:{error.code}")


class AuthorizedStringChoice(_PortModel):
    kind: Literal["string"] = "string"
    choice_id: NonEmptyString
    presentation_key: NonEmptyString
    value: str


class AuthorizedIntegerChoice(_PortModel):
    kind: Literal["integer"] = "integer"
    choice_id: NonEmptyString
    presentation_key: NonEmptyString
    value: int


class AuthorizedBooleanChoice(_PortModel):
    kind: Literal["boolean"] = "boolean"
    choice_id: NonEmptyString
    presentation_key: NonEmptyString
    value: bool


type AuthorizedChoice = Annotated[
    AuthorizedStringChoice | AuthorizedIntegerChoice | AuthorizedBooleanChoice,
    Field(discriminator="kind"),
]


class StringChallengeConstraints(_PortModel):
    kind: Literal["string"] = "string"
    minimum_length: NonNegativeInt | None = None
    maximum_length: NonNegativeInt | None = None
    format: Literal["plain", "identifier", "release_version"]

    @model_validator(mode="after")
    def ordered_bounds(self) -> StringChallengeConstraints:
        if (
            self.minimum_length is not None
            and self.maximum_length is not None
            and self.minimum_length > self.maximum_length
        ):
            raise ValueError("string challenge bounds are reversed")
        return self


class IntegerChallengeConstraints(_PortModel):
    kind: Literal["integer"] = "integer"
    minimum: int | None = None
    maximum: int | None = None

    @model_validator(mode="after")
    def ordered_bounds(self) -> IntegerChallengeConstraints:
        if self.minimum is not None and self.maximum is not None and self.minimum > self.maximum:
            raise ValueError("integer challenge bounds are reversed")
        return self


class EnumChallengeConstraints(_PortModel):
    kind: Literal["enum"] = "enum"
    minimum_selections: NonNegativeInt
    maximum_selections: NonNegativeInt

    @model_validator(mode="after")
    def closed_single_selection(self) -> EnumChallengeConstraints:
        if self.minimum_selections > self.maximum_selections or self.maximum_selections != 1:
            raise ValueError("enum challenge must be a bounded single selection")
        return self


class CollectionChallengeConstraints(_PortModel):
    kind: Literal["collection"] = "collection"
    item_kind: Literal["string", "integer", "boolean", "enum"]
    minimum_items: NonNegativeInt
    maximum_items: NonNegativeInt
    unique_items: bool

    @model_validator(mode="after")
    def ordered_bounded_items(self) -> CollectionChallengeConstraints:
        if self.minimum_items > self.maximum_items or self.maximum_items > 256:
            raise ValueError("collection challenge bounds are invalid")
        return self


type ChallengeConstraints = Annotated[
    StringChallengeConstraints
    | IntegerChallengeConstraints
    | EnumChallengeConstraints
    | CollectionChallengeConstraints,
    Field(discriminator="kind"),
]


type ChallengeInputKind = Literal[
    "string",
    "integer",
    "boolean",
    "enum",
    "string_collection",
    "integer_collection",
    "boolean_collection",
    "enum_collection",
]


class InputRequirement(_PortModel):
    semantic_field_id: NonEmptyString
    input_kind: ChallengeInputKind
    presentation_key: NonEmptyString
    description_key: str | None = None
    required: bool
    constraints: ChallengeConstraints | None = None
    authorized_choices: tuple[AuthorizedChoice, ...] = ()

    @model_validator(mode="after")
    def closed_requirement(self) -> InputRequirement:
        keys = (self.semantic_field_id, self.presentation_key)
        if not all(_safe_contract_key(value) for value in keys) or (
            self.description_key is not None and not _safe_contract_key(self.description_key)
        ):
            raise ValueError("challenge requirement keys are not safe contract identifiers")

        constraint_matches = (
            (
                self.input_kind == "string"
                and isinstance(self.constraints, StringChallengeConstraints)
            )
            or (
                self.input_kind == "integer"
                and isinstance(self.constraints, IntegerChallengeConstraints)
            )
            or (self.input_kind == "boolean" and self.constraints is None)
            or (
                self.input_kind == "enum" and isinstance(self.constraints, EnumChallengeConstraints)
            )
            or (
                self.input_kind.endswith("_collection")
                and isinstance(self.constraints, CollectionChallengeConstraints)
                and self.constraints.item_kind == self.input_kind.removesuffix("_collection")
            )
        )
        choices_required = self.input_kind in {"enum", "enum_collection"}
        if not constraint_matches or choices_required != bool(self.authorized_choices):
            raise ValueError("challenge input kind, constraints, and choices disagree")

        choice_ids: set[str] = set()
        for choice in self.authorized_choices:
            if (
                not _safe_contract_key(choice.choice_id)
                or not _safe_contract_key(choice.presentation_key)
                or choice.choice_id in choice_ids
            ):
                raise ValueError("challenge choices have unsafe or duplicate identifiers")
            choice_ids.add(choice.choice_id)
        return self


class StringInputAnswer(_PortModel):
    kind: Literal["string"] = "string"
    semantic_field_id: NonEmptyString
    value: str


class IntegerInputAnswer(_PortModel):
    kind: Literal["integer"] = "integer"
    semantic_field_id: NonEmptyString
    value: int


class BooleanInputAnswer(_PortModel):
    kind: Literal["boolean"] = "boolean"
    semantic_field_id: NonEmptyString
    value: bool


class ChoiceInputAnswer(_PortModel):
    kind: Literal["choice"] = "choice"
    semantic_field_id: NonEmptyString
    choice_id: NonEmptyString


class StringCollectionInputAnswer(_PortModel):
    kind: Literal["string_collection"] = "string_collection"
    semantic_field_id: NonEmptyString
    values: tuple[str, ...]


class IntegerCollectionInputAnswer(_PortModel):
    kind: Literal["integer_collection"] = "integer_collection"
    semantic_field_id: NonEmptyString
    values: tuple[int, ...]


class BooleanCollectionInputAnswer(_PortModel):
    kind: Literal["boolean_collection"] = "boolean_collection"
    semantic_field_id: NonEmptyString
    values: tuple[bool, ...]


class ChoiceCollectionInputAnswer(_PortModel):
    kind: Literal["choice_collection"] = "choice_collection"
    semantic_field_id: NonEmptyString
    choice_ids: tuple[NonEmptyString, ...]


type ChallengeAnswer = Annotated[
    StringInputAnswer
    | IntegerInputAnswer
    | BooleanInputAnswer
    | ChoiceInputAnswer
    | StringCollectionInputAnswer
    | IntegerCollectionInputAnswer
    | BooleanCollectionInputAnswer
    | ChoiceCollectionInputAnswer,
    Field(discriminator="kind"),
]


class ValidationIssue(_PortModel):
    code: SafeErrorCode
    semantic_field_id: str = ""
    presentation_key: str = ""
    retryable: bool


class QueryPreparation(_PortModel):
    authority: AuthorityGeneration
    valid: bool
    semantic_request_id: str | None = None
    normalized_request: JsonObject | None = None
    input_requirements: tuple[InputRequirement, ...] = ()
    errors: tuple[ValidationIssue, ...] = ()
    warnings: tuple[ValidationIssue, ...] = ()
    cost_class: str
    estimated_result_bytes: NonNegativeInt
    estimated_result_pages: NonNegativeInt


class AcceptedQuery(_PortModel):
    outcome: Literal["accepted"] = "accepted"
    authority: AuthorityGeneration
    daemon_query_id: NonEmptyString
    semantic_request_id: NonEmptyString
    operation_fingerprint: NonEmptyString
    accepted_at_unix_ms: int
    observation_expires_at_unix_ms: int
    state: QueryState
    idempotent_replay: bool


class InputChallenge(_PortModel):
    outcome: Literal["input_challenge"] = "input_challenge"
    authority: AuthorityGeneration
    semantic_request_id: NonEmptyString
    challenge_id: NonEmptyString
    round: PositiveInt
    remaining_rounds: NonNegativeInt
    issued_at_unix_ms: int
    expires_at_unix_ms: int
    maximum_answer_bytes: PositiveInt
    explanation_code: Literal[
        "required_input_missing",
        "reference_ambiguous",
        "bounded_selection_required",
    ]
    requirements: tuple[InputRequirement, ...]
    daemon_continuation: bytes = Field(repr=False, min_length=1)

    @model_validator(mode="after")
    def closed_challenge(self) -> InputChallenge:
        field_ids = [requirement.semantic_field_id for requirement in self.requirements]
        if not field_ids or len(field_ids) != len(set(field_ids)):
            raise ValueError("challenge requirements are empty or duplicate a semantic field")
        if self.issued_at_unix_ms >= self.expires_at_unix_ms:
            raise ValueError("challenge validity interval is empty or reversed")
        return self


class ValidationRejection(_PortModel):
    outcome: Literal["validation_rejection"] = "validation_rejection"
    authority: AuthorityGeneration
    semantic_request_id: str | None = None
    issues: tuple[ValidationIssue, ...]
    error: SafeError


type StartQueryOutcome = Annotated[
    AcceptedQuery | InputChallenge | ValidationRejection,
    Field(discriminator="outcome"),
]


class ResourceHandle(_PortModel):
    kind: Literal["result_manifest", "result_page", "reference"]
    public_handle: NonEmptyString
    package_id: str | None = None
    page_ordinal: NonNegativeInt | None = None
    media_type: NonEmptyString
    byte_length: NonNegativeInt
    content_checksum: str
    expires_at_unix_ms: int
    authority: AuthorityGeneration


class ResourceReadLimits(_PortModel):
    """Negotiated per-chunk and total-unit bounds for presentation reads."""

    maximum_chunk_bytes: PositiveInt
    maximum_resource_bytes: PositiveInt


class ManifestSelector(_PortModel):
    kind: Literal["manifest"] = "manifest"


class PageSelector(_PortModel):
    kind: Literal["page"] = "page"
    page_ordinal: NonNegativeInt


class ReferenceSelector(_PortModel):
    kind: Literal["reference"] = "reference"
    reference_kind: ReferenceKind
    version: str | None = None


type ResourceSelector = Annotated[
    ManifestSelector | PageSelector | ReferenceSelector,
    Field(discriminator="kind"),
]


class ReferenceDocument(_PortModel):
    authority: AuthorityGeneration
    reference_id: NonEmptyString
    resource: ResourceHandle


class ReferenceCompletionCandidate(_PortModel):
    value: NonEmptyString
    presentation_key: NonEmptyString


class ReferenceCompletion(_PortModel):
    authority: AuthorityGeneration
    candidates: tuple[ReferenceCompletionCandidate, ...]
    total: NonNegativeInt
    has_more: bool


class PublicDaemonStatus(_PortModel):
    """Closed status document accepted from the daemon's canonical JSON projection."""

    lifecycle: Literal["BOOTSTRAPPING", "READY", "DRAINING", "FAILED_CLOSED"]
    lifecycle_sequence: NonNegativeInt
    active_epoch_id: str | None = None
    running_queries: NonNegativeInt
    queued_queries: NonNegativeInt
    accepted_queries: NonNegativeInt
    reserved_result_bytes: NonNegativeInt
    reserved_result_pages: NonNegativeInt


class DaemonStatus(_PortModel):
    authority: AuthorityGeneration
    lifecycle: Literal["BOOTSTRAPPING", "READY", "DRAINING", "FAILED_CLOSED"]
    lifecycle_sequence: NonNegativeInt
    failure: SafeError | None = None
    active_epoch_id: str | None = None
    running_queries: NonNegativeInt
    queued_queries: NonNegativeInt
    public_status: PublicDaemonStatus


class DaemonQueryResult(_PortModel):
    authority: AuthorityGeneration
    semantic_request_id: NonEmptyString
    daemon_query_id: NonEmptyString
    execution_state: QueryState
    epoch_id: str | None
    package_id: str | None
    manifest: ResourceHandle | None
    pages: tuple[ResourceHandle, ...]
    total_rows: NonNegativeInt
    total_pages: NonNegativeInt
    total_bytes: NonNegativeInt
    error: SafeError | None = None
    notices: tuple[str, ...]


class CancellationResult(_PortModel):
    authority: AuthorityGeneration
    cancellation_id: NonEmptyString
    acknowledgement: Literal["accepted", "replayed", "already_terminal", "query_not_found"]
    terminal_state: QueryState | None = None
    terminal_error: SafeError | None = None
    idempotent_replay: bool


class ReleaseResult(_PortModel):
    authority: AuthorityGeneration
    release_id: NonEmptyString
    state: Literal["released", "already_released", "not_found"]
    idempotent_replay: bool


class DaemonPort(Protocol):
    def current_settings(self) -> Settings: ...

    def current_resource_limits(self) -> ResourceReadLimits: ...

    async def connect(self, *, correlation_id: str = "adapter-connect") -> None: ...

    async def status(self, *, correlation_id: str) -> DaemonStatus: ...

    async def reference(
        self,
        kind: ReferenceKind,
        version_value: str | None,
        *,
        correlation_id: str,
    ) -> ReferenceDocument: ...

    async def complete_reference(
        self,
        *,
        variable: Literal["kind", "released_version"],
        prefix: str,
        kind: ReferenceKind | None,
        selector: str | None,
        maximum_candidates: int,
        correlation_id: str,
    ) -> ReferenceCompletion: ...

    async def validate(
        self, value: ValidateToolInput, *, correlation_id: str
    ) -> QueryPreparation: ...

    async def start_query(
        self,
        value: QueryToolInput,
        *,
        correlation_id: str,
    ) -> StartQueryOutcome: ...

    async def continue_query(
        self,
        challenge: InputChallenge,
        answers: tuple[ChallengeAnswer, ...],
        *,
        correlation_id: str,
    ) -> StartQueryOutcome: ...

    async def watch_query(
        self,
        accepted: AcceptedQuery,
        *,
        correlation_id: str,
        progress: ProgressCallback | None = None,
        timeout_seconds: float | None = None,
    ) -> DaemonQueryResult: ...

    async def read_resource(
        self,
        public_handle: str,
        selector: ResourceSelector,
        *,
        offset: int,
        maximum_bytes: int,
        correlation_id: str,
    ) -> bytes: ...

    async def cancel_query(
        self,
        daemon_query_id: str,
        *,
        cancellation_id: str,
        correlation_id: str,
        timeout_seconds: float = 2.0,
    ) -> CancellationResult: ...

    async def release_resource(
        self,
        public_handle: str,
        *,
        release_id: str,
        correlation_id: str,
        timeout_seconds: float = 2.0,
    ) -> ReleaseResult: ...

    async def close(self) -> None: ...


def _safe_error(message: query_pb.SafeErrorMetadata) -> SafeError:
    try:
        code = _SAFE_ERROR_CODE_ADAPTER.validate_python(
            _enum_name(query_pb.SafeErrorCode.Name, message.code, "SAFE_ERROR_CODE_").upper(),
            strict=True,
        )
        layer = _SAFE_ERROR_LAYER_ADAPTER.validate_python(
            _enum_name(query_pb.SafeErrorLayer.Name, message.layer, "SAFE_ERROR_LAYER_").upper(),
            strict=True,
        )
        diagnostic_reference: DiagnosticReference = ""
        if message.HasField("diagnostic_reference"):
            diagnostic_name = _enum_name(
                query_pb.SafeDiagnosticReference.Name,
                message.diagnostic_reference,
                "SAFE_DIAGNOSTIC_REFERENCE_",
            )
            diagnostic_reference = _DIAGNOSTIC_REFERENCE_ADAPTER.validate_python(
                {
                    "LIFECYCLE_FAILED_CLOSED": "lifecycle.failed_closed",
                    "QUERY_CHALLENGE_REJECTED": "query.challenge_rejected",
                    "QUERY_TERMINAL": "query.terminal",
                }.get(diagnostic_name),
                strict=True,
            )
        return SafeError(
            code=code,
            layer=layer,
            retryable=message.retryable,
            retry_after_ms=message.retry_after_ms if message.HasField("retry_after_ms") else None,
            diagnostic_reference=diagnostic_reference,
            correlation_id=message.correlation_id,
        )
    except (ValueError, DaemonProtocolError) as error:
        raise DaemonProtocolError("daemon returned unknown safe error metadata") from error


def _local_transport_error(status: grpc.StatusCode) -> SafeError:
    code = _SAFE_ERROR_CODE_ADAPTER.validate_python(
        {
            grpc.StatusCode.INVALID_ARGUMENT: "INVALID_REQUEST",
            grpc.StatusCode.UNAUTHENTICATED: "NOT_AUTHORIZED",
            grpc.StatusCode.PERMISSION_DENIED: "NOT_AUTHORIZED",
            grpc.StatusCode.RESOURCE_EXHAUSTED: "CAPACITY_UNAVAILABLE",
            grpc.StatusCode.CANCELLED: "CANCELLED",
            grpc.StatusCode.DEADLINE_EXCEEDED: "DAEMON_UNAVAILABLE",
            grpc.StatusCode.UNAVAILABLE: "DAEMON_UNAVAILABLE",
        }.get(status, "INTERNAL"),
        strict=True,
    )
    return SafeError(
        code=code,
        layer="TRANSPORT",
        retryable=status
        in {
            grpc.StatusCode.RESOURCE_EXHAUSTED,
            grpc.StatusCode.DEADLINE_EXCEEDED,
            grpc.StatusCode.UNAVAILABLE,
        },
    )


def _deadline_exceeded_error() -> DaemonRpcError:
    return DaemonRpcError(
        grpc.StatusCode.DEADLINE_EXCEEDED,
        _local_transport_error(grpc.StatusCode.DEADLINE_EXCEEDED),
    )


def _typed_rpc_error(error: grpc.aio.AioRpcError) -> DaemonRpcError:
    for key, value in error.trailing_metadata() or ():
        if key != "codefabric-safe-error-bin":
            continue
        if not isinstance(value, bytes):
            raise DaemonProtocolError("daemon safe error metadata is not binary")
        try:
            metadata = query_pb.SafeErrorMetadata.FromString(value)
        except ValueError as decode_error:
            raise DaemonProtocolError("daemon safe error metadata is malformed") from decode_error
        return DaemonRpcError(error.code(), _safe_error(metadata))
    return DaemonRpcError(error.code(), _local_transport_error(error.code()))


def _enum_name(name: Callable[[int], str], value: int, prefix: str) -> str:
    try:
        member = name(value)
    except ValueError as error:
        raise DaemonProtocolError("daemon returned an unknown enum value") from error
    if not member.startswith(prefix) or member == f"{prefix}UNSPECIFIED":
        raise DaemonProtocolError("daemon returned an unspecified enum value")
    return member.removeprefix(prefix).lower()


def _lifecycle_name(value: int) -> str:
    return _enum_name(query_pb.LifecycleState.Name, value, "LIFECYCLE_STATE_").upper()


def _execution_name(value: int) -> QueryState:
    state = _enum_name(query_pb.QueryExecutionState.Name, value, "QUERY_EXECUTION_STATE_").upper()
    return _QUERY_STATE_ADAPTER.validate_python(state, strict=True)


def _duration(seconds: float) -> Duration:
    if seconds <= 0:
        raise ValueError("remaining RPC budget must be positive")
    total_nanos = int(seconds * 1_000_000_000)
    whole, nanos = divmod(total_nanos, 1_000_000_000)
    return Duration(seconds=whole, nanos=nanos)


def _decode_authority(message: query_pb.AuthorityGeneration) -> AuthorityGeneration:
    return AuthorityGeneration(
        session_id=message.session_id,
        session_generation=message.session_generation,
        daemon_generation=message.daemon_generation,
        supervisor_generation=message.supervisor_generation,
        policy_generation=message.policy_generation,
        revocation_generation=message.revocation_generation,
    )


def _reference_kind_value(kind: ReferenceKind) -> query_pb.ReferenceKind:
    return cast(
        query_pb.ReferenceKind,
        {
            "capability": query_pb.REFERENCE_KIND_CAPABILITY,
            "guide": query_pb.REFERENCE_KIND_GUIDE,
            "recipe": query_pb.REFERENCE_KIND_RECIPE,
            "request_schema": query_pb.REFERENCE_KIND_REQUEST_SCHEMA,
            "response_schema": query_pb.REFERENCE_KIND_RESPONSE_SCHEMA,
            "snapshot": query_pb.REFERENCE_KIND_SNAPSHOT,
        }[kind],
    )


def _choice(message: query_pb.AuthorizedChoice) -> AuthorizedChoice:
    selected = message.WhichOneof("value")
    if selected == "string_value":
        return AuthorizedStringChoice(
            choice_id=message.choice_id,
            presentation_key=message.presentation_key,
            value=message.string_value,
        )
    if selected == "integer_value":
        return AuthorizedIntegerChoice(
            choice_id=message.choice_id,
            presentation_key=message.presentation_key,
            value=message.integer_value,
        )
    if selected == "boolean_value":
        return AuthorizedBooleanChoice(
            choice_id=message.choice_id,
            presentation_key=message.presentation_key,
            value=message.boolean_value,
        )
    raise DaemonProtocolError("authorized challenge choice omitted its typed value")


def _constraints(message: query_pb.ChallengeConstraints) -> ChallengeConstraints | None:
    selected = message.WhichOneof("constraint")
    if selected is None:
        return None
    if selected == "string_constraints":
        value = message.string_constraints
        string_format = _enum_name(
            query_pb.ChallengeStringFormat.Name,
            value.format,
            "CHALLENGE_STRING_FORMAT_",
        )
        if string_format not in {"plain", "identifier", "release_version"}:
            raise DaemonProtocolError("challenge string format is not allowlisted")
        return StringChallengeConstraints(
            minimum_length=(value.minimum_length if value.HasField("minimum_length") else None),
            maximum_length=(value.maximum_length if value.HasField("maximum_length") else None),
            format=cast(Literal["plain", "identifier", "release_version"], string_format),
        )
    if selected == "integer_constraints":
        value = message.integer_constraints
        return IntegerChallengeConstraints(
            minimum=value.minimum if value.HasField("minimum") else None,
            maximum=value.maximum if value.HasField("maximum") else None,
        )
    if selected == "enum_constraints":
        value = message.enum_constraints
        return EnumChallengeConstraints(
            minimum_selections=value.minimum_selections,
            maximum_selections=value.maximum_selections,
        )
    if selected == "collection_constraints":
        value = message.collection_constraints
        item_kind = _enum_name(
            query_pb.ChallengeCollectionItemKind.Name,
            value.item_kind,
            "CHALLENGE_COLLECTION_ITEM_KIND_",
        )
        if item_kind not in {"string", "integer", "boolean", "enum"}:
            raise DaemonProtocolError("challenge collection item kind is not allowlisted")
        return CollectionChallengeConstraints(
            item_kind=cast(Literal["string", "integer", "boolean", "enum"], item_kind),
            minimum_items=value.minimum_items,
            maximum_items=value.maximum_items,
            unique_items=value.unique_items,
        )
    raise DaemonProtocolError("challenge constraint selected an unknown variant")


def _requirement(message: query_pb.InputRequirement) -> InputRequirement:
    input_kind = _enum_name(
        query_pb.ChallengeInputKind.Name,
        message.input_kind,
        "CHALLENGE_INPUT_KIND_",
    )
    allowed = {
        "string",
        "integer",
        "boolean",
        "enum",
        "string_collection",
        "integer_collection",
        "boolean_collection",
        "enum_collection",
    }
    if input_kind not in allowed:
        raise DaemonProtocolError("challenge input kind is not allowlisted")
    return InputRequirement(
        semantic_field_id=message.semantic_field_id,
        input_kind=cast(ChallengeInputKind, input_kind),
        presentation_key=message.presentation_key,
        description_key=(message.description_key if message.HasField("description_key") else None),
        required=message.required,
        constraints=_constraints(message.constraints) if message.HasField("constraints") else None,
        authorized_choices=tuple(_choice(choice) for choice in message.authorized_choices),
    )


def _requirements(messages: Iterable[query_pb.InputRequirement]) -> tuple[InputRequirement, ...]:
    try:
        requirements = tuple(_requirement(item) for item in messages)
    except ValidationError:
        raise DaemonProtocolError("challenge requirement contract is malformed") from None
    field_ids = [requirement.semantic_field_id for requirement in requirements]
    if len(field_ids) != len(set(field_ids)):
        raise DaemonProtocolError("challenge requirement contract is malformed")
    return requirements


def _validation_issue(message: query_pb.ValidationIssue) -> ValidationIssue:
    code = _SAFE_ERROR_CODE_ADAPTER.validate_python(
        _enum_name(query_pb.SafeErrorCode.Name, message.code, "SAFE_ERROR_CODE_").upper(),
        strict=True,
    )
    return ValidationIssue(
        code=code,
        semantic_field_id=message.semantic_field_id,
        presentation_key=message.presentation_key,
        retryable=message.retryable,
    )


def _answer(message: ChallengeAnswer) -> query_pb.InputAnswer:
    answer = query_pb.InputAnswer(semantic_field_id=message.semantic_field_id)
    if isinstance(message, StringInputAnswer):
        answer.string_value = message.value
    elif isinstance(message, IntegerInputAnswer):
        answer.integer_value = message.value
    elif isinstance(message, BooleanInputAnswer):
        answer.boolean_value = message.value
    elif isinstance(message, ChoiceInputAnswer):
        answer.choice_id = message.choice_id
    elif isinstance(message, StringCollectionInputAnswer):
        answer.string_collection.values.extend(message.values)
    elif isinstance(message, IntegerCollectionInputAnswer):
        answer.integer_collection.values.extend(message.values)
    elif isinstance(message, BooleanCollectionInputAnswer):
        answer.boolean_collection.values.extend(message.values)
    elif isinstance(message, ChoiceCollectionInputAnswer):
        answer.choice_collection.choice_ids.extend(message.choice_ids)
    return answer


def _resource_selector(selector: ResourceSelector) -> query_pb.ResourceSelector:
    if isinstance(selector, ManifestSelector):
        return query_pb.ResourceSelector(manifest=query_pb.ManifestSelector())
    if isinstance(selector, PageSelector):
        return query_pb.ResourceSelector(
            page=query_pb.PageSelector(page_ordinal=selector.page_ordinal)
        )
    if isinstance(selector, ReferenceSelector):
        read = query_pb.ReferenceReadRequest(kind=_reference_kind_value(selector.reference_kind))
        if selector.version is not None:
            read.version = selector.version
        return query_pb.ResourceSelector(reference=read)
    assert_never(selector)


def _progress_stage(value: int) -> ProgressStage:
    try:
        return _PROGRESS_STAGE_ADAPTER.validate_python(
            _enum_name(query_pb.ProgressStage.Name, value, "PROGRESS_STAGE_"),
            strict=True,
        )
    except (ValueError, DaemonProtocolError) as error:
        raise DaemonProtocolError("daemon returned an unknown public progress stage") from error


def _resource_descriptor_identity(
    descriptor: query_pb.ResourceDescriptor,
) -> tuple[object, ...]:
    """Return stable retained-content identity, excluding reissued public authority."""

    return (
        _enum_name(query_pb.ResourceKind.Name, descriptor.kind, "RESOURCE_KIND_"),
        descriptor.package_id if descriptor.HasField("package_id") else None,
        descriptor.page_ordinal if descriptor.HasField("page_ordinal") else None,
        descriptor.media_type,
        descriptor.byte_length,
        descriptor.content_checksum,
    )


def _safe_error_identity(error: SafeError | None) -> tuple[object, ...] | None:
    if error is None:
        return None
    return (
        error.code,
        error.layer,
        error.retryable,
        error.retry_after_ms,
        error.diagnostic_reference,
        error.correlation_id,
    )


def _query_event_identity(event: query_pb.QueryEvent) -> tuple[object, ...]:
    """Project one event to content identity independent of session/cursor/handle reissue."""

    kind = event.WhichOneof("event")
    if kind == "snapshot_pinned":
        value = event.snapshot_pinned
        return (
            kind,
            value.epoch_id,
            value.source_generation,
            value.activation_head,
            value.lifecycle_watermark,
        )
    if kind == "progress":
        value = event.progress
        return (
            kind,
            _progress_stage(value.stage),
            value.completed,
            value.total if value.HasField("total") else None,
        )
    if kind == "result_ready":
        value = event.result_ready
        if not value.HasField("manifest"):
            raise DaemonProtocolError("result-ready event omitted its manifest resource")
        return (
            kind,
            value.package_id,
            _resource_descriptor_identity(value.manifest),
            tuple(_resource_descriptor_identity(page) for page in value.pages),
            value.total_rows,
            value.total_pages,
            value.total_bytes,
        )
    if kind == "terminal":
        value = event.terminal
        error = _safe_error(value.error) if value.HasField("error") else None
        return (kind, _execution_name(value.state), _safe_error_identity(error))
    raise DaemonProtocolError("query event omitted its closed variant")


class CpgDaemonClient:
    """Lifespan-owned channel and typed daemon-port session."""

    def __init__(self, settings: Settings) -> None:
        self.settings = settings
        self.channel: grpc.aio.Channel = create_local_channel(settings.daemon_target)
        self.stub = query_grpc.CpgQueryServiceStub(self.channel)
        self.handshake_response: query_pb.HandshakeResponse | None = None
        self._authority: AuthorityGeneration | None = None
        self._session_token = b""
        self._connect_lock = asyncio.Lock()

    def current_settings(self) -> Settings:
        """Return the immutable authority record for the current daemon generation."""

        return self.settings

    def current_resource_limits(self) -> ResourceReadLimits:
        """Project the live handshake's distinct chunk and resource-unit bounds."""

        limits = self._handshake().effective_limits
        return ResourceReadLimits(
            maximum_chunk_bytes=min(
                self.settings.maximum_resource_chunk_bytes,
                limits.maximum_resource_chunk_bytes,
            ),
            maximum_resource_bytes=limits.maximum_result_bytes,
        )

    async def connect(
        self,
        *,
        correlation_id: str = "adapter-connect",
        timeout_seconds: float | None = None,
    ) -> None:
        """Wait for transport readiness and consume exactly one registered launch grant."""

        if self.handshake_response is not None:
            return
        timeout = min(
            self.settings.readiness_timeout_seconds,
            timeout_seconds
            if timeout_seconds is not None
            else self.settings.readiness_timeout_seconds,
        )
        if timeout <= 0:
            raise _deadline_exceeded_error()
        loop = asyncio.get_running_loop()
        deadline = loop.time() + timeout
        try:
            async with asyncio.timeout_at(deadline):
                async with self._connect_lock:
                    if self.handshake_response is not None:
                        return
                    await self.channel.channel_ready()
                    remaining = deadline - loop.time()
                    if remaining <= 0:
                        raise _deadline_exceeded_error()
                    response = await self.stub.Handshake(
                        query_pb.HandshakeRequest(
                            launch_grant=self.settings.launch_grant.get_secret_value(),
                            adapter_version=version("codefabric-cpg-mcp"),
                            minimum_minor=RPC_MINOR,
                            maximum_minor=RPC_MINOR,
                            required_feature_bits=0,
                            optional_feature_bits=0,
                            desired_semantic_profiles=[SEMANTIC_PROFILE],
                            maximum_resource_chunk_bytes=(
                                self.settings.maximum_resource_chunk_bytes
                            ),
                            remaining_budget=_duration(remaining),
                            correlation_id=correlation_id,
                        ),
                        timeout=remaining,
                    )
        except TimeoutError:
            raise _deadline_exceeded_error() from None
        except grpc.aio.AioRpcError as error:
            raise _typed_rpc_error(error) from None
        if (
            len(response.session_token) != 32
            or not response.HasField("authority")
            or response.selected_minor != RPC_MINOR
            or response.selected_semantic_profile != SEMANTIC_PROFILE
            or not response.HasField("effective_limits")
            or not response.HasField("reserved_control")
        ):
            raise DaemonProtocolError("handshake response differs from launch authority")
        authority = _decode_authority(response.authority)
        if (
            authority.daemon_generation != self.settings.daemon_generation
            or authority.supervisor_generation != self.settings.supervisor_generation
            or response.session_expires_at_unix_ms != self.settings.session_expires_at_unix_ms
            or response.effective_limits.maximum_resource_chunk_bytes
            > self.settings.maximum_resource_chunk_bytes
            or response.effective_limits.maximum_resource_chunk_bytes == 0
            or response.effective_limits.maximum_result_bytes
            < response.effective_limits.maximum_resource_chunk_bytes
            or response.reserved_control.reserved_capacity <= 0
            or set(response.reserved_control.operations)
            != {
                query_pb.RESERVED_CONTROL_OPERATION_HANDSHAKE,
                query_pb.RESERVED_CONTROL_OPERATION_GET_STATUS,
                query_pb.RESERVED_CONTROL_OPERATION_CANCEL_QUERY,
                query_pb.RESERVED_CONTROL_OPERATION_RELEASE_RESOURCE,
            }
        ):
            raise DaemonProtocolError("handshake authority or control reservation differs")
        self._session_token = bytes(response.session_token)
        self._authority = authority
        self.handshake_response = response

    def _metadata(self) -> tuple[tuple[str, bytes], ...]:
        if len(self._session_token) != 32:
            raise DaemonProtocolError("daemon session is unavailable")
        return ((SESSION_METADATA_KEY, self._session_token),)

    def _handshake(self) -> query_pb.HandshakeResponse:
        response = self.handshake_response
        if response is None:
            raise DaemonProtocolError("daemon session is unavailable")
        return response

    def _context(self, correlation_id: str, timeout_seconds: float) -> query_pb.RequestContext:
        if self._authority is None:
            raise DaemonProtocolError("daemon session is unavailable")
        if not correlation_id:
            raise ValueError("correlation ID must be non-empty")
        timeout = min(timeout_seconds, self.settings.query_timeout_seconds)
        return query_pb.RequestContext(
            correlation_id=correlation_id,
            remaining_budget=_duration(timeout),
        )

    def _assert_authority(self, message: query_pb.AuthorityGeneration) -> AuthorityGeneration:
        current = self._authority
        if current is None:
            raise DaemonProtocolError("daemon session is unavailable")
        received = _decode_authority(message)
        if (
            received.session_id != current.session_id
            or received.session_generation != current.session_generation
            or received.daemon_generation != current.daemon_generation
            or received.supervisor_generation != current.supervisor_generation
            or received.policy_generation < current.policy_generation
            or received.revocation_generation < current.revocation_generation
        ):
            raise DaemonProtocolError("daemon response authority differs from the active session")
        self._authority = received
        return received

    async def _reconnect(self, *, correlation_id: str, timeout_seconds: float) -> bool:
        previous = self._authority
        if previous is None:
            raise DaemonProtocolError("daemon session is unavailable")
        if timeout_seconds <= 0:
            raise _deadline_exceeded_error()
        loop = asyncio.get_running_loop()
        deadline = loop.time() + timeout_seconds
        try:
            async with asyncio.timeout_at(deadline):
                replacement = await asyncio.to_thread(
                    next_settings,
                    timeout_seconds=deadline - loop.time(),
                )
                remaining = deadline - loop.time()
                if remaining <= 0:
                    raise _deadline_exceeded_error()
                await self.channel.close(grace=min(1.0, remaining))
                self.settings = replacement
                self.channel = create_local_channel(replacement.daemon_target)
                self.stub = query_grpc.CpgQueryServiceStub(self.channel)
                self.handshake_response = None
                self._authority = None
                self._session_token = b""
                await self.connect(
                    correlation_id=correlation_id,
                    timeout_seconds=deadline - loop.time(),
                )
        except TimeoutError:
            raise _deadline_exceeded_error() from None
        current = self._authority
        if current is None:
            raise DaemonProtocolError("replacement daemon session is unavailable")
        daemon_changed = current.daemon_generation != previous.daemon_generation
        if (
            current.daemon_generation < previous.daemon_generation
            or current.supervisor_generation < previous.supervisor_generation
            or current.policy_generation < previous.policy_generation
            or current.revocation_generation < previous.revocation_generation
            or (
                not daemon_changed
                and (
                    current.supervisor_generation != previous.supervisor_generation
                    or current.session_generation <= previous.session_generation
                )
            )
            or (
                current.session_id == previous.session_id
                and current.session_generation == previous.session_generation
            )
        ):
            raise DaemonProtocolError("replacement daemon authority did not advance")
        return daemon_changed

    async def status(self, *, correlation_id: str) -> DaemonStatus:
        await self.connect(correlation_id=correlation_id)
        timeout = self.settings.query_timeout_seconds
        try:
            response = await self.stub.GetStatus(
                query_pb.GetStatusRequest(
                    context=self._context(correlation_id, timeout),
                    include_diagnostics=False,
                ),
                metadata=self._metadata(),
                timeout=timeout,
            )
        except grpc.aio.AioRpcError as error:
            raise _typed_rpc_error(error) from None
        canonical = canonicalize_json(response.canonical_public_status_json)
        if canonical != response.canonical_public_status_json:
            raise DaemonProtocolError("daemon status JSON is not canonical")
        try:
            value = PublicDaemonStatus.model_validate_json(canonical, strict=True)
        except ValueError as error:
            raise DaemonProtocolError(
                "daemon status projection is not closed and public"
            ) from error
        lifecycle = _lifecycle_name(response.lifecycle)
        active_epoch_id = response.active_epoch_id if response.HasField("active_epoch_id") else None
        if (
            value.lifecycle != lifecycle
            or value.lifecycle_sequence != response.lifecycle_sequence
            or value.active_epoch_id != active_epoch_id
            or value.running_queries != response.running_queries
            or value.queued_queries != response.queued_queries
        ):
            raise DaemonProtocolError("status lifecycle projections differ")
        return DaemonStatus(
            authority=self._assert_authority(response.authority),
            lifecycle=cast(
                Literal["BOOTSTRAPPING", "READY", "DRAINING", "FAILED_CLOSED"], lifecycle
            ),
            lifecycle_sequence=response.lifecycle_sequence,
            failure=_safe_error(response.failure) if response.HasField("failure") else None,
            active_epoch_id=active_epoch_id,
            running_queries=response.running_queries,
            queued_queries=response.queued_queries,
            public_status=value,
        )

    async def reference(
        self,
        kind: ReferenceKind,
        version_value: str | None,
        *,
        correlation_id: str,
    ) -> ReferenceDocument:
        await self.connect(correlation_id=correlation_id)
        kind = _REFERENCE_KIND_ADAPTER.validate_python(kind, strict=True)
        timeout = self.settings.query_timeout_seconds
        read = query_pb.ReferenceReadRequest(kind=_reference_kind_value(kind))
        if version_value is not None:
            read.version = version_value
        try:
            response = await self.stub.GetReference(
                query_pb.GetReferenceRequest(
                    context=self._context(correlation_id, timeout),
                    read=read,
                ),
                metadata=self._metadata(),
                timeout=timeout,
            )
        except grpc.aio.AioRpcError as error:
            raise _typed_rpc_error(error) from None
        if response.WhichOneof("result") != "reference":
            raise DaemonProtocolError("reference read returned a different closed result")
        document = response.reference
        authority = self._assert_authority(response.authority)
        if not document.HasField("resource"):
            raise DaemonProtocolError("reference document omitted its public resource handle")
        resource = self._resource_handle(document.resource)
        if resource.kind != "reference" or resource.authority != authority:
            raise DaemonProtocolError("reference resource authority or kind differs")
        return ReferenceDocument(
            authority=authority,
            reference_id=document.reference_id,
            resource=resource,
        )

    async def complete_reference(
        self,
        *,
        variable: Literal["kind", "released_version"],
        prefix: str,
        kind: ReferenceKind | None,
        selector: str | None,
        maximum_candidates: int,
        correlation_id: str,
    ) -> ReferenceCompletion:
        await self.connect(correlation_id=correlation_id)
        maximum = min(
            maximum_candidates,
            self._handshake().effective_limits.maximum_reference_completion_candidates,
        )
        if maximum <= 0:
            raise ValueError("completion candidate cap must be positive")
        request = query_pb.ReferenceCompletionRequest(
            variable={
                "kind": query_pb.REFERENCE_TEMPLATE_VARIABLE_KIND,
                "released_version": query_pb.REFERENCE_TEMPLATE_VARIABLE_RELEASED_VERSION,
            }[variable],
            prefix=prefix,
            maximum_candidates=maximum,
        )
        if kind is not None:
            request.kind = _reference_kind_value(
                _REFERENCE_KIND_ADAPTER.validate_python(kind, strict=True)
            )
        if selector is not None:
            request.selector = selector
        timeout = self.settings.query_timeout_seconds
        try:
            response = await self.stub.GetReference(
                query_pb.GetReferenceRequest(
                    context=self._context(correlation_id, timeout),
                    completion=request,
                ),
                metadata=self._metadata(),
                timeout=timeout,
            )
        except grpc.aio.AioRpcError as error:
            raise _typed_rpc_error(error) from None
        if response.WhichOneof("result") != "completion":
            raise DaemonProtocolError("reference completion returned a different closed result")
        completion = response.completion
        if len(completion.candidates) > maximum or completion.total < len(completion.candidates):
            raise DaemonProtocolError("reference completion exceeded its declared bounds")
        return ReferenceCompletion(
            authority=self._assert_authority(response.authority),
            candidates=tuple(
                ReferenceCompletionCandidate(
                    value=candidate.value,
                    presentation_key=candidate.presentation_key,
                )
                for candidate in completion.candidates
            ),
            total=completion.total,
            has_more=completion.has_more,
        )

    async def validate(self, value: ValidateToolInput, *, correlation_id: str) -> QueryPreparation:
        await self.connect(correlation_id=correlation_id)
        request_bytes = canonicalize_value(value.request)
        limits = self._handshake().effective_limits
        timeout = self.settings.query_timeout_seconds
        try:
            response = await self.stub.ValidateQuery(
                query_pb.ValidateQueryRequest(
                    context=self._context(correlation_id, timeout),
                    query=query_pb.QuerySubmission(
                        canonical_request_json=request_bytes,
                        request_checksum=checksum(request_bytes),
                        semantic_profile=SEMANTIC_PROFILE,
                        result_limits=query_pb.ResultLimits(
                            maximum_result_bytes=limits.maximum_result_bytes,
                            maximum_result_pages=limits.maximum_result_pages,
                        ),
                    ),
                ),
                metadata=self._metadata(),
                timeout=timeout,
            )
        except grpc.aio.AioRpcError as error:
            raise _typed_rpc_error(error) from None
        normalized: JsonObject | None = None
        preparation = response.preparation
        if preparation.canonical_normalized_request_json:
            canonical = canonicalize_json(preparation.canonical_normalized_request_json)
            if canonical != preparation.canonical_normalized_request_json:
                raise DaemonProtocolError("normalized query is not canonical JSON")
            normalized = cast(JsonObject, JSON_OBJECT_ADAPTER.validate_json(canonical, strict=True))
        errors = tuple(_validation_issue(issue) for issue in preparation.errors)
        requirements = _requirements(preparation.input_requirements)
        return QueryPreparation(
            authority=self._assert_authority(response.authority),
            valid=not errors and not requirements,
            semantic_request_id=preparation.semantic_request_id or None,
            normalized_request=normalized,
            input_requirements=requirements,
            errors=errors,
            warnings=tuple(_validation_issue(issue) for issue in preparation.warnings),
            cost_class=preparation.cost_class,
            estimated_result_bytes=preparation.estimated_result_bytes,
            estimated_result_pages=preparation.estimated_result_pages,
        )

    def _query_submission(self, value: QueryToolInput) -> query_pb.QuerySubmission:
        canonical_request = canonicalize_value(value.request)
        limits = self._handshake().effective_limits
        return query_pb.QuerySubmission(
            canonical_request_json=canonical_request,
            request_checksum=checksum(canonical_request),
            semantic_profile=SEMANTIC_PROFILE,
            result_limits=query_pb.ResultLimits(
                maximum_result_bytes=limits.maximum_result_bytes,
                maximum_result_pages=limits.maximum_result_pages,
            ),
        )

    def _start_outcome(self, response: query_pb.StartQueryResponse) -> StartQueryOutcome:
        selected = response.WhichOneof("outcome")
        if selected == "accepted":
            value = response.accepted
            return AcceptedQuery(
                authority=self._assert_authority(value.authority),
                daemon_query_id=value.daemon_query_id,
                semantic_request_id=value.semantic_request_id,
                operation_fingerprint=value.operation_fingerprint,
                accepted_at_unix_ms=value.accepted_at_unix_ms,
                observation_expires_at_unix_ms=value.observation_expires_at_unix_ms,
                state=_execution_name(value.state),
                idempotent_replay=value.idempotent_replay,
            )
        if selected == "input_challenge":
            value = response.input_challenge
            explanation = _enum_name(
                query_pb.ChallengeExplanationCode.Name,
                value.explanation_code,
                "CHALLENGE_EXPLANATION_CODE_",
            )
            if explanation not in {
                "required_input_missing",
                "reference_ambiguous",
                "bounded_selection_required",
            }:
                raise DaemonProtocolError("challenge explanation is not allowlisted")
            limits = self._handshake().effective_limits
            if (
                not value.requirements
                or len(value.requirements) > limits.maximum_challenge_fields
                or value.round > limits.maximum_challenge_rounds
                or value.remaining_rounds > limits.maximum_challenge_rounds
                or value.round + value.remaining_rounds != limits.maximum_challenge_rounds
                or value.maximum_answer_bytes > limits.maximum_control_message_bytes
                or value.expires_at_unix_ms > self._handshake().session_expires_at_unix_ms
                or any(
                    len(requirement.authorized_choices) > limits.maximum_choices_per_field
                    for requirement in value.requirements
                )
            ):
                raise DaemonProtocolError("challenge exceeds negotiated bounds")
            try:
                return InputChallenge(
                    authority=self._assert_authority(value.authority),
                    semantic_request_id=value.semantic_request_id,
                    challenge_id=value.challenge_id,
                    round=value.round,
                    remaining_rounds=value.remaining_rounds,
                    issued_at_unix_ms=value.issued_at_unix_ms,
                    expires_at_unix_ms=value.expires_at_unix_ms,
                    maximum_answer_bytes=value.maximum_answer_bytes,
                    explanation_code=cast(
                        Literal[
                            "required_input_missing",
                            "reference_ambiguous",
                            "bounded_selection_required",
                        ],
                        explanation,
                    ),
                    requirements=_requirements(value.requirements),
                    daemon_continuation=bytes(value.daemon_continuation),
                )
            except ValidationError:
                raise DaemonProtocolError("challenge requirement contract is malformed") from None
        if selected == "validation_rejection":
            value = response.validation_rejection
            return ValidationRejection(
                authority=self._assert_authority(value.authority),
                semantic_request_id=(
                    value.semantic_request_id if value.HasField("semantic_request_id") else None
                ),
                issues=tuple(_validation_issue(issue) for issue in value.issues),
                error=_safe_error(value.error),
            )
        raise DaemonProtocolError("start query omitted its closed outcome")

    async def start_query(
        self,
        value: QueryToolInput,
        *,
        correlation_id: str,
    ) -> StartQueryOutcome:
        await self.connect(correlation_id=correlation_id)
        timeout = self.settings.query_timeout_seconds
        try:
            response = await self.stub.StartQuery(
                query_pb.StartQueryRequest(
                    context=self._context(correlation_id, timeout),
                    initial=query_pb.InitialQueryStart(
                        query=self._query_submission(value),
                    ),
                ),
                metadata=self._metadata(),
                timeout=timeout,
            )
        except grpc.aio.AioRpcError as error:
            raise _typed_rpc_error(error) from None
        return self._start_outcome(response)

    async def continue_query(
        self,
        challenge: InputChallenge,
        answers: tuple[ChallengeAnswer, ...],
        *,
        correlation_id: str,
    ) -> StartQueryOutcome:
        await self.connect(correlation_id=correlation_id)
        current = self._authority
        if current is None or challenge.authority != current:
            raise DaemonProtocolError("challenge authority differs from the active session")
        answer_ids = [answer.semantic_field_id for answer in answers]
        requirement_ids = {item.semantic_field_id for item in challenge.requirements}
        required_ids = {item.semantic_field_id for item in challenge.requirements if item.required}
        if (
            len(answer_ids) != len(set(answer_ids))
            or not set(answer_ids).issubset(requirement_ids)
            or not required_ids.issubset(answer_ids)
        ):
            raise ValueError("challenge answers do not match the typed requirements")
        continuation = query_pb.QueryChallengeContinuation(
            daemon_continuation=challenge.daemon_continuation,
            challenge_id=challenge.challenge_id,
            round=challenge.round,
            answers=[_answer(answer) for answer in answers],
        )
        if continuation.ByteSize() > challenge.maximum_answer_bytes:
            raise ValueError("challenge answer exceeds the daemon-declared byte bound")
        timeout = self.settings.query_timeout_seconds
        try:
            response = await self.stub.StartQuery(
                query_pb.StartQueryRequest(
                    context=self._context(correlation_id, timeout),
                    continuation=continuation,
                ),
                metadata=self._metadata(),
                timeout=timeout,
            )
        except grpc.aio.AioRpcError as error:
            raise _typed_rpc_error(error) from None
        return self._start_outcome(response)

    def _resource_handle(self, descriptor: query_pb.ResourceDescriptor) -> ResourceHandle:
        kind = _enum_name(query_pb.ResourceKind.Name, descriptor.kind, "RESOURCE_KIND_")
        if kind not in {"result_manifest", "result_page", "reference"}:
            raise DaemonProtocolError("resource handle kind is not allowlisted")
        return ResourceHandle(
            kind=cast(Literal["result_manifest", "result_page", "reference"], kind),
            public_handle=descriptor.public_handle,
            package_id=descriptor.package_id if descriptor.HasField("package_id") else None,
            page_ordinal=(descriptor.page_ordinal if descriptor.HasField("page_ordinal") else None),
            media_type=descriptor.media_type,
            byte_length=descriptor.byte_length,
            content_checksum=descriptor.content_checksum,
            expires_at_unix_ms=descriptor.expires_at_unix_ms,
            authority=self._assert_authority(descriptor.authority),
        )

    async def watch_query(
        self,
        accepted: AcceptedQuery,
        *,
        correlation_id: str,
        progress: ProgressCallback | None = None,
        timeout_seconds: float | None = None,
    ) -> DaemonQueryResult:
        timeout = min(
            timeout_seconds if timeout_seconds is not None else self.settings.query_timeout_seconds,
            self.settings.query_timeout_seconds,
        )
        loop = asyncio.get_running_loop()
        deadline = loop.time() + timeout
        await self.connect(
            correlation_id=correlation_id,
            timeout_seconds=deadline - loop.time(),
        )
        current = self._authority
        if current is None or accepted.authority != current:
            raise DaemonProtocolError("accepted query authority differs from the active session")
        cursor: bytes | None = None
        epoch_id: str | None = None
        result_ready: query_pb.ResultReadyEvent | None = None
        terminal: query_pb.TerminalEvent | None = None
        last_sequence = 0
        observed_events: dict[int, tuple[object, ...]] = {}
        result_predecessor: tuple[bytes | None, int] | None = None
        reconnects = 0
        while terminal is None:
            remaining = deadline - loop.time()
            if remaining <= 0:
                raise _deadline_exceeded_error()
            try:
                request = query_pb.WatchQueryRequest(
                    context=self._context(correlation_id, remaining),
                    daemon_query_id=accepted.daemon_query_id,
                )
                if cursor is not None:
                    request.cursor = cursor
                stream = self.stub.WatchQuery(
                    request,
                    metadata=self._metadata(),
                    timeout=remaining,
                )
                async for event in stream:
                    kind = event.WhichOneof("event")
                    if kind == "snapshot_pinned":
                        payload = event.snapshot_pinned
                    elif kind == "progress":
                        payload = event.progress
                    elif kind == "result_ready":
                        payload = event.result_ready
                    elif kind == "terminal":
                        payload = event.terminal
                    else:
                        raise DaemonProtocolError("query event omitted its closed variant")
                    if not payload.HasField("header"):
                        raise DaemonProtocolError("query event omitted its bound header")
                    header = payload.header
                    if header.daemon_query_id != accepted.daemon_query_id:
                        raise DaemonProtocolError("query event identity differs")
                    if header.sequence != last_sequence + 1 or not header.cursor:
                        raise DaemonProtocolError("query event cursor or sequence is invalid")
                    identity = _query_event_identity(event)
                    previous_identity = observed_events.get(header.sequence)
                    if previous_identity is not None and previous_identity != identity:
                        raise DaemonProtocolError("replayed query event content changed")
                    replayed = previous_identity is not None
                    self._assert_authority(header.authority)
                    predecessor = (cursor, last_sequence)
                    observed_events[header.sequence] = identity
                    last_sequence = header.sequence
                    cursor = bytes(header.cursor)
                    if kind == "snapshot_pinned":
                        epoch_id = payload.epoch_id
                    elif kind == "progress":
                        if progress is not None and not replayed:
                            total = payload.total if payload.HasField("total") else None
                            try:
                                async with asyncio.timeout_at(deadline):
                                    await progress(
                                        payload.completed,
                                        total,
                                        _progress_stage(payload.stage),
                                    )
                            except TimeoutError:
                                raise _deadline_exceeded_error() from None
                    elif kind == "result_ready":
                        result_predecessor = predecessor
                        result_ready = query_pb.ResultReadyEvent()
                        result_ready.CopyFrom(payload)
                    elif kind == "terminal":
                        terminal = query_pb.TerminalEvent()
                        terminal.CopyFrom(payload)
                        break
                if terminal is None:
                    raise DaemonProtocolError("watch ended before a terminal event")
            except grpc.aio.AioRpcError as error:
                if error.code() is not grpc.StatusCode.UNAVAILABLE or reconnects >= 1:
                    raise _typed_rpc_error(error) from None
                reconnects += 1
                daemon_changed = await self._reconnect(
                    correlation_id=correlation_id,
                    timeout_seconds=deadline - loop.time(),
                )
                if daemon_changed:
                    cursor = None
                    last_sequence = 0
                    epoch_id = None
                    result_ready = None
                    result_predecessor = None
                elif result_ready is not None:
                    if result_predecessor is None:
                        raise DaemonProtocolError(
                            "result-ready event omitted its replay predecessor"
                        ) from None
                    cursor, last_sequence = result_predecessor
                    result_ready = None
                    result_predecessor = None

        state = _execution_name(terminal.state)
        manifest: ResourceHandle | None = None
        page_resources: tuple[ResourceHandle, ...] = ()
        package_id: str | None = None
        rows = pages = bytes_count = 0
        if result_ready is not None:
            if not result_ready.HasField("manifest"):
                raise DaemonProtocolError("result-ready event omitted its manifest resource")
            package_id = result_ready.package_id
            manifest = self._resource_handle(result_ready.manifest)
            if manifest.kind != "result_manifest" or manifest.package_id != package_id:
                raise DaemonProtocolError("result manifest identity differs from its package")
            page_resources = tuple(self._resource_handle(item) for item in result_ready.pages)
            if (
                len(page_resources) != result_ready.total_pages
                or [item.page_ordinal for item in page_resources]
                != list(range(result_ready.total_pages))
                or len({item.public_handle for item in page_resources}) != len(page_resources)
                or manifest.public_handle in {item.public_handle for item in page_resources}
                or any(
                    item.kind != "result_page" or item.package_id != package_id
                    for item in page_resources
                )
            ):
                raise DaemonProtocolError("result page descriptors differ from their package")
            rows = result_ready.total_rows
            pages = result_ready.total_pages
            bytes_count = result_ready.total_bytes
        terminal_error = _safe_error(terminal.error) if terminal.HasField("error") else None
        authority = self._assert_authority(terminal.header.authority)
        return DaemonQueryResult(
            authority=authority,
            semantic_request_id=accepted.semantic_request_id,
            daemon_query_id=accepted.daemon_query_id,
            execution_state=state,
            epoch_id=epoch_id,
            package_id=package_id,
            manifest=manifest,
            pages=page_resources,
            total_rows=rows,
            total_pages=pages,
            total_bytes=bytes_count,
            error=terminal_error,
            notices=((terminal_error.code,) if terminal_error is not None else ()),
        )

    async def read_resource(
        self,
        public_handle: str,
        selector: ResourceSelector,
        *,
        offset: int,
        maximum_bytes: int,
        correlation_id: str,
    ) -> bytes:
        await self.connect(correlation_id=correlation_id)
        if not public_handle or offset != 0 or maximum_bytes <= 0:
            raise ValueError("invalid bounded resource request")
        maximum = min(
            maximum_bytes,
            self._handshake().effective_limits.maximum_resource_chunk_bytes,
        )
        expected_offset = offset
        result = bytearray()
        ended = False
        resource_checksum: str | None = None
        total_bound = self._handshake().effective_limits.maximum_result_bytes
        timeout = self.settings.query_timeout_seconds
        try:
            stream = self.stub.ReadResource(
                query_pb.ReadResourceRequest(
                    context=self._context(correlation_id, timeout),
                    public_handle=public_handle,
                    selector=_resource_selector(selector),
                    offset=offset,
                    maximum_bytes=maximum,
                ),
                metadata=self._metadata(),
                timeout=timeout,
            )
            async for chunk in stream:
                if (
                    ended
                    or chunk.public_handle != public_handle
                    or chunk.offset != expected_offset
                    or len(chunk.content) > maximum
                    or len(result) + len(chunk.content) > total_bound
                    or (
                        resource_checksum is not None
                        and chunk.content_checksum != resource_checksum
                    )
                ):
                    raise DaemonProtocolError("resource chunk framing differs")
                resource_checksum = chunk.content_checksum
                self._assert_authority(chunk.authority)
                result.extend(chunk.content)
                expected_offset += len(chunk.content)
                ended = chunk.end_of_resource
        except grpc.aio.AioRpcError as error:
            raise _typed_rpc_error(error) from None
        if not ended or resource_checksum is None or checksum(bytes(result)) != resource_checksum:
            raise DaemonProtocolError("resource stream ended without a terminal bound")
        return bytes(result)

    async def cancel_query(
        self,
        daemon_query_id: str,
        *,
        cancellation_id: str,
        correlation_id: str,
        timeout_seconds: float = 2.0,
    ) -> CancellationResult:
        await self.connect(correlation_id=correlation_id)
        if not daemon_query_id or not cancellation_id:
            raise ValueError("cancel identity must be non-empty")
        timeout = min(timeout_seconds, self.settings.query_timeout_seconds)
        try:
            response = await self.stub.CancelQuery(
                query_pb.CancelQueryRequest(
                    context=self._context(correlation_id, timeout),
                    daemon_query_id=daemon_query_id,
                    cancellation_id=cancellation_id,
                ),
                metadata=self._metadata(),
                timeout=timeout,
            )
        except grpc.aio.AioRpcError as error:
            raise _typed_rpc_error(error) from None
        if response.cancellation_id != cancellation_id:
            raise DaemonProtocolError("cancel acknowledgement identity differs")
        acknowledgement = _enum_name(
            query_pb.CancellationAcknowledgement.Name,
            response.acknowledgement,
            "CANCELLATION_ACKNOWLEDGEMENT_",
        )
        if acknowledgement not in {
            "accepted",
            "replayed",
            "already_terminal",
            "query_not_found",
        }:
            raise DaemonProtocolError("cancel acknowledgement is not allowlisted")
        terminal = response.terminal if response.HasField("terminal") else None
        return CancellationResult(
            authority=self._assert_authority(response.authority),
            cancellation_id=response.cancellation_id,
            acknowledgement=cast(
                Literal["accepted", "replayed", "already_terminal", "query_not_found"],
                acknowledgement,
            ),
            terminal_state=(_execution_name(terminal.state) if terminal is not None else None),
            terminal_error=(
                _safe_error(terminal.error)
                if terminal is not None and terminal.HasField("error")
                else None
            ),
            idempotent_replay=response.idempotent_replay,
        )

    async def release_resource(
        self,
        public_handle: str,
        *,
        release_id: str,
        correlation_id: str,
        timeout_seconds: float = 2.0,
    ) -> ReleaseResult:
        await self.connect(correlation_id=correlation_id)
        if not public_handle or not release_id:
            raise ValueError("release identity must be non-empty")
        timeout = min(timeout_seconds, self.settings.query_timeout_seconds)
        try:
            response = await self.stub.ReleaseResource(
                query_pb.ReleaseResourceRequest(
                    context=self._context(correlation_id, timeout),
                    public_handle=public_handle,
                    release_id=release_id,
                ),
                metadata=self._metadata(),
                timeout=timeout,
            )
        except grpc.aio.AioRpcError as error:
            raise _typed_rpc_error(error) from None
        if response.release_id != release_id:
            raise DaemonProtocolError("release acknowledgement identity differs")
        state = _enum_name(query_pb.ReleaseState.Name, response.state, "RELEASE_STATE_")
        if state not in {"released", "already_released", "not_found"}:
            raise DaemonProtocolError("release acknowledgement is not allowlisted")
        return ReleaseResult(
            authority=self._assert_authority(response.authority),
            release_id=response.release_id,
            state=cast(Literal["released", "already_released", "not_found"], state),
            idempotent_replay=response.idempotent_replay,
        )

    async def close(self) -> None:
        """Close the lifespan channel; resources are released explicitly by public handle."""

        await self.channel.close(grace=1.0)


__all__ = [
    "AcceptedQuery",
    "AuthorityGeneration",
    "BooleanCollectionInputAnswer",
    "BooleanInputAnswer",
    "CancellationResult",
    "ChallengeAnswer",
    "ChoiceCollectionInputAnswer",
    "ChoiceInputAnswer",
    "CpgDaemonClient",
    "DaemonPort",
    "DaemonProtocolError",
    "DaemonQueryResult",
    "DaemonRpcError",
    "DaemonStatus",
    "InputChallenge",
    "InputRequirement",
    "IntegerCollectionInputAnswer",
    "IntegerInputAnswer",
    "ManifestSelector",
    "PageSelector",
    "QueryPreparation",
    "ReferenceCompletion",
    "ReferenceDocument",
    "ReferenceSelector",
    "ReleaseResult",
    "ResourceHandle",
    "ResourceSelector",
    "SafeError",
    "StartQueryOutcome",
    "StringCollectionInputAnswer",
    "StringInputAnswer",
    "ValidationRejection",
]
