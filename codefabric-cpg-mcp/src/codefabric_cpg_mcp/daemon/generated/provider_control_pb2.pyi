# @generated from catalog primary semantic identity b3:502dfd819e70a154db899bd6bdbe580d01bb56f1654790d5adb241199d43b434,b3:71fb94283214d79068ede88e0f45e1460336b23b9678f80b4ddbece098cd626f,b3:d5b256baca150eed2617f78f88362c607ff12db7a94af9524658a3c82f247973,b3:2f2c24a2877be95dfd1d3acc7d83354838696af2aaac13c99bde83ab743f6c62; do not edit.
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class ProviderRunState(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    PROVIDER_RUN_STATE_UNSPECIFIED: _ClassVar[ProviderRunState]
    PROVIDER_RUN_STATE_QUEUED: _ClassVar[ProviderRunState]
    PROVIDER_RUN_STATE_RUNNING: _ClassVar[ProviderRunState]
    PROVIDER_RUN_STATE_SUCCEEDED: _ClassVar[ProviderRunState]
    PROVIDER_RUN_STATE_PARTIAL: _ClassVar[ProviderRunState]
    PROVIDER_RUN_STATE_FAILED: _ClassVar[ProviderRunState]
    PROVIDER_RUN_STATE_TIMED_OUT: _ClassVar[ProviderRunState]
    PROVIDER_RUN_STATE_CANCELLED: _ClassVar[ProviderRunState]
    PROVIDER_RUN_STATE_SUPERSEDED: _ClassVar[ProviderRunState]
    PROVIDER_RUN_STATE_CRASHED: _ClassVar[ProviderRunState]
    PROVIDER_RUN_STATE_PROTOCOL_ERROR: _ClassVar[ProviderRunState]
    PROVIDER_RUN_STATE_STALE_RESULT: _ClassVar[ProviderRunState]
    PROVIDER_RUN_STATE_STALE_GIT_BASELINE: _ClassVar[ProviderRunState]

class ProviderPlacement(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    PROVIDER_PLACEMENT_UNSPECIFIED: _ClassVar[ProviderPlacement]
    PROVIDER_PLACEMENT_IN_PROCESS: _ClassVar[ProviderPlacement]
    PROVIDER_PLACEMENT_SIDECAR: _ClassVar[ProviderPlacement]
    PROVIDER_PLACEMENT_COMPILER_GROUP: _ClassVar[ProviderPlacement]

class ProviderPriorityClass(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    PROVIDER_PRIORITY_CLASS_UNSPECIFIED: _ClassVar[ProviderPriorityClass]
    PROVIDER_PRIORITY_CLASS_INTERACTIVE: _ClassVar[ProviderPriorityClass]
    PROVIDER_PRIORITY_CLASS_CONTINUOUS: _ClassVar[ProviderPriorityClass]
    PROVIDER_PRIORITY_CLASS_BACKGROUND: _ClassVar[ProviderPriorityClass]

class ProviderScopeKind(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    PROVIDER_SCOPE_KIND_UNSPECIFIED: _ClassVar[ProviderScopeKind]
    PROVIDER_SCOPE_KIND_WORKSPACE: _ClassVar[ProviderScopeKind]
    PROVIDER_SCOPE_KIND_ANALYSIS_CONTEXT: _ClassVar[ProviderScopeKind]
    PROVIDER_SCOPE_KIND_BUILD_UNIT: _ClassVar[ProviderScopeKind]
    PROVIDER_SCOPE_KIND_MODULE_OR_CRATE: _ClassVar[ProviderScopeKind]
    PROVIDER_SCOPE_KIND_SOURCE_FILE: _ClassVar[ProviderScopeKind]
    PROVIDER_SCOPE_KIND_SEMANTIC_OWNER: _ClassVar[ProviderScopeKind]
    PROVIDER_SCOPE_KIND_CALLABLE_OR_MIR_BODY: _ClassVar[ProviderScopeKind]
    PROVIDER_SCOPE_KIND_WORKSPACE_GLOBAL_DERIVATION: _ClassVar[ProviderScopeKind]

class ProviderEventKind(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    PROVIDER_EVENT_KIND_UNSPECIFIED: _ClassVar[ProviderEventKind]
    PROVIDER_EVENT_KIND_ACCEPTED: _ClassVar[ProviderEventKind]
    PROVIDER_EVENT_KIND_PROGRESS: _ClassVar[ProviderEventKind]
    PROVIDER_EVENT_KIND_SCOPE_BEGIN: _ClassVar[ProviderEventKind]
    PROVIDER_EVENT_KIND_OBSERVATION_CHUNK: _ClassVar[ProviderEventKind]
    PROVIDER_EVENT_KIND_SCOPE_END: _ClassVar[ProviderEventKind]
    PROVIDER_EVENT_KIND_TERMINAL: _ClassVar[ProviderEventKind]
    PROVIDER_EVENT_KIND_CANCEL_ACKNOWLEDGED: _ClassVar[ProviderEventKind]

class CancelAcknowledgementState(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CANCEL_ACKNOWLEDGEMENT_STATE_UNSPECIFIED: _ClassVar[CancelAcknowledgementState]
    CANCEL_ACKNOWLEDGEMENT_STATE_NOT_FOUND: _ClassVar[CancelAcknowledgementState]
    CANCEL_ACKNOWLEDGEMENT_STATE_CANCELLATION_REQUESTED: _ClassVar[CancelAcknowledgementState]
    CANCEL_ACKNOWLEDGEMENT_STATE_CANCELLED: _ClassVar[CancelAcknowledgementState]
    CANCEL_ACKNOWLEDGEMENT_STATE_ALREADY_TERMINAL: _ClassVar[CancelAcknowledgementState]
    CANCEL_ACKNOWLEDGEMENT_STATE_FORCE_TERMINATED: _ClassVar[CancelAcknowledgementState]

class CreditControlLimit(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CREDIT_CONTROL_LIMIT_UNSPECIFIED: _ClassVar[CreditControlLimit]
    CREDIT_CONTROL_LIMIT_MAX_OUTSTANDING_CHUNKS: _ClassVar[CreditControlLimit]
    CREDIT_CONTROL_LIMIT_MAX_UNACKNOWLEDGED_MIB: _ClassVar[CreditControlLimit]

class ArrowIpcMetadataVersion(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    ARROW_IPC_METADATA_VERSION_UNSPECIFIED: _ClassVar[ArrowIpcMetadataVersion]
    ARROW_IPC_METADATA_VERSION_V5: _ClassVar[ArrowIpcMetadataVersion]

class RelationIpcRemainderReason(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    RELATION_IPC_REMAINDER_REASON_UNSPECIFIED: _ClassVar[RelationIpcRemainderReason]
    RELATION_IPC_REMAINDER_REASON_UNSUPPORTED: _ClassVar[RelationIpcRemainderReason]
    RELATION_IPC_REMAINDER_REASON_PROVIDER_UNAVAILABLE: _ClassVar[RelationIpcRemainderReason]
    RELATION_IPC_REMAINDER_REASON_RESOURCE_LIMIT: _ClassVar[RelationIpcRemainderReason]
    RELATION_IPC_REMAINDER_REASON_INVALID_SOURCE: _ClassVar[RelationIpcRemainderReason]
    RELATION_IPC_REMAINDER_REASON_CANCELLED: _ClassVar[RelationIpcRemainderReason]
    RELATION_IPC_REMAINDER_REASON_UNKNOWN: _ClassVar[RelationIpcRemainderReason]

class RelationIpcTerminalStatus(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    RELATION_IPC_TERMINAL_STATUS_UNSPECIFIED: _ClassVar[RelationIpcTerminalStatus]
    RELATION_IPC_TERMINAL_STATUS_COMPLETE: _ClassVar[RelationIpcTerminalStatus]
    RELATION_IPC_TERMINAL_STATUS_PARTIAL: _ClassVar[RelationIpcTerminalStatus]
    RELATION_IPC_TERMINAL_STATUS_UNKNOWN: _ClassVar[RelationIpcTerminalStatus]
PROVIDER_RUN_STATE_UNSPECIFIED: ProviderRunState
PROVIDER_RUN_STATE_QUEUED: ProviderRunState
PROVIDER_RUN_STATE_RUNNING: ProviderRunState
PROVIDER_RUN_STATE_SUCCEEDED: ProviderRunState
PROVIDER_RUN_STATE_PARTIAL: ProviderRunState
PROVIDER_RUN_STATE_FAILED: ProviderRunState
PROVIDER_RUN_STATE_TIMED_OUT: ProviderRunState
PROVIDER_RUN_STATE_CANCELLED: ProviderRunState
PROVIDER_RUN_STATE_SUPERSEDED: ProviderRunState
PROVIDER_RUN_STATE_CRASHED: ProviderRunState
PROVIDER_RUN_STATE_PROTOCOL_ERROR: ProviderRunState
PROVIDER_RUN_STATE_STALE_RESULT: ProviderRunState
PROVIDER_RUN_STATE_STALE_GIT_BASELINE: ProviderRunState
PROVIDER_PLACEMENT_UNSPECIFIED: ProviderPlacement
PROVIDER_PLACEMENT_IN_PROCESS: ProviderPlacement
PROVIDER_PLACEMENT_SIDECAR: ProviderPlacement
PROVIDER_PLACEMENT_COMPILER_GROUP: ProviderPlacement
PROVIDER_PRIORITY_CLASS_UNSPECIFIED: ProviderPriorityClass
PROVIDER_PRIORITY_CLASS_INTERACTIVE: ProviderPriorityClass
PROVIDER_PRIORITY_CLASS_CONTINUOUS: ProviderPriorityClass
PROVIDER_PRIORITY_CLASS_BACKGROUND: ProviderPriorityClass
PROVIDER_SCOPE_KIND_UNSPECIFIED: ProviderScopeKind
PROVIDER_SCOPE_KIND_WORKSPACE: ProviderScopeKind
PROVIDER_SCOPE_KIND_ANALYSIS_CONTEXT: ProviderScopeKind
PROVIDER_SCOPE_KIND_BUILD_UNIT: ProviderScopeKind
PROVIDER_SCOPE_KIND_MODULE_OR_CRATE: ProviderScopeKind
PROVIDER_SCOPE_KIND_SOURCE_FILE: ProviderScopeKind
PROVIDER_SCOPE_KIND_SEMANTIC_OWNER: ProviderScopeKind
PROVIDER_SCOPE_KIND_CALLABLE_OR_MIR_BODY: ProviderScopeKind
PROVIDER_SCOPE_KIND_WORKSPACE_GLOBAL_DERIVATION: ProviderScopeKind
PROVIDER_EVENT_KIND_UNSPECIFIED: ProviderEventKind
PROVIDER_EVENT_KIND_ACCEPTED: ProviderEventKind
PROVIDER_EVENT_KIND_PROGRESS: ProviderEventKind
PROVIDER_EVENT_KIND_SCOPE_BEGIN: ProviderEventKind
PROVIDER_EVENT_KIND_OBSERVATION_CHUNK: ProviderEventKind
PROVIDER_EVENT_KIND_SCOPE_END: ProviderEventKind
PROVIDER_EVENT_KIND_TERMINAL: ProviderEventKind
PROVIDER_EVENT_KIND_CANCEL_ACKNOWLEDGED: ProviderEventKind
CANCEL_ACKNOWLEDGEMENT_STATE_UNSPECIFIED: CancelAcknowledgementState
CANCEL_ACKNOWLEDGEMENT_STATE_NOT_FOUND: CancelAcknowledgementState
CANCEL_ACKNOWLEDGEMENT_STATE_CANCELLATION_REQUESTED: CancelAcknowledgementState
CANCEL_ACKNOWLEDGEMENT_STATE_CANCELLED: CancelAcknowledgementState
CANCEL_ACKNOWLEDGEMENT_STATE_ALREADY_TERMINAL: CancelAcknowledgementState
CANCEL_ACKNOWLEDGEMENT_STATE_FORCE_TERMINATED: CancelAcknowledgementState
CREDIT_CONTROL_LIMIT_UNSPECIFIED: CreditControlLimit
CREDIT_CONTROL_LIMIT_MAX_OUTSTANDING_CHUNKS: CreditControlLimit
CREDIT_CONTROL_LIMIT_MAX_UNACKNOWLEDGED_MIB: CreditControlLimit
ARROW_IPC_METADATA_VERSION_UNSPECIFIED: ArrowIpcMetadataVersion
ARROW_IPC_METADATA_VERSION_V5: ArrowIpcMetadataVersion
RELATION_IPC_REMAINDER_REASON_UNSPECIFIED: RelationIpcRemainderReason
RELATION_IPC_REMAINDER_REASON_UNSUPPORTED: RelationIpcRemainderReason
RELATION_IPC_REMAINDER_REASON_PROVIDER_UNAVAILABLE: RelationIpcRemainderReason
RELATION_IPC_REMAINDER_REASON_RESOURCE_LIMIT: RelationIpcRemainderReason
RELATION_IPC_REMAINDER_REASON_INVALID_SOURCE: RelationIpcRemainderReason
RELATION_IPC_REMAINDER_REASON_CANCELLED: RelationIpcRemainderReason
RELATION_IPC_REMAINDER_REASON_UNKNOWN: RelationIpcRemainderReason
RELATION_IPC_TERMINAL_STATUS_UNSPECIFIED: RelationIpcTerminalStatus
RELATION_IPC_TERMINAL_STATUS_COMPLETE: RelationIpcTerminalStatus
RELATION_IPC_TERMINAL_STATUS_PARTIAL: RelationIpcTerminalStatus
RELATION_IPC_TERMINAL_STATUS_UNKNOWN: RelationIpcTerminalStatus

class BlobReference(_message.Message):
    __slots__ = ("blob_id", "content_digest", "byte_length", "read_only_uri")
    BLOB_ID_FIELD_NUMBER: _ClassVar[int]
    CONTENT_DIGEST_FIELD_NUMBER: _ClassVar[int]
    BYTE_LENGTH_FIELD_NUMBER: _ClassVar[int]
    READ_ONLY_URI_FIELD_NUMBER: _ClassVar[int]
    blob_id: str
    content_digest: str
    byte_length: int
    read_only_uri: str
    def __init__(self, blob_id: _Optional[str] = ..., content_digest: _Optional[str] = ..., byte_length: _Optional[int] = ..., read_only_uri: _Optional[str] = ...) -> None: ...

class SourceSnapshotLease(_message.Message):
    __slots__ = ("lease_id", "workspace_id", "source_generation", "source_manifest_digest", "expires_at_unix_ms", "blobs")
    LEASE_ID_FIELD_NUMBER: _ClassVar[int]
    WORKSPACE_ID_FIELD_NUMBER: _ClassVar[int]
    SOURCE_GENERATION_FIELD_NUMBER: _ClassVar[int]
    SOURCE_MANIFEST_DIGEST_FIELD_NUMBER: _ClassVar[int]
    EXPIRES_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    BLOBS_FIELD_NUMBER: _ClassVar[int]
    lease_id: str
    workspace_id: str
    source_generation: int
    source_manifest_digest: str
    expires_at_unix_ms: int
    blobs: _containers.RepeatedCompositeFieldContainer[BlobReference]
    def __init__(self, lease_id: _Optional[str] = ..., workspace_id: _Optional[str] = ..., source_generation: _Optional[int] = ..., source_manifest_digest: _Optional[str] = ..., expires_at_unix_ms: _Optional[int] = ..., blobs: _Optional[_Iterable[_Union[BlobReference, _Mapping]]] = ...) -> None: ...

class ProviderScope(_message.Message):
    __slots__ = ("scope_kind", "scope_id")
    SCOPE_KIND_FIELD_NUMBER: _ClassVar[int]
    SCOPE_ID_FIELD_NUMBER: _ClassVar[int]
    scope_kind: ProviderScopeKind
    scope_id: str
    def __init__(self, scope_kind: _Optional[_Union[ProviderScopeKind, str]] = ..., scope_id: _Optional[str] = ...) -> None: ...

class ResourceEstimate(_message.Message):
    __slots__ = ("input_bytes", "expected_output_bytes", "cpu_weight", "memory_mib")
    INPUT_BYTES_FIELD_NUMBER: _ClassVar[int]
    EXPECTED_OUTPUT_BYTES_FIELD_NUMBER: _ClassVar[int]
    CPU_WEIGHT_FIELD_NUMBER: _ClassVar[int]
    MEMORY_MIB_FIELD_NUMBER: _ClassVar[int]
    input_bytes: int
    expected_output_bytes: int
    cpu_weight: int
    memory_mib: int
    def __init__(self, input_bytes: _Optional[int] = ..., expected_output_bytes: _Optional[int] = ..., cpu_weight: _Optional[int] = ..., memory_mib: _Optional[int] = ...) -> None: ...

class ProviderJobSpec(_message.Message):
    __slots__ = ("provider_run_id", "workspace_id", "analysis_context_id", "source_generation", "source_snapshot_lease", "requested_capability_codes", "scopes", "priority_class", "resource_estimate", "deadline_unix_ms", "supersession_key", "required_bundle_digests", "required_schema_digests", "idempotency_key", "resource_profile_id", "sandbox_profile_digest")
    PROVIDER_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    WORKSPACE_ID_FIELD_NUMBER: _ClassVar[int]
    ANALYSIS_CONTEXT_ID_FIELD_NUMBER: _ClassVar[int]
    SOURCE_GENERATION_FIELD_NUMBER: _ClassVar[int]
    SOURCE_SNAPSHOT_LEASE_FIELD_NUMBER: _ClassVar[int]
    REQUESTED_CAPABILITY_CODES_FIELD_NUMBER: _ClassVar[int]
    SCOPES_FIELD_NUMBER: _ClassVar[int]
    PRIORITY_CLASS_FIELD_NUMBER: _ClassVar[int]
    RESOURCE_ESTIMATE_FIELD_NUMBER: _ClassVar[int]
    DEADLINE_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    SUPERSESSION_KEY_FIELD_NUMBER: _ClassVar[int]
    REQUIRED_BUNDLE_DIGESTS_FIELD_NUMBER: _ClassVar[int]
    REQUIRED_SCHEMA_DIGESTS_FIELD_NUMBER: _ClassVar[int]
    IDEMPOTENCY_KEY_FIELD_NUMBER: _ClassVar[int]
    RESOURCE_PROFILE_ID_FIELD_NUMBER: _ClassVar[int]
    SANDBOX_PROFILE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    provider_run_id: str
    workspace_id: str
    analysis_context_id: str
    source_generation: int
    source_snapshot_lease: SourceSnapshotLease
    requested_capability_codes: _containers.RepeatedScalarFieldContainer[int]
    scopes: _containers.RepeatedCompositeFieldContainer[ProviderScope]
    priority_class: ProviderPriorityClass
    resource_estimate: ResourceEstimate
    deadline_unix_ms: int
    supersession_key: str
    required_bundle_digests: _containers.RepeatedScalarFieldContainer[str]
    required_schema_digests: _containers.RepeatedScalarFieldContainer[str]
    idempotency_key: str
    resource_profile_id: str
    sandbox_profile_digest: str
    def __init__(self, provider_run_id: _Optional[str] = ..., workspace_id: _Optional[str] = ..., analysis_context_id: _Optional[str] = ..., source_generation: _Optional[int] = ..., source_snapshot_lease: _Optional[_Union[SourceSnapshotLease, _Mapping]] = ..., requested_capability_codes: _Optional[_Iterable[int]] = ..., scopes: _Optional[_Iterable[_Union[ProviderScope, _Mapping]]] = ..., priority_class: _Optional[_Union[ProviderPriorityClass, str]] = ..., resource_estimate: _Optional[_Union[ResourceEstimate, _Mapping]] = ..., deadline_unix_ms: _Optional[int] = ..., supersession_key: _Optional[str] = ..., required_bundle_digests: _Optional[_Iterable[str]] = ..., required_schema_digests: _Optional[_Iterable[str]] = ..., idempotency_key: _Optional[str] = ..., resource_profile_id: _Optional[str] = ..., sandbox_profile_digest: _Optional[str] = ...) -> None: ...

class AcceptedProviderJob(_message.Message):
    __slots__ = ("provider_run_id", "accepted_generation", "state", "accepted_at_unix_ms", "event_resume_token")
    PROVIDER_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    ACCEPTED_GENERATION_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    ACCEPTED_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    EVENT_RESUME_TOKEN_FIELD_NUMBER: _ClassVar[int]
    provider_run_id: str
    accepted_generation: int
    state: ProviderRunState
    accepted_at_unix_ms: int
    event_resume_token: str
    def __init__(self, provider_run_id: _Optional[str] = ..., accepted_generation: _Optional[int] = ..., state: _Optional[_Union[ProviderRunState, str]] = ..., accepted_at_unix_ms: _Optional[int] = ..., event_resume_token: _Optional[str] = ...) -> None: ...

class ProviderEventStreamRequest(_message.Message):
    __slots__ = ("provider_run_id", "event_resume_token", "after_sequence")
    PROVIDER_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    EVENT_RESUME_TOKEN_FIELD_NUMBER: _ClassVar[int]
    AFTER_SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    provider_run_id: str
    event_resume_token: str
    after_sequence: int
    def __init__(self, provider_run_id: _Optional[str] = ..., event_resume_token: _Optional[str] = ..., after_sequence: _Optional[int] = ...) -> None: ...

class ProviderEventHeader(_message.Message):
    __slots__ = ("provider_run_id", "workspace_id", "analysis_context_id", "source_generation", "sequence", "event_at_unix_ms", "event_checksum")
    PROVIDER_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    WORKSPACE_ID_FIELD_NUMBER: _ClassVar[int]
    ANALYSIS_CONTEXT_ID_FIELD_NUMBER: _ClassVar[int]
    SOURCE_GENERATION_FIELD_NUMBER: _ClassVar[int]
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    EVENT_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    EVENT_CHECKSUM_FIELD_NUMBER: _ClassVar[int]
    provider_run_id: str
    workspace_id: str
    analysis_context_id: str
    source_generation: int
    sequence: int
    event_at_unix_ms: int
    event_checksum: str
    def __init__(self, provider_run_id: _Optional[str] = ..., workspace_id: _Optional[str] = ..., analysis_context_id: _Optional[str] = ..., source_generation: _Optional[int] = ..., sequence: _Optional[int] = ..., event_at_unix_ms: _Optional[int] = ..., event_checksum: _Optional[str] = ...) -> None: ...

class ProviderAcceptedEvent(_message.Message):
    __slots__ = ("header", "accepted_generation")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    ACCEPTED_GENERATION_FIELD_NUMBER: _ClassVar[int]
    header: ProviderEventHeader
    accepted_generation: int
    def __init__(self, header: _Optional[_Union[ProviderEventHeader, _Mapping]] = ..., accepted_generation: _Optional[int] = ...) -> None: ...

class ProviderProgressEvent(_message.Message):
    __slots__ = ("header", "completed_units", "total_units", "phase")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    COMPLETED_UNITS_FIELD_NUMBER: _ClassVar[int]
    TOTAL_UNITS_FIELD_NUMBER: _ClassVar[int]
    PHASE_FIELD_NUMBER: _ClassVar[int]
    header: ProviderEventHeader
    completed_units: int
    total_units: int
    phase: str
    def __init__(self, header: _Optional[_Union[ProviderEventHeader, _Mapping]] = ..., completed_units: _Optional[int] = ..., total_units: _Optional[int] = ..., phase: _Optional[str] = ...) -> None: ...

class ProviderScopeBeginEvent(_message.Message):
    __slots__ = ("header", "scope")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    header: ProviderEventHeader
    scope: ProviderScope
    def __init__(self, header: _Optional[_Union[ProviderEventHeader, _Mapping]] = ..., scope: _Optional[_Union[ProviderScope, _Mapping]] = ...) -> None: ...

class ProviderObservationChunkEvent(_message.Message):
    __slots__ = ("header", "scope", "observation_family_code", "arrow_ipc", "payload_reference", "schema_digest", "row_count", "chunk_digest")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    OBSERVATION_FAMILY_CODE_FIELD_NUMBER: _ClassVar[int]
    ARROW_IPC_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_REFERENCE_FIELD_NUMBER: _ClassVar[int]
    SCHEMA_DIGEST_FIELD_NUMBER: _ClassVar[int]
    ROW_COUNT_FIELD_NUMBER: _ClassVar[int]
    CHUNK_DIGEST_FIELD_NUMBER: _ClassVar[int]
    header: ProviderEventHeader
    scope: ProviderScope
    observation_family_code: int
    arrow_ipc: bytes
    payload_reference: BlobReference
    schema_digest: str
    row_count: int
    chunk_digest: str
    def __init__(self, header: _Optional[_Union[ProviderEventHeader, _Mapping]] = ..., scope: _Optional[_Union[ProviderScope, _Mapping]] = ..., observation_family_code: _Optional[int] = ..., arrow_ipc: _Optional[bytes] = ..., payload_reference: _Optional[_Union[BlobReference, _Mapping]] = ..., schema_digest: _Optional[str] = ..., row_count: _Optional[int] = ..., chunk_digest: _Optional[str] = ...) -> None: ...

class ProviderScopeEndEvent(_message.Message):
    __slots__ = ("header", "scope", "family_counts", "scope_digest")
    class FamilyCountsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: int
        value: int
        def __init__(self, key: _Optional[int] = ..., value: _Optional[int] = ...) -> None: ...
    HEADER_FIELD_NUMBER: _ClassVar[int]
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    FAMILY_COUNTS_FIELD_NUMBER: _ClassVar[int]
    SCOPE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    header: ProviderEventHeader
    scope: ProviderScope
    family_counts: _containers.ScalarMap[int, int]
    scope_digest: str
    def __init__(self, header: _Optional[_Union[ProviderEventHeader, _Mapping]] = ..., scope: _Optional[_Union[ProviderScope, _Mapping]] = ..., family_counts: _Optional[_Mapping[int, int]] = ..., scope_digest: _Optional[str] = ...) -> None: ...

class CapabilityOutcome(_message.Message):
    __slots__ = ("capability_code", "owner_capability_state_code", "completeness_state_code", "reason_code")
    CAPABILITY_CODE_FIELD_NUMBER: _ClassVar[int]
    OWNER_CAPABILITY_STATE_CODE_FIELD_NUMBER: _ClassVar[int]
    COMPLETENESS_STATE_CODE_FIELD_NUMBER: _ClassVar[int]
    REASON_CODE_FIELD_NUMBER: _ClassVar[int]
    capability_code: int
    owner_capability_state_code: int
    completeness_state_code: int
    reason_code: str
    def __init__(self, capability_code: _Optional[int] = ..., owner_capability_state_code: _Optional[int] = ..., completeness_state_code: _Optional[int] = ..., reason_code: _Optional[str] = ...) -> None: ...

class ProviderTerminalEvent(_message.Message):
    __slots__ = ("header", "state", "capability_outcomes", "overall_digest", "error_code")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    CAPABILITY_OUTCOMES_FIELD_NUMBER: _ClassVar[int]
    OVERALL_DIGEST_FIELD_NUMBER: _ClassVar[int]
    ERROR_CODE_FIELD_NUMBER: _ClassVar[int]
    header: ProviderEventHeader
    state: ProviderRunState
    capability_outcomes: _containers.RepeatedCompositeFieldContainer[CapabilityOutcome]
    overall_digest: str
    error_code: str
    def __init__(self, header: _Optional[_Union[ProviderEventHeader, _Mapping]] = ..., state: _Optional[_Union[ProviderRunState, str]] = ..., capability_outcomes: _Optional[_Iterable[_Union[CapabilityOutcome, _Mapping]]] = ..., overall_digest: _Optional[str] = ..., error_code: _Optional[str] = ...) -> None: ...

class CancelAcknowledgedEvent(_message.Message):
    __slots__ = ("header", "state")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    header: ProviderEventHeader
    state: CancelAcknowledgementState
    def __init__(self, header: _Optional[_Union[ProviderEventHeader, _Mapping]] = ..., state: _Optional[_Union[CancelAcknowledgementState, str]] = ...) -> None: ...

class ProviderEvent(_message.Message):
    __slots__ = ("accepted", "progress", "scope_begin", "observation_chunk", "scope_end", "terminal", "cancel_acknowledged")
    ACCEPTED_FIELD_NUMBER: _ClassVar[int]
    PROGRESS_FIELD_NUMBER: _ClassVar[int]
    SCOPE_BEGIN_FIELD_NUMBER: _ClassVar[int]
    OBSERVATION_CHUNK_FIELD_NUMBER: _ClassVar[int]
    SCOPE_END_FIELD_NUMBER: _ClassVar[int]
    TERMINAL_FIELD_NUMBER: _ClassVar[int]
    CANCEL_ACKNOWLEDGED_FIELD_NUMBER: _ClassVar[int]
    accepted: ProviderAcceptedEvent
    progress: ProviderProgressEvent
    scope_begin: ProviderScopeBeginEvent
    observation_chunk: ProviderObservationChunkEvent
    scope_end: ProviderScopeEndEvent
    terminal: ProviderTerminalEvent
    cancel_acknowledged: CancelAcknowledgedEvent
    def __init__(self, accepted: _Optional[_Union[ProviderAcceptedEvent, _Mapping]] = ..., progress: _Optional[_Union[ProviderProgressEvent, _Mapping]] = ..., scope_begin: _Optional[_Union[ProviderScopeBeginEvent, _Mapping]] = ..., observation_chunk: _Optional[_Union[ProviderObservationChunkEvent, _Mapping]] = ..., scope_end: _Optional[_Union[ProviderScopeEndEvent, _Mapping]] = ..., terminal: _Optional[_Union[ProviderTerminalEvent, _Mapping]] = ..., cancel_acknowledged: _Optional[_Union[CancelAcknowledgedEvent, _Mapping]] = ...) -> None: ...

class CancelProviderRunRequest(_message.Message):
    __slots__ = ("provider_run_id", "reason", "idempotency_key")
    PROVIDER_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    IDEMPOTENCY_KEY_FIELD_NUMBER: _ClassVar[int]
    provider_run_id: str
    reason: str
    idempotency_key: str
    def __init__(self, provider_run_id: _Optional[str] = ..., reason: _Optional[str] = ..., idempotency_key: _Optional[str] = ...) -> None: ...

class CancelAcknowledgement(_message.Message):
    __slots__ = ("provider_run_id", "state", "acknowledged_at_unix_ms", "terminal_state", "cleaning_up_components", "forced_termination")
    PROVIDER_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    ACKNOWLEDGED_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    TERMINAL_STATE_FIELD_NUMBER: _ClassVar[int]
    CLEANING_UP_COMPONENTS_FIELD_NUMBER: _ClassVar[int]
    FORCED_TERMINATION_FIELD_NUMBER: _ClassVar[int]
    provider_run_id: str
    state: CancelAcknowledgementState
    acknowledged_at_unix_ms: int
    terminal_state: ProviderRunState
    cleaning_up_components: _containers.RepeatedScalarFieldContainer[str]
    forced_termination: bool
    def __init__(self, provider_run_id: _Optional[str] = ..., state: _Optional[_Union[CancelAcknowledgementState, str]] = ..., acknowledged_at_unix_ms: _Optional[int] = ..., terminal_state: _Optional[_Union[ProviderRunState, str]] = ..., cleaning_up_components: _Optional[_Iterable[str]] = ..., forced_termination: _Optional[bool] = ...) -> None: ...

class ChunkAccepted(_message.Message):
    __slots__ = ("sequence", "next_credit_bytes", "next_credit_chunks")
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    NEXT_CREDIT_BYTES_FIELD_NUMBER: _ClassVar[int]
    NEXT_CREDIT_CHUNKS_FIELD_NUMBER: _ClassVar[int]
    sequence: int
    next_credit_bytes: int
    next_credit_chunks: int
    def __init__(self, sequence: _Optional[int] = ..., next_credit_bytes: _Optional[int] = ..., next_credit_chunks: _Optional[int] = ...) -> None: ...

class ChunkRejected(_message.Message):
    __slots__ = ("sequence", "error_code")
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    ERROR_CODE_FIELD_NUMBER: _ClassVar[int]
    sequence: int
    error_code: str
    def __init__(self, sequence: _Optional[int] = ..., error_code: _Optional[str] = ...) -> None: ...

class RelationIpcStreamIdentity(_message.Message):
    __slots__ = ("relation_id", "stream_id", "schema_fingerprint", "source_pin", "context_pin")
    RELATION_ID_FIELD_NUMBER: _ClassVar[int]
    STREAM_ID_FIELD_NUMBER: _ClassVar[int]
    SCHEMA_FINGERPRINT_FIELD_NUMBER: _ClassVar[int]
    SOURCE_PIN_FIELD_NUMBER: _ClassVar[int]
    CONTEXT_PIN_FIELD_NUMBER: _ClassVar[int]
    relation_id: bytes
    stream_id: bytes
    schema_fingerprint: bytes
    source_pin: bytes
    context_pin: bytes
    def __init__(self, relation_id: _Optional[bytes] = ..., stream_id: _Optional[bytes] = ..., schema_fingerprint: _Optional[bytes] = ..., source_pin: _Optional[bytes] = ..., context_pin: _Optional[bytes] = ...) -> None: ...

class RelationIpcFrameHeader(_message.Message):
    __slots__ = ("protocol_version", "identity", "sequence")
    PROTOCOL_VERSION_FIELD_NUMBER: _ClassVar[int]
    IDENTITY_FIELD_NUMBER: _ClassVar[int]
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    protocol_version: int
    identity: RelationIpcStreamIdentity
    sequence: int
    def __init__(self, protocol_version: _Optional[int] = ..., identity: _Optional[_Union[RelationIpcStreamIdentity, _Mapping]] = ..., sequence: _Optional[int] = ...) -> None: ...

class RelationIpcOpen(_message.Message):
    __slots__ = ("header", "requested_units", "arrow_type_universe", "metadata_version", "semantic_encoding")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    REQUESTED_UNITS_FIELD_NUMBER: _ClassVar[int]
    ARROW_TYPE_UNIVERSE_FIELD_NUMBER: _ClassVar[int]
    METADATA_VERSION_FIELD_NUMBER: _ClassVar[int]
    SEMANTIC_ENCODING_FIELD_NUMBER: _ClassVar[int]
    header: RelationIpcFrameHeader
    requested_units: int
    arrow_type_universe: str
    metadata_version: ArrowIpcMetadataVersion
    semantic_encoding: str
    def __init__(self, header: _Optional[_Union[RelationIpcFrameHeader, _Mapping]] = ..., requested_units: _Optional[int] = ..., arrow_type_universe: _Optional[str] = ..., metadata_version: _Optional[_Union[ArrowIpcMetadataVersion, str]] = ..., semantic_encoding: _Optional[str] = ...) -> None: ...

class RelationIpcPayload(_message.Message):
    __slots__ = ("header", "arrow_ipc_fragment")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    ARROW_IPC_FRAGMENT_FIELD_NUMBER: _ClassVar[int]
    header: RelationIpcFrameHeader
    arrow_ipc_fragment: bytes
    def __init__(self, header: _Optional[_Union[RelationIpcFrameHeader, _Mapping]] = ..., arrow_ipc_fragment: _Optional[bytes] = ...) -> None: ...

class RelationIpcFlowControlAck(_message.Message):
    __slots__ = ("header", "acknowledged_sequence", "released_bytes", "cancelled")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    ACKNOWLEDGED_SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    RELEASED_BYTES_FIELD_NUMBER: _ClassVar[int]
    CANCELLED_FIELD_NUMBER: _ClassVar[int]
    header: RelationIpcFrameHeader
    acknowledged_sequence: int
    released_bytes: int
    cancelled: bool
    def __init__(self, header: _Optional[_Union[RelationIpcFrameHeader, _Mapping]] = ..., acknowledged_sequence: _Optional[int] = ..., released_bytes: _Optional[int] = ..., cancelled: _Optional[bool] = ...) -> None: ...

class RelationIpcEnd(_message.Message):
    __slots__ = ("header", "declared_ipc_bytes", "declared_batches", "declared_rows")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    DECLARED_IPC_BYTES_FIELD_NUMBER: _ClassVar[int]
    DECLARED_BATCHES_FIELD_NUMBER: _ClassVar[int]
    DECLARED_ROWS_FIELD_NUMBER: _ClassVar[int]
    header: RelationIpcFrameHeader
    declared_ipc_bytes: int
    declared_batches: int
    declared_rows: int
    def __init__(self, header: _Optional[_Union[RelationIpcFrameHeader, _Mapping]] = ..., declared_ipc_bytes: _Optional[int] = ..., declared_batches: _Optional[int] = ..., declared_rows: _Optional[int] = ...) -> None: ...

class RelationIpcCoverageRemainder(_message.Message):
    __slots__ = ("scope", "unit_count", "reason")
    SCOPE_FIELD_NUMBER: _ClassVar[int]
    UNIT_COUNT_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    scope: bytes
    unit_count: int
    reason: RelationIpcRemainderReason
    def __init__(self, scope: _Optional[bytes] = ..., unit_count: _Optional[int] = ..., reason: _Optional[_Union[RelationIpcRemainderReason, str]] = ...) -> None: ...

class RelationIpcCoverageTrailer(_message.Message):
    __slots__ = ("header", "status", "requested_units", "completed_units", "remainders")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    REQUESTED_UNITS_FIELD_NUMBER: _ClassVar[int]
    COMPLETED_UNITS_FIELD_NUMBER: _ClassVar[int]
    REMAINDERS_FIELD_NUMBER: _ClassVar[int]
    header: RelationIpcFrameHeader
    status: RelationIpcTerminalStatus
    requested_units: int
    completed_units: int
    remainders: _containers.RepeatedCompositeFieldContainer[RelationIpcCoverageRemainder]
    def __init__(self, header: _Optional[_Union[RelationIpcFrameHeader, _Mapping]] = ..., status: _Optional[_Union[RelationIpcTerminalStatus, str]] = ..., requested_units: _Optional[int] = ..., completed_units: _Optional[int] = ..., remainders: _Optional[_Iterable[_Union[RelationIpcCoverageRemainder, _Mapping]]] = ...) -> None: ...

class RelationIpcTerminal(_message.Message):
    __slots__ = ("header", "status")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    header: RelationIpcFrameHeader
    status: RelationIpcTerminalStatus
    def __init__(self, header: _Optional[_Union[RelationIpcFrameHeader, _Mapping]] = ..., status: _Optional[_Union[RelationIpcTerminalStatus, str]] = ...) -> None: ...

class RelationIpcFrame(_message.Message):
    __slots__ = ("open", "payload", "flow_control_ack", "ipc_end", "coverage_trailer", "terminal")
    OPEN_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    FLOW_CONTROL_ACK_FIELD_NUMBER: _ClassVar[int]
    IPC_END_FIELD_NUMBER: _ClassVar[int]
    COVERAGE_TRAILER_FIELD_NUMBER: _ClassVar[int]
    TERMINAL_FIELD_NUMBER: _ClassVar[int]
    open: RelationIpcOpen
    payload: RelationIpcPayload
    flow_control_ack: RelationIpcFlowControlAck
    ipc_end: RelationIpcEnd
    coverage_trailer: RelationIpcCoverageTrailer
    terminal: RelationIpcTerminal
    def __init__(self, open: _Optional[_Union[RelationIpcOpen, _Mapping]] = ..., payload: _Optional[_Union[RelationIpcPayload, _Mapping]] = ..., flow_control_ack: _Optional[_Union[RelationIpcFlowControlAck, _Mapping]] = ..., ipc_end: _Optional[_Union[RelationIpcEnd, _Mapping]] = ..., coverage_trailer: _Optional[_Union[RelationIpcCoverageTrailer, _Mapping]] = ..., terminal: _Optional[_Union[RelationIpcTerminal, _Mapping]] = ...) -> None: ...
