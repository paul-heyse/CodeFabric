"""Regression tests for the name-coupled Just recipe census."""

from __future__ import annotations

from scripts.gate_filter_census import render_census


def test_dependency_bearing_recipes_own_their_nextest_selectors() -> None:
    rendered = render_census(
        """
base:
    cargo nextest run --locked --lib -E 'test(base)' --no-tests=fail

governed-check: base
    cargo nextest run --locked --lib -E 'test(governed)' --no-tests=fail

parameterized-check value='default': governed-check
    cargo nextest run --locked --lib -E 'test(parameterized)' --no-tests=fail
""".strip()
    )

    assert [entry["recipe"] for entry in rendered["selectors"]] == [
        "base",
        "governed-check",
        "parameterized-check",
    ]
