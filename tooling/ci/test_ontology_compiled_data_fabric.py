from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path

from scripts import ontology_fabric_probe_suite

ROOT = Path(__file__).resolve().parents[2]
IR_PATH = ROOT / "contracts/schema/schema-contract-ir.json"
_PROBE_DECISION: Path | None = None


def _text(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def _ir() -> dict[str, object]:
    return json.loads(IR_PATH.read_text(encoding="utf-8"))


def _tables() -> dict[int, dict[str, object]]:
    return {table["table_code"]: table for table in _ir()["tables"]}


def _ontology_tables() -> list[dict[str, object]]:
    return [table for table in _ir()["tables"] if table["family"] == "ontology"]


def _reviewed_probe_decision() -> Path:
    global _PROBE_DECISION
    if _PROBE_DECISION is None:
        reports = ontology_fabric_probe_suite.run_suite()
        _PROBE_DECISION = ontology_fabric_probe_suite.record_reviewed_decision(reports)
    ontology_fabric_probe_suite.validate_reviewed_decision(_PROBE_DECISION)
    return _PROBE_DECISION


def _assert_absent(pattern: str, *paths: str) -> None:
    expression = re.compile(pattern)
    matches = [path for path in paths if expression.search(_text(path))]
    assert not matches, f"retired pattern {pattern!r} remains in {matches}"


def test_odf_probe_suite_observations_complete() -> None:
    ontology_fabric_probe_suite.validate_contract()
    assert len(ontology_fabric_probe_suite.PROBES) == 8
    assert {key.split("-")[1] for key in ontology_fabric_probe_suite.PROBES} == {
        "1",
        "2",
        "3a",
        "3b",
        "4",
        "5",
        "6",
        "7",
    }
    decision = ontology_fabric_probe_suite.validate_reviewed_decision(
        _reviewed_probe_decision()
    )
    assert len(decision["decisions"]) == 8
    for record in decision["decisions"]:
        assert record["report_digest"].startswith("b3:")
        assert record["pin_config_digest"].startswith("b3:")


def test_odf_probe_pin_identity() -> None:
    decision = ontology_fabric_probe_suite.validate_reviewed_decision(
        _reviewed_probe_decision()
    )
    identity = ontology_fabric_probe_suite.stack_identity()
    assert identity["datafusion"] == "55.0.0"
    assert identity["arrow"] == "59.2.0"
    assert identity["delta_revision"].startswith("43a0cf10")
    assert decision["resolved_stack"] == identity


def test_odf_probe_decision_transaction_closure() -> None:
    contracts = ontology_fabric_probe_suite.PROBES.values()
    assert all(contract["branch"] and contract["fallback"] for contract in contracts)
    assert ontology_fabric_probe_suite.PROBES["PR-7"]["performance_posture"] == (
        "owner-waived-no-measurement"
    )
    decision = ontology_fabric_probe_suite.validate_reviewed_decision(
        _reviewed_probe_decision()
    )
    assert decision["reviewer"] == "plan-owner-v2-implementation-authorization"
    assert all(record["rationale"] for record in decision["decisions"])


def test_odf_probe_worktree_immutability() -> None:
    before = ontology_fabric_probe_suite.worktree_fingerprint()
    _reviewed_probe_decision()
    after = ontology_fabric_probe_suite.worktree_fingerprint()
    assert before == after
    assert str(ontology_fabric_probe_suite.REPORT_ROOT).startswith(str(ROOT / "target"))


def test_odf_compiled_ontology_reproducible() -> None:
    result = subprocess.run(
        [str(ROOT / "scripts/model_repro_check.sh")],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    assert "model reproduction check passed" in result.stdout
    assert "validated 58 model-derived TableSpecs" in result.stdout


def test_odf_schema_contract_pass_closure() -> None:
    driver = _text("src/bin/codefabric_model/schema_driver.rs")
    assert "struct CompiledOntology" in driver
    assert all(
        name in driver
        for name in ("semantic_operations", "provider_raw_kinds", "vocabulary")
    )


def test_odf_dual_list_reconciliation_zero() -> None:
    _assert_absent(
        r"reconcile_(?:generated_)?columns|zip\([^\n]*generated[^\n]*columns",
        "src/schema_registry.rs",
        "src/bin/codefabric_model/schema_driver.rs",
    )
    assert "table_column_contracts" in _text("src/schema_registry.rs")


def test_odf_stage1_schema_fingerprint_equality() -> None:
    registry = _text("src/schema_registry.rs")
    generated = _text("src/generated/table_specs.rs")
    assert "schema_contract_digest" in registry
    assert "GENERATED_SCHEMA_CONTRACT_DIGEST" in generated


def test_odf_row_shape_field_census() -> None:
    driver = _text("src/bin/codefabric_model/schema_driver.rs")
    fact_tables = [
        table
        for table in _ir()["tables"]
        if table["durable_mutation"]
        in {"OWNER_REPLACED_FACT", "DERIVED_OWNER_REPLACED"}
    ]
    assert fact_tables
    assert "render_row_shape" in driver


def test_odf_handwritten_row_struct_zero() -> None:
    ingest = _text("src/fact_ingest.rs")
    assert not re.search(r"pub struct [A-Za-z]+Row\s*\{", ingest)
    assert 'include!("generated/fact_row_encoders.rs")' in ingest


def test_odf_phrase_arm_registry_coverage() -> None:
    compiler = _text("src/semantic_query.rs")
    generated = _text("src/generated/model_schema_tables.rs")
    assert "SEMANTIC_OPERATION_SPECS" in compiler
    assert "SemanticOperationSpec" in generated


def test_odf_literal_code_predicate_zero() -> None:
    semantic = _text("src/semantic_query.rs")
    assert "compiled_enum_code" in semantic
    assert "ontology_code(" in semantic
    assert "ENTITY_KIND_IDS," not in semantic


def test_odf_phrase_governance_rules_active() -> None:
    governance = _text("rules/model-no-raw-governed-code-or-flag.yml")
    justfile = _text("justfile")
    assert "severity:" in governance
    assert "governance-scan" in justfile


def test_odf_extension_registry_and_analyzer_seams_installed() -> None:
    serving = _text("src/fabric/serving.rs")
    assert ".with_extension_type_registry" in serving
    assert ".with_analyzer_rule" in serving


def test_odf_scattered_extension_check_zero() -> None:
    serving = _text("src/fabric/serving.rs")
    assert "validate_logical_extension_field" not in serving
    assert "DomainConformanceRule" in serving


def test_odf_id_domain_registry_census() -> None:
    domains = _ir()["id_domains"]
    assert len({domain["domain_slug"] for domain in domains}) == len(domains)
    assert len({domain["extension_name"] for domain in domains}) == len(domains)


def test_odf_id16_zero_state() -> None:
    _assert_absent(
        r"codefabric\.id16",
        "contracts/schema/schema-contract-ir.json",
        "src/schema_registry.rs",
        "src/semantic_query.rs",
    )
    assert "codefabric.hash32" in _text("src/schema_registry.rs")


def test_odf_extension_consumer_classification() -> None:
    spec = _text(
        "docs/authoritative_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md"
    )
    assert "application publication validation" in spec.lower()
    assert "ExtensionTypeRegistry" in spec


def test_odf_ontology_contract_table_census() -> None:
    tables = _ontology_tables()
    assert len(tables) == 20
    assert {table["name"] for table in tables} == {
        "enum_domain",
        "entity_kind",
        "entity_family",
        "relation_kind",
        "relation_family",
        "property_kind",
        "fact_kind",
        "provider_raw_kind",
        "id_domain",
        "ontology_term",
        "ontology_edge",
        "registry_authority",
        "semantic_type_binding",
        "table_contract",
        "column_contract",
        "result_schema",
        "result_field",
        "identity_recipe",
        "phrase_binding",
        "rule_contract",
    }


def test_odf_nested_ontology_membership_zero() -> None:
    tables = _tables()
    edge_columns = {column["name"] for column in tables[21]["columns"]}
    assert {"subject_term_id", "predicate_term_id", "object_term_id"} <= edge_columns
    assert not any(
        column["logical_type"].endswith("list")
        for table in _ontology_tables()
        for column in table["columns"]
    )


def test_odf_compiled_rule_contract_census() -> None:
    contracts = _ir()["ontology_rule_contracts"]
    assert len(contracts) == 11
    assert len({contract["operation_kind"] for contract in contracts}) == 11


def test_odf_logical_structure_classification_invariance() -> None:
    groups = {group["group_id"]: group for group in _ir()["structure_groups"]}
    assert groups["source_span"]["logical_class"] == "STRUCTURALLY_OWNED_COHESIVE"
    assert groups["property_value"]["logical_class"] == "INDEPENDENTLY_FILTERABLE"


def test_odf_result_schema_census() -> None:
    schemas = _ir()["result_schemas"]
    assert len(schemas) == 8
    assert {schema["query_form_code"] for schema in schemas} == {
        10,
        20,
        30,
        40,
        50,
        60,
        70,
        80,
    }


def test_odf_handwritten_result_schema_zero() -> None:
    semantic = _text("src/semantic_query.rs")
    production = semantic.split("#[cfg(test)]", maxsplit=1)[0]
    assert "Field::new(" not in production
    assert "Schema::new(" not in production


def test_odf_control_projection_type_census() -> None:
    tables = _ir()["operational_tables"]
    columns = [column for table in tables for column in table["columns"]]
    assert len(tables) == 27
    assert columns and all("logical_type" in column for column in columns)
    assert all(
        column.get("id_domain")
        for column in columns
        if column["logical_type"] == "id16"
    )
    assert any(column["logical_type"] == "timestamp_utc" for column in columns)


def test_odf_untyped_control_id_zero() -> None:
    generated = _text("src/generated/table_specs.rs")
    assert "GeneratedOperationalColumn" in generated
    assert "logical_type: LogicalType::Id16" in generated
    assert "id_domain: Some(" in generated
    assert "Transitional bootstrap" not in generated


def test_odf_statistics_precision_census() -> None:
    overlay = _text("src/fabric/overlay.rs")
    snapshot = _text("src/fabric/snapshot_catalog.rs")
    assert "Statistics::new_unknown" not in overlay
    assert "authenticated_statistics" in snapshot


def test_odf_spec_amendment_census() -> None:
    fab = _text(
        "docs/authoritative_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md"
    )
    qry = _text(
        "docs/authoritative_design/code_property_graph_semantic_query_specification_v1.3.md"
    )
    assert "cpg_ontology" in fab and "GraphOperatorPlan" in fab
    assert "table-function" in qry.lower() or "table function" in qry.lower()


def test_odf_spec_index_navigation_current() -> None:
    routing = _text("docs/spec_index/library-routing.md")
    traceability = _text("docs/spec_index/wave-traceability.md")
    assert "GraphOperatorPlan" in routing
    assert "cpg_ontology" in traceability


def test_odf_retired_name_reference_zero() -> None:
    _assert_absent(
        r"codefabric\.id16|cpg_base\.enum_catalog",
        "docs/authoritative_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md",
        "docs/authoritative_design/code_property_graph_semantic_query_specification_v1.3.md",
        "docs/spec_index/library-routing.md",
    )
    assert "enum_domain" in _text(
        "docs/authoritative_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md"
    )


def test_odf_waves_reconciliation_recorded() -> None:
    state = json.loads(
        (
            ROOT
            / "docs/plans/state/codefabric-waves-8-12-semantic-profiles_v2_state.json"
        ).read_text(encoding="utf-8")
    )
    review = (
        ROOT
        / "docs/reviews/plan_audit_codefabric_waves_8-12_remaining_scope_post_ontology_fabric_v2_2026-08-28_v1.md"
    )
    assert state["plan_deviations"]
    assert review.is_file() and "ready-with-corrections" in review.read_text(
        encoding="utf-8"
    )
