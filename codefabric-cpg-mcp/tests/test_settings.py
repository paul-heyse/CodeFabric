"""Target-only launch authority tests: strict fd3 records, never environment claims."""

from __future__ import annotations

import json
import os
import socket
import sys
import time
from pathlib import Path

import pytest
from pydantic import ValidationError

import codefabric_cpg_mcp.settings as settings_module
from codefabric_cpg_mcp.settings import Settings, next_settings


def _record(generation: int = 1) -> dict[str, object]:
    return {
        "format": "codefabric.adapter-launch.v1",
        "query_socket": "/tmp/codefabric-query.sock",
        "launch_grant_hex": "ab" * 32,
        "adapter_program": str(Path(sys.executable).resolve()),
        "adapter_arguments": ["-m", "codefabric_cpg_mcp"],
        "daemon_generation": generation,
        "supervisor_generation": 41,
        "session_expires_at_unix_ms": 4_000_000_000_000,
        "maximum_request_state_ttl_seconds": 30,
    }


def test_launch_record_is_strict_frozen_and_secret() -> None:
    settings = Settings.model_validate_json(json.dumps(_record()), strict=True)

    assert settings.daemon_target == "unix:///tmp/codefabric-query.sock"
    assert settings.launch_grant.get_secret_value() == bytes.fromhex("ab" * 32)
    assert "abababab" not in repr(settings)
    assert "abababab" not in repr(settings.model_dump())
    assert "abababab" not in settings.model_dump_json()
    assert settings.model_dump_json().count("**********") == 1
    with pytest.raises(ValidationError):
        Settings.model_validate_json(
            json.dumps({**_record(), "workspace_id": "caller-claim"}), strict=True
        )
    with pytest.raises(ValidationError):
        Settings.model_validate_json(
            json.dumps({**_record(), "daemon_generation": "1"}), strict=True
        )
    with pytest.raises(ValidationError):
        Settings.model_validate_json(
            json.dumps({**_record(), "query_socket": "relative.sock"}), strict=True
        )
    with pytest.raises(ValidationError):
        settings.daemon_generation = 2  # type: ignore[misc]


def test_retained_socket_stream_delivers_generation_replacement() -> None:
    reader, writer = socket.socketpair()
    writer.sendall(json.dumps(_record(1)).encode() + b"\n")
    writer.sendall(json.dumps(_record(2)).encode() + b"\n")
    descriptor = reader.detach()
    try:
        first = settings_module._read_launch_descriptor(descriptor)
        second = next_settings()
    finally:
        writer.close()

    assert first.daemon_generation == 1
    assert second.daemon_generation == 2
    assert first.supervisor_generation == second.supervisor_generation
    assert first.session_expires_at_unix_ms == second.session_expires_at_unix_ms
    with pytest.raises(RuntimeError, match="framing"):
        next_settings()


def test_launch_descriptor_rejects_non_socket_and_invalid_framing() -> None:
    read_fd, write_fd = os.pipe()
    try:
        with pytest.raises(RuntimeError, match="not a socket"):
            settings_module._read_launch_descriptor(read_fd)
    finally:
        os.close(read_fd)
        os.close(write_fd)

    reader, writer = socket.socketpair()
    writer.sendall(b'{"format":"not-complete"}')
    writer.close()
    descriptor = reader.detach()
    with pytest.raises(RuntimeError, match="framing"):
        settings_module._read_launch_descriptor(descriptor)


def test_replacement_read_is_bounded_when_supervisor_sends_no_complete_record() -> None:
    reader, writer = socket.socketpair()
    writer.sendall(b'{"format":"partial')
    descriptor = reader.detach()
    started = time.monotonic()
    try:
        with pytest.raises(RuntimeError, match="timed out"):
            settings_module._read_launch_descriptor(descriptor, timeout_seconds=0.05)
    finally:
        writer.close()

    assert time.monotonic() - started < 1.0


def test_environment_cannot_supply_launch_authority(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("CODEFABRIC_CPG_CAPABILITY_TOKEN", "legacy-secret")
    monkeypatch.setenv("CODEFABRIC_WORKSPACE_ID", "caller-workspace")

    with pytest.raises(ValidationError):
        Settings.model_validate({}, strict=True)
