"""Generated registry types and immutable lookup views."""

from enum import IntEnum, IntFlag
from types import MappingProxyType


class EvidenceCertainty(IntEnum):
    SOURCE_EXACT = 10
    COMPILER_EXACT = 20
    STATIC_SEMANTIC = 30
    SOUND_MAY = 40
    MODELLED = 50
    HEURISTIC = 60
    UNRESOLVED = 70


class ResolutionClass(IntEnum):
    EXACT = 10
    STATICALLY_RESOLVED = 20
    SOUND_POSSIBLE = 30
    POSSIBLE = 40
    MODELLED = 50
    HEURISTIC = 60
    UNRESOLVED = 70
    UNAVAILABLE = 80
    NOT_APPLICABLE = 90


class Directness(IntEnum):
    DIRECT = 10
    TRANSITIVE = 20
    SUMMARY = 30
    NOT_APPLICABLE = 40


class Completeness(IntEnum):
    COMPLETE = 10
    PARTIAL = 20
    INDETERMINATE = 30
    UNAVAILABLE = 40
    NOT_APPLICABLE = 50


class OwnerCapabilityState(IntEnum):
    CURRENT = 10
    PENDING = 20
    INVALIDATED = 30
    PARTIAL = 40
    UNAVAILABLE_PARSE = 50
    UNAVAILABLE_COMPILE = 60
    UNAVAILABLE_PROVIDER = 70
    UNAVAILABLE_DERIVATION = 80
    EXCLUDED = 90
    UNSUPPORTED = 100
    REMOVED = 110
    NOT_APPLICABLE = 120


class ProviderRunState(IntEnum):
    QUEUED = 10
    RUNNING = 20
    SUCCEEDED = 30
    PARTIAL = 40
    FAILED = 50
    TIMED_OUT = 60
    CANCELLED = 70
    SUPERSEDED = 80
    CRASHED = 90
    PROTOCOL_ERROR = 100
    STALE_RESULT = 110
    STALE_GIT_BASELINE = 120


class QueryExecutionState(IntEnum):
    ACCEPTED = 10
    RUNNING = 20
    COMPLETE = 30
    FAILED = 40
    CANCELLED = 50
    DEADLINE_EXCEEDED = 60
    NOT_EXECUTED_DEPENDENCY = 70


class QueryAvailabilityState(IntEnum):
    AVAILABLE = 10
    PARTIAL = 20
    UNAVAILABLE = 30
    NOT_APPLICABLE = 40


class CompletenessState(IntEnum):
    COMPLETE = 10
    PARTIAL = 20
    INDETERMINATE = 30
    UNAVAILABLE = 40
    NOT_APPLICABLE = 50


class FreshnessState(IntEnum):
    CURRENT = 10
    POTENTIALLY_STALE = 20
    UNAVAILABLE = 30


class LimitState(IntEnum):
    NOT_APPLIED = 10
    EXPLICIT_LIMIT_REACHED = 20
    HARD_LIMIT_REJECTED = 30


class DependencyState(IntEnum):
    READY = 10
    FAILED_DEPENDENCY = 20
    NOT_APPLICABLE = 30


class DurablePublicationState(IntEnum):
    STAGING = 10
    VALIDATING = 20
    VALIDATED = 30
    COMMITTING = 40
    COMPLETE = 50
    FAILED = 60
    ABANDONED = 70


class ServingActivationState(IntEnum):
    BUILDING = 10
    VALIDATING = 20
    READY = 30
    ACTIVE = 40
    RETIRED = 50
    FAILED = 60


class SourceTrustState(IntEnum):
    UNVERIFIED = 10
    VERIFYING = 20
    CURRENT = 30
    POTENTIALLY_STALE = 40
    UNAVAILABLE = 50


class EventStreamHealth(IntEnum):
    HEALTHY = 10
    RESCAN_REQUIRED = 20
    DEGRADED = 30
    UNAVAILABLE = 40


class GitAccelerationStatus(IntEnum):
    NOT_A_GIT_WORKTREE = 10
    GIT_UNAVAILABLE = 20
    GIT_READY = 30
    GIT_METADATA_DIRTY = 40
    GIT_SCANNING = 50
    GIT_OPERATION_IN_PROGRESS = 60
    GIT_BULK_RECONCILING = 70
    GIT_DEGRADED = 80


class EffectKind(IntEnum):
    READ_MEMORY = 10
    WRITE_MEMORY = 20
    ALLOCATE_MEMORY = 30
    DEALLOCATE_MEMORY = 40
    READ_FILE = 50
    WRITE_FILE = 60
    READ_NETWORK = 70
    WRITE_NETWORK = 80
    READ_DATABASE = 90
    WRITE_DATABASE = 100
    BEGIN_TRANSACTION = 110
    COMMIT_TRANSACTION = 120
    ROLLBACK_TRANSACTION = 130
    READ_STANDARD_INPUT = 140
    WRITE_STANDARD_OUTPUT = 150
    LOG_OR_TELEMETRY = 160
    READ_ENVIRONMENT = 170
    WRITE_ENVIRONMENT = 180
    SPAWN_PROCESS = 190
    SPAWN_THREAD_OR_TASK = 200
    BLOCK_THREAD = 210
    SLEEP_OR_WAIT = 220
    LOAD_DYNAMIC_LIBRARY = 230
    ACQUIRE_LOCK = 240
    RELEASE_LOCK = 250
    SEND_CHANNEL = 260
    RECEIVE_CHANNEL = 270
    READ_TIME = 280
    READ_RANDOMNESS = 290
    READ_GLOBAL_STATE = 300
    WRITE_GLOBAL_STATE = 310
    RAISE_EXCEPTION = 320
    PANIC_OR_ABORT = 330
    UNSAFE_OPERATION = 340
    FFI_CALL = 350
    DYNAMIC_CODE_EXECUTION = 360
    UNKNOWN_EXTERNAL_EFFECT = 370


