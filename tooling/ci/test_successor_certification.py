"""Focused positive and falsification tests for WP42 successor certification."""

from __future__ import annotations

import copy
import json
import shutil
from pathlib import Path

import pytest

from tooling.ci import plan_assurance
from tooling.ci.successor_certification import (
    CONTRACT_PATH,
    EXPECTED_CATEGORIES,
    EXPECTED_DOMAINS,
    EXPECTED_PACKETS,
    ROOT,
    CommandObservation,
    SuccessorCertificationError,
    derive_oracle_catalog,
    execute_certification,
    execute_recipe_set,
    four_domain_recipes,
    load_contract,
    validate_certification_record,
    validate_definition_scope,
    validate_state_provenance,
)


def _copy_contract(destination: Path) -> Path:
    contract = load_contract(ROOT)
    for relative in (
        CONTRACT_PATH,
        Path(str(contract["certification_record_schema_path"])),
    ):
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / relative, target)
    return destination


def _mutate_contract(root: Path, mutation: object) -> None:
    contract = json.loads((root / CONTRACT_PATH).read_text(encoding="utf-8"))
    assert isinstance(contract, dict)
    mutation(contract)
    (root / CONTRACT_PATH).write_text(
        json.dumps(contract, indent=2) + "\n", encoding="utf-8"
    )


def _entry(status: str, commit: str | None) -> dict[str, object]:
    return {
        "status": status,
        "proving_commit": commit,
        "deviations": [],
        "failed_approaches": [],
        "blockers": [],
    }


def _candidate_state() -> dict[str, object]:
    packets = {"WP28": _entry("invalidated", None)}
    packets.update(
        {packet: _entry("complete", "trusted") for packet in EXPECTED_PACKETS[:-1]}
    )
    packets["WP42"] = _entry("in_progress", None)
    milestones = {"M01": _entry("invalidated", None)}
    milestones.update(
        {
            milestone: _entry("complete", "trusted")
            for milestone in ("M02", "M03", "M04", "M05")
        }
    )
    milestones["M06"] = _entry("in_progress", None)
    batches = {
        batch: _entry("complete", "trusted")
        for batch in ("DB09", "DB10", "DB11", "DB12", "DB13")
    }
    batches["DB14"] = _entry("in_progress", None)
    return {
        "packets": packets,
        "milestones": milestones,
        "decommission_batches": batches,
    }


def _trusted(commit: str | None) -> dict[str, bool]:
    return {"exists": commit == "trusted", "ancestor": commit == "trusted"}


def _observation(recipe: str) -> CommandObservation:
    return CommandObservation(
        command=("just", recipe),
        exit_code=0,
        selected_test_count=1,
        elapsed_ms=1,
        resource_summary={
            "child_user_cpu_ms": 1,
            "child_system_cpu_ms": 0,
            "children_ru_maxrss": 1,
            "ru_maxrss_unit": "test",
        },
        output_sha256="0" * 64,
    )


def test_int_owner_adjusted_plan_scope_derives_exactly_56_oracles() -> None:
    contract = load_contract(ROOT)
    catalog = derive_oracle_catalog(ROOT, contract)
    assert tuple(dict.fromkeys(item.packet for item in catalog)) == EXPECTED_PACKETS
    assert (
        tuple(dict.fromkeys(item.category for item in catalog)) == EXPECTED_CATEGORIES
    )
    assert len(catalog) == 56
    assert not any(item.packet == "WP28" for item in catalog)


def test_int_wp28_cannot_reenter_certification_scope(tmp_path: Path) -> None:
    root = _copy_contract(tmp_path / "repo")

    def mutation(contract: dict[str, object]) -> None:
        scope = contract["scope"]
        assert isinstance(scope, dict) and isinstance(scope["packets"], list)
        scope["packets"].insert(0, "WP28")
        scope["oracle_count"] = 60

    _mutate_contract(root, mutation)
    with pytest.raises(SuccessorCertificationError, match="exactly WP29-WP42"):
        load_contract(root)


def test_int_record_schema_cannot_reduce_the_56_result_cardinality(
    tmp_path: Path,
) -> None:
    root = _copy_contract(tmp_path / "repo")
    contract = load_contract(root)
    schema_path = root / str(contract["certification_record_schema_path"])
    schema = json.loads(schema_path.read_text(encoding="utf-8"))
    schema["properties"]["oracle_results"]["minItems"] = 55
    schema_path.write_text(json.dumps(schema, indent=2) + "\n", encoding="utf-8")
    with pytest.raises(SuccessorCertificationError, match="exactly 56 results"):
        load_contract(root)


def test_int_candidate_state_requires_ancestral_wp29_wp41_proofs() -> None:
    state = _candidate_state()
    validate_state_provenance(state, phase="candidate", trust_resolver=_trusted)
    packets = state["packets"]
    assert isinstance(packets, dict) and isinstance(packets["WP36"], dict)
    packets["WP36"]["proving_commit"] = "not-ancestral"
    with pytest.raises(SuccessorCertificationError, match="WP36 proving commit"):
        validate_state_provenance(state, phase="candidate", trust_resolver=_trusted)


