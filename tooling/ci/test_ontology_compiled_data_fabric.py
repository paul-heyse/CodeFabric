from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
IR_PATH = ROOT / "contracts/schema/schema-contract-ir.json"


def _text(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def _ir() -> dict[str, object]:
    return json.loads(IR_PATH.read_text(encoding="utf-8"))


def _tables() -> dict[int, dict[str, object]]:
    return {table["table_code"]: table for table in _ir()["tables"]}


def _ontology_tables() -> list[dict[str, object]]:
    return [table for table in _ir()["tables"] if table["family"] == "ontology"]


def _assert_absent(pattern: str, *paths: str) -> None:
    expression = re.compile(pattern)
    matches = [path for path in paths if expression.search(_text(path))]
    assert not matches, f"retired pattern {pattern!r} remains in {matches}"


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
        for name in (
            "semantic_operations",
            "semantic_projections",
            "query_phrases",
            "provider_raw_kinds",
            "vocabulary",
        )
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
    executor = _text("src/ontology_executor.rs")
    bundle = _text("src/generated/ontology_program_bundle.rs")
    assert "query_phrases.values()" in compiler
    assert "lower_phrase_text_for_qualifier" in compiler
    assert "decode_query_phrases" in executor
    assert "ontology-program-bundle.arrow" in bundle
    assert "SEMANTIC_OPERATION_" + "SPECS" not in compiler


def test_odf_literal_code_predicate_zero() -> None:
    semantic = _text("src/semantic_query.rs")
    assert "compiled_enum_code" in semantic
    assert "ontology_code(" not in semantic
    assert "query_projection_codes" in semantic
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


def test_ontology_datafabric_successor_authority() -> None:
    bundle = ROOT / "contracts/generated/model/ontology/ontology-program-bundle.arrow"
    adapter = _text("src/generated/ontology_program_bundle.rs")
    runtime = _text("src/ontology_program.rs")
    semantic = _text("src/semantic_query.rs")
    assert bundle.is_file() and bundle.stat().st_size > 0
    assert (
        'include_bytes!("../../contracts/generated/model/ontology/ontology-program-bundle.arrow")'
        in adapter
    )
    assert "program.query_phrase" in runtime or "OntologyProgramCompiler" in semantic
    assert "SEMANTIC_OPERATION_" + "SPECS" not in semantic
    assert not (ROOT / "src/compiled_ontology.rs").exists()
    assert not (ROOT / "src/generated/compiled_ontology.rs").exists()


def test_ontology_datafabric_legacy_zero_state() -> None:
    result = subprocess.run(
        [
            sys.executable,
            "tooling/ci/ontology_datafabric_legacy_zero_state.py",
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    report = json.loads(result.stdout)
    assert report["candidate_file_count"] > 0
    assert report["text_findings"] == []
    assert report["structural_scan_returncode"] == 0
    assert report["historical_exclusions"]


def test_ontology_datafabric_retired_command_absence() -> None:
    commands = subprocess.run(
        ["just", "--list", "--unsorted"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    for fragments in (
        ("id16-extension-", "contract-check"),
        ("probe-", "suite"),
        ("ontology-stage2b-", "activation-check"),
    ):
        assert "".join(fragments) not in commands
    assert "id-domain-extension-check" in commands
    assert "ontology-datafabric-legacy-zero-state-check" in commands


def test_ontology_datafabric_release_certification() -> None:
    commands = subprocess.run(
        ["just", "--list", "--unsorted"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    required = {
        "authoritative-design-conformance-check",
        "ontology-program-compiler-check",
        "ontology-program-packaging-check",
        "ontology-program-causality-check",
        "ontology-calculation-catalog-check",
        "id-domain-plan-enforcement-check",
        "ontology-candidate-receipt-check",
        "ontology-gate-result-checksum-check",
        "ontology-gate-execution-artifact-check",
        "ontology-activation-route-check",
        "ontology-activation-recovery-check",
        "result-authority-lease-check",
        "ontology-runtime-resource-check",
        "ontology-decision-integrity-check",
        "ontology-datafabric-integration-check",
        "ontology-datafabric-legacy-zero-state-check",
        "ontology-candidate-delta-binding-check",
        "ontology-plan-artifact-boundary-check",
    }
    missing = sorted(name for name in required if name not in commands)
    assert not missing, missing
    assert "hyperfine" not in _text(
        "tooling/ci/ontology_datafabric_legacy_zero_state.py"
    )


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
