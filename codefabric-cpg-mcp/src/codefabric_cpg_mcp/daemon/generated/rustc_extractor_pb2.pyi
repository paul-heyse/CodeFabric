# @generated from released Protobuf semantic identities b3:502dfd819e70a154db899bd6bdbe580d01bb56f1654790d5adb241199d43b434,b3:71fb94283214d79068ede88e0f45e1460336b23b9678f80b4ddbece098cd626f,b3:d5b256baca150eed2617f78f88362c607ff12db7a94af9524658a3c82f247973,b3:2f2c24a2877be95dfd1d3acc7d83354838696af2aaac13c99bde83ab743f6c62; do not edit.
from . import provider_control_pb2 as _provider_control_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class ExtractorEnvironmentVariable(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    EXTRACTOR_ENVIRONMENT_VARIABLE_UNSPECIFIED: _ClassVar[ExtractorEnvironmentVariable]
    EXTRACTOR_ENVIRONMENT_VARIABLE_ENDPOINT: _ClassVar[ExtractorEnvironmentVariable]
    EXTRACTOR_ENVIRONMENT_VARIABLE_PROVIDER_RUN_ID: _ClassVar[ExtractorEnvironmentVariable]
    EXTRACTOR_ENVIRONMENT_VARIABLE_WORKSPACE_ID: _ClassVar[ExtractorEnvironmentVariable]
    EXTRACTOR_ENVIRONMENT_VARIABLE_ANALYSIS_CONTEXT_ID: _ClassVar[ExtractorEnvironmentVariable]
    EXTRACTOR_ENVIRONMENT_VARIABLE_SOURCE_GENERATION: _ClassVar[ExtractorEnvironmentVariable]
    EXTRACTOR_ENVIRONMENT_VARIABLE_CONTEXT_MANIFEST_DIGEST: _ClassVar[ExtractorEnvironmentVariable]
    EXTRACTOR_ENVIRONMENT_VARIABLE_PROVIDER_RESOURCE_PROFILE_ID: _ClassVar[ExtractorEnvironmentVariable]
    EXTRACTOR_ENVIRONMENT_VARIABLE_SOURCE_SNAPSHOT_MANIFEST_DIGEST: _ClassVar[ExtractorEnvironmentVariable]
    EXTRACTOR_ENVIRONMENT_VARIABLE_CARGO_METADATA_DIGEST: _ClassVar[ExtractorEnvironmentVariable]
    EXTRACTOR_ENVIRONMENT_VARIABLE_CARGO_LOCK_DIGEST: _ClassVar[ExtractorEnvironmentVariable]
    EXTRACTOR_ENVIRONMENT_VARIABLE_CARGO_CONFIG_DIGEST: _ClassVar[ExtractorEnvironmentVariable]

class RejectionRuleErrorCode(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    REJECTION_RULE_ERROR_CODE_UNSPECIFIED: _ClassVar[RejectionRuleErrorCode]
    REJECTION_RULE_ERROR_CODE_UNEXPECTED_OWNER: _ClassVar[RejectionRuleErrorCode]
    REJECTION_RULE_ERROR_CODE_DUPLICATE_SEQUENCE: _ClassVar[RejectionRuleErrorCode]
    REJECTION_RULE_ERROR_CODE_MISSING_END_RECORD: _ClassVar[RejectionRuleErrorCode]
    REJECTION_RULE_ERROR_CODE_COUNT_MISMATCH: _ClassVar[RejectionRuleErrorCode]
    REJECTION_RULE_ERROR_CODE_STALE_SOURCE_OR_CONTEXT: _ClassVar[RejectionRuleErrorCode]
    REJECTION_RULE_ERROR_CODE_PROTOCOL_EOF: _ClassVar[RejectionRuleErrorCode]
    REJECTION_RULE_ERROR_CODE_COMPILER_FAILED: _ClassVar[RejectionRuleErrorCode]
    REJECTION_RULE_ERROR_CODE_CREDIT_EXCEEDED: _ClassVar[RejectionRuleErrorCode]
    REJECTION_RULE_ERROR_CODE_PROVIDER_DEADLINE: _ClassVar[RejectionRuleErrorCode]
    REJECTION_RULE_ERROR_CODE_WORKSPACE_ESCAPE: _ClassVar[RejectionRuleErrorCode]
EXTRACTOR_ENVIRONMENT_VARIABLE_UNSPECIFIED: ExtractorEnvironmentVariable
EXTRACTOR_ENVIRONMENT_VARIABLE_ENDPOINT: ExtractorEnvironmentVariable
EXTRACTOR_ENVIRONMENT_VARIABLE_PROVIDER_RUN_ID: ExtractorEnvironmentVariable
EXTRACTOR_ENVIRONMENT_VARIABLE_WORKSPACE_ID: ExtractorEnvironmentVariable
EXTRACTOR_ENVIRONMENT_VARIABLE_ANALYSIS_CONTEXT_ID: ExtractorEnvironmentVariable
EXTRACTOR_ENVIRONMENT_VARIABLE_SOURCE_GENERATION: ExtractorEnvironmentVariable
EXTRACTOR_ENVIRONMENT_VARIABLE_CONTEXT_MANIFEST_DIGEST: ExtractorEnvironmentVariable
EXTRACTOR_ENVIRONMENT_VARIABLE_PROVIDER_RESOURCE_PROFILE_ID: ExtractorEnvironmentVariable
EXTRACTOR_ENVIRONMENT_VARIABLE_SOURCE_SNAPSHOT_MANIFEST_DIGEST: ExtractorEnvironmentVariable
EXTRACTOR_ENVIRONMENT_VARIABLE_CARGO_METADATA_DIGEST: ExtractorEnvironmentVariable
EXTRACTOR_ENVIRONMENT_VARIABLE_CARGO_LOCK_DIGEST: ExtractorEnvironmentVariable
EXTRACTOR_ENVIRONMENT_VARIABLE_CARGO_CONFIG_DIGEST: ExtractorEnvironmentVariable
REJECTION_RULE_ERROR_CODE_UNSPECIFIED: RejectionRuleErrorCode
REJECTION_RULE_ERROR_CODE_UNEXPECTED_OWNER: RejectionRuleErrorCode
REJECTION_RULE_ERROR_CODE_DUPLICATE_SEQUENCE: RejectionRuleErrorCode
REJECTION_RULE_ERROR_CODE_MISSING_END_RECORD: RejectionRuleErrorCode
REJECTION_RULE_ERROR_CODE_COUNT_MISMATCH: RejectionRuleErrorCode
REJECTION_RULE_ERROR_CODE_STALE_SOURCE_OR_CONTEXT: RejectionRuleErrorCode
REJECTION_RULE_ERROR_CODE_PROTOCOL_EOF: RejectionRuleErrorCode
REJECTION_RULE_ERROR_CODE_COMPILER_FAILED: RejectionRuleErrorCode
REJECTION_RULE_ERROR_CODE_CREDIT_EXCEEDED: RejectionRuleErrorCode
REJECTION_RULE_ERROR_CODE_PROVIDER_DEADLINE: RejectionRuleErrorCode
REJECTION_RULE_ERROR_CODE_WORKSPACE_ESCAPE: RejectionRuleErrorCode

class ExtractorHello(_message.Message):
    __slots__ = ("protocol_major", "protocol_minor", "required_feature_bits", "optional_feature_bits", "extractor_build", "rustc_version", "rustc_commit", "toolchain_identity_digest", "resource_profile_id")
    PROTOCOL_MAJOR_FIELD_NUMBER: _ClassVar[int]
    PROTOCOL_MINOR_FIELD_NUMBER: _ClassVar[int]
    REQUIRED_FEATURE_BITS_FIELD_NUMBER: _ClassVar[int]
    OPTIONAL_FEATURE_BITS_FIELD_NUMBER: _ClassVar[int]
    EXTRACTOR_BUILD_FIELD_NUMBER: _ClassVar[int]
    RUSTC_VERSION_FIELD_NUMBER: _ClassVar[int]
    RUSTC_COMMIT_FIELD_NUMBER: _ClassVar[int]
    TOOLCHAIN_IDENTITY_DIGEST_FIELD_NUMBER: _ClassVar[int]
    RESOURCE_PROFILE_ID_FIELD_NUMBER: _ClassVar[int]
    protocol_major: int
    protocol_minor: int
    required_feature_bits: int
    optional_feature_bits: int
    extractor_build: str
    rustc_version: str
    rustc_commit: str
    toolchain_identity_digest: str
    resource_profile_id: str
    def __init__(self, protocol_major: _Optional[int] = ..., protocol_minor: _Optional[int] = ..., required_feature_bits: _Optional[int] = ..., optional_feature_bits: _Optional[int] = ..., extractor_build: _Optional[str] = ..., rustc_version: _Optional[str] = ..., rustc_commit: _Optional[str] = ..., toolchain_identity_digest: _Optional[str] = ..., resource_profile_id: _Optional[str] = ...) -> None: ...

class ExtractorHelloAck(_message.Message):
    __slots__ = ("protocol_major", "protocol_minor", "negotiated_feature_bits", "daemon_build", "output_schema_bundle_digest", "sandbox_profile_digest", "maximum_outstanding_chunks", "maximum_unacknowledged_bytes", "accepted_resource_profile_id", "provider_deadline_unix_ms")
    PROTOCOL_MAJOR_FIELD_NUMBER: _ClassVar[int]
    PROTOCOL_MINOR_FIELD_NUMBER: _ClassVar[int]
    NEGOTIATED_FEATURE_BITS_FIELD_NUMBER: _ClassVar[int]
    DAEMON_BUILD_FIELD_NUMBER: _ClassVar[int]
    OUTPUT_SCHEMA_BUNDLE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    SANDBOX_PROFILE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_OUTSTANDING_CHUNKS_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_UNACKNOWLEDGED_BYTES_FIELD_NUMBER: _ClassVar[int]
    ACCEPTED_RESOURCE_PROFILE_ID_FIELD_NUMBER: _ClassVar[int]
    PROVIDER_DEADLINE_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    protocol_major: int
    protocol_minor: int
    negotiated_feature_bits: int
    daemon_build: str
    output_schema_bundle_digest: str
    sandbox_profile_digest: str
    maximum_outstanding_chunks: int
    maximum_unacknowledged_bytes: int
    accepted_resource_profile_id: str
    provider_deadline_unix_ms: int
    def __init__(self, protocol_major: _Optional[int] = ..., protocol_minor: _Optional[int] = ..., negotiated_feature_bits: _Optional[int] = ..., daemon_build: _Optional[str] = ..., output_schema_bundle_digest: _Optional[str] = ..., sandbox_profile_digest: _Optional[str] = ..., maximum_outstanding_chunks: _Optional[int] = ..., maximum_unacknowledged_bytes: _Optional[int] = ..., accepted_resource_profile_id: _Optional[str] = ..., provider_deadline_unix_ms: _Optional[int] = ...) -> None: ...

class PackageTargetIdentity(_message.Message):
    __slots__ = ("package_id", "package_name", "target_name", "target_kind", "crate_name", "crate_type", "crate_disambiguator")
    PACKAGE_ID_FIELD_NUMBER: _ClassVar[int]
    PACKAGE_NAME_FIELD_NUMBER: _ClassVar[int]
    TARGET_NAME_FIELD_NUMBER: _ClassVar[int]
    TARGET_KIND_FIELD_NUMBER: _ClassVar[int]
    CRATE_NAME_FIELD_NUMBER: _ClassVar[int]
    CRATE_TYPE_FIELD_NUMBER: _ClassVar[int]
    CRATE_DISAMBIGUATOR_FIELD_NUMBER: _ClassVar[int]
    package_id: str
    package_name: str
    target_name: str
    target_kind: str
    crate_name: str
    crate_type: str
    crate_disambiguator: str
    def __init__(self, package_id: _Optional[str] = ..., package_name: _Optional[str] = ..., target_name: _Optional[str] = ..., target_kind: _Optional[str] = ..., crate_name: _Optional[str] = ..., crate_type: _Optional[str] = ..., crate_disambiguator: _Optional[str] = ...) -> None: ...

class CompilationBegin(_message.Message):
    __slots__ = ("provider_run_id", "compilation_unit_id", "workspace_id", "analysis_context_id", "source_generation", "target", "rustc_version", "rustc_commit", "normalized_rustc_invocation_digest", "cargo_metadata_digest", "cargo_lock_digest", "cargo_config_digest", "build_script_output_digests", "proc_macro_output_digests", "source_snapshot_manifest_digest", "requested_capability_codes", "context_manifest_digest", "resource_profile_id", "toolchain_identity_digest")
    PROVIDER_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    COMPILATION_UNIT_ID_FIELD_NUMBER: _ClassVar[int]
    WORKSPACE_ID_FIELD_NUMBER: _ClassVar[int]
    ANALYSIS_CONTEXT_ID_FIELD_NUMBER: _ClassVar[int]
    SOURCE_GENERATION_FIELD_NUMBER: _ClassVar[int]
    TARGET_FIELD_NUMBER: _ClassVar[int]
    RUSTC_VERSION_FIELD_NUMBER: _ClassVar[int]
    RUSTC_COMMIT_FIELD_NUMBER: _ClassVar[int]
    NORMALIZED_RUSTC_INVOCATION_DIGEST_FIELD_NUMBER: _ClassVar[int]
    CARGO_METADATA_DIGEST_FIELD_NUMBER: _ClassVar[int]
    CARGO_LOCK_DIGEST_FIELD_NUMBER: _ClassVar[int]
    CARGO_CONFIG_DIGEST_FIELD_NUMBER: _ClassVar[int]
    BUILD_SCRIPT_OUTPUT_DIGESTS_FIELD_NUMBER: _ClassVar[int]
    PROC_MACRO_OUTPUT_DIGESTS_FIELD_NUMBER: _ClassVar[int]
    SOURCE_SNAPSHOT_MANIFEST_DIGEST_FIELD_NUMBER: _ClassVar[int]
    REQUESTED_CAPABILITY_CODES_FIELD_NUMBER: _ClassVar[int]
    CONTEXT_MANIFEST_DIGEST_FIELD_NUMBER: _ClassVar[int]
    RESOURCE_PROFILE_ID_FIELD_NUMBER: _ClassVar[int]
    TOOLCHAIN_IDENTITY_DIGEST_FIELD_NUMBER: _ClassVar[int]
    provider_run_id: str
    compilation_unit_id: str
    workspace_id: str
    analysis_context_id: str
    source_generation: int
    target: PackageTargetIdentity
    rustc_version: str
    rustc_commit: str
    normalized_rustc_invocation_digest: str
    cargo_metadata_digest: str
    cargo_lock_digest: str
    cargo_config_digest: str
    build_script_output_digests: _containers.RepeatedScalarFieldContainer[str]
    proc_macro_output_digests: _containers.RepeatedScalarFieldContainer[str]
    source_snapshot_manifest_digest: str
    requested_capability_codes: _containers.RepeatedScalarFieldContainer[int]
    context_manifest_digest: str
    resource_profile_id: str
    toolchain_identity_digest: str
    def __init__(self, provider_run_id: _Optional[str] = ..., compilation_unit_id: _Optional[str] = ..., workspace_id: _Optional[str] = ..., analysis_context_id: _Optional[str] = ..., source_generation: _Optional[int] = ..., target: _Optional[_Union[PackageTargetIdentity, _Mapping]] = ..., rustc_version: _Optional[str] = ..., rustc_commit: _Optional[str] = ..., normalized_rustc_invocation_digest: _Optional[str] = ..., cargo_metadata_digest: _Optional[str] = ..., cargo_lock_digest: _Optional[str] = ..., cargo_config_digest: _Optional[str] = ..., build_script_output_digests: _Optional[_Iterable[str]] = ..., proc_macro_output_digests: _Optional[_Iterable[str]] = ..., source_snapshot_manifest_digest: _Optional[str] = ..., requested_capability_codes: _Optional[_Iterable[int]] = ..., context_manifest_digest: _Optional[str] = ..., resource_profile_id: _Optional[str] = ..., toolchain_identity_digest: _Optional[str] = ...) -> None: ...

class CompilationAccepted(_message.Message):
    __slots__ = ("provider_run_id", "compilation_unit_id", "accepted_generation", "granted_chunk_credits", "granted_credit_bytes")
    PROVIDER_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    COMPILATION_UNIT_ID_FIELD_NUMBER: _ClassVar[int]
    ACCEPTED_GENERATION_FIELD_NUMBER: _ClassVar[int]
    GRANTED_CHUNK_CREDITS_FIELD_NUMBER: _ClassVar[int]
    GRANTED_CREDIT_BYTES_FIELD_NUMBER: _ClassVar[int]
    provider_run_id: str
    compilation_unit_id: str
    accepted_generation: int
    granted_chunk_credits: int
    granted_credit_bytes: int
    def __init__(self, provider_run_id: _Optional[str] = ..., compilation_unit_id: _Optional[str] = ..., accepted_generation: _Optional[int] = ..., granted_chunk_credits: _Optional[int] = ..., granted_credit_bytes: _Optional[int] = ...) -> None: ...

class CompilerOwnerKey(_message.Message):
    __slots__ = ("owner_id", "owner_kind", "file_id", "source_start", "source_end")
    OWNER_ID_FIELD_NUMBER: _ClassVar[int]
    OWNER_KIND_FIELD_NUMBER: _ClassVar[int]
    FILE_ID_FIELD_NUMBER: _ClassVar[int]
    SOURCE_START_FIELD_NUMBER: _ClassVar[int]
    SOURCE_END_FIELD_NUMBER: _ClassVar[int]
    owner_id: str
    owner_kind: str
    file_id: str
    source_start: int
    source_end: int
    def __init__(self, owner_id: _Optional[str] = ..., owner_kind: _Optional[str] = ..., file_id: _Optional[str] = ..., source_start: _Optional[int] = ..., source_end: _Optional[int] = ...) -> None: ...

class OwnerBegin(_message.Message):
    __slots__ = ("provider_run_id", "compilation_unit_id", "sequence", "owner", "expected_observation_family_codes")
    PROVIDER_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    COMPILATION_UNIT_ID_FIELD_NUMBER: _ClassVar[int]
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    OWNER_FIELD_NUMBER: _ClassVar[int]
    EXPECTED_OBSERVATION_FAMILY_CODES_FIELD_NUMBER: _ClassVar[int]
    provider_run_id: str
    compilation_unit_id: str
    sequence: int
    owner: CompilerOwnerKey
    expected_observation_family_codes: _containers.RepeatedScalarFieldContainer[int]
    def __init__(self, provider_run_id: _Optional[str] = ..., compilation_unit_id: _Optional[str] = ..., sequence: _Optional[int] = ..., owner: _Optional[_Union[CompilerOwnerKey, _Mapping]] = ..., expected_observation_family_codes: _Optional[_Iterable[int]] = ...) -> None: ...

class OwnerObservationChunk(_message.Message):
    __slots__ = ("provider_run_id", "compilation_unit_id", "sequence", "owner_id", "observation_family_code", "arrow_ipc", "payload_reference", "schema_digest", "row_count", "chunk_digest")
    PROVIDER_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    COMPILATION_UNIT_ID_FIELD_NUMBER: _ClassVar[int]
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    OWNER_ID_FIELD_NUMBER: _ClassVar[int]
    OBSERVATION_FAMILY_CODE_FIELD_NUMBER: _ClassVar[int]
    ARROW_IPC_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_REFERENCE_FIELD_NUMBER: _ClassVar[int]
    SCHEMA_DIGEST_FIELD_NUMBER: _ClassVar[int]
    ROW_COUNT_FIELD_NUMBER: _ClassVar[int]
    CHUNK_DIGEST_FIELD_NUMBER: _ClassVar[int]
    provider_run_id: str
    compilation_unit_id: str
    sequence: int
    owner_id: str
    observation_family_code: int
    arrow_ipc: bytes
    payload_reference: _provider_control_pb2.BlobReference
    schema_digest: str
    row_count: int
    chunk_digest: str
    def __init__(self, provider_run_id: _Optional[str] = ..., compilation_unit_id: _Optional[str] = ..., sequence: _Optional[int] = ..., owner_id: _Optional[str] = ..., observation_family_code: _Optional[int] = ..., arrow_ipc: _Optional[bytes] = ..., payload_reference: _Optional[_Union[_provider_control_pb2.BlobReference, _Mapping]] = ..., schema_digest: _Optional[str] = ..., row_count: _Optional[int] = ..., chunk_digest: _Optional[str] = ...) -> None: ...

class OwnerEnd(_message.Message):
    __slots__ = ("provider_run_id", "compilation_unit_id", "sequence", "owner_id", "family_counts", "owner_content_digest")
    class FamilyCountsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: int
        value: int
        def __init__(self, key: _Optional[int] = ..., value: _Optional[int] = ...) -> None: ...
    PROVIDER_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    COMPILATION_UNIT_ID_FIELD_NUMBER: _ClassVar[int]
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    OWNER_ID_FIELD_NUMBER: _ClassVar[int]
    FAMILY_COUNTS_FIELD_NUMBER: _ClassVar[int]
    OWNER_CONTENT_DIGEST_FIELD_NUMBER: _ClassVar[int]
    provider_run_id: str
    compilation_unit_id: str
    sequence: int
    owner_id: str
    family_counts: _containers.ScalarMap[int, int]
    owner_content_digest: str
    def __init__(self, provider_run_id: _Optional[str] = ..., compilation_unit_id: _Optional[str] = ..., sequence: _Optional[int] = ..., owner_id: _Optional[str] = ..., family_counts: _Optional[_Mapping[int, int]] = ..., owner_content_digest: _Optional[str] = ...) -> None: ...

class DiagnosticSummary(_message.Message):
    __slots__ = ("error_count", "warning_count", "diagnostics_digest")
    ERROR_COUNT_FIELD_NUMBER: _ClassVar[int]
    WARNING_COUNT_FIELD_NUMBER: _ClassVar[int]
    DIAGNOSTICS_DIGEST_FIELD_NUMBER: _ClassVar[int]
    error_count: int
    warning_count: int
    diagnostics_digest: str
    def __init__(self, error_count: _Optional[int] = ..., warning_count: _Optional[int] = ..., diagnostics_digest: _Optional[str] = ...) -> None: ...

class CompilationEnd(_message.Message):
    __slots__ = ("provider_run_id", "compilation_unit_id", "sequence", "compiler_exit_status", "closed_owner_set_digest", "capability_outcomes", "diagnostic_summary", "overall_stream_digest", "terminal_state", "rejection_error")
    PROVIDER_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    COMPILATION_UNIT_ID_FIELD_NUMBER: _ClassVar[int]
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    COMPILER_EXIT_STATUS_FIELD_NUMBER: _ClassVar[int]
    CLOSED_OWNER_SET_DIGEST_FIELD_NUMBER: _ClassVar[int]
    CAPABILITY_OUTCOMES_FIELD_NUMBER: _ClassVar[int]
    DIAGNOSTIC_SUMMARY_FIELD_NUMBER: _ClassVar[int]
    OVERALL_STREAM_DIGEST_FIELD_NUMBER: _ClassVar[int]
    TERMINAL_STATE_FIELD_NUMBER: _ClassVar[int]
    REJECTION_ERROR_FIELD_NUMBER: _ClassVar[int]
    provider_run_id: str
    compilation_unit_id: str
    sequence: int
    compiler_exit_status: int
    closed_owner_set_digest: str
    capability_outcomes: _containers.RepeatedCompositeFieldContainer[_provider_control_pb2.CapabilityOutcome]
    diagnostic_summary: DiagnosticSummary
    overall_stream_digest: str
    terminal_state: _provider_control_pb2.ProviderRunState
    rejection_error: RejectionRuleErrorCode
    def __init__(self, provider_run_id: _Optional[str] = ..., compilation_unit_id: _Optional[str] = ..., sequence: _Optional[int] = ..., compiler_exit_status: _Optional[int] = ..., closed_owner_set_digest: _Optional[str] = ..., capability_outcomes: _Optional[_Iterable[_Union[_provider_control_pb2.CapabilityOutcome, _Mapping]]] = ..., diagnostic_summary: _Optional[_Union[DiagnosticSummary, _Mapping]] = ..., overall_stream_digest: _Optional[str] = ..., terminal_state: _Optional[_Union[_provider_control_pb2.ProviderRunState, str]] = ..., rejection_error: _Optional[_Union[RejectionRuleErrorCode, str]] = ...) -> None: ...

class ExtractorCommand(_message.Message):
    __slots__ = ("compilation_accepted", "chunk_accepted", "chunk_rejected", "cancel", "relation_ipc_ack")
    COMPILATION_ACCEPTED_FIELD_NUMBER: _ClassVar[int]
    CHUNK_ACCEPTED_FIELD_NUMBER: _ClassVar[int]
    CHUNK_REJECTED_FIELD_NUMBER: _ClassVar[int]
    CANCEL_FIELD_NUMBER: _ClassVar[int]
    RELATION_IPC_ACK_FIELD_NUMBER: _ClassVar[int]
    compilation_accepted: CompilationAccepted
    chunk_accepted: _provider_control_pb2.ChunkAccepted
    chunk_rejected: _provider_control_pb2.ChunkRejected
    cancel: CancelCompilationRequest
    relation_ipc_ack: _provider_control_pb2.RelationIpcFrame
    def __init__(self, compilation_accepted: _Optional[_Union[CompilationAccepted, _Mapping]] = ..., chunk_accepted: _Optional[_Union[_provider_control_pb2.ChunkAccepted, _Mapping]] = ..., chunk_rejected: _Optional[_Union[_provider_control_pb2.ChunkRejected, _Mapping]] = ..., cancel: _Optional[_Union[CancelCompilationRequest, _Mapping]] = ..., relation_ipc_ack: _Optional[_Union[_provider_control_pb2.RelationIpcFrame, _Mapping]] = ...) -> None: ...

class OwnerRelationIpcFrame(_message.Message):
    __slots__ = ("provider_run_id", "compilation_unit_id", "sequence", "owner_id", "observation_family_code", "frame")
    PROVIDER_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    COMPILATION_UNIT_ID_FIELD_NUMBER: _ClassVar[int]
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    OWNER_ID_FIELD_NUMBER: _ClassVar[int]
    OBSERVATION_FAMILY_CODE_FIELD_NUMBER: _ClassVar[int]
    FRAME_FIELD_NUMBER: _ClassVar[int]
    provider_run_id: str
    compilation_unit_id: str
    sequence: int
    owner_id: str
    observation_family_code: int
    frame: _provider_control_pb2.RelationIpcFrame
    def __init__(self, provider_run_id: _Optional[str] = ..., compilation_unit_id: _Optional[str] = ..., sequence: _Optional[int] = ..., owner_id: _Optional[str] = ..., observation_family_code: _Optional[int] = ..., frame: _Optional[_Union[_provider_control_pb2.RelationIpcFrame, _Mapping]] = ...) -> None: ...

class ExtractionEvent(_message.Message):
    __slots__ = ("compilation_begin", "owner_begin", "owner_observation_chunk", "owner_end", "compilation_end", "owner_relation_ipc_frame")
    COMPILATION_BEGIN_FIELD_NUMBER: _ClassVar[int]
    OWNER_BEGIN_FIELD_NUMBER: _ClassVar[int]
    OWNER_OBSERVATION_CHUNK_FIELD_NUMBER: _ClassVar[int]
    OWNER_END_FIELD_NUMBER: _ClassVar[int]
    COMPILATION_END_FIELD_NUMBER: _ClassVar[int]
    OWNER_RELATION_IPC_FRAME_FIELD_NUMBER: _ClassVar[int]
    compilation_begin: CompilationBegin
    owner_begin: OwnerBegin
    owner_observation_chunk: OwnerObservationChunk
    owner_end: OwnerEnd
    compilation_end: CompilationEnd
    owner_relation_ipc_frame: OwnerRelationIpcFrame
    def __init__(self, compilation_begin: _Optional[_Union[CompilationBegin, _Mapping]] = ..., owner_begin: _Optional[_Union[OwnerBegin, _Mapping]] = ..., owner_observation_chunk: _Optional[_Union[OwnerObservationChunk, _Mapping]] = ..., owner_end: _Optional[_Union[OwnerEnd, _Mapping]] = ..., compilation_end: _Optional[_Union[CompilationEnd, _Mapping]] = ..., owner_relation_ipc_frame: _Optional[_Union[OwnerRelationIpcFrame, _Mapping]] = ...) -> None: ...

class CancelCompilationRequest(_message.Message):
    __slots__ = ("provider_run_id", "compilation_unit_id", "reason")
    PROVIDER_RUN_ID_FIELD_NUMBER: _ClassVar[int]
    COMPILATION_UNIT_ID_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    provider_run_id: str
    compilation_unit_id: str
    reason: str
    def __init__(self, provider_run_id: _Optional[str] = ..., compilation_unit_id: _Optional[str] = ..., reason: _Optional[str] = ...) -> None: ...
