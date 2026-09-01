"""Validate the frozen v4 supervisor and launcher acceptance contract.

This is a design-input gate.  It selects only RFV4 claims 019 through 023 from
an independently accepted v4 evidence issuance and validates the security and
lifecycle scenarios that later production packets must execute.  It imports no
daemon, adapter, generated-wire, or predecessor-evidence implementation.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import posixpath
import re
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from tooling.ci.successor_evidence_issuance_v4 import (
    EVIDENCE_RELEASE,
    EXPECTATIONS_PATH,
    FIXTURES_PATH,
    ROOT,
    SUITE_IDENTITY,
    V4EvidenceError,
    V4Issuance,
    validate_issuance,
)

ORACLE = "supervisor-launch-contract-check"
SELECTED_CLAIMS = {
    "RFV4-CLAIM-019": "supervisor_policy",
    "RFV4-CLAIM-020": "supervisor_singleton_multi_agent",
    "RFV4-CLAIM-021": "supervisor_control",
    "RFV4-CLAIM-022": "supervisor_fd3",
    "RFV4-CLAIM-023": "supervisor_restart_revocation",
}
FIXTURE_KINDS = ("causal", "negative")
SHA256_ID = re.compile(r"sha256:[0-9a-f]{64}\Z")
FROZEN_EXPECTATIONS_SHA256 = (
    "d9cd74a9cbd4a78f43117b191ef14ddc86957445fc9eb3016db63cf3f5608e7f"
)
FROZEN_FIXTURES_SHA256 = (
    "cce359c558a988ffa104ce4ca463617a79dc08774dc630eec8c8d66613d02d29"
)

DESIGN_PATH = Path(
    "docs/reviews/interface_design_review_daemon_grpc_fastmcp_boundary_2026-09-01_v5.md"
)
SUITE_PATH = Path(
    "docs/authoritative_design/"
    "codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.2.md"
)
LIFE_PATH = Path(
    "docs/authoritative_design/"
    "codefabric_continuous_cpg_update_lifecycle_management_specification_v2.2.md"
)
SRV_PATH = Path(
    "docs/authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.2.md"
)
ROADMAP_PATH = Path(
    "docs/authoritative_design/codefabric_2.2_implementation_roadmap_v1.0.md"
)

REQUIRED_AUTHORITY_TOKENS = {
    DESIGN_PATH: (
        "verdict: aligned",
        "target_status: accepted",
        "`WorkspaceSupervisor`",
        "`AgentLaunchPolicy`",
        "`AgentStdioLauncher`",
        "`codefabric supervisor serve",
        "`codefabric mcp serve",
        "inherited fd 3",
        "`0600` one-shot file",
        "`supervisor-launch-contract-check`",
    ),
    SUITE_PATH: (
        "`WorkspaceSupervisor`",
        "`AgentLaunchPolicy`",
        "AgentStdioLauncher",
        "fd 3",
        "`0600` one-shot file fallback",
        "`supervisor-launch-contract-check`",
    ),
    LIFE_PATH: (
        "`WorkspaceSupervisor`",
        "`AgentLaunchPolicy`",
        "attach-only",
        "PID/start identity",
        "owner, device, and inode evidence",
        "supervisor-launch-contract-check",
    ),
    SRV_PATH: (
        "`WorkspaceSupervisor`",
        "`AgentStdioLauncher`",
        "allowlisted inherited fd 3",
        "directly inherits host stdin/stdout",
        "new single-use grant after daemon generation changes",
        "mode `0600`",
        "supervisor-launch-contract-check",
    ),
    ROADMAP_PATH: (
        "`WorkspaceSupervisor`",
        "`AgentLaunchPolicy`",
        "attach-only",
        "allowlisted fd 3 grant delivery",
        "owner-verified `0600` one-shot fallback",
    ),
}

NEGATIVE_CODES = {
    "RFV4-CLAIM-019": {
        "launcher_claim_override": "POLICY_CLAIM_OVERRIDE",
        "policy_symlink": "POLICY_PATH_UNSAFE",
        "policy_wrong_owner": "POLICY_OWNER_MISMATCH",
        "policy_wrong_mode": "POLICY_MODE_UNSAFE",
        "policy_outside_authorized_root": "POLICY_ROOT_UNAUTHORIZED",
        "peer_pid_start_mismatch": "LAUNCHER_PEER_IDENTITY_MISMATCH",
        "wrong_uid": "LAUNCHER_PEER_UID_MISMATCH",
        "wrong_policy": "POLICY_NOT_AUTHORIZED",
        "wrong_generation": "SUPERVISOR_GENERATION_MISMATCH",
        "wrong_workspace": "WORKSPACE_NOT_AUTHORIZED",
        "wrong_operation": "OPERATION_NOT_AUTHORIZED",
        "capacity_exhausted": "LAUNCH_CAPACITY_EXHAUSTED",
        "policy_wrong_type": "POLICY_FILE_TYPE_INVALID",
        "policy_wrong_device": "POLICY_FILE_IDENTITY_MISMATCH",
        "policy_wrong_inode": "POLICY_FILE_IDENTITY_MISMATCH",
        "policy_not_yet_valid": "POLICY_NOT_YET_VALID",
        "policy_expired": "POLICY_EXPIRED",
        "policy_revoked": "POLICY_REVOKED",
        "request_replayed": "LAUNCH_REQUEST_REPLAY",
        "policy_schema_not_strict": "POLICY_SCHEMA_INVALID",
        "adapter_distribution_mismatch": "ADAPTER_DISTRIBUTION_MISMATCH",
        "adapter_executable_identity_mismatch": "ADAPTER_EXECUTABLE_IDENTITY_MISMATCH",
    },
    "RFV4-CLAIM-020": {
        "socket_parent_symlink": "UNSAFE_SOCKET_PATH",
        "runtime_root_wrong_type": "UNSAFE_RUNTIME_ROOT_TYPE",
        "socket_wrong_type": "UNSAFE_SOCKET_TYPE",
        "socket_wrong_owner": "UNSAFE_SOCKET_OWNER",
        "socket_wrong_mode": "UNSAFE_SOCKET_MODE",
        "cross_device_replacement": "SOCKET_CROSS_DEVICE_REPLACEMENT",
        "live_foreign_socket": "LIVE_SOCKET_OWNERSHIP_CONFLICT",
        "losing_singleton_racer": "SUPERVISOR_SINGLETON_LOST",
        "stale_replacement_inode_cleanup": "REPLACEMENT_SOCKET_IDENTITY_MISMATCH",
        "owned_stale_socket_recovery": None,
        "partial_spawn_before_control_ack": "PARTIAL_DAEMON_SPAWN_ROLLED_BACK",
        "attach_without_supervisor": "SUPERVISOR_UNAVAILABLE",
    },
    "RFV4-CLAIM-021": {
        "sequence_gap": "CONTROL_SEQUENCE_GAP",
        "exact_replay": "CONTROL_EXACT_REPLAY",
        "changed_replay": "CHANGED_CONTROL_REPLAY",
        "unknown_record": "UNKNOWN_CONTROL_RECORD",
        "wrong_workspace": "CONTROL_WORKSPACE_MISMATCH",
        "wrong_daemon_generation": "CONTROL_DAEMON_GENERATION_MISMATCH",
        "wrong_supervisor_generation": "CONTROL_SUPERVISOR_GENERATION_MISMATCH",
        "wrong_operation_identity": "CONTROL_OPERATION_IDENTITY_MISMATCH",
        "expired_record": "CONTROL_RECORD_EXPIRED",
        "content_integrity_mismatch": "CONTROL_CONTENT_INTEGRITY_MISMATCH",
        "record_too_large": "CONTROL_RECORD_TOO_LARGE",
        "semantic_payload_forbidden": "CONTROL_RECORD_SEMANTIC_PAYLOAD_FORBIDDEN",
        "channel_replacement": "CONTROL_CHANNEL_REPLACED",
        "channel_loss": "CONTROL_CHANNEL_LOST",
    },
    "RFV4-CLAIM-022": {
        "capability_oversized": "CAPABILITY_FRAME_TOO_LARGE",
        "ambient_environment": "AMBIENT_CAPABILITY_FORBIDDEN",
        "ambient_argv": "AMBIENT_CAPABILITY_FORBIDDEN",
        "non_allowlisted_descriptor": "INHERITED_DESCRIPTOR_NOT_ALLOWLISTED",
        "wrong_fixed_fd": "CAPABILITY_DESCRIPTOR_MISMATCH",
        "fd3_not_one_way": "CAPABILITY_DESCRIPTOR_DIRECTION_INVALID",
        "replacement_after_terminal_eof": "CAPABILITY_CHANNEL_EOF",
        "fixed_fd3_unavailable_safe_fallback": None,
        "fallback_when_fixed_fd3_available": "CAPABILITY_FALLBACK_NOT_PERMITTED",
        "fallback_symlink": "UNSAFE_CAPABILITY_FALLBACK_PATH",
        "fallback_wrong_owner": "UNSAFE_CAPABILITY_FALLBACK_OWNER",
        "fallback_wrong_mode": "UNSAFE_CAPABILITY_FALLBACK_MODE",
        "fallback_reread": "CAPABILITY_FALLBACK_NOT_SINGLE_USE",
        "fallback_substituted_path": "CAPABILITY_FALLBACK_PATH_REPLACED",
        "fallback_not_immediately_unlinked": "CAPABILITY_FALLBACK_UNLINK_REQUIRED",
        "fallback_capability_logged": "CAPABILITY_LOGGING_FORBIDDEN",
        "fallback_cleanup_leak": "CAPABILITY_FALLBACK_CLEANUP_INCOMPLETE",
        "partial_adapter_spawn": "PARTIAL_ADAPTER_SPAWN_ROLLED_BACK",
        "adapter_early_exit": "ADAPTER_EARLY_EXIT",
        "adapter_signal": "ADAPTER_SIGNALLED",
        "adapter_timeout": "ADAPTER_TIMEOUT",
    },
    "RFV4-CLAIM-023": {
        "old_session_after_supervisor_restart": "SESSION_GENERATION_MISMATCH",
        "old_cursor_after_supervisor_restart": "CURSOR_GENERATION_MISMATCH",
        "implicit_start_after_restart": "IMPLICIT_QUERY_RESUBMISSION_FORBIDDEN",
        "principal_revocation_ignored": "PRINCIPAL_REVOKED",
        "policy_revocation_ignored": "POLICY_REVOKED",
        "old_daemon_child_unreaped": "SUPERVISOR_RESTART_CHILD_NOT_REAPED",
        "restart_without_fresh_authority": "FRESH_GRANT_REQUIRED",
        "accepted_query_cancelled_on_restart": "ACCEPTED_QUERY_IMPLICIT_CANCELLATION_FORBIDDEN",
    },
}
EXPECTED_NEGATIVE_COUNTS = {
    claim_id: len(reason_codes) for claim_id, reason_codes in NEGATIVE_CODES.items()
}
EXPECTED_NEGATIVE_TOTAL = 77
POLICY_BOUND_KEYS = {
    "mcp_host_bounds": {
        "max_request_bytes",
        "max_inline_response_bytes",
        "max_progress_events",
    },
    "resource_bounds": {
        "max_running_queries",
        "max_result_bytes",
        "max_live_resource_pages",
    },
    "deadline_bounds": {"max_execution_budget_ms", "max_cleanup_budget_ms"},
}
ADAPTER_IDENTITY_KEYS = {
    "distribution",
    "distribution_version",
    "executable_observation_supported",
    "executable_sha256",
}


class SupervisorLaunchContractError(ValueError):
    """A fail-closed supervisor/launcher acceptance-contract failure."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def _require(condition: bool, code: str, message: str) -> None:
    if not condition:
        raise SupervisorLaunchContractError(code, message)


