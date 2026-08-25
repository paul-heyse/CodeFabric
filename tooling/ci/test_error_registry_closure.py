from __future__ import annotations

from pathlib import Path

import pytest

from tooling.ci.error_registry_closure import check


def _fixture(tmp_path: Path, error_name: str) -> Path:
    (tmp_path / "src").mkdir()
    (tmp_path / "rustc-extractor" / "src").mkdir(parents=True)
    (tmp_path / "contracts" / "registry").mkdir(parents=True)
    (tmp_path / "src" / "lib.rs").write_text(
        f'#[derive(thiserror::Error)]\nenum Error {{ #[error("{error_name}:detail")] Value }}',
        encoding="utf-8",
    )
    (tmp_path / "contracts" / "registry" / "error-registry.yaml").write_text(
        "records:\n  - {name: REGISTERED}\n",
        encoding="utf-8",
    )
    return tmp_path


def test_accepts_registry_member(tmp_path: Path) -> None:
    result = check(_fixture(tmp_path, "REGISTERED"))
    assert result["unregistered_public_errors"] == []


def test_rejects_shadow_public_vocabulary(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="SHADOW"):
        check(_fixture(tmp_path, "SHADOW"))
