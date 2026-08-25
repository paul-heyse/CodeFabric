"""Independently validate model-staged JSON Schema and SQLite projections."""

from __future__ import annotations

import argparse
import json
import sqlite3
from pathlib import Path, PurePosixPath
from typing import Any

from jsonschema import Draft202012Validator

DIALECT = "https://json-schema.org/draft/2020-12/schema"
MAX_BYTES = 16 * 1024 * 1024
MANIFEST = PurePosixPath("contracts/generated/model/schema/table-specs.json")
DDL = PurePosixPath("contracts/generated/model/schema/operational-store.sql")


def _reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise ValueError(f"duplicate object key {key!r}")
        value[key] = item
    return value


def _load_json(root: Path, relative: PurePosixPath) -> Any:
    path = root / relative
    data = path.read_bytes()
    if len(data) > MAX_BYTES:
        raise ValueError(f"{relative}: maximum byte budget exceeded")
    return json.loads(data, object_pairs_hook=_reject_duplicates)


def validate(stage_root: Path) -> dict[str, int]:
    """Validate every staged public schema and execute the staged strict DDL."""

    manifest = _load_json(stage_root, MANIFEST)
    declarations = manifest.get("public_schemas")
    if not isinstance(declarations, list) or len(declarations) != 8:
        raise ValueError(f"{MANIFEST}: expected eight public schema declarations")

    seen: set[PurePosixPath] = set()
    schemas: dict[tuple[str, PurePosixPath], dict[str, Any]] = {}
    for index, declaration in enumerate(declarations):
        if not isinstance(declaration, dict):
            raise TypeError(f"{MANIFEST}: public_schemas[{index}] is not an object")
        raw_path = declaration.get("path")
        if not isinstance(raw_path, str):
            raise TypeError(f"{MANIFEST}: public_schemas[{index}].path is not a string")
        path = PurePosixPath(raw_path)
        if (
            path.is_absolute()
            or any(part in {"", ".", ".."} for part in path.parts)
            or path in seen
        ):
            raise ValueError(
                f"{MANIFEST}: unsafe or duplicate public schema path {path}"
            )
        seen.add(path)
        schema = _load_json(stage_root, path)
        if not isinstance(schema, dict):
            raise TypeError(f"{path}: root is not an object")
        if schema.get("$schema") != DIALECT:
            raise ValueError(f"{path}: wrong JSON Schema dialect")
        if schema.get("$id") != f"https://codefabric.dev/{path.as_posix()}":
            raise ValueError(f"{path}: model-derived $id mismatch")
        Draft202012Validator.check_schema(schema)
        schema_kind = declaration.get("schema_kind")
        if not isinstance(schema_kind, str):
            raise TypeError(
                f"{MANIFEST}: public_schemas[{index}].schema_kind is not a string"
            )
        schemas[(schema_kind, path)] = schema

    raw_instances_path = manifest.get("public_schema_instances")
    if not isinstance(raw_instances_path, str):
        raise TypeError(f"{MANIFEST}: public_schema_instances is not a string")
    instances_path = PurePosixPath(raw_instances_path)
    if instances_path.is_absolute() or any(
        part in {"", ".", ".."} for part in instances_path.parts
    ):
        raise ValueError(f"{MANIFEST}: unsafe public schema instances path")
    instance_fixture = _load_json(stage_root, instances_path)
    if not isinstance(instance_fixture, dict) or set(instance_fixture) != {
        "format_version",
        "instances",
    }:
        raise TypeError(f"{instances_path}: invalid fixture envelope")
    if instance_fixture["format_version"] != 1:
        raise ValueError(f"{instances_path}: unsupported format version")
    instances = instance_fixture["instances"]
    if not isinstance(instances, list) or len(instances) != len(schemas):
        raise ValueError(f"{instances_path}: expected one instance per public schema")
    validated_instances: set[tuple[str, PurePosixPath]] = set()
    for index, item in enumerate(instances):
        if not isinstance(item, dict) or set(item) != {
            "schema_kind",
            "schema_path",
            "instance",
        }:
            raise TypeError(f"{instances_path}: instances[{index}] is malformed")
        schema_kind = item["schema_kind"]
        raw_schema_path = item["schema_path"]
        if not isinstance(schema_kind, str) or not isinstance(raw_schema_path, str):
            raise TypeError(
                f"{instances_path}: instances[{index}] identity is malformed"
            )
        key = (schema_kind, PurePosixPath(raw_schema_path))
        schema = schemas.get(key)
        if schema is None or key in validated_instances:
            raise ValueError(
                f"{instances_path}: unknown or duplicate instance identity {key}"
            )
        validated_instances.add(key)
        errors = sorted(
            Draft202012Validator(schema).iter_errors(item["instance"]),
            key=lambda error: tuple(str(part) for part in error.absolute_path),
        )
        if errors:
            location = ".".join(str(part) for part in errors[0].absolute_path) or "$"
            raise ValueError(
                f"{instances_path}: {schema_kind} instance fails at {location}: "
                f"{errors[0].message}"
            )
    if validated_instances != set(schemas):
        raise ValueError(f"{instances_path}: public schema instance census differs")

    ddl = (stage_root / DDL).read_text(encoding="utf-8")
    connection = sqlite3.connect(":memory:")
    try:
        connection.executescript(ddl)
        table_count = connection.execute(
            "SELECT count(*) FROM sqlite_schema "
            "WHERE type = 'table' AND name NOT LIKE 'sqlite_%'"
        ).fetchone()[0]
        actual_views = {
            row[0]
            for row in connection.execute(
                "SELECT name FROM sqlite_schema WHERE type = 'view'"
            )
        }
        declarations = manifest.get("control_projections")
        if not isinstance(declarations, list):
            raise TypeError(f"{MANIFEST}: control_projections is not an array")
        expected_views: dict[str, list[str]] = {}
        for index, declaration in enumerate(declarations):
            if not isinstance(declaration, dict):
                raise TypeError(
                    f"{MANIFEST}: control_projections[{index}] is not an object"
                )
            if declaration.get("projection_role") != "DERIVED_OPERATIONAL":
                continue
            name = declaration.get("view_name")
            columns = declaration.get("columns")
            if (
                not isinstance(name, str)
                or not isinstance(columns, list)
                or not all(isinstance(column, str) for column in columns)
            ):
                raise TypeError(
                    f"{MANIFEST}: control_projections[{index}] has invalid view fields"
                )
            expected_views[name] = columns
        if actual_views != set(expected_views):
            raise ValueError(
                f"{DDL}: derived view mismatch expected={sorted(expected_views)} "
                f"actual={sorted(actual_views)}"
            )
        for name, expected_columns in expected_views.items():
            actual_columns = [
                row[1] for row in connection.execute(f"PRAGMA table_info({name})")
            ]
            if actual_columns != expected_columns:
                raise ValueError(
                    f"{DDL}: view {name} columns differ from the typed projection"
                )
    finally:
        connection.close()
    expected_tables = manifest.get("operational_tables")
    if not isinstance(expected_tables, list) or table_count != len(expected_tables):
        raise ValueError(
            f"{DDL}: table count mismatch expected={len(expected_tables)} actual={table_count}"
        )
    return {
        "public_schema_count": len(seen),
        "public_schema_instance_count": len(validated_instances),
        "sqlite_table_count": table_count,
        "sqlite_view_count": len(actual_views),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("stage_root", type=Path)
    arguments = parser.parse_args()
    print(json.dumps(validate(arguments.stage_root), sort_keys=True))


if __name__ == "__main__":
    main()
