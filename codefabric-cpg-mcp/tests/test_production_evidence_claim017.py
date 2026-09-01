"""Artifact-bound WP38 execution for the released daemon-response projection."""

from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path
from typing import Any

import pytest
from google.protobuf.json_format import ParseDict

from codefabric_cpg_mcp.contracts.json import canonicalize_value, checksum
from codefabric_cpg_mcp.daemon.client import (
    DaemonProjectionError,
    validate_inline_daemon_response,
)
from codefabric_cpg_mcp.daemon.generated import cpg_query_service_pb2 as query_pb

ROOT = Path(__file__).resolve().parents[2]
AUTHORITY = ROOT / "contracts" / "acceptance" / "relational-fabric-v3"


def _jsonl(name: str) -> list[dict[str, Any]]:
    return [json.loads(line) for line in (AUTHORITY / name).read_text().splitlines()]


def _claim() -> dict[str, Any]:
    return next(row for row in _jsonl("expectations.jsonl") if row["claim_id"] == "RFV3-CLAIM-017")


def _fixture(fixture_id: str) -> dict[str, Any]:
    return next(row for row in _jsonl("negative-fixtures.jsonl") if row["fixture_id"] == fixture_id)


def _terminal(value: dict[str, Any]) -> query_pb.TerminalEvent:
    return ParseDict(value, query_pb.TerminalEvent(), ignore_unknown_fields=False)


def _response_for_checksum(inputs: dict[str, Any], checksum: str) -> dict[str, Any]:
    results = inputs["daemon_canonical_response_results"]["results"]
    selected = next(row for row in results if row["canonical_response_checksum"] == checksum)
    decoded = json.loads(selected["canonical_json"])
    assert isinstance(decoded, dict)
    return decoded


def test_wp38_claim_017_positive_executes_frozen_released_response_projection() -> None:
    claim = _claim()
    inputs = claim["complete_input_universe"]["inputs"]
    terminal = _terminal(inputs["internal_terminal"])
    response = _response_for_checksum(inputs, terminal.canonical_response_checksum)

    observed = validate_inline_daemon_response(
        canonicalize_value(response), terminal.canonical_response_checksum, terminal
    )

    assert observed == claim["decoded_expectation"]["rows"][0][0]


def test_wp38_claim_017_causal_terminal_selects_frozen_cancelled_response() -> None:
    claim = _claim()
    causal = _fixture("RFV3-FIX-017-C")
    inputs = claim["complete_input_universe"]["inputs"]
    mutation = causal["mutation"]
    assert mutation["input_role"] == "internal_terminal"
    assert inputs["internal_terminal"] == mutation["before"]
    terminal = _terminal(mutation["after"])
    response = _response_for_checksum(inputs, terminal.canonical_response_checksum)

    observed = validate_inline_daemon_response(
        canonicalize_value(response), terminal.canonical_response_checksum, terminal
    )

    assert observed == causal["expected_decoded"]
    assert causal["expected_terminal"] == "changed"


def test_wp38_claim_017_negative_rejects_frozen_candidate_public_projection() -> None:
    claim = _claim()
    negative = _fixture("RFV3-FIX-017-N")
    inputs = claim["complete_input_universe"]["inputs"]
    mutation = negative["mutation"]
    assert mutation["input_role"] == "candidate_released_projection"
    assert mutation["json_pointer"] == ""
    assert inputs["candidate_released_projection"] == mutation["before"]
    assert inputs["redaction_policy"]["physical_names"] == "deny"
    terminal = _terminal(inputs["internal_terminal"])
    daemon_response = _response_for_checksum(inputs, terminal.canonical_response_checksum)
    assert daemon_response == mutation["before"]
    candidate = deepcopy(mutation["after"])
    assert "internal_table" not in mutation["before"]
    assert candidate["internal_table"] == inputs["private_diagnostics"]["internal_table"]
    candidate_bytes = canonicalize_value(candidate)
    # The attempted projection is content-addressed before it reaches the same production
    # boundary; the rejection is about released fields, not a stale checksum.
    candidate_checksum = checksum(candidate_bytes)
    terminal.canonical_response_checksum = candidate_checksum
    with pytest.raises(DaemonProjectionError) as caught:
        validate_inline_daemon_response(candidate_bytes, candidate_checksum, terminal)

    observed = {
        "error": "RELEASED_PROJECTION_FORBIDDEN_FIELD",
        "forbidden_fields": list(caught.value.forbidden_fields),
        "admission_state": "rejected",
    }
    assert caught.value.reason == "released projection contains a forbidden physical-name field"
    assert observed == negative["expected_decoded"]
    assert negative["expected_terminal"] == "reject"
