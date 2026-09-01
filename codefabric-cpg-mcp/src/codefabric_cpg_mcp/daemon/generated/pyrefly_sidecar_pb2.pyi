# @generated from released Protobuf semantic identities b3:502dfd819e70a154db899bd6bdbe580d01bb56f1654790d5adb241199d43b434,b3:71fb94283214d79068ede88e0f45e1460336b23b9678f80b4ddbece098cd626f,b3:d5b256baca150eed2617f78f88362c607ff12db7a94af9524658a3c82f247973,b3:2f2c24a2877be95dfd1d3acc7d83354838696af2aaac13c99bde83ab743f6c62; do not edit.
from . import provider_control_pb2 as _provider_control_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class Hello(_message.Message):
    __slots__ = ("protocol_major", "protocol_minor", "required_feature_bits", "optional_feature_bits", "daemon_build", "supported_python_versions", "observation_schema_digests", "maximum_frame_bytes", "maximum_arrow_ipc_bytes", "sandbox_profile_digest")
    PROTOCOL_MAJOR_FIELD_NUMBER: _ClassVar[int]
    PROTOCOL_MINOR_FIELD_NUMBER: _ClassVar[int]
    REQUIRED_FEATURE_BITS_FIELD_NUMBER: _ClassVar[int]
    OPTIONAL_FEATURE_BITS_FIELD_NUMBER: _ClassVar[int]
    DAEMON_BUILD_FIELD_NUMBER: _ClassVar[int]
    SUPPORTED_PYTHON_VERSIONS_FIELD_NUMBER: _ClassVar[int]
    OBSERVATION_SCHEMA_DIGESTS_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_FRAME_BYTES_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_ARROW_IPC_BYTES_FIELD_NUMBER: _ClassVar[int]
    SANDBOX_PROFILE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    protocol_major: int
    protocol_minor: int
    required_feature_bits: int
    optional_feature_bits: int
    daemon_build: str
    supported_python_versions: _containers.RepeatedScalarFieldContainer[str]
    observation_schema_digests: _containers.RepeatedScalarFieldContainer[str]
    maximum_frame_bytes: int
    maximum_arrow_ipc_bytes: int
    sandbox_profile_digest: str
    def __init__(self, protocol_major: _Optional[int] = ..., protocol_minor: _Optional[int] = ..., required_feature_bits: _Optional[int] = ..., optional_feature_bits: _Optional[int] = ..., daemon_build: _Optional[str] = ..., supported_python_versions: _Optional[_Iterable[str]] = ..., observation_schema_digests: _Optional[_Iterable[str]] = ..., maximum_frame_bytes: _Optional[int] = ..., maximum_arrow_ipc_bytes: _Optional[int] = ..., sandbox_profile_digest: _Optional[str] = ...) -> None: ...

class HelloAck(_message.Message):
    __slots__ = ("protocol_major", "protocol_minor", "negotiated_feature_bits", "sidecar_build", "pyrefly_source_digest", "supported_python_versions", "observation_schema_digests", "maximum_frame_bytes", "maximum_arrow_ipc_bytes", "sandbox_profile_digest")
    PROTOCOL_MAJOR_FIELD_NUMBER: _ClassVar[int]
    PROTOCOL_MINOR_FIELD_NUMBER: _ClassVar[int]
    NEGOTIATED_FEATURE_BITS_FIELD_NUMBER: _ClassVar[int]
    SIDECAR_BUILD_FIELD_NUMBER: _ClassVar[int]
    PYREFLY_SOURCE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    SUPPORTED_PYTHON_VERSIONS_FIELD_NUMBER: _ClassVar[int]
    OBSERVATION_SCHEMA_DIGESTS_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_FRAME_BYTES_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_ARROW_IPC_BYTES_FIELD_NUMBER: _ClassVar[int]
    SANDBOX_PROFILE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    protocol_major: int
    protocol_minor: int
    negotiated_feature_bits: int
    sidecar_build: str
    pyrefly_source_digest: str
    supported_python_versions: _containers.RepeatedScalarFieldContainer[str]
    observation_schema_digests: _containers.RepeatedScalarFieldContainer[str]
    maximum_frame_bytes: int
    maximum_arrow_ipc_bytes: int
    sandbox_profile_digest: str
    def __init__(self, protocol_major: _Optional[int] = ..., protocol_minor: _Optional[int] = ..., negotiated_feature_bits: _Optional[int] = ..., sidecar_build: _Optional[str] = ..., pyrefly_source_digest: _Optional[str] = ..., supported_python_versions: _Optional[_Iterable[str]] = ..., observation_schema_digests: _Optional[_Iterable[str]] = ..., maximum_frame_bytes: _Optional[int] = ..., maximum_arrow_ipc_bytes: _Optional[int] = ..., sandbox_profile_digest: _Optional[str] = ...) -> None: ...

