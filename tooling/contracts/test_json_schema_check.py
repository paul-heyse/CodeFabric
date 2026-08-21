"""Tests for the hermetic catalog-derived JSON Schema gate."""

from __future__ import annotations

import json
from pathlib import Path, PurePosixPath
from typing import Any

import pytest
import tomllib

from tooling.contracts.json_schema_check import (
    CATALOG_PATH,
    DEPLOYMENT_PROFILE_PATH,
    DEPLOYMENT_SCHEMA_PATH,
    DRAFT_2020_12_URI,
    JSONSCHEMA_VERSION,
    MODEL_PACK_NEGATIVE_PATH,
    MODEL_PACK_POSITIVE_PATH,
    MODEL_PACK_SCHEMA_PATH,
    PYYAML_VERSION,
    SCHEMA_DRIFT_FIXTURE_PATH,
    SchemaCatalogError,
    validate_catalog_schemas,
    validate_deployment_profile,
    validate_model_pack_examples,
    validate_schema_drift_fixture,
)

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = Path("contracts/schema/example.schema.json")
SCHEMA_ID = f"https://codefabric.dev/{SCHEMA_PATH.as_posix()}"


def _write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(f"{json.dumps(value, indent=2)}\n", encoding="utf-8")


def _fixture_root(tmp_path: Path, schema: dict[str, Any]) -> Path:
    catalog = {
        "resource_budget_profiles": [
            {
                "profile_id": "schema-test",
                "max_bytes": 4096,
            }
        ],
        "artifacts": [
            {
                "artifact_id": "codefabric.schema.example",
                "artifact_kind": "json-schema",
                "authority_path": SCHEMA_PATH.as_posix(),
                "resource_budget_profile": "schema-test",
            }
        ],
    }
    _write_json(tmp_path / CATALOG_PATH, catalog)
    _write_json(tmp_path / SCHEMA_PATH, schema)
    return tmp_path


def _valid_schema() -> dict[str, Any]:
    return {
        "$schema": DRAFT_2020_12_URI,
        "$id": SCHEMA_ID,
        "type": "object",
        "properties": {"value": {"type": "string"}},
        "additionalProperties": False,
    }


def test_current_catalog_schema_suite_is_valid() -> None:
    schemas = validate_catalog_schemas(REPOSITORY_ROOT)
    discovered = {
        PurePosixPath(path.relative_to(REPOSITORY_ROOT).as_posix())
        for path in (REPOSITORY_ROOT / "contracts").rglob("*.schema.json")
    }

    assert schemas
    assert set(schemas) == discovered
    assert schemas == tuple(sorted(schemas))


def test_model_pack_schema_enforces_declarative_non_executable_records() -> None:
    catalog = json.loads((REPOSITORY_ROOT / CATALOG_PATH).read_text(encoding="utf-8"))
    profile_by_id = {
        profile["profile_id"]: profile
        for profile in catalog["resource_budget_profiles"]
    }
    descriptor = next(
        artifact
        for artifact in catalog["artifacts"]
        if artifact["authority_path"] == MODEL_PACK_SCHEMA_PATH.as_posix()
    )
    maximum = profile_by_id[descriptor["resource_budget_profile"]]["max_bytes"]

    validate_model_pack_examples(REPOSITORY_ROOT, maximum)
    assert (REPOSITORY_ROOT / MODEL_PACK_POSITIVE_PATH).is_file()
    assert (REPOSITORY_ROOT / MODEL_PACK_NEGATIVE_PATH).is_file()


def test_wp09_negative_schema_version_drift_fixture_is_executable() -> None:
    validate_schema_drift_fixture(REPOSITORY_ROOT)
    fixture = json.loads(
        (REPOSITORY_ROOT / SCHEMA_DRIFT_FIXTURE_PATH).read_text(encoding="utf-8")
    )
    assert fixture["expected_error"] == "SCHEMA_VERSION_NOT_ADVANCED"


def test_deployment_profile_matches_schema() -> None:
    validate_deployment_profile(REPOSITORY_ROOT)
    assert (REPOSITORY_ROOT / DEPLOYMENT_SCHEMA_PATH).is_file()
    assert (REPOSITORY_ROOT / DEPLOYMENT_PROFILE_PATH).is_file()


@pytest.mark.parametrize(
    "dialect", [None, "https://json-schema.org/draft/2019-09/schema"]
)
def test_missing_or_wrong_schema_dialect_fails(
    tmp_path: Path, dialect: str | None
) -> None:
    schema = _valid_schema()
    if dialect is None:
        del schema["$schema"]
    else:
        schema["$schema"] = dialect

    with pytest.raises(SchemaCatalogError, match="invalid_schema_dialect"):
        validate_catalog_schemas(_fixture_root(tmp_path, schema))


@pytest.mark.parametrize("schema_id", [None, "example.schema.json"])
def test_missing_or_unstable_schema_id_fails(
    tmp_path: Path, schema_id: str | None
) -> None:
    schema = _valid_schema()
    if schema_id is None:
        del schema["$id"]
    else:
        schema["$id"] = schema_id

    with pytest.raises(SchemaCatalogError, match="invalid_schema_id"):
        validate_catalog_schemas(_fixture_root(tmp_path, schema))


def test_draft_2020_12_metaschema_rejects_invalid_schema(tmp_path: Path) -> None:
    schema = _valid_schema()
    schema["type"] = 7

    with pytest.raises(SchemaCatalogError, match="invalid_draft_2020_12_schema"):
        validate_catalog_schemas(_fixture_root(tmp_path, schema))


def test_uncataloged_schema_cannot_evade_validation(tmp_path: Path) -> None:
    root = _fixture_root(tmp_path, _valid_schema())
    extra = _valid_schema()
    extra["$id"] = "https://codefabric.dev/contracts/schema/extra.schema.json"
    _write_json(root / "contracts/schema/extra.schema.json", extra)

    with pytest.raises(SchemaCatalogError, match="catalog_schema_census_mismatch"):
        validate_catalog_schemas(root)


def test_catalog_schema_byte_budget_fails_at_limit_plus_one(tmp_path: Path) -> None:
    root = _fixture_root(tmp_path, _valid_schema())
    schema_size = (root / SCHEMA_PATH).stat().st_size
    catalog = json.loads((root / CATALOG_PATH).read_text(encoding="utf-8"))
    catalog["resource_budget_profiles"][0]["max_bytes"] = schema_size
    _write_json(root / CATALOG_PATH, catalog)
    assert validate_catalog_schemas(root) == (PurePosixPath(SCHEMA_PATH),)

    catalog["resource_budget_profiles"][0]["max_bytes"] = schema_size - 1
    _write_json(root / CATALOG_PATH, catalog)

    with pytest.raises(SchemaCatalogError, match="schema_resource_limit") as caught:
        validate_catalog_schemas(root)

    assert f"observed={schema_size}" in str(caught.value)
    assert f"maximum={schema_size - 1}" in str(caught.value)


def test_jsonschema_build_dependency_is_exact() -> None:
    pyproject = tomllib.loads(
        (REPOSITORY_ROOT / "codefabric-cpg-mcp/pyproject.toml").read_text(
            encoding="utf-8"
        )
    )

    assert f"jsonschema=={JSONSCHEMA_VERSION}" in pyproject["dependency-groups"]["dev"]
    assert f"pyyaml=={PYYAML_VERSION}" in pyproject["dependency-groups"]["dev"]
