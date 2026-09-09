"""Bounded child capture and process-group cleanup for development runners."""

from __future__ import annotations

import os
import signal
import subprocess
import threading
import time
from dataclasses import dataclass
from pathlib import Path


@dataclass
class Outcome:
    command: list[str]
    returncode: int
    elapsed_seconds: float
    stdout: str
    stderr: str
    timed_out: bool
    output_exceeded: bool


def run(
    command: list[str],
    *,
    cwd: Path,
    timeout: float = 240,
    maximum_bytes: int = 16_777_216,
) -> Outcome:
    if not command or timeout <= 0 or maximum_bytes <= 0:
        raise ValueError("nonempty command and positive bounds required")
    started = time.monotonic()
    process = subprocess.Popen(
        command,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    captures = [bytearray(), bytearray()]
    exceeded = threading.Event()

    def drain(stream, capture):
        with stream:
            while chunk := stream.read(8192):
                capture.extend(chunk)
                if len(capture) > maximum_bytes:
                    del capture[:-maximum_bytes]
                    exceeded.set()

    threads = [
        threading.Thread(target=drain, args=pair, daemon=True)
        for pair in zip((process.stdout, process.stderr), captures)
    ]
    for thread in threads:
        thread.start()
    timed_out = False
    try:
        while process.poll() is None:
            timed_out = time.monotonic() - started >= timeout
            if timed_out or exceeded.is_set():
                break
            time.sleep(0.02)
    finally:
        # Kill the entire group even after the immediate child exits: descendants
        # may still own pipes or a daemon. No owned background processes survive.
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            pass
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait()
        for thread in threads:
            thread.join(timeout=3)
        if any(thread.is_alive() for thread in threads):
            raise RuntimeError("child output drain did not terminate")
    code = 124 if timed_out else 125 if exceeded.is_set() else process.returncode
    return Outcome(
        command,
        code,
        time.monotonic() - started,
        *(bytes(c).decode(errors="replace") for c in captures),
        timed_out,
        exceeded.is_set(),
    )
