"""Proof that Pyrefly includes the configured production source tree."""

import subprocess
import sys
from pathlib import Path

PROJECT = Path(__file__).resolve().parents[1]
SENTINEL = PROJECT / "src" / "codefabric_cpg_mcp" / "_pyrefly_coverage_sentinel.py"


def run_pyrefly() -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        (sys.executable, "-m", "pyrefly", "check"),
        cwd=PROJECT,
        check=False,
        capture_output=True,
        text=True,
        timeout=60,
    )


def test_pyrefly_source_inclusion_fail_then_pass() -> None:
    assert not SENTINEL.exists()
    try:
        SENTINEL.write_text('sentinel: int = "known-type-error"\n', encoding="utf-8")
        failing = run_pyrefly()
        assert failing.returncode != 0
        assert "_pyrefly_coverage_sentinel.py" in failing.stdout + failing.stderr
    finally:
        SENTINEL.unlink(missing_ok=True)

    clean = run_pyrefly()
    assert clean.returncode == 0, clean.stdout + clean.stderr
