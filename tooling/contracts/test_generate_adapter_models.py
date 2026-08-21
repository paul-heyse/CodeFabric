"""Contract-IR compiler tests for Pydantic source, schemas, and fingerprints."""

import json
from pathlib import Path

import pytest
from codefabric_cpg_mcp.contracts.json import canonicalize_value, checksum
from jsonschema import Draft202012Validator

from tooling.contracts.generate_adapter_models import (
    AdapterModelIr,
    _load_candidate,
    _resolved_input_and_outputs,
    render_outputs,
    render_source,
)

ROOT = Path(__file__).resolve().parents[2]


def test_committed_outputs_are_exact_and_all_schemas_pass_the_metaschema() -> None:
    outputs = render_outputs(ROOT)
    assert len(outputs) == 6
    assert all(
        (ROOT / path).read_bytes() == expected for path, expected in outputs.items()
    )
    schemas = json.loads(
        (
            ROOT
            / "codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/adapter-schemas.json"
        ).read_text(encoding="utf-8")
    )
    for mode in ("validation", "serialization"):
        for schema in schemas[mode].values():
            Draft202012Validator.check_schema(schema)
    for path in (
        "contracts/adapter/fastmcp-input.schema.json",
        "contracts/adapter/fastmcp-output.schema.json",
        "contracts/adapter/fastmcp-public-meta.schema.json",
    ):
        schema = json.loads((ROOT / path).read_text(encoding="utf-8"))
        Draft202012Validator.check_schema(schema)
        assert schema["$schema"] == "https://json-schema.org/draft/2020-12/schema"
        assert schema["$id"] == f"https://codefabric.dev/{path}"
        assert schema["x-codefabric-artifact"]["status"] == "released"


def test_wp09_behavioral_acceptance_public_views_are_model_derived() -> None:
    input_schema = json.loads(
        (ROOT / "contracts/adapter/fastmcp-input.schema.json").read_text()
    )
    output_schema = json.loads(
        (ROOT / "contracts/adapter/fastmcp-output.schema.json").read_text()
    )
    metadata_schema = json.loads(
        (ROOT / "contracts/adapter/fastmcp-public-meta.schema.json").read_text()
    )
    assert {branch["$ref"].split("/")[-1] for branch in input_schema["anyOf"]} == {
        "QueryToolInput",
        "ValidateToolInput",
        "StatusToolInput",
        "ReferenceToolInput",
    }
    assert "QueryToolOutput" in output_schema["$defs"]
    assert metadata_schema["title"] == "FastMCP public metadata contract"
    assert metadata_schema["additionalProperties"] is False


def test_contract_ir_rejects_unknown_fields_and_references() -> None:
    source = json.loads((ROOT / "contracts/adapter/adapter-model-ir.json").read_text())
    source["surprise"] = True
    with pytest.raises(ValueError):
        AdapterModelIr.model_validate(source, strict=False)

    source.pop("surprise")
    source["models"][0]["fields"][0]["type"] = {"kind": "model", "name": "Absent"}
    with pytest.raises(ValueError, match="unknown type"):
        AdapterModelIr.model_validate(source, strict=False)


def test_one_ir_field_mutation_changes_source_both_schema_modes_and_fingerprints() -> (
    None
):
    identity, _ = _resolved_input_and_outputs(ROOT)
    source = json.loads((ROOT / str(identity["authority_path"])).read_text())
    original_ir = AdapterModelIr.model_validate(source, strict=False)
    source["models"][1]["fields"][0]["description"] = (
        "Mutated fact count documentation."
    )
    mutated_ir = AdapterModelIr.model_validate(source, strict=False)

    original_source = render_source(original_ir, identity)
    mutated_source = render_source(mutated_ir, identity)
    assert original_source != mutated_source
    original = _load_candidate(original_source)
    mutated = _load_candidate(mutated_source)
    for mode in ("validation", "serialization"):
        original_schema = original.QueryCounts.model_json_schema(mode=mode)
        mutated_schema = mutated.QueryCounts.model_json_schema(mode=mode)
        assert original_schema != mutated_schema
        assert checksum(canonicalize_value(original_schema)) != checksum(
            canonicalize_value(mutated_schema)
        )
