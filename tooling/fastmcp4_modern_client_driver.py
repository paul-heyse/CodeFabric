"""Drive an installed CodeFabric wheel through FastMCP 4 modern STDIO.

The driver is intentionally independent from the adapter package.  It imports only the
installed FastMCP/MCP stack plus the standard library, accepts a closed JSON scenario, and emits
one closed JSON report.  Server STDERR is continuously drained into a bounded byte capture so a
noisy child cannot deadlock the MCP transport or grow the harness memory without limit.
"""

from __future__ import annotations

import asyncio
import base64
import json
import math
import os
import re
import stat
import sys
import threading
from dataclasses import dataclass, fields, is_dataclass
from importlib.metadata import PackageNotFoundError, distribution, version
from pathlib import Path
from typing import Any, NoReturn, TextIO

from fastmcp import Client
from fastmcp.client.elicitation import ElicitResult
from fastmcp.client.transports import StdioTransport
from mcp.types import (
    ElicitRequestFormParams,
    PromptReference,
    ResourceTemplateReference,
)

SCENARIO_FORMAT = "codefabric.fastmcp4-modern-client-scenario.v1"
REPORT_FORMAT = "codefabric.fastmcp4-modern-client-report.v1"
MODERN_PROTOCOL_VERSION = "2026-07-28"

MAX_SCENARIO_BYTES = 1_048_576
MAX_STEPS = 256
MAX_GUARD_RESPONSES = 64
MAX_ARGUMENTS = 128
MAX_ARGUMENT_BYTES = 4_096
MAX_STRING_BYTES = 1_048_576
MIN_TIMEOUT_SECONDS = 0.1
MAX_TIMEOUT_SECONDS = 300.0
MIN_STDERR_LIMIT_BYTES = 1_024
MAX_STDERR_LIMIT_BYTES = 262_144
MIN_REPORT_LIMIT_BYTES = 16_384
MAX_REPORT_LIMIT_BYTES = 16_777_216
STDERR_DRAIN_JOIN_SECONDS = 5.0

EXPECTED_DISTRIBUTIONS = {
    "codefabric-cpg-mcp": "0.1.0",
    "fastmcp": "4.0.0",
    "mcp": "2.1.1",
    "pydantic": "2.13.4",
}

STEP_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9_-]{0,63}\Z")
TOOL_NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}\Z")
PUBLIC_ERROR = re.compile(
    r"\b(?:[A-Z][A-Z_]{1,63}:[A-Z][A-Z_]{1,63}|[A-Z][A-Z_]{1,63})\b"
)
REFERENCE = re.compile(
    r"(?P<step>[A-Za-z0-9][A-Za-z0-9_-]{0,63})"
    r"(?P<path>(?:\.(?:[A-Za-z0-9_-]+|[0-9]+))*)\Z"
)
EXPECTED_STEP_ERRORS = {
    "CLIENT_OPERATION_FAILED",
    "CLIENT_TIMEOUT",
    "INPUT_REQUIRED_ROUNDS_EXCEEDED",
}


class DriverError(Exception):
    """Safe, classified driver failure without publicizing raw exception text."""

    def __init__(self, code: str) -> None:
        super().__init__(code)
        self.code = code


def _fail(code: str) -> NoReturn:
    raise DriverError(code)