class ResourceKind(IntEnum):
    FILE_HANDLE = 10
    SOCKET_OR_CONNECTION = 20
    DATABASE_CONNECTION_OR_TRANSACTION = 30
    LOCK_GUARD = 40
    CHANNEL_ENDPOINT = 50
    PROCESS_HANDLE = 60
    THREAD_OR_TASK_HANDLE = 70
    MEMORY_ALLOCATION = 80
    USER_DEFINED_MODELLED_RESOURCE = 90
    UNKNOWN_RESOURCE = 100


class WorkspaceLifecycle(IntEnum):
    BOOTSTRAPPING = 10
    READY = 20
    DEGRADED = 30
    DISABLED = 40
    FAILED = 50


class UpdateWaveState(IntEnum):
    COLLECTING = 10
    SNAPSHOTTING = 20
    RUNNING = 30
    PUBLISHING = 40
    COMPLETE = 50
    FAILED = 60
    SUPERSEDED = 70


class ArtifactState(IntEnum):
    BUILDING = 10
    READY = 20
    ACTIVE = 30
    EXPIRED = 40
    FAILED = 50


class WorkspaceRegistryLifecycle(IntEnum):
    REGISTERING = 10
    DISABLED = 20
    OPENING = 30
    BOOTSTRAPPING = 40
    READY = 50
    DEGRADED = 60
    DISABLING = 70
    REMOVING = 80
    REMOVED = 90
    FAILED = 100


class FactFlags(IntFlag):
    NONE = 0
    GENERATED = 1 << 0
    LOWERED = 1 << 1
    EXTERNAL = 1 << 2
    UNKNOWN_REMAINDER = 1 << 3
    PYTHON_PROFILE = 1 << 32
    RUST_PROFILE = 1 << 33
    COMPILER_SYNTHETIC = 1 << 48


