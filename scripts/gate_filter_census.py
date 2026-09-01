#!/usr/bin/env python3
"""Render and validate name-coupled cargo-nextest selectors in the justfile."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
JUSTFILE = ROOT / "justfile"
MANIFEST = ROOT / "tooling/ci/gate-filter-census.json"
RECIPE = re.compile(r"^([a-zA-Z0-9_-]+)(?:\s+[^:]*)?:\s*$")


class GateFilterCensusError(ValueError):
    """The committed selector census differs from the operational API."""


def render_census(justfile_text: str) -> dict[str, Any]:
    """Return every recipe command whose nextest selection depends on test names."""
    recipe: str | None = None
    selectors: list[dict[str, str]] = []
    for raw_line in justfile_text.splitlines():
        match = RECIPE.fullmatch(raw_line)
        if match is not None:
            recipe = match.group(1)
            continue
        command = raw_line.strip()
        if (
            recipe is None
            or "cargo nextest run" not in command
            or " -E " not in command
        ):
            continue
        selectors.append({"recipe": recipe, "command": " ".join(command.split())})
    if not selectors:
        raise GateFilterCensusError(
            "justfile contains no name-coupled nextest selectors"
        )
    return {
        "artifact_id": "codefabric.governance.gate-filter-census",
        "schema_version": 1,
        "selectors": selectors,
    }


def validate_census(
    manifest: dict[str, Any], justfile_text: str, *, require_no_tests_fail: bool = True
) -> tuple[int, int]:
    """Require exact live/committed parity and zero-selection failure semantics."""
    expected_keys = {"artifact_id", "schema_version", "selectors"}
    if set(manifest) != expected_keys:
        raise GateFilterCensusError("gate-filter census root keys are invalid")
    if (
        manifest.get("artifact_id") != "codefabric.governance.gate-filter-census"
        or manifest.get("schema_version") != 1
        or not isinstance(manifest.get("selectors"), list)
    ):
        raise GateFilterCensusError("gate-filter census identity is invalid")
    live = render_census(justfile_text)
    if manifest != live:
        raise GateFilterCensusError(
            "committed gate-filter census differs from justfile"
        )
    selectors = manifest["selectors"]
    recipes = {entry["recipe"] for entry in selectors}
    if require_no_tests_fail:
        missing = sorted(
            entry["recipe"]
            for entry in selectors
            if "--no-tests=fail" not in entry["command"]
        )
        if missing:
            raise GateFilterCensusError(
                f"name-coupled selectors lack --no-tests=fail: {missing}"
            )
    return len(recipes), len(selectors)


def load_manifest(path: Path = MANIFEST) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise GateFilterCensusError("gate-filter census must be a JSON object")
    return value


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("check", "render"))
    args = parser.parse_args()
    justfile_text = JUSTFILE.read_text(encoding="utf-8")
    if args.command == "render":
        json.dump(render_census(justfile_text), sys.stdout, indent=2)
        sys.stdout.write("\n")
        return 0
    recipes, selectors = validate_census(load_manifest(), justfile_text)
    print(f"gate-filter census passed: {recipes} recipes, {selectors} selectors")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
