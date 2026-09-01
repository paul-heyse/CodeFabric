"""Focused positive and falsification tests for the WP38 evidence transaction."""

from __future__ import annotations

import json
import shutil
from collections.abc import Mapping
from pathlib import Path
from typing import Any

import pytest

from tooling.ci.production_evidence import (
    ARTIFACT_BOUND_CLAIM_TESTS,
    BEHAVIOR_ORACLE,
    CAUSAL_ORACLE,
    CLAIM_018_TESTS,
    EXPECTED_ACCEPTANCE_INPUTS,
    EXPECTED_ENTRY_KINDS,
    EXPECTED_LIMITATIONS,
    EXPECTED_OPERATION_CLAIMS,
    EXPECTED_RECIPE_DEPENDENCIES,
    INPUT_ORACLE,
    JUSTFILE_PATH,
    OPERATIONS_ORACLE,
    ORACLES,
    ROOT,
    SUITE,
    TRANSACTION_ID,
    TRANSACTION_PATH,
    ProductionEvidenceError,
    _canonical_b3,
    _expectation_index,
    _select_claims,
    validate_append_only_transaction,
    validate_claim_001_live_binding,
)
from tooling.ci.successor_evidence_issuance import (
    EXPECTATIONS_PATH,
    FIXTURES_PATH,
    ISSUANCE_PATH,
)


def _load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict)
    return value


def _load_jsonl(path: Path) -> list[dict[str, Any]]:
    result = [
        json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()
    ]
    assert all(isinstance(row, dict) for row in result)
    return result


def _rechain(entries: list[dict[str, Any]]) -> None:
    previous: str | None = None
    for sequence, entry in enumerate(entries, 1):
        entry["sequence"] = sequence
        entry["previous_entry_b3"] = previous
        unsigned = {key: value for key, value in entry.items() if key != "entry_b3"}
        entry["entry_b3"] = _canonical_b3(unsigned)
        previous = entry["entry_b3"]


def _append_entry(
    entries: list[dict[str, Any]],
    entry_kind: str,
    payload: Mapping[str, Any],
    *,
    recorded_by: str,
) -> None:
    entry: dict[str, Any] = {
        "schema_version": 1,
        "sequence": len(entries) + 1,
        "transaction_id": TRANSACTION_ID,
        "entry_kind": entry_kind,
        "recorded_by": recorded_by,
        "previous_entry_b3": entries[-1]["entry_b3"] if entries else None,
        "payload": dict(payload),
    }
    entry["entry_b3"] = _canonical_b3(entry)
    entries.append(entry)


