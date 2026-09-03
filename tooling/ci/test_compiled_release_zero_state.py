"""Fault-seeded tests for the target-v7 compiled-release zero state."""

from __future__ import annotations

from pathlib import Path

import pytest

from tooling.ci.compiled_release_zero_state import (
    LEGACY_TOKEN_CLASSES,
    ROOT,
    CompiledReleaseZeroStateError,
    validate_compiled_release_zero_state,
)
from tooling.ci.fastmcp4_post_purge_assurance import EXPECTED_RUNTIME_DEPENDENCIES
from tooling.ci.test_fastmcp4_post_purge_assurance import _minimal_root, _write


def test_compiled_release_legacy_zero_state() -> None:
    report = validate_compiled_release_zero_state(ROOT)
    assert int(report["scanned_live_files"]) > 0
    assert report["live_matches"] == report["unreadable"] == report["unparsed"] == 0


@pytest.mark.parametrize(("category", "token"), sorted(LEGACY_TOKEN_CLASSES.items()))
def test_every_compiled_release_legacy_class_is_detected(
    tmp_path: Path, category: str, token: str
) -> None:
    root = _minimal_root(tmp_path)
    _write(root, "src/legacy.rs", f"// {category}: {token}\n")
    with pytest.raises(CompiledReleaseZeroStateError) as failure:
        validate_compiled_release_zero_state(root)
    assert failure.value.code == "CFV7_RELEASE_LEGACY"


def test_old_mixed_provider_lane_type_is_detected(tmp_path: Path) -> None:
    root = _minimal_root(tmp_path)
    _write(root, "src/legacy.rs", "struct CompiledProviderLane;\n")
    with pytest.raises(CompiledReleaseZeroStateError) as failure:
        validate_compiled_release_zero_state(root)
    assert failure.value.code == "CFV7_RELEASE_LEGACY"


def test_transitive_dependency_version_is_not_suite_authority(tmp_path: Path) -> None:
    root = _minimal_root(tmp_path)
    lock = root / "codefabric-cpg-mcp/uv.lock"
    lock.write_text(
        lock.read_text(encoding="utf-8")
        + '[[package]]\nname = "fixture-transitive"\nversion = "2.2.0"\n',
        encoding="utf-8",
    )
    report = validate_compiled_release_zero_state(root)
    assert tuple(EXPECTED_RUNTIME_DEPENDENCIES)
    assert report["live_matches"] == 0
