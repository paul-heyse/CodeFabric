"""Python Protobuf interoperability and gRPC limit configuration tests."""

import importlib.metadata
import json
from collections.abc import Iterable
from pathlib import Path
from unittest.mock import patch

from google.protobuf import descriptor_pb2, descriptor_pool

from codefabric_cpg_mcp.daemon.channel import (
    GRPC_MESSAGE_OPTIONS,
    MAX_CONTROL_MESSAGE_BYTES,
    create_local_channel,
)
from codefabric_cpg_mcp.daemon.generated import wave0_probe_pb2
from codefabric_cpg_mcp.daemon.generated.wave0_probe_pb2 import ProbeEnvelope

ROOT = Path(__file__).resolve().parents[2]


def clear_derived_json_names(
    messages: Iterable[descriptor_pb2.DescriptorProto],
) -> None:
    for message in messages:
        for field in message.field:
            field.ClearField("json_name")
        clear_derived_json_names(message.nested_type)


def test_python_protobuf_matches_the_shared_wire_fixture() -> None:
    fixture = json.loads(
        (ROOT / "contracts/fixtures/proto/wave0_probe.json").read_text(encoding="utf-8")
    )
    message = ProbeEnvelope(payload=fixture["payload_utf8"].encode())

    encoded = message.SerializeToString(deterministic=True)

    assert encoded.hex() == fixture["wire_hex"]
    assert ProbeEnvelope.FromString(encoded).payload == fixture["payload_utf8"].encode()


def test_generated_descriptor_matches_the_committed_descriptor_authority() -> None:
    descriptor_set = descriptor_pb2.FileDescriptorSet.FromString(
        (ROOT / "tooling/proto/wave0-probe-descriptor.pb").read_bytes()
    )
    files = {file.name: file for file in descriptor_set.file}
    source = files["codefabric_cpg_mcp/daemon/generated/wave0_probe.proto"]

    # Python's generated descriptor omits derivable json_name fields; normalize only
    # that representation detail before comparing every semantic descriptor field.
    clear_derived_json_names(source.message_type)
    generated = descriptor_pb2.FileDescriptorProto()
    wave0_probe_pb2.DESCRIPTOR.CopyToProto(generated)
    assert generated == source
    assert wave0_probe_pb2.DESCRIPTOR.package == "codefabric.wave0.v1"

    pool = descriptor_pool.DescriptorPool()
    pool.AddSerializedFile(files["google/protobuf/empty.proto"].SerializeToString())
    loaded = pool.AddSerializedFile(source.SerializeToString())
    assert loaded.services_by_name["WaveZeroProbe"].methods_by_name["RoundTrip"].name == (
        "RoundTrip"
    )


def test_python_binary_round_trip_preserves_unknown_fields() -> None:
    known = ProbeEnvelope(payload=b"future-compatible").SerializeToString()
    unknown_field_99_varint_42 = b"\x98\x06\x2a"

    older_reader = ProbeEnvelope.FromString(known + unknown_field_99_varint_42)
    reencoded = older_reader.SerializeToString(deterministic=True)

    assert reencoded == known + unknown_field_99_varint_42


def test_exact_python_protobuf_runtime_versions_are_installed() -> None:
    assert importlib.metadata.version("grpcio") == "1.83.0"
    assert importlib.metadata.version("protobuf") == "7.36.0"


def test_python_channel_applies_symmetric_four_mib_limits() -> None:
    assert GRPC_MESSAGE_OPTIONS == (
        ("grpc.max_send_message_length", MAX_CONTROL_MESSAGE_BYTES),
        ("grpc.max_receive_message_length", MAX_CONTROL_MESSAGE_BYTES),
    )
    assert MAX_CONTROL_MESSAGE_BYTES == 4 * 1024 * 1024

    with patch("codefabric_cpg_mcp.daemon.channel.grpc.aio.insecure_channel") as create:
        channel = create_local_channel("unix:///tmp/codefabric.sock")

    create.assert_called_once_with(
        "unix:///tmp/codefabric.sock",
        options=GRPC_MESSAGE_OPTIONS,
    )
    assert channel is create.return_value