class OpenContextRequest(_message.Message):
    __slots__ = ("workspace_id", "analysis_context_id", "immutable_context_manifest", "context_manifest_digest", "source_snapshot_lease", "resource_profile_id", "maximum_contexts", "maximum_memory_mib", "sandbox_profile_digest")
    WORKSPACE_ID_FIELD_NUMBER: _ClassVar[int]
    ANALYSIS_CONTEXT_ID_FIELD_NUMBER: _ClassVar[int]
    IMMUTABLE_CONTEXT_MANIFEST_FIELD_NUMBER: _ClassVar[int]
    CONTEXT_MANIFEST_DIGEST_FIELD_NUMBER: _ClassVar[int]
    SOURCE_SNAPSHOT_LEASE_FIELD_NUMBER: _ClassVar[int]
    RESOURCE_PROFILE_ID_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_CONTEXTS_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_MEMORY_MIB_FIELD_NUMBER: _ClassVar[int]
    SANDBOX_PROFILE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    workspace_id: str
    analysis_context_id: str
    immutable_context_manifest: bytes
    context_manifest_digest: str
    source_snapshot_lease: _provider_control_pb2.SourceSnapshotLease
    resource_profile_id: str
    maximum_contexts: int
    maximum_memory_mib: int
    sandbox_profile_digest: str
    def __init__(self, workspace_id: _Optional[str] = ..., analysis_context_id: _Optional[str] = ..., immutable_context_manifest: _Optional[bytes] = ..., context_manifest_digest: _Optional[str] = ..., source_snapshot_lease: _Optional[_Union[_provider_control_pb2.SourceSnapshotLease, _Mapping]] = ..., resource_profile_id: _Optional[str] = ..., maximum_contexts: _Optional[int] = ..., maximum_memory_mib: _Optional[int] = ..., sandbox_profile_digest: _Optional[str] = ...) -> None: ...

class OpenContextResponse(_message.Message):
    __slots__ = ("context_handle", "context_manifest_digest", "opened_at_unix_ms", "sandbox_profile_digest")
    CONTEXT_HANDLE_FIELD_NUMBER: _ClassVar[int]
    CONTEXT_MANIFEST_DIGEST_FIELD_NUMBER: _ClassVar[int]
    OPENED_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    SANDBOX_PROFILE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    context_handle: str
    context_manifest_digest: str
    opened_at_unix_ms: int
    sandbox_profile_digest: str
    def __init__(self, context_handle: _Optional[str] = ..., context_manifest_digest: _Optional[str] = ..., opened_at_unix_ms: _Optional[int] = ..., sandbox_profile_digest: _Optional[str] = ...) -> None: ...

class ModuleRequest(_message.Message):
    __slots__ = ("module_id", "module_name", "file_id", "source_digest", "source_blob", "dependency_generation", "module_resolution_generation")
    MODULE_ID_FIELD_NUMBER: _ClassVar[int]
    MODULE_NAME_FIELD_NUMBER: _ClassVar[int]
    FILE_ID_FIELD_NUMBER: _ClassVar[int]
    SOURCE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    SOURCE_BLOB_FIELD_NUMBER: _ClassVar[int]
    DEPENDENCY_GENERATION_FIELD_NUMBER: _ClassVar[int]
    MODULE_RESOLUTION_GENERATION_FIELD_NUMBER: _ClassVar[int]
    module_id: str
    module_name: str
    file_id: str
    source_digest: str
    source_blob: _provider_control_pb2.BlobReference
    dependency_generation: int
    module_resolution_generation: int
    def __init__(self, module_id: _Optional[str] = ..., module_name: _Optional[str] = ..., file_id: _Optional[str] = ..., source_digest: _Optional[str] = ..., source_blob: _Optional[_Union[_provider_control_pb2.BlobReference, _Mapping]] = ..., dependency_generation: _Optional[int] = ..., module_resolution_generation: _Optional[int] = ...) -> None: ...