def test_int_wp28_and_m01_cannot_be_promoted_by_state(tmp_path: Path) -> None:
    del tmp_path
    state = _candidate_state()
    milestones = state["milestones"]
    assert isinstance(milestones, dict)
    milestones["M01"] = _entry("complete", "trusted")
    with pytest.raises(
        SuccessorCertificationError, match="M01 must remain invalidated"
    ):
        validate_state_provenance(state, phase="candidate", trust_resolver=_trusted)


def test_beh_every_scoped_oracle_requires_one_substantive_definition() -> None:
    catalog = derive_oracle_catalog(ROOT)
    definitions = [
        plan_assurance.OracleDefinition(item.oracle, "just", "justfile", item.oracle)
        for item in catalog
    ]
    validate_definition_scope(ROOT, catalog, definitions=definitions)
    definitions.pop()
    with pytest.raises(SuccessorCertificationError, match="lacks definitions"):
        validate_definition_scope(ROOT, catalog, definitions=definitions)


def test_beh_nonrecursive_runner_records_exactly_56_oracles(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    executed: list[str] = []

    def runner(recipe: str) -> CommandObservation:
        executed.append(recipe)
        return _observation(recipe)

    monkeypatch.setattr(
        "tooling.ci.successor_certification.validate_live_recipe_inventory",
        lambda *_args, **_kwargs: None,
    )
    monkeypatch.setattr(
        "tooling.ci.successor_certification.validate_definition_scope",
        lambda *_args, **_kwargs: None,
    )
    record = execute_certification(ROOT, runner=runner, require_prerequisites=False)
    assert len(record["oracle_results"]) == 56
    assert record["scope"]["packets"] == list(EXPECTED_PACKETS)
    assert "WP28" not in " ".join(executed)
    validate_certification_record(
        record, load_contract(ROOT), derive_oracle_catalog(ROOT)
    )


def test_beh_record_rejects_a_reintroduced_wp28_result(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        "tooling.ci.successor_certification.validate_live_recipe_inventory",
        lambda *_args, **_kwargs: None,
    )
    monkeypatch.setattr(
        "tooling.ci.successor_certification.validate_definition_scope",
        lambda *_args, **_kwargs: None,
    )
    contract = load_contract(ROOT)
    catalog = derive_oracle_catalog(ROOT, contract)
    record = execute_certification(
        ROOT, runner=_observation, require_prerequisites=False
    )
    record = copy.deepcopy(record)
    record["oracle_results"][0]["packet"] = "WP28"
    with pytest.raises(SuccessorCertificationError, match="identities differ"):
        validate_certification_record(record, contract, catalog)


def test_neg_zero_state_recipe_failure_is_never_promoted() -> None:
    def runner(recipe: str) -> CommandObservation:
        if recipe == "rejected-zero-state":
            raise SuccessorCertificationError("injected legacy route")
        return _observation(recipe)

    with pytest.raises(SuccessorCertificationError, match="injected legacy route"):
        execute_recipe_set(["positive-zero-state", "rejected-zero-state"], runner)


def test_neg_host_unavailability_policy_forbids_fallback(tmp_path: Path) -> None:
    root = _copy_contract(tmp_path / "repo")

    def mutation(contract: dict[str, object]) -> None:
        policy = contract["host_capability_policy"]
        assert isinstance(policy, dict)
        policy["fallback"] = "trusted-local"

    _mutate_contract(root, mutation)
    with pytest.raises(SuccessorCertificationError, match="fail closed"):
        load_contract(root)


def test_neg_legacy_hash_cannot_become_semantic_acceptance(tmp_path: Path) -> None:
    root = _copy_contract(tmp_path / "repo")

    def mutation(contract: dict[str, object]) -> None:
        contract["legacy_hash"] = "0" * 64

    _mutate_contract(root, mutation)
    with pytest.raises(SuccessorCertificationError, match="fields differ"):
        load_contract(root)


def test_ops_release_composition_contains_exactly_four_domains() -> None:
    contract = load_contract(ROOT)
    domains = tuple(entry["domain"] for entry in contract["four_domain_release"])
    assert domains == EXPECTED_DOMAINS
    recipes = four_domain_recipes(contract)
    assert "semantic-sandbox-host-matrix-check" in recipes
    assert "stable-graph-check" in recipes
    assert "adapter-wheel-test" in recipes


def test_ops_missing_domain_is_rejected(tmp_path: Path) -> None:
    root = _copy_contract(tmp_path / "repo")

    def mutation(contract: dict[str, object]) -> None:
        domains = contract["four_domain_release"]
        assert isinstance(domains, list)
        domains.pop()

    _mutate_contract(root, mutation)
    with pytest.raises(SuccessorCertificationError, match="exactly four domains"):
        load_contract(root)


def test_ops_unavailable_host_is_profile_limited_not_fabricated_success(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        "tooling.ci.successor_certification.validate_live_recipe_inventory",
        lambda *_args, **_kwargs: None,
    )
    monkeypatch.setattr(
        "tooling.ci.successor_certification.validate_definition_scope",
        lambda *_args, **_kwargs: None,
    )
    record = execute_certification(
        ROOT, runner=_observation, require_prerequisites=False
    )
    assert record["certification_state"] == "architecture_certified_profile_limited"
    assert record["release_state"] == "blocked_by_unavailable_profile"
    profile = record["environment"]["host_profiles"][0]
    assert profile == {
        "profile": "untrusted-provider-execution",
        "availability": "unavailable",
        "admission": "fail_closed",
        "architecture_effect": "profile_unavailable",
    }
