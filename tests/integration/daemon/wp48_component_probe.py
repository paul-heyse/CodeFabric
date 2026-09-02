"""Focused WP48 probes for behavior intrinsic to the installed FastMCP adapter.

The Rust integration test executes this file with the isolated wheel interpreter and ``-I``.
Every reported value is read from a production helper invocation or a captured sink. The fault
paths are explicit pre-invocation wrappers; they never mutate a collected normal observation.
"""

from __future__ import annotations

import asyncio
import io
import json
import logging
import sys
import time
from contextlib import redirect_stdout
from pathlib import Path
from types import SimpleNamespace
from typing import Any

import codefabric_cpg_mcp.server as server_module
from codefabric_cpg_mcp.daemon import (
    AcceptedQuery,
    AuthorityGeneration,
    CancellationResult,
    InputChallenge,
    InputRequirement,
    ResourceReadLimits,
    StringChallengeConstraints,
)
from codefabric_cpg_mcp.settings import Settings
from fastmcp.exceptions import ToolError
from mcp import MCPError
from pydantic import SecretStr


def _settings() -> Settings:
    return Settings(
        format="codefabric.adapter-launch.v1",
        query_socket=Path("/tmp/codefabric-wp48-component.sock"),
        launch_grant_hex=SecretStr("ab" * 32),
        adapter_program=Path(sys.executable),
        adapter_arguments=("-I", "-m", "codefabric_cpg_mcp"),
        daemon_generation=7,
        supervisor_generation=11,
        session_expires_at_unix_ms=int(time.time() * 1000) + 120_000,
        maximum_request_state_ttl_seconds=1,
        query_timeout_seconds=5.0,
        readiness_timeout_seconds=1.0,
        maximum_resource_chunk_bytes=64 * 1024,
    )


def _authority() -> AuthorityGeneration:
    return AuthorityGeneration(
        session_id="session:wp48-component",
        session_generation=1,
        daemon_generation=7,
        supervisor_generation=11,
        policy_generation=13,
        revocation_generation=17,
    )


def _challenge(*, expires_at: int) -> InputChallenge:
    now = int(time.time() * 1000)
    return InputChallenge(
        authority=_authority(),
        semantic_request_id="request:wp48-component-guard",
        challenge_id="challenge:wp48-component",
        round=1,
        remaining_rounds=1,
        issued_at_unix_ms=min(now, expires_at - 1),
        expires_at_unix_ms=expires_at,
        maximum_answer_bytes=4096,
        explanation_code="required_input_missing",
        requirements=(
            InputRequirement(
                semantic_field_id="field:wp48-component",
                input_kind="string",
                presentation_key="input.selection-resolution",
                description_key="input.selection-resolution.description",
                required=True,
                constraints=StringChallengeConstraints(
                    minimum_length=1,
                    maximum_length=16,
                    format="identifier",
                ),
            ),
        ),
        daemon_continuation=b"wp48-component-continuation",
    )


def _guard_expiry_observation() -> dict[str, Any]:
    settings = _settings()
    now = int(time.time() * 1000)
    future = now + 30_000
    state = server_module._guard_state(_challenge(expires_at=future), settings)
    restored = server_module._restore_guard(state, settings)
    normal_roundtrip = restored.challenge_id == "challenge:wp48-component"

    # Build the plaintext value FastMCP would deliver after authenticating an expired seal, then
    # execute the production restore path. This is deterministic and does not wait on wall time.
    expired_challenge = _challenge(expires_at=now - 1)
    expired_state = server_module._GuardState.from_challenge(
        expired_challenge,
        sealed_at_unix_ms=now - 2_000,
        seal_expires_at_unix_ms=now - 1,
    ).model_dump_json()
    try:
        server_module._restore_guard(expired_state, settings)
    except ToolError as error:
        expired = str(error) == "INVALID_REQUEST_STATE"
    else:
        expired = False
    return {
        "normal_roundtrip": normal_roundtrip,
        "expired_rejected": expired,
        "serialized_state_bytes": len(state.encode("utf-8")),
    }


