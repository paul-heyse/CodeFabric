"""Configured gRPC channel construction for the local daemon boundary."""

import grpc

MAX_CONTROL_MESSAGE_BYTES = 4 * 1024 * 1024
GRPC_DEFAULT_AUTHORITY = "localhost"
GRPC_MESSAGE_OPTIONS = (
    ("grpc.max_send_message_length", MAX_CONTROL_MESSAGE_BYTES),
    ("grpc.max_receive_message_length", MAX_CONTROL_MESSAGE_BYTES),
    ("grpc.default_authority", GRPC_DEFAULT_AUTHORITY),
)


def create_local_channel(target: str) -> grpc.aio.Channel:
    """Create the private asynchronous daemon channel with symmetric limits."""

    if not target.startswith("unix:///") or target == "unix:///" or "\x00" in target:
        raise ValueError("production daemon target must be an absolute Unix socket")
    return grpc.aio.insecure_channel(target, options=GRPC_MESSAGE_OPTIONS)