def _draft_entries(root: Path = ROOT) -> list[dict[str, Any]]:
    issuance = _load_json(root / ISSUANCE_PATH)
    expectations = _load_jsonl(root / EXPECTATIONS_PATH)
    author = issuance["author"]
    reviewer = issuance["reviewer"]
    assert isinstance(author, dict)
    assert isinstance(reviewer, dict)

    entries: list[dict[str, Any]] = []
    _append_entry(
        entries,
        "transaction_opened",
        {
            "suite": SUITE,
            "packet": "WP38",
            "oracles": list(ORACLES),
            "acceptance_inputs": list(EXPECTED_ACCEPTANCE_INPUTS),
            "diagnostic_inputs": [],
            "issuance_binding": {
                "issuance_id": issuance["issuance_id"],
                "reviewed_content_id": issuance["reviewed_content_id"],
                "expectation_author": author["identity"],
                "independent_reviewer": reviewer["identity"],
            },
        },
        recorded_by="wp38-production-evidence-executor",
    )
    claim_rows: list[dict[str, Any]] = []
    for expectation in expectations:
        consumer = expectation["future_consumer"]
        assert isinstance(consumer, dict)
        claim_id = expectation["claim_id"]
        assert isinstance(claim_id, str)
        claim_rows.append(
            {
                "claim_id": claim_id,
                "claim_family": expectation["claim_family"],
                "issued_observation_recipe": consumer["oracle"],
                "input_oracle": INPUT_ORACLE,
                "positive_oracle": BEHAVIOR_ORACLE,
                "causal_oracle": CAUSAL_ORACLE,
                "operations_oracle": (
                    OPERATIONS_ORACLE if claim_id in EXPECTED_OPERATION_CLAIMS else None
                ),
            }
        )
    _append_entry(
        entries,
        "claim_oracle_mapping",
        {"claims": claim_rows},
        recorded_by="wp38-production-evidence-executor",
    )
    _append_entry(
        entries,
        "production_execution_contract",
        {
            "recipe_dependencies": {
                oracle: list(dependencies)
                for oracle, dependencies in EXPECTED_RECIPE_DEPENDENCIES.items()
            },
            "claim_018_successor_tests": list(CLAIM_018_TESTS),
            "production_observation_mode": "live_successor_recipe_execution",
            "historical_executables_required": False,
        },
        recorded_by="wp38-production-evidence-executor",
    )
    evidence = {
        "HOST-UNTRUSTED-CONTAINMENT": [
            "bubblewrap 0.9.0 observed as diagnostic launcher metadata, not admission authority",
            "application-owned compiled seccomp policy absent",
            "capability matrix lacks the full hostile escape proof; untrusted execution remains unavailable",
        ],
        "PERFORMANCE-EVIDENCE-NOT-CLAIMED": [
            (
                "No representative production workload or regression baseline is accepted; "
                "WP38 makes no performance or regression claim."
            )
        ],
        "SCHEDULED-DEEP-ASSURANCE-DEFERRED": [
            "scheduled mutation, fuzz, coverage, and supported-host assurance remain separate"
        ],
        "SUPPORTED-PLATFORM-COVERAGE": [
            "current observations cover one Linux local-workstation development profile"
        ],
    }
    _append_entry(
        entries,
        "limitations_recorded",
        {
            "development_readiness": "eligible_after_live_oracles",
            "release_certification": "not_decided_by_wp38",
            "limitations": [
                {
                    "id": limitation_id,
                    "state": state,
                    "release_effect": release_effect,
                    "evidence": evidence[limitation_id],
                }
                for limitation_id, (
                    state,
                    release_effect,
                ) in EXPECTED_LIMITATIONS.items()
            ],
        },
        recorded_by="wp38-production-evidence-executor",
    )
    assert tuple(entry["entry_kind"] for entry in entries) == EXPECTED_ENTRY_KINDS
    return entries


def _append_review(entries: list[dict[str, Any]]) -> None:
    reviewed_tip = entries[-1]["entry_b3"]
    _append_entry(
        entries,
        "review_accepted",
        {
            "reviewer_identity": "wp38-independent-transaction-reviewer-fixture",
            "reviewed_through_entry_b3": reviewed_tip,
            "implementation_owner": False,
            "expectation_author": False,
            "verdict": "accepted",
            "scope": [
                "append_only_chain",
                "successor_only_acceptance_edges",
                "claim_oracle_mapping",
                "production_recipe_closure",
                "limitations",
            ],
        },
        recorded_by="wp38-independent-transaction-reviewer-fixture",
    )


def _write_entries(root: Path, entries: list[dict[str, Any]]) -> None:
    path = root / TRANSACTION_PATH
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "".join(
            json.dumps(entry, separators=(",", ":"), ensure_ascii=False) + "\n"
            for entry in entries
        ),
        encoding="utf-8",
    )


def _candidate_root(destination: Path, *, reviewed: bool = False) -> Path:
    paths = (EXPECTATIONS_PATH, FIXTURES_PATH, ISSUANCE_PATH, JUSTFILE_PATH)
    for path in paths:
        (destination / path).parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / path, destination / path)
    entries = _draft_entries(destination)
    if reviewed:
        _append_review(entries)
    _write_entries(destination, entries)
    return destination


def _mutate_and_rechain(root: Path, mutation: Any) -> None:
    entries = _load_jsonl(root / TRANSACTION_PATH)
    mutation(entries)
    _rechain(entries)
    _write_entries(root, entries)


def test_int_complete_draft_is_valid_but_independent_review_is_required(
    tmp_path: Path,
) -> None:
    root = _candidate_root(tmp_path / "repo")
    assert validate_append_only_transaction(root, require_review=False) == 18
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_append_only_transaction(root)
    assert failure.value.code == "PRODUCTION_EVIDENCE_TRANSACTION_REVIEW_REQUIRED"


def test_int_repository_transaction_is_mechanically_closed() -> None:
    assert validate_append_only_transaction(ROOT, require_review=False) == 18


