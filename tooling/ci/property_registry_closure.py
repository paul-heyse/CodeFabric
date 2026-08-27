"""Reject semantic publication schemas whose property authority is incomplete."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import yaml
from codefabric_cpg_mcp.contracts.json import canonicalize_value, checksum

ROOT = Path(__file__).resolve().parents[2]
PROPERTY_REGISTRY = Path("contracts/registry/ontology-property-registry.yaml")
SCHEMA_IR = Path("contracts/schema/schema-contract-ir.json")
SEMANTIC_FRAGMENT_PATHS = tuple(
    Path(path)
    for path in (
        "contracts/semantic-fragments/shared.json",
        "contracts/semantic-fragments/python.json",
        "contracts/semantic-fragments/rust.json",
    )
)
FIRST_SEMANTIC_TABLE_CODE = 180
REQUIRED_TYPE_DETAIL_MAPPINGS = {
    "callable_entity_id",
    "canonical_key",
    "display_name",
    "nominal_entity_id",
    "nullable_semantics_code",
    "primitive_code",
    "raw_shape_hash",
    "type_kind_code",
}


class PropertyRegistryClosureError(ValueError):
    """The property registry and publication schema do not form a closed authority."""


def _detached_digest(document: dict[str, Any]) -> str:
    detached = dict(document)
    detached.pop("canonical_digest", None)
    return checksum(canonicalize_value(detached))


def _qualified_column(
    reference: object,
    *,
    tables: dict[str, set[str]],
    field: str,
    property_name: str,
) -> tuple[str, str]:
    if not isinstance(reference, str) or reference.count(".") != 1:
        raise PropertyRegistryClosureError(
            f"{property_name} {field} must be an exact table.column reference"
        )
    table_name, column_name = reference.split(".")
    if table_name not in tables or column_name not in tables[table_name]:
        raise PropertyRegistryClosureError(
            f"{property_name} {field} references absent column {reference}"
        )
    return table_name, column_name


def _merge_records(target: list[dict[str, Any]], additions: object, key: str) -> None:
    if not isinstance(additions, list):
        raise PropertyRegistryClosureError(f"fragment {key} additions must be a list")
    keyed = {record.get(key): record for record in target}
    if None in keyed or len(keyed) != len(target):
        raise PropertyRegistryClosureError(
            f"base records have invalid or duplicate {key}"
        )
    for addition in additions:
        if not isinstance(addition, dict) or addition.get(key) is None:
            raise PropertyRegistryClosureError(f"fragment record has no {key}")
        identity = addition[key]
        existing = keyed.get(identity)
        if existing is not None and existing != addition:
            raise PropertyRegistryClosureError(
                f"fragment conflicts with frozen {key} {identity}"
            )
        if existing is None:
            target.append(addition)
            keyed[identity] = addition
    target.sort(key=lambda record: record[key])


def _compose_semantic_fragments(
    root: Path, registry: dict[str, Any], schema: dict[str, Any]
) -> None:
    records = registry.get("records")
    tables = schema.get("tables")
    if not isinstance(records, list) or not isinstance(tables, list):
        raise PropertyRegistryClosureError(
            "schema tables and property records must be lists"
        )
    for relative in SEMANTIC_FRAGMENT_PATHS:
        document = json.loads((root / relative).read_text(encoding="utf-8"))
        if not isinstance(document, dict):
            raise PropertyRegistryClosureError(f"{relative} root must be an object")
        if document.get("canonical_digest") != _detached_digest(document):
            raise PropertyRegistryClosureError(
                f"semantic fragment {relative} canonical digest is stale"
            )
        model = document.get("model")
        fragment_registry = document.get("registry")
        if not isinstance(model, dict) or not isinstance(fragment_registry, dict):
            raise PropertyRegistryClosureError(
                f"semantic fragment {relative} lacks model or registry"
            )
        _merge_records(tables, model.get("table_additions"), "table_code")
        _merge_records(
            records,
            fragment_registry.get("property_record_additions"),
            "property_code",
        )


def check(root: Path = ROOT) -> dict[str, object]:
    registry = yaml.safe_load((root / PROPERTY_REGISTRY).read_text(encoding="utf-8"))
    schema = json.loads((root / SCHEMA_IR).read_text(encoding="utf-8"))
    if not isinstance(registry, dict) or not isinstance(schema, dict):
        raise PropertyRegistryClosureError("registry and schema roots must be objects")
    registry_digest = _detached_digest(registry)
    if registry.get("canonical_digest") != registry_digest:
        raise PropertyRegistryClosureError(
            "property registry canonical digest is stale"
        )

    authorities = [
        authority
        for authority in schema.get("semantic_authorities", [])
        if authority.get("path") == PROPERTY_REGISTRY.as_posix()
    ]
    if len(authorities) != 1:
        raise PropertyRegistryClosureError(
            "schema has no unique ontology-property-registry authority"
        )
    authority = authorities[0]
    if (
        authority.get("artifact_id") != registry.get("artifact_id")
        or authority.get("canonical_digest") != registry_digest
    ):
        raise PropertyRegistryClosureError(
            "schema ontology-property authority identity or digest is stale"
        )

    _compose_semantic_fragments(root, registry, schema)

    table_contracts = schema.get("tables")
    records = registry.get("records")
    if not isinstance(table_contracts, list) or not isinstance(records, list):
        raise PropertyRegistryClosureError(
            "schema tables and property records must be lists"
        )
    tables: dict[str, set[str]] = {}
    table_codes: dict[str, int] = {}
    table_materialization_roles: dict[str, str] = {}
    for table in table_contracts:
        name = table.get("name")
        code = table.get("table_code")
        columns = table.get("columns")
        if (
            not isinstance(name, str)
            or not isinstance(code, int)
            or not isinstance(columns, list)
        ):
            raise PropertyRegistryClosureError("invalid table identity or columns")
        if name in tables:
            raise PropertyRegistryClosureError(f"duplicate table {name}")
        tables[name] = {
            column.get("name") for column in columns if isinstance(column, dict)
        }
        if None in tables[name] or len(tables[name]) != len(columns):
            raise PropertyRegistryClosureError(
                f"invalid or duplicate columns in {name}"
            )
        table_codes[name] = code
        table_materialization_roles[name] = str(table.get("materialization_role"))
    if "property_fact" not in tables:
        raise PropertyRegistryClosureError("canonical property_fact table is absent")

    codes: set[int] = set()
    names: set[str] = set()
    slugs: set[str] = set()
    extension_mappings: dict[str, set[str]] = {}
    denormalized_mappings: set[tuple[str, str]] = set()
    required_fields = {
        "property_code",
        "property_slug",
        "canonical_name",
        "subject_kind_constraints",
        "value_type",
        "cardinality",
        "owner_rule",
        "context_rule",
        "null_semantics",
        "unknown_value_policy",
        "canonicalization_rule",
        "storage",
    }
    for record in records:
        if not isinstance(record, dict) or not required_fields <= record.keys():
            raise PropertyRegistryClosureError(
                "property record is structurally incomplete"
            )
        code = record["property_code"]
        name = record["canonical_name"]
        slug = record["property_slug"]
        if (
            not isinstance(code, int)
            or code in codes
            or not isinstance(name, str)
            or name in names
            or not isinstance(slug, str)
            or slug in slugs
        ):
            raise PropertyRegistryClosureError(
                f"duplicate or invalid property identity {code!r}/{name!r}/{slug!r}"
            )
        codes.add(code)
        names.add(name)
        slugs.add(slug)
        if record["null_semantics"] != "prohibited":
            raise PropertyRegistryClosureError(f"{name} permits null-as-unknown")
        storage = record["storage"]
        if (
            not isinstance(storage, dict)
            or storage.get("canonical_table") != "property_fact"
        ):
            raise PropertyRegistryClosureError(
                f"{name} does not use canonical property_fact storage"
            )
        if "denormalized_entity_column" in storage:
            mapped = _qualified_column(
                storage["denormalized_entity_column"],
                tables=tables,
                field="denormalized_entity_column",
                property_name=name,
            )
            if mapped[0] != "entity" or mapped in denormalized_mappings:
                raise PropertyRegistryClosureError(
                    f"{name} has a duplicate or non-entity denormalized mapping"
                )
            denormalized_mappings.add(mapped)
        if "extension_table_column" in storage:
            table_name, column_name = _qualified_column(
                storage["extension_table_column"],
                tables=tables,
                field="extension_table_column",
                property_name=name,
            )
            mapped_columns = extension_mappings.setdefault(table_name, set())
            if column_name in mapped_columns:
                raise PropertyRegistryClosureError(
                    f"multiple properties claim {table_name}.{column_name}"
                )
            mapped_columns.add(column_name)

    semantic_extension_tables = {
        name
        for name, code in table_codes.items()
        if code >= FIRST_SEMANTIC_TABLE_CODE
        and table_materialization_roles[name] != "OPERATIONAL_PROJECTION"
    }
    missing_tables = semantic_extension_tables - extension_mappings.keys()
    if missing_tables:
        raise PropertyRegistryClosureError(
            "semantic extension tables lack property round-trip mappings: "
            + ", ".join(sorted(missing_tables))
        )
    if extension_mappings.get("type_detail") != REQUIRED_TYPE_DETAIL_MAPPINGS:
        raise PropertyRegistryClosureError(
            "type_detail property mapping closure differs from FAB section 26"
        )
    if extension_mappings.get("type_fact_detail") != {"type_id"}:
        raise PropertyRegistryClosureError(
            "type_fact_detail TYPE_REF round-trip mapping differs from FAB section 27"
        )
    return {
        "property_count": len(records),
        "semantic_extension_table_count": len(semantic_extension_tables),
        "extension_mapping_count": sum(map(len, extension_mappings.values())),
        "registry_digest": registry_digest,
    }


if __name__ == "__main__":
    print(json.dumps(check(), indent=2, sort_keys=True))
