from __future__ import annotations


def normalized_total(values: list[int]) -> int:
    """Return a deterministic total for the golden provider/query path."""
    return sum(sorted(values))
