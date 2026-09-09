"""Semantic comparison and source mutation helpers; independent of runtime schemas."""

from __future__ import annotations

import json
from collections.abc import Callable
from pathlib import Path


def contains(actual, expected) -> bool:
    """Match an expected fragment without dropping identity, coverage or order."""
    if isinstance(expected, dict):
        return isinstance(actual, dict) and all(
            key in actual and contains(actual[key], value)
            for key, value in expected.items()
        )
    if isinstance(expected, list):
        return (
            isinstance(actual, list)
            and len(actual) == len(expected)
            and all(contains(a, e) for a, e in zip(actual, expected))
        )
    return type(actual) is type(expected) and actual == expected


def compare(actual, expected) -> None:
    if not contains(actual, expected):
        raise AssertionError(
            f"semantic answer mismatch\nexpected={json.dumps(expected, sort_keys=True)}\nactual={json.dumps(actual, sort_keys=True)}"
        )


def apply_edit(root: Path, edit: dict) -> None:
    def path(value):
        result = (root / value).resolve()
        if not result.is_relative_to(root.resolve()) or result == root.resolve():
            raise ValueError("edit must stay within the fixture workspace")
        return result

    destination = path(edit["path"])
    if edit["operation"] == "write":
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(edit["text"])
    elif edit["operation"] == "delete":
        destination.unlink()
    elif edit["operation"] == "rename":
        destination.rename(path(edit["to"]))
    else:
        raise ValueError(f"unsupported edit: {edit['operation']}")


def clean_incremental(
    root: Path,
    edits: list[dict],
    capture: Callable,
    rebuild: Callable,
    wait_converged: Callable,
) -> None:
    """The runtime adapter supplies convergence/rebuild; no fake production hooks."""
    if not edits:
        raise ValueError("differential scenario cannot be empty")
    for edit in edits:
        apply_edit(root, edit)
        wait_converged()
        incremental = capture()
        rebuild()
        wait_converged()
        clean = capture()
        # Exact semantic comparison by default. The caller may explicitly select
        # meaningful fields, but must preserve identity relationships and coverage.
        if incremental != clean:
            raise AssertionError(f"clean/incremental mismatch after {edit}")
