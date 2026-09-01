"""Mutation tests for the frozen v4 supervisor/launcher contract gate."""

from __future__ import annotations

import copy
import json
from collections.abc import Callable
from pathlib import Path

import pytest

import tooling.ci.supervisor_launch_contract_v4 as contract
from tooling.ci.successor_evidence_issuance_v4 import (
    EVIDENCE_RELEASE,
    EVIDENCE_ROOT,
    EXPECTATIONS_PATH,
    FIXTURES_PATH,
    ROOT,
    SUITE_IDENTITY,
    V4Issuance,
)
from tooling.ci.supervisor_launch_contract_v4 import (
    EXPECTED_NEGATIVE_TOTAL,
    NEGATIVE_CODES,
    REQUIRED_AUTHORITY_TOKENS,
    SupervisorLaunchContractError,
    _under_root,
    _validate_frozen_artifact_hashes,
    _validate_required_authority_tokens,
    validate_selected_contract,
    validate_supervisor_launch_contract,
)


def _load_jsonl(path: Path) -> tuple[dict[str, object], ...]:
    return tuple(
        json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()
    )


def _candidate() -> V4Issuance:
    return V4Issuance(
        _load_jsonl(ROOT / EXPECTATIONS_PATH),
        _load_jsonl(ROOT / FIXTURES_PATH),
        {
            "status": "accepted",
            "suite_identity": SUITE_IDENTITY,
            "evidence_release": EVIDENCE_RELEASE,
        },
        {},
    )


def _mutated_candidate(
    mutate: Callable[[list[dict[str, object]], list[dict[str, object]]], None],
) -> V4Issuance:
    candidate = _candidate()
    expectations = copy.deepcopy(list(candidate.expectations))
    fixtures = copy.deepcopy(list(candidate.fixtures))
    mutate(expectations, fixtures)
    return V4Issuance(
        tuple(expectations),
        tuple(fixtures),
        copy.deepcopy(candidate.issuance),
        {},
    )


def _expectation(rows: list[dict[str, object]], claim_id: str) -> dict[str, object]:
    return next(row for row in rows if row["claim_id"] == claim_id)


def _fixture(
    rows: list[dict[str, object]], claim_id: str, kind: str
) -> dict[str, object]:
    return next(
        row
        for row in rows
        if row["claim_id"] == claim_id and row["fixture_kind"] == kind
    )


def _case(
    fixture: dict[str, object], reason: str, *, decoded: bool = False
) -> dict[str, object]:
    container = fixture["expected_decoded"] if decoded else fixture["fixture_input"]
    assert isinstance(container, dict)
    if decoded:
        cases = container["cases"]
    else:
        invalid = container["invalid_change"]
        assert isinstance(invalid, dict)
        cases = invalid["cases"]
    assert isinstance(cases, list)
    return next(
        row for row in cases if isinstance(row, dict) and row.get("reason") == reason
    )


def test_supervisor_contract_selects_exact_reviewed_slice() -> None:
    report = validate_selected_contract(ROOT, _candidate())

    assert report["status"] == "accepted"
    assert report["selected_claims"] == 5
    assert report["selected_fixtures"] == 10
    assert report["selected_negative_scenarios"] == EXPECTED_NEGATIVE_TOTAL
    selector = report["selector"]
    assert isinstance(selector, dict)
    assert selector["claim_ids"] == [
        f"RFV4-CLAIM-{number:03d}" for number in range(19, 24)
    ]
    assert report["negative_reason_code_closure"] == {
        claim_id: [
            {"reason": reason, "code": code} for reason, code in reason_codes.items()
        ]
        for claim_id, reason_codes in NEGATIVE_CODES.items()
    }


