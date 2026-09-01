"""Released feature-bit allocations for the private daemon RPC handshake."""

from enum import IntFlag


class CpgdFeature(IntFlag):
    """Append-only feature bits allocated by the released CPG daemon wire contract."""

    NONE = 0
    QUERY_RESUME = 1
    RESULT_RESOURCES = 2
    ZSTD_PAYLOADS = 4
    TRACE_CONTEXT = 8
    SUPPORTED = QUERY_RESUME | RESULT_RESOURCES | ZSTD_PAYLOADS | TRACE_CONTEXT
    REQUIRED = QUERY_RESUME
