# @generated from catalog primary semantic identity b3:502dfd819e70a154db899bd6bdbe580d01bb56f1654790d5adb241199d43b434,b3:71fb94283214d79068ede88e0f45e1460336b23b9678f80b4ddbece098cd626f,b3:d5b256baca150eed2617f78f88362c607ff12db7a94af9524658a3c82f247973,b3:2f2c24a2877be95dfd1d3acc7d83354838696af2aaac13c99bde83ab743f6c62; do not edit.
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class FreshnessPolicy(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    FRESHNESS_POLICY_UNSPECIFIED: _ClassVar[FreshnessPolicy]
    FRESHNESS_POLICY_BEST_AVAILABLE_SNAPSHOT: _ClassVar[FreshnessPolicy]
    FRESHNESS_POLICY_AWAIT_LATEST: _ClassVar[FreshnessPolicy]
    FRESHNESS_POLICY_REQUIRE_CURRENT_FOR_TARGETS: _ClassVar[FreshnessPolicy]
    FRESHNESS_POLICY_REQUIRE_SOURCE_CURRENT: _ClassVar[FreshnessPolicy]
    FRESHNESS_POLICY_REQUIRE_SEMANTIC_CURRENT: _ClassVar[FreshnessPolicy]

class PayloadCompression(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    PAYLOAD_COMPRESSION_UNSPECIFIED: _ClassVar[PayloadCompression]
    PAYLOAD_COMPRESSION_IDENTITY: _ClassVar[PayloadCompression]
    PAYLOAD_COMPRESSION_ZSTD: _ClassVar[PayloadCompression]

class DeliveryPreference(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    DELIVERY_PREFERENCE_UNSPECIFIED: _ClassVar[DeliveryPreference]
    DELIVERY_PREFERENCE_INLINE: _ClassVar[DeliveryPreference]
    DELIVERY_PREFERENCE_RESOURCE: _ClassVar[DeliveryPreference]
    DELIVERY_PREFERENCE_AUTO: _ClassVar[DeliveryPreference]

class QueryExecutionState(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    QUERY_EXECUTION_STATE_UNSPECIFIED: _ClassVar[QueryExecutionState]
    QUERY_EXECUTION_STATE_ACCEPTED: _ClassVar[QueryExecutionState]
    QUERY_EXECUTION_STATE_WAITING_FOR_FRESHNESS: _ClassVar[QueryExecutionState]
    QUERY_EXECUTION_STATE_RUNNING: _ClassVar[QueryExecutionState]
    QUERY_EXECUTION_STATE_SUCCEEDED: _ClassVar[QueryExecutionState]
    QUERY_EXECUTION_STATE_FAILED: _ClassVar[QueryExecutionState]
    QUERY_EXECUTION_STATE_CANCELLED: _ClassVar[QueryExecutionState]
    QUERY_EXECUTION_STATE_LOST: _ClassVar[QueryExecutionState]

class CancellationState(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CANCELLATION_STATE_UNSPECIFIED: _ClassVar[CancellationState]
    CANCELLATION_STATE_NOT_FOUND: _ClassVar[CancellationState]
    CANCELLATION_STATE_CANCELLATION_REQUESTED: _ClassVar[CancellationState]
    CANCELLATION_STATE_CANCELLED: _ClassVar[CancellationState]
    CANCELLATION_STATE_ALREADY_TERMINAL: _ClassVar[CancellationState]
    CANCELLATION_STATE_FORCE_TERMINATED: _ClassVar[CancellationState]

class WorkspaceReadiness(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    WORKSPACE_READINESS_UNSPECIFIED: _ClassVar[WorkspaceReadiness]
    WORKSPACE_READINESS_BOOTSTRAPPING: _ClassVar[WorkspaceReadiness]
    WORKSPACE_READINESS_READY: _ClassVar[WorkspaceReadiness]
    WORKSPACE_READINESS_DEGRADED: _ClassVar[WorkspaceReadiness]
    WORKSPACE_READINESS_FAILED: _ClassVar[WorkspaceReadiness]
FRESHNESS_POLICY_UNSPECIFIED: FreshnessPolicy
FRESHNESS_POLICY_BEST_AVAILABLE_SNAPSHOT: FreshnessPolicy
FRESHNESS_POLICY_AWAIT_LATEST: FreshnessPolicy
FRESHNESS_POLICY_REQUIRE_CURRENT_FOR_TARGETS: FreshnessPolicy
FRESHNESS_POLICY_REQUIRE_SOURCE_CURRENT: FreshnessPolicy
FRESHNESS_POLICY_REQUIRE_SEMANTIC_CURRENT: FreshnessPolicy
PAYLOAD_COMPRESSION_UNSPECIFIED: PayloadCompression
PAYLOAD_COMPRESSION_IDENTITY: PayloadCompression
PAYLOAD_COMPRESSION_ZSTD: PayloadCompression
DELIVERY_PREFERENCE_UNSPECIFIED: DeliveryPreference
DELIVERY_PREFERENCE_INLINE: DeliveryPreference
DELIVERY_PREFERENCE_RESOURCE: DeliveryPreference
DELIVERY_PREFERENCE_AUTO: DeliveryPreference
QUERY_EXECUTION_STATE_UNSPECIFIED: QueryExecutionState
QUERY_EXECUTION_STATE_ACCEPTED: QueryExecutionState
QUERY_EXECUTION_STATE_WAITING_FOR_FRESHNESS: QueryExecutionState
QUERY_EXECUTION_STATE_RUNNING: QueryExecutionState
QUERY_EXECUTION_STATE_SUCCEEDED: QueryExecutionState
QUERY_EXECUTION_STATE_FAILED: QueryExecutionState
QUERY_EXECUTION_STATE_CANCELLED: QueryExecutionState
QUERY_EXECUTION_STATE_LOST: QueryExecutionState
CANCELLATION_STATE_UNSPECIFIED: CancellationState
CANCELLATION_STATE_NOT_FOUND: CancellationState
CANCELLATION_STATE_CANCELLATION_REQUESTED: CancellationState
CANCELLATION_STATE_CANCELLED: CancellationState
CANCELLATION_STATE_ALREADY_TERMINAL: CancellationState
CANCELLATION_STATE_FORCE_TERMINATED: CancellationState
WORKSPACE_READINESS_UNSPECIFIED: WorkspaceReadiness
WORKSPACE_READINESS_BOOTSTRAPPING: WorkspaceReadiness
WORKSPACE_READINESS_READY: WorkspaceReadiness
WORKSPACE_READINESS_DEGRADED: WorkspaceReadiness
WORKSPACE_READINESS_FAILED: WorkspaceReadiness

class VersionRange(_message.Message):
    __slots__ = ("minimum", "maximum")
    MINIMUM_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_FIELD_NUMBER: _ClassVar[int]
    minimum: str
    maximum: str
    def __init__(self, minimum: _Optional[str] = ..., maximum: _Optional[str] = ...) -> None: ...

class SchemaFingerprint(_message.Message):
    __slots__ = ("schema_id", "version", "digest")
    SCHEMA_ID_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    DIGEST_FIELD_NUMBER: _ClassVar[int]
    schema_id: str
    version: str
    digest: str
    def __init__(self, schema_id: _Optional[str] = ..., version: _Optional[str] = ..., digest: _Optional[str] = ...) -> None: ...

class HostCapabilityProfile(_message.Message):
    __slots__ = ("delivery_modes", "compression_algorithms", "supports_resource_links", "supports_trace_context", "maximum_frame_bytes", "profile_digest")
    DELIVERY_MODES_FIELD_NUMBER: _ClassVar[int]
    COMPRESSION_ALGORITHMS_FIELD_NUMBER: _ClassVar[int]
    SUPPORTS_RESOURCE_LINKS_FIELD_NUMBER: _ClassVar[int]
    SUPPORTS_TRACE_CONTEXT_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_FRAME_BYTES_FIELD_NUMBER: _ClassVar[int]
    PROFILE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    delivery_modes: _containers.RepeatedScalarFieldContainer[DeliveryPreference]
    compression_algorithms: _containers.RepeatedScalarFieldContainer[PayloadCompression]
    supports_resource_links: bool
    supports_trace_context: bool
    maximum_frame_bytes: int
    profile_digest: str
    def __init__(self, delivery_modes: _Optional[_Iterable[_Union[DeliveryPreference, str]]] = ..., compression_algorithms: _Optional[_Iterable[_Union[PayloadCompression, str]]] = ..., supports_resource_links: _Optional[bool] = ..., supports_trace_context: _Optional[bool] = ..., maximum_frame_bytes: _Optional[int] = ..., profile_digest: _Optional[str] = ...) -> None: ...

class CredentialProof(_message.Message):
    __slots__ = ("credential_id", "capability_token")
    CREDENTIAL_ID_FIELD_NUMBER: _ClassVar[int]
    CAPABILITY_TOKEN_FIELD_NUMBER: _ClassVar[int]
    credential_id: str
    capability_token: bytes
    def __init__(self, credential_id: _Optional[str] = ..., capability_token: _Optional[bytes] = ...) -> None: ...

class HandshakeRequest(_message.Message):
    __slots__ = ("adapter_instance_id", "adapter_version", "fastmcp_version", "pydantic_version", "python_version", "rpc_versions", "semantic_query_versions", "schema_fingerprints", "required_feature_bits", "optional_feature_bits", "desired_workspace_ids", "host_capabilities", "credential_proof", "agent_instance_id")
    ADAPTER_INSTANCE_ID_FIELD_NUMBER: _ClassVar[int]
    ADAPTER_VERSION_FIELD_NUMBER: _ClassVar[int]
    FASTMCP_VERSION_FIELD_NUMBER: _ClassVar[int]
    PYDANTIC_VERSION_FIELD_NUMBER: _ClassVar[int]
    PYTHON_VERSION_FIELD_NUMBER: _ClassVar[int]
    RPC_VERSIONS_FIELD_NUMBER: _ClassVar[int]
    SEMANTIC_QUERY_VERSIONS_FIELD_NUMBER: _ClassVar[int]
    SCHEMA_FINGERPRINTS_FIELD_NUMBER: _ClassVar[int]
    REQUIRED_FEATURE_BITS_FIELD_NUMBER: _ClassVar[int]
    OPTIONAL_FEATURE_BITS_FIELD_NUMBER: _ClassVar[int]
    DESIRED_WORKSPACE_IDS_FIELD_NUMBER: _ClassVar[int]
    HOST_CAPABILITIES_FIELD_NUMBER: _ClassVar[int]
    CREDENTIAL_PROOF_FIELD_NUMBER: _ClassVar[int]
    AGENT_INSTANCE_ID_FIELD_NUMBER: _ClassVar[int]
    adapter_instance_id: str
    adapter_version: str
    fastmcp_version: str
    pydantic_version: str
    python_version: str
    rpc_versions: VersionRange
    semantic_query_versions: VersionRange
    schema_fingerprints: _containers.RepeatedCompositeFieldContainer[SchemaFingerprint]
    required_feature_bits: int
    optional_feature_bits: int
    desired_workspace_ids: _containers.RepeatedScalarFieldContainer[str]
    host_capabilities: HostCapabilityProfile
    credential_proof: CredentialProof
    agent_instance_id: str
    def __init__(self, adapter_instance_id: _Optional[str] = ..., adapter_version: _Optional[str] = ..., fastmcp_version: _Optional[str] = ..., pydantic_version: _Optional[str] = ..., python_version: _Optional[str] = ..., rpc_versions: _Optional[_Union[VersionRange, _Mapping]] = ..., semantic_query_versions: _Optional[_Union[VersionRange, _Mapping]] = ..., schema_fingerprints: _Optional[_Iterable[_Union[SchemaFingerprint, _Mapping]]] = ..., required_feature_bits: _Optional[int] = ..., optional_feature_bits: _Optional[int] = ..., desired_workspace_ids: _Optional[_Iterable[str]] = ..., host_capabilities: _Optional[_Union[HostCapabilityProfile, _Mapping]] = ..., credential_proof: _Optional[_Union[CredentialProof, _Mapping]] = ..., agent_instance_id: _Optional[str] = ...) -> None: ...

class BundleIdentity(_message.Message):
    __slots__ = ("bundle_id", "bundle_version", "bundle_digest")
    BUNDLE_ID_FIELD_NUMBER: _ClassVar[int]
    BUNDLE_VERSION_FIELD_NUMBER: _ClassVar[int]
    BUNDLE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    bundle_id: str
    bundle_version: str
    bundle_digest: str
    def __init__(self, bundle_id: _Optional[str] = ..., bundle_version: _Optional[str] = ..., bundle_digest: _Optional[str] = ...) -> None: ...

class WorkspaceClaim(_message.Message):
    __slots__ = ("workspace_id", "repository_id", "worktree_id", "workspace_kind", "readiness", "permission_claims")
    WORKSPACE_ID_FIELD_NUMBER: _ClassVar[int]
    REPOSITORY_ID_FIELD_NUMBER: _ClassVar[int]
    WORKTREE_ID_FIELD_NUMBER: _ClassVar[int]
    WORKSPACE_KIND_FIELD_NUMBER: _ClassVar[int]
    READINESS_FIELD_NUMBER: _ClassVar[int]
    PERMISSION_CLAIMS_FIELD_NUMBER: _ClassVar[int]
    workspace_id: str
    repository_id: str
    worktree_id: str
    workspace_kind: str
    readiness: WorkspaceReadiness
    permission_claims: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, workspace_id: _Optional[str] = ..., repository_id: _Optional[str] = ..., worktree_id: _Optional[str] = ..., workspace_kind: _Optional[str] = ..., readiness: _Optional[_Union[WorkspaceReadiness, str]] = ..., permission_claims: _Optional[_Iterable[str]] = ...) -> None: ...

class EffectiveLimitsProfile(_message.Message):
    __slots__ = ("maximum_control_message_bytes", "maximum_payload_chunk_bytes", "maximum_inline_response_bytes", "maximum_concurrent_queries", "query_orphan_replay_seconds", "profile_digest")
    MAXIMUM_CONTROL_MESSAGE_BYTES_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_PAYLOAD_CHUNK_BYTES_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_INLINE_RESPONSE_BYTES_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_CONCURRENT_QUERIES_FIELD_NUMBER: _ClassVar[int]
    QUERY_ORPHAN_REPLAY_SECONDS_FIELD_NUMBER: _ClassVar[int]
    PROFILE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    maximum_control_message_bytes: int
    maximum_payload_chunk_bytes: int
    maximum_inline_response_bytes: int
    maximum_concurrent_queries: int
    query_orphan_replay_seconds: int
    profile_digest: str
    def __init__(self, maximum_control_message_bytes: _Optional[int] = ..., maximum_payload_chunk_bytes: _Optional[int] = ..., maximum_inline_response_bytes: _Optional[int] = ..., maximum_concurrent_queries: _Optional[int] = ..., query_orphan_replay_seconds: _Optional[int] = ..., profile_digest: _Optional[str] = ...) -> None: ...

class ReadinessSummary(_message.Message):
    __slots__ = ("readiness", "reason_code", "active_snapshot_id", "supported_language_codes", "supported_query_forms", "capability_codes")
    READINESS_FIELD_NUMBER: _ClassVar[int]
    REASON_CODE_FIELD_NUMBER: _ClassVar[int]
    ACTIVE_SNAPSHOT_ID_FIELD_NUMBER: _ClassVar[int]
    SUPPORTED_LANGUAGE_CODES_FIELD_NUMBER: _ClassVar[int]
    SUPPORTED_QUERY_FORMS_FIELD_NUMBER: _ClassVar[int]
    CAPABILITY_CODES_FIELD_NUMBER: _ClassVar[int]
    readiness: WorkspaceReadiness
    reason_code: str
    active_snapshot_id: str
    supported_language_codes: _containers.RepeatedScalarFieldContainer[int]
    supported_query_forms: _containers.RepeatedScalarFieldContainer[str]
    capability_codes: _containers.RepeatedScalarFieldContainer[int]
    def __init__(self, readiness: _Optional[_Union[WorkspaceReadiness, str]] = ..., reason_code: _Optional[str] = ..., active_snapshot_id: _Optional[str] = ..., supported_language_codes: _Optional[_Iterable[int]] = ..., supported_query_forms: _Optional[_Iterable[str]] = ..., capability_codes: _Optional[_Iterable[int]] = ...) -> None: ...

class HandshakeResponse(_message.Message):
    __slots__ = ("daemon_instance_id", "daemon_version", "rust_build", "negotiated_rpc_version", "negotiated_semantic_query_version", "negotiated_feature_bits", "negotiated_compression", "installed_bundles", "active_schema_fingerprints", "effective_limits", "authorized_workspaces", "server_time_unix_ms", "readiness")
    DAEMON_INSTANCE_ID_FIELD_NUMBER: _ClassVar[int]
    DAEMON_VERSION_FIELD_NUMBER: _ClassVar[int]
    RUST_BUILD_FIELD_NUMBER: _ClassVar[int]
    NEGOTIATED_RPC_VERSION_FIELD_NUMBER: _ClassVar[int]
    NEGOTIATED_SEMANTIC_QUERY_VERSION_FIELD_NUMBER: _ClassVar[int]
    NEGOTIATED_FEATURE_BITS_FIELD_NUMBER: _ClassVar[int]
    NEGOTIATED_COMPRESSION_FIELD_NUMBER: _ClassVar[int]
    INSTALLED_BUNDLES_FIELD_NUMBER: _ClassVar[int]
    ACTIVE_SCHEMA_FINGERPRINTS_FIELD_NUMBER: _ClassVar[int]
    EFFECTIVE_LIMITS_FIELD_NUMBER: _ClassVar[int]
    AUTHORIZED_WORKSPACES_FIELD_NUMBER: _ClassVar[int]
    SERVER_TIME_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    READINESS_FIELD_NUMBER: _ClassVar[int]
    daemon_instance_id: str
    daemon_version: str
    rust_build: str
    negotiated_rpc_version: str
    negotiated_semantic_query_version: str
    negotiated_feature_bits: int
    negotiated_compression: PayloadCompression
    installed_bundles: _containers.RepeatedCompositeFieldContainer[BundleIdentity]
    active_schema_fingerprints: _containers.RepeatedCompositeFieldContainer[SchemaFingerprint]
    effective_limits: EffectiveLimitsProfile
    authorized_workspaces: _containers.RepeatedCompositeFieldContainer[WorkspaceClaim]
    server_time_unix_ms: int
    readiness: ReadinessSummary
    def __init__(self, daemon_instance_id: _Optional[str] = ..., daemon_version: _Optional[str] = ..., rust_build: _Optional[str] = ..., negotiated_rpc_version: _Optional[str] = ..., negotiated_semantic_query_version: _Optional[str] = ..., negotiated_feature_bits: _Optional[int] = ..., negotiated_compression: _Optional[_Union[PayloadCompression, str]] = ..., installed_bundles: _Optional[_Iterable[_Union[BundleIdentity, _Mapping]]] = ..., active_schema_fingerprints: _Optional[_Iterable[_Union[SchemaFingerprint, _Mapping]]] = ..., effective_limits: _Optional[_Union[EffectiveLimitsProfile, _Mapping]] = ..., authorized_workspaces: _Optional[_Iterable[_Union[WorkspaceClaim, _Mapping]]] = ..., server_time_unix_ms: _Optional[int] = ..., readiness: _Optional[_Union[ReadinessSummary, _Mapping]] = ...) -> None: ...

class StatusRequest(_message.Message):
    __slots__ = ("agent_instance_id", "workspace_id", "include_diagnostics")
    AGENT_INSTANCE_ID_FIELD_NUMBER: _ClassVar[int]
    WORKSPACE_ID_FIELD_NUMBER: _ClassVar[int]
    INCLUDE_DIAGNOSTICS_FIELD_NUMBER: _ClassVar[int]
    agent_instance_id: str
    workspace_id: str
    include_diagnostics: bool
    def __init__(self, agent_instance_id: _Optional[str] = ..., workspace_id: _Optional[str] = ..., include_diagnostics: _Optional[bool] = ...) -> None: ...

class StatusResponse(_message.Message):
    __slots__ = ("workspace_id", "readiness", "canonical_public_status_json", "status_checksum", "observed_at_unix_ms")
    WORKSPACE_ID_FIELD_NUMBER: _ClassVar[int]
    READINESS_FIELD_NUMBER: _ClassVar[int]
    CANONICAL_PUBLIC_STATUS_JSON_FIELD_NUMBER: _ClassVar[int]
    STATUS_CHECKSUM_FIELD_NUMBER: _ClassVar[int]
    OBSERVED_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    workspace_id: str
    readiness: WorkspaceReadiness
    canonical_public_status_json: bytes
    status_checksum: str
    observed_at_unix_ms: int
    def __init__(self, workspace_id: _Optional[str] = ..., readiness: _Optional[_Union[WorkspaceReadiness, str]] = ..., canonical_public_status_json: _Optional[bytes] = ..., status_checksum: _Optional[str] = ..., observed_at_unix_ms: _Optional[int] = ...) -> None: ...

class ValidateQueryRequest(_message.Message):
    __slots__ = ("agent_instance_id", "workspace_id", "semantic_query_version", "canonical_request_json", "request_checksum", "freshness_policy", "host_capability_profile_digest")
    AGENT_INSTANCE_ID_FIELD_NUMBER: _ClassVar[int]
    WORKSPACE_ID_FIELD_NUMBER: _ClassVar[int]
    SEMANTIC_QUERY_VERSION_FIELD_NUMBER: _ClassVar[int]
    CANONICAL_REQUEST_JSON_FIELD_NUMBER: _ClassVar[int]
    REQUEST_CHECKSUM_FIELD_NUMBER: _ClassVar[int]
    FRESHNESS_POLICY_FIELD_NUMBER: _ClassVar[int]
    HOST_CAPABILITY_PROFILE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    agent_instance_id: str
    workspace_id: str
    semantic_query_version: str
    canonical_request_json: bytes
    request_checksum: str
    freshness_policy: FreshnessPolicy
    host_capability_profile_digest: str
    def __init__(self, agent_instance_id: _Optional[str] = ..., workspace_id: _Optional[str] = ..., semantic_query_version: _Optional[str] = ..., canonical_request_json: _Optional[bytes] = ..., request_checksum: _Optional[str] = ..., freshness_policy: _Optional[_Union[FreshnessPolicy, str]] = ..., host_capability_profile_digest: _Optional[str] = ...) -> None: ...

class ValidateQueryResponse(_message.Message):
    __slots__ = ("valid", "canonical_normalized_request_json", "normalized_request_checksum", "effective_semantic_request_id", "provisional_snapshot_checks", "canonical_error_records_json", "cost_class")
    VALID_FIELD_NUMBER: _ClassVar[int]
    CANONICAL_NORMALIZED_REQUEST_JSON_FIELD_NUMBER: _ClassVar[int]
    NORMALIZED_REQUEST_CHECKSUM_FIELD_NUMBER: _ClassVar[int]
    EFFECTIVE_SEMANTIC_REQUEST_ID_FIELD_NUMBER: _ClassVar[int]
    PROVISIONAL_SNAPSHOT_CHECKS_FIELD_NUMBER: _ClassVar[int]
    CANONICAL_ERROR_RECORDS_JSON_FIELD_NUMBER: _ClassVar[int]
    COST_CLASS_FIELD_NUMBER: _ClassVar[int]
    valid: bool
    canonical_normalized_request_json: bytes
    normalized_request_checksum: str
    effective_semantic_request_id: str
    provisional_snapshot_checks: _containers.RepeatedScalarFieldContainer[str]
    canonical_error_records_json: _containers.RepeatedScalarFieldContainer[bytes]
    cost_class: str
    def __init__(self, valid: _Optional[bool] = ..., canonical_normalized_request_json: _Optional[bytes] = ..., normalized_request_checksum: _Optional[str] = ..., effective_semantic_request_id: _Optional[str] = ..., provisional_snapshot_checks: _Optional[_Iterable[str]] = ..., canonical_error_records_json: _Optional[_Iterable[bytes]] = ..., cost_class: _Optional[str] = ...) -> None: ...

class StartQueryRequest(_message.Message):
    __slots__ = ("agent_instance_id", "workspace_id", "mcp_call_id", "rpc_attempt_id", "semantic_request_id", "semantic_query_version", "canonical_request_json", "request_checksum", "freshness_policy", "delivery_preference", "host_capability_profile_digest", "deadline_unix_ms", "idempotency_key", "payload_compression")
    AGENT_INSTANCE_ID_FIELD_NUMBER: _ClassVar[int]
    WORKSPACE_ID_FIELD_NUMBER: _ClassVar[int]
    MCP_CALL_ID_FIELD_NUMBER: _ClassVar[int]
    RPC_ATTEMPT_ID_FIELD_NUMBER: _ClassVar[int]
    SEMANTIC_REQUEST_ID_FIELD_NUMBER: _ClassVar[int]
    SEMANTIC_QUERY_VERSION_FIELD_NUMBER: _ClassVar[int]
    CANONICAL_REQUEST_JSON_FIELD_NUMBER: _ClassVar[int]
    REQUEST_CHECKSUM_FIELD_NUMBER: _ClassVar[int]
    FRESHNESS_POLICY_FIELD_NUMBER: _ClassVar[int]
    DELIVERY_PREFERENCE_FIELD_NUMBER: _ClassVar[int]
    HOST_CAPABILITY_PROFILE_DIGEST_FIELD_NUMBER: _ClassVar[int]
    DEADLINE_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    IDEMPOTENCY_KEY_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_COMPRESSION_FIELD_NUMBER: _ClassVar[int]
    agent_instance_id: str
    workspace_id: str
    mcp_call_id: str
    rpc_attempt_id: str
    semantic_request_id: str
    semantic_query_version: str
    canonical_request_json: bytes
    request_checksum: str
    freshness_policy: FreshnessPolicy
    delivery_preference: DeliveryPreference
    host_capability_profile_digest: str
    deadline_unix_ms: int
    idempotency_key: str
    payload_compression: PayloadCompression
    def __init__(self, agent_instance_id: _Optional[str] = ..., workspace_id: _Optional[str] = ..., mcp_call_id: _Optional[str] = ..., rpc_attempt_id: _Optional[str] = ..., semantic_request_id: _Optional[str] = ..., semantic_query_version: _Optional[str] = ..., canonical_request_json: _Optional[bytes] = ..., request_checksum: _Optional[str] = ..., freshness_policy: _Optional[_Union[FreshnessPolicy, str]] = ..., delivery_preference: _Optional[_Union[DeliveryPreference, str]] = ..., host_capability_profile_digest: _Optional[str] = ..., deadline_unix_ms: _Optional[int] = ..., idempotency_key: _Optional[str] = ..., payload_compression: _Optional[_Union[PayloadCompression, str]] = ...) -> None: ...

class StartQueryResponse(_message.Message):
    __slots__ = ("daemon_query_id", "resume_token", "accepted_at_unix_ms", "query_execution_state", "queue_class", "queue_position", "negotiated_request_version", "negotiated_response_version", "effective_semantic_request_id", "cancel_token")
    DAEMON_QUERY_ID_FIELD_NUMBER: _ClassVar[int]
    RESUME_TOKEN_FIELD_NUMBER: _ClassVar[int]
    ACCEPTED_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    QUERY_EXECUTION_STATE_FIELD_NUMBER: _ClassVar[int]
    QUEUE_CLASS_FIELD_NUMBER: _ClassVar[int]
    QUEUE_POSITION_FIELD_NUMBER: _ClassVar[int]
    NEGOTIATED_REQUEST_VERSION_FIELD_NUMBER: _ClassVar[int]
    NEGOTIATED_RESPONSE_VERSION_FIELD_NUMBER: _ClassVar[int]
    EFFECTIVE_SEMANTIC_REQUEST_ID_FIELD_NUMBER: _ClassVar[int]
    CANCEL_TOKEN_FIELD_NUMBER: _ClassVar[int]
    daemon_query_id: str
    resume_token: bytes
    accepted_at_unix_ms: int
    query_execution_state: QueryExecutionState
    queue_class: str
    queue_position: int
    negotiated_request_version: str
    negotiated_response_version: str
    effective_semantic_request_id: str
    cancel_token: bytes
    def __init__(self, daemon_query_id: _Optional[str] = ..., resume_token: _Optional[bytes] = ..., accepted_at_unix_ms: _Optional[int] = ..., query_execution_state: _Optional[_Union[QueryExecutionState, str]] = ..., queue_class: _Optional[str] = ..., queue_position: _Optional[int] = ..., negotiated_request_version: _Optional[str] = ..., negotiated_response_version: _Optional[str] = ..., effective_semantic_request_id: _Optional[str] = ..., cancel_token: _Optional[bytes] = ...) -> None: ...

class StreamQueryRequest(_message.Message):
    __slots__ = ("daemon_query_id", "resume_token", "after_sequence")
    DAEMON_QUERY_ID_FIELD_NUMBER: _ClassVar[int]
    RESUME_TOKEN_FIELD_NUMBER: _ClassVar[int]
    AFTER_SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    daemon_query_id: str
    resume_token: bytes
    after_sequence: int
    def __init__(self, daemon_query_id: _Optional[str] = ..., resume_token: _Optional[bytes] = ..., after_sequence: _Optional[int] = ...) -> None: ...

class AttachQueryRequest(_message.Message):
    __slots__ = ("daemon_query_id", "resume_token", "after_sequence", "after_event_checksum", "agent_instance_id", "workspace_id")
    DAEMON_QUERY_ID_FIELD_NUMBER: _ClassVar[int]
    RESUME_TOKEN_FIELD_NUMBER: _ClassVar[int]
    AFTER_SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    AFTER_EVENT_CHECKSUM_FIELD_NUMBER: _ClassVar[int]
    AGENT_INSTANCE_ID_FIELD_NUMBER: _ClassVar[int]
    WORKSPACE_ID_FIELD_NUMBER: _ClassVar[int]
    daemon_query_id: str
    resume_token: bytes
    after_sequence: int
    after_event_checksum: str
    agent_instance_id: str
    workspace_id: str
    def __init__(self, daemon_query_id: _Optional[str] = ..., resume_token: _Optional[bytes] = ..., after_sequence: _Optional[int] = ..., after_event_checksum: _Optional[str] = ..., agent_instance_id: _Optional[str] = ..., workspace_id: _Optional[str] = ...) -> None: ...

class QueryEventHeader(_message.Message):
    __slots__ = ("daemon_query_id", "sequence", "snapshot_id", "event_at_unix_ms", "event_checksum")
    DAEMON_QUERY_ID_FIELD_NUMBER: _ClassVar[int]
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    SNAPSHOT_ID_FIELD_NUMBER: _ClassVar[int]
    EVENT_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    EVENT_CHECKSUM_FIELD_NUMBER: _ClassVar[int]
    daemon_query_id: str
    sequence: int
    snapshot_id: str
    event_at_unix_ms: int
    event_checksum: str
    def __init__(self, daemon_query_id: _Optional[str] = ..., sequence: _Optional[int] = ..., snapshot_id: _Optional[str] = ..., event_at_unix_ms: _Optional[int] = ..., event_checksum: _Optional[str] = ...) -> None: ...

class SnapshotPinnedEvent(_message.Message):
    __slots__ = ("header", "canonical_public_snapshot_metadata_json", "metadata_checksum")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    CANONICAL_PUBLIC_SNAPSHOT_METADATA_JSON_FIELD_NUMBER: _ClassVar[int]
    METADATA_CHECKSUM_FIELD_NUMBER: _ClassVar[int]
    header: QueryEventHeader
    canonical_public_snapshot_metadata_json: bytes
    metadata_checksum: str
    def __init__(self, header: _Optional[_Union[QueryEventHeader, _Mapping]] = ..., canonical_public_snapshot_metadata_json: _Optional[bytes] = ..., metadata_checksum: _Optional[str] = ...) -> None: ...

class ProgressEvent(_message.Message):
    __slots__ = ("header", "phase", "completed_units", "total_units", "message")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    PHASE_FIELD_NUMBER: _ClassVar[int]
    COMPLETED_UNITS_FIELD_NUMBER: _ClassVar[int]
    TOTAL_UNITS_FIELD_NUMBER: _ClassVar[int]
    MESSAGE_FIELD_NUMBER: _ClassVar[int]
    header: QueryEventHeader
    phase: str
    completed_units: int
    total_units: int
    message: str
    def __init__(self, header: _Optional[_Union[QueryEventHeader, _Mapping]] = ..., phase: _Optional[str] = ..., completed_units: _Optional[int] = ..., total_units: _Optional[int] = ..., message: _Optional[str] = ...) -> None: ...

class ResponseChunkEvent(_message.Message):
    __slots__ = ("header", "offset", "uncompressed_length", "payload", "payload_checksum", "encoding", "final_chunk")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    OFFSET_FIELD_NUMBER: _ClassVar[int]
    UNCOMPRESSED_LENGTH_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_CHECKSUM_FIELD_NUMBER: _ClassVar[int]
    ENCODING_FIELD_NUMBER: _ClassVar[int]
    FINAL_CHUNK_FIELD_NUMBER: _ClassVar[int]
    header: QueryEventHeader
    offset: int
    uncompressed_length: int
    payload: bytes
    payload_checksum: str
    encoding: PayloadCompression
    final_chunk: bool
    def __init__(self, header: _Optional[_Union[QueryEventHeader, _Mapping]] = ..., offset: _Optional[int] = ..., uncompressed_length: _Optional[int] = ..., payload: _Optional[bytes] = ..., payload_checksum: _Optional[str] = ..., encoding: _Optional[_Union[PayloadCompression, str]] = ..., final_chunk: _Optional[bool] = ...) -> None: ...

class ArtifactReadyEvent(_message.Message):
    __slots__ = ("header", "artifact_id", "artifact_checksum", "content_type", "encoding", "lease_expires_at_unix_ms", "lease_token")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    ARTIFACT_ID_FIELD_NUMBER: _ClassVar[int]
    ARTIFACT_CHECKSUM_FIELD_NUMBER: _ClassVar[int]
    CONTENT_TYPE_FIELD_NUMBER: _ClassVar[int]
    ENCODING_FIELD_NUMBER: _ClassVar[int]
    LEASE_EXPIRES_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    LEASE_TOKEN_FIELD_NUMBER: _ClassVar[int]
    header: QueryEventHeader
    artifact_id: str
    artifact_checksum: str
    content_type: str
    encoding: PayloadCompression
    lease_expires_at_unix_ms: int
    lease_token: str
    def __init__(self, header: _Optional[_Union[QueryEventHeader, _Mapping]] = ..., artifact_id: _Optional[str] = ..., artifact_checksum: _Optional[str] = ..., content_type: _Optional[str] = ..., encoding: _Optional[_Union[PayloadCompression, str]] = ..., lease_expires_at_unix_ms: _Optional[int] = ..., lease_token: _Optional[str] = ...) -> None: ...

class TerminalEvent(_message.Message):
    __slots__ = ("header", "execution_state", "availability_state", "freshness_state", "limit_state", "dependency_state", "canonical_response_checksum", "canonical_error_record_json", "artifact_id", "result_row_count", "result_byte_count", "cleanup_state", "semantic_execution_state", "completeness_state", "truncated", "query_statuses", "notices")
    HEADER_FIELD_NUMBER: _ClassVar[int]
    EXECUTION_STATE_FIELD_NUMBER: _ClassVar[int]
    AVAILABILITY_STATE_FIELD_NUMBER: _ClassVar[int]
    FRESHNESS_STATE_FIELD_NUMBER: _ClassVar[int]
    LIMIT_STATE_FIELD_NUMBER: _ClassVar[int]
    DEPENDENCY_STATE_FIELD_NUMBER: _ClassVar[int]
    CANONICAL_RESPONSE_CHECKSUM_FIELD_NUMBER: _ClassVar[int]
    CANONICAL_ERROR_RECORD_JSON_FIELD_NUMBER: _ClassVar[int]
    ARTIFACT_ID_FIELD_NUMBER: _ClassVar[int]
    RESULT_ROW_COUNT_FIELD_NUMBER: _ClassVar[int]
    RESULT_BYTE_COUNT_FIELD_NUMBER: _ClassVar[int]
    CLEANUP_STATE_FIELD_NUMBER: _ClassVar[int]
    SEMANTIC_EXECUTION_STATE_FIELD_NUMBER: _ClassVar[int]
    COMPLETENESS_STATE_FIELD_NUMBER: _ClassVar[int]
    TRUNCATED_FIELD_NUMBER: _ClassVar[int]
    QUERY_STATUSES_FIELD_NUMBER: _ClassVar[int]
    NOTICES_FIELD_NUMBER: _ClassVar[int]
    header: QueryEventHeader
    execution_state: QueryExecutionState
    availability_state: str
    freshness_state: str
    limit_state: str
    dependency_state: str
    canonical_response_checksum: str
    canonical_error_record_json: bytes
    artifact_id: str
    result_row_count: int
    result_byte_count: int
    cleanup_state: str
    semantic_execution_state: str
    completeness_state: str
    truncated: bool
    query_statuses: _containers.RepeatedCompositeFieldContainer[QueryStatusSummary]
    notices: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, header: _Optional[_Union[QueryEventHeader, _Mapping]] = ..., execution_state: _Optional[_Union[QueryExecutionState, str]] = ..., availability_state: _Optional[str] = ..., freshness_state: _Optional[str] = ..., limit_state: _Optional[str] = ..., dependency_state: _Optional[str] = ..., canonical_response_checksum: _Optional[str] = ..., canonical_error_record_json: _Optional[bytes] = ..., artifact_id: _Optional[str] = ..., result_row_count: _Optional[int] = ..., result_byte_count: _Optional[int] = ..., cleanup_state: _Optional[str] = ..., semantic_execution_state: _Optional[str] = ..., completeness_state: _Optional[str] = ..., truncated: _Optional[bool] = ..., query_statuses: _Optional[_Iterable[_Union[QueryStatusSummary, _Mapping]]] = ..., notices: _Optional[_Iterable[str]] = ...) -> None: ...

class QueryStatusSummary(_message.Message):
    __slots__ = ("query_id", "execution_state", "canonical_error_record_json", "notices")
    QUERY_ID_FIELD_NUMBER: _ClassVar[int]
    EXECUTION_STATE_FIELD_NUMBER: _ClassVar[int]
    CANONICAL_ERROR_RECORD_JSON_FIELD_NUMBER: _ClassVar[int]
    NOTICES_FIELD_NUMBER: _ClassVar[int]
    query_id: str
    execution_state: str
    canonical_error_record_json: bytes
    notices: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, query_id: _Optional[str] = ..., execution_state: _Optional[str] = ..., canonical_error_record_json: _Optional[bytes] = ..., notices: _Optional[_Iterable[str]] = ...) -> None: ...

class QueryEvent(_message.Message):
    __slots__ = ("snapshot_pinned", "progress", "response_chunk", "artifact_ready", "terminal")
    SNAPSHOT_PINNED_FIELD_NUMBER: _ClassVar[int]
    PROGRESS_FIELD_NUMBER: _ClassVar[int]
    RESPONSE_CHUNK_FIELD_NUMBER: _ClassVar[int]
    ARTIFACT_READY_FIELD_NUMBER: _ClassVar[int]
    TERMINAL_FIELD_NUMBER: _ClassVar[int]
    snapshot_pinned: SnapshotPinnedEvent
    progress: ProgressEvent
    response_chunk: ResponseChunkEvent
    artifact_ready: ArtifactReadyEvent
    terminal: TerminalEvent
    def __init__(self, snapshot_pinned: _Optional[_Union[SnapshotPinnedEvent, _Mapping]] = ..., progress: _Optional[_Union[ProgressEvent, _Mapping]] = ..., response_chunk: _Optional[_Union[ResponseChunkEvent, _Mapping]] = ..., artifact_ready: _Optional[_Union[ArtifactReadyEvent, _Mapping]] = ..., terminal: _Optional[_Union[TerminalEvent, _Mapping]] = ...) -> None: ...

class CancelQueryRequest(_message.Message):
    __slots__ = ("daemon_query_id", "cancel_token", "agent_instance_id", "workspace_id", "reason")
    DAEMON_QUERY_ID_FIELD_NUMBER: _ClassVar[int]
    CANCEL_TOKEN_FIELD_NUMBER: _ClassVar[int]
    AGENT_INSTANCE_ID_FIELD_NUMBER: _ClassVar[int]
    WORKSPACE_ID_FIELD_NUMBER: _ClassVar[int]
    REASON_FIELD_NUMBER: _ClassVar[int]
    daemon_query_id: str
    cancel_token: bytes
    agent_instance_id: str
    workspace_id: str
    reason: str
    def __init__(self, daemon_query_id: _Optional[str] = ..., cancel_token: _Optional[bytes] = ..., agent_instance_id: _Optional[str] = ..., workspace_id: _Optional[str] = ..., reason: _Optional[str] = ...) -> None: ...

class CancelQueryResponse(_message.Message):
    __slots__ = ("daemon_query_id", "state", "acknowledged_at_unix_ms", "terminal_state", "cleaning_up_components", "forced_termination")
    DAEMON_QUERY_ID_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    ACKNOWLEDGED_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    TERMINAL_STATE_FIELD_NUMBER: _ClassVar[int]
    CLEANING_UP_COMPONENTS_FIELD_NUMBER: _ClassVar[int]
    FORCED_TERMINATION_FIELD_NUMBER: _ClassVar[int]
    daemon_query_id: str
    state: CancellationState
    acknowledged_at_unix_ms: int
    terminal_state: QueryExecutionState
    cleaning_up_components: _containers.RepeatedScalarFieldContainer[str]
    forced_termination: bool
    def __init__(self, daemon_query_id: _Optional[str] = ..., state: _Optional[_Union[CancellationState, str]] = ..., acknowledged_at_unix_ms: _Optional[int] = ..., terminal_state: _Optional[_Union[QueryExecutionState, str]] = ..., cleaning_up_components: _Optional[_Iterable[str]] = ..., forced_termination: _Optional[bool] = ...) -> None: ...

class ReadResultRequest(_message.Message):
    __slots__ = ("artifact_id", "offset", "maximum_bytes", "lease_token", "accepted_compression")
    ARTIFACT_ID_FIELD_NUMBER: _ClassVar[int]
    OFFSET_FIELD_NUMBER: _ClassVar[int]
    MAXIMUM_BYTES_FIELD_NUMBER: _ClassVar[int]
    LEASE_TOKEN_FIELD_NUMBER: _ClassVar[int]
    ACCEPTED_COMPRESSION_FIELD_NUMBER: _ClassVar[int]
    artifact_id: str
    offset: int
    maximum_bytes: int
    lease_token: str
    accepted_compression: PayloadCompression
    def __init__(self, artifact_id: _Optional[str] = ..., offset: _Optional[int] = ..., maximum_bytes: _Optional[int] = ..., lease_token: _Optional[str] = ..., accepted_compression: _Optional[_Union[PayloadCompression, str]] = ...) -> None: ...

class ResultChunk(_message.Message):
    __slots__ = ("artifact_id", "offset", "uncompressed_length", "payload", "payload_checksum", "artifact_checksum", "content_type", "encoding", "final_chunk", "lease_expires_at_unix_ms")
    ARTIFACT_ID_FIELD_NUMBER: _ClassVar[int]
    OFFSET_FIELD_NUMBER: _ClassVar[int]
    UNCOMPRESSED_LENGTH_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    PAYLOAD_CHECKSUM_FIELD_NUMBER: _ClassVar[int]
    ARTIFACT_CHECKSUM_FIELD_NUMBER: _ClassVar[int]
    CONTENT_TYPE_FIELD_NUMBER: _ClassVar[int]
    ENCODING_FIELD_NUMBER: _ClassVar[int]
    FINAL_CHUNK_FIELD_NUMBER: _ClassVar[int]
    LEASE_EXPIRES_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    artifact_id: str
    offset: int
    uncompressed_length: int
    payload: bytes
    payload_checksum: str
    artifact_checksum: str
    content_type: str
    encoding: PayloadCompression
    final_chunk: bool
    lease_expires_at_unix_ms: int
    def __init__(self, artifact_id: _Optional[str] = ..., offset: _Optional[int] = ..., uncompressed_length: _Optional[int] = ..., payload: _Optional[bytes] = ..., payload_checksum: _Optional[str] = ..., artifact_checksum: _Optional[str] = ..., content_type: _Optional[str] = ..., encoding: _Optional[_Union[PayloadCompression, str]] = ..., final_chunk: _Optional[bool] = ..., lease_expires_at_unix_ms: _Optional[int] = ...) -> None: ...

class ReleaseResultRequest(_message.Message):
    __slots__ = ("artifact_id", "lease_token")
    ARTIFACT_ID_FIELD_NUMBER: _ClassVar[int]
    LEASE_TOKEN_FIELD_NUMBER: _ClassVar[int]
    artifact_id: str
    lease_token: str
    def __init__(self, artifact_id: _Optional[str] = ..., lease_token: _Optional[str] = ...) -> None: ...

class ReleaseResultResponse(_message.Message):
    __slots__ = ("artifact_id", "released", "remaining_lease_expires_at_unix_ms")
    ARTIFACT_ID_FIELD_NUMBER: _ClassVar[int]
    RELEASED_FIELD_NUMBER: _ClassVar[int]
    REMAINING_LEASE_EXPIRES_AT_UNIX_MS_FIELD_NUMBER: _ClassVar[int]
    artifact_id: str
    released: bool
    remaining_lease_expires_at_unix_ms: int
    def __init__(self, artifact_id: _Optional[str] = ..., released: _Optional[bool] = ..., remaining_lease_expires_at_unix_ms: _Optional[int] = ...) -> None: ...