ENUM_TRIPLES = MappingProxyType(
    {
        "EVIDENCE_CERTAINTY": (
            (
                10,
                "SOURCE_EXACT",
                "source-exact",
            ),
            (
                20,
                "COMPILER_EXACT",
                "compiler-exact",
            ),
            (
                30,
                "STATIC_SEMANTIC",
                "static-semantic",
            ),
            (
                40,
                "SOUND_MAY",
                "sound-may",
            ),
            (
                50,
                "MODELLED",
                "modelled",
            ),
            (
                60,
                "HEURISTIC",
                "heuristic",
            ),
            (
                70,
                "UNRESOLVED",
                "unresolved",
            ),
        ),
        "RESOLUTION_CLASS": (
            (
                10,
                "EXACT",
                "exact",
            ),
            (
                20,
                "STATICALLY_RESOLVED",
                "statically-resolved",
            ),
            (
                30,
                "SOUND_POSSIBLE",
                "sound-possible",
            ),
            (
                40,
                "POSSIBLE",
                "possible",
            ),
            (
                50,
                "MODELLED",
                "modelled",
            ),
            (
                60,
                "HEURISTIC",
                "heuristic",
            ),
            (
                70,
                "UNRESOLVED",
                "unresolved",
            ),
            (
                80,
                "UNAVAILABLE",
                "unavailable",
            ),
            (
                90,
                "NOT_APPLICABLE",
                "not-applicable",
            ),
        ),
        "DIRECTNESS": (
            (
                10,
                "DIRECT",
                "direct",
            ),
            (
                20,
                "TRANSITIVE",
                "transitive",
            ),
            (
                30,
                "SUMMARY",
                "summary",
            ),
            (
                40,
                "NOT_APPLICABLE",
                "not-applicable",
            ),
        ),
        "COMPLETENESS": (
            (
                10,
                "COMPLETE",
                "complete",
            ),
            (
                20,
                "PARTIAL",
                "partial",
            ),
            (
                30,
                "INDETERMINATE",
                "indeterminate",
            ),
            (
                40,
                "UNAVAILABLE",
                "unavailable",
            ),
            (
                50,
                "NOT_APPLICABLE",
                "not-applicable",
            ),
        ),
        "OWNER_CAPABILITY_STATE": (
            (
                10,
                "CURRENT",
                "current",
            ),
            (
                20,
                "PENDING",
                "pending",
            ),
            (
                30,
                "INVALIDATED",
                "invalidated",
            ),
            (
                40,
                "PARTIAL",
                "partial",
            ),
            (
                50,
                "UNAVAILABLE_PARSE",
                "unavailable-parse",
            ),
            (
                60,
                "UNAVAILABLE_COMPILE",
                "unavailable-compile",
            ),
            (
                70,
                "UNAVAILABLE_PROVIDER",
                "unavailable-provider",
            ),
            (
                80,
                "UNAVAILABLE_DERIVATION",
                "unavailable-derivation",
            ),
            (
                90,
                "EXCLUDED",
                "excluded",
            ),
            (
                100,
                "UNSUPPORTED",
                "unsupported",
            ),
            (
                110,
                "REMOVED",
                "removed",
            ),
            (
                120,
                "NOT_APPLICABLE",
                "not-applicable",
            ),
        ),
        "PROVIDER_RUN_STATE": (
            (
                10,
                "QUEUED",
                "queued",
            ),
            (
                20,
                "RUNNING",
                "running",
            ),
            (
                30,
                "SUCCEEDED",
                "succeeded",
            ),
            (
                40,
                "PARTIAL",
                "partial",
            ),
            (
                50,
                "FAILED",
                "failed",
            ),
            (
                60,
                "TIMED_OUT",
                "timed-out",
            ),
            (
                70,
                "CANCELLED",
                "cancelled",
            ),
            (
                80,
                "SUPERSEDED",
                "superseded",
            ),
            (
                90,
                "CRASHED",
                "crashed",
            ),
            (
                100,
                "PROTOCOL_ERROR",
                "protocol-error",
            ),
            (
                110,
                "STALE_RESULT",
                "stale-result",
            ),
            (
                120,
                "STALE_GIT_BASELINE",
                "stale-git-baseline",
            ),
        ),
        "QUERY_EXECUTION_STATE": (
            (
                10,
                "ACCEPTED",
                "accepted",
            ),
            (
                20,
                "RUNNING",
                "running",
            ),
            (
                30,
                "COMPLETE",
                "complete",
            ),
            (
                40,
                "FAILED",
                "failed",
            ),
            (
                50,
                "CANCELLED",
                "cancelled",
            ),
            (
                60,
                "DEADLINE_EXCEEDED",
                "deadline-exceeded",
            ),
            (
                70,
                "NOT_EXECUTED_DEPENDENCY",
                "not-executed-dependency",
            ),
        ),
        "QUERY_AVAILABILITY_STATE": (
            (
                10,
                "AVAILABLE",
                "available",
            ),
            (
                20,
                "PARTIAL",
                "partial",
            ),
            (
                30,
                "UNAVAILABLE",
                "unavailable",
            ),
            (
                40,
                "NOT_APPLICABLE",
                "not-applicable",
            ),
        ),
        "COMPLETENESS_STATE": (
            (
                10,
                "COMPLETE",
                "complete",
            ),
            (
                20,
                "PARTIAL",
                "partial",
            ),
            (
                30,
                "INDETERMINATE",
                "indeterminate",
            ),
            (
                40,
                "UNAVAILABLE",
                "unavailable",
            ),
            (
                50,
                "NOT_APPLICABLE",
                "not-applicable",
            ),
        ),
        "FRESHNESS_STATE": (
            (
                10,
                "CURRENT",
                "current",
            ),
            (
                20,
                "POTENTIALLY_STALE",
                "potentially-stale",
            ),
            (
                30,
                "UNAVAILABLE",
                "unavailable",
            ),
        ),
        "LIMIT_STATE": (
            (
                10,
                "NOT_APPLIED",
                "not-applied",
            ),
            (
                20,
                "EXPLICIT_LIMIT_REACHED",
                "explicit-limit-reached",
            ),
            (
                30,
                "HARD_LIMIT_REJECTED",
                "hard-limit-rejected",
            ),
        ),
        "DEPENDENCY_STATE": (
            (
                10,
                "READY",
                "ready",
            ),
            (
                20,
                "FAILED_DEPENDENCY",
                "failed-dependency",
            ),
            (
                30,
                "NOT_APPLICABLE",
                "not-applicable",
            ),
        ),
        "DURABLE_PUBLICATION_STATE": (
            (
                10,
                "STAGING",
                "staging",
            ),
            (
                20,
                "VALIDATING",
                "validating",
            ),
            (
                30,
                "VALIDATED",
                "validated",
            ),
            (
                40,
                "COMMITTING",
                "committing",
            ),
            (
                50,
                "COMPLETE",
                "complete",
            ),
            (
                60,
                "FAILED",
                "failed",
            ),
            (
                70,
                "ABANDONED",
                "abandoned",
            ),
        ),
        "SERVING_ACTIVATION_STATE": (
            (
                10,
                "BUILDING",
                "building",
            ),
            (
                20,
                "VALIDATING",
                "validating",
            ),
            (
                30,
                "READY",
                "ready",
            ),
            (
                40,
                "ACTIVE",
                "active",
            ),
            (
                50,
                "RETIRED",
                "retired",
            ),
            (
                60,
                "FAILED",
                "failed",
            ),
        ),
        "SOURCE_TRUST_STATE": (
            (
                10,
                "UNVERIFIED",
                "unverified",
            ),
            (
                20,
                "VERIFYING",
                "verifying",
            ),
            (
                30,
                "CURRENT",
                "current",
            ),
            (
                40,
                "POTENTIALLY_STALE",
                "potentially-stale",
            ),
            (
                50,
                "UNAVAILABLE",
                "unavailable",
            ),
        ),
        "EVENT_STREAM_HEALTH": (
            (
                10,
                "HEALTHY",
                "healthy",
            ),
            (
                20,
                "RESCAN_REQUIRED",
                "rescan-required",
            ),
            (
                30,
                "DEGRADED",
                "degraded",
            ),
            (
                40,
                "UNAVAILABLE",
                "unavailable",
            ),
        ),
        "GIT_ACCELERATION_STATUS": (
            (
                10,
                "NOT_A_GIT_WORKTREE",
                "not-a-git-worktree",
            ),
            (
                20,
                "GIT_UNAVAILABLE",
                "git-unavailable",
            ),
            (
                30,
                "GIT_READY",
                "git-ready",
            ),
            (
                40,
                "GIT_METADATA_DIRTY",
                "git-metadata-dirty",
            ),
            (
                50,
                "GIT_SCANNING",
                "git-scanning",
            ),
            (
                60,
                "GIT_OPERATION_IN_PROGRESS",
                "git-operation-in-progress",
            ),
            (
                70,
                "GIT_BULK_RECONCILING",
                "git-bulk-reconciling",
            ),
            (
                80,
                "GIT_DEGRADED",
                "git-degraded",
            ),
        ),
        "EFFECT_KIND": (
            (
                10,
                "READ_MEMORY",
                "read-memory",
            ),
            (
                20,
                "WRITE_MEMORY",
                "write-memory",
            ),
            (
                30,
                "ALLOCATE_MEMORY",
                "allocate-memory",
            ),
            (
                40,
                "DEALLOCATE_MEMORY",
                "deallocate-memory",
            ),
            (
                50,
                "READ_FILE",
                "read-file",
            ),
            (
                60,
                "WRITE_FILE",
                "write-file",
            ),
            (
                70,
                "READ_NETWORK",
                "read-network",
            ),
            (
                80,
                "WRITE_NETWORK",
                "write-network",
            ),
            (
                90,
                "READ_DATABASE",
                "read-database",
            ),
            (
                100,
                "WRITE_DATABASE",
                "write-database",
            ),
            (
                110,
                "BEGIN_TRANSACTION",
                "begin-transaction",
            ),
            (
                120,
                "COMMIT_TRANSACTION",
                "commit-transaction",
            ),
            (
                130,
                "ROLLBACK_TRANSACTION",
                "rollback-transaction",
            ),
            (
                140,
                "READ_STANDARD_INPUT",
                "read-standard-input",
            ),
            (
                150,
                "WRITE_STANDARD_OUTPUT",
                "write-standard-output",
            ),
            (
                160,
                "LOG_OR_TELEMETRY",
                "log-or-telemetry",
            ),
            (
                170,
                "READ_ENVIRONMENT",
                "read-environment",
            ),
            (
                180,
                "WRITE_ENVIRONMENT",
                "write-environment",
            ),
            (
                190,
                "SPAWN_PROCESS",
                "spawn-process",
            ),
            (
                200,
                "SPAWN_THREAD_OR_TASK",
                "spawn-thread-or-task",
            ),
            (
                210,
                "BLOCK_THREAD",
                "block-thread",
            ),
            (
                220,
                "SLEEP_OR_WAIT",
                "sleep-or-wait",
            ),
            (
                230,
                "LOAD_DYNAMIC_LIBRARY",
                "load-dynamic-library",
            ),
            (
                240,
                "ACQUIRE_LOCK",
                "acquire-lock",
            ),
            (
                250,
                "RELEASE_LOCK",
                "release-lock",
            ),
            (
                260,
                "SEND_CHANNEL",
                "send-channel",
            ),
            (
                270,
                "RECEIVE_CHANNEL",
                "receive-channel",
            ),
            (
                280,
                "READ_TIME",
                "read-time",
            ),
            (
                290,
                "READ_RANDOMNESS",
                "read-randomness",
            ),
            (
                300,
                "READ_GLOBAL_STATE",
                "read-global-state",
            ),
            (
                310,
                "WRITE_GLOBAL_STATE",
                "write-global-state",
            ),
            (
                320,
                "RAISE_EXCEPTION",
                "raise-exception",
            ),
            (
                330,
                "PANIC_OR_ABORT",
                "panic-or-abort",
            ),
            (
                340,
                "UNSAFE_OPERATION",
                "unsafe-operation",
            ),
            (
                350,
                "FFI_CALL",
                "ffi-call",
            ),
            (
                360,
                "DYNAMIC_CODE_EXECUTION",
                "dynamic-code-execution",
            ),
            (
                370,
                "UNKNOWN_EXTERNAL_EFFECT",
                "unknown-external-effect",
            ),
        ),
        "RESOURCE_KIND": (
            (
                10,
                "FILE_HANDLE",
                "file-handle",
            ),
            (
                20,
                "SOCKET_OR_CONNECTION",
                "socket-or-connection",
            ),
            (
                30,
                "DATABASE_CONNECTION_OR_TRANSACTION",
                "database-connection-or-transaction",
            ),
            (
                40,
                "LOCK_GUARD",
                "lock-guard",
            ),
            (
                50,
                "CHANNEL_ENDPOINT",
                "channel-endpoint",
            ),
            (
                60,
                "PROCESS_HANDLE",
                "process-handle",
            ),
            (
                70,
                "THREAD_OR_TASK_HANDLE",
                "thread-or-task-handle",
            ),
            (
                80,
                "MEMORY_ALLOCATION",
                "memory-allocation",
            ),
            (
                90,
                "USER_DEFINED_MODELLED_RESOURCE",
                "user-defined-modelled-resource",
            ),
            (
                100,
                "UNKNOWN_RESOURCE",
                "unknown-resource",
            ),
        ),
    }
)

