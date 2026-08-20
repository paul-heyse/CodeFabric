"""Python Protobuf interoperability and gRPC limit configuration tests."""

import json
from pathlib import Path
from unittest.mock import patch

from codefabric_cpg_mcp.daemon.channel import (
    GRPC_MESSAGE_OPTIONS,
    MAX_CONTROL_MESSAGE_BYTES,
    create_local_channel,
)
from codefabric_cpg_mcp.daemon.generated.wave0_probe_pb2 import ProbeEnvelope

ROOT = Path(__file__).resolve().parents[2]


def test_python_protobuf_matches_the_shared_wire_fixture() -> None:
    fixture = json.loads(
        (ROOT / "contracts/fixtures/proto/wave0_probe.json").read_text(encoding="utf-8")
    )
    message = ProbeEnvelope(payload=fixture["payload_utf8"].encode())

    encoded = message.SerializeToString(deterministic=True)

    assert encoded.hex() == fixture["wire_hex"]
    assert ProbeEnvelope.FromString(encoded).payload == fixture["payload_utf8"].encode()


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
