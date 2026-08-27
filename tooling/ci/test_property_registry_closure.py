from __future__ import annotations

import json
from pathlib import Path

import pytest
import yaml
from codefabric_cpg_mcp.contracts.json import canonicalize_value, checksum

from tooling.ci.property_registry_closure import (
    PROPERTY_REGISTRY,
    SCHEMA_IR,
    SEMANTIC_FRAGMENT_PATHS,
    PropertyRegistryClosureError,
    check,
)

ROOT = Path(__file__).resolve().parents[2]


def _detached_digest(document: dict[str, object]) -> str:
    detached = dict(document)
    detached.pop("canonical_digest", None)
    return checksum(canonicalize_value(detached))


def _fixture(tmp_path: Path) -> tuple[dict[str, object], dict[str, object]]:
    registry = yaml.safe_load((ROOT / PROPERTY_REGISTRY).read_text(encoding="utf-8"))
    schema = json.loads((ROOT / SCHEMA_IR).read_text(encoding="utf-8"))
    (tmp_path / PROPERTY_REGISTRY.parent).mkdir(parents=True)
    (tmp_path / SCHEMA_IR.parent).mkdir(parents=True)
    for relative in SEMANTIC_FRAGMENT_PATHS:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(
            (ROOT / relative).read_text(encoding="utf-8"), encoding="utf-8"
        )
    return registry, schema


def _seal(root: Path, registry: dict[str, object], schema: dict[str, object]) -> None:
    registry["canonical_digest"] = _detached_digest(registry)
    authorities = schema["semantic_authorities"]
    assert isinstance(authorities, list)
    authority = next(
        candidate
        for candidate in authorities
        if candidate["path"] == PROPERTY_REGISTRY.as_posix()
    )
    authority["canonical_digest"] = registry["canonical_digest"]
    schema["canonical_digest"] = _detached_digest(schema)
    (root / PROPERTY_REGISTRY).write_text(
        yaml.safe_dump(registry, sort_keys=False), encoding="utf-8"
    )
    (root / SCHEMA_IR).write_text(json.dumps(schema, indent=2) + "\n", encoding="utf-8")


def test_current_schema_has_bidirectional_property_closure() -> None:
    result = check()
    assert result["property_count"] == 17
    assert result["semantic_extension_table_count"] == 2
    assert result["extension_mapping_count"] == 12


def test_rejects_unregistered_semantic_extension_table(tmp_path: Path) -> None:
    registry, schema = _fixture(tmp_path)
    tables = schema["tables"]
    assert isinstance(tables, list)
    tables.append(
        {
            "table_code": 200,
            "name": "unregistered_detail",
            "columns": [{"name": "semantic_value"}],
        }
    )
    _seal(tmp_path, registry, schema)
    with pytest.raises(PropertyRegistryClosureError, match="unregistered_detail"):
        check(tmp_path)


def test_rejects_stale_extension_column_reference(tmp_path: Path) -> None:
    registry, schema = _fixture(tmp_path)
    records = registry["records"]
    assert isinstance(records, list)
    type_kind = next(
        record for record in records if record["canonical_name"] == "TYPE_KIND"
    )
    type_kind["storage"]["extension_table_column"] = "type_detail.absent_kind"
    _seal(tmp_path, registry, schema)
    with pytest.raises(PropertyRegistryClosureError, match="absent_kind"):
        check(tmp_path)


def test_rejects_type_column_without_owning_property(tmp_path: Path) -> None:
    registry, schema = _fixture(tmp_path)
    records = registry["records"]
    assert isinstance(records, list)
    registry["records"] = [
        record for record in records if record["canonical_name"] != "TYPE_KIND"
    ]
    _seal(tmp_path, registry, schema)
    with pytest.raises(
        PropertyRegistryClosureError, match="type_detail property mapping"
    ):
        check(tmp_path)


def test_rejects_stale_registry_digest_before_publication(tmp_path: Path) -> None:
    registry, schema = _fixture(tmp_path)
    _seal(tmp_path, registry, schema)
    registry_path = tmp_path / PROPERTY_REGISTRY
    document = yaml.safe_load(registry_path.read_text(encoding="utf-8"))
    document["records"][0]["property_slug"] = "drifted-name"
    registry_path.write_text(
        yaml.safe_dump(document, sort_keys=False), encoding="utf-8"
    )
    with pytest.raises(PropertyRegistryClosureError, match="digest is stale"):
        check(tmp_path)