STATE_TRANSITIONS = MappingProxyType(
    {
        "WorkspaceLifecycle": (
            (
                "BOOTSTRAPPING",
                "first-valid-snapshot-activated",
                "snapshot-valid",
                "READY",
                ("publish-readiness",),
                "workspace:first-snapshot",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "BOOTSTRAPPING",
                "bootstrap-failed",
                "terminal-failure",
                "FAILED",
                ("publish-diagnostic",),
                "workspace:bootstrap-failed",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "READY",
                "source-inaccessible",
                "source-read-failed",
                "DEGRADED",
                (
                    "preserve-last-snapshot",
                    "reject-strict-current",
                ),
                "workspace:degraded",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "READY",
                "disable",
                "operator-authorized",
                "DISABLED",
                (
                    "stop-watchers",
                    "stop-providers",
                ),
                "workspace:disable",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "DEGRADED",
                "source-reconciled",
                "source-current",
                "READY",
                ("publish-readiness",),
                "workspace:recover",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "DEGRADED",
                "disable",
                "operator-authorized",
                "DISABLED",
                (
                    "stop-watchers",
                    "stop-providers",
                ),
                "workspace:disable",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "DEGRADED",
                "terminal-failure",
                "unrecoverable",
                "FAILED",
                ("publish-diagnostic",),
                "workspace:failed",
                "STATE_TRANSITION_VIOLATION",
            ),
        ),
        "SourceTrustState": (
            (
                "UNVERIFIED",
                "verification-started",
                "root-authorized",
                "VERIFYING",
                ("open-authoritative-source",),
                "source:verify",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "VERIFYING",
                "source-reconciled",
                "stable-reads-accepted",
                "CURRENT",
                ("advance-source-generation",),
                "source:current",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "VERIFYING",
                "source-failed",
                "terminal-read-failure",
                "UNAVAILABLE",
                ("publish-capability-gap",),
                "source:unavailable",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "CURRENT",
                "relevant-event-admitted",
                "event-in-boundary",
                "POTENTIALLY_STALE",
                ("increment-barrier-sequence",),
                "source:event",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "CURRENT",
                "source-failed",
                "terminal-read-failure",
                "UNAVAILABLE",
                ("publish-capability-gap",),
                "source:unavailable",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "POTENTIALLY_STALE",
                "verification-started",
                "reconciliation-admitted",
                "VERIFYING",
                ("open-authoritative-source",),
                "source:reverify",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "POTENTIALLY_STALE",
                "source-failed",
                "terminal-read-failure",
                "UNAVAILABLE",
                ("publish-capability-gap",),
                "source:unavailable",
                "STATE_TRANSITION_VIOLATION",
            ),
        ),
        "EventStreamHealth": (
            (
                "HEALTHY",
                "overflow",
                "overflow-or-rescan-flag",
                "RESCAN_REQUIRED",
                ("schedule-authoritative-rescan",),
                "events:rescan",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "HEALTHY",
                "backend-degraded",
                "backend-warning",
                "DEGRADED",
                ("schedule-reconciliation",),
                "events:degraded",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "HEALTHY",
                "backend-lost",
                "backend-terminal",
                "UNAVAILABLE",
                ("publish-diagnostic",),
                "events:unavailable",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "RESCAN_REQUIRED",
                "rescan-complete",
                "authoritative-walk-complete",
                "HEALTHY",
                ("advance-reconciled-sequence",),
                "events:healthy",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "RESCAN_REQUIRED",
                "backend-lost",
                "backend-terminal",
                "UNAVAILABLE",
                ("publish-diagnostic",),
                "events:unavailable",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "DEGRADED",
                "backend-lost",
                "backend-terminal",
                "UNAVAILABLE",
                ("publish-diagnostic",),
                "events:unavailable",
                "STATE_TRANSITION_VIOLATION",
            ),
        ),
        "GitAccelerationStatus": (
            (
                "GIT_SCANNING",
                "not-worktree",
                "discovery-complete",
                "NOT_A_GIT_WORKTREE",
                ("use-generic-inventory",),
                "git:not-worktree",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "GIT_SCANNING",
                "git-failed",
                "discovery-failed",
                "GIT_UNAVAILABLE",
                ("use-generic-inventory",),
                "git:unavailable",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "GIT_SCANNING",
                "scan-complete",
                "repository-stable",
                "GIT_READY",
                ("publish-git-baseline",),
                "git:ready",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "GIT_READY",
                "metadata-changed",
                "metadata-path-event",
                "GIT_METADATA_DIRTY",
                ("invalidate-git-baseline",),
                "git:dirty",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "GIT_READY",
                "operation-started",
                "operation-marker-present",
                "GIT_OPERATION_IN_PROGRESS",
                ("defer-fast-lane",),
                "git:operation",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "GIT_READY",
                "acceleration-failed",
                "recoverable-error",
                "GIT_DEGRADED",
                ("use-generic-inventory",),
                "git:degraded",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "GIT_METADATA_DIRTY",
                "bulk-reconcile-started",
                "stable-window",
                "GIT_BULK_RECONCILING",
                ("run-authoritative-reconciliation",),
                "git:bulk",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "GIT_OPERATION_IN_PROGRESS",
                "operation-ended",
                "markers-cleared",
                "GIT_BULK_RECONCILING",
                ("run-authoritative-reconciliation",),
                "git:bulk",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "GIT_BULK_RECONCILING",
                "reconcile-failed",
                "terminal-git-failure",
                "GIT_UNAVAILABLE",
                ("use-generic-inventory",),
                "git:unavailable",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "GIT_DEGRADED",
                "git-failed",
                "terminal-git-failure",
                "GIT_UNAVAILABLE",
                ("use-generic-inventory",),
                "git:unavailable",
                "STATE_TRANSITION_VIOLATION",
            ),
        ),
        "UpdateWaveState": (
            (
                "COLLECTING",
                "gather-barrier-closed",
                "stable-window",
                "SNAPSHOTTING",
                ("freeze-path-set",),
                "wave:snapshot",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "SNAPSHOTTING",
                "source-image-accepted",
                "reads-stable",
                "RUNNING",
                ("schedule-providers",),
                "wave:run",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "RUNNING",
                "outputs-staged",
                "required-capabilities-terminal",
                "PUBLISHING",
                ("validate-publication",),
                "wave:publish",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "PUBLISHING",
                "activation-complete",
                "pointer-active",
                "COMPLETE",
                ("release-wave-resources",),
                "wave:complete",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "COLLECTING",
                "newer-wave",
                "no-committed-dependents",
                "SUPERSEDED",
                ("discard-outputs",),
                "wave:supersede",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "SNAPSHOTTING",
                "newer-wave",
                "no-committed-dependents",
                "SUPERSEDED",
                ("discard-outputs",),
                "wave:supersede",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "RUNNING",
                "newer-wave",
                "no-committed-dependents",
                "SUPERSEDED",
                (
                    "cancel-providers",
                    "discard-outputs",
                ),
                "wave:supersede",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "COLLECTING",
                "terminal-failure",
                "unrecoverable",
                "FAILED",
                ("publish-diagnostic",),
                "wave:failed",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "SNAPSHOTTING",
                "terminal-failure",
                "unrecoverable",
                "FAILED",
                ("publish-diagnostic",),
                "wave:failed",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "RUNNING",
                "terminal-failure",
                "unrecoverable",
                "FAILED",
                ("publish-diagnostic",),
                "wave:failed",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "PUBLISHING",
                "terminal-failure",
                "unrecoverable",
                "FAILED",
                ("publish-diagnostic",),
                "wave:failed",
                "STATE_TRANSITION_VIOLATION",
            ),
        ),
        "ProviderRunState": (
            (
                "QUEUED",
                "permit-granted",
                "capacity-available",
                "RUNNING",
                ("start-deadline",),
                "provider:start",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "QUEUED",
                "cancelled",
                "cancellation-active",
                "CANCELLED",
                ("release-permit",),
                "provider:cancel",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "QUEUED",
                "superseded",
                "newer-generation",
                "SUPERSEDED",
                ("discard-request",),
                "provider:supersede",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "QUEUED",
                "git-baseline-stale",
                "git-baseline-invalid",
                "STALE_GIT_BASELINE",
                ("discard-request",),
                "provider:stale-git",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "RUNNING",
                "terminal-manifest-complete",
                "manifest-valid",
                "SUCCEEDED",
                ("stage-output",),
                "provider:success",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "RUNNING",
                "terminal-manifest-partial",
                "manifest-valid-and-partial",
                "PARTIAL",
                ("stage-output",),
                "provider:partial",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "RUNNING",
                "domain-failure",
                "failure-valid",
                "FAILED",
                ("stage-diagnostic",),
                "provider:failed",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "RUNNING",
                "deadline-expired",
                "deadline-reached",
                "TIMED_OUT",
                ("cancel-provider",),
                "provider:timeout",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "RUNNING",
                "cancelled",
                "cancellation-acknowledged",
                "CANCELLED",
                ("discard-output",),
                "provider:cancel",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "RUNNING",
                "superseded",
                "newer-generation",
                "SUPERSEDED",
                (
                    "cancel-provider",
                    "discard-output",
                ),
                "provider:supersede",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "RUNNING",
                "process-exited",
                "unexpected-exit",
                "CRASHED",
                ("stage-diagnostic",),
                "provider:crashed",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "RUNNING",
                "protocol-violated",
                "framing-or-credit-invalid",
                "PROTOCOL_ERROR",
                (
                    "terminate-provider",
                    "stage-diagnostic",
                ),
                "provider:protocol",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "RUNNING",
                "stale-result",
                "source-generation-changed",
                "STALE_RESULT",
                ("discard-output",),
                "provider:stale",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "RUNNING",
                "git-baseline-stale",
                "git-baseline-invalid",
                "STALE_GIT_BASELINE",
                ("discard-output",),
                "provider:stale-git",
                "STATE_TRANSITION_VIOLATION",
            ),
        ),
        "OwnerCapabilityState": (
            (
                "PENDING",
                "capability-current",
                "completeness-satisfied",
                "CURRENT",
                ("record-coverage",),
                "capability:current",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "PENDING",
                "capability-partial",
                "remainder-characterized",
                "PARTIAL",
                ("record-coverage",),
                "capability:partial",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "PENDING",
                "parse-unavailable",
                "parse-terminal",
                "UNAVAILABLE_PARSE",
                ("record-gap",),
                "capability:parse",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "PENDING",
                "compile-unavailable",
                "compile-terminal",
                "UNAVAILABLE_COMPILE",
                ("record-gap",),
                "capability:compile",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "PENDING",
                "provider-unavailable",
                "provider-terminal",
                "UNAVAILABLE_PROVIDER",
                ("record-gap",),
                "capability:provider",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "PENDING",
                "derivation-unavailable",
                "derivation-terminal",
                "UNAVAILABLE_DERIVATION",
                ("record-gap",),
                "capability:derivation",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "PENDING",
                "excluded",
                "scope-excluded",
                "EXCLUDED",
                ("record-exclusion",),
                "capability:excluded",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "PENDING",
                "unsupported",
                "registry-applicability-unsupported",
                "UNSUPPORTED",
                ("record-gap",),
                "capability:unsupported",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "PENDING",
                "removed",
                "owner-removed",
                "REMOVED",
                ("record-removal",),
                "capability:removed",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "PENDING",
                "not-applicable",
                "applicability-false",
                "NOT_APPLICABLE",
                ("record-applicability",),
                "capability:not-applicable",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "PENDING",
                "source-invalidated",
                "source-generation-changed",
                "INVALIDATED",
                ("schedule-recompute",),
                "capability:invalidated",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "INVALIDATED",
                "recompute-scheduled",
                "owner-present",
                "PENDING",
                ("enqueue-owner",),
                "capability:pending",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "INVALIDATED",
                "owner-removed",
                "owner-absent",
                "REMOVED",
                ("record-removal",),
                "capability:removed",
                "STATE_TRANSITION_VIOLATION",
            ),
        ),
        "DurablePublicationState": (
            (
                "STAGING",
                "outputs-staged",
                "manifest-complete",
                "VALIDATING",
                ("validate-tables",),
                "publication:validate",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "VALIDATING",
                "validation-passed",
                "constraints-green",
                "VALIDATED",
                ("seal-manifest",),
                "publication:validated",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "VALIDATED",
                "pointer-lease-held",
                "predecessor-matches",
                "COMMITTING",
                (
                    "write-tables",
                    "write-pointer",
                ),
                "publication:commit",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "COMMITTING",
                "commit-complete",
                "durable-commit-visible",
                "COMPLETE",
                ("release-lease",),
                "publication:complete",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "STAGING",
                "abandoned",
                "superseded",
                "ABANDONED",
                ("discard-staging",),
                "publication:abandon",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "VALIDATING",
                "validation-failed",
                "terminal-validation-error",
                "FAILED",
                ("publish-diagnostic",),
                "publication:failed",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "VALIDATED",
                "commit-failed",
                "terminal-storage-error",
                "FAILED",
                ("publish-diagnostic",),
                "publication:failed",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "COMMITTING",
                "commit-failed",
                "terminal-storage-error",
                "FAILED",
                ("publish-diagnostic",),
                "publication:failed",
                "STATE_TRANSITION_VIOLATION",
            ),
        ),
        "ServingActivationState": (
            (
                "BUILDING",
                "snapshot-built",
                "publication-complete",
                "VALIDATING",
                ("run-serving-validation",),
                "serving:validate",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "VALIDATING",
                "validation-passed",
                "schema-and-index-green",
                "READY",
                ("acquire-pointer-lease",),
                "serving:ready",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "READY",
                "pointer-cas-succeeded",
                "predecessor-matches",
                "ACTIVE",
                ("retire-prior-snapshot",),
                "serving:active",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "ACTIVE",
                "successor-activated",
                "successor-active",
                "RETIRED",
                ("release-active-role",),
                "serving:retired",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "BUILDING",
                "terminal-failure",
                "unrecoverable",
                "FAILED",
                ("publish-diagnostic",),
                "serving:failed",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "VALIDATING",
                "terminal-failure",
                "unrecoverable",
                "FAILED",
                ("publish-diagnostic",),
                "serving:failed",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "READY",
                "terminal-failure",
                "unrecoverable",
                "FAILED",
                ("publish-diagnostic",),
                "serving:failed",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "ACTIVE",
                "terminal-failure",
                "active-snapshot-invalid",
                "FAILED",
                (
                    "withdraw-pointer",
                    "publish-diagnostic",
                ),
                "serving:failed",
                "STATE_TRANSITION_VIOLATION",
            ),
        ),
        "QueryExecutionState": (
            (
                "ACCEPTED",
                "stream-attached",
                "request-authorized",
                "RUNNING",
                (
                    "record-barrier",
                    "begin-planning",
                ),
                "query:run",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "ACCEPTED",
                "dependency-failed",
                "prerequisite-terminal",
                "NOT_EXECUTED_DEPENDENCY",
                ("release-request",),
                "query:dependency",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "ACCEPTED",
                "cancelled",
                "cancellation-active",
                "CANCELLED",
                ("release-request",),
                "query:cancel",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "RUNNING",
                "canonical-response-committed",
                "response-valid",
                "COMPLETE",
                ("release-execution-resources",),
                "query:complete",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "RUNNING",
                "execution-failed",
                "terminal-domain-error",
                "FAILED",
                ("release-execution-resources",),
                "query:failed",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "RUNNING",
                "cancelled",
                "cancellation-acknowledged",
                "CANCELLED",
                ("release-execution-resources",),
                "query:cancel",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "RUNNING",
                "deadline-expired",
                "deadline-reached",
                "DEADLINE_EXCEEDED",
                (
                    "cancel-execution",
                    "release-execution-resources",
                ),
                "query:deadline",
                "STATE_TRANSITION_VIOLATION",
            ),
        ),
        "ArtifactState": (
            (
                "BUILDING",
                "bytes-committed",
                "checksum-valid",
                "READY",
                ("publish-resource-reference",),
                "artifact:ready",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "READY",
                "lease-acquired",
                "resource-not-expired",
                "ACTIVE",
                ("increment-lease-count",),
                "artifact:active",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "READY",
                "ttl-expired",
                "no-active-leases",
                "EXPIRED",
                ("schedule-gc",),
                "artifact:expired",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "ACTIVE",
                "final-lease-released",
                "ttl-expired",
                "EXPIRED",
                ("schedule-gc",),
                "artifact:expired",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "BUILDING",
                "build-failed",
                "terminal-write-error",
                "FAILED",
                ("discard-partial-bytes",),
                "artifact:failed",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "READY",
                "integrity-failed",
                "checksum-mismatch",
                "FAILED",
                ("withdraw-resource-reference",),
                "artifact:failed",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "ACTIVE",
                "integrity-failed",
                "checksum-mismatch",
                "FAILED",
                ("withdraw-resource-reference",),
                "artifact:failed",
                "STATE_TRANSITION_VIOLATION",
            ),
        ),
        "WorkspaceRegistryLifecycle": (
            (
                "REGISTERING",
                "registration-created",
                "root-authorized",
                "DISABLED",
                ("persist-registration",),
                "registry:registered",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "DISABLED",
                "enable",
                "operator-authorized",
                "OPENING",
                ("open-root",),
                "registry:open",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "DISABLED",
                "remove",
                "no-active-leases",
                "REMOVING",
                ("stop-workspace",),
                "registry:remove",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "OPENING",
                "root-opened",
                "root-identity-matches",
                "BOOTSTRAPPING",
                ("start-inventory",),
                "registry:bootstrap",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "OPENING",
                "open-failed",
                "terminal-root-error",
                "FAILED",
                ("publish-diagnostic",),
                "registry:failed",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "BOOTSTRAPPING",
                "first-snapshot-active",
                "snapshot-valid",
                "READY",
                ("publish-readiness",),
                "registry:ready",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "BOOTSTRAPPING",
                "bootstrap-failed",
                "terminal-build-error",
                "FAILED",
                ("publish-diagnostic",),
                "registry:failed",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "READY",
                "source-degraded",
                "source-not-current",
                "DEGRADED",
                ("preserve-last-snapshot",),
                "registry:degraded",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "READY",
                "disable",
                "operator-authorized",
                "DISABLING",
                (
                    "stop-watchers",
                    "stop-providers",
                ),
                "registry:disable",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "DEGRADED",
                "source-current",
                "reconciliation-complete",
                "READY",
                ("publish-readiness",),
                "registry:ready",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "DEGRADED",
                "disable",
                "operator-authorized",
                "DISABLING",
                (
                    "stop-watchers",
                    "stop-providers",
                ),
                "registry:disable",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "DISABLING",
                "stopped",
                "no-provider-work",
                "DISABLED",
                ("persist-disabled",),
                "registry:disabled",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "REMOVING",
                "removal-complete",
                "retention-policy-applied",
                "REMOVED",
                ("retire-registration",),
                "registry:removed",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "FAILED",
                "disable",
                "operator-authorized",
                "DISABLED",
                ("persist-disabled",),
                "registry:disabled",
                "STATE_TRANSITION_VIOLATION",
            ),
            (
                "FAILED",
                "remove",
                "no-active-leases",
                "REMOVING",
                ("stop-workspace",),
                "registry:remove",
                "STATE_TRANSITION_VIOLATION",
            ),
        ),
    }
)

