from __future__ import annotations

import json
import shutil
from pathlib import Path

import pytest
import yaml
from codefabric_cpg_mcp.contracts.json import canonicalize_value, checksum

from tooling.ci.semantic_provider_legacy_zero_state import (
    REGISTRY,
    SemanticProviderLegacyError,
    check,
)

ROOT = Path(__file__).resolve().parents[2]


def _seal(document: dict[str, object]) -> None:
    detached = dict(document)
    detached.pop("canonical_digest", None)
    document["canonical_digest"] = checksum(canonicalize_value(detached))


def _fixture(tmp_path: Path) -> tuple[Path, dict[str, object]]:
    document = yaml.safe_load((ROOT / REGISTRY).read_text(encoding="utf-8"))
    paths = {allow["path"] for allow in document["allows"]}
    for relative in paths:
        source = ROOT / relative
        target = tmp_path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    state_path = Path(document["active_state_path"])
    (tmp_path / state_path.parent).mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT / state_path, tmp_path / state_path)
    (tmp_path / REGISTRY.parent).mkdir(parents=True, exist_ok=True)
    (tmp_path / REGISTRY).write_text(
        yaml.safe_dump(document, sort_keys=False), encoding="utf-8"
    )
    return tmp_path, document


def test_current_reviewed_transition_surface_is_exact() -> None:
    result = check()
    assert result["candidate_count"] == 6
    assert result["open_candidate_count"] == 4
    assert check("python")["open_candidate_count"] == 2
    assert check("rust")["open_candidate_count"] == 2


def test_rejects_unexpected_direct_provider_consumer(tmp_path: Path) -> None:
    root, _ = _fixture(tmp_path)
    (root / "src" / "rogue.rs").write_text(
        "fn rogue() { run_pyrefly(); }\n", encoding="utf-8"
    )
    with pytest.raises(SemanticProviderLegacyError, match="rogue.rs"):
        check("python", root)


def test_rejects_stale_reviewed_allow(tmp_path: Path) -> None:
    root, _ = _fixture(tmp_path)
    path = root / "src" / "fabric" / "serving.rs"
    path.write_text("// cut over\n", encoding="utf-8")
    with pytest.raises(SemanticProviderLegacyError, match="stale reviewed allows"):
        check("python", root)


def test_rejects_allow_after_expiry_packet_completes(tmp_path: Path) -> None:
    root, document = _fixture(tmp_path)
    state_path = root / str(document["active_state_path"])
    state = json.loads(state_path.read_text(encoding="utf-8"))
    state["packets"]["WP08"]["status"] = "complete"
    state_path.write_text(json.dumps(state), encoding="utf-8")
    with pytest.raises(
        SemanticProviderLegacyError, match="ALLOW_DB01_PY_SERVING_TRANSITION"
    ):
        check("python", root)


def test_rejects_reintroduced_hand_authored_schema(tmp_path: Path) -> None:
    root, _ = _fixture(tmp_path)
    hand_schema = root / "contracts/schema/provider-observations/pyrefly-module-v1.json"
    hand_schema.parent.mkdir(parents=True, exist_ok=True)
    hand_schema.write_text("{}\n", encoding="utf-8")
    with pytest.raises(SemanticProviderLegacyError, match="DB02_HAND_AUTHORED"):
        check("all", root)