class AnalyzeModulesRequest(_message.Message):
    __slots__ = ("provider_run_id", "workspace_id", "analysis_context_id", "context_handle", "context_manifest_digest", "source_generation", "source_snapshot_lease_id", "modules", "requested_capability_codes", "deadline_unix_ms", "output_schema_bundle_digest", "initial_frame_credits", "initial_credit_bytes", "sandbox_profile_digest", "trust_profile", "resource_profile_id")
    PROVIDER_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    WORKSPACE_ID_FIELD_NUMBER: _ClassVar[int]
    ANALYSIS_CONTEXT_ID_FIELD_NUMBER: _ClassVar[int]
    CONTEXT_HANDLE_FIELD_NUMBER: _ClassVar[int]
    CONTEXT_MANIFEST_DIGEST_FIELD_NUMBER: _ClassVar[int]
    SOURCE_GENERATION_FIELD_NUMBER: _ClassVar[int]
    SOURCE_SNAPSHOT_LEASE_ID_FIELD_NUMBER: _ClassVar[int]
    MODULES_FIELD_NUMBER: _ClassVar[int]
    REQUESTED_CAPABILITY_CODES_FIELD_NUMBER: _ClassVar[int]
    DEADLINE_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    OUTPUT_SCHEMA_BUNDLE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    INITIAL_FRAME_CREDITS_FIELD_NUMBER: _ClassVar[int]
    INITIAL_CREDIT_BYTES_FIELD_NUMBER: _ClassVar[int]
    SANDBOX_PROFILE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    TRUST_PROFILE_FIELD_NUMBER: _ClassVar[int]
    RESOURCE_PROFILE_ID_FIELD_NUMBER: _ClassVar[int]
    provider_run_id: str
    workspace_id: str
    analysis_context_id: str
    context_handle: str
    context_manifest_digest: str
    source_generation: int
    source_snapshot_lease_id: str
    modules: _containers.RepeatedCompositeFieldContainer[ModuleRequest]
    requested_capability_codes: _containers.RepeatedScalarFieldContainer[int]
    deadline_unix_ms: int
    output_schema_bundle_digest: str
    initial_frame_credits: int
    initial_credit_bytes: int
    sandbox_profile_digest: str
    trust_profile: str
    resource_profile_id: str
    def __init__(self, provider_run_id: _Optional[str] = ..., workspace_id: _Optional[str] = ..., analysis_context_id: _Optional[str] = ..., context_handle: _Optional[str] = ..., context_manifest_digest: _Optional[str] = ..., source_generation: _Optional[int] = ..., source_snapshot_lease_id: _Optional[str] = ..., modules: _Optional[_Iterable[_Union[ModuleRequest, _Mapping]]] = ..., requested_capability_codes: _Optional[_Iterable[int]] = ..., deadline_unix_ms: _Optional[int] = ..., output_schema_bundle_digest: _Optional[str] = ..., initial_frame_credits: _Optional[int] = ..., initial_credit_bytes: _Optional[int] = ..., sandbox_profile_digest: _Optional[str] = ..., trust_profile: _Optional[str] = ..., resource_profile_id: _Optional[str] = ...) -> None: ...

class AnalyzeCommand(_message.Message):
    __slots__ = ("start", "cancel", "relation_ipc_ack")
    START_FIELD_NUMBER: _ClassVar[int]
    CANCEL_FIELD_NUMBER: _ClassVar[int]
    RELATION_IPC_ACK_FIELD_NUMBER: _ClassVar[int]
    start: AnalyzeModulesRequest
    cancel: CancelRunRequest
    relation_ipc_ack: _provider_control_pb2.RelationIpcFrame
    def __init__(self, start: _Optional[_Union[AnalyzeModulesRequest, _Mapping]] = ..., cancel: _Optional[_Union[CancelRunRequest, _Mapping]] = ..., relation_ipc_ack: _Optional[_Union[_provider_control_pb2.RelationIpcFrame, _Mapping]] = ...) -> None: ...