def _reject_duplicate_members(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            _fail("SCENARIO_DUPLICATE_MEMBER")
        value[key] = item
    return value


def _reject_nonfinite(_value: str) -> NoReturn:
    _fail("SCENARIO_NONFINITE_NUMBER")


def _read_scenario(path: str) -> dict[str, Any]:
    if path == "-":
        payload = sys.stdin.buffer.read(MAX_SCENARIO_BYTES + 1)
    else:
        candidate = Path(path)
        try:
            metadata = candidate.stat()
        except OSError:
            _fail("SCENARIO_UNREADABLE")
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_size > MAX_SCENARIO_BYTES:
            _fail("SCENARIO_SIZE_INVALID")
        try:
            payload = candidate.read_bytes()
        except OSError:
            _fail("SCENARIO_UNREADABLE")
    if not payload or len(payload) > MAX_SCENARIO_BYTES:
        _fail("SCENARIO_SIZE_INVALID")
    try:
        decoded = json.loads(
            payload,
            object_pairs_hook=_reject_duplicate_members,
            parse_constant=_reject_nonfinite,
        )
    except DriverError:
        raise
    except (UnicodeError, json.JSONDecodeError):
        _fail("SCENARIO_JSON_INVALID")
    if not isinstance(decoded, dict):
        _fail("SCENARIO_ROOT_INVALID")
    return decoded


def _closed_keys(value: dict[str, Any], allowed: set[str], code: str) -> None:
    if not set(value).issubset(allowed):
        _fail(code)


def _integer(value: Any, minimum: int, maximum: int, code: str) -> int:
    if (
        isinstance(value, bool)
        or not isinstance(value, int)
        or not minimum <= value <= maximum
    ):
        _fail(code)
    return value


def _number(value: Any, minimum: float, maximum: float, code: str) -> float:
    if isinstance(value, bool) or not isinstance(value, int | float):
        _fail(code)
    result = float(value)
    if not math.isfinite(result) or not minimum <= result <= maximum:
        _fail(code)
    return result


def _string(
    value: Any, maximum_bytes: int, code: str, *, allow_empty: bool = False
) -> str:
    if (
        not isinstance(value, str)
        or (not value and not allow_empty)
        or len(value.encode("utf-8")) > maximum_bytes
    ):
        _fail(code)
    return value


def _json_value(value: Any, *, depth: int = 0) -> None:
    if depth > 64:
        _fail("SCENARIO_JSON_DEPTH_EXCEEDED")
    if value is None or isinstance(value, bool | int | str):
        if isinstance(value, str) and len(value.encode("utf-8")) > MAX_STRING_BYTES:
            _fail("SCENARIO_STRING_TOO_LARGE")
        return
    if isinstance(value, float):
        if not math.isfinite(value):
            _fail("SCENARIO_NONFINITE_NUMBER")
        return
    if isinstance(value, list):
        if len(value) > 4_096:
            _fail("SCENARIO_COLLECTION_TOO_LARGE")
        for item in value:
            _json_value(item, depth=depth + 1)
        return
    if isinstance(value, dict):
        if len(value) > 4_096 or not all(isinstance(key, str) for key in value):
            _fail("SCENARIO_COLLECTION_INVALID")
        for key, item in value.items():
            _json_value(key, depth=depth + 1)
            _json_value(item, depth=depth + 1)
        return
    _fail("SCENARIO_VALUE_INVALID")


@dataclass(frozen=True)
class GuardResponse:
    message: str
    action: str
    content: Any
    requested_schema: dict[str, Any] | None


@dataclass(frozen=True)
class ServerCommand:
    command: str
    arguments: tuple[str, ...]
    cwd: str | None


@dataclass(frozen=True)
class Scenario:
    server: ServerCommand
    timeout_seconds: float
    input_required_max_rounds: int
    stderr_limit_bytes: int
    report_limit_bytes: int
    guard_responses: tuple[GuardResponse, ...]
    require_all_guard_responses: bool
    steps: tuple[dict[str, Any], ...]


def _validate_server(raw: Any) -> ServerCommand:
    if not isinstance(raw, dict):
        _fail("SCENARIO_SERVER_INVALID")
    _closed_keys(raw, {"command", "args", "cwd"}, "SCENARIO_SERVER_UNKNOWN_MEMBER")
    command = Path(
        _string(raw.get("command"), MAX_ARGUMENT_BYTES, "SCENARIO_COMMAND_INVALID")
    )
    if (
        not command.is_absolute()
        or not command.is_file()
        or not os.access(command, os.X_OK)
    ):
        _fail("SCENARIO_COMMAND_INVALID")
    arguments = raw.get("args", [])
    if not isinstance(arguments, list) or len(arguments) > MAX_ARGUMENTS:
        _fail("SCENARIO_ARGUMENTS_INVALID")
    checked_arguments: list[str] = []
    for argument in arguments:
        checked = _string(argument, MAX_ARGUMENT_BYTES, "SCENARIO_ARGUMENT_INVALID")
        if "\0" in checked:
            _fail("SCENARIO_ARGUMENT_INVALID")
        checked_arguments.append(checked)
    cwd_raw = raw.get("cwd")
    cwd: str | None = None
    if cwd_raw is not None:
        cwd_path = Path(_string(cwd_raw, MAX_ARGUMENT_BYTES, "SCENARIO_CWD_INVALID"))
        if not cwd_path.is_absolute() or not cwd_path.is_dir():
            _fail("SCENARIO_CWD_INVALID")
        cwd = str(cwd_path)
    return ServerCommand(str(command), tuple(checked_arguments), cwd)


def _validate_guards(raw: Any) -> tuple[GuardResponse, ...]:
    if not isinstance(raw, list) or len(raw) > MAX_GUARD_RESPONSES:
        _fail("SCENARIO_GUARDS_INVALID")
    responses: list[GuardResponse] = []
    for entry in raw:
        if not isinstance(entry, dict):
            _fail("SCENARIO_GUARD_INVALID")
        _closed_keys(
            entry,
            {"message", "action", "content", "requested_schema"},
            "SCENARIO_GUARD_UNKNOWN_MEMBER",
        )
        message = _string(entry.get("message"), 1_024, "SCENARIO_GUARD_MESSAGE_INVALID")
        action = entry.get("action", "accept")
        if action not in {"accept", "decline", "cancel"}:
            _fail("SCENARIO_GUARD_ACTION_INVALID")
        content = entry.get("content")
        if action == "accept" and not isinstance(content, dict):
            _fail("SCENARIO_GUARD_CONTENT_INVALID")
        if action != "accept" and content is not None:
            _fail("SCENARIO_GUARD_CONTENT_INVALID")
        _json_value(content)
        requested_schema = entry.get("requested_schema")
        if requested_schema is not None and not isinstance(requested_schema, dict):
            _fail("SCENARIO_GUARD_SCHEMA_INVALID")
        _json_value(requested_schema)
        responses.append(GuardResponse(message, action, content, requested_schema))
    return tuple(responses)


def _validate_steps(raw: Any) -> tuple[dict[str, Any], ...]:
    if not isinstance(raw, list) or not 1 <= len(raw) <= MAX_STEPS:
        _fail("SCENARIO_STEPS_INVALID")
    identifiers: set[str] = set()
    checked: list[dict[str, Any]] = []
    allowed_by_operation = {
        "discover": {"id", "operation"},
        "call_tool": {"id", "operation", "name", "arguments", "timeout_seconds"},
        "read_resource": {"id", "operation", "uri", "expect_error"},
        "complete": {
            "id",
            "operation",
            "reference",
            "argument",
            "context_arguments",
            "expect_error",
        },
        "cancel_tool": {
            "id",
            "operation",
            "name",
            "arguments",
            "cancel_after_ms",
            "cancel_on_progress",
            "timeout_seconds",
        },
        "sleep": {"id", "operation", "duration_ms"},
    }
    for entry in raw:
        if not isinstance(entry, dict):
            _fail("SCENARIO_STEP_INVALID")
        identifier = entry.get("id")
        operation = entry.get("operation")
        if not isinstance(identifier, str) or STEP_ID.fullmatch(identifier) is None:
            _fail("SCENARIO_STEP_ID_INVALID")
        if identifier in identifiers:
            _fail("SCENARIO_STEP_ID_DUPLICATE")
        identifiers.add(identifier)
        if not isinstance(operation, str) or operation not in allowed_by_operation:
            _fail("SCENARIO_OPERATION_INVALID")
        _closed_keys(
            entry, allowed_by_operation[operation], "SCENARIO_STEP_UNKNOWN_MEMBER"
        )
        if operation in {"call_tool", "cancel_tool"}:
            name = entry.get("name")
            if not isinstance(name, str) or TOOL_NAME.fullmatch(name) is None:
                _fail("SCENARIO_TOOL_NAME_INVALID")
            arguments = entry.get("arguments", {})
            if not isinstance(arguments, dict):
                _fail("SCENARIO_TOOL_ARGUMENTS_INVALID")
            if "timeout_seconds" in entry:
                _number(
                    entry["timeout_seconds"],
                    MIN_TIMEOUT_SECONDS,
                    MAX_TIMEOUT_SECONDS,
                    "SCENARIO_STEP_TIMEOUT_INVALID",
                )
        if operation == "read_resource" and "uri" not in entry:
            _fail("SCENARIO_RESOURCE_URI_INVALID")
        if operation == "complete":
            reference = entry.get("reference")
            argument = entry.get("argument")
            if not isinstance(reference, dict) or not isinstance(argument, dict):
                _fail("SCENARIO_COMPLETION_INVALID")
            if entry.get("context_arguments") is not None and not isinstance(
                entry.get("context_arguments"), dict
            ):
                _fail("SCENARIO_COMPLETION_CONTEXT_INVALID")
        if operation == "cancel_tool":
            cancel_on_progress = entry.get("cancel_on_progress", False)
            if not isinstance(cancel_on_progress, bool):
                _fail("SCENARIO_CANCEL_TRIGGER_INVALID")
            if cancel_on_progress:
                if "cancel_after_ms" in entry:
                    _fail("SCENARIO_CANCEL_TRIGGER_INVALID")
            else:
                _integer(
                    entry.get("cancel_after_ms"),
                    0,
                    60_000,
                    "SCENARIO_CANCEL_DELAY_INVALID",
                )
        if operation == "sleep":
            _integer(entry.get("duration_ms"), 0, 60_000, "SCENARIO_SLEEP_INVALID")
        expected_error = entry.get("expect_error")
        if expected_error is not None and expected_error not in EXPECTED_STEP_ERRORS:
            _fail("SCENARIO_EXPECTED_ERROR_INVALID")
        _json_value(entry)
        checked.append(entry)
    return tuple(checked)


def _validate_scenario(raw: dict[str, Any]) -> Scenario:
    _closed_keys(
        raw,
        {
            "format",
            "server",
            "timeout_seconds",
            "input_required_max_rounds",
            "stderr_limit_bytes",
            "report_limit_bytes",
            "guard_responses",
            "require_all_guard_responses",
            "steps",
        },
        "SCENARIO_UNKNOWN_MEMBER",
    )
    if raw.get("format") != SCENARIO_FORMAT:
        _fail("SCENARIO_FORMAT_INVALID")
    require_all = raw.get("require_all_guard_responses", True)
    if not isinstance(require_all, bool):
        _fail("SCENARIO_GUARD_REQUIREMENT_INVALID")
    return Scenario(
        server=_validate_server(raw.get("server")),
        timeout_seconds=_number(
            raw.get("timeout_seconds", 120.0),
            MIN_TIMEOUT_SECONDS,
            MAX_TIMEOUT_SECONDS,
            "SCENARIO_TIMEOUT_INVALID",
        ),
        input_required_max_rounds=_integer(
            raw.get("input_required_max_rounds", 10),
            1,
            32,
            "SCENARIO_GUARD_ROUND_LIMIT_INVALID",
        ),
        stderr_limit_bytes=_integer(
            raw.get("stderr_limit_bytes", 65_536),
            MIN_STDERR_LIMIT_BYTES,
            MAX_STDERR_LIMIT_BYTES,
            "SCENARIO_STDERR_LIMIT_INVALID",
        ),
        report_limit_bytes=_integer(
            raw.get("report_limit_bytes", 8_388_608),
            MIN_REPORT_LIMIT_BYTES,
            MAX_REPORT_LIMIT_BYTES,
            "SCENARIO_REPORT_LIMIT_INVALID",
        ),
        guard_responses=_validate_guards(raw.get("guard_responses", [])),
        require_all_guard_responses=require_all,
        steps=_validate_steps(raw.get("steps")),
    )


class BoundedStderrCapture:
    """Continuously drain child STDERR while retaining only a fixed byte prefix."""

    def __init__(self, limit: int) -> None:
        self.limit = limit
        self.total_bytes = 0
        self._captured = bytearray()
        self._writer: TextIO | None = None
        self._thread: threading.Thread | None = None
        self._reader_error = False

    def start(self) -> TextIO:
        read_fd, write_fd = os.pipe()
        self._writer = os.fdopen(
            write_fd, "w", encoding="utf-8", errors="strict", buffering=1
        )
        self._thread = threading.Thread(
            target=self._drain,
            args=(read_fd,),
            name="codefabric-wp47-stderr-drain",
            daemon=True,
        )
        self._thread.start()
        return self._writer

    def _drain(self, read_fd: int) -> None:
        try:
            with os.fdopen(read_fd, "rb", buffering=0) as reader:
                while chunk := reader.read(65_536):
                    self.total_bytes += len(chunk)
                    remaining = self.limit - len(self._captured)
                    if remaining > 0:
                        self._captured.extend(chunk[:remaining])
        except OSError:
            self._reader_error = True

    def finish(self) -> dict[str, Any]:
        if self._writer is not None and not self._writer.closed:
            self._writer.close()
        if self._thread is not None:
            self._thread.join(STDERR_DRAIN_JOIN_SECONDS)
            if self._thread.is_alive():
                _fail("STDERR_DRAIN_TIMEOUT")
        if self._reader_error:
            _fail("STDERR_DRAIN_FAILED")
        raw = bytes(self._captured)
        try:
            text = raw.decode("utf-8")
            utf8_valid = True
        except UnicodeDecodeError:
            text = None
            utf8_valid = False
        return {
            "captured_base64": base64.b64encode(raw).decode("ascii"),
            "captured_text": text,
            "captured_bytes": len(raw),
            "total_bytes": self.total_bytes,
            "truncated": self.total_bytes > len(raw),
            "utf8_valid": utf8_valid,
        }


class GuardDriver:
    def __init__(self, responses: tuple[GuardResponse, ...]) -> None:
        self._responses = responses
        self._consumed: set[int] = set()
        self._lock = asyncio.Lock()
        self.observations: list[dict[str, Any]] = []
        self.unexpected = False

    async def __call__(
        self,
        message: str,
        _response_type: Any,
        params: Any,
        _context: Any,
    ) -> ElicitResult[Any]:
        requested_schema = (
            params.requested_schema
            if isinstance(params, ElicitRequestFormParams)
            else None
        )
        async with self._lock:
            selected: tuple[int, GuardResponse] | None = None
            for index, response in enumerate(self._responses):
                if index in self._consumed or response.message != message:
                    continue
                if (
                    response.requested_schema is not None
                    and response.requested_schema != requested_schema
                ):
                    continue
                selected = (index, response)
                break
            if selected is None or not isinstance(params, ElicitRequestFormParams):
                self.unexpected = True
                self.observations.append(
                    {
                        "action": "cancel",
                        "matched": False,
                        "message": message,
                        "requested_schema": requested_schema,
                    }
                )
                return ElicitResult(action="cancel")
            index, response = selected
            self._consumed.add(index)
            self.observations.append(
                {
                    "action": response.action,
                    "matched": True,
                    "message": message,
                    "requested_schema": requested_schema,
                }
            )
            return ElicitResult(
                action=response.action,
                content=_materialize_guard_content(response.content, requested_schema),
            )

    @property
    def all_consumed(self) -> bool:
        return len(self._consumed) == len(self._responses)


def _materialize_guard_content(content: Any, requested_schema: Any) -> Any:
    """Resolve a bounded scenario marker from the live daemon-authored input schema."""

    if isinstance(content, dict):
        if set(content) == {"$requested_schema_presentation"}:
            label = _string(
                content["$requested_schema_presentation"],
                256,
                "SCENARIO_GUARD_LABEL_INVALID",
            )
            try:
                field = requested_schema["properties"]["value"]
                presentations = field["x-codefabric-choice-presentations"]
                candidates = [
                    value
                    for value in field["enum"]
                    if presentations.get(value) == label
                ]
            except (KeyError, TypeError, AttributeError):
                _fail("GUARD_ENUM_SCHEMA_MISSING")
            if len(candidates) != 1:
                _fail("GUARD_ENUM_PRESENTATION_AMBIGUOUS_OR_MISSING")
            return candidates[0]
        if set(content) == {"$requested_schema_enum"}:
            index = _integer(
                content["$requested_schema_enum"],
                0,
                MAX_GUARD_RESPONSES - 1,
                "SCENARIO_GUARD_ENUM_INDEX_INVALID",
            )
            try:
                values = requested_schema["properties"]["value"]["enum"]
            except (KeyError, TypeError):
                _fail("GUARD_ENUM_SCHEMA_MISSING")
            if not isinstance(values, list) or index >= len(values):
                _fail("GUARD_ENUM_SCHEMA_MISSING")
            selected = values[index]
            _json_value(selected)
            return selected
        return {
            key: _materialize_guard_content(value, requested_schema)
            for key, value in content.items()
        }
    if isinstance(content, list):
        return [
            _materialize_guard_content(value, requested_schema) for value in content
        ]
    return content


def _resolve_reference(reference: str, outputs: dict[str, Any]) -> Any:
    match = REFERENCE.fullmatch(reference)
    if match is None or match.group("step") not in outputs:
        _fail("SCENARIO_REFERENCE_INVALID")
    value = outputs[match.group("step")]
    path = match.group("path")
    for component in path.lstrip(".").split(".") if path else ():
        if isinstance(value, dict) and component in value:
            value = value[component]
        elif (
            isinstance(value, list)
            and component.isdigit()
            and int(component) < len(value)
        ):
            value = value[int(component)]
        else:
            _fail("SCENARIO_REFERENCE_UNRESOLVED")
    return value


def _resolve(value: Any, outputs: dict[str, Any]) -> Any:
    if isinstance(value, dict):
        if set(value) == {"$ref"}:
            reference = value["$ref"]
            if not isinstance(reference, str):
                _fail("SCENARIO_REFERENCE_INVALID")
            return _resolve_reference(reference, outputs)
        return {key: _resolve(item, outputs) for key, item in value.items()}
    if isinstance(value, list):
        return [_resolve(item, outputs) for item in value]
    return value


def _model_json(value: Any) -> Any:
    dump = getattr(value, "model_dump", None)
    if callable(dump):
        return _model_json(dump(mode="json", by_alias=False, exclude_none=True))
    if is_dataclass(value) and not isinstance(value, type):
        return {
            field.name: _model_json(getattr(value, field.name))
            for field in fields(value)
            if getattr(value, field.name) is not None
        }
    if isinstance(value, dict):
        return {str(key): _model_json(item) for key, item in value.items()}
    if isinstance(value, list | tuple):
        return [_model_json(item) for item in value]
    _json_value(value)
    return value


async def _discover(client: Client[Any]) -> dict[str, Any]:
    discover = client.session.discover_result
    if discover is None or client.session.protocol_version != MODERN_PROTOCOL_VERSION:
        _fail("MODERN_PROTOCOL_NOT_NEGOTIATED")
    tools, resources, templates, prompts = await asyncio.gather(
        client.list_tools(),
        client.list_resources(),
        client.list_resource_templates(),
        client.list_prompts(),
    )
    return {
        "protocol_version": client.session.protocol_version,
        "supported_versions": list(discover.supported_versions),
        "capabilities": _model_json(discover.capabilities),
        "tools": [_model_json(item) for item in tools],
        "resources": [_model_json(item) for item in resources],
        "resource_templates": [_model_json(item) for item in templates],
        "prompts": [_model_json(item) for item in prompts],
    }


async def _execute_step(
    client: Client[Any],
    step: dict[str, Any],
    outputs: dict[str, Any],
    default_timeout: float,
) -> Any:
    operation = step["operation"]
    resolved = _resolve(step, outputs)
    if operation == "discover":
        return await _discover(client)
    if operation == "call_tool":
        arguments = resolved.get("arguments", {})
        if not isinstance(arguments, dict):
            _fail("SCENARIO_TOOL_ARGUMENTS_INVALID")
        result = await client.call_tool(
            resolved["name"],
            arguments,
            timeout=resolved.get("timeout_seconds", default_timeout),
        )
        return _model_json(result)
    if operation == "read_resource":
        uri = _string(resolved["uri"], 8_192, "SCENARIO_RESOURCE_URI_INVALID")
        return [_model_json(item) for item in await client.read_resource(uri)]
    if operation == "complete":
        reference = resolved["reference"]
        argument = resolved["argument"]
        if not isinstance(reference, dict) or not isinstance(argument, dict):
            _fail("SCENARIO_COMPLETION_INVALID")
        _closed_keys(reference, {"kind", "uri", "name"}, "SCENARIO_COMPLETION_INVALID")
        kind = reference.get("kind")
        if kind == "resource":
            if set(reference) != {"kind", "uri"}:
                _fail("SCENARIO_COMPLETION_INVALID")
            ref = ResourceTemplateReference(
                uri=_string(reference.get("uri"), 8_192, "SCENARIO_COMPLETION_INVALID")
            )
        elif kind == "prompt":
            if set(reference) != {"kind", "name"}:
                _fail("SCENARIO_COMPLETION_INVALID")
            ref = PromptReference(
                name=_string(reference.get("name"), 128, "SCENARIO_COMPLETION_INVALID")
            )
        else:
            _fail("SCENARIO_COMPLETION_INVALID")
        _closed_keys(
            argument, {"name", "value"}, "SCENARIO_COMPLETION_ARGUMENT_INVALID"
        )
        context_arguments = resolved.get("context_arguments")
        if context_arguments is not None and not isinstance(context_arguments, dict):
            _fail("SCENARIO_COMPLETION_CONTEXT_INVALID")
        completion = await client.complete(
            ref,
            {
                "name": _string(
                    argument.get("name"), 128, "SCENARIO_COMPLETION_ARGUMENT_INVALID"
                ),
                "value": _string(
                    argument.get("value"),
                    4_096,
                    "SCENARIO_COMPLETION_ARGUMENT_INVALID",
                    allow_empty=True,
                ),
            },
            context_arguments=context_arguments,
        )
        return _model_json(completion)
    if operation == "cancel_tool":
        arguments = resolved.get("arguments", {})
        if not isinstance(arguments, dict):
            _fail("SCENARIO_TOOL_ARGUMENTS_INVALID")
        progress_seen = asyncio.Event()

        async def progress_handler(
            _progress: float,
            _total: float | None,
            _message: str | None,
        ) -> None:
            progress_seen.set()

        cancel_on_progress = resolved.get("cancel_on_progress", False)
        task = asyncio.create_task(
            client.call_tool(
                resolved["name"],
                arguments,
                timeout=resolved.get("timeout_seconds", default_timeout),
                progress_handler=progress_handler if cancel_on_progress else None,
            )
        )
        if cancel_on_progress:
            await asyncio.wait_for(progress_seen.wait(), timeout=default_timeout)
        else:
            await asyncio.sleep(resolved["cancel_after_ms"] / 1_000)
        task.cancel()
        try:
            await task
        except asyncio.CancelledError:
            return {
                "cancelled": True,
                "trigger": "progress" if cancel_on_progress else "timer",
            }
        _fail("CLIENT_CANCELLATION_NOT_OBSERVED")
    if operation == "sleep":
        await asyncio.sleep(resolved["duration_ms"] / 1_000)
        return {"slept_ms": resolved["duration_ms"]}
    _fail("SCENARIO_OPERATION_INVALID")


def _verify_installed_stack() -> dict[str, str]:
    observed: dict[str, str] = {}
    for name, expected in EXPECTED_DISTRIBUTIONS.items():
        try:
            observed_version = version(name)
        except PackageNotFoundError:
            _fail("INSTALLED_STACK_INCOMPLETE")
        if observed_version != expected:
            _fail("INSTALLED_STACK_VERSION_MISMATCH")
        observed[name] = observed_version
    adapter = distribution("codefabric-cpg-mcp")
    direct_url = adapter.read_text("direct_url.json")
    if direct_url:
        try:
            origin = json.loads(direct_url, object_pairs_hook=_reject_duplicate_members)
        except (DriverError, json.JSONDecodeError):
            _fail("INSTALLED_WHEEL_PROVENANCE_INVALID")
        directory_origin = origin.get("dir_info") if isinstance(origin, dict) else None
        if (
            not isinstance(origin, dict)
            or (directory_origin is not None and not isinstance(directory_origin, dict))
            or (
                isinstance(directory_origin, dict)
                and bool(directory_origin.get("editable", False))
            )
        ):
            _fail("INSTALLED_WHEEL_REQUIRED")
    if any(
        name == "codefabric_cpg_mcp" or name.startswith("codefabric_cpg_mcp.")
        for name in sys.modules
    ):
        _fail("SOURCE_PACKAGE_IMPORT_FORBIDDEN")
    return observed


def _error_code(error: BaseException) -> str:
    if isinstance(error, DriverError):
        return error.code
    if isinstance(error, TimeoutError | asyncio.TimeoutError):
        return "CLIENT_TIMEOUT"
    if error.__class__.__name__ == "InputRequiredRoundsExceededError":
        return "INPUT_REQUIRED_ROUNDS_EXCEEDED"
    return "CLIENT_OPERATION_FAILED"


def _public_error(error: BaseException) -> str | None:
    """Retain only the daemon's bounded public status/code pair from an exception."""
    match = PUBLIC_ERROR.search(str(error))
    return match.group(0) if match is not None else None


async def _run(scenario: Scenario, stack: dict[str, str]) -> tuple[dict[str, Any], int]:
    capture = BoundedStderrCapture(scenario.stderr_limit_bytes)
    guard = GuardDriver(scenario.guard_responses)
    outputs: dict[str, Any] = {}
    steps: list[dict[str, Any]] = []
    error_code: str | None = None
    failure_step: str | None = None
    failure_class: str | None = None
    failure_public_error: str | None = None
    writer = capture.start()
    transport = StdioTransport(
        command=scenario.server.command,
        args=list(scenario.server.arguments),
        cwd=scenario.server.cwd,
        keep_alive=False,
        log_file=writer,
    )
    try:
        async with Client(
            transport,
            mode="auto",
            elicitation_handler=guard,
            timeout=scenario.timeout_seconds,
            init_timeout=scenario.timeout_seconds,
            input_required_max_rounds=scenario.input_required_max_rounds,
            cache=False,
        ) as client:
            if client.session.protocol_version != MODERN_PROTOCOL_VERSION:
                _fail("MODERN_PROTOCOL_NOT_NEGOTIATED")
            for step in scenario.steps:
                failure_step = step["id"]
                expected_error = step.get("expect_error")
                try:
                    result = await _execute_step(
                        client, step, outputs, scenario.timeout_seconds
                    )
                except Exception as error:
                    observed_error = _error_code(error)
                    if observed_error != expected_error:
                        raise
                    result = {"error_code": observed_error}
                else:
                    if expected_error is not None:
                        _fail("EXPECTED_STEP_ERROR_NOT_OBSERVED")
                outputs[step["id"]] = result
                steps.append(
                    {"id": step["id"], "operation": step["operation"], "result": result}
                )
                if guard.unexpected:
                    _fail("UNEXPECTED_GUARD_REQUEST")
                failure_step = None
            if scenario.require_all_guard_responses and not guard.all_consumed:
                _fail("EXPECTED_GUARD_NOT_OBSERVED")
    except Exception as error:  # noqa: BLE001 - boundary redacts arbitrary library errors
        error_code = _error_code(error)
        failure_class = error.__class__.__name__
        failure_public_error = _public_error(error)
    finally:
        try:
            await transport.close()
        except Exception as error:  # noqa: BLE001 - teardown failures are classified
            if error_code is None:
                error_code = _error_code(error)
    try:
        stderr = capture.finish()
    except Exception as error:  # noqa: BLE001 - capture failures are classified
        stderr = {
            "captured_base64": "",
            "captured_text": None,
            "captured_bytes": 0,
            "total_bytes": 0,
            "truncated": False,
            "utf8_valid": False,
        }
        if error_code is None:
            error_code = _error_code(error)
    report = {
        "format": REPORT_FORMAT,
        "status": "ok" if error_code is None else "error",
        "error_code": error_code,
        "failure_step": failure_step,
        "failure_class": failure_class,
        "failure_public_error": failure_public_error,
        "protocol_mode": MODERN_PROTOCOL_VERSION,
        "stack": stack,
        "steps": steps,
        "guard_observations": guard.observations,
        "stderr": stderr,
    }
    return report, 0 if error_code is None else 1


def _emit(report: dict[str, Any], limit: int) -> bool:
    payload = json.dumps(
        report,
        ensure_ascii=True,
        allow_nan=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    if len(payload) > limit:
        fallback = {
            "error_code": "REPORT_SIZE_EXCEEDED",
            "format": REPORT_FORMAT,
            "protocol_mode": MODERN_PROTOCOL_VERSION,
            "status": "error",
        }
        payload = json.dumps(fallback, sort_keys=True, separators=(",", ":")).encode(
            "ascii"
        )
        within_limit = False
    else:
        within_limit = True
    sys.stdout.buffer.write(payload + b"\n")
    sys.stdout.buffer.flush()
    return within_limit


def main() -> int:
    if len(sys.argv) != 2:
        report = {
            "error_code": "DRIVER_ARGUMENTS_INVALID",
            "format": REPORT_FORMAT,
            "protocol_mode": MODERN_PROTOCOL_VERSION,
            "status": "error",
        }
        _emit(report, MIN_REPORT_LIMIT_BYTES)
        return 2
    report_limit = MIN_REPORT_LIMIT_BYTES
    failure_stage = "scenario-read"
    try:
        raw_scenario = _read_scenario(sys.argv[1])
        failure_stage = "scenario-validate"
        scenario = _validate_scenario(raw_scenario)
        report_limit = scenario.report_limit_bytes
        failure_stage = "installed-stack"
        stack = _verify_installed_stack()
        failure_stage = "client"
        report, status = asyncio.run(_run(scenario, stack))
    except Exception as error:  # noqa: BLE001 - top-level emits a closed failure report
        code = _error_code(error)
        report = {
            "error_code": code,
            "failure_class": error.__class__.__name__,
            "failure_stage": failure_stage,
            "format": REPORT_FORMAT,
            "protocol_mode": MODERN_PROTOCOL_VERSION,
            "status": "error",
        }
        status = 1
    if not _emit(report, report_limit):
        return 1
    if status != 0:
        sys.stderr.write(f"{report.get('error_code', 'CLIENT_DRIVER_FAILED')}\n")
    return status


if __name__ == "__main__":
    raise SystemExit(main())
