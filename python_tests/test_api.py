"""Public Python API behavior.

These tests import from the public package, never from ``codefabric._native`` (spec
section 19.2), which keeps the raw extension free to evolve internally.

They deliberately do not replay the Rust suite's cases. Rust owns domain validation;
what needs proving here is that the interface accepts expected Python values, converts
them correctly in both directions, and maps errors as documented (spec section 19.1).
"""

import pytest

import codefabric


def test_version_is_a_non_empty_string() -> None:
    assert isinstance(codefabric.__version__, str)
    assert codefabric.__version__


@pytest.mark.parametrize(
    ("raw", "expected"),
    [
        ("codefabric", "codefabric"),
        ("  codefabric  ", "codefabric"),
        ("\t codefabric \n", "codefabric"),
        ("two words", "two words"),
    ],
)
def test_workspace_id_is_normalized(raw: str, expected: str) -> None:
    assert codefabric.normalize_workspace_id(raw) == expected


@pytest.mark.parametrize("raw", ["", "   ", "\t\n"])
def test_blank_workspace_id_raises_value_error(raw: str) -> None:
    """The Rust ``Error::EmptyField`` maps to ``ValueError`` at the binding boundary."""
    with pytest.raises(ValueError, match="workspace_id must not be empty"):
        codefabric.normalize_workspace_id(raw)
