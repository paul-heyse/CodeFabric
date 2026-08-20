# @generated from codefabric_cpg_mcp/daemon/generated/wave0_probe.proto sha256:b8eeb2a402f703124442ba9d67687c65e33027348b88d895ab56c39e817985cc; do not edit.
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from typing import ClassVar as _ClassVar, Optional as _Optional

DESCRIPTOR: _descriptor.FileDescriptor

class ProbeEnvelope(_message.Message):
    __slots__ = ("payload", "response_bytes")
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    RESPONSE_BYTES_FIELD_NUMBER: _ClassVar[int]
    payload: bytes
    response_bytes: int
    def __init__(self, payload: _Optional[bytes] = ..., response_bytes: _Optional[int] = ...) -> None: ...
