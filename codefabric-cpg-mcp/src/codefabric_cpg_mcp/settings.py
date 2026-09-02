"""Strict read-once bootstrap settings delivered only on inherited fd 3."""

from __future__ import annotations

import os
import select
import socket
import stat
import threading
import time
from functools import lru_cache
from pathlib import Path
from typing import Annotated

from pydantic import (
    BaseModel,
    ConfigDict,
    Field,
    SecretBytes,
    SecretStr,
    ValidationError,
    field_validator,
)

_LAUNCH_FD = 3
_MAX_LAUNCH_BYTES = 64 * 1024
_DEFAULT_LAUNCH_READ_TIMEOUT_SECONDS = 10.0
_launch_socket: socket.socket | None = None
_launch_buffer = bytearray()
_launch_lock = threading.Lock()


class Settings(BaseModel):
    """Frozen adapter launch authority; no environment variable can supply it."""

    model_config = ConfigDict(
        extra="forbid",
        strict=True,
        frozen=True,
        validate_default=True,
        hide_input_in_errors=True,
    )

    format: Annotated[str, Field(pattern=r"^codefabric\.adapter-launch\.v1$")]
    query_socket: Path
    launch_grant_hex: SecretStr
    adapter_program: Path
    adapter_arguments: tuple[str, ...]
    daemon_generation: Annotated[int, Field(gt=0)]
    supervisor_generation: Annotated[int, Field(gt=0)]
    session_expires_at_unix_ms: Annotated[int, Field(gt=0)]
    maximum_request_state_ttl_seconds: Annotated[int, Field(gt=0, le=60)]

    query_timeout_seconds: Annotated[float, Field(gt=0, le=300)] = 120.0
    readiness_timeout_seconds: Annotated[float, Field(gt=0, le=30)] = 10.0
    cancellation_cleanup_timeout_seconds: Annotated[float, Field(gt=0, le=10)] = 2.0
    maximum_resource_chunk_bytes: Annotated[int, Field(ge=16 * 1024, le=4 * 1024 * 1024)] = (
        1024 * 1024
    )

    @field_validator("query_socket", "adapter_program")
    @classmethod
    def absolute_non_nul_path(cls, value: Path) -> Path:
        if not value.is_absolute() or "\x00" in os.fspath(value):
            raise ValueError("launch paths must be absolute and NUL-free")
        return value

    @field_validator("launch_grant_hex")
    @classmethod
    def exact_lower_hex_grant(cls, value: SecretStr) -> SecretStr:
        encoded = value.get_secret_value()
        if len(encoded) != 64 or any(character not in "0123456789abcdef" for character in encoded):
            raise ValueError("launch grant must be exactly 32 lowercase hexadecimal bytes")
        return value

    @property
    def daemon_target(self) -> str:
        """grpcio's absolute Unix-domain target for the supervisor-selected endpoint."""

        return f"unix://{self.query_socket}"

    @property
    def launch_grant(self) -> SecretBytes:
        """Decode the single-use capability without exposing it in model rendering."""

        return SecretBytes(bytes.fromhex(self.launch_grant_hex.get_secret_value()))


def _read_launch_descriptor(
    fd: int = _LAUNCH_FD,
    *,
    timeout_seconds: float = _DEFAULT_LAUNCH_READ_TIMEOUT_SECONDS,
) -> Settings:
    """Retain a duplicate socket and close the inherited fixed-number descriptor."""

    try:
        metadata = os.fstat(fd)
    except OSError as error:
        raise RuntimeError("adapter launch fd 3 is not a socket") from error
    if not stat.S_ISSOCK(metadata.st_mode):
        raise RuntimeError("adapter launch fd 3 is not a socket")
    global _launch_socket
    duplicate = os.dup(fd)
    os.close(fd)
    retained = socket.socket(fileno=duplicate)
    with _launch_lock:
        if _launch_socket is not None:
            retained.close()
            raise RuntimeError("adapter launch socket is already retained")
        _launch_socket = retained
        _launch_buffer.clear()
    return next_settings(timeout_seconds=timeout_seconds)


def next_settings(*, timeout_seconds: float = _DEFAULT_LAUNCH_READ_TIMEOUT_SECONDS) -> Settings:
    """Read one bounded generation-labelled record from the retained socket."""

    if not 0 < timeout_seconds <= 30:
        raise ValueError("launch record timeout must be in (0, 30] seconds")
    deadline = time.monotonic() + timeout_seconds
    if not _launch_lock.acquire(timeout=timeout_seconds):
        raise RuntimeError("adapter launch descriptor read timed out")
    try:
        retained = _launch_socket
        if retained is None:
            raise RuntimeError("adapter launch socket is unavailable")
        while True:
            newline = _launch_buffer.find(b"\n")
            if newline >= 0:
                if newline + 1 > _MAX_LAUNCH_BYTES:
                    raise RuntimeError("adapter launch descriptor framing is invalid")
                payload = bytes(_launch_buffer[: newline + 1])
                del _launch_buffer[: newline + 1]
                break
            if len(_launch_buffer) > _MAX_LAUNCH_BYTES:
                raise RuntimeError("adapter launch descriptor framing is invalid")
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise RuntimeError("adapter launch descriptor read timed out")
            try:
                readable, _, _ = select.select([retained], [], [], remaining)
                if not readable:
                    raise RuntimeError("adapter launch descriptor read timed out")
                chunk = retained.recv(8192)
            except OSError as error:
                raise RuntimeError("adapter launch descriptor read failed") from error
            if not chunk:
                raise RuntimeError("adapter launch descriptor framing is invalid")
            _launch_buffer.extend(chunk)
    finally:
        _launch_lock.release()
    if not payload or len(payload) > _MAX_LAUNCH_BYTES or not payload.endswith(b"\n"):
        raise RuntimeError("adapter launch descriptor framing is invalid")
    try:
        return Settings.model_validate_json(payload, strict=True)
    except ValidationError as error:
        raise RuntimeError("adapter launch descriptor is not valid strict JSON") from error


@lru_cache(maxsize=1)
def process_settings() -> Settings:
    """Consume and retain the sole process-lifetime launch descriptor."""

    return _read_launch_descriptor()


__all__ = ["Settings", "next_settings", "process_settings"]
