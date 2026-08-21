"""Resolve closed catalog derivations through the Rust authority boundary."""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path
from typing import Any


def clean_environment() -> dict[str, str]:
    """Return a subprocess environment without an unrelated active uv project."""

    environment = os.environ.copy()
    environment.pop("VIRTUAL_ENV", None)
    environment.pop("UV_PROJECT_ENVIRONMENT", None)
    return environment


def resolve_derivation(root: Path, derivation_id: str) -> dict[str, Any]:
    """Resolve one typed unit, source paths, identities, outputs, and generator pin set."""

    completed = subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--locked",
            "--no-default-features",
            "--features",
            "contracts-tooling",
            "--bin",
            "codefabric-contracts",
            "--",
            "resolve-derivation",
            derivation_id,
            "--root",
            str(root),
        ],
        cwd=Path(__file__).resolve().parents[2],
        env=clean_environment(),
        check=True,
        capture_output=True,
        text=True,
    )
    value = json.loads(completed.stdout)
    if not isinstance(value, dict):
        raise TypeError("resolved derivation invocation is not an object")
    return value