def test_supervisor_contract_entrypoint_requires_reviewed_issuance(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    observed: dict[str, object] = {}

    def fake_validate(root: Path, *, require_review: bool) -> V4Issuance:
        observed["root"] = root
        observed["require_review"] = require_review
        return _candidate()

    monkeypatch.setattr(contract, "validate_issuance", fake_validate)
    report = validate_supervisor_launch_contract(ROOT)

    assert observed == {"root": ROOT.resolve(), "require_review": True}
    assert report["status"] == "accepted"


def test_supervisor_contract_rejects_pending_review() -> None:
    candidate = _candidate()
    issuance = dict(candidate.issuance)
    issuance["status"] = "pending-independent-review"
    pending = V4Issuance(candidate.expectations, candidate.fixtures, issuance, {})

    with pytest.raises(
        SupervisorLaunchContractError, match="independently accepted"
    ) as error:
        validate_selected_contract(ROOT, pending)

    assert error.value.code == "SUPERVISOR_REVIEW_REQUIRED"


def test_supervisor_contract_rejects_zero_selection() -> None:
    def mutate(
        expectations: list[dict[str, object]], fixtures: list[dict[str, object]]
    ) -> None:
        expectations[:] = [
            row
            for row in expectations
            if row["claim_id"] not in contract.SELECTED_CLAIMS
        ]
        fixtures[:] = [
            row for row in fixtures if row["claim_id"] not in contract.SELECTED_CLAIMS
        ]

    with pytest.raises(SupervisorLaunchContractError) as error:
        validate_selected_contract(ROOT, _mutated_candidate(mutate))

    assert error.value.code == "SUPERVISOR_SELECTOR_ZERO_SELECTION"


def test_supervisor_contract_rejects_claim_family_substitution() -> None:
    def mutate(
        expectations: list[dict[str, object]], _fixtures: list[dict[str, object]]
    ) -> None:
        _expectation(expectations, "RFV4-CLAIM-021")["family"] = "rpc_handshake"

    with pytest.raises(SupervisorLaunchContractError) as error:
        validate_selected_contract(ROOT, _mutated_candidate(mutate))

    assert error.value.code == "SUPERVISOR_SELECTOR_CLOSURE_INVALID"


@pytest.mark.parametrize(
    ("claim_id", "reason"),
    [
        (claim_id, reason)
        for claim_id, reason_codes in NEGATIVE_CODES.items()
        for reason in reason_codes
    ],
)
def test_supervisor_contract_rejects_removed_negative_scenario(
    claim_id: str, reason: str
) -> None:
    def mutate(
        _expectations: list[dict[str, object]], fixtures: list[dict[str, object]]
    ) -> None:
        target = _fixture(fixtures, claim_id, "negative")
        fixture_input = target["fixture_input"]
        decoded = target["expected_decoded"]
        assert isinstance(fixture_input, dict) and isinstance(decoded, dict)
        invalid = fixture_input["invalid_change"]
        assert isinstance(invalid, dict)
        invalid["cases"] = [
            row
            for row in invalid["cases"]
            if isinstance(row, dict) and row.get("reason") != reason
        ]
        decoded["cases"] = [
            row
            for row in decoded["cases"]
            if isinstance(row, dict) and row.get("reason") != reason
        ]

    with pytest.raises(SupervisorLaunchContractError) as error:
        validate_selected_contract(ROOT, _mutated_candidate(mutate))

    assert error.value.code == "SUPERVISOR_NEGATIVE_CLOSURE_INVALID"


@pytest.mark.parametrize(
    ("path", "root", "expected"),
    [
        ("/etc/codefabric/policies/agent.json", "/etc/codefabric/policies", True),
        ("/etc/codefabric/policies", "/etc/codefabric/policies", False),
        (
            "/etc/codefabric/policies-other/agent.json",
            "/etc/codefabric/policies",
            False,
        ),
        ("/etc/codefabric/policies/../secret", "/etc/codefabric/policies", False),
        ("etc/codefabric/policies/agent.json", "/etc/codefabric/policies", False),
        ("/etc/codefabric//policies/agent.json", "/etc/codefabric/policies", False),
    ],
)
def test_supervisor_contract_normalizes_absolute_containment(
    path: str, root: str, expected: bool
) -> None:
    assert _under_root(path, root) is expected


def test_supervisor_contract_rejects_unverified_policy_owner() -> None:
    def mutate(
        expectations: list[dict[str, object]], _fixtures: list[dict[str, object]]
    ) -> None:
        row = _expectation(expectations, "RFV4-CLAIM-019")
        controlled = row["controlled_input"]
        assert isinstance(controlled, dict)
        policy_file = controlled["policy_file"]
        assert isinstance(policy_file, dict)
        policy_file["owner_uid"] = 1000

    with pytest.raises(SupervisorLaunchContractError) as error:
        validate_selected_contract(ROOT, _mutated_candidate(mutate))

    assert error.value.code == "SUPERVISOR_POLICY_FILE_UNSAFE"


@pytest.mark.parametrize(
    "mutation",
    ["zero_causal_bound", "unobservable_causal_executable"],
)
def test_supervisor_contract_rejects_invalid_causal_policy_authority(
    mutation: str,
) -> None:
    def mutate(
        _expectations: list[dict[str, object]], fixtures: list[dict[str, object]]
    ) -> None:
        causal = _fixture(fixtures, "RFV4-CLAIM-019", "causal")
        fixture_input = causal["fixture_input"]
        decoded = causal["expected_decoded"]
        assert isinstance(fixture_input, dict) and isinstance(decoded, dict)
        patch = fixture_input["merge_patch"]
        assert isinstance(patch, dict)
        policy = patch["policy"]
        assert isinstance(policy, dict)
        if mutation == "zero_causal_bound":
            bounds = policy["resource_bounds"]
            decoded_bounds = decoded["resource_bounds"]
            assert isinstance(bounds, dict) and isinstance(decoded_bounds, dict)
            bounds["max_running_queries"] = 0
            decoded_bounds["max_running_queries"] = 0
            return
        identity = policy["adapter_identity"]
        decoded_identity = decoded["adapter_identity"]
        assert isinstance(identity, dict) and isinstance(decoded_identity, dict)
        identity["executable_observation_supported"] = False
        decoded_identity["executable_observation_supported"] = False

    with pytest.raises(SupervisorLaunchContractError) as error:
        validate_selected_contract(ROOT, _mutated_candidate(mutate))

    assert error.value.code == "SUPERVISOR_POLICY_CAUSAL_INVALID"


def test_supervisor_contract_rejects_live_socket_without_inode_discrimination() -> None:
    def mutate(
        expectations: list[dict[str, object]], fixtures: list[dict[str, object]]
    ) -> None:
        controlled = _expectation(expectations, "RFV4-CLAIM-020")["controlled_input"]
        assert isinstance(controlled, dict)
        socket = controlled["supervisor_socket"]
        assert isinstance(socket, dict)
        row = _case(
            _fixture(fixtures, "RFV4-CLAIM-020", "negative"), "live_foreign_socket"
        )
        foreign = row["supervisor_socket"]
        assert isinstance(foreign, dict)
        foreign["inode"] = socket["inode"]

    with pytest.raises(SupervisorLaunchContractError) as error:
        validate_selected_contract(ROOT, _mutated_candidate(mutate))

    assert error.value.code == "SUPERVISOR_SINGLETON_NEGATIVE_INVALID"


@pytest.mark.parametrize(
    ("reason", "field", "value"),
    [
        ("owned_stale_socket_recovery", "unlinked_paths", 0),
        ("partial_spawn_before_control_ack", "child_reaped", False),
    ],
)
def test_supervisor_contract_rejects_incomplete_singleton_cleanup(
    reason: str, field: str, value: object
) -> None:
    def mutate(
        _expectations: list[dict[str, object]], fixtures: list[dict[str, object]]
    ) -> None:
        negative = _fixture(fixtures, "RFV4-CLAIM-020", "negative")
        _case(negative, reason, decoded=True)[field] = value

    with pytest.raises(SupervisorLaunchContractError) as error:
        validate_selected_contract(ROOT, _mutated_candidate(mutate))

    assert error.value.code == "SUPERVISOR_SINGLETON_NEGATIVE_INVALID"


def test_supervisor_contract_rejects_unproved_runtime_root_output() -> None:
    def mutate(
        expectations: list[dict[str, object]], _fixtures: list[dict[str, object]]
    ) -> None:
        decoded = _expectation(expectations, "RFV4-CLAIM-020")["expected_decoded"]
        assert isinstance(decoded, dict)
        decoded["runtime_root_safe"] = False

    with pytest.raises(SupervisorLaunchContractError) as error:
        validate_selected_contract(ROOT, _mutated_candidate(mutate))

    assert error.value.code == "SUPERVISOR_SINGLETON_INVALID"


def test_supervisor_contract_rejects_non_oversized_control_record() -> None:
    def mutate(
        expectations: list[dict[str, object]], fixtures: list[dict[str, object]]
    ) -> None:
        controlled = _expectation(expectations, "RFV4-CLAIM-021")["controlled_input"]
        assert isinstance(controlled, dict)
        row = _case(
            _fixture(fixtures, "RFV4-CLAIM-021", "negative"), "record_too_large"
        )
        record = row["record"]
        assert isinstance(record, dict)
        record["declared_length_bytes"] = controlled["governed_max_record_bytes"]

    with pytest.raises(SupervisorLaunchContractError) as error:
        validate_selected_contract(ROOT, _mutated_candidate(mutate))

    assert error.value.code == "SUPERVISOR_CONTROL_NEGATIVE_INVALID"


@pytest.mark.parametrize(
    ("reason", "field", "value"),
    [
        ("exact_replay", "new_mutations", 1),
        ("channel_loss", "accepted_pinned_work", "cancelled"),
    ],
)
def test_supervisor_contract_rejects_control_replay_or_loss_mutation(
    reason: str, field: str, value: object
) -> None:
    def mutate(
        _expectations: list[dict[str, object]], fixtures: list[dict[str, object]]
    ) -> None:
        negative = _fixture(fixtures, "RFV4-CLAIM-021", "negative")
        _case(negative, reason, decoded=True)[field] = value

    with pytest.raises(SupervisorLaunchContractError) as error:
        validate_selected_contract(ROOT, _mutated_candidate(mutate))

    assert error.value.code == "SUPERVISOR_CONTROL_NEGATIVE_INVALID"


@pytest.mark.parametrize(
    ("mutation", "code"),
    [
        ("proxy_stdout", "SUPERVISOR_FD3_INVALID"),
        ("unsafe_fallback", "SUPERVISOR_FALLBACK_INVALID"),
        ("leaked_partial_child", "SUPERVISOR_PARTIAL_ADAPTER_INVALID"),
    ],
)
def test_supervisor_contract_rejects_fd3_and_partial_spawn_mutations(
    mutation: str, code: str
) -> None:
    def mutate(
        expectations: list[dict[str, object]], fixtures: list[dict[str, object]]
    ) -> None:
        if mutation == "leaked_partial_child":
            target = _fixture(fixtures, "RFV4-CLAIM-022", "negative")
            _case(target, "partial_adapter_spawn", decoded=True)[
                "lifecycle_tasks_joined"
            ] = False
            return
        row = _expectation(expectations, "RFV4-CLAIM-022")
        controlled = row["controlled_input"]
        assert isinstance(controlled, dict)
        if mutation == "proxy_stdout":
            descriptors = controlled["child_descriptors"]
            assert isinstance(descriptors, dict)
            descriptors["stdout"] = "launcher-proxy"
        else:
            fallback = controlled["one_shot_fallback"]
            assert isinstance(fallback, dict)
            fallback["mode"] = "0644"

    with pytest.raises(SupervisorLaunchContractError) as error:
        validate_selected_contract(ROOT, _mutated_candidate(mutate))

    assert error.value.code == code


@pytest.mark.parametrize(
    ("mutation", "code"),
    [
        (
            "base_probe_disallows_fixed_fd3",
            "SUPERVISOR_FALLBACK_PLATFORM_CONDITION_INVALID",
        ),
        (
            "safe_fallback_probe_passed",
            "SUPERVISOR_FALLBACK_PLATFORM_CONDITION_INVALID",
        ),
        (
            "unsafe_fallback_missing_probe",
            "SUPERVISOR_FALLBACK_PLATFORM_EVIDENCE_MISSING",
        ),
    ],
)
def test_supervisor_contract_rejects_unproved_fallback_selection(
    mutation: str, code: str
) -> None:
    def mutate(
        expectations: list[dict[str, object]], fixtures: list[dict[str, object]]
    ) -> None:
        if mutation == "base_probe_disallows_fixed_fd3":
            controlled = _expectation(expectations, "RFV4-CLAIM-022")[
                "controlled_input"
            ]
            assert isinstance(controlled, dict)
            platform = controlled["platform_descriptor_capability"]
            assert isinstance(platform, dict)
            platform["fixed_fd3_inheritance_available"] = False
            return
        negative = _fixture(fixtures, "RFV4-CLAIM-022", "negative")
        reason = (
            "fixed_fd3_unavailable_safe_fallback"
            if mutation == "safe_fallback_probe_passed"
            else "fallback_wrong_mode"
        )
        case = _case(negative, reason)
        if mutation == "safe_fallback_probe_passed":
            platform = case["platform_descriptor_capability"]
            assert isinstance(platform, dict)
            platform["probe_status"] = "passed"
            return
        del case["platform_descriptor_capability"]

    with pytest.raises(SupervisorLaunchContractError) as error:
        validate_selected_contract(ROOT, _mutated_candidate(mutate))

    assert error.value.code == code


@pytest.mark.parametrize(
    ("reason", "field", "value", "code"),
    [
        (
            "causal_regrant",
            "fd3_channel_open",
            False,
            "SUPERVISOR_REGRANT_INVALID",
        ),
        (
            "replacement_after_terminal_eof",
            "further_handshake_authority_available",
            True,
            "SUPERVISOR_FD3_NEGATIVE_INVALID",
        ),
    ],
)
def test_supervisor_contract_rejects_broken_fd3_lifetime(
    reason: str, field: str, value: object, code: str
) -> None:
    def mutate(
        _expectations: list[dict[str, object]], fixtures: list[dict[str, object]]
    ) -> None:
        causal = _fixture(fixtures, "RFV4-CLAIM-022", "causal")
        if reason == "causal_regrant":
            decoded = causal["expected_decoded"]
            assert isinstance(decoded, dict)
            decoded[field] = value
            return
        negative = _fixture(fixtures, "RFV4-CLAIM-022", "negative")
        _case(negative, reason, decoded=True)[field] = value

    with pytest.raises(SupervisorLaunchContractError) as error:
        validate_selected_contract(ROOT, _mutated_candidate(mutate))

    assert error.value.code == code


def test_supervisor_contract_rejects_restart_resubmission() -> None:
    def mutate(
        expectations: list[dict[str, object]], _fixtures: list[dict[str, object]]
    ) -> None:
        decoded = _expectation(expectations, "RFV4-CLAIM-023")["expected_decoded"]
        assert isinstance(decoded, dict)
        decoded["query_resubmitted"] = True

    with pytest.raises(SupervisorLaunchContractError) as error:
        validate_selected_contract(ROOT, _mutated_candidate(mutate))

    assert error.value.code == "SUPERVISOR_RESTART_INVALID"


@pytest.mark.parametrize(
    ("mutation", "code"),
    [
        ("old_child_not_reaped", "SUPERVISOR_RESTART_INVALID"),
        ("revocation_runs_restart_lifecycle", "SUPERVISOR_RESTART_CAUSAL_INVALID"),
        ("restart_mints_without_fresh_grant", "SUPERVISOR_RESTART_NEGATIVE_INVALID"),
        ("restart_cancels_accepted_query", "SUPERVISOR_RESTART_NEGATIVE_INVALID"),
    ],
)
def test_supervisor_contract_rejects_restart_and_revocation_lifecycle_drift(
    mutation: str, code: str
) -> None:
    def mutate(
        expectations: list[dict[str, object]], fixtures: list[dict[str, object]]
    ) -> None:
        if mutation == "old_child_not_reaped":
            decoded = _expectation(expectations, "RFV4-CLAIM-023")["expected_decoded"]
            assert isinstance(decoded, dict)
            decoded["old_daemon_child_reaped"] = False
            return
        if mutation == "revocation_runs_restart_lifecycle":
            causal = _fixture(fixtures, "RFV4-CLAIM-023", "causal")
            decoded = causal["expected_decoded"]
            assert isinstance(decoded, dict)
            decoded["restart_lifecycle_actions"] = 1
            return
        negative = _fixture(fixtures, "RFV4-CLAIM-023", "negative")
        reason = (
            "restart_without_fresh_authority"
            if mutation == "restart_mints_without_fresh_grant"
            else "accepted_query_cancelled_on_restart"
        )
        field = (
            "session_minted"
            if mutation == "restart_mints_without_fresh_grant"
            else "accepted_query_survives"
        )
        _case(negative, reason, decoded=True)[field] = (
            mutation == "restart_mints_without_fresh_grant"
        )

    with pytest.raises(SupervisorLaunchContractError) as error:
        validate_selected_contract(ROOT, _mutated_candidate(mutate))

    assert error.value.code == code


def test_supervisor_contract_rejects_frozen_artifact_drift(tmp_path: Path) -> None:
    for relative_path in (EXPECTATIONS_PATH, FIXTURES_PATH):
        target = tmp_path / relative_path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes((ROOT / relative_path).read_bytes())

    assert _validate_frozen_artifact_hashes(tmp_path) == {
        EXPECTATIONS_PATH.as_posix(): contract.FROZEN_EXPECTATIONS_SHA256,
        FIXTURES_PATH.as_posix(): contract.FROZEN_FIXTURES_SHA256,
    }
    drifted = tmp_path / FIXTURES_PATH
    drifted.write_bytes(drifted.read_bytes() + b"\n")

    with pytest.raises(SupervisorLaunchContractError) as error:
        _validate_frozen_artifact_hashes(tmp_path)

    assert error.value.code == "SUPERVISOR_FROZEN_ARTIFACT_DRIFT"


def test_supervisor_contract_rejects_removed_accepted_authority_token() -> None:
    texts = {
        path: (ROOT / path).read_text(encoding="utf-8")
        for path in REQUIRED_AUTHORITY_TOKENS
    }
    token = REQUIRED_AUTHORITY_TOKENS[contract.SRV_PATH][2]
    texts[contract.SRV_PATH] = texts[contract.SRV_PATH].replace(token, "removed")

    with pytest.raises(SupervisorLaunchContractError) as error:
        _validate_required_authority_tokens(texts)

    assert error.value.code == "SUPERVISOR_AUTHORITY_TOKEN_MISSING"


def test_supervisor_gate_does_not_import_production_or_predecessor_modules() -> None:
    source = (ROOT / "tooling/ci/supervisor_launch_contract_v4.py").read_text(
        encoding="utf-8"
    )

    assert "codefabric_cpg_mcp" not in source
    assert "successor_evidence_issuance import" not in source
    assert "relational-fabric-v3" not in source
    assert (ROOT / EVIDENCE_ROOT).is_dir()
