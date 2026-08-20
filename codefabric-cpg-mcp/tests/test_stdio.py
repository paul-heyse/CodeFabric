"""Locked-command and STDIO-isolation tests."""

import json
import os
import subprocess
import time
from pathlib import Path

PROJECT = Path(__file__).resolve().parents[1]
LOCKED_COMMAND = (
    "uv",
    "run",
    "--frozen",
    "--project",
    str(PROJECT),
    "python",
    "-m",
    "codefabric_cpg_mcp",
)


def adapter_environment() -> dict[str, str]:
    environment = os.environ.copy()
    environment.update(
        {
            "CODEFABRIC_CPG_DAEMON_TARGET": "unix:///tmp/codefabric.sock",
            "CODEFABRIC_WORKSPACE_ID": "workspace-main",
            "CODEFABRIC_AGENT_INSTANCE_ID": "pytest-stdio",
            "CODEFABRIC_CPG_CAPABILITY_TOKEN": "test-secret",
        }
    )
    return environment


def test_identity_is_stderr_only_and_exact() -> None:
    completed = subprocess.run(
        (*LOCKED_COMMAND, "--identity"),
        check=False,
        capture_output=True,
        env=adapter_environment(),
        timeout=30,
    )

    assert completed.returncode == 0, completed.stderr.decode()
    assert completed.stdout == b""
    identity = json.loads(completed.stderr)
    assert identity == {
        "adapter": "0.1.0",
        "fastmcp": "3.4.7",
        "pydantic": "2.13.4",
        "pydantic-settings": "2.15.0",
        "python": "3.14.7",
    }


def test_locked_stdio_process_starts_and_exits_cleanly_without_output() -> None:
    process = subprocess.Popen(
        LOCKED_COMMAND,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=adapter_environment(),
    )
    assert process.stdin is not None
    assert process.stdout is not None
    assert process.stderr is not None

    time.sleep(0.75)
    assert process.poll() is None, process.stderr.read().decode()

    process.stdin.close()
    returncode = process.wait(timeout=30)
    stdout = process.stdout.read()
    stderr = process.stderr.read()

    assert returncode == 0, stderr.decode()
    assert stdout == b""
