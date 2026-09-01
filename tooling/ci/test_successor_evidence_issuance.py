"""Focused positive and falsification tests for WP33 evidence issuance."""

from __future__ import annotations

import copy
import hashlib
import json
import shutil
from pathlib import Path

import blake3
import pytest
import rfc8785

from tooling.ci.reissue_wp33_r3 import (
    _canonical_shortest_witness,
    _normalize_objective_relations,
    _objective_groups,
    _path_result_recipe,
    _query_source_context_recipe,
    claim_001,
    claim_003,
    claim_013,
    claim_014,
    claim_017,
    claim_review_basis,
    claim_review_rationale,
    derived_call_target_fixture,
    derived_partial_call_graph_expectation,
    refresh_issuance,
    rewrite_query_oracle_fixtures,
    update_query_claim,
)
from tooling.ci.successor_evidence_issuance import (
    EXPECTATIONS_PATH,
    FIXTURES_PATH,
    ISSUANCE_PATH,
    NORMATIVE_TAG_PATHS,
    PLAN_PATH,
    PRINCIPLES_PATH,
    ROOT,
    SuccessorEvidenceError,
    _apply_json_pointer,
    _authorization_scope_id,
    _canonical_sha256,
    _canonical_shortest_query_witness,
    _cbef_analysis_context_id,
    _record_unique_review_rationale,
    _source_semantic_rows,
    _validate_claim_review_specificity,
    _validate_family_inputs,
    _validate_fixture_mutation_semantics,
    _validate_objective_fact_inputs,
    _validate_objective_groups,
    _validate_path_result_recipe,
    _validate_programmatic_activation_chain,
    _validate_query_inputs,
    _validate_security_inputs,
    _validate_wire_inputs,
    validate_expectations,
    validate_expected_behavior_review,
    validate_fixtures,
    validate_negative_fixture_independence,
    validate_readiness,
    validate_transaction_integrity,
)

WIRE_AUTHORITY_PATHS = (
    Path("contracts/schema/cpg-semantic-query-response.schema.json"),
    Path("contracts/rpc/cpg_query_service.proto"),
    Path("contracts/rpc/provider_control.proto"),
)


def _load_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict)
    return value


def _load_jsonl(path: Path) -> list[dict[str, object]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def _write_json(path: Path, value: object) -> None:
    path.write_text(
        json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )


def _write_jsonl(path: Path, values: list[dict[str, object]]) -> None:
    path.write_text(
        "".join(
            json.dumps(value, separators=(",", ":"), ensure_ascii=False) + "\n"
            for value in values
        ),
        encoding="utf-8",
    )


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _copy_candidate_contract(destination: Path) -> Path:
    paths = (
        EXPECTATIONS_PATH,
        FIXTURES_PATH,
        PLAN_PATH,
        PRINCIPLES_PATH,
        *NORMATIVE_TAG_PATHS.values(),
        *WIRE_AUTHORITY_PATHS,
    )
    for path in paths:
        (destination / path).parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / path, destination / path)
    return destination


def _copy_current_issuance(destination: Path) -> Path:
    root = _copy_candidate_contract(destination)
    (root / ISSUANCE_PATH).parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT / ISSUANCE_PATH, root / ISSUANCE_PATH)
    return root


def _refresh_content_identity(root: Path, artifact_name: str, path: Path) -> None:
    issuance_path = root / ISSUANCE_PATH
    issuance = _load_json(issuance_path)
    artifacts = issuance["artifacts"]
    assert isinstance(artifacts, dict)
    artifact = artifacts[artifact_name]
    assert isinstance(artifact, dict)
    artifact["sha256"] = _sha256(root / path)
    expectation = artifacts["expectations"]
    fixtures = artifacts["negative_fixtures"]
    assert isinstance(expectation, dict)
    assert isinstance(fixtures, dict)
    projection = {
        "expectations_sha256": expectation["sha256"],
        "negative_fixtures_sha256": fixtures["sha256"],
    }
    issuance["reviewed_content_id"] = f"sha256:{_canonical_sha256(projection)}"
    _write_json(issuance_path, issuance)


def _expectation(claim_id: str) -> dict[str, object]:
    return next(
        row
        for row in _load_jsonl(ROOT / EXPECTATIONS_PATH)
        if row["claim_id"] == claim_id
    )


def _authored_query_oracle(
    claim_number: int, suffix: str | None = None
) -> tuple[dict[str, object], dict[str, object] | None]:
    claims = {
        f"RFV3-CLAIM-{number:03d}": update_query_claim(
            copy.deepcopy(_expectation(f"RFV3-CLAIM-{number:03d}"))
        )
        for number in (4, 6, 7, 8)
    }
    if suffix is None:
        return claims[f"RFV3-CLAIM-{claim_number:03d}"], None
    fixtures = copy.deepcopy(_load_jsonl(ROOT / FIXTURES_PATH))
    rewrite_query_oracle_fixtures(fixtures, claims)
    fixture = next(
        row
        for row in fixtures
        if row["fixture_id"] == f"RFV3-FIX-{claim_number:03d}-{suffix}"
    )
    return claims[f"RFV3-CLAIM-{claim_number:03d}"], fixture


def _authored_source_context_oracle() -> dict[str, object]:
    return update_query_claim(copy.deepcopy(_expectation("RFV3-CLAIM-011")))


def _authored_activation_oracle() -> dict[str, object]:
    return claim_013(copy.deepcopy(_expectation("RFV3-CLAIM-013")))


def _mutated_query_fixture_inputs(
    claim: dict[str, object], fixture: dict[str, object]
) -> dict[str, object]:
    universe = claim["complete_input_universe"]
    mutation = fixture["mutation"]
    assert isinstance(universe, dict)
    assert isinstance(mutation, dict)
    inputs = copy.deepcopy(universe["inputs"])
    assert isinstance(inputs, dict)
    role = mutation["input_role"]
    assert isinstance(role, str)
    if role == "$input_universe":
        after = mutation["after"]
        assert isinstance(after, dict)
        inputs.update(copy.deepcopy(after))
    else:
        inputs[role] = _apply_json_pointer(
            inputs[role],
            mutation["json_pointer"],
            mutation["after"],
            str(fixture["fixture_id"]),
        )
    return inputs