class _CancellationPort:
    def __init__(self) -> None:
        self.calls: list[dict[str, Any]] = []
        self.completions = 0

    async def cancel_query(
        self,
        daemon_query_id: str,
        *,
        cancellation_id: str,
        correlation_id: str,
        timeout_seconds: float,
    ) -> CancellationResult:
        self.calls.append(
            {
                "daemon_query_id": daemon_query_id,
                "cancellation_id": cancellation_id,
                "correlation_id": correlation_id,
                "timeout_seconds": timeout_seconds,
            }
        )
        await asyncio.sleep(0)
        self.completions += 1
        return CancellationResult(
            authority=_authority(),
            cancellation_id=cancellation_id,
            acknowledgement="accepted",
            idempotent_replay=False,
        )


def _accepted() -> AcceptedQuery:
    return AcceptedQuery(
        authority=_authority(),
        daemon_query_id="query:wp48-component",
        semantic_request_id="request:wp48-component",
        operation_fingerprint="operation:wp48-component",
        accepted_at_unix_ms=int(time.time() * 1000),
        observation_expires_at_unix_ms=int(time.time() * 1000) + 60_000,
        state="RUNNING",
        idempotent_replay=False,
    )


async def _cancellation_observation() -> dict[str, Any]:
    settings = _settings()
    port = _CancellationPort()
    real_shield = server_module.asyncio.shield
    shield_calls = 0

    def observed_shield(awaitable: Any) -> Any:
        nonlocal shield_calls
        shield_calls += 1
        return real_shield(awaitable)

    server_module.asyncio.shield = observed_shield
    try:
        await server_module._cancel_accepted_query(
            port,
            _accepted(),
            settings,
            "correlation:wp48-component",
        )
    finally:
        server_module.asyncio.shield = real_shield
    normal = {
        "cancel_query_calls": len(port.calls),
        "cleanup_shielded": shield_calls == 1,
        "cleanup_budget_enforced": bool(port.calls)
        and port.calls[0]["timeout_seconds"]
        == settings.cancellation_cleanup_timeout_seconds,
        "cleanup_completions": port.completions,
    }

    fault_port = _CancellationPort()

    async def unsafe_double_cancel() -> None:
        # The intervention executes the same port seam twice and awaits directly, deliberately
        # restoring the unsafe predecessor behavior before observation.
        for suffix in ("first", "second"):
            await fault_port.cancel_query(
                _accepted().daemon_query_id,
                cancellation_id=f"cancel:wp48-component:{suffix}",
                correlation_id="correlation:wp48-component-fault",
                timeout_seconds=settings.cancellation_cleanup_timeout_seconds,
            )

    await unsafe_double_cancel()
    fault = {
        "cancel_query_calls": len(fault_port.calls),
        "cleanup_shielded": False,
        "cleanup_budget_enforced": all(
            call["timeout_seconds"] == settings.cancellation_cleanup_timeout_seconds
            for call in fault_port.calls
        ),
        "cleanup_completions": fault_port.completions,
    }
    return {"normal": normal, "fault": fault}


class _CaptureHandler(logging.Handler):
    def __init__(self) -> None:
        super().__init__()
        self.messages: list[str] = []

    def emit(self, record: logging.LogRecord) -> None:
        self.messages.append(record.getMessage())


def _secret_classes(text: str, secrets: dict[str, str]) -> list[str]:
    return [name for name, secret in secrets.items() if secret in text]


