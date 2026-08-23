"""Write-boundary tests for review-only fixture candidates."""

from pathlib import Path

import pytest

from tooling.model.fixture_candidates import (
    NORMATIVE_FIXTURE_ROOT,
    _isolated_output_directory,
    emit_candidates,
)


def test_normative_fixture_destinations_are_rejected() -> None:
    with pytest.raises(ValueError, match="outside normative"):
        _isolated_output_directory(str(NORMATIVE_FIXTURE_ROOT / "candidate"))


def test_candidates_write_only_to_an_empty_review_directory(tmp_path: Path) -> None:
    output = _isolated_output_directory(str(tmp_path / "review"))
    paths = emit_candidates(output)

    assert {path.name for path in paths} == {
        "jcs-candidates.json",
        "projection-candidates.json",
    }
    assert all(path.parent == output for path in paths)
    with pytest.raises(ValueError, match="absent or empty"):
        _isolated_output_directory(str(output))