REGISTRY_IDS = MappingProxyType(
    {
        "entity_kinds": (
            "WORKSPACE",
            "SOURCE_FILE",
            "SOURCE_SPAN",
            "TOKEN",
            "SYNTAX_NODE",
            "PARSE_ERROR",
            "MODULE",
            "SCOPE",
            "SYMBOL",
            "DECLARATION",
            "REFERENCE",
            "CALLABLE",
            "CALL_SITE",
            "SEMANTIC_TYPE",
            "CFG_BLOCK",
            "VALUE",
            "MEMORY_LOCATION",
            "TASK",
            "THREAD",
            "EFFECT",
            "RESOURCE",
            "UNKNOWN",
            "ARTIFACT",
            "SNAPSHOT",
        ),
        "relation_kinds": (
            "CONTAINS",
            "AST_CHILD",
            "DECLARES",
            "REFERS_TO",
            "CALLS",
            "HAS_TYPE",
            "CFG_NORMAL",
            "DEF_USE",
            "POINTS_TO",
            "HAS_EFFECT",
            "USES_RESOURCE",
            "PROGRAM_ORDER_BEFORE",
            "SYNCHRONIZES_WITH_EXACT",
        ),
        "property_kinds": (
            "NAME",
            "QUALIFIED_NAME",
            "RAW_PATH",
            "CONTENT_DIGEST",
            "SPAN_START_BYTE",
            "SPAN_END_BYTE",
            "TYPE_REF",
            "VISIBILITY",
            "LANGUAGE",
            "CATEGORICAL_KIND",
        ),
        "fact_kinds": (
            "ENTITY_EXISTENCE",
            "RELATION",
            "PROPERTY",
        ),
        "unknowns": (
            "UNKNOWN_SYMBOL",
            "UNKNOWN_TYPE",
            "UNKNOWN_MODULE",
            "UNKNOWN_MEMBER",
            "UNKNOWN_CALL_TARGET",
            "UNKNOWN_EXTERNAL_IMPLEMENTATION",
            "UNKNOWN_VALUE",
            "UNKNOWN_MEMORY_LOCATION",
            "UNKNOWN_EFFECT",
            "UNKNOWN_RESOURCE",
            "UNKNOWN_FFI_TARGET",
            "UNKNOWN_CONCURRENCY_TARGET",
            "DYNAMIC_LANGUAGE_OPEN_WORLD",
            "EXTERNAL_BODY_NOT_INDEXED",
            "PROVIDER_UNAVAILABLE",
            "ANALYSIS_WIDENED",
            "REFLECTION_OR_CODE_GENERATION",
            "FFI_UNRESOLVED",
            "UNSUPPORTED_CONSTRUCT",
            "CONFLICTING_EXACT_EVIDENCE",
            "SOURCE_INVALID",
            "PROVEN_DOES_NOT_ALIAS_UNDER_PROFILE",
            "PROVEN_NO_PATH_WITHIN_PROJECTION_AND_BOUNDARY",
            "PROVEN_NOT_SUBTYPE_IN_CLOSED_TYPE_UNIVERSE",
            "PROVEN_NO_RESOLVED_MEMBER_IN_CLOSED_MEMBER_SET",
        ),
        "projections": (
            "SYNTAX_TREE_V1",
            "SYMBOL_BINDING_V1",
            "TYPE_GRAPH_V1",
            "CALL_EXACT_V1",
            "CALL_SOUND_V1",
            "CFG_NORMAL_V1",
            "CFG_FULL_V1",
            "DATAFLOW_V1",
            "ALIAS_V1",
            "OWNERSHIP_V1",
            "EFFECT_V1",
            "DEPENDENCY_V1",
            "CONCURRENCY_V1",
        ),
        "summary_profiles": ("CALLABLE_SUMMARY_BALANCED_V1",),
        "capabilities": (
            "SOURCE_BYTES",
            "SOURCE_INVENTORY",
            "TOKENS",
            "CST",
            "TYPED_AST",
            "SCOPES_BINDINGS",
            "IMPORT_RESOLUTION",
            "DECLARED_TYPES",
            "COMPUTED_TYPES",
            "MEMBER_RESOLUTION",
            "CALL_TARGETS",
            "RUST_MIR",
            "BORROW_LOANS",
            "CFG",
            "DOMINANCE",
            "CONTROL_DEPENDENCE",
            "DEF_USE",
            "LIVENESS",
            "POINTS_TO_ALIAS",
            "EFFECTS",
            "CONCURRENCY",
            "CALLABLE_SUMMARIES",
        ),
        "providers": (
            "tree-sitter",
            "ruff-python",
            "pyrefly-python",
            "rustc-mir",
            "codefabric-derivation",
            "source-substrate",
        ),
        "public_errors": (
            "INCOMPATIBLE_MAJOR",
            "UNSUPPORTED_MINOR",
            "BUNDLE_DIGEST_MISMATCH",
            "WORKSPACE_NOT_AUTHORIZED",
            "PATH_OUTSIDE_AUTHORIZED_ROOT",
            "SOURCE_ACCESS_DENIED",
            "FRESHNESS_DEADLINE_EXCEEDED",
            "CAPABILITY_UNAVAILABLE",
            "NEGATIVE_PROOF_INDETERMINATE",
            "SOURCE_SNAPSHOT_MISMATCH",
            "PROVIDER_PROTOCOL_ERROR",
            "SANDBOX_UNAVAILABLE",
            "QUERY_HARD_LIMIT_EXCEEDED",
            "ENTITY_AMBIGUOUS",
            "SEMANTIC_PHRASE_AMBIGUOUS",
            "SEMANTIC_PHRASE_UNRECOGNIZED",
            "CURRENT_POINTER_CONFLICT",
            "ID_COLLISION",
            "OVERLAY_GENERATION_CONFLICT",
            "CREDENTIAL_REPLAY_DETECTED",
            "IDEMPOTENCY_CONFLICT",
            "RESUME_WINDOW_EXPIRED",
            "RESULT_TOO_LARGE_FOR_HOST",
            "ARTIFACT_ID_COLLISION",
            "RESOURCE_EXPIRED",
            "STATE_TRANSITION_VIOLATION",
            "INTERNAL_INVARIANT_VIOLATION",
            "INVALID_REQUEST_SCHEMA",
            "CONTEXT_NOT_INDEXED",
            "COMPOSITE_SNAPSHOT_UNSUPPORTED",
            "RESOURCE_LIMIT_REJECTED",
            "CANCELLED",
            "ADAPTER_INPUT_NOT_JSON",
            "ADAPTER_INPUT_LIMIT",
            "ADAPTER_INPUT_VALIDATION",
            "ADAPTER_OUTPUT_CONTRACT",
            "UNSUPPORTED_BINARY",
            "UNSUPPORTED_CONTENT",
            "CURRENT_FACTS_UNAVAILABLE",
            "DEFAULT_CONTEXT_UNAVAILABLE",
            "WORKSPACE_BOOTSTRAPPING",
            "FACT_FAMILY_UNAVAILABLE",
            "SNAPSHOT_FRESHNESS_MISMATCH",
            "QUERY_LOST_DAEMON_RESTART",
            "COMPARISON_DOMAIN_MISMATCH",
            "SCHEMA_MISMATCH",
            "TABLE_SET_MISMATCH",
            "ROW_MISMATCH",
            "CAPABILITY_MISMATCH",
            "SNAPSHOT_METADATA_MISMATCH",
            "ID_COLLISION_DETECTED",
            "COMPARATOR_ERROR",
            "REQUIRED_FEATURE_UNSUPPORTED",
            "SCHEMA_DIGEST_MISMATCH",
            "TOOLCHAIN_MISMATCH",
            "MODEL_PACK_INCOMPATIBLE",
            "PLATFORM_UNSUPPORTED",
            "DAEMON_UNAVAILABLE",
            "CONTRACT_MISMATCH",
            "INTERNAL",
        ),
        "derivations": (),
    }
)