def test_int_every_claim_has_detectable_positive_causal_negative_fixture_consumers() -> (
    None
):
    assert set(ARTIFACT_BOUND_CLAIM_TESTS) == {
        f"RFV3-CLAIM-{number:03d}" for number in range(1, 19)
    }
    selectors = [
        selector
        for claim_selectors in ARTIFACT_BOUND_CLAIM_TESTS.values()
        for selector in claim_selectors
    ]
    assert all(
        len(claim_selectors) == 3
        for claim_selectors in ARTIFACT_BOUND_CLAIM_TESTS.values()
    )
    assert len(selectors) == len(set(selectors)) == 54
    justfile = (ROOT / JUSTFILE_PATH).read_text(encoding="utf-8")
    assert all(selector in justfile for selector in selectors)


def test_int_independently_reviewed_candidate_closes_all_claims(tmp_path: Path) -> None:
    root = _candidate_root(tmp_path / "repo", reviewed=True)
    assert validate_append_only_transaction(root) == 18


def test_int_payload_mutation_without_rehash_is_rejected(tmp_path: Path) -> None:
    root = _candidate_root(tmp_path / "repo", reviewed=True)
    entries = _load_jsonl(root / TRANSACTION_PATH)
    entries[0]["payload"]["packet"] = "WP99"
    _write_entries(root, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_append_only_transaction(root)
    assert failure.value.code == "PRODUCTION_EVIDENCE_APPEND_ONLY_CHAIN_INVALID"


def test_int_row_deletion_and_reordering_are_rejected(tmp_path: Path) -> None:
    root = _candidate_root(tmp_path / "repo", reviewed=True)
    entries = _load_jsonl(root / TRANSACTION_PATH)
    entries[1], entries[2] = entries[2], entries[1]
    _write_entries(root, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_append_only_transaction(root)
    assert failure.value.code == "PRODUCTION_EVIDENCE_APPEND_ONLY_CHAIN_INVALID"

    entries = _draft_entries(root)
    entries.pop(1)
    _rechain(entries)
    _write_entries(root, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_append_only_transaction(root, require_review=False)
    assert failure.value.code == "PRODUCTION_EVIDENCE_APPEND_ONLY_CHAIN_INVALID"


def test_int_duplicate_json_member_is_rejected(tmp_path: Path) -> None:
    root = _candidate_root(tmp_path / "repo")
    path = root / TRANSACTION_PATH
    text = path.read_text(encoding="utf-8")
    path.write_text(
        text.replace(
            '{"schema_version":1,', '{"schema_version":1,"schema_version":1,', 1
        ),
        encoding="utf-8",
    )
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_append_only_transaction(root, require_review=False)
    assert failure.value.code == "PRODUCTION_EVIDENCE_DUPLICATE_JSON_MEMBER"


def test_int_claim_001_live_drift_is_a_typed_reissuance_blocker(
    tmp_path: Path,
) -> None:
    root = tmp_path / "repo"
    schema = Path("contracts/rpc/provider_control.proto")
    for path in (EXPECTATIONS_PATH, schema):
        (root / path).parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / path, root / path)
    (root / schema).write_bytes((root / schema).read_bytes() + b"\n// causal drift\n")
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_claim_001_live_binding(root)
    assert failure.value.code == "PRODUCTION_EVIDENCE_INPUT_REISSUANCE_REQUIRED"
    assert failure.value.details["claim_id"] == "RFV3-CLAIM-001"
    assert failure.value.details["reviewed_b3"] != failure.value.details["live_b3"]


def test_int_diagnostic_acceptance_input_is_rejected(tmp_path: Path) -> None:
    root = _candidate_root(tmp_path / "repo")

    def mutate(entries: list[dict[str, Any]]) -> None:
        entries[0]["payload"]["diagnostic_inputs"] = ["historical execution"]

    _mutate_and_rechain(root, mutate)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_append_only_transaction(root, require_review=False)
    assert failure.value.code == "PRODUCTION_EVIDENCE_FORBIDDEN_ACCEPTANCE_EDGE"


def test_beh_every_claim_has_one_exact_live_positive_mapping(tmp_path: Path) -> None:
    root = _candidate_root(tmp_path / "repo", reviewed=True)
    assert validate_append_only_transaction(root) == 18

    def mutate(entries: list[dict[str, Any]]) -> None:
        entries[1]["payload"]["claims"].pop()

    _mutate_and_rechain(root, mutate)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_append_only_transaction(root)
    assert failure.value.code == "PRODUCTION_EVIDENCE_CLAIM_CLOSURE_INVALID"


def test_beh_claim_018_must_name_all_real_successor_tests(tmp_path: Path) -> None:
    root = _candidate_root(tmp_path / "repo", reviewed=True)

    def mutate(entries: list[dict[str, Any]]) -> None:
        entries[2]["payload"]["claim_018_successor_tests"][0] = "fake_equivalence"

    _mutate_and_rechain(root, mutate)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_append_only_transaction(root)
    assert failure.value.code == "PRODUCTION_EVIDENCE_CLAIM_018_EXECUTION_INVALID"


def test_neg_historical_dependency_cannot_enter_the_pass_fail_dag(
    tmp_path: Path,
) -> None:
    root = _candidate_root(tmp_path / "repo", reviewed=True)

    def mutate(entries: list[dict[str, Any]]) -> None:
        dependencies = entries[2]["payload"]["recipe_dependencies"][BEHAVIOR_ORACLE]
        dependencies[-1] = "comparator-check"

    _mutate_and_rechain(root, mutate)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_append_only_transaction(root)
    assert failure.value.code == "PRODUCTION_EVIDENCE_ORACLE_CLOSURE_INVALID"


def test_neg_zero_or_unknown_claim_selection_fails_closed(tmp_path: Path) -> None:
    root = _candidate_root(tmp_path / "repo")
    expectations = _expectation_index(root)
    with pytest.raises(ProductionEvidenceError) as failure:
        _select_claims(expectations, ["RFV3-CLAIM-999"])
    assert failure.value.code == "PRODUCTION_EVIDENCE_ZERO_SELECTION"


def test_ops_operation_claim_scope_is_exact(tmp_path: Path) -> None:
    root = _candidate_root(tmp_path / "repo", reviewed=True)

    def mutate(entries: list[dict[str, Any]]) -> None:
        claims = entries[1]["payload"]["claims"]
        claim = next(row for row in claims if row["claim_id"] == "RFV3-CLAIM-012")
        claim["operations_oracle"] = None

    _mutate_and_rechain(root, mutate)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_append_only_transaction(root)
    assert failure.value.code == "PRODUCTION_EVIDENCE_ORACLE_MAPPING_INVALID"


def test_ops_untrusted_containment_cannot_be_promoted_to_green(tmp_path: Path) -> None:
    root = _candidate_root(tmp_path / "repo", reviewed=True)

    def mutate(entries: list[dict[str, Any]]) -> None:
        limitations = entries[3]["payload"]["limitations"]
        containment = next(
            row for row in limitations if row["id"] == "HOST-UNTRUSTED-CONTAINMENT"
        )
        containment["state"] = "supported"
        containment["release_effect"] = "none"

    _mutate_and_rechain(root, mutate)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_append_only_transaction(root)
    assert failure.value.code == "PRODUCTION_EVIDENCE_LIMITATION_INVALID"


def test_ops_independent_review_must_bind_the_current_chain_tip(tmp_path: Path) -> None:
    root = _candidate_root(tmp_path / "repo", reviewed=True)
    entries = _load_jsonl(root / TRANSACTION_PATH)
    entries[-1]["payload"]["reviewed_through_entry_b3"] = "b3:" + "0" * 64
    _rechain(entries)
    _write_entries(root, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_append_only_transaction(root)
    assert failure.value.code == "PRODUCTION_EVIDENCE_TRANSACTION_REVIEW_INVALID"


def test_ops_transaction_executor_cannot_self_attest_independence(
    tmp_path: Path,
) -> None:
    root = _candidate_root(tmp_path / "repo", reviewed=True)
    entries = _load_jsonl(root / TRANSACTION_PATH)
    executor = entries[0]["recorded_by"]
    entries[-1]["recorded_by"] = executor
    entries[-1]["payload"]["reviewer_identity"] = executor
    _rechain(entries)
    _write_entries(root, entries)
    with pytest.raises(ProductionEvidenceError) as failure:
        validate_append_only_transaction(root)
    assert failure.value.code == "PRODUCTION_EVIDENCE_TRANSACTION_REVIEW_INVALID"


def test_ops_every_required_limitation_retains_its_exact_release_effect(
    tmp_path: Path,
) -> None:
    root = _candidate_root(tmp_path / "repo", reviewed=True)
    assert validate_append_only_transaction(root) == 18
    entries = _load_jsonl(root / TRANSACTION_PATH)
    observed = {
        row["id"]: (row["state"], row["release_effect"])
        for row in entries[3]["payload"]["limitations"]
    }
    assert observed == EXPECTED_LIMITATIONS
    assert observed["PERFORMANCE-EVIDENCE-NOT-CLAIMED"] == ("not_claimed", "none")
