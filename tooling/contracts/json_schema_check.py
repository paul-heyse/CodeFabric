"""Hermetically validate catalog-owned JSON Schemas against Draft 2020-12."""

from __future__ import annotations

import argparse
import json
import sys
from importlib.metadata import version
from pathlib import Path, PurePosixPath
from typing import Any

from jsonschema import Draft202012Validator
from jsonschema.exceptions import SchemaError

CATALOG_PATH = PurePosixPath("contracts/manifests/suite-manifest.json")
DRAFT_2020_12_URI = "https://json-schema.org/draft/2020-12/schema"
JSONSCHEMA_VERSION = "4.26.0"
SCHEMA_ID_PREFIX = "https://codefabric.dev/"
MAX_DIAGNOSTIC_CHARS = 500
CATALOG_BOOTSTRAP_MAX_BYTES = 262_144


class SchemaCatalogError(ValueError):
    """A stable, path-aware JSON Schema catalog validation error."""

    def __init__(self, code: str, path: PurePosixPath, detail: str) -> None:
        self.code = code
        self.path = path
        self.detail = _bounded(detail)
        super().__init__(f"{code}: {path}: {self.detail}")


def _bounded(detail: str) -> str:
    if len(detail) <= MAX_DIAGNOSTIC_CHARS:
        return detail
    return f"{detail[:MAX_DIAGNOSTIC_CHARS]}..."


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise ValueError(f"duplicate JSON object key {key!r}")
        value[key] = item
    return value


def _read_bounded(path: Path, display_path: PurePosixPath, maximum: int) -> bytes:
    try:
        with path.open("rb") as source:
            data = source.read(maximum + 1)
    except OSError as error:
        raise SchemaCatalogError("schema_io", display_path, str(error)) from error
    if len(data) > maximum:
        raise SchemaCatalogError(
            "schema_resource_limit",
            display_path,
            f"max_bytes observed={len(data)} maximum={maximum}",
        )
    return data


def _load_json(path: Path, display_path: PurePosixPath, maximum: int) -> Any:
    try:
        return json.loads(
            _read_bounded(path, display_path, maximum),
            object_pairs_hook=_reject_duplicate_keys,
        )
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
        raise SchemaCatalogError("invalid_json", display_path, str(error)) from error


def _safe_authority_path(value: object) -> PurePosixPath:
    if not isinstance(value, str) or not value:
        raise SchemaCatalogError(
            "invalid_catalog_schema_path",
            CATALOG_PATH,
            "authority_path must be a string",
        )
    path = PurePosixPath(value)
    if path.is_absolute() or any(part in {"", ".", ".."} for part in path.parts):
        raise SchemaCatalogError(
            "invalid_catalog_schema_path",
            CATALOG_PATH,
            f"unsafe authority_path {value!r}",
        )
    if path.parts[0] != "contracts" or not value.endswith(".schema.json"):
        raise SchemaCatalogError(
            "invalid_catalog_schema_path",
            CATALOG_PATH,
            f"JSON Schema authority_path must be contracts/**/*.schema.json, got {value!r}",
        )
    return path