def test_int_find_entity_coverage_closes_every_admitted_candidate() -> None:
    claim, _ = _authored_query_oracle(4)
    universe = claim["complete_input_universe"]
    decoded = claim["decoded_expectation"]
    assert isinstance(universe, dict)
    assert isinstance(decoded, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    relations = inputs["admitted_relations"]
    coverage = inputs["producer_coverage"]
    assert isinstance(relations, dict)
    assert isinstance(coverage, dict)
    rows = relations["entity_rows"]
    dictionary = relations["entity_dictionary"]
    covered = coverage["covered_entity_ids"]
    assert isinstance(rows, list)
    assert isinstance(dictionary, dict)
    assert isinstance(covered, list)
    assert set(dictionary) == {row["entity_id"] for row in rows}
    covered.pop()
    with pytest.raises(
        SuccessorEvidenceError,
        match="find-entity coverage does not close admitted rows",
    ):
        _validate_family_inputs(
            ROOT,
            str(claim["claim_family"]),
            inputs,
            decoded,
            "find-entity incomplete candidate coverage",
        )


def test_int_find_entity_self_consistent_wrong_selection_is_rejected() -> None:
    claim, _ = _authored_query_oracle(4)
    universe = claim["complete_input_universe"]
    decoded = claim["decoded_expectation"]
    assert isinstance(universe, dict)
    assert isinstance(decoded, dict)
    inputs = universe["inputs"]
    rows = decoded["rows"]
    assert isinstance(inputs, dict)
    assert isinstance(rows, list)
    response = rows[0][0]
    assert isinstance(response, dict)
    candidate_rows = inputs["admitted_relations"]["entity_rows"]
    assert isinstance(candidate_rows, list)
    callable_row = copy.deepcopy(
        next(
            row for row in candidate_rows if row["representation"] == "semantic_entity"
        )
    )
    callable_row.pop("alias")
    callable_id = callable_row["entity_id"]
    response["entities"] = {callable_id: callable_row}
    result = response["query_results"][0]
    assert isinstance(result, dict)
    result["entity_ids"] = [callable_id]
    result["resolved_semantics"] = {
        "looking_for": "function",
        "representation": "semantic_entity",
        "semantic_kind": "function_declaration",
    }
    with pytest.raises(
        SuccessorEvidenceError,
        match="find-entity result is not exactly derived",
    ):
        _validate_family_inputs(
            ROOT,
            str(claim["claim_family"]),
            inputs,
            decoded,
            "find-entity wrong selected row",
        )


def test_int_partial_follow_retains_known_facts_and_typed_remainder() -> None:
    claim, fixture = _authored_query_oracle(6, "N")
    assert fixture is not None
    mutated_inputs = _mutated_query_fixture_inputs(claim, fixture)
    expected = fixture["expected_decoded"]
    assert isinstance(expected, dict)
    result = expected["query_result"]
    assert isinstance(result, dict)
    result["facts"] = {}
    with pytest.raises(
        SuccessorEvidenceError,
        match="retain known facts plus a typed unknown remainder",
    ):
        _validate_fixture_mutation_semantics(
            claim,
            fixture,
            mutated_inputs,
            "partial follow without known fact records",
        )


def test_int_follow_causal_edge_requires_atomic_coverage_membership() -> None:
    claim, fixture = _authored_query_oracle(6, "C")
    assert fixture is not None
    mutated_inputs = _mutated_query_fixture_inputs(claim, fixture)
    coverage = mutated_inputs["producer_coverage"]
    assert isinstance(coverage, dict)
    covered_fact_ids = coverage["covered_fact_ids"]
    assert isinstance(covered_fact_ids, list)
    covered_fact_ids.pop()
    with pytest.raises(SuccessorEvidenceError, match="follow-edge result differs"):
        _validate_fixture_mutation_semantics(
            claim,
            fixture,
            mutated_inputs,
            "follow edge without atomic coverage membership",
        )


def test_int_shortest_path_rejects_a_longer_valid_witness() -> None:
    claim, _ = _authored_query_oracle(7)
    universe = claim["complete_input_universe"]
    decoded = claim["decoded_expectation"]
    assert isinstance(universe, dict)
    assert isinstance(decoded, dict)
    inputs = universe["inputs"]
    rows = decoded["rows"]
    assert isinstance(inputs, dict)
    assert isinstance(rows, list)
    response = rows[0][0]
    assert isinstance(response, dict)
    paths = response["paths"]
    assert isinstance(paths, dict)
    path = next(iter(paths.values()))
    assert isinstance(path, dict)
    path["ordered_entity_ids"] = [
        "entity:function:cccccccccccccccccccccccccccccccc",
        "entity:function:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "entity:function:99999999999999999999999999999999",
        "entity:function:ffffffffffffffffffffffffffffffff",
    ]
    path["ordered_fact_ids"] = [
        "fact:call:55555555555555555555555555555555",
        "fact:call:66666666666666666666666666666666",
        "fact:call:77777777777777777777777777777777",
    ]
    path["length"] = 3
    with pytest.raises(
        SuccessorEvidenceError,
        match="exact canonical shortest witness",
    ):
        _validate_family_inputs(
            ROOT,
            str(claim["claim_family"]),
            inputs,
            decoded,
            "longer valid path witness",
        )


def test_int_path_result_cbef_matches_rust_domain_18_known_answer() -> None:
    recipe = _path_result_recipe(
        workspace_id=f"workspace:{'00' * 16}",
        analysis_context_id=f"context:{'11' * 16}",
        fabric_epoch_id=f"fabric-epoch:{'22' * 16}",
        policy_identity="policy:r1",
        ordered_entity_ids=[
            f"entity:function:{'44' * 16}",
            f"entity:function:{'45' * 16}",
        ],
        ordered_fact_ids=[f"fact:call:{'55' * 16}"],
    )

    assert recipe["output_id"] == "path:959e262ba970b5e61f5b3e638a998694"
    assert recipe["digest"]["full_digest_hex"] == (
        "959e262ba970b5e61f5b3e638a9986941e86ded7a0e00a0cd2b6de90afc03e1d"
    )
    assert recipe["record_domain"] == {"code": 18, "name": "PATH_RESULT"}
    assert [field["tag"] for field in recipe["fields"]] == [1, 2, 3, 4, 5, 6]
    assert [field["type_code"]["name"] for field in recipe["fields"]] == [
        "ID",
        "ID",
        "ID",
        "UTF8",
        "ORDERED_LIST",
        "ORDERED_LIST",
    ]
    assert recipe["excluded"] == [
        "path length",
        "witness provenance",
        "certainty summary",
    ]
    assert (
        _validate_path_result_recipe(
            recipe,
            workspace_id=f"workspace:{'00' * 16}",
            analysis_context_id=f"context:{'11' * 16}",
            fabric_epoch_id=f"fabric-epoch:{'22' * 16}",
            policy_identity="policy:r1",
            ordered_entity_ids=[
                f"entity:function:{'44' * 16}",
                f"entity:function:{'45' * 16}",
            ],
            ordered_fact_ids=[f"fact:call:{'55' * 16}"],
            context="Rust domain-18 KAT",
        )
        == "path:959e262ba970b5e61f5b3e638a998694"
    )


def test_int_path_result_validator_rejects_recipe_field_drift() -> None:
    claim, _ = _authored_query_oracle(7)
    universe = claim["complete_input_universe"]
    decoded = claim["decoded_expectation"]
    assert isinstance(universe, dict)
    assert isinstance(decoded, dict)
    inputs = universe["inputs"]
    response = decoded["rows"][0][0]
    assert isinstance(inputs, dict)
    assert isinstance(response, dict)
    path = next(iter(response["paths"].values()))
    assert isinstance(path, dict)
    recipe = path["identity_recipe"]
    assert isinstance(recipe, dict)
    fields = recipe["fields"]
    assert isinstance(fields, list)
    entity_field = fields[4]
    assert isinstance(entity_field, dict)
    entity_field["value"] = list(reversed(entity_field["value"]))

    with pytest.raises(SuccessorEvidenceError, match="CBEF identity recipe differs"):
        _validate_family_inputs(
            ROOT,
            str(claim["claim_family"]),
            inputs,
            decoded,
            "path recipe field drift",
        )


def test_int_path_result_causal_fixture_rejects_non_cbef_recipe() -> None:
    claim, fixture = _authored_query_oracle(7, "C")
    assert fixture is not None
    mutated_inputs = _mutated_query_fixture_inputs(claim, fixture)
    expected = fixture["expected_decoded"]
    assert isinstance(expected, dict)
    result = expected["query_result"]
    assert isinstance(result, dict)
    identity_contract = result["identity_contract"]
    assert isinstance(identity_contract, dict)
    _validate_fixture_mutation_semantics(
        claim,
        fixture,
        mutated_inputs,
        "causal path with CBEF identity recipe",
    )
    identity_contract["identity_recipe"] = {
        "recipe_version": "codefabric.canonical-public-id.v1"
    }

    with pytest.raises(SuccessorEvidenceError, match="CBEF identity recipe differs"):
        _validate_fixture_mutation_semantics(
            claim,
            fixture,
            mutated_inputs,
            "causal path with legacy identity recipe",
        )


def test_int_query_source_context_cbef_matches_rust_known_answer() -> None:
    recipe = _query_source_context_recipe(
        workspace_id=f"workspace:{'00' * 16}",
        analysis_context_id=f"context:{'11' * 16}",
        snapshot_id=f"snapshot:{'33' * 16}",
        entity_id=f"entity:function:{'44' * 16}",
        source_file_id=f"file:{'66' * 16}",
        source_generation=2,
        source_content_digest=f"b3:{'cc' * 32}",
        delivered_start_byte=3,
        delivered_end_byte=8,
        delivered_content_digest=f"b3:{'dd' * 32}",
        disclosure_scope_id=f"access-scope:{'77' * 16}",
        policy_identity="policy:r1",
        context_kind="EXACT_SOURCE_SPAN",
    )
    assert recipe["output_id"] == "context:fb0ea7d9039e939dc039398e082771be"
    assert (
        recipe["digest"]["full_digest_hex"]
        == "fb0ea7d9039e939dc039398e082771be111f7a9fcbc0dd0579390fe53aaa884c"
    )


def test_int_source_context_uses_released_ingress_and_closed_cbef_identities() -> None:
    claim = _authored_source_context_oracle()
    universe = claim["complete_input_universe"]
    decoded = claim["decoded_expectation"]
    assert isinstance(universe, dict)
    assert isinstance(decoded, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    request = inputs["request_envelope"]["decoded"]["queries"][0]
    span = inputs["admitted_relations"]["entity_span"]
    access = inputs["access_scope"]
    response = decoded["rows"][0][0]
    source_context = next(iter(response["source_contexts"].values()))
    assert request["context"] == "exact source span"
    assert inputs["resource_limits"]["max_source_bytes"] == 8
    assert str(span["source_file_id"]).startswith("file:")
    assert access["identity_recipe"]["record_domain"] == {
        "code": 22,
        "name": "ACCESS_SCOPE",
    }
    assert source_context["identity_recipe"]["record_domain"] == {
        "code": 21,
        "name": "QUERY_SOURCE_CONTEXT",
    }
    _validate_family_inputs(
        ROOT,
        str(claim["claim_family"]),
        inputs,
        decoded,
        "source context closed identities",
    )


def test_int_source_context_rejects_noncanonical_file_and_legacy_recipe() -> None:
    claim = _authored_source_context_oracle()
    universe = claim["complete_input_universe"]
    decoded = claim["decoded_expectation"]
    assert isinstance(universe, dict)
    assert isinstance(decoded, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    span = inputs["admitted_relations"]["entity_span"]
    span["source_file_id"] = "source-file:fixture-py"
    with pytest.raises(SuccessorEvidenceError, match="exact CBEF SOURCE_FILE"):
        _validate_family_inputs(
            ROOT,
            str(claim["claim_family"]),
            inputs,
            decoded,
            "source context invalid file",
        )

    claim = _authored_source_context_oracle()
    universe = claim["complete_input_universe"]
    decoded = claim["decoded_expectation"]
    assert isinstance(universe, dict)
    assert isinstance(decoded, dict)
    inputs = universe["inputs"]
    response = decoded["rows"][0][0]
    source_context = next(iter(response["source_contexts"].values()))
    source_context["identity_recipe"] = {
        "recipe_version": "codefabric.canonical-public-id.v1"
    }
    with pytest.raises(SuccessorEvidenceError, match="CBEF identity recipe differs"):
        _validate_family_inputs(
            ROOT,
            str(claim["claim_family"]),
            inputs,
            decoded,
            "source context legacy recipe",
        )


def test_int_source_context_rejects_access_scope_recipe_drift() -> None:
    claim = _authored_source_context_oracle()
    universe = claim["complete_input_universe"]
    decoded = claim["decoded_expectation"]
    assert isinstance(universe, dict)
    assert isinstance(decoded, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    inputs["access_scope"]["allowed_metadata"].append("physical_plan")
    with pytest.raises(SuccessorEvidenceError, match="source disclosure scope"):
        _validate_family_inputs(
            ROOT,
            str(claim["claim_family"]),
            inputs,
            decoded,
            "source context stale access identity",
        )


def test_int_shortest_path_is_independent_of_admitted_edge_order() -> None:
    claim, _ = _authored_query_oracle(7)
    universe = claim["complete_input_universe"]
    decoded = claim["decoded_expectation"]
    assert isinstance(universe, dict)
    assert isinstance(decoded, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    edges = inputs["admitted_relations"]["edges"]
    assert isinstance(edges, list)
    edges.reverse()
    _validate_family_inputs(
        ROOT,
        str(claim["claim_family"]),
        inputs,
        decoded,
        "permuted admitted path edges",
    )


def test_int_shortest_path_tie_breaks_by_ordered_fact_sequence() -> None:
    start = "entity:function:00000000000000000000000000000000"
    lexically_later_entity = "entity:function:ffffffffffffffffffffffffffffffff"
    lexically_earlier_entity = "entity:function:11111111111111111111111111111111"
    target = "entity:function:99999999999999999999999999999999"
    edges = [
        {
            "fact_id": "fact:call:11111111111111111111111111111111",
            "statement": {
                "subject": start,
                "predicate": "calls",
                "object": lexically_later_entity,
            },
        },
        {
            "fact_id": "fact:call:22222222222222222222222222222222",
            "statement": {
                "subject": lexically_later_entity,
                "predicate": "calls",
                "object": target,
            },
        },
        {
            "fact_id": "fact:call:33333333333333333333333333333333",
            "statement": {
                "subject": start,
                "predicate": "calls",
                "object": lexically_earlier_entity,
            },
        },
        {
            "fact_id": "fact:call:44444444444444444444444444444444",
            "statement": {
                "subject": lexically_earlier_entity,
                "predicate": "calls",
                "object": target,
            },
        },
    ]
    expected_entities = [start, lexically_later_entity, target]
    expected_facts = [
        "fact:call:11111111111111111111111111111111",
        "fact:call:22222222222222222222222222222222",
    ]
    assert _canonical_shortest_witness(
        edges,
        start=start,
        target=target,
        families=["calls"],
        maximum_length=2,
    ) == (expected_entities, expected_facts)
    assert _canonical_shortest_query_witness(
        edges,
        start=start,
        target=target,
        families={"calls"},
        maximum_length=2,
        context="equal-length path tie",
    ) == (expected_entities, expected_facts)


def test_int_path_causal_edge_removal_requires_atomic_coverage_removal() -> None:
    claim, fixture = _authored_query_oracle(7, "C")
    assert fixture is not None
    mutated_inputs = _mutated_query_fixture_inputs(claim, fixture)
    coverage = mutated_inputs["producer_coverage"]
    assert isinstance(coverage, dict)
    fact_ids = coverage["fact_ids"]
    assert isinstance(fact_ids, list)
    fact_ids.append("fact:call:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    with pytest.raises(
        SuccessorEvidenceError,
        match="path coverage does not close the admitted graph",
    ):
        _validate_fixture_mutation_semantics(
            claim,
            fixture,
            mutated_inputs,
            "removed path edge with stale coverage membership",
        )


def test_int_typed_pattern_rejects_a_claimed_no_match_with_known_binding() -> None:
    claim, _ = _authored_query_oracle(8)
    universe = claim["complete_input_universe"]
    decoded = claim["decoded_expectation"]
    assert isinstance(universe, dict)
    assert isinstance(decoded, dict)
    inputs = universe["inputs"]
    rows = decoded["rows"]
    assert isinstance(inputs, dict)
    assert isinstance(rows, list)
    response = rows[0][0]
    assert isinstance(response, dict)
    result = response["query_results"][0]
    assert isinstance(result, dict)
    result["bindings"] = []
    result["entity_ids"] = []
    result["fact_ids"] = []
    with pytest.raises(
        SuccessorEvidenceError,
        match="pattern result is not derived from typed nodes",
    ):
        _validate_family_inputs(
            ROOT,
            str(claim["claim_family"]),
            inputs,
            decoded,
            "typed pattern false no-match",
        )


def test_int_partial_pattern_retains_known_bindings_and_facts() -> None:
    claim, fixture = _authored_query_oracle(8, "N")
    assert fixture is not None
    mutated_inputs = _mutated_query_fixture_inputs(claim, fixture)
    expected = fixture["expected_decoded"]
    assert isinstance(expected, dict)
    result = expected["query_result"]
    assert isinstance(result, dict)
    result["facts"] = {}
    with pytest.raises(
        SuccessorEvidenceError,
        match="retain known typed bindings/facts and an explicit remainder",
    ):
        _validate_fixture_mutation_semantics(
            claim,
            fixture,
            mutated_inputs,
            "partial pattern without known facts",
        )


def test_int_pattern_causal_edge_requires_matching_coverage_membership() -> None:
    claim, fixture = _authored_query_oracle(8, "C")
    assert fixture is not None
    mutated_inputs = _mutated_query_fixture_inputs(claim, fixture)
    coverage = mutated_inputs["producer_coverage"]
    assert isinstance(coverage, dict)
    covered_fact_ids = coverage["covered_fact_ids"]
    assert isinstance(covered_fact_ids, list)
    covered_fact_ids.pop()
    with pytest.raises(
        SuccessorEvidenceError,
        match="pattern coverage does not close owner/family/context facts",
    ):
        _validate_fixture_mutation_semantics(
            claim,
            fixture,
            mutated_inputs,
            "pattern edge without coverage membership",
        )


def test_int_provider_fixture_omits_unconsumed_rust_tree_sitter_pin() -> None:
    authored = claim_001(copy.deepcopy(_expectation("RFV3-CLAIM-001")))
    universe = authored["complete_input_universe"]
    decoded = authored["decoded_expectation"]
    assert isinstance(universe, dict)
    assert isinstance(decoded, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    pins = inputs["provider_release_pins"]
    requests = inputs["provider_requests"]
    assert isinstance(pins, dict)
    assert isinstance(requests, list)
    assert "tree_sitter_rust" not in pins
    assert {
        request["provider_id"]
        for request in requests
        if isinstance(request, dict)
        and request["provider_id"].startswith("tree-sitter")
    } == {"tree-sitter-python"}
    _validate_family_inputs(
        ROOT, "exact_provider_facts", inputs, decoded, "claim 001 focused proof"
    )

    inert = copy.deepcopy(inputs)
    inert["provider_release_pins"]["tree_sitter_rust"] = "0.24.2"
    with pytest.raises(SuccessorEvidenceError, match="provider release vector differs"):
        _validate_family_inputs(
            ROOT,
            "exact_provider_facts",
            inert,
            decoded,
            "claim 001 inert pin proof",
        )


def test_int_pyrefly_expectation_uses_production_schema_and_native_target() -> None:
    authored = claim_001(copy.deepcopy(_expectation("RFV3-CLAIM-001")))
    inputs = authored["complete_input_universe"]["inputs"]
    request = next(
        request
        for request in inputs["provider_requests"]
        if request["provider_id"] == "pyrefly"
    )
    row = next(
        row for row in authored["decoded_expectation"]["rows"] if row[0] == "pyrefly"
    )

    assert request["schema_contract"]["schema_identity"] == (
        "b3:eaa5e3fd620822805cc6f4eb3fceca506f7ccabfcd1bdfa1bd15676e70af9169"
    )
    assert row[3] == request["schema_contract"]["schema_identity"]
    assert row[6]["qualified_target"] == "builtins.abs"
    _validate_family_inputs(
        ROOT,
        "exact_provider_facts",
        inputs,
        authored["decoded_expectation"],
        "claim 001 production Pyrefly schema",
    )


def test_int_partial_call_graph_preserves_known_edge_and_adds_remainder() -> None:
    authored = claim_003(copy.deepcopy(_expectation("RFV3-CLAIM-003")))
    partial = derived_call_target_fixture(partial=True)
    inputs = copy.deepcopy(authored["complete_input_universe"]["inputs"])
    inputs["provider_call_targets"] = partial["provider_call_targets"]
    expected = derived_partial_call_graph_expectation(partial)
    fixture = {
        "kind": "negative",
        "mutation": {
            "input_role": "provider_call_targets",
            "json_pointer": "",
            "before": authored["complete_input_universe"]["inputs"][
                "provider_call_targets"
            ],
            "after": partial["provider_call_targets"],
        },
        "expected_decoded": expected,
    }
    _validate_fixture_mutation_semantics(
        authored, fixture, inputs, "claim 003 partial proof"
    )
    known_rows = expected["known_facts"]["rows"]
    unknown_rows = expected["unknown_remainder"]["rows"]
    assert len(known_rows) == 1
    assert known_rows[0][3] == "resolved"
    assert len(unknown_rows) == 1
    assert known_rows[0][0] != unknown_rows[0][0]

    erased = copy.deepcopy(fixture)
    erased["expected_decoded"]["known_facts"]["rows"] = []
    with pytest.raises(
        SuccessorEvidenceError, match="explicit call-graph unknown differs"
    ):
        _validate_fixture_mutation_semantics(
            authored, erased, inputs, "claim 003 erased known edge proof"
        )


def test_int_retrieve_facts_derives_known_and_unknown_property_identities() -> None:
    baseline_source = _expectation("RFV3-CLAIM-005")
    authored = update_query_claim(copy.deepcopy(baseline_source))
    universe = authored["complete_input_universe"]
    decoded = authored["decoded_expectation"]
    assert isinstance(universe, dict)
    assert isinstance(decoded, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    _validate_query_inputs(
        "query_retrieve_facts", inputs, decoded, "claim 005 focused proof"
    )
    relations = inputs["admitted_relations"]
    response = decoded["rows"][0][0]
    assert isinstance(relations, dict)
    assert isinstance(response, dict)
    assert relations["property_kind_registry"]["rows"] == [
        {"property_kind": "type", "property_kind_code": 1},
        {"property_kind": "UNKNOWN_EFFECT", "property_kind_code": 2},
    ]
    admitted_type = relations["fact_rows"][0]
    known = next(
        fact for fact in response["facts"].values() if fact["fact_kind"] == "type"
    )
    unknown = next(
        fact for fact in response["facts"].values() if fact["fact_kind"] == "unknown"
    )
    admitted_projection = copy.deepcopy(admitted_type)
    admitted_projection.pop("alias", None)
    assert known == admitted_projection
    assert known["identity_recipe"]["record_domain"] == {
        "code": 10,
        "name": "PROPERTY_FACT",
    }
    assert unknown["identity_recipe"]["record_domain"] == {
        "code": 10,
        "name": "PROPERTY_FACT",
    }
    assert relations["input_set_identity"]["record_domain"] == {
        "code": 19,
        "name": "OBJECTIVE_INPUT_SET",
    }
    assert (
        admitted_type["direct_provenance"]["input_set_id"]
        == relations["input_set_identity"]["output_id"]
    )
    effects_coverage = next(
        row for row in relations["coverage_rows"] if row["family"] == "effects"
    )
    assert effects_coverage["source_identity"]["file_id"].startswith("file:")
    assert effects_coverage["source_identity"]["canonical_path_bytes_hex"] == (
        b"fixture.py".hex()
    )
    assert unknown["property_kind_code"] == 2
    assert unknown["statement"] == {
        "subject": unknown["owner_id"],
        "predicate": "UNKNOWN_EFFECT",
        "object": "effects",
    }
    assert unknown["identity_recipe"]["fields"][4]["value"] == {
        "variant": 50,
        "member_type": "UTF8",
        "value": "effects",
    }

    mutated_source = copy.deepcopy(baseline_source)
    mutated_type = mutated_source["complete_input_universe"]["inputs"][
        "admitted_relations"
    ]["fact_rows"][0]
    mutated_type["statement"]["object"]["return"]["name"] = "str"
    mutated = update_query_claim(mutated_source)
    mutated_inputs = mutated["complete_input_universe"]["inputs"]
    mutated_decoded = mutated["decoded_expectation"]
    _validate_query_inputs(
        "query_retrieve_facts",
        mutated_inputs,
        mutated_decoded,
        "claim 005 causal proof",
    )
    mutated_known = next(
        fact
        for fact in mutated_decoded["rows"][0][0]["facts"].values()
        if fact["fact_kind"] == "type"
    )
    mutated_unknown = next(
        fact
        for fact in mutated_decoded["rows"][0][0]["facts"].values()
        if fact["fact_kind"] == "unknown"
    )
    assert mutated_known["fact_id"] != known["fact_id"]
    assert mutated_unknown["fact_id"] == unknown["fact_id"]
    assert (
        mutated_inputs["admitted_relations"]["input_set_identity"]["output_id"]
        != relations["input_set_identity"]["output_id"]
    )

    stale = copy.deepcopy(inputs)
    stale["admitted_relations"]["fact_rows"][0]["statement"]["object"]["return"][
        "name"
    ] = "str"
    with pytest.raises(SuccessorEvidenceError, match="not derived from the admitted"):
        _validate_query_inputs(
            "query_retrieve_facts", stale, decoded, "claim 005 stale output proof"
        )


def test_int_retrieve_facts_rejects_label_ids_and_stale_input_set_provenance() -> None:
    authored = update_query_claim(copy.deepcopy(_expectation("RFV3-CLAIM-005")))
    universe = authored["complete_input_universe"]
    decoded = authored["decoded_expectation"]
    assert isinstance(universe, dict)
    assert isinstance(decoded, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)

    stale_source = copy.deepcopy(inputs)
    effects = next(
        row
        for row in stale_source["admitted_relations"]["coverage_rows"]
        if row["family"] == "effects"
    )
    effects["source_identity"]["file_id"] = "source-file:semantic-fixture"
    with pytest.raises(
        SuccessorEvidenceError,
        match="unknown source identity is not its closed CBEF/content recipe",
    ):
        _validate_query_inputs(
            "query_retrieve_facts",
            stale_source,
            decoded,
            "claim 005 label-shaped source identity",
        )

    stale_input_set = copy.deepcopy(inputs)
    stale_input_set["admitted_relations"]["input_set_identity"]["output_id"] = (
        "input-set:00000000000000000000000000000000"
    )
    with pytest.raises(
        SuccessorEvidenceError,
        match="retrieve-facts input set CBEF identity recipe differs",
    ):
        _validate_query_inputs(
            "query_retrieve_facts",
            stale_input_set,
            decoded,
            "claim 005 stale input-set provenance",
        )


def test_int_combine_request_closes_real_producer_result_dag() -> None:
    authored = update_query_claim(copy.deepcopy(_expectation("RFV3-CLAIM-009")))
    universe = authored["complete_input_universe"]
    decoded = authored["decoded_expectation"]
    assert isinstance(universe, dict)
    assert isinstance(decoded, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    _validate_query_inputs(
        "query_combine_results", inputs, decoded, "claim 009 focused proof"
    )

    dangling = copy.deepcopy(inputs)
    relations = dangling["admitted_relations"]
    assert isinstance(relations, dict)
    producer_inputs = relations["producer_inputs"]
    assert isinstance(producer_inputs, dict)
    del producer_inputs["right"]
    with pytest.raises(SuccessorEvidenceError, match="dangling result reference"):
        _validate_query_inputs(
            "query_combine_results", dangling, decoded, "claim 009 dangling proof"
        )

    circular = copy.deepcopy(inputs)
    request = circular["request_envelope"]
    assert isinstance(request, dict)
    envelope = request["decoded"]
    assert isinstance(envelope, dict)
    blocks = envelope["queries"]
    assert isinstance(blocks, list)
    blocks[0]["where"] = [
        {"relation": "query.prior_result.left", "predicate": "member"}
    ]
    request["canonical_json"] = rfc8785.dumps(envelope).decode("utf-8")
    with pytest.raises(SuccessorEvidenceError, match="independent base relation"):
        _validate_query_inputs(
            "query_combine_results", circular, decoded, "claim 009 circular proof"
        )


def test_int_objective_property_change_rederives_transitive_identities() -> None:
    authored = update_query_claim(copy.deepcopy(_expectation("RFV3-CLAIM-010")))
    universe = authored["complete_input_universe"]
    decoded = authored["decoded_expectation"]
    assert isinstance(universe, dict)
    assert isinstance(decoded, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    relations = inputs["admitted_relations"]
    epoch = inputs["pinned_epoch"]
    assert isinstance(relations, dict)
    assert isinstance(epoch, dict)
    policy_identity = str(epoch["policy_release"])
    input_set_id, _, grouped = _validate_objective_fact_inputs(
        relations, policy_identity, "claim 010 focused proof"
    )
    response = decoded["rows"][0][0]
    assert isinstance(response, dict)
    groups = response["groups"]
    assert isinstance(groups, dict)
    _validate_objective_groups(
        list(groups.values()),
        input_set_id=input_set_id,
        grouped_facts=grouped,
        context="claim 010 focused proof",
    )
    assert relations["input_set_identity"]["record_domain"] == {
        "code": 19,
        "name": "OBJECTIVE_INPUT_SET",
    }
    assert {
        group["identity_recipe"]["record_domain"]["code"] for group in groups.values()
    } == {20}
    assert {
        row["identity_recipe"]["record_domain"]["code"]
        for row in relations["syntax_rows"]
    } == {10}
    function_row = next(
        row
        for row in relations["syntax_rows"]
        if row["statement"]["object"] == "function_definition"
    )
    function_entity = relations["entity_dictionary"][function_row["owner_id"]]
    assert function_entity["semantic_kind"] == "function_syntax"

    changed = copy.deepcopy(relations)
    changed_row = next(
        row for row in changed["syntax_rows"] if row["statement"]["object"] == "call"
    )
    old_entity_id = changed_row["owner_id"]
    new_entity_id = old_entity_id.replace("entity:call-site:", "entity:identifier:")
    entity = changed["entity_dictionary"].pop(old_entity_id)
    entity["entity_id"] = new_entity_id
    entity["semantic_kind"] = "identifier_syntax"
    changed["entity_dictionary"][new_entity_id] = entity
    changed_row["owner_id"] = new_entity_id
    changed_row["statement"]["subject"] = new_entity_id
    changed_row["statement"]["object"] = "identifier"
    with pytest.raises(SuccessorEvidenceError, match="CBEF identity recipe differs"):
        _validate_objective_fact_inputs(
            changed, policy_identity, "claim 010 stale identity proof"
        )

    old_fact_id = changed_row["fact_id"]
    _normalize_objective_relations(changed, policy_identity)
    changed_input_set_id, _, changed_grouped = _validate_objective_fact_inputs(
        changed, policy_identity, "claim 010 rederived proof"
    )
    changed_groups = _objective_groups(changed)
    changed_group_ids = _validate_objective_groups(
        list(changed_groups.values()),
        input_set_id=changed_input_set_id,
        grouped_facts=changed_grouped,
        context="claim 010 rederived proof",
    )
    assert changed_row["fact_id"] != old_fact_id
    assert changed_input_set_id != input_set_id
    assert set(changed_group_ids) != set(groups)

    zero = copy.deepcopy(relations)
    zero["property_kind_registry"]["rows"][0]["property_kind_code"] = 0
    with pytest.raises(SuccessorEvidenceError, match="zero, duplicate, or invalid"):
        _validate_objective_fact_inputs(zero, policy_identity, "zero allocation proof")

    duplicate = copy.deepcopy(relations)
    duplicate["property_kind_registry"]["rows"].append(
        {"property_kind": "other", "property_kind_code": 1}
    )
    with pytest.raises(SuccessorEvidenceError, match="zero, duplicate, or invalid"):
        _validate_objective_fact_inputs(
            duplicate, policy_identity, "duplicate allocation proof"
        )

    undeclared = copy.deepcopy(relations)
    undeclared["property_kind_registry"]["rows"] = [
        {"property_kind": "other", "property_kind_code": 2}
    ]
    with pytest.raises(SuccessorEvidenceError, match="declared kind allocation"):
        _validate_objective_fact_inputs(
            undeclared, policy_identity, "undeclared allocation proof"
        )


def test_int_access_scope_identity_changes_with_column_grants() -> None:
    authored = claim_014(copy.deepcopy(_expectation("RFV3-CLAIM-014")))
    universe = authored["complete_input_universe"]
    decoded = authored["decoded_expectation"]
    assert isinstance(universe, dict)
    assert isinstance(decoded, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    _validate_family_inputs(ROOT, "authorization", inputs, decoded, "claim 014 proof")
    assert inputs["access_scope"]["identity_recipe"]["record_domain"] == {
        "code": 22,
        "name": "ACCESS_SCOPE",
    }
    assert inputs["access_scope"]["identity_recipe"]["contract"]["version"] == "1.1"

    stale = copy.deepcopy(inputs)
    scope = stale["access_scope"]
    policy = stale["authorization_policy"]
    assert isinstance(scope, dict)
    assert isinstance(policy, dict)
    supplied = str(scope["scope_id"])
    scope["allowed_columns"]["public.entity"].append("qualified_name")
    derived = _authorization_scope_id(scope, policy)
    assert derived != supplied
    with pytest.raises(
        SuccessorEvidenceError, match="access-scope identity does not bind"
    ):
        _validate_family_inputs(
            ROOT, "authorization", stale, decoded, "claim 014 stale identity proof"
        )

    posture = copy.deepcopy(inputs)
    posture_scope = posture["access_scope"]
    assert isinstance(posture_scope, dict)
    posture_scope["execution_posture"].append("offline")
    assert _authorization_scope_id(posture_scope, policy) != supplied
    with pytest.raises(
        SuccessorEvidenceError, match="access-scope identity does not bind"
    ):
        _validate_family_inputs(
            ROOT, "authorization", posture, decoded, "claim 014 posture proof"
        )


def test_int_candidate_released_projection_rejects_internal_table_injection() -> None:
    authored = claim_017(copy.deepcopy(_expectation("RFV3-CLAIM-017")))
    universe = authored["complete_input_universe"]
    decoded = authored["decoded_expectation"]
    assert isinstance(universe, dict)
    assert isinstance(decoded, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    _validate_wire_inputs(ROOT, inputs, decoded, "claim 017 focused proof")

    leaked = copy.deepcopy(inputs)
    candidate = leaked["candidate_released_projection"]
    private = leaked["private_diagnostics"]
    assert isinstance(candidate, dict)
    assert isinstance(private, dict)
    candidate["internal_table"] = private["internal_table"]
    with pytest.raises(
        SuccessorEvidenceError, match="candidate released projection keys differ"
    ):
        _validate_wire_inputs(ROOT, leaked, decoded, "claim 017 injection proof")


def test_int_repository_evidence_transaction_is_closed_and_frozen() -> None:
    assert validate_transaction_integrity(ROOT) == 18


def test_int_corrupt_artifact_digest_is_rejected(tmp_path: Path) -> None:
    root = _copy_current_issuance(tmp_path / "repo")
    issuance_path = root / ISSUANCE_PATH
    issuance = _load_json(issuance_path)
    artifacts = issuance["artifacts"]
    assert isinstance(artifacts, dict)
    expectation = artifacts["expectations"]
    assert isinstance(expectation, dict)
    expectation["sha256"] = "0" * 64
    _write_json(issuance_path, issuance)
    with pytest.raises(
        SuccessorEvidenceError, match="frozen expectation identity changed"
    ):
        validate_transaction_integrity(root)


def test_int_duplicate_json_member_is_rejected(tmp_path: Path) -> None:
    root = _copy_current_issuance(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    lines = path.read_text(encoding="utf-8").splitlines()
    lines[0] = lines[0].replace(
        '{"claim_id":"RFV3-CLAIM-001",',
        '{"claim_id":"RFV3-CLAIM-001","claim_id":"RFV3-CLAIM-999",',
        1,
    )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    with pytest.raises(SuccessorEvidenceError, match="duplicate JSON member"):
        validate_transaction_integrity(root)


def test_int_claim_specific_input_role_omission_is_rejected(tmp_path: Path) -> None:
    root = _copy_current_issuance(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    universe = expectations[3]["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    inputs.pop("resource_limits")
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="input-role closure differs"):
        validate_transaction_integrity(root)


def test_int_unregistered_or_truncated_pin_is_rejected(tmp_path: Path) -> None:
    root = _copy_current_issuance(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    pins = expectations[0]["exact_pins"]
    assert isinstance(pins, dict)
    pins["deltalake"] = "git:43a0cf10"
    pins["invented_family_release"] = "v1"
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="exact pin set differs"):
        validate_transaction_integrity(root)


def test_int_source_file_identity_uses_the_closed_cbef_recipe(tmp_path: Path) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    images = expectations[0]["complete_input_universe"]["inputs"]["source_images"]
    assert isinstance(images, list)
    assert isinstance(images[0], dict)
    images[0]["file_id"] = "1" * 32
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="closed CBEF recipe"):
        validate_expectations(root)


@pytest.mark.parametrize("field", ["semantic_environment_id", "analysis_context_id"])
def test_int_call_target_context_is_derived_from_declared_inputs(
    tmp_path: Path, field: str
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    source_image = expectations[2]["complete_input_universe"]["inputs"][
        "provider_call_targets"
    ]["source_image"]
    assert isinstance(source_image, dict)
    source_image[field] = "1" * 64
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="call-target authority differs"):
        validate_expectations(root)


def test_int_nested_query_request_omission_is_rejected(tmp_path: Path) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    query = expectations[3]["complete_input_universe"]
    assert isinstance(query, dict)
    inputs = query["inputs"]
    assert isinstance(inputs, dict)
    request = inputs["request_envelope"]
    assert isinstance(request, dict)
    decoded = request["decoded"]
    assert isinstance(decoded, dict)
    queries = decoded["queries"]
    assert isinstance(queries, list)
    assert isinstance(queries[0], dict)
    queries[0].pop("where")
    request["canonical_json"] = rfc8785.dumps(decoded).decode("utf-8")
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="query block 1 keys differ"):
        validate_expectations(root)


def test_int_query_request_requires_exact_canonical_member_order(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    universe = expectations[3]["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    request = inputs["request_envelope"]
    assert isinstance(request, dict)
    request["canonical_json"] = json.dumps(
        request["decoded"], separators=(",", ":"), ensure_ascii=False
    )
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="not exact canonical JSON"):
        validate_expectations(root)


def test_int_query_state_outside_released_vocabulary_is_rejected(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    decoded = expectations[3]["decoded_expectation"]
    assert isinstance(decoded, dict)
    rows = decoded["rows"]
    assert isinstance(rows, list)
    assert isinstance(rows[0], list)
    response = rows[0][0]
    assert isinstance(response, dict)
    query_results = response["query_results"]
    assert isinstance(query_results, list)
    assert isinstance(query_results[0], dict)
    query_results[0]["dependency_state"] = "COMPLETE"
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="released vocabulary"):
        validate_expectations(root)


def test_int_released_wire_nested_schema_omission_is_rejected(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    decoded = expectations[16]["decoded_expectation"]
    assert isinstance(decoded, dict)
    rows = decoded["rows"]
    assert isinstance(rows, list)
    assert isinstance(rows[0], list)
    response = rows[0][0]
    assert isinstance(response, dict)
    query_results = response["query_results"]
    assert isinstance(query_results, list)
    assert isinstance(query_results[0], dict)
    query_results[0].pop("request")
    _write_jsonl(path, expectations)
    with pytest.raises(
        SuccessorEvidenceError,
        match="decoded public response differs from its candidate projection|violates released schema",
    ):
        validate_expectations(root)


def test_int_unresolved_normative_source_anchor_is_rejected(tmp_path: Path) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    expectations[0]["source_anchor"] = "GEN §999"
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="unresolved GEN §999"):
        validate_expectations(root)


def test_int_child_catalog_must_equal_the_authorized_relation_set(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    universe = expectations[13]["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    bindings = inputs["child_catalog_bindings"]
    assert isinstance(bindings, dict)
    installed = bindings["installed_relations"]
    assert isinstance(installed, list)
    installed.append("public.location")
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="exactly reduced"):
        validate_expectations(root)


def test_int_child_catalog_expansion_requires_an_epoch_provider_binding(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    universe = expectations[13]["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    epoch_catalog = inputs["epoch_provider_catalog"]
    assert isinstance(epoch_catalog, dict)
    relations = epoch_catalog["relations"]
    assert isinstance(relations, dict)
    relations.pop("public.location")
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="authorization delta differs"):
        validate_fixtures(root)


def test_int_recursive_authorization_does_not_require_a_magic_hidden_provider(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    universe = expectations[13]["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    epoch_catalog = inputs["epoch_provider_catalog"]
    assert isinstance(epoch_catalog, dict)
    relations = epoch_catalog["relations"]
    assert isinstance(relations, dict)
    relations.pop("internal.source_secret")
    _write_jsonl(path, expectations)
    # Exact recursive authorization is the intersection of the typed scope and
    # installed providers.  A denied relation need not remain installed merely
    # to act as a negative sentinel.
    assert len(validate_fixtures(root)) == 36


def test_int_activation_chain_cannot_cross_workspace_fence_identity(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    expectations[12] = _authored_activation_oracle()
    universe = expectations[12]["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    chain = inputs["activation_chain"]
    assert isinstance(chain, dict)
    events = chain["events"]
    assert isinstance(events, list)
    event = events[0]
    assert isinstance(event, dict)
    event["workspace_id"] = "ffffffffffffffffffffffffffffffff"
    with pytest.raises(SuccessorEvidenceError, match="FabricCommand binding differs"):
        _validate_programmatic_activation_chain(chain, "test")


def test_int_activation_chain_requires_exact_durable_relation_readback(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    expectations[12] = _authored_activation_oracle()
    universe = expectations[12]["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    chain = inputs["activation_chain"]
    assert isinstance(chain, dict)
    events = chain["events"]
    assert isinstance(events, list)
    event = events[0]
    assert isinstance(event, dict)
    event.pop("readback")
    with pytest.raises(SuccessorEvidenceError, match="activation event 1 keys differ"):
        _validate_programmatic_activation_chain(chain, "test")


def test_int_activation_table_version_reference_must_be_runtime_derived(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    expectations[12] = _authored_activation_oracle()
    universe = expectations[12]["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    chain = inputs["activation_chain"]
    assert isinstance(chain, dict)
    events = chain["events"]
    assert isinstance(events, list)
    event = events[0]
    assert isinstance(event, dict)
    pins = event["pins"]
    assert isinstance(pins, dict)
    pins["table_versions"] = "00" * 32
    with pytest.raises(SuccessorEvidenceError, match="table-version binding"):
        _validate_programmatic_activation_chain(chain, "test")


@pytest.mark.parametrize("mutation", ["missing", "extra", "activation-control"])
def test_int_activation_table_version_binding_is_the_exact_observation_relation_set(
    tmp_path: Path, mutation: str
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    expectations[12] = _authored_activation_oracle()
    universe = expectations[12]["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    chain = inputs["activation_chain"]
    assert isinstance(chain, dict)
    events = chain["events"]
    assert isinstance(events, list)
    event = events[0]
    assert isinstance(event, dict)
    pins = event["pins"]
    assert isinstance(pins, dict)
    binding = pins["table_versions"]
    assert isinstance(binding, dict)
    components = binding["components"]
    assert isinstance(components, list)
    if mutation == "missing":
        components.pop()
    elif mutation == "extra":
        components.append(
            {
                "relation_id": "fact.entity",
                "exact_delta_pin": {
                    "root": "publication_runtime_root",
                    "version": "publication_exact_version",
                },
            }
        )
    else:
        component = components[0]
        assert isinstance(component, dict)
        component["relation_id"] = "control.activation_event.v3"
    with pytest.raises(
        SuccessorEvidenceError,
        match="exactly the five programmatic observation histories",
    ):
        _validate_programmatic_activation_chain(chain, "test")


def test_int_activation_table_version_binding_rejects_a_literal_pin(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    expectations[12] = _authored_activation_oracle()
    universe = expectations[12]["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    chain = inputs["activation_chain"]
    assert isinstance(chain, dict)
    events = chain["events"]
    assert isinstance(events, list)
    event = events[0]
    assert isinstance(event, dict)
    pins = event["pins"]
    assert isinstance(pins, dict)
    binding = pins["table_versions"]
    assert isinstance(binding, dict)
    components = binding["components"]
    assert isinstance(components, list)
    component = components[0]
    assert isinstance(component, dict)
    component["exact_delta_pin"] = {"root": "file:///tmp/fixed", "version": 1}
    with pytest.raises(SuccessorEvidenceError, match="literal or sentinel pin"):
        _validate_programmatic_activation_chain(chain, "test")


def test_int_resource_terminal_retains_exact_artifact_lease_and_publication(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    decoded = expectations[14]["decoded_expectation"]
    assert isinstance(decoded, dict)
    rows = decoded["rows"]
    assert isinstance(rows, list)
    terminal_row = rows[0]
    assert isinstance(terminal_row, list)
    terminal = terminal_row[0]
    assert isinstance(terminal, dict)
    provenance = terminal["terminal_provenance"]
    assert isinstance(provenance, dict)
    provenance["publication_state"] = "not_published"
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="resource terminal differs"):
        validate_expectations(root)


def test_int_python_cfg_remainder_cannot_import_provider_evaluation_order(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    universe = expectations[2]["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    cfg = inputs["python_cfg_inputs"]
    assert isinstance(cfg, dict)
    provider_inputs = cfg["provider_inputs"]
    assert isinstance(provider_inputs, list)
    provider_inputs.append("provider.tree_sitter.evaluation_order")
    _write_jsonl(path, expectations)
    with pytest.raises(
        SuccessorEvidenceError,
        match="Python CFG authority differs",
    ):
        validate_expectations(root)


def test_int_python_cfg_remainder_has_no_executable_analysis_definition(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    universe = expectations[2]["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    definitions = inputs["analysis_definitions"]
    assert isinstance(definitions, list)
    stale = json.loads(json.dumps(definitions[0]))
    stale["family_id"] = "python.cfg_edge"
    definitions.append(stale)
    _write_jsonl(path, expectations)
    with pytest.raises(
        SuccessorEvidenceError,
        match="analysis-definition closure differs",
    ):
        validate_expectations(root)


def test_int_rust_control_support_closes_over_all_exact_native_inputs(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    universe = expectations[2]["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    control = inputs["rust_control_native_inputs"]
    assert isinstance(control, dict)
    relations = control["relations"]
    assert isinstance(relations, list)
    relations.remove("provider.rustc.mir_block.v1")
    _write_jsonl(path, expectations)
    with pytest.raises(
        SuccessorEvidenceError, match="Rust control input closure differs"
    ):
        validate_expectations(root)


def test_int_call_graph_requires_exact_canonical_occurrence_owner_join(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    claim = next(row for row in expectations if row["claim_id"] == "RFV3-CLAIM-003")
    inputs = claim["complete_input_universe"]["inputs"]
    occurrences = inputs["canonical_call_occurrences"]
    callables = inputs["canonical_callable_lookup"]
    assert isinstance(occurrences, dict)
    assert isinstance(callables, dict)
    occurrence_rows = occurrences["rows"]
    callable_rows = callables["rows"]
    assert isinstance(occurrence_rows, list)
    assert isinstance(callable_rows, list)
    occurrence = occurrence_rows[0]
    alternate = callable_rows[0]
    assert isinstance(occurrence, list)
    assert isinstance(alternate, list)
    occurrence[2] = alternate[1]
    _write_jsonl(path, expectations)
    with pytest.raises(
        SuccessorEvidenceError,
        match="canonical call occurrence/owner identity differs",
    ):
        validate_expectations(root)


def test_int_call_graph_provider_coordinates_must_join_a_canonical_occurrence(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    claim = next(row for row in expectations if row["claim_id"] == "RFV3-CLAIM-003")
    inputs = claim["complete_input_universe"]["inputs"]
    provider = inputs["provider_call_targets"]
    assert isinstance(provider, dict)
    rows = provider["rows"]
    assert isinstance(rows, list)
    row = rows[0]
    assert isinstance(row, list)
    row[1] += 1
    _write_jsonl(path, expectations)
    with pytest.raises(
        SuccessorEvidenceError, match="does not close both enforced joins"
    ):
        validate_expectations(root)


def test_int_call_graph_provider_target_must_join_canonical_callable_lookup(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    claim = next(row for row in expectations if row["claim_id"] == "RFV3-CLAIM-003")
    inputs = claim["complete_input_universe"]["inputs"]
    provider = inputs["provider_call_targets"]
    assert isinstance(provider, dict)
    rows = provider["rows"]
    assert isinstance(rows, list)
    row = rows[0]
    assert isinstance(row, list)
    row[5] = "fixture.not_declared"
    _write_jsonl(path, expectations)
    with pytest.raises(
        SuccessorEvidenceError, match="does not close both enforced joins"
    ):
        validate_expectations(root)


def test_int_call_graph_causal_fixture_mutates_only_native_qualified_target(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / FIXTURES_PATH
    fixtures = _load_jsonl(path)
    fixture = next(row for row in fixtures if row["fixture_id"] == "RFV3-FIX-003-C")
    mutation = fixture["mutation"]
    assert isinstance(mutation, dict)
    mutation["after"] = "fixture.gamma"
    _write_jsonl(path, fixtures)
    with pytest.raises(SuccessorEvidenceError, match="call-graph causal delta differs"):
        validate_fixtures(root)


def test_int_query_producer_coverage_cannot_cross_workspace_scope(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    universe = expectations[3]["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    coverage = inputs["producer_coverage"]
    assert isinstance(coverage, dict)
    coverage["scope"] = "workspace:ffffffffffffffffffffffffffffffff"
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="coverage crosses workspace"):
        validate_expectations(root)


def test_int_follow_relationship_fixture_preserves_declared_target_order(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / FIXTURES_PATH
    fixtures = _load_jsonl(path)
    fixture = next(row for row in fixtures if row["fixture_id"] == "RFV3-FIX-006-C")
    expected = fixture["expected_decoded"]
    assert isinstance(expected, dict)
    result = expected["query_result"]
    assert isinstance(result, dict)
    fact_ids = result["fact_ids"]
    assert isinstance(fact_ids, list)
    fact_ids.reverse()
    _write_jsonl(path, fixtures)
    with pytest.raises(SuccessorEvidenceError, match="follow-edge result differs"):
        validate_fixtures(root)


def test_int_follow_relationship_fixture_requires_target_dictionary_record(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / FIXTURES_PATH
    fixtures = _load_jsonl(path)
    fixture = next(row for row in fixtures if row["fixture_id"] == "RFV3-FIX-006-C")
    mutation = fixture["mutation"]
    assert isinstance(mutation, dict)
    after = mutation["after"]
    assert isinstance(after, dict)
    admitted = after["admitted_relations"]
    assert isinstance(admitted, dict)
    dictionary = admitted["entity_dictionary"]
    assert isinstance(dictionary, dict)
    dictionary.pop("entity:function:99999999999999999999999999999999")
    _write_jsonl(path, fixtures)
    with pytest.raises(
        SuccessorEvidenceError,
        match="admitted relationship edge is not ontology-closed|dictionary closure differs",
    ):
        validate_fixtures(root)


def test_int_equivalent_routes_require_exact_expectation_provenance(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    decoded = expectations[17]["decoded_expectation"]
    assert isinstance(decoded, dict)
    rows = decoded["rows"]
    assert isinstance(rows, list)
    clean = rows[0]
    assert isinstance(clean, list)
    provenance = clean[6]
    assert isinstance(provenance, dict)
    provenance.pop("expectation_issuance")
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="provenance closure differs"):
        validate_expectations(root)


def test_int_equivalent_routes_require_immutable_source_bytes_and_digest(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    universe = expectations[17]["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    images = inputs["source_images"]
    assert isinstance(images, dict)
    generation = images["generation_g2"]
    assert isinstance(generation, dict)
    generation["content_digest"] = "b3:" + ("0" * 64)
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="source image identity differs"):
        validate_expectations(root)


def test_int_equivalence_identity_contract_requires_recipe_specific_cbef(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    claim = next(row for row in expectations if row["claim_id"] == "RFV3-CLAIM-018")
    inputs = claim["complete_input_universe"]["inputs"]
    assert isinstance(inputs, dict)
    derivation = inputs["change_derivation"]
    assert isinstance(derivation, dict)
    contract = derivation["identity_contract"]
    assert isinstance(contract, dict)
    contract["format"] = "RFC8785-envelope"
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="change derivation differs"):
        validate_expectations(root)


def test_int_equivalent_routes_reject_unresolved_semantic_callee(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    claim = next(row for row in expectations if row["claim_id"] == "RFV3-CLAIM-018")
    universe = claim["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    images = inputs["source_images"]
    assert isinstance(images, dict)
    generation = images["generation_g2"]
    assert isinstance(generation, dict)
    source = "def e3():\n    pass\n\ndef e1():\n    missing()\n"
    generation["bytes_utf8"] = source
    generation["content_digest"] = f"b3:{blake3.blake3(source.encode()).hexdigest()}"
    _write_jsonl(path, expectations)
    with pytest.raises(
        SuccessorEvidenceError,
        match="unresolved call cannot be emitted as a semantic entity",
    ):
        validate_expectations(root)


def test_int_equivalence_function_identity_binds_workspace_module_context_and_path() -> (
    None
):
    expectations = _load_jsonl(ROOT / EXPECTATIONS_PATH)
    claim = next(row for row in expectations if row["claim_id"] == "RFV3-CLAIM-018")
    image = claim["complete_input_universe"]["inputs"]["source_images"]["generation_g2"]
    assert isinstance(image, dict)

    def function_ids(value: dict[str, object]) -> dict[str, str]:
        rows = _source_semantic_rows(value, "identity mutation")
        return {str(row[2]): str(row[0]) for row in rows if row[1] == "function"}

    baseline = function_ids(image)
    for field, replacement in (
        ("module_id", "entity:module:19191919191919191919191919191919"),
    ):
        mutated = json.loads(json.dumps(image))
        mutated[field] = replacement
        observed = function_ids(mutated)
        assert observed["e1"] != baseline["e1"]
        assert observed["e3"] != baseline["e3"]

    for field, replacement in (
        ("workspace_id", "workspace:19191919191919191919191919191919"),
        ("semantic_environment_digest", "b3:" + ("19" * 32)),
    ):
        mutated = json.loads(json.dumps(image))
        mutated[field] = replacement
        mutated["analysis_context_id"] = _cbef_analysis_context_id(
            workspace_id=mutated["workspace_id"],
            language_slug=str(mutated["language"]),
            environment_digest=mutated["semantic_environment_digest"],
        )
        observed = function_ids(mutated)
        assert observed["e1"] != baseline["e1"]
        assert observed["e3"] != baseline["e3"]

    renamed = json.loads(json.dumps(image))
    renamed_source = str(renamed["bytes_utf8"]).replace("e3", "e4")
    renamed["bytes_utf8"] = renamed_source
    renamed["content_digest"] = (
        f"b3:{blake3.blake3(renamed_source.encode()).hexdigest()}"
    )
    renamed_ids = function_ids(renamed)
    assert renamed_ids["e1"] == baseline["e1"]
    assert renamed_ids["e4"] != baseline["e3"]


def test_int_equivalence_call_site_identity_binds_content_range_and_owner() -> None:
    expectations = _load_jsonl(ROOT / EXPECTATIONS_PATH)
    claim = next(row for row in expectations if row["claim_id"] == "RFV3-CLAIM-018")
    image = claim["complete_input_universe"]["inputs"]["source_images"]["generation_g2"]
    assert isinstance(image, dict)
    baseline = _source_semantic_rows(image, "baseline call-site identity")

    shifted = json.loads(json.dumps(image))
    shifted_source = "# exact byte-range shift\n" + str(shifted["bytes_utf8"])
    shifted["bytes_utf8"] = shifted_source
    shifted["content_digest"] = (
        f"b3:{blake3.blake3(shifted_source.encode()).hexdigest()}"
    )
    observed = _source_semantic_rows(shifted, "shifted call-site identity")
    baseline_functions = {row[2]: row[0] for row in baseline if row[1] == "function"}
    observed_functions = {row[2]: row[0] for row in observed if row[1] == "function"}
    assert observed_functions == baseline_functions
    baseline_call_site = next(row for row in baseline if row[1] == "call_site")
    observed_call_site = next(row for row in observed if row[1] == "call_site")
    observed_call = next(row for row in observed if row[1] == "calls")
    assert observed_call_site[0] != baseline_call_site[0]
    assert observed_call[5] == observed_call_site[0]


def test_int_equivalence_calls_fact_requires_its_canonical_call_site_id(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    claim = next(row for row in expectations if row["claim_id"] == "RFV3-CLAIM-018")
    decoded = claim["decoded_expectation"]
    assert isinstance(decoded, dict)
    routes = decoded["rows"]
    assert isinstance(routes, list)
    clean = routes[0]
    assert isinstance(clean, list)
    rows = clean[1]
    assert isinstance(rows, list)
    calls = next(row for row in rows if isinstance(row, list) and row[1] == "calls")
    calls[5] = "entity:call-site:" + ("0" * 32)
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="clean decoded route differs"):
        validate_expectations(root)


def test_int_equivalence_delete_negative_retains_stale_call_site_and_calls_link(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / FIXTURES_PATH
    fixtures = _load_jsonl(path)
    fixture = next(row for row in fixtures if row["fixture_id"] == "RFV3-FIX-018-N")
    expected = fixture["expected_decoded"]
    assert isinstance(expected, dict)
    extra = expected["incremental_extra"]
    assert isinstance(extra, list)
    extra[:] = [
        row for row in extra if not (isinstance(row, list) and row[1] == "call_site")
    ]
    _write_jsonl(path, fixtures)
    with pytest.raises(
        SuccessorEvidenceError, match="equivalence delete-negative rows differ"
    ):
        validate_fixtures(root)


def test_int_provider_rows_follow_the_declared_canonical_order(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    decoded = expectations[0]["decoded_expectation"]
    assert isinstance(decoded, dict)
    rows = decoded["rows"]
    assert isinstance(rows, list)
    rows[0], rows[1] = rows[1], rows[0]
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="canonical ordering"):
        validate_expectations(root)


def test_int_delta_materialized_history_requires_exact_transition_kind(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    expectation = next(
        row for row in expectations if row["claim_id"] == "RFV3-CLAIM-012"
    )
    universe = expectation["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    history = inputs["delta_table_history"]
    assert isinstance(history, dict)
    versions = history["versions"]
    assert isinstance(versions, list)
    latest = versions[-1]
    assert isinstance(latest, dict)
    latest["operation"] = "WRITE_OVERWRITE"
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="Delta transition differs"):
        validate_expectations(root)


def test_int_delta_latest_refresh_cannot_use_the_dml_update_builder(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    expectation = next(
        row for row in expectations if row["claim_id"] == "RFV3-CLAIM-012"
    )
    proof = expectation["complete_input_universe"]["inputs"]["proof_input"]
    assert isinstance(proof, dict)
    latest = proof["latest_snapshot_read"]
    assert isinstance(latest, dict)
    latest["api"] = "DeltaTable::update"
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="Delta selection APIs differ"):
        validate_expectations(root)


def test_int_delta_exact_selection_binds_the_observed_table_root(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    expectation = next(
        row for row in expectations if row["claim_id"] == "RFV3-CLAIM-012"
    )
    universe = expectation["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    identity = inputs["table_root_identity"]
    assert isinstance(identity, dict)
    identity["binding"] = "memory://wrong-root/fact.entity"
    _write_jsonl(path, expectations)
    with pytest.raises(
        SuccessorEvidenceError, match="fabricates a physical Delta root"
    ):
        validate_expectations(root)


def test_int_delta_negative_feature_binds_the_exact_write_under_test(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / FIXTURES_PATH
    fixtures = _load_jsonl(path)
    fixture = next(row for row in fixtures if row["fixture_id"] == "RFV3-FIX-012-N")
    mutation = fixture["mutation"]
    assert isinstance(mutation, dict)
    mutation["json_pointer"] = "/versions/0/protocol"
    _write_jsonl(path, fixtures)
    with pytest.raises(SuccessorEvidenceError, match="unsupported Delta feature fault"):
        validate_fixtures(root)


def test_int_fixture_bad_mutation_pointer_is_rejected(tmp_path: Path) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / FIXTURES_PATH
    fixtures = _load_jsonl(path)
    mutation = fixtures[0]["mutation"]
    assert isinstance(mutation, dict)
    mutation["json_pointer"] = "/missing"
    _write_jsonl(path, fixtures)
    with pytest.raises(
        SuccessorEvidenceError, match="invalid array index|references a missing member"
    ):
        validate_fixtures(root)


def test_int_fixture_wrong_before_value_is_rejected(tmp_path: Path) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / FIXTURES_PATH
    fixtures = _load_jsonl(path)
    mutation = fixtures[0]["mutation"]
    assert isinstance(mutation, dict)
    mutation["before"] = "not-the-authoritative-value"
    _write_jsonl(path, fixtures)
    with pytest.raises(SuccessorEvidenceError, match="before value differs"):
        validate_fixtures(root)


def test_int_fixture_non_input_mutation_is_rejected(tmp_path: Path) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / FIXTURES_PATH
    fixtures = _load_jsonl(path)
    mutation = fixtures[0]["mutation"]
    assert isinstance(mutation, dict)
    mutation["input_role"] = "decoded_expectation"
    _write_jsonl(path, fixtures)
    with pytest.raises(SuccessorEvidenceError, match="targets a non-input role"):
        validate_fixtures(root)


def test_int_fixture_decoded_outcome_must_follow_mutated_input(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / FIXTURES_PATH
    fixtures = _load_jsonl(path)
    expected = fixtures[10]["expected_decoded"]
    assert isinstance(expected, dict)
    result = expected["query_result"]
    assert isinstance(result, dict)
    fact_ids = result["fact_ids"]
    assert isinstance(fact_ids, list)
    fact_ids.pop()
    _write_jsonl(path, fixtures)
    with pytest.raises(SuccessorEvidenceError, match="follow-edge result differs"):
        validate_fixtures(root)


def test_int_wire_cancellation_counts_follow_the_typed_mapping(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / FIXTURES_PATH
    fixtures = _load_jsonl(path)
    fixture = next(row for row in fixtures if row["fixture_id"] == "RFV3-FIX-017-C")
    expected = fixture["expected_decoded"]
    assert isinstance(expected, dict)
    expected["failed_query_count"] = 1
    _write_jsonl(path, fixtures)
    with pytest.raises(SuccessorEvidenceError, match="wire causal terminal differs"):
        validate_fixtures(root)


def test_int_wire_cancellation_requires_partial_coverage(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / FIXTURES_PATH
    fixtures = _load_jsonl(path)
    fixture = next(row for row in fixtures if row["fixture_id"] == "RFV3-FIX-017-C")
    expected = fixture["expected_decoded"]
    assert isinstance(expected, dict)
    query_results = expected["query_results"]
    assert isinstance(query_results, list)
    query_result = query_results[0]
    assert isinstance(query_result, dict)
    query_result["coverage"] = {"state": "COMPLETE"}
    _write_jsonl(path, fixtures)
    with pytest.raises(SuccessorEvidenceError, match="wire causal terminal differs"):
        validate_fixtures(root)


def test_int_wire_access_scope_must_match_request_and_snapshot_workspace(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    universe = expectations[16]["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    access_scope = inputs["access_scope"]
    assert isinstance(access_scope, dict)
    access_scope["workspace"] = "workspace:ffffffffffffffffffffffffffffffff"
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="workspace correlation differs"):
        validate_expectations(root)


def test_int_wire_checksum_is_derived_from_the_decoded_response(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    universe = expectations[16]["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    terminal = inputs["internal_terminal"]
    assert isinstance(terminal, dict)
    terminal["canonical_response_checksum"] = "b3:" + "0" * 64
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="checksum differs"):
        validate_expectations(root)


@pytest.mark.parametrize("fault", ["missing", "changed"])
def test_int_wire_requires_the_exact_daemon_canonical_response(
    tmp_path: Path, fault: str
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    expectation = next(
        row for row in expectations if row["claim_id"] == "RFV3-CLAIM-017"
    )
    universe = expectation["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    source = inputs["daemon_canonical_response_results"]
    assert isinstance(source, dict)
    results = source["results"]
    assert isinstance(results, list)
    complete = next(
        result
        for result in results
        if isinstance(result, dict) and result["result_id"] == "daemon-result:complete"
    )
    if fault == "missing":
        results.remove(complete)
    else:
        response = json.loads(str(complete["canonical_json"]))
        assert isinstance(response, dict)
        response["semantic_request_id"] = "rq-changed"
        complete["canonical_json"] = rfc8785.dumps(response).decode("utf-8")
    _write_jsonl(path, expectations)
    with pytest.raises(SuccessorEvidenceError, match="checksum differs"):
        validate_expectations(root)


def test_int_query_selector_requires_an_admitted_identity_binding(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    universe = expectations[4]["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    request = inputs["request_envelope"]
    assert isinstance(request, dict)
    decoded = request["decoded"]
    assert isinstance(decoded, dict)
    queries = decoded["queries"]
    assert isinstance(queries, list)
    query = queries[0]
    assert isinstance(query, dict)
    query["about"] = ["fn:a"]
    request["canonical_json"] = rfc8785.dumps(decoded).decode("utf-8")
    _write_jsonl(path, expectations)
    with pytest.raises(
        SuccessorEvidenceError, match="lacks an admitted identity binding"
    ):
        validate_expectations(root)


def test_int_query_result_reference_requires_dictionary_record(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    decoded = expectations[3]["decoded_expectation"]
    assert isinstance(decoded, dict)
    rows = decoded["rows"]
    assert isinstance(rows, list)
    assert isinstance(rows[0], list)
    response = rows[0][0]
    assert isinstance(response, dict)
    response["entities"] = {}
    _write_jsonl(path, expectations)
    with pytest.raises(
        SuccessorEvidenceError,
        match="entity dictionary differs|references missing entities",
    ):
        validate_expectations(root)


def test_int_security_authorization_rejects_downstream_evidence_fields(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    claim = next(row for row in expectations if row["claim_id"] == "RFV3-CLAIM-016")
    universe = claim["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    authorization = inputs["explicit_authorization"]
    assert isinstance(authorization, dict)
    authorization["launcher_receipt_id"] = "launcher-receipt:smuggled"
    decoded = claim["decoded_expectation"]
    assert isinstance(decoded, dict)
    with pytest.raises(
        SuccessorEvidenceError, match="explicit authorization keys differ"
    ):
        _validate_security_inputs(inputs, decoded, "claim 016")


def test_int_security_causal_fixture_cannot_smuggle_downstream_evidence(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / FIXTURES_PATH
    fixtures = _load_jsonl(path)
    fixture = next(row for row in fixtures if row["fixture_id"] == "RFV3-FIX-016-C")
    mutation = fixture["mutation"]
    assert isinstance(mutation, dict)
    after = mutation["after"]
    assert isinstance(after, dict)
    after["provenance_id"] = "provenance:smuggled"
    fixtures = [row for row in fixtures if row["claim_id"] == "RFV3-CLAIM-016"]
    _write_jsonl(path, fixtures)
    expectations = _load_jsonl(root / EXPECTATIONS_PATH)
    claim = next(row for row in expectations if row["claim_id"] == "RFV3-CLAIM-016")
    with pytest.raises(
        SuccessorEvidenceError, match="explicit authorization keys differ"
    ):
        validate_fixtures(root, expectations=[claim])


def test_int_security_causal_fixture_must_target_only_authorization(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / FIXTURES_PATH
    fixtures = _load_jsonl(path)
    fixture = next(row for row in fixtures if row["fixture_id"] == "RFV3-FIX-016-C")
    mutation = fixture["mutation"]
    assert isinstance(mutation, dict)
    mutation.update(
        {
            "input_role": "provider_jobs",
            "json_pointer": "/0/requested_profile",
            "before": "trusted_local",
            "after": "untrusted",
        }
    )
    fixtures = [row for row in fixtures if row["claim_id"] == "RFV3-CLAIM-016"]
    _write_jsonl(path, fixtures)
    expectations = _load_jsonl(root / EXPECTATIONS_PATH)
    claim = next(row for row in expectations if row["claim_id"] == "RFV3-CLAIM-016")
    with pytest.raises(
        SuccessorEvidenceError,
        match="authorization fixture must mutate only explicit authorization",
    ):
        validate_fixtures(root, expectations=[claim])


def test_int_security_terminal_requires_logical_receipt_proof_and_provenance(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    claim = next(row for row in expectations if row["claim_id"] == "RFV3-CLAIM-016")
    decoded = claim["decoded_expectation"]
    assert isinstance(decoded, dict)
    rows = decoded["rows"]
    assert isinstance(rows, list)
    hostile = rows[1]
    assert isinstance(hostile, list)
    hostile[7] = "sandbox_proof_observed"
    universe = claim["complete_input_universe"]
    assert isinstance(universe, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    with pytest.raises(
        SuccessorEvidenceError,
        match="unavailable security terminal overclaims execution",
    ):
        _validate_security_inputs(inputs, decoded, "claim 016")


def test_int_security_authorization_fixture_proves_hostile_job_unchanged(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    path = root / FIXTURES_PATH
    fixtures = _load_jsonl(path)
    fixture = next(row for row in fixtures if row["fixture_id"] == "RFV3-FIX-016-C")
    expected = fixture["expected_decoded"]
    assert isinstance(expected, dict)
    expected["untrusted_admission"] = "available"
    fixtures = [row for row in fixtures if row["claim_id"] == "RFV3-CLAIM-016"]
    _write_jsonl(path, fixtures)
    expectations = _load_jsonl(root / EXPECTATIONS_PATH)
    claim = next(row for row in expectations if row["claim_id"] == "RFV3-CLAIM-016")
    with pytest.raises(
        SuccessorEvidenceError,
        match="trusted-local authorization preflight differs",
    ):
        validate_fixtures(root, expectations=[claim])


def test_int_security_name_only_requirement_cannot_supply_observed_proof(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    expectations = _load_jsonl(root / EXPECTATIONS_PATH)
    claim = next(row for row in expectations if row["claim_id"] == "RFV3-CLAIM-016")
    universe = claim["complete_input_universe"]
    decoded = claim["decoded_expectation"]
    assert isinstance(universe, dict)
    assert isinstance(decoded, dict)
    inputs = universe["inputs"]
    assert isinstance(inputs, dict)
    contract = inputs["launcher_evidence_contract"]
    assert isinstance(contract, dict)
    required = contract["required_prerequisites"]
    assert isinstance(required, list)
    required.append("named-but-unobserved-prerequisite")
    with pytest.raises(
        SuccessorEvidenceError,
        match="successor launcher evidence contract differs",
    ):
        _validate_security_inputs(inputs, decoded, "claim 016")


def test_int_security_hostile_action_counts_must_match_observed_closure(
    tmp_path: Path,
) -> None:
    root = _copy_candidate_contract(tmp_path / "repo")
    expectations = _load_jsonl(root / EXPECTATIONS_PATH)
    claim = next(row for row in expectations if row["claim_id"] == "RFV3-CLAIM-016")
    universe = claim["complete_input_universe"]
    decoded = claim["decoded_expectation"]
    assert isinstance(universe, dict)
    assert isinstance(decoded, dict)
    inputs = universe["inputs"]
    rows = decoded["rows"]
    assert isinstance(inputs, dict)
    assert isinstance(rows, list)
    hostile = rows[1]
    assert isinstance(hostile, list)
    hostile[8] = 9
    with pytest.raises(
        SuccessorEvidenceError,
        match="unavailable security terminal overclaims execution",
    ):
        _validate_security_inputs(inputs, decoded, "claim 016")


def test_beh_repository_decoded_values_have_independent_acceptance() -> None:
    assert validate_expected_behavior_review(ROOT) == 18


def test_int_claim_review_basis_binds_exact_rows_and_source_authority() -> None:
    claims = {
        claim_id: _expectation(claim_id)
        for claim_id in ("RFV3-CLAIM-001", "RFV3-CLAIM-003")
    }
    fixtures = _load_jsonl(ROOT / FIXTURES_PATH)
    bases: dict[str, dict[str, object]] = {}
    rationales: dict[str, str] = {}
    for claim_id, claim in claims.items():
        fixture_rows = {
            str(row["kind"]): row for row in fixtures if row["claim_id"] == claim_id
        }
        fixture_digests = {
            kind: _canonical_sha256(fixture_rows[kind])
            for kind in ("causal", "negative")
        }
        basis = claim_review_basis(claim, fixture_digests)
        bases[claim_id] = basis
        rationale = claim_review_rationale(claim, basis, accepted=False)
        rationales[claim_id] = rationale
        assert basis["claim_id"] == claim_id
        assert basis["expectation_sha256"] == _canonical_sha256(claim)
        assert basis["semantic_fixture_sha256"] == fixture_digests
        assert basis["subject"] == claim["subject"]
        assert basis["source_authority"] == {
            "source_anchor": claim["source_anchor"],
            "governing_clauses": claim["governing_clauses"],
        }
        assert claim_id in rationale
        assert basis["review_binding_id"] in rationale
        assert claim["subject"] in rationale
        assert claim["source_anchor"] in rationale
        assert all(clause in rationale for clause in claim["governing_clauses"])
    assert bases["RFV3-CLAIM-001"] != bases["RFV3-CLAIM-003"]
    assert rationales["RFV3-CLAIM-001"] != rationales["RFV3-CLAIM-003"]


def test_int_refresh_issuance_emits_unique_claim_review_records() -> None:
    expectations = _load_jsonl(ROOT / EXPECTATIONS_PATH)
    fixtures = _load_jsonl(ROOT / FIXTURES_PATH)
    issuance = refresh_issuance(expectations, fixtures)
    reviews = issuance["claim_reviews"]
    assert isinstance(reviews, list)
    assert len(reviews) == len(expectations) == 18
    assert len({review["rationale"] for review in reviews}) == 18
    assert (
        len({review["review_basis"]["review_binding_id"] for review in reviews}) == 18
    )
    claim_by_id = {claim["claim_id"]: claim for claim in expectations}
    fixtures_by_claim = {
        claim_id: {
            str(row["kind"]): row for row in fixtures if row["claim_id"] == claim_id
        }
        for claim_id in claim_by_id
    }
    for review in reviews:
        claim_id = review["claim_id"]
        claim = claim_by_id[claim_id]
        fixture_digests = {
            kind: _canonical_sha256(fixtures_by_claim[claim_id][kind])
            for kind in ("causal", "negative")
        }
        expected_basis = claim_review_basis(claim, fixture_digests)
        assert review["expectation_sha256"] == expected_basis["expectation_sha256"]
        assert review["fixture_sha256"] == expected_basis["semantic_fixture_sha256"]
        assert review["review_basis"] == expected_basis
        assert review["disposition"] == "pending"
        accepted = copy.deepcopy(review)
        accepted["rationale"] = claim_review_rationale(
            claim, expected_basis, accepted=True
        )
        _validate_claim_review_specificity(
            accepted,
            claim,
            fixtures_by_claim[claim_id],
            f"{claim_id} generated review proof",
        )


def test_neg_claim_review_rejects_generic_or_copied_acceptance() -> None:
    claims = {
        claim_id: _expectation(claim_id)
        for claim_id in ("RFV3-CLAIM-001", "RFV3-CLAIM-003")
    }
    all_fixtures = _load_jsonl(ROOT / FIXTURES_PATH)
    fixtures_by_claim = {
        claim_id: {
            str(row["kind"]): row for row in all_fixtures if row["claim_id"] == claim_id
        }
        for claim_id in claims
    }
    claim = claims["RFV3-CLAIM-001"]
    fixtures = fixtures_by_claim["RFV3-CLAIM-001"]
    fixture_digests = {
        kind: _canonical_sha256(fixtures[kind]) for kind in ("causal", "negative")
    }
    basis = claim_review_basis(claim, fixture_digests)
    review = {
        "review_basis": basis,
        "rationale": claim_review_rationale(claim, basis, accepted=True),
    }
    rationale = _validate_claim_review_specificity(
        review, claim, fixtures, "claim 001 review proof"
    )

    generic = copy.deepcopy(review)
    generic["rationale"] = "Accepted all claims against the authoritative design."
    with pytest.raises(SuccessorEvidenceError, match="generic or copied"):
        _validate_claim_review_specificity(
            generic, claim, fixtures, "generic review proof"
        )

    copied = copy.deepcopy(review)
    with pytest.raises(SuccessorEvidenceError, match="not bound to its claim"):
        _validate_claim_review_specificity(
            copied,
            claims["RFV3-CLAIM-003"],
            fixtures_by_claim["RFV3-CLAIM-003"],
            "copied review proof",
        )

    authority_drift = copy.deepcopy(review)
    authority_drift["review_basis"]["source_authority"]["governing_clauses"].pop()
    with pytest.raises(SuccessorEvidenceError, match="not bound to its claim"):
        _validate_claim_review_specificity(
            authority_drift, claim, fixtures, "authority drift proof"
        )

    pending = copy.deepcopy(review)
    pending["rationale"] = claim_review_rationale(claim, basis, accepted=False)
    with pytest.raises(SuccessorEvidenceError, match="pending disposition"):
        _validate_claim_review_specificity(
            pending, claim, fixtures, "pending review proof"
        )

    seen: set[str] = set()
    _record_unique_review_rationale(rationale, seen)
    with pytest.raises(SuccessorEvidenceError, match="generic repeated text"):
        _record_unique_review_rationale(rationale, seen)


def test_beh_changed_decoded_expected_value_invalidates_review(tmp_path: Path) -> None:
    root = _copy_current_issuance(tmp_path / "repo")
    path = root / EXPECTATIONS_PATH
    expectations = _load_jsonl(path)
    decoded = expectations[0]["decoded_expectation"]
    assert isinstance(decoded, dict)
    rows = decoded["rows"]
    assert isinstance(rows, list)
    assert isinstance(rows[0], list)
    rows[0][1] = "producer_authored_value"
    _write_jsonl(path, expectations)
    _refresh_content_identity(root, "expectations", EXPECTATIONS_PATH)
    # Full semantic derivation is allowed to reject the forged output before
    # the independent-review binding is reached.
    with pytest.raises(
        SuccessorEvidenceError,
        match="Pyrefly callable expectation is not derived from the exact source|decoded expected value changed",
    ):
        validate_expected_behavior_review(root)


def test_beh_author_and_reviewer_must_be_distinct(tmp_path: Path) -> None:
    root = _copy_current_issuance(tmp_path / "repo")
    issuance_path = root / ISSUANCE_PATH
    issuance = _load_json(issuance_path)
    author = issuance["author"]
    reviewer = issuance["reviewer"]
    assert isinstance(author, dict)
    assert isinstance(reviewer, dict)
    reviewer["identity"] = author["identity"]
    _write_json(issuance_path, issuance)
    with pytest.raises(SuccessorEvidenceError, match="author/reviewer independence"):
        validate_expected_behavior_review(root)


def test_beh_invalidation_policy_requires_reissue_and_consumer_reopening(
    tmp_path: Path,
) -> None:
    root = _copy_current_issuance(tmp_path / "repo")
    issuance_path = root / ISSUANCE_PATH
    issuance = _load_json(issuance_path)
    policy = issuance["invalidation_policy"]
    assert isinstance(policy, dict)
    policy["fixture_change"] = "retain_issuance"
    _write_json(issuance_path, issuance)
    with pytest.raises(SuccessorEvidenceError, match="invalidation policy differs"):
        validate_expected_behavior_review(root)


def test_neg_every_claim_has_semantic_causal_and_negative_fixture() -> None:
    fixtures = validate_fixtures(ROOT)
    assert len(fixtures) == 36
    assert {fixture["kind"] for fixture in fixtures} == {"causal", "negative"}
    assert all(fixture["semantic"] is True for fixture in fixtures)
    assert all(fixture["integrity_only"] is False for fixture in fixtures)


@pytest.mark.parametrize(
    "forbidden",
    [
        "src/provider_admission.rs",
        "contracts/acceptance/relational-fabric-v1/expectations.json",
    ],
)
def test_neg_target_or_historical_output_import_is_rejected(
    tmp_path: Path, forbidden: str
) -> None:
    root = _copy_current_issuance(tmp_path / "repo")
    path = root / FIXTURES_PATH
    fixtures = _load_jsonl(path)
    fixtures[0]["imports"] = [forbidden]
    _write_jsonl(path, fixtures)
    with pytest.raises(
        SuccessorEvidenceError, match="forbidden target or historical output"
    ):
        validate_negative_fixture_independence(root)


def test_neg_missing_claim_fixture_is_rejected(tmp_path: Path) -> None:
    root = _copy_current_issuance(tmp_path / "repo")
    path = root / FIXTURES_PATH
    fixtures = [
        fixture
        for fixture in _load_jsonl(path)
        if fixture["fixture_id"] != "RFV3-FIX-018-N"
    ]
    _write_jsonl(path, fixtures)
    with pytest.raises(
        SuccessorEvidenceError, match="lacks one causal and one negative"
    ):
        validate_negative_fixture_independence(root)


def test_neg_integrity_only_fixture_cannot_supply_semantic_fault(
    tmp_path: Path,
) -> None:
    root = _copy_current_issuance(tmp_path / "repo")
    path = root / FIXTURES_PATH
    fixtures = _load_jsonl(path)
    fixtures[0]["integrity_only"] = True
    _write_jsonl(path, fixtures)
    with pytest.raises(SuccessorEvidenceError, match="digest/count/text-only"):
        validate_negative_fixture_independence(root)


def test_ops_consumer_packets_transitively_depend_on_wp33() -> None:
    assert validate_readiness(ROOT) == 18


def test_ops_dependency_order_break_is_rejected(tmp_path: Path) -> None:
    root = _copy_current_issuance(tmp_path / "repo")
    plan_path = root / PLAN_PATH
    plan = plan_path.read_text(encoding="utf-8")
    plan = plan.replace(
        "**Dependencies.** WP31, WP32, and WP33.",
        "**Dependencies.** WP31 and WP32.",
        1,
    )
    plan_path.write_text(plan, encoding="utf-8")
    with pytest.raises(SuccessorEvidenceError, match="WP34 can progress without"):
        validate_readiness(root)


def test_ops_wp38_consumer_cannot_bypass_wp33(tmp_path: Path) -> None:
    root = _copy_current_issuance(tmp_path / "repo")
    plan_path = root / PLAN_PATH
    plan = plan_path.read_text(encoding="utf-8")
    plan = plan.replace(
        "**Dependencies.** WP37.",
        "**Dependencies.** WP32.",
        1,
    )
    plan_path.write_text(plan, encoding="utf-8")
    with pytest.raises(SuccessorEvidenceError, match="WP38 can progress without"):
        validate_readiness(root)


def test_ops_zero_selected_claims_fail_closed() -> None:
    with pytest.raises(SuccessorEvidenceError, match="selected zero rows"):
        validate_readiness(ROOT, ["RFV3-CLAIM-999"])


def test_ops_fixture_mutation_reopens_dependent_consumers(tmp_path: Path) -> None:
    root = _copy_current_issuance(tmp_path / "repo")
    path = root / FIXTURES_PATH
    fixtures = _load_jsonl(path)
    fixtures[0]["authoritative_change"] = "Change a different semantic provider input."
    _write_jsonl(path, fixtures)
    _refresh_content_identity(root, "negative_fixtures", FIXTURES_PATH)
    with pytest.raises(SuccessorEvidenceError, match="fixture changed after review"):
        validate_readiness(root)


def test_ops_readiness_needs_no_historical_or_target_artifacts(tmp_path: Path) -> None:
    root = _copy_current_issuance(tmp_path / "repo")
    assert not (root / "src").exists()
    assert not (root / "contracts/acceptance/relational-fabric-v1").exists()
    assert validate_readiness(root) == 18