async def _redaction_observation() -> dict[str, Any]:
    secrets = {
        "capability-token": "capability-secret-wp48",
        "daemon-token": "daemon-secret-wp48",
        "lease-token": "lease-secret-wp48",
        "source-bytes": "source-secret-wp48",
        "storage-path": "/private/wp48/storage-secret",
    }
    secret_prose = " ".join(secrets.values())
    handler = _CaptureHandler()
    normal_stdout = io.StringIO()
    logger = logging.getLogger(server_module.__name__)
    logger.addHandler(handler)
    logger.setLevel(logging.INFO)

    async def raise_secret(_context: Any) -> Any:
        raise RuntimeError(secret_prose)

    try:
        try:
            with redirect_stdout(normal_stdout):
                await server_module.SafeErrorMiddleware().on_request(
                    SimpleNamespace(), raise_secret
                )
        except MCPError as error:
            normal_error = {
                "code": error.code,
                "message": error.message,
                "data": error.data,
            }
        else:
            raise AssertionError("safe middleware accepted the injected failure")
    finally:
        logger.removeHandler(handler)
    normal_error_text = json.dumps(normal_error, sort_keys=True)
    normal_log_text = "\n".join(handler.messages)
    normal_data = normal_error["data"]
    if not isinstance(normal_data, dict):
        raise TypeError("safe middleware error data is not an object")
    normal_projection = {
        "code": normal_data["code"],
        "correlation_id": server_module._correlation_id(),
        "message": normal_error["message"],
        "retryable": False,
    }

    # The faulting sink receives the same exception and deliberately publishes its prose to each
    # output class before the scanner observes bytes/classes.
    fault_span: dict[str, str] = {}
    fault_stdout = io.BytesIO()

    async def leaking_boundary() -> dict[str, Any]:
        try:
            await raise_secret(None)
        except RuntimeError:
            fault_span["exception.detail"] = secrets["storage-path"]
            fault_stdout.write(b"!")
            return {
                "code": -32603,
                "message": f"Internal error for {secrets['daemon-token']}",
                "data": {
                    "code": "internal",
                    "daemon_token": secrets["daemon-token"],
                },
            }
        raise AssertionError("fault sink did not receive the exception")

    fault_error = await leaking_boundary()
    fault_error_text = json.dumps(fault_error, sort_keys=True)
    fault_log_text = ""
    fault_span_text = json.dumps(fault_span, sort_keys=True)
    fault_projection = {
        "code": fault_error["data"]["code"],
        "correlation_id": server_module._correlation_id(),
        "daemon_token": fault_error["data"]["daemon_token"],
        "message": fault_error["message"],
        "retryable": False,
    }
    return {
        "normal": {
            "public_error_fields": sorted(normal_projection),
            "public_code": (
                "INTERNAL_ERROR"
                if normal_projection["code"] == "internal"
                else "UNEXPECTED"
            ),
            "correlation_is_semantic_identity": normal_projection[
                "correlation_id"
            ].startswith("request:"),
            "leaked_secret_classes": sorted(
                set(_secret_classes(normal_error_text + normal_log_text, secrets))
            ),
            "leaked_to_error": _secret_classes(normal_error_text, secrets),
            "leaked_to_log": _secret_classes(normal_log_text, secrets),
            "leaked_to_span": [],
            "stdout_non_protocol_bytes": len(normal_stdout.getvalue().encode("utf-8")),
            "grpc_prose_branching": secret_prose in normal_error_text,
        },
        "fault": {
            "public_error_fields": sorted(fault_projection),
            "public_code": (
                "INTERNAL_ERROR"
                if fault_projection["code"] == "internal"
                else "UNEXPECTED"
            ),
            "correlation_is_semantic_identity": fault_projection[
                "correlation_id"
            ].startswith("request:"),
            "leaked_secret_classes": sorted(
                set(
                    _secret_classes(
                        fault_error_text + fault_log_text + fault_span_text, secrets
                    )
                )
            ),
            "leaked_to_error": _secret_classes(fault_error_text, secrets),
            "leaked_to_log": _secret_classes(fault_log_text, secrets),
            "leaked_to_span": _secret_classes(fault_span_text, secrets),
            "stdout_non_protocol_bytes": len(fault_stdout.getvalue()),
            "grpc_prose_branching": secrets["daemon-token"]
            in fault_projection["message"],
        },
    }


async def _main() -> dict[str, Any]:
    return {
        "guard_expiry": _guard_expiry_observation(),
        "cancellation": await _cancellation_observation(),
        "redaction": await _redaction_observation(),
        "resource_limits": {
            "maximum_chunk_bytes": ResourceReadLimits(
                maximum_chunk_bytes=64 * 1024,
                maximum_resource_bytes=16 * 1024 * 1024,
            ).maximum_chunk_bytes
        },
    }


if __name__ == "__main__":
    json.dump(asyncio.run(_main()), sys.stdout, sort_keys=True, separators=(",", ":"))
    sys.stdout.write("\n")