class AnalyzeEventHeader(_message.Message):
    __slots__ = ("provider_run_id", "workspace_id", "analysis_context_id", "source_generation", "sequence", "context_manifest_digest", "source_manifest_digest", "sandbox_profile_digest")
    PROVIDER_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    WORKSPACE_ID_FIELD_NUMBER: _ClassVar[int]
    ANALYSIS_CONTEXT_ID_FIELD_NUMBER: _ClassVar[int]
    SOURCE_GENERATION_FIELD_NUMBER: _ClassVar[int]
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    CONTEXT_MANIFEST_DIGEST_FIELD_NUMBER: _ClassVar[int]
    SOURCE_MANIFEST_DIGEST_FIELD_NUMBER: _ClassVar[int]
    SANDBOX_PROFILE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    provider_run_id: str
    workspace_id: str
    analysis_context_id: str
    source_generation: int
    sequence: int
    context_manifest_digest: str
    source_manifest_digest: str
    sandbox_profile_digest: str
    def __init__(self, provider_run_id: _Optional[str] = ..., workspace_id: _Optional[str] = ..., analysis_context_id: _Optional[str] = ..., source_generation: _Optional[int] = ..., sequence: _Optional[int] = ..., context_manifest_digest: _Optional[str] = ..., source_manifest_digest: _Optional[str] = ..., sandbox_profile_digest: _Optional[str] = ...) -> None: ...

class RunAccepted(_message.Message):
    __slots__ = ("header", "granted_frame_credits", "granted_credit_bytes")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    GRANTED_FRAME_CREDITS_FIELD_NUMBER: _ClassVar[int]
    GRANTED_CREDIT_BYTES_FIELD_NUMBER: _ClassVar[int]
    header: AnalyzeEventHeader
    granted_frame_credits: int
    granted_credit_bytes: int
    def __init__(self, header: _Optional[_Union[AnalyzeEventHeader, _Mapping]] = ..., granted_frame_credits: _Optional[int] = ..., granted_credit_bytes: _Optional[int] = ...) -> None: ...

class RunProgress(_message.Message):
    __slots__ = ("header", "completed_modules", "total_modules")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    COMPLETED_MODULES_FIELD_NUMBER: _ClassVar[int]
    TOTAL_MODULES_FIELD_NUMBER: _ClassVar[int]
    header: AnalyzeEventHeader
    completed_modules: int
    total_modules: int
    def __init__(self, header: _Optional[_Union[AnalyzeEventHeader, _Mapping]] = ..., completed_modules: _Optional[int] = ..., total_modules: _Optional[int] = ...) -> None: ...

class ModuleBegin(_message.Message):
    __slots__ = ("header", "module_id")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    MODULE_ID_FIELD_NUMBER: _ClassVar[int]
    header: AnalyzeEventHeader
    module_id: str
    def __init__(self, header: _Optional[_Union[AnalyzeEventHeader, _Mapping]] = ..., module_id: _Optional[str] = ...) -> None: ...

class RelationIpcFrameEvent(_message.Message):
    __slots__ = ("header", "module_id", "observation_family_code", "frame")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    MODULE_ID_FIELD_NUMBER: _ClassVar[int]
    OBSERVATION_FAMILY_CODE_FIELD_NUMBER: _ClassVar[int]
    FRAME_FIELD_NUMBER: _ClassVar[int]
    header: AnalyzeEventHeader
    module_id: str
    observation_family_code: int
    frame: _provider_control_pb2.RelationIpcFrame
    def __init__(self, header: _Optional[_Union[AnalyzeEventHeader, _Mapping]] = ..., module_id: _Optional[str] = ..., observation_family_code: _Optional[int] = ..., frame: _Optional[_Union[_provider_control_pb2.RelationIpcFrame, _Mapping]] = ...) -> None: ...

class ModuleEnd(_message.Message):
    __slots__ = ("header", "module_id", "family_counts", "module_digest")
    class FamilyCountsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: int
        value: int
        def __init__(self, key: _Optional[int] = ..., value: _Optional[int] = ...) -> None: ...
    HEADER_FIELD_NUMBER: _ClassVar[int]
    MODULE_ID_FIELD_NUMBER: _ClassVar[int]
    FAMILY_COUNTS_FIELD_NUMBER: _ClassVar[int]
    MODULE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    header: AnalyzeEventHeader
    module_id: str
    family_counts: _containers.ScalarMap[int, int]
    module_digest: str
    def __init__(self, header: _Optional[_Union[AnalyzeEventHeader, _Mapping]] = ..., module_id: _Optional[str] = ..., family_counts: _Optional[_Mapping[int, int]] = ..., module_digest: _Optional[str] = ...) -> None: ...

