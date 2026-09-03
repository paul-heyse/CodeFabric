"""Locked-command and STDIO-isolation tests."""

import json
import subprocess
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


def test_identity_is_stderr_only_and_exact() -> None:
    completed = subprocess.run(
        (*LOCKED_COMMAND, "--identity"),
        check=False,
        capture_output=True,
        timeout=30,
    )

    assert completed.returncode == 0, completed.stderr.decode()
    assert completed.stdout == b""
    identity = json.loads(completed.stderr)
    assert identity == {
        "adapter": "0.1.0",
        "fastmcp": "4.0.0",
        "grpcio": "1.83.0",
        "protobuf": "7.36.0",
        "pydantic": "2.13.4",
        "python": "3.14.7",
    }


def test_stdio_process_rejects_startup_without_inherited_launch_socket() -> None:
    process = subprocess.Popen(
        LOCKED_COMMAND,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert process.stdin is not None
    assert process.stdout is not None
    assert process.stderr is not None

    returncode = process.wait(timeout=30)
    stdout = process.stdout.read()
    stderr = process.stderr.read()

    assert returncode != 0
    assert stdout == b""
    assert b"adapter launch fd 3 is not a socket" in stderr
