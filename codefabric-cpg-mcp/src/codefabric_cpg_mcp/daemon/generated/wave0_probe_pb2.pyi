# @generated from catalog proto source set sha256:669c2280548451251fc93c0607e428f02d8fb764525932c5afe1c23c548a9c22; do not edit.
from google.protobuf import empty_pb2 as _empty_pb2
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class ProbeMode(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    PROBE_MODE_UNSPECIFIED: _ClassVar[ProbeMode]
    PROBE_MODE_ECHO: _ClassVar[ProbeMode]
PROBE_MODE_UNSPECIFIED: ProbeMode
PROBE_MODE_ECHO: ProbeMode

class ProbeEnvelope(_message.Message):
    __slots__ = ("payload", "response_bytes", "delay_millis", "trace_id", "generation", "mode", "note")
    PAYLOAD_FIELD_NUMBER: _ClassVar[int]
    RESPONSE_BYTES_FIELD_NUMBER: _ClassVar[int]
    DELAY_MILLIS_FIELD_NUMBER: _ClassVar[int]
    TRACE_ID_FIELD_NUMBER: _ClassVar[int]
    GENERATION_FIELD_NUMBER: _ClassVar[int]
    MODE_FIELD_NUMBER: _ClassVar[int]
    NOTE_FIELD_NUMBER: _ClassVar[int]
    payload: bytes
    response_bytes: int
    delay_millis: int
    trace_id: str
    generation: int
    mode: ProbeMode
    note: str
    def __init__(self, payload: _Optional[bytes] = ..., response_bytes: _Optional[int] = ..., delay_millis: _Optional[int] = ..., trace_id: _Optional[str] = ..., generation: _Optional[int] = ..., mode: _Optional[_Union[ProbeMode, str]] = ..., note: _Optional[str] = ...) -> None: ...

class ProbeMarker(_message.Message):
    __slots__ = ("value",)
    VALUE_FIELD_NUMBER: _ClassVar[int]
    value: _empty_pb2.Empty
    def __init__(self, value: _Optional[_Union[_empty_pb2.Empty, _Mapping]] = ...) -> None: ...