class RunTerminal(_message.Message):
    __slots__ = ("header", "ordered_module_digests", "capability_outcomes", "overall_digest", "terminal_state", "rechecked_module_ids", "sandbox_profile_digest", "trust_profile")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    ORDERED_MODULE_DIGESTS_FIELD_NUMBER: _ClassVar[int]
    CAPABILITY_OUTCOMES_FIELD_NUMBER: _ClassVar[int]
    OVERALL_DIGEST_FIELD_NUMBER: _ClassVar[int]
    TERMINAL_STATE_FIELD_NUMBER: _ClassVar[int]
    RECHECKED_MODULE_IDS_FIELD_NUMBER: _ClassVar[int]
    SANDBOX_PROFILE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    TRUST_PROFILE_FIELD_NUMBER: _ClassVar[int]
    header: AnalyzeEventHeader
    ordered_module_digests: _containers.RepeatedScalarFieldContainer[str]
    capability_outcomes: _containers.RepeatedCompositeFieldContainer[_provider_control_pb2.CapabilityOutcome]
    overall_digest: str
    terminal_state: _provider_control_pb2.ProviderRunState
    rechecked_module_ids: _containers.RepeatedScalarFieldContainer[str]
    sandbox_profile_digest: str
    trust_profile: str
    def __init__(self, header: _Optional[_Union[AnalyzeEventHeader, _Mapping]] = ..., ordered_module_digests: _Optional[_Iterable[str]] = ..., capability_outcomes: _Optional[_Iterable[_Union[_provider_control_pb2.CapabilityOutcome, _Mapping]]] = ..., overall_digest: _Optional[str] = ..., terminal_state: _Optional[_Union[_provider_control_pb2.ProviderRunState, str]] = ..., rechecked_module_ids: _Optional[_Iterable[str]] = ..., sandbox_profile_digest: _Optional[str] = ..., trust_profile: _Optional[str] = ...) -> None: ...

class AnalyzeEvent(_message.Message):
    __slots__ = ("run_accepted", "run_progress", "module_begin", "module_end", "run_terminal", "relation_ipc_frame")
    RUN_ACCEPTED_FIELD_NUMBER: _ClassVar[int]
    RUN_PROGRESS_FIELD_NUMBER: _ClassVar[int]
    MODULE_BEGIN_FIELD_NUMBER: _ClassVar[int]
    MODULE_END_FIELD_NUMBER: _ClassVar[int]
    RUN_TERMINAL_FIELD_NUMBER: _ClassVar[int]
    RELATION_IPC_FRAME_FIELD_NUMBER: _ClassVar[int]
    run_accepted: RunAccepted
    run_progress: RunProgress
    module_begin: ModuleBegin
    module_end: ModuleEnd
    run_terminal: RunTerminal
    relation_ipc_frame: RelationIpcFrameEvent
    def __init__(self, run_accepted: _Optional[_Union[RunAccepted, _Mapping]] = ..., run_progress: _Optional[_Union[RunProgress, _Mapping]] = ..., module_begin: _Optional[_Union[ModuleBegin, _Mapping]] = ..., module_end: _Optional[_Union[ModuleEnd, _Mapping]] = ..., run_terminal: _Optional[_Union[RunTerminal, _Mapping]] = ..., relation_ipc_frame: _Optional[_Union[RelationIpcFrameEvent, _Mapping]] = ...) -> None: ...

class CancelRunRequest(_message.Message):
    __slots__ = ("provider_run_id", "reason")
    PROVIDER_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    provider_run_id: str
    reason: str
    def __init__(self, provider_run_id: _Optional[str] = ..., reason: _Optional[str] = ...) -> None: ...

class CloseContextRequest(_message.Message):
    __slots__ = ("context_handle",)
    CONTEXT_HANDLE_FIELD_NUMBER: _ClassVar[int]
    context_handle: str
    def __init__(self, context_handle: _Optional[str] = ...) -> None: ...

class CloseContextResponse(_message.Message):
    __slots__ = ("closed",)
    CLOSED_FIELD_NUMBER: _ClassVar[int]
    closed: bool
    def __init__(self, closed: _Optional[bool] = ...) -> None: ...

class ShutdownRequest(_message.Message):
    __slots__ = ("reason",)
    REASON_FIELD_NUMBER: _ClassVar[int]
    reason: str
    def __init__(self, reason: _Optional[str] = ...) -> None: ...

class ShutdownResponse(_message.Message):
    __slots__ = ("accepted",)
    ACCEPTED_FIELD_NUMBER: _ClassVar[int]
    accepted: bool
    def __init__(self, accepted: _Optional[bool] = ...) -> None: ...