def catalog_schema_inputs(
    repository_root: Path,
) -> tuple[tuple[PurePosixPath, int], ...]:
    """Return sorted JSON Schema paths and catalog-owned byte limits."""

    catalog_file = repository_root / CATALOG_PATH
    catalog = _load_json(catalog_file, CATALOG_PATH, CATALOG_BOOTSTRAP_MAX_BYTES)
    if not isinstance(catalog, dict) or not isinstance(catalog.get("artifacts"), list):
        raise SchemaCatalogError(
            "invalid_catalog", CATALOG_PATH, "root must contain an artifacts array"
        )

    profiles = {
        profile.get("profile_id"): profile
        for profile in catalog.get("resource_budget_profiles", [])
        if isinstance(profile, dict)
    }
    schema_inputs: list[tuple[PurePosixPath, int]] = []
    for index, artifact in enumerate(catalog["artifacts"]):
        if not isinstance(artifact, dict):
            raise SchemaCatalogError(
                "invalid_catalog",
                CATALOG_PATH,
                f"artifacts[{index}] must be an object",
            )
        if artifact.get("artifact_kind") == "json-schema":
            path = _safe_authority_path(artifact.get("authority_path"))
            profile_id = artifact.get("resource_budget_profile")
            profile = profiles.get(profile_id)
            maximum = profile.get("max_bytes") if isinstance(profile, dict) else None
            if not isinstance(maximum, int) or maximum <= 0:
                raise SchemaCatalogError(
                    "invalid_catalog_schema_budget",
                    CATALOG_PATH,
                    f"{path} selects invalid resource profile {profile_id!r}",
                )
            schema_inputs.append((path, maximum))

    schema_paths = [path for path, _ in schema_inputs]
    if len(schema_paths) != len(set(schema_paths)):
        raise SchemaCatalogError(
            "duplicate_catalog_schema_path",
            CATALOG_PATH,
            "JSON Schema authority paths repeat",
        )

    cataloged = set(schema_paths)
    discovered = {
        PurePosixPath(path.relative_to(repository_root).as_posix())
        for path in (repository_root / "contracts").rglob("*.schema.json")
        if path.is_file()
    }
    if cataloged != discovered:
        missing = sorted(discovered - cataloged)
        nonexistent = sorted(cataloged - discovered)
        raise SchemaCatalogError(
            "catalog_schema_census_mismatch",
            CATALOG_PATH,
            f"uncataloged={missing!r}; missing_authorities={nonexistent!r}",
        )

    return tuple(sorted(schema_inputs))


def catalog_schema_paths(repository_root: Path) -> tuple[PurePosixPath, ...]:
    """Return the complete, sorted JSON Schema path set."""

    return tuple(path for path, _ in catalog_schema_inputs(repository_root))


def validate_schema(
    repository_root: Path, schema_path: PurePosixPath, maximum_bytes: int
) -> None:
    """Validate one catalog-owned schema using only jsonschema's bundled metaschemas."""

    schema = _load_json(repository_root / schema_path, schema_path, maximum_bytes)
    if not isinstance(schema, dict):
        raise SchemaCatalogError(
            "invalid_schema_root", schema_path, "schema root must be an object"
        )

    dialect = schema.get("$schema")
    if dialect != DRAFT_2020_12_URI:
        raise SchemaCatalogError(
            "invalid_schema_dialect",
            schema_path,
            f"$schema must equal {DRAFT_2020_12_URI!r}, got {dialect!r}",
        )

    expected_id = f"{SCHEMA_ID_PREFIX}{schema_path.as_posix()}"
    schema_id = schema.get("$id")
    if schema_id != expected_id:
        raise SchemaCatalogError(
            "invalid_schema_id",
            schema_path,
            f"$id must equal stable catalog-derived ID {expected_id!r}, got {schema_id!r}",
        )

    try:
        Draft202012Validator.check_schema(schema)
    except SchemaError as error:
        location = getattr(error, "json_path", "$")
        raise SchemaCatalogError(
            "invalid_draft_2020_12_schema", schema_path, f"{location}: {error.message}"
        ) from error


def validate_catalog_schemas(repository_root: Path) -> tuple[PurePosixPath, ...]:
    """Validate the complete catalog-derived JSON Schema set."""

    resolved_version = version("jsonschema")
    if resolved_version != JSONSCHEMA_VERSION:
        raise SchemaCatalogError(
            "jsonschema_version_mismatch",
            CATALOG_PATH,
            f"expected jsonschema {JSONSCHEMA_VERSION}, resolved {resolved_version}",
        )
    if Draft202012Validator.META_SCHEMA.get("$id") != DRAFT_2020_12_URI:
        raise SchemaCatalogError(
            "metaschema_identity_mismatch",
            CATALOG_PATH,
            "jsonschema Draft202012Validator does not expose the expected bundled metaschema",
        )

    schema_inputs = catalog_schema_inputs(repository_root)
    for schema_path, maximum_bytes in schema_inputs:
        validate_schema(repository_root, schema_path, maximum_bytes)
    return tuple(path for path, _ in schema_inputs)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repository-root",
        type=Path,
        default=Path(__file__).resolve().parents[2],
        help="repository root containing contracts/manifests/suite-manifest.json",
    )
    arguments = parser.parse_args(argv)

    try:
        schemas = validate_catalog_schemas(arguments.repository_root.resolve())
    except SchemaCatalogError as error:
        print(error, file=sys.stderr)
        return 1

    print(
        f"validated {len(schemas)} catalog JSON Schemas with jsonschema {JSONSCHEMA_VERSION}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
