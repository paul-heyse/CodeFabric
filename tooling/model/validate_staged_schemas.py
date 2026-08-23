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

    ddl = (stage_root / DDL).read_text(encoding="utf-8")
    connection = sqlite3.connect(":memory:")
    try:
        connection.executescript(ddl)
        table_count = connection.execute(
            "SELECT count(*) FROM sqlite_schema "
            "WHERE type = 'table' AND name NOT LIKE 'sqlite_%'"
        ).fetchone()[0]
    finally:
        connection.close()
    expected_tables = manifest.get("operational_tables")
    if not isinstance(expected_tables, list) or table_count != len(expected_tables):
        raise ValueError(
            f"{DDL}: table count mismatch expected={len(expected_tables)} actual={table_count}"
        )
    return {"public_schema_count": len(seen), "sqlite_table_count": table_count}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("stage_root", type=Path)
    arguments = parser.parse_args()
    print(json.dumps(validate(arguments.stage_root), sort_keys=True))


if __name__ == "__main__":
    main()
