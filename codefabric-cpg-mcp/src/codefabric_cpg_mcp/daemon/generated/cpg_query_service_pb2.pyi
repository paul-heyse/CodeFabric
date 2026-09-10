# @generated from released Protobuf semantic identities b3:e59786c90044225c4fb4fc894473b14cab543af9738da3c791cfccb1c402f37c,b3:71fb94283214d79068ede88e0f45e1460336b23b9678f80b4ddbece098cd626f,b3:d5b256baca150eed2617f78f88362c607ff12db7a94af9524658a3c82f247973,b3:2f2c24a2877be95dfd1d3acc7d83354838696af2aaac13c99bde83ab743f6c62; do not edit.
import datetime

from google.protobuf import duration_pb2 as _duration_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class LifecycleState(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    LIFECYCLE_STATE_UNSPECIFIED: _ClassVar[LifecycleState]
    LIFECYCLE_STATE_BOOTSTRAPPING: _ClassVar[LifecycleState]
    LIFECYCLE_STATE_READY: _ClassVar[LifecycleState]
    LIFECYCLE_STATE_DRAINING: _ClassVar[LifecycleState]
    LIFECYCLE_STATE_FAILED_CLOSED: _ClassVar[LifecycleState]

class QueryExecutionState(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    QUERY_EXECUTION_STATE_UNSPECIFIED: _ClassVar[QueryExecutionState]
    QUERY_EXECUTION_STATE_ACCEPTED: _ClassVar[QueryExecutionState]
    QUERY_EXECUTION_STATE_QUEUED: _ClassVar[QueryExecutionState]
    QUERY_EXECUTION_STATE_RUNNING: _ClassVar[QueryExecutionState]
    QUERY_EXECUTION_STATE_SUCCEEDED: _ClassVar[QueryExecutionState]
    QUERY_EXECUTION_STATE_FAILED: _ClassVar[QueryExecutionState]
    QUERY_EXECUTION_STATE_CANCELLED: _ClassVar[QueryExecutionState]
    QUERY_EXECUTION_STATE_LOST: _ClassVar[QueryExecutionState]

class ChallengeInputKind(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CHALLENGE_INPUT_KIND_UNSPECIFIED: _ClassVar[ChallengeInputKind]
    CHALLENGE_INPUT_KIND_STRING: _ClassVar[ChallengeInputKind]
    CHALLENGE_INPUT_KIND_INTEGER: _ClassVar[ChallengeInputKind]
    CHALLENGE_INPUT_KIND_BOOLEAN: _ClassVar[ChallengeInputKind]
    CHALLENGE_INPUT_KIND_ENUM: _ClassVar[ChallengeInputKind]
    CHALLENGE_INPUT_KIND_STRING_COLLECTION: _ClassVar[ChallengeInputKind]
    CHALLENGE_INPUT_KIND_INTEGER_COLLECTION: _ClassVar[ChallengeInputKind]
    CHALLENGE_INPUT_KIND_BOOLEAN_COLLECTION: _ClassVar[ChallengeInputKind]
    CHALLENGE_INPUT_KIND_ENUM_COLLECTION: _ClassVar[ChallengeInputKind]

class ChallengeCollectionItemKind(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CHALLENGE_COLLECTION_ITEM_KIND_UNSPECIFIED: _ClassVar[ChallengeCollectionItemKind]
    CHALLENGE_COLLECTION_ITEM_KIND_STRING: _ClassVar[ChallengeCollectionItemKind]
    CHALLENGE_COLLECTION_ITEM_KIND_INTEGER: _ClassVar[ChallengeCollectionItemKind]
    CHALLENGE_COLLECTION_ITEM_KIND_BOOLEAN: _ClassVar[ChallengeCollectionItemKind]
    CHALLENGE_COLLECTION_ITEM_KIND_ENUM: _ClassVar[ChallengeCollectionItemKind]

class ChallengeStringFormat(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CHALLENGE_STRING_FORMAT_UNSPECIFIED: _ClassVar[ChallengeStringFormat]
    CHALLENGE_STRING_FORMAT_PLAIN: _ClassVar[ChallengeStringFormat]
    CHALLENGE_STRING_FORMAT_IDENTIFIER: _ClassVar[ChallengeStringFormat]
    CHALLENGE_STRING_FORMAT_RELEASE_VERSION: _ClassVar[ChallengeStringFormat]

class ChallengeExplanationCode(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CHALLENGE_EXPLANATION_CODE_UNSPECIFIED: _ClassVar[ChallengeExplanationCode]
    CHALLENGE_EXPLANATION_CODE_REQUIRED_INPUT_MISSING: _ClassVar[ChallengeExplanationCode]
    CHALLENGE_EXPLANATION_CODE_REFERENCE_AMBIGUOUS: _ClassVar[ChallengeExplanationCode]
    CHALLENGE_EXPLANATION_CODE_BOUNDED_SELECTION_REQUIRED: _ClassVar[ChallengeExplanationCode]

class SafeErrorLayer(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    SAFE_ERROR_LAYER_UNSPECIFIED: _ClassVar[SafeErrorLayer]
    SAFE_ERROR_LAYER_TRANSPORT: _ClassVar[SafeErrorLayer]
    SAFE_ERROR_LAYER_AUTHORIZATION: _ClassVar[SafeErrorLayer]
    SAFE_ERROR_LAYER_VALIDATION: _ClassVar[SafeErrorLayer]
    SAFE_ERROR_LAYER_QUERY: _ClassVar[SafeErrorLayer]
    SAFE_ERROR_LAYER_RESOURCE: _ClassVar[SafeErrorLayer]
    SAFE_ERROR_LAYER_LIFECYCLE: _ClassVar[SafeErrorLayer]

class SafeErrorCode(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    SAFE_ERROR_CODE_UNSPECIFIED: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_INVALID_REQUEST: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_VALIDATION_REJECTED: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_INPUT_REQUIRED: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_NOT_AUTHORIZED: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_IDEMPOTENCY_CONFLICT: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_CONTINUATION_EXPIRED: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_CONTINUATION_REPLAYED: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_GENERATION_MISMATCH: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_QUERY_NOT_FOUND: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_RESOURCE_NOT_FOUND: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_RESOURCE_EXPIRED: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_RANGE_NOT_SATISFIABLE: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_CAPACITY_UNAVAILABLE: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_CANCELLED: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_RESUME_WINDOW_EXPIRED: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_DAEMON_UNAVAILABLE: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_INTERNAL: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_FRESHNESS_DEADLINE: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_FRESHNESS_UNAVAILABLE: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_RESOURCE_RELEASED: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_RESULT_NOT_RETAINED: _ClassVar[SafeErrorCode]
    SAFE_ERROR_CODE_QUERY_HARD_LIMIT_EXCEEDED: _ClassVar[SafeErrorCode]

class SafeDiagnosticReference(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    SAFE_DIAGNOSTIC_REFERENCE_UNSPECIFIED: _ClassVar[SafeDiagnosticReference]
    SAFE_DIAGNOSTIC_REFERENCE_LIFECYCLE_FAILED_CLOSED: _ClassVar[SafeDiagnosticReference]
    SAFE_DIAGNOSTIC_REFERENCE_QUERY_CHALLENGE_REJECTED: _ClassVar[SafeDiagnosticReference]
    SAFE_DIAGNOSTIC_REFERENCE_QUERY_TERMINAL: _ClassVar[SafeDiagnosticReference]

class ProgressStage(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    PROGRESS_STAGE_UNSPECIFIED: _ClassVar[ProgressStage]
    PROGRESS_STAGE_EXECUTING: _ClassVar[ProgressStage]

class ResourceKind(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    RESOURCE_KIND_UNSPECIFIED: _ClassVar[ResourceKind]
    RESOURCE_KIND_RESULT_MANIFEST: _ClassVar[ResourceKind]
    RESOURCE_KIND_RESULT_PAGE: _ClassVar[ResourceKind]
    RESOURCE_KIND_REFERENCE: _ClassVar[ResourceKind]

class ReferenceKind(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    REFERENCE_KIND_UNSPECIFIED: _ClassVar[ReferenceKind]
    REFERENCE_KIND_CAPABILITY: _ClassVar[ReferenceKind]
    REFERENCE_KIND_GUIDE: _ClassVar[ReferenceKind]
    REFERENCE_KIND_RECIPE: _ClassVar[ReferenceKind]
    REFERENCE_KIND_REQUEST_SCHEMA: _ClassVar[ReferenceKind]
    REFERENCE_KIND_RESPONSE_SCHEMA: _ClassVar[ReferenceKind]
    REFERENCE_KIND_SNAPSHOT: _ClassVar[ReferenceKind]

class ReferenceTemplateVariable(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    REFERENCE_TEMPLATE_VARIABLE_UNSPECIFIED: _ClassVar[ReferenceTemplateVariable]
    REFERENCE_TEMPLATE_VARIABLE_KIND: _ClassVar[ReferenceTemplateVariable]
    REFERENCE_TEMPLATE_VARIABLE_RELEASED_VERSION: _ClassVar[ReferenceTemplateVariable]

class CancellationAcknowledgement(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CANCELLATION_ACKNOWLEDGEMENT_UNSPECIFIED: _ClassVar[CancellationAcknowledgement]
    CANCELLATION_ACKNOWLEDGEMENT_ACCEPTED: _ClassVar[CancellationAcknowledgement]
    CANCELLATION_ACKNOWLEDGEMENT_REPLAYED: _ClassVar[CancellationAcknowledgement]
    CANCELLATION_ACKNOWLEDGEMENT_ALREADY_TERMINAL: _ClassVar[CancellationAcknowledgement]
    CANCELLATION_ACKNOWLEDGEMENT_QUERY_NOT_FOUND: _ClassVar[CancellationAcknowledgement]

class ReleaseState(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    RELEASE_STATE_UNSPECIFIED: _ClassVar[ReleaseState]
    RELEASE_STATE_RELEASED: _ClassVar[ReleaseState]
    RELEASE_STATE_ALREADY_RELEASED: _ClassVar[ReleaseState]
    RELEASE_STATE_NOT_FOUND: _ClassVar[ReleaseState]

class ReservedControlOperation(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    RESERVED_CONTROL_OPERATION_UNSPECIFIED: _ClassVar[ReservedControlOperation]
    RESERVED_CONTROL_OPERATION_HANDSHAKE: _ClassVar[ReservedControlOperation]
    RESERVED_CONTROL_OPERATION_GET_STATUS: _ClassVar[ReservedControlOperation]
    RESERVED_CONTROL_OPERATION_CANCEL_QUERY: _ClassVar[ReservedControlOperation]
    RESERVED_CONTROL_OPERATION_RELEASE_RESOURCE: _ClassVar[ReservedControlOperation]

class SnapshotFreshness(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    SNAPSHOT_FRESHNESS_UNSPECIFIED: _ClassVar[SnapshotFreshness]
    SNAPSHOT_FRESHNESS_CURRENT: _ClassVar[SnapshotFreshness]
    SNAPSHOT_FRESHNESS_POTENTIALLY_STALE: _ClassVar[SnapshotFreshness]
    SNAPSHOT_FRESHNESS_UNAVAILABLE: _ClassVar[SnapshotFreshness]

class ProcessingState(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    PROCESSING_STATE_UNSPECIFIED: _ClassVar[ProcessingState]
    PROCESSING_STATE_PENDING: _ClassVar[ProcessingState]
    PROCESSING_STATE_RUNNING: _ClassVar[ProcessingState]
    PROCESSING_STATE_COMPLETE: _ClassVar[ProcessingState]
    PROCESSING_STATE_PARTIAL: _ClassVar[ProcessingState]
    PROCESSING_STATE_UNKNOWN: _ClassVar[ProcessingState]
    PROCESSING_STATE_UNAVAILABLE: _ClassVar[ProcessingState]
    PROCESSING_STATE_EXCLUDED: _ClassVar[ProcessingState]
    PROCESSING_STATE_LIMITED: _ClassVar[ProcessingState]
    PROCESSING_STATE_UNSUPPORTED: _ClassVar[ProcessingState]
    PROCESSING_STATE_FAILED: _ClassVar[ProcessingState]
    PROCESSING_STATE_CANCELLED: _ClassVar[ProcessingState]
LIFECYCLE_STATE_UNSPECIFIED: LifecycleState
LIFECYCLE_STATE_BOOTSTRAPPING: LifecycleState
LIFECYCLE_STATE_READY: LifecycleState
LIFECYCLE_STATE_DRAINING: LifecycleState
LIFECYCLE_STATE_FAILED_CLOSED: LifecycleState
QUERY_EXECUTION_STATE_UNSPECIFIED: QueryExecutionState
QUERY_EXECUTION_STATE_ACCEPTED: QueryExecutionState
QUERY_EXECUTION_STATE_QUEUED: QueryExecutionState
QUERY_EXECUTION_STATE_RUNNING: QueryExecutionState
QUERY_EXECUTION_STATE_SUCCEEDED: QueryExecutionState
QUERY_EXECUTION_STATE_FAILED: QueryExecutionState
QUERY_EXECUTION_STATE_CANCELLED: QueryExecutionState
QUERY_EXECUTION_STATE_LOST: QueryExecutionState
CHALLENGE_INPUT_KIND_UNSPECIFIED: ChallengeInputKind
CHALLENGE_INPUT_KIND_STRING: ChallengeInputKind
CHALLENGE_INPUT_KIND_INTEGER: ChallengeInputKind
CHALLENGE_INPUT_KIND_BOOLEAN: ChallengeInputKind
CHALLENGE_INPUT_KIND_ENUM: ChallengeInputKind
CHALLENGE_INPUT_KIND_STRING_COLLECTION: ChallengeInputKind
CHALLENGE_INPUT_KIND_INTEGER_COLLECTION: ChallengeInputKind
CHALLENGE_INPUT_KIND_BOOLEAN_COLLECTION: ChallengeInputKind
CHALLENGE_INPUT_KIND_ENUM_COLLECTION: ChallengeInputKind
CHALLENGE_COLLECTION_ITEM_KIND_UNSPECIFIED: ChallengeCollectionItemKind
CHALLENGE_COLLECTION_ITEM_KIND_STRING: ChallengeCollectionItemKind
CHALLENGE_COLLECTION_ITEM_KIND_INTEGER: ChallengeCollectionItemKind
CHALLENGE_COLLECTION_ITEM_KIND_BOOLEAN: ChallengeCollectionItemKind
CHALLENGE_COLLECTION_ITEM_KIND_ENUM: ChallengeCollectionItemKind
CHALLENGE_STRING_FORMAT_UNSPECIFIED: ChallengeStringFormat
CHALLENGE_STRING_FORMAT_PLAIN: ChallengeStringFormat
CHALLENGE_STRING_FORMAT_IDENTIFIER: ChallengeStringFormat
CHALLENGE_STRING_FORMAT_RELEASE_VERSION: ChallengeStringFormat
CHALLENGE_EXPLANATION_CODE_UNSPECIFIED: ChallengeExplanationCode
CHALLENGE_EXPLANATION_CODE_REQUIRED_INPUT_MISSING: ChallengeExplanationCode
CHALLENGE_EXPLANATION_CODE_REFERENCE_AMBIGUOUS: ChallengeExplanationCode
CHALLENGE_EXPLANATION_CODE_BOUNDED_SELECTION_REQUIRED: ChallengeExplanationCode
SAFE_ERROR_LAYER_UNSPECIFIED: SafeErrorLayer
SAFE_ERROR_LAYER_TRANSPORT: SafeErrorLayer
SAFE_ERROR_LAYER_AUTHORIZATION: SafeErrorLayer
SAFE_ERROR_LAYER_VALIDATION: SafeErrorLayer
SAFE_ERROR_LAYER_QUERY: SafeErrorLayer
SAFE_ERROR_LAYER_RESOURCE: SafeErrorLayer
SAFE_ERROR_LAYER_LIFECYCLE: SafeErrorLayer
SAFE_ERROR_CODE_UNSPECIFIED: SafeErrorCode
SAFE_ERROR_CODE_INVALID_REQUEST: SafeErrorCode
SAFE_ERROR_CODE_VALIDATION_REJECTED: SafeErrorCode
SAFE_ERROR_CODE_INPUT_REQUIRED: SafeErrorCode
SAFE_ERROR_CODE_NOT_AUTHORIZED: SafeErrorCode
SAFE_ERROR_CODE_IDEMPOTENCY_CONFLICT: SafeErrorCode
SAFE_ERROR_CODE_CONTINUATION_EXPIRED: SafeErrorCode
SAFE_ERROR_CODE_CONTINUATION_REPLAYED: SafeErrorCode
SAFE_ERROR_CODE_GENERATION_MISMATCH: SafeErrorCode
SAFE_ERROR_CODE_QUERY_NOT_FOUND: SafeErrorCode
SAFE_ERROR_CODE_RESOURCE_NOT_FOUND: SafeErrorCode
SAFE_ERROR_CODE_RESOURCE_EXPIRED: SafeErrorCode
SAFE_ERROR_CODE_RANGE_NOT_SATISFIABLE: SafeErrorCode
SAFE_ERROR_CODE_CAPACITY_UNAVAILABLE: SafeErrorCode
SAFE_ERROR_CODE_CANCELLED: SafeErrorCode
SAFE_ERROR_CODE_RESUME_WINDOW_EXPIRED: SafeErrorCode
SAFE_ERROR_CODE_DAEMON_UNAVAILABLE: SafeErrorCode
SAFE_ERROR_CODE_INTERNAL: SafeErrorCode
SAFE_ERROR_CODE_FRESHNESS_DEADLINE: SafeErrorCode
SAFE_ERROR_CODE_FRESHNESS_UNAVAILABLE: SafeErrorCode
SAFE_ERROR_CODE_RESOURCE_RELEASED: SafeErrorCode
SAFE_ERROR_CODE_RESULT_NOT_RETAINED: SafeErrorCode
SAFE_ERROR_CODE_QUERY_HARD_LIMIT_EXCEEDED: SafeErrorCode
SAFE_DIAGNOSTIC_REFERENCE_UNSPECIFIED: SafeDiagnosticReference
SAFE_DIAGNOSTIC_REFERENCE_LIFECYCLE_FAILED_CLOSED: SafeDiagnosticReference
SAFE_DIAGNOSTIC_REFERENCE_QUERY_CHALLENGE_REJECTED: SafeDiagnosticReference
SAFE_DIAGNOSTIC_REFERENCE_QUERY_TERMINAL: SafeDiagnosticReference
PROGRESS_STAGE_UNSPECIFIED: ProgressStage
PROGRESS_STAGE_EXECUTING: ProgressStage
RESOURCE_KIND_UNSPECIFIED: ResourceKind
RESOURCE_KIND_RESULT_MANIFEST: ResourceKind
RESOURCE_KIND_RESULT_PAGE: ResourceKind
RESOURCE_KIND_REFERENCE: ResourceKind
REFERENCE_KIND_UNSPECIFIED: ReferenceKind
REFERENCE_KIND_CAPABILITY: ReferenceKind
REFERENCE_KIND_GUIDE: ReferenceKind
REFERENCE_KIND_RECIPE: ReferenceKind
REFERENCE_KIND_REQUEST_SCHEMA: ReferenceKind
REFERENCE_KIND_RESPONSE_SCHEMA: ReferenceKind
REFERENCE_KIND_SNAPSHOT: ReferenceKind
REFERENCE_TEMPLATE_VARIABLE_UNSPECIFIED: ReferenceTemplateVariable
REFERENCE_TEMPLATE_VARIABLE_KIND: ReferenceTemplateVariable
REFERENCE_TEMPLATE_VARIABLE_RELEASED_VERSION: ReferenceTemplateVariable
CANCELLATION_ACKNOWLEDGEMENT_UNSPECIFIED: CancellationAcknowledgement
CANCELLATION_ACKNOWLEDGEMENT_ACCEPTED: CancellationAcknowledgement
CANCELLATION_ACKNOWLEDGEMENT_REPLAYED: CancellationAcknowledgement
CANCELLATION_ACKNOWLEDGEMENT_ALREADY_TERMINAL: CancellationAcknowledgement
CANCELLATION_ACKNOWLEDGEMENT_QUERY_NOT_FOUND: CancellationAcknowledgement
RELEASE_STATE_UNSPECIFIED: ReleaseState
RELEASE_STATE_RELEASED: ReleaseState
RELEASE_STATE_ALREADY_RELEASED: ReleaseState
RELEASE_STATE_NOT_FOUND: ReleaseState
RESERVED_CONTROL_OPERATION_UNSPECIFIED: ReservedControlOperation
RESERVED_CONTROL_OPERATION_HANDSHAKE: ReservedControlOperation
RESERVED_CONTROL_OPERATION_GET_STATUS: ReservedControlOperation
RESERVED_CONTROL_OPERATION_CANCEL_QUERY: ReservedControlOperation
RESERVED_CONTROL_OPERATION_RELEASE_RESOURCE: ReservedControlOperation
SNAPSHOT_FRESHNESS_UNSPECIFIED: SnapshotFreshness
SNAPSHOT_FRESHNESS_CURRENT: SnapshotFreshness
SNAPSHOT_FRESHNESS_POTENTIALLY_STALE: SnapshotFreshness
SNAPSHOT_FRESHNESS_UNAVAILABLE: SnapshotFreshness
PROCESSING_STATE_UNSPECIFIED: ProcessingState
PROCESSING_STATE_PENDING: ProcessingState
PROCESSING_STATE_RUNNING: ProcessingState
PROCESSING_STATE_COMPLETE: ProcessingState
PROCESSING_STATE_PARTIAL: ProcessingState
PROCESSING_STATE_UNKNOWN: ProcessingState
PROCESSING_STATE_UNAVAILABLE: ProcessingState
PROCESSING_STATE_EXCLUDED: ProcessingState
PROCESSING_STATE_LIMITED: ProcessingState
PROCESSING_STATE_UNSUPPORTED: ProcessingState
PROCESSING_STATE_FAILED: ProcessingState
PROCESSING_STATE_CANCELLED: ProcessingState

class RequestContext(_message.Message):
    __slots__ = ("correlation_id", "remaining_budget")
    CORRELATION_ID_FIELD_NUMBER: _ClassVar[int]
    REMAINING_BUDGET_FIELD_NUMBER: _ClassVar[int]
    correlation_id: str
    remaining_budget: _duration_pb2.Duration
    def __init__(self, correlation_id: _Optional[str] = ..., remaining_budget: _Optional[_Union[datetime.timedelta, _duration_pb2.Duration, _Mapping]] = ...) -> None: ...

class AuthorityGeneration(_message.Message):
    __slots__ = ("session_id", "session_generation", "daemon_generation", "supervisor_generation", "policy_generation", "revocation_generation")
    SESSION_ID_FIELD_NUMBER: _ClassVar[int]
    SESSION_GENERATION_FIELD_NUMBER: _ClassVar[int]
    DAEMON_GENERATION_FIELD_NUMBER: _ClassVar[int]
    SUPERVISOR_GENERATION_FIELD_NUMBER: _ClassVar[int]
    POLICY_GENERATION_FIELD_NUMBER: _ClassVar[int]
    REVOCATION_GENERATION_FIELD_NUMBER: _ClassVar[int]
    session_id: str
    session_generation: int
    daemon_generation: int
    supervisor_generation: int
    policy_generation: int
    revocation_generation: int
    def __init__(self, session_id: _Optional[str] = ..., session_generation: _Optional[int] = ..., daemon_generation: _Optional[int] = ..., supervisor_generation: _Optional[int] = ..., policy_generation: _Optional[int] = ..., revocation_generation: _Optional[int] = ...) -> None: ...

class ReservedControlContract(_message.Message):
    __slots__ = ("reserved_capacity", "operations")
    RESERVED_CAPACITY_FIELD_NUMBER: _ClassVar[int]
    OPERATIONS_FIELD_NUMBER: _ClassVar[int]
    reserved_capacity: int
    operations: _containers.RepeatedScalarFieldContainer[ReservedControlOperation]
    def __init__(self, reserved_capacity: _Optional[int] = ..., operations: _Optional[_Iterable[_Union[ReservedControlOperation, str]]] = ...) -> None: ...

class EffectiveLimits(_message.Message):
    __slots__ = ("maximum_control_message_bytes", "maximum_resource_chunk_bytes", "maximum_result_bytes", "maximum_result_pages", "maximum_concurrent_queries", "maximum_watch_events", "maximum_challenge_fields", "maximum_choices_per_field", "maximum_challenge_rounds", "maximum_reference_completion_candidates", "maximum_validation_issues")
    MAXIMUM_CONTROL_MESSAGE_BYTES_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_RESOURCE_CHUNK_BYTES_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_RESULT_BYTES_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_RESULT_PAGES_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_CONCURRENT_QUERIES_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_WATCH_EVENTS_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_CHALLENGE_FIELDS_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_CHOICES_PER_FIELD_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_CHALLENGE_ROUNDS_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_REFERENCE_COMPLETION_CANDIDATES_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_VALIDATION_ISSUES_FIELD_NUMBER: _ClassVar[int]
    maximum_control_message_bytes: int
    maximum_resource_chunk_bytes: int
    maximum_result_bytes: int
    maximum_result_pages: int
    maximum_concurrent_queries: int
    maximum_watch_events: int
    maximum_challenge_fields: int
    maximum_choices_per_field: int
    maximum_challenge_rounds: int
    maximum_reference_completion_candidates: int
    maximum_validation_issues: int
    def __init__(self, maximum_control_message_bytes: _Optional[int] = ..., maximum_resource_chunk_bytes: _Optional[int] = ..., maximum_result_bytes: _Optional[int] = ..., maximum_result_pages: _Optional[int] = ..., maximum_concurrent_queries: _Optional[int] = ..., maximum_watch_events: _Optional[int] = ..., maximum_challenge_fields: _Optional[int] = ..., maximum_choices_per_field: _Optional[int] = ..., maximum_challenge_rounds: _Optional[int] = ..., maximum_reference_completion_candidates: _Optional[int] = ..., maximum_validation_issues: _Optional[int] = ...) -> None: ...

class HandshakeRequest(_message.Message):
    __slots__ = ("launch_grant", "adapter_version", "minimum_minor", "maximum_minor", "required_feature_bits", "optional_feature_bits", "desired_semantic_profiles", "maximum_resource_chunk_bytes", "remaining_budget", "correlation_id")
    LAUNCH_GRANT_FIELD_NUMBER: _ClassVar[int]
    ADAPTER_VERSION_FIELD_NUMBER: _ClassVar[int]
    MINIMUM_MINOR_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_MINOR_FIELD_NUMBER: _ClassVar[int]
    REQUIRED_FEATURE_BITS_FIELD_NUMBER: _ClassVar[int]
    OPTIONAL_FEATURE_BITS_FIELD_NUMBER: _ClassVar[int]
    DESIRED_SEMANTIC_PROFILES_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_RESOURCE_CHUNK_BYTES_FIELD_NUMBER: _ClassVar[int]
    REMAINING_BUDGET_FIELD_NUMBER: _ClassVar[int]
    CORRELATION_ID_FIELD_NUMBER: _ClassVar[int]
    launch_grant: bytes
    adapter_version: str
    minimum_minor: int
    maximum_minor: int
    required_feature_bits: int
    optional_feature_bits: int
    desired_semantic_profiles: _containers.RepeatedScalarFieldContainer[str]
    maximum_resource_chunk_bytes: int
    remaining_budget: _duration_pb2.Duration
    correlation_id: str
    def __init__(self, launch_grant: _Optional[bytes] = ..., adapter_version: _Optional[str] = ..., minimum_minor: _Optional[int] = ..., maximum_minor: _Optional[int] = ..., required_feature_bits: _Optional[int] = ..., optional_feature_bits: _Optional[int] = ..., desired_semantic_profiles: _Optional[_Iterable[str]] = ..., maximum_resource_chunk_bytes: _Optional[int] = ..., remaining_budget: _Optional[_Union[datetime.timedelta, _duration_pb2.Duration, _Mapping]] = ..., correlation_id: _Optional[str] = ...) -> None: ...

class HandshakeResponse(_message.Message):
    __slots__ = ("session_token", "authority", "selected_minor", "selected_feature_bits", "selected_semantic_profile", "lifecycle", "effective_limits", "session_expires_at_unix_ms", "reference_index_revision", "reserved_control")
    SESSION_TOKEN_FIELD_NUMBER: _ClassVar[int]
    AUTHORITY_FIELD_NUMBER: _ClassVar[int]
    SELECTED_MINOR_FIELD_NUMBER: _ClassVar[int]
    SELECTED_FEATURE_BITS_FIELD_NUMBER: _ClassVar[int]
    SELECTED_SEMANTIC_PROFILE_FIELD_NUMBER: _ClassVar[int]
    LIFECYCLE_FIELD_NUMBER: _ClassVar[int]
    EFFECTIVE_LIMITS_FIELD_NUMBER: _ClassVar[int]
    SESSION_EXPIRES_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    REFERENCE_INDEX_REVISION_FIELD_NUMBER: _ClassVar[int]
    RESERVED_CONTROL_FIELD_NUMBER: _ClassVar[int]
    session_token: bytes
    authority: AuthorityGeneration
    selected_minor: int
    selected_feature_bits: int
    selected_semantic_profile: str
    lifecycle: LifecycleState
    effective_limits: EffectiveLimits
    session_expires_at_unix_ms: int
    reference_index_revision: str
    reserved_control: ReservedControlContract
    def __init__(self, session_token: _Optional[bytes] = ..., authority: _Optional[_Union[AuthorityGeneration, _Mapping]] = ..., selected_minor: _Optional[int] = ..., selected_feature_bits: _Optional[int] = ..., selected_semantic_profile: _Optional[str] = ..., lifecycle: _Optional[_Union[LifecycleState, str]] = ..., effective_limits: _Optional[_Union[EffectiveLimits, _Mapping]] = ..., session_expires_at_unix_ms: _Optional[int] = ..., reference_index_revision: _Optional[str] = ..., reserved_control: _Optional[_Union[ReservedControlContract, _Mapping]] = ...) -> None: ...

class SafeErrorMetadata(_message.Message):
    __slots__ = ("code", "layer", "retryable", "retry_after_ms", "diagnostic_reference", "correlation_id")
    CODE_FIELD_NUMBER: _ClassVar[int]
    LAYER_FIELD_NUMBER: _ClassVar[int]
    RETRYABLE_FIELD_NUMBER: _ClassVar[int]
    RETRY_AFTER_MS_FIELD_NUMBER: _ClassVar[int]
    DIAGNOSTIC_REFERENCE_FIELD_NUMBER: _ClassVar[int]
    CORRELATION_ID_FIELD_NUMBER: _ClassVar[int]
    code: SafeErrorCode
    layer: SafeErrorLayer
    retryable: bool
    retry_after_ms: int
    diagnostic_reference: SafeDiagnosticReference
    correlation_id: str
    def __init__(self, code: _Optional[_Union[SafeErrorCode, str]] = ..., layer: _Optional[_Union[SafeErrorLayer, str]] = ..., retryable: _Optional[bool] = ..., retry_after_ms: _Optional[int] = ..., diagnostic_reference: _Optional[_Union[SafeDiagnosticReference, str]] = ..., correlation_id: _Optional[str] = ...) -> None: ...

class GetStatusRequest(_message.Message):
    __slots__ = ("context", "include_diagnostics")
    CONTEXT_FIELD_NUMBER: _ClassVar[int]
    INCLUDE_DIAGNOSTICS_FIELD_NUMBER: _ClassVar[int]
    context: RequestContext
    include_diagnostics: bool
    def __init__(self, context: _Optional[_Union[RequestContext, _Mapping]] = ..., include_diagnostics: _Optional[bool] = ...) -> None: ...

class GetStatusResponse(_message.Message):
    __slots__ = ("authority", "lifecycle", "lifecycle_sequence", "failure", "active_epoch_id", "running_queries", "queued_queries", "canonical_public_status_json", "source_observations")
    AUTHORITY_FIELD_NUMBER: _ClassVar[int]
    LIFECYCLE_FIELD_NUMBER: _ClassVar[int]
    LIFECYCLE_SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    FAILURE_FIELD_NUMBER: _ClassVar[int]
    ACTIVE_EPOCH_ID_FIELD_NUMBER: _ClassVar[int]
    RUNNING_QUERIES_FIELD_NUMBER: _ClassVar[int]
    QUEUED_QUERIES_FIELD_NUMBER: _ClassVar[int]
    CANONICAL_PUBLIC_STATUS_JSON_FIELD_NUMBER: _ClassVar[int]
    SOURCE_OBSERVATIONS_FIELD_NUMBER: _ClassVar[int]
    authority: AuthorityGeneration
    lifecycle: LifecycleState
    lifecycle_sequence: int
    failure: SafeErrorMetadata
    active_epoch_id: str
    running_queries: int
    queued_queries: int
    canonical_public_status_json: bytes
    source_observations: _containers.RepeatedCompositeFieldContainer[WorkspaceSourceObservation]
    def __init__(self, authority: _Optional[_Union[AuthorityGeneration, _Mapping]] = ..., lifecycle: _Optional[_Union[LifecycleState, str]] = ..., lifecycle_sequence: _Optional[int] = ..., failure: _Optional[_Union[SafeErrorMetadata, _Mapping]] = ..., active_epoch_id: _Optional[str] = ..., running_queries: _Optional[int] = ..., queued_queries: _Optional[int] = ..., canonical_public_status_json: _Optional[bytes] = ..., source_observations: _Optional[_Iterable[_Union[WorkspaceSourceObservation, _Mapping]]] = ...) -> None: ...

class WorkspaceSourceObservation(_message.Message):
    __slots__ = ("workspace_id", "selected_source_generation", "requested_watermark", "reconciled_watermark", "freshness", "watch_healthy", "rescan_required", "runnable_pending", "source_reconciled_watermark", "source_freshness", "semantic_pending")
    WORKSPACE_ID_FIELD_NUMBER: _ClassVar[int]
    SELECTED_SOURCE_GENERATION_FIELD_NUMBER: _ClassVar[int]
    REQUESTED_WATERMARK_FIELD_NUMBER: _ClassVar[int]
    RECONCILED_WATERMARK_FIELD_NUMBER: _ClassVar[int]
    FRESHNESS_FIELD_NUMBER: _ClassVar[int]
    WATCH_HEALTHY_FIELD_NUMBER: _ClassVar[int]
    RESCAN_REQUIRED_FIELD_NUMBER: _ClassVar[int]
    RUNNABLE_PENDING_FIELD_NUMBER: _ClassVar[int]
    SOURCE_RECONCILED_WATERMARK_FIELD_NUMBER: _ClassVar[int]
    SOURCE_FRESHNESS_FIELD_NUMBER: _ClassVar[int]
    SEMANTIC_PENDING_FIELD_NUMBER: _ClassVar[int]
    workspace_id: str
    selected_source_generation: int
    requested_watermark: int
    reconciled_watermark: int
    freshness: SnapshotFreshness
    watch_healthy: bool
    rescan_required: bool
    runnable_pending: bool
    source_reconciled_watermark: int
    source_freshness: SnapshotFreshness
    semantic_pending: bool
    def __init__(self, workspace_id: _Optional[str] = ..., selected_source_generation: _Optional[int] = ..., requested_watermark: _Optional[int] = ..., reconciled_watermark: _Optional[int] = ..., freshness: _Optional[_Union[SnapshotFreshness, str]] = ..., watch_healthy: _Optional[bool] = ..., rescan_required: _Optional[bool] = ..., runnable_pending: _Optional[bool] = ..., source_reconciled_watermark: _Optional[int] = ..., source_freshness: _Optional[_Union[SnapshotFreshness, str]] = ..., semantic_pending: _Optional[bool] = ...) -> None: ...

class ReferenceReadRequest(_message.Message):
    __slots__ = ("kind", "version")
    KIND_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    kind: ReferenceKind
    version: str
    def __init__(self, kind: _Optional[_Union[ReferenceKind, str]] = ..., version: _Optional[str] = ...) -> None: ...

class ReferenceCompletionRequest(_message.Message):
    __slots__ = ("variable", "prefix", "kind", "selector", "maximum_candidates")
    VARIABLE_FIELD_NUMBER: _ClassVar[int]
    PREFIX_FIELD_NUMBER: _ClassVar[int]
    KIND_FIELD_NUMBER: _ClassVar[int]
    SELECTOR_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_CANDIDATES_FIELD_NUMBER: _ClassVar[int]
    variable: ReferenceTemplateVariable
    prefix: str
    kind: ReferenceKind
    selector: str
    maximum_candidates: int
    def __init__(self, variable: _Optional[_Union[ReferenceTemplateVariable, str]] = ..., prefix: _Optional[str] = ..., kind: _Optional[_Union[ReferenceKind, str]] = ..., selector: _Optional[str] = ..., maximum_candidates: _Optional[int] = ...) -> None: ...

class GetReferenceRequest(_message.Message):
    __slots__ = ("context", "read", "completion")
    CONTEXT_FIELD_NUMBER: _ClassVar[int]
    READ_FIELD_NUMBER: _ClassVar[int]
    COMPLETION_FIELD_NUMBER: _ClassVar[int]
    context: RequestContext
    read: ReferenceReadRequest
    completion: ReferenceCompletionRequest
    def __init__(self, context: _Optional[_Union[RequestContext, _Mapping]] = ..., read: _Optional[_Union[ReferenceReadRequest, _Mapping]] = ..., completion: _Optional[_Union[ReferenceCompletionRequest, _Mapping]] = ...) -> None: ...

class ReferenceDocument(_message.Message):
    __slots__ = ("reference_id", "resource")
    REFERENCE_ID_FIELD_NUMBER: _ClassVar[int]
    RESOURCE_FIELD_NUMBER: _ClassVar[int]
    reference_id: str
    resource: ResourceDescriptor
    def __init__(self, reference_id: _Optional[str] = ..., resource: _Optional[_Union[ResourceDescriptor, _Mapping]] = ...) -> None: ...

class ReferenceCompletionCandidate(_message.Message):
    __slots__ = ("value", "presentation_key")
    VALUE_FIELD_NUMBER: _ClassVar[int]
    PRESENTATION_KEY_FIELD_NUMBER: _ClassVar[int]
    value: str
    presentation_key: str
    def __init__(self, value: _Optional[str] = ..., presentation_key: _Optional[str] = ...) -> None: ...

class ReferenceCompletion(_message.Message):
    __slots__ = ("candidates", "total", "has_more")
    CANDIDATES_FIELD_NUMBER: _ClassVar[int]
    TOTAL_FIELD_NUMBER: _ClassVar[int]
    HAS_MORE_FIELD_NUMBER: _ClassVar[int]
    candidates: _containers.RepeatedCompositeFieldContainer[ReferenceCompletionCandidate]
    total: int
    has_more: bool
    def __init__(self, candidates: _Optional[_Iterable[_Union[ReferenceCompletionCandidate, _Mapping]]] = ..., total: _Optional[int] = ..., has_more: _Optional[bool] = ...) -> None: ...

class GetReferenceResponse(_message.Message):
    __slots__ = ("authority", "reference", "completion")
    AUTHORITY_FIELD_NUMBER: _ClassVar[int]
    REFERENCE_FIELD_NUMBER: _ClassVar[int]
    COMPLETION_FIELD_NUMBER: _ClassVar[int]
    authority: AuthorityGeneration
    reference: ReferenceDocument
    completion: ReferenceCompletion
    def __init__(self, authority: _Optional[_Union[AuthorityGeneration, _Mapping]] = ..., reference: _Optional[_Union[ReferenceDocument, _Mapping]] = ..., completion: _Optional[_Union[ReferenceCompletion, _Mapping]] = ...) -> None: ...

class ResultLimits(_message.Message):
    __slots__ = ("maximum_result_bytes", "maximum_result_pages")
    MAXIMUM_RESULT_BYTES_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_RESULT_PAGES_FIELD_NUMBER: _ClassVar[int]
    maximum_result_bytes: int
    maximum_result_pages: int
    def __init__(self, maximum_result_bytes: _Optional[int] = ..., maximum_result_pages: _Optional[int] = ...) -> None: ...

class QuerySubmission(_message.Message):
    __slots__ = ("canonical_request_json", "request_checksum", "semantic_request_id", "semantic_profile", "result_limits")
    CANONICAL_REQUEST_JSON_FIELD_NUMBER: _ClassVar[int]
    REQUEST_CHECKSUM_FIELD_NUMBER: _ClassVar[int]
    SEMANTIC_REQUEST_ID_FIELD_NUMBER: _ClassVar[int]
    SEMANTIC_PROFILE_FIELD_NUMBER: _ClassVar[int]
    RESULT_LIMITS_FIELD_NUMBER: _ClassVar[int]
    canonical_request_json: bytes
    request_checksum: str
    semantic_request_id: str
    semantic_profile: str
    result_limits: ResultLimits
    def __init__(self, canonical_request_json: _Optional[bytes] = ..., request_checksum: _Optional[str] = ..., semantic_request_id: _Optional[str] = ..., semantic_profile: _Optional[str] = ..., result_limits: _Optional[_Union[ResultLimits, _Mapping]] = ...) -> None: ...

class ChallengeStringConstraints(_message.Message):
    __slots__ = ("minimum_length", "maximum_length", "format")
    MINIMUM_LENGTH_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_LENGTH_FIELD_NUMBER: _ClassVar[int]
    FORMAT_FIELD_NUMBER: _ClassVar[int]
    minimum_length: int
    maximum_length: int
    format: ChallengeStringFormat
    def __init__(self, minimum_length: _Optional[int] = ..., maximum_length: _Optional[int] = ..., format: _Optional[_Union[ChallengeStringFormat, str]] = ...) -> None: ...

class ChallengeIntegerConstraints(_message.Message):
    __slots__ = ("minimum", "maximum")
    MINIMUM_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_FIELD_NUMBER: _ClassVar[int]
    minimum: int
    maximum: int
    def __init__(self, minimum: _Optional[int] = ..., maximum: _Optional[int] = ...) -> None: ...

class ChallengeEnumConstraints(_message.Message):
    __slots__ = ("minimum_selections", "maximum_selections")
    MINIMUM_SELECTIONS_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_SELECTIONS_FIELD_NUMBER: _ClassVar[int]
    minimum_selections: int
    maximum_selections: int
    def __init__(self, minimum_selections: _Optional[int] = ..., maximum_selections: _Optional[int] = ...) -> None: ...

class ChallengeCollectionConstraints(_message.Message):
    __slots__ = ("item_kind", "minimum_items", "maximum_items", "unique_items")
    ITEM_KIND_FIELD_NUMBER: _ClassVar[int]
    MINIMUM_ITEMS_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_ITEMS_FIELD_NUMBER: _ClassVar[int]
    UNIQUE_ITEMS_FIELD_NUMBER: _ClassVar[int]
    item_kind: ChallengeCollectionItemKind
    minimum_items: int
    maximum_items: int
    unique_items: bool
    def __init__(self, item_kind: _Optional[_Union[ChallengeCollectionItemKind, str]] = ..., minimum_items: _Optional[int] = ..., maximum_items: _Optional[int] = ..., unique_items: _Optional[bool] = ...) -> None: ...

class ChallengeConstraints(_message.Message):
    __slots__ = ("string_constraints", "integer_constraints", "enum_constraints", "collection_constraints")
    STRING_CONSTRAINTS_FIELD_NUMBER: _ClassVar[int]
    INTEGER_CONSTRAINTS_FIELD_NUMBER: _ClassVar[int]
    ENUM_CONSTRAINTS_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_CONSTRAINTS_FIELD_NUMBER: _ClassVar[int]
    string_constraints: ChallengeStringConstraints
    integer_constraints: ChallengeIntegerConstraints
    enum_constraints: ChallengeEnumConstraints
    collection_constraints: ChallengeCollectionConstraints
    def __init__(self, string_constraints: _Optional[_Union[ChallengeStringConstraints, _Mapping]] = ..., integer_constraints: _Optional[_Union[ChallengeIntegerConstraints, _Mapping]] = ..., enum_constraints: _Optional[_Union[ChallengeEnumConstraints, _Mapping]] = ..., collection_constraints: _Optional[_Union[ChallengeCollectionConstraints, _Mapping]] = ...) -> None: ...

class AuthorizedChoice(_message.Message):
    __slots__ = ("choice_id", "presentation_key", "string_value", "integer_value", "boolean_value")
    CHOICE_ID_FIELD_NUMBER: _ClassVar[int]
    PRESENTATION_KEY_FIELD_NUMBER: _ClassVar[int]
    STRING_VALUE_FIELD_NUMBER: _ClassVar[int]
    INTEGER_VALUE_FIELD_NUMBER: _ClassVar[int]
    BOOLEAN_VALUE_FIELD_NUMBER: _ClassVar[int]
    choice_id: str
    presentation_key: str
    string_value: str
    integer_value: int
    boolean_value: bool
    def __init__(self, choice_id: _Optional[str] = ..., presentation_key: _Optional[str] = ..., string_value: _Optional[str] = ..., integer_value: _Optional[int] = ..., boolean_value: _Optional[bool] = ...) -> None: ...

class InputRequirement(_message.Message):
    __slots__ = ("semantic_field_id", "input_kind", "presentation_key", "description_key", "required", "constraints", "authorized_choices")
    SEMANTIC_FIELD_ID_FIELD_NUMBER: _ClassVar[int]
    INPUT_KIND_FIELD_NUMBER: _ClassVar[int]
    PRESENTATION_KEY_FIELD_NUMBER: _ClassVar[int]
    DESCRIPTION_KEY_FIELD_NUMBER: _ClassVar[int]
    REQUIRED_FIELD_NUMBER: _ClassVar[int]
    CONSTRAINTS_FIELD_NUMBER: _ClassVar[int]
    AUTHORIZED_CHOICES_FIELD_NUMBER: _ClassVar[int]
    semantic_field_id: str
    input_kind: ChallengeInputKind
    presentation_key: str
    description_key: str
    required: bool
    constraints: ChallengeConstraints
    authorized_choices: _containers.RepeatedCompositeFieldContainer[AuthorizedChoice]
    def __init__(self, semantic_field_id: _Optional[str] = ..., input_kind: _Optional[_Union[ChallengeInputKind, str]] = ..., presentation_key: _Optional[str] = ..., description_key: _Optional[str] = ..., required: _Optional[bool] = ..., constraints: _Optional[_Union[ChallengeConstraints, _Mapping]] = ..., authorized_choices: _Optional[_Iterable[_Union[AuthorizedChoice, _Mapping]]] = ...) -> None: ...

class StringAnswerCollection(_message.Message):
    __slots__ = ("values",)
    VALUES_FIELD_NUMBER: _ClassVar[int]
    values: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, values: _Optional[_Iterable[str]] = ...) -> None: ...

class IntegerAnswerCollection(_message.Message):
    __slots__ = ("values",)
    VALUES_FIELD_NUMBER: _ClassVar[int]
    values: _containers.RepeatedScalarFieldContainer[int]
    def __init__(self, values: _Optional[_Iterable[int]] = ...) -> None: ...

class BooleanAnswerCollection(_message.Message):
    __slots__ = ("values",)
    VALUES_FIELD_NUMBER: _ClassVar[int]
    values: _containers.RepeatedScalarFieldContainer[bool]
    def __init__(self, values: _Optional[_Iterable[bool]] = ...) -> None: ...

class ChoiceAnswerCollection(_message.Message):
    __slots__ = ("choice_ids",)
    CHOICE_IDS_FIELD_NUMBER: _ClassVar[int]
    choice_ids: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, choice_ids: _Optional[_Iterable[str]] = ...) -> None: ...

class InputAnswer(_message.Message):
    __slots__ = ("semantic_field_id", "string_value", "integer_value", "boolean_value", "choice_id", "string_collection", "integer_collection", "boolean_collection", "choice_collection")
    SEMANTIC_FIELD_ID_FIELD_NUMBER: _ClassVar[int]
    STRING_VALUE_FIELD_NUMBER: _ClassVar[int]
    INTEGER_VALUE_FIELD_NUMBER: _ClassVar[int]
    BOOLEAN_VALUE_FIELD_NUMBER: _ClassVar[int]
    CHOICE_ID_FIELD_NUMBER: _ClassVar[int]
    STRING_COLLECTION_FIELD_NUMBER: _ClassVar[int]
    INTEGER_COLLECTION_FIELD_NUMBER: _ClassVar[int]
    BOOLEAN_COLLECTION_FIELD_NUMBER: _ClassVar[int]
    CHOICE_COLLECTION_FIELD_NUMBER: _ClassVar[int]
    semantic_field_id: str
    string_value: str
    integer_value: int
    boolean_value: bool
    choice_id: str
    string_collection: StringAnswerCollection
    integer_collection: IntegerAnswerCollection
    boolean_collection: BooleanAnswerCollection
    choice_collection: ChoiceAnswerCollection
    def __init__(self, semantic_field_id: _Optional[str] = ..., string_value: _Optional[str] = ..., integer_value: _Optional[int] = ..., boolean_value: _Optional[bool] = ..., choice_id: _Optional[str] = ..., string_collection: _Optional[_Union[StringAnswerCollection, _Mapping]] = ..., integer_collection: _Optional[_Union[IntegerAnswerCollection, _Mapping]] = ..., boolean_collection: _Optional[_Union[BooleanAnswerCollection, _Mapping]] = ..., choice_collection: _Optional[_Union[ChoiceAnswerCollection, _Mapping]] = ...) -> None: ...

class ValidationIssue(_message.Message):
    __slots__ = ("code", "semantic_field_id", "presentation_key", "retryable")
    CODE_FIELD_NUMBER: _ClassVar[int]
    SEMANTIC_FIELD_ID_FIELD_NUMBER: _ClassVar[int]
    PRESENTATION_KEY_FIELD_NUMBER: _ClassVar[int]
    RETRYABLE_FIELD_NUMBER: _ClassVar[int]
    code: SafeErrorCode
    semantic_field_id: str
    presentation_key: str
    retryable: bool
    def __init__(self, code: _Optional[_Union[SafeErrorCode, str]] = ..., semantic_field_id: _Optional[str] = ..., presentation_key: _Optional[str] = ..., retryable: _Optional[bool] = ...) -> None: ...

class QueryPreparation(_message.Message):
    __slots__ = ("canonical_normalized_request_json", "semantic_request_id", "input_requirements", "errors", "warnings", "cost_class", "estimated_result_bytes", "estimated_result_pages")
    CANONICAL_NORMALIZED_REQUEST_JSON_FIELD_NUMBER: _ClassVar[int]
    SEMANTIC_REQUEST_ID_FIELD_NUMBER: _ClassVar[int]
    INPUT_REQUIREMENTS_FIELD_NUMBER: _ClassVar[int]
    ERRORS_FIELD_NUMBER: _ClassVar[int]
    WARNINGS_FIELD_NUMBER: _ClassVar[int]
    COST_CLASS_FIELD_NUMBER: _ClassVar[int]
    ESTIMATED_RESULT_BYTES_FIELD_NUMBER: _ClassVar[int]
    ESTIMATED_RESULT_PAGES_FIELD_NUMBER: _ClassVar[int]
    canonical_normalized_request_json: bytes
    semantic_request_id: str
    input_requirements: _containers.RepeatedCompositeFieldContainer[InputRequirement]
    errors: _containers.RepeatedCompositeFieldContainer[ValidationIssue]
    warnings: _containers.RepeatedCompositeFieldContainer[ValidationIssue]
    cost_class: str
    estimated_result_bytes: int
    estimated_result_pages: int
    def __init__(self, canonical_normalized_request_json: _Optional[bytes] = ..., semantic_request_id: _Optional[str] = ..., input_requirements: _Optional[_Iterable[_Union[InputRequirement, _Mapping]]] = ..., errors: _Optional[_Iterable[_Union[ValidationIssue, _Mapping]]] = ..., warnings: _Optional[_Iterable[_Union[ValidationIssue, _Mapping]]] = ..., cost_class: _Optional[str] = ..., estimated_result_bytes: _Optional[int] = ..., estimated_result_pages: _Optional[int] = ...) -> None: ...

class ValidateQueryRequest(_message.Message):
    __slots__ = ("context", "query")
    CONTEXT_FIELD_NUMBER: _ClassVar[int]
    QUERY_FIELD_NUMBER: _ClassVar[int]
    context: RequestContext
    query: QuerySubmission
    def __init__(self, context: _Optional[_Union[RequestContext, _Mapping]] = ..., query: _Optional[_Union[QuerySubmission, _Mapping]] = ...) -> None: ...

class ValidateQueryResponse(_message.Message):
    __slots__ = ("authority", "preparation")
    AUTHORITY_FIELD_NUMBER: _ClassVar[int]
    PREPARATION_FIELD_NUMBER: _ClassVar[int]
    authority: AuthorityGeneration
    preparation: QueryPreparation
    def __init__(self, authority: _Optional[_Union[AuthorityGeneration, _Mapping]] = ..., preparation: _Optional[_Union[QueryPreparation, _Mapping]] = ...) -> None: ...

class InitialQueryStart(_message.Message):
    __slots__ = ("query",)
    QUERY_FIELD_NUMBER: _ClassVar[int]
    query: QuerySubmission
    def __init__(self, query: _Optional[_Union[QuerySubmission, _Mapping]] = ...) -> None: ...

class QueryChallengeContinuation(_message.Message):
    __slots__ = ("daemon_continuation", "challenge_id", "round", "answers")
    DAEMON_CONTINUATION_FIELD_NUMBER: _ClassVar[int]
    CHALLENGE_ID_FIELD_NUMBER: _ClassVar[int]
    ROUND_FIELD_NUMBER: _ClassVar[int]
    ANSWERS_FIELD_NUMBER: _ClassVar[int]
    daemon_continuation: bytes
    challenge_id: str
    round: int
    answers: _containers.RepeatedCompositeFieldContainer[InputAnswer]
    def __init__(self, daemon_continuation: _Optional[bytes] = ..., challenge_id: _Optional[str] = ..., round: _Optional[int] = ..., answers: _Optional[_Iterable[_Union[InputAnswer, _Mapping]]] = ...) -> None: ...

class StartQueryRequest(_message.Message):
    __slots__ = ("context", "initial", "continuation")
    CONTEXT_FIELD_NUMBER: _ClassVar[int]
    INITIAL_FIELD_NUMBER: _ClassVar[int]
    CONTINUATION_FIELD_NUMBER: _ClassVar[int]
    context: RequestContext
    initial: InitialQueryStart
    continuation: QueryChallengeContinuation
    def __init__(self, context: _Optional[_Union[RequestContext, _Mapping]] = ..., initial: _Optional[_Union[InitialQueryStart, _Mapping]] = ..., continuation: _Optional[_Union[QueryChallengeContinuation, _Mapping]] = ...) -> None: ...

class AcceptedQuery(_message.Message):
    __slots__ = ("authority", "daemon_query_id", "semantic_request_id", "operation_fingerprint", "accepted_at_unix_ms", "observation_expires_at_unix_ms", "state", "idempotent_replay")
    AUTHORITY_FIELD_NUMBER: _ClassVar[int]
    DAEMON_QUERY_ID_FIELD_NUMBER: _ClassVar[int]
    SEMANTIC_REQUEST_ID_FIELD_NUMBER: _ClassVar[int]
    OPERATION_FINGERPRINT_FIELD_NUMBER: _ClassVar[int]
    ACCEPTED_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    OBSERVATION_EXPIRES_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    IDEMPOTENT_REPLAY_FIELD_NUMBER: _ClassVar[int]
    authority: AuthorityGeneration
    daemon_query_id: str
    semantic_request_id: str
    operation_fingerprint: str
    accepted_at_unix_ms: int
    observation_expires_at_unix_ms: int
    state: QueryExecutionState
    idempotent_replay: bool
    def __init__(self, authority: _Optional[_Union[AuthorityGeneration, _Mapping]] = ..., daemon_query_id: _Optional[str] = ..., semantic_request_id: _Optional[str] = ..., operation_fingerprint: _Optional[str] = ..., accepted_at_unix_ms: _Optional[int] = ..., observation_expires_at_unix_ms: _Optional[int] = ..., state: _Optional[_Union[QueryExecutionState, str]] = ..., idempotent_replay: _Optional[bool] = ...) -> None: ...

class InputChallenge(_message.Message):
    __slots__ = ("authority", "semantic_request_id", "challenge_id", "round", "remaining_rounds", "issued_at_unix_ms", "expires_at_unix_ms", "maximum_answer_bytes", "explanation_code", "requirements", "daemon_continuation")
    AUTHORITY_FIELD_NUMBER: _ClassVar[int]
    SEMANTIC_REQUEST_ID_FIELD_NUMBER: _ClassVar[int]
    CHALLENGE_ID_FIELD_NUMBER: _ClassVar[int]
    ROUND_FIELD_NUMBER: _ClassVar[int]
    REMAINING_ROUNDS_FIELD_NUMBER: _ClassVar[int]
    ISSUED_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    EXPIRES_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_ANSWER_BYTES_FIELD_NUMBER: _ClassVar[int]
    EXPLANATION_CODE_FIELD_NUMBER: _ClassVar[int]
    REQUIREMENTS_FIELD_NUMBER: _ClassVar[int]
    DAEMON_CONTINUATION_FIELD_NUMBER: _ClassVar[int]
    authority: AuthorityGeneration
    semantic_request_id: str
    challenge_id: str
    round: int
    remaining_rounds: int
    issued_at_unix_ms: int
    expires_at_unix_ms: int
    maximum_answer_bytes: int
    explanation_code: ChallengeExplanationCode
    requirements: _containers.RepeatedCompositeFieldContainer[InputRequirement]
    daemon_continuation: bytes
    def __init__(self, authority: _Optional[_Union[AuthorityGeneration, _Mapping]] = ..., semantic_request_id: _Optional[str] = ..., challenge_id: _Optional[str] = ..., round: _Optional[int] = ..., remaining_rounds: _Optional[int] = ..., issued_at_unix_ms: _Optional[int] = ..., expires_at_unix_ms: _Optional[int] = ..., maximum_answer_bytes: _Optional[int] = ..., explanation_code: _Optional[_Union[ChallengeExplanationCode, str]] = ..., requirements: _Optional[_Iterable[_Union[InputRequirement, _Mapping]]] = ..., daemon_continuation: _Optional[bytes] = ...) -> None: ...

class ValidationRejection(_message.Message):
    __slots__ = ("authority", "semantic_request_id", "issues", "error")
    AUTHORITY_FIELD_NUMBER: _ClassVar[int]
    SEMANTIC_REQUEST_ID_FIELD_NUMBER: _ClassVar[int]
    ISSUES_FIELD_NUMBER: _ClassVar[int]
    ERROR_FIELD_NUMBER: _ClassVar[int]
    authority: AuthorityGeneration
    semantic_request_id: str
    issues: _containers.RepeatedCompositeFieldContainer[ValidationIssue]
    error: SafeErrorMetadata
    def __init__(self, authority: _Optional[_Union[AuthorityGeneration, _Mapping]] = ..., semantic_request_id: _Optional[str] = ..., issues: _Optional[_Iterable[_Union[ValidationIssue, _Mapping]]] = ..., error: _Optional[_Union[SafeErrorMetadata, _Mapping]] = ...) -> None: ...

class StartQueryResponse(_message.Message):
    __slots__ = ("accepted", "input_challenge", "validation_rejection")
    ACCEPTED_FIELD_NUMBER: _ClassVar[int]
    INPUT_CHALLENGE_FIELD_NUMBER: _ClassVar[int]
    VALIDATION_REJECTION_FIELD_NUMBER: _ClassVar[int]
    accepted: AcceptedQuery
    input_challenge: InputChallenge
    validation_rejection: ValidationRejection
    def __init__(self, accepted: _Optional[_Union[AcceptedQuery, _Mapping]] = ..., input_challenge: _Optional[_Union[InputChallenge, _Mapping]] = ..., validation_rejection: _Optional[_Union[ValidationRejection, _Mapping]] = ...) -> None: ...

class WatchQueryRequest(_message.Message):
    __slots__ = ("context", "daemon_query_id", "cursor")
    CONTEXT_FIELD_NUMBER: _ClassVar[int]
    DAEMON_QUERY_ID_FIELD_NUMBER: _ClassVar[int]
    CURSOR_FIELD_NUMBER: _ClassVar[int]
    context: RequestContext
    daemon_query_id: str
    cursor: bytes
    def __init__(self, context: _Optional[_Union[RequestContext, _Mapping]] = ..., daemon_query_id: _Optional[str] = ..., cursor: _Optional[bytes] = ...) -> None: ...

class QueryEventHeader(_message.Message):
    __slots__ = ("authority", "daemon_query_id", "sequence", "emitted_at_unix_ms", "cursor")
    AUTHORITY_FIELD_NUMBER: _ClassVar[int]
    DAEMON_QUERY_ID_FIELD_NUMBER: _ClassVar[int]
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    EMITTED_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    CURSOR_FIELD_NUMBER: _ClassVar[int]
    authority: AuthorityGeneration
    daemon_query_id: str
    sequence: int
    emitted_at_unix_ms: int
    cursor: bytes
    def __init__(self, authority: _Optional[_Union[AuthorityGeneration, _Mapping]] = ..., daemon_query_id: _Optional[str] = ..., sequence: _Optional[int] = ..., emitted_at_unix_ms: _Optional[int] = ..., cursor: _Optional[bytes] = ...) -> None: ...

class SnapshotPinnedEvent(_message.Message):
    __slots__ = ("header", "epoch_id", "source_generation", "activation_head", "lifecycle_watermark", "freshness", "analysis_context_set_id")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    EPOCH_ID_FIELD_NUMBER: _ClassVar[int]
    SOURCE_GENERATION_FIELD_NUMBER: _ClassVar[int]
    ACTIVATION_HEAD_FIELD_NUMBER: _ClassVar[int]
    LIFECYCLE_WATERMARK_FIELD_NUMBER: _ClassVar[int]
    FRESHNESS_FIELD_NUMBER: _ClassVar[int]
    ANALYSIS_CONTEXT_SET_ID_FIELD_NUMBER: _ClassVar[int]
    header: QueryEventHeader
    epoch_id: str
    source_generation: int
    activation_head: int
    lifecycle_watermark: int
    freshness: SnapshotFreshness
    analysis_context_set_id: str
    def __init__(self, header: _Optional[_Union[QueryEventHeader, _Mapping]] = ..., epoch_id: _Optional[str] = ..., source_generation: _Optional[int] = ..., activation_head: _Optional[int] = ..., lifecycle_watermark: _Optional[int] = ..., freshness: _Optional[_Union[SnapshotFreshness, str]] = ..., analysis_context_set_id: _Optional[str] = ...) -> None: ...

class ProgressEvent(_message.Message):
    __slots__ = ("header", "stage", "completed", "total")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    STAGE_FIELD_NUMBER: _ClassVar[int]
    COMPLETED_FIELD_NUMBER: _ClassVar[int]
    TOTAL_FIELD_NUMBER: _ClassVar[int]
    header: QueryEventHeader
    stage: ProgressStage
    completed: int
    total: int
    def __init__(self, header: _Optional[_Union[QueryEventHeader, _Mapping]] = ..., stage: _Optional[_Union[ProgressStage, str]] = ..., completed: _Optional[int] = ..., total: _Optional[int] = ...) -> None: ...

class ResourceDescriptor(_message.Message):
    __slots__ = ("kind", "public_handle", "package_id", "page_ordinal", "media_type", "byte_length", "content_checksum", "expires_at_unix_ms", "authority")
    KIND_FIELD_NUMBER: _ClassVar[int]
    PUBLIC_HANDLE_FIELD_NUMBER: _ClassVar[int]
    PACKAGE_ID_FIELD_NUMBER: _ClassVar[int]
    PAGE_ORDINAL_FIELD_NUMBER: _ClassVar[int]
    MEDIA_TYPE_FIELD_NUMBER: _ClassVar[int]
    BYTE_LENGTH_FIELD_NUMBER: _ClassVar[int]
    CONTENT_CHECKSUM_FIELD_NUMBER: _ClassVar[int]
    EXPIRES_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    AUTHORITY_FIELD_NUMBER: _ClassVar[int]
    kind: ResourceKind
    public_handle: str
    package_id: str
    page_ordinal: int
    media_type: str
    byte_length: int
    content_checksum: str
    expires_at_unix_ms: int
    authority: AuthorityGeneration
    def __init__(self, kind: _Optional[_Union[ResourceKind, str]] = ..., public_handle: _Optional[str] = ..., package_id: _Optional[str] = ..., page_ordinal: _Optional[int] = ..., media_type: _Optional[str] = ..., byte_length: _Optional[int] = ..., content_checksum: _Optional[str] = ..., expires_at_unix_ms: _Optional[int] = ..., authority: _Optional[_Union[AuthorityGeneration, _Mapping]] = ...) -> None: ...

class ResultReadyEvent(_message.Message):
    __slots__ = ("header", "package_id", "manifest", "total_rows", "total_pages", "total_bytes", "pages", "processing")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    PACKAGE_ID_FIELD_NUMBER: _ClassVar[int]
    MANIFEST_FIELD_NUMBER: _ClassVar[int]
    TOTAL_ROWS_FIELD_NUMBER: _ClassVar[int]
    TOTAL_PAGES_FIELD_NUMBER: _ClassVar[int]
    TOTAL_BYTES_FIELD_NUMBER: _ClassVar[int]
    PAGES_FIELD_NUMBER: _ClassVar[int]
    PROCESSING_FIELD_NUMBER: _ClassVar[int]
    header: QueryEventHeader
    package_id: str
    manifest: ResourceDescriptor
    total_rows: int
    total_pages: int
    total_bytes: int
    pages: _containers.RepeatedCompositeFieldContainer[ResourceDescriptor]
    processing: _containers.RepeatedCompositeFieldContainer[QueryProcessingSummary]
    def __init__(self, header: _Optional[_Union[QueryEventHeader, _Mapping]] = ..., package_id: _Optional[str] = ..., manifest: _Optional[_Union[ResourceDescriptor, _Mapping]] = ..., total_rows: _Optional[int] = ..., total_pages: _Optional[int] = ..., total_bytes: _Optional[int] = ..., pages: _Optional[_Iterable[_Union[ResourceDescriptor, _Mapping]]] = ..., processing: _Optional[_Iterable[_Union[QueryProcessingSummary, _Mapping]]] = ...) -> None: ...

class ProcessingRustBuildSelection(_message.Message):
    __slots__ = ("profile", "features", "default_features")
    PROFILE_FIELD_NUMBER: _ClassVar[int]
    FEATURES_FIELD_NUMBER: _ClassVar[int]
    DEFAULT_FEATURES_FIELD_NUMBER: _ClassVar[int]
    profile: str
    features: _containers.RepeatedScalarFieldContainer[str]
    default_features: bool
    def __init__(self, profile: _Optional[str] = ..., features: _Optional[_Iterable[str]] = ..., default_features: _Optional[bool] = ...) -> None: ...

class ProcessingRemainder(_message.Message):
    __slots__ = ("language", "scope_kind", "path_bytes", "path", "target", "state", "reason_code", "target_kind", "analysis_context_id", "entity_id", "target_platform", "rust_build", "fact_family")
    LANGUAGE_FIELD_NUMBER: _ClassVar[int]
    SCOPE_KIND_FIELD_NUMBER: _ClassVar[int]
    PATH_BYTES_FIELD_NUMBER: _ClassVar[int]
    PATH_FIELD_NUMBER: _ClassVar[int]
    TARGET_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    REASON_CODE_FIELD_NUMBER: _ClassVar[int]
    TARGET_KIND_FIELD_NUMBER: _ClassVar[int]
    ANALYSIS_CONTEXT_ID_FIELD_NUMBER: _ClassVar[int]
    ENTITY_ID_FIELD_NUMBER: _ClassVar[int]
    TARGET_PLATFORM_FIELD_NUMBER: _ClassVar[int]
    RUST_BUILD_FIELD_NUMBER: _ClassVar[int]
    FACT_FAMILY_FIELD_NUMBER: _ClassVar[int]
    language: str
    scope_kind: str
    path_bytes: bytes
    path: str
    target: str
    state: ProcessingState
    reason_code: str
    target_kind: str
    analysis_context_id: str
    entity_id: str
    target_platform: str
    rust_build: ProcessingRustBuildSelection
    fact_family: str
    def __init__(self, language: _Optional[str] = ..., scope_kind: _Optional[str] = ..., path_bytes: _Optional[bytes] = ..., path: _Optional[str] = ..., target: _Optional[str] = ..., state: _Optional[_Union[ProcessingState, str]] = ..., reason_code: _Optional[str] = ..., target_kind: _Optional[str] = ..., analysis_context_id: _Optional[str] = ..., entity_id: _Optional[str] = ..., target_platform: _Optional[str] = ..., rust_build: _Optional[_Union[ProcessingRustBuildSelection, _Mapping]] = ..., fact_family: _Optional[str] = ...) -> None: ...

class QueryProcessingSummary(_message.Message):
    __slots__ = ("query_id", "source_generation", "scope", "family", "languages", "requested_partitions", "completed_partitions", "remaining_partitions", "remainder", "next_offset", "maximum_rows", "additional_rows", "remainder_handle", "remainder_offset")
    QUERY_ID_FIELD_NUMBER: _ClassVar[int]
    SOURCE_GENERATION_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    FAMILY_FIELD_NUMBER: _ClassVar[int]
    LANGUAGES_FIELD_NUMBER: _ClassVar[int]
    REQUESTED_PARTITIONS_FIELD_NUMBER: _ClassVar[int]
    COMPLETED_PARTITIONS_FIELD_NUMBER: _ClassVar[int]
    REMAINING_PARTITIONS_FIELD_NUMBER: _ClassVar[int]
    REMAINDER_FIELD_NUMBER: _ClassVar[int]
    NEXT_OFFSET_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_ROWS_FIELD_NUMBER: _ClassVar[int]
    ADDITIONAL_ROWS_FIELD_NUMBER: _ClassVar[int]
    REMAINDER_HANDLE_FIELD_NUMBER: _ClassVar[int]
    REMAINDER_OFFSET_FIELD_NUMBER: _ClassVar[int]
    query_id: str
    source_generation: int
    scope: str
    family: str
    languages: _containers.RepeatedScalarFieldContainer[str]
    requested_partitions: int
    completed_partitions: int
    remaining_partitions: int
    remainder: _containers.RepeatedCompositeFieldContainer[ProcessingRemainder]
    next_offset: int
    maximum_rows: int
    additional_rows: bool
    remainder_handle: str
    remainder_offset: int
    def __init__(self, query_id: _Optional[str] = ..., source_generation: _Optional[int] = ..., scope: _Optional[str] = ..., family: _Optional[str] = ..., languages: _Optional[_Iterable[str]] = ..., requested_partitions: _Optional[int] = ..., completed_partitions: _Optional[int] = ..., remaining_partitions: _Optional[int] = ..., remainder: _Optional[_Iterable[_Union[ProcessingRemainder, _Mapping]]] = ..., next_offset: _Optional[int] = ..., maximum_rows: _Optional[int] = ..., additional_rows: _Optional[bool] = ..., remainder_handle: _Optional[str] = ..., remainder_offset: _Optional[int] = ...) -> None: ...

class ReadProcessingRemainderRequest(_message.Message):
    __slots__ = ("context", "daemon_query_id", "query_id", "offset")
    CONTEXT_FIELD_NUMBER: _ClassVar[int]
    DAEMON_QUERY_ID_FIELD_NUMBER: _ClassVar[int]
    QUERY_ID_FIELD_NUMBER: _ClassVar[int]
    OFFSET_FIELD_NUMBER: _ClassVar[int]
    context: RequestContext
    daemon_query_id: str
    query_id: str
    offset: int
    def __init__(self, context: _Optional[_Union[RequestContext, _Mapping]] = ..., daemon_query_id: _Optional[str] = ..., query_id: _Optional[str] = ..., offset: _Optional[int] = ...) -> None: ...

class ReadProcessingRemainderResponse(_message.Message):
    __slots__ = ("authority", "package_id", "epoch_id", "processing", "public_handle")
    AUTHORITY_FIELD_NUMBER: _ClassVar[int]
    PACKAGE_ID_FIELD_NUMBER: _ClassVar[int]
    EPOCH_ID_FIELD_NUMBER: _ClassVar[int]
    PROCESSING_FIELD_NUMBER: _ClassVar[int]
    PUBLIC_HANDLE_FIELD_NUMBER: _ClassVar[int]
    authority: AuthorityGeneration
    package_id: str
    epoch_id: str
    processing: QueryProcessingSummary
    public_handle: str
    def __init__(self, authority: _Optional[_Union[AuthorityGeneration, _Mapping]] = ..., package_id: _Optional[str] = ..., epoch_id: _Optional[str] = ..., processing: _Optional[_Union[QueryProcessingSummary, _Mapping]] = ..., public_handle: _Optional[str] = ...) -> None: ...

class TerminalEvent(_message.Message):
    __slots__ = ("header", "state", "error")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    ERROR_FIELD_NUMBER: _ClassVar[int]
    header: QueryEventHeader
    state: QueryExecutionState
    error: SafeErrorMetadata
    def __init__(self, header: _Optional[_Union[QueryEventHeader, _Mapping]] = ..., state: _Optional[_Union[QueryExecutionState, str]] = ..., error: _Optional[_Union[SafeErrorMetadata, _Mapping]] = ...) -> None: ...

class QueryEvent(_message.Message):
    __slots__ = ("snapshot_pinned", "progress", "result_ready", "terminal")
    SNAPSHOT_PINNED_FIELD_NUMBER: _ClassVar[int]
    PROGRESS_FIELD_NUMBER: _ClassVar[int]
    RESULT_READY_FIELD_NUMBER: _ClassVar[int]
    TERMINAL_FIELD_NUMBER: _ClassVar[int]
    snapshot_pinned: SnapshotPinnedEvent
    progress: ProgressEvent
    result_ready: ResultReadyEvent
    terminal: TerminalEvent
    def __init__(self, snapshot_pinned: _Optional[_Union[SnapshotPinnedEvent, _Mapping]] = ..., progress: _Optional[_Union[ProgressEvent, _Mapping]] = ..., result_ready: _Optional[_Union[ResultReadyEvent, _Mapping]] = ..., terminal: _Optional[_Union[TerminalEvent, _Mapping]] = ...) -> None: ...

class CancelQueryRequest(_message.Message):
    __slots__ = ("context", "daemon_query_id", "cancellation_id")
    CONTEXT_FIELD_NUMBER: _ClassVar[int]
    DAEMON_QUERY_ID_FIELD_NUMBER: _ClassVar[int]
    CANCELLATION_ID_FIELD_NUMBER: _ClassVar[int]
    context: RequestContext
    daemon_query_id: str
    cancellation_id: str
    def __init__(self, context: _Optional[_Union[RequestContext, _Mapping]] = ..., daemon_query_id: _Optional[str] = ..., cancellation_id: _Optional[str] = ...) -> None: ...

class TerminalObservation(_message.Message):
    __slots__ = ("state", "observed_at_unix_ms", "error")
    STATE_FIELD_NUMBER: _ClassVar[int]
    OBSERVED_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    ERROR_FIELD_NUMBER: _ClassVar[int]
    state: QueryExecutionState
    observed_at_unix_ms: int
    error: SafeErrorMetadata
    def __init__(self, state: _Optional[_Union[QueryExecutionState, str]] = ..., observed_at_unix_ms: _Optional[int] = ..., error: _Optional[_Union[SafeErrorMetadata, _Mapping]] = ...) -> None: ...

class CancelQueryResponse(_message.Message):
    __slots__ = ("authority", "cancellation_id", "acknowledgement", "terminal", "idempotent_replay")
    AUTHORITY_FIELD_NUMBER: _ClassVar[int]
    CANCELLATION_ID_FIELD_NUMBER: _ClassVar[int]
    ACKNOWLEDGEMENT_FIELD_NUMBER: _ClassVar[int]
    TERMINAL_FIELD_NUMBER: _ClassVar[int]
    IDEMPOTENT_REPLAY_FIELD_NUMBER: _ClassVar[int]
    authority: AuthorityGeneration
    cancellation_id: str
    acknowledgement: CancellationAcknowledgement
    terminal: TerminalObservation
    idempotent_replay: bool
    def __init__(self, authority: _Optional[_Union[AuthorityGeneration, _Mapping]] = ..., cancellation_id: _Optional[str] = ..., acknowledgement: _Optional[_Union[CancellationAcknowledgement, str]] = ..., terminal: _Optional[_Union[TerminalObservation, _Mapping]] = ..., idempotent_replay: _Optional[bool] = ...) -> None: ...

class ManifestSelector(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class PageSelector(_message.Message):
    __slots__ = ("page_ordinal",)
    PAGE_ORDINAL_FIELD_NUMBER: _ClassVar[int]
    page_ordinal: int
    def __init__(self, page_ordinal: _Optional[int] = ...) -> None: ...

class ResourceSelector(_message.Message):
    __slots__ = ("manifest", "page", "reference")
    MANIFEST_FIELD_NUMBER: _ClassVar[int]
    PAGE_FIELD_NUMBER: _ClassVar[int]
    REFERENCE_FIELD_NUMBER: _ClassVar[int]
    manifest: ManifestSelector
    page: PageSelector
    reference: ReferenceReadRequest
    def __init__(self, manifest: _Optional[_Union[ManifestSelector, _Mapping]] = ..., page: _Optional[_Union[PageSelector, _Mapping]] = ..., reference: _Optional[_Union[ReferenceReadRequest, _Mapping]] = ...) -> None: ...

class ReadResourceRequest(_message.Message):
    __slots__ = ("context", "public_handle", "selector", "offset", "maximum_bytes")
    CONTEXT_FIELD_NUMBER: _ClassVar[int]
    PUBLIC_HANDLE_FIELD_NUMBER: _ClassVar[int]
    SELECTOR_FIELD_NUMBER: _ClassVar[int]
    OFFSET_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_BYTES_FIELD_NUMBER: _ClassVar[int]
    context: RequestContext
    public_handle: str
    selector: ResourceSelector
    offset: int
    maximum_bytes: int
    def __init__(self, context: _Optional[_Union[RequestContext, _Mapping]] = ..., public_handle: _Optional[str] = ..., selector: _Optional[_Union[ResourceSelector, _Mapping]] = ..., offset: _Optional[int] = ..., maximum_bytes: _Optional[int] = ...) -> None: ...

class ResourceChunk(_message.Message):
    __slots__ = ("authority", "public_handle", "offset", "content", "content_checksum", "end_of_resource")
    AUTHORITY_FIELD_NUMBER: _ClassVar[int]
    PUBLIC_HANDLE_FIELD_NUMBER: _ClassVar[int]
    OFFSET_FIELD_NUMBER: _ClassVar[int]
    CONTENT_FIELD_NUMBER: _ClassVar[int]
    CONTENT_CHECKSUM_FIELD_NUMBER: _ClassVar[int]
    END_OF_RESOURCE_FIELD_NUMBER: _ClassVar[int]
    authority: AuthorityGeneration
    public_handle: str
    offset: int
    content: bytes
    content_checksum: str
    end_of_resource: bool
    def __init__(self, authority: _Optional[_Union[AuthorityGeneration, _Mapping]] = ..., public_handle: _Optional[str] = ..., offset: _Optional[int] = ..., content: _Optional[bytes] = ..., content_checksum: _Optional[str] = ..., end_of_resource: _Optional[bool] = ...) -> None: ...

class ReleaseResourceRequest(_message.Message):
    __slots__ = ("context", "public_handle", "release_id")
    CONTEXT_FIELD_NUMBER: _ClassVar[int]
    PUBLIC_HANDLE_FIELD_NUMBER: _ClassVar[int]
    RELEASE_ID_FIELD_NUMBER: _ClassVar[int]
    context: RequestContext
    public_handle: str
    release_id: str
    def __init__(self, context: _Optional[_Union[RequestContext, _Mapping]] = ..., public_handle: _Optional[str] = ..., release_id: _Optional[str] = ...) -> None: ...

class ReleaseResourceResponse(_message.Message):
    __slots__ = ("authority", "release_id", "state", "idempotent_replay")
    AUTHORITY_FIELD_NUMBER: _ClassVar[int]
    RELEASE_ID_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    IDEMPOTENT_REPLAY_FIELD_NUMBER: _ClassVar[int]
    authority: AuthorityGeneration
    release_id: str
    state: ReleaseState
    idempotent_replay: bool
    def __init__(self, authority: _Optional[_Union[AuthorityGeneration, _Mapping]] = ..., release_id: _Optional[str] = ..., state: _Optional[_Union[ReleaseState, str]] = ..., idempotent_replay: _Optional[bool] = ...) -> None: ...