def _mapping(value: object, context: str) -> Mapping[str, Any]:
    _require(
        isinstance(value, Mapping),
        "SUPERVISOR_CONTRACT_INVALID",
        f"{context} must be an object",
    )
    assert isinstance(value, Mapping)
    return value


def _list(value: object, context: str) -> list[Any]:
    _require(
        isinstance(value, list),
        "SUPERVISOR_CONTRACT_INVALID",
        f"{context} must be an array",
    )
    assert isinstance(value, list)
    return value


def _field(value: Mapping[str, Any], *path: str) -> Any:
    current: object = value
    for key in path:
        current = _mapping(current, ".".join(path))[key]
    return current


def _under_root(path: object, root: object) -> bool:
    if not isinstance(path, str) or not isinstance(root, str):
        return False
    if not path.startswith("/") or not root.startswith("/"):
        return False
    if ".." in path.split("/") or ".." in root.split("/"):
        return False
    normalized_path = posixpath.normpath(path)
    normalized_root = posixpath.normpath(root)
    if (
        path != normalized_path
        or root != normalized_root
        or normalized_path == normalized_root
    ):
        return False
    try:
        return (
            posixpath.commonpath((normalized_path, normalized_root)) == normalized_root
        )
    except ValueError:
        return False


def _positive_int(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value > 0


def _nonnegative_int(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def _positive_bounds(value: Mapping[str, Any], family: str) -> bool:
    return set(value) == POLICY_BOUND_KEYS[family] and all(
        _positive_int(bound) for bound in value.values()
    )


def _decoded_capability(value: object, context: str) -> bytes:
    _require(
        isinstance(value, str),
        "SUPERVISOR_FD3_INVALID",
        f"{context} must be base64 text",
    )
    try:
        decoded = base64.b64decode(value, validate=True)
    except (ValueError, TypeError) as error:
        raise SupervisorLaunchContractError(
            "SUPERVISOR_FD3_INVALID", f"{context} is not canonical base64"
        ) from error
    _require(
        0 < len(decoded) <= 4096,
        "SUPERVISOR_FD3_INVALID",
        f"{context} is empty or unbounded",
    )
    return decoded


def _selected_rows(
    validated: V4Issuance,
) -> tuple[dict[str, Mapping[str, Any]], dict[tuple[str, str], Mapping[str, Any]]]:
    selected_expectations = [
        row
        for row in validated.expectations
        if row.get("claim_id") in SELECTED_CLAIMS
        or row.get("family") in set(SELECTED_CLAIMS.values())
    ]
    _require(
        bool(selected_expectations),
        "SUPERVISOR_SELECTOR_ZERO_SELECTION",
        "supervisor claim/family selector selected zero claims",
    )
    observed_pairs = [
        (row.get("claim_id"), row.get("family")) for row in selected_expectations
    ]
    expected_pairs = list(SELECTED_CLAIMS.items())
    _require(
        observed_pairs == expected_pairs,
        "SUPERVISOR_SELECTOR_CLOSURE_INVALID",
        f"claim/family selector closure differs: expected={expected_pairs!r} observed={observed_pairs!r}",
    )
    claims = {str(row["claim_id"]): row for row in selected_expectations}
    selected_fixtures = [
        row for row in validated.fixtures if row.get("claim_id") in SELECTED_CLAIMS
    ]
    expected_fixture_pairs = [
        (claim_id, kind) for claim_id in SELECTED_CLAIMS for kind in FIXTURE_KINDS
    ]
    observed_fixture_pairs = [
        (str(row.get("claim_id")), str(row.get("fixture_kind")))
        for row in selected_fixtures
    ]
    _require(
        observed_fixture_pairs == expected_fixture_pairs,
        "SUPERVISOR_SELECTOR_CLOSURE_INVALID",
        "supervisor selector must select exactly one causal and one negative fixture per claim",
    )
    fixtures = {
        (str(row["claim_id"]), str(row["fixture_kind"])): row
        for row in selected_fixtures
    }
    _require(
        bool(fixtures),
        "SUPERVISOR_SELECTOR_ZERO_SELECTION",
        "selector selected zero fixtures",
    )
    return claims, fixtures


def _negative_cases(
    fixture: Mapping[str, Any], claim_id: str
) -> tuple[dict[str, Mapping[str, Any]], dict[str, Mapping[str, Any]]]:
    fixture_input = _mapping(fixture["fixture_input"], f"{claim_id} negative input")
    invalid_change = _mapping(
        fixture_input["invalid_change"], f"{claim_id} invalid_change"
    )
    input_rows = [
        _mapping(row, f"{claim_id} negative case")
        for row in _list(invalid_change["cases"], f"{claim_id} cases")
    ]
    decoded = _mapping(fixture["expected_decoded"], f"{claim_id} negative output")
    output_rows = [
        _mapping(row, f"{claim_id} decoded case")
        for row in _list(decoded["cases"], f"{claim_id} decoded cases")
    ]
    expected = NEGATIVE_CODES[claim_id]
    input_reasons = [str(row.get("reason")) for row in input_rows]
    output_reasons = [str(row.get("reason")) for row in output_rows]
    _require(
        input_reasons == list(expected) and output_reasons == list(expected),
        "SUPERVISOR_NEGATIVE_CLOSURE_INVALID",
        f"{claim_id} negative scenario closure differs",
    )
    outputs = {str(row["reason"]): row for row in output_rows}
    _require(
        all(
            (outputs[reason].get("code") == code)
            if code is not None
            else ("code" not in outputs[reason])
            for reason, code in expected.items()
        ),
        "SUPERVISOR_NEGATIVE_CODE_INVALID",
        f"{claim_id} negative reason/code binding differs",
    )
    return {str(row["reason"]): row for row in input_rows}, outputs


def _validate_policy(
    claim: Mapping[str, Any], causal: Mapping[str, Any], negative: Mapping[str, Any]
) -> int:
    controlled = _mapping(claim["controlled_input"], "supervisor policy input")
    policy = _mapping(controlled["policy"], "launch policy")
    request = _mapping(controlled["launcher_request"], "launcher request")
    policy_file = _mapping(controlled["policy_file"], "policy file")
    peer = _mapping(controlled["launcher_peer"], "launcher peer")
    decoded = _mapping(claim["expected_decoded"], "supervisor policy output")

    policy_identity = {
        "file_type": policy_file.get("file_type"),
        "device": policy_file.get("device"),
        "inode": policy_file.get("inode"),
    }
    replay = _mapping(controlled["anti_replay_registry"], "anti-replay registry")
    host_bounds = _mapping(policy["mcp_host_bounds"], "MCP host bounds")
    resource_bounds = _mapping(policy["resource_bounds"], "resource bounds")
    deadline_bounds = _mapping(policy["deadline_bounds"], "deadline bounds")
    adapter_identity = _mapping(policy["adapter_identity"], "adapter identity")
    observed_adapter = _mapping(controlled["observed_adapter"], "observed adapter")
    _require(
        _under_root(policy_file.get("path"), policy_file.get("authorized_root"))
        and policy_file.get("opened_no_follow") is True
        and policy_file.get("owner_uid") == 0
        and policy_file.get("mode") == "0600"
        and policy_file.get("strict_schema") is True
        and policy_file.get("file_type") == "regular_file"
        and _positive_int(policy_file.get("device"))
        and _positive_int(policy_file.get("inode")),
        "SUPERVISOR_POLICY_FILE_UNSAFE",
        "policy fixture must bind one canonical no-follow regular file under its authorized root",
    )
    _require(
        request.get("policy_id") == policy.get("policy_id")
        and request.get("peer_uid") == peer.get("uid")
        and request.get("peer_pid") == peer.get("pid")
        and request.get("peer_start_time_ticks") == peer.get("start_time_ticks")
        and _positive_int(peer.get("uid"))
        and _positive_int(peer.get("pid"))
        and _positive_int(peer.get("start_time_ticks")),
        "SUPERVISOR_PEER_IDENTITY_INVALID",
        "launcher UID/PID/start identity and policy selection must be exact",
    )
    _decoded_capability(
        request.get("anti_replay_identity"), "launch anti-replay identity"
    )
    _require(
        request.get("supervisor_generation") == controlled.get("supervisor_generation")
        and request.get("requested_workspace")
        in _list(policy.get("workspaces"), "policy workspaces")
        and request.get("requested_operation")
        in _list(policy.get("operations"), "policy operations")
        and _positive_int(controlled.get("launch_capacity_available"))
        and policy.get("issued_at_unix_ms")
        <= policy.get("not_before_unix_ms")
        <= request.get("request_at_unix_ms")
        < policy.get("expires_at_unix_ms")
        and controlled.get("current_revocation_generation")
        == policy.get("revocation_generation")
        and replay.get("identity") == request.get("anti_replay_identity")
        and replay.get("seen") is False,
        "SUPERVISOR_POLICY_AUTHORITY_INVALID",
        "policy selection must bind generation, ACL, time, revocation, replay, and capacity",
    )
    _require(
        decoded.get("outcome") == "authorized"
        and decoded.get("principal") == policy.get("principal")
        and decoded.get("workspaces") == policy.get("workspaces")
        and decoded.get("operations") == policy.get("operations")
        and decoded.get("profiles") == policy.get("profiles")
        and decoded.get("policy_revision") == policy.get("revision")
        and decoded.get("max_launches") == policy.get("max_launches")
        and decoded.get("adapter_claims_authoritative") is False
        and decoded.get("grant_registered") is True
        and decoded.get("policy_file_verified") is True
        and decoded.get("peer_identity_verified") is True
        and decoded.get("launch_capacity_reserved") == 1
        and decoded.get("policy_time_window_verified") is True
        and decoded.get("revocation_generation") == policy.get("revocation_generation")
        and decoded.get("policy_file_identity") == policy_identity
        and decoded.get("anti_replay_identity_accepted") is True
        and _positive_bounds(host_bounds, "mcp_host_bounds")
        and _positive_bounds(resource_bounds, "resource_bounds")
        and _positive_bounds(deadline_bounds, "deadline_bounds")
        and set(adapter_identity) == ADAPTER_IDENTITY_KEYS
        and adapter_identity.get("executable_observation_supported") is True
        and SHA256_ID.fullmatch(str(adapter_identity.get("executable_sha256")))
        is not None
        and observed_adapter
        == {
            key: adapter_identity.get(key)
            for key in ("distribution", "distribution_version", "executable_sha256")
        }
        and decoded.get("mcp_host_bounds") == host_bounds
        and decoded.get("resource_bounds") == resource_bounds
        and decoded.get("deadline_bounds") == deadline_bounds
        and decoded.get("adapter_identity") == adapter_identity
        and decoded.get("adapter_identity_verified") is True
        and decoded.get("strict_schema_verified") is True,
        "SUPERVISOR_POLICY_OUTPUT_INVALID",
        "authorized output must derive all claims from verified policy, file, peer, and replay state",
    )

    causal_input = _mapping(causal["fixture_input"], "policy causal input")
    patch = _mapping(causal_input["merge_patch"], "policy causal patch")
    causal_policy = _mapping(patch["policy"], "policy causal policy")
    causal_request = _mapping(patch["launcher_request"], "policy causal request")
    causal_file = _mapping(patch["policy_file"], "policy causal file")
    causal_output = _mapping(causal["expected_decoded"], "policy causal output")
    causal_replay = _mapping(
        patch["anti_replay_registry"], "causal anti-replay registry"
    )
    causal_observed_adapter = _mapping(
        patch["observed_adapter"], "causal observed adapter"
    )
    causal_host_bounds = _mapping(
        causal_policy["mcp_host_bounds"], "causal MCP host bounds"
    )
    causal_resource_bounds = _mapping(
        causal_policy["resource_bounds"], "causal resource bounds"
    )
    causal_deadline_bounds = _mapping(
        causal_policy["deadline_bounds"], "causal deadline bounds"
    )
    causal_adapter_identity = _mapping(
        causal_policy["adapter_identity"], "causal adapter identity"
    )
    _decoded_capability(
        causal_request.get("anti_replay_identity"), "causal anti-replay identity"
    )
    causal_identity = {
        "file_type": causal_file.get("file_type"),
        "device": causal_file.get("device"),
        "inode": causal_file.get("inode"),
    }
    _require(
        causal_policy.get("policy_id") != policy.get("policy_id")
        and causal_policy.get("policy_id") == causal_request.get("policy_id")
        and causal_request.get("peer_uid") == peer.get("uid")
        and causal_request.get("peer_pid") == peer.get("pid")
        and causal_request.get("peer_start_time_ticks") == peer.get("start_time_ticks")
        and causal_request.get("supervisor_generation")
        == controlled.get("supervisor_generation")
        and causal_request.get("requested_workspace")
        in _list(causal_policy.get("workspaces"), "causal workspaces")
        and causal_request.get("requested_operation")
        in _list(causal_policy.get("operations"), "causal operations")
        and causal_policy.get("issued_at_unix_ms")
        <= causal_policy.get("not_before_unix_ms")
        <= causal_request.get("request_at_unix_ms")
        < causal_policy.get("expires_at_unix_ms")
        and causal_policy.get("revocation_generation")
        == controlled.get("current_revocation_generation")
        and causal_replay.get("identity") == causal_request.get("anti_replay_identity")
        and causal_replay.get("identity") != replay.get("identity")
        and causal_replay.get("seen") is False
        and _under_root(causal_file.get("path"), causal_file.get("authorized_root"))
        and causal_file.get("opened_no_follow") is True
        and causal_file.get("owner_uid") == policy_file.get("owner_uid")
        and causal_file.get("mode") == "0600"
        and causal_file.get("strict_schema") is True
        and causal_file.get("file_type") == "regular_file"
        and _positive_int(causal_file.get("device"))
        and _positive_int(causal_file.get("inode"))
        and causal_identity != policy_identity
        and _positive_int(causal_policy.get("max_launches"))
        and _positive_bounds(causal_host_bounds, "mcp_host_bounds")
        and _positive_bounds(causal_resource_bounds, "resource_bounds")
        and _positive_bounds(causal_deadline_bounds, "deadline_bounds")
        and set(causal_adapter_identity) == ADAPTER_IDENTITY_KEYS
        and causal_adapter_identity.get("executable_observation_supported") is True
        and SHA256_ID.fullmatch(str(causal_adapter_identity.get("executable_sha256")))
        is not None
        and causal_output.get("outcome") == "authorized"
        and causal_output.get("principal") == causal_policy.get("principal")
        and causal_output.get("workspaces") == causal_policy.get("workspaces")
        and causal_output.get("operations") == causal_policy.get("operations")
        and causal_output.get("profiles") == causal_policy.get("profiles")
        and causal_output.get("policy_revision") == causal_policy.get("revision")
        and causal_output.get("max_launches") == causal_policy.get("max_launches")
        and causal_output.get("adapter_claims_authoritative") is False
        and causal_output.get("grant_registered") is True
        and causal_output.get("policy_file_verified") is True
        and causal_output.get("peer_identity_verified") is True
        and causal_output.get("launch_capacity_reserved") == 1
        and causal_output.get("policy_time_window_verified") is True
        and causal_output.get("revocation_generation")
        == causal_policy.get("revocation_generation")
        and causal_output.get("policy_file_identity") == causal_identity
        and causal_output.get("anti_replay_identity_accepted") is True
        and causal_output.get("mcp_host_bounds") == causal_host_bounds
        and causal_output.get("resource_bounds") == causal_resource_bounds
        and causal_output.get("deadline_bounds") == causal_deadline_bounds
        and causal_output.get("adapter_identity") == causal_adapter_identity
        and causal_observed_adapter
        == {
            key: causal_adapter_identity.get(key)
            for key in ("distribution", "distribution_version", "executable_sha256")
        }
        and causal_output.get("adapter_identity_verified") is True
        and causal_output.get("strict_schema_verified") is True,
        "SUPERVISOR_POLICY_CAUSAL_INVALID",
        "causal policy selection must completely and independently change decoded authority",
    )

    cases, outputs = _negative_cases(negative, "RFV4-CLAIM-019")
    _require(
        _field(cases["policy_symlink"], "policy_file", "opened_no_follow") is False,
        "SUPERVISOR_POLICY_NEGATIVE_INVALID",
        "symlink case must defeat no-follow",
    )
    _require(
        _field(cases["policy_wrong_owner"], "policy_file", "owner_uid")
        != policy_file.get("owner_uid"),
        "SUPERVISOR_POLICY_NEGATIVE_INVALID",
        "wrong-owner case must change owner",
    )
    _require(
        _field(cases["policy_wrong_mode"], "policy_file", "mode") != "0600",
        "SUPERVISOR_POLICY_NEGATIVE_INVALID",
        "wrong-mode case must be permissive",
    )
    outside = _mapping(
        _field(cases["policy_outside_authorized_root"], "policy_file"),
        "outside-root policy",
    )
    _require(
        not _under_root(outside.get("path"), outside.get("authorized_root")),
        "SUPERVISOR_POLICY_NEGATIVE_INVALID",
        "outside-root case must escape the authorized root",
    )
    mismatch = _mapping(
        _field(cases["peer_pid_start_mismatch"], "launcher_peer"), "peer mismatch"
    )
    _require(
        mismatch.get("pid") == peer.get("pid")
        and mismatch.get("start_time_ticks") != peer.get("start_time_ticks"),
        "SUPERVISOR_POLICY_NEGATIVE_INVALID",
        "peer case must discriminate PID/start identity",
    )
    wrong_uid = _mapping(cases["wrong_uid"]["launcher_peer"], "wrong UID peer")
    _require(
        wrong_uid.get("uid") != peer.get("uid")
        and _field(cases["wrong_uid"], "launcher_request", "peer_uid")
        == wrong_uid.get("uid"),
        "SUPERVISOR_POLICY_NEGATIVE_INVALID",
        "wrong-UID case must bind the mismatched peer and request",
    )
    _require(
        _field(cases["wrong_policy"], "launcher_request", "policy_id")
        != policy.get("policy_id"),
        "SUPERVISOR_POLICY_NEGATIVE_INVALID",
        "wrong-policy case must select another policy",
    )
    _require(
        _field(cases["wrong_generation"], "launcher_request", "supervisor_generation")
        != controlled.get("supervisor_generation"),
        "SUPERVISOR_POLICY_NEGATIVE_INVALID",
        "generation case must differ",
    )
    _require(
        _field(cases["wrong_workspace"], "launcher_request", "requested_workspace")
        not in policy.get("workspaces", []),
        "SUPERVISOR_POLICY_NEGATIVE_INVALID",
        "workspace case must be unauthorized",
    )
    _require(
        _field(cases["wrong_operation"], "launcher_request", "requested_operation")
        not in policy.get("operations", []),
        "SUPERVISOR_POLICY_NEGATIVE_INVALID",
        "operation case must be unauthorized",
    )
    _require(
        cases["capacity_exhausted"].get("launch_capacity_available") == 0,
        "SUPERVISOR_POLICY_NEGATIVE_INVALID",
        "capacity case must be exhausted",
    )
    _require(
        _field(cases["policy_wrong_type"], "policy_file", "file_type")
        != policy_file.get("file_type")
        and _field(cases["policy_wrong_device"], "policy_file", "device")
        != policy_file.get("device")
        and _field(cases["policy_wrong_device"], "policy_file", "inode")
        == policy_file.get("inode")
        and _field(cases["policy_wrong_inode"], "policy_file", "device")
        == policy_file.get("device")
        and _field(cases["policy_wrong_inode"], "policy_file", "inode")
        != policy_file.get("inode"),
        "SUPERVISOR_POLICY_NEGATIVE_INVALID",
        "policy type/device/inode substitutions must each discriminate exact file identity",
    )
    _require(
        _field(cases["policy_not_yet_valid"], "launcher_request", "request_at_unix_ms")
        < policy.get("not_before_unix_ms")
        and _field(cases["policy_expired"], "launcher_request", "request_at_unix_ms")
        >= policy.get("expires_at_unix_ms")
        and cases["policy_revoked"].get("current_revocation_generation")
        > policy.get("revocation_generation")
        and _field(cases["request_replayed"], "anti_replay_registry", "identity")
        == request.get("anti_replay_identity")
        and _field(cases["request_replayed"], "anti_replay_registry", "seen") is True,
        "SUPERVISOR_POLICY_NEGATIVE_INVALID",
        "time, revocation, and replay cases must cross their controlled authority bounds",
    )
    override = _mapping(
        cases["launcher_claim_override"]["launcher_override"], "launcher claim override"
    )
    _require(
        override.get("principal") != policy.get("principal")
        and set(_list(override.get("operations"), "override operations"))
        - set(_list(policy.get("operations"), "policy operations"))
        and set(_list(override.get("profiles"), "override profiles"))
        - set(_list(policy.get("profiles"), "policy profiles"))
        and override.get("mcp_host_bounds") != host_bounds
        and override.get("resource_bounds") != resource_bounds
        and override.get("deadline_bounds") != deadline_bounds
        and _field(cases["policy_schema_not_strict"], "policy_file", "strict_schema")
        is False,
        "SUPERVISOR_POLICY_NEGATIVE_INVALID",
        "claim/bound overrides and non-strict policy parsing must be rejected",
    )
    distribution_mismatch = _mapping(
        cases["adapter_distribution_mismatch"]["observed_adapter"],
        "adapter distribution mismatch",
    )
    executable_mismatch = _mapping(
        cases["adapter_executable_identity_mismatch"]["observed_adapter"],
        "adapter executable mismatch",
    )
    _require(
        distribution_mismatch.get("distribution")
        != adapter_identity.get("distribution")
        and distribution_mismatch.get("distribution_version")
        == adapter_identity.get("distribution_version")
        and distribution_mismatch.get("executable_sha256")
        == adapter_identity.get("executable_sha256")
        and executable_mismatch.get("distribution")
        == adapter_identity.get("distribution")
        and executable_mismatch.get("distribution_version")
        == adapter_identity.get("distribution_version")
        and executable_mismatch.get("executable_sha256")
        != adapter_identity.get("executable_sha256"),
        "SUPERVISOR_POLICY_NEGATIVE_INVALID",
        "distribution and executable identity mismatches must isolate distinct adapter faults",
    )
    negative_output = _mapping(negative["expected_decoded"], "policy negative output")
    _require(
        negative_output.get("outcome") == "rejected"
        and all(
            output.get("grant_registered") is False
            and output.get("session_minted") is False
            and output.get("adapter_spawned") is False
            and output.get("partial_reservations") == 0
            for output in outputs.values()
        ),
        "SUPERVISOR_POLICY_NEGATIVE_INVALID",
        "policy denial must precede grant, session, spawn, and reservation",
    )
    return len(cases)


def _validate_singleton(
    claim: Mapping[str, Any], causal: Mapping[str, Any], negative: Mapping[str, Any]
) -> int:
    controlled = _mapping(claim["controlled_input"], "singleton input")
    runtime_root = _mapping(controlled["runtime_root"], "runtime root")
    lease = _mapping(controlled["singleton_lease"], "singleton lease")
    socket = _mapping(controlled["supervisor_socket"], "supervisor socket")
    live = _mapping(controlled["live_supervisor"], "live supervisor")
    recovery = _mapping(controlled["socket_recovery_policy"], "socket recovery policy")
    attach_requests = [
        _mapping(row, "attach request")
        for row in _list(controlled["attach_requests"], "attach requests")
    ]
    decoded = _mapping(claim["expected_decoded"], "singleton output")
    _require(
        isinstance(runtime_root.get("path"), str)
        and posixpath.isabs(runtime_root["path"])
        and ".." not in str(runtime_root["path"]).split("/")
        and posixpath.normpath(str(runtime_root["path"])) == runtime_root.get("path")
        and runtime_root.get("authorized_root") is True
        and runtime_root.get("no_symlink_components") is True
        and runtime_root.get("type") == "directory"
        and runtime_root.get("mode") == "0700"
        and _positive_int(runtime_root.get("device"))
        and _under_root(socket.get("path"), runtime_root.get("path"))
        and socket.get("type") == "unix_socket"
        and socket.get("owner_uid") == runtime_root.get("owner_uid")
        and socket.get("mode") == "0600"
        and _positive_int(socket.get("device"))
        and _positive_int(socket.get("inode"))
        and socket.get("generation") == live.get("generation")
        and socket.get("live_probe") == "authenticated",
        "SUPERVISOR_SINGLETON_SOCKET_INVALID",
        "singleton fixture must bind an owned safe runtime root and live exact Unix socket",
    )
    _require(
        lease.get("owned") is True
        and lease.get("winner_pid") == live.get("daemon_pid")
        and _positive_int(lease.get("winner_start_time_ticks"))
        and lease.get("capacity_reserved") is True
        and controlled.get("capacity") == len(attach_requests) == 2
        and [request.get("agent") for request in attach_requests] == ["a", "b"]
        and len({request.get("policy_id") for request in attach_requests}) == 2
        and decoded.get("outcome") == "attached"
        and decoded.get("supervisor_count") == 1
        and decoded.get("daemon_count") == 1
        and decoded.get("adapter_count") == 2
        and decoded.get("grant_count") == 2
        and decoded.get("semantic_state_copies") == 1
        and decoded.get("runtime_root_safe") is True
        and decoded.get("singleton_lease_count") == 1
        and decoded.get("owned_socket_count") == 1
        and decoded.get("losing_racer_mutations") == 0
        and socket.get("cleanup_requires_exact_identity") is True
        and recovery
        == {
            "requires_singleton_lease": True,
            "requires_failed_live_probe": True,
            "requires_exact_type_owner_mode_device_inode_generation": True,
            "replacement_inode_safe_cleanup": True,
        }
        and decoded.get("owned_stale_socket_recovery_supported") is True
        and decoded.get("replacement_inode_safe_cleanup") is True,
        "SUPERVISOR_SINGLETON_INVALID",
        "one supervisor/daemon must serve both bounded agents without duplicate semantic state",
    )
    causal_output = _mapping(causal["expected_decoded"], "singleton causal output")
    _require(
        _field(
            _mapping(causal["fixture_input"], "singleton causal input"),
            "merge_patch",
            "agent_exit",
        )
        == "a"
        and causal_output.get("outcome") == "running"
        and causal_output.get("supervisor_count") == 1
        and causal_output.get("daemon_count") == 1
        and causal_output.get("adapter_count") == 1
        and causal_output.get("surviving_agent") == "b"
        and causal_output.get("singleton_lease_count") == 1
        and causal_output.get("owned_socket_count") == 1
        and causal_output.get("owned_socket_retained") is True
        and causal_output.get("owned_stale_socket_recovery_supported") is True
        and causal_output.get("replacement_inode_safe_cleanup") is True,
        "SUPERVISOR_SINGLETON_CAUSAL_INVALID",
        "one agent exit must not tear down the shared supervisor or daemon",
    )
    cases, outputs = _negative_cases(negative, "RFV4-CLAIM-020")
    _require(
        _field(cases["socket_parent_symlink"], "runtime_root", "no_symlink_components")
        is False
        and outputs["socket_parent_symlink"].get("daemon_spawn_count") == 0,
        "SUPERVISOR_SINGLETON_NEGATIVE_INVALID",
        "socket symlink case is absent",
    )
    _require(
        _field(cases["runtime_root_wrong_type"], "runtime_root", "type")
        != runtime_root.get("type")
        and outputs["runtime_root_wrong_type"].get("daemon_spawn_count") == 0,
        "SUPERVISOR_SINGLETON_NEGATIVE_INVALID",
        "wrong runtime-root type must prevent daemon spawn",
    )
    _require(
        _field(cases["socket_wrong_type"], "supervisor_socket", "type")
        != socket.get("type")
        and outputs["socket_wrong_type"].get("unlinked_paths") == 0,
        "SUPERVISOR_SINGLETON_NEGATIVE_INVALID",
        "wrong socket type must not be unlinked",
    )
    _require(
        _field(cases["socket_wrong_owner"], "supervisor_socket", "owner_uid")
        != socket.get("owner_uid")
        and outputs["socket_wrong_owner"].get("unlinked_paths") == 0,
        "SUPERVISOR_SINGLETON_NEGATIVE_INVALID",
        "wrong-owner socket must not be unlinked",
    )
    cross_device = cases["cross_device_replacement"]
    recorded = _mapping(cross_device["recorded_socket"], "recorded socket")
    observed = _mapping(cross_device["observed_socket"], "observed socket")
    _require(
        recorded == {"device": socket.get("device"), "inode": socket.get("inode")}
        and observed.get("device") != recorded.get("device")
        and observed.get("inode") == recorded.get("inode")
        and outputs["cross_device_replacement"].get("unlinked_paths") == 0
        and outputs["cross_device_replacement"].get("signalled_processes") == 0,
        "SUPERVISOR_SINGLETON_NEGATIVE_INVALID",
        "cross-device replacement must preserve the observed object without signalling",
    )
    _require(
        _field(cases["socket_wrong_mode"], "supervisor_socket", "mode") != "0600"
        and outputs["socket_wrong_mode"].get("unlinked_paths") == 0,
        "SUPERVISOR_SINGLETON_NEGATIVE_INVALID",
        "wrong-mode socket must not be unlinked",
    )
    foreign = _mapping(
        _field(cases["live_foreign_socket"], "supervisor_socket"), "foreign socket"
    )
    _require(
        foreign.get("live_probe") == "authenticated_foreign"
        and foreign.get("device") != socket.get("device")
        and foreign.get("inode") != socket.get("inode")
        and outputs["live_foreign_socket"].get("unlinked_paths") == 0
        and outputs["live_foreign_socket"].get("signalled_processes") == 0,
        "SUPERVISOR_SINGLETON_NEGATIVE_INVALID",
        "foreign live device/inode must survive without unlink or signal",
    )
    racer = _mapping(cases["losing_singleton_racer"]["singleton_lease"], "losing racer")
    _require(
        racer.get("owned") is False
        and racer.get("winner_pid") == lease.get("winner_pid")
        and racer.get("winner_start_time_ticks") == lease.get("winner_start_time_ticks")
        and outputs["losing_singleton_racer"].get("winner_mutations") == 0
        and outputs["losing_singleton_racer"].get("unlinked_paths") == 0
        and outputs["losing_singleton_racer"].get("signalled_processes") == 0,
        "SUPERVISOR_SINGLETON_NEGATIVE_INVALID",
        "losing racer must not mutate the winner",
    )
    replacement_case = cases["stale_replacement_inode_cleanup"]
    replacement_recorded = _mapping(
        replacement_case["recorded_socket"], "shutdown recorded socket"
    )
    shutdown_observed = _mapping(
        replacement_case["shutdown_observed_socket"], "shutdown observed socket"
    )
    _require(
        replacement_recorded
        == {
            "device": socket.get("device"),
            "inode": socket.get("inode"),
            "generation": socket.get("generation"),
        }
        and shutdown_observed.get("device") == replacement_recorded.get("device")
        and shutdown_observed.get("inode") != replacement_recorded.get("inode")
        and shutdown_observed.get("generation")
        != replacement_recorded.get("generation")
        and outputs["stale_replacement_inode_cleanup"].get("outcome") == "preserved"
        and outputs["stale_replacement_inode_cleanup"].get("unlinked_paths") == 0
        and outputs["stale_replacement_inode_cleanup"].get(
            "observed_replacement_preserved"
        )
        is True,
        "SUPERVISOR_SINGLETON_NEGATIVE_INVALID",
        "shutdown must preserve a replacement inode rather than unlink by stale pathname",
    )
    stale_case = cases["owned_stale_socket_recovery"]
    stale_lease = _mapping(stale_case["singleton_lease"], "stale recovery lease")
    stale_socket = _mapping(stale_case["stale_socket"], "owned stale socket")
    replacement_socket = _mapping(
        stale_case["replacement_socket"], "stale recovery replacement"
    )
    stale_output = outputs["owned_stale_socket_recovery"]
    _require(
        stale_lease.get("owned") is True
        and stale_socket.get("type") == socket.get("type")
        and stale_socket.get("owner_uid") == socket.get("owner_uid")
        and stale_socket.get("mode") == socket.get("mode")
        and stale_socket.get("device") == socket.get("device")
        and stale_socket.get("inode") != socket.get("inode")
        and stale_socket.get("generation") < socket.get("generation")
        and stale_socket.get("live_probe") == "failed"
        and replacement_socket
        == {
            key: socket.get(key)
            for key in ("type", "owner_uid", "mode", "device", "inode", "generation")
        }
        and stale_output.get("outcome") == "recovered"
        and stale_output.get("unlinked_paths") == 1
        and stale_output.get("unlinked_socket_identity")
        == {key: stale_socket.get(key) for key in ("device", "inode", "generation")}
        and stale_output.get("new_socket_identity")
        == {
            key: replacement_socket.get(key)
            for key in ("device", "inode", "generation")
        },
        "SUPERVISOR_SINGLETON_NEGATIVE_INVALID",
        "only an exact owned stale socket with failed live probe may be replaced",
    )
    _require(
        cases["partial_spawn_before_control_ack"].get("spawn_stage")
        == "child_started_control_unacknowledged"
        and outputs["partial_spawn_before_control_ack"].get("child_reaped") is True
        and outputs["partial_spawn_before_control_ack"].get("owned_socket_cleaned")
        is True
        and outputs["partial_spawn_before_control_ack"].get("singleton_lease_released")
        is True
        and outputs["partial_spawn_before_control_ack"].get("partial_state_count") == 0,
        "SUPERVISOR_SINGLETON_NEGATIVE_INVALID",
        "partial daemon spawn must reap and release all owned state",
    )
    _require(
        cases["attach_without_supervisor"].get("supervisor_rendezvous") == "absent"
        and cases["attach_without_supervisor"].get("launcher_action") == "spawn_daemon"
        and outputs["attach_without_supervisor"].get("daemon_spawn_count") == 0,
        "SUPERVISOR_SINGLETON_NEGATIVE_INVALID",
        "attach-only absence must never spawn a per-agent daemon",
    )
    negative_output = _mapping(
        negative["expected_decoded"], "singleton negative output"
    )
    _require(
        negative_output.get("outcome") == "case_matrix"
        and all(
            output.get("outcome")
            == (
                "recovered"
                if reason == "owned_stale_socket_recovery"
                else "preserved"
                if reason == "stale_replacement_inode_cleanup"
                else "rejected"
            )
            for reason, output in outputs.items()
        ),
        "SUPERVISOR_SINGLETON_NEGATIVE_INVALID",
        "singleton negative cases must retain their exact fail-closed outcomes",
    )
    return len(cases)


def _validate_control(
    claim: Mapping[str, Any], causal: Mapping[str, Any], negative: Mapping[str, Any]
) -> int:
    controlled = _mapping(claim["controlled_input"], "control input")
    records = [
        _mapping(row, "control record")
        for row in _list(controlled["records"], "control records")
    ]
    decoded = _mapping(claim["expected_decoded"], "control output")
    workspace = controlled.get("workspace")
    daemon_generation = controlled.get("daemon_generation")
    supervisor_generation = controlled.get("supervisor_generation")
    governed_max = controlled.get("governed_max_record_bytes")
    operation_identities = {
        "AdvanceSupervisorGeneration": "advance-supervisor-generation@1",
        "RegisterLaunchGrant": "register-launch-grant@1",
        "Acknowledgement": "acknowledgement@1",
        "RevokePrincipal": "revoke-principal@1",
    }

    def record_is_bound(record: Mapping[str, Any]) -> bool:
        operation = record.get("operation")
        length = record.get("declared_length_bytes")
        return (
            record.get("workspace") == workspace
            and record.get("daemon_generation") == daemon_generation
            and record.get("supervisor_generation") == supervisor_generation
            and record.get("operation_identity") == operation_identities.get(operation)
            and _positive_int(record.get("expires_at_unix_ms"))
            and _positive_int(length)
            and isinstance(governed_max, int)
            and length <= governed_max
            and SHA256_ID.fullmatch(str(record.get("content_sha256"))) is not None
        )

    channel = _mapping(controlled["channel"], "control channel")
    _require(
        _positive_int(supervisor_generation)
        and _positive_int(daemon_generation)
        and _positive_int(governed_max)
        and [row.get("sequence") for row in records] == [1, 2, 3]
        and [row.get("operation") for row in records]
        == ["AdvanceSupervisorGeneration", "RegisterLaunchGrant", "Acknowledgement"]
        and all(record_is_bound(row) for row in records)
        and len({row.get("content_sha256") for row in records}) == len(records)
        and SHA256_ID.fullmatch(str(records[1].get("grant_digest"))) is not None
        and channel
        == {
            "transport": "unnamed_socketpair",
            "channel_generation": supervisor_generation,
            "state": "healthy",
        }
        and decoded.get("outcome") == "accepted"
        and decoded.get("next_sequence") == 4
        and decoded.get("semantic_payload_records") == 0
        and decoded.get("record_bindings_verified") is True
        and decoded.get("max_record_bytes_observed")
        == max(row["declared_length_bytes"] for row in records)
        and decoded.get("channel_state") == channel.get("state"),
        "SUPERVISOR_CONTROL_INVALID",
        "control records must bind the derived workspace/generations/operations/bounds/channel",
    )
    causal_records = _list(
        _field(
            _mapping(causal["fixture_input"], "control causal input"),
            "merge_patch",
            "records",
        ),
        "control causal records",
    )
    causal_output = _mapping(causal["expected_decoded"], "control causal output")
    causal_bound = [_mapping(row, "causal control record") for row in causal_records]
    fourth = causal_bound[-1]
    _require(
        len(causal_bound) == 4
        and [row.get("sequence") for row in causal_bound] == [1, 2, 3, 4]
        and all(record_is_bound(row) for row in causal_bound)
        and causal_bound[:3] == records
        and fourth.get("operation") == "RevokePrincipal"
        and fourth.get("principal") == "principal:a"
        and causal_output.get("outcome") == "accepted"
        and causal_output.get("next_sequence") == 5
        and causal_output.get("revoked") == ["principal:a"]
        and causal_output.get("registered_grants") == decoded.get("registered_grants")
        and causal_output.get("semantic_payload_records") == 0
        and causal_output.get("record_bindings_verified") is True
        and causal_output.get("max_record_bytes_observed")
        == max(row["declared_length_bytes"] for row in causal_bound)
        and causal_output.get("channel_state") == channel.get("state"),
        "SUPERVISOR_CONTROL_CAUSAL_INVALID",
        "causal revoke record must preserve all derived bindings and advance exactly once",
    )
    cases, outputs = _negative_cases(negative, "RFV4-CLAIM-021")
    next_sequence = decoded.get("next_sequence")
    sequence_gap = _mapping(cases["sequence_gap"]["record"], "sequence gap")
    exact_replay = _mapping(cases["exact_replay"]["record"], "exact replay")
    replay = _mapping(cases["changed_replay"]["record"], "changed replay")
    unknown = _mapping(cases["unknown_record"]["record"], "unknown record")
    wrong_workspace = _mapping(cases["wrong_workspace"]["record"], "wrong workspace")
    wrong_daemon = _mapping(
        cases["wrong_daemon_generation"]["record"], "wrong daemon generation"
    )
    wrong_supervisor = _mapping(
        cases["wrong_supervisor_generation"]["record"], "wrong supervisor generation"
    )
    wrong_operation = _mapping(
        cases["wrong_operation_identity"]["record"], "wrong operation identity"
    )
    expired = _mapping(cases["expired_record"]["record"], "expired record")
    integrity = _mapping(
        cases["content_integrity_mismatch"]["record"], "integrity mismatch"
    )
    semantic = _mapping(
        cases["semantic_payload_forbidden"]["record"], "semantic record"
    )
    oversized = _mapping(cases["record_too_large"]["record"], "oversized record")
    replacement_channel = _mapping(
        cases["channel_replacement"]["channel"], "replacement channel"
    )
    lost_channel = _mapping(cases["channel_loss"]["channel"], "lost channel")
    negative_output = _mapping(negative["expected_decoded"], "control negative output")
    _require(
        sequence_gap.get("sequence") > next_sequence
        and exact_replay == records[1]
        and outputs["exact_replay"].get("outcome") == "replayed"
        and outputs["exact_replay"].get("new_mutations") == 0
        and outputs["exact_replay"].get("next_sequence") == next_sequence
        and replay.get("sequence") == exact_replay.get("sequence")
        and replay != exact_replay
        and replay.get("grant_digest") != records[1].get("grant_digest")
        and unknown.get("operation") not in operation_identities
        and wrong_workspace.get("workspace") != workspace
        and wrong_daemon.get("daemon_generation") != daemon_generation
        and wrong_supervisor.get("supervisor_generation") != supervisor_generation
        and wrong_operation.get("operation") in operation_identities
        and wrong_operation.get("operation_identity")
        != operation_identities[wrong_operation["operation"]]
        and expired.get("expires_at_unix_ms")
        < cases["expired_record"].get("received_at_unix_ms")
        and integrity.get("content_sha256")
        != cases["content_integrity_mismatch"].get("observed_content_sha256")
        and semantic.get("operation") == "SemanticQueryPayload"
        and oversized.get("declared_length_bytes") == governed_max + 1
        and outputs["record_too_large"].get("governed_max_record_bytes") == governed_max
        and replacement_channel.get("transport") == channel.get("transport")
        and replacement_channel.get("channel_generation")
        != channel.get("channel_generation")
        and replacement_channel.get("state") == "replacement"
        and lost_channel
        == {
            "transport": channel.get("transport"),
            "channel_generation": channel.get("channel_generation"),
            "state": "lost",
        }
        and outputs["channel_loss"].get("outcome") == "degraded_draining"
        and outputs["channel_loss"].get("new_handshakes") == "closed"
        and outputs["channel_loss"].get("grant_renewals") == "closed"
        and outputs["channel_loss"].get("accepted_pinned_work") == "survives"
        and outputs["channel_loss"].get("implicit_cancellation") is False
        and negative_output.get("outcome") == "case_matrix"
        and negative_output.get("failed_cases_new_handshakes") == "closed",
        "SUPERVISOR_CONTROL_NEGATIVE_INVALID",
        "every replay, binding, bound, semantic, replacement, and loss scenario must derive from the base control contract",
    )
    _require(
        all(
            output.get("outcome")
            == (
                "replayed"
                if reason == "exact_replay"
                else "degraded_draining"
                if reason == "channel_loss"
                else "rejected"
            )
            for reason, output in outputs.items()
        ),
        "SUPERVISOR_CONTROL_NEGATIVE_INVALID",
        "control negative cases must retain their exact replay, drain, or rejection outcomes",
    )
    return len(cases)


def _validate_fallback_platform_condition(
    controlled: Mapping[str, Any],
    cases: Mapping[str, Mapping[str, Any]],
    outputs: Mapping[str, Mapping[str, Any]],
) -> None:
    condition_value = controlled.get("platform_descriptor_capability")
    _require(
        isinstance(condition_value, Mapping),
        "SUPERVISOR_FALLBACK_PLATFORM_CONDITION_MISSING",
        "RFV4-CLAIM-022 lacks an explicit process-probed fixed-fd3 platform decision",
    )
    condition = _mapping(condition_value, "platform descriptor capability")
    _require(
        condition
        == {
            "fixed_fd3_inheritance_available": True,
            "probe_status": "passed",
            "selected_transport": "fixed_fd3",
        }
        and _field(controlled, "one_shot_fallback", "enabled") is False,
        "SUPERVISOR_FALLBACK_PLATFORM_CONDITION_INVALID",
        "passing fixed-fd3 evidence must select fixed fd3 and keep fallback disabled",
    )
    unavailable = cases["fixed_fd3_unavailable_safe_fallback"]
    unavailable_platform = _mapping(
        unavailable["platform_descriptor_capability"], "unavailable platform capability"
    )
    unavailable_fallback = _mapping(
        unavailable["one_shot_fallback"], "authorized fallback"
    )
    unavailable_descriptors = _mapping(
        unavailable["child_descriptors"], "fallback child descriptors"
    )
    unavailable_output = outputs["fixed_fd3_unavailable_safe_fallback"]
    _require(
        unavailable_platform
        == {
            "fixed_fd3_inheritance_available": False,
            "probe_status": "unavailable",
            "selected_transport": "one_shot_file",
        }
        and unavailable_fallback.get("enabled") is True
        and unavailable_fallback.get("opened_no_follow") is True
        and unavailable_fallback.get("owner_uid")
        == _field(controlled, "one_shot_fallback", "owner_uid")
        and unavailable_fallback.get("mode") == "0600"
        and unavailable_fallback.get("path_device")
        == unavailable_fallback.get("opened_device")
        and unavailable_fallback.get("path_inode")
        == unavailable_fallback.get("opened_inode")
        and unavailable_fallback.get("max_reads") == 1
        and unavailable_fallback.get("unlinked_immediately_after_open") is True
        and unavailable_fallback.get("capability_logged") is False
        and unavailable_fallback.get("cleanup_complete") is True
        and unavailable_descriptors
        == {
            "stdin": "host-stdin",
            "stdout": "host-stdout",
            "stderr": "bounded-pipe",
            "fd3": None,
        }
        and unavailable_output.get("outcome") == "delivered_via_fallback"
        and unavailable_output.get("selected_transport") == "one_shot_file"
        and unavailable_output.get("fallback_used") is True
        and unavailable_output.get("capability_read_count") == 1
        and unavailable_output.get("second_read_bytes") == 0
        and unavailable_output.get("inherited_extra_fds") == []
        and unavailable_output.get("opened_identity")
        == {
            "device": unavailable_fallback.get("opened_device"),
            "inode": unavailable_fallback.get("opened_inode"),
        }
        and unavailable_output.get("unlinked_immediately_after_open") is True
        and unavailable_output.get("path_visible_after_open") is False
        and unavailable_output.get("capability_logged") is False
        and unavailable_output.get("cleanup_complete") is True
        and unavailable_output.get("adapter_spawned") is True
        and unavailable_output.get("fd3_inherited") is False
        and unavailable_output.get("direct_host_stdio") is True,
        "SUPERVISOR_FALLBACK_PLATFORM_CONDITION_INVALID",
        "fixed-fd3 unavailability must be the sole condition selecting the exact safe fallback",
    )
    unnecessary = cases["fallback_when_fixed_fd3_available"]
    _require(
        "platform_descriptor_capability" not in unnecessary
        and _field(unnecessary, "one_shot_fallback", "enabled") is True
        and outputs["fallback_when_fixed_fd3_available"].get("adapter_spawned")
        is False,
        "SUPERVISOR_FALLBACK_PLATFORM_CONDITION_INVALID",
        "fallback must be rejected while inherited fixed fd3 remains available",
    )
    unsafe_fallback_reasons = (
        "fallback_symlink",
        "fallback_wrong_owner",
        "fallback_wrong_mode",
        "fallback_reread",
        "fallback_substituted_path",
        "fallback_not_immediately_unlinked",
        "fallback_capability_logged",
        "fallback_cleanup_leak",
    )
    for reason in unsafe_fallback_reasons:
        scenario = cases[reason]
        scenario_condition_value = scenario.get("platform_descriptor_capability")
        _require(
            isinstance(scenario_condition_value, Mapping),
            "SUPERVISOR_FALLBACK_PLATFORM_EVIDENCE_MISSING",
            f"{reason} lacks fixed-fd3-unavailable platform evidence",
        )
        _require(
            _mapping(
                scenario_condition_value, f"{reason} platform descriptor capability"
            )
            == unavailable_platform
            and _field(scenario, "one_shot_fallback", "enabled") is True,
            "SUPERVISOR_FALLBACK_PLATFORM_CONDITION_INVALID",
            f"{reason} is not conditioned on fixed-fd3 unavailability",
        )


def _validate_fd3(
    claim: Mapping[str, Any], causal: Mapping[str, Any], negative: Mapping[str, Any]
) -> int:
    controlled = _mapping(claim["controlled_input"], "fd3 input")
    descriptors = _mapping(controlled["child_descriptors"], "child descriptors")
    policy = _mapping(controlled["descriptor_policy"], "descriptor policy")
    fallback = _mapping(controlled["one_shot_fallback"], "one-shot fallback")
    spawn = _mapping(controlled["adapter_spawn"], "adapter spawn")
    stdio = _mapping(controlled["stdio_policy"], "stdio policy")
    stderr = _mapping(controlled["stderr_policy"], "stderr policy")
    decoded = _mapping(claim["expected_decoded"], "fd3 output")
    first_capability = _decoded_capability(
        controlled.get("capability"), "launch capability"
    )
    _require(
        descriptors
        == {
            "stdin": "host-stdin",
            "stdout": "host-stdout",
            "stderr": "bounded-pipe",
            "fd3": "capability-channel",
        }
        and controlled.get("argv_contains_capability") is False
        and controlled.get("environment_contains_capability") is False
        and policy.get("capability_fd") == 3
        and policy.get("allowlisted_inherited_fds") == [0, 1, 2, 3]
        and policy.get("unrelated_descriptors_close_on_exec") is True
        and policy.get("fd3_direction") == "supervisor_to_adapter_only"
        and policy.get("frame_contract") == "length_bounded_generation_labelled"
        and policy.get("fd3_channel_open_after_delivery") is True
        and policy.get("parent_writer_open_for_replacement") is True
        and policy.get("child_fd3_open_for_replacement") is True
        and policy.get("terminal_eof_observed") is False
        and stdio
        == {
            "stdin": "direct_host",
            "stdout": "direct_host",
            "launcher_proxy_copies": 0,
        },
        "SUPERVISOR_FD3_INVALID",
        "adapter must directly inherit host STDIO and one bounded unidirectional fd-3 channel",
    )
    capability_length = controlled.get("capability_length_bytes")
    capability_max = controlled.get("capability_max_bytes")
    _require(
        len(first_capability) == capability_length
        and _positive_int(capability_length)
        and _positive_int(capability_max)
        and capability_length <= capability_max
        and controlled.get("issued_random_bits") == capability_length * 8,
        "SUPERVISOR_FD3_INVALID",
        "capability bytes, declared length, maximum, and issued entropy must agree",
    )
    fallback_path = fallback.get("path")
    _require(
        isinstance(fallback_path, str)
        and posixpath.isabs(fallback_path)
        and ".." not in fallback_path.split("/")
        and posixpath.normpath(fallback_path) == fallback_path
        and fallback.get("enabled") is False
        and fallback.get("authorized_root") is True
        and fallback.get("opened_no_follow") is True
        and fallback.get("owner_uid") == 1000
        and fallback.get("mode") == "0600"
        and fallback.get("path_device") == fallback.get("opened_device")
        and fallback.get("path_inode") == fallback.get("opened_inode")
        and fallback.get("unlinked_after_open") is True
        and fallback.get("max_reads") == 1
        and fallback.get("unlinked_immediately_after_open") is True
        and fallback.get("capability_logged") is False
        and fallback.get("cleanup_complete") is True,
        "SUPERVISOR_FALLBACK_INVALID",
        "the disabled fallback contract must still define exact no-follow owner/mode/inode single-use safety",
    )
    _require(
        spawn.get("stage") == "ready"
        and _positive_int(spawn.get("child_pid"))
        and spawn.get("process_group_owned") is True
        and spawn.get("grant_registered") is True
        and spawn.get("diagnostic_tasks_joined") is True
        and spawn.get("descriptor_cleanup_complete") is True
        and _positive_int(stderr.get("byte_limit"))
        and _nonnegative_int(stderr.get("emitted_bytes"))
        and _nonnegative_int(stderr.get("forwarded_bytes"))
        and _nonnegative_int(stderr.get("truncated_bytes"))
        and stderr.get("forwarded_bytes") == stderr.get("byte_limit")
        and stderr.get("emitted_bytes")
        == stderr.get("forwarded_bytes") + stderr.get("truncated_bytes")
        and stderr.get("accounting_complete") is True
        and decoded.get("outcome") == "delivered"
        and decoded.get("read_count") == 1
        and decoded.get("fd") == 3
        and decoded.get("reusable") is False
        and decoded.get("unrelated_inherited_fds") == []
        and decoded.get("stdout_proxy_copies") == 0
        and decoded.get("fallback_used") is False
        and decoded.get("adapter_processes") == 1
        and decoded.get("grant_registered_before_delivery") is True
        and decoded.get("capability_bytes") == capability_length
        and decoded.get("capability_max_bytes") == capability_max
        and decoded.get("fd3_direction") == policy.get("fd3_direction")
        and decoded.get("frame_contract") == policy.get("frame_contract")
        and decoded.get("fd3_channel_open") is True
        and decoded.get("parent_writer_open_for_replacement") is True
        and decoded.get("child_fd3_open_for_replacement") is True
        and decoded.get("terminal_eof_observed") is False
        and decoded.get("further_handshake_authority_available") is True
        and decoded.get("selected_transport") == "fixed_fd3"
        and decoded.get("direct_host_stdio") is True
        and decoded.get("stderr_forwarded_bytes") == stderr.get("forwarded_bytes")
        and decoded.get("stderr_truncated_bytes") == stderr.get("truncated_bytes")
        and decoded.get("stderr_accounting_complete") is True
        and decoded.get("capability_logged") is False
        and decoded.get("descriptor_cleanup_complete") is True,
        "SUPERVISOR_FD3_OUTPUT_INVALID",
        "fd-3 delivery must be one-shot, registered-before-delivery, direct, and leak-free",
    )
    causal_patch = _mapping(
        _field(_mapping(causal["fixture_input"], "fd3 causal input"), "merge_patch"),
        "fd3 causal patch",
    )
    replacement = _decoded_capability(
        causal_patch.get("replacement_capability"), "replacement capability"
    )
    causal_output = _mapping(causal["expected_decoded"], "fd3 causal output")
    _require(
        replacement != first_capability
        and len(replacement) == causal_output.get("replacement_capability_bytes")
        and len(replacement) <= capability_max
        and causal_patch.get("daemon_generation")
        == controlled.get("daemon_generation", 0) + 1
        and causal_output.get("outcome") == "delivered"
        and causal_output.get("fd") == 3
        and causal_output.get("replacement_read_count") == 1
        and causal_output.get("replacement_frame_daemon_generation")
        == causal_patch.get("daemon_generation")
        and causal_output.get("old_capability_valid") is False
        and causal_output.get("frame_contract") == policy.get("frame_contract")
        and causal_output.get("fd3_channel_open") is True
        and causal_output.get("parent_writer_open_for_replacement") is True
        and causal_output.get("child_fd3_open_for_replacement") is True
        and causal_output.get("terminal_eof_observed") is False
        and causal_output.get("further_handshake_authority_available") is True
        and causal_output.get("selected_transport") == "fixed_fd3"
        and causal_output.get("direct_host_stdio") is True
        and causal_output.get("stderr_forwarded_bytes") == stderr.get("forwarded_bytes")
        and causal_output.get("stderr_truncated_bytes") == stderr.get("truncated_bytes")
        and causal_output.get("stderr_accounting_complete") is True
        and causal_output.get("capability_logged") is False
        and causal_output.get("descriptor_cleanup_complete") is True,
        "SUPERVISOR_REGRANT_INVALID",
        "generation change must deliver one distinct replacement on fd 3 and invalidate the old grant",
    )
    cases, outputs = _negative_cases(negative, "RFV4-CLAIM-022")
    _validate_fallback_platform_condition(controlled, cases, outputs)
    _require(
        cases["capability_oversized"].get("capability_length_bytes")
        == capability_max + 1
        and outputs["capability_oversized"].get("capability_delivered") is False,
        "SUPERVISOR_FD3_NEGATIVE_INVALID",
        "oversized capability must derive from the governed maximum and deliver no bytes",
    )
    _require(
        cases["ambient_environment"].get("environment_contains_capability") is True,
        "SUPERVISOR_FD3_NEGATIVE_INVALID",
        "environment leakage case is absent",
    )
    _require(
        _field(cases["fd3_not_one_way"], "descriptor_policy", "fd3_direction")
        != policy.get("fd3_direction")
        and outputs["fd3_not_one_way"].get("capability_delivered") is False,
        "SUPERVISOR_FD3_NEGATIVE_INVALID",
        "bidirectional fd3 must deliver no capability",
    )
    terminal_case = cases["replacement_after_terminal_eof"]
    terminal_policy = _mapping(
        terminal_case["descriptor_policy"], "terminal EOF descriptor state"
    )
    terminal_output = outputs["replacement_after_terminal_eof"]
    _require(
        terminal_case.get("replacement_capability")
        == causal_patch.get("replacement_capability")
        and terminal_case.get("daemon_generation")
        == causal_patch.get("daemon_generation")
        and terminal_policy
        == {
            "terminal_eof_observed": True,
            "fd3_channel_open_after_delivery": False,
            "parent_writer_open_for_replacement": False,
            "child_fd3_open_for_replacement": False,
        }
        and set(terminal_policy) <= set(policy)
        and terminal_output.get("replacement_capability_delivered") is False
        and terminal_output.get("further_handshake_authority_available") is False
        and terminal_output.get("parent_writer_closed") is True
        and terminal_output.get("child_fd3_closed") is True,
        "SUPERVISOR_FD3_NEGATIVE_INVALID",
        "terminal EOF must permanently close replacement-grant authority and both endpoints",
    )
    _require(
        cases["ambient_argv"].get("argv_contains_capability") is True,
        "SUPERVISOR_FD3_NEGATIVE_INVALID",
        "argv leakage case is absent",
    )
    _require(
        _field(
            cases["fallback_not_immediately_unlinked"],
            "one_shot_fallback",
            "unlinked_immediately_after_open",
        )
        is False
        and outputs["fallback_not_immediately_unlinked"].get("path_visible") is False
        and _field(
            cases["fallback_capability_logged"],
            "one_shot_fallback",
            "capability_logged",
        )
        is True
        and outputs["fallback_capability_logged"].get("logged_capability_bytes") == 0
        and _field(
            cases["fallback_cleanup_leak"],
            "one_shot_fallback",
            "cleanup_complete",
        )
        is False
        and outputs["fallback_cleanup_leak"].get("cleanup_completed") is True,
        "SUPERVISOR_FALLBACK_NEGATIVE_INVALID",
        "fallback unlink, logging, and cleanup faults must leave no visible or logged capability",
    )
    _require(
        _field(
            cases["non_allowlisted_descriptor"],
            "descriptor_policy",
            "allowlisted_inherited_fds",
        )
        != [0, 1, 2, 3],
        "SUPERVISOR_FD3_NEGATIVE_INVALID",
        "descriptor allowlist leakage case is absent",
    )
    _require(
        _field(cases["wrong_fixed_fd"], "descriptor_policy", "capability_fd") != 3,
        "SUPERVISOR_FD3_NEGATIVE_INVALID",
        "wrong fixed-fd case is absent",
    )
    _require(
        _field(cases["fallback_symlink"], "one_shot_fallback", "opened_no_follow")
        is False,
        "SUPERVISOR_FALLBACK_NEGATIVE_INVALID",
        "fallback symlink case is absent",
    )
    _require(
        _field(cases["fallback_wrong_owner"], "one_shot_fallback", "owner_uid")
        != fallback.get("owner_uid"),
        "SUPERVISOR_FALLBACK_NEGATIVE_INVALID",
        "fallback wrong-owner case is absent",
    )
    _require(
        _field(cases["fallback_wrong_mode"], "one_shot_fallback", "mode") != "0600",
        "SUPERVISOR_FALLBACK_NEGATIVE_INVALID",
        "fallback wrong-mode case is absent",
    )
    _require(
        _field(cases["fallback_reread"], "one_shot_fallback", "max_reads") > 1
        and outputs["fallback_reread"].get("second_read_bytes") == 0,
        "SUPERVISOR_FALLBACK_NEGATIVE_INVALID",
        "fallback reread must return zero bytes",
    )
    substituted = _mapping(
        cases["fallback_substituted_path"]["one_shot_fallback"], "substituted fallback"
    )
    _require(
        (substituted.get("path_device"), substituted.get("path_inode"))
        != (substituted.get("opened_device"), substituted.get("opened_inode"))
        and outputs["fallback_substituted_path"].get("capability_delivered") is False,
        "SUPERVISOR_FALLBACK_NEGATIVE_INVALID",
        "substituted fallback device/inode must deliver no capability",
    )
    exit_stages = {
        "partial_adapter_spawn": "child_started_before_grant_delivery",
        "adapter_early_exit": "early_exit",
        "adapter_signal": "signalled",
        "adapter_timeout": "timeout",
    }
    for reason, stage in exit_stages.items():
        failed_spawn = _mapping(cases[reason]["adapter_spawn"], f"{reason} spawn")
        failed_output = outputs[reason]
        _require(
            failed_spawn.get("stage") == stage
            and _positive_int(failed_spawn.get("child_pid"))
            and failed_spawn.get("process_group_owned") is True
            and failed_spawn.get("grant_registered") is True
            and failed_output.get("grant_revoked") is True
            and failed_output.get("parent_writer_closed") is True
            and failed_output.get("child_fd3_closed") is True
            and failed_output.get("child_reaped") is True
            and failed_output.get("lifecycle_tasks_joined") is True
            and failed_output.get("adapter_processes") == 0,
            "SUPERVISOR_PARTIAL_ADAPTER_INVALID",
            f"{reason} must revoke, close descriptors, join, and reap",
        )
    _require(
        all(
            outputs[reason].get("adapter_spawned") is False
            for reason in (
                "ambient_environment",
                "ambient_argv",
                "non_allowlisted_descriptor",
                "wrong_fixed_fd",
                "fallback_symlink",
                "fallback_wrong_owner",
                "fallback_wrong_mode",
            )
        ),
        "SUPERVISOR_FD3_NEGATIVE_INVALID",
        "unsafe capability delivery must be denied before adapter spawn",
    )
    negative_output = _mapping(negative["expected_decoded"], "fd3 negative output")
    _require(
        negative_output.get("outcome") == "case_matrix"
        and negative_output.get("direct_host_stdio_preserved") is True
        and negative_output.get("ambient_inherited_fds") == []
        and negative_output.get("safe_fallback_required") is True
        and negative_output.get("fixed_fd3_preferred_when_available") is True,
        "SUPERVISOR_FD3_NEGATIVE_INVALID",
        "negative matrix must preserve direct host STDIO and prohibit ambient descriptors",
    )
    _require(
        all(
            output.get("outcome")
            == (
                "delivered_via_fallback"
                if reason == "fixed_fd3_unavailable_safe_fallback"
                else "rejected"
            )
            for reason, output in outputs.items()
        ),
        "SUPERVISOR_FD3_NEGATIVE_INVALID",
        "fd3 negative cases must retain exact fallback or rejection outcomes",
    )
    return len(cases)


def _validate_restart(
    claim: Mapping[str, Any], causal: Mapping[str, Any], negative: Mapping[str, Any]
) -> int:
    controlled = _mapping(claim["controlled_input"], "restart input")
    decoded = _mapping(claim["expected_decoded"], "restart output")
    old_grant = controlled.get("old_grant")
    old_session = controlled.get("old_session")
    old_cursor = controlled.get("old_cursor")
    fresh_grant = controlled.get("fresh_grant")
    _decoded_capability(old_grant, "old grant")
    _decoded_capability(old_session, "old session")
    _decoded_capability(old_cursor, "old cursor")
    _decoded_capability(fresh_grant, "fresh grant")
    old_process = _mapping(controlled["old_daemon_process"], "old daemon process")
    restart_policy = _mapping(controlled["restart_policy"], "restart policy")
    _require(
        controlled.get("event") == "supervisor_restart"
        and controlled.get("supervisor_restart") is True
        and controlled.get("daemon_restart") is False
        and controlled.get("new_supervisor_generation")
        == controlled.get("old_supervisor_generation", 0) + 1
        and controlled.get("new_daemon_generation")
        == controlled.get("old_daemon_generation", 0) + 1
        and old_process.get("state") == "running"
        and old_process.get("process_group_owned") is True
        and _positive_int(old_process.get("pid"))
        and _positive_int(controlled.get("replacement_daemon_pid"))
        and controlled.get("replacement_daemon_pid") != old_process.get("pid")
        and _positive_int(restart_policy.get("drain_timeout_ms"))
        and _positive_int(restart_policy.get("kill_timeout_ms"))
        and all(
            restart_policy.get(key) is True
            for key in (
                "join_lifecycle_tasks",
                "reap_owned_children",
                "cleanup_owned_socket_by_exact_identity",
                "reacquire_singleton_lease",
            )
        )
        and controlled.get("retained_terminal") is True
        and controlled.get("revocation_targets") == []
        and decoded.get("outcome") == "supervisor_restarted"
        and decoded.get("old_supervisor_generation")
        == controlled.get("old_supervisor_generation")
        and decoded.get("new_supervisor_generation")
        == controlled.get("new_supervisor_generation")
        and decoded.get("old_daemon_generation")
        == controlled.get("old_daemon_generation")
        and decoded.get("new_daemon_generation")
        == controlled.get("new_daemon_generation")
        and decoded.get("invalidated") == [old_grant, old_session, old_cursor]
        and decoded.get("old_daemon_pid") == old_process.get("pid")
        and decoded.get("replacement_daemon_pid")
        == controlled.get("replacement_daemon_pid")
        and decoded.get("old_daemon_child_joined") is True
        and decoded.get("old_daemon_child_reaped") is True
        and decoded.get("old_process_group_remaining") == 0
        and decoded.get("lifecycle_tasks_joined") is True
        and decoded.get("owned_socket_cleanup_complete") is True
        and decoded.get("singleton_lease_reacquired") is True
        and decoded.get("orphan_children") == 0
        and decoded.get("query_resubmitted") is False
        and decoded.get("reconnect_action") == "fresh_grant_session_then_watch_query"
        and decoded.get("retained_query") == controlled.get("accepted_query")
        and decoded.get("fresh_grant_required") is True
        and decoded.get("fresh_grant") == fresh_grant
        and decoded.get("accepted_query_survives") is True
        and decoded.get("start_query_calls") == 0
        and decoded.get("watch_resume_only") is True,
        "SUPERVISOR_RESTART_INVALID",
        "supervisor restart must join/reap the old daemon, advance generations, and resume accepted work without resubmission",
    )
    causal_patch = _mapping(
        _field(
            _mapping(causal["fixture_input"], "restart causal input"), "merge_patch"
        ),
        "restart causal patch",
    )
    causal_output = _mapping(causal["expected_decoded"], "restart causal output")
    _require(
        causal_patch.get("event") == "principal_policy_revocation"
        and causal_patch.get("supervisor_restart") is False
        and causal_patch.get("daemon_restart") is False
        and causal_patch.get("new_supervisor_generation")
        == controlled.get("old_supervisor_generation")
        and causal_patch.get("new_daemon_generation")
        == controlled.get("old_daemon_generation")
        and causal_patch.get("replacement_daemon_pid") is None
        and causal_patch.get("restart_policy") is None
        and causal_patch.get("revocation_targets")
        == [
            {"kind": "principal", "id": controlled.get("principal")},
            {"kind": "policy", "id": controlled.get("policy_id")},
        ]
        and causal_output.get("outcome") == "authority_revoked"
        and causal_output.get("revoked")
        == [controlled.get("principal"), controlled.get("policy_id")]
        and causal_output.get("invalidated") == [old_grant, old_session, old_cursor]
        and causal_output.get("replacement_authority_required") is True
        and causal_output.get("fresh_grant_required") is True
        and causal_output.get("accepted_query") == controlled.get("accepted_query")
        and causal_output.get("accepted_query_survives") is True
        and causal_output.get("query_cancelled") is False
        and causal_output.get("start_query_calls") == 0
        and causal_output.get("watch_resume_requires_fresh_authority") is True
        and causal_output.get("supervisor_restart") is False
        and causal_output.get("daemon_restart") is False
        and causal_output.get("restart_lifecycle_actions") == 0
        and causal_output.get("old_daemon_pid") == old_process.get("pid")
        and causal_output.get("old_daemon_remains_owned") is True
        and causal_output.get("fresh_grant") == fresh_grant,
        "SUPERVISOR_RESTART_CAUSAL_INVALID",
        "principal/policy revocation must invalidate volatile authority without cancelling accepted work",
    )
    cases, outputs = _negative_cases(negative, "RFV4-CLAIM-023")
    negative_output = _mapping(negative["expected_decoded"], "restart negative output")
    _require(
        cases["old_session_after_supervisor_restart"].get("present_session")
        == old_session
        and cases["old_session_after_supervisor_restart"].get("supervisor_generation")
        == controlled.get("new_supervisor_generation")
        and cases["old_cursor_after_supervisor_restart"].get("present_cursor")
        == old_cursor
        and cases["old_cursor_after_supervisor_restart"].get("supervisor_generation")
        == controlled.get("new_supervisor_generation")
        and cases["implicit_start_after_restart"].get("implementation_action")
        == "StartQuery"
        and cases["principal_revocation_ignored"].get("revoked_principal")
        == controlled.get("principal")
        and cases["principal_revocation_ignored"].get("present_session") == old_session
        and cases["policy_revocation_ignored"].get("revoked_policy")
        == controlled.get("policy_id")
        and cases["policy_revocation_ignored"].get("present_session") == old_session
        and _mapping(
            cases["old_daemon_child_unreaped"]["restart_observation"],
            "unreaped daemon observation",
        )
        == {
            "old_daemon_child_joined": False,
            "old_daemon_child_reaped": False,
            "orphan_children": 1,
        }
        and outputs["old_daemon_child_unreaped"].get("old_daemon_child_joined") is True
        and outputs["old_daemon_child_unreaped"].get("old_daemon_child_reaped") is True
        and outputs["old_daemon_child_unreaped"].get("orphan_children") == 0
        and cases["restart_without_fresh_authority"].get("fresh_grant_available")
        is False
        and outputs["restart_without_fresh_authority"].get("session_minted") is False
        and cases["accepted_query_cancelled_on_restart"].get("implementation_action")
        == "CancelQuery"
        and negative_output.get("outcome") == "rejected"
        and all(
            output.get("accepted_query_survives") is True
            and output.get("start_query_calls") == 0
            and (
                output.get("implicit_cancellation") is False
                or "implicit_cancellation" not in output
            )
            for output in outputs.values()
        ),
        "SUPERVISOR_RESTART_NEGATIVE_INVALID",
        "restart and revocation faults must preserve accepted work without implicit StartQuery",
    )
    return len(cases)


def _validate_authority_tokens(root: Path) -> dict[str, int]:
    texts: dict[Path, str] = {}
    for path in REQUIRED_AUTHORITY_TOKENS:
        try:
            texts[path] = (root / path).read_text(encoding="utf-8")
        except (OSError, UnicodeError) as error:
            raise SupervisorLaunchContractError(
                "SUPERVISOR_AUTHORITY_UNREADABLE",
                f"cannot read accepted authority {path}: {error}",
            ) from error
    return _validate_required_authority_tokens(texts)


def _validate_frozen_artifact_hashes(root: Path) -> dict[str, str]:
    expected = {
        EXPECTATIONS_PATH: FROZEN_EXPECTATIONS_SHA256,
        FIXTURES_PATH: FROZEN_FIXTURES_SHA256,
    }
    observed: dict[str, str] = {}
    for path, digest in expected.items():
        try:
            actual = hashlib.sha256((root / path).read_bytes()).hexdigest()
        except OSError as error:
            raise SupervisorLaunchContractError(
                "SUPERVISOR_FROZEN_ARTIFACT_UNREADABLE",
                f"cannot read frozen supervisor evidence {path}: {error}",
            ) from error
        _require(
            actual == digest,
            "SUPERVISOR_FROZEN_ARTIFACT_DRIFT",
            f"frozen supervisor evidence digest differs for {path}",
        )
        observed[path.as_posix()] = actual
    return observed


def _validate_required_authority_tokens(texts: Mapping[Path, str]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for path, tokens in REQUIRED_AUTHORITY_TOKENS.items():
        text = " ".join(texts.get(path, "").split())
        missing = [token for token in tokens if " ".join(token.split()) not in text]
        _require(
            not missing,
            "SUPERVISOR_AUTHORITY_TOKEN_MISSING",
            f"accepted authority {path} is missing required tokens: {missing!r}",
        )
        counts[path.as_posix()] = len(tokens)
    return counts


def validate_selected_contract(root: Path, validated: V4Issuance) -> dict[str, object]:
    """Validate the exact supervisor slice of one already-decoded v4 issuance."""

    _require(
        validated.issuance.get("status") == "accepted",
        "SUPERVISOR_REVIEW_REQUIRED",
        "supervisor contract requires an independently accepted v4 issuance",
    )
    try:
        claims, fixtures = _selected_rows(validated)
        scenario_counts = {
            "RFV4-CLAIM-019": _validate_policy(
                claims["RFV4-CLAIM-019"],
                fixtures[("RFV4-CLAIM-019", "causal")],
                fixtures[("RFV4-CLAIM-019", "negative")],
            ),
            "RFV4-CLAIM-020": _validate_singleton(
                claims["RFV4-CLAIM-020"],
                fixtures[("RFV4-CLAIM-020", "causal")],
                fixtures[("RFV4-CLAIM-020", "negative")],
            ),
            "RFV4-CLAIM-021": _validate_control(
                claims["RFV4-CLAIM-021"],
                fixtures[("RFV4-CLAIM-021", "causal")],
                fixtures[("RFV4-CLAIM-021", "negative")],
            ),
            "RFV4-CLAIM-022": _validate_fd3(
                claims["RFV4-CLAIM-022"],
                fixtures[("RFV4-CLAIM-022", "causal")],
                fixtures[("RFV4-CLAIM-022", "negative")],
            ),
            "RFV4-CLAIM-023": _validate_restart(
                claims["RFV4-CLAIM-023"],
                fixtures[("RFV4-CLAIM-023", "causal")],
                fixtures[("RFV4-CLAIM-023", "negative")],
            ),
        }
    except SupervisorLaunchContractError:
        raise
    except (KeyError, IndexError, TypeError) as error:
        raise SupervisorLaunchContractError(
            "SUPERVISOR_CONTRACT_INVALID",
            f"supervisor expectation slice is incomplete or malformed: {error}",
        ) from error
    _require(
        scenario_counts == EXPECTED_NEGATIVE_COUNTS
        and sum(scenario_counts.values()) == EXPECTED_NEGATIVE_TOTAL,
        "SUPERVISOR_NEGATIVE_CLOSURE_INVALID",
        f"supervisor selector must retain the exact ordered {EXPECTED_NEGATIVE_TOTAL}-scenario negative matrix",
    )
    frozen_artifacts = _validate_frozen_artifact_hashes(root.resolve())
    authority_tokens = _validate_authority_tokens(root.resolve())
    return {
        "oracle": ORACLE,
        "status": "accepted",
        "suite": SUITE_IDENTITY,
        "evidence_release": EVIDENCE_RELEASE,
        "selector": {
            "claim_ids": list(SELECTED_CLAIMS),
            "families": list(SELECTED_CLAIMS.values()),
            "fixture_kinds": list(FIXTURE_KINDS),
        },
        "selected_claims": len(claims),
        "selected_fixtures": len(fixtures),
        "selected_negative_scenarios": sum(scenario_counts.values()),
        "negative_scenarios_by_claim": scenario_counts,
        "negative_reason_code_closure": {
            claim_id: [
                {"reason": reason, "code": code}
                for reason, code in reason_codes.items()
            ]
            for claim_id, reason_codes in NEGATIVE_CODES.items()
        },
        "frozen_artifact_sha256": frozen_artifacts,
        "authority_token_counts": authority_tokens,
    }


def validate_supervisor_launch_contract(root: Path = ROOT) -> dict[str, object]:
    """Require accepted v4 issuance closure, then validate its supervisor slice."""

    validated = validate_issuance(root.resolve(), require_review=True)
    return validate_selected_contract(root, validated)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args(argv)
    try:
        report = validate_supervisor_launch_contract(ROOT)
    except (SupervisorLaunchContractError, V4EvidenceError) as error:
        print(
            json.dumps(
                {
                    "oracle": ORACLE,
                    "status": "rejected",
                    "code": error.code,
                    "message": str(error),
                },
                sort_keys=True,
                separators=(",", ":"),
            ),
            file=sys.stderr,
        )
        return 1
    print(json.dumps(report, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
