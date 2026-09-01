"""Causal tests for the WP39 disposition and package-boundary oracles."""

from __future__ import annotations

import json
import shutil
from pathlib import Path

import pytest

from tooling.ci.post_purge_assurance import (
    EXPECTED_CARGO_MANIFESTS,
    LEDGER_PATH,
    PLAN_PATH,
    PYTHON_MANIFEST,
    ROOT,
    PostPurgeAssuranceError,
    validate_disposition_integrity,
    validate_package_inventory,
    validate_retained_recipe_contract,
)


def _copy_file(root: Path, path: str | Path) -> None:
    relative = Path(path)
    destination = root / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT / relative, destination)


def _copy_disposition_contract(root: Path) -> Path:
    for path in (LEDGER_PATH, PLAN_PATH, Path("justfile")):
        _copy_file(root, path)
    return root


def _copy_package_contract(root: Path) -> Path:
    for path in (
        *sorted(EXPECTED_CARGO_MANIFESTS),
        "Cargo.lock",
        "rustc-extractor/Cargo.lock",
        "pyrefly-sidecar/Cargo.lock",
        PYTHON_MANIFEST,
        "codefabric-cpg-mcp/uv.lock",
    ):
        _copy_file(root, path)
    # Discovery reports every skipped directory class rather than silently pruning it.
    (root / "target").mkdir(parents=True)
    return root


def _load_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict)
    return value


def _write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def test_wp39_int_live_disposition_ledger_covers_exact_plan() -> None:
    report = validate_disposition_integrity(ROOT)
    assert report["l_dispositions"] == 36
    assert report["db_dispositions"] == 5
    assert report["history_exclusion_classes"] == 3
    assert report["oracles"] == 4


def test_wp39_int_missing_l_disposition_is_rejected(tmp_path: Path) -> None:
    root = _copy_disposition_contract(tmp_path / "repo")
    plan_path = root / PLAN_PATH
    lines = plan_path.read_text(encoding="utf-8").splitlines()
    plan_path.write_text(
        "\n".join(line for line in lines if not line.startswith("| L-20 |")) + "\n",
        encoding="utf-8",
    )
    with pytest.raises(PostPurgeAssuranceError, match="L disposition coverage differs"):
        validate_disposition_integrity(root)


def test_wp39_int_history_exclusion_drift_is_rejected(tmp_path: Path) -> None:
    root = _copy_disposition_contract(tmp_path / "repo")
    path = root / LEDGER_PATH
    ledger = _load_json(path)
    exclusions = ledger["history_exclusions"]
    assert isinstance(exclusions, dict)
    retained = exclusions["retained_history_globs"]
    assert isinstance(retained, list)
    retained.pop()
    _write_json(path, ledger)
    with pytest.raises(PostPurgeAssuranceError, match="history exclusions differ"):
        validate_disposition_integrity(root)


def test_wp39_beh_recipe_keeps_every_retained_target_consumer() -> None:
    report = validate_retained_recipe_contract(ROOT)
    assert report == {
        "retained_behavior_dependencies": 5,
        "package_operation_dependencies": 7,
    }


def test_wp39_ops_live_four_domain_package_inventory_is_exact() -> None:
    report = validate_package_inventory(ROOT)
    assert len(report["production_build_domains"]) == 4
    assert len(report["assurance_cargo_roots"]) == 2
    assert len(report["root_features"]) == 12
    assert len(report["root_binaries"]) == 3
    assert len(report["locks"]) == 4


def test_wp39_ops_unclassified_cargo_root_is_rejected(tmp_path: Path) -> None:
    root = _copy_package_contract(tmp_path / "repo")
    path = root / "hidden-domain/Cargo.toml"
    path.parent.mkdir(parents=True)
    path.write_text(
        '[package]\nname = "hidden-domain"\nversion = "0.0.0"\n'
        'edition = "2024"\npublish = false\n',
        encoding="utf-8",
    )
    with pytest.raises(
        PostPurgeAssuranceError, match="Cargo manifest inventory differs"
    ):
        validate_package_inventory(root)


def test_wp39_ops_unclassified_root_feature_is_rejected(tmp_path: Path) -> None:
    root = _copy_package_contract(tmp_path / "repo")
    path = root / "Cargo.toml"
    manifest = path.read_text(encoding="utf-8")
    manifest = manifest.replace(
        "[features]\n",
        "[features]\nstale-authority = []\n",
        1,
    )
    path.write_text(manifest, encoding="utf-8")
    with pytest.raises(PostPurgeAssuranceError, match="root feature inventory differs"):
        validate_package_inventory(root)
