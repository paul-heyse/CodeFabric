# @generated from catalog primary semantic identity b3:e277cbb865a7d6b6f43d017e3d0522b9381210add57ef7ab7c67e5ef7936803f,b3:600bc0072c5a938c6ae1e7818d42c837d7141ddbebf3a0c732a2da13f943f47c,b3:5ba59c314ad97b338ebf612a4cd9a4158e6b0e4531c1497a2af8e1c4a35fe1b0,b3:69b7abee0fcf580e76582178a07811a628af6ddf59ff227fa3543356652fec5c,b3:19aafdb1017ec7847d5c7229e0d68794d1083fc5c480b5dab3247eb3f782111b; do not edit.
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
